use crate::{
    editor::Editor,
    export,
    story::{Character, Document, Kind, PALETTE, Story, StoryBlock, valid_color},
    ui,
};
use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
};
use std::{
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

pub struct Field {
    pub label: &'static str,
    pub editor: Editor,
}

pub enum FormKind {
    Metadata,
    Character(Option<u64>),
    Export,
    SaveCopy,
}

pub struct Form {
    pub kind: FormKind,
    pub title: &'static str,
    pub fields: Vec<Field>,
    pub selected: usize,
    pub error: String,
}

impl Form {
    fn new(kind: FormKind, title: &'static str, fields: Vec<(&'static str, String)>) -> Self {
        Self {
            kind,
            title,
            fields: fields
                .into_iter()
                .map(|(label, text)| Field {
                    label,
                    editor: Editor::new(text),
                })
                .collect(),
            selected: 0,
            error: String::new(),
        }
    }
}

pub enum Confirm {
    Quit,
    DeleteBlock,
    DeleteCharacter(u64),
    Overwrite(PathBuf),
}
pub enum Modal {
    Help,
    Add,
    Cast,
    Form(Form),
    Confirm(Confirm),
}

pub struct App {
    pub document: Document,
    pub selected: usize,
    pub editing: bool,
    pub editor: Editor,
    pub modal: Option<Modal>,
    pub cast_selected: usize,
    pub status: String,
    pub error: bool,
    pub editor_width: usize,
    pub outline_scroll: usize,
    pub preview_scroll: u16,
    pub help_scroll: u16,
    undo: Vec<Story>,
    redo: Vec<Story>,
    last_edit: Option<Instant>,
    pub quit: bool,
}

impl App {
    pub fn new(document: Document) -> Self {
        let editor = Editor::new(document.story.blocks[0].text.clone());
        Self {
            document,
            selected: 0,
            editing: false,
            editor,
            modal: None,
            cast_selected: 0,
            status: "Welcome. Enter to write · n to add a block · c for characters · ? for help"
                .into(),
            error: false,
            editor_width: 40,
            outline_scroll: 0,
            preview_scroll: 0,
            help_scroll: 0,
            undo: vec![],
            redo: vec![],
            last_edit: None,
            quit: false,
        }
    }

    pub fn block(&self) -> &StoryBlock {
        &self.document.story.blocks[self.selected]
    }

    fn message(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.error = false;
    }
    fn failure(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.error = true;
    }

    fn checkpoint(&mut self) {
        if self.undo.last() != Some(&self.document.story) {
            self.undo.push(self.document.story.clone());
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_edit = None;
    }

    fn sync_editor(&mut self) {
        self.selected = self.selected.min(self.document.story.blocks.len() - 1);
        self.cast_selected = self
            .cast_selected
            .min(self.document.story.characters.len().saturating_sub(1));
        self.editor = Editor::new(self.block().text.clone());
        self.last_edit = None;
        self.preview_scroll = 0;
    }

    fn text_changed(&mut self, old: &str) {
        if old != self.editor.text {
            if self
                .last_edit
                .is_none_or(|t| t.elapsed() > Duration::from_millis(700))
            {
                self.checkpoint();
            }
            self.document.story.blocks[self.selected].text = self.editor.text.clone();
            self.last_edit = Some(Instant::now());
        }
    }

    pub fn paste(&mut self, text: &str) {
        match self.modal.as_mut() {
            Some(Modal::Form(form)) => form.fields[form.selected]
                .editor
                .insert(&text.replace(['\r', '\n'], " ")),
            None if self.editing => {
                let old = self.editor.text.clone();
                self.last_edit = None;
                self.editor.insert(text);
                self.text_changed(&old);
                self.last_edit = None;
            }
            _ => {}
        }
    }

    fn save(&mut self) -> bool {
        match self.document.save() {
            Ok(()) => {
                self.last_edit = None;
                self.message(format!("Saved {}", self.document.path.display()));
                true
            }
            Err(e) => {
                self.failure(e);
                false
            }
        }
    }

    fn export_form(&mut self) {
        self.modal = Some(Modal::Form(Form::new(
            FormKind::Export,
            " Export story ",
            vec![(
                "HTML file",
                self.document
                    .path
                    .with_extension("html")
                    .display()
                    .to_string(),
            )],
        )));
    }

    fn metadata_form(&mut self) {
        let story = &self.document.story;
        self.modal = Some(Modal::Form(Form::new(
            FormKind::Metadata,
            " Story details ",
            vec![
                ("Title", story.title.clone()),
                ("Subtitle", story.subtitle.clone()),
                ("Author", story.author.clone()),
            ],
        )));
    }

    fn character_form(&mut self, id: Option<u64>) {
        let c = self.document.story.character(id);
        let fields = vec![
            ("Name", c.map(|c| c.name.clone()).unwrap_or_default()),
            (
                "Description (published in cast)",
                c.map(|c| c.description.clone()).unwrap_or_default(),
            ),
            (
                "Color (#RRGGBB; Ctrl+P cycles palette)",
                c.map(|c| c.color.clone()).unwrap_or_else(|| {
                    PALETTE[self.document.story.characters.len() % PALETTE.len()].into()
                }),
            ),
        ];
        self.modal = Some(Modal::Form(Form::new(
            FormKind::Character(id),
            " Character details ",
            fields,
        )));
    }

    fn export_to(&mut self, path: PathBuf, overwrite: bool) {
        match export::write(&self.document.story, &path, overwrite) {
            Ok(()) => self.message(format!(
                "Exported {} · private notes omitted",
                path.display()
            )),
            Err(e) => self.failure(e),
        }
    }

    fn submit(&mut self, mut form: Form) {
        let values: Vec<String> = form
            .fields
            .iter()
            .map(|f| f.editor.text.trim().to_owned())
            .collect();
        match form.kind {
            FormKind::Metadata => {
                if values[0].is_empty() {
                    form.error = "Give your story a title.".into();
                    self.modal = Some(Modal::Form(form));
                    return;
                }
                self.checkpoint();
                self.document.story.title = values[0].clone();
                self.document.story.subtitle = values[1].clone();
                self.document.story.author = values[2].clone();
                self.message("Story details updated");
            }
            FormKind::Character(id) => {
                if values[0].is_empty() || !valid_color(&values[2]) {
                    form.error = "A name and a valid #RRGGBB color are required.".into();
                    self.modal = Some(Modal::Form(form));
                    return;
                }
                self.checkpoint();
                let id = id.unwrap_or_else(|| {
                    (1..)
                        .find(|id| !self.document.story.characters.iter().any(|c| c.id == *id))
                        .unwrap()
                });
                let character = Character {
                    id,
                    name: values[0].clone(),
                    description: values[1].clone(),
                    color: values[2].clone(),
                };
                if let Some(c) = self
                    .document
                    .story
                    .characters
                    .iter_mut()
                    .find(|c| c.id == id)
                {
                    *c = character;
                } else {
                    self.document.story.characters.push(character);
                    self.cast_selected = self.document.story.characters.len() - 1;
                }
                self.message(
                    "Character saved · select them and press Enter to assign to this block",
                );
                self.modal = Some(Modal::Cast);
            }
            FormKind::Export => {
                let path = PathBuf::from(&values[0]);
                // A story may have any extension; never let an export replace the source.
                let same = path == self.document.path
                    || std::fs::canonicalize(&path)
                        .ok()
                        .zip(std::fs::canonicalize(&self.document.path).ok())
                        .is_some_and(|(a, b)| a == b);
                if same {
                    form.error = "The export must use a different path from your story.".into();
                    self.modal = Some(Modal::Form(form));
                } else if path.exists() {
                    self.modal = Some(Modal::Confirm(Confirm::Overwrite(path)));
                } else {
                    self.export_to(path, false);
                }
            }
            FormKind::SaveCopy => match self.document.save_copy(PathBuf::from(&values[0])) {
                Ok(()) => self.message(format!("Saved copy to {}", self.document.path.display())),
                Err(e) => {
                    form.error = e;
                    self.modal = Some(Modal::Form(form));
                }
            },
        }
    }

    pub fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        if let Some(modal) = self.modal.take() {
            self.modal_key(modal, key);
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl {
            match key.code {
                KeyCode::Char('s') => {
                    self.save();
                }
                KeyCode::Char('e') => self.export_form(),
                KeyCode::Char('q') | KeyCode::Char('c') => self.request_quit(),
                KeyCode::Char('n') => self.modal = Some(Modal::Add),
                KeyCode::Char('k') => self.modal = Some(Modal::Cast),
                KeyCode::Char('t') => self.metadata_form(),
                KeyCode::Char('z') => self.undo(false),
                KeyCode::Char('y') => self.undo(true),
                KeyCode::Char('b') if self.editing => self.insert_format("**"),
                KeyCode::Char('i') if self.editing => self.insert_format("*"),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::F(1) => {
                self.modal = Some(Modal::Help);
                return;
            }
            KeyCode::F(2) => {
                self.metadata_form();
                return;
            }
            KeyCode::F(4) => {
                self.modal = Some(Modal::Form(Form::new(
                    FormKind::SaveCopy,
                    " Save a copy (new filename) ",
                    vec![(
                        "Story file",
                        self.document
                            .path
                            .with_extension("copy.json")
                            .display()
                            .to_string(),
                    )],
                )));
                return;
            }
            KeyCode::F(5) => {
                self.export_form();
                return;
            }
            _ => {}
        }
        if self.editing {
            if key.code == KeyCode::Esc {
                self.editing = false;
                self.last_edit = None;
                return;
            }
            if key.code == KeyCode::Tab {
                self.cycle_character();
                return;
            }
            let old = self.editor.text.clone();
            self.editor.key(key, self.editor_width, true);
            self.text_changed(&old);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.request_quit(),
            KeyCode::Char('?') => self.modal = Some(Modal::Help),
            KeyCode::Char('n') => self.modal = Some(Modal::Add),
            KeyCode::Char('c') => self.modal = Some(Modal::Cast),
            KeyCode::Char('t') => self.metadata_form(),
            KeyCode::Char('e') => self.export_form(),
            KeyCode::Char('s') => {
                self.save();
            }
            KeyCode::Char('u') => self.undo(false),
            KeyCode::Char('r') => self.undo(true),
            KeyCode::Enter | KeyCode::Char('i') => {
                if self.block().kind == Kind::Divider {
                    self.message("Scene breaks have no text. Press 1–8 to change the block type.");
                } else {
                    self.editing = true;
                    self.last_edit = None;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                self.sync_editor();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.document.story.blocks.len() - 1);
                self.sync_editor();
            }
            KeyCode::Home => {
                self.selected = 0;
                self.sync_editor();
            }
            KeyCode::End => {
                self.selected = self.document.story.blocks.len() - 1;
                self.sync_editor();
            }
            KeyCode::PageDown => self.preview_scroll = self.preview_scroll.saturating_add(5),
            KeyCode::PageUp => self.preview_scroll = self.preview_scroll.saturating_sub(5),
            KeyCode::Char('J') => self.move_block(1),
            KeyCode::Char('K') => self.move_block(-1),
            KeyCode::Char('d') | KeyCode::Delete => {
                self.modal = Some(Modal::Confirm(Confirm::DeleteBlock))
            }
            KeyCode::Char('D') => {
                self.checkpoint();
                self.document
                    .story
                    .blocks
                    .insert(self.selected + 1, self.block().clone());
                self.selected += 1;
                self.sync_editor();
                self.message("Block duplicated");
            }
            KeyCode::Tab => self.cycle_character(),
            KeyCode::Char(c @ '1'..='8') => {
                self.checkpoint();
                self.document.story.blocks[self.selected].kind =
                    Kind::ALL[c as usize - '1' as usize];
                self.message(format!("Block changed to {}", self.block().kind.label()));
            }
            _ => {}
        }
    }

    fn insert_format(&mut self, delimiter: &str) {
        let old = self.editor.text.clone();
        self.last_edit = None;
        self.editor.insert(&delimiter.repeat(2));
        self.editor.cursor -= delimiter.len();
        self.text_changed(&old);
        self.last_edit = None;
    }

    fn request_quit(&mut self) {
        if self.document.dirty() {
            self.modal = Some(Modal::Confirm(Confirm::Quit));
        } else {
            self.quit = true;
        }
    }

    fn move_block(&mut self, direction: isize) {
        let target = self
            .selected
            .saturating_add_signed(direction)
            .min(self.document.story.blocks.len() - 1);
        if target != self.selected {
            self.checkpoint();
            self.document.story.blocks.swap(self.selected, target);
            self.selected = target;
            self.sync_editor();
        }
    }

    fn cycle_character(&mut self) {
        let chars = &self.document.story.characters;
        if chars.is_empty() {
            self.character_form(None);
            return;
        }
        let next = chars
            .iter()
            .position(|c| Some(c.id) == self.block().character)
            .map(|i| i + 1)
            .unwrap_or(0);
        let id = chars.get(next).map(|c| c.id);
        self.checkpoint();
        self.document.story.blocks[self.selected].character = id;
    }

    fn undo(&mut self, redo: bool) {
        let previous = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        if let Some(previous) = previous {
            let current = std::mem::replace(&mut self.document.story, previous);
            if redo {
                self.undo.push(current);
            } else {
                self.redo.push(current);
            }
            self.sync_editor();
            self.message(if redo {
                "Change restored"
            } else {
                "Change undone"
            });
        }
    }

    fn modal_key(&mut self, modal: Modal, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            if matches!(
                modal,
                Modal::Form(Form {
                    kind: FormKind::Character(_),
                    ..
                })
            ) {
                self.modal = Some(Modal::Cast);
            }
            return;
        }
        match modal {
            Modal::Help => {
                match key.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.help_scroll = (self.help_scroll + 1).min(23)
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.help_scroll = self.help_scroll.saturating_sub(1)
                    }
                    KeyCode::PageDown => self.help_scroll = (self.help_scroll + 6).min(23),
                    KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(6),
                    _ => {}
                }
                if !matches!(
                    key.code,
                    KeyCode::Char('?') | KeyCode::F(1) | KeyCode::Enter
                ) {
                    self.modal = Some(Modal::Help);
                } else {
                    self.help_scroll = 0;
                }
            }
            Modal::Add => {
                if let KeyCode::Char(c @ '1'..='8') = key.code {
                    self.checkpoint();
                    let kind = Kind::ALL[c as usize - '1' as usize];
                    let character = if matches!(kind, Kind::Dialogue | Kind::Action | Kind::Thought)
                    {
                        self.block().character
                    } else {
                        None
                    };
                    self.selected += 1;
                    self.document.story.blocks.insert(
                        self.selected,
                        StoryBlock {
                            kind,
                            character,
                            text: String::new(),
                        },
                    );
                    self.sync_editor();
                    self.editing = kind != Kind::Divider;
                    self.message("New block · Tab assigns a character · Esc returns to outline");
                } else {
                    self.modal = Some(Modal::Add);
                }
            }
            Modal::Cast => {
                self.modal = Some(Modal::Cast);
                let len = self.document.story.characters.len();
                match key.code {
                    KeyCode::Char('n') => self.character_form(None),
                    KeyCode::Char('e') if len > 0 => self.character_form(Some(
                        self.document.story.characters[self.cast_selected].id,
                    )),
                    KeyCode::Char('d') | KeyCode::Delete if len > 0 => {
                        self.modal = Some(Modal::Confirm(Confirm::DeleteCharacter(
                            self.document.story.characters[self.cast_selected].id,
                        )))
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.cast_selected = self.cast_selected.saturating_sub(1)
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.cast_selected = (self.cast_selected + 1).min(len.saturating_sub(1))
                    }
                    KeyCode::Enter if len > 0 => {
                        self.checkpoint();
                        self.document.story.blocks[self.selected].character =
                            Some(self.document.story.characters[self.cast_selected].id);
                        self.modal = None;
                        self.message("Character assigned");
                    }
                    KeyCode::Char('0') => {
                        self.checkpoint();
                        self.document.story.blocks[self.selected].character = None;
                        self.modal = None;
                    }
                    _ => {}
                }
            }
            Modal::Form(mut form) => {
                match key.code {
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        form.fields[form.selected].editor = Editor::default();
                    }
                    KeyCode::Enter => {
                        self.submit(form);
                        return;
                    }
                    KeyCode::Tab | KeyCode::Down => {
                        form.selected = (form.selected + 1) % form.fields.len()
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        form.selected = (form.selected + form.fields.len() - 1) % form.fields.len()
                    }
                    KeyCode::Char('p')
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && matches!(form.kind, FormKind::Character(_)) =>
                    {
                        let i = PALETTE
                            .iter()
                            .position(|c| *c == form.fields[2].editor.text)
                            .map(|i| i + 1)
                            .unwrap_or(0)
                            % PALETTE.len();
                        form.fields[2].editor = Editor::new(PALETTE[i].into());
                    }
                    _ => form.fields[form.selected].editor.key(key, 60, false),
                }
                self.modal = Some(Modal::Form(form));
            }
            Modal::Confirm(confirm) => match (&confirm, key.code) {
                (Confirm::Quit, KeyCode::Char('s')) => {
                    if self.save() {
                        self.quit = true;
                    }
                }
                (Confirm::Quit, KeyCode::Char('d')) => self.quit = true,
                (Confirm::DeleteBlock, KeyCode::Char('y')) => {
                    self.checkpoint();
                    self.document.story.blocks.remove(self.selected);
                    if self.document.story.blocks.is_empty() {
                        self.document.story.blocks.push(StoryBlock::default());
                    }
                    self.sync_editor();
                    self.message("Block deleted · u to undo");
                }
                (Confirm::DeleteCharacter(id), KeyCode::Char('y')) => {
                    self.checkpoint();
                    self.document.story.characters.retain(|c| c.id != *id);
                    for b in &mut self.document.story.blocks {
                        if b.character == Some(*id) {
                            b.character = None;
                        }
                    }
                    self.cast_selected = self
                        .cast_selected
                        .min(self.document.story.characters.len().saturating_sub(1));
                    self.modal = Some(Modal::Cast);
                    self.message("Character removed; their text is preserved · u to undo");
                }
                (Confirm::Overwrite(path), KeyCode::Char('y')) => {
                    self.export_to(path.clone(), true)
                }
                (_, KeyCode::Char('n')) => {}
                _ => self.modal = Some(Modal::Confirm(confirm)),
            },
        }
    }
}

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste);
        ratatui::restore();
    }
}

pub fn run(document: Document) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let _restore = Restore;
    execute!(io::stdout(), EnableBracketedPaste)?;
    let mut app = App::new(document);
    while !app.quit {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        match event::read()? {
            Event::Key(key) => app.key(key),
            Event::Paste(text) => app.paste(&text),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        App::new(Document::new("unused.json".into(), Story::demo()))
    }
    fn key(app: &mut App, code: KeyCode) {
        app.key(code.into());
    }

    #[test]
    fn edit_add_assign_reorder_and_undo() {
        let mut a = app();
        key(&mut a, KeyCode::Char('n'));
        key(&mut a, KeyCode::Char('2'));
        a.paste("Hello, **world**!\nA second line.");
        key(&mut a, KeyCode::Tab);
        assert_eq!(a.block().character, Some(1));
        key(&mut a, KeyCode::Esc);
        key(&mut a, KeyCode::Char('J'));
        assert_eq!(a.selected, 2);
        key(&mut a, KeyCode::Char('u'));
        assert_eq!(
            a.document.story.blocks[1].text,
            "Hello, **world**!\nA second line."
        );
        assert_eq!(a.document.story.blocks[1].character, Some(1));
    }

    #[test]
    fn quit_requires_explicit_choice_and_delete_can_be_undone() {
        let mut a = app();
        key(&mut a, KeyCode::Char('q'));
        assert!(!a.quit);
        key(&mut a, KeyCode::Esc);
        key(&mut a, KeyCode::Char('d'));
        key(&mut a, KeyCode::Char('y'));
        assert_eq!(a.document.story.blocks.len(), 11);
        key(&mut a, KeyCode::Char('u'));
        assert_eq!(a.document.story.blocks.len(), 12);
    }
}
