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

use anvil::config;
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

    // ── Restore saved window geometry (if any) ─────────────────────────────
    #[derive(serde::Deserialize)]
    struct SavedGeometry {
        width: f32,
        height: f32,
        x: Option<f32>,
        y: Option<f32>,
    }

    let geom_path = paths::config_dir().join("window.json");
    let saved_geometry: Option<SavedGeometry> = std::fs::read_to_string(&geom_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    // ── Load config ──────────────────────────────────────────────────────
    // Same resolution path as the CLI, so `active_profile` is honoured here
    // without duplicating the precedence rules.
    let config_path = paths::config_dir().join("config.toml");
    let resolved = config::resolve_from_file(
        config::load(&config_path).unwrap_or_else(|e| {
            tracing::warn!("config load failed: {} — using defaults", e);
            Default::default()
        }),
        None,
        None,
        None,
        &config_path,
    )
    .expect("GUI passes no --profile flag, so resolution cannot fail");

    // ── Launch GUI ──────────────────────────────────────────────────────
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1100.0, 750.0])
        .with_min_inner_size([800.0, 500.0]);

    if let Some(ref geo) = saved_geometry {
        viewport = viewport.with_inner_size([geo.width, geo.height]);
        if let (Some(x), Some(y)) = (geo.x, geo.y) {
            viewport = viewport.with_position([x, y]);
        }
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let app = anvil::gui::AnvilApp::new(resolved);
    eframe::run_native(
        "Anvil — Minecraft Mod Updater",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
