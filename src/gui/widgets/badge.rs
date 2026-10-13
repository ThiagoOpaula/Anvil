//! Badge widgets — StatusBadge, LoaderBadge, CountBadge.

use egui::{Color32, CornerRadius, RichText, Ui, Vec2};

use crate::gui::theme;
use crate::types::ModOutcome;

// ── Status kind ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Updated,
    UpToDate,
    Failed,
    Warning,
    Info,
    Unknown,
    Filtered,
}

impl StatusKind {
    pub fn from_outcome(outcome: &ModOutcome) -> Self {
        match outcome {
            ModOutcome::Updated { .. } => StatusKind::Updated,
            ModOutcome::UpToDate { .. } => StatusKind::UpToDate,
            ModOutcome::Unavailable { .. } => StatusKind::Warning,
            ModOutcome::Unknown { .. } => StatusKind::Unknown,
            ModOutcome::FilteredOut { .. } => StatusKind::Filtered,
            ModOutcome::Failed { .. } => StatusKind::Failed,
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            StatusKind::Updated => theme::ACCENT,
            StatusKind::UpToDate => theme::TEXT_SECONDARY,
            StatusKind::Failed => theme::ERROR,
            StatusKind::Warning => theme::WARNING,
            StatusKind::Info => theme::INFO,
            StatusKind::Unknown => theme::INFO,
            StatusKind::Filtered => theme::TEXT_MUTED,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StatusKind::Updated => "Updated",
            StatusKind::UpToDate => "Up to date",
            StatusKind::Failed => "Failed",
            StatusKind::Warning => "Warning",
            StatusKind::Info => "Info",
            StatusKind::Unknown => "Unknown",
            StatusKind::Filtered => "Filtered",
        }
    }
}

// ── StatusBadge ───────────────────────────────────────────────────────────

/// A rounded pill badge with semantic coloring.
pub struct StatusBadge {
    kind: StatusKind,
    text: Option<String>,
}

impl StatusBadge {
    pub fn new(kind: StatusKind) -> Self {
        Self { kind, text: None }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn from_outcome(outcome: &ModOutcome) -> Self {
        Self::new(StatusKind::from_outcome(outcome))
    }

    pub fn show(self, ui: &mut Ui) -> egui::Response {
        let color = self.kind.color();
        let text = self.text.unwrap_or_else(|| self.kind.label().to_string());
        let fill = theme::badge_fill(color);
        let text_color = theme::badge_text(color);

        let inner = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(theme::SPACE_XS, 0.0);
            let label = RichText::new(&text)
                .size(theme::FONT_CAPTION)
                .color(text_color);
            let frame = egui::Frame::new()
                .fill(fill)
                .corner_radius(CornerRadius::same(theme::RADIUS_PILL as u8))
                .inner_margin(egui::Margin::symmetric(theme::SPACE_SM as i8, theme::SPACE_XS as i8));
            frame.show(ui, |ui| {
                ui.label(label);
            });
        });
        inner.response
    }
}

// ── LoaderBadge ───────────────────────────────────────────────────────────

/// A small pill showing a mod loader name (fabric, forge, etc.).
pub struct LoaderBadge<'a> {
    loader: &'a str,
}

impl<'a> LoaderBadge<'a> {
    pub fn new(loader: &'a str) -> Self {
        Self { loader }
    }

    pub fn show(self, ui: &mut Ui) {
        let frame = egui::Frame::new()
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .corner_radius(CornerRadius::same(theme::RADIUS_PILL as u8))
            .inner_margin(egui::Margin::symmetric(theme::SPACE_SM as i8, 2));
        frame.show(ui, |ui| {
            ui.label(
                RichText::new(self.loader)
                    .size(theme::FONT_CAPTION)
                    .color(theme::TEXT_SECONDARY),
            );
        });
    }
}

// ── CountBadge ────────────────────────────────────────────────────────────

/// A small filled pill showing a number.
pub struct CountBadge {
    count: usize,
    color: Color32,
}

impl CountBadge {
    pub fn new(count: usize) -> Self {
        Self {
            count,
            color: theme::ERROR,
        }
    }

    pub fn colored(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    pub fn show(self, ui: &mut Ui) {
        let fill = theme::badge_fill(self.color);
        let text_color = theme::badge_text(self.color);
        let frame = egui::Frame::new()
            .fill(fill)
            .corner_radius(CornerRadius::same(theme::RADIUS_PILL as u8))
            .inner_margin(egui::Margin::symmetric(theme::SPACE_SM as i8, 2));
        frame.show(ui, |ui| {
            ui.label(
                RichText::new(self.count.to_string())
                    .size(theme::FONT_CAPTION)
                    .color(text_color)
                    .strong(),
            );
        });
    }
}
