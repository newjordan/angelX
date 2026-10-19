//! Clickable transcript links. The hit-map is rendered with the very same
//! Paragraph wrapping as visible text; destination identity never comes from
//! matching labels or from the terminal emulator's hyperlink support.
use super::{
    markdown,
    transcript::{Message, Role},
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct LinkHit {
    pub(crate) rect: Rect,
    pub(crate) target: Arc<str>,
}

#[derive(Clone, Debug)]
pub(crate) struct PressedLink {
    pub(crate) point: (u16, u16),
    pub(crate) target: Arc<str>,
}

/// Record only links in visible ordinary assistant blocks. A semantic marker
/// is painted into an offscreen cell buffer, never into the actual frame.
pub(crate) fn record(
    hits: &mut Vec<LinkHit>,
    message: &Message,
    width: usize,
    rect: Rect,
    skip: u16,
) {
    if !matches!(message.role, Role::Angel) {
        return;
    }
    record_text(hits, &message.text, width, rect, skip, false);
}

pub(crate) fn record_partial(
    hits: &mut Vec<LinkHit>,
    text: &str,
    width: usize,
    rect: Rect,
    skip: u16,
) {
    // This is the same bound as transcript::partial_render's rich path.
    if text.len() <= 32 * 1024 {
        record_text(hits, text, width, rect, skip, true);
    }
}

fn record_text(
    hits: &mut Vec<LinkHit>,
    text: &str,
    width: usize,
    rect: Rect,
    skip: u16,
    partial: bool,
) {
    if !text.contains(['[', '<']) || rect.is_empty() {
        return;
    }
    let links = markdown::links(text, width);
    if links.is_empty() {
        return;
    }
    let cached = if partial {
        markdown::render_partial(text, width)
    } else {
        markdown::render(text, width)
    };
    let mut lines: Vec<Line<'_>> = cached
        .iter()
        .map(|line| Line {
            spans: line
                .spans
                .iter()
                .map(|span| Span::styled(span.content.as_ref(), span.style.bg(Color::Reset)))
                .collect(),
            style: Style::new(),
            alignment: line.alignment,
        })
        .collect();
    for (id, link) in links.iter().enumerate() {
        if let Some(span) = lines
            .get_mut(link.line)
            .and_then(|line| line.spans.get_mut(link.span))
        {
            // Rgb is a marker for one span; contiguous spans sharing a URL are
            // folded back into cell runs below. Normal prose stays Reset.
            let id = id as u32 + 1;
            span.style = span
                .style
                .bg(Color::Rgb((id >> 16) as u8, (id >> 8) as u8, id as u8));
        }
    }
    if let Some(first) = lines.first_mut() {
        first.spans.insert(0, Span::raw("angel "));
    }
    if partial {
        if let Some(last) = lines.last_mut() {
            last.spans.push(Span::raw(" ▌"));
        }
    } else {
        // Match the breathing row appended to committed assistant messages.
        lines.push(Line::raw(""));
    }
    let local = Rect::new(0, 0, rect.width, rect.height);
    let mut buffer = Buffer::empty(local);
    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((skip, 0))
        .render(local, &mut buffer);
    for y in 0..rect.height {
        let mut x = 0;
        while x < rect.width {
            let Color::Rgb(r, g, b) = buffer[(x, y)].bg else {
                x += 1;
                continue;
            };
            let id = ((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)).wrapping_sub(1)
                as usize;
            let Some(link) = links.get(id) else {
                x += 1;
                continue;
            };
            let start = x;
            while x < rect.width && buffer[(x, y)].bg == Color::Rgb(r, g, b) {
                x += 1;
            }
            hits.push(LinkHit {
                rect: Rect::new(rect.x + start, rect.y + y, x - start, 1),
                target: Arc::clone(&link.target),
            });
        }
    }
}

impl crate::app::App {
    pub(crate) fn activate_transcript_link(&mut self, target: &str) {
        let root = self.tools.current_workspace().to_path_buf();
        let result = super::media::model_presentation_target_in(target, &root).and_then(|target| {
            if target.starts_with("https://") || target.starts_with("http://") {
                return super::media::open_target(&target).map_err(|error| error.to_string());
            }
            let card = super::media::artifact_from_path_in(&target, &root)?;
            let root = root.canonicalize().map_err(|error| error.to_string())?;
            self.media.push(super::media::Media::Confined {
                card: Box::new(card),
                root,
            });
            self.scryglass.reveal_media(self.media.len() - 1, true);
            self.focus_module("artifacts");
            Ok(())
        });
        if let Err(error) = result {
            self.system_msg(format!("Cannot open transcript link: {error}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    #[test]
    fn transcript_link_hitmap_matches_visible_unicode_wrap_and_scroll_cells() {
        let text = "# Heading\n\nPlain text before [same **bold 界 e\u{301}** link](Cargo.toml), then [same](other.md).\n\n> Quoted [a much longer link spanning several rows](quoted.md)\n\n- [reference link][ref]\n\n[ref]: reference.md\n\n```text\n[not a real link](fake.md)\n```";
        let message = Message::new(Role::Angel, text);
        for width in [11u16, 19, 36, 80] {
            for skip in [0, 1, 3, 8, 15] {
                let rect = Rect::new(4, 6, width, 12);
                let mut hits = Vec::new();
                record(&mut hits, &message, usize::from(width), rect, skip);
                let mut renders = Vec::new();
                let lines = super::super::transcript::lines(
                    std::slice::from_ref(&message),
                    "",
                    false,
                    usize::from(width),
                    &mut renders,
                );
                let mut visible = Buffer::empty(rect);
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .scroll((skip, 0))
                    .render(rect, &mut visible);
                for y in rect.y..rect.bottom() {
                    for x in rect.x..rect.right() {
                        let cell = &visible[(x, y)];
                        if cell.symbol().trim().is_empty() {
                            continue;
                        }
                        let mapped = hits.iter().any(|hit| hit.rect.contains((x, y).into()));
                        assert_eq!(
                            mapped,
                            cell.modifier.contains(Modifier::UNDERLINED),
                            "width {width}, skip {skip}, ({x},{y}) {:?}",
                            cell.symbol()
                        );
                    }
                }
                for hit in &hits {
                    assert_eq!(hit.rect.intersection(rect), hit.rect);
                    assert_ne!(hit.target.as_ref(), "fake.md");
                }
            }
        }
    }

    #[test]
    fn transcript_link_angle_bracket_autolink_is_rendered_and_mapped() {
        let message = Message::new(Role::Angel, "Open <https://example.com/report> now.");
        let rect = Rect::new(0, 0, 48, 4);
        let mut hits = Vec::new();
        record(&mut hits, &message, 48, rect, 0);
        let hit = hits.first().expect("autolink hit");
        assert_eq!(hit.target.as_ref(), "https://example.com/report");
        let mut renders = Vec::new();
        let lines = super::super::transcript::lines(
            std::slice::from_ref(&message),
            "",
            false,
            48,
            &mut renders,
        );
        let mut visible = Buffer::empty(rect);
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(rect, &mut visible);
        assert!(
            visible[(hit.rect.x, hit.rect.y)]
                .modifier
                .contains(Modifier::UNDERLINED)
        );
        assert_eq!(visible[(hit.rect.x, hit.rect.y)].symbol(), "h");
    }

    #[test]
    fn transcript_link_duplicate_labels_keep_their_exact_destinations() {
        let message = Message::new(Role::Angel, "[same](first.md) / [same](second.md)");
        let mut hits = Vec::new();
        record(&mut hits, &message, 60, Rect::new(0, 0, 60, 3), 0);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].target.as_ref(), "first.md");
        assert_eq!(hits[1].target.as_ref(), "second.md");
        assert!(hits[0].rect.intersection(hits[1].rect).is_empty());
    }

    #[test]
    fn transcript_link_unsafe_targets_are_rejected_without_stage_mutation() {
        let _guard = crate::tests::env_lock();
        let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
        let count = app.media.len();
        for target in [
            "javascript:alert(1)",
            "ssh://host/file",
            "file:///etc/passwd",
            "/etc/passwd",
            "../Cargo.toml",
            "bad\npath",
            "missing-transcript-file.png",
        ] {
            app.activate_transcript_link(target);
            assert_eq!(app.media.len(), count, "accepted {target:?}");
            assert!(
                app.messages
                    .last()
                    .unwrap()
                    .text
                    .starts_with("Cannot open transcript link:")
            );
        }
    }
}
