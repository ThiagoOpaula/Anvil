//! Stat tile widgets for summary dashboards.

use egui::{Color32, CornerRadius, Frame, Margin, RichText, Ui, Vec2};

use crate::gui::theme;

/// A large stat tile with a number, label, and accent top border.
pub struct StatTile<'a> {
    value: String,
    label: &'a str,
    accent: Color32,
}

impl<'a> StatTile<'a> {
    pub fn new(value: impl Into<String>, label: &'a str) -> Self {
        Self {
            value: value.into(),
            label,
            accent: theme::ACCENT,
        }
    }

    pub fn accent(mut self, color: Color32) -> Self {
        self.accent = color;
        self
    }

    pub fn show(self, ui: &mut Ui) {
        let dark = ui.visuals().dark_mode;
        let frame = Frame::new()
            .fill(theme::elevated_color(dark))
            .corner_radius(CornerRadius::same(theme::RADIUS_CARD as u8))
            .inner_margin(Margin::same(theme::SPACE_MD as i8));

        let inner = frame.show(ui, |ui| {
            ui.set_min_width(80.0);
            ui.add_space(4.0);
            ui.label(
                RichText::new(&self.value)
                    .size(theme::FONT_HEADING)
                    .color(theme::text_primary_color(dark))
                    .strong(),
            );
            ui.add_space(2.0);
            ui.label(
                RichText::new(self.label)
                    .size(theme::FONT_CAPTION)
                    .color(theme::text_secondary_color(dark)),
            );
        });

        // Accent top line — paint on the frame's outer rect (correct width).
        let rect = inner.response.rect;
        let paint_rect = egui::Rect::from_min_size(
            rect.min,
            Vec2::new(rect.width(), 3.0),
        );
        ui.painter().rect_filled(paint_rect, 0.0, self.accent);
    }
}

/// A horizontal row of stat tiles.
pub fn stat_row(ui: &mut Ui, tiles: Vec<StatTile>) {
    ui.horizontal(|ui| {
        for tile in tiles {
            tile.show(ui);
            ui.add_space(theme::SPACE_SM);
        }
    });
}
