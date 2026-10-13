//! Styled button helpers — colors adapt to the active theme.

use egui::{Color32, CornerRadius, RichText, Ui};

use crate::gui::theme;

/// Primary action button (accent fill, adapts to theme).
pub fn primary_button(ui: &mut Ui, text: &str) -> egui::Response {
    let accent = if ui.visuals().dark_mode { theme::ACCENT } else { theme::LIGHT_ACCENT };
    let on_accent = if ui.visuals().dark_mode { theme::BG_BASE } else { Color32::WHITE };
    styled_button(ui, text, accent, on_accent, accent)
}

/// Secondary action button (elevated fill, border).
pub fn secondary_button(ui: &mut Ui, text: &str) -> egui::Response {
    let fill = ui.visuals().widgets.inactive.bg_fill;
    let fg = ui.visuals().widgets.inactive.fg_stroke.color;
    let stroke = ui.visuals().widgets.inactive.bg_stroke;
    styled_button(ui, text, fill, fg, stroke.color)
}

/// Danger button (red tinted fill and stroke).
pub fn danger_button(ui: &mut Ui, text: &str) -> egui::Response {
    let error = theme::ERROR;
    let fill = error.linear_multiply(if ui.visuals().dark_mode { 0.2 } else { 0.12 });
    styled_button(ui, text, fill, error, error)
}

fn styled_button(
    ui: &mut Ui,
    text: &str,
    fill: Color32,
    text_color: Color32,
    stroke_color: Color32,
) -> egui::Response {
    let button = egui::Button::new(
        RichText::new(text).size(theme::FONT_BODY).color(text_color),
    )
    .fill(fill)
    .stroke(egui::Stroke::new(1.0, stroke_color))
    .corner_radius(CornerRadius::same(theme::RADIUS_BUTTON as u8));

    ui.add(button)
}
