# ADR 0002: Adopt cmux-style "workspace" terminology

- **Status:** Accepted
- **Date:** 2026-07-27
- **Phase:** 2 (File System & Tree)
- **Supersedes vocabulary in:** `CONTEXT.md` (initial seed), PRD §2–§6 wording

## Context

The PRD names an opened directory root a **"project"** (the far-left "Projects
Pane", the `[[projects]]` config array, `active_project_index`) and uses
**"workspace"** for application state (the `[workspace]` config table). The
initial `CONTEXT.md` glossary and the first Phase 2 implementation followed that
wording (`Project` type, `src/project.rs`, `show_projects_pane`).

In practice the maintainer's mental model is cmux-style: **you open a folder and
that becomes a workspace**, and the File Tree reflects that workspace's
structure. "Workspace" (not "project") is the natural word for an opened folder,
matching cmux/VS Code usage. Keeping "project" fought that model.

## Decision

Rename the domain vocabulary so **workspace = an opened folder (a directory
root)**, and give the former "workspace" (panel/app state) a distinct name,
**Layout**.

Mapping applied across code, `CONTEXT.md`, and the Phase 2 issues:

| Before | After |
| --- | --- |
| `Project` (directory root), `src/project.rs` | `Workspace`, `src/workspace.rs` |
| `Workspace` (panel/app state), `src/workspace.rs` | `Layout`, `src/layout.rs` |
| "Projects Pane" (far-left) | "Workspaces Pane" |
| `show_projects_pane` / `toggle_projects_pane` | `show_workspaces_pane` / `toggle_workspaces_pane` |
| "active project" | "active workspace" |
| config `[[projects]]`, `active_project_index` | `[[workspaces]]`, `active_workspace_index` (not yet built) |
| config `[workspace]` / `[panels]` | `[layout]` (not yet built) |

Unchanged: File Tree, Quick Info, Context Column, Editor Canvas, Focus Mode,
Entry, "active file".

## Consequences

- `CONTEXT.md` diverges deliberately from the PRD's "project"/"workspace"
  wording; the glossary carries a note pointing here, and is authoritative.
- The PRD text is left as the historical spec; its `SidePanel`/"project" wording
  is now dated (see also [ADR 0001](0001-egui-0.35-unified-panel-api.md)).
- Config keys (`[layout]`, `[[workspaces]]`) are settled now, before the config
  subsystem is built (Phase 2, issue #5), so no migration is needed.
- The clean split — `Workspace` (an opened folder) vs `Layout` (panel state) —
  removes the earlier overload where "workspace" meant app state.

## References

- `src/workspace.rs`, `src/layout.rs`, `src/app.rs` — the renamed code.
- MarkSpace issues #1–#5 — updated to this vocabulary.
