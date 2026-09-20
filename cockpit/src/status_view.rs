use crate::club::TokenUsage;
use crate::glyphs::Glyph;
use crate::hud::{HUD_BLUE, HUD_DANGER, HUD_DIM, HUD_GOLD};
#[cfg(test)]
use crate::overwatch::OverwatchSnapshot;
use crate::swarm::ledger::MoaTokenReport;
use dotmax::BrailleGrid;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[cfg(test)]
const MESSAGE_TITLE_CACHE_LIMIT: usize = 16;
const REASONING_TITLE_CACHE_LIMIT: usize = 16;
const HUD_BLUE_STYLE: Style = Style::new().fg(HUD_BLUE);
const HUD_DIM_STYLE: Style = Style::new().fg(HUD_DIM);
const HUD_TEXT_STYLE: Style = Style::new().fg(crate::hud::HUD_TEXT);
const HUD_BLUE_BOLD_STYLE: Style = Style::new().fg(HUD_BLUE).add_modifier(Modifier::BOLD);
const HUD_GOLD_BOLD_STYLE: Style = Style::new().fg(HUD_GOLD).add_modifier(Modifier::BOLD);
const HUD_DANGER_BOLD_STYLE: Style = Style::new().fg(HUD_DANGER).add_modifier(Modifier::BOLD);
const COMPOSER_SELECTION_STYLE: Style = Style::new()
    .fg(Color::Black)
    .bg(HUD_BLUE)
    .add_modifier(Modifier::BOLD);
#[cfg_attr(not(test), allow(dead_code))]
const YELLOW_STYLE: Style = Style::new().fg(Color::Yellow);

#[cfg_attr(not(test), allow(dead_code))]
pub fn header_line(label: &str, state: Glyph, cpu_pct: f32, gpu_pct: f32) -> Line<'_> {
    if state == Glyph::Idle
        && cpu_pct == 0.0
        && gpu_pct == 0.0
        && let Some(line) = fixed_idle_header_line(label)
    {
        return line.clone();
    }
    let active = state == Glyph::Thinking || state == Glyph::Active;
    Line::from(vec![
        Span::styled("angel0 ", HUD_BLUE_BOLD_STYLE),
        Span::raw(label),
        Span::raw(" "),
        Span::styled(
            state.token(),
            if active { YELLOW_STYLE } else { HUD_DIM_STYLE },
        ),
        Span::raw(" "),
        Span::raw(state.label()),
        Span::raw(compact_load_suffix(cpu_pct, gpu_pct)),
    ])
}

#[cfg_attr(not(test), allow(dead_code))]
fn fixed_idle_header_line(label: &str) -> Option<&'static Line<'static>> {
    fn cached(
        slot: &'static OnceLock<Line<'static>>,
        label: &'static str,
    ) -> &'static Line<'static> {
        slot.get_or_init(|| {
            Line::from(vec![
                Span::styled("angel0 ", HUD_BLUE_BOLD_STYLE),
                Span::raw(label),
                Span::raw(" "),
                Span::styled(Glyph::Idle.token(), HUD_DIM_STYLE),
                Span::raw(" "),
                Span::raw(Glyph::Idle.label()),
                Span::raw(compact_load_suffix(0.0, 0.0)),
            ])
        })
    }

    static PRACTICE: OnceLock<Line<'static>> = OnceLock::new();
    static SPARK: OnceLock<Line<'static>> = OnceLock::new();
    static SPARK_R1: OnceLock<Line<'static>> = OnceLock::new();
    static TURBO: OnceLock<Line<'static>> = OnceLock::new();
    static ATLAS: OnceLock<Line<'static>> = OnceLock::new();
    static GEMMA: OnceLock<Line<'static>> = OnceLock::new();
    static SWARM: OnceLock<Line<'static>> = OnceLock::new();
    static DELI: OnceLock<Line<'static>> = OnceLock::new();
    static CODEX: OnceLock<Line<'static>> = OnceLock::new();
    static MATH_GOD: OnceLock<Line<'static>> = OnceLock::new();
    static GPU_COMP: OnceLock<Line<'static>> = OnceLock::new();

    match label {
        "practice" | "Practice" => Some(cached(&PRACTICE, "Practice")),
        "spark" | "Sparky" => Some(cached(&SPARK, "Sparky")),
        "spark-r1" => Some(cached(&SPARK_R1, "spark-r1")),
        "turbo" | "Turbo" => Some(cached(&TURBO, "Turbo")),
        "atlas" | "Atlas" => Some(cached(&ATLAS, "Atlas")),
        "gemma" => Some(cached(&GEMMA, "gemma")),
        "moa" => Some(cached(&SWARM, "moa")),
        "deli" => Some(cached(&DELI, "deli")),
        "Codex" => Some(cached(&CODEX, "Codex")),
        "Math God" => Some(cached(&MATH_GOD, "Math God")),
        "GPU Comp" => Some(cached(&GPU_COMP, "GPU Comp")),
        _ => None,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn fmt_clock(secs: f32) -> String {
    let s = secs.max(0.0) as u64;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m:02}:{sec:02}")
    }
}

#[cfg(test)]
#[cfg(test)]
pub fn working_title(club: &str, secs: f32, load_pct: f32) -> Cow<'static, str> {
    let clock = fmt_clock(secs);
    working_title_with_clock(club, &clock, load_pct)
}

#[cfg(test)]
pub fn working_title_with_clock(club: &str, clock: &str, load_pct: f32) -> Cow<'static, str> {
    if clock == "00:00"
        && load_pct == 0.0
        && let Some(title) = fixed_zero_working_title(club)
    {
        return Cow::Borrowed(title);
    }
    Cow::Owned(format!(
        " {} working · {club} · {} · load {:02.0}% ",
        Glyph::Loading.token(),
        clock,
        load_pct
    ))
}

#[cfg(test)]
fn fixed_zero_working_title(label: &str) -> Option<&'static str> {
    fn cached(slot: &'static OnceLock<String>, label: &'static str) -> &'static str {
        slot.get_or_init(|| {
            format!(
                " {} working · {} · 00:00 · load 00% ",
                Glyph::Loading.token(),
                label
            )
        })
        .as_str()
    }

    static PRACTICE: OnceLock<String> = OnceLock::new();
    static SPARK: OnceLock<String> = OnceLock::new();
    static SPARK_R1: OnceLock<String> = OnceLock::new();
    static TURBO: OnceLock<String> = OnceLock::new();
    static ATLAS: OnceLock<String> = OnceLock::new();
    static GEMMA: OnceLock<String> = OnceLock::new();
    static SWARM: OnceLock<String> = OnceLock::new();
    static DELI: OnceLock<String> = OnceLock::new();

    match label {
        "practice" => Some(cached(&PRACTICE, "practice")),
        "spark" => Some(cached(&SPARK, "spark")),
        "spark-r1" => Some(cached(&SPARK_R1, "spark-r1")),
        "turbo" => Some(cached(&TURBO, "turbo")),
        "atlas" => Some(cached(&ATLAS, "atlas")),
        "gemma" => Some(cached(&GEMMA, "gemma")),
        "moa" => Some(cached(&SWARM, "moa")),
        "deli" => Some(cached(&DELI, "deli")),
        _ => None,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn compact_load_suffix(cpu_pct: f32, gpu_pct: f32) -> Cow<'static, str> {
    if cpu_pct == 0.0 && gpu_pct == 0.0 {
        static ZERO_LOAD: OnceLock<String> = OnceLock::new();
        return Cow::Borrowed(
            ZERO_LOAD
                .get_or_init(|| format!("  {} 00% {} 00%", Glyph::Cpu.token(), Glyph::Gpu.token()))
                .as_str(),
        );
    }
    Cow::Owned(format!(
        "  {} {:02.0}% {} {:02.0}%",
        Glyph::Cpu.token(),
        cpu_pct,
        Glyph::Gpu.token(),
        gpu_pct
    ))
}

#[cfg(test)]
pub fn agent_bay_title(
    badge: &str,
    profile_name: &str,
    state: Glyph,
    cpu_pct: f32,
    gpu_pct: f32,
) -> Cow<'static, str> {
    if state == Glyph::Idle
        && cpu_pct == 0.0
        && gpu_pct == 0.0
        && let Some(title) = idle_agent_bay_title(badge, profile_name)
    {
        return Cow::Borrowed(title);
    }

    Cow::Owned(format!(
        " {} agent · {} · {} {} · {} ",
        badge,
        profile_name,
        state.token(),
        state.label(),
        compact_load_suffix(cpu_pct, gpu_pct).trim_start()
    ))
}

#[cfg(test)]
fn idle_agent_bay_title(badge: &str, profile_name: &str) -> Option<&'static str> {
    fn cached(
        slot: &'static OnceLock<String>,
        badge: &str,
        profile_name: &'static str,
    ) -> &'static str {
        slot.get_or_init(|| {
            format!(
                " {} agent · {} · {} idle · {} ",
                badge,
                profile_name,
                Glyph::Idle.token(),
                compact_load_suffix(0.0, 0.0).trim_start()
            )
        })
        .as_str()
    }

    static TURBO: OnceLock<String> = OnceLock::new();
    static ATLAS: OnceLock<String> = OnceLock::new();
    static SPARKY: OnceLock<String> = OnceLock::new();
    static APOLLO: OnceLock<String> = OnceLock::new();
    static AGENT: OnceLock<String> = OnceLock::new();

    match profile_name {
        "Turbo" => Some(cached(&TURBO, badge, "Turbo")),
        "Atlas" => Some(cached(&ATLAS, badge, "Atlas")),
        "Sparky" => Some(cached(&SPARKY, badge, "Sparky")),
        "Apollo" => Some(cached(&APOLLO, badge, "Apollo")),
        "Agent" if badge == Glyph::Agent.mark() => Some(cached(&AGENT, badge, "Agent")),
        _ => None,
    }
}

pub fn agent_metrics_line(
    cpu_pct: f32,
    mem_pct: f32,
    gpu_pct: f32,
    active: bool,
) -> Cow<'static, str> {
    if cpu_pct == 0.0 && mem_pct == 0.0 && gpu_pct == 0.0 {
        static IDLE_METRICS: OnceLock<String> = OnceLock::new();
        static ACTIVE_METRICS: OnceLock<String> = OnceLock::new();
        let line = if active {
            ACTIVE_METRICS.get_or_init(|| {
                format!(
                    "{}00% {}00% {}00% {} active",
                    Glyph::Cpu.token(),
                    Glyph::Memory.token(),
                    Glyph::Gpu.token(),
                    Glyph::Active.token()
                )
            })
        } else {
            IDLE_METRICS.get_or_init(|| {
                format!(
                    "{}00% {}00% {}00% {} idle",
                    Glyph::Cpu.token(),
                    Glyph::Memory.token(),
                    Glyph::Gpu.token(),
                    Glyph::Idle.token()
                )
            })
        };
        return Cow::Borrowed(line.as_str());
    }
    let state = if active { Glyph::Active } else { Glyph::Idle };
    Cow::Owned(format!(
        "{}{:02.0}% {}{:02.0}% {}{:02.0}% {} {}",
        Glyph::Cpu.token(),
        cpu_pct,
        Glyph::Memory.token(),
        mem_pct,
        Glyph::Gpu.token(),
        gpu_pct,
        state.token(),
        state.label()
    ))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn active_route_label(tabs: &[crate::club::ClubTab]) -> Option<String> {
    let tab = tabs.iter().find(|t| t.in_hand)?;
    // Prefer the live mode/checkpoint. Prefixing the machine slug (`spark·swarm`)
    // made the bay title read like two internal nicknames.
    let mut label = tab
        .mode
        .clone()
        .filter(|mode| !mode.eq_ignore_ascii_case(&tab.label))
        .unwrap_or_else(|| tab.label.clone());
    if !tab.available {
        label.push(' ');
        label.push_str(Glyph::Offline.token());
    }
    Some(label)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn agent_route_title(
    tabs: &[crate::club::ClubTab],
    clock: Option<&str>,
    load_pct: Option<f32>,
    max_chars: usize,
) -> Option<String> {
    if max_chars < 8 {
        return None;
    }
    let route = active_route_label(tabs)?;
    let mut variants = Vec::with_capacity(3);
    if let (Some(clock), Some(load_pct)) = (clock, load_pct) {
        variants.push(format!("{route} · {clock} · load {:02.0}%", load_pct));
    }
    if let Some(clock) = clock {
        variants.push(format!("{route} · {clock}"));
    }
    variants.push(route);

    let body_max = max_chars.saturating_sub(2);
    let mut body = variants
        .into_iter()
        .find(|candidate| candidate.chars().count() <= body_max)
        .unwrap_or_else(|| active_route_label(tabs).unwrap_or_default());
    if body.chars().count() > body_max {
        let take = body_max.saturating_sub(1);
        body = body.chars().take(take).collect::<String>();
        body.push('…');
    }
    Some(format!(" {body} "))
}

/// Visual/interaction intent for the composer. This is deliberately derived
/// from the draft rather than stored in [`App`](crate::App), so paste, vim
/// edits, and restored drafts cannot leave the chrome in a stale state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComposerIntent {
    Empty,
    Message,
    Command,
    CriticalCommand,
}

impl ComposerIntent {
    pub fn title(self, max_chars: usize) -> &'static str {
        let (full, compact) = match self {
            Self::Empty => ("", ""),
            Self::Message => ("", ""),
            Self::Command => (" A> command ", " A> command "),
            Self::CriticalCommand => (" A> review command ", " A> review command "),
        };
        if full.chars().count() <= max_chars {
            full
        } else {
            compact
        }
    }
}

/// Classify only commands whose effect is easy to trigger and meaningful to
/// reverse. Ordinary prose containing words such as "delete" remains a normal
/// message; the warning rail is command-aware, not a keyword alarm.
pub fn composer_intent(input: &str) -> ComposerIntent {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return ComposerIntent::Empty;
    }
    // Classify from the first tokens only. Lowercasing the whole draft on every
    // composer frame allocated O(input) while typing a long message.
    // Only short command tokens matter. Bound unbroken megabyte tokens without
    // truncating arbitrarily long whitespace between /self and discard.
    fn token_prefix(text: &str) -> &str {
        let mut end = text.len().min(16);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text[..end].split_whitespace().next().unwrap_or("")
    }
    let head = token_prefix(trimmed);
    let tail = if head.eq_ignore_ascii_case("/self") || head.eq_ignore_ascii_case("/loop") {
        token_prefix(trimmed[head.len()..].trim_start())
    } else {
        ""
    };
    let critical = head.eq_ignore_ascii_case("exit")
        || head.eq_ignore_ascii_case("quit")
        || head.eq_ignore_ascii_case("/exit")
        || head.eq_ignore_ascii_case("/quit")
        || head.eq_ignore_ascii_case("/new")
        || head.eq_ignore_ascii_case("/clear")
        || head.eq_ignore_ascii_case("/delete")
        || head.eq_ignore_ascii_case("/logout")
        || (head.eq_ignore_ascii_case("/self") && tail.eq_ignore_ascii_case("discard"))
        || (head.eq_ignore_ascii_case("/loop") && tail.eq_ignore_ascii_case("stop"));
    if critical {
        ComposerIntent::CriticalCommand
    } else if head.starts_with('/') {
        ComposerIntent::Command
    } else {
        ComposerIntent::Message
    }
}

/// Composer box title. Idle/draft/command chrome is a static borrow; vim,
/// selection, steer-queue, and MoA chips allocate only when they actually
/// extend the title.
#[derive(Default)]
pub struct ComposerTitleState<'a> {
    pub busy: bool,
    pub steerable: bool,
    pub vim_normal: Option<bool>,
    pub selection_len: Option<usize>,
    pub steer_queued: usize,
    pub moa_chip: Option<&'a str>,
    /// Staged Ctrl-V screenshot chip (`None` when nothing is attached).
    pub attachment_chip: Option<&'a str>,
}

pub fn composer_frame_title(
    intent: ComposerIntent,
    max_chars: usize,
    state: ComposerTitleState<'_>,
) -> Cow<'static, str> {
    let ComposerTitleState {
        busy,
        steerable,
        vim_normal,
        selection_len,
        steer_queued,
        moa_chip,
        attachment_chip,
    } = state;
    let mut title: Cow<'static, str> = if busy && steerable {
        Cow::Borrowed(" A> guidance ")
    } else {
        Cow::Borrowed(intent.title(max_chars))
    };
    if let Some(normal) = vim_normal {
        title
            .to_mut()
            .push_str(if normal { "[N] " } else { "[I] " });
    }
    if let Some(len) = selection_len {
        title.to_mut().push_str(&format!("[select {len}] "));
    }
    if steerable && let Some(hint) = composer_steer_hint(busy, steer_queued) {
        title.to_mut().push_str(&hint);
    }
    if let Some(chip) = moa_chip {
        title.to_mut().push_str(chip);
    }
    if let Some(chip) = attachment_chip {
        title.to_mut().push_str(chip);
    }
    title
}

/// Right-hand border chip on the composer: what Enter does right now.
#[allow(dead_code)]
pub fn composer_enter_hint(intent: ComposerIntent, busy: bool, steerable: bool) -> &'static str {
    // `⏎` rather than a literal "[Enter]": the Scryglass verb rail owns that
    // token and frame assertions read it as the world's enter button.
    if busy && steerable {
        " ⏎ Steer "
    } else {
        match intent {
            ComposerIntent::Empty | ComposerIntent::Message => " ⏎ Send ",
            ComposerIntent::Command | ComposerIntent::CriticalCommand => " ⏎ Run ",
        }
    }
}

pub fn composer_placeholder(busy: bool, inner_width: usize) -> Line<'static> {
    let text = if busy {
        "Add guidance… Enter queues it"
    } else {
        "Write a message…"
    };
    // Pad to the full inner width. `panel_style` only sets fg (no bg), so a short
    // line leaves right-side cells untouched — a previous longer draft then
    // ghosts as bright leftover text beside the dim placeholder (busy-steer
    // mess: "Add guidance… Enter queues it" + stale HTTP error tail).
    // A3: truncate by display width (not char count) so wide-glyph placeholders
    // cannot under-fill the row before pad rewrites residual cells.
    Line::from(Span::styled(
        pad_composer_row(text, inner_width),
        HUD_DIM_STYLE,
    ))
}

/// True when a scalar must be rewritten before pad/wrap measure (A3 ghosts).
/// Controls (TAB) expand in the terminal; several format/ignorable characters
/// report unicode-width 0 while some terminals still reserve a cell (or paint a
/// dotted replacement) — counting them as 0 under-pads and leaves residual
/// neighbor cells. ZWJ / variation selectors stay intact for emoji clusters.
fn composer_char_needs_sanitize(ch: char) -> bool {
    if ch.is_control() {
        return true;
    }
    matches!(
        ch,
        '\u{00AD}' // soft hyphen
            | '\u{200B}' // ZERO WIDTH SPACE
            // A3: ZWNJ + bidi marks/embeddings report width 0 (like ZWSP) but
            // paste from rich editors / chat often injects them; under-padding
            // left residual neighbor cells after the draft cleared.
            | '\u{200C}' // ZERO WIDTH NON-JOINER
            | '\u{200E}' // LEFT-TO-RIGHT MARK
            | '\u{200F}' // RIGHT-TO-LEFT MARK
            | '\u{202A}' // LEFT-TO-RIGHT EMBEDDING
            | '\u{202B}' // RIGHT-TO-LEFT EMBEDDING
            | '\u{202C}' // POP DIRECTIONAL FORMATTING
            | '\u{202D}' // LEFT-TO-RIGHT OVERRIDE
            | '\u{202E}' // RIGHT-TO-LEFT OVERRIDE
            | '\u{2060}' // word joiner
            | '\u{2066}' // LEFT-TO-RIGHT ISOLATE
            | '\u{2067}' // RIGHT-TO-LEFT ISOLATE
            | '\u{2068}' // FIRST STRONG ISOLATE
            | '\u{2069}' // POP DIRECTIONAL ISOLATE
            | '\u{FEFF}' // BOM / ZWNBSP
            | '\u{180E}' // mongolian vowel separator
            // Unicode hard line breaks: wrap treats them as newlines; compact /
            // pad paths that do not re-wrap still must not emit a terminal
            // line-break scalar (A3 ghosts on the next painted row).
            | '\u{2028}' // LINE SEPARATOR
            | '\u{2029}' // PARAGRAPH SEPARATOR
    )
}

/// Explicit line breaks in the draft: LF plus Unicode LS/PS (paste from rich
/// editors). Treated like `\n` for wrap so the terminal cannot interpret a
/// mid-row LS/PS as an extra physical line under a short pad (A3 ghosts).
fn is_composer_hard_break(ch: char) -> bool {
    ch == '\n' || ch == '\u{2028}' || ch == '\u{2029}'
}

/// Control/format characters paint as a single space so pad width matches the
/// wrap budget and the terminal cannot expand them past the pad into residual
/// neighbor cells (A3 composer ghosts).
fn sanitize_composer_display_char(ch: char) -> char {
    if composer_char_needs_sanitize(ch) {
        ' '
    } else {
        ch
    }
}

fn sanitize_composer_display(text: &str) -> String {
    text.chars().map(sanitize_composer_display_char).collect()
}

#[cfg(test)]
fn composer_char_display_width(ch: char) -> usize {
    let ch = sanitize_composer_display_char(ch);
    unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1)
}

/// Display-cell width of `text` (terminal columns), not Unicode scalar count.
/// Wide glyphs (emoji, CJK) occupy 2 cells; counting chars under-padded rows and
/// left bright residual cells after a draft cleared (A3 composer ghosts).
/// Tabs/controls/format-ZW count as 1 (space) so pad never under-fills vs wrap.
fn composer_display_width(text: &str) -> usize {
    text.graphemes(true).map(UnicodeWidthStr::width).sum()
}

/// Truncate `text` to at most `width` terminal columns, padding with spaces if a
/// trailing wide glyph would leave a short row. Controls become spaces.
fn fit_display_width(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::with_capacity(text.len().min(width * 4));
    let mut used = 0usize;
    let text = sanitize_composer_display(text);
    for grapheme in text.graphemes(true) {
        let w = grapheme.width();
        if used.saturating_add(w) > width {
            break;
        }
        out.push_str(grapheme);
        used = used.saturating_add(w);
    }
    if used < width {
        out.extend(std::iter::repeat_n(' ', width - used));
    }
    out
}

/// Extend a composer row to `width` **display cells** so residual glyphs from a
/// prior frame cannot survive under empty trailing cells.
pub(crate) fn pad_composer_row(text: impl AsRef<str>, width: usize) -> String {
    let text = sanitize_composer_display(text.as_ref());
    if width == 0 {
        return String::new();
    }
    let n = composer_display_width(&text);
    if n == width {
        return text;
    }
    if n > width {
        return fit_display_width(&text, width);
    }
    let mut out = String::with_capacity(text.len() + (width - n));
    out.push_str(&text);
    out.extend(std::iter::repeat_n(' ', width - n));
    out
}

/// Pad every line of a composer view to the pane width (see [`pad_composer_row`]).
pub(crate) fn pad_composer_lines(lines: &mut [Line<'_>], width: usize) {
    if width == 0 {
        return;
    }
    for line in lines.iter_mut() {
        // Rewrite controls/format-ZW to spaces before measuring so pad width and
        // rendered cells stay 1:1 with the terminal.
        for span in line.spans.iter_mut() {
            let raw = span.content.as_ref();
            if raw.chars().any(composer_char_needs_sanitize) {
                let style = span.style;
                *span = Span::styled(sanitize_composer_display(raw), style);
            }
        }
        let used: usize = line
            .spans
            .iter()
            .map(|s| composer_display_width(s.content.as_ref()))
            .sum();
        if used >= width {
            // Truncate overflow rather than leave a wrapping ghost.
            let mut kept = 0usize;
            let mut spans = Vec::with_capacity(line.spans.len());
            for span in line.spans.drain(..) {
                let n = composer_display_width(span.content.as_ref());
                if kept >= width {
                    break;
                }
                if kept + n <= width {
                    kept += n;
                    spans.push(span);
                } else {
                    let take = width - kept;
                    let clipped = fit_display_width(span.content.as_ref(), take);
                    // fit_display_width pads to `take`; keep only the truncated
                    // content portion for this span (no double-pad from later).
                    let clipped_w = composer_display_width(&clipped);
                    if clipped_w > 0 {
                        // Drop trailing pad spaces we just added if the next
                        // logic expects exact width via a final pad — here the
                        // row is already at width after this span.
                        spans.push(Span::styled(clipped, span.style));
                    }
                    break;
                }
            }
            line.spans = spans;
            // Ensure the truncated row still fills every cell (wide-glyph edge).
            let after: usize = line
                .spans
                .iter()
                .map(|s| composer_display_width(s.content.as_ref()))
                .sum();
            if after < width {
                line.spans
                    .push(Span::styled(" ".repeat(width - after), HUD_TEXT_STYLE));
            }
            continue;
        }
        line.spans.push(Span::styled(
            " ".repeat(width - used),
            // Match body text style so the pad is invisible on a dark terminal
            // while still rewriting every cell's symbol (clears ghosts).
            HUD_TEXT_STYLE,
        ));
    }
}

/// Composer-title hint while the flight slot is busy: typing still works — a
/// plain message queues as a mid-run steer (see `steer.rs`). Once one is
/// waiting, the queue depth replaces the hint. `None` when idle (the default
/// title stands alone).
pub fn composer_steer_hint(busy: bool, queued: usize) -> Option<Cow<'static, str>> {
    if queued > 0 {
        Some(Cow::Owned(format!(
            "· {queued} steer{} queued ",
            if queued == 1 { "" } else { "s" }
        )))
    } else if busy {
        Some(Cow::Borrowed("· busy — Enter queues a steer "))
    } else {
        None
    }
}

pub fn token_usage_meter(usage: Option<TokenUsage>) -> String {
    static CACHE: std::sync::Mutex<Option<(Option<TokenUsage>, String)>> =
        std::sync::Mutex::new(None);
    if let Ok(guard) = CACHE.lock()
        && let Some((cached, text)) = guard.as_ref()
        && *cached == usage
    {
        return text.clone();
    }
    let text = match usage {
        Some(u) => {
            let mut out = format!(
                "tok in {}/{} · out {}/{}",
                u.last_input, u.total_input, u.last_output, u.total_output
            );
            if u.last_reasoning > 0 || u.total_reasoning > 0 {
                out.push_str(&format!(
                    " · rsn {}/{}",
                    u.last_reasoning, u.total_reasoning
                ));
            }
            out
        }
        None => "tok in --/-- · out --/--".to_string(),
    };
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((usage, text.clone()));
    }
    text
}

pub fn moa_token_report_lines(
    report: &MoaTokenReport,
    width: u16,
    max_lines: usize,
) -> Vec<Line<'static>> {
    if width < 18 || max_lines == 0 {
        return Vec::new();
    }
    let total_last: u64 = report.rows.iter().map(|r| r.last_total()).sum();
    let total_session: u64 = report.rows.iter().map(|r| r.session_total()).sum();
    let status = if report.latest_ok { "ok" } else { "err" };
    let header = fit_ascii(
        &format!(
            "moa tok {} · total {} · {} {status}",
            kfmt(total_last),
            kfmt(total_session),
            report.latest_route
        ),
        width as usize,
    );
    let mut lines = vec![Line::from(vec![Span::styled(header, HUD_BLUE_STYLE)])];
    if max_lines == 1 {
        return lines;
    }

    let max_last = report
        .rows
        .iter()
        .map(|r| r.last_total())
        .max()
        .unwrap_or(0);
    let max_session = report
        .rows
        .iter()
        .map(|r| r.session_total())
        .max()
        .unwrap_or(0);
    let denom = max_last.max(max_session).max(1);
    let row_limit = max_lines.saturating_sub(1);
    for row in report.rows.iter().take(row_limit) {
        lines.push(moa_token_row_line(row, denom, width as usize));
    }
    if report.rows.len() > row_limit && row_limit > 0 {
        lines.push(Line::from(Span::styled(
            format!("+{} more model(s)", report.rows.len() - row_limit),
            HUD_DIM_STYLE,
        )));
    }
    lines.truncate(max_lines);
    lines
}

fn moa_token_row_line(
    row: &crate::swarm::ledger::MoaModelTokenRow,
    denom: u64,
    width: usize,
) -> Line<'static> {
    let label_w = if width >= 52 {
        12
    } else if width >= 38 {
        10
    } else {
        8
    };
    let label = fit_ascii(&row.label, label_w);
    let counts = if width >= 44 {
        format!(
            " {}/{} Σ{} ",
            kfmt(row.last_input),
            kfmt(row.last_output),
            kfmt(row.session_total())
        )
    } else {
        format!(
            " {} Σ{} ",
            kfmt(row.last_total()),
            kfmt(row.session_total())
        )
    };
    let fixed = label_w + counts.chars().count();
    let bar_w = width.saturating_sub(fixed).clamp(4, 18);
    let frac = row.last_total() as f32 / denom.max(1) as f32;
    Line::from(vec![
        Span::styled(label, HUD_BLUE_STYLE),
        Span::raw(counts),
        Span::styled(dotmax_meter_bar(frac, bar_w), Style::new().fg(Color::Cyan)),
    ])
}

fn dotmax_meter_bar(progress: f32, width: usize) -> String {
    let width = width.max(1);
    let mut grid = match BrailleGrid::new(width, 1) {
        Ok(g) => g,
        Err(_) => return "░".repeat(width),
    };
    let filled = (progress.clamp(0.0, 1.0) * (width * 2) as f32).round() as usize;
    for x in 0..filled.min(width * 2) {
        for y in 0..4 {
            let _ = grid.set_dot(x, y);
        }
    }
    (0..width)
        .map(|x| {
            let ch = grid.get_char(x, 0);
            if ch == '\u{2800}' || ch == ' ' {
                '░'
            } else {
                ch
            }
        })
        .collect()
}

fn fit_ascii(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out = s.chars().take(max.saturating_sub(1)).collect::<String>();
    out.push('…');
    out
}

fn kfmt(n: u64) -> String {
    if n >= 10_000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
#[cfg(test)]
pub fn working_lines(club: &str, secs: f32, snapshot: OverwatchSnapshot) -> Vec<Line<'_>> {
    if secs < 1.0
        && snapshot.cpu_pct == 0.0
        && snapshot.gpu_pct == 0.0
        && let Some(lines) = fixed_zero_working_lines(club)
    {
        return lines.to_vec();
    }
    vec![
        Line::from(vec![
            Span::styled(Glyph::Agent.token(), HUD_BLUE_STYLE),
            Span::styled(" agent ", HUD_BLUE_STYLE),
            Span::raw(club),
            Span::styled("  elapsed ", HUD_BLUE_STYLE),
            Span::raw(fmt_clock(secs)),
            Span::styled("  ", HUD_BLUE_STYLE),
            Span::styled(Glyph::Cpu.token(), HUD_BLUE_STYLE),
            Span::styled(" ", HUD_BLUE_STYLE),
            Span::raw(format!("{:04.1}%", snapshot.cpu_pct)),
            Span::styled("  ", HUD_BLUE_STYLE),
            Span::styled(Glyph::Gpu.token(), HUD_BLUE_STYLE),
            Span::styled(" ", HUD_BLUE_STYLE),
            Span::raw(format!("{:04.1}%", snapshot.gpu_pct)),
        ]),
        Line::from(Span::styled(
            "response pending; transcript will update as chunks arrive",
            HUD_DIM_STYLE,
        )),
    ]
}

#[cfg(test)]
pub fn working_lines_with_clock<'a>(
    club: &'a str,
    clock: &'a str,
    snapshot: OverwatchSnapshot,
) -> Vec<Line<'a>> {
    if clock == "00:00"
        && snapshot.cpu_pct == 0.0
        && snapshot.gpu_pct == 0.0
        && let Some(lines) = fixed_zero_working_lines(club)
    {
        return lines.to_vec();
    }
    vec![
        Line::from(vec![
            Span::styled(Glyph::Agent.token(), HUD_BLUE_STYLE),
            Span::styled(" agent ", HUD_BLUE_STYLE),
            Span::raw(club),
            Span::styled("  elapsed ", HUD_BLUE_STYLE),
            Span::raw(clock),
            Span::styled("  ", HUD_BLUE_STYLE),
            Span::styled(Glyph::Cpu.token(), HUD_BLUE_STYLE),
            Span::styled(" ", HUD_BLUE_STYLE),
            Span::raw(format!("{:04.1}%", snapshot.cpu_pct)),
            Span::styled("  ", HUD_BLUE_STYLE),
            Span::styled(Glyph::Gpu.token(), HUD_BLUE_STYLE),
            Span::styled(" ", HUD_BLUE_STYLE),
            Span::raw(format!("{:04.1}%", snapshot.gpu_pct)),
        ]),
        Line::from(Span::styled(
            "response pending; transcript will update as chunks arrive",
            HUD_DIM_STYLE,
        )),
    ]
}

#[cfg(test)]
fn fixed_zero_working_lines(label: &str) -> Option<&'static [Line<'static>; 2]> {
    fn cached(
        slot: &'static OnceLock<[Line<'static>; 2]>,
        label: &'static str,
    ) -> &'static [Line<'static>; 2] {
        slot.get_or_init(|| {
            [
                Line::from(vec![
                    Span::styled(Glyph::Agent.token(), HUD_BLUE_STYLE),
                    Span::styled(" agent ", HUD_BLUE_STYLE),
                    Span::raw(label),
                    Span::styled("  elapsed ", HUD_BLUE_STYLE),
                    Span::raw("00:00"),
                    Span::styled("  ", HUD_BLUE_STYLE),
                    Span::styled(Glyph::Cpu.token(), HUD_BLUE_STYLE),
                    Span::styled(" ", HUD_BLUE_STYLE),
                    Span::raw("00.0%"),
                    Span::styled("  ", HUD_BLUE_STYLE),
                    Span::styled(Glyph::Gpu.token(), HUD_BLUE_STYLE),
                    Span::styled(" ", HUD_BLUE_STYLE),
                    Span::raw("00.0%"),
                ]),
                Line::from(Span::styled(
                    "response pending; transcript will update as chunks arrive",
                    HUD_DIM_STYLE,
                )),
            ]
        })
    }

    static PRACTICE: OnceLock<[Line<'static>; 2]> = OnceLock::new();
    static SPARK: OnceLock<[Line<'static>; 2]> = OnceLock::new();
    static SPARK_R1: OnceLock<[Line<'static>; 2]> = OnceLock::new();
    static TURBO: OnceLock<[Line<'static>; 2]> = OnceLock::new();
    static ATLAS: OnceLock<[Line<'static>; 2]> = OnceLock::new();
    static GEMMA: OnceLock<[Line<'static>; 2]> = OnceLock::new();
    static SWARM: OnceLock<[Line<'static>; 2]> = OnceLock::new();
    static DELI: OnceLock<[Line<'static>; 2]> = OnceLock::new();

    match label {
        "practice" => Some(cached(&PRACTICE, "practice")),
        "spark" => Some(cached(&SPARK, "spark")),
        "spark-r1" => Some(cached(&SPARK_R1, "spark-r1")),
        "turbo" => Some(cached(&TURBO, "turbo")),
        "atlas" => Some(cached(&ATLAS, "atlas")),
        "gemma" => Some(cached(&GEMMA, "gemma")),
        "moa" => Some(cached(&SWARM, "moa")),
        "deli" => Some(cached(&DELI, "deli")),
        _ => None,
    }
}

pub fn shell_hint() -> &'static str {
    static SHELL_HINT: OnceLock<String> = OnceLock::new();
    SHELL_HINT
        .get_or_init(|| {
            format!(
                "{} shell focused - keys go to the shell · ^G to return",
                Glyph::Shell.token()
            )
        })
        .as_str()
}

pub fn shell_focus_title() -> &'static str {
    static SHELL_FOCUS_TITLE: OnceLock<String> = OnceLock::new();
    SHELL_FOCUS_TITLE
        .get_or_init(|| format!(" {} shell · ^G to return to chat ", Glyph::Shell.token()))
        .as_str()
}

pub fn agent_shell_title() -> &'static str {
    static AGENT_SHELL_TITLE: OnceLock<String> = OnceLock::new();
    AGENT_SHELL_TITLE
        .get_or_init(|| format!(" {} agent shell · modular harness ", Glyph::Shell.token()))
        .as_str()
}

pub fn image_viewer_title() -> &'static str {
    static IMAGE_VIEWER_TITLE: OnceLock<String> = OnceLock::new();
    IMAGE_VIEWER_TITLE
        .get_or_init(|| {
            format!(
                " {} · /hide to return ",
                crate::glyphs::media_prefix("image").trim()
            )
        })
        .as_str()
}

/// The input-pane title as a live tab strip of reachable **boxes** (agents). The
/// in-hand box is bracketed with its active mode inline (`[spark·moa]`); the
/// rest are listed plain by box name. A box that just went offline while in hand
/// keeps its slot but carries the offline mark. Offline boxes other than the
/// in-hand one never appear. When the in-hand box has more than one mode, the
/// `←/→` hint is shown so the subcontrol is discoverable.
#[cfg(test)]
pub fn message_title_tabs(tabs: &[crate::club::ClubTab]) -> Cow<'static, str> {
    let Some(in_hand) = tabs.iter().find(|t| t.in_hand) else {
        return Cow::Borrowed(" A> message · no live model ");
    };
    // A lone reachable box with no extra mode is just the cached single-label title.
    if tabs.len() == 1 && in_hand.available && in_hand.mode.is_none() {
        return message_title(&in_hand.label);
    }
    let roster = tabs
        .iter()
        .map(|t| match (t.in_hand, &t.mode) {
            // In-hand box: bracket it, show its active mode, flag if it dropped.
            (true, mode) => {
                let body = match mode {
                    Some(m) => format!("{}·{m}", t.label),
                    None => t.label.clone(),
                };
                if t.available {
                    format!("[{body}]")
                } else {
                    format!("[{body} {}]", Glyph::Offline.token())
                }
            }
            // Other boxes: just the machine name.
            (false, _) => t.label.clone(),
        })
        .collect::<Vec<_>>()
        .join(" ");
    let switch = Glyph::Switch.token();
    let hint = if in_hand.mode.is_some() {
        format!("Tab={switch} box · ←→ mode")
    } else {
        format!("Tab={switch} switch box")
    };
    Cow::Owned(format!(
        " {} message · {roster} · {hint} ",
        Glyph::Agent.token()
    ))
}

#[cfg(test)]
pub fn message_title(label: &str) -> Cow<'static, str> {
    if let Some(title) = fixed_message_title(label) {
        return Cow::Borrowed(title);
    }

    static MESSAGE_TITLES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let cache = MESSAGE_TITLES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(title) = cache.get(label)
    {
        return Cow::Owned(title.clone());
    }

    let title = format!(
        " {} message · [{label}] · Tab={} switch agent ",
        Glyph::Agent.token(),
        Glyph::Switch.token()
    );
    if let Ok(mut cache) = cache.lock() {
        if cache.len() >= MESSAGE_TITLE_CACHE_LIMIT
            && let Some(oldest) = cache.keys().next().cloned()
        {
            cache.remove(&oldest);
        }
        cache.insert(label.to_string(), title.clone());
    }
    Cow::Owned(title)
}

pub fn reasoning_title(label: &str) -> Cow<'static, str> {
    if let Some(title) = fixed_reasoning_title(label) {
        return Cow::Borrowed(title);
    }

    static REASONING_TITLES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let cache = REASONING_TITLES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(title) = cache.get(label)
    {
        return Cow::Owned(title.clone());
    }

    let title = format!(" {} {} thinking ", label, Glyph::Thinking.token());
    if let Ok(mut cache) = cache.lock() {
        if cache.len() >= REASONING_TITLE_CACHE_LIMIT
            && let Some(oldest) = cache.keys().next().cloned()
        {
            cache.remove(&oldest);
        }
        cache.insert(label.to_string(), title.clone());
    }
    Cow::Owned(title)
}

#[cfg(test)]
fn fixed_message_title(label: &str) -> Option<&'static str> {
    fn cached(slot: &'static OnceLock<String>, label: &'static str) -> &'static str {
        slot.get_or_init(|| {
            format!(
                " {} message · [{}] · Tab={} switch agent ",
                Glyph::Agent.token(),
                label,
                Glyph::Switch.token()
            )
        })
        .as_str()
    }

    static PRACTICE: OnceLock<String> = OnceLock::new();
    static SPARK: OnceLock<String> = OnceLock::new();
    static SPARK_R1: OnceLock<String> = OnceLock::new();
    static TURBO: OnceLock<String> = OnceLock::new();
    static ATLAS: OnceLock<String> = OnceLock::new();
    static GEMMA: OnceLock<String> = OnceLock::new();
    static SWARM: OnceLock<String> = OnceLock::new();
    static DELI: OnceLock<String> = OnceLock::new();

    match label {
        "practice" => Some(cached(&PRACTICE, "practice")),
        "spark" => Some(cached(&SPARK, "spark")),
        "spark-r1" => Some(cached(&SPARK_R1, "spark-r1")),
        "turbo" => Some(cached(&TURBO, "turbo")),
        "atlas" => Some(cached(&ATLAS, "atlas")),
        "gemma" => Some(cached(&GEMMA, "gemma")),
        "moa" => Some(cached(&SWARM, "moa")),
        "deli" => Some(cached(&DELI, "deli")),
        _ => None,
    }
}

fn fixed_reasoning_title(label: &str) -> Option<&'static str> {
    fn cached(slot: &'static OnceLock<String>, label: &'static str) -> &'static str {
        slot.get_or_init(|| format!(" {} {} thinking ", label, Glyph::Thinking.token()))
            .as_str()
    }

    static PRACTICE: OnceLock<String> = OnceLock::new();
    static SPARK: OnceLock<String> = OnceLock::new();
    static SPARK_R1: OnceLock<String> = OnceLock::new();
    static TURBO: OnceLock<String> = OnceLock::new();
    static ATLAS: OnceLock<String> = OnceLock::new();
    static GEMMA: OnceLock<String> = OnceLock::new();
    static SWARM: OnceLock<String> = OnceLock::new();
    static DELI: OnceLock<String> = OnceLock::new();
    static CODEX: OnceLock<String> = OnceLock::new();
    static MATH_GOD: OnceLock<String> = OnceLock::new();
    static GPU_COMP: OnceLock<String> = OnceLock::new();

    match label {
        "practice" | "Practice" => Some(cached(&PRACTICE, "Practice")),
        "spark" | "Sparky" => Some(cached(&SPARK, "Sparky")),
        "spark-r1" => Some(cached(&SPARK_R1, "spark-r1")),
        "turbo" | "Turbo" => Some(cached(&TURBO, "Turbo")),
        "atlas" | "Atlas" => Some(cached(&ATLAS, "Atlas")),
        "gemma" => Some(cached(&GEMMA, "gemma")),
        "moa" => Some(cached(&SWARM, "moa")),
        "deli" => Some(cached(&DELI, "deli")),
        "Codex" => Some(cached(&CODEX, "Codex")),
        "Math God" => Some(cached(&MATH_GOD, "Math God")),
        "GPU Comp" => Some(cached(&GPU_COMP, "GPU Comp")),
        _ => None,
    }
}

pub fn empty_artifacts_line() -> Line<'static> {
    static EMPTY_ARTIFACTS_LINE: OnceLock<Line<'static>> = OnceLock::new();
    EMPTY_ARTIFACTS_LINE
        .get_or_init(|| Line::from(Span::styled("no delivered artifacts", HUD_DIM_STYLE)))
        .clone()
}

pub const fn empty_artifacts_text() -> &'static str {
    "no delivered artifacts"
}

pub struct ComposerView<'a> {
    pub lines: Vec<Line<'a>>,
    pub cursor_col: u16,
    pub cursor_row: u16,
    /// Legacy large-draft indicator: exceeds the original two-row footprint
    /// (or the available rows). Content now wraps/scrolls vertically rather than
    /// being replaced by a horizontal excerpt.
    #[allow(dead_code)] // surfaced to callers; asserted on in unit tests
    pub compacted: bool,
}

/// Render-model for the chat composer. The input buffer itself is untouched:
/// soft wraps and explicit newlines share a cursor-following vertical viewport.
/// No horizontal offset or compact tail replaces the draft, even for large pastes.
#[cfg(test)]
pub fn composer_view(
    input: &str,
    inner_width: usize,
    inner_height: u16,
    cursor: usize,
) -> ComposerView<'static> {
    composer_view_with_selection(input, inner_width, inner_height, cursor, None)
}

pub fn composer_view_with_selection(
    input: &str,
    inner_width: usize,
    inner_height: u16,
    cursor: usize,
    selection: Option<(usize, usize)>,
) -> ComposerView<'static> {
    if inner_width == 0 || inner_height == 0 {
        return ComposerView {
            lines: Vec::new(),
            cursor_col: 0,
            cursor_row: 0,
            compacted: false,
        };
    }
    with_composer_rows(input, inner_width, cursor, |wrapped| {
        let cur = cursor.min(wrapped.char_count);
        let (cursor_row, cursor_col) = wrapped_cursor(&wrapped.rows, cur);
        // Preserve useful large-paste stats without replacing the wrapped text.
        let stats = wrapped.stats.as_ref().filter(|_| inner_height > 1);
        let stats_rows = usize::from(stats.is_some());
        let rows = usize::from(inner_height) - stats_rows;
        let top = cursor_row.saturating_sub(rows - 1);
        let intent = composer_intent(input);
        let mut lines = Vec::with_capacity(usize::from(inner_height));
        if let Some(stats) = stats {
            lines.push(Line::from(Span::styled(
                pad_composer_row(stats, inner_width),
                HUD_DIM_STYLE,
            )));
        }
        lines.extend(wrapped.rows.iter().skip(top).take(rows).map(|row| {
            styled_composer_line_with_selection(&row.text, intent, row.start, selection)
        }));
        let mut view_row = cursor_row - top + stats_rows;
        let mut view_col = cursor_col.min(inner_width.saturating_sub(1));
        // Keep the established insertion-point API when both the full row and
        // its empty successor are visible. draw's caret publisher moves this
        // sentinel to the successor; at the viewport top use (0, 0) instead.
        if cursor_row > top
            && cursor_col == 0
            && wrapped.rows[cursor_row].text.is_empty()
            && wrapped.rows[cursor_row - 1].end == cur
            && composer_display_width(&wrapped.rows[cursor_row - 1].text) == inner_width
        {
            view_row -= 1;
            view_col = inner_width;
        }
        ComposerView {
            lines,
            cursor_col: view_col as u16,
            cursor_row: view_row as u16,
            compacted: wrapped.rows.len() > rows.min(2),
        }
    })
}

/// Two to five content rows plus borders, leaving room above for the transcript.
/// The bottom-anchored layout consumes this height, so growth is upward only.
pub(crate) fn composer_height(input: &str, width: u16, available: u16) -> u16 {
    // Oversized drafts always fill the capped composer; do not lay out a
    // megabyte just to learn that it exceeds five rows.
    if input.len() > 4096 {
        return 7.min(available);
    }
    let rows = with_composer_rows(
        input,
        usize::from(width.saturating_sub(2)).max(1),
        0,
        |wrapped| (wrapped.rows.len() + usize::from(wrapped.stats.is_some())).clamp(2, 5) as u16,
    );
    (rows + 2).min(available.saturating_sub(1))
}

struct ComposerLayout {
    input: String,
    width: usize,
    wrapped: WrappedComposer,
}

thread_local! {
    // Only the latest draft/width is retained. Both height and paint reuse this
    // layout; unchanged megabyte pastes must not be reflowed every frame.
    static COMPOSER_LAYOUT: RefCell<Option<ComposerLayout>> = const { RefCell::new(None) };
}

fn with_composer_rows<T>(
    input: &str,
    width: usize,
    cursor: usize,
    f: impl FnOnce(&WrappedComposer) -> T,
) -> T {
    // Large ASCII drafts use cell wrapping rather than word wrapping. Only a
    // handful of rows around the caret is allocated, even on *changed* drafts.
    // is_ascii/rfind are cheap byte scans, not scalar collection/full reflow.
    if input.len() > 4096 && input.is_ascii() {
        return f(&ascii_composer_window(
            input,
            width.max(1),
            cursor.min(input.len()),
        ));
    }
    COMPOSER_LAYOUT.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache
            .as_ref()
            .is_none_or(|layout| layout.width != width || layout.input != input)
        {
            let chars: Vec<char> = input.chars().collect();
            *cache = Some(ComposerLayout {
                input: input.to_owned(),
                width,
                wrapped: wrap_composer_rows(&chars, width, usize::MAX),
            });
        }
        f(&cache.as_ref().expect("composer layout initialized").wrapped)
    })
}

/// Bounded, stable cell rows for large ASCII drafts. Source offsets are global,
/// so painting, selection and vertical motion share the same insertion points.
fn ascii_composer_window(input: &str, width: usize, cursor: usize) -> WrappedComposer {
    let line_start = input[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let mut start = line_start + (cursor - line_start) / width * width;
    let mut logical_start = line_start;
    // Enough preceding rows for the five-row viewport, plus navigation.
    for _ in 0..6 {
        if start > logical_start {
            start -= width;
        } else if start > 0 {
            let end = start - 1;
            logical_start = input[..end].rfind('\n').map_or(0, |i| i + 1);
            start = logical_start + (end - logical_start) / width * width;
        } else {
            break;
        }
    }
    let mut rows = Vec::with_capacity(14);
    for _ in 0..14 {
        let limit = start.saturating_add(width).min(input.len());
        let end = input[start..limit].find('\n').map_or(limit, |i| start + i);
        rows.push(ComposerRow {
            text: sanitize_composer_display(&input[start..end]),
            start,
            end,
        });
        if end - start == width {
            // Include the insertion row even just before a hard newline.
            start = end;
        } else if end < input.len() {
            start = end + 1;
        } else {
            break;
        }
    }
    WrappedComposer {
        rows,
        char_count: input.len(),
        stats: Some(format!(
            "long draft · {} chars · cell wrap (words not counted)",
            input.len()
        )),
    }
}

pub(crate) fn composer_vertical_cursor(
    input: &str,
    width: usize,
    cursor: usize,
    delta: isize,
) -> usize {
    with_composer_rows(input, width.max(1), cursor, |wrapped| {
        let (row, col) = wrapped_cursor(&wrapped.rows, cursor.min(wrapped.char_count));
        let target = row.saturating_add_signed(delta).min(wrapped.rows.len() - 1);
        if target == row {
            return cursor;
        }
        let row = &wrapped.rows[target];
        let mut source = row.start;
        let mut cells = 0;
        for glyph in row.text.graphemes(true) {
            let next = source + glyph.chars().count();
            if cells + glyph.width() > col {
                break;
            }
            // A soft boundary belongs to the following row. Stay on the target
            // row rather than accidentally skipping it when its text is short.
            if next == row.end
                && wrapped
                    .rows
                    .get(target + 1)
                    .is_some_and(|r| r.start == next)
            {
                break;
            }
            cells += glyph.width();
            source = next;
        }
        source
    })
}

fn styled_composer_line_with_selection(
    text: &str,
    intent: ComposerIntent,
    row_start: usize,
    selection: Option<(usize, usize)>,
) -> Line<'static> {
    let Some((selection_start, selection_end)) = selection else {
        return styled_composer_line(text, intent);
    };
    let row_end = row_start.saturating_add(text.chars().count());
    let start = selection_start.max(row_start).min(row_end);
    let end = selection_end.max(row_start).min(row_end);
    if start >= end {
        return styled_composer_line(text, intent);
    }
    let chars = text.chars().collect::<Vec<_>>();
    let mut local_start = start - row_start;
    let mut local_end = end - row_start;
    let mut offset = 0;
    for grapheme in text.graphemes(true) {
        let next = offset + grapheme.chars().count();
        if offset < local_start && local_start < next {
            local_start = offset;
        }
        if offset < local_end && local_end < next {
            local_end = next;
        }
        offset = next;
    }
    let before = chars[..local_start].iter().collect::<String>();
    let selected = chars[local_start..local_end].iter().collect::<String>();
    let after = chars[local_end..].iter().collect::<String>();
    let mut spans = styled_composer_line(&before, intent).spans;
    spans.push(Span::styled(selected, COMPOSER_SELECTION_STYLE));
    spans.extend(styled_composer_line(&after, intent).spans);
    Line::from(spans)
}

#[derive(Debug)]
struct ComposerRow {
    text: String,
    start: usize,
    end: usize,
}

#[derive(Debug)]
struct WrappedComposer {
    rows: Vec<ComposerRow>,
    char_count: usize,
    stats: Option<String>,
}

/// Wrap whole graphemes at words and hard breaks, retaining source *character*
/// indices (not byte offsets). Controls paint as spaces without changing those
/// indices. Widths match Ratatui's grapheme-cell rendering, including ZWJ emoji.
fn wrap_composer_rows(chars: &[char], width: usize, row_limit: usize) -> WrappedComposer {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut source_start = 0;
    for segment in chars.split(|ch| is_composer_hard_break(*ch)) {
        let text: String = segment
            .iter()
            .copied()
            .map(sanitize_composer_display_char)
            .collect();
        let mut glyphs = Vec::new();
        let mut source = source_start;
        for grapheme in text.graphemes(true) {
            let end = source + grapheme.chars().count();
            glyphs.push((grapheme, source, end));
            source = end;
        }
        let mut start = 0;
        if glyphs.is_empty() && rows.len() < row_limit {
            rows.push(ComposerRow {
                text: String::new(),
                start: source_start,
                end: source_start,
            });
        }
        while start < glyphs.len() && rows.len() < row_limit {
            let mut end = start;
            let mut cells = 0;
            let mut last_break = None;
            while end < glyphs.len() {
                let glyph_width = glyphs[end].0.width();
                if end > start && cells + glyph_width > width {
                    break;
                }
                cells += glyph_width;
                end += 1;
                if glyphs[end - 1].0.chars().all(char::is_whitespace) {
                    last_break = Some(end);
                }
                if cells >= width {
                    break;
                }
            }
            if end < glyphs.len() {
                end = last_break.unwrap_or(end);
            }
            rows.push(ComposerRow {
                text: glyphs[start..end].iter().map(|g| g.0).collect(),
                start: glyphs[start].1,
                end: glyphs[end - 1].2,
            });
            start = end;
        }
        // Full rows need a distinct insertion cell before *every* hard break,
        // not only at EOF; otherwise the caret overwrites the last glyph.
        if rows.len() < row_limit
            && rows
                .last()
                .is_some_and(|row| composer_display_width(&row.text) >= width)
        {
            let end = source_start + segment.len();
            rows.push(ComposerRow {
                text: String::new(),
                start: end,
                end,
            });
        }
        source_start += segment.len() + 1;
        if rows.len() >= row_limit {
            break;
        }
    }
    let stats = (chars.len() >= 4096).then(|| {
        let words = chars
            .split(|ch| ch.is_whitespace())
            .filter(|word| !word.is_empty())
            .count();
        format!("long draft · {} chars · {} words", chars.len(), words)
    });
    WrappedComposer {
        rows,
        char_count: chars.len(),
        stats,
    }
}

fn wrapped_cursor(rows: &[ComposerRow], cur: usize) -> (usize, usize) {
    // A soft boundary belongs to the following row; a hard break's preceding
    // caret still belongs to the row before it, even for consecutive newlines.
    for (index, row) in rows.iter().enumerate().rev() {
        if cur >= row.start && cur <= row.end {
            let mut source = row.start;
            let mut col = 0;
            for grapheme in row.text.graphemes(true) {
                source += grapheme.chars().count();
                if source > cur {
                    break;
                }
                col += grapheme.width();
            }
            return (index, col);
        }
    }
    (0, 0)
}

fn styled_composer_line(text: &str, intent: ComposerIntent) -> Line<'static> {
    if text.is_empty() {
        return Line::from(String::new());
    }
    let mut spans = Vec::new();
    let mut token = String::new();
    let mut at_line_start = true;
    let flush = |token: &mut String, at_line_start: &mut bool, spans: &mut Vec<Span<'static>>| {
        if token.is_empty() {
            return;
        }
        let clean = token.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '/' && ch != '@');
        let upper = clean.to_ascii_uppercase();
        let style = if *at_line_start && clean.starts_with('/') {
            if intent == ComposerIntent::CriticalCommand {
                HUD_DANGER_BOLD_STYLE
            } else {
                HUD_GOLD_BOLD_STYLE
            }
        } else if clean.starts_with('@') && clean.len() > 1 {
            HUD_BLUE_BOLD_STYLE
        } else if matches!(upper.as_str(), "CRITICAL" | "URGENT" | "BLOCKER" | "DANGER") {
            HUD_DANGER_BOLD_STYLE
        } else if token.starts_with('`') && token.ends_with('`') && token.len() > 1 {
            HUD_GOLD_BOLD_STYLE
        } else {
            HUD_TEXT_STYLE
        };
        spans.push(Span::styled(std::mem::take(token), style));
        *at_line_start = false;
    };
    for ch in text.chars() {
        if ch.is_whitespace() {
            flush(&mut token, &mut at_line_start, &mut spans);
            spans.push(Span::raw(ch.to_string()));
        } else {
            token.push(ch);
        }
    }
    flush(&mut token, &mut at_line_start, &mut spans);
    Line::from(spans)
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

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn fmt_clock_handles_minutes_and_hours() {
        assert_eq!(fmt_clock(0.0), "00:00");
        assert_eq!(fmt_clock(65.0), "01:05");
        assert_eq!(fmt_clock(3661.0), "1:01:01");
    }

    #[test]
    fn header_line_uses_cached_idle_lines_for_known_agents() {
        let line = header_line("practice", Glyph::Idle, 0.0, 0.0);
        let text = flatten(std::slice::from_ref(&line));
        assert!(text.contains("angel0"));
        assert!(text.contains("Practice"));
        assert!(text.contains("idle"));
        assert!(text.contains("00%"));
        assert!(matches!(
            header_line("spark", Glyph::Idle, 0.0, 0.0).spans[1].content,
            Cow::Borrowed(_)
        ));
        assert!(matches!(
            header_line("spark-r1", Glyph::Idle, 0.0, 0.0).spans[1].content,
            Cow::Borrowed(_)
        ));
        assert!(matches!(
            header_line("custom", Glyph::Idle, 0.0, 0.0).spans[1].content,
            Cow::Borrowed(_)
        ));
        assert!(matches!(
            header_line("practice", Glyph::Thinking, 0.0, 0.0).spans[1].content,
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn working_lines_include_agent_clock_and_load_metrics() {
        let snapshot = OverwatchSnapshot {
            cpu_pct: 12.3,
            gpu_pct: 45.6,
            ..OverwatchSnapshot::default()
        };
        let title = working_title("moa", 65.0, snapshot.load_pct());
        assert!(matches!(title, Cow::Owned(_)));
        let text = flatten(&working_lines("moa", 65.0, snapshot));
        assert!(title.contains("moa"));
        assert!(title.contains("01:05"));
        assert!(text.contains("12.3%"));
        assert!(text.contains("45.6%"));
    }

    #[test]
    fn working_helpers_can_share_preformatted_clock() {
        let snapshot = OverwatchSnapshot {
            cpu_pct: 12.3,
            gpu_pct: 45.6,
            ..OverwatchSnapshot::default()
        };
        let clock = fmt_clock(65.0);
        let title = working_title_with_clock("moa", &clock, snapshot.load_pct());
        let lines = working_lines_with_clock("moa", &clock, snapshot);
        assert!(title.contains("01:05"));
        assert!(matches!(lines[0].spans[4].content, Cow::Borrowed(_)));
        assert_eq!(lines[0].spans[4].content.as_ptr(), clock.as_ptr());
    }

    #[test]
    fn working_title_borrows_zero_state_for_known_agents() {
        let title = working_title("practice", 0.0, 0.0);
        assert!(matches!(title, Cow::Borrowed(_)));
        assert!(title.contains("00:00"));
        assert!(matches!(working_title("spark", 0.2, 0.0), Cow::Borrowed(_)));
        assert!(matches!(
            working_title("spark-r1", 0.0, 0.0),
            Cow::Borrowed(_)
        ));
        assert!(matches!(working_title("custom", 0.0, 0.0), Cow::Owned(_)));
        assert!(matches!(working_title("practice", 1.0, 0.0), Cow::Owned(_)));
        assert!(matches!(
            working_title("practice", 0.0, 12.0),
            Cow::Owned(_)
        ));
    }

    #[test]
    fn working_lines_borrow_zero_state_for_known_agents() {
        let lines = working_lines("practice", 0.0, OverwatchSnapshot::default());
        let text = flatten(&lines);
        assert!(text.contains("practice"));
        assert!(text.contains("00.0%"));
        assert!(matches!(lines[0].spans[2].content, Cow::Borrowed(_)));
        let dynamic = working_lines(
            "practice",
            0.0,
            OverwatchSnapshot {
                cpu_pct: 1.0,
                ..OverwatchSnapshot::default()
            },
        );
        assert!(matches!(dynamic[0].spans[8].content, Cow::Owned(_)));
    }

    #[test]
    fn compact_load_suffix_borrows_default_zero_metrics() {
        let suffix = compact_load_suffix(0.0, 0.0);
        assert!(matches!(suffix, Cow::Borrowed(_)));
        assert!(suffix.contains("00%"));
        assert!(matches!(compact_load_suffix(12.0, 0.0), Cow::Owned(_)));
    }

    #[test]
    fn agent_bay_title_borrows_default_idle_agent() {
        let title = agent_bay_title("A>", "Agent", Glyph::Idle, 0.0, 0.0);
        assert!(matches!(title, Cow::Borrowed(_)));
        assert!(title.contains("Agent"));
        let sparky = agent_bay_title("Sp", "Sparky", Glyph::Idle, 0.0, 0.0);
        assert!(matches!(sparky, Cow::Borrowed(_)));
        assert!(sparky.contains("Sparky"));
        assert!(matches!(
            agent_bay_title("Sp", "Agent", Glyph::Idle, 0.0, 0.0),
            Cow::Owned(_)
        ));
    }

    #[test]
    fn agent_metrics_line_borrows_default_idle_metrics() {
        let line = agent_metrics_line(0.0, 0.0, 0.0, false);
        assert!(matches!(line, Cow::Borrowed(_)));
        assert_eq!(line, "C>00% M>00% G>00% .. idle");
        let loaded = agent_metrics_line(1.0, 42.0, 0.0, false);
        assert!(matches!(loaded, Cow::Owned(_)));
        assert!(loaded.contains("M>42%"), "{loaded}");
        let active = agent_metrics_line(0.0, 0.0, 0.0, true);
        assert!(matches!(active, Cow::Borrowed(_)));
        assert_eq!(active, "C>00% M>00% G>00% *> active");
    }

    #[test]
    fn agent_route_title_uses_active_box_mode_and_busy_metrics() {
        use crate::club::ClubTab;
        let tabs = [
            ClubTab {
                label: "openai".into(),
                mode: None,
                in_hand: false,
                available: true,
            },
            ClubTab {
                label: "sota".into(),
                mode: Some("longcat".into()),
                in_hand: true,
                available: true,
            },
        ];
        let title = agent_route_title(&tabs, Some("08:28"), Some(6.0), 40).unwrap();
        assert!(title.contains("longcat"), "{title}");
        assert!(
            !title.contains("sota·"),
            "bay title must not prefix the machine slug: {title}"
        );
        assert!(title.contains("08:28"), "{title}");
        assert!(title.contains("load 06%"), "{title}");
        assert!(
            !title.contains("openai"),
            "agent chrome shows current route, not the whole tab strip: {title}"
        );
    }

    #[test]
    fn token_usage_meter_formats_last_and_session_counts() {
        let meter = token_usage_meter(Some(TokenUsage {
            turns: 2,
            last_input: 20,
            last_output: 6,
            last_reasoning: 3,
            total_input: 30,
            total_output: 10,
            total_reasoning: 3,
        }));
        assert!(meter.contains("tok in 20/30"));
        assert!(meter.contains("out 6/10"));
        assert!(meter.contains("rsn 3/3"));
        assert_eq!(token_usage_meter(None), "tok in --/-- · out --/--");
    }

    #[test]
    fn moa_token_report_lines_show_per_model_counts_and_bars() {
        let report = crate::swarm::ledger::MoaTokenReport {
            turns: 3,
            latest_route: "deliberate".into(),
            latest_ok: true,
            latest_ms: 1200,
            rows: vec![
                crate::swarm::ledger::MoaModelTokenRow {
                    label: "openai".into(),
                    last_input: 1200,
                    last_output: 300,
                    total_input: 2400,
                    total_output: 600,
                },
                crate::swarm::ledger::MoaModelTokenRow {
                    label: "deepseek".into(),
                    last_input: 600,
                    last_output: 100,
                    total_input: 900,
                    total_output: 150,
                },
            ],
        };
        let lines = moa_token_report_lines(&report, 48, 3);
        let text = flatten(&lines);
        assert!(text.contains("moa tok 2200"), "{text}");
        assert!(text.contains("openai"), "{text}");
        assert!(text.contains("1200/300"), "{text}");
        assert!(text.contains("deepseek"), "{text}");
        assert!(
            text.chars()
                .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch)),
            "dotmax braille bar missing: {text}"
        );
    }

    #[test]
    fn message_title_includes_active_agent_label() {
        let title = message_title("spark");
        assert!(matches!(title, Cow::Borrowed(_)));
        assert!(title.contains("[spark]"));
        assert_eq!(title, message_title("spark"));
        assert!(matches!(message_title("spark-r1"), Cow::Borrowed(_)));
        assert!(matches!(message_title("turbo"), Cow::Borrowed(_)));
        assert!(matches!(message_title("atlas"), Cow::Borrowed(_)));
        assert!(matches!(message_title("gemma"), Cow::Borrowed(_)));
        assert!(matches!(message_title("moa"), Cow::Borrowed(_)));
        assert!(matches!(message_title("deli"), Cow::Borrowed(_)));
        assert!(matches!(message_title("custom"), Cow::Owned(_)));
    }

    #[test]
    fn practice_message_title_uses_static_fast_path() {
        let title = message_title("practice");
        assert!(matches!(title, Cow::Borrowed(_)));
        assert!(title.contains("[practice]"));
    }

    #[test]
    fn message_title_tabs_lists_boxes_with_active_mode() {
        use crate::club::ClubTab;
        let tab = |label: &str, mode: Option<&str>, in_hand: bool, available: bool| ClubTab {
            label: label.to_string(),
            mode: mode.map(str::to_string),
            in_hand,
            available,
        };
        // In-hand spark box on its moa mode, plus reachable atlas box. The offline
        // turbo box is already absent from the list the caller built.
        let tabs = [
            tab("spark", Some("moa"), true, true),
            tab("atlas", None, false, true),
        ];
        let title = message_title_tabs(&tabs);
        assert!(
            title.contains("[spark·moa]"),
            "in-hand box shows its mode: {title}"
        );
        assert!(
            title.contains("atlas"),
            "the other reachable box is listed: {title}"
        );
        assert!(
            !title.contains("turbo"),
            "offline boxes are absent: {title}"
        );
        assert!(
            title.contains("←→ mode"),
            "multi-mode box shows the subcontrol hint: {title}"
        );

        // A lone reachable single-mode box collapses to the cached label title.
        let one = [tab("atlas", None, true, true)];
        assert_eq!(message_title_tabs(&one), message_title("atlas"));

        // An in-hand box that just went offline is still shown, flagged down.
        let down = [
            tab("spark", Some("moa"), true, false),
            tab("atlas", None, false, true),
        ];
        let title = message_title_tabs(&down);
        assert!(
            title.contains(Glyph::Offline.token()),
            "down in-hand box carries the offline mark: {title}"
        );
    }

    #[test]
    fn reasoning_title_uses_static_fast_path_for_known_agents() {
        let title = reasoning_title("practice");
        assert!(matches!(title, Cow::Borrowed(_)));
        assert!(title.contains("thinking"));
        assert!(title.contains("Practice"));
        assert!(matches!(reasoning_title("spark"), Cow::Borrowed(_)));
        assert!(matches!(reasoning_title("spark-r1"), Cow::Borrowed(_)));
        assert!(matches!(reasoning_title("turbo"), Cow::Borrowed(_)));
        assert!(matches!(reasoning_title("atlas"), Cow::Borrowed(_)));
        assert!(matches!(reasoning_title("gemma"), Cow::Borrowed(_)));
        assert!(matches!(reasoning_title("moa"), Cow::Borrowed(_)));
        assert!(matches!(reasoning_title("deli"), Cow::Borrowed(_)));
        assert!(matches!(reasoning_title("custom"), Cow::Owned(_)));
    }

    #[test]
    fn reasoning_title_caches_custom_labels() {
        let first = reasoning_title("operator-x");
        let second = reasoning_title("operator-x");
        assert!(matches!(first, Cow::Owned(_)));
        assert_eq!(first, second);
        assert!(second.contains("operator-x"));
    }

    #[test]
    fn empty_artifacts_line_is_cached_and_readable() {
        let line = empty_artifacts_line();
        let text = flatten(std::slice::from_ref(&line));
        assert!(text.contains("no delivered artifacts"));
        assert_eq!(
            line.spans[0].content.as_ptr(),
            empty_artifacts_line().spans[0].content.as_ptr()
        );
    }

    #[test]
    fn agent_shell_title_is_cached_and_readable() {
        let title = agent_shell_title();
        assert!(title.contains("agent shell"));
        assert_eq!(title, agent_shell_title());
    }

    #[test]
    fn shell_focus_title_is_cached_and_readable() {
        let title = shell_focus_title();
        assert!(title.contains("shell"));
        assert!(title.contains("^G"));
        assert_eq!(title.as_ptr(), shell_focus_title().as_ptr());
    }

    #[test]
    fn image_viewer_title_is_cached_and_readable() {
        let title = image_viewer_title();
        assert!(title.contains("/hide to return"));
        assert_eq!(title.as_ptr(), image_viewer_title().as_ptr());
    }

    #[test]
    fn shell_hint_is_cached_static_text() {
        let hint = shell_hint();
        assert!(hint.contains("shell focused"));
        assert_eq!(hint.as_ptr(), shell_hint().as_ptr());
    }

    #[test]
    fn composer_placeholder_pads_full_width_to_kill_ghosts() {
        let width = 80;
        for busy in [false, true] {
            let line = composer_placeholder(busy, width);
            let flat: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert_eq!(
                composer_display_width(&flat),
                width,
                "busy={busy}: placeholder must fill every display cell (got {flat:?})"
            );
            assert!(
                line.spans
                    .iter()
                    .all(|s| s.style.fg == Some(crate::hud::HUD_DIM)),
                "busy={busy}: placeholder stays dim"
            );
        }
    }

    /// A3: a double-width glyph in a width-1 pane must still pad/clip to one cell
    /// after the draw-path pad pass (otherwise a neighbor cell ghosts).
    #[test]
    fn pad_composer_lines_clips_wide_glyph_in_narrow_pane() {
        let width = 1;
        let mut view = composer_view("🙂", width, 1, 1);
        pad_composer_lines(&mut view.lines, width);
        assert!(!view.lines.is_empty());
        let flat: String = view.lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(
            composer_display_width(&flat),
            width,
            "narrow pane must not leave a 2-cell emoji row: {flat:?}"
        );
    }

    /// A3: multi-span rows (styled tokens + wide glyphs) truncate to exact width.
    #[test]
    fn pad_composer_lines_multi_span_truncates_to_exact_display_width() {
        let width = 8;
        let mut lines = vec![Line::from(vec![
            Span::styled("hi", HUD_TEXT_STYLE),
            Span::styled("🙂🙂🙂", HUD_GOLD_BOLD_STYLE), // typically 6 cells
        ])];
        pad_composer_lines(&mut lines, width);
        let flat: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(composer_display_width(&flat), width, "exact={flat:?}");

        let mut long = vec![Line::from(vec![
            Span::styled("hello", HUD_TEXT_STYLE),    // 5
            Span::styled("世界世界", HUD_TEXT_STYLE), // 8
        ])];
        pad_composer_lines(&mut long, width);
        let flat: String = long[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(
            composer_display_width(&flat),
            width,
            "multi-span overflow must clip to {width}: {flat:?}"
        );
    }

    #[test]
    fn pad_composer_lines_rewrites_short_rows() {
        let width = 40;
        let mut lines = vec![Line::from(Span::styled("short", HUD_TEXT_STYLE))];
        pad_composer_lines(&mut lines, width);
        let flat: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(flat.starts_with("short"));
        assert!(flat.ends_with(' '));
        assert_eq!(composer_display_width(&flat), width);
    }

    /// A3: wide glyphs are 2 terminal cells — char-count pad left ghost cells.
    #[test]
    fn pad_composer_row_uses_display_width_for_wide_glyphs() {
        let width = 10;
        // "🙂" is typically width 2; "hi" is 2 → content display 4 → pad 6 spaces.
        let row = pad_composer_row("hi🙂", width);
        assert_eq!(
            composer_display_width(&row),
            width,
            "row={row:?} must fill {width} cells"
        );
        // Truncate: a string wider than the pane must still land on exact width.
        let wide = pad_composer_row("日本語テスト長め", 6);
        assert_eq!(composer_display_width(&wide), 6, "wide={wide:?}");
    }

    #[test]
    fn pad_composer_lines_clears_wide_glyph_residue_on_shorten() {
        let width = 12;
        // Simulate a long draft row that then shrinks to a short one.
        let mut lines = vec![Line::from(Span::styled(
            pad_composer_row("旧い長い下書き🙂残渣", width),
            HUD_TEXT_STYLE,
        ))];
        assert_eq!(composer_display_width(&flatten_line(&lines[0])), width);
        // User cleared to a short draft.
        lines[0] = Line::from(Span::styled("ok", HUD_TEXT_STYLE));
        pad_composer_lines(&mut lines, width);
        assert_eq!(
            composer_display_width(&flatten_line(&lines[0])),
            width,
            "shortened draft must rewrite every cell"
        );
        let flat = flatten_line(&lines[0]);
        assert!(flat.starts_with("ok"));
        assert!(!flat.contains('旧'), "no residual CJK from prior draft");
    }

    /// A3: pasted tabs (and other controls) must not under-pad — unicode-width
    /// treats TAB as 0 while the terminal expands it, leaving residual cells.
    #[test]
    fn pad_composer_row_sanitizes_tabs_to_exact_display_width() {
        let width = 10;
        let row = pad_composer_row("a\tb\tc", width);
        assert!(
            !row.contains('\t'),
            "TAB must not survive into painted cells: {row:?}"
        );
        assert_eq!(
            composer_display_width(&row),
            width,
            "tabbed draft must still fill every cell: {row:?}"
        );
        // Multi-span pad path also rewrites controls.
        let mut lines = vec![Line::from(vec![
            Span::styled("id\t", HUD_TEXT_STYLE),
            Span::styled("name\tval", HUD_GOLD_BOLD_STYLE),
        ])];
        pad_composer_lines(&mut lines, width);
        let flat = flatten_line(&lines[0]);
        assert!(!flat.contains('\t'), "multi-span: {flat:?}");
        assert_eq!(composer_display_width(&flat), width, "multi-span: {flat:?}");
    }

    /// A3: soft-hyphen / ZWSP / BOM report width 0 but terminals may still reserve
    /// a cell — counting them as 0 under-pads and leaves residual neighbor cells.
    /// Format ignorables become spaces (1 cell); emoji ZWJ stays for clusters.
    #[test]
    fn pad_composer_row_sanitizes_format_zwsp_to_exact_display_width() {
        let width = 8;
        let dirty = "ab\u{200b}\u{00ad}cd\u{feff}"; // 4 letters + ZWSP + SHY + BOM
        let row = pad_composer_row(dirty, width);
        assert!(
            !row.chars().any(|c| matches!(
                c,
                '\u{200B}' | '\u{00AD}' | '\u{FEFF}' | '\u{2060}' | '\u{180E}'
            )),
            "format ZW must not survive into painted cells: {row:?}"
        );
        assert_eq!(
            composer_display_width(&row),
            width,
            "format-ZW draft must fill every cell: {row:?}"
        );
        for ch in row.chars() {
            assert!(
                composer_char_display_width(ch) >= 1,
                "painted cell must be ≥1 wide after sanitize: {ch:?} in {row:?}"
            );
        }
        // Multi-span pad path rewrites format ZW the same way.
        let mut lines = vec![Line::from(vec![
            Span::styled("a\u{200b}", HUD_TEXT_STYLE),
            Span::styled("b\u{00ad}c", HUD_GOLD_BOLD_STYLE),
        ])];
        pad_composer_lines(&mut lines, width);
        let flat = flatten_line(&lines[0]);
        assert!(
            !flat.contains('\u{200b}') && !flat.contains('\u{00ad}'),
            "multi-span format ZW: {flat:?}"
        );
        assert_eq!(composer_display_width(&flat), width, "multi-span: {flat:?}");

        // Wrap path must match pad budget (no TAB-style measure skew).
        let wrapped = wrap_composer_rows(&"x\u{200b}\u{00ad}y".chars().collect::<Vec<_>>(), 4, 2);
        assert!(!wrapped.rows.is_empty());
        let painted = &wrapped.rows[0].text;
        assert!(
            !painted.contains('\u{200b}') && !painted.contains('\u{00ad}'),
            "wrap must sanitize format ZW: {painted:?}"
        );
        assert_eq!(
            composer_display_width(painted),
            painted.chars().count(),
            "after sanitize every char is 1-cell space or glyph: {painted:?}"
        );

        // Emoji ZWJ sequences stay joinable (not forced to spaces).
        let family = "👨\u{200d}👩";
        let kept = sanitize_composer_display(family);
        assert!(
            kept.contains('\u{200d}'),
            "ZWJ must remain for emoji clusters: {kept:?}"
        );
    }

    /// A3 adversarial: ZWNJ and bidi marks/embeddings report unicode-width 0
    /// (same class as ZWSP) but were not sanitized — pad under-filled and left
    /// residual cells after a cleared draft. ZWJ / VS16 stay for emoji.
    #[test]
    fn pad_composer_row_sanitizes_zwnj_and_bidi_marks() {
        let width = 10;
        let dirty = "ab\u{200c}\u{200e}\u{200f}cd\u{202a}\u{202c}\u{2066}\u{2069}";
        let row = pad_composer_row(dirty, width);
        assert!(
            !row.chars().any(|c| matches!(
                c,
                '\u{200C}'
                    | '\u{200E}'
                    | '\u{200F}'
                    | '\u{202A}'
                    | '\u{202B}'
                    | '\u{202C}'
                    | '\u{202D}'
                    | '\u{202E}'
                    | '\u{2066}'
                    | '\u{2067}'
                    | '\u{2068}'
                    | '\u{2069}'
            )),
            "bidi/ZWNJ format marks must not survive paint: {row:?}"
        );
        assert_eq!(
            composer_display_width(&row),
            width,
            "bidi/ZWNJ draft must fill every cell: {row:?}"
        );
        for ch in row.chars() {
            assert!(
                composer_char_display_width(ch) >= 1,
                "painted cell ≥1 after sanitize: {ch:?} in {row:?}"
            );
        }
        // Multi-span + wrap paths share sanitize.
        let mut lines = vec![Line::from(vec![
            Span::styled("x\u{200c}", HUD_TEXT_STYLE),
            Span::styled("y\u{200e}z", HUD_GOLD_BOLD_STYLE),
        ])];
        pad_composer_lines(&mut lines, width);
        let flat = flatten_line(&lines[0]);
        assert!(
            !flat.contains('\u{200c}') && !flat.contains('\u{200e}'),
            "multi-span bidi/ZWNJ: {flat:?}"
        );
        assert_eq!(composer_display_width(&flat), width, "multi-span: {flat:?}");

        let wrapped = wrap_composer_rows(&"p\u{200c}\u{202a}q".chars().collect::<Vec<_>>(), 4, 2);
        assert!(!wrapped.rows.is_empty());
        let painted = &wrapped.rows[0].text;
        assert!(
            !painted.contains('\u{200c}') && !painted.contains('\u{202a}'),
            "wrap must sanitize bidi/ZWNJ: {painted:?}"
        );

        // ZWJ + variation selector still kept for emoji clusters.
        let emoji = "❤\u{fe0f}\u{200d}🔥";
        let kept = sanitize_composer_display(emoji);
        assert!(
            kept.contains('\u{200d}') && kept.contains('\u{fe0f}'),
            "ZWJ/VS must remain: {kept:?}"
        );
    }

    /// A3: Unicode LINE/PARAGRAPH SEPARATOR must hard-break like LF so the
    /// terminal cannot paint an extra physical row under a short pad (ghosts).
    /// Compact/pad paths also sanitize residual LS/PS to spaces.
    #[test]
    fn wrap_composer_rows_hard_breaks_on_unicode_line_separators() {
        let width = 40;
        for sep in ['\u{2028}', '\u{2029}'] {
            let chars: Vec<char> = format!("hello{sep}world").chars().collect();
            let wrapped = wrap_composer_rows(&chars, width, 4);
            assert!(
                wrapped.rows.len() >= 2,
                "U+{:04X} must split into ≥2 visual rows, got {}: {:?}",
                sep as u32,
                wrapped.rows.len(),
                wrapped.rows.iter().map(|r| &r.text).collect::<Vec<_>>()
            );
            assert_eq!(wrapped.rows[0].text, "hello", "sep U+{:04X}", sep as u32);
            assert_eq!(wrapped.rows[1].text, "world", "sep U+{:04X}", sep as u32);
            for row in &wrapped.rows {
                assert!(
                    !row.text.contains(sep),
                    "hard break must not paint into row text: {:?}",
                    row.text
                );
            }
            // Full composer_view + pad: every painted row fills the pane.
            let mut view = composer_view(&format!("hello{sep}world"), width, 3, 0);
            pad_composer_lines(&mut view.lines, width);
            assert!(view.lines.len() >= 2);
            for line in &view.lines {
                let flat: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                assert!(
                    !flat.contains('\u{2028}') && !flat.contains('\u{2029}'),
                    "LS/PS must not survive paint: {flat:?}"
                );
                assert_eq!(
                    composer_display_width(&flat),
                    width,
                    "padded row must fill pane: {flat:?}"
                );
            }
        }
        // LF still behaves the same.
        let lf = wrap_composer_rows(&"hello\nworld".chars().collect::<Vec<_>>(), width, 4);
        assert_eq!(lf.rows[0].text, "hello");
        assert_eq!(lf.rows[1].text, "world");
        // Pad-only path: residual LS becomes a space (compact drafts).
        let row = pad_composer_row("ab\u{2028}cd", 8);
        assert!(!row.contains('\u{2028}'));
        assert_eq!(composer_display_width(&row), 8);
    }

    fn flatten_line(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn composer_steer_hint_tracks_busy_and_queue_depth() {
        assert_eq!(composer_steer_hint(false, 0), None, "idle → default title");
        assert_eq!(
            composer_steer_hint(true, 0).as_deref(),
            Some("· busy — Enter queues a steer ")
        );
        assert_eq!(
            composer_steer_hint(true, 1).as_deref(),
            Some("· 1 steer queued ")
        );
        assert_eq!(
            composer_steer_hint(true, 3).as_deref(),
            Some("· 3 steers queued ")
        );
    }

    #[test]
    fn composer_intent_distinguishes_drafts_commands_and_high_impact_commands() {
        assert_eq!(composer_intent(""), ComposerIntent::Empty);
        assert_eq!(
            composer_intent(&format!("/self {}discard", " ".repeat(200))),
            ComposerIntent::CriticalCommand
        );
        assert_eq!(
            composer_intent("/self discardxxxxxxxxxxxxxxxx"),
            ComposerIntent::Command
        );
        assert_eq!(
            composer_intent("please delete the stale note"),
            ComposerIntent::Message
        );
        assert_eq!(composer_intent("/status"), ComposerIntent::Command);
        assert_eq!(composer_intent("/clear"), ComposerIntent::CriticalCommand);
        assert_eq!(
            composer_intent("  /self discard  "),
            ComposerIntent::CriticalCommand
        );
        assert_eq!(composer_intent("EXIT"), ComposerIntent::CriticalCommand);
        assert_eq!(composer_intent("/Clear"), ComposerIntent::CriticalCommand);
        assert_eq!(
            composer_intent("/LOOP STOP extra"),
            ComposerIntent::CriticalCommand
        );
        assert_eq!(
            composer_intent(&format!("{} /clear", "word ".repeat(200))),
            ComposerIntent::Message,
            "critical tokens in the body must not paint the review rail"
        );
        assert_eq!(ComposerIntent::Empty.title(46), "");
        assert_eq!(ComposerIntent::Empty.title(70), "");
        assert!(!ComposerIntent::Empty.title(70).contains("Enter sends"));
        assert!(!ComposerIntent::Empty.title(70).contains("Shift+Enter"));
        let src = include_str!("status_view.rs");
        let start = src
            .find("pub fn composer_intent(")
            .expect("composer_intent");
        let body = src[start..]
            .split("pub fn composer_frame_title(")
            .next()
            .expect("intent body");
        assert!(
            !body.contains("to_ascii_lowercase"),
            "composer_intent must not lowercase the whole draft: {body}"
        );
    }

    #[test]
    fn composer_frame_title_borrows_idle_chrome() {
        let idle = composer_frame_title(
            ComposerIntent::Empty,
            80,
            ComposerTitleState {
                steerable: true,
                ..Default::default()
            },
        );
        assert!(
            matches!(idle, Cow::Borrowed(_)),
            "idle title must stay a static borrow, got {idle:?}"
        );
        assert_eq!(idle.as_ref(), ComposerIntent::Empty.title(80));

        let draft = composer_frame_title(
            ComposerIntent::Message,
            80,
            ComposerTitleState {
                steerable: true,
                ..Default::default()
            },
        );
        assert!(matches!(draft, Cow::Borrowed(_)));

        let busy = composer_frame_title(
            ComposerIntent::Empty,
            80,
            ComposerTitleState {
                busy: true,
                steerable: true,
                ..Default::default()
            },
        );
        assert!(
            matches!(busy, Cow::Owned(_)),
            "busy steer hint extends the title"
        );
        assert!(busy.contains("guidance"));
        assert!(busy.contains("busy"));
    }

    #[test]
    fn composer_view_wraps_prompt_until_capacity() {
        let view = composer_view("abcdef", 3, 2, 6);
        assert_eq!(view.lines.len(), 2);
        assert_eq!(line_text(&view.lines[0]), "def");
        assert_eq!(line_text(&view.lines[1]), "");
        assert_eq!(view.cursor_row, 0);
        assert_eq!(view.cursor_col, 3);
    }

    #[test]
    fn composer_view_wraps_at_words_and_respects_pasted_newlines() {
        let view = composer_view("hello gentle\nworld", 8, 3, 18);
        assert_eq!(line_text(&view.lines[0]), "hello ");
        assert_eq!(line_text(&view.lines[1]), "gentle");
        assert_eq!(line_text(&view.lines[2]), "world");
        assert_eq!(view.cursor_row, 2);
        assert_eq!(view.cursor_col, 5);
    }

    #[test]
    fn composer_view_scrolls_prompt_vertically_without_stats_or_horizontal_tail() {
        let input = "word ".repeat(20);
        let view = composer_view(&input, 48, 2, input.chars().count());
        assert_eq!(view.lines.len(), 2);
        assert_eq!(line_text(&view.lines[0]), "word ".repeat(9));
        assert_eq!(line_text(&view.lines[1]), "word ".repeat(2));
        assert_eq!((view.cursor_row, view.cursor_col), (1, 10));
        let home = composer_view(&input, 48, 2, 0);
        assert_eq!(line_text(&home.lines[1]), "word ".repeat(9));
        assert_eq!((home.cursor_row, home.cursor_col), (0, 0));
    }

    #[test]
    fn composer_view_handles_multibyte_prompt_safely() {
        let input = "ធ្វើការ inspect workspace quickly for HTML5 game assets/code.";
        let view = composer_view(input, 24, 1, input.chars().count());
        assert_eq!(view.lines.len(), 1);
        assert!(line_text(&view.lines[0]).width() <= 24);
    }

    #[test]
    fn composer_window_uses_terminal_cells_for_wide_unicode() {
        let input = "界".repeat(20);
        let view = composer_view(&input, 10, 1, 10);
        assert_eq!(line_text(&view.lines[0]), "界".repeat(5));
        assert_eq!(view.cursor_col, 0, "soft boundary belongs to the next row");
    }

    #[test]
    fn composer_unicode_wrap_and_caret_are_cell_bounded_at_every_cursor_and_width() {
        for input in [
            "界🙂abc界🙂".repeat(6),
            "e\u{301}界🙂z\u{301}".repeat(6),
            "👩‍💻界\n\nnext\u{2028}last\u{2029}".repeat(6),
        ] {
            for width in 1..=16 {
                let chars: Vec<_> = input.chars().collect();
                let wrapped = wrap_composer_rows(&chars, width, usize::MAX);
                for row in &wrapped.rows {
                    assert_eq!(
                        row.text,
                        chars[row.start..row.end].iter().collect::<String>()
                    );
                    // Never split a combining/ZWJ cluster at a soft boundary.
                    let byte = input
                        .char_indices()
                        .nth(row.start)
                        .map_or(input.len(), |(i, _)| i);
                    assert!(
                        byte == input.len() || input.grapheme_indices(true).any(|(i, _)| i == byte)
                    );
                }
                for cursor in 0..=chars.len() {
                    let mut view = composer_view(&input, width, 3, cursor);
                    assert!(usize::from(view.cursor_col) <= width);
                    assert!(view.cursor_row < 3);
                    if usize::from(view.cursor_col) == width {
                        assert!(view.cursor_row + 1 < 3);
                    }
                    pad_composer_lines(&mut view.lines, width);
                    for line in &view.lines {
                        assert_eq!(line.width(), width, "{width}, {cursor}: {line:?}");
                    }
                }
            }
        }
        let view = composer_view("👩‍💻!", 3, 2, 3);
        assert_eq!((view.cursor_row, view.cursor_col), (0, 2));
    }

    #[test]
    fn composer_multiline_unicode_selection_tracks_scrolled_source_offsets() {
        let input = "前\n\n界e\u{301}👩‍💻\u{2028}終\n";
        let chars: Vec<_> = input.chars().collect();
        let wrapped = wrap_composer_rows(&chars, 4, usize::MAX);
        assert_eq!(
            wrapped
                .rows
                .iter()
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>(),
            ["前", "", "界e\u{301}", "👩‍💻", "終", ""]
        );
        for (cursor, expected) in [
            (0, (0, 0)),
            (1, (0, 2)),
            (2, (1, 0)),
            (3, (2, 0)),
            (4, (2, 2)),
            (6, (3, 0)),
            (9, (3, 2)),
            (10, (4, 0)),
            (12, (5, 0)),
        ] {
            assert_eq!(
                wrapped_cursor(&wrapped.rows, cursor),
                expected,
                "cursor {cursor}"
            );
        }
        let view = composer_view_with_selection(input, 4, 2, 9, Some((3, 9)));
        assert_eq!((view.cursor_row, view.cursor_col), (1, 2));
        let selected: String = view
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .filter(|span| span.style.bg == Some(HUD_BLUE))
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(selected, "界e\u{301}👩‍💻");
        // Scalar-index selections inside a cluster paint the whole glyph, not
        // split spans that would change its terminal width or caret position.
        let mut view = composer_view_with_selection("👩‍💻!", 4, 2, 3, Some((1, 2)));
        pad_composer_lines(&mut view.lines, 4);
        assert_eq!(view.lines[0].width(), 4);
        assert!(
            view.lines[0]
                .spans
                .iter()
                .any(|s| s.content == "👩‍💻" && s.style.bg == Some(HUD_BLUE))
        );
    }

    #[test]
    fn composer_height_grows_then_caps_and_reflows_on_resize_and_clear() {
        assert_eq!(composer_height("", 12, 30), 4);
        assert_eq!(composer_height("one\ntwo", 12, 30), 4);
        assert_eq!(composer_height("one\ntwo\n三", 12, 30), 5);
        assert_eq!(composer_height("界".repeat(11).as_str(), 12, 30), 5);
        assert_eq!(composer_height("界".repeat(11).as_str(), 24, 30), 4);
        assert_eq!(composer_height("a\n".repeat(20).as_str(), 12, 30), 7);
        assert_eq!(composer_height("", 12, 30), 4);
        for available in 0..=9 {
            assert!(
                composer_height("界\n".repeat(10).as_str(), 3, available)
                    <= available.saturating_sub(1)
            );
        }
        // Match draw.rs's bottom-anchored layout: growth reduces only the
        // transcript above, never shifts the composer bottom down.
        use ratatui::layout::{Constraint, Layout, Rect};
        let frame = Rect::new(2, 3, 40, 30);
        let sections = |input| {
            Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(composer_height(input, frame.width, frame.height - 3)),
            ])
            .spacing(-1)
            .split(frame)
        };
        let small = sections("");
        let large = sections("1\n2\n3\n4\n5\n6");
        assert_eq!(small[2].bottom(), large[2].bottom());
        assert_eq!(small[2].y - large[2].y, 3);
        assert_eq!(small[1].height - large[1].height, 3);
    }

    #[test]
    fn composer_large_unicode_paste_keeps_vertical_rows_and_selection_without_a_tail_window() {
        let input = format!("{}界e\u{301}👩‍💻\nlast", "先頭\n".repeat(1500));
        let cursor = input.chars().count();
        let before = input.clone();
        assert_eq!(composer_height(&input, 14, 30), 7);
        let mut view =
            composer_view_with_selection(&input, 12, 5, cursor, Some((cursor - 4, cursor)));
        assert!(line_text(&view.lines[0]).contains("long draft"));
        let visible: Vec<_> = view.lines[1..].iter().map(line_text).collect();
        assert_eq!(visible, ["先頭", "先頭", "界e\u{301}👩‍💻", "last"]);
        assert_eq!((view.cursor_row, view.cursor_col), (4, 4));
        assert!(
            view.lines[4]
                .spans
                .iter()
                .any(|s| s.content == "last" && s.style.bg == Some(HUD_BLUE))
        );
        pad_composer_lines(&mut view.lines, 12);
        assert!(view.lines.iter().all(|line| line.width() == 12));
        let home = composer_view(&input, 12, 5, 0);
        assert_eq!((home.cursor_row, home.cursor_col), (1, 0));
        assert!(home.lines[1..].iter().all(|line| line_text(line) == "先頭"));
        assert_eq!(
            input, before,
            "rendering never alters submitted source bytes"
        );
        for (width, height) in [(0, 5), (12, 0), (0, 0)] {
            let view = composer_view(&input, width, height, cursor);
            assert!(view.lines.is_empty());
            assert_eq!((view.cursor_row, view.cursor_col), (0, 0));
        }
    }

    #[test]
    fn corrective_large_ascii_window_matches_full_cell_rows_across_hard_breaks() {
        let input = format!("xxxx\nx\n{}\n", "x".repeat(5000));
        let chars: Vec<char> = input.chars().collect();
        for width in [1, 4, 80] {
            let full = wrap_composer_rows(&chars, width, usize::MAX);
            for cursor in (0..24).chain(input.len() - 24..=input.len()) {
                let window = ascii_composer_window(&input, width, cursor);
                let (row, col) = wrapped_cursor(&full.rows, cursor);
                let (local, local_col) = wrapped_cursor(&window.rows, cursor);
                assert_eq!(col, local_col, "cursor {cursor}, width {width}");
                assert_eq!(full.rows[row].start, window.rows[local].start);
                assert_eq!(full.rows[row].text, window.rows[local].text);
                if row > 0 {
                    assert!(local > 0);
                    assert_eq!(full.rows[row - 1].start, window.rows[local - 1].start);
                }
                if row + 1 < full.rows.len() {
                    assert_eq!(full.rows[row + 1].start, window.rows[local + 1].start);
                }
            }
        }
    }

    #[test]
    fn composer_view_large_ascii_wrap_has_bounded_unchanged_frame_cost() {
        let input = "x".repeat(1_000_001);
        let started = std::time::Instant::now();
        let mut last = None;
        for _ in 0..100 {
            last = Some(composer_view(&input, 80, 2, input.len()));
        }
        let elapsed = started.elapsed();
        let view = last.expect("rendered");
        let text = flatten(&view.lines);
        assert!(text.contains("long draft") && !text.contains("…"), "{text}");
        assert!(
            view.lines
                .iter()
                .all(|line| line_text(line).chars().count() <= 80)
        );
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "100 large-draft frames took {elapsed:?}"
        );
    }

    #[test]
    fn composer_view_large_ascii_selection_keeps_bounded_unchanged_frame_cost() {
        let input = "x".repeat(1_000_001);
        let selection = Some((input.len() - 12, input.len()));
        let started = std::time::Instant::now();
        let mut last = None;
        for _ in 0..100 {
            last = Some(composer_view_with_selection(
                &input,
                80,
                3,
                input.len(),
                selection,
            ));
        }
        let elapsed = started.elapsed();
        let view = last.expect("rendered");
        assert_eq!(
            view.lines
                .iter()
                .flat_map(|line| &line.spans)
                .filter(|span| span.style.bg == Some(HUD_BLUE))
                .map(|span| span.content.chars().count())
                .sum::<usize>(),
            12
        );
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "100 selected large-draft frames took {elapsed:?}"
        );

        let off_window = composer_view_with_selection(&input, 80, 2, input.len(), Some((0, 12)));
        assert!(
            off_window
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .all(|span| span.style.bg != Some(HUD_BLUE)),
            "an off-window selection is counted by the title but not painted onto unrelated tail text"
        );
    }

    #[test]
    fn fixed_zero_working_lines_resolves_known_agents_only() {
        for label in [
            "practice", "spark", "spark-r1", "turbo", "atlas", "gemma", "moa", "deli",
        ] {
            let lines = fixed_zero_working_lines(label)
                .unwrap_or_else(|| panic!("known agent {label} has a zero-working portrait"));
            // Two cached lines: a header naming the agent + a "pending" subline.
            assert_eq!(lines.len(), 2);
            let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(header.contains(label), "header names {label}: {header}");
        }
        // An unknown label has no fixed portrait.
        assert!(fixed_zero_working_lines("nonesuch").is_none());
    }
}
