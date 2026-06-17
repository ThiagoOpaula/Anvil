//! Anvil GUI — egui/eframe desktop application.
//!
//! Wraps the existing `anvil` library (scanner, updater, API, cache) with
//! a graphical interface. The async pipeline runs on a background worker
//! thread; the GUI polls for events each frame via crossbeam channels.

pub mod app;
pub mod progress;
pub mod tabs;
pub mod worker;

pub use app::AnvilApp;
pub use worker::{spawn_worker, WorkerHandle};
