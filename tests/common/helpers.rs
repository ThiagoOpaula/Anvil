//! Factory functions and temporary-directory helpers for integration tests.
//!
//! Every function builds a struct with sensible test defaults so individual
//! tests only need to override the fields they care about.

use anvil::types::{FileHashes, IdentifiedMod, LockFile, LockedMod, ModFile, ModVersion, Project, ProjectStatus};
use anvil::config::{LogLevel, ResolvedConfig};
use sha1::{Digest, Sha1};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

// ── Factory functions ───────────────────────────────────────────────────────

/// Build a `ModVersion` with sensible test defaults.
///
/// Defaults: fabric loader, Minecraft 1.21, no changelog, no files, no deps.
/// The `project_id` is derived as `"project-{id}"`.
pub fn make_test_version(id: &str, name: &str, version_number: &str) -> ModVersion {
    ModVersion {
        id: id.to_string(),
        project_id: format!("project-{}", id),
        name: name.to_string(),
        version_number: version_number.to_string(),
        changelog: None,
        loaders: vec!["fabric".to_string()],
        game_versions: vec!["1.21".to_string()],
        files: Vec::new(),
        dependencies: Vec::new(),
    }
}

/// Build a `ModVersion` with explicit project_id and files (5-arg form used by
/// updater integration tests).
pub fn make_version(
    id: &str,
    project_id: &str,
    name: &str,
    version_number: &str,
    files: Vec<ModFile>,
) -> ModVersion {
    ModVersion {
        id: id.to_string(),
        project_id: project_id.to_string(),
        name: name.to_string(),
        version_number: version_number.to_string(),
        changelog: None,
        loaders: vec!["fabric".to_string()],
        game_versions: vec!["1.21".to_string()],
        files,
        dependencies: Vec::new(),
    }
}

/// Build an `Approved` `Project` with the given id, slug, and title.
pub fn make_test_project(id: &str, slug: &str, title: &str) -> Project {
    Project {
        id: id.to_string(),
        slug: slug.to_string(),
        title: title.to_string(),
        status: ProjectStatus::Approved,
    }
}

/// Build a primary `ModFile` with the given url, filename, and sha1 hash.
///
/// The `sha512` field is set to 128 zero characters (a clearly invalid hash
/// that tests can recognise as a placeholder).
pub fn make_test_file(url: &str, filename: &str, sha1: &str) -> ModFile {
    ModFile {
        url: url.to_string(),
        filename: filename.to_string(),
        primary: true,
        size: 0,
        hashes: FileHashes {
            sha1: sha1.to_string(),
            sha512: "0".repeat(128),
        },
    }
}

/// Alias for `make_test_file` (updater tests use this name).
pub fn make_file(url: &str, filename: &str, sha1: &str) -> ModFile {
    make_test_file(url, filename, sha1)
}

/// Alias for `make_test_project` (updater tests use this name).
pub fn make_project(id: &str, slug: &str, title: &str) -> Project {
    make_test_project(id, slug, title)
}

/// Build a default `ResolvedConfig` pointing at the given `mods_dir`.
///
/// Defaults: backup enabled, no forced loader or game version, empty
/// include/exclude filters, no max-updates cap, Info log level, dry-run
/// disabled, confirm enabled, changelog disabled.
pub fn make_test_config(mods_dir: &PathBuf) -> ResolvedConfig {
    ResolvedConfig {
        mods_dir: mods_dir.clone(),
        backup: true,
        loader: None,
        game_version: None,
        include: Vec::new(),
        exclude: Vec::new(),
        max_updates: None,
        log_level: LogLevel::Info,
        dry_run: false,
        confirm: true,
        changelog: false,
        dark_mode: false,
    }
}

/// Build a `ResolvedConfig` with updater-test defaults (backup off, confirm off).
///
/// This is the function used by updater integration tests. It differs from
/// `make_test_config`: backup is `false` and confirm is `false` so that tests
/// without update-candidates don't accidentally trigger backup or confirm pathways.
pub fn make_config(mods_dir: &Path) -> ResolvedConfig {
    ResolvedConfig {
        mods_dir: mods_dir.to_path_buf(),
        backup: false,
        loader: None,
        game_version: None,
        include: Vec::new(),
        exclude: Vec::new(),
        max_updates: None,
        log_level: LogLevel::Info,
        dry_run: false,
        confirm: false,
        changelog: false,
        dark_mode: false,
    }
}

/// Build an `IdentifiedMod` for testing lockfile operations.
pub fn make_identified_mod(filename: &str, project_id: &str, name: &str, version: &str) -> IdentifiedMod {
    IdentifiedMod {
        path: PathBuf::from(format!("/mods/{}", filename)),
        sha1: "abc123".into(),
        filename: filename.into(),
        current_version: ModVersion {
            id: format!("ver-{}", name),
            project_id: project_id.into(),
            name: name.into(),
            version_number: version.into(),
            changelog: None,
            loaders: vec!["fabric".into()],
            game_versions: vec!["1.21".into()],
            files: vec![],
            dependencies: vec![],
        },
    }
}

/// Build a `LockedMod` for testing diff/roundtrip operations.
pub fn make_locked_mod(slug: &str, version: &str) -> LockedMod {
    LockedMod {
        filename: format!("{}-{}.jar", slug, version),
        sha1: format!("sha-{}", slug),
        project_id: format!("proj-{}", slug),
        slug: slug.into(),
        version_id: format!("ver-{}-{}", slug, version),
        version_number: version.into(),
        loaders: vec!["fabric".into()],
        game_versions: vec!["1.21".into()],
    }
}

/// Build a `LockFile` for testing diff/roundtrip operations.
pub fn make_lockfile(mods: Vec<LockedMod>) -> LockFile {
    LockFile {
        version: 1,
        updated_at: "2024-01-01T00:00:00Z".into(),
        target_game_version: None,
        target_loader: None,
        mods,
    }
}

// ── Temporary directory helpers ─────────────────────────────────────────────

/// Generate a unique temporary directory path (does **not** create the
/// directory).
///
/// Uses an atomic counter plus the current process ID to guarantee
/// uniqueness across concurrent test runs.  The pattern is:
/// `{temp}/anvil-{label}-{pid}-{counter}`.
pub fn unique_temp_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let pid = std::process::id();
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("anvil-{}-{}-{}", label, pid, counter))
}

/// Create a temporary directory populated with the given `(filename, content)`
/// pairs and return its path.
///
/// Useful for setting up a fake mods folder for scanner / updater tests.
pub fn setup_temp_mods_dir(jar_contents: &[(&str, &[u8])]) -> PathBuf {
    let dir = unique_temp_dir("mods");
    fs::create_dir_all(&dir).unwrap();
    for (filename, content) in jar_contents {
        fs::write(dir.join(filename), content).unwrap();
    }
    dir
}

// ── Hash helpers ────────────────────────────────────────────────────────────

/// Compute the SHA1 hex digest of `data`.
pub fn sha1_hex(data: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}
