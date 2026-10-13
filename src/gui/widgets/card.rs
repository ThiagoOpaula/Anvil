//! Card containers.

use egui::{CornerRadius, RichText, Stroke, Ui};

use crate::gui::theme;

/// A section card with a title header and content area.
pub struct SectionCard<'a> {
    title: &'a str,
    collapsible: bool,
    default_open: bool,
}

impl<'a> SectionCard<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            collapsible: false,
            default_open: true,
        }
    }

    pub fn collapsible(mut self, open: bool) -> Self {
        self.collapsible = true;
        self.default_open = open;
        self
    }

    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
        let dark = ui.visuals().dark_mode;
        let frame = egui::Frame::new()
            .fill(theme::surface_color(dark))
            .stroke(Stroke::new(1.0, theme::border_color(dark)))
            .corner_radius(CornerRadius::same(theme::RADIUS_CARD as u8))
            .inner_margin(egui::Margin::same(theme::SPACE_MD as i8));

        frame.show(ui, |ui| {
            ui.label(
                RichText::new(self.title)
                    .size(theme::FONT_CAPTION)
                    .color(theme::text_secondary_color(dark)),
            );
            ui.add_space(theme::SPACE_SM);
            ui.separator();
            ui.add_space(theme::SPACE_SM);
            add_contents(ui)
        })
    }
}

/// A simple bordered card without a title.
pub struct Card;

impl Card {
    pub fn show<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
        let dark = ui.visuals().dark_mode;
        let frame = egui::Frame::new()
            .fill(theme::surface_color(dark))
            .stroke(Stroke::new(1.0, theme::border_color(dark)))
            .corner_radius(CornerRadius::same(theme::RADIUS_CARD as u8))
            .inner_margin(egui::Margin::same(theme::SPACE_MD as i8));

        frame.show(ui, add_contents)
    }
}

/// An elevated card (lighter fill, for hover states or featured items).
pub struct ElevatedCard;

impl ElevatedCard {
    pub fn show<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
        let dark = ui.visuals().dark_mode;
        let frame = egui::Frame::new()
            .fill(theme::elevated_color(dark))
            .stroke(Stroke::new(1.0, theme::border_color(dark)))
            .corner_radius(CornerRadius::same(theme::RADIUS_CARD as u8))
            .inner_margin(egui::Margin::same(theme::SPACE_MD as i8));

        frame.show(ui, add_contents)
    }
}

/// Section header with optional right-aligned action.
pub struct SectionHeader<'a> {
    title: &'a str,
}

impl<'a> SectionHeader<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title }
    }

    pub fn show(self, ui: &mut Ui) {
        let dark = ui.visuals().dark_mode;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(self.title)
                    .size(theme::FONT_SUBHEADING)
                    .color(theme::text_primary_color(dark))
                    .strong(),
            );
        });
        ui.add_space(theme::SPACE_XS);
        ui.separator();
        ui.add_space(theme::SPACE_SM);
    }
}


