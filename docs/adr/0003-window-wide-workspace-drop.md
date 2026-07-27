# ADR 0003: Workspace drag-drop is window-wide, not pane-scoped

- **Status:** Accepted
- **Date:** 2026-07-27
- **Phase:** 2 (File System & Tree), issue #1

## Context

The intended interaction was: drop a folder **onto the Workspaces Pane** to add
it as a workspace, while drops elsewhere (File Tree, editor) do nothing. An
implementation that scoped the drop to the pane's rectangle was tried and
**failed**: the drop was silently discarded (the OS showed the "+" drop cursor,
but nothing happened).

Root cause (confirmed from source, not guessed):

- `egui-winit` 0.35's `WindowEvent::DroppedFile` handler pushes the dropped file
  with **only its path** and emits **no pointer position / `PointerMoved`**
  event. The `DroppedFile` itself carries no drop location on native.
- egui's pointer position comes solely from `CursorMoved` events. On macOS,
  `CursorMoved` is **not delivered during an external Finder→window file drag**
  (the OS runs a modal drag session), so `pointer.latest_pos()` is `None` or
  stale at the drop frame.

Therefore the scoping guard (`pane_rect.contains(drop_pos)`) could never pass.
There is no reliable way to know *where* within the window an external file-drop
landed with egui/winit on macOS.

## Decision

Accept folder drops **window-wide**. A drop anywhere adds a workspace via
`WorkspaceList::open` (which ignores non-directories). Because the *only* drop
behavior in MarkSpace is "add a workspace", there is no competing per-region
drop action to disambiguate — so window-wide acceptance honors the intent
("drops only ever mean workspace") even though it can't be geometrically scoped.

## Consequences

- The `Entry`/pane geometry is not consulted for drops; the app carries no
  drop-target rectangle state.
- Discoverability and intent can still be improved **without** a drop position,
  because `hovered_files` *is* delivered during a drag (position-free):
  - highlight the Workspaces Pane / show "Drop to add a workspace" while
    `hovered_files` is non-empty (deferred);
  - add a native "Open folder…" picker button via the `rfd` crate (deferred).
- If precise per-region file-drop is ever required, it would need
  platform-specific code (a custom Cocoa dragging destination), which is out of
  scope.

## References

- `src/app.rs` `handle_drops` — the window-wide implementation.
- egui-winit 0.35 `src/lib.rs` — `DroppedFile`/`CursorMoved` handling.
