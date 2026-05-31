# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

**Anvil** — a Rust CLI tool named after Minecraft's repair-and-upgrade block. Scans a local mods folder, identifies each JAR by its SHA1 hash against the Modrinth API, checks for newer versions matching the same mod loader and game version, and downloads updates (with optional backup).

## Commands

```bash
# Build
cargo build                  # Debug
cargo build --release        # Optimised (≈7 MB binary)

# Check without building
cargo check

# Run tests
cargo test

# Lint
cargo clippy

# Run the tool
cargo run -- --help
cargo run -- --dry-run
cargo run -- --mods-dir "C:\path\to\mods"
cargo run -- --game-version 1.21.10 --dry-run
cargo run -- list
cargo run -- rollback
```

## Architecture

**15 modules** under `src/`, compiled as a library (`src/lib.rs`) + thin binary (`src/main.rs`):

| Module | Purpose |
|--------|-------|
| `lib.rs` | Crate root: all `pub mod` declarations, re-exports, `run_list()` |
| `main.rs` | Thin binary entry point: CLI parse, tracing, dispatch |
| `cli.rs` | clap derive CLI — 3 subcommands, 15+ flags |
| `config.rs` | TOML config loading + CLI override merge |
| `paths.rs` | Cross-platform mods/config/cache dirs (uses `dirs` crate) |
| `types.rs` | All shared structs, enums, traits (`ApiClient`, `ProgressRenderer`) |
| `error.rs` | `thiserror` Error enum + `Result` alias |
| `api.rs` | `ModrinthApi`: rate-limited (~4 req/s), retry, streaming downloads |
| `cache.rs` | File-based JSON cache keyed by SHA1, composite update key |
| `scanner.rs` | JAR discovery, SHA1 hashing, batch identification |
| `filters.rs` | Include/exclude slug/name filtering, loader + game-version filters |
| `updater.rs` | 12-phase update pipeline (the core orchestrator) |
| `backup.rs` | Timestamped backup dirs, atomic moves, rollback |
| `locking.rs` | Lockfile (`mod-updater.lock`) read/write, state diffing |
| `output.rs` | indicatif progress bars, terminal-width tables, changelogs |

**Lib+bin structure:** The crate follows the standard Rust pattern — `src/lib.rs` declares all modules as `pub` and exports `run_list()`, while `src/main.rs` is a thin wrapper that calls into the library via `use anvil::*`. This enables integration tests in `tests/`.

**Traits decouple modules:**
- `ApiClient` (async-trait) — implemented by `ModrinthApi`, mockable for tests
- `ProgressRenderer` — implemented by `ConsoleProgress`, no-op for tests

**Key dependencies:** clap 4.5, reqwest 0.12 (rustls), tokio 1, serde, indicatif 0.17, tracing 0.1

## Test organization

Tests follow a clean-split pattern: private-item tests use `#[path]` to stay in the crate, public-API tests live in `tests/` as integration tests.

### Unit tests (`src/tests/`) — for private items

When a test accesses a **private function, method, constant, or field**, it lives in `src/tests/` and is wired via the `#[path]` attribute:

```rust
// In src/cache.rs (or api.rs, output.rs)
#[cfg(test)]
#[path = "tests/test_cache.rs"]
mod tests;
```

Only three modules use this pattern:
- `src/tests/test_cache.rs` — tests private `ApiCache` methods (`update_key()`, `version_path()`, etc.)
- `src/tests/test_api.rs` — tests private `ModrinthApi::rate_limit()`, module constants (`BASE_URL`, `USER_AGENT`, `RATE_LIMIT_INTERVAL`, `MAX_RETRIES`)
- `src/tests/test_output.rs` — tests private `wrap_lines()`

### Integration tests (`tests/`) — for public API

All other modules test only public items. Their tests live in `tests/` as proper Rust integration test binaries, one per module:

| Test file | Module under test |
|-----------|-------------------|
| `tests/updater.rs` | `anvil::updater` |
| `tests/locking.rs` | `anvil::locking` |
| `tests/types.rs` | `anvil::types` |
| `tests/config.rs` | `anvil::config` |
| `tests/backup.rs` | `anvil::backup` |
| `tests/error.rs` | `anvil::error` |
| `tests/scanner.rs` | `anvil::scanner` |
| `tests/cli.rs` | `anvil::cli` |
| `tests/filters.rs` | `anvil::filters` |
| `tests/paths.rs` | `anvil::paths` |

Each integration test file imports the module under test via `use anvil::module_name::...` and uses shared utilities from `tests/common/mod.rs`.

### Shared test utilities

**`tests/common/mod.rs`** — Integration test mocks and helpers (not compiled as a standalone test). Contains:
- `tests/common/mocks.rs` — `MockApi` (implements `ApiClient`), `MockProgress` (implements `ProgressRenderer`)
- `tests/common/helpers.rs` — Factory functions (`make_test_version()`, `make_project()`, `make_file()`, `make_config()`), temp-dir helpers (`unique_temp_dir()`, `setup_temp_mods_dir()`), hash helper (`sha1_hex()`)

**`src/test_utils.rs`** — Crate-internal helpers for `#[path]` unit tests (`#[cfg(test)]` gated). Contains `make_test_version()` and `unique_temp_dir()`.

### Adding new tests

1. If the test needs a **private item** (a function, method, constant, or field not marked `pub`): add to the appropriate `src/tests/test_*.rs` file.
2. If the test uses only **public items**: add a `#[test]` or `#[tokio::test]` function in the corresponding `tests/*.rs` file.
3. Mocks and helpers go in `tests/common/` (integration) or `src/test_utils.rs` (unit) — never duplicate a mock.

### Verification

```bash
cargo test           # All tests pass
cargo clippy         # Clean
cargo build --release # Binary compiles
```

## Modrinth API notes

- API base: `https://api.modrinth.com/v2`
- `GET /version_file/{hash}?algorithm=sha1` — identify mod by SHA1
- `POST /version_file/{hash}/update` — get latest version filtered by loader + game version
- `GET /project/{id}` — get project slug, title, status
- Rate limit: 300 req/min; tool enforces 250 ms between requests
- Requires `User-Agent` header
