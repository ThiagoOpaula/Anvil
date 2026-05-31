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

**14 modules** under `src/`, compiled to a single static binary:

| Module | Lines | Purpose |
|--------|-------|---------|
| `main.rs` | 262 | Entry point: tracing init, config load, command dispatch |
| `cli.rs` | 111 | clap derive CLI — 3 subcommands, 15+ flags |
| `config.rs` | 186 | TOML config loading + CLI override merge |
| `paths.rs` | 82 | Cross-platform mods/config/cache dirs (uses `dirs` crate) |
| `types.rs` | 304 | All shared structs, enums, traits (`ApiClient`, `ProgressRenderer`) |
| `error.rs` | 43 | `thiserror` Error enum + `Result` alias |
| `api.rs` | 207 | `ModrinthApi`: rate-limited (~4 req/s), retry, streaming downloads |
| `cache.rs` | 179 | File-based JSON cache keyed by SHA1, composite update key |
| `scanner.rs` | 183 | JAR discovery, SHA1 hashing, batch identification |
| `filters.rs` | 289 | Include/exclude slug/name filtering, loader + game-version filters |
| `updater.rs` | 656 | 12-phase update pipeline (the core orchestrator) |
| `backup.rs` | 153 | Timestamped backup dirs, atomic moves, rollback |
| `locking.rs` | 332 | Lockfile (`mod-updater.lock`) read/write, state diffing |
| `output.rs` | 395 | indicatif progress bars, terminal-width tables, changelogs |

**Traits decouple modules:**
- `ApiClient` (async-trait) — implemented by `ModrinthApi`, mockable for tests
- `ProgressRenderer` — implemented by `ConsoleProgress`, no-op for tests

**Key dependencies:** clap 4.5, reqwest 0.12 (rustls), tokio 1, serde, indicatif 0.17, tracing 0.1

## Modrinth API notes

- API base: `https://api.modrinth.com/v2`
- `GET /version_file/{hash}?algorithm=sha1` — identify mod by SHA1
- `POST /version_file/{hash}/update` — get latest version filtered by loader + game version
- `GET /project/{id}` — get project slug, title, status
- Rate limit: 300 req/min; tool enforces 250 ms between requests
- Requires `User-Agent` header
