//! The `/self` controller: Layer 2/3 of the self-model — point the autonomous
//! loop at the cockpit's **own crate**, safely.
//!
//! `/self <goal>` composes pieces that already exist (see `docs/SELF_MODEL.md`):
//! it locates the live source ([`self_model::source_root`]), checks out an
//! isolated **git worktree** on a fresh `angel/self-*` branch, re-roots the tool
//! registry there (the same swap `/cd` does), and arms the [`loop_ctl`] run with
//! `self_edit = true` — so every iteration is a normal guarded turn editing the
//! worktree, never the live tree.
//!
//! Done-detection is the **Layer-2 gate**, not the model's word: a `LOOP_DONE`
//! claim runs `cargo build` + `cargo test` (plus the pass-count regression
//! guard) in the worktree ([`loop_spawn_self_gate`](crate::App::loop_spawn_self_gate)).
//! A green gate then asks the *operator* to approve integration; only that
//! keystroke merges the branch into the live tree. The invariant holds
//! structurally: the only merge path is the approval arm of the gate-green
//! verdict, so *no self-edit reaches the live tree unless the gate is green and
//! a human said yes*. Everything is git — a bad run is `/self discard`.
//!
//! Worktrees live under `~/.angel0/self-worktrees` (override:
//! `ANGEL_SELF_WORKTREE_DIR`, used by tests).

use crate::harness::run_git;
use crate::viz::lifecycle_viz::CeremonyKind;
use crate::loop_ctl::{self, LoopPending, LoopState, LoopStatus, count_passed_in};
use crate::tools::self_model;
use std::path::{Path, PathBuf};
use std::sync::Arc;

impl crate::App {
    /// `/self …` command surface. Verbs mirror `/loop`'s tokenization; anything
    /// that isn't a known verb is the goal text and starts a run.
    pub(crate) fn self_command(&mut self, arg: Option<String>) -> String {
        let raw = arg.unwrap_or_default();
        let raw = raw.trim().to_string();
        let verb = raw
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match verb.as_str() {
            "" | "status" => self.self_status_text(),
            "integrate" | "merge" => self.self_integrate_command(),
            "discard" | "drop" | "abandon" => self.self_discard(),
            "reborn" | "rebirth" | "restart" => self.self_reborn(),
            _ => self.self_start(raw),
        }
    }

    /// Start a self-modification run: worktree + re-rooted tools + armed loop.
    fn self_start(&mut self, goal: String) -> String {
        if self.thinking.is_some()
            || self.bg_job.is_some()
            || self.pending_turn.is_some()
            || self.loop_pending.is_some()
        {
            return "busy — finish or interrupt the turn before /self".to_string();
        }
        if self.loop_active() {
            return "a loop is already running — /loop status (or /loop stop) first".to_string();
        }
        if self.loop_ctl.status == LoopStatus::Paused {
            return "a paused loop exists — /loop resume, /loop clear, or /self discard first"
                .to_string();
        }
        let Some(crate_root) = self_model::source_root() else {
            return "cannot locate the cockpit's own source — set ANGEL_SELF_SRC to the crate root"
                .to_string();
        };
        let current_workspace = self.tools.current_workspace();
        if !same_canonical_project(current_workspace, &crate_root) {
            return format!(
                "/self blocked by project isolation: the current project is {} but Angel's source is {}. Use `/cd {}` first so the normal project boundary saves this thread and starts a fresh Angel-source thread.",
                current_workspace.display(),
                crate_root.display(),
                crate_root.display(),
            );
        }
        let git_root = match run_git(&crate_root, &["rev-parse", "--show-toplevel"]) {
            Ok(s) => PathBuf::from(s.trim()),
            Err(e) => return format!("/self: the crate root is not in a git repo: {e}"),
        };
        let rel = crate_root
            .strip_prefix(&git_root)
            .unwrap_or(Path::new(""))
            .to_path_buf();

        // Fresh worktree on a new branch from the live HEAD.
        let stamp = format!("{}-{}", now_ms(), std::process::id());
        let branch = format!("angel/self-{stamp}");
        let base = worktree_base();
        if let Err(e) = std::fs::create_dir_all(&base) {
            return format!("/self: mkdir {}: {e}", base.display());
        }
        let wt_top = base.join(format!("self-{stamp}"));
        let wt_str = wt_top.to_string_lossy().into_owned();
        if let Err(e) = run_git(
            &git_root,
            &["worktree", "add", "-q", "-b", &branch, &wt_str, "HEAD"],
        ) {
            return format!("/self: worktree add failed: {e}");
        }
        let wt_crate = wt_top.join(&rel);
        if !wt_crate.join("Cargo.toml").is_file() {
            let _ = run_git(&git_root, &["worktree", "remove", "--force", &wt_str]);
            let _ = run_git(&git_root, &["branch", "-D", &branch]);
            return format!(
                "/self: {} has no Cargo.toml — worktree dropped",
                wt_crate.display()
            );
        }

        // Re-root the tool registry in the worktree's crate (the `/cd` swap: the
        // old registry Arc drops its MCP/LSP children; a running turn can't race
        // this — we checked the flight slot above).
        let prev = self.tools.current_workspace().to_path_buf();
        let mut registry =
            crate::bootstrap::build_registry(&self.bag, &self.session.id, wt_crate.clone());
        crate::ui_inspect::install(&mut registry, std::sync::Arc::clone(&self.ui_broker));
        self.tools = Arc::new(registry);

        // Arm the loop as a self run. No accept_cmd: the Layer-2 gate *is* the
        // acceptance predicate, hard-coded and unswappable by the model.
        let mut st = LoopState::configured_from_env();
        st.owner_session_id = Some(self.session.id.clone());
        st.workspace = Some(wt_crate.clone());
        st.task = goal.clone();
        st.self_edit = true;
        st.self_branch = Some(branch.clone());
        st.self_root = Some(crate_root);
        st.self_prev_workspace = Some(prev);
        st.start_rev = loop_ctl::git_head(Some(&wt_crate));
        st.started_ms = now_ms();
        st.updated_ms = now_ms();
        st.status = LoopStatus::Baselining;
        self.loop_ctl = st;

        // Pre-edit baseline: the worktree's passing-test count, so a "green"
        // that quietly shed tests is rejected by the gate's regression guard.
        let ws = wt_crate.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(count_passed_in("cargo test", &ws));
        });
        self.loop_pending = Some(LoopPending::Baseline(rx, LoopStatus::Running));
        loop_ctl::save(&self.loop_ctl);

        self.start_lifecycle_ceremony(CeremonyKind::LoopStart, format!("self-forge: {goal}"));
        format!(
            "self-forge started → {goal}\n  worktree {} (branch {branch})\n  \
             baselining: cargo test in the worktree (the first build takes a while)\n  \
             gate: cargo build + cargo test + regression guard · green → your approval → merge",
            wt_crate.display()
        )
    }

    fn self_status_text(&mut self) -> String {
        if !self.loop_ctl.self_edit {
            return "no self run — /self <goal> starts one (worktree + gate; see docs/SELF_MODEL.md)"
                .to_string();
        }
        self.loop_command(Some("status".to_string()))
    }

    /// `/self integrate`: re-run the gate on the pinned worktree; a green verdict
    /// lands in the same approval → merge path as an autonomous done-claim, so
    /// the merge invariant can't be bypassed by asking manually.
    fn self_integrate_command(&mut self) -> String {
        if !self.loop_ctl.self_edit {
            return "no self run to integrate — /self <goal> first".to_string();
        }
        if self.thinking.is_some() || self.bg_job.is_some() {
            return "busy — finish or interrupt the turn before integrating".to_string();
        }
        if matches!(
            self.loop_ctl.status,
            LoopStatus::Verifying | LoopStatus::AwaitingApproval
        ) || self.loop_pending.is_some()
        {
            return "already verifying / awaiting approval — answer the modal".to_string();
        }
        self.loop_spawn_self_gate();
        "self · re-running the Layer-2 gate; if it comes back green you'll be asked to approve \
         the merge"
            .to_string()
    }

    /// `/self discard`: stop the run, drop the worktree and its branch, restore
    /// the previous workspace. The live tree was never touched.
    fn self_discard(&mut self) -> String {
        if !self.loop_ctl.self_edit {
            return "no self run to discard".to_string();
        }
        if self.thinking.is_some() || self.bg_job.is_some() || self.loop_pending.is_some() {
            if self.loop_active() {
                let _ = self.loop_command(Some("stop".to_string()));
            }
            return "busy — retry /self discard after existing worker work drains".to_string();
        }
        // Capture the run's identity before clearing state.
        let branch = self.loop_ctl.self_branch.clone();
        let wt_crate = self.loop_ctl.workspace.clone();
        let live_root = self.loop_ctl.self_root.clone();
        if self.loop_active() {
            let _ = self.loop_command(Some("stop".to_string()));
        }
        self.self_restore_workspace();
        let mut out = String::from("self run discarded");
        if let (Some(live_root), Some(wt_crate)) = (live_root, wt_crate)
            && let Ok(git_root) = run_git(&live_root, &["rev-parse", "--show-toplevel"])
        {
            let git_root = PathBuf::from(git_root.trim());
            if let Some(wt_top) = worktree_top(&wt_crate) {
                match run_git(
                    &git_root,
                    &["worktree", "remove", "--force", &wt_top.to_string_lossy()],
                ) {
                    Ok(_) => out.push_str(&format!(" · worktree {} removed", wt_top.display())),
                    Err(e) => out.push_str(&format!(" · worktree remove failed: {e}")),
                }
            }
            if let Some(branch) = &branch {
                match run_git(&git_root, &["branch", "-D", branch]) {
                    Ok(_) => out.push_str(&format!(" · branch {branch} deleted")),
                    Err(e) => out.push_str(&format!(" · branch delete failed: {e}")),
                }
            }
        }
        let _ = self.loop_command(Some("clear".to_string()));
        out
    }

    /// A gate-green verdict on a self run: put the merge decision in front of
    /// the operator (the standard approval modal). Called from the loop's
    /// verify-drain; the reply is drained as [`LoopPending::SelfIntegrate`].
    /// Under YOLO SMART/FULL the gate-green merge proceeds immediately — the
    /// operator already opted into self-modification power.
    pub(crate) fn self_request_integrate(&mut self, gate_summary: &str) {
        if self.loop_pending.is_some() {
            self.system_msg("self · waiting for existing verifier work to drain".to_string());
            return;
        }
        if self.exit_request.is_some() {
            self.loop_finish(
                LoopStatus::Done,
                &format!("gate green ({gate_summary}) — exit requested; branch kept unmerged; /self integrate re-runs the gate before merging later"),
            );
            return;
        }
        let branch = self
            .loop_ctl
            .self_branch
            .clone()
            .unwrap_or_else(|| "(unknown branch)".to_string());
        if crate::yolo::auto_integrate_self_edits() {
            self.system_msg(format!(
                "self-edit gate GREEN ({gate_summary}) — YOLO {} auto-merging {branch} into the live tree",
                crate::yolo::profile().label()
            ));
            self.self_integrate_now();
            return;
        }
        if self.pending_approval.is_some() {
            // Another modal is up; don't lose the green verdict — park instead.
            self.loop_finish(
                LoopStatus::Paused,
                &format!("gate GREEN ({gate_summary}) — /self integrate to merge"),
            );
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.pending_approval = Some(crate::PendingApproval {
            prompt: format!(
                "self-edit gate GREEN ({gate_summary}) — merge {branch} into the live tree? \
                 (y merge · n keep the branch unmerged)"
            ),
            scope_label: None,
            reply: tx,
        });
        self.loop_pending = Some(LoopPending::SelfIntegrate(rx));
        self.loop_ctl.status = LoopStatus::AwaitingApproval;
        self.loop_ctl.awaiting_turn = false;
        self.loop_ctl.cycle_started_ms = None;
        loop_ctl::save(&self.loop_ctl);
    }

    /// The approved merge: commit the worktree's work on its branch, merge
    /// `--no-ff` into the live tree, drop the worktree (the branch stays as
    /// history), restore the previous workspace. Only reachable from the
    /// gate-green approval arm — that is the Layer-2 invariant.
    pub(crate) fn self_integrate_now(&mut self) {
        if self.loop_pending.is_some() {
            self.system_msg("self · waiting for existing verifier work to drain".to_string());
            return;
        }
        if self.exit_request.is_some() {
            self.loop_finish(
                LoopStatus::Paused,
                "integration held by exit; branch kept unmerged; /self integrate re-runs the gate before merging later",
            );
            return;
        }
        let (Some(branch), Some(wt_crate), Some(live_root)) = (
            self.loop_ctl.self_branch.clone(),
            self.loop_ctl.workspace.clone(),
            self.loop_ctl.self_root.clone(),
        ) else {
            self.loop_finish(
                LoopStatus::Failed,
                "self run lost its branch/worktree state",
            );
            return;
        };
        let task = self.loop_task_text();
        let msg = format!("self: {}", truncate(&task, 60));
        if let Err(e) = run_git(&wt_crate, &["add", "-A"])
            .and_then(|_| run_git(&wt_crate, &["commit", "-q", "--allow-empty", "-m", &msg]))
        {
            self.loop_finish(
                LoopStatus::Failed,
                &format!("could not commit the worktree: {e}"),
            );
            return;
        }
        let git_root = match run_git(&live_root, &["rev-parse", "--show-toplevel"]) {
            Ok(s) => PathBuf::from(s.trim()),
            Err(e) => {
                self.loop_finish(
                    LoopStatus::Failed,
                    &format!("live tree not a git repo: {e}"),
                );
                return;
            }
        };
        let merge_msg = format!("integrate {branch} (self-edit, gate green)");
        match run_git(&git_root, &["merge", "--no-ff", "-m", &merge_msg, &branch]) {
            Ok(_) => {
                if let Some(wt_top) = worktree_top(&wt_crate)
                    && let Err(e) = remove_worktree_robust(&git_root, &wt_top)
                {
                    self.note(format!(
                        "self · worktree not fully removed ({e}) — a background \
                             process may still be writing there; `git worktree prune` \
                             and delete {} once it settles",
                        wt_top.display()
                    ));
                }
                self.self_restore_workspace();
                self.loop_finish(
                    LoopStatus::Done,
                    &format!(
                        "self-edit integrated — {branch} merged into the live tree; \
                         /self reborn rebuilds and restarts into the new self, resuming \
                         this session"
                    ),
                );
            }
            Err(e) => {
                let conflicts = run_git(&git_root, &["diff", "--name-only", "--diff-filter=U"])
                    .unwrap_or_default();
                let _ = run_git(&git_root, &["merge", "--abort"]);
                self.loop_finish(
                    LoopStatus::Failed,
                    &format!(
                        "merge conflict on {branch} (aborted); conflicts: {}; the worktree and \
                         branch are kept — resolve manually or /self discard; {e}",
                        conflicts.trim()
                    ),
                );
            }
        }
    }

    /// `/self reborn` — the phoenix step: rebuild the live crate off-thread and,
    /// when the build lands green, exec() into the new binary with
    /// `--resume <session>` so the conversation continues *inside the new self*.
    /// The exec itself happens in `main` after terminal teardown (see
    /// [`drain_reborn`](Self::drain_reborn)); a red build changes nothing.
    fn self_reborn(&mut self) -> String {
        if self.thinking.is_some() || self.bg_job.is_some() {
            return "busy — finish or interrupt the turn before a reborn".to_string();
        }
        if self.reborn_rx.is_some() {
            return "already rebuilding — the cockpit restarts when the build lands".to_string();
        }
        if self.loop_active() {
            return "a loop is live — /loop pause (or /loop stop) before a reborn".to_string();
        }
        let Some(crate_root) = self_model::source_root() else {
            return "cannot locate the cockpit's own source — set ANGEL_SELF_SRC to the crate root"
                .to_string();
        };
        let crate_root = crate_root.canonicalize().unwrap_or(crate_root);
        // Rebuilding only helps if the running binary *is* this crate's build
        // product — then `cargo build` rewrites the very file we re-exec.
        let exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.canonicalize().ok())
            .filter(|p| p.starts_with(&crate_root));
        let Some(exe) = exe else {
            return "reborn: the running binary was not built from this crate's target dir — \
                    rebuild and restart manually"
                .to_string();
        };
        let release = exe.components().any(|c| c.as_os_str() == "release");
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut cmd = std::process::Command::new("cargo");
            cmd.arg("build").current_dir(&crate_root);
            if release {
                cmd.arg("--release");
            }
            let res = match crate::harness::output_timed(cmd, None) {
                Ok((o, false)) if o.status.success() => Ok(exe),
                Ok((o, false)) => {
                    let err = String::from_utf8_lossy(&o.stderr);
                    let tail = err
                        .lines()
                        .rev()
                        .find(|l| !l.trim().is_empty())
                        .unwrap_or("cargo build failed");
                    Err(tail.trim().to_string())
                }
                Ok((_, true)) => Err("cargo build timed out".to_string()),
                Err(e) => Err(format!("cargo build: {e}")),
            };
            let _ = tx.send(res);
        });
        self.reborn_rx = Some(rx);
        format!(
            "reborn · rebuilding the live crate ({}) — on a green build the cockpit execs \
             into the new binary and resumes this session",
            if release { "release" } else { "debug" }
        )
    }

    /// Drain the reborn build (called each frame from `advance`). A green build
    /// stages the exec and ends the run loop; a red build reports and stays put.
    pub(crate) fn drain_reborn(&mut self) {
        let Some(rx) = self.reborn_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(exe)) => {
                self.system_msg(format!(
                    "reborn · build green — restarting into {} (resuming session {})",
                    exe.display(),
                    self.session.id
                ));
                self.reborn_exec = Some(exe);
                self.should_quit = true;
            }
            Ok(Err(e)) => {
                self.system_msg(format!("reborn · build failed — staying in this self: {e}"));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => self.reborn_rx = Some(rx),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.system_msg("reborn · build worker died — staying in this self".to_string());
            }
        }
    }

    /// Re-root the tool registry back at the workspace `/self` displaced.
    pub(crate) fn self_restore_workspace(&mut self) {
        let Some(prev) = self.loop_ctl.self_prev_workspace.clone() else {
            return;
        };
        if self.tools.current_workspace() == prev.as_path() {
            return;
        }
        let mut registry =
            crate::bootstrap::build_registry(&self.bag, &self.session.id, prev.clone());
        crate::ui_inspect::install(&mut registry, std::sync::Arc::clone(&self.ui_broker));
        self.tools = Arc::new(registry);
        self.system_msg(format!("workspace restored → {}", prev.display()));
    }
}

/// Where self-run worktrees are created. `ANGEL_SELF_WORKTREE_DIR` overrides the
/// default `~/.angel0/self-worktrees` (tests point it at a scratch dir).
fn worktree_base() -> PathBuf {
    if let Ok(p) = std::env::var("ANGEL_SELF_WORKTREE_DIR")
        && !p.trim().is_empty()
    {
        return PathBuf::from(p);
    }
    crate::workspace_store::angel_subdir("self-worktrees")
}

/// Walk up from a dir inside a worktree to the worktree's top (the dir holding
/// its `.git` link file). `None` if no `.git` is found on the way up.
/// Remove an integrated worktree, surviving a concurrent writer — e.g. a
/// background rust-analyzer flycheck (`cargo check`) still writing `target/`
/// artifacts into it; killing the server wouldn't help since its spawned
/// cargo survives as an orphan. A racing writer can also re-create
/// directories AFTER a "successful" `git worktree remove`, so success is
/// judged by the path being gone at the end, never by git's exit code.
fn remove_worktree_robust(git_root: &Path, wt_top: &Path) -> Result<(), String> {
    let mut last_err = String::new();
    for attempt in 0..5 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        if let Err(e) = run_git(
            git_root,
            &["worktree", "remove", "--force", &wt_top.to_string_lossy()],
        ) {
            last_err = e;
        }
        if !wt_top.exists() {
            return Ok(());
        }
        // The writer re-created part of the tree between git's delete and now:
        // drop the stale registration and delete what's left directly.
        let _ = run_git(git_root, &["worktree", "prune"]);
        let _ = std::fs::remove_dir_all(wt_top);
        if !wt_top.exists() {
            return Ok(());
        }
    }
    Err(if last_err.is_empty() {
        "directory keeps reappearing".to_string()
    } else {
        last_err
    })
}

fn worktree_top(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start.to_path_buf());
    while let Some(d) = dir {
        if d.join(".git").exists() {
            return Some(d);
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    None
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn same_canonical_project(left: &Path, right: &Path) -> bool {
    let left = crate::workspace_store::repo_identity(left);
    let right = crate::workspace_store::repo_identity(right);
    left.key == right.key && left.root == right.root
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/self_loop__tests.rs"]
mod tests;
