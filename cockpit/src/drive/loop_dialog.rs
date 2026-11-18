//! The `/loop` launch's state: the three caps (laps, clock, fuel), the typed
//! custom length, focus, and the settings Start hands the loop. The card
//! that shows it, in the window or a terminal, is `ui::loop_card`.

use ratatui::layout::Rect;

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
    laps: Laps,
    budget_idx: usize,
    custom_duration: Option<u64>,
    custom_iterations: Option<usize>,
    custom_budget: Option<usize>,
    podrace: bool,
    focus: LoopDialogFocus,
    entry: Option<LengthEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Laps {
    Preset(usize),
    Custom,
}

const ENDLESS_LAPS: Laps = Laps::Preset(ITERATIONS.len() - 1);

impl Laps {
    fn slot(self) -> usize {
        match self {
            Laps::Preset(i) => i,
            Laps::Custom => ITERATIONS.len(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LengthEntry {
    typed: String,
    return_to: Laps,
}

pub(crate) struct Picker {
    pub(crate) focus: LoopDialogFocus,
    pub(crate) choices: Vec<String>,
    pub(crate) value: String,
    pub(crate) selected: usize,
    pub(crate) entry: bool,
    pub(crate) action: fn(usize) -> LoopDialogAction,
}

impl Picker {
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
            laps: ENDLESS_LAPS,
            budget_idx: BUDGETS.len() - 1,
            custom_duration: None,
            custom_iterations: None,
            custom_budget: None,
            podrace: false,
            focus: LoopDialogFocus::Duration,
            entry: None,
        }
    }

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
            laps: ENDLESS_LAPS,
            budget_idx: BUDGETS.len() - 1,
            custom_duration: None,
            custom_iterations: None,
            custom_budget: None,
            podrace: true,
            focus: LoopDialogFocus::Duration,
            entry: None,
        }
    }

    pub(crate) fn handoff_rl(task: impl Into<String>, interval_secs: u64) -> Self {
        let mut dialog = Self::new(task, interval_secs, false);
        dialog.purpose = LoopLaunchPurpose::HandoffRl;
        dialog.duration_idx = DURATIONS.len() - 1;
        dialog.budget_idx = BUDGETS.len() - 1;
        dialog.laps = ENDLESS_LAPS;
        dialog
    }

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
            max_iters: self
                .entry
                .as_ref()
                .and_then(|entry| entry.typed.trim().parse::<usize>().ok())
                .filter(|&n| n > 0)
                .or_else(|| match self.laps {
                    Laps::Preset(i) => ITERATIONS.get(i).map(|c| c.iters),
                    Laps::Custom => self.custom_iterations,
                })
                .expect("selected iteration cap exists"),
            token_budget: BUDGETS
                .get(self.budget_idx)
                .map(|c| c.tokens)
                .or(self.custom_budget)
                .expect("selected token cap exists"),
            podrace: self.podrace,
        }
    }

    pub(crate) fn restore_settings(&mut self, settings: LoopLaunchSettings) {
        self.duration_idx = DURATIONS
            .iter()
            .position(|c| c.secs == settings.deadline_secs)
            .unwrap_or(DURATIONS.len());
        self.laps = ITERATIONS
            .iter()
            .position(|c| c.iters == settings.max_iters)
            .map_or(Laps::Custom, Laps::Preset);
        self.budget_idx = BUDGETS
            .iter()
            .position(|c| c.tokens == settings.token_budget)
            .unwrap_or(BUDGETS.len());
        self.custom_duration =
            (self.duration_idx == DURATIONS.len()).then_some(settings.deadline_secs);
        self.custom_iterations = (self.laps == Laps::Custom).then_some(settings.max_iters);
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

    pub(crate) fn is_podrace(&self) -> bool {
        self.podrace
    }

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
                    pick(&laps, self.laps.slot())
                },
                selected: if entry.is_some() {
                    ITERATIONS.len()
                } else {
                    self.laps.slot()
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
                if self.entry.is_some() {
                    return;
                }
                self.select_iterations(cycle_index(
                    self.laps.slot(),
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
        if index >= ITERATIONS.len() {
            self.begin_custom_length();
            return;
        }
        self.laps = Laps::Preset(index);
        if self.settings().max_iters == 0 {
            self.duration_idx = DURATIONS.len() - 1;
            self.budget_idx = BUDGETS.len() - 1;
        }
    }

    #[cfg(test)]
    pub(crate) fn custom_length_slot() -> usize {
        ITERATIONS.len()
    }

    pub(crate) fn custom_entry_active(&self) -> bool {
        self.entry.is_some()
    }

    pub(crate) fn custom_entry_text(&self) -> Option<&str> {
        self.entry.as_ref().map(|entry| entry.typed.as_str())
    }

    pub(crate) fn begin_custom_length(&mut self) {
        if self.entry.is_none() {
            self.entry = Some(LengthEntry {
                typed: String::new(),
                return_to: self.laps,
            });
        }
        if let Some(committed) = self.custom_iterations
            && let Some(entry) = self.entry.as_mut()
            && entry.typed.is_empty()
        {
            entry.typed = committed.to_string();
        }
        self.focus = LoopDialogFocus::Iterations;
    }

    pub(crate) fn custom_length_digit(&mut self, ch: char) -> bool {
        let Some(LengthEntry { typed: buffer, .. }) = self.entry.as_mut() else {
            return false;
        };
        if !ch.is_ascii_digit() {
            return false;
        }
        if buffer.len() >= 7 {
            return true;
        }
        if buffer == "0" {
            buffer.clear();
        }
        buffer.push(ch);
        true
    }

    pub(crate) fn custom_length_backspace(&mut self) -> bool {
        let Some(LengthEntry { typed: buffer, .. }) = self.entry.as_mut() else {
            return false;
        };
        buffer.pop();
        true
    }

    pub(crate) fn commit_custom_length(&mut self) -> bool {
        let Some(entry) = self.entry.take() else {
            return false;
        };
        match entry.typed.trim().parse::<usize>() {
            Ok(iters) if iters > 0 => {
                self.custom_iterations = Some(iters);
                self.laps = Laps::Custom;
                true
            }
            _ => {
                self.laps = entry.return_to;
                false
            }
        }
    }

    pub(crate) fn cancel_custom_length(&mut self) -> bool {
        let Some(entry) = self.entry.take() else {
            return false;
        };
        self.laps = entry.return_to;
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

fn span_mark(secs: u64) -> String {
    match secs {
        0 => "∞".to_string(),
        s if s % 86_400 == 0 => format!("{}d", s / 86_400),
        s if s % 3_600 == 0 => format!("{}h", s / 3_600),
        s if s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

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
