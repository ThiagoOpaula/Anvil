//! Unit tests for AppState pure methods.

use super::*;
use crate::types::{IdentifiedMod, ModVersion};

fn make_version(name: &str, version: &str, loaders: &[&str], game_versions: &[&str]) -> ModVersion {
    ModVersion {
        id: format!("ver-{}", name),
        project_id: format!("proj-{}", name),
        name: name.to_string(),
        version_number: version.to_string(),
        changelog: Some(String::new()),
        loaders: loaders.iter().map(|s| s.to_string()).collect(),
        game_versions: game_versions.iter().map(|s| s.to_string()).collect(),
        files: vec![],
        dependencies: vec![],
    }
}

fn make_identified(name: &str, loaders: &[&str], game_versions: &[&str]) -> IdentifiedMod {
    IdentifiedMod {
        path: std::path::PathBuf::from(format!("{}.jar", name)),
        sha1: "abc123".to_string(),
        filename: format!("{}.jar", name),
        current_version: make_version(name, "1.0.0", loaders, game_versions),
    }
}

fn test_config() -> crate::config::ResolvedConfig {
    crate::config::ResolvedConfig {
        mods_dir: std::path::PathBuf::from("/tmp/mods"),
        backup: true,
        loader: None,
        game_version: None,
        include: vec![],
        exclude: vec![],
        max_updates: None,
        log_level: crate::config::LogLevel::Info,
        dry_run: false,
        confirm: true,
        changelog: false,
        dark_mode: true,
        profiles: vec![],
        active_profile: None,
        config_path: std::path::PathBuf::from("/tmp/config.toml"),
    }
}

fn test_state() -> AppState {
    AppState::from_config(&test_config())
}

// ── from_config ───────────────────────────────────────────────────────────

#[test]
fn from_config_sets_defaults() {
    let state = test_state();
    assert_eq!(state.active_tab, Tab::Scan);
    assert!(!state.worker_busy);
    assert!(state.identified_mods.is_empty());
    assert_eq!(state.total_jars, 0);
}

// ── filtered_mods ─────────────────────────────────────────────────────────

#[test]
fn filtered_mods_empty_returns_empty() {
    let state = test_state();
    assert!(state.filtered_mods().is_empty());
}

#[test]
fn filtered_mods_no_filters_returns_all() {
    let mut state = test_state();
    state.identified_mods = vec![
        make_identified("alpha", &["fabric"], &["1.21"]),
        make_identified("beta", &["forge"], &["1.20"]),
    ];
    assert_eq!(state.filtered_mods().len(), 2);
}

#[test]
fn filtered_mods_by_loader() {
    let mut state = test_state();
    state.identified_mods = vec![
        make_identified("alpha", &["fabric"], &["1.21"]),
        make_identified("beta", &["forge"], &["1.20"]),
    ];
    state.filter_loader = "fabric".to_string();
    let result = state.filtered_mods();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].current_version.name, "alpha");
}

#[test]
fn filtered_mods_by_game_version() {
    let mut state = test_state();
    state.identified_mods = vec![
        make_identified("alpha", &["fabric"], &["1.21"]),
        make_identified("beta", &["forge"], &["1.20"]),
    ];
    state.filter_game_version = "1.21".to_string();
    let result = state.filtered_mods();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].current_version.name, "alpha");
}

#[test]
fn filtered_mods_by_include() {
    let mut state = test_state();
    state.identified_mods = vec![
        make_identified("sodium", &["fabric"], &["1.21"]),
        make_identified("lithium", &["fabric"], &["1.21"]),
    ];
    state.filter_include = "sodium".to_string();
    let result = state.filtered_mods();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].current_version.name, "sodium");
}

#[test]
fn filtered_mods_by_exclude() {
    let mut state = test_state();
    state.identified_mods = vec![
        make_identified("sodium", &["fabric"], &["1.21"]),
        make_identified("lithium", &["fabric"], &["1.21"]),
    ];
    state.filter_exclude = "sodium".to_string();
    let result = state.filtered_mods();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].current_version.name, "lithium");
}

// ── export_scan_results ───────────────────────────────────────────────────

#[test]
fn export_csv_has_header() {
    let state = test_state();
    let csv = state.export_scan_results(ExportFormat::Csv);
    assert!(csv.starts_with("Filename,Name,Version,Project ID,Loader,Game Versions\n"));
}

#[test]
fn export_json_empty_is_array() {
    let state = test_state();
    let json = state.export_scan_results(ExportFormat::Json);
    assert_eq!(json, "[]");
}

#[test]
fn export_markdown_has_separator() {
    let state = test_state();
    let md = state.export_scan_results(ExportFormat::Markdown);
    assert!(md.contains("| --- |"));
}

// ── set_imported_mods_deduped ─────────────────────────────────────────────

#[test]
fn import_dedup_skips_existing() {
    let mut state = test_state();
    state.identified_mods = vec![make_identified("alpha", &["fabric"], &["1.21"])];
    state.set_imported_mods_deduped(vec![
        crate::types::ImportedMod {
            project_id: "proj-alpha".to_string(),
            slug: Some("alpha".into()),
            name: Some("Alpha".into()),
            version_number: None,
            loaders: vec![],
            game_versions: vec![],
        },
        crate::types::ImportedMod {
            project_id: "proj-new".to_string(),
            slug: Some("new-mod".into()),
            name: Some("New Mod".into()),
            version_number: None,
            loaders: vec![],
            game_versions: vec![],
        },
    ]);
    assert_eq!(state.imported_mods.len(), 1);
    assert_eq!(state.imported_mods[0].project_id, "proj-new");
}

// ── Tab ───────────────────────────────────────────────────────────────────

#[test]
fn tab_labels_are_distinct() {
    assert_ne!(Tab::Scan.label(), Tab::Updates.label());
    assert_ne!(Tab::Scan.label(), Tab::Settings.label());
    assert_ne!(Tab::Scan.label(), Tab::Rollback.label());
}

#[test]
fn tab_icons_are_distinct() {
    assert_ne!(Tab::Scan.icon(), Tab::Updates.icon());
}
