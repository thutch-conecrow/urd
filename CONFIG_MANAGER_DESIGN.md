# urd — Project Design Document

A CLI tool for managing configuration and secrets across multiple projects and environments. Built in Rust, using `age` encryption (via `rage`) for secure, git-native storage.

The name comes from Urð, the Norse Norn who tends the Well of Urd (Urðarbrunnr) — the source of truth that nourishes Yggdrasil. She represents "what has been established," fitting for a canonical config store.

## Problem Statement

Managing environment configuration across multiple projects is painful for solo developers and small teams. The problem has several dimensions:

1. **Provisioning** — When you sign up for a new service or create a new resource, where does the API key / URL / config value go? There's no single entry point. You end up updating a password manager, a Consul KV store, template files, and generation scripts — all separately.

2. **Documentation** — What does each config value do? Where did it come from? Which components use it? This knowledge lives in the developer's head, not in the system.

3. **Distribution** — How do projects consume their configuration locally? Different frameworks expect different filenames (`.env`, `.env.local`), different key naming conventions (`NEXT_PUBLIC_` prefixes), and the same underlying value may appear under different key names in different components.

4. **Topology** — Local development isn't just "dev" or "prod." It's a mix-and-match of local and remote components: local frontend talking to prod API, local API talking to prod database, everything local, etc. The configuration system needs to support these combinations.

5. **Cognitive load** — When you only touch this system infrequently, you shouldn't have to remember where things go, how templates work, or which scripts to run. The tooling should guide you.

## Background and Prior Art

### What this replaces

The current approach uses HashiCorp Consul as a KV store, `consul-template` for rendering `.env` files from HCL templates, and a set of bash scripts that generate and sync files to sibling project repos. This works but doesn't scale:

- Adding a new env var requires updating Consul, updating templates, and updating scripts
- No metadata or documentation attached to config values
- No concept of config items as first-class entities that span components
- Topology support is ad-hoc (hardcoded `prod-local` environment branches in templates)
- Requires a running Consul server

### Configuration management lineage

This is fundamentally a **configuration management** problem — the same class of problem that Chef, Puppet, and Ansible solve for server fleets. The conceptual model maps directly:

| This tool | Chef equivalent | Puppet equivalent |
|-----------|----------------|-------------------|
| Config Catalog | Cookbooks | Manifests |
| Store (values) | Data Bags | Hiera |
| Component Manifests | Node Roles | Node Classification |
| Topologies | Environments | Environments |
| Assembly | Convergence Run | Catalog Application |

But those tools are designed for managing fleets of servers. This tool applies the same conceptual model at a much smaller scale: a solo developer managing `.env` files across a handful of local projects.

### Config stores considered

| Store | Why not |
|-------|---------|
| Consul | Overkill — requires running a server for what amounts to tens/hundreds of keys |
| etcd | Even more infrastructure-oriented (Kubernetes backing store) |
| ZooKeeper | Heaviest option |
| Redis | Ephemeral by default, not purpose-built for config |
| SQLite | Possible but no encryption story, no structured diff |
| Doppler/Infisical | SaaS cost or self-hosting overhead for a solo dev |

### Encryption approach: SOPS-style selective encryption

The store file uses **selective encryption**: keys and structure remain in plaintext, only sensitive values are encrypted. This makes the file:

- **Git-diffable** — you can see which keys changed, in which environments, without exposing secret values
- **Human-readable** — the file structure serves as documentation
- **Single-file** — no consistency problems between separate changelog and data files

Encryption uses the **age** format via the **rage** Rust library:
- `age` was designed by Filippo Valsorda (former Go crypto team at Google) as a modern replacement for GPG
- Uses X25519 key exchange, ChaCha20-Poly1305 symmetric encryption, HKDF key derivation
- Spec is public, small, and intentionally simple
- `rage` is an interoperable Rust implementation by str4d (Zcash cryptography engineer)
- No custom crypto — all encryption/decryption delegated to rage

## System Architecture

The system has five components:

### 1. Config Catalog (Schema)

Defines every configuration **item** as a first-class entity. An item is the atomic unit — one logical piece of configuration, independent of which components consume it or what env var names they use.

Each item has:
- **ID** — namespaced identifier (e.g., `paddle.personal_credit_5.product_id`)
- **Description** — what this value is, what it's for
- **Sensitivity** — `plaintext`, `sensitive`, or `secret` (determines encryption behavior)
- **Origin** — where the value comes from (e.g., "Paddle dashboard > Product Catalog", "Supabase project settings")
- **Environments** — which environments this item applies to (e.g., `[dev, prod]`, or `[all]`)
- **Tags** — optional metadata for filtering/searching (e.g., `vendor:paddle`, `scope:payments`)

The catalog is **declarative** and lives in the consuming repo. It defines what the store should contain for this project.

### 2. Store (Values)

The stateful component. Holds actual values keyed by config item ID and environment. This is the single encrypted file that lives in a git repo.

The store provides the CLI interface for:
- **Provisioning** — adding a new value (ideally with an EAS-like interactive TUI: prompts for name, sensitivity, value, which environments)
- **Updating** — changing a value
- **Listing/searching** — enumerating items, filtering by tag, environment, sensitivity
- **Validation** — diffing the catalog against the store to find missing values, orphaned entries, etc.

The store file format uses selective encryption: plaintext keys/structure, encrypted sensitive values. Example:

```yaml
paddle.personal_credit_5.product_id:
  prod: ENC[age,data:abc123...]
  dev: "pro_test_xyz"  # sandbox ID, not sensitive
paddle.api_key:
  prod: ENC[age,data:def456...]
  dev: ENC[age,data:ghi789...]
supabase.url:
  prod: "https://myproject.supabase.co"
  dev: "http://localhost:54321"
```

### 3. Component Manifests

Per-component declarations that map config item IDs to environment variable names. Each component declares what it needs and how it names things.

Lives in the consuming repo alongside the component. Example:

```yaml
# augur/api/env.manifest.yaml
target: ".env"
vars:
  PADDLE_PERSONAL_PRICE_5: paddle.personal_credit_5.price_id
  PADDLE_API_KEY: paddle.api_key
  DATABASE_URL: supabase.database_url
  SUPABASE_JWT_SECRET: supabase.jwt_secret

# augur/console-web/env.manifest.yaml
target: ".env.local"
vars:
  NEXT_PUBLIC_PADDLE_PERSONAL_PRICE_5: paddle.personal_credit_5.price_id
  NEXT_PUBLIC_SUPABASE_URL: supabase.url
  NEXT_PUBLIC_SUPABASE_ANON_KEY: supabase.anon_key
```

This makes explicit the mapping that currently only exists in the developer's head: the same `paddle.personal_credit_5.price_id` config item appears as `PADDLE_PERSONAL_PRICE_5` in the API and `NEXT_PUBLIC_PADDLE_PERSONAL_PRICE_5` in the console.

### 4. Topologies

Named presets that define, for each component, which environment's values to resolve against. This is what enables mix-and-match local development setups.

Lives in the consuming repo. Example:

```yaml
# topologies.yaml
all-local:
  api: dev
  console-web: dev
  landing: dev

prod-backend:
  api: prod
  console-web: prod
  landing: prod

hybrid:
  api: dev         # local API...
  console-web: dev
  landing: dev
  # ...but api's supabase.* items resolve against prod
  overrides:
    api:
      supabase.*: prod
```

### 5. Assembly

The engine that reads a topology, resolves each component's manifest against the store using the appropriate environment, evaluates dynamic expressions, and writes the `.env` files.

Assembly handles:
- **Value resolution** — look up each manifest entry in the store for the topology-selected environment
- **Dynamic expressions** — some values can't come from the store. Examples:
  - Local IP detection (for Expo/React Native, which needs a LAN-accessible address)
  - URL construction from other values (e.g., `http://${local_ip}:${port}/graphql`)
  - Values derived from running infrastructure (e.g., Supabase CLI output)
- **Validation** — warn about missing values, type mismatches, unreferenced items
- **File writing** — write the correct file (`.env`, `.env.local`, etc.) for each component

Invocation would look like:

```bash
# Set up local dev environment with everything local
urd assemble --topology all-local

# Set up hybrid: local frontends, local API, prod Supabase
urd assemble --topology hybrid

# Or shorthand scripts in the repo
./use-local    # wraps urd assemble --topology all-local
./use-prod     # wraps urd assemble --topology prod-backend
```

## CLI Design

The CLI should have an interactive/TUI provisioning flow inspired by Expo's `eas env:create`:

```
$ urd set
? Config item ID: paddle.personal_credit_5.product_id
? Description: Paddle product ID for $5 personal credit pack
? Sensitivity: (plaintext / sensitive / secret) → secret
? Origin: Paddle dashboard > Product Catalog
? Value: pro_abc123
? Environments: [x] dev  [x] prod  [ ] staging
✓ Stored paddle.personal_credit_5.product_id for dev, prod
```

Other commands:

```bash
# Store operations
urd set                          # interactive provisioning (TUI)
urd set <id> --env prod <value>  # non-interactive set
urd get <id> --env dev           # retrieve a value
urd list                         # list all items
urd list --tag vendor:paddle     # filter by tag
urd remove <id>                  # remove an item

# Validation
urd validate                     # diff catalog against store, report gaps

# Assembly
urd assemble --topology <name>   # generate .env files for all components
urd assemble --component api     # generate for one component only

# Key management
urd keys init                    # generate a new age keypair
urd keys export                  # export public key (for sharing)
```

## Technology Choices

| Concern | Choice | Rationale |
|---------|--------|-----------|
| Language | Rust | Fast, single binary, strong ecosystem for CLI tools |
| Encryption | rage (age format) | Trusted format, Rust-native library, no custom crypto |
| CLI framework | TBD (clap, etc.) | |
| TUI | TBD (ratatui, dialoguer, inquire, etc.) | For interactive provisioning flow |
| File format | YAML or TOML | Human-readable, git-diffable, supports selective encryption |
| Store location | Git repo (encrypted file) | No server, push/pull for sync, history via git |

## Design Decisions

### Name: `urd`

See header.

### Store location: local + global (npm-style)

Both local (within a project repo, e.g., `.urd/store.yaml`) and global (`~/.config/urd/store.yaml`). The `-g` flag specifies global, mirroring `npm install -g`:

```bash
urd set paddle.api_key              # stores in local project store
urd set -g paddle.api_key           # stores in global store
```

Resolution: local overrides global. When assembling, `urd` checks the local store first, falls back to global. Shared values like `supabase.url` live globally; a project can override if needed.

### Multi-project store: solved by local + global

The global store holds cross-project values. Local stores hold project-specific ones. No namespacing scheme needed — item IDs provide natural namespacing.

### Catalog placement: local + global (same model as store)

Both local and global catalogs, following the same resolution model. Some config items are shared across projects (global catalog), others are project-specific (local catalog). `urd validate` diffs both catalogs against both stores.

### Dynamic expressions: Handlebars syntax

Handlebars `{{expression}}` syntax for dynamic values in manifests. Visually distinct from shell variables and won't collide with `${ENV_VAR}` syntax in output files.

### Component scope summary

| Component | Scope | Rationale |
|-----------|-------|-----------|
| Catalog | Local + global | Some items are shared across projects, others are project-specific |
| Store | Local + global | Same reasoning; local overrides global |
| Component Manifests | Always local | Per-component by nature |
| Topologies | Always local | Describe a specific project's component arrangement |
