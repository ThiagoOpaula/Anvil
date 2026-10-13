//! Empty, loading, and error state widgets.

use egui::{CornerRadius, Layout, RichText, Stroke, Ui, Vec2};

use crate::gui::theme;

/// A centered empty-state display with optional CTA button.
pub struct EmptyState<'a> {
    icon: &'a str,
    title: &'a str,
    subtitle: Option<&'a str>,
    cta_text: Option<&'a str>,
}

impl<'a> EmptyState<'a> {
    pub fn new(icon: &'a str, title: &'a str) -> Self {
        Self {
            icon,
            title,
            subtitle: None,
            cta_text: None,
        }
    }

    pub fn subtitle(mut self, text: &'a str) -> Self {
        self.subtitle = Some(text);
        self
    }

    pub fn cta(mut self, text: &'a str) -> Self {
        self.cta_text = Some(text);
        self
    }

    pub fn show(self, ui: &mut Ui) -> egui::Response {
        let dark = ui.visuals().dark_mode;
        ui.vertical_centered(|ui| {
            ui.add_space(theme::SPACE_XL);
            ui.label(
                RichText::new(self.icon)
                    .size(32.0)
                    .color(theme::text_muted_color(dark)),
            );
            ui.add_space(theme::SPACE_MD);
            ui.label(
                RichText::new(self.title)
                    .size(theme::FONT_SUBHEADING)
                    .color(theme::text_primary_color(dark)),
            );
            if let Some(sub) = self.subtitle {
                ui.add_space(theme::SPACE_SM);
                ui.label(
                    RichText::new(sub)
                        .size(theme::FONT_BODY)
                        .color(theme::text_secondary_color(dark)),
                );
            }
            if let Some(cta) = self.cta_text {
                ui.add_space(theme::SPACE_MD);
                if ui.button(
                    RichText::new(cta)
                        .size(theme::FONT_BODY)
                        .color(theme::BG_BASE),
                ).clicked() {}
            }
            ui.add_space(theme::SPACE_XL);
        })
        .response
    }
}

/// A loading state with spinner and message.
pub struct LoadingState<'a> {
    message: &'a str,
}

impl<'a> LoadingState<'a> {
    pub fn new(message: &'a str) -> Self {
        Self { message }
    }

    pub fn show(self, ui: &mut Ui) {
        let dark = ui.visuals().dark_mode;
        ui.vertical_centered(|ui| {
            ui.add_space(theme::SPACE_LG);
            ui.spinner();
            ui.add_space(theme::SPACE_SM);
            ui.label(
                RichText::new(self.message)
                    .size(theme::FONT_BODY)
                    .color(theme::text_secondary_color(dark)),
            );
            ui.add_space(theme::SPACE_LG);
        });
    }
}

/// A red-tinted error banner with dismiss option.
pub struct ErrorBanner<'a> {
    message: &'a str,
}

impl<'a> ErrorBanner<'a> {
    pub fn new(message: &'a str) -> Self {
        Self { message }
    }

    /// Show the banner. Returns `true` if the dismiss button was clicked.
    pub fn show(self, ui: &mut Ui) -> bool {
        let mut dismissed = false;
        let frame = egui::Frame::new()
            .fill(theme::ERROR.linear_multiply(0.12))
            .stroke(Stroke::new(1.0, theme::ERROR.linear_multiply(0.4)))
            .corner_radius(CornerRadius::same(theme::RADIUS_CARD as u8))
            .inner_margin(egui::Margin::same(theme::SPACE_MD as i8));

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("\u{2717}")
                        .size(theme::FONT_BODY)
                        .color(theme::ERROR),
                );
                ui.label(
                    RichText::new(self.message)
                        .size(theme::FONT_BODY)
                        .color(theme::ERROR),
                );
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Dismiss").clicked() {
                        dismissed = true;
                    }
                });
            });
        });
        dismissed
    }
}

/// A warning banner (amber tinted).
pub struct WarningBanner<'a> {
    message: &'a str,
}

impl<'a> WarningBanner<'a> {
    pub fn new(message: &'a str) -> Self {
        Self { message }
    }

    pub fn show(self, ui: &mut Ui) {
        let frame = egui::Frame::new()
            .fill(theme::WARNING.linear_multiply(0.12))
            .stroke(Stroke::new(1.0, theme::WARNING.linear_multiply(0.4)))
            .corner_radius(CornerRadius::same(theme::RADIUS_CARD as u8))
            .inner_margin(egui::Margin::same(theme::SPACE_MD as i8));

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("\u{26a0}")
                        .size(theme::FONT_BODY)
                        .color(theme::WARNING),
                );
                ui.label(
                    RichText::new(self.message)
                        .size(theme::FONT_BODY)
                        .color(theme::WARNING),
                );
            });
        });
    }
}

// silence unused
#[allow(dead_code)]
fn _unused(_: Vec2) {}
