//! Configuration loading and resolution.
//!
//! Merges values from an optional `config.toml` file with CLI flags.
//! CLI flags always take precedence over config file values, which in
//! turn take precedence over hard-coded defaults.

use std::path::{Path, PathBuf};

use crate::cli::{Cli, Command};
use crate::error::{Error, Result};
use crate::paths;
use crate::types::Config;

// ── Log level ────────────────────────────────────────────────────────────

/// Controls how much output the tool produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Only fatal / actionable errors.
    Error,
    /// Errors plus warnings.
    Warn,
    /// Normal output — progress, summaries, prompts (the default).
    Info,
    /// Detailed trace for debugging.
    Debug,
}

// ── Resolved config ──────────────────────────────────────────────────────

/// Fully resolved runtime configuration with all defaults applied.
///
/// Every field is a concrete value (no `Option` where a default exists).
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// Absolute path to the mods folder being operated on.
    pub mods_dir: PathBuf,
    /// Whether to create a backup before replacing JARs.
    pub backup: bool,
    /// Forced mod loader (fabric, forge, quilt, neoforge). `None` = auto-detect.
    pub loader: Option<String>,
    /// Target Minecraft version. `None` = auto-detect from each mod.
    pub game_version: Option<String>,
    /// Slug/name patterns that a mod MUST match to be processed.
    pub include: Vec<String>,
    /// Slug/name patterns that a mod must NOT match to be processed.
    pub exclude: Vec<String>,
    /// Cap on how many mods to update in one run. `None` = unlimited.
    pub max_updates: Option<usize>,
    /// Current log verbosity level.
    pub log_level: LogLevel,
    /// `true` = report only, do not write anything to disk.
    pub dry_run: bool,
    /// `true` = ask the user before downloading updates.
    pub confirm: bool,
    /// `true` = print the changelog for each updated mod.
    pub changelog: bool,
    /// Whether to use the dark theme in the GUI.
    pub dark_mode: bool,
}

// ── Loading ──────────────────────────────────────────────────────────────

/// Deserialise a `Config` from a TOML file at `path`.
///
/// Returns `Config::default()` (all `None`) when the file does not exist,
/// so callers can treat a missing config as "no overrides".
pub fn load(path: &Path) -> Result<Config> {
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(|e| {
            Error::Config(format!("failed to read config file '{}': {}", path.display(), e))
        })?;
        let config: Config = toml::from_str(&content).map_err(|e| {
            Error::Config(format!(
                "failed to parse config file '{}': {}",
                path.display(),
                e
            ))
        })?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}

// ── Resolution ───────────────────────────────────────────────────────────

/// Load the config file (if available) and merge CLI flags on top,
/// producing a fully resolved `ResolvedConfig` with all defaults applied.
///
/// When `config_path` is `None` the default path
/// (`paths::config_dir() / "config.toml"`) is used.
pub fn resolve(cli: &Cli, config_path: Option<&Path>) -> Result<ResolvedConfig> {
    let path = config_path
        .map(PathBuf::from)
        .unwrap_or_else(|| paths::config_dir().join("config.toml"));

    let file = load(&path)?;

    // ── Split the parsed CLI into common flags + update-only flags ────────
    let (common, cli_no_backup, cli_max_updates, cli_yes, cli_changelog) = match &cli.command {
        Some(Command::Update(args)) => (
            Some(&args.common),
            args.no_backup,
            args.max_updates,
            args.yes,
            args.changelog,
        ),
        Some(Command::List(args)) => (Some(&args.common), false, None, false, false),
        Some(Command::Rollback) | None => (None, false, None, false, false),
    };

    // ── Resolve each field ────────────────────────────────────────────────

    let mods_dir = cli
        .mods_dir
        .clone()
        .or(file.mods_dir)
        .unwrap_or_else(paths::default_mods_dir);

    let backup = if cli_no_backup {
        false
    } else {
        file.backup.unwrap_or(true)
    };

    let loader = common
        .and_then(|c| c.loader.clone())
        .or(file.loader);

    let game_version = common
        .and_then(|c| c.game_version.clone())
        .or(file.game_version);

    // CLI include/exclude fully replace config values when non-empty.
    let include = common
        .and_then(|c| if c.include.is_empty() { None } else { Some(c.include.clone()) })
        .or(file.include)
        .unwrap_or_default();

    let exclude = common
        .and_then(|c| if c.exclude.is_empty() { None } else { Some(c.exclude.clone()) })
        .or(file.exclude)
        .unwrap_or_default();

    let max_updates = cli_max_updates.or(file.max_updates);

    let log_level = match common {
        Some(c) if c.quiet => LogLevel::Error,
        Some(c) if c.verbose => LogLevel::Debug,
        _ => {
            if file.quiet == Some(true) {
                LogLevel::Error
            } else if file.verbose == Some(true) {
                LogLevel::Debug
            } else {
                LogLevel::Info
            }
        }
    };

    let dry_run = common
        .map(|c| c.dry_run)
        .unwrap_or(false)
        || file.dry_run.unwrap_or(false);

    let confirm = if cli_yes {
        false
    } else {
        file.confirm.unwrap_or(true)
    };

    let changelog = cli_changelog || file.changelog.unwrap_or(false);

    let dark_mode = file.dark_mode.unwrap_or(false);

    Ok(ResolvedConfig {
        mods_dir,
        backup,
        loader,
        game_version,
        include,
        exclude,
        max_updates,
        log_level,
        dry_run,
        confirm,
        changelog,
        dark_mode,
    })
}
