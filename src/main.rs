mod app;
mod editor;
mod export;
mod format;
mod story;
mod ui;

use std::{
    env,
    io::{self, IsTerminal},
    path::PathBuf,
    process::ExitCode,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("playrite: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--help" || s == "-h") {
        println!(
            "Playrite — a quiet place to write\n\nUSAGE\n  playrite [STORY.playrite]                  Open or start a story\n  playrite demo [STORY.playrite]             Create and open a sample story\n  playrite export STORY.playrite [-o OUT.html] [--force]\n                                        Export without opening the editor\n\nThe default story is story.playrite. Demo defaults to demo.playrite.\nExport refuses to overwrite an existing file unless --force is supplied.\nInside the editor, press ? for help. Stories are saved with Ctrl+S."
        );
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "--version" || s == "-V") {
        println!("playrite {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "export") {
        let input = args
            .get(1)
            .filter(|s| !s.starts_with('-'))
            .ok_or("Usage: playrite export STORY.playrite [-o OUT.html] [--force]")?;
        let mut output = PathBuf::from(input).with_extension("html");
        let mut force = false;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "-o" | "--output" => {
                    i += 1;
                    output = args
                        .get(i)
                        .filter(|s| !s.starts_with('-'))
                        .ok_or("Missing output path after -o")?
                        .into();
                }
                "--force" => force = true,
                other => return Err(format!("Unknown export argument: {other}")),
            }
            i += 1;
        }
        if !std::path::Path::new(input).is_file() {
            return Err(format!("Story does not exist: {input}"));
        }
        if std::fs::canonicalize(input)
            .ok()
            .zip(std::fs::canonicalize(&output).ok())
            .is_some_and(|(a, b)| a == b)
        {
            return Err("The export must use a different path from your story.".into());
        }
        let document = story::Document::open(input.into())?;
        export::write(&document.story, &output, force)?;
        println!(
            "Exported {} words to {}",
            document.story.word_count(),
            output.display()
        );
        return Ok(());
    }
    let demo = args.first().is_some_and(|s| s == "demo");
    if args.len() > if demo { 2 } else { 1 } || args.first().is_some_and(|s| s.starts_with('-')) {
        return Err("Unknown argument. Run playrite --help for usage.".into());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("The editor needs an interactive terminal. Use `playrite export` for a non-interactive export.".into());
    }
    let path = PathBuf::from(if demo {
        args.get(1).map(String::as_str).unwrap_or("demo.playrite")
    } else {
        args.first().map(String::as_str).unwrap_or("story.playrite")
    });
    let document = if demo {
        if path.exists() {
            return Err(format!(
                "{} already exists; open it with `playrite {}`",
                path.display(),
                path.display()
            ));
        }
        story::Document::new(path, story::Story::demo())
    } else {
        story::Document::open(path)?
    };
    app::run(document).map_err(|e| e.to_string())
}
