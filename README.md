# MarkSpace

A local-first, native **Rust** desktop markdown editor built for zero-latency
editing and structured workspace navigation.

Unlike heavy Electron-based editors that consume 100MB+ on disk and hundreds of
megabytes of RAM, MarkSpace targets a tiny footprint (**< 15 MB binary**,
**< 50 MB RAM**) using the `egui` / `eframe` immediate-mode GUI ecosystem.

## Layout

MarkSpace uses a tripartite spatial layout:

1. **Projects Pane (far-left)** — multi-workspace project/vault selector.
2. **Editor Canvas (center)** — live-rendering WYSIWYG markdown editor with
   near-zero input latency.
3. **Context Column (far-right)** — file tree explorer on top, live document
   metadata (word/line/char counts, size, path, modified time) on the bottom.

## Tech Stack

- **Language:** Rust
- **GUI:** [`eframe`](https://crates.io/crates/eframe) / [`egui`](https://crates.io/crates/egui)
- **Markdown:** [`pulldown-cmark`](https://crates.io/crates/pulldown-cmark)
- **File system:** [`walkdir`](https://crates.io/crates/walkdir) + [`notify`](https://crates.io/crates/notify)
- **Config:** TOML at `~/.config/markspace/config.toml`

## Keyboard Shortcuts

| Shortcut | Action |
| --- | --- |
| `Cmd/Ctrl + 1` | Toggle Projects Pane |
| `Cmd/Ctrl + 2` | Toggle Explorer Column |
| `Cmd/Ctrl + I` | Collapse/Expand Quick Info |
| `Cmd/Ctrl + Shift + F` | Focus Mode (collapse both sidebars) |
| `Cmd/Ctrl + P` | Quick-open file palette |
| `Cmd/Ctrl + N` | New markdown document |

## Status

Early development. See the full
[Product Requirements Document](docs/Product-Requirements-Document.md) for the
complete spec and phased roadmap.

## License

Licensed under the [MIT License](LICENSE).
