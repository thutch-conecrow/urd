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

### Assembly

Assembly generates `.env` files by resolving component manifests against the store through a topology.

**Component manifests** live alongside each component and declare which store items map to which env vars:

```yaml
# api/env.manifest.yaml
target: ".env"
vars:
  DATABASE_URL: supabase.database_url
  STRIPE_SECRET_KEY: stripe.secret_key
  STRIPE_WEBHOOK_SECRET: stripe.webhook_secret
```

```yaml
# web/env.manifest.yaml
target: ".env.local"
vars:
  NEXT_PUBLIC_SUPABASE_URL: supabase.url
  NEXT_PUBLIC_SUPABASE_ANON_KEY: supabase.anon_key
  NEXT_PUBLIC_STRIPE_PUBLISHABLE_KEY: stripe.publishable_key
```

**Topologies** define which environment each component resolves against:

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
```

Each component gets its `.env` file written with decrypted values, ready to use.

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

### Validation

Check your store for completeness:

```bash
urd validate
```

Reports items missing descriptions, sensitivity levels, or expected environment values.

### TUI

Run `urd` with no arguments to launch the interactive terminal UI. Browse items, search, edit values and metadata, add new items, clone existing ones, and undo/redo changes — all without leaving the terminal.

## Demo

The `demo/` directory contains a working example simulating a monorepo with three components (api, web, worker), a pre-populated store with Supabase and Stripe config, and topology presets. The demo encryption key is included since all values are fake.

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

## CLI reference

```
urd                                          Launch TUI
urd set                                      Interactive provisioning
urd set <id> -e <env> <value>                Set a value
urd set <id> -e <env> --secret <value>       Set an encrypted value
urd get <id> [-e <env>] [--reveal]           Get a value
urd list [-e <env>] [-t <tag>] [--reveal]    List items
urd remove <id>                              Remove an item
urd catalog add <id> [-d ...] [-s ...] ...   Add/update catalog metadata
urd catalog list [-e ...] [-t ...] [-s ...]  List catalog entries
urd catalog show <id>                        Show full item details
urd catalog remove <id>                      Remove an item
urd validate                                 Check store completeness
urd assemble -t <topology> [-c <component>]  Generate .env files
urd keys init                                Generate encryption key
urd keys status                              Show key status
urd keys export                              Print key for sharing
```

## License

MIT
