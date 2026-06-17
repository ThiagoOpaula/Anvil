//! Mock implementations of `ApiClient` and `ProgressRenderer` for integration tests.
//!
//! `MockApi` serves canned responses from in-memory maps and tracks every call
//! so tests can assert exactly which API interactions occurred.
//!
//! `MockProgress` records phase transitions, table/changelog output, and confirm
//! prompts so tests can verify the correct UI behaviour without a real terminal.

use anvil::error::{Error, Result};
use anvil::types::{ApiClient, ModVersion, ProgressRenderer, Project};
use async_trait::async_trait;
use sha1::{Digest, Sha1};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// ── MockApi ─────────────────────────────────────────────────────────────────

/// A mock API client that returns canned responses for testing.
///
/// Every map is behind a `Mutex` so `&self` methods can mutate state.
/// Call-tracking vectors let tests assert which API calls were made and
/// with what arguments.
pub struct MockApi {
    /// Maps SHA1 hex string to an optional `ModVersion` (None = 404 / not on Modrinth).
    pub version_map: Mutex<HashMap<String, Option<ModVersion>>>,
    /// Maps SHA1 hex string to the latest `ModVersion` (None = no update available).
    pub latest_map: Mutex<HashMap<String, Option<ModVersion>>>,
    /// Maps project ID to `Project` metadata.
    pub project_map: Mutex<HashMap<String, Project>>,
    /// Raw bytes returned by `download_file`.
    pub download_content: Mutex<Vec<u8>>,
    /// When set, `download_file` returns this SHA1 instead of computing it from content.
    pub download_sha1_override: Mutex<Option<String>>,
    /// When true, `download_file` returns `Err(Error::Download { ... })`.
    pub download_should_fail: Mutex<bool>,
    /// Game versions returned by `get_game_versions`.
    pub game_versions: Mutex<Vec<String>>,
    /// Loaders returned by `get_loaders`.
    pub loaders: Mutex<Vec<String>>,

    // Call tracking
    /// Every SHA1 passed to `get_version_from_hash`.
    pub version_calls: Mutex<Vec<String>>,
    /// Every `(sha1, loaders, game_versions)` tuple passed to `get_latest_version`.
    pub latest_calls: Mutex<Vec<(String, Vec<String>, Vec<String>)>>,
    /// Every project ID passed to `get_project`.
    pub project_calls: Mutex<Vec<String>>,
    /// Every `(url, dest)` pair passed to `download_file`.
    pub download_calls: Mutex<Vec<(String, PathBuf)>>,
}

impl MockApi {
    /// Create a `MockApi` with empty maps and zeroed call tracking.
    pub fn new() -> Self {
        Self {
            version_map: Mutex::new(HashMap::new()),
            latest_map: Mutex::new(HashMap::new()),
            project_map: Mutex::new(HashMap::new()),
            download_content: Mutex::new(Vec::new()),
            download_sha1_override: Mutex::new(None),
            download_should_fail: Mutex::new(false),
            game_versions: Mutex::new(vec![
                "1.21.5".to_string(),
                "1.21.4".to_string(),
                "1.21.1".to_string(),
            ]),
            loaders: Mutex::new(vec!["fabric".to_string(), "forge".to_string()]),
            version_calls: Mutex::new(Vec::new()),
            latest_calls: Mutex::new(Vec::new()),
            project_calls: Mutex::new(Vec::new()),
            download_calls: Mutex::new(Vec::new()),
        }
    }

    /// Register a version to be returned for a given SHA1 hash.
    /// Pass `None` to simulate a hash that is not on Modrinth (404).
    pub fn set_version(&self, sha1: &str, version: Option<ModVersion>) {
        self.version_map
            .lock()
            .unwrap()
            .insert(sha1.to_string(), version);
    }

    /// Register the latest version available for a given SHA1 hash.
    /// Pass `None` to simulate "no update available".
    pub fn set_latest(&self, sha1: &str, version: Option<ModVersion>) {
        self.latest_map
            .lock()
            .unwrap()
            .insert(sha1.to_string(), version);
    }

    /// Register a project to be returned for a given project ID.
    pub fn set_project(&self, project_id: &str, project: Project) {
        self.project_map
            .lock()
            .unwrap()
            .insert(project_id.to_string(), project);
    }

    /// Set the raw bytes that `download_file` will write to the destination.
    /// Optionally override the SHA1 that the method returns (otherwise the
    /// real SHA1 of `content` is computed).
    pub fn set_download_content(&self, content: Vec<u8>, sha1_override: Option<String>) {
        *self.download_content.lock().unwrap() = content;
        *self.download_sha1_override.lock().unwrap() = sha1_override;
    }

    /// When set to `true`, `download_file` returns `Err(Error::Download { ... })`.
    pub fn set_download_should_fail(&self, fail: bool) {
        *self.download_should_fail.lock().unwrap() = fail;
    }
}

// ── ApiClient impl ──────────────────────────────────────────────────────────

#[async_trait]
impl ApiClient for MockApi {
    async fn get_version_from_hash(&self, sha1: &str) -> Result<Option<ModVersion>> {
        self.version_calls.lock().unwrap().push(sha1.to_string());
        Ok(self
            .version_map
            .lock()
            .unwrap()
            .get(sha1)
            .cloned()
            .unwrap_or(None))
    }

    async fn get_latest_version(
        &self,
        sha1: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<Option<ModVersion>> {
        self.latest_calls.lock().unwrap().push((
            sha1.to_string(),
            loaders.to_vec(),
            game_versions.to_vec(),
        ));
        Ok(self
            .latest_map
            .lock()
            .unwrap()
            .get(sha1)
            .cloned()
            .unwrap_or(None))
    }

    async fn get_project(&self, project_id: &str) -> Result<Project> {
        self.project_calls
            .lock()
            .unwrap()
            .push(project_id.to_string());
        self.project_map
            .lock()
            .unwrap()
            .get(project_id)
            .cloned()
            .ok_or_else(|| Error::Other(format!("mock: project not found: {}", project_id)))
    }

    async fn download_file(
        &self,
        url: &str,
        dest: &Path,
        progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
    ) -> Result<String> {
        self.download_calls
            .lock()
            .unwrap()
            .push((url.to_string(), dest.to_path_buf()));

        if *self.download_should_fail.lock().unwrap() {
            return Err(Error::Download {
                url: url.to_string(),
                reason: "mock download failure".to_string(),
            });
        }

        let content = self.download_content.lock().unwrap();

        // Ensure the destination directory exists.
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        let total = content.len() as u64;
        progress(0, Some(total));
        fs::write(dest, &*content)?;
        progress(total, Some(total));

        // Return the override SHA1 if one was set, otherwise compute the real
        // SHA1 of the canned content.
        if let Some(override_sha1) = self.download_sha1_override.lock().unwrap().as_ref() {
            Ok(override_sha1.clone())
        } else {
            let mut hasher = Sha1::new();
            hasher.update(&*content);
            Ok(hasher
                .finalize()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect())
        }
    }

    async fn get_game_versions(&self) -> anvil::error::Result<Vec<String>> {
        Ok(self.game_versions.lock().expect("lock").clone())
    }

    async fn get_loaders(&self) -> anvil::error::Result<Vec<String>> {
        Ok(self.loaders.lock().expect("lock").clone())
    }
}

// ── MockProgress ────────────────────────────────────────────────────────────

/// A mock progress renderer that records every call so tests can assert
/// exactly what the UI pipeline emitted.
pub struct MockProgress {
    /// Each `(label, total)` recorded by `start_phase`.
    pub phases: Mutex<Vec<(String, u64)>>,
    /// Running sum of all `increment(n)` calls.
    pub increments: Mutex<u64>,
    /// How many times `finish_phase` was called.
    pub finish_count: Mutex<u32>,
    /// Each `(headers, rows)` tuple recorded by `print_table`.
    pub tables: Mutex<Vec<(Vec<String>, Vec<Vec<String>>)>>,
    /// Each `(slug, version, changelog)` recorded by `print_changelog`.
    pub changelogs: Mutex<Vec<(String, String, String)>>,
    /// Queue of yes/no answers returned by `confirm`. Front = next answer.
    pub confirm_answers: Mutex<VecDeque<bool>>,
    /// Every question that was passed to `confirm`.
    pub confirm_questions: Mutex<Vec<String>>,
}

impl MockProgress {
    /// Create a `MockProgress` with empty tracking state.
    pub fn new() -> Self {
        Self {
            phases: Mutex::new(Vec::new()),
            increments: Mutex::new(0),
            finish_count: Mutex::new(0),
            tables: Mutex::new(Vec::new()),
            changelogs: Mutex::new(Vec::new()),
            confirm_answers: Mutex::new(VecDeque::new()),
            confirm_questions: Mutex::new(Vec::new()),
        }
    }

    /// Push a `true` / `false` answer onto the confirm queue.
    /// The next call to `confirm()` will pop and return this value.
    pub fn push_confirm(&self, answer: bool) {
        self.confirm_answers.lock().unwrap().push_back(answer);
    }
}

// ── ProgressRenderer impl ───────────────────────────────────────────────────

impl ProgressRenderer for MockProgress {
    fn start_phase(&self, label: &str, total: u64) {
        self.phases
            .lock()
            .unwrap()
            .push((label.to_string(), total));
    }

    fn increment(&self, n: u64) {
        *self.increments.lock().unwrap() += n;
    }

    fn finish_phase(&self) {
        *self.finish_count.lock().unwrap() += 1;
    }

    fn print_table(&self, headers: &[&str], rows: &[Vec<String>]) {
        self.tables.lock().unwrap().push((
            headers.iter().map(|h| h.to_string()).collect(),
            rows.to_vec(),
        ));
    }

    fn print_changelog(&self, slug: &str, version: &str, changelog: &str) {
        self.changelogs.lock().unwrap().push((
            slug.to_string(),
            version.to_string(),
            changelog.to_string(),
        ));
    }

    fn confirm(&self, question: &str) -> bool {
        self.confirm_questions
            .lock()
            .unwrap()
            .push(question.to_string());
        self.confirm_answers
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(false)
    }
}
