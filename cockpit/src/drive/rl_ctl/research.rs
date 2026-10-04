//! Optional Sloptomizer research on the active loop's route and lifetime.

use super::research_bridge as bridge;
use super::*;
use crate::agent::harness::book::{self, d45_iteration, d2467_research, ow_ledgers};
use crate::agent::harness::{LoopExperimentRequest, LoopExperimentResult};
use serde_json::{Value, json};

#[derive(Default)]
pub(super) struct ResearchState {
    active: Option<ResearchRun>,
    /// Loop verdicts folded into the Sloptomizer (see `observe_loop_verdict`).
    loop_observations: Arc<Mutex<LoopObservations>>,
    notified_run: Option<String>,
}

#[derive(Default)]
struct LoopObservations {
    admitted: usize,
    last_error: Option<String>,
}

struct ResearchRun {
    cancel: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    record: Arc<Mutex<Value>>,
}

impl ResearchState {
    pub(super) fn stop(&self) -> bool {
        self.active.as_ref().is_some_and(|run| {
            !run.done.load(Ordering::Acquire) && !run.cancel.swap(true, Ordering::AcqRel)
        })
    }
    fn status(&self) -> Value {
        let mut status = self
            .active
            .as_ref()
            .map_or(json!({"status":"idle"}), |run| {
                let mut value = run.record.lock().unwrap_or_else(|e| e.into_inner()).clone();
                value["stop_requested"] = json!(run.cancel.load(Ordering::Acquire));
                value["running"] = json!(!run.done.load(Ordering::Acquire));
                value
            });
        let tally = self
            .loop_observations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if tally.admitted > 0 || tally.last_error.is_some() {
            status["loop_observations"] =
                json!({"admitted": tally.admitted, "last_error": tally.last_error});
        }
        status
    }
}

fn runs_root(workspace: &Path) -> PathBuf {
    workspace_run_root(workspace).join("research")
}

fn scope_root(
    workspace: &Path,
    context: &LoopCampaignContext,
    task: &str,
    verify: Option<&str>,
) -> PathBuf {
    learning_scope(workspace, &context.club.route_identity(), task, verify)
}

/// One learner per objective: loop_research runs, verified /loop iterations
/// and rl_campaign rounds on the same task, verifier and route share a state.
fn learning_scope(
    workspace: &Path,
    route: &crate::agent::club::RouteIdentity,
    task: &str,
    verify: Option<&str>,
) -> PathBuf {
    let scope = json!({"schema":1,"task":task,"verify":verify,"route":route});
    runs_root(workspace)
        .join("learning")
        .join(crate::knowledge::cut::sha256_hex(
            scope.to_string().as_bytes(),
        ))
}

/// Whether /loop verdicts and rl_campaign rounds teach the Sloptomizer
/// (`ANGEL_LOOP_OBSERVE`, default on). An ablation switch for measuring the
/// shared learner: off means only "do not teach", never "refuse".
fn loop_observe_enabled() -> bool {
    crate::agent::harness::env_flag("ANGEL_LOOP_OBSERVE", true)
}

/// The Sloptomizer keys an idea by its exact text, so a loop's direction and a
/// campaign's proposal take one form before they are observed: routes and
/// ledger addresses dropped, whitespace collapsed. The same idea from /loop and
/// from a campaign then lands in the same learning bucket.
pub(super) fn idea_form(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| !('\u{2800}'..='\u{28FF}').contains(c))
                .collect::<String>()
        })
        .filter(|word| !word.is_empty() && !word.contains("ledger://") && word != "·")
        .collect::<Vec<_>>()
        .join(" ")
}

/// A measured rl_campaign round is a paired observation for the Sloptomizer:
/// the reflector's proposal, whether it was promoted over the incumbent, and
/// the measured mean reward difference, in the learning scope loop_research
/// uses for the same task, verifier and route. A round without evaluator
/// receipts carries no physical evidence and is not observed.
pub(super) fn observe_campaign_round(
    workspace: &Path,
    route: &crate::agent::club::RouteIdentity,
    case: &RlCase,
    round_id: &str,
    proposal: &str,
    report: &crate::drive::reinforce::promotion::PromotionReport,
    cancel: &AtomicBool,
) -> Result<(), String> {
    const MAX_IDEA_CHARS: usize = 4000;
    let idea: String = idea_form(proposal).chars().take(MAX_IDEA_CHARS).collect();
    if !loop_observe_enabled() || idea.is_empty() || report.evaluator_receipt_sha256s.is_empty() {
        return Ok(());
    }
    let receipts = report.evaluator_receipt_sha256s.join("\n");
    let paired_delta = report
        .mean_delta
        .filter(|delta| delta.is_finite())
        .map(|delta| f64::from(delta.clamp(-1.0, 1.0)));
    let observation = json!({"id": round_id, "idea": idea, "approach": "rl_campaign",
        "task": case.task, "passed": report.promoted(), "paired_delta": paired_delta,
        "source_sha256": report.cohort_manifest_sha256,
        "receipt_sha256": crate::knowledge::cut::sha256_hex(receipts.as_bytes()),
        "evidence_sha256": report.candidate_prompt_sha256});
    let scope = learning_scope(workspace, route, &case.task, Some(case.verify.as_str()));
    teach(
        &scope,
        &super::research_live::scope(workspace, &case.task, Some(&case.verify)),
        &json!(route),
        observation,
        cancel,
    )
    .map(|_| ())
}

/// One measured result updates existing fitness learning and the shared
/// relationship memory. Context failure is reported without erasing a reward
/// already learned or withholding the experiment's result.
fn teach(
    scope: &Path,
    relationships: &Path,
    route: &Value,
    observation: Value,
    cancel: &AtomicBool,
) -> Result<Value, String> {
    let task = observation["task"].as_str().unwrap_or_default();
    let event = json!({
        "id": format!("experiment:{}", observation["id"].as_str().unwrap_or_default()),
        "check": crate::knowledge::cut::sha256_hex(json!(["objective-verifier", task]).to_string().as_bytes()),
        "receipt": observation["receipt_sha256"], "source_sha256": observation["source_sha256"],
        "tool": observation["approach"], "route": route.to_string(),
        "hypothesis": observation["idea"].as_str().unwrap_or_default().chars().take(240).collect::<String>(),
        "verdict": if observation["passed"] == true { "passed" } else { "failed" },
        "basis": "measured-experiment", "paired_delta": observation["paired_delta"],
    });
    let mut response = bridge::transform(
        scope,
        json!({"action":"observe","task":task,"observation":observation}),
        cancel,
    )?;
    match bridge::transform(
        relationships,
        json!({"action":"relate","events":[event]}),
        cancel,
    ) {
        Ok(result) => response["relations_updated"] = result["updated"].clone(),
        Err(error) => response["relations_error"] = json!(error),
    }
    Ok(response)
}

fn text_arg<'a>(args: &'a Value, key: &str, fallback: &'a str) -> Result<&'a str, String> {
    args.get(key).map_or(Ok(fallback), |v| {
        v.as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| format!("{key} must be a nonempty string"))
    })
}

fn flag(args: &Value, key: &str, fallback: bool) -> Result<bool, String> {
    args.get(key).map_or(Ok(fallback), |v| {
        v.as_bool().ok_or_else(|| format!("{key} must be boolean"))
    })
}

/// A run's record with the `⡪` routes its state raises (live, a verdict, a
/// red baseline, a learning error), their evidence in the workspace ledger.
fn routed(workspace: &Path, mut record: Value) -> Value {
    let routes = d2467_research::research(&record);
    if !routes.is_empty() {
        let evidence = json!({"research_run":record["run_id"],"status":record["status"],
            "measurements":record["measurements"],"learning_error":record["learning_error"],
            "error":record["error"]})
        .to_string();
        let raises: Vec<book::Raise> = routes
            .iter()
            .map(|route| book::Raise::new(*route, Some(evidence.clone())))
            .collect();
        record["warpath"] = json!(book::warpath(workspace, &raises));
    }
    record
}

/// Advice with its evidence note as its `⡪⠓` pages, and the cold-start page
/// when no observation stands behind its ranking yet.
fn routed_advice(mut value: Value) -> Value {
    if let Some(advice) = value.get_mut("advice") {
        if advice.get("evidence_note").is_some() {
            advice["evidence_note"] = json!(d2467_research::EVIDENCE_NOTE);
        }
        if d2467_research::cold_advice(advice) {
            value["warpath"] = json!(d2467_research::COLD);
        }
    }
    value
}

fn persist(dir: &Path, record: &Value) -> Result<(), String> {
    bridge::write(
        &dir.join("research.json"),
        &serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?,
    )
}

impl RlState {
    /// Deliver a settled experiment to the current turn once. Polling this
    /// reads in-memory scalars; the model need not burn tool calls waiting.
    pub(crate) fn take_research_notice(&mut self, workspace: &Path) -> Option<String> {
        let run = self.research.active.as_ref()?;
        if !run.done.load(Ordering::Acquire) {
            return None;
        }
        let record = run.record.lock().unwrap_or_else(|e| e.into_inner());
        let id = record["run_id"].as_str()?;
        if self.research.notified_run.as_deref() == Some(id) {
            return None;
        }
        self.research.notified_run = Some(id.to_string());
        let routes = d2467_research::research(&record);
        if routes.is_empty() {
            return None;
        }
        let evidence = json!({"run":id,"status":record["status"],
            "measurements":record["measurements"],"learning_error":record["learning_error"]})
        .to_string();
        let raises = routes
            .into_iter()
            .map(|route| book::Raise::new(route, Some(evidence.clone())))
            .collect::<Vec<_>>();
        Some(format!(
            "⚠{}\n{}",
            book::warpath(workspace, &raises),
            evidence
        ))
    }
    /// The live observer and explicit experiments use one objective context.
    pub(crate) fn research_objective(&self, fallback: &str) -> (String, Option<String>) {
        self.loop_context.as_ref().map_or_else(
            || (fallback.to_string(), None),
            |context| (context.task.clone(), context.verify.clone()),
        )
    }
    /// A loop_research run is in flight.
    /// A verified /loop iteration is an observation for the Sloptomizer: the
    /// direction it named, the acceptance verdict, and the evidence digests,
    /// in the learning scope loop_research uses for this loop (its task,
    /// verifier and route), so `suggest` ranks ideas with the loop's own
    /// history. Unpaired, like a run without compare. Runs off the caller's
    /// thread; the store's lock orders it against a research run's learning.
    pub(crate) fn observe_loop_verdict(
        &self,
        workspace: &Path,
        iteration: usize,
        idea: &str,
        passed: bool,
        receipt: &crate::drive::loop_ctl::VerifyReceipt,
    ) -> Option<std::thread::JoinHandle<()>> {
        let context = self.loop_context.clone()?;
        let idea = idea_form(idea);
        if !loop_observe_enabled() || idea.is_empty() || context.task.trim().is_empty() {
            return None;
        }
        let scope = scope_root(
            workspace,
            &context,
            &context.task,
            context.verify.as_deref(),
        );
        let observation = json!({"id": format!("{}-iteration-{iteration}", context.loop_id),
            "idea": idea, "approach": "loop-iteration", "task": context.task, "passed": passed,
            "paired_delta": null, "source_sha256": receipt.workspace_sha256,
            "receipt_sha256": receipt.manifest_sha256, "evidence_sha256": receipt.manifest_sha256,
            "loop_id": context.loop_id});
        let relationships =
            super::research_live::scope(workspace, &context.task, context.verify.as_deref());
        let route = json!(context.club.route_identity());
        let tally = Arc::clone(&self.research.loop_observations);
        std::thread::Builder::new()
            .name("angel-loop-observe".into())
            .spawn(move || {
                let outcome = teach(
                    &scope,
                    &relationships,
                    &route,
                    observation,
                    &AtomicBool::new(false),
                );
                let mut tally = tally.lock().unwrap_or_else(|e| e.into_inner());
                match outcome {
                    Ok(_) => tally.admitted += 1,
                    Err(error) => tally.last_error = Some(error),
                }
            })
            .ok()
    }

    pub(crate) fn research_running(&self) -> bool {
        self.research
            .active
            .as_ref()
            .is_some_and(|run| !run.done.load(Ordering::Acquire))
    }

    pub(crate) fn research_call(
        &mut self,
        workspace: &Path,
        args: &Value,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        if cancel.load(Ordering::Acquire) {
            return Err("loop research cancelled before dispatch".into());
        }
        match args["action"].as_str().unwrap_or("options") {
            "options" => Ok(json!({
                "available":self.loop_enabled(), "engine":"sloptomizer", "experimental":true,
                "methods":["pareto","bandit","memory"],
                "actions":["options","context","suggest","run","status","results","stop"],
                // The notes are `⠪⠚` pages; the facts around them are data.
                "execution":ow_ledgers::RESEARCH_EXECUTION,
                "learning":ow_ledgers::RESEARCH_LEARNING,
                "runtime":ow_ledgers::RESEARCH_RUNTIME,
                "related":["rl_campaign","consult_model(method=deli, club=self)","spawn(formation=moa)","continual_harness"]
            }).to_string()),
            "context" => {
                let fallback = self.loop_context.as_ref().map(|c| c.task.as_str()).unwrap_or("");
                let task = text_arg(args, "task", fallback)?;
                if task.is_empty() { return Err("context needs task outside an active loop".into()); }
                let verify = match args.get("verify") {
                    None => self.loop_context.as_ref().and_then(|c| c.verify.as_deref()),
                    Some(Value::Null) => None,
                    Some(value) => Some(value.as_str().filter(|s| !s.trim().is_empty())
                        .ok_or("verify must be a nonempty command or null")?),
                };
                let advice = super::research_live::context(workspace, task, verify, cancel)?;
                let cue = book::d12467_sloptomizer::context_turn(workspace, &advice);
                Ok(json!({"advice":book::d12467_sloptomizer::data_value(&advice),"context":cue}).to_string())
            }
            "status" => Ok(routed(workspace, self.research.status()).to_string()),
            "stop" => Ok(json!({"stop_requested":self.research.stop(),"research":routed(workspace, self.research.status())}).to_string()),
            "results" => {
                let id = args.get("run_id").map(|v| v.as_str().ok_or("run_id must be a string")).transpose()?;
                let limit = args.get("limit").map(|v| v.as_u64().filter(|n| *n > 0).ok_or("limit must be positive")).transpose()?.unwrap_or(5);
                let runs = self.research_results(workspace, id, usize::try_from(limit).unwrap_or(usize::MAX))?;
                let runs: Vec<Value> = runs.into_iter().map(|row| routed(workspace, row)).collect();
                Ok(json!({"runs":runs}).to_string())
            }
            action @ ("suggest" | "run") => {
                let context = self.loop_context.clone().ok_or(d45_iteration::RESEARCH_NEEDS_LOOP)?;
                let task = text_arg(args, "task", &context.task)?.to_owned();
                let verify = match args.get("verify") {
                    None => context.verify.clone(), Some(Value::Null) => None,
                    Some(v) => Some(v.as_str().filter(|s| !s.trim().is_empty()).ok_or("verify must be a nonempty command or null")?.to_owned()),
                };
                let scope = scope_root(workspace, &context, &task, verify.as_deref());
                if action == "suggest" {
                    let mut request = json!({"action":"suggest","task":task});
                    for key in ["methods","seed","candidates"] {
                        if let Some(value) = args.get(key) { request[key] = value.clone(); }
                    }
                    if let Some(candidates) = request.get("candidates") {
                        for row in candidates.as_array().ok_or("candidates must be an array")? {
                            if text_arg(row,"idea", "")?.trim().is_empty() {
                                return Err(d45_iteration::CANDIDATE_NEEDS_IDEA.into());
                            }
                            text_arg(row,"approach","direct")?;
                        }
                    }
                    let mut result = bridge::transform(&scope, request, cancel)?;
                    // Cross-model relationships are contextual evidence, kept
                    // distinct from the route's measured fitness statistics.
                    match super::research_live::context(workspace, &task, verify.as_deref(), cancel) {
                        Ok(relations) => result["advice"]["relations"] = book::d12467_sloptomizer::data_value(&relations),
                        Err(error) => result["advice"]["relations_error"] = json!(error),
                    }
                    return Ok(routed_advice(result).to_string());
                }
                let idea = text_arg(args,"idea", "")?.to_owned();
                if idea.trim().is_empty() { return Err(d45_iteration::RUN_NEEDS_IDEA.into()); }
                let approach = text_arg(args,"approach", "direct")?.to_owned();
                let compare = flag(args,"compare",false)?;
                let use_memory = flag(args,"use_memory",true)?;
                if compare && verify.is_none() { return Err(d45_iteration::RESEARCH_NEEDS_VERIFIER.into()); }
                if self.research.active.as_ref().is_some_and(|run| !run.done.load(Ordering::Acquire)) {
                    return Err(ow_ledgers::RESEARCH_ACTIVE.into());
                }
                // Probe before spending model calls; invalid/missing state never
                // silently falls back to fresh learning.
                let advice_started = Instant::now();
                let advice = bridge::transform(&scope, json!({"action":"suggest","task":task,"methods":if use_memory { vec!["memory"] } else { vec![] }}),cancel)?;
                let id = new_run_id();
                let dir = runs_root(workspace).join(&id);
                bridge::ensure_dir(&dir)?;
                let record = json!({"schema":"angel.loop-research/v1", "run_id":id,
                    "loop_id":context.loop_id, "status":"running", "task":task,"verify":verify,
                    "idea":idea,"approach":approach,"compare":compare,"use_memory":use_memory,
                    "route":context.club.route_identity(),"artifacts":dir,"state_path":scope.join("state.json"),
                    "memory":advice["advice"]["memory"],"experimental":true,
                    "timings_ms":{"advice":advice_started.elapsed().as_millis()}});
                persist(&dir, &record)?;
                let shared = Arc::new(Mutex::new(record));
                let done = Arc::new(AtomicBool::new(false));
                let worker_cancel = Arc::new(AtomicBool::new(false));
                let (club,budget) = loop_campaign::campaign_club(Arc::clone(&context.club),Some(&context),Arc::clone(&self.loop_tokens),&worker_cancel);
                let source = workspace.to_path_buf();
                let worker_record = Arc::clone(&shared);
                let worker_done = Arc::clone(&done);
                let worker_flag = Arc::clone(&worker_cancel);
                let launched = dir.clone();
                std::thread::Builder::new().name("angel-loop-research".into()).spawn(move || {
                    let worker_started = Instant::now();
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let result = run(&source,&dir,&scope,club,&worker_flag,&worker_record);
                        budget.finish(result)
                    })).unwrap_or_else(|_| Err(d45_iteration::RESEARCH_PANICKED.into()));
                    let mut record = worker_record.lock().unwrap_or_else(|e| e.into_inner());
                    record["timings_ms"]["worker_total"] = json!(worker_started.elapsed().as_millis());
                    record["status"] = json!(if worker_flag.load(Ordering::Acquire) { "stopped" } else if outcome.is_err() { "failed" } else if !record["learning_error"].is_null() { "completed_with_learning_error" } else { "completed" });
                    if let Err(error) = outcome { record["error"] = json!(error); }
                    if let Err(error) = persist(&dir,&record) { record["retention_error"] = json!(error); }
                    worker_done.store(true,Ordering::Release);
                }).map_err(|e| format!("{} {e}", d45_iteration::RESEARCH_UNSTARTED))?;
                self.research.active = Some(ResearchRun { cancel:worker_cancel,done,record:shared });
                Ok(json!({"run_id":id,"artifacts":launched,"status":"running","message":ow_ledgers::RESEARCH_RUNNING}).to_string())
            }
            _ => Err(d45_iteration::RESEARCH_UNKNOWN_ACTION.into()),
        }
    }

    fn research_results(
        &self,
        workspace: &Path,
        id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        if id.is_some_and(|id| {
            !id.starts_with("run-") || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        }) {
            return Err(d45_iteration::RESEARCH_ID_INVALID.into());
        }
        let mut dirs = match std::fs::read_dir(runs_root(workspace)) {
            Ok(entries) => entries
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter(|e| {
                    e.file_type().is_ok_and(|t| t.is_dir())
                        && e.file_name().to_string_lossy().starts_with("run-")
                })
                .map(|e| e.path())
                .collect::<Vec<_>>(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.to_string()),
        };
        dirs.sort_by(|a, b| b.cmp(a));
        let active = self.research.status();
        let mut rows = Vec::new();
        for dir in dirs {
            if id.is_some_and(|id| dir.file_name().is_none_or(|name| name != id)) {
                continue;
            }
            let mut row: Value = serde_json::from_slice(&bridge::read(&dir.join("research.json"))?)
                .map_err(|e| format!("research receipt corrupt: {e}"))?;
            if id.is_none()
                && self
                    .loop_context
                    .as_ref()
                    .is_some_and(|c| row["loop_id"].as_str() != Some(&c.loop_id))
            {
                continue;
            }
            if row["run_id"] == active["run_id"] && row["artifacts"] == active["artifacts"] {
                row = active.clone();
            } else if row["status"] == "running" {
                row["status"] = json!("incomplete");
            }
            rows.push(row);
            if rows.len() >= limit {
                break;
            }
        }
        if id.is_some() && rows.is_empty() {
            return Err(d45_iteration::RESEARCH_ID_UNKNOWN.into());
        }
        Ok(rows)
    }

    /// The loop's research history: data beside `⠪⠓`, whose words are the
    /// ledger pages.
    pub(super) fn research_context(&self, workspace: &Path) -> String {
        let mut text = String::new();
        match self.research_results(workspace, None, 3) {
            Ok(rows) => {
                for row in rows {
                    let row = routed(workspace, row);
                    let mut summary = json!({"research_run":row["run_id"],"status":row["status"],"warpath":row["warpath"],"artifacts":row["artifacts"],"measurements":row["measurements"],"timings_ms":row["timings_ms"],"learning_error":row["learning_error"],"error":row["error"],"retention_error":row["retention_error"]});
                    if summary["warpath"].is_null()
                        && let Some(fields) = summary.as_object_mut()
                    {
                        fields.remove("warpath");
                    }
                    let summary = summary.to_string();
                    // Full evidence and errors remain in results/artifacts.
                    text.push_str(&summary.chars().take(4000).collect::<String>());
                    text.push('\n');
                }
            }
            Err(e) => text.push_str(&format!(
                "{} {e}\n",
                crate::agent::harness::book::ow_ledgers::RESEARCH_HISTORY_UNAVAILABLE
            )),
        }
        text
    }
}

/// Only a completed, persisted physical verifier contributes a learning event.
/// Cancelled/unverified/model-error attempts still retain their artifacts.
fn measured(result: &LoopExperimentResult) -> Option<bool> {
    let verification = result.verification.as_ref()?;
    let evidence = result.evidence.as_ref()?;
    if result.error.is_some()
        || result.stop_reason != "answer"
        || result.result_sha256.is_none()
        || result.evidence_path.is_none()
        || verification.timed_out
        || verification.cancelled
    {
        return None;
    }
    evidence.exit_code().map(|code| code == 0)
}

fn attempt(
    source: &Path,
    dir: &Path,
    task: String,
    verify: Option<String>,
    club: Arc<dyn Club>,
    cancel: &Arc<AtomicBool>,
) -> Result<LoopExperimentResult, String> {
    crate::agent::harness::run_loop_experiment(
        LoopExperimentRequest {
            workspace: source.into(),
            artifact_dir: dir.into(),
            task,
            max_hops: 0,
            deadline_secs: 0,
            token_budget: 0,
            verify_command: verify,
            policy_note: None,
        },
        club,
        Arc::clone(cancel),
    )
}

fn run(
    workspace: &Path,
    dir: &Path,
    scope: &Path,
    club: Arc<dyn Club>,
    cancel: &Arc<AtomicBool>,
    record: &Arc<Mutex<Value>>,
) -> Result<(), String> {
    let launch = record.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let task = launch["task"].as_str().ok_or("research task missing")?;
    let verify = launch["verify"].as_str().map(str::to_owned);
    let source = dir.join("source");
    let snapshot_started = Instant::now();
    let source_sha = crate::agent::harness::freeze_active_source(workspace, &source, cancel)?;
    record.lock().unwrap_or_else(|e| e.into_inner())["timings_ms"]["source_snapshot"] =
        json!(snapshot_started.elapsed().as_millis());
    let baseline = if launch["compare"] == true {
        let started = Instant::now();
        let result = attempt(
            &source,
            &dir.join("baseline"),
            task.into(),
            verify.clone(),
            Arc::clone(&club),
            cancel,
        )?;
        let mut row = record.lock().unwrap_or_else(|e| e.into_inner());
        row["timings_ms"]["baseline"] = json!(started.elapsed().as_millis());
        row["baseline"] = serde_json::to_value(&result).map_err(|e| e.to_string())?;
        persist(dir, &row)?;
        if measured(&result).is_none() {
            return Err(format!(
                "paired research baseline has no completed verifier evidence ({}); candidate was not started",
                result.error.as_deref().unwrap_or(&result.stop_reason)
            ));
        }
        Some(result)
    } else {
        None
    };
    if cancel.load(Ordering::Acquire) {
        return Err("research cancelled after baseline".into());
    }
    // The attempt reads its labels off `⠘⠊`; the idea and memory are data.
    let candidate_task = format!(
        "{task}\n\n{}\n{}\n\n{}\n{}",
        d45_iteration::RESEARCH_APPROACH,
        launch["idea"].as_str().unwrap_or_default(),
        d45_iteration::RESEARCH_SNIPPETS,
        launch["memory"]
    );
    let candidate_started = Instant::now();
    let candidate = attempt(
        &source,
        &dir.join("candidate"),
        candidate_task,
        verify,
        club,
        cancel,
    )?;
    let candidate_pass = measured(&candidate);
    let baseline_pass = baseline.as_ref().and_then(measured);
    let delta = candidate_pass
        .zip(baseline_pass)
        .map(|(c, b)| i32::from(c) - i32::from(b));
    {
        let mut row = record.lock().unwrap_or_else(|e| e.into_inner());
        row["timings_ms"]["candidate"] = json!(candidate_started.elapsed().as_millis());
        row["source_sha256"] = json!(source_sha);
        row["candidate"] = serde_json::to_value(&candidate).map_err(|e| e.to_string())?;
        row["measurements"] = json!({"candidate_passed":candidate_pass,"baseline_passed":baseline_pass,"paired_delta":delta});
        persist(dir, &row)?;
    }
    if cancel.load(Ordering::Acquire) {
        return Err("research cancelled; no cancelled attempt rewarded".into());
    }
    let learning_started = Instant::now();
    let learning = (|| {
        let mut updates = Vec::new();
        for (suffix, result, passed, idea, approach, paired_delta) in [
            (
                "baseline",
                baseline.as_ref(),
                baseline_pass,
                "Direct attempt without the selected research idea",
                "baseline",
                None,
            ),
            (
                "candidate",
                Some(&candidate),
                candidate_pass,
                launch["idea"].as_str().unwrap(),
                launch["approach"].as_str().unwrap(),
                delta,
            ),
        ] {
            if let (Some(result), Some(passed)) = (result, passed) {
                if result.snapshot_sha256 != source_sha {
                    return Err("experiment source mismatch; no feedback admitted".into());
                }
                let observation = json!({"id":format!("{}-{suffix}",launch["run_id"].as_str().unwrap()),
                    "idea":idea,"approach":approach,"task":task,"passed":passed,"paired_delta":paired_delta,
                    "source_sha256":source_sha,"receipt_sha256":result.result_sha256,
                    "evidence_sha256":result.evidence.as_ref().unwrap().manifest_sha256(),"loop_id":launch["loop_id"]});
                updates.push(teach(
                    scope,
                    &super::research_live::scope(workspace, task, launch["verify"].as_str()),
                    &launch["route"],
                    observation,
                    cancel,
                )?);
            }
        }
        Ok::<_, String>(updates)
    })();
    let mut row = record.lock().unwrap_or_else(|e| e.into_inner());
    row["timings_ms"]["learning"] = json!(learning_started.elapsed().as_millis());
    match learning {
        Ok(updates) => row["learning"] = json!(updates),
        Err(e) => row["learning_error"] = json!(e),
    }
    persist(dir, &row)?;
    if let Some(error) = candidate.error {
        return Err(error);
    }
    Ok(())
}
