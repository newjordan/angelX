//! The `/loop` launch's state: the three caps (laps, clock, fuel), the typed
//! custom length, focus, and the settings Start hands the loop. The card
//! that shows it, in the window or a terminal, is `ui::loop_card`.

use ratatui::layout::Rect;

/// Keep the established keyboard cycle: clock, laps, fuel, then the actions.
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

/// One of the card's three pickers as it is drawn: which focus it is, its
/// choices as short marks (`∞` for endless), the chosen one in words, and
/// how a click names a choice.
pub(crate) struct Picker {
    pub(crate) focus: LoopDialogFocus,
    pub(crate) choices: Vec<String>,
    pub(crate) value: String,
    pub(crate) selected: usize,
    /// The typed length is open on this picker.
    pub(crate) entry: bool,
    pub(crate) action: fn(usize) -> LoopDialogAction,
}

impl Picker {
    /// The choice one step back or on, as a click would name it: the same
    /// ring the arrow keys walk.
    pub(crate) fn step(&self, forward: bool) -> LoopDialogAction {
        (self.action)(cycle_index(self.selected, self.choices.len(), forward))
    }
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

/// The always-present laps choice that opens the typed length entry.
const CUSTOM_SLOT_MARK: &str = "custom";

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
            // A typed-but-uncommitted length is what the row shows, so it is
            // what a start launches with; an empty or zero draft is not a
            // length and the standing choice underneath applies.
            max_iters: self
                .custom_entry
                .as_deref()
                .and_then(|typed| typed.trim().parse::<usize>().ok())
                .filter(|&n| n > 0)
                .or_else(|| ITERATIONS.get(self.iterations_idx).map(|c| c.iters))
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

    pub(crate) fn focus(&self) -> LoopDialogFocus {
        self.focus
    }

    /// Whether this launch is the five-day podrace profile.
    pub(crate) fn is_podrace(&self) -> bool {
        self.podrace
    }

    /// The card's three pickers in its order: laps, clock, fuel. The marks
    /// are the presets' own labels, endless as `∞`; a custom cap shows as
    /// its value, and the laps picker always ends in the custom slot.
    pub(crate) fn pickers(&self) -> [Picker; 3] {
        let endless = |label: &str| {
            if label == "endless" {
                "∞".to_string()
            } else {
                label.to_string()
            }
        };
        let words = |mark: &str| {
            if mark == "∞" {
                "endless".to_string()
            } else {
                mark.to_string()
            }
        };
        let entry = self.custom_entry_text();
        let custom_lap = match entry {
            Some(typed) => format!("{typed}▏"),
            None => self
                .custom_iterations
                .map_or_else(|| CUSTOM_SLOT_MARK.to_string(), |n| n.to_string()),
        };
        let laps: Vec<String> = ITERATIONS
            .iter()
            .map(|c| endless(c.label))
            .chain([custom_lap])
            .collect();
        let clock: Vec<String> = DURATIONS
            .iter()
            .map(|c| endless(c.label))
            .chain(self.custom_duration.map(span_mark))
            .collect();
        let fuel: Vec<String> = BUDGETS
            .iter()
            .map(|c| endless(c.label).to_uppercase())
            .chain(self.custom_budget.map(token_mark))
            .collect();
        let pick =
            |choices: &[String], at: usize| choices.get(at).map(|c| words(c)).unwrap_or_default();
        [
            Picker {
                focus: LoopDialogFocus::Iterations,
                value: if entry.is_some() {
                    laps[ITERATIONS.len()].clone()
                } else {
                    pick(&laps, self.iterations_idx)
                },
                // While a length is typed the custom slot is the one in hand.
                selected: if entry.is_some() {
                    ITERATIONS.len()
                } else {
                    self.iterations_idx.min(laps.len() - 1)
                },
                entry: entry.is_some(),
                choices: laps,
                action: LoopDialogAction::SelectIterations,
            },
            Picker {
                focus: LoopDialogFocus::Duration,
                value: pick(&clock, self.duration_idx),
                selected: self.duration_idx.min(clock.len() - 1),
                entry: false,
                choices: clock,
                action: LoopDialogAction::SelectDuration,
            },
            Picker {
                focus: LoopDialogFocus::Budget,
                value: pick(&fuel, self.budget_idx),
                selected: self.budget_idx.min(fuel.len() - 1),
                entry: false,
                choices: fuel,
                action: LoopDialogAction::SelectBudget,
            },
        ]
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
        // The custom slot lives one past the presets, and selecting it always
        // opens the entry — seeded with the committed length when there is
        // one — so a set length can be revised instead of being one-shot.
        // Enter commits; Esc hands the row back to the choice it left.
        if index >= ITERATIONS.len() {
            self.begin_custom_length();
            return;
        }
        self.iterations_idx = index;
        if self.settings().max_iters == 0 {
            self.duration_idx = DURATIONS.len() - 1;
            self.budget_idx = BUDGETS.len() - 1;
        }
    }

    /// The index of the always-present custom-length slot in the iters row.
    /// Test-only: production compares against `ITERATIONS.len()` inline.
    #[cfg(test)]
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
        if let Some(committed) = self.custom_iterations
            && self.custom_entry.as_deref().is_some_and(str::is_empty)
        {
            self.custom_entry = Some(committed.to_string());
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

/// A custom deadline as a short mark: whole days, hours or minutes when it
/// is one, else seconds.
fn span_mark(secs: u64) -> String {
    match secs {
        0 => "∞".to_string(),
        s if s % 86_400 == 0 => format!("{}d", s / 86_400),
        s if s % 3_600 == 0 => format!("{}h", s / 3_600),
        s if s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

/// A custom token cap as a short mark: whole millions or thousands when it
/// is one, else every digit.
fn token_mark(tokens: usize) -> String {
    match tokens {
        0 => "∞".to_string(),
        t if t % 1_000_000 == 0 => format!("{}M", t / 1_000_000),
        t if t % 1_000 == 0 => format!("{}K", t / 1_000),
        t => t.to_string(),
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/loop_dialog__tests.rs"]
mod tests;
