use crate::{
    app::{App, Confirm, FormKind, Modal},
    export,
    story::{Kind, Story, StoryBlock},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

const BG: Color = Color::Rgb(22, 27, 32);
const PANEL: Color = Color::Rgb(27, 33, 39);
const INK: Color = Color::Rgb(225, 221, 210);
const MUTED: Color = Color::Rgb(144, 155, 163);
const LINE: Color = Color::Rgb(56, 67, 75);
const GOLD: Color = Color::Rgb(224, 180, 112);
const GREEN: Color = Color::Rgb(156, 192, 159);
const RED: Color = Color::Rgb(242, 145, 137);

fn color(hex: &str) -> Color {
    let rgb = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xb8afa0);
    // Lift dark character colors enough to be legible on the terminal background.
    Color::Rgb(
        ((rgb >> 16) as u8).saturating_add(55),
        ((rgb >> 8) as u8).saturating_add(55),
        (rgb as u8).saturating_add(55),
    )
}

fn panel(title: impl Into<Line<'static>>, active: bool) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .border_style(Style::default().fg(if active { GOLD } else { LINE }))
        .style(Style::default().bg(PANEL).fg(INK))
        .padding(Padding::horizontal(1))
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(BG).fg(INK)),
        area,
    );
    if area.width < 56 || area.height < 18 {
        let text = if matches!(app.modal, Some(Modal::Confirm(Confirm::Quit))) {
            "PLAYRITE · Unsaved changes\n\ns: Save and quit\nd: Discard and quit\nEsc: Keep writing"
                .to_owned()
        } else if app.error {
            format!("PLAYRITE\n{}\nResize to 56 × 18 to continue.", app.status)
        } else {
            "PLAYRITE\n\nPlease resize to at least 56 columns × 18 rows.\n\nYour writing is still here.\nCtrl+S saves · Ctrl+Q quits (with a save prompt)".to_owned()
        };
        frame.render_widget(
            Paragraph::new(text)
                .style(Style::default().fg(GOLD))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(2),
        Constraint::Length(2),
    ])
    .split(area);
    let dirty = app.document.dirty();
    let header = vec![
        Line::from(vec![
            Span::styled(
                "  PLAYRITE",
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  /  a quiet place to write", Style::default().fg(MUTED)),
        ]),
        Line::from(vec![
            Span::raw(format!("  {}", app.document.story.title)),
            Span::styled(
                if dirty {
                    "  ● unsaved"
                } else {
                    "  ✓ saved"
                },
                Style::default().fg(if dirty { GOLD } else { GREEN }),
            ),
            Span::styled(
                format!(
                    "    {} words · {} blocks",
                    app.document.story.word_count(),
                    app.document.story.blocks.len()
                ),
                Style::default().fg(MUTED),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(header), rows[0]);
    let columns = if area.width >= 110 {
        Layout::horizontal([
            Constraint::Percentage(25),
            Constraint::Percentage(43),
            Constraint::Percentage(32),
        ])
        .split(rows[1])
    } else {
        Layout::horizontal([Constraint::Length(23), Constraint::Min(20)]).split(rows[1])
    };
    outline(frame, app, columns[0]);
    manuscript(frame, app, columns[1]);
    if columns.len() > 2 {
        preview(frame, app, columns[2]);
    }
    frame.render_widget(
        Paragraph::new(format!("  {}", app.status))
            .style(Style::default().fg(if app.error { RED } else { MUTED }))
            .wrap(Wrap { trim: false }),
        rows[2],
    );
    let shortcuts = if app.editing {
        "  WRITING  Esc outline   Tab character   ^B bold   ^S save   ^E export\n  Enter newline   ^N new block   ^Z undo   ^Y redo   F1 help"
    } else {
        "  OUTLINE  ↑↓ select   Enter write   n add   c characters   t title\n  1–8 type   Tab speaker   J/K move   ^S save   ^E export   ? help"
    };
    frame.render_widget(
        Paragraph::new(shortcuts).style(Style::default().fg(GOLD)),
        rows[3],
    );
    if app.modal.is_some() {
        modal(frame, app);
    }
}

fn outline(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = panel(" Manuscript ", !app.editing && app.modal.is_none());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let capacity = (inner.height as usize / 3).max(1);
    if app.selected < app.outline_scroll {
        app.outline_scroll = app.selected;
    }
    if app.selected >= app.outline_scroll + capacity {
        app.outline_scroll = app.selected + 1 - capacity;
    }
    let items: Vec<ListItem> = app
        .document
        .story
        .blocks
        .iter()
        .enumerate()
        .skip(app.outline_scroll)
        .take(capacity)
        .map(|(i, b)| {
            let selected = i == app.selected;
            let c = app.document.story.character(b.character);
            let label = format!(
                "{} {:02} {}",
                if selected { "▸" } else { " " },
                i + 1,
                b.kind.label()
            );
            let snippet = if b.kind == Kind::Divider {
                "· · ·".into()
            } else {
                let text = b.text.lines().next().unwrap_or("");
                if text.is_empty() {
                    "Start writing…".into()
                } else {
                    format!(
                        "{}{}",
                        c.map(|c| format!("{}: ", c.name)).unwrap_or_default(),
                        text
                    )
                }
            };
            ListItem::new(vec![
                Line::styled(
                    label,
                    Style::default().fg(if selected {
                        GOLD
                    } else if b.kind == Kind::Note {
                        MUTED
                    } else {
                        INK
                    }),
                ),
                Line::styled(format!("   {snippet}"), Style::default().fg(MUTED)),
                Line::raw(""),
            ])
            .style(if selected {
                Style::default().bg(Color::Rgb(40, 46, 51))
            } else {
                Style::default()
            })
        })
        .collect();
    frame.render_widget(List::new(items), inner);
}

fn manuscript(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = panel(
        format!(
            " {} {} · {} ",
            app.block().kind.marker(),
            app.block().kind.label(),
            if app.editing {
                "writing"
            } else {
                "Enter to edit"
            }
        ),
        app.editing && app.modal.is_none(),
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let sections = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(inner);
    let c = app.document.story.character(app.block().character);
    let speaker = c.map(|c| c.name.as_str()).unwrap_or("Unassigned");
    frame.render_widget(
        Paragraph::new(format!("{speaker}  ·  Tab to change"))
            .style(Style::default().fg(c.map(|c| color(&c.color)).unwrap_or(MUTED))),
        sections[0],
    );
    let text_area = sections[1];
    app.editor_width = (text_area.width as usize).max(2);
    let wrapped = app.editor.wrapped(app.editor_width);
    let height = (text_area.height as usize).max(1);
    if wrapped.row < app.editor.scroll {
        app.editor.scroll = wrapped.row;
    }
    if wrapped.row >= app.editor.scroll + height {
        app.editor.scroll = wrapped.row + 1 - height;
    }
    if app.editor.text.is_empty() {
        let hint = match app.block().kind {
            Kind::Dialogue => {
                "What do they say?\n\nWrite dialogue without outer quotation marks. Assign a speaker with Tab."
            }
            Kind::Action => "A gesture. A movement. A pause.\n\nDescribe what happens.",
            Kind::Thought => "What goes unsaid?",
            Kind::Scene => "Give this scene a heading.",
            Kind::Divider => {
                "                 · · ·\n\nA little space between moments.\nThis block becomes a scene break."
            }
            Kind::Note => {
                "A thought for your future self.\n\nPrivate notes are saved here and omitted from the HTML export."
            }
            Kind::Quote => "Words worth setting apart.",
            Kind::Narration => "Every story starts somewhere.\n\nPress Enter and begin.",
        };
        frame.render_widget(
            Paragraph::new(hint)
                .style(Style::default().fg(MUTED))
                .wrap(Wrap { trim: false }),
            text_area,
        );
    } else {
        let lines: Vec<Line> = wrapped
            .lines
            .iter()
            .skip(app.editor.scroll)
            .take(height)
            .map(|s| Line::raw(s.clone()))
            .collect();
        frame.render_widget(Paragraph::new(lines), text_area);
    }
    let hint = if app.block().kind == Kind::Note {
        "PRIVATE · omitted from export\nCtrl+S saves your manuscript"
    } else {
        "**bold**  *italic*  ~~strike~~  `code`\nCtrl+N adds the next story block"
    };
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(MUTED)),
        sections[2],
    );
    if app.editing && app.modal.is_none() && text_area.height > 0 && text_area.width > 0 {
        frame.set_cursor_position((
            text_area.x + (wrapped.col as u16).min(text_area.width - 1),
            text_area.y + (wrapped.row - app.editor.scroll) as u16,
        ));
    }
}

fn formatted_lines(text: &str, base: Style) -> Vec<Line<'static>> {
    let mut lines = vec![Line::default()];
    for run in export::inline_runs(text) {
        let mut style = base;
        if run.format.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if run.format.italic {
            style = style.add_modifier(Modifier::ITALIC);
        }
        if run.format.strike {
            style = style.add_modifier(Modifier::CROSSED_OUT);
        }
        if run.format.code {
            style = style.fg(GOLD);
        }
        for (i, text) in run.text.split('\n').enumerate() {
            if i > 0 {
                lines.push(Line::default());
            }
            lines
                .last_mut()
                .unwrap()
                .spans
                .push(Span::styled(text.to_owned(), style));
        }
    }
    lines
}

fn block_preview(story: &Story, b: &StoryBlock) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if b.kind == Kind::Note {
        return lines;
    }
    if b.kind == Kind::Divider {
        return vec![
            Line::styled("           · · ·", Style::default().fg(GOLD)),
            Line::raw(""),
        ];
    }
    let c = story.character(b.character);
    if let Some(c) = c.filter(|_| {
        matches!(
            b.kind,
            Kind::Dialogue | Kind::Action | Kind::Thought | Kind::Quote
        )
    }) {
        lines.push(Line::styled(
            c.name.to_uppercase(),
            Style::default()
                .fg(color(&c.color))
                .add_modifier(Modifier::BOLD),
        ));
    }
    let base = match b.kind {
        Kind::Scene => Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        Kind::Action | Kind::Thought | Kind::Quote => {
            Style::default().fg(MUTED).add_modifier(Modifier::ITALIC)
        }
        _ => Style::default().fg(INK),
    };
    let text = if b.kind == Kind::Dialogue && !b.text.is_empty() {
        format!("“{}”", b.text)
    } else {
        b.text.clone()
    };
    lines.extend(formatted_lines(&text, base));
    lines.push(Line::raw(""));
    lines
}

fn preview(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = panel(" Reading preview ", false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let regions = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(inner);
    let mut lines = Vec::new();
    if app.block().kind == Kind::Note {
        lines.push(Line::styled(
            "Private note hidden from readers.",
            Style::default().fg(MUTED),
        ));
        lines.push(Line::raw(""));
    }
    for b in app.document.story.blocks.iter().skip(app.selected).take(8) {
        lines.extend(block_preview(&app.document.story, b));
    }
    let estimate: usize = lines
        .iter()
        .map(|l| l.width().div_ceil(regions[0].width.max(1) as usize).max(1))
        .sum();
    app.preview_scroll = app
        .preview_scroll
        .min(estimate.saturating_sub(1).min(u16::MAX as usize) as u16);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.preview_scroll, 0)),
        regions[0],
    );
    frame.render_widget(
        Paragraph::new("From selected block · PgUp/PgDn\nCtrl+E exports the complete story")
            .style(Style::default().fg(MUTED)),
        regions[1],
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(4));
    let h = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

fn modal(frame: &mut Frame, app: &mut App) {
    let modal = app.modal.as_ref().unwrap();
    let (title, width, height) = match modal {
        Modal::Help => (" Help · ↑↓ scroll · Esc close ", 84, 29),
        Modal::Add => (" Add a block · choose 1–8 ", 58, 15),
        Modal::Cast => (" Characters ", 72, 22),
        Modal::Form(f) => (f.title, 76, 7 + f.fields.len() as u16 * 3),
        Modal::Confirm(_) => (" One moment ", 72, 10),
    };
    let area = centered(frame.area(), width, height);
    frame.render_widget(Clear, area);
    let block = panel(title, true).padding(Padding::new(2, 2, 1, 1));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    match app.modal.as_mut().unwrap() {
        Modal::Help => {
            let text = "WRITE\nEnter / i   Edit the selected block; Esc returns to the outline\nEnter       New line while writing (Ctrl+N adds a new block)\nTab         Cycle the block’s character, including unassigned\nCtrl+B / I  Insert bold / italic markers around the cursor\n\nSHAPE YOUR STORY (in the outline)\n↑↓ / j k    Select a block           n       Add a block\n1–8         Change block type       J / K   Move down / up\nd           Delete (with prompt)    D       Duplicate\nc           Manage / assign cast    t / F2  Title and author\nu / r       Undo / redo             PgUp/Dn Scroll preview\n\nANYWHERE IN THE MANUSCRIPT\nCtrl+S      Save                    F4      Save a copy\nCtrl+E / F5 Export HTML             Ctrl+N  Add a block\nCtrl+K      Characters              Ctrl+T  Story details\nCtrl+Z / Y  Undo / redo             Ctrl+Q  Quit (save prompt)\n\nINLINE FORMATTING\n**bold**   *italic*   ~~strikethrough~~   `code`\nUse a backslash to escape a marker: \\*literal asterisk\\*\nPrivate notes never appear in the exported HTML.\nCharacter descriptions appear in the export’s cast list.";
            frame.render_widget(
                Paragraph::new(text)
                    .style(Style::default().fg(INK))
                    .wrap(Wrap { trim: false })
                    .scroll((app.help_scroll, 0)),
                inner,
            );
        }
        Modal::Add => {
            let descriptions = [
                "The voice of your story",
                "Words spoken by a character",
                "Gestures, movement, stage direction",
                "A character’s inner voice",
                "An excerpt or words set apart",
                "A heading, included in the contents",
                "A pause between moments",
                "For you only; never exported",
            ];
            let mut lines = vec![];
            for (i, kind) in Kind::ALL.iter().enumerate() {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{}  {:<13}", i + 1, kind.label()),
                        Style::default().fg(GOLD),
                    ),
                    Span::raw(descriptions[i]),
                ]));
            }
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "Inserted after the selected block. Esc cancels.",
                Style::default().fg(MUTED),
            ));
            frame.render_widget(Paragraph::new(lines), inner);
        }
        Modal::Cast => {
            let rows = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(inner);
            app.cast_selected = app
                .cast_selected
                .min(app.document.story.characters.len().saturating_sub(1));
            if app.document.story.characters.is_empty() {
                frame.render_widget(Paragraph::new("Who lives in your story?\n\nPress n to create your first character.\nGive them a name, a description, and a color.").style(Style::default().fg(MUTED)).wrap(Wrap { trim: false }), rows[0]);
            } else {
                let items: Vec<ListItem> = app
                    .document
                    .story
                    .characters
                    .iter()
                    .map(|c| {
                        ListItem::new(vec![
                            Line::styled(
                                c.name.clone(),
                                Style::default()
                                    .fg(color(&c.color))
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Line::styled(c.description.clone(), Style::default().fg(MUTED)),
                            Line::raw(""),
                        ])
                    })
                    .collect();
                let mut state = ListState::default().with_selected(Some(app.cast_selected));
                frame.render_stateful_widget(
                    List::new(items)
                        .highlight_symbol("▸ ")
                        .highlight_style(Style::default().bg(LINE)),
                    rows[0],
                    &mut state,
                );
            }
            frame.render_widget(Paragraph::new("n new   e edit   d remove   ↑↓ select\nEnter assigns to current block   0 clears assignment\nEsc closes · descriptions are included in HTML").style(Style::default().fg(GOLD)), rows[1]);
        }
        Modal::Form(form) => {
            for (i, field) in form.fields.iter().enumerate() {
                let y = inner.y + i as u16 * 3;
                if y + 1 >= inner.bottom() {
                    break;
                }
                let selected = i == form.selected;
                frame.render_widget(
                    Paragraph::new(field.label).style(Style::default().fg(if selected {
                        GOLD
                    } else {
                        MUTED
                    })),
                    Rect::new(inner.x, y, inner.width, 1),
                );
                let before = UnicodeWidthStr::width(&field.editor.text[..field.editor.cursor]);
                let scroll = before
                    .saturating_sub(inner.width.saturating_sub(1) as usize)
                    .min(u16::MAX as usize) as u16;
                frame.render_widget(
                    Paragraph::new(field.editor.text.clone())
                        .style(Style::default().bg(LINE).fg(INK))
                        .scroll((0, scroll)),
                    Rect::new(inner.x, y + 1, inner.width, 1),
                );
                if selected {
                    frame.set_cursor_position((
                        inner.x
                            + (before.saturating_sub(scroll as usize) as u16)
                                .min(inner.width.saturating_sub(1)),
                        y + 1,
                    ));
                }
            }
            let footer_y = inner.y + form.fields.len() as u16 * 3;
            if footer_y < inner.bottom() {
                let hint = if !form.error.is_empty() {
                    form.error.clone()
                } else if matches!(form.kind, FormKind::Export) {
                    "Enter export · Esc cancel · includes unsaved edits".into()
                } else {
                    "Tab next · Ctrl+U clear · Enter apply · Esc cancel".into()
                };
                frame.render_widget(
                    Paragraph::new(hint)
                        .style(Style::default().fg(if form.error.is_empty() { MUTED } else { RED }))
                        .wrap(Wrap { trim: false }),
                    Rect::new(inner.x, footer_y, inner.width, inner.bottom() - footer_y),
                );
            }
        }
        Modal::Confirm(confirm) => {
            let text = match confirm {
                Confirm::Quit => "Your manuscript has unsaved changes.\n\ns  Save and quit    d  Discard and quit\nEsc  Keep writing".into(),
                Confirm::DeleteBlock => "Delete the selected block?\n\ny  Delete    n / Esc  Keep it\nYou can undo this with u or Ctrl+Z.".into(),
                Confirm::DeleteCharacter(_) => "Remove this character and their block assignments?\nTheir dialogue and other text will be preserved.\n\ny  Remove    n / Esc  Keep them".into(),
                Confirm::Overwrite(path) => format!("{} already exists.\n\ny  Replace the HTML file    n / Esc  Cancel", path.display()),
            };
            frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::Document;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn renders_wide_narrow_and_small_terminals_with_dialogs() {
        for (w, h) in [(140, 40), (80, 24), (56, 18), (30, 8)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            let mut app = App::new(Document::new("test.json".into(), Story::demo()));
            terminal.draw(|f| draw(f, &mut app)).unwrap();
            app.key(crossterm::event::KeyCode::Char('c').into());
            terminal.draw(|f| draw(f, &mut app)).unwrap();
            app.key(crossterm::event::KeyCode::Char('n').into());
            terminal.draw(|f| draw(f, &mut app)).unwrap();
            let content: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            assert!(content.contains("PLAYRITE"));
        }
    }
}
