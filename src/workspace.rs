//! A workspace: an opened folder (a directory root), and the registry of
//! recently opened workspaces.
//!
//! Opening a folder — by drag-and-drop, cmux-style — creates a workspace; the
//! `WorkspaceList` tracks the open/recent ones with one selected as active.
//! Pure logic with no dependency on egui, so it stays unit-testable. Reading a
//! workspace's contents into the File Tree lives in the `scan` module. See
//! `CONTEXT.md` for the vocabulary (workspace, active workspace).

use std::path::PathBuf;

/// An open workspace: a directory root whose contents feed the File Tree.
pub struct Workspace {
    pub root: PathBuf,
}

impl Workspace {
    /// Open `path` as a workspace, or `None` if it isn't a directory (so a
    /// dropped file or a bad path is ignored rather than crashing).
    pub fn open(path: PathBuf) -> Option<Workspace> {
        path.is_dir().then_some(Workspace { root: path })
    }

    /// Display name for the Workspaces Pane: the root's final path component,
    /// falling back to the full path (e.g. for a filesystem root).
    pub fn name(&self) -> String {
        self.root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.root.to_string_lossy().into_owned())
    }
}

/// Maximum recent workspaces kept in the list / persisted (PRD §4.1).
const MAX_RECENT: usize = 20;

/// The set of open workspaces, cmux-style, with one selected as active.
pub struct WorkspaceList {
    items: Vec<Workspace>,
    active: Option<usize>,
}

impl WorkspaceList {
    /// An empty list with no active workspace.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            active: None,
        }
    }

    /// Open `path` as a workspace: validate it's a directory, add it, and make
    /// it active. Returns `false` (and changes nothing) for a non-directory.
    pub fn open(&mut self, path: PathBuf) -> bool {
        match Workspace::open(path) {
            Some(workspace) => {
                let index = match self.items.iter().position(|w| w.root == workspace.root) {
                    Some(i) => i,
                    None => {
                        self.items.push(workspace);
                        // Cap the list, dropping the oldest (front) entry.
                        if self.items.len() > MAX_RECENT {
                            self.items.remove(0);
                        }
                        self.items.len() - 1
                    }
                };
                self.active = Some(index);
                true
            }
            None => false,
        }
    }

    /// Make the workspace at `index` active. Out-of-range indices are ignored.
    pub fn select(&mut self, index: usize) {
        if index < self.items.len() {
            self.active = Some(index);
        }
    }

    /// Activate the next workspace down the list (clamped at the last).
    pub fn select_next(&mut self) {
        if let Some(i) = self.active {
            self.select((i + 1).min(self.items.len().saturating_sub(1)));
        } else if !self.items.is_empty() {
            self.select(0);
        }
    }

    /// Activate the previous workspace up the list (clamped at the first).
    pub fn select_prev(&mut self) {
        if let Some(i) = self.active {
            self.select(i.saturating_sub(1));
        } else if !self.items.is_empty() {
            self.select(0);
        }
    }

    /// Rebuild the list from persisted roots (startup), keeping those that
    /// still exist as directories, capped and de-duplicated, with nothing
    /// active until the user picks one.
    pub fn from_paths(paths: Vec<PathBuf>) -> Self {
        let mut list = Self::new();
        for path in paths {
            list.open(path);
        }
        list.active = None;
        list
    }

    /// The workspace roots in list order, for persisting to config.
    pub fn paths(&self) -> Vec<PathBuf> {
        self.items.iter().map(|w| w.root.clone()).collect()
    }

    /// The currently active workspace, if any.
    pub fn active(&self) -> Option<&Workspace> {
        self.active.map(|i| &self.items[i])
    }

    /// Index of the active workspace, for rendering selection in the pane.
    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    /// Iterate the open workspaces in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &Workspace> {
        self.items.iter()
    }
}

impl Default for WorkspaceList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn open_accepts_a_directory() {
        let dir = tempdir().unwrap();
        let workspace = Workspace::open(dir.path().to_path_buf());
        assert_eq!(workspace.map(|w| w.root), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn open_rejects_a_file_or_missing_path() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("note.md");
        fs::write(&file, "").unwrap();

        assert!(Workspace::open(file).is_none(), "a file is not a workspace");
        assert!(
            Workspace::open(dir.path().join("nope")).is_none(),
            "a missing path is not a workspace"
        );
    }

    #[test]
    fn name_is_the_final_path_component() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("my-notes");
        fs::create_dir(&sub).unwrap();

        let workspace = Workspace::open(sub).unwrap();

        assert_eq!(workspace.name(), "my-notes");
    }

    #[test]
    fn new_workspace_list_is_empty_with_no_active() {
        let list = WorkspaceList::new();
        assert!(list.active().is_none());
        assert_eq!(list.iter().count(), 0);
    }

    #[test]
    fn open_adds_a_directory_and_makes_it_active() {
        let dir = tempdir().unwrap();
        let mut list = WorkspaceList::new();

        let opened = list.open(dir.path().to_path_buf());

        assert!(opened);
        assert_eq!(list.iter().count(), 1);
        assert_eq!(list.active().map(|w| &w.root), Some(&dir.path().to_path_buf()));
    }

    #[test]
    fn open_rejects_a_non_directory_and_leaves_the_list_unchanged() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("note.md");
        fs::write(&file, "").unwrap();
        let mut list = WorkspaceList::new();

        let opened = list.open(file);

        assert!(!opened);
        assert_eq!(list.iter().count(), 0);
        assert!(list.active().is_none());
    }

    #[test]
    fn open_does_not_duplicate_an_already_open_workspace() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        let mut list = WorkspaceList::new();

        list.open(a.path().to_path_buf()); // active = a
        list.open(b.path().to_path_buf()); // active = b
        let reopened = list.open(a.path().to_path_buf()); // already open

        assert!(reopened);
        assert_eq!(list.iter().count(), 2, "no duplicate entry");
        assert_eq!(
            list.active().map(|w| &w.root),
            Some(&a.path().to_path_buf()),
            "reopening selects the existing workspace"
        );
    }

    #[test]
    fn from_paths_restores_existing_dirs_with_nothing_active() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        let bad = a.path().join("gone"); // never created

        let list = WorkspaceList::from_paths(vec![
            a.path().to_path_buf(),
            bad,
            b.path().to_path_buf(),
        ]);

        assert_eq!(
            list.paths(),
            vec![a.path().to_path_buf(), b.path().to_path_buf()],
            "stale path filtered, order preserved"
        );
        assert!(list.active().is_none(), "nothing active until the user picks");
    }

    #[test]
    fn open_caps_the_list_at_twenty_dropping_the_oldest() {
        let root = tempdir().unwrap();
        let mut list = WorkspaceList::new();
        let mut dirs = Vec::new();
        for i in 0..21 {
            let p = root.path().join(format!("w{i:02}"));
            fs::create_dir(&p).unwrap();
            list.open(p.clone());
            dirs.push(p);
        }

        assert_eq!(list.iter().count(), 20, "capped at 20");
        assert!(
            list.iter().all(|w| w.root != dirs[0]),
            "oldest (w00) was dropped"
        );
        assert!(
            list.iter().any(|w| w.root == dirs[20]),
            "newest (w20) is kept"
        );
    }

    #[test]
    fn select_next_and_prev_walk_the_list_and_clamp() {
        let (a, b, c) = (tempdir().unwrap(), tempdir().unwrap(), tempdir().unwrap());
        let mut list = WorkspaceList::new();
        list.open(a.path().to_path_buf());
        list.open(b.path().to_path_buf());
        list.open(c.path().to_path_buf()); // active = c (index 2)

        list.select(0); // a
        list.select_next();
        assert_eq!(list.active().map(|w| &w.root), Some(&b.path().to_path_buf()));
        list.select_next();
        assert_eq!(list.active().map(|w| &w.root), Some(&c.path().to_path_buf()));
        list.select_next(); // clamp at last
        assert_eq!(list.active().map(|w| &w.root), Some(&c.path().to_path_buf()));

        list.select_prev();
        assert_eq!(list.active().map(|w| &w.root), Some(&b.path().to_path_buf()));
        list.select_prev();
        list.select_prev(); // clamp at first
        assert_eq!(list.active().map(|w| &w.root), Some(&a.path().to_path_buf()));
    }

    #[test]
    fn select_changes_the_active_workspace() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        let mut list = WorkspaceList::new();
        list.open(a.path().to_path_buf());
        list.open(b.path().to_path_buf()); // active = b (index 1)

        list.select(0);

        assert_eq!(list.active().map(|w| &w.root), Some(&a.path().to_path_buf()));
    }
}
