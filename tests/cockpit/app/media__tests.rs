use super::*;

#[cfg(unix)]
#[test]
fn show_work_source_authority_rejects_escape_symlinks_and_quarantine() {
    use std::os::unix::fs::symlink;
    let root = std::env::temp_dir().join(format!("angel-media-authority-{}", std::process::id()));
    let workspace = root.join("work");
    std::fs::create_dir_all(&workspace).unwrap();
    let outside = root.join("outside.md");
    std::fs::write(&outside, "OUTSIDE FIXTURE").unwrap();
    std::fs::write(workspace.join("inside.md"), "ACTUAL REQUESTED BYTES").unwrap();
    assert!(model_presentation_target_in("../outside.md", &workspace).is_err());
    assert!(model_presentation_target_in(&outside.to_string_lossy(), &workspace).is_err());
    assert!(model_presentation_target_in("off-limits/fixture.md", &workspace).is_err());
    assert!(artifact_from_path_in("off-limits/fixture.md", &workspace).is_err());
    symlink(&outside, workspace.join("alias.md")).unwrap();
    let source = MediaSource {
        root: Some(workspace.clone()),
        path: workspace.join("alias.md"),
    };
    assert!(
        source.open().is_err(),
        "decoder authority must reject external aliases"
    );
    let source = MediaSource {
        root: Some(workspace.clone()),
        path: workspace.join("inside.md"),
    };
    let mut pinned_file = source.open().unwrap();
    std::fs::rename(workspace.join("inside.md"), workspace.join("moved.md")).unwrap();
    symlink(&outside, workspace.join("inside.md")).unwrap();
    let mut actual = String::new();
    pinned_file.read_to_string(&mut actual).unwrap();
    assert_eq!(
        actual, "ACTUAL REQUESTED BYTES",
        "path replacement cannot redirect an opened artifact"
    );
    assert!(
        source.open().is_err(),
        "a later request must not follow the replacement alias"
    );
    symlink(workspace.join("off-limits"), root.join("quarantine-alias")).unwrap();
    assert!(
        MediaSource::operator(root.join("quarantine-alias/fixture.md"))
            .open()
            .is_err()
    );
    for malformed in [
        "label\x1b[2J",
        "source\nreplacement",
        "identity\u{202e}spoof",
    ] {
        assert!(!presentation_text_valid(malformed, 512));
        let result = serde_json::json!({"status":"queued", "kind":"resource", "label":malformed, "url":"/tmp/x"}).to_string();
        assert!(presentation_from_result(&result).is_none());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn media_targets_resolve_for_images_and_graphs() {
    let image = Media::Image {
        label: "shot".to_string(),
        path: "/tmp/shot.png".to_string(),
    };
    assert!(image.is_image());
    assert_eq!(image.target(), "/tmp/shot.png");
    assert_eq!(image.local_path().unwrap(), PathBuf::from("/tmp/shot.png"));

    let graph = Media::Graph {
        label: "viz".to_string(),
        url: String::new(),
    };
    assert_eq!(graph.target(), format!("http://localhost:{}/", web_port()));
    assert_eq!(graph.artifact_summary(), graph.target());
    assert!(matches!(graph.artifact_summary_cow(), Cow::Borrowed(_)));

    let link = Media::Link {
        label: "site".to_string(),
        url: "https://example.com".to_string(),
    };
    assert!(link.local_path().is_none());
}

#[test]
fn byte_formatting_is_compact() {
    assert_eq!(format_bytes(32), "32 B");
    assert_eq!(format_bytes(1536), "1.5 KB");
}

#[test]
fn remote_artifact_summary_is_truncated_target() {
    let link = Media::Link {
        label: "docs".to_string(),
        url: "https://example.com/some/really/long/resource/path".to_string(),
    };
    assert_eq!(link.artifact_summary(), "https://example.com/some");
    assert!(matches!(link.artifact_summary_cow(), Cow::Borrowed(_)));
}

#[test]
fn non_ascii_remote_artifact_summary_truncates_safely() {
    let link = Media::Link {
        label: "docs".to_string(),
        url: "https://example.com/日本語/日本語/日本語/日本語".to_string(),
    };
    let summary = link.artifact_summary_cow();
    assert!(matches!(summary, Cow::Borrowed(_)));
    assert_eq!(summary.chars().count(), 24);
}

#[test]
fn local_artifact_summary_handles_absolute_and_file_urls() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/apollo-neutral.png");
    let raw = path.to_string_lossy().to_string();
    let direct = Media::Image {
        label: "direct".to_string(),
        path: raw.clone(),
    };
    let file = Media::Resource {
        label: "file".to_string(),
        url: format!("file://{raw}"),
    };

    assert!(direct.artifact_summary().contains('x'));
    assert_eq!(direct.artifact_summary(), file.artifact_summary());
}

#[test]
fn local_artifact_summary_cache_returns_borrowed_text() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/apollo-neutral.png");
    let media = Media::Image {
        label: "direct".to_string(),
        path: path.to_string_lossy().to_string(),
    };
    let first = media.artifact_summary_cow();
    let second = media.artifact_summary_cow();
    assert!(first.contains('x'));
    assert!(matches!(first, Cow::Borrowed(_)));
    assert!(matches!(second, Cow::Borrowed(_)));
    assert_eq!(first, second);
}

#[test]
fn label_sigil_and_target_cover_every_variant() {
    let img = Media::Image {
        label: "shot".into(),
        path: "/tmp/a.png".into(),
    };
    let link = Media::Link {
        label: "site".into(),
        url: "https://ex.com".into(),
    };
    let res = Media::Resource {
        label: "dl".into(),
        url: "/tmp/file.bin".into(),
    };
    let graph_url = Media::Graph {
        label: "viz".into(),
        url: "https://graph.host/g".into(),
    };
    assert_eq!((img.label(), img.sigil()), ("shot", "img"));
    assert_eq!((link.label(), link.sigil()), ("site", "link"));
    assert_eq!((res.label(), res.sigil()), ("dl", "res"));
    assert_eq!((graph_url.label(), graph_url.sigil()), ("viz", "graph"));
    // target(): Link/Resource pass the url through; a non-empty Graph keeps its url.
    assert_eq!(link.target(), "https://ex.com");
    assert_eq!(res.target(), "/tmp/file.bin");
    assert_eq!(graph_url.target(), "https://graph.host/g");
    // local_path(): a Resource with an absolute path resolves; a Link never does.
    assert_eq!(res.local_path().unwrap(), PathBuf::from("/tmp/file.bin"));
    assert!(link.local_path().is_none());
    assert!(graph_url.local_path().is_none());
    assert!(!link.is_image());
}

#[test]
fn format_bytes_scales_through_mb_and_gb() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1023), "1023 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    // Beyond a GB stays in GB (the unit index is capped).
    assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.0 GB");
}

#[test]
fn short_remote_target_is_returned_whole() {
    // <= 24 chars → the full url, borrowed (the early-return branch).
    let link = Media::Link {
        label: "x".into(),
        url: "https://a.co".into(),
    };
    let summary = link.artifact_summary_cow();
    assert_eq!(summary, "https://a.co");
    assert!(matches!(summary, Cow::Borrowed(_)));
}

#[test]
fn missing_local_artifact_summarizes_as_missing() {
    // An absolute, non-existent, non-PNG path: png fast-path misses, metadata
    // fails → the "missing" sentinel.
    let res = Media::Resource {
        label: "gone".into(),
        url: "/definitely/not/here/artifact.bin".into(),
    };
    assert_eq!(res.artifact_summary(), "missing");
}

#[test]
fn legacy_relative_local_media_path_is_inert_without_a_workspace() {
    let media = Media::Image {
        label: "missing".to_string(),
        path: "definitely-missing-terminal-artifact.png".to_string(),
    };
    assert!(media.local_path().is_none());
}
