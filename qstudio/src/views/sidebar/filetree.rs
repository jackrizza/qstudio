use crate::models::ui::UIEvent;
use crate::Channels;

use egui::epaint::StrokeKind;
use egui::*;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

// ---------- Model ----------

#[derive(Debug, Clone)]
pub enum Fs {
    Folder {
        name: String,
        path: PathBuf,
        modified: Option<SystemTime>,
        children: Vec<Fs>,
    },
    File {
        name: String,
        path: PathBuf,
        modified: Option<SystemTime>,
        size: Option<u64>,
    },
}

impl Fs {
    pub fn is_file(&self) -> bool {
        matches!(self, Fs::File { .. })
    }
    pub fn is_dir(&self) -> bool {
        matches!(self, Fs::Folder { .. })
    }
    pub fn path(&self) -> &Path {
        match self {
            Fs::Folder { path, .. } | Fs::File { path, .. } => path.as_path(),
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Fs::Folder { name, .. } | Fs::File { name, .. } => name,
        }
    }
    pub fn modified(&self) -> Option<SystemTime> {
        match self {
            Fs::Folder { modified, .. } | Fs::File { modified, .. } => *modified,
        }
    }
    pub fn children(&self) -> &[Fs] {
        match self {
            Fs::Folder { children, .. } => children,
            _ => &[],
        }
    }
}

#[derive(Debug, Clone)]
enum ChangeFS {
    AddFile { parent_dir: PathBuf, name: String },
    AddFolder { parent_dir: PathBuf, name: String },
    Move { from: PathBuf, to_dir: PathBuf },
    RemoveMany { targets: Vec<PathBuf> },
    Rename { from: PathBuf, new_name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortMode {
    NameAsc,
    NameDesc,
    FoldersFirst,
    ModifiedNewest,
    ModifiedOldest,
}

// ---------- Component State ----------

#[derive(Debug, Clone)]
pub struct FolderTree {
    root_dir: PathBuf,
    file_system: Arc<Fs>,

    // Pending FS ops (applied after UI)
    pending_ops: Vec<ChangeFS>,

    // Selection & drag
    selection: HashSet<PathBuf>,
    last_clicked: Option<PathBuf>,
    dragging: Option<Vec<PathBuf>>, // supports dragging multiple

    // “New …”
    pending_new: Option<(PathBuf, bool)>, // (parent, is_folder)
    new_name_buf: String,

    // Rename
    renaming: Option<PathBuf>,
    rename_buf: String,

    // Delete confirmation (for multi-delete)
    confirm_delete: Option<Vec<PathBuf>>,

    // Filter / sort
    filter_text: String,
    sort_mode: SortMode,
    show_hidden: bool,
}

impl FolderTree {
    pub fn new(file_path: String) -> Self {
        let root = PathBuf::from(file_path);
        let file_system = Arc::new(Self::build_fs_tree(&root));
        Self {
            root_dir: root,
            file_system,
            pending_ops: Vec::new(),
            selection: HashSet::new(),
            last_clicked: None,
            dragging: None,
            pending_new: None,
            new_name_buf: String::new(),
            renaming: None,
            rename_buf: String::new(),
            confirm_delete: None,
            filter_text: String::new(),
            sort_mode: SortMode::FoldersFirst,
            show_hidden: false,
        }
    }

    // ---------- Filesystem ops ----------

    fn queue(&mut self, op: ChangeFS) {
        self.pending_ops.push(op);
    }

    fn apply_pending(&mut self) {
        if self.pending_ops.is_empty() {
            return;
        }
        let ops = std::mem::take(&mut self.pending_ops);
        for op in ops {
            self.execute(op);
        }
        self.refresh();
        self.prune_selection_nonexistent();
    }

    fn execute(&mut self, change: ChangeFS) {
        match change {
            ChangeFS::AddFile { parent_dir, name } => {
                let path = parent_dir.join(name);
                if let Some(p) = path.parent() {
                    let _ = fs::create_dir_all(p);
                }
                if let Err(e) = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&path)
                {
                    log::error!("create file {:?}: {}", path, e);
                }
            }
            ChangeFS::AddFolder { parent_dir, name } => {
                let dir = parent_dir.join(name);
                if let Err(e) = fs::create_dir_all(&dir) {
                    log::error!("create folder {:?}: {}", dir, e);
                }
            }
            ChangeFS::Move { from, to_dir } => {
                if is_descendant(&from, &to_dir) {
                    log::warn!(
                        "Skipping move: cannot drop into descendant: {:?} -> {:?}",
                        from,
                        to_dir
                    );
                    return;
                }
                let to = to_dir.join(from.file_name().unwrap_or_default());
                if to.exists() {
                    log::warn!("Destination exists, skipping: {:?}", to);
                    return;
                }
                if let Err(e) = fs::rename(&from, &to) {
                    log::error!("move {:?} -> {:?}: {}", from, to, e);
                }
            }
            ChangeFS::RemoveMany { targets } => {
                for target in targets {
                    let res = if target.is_dir() {
                        fs::remove_dir_all(&target)
                    } else {
                        fs::remove_file(&target)
                    };
                    if let Err(e) = res {
                        log::error!("remove {:?}: {}", target, e);
                    }
                }
            }
            ChangeFS::Rename { from, new_name } => {
                let parent = match from.parent() {
                    Some(p) => p.to_path_buf(),
                    None => {
                        log::warn!("Cannot rename root {:?}", from);
                        return;
                    }
                };
                if new_name.is_empty() {
                    return;
                }
                let to = parent.join(new_name);
                if to.exists() {
                    log::warn!("Rename target exists: {:?}", to);
                    return;
                }
                if let Err(e) = fs::rename(&from, &to) {
                    log::error!("rename {:?} -> {:?}: {}", from, to, e);
                }
            }
        }
    }

    pub fn refresh(&mut self) {
        self.file_system = Arc::new(Self::build_fs_tree(&self.root_dir));
    }

    fn prune_selection_nonexistent(&mut self) {
        self.selection.retain(|p| p.exists());
        if let Some(p) = &self.renaming {
            if !p.exists() {
                self.renaming = None;
                self.rename_buf.clear();
            }
        }
        if let Some(list) = &mut self.confirm_delete {
            list.retain(|p| p.exists());
            if list.is_empty() {
                self.confirm_delete = None;
            }
        }
    }

    // ---------- Build FS tree ----------

    fn build_fs_tree(path: &Path) -> Fs {
        let md = fs::metadata(path).ok();
        let modified = md.as_ref().and_then(|m| m.modified().ok());

        if path.is_dir() {
            let children: Vec<Fs> = match fs::read_dir(path) {
                Ok(rd) => rd
                    .filter_map(Result::ok)
                    .map(|e| Self::build_fs_tree(&e.path()))
                    .collect(),
                Err(_) => vec![],
            };
            Fs::Folder {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "root".to_string()),
                path: path.to_path_buf(),
                modified,
                children,
            }
        } else {
            let size = md.as_ref().map(|m| m.len());
            Fs::File {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                path: path.to_path_buf(),
                modified,
                size,
            }
        }
    }

    // ---------- UI helpers ----------

    fn visible_by_filter(&self, node: &Fs) -> bool {
        if !self.show_hidden && node.name().starts_with('.') {
            return false;
        }
        if self.filter_text.trim().is_empty() {
            return true;
        }
        let needle = self.filter_text.to_lowercase();
        self.node_or_descendant_contains(node, &needle)
    }

    fn node_or_descendant_contains(&self, node: &Fs, needle_lower: &str) -> bool {
        if node.name().to_lowercase().contains(needle_lower) {
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

    fn sort_children_owned(&self, mut v: Vec<Fs>) -> Vec<Fs> {
        match self.sort_mode {
            SortMode::FoldersFirst => v.sort_by(|a, b| {
                use std::cmp::Ordering::*;
                match (a.is_file(), b.is_file()) {
                    (false, true) => Less,
                    (true, false) => Greater,
                    _ => a.name().to_lowercase().cmp(&b.name().to_lowercase()),
                }
            }),
            SortMode::NameAsc => v.sort_by_key(|n| n.name().to_lowercase()),
            SortMode::NameDesc => {
                v.sort_by(|a, b| b.name().to_lowercase().cmp(&a.name().to_lowercase()))
            }
            SortMode::ModifiedNewest => v.sort_by(|a, b| b.modified().cmp(&a.modified())),
            SortMode::ModifiedOldest => v.sort_by(|a, b| a.modified().cmp(&b.modified())),
        }
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

    fn commit_rename_if_any(&mut self) {
        if let Some(from) = self.renaming.take() {
            let new_name = std::mem::take(&mut self.rename_buf);
            if !new_name.trim().is_empty()
                && new_name != from.file_name().unwrap_or_default().to_string_lossy()
            {
                self.queue(ChangeFS::Rename { from, new_name });
            }
        }
    }

    fn cancel_rename(&mut self) {
        self.renaming = None;
        self.rename_buf.clear();
    }

    fn select_all_visible(&mut self) {
        let mut stack = VecDeque::new();
        stack.push_back(self.file_system.as_ref());
        let needle = self.filter_text.to_lowercase();
        while let Some(node) = stack.pop_front() {
            if self.node_or_descendant_contains(node, &needle) && self.visible_by_filter(node) {
                self.selection.insert(node.path().to_path_buf());
                if let Fs::Folder { children, .. } = node {
                    for c in children {
                        stack.push_back(c);
                    }
                }
            }
        }
    }

    // ---------- Rendering a node (with interactions) ----------

    fn row_for_folder(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        path: &PathBuf,
        is_root: bool,
    ) -> Response {
        let selected = self.selection.contains(path);
        let resp = ui.add(egui::SelectableLabel::new(selected, name));

        // Selection logic
        if resp.clicked() {
            let modifiers = ui.input(|i| i.modifiers);
            if modifiers.command || modifiers.ctrl {
                if !self.selection.insert(path.clone()) {
                    self.selection.remove(path);
                }
                self.last_clicked = Some(path.clone());
            } else {
                self.selection.clear();
                self.selection.insert(path.clone());
                self.last_clicked = Some(path.clone());
            }
        }

        // Begin drag
        if resp.drag_started() {
            if !self.selection.contains(path) {
                self.selection.clear();
                self.selection.insert(path.clone());
            }
            self.dragging = Some(self.selection.iter().cloned().collect());
        }

        // Context menu
        resp.context_menu(|ui| {
            if ui.button("New File…").clicked() {
                self.pending_new = Some((path.clone(), false));
                self.new_name_buf.clear();
                ui.close();
            }
            if ui.button("New Folder…").clicked() {
                self.pending_new = Some((path.clone(), true));
                self.new_name_buf.clear();
                ui.close();
            }
            ui.separator();
            if !is_root {
                if ui.button("Rename…").clicked() {
                    self.start_rename(path.clone());
                    ui.close();
                }
                if ui.button("Delete…").clicked() {
                    self.confirm_delete = Some(vec![path.clone()]);
                    ui.close();
                }
            }
        });

        resp
    }

    fn row_for_file(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        path: &PathBuf,
        channels: Arc<Channels>,
    ) -> Response {
        let selected = self.selection.contains(path);
        let resp = ui.selectable_label(selected, name);

        // Open on double-click
        if resp.double_clicked() {
            channels
                .senders()
                .ui_tx
                .lock()
                .unwrap()
                .send(UIEvent::AddPane(crate::models::ui::UIEventPane::Text(
                    path.to_string_lossy().into(),
                )))
                .unwrap();
        }

        // Selection logic
        if resp.clicked() {
            let modifiers = ui.input(|i| i.modifiers);
            if modifiers.command || modifiers.ctrl {
                if !self.selection.insert(path.clone()) {
                    self.selection.remove(path);
                }
                self.last_clicked = Some(path.clone());
            } else {
                self.selection.clear();
                self.selection.insert(path.clone());
                self.last_clicked = Some(path.clone());
            }
        }

        // Drag
        if resp.drag_started() {
            if !self.selection.contains(path) {
                self.selection.clear();
                self.selection.insert(path.clone());
            }
            self.dragging = Some(self.selection.iter().cloned().collect());
        }

        // Context menu
        resp.context_menu(|ui| {
            if ui.button("Rename…").clicked() {
                self.start_rename(path.clone());
                ui.close();
            }
            if ui.button("Delete…").clicked() {
                self.confirm_delete = Some(vec![path.clone()]);
                ui.close();
            }
        });

        resp
    }

    fn show_node(&mut self, ui: &mut egui::Ui, node: &Fs, channels: Arc<Channels>) {
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
                let is_renaming_here = self.renaming.as_ref().map_or(false, |p| p == path);

                // Build header first
                let header =
                    CollapsingHeader::new(if is_renaming_here { " " } else { name.as_str() })
                        .default_open(false);

                let collapse = header.show(ui, |ui| {
                    if is_renaming_here {
                        ui.horizontal(|ui| {
                            let te = ui.add(
                                TextEdit::singleline(&mut self.rename_buf)
                                    .desired_width(200.0)
                                    .hint_text("New name"),
                            );
                            if te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                self.commit_rename_if_any();
                            }
                            if ui.button("Save").clicked() {
                                self.commit_rename_if_any();
                            }
                            if ui.button("Cancel").clicked() {
                                self.cancel_rename();
                            }
                        });
                    }

                    // Clone + sort owned children to avoid borrows while recursing
                    let children_sorted = self.sort_children_owned(children.clone());
                    for child in children_sorted {
                        self.show_node(ui, &child, channels.clone());
                        ui.add_space(4.0);
                    }
                });

                // Use header_response directly as our selectable/drag target
                let resp = collapse.header_response;

                // Paint selection background if selected
                if self.selection.contains(path) {
                    ui.painter()
                        .rect_filled(resp.rect, 4.0, ui.visuals().selection.bg_fill);
                }

                // Run our folder-row logic using the same interactions as resp:
                // selection:
                if resp.clicked() {
                    let modifiers = ui.input(|i| i.modifiers);
                    if modifiers.command || modifiers.ctrl {
                        if !self.selection.insert(path.clone()) {
                            self.selection.remove(path);
                        }
                        self.last_clicked = Some(path.clone());
                    } else {
                        self.selection.clear();
                        self.selection.insert(path.clone());
                        self.last_clicked = Some(path.clone());
                    }
                }

                // start drag
                if resp.drag_started() {
                    if !self.selection.contains(path) {
                        self.selection.clear();
                        self.selection.insert(path.clone());
                    }
                    self.dragging = Some(self.selection.iter().cloned().collect());
                }

                // context menu on header
                resp.context_menu(|ui| {
                    if ui.button("New File…").clicked() {
                        self.pending_new = Some((path.clone(), false));
                        self.new_name_buf.clear();
                        ui.close();
                    }
                    if ui.button("New Folder…").clicked() {
                        self.pending_new = Some((path.clone(), true));
                        self.new_name_buf.clear();
                        ui.close();
                    }
                    ui.separator();
                    let is_root = path == &self.root_dir;
                    if !is_root {
                        if ui.button("Rename…").clicked() {
                            self.start_rename(path.clone());
                            ui.close();
                        }
                        if ui.button("Delete…").clicked() {
                            self.confirm_delete = Some(vec![path.clone()]);
                            ui.close();
                        }
                    }
                });

                // Highlight as drop target when dragging
                if self.dragging.is_some() && resp.hovered() {
                    ui.painter().rect_stroke(
                        resp.rect,
                        4.0,
                        Stroke {
                            width: 1.0,
                            color: ui.visuals().selection.stroke.color,
                        },
                        StrokeKind::Inside,
                    );
                }

                // Drop into folder on release
                if resp.hovered() && ui.input(|i| i.pointer.any_released()) {
                    if let Some(items) = self.dragging.take() {
                        let to_dir = path.clone();
                        for from in items {
                            if &from != path && !is_descendant(&from, &to_dir) {
                                self.queue(ChangeFS::Move {
                                    from,
                                    to_dir: to_dir.clone(),
                                });
                            }
                        }
                    }
                }

                // Global rename shortcut (F2) if exactly one selected
                if ui.input(|i| i.key_pressed(Key::F2)) {
                    if let Some(one) = self.single_selected_path() {
                        self.start_rename(one);
                    }
                }
            }
            Fs::File { name, path, .. } => {
                // Inline rename for file
                if self.renaming.as_ref().map_or(false, |p| p == path) {
                    ui.horizontal(|ui| {
                        let te = ui.add(
                            TextEdit::singleline(&mut self.rename_buf)
                                .desired_width(200.0)
                                .hint_text("New name"),
                        );
                        if te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            self.commit_rename_if_any();
                        }
                        if ui.button("Save").clicked() {
                            self.commit_rename_if_any();
                        }
                        if ui.button("Cancel").clicked() {
                            self.cancel_rename();
                        }
                    });
                } else {
                    let resp = self.row_for_file(ui, name, path, channels);

                    // Global rename shortcut
                    if ui.input(|i| i.key_pressed(Key::F2)) {
                        if let Some(one) = self.single_selected_path() {
                            self.start_rename(one);
                        }
                    }
                }
            }
        }
    }

    // ---------- Public UI ----------

    pub fn ui(&mut self, ui: &mut egui::Ui, channels: Arc<Channels>) {
        Frame::new()
            .inner_margin(0.0)
            .outer_margin(0.0)
            .show(ui, |ui| {
                ui.set_min_height(ui.available_height());
                ui.set_max_width(256.0);
                ui.add_space(8.0);
                ui.vertical(|ui| {
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
                        .id_source("filetree-toolbar-scroll")
                        .max_width(256.0)
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

                                // Toolbar actions (create under selected folder or root)
                                let target_parent = self
                                    .single_selected_path()
                                    .filter(|p| p.is_dir())
                                    .unwrap_or(self.root_dir.clone());

                                if ui.button("New File").clicked() {
                                    self.pending_new = Some((target_parent.clone(), false));
                                    self.new_name_buf.clear();
                                }
                                if ui.button("New Folder").clicked() {
                                    self.pending_new = Some((target_parent.clone(), true));
                                    self.new_name_buf.clear();
                                }

                                ui.separator();

                                let has_selection = !self.selection.is_empty();
                                let del_btn = ui.add_enabled(has_selection, Button::new("Delete Selected"));
                                if del_btn.clicked() {
                                    self.confirm_delete = Some(self.selection.iter().cloned().collect());
                                }

                                if ui.button("Select All (filtered)").clicked() {
                                    self.selection.clear();
                                    self.select_all_visible();
                                }
                                if ui.button("Clear Selection").clicked() {
                                    self.selection.clear();
                                }
                            });

                            ui.separator();
                        });
                    ui.add_space(4.0);

                    // Scroll area with tree
                    ScrollArea::vertical()
                        // .auto_shrink([false; 2])
                        .max_width(256.0)
                        .show(ui, |ui| {
                            if let Fs::Folder { children, .. } = self.file_system.as_ref() {
                                let children_sorted = self.sort_children_owned(children.clone());
                                for child in children_sorted {
                                    self.show_node(ui, &child, channels.clone());
                                    ui.add_space(4.0);
                                }
                            }
                        });

                    // “New …” popup — clone to avoid borrow-in-closure conflicts
                    if let Some((parent_dir, is_folder)) = self.pending_new.clone() {
                        Window::new(if is_folder { "New Folder" } else { "New File" })
                            .collapsible(false)
                            .resizable(false)
                            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                            .show(ui.ctx(), |ui| {
                                ui.label(format!("Parent: {}", parent_dir.display()));
                                ui.add(
                                    TextEdit::singleline(&mut self.new_name_buf).hint_text("name"),
                                );
                                ui.horizontal(|ui| {
                                    if ui.button("Create").clicked()
                                        && !self.new_name_buf.trim().is_empty()
                                    {
                                        let name = self.new_name_buf.trim().to_string();
                                        if is_folder {
                                            self.queue(ChangeFS::AddFolder {
                                                parent_dir: parent_dir.clone(),
                                                name,
                                            });
                                        } else {
                                            self.queue(ChangeFS::AddFile {
                                                parent_dir: parent_dir.clone(),
                                                name,
                                            });
                                        }
                                        self.pending_new = None;
                                        self.new_name_buf.clear();
                                    }
                                    if ui.button("Cancel").clicked() {
                                        self.pending_new = None;
                                        self.new_name_buf.clear();
                                    }
                                });
                            });
                    }

                    // Delete confirmation — clone to avoid borrow-in-closure conflicts
                    if let Some(targets) = self.confirm_delete.clone() {
                        if !targets.is_empty() {
                            Window::new(if targets.len() == 1 {
                                "Delete item?"
                            } else {
                                "Delete items?"
                            })
                            .collapsible(false)
                            .resizable(false)
                            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                            .show(ui.ctx(), |ui| {
                                if targets.len() == 1 {
                                    ui.label(format!("Delete: {}", targets[0].display()));
                                } else {
                                    ui.label(format!("Delete {} items.", targets.len()));
                                }
                                ui.horizontal(|ui| {
                                    if ui.button("Delete").clicked() {
                                        self.queue(ChangeFS::RemoveMany {
                                            targets: targets.clone(),
                                        });
                                        for t in &targets {
                                            self.selection.remove(t);
                                        }
                                        self.confirm_delete = None;
                                    }
                                    if ui.button("Cancel").clicked() {
                                        self.confirm_delete = None;
                                    }
                                });
                            });
                        } else {
                            self.confirm_delete = None;
                        }
                    }

                    // Global keybinds (F2 for rename, Delete to delete selection)
                    if ui.input(|i| i.key_pressed(Key::F2)) {
                        if let Some(one) = self.single_selected_path() {
                            self.start_rename(one);
                        }
                    }
                    if ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
                        if !self.selection.is_empty() {
                            self.confirm_delete = Some(self.selection.iter().cloned().collect());
                        }
                    }

                    // Cancel drag if released anywhere not handled
                    if ui.input(|i| i.pointer.any_released()) {
                        self.dragging = None;
                    }
                });

                // Apply all operations queued during UI
                self.apply_pending();
            });
    }
}

// ---------- Utilities ----------

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
