//! Integration tests for the `anvil::scanner` module.
//!
//! Tests JAR file discovery, SHA1 hashing, and batch identification against
//! a mock API client.

mod common;
use common::helpers::*;
use common::mocks::*;

use std::fs;
use std::path::{Path, PathBuf};

use anvil::scanner;

// ── find_jars ───────────────────────────────────────────────────────

#[test]
fn find_jars_nonexistent_dir() {
    let result = scanner::find_jars(Path::new("/nonexistent/dir/abc123")).unwrap();
    assert!(result.is_empty());
}

#[test]
fn find_jars_discovers_jar_files() {
    let dir = std::env::temp_dir().join(format!("anvil-find-jars-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(dir.join("mod1.jar"), b"hello").unwrap();
    fs::write(dir.join("mod2.JAR"), b"world").unwrap();
    fs::write(dir.join("readme.txt"), b"not a jar").unwrap();
    fs::create_dir(dir.join("subdir")).unwrap();

    let jars = scanner::find_jars(&dir).unwrap();
    assert_eq!(
        jars.len(),
        2,
        "should find exactly 2 JAR files (case-insensitive)"
    );

    // JARs should be sorted by filename.
    let names: Vec<String> = jars
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_lowercase())
        .collect();
    assert!(names[0] <= names[1], "JARs should be sorted");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn find_jars_empty_directory() {
    let dir = std::env::temp_dir().join(format!("anvil-empty-dir-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let jars = scanner::find_jars(&dir).unwrap();
    assert!(jars.is_empty());

    let _ = fs::remove_dir_all(&dir);
}

// ── compute_sha1 ────────────────────────────────────────────────────

#[test]
fn compute_sha1_known_digest() {
    // empty file → da39a3ee5e6b4b0d3255bfef95601890afd80709
    let dir = std::env::temp_dir().join("mod-updater-test-sha1");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("empty.bin");
    fs::write(&path, b"").unwrap();

    let hash = scanner::compute_sha1(&path).unwrap();
    assert_eq!(hash, "da39a3ee5e6b4b0d3255bfef95601890afd80709");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn compute_sha1_known_content() {
    let dir = std::env::temp_dir().join(format!("anvil-sha1-known-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("hello.bin");
    fs::write(&path, b"hello world").unwrap();

    let hash = scanner::compute_sha1(&path).unwrap();
    // SHA1 of "hello world" is 2aae6c35c94fcfb415dbe95f408b9ce91ee846ed
    assert_eq!(hash, "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn compute_sha1_nonexistent_file() {
    let result = scanner::compute_sha1(Path::new("/nonexistent/file.bin"));
    assert!(result.is_err());
}

// ── hash_all ────────────────────────────────────────────────────────

#[test]
fn hash_all_processes_valid_files() {
    let dir = std::env::temp_dir().join(format!("anvil-hash-all-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let path1 = dir.join("a.jar");
    let path2 = dir.join("b.jar");
    fs::write(&path1, b"content1").unwrap();
    fs::write(&path2, b"content2").unwrap();

    let hashes = scanner::hash_all(&[path1.clone(), path2.clone()]);
    assert_eq!(hashes.len(), 2);
    // First entry should be the first file.
    assert_eq!(hashes[0].0, path1);
    assert!(!hashes[0].1.is_empty());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hash_all_skips_unreadable_files() {
    let dir = std::env::temp_dir().join(format!("anvil-hash-skip-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let valid = dir.join("valid.jar");
    let nonexistent = dir.join("missing.jar");
    fs::write(&valid, b"data").unwrap();

    let hashes = scanner::hash_all(&[valid.clone(), nonexistent]);
    // The nonexistent file should be skipped; only the valid one returned.
    assert_eq!(hashes.len(), 1);
    assert_eq!(hashes[0].0, valid);

    let _ = fs::remove_dir_all(&dir);
}

// ── batch_identify ──────────────────────────────────────────────────

#[tokio::test]
async fn batch_identify_known_mods() {
    let api = MockApi::new();
    api.set_version("hash1", Some(make_test_version("v1", "Mod1", "1.0")));
    api.set_version("hash2", Some(make_test_version("v2", "Mod2", "2.0")));

    let hashes = vec![
        (PathBuf::from("/mods/mod1.jar"), "hash1".to_string()),
        (PathBuf::from("/mods/mod2.jar"), "hash2".to_string()),
    ];

    let results = scanner::batch_identify(&hashes, &api).await;
    assert_eq!(results.len(), 2);
    assert!(results[0].is_some());
    assert!(results[1].is_some());
    assert_eq!(results[0].as_ref().unwrap().sha1, "hash1");
    assert_eq!(results[1].as_ref().unwrap().sha1, "hash2");
}

#[tokio::test]
async fn batch_identify_unknown_mod() {
    let api = MockApi::new();
    // Don't set any versions — all hashes are unknown.

    let hashes = vec![(PathBuf::from("/mods/unknown.jar"), "unknown-hash".to_string())];

    let results = scanner::batch_identify(&hashes, &api).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].is_none());
}

#[tokio::test]
async fn batch_identify_mixed_results() {
    let api = MockApi::new();
    api.set_version("good", Some(make_test_version("v1", "GoodMod", "1.0")));
    // "bad" not set → None

    let hashes = vec![
        (PathBuf::from("/mods/good.jar"), "good".to_string()),
        (PathBuf::from("/mods/bad.jar"), "bad".to_string()),
    ];

    let results = scanner::batch_identify(&hashes, &api).await;
    assert_eq!(results.len(), 2);
    assert!(results[0].is_some());
    assert!(results[1].is_none());
}

#[tokio::test]
async fn batch_identify_empty_input() {
    let api = MockApi::new();
    let results = scanner::batch_identify(&[], &api).await;
    assert!(results.is_empty());
}
