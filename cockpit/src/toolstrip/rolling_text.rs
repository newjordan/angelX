//! Overflow-only secondary text. No timed reveal, split graphemes, or catch-up
//! after a pane was hidden. The caller supplies the shared monotonic frame time.
use ratatui::{style::Style, text::Span};
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

const DWELL_MS: u128 = 1_000;
const STEP_MS: u128 = 160;

#[derive(Default)]
pub(super) struct RollingText {
    text: String,
    total: usize,
    last: Option<(Instant, bool)>,
    elapsed: Duration,
    width: usize,
}

impl RollingText {
    pub(super) fn new(text: String) -> Self {
        let total = UnicodeWidthStr::width(text.as_str());
        Self {
            text,
            total,
            ..Self::default()
        }
    }

    pub(super) fn render(
        &mut self,
        width: usize,
        now: Instant,
        enabled: bool,
        paused: bool,
    ) -> String {
        if self.width != width || !enabled {
            self.elapsed = Duration::ZERO;
            self.width = width;
        }
        let running = enabled && !paused && self.total > width;
        if let Some((last, was_running)) = self.last.replace((now, running)) {
            let dt = now.saturating_duration_since(last);
            // A hidden pane does no work and resumes its previous position.
            if running && was_running && dt <= Duration::from_millis(250) {
                self.elapsed = self.elapsed.saturating_add(dt);
            }
        }
        if !enabled {
            return super::ellipsize_with_width(&self.text, width, self.total);
        }
        window_with_width(&self.text, self.total, width, self.elapsed)
    }
}

#[cfg(test)]
fn window(text: &str, width: usize, elapsed: Duration) -> String {
    let total = UnicodeWidthStr::width(text);
    window_with_width(text, total, width, elapsed)
}

fn window_with_width(text: &str, total: usize, width: usize, elapsed: Duration) -> String {
    if total <= width || width <= 2 {
        return super::ellipsize_with_width(text, width, total);
    }
    // Fixed marker cells keep the reading window stable at both ends.
    let view = width - 2;
    let distance = total - view;
    let travel = distance as u128 * STEP_MS;
    let phase = elapsed.as_millis() % (2 * (DWELL_MS + travel));
    let offset = if phase <= DWELL_MS {
        0
    } else if phase <= DWELL_MS + travel {
        ((phase - DWELL_MS) / STEP_MS) as usize
    } else if phase <= 2 * DWELL_MS + travel {
        distance
    } else {
        distance - ((phase - 2 * DWELL_MS - travel) / STEP_MS) as usize
    };
    let mut out = String::with_capacity(width);
    out.push(if offset == 0 { ' ' } else { '…' });
    let span = Span::raw(text);
    let mut pos = 0;
    let mut used = 0;
    for grapheme in span.styled_graphemes(Style::new()) {
        let cells = UnicodeWidthStr::width(grapheme.symbol);
        let end = pos + cells;
        if end > offset && pos < offset + view {
            let visible = end.min(offset + view) - pos.max(offset);
            if pos >= offset && end <= offset + view {
                out.push_str(grapheme.symbol);
            } else {
                // Never paint half a wide glyph or detach a combining mark.
                out.extend(std::iter::repeat_n(' ', visible));
            }
            used += visible;
        }
        pos = end;
        if pos >= offset + view {
            break;
        }
    }
    out.extend(std::iter::repeat_n(' ', view - used));
    out.push(if offset == distance { ' ' } else { '…' });
    out
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/toolstrip__rolling_text__tests.rs"]
mod tests;
