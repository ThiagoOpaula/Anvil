//! Configuration loading and resolution.
//!
//! Merges values from an optional `config.toml` file with CLI flags.
//! CLI flags always take precedence over config file values, which in
//! turn take precedence over hard-coded defaults.

use std::path::{Path, PathBuf};

use crate::cli::{Cli, Command, CommonArgs};
use crate::error::{Error, Result};
use crate::paths;
use crate::types::{Config, Profile};

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
    /// Named mods folders from the config file.
    pub profiles: Vec<Profile>,
    /// Profile to use when `--profile` is not passed.
    pub active_profile: Option<String>,
    /// Path to the config file this instance was resolved from.
    pub config_path: PathBuf,
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
        Some(Command::Profiles(_)) => (None, false, None, false, false),
        Some(Command::Rollback) | None => (None, false, None, false, false),
    };

    let mut resolved = resolve_from_file(
        file,
        cli.mods_dir.clone(),
        common,
        cli.profile.as_deref(),
        &path,
    )?;

    // Update-only flags have no config-file counterpart: a flag is either
    // passed (and wins) or absent (and the file value stands).
    if cli_no_backup {
        resolved.backup = false;
    }
    if cli_max_updates.is_some() {
        resolved.max_updates = cli_max_updates;
    }
    if cli_yes {
        resolved.confirm = false;
    }
    if cli_changelog {
        resolved.changelog = true;
    }

    Ok(resolved)
}

/// Merge a loaded `Config` file with CLI inputs, applying profile selection
/// and the full precedence ruleset.
///
/// Both binaries call this, so the ruleset lives exactly once: `bin/gui.rs`
/// passes `(file, None, None, None, &config_path)` and therefore honours
/// `active_profile` for free.
///
/// # Precedence
///
/// | field | order |
/// |---|---|
/// | `mods_dir` | `--mods-dir` > `--profile NAME` > `active_profile` > `file.mods_dir` > default |
/// | `loader` / `game_version` | CLI flag > selected profile > `file.*` > auto-detect (`None`) |
///
/// Error behaviour is deliberately asymmetric:
///
/// - `--profile NAME` that matches nothing is a **hard error** — the user asked
///   for a specific folder, and silently operating on a different one would
///   write backups and JARs into the wrong tree;
/// - a stale `active_profile` only **warns** and falls through, so a leftover
///   name in a hand-edited config never bricks every command.
pub fn resolve_from_file(
    file: Config,
    cli_mods_dir: Option<PathBuf>,
    common: Option<&CommonArgs>,
    profile_flag: Option<&str>,
    config_path: &Path,
) -> Result<ResolvedConfig> {
    // ── Profile selection ────────────────────────────────────────────────
    let (profile_mods_dir, profile_loader, profile_game_version) = match profile_flag {
        Some(name) => match find_profile(&file.profiles, name) {
            Some(p) => (
                Some(p.mods_dir.clone()),
                p.loader.clone(),
                p.game_version.clone(),
            ),
            None => {
                return Err(Error::Config(format!(
                    "profile '{}' not found in config file '{}'",
                    name,
                    config_path.display()
                )));
            }
        },
        None => match file.active_profile.as_deref() {
            Some(name) => match find_profile(&file.profiles, name) {
                Some(p) => (
                    Some(p.mods_dir.clone()),
                    p.loader.clone(),
                    p.game_version.clone(),
                ),
                None => {
                    tracing::warn!(
                        profile = name,
                        "active_profile not found in config — falling back to mods_dir"
                    );
                    (None, None, None)
                }
            },
            None => (None, None, None),
        },
    };

    // ── Resolve each field ────────────────────────────────────────────────

    let mods_dir = cli_mods_dir
        .or(profile_mods_dir)
        .or(file.mods_dir)
        .unwrap_or_else(paths::default_mods_dir);

    let backup = file.backup.unwrap_or(true);

    let loader = common
        .and_then(|c| c.loader.clone())
        .or(profile_loader)
        .or(file.loader);

    let game_version = common
        .and_then(|c| c.game_version.clone())
        .or(profile_game_version)
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

    let max_updates = file.max_updates;

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

    let confirm = file.confirm.unwrap_or(true);

    let changelog = file.changelog.unwrap_or(false);

    let dark_mode = file.dark_mode.unwrap_or(true);

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
        profiles: file.profiles,
        active_profile: file.active_profile,
        config_path: config_path.to_path_buf(),
    })
}

/// First profile with `name` wins; duplicate names are not an error.
fn find_profile<'a>(profiles: &'a [Profile], name: &str) -> Option<&'a Profile> {
    profiles.iter().find(|p| p.name == name)
}

// ── GUI settings merge ───────────────────────────────────────────────────

/// The subset of settings the GUI Settings tab owns.
#[derive(Debug, Clone, Default)]
pub struct SettingsForm {
    /// Mods folder typed into the field. `None` while a profile is active.
    pub mods_dir: Option<PathBuf>,
    pub backup: bool,
    pub loader: Option<String>,
    pub game_version: Option<String>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub max_updates: Option<usize>,
    pub confirm: bool,
    pub changelog: bool,
    pub dark_mode: bool,
    pub profiles: Vec<Profile>,
    pub active_profile: Option<String>,
}

/// Apply the GUI's settings onto an existing `Config`, preserving every key the
/// GUI does not own.
///
/// `save_settings` must never construct a `Config` from scratch: the file is
/// the only store, so a from-scratch literal silently deletes `verbose`,
/// `quiet`, `dry_run`, and any key added later that the GUI has not learned.
/// TOML comments and unmodelled keys are still lost (that needs `toml_edit`).
///
/// While a profile is active the global `mods_dir` is deliberately left
/// untouched: writing the profile's path into it would freeze that path into
/// the global key and let it survive a later switch to `(none)`. `loader` and
/// `game_version` carry no such guard — the Settings tab shows one combo box
/// for both scopes, so they are always written.
pub fn merge_settings(mut existing: Config, form: &SettingsForm) -> Config {
    if form.active_profile.is_none() {
        existing.mods_dir = form.mods_dir.clone();
    }
    existing.backup = Some(form.backup);
    existing.loader = form.loader.clone();
    existing.game_version = form.game_version.clone();
    existing.include = form.include.clone();
    existing.exclude = form.exclude.clone();
    existing.max_updates = form.max_updates;
    existing.confirm = Some(form.confirm);
    existing.changelog = Some(form.changelog);
    existing.dark_mode = Some(form.dark_mode);
    existing.profiles = form.profiles.clone();
    existing.active_profile = form.active_profile.clone();
    existing
}
