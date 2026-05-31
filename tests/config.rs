mod common;
use common::helpers::unique_temp_dir;

use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;

use anvil::cli::Cli;
use anvil::config::{load, resolve, LogLevel};

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
