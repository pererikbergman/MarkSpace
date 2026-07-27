//! The eframe application shell for MarkSpace.
//!
//! Thin glue: each frame it wires keyboard shortcuts and arrow-key navigation
//! to the state types ([`Layout`], [`WorkspaceList`], [`LiveTree`]), then draws
//! the three-panel layout. All logic lives in those types; this module only
//! renders them and routes input. See `CONTEXT.md` for the vocabulary.

use std::path::PathBuf;

use eframe::egui;

use crate::config::Config;
use crate::layout::Layout;
use crate::live_tree::LiveTree;
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
    /// The active workspace's File Tree, kept live with the filesystem.
    live_tree: LiveTree,
    /// Where the recent-workspace registry is persisted, if a home dir exists.
    config_path: Option<PathBuf>,
    /// Which pane the arrow keys drive.
    focus: FocusPane,
}

/// Row data collected from the `FileTree` for a frame, owned so rendering and
/// click-handling don't hold a borrow on `live_tree` while it mutates.
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
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = default_config_path();
        let workspaces = match &config_path {
            Some(path) => WorkspaceList::from_paths(Config::load(path).workspaces),
            None => WorkspaceList::new(),
        };

        // Repaint the UI when the live tree wants attention (scan done, fs event).
        let ctx = cc.egui_ctx.clone();
        let live_tree = LiveTree::new(move || ctx.request_repaint());

        Self {
            layout: Layout::new(),
            workspaces,
            live_tree,
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
                let tree = self.live_tree.tree_mut();
                if up {
                    tree.select_prev();
                }
                if down {
                    tree.select_next();
                }
                if left {
                    tree.move_left();
                }
                if right {
                    tree.move_right();
                }
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

        // Keep the live tree pointed at the active workspace and advance it.
        self.live_tree
            .set_root(self.workspaces.active().map(|w| w.root.clone()));
        self.live_tree.poll();

        // Panel A: far-left workspaces pane.
        if self.layout.show_workspaces_pane {
            self.show_workspaces_pane(ui);
        }

        // Collect File Tree rows up front (owned) so the render closures don't
        // hold a borrow on `live_tree` while we apply clicks afterwards.
        let selected = self.live_tree.tree().selected().map(|p| p.to_path_buf());
        let rows: Vec<FileRow> = self
            .live_tree
            .tree()
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
        let scanning = self.live_tree.is_scanning();
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
            let tree = self.live_tree.tree_mut();
            tree.select(path.clone());
            if is_dir {
                tree.toggle_expanded(&path);
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
