//! The versioned binary manuscript format. See FORMAT.md for its wire layout.
use crate::story::{Character, Kind, Story, StoryBlock};

const MAGIC: &[u8; 8] = b"PLAYRITE";

pub fn decode(bytes: &[u8]) -> Result<Story, String> {
    // Legacy manuscripts remain readable regardless of their filename.
    if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
        let story: Story = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        story.validate()?;
        return Ok(story);
    }
    let mut reader = Reader { remaining: bytes };
    if reader.take(MAGIC.len())? != MAGIC {
        return Err("Not a Playrite manuscript (missing PLAYRITE signature)".into());
    }
    let version = reader.u32()?;
    if version != 1 {
        return Err(format!("Unsupported story version: {version}"));
    }
    let title = reader.string()?;
    let subtitle = reader.string()?;
    let author = reader.string()?;
    // Do not allocate based on untrusted counts or lengths. Each record must
    // consume bytes successfully before it can be added to a collection.
    let mut characters = Vec::new();
    for _ in 0..reader.u32()? {
        characters.push(Character {
            id: reader.u64()?,
            name: reader.string()?,
            description: reader.string()?,
            color: reader.string()?,
        });
    }
    let mut blocks = Vec::new();
    for _ in 0..reader.u32()? {
        let kind = match reader.byte()? {
            0 => Kind::Narration,
            1 => Kind::Dialogue,
            2 => Kind::Action,
            3 => Kind::Thought,
            4 => Kind::Quote,
            5 => Kind::Scene,
            6 => Kind::Divider,
            7 => Kind::Note,
            tag => return Err(format!("Unknown block type: {tag}")),
        };
        let character = match reader.byte()? {
            0 => None,
            1 => Some(reader.u64()?),
            tag => return Err(format!("Invalid character reference tag: {tag}")),
        };
        blocks.push(StoryBlock {
            kind,
            character,
            text: reader.string()?,
        });
    }
    if !reader.remaining.is_empty() {
        return Err("Unexpected trailing data in Playrite manuscript".into());
    }
    let story = Story {
        version,
        title,
        subtitle,
        author,
        characters,
        blocks,
    };
    story.validate()?;
    Ok(story)
}

pub fn encode(story: &Story) -> Result<Vec<u8>, String> {
    story.validate()?;
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&story.version.to_le_bytes());
    string(&mut bytes, &story.title)?;
    string(&mut bytes, &story.subtitle)?;
    string(&mut bytes, &story.author)?;
    length(&mut bytes, story.characters.len())?;
    for character in &story.characters {
        bytes.extend_from_slice(&character.id.to_le_bytes());
        string(&mut bytes, &character.name)?;
        string(&mut bytes, &character.description)?;
        string(&mut bytes, &character.color)?;
    }
    length(&mut bytes, story.blocks.len())?;
    for block in &story.blocks {
        bytes.push(match block.kind {
            Kind::Narration => 0,
            Kind::Dialogue => 1,
            Kind::Action => 2,
            Kind::Thought => 3,
            Kind::Quote => 4,
            Kind::Scene => 5,
            Kind::Divider => 6,
            Kind::Note => 7,
        });
        if let Some(id) = block.character {
            bytes.push(1);
            bytes.extend_from_slice(&id.to_le_bytes());
        } else {
            bytes.push(0);
        }
        string(&mut bytes, &block.text)?;
    }
    Ok(bytes)
}

fn length(bytes: &mut Vec<u8>, value: usize) -> Result<(), String> {
    let value = u32::try_from(value).map_err(|_| "Manuscript exceeds the format's size limits")?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn string(bytes: &mut Vec<u8>, value: &str) -> Result<(), String> {
    length(bytes, value.len())?;
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let (value, rest) = self
            .remaining
            .split_at_checked(length)
            .ok_or("Truncated Playrite manuscript")?;
        self.remaining = rest;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn string(&mut self) -> Result<String, String> {
        let length = self.u32()? as usize;
        std::str::from_utf8(self.take(length)?)
            .map(str::to_owned)
            .map_err(|_| "Invalid UTF-8 in Playrite manuscript".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_wire_layout_is_stable() {
        let story = Story {
            title: String::new(),
            ..Story::default()
        };
        // Signature, version, three empty strings, no characters, one block,
        // narration tag, no character, and empty text (all integers LE).
        let bytes = b"PLAYRITE\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0\0\0\0\0\0";
        assert_eq!(encode(&story).unwrap(), bytes);
        assert_eq!(decode(bytes).unwrap(), story);
    }

    #[test]
    fn preserves_all_fields_and_block_types() {
        let text = "🕯 café 水\0\r\n\t\\ **words**\n\n";
        let mut story = Story::demo();
        story.title = text.into();
        story.subtitle = text.into();
        story.author = text.into();
        story.characters[0].id = 0;
        story.characters[0].name = text.into();
        story.characters[0].description = text.into();
        story.characters[0].color = "#AaBb09".into();
        story.characters[1].id = u64::MAX;
        story.blocks = Kind::ALL
            .into_iter()
            .flat_map(|kind| {
                [None, Some(0), Some(u64::MAX)].map(|character| StoryBlock {
                    kind,
                    character,
                    text: text.into(),
                })
            })
            .collect();
        assert_eq!(decode(&encode(&story).unwrap()).unwrap(), story);
    }

    #[test]
    fn sample_matches_legacy_and_binary_is_smaller() {
        let legacy = include_bytes!("../tests/fixtures/legacy.playrite.json");
        let binary = include_bytes!("../demo.playrite");
        assert_eq!(decode(legacy).unwrap(), Story::demo());
        assert_eq!(decode(binary).unwrap(), Story::demo());
        assert_eq!(encode(&Story::demo()).unwrap(), binary);
        assert!(binary.len() < legacy.len());
    }

    #[test]
    fn rejects_every_truncation_and_trailing_data() {
        let mut bytes = encode(&Story::demo()).unwrap();
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end]).is_err(), "accepted prefix {end}");
        }
        bytes.push(0);
        assert!(decode(&bytes).unwrap_err().contains("trailing"));
    }

    #[test]
    fn rejects_invalid_headers_lengths_counts_tags_and_utf8() {
        let bytes = encode(&Story {
            title: String::new(),
            ..Story::default()
        })
        .unwrap();
        for (offset, value, message) in [
            (0, 0, "signature"),
            (8, 2, "version"),
            (12, 255, "Truncated"),
            (24, 255, "Truncated"),
            (28, 255, "Truncated"),
            (32, 8, "block type"),
            (33, 2, "reference tag"),
            (34, 255, "Truncated"),
        ] {
            let mut invalid = bytes.clone();
            invalid[offset] = value;
            assert!(decode(&invalid).unwrap_err().contains(message));
        }
        let mut invalid = bytes;
        invalid[34..38].copy_from_slice(&1u32.to_le_bytes());
        invalid.push(255);
        assert!(decode(&invalid).unwrap_err().contains("UTF-8"));
        invalid[34..38].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(&invalid).unwrap_err().contains("Truncated"));
    }

    #[test]
    fn validates_binary_character_references_and_empty_stories() {
        let mut story = Story::demo();
        story.blocks[0].character = Some(999);
        assert!(encode(&story).is_err());
        let mut bytes = encode(&Story::demo()).unwrap();
        // The first character ID follows the header, metadata and count.
        let id_offset = 12 + 12 + story.title.len() + story.subtitle.len() + story.author.len() + 4;
        bytes[id_offset..id_offset + 8].copy_from_slice(&999u64.to_le_bytes());
        assert!(decode(&bytes).unwrap_err().contains("missing character"));
        let mut bytes = encode(&Story {
            title: String::new(),
            ..Story::default()
        })
        .unwrap();
        bytes.truncate(32);
        bytes[28..32].fill(0);
        assert!(decode(&bytes).unwrap_err().contains("at least one block"));
    }
}
