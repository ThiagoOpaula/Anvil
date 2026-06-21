<p align="center">
  <h1 align="center">⚒ anvil</h1>
  <p align="center">
    <b>Minecraft Mod Updater</b><br>
    In Minecraft, anvil repairs and upgrades your gear. Anvil does the same for your mods.<br>
    CLI + GUI. One binary. No launcher. No bloat.
  </p>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/release-v0.3.1-blue?style=flat-square" alt="Latest Release">
  <img src="https://img.shields.io/badge/Rust-%23000000?style=flat-square&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Windows-0078D6?style=flat-square&logo=windows&logoColor=white" alt="Windows">
  <img src="https://img.shields.io/badge/Linux-FCC624?style=flat-square&logo=linux&logoColor=black" alt="Linux">
  <img src="https://img.shields.io/badge/macOS-000000?style=flat-square&logo=apple&logoColor=white" alt="macOS">
  <img src="https://img.shields.io/badge/license-PolyForm%20Noncommercial-blue?style=flat-square" alt="License">
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

**New:** Desktop GUI with dark mode, export/import, keyboard shortcuts, and auto-update checks. Or use the interactive CLI — no flags required. Run `anvil-gui.exe` or just `anvil`.

It auto-detects your mods folder. No config required.

<br>

## 🤔 Why Anvil?

Updating mods manually is tedious — search Modrinth, check loader and game-version compatibility, download JARs, drag them into the folder, pray nothing broke.

Anvil automates all of that. Point it at your mods folder and it scans every JAR by its content hash, queries the Modrinth API for updates, and downloads the right version — matching your exact Minecraft version and mod loader. No launcher required. No lock-in. No bloat.

Before applying updates, Anvil creates a timestamped backup and verifies every download by SHA1. If something goes wrong, one command restores the previous state.

<br>

## 🖥️ GUI

For users who prefer a graphical interface, Anvil ships a desktop app (Windows / Linux / macOS):

```
┌─────────────────────────────────────────────────────┐
│  [Scan & Identify] [Updates] [Settings] [Rollback]  │
├─────────────────────────────────────────────────────┤
│  Game version: [1.21.1 ▼]  Loader: [fabric ▼]       │
│  [Check for Updates]  [Download 3 Update(s)]        │
│  ───────────────────────────────────────────────────│
│  ✓  sodium         0.5.11 → 0.6.1                   │
│  ⬆  iris           1.7.5  → 1.8.2                   │
│  ✗  lithium        0.13.0 → —                       │
│  ...                                                │
├─────────────────────────────────────────────────────┤
│  Idle                              Scan complete    │
└─────────────────────────────────────────────────────┘
```

- **Scan & Identify** — browse your mods folder, see every mod's loader and game version, click mod names to open their Modrinth page. Export results as CSV, Markdown, or JSON
- **Import Mod List** — load an exported mod list and download all mods with one click
- **Updates** — pick a target Minecraft version and loader, check for updates, download with one click
- **Settings** — configure backup, changelog, dark mode, include/exclude filters, and save to `config.toml`
- **Rollback** — restore mods from the last backup
- **Keyboard shortcuts** — `Ctrl+R` to scan, `Ctrl+U` to check updates

Download `anvil-gui.exe` from the [Releases](https://github.com/thiagoOpaula/anvil/releases) page, or build from source:

```bash
cargo run --features gui --bin anvil-gui
```

<br>

## Demo

*Animated demo GIF showing the CLI workflow — coming soon.*

[GIF_PLACEHOLDER]

## GUI Screenshots

*Screenshots of the desktop GUI — coming soon.*

[SCREENSHOT_PLACEHOLDER_SCAN]

[SCREENSHOT_PLACEHOLDER_UPDATES]

[SCREENSHOT_PLACEHOLDER_SETTINGS]

[SCREENSHOT_PLACEHOLDER_ROLLBACK]

<br>

## 📦 Install

```bash
# Download binaries (Windows x64)
# anvil.exe (CLI) + anvil-gui.exe (GUI) from the Releases page

# Or install via Cargo
cargo install --git https://github.com/thiagoOpaula/anvil

# Or build from source
git clone https://github.com/thiagoOpaula/anvil
cd anvil
cargo build --release                    # CLI only
cargo build --release --features gui     # Both binaries
```

<br>

## 📖 Usage

```bash
# ── Interactive (no flags) ──────────────────
anvil                               # Pick version/loader from menus
anvil list                          # List mods with interactive filters

# ── Preview ──────────────────────────────────
anvil --dry-run                     # Check without downloading
anvil list                          # Table of all identified mods
```

```text
$ anvil --dry-run

Found 47 mods

 ✓ Sodium         0.5.11  →  0.6.1
 ✓ Iris           1.7.5   →  1.8.2
 ✓ Lithium        Up-to-date
 ✓ Fabric API     0.102.0 →  0.105.1
 ✓ JEI            18.3.0  →  18.4.5
 ✓ Jade           14.0.2  →  14.1.0
 ✓ EMI            1.1.10  →  1.1.13
 ✓ Create         0.5.1   →  0.5.1  (same — latest)
 ✗ mystery-mod    Not on Modrinth

3 updates available
```

```bash
# ── Update ───────────────────────────────────
anvil                               # Update everything
anvil --changelog                   # Show what changed per mod
anvil --max-updates 5               # Limit to 5 updates
anvil -y                            # Skip confirmation prompt

# ── Target ───────────────────────────────────
anvil --game-version 1.21.4         # Lock to a Minecraft version
anvil --loader fabric               # Force a specific loader
anvil --game-version 1.21.4 --loader neoforge

# ── Filter ───────────────────────────────────
anvil --include "sodium*"           # Only update Sodium family
anvil --exclude "iris*"             # Skip shader mods
anvil --include "/^(sodium|lithium)$/"  # Regex — exact slug match
anvil --include "S*" --exclude "*-dev"  # Combine patterns

# ── Rollback ─────────────────────────────────
anvil rollback                      # Restore from last backup

# ── Custom mods folder ───────────────────────
anvil --mods-dir "D:\modpack\mods" --dry-run
```

<br>

## 🛡️ Safety

Anvil is designed with safety as a core principle:

- **📦 Auto backup** — before applying updates, Anvil moves the current JARs into a timestamped backup directory (`backup_DD-MM-YYYY_mc{version}`)
- **🔐 SHA1 verification** — every downloaded file is hash-checked against Modrinth's records. Corrupted or tampered downloads are rejected
- **♻️ Rollback** — `anvil rollback` restores mods from the last backup. If a rollback itself goes wrong, Anvil creates a *safety backup* of the current state before restoring
- **👁️ Dry-run mode** — `--dry-run` shows what would happen without downloading or modifying any files. Preview before you commit
- **📊 Lockfile** — a `lock.json` tracks the state between runs, so you can always see what changed

<br>

## ✨ Features

### Core Features

| Feature | Description |
|---------|-------------|
| 🔍 **SHA1 identification** | Finds mods by content hash — rename-safe, no filename guessing |
| 📦 **Auto backup** | Timestamped snapshots before every update |
| 🔄 **Rollback** | `anvil rollback` restores everything |
| 📝 **Changelogs** | `--changelog` reads what's new before updating |
| 🎯 **Smart filters** | Include/exclude by slug, wildcard, or regex |
| 🔐 **SHA1 verification** | Every download hash-checked — no corruption |
| ⚠️ **Conflict detection** | Warns about incompatible dependency combos |
| 🛡️ **Deprecation warnings** | Alerts on archived/withdrawn Modrinth projects |
| 📤 **Export mod list** | Save scan results as CSV, Markdown, or JSON |
| 📥 **Import mod list** | Load an exported list and download all mods at once |

### Power User Features

| Feature | Description |
|---------|-------------|
| 🎛️ **Config file** | Set defaults in `config.toml` |
| 📊 **Lockfile** | Tracks state across runs — see what changed |
| 💾 **Disk cache** | API results cached — second run is instant |
| 📋 **Mod listing** | `anvil list` — version, loader, game version per mod |

### User Experience

| Feature | Description |
|---------|-------------|
| ⚡ **Parallel API** | 4 concurrent requests, cache-before-network |
| 🖥️ **Desktop GUI** | egui-based 4-tab app — no terminal needed |
| 🎮 **Interactive CLI** | Fuzzy-select menus for version and loader when no flags given |
| 🌍 **Cross-platform** | Windows / Linux / macOS — auto-detects mods folder |
| 📦 **Single binary** | ~7 MB CLI, ~14 MB GUI — no Python, no JRE, no runtime |
| 🎨 **Dark mode** | Light/dark theme toggle in GUI Settings |
| ⌨️ **Keyboard shortcuts** | `Ctrl+R` scan, `Ctrl+U` check updates in GUI |
| 🔔 **App update check** | Anvil checks for new releases on startup — never miss a version |

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
    ┌──────────────────────────────────┐
    │  backup_17-06-2026_mc1.21.1/     │  ← old JARs moved here
    │  └── sodium-0.5.11.jar           │
    └──────────────────────────────────┘
           │
           ▼
    sodium-0.6.1.jar  ← downloaded & SHA1-verified
    lock.json         ← state snapshot (in cache dir)

    Rollback: current files saved to backup_before_rollback_*
    before restoring — nothing is lost.
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

CLI flags always win over config values. The GUI Settings tab writes this file automatically.

<br>

## 🧬 Tech

**Language:** Rust · **Async:** Tokio · **HTTP:** reqwest (rustls) · **CLI:** clap · **TUI prompts:** dialoguer · **Progress:** indicatif · **GUI:** egui/eframe · **Logging:** tracing · **Size:** ~7 MB CLI, ~14 MB GUI static binary

## 🗺️ Roadmap

| Priority | Feature |
|----------|---------|
| 🌳 | Dependency tree — visualize which mods depend on which |
| 📦 | Modpack support — import and update `.mrpack` files |
| 🔄 | Self-updater — Anvil updates itself |
| 🔎 | Mod discovery — browse Modrinth from inside the app |
| 🎨 | Shaders + resource pack updates |

See the [full roadmap](ROADMAP.md) for details.

<br>

## 📜 License

**PolyForm Noncommercial 1.0.0** — free for personal, educational, and non-profit use. Commercial use requires a separate license. See [LICENSE](LICENSE).

---

<p align="center">
  <sub>Built with Rust. Powered by <a href="https://modrinth.com">Modrinth</a>. ⚒</sub>
</p>
