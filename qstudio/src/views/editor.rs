use core::fmt;
use eframe::egui;
use egui::{Ui, WidgetText};
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};
use egui_commonmark::*;
use egui_dock::{DockArea, DockState, OverlayType, Style, TabAddAlign, TabViewer};
use egui_extras::{Column, TableBuilder};
use engine::parser::Graph;
use engine::Engine;
use polars::frame::DataFrame;
use polars::prelude::*;
use rand::distr::Alphanumeric;
use rand::Rng;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::rc::Rc;

use crate::models::{QueryResult, Settings};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TabKind {
    Code(String),
    Markdown(String),
    Settings(String),
}

impl TabKind {
    pub fn as_str(&self) -> &str {
        match self {
            TabKind::Markdown(s) | TabKind::Code(s) => s,
            TabKind::Settings(_) => "Settings",
        }
    }
}

pub type Tab = TabKind;

#[derive(Debug)]
pub struct Code {
    pub code: String,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
}

impl Code {
    fn files_been_edited(&self) -> bool {
        if self.file_path.is_some() && self.file_name.is_some() {
            if let Some(path) = &self.file_path {
                match fs::read_to_string(path) {
                    Ok(file_data) => {
                        fn calculate_checksum<T: Hash>(t: &T) -> u64 {
                            let mut s = DefaultHasher::new();
                            t.hash(&mut s);
                            s.finish()
                        }

                        let code_checksum = calculate_checksum(&self.code);
                        let file_checksum = calculate_checksum(&file_data);

                        code_checksum != file_checksum
                    }
                    Err(_) => false,
                }
            } else {
                false
            }
        } else {
            true
        }
    }
}

pub struct MyTabViewer {
    pub data: HashMap<String, Code>,
    settings: Rc<RefCell<Settings>>,
}

impl fmt::Debug for MyTabViewer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MyTabViewer")
            .field("data", &self.data)
            .field("settings", &self.settings)
            .finish()
    }
}

impl MyTabViewer {
    pub fn new(settings: Rc<RefCell<Settings>>) -> Self {
        let mut hm = HashMap::new();
        hm.insert(
            "0".to_string(),
            Code {
                code: String::new(),
                file_path: None,
                file_name: None,
            },
        );
        MyTabViewer { data: hm, settings }
    }

    pub fn new_code_tab(&mut self, key: String, code: String) {
        self.data.insert(
            key,
            Code {
                code,
                file_path: None,
                file_name: None,
            },
        );
    }

    pub fn open_code_tab(&mut self, key: &str) -> Option<&Code> {
        self.data.get(key)
    }

    pub fn mut_code(&mut self, key: String) -> &mut String {
        &mut self
            .data
            .entry(key)
            .or_insert(Code {
                code: String::new(),
                file_path: None,
                file_name: None,
            })
            .code
    }

    pub fn save_code(&mut self, key: &str) -> Result<(), String> {
        if let Some(code) = self.data.get(key) {
            if let Some(path) = &code.file_path {
                fs::write(path, &code.code).map_err(|e| e.to_string())
            } else {
                self.save_new_code(key, code.code.clone())
            }
        } else {
            Err("No code found for key.".to_string())
        }
    }

    pub fn save_new_code(&mut self, key: &str, code: String) -> Result<(), String> {
        let save = rfd::FileDialog::new()
            .add_filter("Quant Query Language", &["qql"])
            .save_file();
        let opened_file = self
            .data
            .get_mut(key)
            .ok_or("No opened_file found for key")?;
        opened_file.file_name = Some(
            PathBuf::from(save.as_ref().unwrap().as_path())
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        );
        opened_file.file_path = save.map(|p| p.to_string_lossy().into_owned());
        fs::write(opened_file.file_path.as_ref().unwrap(), &opened_file.code)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn open_file(&mut self, key: &str, path: &str) -> Result<(), String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.data.insert(
            key.to_string(),
            Code {
                code: content,
                file_path: Some(path.to_string()),
                file_name: Some(
                    PathBuf::from(path)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into(),
                ),
            },
        );
        Ok(())
    }
}

impl TabViewer for MyTabViewer {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        if tab.as_str() == "0" {
            return WidgetText::from("Welcome to Q Studio");
        }
        if tab.as_str() == "1" {
            return WidgetText::from("Settings");
        }
        let s = self
            .data
            .get(tab.as_str())
            .map(|code| code.file_name.clone())
            .unwrap_or_default();

        let edited = self
            .data
            .get(tab.as_str())
            .map(|code| code.files_been_edited())
            .unwrap_or(false);

        let title = if edited {
            format!("{}*", s.unwrap_or_else(|| "Untitled".to_string()))
        } else {
            s.unwrap_or_else(|| "Untitled".to_string())
        };

        WidgetText::from(title)
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        let rows = (ui.available_height() / 18.0).floor() as usize;
        match tab {
            TabKind::Code(key) => {
                CodeEditor::default()
                    .id_source("code editor")
                    .with_rows(rows)
                    .with_fontsize(14.0)
                    .with_theme(if ui.ctx().style().visuals.dark_mode {
                        ColorTheme::GITHUB_DARK
                    } else {
                        ColorTheme::GITHUB_LIGHT
                    })
                    .with_syntax(Syntax::qql())
                    .with_numlines(true)
                    .show(ui, self.mut_code(key.clone()));
            }
            TabKind::Markdown(_) => {
                let text = include_str!("../../../qql.md");
                let mut cache = CommonMarkCache::default();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let md = CommonMarkViewer::new();
                    md.show(ui, &mut cache, text);
                });
            }
            TabKind::Settings(_) => {
                ui.label("Settings");
                ui.separator();
                let mut settings = self.settings.borrow_mut();
                ui.checkbox(&mut settings.dark_mode, "Enable Dark Mode");
            }
        }
    }
}
