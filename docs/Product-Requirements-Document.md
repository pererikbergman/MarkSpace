```
# Product Requirements Document (PRD): MarkSpace

**Document Version:** 1.0.0  
**Status:** Approved / Draft  
**Target Architecture:** Native Rust (`egui` / `eframe` ecosystem)  
**Binary Footprint:** Target < 15 MB | Memory Footprint < 50 MB RAM  

---

## 1. Executive Product Vision

The objective of **MarkSpace** is to engineer a local-first desktop application for software developers, technical writers, and knowledge workers who demand zero-latency markdown editing and structured workspace navigation. 

Unlike heavy Electron wrappers (MarkText, Obsidian, VS Code) that consume 100MB+ in disk space and hundreds of megabytes in memory, MarkSpace relies on a native, compiled Rust architecture. It introduces a tripartite spatial layout:

1. **Far-Left Navigation (Projects Pane):** A multi-workspace project selector inspired by `cmux` and terminal multiplexers.
2. **Center Canvas (WYSIWYG Editor):** A live-rendering Markdown editor with near-zero input latency.
3. **Far-Right Context Column:** A unified vertical split housing a file tree explorer on top and dynamic document metadata on the bottom.

---

## 2. Spatial Layout & UI Architecture

### Panel Layout Overview

+-------------+---------------------------------+------------------------------+
| Projects    | Main Editor Pane                | Explorer & Metadata Column   |
| (cmux)      | (WYSIWYG Workspace)             |                              |
+-------------+---------------------------------+------------------------------+
|             |                                 | [FILE TREE - Top Section]    |
| [P1] Core   | # System Architecture           |   src/                       |
| [P2] Docs   |                                 |     main.rs                  |
| [P3] Notes  | The primary writing surface...  |   docs/                      |
|             |                                 |   * architecture.md (Active) |
|             |                                 +------------------------------+
|             |                                 | [QUICK INFO - Bottom Split]  |
|             |                                 |   Words: 1,420 | Lines: 128  |
|             |                                 |   Size: 8.4 KB | Modified: 2m|
|             |                                 |   Path: /docs/architecture.md|
+-------------+---------------------------------+------------------------------+
| Collapsible | Focus Mode (Center Canvas)      | Collapsible (Vertical Split) |
+-------------+---------------------------------+------------------------------+

### 2.1 Panel Specifications & Behaviors

#### Panel A: Projects Pane (Far-Left)
* **Role:** Manage workspace directory roots (vaults/projects).
* **Behavior:** Collapsible vertical icon/label bar.
* **Shortcut:** `Cmd/Ctrl + 1` to toggle visibility.
* **Functionality:** 
  * Displays a list of active project roots saved in local configuration.
  * Allows quick switching between distinct file trees without resetting application state.
  * Drag-and-drop support to register new directory roots instantly.

#### Panel B: Main Editor Canvas (Center)
* **Role:** Primary live-rendering Markdown text editor.
* **Behavior:** Always visible; auto-expands to fill available horizontal screen real estate when sidebars are collapsed.
* **WYSIWYG Engine:** Hides raw formatting syntax (`#`, `**`, `*`, `>`) when the caret/cursor exits the active line block. Re-exposes raw syntax immediately upon caret entry.

#### Panel C: File Explorer & Context Column (Far-Right)
* **Role:** Combined structural browser and file property inspection pane.
* **Behavior:** Collapsible as a unified column using `Cmd/Ctrl + 2`.
* **Sub-Panel C1 (Top ~70%): File Tree**
  * Displays nested folder structures for the active project.
  * Supports asynchronous disk reads and dynamic directory expand/collapse.
* **Sub-Panel C2 (Bottom ~30%): Quick Info**
  * Displays active file stats: total word count, character count, line count, file size on disk, absolute/relative file path, and last modification timestamp.
  * Can be independently collapsed into an integrated 24px bottom status bar (`Cmd/Ctrl + I`).

---

## 3. Core Technical Requirements

### 3.1 Non-Functional Performance Thresholds
* **Compiled Binary Size:** Maximum 15 MB (target 8 to 10 MB).
* **Cold Boot Time:** Under 50ms from execution to interactive canvas.
* **Idle Memory Usage:** 25 MB to 45 MB RSS.
* **Directory Scan Latency:** Less than 10ms for folders containing up to 10,000 files.

### 3.2 Technology Stack
* **Language:** Rust.
* **GUI Engine:** `eframe` / `egui` (Immediate Mode GUI for ultra-low overhead).
* **Markdown Parser:** `pulldown-cmark` (Fast pull-parser for real-time token processing).
* **File System Operations:** `walkdir` (Directory traversal) + `notify` (Real-time OS file-system watching).
* **Configuration Format:** Plain text TOML stored at `~/.config/markspace/config.toml`.

---

## 4. Feature Matrix & System Capabilities

### 4.1 Project & Directory Management

* **Workspace Indexing:** Deep-scan active project directories asynchronously without blocking UI thread using `walkdir` running on a background worker thread via channels.
* **File System Watching:** Reflect external file additions, deletions, or renames instantly in the UI using the `notify` crate sending events to the event loop.
* **Project Switcher:** Store and recall up to 20 recent project root paths via a serialized TOML array loaded on startup.

### 4.2 WYSIWYG Editing Experience

* **Block Rendering:** Render Headers (H1-H6), Blockquotes, Lists, and Code blocks natively in custom fonts/sizes using a custom `egui` layout pipeline wrapping `pulldown-cmark` events.
* **Inline Formatting:** Auto-apply Bold, Italic, Strikethrough, and Inline Code styles via dynamic text layout styling based on caret position token matching.
* **Image Asset Staging:** Dropping an image creates a relative `./assets/` copy and inserts clean Markdown image syntax via a system drag-and-drop handler + filesystem copy utility.

---

## 5. Keyboard Navigation & Shortcuts

To support high-velocity, mouse-free workflows, all major layout states map to unified shortcuts:

* `Cmd/Ctrl + 1`: Toggle Projects Pane (Left)
* `Cmd/Ctrl + 2`: Toggle Explorer Column (Right)
* `Cmd/Ctrl + I`: Collapse/Expand Quick Info Sub-panel
* `Cmd/Ctrl + Shift + F`: **Focus Mode** (Collapse both Left and Right panels simultaneously)
* `Cmd/Ctrl + P`: Quick-open file palette across active workspace
* `Cmd/Ctrl + N`: Create new Markdown document in current directory

---

## 6. Data & Configuration Schema

Application layout state and project registry are stored locally in standard TOML format.

Sample configuration structure (`config.toml`):

[workspace]  
active_project_index = 0  
focus_mode = false  

[panels]  
show_projects_pane = true  
show_explorer_column = true  
quick_info_expanded = true  
explorer_split_ratio = 0.70  

[[projects]]  
name = "Core Docs"  
path = "/Users/developer/projects/core-docs"  

[[projects]]  
name = "Personal Notes"  
path = "/Users/developer/notes"  

---

## 7. Development Roadmap & Phased Execution

* **Phase 1 (Core Shell):** Set up `eframe`/`egui` application shell for MarkSpace, implement multi-panel layout containers (`SidePanel::left`, `SidePanel::right`, `CentralPanel`), and implement state toggles.
* **Phase 2 (File System & Tree):** Integrate `walkdir` and `notify` to render interactive collapsible folder trees in the top-right panel.
* **Phase 3 (Quick Info Integration):** Hook active file handle metadata (word count, size, timestamps) into the bottom-right panel.
* **Phase 4 (WYSIWYG Editor Engine):** Build the inline rendering pipeline using `pulldown-cmark` to handle live Markdown formatting state transitions based on caret location.
* **Phase 5 (Performance Optimization & Polish):** Optimize binary compilation flags (`strip = true`, `opt-level = "z"`), refine keyboard navigation shortcuts, and implement focus mode.

```
