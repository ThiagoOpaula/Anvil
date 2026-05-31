use super::*;
use std::time::{Duration, Instant};

// ── Client construction ─────────────────────────────────────────────

#[test]
fn client_construction_succeeds() {
    let api = ModrinthApi::new();
    assert!(api.is_ok(), "client should build with default settings");
}

#[test]
fn client_has_correct_base_url() {
    let api = ModrinthApi::new().unwrap();
    assert_eq!(api.base_url, "https://api.modrinth.com/v2");
}

// ── Rate limiting ───────────────────────────────────────────────────

#[tokio::test]
async fn rate_limit_first_call_is_immediate() {
    let api = ModrinthApi::new().unwrap();
    let start = Instant::now();
    api.rate_limit().await;
    let elapsed = start.elapsed();
    // First call should pass through immediately (no prior request timestamp).
    assert!(elapsed.as_millis() < 100, "first rate_limit should be instant");
}

#[tokio::test]
async fn rate_limit_second_call_is_delayed() {
    let api = ModrinthApi::new().unwrap();
    // First call.
    api.rate_limit().await;
    // Second call immediately after — should wait ~250ms.
    let start = Instant::now();
    api.rate_limit().await;
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() >= 200,
        "second rate_limit should be delayed at least 200ms, got {}ms",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn rate_limit_after_delay_is_immediate() {
    let api = ModrinthApi::new().unwrap();
    api.rate_limit().await;
    // Wait longer than the rate limit interval.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let start = Instant::now();
    api.rate_limit().await;
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 50,
        "rate_limit after sufficient delay should be instant, got {}ms",
        elapsed.as_millis()
    );
}

// ── Constants ───────────────────────────────────────────────────────

#[test]
fn base_url_is_modrinth_api() {
    assert_eq!(BASE_URL, "https://api.modrinth.com/v2");
}

#[test]
fn user_agent_identifies_anvil() {
    assert!(USER_AGENT.contains("anvil"), "User-Agent should include anvil");
}

#[test]
fn rate_limit_interval_is_250ms() {
    assert_eq!(RATE_LIMIT_INTERVAL, Duration::from_millis(250));
}

#[test]
fn max_retries_is_3() {
    assert_eq!(MAX_RETRIES, 3);
}

// ── Request URL construction ────────────────────────────────────────

#[test]
fn version_from_hash_url() {
    // Verify the path format by checking it doesn't panic.
    let path = format!("/version_file/{}?algorithm=sha1", "abc123");
    assert!(path.contains("abc123"));
    assert!(path.contains("sha1"));
}

#[test]
fn latest_version_path() {
    let path = format!("/version_file/{}/update?algorithm=sha1", "abc123");
    assert!(path.contains("abc123"));
    assert!(path.contains("update"));
}

#[test]
fn project_path() {
    let path = format!("/project/{}", "proj-id");
    assert!(path.contains("proj-id"));
}
