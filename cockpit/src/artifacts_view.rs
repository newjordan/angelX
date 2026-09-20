use crate::glyphs;
use crate::hud::{HUD_BLUE, HUD_DIM};
use crate::media::Media;
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::borrow::Cow;
use std::sync::OnceLock;

const ARTIFACT_LABEL_WIDTH: usize = 22;
const MAX_ARTIFACT_ROWS_PER_FRAME: usize = 36;
const STATIC_INDEX_PREFIX_LIMIT: usize = 240;
const INLINE_INDEX_PREFIX_LIMIT: usize = 99;
const INDEX_STYLE: Style = Style::new().fg(HUD_BLUE);
const DIM_STYLE: Style = Style::new().fg(HUD_DIM);
const INLINE_INDEX_PREFIXES: &str = "\
01 02 03 04 05 06 07 08 09 10 \
11 12 13 14 15 16 17 18 19 20 \
21 22 23 24 25 26 27 28 29 30 \
31 32 33 34 35 36 37 38 39 40 \
41 42 43 44 45 46 47 48 49 50 \
51 52 53 54 55 56 57 58 59 60 \
61 62 63 64 65 66 67 68 69 70 \
71 72 73 74 75 76 77 78 79 80 \
81 82 83 84 85 86 87 88 89 90 \
91 92 93 94 95 96 97 98 99 ";

#[cfg(test)]
pub fn media_meta(m: &Media) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled("target ", Style::new().fg(HUD_BLUE)),
        Span::raw(m.target()),
    ])];
    if let Some(path) = m.local_path() {
        let exists = path.exists();
        let size = std::fs::metadata(&path)
            .ok()
            .map(|m| crate::media::format_bytes(m.len()))
            .unwrap_or_else(|| "missing".to_string());
        lines.push(Line::from(vec![
            Span::styled("file   ", Style::new().fg(HUD_BLUE)),
            Span::raw(if exists { "local " } else { "missing " }),
            Span::raw(size),
        ]));
        if m.is_image() {
            match image::image_dimensions(&path) {
                Ok((w, h)) => lines.push(Line::from(vec![
                    Span::styled("image  ", Style::new().fg(HUD_BLUE)),
                    Span::raw(format!("{w}x{h} · terminal previewable")),
                ])),
                Err(e) => lines.push(Line::from(vec![
                    Span::styled("image  ", Style::new().fg(HUD_BLUE)),
                    Span::raw(format!("dimension read failed: {e}")),
                ])),
            }
        }
    }
    lines
}

pub fn artifact_lines(media: &[Media], scroll: usize, height: u16) -> Vec<Line<'_>> {
    let visible = (height as usize).min(MAX_ARTIFACT_ROWS_PER_FRAME);
    let visible_media = media.get(scroll..).unwrap_or(&[]);
    if visible == 0 || visible_media.is_empty() {
        return vec![crate::status_view::empty_artifacts_line()];
    }
    let row_count = visible.min(visible_media.len());
    let visible_media = &visible_media[..row_count];
    let mut lines = Vec::with_capacity(row_count);
    let prefixes = media_prefixes();
    let mut last_summary = None;
    for (offset, m) in visible_media.iter().enumerate() {
        let index = scroll + offset + 1;
        let summary = frame_summary(m, &mut last_summary);
        let label = m.label();
        if let Some(label) = clipped_ascii_label(label) {
            lines.push(Line::from(vec![
                index_span(index, INDEX_STYLE),
                Span::styled(media_prefix(m, prefixes), DIM_STYLE),
                Span::raw(label),
                Span::raw("  "),
                Span::styled(summary, DIM_STYLE),
            ]));
        } else {
            let mut spans = Vec::with_capacity(6);
            spans.push(index_span(index, INDEX_STYLE));
            spans.push(Span::styled(media_prefix(m, prefixes), DIM_STYLE));
            append_label_spans(&mut spans, label);
            spans.push(Span::raw("  "));
            spans.push(Span::styled(summary, DIM_STYLE));
            lines.push(Line::from(spans));
        }
    }
    lines
}

#[derive(Clone, Copy)]
struct MediaPrefixes {
    image: &'static str,
    video: &'static str,
    link: &'static str,
    graph: &'static str,
    resource: &'static str,
}

fn media_prefixes() -> MediaPrefixes {
    static PREFIXES: OnceLock<MediaPrefixes> = OnceLock::new();
    *PREFIXES.get_or_init(|| MediaPrefixes {
        image: glyphs::media_prefix("img"),
        video: glyphs::media_prefix("video"),
        link: glyphs::media_prefix("link"),
        graph: glyphs::media_prefix("graph"),
        resource: glyphs::media_prefix("res"),
    })
}

fn media_prefix(media: &Media, prefixes: MediaPrefixes) -> &'static str {
    match media {
        Media::Image { .. } => prefixes.image,
        Media::Video { .. } => prefixes.video,
        Media::Link { .. } => prefixes.link,
        Media::Graph { .. } => prefixes.graph,
        Media::Resource { .. } => prefixes.resource,
        Media::Confined { card, .. } => media_prefix(card, prefixes),
    }
}

fn index_span(index: usize, style: Style) -> Span<'static> {
    match index_prefix(index) {
        Some(prefix) => Span::styled(prefix, style),
        None => Span::styled(format!("{index:02} "), style),
    }
}

fn index_prefix(index: usize) -> Option<&'static str> {
    if index != 0 && index <= INLINE_INDEX_PREFIX_LIMIT {
        let start = (index - 1) * 3;
        return Some(&INLINE_INDEX_PREFIXES[start..start + 3]);
    }
    static_index_prefix(index)
}

fn static_index_prefix(index: usize) -> Option<&'static str> {
    if index <= INLINE_INDEX_PREFIX_LIMIT || index > STATIC_INDEX_PREFIX_LIMIT {
        return None;
    }
    static PREFIXES: OnceLock<Vec<String>> = OnceLock::new();
    PREFIXES
        .get_or_init(|| {
            (INLINE_INDEX_PREFIX_LIMIT + 1..=STATIC_INDEX_PREFIX_LIMIT)
                .map(|index| format!("{index} "))
                .collect()
        })
        .get(index - INLINE_INDEX_PREFIX_LIMIT - 1)
        .map(String::as_str)
}

fn frame_summary<'a>(
    media: &'a Media,
    last_summary: &mut Option<(&'a str, Cow<'a, str>)>,
) -> Cow<'a, str> {
    let key = match media {
        Media::Image { path, .. } | Media::Video { path, .. } => Some(path.as_str()),
        Media::Resource { url, .. } => Some(url.as_str()),
        _ => None,
    };
    let Some(key) = key else {
        return media.artifact_summary_cow();
    };
    if let Some((cached, summary)) = last_summary.as_ref()
        && *cached == key
    {
        return summary.clone();
    }
    let summary = media.artifact_summary_cow();
    *last_summary = Some((key, summary.clone()));
    summary
}

fn clipped_ascii_label(label: &str) -> Option<&str> {
    if label.is_ascii() && label.len() >= ARTIFACT_LABEL_WIDTH {
        Some(&label[..ARTIFACT_LABEL_WIDTH])
    } else {
        None
    }
}

fn append_label_spans<'a>(spans: &mut Vec<Span<'a>>, label: &'a str) {
    if label.is_ascii() {
        if label.len() >= ARTIFACT_LABEL_WIDTH {
            spans.push(Span::raw(&label[..ARTIFACT_LABEL_WIDTH]));
        } else {
            spans.push(Span::raw(label));
            spans.push(Span::raw(padding(ARTIFACT_LABEL_WIDTH - label.len())));
        }
        return;
    }

    let mut char_count = 0usize;
    for (index, _) in label.char_indices() {
        if char_count == ARTIFACT_LABEL_WIDTH {
            spans.push(Span::raw(&label[..index]));
            return;
        }
        char_count += 1;
    }
    spans.push(Span::raw(label));
    spans.push(Span::raw(padding(ARTIFACT_LABEL_WIDTH - char_count)));
}

fn padding(width: usize) -> &'static str {
    const PADDING: &str = "                      ";
    &PADDING[..width.min(ARTIFACT_LABEL_WIDTH)]
}

#[cfg(test)]
mod tests {
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
            crate::status_view::empty_artifacts_line().spans[0]
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
            crate::status_view::empty_artifacts_line().spans[0]
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
}
