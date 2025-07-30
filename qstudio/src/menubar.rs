use crate::random_string;
use crate::QStudio;

use egui::Ui;
use crate::TabKind;

pub fn menubar(ui: &mut egui::Ui, app: &mut QStudio) {
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("New").clicked() {
                let s: String = random_string();
                app.dock_state
                    .push_to_focused_leaf(TabKind::Code(s.clone()));
            }
            if ui.button("Open").clicked() {
                let path = rfd::FileDialog::new()
                    .add_filter("Quant Query Files", &["qql"])
                    .add_filter("All files", &["*"])
                    .pick_file();

                if path.is_none() {
                    return;
                }
                let path = path.unwrap().to_string_lossy().into_owned();
                let s = random_string();
                if let Err(e) = app.tab_viewer.open_file(&s, &path) {
                    eprintln!("Failed to open file: {}", e);
                    return;
                }

                let s: String = random_string();
                let _ = app.tab_viewer.open_file(&s, &path);
                app.dock_state.push_to_focused_leaf(TabKind::Code(s));
            }
            if ui.button("Save").clicked() {}
        });

        ui.menu_button("Tools", |ui| {
            if ui
                .button("Run Query")
                .on_hover_text("Execute the query in the editor")
                .clicked()
            {
                app.run_query();
            }
            if ui.button("Debug").clicked() {
                app.debug_panel = !app.debug_panel;
            }
        });

        ui.menu_button("View", |ui| {
            if ui.button("Cheat Sheet").clicked() {
                app.dock_state
                    .push_to_focused_leaf(TabKind::Markdown("0".to_string()));
            }
            ui.separator();
            if ui.button("Settings").clicked() {
                app.dock_state
                    .push_to_focused_leaf(TabKind::Settings("1".to_string()));
            }
        });
    });
}
