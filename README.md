<p align="center">
  <h1 align="center">⚒ anvil</h1>
  <p align="center">
    <b>Minecraft Mod Updater</b><br>
    Named after the block that repairs and upgrades.<br>
    CLI + GUI. One binary. No launcher. No bloat.
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

**New:** interactive prompts — just run `anvil` with no flags and pick your version/loader from a menu. Or use the GUI: `anvil-gui.exe` (4-tab desktop app).

It auto-detects your mods folder. No config required.

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

- **Scan & Identify** — browse your mods folder, see every mod's loader and game version
- **Updates** — pick a target Minecraft version and loader, check for updates, download with one click
- **Settings** — configure backup, changelog, include/exclude filters, and save to `config.toml`
- **Rollback** — restore mods from the last backup

Download `anvil-gui.exe` from the [Releases](https://github.com/thiagoOpaula/anvil/releases) page, or build from source:

```bash
cargo run --features gui --bin anvil-gui
```

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
| 🖥️ **Desktop GUI** | egui-based 4-tab app — no terminal needed |
| 🎮 **Interactive CLI** | Fuzzy-select menus for version and loader when no flags given |
| 📦 **Single binary** | ~7 MB CLI, ~14 MB GUI — no Python, no JRE, no runtime |

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

### Quick wins
- 🎨 **Dark mode toggle** — switch between light and dark themes
- 🔗 **Modrinth links** — click a mod to open its project page
- 📤 **Export mod list** — save scan results as CSV / Markdown / JSON
- 🔔 **App update check** — notify when a new version of Anvil is available
- ⌨️ **Keyboard shortcuts** — `Ctrl+R` scan, `Ctrl+U` check updates
- 🪟 **Remember window size** — persist window geometry across sessions

### Medium
- 🌳 **Dependency tree** — visualize which mods depend on which
- 📦 **Modpack support** — import and update `.mrpack` files
- 👥 **Profile system** — switch between multiple mods folders
- ⏰ **Scheduled checks** — opt-in daily update notifications
- 🫱🏽‍🫲🏾 **Share mod list** — export a list of your mods to share with friends
- 🖱️ **Drag & drop JARs** — drop a file to identify it instantly

### Big features
- 🔄 **Self-updater** — Anvil updates itself
- 🔎 **Mod discovery** — browse Modrinth from inside the app, install in one click
- 🎨 **Shaders + resource packs** — scan and update beyond just mods
- 👁️ **CLI daemon mode** — `anvil watch` auto-identifies new JARs
- 🐧 **Linux/macOS packages** — `.deb`, `.rpm`, Homebrew, AUR

### Non-code
- 🌐 **Landing page** — simple website with downloads and screenshots
- 🎬 **YouTube demo** — 2-minute walkthrough of the GUI
- 📋 **Modrinth project** — list Anvil as a tool on Modrinth

<br>

## 📜 License

**PolyForm Noncommercial 1.0.0** — free for personal, educational, and non-profit use. Commercial use requires a separate license. See [LICENSE](LICENSE).

---

<p align="center">
  <sub>Built with Rust. Powered by <a href="https://modrinth.com">Modrinth</a>. ⚒</sub>
  <br>
  <sub>Made using <a href="https://claude.ai/code">Claude Code</a> + DeepSeek V4 Pro</sub>
</p>
