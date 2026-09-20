//! Research navigation is local and remains available during model work.

use super::*;
use crate::drive::research_workspace::{Action, Entry, Lens, Place, State, bounded};

impl App {
    pub(crate) fn ensure_research_session(&mut self) {
        let scope = format!(
            "{}:{}",
            self.tools.current_workspace().display(),
            self.session.id
        );
        self.research.ensure_session(&scope, &self.history);
    }

    pub(crate) fn open_research(&mut self, arg: Option<&str>) -> String {
        let place = match arg.unwrap_or("").trim().to_ascii_lowercase().as_str() {
            "" | "keep" => Place::Keep,
            "table" | "council" => Place::Council,
            "smithy" | "forge" => Place::Smithy,
            "observatory" | "scope" => Place::Observatory,
            "library" | "books" => Place::Library,
            _ => return "usage: /research [keep|table|smithy|observatory|library]".into(),
        };
        self.ensure_research_session();
        self.research.action(Action::Place(place));
        self.viewer.clear();
        self.scryglass.back_overlay();
        self.scryglass
            .navigate(crate::ui::scryglass::StageRoute::Research);
        self.focus_module("artifacts");
        format!(
            "Research · {} · ↑↓ select · Enter inspect · 1/2/3 views · z expand · w world",
            place.label()
        )
    }

    pub(crate) fn research_action(&mut self, action: Action) {
        if action == Action::World {
            self.scryglass
                .navigate(crate::ui::scryglass::StageRoute::Realm);
        } else {
            self.research.action(action);
            self.scryglass
                .navigate(crate::ui::scryglass::StageRoute::Research);
        }
        self.focus_module("artifacts");
    }

    pub(crate) fn research_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        if !self.input.is_empty() || modifiers != KeyModifiers::NONE {
            return false;
        }
        let page = self.research.viewport_height.max(1) as isize;
        let action = match code {
            KeyCode::Char(ch @ '1'..='9') if self.research.inspecting => {
                Action::Related((ch as u8 - b'1') as usize)
            }
            KeyCode::Esc | KeyCode::Left if self.research.inspecting => Action::Back,
            KeyCode::Esc if self.research.filter.is_some() => Action::ClearFilter,
            KeyCode::Esc if self.research.experiment_filter.is_some() => Action::ClearExperiment,
            KeyCode::Left => Action::Place(self.research.place.step(-1)),
            KeyCode::Right => Action::Place(self.research.place.step(1)),
            KeyCode::Up | KeyCode::Char('k') => {
                self.research.move_selection(-1);
                return true;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.research.move_selection(1);
                return true;
            }
            KeyCode::PageUp => {
                self.research.move_selection(-page);
                return true;
            }
            KeyCode::PageDown => {
                self.research.move_selection(page);
                return true;
            }
            KeyCode::Home => {
                self.research.move_selection(-1_000_000);
                return true;
            }
            KeyCode::End => {
                self.research.move_selection(1_000_000);
                return true;
            }
            KeyCode::Enter => Action::Inspect,
            KeyCode::Char('1') => Action::Lens(Lens::Story),
            KeyCode::Char('2') => Action::Lens(Lens::Ledger),
            KeyCode::Char('3') => Action::Lens(Lens::Flow),
            KeyCode::Char('f') => Action::Follow,
            KeyCode::Char('e')
                if self.research.experiment_filter.is_some() && !self.research.inspecting =>
            {
                Action::ClearExperiment
            }
            KeyCode::Char('e')
                if self
                    .research
                    .selected_entry()
                    .is_some_and(|e| e.experiment_id.is_some()) =>
            {
                Action::FocusExperiment
            }
            KeyCode::Char('v') if self.research.selected_entry().is_some() => {
                Action::Filter(self.research.selected_entry().unwrap().state)
            }
            KeyCode::Char('z') => Action::Expand,
            KeyCode::Char('w') => Action::World,
            _ => return false,
        };
        self.research_action(action);
        true
    }

    /// Reuse project-bound loop evidence. Checkpoints are observations, never
    /// inferred candidate scores or new reward labels.
    pub(crate) fn refresh_research(&mut self) -> String {
        self.ensure_research_session();
        if !self.research.needs_refresh() {
            return self.research.context.clone();
        }
        let mut extra = Vec::new();
        extra.extend(
            self.research
                .journal
                .refresh(self.tools.current_workspace()),
        );
        let mut omitted_snapshot_nodes = 0;
        if let Some(thinking) = self.thinking.as_ref()
            && let Some(stage) = crate::ui::viz::agentviz::current()
        {
            for (index, agent) in stage.agents.iter().take(64).enumerate() {
                let state = match stage.seat_states.get(index).copied().unwrap_or_default() {
                    crate::ui::viz::agentviz::SeatState::Running => State::Running,
                    crate::ui::viz::agentviz::SeatState::Returned => State::Succeeded,
                    crate::ui::viz::agentviz::SeatState::Failed => State::Failed,
                    crate::ui::viz::agentviz::SeatState::Cut => State::Cancelled,
                };
                extra.push(Entry::new(format!("formation:{:?}:{}:{index}", thinking.started, stage.stage_id), Place::Council,
                        agent, state, &stage.name, &format!("Latest published formation stage: {}\nSeat {} / {}\nAgent: {}\nState: {}\n\nProcess-local stage telemetry reports whether a seat returned. It is not a complete parallel-agent trace and does not assign a task reward.", stage.name, index + 1, stage.agents.len(), agent, state.label()), "live formation stage"));
            }
        }
        if let Some(graph) = crate::agent::harness::current_graph_snapshot() {
            let workspace_key = crate::knowledge::cut::sha256_hex(
                crate::platform::workspace_store::workspace_key(self.tools.current_workspace())
                    .as_bytes(),
            );
            if graph.workspace_key_sha256 == workspace_key {
                use crate::agent::harness::GraphNodePhase;
                omitted_snapshot_nodes = graph.nodes.len().saturating_sub(128);
                for node in graph.nodes.iter().take(128) {
                    let state = match node.phase {
                        GraphNodePhase::Pending => State::Pending,
                        GraphNodePhase::Running
                            if self.agent_graph.running() || self.thinking.is_some() =>
                        {
                            State::Running
                        }
                        GraphNodePhase::Running => State::Inconclusive,
                        GraphNodePhase::Done => State::Succeeded,
                        GraphNodePhase::Failed => State::Failed,
                        GraphNodePhase::Skipped => State::Cancelled,
                    };
                    let summary = format!(
                        "{} · {:.2}s · {} retries",
                        node.club,
                        node.elapsed_ms as f64 / 1000.0,
                        node.retries
                    );
                    let detail = format!(
                        "Agent graph: {}\nTask: {}\n\nNode: {}\nModel: {}\nRole: {}\nTool grant: {}\nDependencies: {}\nPool: {}\nLease: {:?}\nElapsed: {} ms\nRetries: {}\nOutput size: {} characters\nControl gate: {}",
                        graph.graph,
                        bounded(&graph.task, 1200),
                        node.id,
                        node.club,
                        node.persona,
                        node.grant,
                        node.deps.join(", "),
                        node.pool.as_deref().unwrap_or("none"),
                        node.lease_id,
                        node.elapsed_ms,
                        node.retries,
                        node.output_chars,
                        node.gate.as_deref().unwrap_or("none")
                    );
                    let mut entry = Entry::new(
                        format!("graph:{}:{}", graph.episode_id, node.id),
                        Place::Council,
                        &node.id,
                        state,
                        &summary,
                        &detail,
                        "agent graph episode",
                    );
                    entry.parent = Some(format!("{} · episode {}", graph.graph, graph.episode_id));
                    entry.links = node
                        .deps
                        .iter()
                        .take(128)
                        .map(|id| {
                            (
                                format!("graph:{}:{id}", graph.episode_id),
                                format!("Dependency · {id}"),
                            )
                        })
                        .collect();
                    entry.links.extend(
                        graph
                            .nodes
                            .iter()
                            .take(128)
                            .filter(|other| other.deps.contains(&node.id))
                            .map(|other| {
                                (
                                    format!("graph:{}:{}", graph.episode_id, other.id),
                                    format!("Consumer · {}", other.id),
                                )
                            }),
                    );
                    extra.push(entry);
                }
            }
        }
        let loop_state = &self.loop_ctl;
        let loop_in_scope = loop_state
            .workspace
            .as_deref()
            .is_some_and(|workspace| workspace == self.tools.current_workspace());
        if loop_in_scope {
            for (index, finding) in loop_state
                .findings
                .iter()
                .enumerate()
                .skip(loop_state.findings.len().saturating_sub(64))
            {
                let mut entry = Entry::new(
                    format!("loop:{}:finding:{index}", loop_state.id),
                    Place::Library,
                    &format!("Finding {}", index + 1),
                    State::Recorded,
                    finding,
                    finding,
                    "loop evidence ledger",
                );
                entry.parent = Some(format!("loop {}", loop_state.id));
                extra.push(entry);
            }
            for (index, hypothesis) in loop_state
                .hypotheses
                .iter()
                .enumerate()
                .skip(loop_state.hypotheses.len().saturating_sub(32))
            {
                extra.push(Entry::new(
                    format!("loop:{}:hypothesis:{index}", loop_state.id),
                    Place::Library,
                    &format!("Hypothesis {}", index + 1),
                    State::Recorded,
                    "Unverified lead",
                    hypothesis,
                    "loop hypothesis ledger",
                ));
            }
            for iteration in loop_state
                .log
                .iter()
                .skip(loop_state.log.len().saturating_sub(64))
            {
                let summary = format!(
                    "{} findings · {} tool calls · {} errors",
                    iteration.new_findings, iteration.tool_calls, iteration.tool_errors
                );
                let detail = format!(
                    "Direction\n{}\n\nEvidence\n{summary}\nNovel outcome actions: {}\nRepeated costly actions: {}\nConsecutive stalls: {}",
                    bounded(&iteration.direction, 1600),
                    iteration.novel_outcome_actions,
                    iteration.duplicate_costly_actions,
                    iteration.stale_count
                );
                extra.push(Entry::new(
                    format!("loop:{}:iteration:{}", loop_state.id, iteration.iteration),
                    Place::Keep,
                    &format!("Iteration {}", iteration.iteration),
                    State::Recorded,
                    &summary,
                    &detail,
                    "loop iteration ledger",
                ));
            }
        }
        if self.tools.rl().mode == crate::drive::rl_ctl::RlMode::Campaign {
            let progress = self.tools.rl().progress_snapshot();
            for point in progress
                .points
                .iter()
                .skip(progress.points.len().saturating_sub(128))
            {
                let summary = format!("reward {:.2} · {}ms", point.reward, point.latency_ms);
                let detail = format!(
                    "Measured objective attempt\nAttempt {} / {}\nOperator verifier reward: {:.4}\nAttempt wall time: {} ms\n\n{} observations received; this view retains the latest 128 stored attempts. The reward is the operator's own verifier verdict on that attempt's work, not model text.",
                    point.step,
                    progress.planned_attempts,
                    point.reward,
                    point.latency_ms,
                    progress.observed_attempts()
                );
                extra.push(Entry::new(
                    format!("rl:{:?}:attempt:{}", self.tools.rl().started, point.step),
                    Place::Observatory,
                    &format!("Attempt {}", point.step),
                    State::Recorded,
                    &summary,
                    &detail,
                    "measured RL campaign",
                ));
            }
        }
        if self.tools.rl().mode == crate::drive::rl_ctl::RlMode::Campaign
            && let Some(snapshot) = crate::drive::reinforce::telemetry::current()
        {
            for event in &snapshot.events {
                extra.push(Entry::new(
                    format!("rl:{:?}:event:{}", self.tools.rl().started, event.seq),
                    Place::Observatory,
                    &event.label,
                    State::Recorded,
                    &format!(
                        "round {} · policy {}",
                        snapshot.round, snapshot.policy_version
                    ),
                    &event.label,
                    "RL campaign telemetry",
                ));
            }
        }
        self.research.project(extra);
        let active = self
            .research
            .visible
            .iter()
            .filter(|entry| entry.state == State::Running)
            .count();
        let mut context = format!("{} records · {active} running", self.research.visible.len());
        if loop_in_scope && !loop_state.id.is_empty() {
            context.push_str(&format!(
                " · loop {} · {:?}",
                loop_state.iteration, loop_state.status
            ));
        }
        if self.research.omitted > 0 {
            context.push_str(&format!(
                " · {} earlier records omitted",
                self.research.omitted
            ));
        }
        if self.research.illustrative {
            context = format!("ILLUSTRATIVE · {context}");
        }
        if omitted_snapshot_nodes > 0 {
            context.push_str(&format!(
                " · {omitted_snapshot_nodes} graph nodes outside view limit"
            ));
        }
        if self.research.diagnostics > 0 {
            context.push_str(&format!(
                " · {} unmatched/duplicate events",
                self.research.diagnostics
            ));
        }
        if let Some(issue) = self.research.journal.issue {
            context.push_str(&format!(" · {issue}"));
        }
        if self.research.journal.omitted > 0 {
            context.push_str(&format!(
                " · {} earlier experiment records omitted",
                self.research.journal.omitted
            ));
        }
        self.research.context = context.clone();
        context
    }
}
