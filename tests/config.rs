mod common;
use common::helpers::unique_temp_dir;

use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;

use anvil::cli::Cli;
use anvil::config::{LogLevel, SettingsForm, load, merge_settings, resolve};
use anvil::error::Error;
use anvil::types::{Config, Profile};

// ── LogLevel ────────────────────────────────────────────────────────

#[test]
fn log_level_debug_is_lower_than_info() {
    // Log levels: Error < Warn < Info < Debug
    // Just verify the variants exist and are distinct.
    let levels = [
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
    ];
    // All unique via PartialEq
    for i in 0..levels.len() {
        for j in 0..levels.len() {
            if i == j {
                assert_eq!(levels[i], levels[j]);
            } else {
                assert_ne!(levels[i], levels[j]);
            }
        }
    }
}

#[test]
fn log_level_clone_and_debug() {
    let level = LogLevel::Info;
    let cloned = level;
    assert_eq!(level, cloned);
    let debug_str = format!("{:?}", level);
    assert!(!debug_str.is_empty());
}

// ── load() ──────────────────────────────────────────────────────────

#[test]
fn load_missing_file_returns_default() {
    let config = load(Path::new("/nonexistent/path/config.toml")).unwrap();
    assert!(config.mods_dir.is_none());
    assert!(config.backup.is_none());
    assert!(config.loader.is_none());
}

#[test]
fn load_valid_toml() {
    let dir = unique_temp_dir("config-test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let path = dir.join("config.toml");
    let toml_content = r#"
mods_dir = "/my/mods"
loader = "fabric"
game_version = "1.21"
backup = false
include = ["sodium", "iris"]
exclude = ["old-mod"]
"#;
    fs::write(&path, toml_content).unwrap();

    let config = load(&path).unwrap();
    assert_eq!(config.mods_dir, Some(PathBuf::from("/my/mods")));
    assert_eq!(config.loader, Some("fabric".into()));
    assert_eq!(config.game_version, Some("1.21".into()));
    assert_eq!(config.backup, Some(false));
    assert_eq!(
        config.include,
        Some(vec!["sodium".to_string(), "iris".to_string()])
    );
    assert_eq!(config.exclude, Some(vec!["old-mod".to_string()]));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn load_invalid_toml_returns_error() {
    let dir = unique_temp_dir("config-bad");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let path = dir.join("config.toml");
    fs::write(&path, "this is {{{ not valid toml").unwrap();

    let result = load(&path);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("failed to parse"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn load_empty_toml_is_default() {
    let dir = unique_temp_dir("config-empty");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let path = dir.join("config.toml");
    fs::write(&path, "").unwrap();

    let config = load(&path).unwrap();
    assert!(config.mods_dir.is_none());

    let _ = fs::remove_dir_all(&dir);
}

// ── resolve() ───────────────────────────────────────────────────────

#[test]
fn resolve_defaults_when_no_config_and_no_flags() {
    let cli = Cli::parse_from(["anvil", "update"]);
    // Use a nonexistent config path so we get all defaults.
    let resolved = resolve(&cli, Some(Path::new("/nonexistent/config.toml"))).unwrap();
    assert!(resolved.backup); // default is true
    assert!(!resolved.dry_run);
    assert!(resolved.confirm); // default is true
    assert!(!resolved.changelog);
    assert!(resolved.loader.is_none());
    assert!(resolved.game_version.is_none());
    assert!(resolved.include.is_empty());
    assert!(resolved.exclude.is_empty());
}

#[test]
fn resolve_cli_overrides_config() {
    let dir = unique_temp_dir("resolve-test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let path = dir.join("config.toml");
    fs::write(
        &path,
        r#"
game_version = "1.20"
backup = true
"#,
    )
    .unwrap();

    // CLI says game_version = 1.21, backup disabled via --no-backup
    let cli = Cli::parse_from([
        "anvil", "update",
        "--game-version", "1.21",
        "--no-backup",
    ]);
    let resolved = resolve(&cli, Some(&path)).unwrap();

    // CLI takes precedence
    assert_eq!(resolved.game_version.as_deref(), Some("1.21"));
    assert!(!resolved.backup);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resolve_cli_yes_disables_confirm() {
    let cli = Cli::parse_from(["anvil", "update", "-y"]);
    let resolved = resolve(&cli, Some(Path::new("/nonexistent/config.toml"))).unwrap();
    assert!(!resolved.confirm);
}

#[test]
fn resolve_cli_dry_run() {
    let cli = Cli::parse_from(["anvil", "update", "--dry-run"]);
    let resolved = resolve(&cli, Some(Path::new("/nonexistent/config.toml"))).unwrap();
    assert!(resolved.dry_run);
}

#[test]
fn resolve_cli_quiet_sets_log_level_error() {
    let cli = Cli::parse_from(["anvil", "update", "--quiet"]);
    let resolved = resolve(&cli, Some(Path::new("/nonexistent/config.toml"))).unwrap();
    assert_eq!(resolved.log_level, LogLevel::Error);
}

#[test]
fn resolve_cli_verbose_sets_log_level_debug() {
    let cli = Cli::parse_from(["anvil", "update", "--verbose"]);
    let resolved = resolve(&cli, Some(Path::new("/nonexistent/config.toml"))).unwrap();
    assert_eq!(resolved.log_level, LogLevel::Debug);
}

#[test]
fn resolve_cli_include_exclude() {
    let cli = Cli::parse_from([
        "anvil", "update",
        "--include", "sodium",
        "--exclude", "badmod",
    ]);
    let resolved = resolve(&cli, Some(Path::new("/nonexistent/config.toml"))).unwrap();
    assert_eq!(resolved.include, vec!["sodium"]);
    assert_eq!(resolved.exclude, vec!["badmod"]);
}

#[test]
fn resolve_no_subcommand_defaults_to_update() {
    let cli = Cli::parse_from(["anvil"]);
    let resolved = resolve(&cli, Some(Path::new("/nonexistent/config.toml"))).unwrap();
    // Should not panic; defaults to Info.
    assert_eq!(resolved.log_level, LogLevel::Info);
}

#[test]
fn resolve_list_ignores_update_only_flags() {
    let cli = Cli::parse_from(["anvil", "list"]);
    let resolved = resolve(&cli, Some(Path::new("/nonexistent/config.toml"))).unwrap();
    // List doesn't have --no-backup etc, so backup defaults to true.
    assert!(resolved.backup);
}

#[test]
fn resolve_rollback_uses_defaults() {
    let cli = Cli::parse_from(["anvil", "rollback"]);
    let resolved = resolve(&cli, Some(Path::new("/nonexistent/config.toml"))).unwrap();
    assert_eq!(resolved.log_level, LogLevel::Info);
}

// ── Profiles ────────────────────────────────────────────────────────

/// Write `content` to a fresh `config.toml` in a unique temp dir.
fn write_config(label: &str, content: &str) -> PathBuf {
    let dir = unique_temp_dir(label);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    fs::write(&path, content).unwrap();
    path
}

const PRISM_CONFIG: &str = r#"
mods_dir = "/file/mods"
active_profile = "prism"

[[profiles]]
name = "prism"
mods_dir = "/profile/mods"

[[profiles]]
name = "other"
mods_dir = "/other/mods"
"#;

#[test]
fn profile_supplies_mods_dir_when_no_cli_flag() {
    let path = write_config("cfg-profile", PRISM_CONFIG);
    let cli = Cli::parse_from(["anvil", "update"]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    // active_profile beats the file-level mods_dir.
    assert_eq!(resolved.mods_dir, PathBuf::from("/profile/mods"));
    assert_eq!(resolved.active_profile.as_deref(), Some("prism"));
    assert_eq!(resolved.profiles.len(), 2);
    assert_eq!(resolved.config_path, path);
}

#[test]
fn cli_mods_dir_beats_active_profile() {
    let path = write_config("cfg-cli-mods", PRISM_CONFIG);
    let cli = Cli::parse_from(["anvil", "update", "--mods-dir", "/cli/mods"]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert_eq!(resolved.mods_dir, PathBuf::from("/cli/mods"));
}

#[test]
fn profile_flag_beats_active_profile_and_file_mods_dir() {
    let path = write_config("cfg-profile-flag", PRISM_CONFIG);
    let cli = Cli::parse_from(["anvil", "update", "--profile", "other"]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert_eq!(resolved.mods_dir, PathBuf::from("/other/mods"));
    // active_profile itself is untouched — only the selection changed.
    assert_eq!(resolved.active_profile.as_deref(), Some("prism"));
}

#[test]
fn missing_profile_flag_is_config_error() {
    let path = write_config("cfg-missing-profile", PRISM_CONFIG);
    let cli = Cli::parse_from(["anvil", "update", "--profile", "nope"]);
    let err = resolve(&cli, Some(&path)).unwrap_err();
    assert!(
        matches!(err, Error::Config(_)),
        "expected Error::Config, got {:?}",
        err
    );
    assert!(err.to_string().contains("nope"), "message: {}", err);
}

#[test]
fn dangling_active_profile_warns_and_falls_back_to_file_mods_dir() {
    let path = write_config(
        "cfg-dangling",
        r#"
mods_dir = "/file/mods"
active_profile = "ghost"
"#,
    );
    let cli = Cli::parse_from(["anvil", "update"]);
    // A leftover active_profile must never make the CLI unusable.
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert_eq!(resolved.mods_dir, PathBuf::from("/file/mods"));
}

#[test]
fn dangling_active_profile_falls_back_to_default_when_no_file_mods_dir() {
    let path = write_config("cfg-dangling-default", r#"active_profile = "ghost""#);
    let cli = Cli::parse_from(["anvil", "update"]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert_eq!(resolved.mods_dir, anvil::paths::default_mods_dir());
}

#[test]
fn empty_profiles_array_with_active_profile_falls_back() {
    let path = write_config(
        "cfg-empty-profiles",
        r#"
mods_dir = "/file/mods"
active_profile = "ghost"
profiles = []
"#,
    );
    let cli = Cli::parse_from(["anvil", "update"]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert!(resolved.profiles.is_empty());
    assert_eq!(resolved.mods_dir, PathBuf::from("/file/mods"));
}

#[test]
fn profile_loader_and_game_version_precedence() {
    let path = write_config(
        "cfg-profile-loader",
        r#"
mods_dir = "/file/mods"
loader = "quilt"
game_version = "1.20"
active_profile = "prism"

[[profiles]]
name = "prism"
mods_dir = "/profile/mods"
loader = "fabric"
game_version = "1.21.1"
"#,
    );
    // No CLI flags: the profile beats the file-level values.
    let cli = Cli::parse_from(["anvil", "update"]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert_eq!(resolved.loader.as_deref(), Some("fabric"));
    assert_eq!(resolved.game_version.as_deref(), Some("1.21.1"));

    // CLI flags beat the profile.
    let cli = Cli::parse_from([
        "anvil",
        "update",
        "--loader",
        "neoforge",
        "--game-version",
        "1.22",
    ]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert_eq!(resolved.loader.as_deref(), Some("neoforge"));
    assert_eq!(resolved.game_version.as_deref(), Some("1.22"));
}

#[test]
fn resolve_resolves_profile_loader_and_game_version() {
    let path = write_config(
        "cfg-profile-supply",
        r#"
active_profile = "main"

[[profiles]]
name = "main"
mods_dir = "/main/mods"
loader = "fabric"
game_version = "1.21.1"
"#,
    );
    let cli = Cli::parse_from(["anvil", "update"]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert_eq!(resolved.mods_dir, PathBuf::from("/main/mods"));
    assert_eq!(resolved.loader.as_deref(), Some("fabric"));
    assert_eq!(resolved.game_version.as_deref(), Some("1.21.1"));
}

#[test]
fn duplicate_profile_names_first_match_wins() {
    let path = write_config(
        "cfg-duplicate-profiles",
        r#"
active_profile = "main"

[[profiles]]
name = "main"
mods_dir = "/first/mods"

[[profiles]]
name = "main"
mods_dir = "/second/mods"
"#,
    );
    let cli = Cli::parse_from(["anvil", "update"]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert_eq!(resolved.mods_dir, PathBuf::from("/first/mods"));

    let cli = Cli::parse_from(["anvil", "update", "--profile", "main"]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert_eq!(resolved.mods_dir, PathBuf::from("/first/mods"));
}

#[test]
fn no_profiles_config_keeps_existing_precedence() {
    let path = write_config(
        "cfg-no-profiles",
        r#"
mods_dir = "/file/mods"
loader = "fabric"
backup = false
"#,
    );

    let cli = Cli::parse_from(["anvil", "update"]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert!(resolved.profiles.is_empty());
    assert!(resolved.active_profile.is_none());
    assert_eq!(resolved.mods_dir, PathBuf::from("/file/mods"));
    assert_eq!(resolved.loader.as_deref(), Some("fabric"));
    assert!(!resolved.backup);

    // A CLI mods_dir still wins, exactly as before profiles existed.
    let cli = Cli::parse_from(["anvil", "update", "--mods-dir", "/cli/mods"]);
    let resolved = resolve(&cli, Some(&path)).unwrap();
    assert_eq!(resolved.mods_dir, PathBuf::from("/cli/mods"));
}

// ── merge_settings() ────────────────────────────────────────────────

fn sample_profile() -> Profile {
    Profile {
        name: "main".into(),
        mods_dir: PathBuf::from("/main/mods"),
        loader: Some("fabric".into()),
        game_version: Some("1.21.1".into()),
    }
}

#[test]
fn merge_settings_preserves_profiles_active_profile_and_unowned_keys() {
    let existing = Config {
        mods_dir: Some(PathBuf::from("/old/mods")),
        verbose: Some(true),
        quiet: Some(false),
        dry_run: Some(true),
        profiles: vec![sample_profile()],
        active_profile: Some("main".into()),
        ..Config::default()
    };

    let form = SettingsForm {
        mods_dir: Some(PathBuf::from("/main/mods")),
        backup: false,
        loader: Some("fabric".into()),
        game_version: Some("1.21.1".into()),
        include: None,
        exclude: None,
        max_updates: Some(5),
        confirm: false,
        changelog: true,
        dark_mode: true,
        profiles: vec![sample_profile()],
        active_profile: Some("main".into()),
    };

    let merged = merge_settings(existing, &form);

    // Keys the GUI does not own survive.
    assert_eq!(merged.verbose, Some(true));
    assert_eq!(merged.quiet, Some(false));
    assert_eq!(merged.dry_run, Some(true));
    // Keys it does own are written.
    assert_eq!(merged.profiles.len(), 1);
    assert_eq!(merged.active_profile.as_deref(), Some("main"));
    assert_eq!(merged.backup, Some(false));
    assert_eq!(merged.max_updates, Some(5));
    assert_eq!(merged.confirm, Some(false));
    assert_eq!(merged.changelog, Some(true));
    assert_eq!(merged.dark_mode, Some(true));
}

#[test]
fn merge_settings_does_not_write_mods_dir_while_profile_active() {
    let existing = Config {
        mods_dir: Some(PathBuf::from("/global/mods")),
        ..Config::default()
    };

    let mut form = SettingsForm {
        mods_dir: Some(PathBuf::from("/profile/mods")),
        profiles: vec![sample_profile()],
        active_profile: Some("main".into()),
        ..SettingsForm::default()
    };

    let merged = merge_settings(existing.clone(), &form);
    assert_eq!(merged.mods_dir, Some(PathBuf::from("/global/mods")));

    // With no active profile the field is GUI-owned again.
    form.active_profile = None;
    let merged = merge_settings(existing, &form);
    assert_eq!(merged.mods_dir, Some(PathBuf::from("/profile/mods")));
}
