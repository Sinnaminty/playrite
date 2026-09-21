use std::{fs, process::Command};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_playrite"))
}

const STORY: &str = r##"{
  "version": 1, "title": "A <quiet> story", "subtitle": "", "author": "A writer",
  "characters": [{"id": 1, "name": "Mara", "description": "The keeper", "color": "#9b513c"}],
  "blocks": [
    {"kind": "scene", "character": null, "text": "The shore"},
    {"kind": "dialogue", "character": 1, "text": "**Hello**, world."},
    {"kind": "note", "character": null, "text": "SECRET DRAFT NOTE"}
  ]
}"##;

const BINARY_STORY: &[u8] = include_bytes!("fixtures/story.playrite");

#[test]
fn cli_export_round_trip_and_overwrite_guards() {
    check_export("story.playrite", BINARY_STORY);
    check_export("story.json", STORY.as_bytes());
}

fn check_export(filename: &str, bytes: &[u8]) {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join(filename);
    let output = dir.path().join("story.html");
    fs::write(&source, bytes).unwrap();
    let result = command().arg("export").arg(&source).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let html = fs::read_to_string(&output).unwrap();
    assert!(html.contains("A &lt;quiet&gt; story"));
    assert!(html.contains("<strong>Hello</strong>"));
    assert!(!html.contains("SECRET DRAFT NOTE"));
    assert!(
        !command()
            .arg("export")
            .arg(&source)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        command()
            .arg("export")
            .arg(&source)
            .arg("--force")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !command()
            .arg("export")
            .arg(&source)
            .arg("-o")
            .arg(&source)
            .arg("--force")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fs::read(&source).unwrap(), bytes);
}

#[test]
fn cli_rejects_missing_invalid_and_same_source_with_html_extension() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        !command()
            .arg("export")
            .arg(dir.path().join("missing.playrite"))
            .output()
            .unwrap()
            .status
            .success()
    );
    let path = dir.path().join("manuscript.html");
    fs::write(&path, BINARY_STORY).unwrap();
    assert!(
        !command()
            .arg("export")
            .arg(&path)
            .arg("--force")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fs::read(&path).unwrap(), BINARY_STORY);
    fs::write(&path, b"PLAYRITE\x01").unwrap();
    assert!(
        !command()
            .arg("export")
            .arg(&path)
            .arg("-o")
            .arg(dir.path().join("out.html"))
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(!dir.path().join("out.html").exists());
}
