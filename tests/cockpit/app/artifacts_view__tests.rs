use super::*;

fn flatten(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn artifact_lines_render_empty_state() {
    let lines = artifact_lines(&[], 0, 4);
    let text = flatten(&lines);
    assert!(text.contains("no delivered artifacts"));
    assert_eq!(
        lines[0].spans[0].content.as_ptr(),
        crate::ui::views::status_view::empty_artifacts_line().spans[0]
            .content
            .as_ptr()
    );

    let media = vec![Media::Link {
        label: "operator notes".to_string(),
        url: "https://example.com/notes".to_string(),
    }];
    let over_scrolled = artifact_lines(&media, 5, 4);
    assert_eq!(
        over_scrolled[0].spans[0].content.as_ptr(),
        crate::ui::views::status_view::empty_artifacts_line().spans[0]
            .content
            .as_ptr()
    );
}

#[test]
fn artifact_lines_include_index_label_and_summary() {
    let media = vec![Media::Link {
        label: "operator notes".to_string(),
        url: "https://example.com/notes".to_string(),
    }];
    let text = flatten(&artifact_lines(&media, 0, 4));
    assert!(text.contains("01"));
    assert!(text.contains("operator notes"));
    assert!(text.contains("example.com"));
    assert_eq!(artifact_lines(&media, 0, 4).capacity(), 1);
}

#[test]
fn repeated_local_artifact_rows_share_frame_summary() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets/agents/apollo-neutral.png");
    let media = vec![
        Media::Image {
            label: "apollo one".to_string(),
            path: path.to_string_lossy().to_string(),
        },
        Media::Image {
            label: "apollo two".to_string(),
            path: path.to_string_lossy().to_string(),
        },
    ];
    let text = flatten(&artifact_lines(&media, 0, 4));
    assert!(text.contains("apollo one"));
    assert!(text.contains("apollo two"));
    assert!(text.matches("x").count() >= 2);
}

#[test]
fn repeated_borrowed_artifact_summaries_stay_borrowed() {
    let media = Media::Resource {
        label: "remote resource".to_string(),
        url: "https://example.com/really/long/resource".to_string(),
    };
    let mut last_summary = None;
    let first = frame_summary(&media, &mut last_summary);
    let second = frame_summary(&media, &mut last_summary);
    assert!(matches!(first, Cow::Borrowed(_)));
    assert!(matches!(second, Cow::Borrowed(_)));
    assert_eq!(first, second);
}

#[test]
fn tall_artifact_panels_cap_rows_per_frame() {
    let media = (0..80)
        .map(|i| Media::Link {
            label: format!("operator notes {i}"),
            url: format!("https://example.com/notes/{i}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        artifact_lines(&media, 0, 120).len(),
        MAX_ARTIFACT_ROWS_PER_FRAME
    );
    let scrolled = flatten(&artifact_lines(&media, 40, 120));
    assert!(scrolled.contains("operator notes 40"));
    assert!(!scrolled.contains("operator notes 0"));
}

#[test]
fn label_spans_pad_and_truncate_to_panel_width() {
    let mut short = Vec::new();
    append_label_spans(&mut short, "short");
    let short_text = short
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    assert_eq!(short_text.len(), ARTIFACT_LABEL_WIDTH);
    assert!(matches!(short[0].content, Cow::Borrowed(_)));

    let mut ascii = Vec::new();
    append_label_spans(&mut ascii, "abcdefghijklmnopqrstuvwxyz");
    assert_eq!(
        ascii
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>(),
        "abcdefghijklmnopqrstuv"
    );
    assert_eq!(
        clipped_ascii_label("abcdefghijklmnopqrstuvwxyz"),
        Some("abcdefghijklmnopqrstuv")
    );
    assert_eq!(clipped_ascii_label("short"), None);

    let mut unicode = Vec::new();
    append_label_spans(&mut unicode, "日本語artifact");
    assert_eq!(
        unicode
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
            .chars()
            .count(),
        ARTIFACT_LABEL_WIDTH
    );

    let mut long_unicode = Vec::new();
    append_label_spans(
        &mut long_unicode,
        "日本語artifact日本語artifact日本語artifact",
    );
    assert_eq!(
        long_unicode[0].content.chars().count(),
        ARTIFACT_LABEL_WIDTH
    );
    assert!(matches!(long_unicode[0].content, Cow::Borrowed(_)));
}

#[test]
fn index_prefixes_cover_visible_window_and_fallback() {
    assert_eq!(index_prefix(1), Some("01 "));
    assert_eq!(index_prefix(MAX_ARTIFACT_ROWS_PER_FRAME), Some("36 "));
    assert_eq!(index_prefix(121), Some("121 "));
    assert_eq!(index_prefix(STATIC_INDEX_PREFIX_LIMIT), Some("240 "));
    assert_eq!(index_prefix(STATIC_INDEX_PREFIX_LIMIT + 1), None);
    assert_eq!(index_span(120, Style::new()).content.as_ref(), "120 ");
    assert_eq!(index_span(241, Style::new()).content.as_ref(), "241 ");
}

#[test]
fn media_meta_includes_local_image_dimensions() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets/agents/sparky-neutral.png");
    let media = Media::Image {
        label: "sparky".to_string(),
        path: path.to_string_lossy().to_string(),
    };
    let text = flatten(&media_meta(&media));
    assert!(text.contains("target"));
    assert!(text.contains("image"));
    assert!(text.contains("x"));
}
