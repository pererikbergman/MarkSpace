//! The eframe application shell for MarkSpace.
//!
//! This is thin glue: every frame it wires keyboard shortcuts to
//! [`Workspace`] toggle methods, then draws the three-panel layout based on the
//! resulting state. All layout *logic* lives in [`Workspace`]; this module only
//! renders it. See `CONTEXT.md` for the panel vocabulary.

use eframe::egui;

use crate::workspace::Workspace;

/// The top-level MarkSpace application.
pub struct MarkSpaceApp {
    workspace: Workspace,
}

impl MarkSpaceApp {
    /// Build the app with a default workspace.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            workspace: Workspace::new(),
        }
    }

    /// Map the PRD's layout shortcuts to workspace toggles. `COMMAND` resolves
    /// to Cmd on macOS and Ctrl elsewhere, matching the PRD's `Cmd/Ctrl`.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};

        let pressed = |mods: Modifiers, key: Key| -> bool {
            ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(mods, key)))
        };

        if pressed(Modifiers::COMMAND, Key::Num1) {
            self.workspace.toggle_projects_pane();
        }
        if pressed(Modifiers::COMMAND, Key::Num2) {
            self.workspace.toggle_context_column();
        }
        if pressed(Modifiers::COMMAND, Key::I) {
            self.workspace.toggle_quick_info();
        }
        if pressed(Modifiers::COMMAND | Modifiers::SHIFT, Key::F) {
            self.workspace.toggle_focus_mode();
        }
    }
}

impl eframe::App for MarkSpaceApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // `ctx` is a cheap Arc handle; clone it so the immutable borrow of `ui`
        // ends before we draw into `ui` below.
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);

        // Panel A: far-left projects pane.
        if self.workspace.show_projects_pane {
            egui::Panel::left("projects_pane")
                .default_size(180.0)
                .show(ui, |ui| {
                    ui.heading("Projects");
                    ui.separator();
                    ui.label("(no projects registered)");
                });
        }

        // Panel C: far-right context column — file tree over quick info.
        if self.workspace.show_context_column {
            egui::Panel::right("context_column")
                .default_size(260.0)
                .show(ui, |ui| {
                    if self.workspace.quick_info_expanded {
                        egui::Panel::bottom("quick_info")
                            .resizable(false)
                            .show(ui, |ui| {
                                ui.heading("Quick Info");
                                ui.label("Words: — · Lines: — · Size: — · Modified: —");
                            });
                    }
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.heading("File Tree");
                        ui.label("(no project open)");
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
