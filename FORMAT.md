# Playrite binary format, version 1

Manuscripts use the `.playrite` extension. The format stores story metadata,
characters, and ordered blocks directly, without JSON keys or text escaping.
It is uncompressed and unencrypted. Private notes are part of the manuscript;
HTML export omits them.

All integers are unsigned and little-endian. There is no alignment or padding.
A **string** is a `u32` byte length followed by exactly that many UTF-8 bytes.
Empty strings have length zero. Text is preserved verbatim, including line
endings, whitespace, and embedded NULs; lengths count bytes, not characters.

## File layout

| Field | Encoding |
| --- | --- |
| Signature | 8 ASCII bytes: `PLAYRITE` |
| Version | `u32`, currently `1` |
| Title | string |
| Subtitle | string |
| Author | string |
| Character count | `u32` |
| Characters | That many character records, in display order |
| Block count | `u32`, at least `1` |
| Blocks | That many block records, in manuscript order |

## Character record

| Field | Encoding |
| --- | --- |
| ID | `u64`, unique within the story; zero is valid |
| Name | string, must contain a non-whitespace character |
| Description | string |
| Color | string, `#RRGGBB` (hex digits may be upper or lower case) |

## Block record

| Field | Encoding |
| --- | --- |
| Kind | `u8`, from the table below |
| Has character | `u8`: `0` for none, `1` for an ID |
| Character ID | `u64`, present only when Has character is `1` |
| Text | string |

| Kind value | Meaning |
| --- | --- |
| 0 | Narration |
| 1 | Dialogue |
| 2 | Action |
| 3 | Thought |
| 4 | Quotation |
| 5 | Scene heading |
| 6 | Scene break |
| 7 | Private note |

A character reference must match a character record. Every block kind may
retain both text and a character assignment, even when the reader does not
display them. These numeric values are part of the format contract and must
not be changed by reordering Rust enums.

## Reading and compatibility

Readers reject unsupported versions, unknown tags, invalid UTF-8, truncated
fields, trailing bytes, and invalid story data. Counts and lengths do not
cause allocations before the corresponding data has been read. Each string
and each record count is limited to `u32::MAX`; practical limits depend on
available memory. There is no checksum, so structurally valid byte changes
are not detected as corruption.

Legacy version-1 JSON manuscripts are detected by an opening `{` after optional
ASCII whitespace and remain readable, independent of filename. All saves write
the binary format. Saving an opened JSON manuscript with Ctrl+S replaces its
contents with binary data at the same path. To keep the JSON original, use F4
and choose a new `.playrite` filename. Export only reads the source manuscript.

Future incompatible layouts require a new version. Version 1 has no extension
fields; readers must not silently ignore extra data.
