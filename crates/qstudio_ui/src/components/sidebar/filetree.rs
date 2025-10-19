use busbar::Aluminum;
use egui::*;
use events::Event;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use qstudio_tcp::Client;

// ---- Your Fs model comes from events::events::files ----
type Fs = events::events::files::Fs;

// ===== FolderTree: pure UI state & rendering =====

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortMode {
    NameAsc,
    NameDesc,
    FoldersFirst,
    ModifiedNewest,
    ModifiedOldest,
}

// Helper functions for sorting
fn a_name(node: &Fs) -> std::borrow::Cow<'_, str> {
    match node {
        Fs::Folder { name, .. } | Fs::File { name, .. } => name.to_lowercase().into(),
    }
}
fn b_name(node: &Fs) -> std::borrow::Cow<'_, str> {
    a_name(node)
}
fn a_mod(node: &Fs) -> Option<std::time::SystemTime> {
    match node {
        Fs::Folder { modified, .. } | Fs::File { modified, .. } => *modified,
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum FileCmd {
    NewFile { parent: PathBuf },
    NewFolder { parent: PathBuf },
    DeleteMany { targets: Vec<PathBuf> },
    Move { from: PathBuf, to_dir: PathBuf },
    Rename { from: PathBuf, new_name: String },
    Open { path: PathBuf }, // double-click file
}

#[derive(Debug, Clone)]
struct FolderTree {
    // Latest snapshot (immutable; usually an Arc from your backend)
    file_system: Option<Arc<Fs>>,

    // UI-only state
    root_dir: PathBuf,
    selection: HashSet<PathBuf>,
    last_clicked: Option<PathBuf>,
    dragging: Option<Vec<PathBuf>>,
    renaming: Option<PathBuf>,
    rename_buf: String,
    filter_text: String,
    sort_mode: SortMode,
    show_hidden: bool,

    filetree_aluminum: Arc<Aluminum<(Client, Event)>>,
    only_client: Client,
}

impl FolderTree {
    fn new(filetree_aluminum: Arc<Aluminum<(Client, Event)>>, only_client: Client) -> Self {
        Self {
            file_system: None,
            root_dir: PathBuf::new(),
            selection: HashSet::new(),
            last_clicked: None,
            dragging: None,
            renaming: None,
            rename_buf: String::new(),
            filter_text: String::new(),
            sort_mode: SortMode::FoldersFirst,
            show_hidden: false,
            filetree_aluminum,
            only_client,
        }
    }

    fn set_snapshot(&mut self, new_fs: Arc<Fs>) {
        self.file_system = Some(new_fs);
        // (optional) prune selection to existing paths here
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.ui_with(ui, |_cmd| {});
    }

    fn ui_with(&mut self, ui: &mut egui::Ui, mut on_cmd: impl FnMut(FileCmd)) {
        ui.set_width(ui.available_width() - 4.0);
        ui.set_height(ui.available_height() - 4.0);
        // --- toolbar ---
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.set_height(ui.available_height());
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.heading("File Tree");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_sized(
                        [160.0, 22.0],
                        TextEdit::singleline(&mut self.filter_text).hint_text("Filter…"),
                    );
                });
            });
            ui.add_space(6.0);

            ScrollArea::horizontal()
                .id_salt("filetree-toolbar-scroll")
                .max_width(ui.available_width())
                .max_height(48.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ComboBox::from_label("Sort")
                            .selected_text(match self.sort_mode {
                                SortMode::FoldersFirst => "Folders first",
                                SortMode::NameAsc => "Name ↑",
                                SortMode::NameDesc => "Name ↓",
                                SortMode::ModifiedNewest => "Modified (newest)",
                                SortMode::ModifiedOldest => "Modified (oldest)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.sort_mode,
                                    SortMode::FoldersFirst,
                                    "Folders first",
                                );
                                ui.selectable_value(
                                    &mut self.sort_mode,
                                    SortMode::NameAsc,
                                    "Name ↑",
                                );
                                ui.selectable_value(
                                    &mut self.sort_mode,
                                    SortMode::NameDesc,
                                    "Name ↓",
                                );
                                ui.selectable_value(
                                    &mut self.sort_mode,
                                    SortMode::ModifiedNewest,
                                    "Modified (newest)",
                                );
                                ui.selectable_value(
                                    &mut self.sort_mode,
                                    SortMode::ModifiedOldest,
                                    "Modified (oldest)",
                                );
                            });

                        ui.checkbox(&mut self.show_hidden, "Show hidden");
                        ui.separator();

                        // Actions emit commands; backend applies & later publishes a new snapshot
                        let target_parent = self
                            .single_selected_path()
                            .filter(|p| p.is_dir())
                            .unwrap_or(self.root_dir.clone());

                        if ui.button("New File").clicked() {
                            on_cmd(FileCmd::NewFile {
                                parent: target_parent.clone(),
                            });
                        }
                        if ui.button("New Folder").clicked() {
                            on_cmd(FileCmd::NewFolder {
                                parent: target_parent.clone(),
                            });
                        }

                        ui.separator();

                        let has_selection = !self.selection.is_empty();
                        if ui
                            .add_enabled(has_selection, Button::new("Delete Selected"))
                            .clicked()
                        {
                            on_cmd(FileCmd::DeleteMany {
                                targets: self.selection.iter().cloned().collect(),
                            });
                            self.selection.clear();
                        }

                        if ui.button("Clear Selection").clicked() {
                            self.selection.clear();
                        }
                    });
                    ui.separator();
                });

            ui.add_space(4.0);

            // --- tree ---
            ScrollArea::vertical().max_width(256.0).show(ui, |ui| {
                if let Some(fs_arc) = self.file_system.clone() {
                    // `fs_arc` is now owned by this scope; no borrow of `self` is held.
                    self.show_node(ui, fs_arc.as_ref(), &mut on_cmd);
                } else {
                    ui.label("No directory listing available.");
                }
            });

            // Global keybinds
            if ui.input(|i| i.key_pressed(Key::F2)) {
                if let Some(one) = self.single_selected_path() {
                    self.start_rename(one);
                }
            }
            if ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
                if !self.selection.is_empty() {
                    on_cmd(FileCmd::DeleteMany {
                        targets: self.selection.iter().cloned().collect(),
                    });
                    self.selection.clear();
                }
            }
            if ui.input(|i| i.pointer.any_released()) {
                self.dragging = None;
            }
        });
    }

    fn show_node(&mut self, ui: &mut egui::Ui, node: &Fs, on_cmd: &mut impl FnMut(FileCmd)) {
        if !self.visible_by_filter(node) {
            return;
        }

        match node {
            Fs::Folder {
                name,
                path,
                children,
                ..
            } => {
                let is_root = path == &self.root_dir;
                let is_renaming_here = self.renaming.as_ref().map_or(false, |p| p == path);

                let id = egui::Id::new(("folder", path));
                let header = CollapsingHeader::new(if is_renaming_here { " " } else { name })
                    .id_salt(id)
                    .default_open(false);

                let collapse = header.show(ui, |ui| {
                    if is_renaming_here {
                        self.rename_inline(ui, on_cmd);
                    }

                    for child in self.sorted_refs(children.iter().collect()) {
                        // `sorted_refs` returns `&Fs`, so pass `child` directly:
                        self.show_node(ui, child, on_cmd);
                        ui.add_space(4.0);
                    }
                });

                let resp = collapse.header_response;

                // highlight selection
                if self.selection.contains(path) {
                    ui.painter()
                        .rect_filled(resp.rect, 4.0, ui.visuals().selection.bg_fill);
                }

                // click/selection
                if resp.clicked() {
                    let additive =
                        ui.input(|i| i.modifiers).command || ui.input(|i| i.modifiers).ctrl;
                    self.toggle_select(path.clone(), additive);
                }

                // drag
                if resp.drag_started() {
                    self.ensure_selected(path.clone());
                    self.dragging = Some(self.selection.iter().cloned().collect());
                }

                // context
                resp.context_menu(|ui| {
                    if ui.button("New File…").clicked() {
                        on_cmd(FileCmd::NewFile {
                            parent: path.clone(),
                        });
                        ui.close();
                    }
                    if ui.button("New Folder…").clicked() {
                        on_cmd(FileCmd::NewFolder {
                            parent: path.clone(),
                        });
                        ui.close();
                    }
                    ui.separator();
                    if !is_root {
                        if ui.button("Rename…").clicked() {
                            self.start_rename(path.clone());
                            ui.close();
                        }
                        if ui.button("Delete…").clicked() {
                            on_cmd(FileCmd::DeleteMany {
                                targets: vec![path.clone()],
                            });
                            ui.close();
                        }
                    }
                });

                // drop target
                if self.dragging.is_some() && resp.hovered() {
                    ui.painter().rect_stroke(
                        resp.rect,
                        4.0,
                        egui::epaint::Stroke {
                            width: 1.0,
                            color: ui.visuals().selection.stroke.color,
                        },
                        egui::epaint::StrokeKind::Inside,
                    );
                }
                if resp.hovered() && ui.input(|i| i.pointer.any_released()) {
                    if let Some(items) = self.dragging.take() {
                        let to_dir = path.clone();
                        for from in items {
                            if &from != path && !is_descendant(&from, &to_dir) {
                                on_cmd(FileCmd::Move {
                                    from,
                                    to_dir: to_dir.clone(),
                                });
                            }
                        }
                    }
                }
            }
            Fs::File { name, path, .. } => {
                if self.renaming.as_ref().map_or(false, |p| p == path) {
                    self.rename_inline(ui, on_cmd);
                } else {
                    let selected = self.selection.contains(path);
                    let resp = ui.selectable_label(selected, name);

                    if resp.double_clicked() {
                        // this is where aluminum will send an event to the backend to open the file

                        let _ = self.filetree_aluminum.backend_tx.send((
                            self.only_client.clone(),
                            Event::DockEvent(events::events::dock::DockEvent::OpenFile {
                                path: path.to_string_lossy().into(),
                            }),
                        ));
                        let _ = self.filetree_aluminum.backend_tx.send((
                            self.only_client.clone(),
                            Event::EngineEvent(events::events::engine::EngineEvent::Start {
                                filename: path.to_string_lossy().into(),
                            }),
                        ));
                    }
                    if resp.clicked() {
                        let additive =
                            ui.input(|i| i.modifiers).command || ui.input(|i| i.modifiers).ctrl;
                        self.toggle_select(path.clone(), additive);
                    }

                    if resp.drag_started() {
                        self.ensure_selected(path.clone());
                        self.dragging = Some(self.selection.iter().cloned().collect());
                    }

                    resp.context_menu(|ui| {
                        if ui.button("Rename…").clicked() {
                            self.start_rename(path.clone());
                            ui.close();
                        }
                        if ui.button("Delete…").clicked() {
                            // delete all selected files, not just this one
                            ui.close();
                        }
                    });
                }
            }
        }
    }

    fn rename_inline(&mut self, ui: &mut egui::Ui, on_cmd: &mut impl FnMut(FileCmd)) {
        ui.horizontal(|ui| {
            let te = ui.add(
                TextEdit::singleline(&mut self.rename_buf)
                    .desired_width(200.0)
                    .hint_text("New name"),
            );
            if te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.commit_rename_if_any(on_cmd);
            }
            if ui.button("Save").clicked() {
                self.commit_rename_if_any(on_cmd);
            }
            if ui.button("Cancel").clicked() {
                self.cancel_rename();
            }
        });
    }

    fn commit_rename_if_any(&mut self, on_cmd: &mut impl FnMut(FileCmd)) {
        if let Some(from) = self.renaming.take() {
            let new_name = std::mem::take(&mut self.rename_buf);
            if !new_name.trim().is_empty()
                && new_name != from.file_name().unwrap_or_default().to_string_lossy()
            {
                on_cmd(FileCmd::Rename { from, new_name });
            }
        }
    }

    // --- misc helpers ---

    fn visible_by_filter(&self, node: &Fs) -> bool {
        let name = match node {
            Fs::Folder { name, .. } | Fs::File { name, .. } => name,
        };
        if !self.show_hidden && name.starts_with('.') {
            return false;
        }
        let needle = self.filter_text.trim();
        if needle.is_empty() {
            return true;
        }
        let needle = needle.to_lowercase();
        self.node_or_descendant_contains(node, &needle)
    }

    fn node_or_descendant_contains(&self, node: &Fs, needle_lower: &str) -> bool {
        let name = match node {
            Fs::Folder { name, .. } | Fs::File { name, .. } => name,
        };
        if name.to_lowercase().contains(needle_lower) {
            return true;
        }
        if let Fs::Folder { children, .. } = node {
            children
                .iter()
                .any(|c| self.node_or_descendant_contains(c, needle_lower))
        } else {
            false
        }
    }

    fn sorted_refs<'a>(&self, mut v: Vec<&'a Fs>) -> Vec<&'a Fs> {
        use std::cmp::Ordering::*;
        v.sort_by(|a, b| {
            let (a_is_file, b_is_file) =
                (matches!(a, Fs::File { .. }), matches!(b, Fs::File { .. }));
            match self.sort_mode {
                SortMode::FoldersFirst => match (a_is_file, b_is_file) {
                    (false, true) => Less,
                    (true, false) => Greater,
                    _ => a_name(a).cmp(&b_name(b)),
                },
                SortMode::NameAsc => a_name(a).cmp(&b_name(b)),
                SortMode::NameDesc => b_name(b).cmp(&a_name(a)),
                SortMode::ModifiedNewest => a_mod(b).cmp(&a_mod(a)), // newer first
                SortMode::ModifiedOldest => a_mod(a).cmp(&a_mod(b)),
            }
        });
        v
    }

    fn single_selected_path(&self) -> Option<PathBuf> {
        if self.selection.len() == 1 {
            self.selection.iter().next().cloned()
        } else {
            None
        }
    }

    fn start_rename(&mut self, path: PathBuf) {
        self.renaming = Some(path.clone());
        self.rename_buf = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
    }

    fn cancel_rename(&mut self) {
        self.renaming = None;
        self.rename_buf.clear();
    }

    fn toggle_select(&mut self, path: PathBuf, additive: bool) {
        if additive {
            if !self.selection.insert(path.clone()) {
                self.selection.remove(&path);
            }
        } else {
            self.selection.clear();
            self.selection.insert(path);
        }
        self.last_clicked = self.selection.iter().next().cloned();
    }

    fn ensure_selected(&mut self, path: PathBuf) {
        if !self.selection.contains(&path) {
            self.selection.clear();
            self.selection.insert(path);
        }
    }
}

fn is_descendant(ancestor: &Path, candidate_child: &Path) -> bool {
    let mut c = candidate_child;
    while let Some(p) = c.parent() {
        if p == ancestor {
            return true;
        }
        c = p;
    }
    false
}

// ===== FileTreeUi: integrates FolderTree with your Aluminum<Event> bus =====

#[derive(Debug, Clone)]
pub struct FileTreeUi {
    pub get_initial_listing: bool,
    filetree_aluminum: Arc<Aluminum<(Client, Event)>>,
    tree: FolderTree,
    _only_client: Client,
}

impl FileTreeUi {
    pub fn new(filetree_aluminum: Arc<Aluminum<(Client, Event)>>, only_client: Client) -> Self {
        Self {
            get_initial_listing: false,
            filetree_aluminum: Arc::clone(&filetree_aluminum),
            tree: FolderTree::new(Arc::clone(&filetree_aluminum), only_client.clone()),
            _only_client: only_client,
        }
    }

    /// Read-only draw (no commands emitted).
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.pump_snapshots(ui);
        self.tree.ui(ui);
    }

    /// Draw with a command handler. Pipe the `FileCmd` into your backend.
    pub fn _ui_with(&mut self, ui: &mut egui::Ui, mut on_cmd: impl FnMut(FileCmd)) {
        self.pump_snapshots(ui);
        self.tree.ui_with(ui, move |cmd| on_cmd(cmd));
    }

    fn pump_snapshots(&mut self, ui: &mut egui::Ui) {
        // Drain all pending messages and keep only the latest Fs snapshot
        let mut updated = false;
        while let Ok(ev) = self.filetree_aluminum.filetree_rx.try_recv() {
            if let Event::FileEvent(events::events::files::FileEvent::DirectoryListing {
                listing,
                ..
            }) = ev.1
            {
                if let Some(list) = listing {
                    // Prefer Arc so we don’t deep-clone large trees
                    self.tree.set_snapshot(Arc::new(list));
                    updated = true;
                }
            }
        }
        if updated {
            ui.ctx().request_repaint();
        }
    }
}
