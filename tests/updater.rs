//! Integration tests for the `anvil::updater` module.
//!
//! Tests the full update pipeline end-to-end using mock API and progress
//! implementations from `tests/common/`.

mod common;
use common::helpers::*;
use common::mocks::*;

use std::fs;

use anvil::cache::ApiCache;
use anvil::error::Error;

// ═══════════════════════════════════════════════════════════════════════════
// ── 1. Empty mods directory → RunSummary::default() ───────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn empty_mods_dir_returns_default_summary() {
    let dir = setup_temp_mods_dir(&[]);
    let config = make_config(&dir);
    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.total_jars, 0);
    assert_eq!(summary.identified, 0);
    assert_eq!(summary.unknown, 0);
    assert_eq!(summary.updates_available, 0);
    assert_eq!(summary.updates_applied, 0);
    assert_eq!(summary.up_to_date, 0);
    assert_eq!(summary.unavailable, 0);
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.failed, 0);

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 2. Identify mods, report up-to-date ───────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn identifies_mods_and_reports_up_to_date() {
    let content = b"test-mod-content-v1";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[("sodium-0.5.jar", content)]);
    let config = make_config(&dir);
    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();

    // Same version id → up to date.
    let current_version = make_version("ver-1", "proj-sodium", "Sodium", "0.5.11", vec![]);
    api.set_version(&sha1, Some(current_version.clone()));
    api.set_latest(&sha1, Some(current_version)); // same id → no update

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.total_jars, 1);
    assert_eq!(summary.identified, 1);
    assert_eq!(summary.unknown, 0);
    assert_eq!(summary.updates_available, 0);
    assert_eq!(summary.updates_applied, 0);
    assert_eq!(summary.up_to_date, 1);
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.failed, 0);

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 3. Detect updates available & download ────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn detects_updates_available() {
    let content = b"mod-content-for-update-test";
    let sha1 = sha1_hex(content);
    let download_content = b"mod-content-v2-updated";
    let download_sha1 = sha1_hex(download_content);

    let dir = setup_temp_mods_dir(&[("testmod-1.0.jar", content)]);
    let mut config = make_config(&dir);
    config.confirm = true; // will prompt

    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();
    progress.push_confirm(true);

    // Current version.
    let current = make_version(
        "ver-1",
        "proj-test",
        "TestMod",
        "1.0.0",
        vec![],
    );
    api.set_version(&sha1, Some(current));

    // Newer version with a file.
    let new_file = make_file(
        "https://example.com/testmod-2.0.jar",
        "testmod-2.0.jar",
        &download_sha1,
    );
    let latest = make_version(
        "ver-2",
        "proj-test",
        "TestMod",
        "2.0.0",
        vec![new_file],
    );
    api.set_latest(&sha1, Some(latest.clone()));
    api.set_project("proj-test", make_project("proj-test", "testmod", "TestMod"));
    api.set_download_content(download_content.to_vec(), None); // real SHA1

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.total_jars, 1);
    assert_eq!(summary.identified, 1);
    assert_eq!(summary.updates_available, 1);
    assert_eq!(summary.updates_applied, 1);
    assert_eq!(summary.failed, 0);

    // Verify the new file was written to the mods dir.
    assert!(dir.join("testmod-2.0.jar").exists());

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 4. Download and update — verify Updated outcome details ───────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn downloads_and_updates_mods() {
    let content = b"old-mod-content";
    let sha1 = sha1_hex(content);
    let new_content = b"new-mod-content";
    let new_sha1 = sha1_hex(new_content);

    let dir = setup_temp_mods_dir(&[("iris-1.7.jar", content)]);
    let mut config = make_config(&dir);
    config.confirm = true;
    config.changelog = true;

    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();
    progress.push_confirm(true);

    let current = make_version("v1", "proj-iris", "Iris", "1.7.0", vec![]);
    api.set_version(&sha1, Some(current));

    let new_file = make_file(
        "https://dl.example.com/iris-1.8.jar",
        "iris-1.8.jar",
        &new_sha1,
    );
    let mut latest = make_version(
        "v2",
        "proj-iris",
        "Iris",
        "1.8.0",
        vec![new_file],
    );
    latest.changelog = Some("Fixed bugs and improved performance.".into());
    api.set_latest(&sha1, Some(latest));
    api.set_project("proj-iris", make_project("proj-iris", "iris", "Iris"));
    api.set_download_content(new_content.to_vec(), None);

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.updates_available, 1);
    assert_eq!(summary.updates_applied, 1);
    assert_eq!(summary.failed, 0);

    // New file exists, old file was removed (backup disabled).
    assert!(dir.join("iris-1.8.jar").exists());
    assert!(!dir.join("iris-1.7.jar").exists());

    // Changelog was printed.
    let changelogs = progress.changelogs.lock().unwrap();
    assert_eq!(changelogs.len(), 1);
    assert_eq!(changelogs[0].0, "iris");
    assert!(changelogs[0].2.contains("Fixed bugs"));

    // Summary table was printed.
    let tables = progress.tables.lock().unwrap();
    assert_eq!(tables.len(), 1);
    // At least one row should contain ↑ (update arrow) or the slug.
    let has_updated_row = tables[0]
        .1
        .iter()
        .any(|row| row.iter().any(|cell| cell.contains('\u{2191}')));
    assert!(has_updated_row);

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 5. Dry-run mode — no files changed ────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn dry_run_mode_no_files_changed() {
    let content = b"dry-run-test-content";
    let sha1 = sha1_hex(content);
    let new_content = b"should-not-be-written";
    let new_sha1 = sha1_hex(new_content);

    let dir = setup_temp_mods_dir(&[("fabric-api-0.90.jar", content)]);
    let mut config = make_config(&dir);
    config.dry_run = true;
    // Confirm is still true, but dry-run short-circuits before the prompt.
    config.confirm = true;

    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();

    let current = make_version("va", "proj-fapi", "Fabric API", "0.90.0", vec![]);
    api.set_version(&sha1, Some(current));

    let new_file = make_file(
        "https://example.com/fapi-0.91.jar",
        "fabric-api-0.91.jar",
        &new_sha1,
    );
    let latest = make_version("vb", "proj-fapi", "Fabric API", "0.91.0", vec![new_file]);
    api.set_latest(&sha1, Some(latest));
    api.set_project("proj-fapi", make_project("proj-fapi", "fabric-api", "Fabric API"));
    api.set_download_content(new_content.to_vec(), None);

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    // Should report 1 update available but 0 applied.
    assert_eq!(summary.updates_available, 1);
    assert_eq!(summary.updates_applied, 0);

    // Original file still exists, new file was NOT created.
    assert!(dir.join("fabric-api-0.90.jar").exists());
    assert!(!dir.join("fabric-api-0.91.jar").exists());

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 6. User cancellation ──────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn user_cancellation_returns_error() {
    let content = b"cancel-test-content";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[("mod-to-cancel.jar", content)]);
    let mut config = make_config(&dir);
    config.confirm = true;

    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();
    progress.push_confirm(false); // user says no

    let current = make_version("v1", "proj-cancel", "CancelMod", "1.0.0", vec![]);
    api.set_version(&sha1, Some(current));

    let new_file = make_file(
        "https://example.com/cancel-2.0.jar",
        "cancel-2.0.jar",
        "abc",
    );
    let latest = make_version("v2", "proj-cancel", "CancelMod", "2.0.0", vec![new_file]);
    api.set_latest(&sha1, Some(latest));
    api.set_project("proj-cancel", make_project("proj-cancel", "cancelmod", "CancelMod"));

    let result = anvil::updater::run(&config, &api, &cache, &progress).await;

    match result {
        Err(Error::Cancelled) => {} // expected
        other => panic!("expected Error::Cancelled, got {:?}", other),
    }

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 7. SHA1 verification failure ──────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn sha1_verification_failure_reports_failed() {
    let content = b"sha1-fail-content";
    let sha1 = sha1_hex(content);
    let downloaded_content = b"actually-downloaded-bytes";

    let dir = setup_temp_mods_dir(&[("bad-mod.jar", content)]);
    let mut config = make_config(&dir);
    config.confirm = true;

    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();
    progress.push_confirm(true);

    let current = make_version("v1", "proj-bad", "BadMod", "1.0.0", vec![]);
    api.set_version(&sha1, Some(current));

    // The file's expected SHA1.
    let expected_sha1 = "expected-sha1-for-this-mod-file";
    let new_file = make_file(
        "https://example.com/bad-2.0.jar",
        "bad-mod-2.0.jar",
        expected_sha1,
    );
    let latest = make_version("v2", "proj-bad", "BadMod", "2.0.0", vec![new_file]);
    api.set_latest(&sha1, Some(latest));
    api.set_project("proj-bad", make_project("proj-bad", "badmod", "BadMod"));

    // Return a different SHA1 than what the file claims.
    api.set_download_content(
        downloaded_content.to_vec(),
        Some("wrong-sha1-here".into()),
    );

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.updates_available, 1);
    assert_eq!(summary.updates_applied, 0);
    assert_eq!(summary.failed, 1);

    // The partially-downloaded file should have been cleaned up.
    assert!(!dir.join("bad-mod-2.0.jar").exists());

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 8. Download failure / error ───────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn download_failure_reports_failed() {
    let content = b"dl-fail-content";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[("error-mod.jar", content)]);
    let mut config = make_config(&dir);
    config.confirm = true;

    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();
    progress.push_confirm(true);

    let current = make_version("v1", "proj-err", "ErrorMod", "1.0.0", vec![]);
    api.set_version(&sha1, Some(current));

    let new_file = make_file(
        "https://example.com/error-2.0.jar",
        "error-mod-2.0.jar",
        "abc",
    );
    let latest = make_version("v2", "proj-err", "ErrorMod", "2.0.0", vec![new_file]);
    api.set_latest(&sha1, Some(latest));
    api.set_project("proj-err", make_project("proj-err", "errormod", "ErrorMod"));
    api.set_download_should_fail(true);

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.updates_available, 1);
    assert_eq!(summary.updates_applied, 0);
    assert_eq!(summary.failed, 1);

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 9. Filters — include / exclude / loader ───────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn filters_include_exclude_loader() {
    let content_a = b"content-for-mod-a";
    let content_b = b"content-for-mod-b";
    let content_c = b"content-for-mod-c";
    let sha1_a = sha1_hex(content_a);
    let sha1_b = sha1_hex(content_b);
    let sha1_c = sha1_hex(content_c);

    let dir = setup_temp_mods_dir(&[
        ("sodium-0.5.jar", content_a),
        ("iris-1.7.jar", content_b),
        ("forgemod-1.0.jar", content_c),
    ]);
    let mut config = make_config(&dir);
    // Include only mods matching "sodium" or "iris".
    config.include = vec!["sodium".into(), "iris".into()];
    // Exclude iris.
    config.exclude = vec!["iris".into()];
    // Only fabric loader.
    config.loader = Some("fabric".into());

    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();

    // Sodium: fabric, matches include, doesn't match exclude.
    let mut sodium_ver = make_version("vs", "proj-sodium", "Sodium", "0.5.11", vec![]);
    sodium_ver.loaders = vec!["fabric".into()];
    api.set_version(&sha1_a, Some(sodium_ver.clone()));
    api.set_latest(&sha1_a, Some(sodium_ver)); // up to date

    // Iris: fabric, but matches exclude → filtered out.
    let mut iris_ver = make_version("vi", "proj-iris", "Iris", "1.7.0", vec![]);
    iris_ver.loaders = vec!["fabric".into()];
    api.set_version(&sha1_b, Some(iris_ver.clone()));
    api.set_latest(&sha1_b, Some(iris_ver));

    // ForgeMod: forge loader (doesn't match fabric filter → filtered out).
    let mut forge_ver = make_version("vf", "proj-forge", "ForgeMod", "1.0.0", vec![]);
    forge_ver.loaders = vec!["forge".into()];
    api.set_version(&sha1_c, Some(forge_ver.clone()));
    api.set_latest(&sha1_c, Some(forge_ver));

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.total_jars, 3);
    assert_eq!(summary.identified, 3);
    assert_eq!(summary.up_to_date, 1); // only Sodium passes all filters
    assert_eq!(summary.skipped, 2); // Iris (excluded) + ForgeMod (loader)

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 10. max_updates limit ─────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn max_updates_limit() {
    let content_a = b"max-updates-mod-a";
    let content_b = b"max-updates-mod-b";
    let sha1_a = sha1_hex(content_a);
    let sha1_b = sha1_hex(content_b);
    let dl_a = b"dl-mod-a-v2";
    let dl_b = b"dl-mod-b-v2";
    let dl_sha1_a = sha1_hex(dl_a);
    let dl_sha1_b = sha1_hex(dl_b);

    let dir = setup_temp_mods_dir(&[
        ("mod-a-1.0.jar", content_a),
        ("mod-b-1.0.jar", content_b),
    ]);
    let mut config = make_config(&dir);
    config.confirm = true;
    config.max_updates = Some(1); // only update 1

    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();
    progress.push_confirm(true);

    let current_a = make_version("va1", "proj-a", "ModA", "1.0.0", vec![]);
    api.set_version(&sha1_a, Some(current_a));
    let new_a = make_file(
        "https://example.com/mod-a-2.0.jar",
        "mod-a-2.0.jar",
        &dl_sha1_a,
    );
    let latest_a = make_version("va2", "proj-a", "ModA", "2.0.0", vec![new_a]);
    api.set_latest(&sha1_a, Some(latest_a));
    api.set_project("proj-a", make_project("proj-a", "mod-a", "ModA"));

    let current_b = make_version("vb1", "proj-b", "ModB", "1.0.0", vec![]);
    api.set_version(&sha1_b, Some(current_b));
    let new_b = make_file(
        "https://example.com/mod-b-2.0.jar",
        "mod-b-2.0.jar",
        &dl_sha1_b,
    );
    let latest_b = make_version("vb2", "proj-b", "ModB", "2.0.0", vec![new_b]);
    api.set_latest(&sha1_b, Some(latest_b));
    api.set_project("proj-b", make_project("proj-b", "mod-b", "ModB"));

    // Both downloads would succeed if allowed.
    api.set_download_content(dl_a.to_vec(), None); // but will be overwritten by second call

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.total_jars, 2);
    assert_eq!(summary.identified, 2);
    assert_eq!(summary.updates_available, 2);
    assert_eq!(summary.updates_applied, 1); // capped by max_updates
    assert_eq!(summary.failed, 0);

    // Only one download call was made.
    let dl_calls = api.download_calls.lock().unwrap();
    assert_eq!(dl_calls.len(), 1);

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 11. Unknown mods (SHA1 not on Modrinth) ───────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn unknown_mods_sha1_not_found() {
    let content = b"unknown-mod-content";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[("mystery.jar", content)]);
    let config = make_config(&dir);
    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();

    // SHA1 returns None → mod not on Modrinth.
    api.set_version(&sha1, None);

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.total_jars, 1);
    assert_eq!(summary.identified, 0);
    assert_eq!(summary.unknown, 1);
    assert_eq!(summary.updates_available, 0);

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 12. Unavailable outcome (game version specified, no match) ────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn unavailable_outcome_when_no_matching_game_version() {
    let content = b"unavailable-mod-content";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[("oldmod-1.0.jar", content)]);
    let mut config = make_config(&dir);
    // The mod supports 1.21 (so it passes the filter). We request 1.21
    // explicitly, and then the API returns None for get_latest_version,
    // which triggers the "Unavailable" outcome.
    config.game_version = Some("1.21".into());

    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();

    let current = make_version("v1", "proj-old", "OldMod", "1.0.0", vec![]);
    api.set_version(&sha1, Some(current));

    // Don't set any entry in latest_map for this SHA1, so
    // get_latest_version returns None. Together with game_version.is_some(),
    // this produces the Unavailable outcome.

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.total_jars, 1);
    assert_eq!(summary.identified, 1);
    assert_eq!(summary.updates_available, 0);
    assert_eq!(summary.unavailable, 1);

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 13. No candidates → no confirmation prompt ────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn no_candidates_no_confirm_prompt() {
    let content = b"no-candidate-content";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[("stable.jar", content)]);
    let mut config = make_config(&dir);
    config.confirm = true; // would prompt if there were candidates

    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();
    // Don't push any confirm answer — it should never be popped.

    let version = make_version("v1", "proj-stable", "StableMod", "1.0.0", vec![]);
    api.set_version(&sha1, Some(version.clone()));
    api.set_latest(&sha1, Some(version)); // same version → no candidate

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.updates_available, 0);
    assert_eq!(summary.updates_applied, 0);
    assert_eq!(summary.up_to_date, 1);

    // confirm_questions should be empty — prompt was never shown.
    let questions = progress.confirm_questions.lock().unwrap();
    assert!(questions.is_empty());

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 14. Progress tracking — phases are recorded ───────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn progress_phases_are_recorded() {
    let content = b"progress-test-content";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[("progress-mod.jar", content)]);
    let config = make_config(&dir);
    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();

    let version = make_version("v1", "proj-prog", "ProgressMod", "1.0.0", vec![]);
    api.set_version(&sha1, Some(version.clone()));
    api.set_latest(&sha1, Some(version));

    let _summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    let phases = progress.phases.lock().unwrap();
    // Should have at least "Hashing" and "Identifying" phases.
    let phase_names: Vec<&str> = phases.iter().map(|(n, _)| n.as_str()).collect();
    assert!(phase_names.contains(&"Hashing"));
    assert!(phase_names.contains(&"Identifying"));

    // Phases should have been finished.
    let finish_count = *progress.finish_count.lock().unwrap();
    assert!(finish_count >= 2);

    // Increments should have been called.
    let increments = *progress.increments.lock().unwrap();
    assert!(increments > 0);

    // Table should have been printed.
    let tables = progress.tables.lock().unwrap();
    assert_eq!(tables.len(), 1);

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 15. Mixed outcomes in one run ─────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn mixed_outcomes_unknown_and_up_to_date() {
    let content_known = b"mixed-known-content";
    let content_unknown = b"mixed-unknown-content";
    let sha1_known = sha1_hex(content_known);
    let sha1_unknown = sha1_hex(content_unknown);

    let dir = setup_temp_mods_dir(&[
        ("known.jar", content_known),
        ("unknown.jar", content_unknown),
    ]);
    let config = make_config(&dir);
    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();

    let version = make_version("v1", "proj-known", "KnownMod", "1.0.0", vec![]);
    api.set_version(&sha1_known, Some(version.clone()));
    api.set_latest(&sha1_known, Some(version));
    api.set_version(&sha1_unknown, None); // unknown

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.total_jars, 2);
    assert_eq!(summary.identified, 1);
    assert_eq!(summary.unknown, 1);
    assert_eq!(summary.up_to_date, 1);

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 16. Backup creates backup directory and restores on failure ───────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn backup_restores_on_sha1_mismatch() {
    let content = b"backup-test-content";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[("keep-me.jar", content)]);
    let mut config = make_config(&dir);
    config.backup = true; // enable backup
    config.confirm = true;

    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();
    progress.push_confirm(true);

    let current = make_version("v1", "proj-keep", "KeepMe", "1.0.0", vec![]);
    api.set_version(&sha1, Some(current));

    let new_file = make_file(
        "https://example.com/keep-me-2.0.jar",
        "keep-me-2.0.jar",
        "expected-sha1",
    );
    let latest = make_version("v2", "proj-keep", "KeepMe", "2.0.0", vec![new_file]);
    api.set_latest(&sha1, Some(latest));
    api.set_project("proj-keep", make_project("proj-keep", "keep-me", "KeepMe"));

    // Return wrong SHA1 → fall into SHA1 mismatch path, which restores from backup.
    api.set_download_content(b"corrupt".to_vec(), Some("bad-sha1".into()));

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.failed, 1);
    // Old file should still exist (restored from backup).
    assert!(dir.join("keep-me.jar").exists());
    // The corrupt downloaded file should be gone.
    assert!(!dir.join("keep-me-2.0.jar").exists());

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 17. Multiple JARs with a read-error skip ──────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn unreadable_jar_is_skipped() {
    let content = b"good-jar-content";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[("good.jar", content)]);

    let content2 = b"good-jar-content-2";
    let sha1_2 = sha1_hex(content2);
    fs::write(dir.join("good2.jar"), content2).unwrap();

    let config = make_config(&dir);
    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();

    let v1 = make_version("va", "pa", "ModA", "1.0.0", vec![]);
    api.set_version(&sha1, Some(v1.clone()));
    api.set_latest(&sha1, Some(v1));
    let v2 = make_version("vb", "pb", "ModB", "1.0.0", vec![]);
    api.set_version(&sha1_2, Some(v2.clone()));
    api.set_latest(&sha1_2, Some(v2));

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.total_jars, 2);
    assert_eq!(summary.identified, 2);

    let _ = fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════════
// ── 18. Non-JAR files are not scanned ─────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn non_jar_files_are_ignored() {
    let content = b"only-jar-matters";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[("real.jar", content)]);
    // Create non-JAR files.
    fs::write(dir.join("readme.txt"), b"ignore me").unwrap();
    fs::write(dir.join("notes.md"), b"ignore me too").unwrap();
    fs::create_dir(dir.join("subdir")).unwrap();

    let config = make_config(&dir);
    let api = MockApi::new();
    let cache = ApiCache::new();
    let progress = MockProgress::new();

    let version = make_version("v1", "proj-real", "RealMod", "1.0.0", vec![]);
    api.set_version(&sha1, Some(version.clone()));
    api.set_latest(&sha1, Some(version));

    let summary = anvil::updater::run(&config, &api, &cache, &progress)
        .await
        .unwrap();

    assert_eq!(summary.total_jars, 1, "only .jar files should be counted");
    assert_eq!(summary.identified, 1);

    let _ = fs::remove_dir_all(&dir);
}
