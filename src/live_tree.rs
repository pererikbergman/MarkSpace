//! The live File Tree: keeps a [`FileTree`] in sync with a workspace root on
//! disk, hiding the background scan, the `notify` watcher, event coalescing,
//! and the initial-load-vs-live-refresh distinction behind a small interface.
//!
//! No dependency on egui — repaint is delivered through an injected callback —
//! so the coalescing policy is unit-testable. See ADR 0001 (app.rs is thin
//! glue) and `CONTEXT.md` (File Tree, active workspace).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::file_tree::FileTree;
use crate::scan::{spawn_scan, Node};

/// Whether a delivered scan is an initial load (reset expand/selection) or a
/// live refresh (preserve them).
#[derive(Debug, PartialEq, Clone, Copy)]
enum ScanKind {
    Initial,
    Refresh,
}

/// Keeps a [`FileTree`] in sync with a workspace root: background scan +
/// recursive `notify` watcher + coalesced refresh, all behind `set_root` /
/// `poll`. Repaint is delivered via the injected `on_change` callback, so this
/// type has no dependency on egui.
pub struct LiveTree {
    root: Option<PathBuf>,
    tree: FileTree,
    state: ScanState,
    scan_rx: Option<Receiver<Vec<Node>>>,
    scan_kind: Option<ScanKind>,
    watcher: Option<RecommendedWatcher>,
    watch_rx: Option<Receiver<()>>,
    on_change: Arc<dyn Fn() + Send + Sync>,
}

impl LiveTree {
    /// Build an empty live tree. `on_change` is called (possibly from the
    /// watcher thread) whenever the UI should repaint.
    pub fn new(on_change: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            root: None,
            tree: FileTree::new(Vec::new()),
            state: ScanState::default(),
            scan_rx: None,
            scan_kind: None,
            watcher: None,
            watch_rx: None,
            on_change: Arc::new(on_change),
        }
    }

    /// Point the live tree at a workspace root (or `None`). A no-op if the root
    /// is unchanged; otherwise resets the tree, launches an initial scan, and
    /// re-points the watcher (dropping the old one).
    pub fn set_root(&mut self, root: Option<PathBuf>) {
        if root == self.root {
            return;
        }
        self.root = root.clone();
        self.tree = FileTree::new(Vec::new());
        self.scan_rx = None;
        self.scan_kind = None;

        match root {
            Some(root) => {
                let kind = self.state.root_opened();
                self.start_scan(root.clone(), kind);
                self.start_watch(&root);
            }
            None => {
                self.state.root_closed();
                self.watcher = None;
                self.watch_rx = None;
            }
        }
    }

    /// Advance the lifecycle one frame: absorb watcher signals, launch a
    /// coalesced refresh if due, and apply a delivered scan.
    pub fn poll(&mut self) {
        if let Some(rx) = &self.watch_rx {
            while rx.try_recv().is_ok() {
                self.state.mark_dirty();
            }
        }

        if self.root.is_some()
            && let Some(kind) = self.state.take_refresh()
        {
            let root = self.root.clone().expect("root present");
            self.start_scan(root, kind);
        }

        if let Some(rx) = &self.scan_rx {
            match rx.try_recv() {
                Ok(nodes) => {
                    if self.scan_kind == Some(ScanKind::Initial) {
                        self.tree = FileTree::new(nodes); // reset expand/selection
                    } else {
                        self.tree.update_roots(nodes); // preserve them
                    }
                    self.scan_rx = None;
                    self.scan_kind = None;
                    self.state.scan_finished();
                }
                Err(_) => (self.on_change)(), // keep waking until the scan lands
            }
        }
    }

    /// The current File Tree (read-only).
    pub fn tree(&self) -> &FileTree {
        &self.tree
    }

    /// The current File Tree, mutable for selection / navigation.
    pub fn tree_mut(&mut self) -> &mut FileTree {
        &mut self.tree
    }

    /// Whether a scan is currently in flight (for a "Scanning…" indicator).
    pub fn is_scanning(&self) -> bool {
        self.state.is_scanning()
    }

    fn start_scan(&mut self, root: PathBuf, kind: ScanKind) {
        self.scan_rx = Some(spawn_scan(root));
        self.scan_kind = Some(kind);
    }

    fn start_watch(&mut self, root: &Path) {
        let on_change = Arc::clone(&self.on_change);
        let (tx, rx) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if res.is_ok() {
                let _ = tx.send(());
                on_change();
            }
        });

        if let Ok(mut watcher) = watcher
            && watcher.watch(root, RecursiveMode::Recursive).is_ok()
        {
            self.watcher = Some(watcher);
            self.watch_rx = Some(rx);
        } else {
            self.watcher = None;
            self.watch_rx = None;
        }
    }
}

/// The scan-scheduling policy: pure state, no IO. Encodes the coalescing rule
/// (a burst of filesystem events collapses to one rescan; never scan while a
/// scan is in flight) and the initial-vs-refresh decision.
#[derive(Default)]
struct ScanState {
    scanning: bool,
    dirty: bool,
}

impl ScanState {
    /// The active root changed to an actual workspace: launch an initial scan
    /// now, superseding any pending refresh.
    fn root_opened(&mut self) -> ScanKind {
        self.dirty = false;
        self.scanning = true;
        ScanKind::Initial
    }

    fn is_scanning(&self) -> bool {
        self.scanning
    }

    /// A filesystem change was observed under the root.
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Launch a coalesced refresh if a change is pending and no scan is in
    /// flight. Returns the scan to run, or `None`.
    fn take_refresh(&mut self) -> Option<ScanKind> {
        if self.dirty && !self.scanning {
            self.dirty = false;
            self.scanning = true;
            Some(ScanKind::Refresh)
        } else {
            None
        }
    }

    /// The in-flight scan delivered.
    fn scan_finished(&mut self) {
        self.scanning = false;
    }

    /// The active root was cleared: cancel any pending or in-flight work.
    fn root_closed(&mut self) {
        self.scanning = false;
        self.dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_a_root_requests_an_initial_scan() {
        let mut state = ScanState::default();

        let kind = state.root_opened();

        assert_eq!(kind, ScanKind::Initial);
        assert!(state.is_scanning());
    }

    #[test]
    fn a_burst_of_events_coalesces_into_one_refresh() {
        let mut state = ScanState::default();

        state.mark_dirty();
        state.mark_dirty();
        state.mark_dirty();

        assert_eq!(state.take_refresh(), Some(ScanKind::Refresh));
        assert_eq!(state.take_refresh(), None, "burst collapsed to a single rescan");
    }

    #[test]
    fn no_refresh_while_a_scan_is_in_flight() {
        let mut state = ScanState::default();
        state.root_opened(); // initial scan now in flight
        state.mark_dirty(); // change arrives mid-scan

        assert_eq!(state.take_refresh(), None, "must not scan while one is running");

        state.scan_finished();
        assert_eq!(
            state.take_refresh(),
            Some(ScanKind::Refresh),
            "pending change scanned once the in-flight scan completes"
        );
    }

    #[test]
    fn opening_a_root_supersedes_a_pending_change() {
        let mut state = ScanState::default();
        state.mark_dirty(); // a stale change from a previous root

        state.root_opened(); // fresh initial scan will cover current state
        state.scan_finished();

        assert_eq!(
            state.take_refresh(),
            None,
            "the initial scan already reflects the current tree; no extra refresh"
        );
    }

    #[test]
    fn closing_the_root_cancels_pending_work() {
        let mut state = ScanState::default();
        state.root_opened();
        state.mark_dirty();

        state.root_closed();

        assert!(!state.is_scanning());
        assert_eq!(state.take_refresh(), None, "no scan without a workspace");
    }
}
