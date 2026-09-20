//! `App` — the cockpit's central state: transcript, input, bag of clubs,
//! shells, panes, session, and the model-side methods that mutate it.
//! Behavior lives in sibling `impl App` extensions (app_control/, goal.rs,
//! loop_ctl.rs) and the draw layer reads it from draw.rs. Split out of
//! main.rs; fields are pub(crate) because App state is genuinely shared
//! across the module tree.

/// Process-local graceful exit intent. Failed saves remain held for export/retry;
/// a newly submitted task or command cancels the request explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExitRequest {
    WaitingForIdle,
    CheckpointFailed,
}

use super::*;

/// Temporary tutor composition; ordinary work returns on send or cancellation.
pub(crate) struct TutorDraft {
    pub(crate) name: String,
    pub(crate) context: String,
    pub(crate) saved_input: String,
    pub(crate) saved_cursor: usize,
    pub(crate) saved_selection: Option<usize>,
    pub(crate) saved_focus: Option<String>,
}

/// Read the repository's visible top-level directory names for the district
/// layer. Any filesystem error disables the layer rather than delaying startup.
pub(crate) fn scan_workspace_districts(root: &std::path::Path) -> Vec<String> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => return Vec::new(),
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => return Vec::new(),
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.')
            || matches!(
                name.as_str(),
                "target" | "node_modules" | "vendor" | "dist" | "build"
            )
        {
            continue;
        }
        names.push(name);
    }
    names.sort();
    names.truncate(12);
    names
}

fn active_profile_cache_matches(
    label: &str,
    agent: &str,
    driver: &str,
    model: Option<&str>,
) -> bool {
    if agent.eq_ignore_ascii_case(driver) && model.is_none() {
        return label == agent;
    }
    let model = model.unwrap_or("native");
    let Some(rest) = label.strip_prefix(agent) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(" ▸ ") else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(driver) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(" ▸ ") else {
        return false;
    };
    rest == model
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/app__district_scan_tests.rs"]
mod district_scan_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TranscriptMode {
    Conversation,
    Trace,
}

/// Layout inputs for the thinking-bay settled paragraph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ReasoningWrapKey {
    pub(crate) width: u16,
    pub(crate) streaming: bool,
    pub(crate) wrap_trim: bool,
    pub(crate) shown: usize,
    pub(crate) reasoning_len: usize,
    pub(crate) live_thinking: bool,
    pub(crate) bg_job: bool,
}

/// One exact content snapshot avoids wrapping unchanged text each frame.
/// Comparing every byte also detects same-length edits between the positions
/// that the former sampled fingerprint inspected.
#[derive(Clone, Debug, Default)]
pub(crate) struct ReasoningWrapMemo {
    pub(crate) key: Option<ReasoningWrapKey>,
    pub(crate) observed: String,
    pub(crate) lines: usize,
    pub(crate) rows: Vec<std::ops::Range<usize>>,
    pub(crate) viewport_height: usize,
    pub(crate) fresh_row: bool,
}

impl ReasoningWrapMemo {
    pub(crate) fn settled_lines(
        &mut self,
        key: ReasoningWrapKey,
        settled: &str,
        compute: impl FnOnce() -> usize,
    ) -> usize {
        if self.key == Some(key) && self.observed == settled {
            return self.lines;
        }
        let lines = compute();
        replace_wrap_snapshot(&mut self.observed, settled);
        self.key = Some(key);
        self.lines = lines;
        lines
    }
}

fn replace_wrap_snapshot(observed: &mut String, text: &str) {
    // Reuse modest allocations during streaming, but release a large prior
    // turn/lesson instead of retaining its high-water capacity indefinitely.
    if observed.capacity() > text.len().saturating_mul(4).max(64 * 1024) {
        *observed = text.to_owned();
    } else {
        observed.clear();
        observed.push_str(text);
    }
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/app__reasoning_wrap_tests.rs"]
mod reasoning_wrap_tests;

/// One exact world-lesson text snapshot, keyed by width and wrap behavior.
#[derive(Clone, Debug, Default)]
pub(crate) struct LessonWrapMemo {
    pub(crate) key: Option<(u16, bool)>,
    pub(crate) observed: String,
    pub(crate) lines: u16,
}

impl LessonWrapMemo {
    pub(crate) fn wrapped_lines(
        &mut self,
        width: u16,
        wrap_trim: bool,
        text: &str,
        compute: impl FnOnce() -> u16,
    ) -> u16 {
        let key = (width, wrap_trim);
        if self.key == Some(key) && self.observed == text {
            return self.lines;
        }
        let lines = compute();
        replace_wrap_snapshot(&mut self.observed, text);
        self.key = Some(key);
        self.lines = lines;
        lines
    }
}

/// A clickable control on the Scryglass world, Vault, or Formation deck.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorldButton {
    /// Open or operate the one canonical agent-formation deck.
    FormationDeck,
    SelectFormation(crate::formations::FormationId),
    SelectFormationSlot(usize),
    AssignFormationModel(usize),
    /// Open the THINK (effort) picker for formation seat `index`.
    StageFormationSeatThink(usize),
    /// Commit THINK picker row `index` (ladder rows, then the env-default clear row).
    StageFormationEffort(usize),
    ArmFormationTurn,
    ArmFormationSession,
    ClearFormation,
    /// Change only the evidence lens on the Realm/Reinforce miniviz.
    RlView(crate::rl_viz::RlView),
    Research(crate::research_workspace::Action),
    /// Scryglass stage navigation and camera controls.
    ScryglassWorld,
    ScryglassLibrary,
    ScryglassVault,
    #[allow(dead_code)]
    ScryglassPrev,
    #[allow(dead_code)]
    ScryglassNext,
    #[allow(dead_code)]
    ScryglassPin,
    Still(crate::still_inspector::Action),
    ScryglassMap,
    ScryglassEnter,
    ScryglassLeave,
    ScryglassCatalog,
    ScryglassStudy,
    ScryglassAskTutor,
    ScryglassCopySource,
    ScryglassFollow,
    #[allow(dead_code)]
    ScryglassLandmark(crate::world_viz::Building),
    ScryglassVideoToggle,
    ScryglassVideoBack,
    ScryglassVideoForward,
    /// Leave the active visual/game surface and return focus to the message pane.
    Back,
}

/// A clickable agent control in either the identity panel or compact header rail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentButton {
    /// Open the concrete model-route deck.
    Model,
    /// Open the selected model's supported reasoning levels.
    ReasoningEffort,
    /// Open the SOTA-MoA formation roster board.
    MoaDeck,
    /// Explicitly label the last completed answer useful.
    RateUseful,
    /// Explicitly label the last completed answer a miss.
    RateMiss,
}

#[derive(Clone, Debug)]
pub(crate) struct LastCompletedRoute {
    pub(crate) route: crate::club::RouteIdentity,
    pub(crate) completed_ms: u64,
    pub(crate) verdict: Option<crate::experience::RouteVerdict>,
}

#[derive(Clone, Debug)]
pub(crate) struct BrainRouteReceipt {
    /// Human-readable exact route committed by the selector, including effort.
    pub(crate) label: String,
    pub(crate) applied_at: Instant,
}

/// Cached MODEL/THINK/FORMATION chip strings. The rail asks every frame;
/// an unchanged snapshot skips format!/truncate.
#[derive(Clone, Debug)]
pub(crate) struct AgentControlChipCache {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) busy: bool,
    pub(crate) route_just_set: bool,
    pub(crate) fallback_armed: bool,
    pub(crate) effort_selectable: bool,
    pub(crate) mode: Option<String>,
    pub(crate) effort: Option<String>,
    pub(crate) route: String,
    pub(crate) has_route_choices: bool,
    pub(crate) moa_scope: u8,
    pub(crate) moa_formation: u64,
    pub(crate) stacked: bool,
    pub(crate) include_think: bool,
    pub(crate) include_moa: bool,
    pub(crate) model_text: String,
    pub(crate) think_text: String,
    pub(crate) moa_text: String,
}

pub(crate) struct UndoneExchange {
    pub(crate) anchor_sha256: String,
    pub(crate) messages: Vec<ChatMsg>,
}

/// A submitted turn waiting for its echo frame. `submit` stashes the parsed
/// turn here and returns immediately so the operator's message paints on the
/// very next draw; the heavy pre-flight (goal reload, formation engage,
/// context blocks, atlas lens, broker selection, session save) runs in
/// `advance` one frame later. `echo_drawn` is stamped by the draw loop.
/// `retry_draft` is the composer text restored when launch aborts after the
/// echo (stale formation) — Esc-while-parked still restores [`Self::raw`].
pub(crate) struct PendingTurn {
    pub(crate) raw: Arc<str>,
    pub(crate) user_msg: ChatMsg,
    pub(crate) turn_evidence: Option<ChatMsg>,
    pub(crate) echo_drawn: bool,
    pub(crate) retry_draft: Arc<str>,
    /// How many trailing attachments came from the composer's staged screenshots.
    /// A canceled turn restores exactly those; earlier ones came from `/see` and
    /// its restored draft text reconstructs them on resubmit.
    pub(crate) clipboard_images: usize,
}

/// One confined path-completion snapshot, valid only for an uninterrupted Tab
/// sequence. Matches are already bounded by the completion admission path.
pub(crate) struct WorkspaceCompletionCache {
    pub(crate) parent: String,
    pub(crate) query: String,
    pub(crate) matches: Vec<String>,
}

/// One completed turn's bounded Atlas payload. Completion owns the visible
/// frame first; the next UI tick queues the durable snapshot write.
pub(crate) struct PendingAtlasHarvest {
    pub(crate) atlas: Arc<crate::atlas::AtlasService>,
    pub(crate) task: String,
    pub(crate) final_answer: String,
    pub(crate) source_ids: Vec<String>,
    pub(crate) verifier_receipts: Vec<String>,
}

/// Serial local persistence worker. Closing the app drains accepted harvests.
/// The tick after completion waits at most `ATLAS_ACK_WAIT` for the durable
/// acknowledgement — long enough for an ordinary harvest to land on that tick
/// (the one-tick contract), short enough to keep the post-turn tick inside the
/// R04 interaction floor (150 ms) when the answer is large. Slow writes remain
/// queued, are never cancelled, and their acknowledgement is collected on later
/// ticks by `error()`.
pub(crate) struct AtlasHarvester {
    sender: Option<mpsc::Sender<(PendingAtlasHarvest, mpsc::Sender<()>)>>,
    errors: mpsc::Receiver<String>,
    worker: Option<std::thread::JoinHandle<()>>,
    /// Acknowledgements still outstanding after their bounded wait.
    outstanding: std::cell::RefCell<Vec<mpsc::Receiver<()>>>,
}

/// Bounded UI wait for one harvest acknowledgement (see `AtlasHarvester`).
const ATLAS_ACK_WAIT: std::time::Duration = std::time::Duration::from_millis(100);

impl AtlasHarvester {
    pub(crate) fn new() -> std::io::Result<Self> {
        let (sender, jobs) = mpsc::channel::<(PendingAtlasHarvest, mpsc::Sender<()>)>();
        let (errors, received_errors) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("atlas-harvest-write".into())
            .spawn(move || {
                for (pending, completed) in jobs {
                    if let Err(error) = pending.atlas.enqueue_harvest(
                        &pending.task,
                        &pending.final_answer,
                        &pending.source_ids,
                        &pending.verifier_receipts,
                    ) {
                        let _ = errors.send(error);
                    }
                    let _ = completed.send(());
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            errors: received_errors,
            worker: Some(worker),
            outstanding: std::cell::RefCell::new(Vec::new()),
        })
    }

    pub(crate) fn enqueue(&self, pending: PendingAtlasHarvest) -> Result<(), String> {
        let (completed, completion) = mpsc::channel();
        self.sender
            .as_ref()
            .ok_or("Atlas harvest writer closed")?
            .send((pending, completed))
            .map_err(|_| "Atlas harvest writer disconnected".to_string())?;
        match completion.recv_timeout(ATLAS_ACK_WAIT) {
            Ok(()) => Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Large harvest: the write continues on the worker; the tick
                // moves on and `error()` collects the acknowledgement later.
                self.outstanding.borrow_mut().push(completion);
                Ok(())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("Atlas harvest writer disconnected before acknowledgement".into())
            }
        }
    }

    /// Number of harvests whose durable acknowledgement has not arrived yet.
    #[cfg(test)]
    pub(crate) fn outstanding(&self) -> usize {
        self.collect_acks();
        self.outstanding.borrow().len()
    }

    fn collect_acks(&self) {
        self.outstanding.borrow_mut().retain(|ack| {
            !matches!(
                ack.try_recv(),
                Ok(()) | Err(mpsc::TryRecvError::Disconnected)
            )
        });
    }

    pub(crate) fn error(&self) -> Option<String> {
        self.collect_acks();
        self.errors.try_recv().ok()
    }
}

impl Drop for AtlasHarvester {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(crate) struct App {
    pub(crate) messages: Vec<Message>,
    pub(crate) input: String,
    pub(crate) startup_intro: crate::startup_intro::StartupIntro,
    pub(crate) tutor_draft: Option<TutorDraft>,
    pub(crate) explain_requested: bool,
    pub(crate) pending_tutor_selection: Option<String>,
    pub(crate) bag: Bag,
    /// Model-facing conversation (system + user/assistant/tool turns).
    pub(crate) history: Vec<ChatMsg>,
    /// One conversation-only exchange that `/redo` may restore while the
    /// post-undo history remains byte-identical.
    pub(crate) undone_exchange: Option<UndoneExchange>,
    pub(crate) tools: Arc<harness::ToolRegistry>,
    /// Living Atlas service shared with the active registry's deferred tool.
    /// It is repository-bound and replaced together with `tools` on `/cd`.
    pub(crate) atlas: Arc<crate::atlas::AtlasService>,
    pub(crate) pending_atlas_harvest: Option<PendingAtlasHarvest>,
    pub(crate) atlas_harvester: Option<AtlasHarvester>,
    pub(crate) atlas_view: crate::atlas::AtlasViewState,
    /// Shared request broker for the interactive agent's read-only UI view.
    pub(crate) ui_broker: Arc<crate::ui_inspect::UiSnapshotBroker>,
    pub(crate) viewer: Viewer,
    /// One-flight surface-free WebGPU producer. It is active only when the
    /// calibrated Viewer selected Kitty and a renderer binary is available.
    pub(crate) agentviz_portal: crate::agentviz_portal::PortalRuntime,
    /// Embedded full-access shell (lazily spawned on first ^G).
    pub(crate) shell: Option<ShellPane>,
    pub(crate) shell_focused: bool,
    /// Whether the host terminal currently reports keyboard focus. Completed
    /// foreground work requests one coalesced BEL only while this is false.
    pub(crate) terminal_focused: bool,
    pub(crate) attention_requested: bool,
    pub(crate) thinking: Option<Thinking>,
    /// A turn submitted but not yet launched: the echo frame draws first, the
    /// heavy pre-flight follows on the next `advance`. Holds the flight slot —
    /// `submit`'s busy gate treats it exactly like `thinking`.
    pub(crate) pending_turn: Option<PendingTurn>,
    /// Set by the interactive event loop. When false (tests, headless), submit
    /// launches its turn synchronously through the same pending-turn path.
    pub(crate) submit_deferral: bool,
    /// Last wall-clock world tick — `advance` paces the miniworld to ~40fps so
    /// keystroke bursts don't advance (and re-render) the scenery per event.
    pub(crate) world_ticked_at: Instant,
    /// Mid-run steering: messages typed while a turn/loop holds the flight
    /// slot, queued for the worker to inject at its next hop boundary (see
    /// `steer.rs`). One session-long queue, shared with every spawned worker,
    /// so a steer that misses one turn is picked up by the next (or flushed as
    /// a follow-up turn when the slot goes idle).
    pub(crate) steer_queue: Arc<steer::SteerQueue>,
    /// A background job (e.g. `/compact` distilling, `/memories palace` querying)
    /// running off the UI thread, drained in `advance`. Mutually exclusive with
    /// `thinking` — `submit` is gated on both — so history can't shift under it.
    pub(crate) bg_job: Option<app_control::BackgroundJob>,
    /// The latest terminal background transition's already-bounded,
    /// terminal-safe output. This makes `/copy live` useful immediately after
    /// completion/cancellation without retaining an unbounded process log.
    pub(crate) last_background_output: Option<Arc<str>>,
    /// Trusted static operation label paired with `last_background_output`.
    pub(crate) last_background_operation: Option<&'static str>,
    /// Detached work (currently post-compaction memory filing) reports bounded
    /// completion feedback here after releasing the primary flight slot.
    pub(crate) background_notice_tx: mpsc::Sender<app_control::BackgroundNotice>,
    pub(crate) background_notice_rx: mpsc::Receiver<app_control::BackgroundNotice>,
    /// Assistant text streamed so far for the in-flight turn — rendered live
    /// below the transcript with a cursor, then cleared when the turn ends.
    pub(crate) partial: String,
    /// Stateful display-only filter for provider prose. Model history keeps the
    /// exact bytes; terminal controls and bidi formatters never reach layout.
    pub(crate) stream_text_sanitizer: app_control::TerminalTextSanitizer,
    pub(crate) partial_height_cache: transcript::PartialHeightCache,
    pub(crate) stream_artifact_detector: crate::app_control::StreamArtifactDetector,
    /// A large generated code document is currently streaming; suppress further
    /// document text in chat and replace the final answer with an artifact card.
    pub(crate) stream_artifact_notice: bool,
    /// Transcript index of the draft retained by an error path (display only).
    /// When the follow-up turn commits a real answer, the stale draft collapses
    /// to a stub so the transcript never shows the same answer twice.
    pub(crate) retained_partial_msg: Option<usize>,
    /// Transcript index of the most recent streamed draft that `flush_partial`
    /// committed mid-turn (a notice or tool boundary landed after the prose).
    /// The turn's final reply reuses that block when it begins with the same
    /// text, so one answer never renders twice.
    pub(crate) flushed_partial_msg: Option<usize>,
    /// The agent's private reasoning streamed this turn (`reasoning_content` from
    /// reasoning models) — shown live in the canvas pane, never saved to history.
    /// Cleared at the start of each turn; persists after so the last think is readable.
    pub(crate) reasoning: String,
    pub(crate) reasoning_text_sanitizer: app_control::TerminalTextSanitizer,
    /// Bytes of `reasoning` rolled into the avatar canvas so far. The reasoning
    /// FEED arrives at full speed (never delayed); this is a display-only cursor
    /// that rolls the cached text into the avatar pane slowly. After the turn
    /// ends the roll keeps ticking until the whole think is on screen — the
    /// reply itself is never held up by it.
    pub(crate) reasoning_shown: usize,
    /// Agent thinking-pane scrollback offset in wrapped rows from the bottom.
    /// Mouse wheel and focused-pane keys use this so private reasoning can be
    /// inspected and copied instead of being pinned to the newest line forever.
    pub(crate) reasoning_scroll: usize,
    /// Per-app wrap-count memo for the thinking bay. Not process-wide: tests
    /// share a process and must not inherit another case's line count.
    pub(crate) reasoning_wrap: ReasoningWrapMemo,
    /// Per-app wrap-count memo for the Scryglass world lesson. Not process-wide.
    pub(crate) lesson_wrap: LessonWrapMemo,
    /// Chat scrollback offset in wrapped rows from the bottom (0 = newest).
    pub(crate) scroll: u16,
    /// Cached per-message visual heights (wrapped rows) at `transcript_heights_w`.
    /// `messages` is append-only, so this is synced incrementally (wrap each new
    /// message once) and lets `render_transcript` window to the viewport instead
    /// of re-wrapping the whole unbounded history every frame.
    pub(crate) transcript_heights: Vec<u16>,
    /// Cumulative row offsets for `transcript_heights`, always beginning with
    /// zero and containing one extra entry. This makes total-height lookup O(1)
    /// and visible-message lookup logarithmic in long sessions.
    pub(crate) transcript_height_prefix: Vec<u32>,
    /// Published inner width; resize work builds a complete replacement separately.
    pub(crate) transcript_heights_w: u16,
    /// One unpublished resize index, canceled by non-append content mutations.
    pub(crate) pending_transcript_reflow: Option<transcript::reflow::PendingReflow>,
    /// One bounded worker and viewport cache for oversized transcript messages.
    pub(crate) transcript_layouts: transcript::oversized::Layouts,
    /// Bottom-most valid transcript origin from the preceding draw. Together
    /// with `transcript_anchor_w`, this keeps a manually scrolled reading
    /// position fixed while a live partial gains wrapped rows.
    pub(crate) transcript_last_bottom: u16,
    /// Text viewport width paired with `transcript_last_bottom`. A width change
    /// invalidates the row anchor because wrapping has changed.
    pub(crate) transcript_anchor_w: u16,
    /// Per-message roll-in spawn times (seconds since `started`), parallel to
    /// `messages` and synced lazily by the transcript draw. `NEG_INFINITY`
    /// marks a block settled: it renders in place and never animates.
    pub(crate) transcript_spawns: Vec<f32>,
    /// True while a transcript block is still rolling in (set each draw);
    /// joins the animation-tick predicate so the slide gets frames.
    pub(crate) transcript_rolling: bool,
    pub(crate) divider_motion: crate::pane_motion::Divider,
    /// Per-turn tool activity for the live strip at the bottom of the agent
    /// shell (see `toolstrip`). Replaces the old per-call T:/R: transcript
    /// spam with a compact status row + lateral loading bar.
    pub(crate) tool_strip: toolstrip::ToolStrip,
    /// `/trace`: flip the old full T:/R: tool trace back on for debugging.
    pub(crate) transcript_mode: TranscriptMode,
    pub(crate) receipt_run_open: bool,
    pub(crate) turn_renown_started: u64,
    pub(crate) turn_truncated: bool,
    pub(crate) turn_route_receipt: Option<String>,
    /// Milliseconds from worker spawn to the first operator-visible event in
    /// the active turn. Kept separately from total turn duration so route
    /// receipts expose responsiveness instead of only completion latency.
    pub(crate) turn_first_output_ms: Option<u64>,
    /// Six-state portrait: the short-lived Victory/Recovery marker set where a
    /// turn settles, with its set time. The bay expires it by age at render
    /// (see `agent_view::PORTRAIT_MARKER_LIFETIME`); nothing has to clear it.
    pub(crate) portrait_turn_marker: Option<(crate::agent_view::PortraitMarker, Instant)>,
    /// Consecutive failed turn settlements, tracked from turn facts at the two
    /// `turn_ended` sites — never read back from world state. A success after
    /// a run of >= 2 marks the portrait Recovery instead of Victory.
    pub(crate) portrait_fail_run: u32,
    /// Session prompt-cache hit accounting, folded from per-turn provider
    /// deltas as each worker completes (see `turn::CacheMeter`). Shown by
    /// `/usage`; stays "n/a" for backends that report no cache fields.
    pub(crate) cache_meter: crate::turn::CacheMeter,
    /// The session goal for a long-running task (`/goal`); shown in `/status`.
    pub(crate) goal: Option<crate::goal::Goal>,
    /// Project-bound proof-campaign authority. Its explicit advance command
    /// hands one frozen round to the internal swarm-compiler service.
    pub(crate) campaign: crate::campaign::CampaignController,
    /// Operator-authorized campaign work occupying the same one-flight
    /// boundary as an interactive turn.
    pub(crate) campaign_pending: Option<crate::app_control::CampaignPending>,
    /// The autonomous `/loop` controller state (persisted; see `loop_ctl.rs`).
    pub(crate) loop_ctl: crate::loop_ctl::LoopState,
    /// Live competition submission-slot state projected by the harness watcher
    /// into the loop trench HUD. Display-only; never persisted or model-facing.
    pub(crate) submission_slot: crate::harness::SubmissionSlotTelemetry,
    /// Personal submission telemetry across Yukon's currently open
    /// competitions. Polling is one-shot, off-thread, and competition-only.
    pub(crate) yukon_fleet: crate::harness::comp_packages::yukon::fleet::YukonFleetState,
    pub(crate) yukon_fleet_rx:
        Option<mpsc::Receiver<Result<crate::harness::comp_packages::yukon::fleet::YukonFleetSnapshot, String>>>,
    pub(crate) yukon_fleet_polled_at: Instant,
    /// Off-thread work the loop is awaiting (acceptance command / SOTA approval).
    pub(crate) loop_pending: Option<crate::loop_ctl::LoopPending>,
    /// Independent deep experiment; its receiver owns cancellation settlement.
    pub(crate) loop_experiment: Option<crate::loop_ctl::ExperimentPending>,
    /// Last-seen loop facts diffed each frame into `AdventureEvent`s for the
    /// world quest (`adventure.rs`). Drains whether or not the world pane is
    /// visible — quest state must track the loop like the village pulses.
    pub(crate) loop_mirror: world_viz::LoopMirror,
    /// In-world loop launcher: an RPG-style workshop dialog shown over the map.
    pub(crate) loop_dialog: Option<crate::loop_dialog::LoopLaunchDialog>,
    /// Clickable regions from the last loop dialog draw.
    pub(crate) loop_dialog_hits: Vec<crate::loop_dialog::LoopDialogHit>,
    /// Optional display name for the thread (`/rename`); shown in `/status`.
    pub(crate) session_title: Option<String>,
    /// Plan-mode (`/plan`): prepend a "plan before acting" steer to each message.
    pub(crate) plan_mode: bool,
    /// Communication style (`/personality`): prepended as a steer to each message.
    pub(crate) personality: Option<String>,
    /// Relentless execution latch: prepended until a useful answer is delivered.
    pub(crate) relentless_execution: bool,
    /// Retard mode: plain-language, bug-free / easy-to-use communication steer.
    pub(crate) retard_mode: bool,
    /// Solo mode: in-hand agent owns the workload — no paid SOTA outsourcing.
    pub(crate) solo_mode: bool,
    /// Persistent memories (`/memories`) injected into every agent turn.
    /// Shared with `KnowledgeCandidate.content` on the launch tick.
    pub(crate) memories: Vec<Arc<str>>,
    /// Parked main/parent threads while inside `/btw` side-conversations (a stack,
    /// so side-threads can nest). Each entry is a saved `history`.
    pub(crate) parked_threads: Vec<Vec<ChatMsg>>,
    /// Custom header status item (`/statusline <text>`).
    pub(crate) statusline: Option<String>,
    /// A header pet (`/pet <name>`), rendered after the status item.
    pub(crate) pet: Option<String>,
    /// `/moa` formation roster/graph panel state.
    pub(crate) moa_deck: Option<crate::formations::MoaDeckState>,
    /// One amplified SOTA-MoA formation + exact model roster for the next turn.
    pub(crate) moa_one_shot: Option<crate::formations::MoaEngagement>,
    /// Session-long formation roster; every user turn is prefixed with `moa:`.
    pub(crate) moa_session: Option<crate::formations::MoaEngagement>,
    /// Concrete route that owned the cockpit before the first active MoA
    /// wrapper. It remains the session's return point until MoA is cleared.
    pub(crate) moa_restore_route: Option<(usize, usize)>,
    /// Restore the base route (or a newly staged session roster) when the
    /// current worker releases its cloned MoA wrapper.
    pub(crate) moa_restore_after_turn: bool,
    /// Terminal window title override (`/title <text>`), applied by the run loop.
    pub(crate) title_override: Option<String>,
    /// Custom key→action overrides (`/keymap`); checked before the default keys.
    pub(crate) keybinds: Vec<app_control::Keybind>,
    /// `/vim` modal composer: enabled, in-normal-mode, and the char cursor index.
    pub(crate) vim_mode: bool,
    pub(crate) vim_normal: bool,
    pub(crate) cursor: usize,
    /// Latest shell-style composer kill (Ctrl-W/U/K), retained locally for
    /// exact Ctrl-Y recovery without touching the system clipboard.
    pub(crate) composer_kill_buffer: Option<String>,
    /// Ctrl-P/Ctrl-N navigation over eligible persisted operator prompts.
    /// The scratch draft is restored exactly when navigation returns to newest.
    pub(crate) composer_history_index: Option<usize>,
    pub(crate) composer_history_draft: Option<String>,
    pub(crate) composer_history_query: Option<String>,
    /// Ctrl-V clipboard ingress: at most one read in flight plus every screenshot
    /// already admitted and staged for the next submitted turn. Display-only until
    /// that submit; never written to a file.
    pub(crate) clipboard_paste: crate::clipboard::ClipboardPaste,
    /// Keyboard-authored prompt selection anchor; `cursor` is the moving end.
    /// Character indices keep range extraction UTF-8 safe.
    pub(crate) composer_selection_anchor: Option<usize>,
    /// One fresh admitted-name scan reused only across a consecutive Tab
    /// completion sequence. Any non-Tab composer action drops it, so catalog
    /// edits become visible on the next completion without rescanning roots for
    /// every ambiguous candidate step.
    pub(crate) skill_completion_catalog: Option<Vec<String>>,
    /// One bounded directory-query snapshot reused while Tab only extends the
    /// same parent/prefix. Any real key or paste invalidates it, so filesystem
    /// changes appear on the next edited completion interaction.
    pub(crate) workspace_completion_cache: Option<WorkspaceCompletionCache>,
    /// Rich-media carousel: cards surfaced this session (images, links, graphs).
    pub(crate) media: Vec<Media>,
    /// Carousel scroll offset — index of the first visible card.
    pub(crate) media_scroll: usize,
    /// Read-only campaign/report ledger shown inside the native artifacts pane.
    pub(crate) observatory: crate::observatory::ObservatoryState,
    /// Responsive Quest Board source. The trace is stored verbatim and rendered
    /// to the current Stage width; fixed-width transcript art is never cached.
    pub(crate) quest_stage: Option<QuestStage>,
    /// Braille-only world/media stage layered over the artifacts module.
    /// Session visibility override, independent of the selected world renderer.
    pub(crate) scryglass_enabled: bool,
    pub(crate) scryglass: crate::scryglass::Scryglass,
    /// `/rl`: the Realm/Reinforce stage — live RL pipeline state graph plus
    /// telemetry spine.
    /// Selected evidence lens inside the Reinforce miniviz. This is display
    /// state only; switching lenses never changes or launches an RL run.
    pub(crate) rl_view: crate::rl_viz::RlView,
    pub(crate) research: crate::research_workspace::Workspace,
    pub(crate) agent_graph: crate::graph_ctl::GraphState,
    /// `/handoff-rl`: fresh-context handoff RL competition loop controller.
    pub(crate) handoff_rl: crate::handoff_rl::HandoffRlState,
    /// The miniworld: the artifacts pane's default content — a procedurally
    /// generated island town driven by the agent's real tool traffic
    /// (see `world_viz`). Display-only.
    pub(crate) world: world_viz::World,
    /// Stamped by each draw pass: the miniworld actually rendered this frame.
    /// An animating world in a hidden pane (artifacts closed, narrow terminal,
    /// a fullscreen overlay) must not hold the event loop on the fast tick —
    /// nothing it animates is visible.
    pub(crate) world_pane_visible: bool,
    /// Edge latch for the automatic Realm→Quintain presentation. Operator
    /// routes are never displaced, and Back cannot reopen the same visit.
    pub(crate) quintain_route_presented: bool,
    /// Pulses from the village's single background sensing thread (forge +
    /// head health, rare chatter), drained in `advance`. `None` when the
    /// village is off (`ANGEL_VILLAGE=0`) or under test.
    pub(crate) village_rx: Option<mpsc::Receiver<village::VillagePulse>>,
    /// War-room button hitboxes recorded by the last draw (absolute buffer
    /// rects): formation presets + the quorum toggle on the miniworld pane.
    /// Cleared whenever the pane doesn't render them.
    pub(crate) world_buttons: Vec<(Rect, WorldButton)>,
    /// Agent-panel button hitboxes recorded by the last draw.
    pub(crate) agent_buttons: Vec<(Rect, AgentButton)>,
    /// Floating selector opened by MODEL or THINK, plus its last-drawn geometry.
    pub(crate) agent_menu: Option<crate::agent_controls::AgentControlMenu>,
    /// `Some` while `/` filter mode owns printable keys; empty means the filter
    /// prompt is active but still matches every row.
    pub(crate) agent_menu_search: Option<String>,
    pub(crate) agent_menu_details: bool,
    pub(crate) agent_menu_show_unavailable: bool,
    pub(crate) agent_menu_hits: crate::agent_controls::AgentMenuHits,
    pub(crate) agent_menu_area: Option<Rect>,
    pub(crate) agent_control_area: Option<Rect>,
    /// Centered portrait plate on the roomy header bar, when that layout owns it.
    pub(crate) header_portrait_area: Option<Rect>,
    /// Header data card rect (roomy layout): identity + telemetry live here,
    /// so the agent bay is the thinking box with the portrait in its corner.
    pub(crate) header_card_area: Option<Rect>,
    /// Portrait plate inside the agent bay (card layout), for hit/tests.
    pub(crate) bay_portrait_area: Option<Rect>,
    /// Header-side agent telemetry bay, when the roomy split layout owns it.
    pub(crate) agent_info_area: Option<Rect>,
    /// Bounded operational history loaded lazily when the Brain Route deck opens.
    pub(crate) route_evidence: crate::route_intelligence::RouteEvidenceSnapshot,
    pub(crate) route_evidence_loaded_at: Option<Instant>,
    /// Exact backend that produced the last user-visible completed answer.
    pub(crate) last_completed_route: Option<LastCompletedRoute>,
    /// Bounded attribution receipt for the most recently settled turn.
    pub(crate) last_turn_outcome: Option<crate::backplane::TurnOutcome>,
    /// Brief, non-transcript confirmation after MODEL/THINK commits a route.
    pub(crate) brain_route_receipt: Option<BrainRouteReceipt>,
    /// Short state-change ceremony that borrows the Realm Stage after goal or
    /// loop lifecycle commands. This is display-only; backend work is still
    /// driven by `/loop` and normal turns.
    pub(crate) lifecycle_ceremony: Option<LifecycleCeremony>,
    /// Brief display-only acknowledgement of a whole-million metered input
    /// milestone. It never changes turn, loop, transcript, or trace state.
    pub(crate) spend_coin: Option<SpendCoin>,
    /// Display-only first-person look offset while settled at a landmark.
    pub(crate) world_yaw_offset: f32,
    /// Explicitly sampled visual motion policy. Renderers receive this as an
    /// input; they never read environment or mutate operational state.
    pub(crate) visual_motion: lifecycle_viz::MotionMode,
    pub(crate) image_title_label: Option<String>,
    pub(crate) image_title_cache: String,
    /// Last drawable PTY cell area. The shell is resized from the same rect it
    /// renders into, so terminal resizes cannot drift from layout math.
    pub(crate) shell_area: Option<Rect>,
    /// Content rects of the selectable panes, rebuilt every draw in `ui()` and
    /// used to hit-test the mouse so a drag-select is confined to one pane.
    pub(crate) panes: mouse::PaneRegistry,
    /// Outer (framed) rects of every cockpit panel, rebuilt every draw in `ui()`.
    /// Parallel to `panes` (which tracks chrome-free content rects), this tracks
    /// the bordered boxes themselves for border repair and layout inspection.
    pub(crate) panel_frames: panels::PanelFrames,
    /// The active freeform mouse selection (confined to a single pane), or none.
    pub(crate) selection: Option<mouse::Selection>,
    /// Last pointer cell while the operator is dragging the Scryglass camera.
    /// This is display-only and never changes the agent-driven world position.
    pub(crate) scryglass_drag: Option<crate::world_viz::world_camera::WorldDrag>,
    /// Set on mouse-up: the next draw extracts the selected text from the
    /// rendered buffer into `pending_clipboard`.
    pub(crate) copy_requested: bool,
    /// Text extracted from a finished selection, awaiting the clipboard write in
    /// the run loop (kept out of `ui()` so the stdout write never races a draw).
    pub(crate) pending_clipboard: Option<String>,
    /// One-shot full backend invalidation requested by `/redraw` or Ctrl-L.
    /// The run loop consumes it immediately before the next draw.
    pub(crate) redraw_requested: bool,
    /// Deferred local glossary lookup. Ordinary copy never queues this; the
    /// run loop starts the lookup outside rendering.
    pub(crate) pending_quick_lookup: Option<String>,
    pub(crate) last_root_area: Option<Rect>,
    pub(crate) last_resize_at: Instant,
    /// Idle bay-title cache: `(tabs ptr, max_chars, treebeard, title)`.
    /// Live turns skip it because the clock changes every second.
    pub(crate) idle_route_title: Option<(usize, usize, bool, String)>,
    /// MODEL/THINK/FORMATION chip strings for the last unchanged rail.
    pub(crate) agent_control_chips: Option<AgentControlChipCache>,
    pub(crate) overwatch: Overwatch,
    pub(crate) should_quit: bool,
    pub(crate) exit_request: Option<ExitRequest>,
    /// Persists the thread to disk every turn so a cutoff is recoverable.
    pub(crate) session: session::Session,
    pub(crate) session_warning_notified: bool,
    pub(crate) started: Instant,
    /// Pending human-approval request from the pipeline (e.g. the swarm wanting
    /// to phone a SOTA). While `Some`, a modal is shown and keys answer it.
    pub(crate) pending_approval: Option<PendingApproval>,
    /// Inbox for approval requests emitted by worker threads, drained in `advance`.
    pub(crate) approval_rx: mpsc::Receiver<approval::Request>,
    pub(crate) apollo_specialist_present: Cell<bool>,
    pub(crate) specialist_message_scan_len: Cell<usize>,
    pub(crate) specialist_media_scan_len: Cell<usize>,
    pub(crate) active_profile_label: String,
    pub(crate) active_profile_specialist: bool,
    pub(crate) active_profile_cache: Option<AgentProfile>,
    /// One-shot context injected on the next turn after `/cd` changes the working
    /// directory — the new root's project docs (AGENTS.md), prepended to the
    /// outgoing user message (backend-safe, unlike a second system message) and
    /// cleared once consumed.
    /// `/self reborn` — the off-thread `cargo build` of the live crate, drained
    /// in `advance`; on success `reborn_exec` is staged and the run loop exits.
    pub(crate) reborn_rx: Option<mpsc::Receiver<Result<std::path::PathBuf, String>>>,
    /// The freshly-built binary `main` execs into after terminal teardown (the
    /// phoenix step), resuming the current session.
    pub(crate) reborn_exec: Option<std::path::PathBuf>,
    /// Presentation/module lifecycle for the composited Rust cockpit host.
    pub(crate) module_host: runtime::ModuleHost,
}

/// An approval awaiting the user's keypress; `reply` sends the decision back to
/// the blocked worker.
pub(crate) struct PendingApproval {
    pub(crate) prompt: String,
    /// Canonical label derived from the broker's typed cache key. Direct UI
    /// ceremonies without broker scope leave this absent.
    pub(crate) scope_label: Option<String>,
    pub(crate) reply: mpsc::Sender<approval::Decision>,
}

pub(crate) struct LifecycleCeremony {
    pub(crate) kind: lifecycle_viz::CeremonyKind,
    pub(crate) label: String,
    pub(crate) started: Instant,
}

#[derive(Clone, Debug)]
pub(crate) struct SpendCoin {
    pub(crate) started: Instant,
}

#[derive(Clone, Debug)]
pub(crate) struct QuestStage {
    pub(crate) label: String,
    pub(crate) trace: String,
    pub(crate) theme: Option<String>,
    pub(crate) lexicon: bool,
}

impl App {
    pub(crate) fn new(mut bag: Bag, viewer: Viewer) -> Self {
        // Open the session first so its id can be stamped onto the tool registry
        // (drawer provenance) before any turn runs.
        let mut session = session::Session::new();
        // An explicit interactive choice survives restarts, but only when its
        // exact route is reachable and no environment pin claims precedence.
        let _ = crate::route_preferences::restore(&mut bag);
        let startup = bootstrap::build(&bag, &session.id);
        session.bind(startup.tools.current_workspace());
        let approval_rx = approval::install_ui();
        let mut app = Self::from_parts(
            bag,
            viewer,
            startup.history,
            startup.tools,
            approval_rx,
            session,
            Overwatch::new(),
        );
        // Renderer preferences apply to Explore, not the Realm overview map.
        app.scryglass = crate::scryglass::Scryglass::for_world(app.world.destination());
        // Recall only this project's persisted instruction-bearing state. A
        // goal or memory from another repository must never enter this prompt.
        app.memories = memory::load_for(app.tools.current_workspace())
            .into_iter()
            .map(Arc::<str>::from)
            .collect();
        app.goal = goal::load_for(app.tools.current_workspace());
        if let Some(notice) = app.campaign.startup_notice() {
            app.messages.push(Message {
                role: Role::System,
                text: notice.into(),
            });
        }
        // Restore only this cockpit session's active intent. Concurrent shells
        // in the same repository have distinct loop checkpoints and therefore
        // cannot inherit or drive one another's autonomous work.
        if let Some(st) = loop_ctl::load_for_session(app.tools.current_workspace(), &app.session.id)
        {
            if matches!(
                st.status,
                loop_ctl::LoopStatus::Running | loop_ctl::LoopStatus::Baselining
            ) {
                let iteration = st.iteration;
                app.loop_ctl = st;
                app.messages.push(Message {
                    role: Role::System,
                    text: format!(
                        "saved active loop restored after cockpit restart at iteration {iteration}"
                    )
                    .into(),
                });
            } else if st.status == loop_ctl::LoopStatus::Paused {
                app.messages.push(Message {
                    role: Role::System,
                    text: format!(
                        "saved loop found (paused) — {} iteration(s), {} finding(s); /loop resume loads it, /loop restart resets counters, /loop clear removes it",
                        st.iteration,
                        st.findings.len()
                    ).into(),
                });
            }
        }
        // One line when habitsmith drafted new skill proposals since the last
        // launch (H4) — the drafts wait silently otherwise.
        if let Some(note) = habits::startup_notice(app.tools.current_workspace()) {
            app.messages.push(Message {
                role: Role::System,
                text: note.into(),
            });
        }
        // One line when the Conductor has parked new gated branches since the
        // last launch. Review remains command-only; nothing enters prompts.
        if let Some(note) = conductor::startup_notice(app.tools.current_workspace()) {
            app.messages.push(Message {
                role: Role::System,
                text: note.into(),
            });
        }
        app
    }

    pub(crate) fn preview(viewer: Viewer) -> Self {
        let (_approval_tx, approval_rx) = mpsc::channel();
        let mut app = Self::from_parts(
            Bag::practice_only(),
            viewer,
            Vec::new(),
            Arc::new(harness::ToolRegistry::new()),
            approval_rx,
            session::Session::disabled(),
            Overwatch::disabled(),
        );
        // Static previews describe the cockpit after startup; they must not
        // replace a requested world/room with an asynchronous launch ceremony.
        // Live launches use from_parts directly and retain the intro.
        app.startup_intro
            .dismiss(Instant::now(), lifecycle_viz::MotionMode::Off);
        app
    }

    pub(crate) fn start_lifecycle_ceremony(
        &mut self,
        kind: lifecycle_viz::CeremonyKind,
        label: impl Into<String>,
    ) -> bool {
        // A ceremony is presentation, never control state. If the last
        // completed frame had no Stage pane, the caller's ordinary goal/loop
        // receipt remains the truthful compact record and no hidden animation
        // is armed. Tests that have not drawn yet retain the neutral preview
        // behavior.
        let stage_available = self.last_root_area.is_none()
            || self
                .panes
                .rect_of(crate::mouse::PaneId::Artifacts)
                .is_some();
        if !stage_available
            || !self
                .scryglass
                .controller
                .show_overlay(crate::scryglass::StageOverlay::Lifecycle { kind })
        {
            return false;
        }
        lifecycle_viz::warm_assets();
        self.lifecycle_ceremony = Some(LifecycleCeremony {
            kind,
            label: label.into(),
            started: Instant::now(),
        });
        self.world.set_completion_ceremony_active(matches!(
            kind,
            lifecycle_viz::CeremonyKind::GoalDone | lifecycle_viz::CeremonyKind::GoalCleared
        ));
        let _ = self
            .module_host
            .activate(&runtime::ModuleId::new("artifacts"));
        true
    }

    pub(crate) fn reset_world_yaw(&mut self) {
        self.world_yaw_offset = 0.0;
    }

    pub(crate) fn lifecycle_ceremony_active(&self) -> bool {
        self.lifecycle_ceremony.as_ref().is_some_and(|ceremony| {
            ceremony.started.elapsed().as_secs_f32() < ceremony.kind.duration_secs()
        })
    }

    pub(crate) fn lifecycle_ceremony_animating(&self) -> bool {
        crate::comp_mode::ambient_stage_sim_allowed()
            && self.visual_motion.animates()
            && self.lifecycle_ceremony_active()
            && matches!(
                self.scryglass.controller.overlay(),
                Some(crate::scryglass::StageOverlay::Lifecycle { .. })
            )
            && (self.last_root_area.is_none()
                || self
                    .panes
                    .rect_of(crate::mouse::PaneId::Artifacts)
                    .is_some())
    }

    pub(crate) fn moa_deck_animating(&self) -> bool {
        match self.visual_motion {
            lifecycle_viz::MotionMode::Full | lifecycle_viz::MotionMode::Reduced => self
                .moa_deck
                .as_ref()
                .is_some_and(|deck| deck.transition().is_some()),
            lifecycle_viz::MotionMode::Off => false,
        }
    }

    pub(crate) fn clear_expired_lifecycle_ceremony(&mut self) {
        if !self.lifecycle_ceremony_active() {
            self.lifecycle_ceremony = None;
            self.world.set_completion_ceremony_active(false);
            if matches!(
                self.scryglass.controller.overlay(),
                Some(crate::scryglass::StageOverlay::Lifecycle { .. })
            ) {
                self.scryglass.controller.clear_overlay();
            }
        }
    }

    pub(crate) fn from_parts(
        bag: Bag,
        viewer: Viewer,
        history: Vec<ChatMsg>,
        tools: Arc<harness::ToolRegistry>,
        approval_rx: mpsc::Receiver<approval::Request>,
        session: session::Session,
        overwatch: Overwatch,
    ) -> Self {
        // Decode the miniviz location/rider sources away from both startup and
        // the first draw. The viewer deliberately leaves frame one on the
        // braille fallback, so this worker has a full fast-tick interval to
        // finish before illustrated world playback begins.
        world_viz::warm_cinematic_assets();
        let mut module_host = runtime::ModuleHost::from_default_manifests().unwrap_or_else(|e| {
            let mut fallback = runtime::ModuleHost::default();
            let _ = fallback.register(runtime::ModuleManifest {
                id: runtime::ModuleId::new("core"),
                title: format!("Core Agent Chat (manifest error: {e})"),
                kind: runtime::ModuleKind::Chat,
                default_rect: runtime::WindowRect::new(0, 0, 80, 24),
                activation: runtime::ModuleActivation::Startup,
                capabilities: vec![runtime::ModuleCapability::Chat],
                data_sources: vec!["harness".to_string()],
            });
            let _ = fallback.activate(&runtime::ModuleId::new("core"));
            fallback
        });
        // §3.2 (ledger T4): a manifest's `data_sources` is a spec, so the feeds this
        // cockpit is actually holding are declared here — `tools` is the harness
        // registry, `bag`, `viewer`, and `session` are handles this constructor was
        // given, and `transcript` is the message log `core` renders. Feeds Angel cannot judge from here (a terminal's
        // image backend, whether a native codec is compiled in) are deliberately left
        // *undeclared*: unknown is not absent, so a surface whose feed has simply not
        // been mentioned keeps working exactly as it did.
        for source in ["harness", "bag", "viewer", "session", "transcript"] {
            let _ = module_host.declare_data_source(source, true);
        }
        let mut world = if session.is_disabled() {
            world_viz::World::new(7)
        } else {
            world_viz::World::for_workspace(tools.current_workspace())
        };
        world.enable_districts(scan_workspace_districts(tools.current_workspace()));
        let agentviz_portal =
            crate::agentviz_portal::PortalRuntime::discover(viewer.supports_agentviz_portal());
        let (background_notice_tx, background_notice_rx) = mpsc::channel();
        let campaign = if session.is_disabled() {
            crate::campaign::CampaignController::disabled(tools.current_workspace())
        } else {
            crate::campaign::CampaignController::open(tools.current_workspace())
        };
        let mut app = Self {
            messages: Vec::new(),
            input: String::new(),
            startup_intro: crate::startup_intro::StartupIntro::default(),
            tutor_draft: None,
            explain_requested: false,
            pending_tutor_selection: None,
            bag,
            history,
            undone_exchange: None,
            atlas: tools.atlas(),
            pending_atlas_harvest: None,
            atlas_harvester: None,
            atlas_view: crate::atlas::AtlasViewState::default(),
            tools,
            ui_broker: crate::ui_inspect::interactive_broker(),
            viewer,
            agentviz_portal,
            shell: None,
            shell_focused: false,
            thinking: None,
            pending_turn: None,
            submit_deferral: false,
            world_ticked_at: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or_else(Instant::now),
            steer_queue: Arc::new(steer::SteerQueue::default()),
            bg_job: None,
            last_background_output: None,
            last_background_operation: None,
            background_notice_tx,
            background_notice_rx,
            partial: String::new(),
            stream_text_sanitizer: app_control::TerminalTextSanitizer::default(),
            partial_height_cache: transcript::PartialHeightCache::default(),
            stream_artifact_detector: Default::default(),
            stream_artifact_notice: false,
            retained_partial_msg: None,
            flushed_partial_msg: None,
            reasoning: String::new(),
            reasoning_text_sanitizer: app_control::TerminalTextSanitizer::default(),
            reasoning_shown: 0,
            reasoning_scroll: 0,
            reasoning_wrap: ReasoningWrapMemo::default(),
            lesson_wrap: LessonWrapMemo::default(),
            scroll: 0,
            transcript_heights: Vec::new(),
            transcript_height_prefix: vec![0],
            transcript_heights_w: 0,
            pending_transcript_reflow: None,
            transcript_layouts: transcript::oversized::Layouts::default(),
            transcript_last_bottom: 0,
            transcript_anchor_w: 0,
            transcript_spawns: Vec::new(),
            transcript_rolling: false,
            divider_motion: crate::pane_motion::Divider::default(),
            tool_strip: toolstrip::ToolStrip::default(),
            transcript_mode: TranscriptMode::Conversation,
            receipt_run_open: false,
            turn_renown_started: 0,
            turn_truncated: false,
            turn_route_receipt: None,
            turn_first_output_ms: None,
            portrait_turn_marker: None,
            portrait_fail_run: 0,
            cache_meter: crate::turn::CacheMeter::default(),
            goal: None,
            campaign,
            campaign_pending: None,
            loop_ctl: crate::loop_ctl::LoopState::default(),
            submission_slot: crate::harness::SubmissionSlotTelemetry::default(),
            yukon_fleet: crate::harness::comp_packages::yukon::fleet::YukonFleetState::default(),
            yukon_fleet_rx: None,
            yukon_fleet_polled_at: Instant::now()
                .checked_sub(crate::harness::comp_packages::yukon::fleet::POLL_INTERVAL)
                .unwrap_or_else(Instant::now),
            loop_pending: None,
            loop_experiment: None,
            loop_mirror: world_viz::LoopMirror::default(),
            loop_dialog: None,
            loop_dialog_hits: Vec::new(),
            session_title: None,
            plan_mode: false,
            personality: None,
            relentless_execution: false,
            retard_mode: false,
            solo_mode: false,
            memories: Vec::new(),
            parked_threads: Vec::new(),
            statusline: None,
            pet: None,
            moa_deck: None,
            moa_one_shot: None,
            moa_session: None,
            moa_restore_route: None,
            moa_restore_after_turn: false,
            title_override: None,
            keybinds: Vec::new(),
            vim_mode: false,
            vim_normal: false,
            cursor: 0,
            composer_kill_buffer: None,
            composer_history_index: None,
            composer_history_draft: None,
            composer_history_query: None,
            clipboard_paste: crate::clipboard::ClipboardPaste::new(),
            composer_selection_anchor: None,
            skill_completion_catalog: None,
            workspace_completion_cache: None,
            media: Vec::new(),
            media_scroll: 0,
            observatory: crate::observatory::ObservatoryState::default(),
            quest_stage: None,
            scryglass_enabled: std::env::var("ANGEL_SCRYGLASS")
                .map(|value| {
                    !matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "0" | "off" | "false" | "no"
                    )
                })
                .unwrap_or(true),
            scryglass: crate::scryglass::Scryglass::default(),
            rl_view: crate::rl_viz::RlView::default(),
            research: crate::research_workspace::Workspace::default(),
            agent_graph: crate::graph_ctl::GraphState::default(),
            handoff_rl: crate::handoff_rl::HandoffRlState::default(),
            lifecycle_ceremony: None,
            spend_coin: None,
            world_yaw_offset: 0.0,
            visual_motion: lifecycle_viz::MotionMode::from_env(),
            image_title_label: None,
            image_title_cache: String::new(),
            shell_area: None,
            panes: mouse::PaneRegistry::default(),
            panel_frames: panels::PanelFrames::default(),
            selection: None,
            scryglass_drag: None,
            copy_requested: false,
            pending_clipboard: None,
            redraw_requested: false,
            pending_quick_lookup: None,
            terminal_focused: true,
            attention_requested: false,
            last_root_area: None,
            last_resize_at: Instant::now(),
            idle_route_title: None,
            agent_control_chips: None,
            overwatch,
            world,
            // Earned only when Stage paint runs this frame (draw resets to false).
            // Starting true charged headless / ANGEL_BACKDROP=off sessions for
            // invisible scenery on every advance.
            world_pane_visible: false,
            quintain_route_presented: false,
            village_rx: None,
            world_buttons: Vec::new(),
            agent_buttons: Vec::new(),
            agent_menu: None,
            agent_menu_search: None,
            agent_menu_details: false,
            agent_menu_show_unavailable: false,
            agent_menu_hits: Vec::new(),
            agent_menu_area: None,
            agent_control_area: None,
            header_portrait_area: None,
            header_card_area: None,
            bay_portrait_area: None,
            agent_info_area: None,
            route_evidence: crate::route_intelligence::RouteEvidenceSnapshot::default(),
            route_evidence_loaded_at: None,
            last_completed_route: None,
            last_turn_outcome: None,
            brain_route_receipt: None,
            should_quit: false,
            exit_request: None,
            session,
            session_warning_notified: false,
            started: Instant::now(),
            pending_approval: None,
            approval_rx,
            apollo_specialist_present: Cell::new(false),
            specialist_message_scan_len: Cell::new(0),
            specialist_media_scan_len: Cell::new(0),
            active_profile_label: String::new(),
            active_profile_specialist: false,
            active_profile_cache: None,
            reborn_rx: None,
            reborn_exec: None,
            module_host,
        };
        // Wake the village: adopt the persisted hamlet and start the single
        // sensing thread (forge + heads, staggered probes). Never under test —
        // unit-test Apps stay off the network and off the disk.
        if !cfg!(test) && village::enabled() {
            let path = village::state_path();
            let state = village::load_state(path.as_deref());
            let heads = village::discover_heads();
            let head_ids: Vec<String> = heads.iter().map(|(id, _)| id.clone()).collect();
            let seed = village::PollerSeed {
                forge_url: village::forge_url(),
                voice_url: village::voice_url(),
                flavor: village::flavor_enabled(),
                heads,
                last_adapter: state.last_adapter.clone(),
                need_name: state.apprentice_name.is_empty(),
                need_boot_chatter: state.chatter.is_empty(),
                town: app.world.town_name().to_string(),
            };
            app.world.enable_village(state, path, &head_ids);
            app.village_rx = Some(village::spawn_poller(seed));
        }
        // The carousel starts EMPTY — it's the angel's delivery surface: cards
        // appear only when the agent calls the `present` tool (see harness.rs).
        app.scryglass.sync_arrival(app.world.arrived_building());
        app
    }

    /// Rebuild the visible transcript from `history` (used after /resume).
    /// Shows user turns and assistant text; tool plumbing stays hidden.
    pub(crate) fn rebuild_display(&mut self) {
        self.messages.clear();
        self.invalidate_transcript_layout();
        let visible: Vec<(ChatRole, Arc<str>)> = self
            .history
            .iter()
            .filter_map(|m| match m.role {
                ChatRole::User => Some((ChatRole::User, Arc::clone(&m.content))),
                ChatRole::Assistant
                    if !m.content.is_empty()
                        && !crate::compaction::is_plan_snapshot(&m.content) =>
                {
                    Some((ChatRole::Assistant, Arc::clone(&m.content)))
                }
                _ => None,
            })
            .collect();
        for (role, content) in visible {
            match role {
                ChatRole::User => self.messages.push(Message::new(Role::User, content)),
                ChatRole::Assistant if !content.is_empty() => {
                    let text = self.display_reply(content.to_string());
                    self.messages.push(Message::new(Role::Angel, text));
                }
                _ => {}
            }
        }
        // Restored/rebuilt history was not newly delivered on screen, so it
        // must start settled rather than replaying old entry motion.
        self.transcript_spawns
            .resize(self.messages.len(), f32::NEG_INFINITY);
    }

    /// Invalidate every cache whose indices or row counts are tied to the
    /// current `messages` contents. Call whenever the transcript is replaced,
    /// even if the new history happens to have the same message count.
    pub(crate) fn invalidate_transcript_layout(&mut self) {
        self.transcript_layouts.invalidate();
        self.selection = None;
        self.copy_requested = false;
        self.pending_transcript_reflow = None;
        self.transcript_heights.clear();
        self.transcript_height_prefix.clear();
        self.transcript_height_prefix.push(0);
        self.transcript_heights_w = 0;
        self.transcript_spawns.clear();
        self.transcript_last_bottom = 0;
        self.transcript_anchor_w = 0;
    }

    /// `(progress 0..1, elapsed secs, club label)` while thinking, else `None`.
    /// Progress asymptotically approaches 1 — indeterminate, since we don't know
    /// the reply's ETA — and resets when the reply lands.
    pub(crate) fn think_state(&self) -> Option<(f32, f32, &str)> {
        self.thinking.as_ref().map(Thinking::progress)
    }

    /// Decorative avatar reasoning slide. Comp / lean snaps the think text
    /// immediately; default still rolls. The feed itself is never delayed.
    pub(crate) fn reasoning_roll_in_allowed() -> bool {
        crate::comp_mode::ambient_stage_sim_allowed()
    }

    /// The reasoning display hasn't caught up with the reasoning feed yet —
    /// keep ticking the roll-in (and the event loop) until it has, even after
    /// the turn itself has finished. Comp / lean never holds the 33 ms cadence
    /// for this slide.
    pub(crate) fn reasoning_roll_pending(&self) -> bool {
        Self::reasoning_roll_in_allowed() && self.reasoning_shown < self.reasoning.len()
    }

    pub(crate) fn quick_lookup_roll_pending(&self) -> bool {
        self.scryglass.lesson_roll_pending()
    }

    /// The miniworld has motion actually ON SCREEN (avatar walking, storm, or
    /// sparkles in a pane the last draw laid out). A hidden world never earns
    /// scenery frames. Comp / lean still lays Stage chrome (`world_pane_visible`)
    /// but must not keep the event loop on the scenery cadence — `world.tick`
    /// is already skipped, so leftover dest/sparkle flags would otherwise
    /// never settle.
    ///
    /// A2: scryglass.animating already requires visible+renderable; still gate
    /// on `stage_world_mirrors_allowed` so a Stage that was not laid out on the
    /// last draw (ANGEL_BACKDROP=off, shell, image) cannot force the cadence.
    pub(crate) fn world_animating(&self) -> bool {
        crate::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible)
            && (self.world.animating() || self.scryglass.animating())
    }

    pub(crate) fn show_spend_coin(&mut self) {
        self.spend_coin = Some(SpendCoin {
            started: Instant::now(),
        });
    }

    pub(crate) fn spend_coin_motion(&self) -> lifecycle_viz::MotionMode {
        if crate::comp_mode::enabled() {
            lifecycle_viz::MotionMode::Off
        } else {
            self.visual_motion
        }
    }

    pub(crate) fn spend_coin_active(&self) -> bool {
        self.spend_coin.as_ref().is_some_and(|coin| {
            crate::spend_viz::is_active(
                coin.started.elapsed().as_secs_f32(),
                self.spend_coin_motion(),
            )
        })
    }

    pub(crate) fn spend_coin_animating(&self) -> bool {
        self.spend_coin_motion().animates() && self.spend_coin_active()
    }

    /// Expensive scenery must yield while any foreground or detached agent
    /// operation owns the machine. The ride cache still lets direct camera
    /// input repaint immediately because view-key changes bypass relaxation.
    pub(crate) fn scenery_relaxed(&self) -> bool {
        self.thinking.is_some()
            || self.pending_turn.is_some()
            || self.bg_job.is_some()
            || self.loop_ctl.cycle_started_ms.is_some()
    }

    /// Stage-owned 33 ms cadence. Hidden / backdrop-off / compact Core never
    /// pay for invisible loop viz, raytrace, lesson roll, or MoA transitions.
    /// Visible Stage still requests the fast lane; hammertime stays off in
    /// comp/lean mode.
    pub(crate) fn stage_display_wants_fast_tick(&self) -> bool {
        if !crate::comp_mode::stage_world_mirrors_allowed(self.world_pane_visible) {
            return false;
        }
        self.quick_lookup_roll_pending()
            || self.scryglass.animating()
            || self.scryglass.surface == crate::scryglass::StageSurface::Raytrace
            || self.moa_deck_animating()
            || self.loop_ctl.cycle_started_ms.is_some()
            || self.spend_coin_animating()
            || crate::loop_viz::hammertime_active(&self.loop_ctl)
    }

    /// Pure frame-wait decision for the event loop and deterministic tests.
    /// Display-only animation may request the 33 ms cadence only while its
    /// bounded, latency-sensitive state is active. Ambient world motion is
    /// deliberately excluded and handled by `needs_responsive_tick`.
    /// Side-column visuals (agent-bay portal). Backdrop-off never pays.
    /// Comp / lean mode is additional.
    pub(crate) fn side_column_visuals_allowed() -> bool {
        crate::surfaces::BackdropMode::from_env().shows_side_column()
            && crate::comp_mode::ambient_stage_sim_allowed()
    }

    /// Agentviz portal 33 ms cadence. Hidden / text-only side column never
    /// keeps the UI on the fast lane for an invisible portal.
    pub(crate) fn agentviz_wants_fast_tick(&self) -> bool {
        Self::side_column_visuals_allowed()
            && (self.agentviz_portal.is_rendering() || self.viewer.agentviz_portal_pending())
    }

    pub(crate) fn transcript_reflow_wants_fast_tick(&self) -> bool {
        self.terminal_focused
            && self.panes.rect_of(mouse::PaneId::Transcript).is_some()
            && self.transcript_layouts.failure().is_none()
            && (self.transcript_heights.len() < self.messages.len()
                || self.transcript_layouts.busy()
                || self
                    .pending_transcript_reflow
                    .as_ref()
                    .is_some_and(|work| !work.ready(self.messages.len())))
    }

    pub(crate) fn needs_fast_tick(&self) -> bool {
        self.thinking.is_some()
            // A clipboard read is sub-second in practice; polling it on the fast
            // lane keeps the staged chip from waiting on the 200 ms idle tick.
            || self.clipboard_paste.loading()
            || (self.terminal_focused
                && Self::side_column_visuals_allowed()
                && self
                    .startup_intro
                    .animating(Instant::now(), self.visual_motion))
            || self.pending_turn.is_some()
            || self.reasoning_roll_pending()
            || self.transcript_rolling
            || (self.terminal_focused
                && Self::side_column_visuals_allowed()
                && self.panel_frames.get(panels::PanelKind::AgentBay).is_some()
                && self
                    .panel_frames
                    .get(panels::PanelKind::Artifacts)
                    .is_some()
                && self.divider_motion.active(Instant::now()))
            || self.transcript_reflow_wants_fast_tick()
            || self.shell_focused
            || self.lifecycle_ceremony_animating()
            || self.agentviz_wants_fast_tick()
            || self.ui_broker.has_pending()
            || self.stage_display_wants_fast_tick()
    }

    /// Medium cadence for work that should feel alive without monopolizing
    /// full-terminal redraws. Background jobs are polled promptly here too,
    /// instead of waiting on the 200 ms idle lane. Scryglass transitions also
    /// satisfy this predicate, but `needs_fast_tick` wins in the event loop.
    pub(crate) fn needs_responsive_tick(&self) -> bool {
        self.bg_job.is_some() || self.world_animating()
    }

    /// Concrete route under the Brain Route cursor. This is a reversible visual
    /// preview only: the Bag remains the committed route until Enter/click.
    fn agent_menu_profile_choice(&self) -> Option<crate::club::RouteChoice> {
        if self.thinking.is_some() || self.bg_job.is_some() {
            return None;
        }
        let menu = self.agent_menu?;
        let choices = self.bag.route_choices();
        match menu.kind {
            crate::agent_controls::AgentMenuKind::Model => choices.get(menu.selected).cloned(),
            crate::agent_controls::AgentMenuKind::Thinking => {
                menu.route_target.and_then(|target| {
                    choices
                        .iter()
                        .find(|choice| {
                            choice.agent_index == target.0 && choice.slot_index == target.1
                        })
                        .cloned()
                })
            }
        }
    }

    /// Agent label under the cursor, retained for the compact caption. Portrait
    /// identity itself also consumes the previewed driver/model below.
    pub(crate) fn agent_menu_profile_preview(&self) -> Option<String> {
        self.agent_menu_profile_choice().map(|choice| choice.agent)
    }

    /// Test-facing proof that moving the THINK cursor does not commit backend
    /// state before Enter/click.
    #[cfg(test)]
    pub(crate) fn agent_menu_effort_preview(&self) -> Option<String> {
        if self.thinking.is_some() || self.bg_job.is_some() {
            return None;
        }
        let menu = self.agent_menu?;
        let choices = self.bag.route_choices();
        match menu.kind {
            crate::agent_controls::AgentMenuKind::Model => choices
                .get(menu.selected)
                .filter(|choice| choice.available)
                .and_then(|choice| choice.reasoning_effort.clone()),
            crate::agent_controls::AgentMenuKind::Thinking => {
                let choice = menu.route_target.and_then(|target| {
                    choices.iter().find(|choice| {
                        choice.agent_index == target.0 && choice.slot_index == target.1
                    })
                })?;
                if !choice.available {
                    return None;
                }
                choice.reasoning_levels.get(menu.selected).cloned()
            }
        }
    }

    pub(crate) fn active_profile(&mut self) -> AgentProfile {
        // The Apollo specialist persona is a *transient* swarm-routing hint: it may
        // recolour the swarm host's portrait only while a turn is actually in
        // flight. When idle, the avatar is a pure function of the in-hand agent, so
        // after any turn the portrait returns to the agent in hand instead of
        // sticking on a persona (see `reset_specialist_persona`).
        let specialist = self.thinking.is_some() && self.apollo_specialist_present();
        if let Some(thinking) = self.thinking.as_ref() {
            // A failover wrapper publishes the route that actually answered as
            // soon as it resolves. Before that point this is its requested
            // primary, so the portrait changes at the same causal boundary as
            // automatic routing without guessing from streamed text.
            // Draw never rebuilds a full route identity: that allocates a
            // fresh identity every frame for ordinary clubs.
            let resolved = thinking
                .club
                .as_ref()
                .and_then(|club| club.resolved_route_if_known());
            let route = resolved.as_ref().unwrap_or(&thinking.requested_route);
            if let Some(profile) = self.hit_active_profile_cache(
                specialist,
                thinking.club_label.as_str(),
                route.driver.as_str(),
                route.model.as_deref(),
            ) {
                return profile;
            }
            let agent = thinking.club_label.clone();
            let driver = route.driver.clone();
            let model = route.model.clone();
            return self.store_active_profile(specialist, &agent, &driver, model.as_deref());
        }

        if self.bg_job.is_none()
            && let Some(menu) = self.agent_menu
        {
            let choices = self.bag.route_choices();
            let choice = match menu.kind {
                crate::agent_controls::AgentMenuKind::Model => choices.get(menu.selected),
                crate::agent_controls::AgentMenuKind::Thinking => {
                    menu.route_target.and_then(|target| {
                        choices.iter().find(|choice| {
                            choice.agent_index == target.0 && choice.slot_index == target.1
                        })
                    })
                }
            };
            if let Some(choice) = choice {
                if let Some(profile) = self.hit_active_profile_cache(
                    specialist,
                    &choice.agent,
                    &choice.driver,
                    Some(&choice.model),
                ) {
                    return profile;
                }
                let agent = choice.agent.clone();
                let driver = choice.driver.clone();
                let model = choice.model.clone();
                return self.store_active_profile(specialist, &agent, &driver, Some(&model));
            }
        }

        let choices = self.bag.route_choices();
        if let Some(choice) = choices.iter().find(|choice| choice.selected) {
            if let Some(profile) = self.hit_active_profile_cache(
                specialist,
                &choice.agent,
                &choice.driver,
                Some(&choice.model),
            ) {
                return profile;
            }
            let agent = choice.agent.clone();
            let driver = choice.driver.clone();
            let model = choice.model.clone();
            return self.store_active_profile(specialist, &agent, &driver, Some(&model));
        }
        drop(choices);
        let label = self.bag.in_hand_label();
        if let Some(profile) = self.hit_active_profile_cache(specialist, label, label, None) {
            return profile;
        }
        let label = label.to_string();
        self.store_active_profile(specialist, &label, &label, None)
    }

    fn hit_active_profile_cache(
        &self,
        specialist: bool,
        agent: &str,
        driver: &str,
        model: Option<&str>,
    ) -> Option<AgentProfile> {
        if self.active_profile_specialist == specialist
            && active_profile_cache_matches(&self.active_profile_label, agent, driver, model)
        {
            self.active_profile_cache
        } else {
            None
        }
    }

    fn store_active_profile(
        &mut self,
        specialist: bool,
        agent: &str,
        driver: &str,
        model: Option<&str>,
    ) -> AgentProfile {
        let cache_label = if agent.eq_ignore_ascii_case(driver) && model.is_none() {
            agent.to_string()
        } else {
            format!("{agent} ▸ {driver} ▸ {}", model.unwrap_or("native"))
        };
        let profile = profile_for_route(agent, driver, model, specialist);
        self.active_profile_label = cache_label;
        self.active_profile_specialist = specialist;
        self.active_profile_cache = Some(profile);
        profile
    }

    pub(crate) fn apollo_specialist_present(&self) -> bool {
        if self.apollo_specialist_present.get() {
            return true;
        }

        let message_start = scan_start(self.specialist_message_scan_len.get(), self.messages.len());
        let media_start = scan_start(self.specialist_media_scan_len.get(), self.media.len());
        let present = self.messages[message_start..]
            .iter()
            .any(|m| specialist_text(&m.text))
            || self.media[media_start..]
                .iter()
                .any(|m| specialist_text(m.label()));
        self.specialist_message_scan_len.set(self.messages.len());
        self.specialist_media_scan_len.set(self.media.len());
        if present {
            self.apollo_specialist_present.set(true);
        }
        present
    }

    /// Clear the latched Apollo-specialist signal at a turn boundary so a fresh
    /// turn never inherits the previous turn's persona. The scan cursors are
    /// advanced to the current transcript end, so already-seen apollo/kernel
    /// mentions can't immediately re-latch — only *new* specialist work in the
    /// next turn re-activates the transient coloring. Without this the avatar
    /// stuck on Apollo forever once any apollo-class word appeared.
    pub(crate) fn reset_specialist_persona(&mut self) {
        self.apollo_specialist_present.set(false);
        self.specialist_message_scan_len.set(self.messages.len());
        self.specialist_media_scan_len.set(self.media.len());
    }

    pub(crate) fn resize_shell_to_area(&mut self, area: Rect) {
        if self.shell_area == Some(area) {
            return;
        }
        self.shell_area = Some(area);
        if let Some(shell) = self.shell.as_mut() {
            shell.resize(area.height.max(1), area.width.max(1));
        }
    }

    pub(crate) fn image_viewer_title(&mut self) -> Cow<'_, str> {
        let Some(label) = self.viewer.label() else {
            if self.image_title_label.is_some() {
                self.image_title_label = None;
            }
            if !self.image_title_cache.is_empty() {
                self.image_title_cache.clear();
            }
            return Cow::Borrowed(status_view::image_viewer_title());
        };
        if self.image_title_label.as_deref() != Some(label) {
            let label = label.to_owned();
            let image_mark = glyphs::media_prefix("image").trim();
            // Hint first so a narrow panel truncates the (long) path, never the
            // actionable "/hide to return" (Phase A's bordered image box is narrow).
            self.image_title_cache = format!(" {image_mark} · /hide to return · {label} ");
            self.image_title_label = Some(label);
        }
        Cow::Borrowed(self.image_title_cache.as_str())
    }
}
