pub(crate) mod oversized;
pub(crate) mod reflow;

use crate::hud::{HUD_BLUE, HUD_DIM, HUD_GOLD, HUD_PHOSPHOR, HUD_PURPLE};
use crate::markdown;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use std::sync::Arc;

// Speaker identity in the Camelot-noir palette: the operator speaks in
// phosphor, the knight answers in candlelight gold, the system murmurs dim.
const EMPTY_STYLE: Style = Style::new();
const USER_TEXT_STYLE: Style = Style::new().fg(HUD_PHOSPHOR);
const USER_TAG_STYLE: Style = Style::new().fg(HUD_PURPLE).add_modifier(Modifier::BOLD);
const ANGEL_TAG_STYLE: Style = Style::new().fg(HUD_GOLD).add_modifier(Modifier::BOLD);
const SYSTEM_TAG_STYLE: Style = Style::new().fg(Color::DarkGray);
const SYSTEM_TEXT_STYLE: Style = Style::new().fg(HUD_DIM);
/// Activity murmurs sit in the same quiet blue-grey as system text — gold is
/// reserved for the knight's own words, so progress chrome never reads as a
/// wall of compressed answer text.
const ACTIVITY_STYLE: Style = Style::new().fg(HUD_DIM);
/// Guard-gauge tiers: a repeating guard notice holds one row whose background
/// fills left→right with the repeat count — a severity meter, not a wall of
/// identical lines. Low repeats simmer in steel blue, sustained repeats heat
/// to gold, and a near-treadmill-stop count burns danger red.
const GAUGE_LOW_FILL: Style = Style::new()
    .fg(crate::hud::HUD_TEXT)
    .bg(Color::Rgb(31, 58, 79));
const GAUGE_MID_FILL: Style = Style::new().fg(HUD_GOLD).bg(Color::Rgb(87, 66, 20));
const GAUGE_HOT_FILL: Style = Style::new()
    .fg(crate::hud::HUD_DANGER)
    .bg(Color::Rgb(96, 30, 42));
/// Repeats at the mid/hot color transitions.
const GAUGE_MID_AT: usize = 5;
const GAUGE_HOT_AT: usize = 9;
/// Sub-agent traffic (spawn seats, delegates) speaks under its own banner.
const COUNCIL_TAG_STYLE: Style = Style::new().fg(HUD_BLUE).add_modifier(Modifier::BOLD);
const PARTIAL_CURSOR_STYLE: Style = Style::new().fg(HUD_GOLD);
/// Above this point, a growing response renders as plain text until completion.
/// This caps repeated CommonMark + syntax-highlight work while preserving the
/// exact bytes and cursor; the committed answer receives its normal cached
/// Markdown render once.
const MAX_LIVE_MARKDOWN_BYTES: usize = 32 * 1024;
const MAX_LIVE_PARTIAL_WINDOW_BYTES: usize = 16 * 1024;

#[derive(Default)]
pub struct PartialHeightCache {
    initialized: bool,
    width: usize,
    text_len: usize,
    last_line_start: usize,
    stable_rows: u32,
    height: u16,
    observed: String,
}

impl PartialHeightCache {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Copy)]
pub enum Role {
    User,
    Angel,
    System,
    /// Live tool-call activity trace streamed during a turn.
    Activity,
    /// Agent-to-agent traffic surfaced to the operator: spawn-seat replies,
    /// delegate reports — the conversations between agents, not chrome.
    Council,
}

pub struct Message {
    pub role: Role,
    /// Committed transcript text. Immutable after construction except for
    /// explicit in-place rewrite (superseded-draft collapse). Echo, history,
    /// and retry share this buffer with `ChatMsg.content` when the bytes match.
    pub text: Arc<str>,
}

impl Message {
    pub fn new(role: Role, text: impl Into<Arc<str>>) -> Self {
        Self {
            role,
            text: text.into(),
        }
    }
}

/// One message's cached markdown render (`None` = the plain-text path). The
/// caller of [`lines`] owns a scratch `Vec` of these so the returned lines can
/// borrow span text straight out of the render cache — cloning the Arcs is the
/// only per-frame cost, never the rendered Lines' strings.
pub type MdRender = Option<Arc<Vec<Line<'static>>>>;

pub fn lines<'a>(
    messages: &'a [Message],
    partial: &'a str,
    thinking: bool,
    inner_width: usize,
    renders: &'a mut Vec<MdRender>,
) -> Vec<Line<'a>> {
    let live = thinking && !partial.is_empty();
    // Collect the renders first (one slot per message, plus one for the live
    // partial), then reborrow shared so the output lines can borrow from them.
    renders.clear();
    renders.reserve(messages.len() + usize::from(live));
    for message in messages {
        renders.push(message_render(message, inner_width));
    }
    if live {
        renders.push(partial_render(partial, inner_width));
    }
    let renders: &'a [MdRender] = renders;

    let mut lines: Vec<Line> =
        Vec::with_capacity(messages.len().saturating_mul(2) + usize::from(live));
    for (message, md) in messages.iter().zip(renders) {
        append_message_lines(&mut lines, message, borrowed(md), inner_width);
    }

    if live {
        append_partial_lines(&mut lines, partial, borrowed(&renders[messages.len()]));
    } else if thinking {
        // Nothing streamed yet: show the reply cursor immediately so the send
        // visibly registered — the answer begins here, it just hasn't spoken.
        let (tag, tag_style) = message_tag(Role::Angel);
        lines.push(Line::from(vec![
            Span::styled(tag, tag_style),
            Span::styled("\u{258c}", PARTIAL_CURSOR_STYLE),
        ]));
    }

    lines
}

/// Markdown render for one committed message (`None` = the plain fast path).
fn message_render(message: &Message, inner_width: usize) -> MdRender {
    match message.role {
        Role::Angel if !is_plain_agent_text(&message.text) => {
            Some(markdown::render(&message.text, inner_width))
        }
        _ => None,
    }
}

/// Markdown render for the streaming partial — through the dedicated single-slot
/// cache, so its ever-growing text never enters (and thrashes) the shared LRU.
fn partial_render(partial: &str, inner_width: usize) -> MdRender {
    if partial.len() > MAX_LIVE_MARKDOWN_BYTES || is_plain_agent_text(partial) {
        None
    } else {
        Some(markdown::render_partial(partial, inner_width))
    }
}

fn borrowed(md: &MdRender) -> Option<&[Line<'static>]> {
    md.as_deref().map(Vec::as_slice)
}

/// Visual (wrapped) height of one message at `inner_width` — exactly how
/// `render_transcript` wraps it (same `Wrap { trim: false }`). Lets the renderer
/// cache per-message heights and window the transcript to the viewport instead
/// of re-wrapping the whole (unbounded) history every frame.
pub fn message_height(message: &Message, inner_width: usize) -> u16 {
    let md = message_render(message, inner_width);
    let mut tmp: Vec<Line> = Vec::new();
    append_message_lines(&mut tmp, message, borrowed(&md), inner_width);
    wrapped_height(tmp, inner_width)
}

/// Visual height of the streaming partial (with its cursor row), or 0 when there
/// is no live partial. Computed per frame; the single-slot partial cache makes
/// this and the same-frame `lines()` call share one markdown render.
/// Append-aware height for the live partial. Once the Markdown live-render cap
/// is crossed, completed logical lines become stable: only the prior last line
/// plus newly appended bytes need wrapping on the next frame.
pub fn partial_height_cached(
    cache: &mut PartialHeightCache,
    partial: &str,
    thinking: bool,
    inner_width: usize,
) -> u16 {
    if !thinking {
        cache.clear();
        return 0;
    }
    if partial.is_empty() {
        // The pre-stream reply cursor row (`lines` renders "angel ▌").
        cache.clear();
        return 1;
    }
    let md = partial_render(partial, inner_width);
    if let Some(md) = md {
        cache.clear();
        let mut tmp: Vec<Line> = Vec::new();
        append_partial_lines(&mut tmp, partial, Some(md.as_slice()));
        return wrapped_height(tmp, inner_width);
    }

    if cache.initialized && cache.width == inner_width && cache.text_len == partial.len() {
        return cache.height;
    }
    let can_extend = cache.initialized
        && cache.width == inner_width
        && cache.text_len < partial.len()
        && partial.is_char_boundary(cache.text_len)
        && partial.starts_with(&cache.observed);
    if !can_extend {
        cache.clear();
        cache.initialized = true;
        cache.width = inner_width;
        cache.last_line_start = displayed_last_line_start(partial);
        cache.stable_rows = if cache.last_line_start == 0 {
            0
        } else {
            plain_segment_height(&partial[..cache.last_line_start], true, false, inner_width)
        };
        cache.observed.push_str(partial);
    } else {
        let new_last_start = displayed_last_line_start(partial);
        if new_last_start < cache.last_line_start {
            cache.clear();
            return partial_height_cached(cache, partial, thinking, inner_width);
        }
        if new_last_start > cache.last_line_start {
            cache.stable_rows = cache.stable_rows.saturating_add(plain_segment_height(
                &partial[cache.last_line_start..new_last_start],
                cache.last_line_start == 0,
                false,
                inner_width,
            ));
        }
        cache.last_line_start = new_last_start;
        cache.observed.push_str(&partial[cache.text_len..]);
    }
    cache.text_len = partial.len();
    let current_rows = plain_segment_height(
        &partial[cache.last_line_start..],
        cache.last_line_start == 0,
        true,
        inner_width,
    );
    cache.height = cache
        .stable_rows
        .saturating_add(current_rows)
        .min(u16::MAX as u32) as u16;
    cache.height
}

fn displayed_last_line_start(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let search_end = text
        .strip_suffix('\n')
        .map_or(text.len(), |without_newline| without_newline.len());
    text[..search_end].rfind('\n').map_or(0, |index| index + 1)
}

fn plain_segment_height(text: &str, first: bool, cursor: bool, inner_width: usize) -> u32 {
    let mut tmp: Vec<Line> = Vec::new();
    let (tag, tag_style) = message_tag(Role::Angel);
    append_plain_message_lines(
        &mut tmp,
        text,
        if first { tag } else { "" },
        tag_style,
        None,
    );
    if cursor && let Some(last) = tmp.last_mut() {
        last.spans.push(Span::styled(" ▌", PARTIAL_CURSOR_STYLE));
    }
    wrapped_height_u32(tmp, inner_width)
}

/// Plain, bounded tail used only for the pinned-to-latest oversized streaming
/// view. The full partial remains stored and its exact accumulated height stays
/// in the cache, so scrolling back or committing the turn loses no content.
pub fn oversized_partial_window(partial: &str) -> Option<&str> {
    if partial.len() <= MAX_LIVE_MARKDOWN_BYTES {
        return None;
    }
    let mut start = partial.len().saturating_sub(MAX_LIVE_PARTIAL_WINDOW_BYTES);
    while start < partial.len() && !partial.is_char_boundary(start) {
        start += 1;
    }
    Some(&partial[start..])
}

pub fn plain_partial_lines(partial: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    append_partial_lines(&mut lines, partial, None);
    lines
}

pub fn plain_partial_height(partial: &str, inner_width: usize) -> u16 {
    plain_segment_height(partial, true, true, inner_width).min(u16::MAX as u32) as u16
}

fn wrapped_height(lines: Vec<Line>, inner_width: usize) -> u16 {
    wrapped_height_u32(lines, inner_width).min(u16::MAX as u32) as u16
}

fn wrapped_height_u32(lines: Vec<Line>, inner_width: usize) -> u32 {
    let w = inner_width.max(1) as u16;
    if w == 0 {
        return 0;
    }
    lines.into_iter().fold(0u32, |height, line| {
        // Printable ASCII has additive one-cell widths. General Line::width
        // is not a safe fit oracle: contextual Unicode widths (e.g. Arabic
        // lam-alef) can differ from Paragraph's styled-grapheme wrapping.
        let mut cells = 0;
        let fits = line.spans.iter().all(|span| {
            cells += span.content.len();
            cells <= usize::from(w) && span.content.bytes().all(|b| (b' '..=b'~').contains(&b))
        });
        let rows = if fits {
            1
        } else {
            Paragraph::new(line)
                .wrap(Wrap { trim: false })
                .line_count(w)
        };
        height.saturating_add(rows.min(u32::MAX as usize) as u32)
    })
}

fn append_partial_lines<'a>(
    lines: &mut Vec<Line<'a>>,
    partial: &'a str,
    md: Option<&'a [Line<'static>]>,
) {
    let (tag, tag_style) = message_tag(Role::Angel);
    match md {
        Some(cached) => lines.extend(angel_message_lines(cached, tag, tag_style)),
        None => append_plain_message_lines(lines, partial, tag, tag_style, None),
    }
    if let Some(last) = lines.last_mut() {
        last.spans.push(Span::styled(" ▌", PARTIAL_CURSOR_STYLE));
    }
}

fn append_message_lines<'a>(
    lines: &mut Vec<Line<'a>>,
    message: &'a Message,
    md: Option<&'a [Line<'static>]>,
    inner_width: usize,
) {
    let (tag, tag_style) = message_tag(message.role);
    match message.role {
        Role::Angel => {
            match md {
                Some(cached) => lines.extend(angel_message_lines(cached, tag, tag_style)),
                None => append_plain_message_lines(lines, &message.text, tag, tag_style, None),
            }
            // One trailing blank after conversational turns — breathing room
            // between blocks without double-spacing every prose line.
            ensure_trailing_blank(lines);
        }
        Role::User => {
            // A short prompt can sit on the right without layout blanks in
            // copied continuation rows. Multiline/wrapped pastes instead get a
            // right-hand speaker header and an unpadded body: independent row
            // alignment destroys code/table columns and pollutes mouse copy.
            let single_row = !message.text.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
                && Line::raw(format!("{tag}{}", message.text)).width() <= inner_width;
            if single_row {
                let start = lines.len();
                append_plain_message_lines(
                    lines,
                    &message.text,
                    tag,
                    tag_style,
                    Some(USER_TEXT_STYLE),
                );
                lines[start].alignment = Some(ratatui::layout::Alignment::Right);
            } else {
                lines.push(Line::from(Span::styled(tag.trim_end(), tag_style)).right_aligned());
                append_plain_message_lines(
                    lines,
                    &message.text,
                    "",
                    tag_style,
                    Some(USER_TEXT_STYLE),
                );
            }
            ensure_trailing_blank(lines);
        }
        Role::System => {
            append_plain_message_lines(
                lines,
                &message.text,
                tag,
                tag_style,
                Some(SYSTEM_TEXT_STYLE),
            );
        }
        Role::Activity => {
            for line in message.text.lines() {
                lines.push(activity_line(line));
            }
        }
        Role::Council => {
            append_plain_message_lines(lines, &message.text, tag, tag_style, None);
            ensure_trailing_blank(lines);
        }
    }
}

/// One rendered Activity row. A guard-gauge row (`… ×N`) gets a progressive
/// background fill proportional to its repeat count; everything else keeps the
/// quiet dim style. Same text, same spans-per-line wrap → identical height, so
/// in-place gauge bumps only re-measure their own row.
fn activity_line<'a>(line: &'a str) -> Line<'a> {
    let Some(count) = crate::turn_event_view::activity_gauge_count(line) else {
        return Line::from(vec![
            Span::styled("  ", EMPTY_STYLE),
            Span::styled(line, ACTIVITY_STYLE),
        ]);
    };
    let fill_style = if count >= GAUGE_HOT_AT {
        GAUGE_HOT_FILL
    } else if count >= GAUGE_MID_AT {
        GAUGE_MID_FILL
    } else {
        GAUGE_LOW_FILL
    };
    let (body, chip) = line.rsplit_once(" ×").expect("gauge rows carry a count");
    let (chip, tail) = chip
        .find(" · ")
        .map_or((chip, ""), |index| chip.split_at(index));
    // One left-to-right fill across both body segments; the chip itself has
    // no background. Guard notices have an empty tail and paint as before.
    let full = crate::turn_event_view::NOTICE_GAUGE_FULL;
    let chars = body.chars().count() + tail.chars().count();
    let mut remaining = (chars * count.min(full)).div_ceil(full);
    let mut paint = |text: &'a str| {
        let filled = remaining.min(text.chars().count());
        remaining = remaining.saturating_sub(filled);
        let split = text
            .char_indices()
            .nth(filled)
            .map_or(text.len(), |(index, _)| index);
        let mut spans = vec![Span::styled(&text[..split], fill_style)];
        if split < text.len() {
            spans.push(Span::styled(&text[split..], ACTIVITY_STYLE));
        }
        spans
    };
    let mut spans = vec![Span::styled("  ", EMPTY_STYLE)];
    spans.extend(paint(body));
    spans.push(Span::styled(" ×", ACTIVITY_STYLE));
    spans.push(Span::styled(
        chip,
        fill_style.bg(Color::Reset).add_modifier(Modifier::BOLD),
    ));
    if !tail.is_empty() {
        spans.extend(paint(tail));
    }
    Line::from(spans)
}

/// Append a single empty row when the last line already carries content.
fn ensure_trailing_blank(lines: &mut Vec<Line<'_>>) {
    if lines.last().is_some_and(|line| !line.spans.is_empty()) {
        lines.push(Line::raw(""));
    }
}

/// Display lines for a markdown-rendered angel message: fresh Line/Span
/// containers (so the tag/cursor can be attached) whose text borrows from the
/// cached render — the strings themselves are never copied per frame.
fn angel_message_lines<'a>(
    cached: &'a [Line<'static>],
    tag: &'static str,
    tag_style: Style,
) -> Vec<Line<'a>> {
    let mut md_lines: Vec<Line<'a>> = cached.iter().map(borrow_line).collect();
    if md_lines.is_empty() {
        md_lines.push(Line::raw(""));
    }
    prepend_tag(&mut md_lines, tag, tag_style);
    md_lines
}

fn borrow_line<'a>(line: &'a Line<'static>) -> Line<'a> {
    Line {
        spans: line
            .spans
            .iter()
            .map(|s| Span::styled(s.content.as_ref(), s.style))
            .collect(),
        style: line.style,
        alignment: line.alignment,
    }
}

fn is_plain_agent_text(text: &str) -> bool {
    // Newlines are allowed: multi-line agent prose is the common streaming
    // case and must not force a full CommonMark walk every token. Markdown
    // structure markers still force the rich path.
    !text.as_bytes().iter().any(|byte| {
        matches!(
            byte,
            b'#' | b'*'
                | b'_'
                | b'`'
                | b'['
                | b']'
                | b'('
                | b')'
                | b'>'
                | b'|'
                | b'-'
                | b'!'
                | b'$'
                | b'\\'
        )
    })
}

#[cfg(test)]
fn plain_message_lines<'a>(
    text: &'a str,
    tag: &'static str,
    tag_style: Style,
    text_style: Option<Style>,
) -> Vec<Line<'a>> {
    let mut lines = Vec::with_capacity(plain_line_capacity(text));
    append_plain_message_lines(&mut lines, text, tag, tag_style, text_style);
    lines
}

#[cfg(test)]
fn plain_line_capacity(text: &str) -> usize {
    if text.is_empty() {
        return 1;
    }
    text.as_bytes()
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

fn append_plain_message_lines<'a>(
    lines: &mut Vec<Line<'a>>,
    text: &'a str,
    tag: &'static str,
    tag_style: Style,
    text_style: Option<Style>,
) {
    let start_len = lines.len();
    for (index, line) in text.lines().enumerate() {
        let first = index == 0 && !tag.is_empty();
        let mut spans = Vec::with_capacity(if first { 2 } else { 1 });
        if first {
            spans.push(Span::styled(tag, tag_style));
        }
        spans.push(match text_style {
            Some(style) => Span::styled(line, style),
            None => Span::raw(line),
        });
        lines.push(Line::from(spans));
    }
    if lines.len() == start_len {
        if tag.is_empty() {
            lines.push(Line::raw(""));
        } else {
            lines.push(Line::from(vec![Span::styled(tag, tag_style)]));
        }
    }
}

fn message_tag(role: Role) -> (&'static str, Style) {
    match role {
        Role::User => ("you ", USER_TAG_STYLE),
        Role::Angel => ("angel ", ANGEL_TAG_STYLE),
        Role::System => ("· ", SYSTEM_TAG_STYLE),
        Role::Activity => ("", EMPTY_STYLE),
        Role::Council => ("agents ", COUNCIL_TAG_STYLE),
    }
}

fn prepend_tag(lines: &mut [Line<'_>], tag: &'static str, tag_style: Style) {
    if tag.is_empty() || lines.is_empty() {
        return;
    }
    lines[0].spans.insert(0, Span::styled(tag, tag_style));
}

#[cfg(test)]
mod wrapping_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn plain_text(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn user_and_angel_lines_get_role_tags() {
        let messages = [
            Message {
                role: Role::User,
                text: "hello".into(),
            },
            Message {
                role: Role::Angel,
                text: "world".into(),
            },
        ];
        let mut renders = Vec::new();
        let rendered = lines(&messages, "", false, 80, &mut renders);
        let text = plain_text(&rendered);
        assert!(text.contains("you "));
        assert!(text.contains("hello"));
        assert!(text.contains("angel "));
        assert!(text.contains("world"));
    }

    #[test]
    fn partial_response_renders_with_cursor_while_thinking() {
        let mut renders = Vec::new();
        let rendered = lines(&[], "streaming", true, 80, &mut renders);
        assert_eq!(rendered.capacity(), 1);
        let text = plain_text(&rendered);
        assert!(text.contains("angel "));
        assert!(text.contains("streaming"));
        assert!(text.contains("▌"));
        assert!(matches!(
            rendered[0]
                .spans
                .iter()
                .find(|span| span.content == "streaming")
                .unwrap()
                .content,
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn markdown_partial_response_renders_with_cursor() {
        let mut renders = Vec::new();
        let rendered = lines(&[], "**streaming**", true, 80, &mut renders);
        let text = plain_text(&rendered);
        assert!(text.contains("angel "));
        assert!(text.contains("streaming"));
        assert!(text.contains("▌"));
    }

    #[test]
    fn oversized_markdown_partial_defers_formatting_until_commit() {
        let partial = format!("# streamed\n\n{}", "**live markdown**\n".repeat(4_000));
        assert!(partial.len() > MAX_LIVE_MARKDOWN_BYTES);
        assert!(
            partial_render(&partial, 80).is_none(),
            "a growing oversized partial must not repeatedly parse Markdown"
        );

        let mut renders = Vec::new();
        let rendered = lines(&[], &partial, true, 80, &mut renders);
        let text = plain_text(&rendered);
        assert!(text.contains("# streamed"));
        assert!(text.contains("**live markdown**"));
        assert!(text.contains('▌'));

        let committed = Message {
            role: Role::Angel,
            text: partial.into(),
        };
        assert!(
            message_render(&committed, 80).is_some(),
            "the settled answer must receive normal cached Markdown rendering"
        );
    }

    #[test]
    fn growing_oversized_markdown_keeps_one_hundred_height_frames_interactive() {
        let chunk = format!(
            "## streamed section\n\n```rust\n{}\n```\n\n",
            "let value = **not actually emphasis**;\n".repeat(64)
        );
        let mut partial = String::new();
        let mut cache = PartialHeightCache::default();
        let started = std::time::Instant::now();
        for _ in 0..100 {
            partial.push_str(&chunk);
            std::hint::black_box(partial_height_cached(&mut cache, &partial, true, 96));
        }
        let elapsed = started.elapsed();
        assert!(partial.len() > 200_000, "fixture must cross the live cap");
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "100 growing Markdown frames took {elapsed:?}"
        );
    }

    #[test]
    fn incremental_plain_height_matches_full_rewrap_across_appends_and_reset() {
        let mut cache = PartialHeightCache::default();
        let mut partial = "# start\n".repeat(5_000);
        for suffix in [
            "unicode 界界\n",
            "trailing newline\n",
            "\n",
            "one very long final line ".repeat(40).as_str(),
        ] {
            partial.push_str(suffix);
            let cached = partial_height_cached(&mut cache, &partial, true, 73);
            let full = plain_segment_height(&partial, true, true, 73).min(u16::MAX as u32) as u16;
            assert_eq!(cached, full, "cache drift after suffix {suffix:?}");
        }

        let replacement = format!("replacement\n{}", "界 mixed width words\n".repeat(4_000));
        let cached = partial_height_cached(&mut cache, &replacement, true, 73);
        let full = plain_segment_height(&replacement, true, true, 73).min(u16::MAX as u32) as u16;
        assert_eq!(cached, full, "non-append replacement must rebuild");

        let resized = partial_height_cached(&mut cache, &replacement, true, 41);
        let resized_full =
            plain_segment_height(&replacement, true, true, 41).min(u16::MAX as u32) as u16;
        assert_eq!(resized, resized_full, "width change must rebuild");
    }

    #[test]
    fn plain_agent_text_uses_borrowed_fast_path() {
        let messages = [Message {
            role: Role::Angel,
            text: "plain response".into(),
        }];
        let mut renders = Vec::new();
        let rendered = lines(&messages, "", false, 80, &mut renders);
        assert_eq!(plain_text(&rendered), "angel \nplain response");
        assert!(matches!(
            rendered[0].spans.last().unwrap().content,
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn plain_message_lines_reserve_expected_capacity() {
        let (tag, style) = message_tag(Role::Angel);
        let single = plain_message_lines("plain response", tag, style, None);
        assert_eq!(single.capacity(), 1);
        let multi = plain_message_lines("one\ntwo\nthree", tag, style, None);
        assert_eq!(multi.capacity(), 3);
        assert_eq!(multi.len(), 3);
    }

    #[test]
    fn system_message_uses_dim_tag_and_text() {
        let messages = [Message {
            role: Role::System,
            text: "compacted earlier".into(),
        }];
        let mut renders = Vec::new();
        let rendered = lines(&messages, "", false, 80, &mut renders);
        let text = plain_text(&rendered);
        assert!(text.contains("· "), "system tag present:\n{text}");
        assert!(text.contains("compacted earlier"));
        // The system body span carries the dim text style.
        let body = rendered[0]
            .spans
            .iter()
            .find(|s| s.content == "compacted earlier")
            .expect("system body span");
        assert_eq!(body.style, SYSTEM_TEXT_STYLE);
    }

    #[test]
    fn activity_lines_are_indented_and_yellow_per_line() {
        let messages = [Message {
            role: Role::Activity,
            text: "T: run tests\nR: 5 passed".into(),
        }];
        let mut renders = Vec::new();
        let rendered = lines(&messages, "", false, 80, &mut renders);
        // One rendered line per source line, each indented and styled as activity.
        assert_eq!(rendered.len(), 2);
        for line in &rendered {
            assert_eq!(line.spans[0].content.as_ref(), "  ", "two-space indent");
            assert_eq!(line.spans[1].style, ACTIVITY_STYLE);
        }
        assert_eq!(rendered[0].spans[1].content.as_ref(), "T: run tests");
        assert_eq!(rendered[1].spans[1].content.as_ref(), "R: 5 passed");
    }

    #[test]
    fn receipt_gauge_testbackend_matches_guard_fill() {
        use ratatui::{Terminal, backend::TestBackend, widgets::Paragraph};
        let receipt = crate::turn_event_view::receipt_gauge_text(
            ".. action receipt · shell dispatch error ×29 · last 2 ms · 1–29 ms",
            "action receipt · shell dispatch error · 30 ms",
        )
        .unwrap();
        let guard = crate::turn_event_view::notice_gauge_text("passive wait blocked", 30);
        let mut terminal = Terminal::new(TestBackend::new(140, 2)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_widget(
                    Paragraph::new(vec![activity_line(&receipt), activity_line(&guard)]),
                    frame.area(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(2, 0)].bg, buffer[(2, 1)].bg);
        assert_eq!(buffer[(2, 0)].bg, GAUGE_HOT_FILL.bg.unwrap());
        assert_ne!(buffer[(2, 0)].bg, Color::Reset);
        println!(
            "receipt TestBackend: saturated receipt and guard background match {:?}",
            buffer[(2, 0)].bg
        );
    }

    #[test]
    fn gauge_rows_fill_progressively_and_keep_height_stable() {
        let note = "passive wait blocked: status/sleep calls were not started";
        let low = crate::turn_event_view::notice_gauge_text(note, 3);
        let hot = crate::turn_event_view::notice_gauge_text(note, 17);
        for (text, fill) in [(&low, GAUGE_LOW_FILL), (&hot, GAUGE_HOT_FILL)] {
            let message = Message {
                role: Role::Activity,
                text: text.as_str().into(),
            };
            let mut renders = Vec::new();
            let rendered = lines(std::slice::from_ref(&message), "", false, 120, &mut renders);
            assert_eq!(rendered.len(), 1);
            let filled = &rendered[0].spans[1];
            assert_eq!(filled.style, fill, "leading span carries the fill: {text}");
            // The chip is the styled count at the end of the row.
            let chip = rendered[0].spans.last().unwrap();
            assert!(chip.style.add_modifier.contains(Modifier::BOLD));
            // Same text bytes in, same text bytes out — wrap height can't drift.
            let joined: String = rendered[0]
                .spans
                .iter()
                .skip(1)
                .map(|span| span.content.as_ref())
                .collect();
            assert_eq!(&joined, text);
        }
        // Low repeat fills part of the row; saturation covers the whole body.
        let low_line = activity_line(&low);
        assert!(
            low_line
                .spans
                .iter()
                .any(|span| span.style == ACTIVITY_STYLE && !span.content.trim().is_empty()),
            "a low count leaves unfilled body text"
        );
        let hot_line = activity_line(&hot);
        assert!(
            !hot_line
                .spans
                .iter()
                .any(|span| span.style == ACTIVITY_STYLE && span.content.len() > 3),
            "a saturated gauge fills the entire body"
        );
        // Plain notices render exactly as before.
        let plain = activity_line(".. context note");
        assert_eq!(plain.spans[1].style, ACTIVITY_STYLE);
    }

    #[test]
    fn empty_message_still_emits_a_tag_line() {
        // Empty text with a tag → a single line carrying just the tag (the
        // start_len == len fallback in append_plain_message_lines).
        let (tag, style) = message_tag(Role::User);
        let rendered = plain_message_lines("", tag, style, None);
        assert_eq!(rendered.len(), 1);
        assert_eq!(rendered[0].spans.len(), 1);
        assert_eq!(rendered[0].spans[0].content.as_ref(), "you ");
        // plain_line_capacity reserves 1 for empty text.
        assert_eq!(plain_line_capacity(""), 1);
    }

    #[test]
    fn empty_message_with_no_tag_emits_a_blank_line() {
        // The Activity tag is empty: empty text → a bare blank line, no tag span.
        let (tag, style) = message_tag(Role::Activity);
        assert_eq!(tag, "");
        let rendered = plain_message_lines("", tag, style, None);
        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].spans.is_empty(), "blank line has no spans");
    }

    #[test]
    fn markdown_agent_text_still_renders_markdown() {
        let messages = [Message {
            role: Role::Angel,
            text: "**bold**".into(),
        }];
        let mut renders = Vec::new();
        let rendered = lines(&messages, "", false, 80, &mut renders);
        let text = plain_text(&rendered);
        assert!(text.contains("angel "));
        assert!(text.contains("bold"));
        // The markdown body borrows from the cached render — a deep clone per
        // visible message per frame is the regression this guards against.
        assert!(matches!(
            rendered[0]
                .spans
                .iter()
                .find(|span| span.content == "bold")
                .unwrap()
                .content,
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn latex_only_agent_text_uses_the_markdown_math_renderer() {
        let messages = [Message {
            role: Role::Angel,
            text: r"$E = mc^2$".into(),
        }];
        let mut renders = Vec::new();
        let rendered = lines(&messages, "", false, 80, &mut renders);
        let text = plain_text(&rendered);
        assert!(text.contains("E = mc²"), "{text}");
        assert!(!text.contains('$'), "{text}");
    }
}
