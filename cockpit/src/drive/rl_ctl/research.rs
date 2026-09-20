//! Optional Sloptomizer research on the active loop's route and lifetime.

use super::research_bridge as bridge;
use super::*;
use crate::agent::harness::{LoopExperimentRequest, LoopExperimentResult};
use serde_json::{Value, json};

#[derive(Default)]
pub(super) struct ResearchState {
    active: Option<ResearchRun>,
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
        self.active
            .as_ref()
            .map_or(json!({"status":"idle"}), |run| {
                let mut value = run.record.lock().unwrap_or_else(|e| e.into_inner()).clone();
                value["stop_requested"] = json!(run.cancel.load(Ordering::Acquire));
                value["running"] = json!(!run.done.load(Ordering::Acquire));
                value
            })
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
    let scope =
        json!({"schema":1,"task":task,"verify":verify,"route":context.club.route_identity()});
    runs_root(workspace)
        .join("learning")
        .join(crate::knowledge::cut::sha256_hex(
            scope.to_string().as_bytes(),
        ))
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

fn persist(dir: &Path, record: &Value) -> Result<(), String> {
    bridge::write(
        &dir.join("research.json"),
        &serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?,
    )
}

impl RlState {
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
                "actions":["options","suggest","run","status","results","stop"],
                "execution":"Optional isolated attempt on the current loop route. compare=true adds a baseline attempt from the same frozen source. No extra hop, time or thinking caps; explicit loop budgets and cancellation apply.",
                "learning":"Physical verifier receipts update original Sloptomizer UCB, Pareto and MicroLearner state. Exploratory advice, not an audited policy install or provider weight training.",
                "runtime":"Bundled algorithms; requires Python 3 standard library. suggest checks the runtime without a model call.",
                "related":["rl_campaign","consult_model(method=deli, club=self)","spawn(formation=moa)","continual_harness"]
            }).to_string()),
            "status" => Ok(self.research.status().to_string()),
            "stop" => Ok(json!({"stop_requested":self.research.stop(),"research":self.research.status()}).to_string()),
            "results" => {
                let id = args.get("run_id").map(|v| v.as_str().ok_or("run_id must be a string")).transpose()?;
                let limit = args.get("limit").map(|v| v.as_u64().filter(|n| *n > 0).ok_or("limit must be positive")).transpose()?.unwrap_or(5);
                Ok(json!({"runs":self.research_results(workspace, id, usize::try_from(limit).unwrap_or(usize::MAX))?}).to_string())
            }
            action @ ("suggest" | "run") => {
                let context = self.loop_context.clone().ok_or("loop_research needs an active /loop")?;
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
                                return Err("candidate requires idea".into());
                            }
                            text_arg(row,"approach","direct")?;
                        }
                    }
                    return Ok(bridge::transform(&scope, request, cancel)?.to_string());
                }
                let idea = text_arg(args,"idea", "")?.to_owned();
                if idea.trim().is_empty() { return Err("run requires the idea you want to try".into()); }
                let approach = text_arg(args,"approach", "direct")?.to_owned();
                let compare = flag(args,"compare",false)?;
                let use_memory = flag(args,"use_memory",true)?;
                if compare && verify.is_none() { return Err("compare requires a verifier to measure a difference".into()); }
                if self.research.active.as_ref().is_some_and(|run| !run.done.load(Ordering::Acquire)) {
                    return Err("a research run is already active; continue useful work or request stop and inspect status".into());
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
                    })).unwrap_or_else(|_| Err("research worker panicked; inspect retained artifacts".into()));
                    let mut record = worker_record.lock().unwrap_or_else(|e| e.into_inner());
                    record["timings_ms"]["worker_total"] = json!(worker_started.elapsed().as_millis());
                    record["status"] = json!(if worker_flag.load(Ordering::Acquire) { "stopped" } else if outcome.is_err() { "failed" } else if !record["learning_error"].is_null() { "completed_with_learning_error" } else { "completed" });
                    if let Err(error) = outcome { record["error"] = json!(error); }
                    if let Err(error) = persist(&dir,&record) { record["retention_error"] = json!(error); }
                    worker_done.store(true,Ordering::Release);
                }).map_err(|e| format!("could not start research worker; launch record retained: {e}"))?;
                self.research.active = Some(ResearchRun { cancel:worker_cancel,done,record:shared });
                Ok(json!({"run_id":id,"artifacts":launched,"status":"running","message":"Continue useful work. status/results expose evidence and learning; stop exits this research run without ending the main loop."}).to_string())
            }
            _ => Err("unknown loop_research action".into()),
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
            return Err("run_id must be an identifier returned by loop_research".into());
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
            return Err("no research run with that id in this workspace".into());
        }
        Ok(rows)
    }

    pub(super) fn research_context(&self, workspace: &Path) -> String {
        let mut text = "Optional loop_research: Sloptomizer suggest offers pareto, bandit and memory advice; run tries your chosen idea asynchronously on this route. compare=true measures a baseline and candidate from the same source. status/results/stop let you step out and back into ordinary work. Verifier receipts update exploratory memory; no forced research step or policy install. Submit a verified winner when ready.\n".to_owned();
        match self.research_results(workspace, None, 3) {
            Ok(rows) => {
                for row in rows {
                    let summary = json!({"research_run":row["run_id"],"status":row["status"],"artifacts":row["artifacts"],"measurements":row["measurements"],"timings_ms":row["timings_ms"],"learning_error":row["learning_error"],"error":row["error"],"retention_error":row["retention_error"]}).to_string();
                    // Full evidence and errors remain in results/artifacts.
                    text.push_str(&summary.chars().take(4000).collect::<String>());
                    text.push('\n');
                }
            }
            Err(e) => text.push_str(&format!("Research history unavailable: {e}\n")),
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
    let candidate_task = format!(
        "{task}\n\n[Selected experimental approach — task context]\n{}\n\n[Optional historical research snippets — evidence to assess]\n{}",
        launch["idea"].as_str().unwrap_or_default(),
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
                updates.push(bridge::transform(
                    scope,
                    json!({"action":"observe","task":task,"observation":observation}),
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
