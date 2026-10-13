//! Theme token tests — pure assertions on design system constants.

use super::*;

#[test]
fn dark_and_light_bg_differ() {
    assert_ne!(BG_BASE, LIGHT_BG_BASE);
    assert_ne!(BG_SURFACE, LIGHT_BG_SURFACE);
}

#[test]
fn accent_is_green() {
    // Modrinth green: high green channel, lower red/blue.
    assert!(ACCENT.g() > ACCENT.r(), "accent green channel should dominate red");
    assert!(ACCENT.g() > ACCENT.b(), "accent green channel should dominate blue");
    assert!(ACCENT.g() > 150, "accent green channel should be bright");
}

#[test]
fn error_is_red() {
    assert!(ERROR.r() > ERROR.g(), "error red channel should dominate green");
    assert!(ERROR.r() > ERROR.b(), "error red channel should dominate blue");
}

#[test]
fn warning_is_amber() {
    assert!(WARNING.r() > 150, "warning red should be bright");
    assert!(WARNING.g() > 100, "warning green should be present");
    assert!(WARNING.b() < 80, "warning blue should be low");
}

#[test]
fn badge_fill_has_alpha() {
    let fill = badge_fill(ACCENT);
    assert!(fill.a() > 0, "badge fill should have some alpha");
    assert!(fill.a() < 255, "badge fill should be semi-transparent");
}

#[test]
fn spacing_grid_is_consistent() {
    assert_eq!(SPACE_SM, 8.0);
    assert_eq!(SPACE_MD, 16.0);
    assert_eq!(SPACE_LG, 24.0);
    // XS and XL should be multiples of 4.
    assert_eq!(SPACE_XS, 4.0);
    assert_eq!(SPACE_XL, 32.0);
}

#[test]
fn radii_are_ordered() {
    assert!(RADIUS_INPUT <= RADIUS_BUTTON);
    assert!(RADIUS_BUTTON <= RADIUS_CARD);
    assert!(RADIUS_PILL >= RADIUS_CARD);
}

#[test]
fn font_sizes_are_ordered() {
    assert!(FONT_CAPTION < FONT_BODY);
    assert!(FONT_BODY < FONT_SUBHEADING);
    assert!(FONT_SUBHEADING < FONT_HEADING);
}

#[test]
fn apply_theme_dark_and_light_differ() {
    let ctx_dark = egui::Context::default();
    apply_theme(&ctx_dark, true);
    let dark_visuals = ctx_dark.global_style().visuals.clone();

    let ctx_light = egui::Context::default();
    apply_theme(&ctx_light, false);
    let light_visuals = ctx_light.global_style().visuals.clone();

    assert_ne!(
        dark_visuals.panel_fill,
        light_visuals.panel_fill,
        "dark and light panel_fill should differ"
    );
}
