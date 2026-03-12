# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

urd — a CLI config and secrets manager in Rust. Named after the Norse Norn who keeps the Well of Urd (source of truth). Running `urd` with no subcommand launches a ratatui TUI for interactive browsing/editing. CLI subcommands (`set`, `get`, `list`, `catalog add`, etc.) work for scripting.

## Build & Test Commands

```bash
cargo check                          # type-check
cargo clippy                         # lint (all+pedantic+nursery enabled)
cargo test                           # run all tests
cargo test <test_name>               # run a single test
cargo test <test_name> -- --nocapture  # run with stdout visible
cargo run                            # launch TUI (reads .urd/store.yaml)
cargo run -- set mykey -e dev myval  # CLI usage example
```

## Lint Configuration

Edition 2024. Clippy runs with `all`, `pedantic`, and `nursery` at warn level. `unsafe_code` is forbidden. `module_name_repetitions` and `must_use_candidate` are allowed. Fix all clippy warnings before committing.

## Architecture

Single merged store: catalog metadata (description, sensitivity, origin, tags, environments) and per-environment values live together in one YAML file (`.urd/store.yaml`). The `Store` type is `BTreeMap<String, Item>` for alphabetical ordering and git-friendly diffs.

### Module layout

- **`store/`** — `Item` struct, `Store` type alias, YAML load/save, set/get/list/remove commands with interactive mode
- **`catalog/`** — catalog metadata commands (add/remove/list/show) and validate
- **`crypto/`** — AES-256-GCM encryption/decryption via `aes-gcm` crate, key management. Encrypted format: `ENC[aes:sensitive,...]` or `ENC[aes:secret,...]`
- **`tui/`** — ratatui+crossterm TUI. `app.rs` (state, mutations, undo/redo), `ui.rs` (three-panel rendering), `input.rs` (mode-dispatched key handling), `mod.rs` (terminal setup, event loop)
- **`cli.rs`** — clap derive definitions. `command` is `Option<Command>`; `None` launches TUI
- **`paths.rs`** — path resolution. `URD_HOME` env var overrides defaults (used by integration tests)

### Key patterns

- **`#[serde(flatten)]`** on `Item.values`: env values (`dev`, `prod`) are sibling keys to metadata fields in YAML, not nested under a `values:` key. This is critical for the YAML format — removing it silently drops all values.
- **`URD_HOME`** env var: integration tests use `tempfile::TempDir` and set `URD_HOME` to isolate test state. Without `URD_HOME`, store path is `.urd/store.yaml` relative to cwd.
- **TUI event loop**: crossterm 0.28 emits `Press`, `Repeat`, and `Release` key events. The event loop filters to `KeyEventKind::Press` only.
- **TUI modes**: `Browse`, `ConfirmDelete`, `EditValue`, `EditMetadata`, `Add`, `AddEnv`, `Clone`. Mode determines which keys are active.
- **Interactive CLI** (`urd set` with no args): state machine with steps (Id → Env → Sensitivity → Value → Description → Origin → Tags). `Select` prompts support Escape to go back; `Input` prompts use Ctrl+C to abort.

## Code Rules

- **No `unwrap()`** in non-test code. Use `if let`, `let-else`, `?`, or `expect()` with a descriptive message instead.

## Edition 2024 Gotchas

- No `ref` or `ref mut` in patterns that implicitly borrow. Use direct bindings instead.
- `let chains` in `if let` are stable and used throughout.
