//! File-based JSON cache — deterministic, no TTL.
//!
//! All entries are keyed by hash or composite parameters so a given key always
//! resolves to the same file.  Stale cache entries are silently treated as
//! misses (logged at WARN) so the updater simply re-fetches.

use crate::error::{Error, Result};
use crate::types::{ModVersion, Project};
use std::path::PathBuf;

/// Persistent cache rooted at the directory returned by `paths::cache_dir()`.
pub struct ApiCache {
    root: PathBuf,
}

impl ApiCache {
    /// Create a new cache using the platform-appropriate cache directory.
    pub fn new() -> Self {
        Self {
            root: crate::paths::cache_dir(),
        }
    }

    // ── path helpers ──────────────────────────────────────────────────────

    /// Directory for version files: `<root>/versions/{first_2_chars}/`
    fn versions_dir(&self, sha1: &str) -> PathBuf {
        let prefix = &sha1[..2.min(sha1.len())];
        self.root.join("versions").join(prefix)
    }

    /// Full path for a version JSON: `<root>/versions/{p2}/{sha1}.json`
    fn version_path(&self, sha1: &str) -> PathBuf {
        self.versions_dir(sha1).join(format!("{}.json", sha1))
    }

    /// Build a filesystem-safe composite key for update lookups.
    ///
    /// Format: `{sha1}_{loaders_sorted_joined}_{game_versions_sorted_joined}`
    ///
    /// Modrinth loaders and game versions are alphanumeric plus dot/dash so
    /// joining with `-` and separating groups with `_` is safe on all platforms.
    fn update_key(&self, sha1: &str, loaders: &[String], game_versions: &[String]) -> String {
        let mut sorted_loaders = loaders.to_vec();
        sorted_loaders.sort();
        let loaders_str = sorted_loaders.join("-");

        let mut sorted_gv = game_versions.to_vec();
        sorted_gv.sort();
        let gv_str = sorted_gv.join("-");

        format!("{}_{}_{}", sha1, loaders_str, gv_str)
    }

    /// Directory for update cache files: `<root>/updates/`
    fn updates_dir(&self) -> PathBuf {
        self.root.join("updates")
    }

    /// Full path for an update JSON: `<root>/updates/{composite_key}.json`
    fn update_path(
        &self,
        sha1: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> PathBuf {
        let key = self.update_key(sha1, loaders, game_versions);
        self.updates_dir().join(format!("{}.json", key))
    }

    /// Directory for project cache files: `<root>/projects/`
    fn projects_dir(&self) -> PathBuf {
        self.root.join("projects")
    }

    /// Full path for a project JSON: `<root>/projects/{project_id}.json`
    fn project_path(&self, project_id: &str) -> PathBuf {
        self.projects_dir().join(format!("{}.json", project_id))
    }

    // ── version cache ─────────────────────────────────────────────────────

    /// Look up a `ModVersion` by SHA1 hex hash.
    ///
    /// Returns `Ok(None)` when the cache file does not exist **or** when the
    /// file exists but cannot be parsed (stale / corrupt cache).
    pub fn get_version(&self, sha1: &str) -> Result<Option<ModVersion>> {
        let path = self.version_path(sha1);
        self.read_json(&path)
    }

    /// Persist a `ModVersion` keyed by its SHA1 hash.
    pub fn set_version(&self, sha1: &str, version: &ModVersion) -> Result<()> {
        let path = self.version_path(sha1);
        self.write_json(&path, version)
    }

    // ── update cache ──────────────────────────────────────────────────────

    /// Look up a cached "latest version" result.
    pub fn get_update(
        &self,
        sha1: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<Option<ModVersion>> {
        let path = self.update_path(sha1, loaders, game_versions);
        self.read_json(&path)
    }

    /// Persist a "latest version" result keyed by the composite of SHA1,
    /// sorted loaders, and sorted game versions.
    pub fn set_update(
        &self,
        sha1: &str,
        loaders: &[String],
        game_versions: &[String],
        version: &ModVersion,
    ) -> Result<()> {
        let path = self.update_path(sha1, loaders, game_versions);
        self.write_json(&path, version)
    }

    // ── project cache ─────────────────────────────────────────────────────

    /// Look up a `Project` by Modrinth project ID.
    pub fn get_project(&self, project_id: &str) -> Result<Option<Project>> {
        let path = self.project_path(project_id);
        self.read_json(&path)
    }

    /// Persist a `Project` keyed by its Modrinth project ID.
    pub fn set_project(&self, project_id: &str, project: &Project) -> Result<()> {
        let path = self.project_path(project_id);
        self.write_json(&path, project)
    }

    // ── internal helpers ─────────────────────────────────────────────────

    /// Read and deserialize from a cache file.
    ///
    /// * File missing → `Ok(None)`
    /// * Parse failure → `tracing::warn!` + `Ok(None)` (stale cache is not fatal)
    fn read_json<T: serde::de::DeserializeOwned>(&self, path: &PathBuf) -> Result<Option<T>> {
        match std::fs::read_to_string(path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(value) => Ok(Some(value)),
                Err(e) => {
                    tracing::warn!(
                        "corrupt cache entry {} (parse error: {}), will re-fetch",
                        path.display(),
                        e
                    );
                    Ok(None)
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => {
                tracing::warn!(
                    "failed to read cache entry {}: {}",
                    path.display(),
                    e
                );
                Err(Error::Io(e))
            }
        }
    }

    /// Serialize and write a value to a cache file, creating parent
    /// directories as needed.
    fn write_json<T: serde::Serialize>(&self, path: &PathBuf, value: &T) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        let file = std::fs::File::create(path).map_err(Error::Io)?;
        serde_json::to_writer_pretty(file, value)
            .map_err(|e| Error::Other(format!("failed to serialize cache entry: {}", e)))
    }
}

#[cfg(test)]
#[path = "tests/test_cache.rs"]
mod tests;
