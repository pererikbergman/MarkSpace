# MarkSpace — Domain Context

The shared vocabulary for MarkSpace. When code, issues, tests, or docs name a
concept below, use the **canonical term** — not a synonym. Seeded from the
[PRD](docs/Product-Requirements-Document.md); extend it via `/grill-with-docs`
as decisions crystallise.

## Glossary

### Project
A workspace directory root — a single folder on disk that MarkSpace treats as a
vault of markdown files. The **Projects Pane** lists them; the config stores up
to 20 recent ones as a TOML `[[projects]]` array.

> **Canonical term is _project_.** The PRD also says "workspace root" and
> "vault"; treat those as informal synonyms and prefer "project" in code and
> identifiers (`active_project_index`, `projects`). Reserve **workspace** for
> the overall app state, not a single directory root.

### Workspace
The overall application state — which project is active, panel visibility, focus
mode. Persisted under the config's `[workspace]` table. Not a single directory
(that's a **project**).

### Panels
The three top-level layout regions:

- **Projects Pane** — far-left, the project selector. Toggle: `Cmd/Ctrl + 1`.
- **Editor Canvas** (a.k.a. **Center Canvas**) — the center WYSIWYG editing
  surface. Always visible.
- **Context Column** — the far-right column, a vertical split of two sub-panels.
  Toggle: `Cmd/Ctrl + 2`.

### Context Column sub-panels
- **File Tree** — top ~70%, the nested folder/file explorer for the active
  project.
- **Quick Info** — bottom ~30%, live stats for the active file (word/char/line
  count, size, path, modified time). Collapses into a 24px status bar via
  `Cmd/Ctrl + I`.

### Focus Mode
The layout state where both the Projects Pane and Context Column are collapsed,
leaving only the Editor Canvas. Toggle: `Cmd/Ctrl + Shift + F`. Stored as
`workspace.focus_mode`.

### WYSIWYG rendering
The editor's live-rendering behaviour: raw markdown syntax (`#`, `**`, `>`, …)
is **hidden** when the caret leaves a line block and **re-exposed** when the
caret enters it. Driven by caret-position token matching over `pulldown-cmark`
events.

### Active file
The single markdown document currently open in the Editor Canvas. Its handle
feeds the Quick Info sub-panel.

### Config
Plain-text TOML at `~/.config/markspace/config.toml`. Holds `[workspace]`,
`[panels]`, and the `[[projects]]` registry.

## Terms to avoid

- **"vault"**, **"workspace root"** → use **project**.
- **"sidebar"** on its own is ambiguous → say **Projects Pane** or
  **Context Column**.
- **"metadata panel"** → use **Quick Info**.
