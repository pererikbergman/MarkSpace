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
}

/// One top-level item shown in the File Tree.
pub struct Entry {
    pub name: String,
    /// Absolute path. Consumed by File Tree selection in issue #3; the display
    /// glue only needs `name`/`is_dir` for now.
    #[allow(dead_code)]
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Scan a directory's immediate children (depth 1).
pub fn scan_children(root: &Path) -> Vec<Entry> {
    let mut sorted: Vec<Entry> = WalkDir::new(root)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .map(|entry| Entry {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir: entry.file_type().is_dir(),
            path: entry.into_path(),
        })
        .collect();

    // Directories before files, then case-insensitive alphabetical within each.
    sorted.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    sorted
}

/// Scan `root` on a background thread, delivering the sorted entries over a
/// channel so the UI thread never blocks on disk I/O (PRD §4.1). The receiver
/// yields exactly one message when the scan completes.
pub fn spawn_scan(root: PathBuf) -> Receiver<Vec<Entry>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        // If the receiver was dropped (workspace changed), the send just fails.
        let _ = tx.send(scan_children(&root));
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scan_children_lists_immediate_entries() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("notes")).unwrap();
        fs::write(dir.path().join("readme.md"), "hi").unwrap();

        let entries = scan_children(dir.path());
        let by_name: Vec<(&str, bool)> =
            entries.iter().map(|e| (e.name.as_str(), e.is_dir)).collect();

        assert!(by_name.contains(&("notes", true)));
        assert!(by_name.contains(&("readme.md", false)));
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn scan_children_sorts_dirs_first_then_case_insensitive_alpha() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("banana.txt"), "").unwrap();
        fs::create_dir(dir.path().join("Apple")).unwrap();
        fs::create_dir(dir.path().join("cherry")).unwrap();
        fs::write(dir.path().join("date.md"), "").unwrap();

        let order: Vec<String> = scan_children(dir.path())
            .into_iter()
            .map(|e| e.name)
            .collect();

        assert_eq!(order, ["Apple", "cherry", "banana.txt", "date.md"]);
    }

    #[test]
    fn scan_children_excludes_nested_entries() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("deep.md"), "").unwrap();

        let names: Vec<String> = scan_children(dir.path())
            .into_iter()
            .map(|e| e.name)
            .collect();

        assert_eq!(names, ["sub"], "only the immediate child, not deep.md");
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
