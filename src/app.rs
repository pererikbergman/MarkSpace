//! The eframe application shell for MarkSpace.
//!
//! This is thin glue: every frame it wires keyboard shortcuts to [`Layout`]
//! toggle methods, then draws the three-panel layout based on the resulting
//! state. All layout *logic* lives in [`Layout`] and workspace logic in
//! [`WorkspaceList`]; this module only renders them. See `CONTEXT.md` for the
//! panel vocabulary.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use eframe::egui;

use crate::layout::Layout;
use crate::workspace::{spawn_scan, Node, WorkspaceList};

/// The top-level MarkSpace application.
pub struct MarkSpaceApp {
    /// Panel visibility / focus-mode state.
    layout: Layout,
    /// Open workspaces, one active, shown in the Workspaces Pane.
    workspaces: WorkspaceList,
    /// Nested File Tree for the active workspace.
    tree: Vec<Node>,
    /// Pending background scan; drained each frame until it delivers.
    scan_rx: Option<Receiver<Vec<Node>>>,
    /// Which workspace root `tree`/`scan_rx` correspond to, so we rescan
    /// only when the active workspace changes.
    scanned_root: Option<PathBuf>,
}

impl MarkSpaceApp {
    /// Build the app with a default layout and no workspaces open.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            layout: Layout::new(),
            workspaces: WorkspaceList::new(),
            tree: Vec::new(),
            scan_rx: None,
            scanned_root: None,
        }
    }

    /// Open a folder dropped anywhere on the window as a workspace. macOS/winit
    /// doesn't report *where* an external file-drop landed, so we can't scope
    /// the drop to the Workspaces Pane; a drop always means "add a workspace".
    /// Non-directory drops are ignored by `WorkspaceList::open`.
    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()));
        if let Some(path) = dropped {
            self.workspaces.open(path);
        }
    }

    /// Start a fresh background scan whenever the active workspace changes.
    fn sync_active_scan(&mut self) {
        let active_root = self.workspaces.active().map(|w| w.root.clone());
        if active_root != self.scanned_root {
            self.scanned_root = active_root.clone();
            self.tree.clear();
            self.scan_rx = active_root.map(spawn_scan);
        }
    }

    /// Drain a pending background scan; keep repainting until it arrives.
    fn drain_scan(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.scan_rx {
            match rx.try_recv() {
                Ok(tree) => {
                    self.tree = tree;
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

    /// Draw the far-left Workspaces Pane: a clickable list of open workspaces
    /// with the active one highlighted.
    fn show_workspaces_pane(&mut self, ui: &mut egui::Ui) {
        let active_index = self.workspaces.active_index();
        let names: Vec<String> = self.workspaces.iter().map(|w| w.name()).collect();
        let mut clicked = None;

        egui::Panel::left("workspaces_pane")
            .default_size(180.0)
            .show(ui, |ui| {
                ui.heading("Workspaces");
                ui.separator();
                if names.is_empty() {
                    ui.label("(drop a folder to open a workspace)");
                } else {
                    for (i, name) in names.iter().enumerate() {
                        if ui.selectable_label(Some(i) == active_index, name).clicked() {
                            clicked = Some(i);
                        }
                    }
                }
            });

        if let Some(i) = clicked {
            self.workspaces.select(i);
        }
    }
}

/// Recursively render File Tree nodes: directories as collapsible headers
/// (egui persists their open/closed state across frames, keyed by path), files
/// as leaf labels.
fn show_tree(ui: &mut egui::Ui, nodes: &[Node]) {
    for node in nodes {
        if node.is_dir {
            egui::CollapsingHeader::new(format!("📁  {}", node.name))
                .id_salt(&node.path)
                .show(ui, |ui| show_tree(ui, &node.children));
        } else {
            ui.label(format!("📄  {}", node.name));
        }
    }
}

impl eframe::App for MarkSpaceApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // `ctx` is a cheap Arc handle; clone it so the immutable borrow of `ui`
        // ends before we draw into `ui` below.
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        self.handle_drops(&ctx);
        self.sync_active_scan();
        self.drain_scan(&ctx);

        // Panel A: far-left workspaces pane.
        if self.layout.show_workspaces_pane {
            self.show_workspaces_pane(ui);
        }

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
                        ui.heading("File Tree");
                        if self.workspaces.active().is_none() {
                            ui.label("(no workspace open)");
                        } else if self.scan_rx.is_some() {
                            ui.label("Scanning…");
                        } else {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                show_tree(ui, &self.tree);
                            });
                        }
                    });
                });
        }

        // Panel B: center editor canvas, always visible.
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("MarkSpace");
            ui.label("Editor canvas — WYSIWYG engine lands in Phase 4.");
        });
    }
}
