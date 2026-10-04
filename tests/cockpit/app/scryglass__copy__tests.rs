use super::*;
use crate::ui::scryglass::{StageOverlay, StageRoute};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("angel-stage-copy-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn resource(path: &Path) -> Media {
    Media::Resource {
        label: "report".into(),
        url: path.to_str().unwrap().into(),
    }
}

fn text_ready(stage: &mut Scryglass, cards: &[Media]) -> StageCopyPayload {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match stage.copy_payload(cards, StageCopyTarget::Text) {
            Ok(payload) => return payload,
            Err(error) if error.contains("loading") && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("copy failed: {error}"),
        }
    }
}

#[test]
fn stage_copy_location_preserves_absolute_image_video_and_file_uri_paths() {
    let base = root("location");
    let path = base.join("shape with spaces.png");
    let expected = path.to_str().unwrap();
    // Location copying does not decode or read files, including missing files.
    let cards = vec![
        Media::Image {
            label: "image".into(),
            path: expected.into(),
        },
        Media::Video {
            label: "video".into(),
            path: expected.into(),
        },
        Media::Resource {
            label: "file URI".into(),
            url: format!("file://{expected}"),
        },
        Media::Confined {
            card: Box::new(resource(&path)),
            root: base.clone(),
        },
    ];
    let mut stage = Scryglass::default();
    for index in 0..cards.len() {
        stage.reveal_media(index, true);
        let payload = stage
            .copy_payload(&cards, StageCopyTarget::Location)
            .unwrap();
        assert_eq!(payload.text, expected);
        assert_eq!(payload.fallback_file, "stage-location.txt");
    }
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn stage_copy_remote_url_is_exact_and_text_never_fetches_a_page() {
    let url = "https://example.invalid/view?shape=triangles&phase=2#tile";
    let cards = vec![Media::Link {
        label: "remote".into(),
        url: url.into(),
    }];
    let mut stage = Scryglass::default();
    stage.reveal_media(0, true);
    assert_eq!(
        stage
            .copy_payload(&cards, StageCopyTarget::Location)
            .unwrap()
            .text,
        url
    );
    assert!(
        stage
            .copy_payload(&cards, StageCopyTarget::Text)
            .err()
            .unwrap()
            .contains("not loaded")
    );
}

#[test]
fn stage_copy_requires_current_media_overlay_not_a_stale_reveal_or_selection() {
    let cards = vec![Media::Image {
        label: "image".into(),
        path: "/tmp/shape.png".into(),
    }];
    let mut stage = Scryglass::default();
    assert!(
        stage
            .copy_payload(&cards, StageCopyTarget::Location)
            .is_err()
    );
    stage.reveal_media(0, true);
    stage.controller.clear_overlay();
    assert!(stage.controller.show_overlay(StageOverlay::Lesson));
    // The reveal may remain active behind a lesson. Never copy it as visible work.
    assert_eq!(stage.active_media(), Some(0));
    assert!(
        stage
            .copy_payload(&cards, StageCopyTarget::Location)
            .is_err()
    );
    stage.controller.clear_overlay();
    stage.controller.navigate(StageRoute::Workshop);
    assert!(
        stage
            .copy_payload(&cards, StageCopyTarget::Location)
            .is_err()
    );
    stage
        .controller
        .show_overlay(StageOverlay::Media { index: 4 });
    assert!(
        stage
            .copy_payload(&cards, StageCopyTarget::Location)
            .err()
            .unwrap()
            .contains("unavailable")
    );
}

#[test]
fn stage_copy_rejects_relative_path_and_visual_text_without_guessing() {
    let mut stage = Scryglass::default();
    let cards = vec![Media::Image {
        label: "image".into(),
        path: "shape.png".into(),
    }];
    stage.reveal_media(0, true);
    assert!(
        stage
            .copy_payload(&cards, StageCopyTarget::Location)
            .is_err()
    );
    assert!(
        stage
            .copy_payload(&cards, StageCopyTarget::Text)
            .err()
            .unwrap()
            .contains("no document text")
    );
}

#[test]
fn stage_copy_text_is_the_sanitized_loaded_snapshot_and_switches_cards_safely() {
    let base = root("snapshot");
    let a = base.join("a.md");
    let b = base.join("b.md");
    std::fs::write(&a, "# Exact result\nmeasured: 17\n\x1b[2J").unwrap();
    std::fs::write(&b, "# A different result\nmeasured: 23").unwrap();
    let cards = vec![resource(&a), resource(&b)];
    let mut stage = Scryglass::default();
    stage.reveal_media(0, true);
    let first = text_ready(&mut stage, &cards);
    assert!(first.text.contains("measured: 17"));
    assert!(!first.text.contains('\x1b'));
    assert!(first.description.contains("SHA256"));
    assert_eq!(first.fallback_file, "stage-document.txt");
    // Copy what was loaded, not a later changed file masquerading as the preview.
    std::fs::write(&a, "changed after display").unwrap();
    assert_eq!(text_ready(&mut stage, &cards).text, first.text);
    stage.reveal_media(1, true);
    let second = text_ready(&mut stage, &cards);
    assert!(second.text.contains("measured: 23"));
    assert!(!second.text.contains("measured: 17"));
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn stage_copy_text_reports_truncation_and_empty_files_honestly() {
    let base = root("bounds");
    let long = base.join("long.txt");
    let empty = base.join("empty.txt");
    std::fs::write(&long, "x".repeat(300_000)).unwrap();
    std::fs::write(&empty, "").unwrap();
    let cards = vec![resource(&long), resource(&empty)];
    let mut stage = Scryglass::default();
    stage.reveal_media(0, true);
    let payload = text_ready(&mut stage, &cards);
    assert!(payload.description.contains("TRUNCATED"));
    assert!(payload.text.contains("Preview truncated at 256 KiB"));
    stage.reveal_media(1, true);
    assert_eq!(
        text_ready(&mut stage, &cards).text,
        "[The requested file is empty.]"
    );
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn stage_copy_loading_and_missing_documents_are_explicit_errors() {
    let base = root("missing");
    let cards = vec![resource(&base.join("missing.md"))];
    let mut stage = Scryglass::default();
    stage.reveal_media(0, true);
    let first = stage
        .copy_payload(&cards, StageCopyTarget::Text)
        .err()
        .unwrap();
    assert!(first.contains("loading"));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let error = stage
            .copy_payload(&cards, StageCopyTarget::Text)
            .err()
            .unwrap();
        if !error.contains("loading") {
            assert!(error.contains("Cannot read requested file"));
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    std::fs::remove_dir_all(base).unwrap();
}

#[cfg(unix)]
#[test]
fn stage_copy_text_keeps_confined_symlink_denial() {
    let base = root("confined");
    let inside = base.join("workspace");
    std::fs::create_dir_all(&inside).unwrap();
    let outside = base.join("outside.md");
    std::fs::write(&outside, "must not copy").unwrap();
    let alias = inside.join("alias.md");
    std::os::unix::fs::symlink(&outside, &alias).unwrap();
    let cards = vec![Media::Confined {
        card: Box::new(resource(&alias)),
        root: inside,
    }];
    let mut stage = Scryglass::default();
    stage.reveal_media(0, true);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let error = stage
            .copy_payload(&cards, StageCopyTarget::Text)
            .err()
            .unwrap();
        if !error.contains("loading") {
            assert!(error.contains("symlinks") || error.contains("Cannot read"));
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    std::fs::remove_dir_all(base).unwrap();
}
