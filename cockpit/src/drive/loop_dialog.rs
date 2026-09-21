use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoopDialogFocus {
    Duration,
    Iterations,
    Budget,
    Start,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoopDialogAction {
    SelectDuration(usize),
    SelectIterations(usize),
    SelectBudget(usize),
    Start,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LoopDialogHit {
    pub(crate) rect: Rect,
    pub(crate) action: LoopDialogAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LoopLaunchSettings {
    pub(crate) deadline_secs: u64,
    pub(crate) max_iters: usize,
    pub(crate) token_budget: usize,
    pub(crate) podrace: bool,
}

/// Who owns the workshop Start action — agent `/loop` or handoff-RL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LoopLaunchPurpose {
    #[default]
    AgentLoop,
    HandoffRl,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoopLaunchDialog {
    pub(crate) task: String,
    pub(crate) interval_secs: u64,
    pub(crate) deli: bool,
    pub(crate) purpose: LoopLaunchPurpose,
    duration_idx: usize,
    iterations_idx: usize,
    budget_idx: usize,
    custom_duration: Option<u64>,
    custom_iterations: Option<usize>,
    custom_budget: Option<usize>,
    podrace: bool,
    focus: LoopDialogFocus,
    /// The typed buffer while the operator sets a custom loop length. `Some`
    /// means the iters row is in entry mode: digits edit it, Enter commits the
    /// length, Esc drops it and the row returns to the choice it left.
    custom_entry: Option<String>,
    /// The choice the iters row falls back to when the entry is dropped.
    pre_custom_idx: usize,
}

#[derive(Clone, Copy)]
struct DurationChoice {
    label: &'static str,
    secs: u64,
}

#[derive(Clone, Copy)]
struct IterChoice {
    label: &'static str,
    iters: usize,
}

#[derive(Clone, Copy)]
struct BudgetChoice {
    label: &'static str,
    tokens: usize,
}

const DURATIONS: &[DurationChoice] = &[
    DurationChoice {
        label: "15m",
        secs: 15 * 60,
    },
    DurationChoice {
        label: "1h",
        secs: 60 * 60,
    },
    DurationChoice {
        label: "4h",
        secs: 4 * 60 * 60,
    },
    DurationChoice {
        label: "5d",
        secs: 5 * 24 * 60 * 60,
    },
    DurationChoice {
        label: "endless",
        secs: 0,
    },
];

const ITERATIONS: &[IterChoice] = &[
    IterChoice {
        label: "5",
        iters: 5,
    },
    IterChoice {
        label: "25",
        iters: 25,
    },
    IterChoice {
        label: "100",
        iters: 100,
    },
    IterChoice {
        label: "endless",
        iters: 0,
    },
];

/// The always-present iters choice that opens the typed length entry.
const CUSTOM_LENGTH_LABEL: &str = "set custom";

const BUDGETS: &[BudgetChoice] = &[
    BudgetChoice {
        label: "250k",
        tokens: 250_000,
    },
    BudgetChoice {
        label: "1m",
        tokens: 1_000_000,
    },
    BudgetChoice {
        label: "2m",
        tokens: 2_000_000,
    },
    BudgetChoice {
        label: "endless",
        tokens: 0,
    },
];

struct ChoiceRow<'a> {
    label: &'static str,
    choices: &'a [&'a str],
    selected: usize,
    focused: bool,
}

impl LoopLaunchDialog {
    pub(crate) fn new(task: impl Into<String>, interval_secs: u64, deli: bool) -> Self {
        Self {
            task: task.into(),
            interval_secs,
            deli,
            purpose: LoopLaunchPurpose::AgentLoop,
            duration_idx: DURATIONS.len() - 1,
            iterations_idx: ITERATIONS.len() - 1,
            budget_idx: BUDGETS.len() - 1,
            custom_duration: None,
            custom_iterations: None,
            custom_budget: None,
            podrace: false,
            focus: LoopDialogFocus::Duration,
            custom_entry: None,
            pre_custom_idx: ITERATIONS.len() - 1,
        }
    }

    /// Five-day competition profile: a fixed campaign deadline with no
    /// iteration or token cap. Per-turn watchdogs remain active so one dead
    /// provider attempt cannot pin the entire run.
    pub(crate) fn podrace(task: impl Into<String>, interval_secs: u64) -> Self {
        Self {
            task: task.into(),
            interval_secs,
            deli: false,
            purpose: LoopLaunchPurpose::AgentLoop,
            duration_idx: DURATIONS
                .iter()
                .position(|choice| choice.secs == 5 * 24 * 60 * 60)
                .expect("5d duration is built in"),
            iterations_idx: ITERATIONS.len() - 1,
            budget_idx: BUDGETS.len() - 1,
            custom_duration: None,
            custom_iterations: None,
            custom_budget: None,
            podrace: true,
            focus: LoopDialogFocus::Duration,
            custom_entry: None,
            pre_custom_idx: ITERATIONS.len() - 1,
        }
    }

    /// Same workshop controls as `/loop`, but Start arms the handoff-RL
    /// forced clear/inject campaign instead of the agent loop.
    ///
    /// All caps default to endless; only operator choices bound the campaign.
    pub(crate) fn handoff_rl(task: impl Into<String>, interval_secs: u64) -> Self {
        let mut dialog = Self::new(task, interval_secs, false);
        dialog.purpose = LoopLaunchPurpose::HandoffRl;
        dialog.duration_idx = DURATIONS.len() - 1;
        dialog.budget_idx = BUDGETS.len() - 1;
        dialog.iterations_idx = ITERATIONS.len() - 1; // endless rolls
        dialog
    }

    /// Podrace profile for handoff-RL: 5-day deadline, unlimited rolls/tokens.
    pub(crate) fn handoff_rl_podrace(task: impl Into<String>, interval_secs: u64) -> Self {
        let mut dialog = Self::podrace(task, interval_secs);
        dialog.purpose = LoopLaunchPurpose::HandoffRl;
        dialog
    }

    pub(crate) fn is_handoff_rl(&self) -> bool {
        matches!(self.purpose, LoopLaunchPurpose::HandoffRl)
    }

    pub(crate) fn settings(&self) -> LoopLaunchSettings {
        LoopLaunchSettings {
            deadline_secs: DURATIONS
                .get(self.duration_idx)
                .map(|c| c.secs)
                .or(self.custom_duration)
                .expect("selected duration exists"),
            max_iters: ITERATIONS
                .get(self.iterations_idx)
                .map(|c| c.iters)
                .or(self.custom_iterations)
                .expect("selected iteration cap exists"),
            token_budget: BUDGETS
                .get(self.budget_idx)
                .map(|c| c.tokens)
                .or(self.custom_budget)
                .expect("selected token cap exists"),
            podrace: self.podrace,
        }
    }

    /// Restore operator authority exactly, including values outside presets.
    /// Merely opening a restart dialog must never clear or round up a cap.
    pub(crate) fn restore_settings(&mut self, settings: LoopLaunchSettings) {
        self.duration_idx = DURATIONS
            .iter()
            .position(|c| c.secs == settings.deadline_secs)
            .unwrap_or(DURATIONS.len());
        self.iterations_idx = ITERATIONS
            .iter()
            .position(|c| c.iters == settings.max_iters)
            .unwrap_or(ITERATIONS.len());
        self.budget_idx = BUDGETS
            .iter()
            .position(|c| c.tokens == settings.token_budget)
            .unwrap_or(BUDGETS.len());
        self.custom_duration =
            (self.duration_idx == DURATIONS.len()).then_some(settings.deadline_secs);
        self.custom_iterations =
            (self.iterations_idx == ITERATIONS.len()).then_some(settings.max_iters);
        self.custom_budget = (self.budget_idx == BUDGETS.len()).then_some(settings.token_budget);
        self.podrace = settings.podrace;
    }

    pub(crate) fn focus_next(&mut self) {
        self.focus = match self.focus {
            LoopDialogFocus::Duration => LoopDialogFocus::Iterations,
            LoopDialogFocus::Iterations => LoopDialogFocus::Budget,
            LoopDialogFocus::Budget => LoopDialogFocus::Start,
            LoopDialogFocus::Start => LoopDialogFocus::Cancel,
            LoopDialogFocus::Cancel => LoopDialogFocus::Duration,
        };
    }

    pub(crate) fn focus_prev(&mut self) {
        self.focus = match self.focus {
            LoopDialogFocus::Duration => LoopDialogFocus::Cancel,
            LoopDialogFocus::Iterations => LoopDialogFocus::Duration,
            LoopDialogFocus::Budget => LoopDialogFocus::Iterations,
            LoopDialogFocus::Start => LoopDialogFocus::Budget,
            LoopDialogFocus::Cancel => LoopDialogFocus::Start,
        };
    }

    pub(crate) fn adjust(&mut self, forward: bool) {
        match self.focus {
            LoopDialogFocus::Duration => {
                self.duration_idx = cycle_index(
                    self.duration_idx,
                    DURATIONS.len() + usize::from(self.custom_duration.is_some()),
                    forward,
                );
            }
            LoopDialogFocus::Iterations => {
                if self.custom_entry.is_some() {
                    return; // the entry owns the row until it is committed or dropped
                }
                self.select_iterations(cycle_index(
                    self.iterations_idx,
                    // presets + the always-present custom-length slot
                    ITERATIONS.len() + 1,
                    forward,
                ));
            }
            LoopDialogFocus::Budget => {
                self.budget_idx = cycle_index(
                    self.budget_idx,
                    BUDGETS.len() + usize::from(self.custom_budget.is_some()),
                    forward,
                );
            }
            LoopDialogFocus::Start | LoopDialogFocus::Cancel => {
                self.focus = if matches!(self.focus, LoopDialogFocus::Start) {
                    LoopDialogFocus::Cancel
                } else {
                    LoopDialogFocus::Start
                };
            }
        }
    }

    pub(crate) fn apply(&mut self, action: LoopDialogAction) {
        match action {
            LoopDialogAction::SelectDuration(i) => {
                self.duration_idx =
                    i.min(DURATIONS.len() + usize::from(self.custom_duration.is_some()) - 1);
                self.focus = LoopDialogFocus::Duration;
            }
            LoopDialogAction::SelectIterations(i) => {
                self.select_iterations(i);
                self.focus = LoopDialogFocus::Iterations;
            }
            LoopDialogAction::SelectBudget(i) => {
                self.budget_idx =
                    i.min(BUDGETS.len() + usize::from(self.custom_budget.is_some()) - 1);
                self.focus = LoopDialogFocus::Budget;
            }
            LoopDialogAction::Start => self.focus = LoopDialogFocus::Start,
            LoopDialogAction::Cancel => self.focus = LoopDialogFocus::Cancel,
        }
    }

    fn select_iterations(&mut self, index: usize) {
        // The custom slot lives one past the presets. With a committed length it
        // is an ordinary choice; without one it opens the entry instead of
        // claiming a cap nobody set.
        if index >= ITERATIONS.len() {
            if self.custom_iterations.is_some() {
                self.iterations_idx = ITERATIONS.len();
            } else {
                self.begin_custom_length();
            }
            return;
        }
        self.iterations_idx = index;
        if self.settings().max_iters == 0 {
            self.duration_idx = DURATIONS.len() - 1;
            self.budget_idx = BUDGETS.len() - 1;
        }
    }

    /// The index of the always-present custom-length slot in the iters row.
    pub(crate) fn custom_length_slot() -> usize {
        ITERATIONS.len()
    }

    /// True while the iters row is taking a typed length.
    pub(crate) fn custom_entry_active(&self) -> bool {
        self.custom_entry.is_some()
    }

    /// The typed buffer, as it should appear on screen.
    pub(crate) fn custom_entry_text(&self) -> Option<&str> {
        self.custom_entry.as_deref()
    }

    /// Open the custom-length entry, seeded with the committed length if there
    /// is one, so re-opening it never loses what the operator set.
    pub(crate) fn begin_custom_length(&mut self) {
        if self.custom_entry.is_none() {
            self.pre_custom_idx = self.iterations_idx.min(ITERATIONS.len() - 1);
            self.custom_entry = Some(String::new());
        }
        if let Some(committed) = self.custom_iterations {
            if self.custom_entry.as_deref().is_some_and(str::is_empty) {
                self.custom_entry = Some(committed.to_string());
            }
        }
        self.focus = LoopDialogFocus::Iterations;
    }

    /// Type a digit into the entry. Returns whether the key was consumed.
    pub(crate) fn custom_length_digit(&mut self, ch: char) -> bool {
        let Some(buffer) = self.custom_entry.as_mut() else {
            return false;
        };
        if !ch.is_ascii_digit() {
            return false;
        }
        if buffer.len() >= 7 {
            return true; // consumed, but seven digits is the cap
        }
        if buffer == "0" {
            buffer.clear(); // no leading zero: 0 is not a length
        }
        buffer.push(ch);
        true
    }

    /// Delete the last typed digit.
    pub(crate) fn custom_length_backspace(&mut self) -> bool {
        let Some(buffer) = self.custom_entry.as_mut() else {
            return false;
        };
        buffer.pop();
        true
    }

    /// Commit the typed length. An empty entry, or a zero, is not a length: the
    /// row falls back to the choice it had before the entry opened.
    pub(crate) fn commit_custom_length(&mut self) -> bool {
        let Some(buffer) = self.custom_entry.take() else {
            return false;
        };
        match buffer.trim().parse::<usize>() {
            Ok(iters) if iters > 0 => {
                self.custom_iterations = Some(iters);
                self.iterations_idx = ITERATIONS.len();
                true
            }
            _ => {
                self.iterations_idx = self.pre_custom_idx.min(ITERATIONS.len() - 1);
                false
            }
        }
    }

    /// Drop the typed length and return the row to the choice it left.
    pub(crate) fn cancel_custom_length(&mut self) -> bool {
        if self.custom_entry.take().is_none() {
            return false;
        }
        self.iterations_idx = self.pre_custom_idx.min(ITERATIONS.len() - 1);
        true
    }

    pub(crate) fn focused_action(&self) -> Option<LoopDialogAction> {
        match self.focus {
            LoopDialogFocus::Start => Some(LoopDialogAction::Start),
            LoopDialogFocus::Cancel => Some(LoopDialogAction::Cancel),
            _ => None,
        }
    }
}

fn cycle_index(idx: usize, len: usize, forward: bool) -> usize {
    if len == 0 {
        return 0;
    }
    if forward {
        (idx + 1) % len
    } else {
        idx.checked_sub(1).unwrap_or(len - 1)
    }
}

pub(crate) fn render(
    frame: &mut Frame,
    root: Rect,
    dialog: &LoopLaunchDialog,
) -> Vec<LoopDialogHit> {
    let area = modal_area(root);
    let mut hits = Vec::new();
    if area.width == 0 || area.height == 0 {
        return hits;
    }
    frame.render_widget(Clear, area);
    let workshop_title = if dialog.is_handoff_rl() {
        " W> handoff-rl workshop "
    } else {
        " W> loop workshop "
    };
    let block = Block::default().borders(Borders::ALL).title(workshop_title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width < 24 || inner.height < 8 {
        frame.render_widget(
            Paragraph::new(if dialog.is_handoff_rl() {
                "handoff-rl workshop\nresize for controls"
            } else {
                "loop workshop\nresize for controls"
            })
            .style(dim()),
            inner,
        );
        return hits;
    }

    let mut y = inner.y;
    let title = if dialog.is_handoff_rl() {
        if dialog.podrace {
            "enter workshop · handoff-rl 5-day podrace"
        } else {
            "enter workshop · handoff-rl campaign"
        }
    } else if dialog.podrace {
        "enter workshop · 5-day competition podrace"
    } else if dialog.deli {
        "enter workshop · deli loop"
    } else {
        "enter workshop · agent loop"
    };
    let title = if dialog.custom_entry_active() {
        format!("{title} · set custom length")
    } else {
        title.to_string()
    };
    let task = if dialog.task.trim().is_empty() {
        "task: active goal".to_string()
    } else {
        fit_ascii(
            &format!("task: {}", dialog.task.trim()),
            inner.width.saturating_sub(28),
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(title, bright()),
            Span::raw("  "),
            Span::styled(task, dim()),
        ])),
        Rect::new(inner.x, y, inner.width, 1),
    );
    y = y.saturating_add(2);

    let custom_duration = dialog.custom_duration.map(|v| format!("{v}s"));
    let duration_choices: Vec<_> = DURATIONS
        .iter()
        .map(|c| c.label)
        .chain(custom_duration.as_deref())
        .collect();
    render_choices(
        frame,
        Rect::new(inner.x, y, inner.width, 1),
        ChoiceRow {
            label: "time",
            choices: &duration_choices,
            selected: dialog.duration_idx,
            focused: matches!(dialog.focus, LoopDialogFocus::Duration),
        },
        LoopDialogAction::SelectDuration,
        &mut hits,
    );
    y = y.saturating_add(1);
    let custom_iterations = dialog.custom_iterations.map(|v| v.to_string());
    // The custom-length slot is always on the row: it is how an operator asks
    // for a length the presets do not offer. While the entry is open it shows
    // the buffer with a caret instead of a label.
    let custom_length = match dialog.custom_entry_text() {
        Some(buffer) if buffer.is_empty() => format!("{CUSTOM_LENGTH_LABEL}_"),
        Some(buffer) => format!("{buffer}_"),
        None => custom_iterations.unwrap_or_else(|| CUSTOM_LENGTH_LABEL.to_string()),
    };
    let iteration_choices: Vec<_> = ITERATIONS
        .iter()
        .map(|c| c.label)
        .chain([custom_length.as_str()])
        .collect();
    let iters_label = if dialog.is_handoff_rl() {
        "rolls"
    } else {
        "iters"
    };
    render_choices(
        frame,
        Rect::new(inner.x, y, inner.width, 1),
        ChoiceRow {
            label: iters_label,
            choices: &iteration_choices,
            selected: dialog.iterations_idx,
            focused: matches!(dialog.focus, LoopDialogFocus::Iterations),
        },
        LoopDialogAction::SelectIterations,
        &mut hits,
    );
    y = y.saturating_add(1);
    let custom_budget = dialog.custom_budget.map(|v| v.to_string());
    let budget_choices: Vec<_> = BUDGETS
        .iter()
        .map(|c| c.label)
        .chain(custom_budget.as_deref())
        .collect();
    render_choices(
        frame,
        Rect::new(inner.x, y, inner.width, 1),
        ChoiceRow {
            label: "operator token cap",
            choices: &budget_choices,
            selected: dialog.budget_idx,
            focused: matches!(dialog.focus, LoopDialogFocus::Budget),
        },
        LoopDialogAction::SelectBudget,
        &mut hits,
    );
    y = y.saturating_add(2);

    let start = "  START  ";
    let cancel = "  BACK  ";
    let sx = inner.x + 2;
    let cx = sx + start.len() as u16 + 2;
    let start_rect = Rect::new(sx, y, start.len() as u16, 1);
    let cancel_rect = Rect::new(cx, y, cancel.len() as u16, 1);
    hits.push(LoopDialogHit {
        rect: start_rect,
        action: LoopDialogAction::Start,
    });
    hits.push(LoopDialogHit {
        rect: cancel_rect,
        action: LoopDialogAction::Cancel,
    });
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                start,
                button_style(matches!(dialog.focus, LoopDialogFocus::Start), true),
            ),
            Span::raw("  "),
            Span::styled(
                cancel,
                button_style(matches!(dialog.focus, LoopDialogFocus::Cancel), false),
            ),
        ])),
        Rect::new(sx, y, inner.width.saturating_sub(2), 1),
    );

    let help_y = inner.y + inner.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new(if dialog.custom_entry_active() {
            "digits set the length · Backspace edits · Enter sets it · Esc drops it"
        } else {
            "Tab/arrow keys or click · Enter starts · Esc exits"
        })
        .style(dim()),
        Rect::new(inner.x, help_y, inner.width, 1),
    );
    hits
}

fn render_choices(
    frame: &mut Frame,
    area: Rect,
    row: ChoiceRow<'_>,
    action: impl Fn(usize) -> LoopDialogAction,
    hits: &mut Vec<LoopDialogHit>,
) {
    let mut spans = Vec::new();
    let prefix = format!("{:<7}", row.label);
    let prefix_width = prefix.chars().count() as u16;
    spans.push(Span::styled(prefix, bright()));
    let mut x = area.x.saturating_add(prefix_width);
    // Keep the actual selection visible even when a custom cap would otherwise
    // be clipped after the presets. Hit actions retain their original indices.
    let selected_end = prefix_width as usize
        + row
            .choices
            .iter()
            .take(row.selected + 1)
            .map(|c| c.chars().count() + 3)
            .sum::<usize>();
    let first = if selected_end > area.width as usize {
        row.selected
    } else {
        0
    };
    for (idx, choice) in row
        .choices
        .iter()
        .enumerate()
        .cycle()
        .skip(first)
        .take(row.choices.len())
    {
        let text = format!(" {choice} ");
        let w = text.chars().count() as u16;
        if x.saturating_add(w) <= area.x.saturating_add(area.width) {
            hits.push(LoopDialogHit {
                rect: Rect::new(x, area.y, w, 1),
                action: action(idx),
            });
        }
        spans.push(Span::styled(
            text,
            button_style(row.focused && idx == row.selected, idx == row.selected),
        ));
        spans.push(Span::raw(" "));
        x = x.saturating_add(w + 1);
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn modal_area(root: Rect) -> Rect {
    let w = root.width.min(70).max(root.width.min(24));
    let h = root.height.min(12).max(root.height.min(8));
    Rect::new(
        root.x + root.width.saturating_sub(w) / 2,
        root.y + root.height.saturating_sub(h) / 2,
        w,
        h,
    )
}

fn bright() -> Style {
    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

fn button_style(focused: bool, selected: bool) -> Style {
    let mut style = if selected {
        Style::new().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::new().fg(Color::Gray)
    };
    if focused {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

fn fit_ascii(text: &str, max_width: u16) -> String {
    let max_width = usize::from(max_width);
    if max_width == 0 || text.chars().count() <= max_width {
        return text.to_string();
    }
    if max_width <= 1 {
        return "~".to_string();
    }
    let mut out = text
        .chars()
        .take(max_width.saturating_sub(1))
        .collect::<String>();
    out.push('~');
    out
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/loop_dialog__tests.rs"]
mod tests;
