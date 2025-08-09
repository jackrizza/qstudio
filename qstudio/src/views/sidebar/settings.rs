use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::models::ui::UIEventPane;
use crate::{EngineEvent, UIEvent};
use egui_material_icons::icon_button;
use engine::EngineStatus;

use crate::Channels;
use egui::RichText;

pub fn settings_ui(ui: &mut egui::Ui) {
    egui::Frame::none()
        .inner_margin(0.0)
        .outer_margin(0.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_min_height(ui.available_height());
                ui.set_max_width(256.0);
                ui.add_space(12.0);
                ui.heading("Settings");
                ui.separator();
                ui.add_space(12.0);
                if ui.button("Toggle Dark/Light Mode").clicked() {
                    let current = ui.ctx().style().visuals.dark_mode;
                    ui.ctx().set_visuals(if current {
                        egui::Visuals::light()
                    } else {
                        egui::Visuals::dark()
                    });
                }
            });
        });
}
