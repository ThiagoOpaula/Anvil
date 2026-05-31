<p align="center">
  <h1 align="center">⚒ anvil</h1>
  <p align="center">
    <b>Minecraft Mod Updater</b><br>
    Named after the block that repairs and upgrades.<br>
    One binary. No launcher. No bloat.
  </p>
</p>

<p align="center">
  <a href="#-install"><b>Install</b></a> ·
  <a href="#-usage"><b>Usage</b></a> ·
  <a href="#-features"><b>Features</b></a> ·
  <a href="#-how-it-works"><b>How It Works</b></a>
</p>

<br>

## 🚀 Quick Start

```bash
# 1. Grab the binary from Releases → anvil.exe

# 2. Preview what's outdated (safe — touches nothing)
anvil --dry-run

# 3. Apply all updates
anvil

# 4. Need a specific Minecraft version?
anvil --game-version 1.21.4
```

It auto-detects your mods folder. No config required.

<br>

## 📦 Install

```bash
# Download binary (Windows x64)
# anvil.exe from the Releases page

# Or install via Cargo
cargo install --git https://github.com/thiagoOpaula/anvil

# Or build from source
git clone https://github.com/thiagoOpaula/anvil
cd anvil
cargo build --release
```

<br>

## 🖥️ Usage

```bash
# ── Preview ──────────────────────────────
anvil --dry-run                     # Check without downloading
anvil list                          # Table of all identified mods

# ── Update ───────────────────────────────
anvil                               # Update everything
anvil --changelog                   # Show what changed per mod
anvil --max-updates 5               # Limit to 5 updates
anvil -y                            # Skip confirmation prompt

# ── Target ───────────────────────────────
anvil --game-version 1.21.4         # Lock to a Minecraft version
anvil --loader fabric               # Force a specific loader
anvil --game-version 1.21.4 --loader neoforge

# ── Filter ───────────────────────────────
anvil --include "sodium*"           # Only update Sodium family
anvil --exclude "iris*"             # Skip shader mods
anvil --include "/^(sodium|lithium)$/"  # Regex — exact slug match
anvil --include "S*" --exclude "*-dev"  # Combine patterns

# ── Rollback ─────────────────────────────
anvil rollback                      # Restore from last backup

# ── Custom mods folder ───────────────────
anvil --mods-dir "D:\modpack\mods" --dry-run
```

<br>

## ✨ Features

| Feature | Description |
|---------|-------------|
| 🔍 **SHA1 identification** | Finds mods by content hash — rename-safe |
| ⚡ **Parallel API** | 4 concurrent requests, cache-before-network |
| 📦 **Auto backup** | Timestamped snapshots before every update |
| 🔄 **Rollback** | `anvil rollback` restores everything |
| 🎯 **Smart filters** | Include/exclude by slug, wildcard, or regex |
| 🔐 **SHA1 verification** | Every download hash-checked — no corruption |
| 💾 **Disk cache** | API results cached — second run is instant |
| 📋 **Mod listing** | `anvil list` — version, loader, game version per mod |
| 📝 **Changelogs** | `--changelog` reads what's new before updating |
| 📊 **Lockfile** | Tracks state across runs — see what changed |
| 🌍 **Cross-platform** | Windows / Linux / macOS — auto-detects mods folder |
| 🎛️ **Config file** | Set defaults in `config.toml` |
| 🛡️ **Deprecation warnings** | Alerts on archived/withdrawn Modrinth projects |
| ⚠️ **Conflict detection** | Warns about incompatible dependency combos |
| 📦 **Single binary** | ~7 MB — no Python, no JRE, no runtime |

<br>

## 🔧 How It Works

```
your mods folder
    │
    ├── sodium-0.5.11.jar  ──SHA1──▶  Modrinth API  ──▶  "v0.6.1 available!"
    ├── iris-1.7.5.jar     ──SHA1──▶  Modrinth API  ──▶  "Already latest ✓"
    ├── mystery.jar         ──SHA1──▶  Modrinth API  ──▶  "Not on Modrinth — skipped"
    └── ...
           │
           ▼
    ┌─────────────────────────┐
    │  backup_20250531_120000/ │  ← old JARs moved here
    │  └── sodium-0.5.11.jar  │
    └─────────────────────────┘
           │
           ▼
    sodium-0.6.1.jar  ← downloaded & SHA1-verified
    anvil.lock        ← state snapshot written
```

- **Hash-based** — no filename parsing, no guessing. SHA1 the file, ask Modrinth.
- **Parallel** — 4 concurrent HTTP connections, streaming downloads with progress bars.
- **Safe** — backups first, SHA1 verification, `--dry-run` previews everything.

<br>

## 🎛️ Config file

Place `config.toml` at:

| Platform | Path |
|----------|------|
| Windows | `%APPDATA%/anvil/config.toml` |
| Linux | `~/.config/anvil/config.toml` |
| macOS | `~/Library/Application Support/anvil/config.toml` |

```toml
game_version = "1.21.4"
loader = "fabric"
include = ["sodium*", "lithium*"]
exclude = ["*-dev"]
changelog = true
```

CLI flags always win over config values.

<br>

## 🧬 Tech

**Language:** Rust · **Async:** Tokio · **HTTP:** reqwest (rustls) · **CLI:** clap · **Progress:** indicatif · **Logging:** tracing · **Size:** ~7 MB static binary

## 📜 License

MIT

---

<p align="center">
  <sub>Built with Rust. Powered by <a href="https://modrinth.com">Modrinth</a>. ⚒</sub>
</p>
