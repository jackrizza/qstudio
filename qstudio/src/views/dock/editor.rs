use egui_code_editor::{CodeEditor, ColorTheme, Syntax};
use std::sync::Arc;
use theme::Theme;
use crate::{Notification, EngineEvent};

use crate::Channels;

pub fn code_editor(
    ui: &mut egui::Ui,
    file_path: String,
    buffer: &mut String,
    channels: Arc<Channels>,
) {
    let rows = (ui.available_height() / 18.0).floor() as usize;

    let ctx = ui.ctx();
    if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command) {
        // Save the buffer to the file
        if let Err(e) = std::fs::write(&file_path, buffer.clone()) {
            channels
                .notification_tx
                .lock()
                .unwrap()
                .send(Notification::Error(format!("Failed to save file: {}", e)))
                .unwrap();
        } else {
            channels
                .notification_tx
                .lock()
                .unwrap()
                .send(Notification::Success(format!("Saved file: {}", file_path)))
                .unwrap();
        }
    }

    // If Command+R is pressed, restart connected engine
    if ctx.input(|i| i.key_pressed(egui::Key::R) && i.modifiers.command) {
        channels
            .engine_tx
            .lock()
            .unwrap()
            .send(EngineEvent::Restart(file_path.clone()))
            .unwrap();
        channels
            .notification_tx
            .lock()
            .unwrap()
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
