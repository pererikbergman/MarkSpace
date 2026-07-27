# ADR 0004: WYSIWYG editor — per-block live preview

- **Status:** Accepted
- **Date:** 2026-07-27
- **Phase:** 4 (WYSIWYG Editor Engine)
- **From:** design spike #23

## Context

The PRD (§2.1 Panel B, §4.2, §7) wants a live WYSIWYG markdown editor that
**hides raw syntax** (`#`, `**`, `>`, …) except on the block containing the
caret, re-exposing it on caret entry.

egui 0.35's `TextEdit` renders through a `layouter`
(`FnMut(&Ui, &dyn TextBuffer, f32) -> Arc<Galley>`). The caret and selection
map by character index into the produced **galley**, and the galley is laid out
from the buffer text. A layouter can therefore *style* characters (size, colour,
font) but **cannot omit them** — dropping the `#`/`**` glyphs desynchronises the
galley from the buffer and breaks caret/selection mapping. (Confirmed by reading
`egui-0.35 .../text_edit/builder.rs`.) So true syntax *hiding* is impossible in a
single `TextEdit`.

## Decision

Render the editor as **per-block live preview**:

- The **Document source** is the active file's full markdown as a single
  `String` — the single source of truth.
- Parsing (via `pulldown-cmark`'s offset iterator) yields **blocks** as
  byte-range spans. A block is a block-level element: heading, paragraph,
  blockquote, fenced code block, list item, thematic break; multi-line blocks
  stay whole.
- The **active block** (the one containing the caret) renders as a **raw,
  editable `TextEdit`** over its span (syntax shown). Every other block renders
  as **styled markdown** with syntax hidden.
- Editing the active block splices its new text back into the Document source
  and re-parses; **saving writes the source string**.

This reuses egui's per-block text editing instead of reimplementing a text
engine, and satisfies the hide-except-caret-block requirement structurally.

## Scope (MVP) and deferrals

- **Coarse caret placement** when entering a rendered block (block start /
  nearest line); glyph-precise rendered→source mapping is deferred (feasible,
  proven by the spike, built later).
- **Within-block editing.** Splitting emerges naturally from re-parsing when a
  blank line is typed. **Deferred:** cross-block caret flow (arrow past a
  block's edge) and backspace-at-start **merge** — the top fast-follow.
- Any block can be clicked to activate and edit it.

## Consequences

- `#24` (editable buffer) stays a design-independent tracer bullet: the whole
  Document source in one plain `TextEdit`. The block model arrives in `#27`.
- `#27` becomes "introduce the block model + render inactive blocks styled,
  active block raw"; `#28` becomes "caret-aware activation transitions" (both
  re-specified to this architecture).
- Deferred items (precise mapping, cross-block flow/merge) are the known gaps to
  reach editor-grade feel.

## Rejected alternatives

- **Single `TextEdit` + layouter styling** — cannot hide syntax (caret mapping
  breaks); fails the core requirement.
- **Fully custom text widget** — would reimplement selection, IME, clipboard,
  undo/redo, and keyboard navigation. Highest fidelity, disproportionate cost
  and risk for now.
- **True WYSIWYG (Medium / Notion style)** — never show markdown syntax; edit
  formatted rich text directly, format via ⌘B / a toolbar, with markdown as
  storage only. Rejected because (a) it contradicts PRD §2.1, which requires
  raw syntax to be *re-exposed on caret entry* — live preview, not rich text;
  and (b) editing formatted text with a caret inside styled spans is the same
  hidden-glyphs-break-the-caret problem, so it needs the custom-widget engine
  above. Chosen model shows raw markdown in the active block (Obsidian/Typora
  live preview), not a Medium-style rich-text surface.

## References

- `CONTEXT.md` — Document source, Block, Active block, WYSIWYG rendering.
- Issues #24, #27, #28.
- egui 0.35 `src/widgets/text_edit/builder.rs` — the `layouter` contract.
