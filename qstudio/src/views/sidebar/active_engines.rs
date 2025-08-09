use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{EngineEvent, UIEvent};
use crate::models::ui::UIEventPane;
use egui_material_icons::icon_button;
use engine::EngineStatus;

use crate::Channels;
use egui::RichText;

pub fn active_engines_ui(
    ui: &mut egui::Ui,
    engines: &HashMap<String, Arc<Mutex<engine::Engine>>>,
    channels: Arc<Channels>,
) {
    egui::Frame::none()
        .inner_margin(0.0)
        .outer_margin(0.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_min_height(ui.available_height());
                ui.set_max_width(256.0);
                ui.add_space(12.0);
                ui.heading("Engines");
                ui.separator();
                ui.add_space(12.0);
                for (file_path, engine) in engines.iter() {
                    egui::CollapsingHeader::new(file_path)
                        .default_open(true)
                        .show(ui, |ui| {
                            let engine = engine.lock().unwrap();
                            ui.label(format!("Status: {:?}", engine.status()));
                            match engine.analyze() {
                                Ok(_) => {
                                    ui.label("Analysis successful.");
                                }
                                Err(e) => {
                                    ui.label(format!("Error: {}", e));
                                }
                            }
                            ui.horizontal(|ui| {
                                match engine.status() {
                                    EngineStatus::Running => {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new(
                                                        egui_material_icons::icons::ICON_STOP,
                                                    )
                                                    .color(egui::Color32::ORANGE)
                                                    .size(16.0),
                                                )
                                                .fill(egui::Color32::TRANSPARENT),
                                            )
                                            .on_hover_text("Stop engine")
                                            .clicked()
                                        {
                                            // Handle stop button click
                                            channels
                                                .engine_tx
                                                .lock()
                                                .unwrap()
                                                .send(EngineEvent::Stop(file_path.clone()))
                                                .unwrap();
                                        }
                                    }
                                    _ => {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new(
                                                        egui_material_icons::icons::ICON_START,
                                                    )
                                                    .color(egui::Color32::GREEN)
                                                    .size(16.0),
                                                )
                                                .fill(egui::Color32::TRANSPARENT),
                                            )
                                            .on_hover_text("Start engine")
                                            .clicked()
                                        {
                                            // Handle start button click
                                            channels
                                                .engine_tx
                                                .lock()
                                                .unwrap()
                                                .send(EngineEvent::Start(file_path.clone()))
                                                .unwrap();
                                        }
                                    }
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(egui_material_icons::icons::ICON_RESTORE)
                                                .color(egui::Color32::BLUE)
                                                .size(16.0),
                                        )
                                        .fill(egui::Color32::TRANSPARENT),
                                    )
                                    .on_hover_text("Restore engine")
                                    .clicked()
                                {
                                    // Handle restore button click
                                    channels
                                        .engine_tx
                                        .lock()
                                        .unwrap()
                                        .send(EngineEvent::Restart(file_path.clone()))
                                        .unwrap();
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(egui_material_icons::icons::ICON_DELETE)
                                                .color(egui::Color32::RED)
                                                .size(16.0),
                                        )
                                        .fill(egui::Color32::TRANSPARENT),
                                    )
                                    .on_hover_text("Delete engine")
                                    .clicked()
                                {
                                    // Handle delete button click
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(
                                                egui_material_icons::icons::ICON_CHART_DATA,
                                            )
                                            .size(16.0),
                                        )
                                        .fill(egui::Color32::TRANSPARENT),
                                    )
                                    .on_hover_text("View engine data")
                                    .clicked()
                                {
                                    // Handle view button click
                                    channels
                                        .ui_tx
                                        .lock()
                                        .unwrap()
                                        .send(UIEvent::AddPane(
                                            UIEventPane::GraphView(file_path.clone()),
                                        ))
                                        .unwrap();
                                }
                            });
                        });
                }
            });
        });
}
