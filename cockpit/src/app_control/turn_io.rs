//! Streaming advance loop and input/key/mouse/paste handling.
use super::*;
use crate::WorldButton;

/// Keep one provider burst from monopolizing the UI thread. The count bound
/// handles tiny token deltas; the payload bound handles unusually large
/// gateway chunks. One event may cross the byte limit because it has already
/// left the channel, but the next event waits for the following frame.
/// Cap ingest so a provider flood cannot monopolize a UI frame: cancel/steer
/// keys must be able to paint within a second even while tokens arrive.
pub(crate) const STREAM_EVENTS_PER_FRAME: usize = 64;
pub(crate) const STREAM_BYTES_PER_FRAME: usize = 8 * 1024;
/// Display-side sensing can arrive in a reconnect burst. Preserve FIFO while
/// preventing that optional telemetry from monopolizing input/draw latency.
pub(crate) const VILLAGE_PULSES_PER_FRAME: usize = 64;
/// A pasted draft larger than this would make every subsequent composer wrap
/// and draw traverse context the model cannot use effectively. Ordinary typing
/// remains unrestricted; this guards accidental bulk clipboard ingress.
pub(crate) const MAX_COMPOSER_PASTE_BYTES: usize = 256 * 1024;
/// A destructive editing chord never captures an unbounded second copy of a
/// draft. Oversized kills are refused before mutation so recovery stays exact.
pub(crate) const MAX_COMPOSER_KILL_BYTES: usize = 256 * 1024;
/// Recall stays bounded even across imported/legacy sessions that predate paste
/// admission. Attachment-bearing prompts are excluded separately because their
/// text cannot truthfully replay the omitted media.
pub(crate) const MAX_COMPOSER_HISTORY_BYTES: usize = 256 * 1024;
pub(crate) const MAX_COMPOSER_HISTORY_ENTRIES: usize = 100;
/// Ctrl-R case folding and substring checks never traverse an unbounded imported
/// history in one key event.
pub(crate) const MAX_COMPOSER_HISTORY_SEARCH_BYTES: usize = 2 * 1024 * 1024;
/// The simulation clock stays at 40 Hz even though scenery is painted at a
/// lower cadence. Intermediate states are advanced without being rendered.
const WORLD_TICK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(25);
/// Never replay an unbounded world backlog after suspension or a long stall.
/// Eight ticks cover two hundred milliseconds, enough for normal draw jitter.
const MAX_WORLD_CATCH_UP_TICKS: u32 = 8;
const RETAINED_PARTIAL_NOTE: &str =
    " · partial reply retained above (display only; retry starts from the last committed turn)";

/// Reveal only complete extended graphemes already present in the received text.
/// The cursor examines local boundary context instead of scanning the entire
/// revealed prefix on every frame. Provider chunks can still extend the last
/// previously visible grapheme; this checks against all bytes available now.
fn reasoning_reveal_boundary(text: &str, shown: usize, step: usize) -> usize {
    let mut target = shown.saturating_add(step).min(text.len());
    while target < text.len() && !text.is_char_boundary(target) {
        target += 1;
    }
    let mut cursor = unicode_segmentation::GraphemeCursor::new(target, text.len(), true);
    if matches!(cursor.is_boundary(text, 0), Ok(true)) {
        target
    } else {
        // All source context is supplied. On an unexpected incomplete result,
        // the full received text remains a safe boundary and guarantees progress.
        cursor
            .next_boundary(text, 0)
            .ok()
            .flatten()
            .unwrap_or(text.len())
    }
}

fn queue_proc_completion_for_loop(
    state: &mut crate::loop_ctl::LoopState,
    workspace: &std::path::Path,
    message: String,
) -> bool {
    if state.status != crate::loop_ctl::LoopStatus::Running
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

/// Incremental terminal-text filter for untrusted provider prose. Escape
/// sequences may cross stream chunks, so filtering each delta independently
/// would leak printable tails such as `[31m` into transcript wrapping.
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

    let mut matches = crate::harness::confined_read_dir(workspace, parent)
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

/// `ANGEL_TUI_ATTENTION` is launch config (quiet-mode escape hatch).
///
/// A7: completion/error paths call [`App::request_terminal_attention`] many
/// times per turn; production caches the first env read. Tests re-read so
/// `TestEnvGuard` remains live.
fn tui_attention_enabled() -> bool {
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| crate::harness::env_flag("ANGEL_TUI_ATTENTION", true))
    }
    #[cfg(test)]
    crate::harness::env_flag("ANGEL_TUI_ATTENTION", true)
}

fn stream_event_is_operator_visible(event: &TurnEvent) -> bool {
    match event {
        TurnEvent::Token(text)
        | TurnEvent::Reasoning(text)
        | TurnEvent::Notice(text)
        | TurnEvent::RolloutCaptureError(text) => !text.is_empty(),
        TurnEvent::Heartbeat | TurnEvent::SuppressPartial => false,
        TurnEvent::ToolCall { .. }
        | TurnEvent::ToolResult { .. }
        | TurnEvent::SpendMilestone { .. }
        | TurnEvent::SubmissionSlot(_)
        | TurnEvent::Media { .. } => true,
    }
}

/// Index of this turn's one-slot row for a repeating notice: same coalesce key
/// when the guard has one, byte-identical body otherwise — so ANY notifier
/// that repeats verbatim coalesces, not just the curated guard families.
fn notice_gauge_row_index(messages: &[Message], note: &str) -> Option<usize> {
    if note.starts_with("action receipt · ") {
        return None;
    }
    let key = turn_event_view::notice_coalesce_key(note);
    messages
        .iter()
        .enumerate()
        .rev()
        .take_while(|(_, message)| !matches!(message.role, Role::User))
        .filter(|(_, message)| matches!(message.role, Role::Activity))
        .find_map(|(index, message)| {
            let body = turn_event_view::activity_notice_body(&message.text)?;
            let same_slot = match key {
                Some(key) => turn_event_view::notice_coalesce_key(body) == Some(key),
                None => body == note,
            };
            same_slot.then_some(index)
        })
}

/// Number of simulation ticks due, plus whether older backlog must be dropped.
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
        if !crate::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
            // Hidden time is not animation debt. Otherwise reopening the Stage
            // after minutes away would immediately enter a catch-up storm.
            // Comp / lean mode is additional: default cockpit still ticks.
            self.world_ticked_at = now;
            return;
        }
        let elapsed = now.saturating_duration_since(self.world_ticked_at);
        let (ticks, dropped_backlog) = world_tick_budget(elapsed);
        if ticks == 0 {
            return;
        }
        for _ in 0..ticks {
            self.world.tick();
        }
        self.world_ticked_at = if dropped_backlog {
            now
        } else {
            self.world_ticked_at
                .checked_add(WORLD_TICK_INTERVAL.saturating_mul(ticks))
                .unwrap_or(now)
        };
    }

    /// Roll the cached reasoning into the avatar canvas by one frame's worth. The
    /// reasoning FEED itself is never touched or delayed; this only paces how fast
    /// the *display* fills. The pane stays up after the turn ends (the roll keeps
    /// ticking until it catches up), so the reply is never held up — it lands in
    /// the transcript while the think finishes rolling alongside it. Display-only;
    /// never affects the model or the turn.
    fn roll_reasoning(&mut self) {
        const ROLL_STEP: usize = 8; // ~240 chars/s at 30fps — a slow, readable roll
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

    /// Clear the streamed partial and its incremental detector together so
    /// marker overlap and fence state cannot leak into the next stream.
    pub(crate) fn clear_partial(&mut self) {
        self.partial.clear();
        self.stream_text_sanitizer.reset();
        self.stream_artifact_detector = Default::default();
        self.partial_height_cache.clear();
    }

    /// The production event and captured-document paths share one Stage delivery.
    pub(crate) fn present_media(&mut self, kind: &str, label: &str, url: &str) {
        if !crate::media::presentation_text_valid(label, 512)
            || !crate::media::presentation_text_valid(url, 8192)
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
            card: Box::new(turn_event_view::media_card(kind, label, url)),
            root,
        };
        self.media.push(card);
        self.scryglass.reveal_media(self.media.len() - 1, true);
        let _ = self
            .module_host
            .activate(&crate::runtime::ModuleId::new("artifacts"));
        self.messages.push(Message {
            role: Role::Activity,
            text: format!(
                "{}\nSource: {}",
                turn_event_view::media_delivery_text(
                    label,
                    Ok("requested in Stage · stays open until dismissed".into())
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
                self.scryglass.reveal_media(self.media.len() - 1, true);
                let _ = self
                    .module_host
                    .activate(&crate::runtime::ModuleId::new("artifacts"));
                let mut note = format!(
                    "artifact captured · {label}\nfile: {}\n(full generated document is in the artifacts panel, not chat)",
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
        crate::harness::confined_create_new_no_symlinks(
            root,
            &relative,
            candidate.body.as_bytes(),
        )?;
        Ok(root.join(relative))
    }

    /// Show completed proc_run work even after the model has answered. Only an
    /// already-running loop in this exact workspace may use a notice to bring
    /// forward its next cycle; loop_arm still enforces every budget/goal gate.
    fn drain_background_process_completions(&mut self) {
        let workspace = self.tools.current_workspace().to_path_buf();
        let canonical = &self.tools.workspace_boundary().canonical_root;
        let driving = self.loop_ctl.status == crate::loop_ctl::LoopStatus::Running
            && self.loop_ctl.workspace.as_deref() == Some(workspace.as_path());
        let limit = if driving {
            8usize.saturating_sub(self.loop_ctl.pending_proc_completions.len())
        } else {
            8
        };
        for completion in crate::tools::proc::take_completions(canonical, limit) {
            let message = completion.message();
            self.system_msg(message.clone());
            self.history.push(ChatMsg::harness(message.clone()));
            queue_proc_completion_for_loop(&mut self.loop_ctl, &workspace, message);
        }
    }

    /// Poll the worker; when the reply (or error) lands, record it and clear.
    /// Also drains live tool-call events so the UI shows activity as it happens.
    pub(crate) fn advance(&mut self) {
        self.drain_background_process_completions();
        self.flush_pending_atlas_harvest();
        // Hidden / backdrop-off never paint artifacts, so expired ceremonies
        // would otherwise keep overlay + completion fireworks latched.
        self.clear_expired_lifecycle_ceremony();
        // A7: no portal renderer → skip the process-global activity mutex and
        // packet projection every UI tick (common for ordinary terminal runs).
        if Self::side_column_visuals_allowed() && self.agentviz_portal.has_renderer() {
            self.agentviz_portal.advance(&crate::agentviz::activity());
        }
        // Detached work has already released the flight slot. Drain a bounded
        // number of completion notices per frame so success and failure remain
        // visible without letting a burst monopolize the UI loop.
        for _ in 0..8 {
            let Ok(notice) = self.background_notice_rx.try_recv() else {
                break;
            };
            let message = notice.message(self.tools.current_workspace());
            self.system_msg(message);
        }
        // Ctrl-V clipboard ingress: one drained read per tick. A staged image is
        // then visible on the composer chip; an Enter that arrived while the read
        // was still running is honored now, with the image attached.
        match self.clipboard_paste.poll() {
            Some(crate::clipboard::Drained::Staged)
                if self.clipboard_paste.take_deferred_submit() =>
            {
                self.submit();
            }
            Some(crate::clipboard::Drained::Text(text)) => {
                self.insert_pasted_text(&text);
                if self.clipboard_paste.take_deferred_submit() {
                    self.submit();
                }
            }
            Some(crate::clipboard::Drained::Failed(error)) => {
                self.system_msg(format!("clipboard paste failed · {error}"));
            }
            Some(crate::clipboard::Drained::Staged) | None => {}
        }
        let foreground_idle = self.exit_request.is_none()
            && self.thinking.is_none()
            && self.bg_job.is_none()
            && self.loop_pending.is_none()
            && self.pending_approval.is_none();
        let clerk = self.tools.clerk();
        if crate::backplane::mode() != crate::backplane::BackplaneMode::Legacy {
            clerk.tick(
                foreground_idle,
                || self.bag.atlas_teacher_route(),
                self.tools.backplane(),
            );
        }
        // A2: miniworld status mirrors (memory/atlas/backplane badges) are
        // display-only — skip when the Stage was not laid out last frame
        // (ANGEL_BACKDROP=off / shell / image / compact Core). Comp / lean
        // keeps chrome but must not pay the badge/classify tax.
        if crate::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
            self.world.note_memory_health(self.tools.store.health());
            let atlas_status = self.atlas.status();
            self.world
                .note_atlas_status(atlas_status.health, atlas_status.review_count);
            let clerk_status = clerk.status();
            self.world.note_backplane_status(
                clerk_status.health,
                clerk_status.queue_depth,
                usize::from(
                    clerk_status.health == crate::atlas_clerk::ClerkHealth::ResourceConflict,
                ),
            );
        }
        match self.session.save_status() {
            crate::session::SessionSaveStatus::Failed(error) => {
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
            crate::session::SessionSaveStatus::Pending => {}
            crate::session::SessionSaveStatus::Healthy => {
                self.session_warning_notified = false;
            }
        }
        // A `/self reborn` rebuild landing green stages the exec and ends the
        // run loop (the phoenix step happens in `main` after teardown).
        self.drain_reborn();
        // Avatar roll-in: pace the reasoning *display*. Keeps ticking after the
        // turn ends until the whole think is on screen — the pane persists as a
        // conversation-position marker until the next submit clears it.
        // A2: Stage lesson roll is display-only — skip when the Stage was not
        // laid out last frame (ANGEL_BACKDROP=off / shell / image) so invisible
        // roll work does not run on every advance. Comp / lean keeps chrome.
        if crate::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
            self.scryglass.poll_lesson(self.visual_motion.animates());
        }
        if !Self::reasoning_roll_in_allowed() {
            // Snap so the think pane still paints after the turn ends.
            self.reasoning_shown = self.reasoning.len();
        } else if self.thinking.is_some() || self.reasoning_roll_pending() {
            self.roll_reasoning();
        }
        // The miniworld advances on a 40 Hz wall clock, independently of its
        // cheaper paint cadence. A slow draw catches up several simulation
        // steps before painting one frame, so motion duration stays stable
        // without paying to render every intermediate state.
        // A2: scenery earns ticks only while a Stage surface was laid out on the
        // last draw. ANGEL_BACKDROP=off, shell focus, image view, and compact
        // Core never pay pathfinding / hearth / muster work on the UI loop.
        self.advance_world_clock();
        // A2: loop loom / quintain routing is Stage presentation — do not build
        // budget snapshots or note_loop when the world is not painted.
        // Comp / lean keeps chrome; the loom tax is additional.
        // Z1: the adventure mirror drains first (and whether or not the pane
        // is visible) so the world sees the quest events before note_loop.
        self.adventure_mirror_drain();
        if crate::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
            let active = self.loop_active();
            let iteration = self.loop_ctl.iteration;
            // Share the visual inactivity/blocker interpretation with the quest.
            // Objective-comparison absence is not evidence of an idle party.
            let agitated = active && crate::world_viz::LoopMirror::stall_level(&self.loop_ctl) >= 2;
            let done = self.loop_ctl.status == crate::loop_ctl::LoopStatus::Done;
            let loop_budget = active.then(|| {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                crate::world_viz::LoopBudgetSnapshot {
                    iteration,
                    max_iters: self.loop_ctl.max_iters,
                    tokens_spent: self.loop_ctl.tokens_spent,
                    token_budget: self.loop_ctl.token_budget,
                    elapsed_secs: if self.loop_ctl.started_ms == 0 {
                        0
                    } else {
                        now_ms.saturating_sub(self.loop_ctl.started_ms) / 1000
                    },
                    deadline_secs: self.loop_ctl.deadline_secs,
                }
            });
            self.world
                .note_loop(active, iteration, agitated, done, loop_budget);
            let visiting_quintain = self.world.visiting_quintain();
            if visiting_quintain
                && !self.quintain_route_presented
                && self.scryglass.controller.route() == crate::scryglass::StageRoute::Realm
                && self.scryglass.controller.overlay().is_none()
            {
                self.scryglass.navigate(crate::scryglass::StageRoute::Loop);
            }
            self.quintain_route_presented = visiting_quintain;
        } else {
            // Forget presentation latch while hidden so a later visible visit
            // can still expand the loop viz.
            self.quintain_route_presented = false;
        }
        // Village pulses fold in whether or not the pane is showing — the
        // persisted hamlet state must track the fleet either way.
        self.drain_village_pulses();
        self.advance_yukon_fleet();
        // Surface a pending approval (e.g. the swarm wanting to phone a SOTA) as a
        // modal. One at a time — the worker blocks until this one is answered.
        if self.pending_approval.is_none()
            && let Ok(req) = self.approval_rx.try_recv()
        {
            if self.thinking.as_ref().is_none_or(Thinking::is_draining) && self.bg_job.is_none() {
                // The owning turn/job was hard-stopped before the UI drained
                // its request. Deny the stale request instead of presenting
                // an orphan modal with no live action behind it.
                let _ = req.reply.send(crate::approval::Decision::Deny);
            } else {
                self.pending_approval = Some(PendingApproval {
                    prompt: req.prompt,
                    scope_label: Some(req.scope.label()),
                    reply: req.reply,
                });
                self.request_terminal_attention();
            }
        }
        // YOLO is an operator-wide blanket approval, including the two direct
        // UI gates (SOTA loop escalation and green self-edit integration) that
        // do not pass through the shared approval broker.
        if crate::yolo::enabled()
            && let Some(pending) = self.pending_approval.take()
        {
            let _ = pending.reply.send(crate::approval::Decision::ApproveAll);
        }
        self.campaign_drain_pending();
        self.loop_drain_experiment();
        // Drain a finished background job (e.g. `/compact`, `/memories palace`).
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
                                if let Ok(content) = crate::harness::park_compaction_window(
                                    &self.history[range.clone()],
                                    &note.content,
                                ) {
                                    let newer_user_survives = self.history[range.end..]
                                        .iter()
                                        .any(|m| m.role == ChatRole::User);
                                    let anchors = crate::harness::compaction_task_anchors(
                                        &self.history,
                                        range.start,
                                        range.end,
                                        crate::compaction::configured_compact_chunk_tokens()
                                            .saturating_mul(2),
                                    )
                                    .into_iter()
                                    .filter(|anchor| {
                                        !newer_user_survives
                                            || crate::harness::operator_directive(anchor)
                                    })
                                    .collect::<Vec<_>>();
                                    let turn_context =
                                        crate::harness::compaction_turn_context_anchor(
                                            &self.history,
                                            range.start,
                                            range.end,
                                        );
                                    let mut note = note;
                                    note.content =
                                        if crate::compaction::is_compaction_note(&note) {
                                            crate::harness::constraint_retention_note(
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
                Err(mpsc::TryRecvError::Empty) => {} // still running
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
        // A requested exit owns the next idle boundary. Keep polling current
        // owners, but do not launch queued input or another loop iteration.
        if self.exit_request.is_some() && self.thinking.is_none() {
            self.loop_drain_pending();
            self.finish_requested_exit();
            return;
        }
        // A parked manual turn must not prevent its blocking owner from draining.
        if self.thinking.is_none() && self.pending_turn.is_some() {
            self.loop_drain_pending();
        }
        // A submitted turn parked for its echo frame: launch it now that the
        // echo painted. The pre-flight runs here — one tick after the
        // keypress — so Enter → echo latency never includes it.
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
                // A parked turn still waiting on its echo frame owns the
                // flight slot: the loop controller and steer flush must not
                // race a launch that is one tick away.
                if self.pending_turn.is_some() {
                    return;
                }
                // No turn in flight: service the loop controller — drain any
                // off-thread verify/approval result, then arm the next iteration
                // if one is due (honors the single-flight + interval timing).
                if self.campaign_pending.is_none() {
                    self.loop_drain_pending();
                    self.loop_arm();
                    // Steers the last turn never consumed (it ended first) become
                    // the immediate next user turn once nothing else will run.
                    self.flush_queued_steers();
                }
                return;
            }
        };

        // A hard stop retires the visible turn, not its ownership. Keep the
        // shared foreground slot reserved until the worker's terminal send (or
        // sender disconnect) proves quiescence; discard every late result so a
        // cancelled lifecycle can never rewrite conversation state. This path
        // deliberately does not process stream events from the retired turn.
        if thinking.is_draining() {
            match thinking.rx.try_recv() {
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {
                    // Cancellation suppresses the abandoned answer, not real
                    // provider spend. The worker has settled, so its cumulative
                    // counters are now stable and can be folded exactly once.
                    self.fold_turn_cache(&thinking);
                    self.turn_first_output_ms = None;
                    self.route_evidence_loaded_at = None;
                    self.messages.push(Message {
                        role: Role::Activity,
                        text: "stopped worker drained — provider/tool slot released".into(),
                    });
                    self.scroll = 0;
                    self.request_terminal_attention();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.thinking = Some(thinking);
                }
            }
            return;
        }

        // Drain live events: stream text deltas into `partial`, tool-call
        // activity into the scrollback. When a tool call starts, flush any
        // streamed preamble text as a finished message first, so the order reads
        // naturally: [preamble] ▸ tool ↳ result [next reply].
        // ── Steer-idle interrupt ────────────────────────────────────────
        // A stalled provider stream with operator steers queued should deliver
        // them now, not after the full idle timeout (observed live: a Responses
        // route silent for 8+ minutes while three steers sat undeliverable).
        // Interrupt the way Esc's soft press does — set only the cancel flag —
        // but deliberately KEEP the steer queue (Esc drains it); the worker
        // winds down as an ordinary (interrupted) turn end, and the normal
        // flight-slot release below then calls `flush_queued_steers()` so the
        // steers become the next user message. One interrupt per stall: only
        // stream progress (`note_stream_progress`) re-arms it. The tool strip's
        // `has_running_calls()` is the same signal the abandon branch trusts to
        // distinguish "waiting on the provider" from "tool running".
        let steer_idle_secs = thinking.last_stream_at.elapsed().as_secs();
        if !self.tool_strip.has_running_calls()
            && !thinking.steer_interrupt_fired
            && crate::turn::steer_interrupt_due(
                steer_idle_secs,
                self.steer_queue.len(),
                thinking.steer_idle_interrupt_secs,
            )
            && !crate::tools::proc::background_work_pending()
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
                    crate::turn_event_view::route_label(&thinking.requested_route),
                )
                .into(),
            });
            self.thinking = Some(thinking);
            return;
        }
        // ── Turn watchdog ────────────────────────────────────────────────
        // A provider socket that went silent (crashed box, dropped TCP without
        // FIN, hung reasoning stream) leaves the worker blocked forever. The
        // harness has no deadline for non-competition turns, so the UI would
        // spin at 30fps indefinitely burning a core. Abandon instead: if no
        // stream event has arrived for `ANGEL_TURN_IDLE_TIMEOUT_SECS` (default
        // 600s = 10min) AND the turn has been alive at least that long, signal
        // cancel, surface a timeout message, and let the worker wind down.
        // A dispatched tool owns its own deadline and naturally leaves the
        // provider stream quiet until its result event; never classify that
        // productive interval as provider idleness.
        if let Some(idle_timeout) = thinking
            .idle_timeout_secs
            .filter(|_| !self.tool_strip.has_running_calls())
        {
            let idle_secs = thinking.last_stream_at.elapsed().as_secs();
            let total_secs = thinking.started.elapsed().as_secs();
            if idle_secs >= idle_timeout && total_secs >= idle_timeout {
                thinking.begin_draining();
                if let Some(pending) = self.pending_approval.take() {
                    let _ = pending.reply.send(crate::approval::Decision::Deny);
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
                        crate::turn_event_view::route_label(&thinking.requested_route),
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
            // Escalating pre-abandonment truth: a one-shot notice at 50% and
            // 80% of the idle budget names the stalled route while there is
            // still time to steer or cancel, so the watchdog is never
            // silent-until-dead. Stream progress re-arms the stages (see
            // `Thinking::note_stream_progress`).
            if let Some(warning) = thinking.idle_warning_due() {
                self.messages.push(Message {
                    role: Role::Activity,
                    text: format!(
                        "⏱ {} idle {}s — no stream progress; abandoning at {}s ({}% of idle budget; ANGEL_TURN_IDLE_TIMEOUT_SECS)",
                        crate::turn_event_view::route_label(&thinking.requested_route),
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
                // Private reasoning → the agent's canvas pane (never the answer).
                TurnEvent::Reasoning(reasoning) => {
                    let reasoning = self.reasoning_text_sanitizer.push(&reasoning);
                    self.reasoning.push_str(&reasoning);
                }
                TurnEvent::Heartbeat => {}
                TurnEvent::SuppressPartial => {
                    self.clear_partial();
                    self.stream_artifact_notice = false;
                }
                // Tool activity feeds the live strip (a status row + lateral
                // loading bar pinned under the transcript), not the scrollback
                // — the old full T:/R: trace is back behind `/trace`.
                TurnEvent::ToolCall {
                    id,
                    name,
                    args_summary,
                } => {
                    self.ensure_research_session();
                    self.research.start(&id, &name, &args_summary);
                    self.flush_partial();
                    // A2: knight journeys / landmark pulses are Stage presentation.
                    // Skip when the Stage was not laid out last frame
                    // (ANGEL_BACKDROP=off / shell / image) so tool storms do not
                    // pay classify + active_work + scryglass overlay tax.
                    // Comp / lean keeps chrome; the journey tax is additional.
                    if crate::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
                        let destination =
                            crate::world_viz::classify_tool_activity(&name, &args_summary).building;
                        self.world
                            .note_tool_call_event(id.clone(), &name, &args_summary);
                        self.reset_world_yaw();
                        self.scryglass.begin_journey(
                            id.clone(),
                            destination,
                            self.visual_motion != crate::lifecycle_viz::MotionMode::Full,
                        );
                    }
                    if self.transcript_mode == crate::app::TranscriptMode::Trace {
                        self.messages.push(Message {
                            role: Role::Activity,
                            text: turn_event_view::tool_call_text(&name, &args_summary).into(),
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
                    // A2: Stage-only result sparks — strip/trace still update.
                    if crate::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
                        self.world
                            .note_tool_result_event(&id, &name, &summary, outcome);
                    }
                    if self.transcript_mode == crate::app::TranscriptMode::Trace {
                        self.messages.push(Message {
                            role: Role::Activity,
                            text: turn_event_view::tool_result_text(&name, &summary).into(),
                        });
                    } else if turn_event_view::is_council_tool(&name) {
                        // Sub-agent replies are the conversation the operator
                        // actually wants to see — never bury them in the strip.
                        self.flush_partial();
                        self.messages.push(Message {
                            role: Role::Council,
                            text: format!("{name} · {summary}").into(),
                        });
                    }
                    self.tool_strip.result_event(&id, &name, &summary, outcome);
                }
                TurnEvent::Notice(note) | TurnEvent::RolloutCaptureError(note) => {
                    // A2: world notice banners are Stage chrome.
                    if crate::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
                        self.world.note_notice(&note);
                    }
                    // Routine murmurs ride the strip's in-place note line; a
                    // repeating guard holds ONE gauge row that bumps its
                    // ×count in place instead of stacking scrollback; only
                    // first-fire failures and unrecognized gates open a new
                    // line. Crucially, a strip- or gauge-bound note never
                    // flushes the streaming answer — the agent's prose stays
                    // one block instead of being shredded into fragments.
                    let conversation =
                        self.transcript_mode == crate::app::TranscriptMode::Conversation;
                    let receipt = note.starts_with("action receipt · ");
                    if !receipt && !turn_event_view::notice_rides_strip(&note) {
                        self.receipt_run_open = false;
                    }
                    if receipt {
                        self.flush_partial();
                        if !conversation
                            || !self.receipt_run_open
                            || !self.bump_receipt_gauge(&note)
                        {
                            self.messages.push(Message::new(
                                Role::Activity,
                                turn_event_view::notice_text(&note),
                            ));
                        }
                        self.receipt_run_open = conversation;
                    } else if conversation && self.bump_notice_gauge(&note) {
                        // The gauge row advanced in place.
                    } else if conversation && turn_event_view::notice_rides_strip(&note) {
                        self.tool_strip.note_event(&note);
                    } else {
                        self.flush_partial();
                        self.messages.push(Message {
                            role: Role::Activity,
                            text: turn_event_view::notice_text(&note).into(),
                        });
                    }
                }
                TurnEvent::SpendMilestone { input_tokens } => {
                    self.show_spend_coin();
                    self.tool_strip.note_event(&format!(
                        "gold coin · you just spent {} input tokens — continuing",
                        crate::spend_viz::format_input_tokens(input_tokens)
                    ));
                }
                TurnEvent::SubmissionSlot(slot) => {
                    self.submission_slot = slot;
                }
                // The agent delivered a rich-media card via the `present` tool —
                // push it onto the carousel and note it in the activity trace.
                TurnEvent::Media { kind, label, url } => {
                    self.present_media(&kind, &label, &url);
                }
            }
            touched = true;
        }
        if touched {
            // Refresh the watchdog and re-arm its escalation notices: we
            // received stream activity this frame.
            thinking.note_stream_progress();
            // Give every drained stream batch at least one frame on screen
            // before harvesting a simultaneously-ready final result. This is
            // especially important for fast local routes, whose full answer can
            // otherwise arrive and be committed between two draws.
            self.thinking = Some(thinking);
            return;
        }

        match thinking.rx.try_recv() {
            Ok(Ok((new_history, reply, resolved_route, stop_reason))) => {
                let completed = stop_reason == crate::harness::TurnStopReason::Answer;
                let outcome_tools = self.tool_strip.snapshot();
                self.fold_turn_cache(&thinking);
                self.route_evidence_loaded_at = None;
                self.turn_truncated = reply.contains("response incomplete:");
                self.finish_world_turn(completed);
                // Six-state portrait: a clean settlement is a Victory; one that
                // ends a failure run of >= 2 is a Recovery. Decided from the
                // App's own turn facts, never read back from world state.
                if let Some(marker) =
                    crate::agent_view::turn_end_marker(completed, self.portrait_fail_run)
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
                    turn_event_view::answer_receipt_text(
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
                // A loop iteration: fold it into loop state (fresh context — it
                // never touches the user thread or session) and run the stop ladder.
                if self.loop_ctl.awaiting_turn {
                    if completed {
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
                let outcome = crate::backplane::TurnOutcome::settled(
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
                crate::experience::record_backplane_outcome(
                    &outcome,
                    self.tools.current_workspace(),
                );
                self.last_turn_outcome = Some(outcome);
                // Persist the completed turn — off-thread; serializing a long
                // conversation on the UI thread was a visible frame hitch.
                let _ = self.session.save_async(&self.history);
                // An exact streamed answer has already passed the incremental
                // terminal sanitizer and artifact detector. Reuse that work;
                // rescanning megabytes here stalls cancel/turn settlement.
                let streamed_display = (!self.stream_artifact_notice
                    && self.stream_artifact_detector.covers(&self.partial)
                    && reply == self.partial)
                    .then(|| std::mem::take(&mut self.partial));
                self.clear_partial();
                let clear_relentless =
                    self.relentless_execution && relentless_output_delivered(&reply);
                let reply: Arc<str> =
                    Arc::from(streamed_display.unwrap_or_else(|| self.display_reply(reply)));
                // A draft flushed mid-turn that this reply extends is the same
                // answer: rewrite it in place instead of rendering it twice.
                let reused = self.reuse_flushed_partial(&reply);
                if !reused {
                    self.collapse_superseded_partial();
                }
                // Same rule at turn end: a blank final reply (tool-only
                // finish, empty-reply error path) gets no headless block.
                if !reused && !reply.trim().is_empty() {
                    self.messages
                        .push(Message::new(Role::Angel, Arc::clone(&reply)));
                }
                // Handoff RL: charge reply tokens, then demand a forced
                // clear/inject restart only after a submission *result* is in
                // (successful submit, then score/status — same or later turn).
                // Prose alone never trips. Budgets match the agent loop.
                if self.handoff_rl.active {
                    self.handoff_rl.charge_tokens(&reply);
                    if let Some(why) = self.handoff_rl.budget_tripped() {
                        let msg = self.handoff_rl.stop_for_budget(&why);
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
                // The reply was already visible as `partial`; committing it must
                // not replay entry motion or yank a reader out of scrollback.
                self.settle_transcript_spawns();
                self.request_terminal_attention();
            }
            Ok(Err(err)) => {
                self.turn_first_output_ms = None;
                let outcome_tools = self.tool_strip.snapshot();
                self.fold_turn_cache(&thinking);
                self.route_evidence_loaded_at = None;
                self.finish_world_turn(false);
                // Six-state portrait: a failed settlement extends the fail run
                // and drops any lingering marker (no Victory glow over an error).
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
                let outcome = crate::backplane::TurnOutcome::settled(
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
                crate::experience::record_backplane_outcome(
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
                // Still thinking — put it back for the next advance() call.
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

    /// Persist a completion payload after its answer has had one frame to land.
    /// Queue Atlas sanitization and persistence off the UI thread. Accepted
    /// writes retain lock/reload/fsync/rename durability and drain on app drop.
    /// The next tick waits up to 500 ms for durability; a slow write stays queued.
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

    /// Update host focus state from crossterm focus events. Regaining focus
    /// cancels a not-yet-flushed alert because the completed result is already
    /// visible to the operator.
    pub(crate) fn set_terminal_focused(&mut self, focused: bool) {
        self.terminal_focused = focused;
        if !focused {
            self.scryglass_drag = None;
            self.viewer.inspector.drag = None;
        }
        if focused {
            self.attention_requested = false;
        }
    }

    /// Coalesce any number of completion/error events into one signal between
    /// frames. `ANGEL_TUI_ATTENTION=0` is a quiet-mode escape hatch.
    pub(crate) fn request_terminal_attention(&mut self) {
        if !self.terminal_focused && tui_attention_enabled() {
            self.attention_requested = true;
        }
    }

    pub(crate) fn take_attention_request(&mut self) -> bool {
        std::mem::take(&mut self.attention_requested)
    }

    pub(crate) fn drain_village_pulses(&mut self) -> usize {
        let Some(rx) = &self.village_rx else {
            return 0;
        };
        let mut drained = 0;
        for _ in 0..VILLAGE_PULSES_PER_FRAME {
            let Ok(pulse) = rx.try_recv() else {
                break;
            };
            self.world.note_village(pulse);
            drained += 1;
        }
        drained
    }

    /// Z1: diff `loop_ctl` against the last-seen mirror and fold the edge
    /// events into the world quest. Runs every frame regardless of pane
    /// visibility — the quest is session state, like the village pulses —
    /// and before `note_loop` so the world sees the events first.
    pub(crate) fn adventure_mirror_drain(&mut self) {
        let party = self.adventure_party_size();
        let events = self
            .loop_mirror
            .drain(&self.loop_ctl, self.world.tool_mix(), party);
        for ev in events {
            self.world.note_adventure(ev);
        }
    }

    /// Live formation width as the hero's party. No single source of truth
    /// exists (`ANGEL_SWARM_WIDTH` only seeds defaults; the worker owns the
    /// engaged roster mid-turn), so Z1 reads the armed MoA engagement's
    /// assigned seat count — one-shot first, then the session formation.
    /// Solo hero = 1.
    fn adventure_party_size(&self) -> u8 {
        let engaged = self.moa_one_shot.as_ref().or(self.moa_session.as_ref());
        let seats = engaged
            .map(|engagement| engagement.roster.assigned_count())
            .filter(|&seats| seats > 0)
            .unwrap_or(1);
        seats.clamp(1, 8) as u8
    }

    /// Keep the competition fleet fresh without ever running Yukon on the UI
    /// thread. A worker must settle before the cadence can arm another;
    /// hidden, ordinary, and test loops never start network work.
    fn advance_yukon_fleet(&mut self) {
        let mut settled = None;
        if let Some(rx) = self.yukon_fleet_rx.as_ref() {
            match rx.try_recv() {
                Ok(result) => settled = Some(result),
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    settled = Some(Err("Yukon fleet watcher disconnected".to_string()));
                }
            }
        }
        if let Some(result) = settled {
            self.yukon_fleet_rx = None;
            self.yukon_fleet_polled_at = std::time::Instant::now();
            match result {
                Ok(snapshot) => self.yukon_fleet.apply(snapshot),
                Err(error) => self.yukon_fleet.fail(error),
            }
        }

        let competition_loop = crate::loop_viz::visible(&self.loop_ctl)
            && self.submission_slot.phase != crate::harness::SubmissionSlotPhase::Dormant;
        if !cfg!(test)
            && competition_loop
            && self.yukon_fleet_rx.is_none()
            && self.yukon_fleet_polled_at.elapsed() >= crate::yukon_fleet::POLL_INTERVAL
        {
            self.yukon_fleet.begin_scan();
            self.yukon_fleet_rx = Some(crate::yukon_fleet::spawn_poll());
        }
    }

    /// Fold the finished worker's provider-reported cache/token deltas into
    /// the session meter. The club's counters are cumulative, so the turn's
    /// own share is now-minus-spawn-snapshot; a provider that reported no
    /// cache accounting this turn is folded as unreported so the meter shows
    /// "n/a" rather than a fake 0%.
    fn fold_turn_cache(&mut self, thinking: &Thinking) {
        let Some(club) = thinking.club.as_ref() else {
            return;
        };
        let Some(before) = thinking.spawn_usage.get().copied() else {
            // Worker thread never started, so no hops ran and this turn has
            // no provider share to fold. Treating a missing snapshot as zero
            // would attribute the club's whole session to a failed spawn.
            return;
        };
        let cache = club.cache_usage();
        let cache_read = cache
            .read_input_tokens
            .saturating_sub(before.cache_before.read_input_tokens);
        let reported =
            cache.read_accounting_responses > before.cache_before.read_accounting_responses;
        let input = club.token_usage().map_or(0, |after| {
            after
                .total_input
                .saturating_sub(before.usage_before.map_or(0, |usage| usage.total_input))
        });
        self.cache_meter.fold_turn(cache_read, input, reported);
    }

    /// Collapse the turn's tool activity into one compact tally line in the
    /// transcript (the strip's send-off), then reset the strip. A no-op when no
    /// tools ran or `/trace` already put the full trace in the scrollback.
    pub(crate) fn flush_tool_summary(&mut self) {
        let mut summary = self
            .tool_strip
            .take_summary()
            .unwrap_or_else(|| format!("{} 0 calls", Glyph::Tool.token()));
        // Only the exceptional states earn extra words: an incomplete answer
        // is called out, a complete one is simply the answer above the line.
        // (Renown lives in the world pane, not the conversation.)
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

    /// Bound transcript memory over long or endless sessions. `messages` and its
    /// two index-parallel vecs (`transcript_spawns`, `transcript_heights`)
    /// otherwise grow without limit across a `/loop endless` run until OOM —
    /// nothing evicts them (the only shrinkers are `/new` and `/resume`, which
    /// never fire during an unattended loop). `scroll` is an offset in rows from
    /// the *bottom* (0 = newest), so dropping the oldest entries never disturbs
    /// the live viewport, and the draw path already clamps `scroll` to range.
    /// Called from `flush_tool_summary`, which runs once per turn (interactive
    /// and loop), so growth is checked on every turn.
    fn cap_scrollback(&mut self) {
        const SCROLLBACK_CAP: usize = 4000;
        const SCROLLBACK_TARGET: usize = 3000;
        if self.messages.len() <= SCROLLBACK_CAP {
            return;
        }
        self.pending_transcript_reflow = None;
        let drop = self.messages.len() - SCROLLBACK_TARGET;
        self.messages.drain(0..drop);
        // The parallel vecs may trail `messages` (draw syncs them lazily); drain
        // only what exists so each stays a valid prefix of `messages`.
        let sp = drop.min(self.transcript_spawns.len());
        self.transcript_spawns.drain(0..sp);
        let hp = drop.min(self.transcript_heights.len());
        self.transcript_heights.drain(0..hp);
    }

    /// Loop notices span model turns; reuse the existing notice gauge renderer
    /// and row-height invalidation while matching the exact diagnostic.
    pub(crate) fn loop_verifier_notice(&mut self, note: &str, count: usize) {
        let row = if count > 1 {
            crate::turn_event_view::notice_gauge_text(note, count)
        } else {
            crate::turn_event_view::notice_text(note)
        };
        let index = (count > 1)
            .then(|| {
                self.messages.iter().rposition(|m| {
                    matches!(m.role, Role::Activity)
                        && crate::turn_event_view::activity_notice_body(&m.text) == Some(note)
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

    fn bump_receipt_gauge(&mut self, note: &str) -> bool {
        let Some(index) = self.messages.len().checked_sub(1) else {
            return false;
        };
        if !matches!(self.messages[index].role, Role::Activity) {
            return false;
        }
        let Some(text) = turn_event_view::receipt_gauge_text(&self.messages[index].text, note)
        else {
            return false;
        };
        self.messages[index] = Message::new(Role::Activity, text);
        self.refresh_transcript_row_height(index);
        true
    }

    /// Advance a repeating notice's one-slot gauge row in place: latest text,
    /// bumped ×count (the draw layer turns the count into a progressive
    /// background fill). Returns false when no matching row exists this turn —
    /// the caller then routes the first occurrence as usual.
    fn bump_notice_gauge(&mut self, note: &str) -> bool {
        let Some(index) = notice_gauge_row_index(&self.messages, note) else {
            return false;
        };
        let count = turn_event_view::activity_gauge_count(&self.messages[index].text)
            .unwrap_or(1)
            .saturating_add(1);
        self.messages[index] = Message::new(
            Role::Activity,
            turn_event_view::notice_gauge_text(note, count),
        );
        self.refresh_transcript_row_height(index);
        true
    }

    /// Re-measure one rewritten transcript row and patch the height caches so
    /// an in-place gauge bump never forces a full history re-wrap.
    fn refresh_transcript_row_height(&mut self, index: usize) {
        // Equal height at the published width can still differ at a pending width.
        self.pending_transcript_reflow = None;
        if index >= self.transcript_heights.len() {
            // Not measured yet — the append-only sync will measure it.
            return;
        }
        let width = usize::from(self.transcript_heights_w);
        if width == 0 {
            self.invalidate_transcript_layout();
            return;
        }
        let height = crate::transcript::message_height(&self.messages[index], width);
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

    /// Commit any streamed-but-unfinalized assistant text to the transcript as a
    /// completed Angel message. Called when a tool call interrupts the stream;
    /// a no-op when nothing is buffered.
    pub(crate) fn flush_partial(&mut self) {
        if !self.partial.is_empty() {
            let text = std::mem::take(&mut self.partial);
            self.stream_artifact_detector = Default::default();
            self.partial_height_cache.clear();
            self.stream_artifact_notice = false;
            // A tool-call-only hop often streams nothing but newlines before
            // its calls (grok/sota, 2026-09-02): committing that as a block
            // paints an empty `angel` tag plus blank rows per hop — the
            // "broken visuals" transcript of stacked headless angel stubs.
            if text.trim().is_empty() {
                return;
            }
            self.messages.push(Message::new(Role::Angel, text));
            self.flushed_partial_msg = Some(self.messages.len().saturating_sub(1));
            // The streamed text is already on screen — committing it into
            // history must not replay it as a roll-in.
            self.settle_transcript_spawns();
        }
    }

    /// The turn's final reply begins with a draft that a mid-turn flush already
    /// committed (a strip-bound notice arriving after the prose finished, a
    /// tool boundary): rewrite that block in place and report `true` so the
    /// caller skips pushing the answer a second time. Display-only.
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

    /// Settle a public stream that ended without a completed model turn. The
    /// text was already visible to the operator, so retain it in the transcript
    /// to preserve spatial continuity, but leave `history` untouched so retries
    /// begin at the last committed turn.
    fn retain_interrupted_partial(&mut self) -> bool {
        let retained = !self.partial.is_empty();
        self.flush_partial();
        if retained {
            // Remember the flushed draft so a successful follow-up answer can
            // collapse it — a retained draft plus the retry's full reply is
            // the "copies itself twice" transcript (2026-08-15).
            self.retained_partial_msg = Some(self.messages.len().saturating_sub(1));
        }
        retained
    }

    /// A real answer landed after an error path retained a draft: shrink the
    /// stale draft to a stub so the transcript shows one answer, not two.
    /// Display-only — history was never touched by the draft.
    fn collapse_superseded_partial(&mut self) {
        let Some(idx) = self.retained_partial_msg.take() else {
            return;
        };
        let Some(message) = self.messages.get_mut(idx) else {
            return;
        };
        // Index could drift if the transcript was cleared between retention and
        // commit; only ever rewrite the Angel-role draft that was flushed.
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

    /// Fill any missing transcript roll-in slots as settled, for blocks whose
    /// content was already visible when they entered history. Existing slots
    /// keep their spawn so a block mid-roll finishes its slide.
    pub(crate) fn settle_transcript_spawns(&mut self) {
        self.transcript_spawns
            .resize(self.messages.len(), f32::NEG_INFINITY);
    }

    /// Stop the in-flight turn. The first Esc/^C is a soft interrupt: the worker
    /// stops cleanly at its next step boundary. A second press while it's still
    /// running is a hard stop.
    pub(crate) fn interrupt(&mut self) -> bool {
        // A parked turn (submitted, worker not yet spawned) cancels cleanly:
        // drop it and restore the draft to the composer — nothing was sent.
        if self.thinking.is_none()
            && let Some(pending) = self.pending_turn.take()
        {
            if matches!(self.messages.last(), Some(m) if matches!(m.role, Role::User)) {
                self.messages.pop();
                self.invalidate_transcript_layout();
            }
            self.input = pending.raw.to_string();
            self.cursor = self.input.chars().count();
            // The unsent turn's screenshots were folded in at submit: put them
            // back in the composer staging area instead of dropping them. Only
            // the composer's own images — a `/see` attachment is rebuilt by the
            // restored draft text.
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
            if let Some(pending) = self.pending_approval.take() {
                let _ = pending.reply.send(crate::approval::Decision::Deny);
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
        if let Some(pending) = self.pending_approval.take() {
            let _ = pending.reply.send(crate::approval::Decision::Deny);
        }
        // Esc means stop: queued steers must not outlive the interrupt (they'd
        // otherwise flush as a surprise follow-up turn the moment the slot idles).
        let dropped = self.steer_queue.drain().len();
        if dropped > 0 {
            self.messages.push(Message {
                role: Role::System,
                text: format!("{dropped} queued steer(s) dropped with the interrupt").into(),
            });
        }
        // Repeated stop presses share the existing drain boundary. They may
        // retract a queued post-stop follow-up above, but never duplicate
        // teardown state, transcript receipts, or tool-summary settlement.
        if already_draining {
            // A loop may have been explicitly resumed while its old worker
            // drains. A fresh stop still parks that intent without redoing teardown.
            self.loop_on_interrupt();
            return true;
        }
        if cancel.swap(true, Ordering::Relaxed) {
            // The result is no longer visible, but the worker still owns the
            // shared route/tool lifecycle. Retain it as an explicit draining
            // turn until advance() observes terminal channel settlement.
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
        // If an autonomous loop owns this turn, park it so it doesn't re-arm.
        self.loop_on_interrupt();
        true
    }

    /// Esc / ^C: interrupt a running turn. When idle it's a no-op — the cockpit is
    /// closed deliberately by typing `exit`/`quit`, never by a stray keystroke.
    pub(crate) fn interrupt_idle_safe(&mut self) {
        self.interrupt();
    }

    /// Identity check is required even between route/Reveal changes and the next
    /// draw. A previously painted viewport never grants authority to a new asset.
    pub(crate) fn still_active(&self) -> bool {
        matches!(self.scryglass.surface, crate::scryglass::StageSurface::Still(index)
            if self.scryglass.active_media() == Some(index)
            && self.scryglass.active_error().is_none()
            && self.media.get(index).and_then(|m| m.source()).is_some_and(|source|
                self.viewer.still_matches(&source, self.scryglass.media_request_id())))
            && matches!(self.scryglass.controller.overlay(), Some(crate::scryglass::StageOverlay::Media { index })
                if self.scryglass.active_media() == Some(*index))
    }

    pub(crate) fn inspect_still(&mut self, action: crate::still_inspector::Action) -> bool {
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
        // Always release, including outside the pane or beneath a modal.
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
            .is_some_and(|r| crate::mouse::point_in(r, x, y))
        {
            // First-load/fault areas have no painted mapping. Same-scene pending
            // updates keep viewport authority and take the cumulative path below.
            return self
                .viewer
                .inspector
                .scene
                .is_some_and(|r| crate::mouse::point_in(r, x, y))
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
                .action(crate::still_inspector::Action::Fit),
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
        // The global stop contract wins over stage navigation.
        if matches!(code, KeyCode::Esc) && (self.thinking.is_some() || self.bg_job.is_some()) {
            return false;
        }
        if matches!(
            self.scryglass.surface,
            crate::scryglass::StageSurface::Still(_)
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
            use crate::still_inspector::Action;
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
            // Recognized display keys do not become a draft while first decode
            // is pending, but arbitrary printable text still belongs to input.
            self.inspect_still(action);
            return true;
        }
        // Printable Stage controls only own the keyboard when the operator has
        // explicitly focused Scryglass *and* the composer is empty. Once a
        // draft exists, every printable character belongs to that draft. A
        // modifier chord is likewise never interpreted as a Stage shortcut.
        if matches!(code, KeyCode::Char(_))
            && (!self.input.is_empty() || modifiers != KeyModifiers::NONE)
        {
            return false;
        }
        if self.scryglass.surface == crate::scryglass::StageSurface::Research {
            if self.research_key(code, modifiers) {
                return true;
            }
            if code == KeyCode::Esc {
                self.back_from_visual_surface();
                return true;
            }
            return false;
        }
        if self.scryglass.surface == crate::scryglass::StageSurface::WorldMap
            && self.input.is_empty()
            && modifiers == KeyModifiers::NONE
            && code == KeyCode::Char('r')
        {
            self.open_research(None);
            return true;
        }
        if self.scryglass.surface == crate::scryglass::StageSurface::Vault && self.atlas.enabled() {
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
        if self.scryglass.surface == crate::scryglass::StageSurface::Observatory {
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
                            == crate::observatory::ObservatoryFocus::Campaigns
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
            // Printable keys and non-empty Enter keep flowing to the composer;
            // gallery navigation never steals an in-progress slash command.
            return false;
        }
        if self.scryglass.surface == crate::scryglass::StageSurface::Reinforce {
            if matches!(code, KeyCode::Esc) {
                self.back_from_visual_surface();
                return true;
            }
            if self.input.is_empty() && modifiers == KeyModifiers::NONE {
                self.rl_view = match code {
                    KeyCode::Left | KeyCode::Char('h') => self.rl_view.step(-1),
                    KeyCode::Right | KeyCode::Char('l') => self.rl_view.step(1),
                    KeyCode::Char('1') | KeyCode::Char('b') => crate::rl_viz::RlView::Branch,
                    KeyCode::Char('2') | KeyCode::Char('r') => crate::rl_viz::RlView::Research,
                    KeyCode::Char('3') | KeyCode::Char('s') => crate::rl_viz::RlView::Sankey,
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
                crate::scryglass::StageSurface::Document(_)
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
            if self.scryglass.surface == crate::scryglass::StageSurface::Catalog {
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
            if self.scryglass.surface == crate::scryglass::StageSurface::Lesson {
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
                crate::scryglass::StageSurface::Arrival(_)
            ) && matches!(code, KeyCode::Enter)
            {
                self.enter_world_interior();
                return true;
            }
            if self.scryglass.surface == crate::scryglass::StageSurface::WorldFirstPerson {
                if self.world.interior_building() == Some(crate::world_viz::Building::Scriptorium)
                    && matches!(code, KeyCode::Enter | KeyCode::Char('c'))
                {
                    self.scryglass.open_catalog();
                    return true;
                }
                // Keep the familiar key as a status shortcut. Dotmax is the
                // sole outdoor renderer; room entry owns plate transitions.
                if matches!(code, KeyCode::Char('v')) {
                    crate::world_viz::world3d::cycle();
                    self.system_msg(crate::world_viz::world3d::status_line());
                    return true;
                }
                if !self.world.inside_interior()
                    && !self.world.riding()
                    && matches!(code, KeyCode::Enter)
                {
                    self.enter_world_interior();
                    return true;
                }
            }
        }
        if self.input.is_empty()
            && self.scryglass.surface == crate::scryglass::StageSurface::WorldMap
        {
            match code {
                KeyCode::Left | KeyCode::Char('h') => {
                    self.world.cycle_landmark(-1);
                    return true;
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    self.world.cycle_landmark(1);
                    return true;
                }
                KeyCode::Enter => {
                    self.reset_world_yaw();
                    self.scryglass.toggle_world_route(self.world.destination());
                    return true;
                }
                _ => {}
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
                self.scryglass.navigate(crate::scryglass::StageRoute::Vault);
            }
            KeyCode::Char('m') => {
                self.reset_world_yaw();
                self.scryglass.toggle_world_route(self.world.destination());
            }
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
            KeyCode::Char('0') | KeyCode::Char('r')
                if matches!(
                    self.scryglass.controller.route(),
                    crate::scryglass::StageRoute::Explore(_)
                ) =>
            {
                self.scryglass.follow()
            }
            KeyCode::Left | KeyCode::Char('h')
                if matches!(
                    self.scryglass.controller.route(),
                    crate::scryglass::StageRoute::Explore(_)
                ) =>
            {
                self.scryglass.adjust_look(-0.17, 0.0)
            }
            KeyCode::Right | KeyCode::Char('l')
                if matches!(
                    self.scryglass.controller.route(),
                    crate::scryglass::StageRoute::Explore(_)
                ) =>
            {
                self.scryglass.adjust_look(0.17, 0.0)
            }
            KeyCode::Up | KeyCode::Char('k')
                if matches!(
                    self.scryglass.controller.route(),
                    crate::scryglass::StageRoute::Explore(_)
                ) =>
            {
                self.scryglass.adjust_look(0.0, -0.05)
            }
            KeyCode::Down | KeyCode::Char('j')
                if matches!(
                    self.scryglass.controller.route(),
                    crate::scryglass::StageRoute::Explore(_)
                ) =>
            {
                self.scryglass.adjust_look(0.0, 0.05)
            }
            KeyCode::Char('+') | KeyCode::Char('=')
                if matches!(
                    self.scryglass.controller.route(),
                    crate::scryglass::StageRoute::Explore(_)
                ) =>
            {
                self.scryglass.adjust_fov(-0.05)
            }
            KeyCode::Char('-')
                if matches!(
                    self.scryglass.controller.route(),
                    crate::scryglass::StageRoute::Explore(_)
                ) =>
            {
                self.scryglass.adjust_fov(0.05)
            }
            _ => return false,
        }
        true
    }

    /// The visual panes are deliberately display-only, but each one still needs
    /// an obvious way back to the operator's message surface. Keep the mouse
    /// button and Esc on the same path so neither can strand a focused pane.
    fn back_from_visual_surface(&mut self) {
        self.viewer.clear_still();
        if self.scryglass.back_overlay() {
            self.lifecycle_ceremony = None;
            return;
        }
        if self.world.inside_interior() {
            if self.world.leave_interior() {
                self.reset_world_yaw();
            }
            return;
        }
        match self.scryglass.surface {
            crate::scryglass::StageSurface::Moa => {
                self.moa_deck = None;
            }
            crate::scryglass::StageSurface::Raytrace => {
                let _ = self
                    .module_host
                    .suspend(&crate::runtime::ModuleId::new("graph"));
            }
            crate::scryglass::StageSurface::Lifecycle => {
                self.lifecycle_ceremony = None;
            }
            crate::scryglass::StageSurface::Observatory => {
                self.observatory.clear_viewport();
            }
            crate::scryglass::StageSurface::Reinforce => {}
            crate::scryglass::StageSurface::Research => {
                self.research.expanded = false;
            }
            crate::scryglass::StageSurface::AgentGraph => {}
            crate::scryglass::StageSurface::Quest => {}
            crate::scryglass::StageSurface::Lesson => {}
            crate::scryglass::StageSurface::Catalog => {}
            crate::scryglass::StageSurface::WorldMap => {}
            crate::scryglass::StageSurface::Arrival(_)
            | crate::scryglass::StageSurface::Still(_)
            | crate::scryglass::StageSurface::Document(_)
            | crate::scryglass::StageSurface::Video(_)
            | crate::scryglass::StageSurface::Vault
            | crate::scryglass::StageSurface::Fault => {}
            crate::scryglass::StageSurface::Workshop
            | crate::scryglass::StageSurface::Loop
            | crate::scryglass::StageSurface::WorldFirstPerson
            | crate::scryglass::StageSurface::Hidden => {}
        }
        if !self.scryglass.controller.back() {
            self.focus_module("core");
        }
    }

    pub(crate) fn on_key(&mut self, key: event::KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        if !matches!(key.code, KeyCode::Tab) {
            self.skill_completion_catalog = None;
            self.workspace_completion_cache = None;
        }
        // An approval modal captures keys until answered: y/Enter approve,
        // a approve-all (this kind, rest of turn), n/Esc deny. Other keys ignored.
        if let Some(pa) = self.pending_approval.take() {
            match approval_view::decision_for_key(key.code) {
                Some(d) => {
                    let _ = pa.reply.send(d);
                    self.messages.push(Message {
                        role: Role::Activity,
                        text: format!(
                            "{} approval: {}",
                            Glyph::Approval.token(),
                            approval_view::decision_text(d)
                        )
                        .into(),
                    });
                }
                None => self.pending_approval = Some(pa), // keep waiting for a valid key
            }
            return;
        }
        if self.loop_dialog.is_some() && self.loop_dialog_key(key.code) {
            return;
        }
        if self.agent_menu.is_some() && self.agent_menu_key(key.code) {
            return;
        }
        if self.moa_deck.is_some() && self.moa_deck_key(key.code, key.modifiers) {
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
        // ^G toggles the shell pane — and is the escape hatch back out of it.
        if ctrl && matches!(key.code, KeyCode::Char('g')) {
            self.toggle_shell();
            return;
        }
        // When the shell is focused, every other key goes to the PTY.
        if self.shell_focused {
            if let (Some(sh), Some(bytes)) = (self.shell.as_mut(), pty::key_to_bytes(&key)) {
                sh.send(&bytes);
            }
            return;
        }
        // Ctrl-L mirrors `/redraw`: clear stale outer-terminal cells on the
        // next frame without touching the conversation or an active turn.
        if ctrl && matches!(key.code, KeyCode::Char('l')) {
            self.request_redraw("full terminal redraw queued (Ctrl-L)");
            return;
        }
        if self.handle_module_focus_key(key.code) {
            return;
        }
        self.normalize_composer_cursor();
        // /vim modal composer: in normal mode the letter keys are motions/edits;
        // in insert mode Esc returns to normal. Non-vim keys fall through.
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
        // Custom keybinds (set via /keymap) take precedence over the defaults.
        if let Some(action) = self.lookup_keybind(key.code, key.modifiers) {
            self.do_action(action);
            return;
        }
        if self.scryglass_key(key.code, key.modifiers) {
            return;
        }
        match key.code {
            // Esc / ^C soft-interrupt a running turn (press again to hard-stop).
            // They never close the app — type `exit` (or `quit`) to do that.
            KeyCode::Esc
                if self.thinking.is_none()
                    && self.bg_job.is_none()
                    && self.composer_selection_range().is_some() =>
            {
                self.composer_selection_anchor = None;
            }
            // Esc has no interrupt or cancel to make right now, so it removes the
            // staged screenshot(s) and keeps the typed draft. A running turn,
            // parked turn, background job or loop keeps its own Esc meaning
            // (interrupt), and an active selection is cleared by the arm above.
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
            // Ctrl-V: an in-app image paste. The read runs off the UI thread and
            // the composer chip appears when it lands; text clipboards paste as
            // text. The shell pane and open modals returned above, so each keeps
            // its own input ownership.
            KeyCode::Char('v') if ctrl && !shift => self.paste_clipboard_image(),
            KeyCode::Char('a') if ctrl && shift => {
                let end = self.input.chars().count();
                self.cursor = end;
                self.composer_selection_anchor = (end > 0).then_some(0);
            }
            // Tab switches the agent (box/PC). Left/Right page the transcript —
            // they never cycle the model. Route/mode selection is deliberate:
            // mouse, Tab, or the route deck (F9/F10); `[`/`]` cycle effort.
            KeyCode::Tab if self.complete_slash_command() => {}
            KeyCode::Tab
                if self.thinking.is_none()
                    && self.bg_job.is_none()
                    && self.pending_turn.is_none() =>
            {
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
            // Module toggles: keep them in the Rust host, matching the manifest ids.
            KeyCode::F(2) => {
                let _ = self
                    .module_host
                    .focus(&crate::runtime::ModuleId::new("core"));
            }
            KeyCode::F(3) => self.focus_module("agent"),
            KeyCode::F(4) => self.focus_module("artifacts"),
            KeyCode::F(5) => self.toggle_module("graph"),
            KeyCode::F(9) => self.open_agent_menu(crate::agent_controls::AgentMenuKind::Model),
            KeyCode::F(10) => self.open_agent_menu(crate::agent_controls::AgentMenuKind::Thinking),
            // Scrollback (clamped to the top in ui).
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
        if self.pending_approval.is_some() {
            return;
        }
        if self.loop_dialog.is_some() {
            return;
        }

        if self.moa_deck_owns_input() {
            return;
        }

        if self.agent_menu.is_some() {
            return;
        }
        if self.shell_focused {
            if let Some(sh) = self.shell.as_mut() {
                sh.send(text.as_bytes());
            }
            return;
        }
        self.insert_pasted_text(text);
    }

    /// Insert clipboard text at the caret with the single-shot clip receipt.
    /// Shared by native bracketed paste and the Ctrl-V text fallback so both
    /// paths report the same cap truthfully.
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

    /// Ctrl-V ingress: read the clipboard on a worker thread. `advance` drains the
    /// result into a staged screenshot, an ordinary text paste, or a visible
    /// failure that leaves the draft alone.
    fn paste_clipboard_image(&mut self) {
        self.clipboard_paste.start();
    }

    /// True when Esc has no interrupt or cancel to make, i.e. the composer owns
    /// the keystroke rather than a turn, a parked turn, a loop or a bg job.
    fn esc_is_free(&self) -> bool {
        self.thinking.is_none()
            && self.bg_job.is_none()
            && self.pending_turn.is_none()
            && self.loop_pending.is_none()
            && self.loop_experiment.is_none()
    }

    /// Tab-complete a command name without touching the provider or stealing
    /// ordinary Tab agent cycling outside a slash word.
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
            self.skill_completion_catalog =
                Some(crate::skills::list_for(self.tools.current_workspace()));
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

    /// Handle a captured mouse event. Routing, in order:
    ///   1. While a modal owns the screen, mouse is ignored.
    ///   2. Over the shell pane, if the PTY program is tracking the mouse, the
    ///      event is *forwarded* to it (SGR/X10 encoded) so vim/htop/etc. work.
    ///      Shift-drag is the explicit app-selection override; a shell that is
    ///      not tracking falls through to selection without the modifier.
    ///   3. The wheel scrolls the pane under the pointer (transcript by default,
    ///      agent thinking when over the agent bay).
    ///   4. Left button down/drag/up drives a transcript-confined freeform
    ///      selection. Other panes remain focusable and scrollable, but can
    ///      never become clipboard sources.
    pub(crate) fn on_mouse(&mut self, ev: event::MouseEvent) {
        use crate::mouse;
        use event::{MouseButton, MouseEventKind};

        let world_drag_valid = self.scryglass_drag.is_some_and(|drag| {
            self.scryglass.visible
                && self.scryglass.renderable
                && self.scryglass.surface == drag.surface
                && self.panes.rect_of(crate::mouse::PaneId::Artifacts) == Some(drag.rect)
                && self
                    .module_host
                    .focused()
                    .is_some_and(|id| id.as_str() == "artifacts")
                && crate::mouse::point_in(drag.rect, ev.column, ev.row)
        });
        if !world_drag_valid
            || matches!(ev.kind, MouseEventKind::Up(_) | MouseEventKind::Down(_))
            || self.pending_approval.is_some()
            || self.loop_dialog.is_some()
            || self.agent_menu.is_some()
        {
            self.scryglass_drag = None;
        }
        if matches!(ev.kind, MouseEventKind::Up(_)) && self.viewer.inspector.drag.take().is_some() {
            return;
        }
        if self.pending_approval.is_some() {
            self.viewer.inspector.drag = None;
            return;
        }
        let (x, y) = (ev.column, ev.row);
        if self.loop_dialog.is_some() {
            self.viewer.inspector.drag = None;
            if matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
                self.loop_dialog_click(x, y);
            }
            return;
        }

        // The floating Brain Route deck owns the pointer while open. A click
        // outside closes it; wheel motion navigates without changing panes.
        if self.agent_menu.is_some() {
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

        // Shell pane: forward to the PTY when the program asked for the mouse.
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
                // Tracking program owns every mouse event over its pane —
                // never leak it into app selection/scroll.
                return;
            }
            // Not tracking → fall through; `PaneId::Shell` is registered
            // so the selection logic below picks it up.
        }

        if self.still_mouse(ev) {
            return;
        }
        match ev.kind {
            // Keep pane selection alive while scrolling. Mouse capture disables the
            // terminal's native scrollback selection, so the cockpit has to allow
            // the active pane to move under the app-owned highlight during copy.
            MouseEventKind::ScrollUp => {
                if matches!(
                    self.scryglass.surface,
                    crate::scryglass::StageSurface::Document(_)
                ) && self
                    .panes
                    .rect_of(mouse::PaneId::Artifacts)
                    .is_some_and(|rect| mouse::point_in(rect, x, y))
                {
                    self.scryglass.scroll_document(-3);
                    return;
                }
                if self.scryglass.surface == crate::scryglass::StageSurface::Research
                    && self
                        .panes
                        .rect_of(mouse::PaneId::Artifacts)
                        .is_some_and(|rect| mouse::point_in(rect, x, y))
                {
                    self.research.move_selection(-3);
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
                    crate::scryglass::StageSurface::Document(_)
                ) && self
                    .panes
                    .rect_of(mouse::PaneId::Artifacts)
                    .is_some_and(|rect| mouse::point_in(rect, x, y))
                {
                    self.scryglass.scroll_document(3);
                    return;
                }
                if self.scryglass.surface == crate::scryglass::StageSurface::Research
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
                        AgentButton::Model if self.thinking.is_none() && self.bg_job.is_none() => {
                            self.open_agent_menu(crate::agent_controls::AgentMenuKind::Model)
                        }
                        AgentButton::ReasoningEffort
                            if self.thinking.is_none() && self.bg_job.is_none() =>
                        {
                            self.open_agent_menu(crate::agent_controls::AgentMenuKind::Thinking);
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
                // Scryglass and Formation-deck controls swallow the click before
                // pane selection sees it.
                if let Some(&(_, btn)) = self
                    .world_buttons
                    .iter()
                    .find(|(r, _)| mouse::point_in(*r, x, y))
                {
                    self.apply_world_button(btn);
                    return;
                }
                if self.scryglass_camera_hit(x, y) {
                    self.focus_module("artifacts");
                    self.scryglass.follow_agent = false;
                    if let Some(rect) = self.panes.rect_of(mouse::PaneId::Artifacts) {
                        self.scryglass_drag = Some(crate::world_viz::world_camera::WorldDrag {
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
                // Only registered text viewports can anchor a selection. The
                // AgentBay viewport excludes its portrait and controls. A miss
                // clears any prior highlight instead of retaining a stale rect.
                self.selection = hit.and_then(|(pane, rect)| {
                    crate::surfaces::pane_accepts_clipboard(pane)
                        .then(|| mouse::Selection::new(pane, rect, x, y))
                });
                self.copy_requested = false;
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
                    // Never switch ownership to the pane under the cursor.
                    // Layout changes may shrink or remove the original viewport.
                    let rebound = crate::surfaces::pane_accepts_clipboard(sel.pane)
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
                    if crate::surfaces::pane_accepts_clipboard(pane) {
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
                // Keep the selection (stays highlighted); the next draw reads the
                // text out of the buffer and `flush_clipboard` copies it.
                self.copy_requested = true;
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.scryglass_drag = None;
                if let Some(selection) = self.selection.as_mut() {
                    selection.finish();
                }
                // A click with no drag is an empty selection. Don't leave it
                // anchored — it would render as a phantom reverse-video cell
                // (a stuck "white cell") that typing never clears.
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
            crate::scryglass::StageSurface::WorldFirstPerson
                | crate::scryglass::StageSurface::WorldMap
        ) && self.interaction_pane_at(x, y) == Some(crate::mouse::PaneId::Artifacts)
    }

    fn zoom_world_at(&mut self, _x: u16, _y: u16, inward: bool) {
        self.focus_pane_module(crate::mouse::PaneId::Artifacts);
        self.scryglass.adjust_fov(if inward { -0.05 } else { 0.05 });
    }

    fn scryglass_map_hit(&self, x: u16, y: u16) -> bool {
        self.scryglass.surface == crate::scryglass::StageSurface::WorldMap
            && self
                .panes
                .rect_of(crate::mouse::PaneId::Artifacts)
                .is_some_and(|rect| crate::mouse::point_in(rect, x, y))
    }

    fn enter_world_interior(&mut self) {
        if self.world.enter_interior() {
            self.reset_world_yaw();
            if matches!(
                self.scryglass.controller.overlay(),
                Some(crate::scryglass::StageOverlay::Arrival { .. })
            ) {
                self.scryglass.back_overlay();
            }
        }
    }

    fn study_selected_catalog(&mut self) {
        let shelf = self.scryglass.selected_shelf();
        let topic = shelf
            .arc
            .first()
            .copied()
            .unwrap_or(shelf.title)
            .to_string();
        if self.scryglass.begin_selected_lesson() {
            self.world.begin_teaching_lesson(&topic);
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
            crate::library::local_lesson_for_shelf(self.scryglass.selected_shelf().id)
        } else {
            crate::library::local_lesson(question).or_else(|| {
                crate::library::local_lesson_for_shelf(
                    crate::library::primary_shelf(crate::library::classify(question)).id,
                )
            })
        }
        .or_else(|| crate::library::local_lesson_for_shelf(self.scryglass.selected_shelf().id));
        lesson.map(|lesson| (lesson.tutor.name.to_string(), lesson.ask_tutor_prompt()))
            .unwrap_or_else(|| ("Tutor".into(), "Use a concrete example and check understanding. No reference has been retrieved.".into()))
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
            .map(crate::term_lookup::QuickLookup::source_url)
            .map(str::to_string)
        else {
            self.system_msg("Open a lesson before copying its source.".to_string());
            return;
        };
        self.pending_clipboard = Some(source);
    }

    /// A formation-board control was clicked. Editing or arming only changes the
    /// staged next/session roster; an in-flight turn owns its existing wrapper
    /// until completion.
    fn apply_world_button(&mut self, btn: WorldButton) {
        match btn {
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
                self.world.begin_teaching_lesson("open shelves");
                self.scryglass
                    .navigate(crate::scryglass::StageRoute::Explore(
                        crate::world_viz::Building::Scriptorium,
                    ));
                self.scryglass.open_catalog();
                self.focus_module("artifacts");
            }
            WorldButton::ScryglassVault => {
                self.scryglass.back_overlay();
                self.scryglass.navigate(crate::scryglass::StageRoute::Vault);
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
            WorldButton::ScryglassMap => {
                self.reset_world_yaw();
                self.scryglass.toggle_world_route(self.world.destination());
            }
            WorldButton::ScryglassEnter => {
                self.enter_world_interior();
            }
            WorldButton::ScryglassLeave => {
                if self.world.leave_interior() {
                    self.reset_world_yaw();
                }
            }
            WorldButton::ScryglassCatalog => {
                if self.world.interior_building() == Some(crate::world_viz::Building::Scriptorium) {
                    self.scryglass.open_catalog();
                }
            }
            WorldButton::ScryglassStudy => self.study_selected_catalog(),
            WorldButton::ScryglassAskTutor => self.draft_current_lesson_for_tutor(),
            WorldButton::ScryglassCopySource => self.copy_current_lesson_source(),
            WorldButton::ScryglassFollow => {
                if self.scryglass.controller.route() == crate::scryglass::StageRoute::Realm {
                    self.world.cycle_camera_zoom();
                } else {
                    self.scryglass.follow();
                }
            }
            WorldButton::ScryglassLandmark(building) => {
                self.reset_world_yaw();
                self.world.select_landmark(building);
                self.scryglass.return_to_world();
            }
            WorldButton::ScryglassVideoToggle => self.scryglass.toggle_video(),
            WorldButton::ScryglassVideoBack => self.scryglass.seek_video(-5),
            WorldButton::ScryglassVideoForward => self.scryglass.seek_video(5),
            WorldButton::Back => self.back_from_visual_surface(),
        }
    }

    /// Copy any text extracted by the last draw to the system clipboard. Called
    /// in the run loop *between* draws so the OSC-52 stdout write never races the
    /// ratatui backend.
    pub(crate) fn flush_clipboard(&mut self) {
        if let Some(text) = self.pending_clipboard.take()
            && !text.is_empty()
        {
            let status = deliver_to_clipboard(&text, "last-selection.txt");
            self.system_msg(status);
        }
        if let Some(selection) = self.pending_tutor_selection.take() {
            // Opening a second explanation must not replace an unsent question.
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
                // A2: knight walk to Scriptorium is Stage presentation only.
                if crate::surfaces::BackdropMode::from_env().paints_in_process() {
                    self.world.begin_teaching_lesson(&world_term);
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

    /// End-of-turn miniworld settlement. When `ANGEL_BACKDROP=off` the Stage is
    /// never painted — skip renown disk I/O / fireworks while still clearing
    /// agentviz + active tool work (A2). Hidden / unpainted Stage and Comp /
    /// lean are the same tax: chrome may stay, settlement fireworks must not.
    pub(crate) fn finish_world_turn(&mut self, ok: bool) {
        self.research.finish_turn();
        if crate::surfaces::BackdropMode::from_env().paints_in_process()
            && crate::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible)
        {
            self.world.turn_ended(ok);
        } else {
            self.world.turn_ended_hidden_stage(ok);
        }
    }

    fn vertical_composer_cursor(&self, delta: isize) -> usize {
        let width = self
            .panel_frames
            .get(crate::panels::PanelKind::Input)
            .map(|area| area.width.saturating_sub(2) as usize)
            .unwrap_or(80);
        crate::status_view::composer_vertical_cursor(&self.input, width, self.cursor, delta)
    }

    fn normalize_composer_cursor(&mut self) {
        // Normalize legacy/restored scalar offsets at the editor boundary. A
        // partial-cluster selection expands in the *source* as well as paint.
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

    /// Insert a char at the cursor (cursor-aware composer; cursor stays at the end
    /// in non-vim use, so this matches the old append behavior).
    fn input_insert(&mut self, c: char) {
        self.startup_intro
            .dismiss(std::time::Instant::now(), self.visual_motion);
        self.normalize_composer_cursor();
        self.delete_composer_selection();
        self.reset_composer_history_recall();
        // The ordinary composer is ASCII with its caret at the end. In that case
        // `cursor` (a char index) equals the byte length, so append without a
        // full Unicode count + byte-offset scan on every keystroke. A draft with
        // any multibyte scalar, or a caret in the middle, cannot satisfy this
        // equality and safely falls through to the cursor-aware path.
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

    /// Insert a paste in one buffer operation. The old per-character path
    /// repeatedly counted and searched the growing draft, making large pastes
    /// quadratic. Normalize terminal/clipboard CRLF at the display boundary
    /// while preserving intentional line breaks in the prompt.
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

    /// Delete the grapheme before the cursor.
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

    /// Capture and remove one exact character range. Empty kills preserve the
    /// previous buffer, matching shell kill-ring behavior. A range over the
    /// fixed memory budget is left untouched instead of becoming unrecoverable.
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

    /// Restore the latest exact kill at the caret. This is intentionally not
    /// routed through paste admission: the bytes already belonged to this
    /// composer and passed the stricter bounded kill capture.
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
        // A restored base can join a leading combining mark already in the
        // draft. Match paste admission: paint and the next edit must agree.
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

    /// Handle a key in `/vim` normal mode. Returns `true` if it was a vim key
    /// (so `on_key` stops); `false` lets non-vim keys (Tab/Enter/^G…) fall through.
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
                // `dd` (whole line) — simplified to clear; `D` deletes to end.
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
mod terminal_text_tests {
    use super::*;

    #[test]
    fn streamed_provider_text_strips_split_terminal_controls_and_bidi() {
        let mut sanitizer = TerminalTextSanitizer::default();
        assert_eq!(sanitizer.push("safe\u{1b}[3"), "safe");
        assert_eq!(sanitizer.push("1m red\u{1b}]0;host"), " red");
        assert_eq!(sanitizer.push("ile title\u{7} tail\u{202e}"), " tail");
        assert_eq!(sanitizer.push("plain"), "plain");
    }

    #[test]
    fn streamed_provider_text_normalizes_split_carriage_returns() {
        let mut sanitizer = TerminalTextSanitizer::default();
        assert_eq!(sanitizer.push("one\r"), "one");
        assert_eq!(sanitizer.push("\ntwo\rthree"), "\ntwo\nthree");
        assert_eq!(sanitizer.finish(), "");
        assert_eq!(sanitize_complete_terminal_text("tail\r"), "tail\n");
    }

    #[test]
    fn reset_releases_an_unterminated_sequence_for_the_next_turn() {
        let mut sanitizer = TerminalTextSanitizer::default();
        assert_eq!(sanitizer.push("\u{1b}]0;never-ended"), "");
        sanitizer.reset();
        assert_eq!(sanitizer.push("next answer"), "next answer");
    }
}

#[cfg(test)]
mod world_clock_tests {
    use super::*;

    #[test]
    fn world_tick_budget_preserves_wall_time_at_scenery_cadence() {
        assert_eq!(
            world_tick_budget(std::time::Duration::from_millis(24)),
            (0, false)
        );
        assert_eq!(
            world_tick_budget(std::time::Duration::from_millis(25)),
            (1, false)
        );
        assert_eq!(
            world_tick_budget(std::time::Duration::from_millis(83)),
            (3, false)
        );
        assert_eq!(
            world_tick_budget(std::time::Duration::from_millis(100)),
            (4, false)
        );
        assert_eq!(
            world_tick_budget(std::time::Duration::from_millis(200)),
            (8, false)
        );
    }

    #[test]
    fn world_tick_budget_drops_pathological_backlog() {
        assert_eq!(
            world_tick_budget(std::time::Duration::from_millis(224)),
            (8, false)
        );
        assert_eq!(
            world_tick_budget(std::time::Duration::from_millis(225)),
            (8, true)
        );
        assert_eq!(
            world_tick_budget(std::time::Duration::from_secs(60)),
            (8, true)
        );
    }
}

#[cfg(test)]
mod reflow_tests;

#[cfg(test)]
mod proc_completion_continuation_tests {
    use super::queue_proc_completion_for_loop;
    use crate::loop_ctl::{LoopState, LoopStatus};
    use std::path::Path;

    #[test]
    fn completion_wakes_only_authorized_matching_running_loop() {
        let workspace = Path::new("/solver/project-a");
        let mut state = LoopState {
            workspace: Some(workspace.to_path_buf()),
            status: LoopStatus::Running,
            max_iters: 4,
            iteration: 4, // wakeup must not replenish an exhausted budget
            token_budget: 100,
            tokens_spent: 100,
            ..Default::default()
        };
        assert!(!queue_proc_completion_for_loop(
            &mut state,
            Path::new("/solver/project-b"),
            "foreign".into()
        ));
        assert!(state.pending_proc_completions.is_empty());
        assert!(queue_proc_completion_for_loop(
            &mut state,
            workspace,
            "proof exited 2; not accepted".into()
        ));
        assert!(state.wake_at.is_some());
        assert_eq!(
            (
                state.max_iters,
                state.iteration,
                state.token_budget,
                state.tokens_spent
            ),
            (4, 4, 100, 100)
        );
        for status in [
            LoopStatus::Idle,
            LoopStatus::Paused,
            LoopStatus::Stopped,
            LoopStatus::Done,
            LoopStatus::Failed,
        ] {
            state.status = status;
            state.wake_at = None;
            assert!(!queue_proc_completion_for_loop(
                &mut state,
                workspace,
                "completion".into()
            ));
            assert!(state.wake_at.is_none());
        }
    }

    #[test]
    fn completion_context_is_bounded_and_does_not_collide_with_active_turn() {
        let workspace = Path::new("/solver/project");
        let mut state = LoopState {
            workspace: Some(workspace.into()),
            status: LoopStatus::Running,
            awaiting_turn: true,
            ..Default::default()
        };
        for n in 0..8 {
            assert!(queue_proc_completion_for_loop(
                &mut state,
                workspace,
                format!("job {n} exited")
            ));
        }
        assert!(!queue_proc_completion_for_loop(
            &mut state,
            workspace,
            "overflow".into()
        ));
        assert_eq!(state.pending_proc_completions.len(), 8);
        assert!(state.wake_at.is_none());
    }
}

#[cfg(test)]
mod reasoning_roll_grapheme_tests {
    use super::reasoning_reveal_boundary;
    use unicode_segmentation::UnicodeSegmentation;

    #[test]
    fn reasoning_roll_keeps_received_graphemes_complete() {
        for text in [
            "12345678Z",
            "1234567e\u{301}Z",
            "a👩‍💻Z",
            "abcd👍🏻Z",
            "1234🇺🇸Z",
            "\u{600}aZ",
            "क्‍षZ",
        ] {
            let mut shown = 0;
            while shown < text.len() {
                let next = reasoning_reveal_boundary(text, shown, 8);
                assert!(next > shown && next <= text.len());
                assert!(
                    next == text.len() || text.grapheme_indices(true).any(|(i, _)| i == next),
                    "{text:?} shown={shown} next={next}"
                );
                shown = next;
            }
        }
        assert_eq!(reasoning_reveal_boundary("", 0, 8), 0);
        assert_eq!(reasoning_reveal_boundary("12345678Z", 0, 8), 8);
    }

    #[test]
    fn reasoning_roll_handles_extensions_to_a_prior_chunk() {
        for (before, after) in [
            ("a👩", "a👩‍💻Z"),
            ("1234567e", "1234567e\u{301}Z"),
            ("abcd👍", "abcd👍🏻Z"),
            ("1234🇺", "1234🇺🇸Z"),
        ] {
            let next = reasoning_reveal_boundary(after, before.len(), 8);
            assert!(next == after.len() || after.grapheme_indices(true).any(|(i, _)| i == next));
        }
    }

    #[test]
    fn reasoning_roll_boundary_property_sweep() {
        let samples = [
            "".to_string(),
            "ascii".to_string(),
            "1234567e\u{301}Z".to_string(),
            "a👩‍💻Z".to_string(),
            "abcd👍🏻Z".to_string(),
            "1234🇺🇸Z".to_string(),
            "\u{600}aZ".to_string(),
            "क्‍षZ".to_string(),
            "🇺🇸".repeat(64),
            format!("e{}Z", "\u{301}".repeat(128)),
            "e\u{301}".repeat(64),
        ];
        for text in samples {
            let mut boundaries = text
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            boundaries.push(text.len());
            let mut previous_by_step = [0; 5];
            for shown in text
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(text.len()))
            {
                let mut previous = shown;
                for (index, step) in [0, 1, 2, 8, usize::MAX].into_iter().enumerate() {
                    let next = reasoning_reveal_boundary(&text, shown, step);
                    assert!(
                        shown <= next
                            && previous <= next
                            && previous_by_step[index] <= next
                            && next <= text.len(),
                        "shown={shown} step={step} next={next} text={text:?}"
                    );
                    assert!(
                        boundaries.contains(&next),
                        "partial grapheme: shown={shown} step={step} next={next} text={text:?}"
                    );
                    previous = next;
                    previous_by_step[index] = next;
                }
            }
        }
    }
}

#[cfg(test)]
mod r04c_cont2_measurements {
    #[test]
    fn r04c_cont2_original_final_sanitizer_cost() {
        let _guard = crate::tests::env_lock();
        let text = "ordinary prose with words and a newline.\n".repeat(160_000);
        if let Some(path) = std::env::var_os("ANGEL_FRAME_TIMING_LOG") {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            writeln!(
                file,
                "# fixture=original_final_sanitizer_only (not the complete frame loop)"
            )
            .unwrap();
        }
        let mut timing = crate::frame_timing::FrameTiming::from_env();
        let start = std::time::Instant::now();
        let sanitized = super::sanitize_complete_terminal_text(&text);
        let elapsed = start.elapsed();
        assert_eq!(sanitized, text);
        if let Some(timing) = &mut timing {
            let draw = std::time::Instant::now();
            timing.completed(
                draw,
                draw,
                crate::frame_timing::Phases {
                    advance_us: elapsed.as_micros(),
                    ..Default::default()
                },
            );
        }
        eprintln!(
            "original final sanitizer bytes={} advance_component_us={}",
            text.len(),
            elapsed.as_micros()
        );
    }
}
