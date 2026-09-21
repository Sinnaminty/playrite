# Playrite

A terminal writing desk for character-driven stories, with a standalone HTML reader.

Write in small, movable blocks. Give dialogue a speaker, distinguish actions from thoughts, and keep private notes alongside the manuscript. Export a single HTML file with book-like typography, character colors, scene navigation, dark mode, and print styling. No network, fonts, scripts, or companion files are needed to read an export.

## Run

Requires Rust 1.88 or newer and an interactive terminal.

```sh
cargo run --release -- my-story.playrite
```

An existing file opens for editing; a missing file starts a blank story. With no filename, Playrite uses `story.playrite`. Save with **Ctrl+S**. Playrite does not autosave; quitting with unsaved changes offers save, discard, or cancel.

Try the included sample, *The last light*:

```sh
cargo run --release -- demo.playrite
```

To create a fresh sample at another path, run `cargo run --release -- demo my-demo.playrite` and press Ctrl+S to save. The `demo` command defaults to `demo.playrite` and refuses to replace an existing file.

To install the executable:

```sh
cargo install --path .
playrite my-story.playrite
```

## A first writing session

1. Press **t** to set your title, subtitle, and author. Tab moves between fields; Enter applies the form.
2. Press **c**, then **n**, to create a character. Add a name, description, and color. Ctrl+P cycles suggested colors. Descriptions appear in the HTML character list.
3. Press **Esc** to return to the manuscript, then **Enter** to write in the selected block.
4. Press **Ctrl+N**, then a number, to add your next block. For dialogue, choose **2**. **Tab** cycles speakers, including an unassigned option.
5. Press **Esc** to navigate the outline. **J** and **K** move the selected block down and up.
6. **Ctrl+S** saves the story. **Ctrl+E** exports it to an HTML file.

The terminal uses an outline, writing pane, and live reading preview. At widths below 110 columns, the preview is hidden to give the editor more room. The minimum terminal size is 56 × 18. Help is available with **?** in the outline or **F1** while writing; arrow keys scroll help.

## Story blocks

| Key | Type | Reader appearance |
| --- | --- | --- |
| 1 | Narration | Ordinary prose |
| 2 | Dialogue | Speaker label, character-colored rule, quotation marks |
| 3 | Action | Italic action or stage direction, optional character label |
| 4 | Thought | Interior voice, labeled as thinking |
| 5 | Quotation | Indented quotation, optional character attribution |
| 6 | Scene | Heading and automatic contents entry |
| 7 | Scene break | A decorative pause |
| 8 | Private note | Saved in the manuscript; omitted from export |

In the outline, a number changes the selected block’s type. After **n** or **Ctrl+N**, a number creates a new block after the current one. Changing type preserves its text and character assignment. Scene breaks do not display their text; changing back to another type restores its display.

Dialogue gets outer quotation marks automatically, so you can write the spoken words without them. Enter inserts a newline within a block; two newlines separate paragraphs. Character assignment is displayed for dialogue, actions, thoughts, and quotations.

Inline formatting works in prose and scene headings:

```text
**bold**   *italic*   ~~strikethrough~~   `code`
\*literal asterisks\*
```

This is a small inline language, not full Markdown. Raw HTML and URLs are treated as text. Ctrl+B inserts paired bold markers around the cursor; Ctrl+I does the same for italics in terminals that distinguish it from Tab. In most terminals, type `*` directly for italics because Ctrl+I is sent as Tab.

## Keys

| Context | Keys | Action |
| --- | --- | --- |
| Outline | ↑/↓ or j/k | Select block |
| Outline | Enter or i | Write in selected block |
| Writing | Esc | Return to outline |
| Writing | Arrows, Home, End | Move cursor; text wraps automatically |
| Manuscript | Tab | Cycle assigned character |
| Outline | n | Add block |
| Outline | J / K | Move block down / up |
| Outline | D | Duplicate block |
| Outline | d or Delete | Delete block, with confirmation |
| Outline | c / t | Characters / story details |
| Outline | u / r | Undo / redo |
| Outline | PgUp / PgDn | Scroll reading preview |
| Manuscript | Ctrl+N / Ctrl+K / Ctrl+T | New block / characters / story details |
| Manuscript | Ctrl+Z / Ctrl+Y | Undo / redo (up to 100 change groups) |
| Manuscript | Ctrl+S | Save |
| Manuscript | F4 | Save to a new filename and continue editing that copy |
| Manuscript | Ctrl+E or F5 | Export HTML |
| Manuscript | Ctrl+Q | Quit, with an unsaved-changes prompt |
| Form | Tab / Shift+Tab | Next / previous field |
| Form | Enter / Esc | Apply / cancel |
| Form | Ctrl+U | Clear the current field |

Character management also supports **e** to edit, **d** to remove, **Enter** to assign, and **0** to clear the current block’s assignment. Removing a character preserves all story text, and can be undone. Undo history lasts for the current editing session.

## Export without the editor

```sh
playrite export my-story.playrite -o my-story.html
```

The default output replaces the source extension with `.html`. Existing exports require `--force` on the command line, or confirmation in the editor. The source story cannot be used as the export destination.

The editor exports the current manuscript, including unsaved changes. Command-line export reads the saved file. Private notes are omitted in both cases. HTML includes all styles, adapts to the reader’s light/dark preference, supports mobile screens, and has print-specific styling. The cast list can be expanded, and stories with multiple scene headings get a linked contents list.

## Files and development

Stories use a compact, versioned binary `.playrite` format with metadata, characters, and an ordered list of blocks. Text is stored as UTF-8, with no JSON keys or escaping. The [format specification](FORMAT.md) documents the byte layout. Keep the `.playrite` manuscript to continue editing; HTML is the published reading copy and cannot be imported as a manuscript.

Existing JSON stories still open and export. All saves use binary: **Ctrl+S** replaces the contents at the current path, even if it ends in `.json`. To preserve a JSON original, open it and use **F4** to save a new `.playrite` copy.

Saves use a temporary file in the destination directory followed by an atomic replacement. If a story changes on disk after being opened, Playrite refuses to overwrite it; **F4** saves your edits to a separate file. Parent directories must already exist.

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Built with [Ratatui](https://docs.rs/ratatui/0.29.0/ratatui/) and Crossterm.
