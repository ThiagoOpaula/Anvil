//! Shared test utilities for crate-internal unit tests.
//!
//! This module is gated behind `#[cfg(test)]` so it only compiles during
//! test runs. Integration tests (in `tests/`) cannot see items from this
//! module; they use `tests/common/mod.rs` instead.

#![cfg(test)]

use crate::types::ModVersion;
use std::path::PathBuf;

/// Build a `ModVersion` with sensible defaults for testing.
pub fn make_test_version(id: &str, name: &str, version_number: &str) -> ModVersion {
    ModVersion {
        id: id.into(),
        project_id: format!("proj-{}", id),
        name: name.into(),
        version_number: version_number.into(),
        changelog: None,
        loaders: vec!["fabric".into()],
        game_versions: vec!["1.21".into()],
        files: vec![],
        dependencies: vec![],
    }
}

/// Generate a unique temp directory path. Caller is responsible for cleanup.
pub fn unique_temp_dir(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("anvil-{}-{}-{}", label, std::process::id(), n))
}
