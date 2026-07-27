//! Interactive state for the File Tree: which folders are expanded and which
//! node is selected, plus keyboard navigation over the *visible* rows.
//!
//! Pure state with no dependency on egui, so it stays unit-testable. The app
//! rebuilds a `FileTree` from a completed scan and renders `visible_rows()`.
//! The selected node, when it's a file, is the **active file** (`CONTEXT.md`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::workspace::Node;

/// A visible row in the rendered File Tree: a node at a given indent `depth`,
/// with `expanded` telling the renderer which triangle to draw for a directory.
pub struct Row<'a> {
    pub node: &'a Node,
    pub depth: usize,
    pub expanded: bool,
}

/// Expansion + selection state over a scanned node tree.
pub struct FileTree {
    roots: Vec<Node>,
    expanded: HashSet<PathBuf>,
    selected: Option<PathBuf>,
}

impl FileTree {
    /// Build a tree from scanned roots: all folders collapsed, nothing selected.
    pub fn new(roots: Vec<Node>) -> Self {
        Self {
            roots,
            expanded: HashSet::new(),
            selected: None,
        }
    }

    /// The selected node's path, if any. When it points to a file, that's the
    /// active file.
    pub fn selected(&self) -> Option<&Path> {
        self.selected.as_deref()
    }

    /// Whether a directory is currently expanded.
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    /// Replace the tree after a rescan, preserving expand/collapse and
    /// selection for paths that still exist (stale entries are pruned).
    pub fn update_roots(&mut self, roots: Vec<Node>) {
        self.roots = roots;
        self.expanded.retain(|p| find_node(&self.roots, p).is_some());
        if let Some(sel) = &self.selected
            && find_node(&self.roots, sel).is_none()
        {
            self.selected = None;
        }
    }

    /// Select a node by path (e.g. from a click).
    pub fn select(&mut self, path: PathBuf) {
        self.selected = Some(path);
    }

    /// Move the selection down one visible row (clamped at the last row); with
    /// nothing selected, selects the first row.
    pub fn select_next(&mut self) {
        let paths = self.visible_paths();
        if paths.is_empty() {
            return;
        }
        let next = match self.selected_index(&paths) {
            None => 0,
            Some(i) => (i + 1).min(paths.len() - 1),
        };
        self.selected = Some(paths[next].clone());
    }

    /// Move the selection up one visible row (clamped at the first row); with
    /// nothing selected, selects the first row.
    pub fn select_prev(&mut self) {
        let paths = self.visible_paths();
        if paths.is_empty() {
            return;
        }
        let prev = match self.selected_index(&paths) {
            None => 0,
            Some(i) => i.saturating_sub(1),
        };
        self.selected = Some(paths[prev].clone());
    }

    /// Right arrow: expand a collapsed directory; if already expanded, move
    /// into its first child. No-op on a file (or a childless expanded dir).
    pub fn move_right(&mut self) {
        let Some(selected) = self.selected.clone() else {
            self.select_next();
            return;
        };
        let Some((is_dir, first_child)) = find_node(&self.roots, &selected)
            .map(|n| (n.is_dir, n.children.first().map(|c| c.path.clone())))
        else {
            return;
        };
        if !is_dir {
            return;
        }
        if !self.is_expanded(&selected) {
            self.expanded.insert(selected);
        } else if let Some(first) = first_child {
            self.selected = Some(first);
        }
    }

    /// Left arrow: collapse an expanded directory; otherwise select the parent
    /// node (if it's in the tree). No-op at the top level (parent is the
    /// workspace root, which isn't a tree node).
    pub fn move_left(&mut self) {
        let Some(selected) = self.selected.clone() else {
            self.select_next();
            return;
        };
        let is_dir = find_node(&self.roots, &selected).is_some_and(|n| n.is_dir);
        if is_dir && self.is_expanded(&selected) {
            self.expanded.remove(&selected);
        } else if let Some(parent) = selected.parent()
            && find_node(&self.roots, parent).is_some()
        {
            self.selected = Some(parent.to_path_buf());
        }
    }

    fn visible_paths(&self) -> Vec<PathBuf> {
        self.visible_rows()
            .into_iter()
            .map(|r| r.node.path.clone())
            .collect()
    }

    fn selected_index(&self, paths: &[PathBuf]) -> Option<usize> {
        self.selected
            .as_ref()
            .and_then(|s| paths.iter().position(|p| p == s))
    }

    /// Expand a collapsed directory or collapse an expanded one.
    pub fn toggle_expanded(&mut self, path: &Path) {
        if !self.expanded.remove(path) {
            self.expanded.insert(path.to_path_buf());
        }
    }

    /// The visible rows, top to bottom, skipping the contents of collapsed
    /// directories.
    pub fn visible_rows(&self) -> Vec<Row<'_>> {
        let mut rows = Vec::new();
        self.push_rows(&self.roots, 0, &mut rows);
        rows
    }

    fn push_rows<'a>(&'a self, nodes: &'a [Node], depth: usize, rows: &mut Vec<Row<'a>>) {
        for node in nodes {
            let expanded = self.is_expanded(&node.path);
            rows.push(Row { node, depth, expanded });
            if node.is_dir && expanded {
                self.push_rows(&node.children, depth + 1, rows);
            }
        }
    }
}

/// Find a node by exact path within a forest, searching recursively.
fn find_node<'a>(nodes: &'a [Node], path: &Path) -> Option<&'a Node> {
    for node in nodes {
        if node.path == path {
            return Some(node);
        }
        if node.is_dir
            && let Some(found) = find_node(&node.children, path)
        {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> Node {
        Node {
            name: leaf(path),
            path: PathBuf::from(path),
            is_dir: false,
            children: Vec::new(),
        }
    }

    fn dir(path: &str, children: Vec<Node>) -> Node {
        Node {
            name: leaf(path),
            path: PathBuf::from(path),
            is_dir: true,
            children,
        }
    }

    fn leaf(path: &str) -> String {
        path.rsplit('/').next().unwrap().to_string()
    }

    fn names(rows: &[Row<'_>]) -> Vec<String> {
        rows.iter().map(|r| r.node.name.clone()).collect()
    }

    #[test]
    fn visible_rows_are_top_level_and_collapsed_by_default() {
        let tree = FileTree::new(vec![
            dir("/w/src", vec![file("/w/src/main.rs")]),
            file("/w/readme.md"),
        ]);

        let rows = tree.visible_rows();

        assert_eq!(names(&rows), ["src", "readme.md"]);
        assert!(rows.iter().all(|r| r.depth == 0), "top level is depth 0");
        assert!(tree.selected().is_none(), "nothing selected initially");
    }

    #[test]
    fn toggle_expanded_shows_and_hides_children() {
        let mut tree = FileTree::new(vec![
            dir("/w/src", vec![file("/w/src/main.rs")]),
            file("/w/readme.md"),
        ]);

        tree.toggle_expanded(&PathBuf::from("/w/src"));
        let rows = tree.visible_rows();
        assert_eq!(names(&rows), ["src", "main.rs", "readme.md"]);
        let main = rows.iter().find(|r| r.node.name == "main.rs").unwrap();
        assert_eq!(main.depth, 1, "child indented one level");

        tree.toggle_expanded(&PathBuf::from("/w/src"));
        assert_eq!(names(&tree.visible_rows()), ["src", "readme.md"], "collapsed again");
    }

    #[test]
    fn select_marks_the_selected_path() {
        let mut tree = FileTree::new(vec![file("/w/readme.md")]);

        tree.select(PathBuf::from("/w/readme.md"));

        assert_eq!(tree.selected(), Some(Path::new("/w/readme.md")));
    }

    #[test]
    fn select_next_walks_down_visible_rows_and_clamps() {
        let mut tree = FileTree::new(vec![
            dir("/w/src", vec![file("/w/src/main.rs")]),
            file("/w/readme.md"),
        ]);
        tree.toggle_expanded(&PathBuf::from("/w/src")); // visible: src, main.rs, readme.md

        tree.select_next(); // from None -> first row
        assert_eq!(tree.selected(), Some(Path::new("/w/src")));
        tree.select_next();
        assert_eq!(tree.selected(), Some(Path::new("/w/src/main.rs")));
        tree.select_next();
        assert_eq!(tree.selected(), Some(Path::new("/w/readme.md")));
        tree.select_next(); // clamp at last
        assert_eq!(tree.selected(), Some(Path::new("/w/readme.md")));
    }

    #[test]
    fn select_prev_walks_up_visible_rows_and_clamps() {
        let mut tree = FileTree::new(vec![
            dir("/w/src", vec![file("/w/src/main.rs")]),
            file("/w/readme.md"),
        ]);
        tree.toggle_expanded(&PathBuf::from("/w/src"));
        tree.select(PathBuf::from("/w/readme.md")); // start at last

        tree.select_prev();
        assert_eq!(tree.selected(), Some(Path::new("/w/src/main.rs")));
        tree.select_prev();
        assert_eq!(tree.selected(), Some(Path::new("/w/src")));
        tree.select_prev(); // clamp at first
        assert_eq!(tree.selected(), Some(Path::new("/w/src")));
    }

    #[test]
    fn move_right_expands_then_enters_child_and_ignores_files() {
        let mut tree = FileTree::new(vec![
            dir("/w/src", vec![file("/w/src/main.rs")]),
            file("/w/readme.md"),
        ]);
        tree.select(PathBuf::from("/w/src")); // collapsed folder

        tree.move_right(); // expands, stays on folder
        assert!(tree.is_expanded(Path::new("/w/src")));
        assert_eq!(tree.selected(), Some(Path::new("/w/src")));

        tree.move_right(); // already expanded -> into first child
        assert_eq!(tree.selected(), Some(Path::new("/w/src/main.rs")));

        tree.move_right(); // on a file -> no change
        assert_eq!(tree.selected(), Some(Path::new("/w/src/main.rs")));
    }

    #[test]
    fn update_roots_preserves_expansion_and_shows_new_entries() {
        let mut tree = FileTree::new(vec![dir("/w/src", vec![file("/w/src/main.rs")])]);
        tree.toggle_expanded(&PathBuf::from("/w/src"));

        // A new file appeared under src (filesystem changed → rescan).
        tree.update_roots(vec![dir(
            "/w/src",
            vec![file("/w/src/main.rs"), file("/w/src/lib.rs")],
        )]);

        assert!(tree.is_expanded(Path::new("/w/src")), "expansion preserved");
        assert_eq!(
            names(&tree.visible_rows()),
            ["src", "main.rs", "lib.rs"],
            "new file is visible under the still-expanded folder"
        );
    }

    #[test]
    fn update_roots_keeps_selection_if_present_else_clears_it() {
        let mut tree = FileTree::new(vec![file("/w/a.md"), file("/w/b.md")]);
        tree.select(PathBuf::from("/w/b.md"));

        // b.md still there -> selection kept.
        tree.update_roots(vec![file("/w/a.md"), file("/w/b.md")]);
        assert_eq!(tree.selected(), Some(Path::new("/w/b.md")));

        // b.md deleted -> selection cleared.
        tree.update_roots(vec![file("/w/a.md")]);
        assert_eq!(tree.selected(), None);
    }

    #[test]
    fn update_roots_prunes_expansion_of_removed_folders() {
        let mut tree = FileTree::new(vec![
            dir("/w/a", vec![file("/w/a/x.md")]),
            dir("/w/b", vec![file("/w/b/y.md")]),
        ]);
        tree.toggle_expanded(&PathBuf::from("/w/a"));
        tree.toggle_expanded(&PathBuf::from("/w/b"));

        // Folder b was deleted.
        tree.update_roots(vec![dir("/w/a", vec![file("/w/a/x.md")])]);

        assert!(tree.is_expanded(Path::new("/w/a")), "surviving folder stays expanded");
        assert!(!tree.is_expanded(Path::new("/w/b")), "removed folder's expansion pruned");
    }

    #[test]
    fn move_left_collapses_then_goes_to_parent_and_stops_at_top() {
        let mut tree = FileTree::new(vec![
            dir(
                "/w/src",
                vec![
                    dir("/w/src/sub", vec![file("/w/src/sub/deep.md")]),
                    file("/w/src/main.rs"),
                ],
            ),
            file("/w/readme.md"),
        ]);

        // Expanded dir -> collapse, stays selected.
        tree.select(PathBuf::from("/w/src"));
        tree.toggle_expanded(&PathBuf::from("/w/src"));
        tree.move_left();
        assert!(!tree.is_expanded(Path::new("/w/src")));
        assert_eq!(tree.selected(), Some(Path::new("/w/src")));

        // File -> select parent.
        tree.select(PathBuf::from("/w/src/main.rs"));
        tree.move_left();
        assert_eq!(tree.selected(), Some(Path::new("/w/src")));

        // Collapsed nested dir -> select parent.
        tree.select(PathBuf::from("/w/src/sub"));
        tree.move_left();
        assert_eq!(tree.selected(), Some(Path::new("/w/src")));

        // Top-level node -> no parent in tree, no-op.
        tree.select(PathBuf::from("/w/readme.md"));
        tree.move_left();
        assert_eq!(tree.selected(), Some(Path::new("/w/readme.md")));
    }
}
