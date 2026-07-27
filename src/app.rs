//! The eframe application shell for MarkSpace.
//!
//! This is thin glue: every frame it wires keyboard shortcuts to [`Layout`]
//! toggle methods, then draws the three-panel layout based on the resulting
//! state. All layout *logic* lives in [`Layout`]; this module only renders it.
//! See `CONTEXT.md` for the panel vocabulary.

use std::sync::mpsc::Receiver;

use eframe::egui;

use crate::layout::Layout;
use crate::workspace::{spawn_scan, Entry, Workspace};

/// The top-level MarkSpace application.
pub struct MarkSpaceApp {
    /// Panel visibility / focus-mode state.
    layout: Layout,
    /// The currently open workspace (an opened folder), if any.
    workspace: Option<Workspace>,
    /// Top-level entries shown in the File Tree.
    entries: Vec<Entry>,
    /// Pending background scan; drained each frame until it delivers.
    scan_rx: Option<Receiver<Vec<Entry>>>,
}

impl MarkSpaceApp {
    /// Build the app with a default layout and no workspace open.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            layout: Layout::new(),
            workspace: None,
            entries: Vec::new(),
            scan_rx: None,
        }
    }

    /// If the user dropped a folder onto the window, open it as the active
    /// workspace and start a background scan. Non-directory drops are ignored.
    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()));

        if let Some(path) = dropped
            && let Some(workspace) = Workspace::open(path)
        {
            self.scan_rx = Some(spawn_scan(workspace.root.clone()));
            self.entries.clear();
            self.workspace = Some(workspace);
        }
    }

    /// Drain a pending background scan; keep repainting until it arrives.
    fn drain_scan(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.scan_rx {
            match rx.try_recv() {
                Ok(entries) => {
                    self.entries = entries;
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
}

impl eframe::App for MarkSpaceApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // `ctx` is a cheap Arc handle; clone it so the immutable borrow of `ui`
        // ends before we draw into `ui` below.
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        self.handle_drops(&ctx);
        self.drain_scan(&ctx);

        // Panel A: far-left workspaces pane.
        if self.layout.show_workspaces_pane {
            egui::Panel::left("workspaces_pane")
                .default_size(180.0)
                .show(ui, |ui| {
                    ui.heading("Workspaces");
                    ui.separator();
                    ui.label("(no workspaces open)");
                });
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
                        match &self.workspace {
                            None => {
                                ui.label("(drop a folder here to open a workspace)");
                            }
                            Some(_) if self.scan_rx.is_some() => {
                                ui.label("Scanning…");
                            }
                            Some(_) => {
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    for entry in &self.entries {
                                        let icon = if entry.is_dir { "📁" } else { "📄" };
                                        ui.label(format!("{icon}  {}", entry.name));
                                    }
                                });
                            }
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
