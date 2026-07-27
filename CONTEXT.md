# MarkSpace — Domain Context

The shared vocabulary for MarkSpace. When code, issues, tests, or docs name a
concept below, use the **canonical term** — not a synonym. Seeded from the
[PRD](docs/Product-Requirements-Document.md) and refined since; extend it via
`/grill-with-docs` as decisions crystallise.

> **Terminology note:** the PRD calls an opened directory a "project" and uses
> "workspace" for app state. MarkSpace instead adopts cmux-style **workspace**
> terminology — see [ADR 0002](docs/adr/0002-workspace-terminology.md). This
> glossary is authoritative where it diverges from the PRD's wording.

## Glossary

### Workspace
An **opened folder** — a single directory root on disk. Opening a folder (by
drag-and-drop, cmux-style) creates a workspace, and its contents populate the
**File Tree**. The **Workspaces Pane** lists open workspaces; the config stores
up to 20 recent ones as a TOML `[[workspaces]]` array.

> **Canonical term is _workspace_.** In code this is the `Workspace` type
> (`src/workspace.rs`). Avoid "project", "vault", and "workspace root" — the PRD
> uses those, but MarkSpace says **workspace**.

### Layout
The app's **panel state** — which panels are visible, whether quick info is
expanded, and focus mode. In code this is the `Layout` type (`src/layout.rs`).
It is *not* an opened folder (that's a **workspace**). Persisted under the
config's `[layout]` table (formerly the PRD's `[workspace]`/`[panels]`).

### Active workspace
The workspace currently selected in the Workspaces Pane, whose File Tree is
shown. Recalled from the config's recent-workspace registry on startup.

### Panels
The three top-level layout regions:

- **Workspaces Pane** — far-left, the open-workspace selector. Toggle:
  `Cmd/Ctrl + 1`.
- **Editor Canvas** (a.k.a. **Center Canvas**) — the center WYSIWYG editing
  surface. Always visible.
- **Context Column** — the far-right column, a vertical split of two sub-panels.
  Toggle: `Cmd/Ctrl + 2`.

### Context Column sub-panels
- **File Tree** — top ~70%, the nested folder/file explorer for the active
  workspace.
- **Quick Info** — bottom ~30%, live stats for the active file (word/char/line
  count, size, path, modified time). Collapses into a 24px status bar via
  `Cmd/Ctrl + I`.

### Entry
One item in the File Tree — a file or a directory within a workspace. In code,
the `Entry` type (name, path, is_dir).

### Focus Mode
The layout state where both the Workspaces Pane and Context Column are
collapsed, leaving only the Editor Canvas. Toggle: `Cmd/Ctrl + Shift + F`.
Stored as `layout.focus_mode`.

### WYSIWYG rendering
The editor's live-rendering behaviour: raw markdown syntax (`#`, `**`, `>`, …)
is **hidden** when the caret leaves a line block and **re-exposed** when the
caret enters it. Driven by caret-position token matching over `pulldown-cmark`
events.

### Active file
The single markdown document currently open in the Editor Canvas, selected in
the File Tree. Its handle feeds the Quick Info sub-panel.

### Config
Plain-text TOML at `~/.config/markspace/config.toml`. Holds `[layout]` and the
`[[workspaces]]` registry.

## Terms to avoid

- **"project"**, **"vault"**, **"workspace root"** → use **workspace** (the
  opened folder).
- **"Projects Pane"** → use **Workspaces Pane**.
- **"sidebar"** on its own is ambiguous → say **Workspaces Pane** or
  **Context Column**.
- **"metadata panel"** → use **Quick Info**.
- Don't use **workspace** for panel/app state → that's the **Layout**.
