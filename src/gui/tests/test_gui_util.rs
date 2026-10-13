//! Unit tests for pure GUI helpers.

use super::*;

// ── slugify ───────────────────────────────────────────────────────────────

#[test]
fn slugify_basic() {
    assert_eq!(slugify("My Profile"), "my-profile");
}

#[test]
fn slugify_removes_leading_trailing_dashes() {
    assert_eq!(slugify("  Hello  "), "hello");
    assert_eq!(slugify("--test--"), "test");
}

#[test]
fn slugify_preserves_alphanumerics() {
    assert_eq!(slugify("fabric123"), "fabric123");
}

#[test]
fn slugify_empty() {
    assert_eq!(slugify(""), "");
    assert_eq!(slugify("   "), "");
}

// ── split_patterns ────────────────────────────────────────────────────────

#[test]
fn split_patterns_blank_returns_none() {
    assert_eq!(split_patterns(""), None);
    assert_eq!(split_patterns("   "), None);
}

#[test]
fn split_patterns_single() {
    assert_eq!(split_patterns("fabric"), Some(vec!["fabric".to_string()]));
}

#[test]
fn split_patterns_multiple() {
    assert_eq!(
        split_patterns("fabric, forge"),
        Some(vec!["fabric".to_string(), "forge".to_string()])
    );
}

#[test]
fn split_patterns_filters_empty() {
    assert_eq!(
        split_patterns("a,,b, ,c"),
        Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
    );
}

// ── instance_label ────────────────────────────────────────────────────────

#[test]
fn instance_label_returns_none_for_standard_minecraft() {
    // The official launcher path — no instance name.
    let path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/home/test"))
        .join(".minecraft")
        .join("mods");
    assert_eq!(instance_label(&path), None);
}

#[test]
fn instance_label_returns_instance_name() {
    let path = std::path::Path::new("/home/user/instances/spark/minecraft/mods");
    assert_eq!(instance_label(path), Some("spark".to_string()));
}

#[test]
fn instance_label_returns_none_for_hidden_or_mods_parent() {
    // When parent is ".something" (hidden), returns None.
    let path = std::path::Path::new("/home/user/.cache/mods");
    assert_eq!(instance_label(path), None);
}

// ── outcome helpers ───────────────────────────────────────────────────────

#[test]
fn outcome_label_updated_returns_slug() {
    let o = ModOutcome::Updated {
        slug: "sodium".into(),
        old_filename: "sodium-1.0.jar".into(),
        new_filename: "sodium-2.0.jar".into(),
        old_version: "1.0".into(),
        new_version: "2.0".into(),
    };
    assert_eq!(outcome_label(&o), "sodium");
    assert_eq!(outcome_current(&o), "1.0");
    assert_eq!(outcome_latest(&o), "2.0");
}

#[test]
fn outcome_label_uptodate_returns_slug() {
    let o = ModOutcome::UpToDate {
        slug: "lithium".into(),
        filename: "lithium-3.0.jar".into(),
        version: "3.0".into(),
    };
    assert_eq!(outcome_label(&o), "lithium");
    assert_eq!(outcome_current(&o), "3.0");
    assert_eq!(outcome_latest(&o), "3.0");
}

#[test]
fn outcome_label_unknown_returns_filename() {
    let o = ModOutcome::Unknown {
        filename: "mystery.jar".into(),
    };
    assert_eq!(outcome_label(&o), "mystery.jar");
    assert_eq!(outcome_current(&o), "\u{2014}");
    assert_eq!(outcome_latest(&o), "\u{2014}");
}

#[test]
fn outcome_label_failed_returns_filename() {
    let o = ModOutcome::Failed {
        filename: "broken.jar".into(),
        error: "network error".into(),
    };
    assert_eq!(outcome_label(&o), "broken.jar");
}

// ── parse_imported_csv ────────────────────────────────────────────────────

#[test]
fn parse_csv_empty() {
    assert!(parse_imported_csv("").is_empty());
}

#[test]
fn parse_csv_header_only() {
    let content = "Filename,Name,Version,Project ID,Loader,Game Versions\n";
    assert!(parse_imported_csv(content).is_empty());
}

#[test]
fn parse_csv_single_row() {
    let content = "Filename,Name,Version,Project ID,Loader,Game Versions\n\
                   sodium.jar,Sodium,1.0.0,abc123,fabric,1.21";
    let mods = parse_imported_csv(content);
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].project_id, "abc123");
    assert_eq!(mods[0].name.as_deref(), Some("Sodium"));
    assert_eq!(mods[0].version_number.as_deref(), Some("1.0.0"));
    assert_eq!(mods[0].loaders, vec!["fabric"]);
    assert_eq!(mods[0].game_versions, vec!["1.21"]);
}

#[test]
fn parse_csv_quoted_fields() {
    let content = "Filename,Name,Version,Project ID,Loader,Game Versions\n\
                   \"sodium.jar\",\"Sodium\",\"1.0.0\",\"abc123\",\"fabric\",\"1.21\"";
    let mods = parse_imported_csv(content);
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].project_id, "abc123");
}

#[test]
fn parse_csv_skips_blank_lines() {
    let content = "Filename,Name,Version,Project ID\n\
                   a.jar,A,1.0,proj1\n\
                   \n\
                   b.jar,B,2.0,proj2\n";
    let mods = parse_imported_csv(content);
    assert_eq!(mods.len(), 2);
}

#[test]
fn parse_csv_skips_rows_without_project_id() {
    let content = "Filename,Name,Version,Project ID\n\
                   a.jar,A,1.0,\n\
                   b.jar,B,2.0,proj2\n";
    let mods = parse_imported_csv(content);
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].project_id, "proj2");
}
