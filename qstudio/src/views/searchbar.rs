use chat_gpt_rs::prelude::*;
use dotenv::dotenv;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::models::ui;

const WRAPPER_PROMPT: &str = r#"
You are a JSON-only API.  
Respond with a single valid JSON object that parses without errors.  
Never include text outside JSON, no markdown fences, no comments.

Rules:
1) "status" MUST be one of: "markdown", "event", "error".
2) For "markdown": include only a "body" string containing **fully prewritten markdown**. Escape quotes/newlines for valid JSON.
3) For "event": include:
   - "name": string (machine-friendly identifier)
   - "payload": object (structured data for the app)
4) For "error": include:
   - "code": string (stable, machine-parsable)
   - "message": string (human-readable)
   - "details": object | null
5) Always include "meta":
   - "request_id": string (echo or synthesize an ID if not provided)
   - "timestamp": ISO-8601 string (UTC)
6) No trailing commas. Use null for unused fields.

Schema (shape, not JSON Schema):
{
  "status": "markdown" | "event" | "error",
  "markdown": { "body": string } | null,
  "event": { "name": string, "payload": object } | null,
  "error": { "code": string, "message": string, "details": object|null } | null,
  "meta": { "request_id": string, "timestamp": string }
}

Now respond to the following request as JSON only:
"#;

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

pub struct SearchBar {
    pub query: String,
    expanded: bool,
    answer: Arc<Mutex<Answered>>,
}

impl SearchBar {
    pub fn new() -> Self {
        SearchBar {
            query: String::new(),
            expanded: false,
            answer: Arc::new(Mutex::new(Answered::None)),
        }
    }

    pub fn reset(&mut self) {
        self.query.clear();
        self.expanded = false;
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
        let mut answer = String::new();

        let env_token = env::var("CHAT_GPT_API_KEY").unwrap_or_else(|_| "YOUR_API_KEY".into());
        let token = Token::new(env_token);
        let api = Api::new(token);
        let request = Request {
            model: Model::Gpt4,
            messages: vec![Message {
                role: "user".to_string(),
                content: format!("{}\n{}", WRAPPER_PROMPT, query),
            }],
            ..Default::default()
        };

        {
            let mut answer_lock = self.answer.lock().unwrap();
            *answer_lock = Answered::Pending;
        }

        let answer_arc = Arc::clone(&self.answer);
        thread::spawn(move || {
            let string = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let res = api.chat(request).await;
                    match res {
                        Ok(response) => {
                            if let Some(answer) = response.choices.first() {
                                answer.message.content.clone()
                            } else {
                                "No answer received".to_string()
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            "Error occurred while fetching answer".to_string()
                        }
                    }
                });

            let mut answer_lock = answer_arc.lock().unwrap();
            match serde_json::from_str::<GptApiResponse>(&string) {
                Ok(parsed_response) => *answer_lock = Answered::Yes(parsed_response),
                Err(e) => {
                    eprintln!("Failed to parse response: {}", e);
                    *answer_lock = Answered::Error(format!("Failed to parse response: {}", e));
                }
            }
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

                let response = ui
                    .add(egui::TextEdit::singleline(&mut self.query).desired_width(f32::INFINITY));

                if self.expanded {
                    let mut answer_lock = self.answer.lock().unwrap();
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
            });
    }
}
