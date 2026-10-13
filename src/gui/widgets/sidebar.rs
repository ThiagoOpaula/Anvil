//! Sidebar navigation widget.

use egui::{CornerRadius, Layout, RichText, Stroke, Ui, Vec2};

use crate::gui::state::Tab;
use crate::gui::theme;

/// A single sidebar navigation item.
pub struct SidebarItem {
    tab: Tab,
    active: bool,
}

impl SidebarItem {
    pub fn new(tab: Tab, active: bool) -> Self {
        Self { tab, active }
    }

    /// Show the item. Returns `true` if clicked.
    pub fn show(self, ui: &mut Ui) -> bool {
        let (bg, fg, stroke) = if self.active {
            (
                theme::BG_ELEVATED,
                theme::TEXT_PRIMARY,
                Stroke::new(1.0, theme::ACCENT),
            )
        } else {
            (
                theme::BG_SURFACE,
                theme::TEXT_SECONDARY,
                Stroke::NONE,
            )
        };

        let frame = egui::Frame::new()
            .fill(bg)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(4u8))
            .inner_margin(egui::Margin::symmetric(
                theme::SPACE_MD as i8,
                theme::SPACE_SM as i8,
            ));

        let response = frame
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.tab.icon())
                            .size(theme::FONT_BODY)
                            .color(fg),
                    );
                    ui.add_space(theme::SPACE_SM);
                    ui.label(
                        RichText::new(self.tab.label())
                            .size(theme::FONT_BODY)
                            .color(fg),
                    );
                });
            })
            .response;

        response.interact(egui::Sense::click()).clicked()
    }
}

/// The full sidebar panel with brand header and nav items.
pub struct Sidebar {
    pub active: Tab,
    pub version: &'static str,
    pub has_update: bool,
    pub dark_mode: bool,
}

impl Sidebar {
    pub fn show(
        self,
        ui: &mut Ui,
    ) -> SidebarResult {
        let mut result = SidebarResult {
            switch_to: None,
            toggle_theme: false,
        };

        ui.vertical(|ui| {
            // Brand header
            ui.add_space(theme::SPACE_MD);
            ui.horizontal(|ui| {
                ui.add_space(theme::SPACE_SM);
                ui.label(
                    RichText::new("\u{2692}")
                        .size(theme::FONT_SUBHEADING)
                        .color(theme::ACCENT),
                );
                ui.add_space(theme::SPACE_SM);
                ui.label(
                    RichText::new("Anvil")
                        .size(theme::FONT_SUBHEADING)
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                );
            });
            ui.add_space(theme::SPACE_SM);
            ui.separator();
            ui.add_space(theme::SPACE_SM);

            // Navigation items
            for tab in [
                Tab::Scan,
                Tab::Updates,
                Tab::Rollback,
                Tab::Settings,
            ] {
                if SidebarItem::new(tab, tab == self.active).show(ui) {
                    result.switch_to = Some(tab);
                }
                ui.add_space(4.0);
            }

            // Bottom: theme toggle + version
            ui.with_layout(Layout::bottom_up(egui::Align::Min), |ui| {
                ui.add_space(theme::SPACE_SM);
                ui.horizontal(|ui| {
                    ui.add_space(theme::SPACE_SM);
                    ui.label(
                        RichText::new(format!("v{}", self.version))
                            .size(theme::FONT_CAPTION)
                            .color(theme::TEXT_MUTED),
                    );
                    if self.has_update {
                        ui.add_space(theme::SPACE_SM);
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::splat(8.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().circle_filled(rect.center(), 4.0, theme::ACCENT);
                    }
                });
                ui.add_space(theme::SPACE_SM);
                ui.horizontal(|ui| {
                    ui.add_space(theme::SPACE_SM);
                    let label = if self.dark_mode { "Light mode" } else { "Dark mode" };
                    if ui
                        .button(RichText::new(label).size(theme::FONT_CAPTION))
                        .clicked()
                    {
                        result.toggle_theme = true;
                    }
                });
                ui.add_space(theme::SPACE_MD);
            });
        });

        result
    }
}

/// Result of rendering the sidebar.
pub struct SidebarResult {
    pub switch_to: Option<Tab>,
    pub toggle_theme: bool,
}
