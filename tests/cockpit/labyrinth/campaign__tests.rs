use super::*;
use crate::agent::club::{ChatRole, ToolCall};
use std::sync::atomic::AtomicU64;

const CORRECTED: &str = "Odd integers have odd squares.";
const ORIGINAL: &str = "All integers have odd squares.";

fn workspace() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "angel-labyrinth-campaign-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join("problem.json"),
        "{\"values\":[-5,-4,-3,-2,-1,0,1,2,3,4,5]}\n",
    )
    .unwrap();
    std::fs::write(path.join("notes.md"), "# Parity\n\nTODO\n").unwrap();
    std::fs::write(path.join("check.py"),"import json\nfrom pathlib import Path\nv=json.loads(Path('problem.json').read_text())['values']\nassert all((n*n)%2 == n%2 for n in v)\nassert Path('notes.md').read_text().startswith('# Parity')\nprint('checked exact input members')\n").unwrap();
    std::fs::write(path.join("spot.py"),format!("from pathlib import Path\nassert {CORRECTED:?} in Path('notes.md').read_text()\nprint('coordinator checked corrected claim')\n")).unwrap();
    path
}

fn spec() -> CampaignSpec {
    CampaignSpec {
        id: "parity".into(),
        task: "Check a parity claim and integrate only corrected measured evidence".into(),
        doors: vec![CampaignDoor {
            id: "odd".into(),
            statement: ORIGINAL.into(),
            missing: "Exact domain and independently computed small cases".into(),
            perspectives: vec![
                "Factor n²-n and inspect parity".into(),
                "Enumerate -5 through 5 including zero".into(),
                "Classify residues modulo two".into(),
                "Search the least counterexample to a universal claim".into(),
                "Read the supplied arithmetic definitions".into(),
            ],
        }],
        inputs: vec!["problem.json".into(), "check.py".into(), "spot.py".into()],
        referee_inputs: vec!["problem.json".into()],
        canonical_documents: vec!["notes.md".into()],
        checks: vec![CheckCommand {
            argv: vec!["python3".into(), "check.py".into()],
            timeout_secs: 10,
        }],
        spot_checks: vec![CheckCommand {
            argv: vec!["python3".into(), "spot.py".into()],
            timeout_secs: 10,
        }],
        closed_routes: vec![ClosedRoute {
            door_id: "odd".into(),
            route: "Positive odd examples alone".into(),
            lesson: "Examples do not justify all integers or the zero case".into(),
        }],
        side_projects: vec![],
        compute_rules: default_compute_rules(),
        concurrency: 2,
        max_hops: 12,
        role_timeout_secs: 30,
        campaign_timeout_secs: 120,
        max_rounds: 1,
        writer_retries: 0,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Correct,
    MissingCorrection,
    RefereeGap,
    RefereeReusedCode,
    RefereeNominalSpoof,
    RefereeProgramFails,
    RefereeTransientMutation,
    RefereeHostRead,
    RefereeSiblingRead,
    ExactCandidateInventory,
    RefereeChangingOutputs,
    RefereeTransientFailure,
    RefereeChangingCode,
    VerifiedRefutation,
    CheckedGap,
    SourceMutation,
    CanonicalConflict,
    RepairWriter,
}

struct ScriptedCampaignClub {
    scenario: Scenario,
    canonical: PathBuf,
    steps: Mutex<BTreeMap<String, usize>>,
    observations: Mutex<Vec<Value>>,
}
fn data(messages: &[ChatMsg]) -> Value {
    messages
        .iter()
        .flat_map(|m| m.content.lines())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|v| v.get("campaign_id").is_some())
        .expect("native role carries its structured data")
}
fn call(id: &str, name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: name.into(),
        args,
    }
}
fn write_call(id: &str, path: &str, text: &str) -> ToolCall {
    call(
        id,
        "campaign_file",
        json!({"action":"write","path":path,"text":text}),
    )
}
const AUTHOR_CODE: &str = "from pathlib import Path\nimport json\nv=[1,3,5]\nassert all(n*n%2==1 for n in v)\nPath('artifacts/author-output.json').write_text(json.dumps({'odd_samples':v}))\nprint('author odd samples only')\n";
const REFEREE_CODE: &str = "import json\nfrom pathlib import Path\nv=json.loads(Path('problem.json').read_text())['values']\nchecked=[(n,n*n%2) for n in v]\nassert all(parity==1 for n,parity in checked if n%2)\nassert any(parity==0 for n,parity in checked)\nPath('artifacts/referee-output.json').write_text(json.dumps({'members':checked,'counterexample':0}))\nprint('independent member-by-member exact check')\n";

impl Club for ScriptedCampaignClub {
    fn label(&self) -> &str {
        "scripted-campaign-oracle"
    }
    fn model_identity(&self) -> Option<String> {
        Some("fixture/independent-scripted-oracle".into())
    }
    fn route_identity(&self) -> RouteIdentity {
        RouteIdentity {
            driver: self.label().into(),
            model: self.model_identity(),
            reasoning_effort: None,
        }
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        Err("campaign roles must execute tool-enabled run_turn".into())
    }
    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        let payload = data(messages);
        let id = payload["role_id"].as_str().unwrap();
        let role = payload["role"].as_str().unwrap();
        for system in messages.iter().filter(|m| m.role == ChatRole::System) {
            assert!(
                system
                    .content
                    .chars()
                    .all(|c| c == '\n' || ('\u{2800}'..='\u{28ff}').contains(&c)),
                "role policy must be native Legend cells"
            );
        }
        assert!(tools.iter().any(|d| d.name == "read_file"));
        assert!(tools.iter().any(|d| d.name == "campaign_file"));
        assert!(!tools.iter().any(|d| matches!(
            d.name.as_str(),
            "write_file" | "shell" | "spawn" | "yukon_submit"
        )));
        if role == "referee" {
            assert!(
                !messages.iter().any(|m| m.role == ChatRole::Assistant
                    && m.content.contains("author private reasoning"))
            );
            if self.scenario == Scenario::ExactCandidateInventory {
                assert_eq!(
                    payload["inputs"],
                    json!([
                        "problem.json",
                        "submission/nested/candidate.lean",
                        "submission/nested/layout.json"
                    ])
                );
            } else {
                assert_eq!(payload["inputs"], json!(["problem.json"]));
            }
        }
        self.observations.lock().unwrap().push(json!({"id":id,"role":role,"has_literature":messages.iter().any(|m|m.content.contains("lead-parity")),"has_native_decoder":messages.iter().any(|m|m.role==ChatRole::Tool&&m.content.contains("campaign"))}));
        let step = {
            let mut steps = self.steps.lock().unwrap();
            let step = steps.entry(id.into()).or_default();
            let old = *step;
            *step += 1;
            old
        };
        if step == 0 {
            let route = match role {
                "literature" => chapter::LITERATURE,
                "attack" => chapter::ATTACK,
                "referee" => chapter::REFEREE,
                "writer" => chapter::WRITER,
                _ => panic!("unexpected role"),
            };
            return Ok(ClubReply::Calls(vec![call(
                "legend",
                "read_file",
                json!({"path":format!("ledger://{}",route.cells())}),
            )]));
        }
        if step == 1 {
            let brief = payload["workflow_brief_paths"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()
                .as_str()
                .unwrap();
            return Ok(ClubReply::Calls(vec![call(
                "brief",
                "campaign_file",
                json!({"action":"read","path":brief}),
            )]));
        }
        match role {
            "literature" => {
                let lead = json!({"id":"lead-parity","doors":["odd"],"statement":"Separate the even and odd residue classes before testing universality","source":"problem.json (supplied exact integer cases)"});
                if step == 2 {
                    return Ok(ClubReply::Calls(vec![call("lead", "campaign_lead", lead)]));
                }
                Ok(ClubReply::Text(json!({"leads":[lead]}).to_string()))
            }
            "attack" => {
                if step == 2 {
                    return Ok(ClubReply::Calls(vec![write_call(
                        "author-code",
                        "artifacts/author.py",
                        AUTHOR_CODE,
                    )]));
                }
                if step == 3 {
                    return Ok(ClubReply::Calls(vec![call(
                        "author-run",
                        "campaign_exec",
                        json!({"argv":["python3","artifacts/author.py"]}),
                    )]));
                }
                Ok(ClubReply::Text(json!({"claims":[{"id":"odd-claim","statement":ORIGINAL,"status":"EVIDENCE","test":"Include zero and all integer residue classes","code":"artifacts/author.py","evidence":["artifacts/author-output.json"]}],"closed_routes":[]}).to_string()))
            }
            "referee" => {
                let code = if self.scenario == Scenario::RefereeReusedCode {
                    AUTHOR_CODE.to_string()
                } else if matches!(
                    self.scenario,
                    Scenario::RefereeNominalSpoof | Scenario::RefereeProgramFails
                ) {
                    "raise SystemExit(37)\n".to_string()
                } else if self.scenario == Scenario::RefereeTransientMutation {
                    "from pathlib import Path\np=Path('problem.json')\noriginal=p.read_bytes()\np.write_bytes(b'{}')\nassert p.read_bytes()==b'{}'\np.write_bytes(original)\nprint('fabricated transient input passed')\n".to_string()
                } else if self.scenario == Scenario::RefereeHostRead {
                    format!(
                        "from pathlib import Path\nprint(Path({:?}).read_text())\n",
                        self.canonical.join("notes.md").to_string_lossy()
                    )
                } else if self.scenario == Scenario::RefereeSiblingRead {
                    format!("from pathlib import Path\nprint(Path({:?}).read_text())\n",self.canonical.join("labyrinth/angel/campaigns/parity/workspaces/attack-odd-r0-a0/artifacts/author.py").to_string_lossy())
                } else if self.scenario == Scenario::ExactCandidateInventory {
                    format!("{REFEREE_CODE}{EXACT_CANDIDATE_INVENTORY_CHECK}")
                } else if self.scenario == Scenario::RefereeChangingOutputs {
                    format!(
                        "{REFEREE_CODE}p=Path('artifacts/replay-count.json')\nn=json.loads(p.read_text()) if p.exists() else 0\np.write_text(json.dumps(n+1))\n"
                    )
                } else if self.scenario == Scenario::RefereeTransientFailure {
                    format!(
                        "{REFEREE_CODE}p=Path('artifacts/replay-count.json')\nn=json.loads(p.read_text()) if p.exists() else 0\np.write_text(json.dumps(n+1))\nassert n > 0, 'first replay failed transiently'\n"
                    )
                } else if self.scenario == Scenario::RefereeChangingCode {
                    format!(
                        "{REFEREE_CODE}Path(__file__).write_text('# changed while executing\\n')\n"
                    )
                } else {
                    REFEREE_CODE.to_string()
                };
                if step == 2 {
                    return Ok(ClubReply::Calls(vec![write_call(
                        "own-code",
                        "artifacts/independent.py",
                        &code,
                    )]));
                }
                if step == 3 && self.scenario == Scenario::RefereeChangingOutputs {
                    return Ok(ClubReply::Calls(vec![call(
                        "referee-own-run",
                        "campaign_exec",
                        json!({"argv":["python3","-I","-S","-B","artifacts/independent.py"]}),
                    )]));
                }
                let (verdict, corrections) = if matches!(
                    self.scenario,
                    Scenario::RefereeGap | Scenario::CheckedGap
                ) {
                    ("GAP", json!([]))
                } else if self.scenario == Scenario::VerifiedRefutation {
                    ("FALSE", json!([]))
                } else {
                    (
                        "ESTABLISHED WITH CORRECTIONS",
                        json!([{"id":"domain","claim_id":"odd-claim","old":"All integers","new":"Odd integers"}]),
                    )
                };
                let argv = if self.scenario == Scenario::RefereeNominalSpoof {
                    json!(["true", "artifacts/independent.py"])
                } else {
                    json!(["python3", "-I", "-S", "-B", "artifacts/independent.py"])
                };
                Ok(ClubReply::Text(json!({"verdicts":[{"claim_id":"odd-claim","verdict":verdict,"reason":"Zero has an even square; the evidence supports only the odd domain","lesson":"Finite odd examples cannot establish the unrestricted integer domain","counterexample":"artifacts/referee-output.json","test":"Enumerate zero and both residue classes before asserting universality","corrections":corrections}],"independent_check":{"code":"artifacts/independent.py","argv":argv}}).to_string()))
            }
            "writer" => {
                if step == 2 {
                    return Ok(ClubReply::Calls(vec![
                        call(
                            "author-material",
                            "campaign_file",
                            json!({"action":"read","path":payload["review_materials"]["author"]["report_path"]}),
                        ),
                        call(
                            "referee-material",
                            "campaign_file",
                            json!({"action":"read","path":"writer-review/referee/artifacts/independent.py"}),
                        ),
                    ]));
                }
                assert!(
                    messages.iter().any(|message| message.role == ChatRole::Tool
                        && message.content.contains(ORIGINAL)),
                    "writer receives verbatim original author material inside its sealed workspace"
                );
                assert!(
                    messages.iter().any(|message| message.role == ChatRole::Tool
                        && message.content.contains("member-by-member")),
                    "writer receives checked referee own-code material inside its sealed workspace"
                );
                if matches!(
                    self.scenario,
                    Scenario::VerifiedRefutation | Scenario::CheckedGap
                ) {
                    let claim = payload["claims"][0].clone();
                    let node = if self.scenario == Scenario::VerifiedRefutation {
                        json!({"id":"ev.parity-refutation","kind":"deadend","status":"refuted","claim_id":"odd-claim","title":"The unrestricted parity route has a counterexample","statement":claim["statement"],"lesson":claim["lesson"]})
                    } else {
                        json!({"id":"cj.parity-gap","kind":"conjecture","tier":"T5","status":"open","claim_id":"odd-claim","title":"Parity claim remains unestablished","statement":claim["statement"],"test":claim["test"]})
                    };
                    return Ok(ClubReply::Text(json!({"claims":payload["claims"],"applied_corrections":[],"edits":[{"path":"notes.md","old":"TODO","new":format!("{}: {ORIGINAL}\nThe independently reviewed attempted route remains recorded.",if self.scenario==Scenario::VerifiedRefutation{"REFUTED"}else{"CONJECTURE"})}],"nodes":[node],"sota":[],"frontier":null}).to_string()));
                }
                if self.scenario == Scenario::CanonicalConflict {
                    std::fs::write(
                        self.canonical.join("notes.md"),
                        "# Parity\n\nA concurrent user edit.\n",
                    )
                    .unwrap();
                }
                let omit = self.scenario == Scenario::MissingCorrection
                    || (self.scenario == Scenario::RepairWriter && id.ends_with("-a0"));
                let claims = payload["claims"].clone();
                Ok(ClubReply::Text(json!({"claims":claims,"applied_corrections":if omit{json!([])}else{json!(["domain"])},
                    "edits":[{"path":"notes.md","old":"TODO","new":format!("Evidence (T4, independently checked): {CORRECTED}\nEven integers, including zero, invalidate the original unrestricted statement.")}],
                    "nodes":[],"sota":[],"frontier":null}).to_string()))
            }
            _ => panic!("unknown role"),
        }
    }
}

#[derive(Default)]
struct CheckingIntegrator {
    bundles: Mutex<Vec<IntegrationBundle>>,
}
impl CampaignIntegrator for CheckingIntegrator {
    fn integrate(
        &self,
        root: &Path,
        bundle: &IntegrationBundle,
        cancel: &AtomicBool,
    ) -> Result<Value, String> {
        assert!(!cancel.load(Ordering::Acquire));
        let ids = [
            &bundle.literature.id,
            &bundle.author.id,
            &bundle.referee.id,
            &bundle.writer_before.id,
            &bundle.writer.id,
            &bundle.coordinator.id,
        ];
        assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), 6);
        assert_eq!(bundle.claims[0]["statement"], CORRECTED);
        assert_eq!(bundle.claims[0]["status"], "EVIDENCE");
        assert_eq!(bundle.applied_corrections, ["domain"]);
        assert_eq!(
            bundle
                .checks
                .iter()
                .filter(|r| r.stage == "writer-before")
                .count(),
            1
        );
        assert_eq!(
            bundle.checks.iter().filter(|r| r.stage == "writer").count(),
            2
        );
        assert_eq!(
            bundle
                .checks
                .iter()
                .filter(|r| r.stage == "referee")
                .count(),
            1
        );
        assert_eq!(
            bundle
                .checks
                .iter()
                .filter(|r| r.stage == "coordinator")
                .count(),
            1
        );
        for check in &bundle.checks {
            assert_eq!(check.exit_code, Some(0));
            assert!(check.fresh && !check.timed_out && !check.cancelled);
            assert_eq!(check.source_sha256, check.source_after_sha256);
            assert_eq!(
                sha256_hex(&read(root, Path::new(&check.output_path), FILE_BYTES)?),
                check.output_sha256
            );
        }
        let author_code = bundle.claims[0]["code"]["sha256"].as_str().unwrap();
        let referee_code = bundle
            .checks
            .iter()
            .find(|r| r.stage == "referee")
            .unwrap()
            .code_sha256
            .as_deref()
            .unwrap();
        assert_ne!(author_code, referee_code);
        for edit in &bundle.edits {
            let text = String::from_utf8(read(root, Path::new(&edit.path), FILE_BYTES)?).unwrap();
            assert_eq!(sha256_hex(text.as_bytes()), edit.before_sha256);
            assert_eq!(text.matches(&edit.old).count(), 1);
            let new = text.replacen(&edit.old, &edit.new, 1);
            assert_eq!(sha256_hex(new.as_bytes()), edit.after_sha256);
            confined_write(root, Path::new(&edit.path), new.as_bytes())?;
        }
        self.bundles.lock().unwrap().push(bundle.clone());
        Ok(json!({"id":bundle.id,"map_changed":true,"integrated":true}))
    }
}

fn runtime(
    root: &Path,
    scenario: Scenario,
) -> (
    CampaignRuntime,
    Arc<ScriptedCampaignClub>,
    Arc<CheckingIntegrator>,
) {
    let club = Arc::new(ScriptedCampaignClub {
        scenario,
        canonical: root.into(),
        steps: Mutex::new(BTreeMap::new()),
        observations: Mutex::new(Vec::new()),
    });
    let integrator = Arc::new(CheckingIntegrator::default());
    (
        CampaignRuntime {
            literature: club.clone(),
            attack: club.clone(),
            referee: club.clone(),
            writer: club.clone(),
            integrator: integrator.clone(),
        },
        club,
        integrator,
    )
}

#[test]
fn omitted_hop_limit_completes_a_multi_case_campaign_and_explicit_limit_still_stops_it() {
    let _guard = crate::tests::env_lock();
    struct MultiCaseClub {
        inner: Arc<ScriptedCampaignClub>,
        case: AtomicUsize,
    }
    impl Club for MultiCaseClub {
        fn label(&self) -> &str {
            self.inner.label()
        }
        fn model_identity(&self) -> Option<String> {
            self.inner.model_identity()
        }
        fn route_identity(&self) -> RouteIdentity {
            self.inner.route_identity()
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("multi-case fixture requires its native tool channel".into())
        }
        fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
            let payload = data(messages);
            let role_id = payload["role_id"].as_str().unwrap();
            let stage = self
                .inner
                .steps
                .lock()
                .unwrap()
                .get(role_id)
                .copied()
                .unwrap_or(0);
            if payload["role"] == "attack" && stage >= 2 {
                let case = self.case.fetch_add(1, Ordering::AcqRel);
                if case < 28 {
                    return Ok(ClubReply::Calls(vec![call(
                        &format!("case-{case}"),
                        "campaign_file",
                        json!({"action":"read","path":format!("cases/{case}.json")}),
                    )]));
                }
            }
            self.inner.chat(messages, tools)
        }
    }
    for explicit_limit in [None, Some(24)] {
        let root = workspace();
        std::fs::create_dir(root.join("cases")).unwrap();
        let mut encoded = serde_json::to_value(spec()).unwrap();
        encoded.as_object_mut().unwrap().remove("max_hops");
        if let Some(limit) = explicit_limit {
            encoded["max_hops"] = json!(limit);
        }
        let mut configured: CampaignSpec = serde_json::from_value(encoded).unwrap();
        for case in 0..28 {
            let path = format!("cases/{case}.json");
            std::fs::write(root.join(&path), format!("{{\"case\":{case}}}\n")).unwrap();
            configured.inputs.push(path);
        }
        start(&root, configured).unwrap();
        let (mut runtime, inner, integrator) = runtime(&root, Scenario::Correct);
        runtime.attack = Arc::new(MultiCaseClub {
            inner,
            case: AtomicUsize::new(0),
        });
        let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
        if explicit_limit.is_none() {
            assert_eq!(result["status"], "completed", "{result}");
            assert_eq!(integrator.bundles.lock().unwrap().len(), 1);
            assert!(
                std::fs::read_to_string(root.join("notes.md"))
                    .unwrap()
                    .contains(CORRECTED)
            );
        } else {
            assert_eq!(result["status"], "budget_exhausted", "{result}");
            assert!(integrator.bundles.lock().unwrap().is_empty());
            assert!(
                std::fs::read_to_string(root.join("notes.md"))
                    .unwrap()
                    .contains("TODO")
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn native_campaign_runs_real_roles_own_code_and_checked_correction_integration() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    let configured = spec();
    let initial = start(&root, configured.clone()).unwrap();
    assert_eq!(initial["status"], "ready");
    let (runtime, club, integrator) = runtime(&root, Scenario::Correct);
    let result = run(&root, "parity", runtime.clone(), &AtomicBool::new(false)).unwrap();
    assert_eq!(result["status"], "completed", "{result:#}");
    assert!(
        std::fs::read_to_string(root.join("notes.md"))
            .unwrap()
            .contains(CORRECTED)
    );
    let bundles = integrator.bundles.lock().unwrap();
    assert_eq!(bundles.len(), 1);
    let bundle = &bundles[0];
    let attack_original = read(&root, Path::new(&bundle.author.report_path), REPORT_BYTES).unwrap();
    assert!(
        String::from_utf8(attack_original.clone())
            .unwrap()
            .contains(ORIGINAL)
    );
    assert_eq!(sha256_hex(&attack_original), bundle.author.report_sha256);
    assert_eq!(
        bundles[0].nodes.len(),
        0,
        "review agreement must not manufacture proof nodes"
    );
    drop(bundles);
    let observations = club.observations.lock().unwrap();
    assert!(
        observations
            .iter()
            .any(|o| o["role"] == "attack" && o["has_literature"] == true),
        "live literature must reach running attacks"
    );
    assert!(observations.iter().any(|o| o["has_native_decoder"] == true));
    drop(observations);
    let calls = club.steps.lock().unwrap().clone();
    assert_eq!(
        run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap()["status"],
        "completed"
    );
    assert_eq!(
        *club.steps.lock().unwrap(),
        calls,
        "completed stages are not replayed after restart"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn retained_referee_recheck_executes_fresh_code_and_preserves_reports_and_completed_models() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    start(&root, spec()).unwrap();
    let (runtime, club, integrator) = runtime(&root, Scenario::RefereeTransientFailure);
    let failed = run(&root, "parity", runtime.clone(), &AtomicBool::new(false)).unwrap();
    assert_eq!(failed["status"], "budget_exhausted");
    assert_eq!(failed["doors"][0]["referee"]["state"], "failed");
    let original_steps = club.steps.lock().unwrap().clone();
    let archive = role_archive("parity", "referee-odd-r0-a0");
    let report = read(&root, &archive.join("report.md"), REPORT_BYTES).unwrap();
    let first = read(&root, &archive.join("checks/referee-0.json"), REPORT_BYTES).unwrap();
    assert_ne!(
        serde_json::from_slice::<Value>(&first).unwrap()["exit_code"],
        0
    );
    let ready = recheck(&root, "parity").unwrap();
    assert_eq!(ready["status"], "ready");
    assert_eq!(ready["doors"][0]["referee_rechecks"], 1);
    assert_eq!(*club.steps.lock().unwrap(), original_steps);
    let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(result["status"], "completed", "{result:#}");
    assert_eq!(
        read(&root, &archive.join("report.md"), REPORT_BYTES).unwrap(),
        report
    );
    assert_eq!(
        read(&root, &archive.join("checks/referee-0.json"), REPORT_BYTES).unwrap(),
        first
    );
    let next: Value = serde_json::from_slice(
        &read(&root, &archive.join("checks/referee-1.json"), REPORT_BYTES).unwrap(),
    )
    .unwrap();
    assert_eq!(next["exit_code"], 0);
    assert_eq!(next["fresh"], true);
    for (id, steps) in original_steps {
        assert_eq!(club.steps.lock().unwrap()[&id], steps);
    }
    assert_eq!(integrator.bundles.lock().unwrap().len(), 1);
    assert!(recheck(&root, "parity").is_err());
    assert!(
        !root
            .join(archive.join("source").join(SOURCE_INDEX))
            .exists(),
        "recheck must not write the immutable source archive"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn referee_rechecks_are_bounded_and_reject_changed_immutable_inputs() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    start(&root, spec()).unwrap();
    let (route, _, _) = runtime(&root, Scenario::RefereeProgramFails);
    for attempt in 0..=3 {
        let failed = run(&root, "parity", route.clone(), &AtomicBool::new(false)).unwrap();
        assert_eq!(failed["status"], "budget_exhausted");
        if attempt < 3 {
            recheck(&root, "parity").unwrap();
        }
    }
    assert!(recheck(&root, "parity").unwrap_err().contains("eligible"));
    std::fs::remove_dir_all(root).unwrap();
    let root = workspace();
    start(&root, spec()).unwrap();
    let (runtime, _, _) = runtime(&root, Scenario::RefereeProgramFails);
    run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    std::fs::write(
        root.join(role_archive("parity", "referee-odd-r0-a0"))
            .join("source/problem.json"),
        "{}",
    )
    .unwrap();
    assert!(
        recheck(&root, "parity")
            .unwrap_err()
            .contains("source changed")
    );
    assert_eq!(
        load_state(&root, "parity").unwrap().1.status,
        "budget_exhausted"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn completed_writer_integration_rechecks_use_frozen_inputs_and_preserve_all_model_results() {
    let _guard = crate::tests::env_lock();
    struct FailFirstPreview {
        first: AtomicBool,
        inner: Arc<CheckingIntegrator>,
    }
    impl CampaignIntegrator for FailFirstPreview {
        fn preview(&self, _: &Path, _: &IntegrationBundle) -> Result<Value, String> {
            if self.first.swap(false, Ordering::AcqRel) {
                Err("operational preview failure".into())
            } else {
                Ok(json!({"files":[]}))
            }
        }
        fn integrate(
            &self,
            root: &Path,
            bundle: &IntegrationBundle,
            cancel: &AtomicBool,
        ) -> Result<Value, String> {
            self.inner.integrate(root, bundle, cancel)
        }
    }
    let root = workspace();
    start(&root, spec()).unwrap();
    let (mut route, club, integrator) = runtime(&root, Scenario::Correct);
    route.integrator = Arc::new(FailFirstPreview {
        first: AtomicBool::new(true),
        inner: integrator.clone(),
    });
    let failed = run(&root, "parity", route.clone(), &AtomicBool::new(false)).unwrap();
    assert_eq!(failed["status"], "budget_exhausted");
    assert_eq!(failed["doors"][0]["writer"]["state"], "finished");
    let calls = club.steps.lock().unwrap().clone();
    let raw = role_work("parity", "writer-odd-r0-a0");
    assert!(
        std::fs::read_to_string(root.join(&raw).join("notes.md"))
            .unwrap()
            .contains("TODO")
    );
    // Earlier coordinators edited this private working copy before preview;
    // replay must use its frozen archive and leave the historical work alone.
    std::fs::write(
        root.join(&raw).join("notes.md"),
        "legacy coordinator derived copy\n",
    )
    .unwrap();
    let ready = recheck(&root, "parity").unwrap();
    assert_eq!(ready["doors"][0]["integration_rechecks"], 1);
    let completed = run(&root, "parity", route, &AtomicBool::new(false)).unwrap();
    assert_eq!(completed["status"], "completed", "{completed:#}");
    assert_eq!(*club.steps.lock().unwrap(), calls);
    assert_eq!(
        std::fs::read_to_string(root.join(&raw).join("notes.md")).unwrap(),
        "legacy coordinator derived copy\n"
    );
    let bundles = integrator.bundles.lock().unwrap();
    assert_eq!(bundles.len(), 1);
    assert!(bundles[0].id.ends_with("-i1"));
    assert!(bundles[0].writer.id.ends_with("-checked-i1"));
    assert!(
        root.join(role_archive("parity", "writer-odd-r0-a0"))
            .join("checks/writer-before-0.json")
            .exists()
    );
    assert!(
        root.join(role_archive("parity", "writer-odd-r0-a0"))
            .join("checks/writer-before-1.json")
            .exists()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn exact_corrections_target_bounded_text_fields_and_reject_ambiguous_or_structural_edits() {
    let mut claim = json!({"statement":"A measured claim", "test":"Original test", "lesson":"Original lesson", "status":"EVIDENCE"});
    for fix in [
        json!({"id":"test","claim_id":"c","field":"test","old":"Original test","new":"Checked test"}),
        json!({"id":"lesson","claim_id":"c","old":"Original lesson","new":"Checked lesson"}),
    ] {
        apply_claim_correction(&mut claim, &serde_json::from_value(fix).unwrap()).unwrap();
    }
    assert_eq!(claim["test"], "Checked test");
    assert_eq!(claim["lesson"], "Checked lesson");
    let ambiguous: Correction = serde_json::from_value(
        json!({"id":"ambiguous","claim_id":"c","old":"Checked","new":"Fresh"}),
    )
    .unwrap();
    let before = claim.clone();
    assert!(apply_claim_correction(&mut claim, &ambiguous).is_err());
    assert_eq!(claim, before);
    for field in ["status", "code", "proof", "evidence"] {
        assert!(serde_json::from_value::<Correction>(json!({"id":"structural","claim_id":"c","field":field,"old":"EVIDENCE","new":"PROVED"})).is_err());
    }
    let oversized: Correction = serde_json::from_value(
        json!({"id":"large","claim_id":"c","field":"statement","old":"A","new":"x".repeat(16_000)}),
    )
    .unwrap();
    assert!(apply_claim_correction(&mut claim, &oversized).is_err());
    assert_eq!(claim, before);
}

#[test]
fn native_campaign_integrates_with_the_real_canonical_artifact_owner() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    start(&root, spec()).unwrap();
    let (mut runtime, club, _) = runtime(&root, Scenario::Correct);
    runtime.integrator = Arc::new(crate::drive::labyrinth::workflow::ArtifactIntegrator);
    let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(result["status"], "completed", "{result:#}");
    assert!(
        std::fs::read_to_string(root.join("notes.md"))
            .unwrap()
            .contains(CORRECTED)
    );
    assert_eq!(result["integrations"].as_array().unwrap().len(), 1);
    let map = super::super::load(&root).unwrap().unwrap();
    assert!(
        !map.nodes
            .iter()
            .any(|node| matches!(node["tier"].as_str(), Some("T1" | "T3"))),
        "a successful native campaign cannot award proof/certification tiers"
    );
    assert!(
        club.observations
            .lock()
            .unwrap()
            .iter()
            .any(|o| o["role"] == "referee")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unsafe_claims_missing_corrections_and_reused_referee_code_never_integrate() {
    let _guard = crate::tests::env_lock();
    for scenario in [
        Scenario::MissingCorrection,
        Scenario::RefereeGap,
        Scenario::RefereeReusedCode,
    ] {
        let root = workspace();
        start(&root, spec()).unwrap();
        let (runtime, _, integrator) = runtime(&root, scenario);
        let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
        assert_eq!(result["status"], "budget_exhausted", "{result:#}");
        assert_eq!(
            std::fs::read_to_string(root.join("notes.md")).unwrap(),
            "# Parity\n\nTODO\n"
        );
        assert!(integrator.bundles.lock().unwrap().is_empty());
        assert!(!result["doors"][0]["error"].is_null());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn failed_writer_can_correct_its_draft_in_a_new_owned_workspace() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    let mut configured = spec();
    configured.writer_retries = 1;
    start(&root, configured).unwrap();
    let (runtime, club, integrator) = runtime(&root, Scenario::RepairWriter);
    let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(result["status"], "completed", "{result:#}");
    assert_eq!(integrator.bundles.lock().unwrap().len(), 1);
    let steps = club.steps.lock().unwrap();
    assert!(steps.contains_key("writer-odd-r0-a0"));
    assert!(steps.contains_key("writer-odd-r0-a1"));
    assert!(
        root.join("labyrinth/angel/campaigns/parity/agents/writer-odd-r0-a0/report.md")
            .exists()
    );
    assert!(
        root.join("labyrinth/angel/campaigns/parity/agents/writer-odd-r0-a1/report.md")
            .exists()
    );
    drop(steps);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_mutating_checks_and_concurrent_canonical_edits_block_integration() {
    let _guard = crate::tests::env_lock();
    for scenario in [Scenario::SourceMutation, Scenario::CanonicalConflict] {
        let root = workspace();
        if scenario == Scenario::SourceMutation {
            std::fs::write(root.join("check.py"),"from pathlib import Path\nPath('problem.json').write_text('{}')\nprint('false green checker mutated its source')\n").unwrap();
        }
        start(&root, spec()).unwrap();
        let (runtime, _, integrator) = runtime(&root, scenario);
        let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
        assert_eq!(result["status"], "budget_exhausted", "{result:#}");
        assert!(integrator.bundles.lock().unwrap().is_empty());
        assert!(
            !std::fs::read_to_string(root.join("notes.md"))
                .unwrap()
                .contains(CORRECTED)
        );
        assert!(
            std::fs::read_to_string(root.join("problem.json"))
                .unwrap()
                .contains("values"),
            "checks mutate only their owned copy"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn stalled_campaign_executes_a_distinct_native_escalation_and_retains_closed_routes() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    let mut configured = spec();
    configured.max_rounds = 2;
    start(&root, configured).unwrap();
    let (runtime, club, integrator) = runtime(&root, Scenario::RefereeProgramFails);
    let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(result["status"], "budget_exhausted", "{result:#}");
    assert_eq!(result["round"], 1);
    assert_eq!(result["escalations"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["escalations"][0],
        book::d3_roles::pages(chapter::ESCALATE, [1])
    );
    assert!(result["closed_routes"].as_array().unwrap().len() >= 3);
    assert!(integrator.bundles.lock().unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("notes.md")).unwrap(),
        "# Parity\n\nTODO\n"
    );
    for round in 0..2 {
        let route = format!("round {round} rejected attack/referee");
        assert!(
            result["closed_routes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|closed| closed["route"] == route
                    && closed["lesson"]
                        .as_str()
                        .is_some_and(|lesson| lesson.contains("Some(37)")))
        );
        let receipt: CheckReceipt = serde_json::from_slice(
            &read(
                &root,
                &role_archive("parity", &format!("referee-odd-r{round}-a0"))
                    .join("checks/referee-0.json"),
                REPORT_BYTES,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(receipt.exit_code, Some(37));
        assert!(receipt.fresh && !receipt.timed_out && !receipt.cancelled);
    }
    let plan: Value = serde_json::from_slice(
        &read(
            &root,
            &role_archive("parity", "attack-odd-r1-a0").join("plan.json"),
            REPORT_BYTES,
        )
        .unwrap(),
    )
    .unwrap();
    let resumed_data: Value = plan["brief"]
        .as_str()
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|value| value.get("campaign_id").is_some())
        .unwrap();
    assert_eq!(resumed_data["round"], 1);
    assert_eq!(resumed_data["escalation_routes"], result["escalations"]);
    let steps = club.steps.lock().unwrap();
    assert!(steps.contains_key("attack-odd-r0-a0"));
    assert!(steps.contains_key("attack-odd-r1-a0"));
    assert!(!steps.keys().any(|id| id.starts_with("writer-")));
    drop(steps);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancelled_campaign_admits_no_model_calls_and_restarts_with_durable_identity() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    start(&root, spec()).unwrap();
    let (runtime, club, _) = runtime(&root, Scenario::Correct);
    let stopped = run(&root, "parity", runtime.clone(), &AtomicBool::new(true)).unwrap();
    assert_eq!(stopped["status"], "cancelled");
    assert!(club.steps.lock().unwrap().is_empty());
    let resumed = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(resumed["status"], "completed", "{resumed:#}");
    assert_ne!(stopped["run_id"], resumed["run_id"]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn campaign_rejects_path_aliases_missing_perspectives_and_unknown_spec_fields() {
    let root = workspace();
    let mut configured = spec();
    configured.doors[0].perspectives.pop();
    assert!(start(&root, configured).is_err());
    let mut configured = spec();
    configured.inputs.push("../notes.md".into());
    assert!(start(&root, configured).is_err());
    let mut encoded = serde_json::to_value(spec()).unwrap();
    encoded["submit"] = json!(true);
    assert!(serde_json::from_value::<CampaignSpec>(encoded).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("problem.json"), root.join("alias.json")).unwrap();
        let mut configured = spec();
        configured.inputs.push("alias.json".into());
        assert!(start(&root, configured).is_err());
    }
    assert!(
        !root
            .join("labyrinth/angel/campaigns/parity/spec.json")
            .exists()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovery_accepts_only_complete_hash_bound_reports_and_replaces_interrupted_roles() {
    let root = workspace();
    let configured = spec();
    start(&root, configured.clone()).unwrap();
    let mut unfinished = role("parity", "odd", "attack", 0, 0);
    unfinished.state = "running".into();
    recover_role(&root, &configured, &mut unfinished).unwrap();
    assert_eq!(unfinished.id, "attack-odd-r0-a1");
    assert_eq!(unfinished.state, "pending");
    let mut role = role("parity", "odd", "attack", 0, 2);
    let work = prepare_role(&root, &configured, &role).unwrap();
    let report = "{\"claims\":[{\"id\":\"lead\",\"statement\":\"A testable question\",\"status\":\"CONJECTURE\",\"test\":\"Enumerate the exact input\"}]}";
    let output = archive_role(
        &root,
        &configured,
        &role,
        &work,
        report,
        json!({"stop_reason":"answer"}),
    )
    .unwrap();
    role.state = "running".into();
    recover_role(&root, &configured, &mut role).unwrap();
    assert_eq!(role.state, "finished");
    assert_eq!(
        role.outcome.unwrap().evidence.report_sha256,
        output.evidence.report_sha256
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn referee_checks_execute_actual_owned_code_and_cannot_read_or_mutate_peer_inputs() {
    let _guard = crate::tests::env_lock();
    for scenario in [
        Scenario::RefereeNominalSpoof,
        Scenario::RefereeProgramFails,
        Scenario::RefereeTransientMutation,
        Scenario::RefereeHostRead,
        Scenario::RefereeSiblingRead,
    ] {
        let root = workspace();
        start(&root, spec()).unwrap();
        let (runtime, _, integrator) = runtime(&root, scenario);
        let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
        assert_eq!(result["status"], "budget_exhausted", "{result:#}");
        assert!(integrator.bundles.lock().unwrap().is_empty());
        assert_eq!(
            std::fs::read_to_string(root.join("notes.md")).unwrap(),
            "# Parity\n\nTODO\n"
        );
        let receipt = root.join(
            "labyrinth/angel/campaigns/parity/agents/referee-odd-r0-a0/checks/referee-0.json",
        );
        if scenario == Scenario::RefereeNominalSpoof {
            assert!(
                !receipt.exists(),
                "a nominal true argument match is rejected before execution"
            );
        } else {
            let receipt: CheckReceipt =
                serde_json::from_slice(&std::fs::read(receipt).unwrap()).unwrap();
            assert_ne!(receipt.exit_code, Some(0));
            if scenario == Scenario::RefereeProgramFails {
                assert_eq!(receipt.exit_code, Some(37));
            }
            if scenario == Scenario::RefereeTransientMutation {
                assert_eq!(
                    receipt.source_sha256, receipt.source_after_sha256,
                    "source was immutable throughout execution, not repaired after a fabricated test"
                );
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn fresh_replay_retains_changed_outputs_without_overwriting_the_model_archive() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    start(&root, spec()).unwrap();
    let (runtime, _, integrator) = runtime(&root, Scenario::RefereeChangingOutputs);
    let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(result["status"], "completed", "{result:#}");
    let archive = "labyrinth/angel/campaigns/parity/agents/referee-odd-r0-a0";
    assert_eq!(
        std::fs::read_to_string(root.join(archive).join("artifacts/replay-count.json")).unwrap(),
        "1"
    );
    let bundles = integrator.bundles.lock().unwrap();
    let receipt = bundles[0]
        .checks
        .iter()
        .find(|r| r.stage == "referee")
        .unwrap();
    let replay = receipt
        .output_artifacts
        .iter()
        .find(|a| {
            a.path
                .ends_with("checks/referee-0/artifacts/replay-count.json")
        })
        .unwrap();
    let bytes = std::fs::read(root.join(&replay.path)).unwrap();
    assert_eq!(bytes, b"2");
    assert_eq!(sha256_hex(&bytes), replay.sha256);
    assert_eq!(receipt.code_sha256, receipt.code_after_sha256);
    assert!(
        bundles[0]
            .referee
            .artifacts
            .iter()
            .any(|a| a.path == replay.path)
    );
    drop(bundles);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn independent_code_changes_during_replay_remain_failed_and_retain_both_versions() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    start(&root, spec()).unwrap();
    let (runtime, _, integrator) = runtime(&root, Scenario::RefereeChangingCode);
    let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(result["status"], "budget_exhausted", "{result:#}");
    assert!(integrator.bundles.lock().unwrap().is_empty());
    let receipt: CheckReceipt = serde_json::from_slice(
        &std::fs::read(root.join(
            "labyrinth/angel/campaigns/parity/agents/referee-odd-r0-a0/checks/referee-0.json",
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(receipt.exit_code, Some(0));
    assert_ne!(receipt.code_sha256, receipt.code_after_sha256);
    assert!(
        std::fs::read_to_string(root.join(receipt.code_path.as_ref().unwrap()))
            .unwrap()
            .contains("member-by-member")
    );
    let changed = receipt
        .output_artifacts
        .iter()
        .find(|a| {
            a.path
                .ends_with("checks/referee-0/artifacts/independent.py")
        })
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join(&changed.path)).unwrap(),
        "# changed while executing\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("notes.md")).unwrap(),
        "# Parity\n\nTODO\n"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn ordinary_imports_and_bounded_build_outputs_preserve_supplied_source_identity() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    std::fs::write(
        root.join("arithmetic.py"),
        "def square(n):\n    return n*n\n",
    )
    .unwrap();
    std::fs::write(root.join("check.py"),"import json\nfrom pathlib import Path\nfrom arithmetic import square\nv=json.loads(Path('problem.json').read_text())['values']\nassert all(square(n)%2==n%2 for n in v)\nPath('target/checked.json').write_text(json.dumps(v))\nassert Path('notes.md').read_text().startswith('# Parity')\nprint('ordinary imported module and private build output checked')\n").unwrap();
    let mut configured = spec();
    configured.inputs.push("arithmetic.py".into());
    start(&root, configured).unwrap();
    let (runtime, _, integrator) = runtime(&root, Scenario::Correct);
    let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(result["status"], "completed", "{result:#}");
    let bundles = integrator.bundles.lock().unwrap();
    assert_eq!(bundles.len(), 1);
    assert!(
        bundles[0]
            .checks
            .iter()
            .all(|receipt| receipt.source_sha256 == receipt.source_after_sha256)
    );
    assert!(
        root.join(role_work("parity", &bundles[0].writer.id))
            .join("target/checked.json")
            .exists()
    );
    assert!(
        std::fs::read_to_string(root.join("arithmetic.py"))
            .unwrap()
            .contains("return n*n")
    );
    drop(bundles);
    std::fs::remove_dir_all(root).unwrap();
}

const EXACT_CANDIDATE_INVENTORY_CHECK: &str = "expected=json.loads(Path('problem.json').read_text())['candidate_files']\nactual=sorted(p.relative_to('submission').as_posix() for p in Path('submission').rglob('*') if p.is_file())\nassert actual==sorted(expected), (actual,sorted(expected))\nassert all(Path('submission',name).read_text()==text for name,text in expected.items())\nprint('exact nested candidate membership and bytes checked')\n";

#[test]
fn isolated_file_slices_reconstruct_large_primary_material_without_cross_role_handles() {
    let root = workspace();
    let body = "Primary evidence ⡬: café\n".repeat(10000);
    std::fs::write(root.join("primary.md"), &body).unwrap();
    let tool = RoleFiles { root: root.clone() };
    let mut offset = 0;
    let mut reconstructed = String::new();
    loop {
        let result = tool
            .call(&json!({"action":"read","path":"primary.md","offset":offset,"max_bytes":8192}))
            .unwrap();
        assert!(
            result.len() < 10 * 1024,
            "slice stays inline under the default eager-offload floor"
        );
        let (header, text) = result.split_once('\n').unwrap();
        let header: Value = serde_json::from_str(header).unwrap();
        assert_eq!(header["read_slice"]["offset"], offset);
        assert_eq!(header["read_slice"]["total_bytes"], body.len());
        assert_eq!(header["read_slice"]["sha256"], sha256_hex(body.as_bytes()));
        reconstructed.push_str(text);
        match header["read_slice"]["next_offset"].as_u64() {
            Some(next) => {
                assert!(next > offset);
                offset = next;
            }
            None => break,
        }
    }
    assert_eq!(reconstructed, body);
    assert_eq!(
        sha256_hex(&std::fs::read(root.join("primary.md")).unwrap()),
        sha256_hex(body.as_bytes())
    );
    let glyph = body.find('⡬').unwrap() as u64;
    assert!(
        tool.call(&json!({"action":"read","path":"primary.md","offset":glyph+1}))
            .is_err()
    );
    assert!(
        tool.call(&json!({"action":"read","path":"primary.md","offset":glyph,"max_bytes":1}))
            .is_err()
    );
    assert!(
        tool.call(&json!({"action":"read","path":"../primary.md"}))
            .is_err()
    );
    assert!(
        tool.call(&json!({"action":"read","path":"primary.md","max_bytes":8193}))
            .is_err()
    );
    assert!(
        tool.def()
            .description
            .chars()
            .all(|c| ('\u{2800}'..='\u{28ff}').contains(&c))
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn exact_nested_candidate_inventory_survives_referee_writer_and_coordinator_replays() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    let candidates = json!({"nested/candidate.lean":"def value : Nat := 3\n", "nested/layout.json":"{\"offset\":8}\n"});
    std::fs::create_dir_all(root.join("submission/nested")).unwrap();
    for (name, text) in candidates.as_object().unwrap() {
        std::fs::write(root.join("submission").join(name), text.as_str().unwrap()).unwrap();
    }
    let problem: Value =
        serde_json::from_slice(&std::fs::read(root.join("problem.json")).unwrap()).unwrap();
    std::fs::write(
        root.join("problem.json"),
        serde_json::to_vec(&json!({"values":problem["values"],"candidate_files":candidates}))
            .unwrap(),
    )
    .unwrap();
    for script in ["check.py", "spot.py"] {
        let original = std::fs::read_to_string(root.join(script)).unwrap();
        std::fs::write(
            root.join(script),
            format!("import json\n{original}{EXACT_CANDIDATE_INVENTORY_CHECK}"),
        )
        .unwrap();
    }
    let mut configured = spec();
    for name in [
        "submission/nested/candidate.lean",
        "submission/nested/layout.json",
    ] {
        configured.inputs.push(name.into());
        configured.referee_inputs.push(name.into());
    }
    start(&root, configured).unwrap();
    let (runtime, _, integrator) = runtime(&root, Scenario::ExactCandidateInventory);
    let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(result["status"], "completed", "{result:#}");
    let bundles = integrator.bundles.lock().unwrap();
    assert_eq!(bundles.len(), 1);
    for receipt in &bundles[0].checks {
        let output = std::fs::read_to_string(root.join(&receipt.output_path)).unwrap();
        assert!(
            output.contains("exact nested candidate membership and bytes checked"),
            "{}: {output}",
            receipt.stage
        );
    }
    drop(bundles);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn generated_output_grants_cannot_overlap_explicit_trusted_inputs() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    std::fs::create_dir(root.join("target")).unwrap();
    std::fs::write(root.join("target/trusted.json"), "{\"answer\":17}\n").unwrap();
    std::fs::write(root.join("check.py"),"from pathlib import Path\np=Path('target/trusted.json')\noriginal=p.read_bytes()\np.write_text('{}')\np.write_bytes(original)\n").unwrap();
    let mut configured = spec();
    configured.inputs.push("target/trusted.json".into());
    start(&root, configured).unwrap();
    let (runtime, _, integrator) = runtime(&root, Scenario::Correct);
    let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(result["status"], "budget_exhausted", "{result:#}");
    assert!(integrator.bundles.lock().unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("target/trusted.json")).unwrap(),
        "{\"answer\":17}\n"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpoints_repair_state_only_events_and_skip_legacy_orphan_event_indices() {
    let root = workspace();
    start(&root, spec()).unwrap();
    let initial = load_state(&root, "parity").unwrap().1;
    let event_path = base("parity").join(format!("events/{:06}.json", initial.sequence));
    let exact = std::fs::read(root.join(&event_path)).unwrap();
    std::fs::remove_file(root.join(&event_path)).unwrap();
    assert_eq!(status(&root, Some("parity")).unwrap()["status"], "ready");
    assert_eq!(
        std::fs::read(root.join(event_path)).unwrap(),
        exact,
        "state-only crash repair retains the exact committed event and timestamp"
    );
    let orphan = base("parity").join(format!("events/{:06}.json", initial.sequence + 1));
    let bytes = b"{\"legacy_orphan\":true}";
    publish(&root, &orphan, bytes).unwrap();
    let mut resumed = load_state(&root, "parity").unwrap().1;
    save(&root, &mut resumed, json!({"type":"recovery-progress"})).unwrap();
    assert_eq!(resumed.sequence, initial.sequence + 2);
    assert_eq!(std::fs::read(root.join(orphan)).unwrap(), bytes);
    assert_eq!(
        load_state(&root, "parity").unwrap().1.sequence,
        resumed.sequence
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn specification_only_initialization_resumes_its_retained_baseline() {
    let root = workspace();
    let configured = spec();
    start(&root, configured.clone()).unwrap();
    std::fs::remove_dir_all(root.join(base("parity").join("states"))).unwrap();
    std::fs::remove_dir_all(root.join(base("parity").join("events"))).unwrap();
    std::fs::write(root.join("problem.json"), "{}\n").unwrap();
    let resumed = start(&root, configured).unwrap();
    assert_eq!(resumed["status"], "ready");
    assert!(
        String::from_utf8(
            read(
                &root,
                &base("parity").join("baseline/problem.json"),
                FILE_BYTES
            )
            .unwrap()
        )
        .unwrap()
        .contains("values")
    );
    std::fs::remove_dir_all(root).unwrap();
}

struct UninterruptibleClub {
    calls: AtomicUsize,
    release: Arc<AtomicBool>,
}
impl Club for UninterruptibleClub {
    fn label(&self) -> &str {
        "scripted-uninterruptible-provider"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        Err("requires chat".into())
    }
    fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        while !self.release.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(ClubReply::Text("{\"leads\":[]}".into()))
    }
}

#[test]
fn cancelled_provider_capacity_remains_owned_across_coordinator_resumes() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    let mut configured = spec();
    configured.concurrency = 1;
    configured.role_timeout_secs = 1;
    configured.campaign_timeout_secs = 10;
    configured.max_rounds = 2;
    start(&root, configured).unwrap();
    let release = Arc::new(AtomicBool::new(false));
    let club = Arc::new(UninterruptibleClub {
        calls: AtomicUsize::new(0),
        release: Arc::clone(&release),
    });
    let runtime = CampaignRuntime {
        literature: club.clone(),
        attack: club.clone(),
        referee: club.clone(),
        writer: club.clone(),
        integrator: Arc::new(CheckingIntegrator::default()),
    };
    let blocked = run(&root, "parity", runtime.clone(), &AtomicBool::new(false)).unwrap();
    let resumed = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
    let calls = club.calls.load(Ordering::Acquire);
    release.store(true, Ordering::Release);
    let deadline = Instant::now() + Duration::from_secs(2);
    while campaign_seats(&root, "parity")
        .unwrap()
        .load(Ordering::Acquire)
        > 0
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(blocked["status"], "execution_blocked", "{blocked:#}");
    assert_eq!(resumed["status"], "execution_blocked", "{resumed:#}");
    assert_eq!(
        calls, 1,
        "resume cannot replace an unfinished physically occupying provider"
    );
    assert_eq!(
        campaign_seats(&root, "parity")
            .unwrap()
            .load(Ordering::Acquire),
        0
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_map_ids_are_retained_exactly_and_private_path_aliases_are_refused() {
    let root = workspace();
    let mut configured = spec();
    configured.doors[0].id = "q.sig-golf".into();
    configured.closed_routes[0].door_id = "q.sig-golf".into();
    let started = start(&root, configured).unwrap();
    assert_eq!(started["doors"][0]["door_id"], "q.sig-golf");
    let role_id = started["doors"][0]["attack"]["id"].as_str().unwrap();
    assert!(identifier(role_id));
    assert!(!role_id.contains('.'));
    for alias in [
        "labyrinth//angel/campaigns/other/report.md",
        "labyrinth/./angel/campaigns/other/report.md",
        "notes\\.md",
        "notes.md/",
    ] {
        assert!(
            source_path(alias).is_err(),
            "alias {alias} bypassed source ownership"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn checked_negative_reviews_project_real_failed_routes_and_test_the_proposed_map() {
    let _guard = crate::tests::env_lock();
    for scenario in [Scenario::VerifiedRefutation, Scenario::CheckedGap] {
        let root = workspace();
        super::super::initialize(&root).unwrap();
        let path = root.join("labyrinth/knowledge.json");
        let mut knowledge: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        knowledge["nodes"].as_array_mut().unwrap().push(json!({"id":"q.parity","kind":"question","title":"Exact parity claim","statement":ORIGINAL,"status":"open"}));
        std::fs::write(&path, serde_json::to_vec_pretty(&knowledge).unwrap()).unwrap();
        let (node, verdict, kind) = if scenario == Scenario::VerifiedRefutation {
            ("ev.parity-refutation", "FALSE", "deadend")
        } else {
            ("cj.parity-gap", "GAP", "conjecture")
        };
        std::fs::write(root.join("spot.py"),format!("import json\nfrom pathlib import Path\nassert {ORIGINAL:?} in Path('notes.md').read_text()\nnodes={{node['id']:node for node in json.loads(Path('labyrinth/knowledge.json').read_text())['nodes']}}\nassert nodes[{node:?}]['kind']=={kind:?}\nquestion=nodes['q.parity']\nassert question['status']=='open'\nassert any(route['verdict']=={verdict:?} for route in question['failed_routes'])\nprint('coordinator checked proposed map and retained open question')\n")).unwrap();
        let mut configured = spec();
        configured.doors[0].id = "q.parity".into();
        configured.closed_routes[0].door_id = "q.parity".into();
        start(&root, configured).unwrap();
        let (mut runtime, _, _) = runtime(&root, scenario);
        runtime.integrator = Arc::new(super::super::workflow::ArtifactIntegrator);
        let result = run(&root, "parity", runtime, &AtomicBool::new(false)).unwrap();
        assert_eq!(result["status"], "completed", "{result:#}");
        let map = super::super::load(&root).unwrap().unwrap();
        let question = map
            .nodes
            .iter()
            .find(|node| node["id"] == "q.parity")
            .unwrap();
        assert_eq!(question["status"], "open");
        assert!(
            question["failed_routes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|route| route["verdict"] == verdict)
        );
        let mapped = map.nodes.iter().find(|value| value["id"] == node).unwrap();
        if scenario == Scenario::VerifiedRefutation {
            assert_eq!(super::super::passage(mapped).label(), "wall");
        } else {
            assert_eq!(mapped["tier"], "T5");
            assert_eq!(mapped["review"]["state"], "under-review");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn role_compute_and_coordinator_checks_share_one_cancellable_subprocess_slot() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    let configured = spec();
    start(&root, configured.clone()).unwrap();
    let a = prepare_role(&root, &configured, &role("parity", "odd", "attack", 0, 7)).unwrap();
    let b = prepare_role(&root, &configured, &role("parity", "odd", "referee", 0, 7)).unwrap();
    let script = "import json,time\nfrom pathlib import Path\nstart=time.time_ns()\ntime.sleep(.12)\nPath('artifacts/times.json').write_text(json.dumps([start,time.time_ns()]))\n";
    publish(&a, Path::new("artifacts/clock.py"), script.as_bytes()).unwrap();
    publish(&b, Path::new("artifacts/clock.py"), script.as_bytes()).unwrap();
    let exec = RoleExec {
        root: a.clone(),
        timeout: Duration::from_secs(3),
    };
    std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            exec.call(&json!({"argv":["python3","artifacts/clock.py"]}))
                .unwrap()
        });
        let second = scope.spawn(|| {
            run_check(
                &root,
                &configured,
                "referee-odd-r0-a7",
                "referee",
                &b,
                &CheckCommand {
                    argv: vec![
                        "python3".into(),
                        "-I".into(),
                        "-S".into(),
                        "-B".into(),
                        "artifacts/clock.py".into(),
                    ],
                    timeout_secs: 3,
                },
                0,
                Some("artifacts/clock.py"),
                &AtomicBool::new(false),
            )
            .unwrap()
        });
        first.join().unwrap();
        second.join().unwrap();
    });
    let aa: Vec<u128> =
        serde_json::from_slice(&read(&a, Path::new("artifacts/times.json"), REPORT_BYTES).unwrap())
            .unwrap();
    let bb: Vec<u128> =
        serde_json::from_slice(&read(&b, Path::new("artifacts/times.json"), REPORT_BYTES).unwrap())
            .unwrap();
    assert!(
        aa[1] <= bb[0] || bb[1] <= aa[0],
        "role compute and coordinator check physically overlapped: {aa:?} {bb:?}"
    );
    let permit = SubprocessPermit::acquire(Instant::now() + Duration::from_secs(1), None).unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let child_cancel = Arc::clone(&cancelled);
    let exec = RoleExec {
        root: a.clone(),
        timeout: Duration::from_secs(3),
    };
    let blocked = std::thread::spawn(move || {
        exec.call_with_cancel(&json!({"argv":["python3","-c","from pathlib import Path; Path('artifacts/never-started').write_text('unsafe')"]}),Some(&child_cancel))
    });
    std::thread::sleep(Duration::from_millis(30));
    cancelled.store(true, Ordering::Release);
    let result = blocked.join().unwrap();
    drop(permit);
    assert!(result.unwrap_err().contains("admission cancelled"));
    assert!(!a.join("artifacts/never-started").exists());
    std::fs::remove_dir_all(root).unwrap();
}

struct ScriptedFetch;
impl Tool for ScriptedFetch {
    fn name(&self) -> &str {
        "web_fetch"
    }
    fn def(&self) -> ToolDef {
        crate::agent::tools::web::WebFetchTool.def()
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        assert_eq!(args["url"], "https://example.invalid/retained-paper");
        Ok("[200 text/plain] https://example.invalid/retained-paper\n\nZero is even; odd-square universality requires its domain restriction.".into())
    }
}

#[test]
fn bounded_role_schemas_keep_scoped_capabilities_and_retrieval_is_immutably_grounded() {
    let _guard = crate::tests::env_lock();
    let root = workspace();
    let configured = spec();
    start(&root, configured.clone()).unwrap();
    let literature = role("parity", "all", "literature", 0, 8);
    let work = prepare_role(&root, &configured, &literature).unwrap();
    let leads = Arc::new(Mutex::new(Vec::new()));
    let registry = role_registry(
        &work,
        "literature",
        &configured,
        &root,
        &literature.id,
        &leads,
    );
    let defs = registry.defs_for_run(Some(1024), true);
    for name in ["read_file", "campaign_file", "campaign_lead"] {
        assert!(
            defs.iter().any(|def| def.name == name),
            "bounded role lost core tool {name}"
        );
    }
    if crate::agent::harness::env_flag("ANGEL_WEB_SEARCH", true) {
        assert!(defs.iter().any(|def| def.name == "web_search"));
    }
    if crate::agent::harness::env_flag("ANGEL_WEB_FETCH", true) {
        assert!(defs.iter().any(|def| def.name == "web_fetch"));
    }
    assert!(
        !defs
            .iter()
            .any(|def| matches!(def.name.as_str(), "shell" | "write_file" | "campaign_exec"))
    );
    let attack = role("parity", "odd", "attack", 0, 8);
    let attack_work = prepare_role(&root, &configured, &attack).unwrap();
    let defs = role_registry(
        &attack_work,
        "attack",
        &configured,
        &root,
        &attack.id,
        &leads,
    )
    .defs_for_run(Some(1024), true);
    assert!(defs.iter().any(|def| def.name == "campaign_exec"));
    assert!(!defs.iter().any(|def| def.name == "web_fetch"));
    let archive = role_archive(&configured.id, &literature.id);
    let fetch = RetainedRead {
        inner: Box::new(ScriptedFetch),
        canonical: root.clone(),
        work: work.clone(),
        archive: archive.clone(),
        requests: Arc::new(AtomicUsize::new(0)),
    };
    assert!(
        fetch
            .def()
            .description
            .chars()
            .all(|c| c == '\n' || ('\u{2800}'..='\u{28ff}').contains(&c))
    );
    let fetched: Value = serde_json::from_str(
        &fetch
            .call(&json!({"url":"https://example.invalid/retained-paper"}))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(fetched["source_checked"], true);
    assert_eq!(fetched["claim_verified"], false);
    let lead_tool = LeadTool {
        campaign_root: root.clone(),
        archive: archive.clone(),
        mailbox: Arc::clone(&leads),
    };
    let lead = json!({"id":"source-parity","doors":["odd"],"statement":"Separate odd and even classes","source":"Paper citation","source_url":"https://example.invalid/retained-paper","source_receipt":fetched["retrieval_receipt"]});
    lead_tool.call(&lead).unwrap();
    assert_eq!(leads.lock().unwrap()[0]["source_checked"], true);
    assert_eq!(leads.lock().unwrap()[0]["claim_verified"], false);
    assert!(
        leads.lock().unwrap()[0]["source_excerpt"]
            .as_str()
            .unwrap()
            .contains("Zero is even")
    );
    let receipt_path = fetched["retrieval_receipt"]["path"].as_str().unwrap();
    let original = read(&root, Path::new(receipt_path), REPORT_BYTES).unwrap();
    let mut wrong = lead.clone();
    wrong["id"] = json!("wrong-source");
    wrong["source_url"] = json!("https://example.invalid/unfetched-paper");
    lead_tool.call(&wrong).unwrap();
    assert_eq!(leads.lock().unwrap()[1]["source_checked"], false);
    let output = archive_role(
        &root,
        &configured,
        &literature,
        &work,
        "{\"leads\":[]}",
        json!({"id":literature.id,"role":"literature","stop_reason":"answer"}),
    )
    .unwrap();
    assert!(
        output
            .evidence
            .artifacts
            .iter()
            .any(|artifact| artifact.path == receipt_path
                && artifact.sha256 == sha256_hex(&original)),
        "original retrieval bytes and request receipt join immutable role provenance"
    );
    let mut forged = lead;
    forged["source_receipt"]["sha256"] = json!("0".repeat(64));
    assert!(lead_tool.call(&forged).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
