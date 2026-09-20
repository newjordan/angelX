use super::schema::{CampaignOwnership, Contribution, NodeState, RunState, SwarmRun};
use super::store::compact_scrubbed;
use super::tool::{
    AuthorizedCampaignBase, CampaignBase, CampaignCompileRequest, CompileRequest, PreparedSwarmRun,
    SwarmCompilerEngine, SwarmRunOutcome, SwarmRunReceipt,
};
use crate::agent::harness::{DelegateMode, DelegateOutcome, ensure_git_workspace, run_git};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

impl SwarmCompilerEngine {
    pub(super) fn start(&self, request: CompileRequest) -> Result<String, String> {
        let base = self.inspect_campaign_base()?;
        let run_id = self.store.new_run_id();
        let run = self.prepare_run(request, base, run_id, None)?;
        self.execute(run)
    }

    /// Resolve and freeze the exact clean base used by an internal campaign.
    /// This is read-only and may run off the UI thread.
    pub(crate) fn inspect_campaign_base(&self) -> Result<CampaignBase, String> {
        let repo = ensure_git_workspace(&self.workspace)?;
        let workspace = self
            .workspace
            .canonicalize()
            .map_err(|e| format!("resolve workspace {}: {e}", self.workspace.display()))?;
        let workspace_rel = workspace
            .strip_prefix(&repo)
            .map_err(|_| "workspace is outside its git root".to_string())?
            .to_path_buf();
        let workspace_pathspec = path_text(&workspace_rel);
        let dirty = run_git(
            &repo,
            &["status", "--porcelain", "--", workspace_pathspec.as_str()],
        )?;
        if !dirty.trim().is_empty() {
            return Err(
                "swarm_compile requires a clean workspace so its frozen base cannot omit user changes"
                    .to_string(),
            );
        }
        let base_oid = run_git(&repo, &["rev-parse", "HEAD"])?.trim().to_string();
        Ok(CampaignBase {
            workspace,
            repo_root: repo,
            workspace_rel: workspace_pathspec,
            base_oid,
        })
    }

    /// Persist an idempotent, campaign-owned swarm graph without executing it.
    /// The caller must attach the returned run id to campaign state before
    /// calling [`campaign_execute`](Self::campaign_execute).
    pub(crate) fn campaign_prepare(
        &self,
        request: CampaignCompileRequest,
        authorization: AuthorizedCampaignBase,
    ) -> Result<PreparedSwarmRun, String> {
        validate_authorization(&authorization)?;
        let current = self.inspect_campaign_base()?;
        if current.workspace != authorization.base.workspace
            || current.repo_root != authorization.base.repo_root
            || current.workspace_rel != authorization.base.workspace_rel
        {
            return Err("campaign project changed before swarm preparation".to_string());
        }
        let commit_spec = format!("{}^{{commit}}", authorization.base.base_oid);
        let resolved = run_git(
            &authorization.base.repo_root,
            &["rev-parse", "--verify", commit_spec.as_str()],
        )?;
        if resolved.trim() != authorization.base.base_oid {
            return Err("campaign frozen base no longer resolves exactly".to_string());
        }
        let run_id = campaign_run_id(&authorization);
        if self.store.has_run(&run_id)? {
            let existing = self.store.load(&run_id)?;
            validate_campaign_run(&existing, &authorization)?;
            validate_campaign_request(&existing, &request)?;
            return Ok(PreparedSwarmRun {
                run_id,
                base_oid: existing.base_oid,
            });
        }
        let ownership = CampaignOwnership {
            campaign_id: authorization.campaign_id,
            campaign_revision: authorization.campaign_revision,
            round: authorization.round,
            contract_digest: authorization.contract_digest,
        };
        let mut authorized_base = current;
        authorized_base.base_oid = authorization.base.base_oid;
        let run = self.prepare_run(
            request.into(),
            authorized_base,
            run_id.clone(),
            Some(ownership),
        )?;
        Ok(PreparedSwarmRun {
            run_id,
            base_oid: run.base_oid,
        })
    }

    /// Execute or resume one previously prepared campaign run. A provider or
    /// stage failure is returned as a durable Paused receipt, not as permission
    /// to create a replacement run.
    pub(crate) fn campaign_execute(
        &self,
        run_id: String,
        authorization: AuthorizedCampaignBase,
        cancelled: &AtomicBool,
    ) -> Result<SwarmRunReceipt, String> {
        validate_authorization(&authorization)?;
        let run = self.store.load(&run_id)?;
        validate_campaign_run(&run, &authorization)?;
        if !run.state.terminal() {
            let _ = self.execute_with_cancel(run, Some(cancelled));
        }
        let durable = self.store.load(&run_id)?;
        validate_campaign_run(&durable, &authorization)?;
        self.campaign_receipt(&durable)
    }

    fn prepare_run(
        &self,
        request: CompileRequest,
        base: CampaignBase,
        run_id: String,
        campaign: Option<CampaignOwnership>,
    ) -> Result<SwarmRun, String> {
        let roles = ["investigator", "test_author", "implementer", "reviewer"];
        let routes = roles
            .iter()
            .map(|role| self.resolve_route(&request.task_type, role, request.route_for(role)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut run = SwarmRun::new(
            run_id,
            base.workspace.to_string_lossy().into_owned(),
            base.repo_root.to_string_lossy().into_owned(),
            base.workspace_rel,
            base.base_oid,
            compact_scrubbed(&request.goal, 4_000),
            request.task_type,
            request.targeted_test_cmd,
            request.accept_cmd,
            request.quality_cmds,
            request.test_scope,
            routes,
            now_ms(),
        );
        run.campaign = campaign;
        self.store.save(&run)?;
        self.store.event(
            &run.id,
            "decision",
            "run_created",
            "proof graph and routes frozen",
        )?;
        Ok(run)
    }

    pub(super) fn resume(&self, run_id: String) -> Result<String, String> {
        let run = self.store.load(&run_id)?;
        if run.state.terminal() {
            return render_run(&run, self.store.run_dir(&run.id)?.join("run.json"));
        }
        let current = self
            .workspace
            .canonicalize()
            .map_err(|e| format!("resolve workspace {}: {e}", self.workspace.display()))?;
        if current.as_path() != Path::new(&run.workspace) {
            return Err(format!(
                "run {} belongs to {}, not {}",
                run.id,
                run.workspace,
                current.display()
            ));
        }
        self.store.event(
            &run.id,
            "decision",
            "run_resumed",
            "continuing from durable graph state",
        )?;
        self.execute(run)
    }

    pub(super) fn status(&self, run_id: String) -> Result<String, String> {
        let run = self.store.load(&run_id)?;
        render_run(&run, self.store.run_dir(&run.id)?.join("run.json"))
    }

    fn campaign_receipt(&self, run: &SwarmRun) -> Result<SwarmRunReceipt, String> {
        let outcome = match run.state {
            RunState::Verified => SwarmRunOutcome::Verified,
            RunState::Rejected => SwarmRunOutcome::Rejected,
            RunState::Planning | RunState::Running | RunState::Paused => SwarmRunOutcome::Paused,
        };
        let candidate_oid = run
            .parked_branch
            .as_deref()
            .map(|branch| run_git(Path::new(&run.repo_root), &["rev-parse", branch]))
            .transpose()?
            .map(|oid| oid.trim().to_string());
        let changed_paths = match (run.parked_branch.as_deref(), candidate_oid.as_ref()) {
            (Some(branch), Some(_)) => {
                super::verify::changed_paths(Path::new(&run.repo_root), &run.base_oid, branch)?
            }
            _ => Vec::new(),
        };
        Ok(SwarmRunReceipt {
            run_id: run.id.clone(),
            outcome,
            base_oid: run.base_oid.clone(),
            parked_branch: run.parked_branch.clone(),
            candidate_oid,
            changed_paths,
            technical_pass: run.verification.technical_pass,
            code_review_pass: run.verification.review_pass,
            proof_path: self.store.run_dir(&run.id)?.join("run.json"),
            error: run.last_error.clone(),
        })
    }

    fn execute(&self, run: SwarmRun) -> Result<String, String> {
        self.execute_with_cancel(run, None)
    }

    fn execute_with_cancel(
        &self,
        mut run: SwarmRun,
        cancelled: Option<&AtomicBool>,
    ) -> Result<String, String> {
        let _lease = self.store.acquire_lease(&run.id)?;
        run.state = RunState::Running;
        run.last_error = None;
        self.persist(&mut run, "run_started", "execution entered")?;
        match self.execute_inner(&mut run, cancelled) {
            Ok(()) => {
                let path = self.store.save(&run)?;
                render_run(&run, path)
            }
            Err(error) => {
                run.state = RunState::Paused;
                run.last_error = Some(compact_scrubbed(&error, 1_000));
                if let Some(node) = run
                    .nodes
                    .iter_mut()
                    .find(|node| node.state == NodeState::Running)
                {
                    node.state = NodeState::Failed;
                }
                run.updated_ms = now_ms();
                let _ = self.store.save(&run);
                let _ = self.store.event(&run.id, "error", "run_paused", &error);
                Err(format!(
                    "swarm run {} paused (resume with action=resume): {error}",
                    run.id
                ))
            }
        }
    }

    pub(super) fn persist(
        &self,
        run: &mut SwarmRun,
        event: &str,
        detail: &str,
    ) -> Result<(), String> {
        run.updated_ms = now_ms();
        self.store.save(run)?;
        self.store.event(&run.id, "info", event, detail)
    }

    pub(super) fn begin_node(&self, run: &mut SwarmRun, role: &str) -> Result<(), String> {
        let node = run
            .node_mut(role)
            .ok_or_else(|| format!("missing proof-graph node '{role}'"))?;
        node.state = NodeState::Running;
        self.persist(run, "node_started", role)
    }

    pub(super) fn finish_node(
        &self,
        run: &mut SwarmRun,
        role: &str,
        state: NodeState,
    ) -> Result<(), String> {
        run.node_mut(role)
            .ok_or_else(|| format!("missing proof-graph node '{role}'"))?
            .state = state;
        self.persist(run, "node_finished", &format!("{role}:{state:?}"))
    }

    pub(super) fn route(&self, run: &SwarmRun, role: &str) -> Result<String, String> {
        run.nodes
            .iter()
            .find(|node| node.role == role)
            .map(|node| node.route.clone())
            .ok_or_else(|| format!("missing route for '{role}'"))
    }

    pub(super) fn delegate_stage(
        &self,
        run: &mut SwarmRun,
        role: &str,
        mode: DelegateMode,
        base_ref: &str,
        prompt: String,
    ) -> Result<DelegateOutcome, String> {
        self.begin_node(run, role)?;
        let route = self.route(run, role)?;
        self.delegate.run_from(&route, &prompt, mode, base_ref)
    }

    pub(super) fn record_contribution(
        &self,
        run: &mut SwarmRun,
        role: &str,
        outcome: &DelegateOutcome,
        branch: Option<String>,
        changed_paths: Vec<String>,
        review_verdict: Option<super::schema::ReviewVerdict>,
    ) -> Result<(), String> {
        run.contributions.push(Contribution {
            task_id: format!("task-{role}"),
            role: role.to_string(),
            club: outcome.club.clone(),
            resolved_route: (!outcome.resolved_route.trim().is_empty())
                .then(|| outcome.resolved_route.clone()),
            model_revision: outcome.model_revision.clone(),
            base_oid: outcome.base_oid.clone(),
            branch,
            diff_hash: (!outcome.diff.trim().is_empty()).then(|| content_hash(&outcome.diff)),
            changed_paths,
            summary: compact_scrubbed(&outcome.answer, 1_500),
            elapsed_ms: outcome.elapsed_ms,
            tokens: outcome.tokens,
            review_verdict,
        });
        self.finish_node(run, role, NodeState::Completed)
    }

    pub(super) fn reject(&self, run: &mut SwarmRun, reason: &str) -> Result<(), String> {
        run.state = RunState::Rejected;
        run.last_error = Some(compact_scrubbed(reason, 1_000));
        run.credits = super::credit::derive_credits(run);
        if !run.credits.is_empty()
            && let Err(error) = self.policy.record(&run.id, &run.task_type, &run.credits)
        {
            let _ = self
                .store
                .event(&run.id, "warn", "routing_policy_update_failed", &error);
        }
        self.persist(run, "run_rejected", reason)
    }
}

fn campaign_run_id(authorization: &AuthorizedCampaignBase) -> String {
    let material = format!(
        "{}\0{}\0{}",
        authorization.campaign_id,
        authorization.contract_digest,
        authorization.base.repo_root.display()
    );
    let digest = crate::knowledge::cut::sha256_hex(material.as_bytes());
    format!("swr-campaign-{}", &digest[..40])
}

fn validate_authorization(authorization: &AuthorizedCampaignBase) -> Result<(), String> {
    if !authorization.campaign_id.starts_with("cmp-")
        || authorization.campaign_id.len() > 96
        || !authorization
            .campaign_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
        || authorization.campaign_revision == 0
        || authorization.round == 0
        || authorization.contract_digest.len() < 16
        || authorization.contract_digest.len() > 64
        || !authorization
            .contract_digest
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        || !(7..=64).contains(&authorization.base.base_oid.len())
        || !authorization
            .base
            .base_oid
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("invalid campaign swarm authorization".to_string());
    }
    Ok(())
}

fn validate_campaign_run(
    run: &SwarmRun,
    authorization: &AuthorizedCampaignBase,
) -> Result<(), String> {
    let expected = CampaignOwnership {
        campaign_id: authorization.campaign_id.clone(),
        campaign_revision: authorization.campaign_revision,
        round: authorization.round,
        contract_digest: authorization.contract_digest.clone(),
    };
    if run.campaign.as_ref() != Some(&expected)
        || run.base_oid != authorization.base.base_oid
        || Path::new(&run.workspace) != authorization.base.workspace
        || Path::new(&run.repo_root) != authorization.base.repo_root
        || run.workspace_rel != authorization.base.workspace_rel
    {
        return Err("campaign swarm ownership/base mismatch".to_string());
    }
    Ok(())
}

fn validate_campaign_request(
    run: &SwarmRun,
    request: &CampaignCompileRequest,
) -> Result<(), String> {
    let goal = compact_scrubbed(&request.goal, 4_000);
    if run.goal != goal
        || run.task_type != request.task_type
        || run.targeted_test_cmd != request.targeted_test_cmd
        || run.accept_cmd != request.accept_cmd
        || run.quality_cmds != request.quality_cmds
        || run.test_scope != request.test_scope
    {
        return Err("existing campaign swarm run has a different frozen request".to_string());
    }
    Ok(())
}

fn content_hash(text: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn path_text(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        path.to_string_lossy().into_owned()
    }
}

fn render_run(run: &SwarmRun, path: PathBuf) -> Result<String, String> {
    let next_step = match run.state {
        RunState::Verified => format!(
            "Candidate is verified and parked on {}. Inspect it, then use integrate explicitly.",
            run.parked_branch.as_deref().unwrap_or("(missing branch)")
        ),
        RunState::Paused => format!("Resume with action=resume, run_id={}", run.id),
        RunState::Rejected => {
            "Inspect the proof report and parked rejected branch; do not integrate.".to_string()
        }
        _ => "Run is still active.".to_string(),
    };
    serde_json::to_string_pretty(&serde_json::json!({
        "ok": run.state == RunState::Verified,
        "run": run,
        "path": path,
        "next_step": next_step,
    }))
    .map_err(|e| format!("render swarm run: {e}"))
}
