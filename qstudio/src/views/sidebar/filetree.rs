use crate::models::ui::UIEvent;
use std::fs;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Fs {
    Folder { name: String, children: Vec<Fs> },
    File { name: String, file_path: String },
}

use crate::Channels;
#[derive(Debug, Clone)]
pub struct FolderTree {
    file_system: Arc<Fs>,
    file_path: String,
}

impl FolderTree {
    pub fn new(file_path: String) -> Self {
        let file_system = Arc::new(Self::build_fs_tree(&std::path::Path::new(&file_path)));
        FolderTree {
            file_system,
            file_path,
        }
    }

    pub fn refresh(&mut self) {
        self.file_system = Arc::new(Self::build_fs_tree(&std::path::Path::new(&self.file_path)));
    }

    fn build_fs_tree(path: &std::path::Path) -> Fs {
        if path.is_dir() {
            let children = fs::read_dir(path)
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| Self::build_fs_tree(&entry.path()))
                .collect();
            Fs::Folder {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "root".to_string()),
                children,
            }
        } else {
            Fs::File {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "".to_string()),
                file_path: path.to_string_lossy().into(),
            }
        }
    }

    fn show_node(&self, ui: &mut egui::Ui, node: &Fs, channels: Arc<Channels>) {
        match node {
            Fs::Folder { name, children } => {
                egui::CollapsingHeader::new(name)
                    .default_open(false)
                    .show(ui, |ui| {
                        for child in children {
                            self.show_node(ui, child, channels.clone());
                            ui.add_space(4.0);
                        }
                    });
            }
            Fs::File { name, file_path } => {
                if ui.link(name).clicked() {
                    channels
                        .senders()
                        .ui_tx
                        .lock()
                        .unwrap()
                        .send(UIEvent::AddPane(crate::models::ui::UIEventPane::Text(
                            file_path.clone(),
                        )))
                        .unwrap();
                }

                ui.add_space(4.0);
            }
        }
    }
}

impl FolderTree {
    pub fn ui(&mut self, ui: &mut egui::Ui, channels: Arc<Channels>) {
        egui::Frame::new()
            .inner_margin(0.0)
            .outer_margin(0.0)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_min_height(ui.available_height());
                    ui.set_max_width(256.0);
                    ui.add_space(12.0);
                    ui.heading("Folder");
                    ui.separator();
                    ui.add_space(12.0);

                    egui::ScrollArea::vertical()
                        .max_height(ui.available_height() - 24.0)
                        .max_width(ui.available_width())
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            // Folder tree
                            if let Fs::Folder { children, .. } = &*self.file_system {
                                for child in children {
                                    self.show_node(ui, child, channels.clone());
                                }
                            }
                        });
                });
            });
    }
}
