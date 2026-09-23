//! Loop ownership and durable handoff for the existing native campaign engine.

use super::*;
use crate::agent::club::{StreamDelta, ToolDef};
use serde_json::{Value, json};

#[derive(Clone)]
pub(crate) struct LoopCampaignContext {
    pub loop_id: String,
    pub task: String,
    pub verify: Option<String>,
    pub club: Arc<dyn Club>,
    /// Explicit operator bounds only. None means unbounded.
    pub deadline: Option<Instant>,
    pub remaining_tokens: Option<usize>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct CampaignRecord {
    schema: String,
    run_id: String,
    loop_id: Option<String>,
    objective: LaunchedObjective,
    route: RouteIdentity,
    pub(super) outcome: Option<Result<CampaignOutcome, String>>,
}

impl CampaignRecord {
    pub(super) fn new(
        id: &str,
        owner: Option<&LoopCampaignContext>,
        plan: &RlPlan,
        route: RouteIdentity,
    ) -> Self {
        Self {
            schema: "angel.rl-handoff/v1".into(),
            run_id: id.into(),
            loop_id: owner.map(|owner| owner.loop_id.clone()),
            objective: LaunchedObjective {
                cases: plan.cases.clone(),
                audit: plan.audit.clone(),
                verifier_scope: plan.verifier_scope.clone(),
            },
            route,
            outcome: None,
        }
    }

    pub(super) fn write(&self, dir: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let temporary = dir.join("campaign.json.tmp");
        std::fs::write(&temporary, bytes)
            .and_then(|()| std::fs::rename(temporary, dir.join("campaign.json")))
            .map_err(|e| format!("could not retain RL handoff: {e}"))
    }

    fn value(&self, dir: &Path) -> Value {
        json!({
            "run_id": self.run_id, "loop_id": self.loop_id,
            "objective": self.objective, "route": self.route,
            "status": match &self.outcome { Some(Ok(_)) => "completed", Some(Err(_)) => "failed", None => "incomplete" },
            "outcome": self.outcome,
            "artifacts": dir,
            "report": dir.join("report.json").is_file().then(|| dir.join("report.json")),
        })
    }
}

impl RlState {
    pub(crate) fn loop_enabled(&self) -> bool {
        self.loop_context.is_some()
    }

    /// The id of the loop this controller is bound to, if any.
    pub(crate) fn bound_loop_id(&self) -> Option<&str> {
        self.loop_context
            .as_ref()
            .map(|context| context.loop_id.as_str())
    }

    pub(crate) fn bind_loop(&mut self, context: LoopCampaignContext) {
        if self.loop_account_owner.as_deref() != Some(&context.loop_id) {
            // A retiring worker retains its old counter and can never charge
            // a replacement loop after it observes cancellation.
            self.loop_tokens = Arc::new(AtomicUsize::new(0));
            self.loop_account_owner = Some(context.loop_id.clone());
        }
        if self
            .loop_context
            .as_ref()
            .is_some_and(|old| old.loop_id != context.loop_id)
        {
            self.end_loop();
        }
        self.loop_context = Some(context);
    }

    /// A campaign belongs to the whole loop, not to an individual model turn.
    /// Pausing/stopping/detaching the loop cancels its work, including a verifier.
    pub(crate) fn end_loop(&mut self) {
        self.research.stop();
        if self.loop_owner.is_some() {
            self.stop();
        }
        self.loop_context = None;
    }

    pub(crate) fn take_loop_tokens(&self) -> usize {
        self.loop_tokens.swap(0, Ordering::AcqRel)
    }

    pub(crate) fn take_loop_tokens_for(&self, loop_id: &str) -> usize {
        if self.loop_account_owner.as_deref() == Some(loop_id) {
            self.take_loop_tokens()
        } else {
            0
        }
    }

    pub(crate) fn tool_call(&mut self, workspace: &Path, args: &Value) -> Result<String, String> {
        let action = args
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("status");
        let mut launched = None;
        match action {
            "run" => {
                let context = self.loop_context.clone().ok_or("rl_campaign run needs an active /loop; /rl run remains available to the operator")?;
                let task = args
                    .get("task")
                    .and_then(Value::as_str)
                    .unwrap_or(&context.task);
                let verify = args.get("verify").and_then(Value::as_str).or(context.verify.as_deref())
                    .ok_or("supply a real verifier in 'verify', or bind /goal cmd; RL needs a measurement")?;
                let mut argv = vec![
                    "--task".into(),
                    task.into(),
                    "--verify".into(),
                    verify.into(),
                ];
                for name in ["rounds", "group", "samples"] {
                    if let Some(value) = args.get(name) {
                        let n = value
                            .as_u64()
                            .filter(|n| *n > 0)
                            .ok_or_else(|| format!("{name} must be a positive integer"))?;
                        if name == "samples" && n < 2 {
                            return Err(
                                "samples must be at least 2 for measured policy comparison".into(),
                            );
                        }
                        argv.extend([format!("--{name}"), n.to_string()]);
                    }
                }
                if let Some(scopes) = args.get("verifier_scope") {
                    for scope in scopes.as_array().ok_or("verifier_scope must be an array")? {
                        argv.extend([
                            "--verify-scope".into(),
                            scope
                                .as_str()
                                .ok_or("verifier_scope entries must be strings")?
                                .into(),
                        ]);
                    }
                }
                if let Some(audits) = args.get("audit") {
                    for audit in audits.as_array().ok_or("audit must be an array")? {
                        let task = audit["task"].as_str().ok_or("audit requires task")?;
                        let verify = audit["verify"].as_str().ok_or("audit requires verify")?;
                        let source = audit["source"]
                            .as_str()
                            .ok_or("audit requires independent source")?;
                        argv.extend(["--audit".into(), format!("{task} :: {verify} :: {source}")]);
                    }
                }
                let plan = RlPlan::parse(workspace, &argv)?;
                launched = Some(self.start_recorded(
                    plan,
                    workspace,
                    Arc::clone(&context.club),
                    Some(context),
                )?);
            }
            "stop" => {
                let requested = self.stop();
                return Ok(
                    json!({"stop_requested": requested, "campaign": self.status_value()})
                        .to_string(),
                );
            }
            "status" => {}
            "results" => {
                let id = args.get("run_id").and_then(Value::as_str);
                let limit = args
                    .get("limit")
                    .map(|v| {
                        v.as_u64()
                            .filter(|n| *n > 0)
                            .ok_or("limit must be positive")
                    })
                    .transpose()?
                    .unwrap_or(5);
                let results = self.retained_results(
                    workspace,
                    id,
                    usize::try_from(limit).unwrap_or(usize::MAX),
                )?;
                return Ok(json!({"campaigns": results}).to_string());
            }
            _ => {
                return Err("unknown rl_campaign action; use run, status, results, or stop".into());
            }
        }
        Ok(json!({"launch": launched, "campaign": self.status_value()}).to_string())
    }

    fn status_value(&self) -> Value {
        let progress = self.progress_snapshot();
        json!({
            "available": self.loop_enabled(),
            "status": if self.running() { "running" } else if progress.outcome.is_some() { "settled" } else { "idle" },
            "stop_requested": self.cancel.as_ref().is_some_and(|c| c.load(Ordering::Acquire)),
            "loop_id": self.loop_owner,
            "run_id": self.run_dir.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy()),
            "artifacts": self.run_dir,
            "objective": progress.objective,
            "attempted": progress.observed_attempts(), "planned_attempts": progress.planned_attempts,
            "passed": progress.passed, "red": progress.red,
            "rounds_done": progress.rounds_done, "rounds_planned": progress.rounds_planned,
            "outcome": progress.outcome, "log_tail": progress.log_tail,
        })
    }

    fn retained_results(
        &self,
        workspace: &Path,
        id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        let root = workspace_run_root(workspace);
        if let Some(id) = id
            && (!id.starts_with("run-")
                || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'))
        {
            return Err("run_id must be a campaign identifier returned by this tool".into());
        }
        let mut dirs = match std::fs::read_dir(&root) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.path())
                .collect::<Vec<_>>(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(format!("could not read campaign history: {e}")),
        };
        dirs.sort_by(|a, b| b.cmp(a));
        let mut results = Vec::new();
        for dir in dirs {
            if id.is_some_and(|id| dir.file_name().is_none_or(|name| name != id)) {
                continue;
            }
            let Some(record) = std::fs::read(dir.join("campaign.json"))
                .ok()
                .and_then(|raw| serde_json::from_slice::<CampaignRecord>(&raw).ok())
            else {
                continue;
            };
            // A run-id lookup is workspace scoped. Default recall is also loop scoped.
            if id.is_none()
                && self
                    .loop_context
                    .as_ref()
                    .is_some_and(|context| record.loop_id.as_deref() != Some(&context.loop_id))
            {
                continue;
            }
            let mut value = record.value(&dir);
            if self.run_dir.as_ref() == Some(&dir) && self.running() {
                value["status"] = json!("running");
            }
            results.push(value);
            if results.len() >= limit {
                break;
            }
        }
        if id.is_some() && results.is_empty() {
            return Err("no retained campaign with that run_id in this workspace".into());
        }
        Ok(results)
    }

    pub(crate) fn loop_context_text(&self, workspace: &Path) -> String {
        if !self.loop_enabled() {
            return String::new();
        }
        let mut text = "[RL campaigns]\nrl_campaign is available throughout this loop: run starts asynchronous measured attempts on the current route; status, results, and stop manage them. Use it when comparing approaches or improving a policy would help. Continue useful work while it runs. Inspect actual verifier outcomes and retained attempt artifacts; apply a useful candidate to the main workspace and verify it there. Submit a verified winner when ready. Campaign availability does not require you to launch one.\nCampaigns without an independent audit are measured exploration; they do not install validated learning.\n".to_string();
        text.push_str("Optional research paths: consult_model(method=\"deli\", club=\"self\") explores directions and returns a synthesis to this turn; spawn(formation=\"moa\") compares parallel approaches; continual_harness retains useful supplemental notes. Choose them when helpful, return to ordinary tools when ready, and check proposals against actual evidence.\n");
        text.push_str(&self.research_context(workspace));
        match self.retained_results(workspace, None, 3) {
            Ok(results) => {
                for result in results {
                    // Keep the fresh iteration compact; full objectives, policies,
                    // evidence and errors remain accessible through results/artifacts.
                    let summary = json!({"run_id": result["run_id"], "status": result["status"],
                        "outcome": result["outcome"], "artifacts": result["artifacts"]});
                    let line = summary.to_string();
                    text.push_str(&line.chars().take(4000).collect::<String>());
                    text.push('\n');
                }
            }
            Err(error) => text.push_str(&format!("History unavailable: {error}\n")),
        }
        text
    }
}

impl Drop for RlState {
    fn drop(&mut self) {
        self.research.stop();
        self.stop();
    }
}

/// Loop limits remain explicit. Campaign model usage is estimated just like
/// the parent loop and is returned to its cumulative ledger between iterations.
struct CampaignBudget {
    cancel: Arc<AtomicBool>,
    tokens: Arc<AtomicUsize>,
    used: AtomicUsize,
    limit: Option<usize>,
    reason: Mutex<Option<String>>,
}

impl CampaignBudget {
    fn charge(&self, tokens: usize) {
        self.tokens.fetch_add(tokens, Ordering::AcqRel);
        let used = self
            .used
            .fetch_add(tokens, Ordering::AcqRel)
            .saturating_add(tokens);
        if self.limit.is_some_and(|limit| used >= limit) {
            self.stop("operator loop token budget reached");
        }
    }
    fn stop(&self, reason: &str) {
        if let Ok(mut slot) = self.reason.lock() {
            slot.get_or_insert_with(|| reason.into());
        }
        self.cancel.store(true, Ordering::Release);
    }
}

pub(super) struct CampaignBudgetScope {
    budget: Option<Arc<CampaignBudget>>,
    done: Option<std::sync::mpsc::Sender<()>>,
}

impl CampaignBudgetScope {
    pub(super) fn finish<T>(mut self, outcome: Result<T, String>) -> Result<T, String> {
        if let Some(done) = self.done.take() {
            let _ = done.send(());
        }
        match (
            outcome,
            self.budget
                .as_ref()
                .and_then(|b| b.reason.lock().ok()?.clone()),
        ) {
            (Err(error), Some(reason)) => Err(format!("{reason}: {error}")),
            (outcome, _) => outcome,
        }
    }
}

pub(super) fn campaign_club(
    inner: Arc<dyn Club>,
    owner: Option<&LoopCampaignContext>,
    tokens: Arc<AtomicUsize>,
    cancel: &Arc<AtomicBool>,
) -> (Arc<dyn Club>, CampaignBudgetScope) {
    let Some(owner) = owner else {
        return (
            inner,
            CampaignBudgetScope {
                budget: None,
                done: None,
            },
        );
    };
    let budget = Arc::new(CampaignBudget {
        cancel: Arc::clone(cancel),
        tokens,
        used: AtomicUsize::new(0),
        limit: owner.remaining_tokens,
        reason: Mutex::new(None),
    });
    let done = owner.deadline.map(|deadline| {
        let (tx, rx) = std::sync::mpsc::channel();
        let watch = Arc::clone(&budget);
        std::thread::spawn(move || {
            if matches!(
                rx.recv_timeout(deadline.saturating_duration_since(Instant::now())),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ) {
                watch.stop("operator loop deadline reached");
            }
        });
        tx
    });
    (
        Arc::new(CampaignClub {
            inner,
            budget: Arc::clone(&budget),
        }),
        CampaignBudgetScope {
            budget: Some(budget),
            done,
        },
    )
}

struct CampaignClub {
    inner: Arc<dyn Club>,
    budget: Arc<CampaignBudget>,
}

impl Club for CampaignClub {
    fn label(&self) -> &str {
        self.inner.label()
    }
    fn env_namespace(&self) -> Option<&str> {
        self.inner.env_namespace()
    }
    fn is_available(&self) -> bool {
        self.inner.is_available()
    }
    fn model_identity(&self) -> Option<String> {
        self.inner.model_identity()
    }
    fn route_identity(&self) -> RouteIdentity {
        self.inner.route_identity()
    }
    fn resolved_route_identity(&self) -> RouteIdentity {
        self.inner.resolved_route_identity()
    }
    fn metadata(&self) -> Option<crate::agent::club::Metadata> {
        self.inner.metadata()
    }
    fn metadata_cached(&self) -> Option<crate::agent::club::Metadata> {
        self.inner.metadata_cached()
    }
    fn reasoning_effort(&self) -> Option<String> {
        self.inner.reasoning_effort()
    }
    fn reasoning_levels(&self) -> &[String] {
        self.inner.reasoning_levels()
    }
    fn usage_accounting(&self) -> crate::agent::club::AccountingView {
        self.inner.usage_accounting()
    }
    fn token_usage(&self) -> Option<crate::agent::club::TokenUsage> {
        self.inner.token_usage()
    }
    fn supports_formation_budget(&self) -> bool {
        self.inner.supports_formation_budget()
    }
    fn bind_run_identity(&self, effort: Option<&str>) -> Result<(), String> {
        self.inner.bind_run_identity(effort)
    }
    fn resolved_route_if_known(&self) -> Option<RouteIdentity> {
        self.inner.resolved_route_if_known()
    }
    fn prompt_cache_capable(&self) -> bool {
        self.inner.prompt_cache_capable()
    }
    fn cache_usage(&self) -> crate::agent::club::CacheUsage {
        self.inner.cache_usage()
    }
    fn truncation_usage(&self) -> crate::agent::club::TruncationUsage {
        self.inner.truncation_usage()
    }
    fn effort_gate_usage(&self) -> crate::agent::club::EffortGateUsage {
        self.inner.effort_gate_usage()
    }
    fn quota_cooldown(&self) -> Option<Duration> {
        self.inner.quota_cooldown()
    }
    fn set_reasoning_effort(&self, effort: &str) -> Option<String> {
        self.inner.set_reasoning_effort(effort)
    }
    fn respond(&self, prompt: &str) -> Result<String, String> {
        match self.chat(&[ChatMsg::user(prompt)], &[])? {
            ClubReply::Text(text) => Ok(text),
            _ => Err("text response required".into()),
        }
    }
    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        self.chat_streaming(messages, tools, &self.budget.cancel, &mut |_| {})
    }
    fn chat_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
    ) -> Result<ClubReply, String> {
        self.chat_streaming_with_effort(messages, tools, effort, &self.budget.cancel, &mut |_| {})
    }
    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.chat_streaming_with_effort(messages, tools, None, cancel, on_delta)
    }
    fn chat_streaming_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        let input = messages
            .iter()
            .map(|m| m.content.len() + serde_json::to_vec(&m.tool_calls).map_or(0, |v| v.len()))
            .sum::<usize>()
            + tools
                .iter()
                .map(|tool| {
                    tool.name.len() + tool.description.len() + tool.params.to_string().len()
                })
                .sum::<usize>();
        self.budget.charge(input.div_ceil(4));
        if cancel.load(Ordering::Acquire) || self.budget.cancel.load(Ordering::Acquire) {
            return Err("campaign cancelled".into());
        }
        let mut streamed = 0usize;
        let reply = self.inner.chat_streaming_with_effort(
            messages,
            tools,
            effort,
            cancel,
            &mut |delta| {
                if let StreamDelta::Content(text) | StreamDelta::Reasoning(text) = delta {
                    let before = streamed.div_ceil(4);
                    streamed = streamed.saturating_add(text.len());
                    self.budget.charge(streamed.div_ceil(4) - before);
                }
                on_delta(delta);
            },
        )?;
        let returned = match &reply {
            ClubReply::Text(text) => text.len(),
            ClubReply::Calls(calls) => serde_json::to_vec(calls).map_or(0, |v| v.len()),
        };
        self.budget
            .charge(returned.saturating_sub(streamed).div_ceil(4));
        Ok(reply)
    }
}
