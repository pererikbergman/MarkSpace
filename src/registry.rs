//! The workspace registry: the recent-workspace list together with its own
//! persistence. Opening a workspace saves the config as a side effect and
//! startup restores it, so no caller can forget to persist.
//!
//! Composes the pure [`WorkspaceList`] with [`Config`] and a config path. See
//! `CONTEXT.md` (workspace registry) and ADR 0002 for the config keys.

use std::path::PathBuf;

use crate::config::Config;
use crate::workspace::{Workspace, WorkspaceList};

/// The recent-workspace registry: an in-memory [`WorkspaceList`] that persists
/// itself to a config file whenever the list changes.
pub struct WorkspaceRegistry {
    list: WorkspaceList,
    path: Option<PathBuf>,
}

impl WorkspaceRegistry {
    /// Load the registry from the default config location:
    /// `~/.config/markspace/config.toml` (explicitly `~/.config`, even on
    /// macOS, per the PRD). Persistence is disabled if there's no home dir.
    pub fn load() -> Self {
        Self::load_from(default_path())
    }

    /// Load the registry from a config path (`None` disables persistence).
    /// A missing or malformed file yields an empty registry.
    pub fn load_from(path: Option<PathBuf>) -> Self {
        let list = match &path {
            Some(path) => WorkspaceList::from_paths(Config::load(path).workspaces),
            None => WorkspaceList::new(),
        };
        Self { list, path }
    }

    /// Open a workspace and persist the registry. Returns whether it opened.
    pub fn open(&mut self, path: PathBuf) -> bool {
        let opened = self.list.open(path);
        self.save();
        opened
    }

    /// The currently active workspace, if any.
    pub fn active(&self) -> Option<&Workspace> {
        self.list.active()
    }

    /// Iterate the open workspaces in list order.
    pub fn iter(&self) -> impl Iterator<Item = &Workspace> {
        self.list.iter()
    }

    /// Index of the active workspace, for rendering selection in the pane.
    pub fn active_index(&self) -> Option<usize> {
        self.list.active_index()
    }

    /// Make the workspace at `index` active. Selection isn't persisted.
    pub fn select(&mut self, index: usize) {
        self.list.select(index);
    }

    /// Activate the next workspace down the list (clamped).
    pub fn select_next(&mut self) {
        self.list.select_next();
    }

    /// Activate the previous workspace up the list (clamped).
    pub fn select_prev(&mut self) {
        self.list.select_prev();
    }

    /// Persist the current registry to the config path (best-effort).
    fn save(&self) {
        if let Some(path) = &self.path {
            let config = Config {
                workspaces: self.list.paths(),
            };
            let _ = config.save(path);
        }
    }
}

/// The config location per the PRD: `~/.config/markspace/config.toml`
/// (explicitly `~/.config`, even on macOS). `None` if there's no home dir.
fn default_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".config/markspace/config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn opening_persists_and_a_reload_restores() {
        let cfg_dir = tempdir().unwrap();
        let cfg = cfg_dir.path().join("config.toml");
        let ws = tempdir().unwrap();

        let mut registry = WorkspaceRegistry::load_from(Some(cfg.clone()));
        assert!(registry.active().is_none(), "starts empty");

        registry.open(ws.path().to_path_buf()); // persists as a side effect

        let reloaded = WorkspaceRegistry::load_from(Some(cfg));
        let roots: Vec<PathBuf> = reloaded.iter().map(|w| w.root.clone()).collect();
        assert_eq!(
            roots,
            vec![ws.path().to_path_buf()],
            "the opened workspace was restored from disk"
        );
    }

    #[test]
    fn works_without_a_config_path() {
        let ws = tempdir().unwrap();
        let mut registry = WorkspaceRegistry::load_from(None);

        let opened = registry.open(ws.path().to_path_buf()); // must not panic

        assert!(opened);
        assert_eq!(registry.active().map(|w| &w.root), Some(&ws.path().to_path_buf()));
    }

    #[test]
    fn missing_config_file_loads_empty() {
        let dir = tempdir().unwrap();
        let registry = WorkspaceRegistry::load_from(Some(dir.path().join("nope.toml")));

        assert!(registry.active().is_none());
        assert_eq!(registry.iter().count(), 0);
    }
}
