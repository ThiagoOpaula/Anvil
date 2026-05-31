use super::*;
use crate::test_utils::{make_test_version, unique_temp_dir};
use crate::types::{Project, ProjectStatus};
use std::fs;

fn temp_cache() -> ApiCache {
    let dir = unique_temp_dir("cache");
    ApiCache { root: dir }
}

// ── Key generation ──────────────────────────────────────────────────

#[test]
fn update_key_is_deterministic() {
    let cache = temp_cache();
    let loaders: Vec<String> = vec!["fabric".into(), "quilt".into()];
    let gv: Vec<String> = vec!["1.21".into(), "1.20.4".into()];
    let key1 = cache.update_key("abc123", &loaders, &gv);
    let key2 = cache.update_key("abc123", &loaders, &gv);
    assert_eq!(key1, key2);
}

#[test]
fn update_key_sorts_loaders() {
    let cache = temp_cache();
    let key1 = cache.update_key(
        "abc",
        &["fabric".into(), "quilt".into()],
        &["1.21".into()],
    );
    let key2 = cache.update_key(
        "abc",
        &["quilt".into(), "fabric".into()],
        &["1.21".into()],
    );
    assert_eq!(key1, key2, "key should be order-independent for loaders");
}

#[test]
fn update_key_sorts_game_versions() {
    let cache = temp_cache();
    let key1 = cache.update_key(
        "abc",
        &["fabric".into()],
        &["1.21".into(), "1.20.4".into()],
    );
    let key2 = cache.update_key(
        "abc",
        &["fabric".into()],
        &["1.20.4".into(), "1.21".into()],
    );
    assert_eq!(
        key1, key2,
        "key should be order-independent for game versions"
    );
}

#[test]
fn update_key_differs_on_sha1() {
    let cache = temp_cache();
    let key1 = cache.update_key("abc", &[], &[]);
    let key2 = cache.update_key("def", &[], &[]);
    assert_ne!(key1, key2);
}

#[test]
fn update_key_differs_on_loaders() {
    let cache = temp_cache();
    let key1 = cache.update_key("abc", &["fabric".into()], &[]);
    let key2 = cache.update_key("abc", &["forge".into()], &[]);
    assert_ne!(key1, key2);
}

// ── Cache read/write round-trip ─────────────────────────────────────

#[test]
fn version_cache_round_trip() {
    let cache = temp_cache();
    let sha1 = "abc123def456";
    let version = make_test_version("ver-1", "TestMod", "1.0.0");

    cache.set_version(sha1, &version).unwrap();
    let retrieved = cache.get_version(sha1).unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().name, "TestMod");

    // Cleanup.
    let _ = fs::remove_dir_all(&cache.root);
}

#[test]
fn version_cache_miss_returns_none() {
    let cache = temp_cache();
    let result = cache.get_version("nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn update_cache_round_trip() {
    let cache = temp_cache();
    let sha1 = "abc123";
    let loaders: Vec<String> = vec!["fabric".into()];
    let gv: Vec<String> = vec!["1.21".into()];
    let version = make_test_version("ver-1", "TestMod", "1.0.0");

    cache
        .set_update(sha1, &loaders, &gv, &version)
        .unwrap();
    let retrieved = cache
        .get_update(sha1, &loaders, &gv)
        .unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().version_number, "1.0.0");

    let _ = fs::remove_dir_all(&cache.root);
}

#[test]
fn project_cache_round_trip() {
    let cache = temp_cache();
    let project = Project {
        id: "proj-1".into(),
        slug: "test-mod".into(),
        title: "Test Mod".into(),
        status: ProjectStatus::Approved,
    };

    cache.set_project("proj-1", &project).unwrap();
    let retrieved = cache.get_project("proj-1").unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().slug, "test-mod");

    let _ = fs::remove_dir_all(&cache.root);
}

#[test]
fn project_cache_miss_returns_none() {
    let cache = temp_cache();
    let result = cache.get_project("no-such-project").unwrap();
    assert!(result.is_none());
}

// ── Path helpers ────────────────────────────────────────────────────

#[test]
fn version_path_contains_sha1() {
    let cache = temp_cache();
    let path = cache.version_path("abc123");
    let path_str = path.to_string_lossy();
    assert!(path_str.contains("abc123"));
    assert!(path_str.ends_with(".json"));
}

#[test]
fn version_path_uses_prefix_dir() {
    let cache = temp_cache();
    let sha1 = "abc123def456";
    let path = cache.version_path(sha1);
    let parent = path.parent().unwrap();
    // The parent dir name should be the first 2 chars of sha1.
    assert!(parent.ends_with("ab"));
}

#[test]
fn update_path_contains_sha1() {
    let cache = temp_cache();
    let path = cache.update_path("abc", &[], &[]);
    let path_str = path.to_string_lossy();
    assert!(path_str.contains("abc"));
    assert!(path_str.ends_with(".json"));
}

#[test]
fn project_path_contains_project_id() {
    let cache = temp_cache();
    let path = cache.project_path("my-project-id");
    let path_str = path.to_string_lossy();
    assert!(path_str.contains("my-project-id"));
}

// ── Corrupt cache ───────────────────────────────────────────────────

#[test]
fn corrupt_cache_returns_none() {
    let cache = temp_cache();
    let sha1 = "corrupt-entry";
    let path = cache.version_path(sha1);

    // Write invalid JSON.
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&path, "this is not valid json").unwrap();

    let result = cache.get_version(sha1).unwrap();
    assert!(result.is_none(), "corrupt cache should return None");

    let _ = fs::remove_dir_all(&cache.root);
}

#[test]
fn overwrite_existing_cache_entry() {
    let cache = temp_cache();
    let sha1 = "overwrite-test";

    let v1 = make_test_version("ver-1", "TestMod", "1.0.0");
    cache.set_version(sha1, &v1).unwrap();

    let v2 = make_test_version("ver-1", "TestMod", "2.0.0");
    cache.set_version(sha1, &v2).unwrap();

    let retrieved = cache.get_version(sha1).unwrap();
    assert_eq!(retrieved.unwrap().version_number, "2.0.0");

    let _ = fs::remove_dir_all(&cache.root);
}
