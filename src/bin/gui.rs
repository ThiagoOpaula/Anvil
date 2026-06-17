//! Anvil GUI — Minecraft Mod Updater (eframe desktop GUI).
//!
//! Launches an egui/eframe window with tabs for scanning mods, checking for
//! updates, downloading updates, managing settings, and rolling back backups.
//!
//! Requires the `gui` feature:
//! ```bash
//! cargo run --features gui --bin anvil-gui
//! ```
//!
//! The CLI (`src/main.rs`) is unaffected and continues to work without the
//! `gui` feature.

use anvil::config::{self, ResolvedConfig};
use anvil::paths;

fn main() -> eframe::Result<()> {
    // ── Set up tracing to a file (no console in GUI mode) ────────────────
    let log_dir = paths::config_dir();
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("anvil-gui.log");

    let file = std::fs::File::create(&log_path).unwrap_or_else(|e| {
        eprintln!("warning: cannot create log file: {}", e);
        // Fallback: write to stderr just in case it's visible.
        std::fs::File::create("anvil-gui.log").unwrap()
    });

    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(file))
        .with_target(false)
        .with_level(true)
        .with_ansi(false)
        .init();

    tracing::info!("Anvil GUI starting");

    // ── Load config ──────────────────────────────────────────────────────
    let config_path = paths::config_dir().join("config.toml");
    let resolved = match config::load(&config_path) {
        Ok(file_cfg) => {
            // Build ResolvedConfig from file + defaults.
            ResolvedConfig {
                mods_dir: file_cfg
                    .mods_dir
                    .unwrap_or_else(paths::default_mods_dir),
                backup: file_cfg.backup.unwrap_or(true),
                loader: file_cfg.loader,
                game_version: file_cfg.game_version,
                include: file_cfg.include.unwrap_or_default(),
                exclude: file_cfg.exclude.unwrap_or_default(),
                max_updates: file_cfg.max_updates,
                log_level: anvil::config::LogLevel::Info,
                dry_run: file_cfg.dry_run.unwrap_or(false),
                confirm: file_cfg.confirm.unwrap_or(true),
                changelog: file_cfg.changelog.unwrap_or(false),
            }
        }
        Err(e) => {
            tracing::warn!("config load failed: {} — using defaults", e);
            ResolvedConfig {
                mods_dir: paths::default_mods_dir(),
                backup: true,
                loader: None,
                game_version: None,
                include: vec![],
                exclude: vec![],
                max_updates: None,
                log_level: anvil::config::LogLevel::Info,
                dry_run: false,
                confirm: true,
                changelog: false,
            }
        }
    };

    // ── Launch GUI ──────────────────────────────────────────────────────
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    let app = anvil::gui::AnvilApp::new(resolved);
    eframe::run_native(
        "Anvil — Minecraft Mod Updater",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
