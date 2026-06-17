//! `GuiProgress` — a `ProgressRenderer` implementation for the egui GUI.
//!
//! All trait methods send events through a `crossbeam::Sender<WorkerEvent>`
//! so the GUI can update widgets on the next frame. The `confirm()` method
//! blocks the worker thread until the GUI user clicks Yes / No.
//!
//! This lives on the **worker thread** — all methods are called from async
//! code running on the tokio runtime.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::types::ProgressRenderer;

use super::worker::{ConfirmState, WorkerEvent};

/// A `ProgressRenderer` that broadcasts progress to the GUI via a channel.
pub struct GuiProgress {
    event_tx: crossbeam::channel::Sender<WorkerEvent>,
    confirm_state: Arc<Mutex<Option<ConfirmState>>>,
    cancel_flag: Arc<AtomicBool>,
    /// Accumulated progress counter for the current phase.
    current: Arc<std::sync::atomic::AtomicU64>,
}

impl GuiProgress {
    /// Create a new `GuiProgress` wired to the given event channel and
    /// shared confirmation / cancellation state.
    pub fn new(
        event_tx: crossbeam::channel::Sender<WorkerEvent>,
        confirm_state: Arc<Mutex<Option<ConfirmState>>>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            event_tx,
            confirm_state,
            cancel_flag,
            current: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
}

impl ProgressRenderer for GuiProgress {
    fn start_phase(&self, label: &str, total: u64) {
        self.current.store(0, Ordering::SeqCst);
        let _ = self.event_tx.send(WorkerEvent::PhaseStarted {
            label: label.to_string(),
            total,
        });
    }

    fn increment(&self, n: u64) {
        let current = self.current.fetch_add(n, Ordering::SeqCst) + n;
        let _ = self.event_tx.send(WorkerEvent::PhaseProgress { current });
    }

    fn finish_phase(&self) {
        let _ = self.event_tx.send(WorkerEvent::PhaseFinished);
    }

    fn print_table(&self, headers: &[&str], rows: &[Vec<String>]) {
        let _ = self.event_tx.send(WorkerEvent::TableReady {
            headers: headers.iter().map(|h| h.to_string()).collect(),
            rows: rows.to_vec(),
        });
    }

    fn print_changelog(&self, slug: &str, version: &str, changelog: &str) {
        let _ = self.event_tx.send(WorkerEvent::ChangelogReady {
            slug: slug.to_string(),
            version: version.to_string(),
            changelog: changelog.to_string(),
        });
    }

    fn report_outcomes(&self, outcomes: &[crate::types::ModOutcome]) {
        let _ = self
            .event_tx
            .send(WorkerEvent::OutcomesReady(outcomes.to_vec()));
    }

    fn confirm(&self, question: &str) -> bool {
        let (tx, rx) = tokio::sync::oneshot::channel();

        // Notify the GUI that a confirmation dialog is needed.
        let _ = self.event_tx.send(WorkerEvent::ConfirmRequest {
            question: question.to_string(),
        });

        // Store the sender so the GUI can respond.
        let state = ConfirmState {
            question: question.to_string(),
            reply_tx: tx,
        };
        *self.confirm_state.lock().expect("lock") = Some(state);

        // Block until the GUI user clicks a button.
        // Wrapping in `block_in_place` tells tokio this is a deliberate
        // synchronous block — the worker thread has nothing else to do
        // until the user responds.
        tokio::task::block_in_place(|| rx.blocking_recv().unwrap_or(false))
    }

    fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }
}
