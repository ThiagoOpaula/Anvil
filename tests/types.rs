mod common;

use anvil::types::*;

// ── ProjectStatus::is_problematic ───────────────────────────────────

#[test]
fn project_status_archived_is_problematic() {
    assert!(ProjectStatus::Archived.is_problematic());
}

#[test]
fn project_status_rejected_is_problematic() {
    assert!(ProjectStatus::Rejected.is_problematic());
}

#[test]
fn project_status_withheld_is_problematic() {
    assert!(ProjectStatus::Withheld.is_problematic());
}

#[test]
fn project_status_approved_is_not_problematic() {
    assert!(!ProjectStatus::Approved.is_problematic());
}

#[test]
fn project_status_draft_is_not_problematic() {
    assert!(!ProjectStatus::Draft.is_problematic());
}

#[test]
fn project_status_unknown_is_not_problematic() {
    assert!(!ProjectStatus::Unknown.is_problematic());
}

// ── Serde round-trips ──────────────────────────────────────────────

#[test]
fn mod_version_round_trip() {
    let json = r#"{
        "id": "ver-1",
        "project_id": "proj-1",
        "name": "Sodium",
        "version_number": "0.5.11",
        "loaders": ["fabric"],
        "game_versions": ["1.21"],
        "files": [],
        "dependencies": []
    }"#;
    let v: ModVersion = serde_json::from_str(json).unwrap();
    assert_eq!(v.name, "Sodium");
    assert_eq!(v.version_number, "0.5.11");
    let re_encoded = serde_json::to_string(&v).unwrap();
    let v2: ModVersion = serde_json::from_str(&re_encoded).unwrap();
    assert_eq!(v2.id, v.id);
}

#[test]
fn mod_version_defaults_missing_fields() {
    let json = r#"{"id":"v1","project_id":"p1","name":"Test","version_number":"1.0"}"#;
    let v: ModVersion = serde_json::from_str(json).unwrap();
    assert!(v.changelog.is_none());
    assert!(v.loaders.is_empty());
    assert!(v.game_versions.is_empty());
    assert!(v.files.is_empty());
    assert!(v.dependencies.is_empty());
}

#[test]
fn project_round_trip() {
    let json = r#"{
        "id": "proj-1",
        "slug": "sodium",
        "title": "Sodium",
        "status": "approved"
    }"#;
    let p: Project = serde_json::from_str(json).unwrap();
    assert_eq!(p.slug, "sodium");
    assert_eq!(p.status, ProjectStatus::Approved);
}

#[test]
fn project_status_unknown_serde_catch_all() {
    let json = r#"{"id":"p1","slug":"mod","title":"Mod","status":"future_status"}"#;
    let p: Project = serde_json::from_str(json).unwrap();
    assert_eq!(p.status, ProjectStatus::Unknown);
}

#[test]
fn dependency_type_serde_lowercase() {
    let json = r#""required""#;
    let dt: DependencyType = serde_json::from_str(json).unwrap();
    assert_eq!(dt, DependencyType::Required);

    let encoded = serde_json::to_string(&DependencyType::Incompatible).unwrap();
    assert_eq!(encoded, r#""incompatible""#);
}

#[test]
fn lock_file_round_trip() {
    let lock = LockFile {
        version: 1,
        updated_at: "2024-01-01T00:00:00Z".into(),
        target_game_version: Some("1.21".into()),
        target_loader: Some("fabric".into()),
        mods: vec![LockedMod {
            filename: "sodium.jar".into(),
            sha1: "abc123".into(),
            project_id: "p1".into(),
            slug: "sodium".into(),
            version_id: "v1".into(),
            version_number: "0.5.11".into(),
            loaders: vec!["fabric".into()],
            game_versions: vec!["1.21".into()],
        }],
    };
    let json = serde_json::to_string_pretty(&lock).unwrap();
    let lock2: LockFile = serde_json::from_str(&json).unwrap();
    assert_eq!(lock2.version, 1);
    assert_eq!(lock2.mods.len(), 1);
    assert_eq!(lock2.mods[0].slug, "sodium");
}

#[test]
fn config_default_all_none() {
    let config = Config::default();
    assert!(config.mods_dir.is_none());
    assert!(config.backup.is_none());
    assert!(config.loader.is_none());
    assert!(config.game_version.is_none());
    assert!(config.include.is_none());
    assert!(config.exclude.is_none());
    assert!(config.max_updates.is_none());
}

#[test]
fn config_deserialize_from_toml() {
    let toml_str = r#"
mods_dir = "/path/to/mods"
backup = false
loader = "fabric"
game_version = "1.21"
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.mods_dir, Some(std::path::PathBuf::from("/path/to/mods")));
    assert_eq!(config.backup, Some(false));
    assert_eq!(config.loader, Some("fabric".to_string()));
    assert_eq!(config.game_version, Some("1.21".to_string()));
}

#[test]
fn filter_opts_default_empty() {
    let opts = FilterOpts::default();
    assert!(opts.include.is_empty());
    assert!(opts.exclude.is_empty());
    assert!(opts.loader.is_none());
    assert!(opts.game_version.is_none());
}

#[test]
fn run_summary_default_all_zero() {
    let s = RunSummary::default();
    assert_eq!(s.total_jars, 0);
    assert_eq!(s.identified, 0);
    assert_eq!(s.unknown, 0);
    assert_eq!(s.updates_available, 0);
    assert_eq!(s.updates_applied, 0);
    assert_eq!(s.up_to_date, 0);
    assert_eq!(s.unavailable, 0);
    assert_eq!(s.skipped, 0);
    assert_eq!(s.failed, 0);
}

#[test]
fn locked_mod_partial_eq() {
    let a = LockedMod {
        filename: "a.jar".into(),
        sha1: "abc".into(),
        project_id: "p1".into(),
        slug: "mod".into(),
        version_id: "v1".into(),
        version_number: "1.0".into(),
        loaders: vec!["fabric".into()],
        game_versions: vec!["1.21".into()],
    };
    let b = LockedMod {
        filename: "a.jar".into(),
        sha1: "abc".into(),
        project_id: "p1".into(),
        slug: "mod".into(),
        version_id: "v1".into(),
        version_number: "1.0".into(),
        loaders: vec!["fabric".into()],
        game_versions: vec!["1.21".into()],
    };
    let c = LockedMod {
        filename: "b.jar".into(),
        sha1: "def".into(),
        project_id: "p2".into(),
        slug: "other".into(),
        version_id: "v2".into(),
        version_number: "2.0".into(),
        loaders: vec!["forge".into()],
        game_versions: vec!["1.20".into()],
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}
