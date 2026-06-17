//! Shared types, enums, and traits — the contract between all modules.
//!
//! Every other module imports from here. Changing a type signature here
//! affects the entire crate, so keep this file stable.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── API Response Types (Modrinth JSON shapes) ──────────────────────────

/// A version of a mod on Modrinth.
/// Returned by `GET /v2/version_file/{hash}` and `POST /v2/version_file/{hash}/update`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub changelog: Option<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub files: Vec<ModFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

/// A file attached to a ModVersion (usually a JAR).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
    pub hashes: FileHashes,
}

/// SHA1/SHA512 hashes of a file on Modrinth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHashes {
    pub sha1: String,
    pub sha512: String,
}

/// A dependency relationship declared by a mod version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    pub dependency_type: DependencyType,
}

/// The kind of dependency between mods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

/// Project metadata from `GET /v2/project/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub status: ProjectStatus,
}

/// Moderation / lifecycle status of a Modrinth project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Approved,
    Archived,
    Rejected,
    Draft,
    Unlisted,
    Processing,
    Withheld,
    Scheduled,
    Private,
    /// Catch-all for newly added statuses in the API.
    #[serde(other)]
    Unknown,
}

impl ProjectStatus {
    /// Whether this status means the mod should generally be avoided.
    pub fn is_problematic(self) -> bool {
        matches!(self, Self::Archived | Self::Rejected | Self::Withheld)
    }
}

// ── Internal domain types ──────────────────────────────────────────────

/// A mod JAR that was successfully identified on Modrinth.
#[derive(Debug, Clone)]
pub struct IdentifiedMod {
    /// Absolute path to the JAR on disk.
    pub path: PathBuf,
    /// SHA1 hex digest of the JAR.
    pub sha1: String,
    /// Original filename (e.g. `sodium-fabric-0.5.11.jar`).
    pub filename: String,
    /// The current version metadata from Modrinth.
    pub current_version: ModVersion,
}

/// A mod that has an update available.
#[derive(Debug, Clone)]
pub struct UpdateCandidate {
    pub identified: IdentifiedMod,
    pub latest_version: ModVersion,
    pub project: Option<Project>,
}

/// Tracks the final outcome for each mod in the run.
#[derive(Debug, Clone)]
pub enum ModOutcome {
    Updated {
        slug: String,
        old_filename: String,
        new_filename: String,
        old_version: String,
        new_version: String,
    },
    UpToDate {
        slug: String,
        filename: String,
        version: String,
    },
    Unavailable {
        slug: String,
        filename: String,
        current_version: String,
        game_version: String,
    },
    Unknown {
        filename: String,
    },
    FilteredOut {
        filename: String,
        reason: String,
    },
    Failed {
        filename: String,
        error: String,
    },
}

/// Summary emitted at the end of a run.
#[derive(Debug, Default, Clone)]
pub struct RunSummary {
    pub total_jars: usize,
    pub identified: usize,
    pub unknown: usize,
    pub updates_available: usize,
    pub updates_applied: usize,
    pub up_to_date: usize,
    pub unavailable: usize,
    pub skipped: usize,
    pub failed: usize,
}

// ── Lockfile types ─────────────────────────────────────────────────────

/// Serializable mod state file stored at `<mods_dir>/mod-updater.lock`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    pub version: u32,
    pub updated_at: String, // ISO 8601
    pub target_game_version: Option<String>,
    pub target_loader: Option<String>,
    pub mods: Vec<LockedMod>,
}

/// A single entry in the lockfile representing an installed mod.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockedMod {
    pub filename: String,
    pub sha1: String,
    pub project_id: String,
    pub slug: String,
    pub version_id: String,
    pub version_number: String,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
}

// ── Config types ───────────────────────────────────────────────────────

/// User-configurable options from config file and CLI.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub mods_dir: Option<PathBuf>,

    #[serde(default)]
    pub backup: Option<bool>,

    #[serde(default)]
    pub loader: Option<String>,

    #[serde(default)]
    pub game_version: Option<String>,

    #[serde(default)]
    pub include: Option<Vec<String>>,

    #[serde(default)]
    pub exclude: Option<Vec<String>>,

    #[serde(default)]
    pub max_updates: Option<usize>,

    #[serde(default)]
    pub verbose: Option<bool>,

    #[serde(default)]
    pub quiet: Option<bool>,

    #[serde(default)]
    pub dry_run: Option<bool>,

    #[serde(default)]
    pub confirm: Option<bool>,

    #[serde(default)]
    pub changelog: Option<bool>,
}

// ── Filter options (derived from CLI + Config) ─────────────────────────

/// Resolved filtering options used by `filters::apply`.
#[derive(Debug, Clone, Default)]
pub struct FilterOpts {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub loader: Option<String>,
    pub game_version: Option<String>,
}

/// A game version tag returned by `GET /v2/tag/game_version`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameVersionTag {
    pub version: String,
    pub version_type: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub major: bool,
}

/// A loader tag returned by `GET /v2/tag/loader`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderTag {
    pub name: String,
    #[serde(default)]
    pub supported_project_types: Vec<String>,
}

// ── Traits (module decoupling) ─────────────────────────────────────────

/// Abstracts the Modrinth API surface needed by the updater.
///
/// Implementations: `ModrinthApi` (real HTTP), mock for testing.
#[async_trait::async_trait]
pub trait ApiClient: Send + Sync {
    /// Look up a version by SHA1 hash. Returns `None` on 404 (mod not on Modrinth).
    async fn get_version_from_hash(&self, sha1: &str) -> crate::Result<Option<ModVersion>>;

    /// Get the latest version matching the given loaders and game versions.
    async fn get_latest_version(
        &self,
        sha1: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> crate::Result<Option<ModVersion>>;

    /// Fetch project metadata (slug, title, status).
    async fn get_project(&self, project_id: &str) -> crate::Result<Project>;

    /// Download a file to `dest`, returning its **actual** SHA1 hex digest for verification.
    ///
    /// The `progress` callback receives `(bytes_downloaded, total_bytes)`.
    async fn download_file(
        &self,
        url: &str,
        dest: &Path,
        progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
    ) -> crate::Result<String>;

    /// Fetch all game versions known to Modrinth (release versions, newest first).
    async fn get_game_versions(&self) -> crate::Result<Vec<String>>;

    /// Fetch all mod loaders known to Modrinth (filtered to "mod" project type).
    async fn get_loaders(&self) -> crate::Result<Vec<String>>;
}

/// Abstracts progress reporting during the update run.
///
/// Implementations: `ConsoleProgress` (indicatif bars), no-op for tests.
pub trait ProgressRenderer: Send + Sync {
    /// Begin a named phase with a known item count.
    fn start_phase(&self, label: &str, total: u64);

    /// Advance progress by `n` items.
    fn increment(&self, n: u64);

    /// Finish and clear the current phase.
    fn finish_phase(&self);

    /// Print a formatted table to stdout.
    fn print_table(&self, headers: &[&str], rows: &[Vec<String>]);

    /// Display a changelog block for a mod.
    fn print_changelog(&self, slug: &str, version: &str, changelog: &str);

    /// Prompt the user for a yes/no answer. Returns `true` on yes.
    fn confirm(&self, question: &str) -> bool;
}
