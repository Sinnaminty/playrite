use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const PALETTE: [&str; 8] = [
    "#9b513c", "#386d73", "#705589", "#627440", "#996622", "#a34f70", "#456da1", "#716355",
];

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Narration,
    Dialogue,
    Action,
    Thought,
    Quote,
    Scene,
    Divider,
    Note,
}

impl Kind {
    pub const ALL: [Self; 8] = [
        Self::Narration,
        Self::Dialogue,
        Self::Action,
        Self::Thought,
        Self::Quote,
        Self::Scene,
        Self::Divider,
        Self::Note,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Narration => "Narration",
            Self::Dialogue => "Dialogue",
            Self::Action => "Action",
            Self::Thought => "Thought",
            Self::Quote => "Quotation",
            Self::Scene => "Scene",
            Self::Divider => "Scene break",
            Self::Note => "Private note",
        }
    }

    pub fn marker(self) -> &'static str {
        match self {
            Self::Narration => "¶",
            Self::Dialogue => "“",
            Self::Action => "↗",
            Self::Thought => "~",
            Self::Quote => "❝",
            Self::Scene => "§",
            Self::Divider => "⁂",
            Self::Note => "·",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Character {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub color: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct StoryBlock {
    pub kind: Kind,
    pub character: Option<u64>,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Story {
    pub version: u32,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    pub characters: Vec<Character>,
    pub blocks: Vec<StoryBlock>,
}

impl Default for Story {
    fn default() -> Self {
        Self {
            version: 1,
            title: "Untitled story".into(),
            subtitle: String::new(),
            author: String::new(),
            characters: vec![],
            blocks: vec![StoryBlock::default()],
        }
    }
}

impl Story {
    pub fn character(&self, id: Option<u64>) -> Option<&Character> {
        self.characters.iter().find(|c| Some(c.id) == id)
    }

    pub fn word_count(&self) -> usize {
        self.blocks
            .iter()
            .filter(|b| b.kind != Kind::Note && b.kind != Kind::Divider)
            .map(|b| b.text.split_whitespace().count())
            .sum()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("Unsupported story version: {}", self.version));
        }
        if self.blocks.is_empty() {
            return Err("A story must contain at least one block".into());
        }
        let mut ids = HashSet::new();
        for c in &self.characters {
            if !ids.insert(c.id) {
                return Err(format!("Duplicate character ID: {}", c.id));
            }
            if c.name.trim().is_empty() {
                return Err("Character names cannot be empty".into());
            }
            if !valid_color(&c.color) {
                return Err(format!(
                    "Invalid character color: {} (expected #RRGGBB)",
                    c.color
                ));
            }
        }
        for b in &self.blocks {
            if b.character.is_some_and(|id| !ids.contains(&id)) {
                return Err("A block references a missing character".into());
            }
        }
        Ok(())
    }

    pub fn demo() -> Self {
        let mut story = Self {
            title: "The last light".into(),
            subtitle: "A small story about finding the way home".into(),
            author: "Your name".into(),
            characters: vec![
                Character {
                    id: 1,
                    name: "Mara".into(),
                    description: "The lighthouse keeper. Collects things the sea returns.".into(),
                    color: PALETTE[0].into(),
                },
                Character {
                    id: 2,
                    name: "Ellis".into(),
                    description: "A traveler with a letter he has never opened.".into(),
                    color: PALETTE[1].into(),
                },
            ],
            ..Self::default()
        };
        story.blocks = vec![
            StoryBlock { kind: Kind::Scene, character: None, text: "I. The shore".into() },
            StoryBlock { text: "By the time the stranger reached the lighthouse, the tide had erased his footprints. Above him, a single window held the last **amber light** of evening.".into(), ..StoryBlock::default() },
            StoryBlock { kind: Kind::Action, character: Some(1), text: "Sets two cups on the windowsill, as though she had been expecting someone.".into() },
            StoryBlock { kind: Kind::Dialogue, character: Some(1), text: "You took the long way.".into() },
            StoryBlock { kind: Kind::Dialogue, character: Some(2), text: "I wasn't sure there was a short one.".into() },
            StoryBlock { kind: Kind::Thought, character: Some(2), text: "Tell her about the letter. *Before you lose your nerve.*".into() },
            StoryBlock { kind: Kind::Quote, character: None, text: "Some lights are kept for ships. Others are kept for the people who never learned to leave.".into() },
            StoryBlock { kind: Kind::Divider, ..StoryBlock::default() },
            StoryBlock { kind: Kind::Scene, character: None, text: "II. What the sea returns".into() },
            StoryBlock { text: "Mara turned the envelope over. The paper was soft at the edges, its creases worn nearly through. Outside, the lamp began its patient circle.".into(), ..StoryBlock::default() },
            StoryBlock { kind: Kind::Dialogue, character: Some(1), text: "Well. You're here now.".into() },
            StoryBlock { kind: Kind::Note, character: None, text: "What is inside the letter? Leave space for the reader. This private note will not appear in the HTML export.".into() },
        ];
        story
    }
}

pub fn valid_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|c| c.is_ascii_hexdigit())
}

pub struct Document {
    pub story: Story,
    pub path: PathBuf,
    saved: Option<Vec<u8>>,
}

impl Document {
    pub fn new(path: PathBuf, story: Story) -> Self {
        Self {
            story,
            path,
            saved: None,
        }
    }

    pub fn open(path: PathBuf) -> Result<Self, String> {
        match fs::read(&path) {
            Ok(bytes) => {
                let story: Story = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
                story.validate()?;
                Ok(Self {
                    story,
                    path,
                    saved: Some(bytes),
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::new(path, Story::default()))
            }
            Err(e) => Err(format!("Cannot open {}: {e}", path.display())),
        }
    }

    pub fn dirty(&self) -> bool {
        self.saved.as_ref().is_none_or(|bytes| {
            serde_json::from_slice::<Story>(bytes).ok().as_ref() != Some(&self.story)
        })
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.story.validate()?;
        match (fs::read(&self.path), &self.saved) {
            (Ok(current), Some(saved)) if current == *saved => {},
            (Err(e), None) if e.kind() == std::io::ErrorKind::NotFound => {},
            (Err(e), _) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => return Err("Story changed on disk; save refused to protect those changes. Your edits remain in the editor. Use F4 to save a copy.".into()),
        }
        let bytes = serde_json::to_vec_pretty(&self.story).map_err(|e| e.to_string())?;
        atomic_write(&self.path, &bytes, self.saved.is_some())?;
        self.saved = Some(bytes);
        Ok(())
    }

    pub fn save_copy(&mut self, path: PathBuf) -> Result<(), String> {
        self.story.validate()?;
        let bytes = serde_json::to_vec_pretty(&self.story).map_err(|e| e.to_string())?;
        atomic_write(&path, &bytes, false)?;
        self.path = path;
        self.saved = Some(bytes);
        Ok(())
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8], overwrite: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    file.write_all(bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    if overwrite {
        file.persist(path)
    } else {
        file.persist_noclobber(path)
    }
    .map_err(|e| format!("Cannot write {}: {}", path.display(), e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_external_edit_protection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("story.json");
        let mut doc = Document::new(path.clone(), Story::demo());
        assert!(doc.dirty());
        doc.save().unwrap();
        assert!(!doc.dirty());
        assert_eq!(Document::open(path.clone()).unwrap().story, doc.story);
        fs::write(&path, b"external edit").unwrap();
        doc.story.title = "Changed".into();
        assert!(doc.save().is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "external edit");
        doc.save_copy(dir.path().join("copy.json")).unwrap();
        assert!(!doc.dirty());
    }

    #[test]
    fn reject_bad_references_and_colors() {
        let mut story = Story::demo();
        story.blocks[0].character = Some(999);
        assert!(story.validate().is_err());
        assert!(!valid_color("#000;xx"));
        assert!(!valid_color("éaaaaa"));
        assert!(valid_color("#aAbB09"));
    }
}
