use egui_code_editor::{CodeEditor, ColorTheme, Syntax};
use std::sync::{Arc, Mutex};

pub fn code_editor(ui: &mut egui::Ui, file_path: String, buffer: &mut String) {
    let rows = (ui.available_height() / 14.0).floor() as usize;

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
