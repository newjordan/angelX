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
        return vec![crate::views::status_view::empty_artifacts_line()];
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
#[path = "../../../tests/cockpit/app/artifacts_view__tests.rs"]
mod tests;
