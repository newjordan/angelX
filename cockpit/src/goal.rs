//! The long-running **goal**: a durable, structured objective for a session.
//!
//! Unlike a chat message, the goal persists across restarts and — critically — is
//! injected into non-casual agent turns (see `App::submit`), so the model actually
//! steers toward it. It can carry acceptance criteria and an optional *verifiable*
//! command (a test/build/lint invocation) that the [`loop_ctl`](crate::loop_ctl)
//! controller uses as ground-truth done-detection rather than trusting the model's
//! own "I'm done".
//!
//! Backed by a canonical-project-keyed file under `~/.angel0/goals/` (override
//! with `ANGEL_GOAL_FILE`, used by tests so they never touch the real store).
//! Both the filename and serialized binding are checked on load: an unscoped
//! legacy goal or a record from another project is ignored rather than injected.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Opening line of the injected goal block, and its closing sentinel. Kept as
/// constants so the submit path can strip a prior block as a precise span (header
/// line … sentinel line) and keep only the freshest copy per turn.
pub(crate) const GOAL_BLOCK_HEADER: &str =
    "[goal — standing objective; keep every action aligned to it]";
pub(crate) const GOAL_BLOCK_SENTINEL: &str = "[/goal]";
pub(crate) const MAX_GOAL_RECORD_BYTES: usize = 512 * 1024;
pub(crate) const MAX_GOAL_TEXT_BYTES: usize = 8192;
pub(crate) const MAX_GOAL_ITEM_BYTES: usize = 1024;
pub(crate) const MAX_GOAL_COMMAND_BYTES: usize = 4096;
pub(crate) const MAX_GOAL_ACCEPTANCE_ITEMS: usize = 32;
pub(crate) const MAX_GOAL_NOTES: usize = 256;
pub(crate) const MAX_GOAL_CONTEXT_BYTES: usize = 64 * 1024;
/// A blocking condition must be ticked this many consecutive rounds (with the
/// identical reason) before the goal flips to `Blocked` — a transient failure
/// can never stop a long-running objective on its own.
pub(crate) const GOAL_BLOCKED_MIN_ROUNDS: u32 = 3;
pub(crate) const MAX_GOAL_BLOCKED_REASON_BYTES: usize = 1024;
/// Hard ceiling for a `/goal rounds N` budget (kept well below u32::MAX so a
/// typo can never mint an effectively unbounded autonomous loop).
pub(crate) const MAX_GOAL_ROUNDS_HARD: u32 = 10_000;

/// Where the goal stands. Only an `Active` goal is injected into turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    /// Being pursued — injected into every turn.
    #[default]
    Active,
    /// Met (manually, or verified by the loop's acceptance command).
    Done,
    /// Set aside without completing; re-armable via `/goal resume`. Canonical
    /// mission vocabulary is `paused`; legacy records with `"abandoned"` load
    /// here as a read alias and are re-written as `paused` on the next save.
    #[serde(rename = "paused", alias = "abandoned")]
    Paused,
    /// The same concrete blocking condition persisted for
    /// [`GOAL_BLOCKED_MIN_ROUNDS`] consecutive rounds. Terminal for autonomous
    /// continuation — turns stop inheriting it — but the operator may
    /// `/goal resume` once the blocker clears.
    Blocked,
}

/// A durable, structured objective.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Goal {
    /// Canonical main-worktree root and stable key this goal belongs to. Legacy
    /// records lack these fields and intentionally fail closed in `load_for`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_key: Option<String>,
    /// The objective in one line / paragraph.
    pub text: String,
    /// Acceptance criteria — the definition of done, in the user's words.
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Optional verifiable check: a shell/cargo command whose success means the
    /// goal is *actually* met (ground truth for the loop's done-detection).
    #[serde(default)]
    pub accept_cmd: Option<String>,
    /// Progress notes appended over time.
    #[serde(default)]
    pub notes: Vec<String>,
    /// Lifecycle status.
    #[serde(default)]
    pub status: GoalStatus,
    /// Continuation rounds completed (`/goal tick`). Bounded by `max_rounds`
    /// only when the operator explicitly sets it.
    #[serde(default)]
    pub rounds: u32,
    /// Optional budget on autonomous continuation rounds (`/goal rounds N`).
    /// `None` = unbounded. Rounds already spent are never reset by edits.
    #[serde(default)]
    pub max_rounds: Option<u32>,
    /// The concrete condition currently blocking progress, and how many
    /// consecutive rounds it has persisted (`/goal blocked <reason>`).
    #[serde(default)]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub blocked_streak: u32,
    /// Creation / last-update timestamps (unix ms).
    #[serde(default)]
    pub created_ms: u64,
    #[serde(default)]
    pub updated_ms: u64,
}

impl Goal {
    /// A fresh active goal with the given text.
    pub fn new(text: impl Into<String>) -> Self {
        let now = now_ms();
        Self {
            workspace: None,
            project_key: None,
            text: text.into(),
            acceptance: Vec::new(),
            accept_cmd: None,
            notes: Vec::new(),
            status: GoalStatus::Active,
            rounds: 0,
            max_rounds: None,
            blocked_reason: None,
            blocked_streak: 0,
            created_ms: now,
            updated_ms: now,
        }
    }

    /// Whether another autonomous continuation round is allowed. `Blocked` /
    /// `Done` / `Paused` goals and a spent round budget are all false —
    /// continuing past any of them is an explicit operator act.
    pub(crate) fn can_continue(&self) -> bool {
        self.status == GoalStatus::Active && self.max_rounds.is_none_or(|cap| self.rounds < cap)
    }

    /// Record one completed continuation round. Returns `false` (and leaves
    /// state untouched) when the round budget is spent or the goal is not
    /// active. A real continuation round clears any blocking candidates — the
    /// blocker demonstrably did not stop progress.
    pub(crate) fn record_round(&mut self) -> bool {
        if !self.can_continue() {
            return false;
        }
        self.rounds = self.rounds.saturating_add(1);
        self.blocked_reason = None;
        self.blocked_streak = 0;
        self.updated_ms = now_ms();
        true
    }

    /// Record one blocked round. The status flips to `Blocked` only after the
    /// SAME reason has persisted for [`GOAL_BLOCKED_MIN_ROUNDS`] consecutive
    /// rounds; a changing reason restarts the streak and the goal stays
    /// active. Returns `true` when this tick made the goal terminal.
    pub(crate) fn tick_blocked(&mut self, reason: &str) -> bool {
        if self.status != GoalStatus::Active {
            return false;
        }
        let reason = reason.trim();
        self.blocked_streak = if self.blocked_reason.as_deref() == Some(reason) {
            self.blocked_streak.saturating_add(1)
        } else {
            1
        };
        self.blocked_reason = Some(reason.to_string());
        self.rounds = self.rounds.saturating_add(1);
        self.updated_ms = now_ms();
        if self.blocked_streak >= GOAL_BLOCKED_MIN_ROUNDS {
            self.status = GoalStatus::Blocked;
            return true;
        }
        false
    }

    /// Re-arm a `Blocked` (or `Paused`) goal. Keeps rounds, budget,
    /// criteria, and notes — only the blocker state clears. Terminal-by-completion
    /// (`Done`) goals are not resumable: mint a new one.
    pub(crate) fn rearm(&mut self) -> bool {
        match self.status {
            GoalStatus::Blocked | GoalStatus::Paused => {
                self.status = GoalStatus::Active;
                self.blocked_reason = None;
                self.blocked_streak = 0;
                self.updated_ms = now_ms();
                true
            }
            _ => false,
        }
    }

    /// Copy this goal for explicit campaign import only when it is active and
    /// bound to the same canonical project. The campaign receives a snapshot;
    /// authoring it never mutates the standing `/goal` record.
    pub(crate) fn campaign_snapshot_for(&self, workspace: &Path) -> Option<Self> {
        (self.status == GoalStatus::Active && matches_workspace(self, workspace))
            .then(|| self.clone())
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn explicit_goal_file() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ANGEL_GOAL_FILE")
        && !p.trim().is_empty()
    {
        return Some(PathBuf::from(p));
    }
    None
}

fn binding(workspace: &Path) -> crate::workspace_store::RepoIdentity {
    crate::workspace_store::repo_identity(workspace)
}

fn store_path_for(workspace: &Path) -> PathBuf {
    if let Some(path) = explicit_goal_file() {
        return path;
    }
    let identity = binding(workspace);
    crate::workspace_store::workspace_json_path_in(
        &crate::workspace_store::angel_subdir("goals"),
        &identity.root,
    )
}

fn matches_workspace(goal: &Goal, workspace: &Path) -> bool {
    match (goal.workspace.as_deref(), goal.project_key.as_deref()) {
        (Some(root), Some(key)) => crate::workspace_store::matches_project(workspace, root, key),
        _ => false,
    }
}

/// Load only the goal bound to `workspace`. Missing, malformed, legacy-global,
/// or cross-project records are all inert.
pub fn load_for(workspace: &Path) -> Option<Goal> {
    let raw = read_bounded_utf8(&store_path_for(workspace), MAX_GOAL_RECORD_BYTES)?;
    let goal: Goal = serde_json::from_str(&raw).ok()?;
    (matches_workspace(&goal, workspace) && goal_within_limits(&goal)).then_some(goal)
}

/// Persist the goal atomically and durably. Callers must surface failure: a
/// control command must never claim a restart-safe transition that did not
/// cross the storage boundary.
pub fn save_for(goal: &mut Goal, workspace: &Path) -> Result<(), String> {
    if !goal_within_limits(goal) {
        return Err("goal record violates configured bounds".to_string());
    }
    let identity = binding(workspace);
    goal.workspace = Some(identity.root);
    goal.project_key = Some(identity.key);
    let path = store_path_for(workspace);
    if path.exists() {
        let existing_matches = read_bounded_utf8(&path, MAX_GOAL_RECORD_BYTES)
            .and_then(|raw| serde_json::from_str::<Goal>(&raw).ok())
            .is_some_and(|existing| matches_workspace(&existing, workspace));
        if !existing_matches {
            return Err("goal store belongs to another project or is unreadable".to_string());
        }
    }
    let json = serde_json::to_vec_pretty(goal).map_err(|error| format!("encode goal: {error}"))?;
    crate::workspace_store::write_private_atomic(&path, &json)
}

/// Remove the persisted goal file and durably publish the deletion.
pub fn clear_for(workspace: &Path) -> Result<(), String> {
    let path = store_path_for(workspace);
    let owned = read_bounded_utf8(&path, MAX_GOAL_RECORD_BYTES)
        .and_then(|raw| serde_json::from_str::<Goal>(&raw).ok())
        .is_some_and(|goal| matches_workspace(&goal, workspace));
    if owned {
        crate::workspace_store::remove_durable(&path)
    } else if path.exists() {
        Err("goal store belongs to another project or is unreadable".to_string())
    } else {
        Ok(())
    }
}

fn goal_persist_error(action: &str, error: impl std::fmt::Display) -> String {
    format!(
        "/goal {action}: durability checkpoint failed ({error}); the in-memory state may be newer, so inspect `/goal show` and retry before restarting"
    )
}

fn goal_within_limits(goal: &Goal) -> bool {
    !goal.text.trim().is_empty()
        && goal.text.len() <= MAX_GOAL_TEXT_BYTES
        && goal.acceptance.len() <= MAX_GOAL_ACCEPTANCE_ITEMS
        && goal
            .acceptance
            .iter()
            .all(|item| !item.trim().is_empty() && item.len() <= MAX_GOAL_ITEM_BYTES)
        && goal.accept_cmd.as_ref().is_none_or(|command| {
            !command.trim().is_empty() && command.len() <= MAX_GOAL_COMMAND_BYTES
        })
        && goal.notes.len() <= MAX_GOAL_NOTES
        && goal
            .notes
            .iter()
            .all(|note| !note.trim().is_empty() && note.len() <= MAX_GOAL_ITEM_BYTES)
}

fn read_bounded_utf8(path: &Path, max_bytes: usize) -> Option<String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = Path::new(path.file_name()?);
    let bytes = crate::harness::confined_read_limited(parent, name, max_bytes)
        .ok()
        .flatten()?;
    String::from_utf8(bytes).ok()
}

impl crate::App {
    /// The goal the next turn should steer by, read through to the canonical
    /// store. Agent tools (`goal` set/clear) write the store directly — they
    /// hold no `&mut App` — so reading through here (rather than `self.goal`)
    /// makes a goal the model just set or cleared effective on the very next
    /// turn without a restart or `/cd`. The operator's `/goal` verb persists
    /// every mutation, so the store is the single source of truth and this
    /// equals `self.goal` except immediately after an agent tool write.
    fn current_goal(&self) -> Option<Goal> {
        load_for(self.tools.current_workspace())
    }

    /// Re-sync the in-memory goal from the canonical store. Local goal-reading
    /// commands (`/status`, `/goal`, `/loop`, `/campaign`) call this so they
    /// reflect a goal the agent's `goal` tool just wrote; `launch_pending_turn`
    /// calls it after the operator echo paints so Enter is not blocked on the
    /// store read. `/goal` persists every mutation, so the store is authoritative
    /// and this reload is cheap and idempotent.
    pub(crate) fn refresh_goal_from_disk(&mut self) {
        self.goal = load_for(self.tools.current_workspace());
    }

    /// Render the active goal as bounded Harness-role turn context. Empty string
    /// unless there's a valid `Active` goal bound to this workspace.
    pub(crate) fn goal_context_block(&self, raw: Option<&str>) -> String {
        let Some(g) = self.current_goal() else {
            return String::new();
        };
        let g = &g;
        if raw.is_some_and(casual_goal_bypass)
            || !matches_workspace(g, self.tools.current_workspace())
            || g.status != GoalStatus::Active
            || g.text.trim().is_empty()
            || !goal_within_limits(g)
        {
            return String::new();
        }
        let objective = serde_json::to_string(g.text.trim())
            .unwrap_or_else(|_| "\"<invalid objective>\"".to_string());
        let mut out = format!("{GOAL_BLOCK_HEADER}\nobjective: {objective}\n");
        let fits = |current: &str, fragment: &str| {
            current.len() + fragment.len() + GOAL_BLOCK_SENTINEL.len() + 128
                <= MAX_GOAL_CONTEXT_BYTES
        };
        let mut omitted = 0usize;
        if !g.acceptance.is_empty() {
            out.push_str("acceptance criteria:\n");
            for (index, c) in g.acceptance.iter().enumerate() {
                let criterion = serde_json::to_string(c)
                    .unwrap_or_else(|_| "\"<invalid criterion>\"".to_string());
                let line = format!("- {criterion}\n");
                if !fits(&out, &line) {
                    omitted += g.acceptance.len() - index;
                    break;
                }
                out.push_str(&line);
            }
        }
        if let Some(cmd) = &g.accept_cmd {
            let command =
                serde_json::to_string(cmd).unwrap_or_else(|_| "\"<invalid command>\"".to_string());
            let line = format!("verifiable check (must pass): {command}\n");
            if fits(&out, &line) {
                out.push_str(&line);
            } else {
                omitted += 1;
            }
        }
        if g.max_rounds.is_some() || g.blocked_reason.is_some() {
            let mut state = String::new();
            if let Some(cap) = g.max_rounds {
                state.push_str(&format!("round budget: {}/{}", g.rounds, cap));
            }
            if let Some(reason) = &g.blocked_reason {
                if !state.is_empty() {
                    state.push_str("; ");
                }
                state.push_str(&format!(
                    "blocked ({}/{GOAL_BLOCKED_MIN_ROUNDS}): {reason}",
                    g.blocked_streak
                ));
            }
            let line = format!("progress state: {state}\n");
            if fits(&out, &line) {
                out.push_str(&line);
            } else {
                omitted += 1;
            }
        }
        if !g.notes.is_empty() {
            out.push_str("progress so far:\n");
            // Only the last few notes — the running log can grow without bound.
            let notes = g.notes.iter().rev().take(5).rev().collect::<Vec<_>>();
            for (index, n) in notes.iter().enumerate() {
                let note =
                    serde_json::to_string(n).unwrap_or_else(|_| "\"<invalid note>\"".to_string());
                let line = format!("- {note}\n");
                if !fits(&out, &line) {
                    omitted += notes.len() - index;
                    break;
                }
                out.push_str(&line);
            }
        }
        if omitted > 0 {
            out.push_str(&format!(
                "[harness omitted {omitted} goal field(s) outside the bounded context]\n"
            ));
        }
        // Closing sentinel + trailing blank line so the submit path can strip a
        // prior goal block as a precise span and keep only the freshest copy.
        out.push_str(GOAL_BLOCK_SENTINEL);
        out.push_str("\n\n");
        out
    }

    /// Gate a goal-driving loop and durably reserve one continuation round.
    /// Explicit status/budget gates return a reason; checkpoint failures return
    /// an error without publishing candidate counters or clearing blockers.
    pub(crate) fn loop_goal_gate(&mut self) -> Result<Option<String>, String> {
        let workspace = self.tools.current_workspace().to_path_buf();
        let Some(g) = self.goal.as_mut() else {
            return Ok(None);
        };
        if !matches_workspace(g, &workspace) {
            return Ok(None);
        }
        let reason = match g.status {
            GoalStatus::Blocked => Some(format!(
                "goal is blocked ({})",
                g.blocked_reason.clone().unwrap_or_default()
            )),
            GoalStatus::Done => Some("goal is done".to_string()),
            GoalStatus::Paused => Some("goal is paused — /goal resume to re-arm".to_string()),
            GoalStatus::Active => None,
        };
        if let Some(reason) = reason {
            return Ok(Some(reason));
        }
        let mut candidate = g.clone();
        if !candidate.record_round() {
            return Ok(Some(match g.max_rounds {
                Some(cap) => format!("goal round budget ({}/{})", g.rounds, cap),
                None => "goal cannot continue".to_string(),
            }));
        }
        save_for(&mut candidate, &workspace)
            .map_err(|error| format!("goal durability checkpoint failed ({error})"))?;
        *g = candidate;
        Ok(None)
    }

    /// One-line goal summary for `/status`.
    pub(crate) fn goal_status_line(&self) -> String {
        match self.goal.as_ref() {
            None => "(none — /goal <text>)".to_string(),
            Some(g) if !matches_workspace(g, self.tools.current_workspace()) => {
                "(none for this project — /goal <text>)".to_string()
            }
            Some(g) => {
                let status = match g.status {
                    GoalStatus::Active => "active",
                    GoalStatus::Done => "done",
                    GoalStatus::Paused => "paused",
                    GoalStatus::Blocked => "blocked",
                };
                let budget = match g.max_rounds {
                    Some(cap) => format!("r{}/{}", g.rounds, cap),
                    None => format!("r{}", g.rounds),
                };
                let blocked = g
                    .blocked_reason
                    .as_ref()
                    .map(|reason| {
                        format!(
                            " · blocked({}/{GOAL_BLOCKED_MIN_ROUNDS}): {reason}",
                            g.blocked_streak
                        )
                    })
                    .unwrap_or_default();
                let extras = {
                    let mut parts = Vec::new();
                    if !g.acceptance.is_empty() {
                        parts.push(format!("{} criteria", g.acceptance.len()));
                    }
                    if g.accept_cmd.is_some() {
                        parts.push("verifiable".to_string());
                    }
                    if parts.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", parts.join(", "))
                    }
                };
                format!("{} [{status} · {budget}{blocked}]{extras}", g.text.trim())
            }
        }
    }

    /// `/goal`: set / show / clear / refine the long-running goal. Subcommands are
    /// parsed from the raw arg so `input.rs` needs no special shape.
    pub(crate) fn update_goal(&mut self, arg: Option<String>) -> String {
        let Some(raw) = arg else {
            return self.goal_show();
        };
        let raw = raw.trim();
        if raw.len() > MAX_GOAL_TEXT_BYTES {
            return format!(
                "/goal: input is {} bytes; maximum is {MAX_GOAL_TEXT_BYTES}",
                raw.len()
            );
        }
        let lower = raw.to_ascii_lowercase();
        // Bare keywords.
        match lower.as_str() {
            "" | "status" | "show" => return self.goal_show(),
            "clear" | "none" | "off" => {
                if let Err(error) = clear_for(self.tools.current_workspace()) {
                    return goal_persist_error("clear", error);
                }
                self.goal = None;
                self.start_lifecycle_ceremony(
                    crate::viz::lifecycle_viz::CeremonyKind::GoalCleared,
                    "goal cleared",
                );
                return "goal cleared".to_string();
            }
            "done" | "complete" | "achieved" => {
                let workspace = self.tools.current_workspace().to_path_buf();
                return match self.goal.as_mut() {
                    Some(g) => {
                        g.status = GoalStatus::Done;
                        g.blocked_reason = None;
                        g.blocked_streak = 0;
                        g.updated_ms = now_ms();
                        let label = g.text.clone();
                        if let Err(error) = save_for(g, &workspace) {
                            return goal_persist_error("done", error);
                        }
                        self.start_lifecycle_ceremony(
                            crate::viz::lifecycle_viz::CeremonyKind::GoalDone,
                            label,
                        );
                        "goal marked done".to_string()
                    }
                    None => "no goal set — /goal <text> to set one".to_string(),
                };
            }
            "pause" | "abandon" | "drop" => {
                let workspace = self.tools.current_workspace().to_path_buf();
                return match self.goal.as_mut() {
                    Some(g) => {
                        g.status = GoalStatus::Paused;
                        g.blocked_reason = None;
                        g.blocked_streak = 0;
                        g.updated_ms = now_ms();
                        if let Err(error) = save_for(g, &workspace) {
                            return goal_persist_error("pause", error);
                        }
                        self.start_lifecycle_ceremony(
                            crate::viz::lifecycle_viz::CeremonyKind::GoalCleared,
                            "goal paused",
                        );
                        "goal paused — /goal resume re-arms it".to_string()
                    }
                    None => "no goal set".to_string(),
                };
            }
            // One bounded continuation round. A real round clears any blocking
            // candidates — the blocker demonstrably did not stop progress.
            "tick" | "round" => {
                let workspace = self.tools.current_workspace().to_path_buf();
                return match self.goal.as_mut() {
                    Some(g) => {
                        if !g.record_round() {
                            let reason = match g.status {
                                GoalStatus::Blocked => {
                                    "goal is blocked — /goal resume after the blocker clears"
                                }
                                GoalStatus::Done => "goal is done",
                                GoalStatus::Paused => "goal is paused — /goal resume to re-arm it",
                                GoalStatus::Active => {
                                    return format!(
                                        "/goal tick: round budget reached — operator cap /goal rounds {}; /goal rounds unset to continue",
                                        g.max_rounds.unwrap_or(0)
                                    );
                                }
                            };
                            format!("/goal tick: {reason}")
                        } else {
                            let msg = match g.max_rounds {
                                Some(cap) => {
                                    let spent = if g.can_continue() {
                                        String::new()
                                    } else {
                                        " — budget spent".to_string()
                                    };
                                    format!("round {}/{} recorded{spent}", g.rounds, cap)
                                }
                                None => format!("round {} recorded (unbounded)", g.rounds),
                            };
                            if let Err(error) = save_for(g, &workspace) {
                                return goal_persist_error("tick", error);
                            }
                            msg
                        }
                    }
                    None => "no goal set — /goal <text> to set one".to_string(),
                };
            }
            // Re-arm a blocked/paused goal. Keeps rounds, budget, criteria,
            // cmd, and notes — only the blocker state clears.
            "resume" => {
                let workspace = self.tools.current_workspace().to_path_buf();
                return match self.goal.as_mut() {
                    Some(g) if g.status == GoalStatus::Done => {
                        "a done goal is not resumable — /goal <text> to mint a new one".to_string()
                    }
                    Some(g) => {
                        if g.rearm() {
                            if let Err(error) = save_for(g, &workspace) {
                                return goal_persist_error("resume", error);
                            }
                            format!("goal re-armed: {}", g.text.trim())
                        } else {
                            "goal is already active".to_string()
                        }
                    }
                    None => "no goal set".to_string(),
                };
            }
            // Drive at the goal: hand off to the loop controller so setting an
            // objective and pursuing it are one gesture, not two commands apart.
            "go" | "run" | "drive" | "pursue" => {
                let active = self
                    .goal
                    .as_ref()
                    .is_some_and(|g| g.status == GoalStatus::Active && !g.text.trim().is_empty());
                if !active {
                    return "no active goal to drive — /goal <text> first, then /goal go"
                        .to_string();
                }
                if self.loop_ctl.status == crate::loop_ctl::LoopStatus::Paused {
                    return self.loop_command(Some("resume".to_string()));
                }
                if self.loop_active() {
                    return "a loop is already driving — /loop status (or /loop stop to restart)"
                        .to_string();
                }
                return self.loop_command(Some("start".to_string()));
            }
            _ => {}
        }
        // `<subcommand> <rest>` forms that refine an existing goal.
        if let Some(rest) = strip_word(raw, &["criteria", "accept", "+"]) {
            if rest.len() > MAX_GOAL_ITEM_BYTES {
                return format!(
                    "/goal: acceptance criterion is {} bytes; maximum is {MAX_GOAL_ITEM_BYTES}",
                    rest.len()
                );
            }
            if self
                .goal
                .as_ref()
                .is_some_and(|goal| goal.acceptance.len() >= MAX_GOAL_ACCEPTANCE_ITEMS)
            {
                return format!(
                    "/goal: acceptance list is full at {MAX_GOAL_ACCEPTANCE_ITEMS} items"
                );
            }
            return self.goal_refine(|g| {
                g.acceptance.push(rest.to_string());
                format!("acceptance criterion added ({} total)", g.acceptance.len())
            });
        }
        if let Some(rest) = strip_word(raw, &["cmd", "verify", "check"]) {
            if rest.len() > MAX_GOAL_COMMAND_BYTES {
                return format!(
                    "/goal: verification command is {} bytes; maximum is \
                     {MAX_GOAL_COMMAND_BYTES}",
                    rest.len()
                );
            }
            let mut msg = self.goal_refine(|g| {
                g.accept_cmd = Some(rest.to_string());
                format!("verifiable check set: {rest}")
            });
            // A live loop pinned its accept_cmd at start (anti-reward-hack); the
            // operator rebinding it here is the intended override, so re-pin the
            // run too — `/goal cmd <check>` then `/loop resume` just works.
            if self.goal.is_some() {
                match self.repin_loop_accept_cmd(rest) {
                    Some(true) => msg.push_str(
                        "\n  re-pinned on the live loop — capturing a fresh baseline \
                         against the new check now",
                    ),
                    Some(false)
                        if self.loop_ctl.status == crate::loop_ctl::LoopStatus::Baselining
                            && self.loop_ctl.baseline_resume_to.is_some() =>
                    {
                        msg.push_str("\n  re-pinned on the live loop — fresh baseline queued until existing work drains")
                    }
                    Some(false) => msg.push_str(
                        "\n  re-pinned on the live loop — run is mid-flight, so the \
                         pass-count regression guard stays OFF until you re-issue \
                         `/goal cmd` while it's quiet",
                    ),
                    None => {}
                }
            }
            return msg;
        }
        if let Some(rest) = strip_word(raw, &["note"]) {
            if rest.len() > MAX_GOAL_ITEM_BYTES {
                return format!(
                    "/goal: progress note is {} bytes; maximum is {MAX_GOAL_ITEM_BYTES}",
                    rest.len()
                );
            }
            if self
                .goal
                .as_ref()
                .is_some_and(|goal| goal.notes.len() >= MAX_GOAL_NOTES)
            {
                return format!("/goal: progress log is full at {MAX_GOAL_NOTES} notes");
            }
            return self.goal_refine(|g| {
                g.notes.push(rest.to_string());
                format!("progress note added ({} total)", g.notes.len())
            });
        }
        // `/goal blocked <reason>`: DSH blocked semantics — the status flips to
        // Blocked only after the SAME reason persists for GOAL_BLOCKED_MIN_ROUNDS
        // consecutive rounds; a changing reason restarts the streak.
        if let Some(reason) = strip_word(raw, &["blocked", "block", "stuck"]) {
            if reason.len() > MAX_GOAL_BLOCKED_REASON_BYTES {
                return format!(
                    "/goal: blocked reason is {} bytes; maximum is {MAX_GOAL_BLOCKED_REASON_BYTES}",
                    reason.len()
                );
            }
            let workspace = self.tools.current_workspace().to_path_buf();
            return match self.goal.as_mut() {
                Some(g) => {
                    let terminal = g.tick_blocked(reason);
                    if let Err(error) = save_for(g, &workspace) {
                        return goal_persist_error("blocked", error);
                    }
                    if terminal {
                        format!(
                            "goal BLOCKED after {GOAL_BLOCKED_MIN_ROUNDS} consecutive rounds \
                             with the same reason: {reason}\n  /goal resume re-arms it once \
                             the blocker clears"
                        )
                    } else {
                        format!(
                            "blocking round recorded ({}/{GOAL_BLOCKED_MIN_ROUNDS}): {reason}",
                            g.blocked_streak
                        )
                    }
                }
                None => "no goal set — /goal <text> to set one".to_string(),
            };
        }
        // `/goal rounds <N|unset>`: bound autonomous continuation.
        if let Some(rest) = strip_word(raw, &["rounds", "budget"]) {
            let workspace = self.tools.current_workspace().to_path_buf();
            return match self.goal.as_mut() {
                Some(g) => {
                    if matches!(
                        rest.to_ascii_lowercase().as_str(),
                        "unset" | "none" | "off" | "unbounded"
                    ) {
                        g.max_rounds = None;
                        g.updated_ms = now_ms();
                        if let Err(error) = save_for(g, &workspace) {
                            return goal_persist_error("rounds", error);
                        }
                        format!(
                            "round budget removed — goal is unbounded ({} rounds spent)",
                            g.rounds
                        )
                    } else {
                        match rest.parse::<u32>() {
                            Ok(0) => "/goal rounds: 0 is not a budget — use 'unset' for unbounded"
                                .to_string(),
                            Ok(n) if n > MAX_GOAL_ROUNDS_HARD => {
                                format!("/goal rounds: maximum is {MAX_GOAL_ROUNDS_HARD}")
                            }
                            Ok(n) => {
                                g.max_rounds = Some(n);
                                g.updated_ms = now_ms();
                                if let Err(error) = save_for(g, &workspace) {
                                    return goal_persist_error("rounds", error);
                                }
                                if g.rounds >= n && g.status == GoalStatus::Active {
                                    format!(
                                        "round budget set to {n} — already spent ({}/{}); \
                                         /goal done or raise it",
                                        g.rounds, n
                                    )
                                } else {
                                    format!("round budget set to {n} ({} spent)", g.rounds)
                                }
                            }
                            Err(_) => "/goal rounds: expected a number, or 'unset'".to_string(),
                        }
                    }
                }
                None => "no goal set — /goal <text> to set one".to_string(),
            };
        }
        // Otherwise: set/replace the goal text (becomes Active). Setting a goal
        // steers turns but starts nothing — say what actually happens next, so
        // "/goal did nothing" can't be the takeaway.
        let msg = if self.loop_active() {
            format!("goal set: {raw}\n  the running loop picks this up on its next iteration")
        } else {
            format!(
                "goal set: {raw}\n  steers every turn · /goal go drives at it autonomously · \
                 /goal rounds <N> bounds it · /goal cmd <check> binds verifiable done-detection"
            )
        };
        let workspace = self.tools.current_workspace().to_path_buf();
        match self.goal.as_mut() {
            // Replacing the text of an existing goal keeps its criteria/cmd/notes,
            // rounds, and budget — but a fresh objective clears any blocking
            // candidates, because the blocking question is now a different one.
            Some(g) => {
                g.text = raw.to_string();
                g.status = GoalStatus::Active;
                g.blocked_reason = None;
                g.blocked_streak = 0;
                g.updated_ms = now_ms();
                if let Err(error) = save_for(g, &workspace) {
                    return goal_persist_error("set", error);
                }
            }
            None => {
                let mut g = Goal::new(raw);
                if let Err(error) = save_for(&mut g, &workspace) {
                    return goal_persist_error("set", error);
                }
                self.goal = Some(g);
            }
        }
        self.start_lifecycle_ceremony(crate::viz::lifecycle_viz::CeremonyKind::GoalSet, raw);
        msg
    }

    /// Apply `f` to the active goal, persist, and return its message (or a "no
    /// goal" hint). Keeps the refine subcommands DRY.
    fn goal_refine(&mut self, f: impl FnOnce(&mut Goal) -> String) -> String {
        let workspace = self.tools.current_workspace().to_path_buf();
        match self.goal.as_mut() {
            Some(g) => {
                let msg = f(g);
                g.updated_ms = now_ms();
                match save_for(g, &workspace) {
                    Ok(()) => msg,
                    Err(error) => goal_persist_error("update", error),
                }
            }
            None => "no goal set — /goal <text> to set one first".to_string(),
        }
    }

    /// Multi-line rendering of the current goal for the `/goal` command.
    fn goal_show(&self) -> String {
        match self.goal.as_ref() {
            None => "no goal set — /goal <text> to set one".to_string(),
            Some(g) if !matches_workspace(g, self.tools.current_workspace()) => {
                "no goal set for this project — /goal <text> to set one".to_string()
            }
            Some(g) => {
                let status = match g.status {
                    GoalStatus::Active => "active",
                    GoalStatus::Done => "done",
                    GoalStatus::Paused => "paused",
                    GoalStatus::Blocked => "blocked",
                };
                let budget = match g.max_rounds {
                    Some(cap) => format!("round {}/{}", g.rounds, cap),
                    None => format!("round {}", g.rounds),
                };
                let mut out = format!("goal [{status} · {budget}]: {}", g.text.trim());
                if let Some(reason) = &g.blocked_reason {
                    out.push_str(&format!(
                        "\n  blocking ({}/{GOAL_BLOCKED_MIN_ROUNDS}): {reason}",
                        g.blocked_streak
                    ));
                }
                if !g.acceptance.is_empty() {
                    out.push_str("\n  acceptance:");
                    for c in &g.acceptance {
                        out.push_str(&format!("\n    - {c}"));
                    }
                }
                if let Some(cmd) = &g.accept_cmd {
                    out.push_str(&format!("\n  verify: {cmd}"));
                }
                if !g.notes.is_empty() {
                    out.push_str(&format!("\n  notes: {}", g.notes.len()));
                }
                if g.status == GoalStatus::Blocked {
                    out.push_str("\n  /goal resume re-arms it once the blocker clears");
                } else if g.status == GoalStatus::Paused {
                    out.push_str("\n  /goal resume re-arms it");
                } else if self.loop_active() && self.loop_ctl.task.trim().is_empty() {
                    out.push_str("\n  driven by the live /loop (/loop status)");
                } else if g.status == GoalStatus::Active {
                    out.push_str("\n  /goal go to drive at it autonomously");
                }
                out
            }
        }
    }
}

fn casual_goal_bypass(raw: &str) -> bool {
    let s = raw.trim().to_ascii_lowercase();
    if s.is_empty() {
        return true;
    }
    let compact = s.trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace());
    if matches!(
        compact,
        "hi" | "hello"
            | "hey"
            | "yo"
            | "sup"
            | "ping"
            | "test"
            | "ok"
            | "okay"
            | "cool"
            | "nice"
            | "thanks"
            | "thank you"
            | "lol"
            | "lmao"
    ) {
        return true;
    }
    let words = compact.split_whitespace().count();
    words <= 4
        && (compact.starts_with("hello ")
            || compact.starts_with("hi ")
            || compact.starts_with("hey ")
            || compact.starts_with("thanks ")
            || compact.starts_with("thank you "))
}

/// If `raw` begins with one of `words` followed by whitespace, return the trimmed
/// remainder. The `+` shorthand needs no trailing space (`+ship it` / `+ ship it`).
fn strip_word<'a>(raw: &'a str, words: &[&str]) -> Option<&'a str> {
    for w in words {
        if *w == "+" {
            if let Some(rest) = raw.strip_prefix('+') {
                let rest = rest.trim();
                if !rest.is_empty() {
                    return Some(rest);
                }
            }
            continue;
        }
        if let Some(after) = strip_ascii_prefix_ci(raw, w)
            && after.starts_with(char::is_whitespace)
        {
            let rest = after.trim();
            if !rest.is_empty() {
                return Some(rest);
            }
        }
    }
    None
}

fn strip_ascii_prefix_ci<'a>(raw: &'a str, prefix: &str) -> Option<&'a str> {
    let prefix_bytes = prefix.len();
    let head = raw.get(..prefix_bytes)?;
    if !head.eq_ignore_ascii_case(prefix) {
        return None;
    }
    raw.get(prefix_bytes..)
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/goal__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/cockpit/app/goal__loop_retry_tests.rs"]
mod loop_retry_tests;
