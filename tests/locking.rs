//! Integration tests for the `anvil::locking` module.
//!
//! Tests lockfile read/write, state serialisation, and diffing between runs.

mod common;
use common::helpers::*;

use std::path::PathBuf;

use anvil::locking;
use anvil::types::{LockedMod, ModOutcome};

// ── build_locked_mods ────────────────────────────────────────────────

#[test]
fn build_locked_mods_updated() {
    let identified = vec![make_identified_mod(
        "sodium-0.5.jar",
        "proj-sodium",
        "Sodium",
        "0.5.11",
    )];

    let outcomes = vec![ModOutcome::Updated {
        slug: "sodium".into(),
        old_filename: "sodium-0.5.jar".into(),
        new_filename: "sodium-0.6.jar".into(),
        old_version: "0.5.11".into(),
        new_version: "0.6.0".into(),
    }];

    let locked = locking::build_locked_mods(&outcomes, &identified);
    assert_eq!(locked.len(), 1);
    assert_eq!(locked[0].filename, "sodium-0.6.jar");
    assert_eq!(locked[0].slug, "sodium");
    assert_eq!(locked[0].version_number, "0.6.0");
}

#[test]
fn build_locked_mods_up_to_date() {
    let identified = vec![make_identified_mod("iris.jar", "proj-iris", "Iris", "1.8.0")];

    let outcomes = vec![ModOutcome::UpToDate {
        slug: "iris".into(),
        filename: "iris.jar".into(),
        version: "1.8.0".into(),
    }];

    let locked = locking::build_locked_mods(&outcomes, &identified);
    assert_eq!(locked.len(), 1);
    assert_eq!(locked[0].version_number, "1.8.0");
}

#[test]
fn build_locked_mods_skips_failed() {
    let identified = vec![make_identified_mod("bad.jar", "proj-bad", "BadMod", "1.0")];

    let outcomes = vec![ModOutcome::Failed {
        filename: "bad.jar".into(),
        error: "download error".into(),
    }];

    let locked = locking::build_locked_mods(&outcomes, &identified);
    assert!(locked.is_empty());
}

// ── diff_lockfile ────────────────────────────────────────────────────

#[test]
fn diff_detects_changes() {
    let old = make_lockfile(vec![
        make_locked_mod("sodium", "0.5.11"),
        make_locked_mod("old-mod", "1.0"),
    ]);

    let new = make_lockfile(vec![
        make_locked_mod("sodium", "0.6.0"),
        make_locked_mod("iris", "1.8.0"),
    ]);

    let diff = locking::diff_lockfile(&old, &new);
    // sodium updated, old-mod removed, iris added
    assert_eq!(diff.len(), 3);
    assert!(diff.iter().any(|l| l.contains("Updated: sodium")));
    assert!(diff.iter().any(|l| l.contains("Removed: old-mod")));
    assert!(diff.iter().any(|l| l.contains("Added: iris")));
}

// ── lockfile_path ────────────────────────────────────────────────────

#[test]
fn lockfile_path_joins_correctly() {
    let mods_dir = PathBuf::from("/some/mods");
    let path = locking::lockfile_path(&mods_dir);
    assert_eq!(path, PathBuf::from("/some/mods/anvil.lock"));
}

#[test]
fn lockfile_path_joins_windows_style() {
    let mods_dir = PathBuf::from(r"C:\Users\mc\mods");
    let path = locking::lockfile_path(&mods_dir);
    assert_eq!(path, PathBuf::from(r"C:\Users\mc\mods\anvil.lock"));
}

// ── read_lockfile ────────────────────────────────────────────────────

#[test]
fn read_lockfile_missing_returns_none() {
    let dir = std::env::temp_dir().join("anvil-test-missing-lock");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let result = locking::read_lockfile(&dir).unwrap();
    assert!(result.is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_lockfile_valid_json_returns_some() {
    let dir = std::env::temp_dir().join("anvil-test-valid-lock");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let lockfile = make_lockfile(vec![LockedMod {
        filename: "test.jar".into(),
        sha1: "abc".into(),
        project_id: "p1".into(),
        slug: "test-mod".into(),
        version_id: "v1".into(),
        version_number: "1.0.0".into(),
        loaders: vec!["fabric".into()],
        game_versions: vec!["1.21".into()],
    }]);
    let json = serde_json::to_string_pretty(&lockfile).unwrap();
    std::fs::write(dir.join("anvil.lock"), json).unwrap();

    let result = locking::read_lockfile(&dir).unwrap();
    assert!(result.is_some());
    let read = result.unwrap();
    assert_eq!(read.version, 1);
    assert_eq!(read.mods.len(), 1);
    assert_eq!(read.mods[0].slug, "test-mod");
    assert_eq!(read.mods[0].version_number, "1.0.0");
    assert_eq!(read.mods[0].filename, "test.jar");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_lockfile_invalid_json_returns_err() {
    let dir = std::env::temp_dir().join("anvil-test-invalid-lock");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("anvil.lock"), "not valid json {{{").unwrap();

    let result = locking::read_lockfile(&dir);
    assert!(result.is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_lockfile_empty_file_returns_err() {
    let dir = std::env::temp_dir().join("anvil-test-empty-lock");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("anvil.lock"), "").unwrap();

    let result = locking::read_lockfile(&dir);
    assert!(result.is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

// ── write_lockfile (round-trip) ──────────────────────────────────────

#[test]
fn write_lockfile_round_trip() {
    let dir = std::env::temp_dir().join("anvil-test-write-lock");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mods = vec![
        LockedMod {
            filename: "mod-a.jar".into(),
            sha1: "sha1a".into(),
            project_id: "proj-a".into(),
            slug: "mod-a".into(),
            version_id: "ver-a".into(),
            version_number: "1.0.0".into(),
            loaders: vec!["fabric".into()],
            game_versions: vec!["1.21".into()],
        },
        LockedMod {
            filename: "mod-b.jar".into(),
            sha1: "sha1b".into(),
            project_id: "proj-b".into(),
            slug: "mod-b".into(),
            version_id: "ver-b".into(),
            version_number: "2.0.0".into(),
            loaders: vec!["forge".into()],
            game_versions: vec!["1.20".into()],
        },
    ];

    locking::write_lockfile(&dir, Some("1.21"), Some("fabric"), &mods).unwrap();

    // Verify file exists on disk
    let lock_path = locking::lockfile_path(&dir);
    assert!(lock_path.exists());

    // Round-trip: read it back
    let read = locking::read_lockfile(&dir).unwrap().expect("lockfile should exist");
    assert_eq!(read.mods.len(), 2);
    assert_eq!(read.target_game_version, Some("1.21".to_string()));
    assert_eq!(read.target_loader, Some("fabric".to_string()));
    assert_eq!(read.version, 1);
    assert!(!read.updated_at.is_empty());

    // Order preserved
    assert_eq!(read.mods[0].slug, "mod-a");
    assert_eq!(read.mods[0].version_number, "1.0.0");
    assert_eq!(read.mods[1].slug, "mod-b");
    assert_eq!(read.mods[1].version_number, "2.0.0");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_lockfile_with_none_filters() {
    let dir = std::env::temp_dir().join("anvil-test-write-lock-none");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mods = vec![LockedMod {
        filename: "x.jar".into(),
        sha1: "abc".into(),
        project_id: "p".into(),
        slug: "x".into(),
        version_id: "v".into(),
        version_number: "1.0".into(),
        loaders: vec![],
        game_versions: vec![],
    }];

    locking::write_lockfile(&dir, None, None, &mods).unwrap();

    let read = locking::read_lockfile(&dir).unwrap().unwrap();
    assert_eq!(read.target_game_version, None);
    assert_eq!(read.target_loader, None);
    assert_eq!(read.mods.len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_lockfile_overwrites_existing() {
    let dir = std::env::temp_dir().join("anvil-test-overwrite-lock");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let first = vec![LockedMod {
        filename: "first.jar".into(),
        sha1: "abc".into(),
        project_id: "p1".into(),
        slug: "first".into(),
        version_id: "v1".into(),
        version_number: "1.0".into(),
        loaders: vec![],
        game_versions: vec![],
    }];
    locking::write_lockfile(&dir, None, None, &first).unwrap();

    let second = vec![LockedMod {
        filename: "second.jar".into(),
        sha1: "def".into(),
        project_id: "p2".into(),
        slug: "second".into(),
        version_id: "v2".into(),
        version_number: "2.0".into(),
        loaders: vec![],
        game_versions: vec![],
    }];
    locking::write_lockfile(&dir, None, None, &second).unwrap();

    let read = locking::read_lockfile(&dir).unwrap().unwrap();
    assert_eq!(read.mods.len(), 1);
    assert_eq!(read.mods[0].slug, "second");

    let _ = std::fs::remove_dir_all(&dir);
}

// ── build_locked_mods ────────────────────────────────────────────────

#[test]
fn build_locked_mods_unavailable() {
    let identified = vec![make_identified_mod(
        "sodium.jar",
        "proj-sodium",
        "Sodium",
        "0.5.11",
    )];

    let outcomes = vec![ModOutcome::Unavailable {
        slug: "sodium".into(),
        filename: "sodium.jar".into(),
        current_version: "0.5.11".into(),
        game_version: "1.21".into(),
    }];

    let locked = locking::build_locked_mods(&outcomes, &identified);
    assert_eq!(locked.len(), 1);
    assert_eq!(locked[0].slug, "sodium");
    assert_eq!(locked[0].version_number, "0.5.11");
    assert_eq!(locked[0].filename, "sodium.jar");
    assert_eq!(locked[0].project_id, "proj-sodium");
}

#[test]
fn build_locked_mods_mixed_outcomes() {
    let identified = vec![
        make_identified_mod("updated.jar", "proj-up", "UpdatedMod", "1.0"),
        make_identified_mod("current.jar", "proj-cur", "CurrentMod", "2.0"),
        make_identified_mod("failed.jar", "proj-fail", "FailedMod", "3.0"),
    ];

    let outcomes = vec![
        ModOutcome::Updated {
            slug: "updated-mod".into(),
            old_filename: "updated.jar".into(),
            new_filename: "updated-v2.jar".into(),
            old_version: "1.0".into(),
            new_version: "2.0".into(),
        },
        ModOutcome::UpToDate {
            slug: "current-mod".into(),
            filename: "current.jar".into(),
            version: "2.0".into(),
        },
        ModOutcome::Failed {
            filename: "failed.jar".into(),
            error: "oops".into(),
        },
    ];

    let locked = locking::build_locked_mods(&outcomes, &identified);
    // Failed is skipped, so we expect 2 entries
    assert_eq!(locked.len(), 2);
    assert!(locked.iter().any(|m| m.slug == "updated-mod"));
    assert!(locked.iter().any(|m| m.slug == "current-mod"));
    assert!(locked.iter().all(|m| m.slug != "failed-mod"));
}

#[test]
fn build_locked_mods_empty_inputs() {
    let identified: Vec<anvil::types::IdentifiedMod> = vec![];
    let outcomes: Vec<ModOutcome> = vec![];
    let locked = locking::build_locked_mods(&outcomes, &identified);
    assert!(locked.is_empty());
}

#[test]
fn build_locked_mods_skips_unknown() {
    let identified = vec![make_identified_mod(
        "unknown.jar",
        "proj-unk",
        "UnknownMod",
        "1.0",
    )];

    let outcomes = vec![ModOutcome::Unknown {
        filename: "unknown.jar".into(),
    }];

    let locked = locking::build_locked_mods(&outcomes, &identified);
    assert!(locked.is_empty());
}

#[test]
fn build_locked_mods_skips_filtered_out() {
    let identified = vec![make_identified_mod(
        "filtered.jar",
        "proj-filt",
        "FilteredMod",
        "1.0",
    )];

    let outcomes = vec![ModOutcome::FilteredOut {
        filename: "filtered.jar".into(),
        reason: "excluded".into(),
    }];

    let locked = locking::build_locked_mods(&outcomes, &identified);
    assert!(locked.is_empty());
}

#[test]
fn build_locked_mods_outcome_no_identified_match() {
    // Outcome references a filename not in the identified list
    let identified = vec![make_identified_mod(
        "other.jar",
        "proj-other",
        "OtherMod",
        "1.0",
    )];

    let outcomes = vec![ModOutcome::Updated {
        slug: "orphan".into(),
        old_filename: "nonexistent.jar".into(),
        new_filename: "nonexistent-v2.jar".into(),
        old_version: "1.0".into(),
        new_version: "2.0".into(),
    }];

    let locked = locking::build_locked_mods(&outcomes, &identified);
    assert!(locked.is_empty());
}

// ── diff_lockfile ────────────────────────────────────────────────────

#[test]
fn diff_lockfile_no_changes() {
    let old = make_lockfile(vec![
        make_locked_mod("sodium", "0.5.11"),
        make_locked_mod("iris", "1.8.0"),
    ]);
    let new = make_lockfile(vec![
        make_locked_mod("sodium", "0.5.11"),
        make_locked_mod("iris", "1.8.0"),
    ]);

    let diff = locking::diff_lockfile(&old, &new);
    assert!(diff.is_empty());
}

#[test]
fn diff_lockfile_only_additions() {
    let old = make_lockfile(vec![make_locked_mod("sodium", "0.5.11")]);
    let new = make_lockfile(vec![
        make_locked_mod("sodium", "0.5.11"), // same version, unchanged
        make_locked_mod("iris", "1.8.0"),    // new
    ]);

    let diff = locking::diff_lockfile(&old, &new);
    assert_eq!(diff.len(), 1);
    assert!(diff[0].contains("Added: iris"));
    assert!(diff[0].contains("v1.8.0"));
}

#[test]
fn diff_lockfile_only_removals() {
    let old = make_lockfile(vec![
        make_locked_mod("sodium", "0.5.11"),
        make_locked_mod("old-mod", "1.0.0"),
    ]);
    let new = make_lockfile(vec![
        make_locked_mod("sodium", "0.5.11"), // same version, unchanged
    ]);

    let diff = locking::diff_lockfile(&old, &new);
    assert_eq!(diff.len(), 1);
    assert!(diff[0].contains("Removed: old-mod"));
    assert!(diff[0].contains("v1.0.0"));
}

#[test]
fn diff_lockfile_only_updates() {
    let old = make_lockfile(vec![make_locked_mod("sodium", "0.5.11")]);
    let new = make_lockfile(vec![make_locked_mod("sodium", "0.6.0")]);

    let diff = locking::diff_lockfile(&old, &new);
    assert_eq!(diff.len(), 1);
    assert!(diff[0].contains("Updated: sodium"));
    assert!(diff[0].contains("0.5.11"));
    assert!(diff[0].contains("0.6.0"));
}

#[test]
fn diff_lockfile_both_empty() {
    let old = make_lockfile(vec![]);
    let new = make_lockfile(vec![]);

    let diff = locking::diff_lockfile(&old, &new);
    assert!(diff.is_empty());
}

#[test]
fn diff_lockfile_same_mods_different_order() {
    let old = make_lockfile(vec![
        make_locked_mod("a", "1.0"),
        make_locked_mod("b", "2.0"),
    ]);
    let new = make_lockfile(vec![
        make_locked_mod("b", "2.0"),
        make_locked_mod("a", "1.0"),
    ]);

    let diff = locking::diff_lockfile(&old, &new);
    // Same slugs and versions, just different order — no diff expected
    assert!(diff.is_empty());
}
