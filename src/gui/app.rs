//! Main `eframe::App` implementation — the Anvil GUI.
//!
//! Renders a single window with a tab bar (Scan, Updates, Settings, Rollback),
//! a central content area, and a persistent status bar. The GUI polls the
//! background worker for events each frame. Config is shared with the worker
//! via `Arc<Mutex<ResolvedConfig>>` — no worker re-spawning needed.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::config::ResolvedConfig;
use crate::types::{IdentifiedMod, ImportedMod, ModOutcome, RunSummary};

use super::worker::{spawn_worker, GuiCommand, WorkerEvent, WorkerHandle};

// ── Tab enum ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Scan,
    Updates,
    Settings,
    Rollback,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Scan => "Scan & Identify",
            Tab::Updates => "Updates",
            Tab::Settings => "Settings",
            Tab::Rollback => "Rollback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportFormat {
    Csv,
    Markdown,
    Json,
}

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
                } => {
                    self.identified_mods = identified;
                    self.unknown_count = unknown_count;
                    self.total_jars = total_jars;
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

    fn outcome_icon(outcome: &ModOutcome) -> &'static str {
        match outcome {
            ModOutcome::Updated { .. } => "\u{2b06}",
            ModOutcome::UpToDate { .. } => "\u{2713}",
            ModOutcome::Unavailable { .. } => "\u{2717}",
            ModOutcome::Unknown { .. } => "?",
            ModOutcome::FilteredOut { .. } => "\u{2298}",
            ModOutcome::Failed { .. } => "!",
        }
    }
    fn apply_theme(&mut self, ctx: &egui::Context) {
        if self.settings_dark_mode != self.applied_dark_mode {
            if self.settings_dark_mode {
                ctx.set_visuals(egui::Visuals::dark());
            } else {
                ctx.set_visuals(egui::Visuals::light());
            }
            self.applied_dark_mode = self.settings_dark_mode;
        }
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
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                                let mods = parse_imported_csv(&content);
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
        if !ctx.wants_keyboard_input() {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::CTRL, egui::Key::R) {
                    self.active_tab = Tab::Scan;
                    let dir = PathBuf::from(&self.mods_dir_input);
                    self.identified_mods.clear();
                    self.unknown_count = 0;
                    self.total_jars = 0;
                    self.log_messages.clear();
                    self.worker.send_command(GuiCommand::ScanMods(dir));
                }
                if i.consume_key(egui::Modifiers::CTRL, egui::Key::U) {
                    self.active_tab = Tab::Updates;
                    self.update_outcomes.clear();
                    self.candidates_count = 0;
                    self.summary = None;
                    self.log_messages.clear();
                    self.sync_config_to_worker();
                    self.worker.send_command(GuiCommand::CheckUpdates);
                }
            });
        }

        // ── Top tab bar ──────────────────────────────────────────────────
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for tab in &[Tab::Scan, Tab::Updates, Tab::Settings, Tab::Rollback] {
                    ui.selectable_value(&mut self.active_tab, *tab, tab.label());
                }
            });
            // Show update banner if a newer version is available.
            if let Some(ref info) = self.app_update_info
                && info.is_newer
                && !self.update_banner_dismissed
            {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        format!(
                            "Anvil {} is available! You are running {}.",
                            info.latest_version, info.current_version
                        ),
                    );
                    ui.hyperlink_to("Download", &info.url);
                    if ui.small_button("Dismiss").clicked() {
                        self.update_banner_dismissed = true;
                    }
                });
            }
        });

        // ── Central content ──────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| match self.active_tab {
            Tab::Scan => self.render_scan_tab(ui),
            Tab::Updates => self.render_updates_tab(ui),
            Tab::Settings => self.render_settings_tab(ui),
            Tab::Rollback => self.render_rollback_tab(ui),
        });

        // ── Confirmation dialog ──────────────────────────────────────────
        if self.confirm_dialog.is_some() {
            let question = self.confirm_dialog.as_ref().unwrap().question.clone();
            let mut closed = false;
            let mut answer = false;

            egui::Window::new("Confirm")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(&question);
                    ui.horizontal(|ui| {
                        if ui.button("Yes").clicked() {
                            answer = true;
                            closed = true;
                        }
                        if ui.button("No").clicked() {
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

        // ── Bottom status bar ────────────────────────────────────────────
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(ref phase) = self.current_phase {
                    ui.label(format!(
                        "{}: {}/{}",
                        phase.label, self.progress_current, self.progress_total
                    ));
                    if self.progress_total > 0 {
                        let frac = self.progress_current as f32 / self.progress_total as f32;
                        let bar = egui::ProgressBar::new(frac).desired_width(200.0);
                        ui.add(bar);
                    }
                    if ui.button("Cancel").clicked() {
                        self.worker.send_command(GuiCommand::Cancel);
                    }
                } else if self.worker_busy {
                    ui.spinner();
                    ui.label("Working...");
                } else {
                    ui.label("Idle");
                }

                if let Some(last) = self.log_messages.last() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(&last.message)
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                    });
                }
            });
        });

        // Request repaint if worker is busy (for smooth progress updates).
        if self.worker_busy {
            ctx.request_repaint();
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
        ui.heading("Updates");

        // Target game version & loader — live here so users see what
        // they're updating to without switching to the Settings tab.
        ui.horizontal(|ui| {
            ui.label("Game version:");
            egui::ComboBox::from_id_salt("updates_game_version")
                .width(100.0)
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

            ui.label("Loader:");
            egui::ComboBox::from_id_salt("updates_loader")
                .width(90.0)
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

        // Sync settings to shared config on every frame so the worker
        // always has the latest values.
        self.sync_config_to_worker();

        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.worker_busy,
                    egui::Button::new("Check for Updates"),
                )
                .clicked()
            {
                self.update_outcomes.clear();
                self.candidates_count = 0;
                self.summary = None;
                self.log_messages.clear();
                self.worker.send_command(GuiCommand::CheckUpdates);
            }

            let can_download = self.candidates_count > 0 && !self.worker_busy;
            if ui
                .add_enabled(
                    can_download,
                    egui::Button::new(format!(
                        "Download {} Update(s)",
                        self.candidates_count
                    )),
                )
                .clicked()
            {
                self.update_outcomes.clear();
                self.summary = None;
                self.worker.send_command(GuiCommand::DownloadUpdates);
            }

            if self.worker_busy {
                ui.spinner();
            }
        });

        ui.separator();

        // Summary
        if let Some(ref summary) = self.summary {
            ui.group(|ui| {
                ui.label(format!("Total JARs: {}", summary.total_jars));
                ui.label(format!("Identified: {}", summary.identified));
                ui.label(format!("Updates applied: {}", summary.updates_applied));
                ui.label(format!("Up-to-date: {}", summary.up_to_date));
                ui.label(format!("Unavailable: {}", summary.unavailable));
                ui.label(format!("Skipped: {}", summary.skipped));
                ui.label(format!("Failed: {}", summary.failed));
            });
        }

        ui.separator();

        // Show what the check was performed with.
        ui.horizontal(|ui| {
            ui.label("Checking with:");
            let gv = self.settings_game_version.as_str();
            let ld = self.settings_loader.as_str();
            if gv.is_empty() && ld.is_empty() {
                ui.label("(auto-detect per mod)");
            } else {
                if !gv.is_empty() {
                    ui.label(format!("MC {gv}"));
                }
                if !ld.is_empty() {
                    ui.label(format!("/ {ld}"));
                }
            }
            if let Some(ref summary) = self.summary {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!(
                        "{} available update(s)",
                        summary.updates_applied
                    ));
                });
            }
        });

        if self.update_outcomes.is_empty() {
            ui.label("No results yet. Click 'Check for Updates' to scan for updates.");
        } else {
            let available_height = ui.available_height().max(150.0);
            egui::ScrollArea::vertical()
                .id_salt("updates_outcomes")
                .max_height(available_height)
                .show(ui, |ui| {
                    egui_extras::TableBuilder::new(ui)
                        .striped(true)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(egui_extras::Column::exact(24.0))
                        .column(egui_extras::Column::remainder().at_least(120.0))
                        .column(egui_extras::Column::remainder().at_least(80.0))
                        .column(egui_extras::Column::remainder().at_least(80.0))
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.strong("");
                            });
                            header.col(|ui| {
                                ui.strong("Mod");
                            });
                            header.col(|ui| {
                                ui.strong("Cur. Version");
                            });
                            header.col(|ui| {
                                ui.strong("Latest Version");
                            });
                        })
                        .body(|body| {
                            body.rows(18.0, self.update_outcomes.len(), |mut row| {
                                let idx = row.index();
                                if let Some(outcome) = self.update_outcomes.get(idx) {
                                    row.col(|ui| {
                                        ui.label(Self::outcome_icon(outcome));
                                    });
                                    row.col(|ui| {
                                        render_outcome_label(ui, outcome);
                                    });
                                    row.col(|ui| {
                                        ui.label(outcome_current(outcome));
                                    });
                                    row.col(|ui| {
                                        ui.label(outcome_latest(outcome));
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
            self.save_settings();
            self.log_messages.push(LogEntry {
                message: "Settings saved.".into(),
            });
        }
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
    }

    fn save_settings(&self) {
        use crate::paths;
        use crate::types::Config;

        let config_path = paths::config_dir().join("config.toml");
        let cfg = Config {
            mods_dir: Some(PathBuf::from(&self.mods_dir_input)),
            backup: Some(self.settings_backup),
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
            include: if self.settings_include.trim().is_empty() {
                None
            } else {
                Some(
                    self.settings_include
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect(),
                )
            },
            exclude: if self.settings_exclude.trim().is_empty() {
                None
            } else {
                Some(
                    self.settings_exclude
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect(),
                )
            },
            max_updates: self.settings_max_updates.parse::<usize>().ok(),
            verbose: None,
            quiet: None,
            dry_run: None,
            confirm: Some(self.settings_confirm),
            changelog: Some(self.settings_changelog),
            dark_mode: Some(self.settings_dark_mode),
        };

        if let Some(parent) = config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match toml::to_string_pretty(&cfg) {
            Ok(content) => {
                let _ = std::fs::write(&config_path, content);
            }
            Err(e) => {
                tracing::error!("failed to serialize config: {}", e);
            }
        }
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
            ui.label(outcome_label(outcome));
        }
    }
}

fn outcome_label(outcome: &ModOutcome) -> &str {
    match outcome {
        ModOutcome::Updated { slug, .. }
        | ModOutcome::UpToDate { slug, .. }
        | ModOutcome::Unavailable { slug, .. } => slug,
        ModOutcome::Unknown { filename } => filename,
        ModOutcome::FilteredOut { filename, .. } => filename,
        ModOutcome::Failed { filename, .. } => filename,
    }
}

fn outcome_current(outcome: &ModOutcome) -> &str {
    match outcome {
        ModOutcome::Updated { old_version, .. } => old_version,
        ModOutcome::UpToDate { version, .. } => version,
        ModOutcome::Unavailable {
            current_version, ..
        } => current_version,
        _ => "\u{2014}",
    }
}

fn outcome_latest(outcome: &ModOutcome) -> &str {
    match outcome {
        ModOutcome::Updated { new_version, .. } => new_version,
        ModOutcome::UpToDate { version, .. } => version,
        _ => "\u{2014}",
    }
}

/// Parse an imported CSV file with header row and project_id column.
///
/// Expected format (matching what export generates):
/// `Filename,Name,Version,Project ID,Loader,Game Versions`
/// Each row is parsed, the Project ID column is required.
fn parse_imported_csv(content: &str) -> Vec<ImportedMod> {
    let mut mods = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue; // Skip header and blank lines
        }
        // Simple comma split — handles basic CSV without quoting (the export
        // format quotes everything, but for import we accept both).
        let fields: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
        if fields.len() >= 4 {
            let project_id = fields[3].to_string();
            if !project_id.is_empty() {
                mods.push(ImportedMod {
                    project_id,
                    slug: None,
                    name: if fields.len() > 1 && !fields[1].is_empty() {
                        Some(fields[1].to_string())
                    } else {
                        None
                    },
                    version_number: if fields.len() > 2 && !fields[2].is_empty() {
                        Some(fields[2].to_string())
                    } else {
                        None
                    },
                    loaders: if fields.len() > 4 && !fields[4].is_empty() {
                        fields[4].split(", ").map(|s| s.to_string()).collect()
                    } else {
                        vec![]
                    },
                    game_versions: if fields.len() > 5 && !fields[5].is_empty() {
                        fields[5].split(", ").map(|s| s.to_string()).collect()
                    } else {
                        vec![]
                    },
                });
            }
        }
    }
    mods
}
