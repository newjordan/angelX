//! Markdown → ratatui rendering.
//!
//! Walks a [`pulldown_cmark`] event stream and builds styled [`Line`]s with:
//! - Headings (bold + colored by level; gold sigils on H1/H2)
//! - Bold / italic / strikethrough / inline code spans
//! - Fenced code blocks with syntect syntax highlighting (rounded frame)
//! - Unordered / ordered lists (`▸` top-level, `·` nested)
//! - Block quotes (dim `▎ ` rail)
//! - Horizontal rules (dim dotted-end hairlines)
//! - Links (cyan, shows text; URL in dim parens when different)
//! - Paragraphs with word-wrap and intentional blank separators
//!
//! The renderer holds the syntect SyntaxSet + ThemeSet as static state (loaded
//! once, reused across all messages — they're ~5MB of grammar data).

use crate::ui::hud::{HUD_BLUE, HUD_DIM, HUD_GOLD, HUD_PHOSPHOR, HUD_TEXT};
use crate::ui::math;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const EMPTY_STYLE: Style = Style::new();
const BULLET_STYLE: Style = Style::new().fg(HUD_BLUE);
const LINK_STYLE: Style = Style::new()
    .fg(HUD_PHOSPHOR)
    .add_modifier(Modifier::UNDERLINED);
const DIM_STYLE: Style = Style::new().fg(HUD_DIM);
const INLINE_CODE_STYLE: Style = Style::new().fg(HUD_GOLD);
const CODE_TAB_STYLE: Style = Style::new().fg(HUD_GOLD);
const TASK_STYLE: Style = Style::new().fg(Color::Rgb(99, 241, 169));
const IMAGE_STYLE: Style = Style::new().fg(HUD_DIM).add_modifier(Modifier::ITALIC);
const BOLD_STYLE: Style = Style::new().add_modifier(Modifier::BOLD);
const ITALIC_STYLE: Style = Style::new().add_modifier(Modifier::ITALIC);
const CROSSED_OUT_STYLE: Style = Style::new().add_modifier(Modifier::CROSSED_OUT);
const MATH_STYLE: Style = Style::new().fg(HUD_TEXT);
const DISPLAY_MATH_STYLE: Style = Style::new().fg(HUD_TEXT).add_modifier(Modifier::BOLD);

struct SyntaxState {
    syntax_set: syntect::parsing::SyntaxSet,
    theme_set: syntect::highlighting::ThemeSet,
    theme_name: String,
}

static SYNTAX: OnceLock<SyntaxState> = OnceLock::new();

fn syntax() -> &'static SyntaxState {
    SYNTAX.get_or_init(|| SyntaxState {
        syntax_set: syntect::parsing::SyntaxSet::load_defaults_newlines(),
        theme_set: syntect::highlighting::ThemeSet::load_defaults(),
        theme_name: "base16-ocean.dark".to_string(),
    })
}

static SYNTAX_HIGHLIGHTING_ENABLED: AtomicBool = AtomicBool::new(true);
static SYNTAX_HIGHLIGHTING_SEEDED: AtomicBool = AtomicBool::new(false);

fn syntax_highlighting_from_env() -> bool {
    !matches!(
        std::env::var("ANGEL_MARKDOWN_SYNTAX")
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref(),
        Ok("0") | Ok("off") | Ok("false") | Ok("no")
    )
}

fn seed_syntax_highlighting_from_env() {
    if SYNTAX_HIGHLIGHTING_SEEDED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        SYNTAX_HIGHLIGHTING_ENABLED.store(syntax_highlighting_from_env(), Ordering::Relaxed);
    }
}

fn syntax_highlighting_enabled() -> bool {
    seed_syntax_highlighting_from_env();
    SYNTAX_HIGHLIGHTING_ENABLED.load(Ordering::Relaxed)
}

#[cfg(test)]
fn set_syntax_highlighting_enabled(on: bool) {
    SYNTAX_HIGHLIGHTING_SEEDED.store(true, Ordering::Relaxed);
    SYNTAX_HIGHLIGHTING_ENABLED.store(on, Ordering::Relaxed);
}

#[cfg(test)]
fn resync_syntax_highlighting_from_env() {
    SYNTAX_HIGHLIGHTING_SEEDED.store(false, Ordering::Relaxed);
    seed_syntax_highlighting_from_env();
}

pub fn render(text: &str, width: usize) -> Arc<Vec<Line<'static>>> {
    RENDER_CACHE.with(|c| c.borrow_mut().get_or_render(text, width))
}

pub fn render_partial(text: &str, width: usize) -> Arc<Vec<Line<'static>>> {
    PARTIAL_CACHE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some((t, w, lines)) = slot.as_ref()
            && *w == width
            && t.as_ref() == text
        {
            return Arc::clone(lines);
        }
        let lines = Arc::new(render_uncached(text, width));
        *slot = Some((Arc::from(text), width, Arc::clone(&lines)));
        lines
    })
}

fn render_uncached(text: &str, width: usize) -> Vec<Line<'static>> {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_MATH;
    let normalized = normalize_tex_delimiters(text);
    let parser = Parser::new_ext(&normalized, opts);
    let mut walker = Walker::new(width);
    for event in parser {
        walker.handle(event);
    }
    walker.finish()
}

fn normalize_tex_delimiters(text: &str) -> Cow<'_, str> {
    if !text.contains(r"\(")
        && !text.contains(r"\)")
        && !text.contains(r"\[")
        && !text.contains(r"\]")
    {
        return Cow::Borrowed(text);
    }

    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut at = 0usize;
    let mut line_start = true;
    let mut fence: Option<(u8, usize)> = None;
    let mut code_ticks: Option<usize> = None;

    while at < bytes.len() {
        if line_start {
            if let (None, Some((marker, count))) = (code_ticks, fence_marker(&bytes[at..])) {
                match fence {
                    Some((open, minimum)) if open == marker && count >= minimum => fence = None,
                    None => fence = Some((marker, count)),
                    _ => {}
                }
            }
            line_start = false;
        }

        if bytes[at] == b'\n' {
            out.push('\n');
            at += 1;
            line_start = true;
            continue;
        }

        if fence.is_none() && bytes[at] == b'`' {
            let count = byte_run(bytes, at, b'`');
            match code_ticks {
                Some(open) if count == open => code_ticks = None,
                None => code_ticks = Some(count),
                _ => {}
            }
            out.push_str(&text[at..at + count]);
            at += count;
            continue;
        }

        if fence.is_none()
            && code_ticks.is_none()
            && bytes[at] == b'\\'
            && (at == 0 || bytes[at - 1] != b'\\')
            && bytes.get(at + 1).is_some()
        {
            let replacement = match bytes[at + 1] {
                b'(' | b')' => Some("$"),
                b'[' | b']' => Some("$$"),
                _ => None,
            };
            if let Some(replacement) = replacement {
                out.push_str(replacement);
                at += 2;
                continue;
            }
        }

        let ch = text[at..].chars().next().expect("valid UTF-8 boundary");
        out.push(ch);
        at += ch.len_utf8();
    }
    Cow::Owned(out)
}

fn byte_run(bytes: &[u8], start: usize, needle: u8) -> usize {
    bytes[start..]
        .iter()
        .take_while(|byte| **byte == needle)
        .count()
}

fn fence_marker(line: &[u8]) -> Option<(u8, usize)> {
    let indent = line.iter().take_while(|byte| **byte == b' ').count().min(4);
    if indent > 3 {
        return None;
    }
    let marker = *line.get(indent)?;
    if !matches!(marker, b'`' | b'~') {
        return None;
    }
    let count = byte_run(line, indent, marker);
    (count >= 3).then_some((marker, count))
}

const RENDER_CACHE_CAP: usize = 128;

type PartialRenderCache = Option<(Arc<str>, usize, Arc<Vec<Line<'static>>>)>;

thread_local! {
    static RENDER_CACHE: RefCell<RenderCache> = RefCell::new(RenderCache::default());
    static PARTIAL_CACHE: RefCell<PartialRenderCache> = const { RefCell::new(None) };
}

struct CacheEntry {
    content: String,
    width: usize,
    lines: Arc<Vec<Line<'static>>>,
    last_used: u64,
}

#[derive(Default)]
struct RenderCache {
    map: HashMap<u64, CacheEntry>,
    tick: u64,
}

impl RenderCache {
    fn get_or_render(&mut self, text: &str, width: usize) -> Arc<Vec<Line<'static>>> {
        self.tick = self.tick.wrapping_add(1);
        let key = hash_key(text, width);
        if let Some(e) = self.map.get_mut(&key)
            && e.width == width
            && e.content == text
        {
            e.last_used = self.tick;
            return Arc::clone(&e.lines);
        }
        let lines = Arc::new(render_uncached(text, width));
        self.insert(key, text.to_string(), width, Arc::clone(&lines));
        lines
    }

    fn insert(&mut self, key: u64, content: String, width: usize, lines: Arc<Vec<Line<'static>>>) {
        if self.map.len() >= RENDER_CACHE_CAP && !self.map.contains_key(&key) {
            if let Some(victim) = self
                .map
                .iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(k, _)| *k)
            {
                self.map.remove(&victim);
            }
        }
        self.map.insert(
            key,
            CacheEntry {
                content,
                width,
                lines,
                last_used: self.tick,
            },
        );
    }
}

fn hash_key(text: &str, width: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    width.hash(&mut h);
    h.finish()
}

#[allow(dead_code)]
pub fn render_plain(text: &str, style: Style) -> Vec<Line<'static>> {
    text.lines()
        .map(|l| Line::from(vec![Span::styled(l.to_string(), style)]))
        .collect()
}

#[derive(Clone, Debug)]
pub(crate) struct MarkdownLink {
    pub(crate) line: usize,
    pub(crate) span: usize,
    pub(crate) target: std::sync::Arc<str>,
}

type LinkCacheEntry = (String, usize, Arc<Vec<MarkdownLink>>);
thread_local! {
    static LINK_CACHE: RefCell<std::collections::VecDeque<LinkCacheEntry>> = const {
        RefCell::new(std::collections::VecDeque::new())
    };
}

pub(crate) fn links(text: &str, width: usize) -> Arc<Vec<MarkdownLink>> {
    LINK_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(index) = cache.iter().position(|(t, w, _)| t == text && *w == width) {
            let entry = cache.remove(index).unwrap();
            let result = Arc::clone(&entry.2);
            cache.push_front(entry);
            return result;
        }
        let normalized = normalize_tex_delimiters(text);
        let parser = Parser::new_ext(
            &normalized,
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_MATH,
        );
        let mut walker = Walker::new(width);
        for event in parser {
            walker.handle(event);
        }
        let result = Arc::new(walker.link_spans);
        cache.push_front((text.to_owned(), width, Arc::clone(&result)));
        cache.truncate(16);
        result
    })
}

struct Walker {
    lines: Vec<Line<'static>>,
    link_target: Option<Arc<str>>,
    link_spans: Vec<MarkdownLink>,
    current: Vec<Span<'static>>,
    pending_text: bool,
    style_stack: Vec<Style>,
    list_depth: usize,
    list_counters: Vec<Option<usize>>,
    in_code_block: Option<Option<String>>,
    code_lines: Vec<String>,
    quote_depth: usize,
    width: usize,
}

impl Walker {
    fn new(width: usize) -> Self {
        Self {
            lines: Vec::new(),
            link_target: None,
            link_spans: Vec::new(),
            current: Vec::new(),
            pending_text: false,
            style_stack: Vec::new(),
            list_depth: 0,
            list_counters: Vec::new(),
            in_code_block: None,
            code_lines: Vec::new(),
            quote_depth: 0,
            width,
        }
    }

    fn style(&self) -> Style {
        self.style_stack
            .iter()
            .copied()
            .fold(Style::new(), |acc, s| acc.patch(s))
    }

    fn push_span(&mut self, text: impl Into<String>, style: Style) {
        let text = text.into();
        if !text.is_empty() {
            if let Some(target) = &self.link_target {
                self.link_spans.push(MarkdownLink {
                    line: self.lines.len(),
                    span: self.current.len() + usize::from(self.quote_depth > 0),
                    target: Arc::clone(target),
                });
            }
            self.current.push(Span::styled(text, style));
            self.pending_text = true;
        }
    }

    fn flush_line(&mut self) {
        let mut spans = Vec::new();
        if self.quote_depth > 0 {
            spans.push(Span::styled("▎ ".repeat(self.quote_depth), DIM_STYLE));
        }
        spans.append(&mut self.current);
        if spans.is_empty() {
            self.lines.push(Line::raw(""));
        } else {
            self.lines.push(Line::from(spans));
        }
        self.pending_text = false;
    }

    fn blank_line(&mut self) {
        if self.pending_text || !self.current.is_empty() {
            self.flush_line();
        }
        if self.lines.last().is_some_and(|line| !line.spans.is_empty()) {
            self.lines.push(Line::raw(""));
        }
    }

    fn indent(&self) -> String {
        "  ".repeat(self.list_depth.saturating_sub(1))
    }

    fn handle(&mut self, event: Event) {
        if self.in_code_block.is_some() {
            match event {
                Event::Text(t) => self.code_lines.push(t.into_string()),
                Event::End(TagEnd::CodeBlock) => {
                    self.emit_code_block();
                    self.in_code_block = None;
                    self.code_lines.clear();
                }
                _ => {}
            }
            return;
        }

        match event {
            Event::Text(t) => {
                let mut parts = t.split('\n');
                if let Some(first) = parts.next()
                    && !first.is_empty()
                {
                    self.push_span(first, self.style());
                }
                for rest in parts {
                    self.flush_line();
                    if !rest.is_empty() {
                        self.push_span(rest, self.style());
                    }
                }
            }
            Event::Code(t) => {
                self.push_span(t.into_string(), self.style().patch(INLINE_CODE_STYLE));
            }
            Event::InlineMath(t) => {
                self.push_span(math::inline(&t), self.style().patch(MATH_STYLE));
            }
            Event::DisplayMath(t) => {
                self.blank_line();
                for row in math::display(&t, self.width) {
                    self.push_span(row, DISPLAY_MATH_STYLE);
                    self.flush_line();
                }
                self.blank_line();
            }
            Event::SoftBreak => self.flush_line(),
            Event::HardBreak => {
                self.flush_line();
            }

            Event::Start(Tag::Strong) => self.style_stack.push(BOLD_STYLE),
            Event::End(TagEnd::Strong) => {
                self.style_stack.pop();
            }
            Event::Start(Tag::Emphasis) => self.style_stack.push(ITALIC_STYLE),
            Event::End(TagEnd::Emphasis) => {
                self.style_stack.pop();
            }
            Event::Start(Tag::Strikethrough) => self.style_stack.push(CROSSED_OUT_STYLE),
            Event::End(TagEnd::Strikethrough) => {
                self.style_stack.pop();
            }

            Event::Start(Tag::Link { dest_url, .. }) => {
                self.link_target = Some(Arc::from(dest_url.as_ref()));
                self.style_stack.push(LINK_STYLE);
            }
            Event::End(TagEnd::Link) => {
                self.link_target = None;
                self.style_stack.pop();
            }

            Event::Start(Tag::Heading { level, .. }) => {
                self.blank_line();
                let (color, modi) = heading_style(level);
                self.style_stack
                    .push(Style::new().fg(color).add_modifier(modi));
                match level {
                    HeadingLevel::H1 => self.push_span("▌ ", CODE_TAB_STYLE),
                    HeadingLevel::H2 => self.push_span("▎ ", CODE_TAB_STYLE),
                    HeadingLevel::H3 => self.push_span("· ", CODE_TAB_STYLE),
                    _ => {}
                }
            }
            Event::End(TagEnd::Heading(level)) => {
                self.style_stack.pop();
                self.flush_line();
                if level == HeadingLevel::H1 {
                    let rule = "─".repeat(self.width.clamp(10, 48));
                    self.lines
                        .push(Line::from(vec![Span::styled(rule, DIM_STYLE)]));
                }
                self.blank_line();
            }

            Event::Start(Tag::Paragraph) if !self.lines.is_empty() => {
                self.blank_line();
            }
            Event::End(TagEnd::Paragraph) => {
                self.flush_line();
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                self.blank_line();
                let lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(s) => Some(s.into_string()),
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                self.in_code_block = Some(lang);
            }

            Event::Start(Tag::List(start)) => {
                if self.list_depth == 0 {
                    self.blank_line();
                }
                self.list_depth += 1;
                self.list_counters.push(start.map(|n| n as usize));
            }
            Event::End(TagEnd::List(false)) => {
                self.list_depth = self.list_depth.saturating_sub(1);
                self.list_counters.pop();
                if self.list_depth == 0 {
                    self.blank_line();
                }
            }
            Event::End(TagEnd::List(true)) => {
                self.list_depth = self.list_depth.saturating_sub(1);
                self.list_counters.pop();
                if self.list_depth == 0 {
                    self.blank_line();
                }
            }
            Event::Start(Tag::Item) => {
                if self.pending_text {
                    self.flush_line();
                }
                let indent = self.indent();
                if let Some(depth) = self.list_depth.checked_sub(1) {
                    if let Some(Some(counter)) = self.list_counters.get_mut(depth) {
                        let n = *counter;
                        *counter += 1;
                        self.push_span(format!("{indent}{n}. "), BULLET_STYLE);
                    } else {
                        let bullet = if depth == 0 { "▸ " } else { "· " };
                        self.push_span(format!("{indent}{bullet}"), BULLET_STYLE);
                    }
                } else {
                    self.push_span("▸ ", BULLET_STYLE);
                }
            }
            Event::End(TagEnd::Item) => {
                self.flush_line();
            }

            Event::Start(Tag::BlockQuote(_)) => {
                self.blank_line();
                self.quote_depth += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                if self.pending_text {
                    self.flush_line();
                }
                self.quote_depth = self.quote_depth.saturating_sub(1);
                self.blank_line();
            }

            Event::Rule => {
                self.blank_line();
                let inner = self.width.clamp(8, 56).saturating_sub(2);
                let bar = format!("·{}·", "─".repeat(inner));
                self.lines
                    .push(Line::from(vec![Span::styled(bar, DIM_STYLE)]));
                self.blank_line();
            }

            Event::Start(Tag::Table(_))
            | Event::Start(Tag::TableHead)
            | Event::Start(Tag::TableRow) => {
            }
            Event::Start(Tag::TableCell) => {
                self.push_span(" ", EMPTY_STYLE);
            }
            Event::End(TagEnd::TableCell) => {
                self.push_span(" │ ", DIM_STYLE);
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                self.flush_line();
            }
            Event::End(TagEnd::Table) => {
                self.blank_line();
            }

            Event::Start(Tag::Image { .. }) => {
                self.push_span("[", IMAGE_STYLE);
            }
            Event::End(TagEnd::Image) => {
                self.push_span("]", IMAGE_STYLE);
            }

            Event::FootnoteReference(f) => {
                self.push_span(format!("[{}]", f), DIM_STYLE);
            }
            Event::TaskListMarker(checked) => {
                let mark = if checked { "☑ " } else { "☐ " };
                self.push_span(mark, TASK_STYLE);
            }
            Event::InlineHtml(h) | Event::Html(h) => {
                self.push_span(h.into_string(), DIM_STYLE);
            }
            _ => {}
        }
    }

    fn emit_code_block(&mut self) {
        let lang_str = self.in_code_block.as_ref().and_then(|l| l.as_deref());
        let syntax_state = syntax_highlighting_enabled().then(syntax);
        let syntax = syntax_state.and_then(|st| {
            lang_str.and_then(|l| {
                st.syntax_set
                    .find_syntax_by_token(l)
                    .or_else(|| st.syntax_set.find_syntax_by_extension(l))
            })
        });

        let label = lang_str.unwrap_or("");
        self.lines.push(code_frame_top(self.width, label));

        let source = self.code_lines.concat();
        let physical_lines = physical_code_lines(&source);

        let mut highlighted: Vec<Vec<(syntect::highlighting::Style, String)>> = Vec::new();

        if let (Some(st), Some(syntax)) = (syntax_state, syntax) {
            use syntect::easy::HighlightLines;
            let theme = &st.theme_set.themes[&st.theme_name];
            let mut h = HighlightLines::new(syntax, theme);
            for raw_line in &physical_lines {
                let syntax_line = format!("{raw_line}\n");
                let regions: Vec<(syntect::highlighting::Style, &str)> = h
                    .highlight_line(&syntax_line, &st.syntax_set)
                    .unwrap_or_default();
                let owned: Vec<(syntect::highlighting::Style, String)> = regions
                    .into_iter()
                    .map(|(style, text)| (style, text.trim_end_matches(['\r', '\n']).to_string()))
                    .collect();
                highlighted.push(owned);
            }
        } else {
            for raw_line in &physical_lines {
                highlighted.push(vec![(
                    syntect::highlighting::Style::default(),
                    raw_line.clone(),
                )]);
            }
        }

        for regions in &highlighted {
            let spans: Vec<Span> = regions
                .iter()
                .map(|(style, text)| Span::styled(text.clone(), syntect_to_ratatui_style(*style)))
                .collect();
            self.lines.extend(wrap_code_spans(spans, self.width));
        }

        self.lines.push(code_frame_bottom(self.width));
        self.blank_line();
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        if self.pending_text || !self.current.is_empty() {
            self.flush_line();
        }
        while self.lines.last().is_some_and(|line| line.spans.is_empty()) {
            self.lines.pop();
        }
        self.lines
    }
}

fn physical_code_lines(source: &str) -> Vec<String> {
    let mut lines: Vec<&str> = source.split('\n').collect();
    if source.ends_with('\n') {
        lines.pop();
    }
    lines
        .into_iter()
        .map(|line| expand_code_tabs(line.strip_suffix('\r').unwrap_or(line)))
        .collect()
}

fn expand_code_tabs(line: &str) -> String {
    const TAB_STOP: usize = 4;
    let mut out = String::with_capacity(line.len());
    let mut column = 0usize;
    for ch in line.chars() {
        if ch == '\t' {
            let spaces = TAB_STOP - column % TAB_STOP;
            out.push_str(&" ".repeat(spaces));
            column += spaces;
        } else if !ch.is_control() {
            out.push(ch);
            column += UnicodeWidthChar::width(ch).unwrap_or(0);
        }
    }
    out
}

fn code_frame_top(width: usize, label: &str) -> Line<'static> {
    let frame_width = width.min(58);
    if frame_width == 0 {
        return Line::raw("");
    }
    let prefix = match frame_width {
        1 => "╭",
        2 => " ╭",
        _ => " ╭─",
    };
    let mut spans = vec![Span::styled(prefix.to_string(), DIM_STYLE)];
    let mut used = UnicodeWidthStr::width(prefix);
    let available = frame_width.saturating_sub(used);
    if !label.is_empty() && available >= 3 {
        let fitted = fit_cells(label, available - 2);
        if !fitted.is_empty() {
            let tab = format!(" {fitted} ");
            used += UnicodeWidthStr::width(tab.as_str());
            spans.push(Span::styled(tab, CODE_TAB_STYLE));
        }
    }
    spans.push(Span::styled(
        "─".repeat(frame_width.saturating_sub(used)),
        DIM_STYLE,
    ));
    Line::from(spans)
}

fn code_frame_bottom(width: usize) -> Line<'static> {
    let frame_width = width.min(58);
    let text = match frame_width {
        0 => String::new(),
        1 => "╰".to_string(),
        _ => format!(" ╰{}", "─".repeat(frame_width - 2)),
    };
    Line::from(Span::styled(text, DIM_STYLE))
}

fn fit_cells(text: &str, max_cells: usize) -> String {
    if UnicodeWidthStr::width(text) <= max_cells {
        return text.to_string();
    }
    if max_cells == 0 {
        return String::new();
    }
    let keep = max_cells.saturating_sub(1);
    let mut fitted = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let cells = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used.saturating_add(cells) > keep {
            break;
        }
        fitted.push(ch);
        used += cells;
    }
    fitted.push('…');
    fitted
}

fn wrap_code_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Line<'static>> {
    if width == 0 || spans.is_empty() {
        return vec![Line::raw("")];
    }

    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut row_width = 0usize;
    for span in &spans {
        let mut chunk = String::new();
        for grapheme in span.styled_graphemes(Style::new()) {
            let cells = UnicodeWidthStr::width(grapheme.symbol);
            if cells > width {
                if !chunk.is_empty() {
                    row.push(Span::styled(std::mem::take(&mut chunk), span.style));
                }
                if !row.is_empty() {
                    rows.push(Line::from(std::mem::take(&mut row)));
                    row_width = 0;
                }
                rows.push(Line::from(Span::styled("…", span.style)));
                continue;
            }
            if cells > 0 && row_width.saturating_add(cells) > width {
                if !chunk.is_empty() {
                    row.push(Span::styled(std::mem::take(&mut chunk), span.style));
                }
                rows.push(Line::from(std::mem::take(&mut row)));
                row_width = 0;
            }
            chunk.push_str(grapheme.symbol);
            row_width += cells;
        }
        if !chunk.is_empty() {
            row.push(Span::styled(chunk, span.style));
        }
    }
    if !row.is_empty() {
        rows.push(Line::from(row));
    }
    if rows.is_empty() {
        rows.push(Line::raw(""));
    }
    rows
}

fn heading_style(level: HeadingLevel) -> (Color, Modifier) {
    match level {
        HeadingLevel::H1 => (HUD_TEXT, Modifier::BOLD),
        HeadingLevel::H2 => (HUD_BLUE, Modifier::BOLD),
        HeadingLevel::H3 => (HUD_PHOSPHOR, Modifier::BOLD),
        HeadingLevel::H4 => (HUD_BLUE, Modifier::empty()),
        HeadingLevel::H5 => (HUD_DIM, Modifier::BOLD),
        HeadingLevel::H6 => (HUD_DIM, Modifier::ITALIC),
    }
}

fn syntect_to_ratatui_style(s: syntect::highlighting::Style) -> Style {
    let fg = s.foreground;
    let mut style = if fg.r == 0 && fg.g == 0 && fg.b == 0 {
        Style::new().fg(Color::Rgb(51, 255, 51))
    } else {
        Style::new().fg(Color::Rgb(fg.r, fg.g, fg.b))
    };
    if s.font_style
        .contains(syntect::highlighting::FontStyle::BOLD)
    {
        style = style.add_modifier(Modifier::BOLD);
    }
    if s.font_style
        .contains(syntect::highlighting::FontStyle::UNDERLINE)
    {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/markdown__tests.rs"]
mod tests;
