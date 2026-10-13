//! Reusable UI widgets for the Anvil GUI.
//!
//! All widgets are pure render helpers — they take `&mut egui::Ui` and
//! return `egui::Response` or `()`. No worker access, no state mutation.

pub mod badge;
pub mod buttons;
pub mod card;
pub mod empty_state;
pub mod sidebar;
pub mod stat_tiles;
