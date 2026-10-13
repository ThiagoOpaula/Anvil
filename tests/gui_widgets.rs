//! GUI widget tests using egui_kittest.
//!
//! These tests verify that widgets render without panicking and produce
//! accessible UI elements that can be queried.

#![cfg(feature = "gui")]

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

// ── Badge tests ───────────────────────────────────────────────────────────

#[test]
fn status_badge_renders_updated() {
    let mut harness = Harness::new_ui(|ui| {
        anvil::gui::widgets::badge::StatusBadge::new(
            anvil::gui::widgets::badge::StatusKind::Updated,
        )
        .show(ui);
    });
    harness.run();
    // Badge should exist in the accessibility tree
    assert!(harness.query_by_label("Updated").is_some());
}

#[test]
fn status_badge_renders_all_kinds() {
    for kind in [
        anvil::gui::widgets::badge::StatusKind::Updated,
        anvil::gui::widgets::badge::StatusKind::UpToDate,
        anvil::gui::widgets::badge::StatusKind::Failed,
        anvil::gui::widgets::badge::StatusKind::Warning,
        anvil::gui::widgets::badge::StatusKind::Unknown,
        anvil::gui::widgets::badge::StatusKind::Filtered,
    ] {
        let label = kind.label();
        let mut harness = Harness::new_ui(|ui| {
            anvil::gui::widgets::badge::StatusBadge::new(kind).show(ui);
        });
        harness.run();
        assert!(
            harness.query_by_label(label).is_some(),
            "Badge '{}' should render",
            label
        );
    }
}

#[test]
fn status_badge_custom_text() {
    let mut harness = Harness::new_ui(|ui| {
        anvil::gui::widgets::badge::StatusBadge::new(
            anvil::gui::widgets::badge::StatusKind::Updated,
        )
        .with_text("Custom Label")
        .show(ui);
    });
    harness.run();
    assert!(harness.query_by_label("Custom Label").is_some());
}

// ── Button tests ──────────────────────────────────────────────────────────

#[test]
fn primary_button_clickable() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let clicked = Rc::new(RefCell::new(false));
    let clicked_clone = clicked.clone();
    let mut harness = Harness::new_ui(move |ui| {
        if anvil::gui::widgets::buttons::primary_button(ui, "Click Me").clicked() {
            *clicked_clone.borrow_mut() = true;
        }
    });
    harness.run();
    if let Some(node) = harness.query_by_label("Click Me") {
        node.click();
        harness.run();
    }
    assert!(*clicked.borrow(), "Primary button should be clickable");
}

#[test]
fn secondary_button_clickable() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let clicked = Rc::new(RefCell::new(false));
    let clicked_clone = clicked.clone();
    let mut harness = Harness::new_ui(move |ui| {
        if anvil::gui::widgets::buttons::secondary_button(ui, "Cancel").clicked() {
            *clicked_clone.borrow_mut() = true;
        }
    });
    harness.run();
    if let Some(node) = harness.query_by_label("Cancel") {
        node.click();
        harness.run();
    }
    assert!(*clicked.borrow(), "Secondary button should be clickable");
}

// ── Card tests ────────────────────────────────────────────────────────────

#[test]
fn section_card_renders_title_and_content() {
    let mut harness = Harness::new_ui(|ui| {
        anvil::gui::widgets::card::SectionCard::new("My Section").show(ui, |ui| {
            ui.label("Inner content");
        });
    });
    harness.run();
    assert!(harness.query_by_label("My Section").is_some());
    assert!(harness.query_by_label("Inner content").is_some());
}

// ── EmptyState tests ──────────────────────────────────────────────────────

#[test]
fn empty_state_shows_title_and_subtitle() {
    let mut harness = Harness::new_ui(|ui| {
        anvil::gui::widgets::empty_state::EmptyState::new("\u{26cf}", "Nothing here")
            .subtitle("Try scanning for mods.")
            .show(ui);
    });
    harness.run();
    assert!(harness.query_by_label("Nothing here").is_some());
    assert!(harness.query_by_label("Try scanning for mods.").is_some());
}

#[test]
fn empty_state_shows_cta() {
    let mut harness = Harness::new_ui(|ui| {
        anvil::gui::widgets::empty_state::EmptyState::new("\u{26cf}", "Empty")
            .cta("Scan Now")
            .show(ui);
    });
    harness.run();
    // The CTA is a button; verify it exists
    assert!(harness.query_by_label("Scan Now").is_some());
}

// ── StatTile tests ────────────────────────────────────────────────────────

#[test]
fn stat_tile_shows_value_and_label() {
    let mut harness = Harness::new_ui(|ui| {
        anvil::gui::widgets::stat_tiles::StatTile::new("42", "Total Mods").show(ui);
    });
    harness.run();
    assert!(harness.query_by_label("42").is_some());
    assert!(harness.query_by_label("Total Mods").is_some());
}

// ── Sidebar tests ─────────────────────────────────────────────────────────

#[test]
fn sidebar_shows_all_nav_items() {
    let mut harness = Harness::new_ui(|ui| {
        let sidebar = anvil::gui::widgets::sidebar::Sidebar {
            active: anvil::gui::state::Tab::Scan,
            version: "0.6.0",
            has_update: false,
            dark_mode: true,
        };
        sidebar.show(ui);
    });
    harness.run();
    // All four tabs should have labels in the sidebar
    assert!(harness.query_by_label("Scan & Identify").is_some());
    assert!(harness.query_by_label("Updates").is_some());
    assert!(harness.query_by_label("Rollback").is_some());
    assert!(harness.query_by_label("Settings").is_some());
}

#[test]
fn sidebar_shows_version() {
    let mut harness = Harness::new_ui(|ui| {
        let sidebar = anvil::gui::widgets::sidebar::Sidebar {
            active: anvil::gui::state::Tab::Scan,
            version: "1.2.3",
            has_update: false,
            dark_mode: true,
        };
        sidebar.show(ui);
    });
    harness.run();
    assert!(harness.query_by_label("v1.2.3").is_some());
}

// ── Theme tests ───────────────────────────────────────────────────────────

#[test]
fn theme_applies_without_panic() {
    let ctx = egui::Context::default();
    anvil::gui::theme::apply_theme(&ctx, true);
    anvil::gui::theme::apply_theme(&ctx, false);
    // Both should complete without panicking
}

// ── Tab label tests ───────────────────────────────────────────────────────

#[test]
fn tab_labels_and_icons_distinct() {
    use anvil::gui::state::Tab;
    let tabs = [Tab::Scan, Tab::Updates, Tab::Settings, Tab::Rollback];
    let labels: Vec<_> = tabs.iter().map(|t| t.label()).collect();
    let icons: Vec<_> = tabs.iter().map(|t| t.icon()).collect();

    // All labels unique
    let mut unique_labels = labels.clone();
    unique_labels.sort();
    unique_labels.dedup();
    assert_eq!(unique_labels.len(), labels.len(), "Tab labels should be unique");

    // All icons unique
    let mut unique_icons = icons.clone();
    unique_icons.sort();
    unique_icons.dedup();
    assert_eq!(unique_icons.len(), icons.len(), "Tab icons should be unique");
}
