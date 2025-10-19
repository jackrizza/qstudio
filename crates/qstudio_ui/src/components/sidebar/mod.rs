use egui::RichText;

use egui_flex::{Flex, FlexAlignContent, FlexItem};

use egui_material_icons::icons;
use std::sync::Arc;

use busbar::Aluminum;
use events::Event;

mod engine;
mod filetree;
mod settings;

use engine::ActiveEngines;
use filetree::FileTreeUi;
use qstudio_tcp::Client;

#[derive(Debug, Clone)]
pub struct SideBar {
    pub show_folder_tree: bool,
    pub show_settings: bool,
    pub show_active_engines: bool,
    pub show_search: bool,

    pub filetree: FileTreeUi,
    pub active_engines: ActiveEngines,
    pub settings: settings::Settings,
    // Add other sidebar components as needed
    only_client: Client,
}

impl SideBar {
    pub fn new(filetree_arc: Arc<Aluminum<(Client, Event)>>, only_client: Client) -> Self {
        SideBar {
            show_folder_tree: false,
            show_settings: false,
            show_active_engines: false,
            show_search: false,
            filetree: FileTreeUi::new(Arc::clone(&filetree_arc), only_client.clone()),
            active_engines: ActiveEngines::new(Arc::clone(&filetree_arc), only_client.clone()),
            settings: settings::Settings::new(),
            only_client,
        }
    }

    pub fn width(&self) -> f32 {
        if self.show_folder_tree || self.show_settings || self.show_active_engines {
            64.0 + 280.0 + 8.0 // Fixed width for the sidebar
        } else {
            64.0
        }
    }
}

impl SideBar {
    pub fn ui(&mut self, ui: &mut egui::Ui, aluminum: Arc<Aluminum<(Client, Event)>>) {
        let primary_background = theme::get_mode_theme(ui.ctx()).crust;
        let secondary_background = primary_background;

        Flex::horizontal()
            .align_content(FlexAlignContent::Stretch)
            .show(ui, |flex| {
                flex.add_ui(FlexItem::new().grow(1.0), |ui| {
                    egui::Frame::new()
                        .inner_margin(0.0)
                        .outer_margin(0.0)
                        .fill(primary_background)
                        .show(ui, |ui| {
                            ui.set_max_width(64.0);
                            ui.set_min_width(64.0);
                            ui.vertical_centered(|ui| {
                                ui.set_min_height(ui.available_height());

                                // Top margin
                                ui.add_space(24.0);

                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(icons::ICON_LOW_PRIORITY).size(32.0),
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
                                            RichText::new(icons::ICON_FOLDER).size(32.0),
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
                                            RichText::new(icons::ICON_SEARCH).size(32.0),
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
                                            RichText::new(icons::ICON_SETTINGS).size(32.0),
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
                    egui::Frame::NONE
                        .inner_margin(0.0)
                        .outer_margin(0.0)
                        .fill(secondary_background)
                        .show(ui, |ui| {
                            ui.set_min_width(self.width() - 64.0);
                            ui.set_max_width(self.width() - 64.0);
                            ui.set_min_height(ui.available_height());
                            egui::Frame::NONE
                                .inner_margin(
                                    if self.show_folder_tree
                                        || self.show_settings
                                        || self.show_active_engines
                                    {
                                        4.0
                                    } else {
                                        0.0
                                    },
                                )
                                .outer_margin(0.0)
                                .show(ui, |ui| {
                                    if self.show_folder_tree {
                                        if !self.filetree.get_initial_listing {
                                            let _ = aluminum.backend_tx.send((
                                                self.only_client.clone(),
                                                Event::FileEvent(
                                                    events::events::files::FileEvent::GetDirectoryListing,
                                                ),
                                            ));
                                            self.filetree.get_initial_listing = true;
                                        }
                                        self.filetree.ui(ui);
                                    } else if self.show_settings {
                                        self.settings.ui(ui);
                                    } else if self.show_active_engines {
                                        self.active_engines.ui(ui);
                                    }
                                });
                        });
                });
            });
    }
}
