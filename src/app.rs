//! The eframe application shell for MarkSpace.
//!
//! Thin glue: each frame it wires keyboard shortcuts and arrow-key navigation
//! to the state types ([`Layout`], [`WorkspaceList`], [`FileTree`]), then draws
//! the three-panel layout. All logic lives in those types; this module only
//! renders them and routes input. See `CONTEXT.md` for the vocabulary.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use eframe::egui;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::config::Config;
use crate::file_tree::FileTree;
use crate::layout::Layout;
use crate::scan::{spawn_scan, Node};
use crate::workspace::WorkspaceList;

/// Which pane the arrow keys currently drive.
#[derive(PartialEq, Clone, Copy)]
enum FocusPane {
    Workspaces,
    Files,
}

/// The top-level MarkSpace application.
pub struct MarkSpaceApp {
    /// Panel visibility / focus-mode state.
    layout: Layout,
    /// Open workspaces, one active, shown in the Workspaces Pane.
    workspaces: WorkspaceList,
    /// Expansion + selection state for the active workspace's File Tree.
    file_tree: FileTree,
    /// Pending background scan; drained each frame until it delivers.
    scan_rx: Option<Receiver<Vec<Node>>>,
    /// Whether the pending scan is a live refresh (preserve expand/selection)
    /// rather than an initial load (reset state).
    scan_is_refresh: bool,
    /// Which workspace root the current `file_tree`/`scan_rx` correspond to, so
    /// we rescan only when the active workspace changes.
    scanned_root: Option<PathBuf>,
    /// Filesystem watcher on the active workspace root; kept alive here so
    /// dropping/replacing it re-points watching when the workspace changes.
    watcher: Option<RecommendedWatcher>,
    /// Signals from the watcher that the workspace changed on disk.
    watch_rx: Option<Receiver<()>>,
    /// A filesystem change is pending a rescan (coalesces event bursts).
    dirty: bool,
    /// Where the recent-workspace registry is persisted, if a home dir exists.
    config_path: Option<PathBuf>,
    /// Which pane the arrow keys drive.
    focus: FocusPane,
}

/// Row data collected from the `FileTree` for a frame, owned so rendering and
/// click-handling don't hold a borrow on `self.file_tree` while it mutates.
struct FileRow {
    path: PathBuf,
    name: String,
    is_dir: bool,
    depth: usize,
    expanded: bool,
    selected: bool,
}

impl MarkSpaceApp {
    /// Build the app, restoring the recent-workspace registry from config.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = default_config_path();
        let workspaces = match &config_path {
            Some(path) => WorkspaceList::from_paths(Config::load(path).workspaces),
            None => WorkspaceList::new(),
        };

        Self {
            layout: Layout::new(),
            workspaces,
            file_tree: FileTree::new(Vec::new()),
            scan_rx: None,
            scan_is_refresh: false,
            scanned_root: None,
            watcher: None,
            watch_rx: None,
            dirty: false,
            config_path,
            focus: FocusPane::Files,
        }
    }

    /// Persist the current recent-workspace registry (best-effort).
    fn save_config(&self) {
        if let Some(path) = &self.config_path {
            let config = Config {
                workspaces: self.workspaces.paths(),
            };
            let _ = config.save(path);
        }
    }

    /// Open a folder dropped anywhere on the window as a workspace. macOS/winit
    /// doesn't report *where* an external file-drop landed, so we can't scope
    /// the drop to the Workspaces Pane (ADR 0003); a drop always adds a
    /// workspace. Non-directory drops are ignored by `WorkspaceList::open`.
    /// Returns whether a workspace was opened (so the caller can persist).
    fn handle_drops(&mut self, ctx: &egui::Context) -> bool {
        let dropped = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()));
        match dropped {
            Some(path) => self.workspaces.open(path),
            None => false,
        }
    }

    /// Route arrow keys to the focused pane.
    fn handle_navigation(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        let key = |k| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
        let (up, down, left, right) = (
            key(Key::ArrowUp),
            key(Key::ArrowDown),
            key(Key::ArrowLeft),
            key(Key::ArrowRight),
        );

        match self.focus {
            FocusPane::Workspaces => {
                if up {
                    self.workspaces.select_prev();
                }
                if down {
                    self.workspaces.select_next();
                }
            }
            FocusPane::Files => {
                if up {
                    self.file_tree.select_prev();
                }
                if down {
                    self.file_tree.select_next();
                }
                if left {
                    self.file_tree.move_left();
                }
                if right {
                    self.file_tree.move_right();
                }
            }
        }
    }

    /// When the active workspace changes, reset the tree, kick off an initial
    /// scan, and re-point the filesystem watcher (dropping the old one).
    fn sync_active_workspace(&mut self, ctx: &egui::Context) {
        let active_root = self.workspaces.active().map(|w| w.root.clone());
        if active_root == self.scanned_root {
            return;
        }
        self.scanned_root = active_root.clone();
        self.file_tree = FileTree::new(Vec::new());
        self.dirty = false;

        match active_root {
            Some(root) => {
                self.scan_is_refresh = false;
                self.scan_rx = Some(spawn_scan(root.clone()));
                match spawn_watch(&root, ctx.clone()) {
                    Some((watcher, rx)) => {
                        self.watcher = Some(watcher);
                        self.watch_rx = Some(rx);
                    }
                    None => {
                        self.watcher = None;
                        self.watch_rx = None;
                    }
                }
            }
            None => {
                self.scan_rx = None;
                self.watcher = None;
                self.watch_rx = None;
            }
        }
    }

    /// Drain watcher signals and, once idle, coalesce them into a single live
    /// rescan of the active workspace.
    fn poll_watch(&mut self) {
        if let Some(rx) = &self.watch_rx {
            while rx.try_recv().is_ok() {
                self.dirty = true;
            }
        }
        if self.dirty && self.scan_rx.is_none() {
            if let Some(root) = self.scanned_root.clone() {
                self.scan_is_refresh = true;
                self.scan_rx = Some(spawn_scan(root));
            }
            self.dirty = false;
        }
    }

    /// Drain a pending background scan; keep repainting until it arrives.
    fn drain_scan(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.scan_rx {
            match rx.try_recv() {
                Ok(tree) => {
                    if self.scan_is_refresh {
                        self.file_tree.update_roots(tree); // live refresh: keep state
                    } else {
                        self.file_tree = FileTree::new(tree); // initial load: reset
                    }
                    self.scan_rx = None;
                }
                Err(_) => ctx.request_repaint(), // scan still running
            }
        }
    }

    /// Map the PRD's layout shortcuts to panel toggles. `COMMAND` resolves to
    /// Cmd on macOS and Ctrl elsewhere, matching the PRD's `Cmd/Ctrl`.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};

        let pressed = |mods: Modifiers, key: Key| -> bool {
            ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(mods, key)))
        };

        if pressed(Modifiers::COMMAND, Key::Num1) {
            self.layout.toggle_workspaces_pane();
        }
        if pressed(Modifiers::COMMAND, Key::Num2) {
            self.layout.toggle_context_column();
        }
        if pressed(Modifiers::COMMAND, Key::I) {
            self.layout.toggle_quick_info();
        }
        if pressed(Modifiers::COMMAND | Modifiers::SHIFT, Key::F) {
            self.layout.toggle_focus_mode();
        }
    }

    /// Draw the far-left Workspaces Pane: a full-width, clickable list of open
    /// workspaces with the active one highlighted.
    fn show_workspaces_pane(&mut self, ui: &mut egui::Ui) {
        let active_index = self.workspaces.active_index();
        let names: Vec<String> = self.workspaces.iter().map(|w| w.name()).collect();
        let focused = self.focus == FocusPane::Workspaces;
        let mut clicked = None;

        egui::Panel::left("workspaces_pane")
            .default_size(180.0)
            .show(ui, |ui| {
                ui.heading(if focused { "▸ Workspaces" } else { "Workspaces" });
                ui.separator();
                if names.is_empty() {
                    ui.label("(drop a folder to open a workspace)");
                } else {
                    ui.with_layout(
                        egui::Layout::top_down_justified(egui::Align::LEFT),
                        |ui| {
                            for (i, name) in names.iter().enumerate() {
                                let selected = Some(i) == active_index;
                                if ui.selectable_label(selected, name.as_str()).clicked() {
                                    clicked = Some(i);
                                }
                            }
                        },
                    );
                }
            });

        if let Some(i) = clicked {
            self.workspaces.select(i);
            self.focus = FocusPane::Workspaces;
        }
    }
}

impl eframe::App for MarkSpaceApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // `ctx` is a cheap Arc handle; clone it so the immutable borrow of `ui`
        // ends before we draw into `ui` below.
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        if self.handle_drops(&ctx) {
            self.save_config();
        }
        self.handle_navigation(&ctx);
        self.sync_active_workspace(&ctx);
        self.poll_watch();
        self.drain_scan(&ctx);

        // Panel A: far-left workspaces pane.
        if self.layout.show_workspaces_pane {
            self.show_workspaces_pane(ui);
        }

        // Collect File Tree rows up front (owned) so the render closures don't
        // hold a borrow on `self.file_tree` while we apply clicks afterwards.
        let selected = self.file_tree.selected().map(|p| p.to_path_buf());
        let rows: Vec<FileRow> = self
            .file_tree
            .visible_rows()
            .iter()
            .map(|r| FileRow {
                selected: Some(r.node.path.as_path()) == selected.as_deref(),
                path: r.node.path.clone(),
                name: r.node.name.clone(),
                is_dir: r.node.is_dir,
                depth: r.depth,
                expanded: r.expanded,
            })
            .collect();
        let no_workspace = self.workspaces.active().is_none();
        let scanning = self.scan_rx.is_some();
        let files_focused = self.focus == FocusPane::Files;
        let mut file_clicked: Option<(PathBuf, bool)> = None;

        // Panel C: far-right context column — file tree over quick info.
        if self.layout.show_context_column {
            egui::Panel::right("context_column")
                .default_size(260.0)
                .show(ui, |ui| {
                    if self.layout.quick_info_expanded {
                        egui::Panel::bottom("quick_info")
                            .resizable(false)
                            .show(ui, |ui| {
                                ui.heading("Quick Info");
                                ui.label("Words: — · Lines: — · Size: — · Modified: —");
                            });
                    }
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.heading(if files_focused { "▸ File Tree" } else { "File Tree" });
                        if no_workspace {
                            ui.label("(no workspace open)");
                        } else if scanning {
                            ui.label("Scanning…");
                        } else {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    for row in &rows {
                                        if row_clicked(ui, row) {
                                            file_clicked = Some((row.path.clone(), row.is_dir));
                                        }
                                    }
                                });
                        }
                    });
                });
        }

        if let Some((path, is_dir)) = file_clicked {
            self.file_tree.select(path.clone());
            if is_dir {
                self.file_tree.toggle_expanded(&path);
            }
            self.focus = FocusPane::Files;
        }

        // Panel B: center editor canvas, always visible.
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("MarkSpace");
            ui.label("Editor canvas — WYSIWYG engine lands in Phase 4.");
        });
    }
}

/// The config location per the PRD: `~/.config/markspace/config.toml` (note:
/// explicitly `~/.config`, even on macOS). `None` if there's no home dir.
fn default_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".config/markspace/config.toml"))
}

/// Start a recursive filesystem watcher on `root`. Each change sends a signal
/// over the channel and requests a repaint so the UI wakes to rescan. Returns
/// the watcher (which must be kept alive) and the receiver, or `None` if the
/// watcher couldn't be created. Non-UI I/O, verified by running.
fn spawn_watch(root: &Path, ctx: egui::Context) -> Option<(RecommendedWatcher, Receiver<()>)> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if res.is_ok() {
            let _ = tx.send(());
            ctx.request_repaint();
        }
    })
    .ok()?;
    watcher.watch(root, RecursiveMode::Recursive).ok()?;
    Some((watcher, rx))
}

/// Render one File Tree row as a full-width, left-aligned, indented selectable
/// item with faint indent-guide lines. Directories show a ▸/▾ disclosure
/// triangle. Returns whether it was clicked.
fn row_clicked(ui: &mut egui::Ui, row: &FileRow) -> bool {
    const INDENT: f32 = 14.0;
    let icon = if row.is_dir {
        if row.expanded { "▾ 📁" } else { "▸ 📁" }
    } else {
        "📄"
    };
    let text = format!("{icon}  {}", row.name);

    let inner = ui.horizontal(|ui| {
        ui.add_space(row.depth as f32 * INDENT);
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            ui.selectable_label(row.selected, text).clicked()
        })
        .inner
    });

    // Faint vertical guide line at each ancestor indent level.
    let rect = inner.response.rect;
    let color = ui.visuals().weak_text_color();
    for level in 0..row.depth {
        let x = rect.left() + level as f32 * INDENT + 7.0;
        ui.painter()
            .vline(x, rect.y_range(), egui::Stroke::new(1.0, color));
    }

    inner.inner
}
