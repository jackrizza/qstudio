use crate::{EngineEvent, Notification};
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};
use std::sync::{Arc, Mutex};

use crate::Channels;

pub fn code_editor(
    ui: &mut egui::Ui,
    file_path: String,
    buffer: &mut String,
    channels: Arc<Channels>,
) {
    let rows = (ui.available_height() / 14.0).floor() as usize;

    let ctx = ui.ctx();
    if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command) {
        // Save the buffer to the file
        if let Err(e) = std::fs::write(&file_path, buffer.clone()) {
            channels
                .senders()
                .notification_tx()
                .send(Notification::Error(format!("Failed to save file: {}", e)))
                .unwrap();
        } else {
            channels
                .senders()
                .engine_tx()
                .send(Mutex::new(EngineEvent::UpdateSource(file_path.clone())))
                .unwrap();
            channels
                .senders()
                .notification_tx()
                .send(Notification::Success(format!("Saved file: {}", file_path)))
                .unwrap();
        }
    }

    // If Command+R is pressed, restart connected engine
    if ctx.input(|i| i.key_pressed(egui::Key::R) && i.modifiers.command) {
        channels
            .senders()
            .engine_tx()
            .send(Mutex::new(EngineEvent::Restart(file_path.clone())))
            .unwrap();
        channels
            .senders()
            .notification_tx()
            .send(Notification::Success("Restarting engine...".to_string()))
            .unwrap();
    }

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
        .show(ui, buffer);
}
