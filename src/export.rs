use crate::story::{self, Kind, Story};
use std::{fmt::Write, path::Path};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Format {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
}

#[derive(Debug)]
pub struct Run {
    pub text: String,
    pub format: Format,
}

/// A deliberately small inline language. Raw HTML and URLs are always text.
pub fn inline_runs(text: &str) -> Vec<Run> {
    fn parse(text: &str, format: Format, depth: usize, out: &mut Vec<Run>) {
        let mut rest = text;
        let mut plain = String::new();
        while !rest.is_empty() {
            if let Some(escaped) = rest.strip_prefix('\\')
                && let Some(c) = escaped.chars().next().filter(|c| "*~`\\".contains(*c))
            {
                plain.push(c);
                rest = &escaped[c.len_utf8()..];
                continue;
            }
            let token = if rest.starts_with("**") {
                "**"
            } else if rest.starts_with('*') {
                "*"
            } else if rest.starts_with("~~") {
                "~~"
            } else if rest.starts_with('`') {
                "`"
            } else {
                ""
            };
            if !token.is_empty() && depth < 16 {
                let tail = &rest[token.len()..];
                let closing = tail.match_indices(token).find(|(i, _)| {
                    *i > 0 && tail[..*i].chars().rev().take_while(|c| *c == '\\').count() % 2 == 0
                });
                if let Some((end, _)) = closing {
                    if !plain.is_empty() {
                        out.push(Run {
                            text: std::mem::take(&mut plain),
                            format,
                        });
                    }
                    let mut nested = format;
                    match token {
                        "**" => nested.bold = true,
                        "*" => nested.italic = true,
                        "~~" => nested.strike = true,
                        "`" => nested.code = true,
                        _ => {}
                    }
                    if nested.code {
                        out.push(Run {
                            text: tail[..end].into(),
                            format: nested,
                        });
                    } else {
                        parse(&tail[..end], nested, depth + 1, out);
                    }
                    rest = &tail[end + token.len()..];
                    continue;
                }
            }
            let c = rest.chars().next().unwrap();
            plain.push(c);
            rest = &rest[c.len_utf8()..];
        }
        if !plain.is_empty() {
            out.push(Run {
                text: plain,
                format,
            });
        }
    }
    let mut runs = Vec::new();
    parse(text, Format::default(), 0, &mut runs);
    runs
}

pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn inline(text: &str) -> String {
    let mut html = String::new();
    for run in inline_runs(text) {
        let f = run.format;
        if f.bold {
            html.push_str("<strong>");
        }
        if f.italic {
            html.push_str("<em>");
        }
        if f.strike {
            html.push_str("<s>");
        }
        if f.code {
            html.push_str("<code>");
        }
        html.push_str(&escape(&run.text).replace('\n', "<br>\n"));
        if f.code {
            html.push_str("</code>");
        }
        if f.strike {
            html.push_str("</s>");
        }
        if f.italic {
            html.push_str("</em>");
        }
        if f.bold {
            html.push_str("</strong>");
        }
    }
    html
}

fn paragraphs(text: &str) -> String {
    text.split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .map(|p| format!("<p>{}</p>\n", inline(p)))
        .collect()
}

pub fn render(story: &Story) -> String {
    let mut html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"color-scheme\" content=\"light dark\">\n<meta name=\"generator\" content=\"Playrite\">\n<title>{}</title>\n<style>\n{}\n</style>\n</head>\n<body>\n<a class=\"skip\" href=\"#story\">Skip to story</a>\n<main>\n<header class=\"cover\">\n<h1>{}</h1>\n",
        escape(&story.title),
        include_str!("reader.css"),
        escape(&story.title)
    );
    if !story.subtitle.trim().is_empty() {
        let _ = writeln!(
            html,
            "<p class=\"subtitle\">{}</p>",
            escape(&story.subtitle)
        );
    }
    if !story.author.trim().is_empty() {
        let _ = writeln!(html, "<p class=\"byline\">By {}</p>", escape(&story.author));
    }
    let _ = writeln!(
        html,
        "<p class=\"reading-time\">{} words <span aria-hidden=\"true\">·</span> {} min read</p>\n</header>",
        story.word_count(),
        story.word_count().div_ceil(220).max(1)
    );
    if !story.characters.is_empty() {
        html.push_str("<details class=\"cast\"><summary>The characters</summary><dl>\n");
        for c in &story.characters {
            let _ = writeln!(
                html,
                "<div class=\"cast-member\" style=\"--character: {}\"><dt>{}</dt><dd>{}</dd></div>",
                escape(&c.color),
                escape(&c.name),
                escape(&c.description)
            );
        }
        html.push_str("</dl></details>\n");
    }
    let scenes: Vec<_> = story
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == Kind::Scene && !b.text.trim().is_empty())
        .collect();
    if scenes.len() > 1 {
        html.push_str("<nav class=\"contents\" aria-label=\"Scenes\"><span class=\"eyebrow\">CONTENTS</span><ol>\n");
        for (i, b) in scenes {
            let _ = writeln!(
                html,
                "<li><a href=\"#scene-{i}\">{}</a></li>",
                inline(&b.text)
            );
        }
        html.push_str("</ol></nav>\n");
    }
    html.push_str("<article id=\"story\" aria-label=\"Story\">\n");
    for (i, b) in story.blocks.iter().enumerate() {
        if b.kind == Kind::Note || (b.kind != Kind::Divider && b.text.trim().is_empty()) {
            continue;
        }
        let character = story.character(b.character);
        let color = character.map(|c| c.color.as_str()).unwrap_or("#716355");
        let label = character
            .map(|c| format!("<span class=\"speaker\">{}</span>", escape(&c.name)))
            .unwrap_or_default();
        let body = paragraphs(&b.text);
        match b.kind {
            Kind::Scene => {
                let _ = writeln!(html, "<h2 id=\"scene-{i}\">{}</h2>", inline(&b.text));
            }
            Kind::Narration => {
                let _ = writeln!(html, "<div class=\"narration\">{body}</div>");
            }
            Kind::Dialogue => {
                let _ = writeln!(
                    html,
                    "<div class=\"dialogue\" style=\"--character: {color}\">{label}<div class=\"speech\">{body}</div></div>"
                );
            }
            Kind::Action => {
                let _ = writeln!(
                    html,
                    "<div class=\"action\" style=\"--character: {color}\">{label}{body}</div>"
                );
            }
            Kind::Thought => {
                let _ = writeln!(
                    html,
                    "<div class=\"thought\" style=\"--character: {color}\">{label}<span class=\"kind-label\">thinking</span>{body}</div>"
                );
            }
            Kind::Quote => {
                let _ = writeln!(
                    html,
                    "<figure class=\"quotation\"><blockquote>{body}</blockquote>{}</figure>",
                    character
                        .map(|c| format!("<figcaption>— {}</figcaption>", escape(&c.name)))
                        .unwrap_or_default()
                );
            }
            Kind::Divider => {
                html.push_str("<hr class=\"scene-break\" aria-label=\"Scene break\">\n")
            }
            Kind::Note => {}
        }
    }
    html.push_str("</article>\n<footer><span aria-hidden=\"true\">⁂</span><p>The end</p></footer>\n</main>\n</body>\n</html>\n");
    html
}

pub fn write(story: &Story, path: &Path, overwrite: bool) -> Result<(), String> {
    story.validate()?;
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
    {
        return Err("Choose an .html or .htm output filename".into());
    }
    story::atomic_write(path, render(story).as_bytes(), overwrite)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_formatting_is_safe_and_handles_unicode() {
        assert_eq!(
            inline("**Bold** and *café* ~~gone~~ `x<y`"),
            "<strong>Bold</strong> and <em>café</em> <s>gone</s> <code>x&lt;y</code>"
        );
        assert_eq!(
            inline("\\*literal\\* <script>alert('x')</script>"),
            "*literal* &lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;"
        );
        assert_eq!(
            inline("**bold with *emphasis* inside**"),
            "<strong>bold with </strong><strong><em>emphasis</em></strong><strong> inside</strong>"
        );
        assert_eq!(inline("unfinished *italic"), "unfinished *italic");
    }

    #[test]
    fn complete_export_has_no_private_notes_or_external_assets() {
        let mut story = Story::demo();
        story.title = "<script>bad()</script>".into();
        let html = render(&story);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("&lt;script&gt;bad()&lt;/script&gt;"));
        assert!(html.contains("href=\"#scene-8\""));
        assert!(html.contains("<strong>amber light</strong>"));
        assert!(!html.contains("What is inside the letter?"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("https://"));
    }

    #[test]
    fn export_requires_explicit_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("story.html");
        write(&Story::demo(), &path, false).unwrap();
        assert!(write(&Story::default(), &path, false).is_err());
        write(&Story::default(), &path, true).unwrap();
        assert!(write(&Story::default(), &dir.path().join("story.json"), true).is_err());
    }
}
