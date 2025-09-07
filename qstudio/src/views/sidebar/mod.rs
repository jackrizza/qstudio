use egui::RichText;

use egui_flex::{Flex, FlexAlignContent, FlexItem};

use crate::Channels;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

mod active_engines;
mod filetree;
mod settings;

#[derive(Debug, Clone)]
pub struct SideBar {
    pub show_folder_tree: bool,
    pub show_settings: bool,
    pub show_active_engines: bool,
    pub show_search: bool,

    file_tree: filetree::FolderTree,
    // Add other sidebar components as needed
    engines: Arc<Mutex<HashMap<String, Arc<Mutex<engine::Engine>>>>>,
}

impl SideBar {
    pub fn new(
        file_path: String,
        engines: Arc<Mutex<HashMap<String, Arc<Mutex<engine::Engine>>>>>,
    ) -> Self {
        SideBar {
            file_tree: filetree::FolderTree::new(file_path),
            show_folder_tree: false,
            show_settings: false,
            show_active_engines: false,
            show_search: false,
            engines,
        }
    }

    pub fn width(&self) -> f32 {
        if self.show_folder_tree || self.show_settings || self.show_active_engines {
            64.0 + 270.0 // Fixed width for the sidebar
        } else {
            64.0
        }
    }
}

impl SideBar {
    pub fn fs_refresh(&mut self) {
        self.file_tree.refresh();
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, channels: Arc<Channels>) {
        let primary_background = if ui.visuals().dark_mode {
            theme::GITHUB_DARK.crust
        } else {
            theme::GITHUB_LIGHT.crust
        };

        Flex::horizontal()
            .align_content(FlexAlignContent::Stretch)
            .show(ui, |flex| {
                flex.add_ui(FlexItem::new().grow(1.0), |ui| {
                    egui::Frame::new().fill(primary_background).show(ui, |ui| {
                        ui.set_max_width(64.0);
                        ui.set_min_width(64.0);
                        ui.vertical_centered(|ui| {
                            ui.set_min_height(ui.available_height());

                            // Top margin
                            ui.add_space(24.0);

                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(
                                            egui_material_icons::icons::ICON_LOW_PRIORITY,
                                        )
                                        .size(32.0),
                                    )
                                    .fill(egui::Color32::TRANSPARENT),
                                )
                                .on_hover_text("View active engines")
                                .clicked()
                            {
                                // Logic to add a new pane can be implemented here
                                self.show_folder_tree = false;
                                self.show_settings = false;
                                self.show_active_engines = !self.show_active_engines;
                            }

                            ui.add_space(16.0); // Space between buttons

                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(egui_material_icons::icons::ICON_FOLDER)
                                            .size(32.0),
                                    )
                                    .fill(egui::Color32::TRANSPARENT),
                                )
                                .on_hover_text("View folder structure")
                                .clicked()
                            {
                                self.show_active_engines = false;
                                self.show_settings = false;
                                self.show_folder_tree = !self.show_folder_tree;
                            }

                            ui.add_space(16.0); // Space between buttons

                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(egui_material_icons::icons::ICON_SEARCH)
                                            .size(32.0),
                                    )
                                    .fill(egui::Color32::TRANSPARENT),
                                )
                                .on_hover_text("Use chat gpt")
                                .clicked()
                            {
                                self.show_search = !self.show_search;
                            }

                            let space_between = ui.available_height() - 72.0; // Remaining space
                            ui.add_space(space_between); // Space between buttons

                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(egui_material_icons::icons::ICON_SETTINGS)
                                            .size(32.0),
                                    )
                                    .fill(egui::Color32::TRANSPARENT),
                                )
                                .on_hover_text("Open settings")
                                .clicked()
                            {
                                self.show_active_engines = false;
                                self.show_folder_tree = false;
                                self.show_settings = !self.show_settings;
                            }
                            // ui.add_space(8.0); // Space between buttons
                        });
                    });
                });

                flex.add_ui(FlexItem::new().grow(1.0), |ui| {
                    let secondary_background = if ui.visuals().dark_mode {
                        theme::GITHUB_DARK.mantle
                    } else {
                        theme::GITHUB_LIGHT.mantle
                    };

                    egui::Frame::none()
                        .fill(secondary_background)
                        .show(ui, |ui| {
                            if self.show_folder_tree {
                                ui.add_space(8.0);
                                self.file_tree.ui(ui, channels.clone());
                            } else if self.show_settings {
                                ui.add_space(8.0);
                                settings::settings_ui(ui);
                            } else if self.show_active_engines {
                                ui.add_space(8.0);
                                active_engines::active_engines_ui(
                                    ui,
                                    &self.engines.lock().unwrap(),
                                    channels,
                                );
                            }
                        });
                });
            });
    }
}
