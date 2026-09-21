//! Orchestrator layer: delegate / integrate over git worktrees.

use super::*;

/// One short, tool-agnostic hop-efficiency advisory shared by every
/// tool-bearing seat prompt (driver, delegate, graph node, spawn). Advisory only: no
/// cap, no gate, no route selection. It states that independent calls may share
/// one assistant response and that a compact result beats bulk in history.
pub(crate) const TOOL_BATCHING_HINT: &str = "Batch independent tool calls in one \
     assistant response instead of waiting between them; combine related read/filter \
     work into fewer calls and consume the compact result.";

/// Build the orchestrator (Driver) system prompt: who's on the team and how to
/// delegate work to them over the shared workspace.
pub fn orchestrator_system_prompt(specialists: &[String]) -> String {
    let mut s = String::from(
        "You are Angel — the Driver for this workspace. \
         Use file and shell tools in the active workspace, choosing language and build commands from its actual project files, \
         and you may delegate specialized work when an appropriate configured route is usable.\n",
    );
    s.push_str(
        "\nTool protocol: tools are available through the structured tool-call interface \
         advertised by the host. Do not print raw `<tool_call>`, `<function=...>`, \
         `shell(...)`, or `delegate(...)` markup as chat text. Use built-in tools for \
         local work, `skill(name)` for reusable playbooks, `<server>__mcp` tools for \
         MCP resources/prompts, and `delegate` only when a specialist should work in \
         an isolated subagent workspace. Tool execution is still bounded by the active \
         workspace, sandbox, hooks, and approval gates. ",
    );
    s.push_str(TOOL_BATCHING_HINT);
    s.push_str(
        " For multi-file reading or filtering, prefer one `code_mode` batch or the \
         dedicated repository tools when available, over a serial chain of shell reconnaissance.",
    );
    s.push_str(
        "\n\nDo not end a turn with a promise to inspect, check, brief the council, or pull \
         context. If you say you need reconnaissance or council input, make the \
         corresponding tool calls in that same turn, then answer from the results.\n",
    );
    s.push_str(
        "\nDefault posture: work only on the user's current task. Maintenance of the \
         cockpit itself — diagnostics, self-checks, or changes to your own code or config — \
         happens only when the user explicitly asks for it in this session. If a tool fails, \
         adapt your approach to the task, or report the blocker and ask the user how to \
         proceed; a failing tool is never, on its own, a reason to switch to maintenance \
         work. If you have no task, ask the user what they would like to do and wait.\n\
         \nLocal fleet seats (turbo, spark, atlas, gemma, and other LAN boxes) are optional. \
         Only use a named local club while it is reachable. If a local seat is down, do the \
         work yourself — a down local is not a failed turn and is not worth retrying.\n",
    );
    s.push_str(
        "\nVerification posture: after editing code, run the smallest relevant verifier. One \
         conclusive green test or build on the unchanged workspace is enough; do not stack \
         broader, overlapping tests, builds, lint, or vet commands unless the task explicitly \
         requires distinct gates or the first verifier produced a diagnostic that demands one.\n",
    );
    if specialists.is_empty() {
        s.push_str(
            "No delegate routes are configured for this session; do the work yourself. Be concise.",
        );
        return s;
    }
    s.push_str("\nConfigured delegate routes — use the structured `delegate` tool with `club`, `task`, and optional `mode` args only when that route is appropriate and reachable:\n");
    for label in specialists {
        s.push_str(&format!("- {label}: {}\n", club_role(label)));
    }
    s.push_str(
        "\n`delegate` runs that teammate in an isolated git worktree and returns a summary, a \
         branch name, and a diff. Use `mode=review` or `mode=read_only` for inspection-only \
         reviewers. Use `mode=write` for implementation, then call `integrate` with the returned \
         branch afterwards (one branch at a time). Be concise.",
    );
    s.push_str(
        "\n\n`spawn` fans a question across parallel sub-agents mid-turn — copies of yourself \
         (club=self) or the fleet (club=auto) — in a formation (panel / moa / quorum), each seat \
         optionally wearing a persona and a scoped tool grant. Reach for it when independent \
         perspectives, adversarial review, or breadth beat working alone; it returns a labeled \
         digest, never file changes.",
    );
    s.push_str(
        "\n\n`swarm_compile` is the proof-carrying coding path: it freezes a base commit, \
         isolates investigation/test/implementation/review contributions, requires an intentional \
         red regression marker followed by green targeted and full gates, parks the candidate \
         branch, and learns routing only from post-action outcomes. Prefer direct action when a \
         trustworthy red verifier already exists. Use `swarm_compile` when a green base needs an \
         independently authored regression, adversarial review, or a durable audit trail, and the \
         task has a concrete test scope plus a green baseline acceptance command. Integration \
         remains an explicit later action.",
    );
    s
}

pub(crate) static DELEGATE_SEQ: AtomicU64 = AtomicU64::new(0);
static INTEGRATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub(crate) const DELEGATE_INLINE_MAX_CHARS: usize = 3_000;
pub(crate) const DELEGATE_PREVIEW_MAX_BYTES: usize = 1_600;
pub(crate) const DELEGATE_PREVIEW_MAX_LINES: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DelegateMode {
    Write,
    ReadOnly,
}

impl DelegateMode {
    pub(crate) fn parse(raw: Option<&str>) -> Result<Self, String> {
        let normalized = raw
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_lowercase());
        match normalized.as_deref() {
            None => Ok(Self::Write),
            Some("write") | Some("edit") | Some("implement") => Ok(Self::Write),
            Some("review") | Some("read_only") | Some("readonly") | Some("inspect") => {
                Ok(Self::ReadOnly)
            }
            Some(other) => Err(format!(
                "invalid delegate mode '{other}'; expected write, review, or read_only"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::ReadOnly => "read_only",
        }
    }

    fn is_read_only(self) -> bool {
        matches!(self, Self::ReadOnly)
    }
}

pub(crate) struct DelegateOutcome {
    pub(crate) lineage: Option<super::delegated_lineage::DelegateArtifact>,
    pub(crate) branch: String,
    pub(crate) base_oid: String,
    pub(crate) club: String,
    /// Compatibility summary sampled at completion. Audited contributor claims
    /// use `lineage` and its complete action-route set, not this last sample.
    pub(crate) resolved_route: String,
    pub(crate) model_revision: Option<String>,
    pub(crate) answer: String,
    pub(crate) diff: String,
    pub(crate) elapsed_ms: u128,
    pub(crate) tokens: u64,
    /// Failed tool results observed during this delegate turn. Existing callers
    /// treat this as diagnostic; strict read-only gates may fail closed on it.
    pub(crate) tool_failures: usize,
}

/// The delegate seat system prompt: the worktree contract plus the shared
/// batching advisory. Kept as one function so the text is reviewable and
/// testable without launching a delegate run, and it names no tool the
/// worktree registry does not register.
pub(crate) fn delegate_system_prompt(mode: DelegateMode) -> String {
    let mut s = if mode.is_read_only() {
        "You are an Angel specialist reviewing in an isolated git worktree. Inspect only: \
         do not edit, create, delete, format, or commit files. Use the read-only shell tool \
         for evidence, then return concrete findings and references."
            .to_string()
    } else {
        "You are an Angel specialist working in an isolated git worktree. Use the \
         shell/cargo tools to make the requested changes to files here, then summarize. \
         Use the repository's development checks for edit/test iterations; reserve \
         optimized release builds for final qualification or optimization-specific bugs. \
         Stop a test/build chain at its first failed prerequisite. Keep long-running \
         commands observable with streamed output; do not hide all progress behind file \
         redirection."
            .to_string()
    };
    s.push(' ');
    s.push_str(TOOL_BATCHING_HINT);
    s
}

/// Runs a specialist club on a task in an isolated git worktree.
pub(crate) struct DelegateTool {
    workspace: PathBuf,
    clubs: HashMap<String, Arc<dyn Club>>,
    cargo: PinnedCargo,
}

impl DelegateTool {
    #[cfg(test)]
    pub(crate) fn new(workspace: PathBuf, roster: Vec<Arc<dyn Club>>) -> Self {
        let cargo = PinnedCargo::capture(&workspace);
        Self::new_with_cargo(workspace, roster, cargo)
    }

    pub(crate) fn new_with_cargo(
        workspace: PathBuf,
        roster: Vec<Arc<dyn Club>>,
        cargo: PinnedCargo,
    ) -> Self {
        let clubs = roster
            .into_iter()
            .map(|c| (normalize_club_name(c.label()), c))
            .collect();
        Self {
            workspace,
            clubs,
            cargo,
        }
    }

    pub(crate) fn worktree_base(&self) -> PathBuf {
        // Keep worktrees beside the repository, even when the active workspace
        // is a nested project directory. Putting `.angel-worktrees` inside the
        // main worktree makes it appear as user dirt and risks recursive scans.
        let canonical = self
            .workspace
            .canonicalize()
            .unwrap_or_else(|_| self.workspace.clone());
        let repo = canonical.ancestors().find(|dir| is_git_root(dir));
        repo.and_then(Path::parent)
            .or_else(|| canonical.parent())
            .unwrap_or(canonical.as_path())
            .join(".angel-worktrees")
            .join(worktree_scope(&self.workspace))
    }

    #[cfg(test)]
    pub(crate) fn run(
        &self,
        club_name: &str,
        task: &str,
        mode: DelegateMode,
    ) -> Result<DelegateOutcome, String> {
        self.run_from(club_name, task, mode, "HEAD")
    }

    /// Run a delegate from an explicit commit-ish. Proof-carrying swarm runs use
    /// this to make a test-only branch, then base the implementation branch on
    /// that immutable test contribution without touching the shared checkout.
    pub(crate) fn run_from(
        &self,
        club_name: &str,
        task: &str,
        mode: DelegateMode,
        base_ref: &str,
    ) -> Result<DelegateOutcome, String> {
        self.run_from_with_cancel(club_name, task, mode, base_ref, None)
    }

    fn run_from_with_cancel(
        &self,
        club_name: &str,
        task: &str,
        mode: DelegateMode,
        base_ref: &str,
        cancel: Option<&AtomicBool>,
    ) -> Result<DelegateOutcome, String> {
        // Normalize so the model can say "Turbo"/"turbo"/"TURBO" interchangeably.
        let club_name = normalize_club_name(club_name);
        let club = self.clubs.get(&club_name).cloned().ok_or_else(|| {
            let mut names: Vec<_> = self.clubs.keys().cloned().collect();
            names.sort();
            // A SOTA label withheld by the roster gate reads as "unknown"
            // here; name the gate so the model routes instead of concluding
            // the capability doesn't exist.
            if crate::agent::club::is_sota_label(&club_name)
                && (crate::agent::tools::solo::solo_mode_active()
                    || !crate::agent::harness::env_flag("ANGEL_ALLOW_SOTA_DELEGATE", true))
            {
                if crate::agent::tools::solo::solo_mode_active() {
                    format!(
                        "solo mode: refuse delegate to paid SOTA seat '{club_name}'. \
                         Do the work yourself. Available now: {}",
                        names.join(", ")
                    )
                } else {
                    format!(
                        "club '{club_name}' is a paid SOTA seat withheld from delegate because \
                         ANGEL_ALLOW_SOTA_DELEGATE=0 (local-only fleet opt-out). Unset that pin or set \
                         ANGEL_ALLOW_SOTA_DELEGATE=1. Available now: {}",
                        names.join(", ")
                    )
                }
            } else {
                format!("unknown club '{club_name}'; have: {}", names.join(", "))
            }
        })?;
        // Solo: even if the SOTA club is on the roster, refuse outbound work.
        if crate::agent::tools::solo::solo_mode_active()
            && crate::agent::club::is_sota_label(club.label())
        {
            return Err(format!(
                "solo mode: refuse delegate to paid/remote seat '{}'. Own the workload.",
                club.label()
            ));
        }
        if !club.is_available() {
            return Err(format!(
                "club '{club_name}' is configured but not reachable right now"
            ));
        }

        let repo = ensure_git_workspace(&self.workspace)?;
        let workspace = self
            .workspace
            .canonicalize()
            .map_err(|e| format!("resolve workspace {}: {e}", self.workspace.display()))?;
        let workspace_rel = workspace.strip_prefix(&repo).map_err(|_| {
            format!(
                "workspace {} is not inside git root {}",
                workspace.display(),
                repo.display()
            )
        })?;
        let n = DELEGATE_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let safe_club = safe_ref_component(&club_name);
        let run_id = format!("{safe_club}-{}-{stamp}-{n}", std::process::id());
        let branch = format!("angel/{run_id}");
        let base_ref = base_ref.trim();
        if base_ref.is_empty() || base_ref.starts_with('-') || base_ref.len() > 240 {
            return Err("invalid delegate base ref".to_string());
        }
        let commitish = format!("{base_ref}^{{commit}}");
        let base_oid = run_git(
            &repo,
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                commitish.as_str(),
            ],
        )?
        .trim()
        .to_string();
        let base = self.worktree_base();
        std::fs::create_dir_all(&base).map_err(|e| format!("mkdir worktrees: {e}"))?;
        let wt = base.join(&run_id);
        let wt_str = wt.to_string_lossy().into_owned();
        let started = Instant::now();
        let usage_before = club.token_usage().unwrap_or_default();

        // Never materialize quarantined repository trees as an intermediate
        // checkout. Install the worktree-local sparse boundary first.
        if let Err(err) =
            prepare_delegate_worktree(&repo, &wt, &branch, &base_oid, "off-limits", cancel)
        {
            // `worktree add` can create the branch before failing later (for
            // example on checkout). Best-effort rollback keeps a retry clean.
            let _ = run_git(&repo, &["worktree", "remove", "--force", wt_str.as_str()]);
            let _ = run_git(&repo, &["branch", "-D", branch.as_str()]);
            return Err(err);
        }
        let wt_workspace = wt.join(workspace_rel);
        let session_dir = delegate_session_dir();
        let session_path = session_dir.join(format!("{run_id}.json"));
        let mut session = crate::knowledge::session::Session::at(session_dir, run_id.clone());
        session.bind(&repo);
        let outcome = (|| {
            // The specialist works in the corresponding workspace directory
            // inside the worktree, not accidentally at the repository root.
            let mut wt_tools = ToolRegistry::new();
            wt_tools.set_workspace(wt_workspace.clone());
            if mode.is_read_only() {
                wt_tools.register(Box::new(ShellTool::read_only_in_dir(wt_workspace.clone())));
            } else {
                wt_tools.register(Box::new(
                    ShellTool::in_dir(wt_workspace.clone())
                        .with_mutation_targets(Arc::clone(&wt_tools.mutation_targets)),
                ));
                wt_tools.register(Box::new(
                    CargoTool::in_dir_with_cargo(wt_workspace.clone(), self.cargo.clone())
                        .with_mutation_targets(Arc::clone(&wt_tools.mutation_targets)),
                ));
            }

            let system = delegate_system_prompt(mode);
            let mut history = vec![ChatMsg::system(system.clone()), ChatMsg::user(task)];
            let checkpoint = |history: &[ChatMsg]| {
                session
                    .checkpoint(history)
                    .map_err(|error| error.to_string())
            };
            let _depth = SubcallDepthGuard::enter();
            let local_cancel = AtomicBool::new(false);
            let cancel = cancel.unwrap_or(&local_cancel);
            let task_sha256 = crate::knowledge::cut::sha256_hex(task.as_bytes());
            let config = serde_json::json!({"system": system, "mode": mode.as_str(),
                "max_hops": default_max_hops(), "club": club.label()});
            let binding = super::rollout::TaskRolloutBindingV1::new(
                Some(run_id.clone()),
                Some(run_id.clone()),
                task_sha256.clone(),
                crate::knowledge::cut::sha256_hex(
                    &serde_json::to_vec(&config).expect("delegate config serializes"),
                ),
                env!("CARGO_PKG_VERSION").into(),
                crate::agent::harness::run_identity::source_sha256().into(),
            );
            let (turn, tool_failures) = observe_delegate_turn(cancel, club.label(), |evt_tx| {
                super::turn::run_delegate_turn_observed(
                    club.as_ref(),
                    &wt_tools,
                    &mut history,
                    cancel,
                    Some(default_max_hops()),
                    evt_tx,
                    &binding,
                    &checkpoint,
                )
            });
            let saved = checkpoint(&history);
            let turn = turn.map_err(|e| {
                format!(
                    "specialist loop failed: {}; rollout={}",
                    e.message,
                    e.rollout_id.as_deref().unwrap_or("uncaptured")
                )
            })?;
            saved?;
            if turn.interrupted
                || turn.deadline_reached
                || turn.max_hops_reached
                || cancel.load(Ordering::Relaxed)
            {
                return Err(format!(
                    "specialist did not complete: {:?}; rollout={}",
                    turn.stop_reason,
                    turn.rollout_id.as_deref().unwrap_or("uncaptured")
                ));
            }
            let answer = turn.answer;
            let resolved_identity = club.resolved_route_identity();

            if mode.is_read_only() {
                reject_read_only_delegate_changes(&wt)?;
            } else {
                // Capture the specialist's work as a commit on its branch (allow-empty so
                // the branch always has a tip even if nothing changed).
                run_git(&wt, &["add", "-A"])?;
                let msg = format!("delegate: {}", truncate(task, 60));
                run_git(&wt, &["commit", "-q", "--allow-empty", "-m", msg.as_str()])?;
            }

            // Compare against the commit this worktree actually forked from.
            // The main branch may have advanced while the specialist ran.
            let tip_oid = run_git(&wt, &["rev-parse", "HEAD"])?.trim().to_string();
            let tree_oid = run_git(&wt, &["rev-parse", "HEAD^{tree}"])?
                .trim()
                .to_string();
            let diff = run_git(
                &repo,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    base_oid.as_str(),
                    tip_oid.as_str(),
                ],
            )?;
            let lineage = (|| {
                let child_rollout_id = turn.rollout_id?;
                let child_audit = super::rollout::audit_delegate_rollout_receipt(
                    &self.workspace,
                    &wt_workspace,
                    &wt,
                    &child_rollout_id,
                )
                .ok()?;
                let artifact = super::delegated_lineage::DelegateArtifact {
                    schema: "angel-delegate-artifact/v1".into(),
                    parent_workspace_sha256: delegate_workspace_digest(&self.workspace),
                    task_sha256,
                    answer_sha256: crate::knowledge::cut::sha256_hex(answer.as_bytes()),
                    mode: if mode.is_read_only() {
                        "read_only"
                    } else {
                        "write"
                    }
                    .into(),
                    diff_sha256: delegate_diff_digest(&repo, &base_oid, &tip_oid, Some(cancel))
                        .ok()?,
                    base_oid: base_oid.clone(),
                    tip_oid,
                    tree_oid,
                    child_rollout_id,
                    child_audit,
                };
                artifact.validate().ok()?;
                Some(artifact)
            })();
            let usage_after = club.token_usage().unwrap_or_default();
            Ok(DelegateOutcome {
                lineage,
                branch: branch.clone(),
                base_oid: base_oid.clone(),
                club: club.label().to_string(),
                resolved_route: resolved_identity.driver,
                model_revision: resolved_identity.model,
                answer,
                diff,
                elapsed_ms: started.elapsed().as_millis(),
                tokens: delegate_token_delta(usage_before, usage_after),
                tool_failures,
            })
        })();

        // Failure/cancellation is not permission to destroy candidate work.
        // Retain dirty files AND already-committed changes for explicit recovery.
        // Empty failed attempts and successful handoffs still clean up normally.
        if let Err(error) = &outcome
            && !mode.is_read_only()
            && delegate_has_recoverable_work(&wt, &base_oid)
        {
            return Err(format!(
                "{error}; incomplete work preserved at {wt_str}; branch={branch}; \
                 session={}; review before integration",
                session_path.display()
            ));
        }
        let cleanup = run_git(&repo, &["worktree", "remove", "--force", wt_str.as_str()]);
        match (outcome, cleanup) {
            (Ok(outcome), Ok(_)) => Ok(outcome),
            (Ok(_), Err(cleanup_err)) => Err(format!(
                "delegate completed on {branch}, but worktree cleanup failed: {cleanup_err}"
            )),
            (Err(err), Ok(_)) => {
                let _ = run_git(&repo, &["branch", "-D", branch.as_str()]);
                Err(format!("{err}; session={}", session_path.display()))
            }
            (Err(err), Err(cleanup_err)) => Err(format!(
                "{err}; additionally failed to clean worktree for {branch}: {cleanup_err}"
            )),
        }
    }

    /// Delete a completed delegate branch after its contribution has been
    /// captured elsewhere. Only Angel-owned branches are eligible.
    pub(crate) fn discard_branch(&self, branch: &str) -> Result<(), String> {
        let _process_guard = INTEGRATE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let branch = branch.trim();
        if !branch.starts_with("angel/") || branch.starts_with('-') {
            return Err("refusing to discard a non-Angel branch".to_string());
        }
        let repo = ensure_git_workspace(&self.workspace)?;
        let _repo_guard = acquire_repo_integrate_lock(&repo)?;
        run_git(&repo, &["check-ref-format", "--branch", branch])?;
        run_git(&repo, &["branch", "-D", "--", branch]).map(|_| ())
    }
}

pub(crate) fn delegate_session_dir() -> PathBuf {
    if cfg!(test) {
        std::env::temp_dir().join(format!("angel-delegate-sessions-{}", std::process::id()))
    } else {
        // A subdirectory keeps child histories out of the operator session picker.
        crate::knowledge::session::sessions_dir().join("delegates")
    }
}

pub(super) fn delegate_has_recoverable_work(worktree: &Path, base_oid: &str) -> bool {
    let unchanged = run_git(worktree, &["status", "--porcelain"])
        .is_ok_and(|status| status.trim().is_empty())
        && run_git(worktree, &["rev-parse", "HEAD"]).is_ok_and(|head| head.trim() == base_oid);
    !unchanged // If inspection fails, retain the directory conservatively.
}

pub(crate) fn normalize_club_name(label: &str) -> String {
    label.trim().to_ascii_lowercase()
}

pub(crate) fn safe_ref_component(label: &str) -> String {
    let normalized = normalize_club_name(label);
    let safe = normalized
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();
    let safe = safe.trim_matches('-');
    if safe.is_empty() {
        "delegate".to_string()
    } else {
        safe.to_string()
    }
}

pub(crate) fn worktree_scope(workspace: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    workspace.hash(&mut hasher);
    let hash = hasher.finish();
    let name = workspace
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("workspace");
    let safe = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("{safe}-{hash:016x}")
}

pub(crate) fn reject_read_only_delegate_changes(wt: &Path) -> Result<(), String> {
    let status = run_git(wt, &["status", "--short"])?;
    if status.trim().is_empty() {
        return Ok(());
    }
    let _ = run_git(wt, &["reset", "--hard", "HEAD"]);
    let _ = run_git(wt, &["clean", "-fd"]);
    Err(format!(
        "read-only delegate attempted file changes; discarded dirty worktree:\n{}",
        status.trim()
    ))
}

pub(crate) fn delegate_result_section(
    workspace: &Path,
    branch: &str,
    kind: &str,
    body: &str,
) -> Result<String, String> {
    let trimmed = body.trim();
    if trimmed.chars().count() <= DELEGATE_INLINE_MAX_CHARS {
        return Ok(if trimmed.is_empty() {
            format!("{kind}: (empty)")
        } else {
            format!("{kind}:\n{trimmed}")
        });
    }
    let dir = workspace.join("angel_test_output").join("delegates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let safe_branch = branch.replace(['/', '\\', ':'], "-");
    let ext = delegate_artifact_ext(kind, trimmed);
    let path = dir.join(format!("{safe_branch}-{kind}-{stamp}.{ext}"));
    std::fs::write(&path, trimmed.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    let preview = cap_text(
        trimmed,
        DELEGATE_PREVIEW_MAX_BYTES,
        DELEGATE_PREVIEW_MAX_LINES,
    );
    Ok(format!(
        "{kind} artifact: {} ({} chars)\n{kind} preview:\n{preview}",
        path.display(),
        trimmed.chars().count()
    ))
}

pub(crate) fn delegate_artifact_ext(kind: &str, body: &str) -> &'static str {
    if kind == "diff" {
        "diff"
    } else {
        let lower = body.to_ascii_lowercase();
        if lower.contains("<!doctype html")
            || (lower.contains("<html") && (lower.contains("<script") || lower.contains("<canvas")))
        {
            "html"
        } else {
            "md"
        }
    }
}

impl Tool for DelegateTool {
    fn name(&self) -> &str {
        "delegate"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "delegate".to_string(),
            description: "Delegate a task to a specialist club (e.g. turbo, spark-r1, atlas). It works \
                          in an isolated git worktree and returns its summary, the branch name, and \
                          the diff. Local fleet seats are optional and only used while reachable — \
                          a down local is skipped, not a failed turn. Use mode=review/read_only for \
                          inspection-only reviewers; use mode=write for implementation, then call \
                          `integrate` with that branch to apply it."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "club": { "type": "string", "description": "specialist club name" },
                    "task": { "type": "string", "description": "what the specialist should do" },
                    "mode": {
                        "type": "string",
                        "enum": ["write", "review", "read_only"],
                        "default": "write",
                        "description": "write implements changes; review/read_only inspects with workspace writes blocked"
                    }
                },
                "required": ["club", "task"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_native_context(args, None, None, &Default::default())
            .map(|result| result.text)
    }
    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        self.call_with_native_context(args, cancel, None, &Default::default())
            .map(|result| result.text)
    }
    fn call_with_native_context(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
        _progress: Option<Arc<ToolOutputProgress>>,
        _context: &super::delegated_lineage::NativeToolContext,
    ) -> Result<super::delegated_lineage::NativeToolResult, String> {
        let club = args["club"].as_str().ok_or("missing 'club'")?;
        let task = args["task"].as_str().ok_or("missing 'task'")?;
        let mode = DelegateMode::parse(args.get("mode").and_then(|v| v.as_str()))?;
        let club_key = normalize_club_name(club);
        if let Some(target) = self.clubs.get(&club_key)
            && crate::agent::tools::consult::is_optional_local_label(target.label())
            && !target.is_available()
        {
            let live: Vec<String> = self
                .clubs
                .values()
                .filter(|c| {
                    crate::agent::tools::consult::is_optional_local_label(c.label())
                        && c.is_available()
                })
                .map(|c| c.label().to_string())
                .collect();
            return Ok(super::delegated_lineage::NativeToolResult::plain(
                crate::agent::tools::consult::skip_down_local_message(
                    "delegate",
                    target.label(),
                    &live,
                ),
            ));
        }
        let o = self.run_from_with_cancel(club, task, mode, "HEAD", cancel)?;
        let answer = delegate_result_section(&self.workspace, &o.branch, "answer", &o.answer)?;
        let diff = if o.diff.trim().is_empty() {
            "diff: (no file changes)".to_string()
        } else {
            delegate_result_section(&self.workspace, &o.branch, "diff", &o.diff)?
        };
        let header = format!("[club={club} mode={} branch={}]", mode.as_str(), o.branch);
        let bulk = format!("{answer}\n--- diff ---\n{diff}");
        // Strategy header stays root-visible; large answer/diff bulk is handle-offloaded
        // so multi-agent RL rollouts keep LID root trajectories.
        let text = if let Some(receipt) = maybe_offload_root_body(
            &bulk,
            HandleKind::Subcall,
            "delegate",
            &format!("delegate|{club}"),
            subcall_offload_min_bytes(),
        ) {
            format!("{header}\n{receipt}")
        } else {
            format!("{header}\n{bulk}")
        };
        Ok(super::delegated_lineage::NativeToolResult {
            text,
            artifact: o.lineage,
            branch: Some(o.branch),
            application: None,
        })
    }
}

/// Merges a delegated branch into the shared workspace, one at a time.
fn delegate_workspace_digest(workspace: &Path) -> String {
    crate::knowledge::cut::sha256_hex(
        crate::platform::workspace_store::repo_identity(workspace)
            .root
            .to_string_lossy()
            .as_bytes(),
    )
}

pub(crate) fn prepare_delegate_worktree(
    repo: &Path,
    worktree: &Path,
    branch: &str,
    base: &str,
    excluded_directory: &str,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    // This internal parameter permits a wholly synthetic owned fixture. The
    // production caller fixes it to the quarantined top-level directory.
    if excluded_directory.is_empty()
        || !excluded_directory
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err("invalid delegate sparse exclusion".into());
    }
    let worktree_path = worktree
        .to_str()
        .ok_or("delegate worktree path is not UTF-8")?;
    delegate_git(
        repo,
        &[
            "worktree",
            "add",
            "--no-checkout",
            "-q",
            "-b",
            branch,
            worktree_path,
            base,
        ],
        cancel,
    )?;
    // Check the no-population precondition before configuring sparse checkout.
    // Only the Git metadata pointer may exist at this stage.
    for entry in
        std::fs::read_dir(worktree).map_err(|e| format!("inspect empty delegate worktree: {e}"))?
    {
        if entry.map_err(|e| e.to_string())?.file_name() != ".git" {
            return Err("delegate worktree unexpectedly populated before sparse boundary".into());
        }
    }
    let exclusion = format!("!/{excluded_directory}/");
    // Git enables worktreeConfig and migrates existing sparse settings safely;
    // these patterns therefore apply only to this owned linked worktree.
    delegate_git(
        worktree,
        &[
            "sparse-checkout",
            "set",
            "--no-cone",
            "--no-sparse-index",
            "/*",
            &exclusion,
        ],
        cancel,
    )?;
    // `set` alone on a no-checkout worktree leaves an empty index. Populate only
    // after the exclusion exists; never run an initial unrestricted checkout.
    delegate_git(worktree, &["read-tree", "-mu", "HEAD"], cancel)?;
    Ok(())
}

fn delegate_diff_digest(
    repo: &Path,
    base: &str,
    tip: &str,
    cancel: Option<&AtomicBool>,
) -> Result<String, String> {
    let mut command = super::workspace_state::pinned_git_command(repo, &[]);
    command.args([
        "diff",
        "--binary",
        "--full-index",
        "--no-ext-diff",
        "--no-textconv",
        base,
        tip,
        "--",
    ]);
    let captured = super::exec::output_timed_captured_cancellable(command, git_timeout(), cancel)?;
    if captured.timed_out
        || captured.cancelled
        || captured.stdout_truncated
        || !captured.output.status.success()
    {
        return Err("delegate binary diff capture did not complete".into());
    }
    Ok(crate::knowledge::cut::sha256_hex(&captured.output.stdout))
}

fn delegate_git(repo: &Path, args: &[&str], cancel: Option<&AtomicBool>) -> Result<String, String> {
    let mut command = super::workspace_state::pinned_git_command(repo, &[]);
    command
        .args(["-c", "user.name=angel", "-c", "user.email=angel@local"])
        .args(args);
    let captured = super::exec::output_timed_captured_cancellable(command, git_timeout(), cancel)?;
    if captured.timed_out
        || captured.cancelled
        || captured.stdout_truncated
        || !captured.output.status.success()
    {
        return Err(format!(
            "delegate Git did not complete: {}",
            String::from_utf8_lossy(&captured.output.stderr)
        ));
    }
    String::from_utf8(captured.output.stdout)
        .map_err(|_| "delegate Git metadata is not UTF-8".into())
}

pub(crate) struct IntegrateTool {
    workspace: PathBuf,
}

pub(crate) struct RepoIntegrateLock {
    file: std::fs::File,
}

impl Drop for RepoIntegrateLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        // SAFETY: this descriptor is owned by `self` and remains open through
        // the syscall. Closing it immediately afterwards would also unlock.
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

/// Cross-process companion to [`INTEGRATE_LOCK`]. `flock` is advisory but all
/// angelX integrators use this common-Git-dir file; the kernel releases it if a
/// process crashes, avoiding stale lock cleanup heuristics.
fn acquire_repo_integrate_lock(repo: &Path) -> Result<RepoIntegrateLock, String> {
    acquire_repo_integrate_lock_with_timeout(repo, git_timeout())
}

fn acquire_repo_integrate_lock_with_timeout(
    repo: &Path,
    timeout: Option<Duration>,
) -> Result<RepoIntegrateLock, String> {
    use std::os::fd::AsRawFd;

    let common = run_git(repo, &["rev-parse", "--git-common-dir"])?;
    let common = PathBuf::from(common.trim());
    let common = if common.is_absolute() {
        common
    } else {
        repo.join(common)
    };
    let path = common.join("angel-integrate.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| format!("open integration lock {}: {e}", path.display()))?;
    let started = Instant::now();
    loop {
        // SAFETY: `file` owns a live descriptor for the duration of this loop.
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            return Ok(RepoIntegrateLock { file });
        }
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if err.kind() != std::io::ErrorKind::WouldBlock {
            return Err(format!(
                "lock integration repository {}: {err}",
                repo.display()
            ));
        }
        if timeout.is_some_and(|limit| started.elapsed() >= limit) {
            return Err(format!(
                "timed out waiting for integration lock on {} after {}s",
                repo.display(),
                timeout.map(|d| d.as_secs()).unwrap_or(0)
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(test)]
pub(crate) fn acquire_repo_integrate_lock_for_test(
    repo: &Path,
    timeout: Duration,
) -> Result<RepoIntegrateLock, String> {
    acquire_repo_integrate_lock_with_timeout(repo, Some(timeout))
}

/// Reasoning is a breakdown of output tokens, not extra generated tokens.
fn delegate_token_delta(
    before: crate::agent::club::TokenUsage,
    after: crate::agent::club::TokenUsage,
) -> u64 {
    after
        .total_input
        .saturating_add(after.total_output)
        .saturating_sub(before.total_input.saturating_add(before.total_output))
}

#[cfg(test)]
include!("../../../../tests/cockpit/harness/orchestrator__standalone_tests.rs");

impl IntegrateTool {
    pub(crate) fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
    pub(crate) fn run(&self, branch: &str) -> Result<String, String> {
        self.run_native(branch, None, &Default::default())
            .map(|result| result.text)
    }

    fn run_native(
        &self,
        branch: &str,
        cancel: Option<&AtomicBool>,
        context: &super::delegated_lineage::NativeToolContext,
    ) -> Result<super::delegated_lineage::NativeToolResult, String> {
        let _process_guard = INTEGRATE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let branch = branch.trim();
        if branch.is_empty() || branch.starts_with('-') {
            return Err("invalid branch name".to_string());
        }
        let repo = ensure_git_workspace(&self.workspace)?;
        let _repo_guard = acquire_repo_integrate_lock(&repo)?;
        run_git(&repo, &["check-ref-format", "--branch", branch])?;
        let selected = delegate_git(
            &repo,
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{branch}^{{commit}}"),
            ],
            cancel,
        )?
        .trim()
        .to_string();
        let artifact = context.artifact(branch);
        if let Some(artifact) = artifact.as_ref() {
            artifact.validate()?;
            if artifact.parent_workspace_sha256 != delegate_workspace_digest(&self.workspace)
                || artifact.tip_oid != selected
                || artifact.tree_oid
                    != delegate_git(
                        &repo,
                        &["rev-parse", &format!("{selected}^{{tree}}")],
                        cancel,
                    )?
                    .trim()
                || artifact.diff_sha256
                    != delegate_diff_digest(&repo, &artifact.base_oid, &selected, cancel)?
            {
                return Err(
                    "delegated artifact changed since capture; integration not started".into(),
                );
            }
        }
        let before_commit = delegate_git(&repo, &["rev-parse", "HEAD"], cancel)?
            .trim()
            .to_string();
        let before_tree = delegate_git(&repo, &["rev-parse", "HEAD^{tree}"], cancel)?
            .trim()
            .to_string();
        let msg = format!("integrate {branch}");
        match delegate_git(
            &repo,
            &["merge", "--no-ff", "-m", msg.as_str(), "--", &selected],
            cancel,
        ) {
            Ok(_) => {
                let application = super::delegated_lineage::DelegateApplication {
                    before_commit,
                    before_tree,
                    after_commit: delegate_git(&repo, &["rev-parse", "HEAD"], cancel)?
                        .trim()
                        .to_string(),
                    after_tree: delegate_git(&repo, &["rev-parse", "HEAD^{tree}"], cancel)?
                        .trim()
                        .to_string(),
                };
                Ok(super::delegated_lineage::NativeToolResult {
                    text: format!("integrated {branch} at {selected}"),
                    artifact,
                    branch: None,
                    application: Some(application),
                })
            }
            Err(e) => {
                let conflicts =
                    run_git(&repo, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
                let _ = run_git(&repo, &["merge", "--abort"]);
                Err(format!(
                    "merge conflict on {branch} (aborted); conflicts: {}; {e}",
                    conflicts.trim()
                ))
            }
        }
    }
}

impl Tool for IntegrateTool {
    fn name(&self) -> &str {
        "integrate"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "integrate".to_string(),
            description:
                "Merge a delegated branch into the shared workspace (serialized, one at a \
                          time). Reports conflicts instead of leaving a mess."
                    .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": { "branch": { "type": "string", "description": "branch from delegate" } },
                "required": ["branch"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let branch = args["branch"].as_str().ok_or("missing 'branch'")?;
        self.run(branch)
    }
    fn call_with_native_context(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
        _progress: Option<Arc<ToolOutputProgress>>,
        context: &super::delegated_lineage::NativeToolContext,
    ) -> Result<super::delegated_lineage::NativeToolResult, String> {
        self.run_native(
            args["branch"].as_str().ok_or("missing 'branch'")?,
            cancel,
            context,
        )
    }
}
