//! Workspace/status/config, session lifecycle, keybinds, skills and memory.
use super::*;
use angel_plug::feed::{Feed, FeedWorld};
use angel_plug::stage::StagePlaces;

fn recap_preview(text: &str, width: usize) -> String {
    let single_line = text
        .split_whitespace()
        .take(48)
        .collect::<Vec<_>>()
        .join(" ");
    if single_line.is_empty() {
        "none".to_string()
    } else {
        crate::ui::toolstrip::ellipsize(&single_line, width)
    }
}

fn recap_path_preview(path: &str, width: usize) -> String {
    let single_line = path.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.chars().count() <= width {
        return single_line;
    }
    let tail = single_line
        .chars()
        .rev()
        .take(width.saturating_sub(1))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("…{tail}")
}

fn mutation_paths(call: &crate::agent::club::ToolCall) -> Vec<String> {
    crate::knowledge::cut::mutation_targets(&call.name, &call.args)
}

fn session_recap_text(history: &[ChatMsg]) -> String {
    let users = history
        .iter()
        .filter(|message| message.role == ChatRole::User)
        .count();
    let replies = history
        .iter()
        .filter(|message| message.role == ChatRole::Assistant && !message.content.trim().is_empty())
        .count();
    let tool_results = history
        .iter()
        .filter(|message| message.role == ChatRole::Tool)
        .count();

    let mut tool_counts = std::collections::HashMap::<String, usize>::new();
    for call in history.iter().flat_map(|message| message.tool_calls.iter()) {
        *tool_counts.entry(call.name.clone()).or_default() += 1;
    }
    let mut tools = tool_counts.into_iter().collect::<Vec<_>>();
    tools.sort_by(|(left_name, left_count), (right_name, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_name.cmp(right_name))
    });
    let tools = if tools.is_empty() {
        "none".to_string()
    } else {
        tools
            .into_iter()
            .take(3)
            .map(|(name, count)| format!("{}×{count}", recap_preview(&name, 24)))
            .collect::<Vec<_>>()
            .join(" · ")
    };

    let mut seen_paths = std::collections::HashSet::new();
    let mut paths = Vec::new();
    for call in history
        .iter()
        .rev()
        .flat_map(|message| message.tool_calls.iter().rev())
    {
        for path in mutation_paths(call).into_iter().rev() {
            if seen_paths.insert(path.clone()) {
                paths.push(recap_path_preview(&path, 48));
                if paths.len() == 3 {
                    break;
                }
            }
        }
        if paths.len() == 3 {
            break;
        }
    }
    let paths = if paths.is_empty() {
        "none".to_string()
    } else {
        paths.join(" · ")
    };

    let latest_user = history
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::User)
        .map(|message| recap_preview(&message.content, 72))
        .unwrap_or_else(|| "none".to_string());
    let latest_reply = history
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::Assistant && !message.content.trim().is_empty())
        .map(|message| recap_preview(&message.content, 72))
        .unwrap_or_else(|| "none".to_string());

    format!(
        "recap\n  exchange {users} user · {replies} replies · {tool_results} tool results\n  \
         top tools {tools}\n  touched  {paths}\n  latest › {latest_user}\n  angel  › {latest_reply}"
    )
}

impl App {
    pub(crate) fn checkpoint_before_boundary(&self, command: &str) -> Result<(), String> {
        self.session.checkpoint(&self.history).map_err(|error| {
            format!(
                "{command}: boundary blocked · could not save current session {} to {} · {error}. \
                 Current conversation and workspace retained; fix the save error and retry.",
                self.session.id,
                self.session.path().display()
            )
        })
    }

    pub(crate) fn change_workspace(&mut self, arg: Option<&str>) -> String {
        self.launch_interaction();
        let current = self.tools.current_workspace().to_path_buf();
        let Some(arg) = arg else {
            return format!("working directory: {}", current.display());
        };
        if self.thinking.is_some()
            || self.bg_job.is_some()
            || self.loop_pending.is_some()
            || self.loop_experiment.is_some()
        {
            return "busy — finish or interrupt the turn before /cd".to_string();
        }
        let expanded: PathBuf = match arg.strip_prefix('~') {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => {
                match std::env::var_os("HOME") {
                    Some(home) => PathBuf::from(home).join(rest.trim_start_matches('/')),
                    None => PathBuf::from(arg),
                }
            }
            _ => PathBuf::from(arg),
        };
        let candidate = if expanded.is_absolute() {
            expanded
        } else {
            current.join(expanded)
        };
        let target = match std::fs::canonicalize(&candidate) {
            Ok(p) => p,
            Err(e) => return format!("/cd: {}: {e}", candidate.display()),
        };
        if !target.is_dir() {
            return format!("/cd: {} is not a directory", target.display());
        }
        if target == current {
            return format!("working directory unchanged: {}", target.display());
        }
        if let Err(message) = self.checkpoint_before_boundary("/cd") {
            return message;
        }
        let loop_paused = self.loop_detach_for_workspace_change();
        let mut new_session = crate::knowledge::session::Session::new();
        new_session.bind(&target);
        let (new_history, mut registry) = crate::app::bootstrap::build_history_and_registry(
            &self.bag,
            &new_session.id,
            target.clone(),
        );
        crate::ui::ui_inspect::install(&mut registry, Arc::clone(&self.ui_broker));
        registry.enable_action_capsules();
        self.tools = Arc::new(registry);
        self.atlas = self.tools.atlas();
        self.atlas_view = crate::knowledge::atlas::AtlasViewState::default();
        self.session = new_session;
        self.history = new_history;
        self.undone_exchange = None;
        self.goal = crate::drive::goal::load_for(&target);
        self.memories = crate::knowledge::memory::load_for(&target)
            .into_iter()
            .map(Arc::<str>::from)
            .collect();
        self.parked_threads.clear();
        self.plan_mode = false;
        self.personality = None;
        self.relentless_execution = false;
        self.solo_mode = false;
        crate::agent::tools::solo::set_solo_mode(false);
        self.clear_moa_arm();
        self.session_title = None;
        self.close_shell(None);
        self.media.clear();
        self.media_scroll = 0;
        self.clipboard_paste.clear();
        self.pending_stage_copy = None;
        self.clipboard_image_copy = None;
        self.realm.feed(Feed::Workspace(&target));
        self.scryglass = crate::ui::scryglass::Scryglass::opening(self.realm.destination_place());
        self.world_buttons.clear();
        self.agent_buttons.clear();
        self.last_completed_route = None;
        self.last_background_output = None;
        self.last_background_operation = None;
        self.rebuild_display();
        self.system_msg(format!(
            "project boundary → {} · fresh thread · prior goal/memory/loop/steers detached{}",
            target.display(),
            if loop_paused {
                " · prior loop saved paused"
            } else {
                ""
            }
        ));
        let _ = self.session.save(&self.history);
        self.emit_session_start(crate::agent::harness::SessionStartSource::Workspace);
        format!("working directory → {}", target.display())
    }

    pub(crate) fn startup_resume(&mut self, id: Option<String>) -> bool {
        let workspace = self.tools.current_workspace().to_path_buf();
        match crate::app::local_command::resume(id, &workspace) {
            crate::app::local_command::ResumeResult::Loaded {
                id,
                mut history,
                turns,
            } => {
                crate::app::bootstrap::refresh_history(&self.bag, &workspace, &mut history);
                self.history = history;
                self.undone_exchange = None;
                self.parked_threads.clear();
                self.last_background_output = None;
                self.last_background_operation = None;
                let _ = self.steer_queue.drain();
                self.rebuild_display();
                self.session = crate::knowledge::session::Session::with_id_for(&id, &workspace);
                self.system_msg(format!("resumed session {id} ({turns} turns) — continuing"));
                true
            }
            crate::app::local_command::ResumeResult::NoSession => {
                self.system_msg("--resume: no saved session found — starting fresh".to_string());
                false
            }
            crate::app::local_command::ResumeResult::Failed(e) => {
                self.system_msg(format!("--resume failed ({e}) — starting fresh"));
                false
            }
        }
    }

    pub(crate) fn announce_startup_session(&mut self, resume: Option<Option<String>>) {
        let resumed = resume.is_some_and(|id| self.startup_resume(id));
        self.emit_session_start(if resumed {
            crate::agent::harness::SessionStartSource::Resume
        } else {
            crate::agent::harness::SessionStartSource::Startup
        });
    }

    pub(crate) fn emit_session_start(&self, source: crate::agent::harness::SessionStartSource) {
        if self.session.is_disabled() {
            return;
        }
        crate::agent::harness::fire_session_start(
            &self.session.id,
            self.tools.current_workspace(),
            source,
        );
    }

    fn in_hand_model(&self) -> String {
        if self.bag.in_hand_label() == "practice" {
            return "no live model".to_string();
        }
        match self.bag.in_hand_mode() {
            Some(mode) => format!("{}·{mode}", self.bag.in_hand_label()),
            None => self.bag.in_hand_label().to_string(),
        }
    }

    pub(crate) fn status_text(&self) -> String {
        let agents = self
            .bag
            .tabs()
            .iter()
            .map(|t| t.label.clone())
            .collect::<Vec<_>>()
            .join(" ");
        let agents = if agents.is_empty() {
            "none reachable".to_string()
        } else {
            agents
        };
        let turns = self
            .history
            .iter()
            .filter(|m| m.role == ChatRole::User)
            .count();
        let goal = self.goal_status_line();
        let mut out = format!(
            "status\n  model    {}\n  agents   {agents}\n  goal     {goal}\n  \
             turns    {turns}\n  tools    {}\n  session  {}",
            self.in_hand_model(),
            self.tools.defs().len(),
            self.session.id,
        );
        if let Some(t) = &self.session_title {
            out.push_str(&format!("\n  thread   {t}"));
        }
        if self.plan_mode {
            out.push_str("\n  plan     ON");
        }
        if let Some(p) = &self.personality {
            out.push_str(&format!("\n  style    {p}"));
        }
        if self.relentless_execution {
            out.push_str("\n  relentless ON");
        }
        if self.solo_mode {
            out.push_str("\n  solo      ON");
        }
        if !self.memories.is_empty() {
            out.push_str(&format!("\n  memories {}", self.memories.len()));
        }
        if self.loop_active() {
            out.push_str(&format!(
                "\n  loop     iter {} · {} findings",
                self.loop_ctl.iteration,
                self.loop_ctl.findings.len()
            ));
        }
        if let Some(mission) = crate::app::mission::latest_mission_line() {
            out.push_str(&format!("\n  mission  {mission}"));
        }
        if let Some(lesson) = self.scryglass.lesson() {
            out.push_str(&format!(
                "\n  lesson   {} · {} · recall ready",
                lesson.term(),
                lesson.tutor_name()
            ));
        }
        let active_modules = self
            .module_host
            .statuses()
            .into_iter()
            .filter(|s| s.state.is_running())
            .map(|s| s.id.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        if !active_modules.is_empty() {
            out.push_str(&format!("\n  modules  {active_modules}"));
        }
        if let Some(usage) = self.bag.in_hand().usage_report() {
            out.push('\n');
            out.push_str(&usage);
        }
        out.push('\n');
        out.push_str(&session_recap_text(&self.history));
        out
    }

    pub(crate) fn save_checkpoint_text(&self) -> String {
        if self.session.is_disabled() {
            return "session checkpoint unavailable · preview/test session persistence is disabled"
                .to_string();
        }
        if self.history.is_empty() {
            return "session checkpoint skipped · no committed conversation history to persist"
                .to_string();
        }
        match self.session.checkpoint(&self.history) {
            Ok(()) => format!(
                "session checkpoint saved · {} committed message(s) · {}",
                self.history.len(),
                self.session.path().display()
            ),
            Err(error) => {
                format!("session checkpoint failed · committed history remains in memory · {error}")
            }
        }
    }

    pub(crate) fn model_info_text(&self) -> String {
        let agents = {
            let agents = self
                .bag
                .tabs()
                .iter()
                .map(|t| {
                    if t.in_hand {
                        format!("[{}]", t.label)
                    } else {
                        t.label.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            if agents.is_empty() {
                "none reachable".to_string()
            } else {
                agents
            }
        };
        let thinking = self
            .bag
            .reasoning_effort()
            .unwrap_or_else(|| "model-native".to_string());
        format!(
            "in hand: {}\nthinking: {thinking}\nagents:  {agents}\n{}\nAgent panel: click MODEL / THINK · deck: / search, o ops, q user, ↑↓, Enter · Tab box · ←→ mode\nctx badges preview fit; ≥95%-full routes require a second commit. /model <filter> or <exact-model>@<effort> and /think <filter> prepare confirmations; /model auto clears remembered choices.",
            self.in_hand_model(),
            self.bag.sota_moa_status()
        )
    }

    pub(crate) fn toggle_approvals(&self, arg: Option<&str>) -> String {
        let raw = arg.map(str::trim).unwrap_or("");
        if let Some(rest) = raw.strip_prefix("actions") {
            if !rest.is_empty() && !rest.chars().next().is_some_and(char::is_whitespace) {
                return "usage: /approvals actions [observe|on|off]".to_string();
            }
            let current = crate::agent::harness::ActionCapsuleMode::parse(
                std::env::var("ANGEL_ACTION_CAPSULES").ok().as_deref(),
            );
            let next = match rest.trim().to_ascii_lowercase().as_str() {
                "" => match current {
                    crate::agent::harness::ActionCapsuleMode::Off => {
                        crate::agent::harness::ActionCapsuleMode::Approve
                    }
                    _ => crate::agent::harness::ActionCapsuleMode::Off,
                },
                "observe" | "preview" => crate::agent::harness::ActionCapsuleMode::Observe,
                "on" | "1" | "yes" | "approve" => crate::agent::harness::ActionCapsuleMode::Approve,
                "off" | "0" | "no" | "disable" => crate::agent::harness::ActionCapsuleMode::Off,
                _ => return "usage: /approvals actions [observe|on|off]".to_string(),
            };
            match next {
                crate::agent::harness::ActionCapsuleMode::Off => {
                    unsafe { std::env::set_var("ANGEL_ACTION_CAPSULES", "off") }
                }
                crate::agent::harness::ActionCapsuleMode::Observe => {
                    unsafe { std::env::set_var("ANGEL_ACTION_CAPSULES", "observe") }
                }
                crate::agent::harness::ActionCapsuleMode::Approve => {
                    unsafe { std::env::set_var("ANGEL_ACTION_CAPSULES", "approve") }
                }
            }
            return format!(
                "action capsules {} (next turn) — local previews/receipts only{}",
                match next {
                    crate::agent::harness::ActionCapsuleMode::Off => "OFF",
                    crate::agent::harness::ActionCapsuleMode::Observe => "OBSERVE",
                    crate::agent::harness::ActionCapsuleMode::Approve => "APPROVE",
                },
                if matches!(next, crate::agent::harness::ActionCapsuleMode::Approve) {
                    "; one approval covers each scoped tool batch"
                } else {
                    ""
                }
            );
        }
        let cur = std::env::var_os("ANGEL_SWARM_APPROVE").is_some();
        let target = match arg.map(|a| a.to_ascii_lowercase()) {
            None => !cur,
            Some(a) if matches!(a.as_str(), "on" | "1" | "yes" | "enable") => true,
            Some(a) if matches!(a.as_str(), "off" | "0" | "no" | "disable") => false,
            Some(_) => return "usage: /approvals [on|off|probe|selftest]".to_string(),
        };
        if target {
            unsafe { std::env::set_var("ANGEL_SWARM_APPROVE", "1") };
        } else {
            unsafe { std::env::remove_var("ANGEL_SWARM_APPROVE") };
        }
        format!(
            "approval gate {} (live) — a non-local swarm action (phone/remote/peer) {} pause for y/a/n",
            if target { "ON" } else { "OFF" },
            if target { "will" } else { "won't" },
        )
    }

    pub(crate) fn toggle_yolo(&self, arg: Option<&str>) -> String {
        match arg
            .map(str::trim)
            .unwrap_or("status")
            .to_ascii_lowercase()
            .as_str()
        {
            "on" | "1" | "yes" | "enable" | "enabled" | "full" => crate::platform::yolo::set(true),
            "smart" | "yolos" | "power" => crate::platform::yolo::set_smart(true),
            "off" | "0" | "no" | "disable" | "disabled" | "guarded" => {
                crate::platform::yolo::clear_all()
            }
            "" | "status" => return crate::platform::yolo::status_text(),
            _ => {
                return "usage: /yolo [on|off|status|smart|full] — smart=/yolos (powerful coding); \
                        on/full=unrestricted machine authority"
                    .to_string();
            }
        }
        crate::platform::yolo::status_text()
    }

    pub(crate) fn toggle_yolos(&self, arg: Option<&str>) -> String {
        match arg
            .map(str::trim)
            .unwrap_or("status")
            .to_ascii_lowercase()
            .as_str()
        {
            "on" | "1" | "yes" | "enable" | "enabled" | "smart" | "power" => {
                crate::platform::yolo::set_smart(true)
            }
            "off" | "0" | "no" | "disable" | "disabled" | "guarded" => {
                if crate::platform::yolo::smart_enabled() {
                    crate::platform::yolo::set_smart(false);
                } else if crate::platform::yolo::enabled() {
                    return format!(
                        "YOLO SMART is already off — full YOLO is still active. {}\n\
                         use /yolo off to leave unrestricted mode",
                        crate::platform::yolo::status_text()
                    );
                } else {
                    crate::platform::yolo::set_smart(false);
                }
            }
            "" | "status" => return crate::platform::yolo::smart_status_text(),
            _ => {
                return "usage: /yolos [on|off|status] — powerful coding (workspace auto-approve, \
                        self-edit auto-merge, effectful code_mode); sandbox/timeouts/hooks stay on"
                    .to_string();
            }
        }
        crate::platform::yolo::smart_status_text()
    }

    pub(crate) fn toggle_comp(&mut self, arg: Option<&str>) -> String {
        let trimmed = arg.map(str::trim).unwrap_or("");
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("status") {
            return crate::drive::comp_mode::status_text();
        }

        let (subcmd, rest) = crate::drive::loop_ctl::split_word(trimmed);
        match subcmd.to_ascii_lowercase().as_str() {
            "on" | "1" | "yes" | "enable" | "enabled" | "lean" | "turbo" => {
                crate::drive::comp_mode::set(true);
                self.request_redraw("turbo comp mode enabled");
                crate::drive::comp_mode::status_text()
            }
            "off" | "0" | "no" | "disable" | "disabled" | "rich" => {
                crate::drive::comp_mode::set(false);
                self.request_redraw("turbo comp mode disabled");
                crate::drive::comp_mode::status_text()
            }
            "podrace" | "competition" | "race" => {
                crate::drive::comp_mode::engage_competition();
                self.request_redraw("turbo podrace engaged");
                let msg = self.loop_start_immediate(rest.to_string(), 0, false, true);
                format!("⚡ TURBO PODRACE ENGAGED (competition mode active) · {msg}")
            }
            "loop" | "go" | "now" | "run" => {
                crate::drive::comp_mode::set(true);
                self.request_redraw("turbo loop engaged");
                let msg = self.loop_start_immediate(rest.to_string(), 0, false, false);
                format!("⚡ TURBO LOOP ENGAGED · {msg}")
            }
            "handoff" | "rl" => {
                crate::drive::comp_mode::engage_competition();
                self.request_redraw("turbo handoff rl engaged");
                let msg = self.handoff_rl_start_immediate(rest.to_string(), 0, true);
                format!("⚡ TURBO HANDOFF RL ENGAGED · {msg}")
            }
            "flash" => {
                crate::drive::comp_mode::set(true);
                self.request_redraw("turbo flash mode enabled");
                if !rest.is_empty() {
                    let msg = self.loop_start_immediate(rest.to_string(), 0, false, false);
                    format!("⚡ TURBO FLASH ENGAGED · {msg}")
                } else {
                    "⚡ TURBO FLASH READY · miniworld detached · maximal speed active".to_string()
                }
            }
            _ => {
                "usage: /turbo [on|off|status|podrace <task>|loop <task>|flash <task>|handoff <task>] — ultra-lean high-speed harness"
                    .to_string()
            }
        }
    }

    pub(crate) fn toggle_experimental(&self, arg: Option<&str>) -> String {
        const FLAGS: &[(&str, &str, bool)] = &[
            ("fallback", "ANGEL_FALLBACK", true),
            ("approve", "ANGEL_SWARM_APPROVE", true),
            ("swarm-max", "ANGEL_SWARM_MAX", false),
            ("code-mode", "ANGEL_CODE_MODE", false),
            ("lsp", "ANGEL_LSP", false),
        ];
        match arg {
            None => {
                let lines = FLAGS
                    .iter()
                    .map(|(n, env, live)| {
                        let on = std::env::var_os(env).is_some();
                        format!(
                            "  {n:<10} {}{}",
                            if on { "on" } else { "off" },
                            if *live { "" } else { "  (restart to apply)" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("experimental flags — /experimental <name> toggles\n{lines}")
            }
            Some(name) => {
                let Some((_, env, live)) =
                    FLAGS.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(name))
                else {
                    return format!(
                        "unknown flag '{name}' — {}",
                        FLAGS
                            .iter()
                            .map(|(n, _, _)| *n)
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                };
                let was_on = std::env::var_os(env).is_some();
                if was_on {
                    unsafe { std::env::remove_var(env) };
                } else {
                    unsafe { std::env::set_var(env, "1") };
                }
                if *env == "ANGEL_FALLBACK" {
                    crate::agent::club::set_fallback_armed(!was_on);
                }
                format!(
                    "{name} → {}{}",
                    if was_on { "off" } else { "on" },
                    if *live {
                        " (live)"
                    } else {
                        " (takes effect next launch)"
                    }
                )
            }
        }
    }

    pub(crate) fn mcp_text(&self) -> String {
        let defs = self.tools.defs();
        let mut names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        names.sort_unstable();
        if names.is_empty() {
            return "no tools registered".to_string();
        }
        format!("tools ({}):\n  {}", names.len(), names.join("  "))
    }

    pub(crate) fn spawn_compact(&mut self) {
        const KEEP: usize = 8;
        let Some((sys_end, window_end)) =
            crate::agent::compaction::select_window(&self.history, KEEP)
        else {
            self.system_msg(format!(
                "nothing to compact — {} messages in context",
                self.history.len()
            ));
            return;
        };
        let dropped = window_end - sys_end;
        let store = self.tools.memory_store();
        let club = self.bag.in_hand_with_fallback();
        let aux = crate::app::bootstrap::utility_roster(&self.bag);
        let workspace = self.tools.current_workspace().to_path_buf();
        let wing = crate::agent::compaction::project_wing_for(&workspace);
        let source = crate::agent::compaction::provenance(&self.session.id, "compact");
        let live = store.is_live();
        let use_model = crate::agent::compaction::sync_compaction_uses_model();
        let chunk_threshold = crate::agent::compaction::configured_compact_chunk_tokens();
        let window: Vec<ChatMsg> = self.history[sys_end..window_end].to_vec();
        let summary_window = crate::agent::harness::compaction_summary_window(&window);
        let (tx, job) = BackgroundJob::channel("context compaction", "Retry /compact");
        std::thread::spawn(move || {
            if tx.is_cancelled() {
                return;
            }
            let started = std::time::Instant::now();
            let result = if use_model {
                let club = crate::agent::harness::summarizer_for(club, &aux);
                crate::agent::compaction::compact_window_with_state_budget(
                    club.as_ref(),
                    &summary_window,
                    &wing,
                    &source,
                    chunk_threshold,
                    chunk_threshold.saturating_mul(2),
                    live,
                )
            } else {
                crate::agent::compaction::compact_window_fast(
                    &summary_window,
                    &wing,
                    &source,
                    chunk_threshold.saturating_mul(2),
                    live,
                )
            };
            let elapsed_ms = started.elapsed().as_millis();
            let mode = if use_model { "model" } else { "local" };
            let outcome = match result {
                Some(result) => {
                    let n = result.drawers.len();
                    let deposits = if live && n > 0 {
                        Some(MemoryDepositBatch::new(
                            Arc::clone(&store),
                            result.drawers.clone(),
                            workspace.clone(),
                        ))
                    } else {
                        None
                    };
                    let message = if live && n > 0 {
                        format!(
                            "compacted {mode} in {elapsed_ms}ms — distilled {dropped} older messages; queued {n} note(s) \
                             for long-term memory filing; kept system + last {KEEP}"
                        )
                    } else {
                        format!(
                            "compacted {mode} in {elapsed_ms}ms — distilled {dropped} older messages into an inline \
                             summary; kept system + last {KEEP} (set ANGEL_MEMPALACE_CMD to also \
                             persist to memory)"
                        )
                    };
                    let mut note = ChatMsg::harness(result.inline_note);
                    note.recovery_context = crate::agent::club::recovery_context_refs(&window);
                    note.book_memory =
                        crate::agent::harness::book::continuity::compaction_memory(&window);
                    BgOutcome::Compact {
                        range: sys_end..window_end,
                        note: Box::new(note),
                        plan: result.plan_snapshot.map(ChatMsg::assistant).map(Box::new),
                        deposits,
                        message,
                    }
                }
                None => BgOutcome::Note(
                    "compact failed — the compactor returned nothing; history left intact"
                        .to_string(),
                ),
            };
            let _ = tx.send(outcome);
        });
        self.bg_job = Some(job);
        self.system_msg("compacting context… (kept responsive; result appears here)".to_string());
    }

    pub(crate) fn fork_session(&mut self) -> String {
        let mut forked = session::Session::new();
        forked.bind(self.tools.current_workspace());
        if let Err(error) = forked.save(&self.history) {
            return format!(
                "/fork: save failed; continuing in session {}: {error}",
                self.session.id
            );
        }
        let id = forked.id.clone();
        self.session = forked;
        self.emit_session_start(crate::agent::harness::SessionStartSource::Fork);
        format!("forked → session {id} (original left intact; now writing to the fork)")
    }

    pub(crate) fn archive_session(&mut self) -> String {
        let src = self.session.path().to_path_buf();
        let Some(dir) = src.parent().map(|p| p.join("archived")) else {
            return "archive: no session path".to_string();
        };
        let _ = std::fs::create_dir_all(&dir);
        let dst = dir.join(src.file_name().unwrap_or_default());
        match std::fs::rename(&src, &dst) {
            Ok(()) => format!("archived {} → {} — closing", self.session.id, dst.display()),
            Err(e) => format!("archive: {e} (closing anyway)"),
        }
    }

    pub(crate) fn delete_session(&mut self) -> String {
        let path = self.session.path().to_path_buf();
        match std::fs::remove_file(&path) {
            Ok(()) => format!("deleted session {} — closing", self.session.id),
            Err(e) => format!("delete: {e} (closing anyway)"),
        }
    }

    pub(crate) fn mention_file(
        &mut self,
        arg: Option<&str>,
    ) -> Option<crate::app::local_command::EvidenceTurn> {
        const MENTION_CAP_BYTES: usize = 20_000;

        let Some(path) = arg.map(str::trim).filter(|path| !path.is_empty()) else {
            self.system_msg("usage: /mention <file>".to_string());
            return None;
        };

        match crate::agent::harness::confined_read_prefix(
            self.tools.current_workspace(),
            Path::new(path),
            MENTION_CAP_BYTES,
        ) {
            Ok(prefix) if prefix.binary => {
                self.system_msg(format!(
                    "/mention: {path} is binary ({} bytes) — use /show for supported media",
                    prefix.total_bytes
                ));
                None
            }
            Ok(prefix) => {
                let raw_prefix_bytes = prefix.bytes.len();
                let mut content = String::from_utf8_lossy(&prefix.bytes).into_owned();
                let normalized_was_larger = content.len() > MENTION_CAP_BYTES;
                crate::agent::harness::truncate_to_char_boundary(&mut content, MENTION_CAP_BYTES);
                use crate::agent::harness::book::{d3_roles::pages, d4567_briefs::MENTION};
                let note = if prefix.truncated {
                    format!(
                        "\n{} cap={MENTION_CAP_BYTES} omitted={}",
                        pages(MENTION, [4, 5]),
                        prefix.total_bytes.saturating_sub(raw_prefix_bytes as u64)
                    )
                } else if normalized_was_larger {
                    format!("\n{} cap={MENTION_CAP_BYTES}", pages(MENTION, [6, 5]))
                } else {
                    String::new()
                };
                let path_label = serde_json::to_string(path)
                    .unwrap_or_else(|_| "\"<invalid path>\"".to_string());
                Some(crate::app::local_command::EvidenceTurn::new(
                    format!("{} {path}", pages(MENTION, [1])),
                    format!(
                        "{}\n\n<mentioned_file path={path_label}>\n{content}\n</mentioned_file>{note}",
                        pages(MENTION, [2, 3])
                    ),
                ))
            }
            Err(error) => {
                self.system_msg(format!("/mention: cannot read {path}: {error}"));
                None
            }
        }
    }

    pub(crate) fn lookup_keybind(&self, code: KeyCode, mods: KeyModifiers) -> Option<Action> {
        self.keybinds
            .iter()
            .find(|b| b.code == code && b.mods == mods)
            .map(|b| b.action)
    }

    pub(crate) fn do_action(&mut self, action: Action) {
        if matches!(
            action,
            Action::NextBox
                | Action::PrevMode
                | Action::NextMode
                | Action::OpenModel
                | Action::OpenThinking
        ) {
            self.launch_interaction();
        }
        if matches!(action, Action::Interrupt) && self.cancel_launch(true) {
            return;
        }
        match action {
            Action::NextBox if !self.flight_owner().is_agent_busy() => {
                self.bag.cycle();
                self.remember_brain_route();
            }
            Action::PrevMode if !self.flight_owner().is_agent_busy() => {
                self.bag.cycle_sub(false);
                self.remember_brain_route();
            }
            Action::NextMode if !self.flight_owner().is_agent_busy() => {
                self.bag.cycle_sub(true);
                self.remember_brain_route();
            }
            Action::NextBox | Action::PrevMode | Action::NextMode => {}
            Action::ToggleShell => self.toggle_shell(),
            Action::Interrupt => self.interrupt_idle_safe(),
            Action::Send => self.submit(),
            Action::ScrollUp => self.scroll_focused_view_up(1),
            Action::ScrollDown => self.scroll_focused_view_down(1),
            Action::ScrollTop => self.scroll_focused_view_top(),
            Action::ScrollBottom => self.scroll_focused_view_bottom(),
            Action::OpenModel => {
                self.open_agent_menu(crate::ui::agent_panel::controls::AgentMenuKind::Model)
            }
            Action::OpenThinking => {
                self.open_agent_menu(crate::ui::agent_panel::controls::AgentMenuKind::Thinking)
            }
        }
    }

    pub(crate) fn run_keymap(&mut self, arg: Option<&str>) -> String {
        match arg {
            None => {
                let mut s = keymap_text();
                if !self.keybinds.is_empty() {
                    s.push_str("\n  custom:");
                    for b in &self.keybinds {
                        s.push_str(&format!(
                            "\n    {} → {}",
                            fmt_key(b.code, b.mods),
                            action_name(b.action)
                        ));
                    }
                }
                s.push_str("\n  /keymap <key> <action> to bind · /keymap reset");
                s
            }
            Some("reset") => {
                self.keybinds.clear();
                "keymap reset to defaults".to_string()
            }
            Some(a) => {
                let mut it = a.splitn(2, char::is_whitespace);
                let key = it.next().unwrap_or("");
                let act = it.next().map(str::trim).unwrap_or("");
                match (parse_key(key), parse_action(act)) {
                    (Some((code, mods)), Some(action)) => {
                        self.keybinds
                            .retain(|b| !(b.code == code && b.mods == mods));
                        self.keybinds.push(Keybind { code, mods, action });
                        format!("bound {} → {}", fmt_key(code, mods), action_name(action))
                    }
                    (None, _) => format!("unknown key '{key}' (try: tab, esc, ^g, f5, left…)"),
                    (_, None) => format!(
                        "unknown action '{act}' (next-box, next-mode, prev-mode, shell, send, \
                         interrupt, scroll-up/down/top/bottom, model, thinking)"
                    ),
                }
            }
        }
    }

    pub(crate) fn import_codex(&mut self, arg: Option<&str>) -> String {
        let sessions = crate::app::codex_import::list(10);
        if sessions.is_empty() {
            return "no Codex sessions under $CODEX_HOME/sessions".to_string();
        }
        let pick = match arg {
            None => None,
            Some("latest") => Some(sessions[0].clone()),
            Some(n) => match n.parse::<usize>() {
                Ok(i) if (1..=sessions.len()).contains(&i) => Some(sessions[i - 1].clone()),
                _ => return format!("usage: /import <1..{}> | latest", sessions.len()),
            },
        };
        let Some(path) = pick else {
            let list = sessions
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    format!(
                        "  {}. {}",
                        i + 1,
                        p.file_name().unwrap_or_default().to_string_lossy()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            return format!("recent Codex sessions (/import <n> | latest):\n{list}");
        };
        match crate::app::codex_import::parse(&path) {
            Ok(mut history) if !history.is_empty() => {
                let turns = history
                    .iter()
                    .filter(|m| matches!(m.role, ChatRole::User))
                    .count();
                crate::app::bootstrap::refresh_history(
                    &self.bag,
                    self.tools.current_workspace(),
                    &mut history,
                );
                self.history = history;
                self.undone_exchange = None;
                self.parked_threads.clear();
                self.last_background_output = None;
                self.last_background_operation = None;
                let _ = self.steer_queue.drain();
                self.rebuild_display();
                self.scroll = 0;
                let _ = self.session.save(&self.history);
                format!(
                    "imported {} ({turns} turns) from Codex — foreign System instructions discarded; current-project prompt restored",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )
            }
            Ok(_) => "/import: that rollout had no messages".to_string(),
            Err(e) => format!("/import: {e}"),
        }
    }

    pub(crate) fn dump_transcript(&self, body: &str) -> String {
        let Ok(home) = std::env::var("HOME") else {
            return "/raw: HOME is unavailable; no transcript written".to_string();
        };
        if home.is_empty() {
            return "/raw: HOME is empty; no transcript written".to_string();
        }
        let path = std::path::Path::new(&home)
            .join(".angelX")
            .join("transcript.txt");
        if let Some(dir) = path.parent()
            && let Err(error) = std::fs::create_dir_all(dir)
        {
            return format!("/raw: create {}: {error}", dir.display());
        }
        match crate::platform::workspace_store::write_atomic(&path, body.as_bytes()) {
            Ok(()) => format!(
                "visible conversation ({} chars) → {} (copy-friendly Markdown)",
                body.chars().count(),
                path.display()
            ),
            Err(error) => format!("/raw: write {}: {error}", path.display()),
        }
    }

    pub(crate) fn run_skill(
        &mut self,
        arg: Option<&str>,
    ) -> Option<crate::app::local_command::EvidenceTurn> {
        if arg.is_some_and(|value| value.trim() == "check") {
            self.system_msg(crate::knowledge::skills::check_for(
                self.tools.current_workspace(),
            ));
            return None;
        }
        let skill_search = arg.and_then(|value| {
            let value = value.trim();
            if value == "search" {
                Some("")
            } else {
                value.strip_prefix("search ")
            }
        });
        if let Some(query) = skill_search {
            self.system_msg(crate::knowledge::skills::search_for(
                self.tools.current_workspace(),
                query,
            ));
            return None;
        }
        let Some(a) = arg else {
            let names = crate::knowledge::skills::list_for(self.tools.current_workspace());
            if names.is_empty() {
                self.system_msg(format!(
                    "no skills — drop <name>.md files in {} (or set ANGEL_SKILLS_DIR)",
                    crate::knowledge::skills::dir_display()
                ));
            } else {
                self.system_msg(format!(
                    "skills ({}): {}\n/skills <name>[,<name>...] [task] applies up to four in order · /skills search <query> filters locally · /skills check diagnoses sources",
                    names.len(),
                    names.join(", ")
                ));
            }
            return None;
        };
        let mut it = a.splitn(2, char::is_whitespace);
        let selector = it.next().unwrap_or("");
        let task = it.next().map(str::trim).filter(|s| !s.is_empty());
        let names = selector.split(',').collect::<Vec<_>>();
        const MAX_STACKED_SKILLS: usize = 4;
        const MAX_STACKED_SKILL_BYTES: usize = 128 * 1024;
        if names.iter().any(|name| name.is_empty()) {
            self.system_msg(
                "/skills: empty name in stack — use /skills <name>[,<name>...] [task]".to_string(),
            );
            return None;
        }
        if names.len() > MAX_STACKED_SKILLS {
            self.system_msg(format!(
                "/skills: stack has {} names; at most {MAX_STACKED_SKILLS} skills may be applied \
                 to one turn",
                names.len()
            ));
            return None;
        }
        let mut unique = std::collections::BTreeSet::new();
        if let Some(duplicate) = names.iter().find(|name| !unique.insert(**name)) {
            self.system_msg(format!(
                "/skills: duplicate skill '{duplicate}' in ordered stack"
            ));
            return None;
        }

        let catalog = crate::agent::harness::load_skills_for(self.tools.current_workspace());
        let mut selected = Vec::with_capacity(names.len());
        for name in &names {
            let Some(skill) = catalog.iter().find(|skill| skill.name == *name) else {
                self.system_msg(format!(
                    "/skills: no usable skill '{name}' — /skills to list; skill files must be \
                     regular UTF-8 files no larger than 256 KiB with a safe identifier"
                ));
                return None;
            };
            selected.push(skill);
        }
        let total_bytes = selected.iter().fold(0usize, |total, skill| {
            total.saturating_add(skill.body.len())
        });
        if total_bytes > MAX_STACKED_SKILL_BYTES {
            self.system_msg(format!(
                "/skills: selected instructions total {} KiB; stacked skill context is capped at \
                 {} KiB",
                total_bytes.div_ceil(1024),
                MAX_STACKED_SKILL_BYTES / 1024
            ));
            return None;
        }

        use crate::agent::harness::book::{d3_roles::pages, d4567_briefs::SKILLS};
        let operator_task = match task {
            Some(task) => task.to_string(),
            None if names.len() == 1 => format!("{} {}", pages(SKILLS, [1]), names[0]),
            None => format!("{} {}", pages(SKILLS, [2]), names.join(", ")),
        };
        Some(crate::app::local_command::EvidenceTurn::new(
            operator_task,
            skill_stack_evidence(&selected),
        ))
    }

    pub(crate) fn run_cartridge_command(
        &mut self,
        name: &str,
        arg: Option<&str>,
    ) -> Option<crate::app::local_command::EvidenceTurn> {
        let (cartridge, command) = crate::agent::harness::cartridges::command(name)?;
        let Some(task) = arg.map(str::trim).filter(|task| !task.is_empty()) else {
            self.system_msg(format!("/{} {}", command.name, command.usage));
            return None;
        };
        match cartridge.command_skills(command) {
            Ok(skills) => Some(crate::app::local_command::EvidenceTurn::new(
                task,
                skill_stack_evidence(&skills.iter().collect::<Vec<_>>()),
            )),
            Err(message) => {
                self.system_msg(message);
                None
            }
        }
    }

    fn commit_memories(&mut self, next: Vec<Arc<str>>, confirmation: String) -> String {
        if let Err(error) =
            crate::knowledge::memory::save_for(&next, self.tools.current_workspace())
        {
            return format!(
                "/memories: save failed; change not applied in this chat: {error}. Inspect /memories and retry the command."
            );
        }
        self.memories = next;
        confirmation
    }

    pub(crate) fn run_memories(&mut self, arg: Option<&str>) -> String {
        let Some(a) = arg else {
            if self.memories.is_empty() {
                return "no memories — /memories add <text>".to_string();
            }
            let list = self
                .memories
                .iter()
                .enumerate()
                .map(|(i, m)| format!("  {}. {m}", i + 1))
                .collect::<Vec<_>>()
                .join("\n");
            return format!(
                "memories ({}) — injected into every turn:\n{list}",
                self.memories.len()
            );
        };
        let mut it = a.splitn(2, char::is_whitespace);
        let sub = it.next().unwrap_or("");
        let rest = it.next().map(str::trim).unwrap_or("");
        match sub {
            "add" if !rest.is_empty() => {
                if let Err(error) = crate::knowledge::memory::validate_add(&self.memories, rest) {
                    return format!("/memories: {error}");
                }
                let mut next = self.memories.clone();
                next.push(Arc::from(rest));
                self.commit_memories(next, format!("remembered: {rest}"))
            }
            "add" => "usage: /memories add <text>".to_string(),
            "clear" => {
                let n = self.memories.len();
                self.commit_memories(Vec::new(), format!("cleared {n} memories"))
            }
            "forget" => match rest.parse::<usize>() {
                Ok(n) if (1..=self.memories.len()).contains(&n) => {
                    let mut next = self.memories.clone();
                    let m = next.remove(n - 1);
                    self.commit_memories(next, format!("forgot: {m}"))
                }
                _ => format!("usage: /memories forget <1..{}>", self.memories.len()),
            },
            "search" | "recall" if !rest.is_empty() => self.spawn_palace_search(rest),
            "search" | "recall" => "usage: /memories search <query>".to_string(),
            "palace" | "status" => {
                let store = self.tools.memory_store();
                if !store.is_live() {
                    return "long-term memory is not configured (set ANGEL_MEMPALACE_CMD)"
                        .to_string();
                }
                if self.bg_job.is_some() {
                    return "a background task is already running — try again in a moment"
                        .to_string();
                }
                let (tx, job) =
                    BackgroundJob::channel("long-term memory status", "Retry /memories palace");
                std::thread::spawn(move || {
                    let msg = match store.status() {
                        Ok(s) if !s.trim().is_empty() => {
                            format!("long-term memory (palace):\n{}", s.trim())
                        }
                        Ok(_) => "long-term memory is empty".to_string(),
                        Err(e) => format!("palace status unavailable: {e}"),
                    };
                    let _ = tx.send(BgOutcome::Note(msg));
                });
                self.bg_job = Some(job);
                "checking long-term memory…".to_string()
            }
            _ => {
                if let Err(error) = crate::knowledge::memory::validate_add(&self.memories, a) {
                    return format!("/memories: {error}");
                }
                let mut next = self.memories.clone();
                next.push(Arc::from(a));
                self.commit_memories(next, format!("remembered: {a}"))
            }
        }
    }

    pub(crate) fn run_recall(&mut self, arg: Option<&str>) -> String {
        match arg.map(str::trim).filter(|s| !s.is_empty()) {
            Some(query) => self.spawn_palace_search(query),
            None => "usage: /recall <query> — search long-term memory".to_string(),
        }
    }

    fn spawn_palace_search(&mut self, query: &str) -> String {
        let store = self.tools.memory_store();
        if !store.is_live() {
            return "long-term memory is not configured (set ANGEL_MEMPALACE_CMD)".to_string();
        }
        if self.bg_job.is_some() {
            return "a background task is already running — try again in a moment".to_string();
        }
        let wing = crate::agent::compaction::project_wing_for(self.tools.current_workspace());
        let q = query.to_string();
        let (tx, job) = BackgroundJob::channel("long-term memory search", "Retry /recall <query>");
        std::thread::spawn(move || {
            let msg = match store.search(&q, 8, Some(&wing)) {
                Ok(hits) if !hits.is_empty() => {
                    let body = hits
                        .iter()
                        .enumerate()
                        .map(|(i, h)| format!("{}. {}", i + 1, h.trim()))
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    format!("recall · {} hit(s) for “{q}”:\n{body}", hits.len())
                }
                Ok(_) => format!("recall · nothing filed for “{q}”"),
                Err(e) => format!("recall failed: {e}"),
            };
            let _ = tx.send(BgOutcome::Note(msg));
        });
        self.bg_job = Some(job);
        format!("searching long-term memory for “{query}”…")
    }

    pub(crate) fn spawn_quest_gauntlet(&mut self, question: &str) -> String {
        const MAX: usize = 6;
        let steer = crate::agent::harness::book::d3_roles::pages(
            crate::agent::harness::book::ing_drivers::STAND_INS,
            [2],
        );

        let question = question.trim();
        if question.is_empty() {
            return "usage: /quest gauntlet <question> — fan it to the fleet, chart every \
                    model's reasoning"
                .to_string();
        }
        if self.flight_owner().is_agent_busy() {
            return "busy — finish or interrupt the current turn before /quest gauntlet"
                .to_string();
        }
        let in_hand = self.bag.in_hand_label().to_string();
        let roster = select_chroniclers(self.bag.roster(), &in_hand, MAX);
        if roster.is_empty() {
            return "gauntlet: no chroniclers — every reachable club is the practice swing \
                    or a metered SOTA link"
                .to_string();
        }

        let n = roster.len();
        let question = question.to_string();
        let (tx, job) =
            BackgroundJob::channel("quest gauntlet", "Retry /quest gauntlet <question>");
        std::thread::spawn(move || {
            let q = question.as_str();
            let steer = steer.as_str();
            let results: Vec<(String, Result<String, String>)> = std::thread::scope(|scope| {
                let handles: Vec<_> = roster
                    .iter()
                    .map(|club| scope.spawn(move || chronicle_one(club.as_ref(), steer, q)))
                    .collect();
                handles
                    .into_iter()
                    .map(|h| {
                        h.join()
                            .unwrap_or_else(|_| ("a club".to_string(), Err("panicked".to_string())))
                    })
                    .collect()
            });

            let mut charted: Vec<(String, String)> = Vec::new();
            let mut skipped: Vec<String> = Vec::new();
            for (label, res) in results {
                match res {
                    Ok(trace) => charted.push((label, trace)),
                    Err(why) => skipped.push(format!("{label} ({why})")),
                }
            }
            let _ = tx.send(BgOutcome::Note(gauntlet_note(&charted, &skipped)));
        });
        self.bg_job = Some(job);
        format!(
            "gauntlet · fanning your question to {n} club(s)… (kept responsive; the \
             chronicle lands here)"
        )
    }
}

fn select_chroniclers(
    roster: Vec<Arc<dyn crate::agent::club::Club>>,
    in_hand: &str,
    max: usize,
) -> Vec<Arc<dyn crate::agent::club::Club>> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<Arc<dyn crate::agent::club::Club>> = Vec::new();
    for club in roster {
        let label = club.label().to_string();
        if label == "practice"
            || crate::agent::club::is_sota_label(&label)
            || club.reports_to_palace()
        {
            continue;
        }
        if seen.insert(label) {
            out.push(club);
        }
    }
    out.sort_by_key(|c| c.label() != in_hand);
    out.truncate(max);
    out
}

fn chronicle_one(
    club: &dyn crate::agent::club::Club,
    steer: &str,
    question: &str,
) -> (String, Result<String, String>) {
    let label = club
        .live_model_name()
        .map(|id| crate::agent::club::short_model_label(&id))
        .unwrap_or_else(|| club.label().to_string());
    if !club.is_available() {
        return (label, Err("unreachable".to_string()));
    }
    let msgs = [ChatMsg::system(steer), ChatMsg::user(question)];
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut reasoning = String::new();
    let mut content = String::new();
    let workspace = crate::agent::harness::book::connect::workspace();
    let outcome =
        crate::agent::harness::book::connect::converse(&workspace, &msgs, |history, tools| {
            club.chat_streaming(history, tools, &cancel, &mut |d| match d {
                crate::agent::club::StreamDelta::Reasoning(r) => reasoning.push_str(r),
                crate::agent::club::StreamDelta::Content(c) => content.push_str(c),
                crate::agent::club::StreamDelta::Heartbeat => {}
            })
        });
    match outcome {
        Ok(_) if !reasoning.trim().is_empty() => (label, Ok(reasoning)),
        Ok(_) if !content.trim().is_empty() => (label, Ok(content)),
        Ok(_) => (label, Err("answered with nothing to chart".to_string())),
        Err(e) => (label, Err(e.chars().take(48).collect())),
    }
}

fn gauntlet_note(charted: &[(String, String)], skipped: &[String]) -> String {
    if charted.is_empty() {
        return format!(
            "gauntlet · no club left a chronicle · sat out: {}",
            skipped.join(", ")
        );
    }
    let mut map = crate::stage::questmap::render_party(charted, 72);
    if !skipped.is_empty() {
        map.push_str(&format!("\n\nsat out: {}", skipped.join(", ")));
    }
    map
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__session_meta__recap_tests.rs"]
mod recap_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__session_meta__gauntlet_tests.rs"]
mod gauntlet_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__session_meta__session_start_tests.rs"]
mod session_start_tests;

fn skill_stack_evidence(selected: &[&crate::agent::harness::Skill]) -> String {
    use crate::agent::harness::book::{d3_roles::pages, d4567_briefs::SKILLS};
    let mut evidence = format!(
        "{} {}\n{}",
        pages(SKILLS, [3]),
        selected.len(),
        pages(SKILLS, [4])
    );
    for skill in selected {
        let name_label = serde_json::to_string(&skill.name)
            .unwrap_or_else(|_| "\"<invalid skill name>\"".to_string());
        evidence.push_str(&format!(
            "\n\n<selected_skill name={name_label}>\n{}\n</selected_skill>",
            skill.body
        ));
    }
    evidence
}
