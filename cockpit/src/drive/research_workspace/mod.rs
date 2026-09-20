//! Local, bounded navigation over observed work. This module never drives an
//! agent, evaluates a candidate, or contributes text to model context.

use crate::agent::club::{ChatMsg, ChatRole};
use crate::agent::harness::{ExecutionOutcome, ToolEventId, ToolOutcome, VerificationOutcome};
use crate::stage::world_viz::Building;
use std::collections::VecDeque;
use std::sync::Arc;

mod journal;
mod measurement;
#[cfg(test)]
#[path = "../../../../tests/cockpit/app/research_workspace__tests.rs"]
mod tests;
pub(crate) mod view;

const RECORD_CAP: usize = 512;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Place {
    #[default]
    Keep,
    Council,
    Smithy,
    Observatory,
    Library,
}

impl Place {
    pub(crate) const ALL: [Self; 5] = [
        Self::Keep,
        Self::Council,
        Self::Smithy,
        Self::Observatory,
        Self::Library,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Keep => "Keep",
            Self::Council => "Round Table",
            Self::Smithy => "Smithy",
            Self::Observatory => "Observatory",
            Self::Library => "Library",
        }
    }

    pub(crate) fn purpose(self) -> &'static str {
        match self {
            Self::Keep => "Campaign overview",
            Self::Council => "Agents & handoffs",
            Self::Smithy => "Changes & verification",
            Self::Observatory => "Research & measurements",
            Self::Library => "Findings & knowledge",
        }
    }

    pub(crate) fn building(self) -> Building {
        match self {
            Self::Keep => Building::Keep,
            Self::Council => Building::RoundTable,
            Self::Smithy => Building::Smithy,
            Self::Observatory => Building::Observatory,
            Self::Library => Building::Scriptorium,
        }
    }

    pub(crate) fn from_building(building: Building) -> Self {
        match building {
            Building::Keep | Building::Gatehouse | Building::Rookery => Self::Keep,
            Building::RoundTable => Self::Council,
            Building::Smithy => Self::Smithy,
            Building::Observatory => Self::Observatory,
            Building::Scriptorium | Building::Chapel => Self::Library,
        }
    }

    pub(crate) fn step(self, delta: isize) -> Self {
        let index = Self::ALL
            .iter()
            .position(|place| *place == self)
            .unwrap_or(0);
        Self::ALL[(index as isize + delta).rem_euclid(5) as usize]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Lens {
    #[default]
    Story,
    Ledger,
    Flow,
}

impl Lens {
    pub(crate) const ALL: [Self; 3] = [Self::Story, Self::Ledger, Self::Flow];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Story => "Story",
            Self::Ledger => "Ledger",
            Self::Flow => "Flow",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Pending,
    Running,
    Verified,
    Succeeded,
    Failed,
    Denied,
    Cancelled,
    Inconclusive,
    Recorded,
}

impl State {
    pub(crate) const ALL: [Self; 9] = [
        Self::Pending,
        Self::Running,
        Self::Verified,
        Self::Succeeded,
        Self::Failed,
        Self::Denied,
        Self::Cancelled,
        Self::Inconclusive,
        Self::Recorded,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Pending => "QUEUED",
            Self::Running => "RUNNING",
            Self::Verified => "VERIFIED",
            Self::Succeeded => "SUCCEEDED",
            Self::Failed => "FAILED",
            Self::Denied => "DENIED",
            Self::Cancelled => "CANCELLED",
            Self::Inconclusive => "INCONCLUSIVE",
            Self::Recorded => "RECORDED",
        }
    }

    fn from_outcome(outcome: ToolOutcome) -> Self {
        match outcome.execution {
            ExecutionOutcome::Succeeded => match outcome.verification {
                VerificationOutcome::Passed => Self::Verified,
                VerificationOutcome::Failed => Self::Failed,
                VerificationOutcome::Inconclusive => Self::Inconclusive,
                VerificationOutcome::NotApplicable => Self::Succeeded,
            },
            ExecutionOutcome::Failed | ExecutionOutcome::Panicked => Self::Failed,
            ExecutionOutcome::Denied => Self::Denied,
            ExecutionOutcome::Cancelled => Self::Cancelled,
            ExecutionOutcome::NotStarted => Self::Inconclusive,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) experiment_id: Option<String>,
    pub(crate) observed_unix_ms: Option<u64>,
    pub(crate) measurement: Option<measurement::Measurement>,
    pub(crate) id: String,
    pub(crate) place: Place,
    pub(crate) title: String,
    pub(crate) state: State,
    pub(crate) summary: String,
    pub(crate) detail: String,
    pub(crate) source: &'static str,
    pub(crate) parent: Option<String>,
    pub(crate) links: Vec<(String, String)>,
}

impl Entry {
    pub(crate) fn new(
        id: String,
        place: Place,
        title: &str,
        state: State,
        summary: &str,
        detail: &str,
        source: &'static str,
    ) -> Self {
        Self {
            experiment_id: None,
            observed_unix_ms: None,
            measurement: None,
            id,
            place,
            title: bounded(title, 160),
            state,
            summary: bounded(summary, 240),
            detail: bounded(detail, 3200),
            source,
            parent: None,
            links: Vec::new(),
        }
    }
}

/// Text is always terminal-safe, bounded, and separate from private reasoning.
pub(crate) fn bounded(text: &str, cap: usize) -> String {
    let mut chars = text
        .chars()
        .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t');
    let mut result: String = chars
        .by_ref()
        .take(cap)
        .map(|ch| if ch == '\t' { ' ' } else { ch })
        .collect();
    if chars.next().is_some() {
        result.push_str("… [truncated]");
    }
    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Place(Place),
    Lens(Lens),
    Select(usize),
    Inspect,
    Back,
    Follow,
    Expand,
    World,
    Filter(State),
    ClearFilter,
    FocusExperiment,
    ClearExperiment,
    Related(usize),
}

#[derive(Default)]
pub(crate) struct Workspace {
    pub(crate) journal: journal::Journal,
    pub(crate) place: Place,
    pub(crate) lens: Lens,
    pub(crate) expanded: bool,
    pub(crate) following: bool,
    pub(crate) inspecting: bool,
    pub(crate) detail_scroll: usize,
    pub(crate) offset: usize,
    pub(crate) viewport_height: usize,
    pub(crate) filter: Option<State>,
    pub(crate) experiment_filter: Option<String>,
    pub(crate) selected: Option<String>,
    pub(crate) visible: Vec<Arc<Entry>>,
    pub(crate) omitted: usize,
    pub(crate) diagnostics: usize,
    scope: String,
    records: VecDeque<Arc<Entry>>,
    saved_selections: [Option<String>; 5],
    saved_offsets: [usize; 5],
    pub(crate) scroll_anchor: Option<(String, isize)>,
    pub(crate) reveal_selection: bool,
    inspected: Option<Arc<Entry>>,
    dirty: bool,
    refreshed: Option<std::time::Instant>,
    pub(crate) context: String,
    pub(crate) illustrative: bool,
    pub(crate) active_places: [usize; 5],
    catalog: Vec<Arc<Entry>>,
    inspector_back: Vec<(Arc<Entry>, usize)>,
}

impl Workspace {
    /// The existing project-bound session is the persistence authority. Restore
    /// only retained tool exchanges, without inventing lost structured verdicts.
    pub(crate) fn ensure_session(&mut self, scope: &str, history: &[ChatMsg]) {
        if self.scope == scope {
            return;
        }
        let expanded = self.expanded;
        *self = Self {
            scope: scope.to_string(),
            following: true,
            expanded,
            dirty: true,
            ..Self::default()
        };
        let start = history.len().saturating_sub(RECORD_CAP * 2);
        let retained = &history[start..];
        let call_count: usize = retained
            .iter()
            .filter(|msg| msg.role == ChatRole::Assistant)
            .map(|msg| msg.tool_calls.len())
            .sum();
        self.omitted = call_count.saturating_sub(RECORD_CAP);
        let calls: Vec<_> = retained
            .iter()
            .filter(|msg| msg.role == ChatRole::Assistant)
            .flat_map(|msg| msg.tool_calls.iter())
            .rev()
            .take(RECORD_CAP)
            .collect();
        for call in calls.into_iter().rev() {
            // Bound restoration across the whole retained history, not per
            // message. Large parallel batches cannot turn resume into a scan
            // and serialization of every argument in the conversation.
            self.start(
                &ToolEventId(call.id.clone()),
                &call.name,
                "Restored from saved conversation; arguments remain in the transcript.",
            );
        }
        for entry in &mut self.records {
            Arc::make_mut(entry).source = "saved conversation";
        }
        for msg in retained {
            if msg.role == ChatRole::Tool
                && let Some(id) = msg.tool_call_id.as_deref()
            {
                let key = format!("tool:{id}");
                if let Some(entry) = self.records.iter_mut().find(|entry| entry.id == key) {
                    let entry = Arc::make_mut(entry);
                    entry.state = State::Recorded;
                    entry.summary = "Restored result · structured verdict unavailable".into();
                    entry.detail = bounded(&msg.content, 3200);
                }
            }
        }
        self.finish_turn();
    }

    pub(crate) fn start(&mut self, id: &ToolEventId, name: &str, args: &str) {
        self.dirty = true;
        let key = format!("tool:{}", id.0);
        if self.records.iter().any(|entry| entry.id == key) {
            self.diagnostics += 1;
            return;
        }
        if self.records.len() >= RECORD_CAP {
            if let Some(index) = self
                .records
                .iter()
                .position(|entry| entry.state != State::Running)
            {
                self.records.remove(index);
            } else {
                self.omitted += 1;
                return;
            }
            self.omitted += 1;
        }
        let place = Place::from_building(
            crate::stage::world_viz::classify_tool_activity(name, args).building,
        );
        self.records.push_back(Arc::new(Entry::new(
            key,
            place,
            name,
            State::Running,
            args,
            args,
            "live tool event",
        )));
    }

    pub(crate) fn result(
        &mut self,
        id: &ToolEventId,
        name: &str,
        summary: &str,
        outcome: ToolOutcome,
    ) {
        self.dirty = true;
        let key = format!("tool:{}", id.0);
        let Some(entry) = self.records.iter_mut().find(|entry| entry.id == key) else {
            self.diagnostics += 1;
            return;
        };
        if entry.state != State::Running || entry.title != bounded(name, 160) {
            self.diagnostics += 1;
            return;
        }
        let entry = Arc::make_mut(entry);
        entry.state = State::from_outcome(outcome);
        entry.detail = format!(
            "Operation\n{}\n\nArguments\n{}\n\nResult\n{}",
            entry.title,
            entry.detail,
            bounded(summary, 1800)
        );
        entry.summary = bounded(summary, 240);
    }

    pub(crate) fn finish_turn(&mut self) {
        for entry in &mut self.records {
            if entry.state == State::Running {
                self.dirty = true;
                let entry = Arc::make_mut(entry);
                entry.state = State::Inconclusive;
                entry.summary = "Turn ended without a correlated terminal result".into();
            }
        }
    }

    pub(crate) fn project(&mut self, extra: Vec<Entry>) {
        self.active_places = [0; 5];
        let entries: Vec<_> = extra
            .into_iter()
            .map(Arc::new)
            .chain(self.records.iter().cloned())
            .collect();
        for entry in &entries {
            if entry.state == State::Running && self.in_experiment(entry) {
                self.active_places[entry.place as usize] += 1;
            }
        }
        self.catalog = entries.clone();
        self.visible = entries
            .into_iter()
            .filter(|entry| self.place == Place::Keep || entry.place == self.place)
            .filter(|entry| self.filter.is_none_or(|state| entry.state == state))
            .filter(|entry| self.in_experiment(entry))
            .collect();
        if self.following && !self.inspecting {
            self.selected = self.visible.last().map(|entry| entry.id.clone());
            self.reveal_selection = self.lens != Lens::Flow;
        } else if !self.inspecting
            && !self
                .visible
                .iter()
                .any(|entry| Some(&entry.id) == self.selected.as_ref())
        {
            // An inspector keeps its detached item visible through a live update
            // in the caller; list selection clamps only when its source expired.
            self.selected = self.visible.first().map(|entry| entry.id.clone());
            self.reveal_selection = true;
        }
        if self.inspecting
            && let Some(entry) = self
                .catalog
                .iter()
                .find(|entry| Some(&entry.id) == self.selected.as_ref())
        {
            self.inspected = Some(Arc::clone(entry));
        }
        self.dirty = false;
        self.refreshed = Some(std::time::Instant::now());
    }

    pub(crate) fn needs_refresh(&self) -> bool {
        self.dirty
            || self
                .refreshed
                .is_none_or(|when| when.elapsed() >= std::time::Duration::from_millis(250))
    }

    fn in_experiment(&self, entry: &Entry) -> bool {
        self.experiment_filter
            .as_ref()
            .is_none_or(|id| entry.experiment_id.as_ref() == Some(id))
    }

    pub(crate) fn selected_index(&self) -> Option<usize> {
        self.visible
            .iter()
            .position(|entry| Some(&entry.id) == self.selected.as_ref())
    }

    pub(crate) fn selected_entry(&self) -> Option<&Entry> {
        if self.inspecting {
            return self.inspected.as_deref();
        }
        self.selected_index()
            .map(|index| self.visible[index].as_ref())
    }

    pub(crate) fn move_selection(&mut self, delta: isize) {
        self.following = false;
        if self.inspecting {
            self.detail_scroll = self.detail_scroll.saturating_add_signed(delta);
            return;
        }
        if self.visible.is_empty() {
            return;
        }
        let index = self
            .selected_index()
            .unwrap_or(0)
            .saturating_add_signed(delta)
            .min(self.visible.len() - 1);
        self.selected = Some(self.visible[index].id.clone());
        self.reveal_selection = true;
    }

    pub(crate) fn action(&mut self, action: Action) {
        match action {
            Action::Place(place) => {
                self.dirty = true;
                self.saved_selections[self.place as usize] = self.selected.clone();
                self.saved_offsets[self.place as usize] = self.offset;
                self.place = place;
                self.selected = self.saved_selections[place as usize].clone();
                self.filter = None;
                self.inspecting = false;
                self.following = false;
                self.offset = self.saved_offsets[place as usize];
                self.scroll_anchor = None;
                self.reveal_selection = true;
            }
            Action::Lens(lens) => {
                self.lens = lens;
                self.inspecting = false;
                self.offset = 0;
                self.scroll_anchor = None;
                self.reveal_selection = lens != Lens::Flow;
            }
            Action::Select(index) => {
                if let Some(entry) = self.visible.get(index) {
                    self.selected = Some(entry.id.clone());
                    self.following = false;
                    self.reveal_selection = true;
                }
            }
            Action::Inspect => {
                self.inspector_back.clear();
                self.inspected = self
                    .selected_index()
                    .map(|index| Arc::clone(&self.visible[index]));
                self.inspecting = self.inspected.is_some();
                self.following = false;
                self.detail_scroll = 0;
            }
            Action::Back => {
                if let Some((entry, scroll)) = self.inspector_back.pop() {
                    self.selected = Some(entry.id.clone());
                    self.inspected = Some(entry);
                    self.detail_scroll = scroll;
                } else {
                    self.inspecting = false;
                    self.inspected = None;
                    self.detail_scroll = 0;
                    self.reveal_selection = true;
                    self.dirty = true;
                }
            }
            Action::Follow => {
                self.following = !self.following;
                self.inspecting = false;
                self.reveal_selection = true;
                self.dirty = true;
            }
            Action::Expand => self.expanded = !self.expanded,
            Action::Filter(state) => {
                self.filter = Some(state);
                self.lens = Lens::Ledger;
                self.following = false;
                self.inspecting = false;
                self.offset = 0;
                self.reveal_selection = true;
                self.dirty = true;
            }
            Action::ClearFilter => {
                self.filter = None;
                self.offset = 0;
                self.reveal_selection = true;
                self.dirty = true;
            }
            Action::FocusExperiment => {
                if let Some(id) = self.selected_entry().and_then(|e| e.experiment_id.clone()) {
                    let selected = self.selected.clone();
                    self.action(Action::Place(Place::Keep));
                    self.selected = selected;
                    self.experiment_filter = Some(id);
                    self.inspector_back.clear();
                    self.inspected = None;
                    self.offset = 0;
                }
            }
            Action::ClearExperiment => {
                self.experiment_filter = None;
                self.offset = 0;
                self.scroll_anchor = None;
                self.reveal_selection = true;
                self.dirty = true;
            }
            Action::Related(index) => {
                let target = self
                    .inspected
                    .as_ref()
                    .and_then(|entry| entry.links.get(index))
                    .map(|(id, _)| id);
                if let Some(next) = self
                    .catalog
                    .iter()
                    .find(|entry| Some(&entry.id) == target)
                    .cloned()
                {
                    if let Some(previous) = self.inspected.take() {
                        if self.inspector_back.len() == 16 {
                            self.inspector_back.remove(0);
                        }
                        self.inspector_back.push((previous, self.detail_scroll));
                    }
                    self.selected = Some(next.id.clone());
                    self.inspected = Some(next);
                    self.detail_scroll = 0;
                }
            }
            Action::World => {}
        }
    }
}
