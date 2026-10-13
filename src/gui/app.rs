//! Main `eframe::App` implementation — the Anvil GUI.
//!
//! Renders a single window with a tab bar (Scan, Updates, Settings, Rollback),
//! a central content area, and a persistent status bar. The GUI polls the
//! background worker for events each frame. Config is shared with the worker
//! via `Arc<Mutex<ResolvedConfig>>` — no worker re-spawning needed.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::config::{ResolvedConfig, SettingsForm};
use crate::discover::DiscoveredModsDir;
use crate::types::{IdentifiedMod, ImportedMod, ModOutcome, Profile, RunSummary};

use super::state::{ExportFormat, Tab};
use super::worker::{spawn_worker, GuiCommand, WorkerEvent, WorkerHandle};

// ── Log entry ──────────────────────────────────────────────────────────────

struct LogEntry {
    message: String,
}

// ── Phase info ─────────────────────────────────────────────────────────────

struct PhaseInfo {
    label: String,
}

// ── Confirm dialog state ───────────────────────────────────────────────────

struct ConfirmDialogState {
    question: String,
}

// ── App state ──────────────────────────────────────────────────────────────

pub struct AnvilApp {
    // Worker communication
    worker: WorkerHandle,

    // Shared config with worker thread (no re-spawn needed)
    config: Arc<Mutex<ResolvedConfig>>,

    // UI state
    active_tab: Tab,
    mods_dir_input: String,

    // Scan results
    identified_mods: Vec<IdentifiedMod>,
    unknown_count: usize,
    total_jars: usize,

    /// Dependency graph over the full scan result (ignores Scan-tab filters).
    dep_graph: Option<crate::deps::DepGraph>,

    // Warnings from the last update run (warn-only, nothing was blocked)
    update_warnings: Vec<String>,

    // Filter state (for scan tab)
    filter_loader: String,
    filter_game_version: String,
    filter_include: String,
    filter_exclude: String,

    // Update results
    update_outcomes: Vec<ModOutcome>,
    candidates_count: usize,
    summary: Option<RunSummary>,

    // Progress tracking
    current_phase: Option<PhaseInfo>,
    progress_current: u64,
    progress_total: u64,

    // Confirmation
    confirm_dialog: Option<ConfirmDialogState>,

    // Log
    log_messages: Vec<LogEntry>,

    // Async data (loaded from API on startup)
    game_versions: Vec<String>,
    loaders: Vec<String>,

    // Worker busy
    worker_busy: bool,

    /// App self-update check result from GitHub.
    app_update_info: Option<crate::types::AppUpdateInfo>,
    /// Whether an app update check is in progress.
    app_update_checking: bool,
    /// Whether the startup update banner has been dismissed.
    update_banner_dismissed: bool,

    // Settings form overrides (stored separately, synced on Save)
    settings_backup: bool,
    settings_loader: String,
    settings_game_version: String,
    settings_max_updates: String,
    settings_confirm: bool,
    settings_changelog: bool,
    settings_include: String,
    settings_exclude: String,
    settings_dark_mode: bool,
    applied_dark_mode: bool,

    // Browse button deferred — native dialog opens between frames
    // so it doesn't conflict with egui's render context.
    browse_pending: bool,

    /// Deferred export dialog — opens between frames.
    export_pending: Option<ExportFormat>,

    /// Deferred import file dialog — opens between frames.
    import_pending: bool,

    /// Mods imported from an external mod list file.
    imported_mods: Vec<ImportedMod>,

    // Named mods folders, persisted in config.toml
    profiles: Vec<Profile>,
    active_profile: Option<String>,
    profile_name_input: String,
    detected_dirs: Vec<DiscoveredModsDir>,
}

impl AnvilApp {
    /// Create a new `AnvilApp` with the given resolved config.
    ///
    /// The config is wrapped in `Arc<Mutex<...>>` and shared with the worker
    /// thread so the GUI can update settings without re-spawning.
    pub fn new(resolved: ResolvedConfig) -> Self {
        let mods_dir_input = resolved.mods_dir.display().to_string();

        let config = Arc::new(Mutex::new(resolved.clone()));
        let worker = spawn_worker(Arc::clone(&config));

        // Request game versions and loaders on startup.
        worker.send_command(GuiCommand::FetchGameVersions);
        worker.send_command(GuiCommand::FetchLoaders);
        worker.send_command(GuiCommand::CheckAppUpdate);

        let sl = resolved.loader.unwrap_or_default();
        let sgv = resolved.game_version.unwrap_or_default();
        let smu = resolved
            .max_updates
            .map(|n| n.to_string())
            .unwrap_or_default();
        let sinc = resolved.include.join(", ");
        let sexc = resolved.exclude.join(", ");

        Self {
            worker,
            config,
            mods_dir_input,
            active_tab: Tab::Scan,
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
            confirm_dialog: None,
            log_messages: Vec::new(),
            game_versions: vec![],
            loaders: vec![],
            worker_busy: false,
            app_update_info: None,
            app_update_checking: true,
            update_banner_dismissed: false,
            settings_backup: resolved.backup,
            settings_loader: sl,
            settings_game_version: sgv,
            settings_max_updates: smu,
            settings_confirm: resolved.confirm,
            settings_changelog: resolved.changelog,
            settings_include: sinc,
            settings_exclude: sexc,
            settings_dark_mode: resolved.dark_mode,
            applied_dark_mode: resolved.dark_mode,
            browse_pending: false,
            export_pending: None,
            import_pending: false,
            imported_mods: vec![],
            profiles: resolved.profiles,
            active_profile: resolved.active_profile,
            profile_name_input: String::new(),
            detected_dirs: vec![],
        }
    }

    // ── Event handling ──────────────────────────────────────────────────

    fn process_events(&mut self) {
        for event in self.worker.try_recv_events() {
            match event {
                WorkerEvent::PhaseStarted { label, total, .. } => {
                    self.current_phase = Some(PhaseInfo {
                        label,
                    });
                    self.progress_current = 0;
                    self.progress_total = total;
                    self.worker_busy = true;
                }
                WorkerEvent::PhaseProgress { current } => {
                    self.progress_current = current;
                }
                WorkerEvent::PhaseFinished => {
                    self.current_phase = None;
                }
                WorkerEvent::OutcomesReady(outcomes) => {
                    self.candidates_count = outcomes
                        .iter()
                        .filter(|o| matches!(o, ModOutcome::Updated { .. }))
                        .count();
                    self.update_outcomes = outcomes;
                }
                WorkerEvent::ChangelogReady { .. } => {}
                WorkerEvent::Warnings(warnings) => {
                    self.update_warnings = warnings;
                }
                WorkerEvent::SummaryReady(summary) => {
                    self.summary = Some(summary);
                }
                WorkerEvent::ConfirmRequest { question } => {
                    self.confirm_dialog = Some(ConfirmDialogState { question });
                }
                WorkerEvent::Log { message } => {
                    if let Some(rest) = message.strip_prefix("game_versions:") {
                        self.game_versions = rest
                            .split(',')
                            .map(|s| s.to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    } else if let Some(rest) = message.strip_prefix("loaders:") {
                        self.loaders = rest
                            .split(',')
                            .map(|s| s.to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    } else {
                        self.log_messages.push(LogEntry { message });
                    }
                }
                WorkerEvent::ScanComplete {
                    identified,
                    unknown_count,
                    total_jars,
                    dep_graph,
                } => {
                    self.identified_mods = identified;
                    self.unknown_count = unknown_count;
                    self.total_jars = total_jars;
                    self.dep_graph = Some(dep_graph);
                    self.worker_busy = false;
                }
                WorkerEvent::UpdatesChecked {
                    outcomes,
                    candidates_count,
                } => {
                    self.update_outcomes = outcomes;
                    self.candidates_count = candidates_count;
                    self.worker_busy = false;
                }
                WorkerEvent::DownloadsComplete { summary } => {
                    self.summary = Some(summary);
                    // Auto re-check so the table refreshes with up-to-date status.
                    self.worker.send_command(GuiCommand::CheckUpdates);
                }
                WorkerEvent::RollbackComplete { count } => {
                    self.worker_busy = false;
                    self.log_messages.push(LogEntry {
                        message: format!("Restored {} mod(s) from backup.", count),
                    });
                }
                WorkerEvent::Error(msg) => {
                    self.worker_busy = false;
                    self.log_messages.push(LogEntry {
                        message: format!("Error: {}", msg),
                    });
                }
                WorkerEvent::Done => {
                    self.worker_busy = false;
                }
                WorkerEvent::AppUpdateResult {
                    latest_version,
                    url,
                    is_newer,
                } => {
                    self.app_update_info = Some(crate::types::AppUpdateInfo {
                        latest_version,
                        current_version: env!("CARGO_PKG_VERSION").to_string(),
                        url,
                        is_newer,
                    });
                    self.app_update_checking = false;
                    if is_newer {
                        self.update_banner_dismissed = false;
                    }
                }
                WorkerEvent::ImportDownloadComplete { success, failed } => {
                    self.worker_busy = false;
                    self.log_messages.push(LogEntry {
                        message: format!(
                            "Download complete: {} succeeded, {} failed. Re-scanning...",
                            success, failed
                        ),
                    });
                    // Auto-re-scan so identified_mods reflects the newly downloaded JARs.
                    let dir = PathBuf::from(&self.mods_dir_input);
                    self.identified_mods.clear();
                    self.unknown_count = 0;
                    self.total_jars = 0;
                    self.dep_graph = None;
                    self.imported_mods.clear();
                    self.worker.send_command(GuiCommand::ScanMods(dir));
                }
                // TableReady is informational — real data arrives via OutcomesReady.
                WorkerEvent::TableReady { .. } => {}
            }
        }
    }

    // ── Filter helpers ───────────────────────────────────────────────────

    fn filtered_mods(&self) -> Vec<&IdentifiedMod> {
        self.identified_mods
            .iter()
            .filter(|m| {
                if !self.filter_loader.is_empty() {
                    let fl = self.filter_loader.to_lowercase();
                    if !m.current_version
                        .loaders
                        .iter()
                        .any(|l| l.to_lowercase() == fl)
                    {
                        return false;
                    }
                }
                if !self.filter_game_version.is_empty()
                    && !m.current_version
                        .game_versions
                        .contains(&self.filter_game_version)
                {
                    return false;
                }
                if !self.filter_include.is_empty() {
                    let pat = self.filter_include.to_lowercase();
                    let name = m.current_version.name.to_lowercase();
                    let vn = m.current_version.version_number.to_lowercase();
                    if !name.contains(&pat) && !vn.contains(&pat) {
                        return false;
                    }
                }
                if !self.filter_exclude.is_empty() {
                    let pat = self.filter_exclude.to_lowercase();
                    let name = m.current_version.name.to_lowercase();
                    let vn = m.current_version.version_number.to_lowercase();
                    if name.contains(&pat) || vn.contains(&pat) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        // Always apply on first frame (applied_dark_mode starts matching settings).
        // After that, only re-apply when the toggle changes.
        crate::gui::theme::apply_theme(ctx, self.settings_dark_mode);
        self.applied_dark_mode = self.settings_dark_mode;
    }

    fn export_scan_results(&self, format: ExportFormat) -> String {
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

    /// Import a list of mods, filtering out any whose `project_id` already
    /// exists in the currently identified mods (i.e., already present on disk)
    /// to avoid pointless re-downloads.
    fn set_imported_mods_deduped(&mut self, new_mods: Vec<ImportedMod>) {
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
}

// ── eframe::App impl ──────────────────────────────────────────────────────

impl eframe::App for AnvilApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain worker events (non-blocking).
        self.process_events();
        self.apply_theme(ctx);

        // Open native folder dialog between frames (outside any ui scope)
        // so it doesn't conflict with egui's render context.
        if self.browse_pending {
            if let Some(path) = rfd::FileDialog::new()
                .set_directory(&self.mods_dir_input)
                .pick_folder() {
                self.mods_dir_input = path.display().to_string();
            }
            self.browse_pending = false;
            ctx.request_repaint();
        }

        if let Some(format) = self.export_pending.take() {
            let filter = match format {
                ExportFormat::Csv => rfd::FileDialog::new()
                    .add_filter("CSV Files", &["csv"])
                    .set_file_name("anvil_mods.csv"),
                ExportFormat::Markdown => rfd::FileDialog::new()
                    .add_filter("Markdown Files", &["md"])
                    .set_file_name("anvil_mods.md"),
                ExportFormat::Json => rfd::FileDialog::new()
                    .add_filter("JSON Files", &["json"])
                    .set_file_name("anvil_mods.json"),
            };
            if let Some(path) = filter.save_file() {
                let content = self.export_scan_results(format);
                let _ = std::fs::write(&path, content);
                self.log_messages.push(LogEntry {
                    message: format!("Exported to {}", path.display()),
                });
            }
            ctx.request_repaint();
        }

        if self.import_pending {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Mod list", &["json", "csv"])
                .add_filter("JSON", &["json"])
                .add_filter("CSV", &["csv"])
                .pick_file()
            {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        let ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("");
                        match ext {
                            "json" => {
                                if let Ok(mods) = serde_json::from_str::<
                                    Vec<ImportedMod>,
                                >(&content)
                                {
                                    self.set_imported_mods_deduped(mods);
                                    self.log_messages.push(LogEntry {
                                        message: format!(
                                            "Imported {} mod(s) from {}",
                                            self.imported_mods.len(),
                                            path.display()
                                        ),
                                    });
                                } else {
                                    self.log_messages.push(LogEntry {
                                        message: format!(
                                            "Error: Failed to parse JSON from {}",
                                            path.display()
                                        ),
                                    });
                                }
                            }
                            "csv" => {
                                let mods = super::util::parse_imported_csv(&content);
                                if !mods.is_empty() {
                                    self.set_imported_mods_deduped(mods);
                                    self.log_messages.push(LogEntry {
                                        message: format!(
                                            "Imported {} mod(s) from {}",
                                            self.imported_mods.len(),
                                            path.display()
                                        ),
                                    });
                                } else {
                                    self.log_messages.push(LogEntry {
                                        message: format!(
                                            "Error: No valid mods found in {}",
                                            path.display()
                                        ),
                                    });
                                }
                            }
                            _ => {
                                self.log_messages.push(LogEntry {
                                    message: format!(
                                        "Error: Unsupported file format: .{}",
                                        ext
                                    ),
                                });
                            }
                        }
                    }
                    Err(e) => {
                        self.log_messages.push(LogEntry {
                            message: format!("Error reading file: {}", e),
                        });
                    }
                }
            }
            self.import_pending = false;
            ctx.request_repaint();
        }

        // Keyboard shortcuts (only when no text field is focused).
        if !ctx.egui_wants_keyboard_input() {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::CTRL, egui::Key::R) {
                    self.active_tab = Tab::Scan;
                    let dir = PathBuf::from(&self.mods_dir_input);
                    self.identified_mods.clear();
                    self.unknown_count = 0;
                    self.total_jars = 0;
                    self.dep_graph = None;
                    self.update_warnings.clear();
                    self.log_messages.clear();
                    self.worker.send_command(GuiCommand::ScanMods(dir));
                }
                if i.consume_key(egui::Modifiers::CTRL, egui::Key::U) {
                    self.active_tab = Tab::Updates;
                    self.update_outcomes.clear();
                    self.candidates_count = 0;
                    self.summary = None;
                    self.update_warnings.clear();
                    self.log_messages.clear();
                    self.sync_config_to_worker();
                    self.worker.send_command(GuiCommand::CheckUpdates);
                }
            });
        }

        // Persist window geometry when the window is closing.
        if ctx.input(|i| i.viewport().close_requested()) {
            let geo_path = crate::paths::config_dir().join("window.json");
            if let Some(parent) = geo_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                let geo = serde_json::json!({
                    "width": rect.size().x,
                    "height": rect.size().y,
                    "x": rect.min.x,
                    "y": rect.min.y,
                });
                if let Ok(json) = serde_json::to_string_pretty(&geo) {
                    let _ = std::fs::write(&geo_path, json);
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // ── Left sidebar ─────────────────────────────────────────────────
        egui::Panel::left("sidebar")
            .resizable(false)
            .exact_size(200.0)
            .show(ui, |ui| {
                let sidebar = super::widgets::sidebar::Sidebar {
                    active: self.active_tab,
                    version: env!("CARGO_PKG_VERSION"),
                    has_update: self.app_update_info.as_ref().is_some_and(|i| i.is_newer),
                    dark_mode: self.settings_dark_mode,
                };
                let result = sidebar.show(ui);
                if let Some(tab) = result.switch_to {
                    self.active_tab = tab;
                }
                if result.toggle_theme {
                    self.settings_dark_mode = !self.settings_dark_mode;
                    self.applied_dark_mode = !self.applied_dark_mode;
                    super::theme::apply_theme(ui.ctx(), self.settings_dark_mode);
                }
            });

        // ── Top banner (app update) ──────────────────────────────────────
        egui::Panel::top("update_banner").show(ui, |ui| {
            if let Some(ref info) = self.app_update_info
                && info.is_newer
                && !self.update_banner_dismissed
            {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "Anvil {} is available! You are running {}.",
                            info.latest_version, info.current_version
                        ))
                        .color(crate::gui::theme::WARNING),
                    );
                    ui.hyperlink_to("Download", &info.url);
                    if ui.small_button("Dismiss").clicked() {
                        self.update_banner_dismissed = true;
                    }
                });
                ui.separator();
            }
        });

        // ── Bottom status bar ────────────────────────────────────────────
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(ref phase) = self.current_phase {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {}/{}",
                            phase.label, self.progress_current, self.progress_total
                        ))
                        .color(crate::gui::theme::TEXT_SECONDARY),
                    );
                    if self.progress_total > 0 {
                        let frac = self.progress_current as f32 / self.progress_total as f32;
                        let bar = egui::ProgressBar::new(frac)
                            .desired_width(200.0)
                            .fill(crate::gui::theme::ACCENT);
                        ui.add(bar);
                    }
                    if super::widgets::buttons::danger_button(ui, "Cancel").clicked() {
                        self.worker.send_command(GuiCommand::Cancel);
                    }
                } else if self.worker_busy {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new("Working...")
                            .color(crate::gui::theme::TEXT_SECONDARY),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("Idle")
                            .color(crate::gui::theme::TEXT_MUTED),
                    );
                }

                if let Some(last) = self.log_messages.last() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(&last.message)
                                .small()
                                .color(crate::gui::theme::TEXT_MUTED),
                        );
                    });
                }
            });
        });

        // ── Central content ──────────────────────────────────────────────
        egui::CentralPanel::default().show(ui, |ui| {
            match self.active_tab {
                Tab::Scan => self.render_scan_tab(ui),
                Tab::Updates => self.render_updates_tab(ui),
                Tab::Settings => self.render_settings_tab(ui),
                Tab::Rollback => self.render_rollback_tab(ui),
            }
        });

        // Push the settings form into the shared config once per frame.
        self.sync_config_to_worker();

        // ── Confirmation dialog ──────────────────────────────────────────
        if self.confirm_dialog.is_some() {
            let question = self.confirm_dialog.as_ref().unwrap().question.clone();
            let mut closed = false;
            let mut answer = false;

            egui::Window::new("Confirm")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    ui.label(&question);
                    ui.add_space(crate::gui::theme::SPACE_MD);
                    ui.horizontal(|ui| {
                        if super::widgets::buttons::primary_button(ui, "Confirm").clicked() {
                            answer = true;
                            closed = true;
                        }
                        if super::widgets::buttons::secondary_button(ui, "Cancel").clicked() {
                            answer = false;
                            closed = true;
                        }
                    });
                });

            if closed {
                if let Some(state) = self
                    .worker
                    .confirm_state
                    .lock()
                    .ok()
                    .and_then(|mut s| s.take())
                {
                    let _ = state.reply_tx.send(answer);
                }
                self.confirm_dialog = None;
            }
        }

        // Request repaint if worker is busy (for smooth progress updates).
        if self.worker_busy {
            ui.ctx().request_repaint();
        }
    }
}

// ── Tab rendering ─────────────────────────────────────────────────────────

impl AnvilApp {
    fn render_scan_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Scan & Identify Mods");

        ui.horizontal(|ui| {
            ui.label("Mods directory:");
            ui.text_edit_singleline(&mut self.mods_dir_input);
            if ui.button("Browse...").clicked() {
                self.browse_pending = true;
            }
        });

        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.worker_busy, egui::Button::new("Scan"))
                .clicked()
            {
                let dir = PathBuf::from(&self.mods_dir_input);
                self.identified_mods.clear();
                self.unknown_count = 0;
                self.total_jars = 0;
                self.dep_graph = None;
                self.update_warnings.clear();
                self.log_messages.clear();
                self.worker.send_command(GuiCommand::ScanMods(dir));
            }
            if self.worker_busy {
                ui.spinner();
            }
        });

        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.identified_mods.is_empty() && !self.worker_busy,
                    egui::Button::new("Export CSV"),
                )
                .clicked()
            {
                self.export_pending = Some(ExportFormat::Csv);
            }
            if ui
                .add_enabled(
                    !self.identified_mods.is_empty() && !self.worker_busy,
                    egui::Button::new("Export Markdown"),
                )
                .clicked()
            {
                self.export_pending = Some(ExportFormat::Markdown);
            }
            if ui
                .add_enabled(
                    !self.identified_mods.is_empty() && !self.worker_busy,
                    egui::Button::new("Export JSON"),
                )
                .clicked()
            {
                self.export_pending = Some(ExportFormat::Json);
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Import Mod List").clicked() {
                self.import_pending = true;
            }
            if !self.imported_mods.is_empty()
                && ui
                    .add_enabled(
                        !self.worker_busy,
                        egui::Button::new("Download All Imported"),
                    )
                    .clicked()
            {
                self.log_messages.clear();
                self.total_jars = self.imported_mods.len();
                self.worker
                    .send_command(GuiCommand::DownloadImportedMods(
                        self.imported_mods.clone(),
                    ));
            }
        });

        ui.separator();

        // Filters
        ui.collapsing("Filters", |ui| {
            ui.horizontal(|ui| {
                ui.label("Loader:");
                ui.text_edit_singleline(&mut self.filter_loader);
                ui.label("Game version:");
                ui.text_edit_singleline(&mut self.filter_game_version);
            });
            ui.horizontal(|ui| {
                ui.label("Include:");
                ui.text_edit_singleline(&mut self.filter_include);
                ui.label("Exclude:");
                ui.text_edit_singleline(&mut self.filter_exclude);
            });
        });

        ui.separator();

        // Dependencies — built over the full identified set (the filters above
        // do not affect it, same as `anvil list --deps`). Placed above the
        // results table because the table consumes the remaining height.
        if let Some(graph) = &self.dep_graph {
            let missing = &graph.missing;
            let incompat = &graph.incompat;
            let tree = crate::deps::format_tree(&self.identified_mods, graph);
            let issue_count = missing.len() + incompat.len();

            let header = if issue_count > 0 {
                format!("Dependencies \u{2014} {} issue(s)", issue_count)
            } else {
                String::from("Dependencies")
            };

            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                ui.make_persistent_id("scan_deps_section"),
                issue_count > 0,
            )
            .show_header(ui, |ui| {
                ui.strong(header);
            })
            .body(|ui| {
                for miss in missing {
                    let label = miss.to_name.as_deref().unwrap_or(&miss.to_project_id);
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("! {} requires {}", miss.dependent_name, label),
                    );
                }

                // Offer to download the missing required deps in one click.
                if !missing.is_empty() {
                    let project_ids: Vec<String> = missing
                        .iter()
                        .filter(|m| m.kind == crate::types::DependencyType::Required)
                        .map(|m| m.to_project_id.clone())
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect();
                    if !project_ids.is_empty()
                        && ui
                            .add_enabled(
                                !self.worker_busy,
                                egui::Button::new(format!(
                                    "Download {} missing dependenc{}",
                                    project_ids.len(),
                                    if project_ids.len() == 1 { "y" } else { "ies" }
                                )),
                            )
                            .clicked()
                    {
                        self.worker_busy = true;
                        self.worker.send_command(GuiCommand::InstallMissingDeps(project_ids));
                    }
                    ui.separator();
                }
                for pair in incompat {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        format!(
                            "! {} conflicts with {}",
                            pair.project_name, pair.conflicts_with_name
                        ),
                    );
                }
                if issue_count > 0 {
                    ui.separator();
                }
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .id_salt("scan_deps_tree")
                    .show(ui, |ui| {
                        for line in &tree {
                            ui.monospace(line);
                        }
                    });
            });

            ui.separator();
        }

        // Results table
        let filtered = self.filtered_mods();
        if filtered.is_empty() && self.total_jars == 0 {
            ui.label("No mods scanned yet. Choose a directory and click Scan.");
        } else {
            ui.label(format!(
                "Total JARs: {} | Identified: {} | Unknown: {} | Showing: {}",
                self.total_jars,
                self.identified_mods.len(),
                self.unknown_count,
                filtered.len(),
            ));

            let available_height = ui.available_height().max(200.0);
            ui.push_id("scan_identified_table", |ui| {
                egui::ScrollArea::vertical()
                    .max_height(available_height)
                    .show(ui, |ui| {
                        egui_extras::TableBuilder::new(ui)
                            .striped(true)
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .column(egui_extras::Column::remainder().at_least(150.0))
                            .column(egui_extras::Column::remainder().at_least(100.0))
                            .column(egui_extras::Column::remainder().at_least(70.0))
                            .column(egui_extras::Column::remainder().at_least(70.0))
                            .column(egui_extras::Column::remainder().at_least(100.0))
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.strong("Filename");
                                });
                                header.col(|ui| {
                                    ui.strong("Name");
                                });
                                header.col(|ui| {
                                    ui.strong("Version");
                                });
                                header.col(|ui| {
                                    ui.strong("Loader");
                                });
                                header.col(|ui| {
                                    ui.strong("Game Versions");
                                });
                            })
                            .body(|body| {
                                body.rows(18.0, filtered.len(), |mut row| {
                                    let idx = row.index();
                                    if let Some(m) = filtered.get(idx) {
                                        let loaders = m.current_version.loaders.join(", ");
                                        let gvs = m.current_version.game_versions.join(", ");
                                        row.col(|ui| {
                                            ui.label(&m.filename);
                                        });
                                        row.col(|ui| {
                                            ui.hyperlink_to(
                                                &m.current_version.name,
                                                format!(
                                                    "https://modrinth.com/mod/{}",
                                                    m.current_version.project_id
                                                ),
                                            );
                                        });
                                        row.col(|ui| {
                                            ui.label(&m.current_version.version_number);
                                        });
                                        row.col(|ui| {
                                            ui.label(if loaders.is_empty() { "\u{2014}" } else { &loaders });
                                        });
                                        row.col(|ui| {
                                            ui.label(if gvs.is_empty() { "\u{2014}" } else { &gvs });
                                        });
                                    }
                                });
                            });
                    });
            });
        }

        if !self.imported_mods.is_empty() {
            ui.separator();
            ui.heading(format!(
                "Imported Mods ({})",
                self.imported_mods.len()
            ));
            let available_height = ui.available_height().max(100.0);
            ui.push_id("scan_imported_table", |ui| {
                egui::ScrollArea::vertical()
                    .max_height(available_height)
                    .show(ui, |ui| {
                        egui_extras::TableBuilder::new(ui)
                            .striped(true)
                            .cell_layout(egui::Layout::left_to_right(
                                egui::Align::Center,
                            ))
                            .column(
                                egui_extras::Column::remainder().at_least(100.0),
                            )
                            .column(
                                egui_extras::Column::remainder().at_least(60.0),
                            )
                            .column(
                                egui_extras::Column::remainder().at_least(80.0),
                            )
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.strong("Name");
                                });
                                header.col(|ui| {
                                    ui.strong("Version");
                                });
                                header.col(|ui| {
                                    ui.strong("Loader");
                                });
                            })
                            .body(|body| {
                                body.rows(18.0, self.imported_mods.len(), |mut row| {
                                    let idx = row.index();
                                    if let Some(m) = self.imported_mods.get(idx) {
                                        let name = m
                                            .name
                                            .as_deref()
                                            .unwrap_or(&m.project_id);
                                        let ver = m
                                            .version_number
                                            .as_deref()
                                            .unwrap_or("(latest)");
                                        let ld = m.loaders.join(", ");
                                        row.col(|ui| {
                                            ui.label(name);
                                        });
                                        row.col(|ui| {
                                            ui.label(ver);
                                        });
                                        row.col(|ui| {
                                            ui.label(if ld.is_empty() {
                                                "\u{2014}"
                                            } else {
                                                &ld
                                            });
                                        });
                                    }
                                });
                            });
                    });
            });
        }
    }

    fn render_updates_tab(&mut self, ui: &mut egui::Ui) {
        use crate::gui::widgets::{badge, buttons, card, empty_state, stat_tiles};

        // Warnings banner
        for warning in &self.update_warnings.clone() {
            empty_state::WarningBanner::new(warning).show(ui);
            ui.add_space(crate::gui::theme::SPACE_XS);
        }

        // Target platform card
        card::SectionCard::new("Target Platform").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Game version:")
                        .color(crate::gui::theme::TEXT_SECONDARY),
                );
                egui::ComboBox::from_id_salt("updates_game_version")
                    .width(120.0)
                    .selected_text(if self.settings_game_version.is_empty() {
                        "(auto)"
                    } else {
                        &self.settings_game_version
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.settings_game_version,
                            String::new(),
                            "(auto-detect)",
                        );
                        for v in &self.game_versions.clone() {
                            ui.selectable_value(
                                &mut self.settings_game_version,
                                v.clone(),
                                v.clone(),
                            );
                        }
                    });

                ui.add_space(crate::gui::theme::SPACE_MD);
                ui.label(
                    egui::RichText::new("Loader:")
                        .color(crate::gui::theme::TEXT_SECONDARY),
                );
                egui::ComboBox::from_id_salt("updates_loader")
                    .width(100.0)
                    .selected_text(if self.settings_loader.is_empty() {
                        "(auto)"
                    } else {
                        &self.settings_loader
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.settings_loader,
                            String::new(),
                            "(auto-detect)",
                        );
                        for l in &self.loaders.clone() {
                            ui.selectable_value(
                                &mut self.settings_loader,
                                l.clone(),
                                l.clone(),
                            );
                        }
                    });
            });
        });
        ui.add_space(crate::gui::theme::SPACE_SM);

        // Action buttons
        ui.horizontal(|ui| {
            let check_btn = if self.worker_busy {
                buttons::secondary_button(ui, "Checking...")
            } else {
                buttons::primary_button(ui, "Check for Updates")
            };
            if check_btn.clicked() {
                self.update_outcomes.clear();
                self.candidates_count = 0;
                self.summary = None;
                self.update_warnings.clear();
                self.log_messages.clear();
                self.worker.send_command(GuiCommand::CheckUpdates);
            }

            let can_download = self.candidates_count > 0 && !self.worker_busy;
            let dl_text = format!("Download {} Update(s)", self.candidates_count);
            let dl_btn = if can_download {
                buttons::primary_button(ui, &dl_text)
            } else {
                buttons::secondary_button(ui, &dl_text)
            };
            if dl_btn.clicked() && can_download {
                self.update_outcomes.clear();
                self.summary = None;
                self.worker.send_command(GuiCommand::DownloadUpdates);
            }

            if self.worker_busy {
                ui.spinner();
            }
        });

        ui.add_space(crate::gui::theme::SPACE_SM);

        // Summary stat tiles
        if let Some(ref summary) = self.summary {
            ui.horizontal(|ui| {
                stat_tiles::StatTile::new(
                    summary.updates_applied.to_string(),
                    "Updates",
                )
                .accent(crate::gui::theme::ACCENT)
                .show(ui);
                stat_tiles::StatTile::new(
                    summary.up_to_date.to_string(),
                    "Up to date",
                )
                .accent(crate::gui::theme::TEXT_SECONDARY)
                .show(ui);
                stat_tiles::StatTile::new(
                    summary.unavailable.to_string(),
                    "Unavailable",
                )
                .accent(crate::gui::theme::WARNING)
                .show(ui);
                stat_tiles::StatTile::new(summary.failed.to_string(), "Failed")
                    .accent(crate::gui::theme::ERROR)
                    .show(ui);
            });
            ui.add_space(crate::gui::theme::SPACE_SM);
        }

        // Context line
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Checking with:")
                    .size(crate::gui::theme::FONT_CAPTION)
                    .color(crate::gui::theme::TEXT_MUTED),
            );
            let gv = self.settings_game_version.as_str();
            let ld = self.settings_loader.as_str();
            if gv.is_empty() && ld.is_empty() {
                ui.label(
                    egui::RichText::new("(auto-detect per mod)")
                        .size(crate::gui::theme::FONT_CAPTION)
                        .color(crate::gui::theme::TEXT_MUTED),
                );
            } else {
                if !gv.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("MC {gv}"))
                            .size(crate::gui::theme::FONT_CAPTION)
                            .color(crate::gui::theme::TEXT_SECONDARY),
                    );
                }
                if !ld.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("/ {ld}"))
                            .size(crate::gui::theme::FONT_CAPTION)
                            .color(crate::gui::theme::TEXT_SECONDARY),
                    );
                }
            }
        });

        ui.add_space(crate::gui::theme::SPACE_SM);

        // Outcomes list
        if self.update_outcomes.is_empty() {
            empty_state::EmptyState::new("\u{2b06}", "No results yet")
                .subtitle("Click 'Check for Updates' to scan for available updates.")
                .show(ui);
        } else {
            let available_height = ui.available_height().max(150.0);
            egui::ScrollArea::vertical()
                .id_salt("updates_outcomes")
                .max_height(available_height)
                .show(ui, |ui| {
                    egui_extras::TableBuilder::new(ui)
                        .striped(true)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(egui_extras::Column::exact(80.0))
                        .column(egui_extras::Column::remainder().at_least(120.0))
                        .column(egui_extras::Column::remainder().at_least(80.0))
                        .column(egui_extras::Column::remainder().at_least(80.0))
                        .header(24.0, |mut header| {
                            header.col(|ui| {
                                ui.strong("Status");
                            });
                            header.col(|ui| {
                                ui.strong("Mod");
                            });
                            header.col(|ui| {
                                ui.strong("Current");
                            });
                            header.col(|ui| {
                                ui.strong("Latest");
                            });
                        })
                        .body(|body| {
                            body.rows(28.0, self.update_outcomes.len(), |mut row| {
                                let idx = row.index();
                                if let Some(outcome) = self.update_outcomes.get(idx) {
                                    row.col(|ui| {
                                        badge::StatusBadge::from_outcome(outcome).show(ui);
                                    });
                                    row.col(|ui| {
                                        render_outcome_label(ui, outcome);
                                    });
                                    row.col(|ui| {
                                        ui.label(
                                            egui::RichText::new(super::util::outcome_current(outcome))
                                                .size(crate::gui::theme::FONT_CAPTION)
                                                .color(crate::gui::theme::TEXT_SECONDARY),
                                        );
                                    });
                                    row.col(|ui| {
                                        ui.label(
                                            egui::RichText::new(super::util::outcome_latest(outcome))
                                                .size(crate::gui::theme::FONT_CAPTION)
                                                .color(crate::gui::theme::TEXT_PRIMARY),
                                        );
                                    });
                                }
                            });
                        });
                });
        }
    }

    fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");

        ui.horizontal(|ui| {
            ui.label("Mods directory:");
            ui.text_edit_singleline(&mut self.mods_dir_input);
            if ui.button("Browse...").clicked() {
                self.browse_pending = true;
            }
        });

        // Profiles belong right under the row they control: they *are*
        // "which folder".
        self.render_profiles_section(ui);

        ui.checkbox(
            &mut self.settings_backup,
            "Create backup before replacing JARs",
        );

        ui.horizontal(|ui| {
            ui.label("Loader:");
            egui::ComboBox::from_id_salt("settings_loader")
                .selected_text(if self.settings_loader.is_empty() {
                    "(auto-detect)"
                } else {
                    &self.settings_loader
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.settings_loader,
                        String::new(),
                        "(auto-detect)",
                    );
                    for l in &self.loaders.clone() {
                        ui.selectable_value(&mut self.settings_loader, l.clone(), l);
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Game version:");
            egui::ComboBox::from_id_salt("settings_game_version")
                .selected_text(if self.settings_game_version.is_empty() {
                    "(auto-detect)"
                } else {
                    &self.settings_game_version
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.settings_game_version,
                        String::new(),
                        "(auto-detect)",
                    );
                    for v in &self.game_versions.clone() {
                        ui.selectable_value(&mut self.settings_game_version, v.clone(), v);
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Max updates:");
            ui.text_edit_singleline(&mut self.settings_max_updates);
        });

        ui.label("Include patterns (comma-separated):");
        ui.text_edit_singleline(&mut self.settings_include);

        ui.label("Exclude patterns (comma-separated):");
        ui.text_edit_singleline(&mut self.settings_exclude);

        ui.checkbox(&mut self.settings_confirm, "Confirm before downloading");
        ui.checkbox(&mut self.settings_changelog, "Show changelog");

        ui.separator();
        ui.heading("App Updates");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.app_update_checking,
                    egui::Button::new("Check for Updates"),
                )
                .clicked()
            {
                self.app_update_checking = true;
                self.app_update_info = None;
                self.worker.send_command(GuiCommand::CheckAppUpdate);
            }
            if self.app_update_checking {
                ui.spinner();
                ui.label("Checking...");
            }
        });
        if let Some(ref info) = self.app_update_info {
            if info.is_newer {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!(
                        "New version available: {} (you are running {}).",
                        info.latest_version, info.current_version
                    ),
                );
                ui.hyperlink_to("Open download page", &info.url);
            } else {
                ui.colored_label(
                    egui::Color32::GREEN,
                    format!(
                        "You are running the latest version ({}).",
                        info.current_version
                    ),
                );
            }
        }

        ui.separator();
        ui.heading("Appearance");
        ui.checkbox(&mut self.settings_dark_mode, "Dark mode");

        ui.separator();

        if ui.button("Save Settings").clicked() {
            self.sync_config_to_worker();
            let message = match self.save_settings() {
                Ok(()) => "Settings saved.".to_string(),
                Err(e) => format!("Error saving settings: {}", e),
            };
            self.log_messages.push(LogEntry { message });
        }
    }

    /// Profiles: choose the active folder, edit the list, add detected folders.
    ///
    /// Every mutation is staged while rendering and applied afterwards — the
    /// `ui` closures borrow `self` mutably, so the list cannot be mutated
    /// inside them.
    fn render_profiles_section(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label(egui::RichText::new("Profiles").strong());

        let mut activate: Option<Option<String>> = None;
        let mut remove: Option<usize> = None;
        let mut rename: Option<(usize, String)> = None;
        let mut update_from_current: Option<usize> = None;
        let mut add_named = false;
        let mut add_detected: Option<usize> = None;

        ui.horizontal(|ui| {
            ui.label("Active:");
            let selected = self
                .active_profile
                .clone()
                .unwrap_or_else(|| "(none)".to_string());
            egui::ComboBox::from_id_salt("settings_profile")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(self.active_profile.is_none(), "(none)")
                        .clicked()
                    {
                        activate = Some(None);
                    }
                    for profile in self.profiles.clone() {
                        let is_active =
                            self.active_profile.as_deref() == Some(profile.name.as_str());
                        if ui.selectable_label(is_active, &profile.name).clicked() {
                            activate = Some(Some(profile.name));
                        }
                    }
                });
        });

        for (idx, profile) in self.profiles.clone().iter().enumerate() {
            ui.horizontal(|ui| {
                let mut name = profile.name.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut name).desired_width(140.0))
                    .changed()
                {
                    rename = Some((idx, name));
                }
                ui.label(profile.mods_dir.display().to_string());
                if ui.button("Use").clicked() {
                    activate = Some(Some(profile.name.clone()));
                }
                if ui.button("Update from current folder").clicked() {
                    update_from_current = Some(idx);
                }
                if ui.button("Remove").clicked() {
                    remove = Some(idx);
                }
            });
        }

        ui.horizontal(|ui| {
            ui.label("New profile:");
            ui.add(
                egui::TextEdit::singleline(&mut self.profile_name_input)
                    .hint_text("name")
                    .desired_width(140.0),
            );
            if ui.button("Add").clicked() {
                add_named = true;
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Detect Mods Folders").clicked() {
                // Pure std::fs at depth <= 2 — no need to defer like `rfd`.
                self.detected_dirs = crate::discover::discover();
                if self.detected_dirs.is_empty() {
                    self.log_messages.push(LogEntry {
                        message: "No launcher mods folders detected.".into(),
                    });
                }
            }
        });
        for (idx, detected) in self.detected_dirs.clone().iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} — {} ({} JAR(s))",
                    detected.launcher.label(),
                    detected.mods_dir.display(),
                    detected.jar_count
                ));
                if ui.button("Add").clicked() {
                    add_detected = Some(idx);
                }
            });
        }

        // ── Apply staged actions ─────────────────────────────────────────
        if let Some((idx, name)) = rename
            && !name.trim().is_empty()
            && let Some(profile) = self.profiles.get_mut(idx)
        {
            profile.name = name;
        }
        if let Some(idx) = update_from_current {
            let dir = PathBuf::from(&self.mods_dir_input);
            if let Some(profile) = self.profiles.get_mut(idx) {
                profile.mods_dir = dir;
            }
        }
        if let Some(idx) = remove {
            let removed = self.profiles.remove(idx);
            // Never leave active_profile pointing at a deleted profile.
            if self.active_profile.as_deref() == Some(removed.name.as_str()) {
                self.active_profile = None;
                self.sync_config_to_worker();
            }
        }
        if add_named {
            let name = self.profile_name_input.clone();
            let dir = PathBuf::from(&self.mods_dir_input);
            self.add_profile(&name, dir);
        }
        if let Some(idx) = add_detected
            && let Some(detected) = self.detected_dirs.get(idx).cloned()
        {
            let name = self.unique_profile_name(&detected);
            self.add_profile(&name, detected.mods_dir);
        }
        if let Some(target) = activate {
            self.activate_profile(target);
        }
    }

    /// Add a profile (updating it in place when the name already exists) and
    /// make it active.
    fn add_profile(&mut self, name: &str, mods_dir: PathBuf) {
        let name = name.trim();
        if name.is_empty() {
            self.log_messages.push(LogEntry {
                message: "Profile name cannot be empty.".into(),
            });
            return;
        }

        match self.profiles.iter_mut().find(|p| p.name == name) {
            Some(existing) => existing.mods_dir = mods_dir,
            None => self.profiles.push(Profile {
                name: name.to_string(),
                mods_dir,
                loader: None,
                game_version: None,
            }),
        }
        self.profile_name_input.clear();
        self.activate_profile(Some(name.to_string()));
    }

    /// Select a profile (or `(none)`), point the mods-directory field at it,
    /// push the change to the worker, and rescan — the previously listed mods
    /// belong to the folder being left behind.
    fn activate_profile(&mut self, name: Option<String>) {
        self.active_profile = name;

        let dir = self
            .active_profile
            .as_ref()
            .and_then(|name| self.profiles.iter().find(|p| &p.name == name))
            .map(|p| p.mods_dir.clone());

        let Some(dir) = dir else {
            // "(none)" only clears the selection: the typed-in folder stands.
            self.sync_config_to_worker();
            return;
        };

        // Must go through `mods_dir_input`: `sync_config_to_worker` overwrites
        // `cfg.mods_dir` from it every frame, so writing the shared config
        // directly would be clobbered.
        self.mods_dir_input = dir.display().to_string();
        self.sync_config_to_worker();

        self.identified_mods.clear();
        self.unknown_count = 0;
        self.total_jars = 0;
        self.dep_graph = None;
        self.worker.send_command(GuiCommand::ScanMods(dir));
    }

    /// A non-clashing profile name for a detected folder, e.g. `prism-spark`.
    fn unique_profile_name(&self, detected: &DiscoveredModsDir) -> String {
        let base = match super::util::instance_label(&detected.mods_dir) {
            Some(instance) => format!("{}-{}", detected.launcher.slug(), super::util::slugify(&instance)),
            None => detected.launcher.slug().to_string(),
        };

        if self.profile_name_is_free(&base) {
            return base;
        }
        (2..)
            .map(|n| format!("{}-{}", base, n))
            .find(|candidate| self.profile_name_is_free(candidate))
            .unwrap_or(base)
    }

    fn profile_name_is_free(&self, name: &str) -> bool {
        !self.profiles.iter().any(|p| p.name == name)
    }

    fn render_rollback_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Rollback");

        ui.label(
            "This restores all mods from a previous backup. Your current \
             JAR files are saved to a safety backup first — nothing is lost.",
        );

        // Show rollback result prominently.
        if let Some(msg) = self.log_messages.iter().find(|e| e.message.starts_with("Restored"))
        {
            ui.colored_label(egui::Color32::GREEN, format!("✓ {}", msg.message));
            ui.label("Your mods have been restored. Switch to the Scan tab to verify.");
        }
        if let Some(msg) = self.log_messages.iter().find(|e| e.message.starts_with("Rollback error"))
        {
            ui.colored_label(egui::Color32::RED, format!("✗ {}", msg.message));
        }

        let mods_dir = self
            .config
            .lock()
            .map(|c| c.mods_dir.clone())
            .unwrap_or_default();

        let backups = crate::backup::find_latest_backup(&mods_dir);

        match &backups {
            Ok(backup_dir) => {
                let file_count = std::fs::read_dir(backup_dir)
                    .map(|d| {
                        d.filter(|e| {
                            e.as_ref()
                                .map(|x| x.path().is_file())
                                .unwrap_or(false)
                        })
                        .count()
                    })
                    .unwrap_or(0);

                ui.label(format!(
                    "Latest backup: {} ({} file(s))",
                    backup_dir.display(),
                    file_count
                ));
                if ui
                    .add_enabled(
                        !self.worker_busy,
                        egui::Button::new("Restore from Backup"),
                    )
                    .clicked()
                {
                    self.log_messages.clear();
                    self.worker.send_command(GuiCommand::Rollback);
                }
            }
            Err(e) => {
                if matches!(e, crate::error::Error::NoBackups) {
                    ui.label(
                        "No backups found in the mods directory. \
                         Run an update first to create one.",
                    );
                } else {
                    ui.label(format!(
                        "Error scanning for backups: {}",
                        e
                    ));
                }
            }
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    /// Sync form settings into the shared config (no worker re-spawn).
    fn sync_config_to_worker(&mut self) {
        let mut cfg = self.config.lock().expect("config lock");
        cfg.mods_dir = PathBuf::from(&self.mods_dir_input);
        cfg.loader = if self.settings_loader.is_empty() {
            None
        } else {
            Some(self.settings_loader.clone())
        };
        cfg.game_version = if self.settings_game_version.is_empty() {
            None
        } else {
            Some(self.settings_game_version.clone())
        };
        cfg.max_updates = self.settings_max_updates.parse::<usize>().ok();
        cfg.backup = self.settings_backup;
        cfg.confirm = self.settings_confirm;
        cfg.changelog = self.settings_changelog;
        cfg.include = self
            .settings_include
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        cfg.exclude = self
            .settings_exclude
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        cfg.profiles = self.profiles.clone();
        cfg.active_profile = self.active_profile.clone();
    }

    /// Persist the Settings tab into `config.toml`.
    ///
    /// Read-modify-write: the file is the only store, so building a fresh
    /// `Config` here would silently delete every key the Settings tab does not
    /// own (`verbose`, `quiet`, `dry_run`) on every save.
    fn save_settings(&self) -> crate::error::Result<()> {
        // Same file the app was resolved from, rather than re-deriving it.
        let config_path = self
            .config
            .lock()
            .map(|cfg| cfg.config_path.clone())
            .unwrap_or_else(|_| crate::paths::config_dir().join("config.toml"));
        let existing = crate::config::load(&config_path)?;

        let form = SettingsForm {
            // While a profile is active the profile owns the folder; writing it
            // into the global key would freeze that path into the file.
            mods_dir: if self.active_profile.is_some() {
                None
            } else {
                Some(PathBuf::from(&self.mods_dir_input))
            },
            backup: self.settings_backup,
            loader: (!self.settings_loader.is_empty()).then(|| self.settings_loader.clone()),
            game_version: (!self.settings_game_version.is_empty())
                .then(|| self.settings_game_version.clone()),
            include: super::util::split_patterns(&self.settings_include),
            exclude: super::util::split_patterns(&self.settings_exclude),
            max_updates: self.settings_max_updates.parse::<usize>().ok(),
            confirm: self.settings_confirm,
            changelog: self.settings_changelog,
            dark_mode: self.settings_dark_mode,
            profiles: self.profiles.clone(),
            active_profile: self.active_profile.clone(),
        };

        let merged = crate::config::merge_settings(existing, &form);

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(&merged).map_err(|e| {
            crate::error::Error::Config(format!("failed to serialize config: {}", e))
        })?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
}

// ── Outcome display helpers ────────────────────────────────────────────────

fn render_outcome_label(ui: &mut egui::Ui, outcome: &ModOutcome) {
    match outcome {
        ModOutcome::Updated { slug, .. }
        | ModOutcome::UpToDate { slug, .. }
        | ModOutcome::Unavailable { slug, .. } => {
            ui.hyperlink_to(slug.as_str(), format!("https://modrinth.com/mod/{}", slug));
        }
        _ => {
            ui.label(super::util::outcome_label(outcome));
        }
    }
}
