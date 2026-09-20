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
#[path = "../../../tests/cockpit/app/status_view__tests.rs"]
mod tests;
