use egui::{self, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Ui,
    Connection,
}

#[derive(Default, Debug, Clone)]
pub struct Settings {
    active: Option<Section>,
}

impl Settings {
    pub fn new() -> Self {
        Self {
            active: Some(Section::Ui),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        egui::Frame::new()
            .inner_margin(0.0)
            .outer_margin(0.0)
            .show(ui, |ui| {
                let height = ui.available_height() - 60.0; // Account for padding and potential scrollbar
                ui.vertical(|ui| {
                    self.accordion_item(
                        ui,
                        "Ui Settings",
                        Section::Ui,
                        height - 16.0,
                        Settings::ui_settings_body,
                    );
                    self.accordion_item(
                        ui,
                        "Connection Settings",
                        Section::Connection,
                        height - 16.0,
                        Settings::connection_settings_body,
                    );
                });
            });
    }

    /// Simple accordion row: a clickable header + conditional body.
    /// `body` is a function pointer (no capture of &mut self), avoiding borrow conflicts.
    fn accordion_item(
        &mut self,
        ui: &mut Ui,
        title: &str,
        section: Section,
        height: f32,
        body: fn(&mut Self, &mut Ui, f32),
    ) {
        let is_open = self.active == Some(section);

        // Header row with a disclosure arrow
        let arrow = if is_open { "▾" } else { "▸" };
        let header_resp = ui
            .horizontal(|ui| {
                // Make the whole row clickable
                let resp = ui.selectable_label(is_open, format!("{arrow} {title}"));
                resp
            })
            .inner;

        if header_resp.clicked() {
            // Toggle this section; ensures mutual exclusivity
            self.active = if is_open { None } else { Some(section) };
        }

        // Render body only when active
        if self.active == Some(section) {
            ui.indent(ui.make_persistent_id(title), |ui| {
                (body)(self, ui, height);
            });
        }

        // Optional visual divider
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);
    }

    // Bodies as plain fns (function items), not closures:
    fn ui_settings_body(&mut self, ui: &mut Ui, height: f32) {
        ui.vertical(|ui| {
            ui.set_min_height(height);
            ui.set_max_height(height);
            ui.add_space(12.0);
            if ui.button("Toggle Dark/Light Mode").clicked() {
                let current = ui.ctx().style().visuals.dark_mode;
                ui.ctx().set_visuals(if current {
                    egui::Visuals::light()
                } else {
                    egui::Visuals::dark()
                });
            }
            ui.add_space(8.0);
            if ui.button("Reset UI Layout").clicked() {
                ui.ctx().memory_mut(|mem| mem.reset_areas());
            }
        });
    }

    fn connection_settings_body(&mut self, ui: &mut Ui, height: f32) {
        ui.vertical(|ui| {
            ui.set_min_height(height);
            ui.set_max_height(height);
            ui.add_space(12.0);
            ui.label("No connection settings available yet.");
            ui.add_space(8.0);
        });
    }
}
