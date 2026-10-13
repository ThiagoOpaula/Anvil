//! Anvil Dark theme — design tokens and egui style configuration.
//!
//! Modern Minecraft-ecosystem palette (Modrinth / Prism Launcher inspired).
//! All colors, spacing, and radii live here as constants so tests can assert
//! values and future themes can swap in cleanly.

use egui::{Color32, CornerRadius, FontFamily, Stroke, TextStyle, Vec2};

// ── Palette (dark) ────────────────────────────────────────────────────────

pub const BG_BASE: Color32 = Color32::from_rgb(0x0F, 0x0F, 0x14);
pub const BG_SURFACE: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x24);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(0x25, 0x25, 0x38);
pub const BORDER: Color32 = Color32::from_rgb(0x33, 0x33, 0x48);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(0xE8, 0xE8, 0xF0);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0x9A, 0x9A, 0xB0);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x6B, 0x6B, 0x80);

pub const ACCENT: Color32 = Color32::from_rgb(0x1B, 0xD9, 0x6A);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x2F, 0xE8, 0x7A);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x14, 0x9B, 0x4D);
pub const WARNING: Color32 = Color32::from_rgb(0xFF, 0xB3, 0x00);
pub const ERROR: Color32 = Color32::from_rgb(0xFF, 0x52, 0x52);
pub const INFO: Color32 = Color32::from_rgb(0x64, 0xB5, 0xF6);

// ── Palette (light) ───────────────────────────────────────────────────────

pub const LIGHT_BG_BASE: Color32 = Color32::from_rgb(0xF4, 0xF5, 0xF7);
pub const LIGHT_BG_SURFACE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const LIGHT_BG_ELEVATED: Color32 = Color32::from_rgb(0xEC, 0xED, 0xF0);
pub const LIGHT_BORDER: Color32 = Color32::from_rgb(0xD8, 0xDA, 0xE0);
pub const LIGHT_TEXT_PRIMARY: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x24);
pub const LIGHT_TEXT_SECONDARY: Color32 = Color32::from_rgb(0x5A, 0x5A, 0x70);
pub const LIGHT_TEXT_MUTED: Color32 = Color32::from_rgb(0x8A, 0x8A, 0x9A);
pub const LIGHT_ACCENT: Color32 = Color32::from_rgb(0x14, 0x9B, 0x4D);

// ── Radii ─────────────────────────────────────────────────────────────────

pub const RADIUS_CARD: f32 = 8.0;
pub const RADIUS_BUTTON: f32 = 6.0;
pub const RADIUS_INPUT: f32 = 4.0;
pub const RADIUS_PILL: f32 = 999.0;

// ── Spacing (8px grid) ────────────────────────────────────────────────────

pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 16.0;
pub const SPACE_LG: f32 = 24.0;
pub const SPACE_XL: f32 = 32.0;

// ── Typography ────────────────────────────────────────────────────────────

pub const FONT_HEADING: f32 = 22.0;
pub const FONT_SUBHEADING: f32 = 16.0;
pub const FONT_BODY: f32 = 13.0;
pub const FONT_CAPTION: f32 = 11.0;
pub const FONT_MONO: f32 = 12.0;

// ── Theme-aware color helpers ─────────────────────────────────────────────

/// Get the appropriate background surface color for the current theme.
pub fn surface_color(dark: bool) -> Color32 {
    if dark { BG_SURFACE } else { LIGHT_BG_SURFACE }
}

/// Get the appropriate elevated background color for the current theme.
pub fn elevated_color(dark: bool) -> Color32 {
    if dark { BG_ELEVATED } else { LIGHT_BG_ELEVATED }
}

/// Get the appropriate border color for the current theme.
pub fn border_color(dark: bool) -> Color32 {
    if dark { BORDER } else { LIGHT_BORDER }
}

/// Get the appropriate primary text color for the current theme.
pub fn text_primary_color(dark: bool) -> Color32 {
    if dark { TEXT_PRIMARY } else { LIGHT_TEXT_PRIMARY }
}

/// Get the appropriate secondary text color for the current theme.
pub fn text_secondary_color(dark: bool) -> Color32 {
    if dark { TEXT_SECONDARY } else { LIGHT_TEXT_SECONDARY }
}

/// Get the appropriate muted text color for the current theme.
pub fn text_muted_color(dark: bool) -> Color32 {
    if dark { TEXT_MUTED } else { LIGHT_TEXT_MUTED }
}

// ── Status badge fill (alpha composite helper) ────────────────────────────

/// Returns a badge background fill for the given semantic color.
/// Uses ~55% opacity for strong background visibility.
pub fn badge_fill(base: Color32) -> Color32 {
    Color32::from_rgba_premultiplied(base.r(), base.g(), base.b(), 140) // ~55%
}

/// Returns a readable text color for badges.
/// Always near-white on dark theme for guaranteed contrast on colored fills.
pub fn badge_text(base: Color32) -> Color32 {
    // On semi-transparent colored fills, near-white text is the most readable.
    // For very light/bright fills, fall back to dark text.
    let luminance = 0.299 * base.r() as f32 + 0.587 * base.g() as f32 + 0.114 * base.b() as f32;
    if luminance > 180.0 {
        // Light color — use dark text
        Color32::from_rgb(30, 30, 40)
    } else {
        // Dark/saturated color — use near-white text
        Color32::from_rgb(235, 235, 245)
    }
}

// ── Theme application ─────────────────────────────────────────────────────

/// Apply the Anvil theme to the egui context.
///
/// When `dark` is true, uses the dark palette (default). Otherwise uses the
/// light palette.
pub fn apply_theme(ctx: &egui::Context, dark: bool) {
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    let (bg_base, bg_surface, bg_elevated, border, text_primary, text_secondary, accent) =
        if dark {
            (BG_BASE, BG_SURFACE, BG_ELEVATED, BORDER, TEXT_PRIMARY, TEXT_SECONDARY, ACCENT)
        } else {
            (
                LIGHT_BG_BASE,
                LIGHT_BG_SURFACE,
                LIGHT_BG_ELEVATED,
                LIGHT_BORDER,
                LIGHT_TEXT_PRIMARY,
                LIGHT_TEXT_SECONDARY,
                LIGHT_ACCENT,
            )
        };

    visuals.panel_fill = bg_surface;
    visuals.window_fill = bg_base;
    visuals.extreme_bg_color = bg_elevated;
    visuals.faint_bg_color = bg_elevated;
    visuals.selection.bg_fill = accent.linear_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, accent);

    visuals.widgets.noninteractive.bg_fill = bg_surface;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text_secondary);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(RADIUS_INPUT as u8);

    visuals.widgets.inactive.bg_fill = bg_elevated;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, text_primary);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(RADIUS_BUTTON as u8);

    visuals.widgets.hovered.bg_fill = bg_elevated.linear_multiply(1.15);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, text_primary);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(RADIUS_BUTTON as u8);

    visuals.widgets.active.bg_fill = accent.linear_multiply(0.25);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, accent);
    visuals.widgets.active.corner_radius = CornerRadius::same(RADIUS_BUTTON as u8);

    visuals.window_corner_radius = CornerRadius::same(RADIUS_CARD as u8);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS_CARD as u8);
    visuals.slider_trailing_fill = true;

    ctx.set_visuals(visuals);

    // Typography and spacing overrides.
    let mut style = (*ctx.global_style()).clone();

    if let Some(heading) = style.text_styles.get_mut(&TextStyle::Heading) {
        heading.size = FONT_HEADING;
        heading.family = FontFamily::Proportional;
    }
    if let Some(body) = style.text_styles.get_mut(&TextStyle::Body) {
        body.size = FONT_BODY;
    }
    if let Some(button) = style.text_styles.get_mut(&TextStyle::Button) {
        button.size = FONT_BODY;
    }
    if let Some(small) = style.text_styles.get_mut(&TextStyle::Small) {
        small.size = FONT_CAPTION;
    }
    if let Some(monospace) = style.text_styles.get_mut(&TextStyle::Monospace) {
        monospace.size = FONT_MONO;
    }

    style.spacing.item_spacing = Vec2::new(SPACE_SM, SPACE_SM);
    style.spacing.button_padding = Vec2::new(SPACE_MD, SPACE_SM + 2.0);
    style.spacing.indent = SPACE_LG;
    style.spacing.interact_size.y = 28.0;

    ctx.set_global_style(style);
}

// ── Theme tests ───────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "tests/test_gui_theme.rs"]
mod tests;
