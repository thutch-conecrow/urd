# urd

A CLI tool for managing configuration and secrets across projects. Encrypted values live in a git-friendly YAML file alongside human-readable metadata. An interactive TUI lets you browse and edit your store, and an assembly engine generates `.env` files for each component in your project.

Named after [Urð](https://en.wikipedia.org/wiki/Ur%C3%B0r), the Norse Norn who tends the Well of Urd — the source of truth that nourishes Yggdrasil.

## Install

Requires [Rust](https://rustup.rs/) 1.85+ (edition 2024).

```bash
git clone <repo-url> && cd config
./install.sh
```

Or directly:

```bash
cargo install --path .
```

## Quick start

```bash
# Initialize encryption for this project
urd keys init

# Store some values
urd set supabase.url -e dev http://localhost:54321
urd set supabase.url -e prod https://myproject.supabase.co
urd set stripe.secret_key -e prod --secret sk_live_abc123

# Or use interactive mode — walks you through every field
urd set

# Bulk import from an existing .env file
urd import .env --env dev

# View your store
urd list
urd get supabase.url
urd get stripe.secret_key --env prod --reveal

# Launch the TUI for browsing and editing
urd
```

## Concepts

### Store

The store is a single YAML file (`.urd/store.yaml`) containing all your config items. Each item has per-environment values and optional catalog metadata. Sensitive values are encrypted in-place using AES-256-GCM — keys and structure stay in plaintext so the file remains git-diffable.

```yaml
supabase.url:
  description: Supabase API URL
  sensitivity: plaintext
  origin: Supabase dashboard
  environments: [dev, prod]
  tags: [vendor:supabase]
  dev: http://localhost:54321
  prod: https://myproject.supabase.co
stripe.secret_key:
  description: Stripe secret key (server-side only)
  sensitivity: secret
  environments: [dev, prod]
  dev: ENC[aes:secret,base64data...]
  prod: ENC[aes:secret,base64data...]
```

### Catalog metadata

Each item can carry documentation alongside its values:

- **description** — what this value is
- **sensitivity** — `plaintext`, `sensitive`, or `secret`
- **origin** — where to obtain the value (e.g., "Stripe dashboard > API keys")
- **environments** — which envs this item applies to
- **tags** — for filtering (e.g., `vendor:stripe`, `scope:billing`)

```bash
urd catalog add stripe.secret_key \
  -d "Stripe secret key (server-side only)" \
  -s secret \
  -o "Stripe dashboard > Developers > API keys" \
  -e dev -e prod \
  -t vendor:stripe -t scope:billing

urd catalog list
urd catalog show stripe.secret_key
```

### Sensitivity levels

| Level | Stored as | Behavior |
|-------|-----------|----------|
| `plaintext` | Raw text | Visible everywhere |
| `sensitive` | `ENC[aes:sensitive,...]` | Redacted in `get`/`list`, revealed with `--reveal` |
| `secret` | `ENC[aes:secret,...]` | Same redaction, signals higher classification |

### Import

Import loads values from an existing `.env` or YAML file into the store for a given environment in one shot — no need to `urd set` each key individually.

```bash
# From a .env file
urd import .env --env dev

# From YAML (auto-detected by extension)
urd import config.yaml --env dev

# From stdin
cat .env | urd import - --env dev

# Encrypt all imported values as secret
urd import vendor-keys.env --env prod --secret

# Don't overwrite keys that already exist
urd import production.env --env prod --skip-existing
```

**Format auto-detection:** `.yaml`/`.yml` extensions are parsed as YAML (flat `key: value` string maps). Everything else is parsed as dotenv. Use `--format dotenv` or `--format yaml` to override.

**Encryption priority per key:** explicit `--secret`/`--sensitive` flag > catalog metadata > plaintext. This means if an item is already marked `secret` in the catalog, its value is encrypted automatically even without `--secret`.

### Assembly

Assembly generates `.env` files by resolving component definitions against the store through a topology. Components can use either **templates** or **manifests** — assembly discovers whichever is present.

#### Templates

Templates are `.env` files with `{{ item.id }}` expressions. Everything else passes through verbatim — comments, blank lines, hardcoded values.

```bash
# api/.env.template
# target: .env
NODE_ENV=dev
PORT=3002
HOST=0.0.0.0
LOG_LEVEL=info

# Database (Supabase)
DATABASE_URL={{ supabase.database_url }}
SUPABASE_URL={{ supabase.url }}
SUPABASE_SERVICE_ROLE_KEY={{ supabase.service_role_key }}

# Stripe
STRIPE_SECRET_KEY={{ stripe.secret_key }}
STRIPE_WEBHOOK_SECRET={{ stripe.webhook_secret }}
# STRIPE_PRICE_PRO_MONTHLY={{ stripe.price_id.pro_monthly }}
```

The output file target is set via a `# target: <path>` frontmatter line, or inferred by stripping `.template` from the filename (`.env.template` → `.env`).

Commented-out lines containing `{{ }}` expressions are still resolved — `# STRIPE_PRICE={{ stripe.price_id }}` becomes `# STRIPE_PRICE=price_abc123`. This lets you show what a value *would* be without activating it.

Templates are ideal when your `.env` files have structure — section comments, commented-out optional values, hardcoded defaults — that you want to preserve.

#### Manifests

Manifests are a simpler YAML format that maps env var names to store item IDs:

```yaml
# web/env.manifest.yaml
target: ".env.local"
vars:
  NEXT_PUBLIC_SUPABASE_URL: supabase.url
  NEXT_PUBLIC_SUPABASE_ANON_KEY: supabase.anon_key
  NEXT_PUBLIC_STRIPE_PUBLISHABLE_KEY: stripe.publishable_key
```

Manifests are good for components where you just need a clean list of key-value pairs with no extra structure.

#### Discovery order

For each component, assembly looks for (first match wins):
1. `env.manifest.yaml`
2. `env.template`
3. `.env.template`

#### Topologies

Topologies define which environment each component resolves against:

```yaml
# topologies.yaml
all-local:
  api: dev
  web: dev
  worker: dev

all-prod:
  api: prod
  web: prod
  worker: prod

# Mix and match: local code, prod database
hybrid:
  api: dev
  web: dev
  worker: dev
  overrides:
    api:
      supabase.*: prod
    worker:
      supabase.*: prod
```

Components can specify an explicit path when the directory name doesn't match:

```yaml
custom-layout:
  api:
    env: dev
    path: services/backend
  web: dev
```

Then assemble:

```bash
urd assemble --topology all-local          # all components
urd assemble --topology hybrid             # mix-and-match
urd assemble --topology hybrid -c api      # just one component
urd assemble --topology all-local --allow-missing  # skip missing values
urd assemble --topology all-local --dry-run        # preview without writing
```

By default, assembly errors if a referenced store item or environment value is missing. Use `--allow-missing` to continue with empty values instead (warnings are printed to stderr).

Use `--dry-run` to preview the resolved output without writing any files. Each component's output is printed to stdout with a `--- <path> ---` separator:

```
$ urd assemble --topology all-local --dry-run
--- api/.env ---
NODE_ENV=dev
PORT=3002
DATABASE_URL=postgresql://localhost:54322/postgres
SUPABASE_URL=http://localhost:54321

--- web/.env.local ---
NEXT_PUBLIC_SUPABASE_URL=http://localhost:54321
NEXT_PUBLIC_SUPABASE_ANON_KEY=eyJhbG...
```

### Encryption

Encryption uses AES-256-GCM via the [RustCrypto](https://github.com/RustCrypto) `aes-gcm` crate. No custom cryptography.

```bash
urd keys init      # generate a 256-bit key
urd keys status    # check key configuration
urd keys export    # print the key (for sharing out-of-band)
```

`keys init` creates two things:

- **Key file** at `~/.config/urd/keys/<id>.key` — the actual secret, never committed
- **Key ID** at `.urd/key-id` — a reference committed to the repo

To share access, export the key and send it securely to your teammate. They place it at `~/.config/urd/keys/<id>.key`.

### Status

Get a quick summary of store health:

```bash
urd status
```

```
Encryption: key a1b2c3d4 (ready)
Items:      29
Values:     54
Envs:       dev, local, prod
Issues:     none
```

Reports encryption key status, item and value counts, all environments in use, and a count of issues (missing values, unencrypted sensitive items, undocumented items). When issues are found, run `urd validate` for details.

### Validation

Check your store for completeness:

```bash
urd validate
```

Reports items with missing environment values, sensitive items stored unencrypted, and items with values but no description.

### TUI

Run `urd` with no arguments to launch the interactive terminal UI.

| Key | Action |
|-----|--------|
| `j`/`k` | Navigate up/down |
| `l`/`Enter` | Expand item |
| `h` | Collapse item |
| `+`/`-` | Expand/collapse all |
| `e` | Edit (metadata on headers, value on env rows) |
| `v` | Edit value (env rows and missing env rows) |
| `a` | Add new item, or new env to existing item |
| `c` | Clone value to another environment |
| `d` | Delete item or env value |
| `r` | Reveal/hide values for selected item |
| `R` | Reveal/hide all values |
| `/` | Search/filter |
| `u` | Undo |
| `Ctrl+r` | Redo |
| `q`/`Esc` | Quit |

Items with declared environments that have no stored values show red "(missing)" rows. Use `e` or `v` on these rows to set a value directly.

## Demos

### Assembly demo

The `demo/` directory contains a working example simulating a monorepo with three components (api uses a template, web and worker use manifests), a pre-populated store with Supabase and Stripe config, and topology presets. The demo encryption key is included since all values are fake.

To try it:

```bash
cd demo

# Copy the demo key into place
mkdir -p ~/.config/urd/keys
cp .urd/demo.key ~/.config/urd/keys/cc916f35.key

# Assemble .env files
urd assemble --topology all-local

# Browse the store
urd list
urd list --tag vendor:stripe
urd get supabase.url

# Launch the TUI
urd
```

### Import demo

The `demo-import/` directory shows how to bootstrap a store from existing config files. It contains four source files and the resulting store after importing them in sequence. Uses the same encryption key as the assembly demo.

```bash
cd demo-import

# Copy the demo key into place (same key as demo/)
mkdir -p ~/.config/urd/keys
cp .urd/demo.key ~/.config/urd/keys/cc916f35.key

# The store was built by running these four imports in order:
urd import basics.env --env dev                          # plain app config
urd import database.yaml --env dev                       # YAML format, auto-detected
urd import vendor-keys.env --env dev --secret            # encrypt all values
urd import production.env --env prod --skip-existing     # layer prod without clobbering

# Inspect the result
urd list
urd get STRIPE_SECRET_KEY --env dev --reveal
```

## CLI reference

```
urd                                          Launch TUI
urd set                                      Interactive provisioning
urd set <id> -e <env> <value>                Set a value
urd set <id> -e <env> --secret <value>       Set an encrypted value
urd get <id> [-e <env>] [--reveal]           Get a value
urd list [-e <env>] [-t <tag>] [--reveal]    List items
urd remove <id>                              Remove an item
urd import <path> -e <env> [--secret] [--skip-existing]  Bulk import from file or stdin
urd catalog add <id> [-d ...] [-s ...] ...   Add/update catalog metadata
urd catalog list [-e ...] [-t ...] [-s ...]  List catalog entries
urd catalog show <id>                        Show full item details
urd catalog remove <id>                      Remove an item
urd status                                   Show store health summary
urd validate                                 Check store completeness
urd assemble -t <topo> [-c <comp>] [--allow-missing] [--dry-run]  Generate .env files
urd keys init                                Generate encryption key
urd keys status                              Show key status
urd keys export                              Print key for sharing
```

## License

MIT
