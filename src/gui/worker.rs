//! Background worker thread that runs the async `updater::run()` pipeline.
//!
//! The GUI (eframe) owns the main thread and runs a synchronous event loop.
//! This module spawns a `std::thread` with its own `tokio::runtime::Runtime`
//! so the async Modrinth API calls don't block the UI.
//!
//! Communication pattern:
//! ```text
//! GUI ──GuiCommand──> cmd_tx ──> Worker thread (tokio)
//! GUI <──WorkerEvent── event_rx <── Worker thread
//! GUI <──ConfirmState── confirm_state (Arc<Mutex>) ──> Worker thread
//! GUI <──config (Arc<Mutex<ResolvedConfig>>) ──> Worker thread
//! ```

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::api::ModrinthApi;
use crate::cache::ApiCache;
use crate::config::ResolvedConfig;
use crate::scanner;
use crate::types::{ApiClient, IdentifiedMod, ModOutcome, ProgressRenderer, RunSummary};
use crate::updater;

use super::progress::GuiProgress;

// ── Message types ─────────────────────────────────────────────────────────

/// Commands the GUI sends to the worker.
#[derive(Debug)]
pub enum GuiCommand {
    /// Scan a mods directory: find, hash, and identify JARs.
    ScanMods(PathBuf),
    /// Check for updates without downloading (dry-run mode).
    CheckUpdates,
    /// Download available updates.
    DownloadUpdates,
    /// Restore mods from the latest backup.
    Rollback,
    /// Fetch the list of Minecraft game versions from the API.
    FetchGameVersions,
    /// Fetch the list of mod loaders from the API.
    FetchLoaders,
    /// Request the worker to stop at the next phase boundary.
    Cancel,
}

/// Events the worker sends back to the GUI.
#[derive(Debug)]
pub enum WorkerEvent {
    /// A named processing phase has started (e.g. "Hashing").
    PhaseStarted { label: String, total: u64 },
    /// Progress within the current phase advanced to `current`.
    PhaseProgress { current: u64 },
    /// The current phase is complete.
    PhaseFinished,
    /// A results table is ready to display.
    TableReady {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// A changelog was printed for a mod.
    ChangelogReady {
        slug: String,
        version: String,
        changelog: String,
    },
    /// A run summary is available.
    SummaryReady(RunSummary),
    /// Structured per-mod outcomes from the pipeline.
    OutcomesReady(Vec<ModOutcome>),
    /// The worker needs user confirmation (yes/no).
    ConfirmRequest { question: String },
    /// A log message from the pipeline.
    Log { message: String },
    /// Scan complete with identified mods.
    ScanComplete {
        identified: Vec<IdentifiedMod>,
        unknown_count: usize,
        total_jars: usize,
    },
    /// Update check complete with per-mod outcomes.
    UpdatesChecked {
        outcomes: Vec<ModOutcome>,
        candidates_count: usize,
    },
    /// Downloads finished.
    DownloadsComplete { summary: RunSummary },
    /// Rollback finished.
    RollbackComplete { count: usize },
    /// An error occurred.
    Error(String),
    /// The worker has finished the current command and is idle.
    Done,
}

/// Shared state for the confirmation dialog bridge.
///
/// When `ProgressRenderer::confirm()` is called (on the worker thread), it
/// writes a `ConfirmState` here and blocks. The GUI polls for this state
/// each frame, renders a modal dialog, and responds via the `oneshot` sender.
pub struct ConfirmState {
    pub question: String,
    pub reply_tx: tokio::sync::oneshot::Sender<bool>,
}

// ── Worker handle ─────────────────────────────────────────────────────────

/// Owns the worker thread and its communication channels.
pub struct WorkerHandle {
    cmd_tx: crossbeam::channel::Sender<GuiCommand>,
    event_rx: crossbeam::channel::Receiver<WorkerEvent>,
    /// Shared confirm state — written by worker, polled by GUI.
    pub confirm_state: Arc<Mutex<Option<ConfirmState>>>,
    /// Set to `true` by the GUI to cancel the current operation.
    pub cancel_flag: Arc<std::sync::atomic::AtomicBool>,
    worker_thread: Option<std::thread::JoinHandle<()>>,
}

impl WorkerHandle {
    /// Send a command to the worker (non-blocking).
    pub fn send_command(&self, cmd: GuiCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    /// Drain all pending events from the worker (non-blocking).
    pub fn try_recv_events(&self) -> Vec<WorkerEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        // Signal cancel so the worker stops quickly.
        self.cancel_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // Drop the command sender so the worker's loop exits.
        if let Some(handle) = self.worker_thread.take() {
            let _ = handle.join();
        }
    }
}

// ── Spawn ─────────────────────────────────────────────────────────────────

/// Spawn the background worker thread and return a handle to it.
///
/// The worker owns its own `tokio::Runtime`, `ModrinthApi`, and `ApiCache`.
/// Config is shared via `Arc<Mutex<ResolvedConfig>>` — the GUI updates it
/// when settings change, and the worker clones it before running each command.
/// No worker re-spawning is ever needed.
pub fn spawn_worker(shared_config: Arc<Mutex<ResolvedConfig>>) -> WorkerHandle {
    let (cmd_tx, cmd_rx) = crossbeam::channel::unbounded::<GuiCommand>();
    let (event_tx, event_rx) = crossbeam::channel::unbounded::<WorkerEvent>();
    let confirm_state: Arc<Mutex<Option<ConfirmState>>> = Arc::new(Mutex::new(None));
    let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let worker_confirm = Arc::clone(&confirm_state);
    let worker_cancel = Arc::clone(&cancel_flag);

    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("failed to create tokio runtime for worker");

        rt.block_on(async {
            let api = match ModrinthApi::new() {
                Ok(a) => a,
                Err(e) => {
                    let _ = event_tx.send(WorkerEvent::Error(format!("API init: {}", e)));
                    return;
                }
            };
            let cache = ApiCache::new();

            // Main command loop.
            while let Ok(cmd) = cmd_rx.recv() {
                // Reset cancel flag before each command.
                worker_cancel.store(false, std::sync::atomic::Ordering::SeqCst);

                let progress = GuiProgress::new(
                    event_tx.clone(),
                    Arc::clone(&worker_confirm),
                    Arc::clone(&worker_cancel),
                );

                // Clone the latest config from shared state.
                let config = shared_config
                    .lock()
                    .expect("config lock")
                    .clone();

                match cmd {
                    GuiCommand::Cancel => {
                        worker_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
                        continue;
                    }
                    GuiCommand::FetchGameVersions => {
                        match api.get_game_versions().await {
                            Ok(versions) => {
                                let _ = event_tx.send(WorkerEvent::Log {
                                    message: format!("game_versions:{}", versions.join(",")),
                                });
                            }
                            Err(e) => {
                                let _ = event_tx
                                    .send(WorkerEvent::Error(format!("fetch versions: {}", e)));
                            }
                        }
                        let _ = event_tx.send(WorkerEvent::Done);
                    }
                    GuiCommand::FetchLoaders => {
                        match api.get_loaders().await {
                            Ok(loaders) => {
                                let _ = event_tx.send(WorkerEvent::Log {
                                    message: format!("loaders:{}", loaders.join(",")),
                                });
                            }
                            Err(e) => {
                                let _ = event_tx
                                    .send(WorkerEvent::Error(format!("fetch loaders: {}", e)));
                            }
                        }
                        let _ = event_tx.send(WorkerEvent::Done);
                    }
                    GuiCommand::ScanMods(dir) => {
                        let mut scan_config = config;
                        scan_config.mods_dir = dir;
                        scan_config.dry_run = true;
                        scan_config.confirm = false;
                        run_scan(&scan_config, &api, &cache, &progress, &event_tx).await;
                        let _ = event_tx.send(WorkerEvent::Done);
                    }
                    GuiCommand::CheckUpdates => {
                        let mut dry_config = config;
                        dry_config.dry_run = true;
                        dry_config.confirm = false;
                        run_update_check(&dry_config, &api, &cache, &progress, &event_tx).await;
                        let _ = event_tx.send(WorkerEvent::Done);
                    }
                    GuiCommand::DownloadUpdates => {
                        let mut dl_config = config;
                        dl_config.dry_run = false;
                        dl_config.confirm = true;
                        run_full_update(&dl_config, &api, &cache, &progress, &event_tx).await;
                        let _ = event_tx.send(WorkerEvent::Done);
                    }
                    GuiCommand::Rollback => {
                        match crate::backup::rollback(&config.mods_dir) {
                            Ok(count) => {
                                let _ = event_tx.send(WorkerEvent::RollbackComplete { count });
                            }
                            Err(e) => {
                                let _ =
                                    event_tx.send(WorkerEvent::Error(format!("rollback: {}", e)));
                            }
                        }
                        let _ = event_tx.send(WorkerEvent::Done);
                    }
                }
            }
        });
    });

    WorkerHandle {
        cmd_tx,
        event_rx,
        confirm_state,
        cancel_flag,
        worker_thread: Some(handle),
    }
}

// ── Pipeline runners ───────────────────────────────────────────────────────

async fn run_scan(
    config: &ResolvedConfig,
    api: &dyn ApiClient,
    cache: &ApiCache,
    progress: &dyn ProgressRenderer,
    event_tx: &crossbeam::channel::Sender<WorkerEvent>,
) {
    let jars = match scanner::find_jars(&config.mods_dir) {
        Ok(j) => j,
        Err(e) => {
            let _ = event_tx.send(WorkerEvent::Error(format!("scan: {}", e)));
            return;
        }
    };
    let total_jars = jars.len();
    if jars.is_empty() {
        let _ = event_tx.send(WorkerEvent::Log {
            message: format!("No JAR files found in {}.", config.mods_dir.display()),
        });
        let _ = event_tx.send(WorkerEvent::ScanComplete {
            identified: vec![],
            unknown_count: 0,
            total_jars: 0,
        });
        return;
    }

    progress.start_phase("Hashing", total_jars as u64);
    let mut hashes: Vec<(PathBuf, String)> = Vec::with_capacity(total_jars);
    for jar in &jars {
        match scanner::compute_sha1(jar) {
            Ok(hash) => hashes.push((jar.clone(), hash)),
            Err(e) => {
                tracing::warn!(path = %jar.display(), error = %e, "skipping JAR");
            }
        }
        progress.increment(1);
    }
    progress.finish_phase();

    use futures::stream::{self, StreamExt};
    progress.start_phase("Identifying", hashes.len() as u64);

    let identify_results: Vec<(PathBuf, String, Option<IdentifiedMod>)> =
        stream::iter(hashes)
            .map(|(path, sha1)| {
                async {
                    if let Ok(Some(version)) = cache.get_version(&sha1) {
                        let filename = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let entry = IdentifiedMod {
                            path: path.clone(),
                            sha1: sha1.clone(),
                            filename,
                            current_version: version,
                        };
                        return (path, sha1, Some(entry));
                    }
                    match api.get_version_from_hash(&sha1).await {
                        Ok(Some(version)) => {
                            let _ = cache.set_version(&sha1, &version);
                            let filename = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            let entry = IdentifiedMod {
                                path: path.clone(),
                                sha1: sha1.clone(),
                                filename,
                                current_version: version,
                            };
                            (path, sha1, Some(entry))
                        }
                        Ok(None) => (path, sha1, None),
                        Err(e) => {
                            tracing::debug!(
                                path = %path.display(),
                                error = %e,
                                "API lookup failed"
                            );
                            (path, sha1, None)
                        }
                    }
                }
            })
            .buffer_unordered(4)
            .inspect(|_| progress.increment(1))
            .collect()
            .await;

    progress.finish_phase();

    let mut identified: Vec<IdentifiedMod> = Vec::new();
    let mut unknown_count = 0usize;
    for (_path, _sha1, result) in identify_results {
        match result {
            Some(mod_info) => identified.push(mod_info),
            None => unknown_count += 1,
        }
    }

    let _ = event_tx.send(WorkerEvent::ScanComplete {
        identified,
        unknown_count,
        total_jars,
    });
}

async fn run_update_check(
    config: &ResolvedConfig,
    api: &dyn ApiClient,
    cache: &ApiCache,
    progress: &dyn ProgressRenderer,
    event_tx: &crossbeam::channel::Sender<WorkerEvent>,
) {
    match updater::run(config, api, cache, progress).await {
        Ok(summary) => {
            let _ = event_tx.send(WorkerEvent::SummaryReady(summary));
        }
        Err(crate::error::Error::Cancelled) => {
            let _ = event_tx.send(WorkerEvent::Log {
                message: "Update check cancelled.".into(),
            });
        }
        Err(e) => {
            let _ = event_tx.send(WorkerEvent::Error(format!("update check: {}", e)));
        }
    }
}

async fn run_full_update(
    config: &ResolvedConfig,
    api: &dyn ApiClient,
    cache: &ApiCache,
    progress: &dyn ProgressRenderer,
    event_tx: &crossbeam::channel::Sender<WorkerEvent>,
) {
    match updater::run(config, api, cache, progress).await {
        Ok(summary) => {
            let _ = event_tx.send(WorkerEvent::DownloadsComplete { summary });
        }
        Err(crate::error::Error::Cancelled) => {
            let _ = event_tx.send(WorkerEvent::Log {
                message: "Update cancelled.".into(),
            });
        }
        Err(e) => {
            let _ = event_tx.send(WorkerEvent::Error(format!("update: {}", e)));
        }
    }
}
