//! Layout state machine for MarkSpace's panels.
//!
//! Pure state with no dependency on egui. `MarkSpaceApp` reads this to decide
//! which panels to draw and wires keyboard shortcuts to its toggle methods.
//! See `CONTEXT.md` for the vocabulary (projects pane, context column, quick
//! info, focus mode).

/// Visibility and layout state for MarkSpace's panels.
pub struct Workspace {
    /// Far-left projects pane (`Cmd/Ctrl + 1`).
    pub show_projects_pane: bool,
    /// Far-right context column: file tree + quick info (`Cmd/Ctrl + 2`).
    pub show_context_column: bool,
    /// Quick info sub-panel expanded vs collapsed to status bar (`Cmd/Ctrl + I`).
    pub quick_info_expanded: bool,
    /// Focus mode: both side panels collapsed (`Cmd/Ctrl + Shift + F`).
    pub focus_mode: bool,
    /// Snapshot of `(show_projects_pane, show_context_column)` taken on entering
    /// focus mode, restored on exit. `None` when not in focus mode.
    focus_snapshot: Option<(bool, bool)>,
}

impl Workspace {
    /// A workspace with the PRD's default `config.toml` layout.
    pub fn new() -> Self {
        Self {
            show_projects_pane: true,
            show_context_column: true,
            quick_info_expanded: true,
            focus_mode: false,
            focus_snapshot: None,
        }
    }

    /// Toggle the far-left projects pane.
    pub fn toggle_projects_pane(&mut self) {
        self.show_projects_pane = !self.show_projects_pane;
    }

    /// Toggle the far-right context column.
    pub fn toggle_context_column(&mut self) {
        self.show_context_column = !self.show_context_column;
    }

    /// Toggle the quick info sub-panel between expanded and collapsed.
    pub fn toggle_quick_info(&mut self) {
        self.quick_info_expanded = !self.quick_info_expanded;
    }

    /// Toggle focus mode. Entering snapshots both side panels' visibility and
    /// hides them; exiting restores the snapshot (a pane hidden beforehand
    /// stays hidden).
    pub fn toggle_focus_mode(&mut self) {
        match self.focus_snapshot.take() {
            None => {
                self.focus_snapshot = Some((self.show_projects_pane, self.show_context_column));
                self.show_projects_pane = false;
                self.show_context_column = false;
                self.focus_mode = true;
            }
            Some((projects, context)) => {
                self.show_projects_pane = projects;
                self.show_context_column = context;
                self.focus_mode = false;
            }
        }
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_workspace_uses_prd_default_visibility() {
        let ws = Workspace::new();

        assert!(ws.show_projects_pane, "projects pane visible by default");
        assert!(ws.show_context_column, "context column visible by default");
        assert!(ws.quick_info_expanded, "quick info expanded by default");
        assert!(!ws.focus_mode, "focus mode off by default");
    }

    #[test]
    fn toggle_projects_pane_flips_visibility() {
        let mut ws = Workspace::new();

        ws.toggle_projects_pane();
        assert!(!ws.show_projects_pane);

        ws.toggle_projects_pane();
        assert!(ws.show_projects_pane);
    }

    #[test]
    fn toggle_context_column_flips_visibility() {
        let mut ws = Workspace::new();

        ws.toggle_context_column();
        assert!(!ws.show_context_column);

        ws.toggle_context_column();
        assert!(ws.show_context_column);
    }

    #[test]
    fn toggle_quick_info_flips_expanded() {
        let mut ws = Workspace::new();

        ws.toggle_quick_info();
        assert!(!ws.quick_info_expanded);

        ws.toggle_quick_info();
        assert!(ws.quick_info_expanded);
    }

    #[test]
    fn entering_focus_mode_hides_both_panes() {
        let mut ws = Workspace::new();

        ws.toggle_focus_mode();

        assert!(ws.focus_mode);
        assert!(!ws.show_projects_pane);
        assert!(!ws.show_context_column);
    }

    #[test]
    fn exiting_focus_mode_restores_prior_pane_state() {
        let mut ws = Workspace::new();
        // Projects pane hidden before focus mode; context column left visible.
        ws.toggle_projects_pane();

        ws.toggle_focus_mode(); // enter — both hidden
        ws.toggle_focus_mode(); // exit — restore snapshot

        assert!(!ws.focus_mode);
        assert!(!ws.show_projects_pane, "was hidden before, stays hidden");
        assert!(ws.show_context_column, "was visible before, restored");
    }

    #[test]
    fn focus_mode_round_trips_from_defaults() {
        let mut ws = Workspace::new();

        ws.toggle_focus_mode();
        assert!(ws.focus_mode);

        ws.toggle_focus_mode();
        assert!(!ws.focus_mode);
        assert!(ws.show_projects_pane);
        assert!(ws.show_context_column);
    }
}
