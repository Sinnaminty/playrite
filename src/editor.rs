use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Default, Clone)]
pub struct Editor {
    pub text: String,
    pub cursor: usize,
    pub scroll: usize,
}

pub struct Wrapped {
    pub lines: Vec<String>,
    pub positions: Vec<(usize, usize, usize)>, // byte offset, row, display column
    pub row: usize,
    pub col: usize,
}

impl Editor {
    pub fn new(text: String) -> Self {
        let cursor = text.len();
        Self {
            text,
            cursor,
            scroll: 0,
        }
    }

    pub fn insert(&mut self, text: &str) {
        // Terminal control characters are not manuscript content.
        let clean: String = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .collect();
        let clean = clean.replace('\t', "    ");
        self.text.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
        // Inserting a combining character can change neighboring grapheme boundaries.
        self.cursor = self
            .text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i >= self.cursor)
            .unwrap_or(self.text.len());
    }

    fn previous(&self) -> usize {
        self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    fn next(&self) -> usize {
        self.text[self.cursor..]
            .graphemes(true)
            .next()
            .map(|g| self.cursor + g.len())
            .unwrap_or(self.cursor)
    }

    pub fn wrapped(&self, width: usize) -> Wrapped {
        let width = width.max(2);
        let mut lines = vec![String::new()];
        let mut positions = Vec::new();
        let (mut row, mut col) = (0, 0);
        let graphemes: Vec<_> = self.text.grapheme_indices(true).collect();
        for (i, &(byte, grapheme)) in graphemes.iter().enumerate() {
            let w = UnicodeWidthStr::width(grapheme);
            let word_start = !grapheme.chars().all(char::is_whitespace)
                && (i == 0 || graphemes[i - 1].1.chars().all(char::is_whitespace));
            let word_width: usize = if word_start {
                graphemes[i..]
                    .iter()
                    .take_while(|(_, g)| !g.chars().any(char::is_whitespace))
                    .map(|(_, g)| UnicodeWidthStr::width(*g))
                    .sum()
            } else {
                0
            };
            let wrap_word =
                word_start && col > 0 && word_width <= width && col + word_width > width;
            if grapheme != "\n" && (col + w > width || wrap_word) {
                lines.push(String::new());
                row += 1;
                col = 0;
            }
            positions.push((byte, row, col));
            if grapheme == "\n" {
                lines.push(String::new());
                row += 1;
                col = 0;
            } else {
                lines[row].push_str(grapheme);
                col += w;
            }
        }
        if col >= width {
            lines.push(String::new());
            row += 1;
            col = 0;
        }
        positions.push((self.text.len(), row, col));
        let (_, row, col) = positions
            .iter()
            .copied()
            .find(|(byte, _, _)| *byte == self.cursor)
            .unwrap_or((0, 0, 0));
        Wrapped {
            lines,
            positions,
            row,
            col,
        }
    }

    pub fn key(&mut self, key: KeyEvent, width: usize, multiline: bool) {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return;
        }
        match key.code {
            KeyCode::Char(c) => self.insert(&c.to_string()),
            KeyCode::Enter if multiline => self.insert("\n"),
            KeyCode::Backspace => {
                let start = self.previous();
                self.text.replace_range(start..self.cursor, "");
                self.cursor = start;
            }
            KeyCode::Delete => {
                self.text.replace_range(self.cursor..self.next(), "");
            }
            KeyCode::Left => self.cursor = self.previous(),
            KeyCode::Right => self.cursor = self.next(),
            KeyCode::Home => {
                self.cursor = self.text[..self.cursor]
                    .rfind('\n')
                    .map(|i| i + 1)
                    .unwrap_or(0)
            }
            KeyCode::End => {
                self.cursor = self.text[self.cursor..]
                    .find('\n')
                    .map(|i| i + self.cursor)
                    .unwrap_or(self.text.len())
            }
            KeyCode::Up | KeyCode::Down if multiline => {
                let wrapped = self.wrapped(width);
                let target = if key.code == KeyCode::Up {
                    wrapped.row.saturating_sub(1)
                } else {
                    (wrapped.row + 1).min(wrapped.lines.len() - 1)
                };
                if let Some((byte, _, _)) = wrapped
                    .positions
                    .iter()
                    .filter(|(_, row, _)| *row == target)
                    .min_by_key(|(_, _, col)| col.abs_diff(wrapped.col))
                {
                    self.cursor = *byte;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grapheme_editing_and_paste() {
        let mut e = Editor::new("café 👩‍🚀".into());
        e.key(KeyCode::Backspace.into(), 20, true);
        assert_eq!(e.text, "café ");
        e.insert("e\u{301}");
        e.key(KeyCode::Backspace.into(), 20, true);
        assert_eq!(e.text, "café ");
        e.insert("hello\r\nworld\x1b");
        assert_eq!(e.text, "café hello\nworld");
    }

    #[test]
    fn wrapping_tracks_wide_characters_and_cursor() {
        let e = Editor::new("ab界c".into());
        let w = e.wrapped(4);
        assert_eq!(w.lines, ["ab界", "c"]);
        assert_eq!((w.row, w.col), (1, 1));
        let mut e = Editor::new("abcdef".into());
        e.key(KeyCode::Up.into(), 4, true);
        assert_eq!(e.cursor, 2);
    }
}
