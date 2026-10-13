//! UI state container — all mutable application state for the GUI.
//!
//! `AppState` is intentionally free of worker/IO dependencies so it can be
//! constructed in tests without spawning threads or hitting the network.

use crate::config::ResolvedConfig;
use crate::discover::DiscoveredModsDir;
use crate::types::{IdentifiedMod, ImportedMod, ModOutcome, Profile, RunSummary};

// ── Tab enum ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Scan,
    Updates,
    Settings,
    Rollback,
}

impl Tab {
    pub fn label(self) -> &'static str {
        match self {
            Tab::Scan => "Scan & Identify",
            Tab::Updates => "Updates",
            Tab::Settings => "Settings",
            Tab::Rollback => "Rollback",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Tab::Scan => "\u{26cf}",
            Tab::Updates => "\u{2b06}",
            Tab::Settings => "\u{2699}",
            Tab::Rollback => "\u{21ba}",
        }
    }
}

// ── Export format ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Markdown,
    Json,
}

// ── Supporting types ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct PhaseInfo {
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct ConfirmDialogState {
    pub question: String,
}

// ── AppState ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct AppState {
    // UI state
    pub active_tab: Tab,
    pub mods_dir_input: String,

    // Scan results
    pub identified_mods: Vec<IdentifiedMod>,
    pub unknown_count: usize,
    pub total_jars: usize,
    pub dep_graph: Option<crate::deps::DepGraph>,

    // Warnings from the last update run
    pub update_warnings: Vec<String>,

    // Filter state (for scan tab)
    pub filter_loader: String,
    pub filter_game_version: String,
    pub filter_include: String,
    pub filter_exclude: String,

    // Update results
    pub update_outcomes: Vec<ModOutcome>,
    pub candidates_count: usize,
    pub summary: Option<RunSummary>,

    // Progress tracking
    pub current_phase: Option<PhaseInfo>,
    pub progress_current: u64,
    pub progress_total: u64,

    // Log
    pub log_messages: Vec<LogEntry>,

    // Async data (loaded from API on startup)
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,

    // Worker busy
    pub worker_busy: bool,

    // App self-update
    pub app_update_info: Option<crate::types::AppUpdateInfo>,
    pub app_update_checking: bool,
    pub update_banner_dismissed: bool,

    // Settings form fields
    pub settings_backup: bool,
    pub settings_loader: String,
    pub settings_game_version: String,
    pub settings_max_updates: String,
    pub settings_confirm: bool,
    pub settings_changelog: bool,
    pub settings_include: String,
    pub settings_exclude: String,
    pub settings_dark_mode: bool,
    pub applied_dark_mode: bool,

    // Imported mods
    pub imported_mods: Vec<ImportedMod>,

    // Profiles
    pub profiles: Vec<Profile>,
    pub active_profile: Option<String>,
    pub profile_name_input: String,
    pub detected_dirs: Vec<DiscoveredModsDir>,
}

impl AppState {
    /// Create state from a resolved config (without any worker).
    pub fn from_config(resolved: &ResolvedConfig) -> Self {
        Self {
            active_tab: Tab::Scan,
            mods_dir_input: resolved.mods_dir.display().to_string(),
            identified_mods: vec![],
            unknown_count: 0,
            total_jars: 0,
            dep_graph: None,
            update_warnings: vec![],
            filter_loader: String::new(),
            filter_game_version: String::new(),
            filter_include: String::new(),
            filter_exclude: String::new(),
            update_outcomes: vec![],
            candidates_count: 0,
            summary: None,
            current_phase: None,
            progress_current: 0,
            progress_total: 0,
            log_messages: Vec::new(),
            game_versions: vec![],
            loaders: vec![],
            worker_busy: false,
            app_update_info: None,
            app_update_checking: true,
            update_banner_dismissed: false,
            settings_backup: resolved.backup,
            settings_loader: resolved.loader.clone().unwrap_or_default(),
            settings_game_version: resolved.game_version.clone().unwrap_or_default(),
            settings_max_updates: resolved
                .max_updates
                .map(|n| n.to_string())
                .unwrap_or_default(),
            settings_confirm: resolved.confirm,
            settings_changelog: resolved.changelog,
            settings_include: resolved.include.join(", "),
            settings_exclude: resolved.exclude.join(", "),
            settings_dark_mode: resolved.dark_mode,
            applied_dark_mode: resolved.dark_mode,
            imported_mods: vec![],
            profiles: resolved.profiles.clone(),
            active_profile: resolved.active_profile.clone(),
            profile_name_input: String::new(),
            detected_dirs: vec![],
        }
    }

    // ── Filtered view of scan results ─────────────────────────────────

    /// Apply client-side filters to the identified mods list.
    pub fn filtered_mods(&self) -> Vec<&IdentifiedMod> {
        self.identified_mods
            .iter()
            .filter(|m| {
                if !self.filter_loader.is_empty()
                    && !m
                        .current_version
                        .loaders
                        .iter()
                        .any(|l| l.eq_ignore_ascii_case(&self.filter_loader))
                {
                    return false;
                }
                if !self.filter_game_version.is_empty()
                    && !m
                        .current_version
                        .game_versions
                        .contains(&self.filter_game_version)
                {
                    return false;
                }
                if !self.filter_include.is_empty() {
                    let needle = self.filter_include.to_lowercase();
                    let name = m.current_version.name.to_lowercase();
                    let ver = m.current_version.version_number.to_lowercase();
                    if !name.contains(&needle) && !ver.contains(&needle) {
                        return false;
                    }
                }
                if !self.filter_exclude.is_empty() {
                    let needle = self.filter_exclude.to_lowercase();
                    let name = m.current_version.name.to_lowercase();
                    let ver = m.current_version.version_number.to_lowercase();
                    if name.contains(&needle) || ver.contains(&needle) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    // ── Export ────────────────────────────────────────────────────────

    /// Serialize scan results in the given format.
    pub fn export_scan_results(&self, format: ExportFormat) -> String {
        let mods: Vec<(&String, &String, &String, &String, String, String)> = self
            .identified_mods
            .iter()
            .map(|m| {
                (
                    &m.filename,
                    &m.current_version.name,
                    &m.current_version.version_number,
                    &m.current_version.project_id,
                    m.current_version.loaders.join(", "),
                    m.current_version.game_versions.join(", "),
                )
            })
            .collect();

        match format {
            ExportFormat::Csv => {
                let mut out =
                    String::from("Filename,Name,Version,Project ID,Loader,Game Versions\n");
                for (f, n, v, pid, l, g) in &mods {
                    let escape = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
                    out.push_str(&format!(
                        "{},{},{},{},{},{}\n",
                        escape(f),
                        escape(n),
                        escape(v),
                        escape(pid),
                        escape(l),
                        escape(g)
                    ));
                }
                out
            }
            ExportFormat::Markdown => {
                let mut out =
                    String::from("| Filename | Name | Version | Project ID | Loader | Game Versions |\n");
                out.push_str("| --- | --- | --- | --- | --- | --- |\n");
                for (f, n, v, pid, l, g) in &mods {
                    out.push_str(&format!("| {} | {} | {} | {} | {} | {} |\n", f, n, v, pid, l, g));
                }
                out
            }
            ExportFormat::Json => {
                let entries: Vec<serde_json::Value> = self
                    .identified_mods
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "filename": m.filename,
                            "name": m.current_version.name,
                            "version_number": m.current_version.version_number,
                            "project_id": m.current_version.project_id,
                            "loaders": m.current_version.loaders,
                            "game_versions": m.current_version.game_versions,
                        })
                    })
                    .collect();
                serde_json::to_string_pretty(&entries).unwrap_or_default()
            }
        }
    }

    // ── Import dedup ──────────────────────────────────────────────────

    /// Import a list of mods, filtering out any whose `project_id` already
    /// exists in the currently identified mods (i.e., already present on disk)
    /// to avoid pointless re-downloads.
    pub fn set_imported_mods_deduped(&mut self, new_mods: Vec<ImportedMod>) {
        let total = new_mods.len();
        let existing_ids: std::collections::HashSet<String> = self
            .identified_mods
            .iter()
            .map(|m| m.current_version.project_id.clone())
            .collect();
        let fresh: Vec<ImportedMod> = new_mods
            .into_iter()
            .filter(|m| !existing_ids.contains(&m.project_id))
            .collect();
        let skipped = total - fresh.len();
        self.imported_mods = fresh;
        if skipped > 0 {
            self.log_messages.push(LogEntry {
                message: format!(
                    "Skipped {} mod(s) already present in scan results.",
                    skipped
                ),
            });
        }
    }

    // ── Config sync ───────────────────────────────────────────────────

    /// Copy settings form fields into a `SettingsForm` for persisting.
    pub fn settings_form(&self) -> crate::config::SettingsForm {
        crate::config::SettingsForm {
            mods_dir: if self.mods_dir_input.is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(&self.mods_dir_input))
            },
            backup: self.settings_backup,
            loader: if self.settings_loader.is_empty() {
                None
            } else {
                Some(self.settings_loader.clone())
            },
            game_version: if self.settings_game_version.is_empty() {
                None
            } else {
                Some(self.settings_game_version.clone())
            },
            include: super::util::split_patterns(&self.settings_include),
            exclude: super::util::split_patterns(&self.settings_exclude),
            max_updates: self
                .settings_max_updates
                .parse::<usize>()
                .ok(),
            confirm: self.settings_confirm,
            changelog: self.settings_changelog,
            dark_mode: self.settings_dark_mode,
            profiles: self.profiles.clone(),
            active_profile: self.active_profile.clone(),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "tests/test_gui_state.rs"]
mod tests;
