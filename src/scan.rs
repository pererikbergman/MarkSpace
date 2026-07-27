//! Filesystem-to-tree scanning: reading a directory root into a nested `Node`
//! tree, synchronously or on a background thread.
//!
//! Pure filesystem logic with no dependency on egui, so it stays unit-testable.
//! The File Tree consumes the `Node` tree; `MarkSpaceApp` kicks off background
//! scans and drains them. See `CONTEXT.md` (File Tree, entry).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use walkdir::WalkDir;

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
    fn spawn_scan_delivers_the_tree_over_the_channel() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("readme.md"), "").unwrap();

        let rx = spawn_scan(dir.path().to_path_buf());
        let tree = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("scan result should arrive on the channel");

        let names: Vec<String> = tree.into_iter().map(|n| n.name).collect();
        assert_eq!(names, ["src", "readme.md"]);
    }
}
