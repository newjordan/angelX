//! Streaming advance loop and input/key/mouse/paste handling.
use super::*;
use crate::{Overlay, WorldButton};
use crate::agent::harness::cartridges::fleet::SubmissionPhase;
use crate::agent::harness::cartridges::records::SubmissionStatus;
use crate::drive::loop_ctl::{EscalationTier, LoopState, LoopStatus};
use angel_plug::stage::StagePlaces as _;
use angel_plug::stage::StageWorld as _;
use angel_plug::feed::{
    Checkpoint, Competition, Feed, FeedRules, FeedWorld, Health, LoopBudget, LoopFacts, LoopPhase,
    LoopTier, Route, Serving,
};

pub(crate) const STREAM_EVENTS_PER_FRAME: usize = 64;
pub(crate) const STREAM_BYTES_PER_FRAME: usize = 8 * 1024;
pub(crate) const MAX_COMPOSER_PASTE_BYTES: usize = 256 * 1024;
pub(crate) const MAX_COMPOSER_KILL_BYTES: usize = 256 * 1024;
pub(crate) const MAX_COMPOSER_HISTORY_BYTES: usize = 256 * 1024;
pub(crate) const MAX_COMPOSER_HISTORY_ENTRIES: usize = 100;
pub(crate) const MAX_COMPOSER_HISTORY_SEARCH_BYTES: usize = 2 * 1024 * 1024;
const WORLD_TICK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(25);
const MAX_WORLD_CATCH_UP_TICKS: u32 = 8;
const RETAINED_PARTIAL_NOTE: &str =
    " · partial reply retained above (display only; retry starts from the last committed turn)";

fn reasoning_reveal_boundary(text: &str, shown: usize, step: usize) -> usize {
    let mut target = shown.saturating_add(step).min(text.len());
    while target < text.len() && !text.is_char_boundary(target) {
        target += 1;
    }
    let mut cursor = unicode_segmentation::GraphemeCursor::new(target, text.len(), true);
    if matches!(cursor.is_boundary(text, 0), Ok(true)) {
        target
    } else {
        cursor
            .next_boundary(text, 0)
            .ok()
            .flatten()
            .unwrap_or(text.len())
    }
}

fn queue_proc_completion_for_loop(
    state: &mut crate::drive::loop_ctl::LoopState,
    workspace: &std::path::Path,
    message: String,
) -> bool {
    if state.status != crate::drive::loop_ctl::LoopStatus::Running
        || state.workspace.as_deref() != Some(workspace)
        || state.pending_proc_completions.len() >= 8
    {
        return false;
    }
    state.pending_proc_completions.push(message);
    if !state.awaiting_turn {
        state.wake_at = Some(std::time::Instant::now());
    }
    true
}

#[derive(Clone, Copy, Default)]
enum TerminalEscapeState {
    #[default]
    Ground,
    Start,
    Intermediate,
    Csi,
    Osc,
    OscEscape,
    String,
    StringEscape,
}

#[derive(Default)]
pub(crate) struct TerminalTextSanitizer {
    escape: TerminalEscapeState,
    pending_carriage_return: bool,
}

impl TerminalTextSanitizer {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn push(&mut self, input: &str) -> String {
        let mut output = String::with_capacity(input.len());
        for character in input.chars() {
            if self.pending_carriage_return {
                output.push('\n');
                self.pending_carriage_return = false;
                if character == '\n' {
                    continue;
                }
            }
            if matches!(character, '\u{18}' | '\u{1a}') {
                self.escape = TerminalEscapeState::Ground;
                continue;
            }
            self.escape = match self.escape {
                TerminalEscapeState::Ground => match character {
                    '\u{1b}' => TerminalEscapeState::Start,
                    '\u{9b}' => TerminalEscapeState::Csi,
                    '\u{9d}' => TerminalEscapeState::Osc,
                    '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}' => TerminalEscapeState::String,
                    '\r' => {
                        self.pending_carriage_return = true;
                        TerminalEscapeState::Ground
                    }
                    '\n' | '\t' => {
                        output.push(character);
                        TerminalEscapeState::Ground
                    }
                    value if value.is_control() || unsafe_terminal_format(value) => {
                        TerminalEscapeState::Ground
                    }
                    value => {
                        output.push(value);
                        TerminalEscapeState::Ground
                    }
                },
                TerminalEscapeState::Start => match character {
                    '[' => TerminalEscapeState::Csi,
                    ']' => TerminalEscapeState::Osc,
                    'P' | 'X' | '^' | '_' => TerminalEscapeState::String,
                    '\u{20}'..='\u{2f}' => TerminalEscapeState::Intermediate,
                    _ => TerminalEscapeState::Ground,
                },
                TerminalEscapeState::Intermediate => {
                    if ('\u{30}'..='\u{7e}').contains(&character) {
                        TerminalEscapeState::Ground
                    } else {
                        TerminalEscapeState::Intermediate
                    }
                }
                TerminalEscapeState::Csi => {
                    if ('\u{40}'..='\u{7e}').contains(&character) {
                        TerminalEscapeState::Ground
                    } else {
                        TerminalEscapeState::Csi
                    }
                }
                TerminalEscapeState::Osc if character == '\u{7}' => TerminalEscapeState::Ground,
                TerminalEscapeState::Osc if character == '\u{1b}' => TerminalEscapeState::OscEscape,
                TerminalEscapeState::Osc => TerminalEscapeState::Osc,
                TerminalEscapeState::OscEscape if character == '\\' => TerminalEscapeState::Ground,
                TerminalEscapeState::OscEscape => TerminalEscapeState::Osc,
                TerminalEscapeState::String if character == '\u{1b}' => {
                    TerminalEscapeState::StringEscape
                }
                TerminalEscapeState::String => TerminalEscapeState::String,
                TerminalEscapeState::StringEscape if character == '\\' => {
                    TerminalEscapeState::Ground
                }
                TerminalEscapeState::StringEscape => TerminalEscapeState::String,
            };
        }
        output
    }

    fn finish(self) -> String {
        if self.pending_carriage_return {
            "\n".to_string()
        } else {
            String::new()
        }
    }
}

fn unsafe_terminal_format(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

fn sanitize_complete_terminal_text(input: &str) -> String {
    let mut sanitizer = TerminalTextSanitizer::default();
    let mut output = sanitizer.push(input);
    output.push_str(&sanitizer.finish());
    output
}

#[cfg(test)]
pub(crate) fn mention_path_matches(workspace: &Path, query: &str) -> Result<Vec<String>, ()> {
    mention_path_matches_inner(workspace, query)
}

fn mention_path_matches_inner(workspace: &Path, query: &str) -> Result<Vec<String>, ()> {
    let (parent_label, leaf) = completion_parent_and_leaf(query)?;
    let parent = Path::new(parent_label);

    let mut matches = crate::agent::harness::confined_read_dir(workspace, parent)
        .map_err(|_| ())?
        .into_iter()
        .filter_map(|entry| {
            let name = entry.name.to_str()?;
            if name == "off-limits" || !safe_completion_name(name) || !name.starts_with(leaf) {
                return None;
            }
            let mut completed = if parent_label.is_empty() {
                name.to_string()
            } else {
                format!("{parent_label}/{name}")
            };
            if entry.is_dir {
                completed.push('/');
            }
            Some(completed)
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    matches.truncate(64);
    Ok(matches)
}

fn completion_parent_and_leaf(query: &str) -> Result<(&str, &str), ()> {
    use std::path::Component;

    let path = Path::new(query);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(());
    }
    let (parent_label, leaf) = query.rsplit_once('/').unwrap_or(("", query));
    let parent = Path::new(parent_label);
    if parent
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .any(|component| component == "off-limits")
    {
        return Err(());
    }
    Ok((parent_label, leaf))
}

fn safe_completion_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|character| {
            !character.is_control()
                && !matches!(
                    character,
                    '\u{061c}'
                        | '\u{200b}'..='\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2060}'..='\u{206f}'
                        | '\u{feff}'
                )
        })
}

#[cfg(test)]
pub(crate) fn path_longest_common_prefix(matches: &[String]) -> String {
    path_longest_common_prefix_inner(matches)
}

fn path_longest_common_prefix_inner(matches: &[String]) -> String {
    let Some(first) = matches.first() else {
        return String::new();
    };
    let mut prefix = first.clone();
    for candidate in &matches[1..] {
        while !candidate.starts_with(&prefix) {
            if prefix.pop().is_none() {
                return prefix;
            }
        }
    }
    prefix
}

fn stream_event_payload_bytes(event: &TurnEvent) -> usize {
    match event {
        TurnEvent::Token(text)
        | TurnEvent::Reasoning(text)
        | TurnEvent::Notice(text)
        | TurnEvent::RolloutCaptureError(text) => text.len(),
        TurnEvent::Heartbeat | TurnEvent::SuppressPartial | TurnEvent::SpendMilestone { .. } => 0,
        TurnEvent::SubmissionSlot(slot) => slot.payload_bytes(),
        TurnEvent::ResearchEvidence(_) => 0,
        TurnEvent::GraphCrop { id, event } => {
            id.0.len()
                + match event.as_ref() {
                    crate::knowledge::graph_crop::GraphEvent::Requested(request) => {
                        serde_json::to_vec(request).map_or(0, |v| v.len())
                    }
                    crate::knowledge::graph_crop::GraphEvent::Returned(receipt) => {
                        serde_json::to_vec(receipt).map_or(0, |v| v.len())
                    }
                }
        }
        TurnEvent::ToolCall {
            name, args_summary, ..
        } => name.len().saturating_add(args_summary.len()),
        TurnEvent::ToolResult { name, summary, .. } => name.len().saturating_add(summary.len()),
        TurnEvent::Media { kind, label, url } => kind
            .len()
            .saturating_add(label.len())
            .saturating_add(url.len()),
    }
}

fn tui_attention_enabled() -> bool {
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| crate::agent::harness::env_flag("ANGEL_TUI_ATTENTION", true))
    }
    #[cfg(test)]
    crate::agent::harness::env_flag("ANGEL_TUI_ATTENTION", true)
}

fn stream_event_is_operator_visible(event: &TurnEvent) -> bool {
    match event {
        TurnEvent::Token(text)
        | TurnEvent::Reasoning(text)
        | TurnEvent::Notice(text)
        | TurnEvent::RolloutCaptureError(text) => !text.is_empty(),
        TurnEvent::Heartbeat | TurnEvent::SuppressPartial | TurnEvent::ResearchEvidence(_) => false,
        TurnEvent::ToolCall { .. }
        | TurnEvent::ToolResult { .. }
        | TurnEvent::GraphCrop { .. }
        | TurnEvent::SpendMilestone { .. }
        | TurnEvent::SubmissionSlot(_)
        | TurnEvent::Media { .. } => true,
    }
}

fn notice_gauge_row_index(messages: &[Message], note: &str) -> Option<usize> {
    let key = crate::ui::views::turn_event_view::notice_coalesce_key(note);
    messages
        .iter()
        .enumerate()
        .rev()
        .take_while(|(_, message)| !matches!(message.role, Role::User))
        .filter(|(_, message)| matches!(message.role, Role::Activity))
        .find_map(|(index, message)| {
            let body = crate::ui::views::turn_event_view::activity_notice_body(&message.text)?;
            let same_slot = match key {
                Some(key) => {
                    crate::ui::views::turn_event_view::notice_coalesce_key(body) == Some(key)
                }
                None => body == note,
            };
            same_slot.then_some(index)
        })
}

impl From<LoopStatus> for LoopPhase {
    fn from(status: LoopStatus) -> Self {
        match status {
            LoopStatus::Idle => Self::Idle,
            LoopStatus::Baselining => Self::Baselining,
            LoopStatus::Running => Self::Running,
            LoopStatus::Verifying => Self::Verifying,
            LoopStatus::AwaitingApproval => Self::AwaitingApproval,
            LoopStatus::Paused => Self::Paused,
            LoopStatus::Done => Self::Done,
            LoopStatus::Stopped => Self::Stopped,
            LoopStatus::Failed => Self::Failed,
        }
    }
}

impl From<EscalationTier> for LoopTier {
    fn from(tier: EscalationTier) -> Self {
        match tier {
            EscalationTier::Local => Self::Local,
            EscalationTier::Swarm => Self::Swarm,
            EscalationTier::Sota => Self::Sota,
        }
    }
}

impl LoopState {
    pub(crate) fn feed_facts(&self) -> LoopFacts {
        let settled = || {
            self.submission_results
                .iter()
                .filter_map(|r| r.official.as_ref())
        };
        let promoted: Vec<_> = settled().filter(|o| o.promoted_improvement()).collect();
        let failed: Vec<_> = settled().filter(|o| promotion_failed(o)).collect();
        let active = matches!(
            self.status,
            LoopStatus::Baselining
                | LoopStatus::Running
                | LoopStatus::Verifying
                | LoopStatus::AwaitingApproval
        );
        let elapsed_secs = if !active || self.started_ms == 0 {
            0
        } else {
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            now_ms.saturating_sub(self.started_ms) / 1000
        };
        LoopFacts {
            active,
            has_id: !self.id.is_empty(),
            phase: self.status.into(),
            iteration: self.iteration,
            tier: self.tier.into(),
            submissions: self.submissions,
            podrace: self.podrace,
            pivot: self.pivot,
            measured_candidates: self.measured_candidates,
            verifier_blocked: self.verifier_blocked.clone(),
            task: self.task.clone(),
            stale_count: self.stale_count,
            checkpoint: self.durable_settlement_events().map(|events| Checkpoint {
                loop_id: events.loop_id,
                workspace: events.workspace,
                rows: events.rows,
                plans: events
                    .plans
                    .iter()
                    .filter_map(|(iteration, plan)| {
                        Some((*iteration, serde_json::to_value(plan).ok()?))
                    })
                    .collect(),
            }),
            last_log_all_errors: self.log.last().is_some_and(|entry| {
                entry.tool_errors > 0 && entry.tool_errors >= entry.tool_calls
            }),
            idle_log_tail: self
                .log
                .iter()
                .rev()
                .take_while(|entry| {
                    entry.tool_calls <= entry.tool_errors
                        && !entry.workspace_changed
                        && entry.new_findings == 0
                        && entry.outcome_progress == 0
                        && entry.novel_outcome_actions == 0
                        && entry.verified_outcome_actions == 0
                })
                .count(),
            promotions: promoted.len(),
            last_promotion_score: promoted.last().and_then(|o| o.official_score.clone()),
            promotion_failures: failed.len(),
            last_promotion_failure_score: failed.last().and_then(|o| o.official_score.clone()),
            budget: LoopBudget {
                max_iters: self.max_iters,
                tokens_spent: self.tokens_spent,
                token_budget: self.token_budget,
                elapsed_secs,
                deadline_secs: self.deadline_secs,
            },
        }
    }
}

fn promotion_failed(o: &SubmissionStatus) -> bool {
    match SubmissionPhase::from_status(&o.status) {
        SubmissionPhase::Rejected | SubmissionPhase::TimedOut => true,
        SubmissionPhase::Accepted => {
            !o.promoted_improvement()
                && (o.improved == Some(false)
                    || o.promotion_status
                        .as_deref()
                        .is_some_and(|s| s != "promoted" && s != "unavailable"))
        }
        _ => false,
    }
}

pub(crate) fn serving_of(
    thinking: Option<&Thinking>,
    bag: &crate::agent::club::Bag,
    engagement: Option<&crate::agent::formations::MoaEngagement>,
) -> Serving {
    let lead = match thinking {
        Some(thinking) => route_of_thinking(thinking),
        None => {
            let chrome = bag.in_hand_chrome();
            route_with_key(
                bag.in_hand_label().to_string(),
                chrome.route.driver.clone(),
                chrome.route.model.clone(),
            )
        }
    };
    let seats = engagement
        .map(|engagement| {
            engagement
                .roster
                .assignments()
                .iter()
                .flatten()
                .map(|seat| {
                    route_with_key(
                        seat.agent.clone(),
                        seat.driver.clone(),
                        Some(seat.model.clone()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    Serving {
        lead,
        seats,
        turn: thinking.is_some(),
    }
}

pub(crate) fn route_of_thinking(thinking: &Thinking) -> Route {
    let resolved = thinking
        .club
        .as_ref()
        .and_then(|club| club.resolved_route_if_known());
    let route = resolved.as_ref().unwrap_or(&thinking.requested_route);
    route_with_key(
        thinking.club_label.clone(),
        route.driver.clone(),
        route.model.clone(),
    )
}

fn route_with_key(label: String, driver: String, model: Option<String>) -> Route {
    let key = crate::ui::agent_panel::profile::profile_for_route(&label, &driver, model.as_deref()).key;
    Route {
        label,
        driver,
        model,
        key,
    }
}

fn world_tick_budget(elapsed: std::time::Duration) -> (u32, bool) {
    let due = elapsed.as_nanos() / WORLD_TICK_INTERVAL.as_nanos();
    (
        due.min(u128::from(MAX_WORLD_CATCH_UP_TICKS)) as u32,
        due > u128::from(MAX_WORLD_CATCH_UP_TICKS),
    )
}

impl App {
    fn advance_world_clock(&mut self) {
        let now = std::time::Instant::now();
        if !crate::drive::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
            self.world_ticked_at = now;
            return;
        }
        let elapsed = now.saturating_duration_since(self.world_ticked_at);
        let (ticks, dropped_backlog) = world_tick_budget(elapsed);
        if ticks == 0 {
            return;
        }
        self.realm.feed(Feed::Ticks(ticks));
        self.world_ticked_at = if dropped_backlog {
            now
        } else {
            self.world_ticked_at
                .checked_add(WORLD_TICK_INTERVAL.saturating_mul(ticks))
                .unwrap_or(now)
        };
    }

    fn roll_reasoning(&mut self) {
        const ROLL_STEP: usize = 8;
        let len = self.reasoning.len();
        if !Self::reasoning_roll_in_allowed() {
            self.reasoning_shown = len;
            return;
        }
        if self.reasoning_shown >= len {
            self.reasoning_shown = len;
            return;
        }
        self.reasoning_shown =
            reasoning_reveal_boundary(&self.reasoning, self.reasoning_shown, ROLL_STEP);
    }

    fn push_stream_token(&mut self, delta: &str) {
        if self.stream_artifact_notice {
            return;
        }
        self.partial.push_str(delta);
        if self.stream_artifact_detector.push(delta) {
            self.stream_artifact_notice = true;
            self.clear_partial();
            self.partial
                .push_str("working on artifact — generated file output is being kept out of chat");
        }
    }

    pub(crate) fn clear_partial(&mut self) {
        self.partial.clear();
        self.stream_text_sanitizer.reset();
        self.stream_artifact_detector = Default::default();
        self.partial_height_cache.clear();
    }

    pub(crate) fn present_media(&mut self, kind: &str, label: &str, url: &str) {
        if !crate::ui::media::presentation_text_valid(label, 512)
            || !crate::ui::media::presentation_text_valid(url, 8192)
        {
            self.system_msg("Stage rejected malformed presentation identity".to_string());
            return;
        }
        let root = self
            .tools
            .current_workspace()
            .canonicalize()
            .unwrap_or_else(|_| self.tools.current_workspace().to_path_buf());
        let card = Media::Confined {
            card: Box::new(crate::ui::views::turn_event_view::media_card(
                kind, label, url,
            )),
            root,
        };
        self.media.push(card);
        crate::ui::showcase::open(&self.media[self.media.len() - 1]);
        let revealed = self.scryglass.receive_media(self.media.len() - 1);
        let _ = self
            .module_host
            .activate(&crate::platform::runtime::ModuleId::new("artifacts"));
        self.messages.push(Message {
            role: Role::Activity,
            text: format!(
                "{}\nSource: {}",
                crate::ui::views::turn_event_view::media_delivery_text(
                    label,
                    Ok(if revealed {
                        "opened in Scryglass · /assets keeps every arrival".into()
                    } else {
                        "saved in Assets · current preview kept open".into()
                    })
                ),
                url.escape_debug()
            )
            .into(),
        });
    }

    pub(crate) fn display_reply(&mut self, reply: String) -> String {
        self.stream_artifact_notice = false;
        let terminal_safe_reply = sanitize_complete_terminal_text(&reply);
        let Some(candidate) = large_code_document(&reply) else {
            return terminal_safe_reply;
        };
        match self.write_reply_artifact(&candidate) {
            Ok(path) => {
                let label = candidate.label();
                self.media.push(Media::Confined {
                    card: Box::new(Media::Resource {
                        label: label.clone(),
                        url: path.to_string_lossy().to_string(),
                    }),
                    root: self.tools.workspace_boundary().canonical_root.clone(),
                });
                self.scryglass.receive_media(self.media.len() - 1);
                let _ = self
                    .module_host
                    .activate(&crate::platform::runtime::ModuleId::new("artifacts"));
                let mut note = format!(
                    "artifact captured · {label}\nfile: {}\nOpen /assets to view and copy the full document.",
                    path.display()
                );
                if let Some(context) = candidate.context_note() {
                    note.push_str("\n\n");
                    note.push_str(&context);
                }
                note
            }
            Err(e) => format!("artifact capture failed ({e}); generated document hidden from chat"),
        }
    }

    fn write_reply_artifact(&self, candidate: &CodeDocument<'_>) -> Result<PathBuf, String> {
        let root = &self.tools.workspace_boundary().canonical_root;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let relative = PathBuf::from("angel_test_output")
            .join(format!("angel-response-{stamp}.{}", candidate.extension()));
        crate::agent::harness::confined_create_new_no_symlinks(
            root,
            &relative,
            candidate.body.as_bytes(),
        )?;
        Ok(root.join(relative))
    }

    fn drain_background_process_completions(&mut self) {
        let workspace = self.tools.current_workspace().to_path_buf();
        let canonical = &self.tools.workspace_boundary().canonical_root;
        let driving = self.loop_ctl.status == crate::drive::loop_ctl::LoopStatus::Running
            && self.loop_ctl.workspace.as_deref() == Some(workspace.as_path());
        let limit = if driving {
            8usize.saturating_sub(self.loop_ctl.pending_proc_completions.len())
        } else {
            8
        };
        let finished = crate::agent::tools::proc::take_completions(canonical, limit);
        for completion in &finished {
            self.system_msg(completion.message());
            queue_proc_completion_for_loop(
                &mut self.loop_ctl,
                &workspace,
                completion.loop_message(),
            );
        }
        if !finished.is_empty() {
            let book = crate::agent::harness::book::p_processes::completions(&finished);
            self.history.push(ChatMsg::harness(format!(
                "{}\n{}",
                crate::agent::harness::book::warpath(&workspace, &book),
                crate::agent::harness::book::p_processes::receipt_lines(&finished),
            )));
        }
    }

    pub(crate) fn advance(&mut self) {
        self.tick_approval();
        self.advance_live_modules();
        if let Some(payload) = crate::ui::showcase::poll() {
            self.pending_stage_copy = Some(payload);
        }
        if let Some(guest) = &mut self.table_guest {
            guest.poll();
        }
        if let Some(host) = &mut self.table_host {
            host.poll();
        }
        self.advance_launch_input();
        self.advance_setup();
        self.tick_realm();
        self.drain_background_process_completions();
        self.flush_pending_atlas_harvest();
        self.poll_shell_status();
        self.clear_expired_lifecycle_ceremony();
        if Self::side_column_visuals_allowed() && self.council.is_live() {
            self.council.tick();
        }
        for _ in 0..8 {
            let Ok(notice) = self.background_notice_rx.try_recv() else {
                break;
            };
            let message = notice.message(self.tools.current_workspace());
            self.system_msg(message);
        }
        match self.clipboard_paste.poll() {
            Some(crate::ui::clipboard::Drained::Staged)
                if self.clipboard_paste.take_deferred_submit() =>
            {
                self.submit();
            }
            Some(crate::ui::clipboard::Drained::Text(text)) => {
                self.insert_pasted_text(&text);
                if self.clipboard_paste.take_deferred_submit() {
                    self.submit();
                }
            }
            Some(crate::ui::clipboard::Drained::Failed(error)) => {
                self.system_msg(format!("clipboard paste failed · {error}"));
            }
            Some(crate::ui::clipboard::Drained::Staged) | None => {}
        }
        let foreground_idle = self.exit_request.is_none()
            && self.thinking.is_none()
            && self.bg_job.is_none()
            && self.loop_pending.is_none()
            && self.pending_approval.is_none();
        let clerk = self.tools.clerk();
        if crate::agent::backplane::mode() != crate::agent::backplane::BackplaneMode::Legacy {
            clerk.tick(
                foreground_idle,
                || self.bag.atlas_teacher_route(),
                self.tools.backplane(),
            );
        }
        if crate::drive::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
            self.realm.health(&|| {
                let memory = self.tools.store.health();
                let atlas_status = self.atlas.status();
                let clerk_status = clerk.status();
                Health {
                    memory,
                    atlas: atlas_status.health,
                    atlas_reviews: atlas_status.review_count,
                    clerk: clerk_status.health,
                    clerk_queue: clerk_status.queue_depth,
                    clerk_conflicts: usize::from(
                        clerk_status.health
                            == crate::knowledge::atlas_clerk::ClerkHealth::ResourceConflict,
                    ),
                }
            });
        }
        self.realm.serving(&|| {
            serving_of(
                self.thinking.as_ref(),
                &self.bag,
                self.moa_one_shot.as_ref().or(self.moa_session.as_ref()),
            )
        });
        match self.session.save_status() {
            crate::knowledge::session::SessionSaveStatus::Failed(error) => {
                if !self.session_warning_notified {
                    self.messages.push(Message {
                        role: Role::System,
                        text: format!(
                            "session save warning · {error} · conversation remains in memory"
                        )
                        .into(),
                    });
                    self.session_warning_notified = true;
                }
            }
            crate::knowledge::session::SessionSaveStatus::Pending => {}
            crate::knowledge::session::SessionSaveStatus::Healthy => {
                self.session_warning_notified = false;
            }
        }
        self.drain_reborn();
        if crate::drive::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
            self.scryglass.poll_lesson(self.visual_motion.animates());
        }
        if !Self::reasoning_roll_in_allowed() {
            self.reasoning_shown = self.reasoning.len();
        } else if self.thinking.is_some() || self.reasoning_roll_pending() {
            self.roll_reasoning();
        }
        self.steer_walk();
        self.advance_world_clock();
        self.adventure_mirror_drain();
        if self.loop_active() {
            self.note_ops_loop(self.loop_ctl.iteration);
        } else {
            self.realm.feed(Feed::LoopIdle);
        }
        if crate::drive::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
            let at_quintain = self
                .realm
                .loop_noted(&|| self.loop_ctl.feed_facts());
            if at_quintain
                && !self.quintain_route_presented
                && self.scryglass.controller.route() == crate::ui::scryglass::StageRoute::Realm
                && self.scryglass.controller.overlay().is_none()
            {
                self.scryglass
                    .navigate(crate::ui::scryglass::StageRoute::Loop);
            }
            self.quintain_route_presented = at_quintain;
        } else {
            self.quintain_route_presented = false;
        }
        self.drain_village_pulses();
        self.advance_comp_fleet();
        if self.pending_approval.is_none()
            && let Ok(req) = self.approval_rx.try_recv()
        {
            if self.thinking.as_ref().is_none_or(Thinking::is_draining) && self.bg_job.is_none() {
                let _ = req.reply.send(crate::agent::approval::Decision::Deny);
            } else if std::time::Instant::now() >= req.deadline {
                let _ = req.reply.send(crate::agent::approval::Decision::Deny);
            } else {
                let scope_label = req.scope.label();
                self.approval_focus = (0, 0);
                self.approval_deadline = Some((
                    crate::ui::views::approval_view::serial(&req.prompt, Some(scope_label.as_str())),
                    req.deadline,
                ));
                self.pending_approval = Some(PendingApproval {
                    prompt: req.prompt,
                    scope_label: Some(scope_label),
                    reply: req.reply,
                });
                self.request_terminal_attention();
            }
        }
        if crate::platform::yolo::enabled()
            && let Some(pending) = self.take_approval()
        {
            let _ = pending
                .reply
                .send(crate::agent::approval::Decision::ApproveAll);
        }
        self.campaign_drain_pending();
        self.loop_drain_experiment();
        if let Some(job) = self.bg_job.as_ref() {
            match job.try_recv() {
                Ok(outcome) => {
                    let output = job.live_output();
                    self.last_background_operation = output.as_ref().map(|_| job.operation());
                    self.last_background_output = output;
                    self.bg_job = None;
                    match outcome {
                        BgOutcome::Note(text) => self.system_msg(text),
                        BgOutcome::Compact {
                            range,
                            note,
                            plan,
                            deposits,
                            message,
                        } => {
                            if range.start <= range.end && range.end <= self.history.len() {
                                if let Ok(content) = crate::agent::harness::park_compaction_window(
                                    &self.history[range.clone()],
                                    &note.content,
                                ) {
                                    let newer_user_survives = self.history[range.end..]
                                        .iter()
                                        .any(|m| m.role == ChatRole::User);
                                    let anchors = crate::agent::harness::compaction_task_anchors(
                                        &self.history,
                                        range.start,
                                        range.end,
                                        crate::agent::compaction::configured_compact_chunk_tokens()
                                            .saturating_mul(2),
                                    )
                                    .into_iter()
                                    .filter(|anchor| {
                                        !newer_user_survives
                                            || crate::agent::harness::operator_directive(anchor)
                                    })
                                    .collect::<Vec<_>>();
                                    let turn_context =
                                        crate::agent::harness::compaction_turn_context_anchor(
                                            &self.history,
                                            range.start,
                                            range.end,
                                        );
                                    let mut note = note;
                                    note.content =
                                        if crate::agent::compaction::is_compaction_note(&note) {
                                            crate::agent::harness::constraint_retention_note(
                                                content,
                                                &self.history[range.clone()],
                                                &anchors,
                                                &self.history[range.end..],
                                            )
                                        } else {
                                            content
                                        }
                                        .into();
                                    self.history.splice(
                                        range,
                                        std::iter::once(*note)
                                            .chain(plan.map(|plan| *plan))
                                            .chain(anchors.into_iter().map(ChatMsg::user))
                                            .chain(turn_context),
                                    );
                                    let _ = self.session.save_async(&self.history);
                                    self.rebuild_display();
                                    if let Some(deposits) = deposits {
                                        deposits.spawn(self.background_notice_tx.clone());
                                    }
                                    self.system_msg(format!(
                                        "{message}; operator task/answer contract retained"
                                    ));
                                    self.realm.feed(Feed::Compacted);
                                } else {
                                    self.system_msg("context compaction evidence could not be retained; history left intact".to_string());
                                }
                            } else {
                                self.system_msg(
                                    "context compaction result rejected — conversation history \
                                     changed before it could be applied; history and long-term \
                                     memory were left intact. Retry /compact"
                                        .to_string(),
                                );
                            }
                        }
                    }
                    self.scroll = 0;
                    self.request_terminal_attention();
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    let message = job.disconnected_message();
                    let output = job.live_output();
                    self.last_background_operation = output.as_ref().map(|_| job.operation());
                    self.last_background_output = output;
                    self.bg_job = None;
                    self.system_msg(message);
                    self.scroll = 0;
                    self.request_terminal_attention();
                }
            }
        }
        if self.exit_request.is_some() && self.thinking.is_none() {
            self.loop_drain_pending();
            self.finish_requested_exit();
            return;
        }
        if self.thinking.is_none() && self.pending_turn.is_some() {
            self.loop_drain_pending();
        }
        if self.thinking.is_none()
            && self.bg_job.is_none()
            && self.loop_pending.is_none()
            && self.pending_turn.as_ref().is_some_and(|p| p.echo_drawn)
        {
            self.launch_pending_turn();
        }
        let mut thinking = match self.thinking.take() {
            Some(t) => t,
            None => {
                if self.pending_turn.is_some() {
                    return;
                }
                if self.campaign_pending.is_none() {
                    self.loop_drain_pending();
                    self.loop_arm();
                    self.flush_queued_steers();
                }
                return;
            }
        };
        self.loop_heartbeat();

        if thinking.is_draining() {
            match thinking.rx.try_recv() {
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {
                    self.fold_turn_cache(&thinking);
                    self.turn_first_output_ms = None;
                    self.route_evidence_loaded_at = None;
                    self.messages.push(Message {
                        role: Role::Activity,
                        text: "stopped worker drained — provider/tool slot released".into(),
                    });
                    self.scroll = 0;
                    self.request_terminal_attention();
                    if let Some(candidate_summary) = self.pending_handoff_rl_restart.take()
                        && let Err(error) = self.force_handoff_rl_restart(candidate_summary.as_deref())
                    {
                        self.system_msg(format!("handoff restart could not resume after drain: {error}"));
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.thinking = Some(thinking);
                }
            }
            return;
        }

        let steer_idle_secs = thinking.last_stream_at.elapsed().as_secs();
        if !self.tool_strip.has_running_calls()
            && !thinking.steer_interrupt_fired
            && crate::agent::turn::steer_interrupt_due(
                steer_idle_secs,
                self.steer_queue.len(),
                thinking.steer_idle_interrupt_secs,
            )
            && !crate::agent::tools::proc::background_work_pending()
        {
            thinking.steer_interrupt_fired = true;
            thinking
                .cancel
                .store(true, std::sync::atomic::Ordering::Release);
            let queued = self.steer_queue.len();
            self.messages.push(Message {
                role: Role::Activity,
                text: format!(
                    "⏱ {} idle {steer_idle_secs}s with {queued} steer(s) queued — \
                     interrupting the stalled request to deliver them \
                     (ANGEL_STEER_IDLE_INTERRUPT_SECS)",
                    crate::ui::views::turn_event_view::route_label(&thinking.requested_route),
                )
                .into(),
            });
            self.thinking = Some(thinking);
            return;
        }
        if let Some(idle_timeout) = thinking
            .idle_timeout_secs
            .filter(|_| !self.tool_strip.has_running_calls())
        {
            let idle_secs = thinking.last_stream_at.elapsed().as_secs();
            let total_secs = thinking.started.elapsed().as_secs();
            if idle_secs >= idle_timeout && total_secs >= idle_timeout {
                thinking.begin_draining();
                if let Some(pending) = self.take_approval() {
                    let _ = pending.reply.send(crate::agent::approval::Decision::Deny);
                }
                self.turn_first_output_ms = None;
                self.route_evidence_loaded_at = None;
                self.finish_world_turn(false);
                self.restore_moa_after_turn();
                let loop_tools = self
                    .loop_ctl
                    .awaiting_turn
                    .then(|| self.tool_strip.snapshot());
                self.flush_tool_summary();
                if self.loop_ctl.awaiting_turn {
                    self.thinking = Some(thinking);
                    self.loop_harvest_error_with_tools(
                        format!("turn idle timeout ({idle_timeout}s with no stream progress)"),
                        loop_tools.unwrap_or_default(),
                    );
                    self.scroll = 0;
                    return;
                }
                let retained_partial = self.retain_interrupted_partial();
                self.stream_artifact_notice = false;
                self.messages.push(Message {
                    role: Role::System,
                    text: format!(
                        "⏱ turn abandoned — no stream progress for {idle_secs}s (operator cap ANGEL_TURN_IDLE_TIMEOUT_SECS={idle_timeout}) on {}. The provider may be hung; try a different route{}",
                        crate::ui::views::turn_event_view::route_label(&thinking.requested_route),
                        if retained_partial {
                            RETAINED_PARTIAL_NOTE
                        } else {
                            ""
                        }
                    ).into(),
                });
                self.request_terminal_attention();
                self.thinking = Some(thinking);
                return;
            }
            if let Some(warning) = thinking.idle_warning_due() {
                self.messages.push(Message {
                    role: Role::Activity,
                    text: format!(
                        "⏱ {} idle {}s — no stream progress; abandoning at {}s ({}% of idle budget; ANGEL_TURN_IDLE_TIMEOUT_SECS)",
                        crate::ui::views::turn_event_view::route_label(&thinking.requested_route),
                        warning.idle_secs,
                        warning.idle_timeout_secs,
                        warning.stage_pct
                    ).into(),
                });
                if warning.stage_pct >= 80 {
                    self.request_terminal_attention();
                }
            }
        }

        let mut touched = false;
        let mut drained_events = 0usize;
        let mut drained_bytes = 0usize;
        let drop_stream = thinking.cancel.load(std::sync::atomic::Ordering::Relaxed);
        if drop_stream {
            thinking.discard_buffered_stream();
        }
        while !drop_stream
            && drained_events < STREAM_EVENTS_PER_FRAME
            && drained_bytes < STREAM_BYTES_PER_FRAME
        {
            let Ok(event) = thinking.event_rx.try_recv() else {
                break;
            };
            if self.turn_first_output_ms.is_none() && stream_event_is_operator_visible(&event) {
                self.turn_first_output_ms = Some(thinking.started.elapsed().as_millis() as u64);
            }
            drained_events += 1;
            drained_bytes = drained_bytes.saturating_add(stream_event_payload_bytes(&event));
            match event {
                TurnEvent::Token(delta) => {
                    let delta = self.stream_text_sanitizer.push(&delta);
                    self.push_stream_token(&delta);
                }
                TurnEvent::Reasoning(reasoning) => {
                    let reasoning = self.reasoning_text_sanitizer.push(&reasoning);
                    self.reasoning.push_str(&reasoning);
                }
                TurnEvent::GraphCrop { id, event } => {
                    if let crate::knowledge::graph_crop::GraphEvent::Returned(receipt) = &*event
                        && matches!(receipt.request, crate::knowledge::graph_crop::GraphRequest::Finish { .. })
                        && let Some(chart) = receipt.chart.as_ref().filter(|chart| chart.finished)
                    {
                        crate::ui::showcase::open_chart(chart);
                    }
                    self.realm.graph_event(&id, &event);
                    self.request_redraw("graph crop data");
                }
                TurnEvent::Heartbeat => {}
                TurnEvent::SuppressPartial => {
                    self.clear_partial();
                    self.stream_artifact_notice = false;
                }
                TurnEvent::ToolCall {
                    id,
                    name,
                    args_summary,
                } => {
                    self.ensure_research_session();
                    self.research.start(&id, &name, &args_summary);
                    self.note_ops_call(&id.0, &name, &args_summary);
                    self.flush_partial();
                    let painted =
                        crate::drive::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible);
                    self.realm.feed(Feed::ToolCall {
                        id: &id,
                        name: &name,
                        args: &args_summary,
                        painted,
                    });
                    if painted
                        && let Some(destination) = realm::GameHost::tool_destination(&name, &args_summary)
                    {
                        self.reset_world_yaw();
                        self.scryglass.begin_journey(
                            id.clone(),
                            destination,
                            self.visual_motion != crate::ui::viz::lifecycle_viz::MotionMode::Full,
                        );
                    }
                    if self.transcript_mode == crate::app::TranscriptMode::Trace {
                        self.messages.push(Message {
                            role: Role::Activity,
                            text: crate::ui::views::turn_event_view::tool_call_text(
                                &name,
                                &args_summary,
                            )
                            .into(),
                        });
                    }
                    self.tool_strip.call_event(id, &name, &args_summary);
                }
                TurnEvent::ToolResult {
                    id,
                    name,
                    summary,
                    outcome,
                } => {
                    self.research.result(&id, &name, &summary, outcome);
                    self.note_ops_result(&id.0, outcome.attributable_success());
                    self.realm.feed(Feed::ToolResult {
                        id: &id,
                        name: &name,
                        summary: &summary,
                        outcome,
                        painted: crate::drive::comp_mode::stage_world_mirrors_allowed(
                            self.world_pane_visible,
                        ),
                    });
                    if self.transcript_mode == crate::app::TranscriptMode::Trace {
                        self.messages.push(Message {
                            role: Role::Activity,
                            text: crate::ui::views::turn_event_view::tool_result_text(
                                &name, &summary,
                            )
                            .into(),
                        });
                    } else if crate::ui::views::turn_event_view::is_council_tool(&name) {
                        self.flush_partial();
                        self.messages.push(Message {
                            role: Role::Council,
                            text: format!("{name} · {summary}").into(),
                        });
                    }
                    self.tool_strip.result_event(&id, &name, &summary, outcome);
                }
                TurnEvent::Notice(note) | TurnEvent::RolloutCaptureError(note) => {
                    self.realm.feed(Feed::Notice {
                        text: &note,
                        painted: crate::drive::comp_mode::stage_world_mirrors_allowed(
                            self.world_pane_visible,
                        ),
                    });
                    let conversation =
                        self.transcript_mode == crate::app::TranscriptMode::Conversation;
                    let receipt = note.starts_with("action receipt · ");
                    if receipt && conversation {
                    } else if receipt {
                        self.messages.push(Message::new(
                            Role::Activity,
                            crate::ui::views::turn_event_view::notice_text(&note),
                        ));
                    } else if conversation && self.bump_notice_gauge(&note) {
                    } else if conversation
                        && crate::ui::views::turn_event_view::notice_rides_strip(&note)
                    {
                        self.tool_strip.note_event(&note);
                    } else {
                        self.messages.push(Message {
                            role: Role::Activity,
                            text: crate::ui::views::turn_event_view::notice_text(&note).into(),
                        });
                    }
                }
                TurnEvent::SpendMilestone { input_tokens } => {
                    self.show_spend_coin(input_tokens);
                    self.tool_strip.note_event(&format!(
                        "gold coin · you just spent {} input tokens — continuing",
                        crate::ui::viz::spend_viz::format_input_tokens(input_tokens)
                    ));
                }
                TurnEvent::ResearchEvidence(evidence) => {
                    self.realm.feed(Feed::Evidence(evidence));
                }
                TurnEvent::SubmissionSlot(slot) => {
                    use crate::agent::harness::SubmissionSlotPhase;
                    let new_flight = self.submission_slot.phase != SubmissionSlotPhase::InFlight
                        || self.submission_slot.id != slot.id;
                    let same_flight = self.submission_slot.phase == SubmissionSlotPhase::InFlight
                        && self.submission_slot.id == slot.id;
                    if slot.phase != SubmissionSlotPhase::InFlight {
                        self.submission_slot_since = None;
                    } else if !same_flight || self.submission_slot_since.is_none() {
                        self.submission_slot_since = Some(std::time::Instant::now());
                    }
                    if slot.id.is_some() {
                        let id = slot.id.as_deref().unwrap_or("unknown");
                        let board = crate::agent::harness::cartridges::active()
                            .map_or("Board", |cartridge| cartridge.label());
                        let label = format!("{board} submission {}", id.chars().take(8).collect::<String>());
                        match slot.phase {
                            SubmissionSlotPhase::InFlight if new_flight => {
                                self.realm.competition(Competition::Started(&label));
                            }
                            SubmissionSlotPhase::InFlight => {
                                self.realm.competition(Competition::Pending(&label));
                            }
                            SubmissionSlotPhase::Accepted => {
                                if let Some(score) = slot.score.as_deref() {
                                    self.realm.competition(Competition::Reported {
                                        label: &label,
                                        score,
                                    });
                                } else {
                                    self.realm.competition(Competition::Complete(&format!(
                                        "{label} · accepted; score not reported"
                                    )));
                                }
                            }
                            SubmissionSlotPhase::Rejected => {
                                self.realm.competition(Competition::Error {
                                    label: &label,
                                    reason: &format!("{board} rejected the submission"),
                                });
                            }
                            SubmissionSlotPhase::TimedOut => {
                                self.realm.competition(Competition::TimedOut(&label));
                            }
                            SubmissionSlotPhase::Empty | SubmissionSlotPhase::Dormant => {}
                        }
                    }
                    self.submission_slot = slot;
                }
                TurnEvent::Media { kind, label, url } => {
                    self.present_media(&kind, &label, &url);
                }
            }
            touched = true;
        }
        if touched {
            thinking.note_stream_progress();
            self.thinking = Some(thinking);
            return;
        }

        if let Some(reason) = self.loop_watchdog(&thinking) {
            thinking.begin_draining();
            if let Some(pending) = self.take_approval() {
                let _ = pending.reply.send(crate::agent::approval::Decision::Deny);
            }
            self.turn_first_output_ms = None;
            self.route_evidence_loaded_at = None;
            self.finish_world_turn(false);
            self.restore_moa_after_turn();
            let loop_tools = self.tool_strip.snapshot();
            self.flush_tool_summary();
            self.request_terminal_attention();
            self.thinking = Some(thinking);
            self.loop_harvest_error_with_tools(reason, loop_tools);
            self.scroll = 0;
            return;
        }

        match thinking.rx.try_recv() {
            Ok(Ok((new_history, reply, resolved_route, stop_reason))) => {
                let completed = stop_reason == crate::agent::harness::TurnStopReason::Answer;
                let outcome_tools = self.tool_strip.snapshot();
                self.fold_turn_cache(&thinking);
                self.route_evidence_loaded_at = None;
                self.turn_truncated = reply.contains("response incomplete:");
                self.finish_world_turn(completed);
                if let Some(marker) =
                    crate::ui::views::agent_view::turn_end_marker(completed, self.portrait_fail_run)
                {
                    self.portrait_turn_marker = Some((marker, std::time::Instant::now()));
                }
                self.portrait_fail_run = if completed {
                    0
                } else {
                    self.portrait_turn_marker = None;
                    self.portrait_fail_run.saturating_add(1)
                };
                if !completed {
                    self.relentless_execution = false;
                    if self.handoff_rl.active {
                        let note = self.handoff_rl.stop_for_guard(stop_reason.as_str());
                        self.handoff_rl_unbind();
                        self.system_msg(note);
                    }
                }
                self.restore_moa_after_turn();
                let elapsed_ms = thinking.started.elapsed().as_millis() as u64;
                let first_output_ms = self
                    .turn_first_output_ms
                    .take()
                    .unwrap_or(elapsed_ms)
                    .min(elapsed_ms);
                self.turn_route_receipt = (!self.loop_ctl.awaiting_turn).then(|| {
                    crate::ui::views::turn_event_view::answer_receipt_text(
                        &thinking.requested_route,
                        &resolved_route,
                        first_output_ms,
                        elapsed_ms,
                    )
                });
                let loop_tools = self
                    .loop_ctl
                    .awaiting_turn
                    .then(|| self.tool_strip.snapshot());
                self.flush_tool_summary();
                if self.loop_ctl.awaiting_turn {
                    let steer_release = stop_reason
                        == crate::agent::harness::TurnStopReason::Interrupt
                        && thinking.steer_interrupt_fired
                        && !self.steer_queue.is_empty();
                    if completed || steer_release {
                        self.loop_harvest_with_tools(reply, loop_tools.unwrap_or_default());
                    } else {
                        self.loop_harvest_stopped(
                            reply,
                            loop_tools.unwrap_or_default(),
                            stop_reason,
                        );
                    }
                    self.scroll = 0;
                    return;
                }
                let completed_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as u64)
                    .unwrap_or(0);
                self.last_completed_route = Some(crate::LastCompletedRoute {
                    route: resolved_route.clone(),
                    completed_ms,
                    verdict: None,
                });
                let atlas_task = self
                    .messages
                    .iter()
                    .rev()
                    .find(|message| matches!(message.role, Role::User))
                    .map(|message| message.text.to_string())
                    .unwrap_or_default();
                self.history = new_history;
                self.research.finish_turn();
                self.undone_exchange = None;
                let verifier_receipts = vec![format!(
                    "tools:calls={} errors={} incomplete={} diagnostics={}",
                    outcome_tools.calls,
                    outcome_tools.errors,
                    outcome_tools.incomplete,
                    outcome_tools.diagnostics
                )];
                let backplane = self.tools.backplane();
                let outcome = crate::agent::backplane::TurnOutcome::settled(
                    self.atlas.project_key(),
                    &self.session.id,
                    backplane.as_ref(),
                    &thinking.requested_route,
                    &resolved_route,
                    &self.history,
                    verifier_receipts.clone(),
                    None,
                    stop_reason.as_str(),
                );
                self.pending_atlas_harvest = Some(crate::app::PendingAtlasHarvest {
                    atlas: Arc::clone(&self.atlas),
                    task: atlas_task,
                    final_answer: reply.clone(),
                    source_ids: outcome.context_source_ids.clone(),
                    verifier_receipts,
                });
                crate::knowledge::experience::record_backplane_outcome(
                    &outcome,
                    self.tools.current_workspace(),
                );
                self.last_turn_outcome = Some(outcome);
                let _ = self.session.save_async(&self.history);
                let streamed_display = (!self.stream_artifact_notice
                    && self.stream_artifact_detector.covers(&self.partial)
                    && reply == self.partial)
                    .then(|| std::mem::take(&mut self.partial));
                self.clear_partial();
                let clear_relentless =
                    self.relentless_execution && relentless_output_delivered(&reply);
                let reply: Arc<str> =
                    Arc::from(streamed_display.unwrap_or_else(|| self.display_reply(reply)));
                let reused = self.reuse_flushed_partial(&reply);
                if !reused {
                    self.collapse_superseded_partial();
                }
                if !reused && !reply.trim().is_empty() {
                    self.messages
                        .push(Message::new(Role::Angel, Arc::clone(&reply)));
                }
                if !reply.trim().is_empty() {
                    self.show_reply_evidence(&reply);
                }
                if self.handoff_rl.active {
                    self.handoff_rl.charge_tokens(&reply);
                    self.handoff_rl_charge_rl_tokens();
                    if let Some(why) = self.handoff_rl.budget_tripped() {
                        let msg = self.handoff_rl.stop_for_budget(&why);
                        self.handoff_rl_unbind();
                        self.system_msg(msg);
                    } else if self.exit_request.is_none()
                        && let Some(demand) = self
                            .handoff_rl
                            .observe_turn(&outcome_tools.outcome_actions, &reply)
                    {
                        match self.force_handoff_rl_restart(Some(&demand.summary)) {
                            Ok(()) => {
                                self.scroll = 0;
                                return;
                            }
                            Err(msg) => self.system_msg(msg),
                        }
                    }
                }
                if clear_relentless {
                    self.relentless_execution = false;
                    self.messages.push(Message {
                        role: Role::System,
                        text: "relentless execution OFF — output delivered".into(),
                    });
                }
                self.settle_transcript_spawns();
                self.request_terminal_attention();
            }
            Ok(Err(err)) => {
                self.turn_first_output_ms = None;
                let outcome_tools = self.tool_strip.snapshot();
                self.fold_turn_cache(&thinking);
                self.route_evidence_loaded_at = None;
                self.finish_world_turn(false);
                self.portrait_fail_run = self.portrait_fail_run.saturating_add(1);
                self.portrait_turn_marker = None;
                self.restore_moa_after_turn();
                let loop_tools = self
                    .loop_ctl
                    .awaiting_turn
                    .then(|| self.tool_strip.snapshot());
                self.flush_tool_summary();
                if self.loop_ctl.awaiting_turn {
                    self.loop_harvest_error_with_tools(err, loop_tools.unwrap_or_default());
                    self.scroll = 0;
                    return;
                }
                let verifier_receipts = vec![format!(
                    "tools:calls={} errors={} incomplete={} diagnostics={}",
                    outcome_tools.calls,
                    outcome_tools.errors,
                    outcome_tools.incomplete,
                    outcome_tools.diagnostics
                )];
                let backplane = self.tools.backplane();
                let outcome = crate::agent::backplane::TurnOutcome::settled(
                    self.atlas.project_key(),
                    &self.session.id,
                    backplane.as_ref(),
                    &thinking.requested_route,
                    &thinking.requested_route,
                    &self.history,
                    verifier_receipts,
                    None,
                    "provider_error",
                );
                crate::knowledge::experience::record_backplane_outcome(
                    &outcome,
                    self.tools.current_workspace(),
                );
                self.last_turn_outcome = Some(outcome);
                let retained_partial = self.retain_interrupted_partial();
                self.stream_artifact_notice = false;
                self.messages.push(Message {
                    role: Role::System,
                    text: format!(
                        "agent error · {err}{}",
                        if retained_partial {
                            RETAINED_PARTIAL_NOTE
                        } else {
                            ""
                        }
                    )
                    .into(),
                });
                self.request_terminal_attention();
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.thinking = Some(thinking);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.turn_first_output_ms = None;
                self.route_evidence_loaded_at = None;
                self.restore_moa_after_turn();
                let loop_tools = self
                    .loop_ctl
                    .awaiting_turn
                    .then(|| self.tool_strip.snapshot());
                self.flush_tool_summary();
                if self.loop_ctl.awaiting_turn {
                    self.loop_harvest_error_with_tools(
                        "worker vanished".to_string(),
                        loop_tools.unwrap_or_default(),
                    );
                    self.scroll = 0;
                    return;
                }
                let retained_partial = self.retain_interrupted_partial();
                self.stream_artifact_notice = false;
                self.messages.push(Message {
                    role: Role::System,
                    text: format!(
                        "agent worker vanished{}",
                        if retained_partial {
                            RETAINED_PARTIAL_NOTE
                        } else {
                            ""
                        }
                    )
                    .into(),
                });
                self.request_terminal_attention();
            }
        }
    }

    pub(crate) fn flush_pending_atlas_harvest(&mut self) {
        for _ in 0..8 {
            let Some(error) = self
                .atlas_harvester
                .as_ref()
                .and_then(|worker| worker.error())
            else {
                break;
            };
            self.system_msg(format!("Atlas harvest save failed: {error}"));
        }
        let Some(pending) = self.pending_atlas_harvest.take() else {
            return;
        };
        if self.atlas_harvester.is_none() {
            match crate::app::AtlasHarvester::new() {
                Ok(worker) => self.atlas_harvester = Some(worker),
                Err(error) => {
                    self.system_msg(format!("Atlas harvest writer could not start: {error}"));
                    return;
                }
            }
        }
        if let Some(worker) = &self.atlas_harvester
            && let Err(error) = worker.enqueue(pending)
        {
            self.system_msg(error);
        }
    }

    pub(crate) fn set_terminal_focused(&mut self, focused: bool) {
        self.terminal_focused = focused;
        if !focused {
            self.clear_dungeon_controls();
            self.steer_walk();
            self.scryglass_drag = None;
            self.viewer.inspector.drag = None;
        }
        if focused {
            self.attention_requested = false;
        }
    }

    pub(crate) fn request_terminal_attention(&mut self) {
        if !self.terminal_focused && tui_attention_enabled() {
            self.attention_requested = true;
        }
    }

    pub(crate) fn take_attention_request(&mut self) -> bool {
        std::mem::take(&mut self.attention_requested)
    }

    pub(crate) fn drain_village_pulses(&mut self) -> usize {
        self.realm.drain_village()
    }

    pub(crate) fn adventure_mirror_drain(&mut self) {
        let party = self.adventure_party_size();
        self.realm
            .adventure(&|| self.loop_ctl.feed_facts(), party);
    }

    fn adventure_party_size(&self) -> u8 {
        let engaged = self.moa_armed();
        let seats = engaged
            .map(|engagement| engagement.roster.assigned_count())
            .filter(|&seats| seats > 0)
            .unwrap_or(1);
        seats.clamp(1, 8) as u8
    }

    fn advance_comp_fleet(&mut self) {
        let cartridge = crate::agent::harness::cartridges::active();
        let label = cartridge.map_or("Fleet", |cartridge| cartridge.label());
        let mut settled = None;
        if let Some(rx) = self.comp_fleet_rx.as_ref() {
            match rx.try_recv() {
                Ok(result) => settled = Some(result),
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    settled = Some(Err(format!("{label} fleet watcher disconnected")));
                }
            }
        }
        if let Some(result) = settled {
            self.comp_fleet_rx = None;
            self.comp_fleet_polled_at = std::time::Instant::now();
            match result {
                Ok(snapshot) => {
                    if snapshot.failed_benchmarks > 0 {
                        self.realm.competition(Competition::Error {
                            label: &format!("{label} status poll"),
                            reason: &format!("{} benchmark queries failed", snapshot.failed_benchmarks),
                        });
                    } else if let Some(entry) = snapshot.entries.iter().find(|entry| {
                        matches!(
                            entry.phase,
                            crate::agent::harness::cartridges::fleet::SubmissionPhase::Running
                                | crate::agent::harness::cartridges::fleet::SubmissionPhase::Queued
                                | crate::agent::harness::cartridges::fleet::SubmissionPhase::Unknown
                        )
                    }) {
                        self.realm.competition(Competition::Pending(&format!(
                            "{} · {} · no score yet",
                            entry.benchmark, entry.status
                        )));
                    } else if let Some(entry) = snapshot.entries.iter().find(|entry| {
                        entry.phase
                            == crate::agent::harness::cartridges::fleet::SubmissionPhase::TimedOut
                    }) {
                        self.realm.competition(Competition::TimedOut(&format!(
                            "{} · {}",
                            entry.benchmark, entry.status
                        )));
                    } else if let Some(entry) = snapshot.entries.iter().find(|entry| {
                        matches!(
                            entry.phase,
                            crate::agent::harness::cartridges::fleet::SubmissionPhase::Accepted
                                | crate::agent::harness::cartridges::fleet::SubmissionPhase::Rejected
                        )
                    }) {
                        if entry.phase
                            == crate::agent::harness::cartridges::fleet::SubmissionPhase::Rejected
                        {
                            self.realm.competition(Competition::Error {
                                label: &entry.benchmark,
                                reason: &format!("{label} status {}", entry.status),
                            });
                        } else if let Some(score) = entry.score.as_deref() {
                            self.realm.competition(Competition::Reported {
                                label: &entry.benchmark,
                                score,
                            });
                        } else {
                            self.realm.competition(Competition::Complete(&format!(
                                "{} · {} · score not reported",
                                entry.benchmark, entry.status
                            )));
                        }
                    }
                    self.comp_fleet.apply(snapshot);
                }
                Err(error) => {
                    self.realm.competition(Competition::Error {
                        label: &format!("{label} status poll"),
                        reason: &error,
                    });
                    self.comp_fleet.fail(error);
                }
            }
        }

        let competition_loop = crate::ui::viz::loop_viz::visible(&self.loop_ctl)
            && self.submission_slot.phase != crate::agent::harness::SubmissionSlotPhase::Dormant;
        if !cfg!(test)
            && competition_loop
            && self.comp_fleet_rx.is_none()
            && let Some(cartridge) = cartridge.filter(|cartridge| cartridge.has_fleet())
            && self.comp_fleet_polled_at.elapsed() >= cartridge.fleet_interval()
        {
            self.comp_fleet.begin_scan();
            self.comp_fleet_rx = Some(crate::agent::harness::cartridges::fleet::spawn_poll(cartridge));
        }
    }

    fn fold_turn_cache(&mut self, thinking: &Thinking) {
        let Some(club) = thinking.club.as_ref() else {
            return;
        };
        let Some(before) = thinking.spawn_usage.get().copied() else {
            return;
        };
        let cache = club.cache_usage();
        let cache_read = cache
            .read_input_tokens
            .saturating_sub(before.cache_before.read_input_tokens);
        let reported =
            cache.read_accounting_responses > before.cache_before.read_accounting_responses;
        let usage = club.token_usage();
        let before_usage = before.usage_before.unwrap_or_default();
        let input = usage.map_or(0, |after| {
            after.total_input.saturating_sub(before_usage.total_input)
        });
        self.cache_meter.fold_turn(cache_read, input, reported);
        let output = usage.map_or(0, |after| {
            after.total_output.saturating_sub(before_usage.total_output)
        });
        self.note_ops_turn(
            cache_read,
            input.saturating_sub(cache_read) + output,
            thinking.started.elapsed().as_secs(),
        );
        {
            let calls = usage.map_or(1, |after| {
                after.turns.saturating_sub(before_usage.turns).max(1) as u32
            });
            self.note_horse_service(thinking, calls);
        }
        if self.loop_ctl.awaiting_turn
            && let Some(after) = usage.filter(|after| after.turns > before_usage.turns)
        {
            let output = after.total_output.saturating_sub(before_usage.total_output);
            self.loop_ctl.settle_turn_spend(input, cache_read, output);
        }
    }

    fn note_horse_service(&mut self, thinking: &Thinking, calls: u32) {
        self.realm.horse_service(
            &|| {
                (
                    route_of_thinking(thinking),
                    serving_of(
                        self.thinking.as_ref(),
                        &self.bag,
                        self.moa_one_shot.as_ref().or(self.moa_session.as_ref()),
                    ),
                )
            },
            calls,
        );
    }

    pub(crate) fn flush_tool_summary(&mut self) {
        let mut summary = self
            .tool_strip
            .take_summary()
            .unwrap_or_else(|| format!("{} 0 calls", Glyph::Tool.token()));
        if self.turn_truncated {
            summary.push_str(" · output incomplete");
        }
        if let Some(route) = self.turn_route_receipt.take() {
            summary.push_str(" · ");
            summary.push_str(&route);
        }
        self.messages.push(Message {
            role: Role::Activity,
            text: summary.into(),
        });
        self.cap_scrollback();
    }

    fn cap_scrollback(&mut self) {
        const SCROLLBACK_CAP: usize = 4000;
        const SCROLLBACK_TARGET: usize = 3000;
        if self.messages.len() <= SCROLLBACK_CAP {
            return;
        }
        self.pending_transcript_reflow = None;
        let drop = self.messages.len() - SCROLLBACK_TARGET;
        self.messages.drain(0..drop);
        self.flushed_partial_msg = self
            .flushed_partial_msg
            .and_then(|index| index.checked_sub(drop));
        self.retained_partial_msg = self
            .retained_partial_msg
            .and_then(|index| index.checked_sub(drop));
        let sp = drop.min(self.transcript_spawns.len());
        self.transcript_spawns.drain(0..sp);
        let hp = drop.min(self.transcript_heights.len());
        self.transcript_heights.drain(0..hp);
    }

    pub(crate) fn loop_verifier_notice(&mut self, note: &str, count: usize) {
        let row = if count > 1 {
            crate::ui::views::turn_event_view::notice_gauge_text(note, count)
        } else {
            crate::ui::views::turn_event_view::notice_text(note)
        };
        let index = (count > 1)
            .then(|| {
                self.messages.iter().rposition(|m| {
                    matches!(m.role, Role::Activity)
                        && crate::ui::views::turn_event_view::activity_notice_body(&m.text)
                            == Some(note)
                })
            })
            .flatten();
        if let Some(index) = index {
            self.messages[index] = Message::new(Role::Activity, row);
            self.refresh_transcript_row_height(index);
        } else {
            self.messages.push(Message::new(Role::Activity, row));
        }
    }

    fn bump_notice_gauge(&mut self, note: &str) -> bool {
        let Some(index) = notice_gauge_row_index(&self.messages, note) else {
            return false;
        };
        let count =
            crate::ui::views::turn_event_view::activity_gauge_count(&self.messages[index].text)
                .unwrap_or(1)
                .saturating_add(1);
        self.messages[index] = Message::new(
            Role::Activity,
            crate::ui::views::turn_event_view::notice_gauge_text(note, count),
        );
        self.refresh_transcript_row_height(index);
        true
    }

    fn refresh_transcript_row_height(&mut self, index: usize) {
        self.pending_transcript_reflow = None;
        if index >= self.transcript_heights.len() {
            return;
        }
        let width = usize::from(self.transcript_heights_w);
        if width == 0 {
            self.invalidate_transcript_layout();
            return;
        }
        let height = crate::ui::transcript::message_height(&self.messages[index], width);
        if self.transcript_heights[index] == height {
            return;
        }
        self.transcript_heights[index] = height;
        self.transcript_height_prefix.truncate(index + 1);
        for position in index..self.transcript_heights.len() {
            let total = self.transcript_height_prefix.last().copied().unwrap_or(0);
            self.transcript_height_prefix
                .push(total.saturating_add(u32::from(self.transcript_heights[position])));
        }
    }

    pub(crate) fn flush_partial(&mut self) {
        if !self.partial.is_empty() {
            let text = std::mem::take(&mut self.partial);
            self.stream_artifact_detector = Default::default();
            self.partial_height_cache.clear();
            self.stream_artifact_notice = false;
            if text.trim().is_empty() {
                return;
            }
            self.messages.push(Message::new(Role::Angel, text));
            self.flushed_partial_msg = Some(self.messages.len().saturating_sub(1));
            self.settle_transcript_spawns();
        }
    }

    fn reuse_flushed_partial(&mut self, reply: &str) -> bool {
        let Some(idx) = self.flushed_partial_msg.take() else {
            return false;
        };
        let Some(message) = self.messages.get_mut(idx) else {
            return false;
        };
        if !matches!(message.role, Role::Angel) {
            return false;
        }
        let draft = message.text.trim_end();
        if draft.is_empty() || !reply.trim_start().starts_with(draft) {
            return false;
        }
        message.text = Arc::from(reply);
        self.invalidate_transcript_layout();
        true
    }

    fn retain_interrupted_partial(&mut self) -> bool {
        let retained = !self.partial.trim().is_empty();
        self.flush_partial();
        if retained {
            self.retained_partial_msg = Some(self.messages.len().saturating_sub(1));
        }
        retained
    }

    fn collapse_superseded_partial(&mut self) {
        let Some(idx) = self.retained_partial_msg.take() else {
            return;
        };
        let Some(message) = self.messages.get_mut(idx) else {
            return;
        };
        if !matches!(message.role, Role::Angel) {
            return;
        }
        let stub: String = message.text.chars().take(80).collect();
        let ellipsis = if message.text.chars().count() > 80 {
            "…"
        } else {
            ""
        };
        message.text = format!("· superseded draft collapsed: “{stub}{ellipsis}”").into();
        self.invalidate_transcript_layout();
    }

    pub(crate) fn settle_transcript_spawns(&mut self) {
        self.transcript_spawns
            .resize(self.messages.len(), f32::NEG_INFINITY);
    }

    pub(crate) fn interrupt(&mut self) -> bool {
        if self.thinking.is_none()
            && let Some(pending) = self.pending_turn.take()
        {
            if matches!(self.messages.last(), Some(m) if matches!(m.role, Role::User)) {
                self.messages.pop();
                self.invalidate_transcript_layout();
            }
            if pending.launch.is_some() {
                if !self.input.is_empty() {
                    self.launch_typed_ahead = Some(std::mem::take(&mut self.input));
                }
                if let Some(input) = self.launch_input.as_mut() {
                    input.state = crate::app::launch::LaunchState::Cancelled;
                }
                self.launch_route = None;
                self.composer_selection_anchor = None;
            }
            self.input = pending.raw.to_string();
            self.cursor = self.input.chars().count();
            self.clipboard_paste
                .restore_images(&pending.user_msg.attachments, pending.clipboard_images);
            self.system_msg("turn canceled before send — draft restored".to_string());
            return true;
        }
        if self.thinking.is_none() && self.loop_pending.is_some() {
            self.loop_on_interrupt();
            self.loop_retire_pending();
            let _ = self.steer_queue.drain();
            return true;
        }
        if self.thinking.is_none() && self.loop_experiment.is_some() {
            self.loop_cancel_experiment();
            self.loop_on_interrupt();
            return true;
        }
        if self.thinking.is_none() {
            let Some(job) = self.bg_job.take() else {
                return false;
            };
            job.cancel();
            let output = job.live_output();
            self.last_background_operation = output.as_ref().map(|_| job.operation());
            self.last_background_output = output;
            let message = job.cancellation_message();
            if let Some(pending) = self.take_approval() {
                let _ = pending.reply.send(crate::agent::approval::Decision::Deny);
            }
            self.messages.push(Message {
                role: Role::System,
                text: format!("{} {message}", Glyph::Back.token()).into(),
            });
            self.scroll = 0;
            return true;
        }
        let Some(cancel) = self.thinking.as_ref().map(|t| Arc::clone(&t.cancel)) else {
            return false;
        };
        let already_draining = self.thinking.as_ref().is_some_and(Thinking::is_draining);
        if let Some(pending) = self.take_approval() {
            let _ = pending.reply.send(crate::agent::approval::Decision::Deny);
        }
        let dropped = self.steer_queue.drain().len();
        if dropped > 0 {
            self.messages.push(Message {
                role: Role::System,
                text: format!("{dropped} queued steer(s) dropped with the interrupt").into(),
            });
        }
        if already_draining {
            self.loop_on_interrupt();
            return true;
        }
        if cancel.swap(true, Ordering::Relaxed) {
            if let Some(thinking) = self.thinking.as_mut() {
                thinking.begin_draining();
            }
            self.finish_world_turn(false);
            self.restore_moa_after_turn();
            let retained_partial = self.retain_interrupted_partial();
            self.flush_tool_summary();
            self.messages.push(Message {
                role: Role::System,
                text: format!(
                    "{} hard-stopped — late output is suppressed; waiting for the provider/tool worker to drain before another turn{}",
                    Glyph::Back.token(),
                    if retained_partial {
                        RETAINED_PARTIAL_NOTE
                    } else {
                        ""
                    }
                ).into(),
            });
        } else {
            self.messages.push(Message {
                role: Role::System,
                text: format!(
                    "{} interrupting after the current step… (Esc again to hard-stop)",
                    Glyph::Back.token()
                )
                .into(),
            });
        }
        self.loop_on_interrupt();
        true
    }

    pub(crate) fn interrupt_idle_safe(&mut self) {
        if self.cancel_launch(true) {
            return;
        }
        self.interrupt();
    }

    pub(crate) fn still_active(&self) -> bool {
        matches!(self.scryglass.surface, crate::ui::scryglass::StageSurface::Still(index)
            if self.scryglass.active_media() == Some(index)
            && self.scryglass.active_error().is_none()
            && self.media.get(index).and_then(|m| m.source()).is_some_and(|source|
                self.viewer.still_matches(&source, self.scryglass.media_request_id())))
            && matches!(self.scryglass.controller.overlay(), Some(crate::ui::scryglass::StageOverlay::Media { index })
                if self.scryglass.active_media() == Some(*index))
    }

    pub(crate) fn inspect_still(&mut self, action: crate::ui::still_inspector::Action) -> bool {
        if !self.still_active()
            || self.viewer.inspector.source.is_none()
            || self.viewer.inspector.scene.is_none()
        {
            return false;
        }
        self.focus_module("artifacts");
        self.scryglass.pin_inspection();
        self.viewer.inspector.action(action);
        true
    }

    fn still_mouse(&mut self, ev: event::MouseEvent) -> bool {
        use event::{MouseButton, MouseEventKind};
        if matches!(ev.kind, MouseEventKind::Up(_)) {
            return self.viewer.inspector.drag.take().is_some();
        }
        if !self.still_active() {
            self.viewer.clear_still();
            return false;
        }
        let (x, y) = (ev.column, ev.row);
        if let MouseEventKind::Drag(MouseButton::Left) = ev.kind
            && let Some((lx, ly)) = self.viewer.inspector.drag
        {
            let p = self.viewer.inspector.pointer(x, y);
            let last = self.viewer.inspector.pointer(lx, ly);
            self.viewer.inspector.pan(p.0 - last.0, p.1 - last.1);
            self.viewer.inspector.drag = Some((x, y));
            self.scryglass.pin_inspection();
            return true;
        }
        if !self
            .viewer
            .inspector
            .viewport
            .is_some_and(|r| crate::ui::mouse::point_in(r, x, y))
        {
            return self
                .viewer
                .inspector
                .scene
                .is_some_and(|r| crate::ui::mouse::point_in(r, x, y))
                && matches!(
                    ev.kind,
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown | MouseEventKind::Down(_)
                );
        }
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.viewer.inspector.drag = Some((x, y));
                self.selection = None;
                self.copy_requested = false;
            }
            MouseEventKind::Down(MouseButton::Right) => self
                .viewer
                .inspector
                .action(crate::ui::still_inspector::Action::Fit),
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let point = self.viewer.inspector.pointer(x, y);
                self.viewer
                    .inspector
                    .zoom(matches!(ev.kind, MouseEventKind::ScrollUp), point);
            }
            _ => return false,
        }
        self.focus_module("artifacts");
        self.scryglass.pin_inspection();
        true
    }

    fn scryglass_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        if self
            .module_host
            .focused()
            .is_none_or(|id| id.as_str() != "artifacts")
        {
            return false;
        }
        let adventure_camera = self.scryglass.surface
            == crate::ui::scryglass::StageSurface::WorldMap
            && self.realm.live_adventure_view();
        let camera_active = adventure_camera
            || matches!(
                self.scryglass.controller.route(),
                crate::ui::scryglass::StageRoute::Explore(_)
            );
        if matches!(code, KeyCode::Esc) && self.flight_owner().is_agent_busy() {
            return false;
        }
        if self.input.is_empty() && modifiers == KeyModifiers::NONE {
            let shelf = self.scryglass.surface == crate::ui::scryglass::StageSurface::Assets;
            if shelf || self.scryglass.active_media().is_some() {
                let button = match code {
                    KeyCode::Char('a') => Some(WorldButton::Assets),
                    KeyCode::Char('t') => Some(WorldButton::Tower),
                    KeyCode::Char('p') => Some(WorldButton::AssetCopy(
                        crate::ui::scryglass::StageCopyTarget::Location,
                    )),
                    KeyCode::Char('o') => Some(WorldButton::AssetOpenExternal),
                    KeyCode::Char('c') => self.scryglass.selected_media(&self.media).map(|index| {
                        let card = &self.media[index];
                        WorldButton::AssetCopy(if shelf && card.is_video() {
                            crate::ui::scryglass::StageCopyTarget::Location
                        } else if card.is_visual() && card.source().is_some() {
                            crate::ui::scryglass::StageCopyTarget::Image
                        } else if card.source().is_some() {
                            crate::ui::scryglass::StageCopyTarget::Text
                        } else {
                            crate::ui::scryglass::StageCopyTarget::Location
                        })
                    }),
                    KeyCode::Char('[') => Some(WorldButton::ScryglassPrev),
                    KeyCode::Char(']') => Some(WorldButton::ScryglassNext),
                    _ => None,
                };
                if let Some(button) = button {
                    self.apply_world_button(button);
                    return true;
                }
            }
            if shelf {
                let delta = match code {
                    KeyCode::Up => Some(-1),
                    KeyCode::Down => Some(1),
                    KeyCode::PageUp => Some(-8),
                    KeyCode::PageDown => Some(8),
                    KeyCode::Home => {
                        Some(-(self.scryglass.selected_media(&self.media).unwrap_or(0) as isize))
                    }
                    KeyCode::End => Some(
                        self.media.len().saturating_sub(1) as isize
                            - self.scryglass.selected_media(&self.media).unwrap_or(0) as isize,
                    ),
                    _ => None,
                };
                if let Some(delta) = delta {
                    self.scryglass.move_asset_selection(delta, &self.media);
                    return true;
                }
                if code == KeyCode::Enter {
                    if let Some(index) = self.scryglass.selected_media(&self.media) {
                        self.apply_world_button(WorldButton::AssetSelect(index));
                    }
                    return true;
                }
                if code == KeyCode::Esc {
                    self.back_from_visual_surface();
                    return true;
                }
            }
        }
        if matches!(
            self.scryglass.surface,
            crate::ui::scryglass::StageSurface::Still(_)
        ) {
            if code == KeyCode::Esc && modifiers == KeyModifiers::NONE {
                self.back_from_visual_surface();
                return true;
            }
            if !self.input.is_empty()
                || (modifiers != KeyModifiers::NONE
                    && !(code == KeyCode::Char('+') && modifiers == KeyModifiers::SHIFT))
            {
                return false;
            }
            use crate::ui::still_inspector::Action;
            let action = match code {
                KeyCode::Char('+') | KeyCode::Char('=') => Action::ZoomIn,
                KeyCode::Char('-') => Action::ZoomOut,
                KeyCode::Char('0') => Action::Fit,
                KeyCode::Left => Action::Left,
                KeyCode::Right => Action::Right,
                KeyCode::Up => Action::Up,
                KeyCode::Down => Action::Down,
                _ => return false,
            };
            self.inspect_still(action);
            return true;
        }
        if matches!(code, KeyCode::Char(_))
            && (!self.input.is_empty() || modifiers != KeyModifiers::NONE)
        {
            return false;
        }
        if self.scryglass.surface == crate::ui::scryglass::StageSurface::Research {
            if self.research_key(code, modifiers) {
                return true;
            }
            if code == KeyCode::Esc {
                self.back_from_visual_surface();
                return true;
            }
            return false;
        }
        if self.scryglass.surface == crate::ui::scryglass::StageSurface::WorldMap
            && self.input.is_empty()
            && modifiers == KeyModifiers::NONE
            && code == KeyCode::Char('r')
        {
            self.open_research(None);
            return true;
        }
        if self.scryglass.surface == crate::ui::scryglass::StageSurface::Vault
            && self.atlas.enabled()
        {
            if matches!(code, KeyCode::Esc) {
                self.back_from_visual_surface();
                return true;
            }
            if self.input.is_empty() && modifiers == KeyModifiers::NONE {
                let items = self.atlas.list(
                    self.atlas_view.lane,
                    (!self.atlas_view.query.is_empty()).then_some(self.atlas_view.query.as_str()),
                );
                let selected = items.get(self.atlas_view.selected).cloned();
                match code {
                    KeyCode::Left => {
                        self.atlas_view.set_lane(self.atlas_view.lane.step(-1));
                    }
                    KeyCode::Right => {
                        self.atlas_view.set_lane(self.atlas_view.lane.step(1));
                    }
                    KeyCode::Up => self.atlas_view.select_delta(-1, items.len()),
                    KeyCode::Down => self.atlas_view.select_delta(1, items.len()),
                    KeyCode::Char('a') => {
                        if let Some(item) = selected {
                            let text = self
                                .atlas
                                .accept(&item.id)
                                .map(|accepted| format!("accepted {}", accepted.id))
                                .unwrap_or_else(|error| format!("atlas: {error}"));
                            self.system_msg(text);
                        }
                    }
                    KeyCode::Char('r') => {
                        if let Some(item) = selected {
                            self.reset_composer_history_recall();
                            self.composer_selection_anchor = None;
                            self.input = format!("/atlas revise {} {}", item.id, item.content);
                            self.cursor = self.input.chars().count();
                        }
                    }
                    KeyCode::Char('x') => {
                        if let Some(item) = selected {
                            let text = self
                                .atlas
                                .reject(&item.id, "rejected from Atlas review")
                                .map(|()| format!("rejected {} · tombstone recorded", item.id))
                                .unwrap_or_else(|error| format!("atlas: {error}"));
                            self.system_msg(text);
                        }
                    }
                    KeyCode::Char('d') => {
                        if let Some(item) = selected {
                            let text = self
                                .atlas
                                .defer(&item.id)
                                .map(|()| format!("deferred {} · remains unreviewed", item.id))
                                .unwrap_or_else(|error| format!("atlas: {error}"));
                            self.system_msg(text);
                        }
                    }
                    KeyCode::Char('p') => {
                        if let Some(item) = selected {
                            self.reset_composer_history_recall();
                            self.composer_selection_anchor = None;
                            self.input =
                                format!("/atlas promote {} {}", item.id, item.content_digest);
                            self.cursor = self.input.chars().count();
                        }
                    }
                    KeyCode::Char('c') => {
                        if let Some(item) = selected {
                            let text = self
                                .atlas
                                .challenge(&item.id, None)
                                .map(|()| {
                                    format!("challenged {} · excluded from task lens", item.id)
                                })
                                .unwrap_or_else(|error| format!("atlas: {error}"));
                            self.system_msg(text);
                        }
                    }
                    KeyCode::Char('u') => {
                        let text = self
                            .atlas
                            .undo()
                            .unwrap_or_else(|error| format!("atlas: {error}"));
                        self.system_msg(text);
                    }
                    _ => return false,
                }
                return true;
            }
            return false;
        }
        if self.scryglass.surface == crate::ui::scryglass::StageSurface::Observatory {
            if matches!(code, KeyCode::Esc) {
                self.back_from_visual_surface();
                return true;
            }
            if self.input.is_empty() && modifiers == KeyModifiers::NONE {
                match code {
                    KeyCode::Left => self.observatory.focus_campaigns(),
                    KeyCode::Right => self.observatory.focus_reports(),
                    KeyCode::Up => self.observatory.move_focused(-1),
                    KeyCode::Down => self.observatory.move_focused(1),
                    KeyCode::Enter => {
                        if self.observatory.focus()
                            == crate::app::observatory::ObservatoryFocus::Campaigns
                        {
                            self.observatory.focus_reports();
                        } else {
                            let text = self.open_selected_observatory_report();
                            self.system_msg(text);
                        }
                    }
                    _ => return false,
                }
                return true;
            }
            if !self.input.is_empty()
                && modifiers == KeyModifiers::NONE
                && matches!(code, KeyCode::Left | KeyCode::Right)
            {
                self.composer_selection_anchor = None;
                self.cursor = match code {
                    KeyCode::Left => previous_grapheme(&self.input, self.cursor),
                    KeyCode::Right => next_grapheme(&self.input, self.cursor),
                    _ => unreachable!(),
                };
                return true;
            }
            return false;
        }
        if self.scryglass.surface == crate::ui::scryglass::StageSurface::Reinforce {
            if matches!(code, KeyCode::Esc) {
                self.back_from_visual_surface();
                return true;
            }
            if self.input.is_empty() && modifiers == KeyModifiers::NONE {
                self.rl_view = match code {
                    KeyCode::Left | KeyCode::Char('h') => self.rl_view.step(-1),
                    KeyCode::Right | KeyCode::Char('l') => self.rl_view.step(1),
                    KeyCode::Char('1') | KeyCode::Char('b') => {
                        crate::ui::viz::rl_viz::RlView::Branch
                    }
                    KeyCode::Char('2') | KeyCode::Char('r') => {
                        crate::ui::viz::rl_viz::RlView::Research
                    }
                    KeyCode::Char('3') | KeyCode::Char('s') => {
                        crate::ui::viz::rl_viz::RlView::Sankey
                    }
                    _ => return false,
                };
                return true;
            }
            return false;
        }
        let video_active = self
            .scryglass
            .active_media()
            .and_then(|index| self.media.get(index))
            .is_some_and(Media::is_video);
        if self.input.is_empty() && modifiers == KeyModifiers::NONE {
            if matches!(
                self.scryglass.surface,
                crate::ui::scryglass::StageSurface::Document(_)
            ) {
                match code {
                    KeyCode::Up => {
                        self.scryglass.scroll_document(-1);
                        return true;
                    }
                    KeyCode::Down => {
                        self.scryglass.scroll_document(1);
                        return true;
                    }
                    KeyCode::PageUp => {
                        self.scryglass.scroll_document(-8);
                        return true;
                    }
                    KeyCode::PageDown => {
                        self.scryglass.scroll_document(8);
                        return true;
                    }
                    KeyCode::Home => {
                        self.scryglass.scroll_document(-65535);
                        return true;
                    }
                    KeyCode::End => {
                        self.scryglass.scroll_document(65535);
                        return true;
                    }
                    _ => {}
                }
            }
            if self.scryglass.surface == crate::ui::scryglass::StageSurface::Catalog {
                match code {
                    KeyCode::Esc => self.back_from_visual_surface(),
                    KeyCode::Up => self.scryglass.move_catalog_selection(-1),
                    KeyCode::Down => self.scryglass.move_catalog_selection(1),
                    KeyCode::PageUp => self.scryglass.move_catalog_selection(-5),
                    KeyCode::PageDown => self.scryglass.move_catalog_selection(5),
                    KeyCode::Home => self.scryglass.select_catalog_first(),
                    KeyCode::End => self.scryglass.select_catalog_last(),
                    KeyCode::Enter => self.study_selected_catalog(),
                    _ => return false,
                }
                return true;
            }
            if self.scryglass.surface == crate::ui::scryglass::StageSurface::Lesson {
                match code {
                    KeyCode::Esc => self.back_from_visual_surface(),
                    KeyCode::Up => self.scryglass.scroll_lesson_up(1),
                    KeyCode::Down => self.scryglass.scroll_lesson_down(1),
                    KeyCode::PageUp => self.scryglass.scroll_lesson_up(8),
                    KeyCode::PageDown => self.scryglass.scroll_lesson_down(8),
                    KeyCode::Home => self.scryglass.scroll_lesson_top(),
                    KeyCode::End => self.scryglass.scroll_lesson_bottom(),
                    KeyCode::Char('a') => self.draft_current_lesson_for_tutor(),
                    KeyCode::Char('y') => self.copy_current_lesson_source(),
                    _ => return false,
                }
                return true;
            }
            if matches!(
                self.scryglass.surface,
                crate::ui::scryglass::StageSurface::Arrival(_)
            ) && matches!(code, KeyCode::Enter)
            {
                self.enter_world_interior();
                return true;
            }
            if self.scryglass.surface == crate::ui::scryglass::StageSurface::WorldFirstPerson
                || adventure_camera
            {
                let act = self.realm.first_person_key(code);
                if act.open_assets {
                    self.apply_world_button(WorldButton::Assets);
                }
                if act.open_catalog {
                    self.scryglass.open_catalog();
                }
                if let Some(line) = act.say {
                    self.system_msg(line);
                }
                if act.enter_interior {
                    self.enter_world_interior();
                }
                if act.handled {
                    return true;
                }
            }
        }
        if self.input.is_empty()
            && self.scryglass.surface == crate::ui::scryglass::StageSurface::WorldMap
            && !adventure_camera
        {
            let act = self.realm.map_key(code);
            if act.toggle_camera {
                self.toggle_world_camera();
            }
            if act.handled {
                return true;
            }
        }
        match code {
            KeyCode::Esc => self.back_from_visual_surface(),
            KeyCode::Char('w') => {
                self.reset_world_yaw();
                self.scryglass.return_to_world();
            }
            KeyCode::Char('v') => {
                self.scryglass.back_overlay();
                self.scryglass
                    .navigate(crate::ui::scryglass::StageRoute::Vault);
            }
            KeyCode::Char('m') => self.toggle_world_camera(),
            KeyCode::Char('[') => {
                self.scryglass.browse(-1, &self.media);
            }
            KeyCode::Char(']') => {
                self.scryglass.browse(1, &self.media);
            }
            KeyCode::Char('p') => self.scryglass.pin_active(),
            KeyCode::Char(' ') if video_active => self.scryglass.toggle_video(),
            KeyCode::Char('r') if video_active => self.scryglass.restart_video(),
            KeyCode::Left if video_active => self.scryglass.seek_video(-5),
            KeyCode::Right if video_active => self.scryglass.seek_video(5),
            KeyCode::Char('0') | KeyCode::Char('r') if camera_active => self.scryglass.follow(),
            KeyCode::Left | KeyCode::Char('h') if camera_active => {
                self.scryglass.adjust_look(-0.17, 0.0)
            }
            KeyCode::Right | KeyCode::Char('l') if camera_active => {
                self.scryglass.adjust_look(0.17, 0.0)
            }
            KeyCode::Up | KeyCode::Char('k') if camera_active => {
                self.scryglass.adjust_look(0.0, -0.05)
            }
            KeyCode::Down | KeyCode::Char('j') if camera_active => {
                self.scryglass.adjust_look(0.0, 0.05)
            }
            KeyCode::Char('+') | KeyCode::Char('=') if camera_active => {
                self.scryglass.adjust_fov(-0.05)
            }
            KeyCode::Char('-') if camera_active => self.scryglass.adjust_fov(0.05),
            _ => return false,
        }
        true
    }

    fn back_from_visual_surface(&mut self) {
        self.viewer.clear_still();
        if self.scryglass.back_overlay() {
            self.lifecycle_ceremony = None;
            return;
        }
        if self.scryglass.controller.route() == crate::ui::scryglass::StageRoute::Assets {
            if !self.scryglass.controller.back() {
                self.scryglass.return_to_world();
            }
            return;
        }
        if let Some(left) = self.realm.leave_interior() {
            if left {
                self.reset_world_yaw();
            }
            return;
        }
        match self.scryglass.surface {
            crate::ui::scryglass::StageSurface::Moa => {
                self.moa_deck = None;
            }
            crate::ui::scryglass::StageSurface::Raytrace => {
                let _ = self
                    .module_host
                    .suspend(&crate::platform::runtime::ModuleId::new("graph"));
            }
            crate::ui::scryglass::StageSurface::Lifecycle => {
                self.lifecycle_ceremony = None;
            }
            crate::ui::scryglass::StageSurface::Observatory => {
                self.observatory.clear_viewport();
            }
            crate::ui::scryglass::StageSurface::Reinforce => {}
            crate::ui::scryglass::StageSurface::Research => {
                self.research.expanded = false;
            }
            crate::ui::scryglass::StageSurface::AgentGraph => {}
            crate::ui::scryglass::StageSurface::Quest => {}
            crate::ui::scryglass::StageSurface::Lesson => {}
            crate::ui::scryglass::StageSurface::Catalog => {}
            crate::ui::scryglass::StageSurface::Assets => {}
            crate::ui::scryglass::StageSurface::WorldMap => {}
            crate::ui::scryglass::StageSurface::Arrival(_)
            | crate::ui::scryglass::StageSurface::Still(_)
            | crate::ui::scryglass::StageSurface::Document(_)
            | crate::ui::scryglass::StageSurface::Video(_)
            | crate::ui::scryglass::StageSurface::Vault
            | crate::ui::scryglass::StageSurface::Fault => {}
            crate::ui::scryglass::StageSurface::Workshop
            | crate::ui::scryglass::StageSurface::WorldFirstPerson
            | crate::ui::scryglass::StageSurface::Hidden => {}
            crate::ui::scryglass::StageSurface::Loop => {}
        }
        if !self.scryglass.controller.back() {
            self.focus_module("core");
        }
    }

    pub(crate) fn on_key(&mut self, key: event::KeyEvent) {
        if self.top_overlay() == Overlay::Setup {
            self.setup_key(key);
            return;
        }
        if key.code == KeyCode::Esc {
            if self.cancel_launch(true) {
                return;
            }
            if self.thinking.is_none()
                && self.pending_turn.is_none()
                && self.bg_job.is_none()
                && let Some(other) = self.launch_typed_ahead.take()
            {
                self.launch_typed_ahead = Some(std::mem::replace(&mut self.input, other));
                self.cursor = self.input.chars().count();
                self.composer_selection_anchor = None;
                return;
            }
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        if !matches!(key.code, KeyCode::Tab) {
            self.skill_completion_catalog = None;
            self.workspace_completion_cache = None;
        }
        match self.top_overlay() {
            Overlay::Approval => {
                use crate::ui::views::approval_view::{KeyOutcome, key_outcome};
                match key_outcome(key.code, self.approval_choice()) {
                    KeyOutcome::Answer(d) => self.answer_approval(d),
                    KeyOutcome::Focus(i) => self.set_approval_choice(i),
                    KeyOutcome::Ignore => {}
                }
                return;
            }
            Overlay::BrowserPicker => {
                if self.browser_picker_key(key.code) {
                    return;
                }
            }
            Overlay::LoopDialog => {
                if self.loop_dialog_key(key.code) {
                    return;
                }
            }
            Overlay::AgentMenu => {
                if self.agent_menu_key(key.code) {
                    return;
                }
            }
            Overlay::MoaDeck => {
                if self.moa_deck_key(key.code, key.modifiers) {
                    return;
                }
            }
            Overlay::Setup | Overlay::None => {}
        }
        if self.dungeon_key(key) {
            return;
        }
        if key.code == KeyCode::Esc && self.cancel_tutor_question() {
            return;
        }
        if ctrl
            && key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('e') | KeyCode::Char('E'))
            && !self.shell_focused
        {
            self.explain_requested = true;
            return;
        }
        if ctrl && matches!(key.code, KeyCode::Char('g')) {
            self.toggle_shell();
            return;
        }
        if self.shell_focused {
            if let Some(sh) = self.shell.as_mut() {
                if !sh.is_alive() {
                    self.close_shell(Some("shell process exited"));
                } else if let Some(bytes) = pty::key_to_bytes(&key) {
                    sh.send(&bytes);
                    return;
                } else {
                    return;
                }
            } else {
                self.close_shell(None);
            }
        }
        if ctrl && matches!(key.code, KeyCode::Char('l')) {
            self.request_redraw("full terminal redraw queued (Ctrl-L)");
            return;
        }
        if self.handle_module_focus_key(key.code) {
            return;
        }
        self.normalize_composer_cursor();
        if self.walk_key(key) {
            return;
        }
        if self.vim_mode {
            if self.vim_normal {
                if self.vim_normal_key(key.code) {
                    return;
                }
            } else if matches!(key.code, KeyCode::Esc) {
                self.vim_normal = true;
                return;
            }
        }
        if let Some(action) = self.lookup_keybind(key.code, key.modifiers) {
            self.do_action(action);
            return;
        }
        if self.scryglass_key(key.code, key.modifiers) {
            return;
        }
        match key.code {
            KeyCode::Esc
                if self.thinking.is_none()
                    && self.bg_job.is_none()
                    && self.composer_selection_range().is_some() =>
            {
                self.composer_selection_anchor = None;
            }
            KeyCode::Esc if self.clipboard_paste.has_staged() && self.esc_is_free() => {
                let removed = self.clipboard_paste.staged_len();
                self.clipboard_paste.clear();
                let message = match removed {
                    1 => "staged screenshot removed — draft unchanged; Ctrl-V attaches another"
                        .to_string(),
                    count => format!(
                        "{count} staged screenshots removed — draft unchanged; Ctrl-V attaches \
                         another"
                    ),
                };
                self.system_msg(message);
            }
            KeyCode::Esc => self.interrupt_idle_safe(),
            KeyCode::Char('c') if ctrl && shift => {
                self.copy_composer_selection();
            }
            KeyCode::Char('c')
                if ctrl
                    && self.thinking.is_none()
                    && self.bg_job.is_none()
                    && self.composer_selection_range().is_some() =>
            {
                self.copy_composer_selection();
            }
            KeyCode::Char('c') if ctrl => self.interrupt_idle_safe(),
            KeyCode::Char('v') if ctrl && !shift => self.paste_clipboard_image(),
            KeyCode::Char('a') if ctrl && shift => {
                let end = self.input.chars().count();
                self.cursor = end;
                self.composer_selection_anchor = (end > 0).then_some(0);
            }
            KeyCode::Tab if self.complete_slash_command() => {}
            KeyCode::Tab
                if self.thinking.is_none()
                    && self.bg_job.is_none()
                    && self.pending_turn.is_none() =>
            {
                self.launch_interaction();
                self.bag.cycle();
                self.remember_brain_route();
            }
            KeyCode::Right if shift && ctrl => {
                self.extend_composer_selection(next_word(&self.input, self.cursor))
            }
            KeyCode::Left if shift && ctrl => {
                self.extend_composer_selection(prev_word(&self.input, self.cursor))
            }
            KeyCode::Right if shift => {
                let target = next_grapheme(&self.input, self.cursor);
                self.extend_composer_selection(target);
            }
            KeyCode::Left if shift => {
                self.extend_composer_selection(previous_grapheme(&self.input, self.cursor));
            }
            KeyCode::Home if shift => self.extend_composer_selection(0),
            KeyCode::End if shift => {
                self.extend_composer_selection(self.input.chars().count());
            }
            KeyCode::Up if shift && !ctrl && !self.input.is_empty() => {
                self.extend_composer_selection(self.vertical_composer_cursor(-1));
            }
            KeyCode::Down if shift && !ctrl && !self.input.is_empty() => {
                self.extend_composer_selection(self.vertical_composer_cursor(1));
            }
            KeyCode::Right if !ctrl && !self.input.is_empty() => {
                self.composer_selection_anchor = None;
                self.cursor = next_grapheme(&self.input, self.cursor);
            }
            KeyCode::Left if !ctrl && !self.input.is_empty() => {
                self.composer_selection_anchor = None;
                self.cursor = previous_grapheme(&self.input, self.cursor);
            }
            KeyCode::Right if !ctrl => self.scroll_focused_view_down(10),
            KeyCode::Left if !ctrl => self.scroll_focused_view_up(10),
            KeyCode::F(2) => {
                let _ = self
                    .module_host
                    .focus(&crate::platform::runtime::ModuleId::new("core"));
            }
            KeyCode::F(3) => self.focus_module("agent"),
            KeyCode::F(4) => self.focus_module("artifacts"),
            KeyCode::F(5) => self.toggle_module("graph"),
            KeyCode::F(9) => {
                self.open_agent_menu(crate::ui::agent_panel::controls::AgentMenuKind::Model)
            }
            KeyCode::F(10) => {
                self.open_agent_menu(crate::ui::agent_panel::controls::AgentMenuKind::Thinking)
            }
            KeyCode::PageUp => self.scroll_focused_view_up(10),
            KeyCode::PageDown => self.scroll_focused_view_down(10),
            KeyCode::Up if !ctrl && !self.input.is_empty() => {
                self.composer_selection_anchor = None;
                self.cursor = self.vertical_composer_cursor(-1);
            }
            KeyCode::Down if !ctrl && !self.input.is_empty() => {
                self.composer_selection_anchor = None;
                self.cursor = self.vertical_composer_cursor(1);
            }
            KeyCode::Up => self.scroll_focused_view_up(1),
            KeyCode::Down => self.scroll_focused_view_down(1),
            KeyCode::Home if !self.input.is_empty() => {
                self.composer_selection_anchor = None;
                self.cursor = 0;
            }
            KeyCode::End if !self.input.is_empty() => {
                self.composer_selection_anchor = None;
                self.cursor = self.input.chars().count();
            }
            KeyCode::Home => self.scroll_focused_view_top(),
            KeyCode::End => self.scroll_focused_view_bottom(),
            KeyCode::Enter
                if key
                    .modifiers
                    .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT) =>
            {
                self.input_insert('\n')
            }
            KeyCode::Char('a') if ctrl => {
                self.composer_selection_anchor = None;
                self.cursor = 0;
            }
            KeyCode::Char('e') if ctrl => {
                self.composer_selection_anchor = None;
                self.cursor = self.input.chars().count();
            }
            KeyCode::Char('w') if ctrl => self.input_delete_previous_word(),
            KeyCode::Char('u') if ctrl => self.input_delete_before_cursor(),
            KeyCode::Char('k') if ctrl => self.input_delete_after_cursor(),
            KeyCode::Char('y') if ctrl => self.input_yank(),
            KeyCode::Char('p') if ctrl => self.input_history_previous(),
            KeyCode::Char('n') if ctrl => self.input_history_next(),
            KeyCode::Char('r') if ctrl => self.input_history_search_previous(),
            KeyCode::Left if ctrl => {
                self.composer_selection_anchor = None;
                self.cursor = grapheme_floor(&self.input, prev_word(&self.input, self.cursor));
            }
            KeyCode::Right if ctrl => {
                self.composer_selection_anchor = None;
                self.cursor = grapheme_ceil(&self.input, next_word(&self.input, self.cursor));
            }
            KeyCode::Enter => self.submit(),
            KeyCode::Backspace if ctrl => self.input_delete_previous_word(),
            KeyCode::Backspace => self.input_backspace(),
            KeyCode::Char(c) if !ctrl => self.input_insert(c),
            _ => {}
        }
    }

    pub(crate) fn on_paste(&mut self, text: &str) {
        match self.top_overlay() {
            Overlay::None => {}
            Overlay::Setup => {
                self.setup_paste(text);
                return;
            }
            Overlay::Approval
            | Overlay::BrowserPicker
            | Overlay::LoopDialog
            | Overlay::AgentMenu
            | Overlay::MoaDeck => return,
        }
        if self.shell_focused {
            if let Some(sh) = self.shell.as_mut() {
                sh.send(text.as_bytes());
            }
            return;
        }
        if self.dungeon_view_active() {
            self.collapse_dungeon();
        }
        self.insert_pasted_text(text);
    }

    fn insert_pasted_text(&mut self, text: &str) {
        self.skill_completion_catalog = None;
        self.workspace_completion_cache = None;
        let (inserted, normalized) = self.input_insert_text(text);
        if inserted < normalized {
            self.system_msg(format!(
                "paste clipped · inserted {inserted}/{normalized} normalized bytes · composer \
                 paste cap {} KiB; clipboard unchanged — use /mention <file> for larger context",
                MAX_COMPOSER_PASTE_BYTES / 1024
            ));
        }
    }

    fn paste_clipboard_image(&mut self) {
        self.clipboard_paste.start();
    }

    fn esc_is_free(&self) -> bool {
        self.thinking.is_none()
            && self.bg_job.is_none()
            && self.pending_turn.is_none()
            && self.loop_pending.is_none()
            && self.loop_experiment.is_none()
    }

    fn complete_slash_command(&mut self) -> bool {
        if !self.input.starts_with('/') {
            return false;
        }
        if self.input.starts_with("/mention ") {
            return self.complete_workspace_path("/mention ", "mention");
        }
        if self.input.starts_with("/diagnostics ") {
            return self.complete_workspace_path("/diagnostics ", "diagnostics");
        }
        if self.input.starts_with("/symbols ") {
            return self.complete_workspace_path("/symbols ", "symbols");
        }
        if self.input.starts_with("/definition ") {
            let query = &self.input["/definition ".len()..];
            if query.chars().any(char::is_whitespace) {
                return true;
            }
            return self.complete_workspace_path("/definition ", "definition");
        }
        for (command, label) in [("/references ", "references"), ("/hover ", "hover")] {
            if self.input.starts_with(command) {
                let query = &self.input[command.len()..];
                if query.chars().any(char::is_whitespace) {
                    return true;
                }
                return self.complete_workspace_path(command, label);
            }
        }
        if self.input.starts_with("/skills ") {
            return self.complete_skill_selector();
        }
        if self.cursor != self.input.chars().count() || self.input.chars().any(char::is_whitespace)
        {
            return true;
        }
        let matches = input::slash_command_matches(&self.input);
        if matches.is_empty() {
            self.slash_completion_notice(format!(
                "slash completion · no command matches {}",
                self.input
            ));
            return true;
        }
        let common = input::slash_longest_common_prefix(&matches);
        if common.len() > self.input.len() {
            self.reset_composer_history_recall();
            self.composer_selection_anchor = None;
            self.input = common;
            self.cursor = self.input.chars().count();
            return true;
        }
        if matches.len() == 1 {
            return true;
        }
        const SHOWN: usize = 8;
        let mut listed = matches
            .iter()
            .take(SHOWN)
            .map(|name| format!("/{name}"))
            .collect::<Vec<_>>()
            .join(" · ");
        if matches.len() > SHOWN {
            listed.push_str(&format!(" · +{} more", matches.len() - SHOWN));
        }
        self.slash_completion_notice(format!("slash matches · {listed}"));
        true
    }

    fn complete_skill_selector(&mut self) -> bool {
        const MAX_SELECTOR_CHARS: usize = 512;
        const SHOWN: usize = 8;
        if self.cursor != self.input.chars().count() || self.input.contains('\n') {
            return true;
        }
        let selector = self.input["/skills ".len()..].to_string();
        if selector.chars().any(char::is_whitespace) {
            return true;
        }
        if selector.chars().count() > MAX_SELECTOR_CHARS {
            self.slash_completion_notice(format!(
                "skill completion · selector is too long (max {MAX_SELECTOR_CHARS} characters)"
            ));
            return true;
        }
        let (prefix, query) = selector
            .rsplit_once(',')
            .map_or(("", selector.as_str()), |(prefix, query)| (prefix, query));
        let already_selected = prefix
            .split(',')
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();
        if self.skill_completion_catalog.is_none() {
            self.skill_completion_catalog = Some(crate::knowledge::skills::list_for(
                self.tools.current_workspace(),
            ));
        }
        let mut matches = self
            .skill_completion_catalog
            .as_ref()
            .expect("skill completion catalog initialized")
            .iter()
            .filter(|name| name.starts_with(query) && !already_selected.contains(&name.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if prefix.is_empty() {
            matches.extend(
                ["check", "search"]
                    .into_iter()
                    .filter(|name| name.starts_with(query))
                    .map(str::to_string),
            );
            matches.sort();
            matches.dedup();
        }
        if matches.is_empty() {
            self.slash_completion_notice(format!(
                "skill completion · no admitted skill matches {query:?}"
            ));
            return true;
        }
        let common = path_longest_common_prefix_inner(&matches);
        if matches.len() == 1 || common.chars().count() > query.chars().count() {
            self.reset_composer_history_recall();
            self.composer_selection_anchor = None;
            self.input = if prefix.is_empty() {
                format!("/skills {common}")
            } else {
                format!("/skills {prefix},{common}")
            };
            self.cursor = self.input.chars().count();
            return true;
        }
        let mut listed = matches
            .iter()
            .take(SHOWN)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" · ");
        if matches.len() > SHOWN {
            listed.push_str(&format!(" · +{} more", matches.len() - SHOWN));
        }
        self.slash_completion_notice(format!("skill matches · {listed}"));
        true
    }

    fn complete_workspace_path(&mut self, command: &str, label: &str) -> bool {
        if self.cursor != self.input.chars().count() || self.input.contains('\n') {
            return true;
        }
        let query = self.input[command.len()..].trim_start().to_string();
        if query.chars().count() > 1_024 {
            self.slash_completion_notice(format!(
                "{label} completion · path prefix is too long (max 1024 characters)"
            ));
            return true;
        }
        let matches = match self.workspace_path_matches(&query) {
            Ok(matches) => matches,
            Err(()) => {
                self.slash_completion_notice(format!(
                    "{label} completion · path is unavailable inside the active workspace"
                ));
                return true;
            }
        };
        if matches.is_empty() {
            self.slash_completion_notice(format!(
                "{label} completion · no workspace path matches {query:?}"
            ));
            return true;
        }
        let common = path_longest_common_prefix_inner(&matches);
        if matches.len() == 1 || common.chars().count() > query.chars().count() {
            self.reset_composer_history_recall();
            self.composer_selection_anchor = None;
            self.input = format!("{command}{common}");
            self.cursor = self.input.chars().count();
            return true;
        }
        const SHOWN: usize = 8;
        let mut listed = matches
            .iter()
            .take(SHOWN)
            .cloned()
            .collect::<Vec<_>>()
            .join(" · ");
        if matches.len() > SHOWN {
            listed.push_str(&format!(" · +{} more", matches.len() - SHOWN));
        }
        self.slash_completion_notice(format!("{label} matches · {listed}"));
        true
    }

    fn workspace_path_matches(&mut self, query: &str) -> Result<Vec<String>, ()> {
        let (parent, _) = completion_parent_and_leaf(query)?;
        if let Some(cache) = self.workspace_completion_cache.as_ref()
            && cache.parent == parent
            && query.starts_with(&cache.query)
        {
            return Ok(cache
                .matches
                .iter()
                .filter(|candidate| candidate.starts_with(query))
                .cloned()
                .collect());
        }
        let matches = mention_path_matches_inner(self.tools.current_workspace(), query)?;
        self.workspace_completion_cache = Some(crate::app::WorkspaceCompletionCache {
            parent: parent.to_string(),
            query: query.to_string(),
            matches: matches.clone(),
        });
        Ok(matches)
    }

    fn slash_completion_notice(&mut self, text: String) {
        if self.messages.last().is_some_and(|message| {
            matches!(message.role, Role::System) && message.text.as_ref() == text
        }) {
            return;
        }
        self.system_msg(text);
    }

    pub(crate) fn approval_choice(&self) -> usize {
        let Some(pa) = &self.pending_approval else {
            return 0;
        };
        let serial = crate::ui::views::approval_view::serial(&pa.prompt, pa.scope_label.as_deref());
        if self.approval_focus.0 == serial { self.approval_focus.1 } else { 0 }
    }

    pub(crate) fn set_approval_choice(&mut self, choice: usize) {
        if let Some(pa) = &self.pending_approval {
            let serial = crate::ui::views::approval_view::serial(&pa.prompt, pa.scope_label.as_deref());
            self.approval_focus = (serial, choice.min(crate::ui::views::approval_view::ANSWERS.len() - 1));
        }
    }

    pub(crate) fn take_approval(&mut self) -> Option<PendingApproval> {
        self.approval_focus = (0, 0);
        self.approval_deadline = None;
        self.pending_approval.take()
    }

    fn tick_approval(&mut self) {
        let Some(pa) = &self.pending_approval else {
            self.approval_focus = (0, 0);
            self.approval_deadline = None;
            return;
        };
        let Some((serial, deadline)) = self.approval_deadline else {
            return;
        };
        if std::time::Instant::now() < deadline
            || serial != crate::ui::views::approval_view::serial(&pa.prompt, pa.scope_label.as_deref())
        {
            return;
        }
        if let Some(pa) = self.take_approval() {
            let _ = pa.reply.send(crate::agent::approval::Decision::Deny);
        }
        self.system_msg("approval request timed out · denied".to_string());
    }

    pub(crate) fn answer_approval(&mut self, d: crate::approval::Decision) {
        let Some(pa) = self.take_approval() else {
            return;
        };
        let _ = pa.reply.send(d);
        self.approval_hits.clear();
        if self.transcript_mode == crate::app::TranscriptMode::Trace {
            self.messages.push(Message {
                role: Role::Activity,
                text: format!(
                    "{} approval: {}",
                    Glyph::Approval.token(),
                    crate::ui::views::approval_view::decision_text(d)
                )
                .into(),
            });
        }
    }

    pub(crate) fn on_mouse(&mut self, ev: event::MouseEvent) {
        use angel_plug::stage::StageWorld;
        use crate::ui::mouse;
        use event::{MouseButton, MouseEventKind};

        let overlay = self.top_overlay();
        if matches!(
            ev.kind,
            MouseEventKind::Down(_)
                | MouseEventKind::Drag(_)
                | MouseEventKind::ScrollUp
                | MouseEventKind::ScrollDown
        ) {
            self.pressed_transcript_link = None;
        }
        if matches!(ev.kind, MouseEventKind::Up(MouseButton::Left)) {
            let pressed = self.pressed_transcript_link.take();
            if !overlay.takes_pointer()
                && self
                    .selection
                    .as_ref()
                    .is_none_or(|selection| selection.is_empty())
                && let Some(pressed) = pressed
                && pressed.point == (ev.column, ev.row)
                && self.transcript_links.iter().any(|hit| {
                    hit.target == pressed.target && mouse::point_in(hit.rect, ev.column, ev.row)
                })
            {
                self.selection = None;
                self.copy_requested = false;
                self.activate_transcript_link(&pressed.target);
                return;
            }
        }

        let world_drag_valid = self.scryglass_drag.is_some_and(|drag| {
            self.scryglass.visible
                && self.scryglass.renderable
                && self.scryglass.surface == drag.surface
                && self.panes.rect_of(crate::ui::mouse::PaneId::Artifacts) == Some(drag.rect)
                && self
                    .module_host
                    .focused()
                    .is_some_and(|id| id.as_str() == "artifacts")
                && crate::ui::mouse::point_in(drag.rect, ev.column, ev.row)
        });
        if !world_drag_valid
            || matches!(ev.kind, MouseEventKind::Up(_) | MouseEventKind::Down(_))
            || overlay.takes_pointer()
        {
            self.scryglass_drag = None;
        }
        if matches!(ev.kind, MouseEventKind::Up(_)) && self.viewer.inspector.drag.take().is_some() {
            return;
        }
        if overlay == Overlay::Approval {
            self.viewer.inspector.drag = None;
            let under = self
                .approval_hits
                .iter()
                .find(|(rect, _)| mouse::point_in(*rect, ev.column, ev.row))
                .map(|&(_, d)| d);
            match (ev.kind, under) {
                (MouseEventKind::Down(MouseButton::Left), Some(d)) => self.answer_approval(d),
                (MouseEventKind::Moved, Some(d)) => {
                    if let Some(i) = crate::ui::views::approval_view::ANSWERS.iter().position(|a| *a == d) {
                        self.set_approval_choice(i);
                    }
                }
                _ => {}
            }
            return;
        }
        let (x, y) = (ev.column, ev.row);
        if overlay == Overlay::Setup {
            self.viewer.inspector.drag = None;
            self.setup_mouse(ev);
            return;
        }
        if overlay == Overlay::BrowserPicker {
            self.viewer.inspector.drag = None;
            if matches!(ev.kind, MouseEventKind::Down(MouseButton::Left))
                && let Some(&(_, WorldButton::BrowserPick(index))) = self
                    .world_buttons
                    .iter()
                    .find(|(rect, button)| {
                        matches!(button, WorldButton::BrowserPick(_)) && mouse::point_in(*rect, x, y)
                    })
            {
                self.commit_browser_pick(index);
            }
            return;
        }
        if overlay == Overlay::LoopDialog {
            self.viewer.inspector.drag = None;
            if matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
                self.loop_dialog_click(x, y);
            }
            return;
        }

        if self.dungeon_view_active() && !self.dungeon_controls_blocked() {
            return;
        }

        if overlay == Overlay::AgentMenu {
            self.viewer.inspector.drag = None;
            match ev.kind {
                MouseEventKind::ScrollUp => self.move_agent_menu(-1),
                MouseEventKind::ScrollDown => self.move_agent_menu(1),
                MouseEventKind::Down(MouseButton::Left) => {
                    let action = self
                        .agent_menu_hits
                        .iter()
                        .find(|(rect, _)| mouse::point_in(*rect, x, y))
                        .map(|(_, action)| action.clone());
                    if let Some(action) = action {
                        self.apply_agent_menu_action(action);
                    } else {
                        self.agent_menu = None;
                        self.agent_menu_search = None;
                    }
                }
                _ => {}
            }
            return;
        }

        if matches!(ev.kind, MouseEventKind::Down(MouseButton::Left))
            && let Some(button) = self.object_dock.hit(x, y)
        {
            self.selection = None;
            self.copy_requested = false;
            self.apply_world_button(button);
            return;
        }

        if self.shell_focused
            && let (Some(shell), Some(rect)) = (self.shell.as_ref(), self.shell_area)
            && mouse::point_in(rect, x, y)
        {
            let (mode, enc) = shell.mouse_mode();
            let selection_override =
                mouse::selection_overrides_pty_mouse(&ev, self.selection.as_ref());
            if mode != mouse::TrackMode::None && !selection_override {
                if matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
                    self.selection = None;
                    self.copy_requested = false;
                }
                if let Some(bytes) = mouse::pty_mouse_bytes(&ev, mode, enc, rect)
                    && let Some(sh) = self.shell.as_mut()
                {
                    sh.send(&bytes);
                }
                return;
            }
        }

        if matches!(ev.kind, MouseEventKind::Down(MouseButton::Right))
            && !ev
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            && matches!(
                self.scryglass.controller.overlay(),
                Some(crate::ui::scryglass::StageOverlay::Media { .. })
            )
            && self
                .panes
                .rect_of(mouse::PaneId::Artifacts)
                .is_some_and(|rect| mouse::point_in(rect, x, y))
        {
            let target = if ev.modifiers.contains(KeyModifiers::SHIFT) {
                "stage text"
            } else {
                "stage"
            };
            let receipt = self.copy_response(Some(target));
            self.system_msg(receipt);
            return;
        }

        if self.still_mouse(ev) {
            return;
        }
        match ev.kind {
            MouseEventKind::ScrollUp => {
                if matches!(
                    self.scryglass.surface,
                    crate::ui::scryglass::StageSurface::Document(_)
                ) && self
                    .panes
                    .rect_of(mouse::PaneId::Artifacts)
                    .is_some_and(|rect| mouse::point_in(rect, x, y))
                {
                    self.scryglass.scroll_document(-3);
                    return;
                }
                if self.scryglass.surface == crate::ui::scryglass::StageSurface::Research
                    && self
                        .panes
                        .rect_of(mouse::PaneId::Artifacts)
                        .is_some_and(|rect| mouse::point_in(rect, x, y))
                {
                    self.research.move_selection(-3);
                    return;
                }
                if let Some(&(_, btn)) = self
                    .world_buttons
                    .iter()
                    .find(|(r, _)| mouse::point_in(*r, x, y))
                {
                    self.apply_world_button(btn);
                    return;
                }
                if self.scryglass_camera_hit(x, y) {
                    self.zoom_world_at(x, y, true);
                    return;
                }
                if self.scryglass_map_hit(x, y) {
                    return;
                }
                self.scroll_pane_at(x, y, 3, true);
            }
            MouseEventKind::ScrollDown => {
                if matches!(
                    self.scryglass.surface,
                    crate::ui::scryglass::StageSurface::Document(_)
                ) && self
                    .panes
                    .rect_of(mouse::PaneId::Artifacts)
                    .is_some_and(|rect| mouse::point_in(rect, x, y))
                {
                    self.scryglass.scroll_document(3);
                    return;
                }
                if self.scryglass.surface == crate::ui::scryglass::StageSurface::Research
                    && self
                        .panes
                        .rect_of(mouse::PaneId::Artifacts)
                        .is_some_and(|rect| mouse::point_in(rect, x, y))
                {
                    self.research.move_selection(3);
                    return;
                }
                if self.scryglass_camera_hit(x, y) {
                    self.zoom_world_at(x, y, false);
                    return;
                }
                if self.scryglass_map_hit(x, y) {
                    return;
                }
                self.scroll_pane_at(x, y, 3, false);
            }
            MouseEventKind::Down(MouseButton::Right) if self.scryglass_camera_hit(x, y) => {
                self.focus_pane_module(mouse::PaneId::Artifacts);
                self.scryglass.follow();
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(&(_, btn)) = self
                    .agent_buttons
                    .iter()
                    .find(|(r, _)| mouse::point_in(*r, x, y))
                {
                    match btn {
                        AgentButton::Model if !self.flight_owner().is_agent_busy() => {
                            self.open_agent_menu(
                                crate::ui::agent_panel::controls::AgentMenuKind::Model,
                            )
                        }
                        AgentButton::ReasoningEffort
                            if !self.flight_owner().is_agent_busy() =>
                        {
                            self.open_agent_menu(
                                crate::ui::agent_panel::controls::AgentMenuKind::Thinking,
                            );
                        }
                        AgentButton::MoaDeck => self.open_moa_deck(None),
                        AgentButton::RateUseful => {
                            let message = self.rate_last_turn(Some("useful"));
                            self.system_msg(message);
                        }
                        AgentButton::RateMiss => {
                            let message = self.rate_last_turn(Some("miss"));
                            self.system_msg(message);
                        }
                        AgentButton::Model | AgentButton::ReasoningEffort => {}
                    }
                    return;
                }
                if self
                    .tool_herald_area
                    .is_some_and(|rect| mouse::point_in(rect, x, y))
                {
                    self.tool_strip.toggle_ledger();
                    return;
                }
                if let Some(&(_, button)) = self.world_buttons.iter()
                    .find(|(rect, _)| mouse::point_in(*rect, x, y))
                {
                    self.apply_world_button(button);
                    return;
                }
                if self.walk_click(x, y) {
                    return;
                }
                if self.gate_click(x, y) {
                    return;
                }
                if self.scryglass_camera_hit(x, y) {
                    self.focus_module("artifacts");
                    self.scryglass.follow_agent = false;
                    if self.realm.has_world()
                        && let Some(rect) = self.panes.rect_of(mouse::PaneId::Artifacts)
                    {
                        self.scryglass_drag = Some(crate::stage::WorldDrag {
                            last: (x, y),
                            rect,
                            surface: self.scryglass.surface,
                        });
                    }
                    self.selection = None;
                    self.copy_requested = false;
                    return;
                }
                let hit = self.panes.pane_at(x, y);
                if let Some(pane) = self.interaction_pane_at(x, y) {
                    self.focus_pane_module(pane);
                }
                self.selection = hit.and_then(|(pane, rect)| {
                    crate::ui::surfaces::pane_accepts_clipboard(pane)
                        .then(|| mouse::Selection::new(pane, rect, x, y))
                });
                self.copy_requested = false;
                if hit.is_some_and(|(pane, _)| pane == mouse::PaneId::Transcript)
                    && let Some(link) = self
                        .transcript_links
                        .iter()
                        .find(|link| mouse::point_in(link.rect, x, y))
                {
                    self.pressed_transcript_link = Some(crate::ui::transcript_links::PressedLink {
                        point: (x, y),
                        target: std::sync::Arc::clone(&link.target),
                    });
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(mut drag) = self.scryglass_drag {
                    let dx = i32::from(x) - i32::from(drag.last.0);
                    let dy = i32::from(y) - i32::from(drag.last.1);
                    drag.last = (x, y);
                    self.scryglass_drag = Some(drag);
                    self.scryglass
                        .adjust_look(dx as f32 * 0.055, dy as f32 * 0.025);
                    return;
                }
                let mut drag_scroll: Option<(mouse::PaneId, bool)> = None;
                if let Some(sel) = self.selection.as_mut() {
                    let rebound = crate::ui::surfaces::pane_accepts_clipboard(sel.pane)
                        && self
                            .panes
                            .rect_of(sel.pane)
                            .is_some_and(|live| sel.rebind(live));
                    if !rebound {
                        self.selection = None;
                        self.copy_requested = false;
                    }
                }
                if let Some(sel) = self.selection.as_mut() {
                    let pane = sel.pane;
                    let rect = sel.rect;
                    if crate::ui::surfaces::pane_accepts_clipboard(pane) {
                        if y < rect.y {
                            drag_scroll = Some((pane, true));
                        } else if y >= rect.y.saturating_add(rect.height) {
                            drag_scroll = Some((pane, false));
                        }
                    }
                    sel.extend(x, y);
                }
                if let Some((pane, up)) = drag_scroll {
                    if up {
                        self.scroll_pane_up(pane, 3);
                    } else {
                        self.scroll_pane_down(pane, 3);
                    }
                }
            }
            MouseEventKind::Up(MouseButton::Left)
                if self.selection.as_ref().is_some_and(|s| !s.is_empty()) =>
            {
                if let Some(selection) = self.selection.as_mut() {
                    selection.finish();
                }
                self.copy_requested = true;
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.scryglass_drag = None;
                if let Some(selection) = self.selection.as_mut() {
                    selection.finish();
                }
                if self.selection.as_ref().is_some_and(|s| s.is_empty()) {
                    self.selection = None;
                }
            }
            _ => {}
        }
    }

    fn scryglass_camera_hit(&self, x: u16, y: u16) -> bool {
        matches!(
            self.scryglass.surface,
            crate::ui::scryglass::StageSurface::WorldFirstPerson
                | crate::ui::scryglass::StageSurface::WorldMap
        ) && self.interaction_pane_at(x, y) == Some(crate::ui::mouse::PaneId::Artifacts)
    }

    fn zoom_world_at(&mut self, _x: u16, _y: u16, inward: bool) {
        self.focus_pane_module(crate::ui::mouse::PaneId::Artifacts);
        self.scryglass.adjust_fov(if inward { -0.05 } else { 0.05 });
    }

    fn scryglass_map_hit(&self, x: u16, y: u16) -> bool {
        let adventure = self.realm.live_adventure_view();
        self.scryglass.surface == crate::ui::scryglass::StageSurface::WorldMap
            && !adventure
            && self
                .panes
                .rect_of(crate::ui::mouse::PaneId::Artifacts)
                .is_some_and(|rect| crate::ui::mouse::point_in(rect, x, y))
    }

    fn toggle_world_camera(&mut self) {
        use angel_plug::stage::StageRules;
        let effect = self.realm.press_toggle_camera();
        self.apply_stage_effect(effect);
    }

    fn enter_world_interior(&mut self) {
        use angel_plug::stage::StageRules;
        let effect = self.realm.press_enter();
        self.apply_stage_effect(effect);
    }

    fn apply_stage_effect(&mut self, effect: angel_plug::stage::StageEffect) {
        use angel_plug::stage::StagePlaces;
        type Places = crate::app::control::realm::GameHost;
        if effect.reset_yaw {
            self.reset_world_yaw();
        }
        if effect.stop_walk {
            angel_live::realm::RealmHost::stop_walk(&mut self.realm);
        }
        if effect.return_to_world {
            self.scryglass.return_to_world();
        }
        if effect.follow {
            self.scryglass.follow();
        }
        if effect.back_from_arrival
            && matches!(
                self.scryglass.controller.overlay(),
                Some(crate::ui::scryglass::StageOverlay::Arrival { .. })
            )
        {
            self.scryglass.back_overlay();
        }
        if effect.open_catalog {
            self.scryglass.open_catalog();
        }
        if let Some(place) = effect.explore.and_then(Places::place_of) {
            self.scryglass
                .navigate(crate::ui::scryglass::StageRoute::Explore(place));
        }
        if let Some(place) = effect.toggle_route.and_then(Places::place_of) {
            self.scryglass.toggle_world_route(place);
        }
    }

    fn study_selected_catalog(&mut self) {
        use angel_plug::stage::StageRules;
        let shelf = self.scryglass.selected_shelf();
        let topic = shelf
            .arc
            .first()
            .copied()
            .unwrap_or(shelf.title)
            .to_string();
        if self.scryglass.begin_selected_lesson() {
            self.realm.teach(&topic);
            self.focus_module("artifacts");
        } else {
            self.system_msg("Librarium is behind another active Scryglass surface.".to_string());
        }
    }

    pub(crate) fn tutor_context(&self, question: &str) -> (String, String) {
        if let Some(lesson) = self.scryglass.lesson() {
            return (lesson.tutor_name().to_string(), lesson.ask_prompt());
        }
        let lesson = if question.trim().is_empty() {
            crate::knowledge::library::local_lesson_for_shelf(self.scryglass.selected_shelf().id)
        } else {
            crate::knowledge::library::local_lesson(question).or_else(|| {
                crate::knowledge::library::local_lesson_for_shelf(
                    crate::knowledge::library::primary_shelf(crate::knowledge::library::classify(
                        question,
                    ))
                    .id,
                )
            })
        }
        .or_else(|| {
            crate::knowledge::library::local_lesson_for_shelf(self.scryglass.selected_shelf().id)
        });
        lesson
            .map(|lesson| (lesson.tutor.name.to_string(), lesson.ask_tutor_prompt()))
            .unwrap_or_else(|| {
                (
                    "Tutor".into(),
                    crate::agent::harness::book::ing_drivers::TUTOR_FALLBACK.cells(),
                )
            })
    }

    pub(crate) fn draft_current_lesson_for_tutor(&mut self) {
        if self.tutor_draft.is_some() {
            self.focus_module("core");
            return;
        }
        let (name, context) = self.tutor_context("");
        self.tutor_draft = Some(crate::app::TutorDraft {
            name,
            context,
            saved_input: std::mem::take(&mut self.input),
            saved_cursor: self.cursor,
            saved_selection: self.composer_selection_anchor.take(),
            saved_focus: self.module_host.focused().map(|id| id.as_str().to_string()),
        });
        self.cursor = 0;
        self.reset_composer_history_recall();
        self.focus_module("core");
    }

    pub(crate) fn restore_tutor_draft(&mut self, draft: crate::app::TutorDraft) {
        self.input = draft.saved_input;
        self.cursor = draft.saved_cursor.min(self.input.chars().count());
        self.composer_selection_anchor = draft.saved_selection;
        if let Some(focus) = draft.saved_focus {
            self.focus_module(&focus);
        }
    }

    pub(crate) fn cancel_tutor_question(&mut self) -> bool {
        let Some(draft) = self.tutor_draft.take() else {
            return false;
        };
        self.restore_tutor_draft(draft);
        true
    }

    fn copy_current_lesson_source(&mut self) {
        let Some(source) = self
            .scryglass
            .lesson()
            .map(crate::ui::term::lookup::QuickLookup::source_url)
            .map(str::to_string)
        else {
            self.system_msg("Open a lesson before copying its source.".to_string());
            return;
        };
        self.pending_clipboard = Some(source);
    }

    pub(crate) fn apply_world_button(&mut self, btn: WorldButton) {
        use angel_plug::stage::StageRules;
        match btn {
            WorldButton::DockToggle => {
                self.object_dock.tiles.clear();
                self.object_dock
                    .toggle(std::time::Instant::now(), self.visual_motion);
                self.world_buttons
                    .retain(|(_, button)| dock_control(button));
            }
            WorldButton::DockNext => {
                self.object_dock.tiles.clear();
                self.object_dock.page = self.object_dock.page.saturating_add(1);
                self.world_buttons
                    .retain(|(_, button)| dock_control(button));
            }
            WorldButton::DockPrevious => {
                self.object_dock.tiles.clear();
                self.object_dock.page = self.object_dock.page.saturating_sub(1);
                self.world_buttons
                    .retain(|(_, button)| dock_control(button));
            }
            WorldButton::Assets => {
                self.scryglass.open_assets();
                self.focus_module("artifacts");
            }
            WorldButton::Tower => {
                while self.scryglass.back_overlay() {}
                let effect = self.realm.press_tower();
                self.apply_stage_effect(effect);
                self.focus_module("artifacts");
            }
            WorldButton::AssetSelect(index) => {
                if index < self.media.len() {
                    self.scryglass.reveal_media(index, true);
                    self.focus_module("artifacts");
                }
            }
            WorldButton::AssetCopy(target) => {
                let command = match target {
                    crate::ui::scryglass::StageCopyTarget::Location => "stage",
                    crate::ui::scryglass::StageCopyTarget::Text => "stage text",
                    crate::ui::scryglass::StageCopyTarget::Image => "stage image",
                };
                let receipt = self.copy_response(Some(command));
                self.system_msg(receipt);
            }
            WorldButton::AssetOpenExternal => {
                if let Some(index) = self.scryglass.selected_media(&self.media) {
                    if crate::ui::showcase::open(&self.media[index]) {
                        return;
                    }
                    let receipt =
                        crate::app::local_command::open_target_command(&self.media[index].target());
                    self.system_msg(receipt);
                }
            }
            WorldButton::Research(action) => self.research_action(action),
            WorldButton::FormationDeck => self.open_moa_deck(None),
            WorldButton::SelectFormation(id) => self.select_or_arm_moa_card(id),
            WorldButton::SelectFormationSlot(index) => self.select_moa_slot(index),
            WorldButton::AssignFormationModel(index) => self.assign_moa_model(index),
            WorldButton::StageFormationSeatThink(index) => {
                if self
                    .moa_deck
                    .as_mut()
                    .is_some_and(|deck| deck.select_slot(index))
                {
                    self.open_moa_effort_picker();
                }
            }
            WorldButton::StageFormationEffort(index) => self.stage_moa_effort(index),
            WorldButton::BrowserPick(index) => self.commit_browser_pick(index),
            WorldButton::ArmFormationTurn => self.play_selected_moa_card(false),
            WorldButton::ArmFormationSession => self.play_selected_moa_card(true),
            WorldButton::ClearFormation => self.clear_moa_cards(),
            WorldButton::RlView(view) => {
                self.rl_view = view;
                self.focus_module("artifacts");
            }
            WorldButton::ScryglassWorld => {
                self.scryglass.return_to_world();
            }
            WorldButton::ScryglassLibrary => {
                self.scryglass.back_overlay();
                let effect = self.realm.press_library();
                self.apply_stage_effect(effect);
                self.scryglass.open_catalog();
                self.focus_module("artifacts");
            }
            WorldButton::ScryglassVault => {
                self.scryglass.back_overlay();
                self.scryglass
                    .navigate(crate::ui::scryglass::StageRoute::Vault);
            }
            WorldButton::ScryglassPrev => {
                self.scryglass.browse(-1, &self.media);
            }
            WorldButton::ScryglassNext => {
                self.scryglass.browse(1, &self.media);
            }
            WorldButton::ScryglassPin => self.scryglass.pin_active(),
            WorldButton::Still(action) => {
                self.inspect_still(action);
            }
            WorldButton::ScryglassMap => self.toggle_world_camera(),
            WorldButton::ScryglassEnter => {
                self.enter_world_interior();
            }
            WorldButton::ScryglassLeave => {
                let effect = self.realm.press_leave();
                self.apply_stage_effect(effect);
            }
            WorldButton::ScryglassCatalog => {
                let effect = self.realm.press_catalog();
                self.apply_stage_effect(effect);
            }
            WorldButton::ScryglassStudy => self.study_selected_catalog(),
            WorldButton::ScryglassAskTutor => self.draft_current_lesson_for_tutor(),
            WorldButton::ScryglassCopySource => self.copy_current_lesson_source(),
            WorldButton::ScryglassFollow => {
                let realm_route =
                    self.scryglass.controller.route() == crate::ui::scryglass::StageRoute::Realm;
                let effect = self.realm.press_follow(realm_route);
                self.apply_stage_effect(effect);
            }
            WorldButton::ScryglassLandmark(id) => {
                let effect = self.realm.press_landmark(id);
                self.apply_stage_effect(effect);
            }
            WorldButton::ScryglassVideoToggle => self.scryglass.toggle_video(),
            WorldButton::ScryglassVideoBack => self.scryglass.seek_video(-5),
            WorldButton::ScryglassVideoForward => self.scryglass.seek_video(5),
            WorldButton::WalkIn => {
                self.with_realm(|realm, cx| angel_live::realm::RealmHost::go_in(realm, cx))
            }
            WorldButton::WalkLeave => angel_live::realm::RealmHost::stop_walk(&mut self.realm),
            WorldButton::Back => self.back_from_visual_surface(),
        }
    }

    pub(crate) fn flush_clipboard(&mut self) {
        if let Some(payload) = self.pending_stage_copy.take() {
            self.clipboard_image_copy = None;
            if let Some(source) = payload.image {
                match crate::ui::clipboard::ClipboardImageCopy::start(source) {
                    Ok(copy) => self.clipboard_image_copy = Some(copy),
                    Err(error) => {
                        let receipt = deliver_to_clipboard(&payload.text, payload.fallback_file);
                        self.system_msg(format!("{error} · {} · {receipt}", payload.description));
                    }
                }
            } else {
                let receipt = deliver_to_clipboard(&payload.text, payload.fallback_file);
                self.system_msg(format!("{} · {receipt}", payload.description));
            }
        }
        if let Some(result) = self
            .clipboard_image_copy
            .as_mut()
            .and_then(|copy| copy.poll())
        {
            self.clipboard_image_copy = None;
            self.system_msg(match result {
                Ok(receipt) => receipt,
                Err(error) => format!("Image copy failed: {error}"),
            });
        }
        if let Some(text) = self.pending_clipboard.take()
            && !text.is_empty()
        {
            self.clipboard_image_copy = None;
            let status = deliver_to_clipboard(&text, "last-selection.txt");
            self.system_msg(status);
        }
        if let Some(selection) = self.pending_tutor_selection.take() {
            if self.tutor_draft.is_none() {
                let (name, context) = self.tutor_context(&selection);
                self.draft_current_lesson_for_tutor();
                if let Some(draft) = self.tutor_draft.as_mut() {
                    draft.name = name;
                    draft.context = context;
                }
                self.input = format!("Explain this selection:\n\n{selection}");
                self.cursor = self.input.chars().count();
            } else {
                self.system_msg("Finish or cancel the current tutor question first.".to_string());
            }
        }
        let Some(term) = self.pending_quick_lookup.take() else {
            return;
        };
        let lookup_available = !self.scryglass.lesson_loading();
        if lookup_available {
            let world_term = term.clone();
            if self.scryglass.begin_lesson(term) {
                if crate::ui::surfaces::BackdropMode::from_env().paints_in_process() {
                    self.realm.feed(Feed::Lesson(&world_term));
                }
                self.focus_module("artifacts");
            } else {
                self.system_msg(
                    "Could not open the selected lesson because another Scryglass surface is active; close it and retry."
                        .to_string(),
                );
            }
        }
    }

    pub(crate) fn finish_world_turn(&mut self, ok: bool) {
        self.research.finish_turn();
        self.note_ops_end(ok);
        self.realm.feed(Feed::TurnEnded {
            ok,
            painted: crate::ui::surfaces::BackdropMode::from_env().paints_in_process()
                && crate::drive::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible),
        });
    }

    fn vertical_composer_cursor(&self, delta: isize) -> usize {
        let width = self
            .panel_frames
            .get(crate::ui::panels::PanelKind::Input)
            .map(|area| area.width.saturating_sub(2) as usize)
            .unwrap_or(80);
        crate::ui::views::status_view::composer_vertical_cursor(
            &self.input,
            width,
            self.cursor,
            delta,
        )
    }

    fn normalize_composer_cursor(&mut self) {
        if let Some((start, end)) = self.composer_selection_range() {
            if self
                .composer_selection_anchor
                .is_some_and(|anchor| anchor < self.cursor)
            {
                self.composer_selection_anchor = Some(start);
                self.cursor = end;
            } else {
                self.composer_selection_anchor = Some(end);
                self.cursor = start;
            }
        } else {
            if self.cursor != self.input.len() {
                self.cursor = grapheme_floor(&self.input, self.cursor);
            }
            self.composer_selection_anchor = None;
        }
    }

    fn input_insert(&mut self, c: char) {
        self.startup_intro
            .dismiss(std::time::Instant::now(), self.visual_motion);
        self.normalize_composer_cursor();
        self.delete_composer_selection();
        self.reset_composer_history_recall();
        if self.cursor == self.input.len() {
            self.input.push(c);
            self.cursor += 1;
            return;
        }
        let n = self.input.chars().count();
        self.cursor = self.cursor.min(n);
        let at = byte_of(&self.input, self.cursor);
        self.input.insert(at, c);
        self.cursor = grapheme_ceil(&self.input, self.cursor + 1);
    }

    fn input_insert_text(&mut self, text: &str) -> (usize, usize) {
        self.normalize_composer_cursor();
        if text.is_empty() {
            return (0, 0);
        }
        let normalized;
        let text = if text.contains('\r') {
            normalized = text.replace("\r\n", "\n").replace('\r', "\n");
            normalized.as_str()
        } else {
            text
        };
        let total = text.len();
        let selected_bytes = self
            .composer_selection_byte_range()
            .map_or(0, |(start, end)| end.saturating_sub(start));
        let base_len = self.input.len().saturating_sub(selected_bytes);
        let available = MAX_COMPOSER_PASTE_BYTES.saturating_sub(base_len);
        let mut accepted = total.min(available);
        while accepted > 0 && !text.is_char_boundary(accepted) {
            accepted -= 1;
        }
        let text = &text[..accepted];
        if text.is_empty() {
            return (0, total);
        }
        self.startup_intro
            .dismiss(std::time::Instant::now(), self.visual_motion);
        self.delete_composer_selection();
        self.reset_composer_history_recall();
        if self.cursor == self.input.len() {
            self.input.push_str(text);
            self.cursor += text.chars().count();
            return (accepted, total);
        }
        let n = self.input.chars().count();
        self.cursor = self.cursor.min(n);
        let at = byte_of(&self.input, self.cursor);
        self.input.insert_str(at, text);
        self.cursor = grapheme_ceil(&self.input, self.cursor + text.chars().count());
        (accepted, total)
    }

    fn input_backspace(&mut self) {
        self.normalize_composer_cursor();
        if self.delete_composer_selection() {
            return;
        }
        if self.cursor == 0 {
            return;
        }
        self.reset_composer_history_recall();
        if self.cursor == self.input.len() && !self.input.ends_with("\r\n") {
            self.input.pop();
            self.cursor -= 1;
            return;
        }
        let previous = previous_grapheme(&self.input, self.cursor);
        let b0 = byte_of(&self.input, previous);
        let b1 = byte_of(&self.input, self.cursor);
        self.input.replace_range(b0..b1, "");
        self.cursor = grapheme_floor(&self.input, previous);
    }

    fn input_delete_previous_word(&mut self) {
        if let Some((start, end)) = self.composer_selection_range() {
            self.input_kill_range(start, end);
            return;
        }
        let start = prev_word(&self.input, self.cursor);
        self.input_kill_range(start, self.cursor);
    }

    fn input_delete_before_cursor(&mut self) {
        if let Some((start, end)) = self.composer_selection_range() {
            self.input_kill_range(start, end);
            return;
        }
        self.input_kill_range(0, self.cursor);
    }

    fn input_delete_after_cursor(&mut self) {
        if let Some((start, end)) = self.composer_selection_range() {
            self.input_kill_range(start, end);
            return;
        }
        let end = self.input.chars().count();
        self.input_kill_range(self.cursor, end);
    }

    fn input_kill_range(&mut self, start: usize, end: usize) {
        let chars = self.input.chars().count();
        let start = grapheme_floor(&self.input, start.min(chars));
        let end = grapheme_ceil(&self.input, end.min(chars));
        if start >= end {
            return;
        }
        let start_byte = byte_of(&self.input, start);
        let end_byte = byte_of(&self.input, end);
        let bytes = end_byte.saturating_sub(start_byte);
        if bytes > MAX_COMPOSER_KILL_BYTES {
            self.system_msg(format!(
                "composer kill refused · {bytes} bytes exceeds the {} KiB recovery buffer; \
                 draft unchanged",
                MAX_COMPOSER_KILL_BYTES / 1024
            ));
            return;
        }
        self.reset_composer_history_recall();
        self.composer_selection_anchor = None;
        let killed = self.input[start_byte..end_byte].to_string();
        self.input.replace_range(start_byte..end_byte, "");
        self.cursor = grapheme_floor(&self.input, start);
        self.composer_kill_buffer = Some(killed);
    }

    fn input_yank(&mut self) {
        if self.composer_kill_buffer.is_none() {
            return;
        }
        self.delete_composer_selection();
        self.reset_composer_history_recall();
        let Some(killed) = self.composer_kill_buffer.as_deref() else {
            return;
        };
        let killed_chars = killed.chars().count();
        let chars = self.input.chars().count();
        self.cursor = self.cursor.min(chars);
        let at = byte_of(&self.input, self.cursor);
        self.input.insert_str(at, killed);
        self.cursor = grapheme_ceil(&self.input, self.cursor + killed_chars);
    }

    pub(crate) fn reset_composer_history_recall(&mut self) {
        self.composer_history_index = None;
        self.composer_history_draft = None;
        self.composer_history_query = None;
    }

    pub(crate) fn composer_selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.composer_selection_anchor?;
        if anchor == self.cursor {
            return None;
        }
        let start = grapheme_floor(&self.input, anchor.min(self.cursor));
        let end = grapheme_ceil(&self.input, anchor.max(self.cursor));
        (start < end).then_some((start, end))
    }

    fn composer_selection_byte_range(&self) -> Option<(usize, usize)> {
        self.composer_selection_range()
            .map(|(start, end)| (byte_of(&self.input, start), byte_of(&self.input, end)))
    }

    fn extend_composer_selection(&mut self, target: usize) {
        let chars = self.input.chars().count();
        let old_cursor = self.cursor.min(chars);
        let target = if target < old_cursor {
            grapheme_floor(&self.input, target)
        } else {
            grapheme_ceil(&self.input, target)
        };
        let anchor = self.composer_selection_anchor.unwrap_or(old_cursor);
        self.cursor = target;
        self.composer_selection_anchor = (anchor != target).then_some(anchor);
    }

    fn delete_composer_selection(&mut self) -> bool {
        let Some((start, end)) = self.composer_selection_byte_range() else {
            self.composer_selection_anchor = None;
            return false;
        };
        let start_chars = self
            .composer_selection_range()
            .map(|range| range.0)
            .unwrap_or(self.cursor);
        self.input.replace_range(start..end, "");
        self.cursor = grapheme_floor(&self.input, start_chars);
        self.composer_selection_anchor = None;
        self.reset_composer_history_recall();
        true
    }

    fn copy_composer_selection(&mut self) {
        let Some((start, end)) = self.composer_selection_byte_range() else {
            self.slash_completion_notice(
                "composer copy · no text selected (Shift+arrows, then Ctrl-Shift-C)".to_string(),
            );
            return;
        };
        self.pending_clipboard = Some(self.input[start..end].to_string());
    }

    fn composer_history_entry(&self, index: usize) -> Option<&str> {
        self.history
            .iter()
            .rev()
            .filter(|message| {
                message.role == ChatRole::User
                    && message.attachments.is_empty()
                    && !message.content.trim().is_empty()
                    && message.content.len() <= MAX_COMPOSER_HISTORY_BYTES
            })
            .take(MAX_COMPOSER_HISTORY_ENTRIES)
            .nth(index)
            .map(|message| message.content.as_ref())
    }

    fn input_history_previous(&mut self) {
        self.composer_history_query = None;
        let index = self
            .composer_history_index
            .map_or(0, |index| index.saturating_add(1));
        let Some(prompt) = self.composer_history_entry(index).map(str::to_owned) else {
            return;
        };
        if self.composer_history_index.is_none() {
            self.composer_history_draft = Some(self.input.clone());
        }
        self.input = prompt;
        self.cursor = self.input.chars().count();
        self.composer_selection_anchor = None;
        self.composer_history_index = Some(index);
    }

    fn input_history_next(&mut self) {
        if self.composer_history_query.is_some() {
            self.input = self.composer_history_draft.take().unwrap_or_default();
            self.cursor = self.input.chars().count();
            self.composer_selection_anchor = None;
            self.composer_history_index = None;
            self.composer_history_query = None;
            return;
        }
        let Some(index) = self.composer_history_index else {
            return;
        };
        if index == 0 {
            self.input = self.composer_history_draft.take().unwrap_or_default();
            self.cursor = self.input.chars().count();
            self.composer_selection_anchor = None;
            self.composer_history_index = None;
            return;
        }
        let next = index - 1;
        let Some(prompt) = self.composer_history_entry(next).map(str::to_owned) else {
            self.reset_composer_history_recall();
            return;
        };
        self.input = prompt;
        self.cursor = self.input.chars().count();
        self.composer_selection_anchor = None;
        self.composer_history_index = Some(next);
    }

    fn input_history_search_previous(&mut self) {
        let start = self
            .composer_history_index
            .map_or(0, |index| index.saturating_add(1));
        if self.composer_history_query.is_none() {
            self.composer_history_draft = Some(self.input.clone());
            self.composer_history_query = Some(self.input.to_lowercase());
            self.composer_history_index = None;
        }
        let query = self.composer_history_query.as_deref().unwrap_or("");
        let mut scanned_bytes = 0usize;
        let mut matched = None;
        for (index, message) in self
            .history
            .iter()
            .rev()
            .filter(|message| {
                message.role == ChatRole::User
                    && message.attachments.is_empty()
                    && !message.content.trim().is_empty()
                    && message.content.len() <= MAX_COMPOSER_HISTORY_BYTES
            })
            .take(MAX_COMPOSER_HISTORY_ENTRIES)
            .enumerate()
            .skip(start)
        {
            let next = scanned_bytes.saturating_add(message.content.len());
            if next > MAX_COMPOSER_HISTORY_SEARCH_BYTES {
                break;
            }
            scanned_bytes = next;
            if message.content.to_lowercase().contains(query) {
                matched = Some((index, message.content.to_string()));
                break;
            }
        }
        let Some((index, prompt)) = matched else {
            return;
        };
        self.input = prompt;
        self.cursor = self.input.chars().count();
        self.composer_selection_anchor = None;
        self.composer_history_index = Some(index);
    }

    fn vim_normal_key(&mut self, code: KeyCode) -> bool {
        let n = self.input.chars().count();
        match code {
            KeyCode::Char('h') => self.cursor = previous_grapheme(&self.input, self.cursor),
            KeyCode::Char('l') => {
                if self.cursor < n {
                    self.cursor = next_grapheme(&self.input, self.cursor);
                }
            }
            KeyCode::Char('0') => self.cursor = 0,
            KeyCode::Char('$') => self.cursor = previous_grapheme(&self.input, n),
            KeyCode::Char('w') => {
                self.cursor = grapheme_ceil(&self.input, next_word(&self.input, self.cursor))
            }
            KeyCode::Char('b') => {
                self.cursor = grapheme_floor(&self.input, prev_word(&self.input, self.cursor))
            }
            KeyCode::Char('i') => self.vim_normal = false,
            KeyCode::Char('a') => {
                if self.cursor < n {
                    self.cursor = next_grapheme(&self.input, self.cursor);
                }
                self.vim_normal = false;
            }
            KeyCode::Char('A') => {
                self.cursor = n;
                self.vim_normal = false;
            }
            KeyCode::Char('I') => {
                self.cursor = 0;
                self.vim_normal = false;
            }
            KeyCode::Char('x') => {
                if self.cursor < n {
                    self.reset_composer_history_recall();
                    self.composer_selection_anchor = None;
                    let b0 = byte_of(&self.input, self.cursor);
                    let b1 = byte_of(&self.input, next_grapheme(&self.input, self.cursor));
                    self.input.replace_range(b0..b1, "");
                    let m = self.input.chars().count();
                    self.cursor = grapheme_floor(&self.input, self.cursor.min(m));
                }
            }
            KeyCode::Char('D') => {
                if self.cursor < n {
                    self.reset_composer_history_recall();
                    self.composer_selection_anchor = None;
                    let at = byte_of(&self.input, self.cursor);
                    self.input.truncate(at);
                }
            }
            KeyCode::Char('C') => {
                if self.cursor < n {
                    self.reset_composer_history_recall();
                    self.composer_selection_anchor = None;
                    let at = byte_of(&self.input, self.cursor);
                    self.input.truncate(at);
                }
                self.vim_normal = false;
            }
            KeyCode::Char('d') => {
                if !self.input.is_empty() {
                    self.reset_composer_history_recall();
                    self.composer_selection_anchor = None;
                }
                self.input.clear();
                self.cursor = 0;
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__turn_io__terminal_text_tests.rs"]
mod terminal_text_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__turn_io__world_clock_tests.rs"]
mod world_clock_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__turn_io__graph_crop_tests.rs"]
mod graph_crop_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__turn_io__reflow_tests.rs"]
mod reflow_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__turn_io__proc_completion_continuation_tests.rs"]
mod proc_completion_continuation_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__turn_io__reasoning_roll_grapheme_tests.rs"]
mod reasoning_roll_grapheme_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__turn_io__r04c_cont2_measurements.rs"]
mod r04c_cont2_measurements;

fn dock_control(button: &WorldButton) -> bool {
    matches!(button, WorldButton::DockToggle)
}
