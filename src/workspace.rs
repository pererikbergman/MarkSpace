//! A workspace: an opened folder (a directory root) and its top-level File
//! Tree listing.
//!
//! Opening a folder — by drag-and-drop, cmux-style — creates a workspace whose
//! contents populate the File Tree. Pure filesystem logic with no dependency on
//! egui, so it stays unit-testable. `MarkSpaceApp` opens a workspace from a
//! dropped path, kicks off a background scan, and drains the entries into the
//! File Tree. See `CONTEXT.md` for the vocabulary (workspace, File Tree, entry).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use walkdir::WalkDir;

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
                let index = self
                    .items
                    .iter()
                    .position(|w| w.root == workspace.root)
                    .unwrap_or_else(|| {
                        self.items.push(workspace);
                        self.items.len() - 1
                    });
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

/// A node in the File Tree: a file or directory. Directories carry their
/// nested `children`; files have none.
pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<Node>,
}

/// Scan a directory into a nested tree of its contents.
pub fn scan_tree(root: &Path) -> Vec<Node> {
    let mut nodes: Vec<Node> = WalkDir::new(root)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let is_dir = entry.file_type().is_dir();
            let path = entry.into_path();
            Node {
                name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                children: if is_dir { scan_tree(&path) } else { Vec::new() },
                is_dir,
                path,
            }
        })
        .collect();

    // Directories before files, then case-insensitive alphabetical. Each
    // recursive call sorts its own level, so the whole tree is sorted.
    nodes.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    nodes
}

/// Scan `root` into a nested tree on a background thread, delivering it over a
/// channel so the UI thread never blocks on disk I/O (PRD §4.1). The receiver
/// yields exactly one message when the scan completes.
pub fn spawn_scan(root: PathBuf) -> Receiver<Vec<Node>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        // If the receiver was dropped (workspace changed), the send just fails.
        let _ = tx.send(scan_tree(&root));
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scan_tree_returns_top_level_nodes() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("notes")).unwrap();
        fs::write(dir.path().join("readme.md"), "hi").unwrap();

        let tree = scan_tree(dir.path());
        let by_name: Vec<(&str, bool)> =
            tree.iter().map(|n| (n.name.as_str(), n.is_dir)).collect();

        assert!(by_name.contains(&("notes", true)));
        assert!(by_name.contains(&("readme.md", false)));
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn scan_tree_nests_directory_children() {
        let dir = tempdir().unwrap();
        let notes = dir.path().join("notes");
        fs::create_dir(&notes).unwrap();
        fs::write(notes.join("todo.md"), "").unwrap();

        let tree = scan_tree(dir.path());
        let notes_node = tree.iter().find(|n| n.name == "notes").unwrap();

        let child_names: Vec<&str> =
            notes_node.children.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(child_names, ["todo.md"]);
    }

    #[test]
    fn scan_tree_files_are_leaves() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("readme.md"), "").unwrap();

        let tree = scan_tree(dir.path());
        let file = tree.iter().find(|n| n.name == "readme.md").unwrap();

        assert!(!file.is_dir);
        assert!(file.children.is_empty());
    }

    #[test]
    fn scan_tree_sorts_dirs_first_case_insensitive_at_every_level() {
        let dir = tempdir().unwrap();
        // Top level: mix of dirs and files, mixed case.
        fs::write(dir.path().join("banana.txt"), "").unwrap();
        fs::create_dir(dir.path().join("Apple")).unwrap();
        fs::create_dir(dir.path().join("cherry")).unwrap();
        // Nested level inside Apple, also needing sort.
        fs::write(dir.path().join("Apple").join("zebra.md"), "").unwrap();
        fs::create_dir(dir.path().join("Apple").join("Box")).unwrap();

        let tree = scan_tree(dir.path());

        let top: Vec<&str> = tree.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(top, ["Apple", "cherry", "banana.txt"]);

        let apple = tree.iter().find(|n| n.name == "Apple").unwrap();
        let nested: Vec<&str> = apple.children.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(nested, ["Box", "zebra.md"], "sorted at nested level too");
    }

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
    fn name_is_the_final_path_component() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("my-notes");
        fs::create_dir(&sub).unwrap();

        let workspace = Workspace::open(sub).unwrap();

        assert_eq!(workspace.name(), "my-notes");
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

    #[test]
    fn spawn_scan_delivers_entries_over_the_channel() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("readme.md"), "").unwrap();

        let rx = spawn_scan(dir.path().to_path_buf());
        let entries = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("scan result should arrive on the channel");

        let names: Vec<String> = entries.into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["src", "readme.md"]);
    }
}
