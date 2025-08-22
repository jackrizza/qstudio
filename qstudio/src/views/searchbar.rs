use chat_gpt_rs::prelude::*;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread;
use std::{env, fs};

use egui_material_icons::icons::ICON_FILE_OPEN;
use std::path::Path;

const WRAPPER_PROMPT: &str = include_str!("../../../prompts/text_wrapper_prompt.txt");
const LANGUAGE_MANUAL: &str = include_str!("../../../qql.md");
const LANGUAGE_WRAPPER: &str = include_str!("../../../prompts/code_wrapper_prompt.txt");

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Markdown,
    Event,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Markdown {
    pub body: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub name: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    pub request_id: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GptApiResponse {
    pub status: Status,
    pub markdown: Option<Markdown>,
    pub event: Option<Event>,
    pub error: Option<ErrorDetail>,
    pub meta: Meta,
}

impl GptApiResponse {
    pub fn ui(&self, ui: &mut egui::Ui) {
        match self.status {
            Status::Markdown => {
                if let Some(markdown) = &self.markdown {
                    let md = CommonMarkViewer::new();
                    let mut cache = CommonMarkCache::default();
                    // Replace escaped newlines with actual newlines before rendering
                    let body = markdown.body.replace("\\n", "\n");
                    md.show(ui, &mut cache, &body);
                }
            }
            Status::Event => {
                if let Some(event) = &self.event {
                    ui.label(format!("Event Name: {}", event.name));
                    ui.label(format!("Payload: {:?}", event.payload));
                }
            }
            Status::Error => {
                if let Some(error) = &self.error {
                    ui.label(format!("Code: {}", error.code));
                    ui.label(format!("Message: {}", error.message));
                    if let Some(details) = &error.details {
                        ui.label(format!("Details: {:?}", details));
                    }
                }
            }
        }
    }
}

pub enum Answered {
    Yes(GptApiResponse),
    Pending,
    Error(String),
    None,
}

impl Answered {
    pub fn ui(&self, ui: &mut egui::Ui) {
        match self {
            Answered::Yes(response) => response.ui(ui),
            Answered::Pending => {
                ui.label("Waiting for response...");
            }
            Answered::Error(err) => {
                ui.label(format!("Error: {}", err));
            }
            Answered::None => {}
        };
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchMode {
    Text,
    File(String),
}

pub struct SearchBar {
    pub search_mode: SearchMode,
    pub query: String,
    pub expanded: bool,
    answer: Arc<Mutex<Answered>>,
}

impl SearchBar {
    pub fn new() -> Self {
        SearchBar {
            search_mode: SearchMode::Text,
            query: String::new(),
            expanded: false,
            answer: Arc::new(Mutex::new(Answered::None)),
        }
    }

    pub fn reset(&mut self) {
        self.query.clear();
        self.expanded = false;
        self.search_mode = SearchMode::Text;
    }

    fn window_height(&self, ctx: &egui::Context) -> f32 {
        if self.expanded {
            ctx.screen_rect().height() * 0.8 // 80% of window height when expanded
        } else {
            0.0 // No height when not expanded
        }
    }

    fn chat_gpt_blocking_on_seperate_thread(&mut self) {
        let query = self.query.clone();

        let env_token = env::var("CHAT_GPT_API_KEY").unwrap_or_else(|_| "YOUR_API_KEY".into());
        let token = Token::new(env_token);
        let api = Api::new(token);

        let mut content = String::new();

        log::info!("Search Mode: {:?}", self.search_mode);
        if let SearchMode::File(ref file_path) = self.search_mode {
            if let Some(file_name) = Path::new(file_path).file_name() {
                fs::read_to_string(file_path)
                    .map(|file_content| {
                        content.push_str(LANGUAGE_WRAPPER);
                        content.push_str(LANGUAGE_MANUAL);
                        content.push_str(&format!("File: {}\n", file_name.to_string_lossy()));
                        content.push_str(&file_content);
                    })
                    .unwrap_or_else(|_| {
                        log::error!("Failed to read file: {}", file_path);
                        content.push_str(&format!("Failed to read file: {}\n", file_path));
                    });
            }
        } else {
            content.push_str(&format!("{}\n", WRAPPER_PROMPT));
        }

        content.push_str("USER PROMPT : ");
        content.push_str(&query);

        fs::write("exported_prompt.txt", &content).unwrap();

        let request = Request {
            model: Model::Gpt4,
            messages: vec![Message {
                role: "user".to_string(),
                content: content,
            }],
            ..Default::default()
        };

        {
            let mut answer_lock = self.answer.lock().unwrap();
            *answer_lock = Answered::Pending;
        }

        let answer_arc = Arc::clone(&self.answer);
        thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let res = api.chat(request).await;
                    let answer = match res {
                        Ok(response) => {
                            if let Some(answer) = response.choices.first() {
                                log::info!("Received answer: {:#?}", answer.message.content);
                                answer.message.content.clone()
                            } else {
                                "No answer received".to_string()
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            "Error occurred while fetching answer".to_string()
                        }
                    };

                    let answer = answer.trim().replace("\r", "\\r").replace("\n", "\\n"); // escape newlines
                    let parsed = serde_json::from_str::<GptApiResponse>(&answer);
                    match parsed {
                        Ok(parsed_response) => {
                            log::info!("Parsed response: {:#?}", parsed_response);
                            let mut answer_lock = answer_arc.lock().unwrap();
                            *answer_lock = Answered::Yes(parsed_response);
                        }
                        Err(e) => {
                            log::error!("Failed to parse JSON response: {}", e);
                            let mut answer_lock = answer_arc.lock().unwrap();
                            *answer_lock =
                                Answered::Error(format!("Failed to parse JSON response: {}", e));
                        }
                    }
                })
        });
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("Search")
            .collapsible(false)
            .min_height(self.window_height(ctx))
            .resizable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                // Set font size to 14
                let mut style = (*ctx.style()).clone();
                style
                    .text_styles
                    .get_mut(&egui::TextStyle::Body)
                    .map(|font| font.size = 24.0);
                ui.set_style(style);

                // Set width to 40% of the window width
                let window_width = ctx.screen_rect().width();
                let desired_width = window_width * 0.4;
                ui.set_min_width(desired_width);

                if let SearchMode::File(file_path) = &self.search_mode {
                    let file_name = Path::new(file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(file_path);

                    ui.horizontal(|ui| {
                        ui.label(ICON_FILE_OPEN);
                        ui.label(file_name);
                    });
                }
                let response = ui
                    .add(egui::TextEdit::singleline(&mut self.query).desired_width(f32::INFINITY));

                if self.expanded {
                    let answer_lock = self.answer.lock().unwrap();
                    ui.add_space(8.0);
                    // Set font size to 12
                    let mut style = (*ctx.style()).clone();
                    style
                        .text_styles
                        .get_mut(&egui::TextStyle::Body)
                        .map(|font| font.size = 14.0);
                    ui.set_style(style);

                    egui::ScrollArea::vertical()
                        .max_height(self.window_height(ctx) - 60.0)
                        .show(ui, |ui| {
                            answer_lock.ui(ui);
                        });
                }

                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    // Handle Enter key pressed
                    // For example, collapse the search bar:
                    self.expanded = true;
                    self.chat_gpt_blocking_on_seperate_thread();
                    println!("Search query: {}", self.query);
                    // Or trigger your search logic here
                }

                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.reset();
                }

                
            });
    }
}
