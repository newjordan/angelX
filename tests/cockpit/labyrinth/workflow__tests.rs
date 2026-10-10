use super::*;
use crate::drive::labyrinth::campaign::{
    ArtifactDigest, CanonicalSource, CheckReceipt, Correction, DocumentEdit,
};

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "angel-labyrinth-workflow-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn write_json(root: &Path, path: &str, value: &Value) -> Vec<u8> {
    let bytes = serde_json::to_vec(value).unwrap();
    write(root, path, &bytes);
    bytes
}

fn role(
    root: &Path,
    name: &str,
    kind: &str,
    result: &Value,
    files: &[(&str, &str)],
) -> RoleEvidence {
    let archive = format!("labyrinth/angel/campaigns/campaign/agents/{name}");
    let report_path = format!("{archive}/report.md");
    let result_path = format!("{archive}/result.json");
    let workspace_path = format!("{archive}/source");
    let report = serde_json::to_vec(result).unwrap();
    write(root, &report_path, &report);
    let mut artifacts = Vec::new();
    let mut sources = BTreeMap::new();
    let mut manifest = Vec::new();
    for (source_path, bytes) in files {
        let artifact = source_path.starts_with("artifacts/");
        let path = if artifact {
            format!("{archive}/{source_path}")
        } else {
            format!("{workspace_path}/{source_path}")
        };
        write(root, &path, bytes.as_bytes());
        let hash = sha256_hex(bytes.as_bytes());
        if artifact {
            artifacts.push(ArtifactDigest {
                path: path.clone(),
                sha256: hash.clone(),
            });
        } else {
            sources.insert(*source_path, hash.clone());
            manifest.push(json!({"source_path":source_path,"path":path,"sha256":hash}));
        }
    }
    let mut pairs: Vec<_> = sources
        .iter()
        .map(|(name, hash)| (*name, hash.as_str()))
        .collect();
    pairs.sort_by(|left, right| Path::new(left.0).cmp(Path::new(right.0)));
    let hash = sha256_hex(&serde_json::to_vec(&pairs).unwrap());
    write_json(
        root,
        &format!("{archive}/source-manifest.json"),
        &json!({
            "schema":1,"source_sha256":hash,"source_after_sha256":hash,"files":manifest
        }),
    );
    let result_bytes = write_json(
        root,
        &result_path,
        &json!({"schema":1,"id":name,"role":kind,
        "stop_reason":"answer","report_sha256":sha256_hex(&report),"source_sha256":hash}),
    );
    RoleEvidence {
        id: name.into(),
        role: kind.into(),
        report_path,
        report_sha256: sha256_hex(&report),
        result_path,
        result_sha256: sha256_hex(&result_bytes),
        workspace_path,
        artifacts,
    }
}

fn receipt(
    root: &Path,
    role: &RoleEvidence,
    name: &str,
    stage: &str,
    code: Option<&str>,
) -> CheckReceipt {
    let archive = role.workspace_path.rsplit_once('/').unwrap().0;
    let source = json_file(root, &format!("{archive}/source-manifest.json")).unwrap();
    let output_path = format!("{archive}/checks/{name}.txt");
    let output = b"check passed\n";
    write(root, &output_path, output);
    let mut command = vec!["python3".to_string()];
    if stage == "referee" {
        command.extend(["-I".into(), "-S".into()]);
    }
    command.push(code.unwrap_or("tests/check.py").to_string());
    let code_path = code.map(|path| format!("{archive}/{path}"));
    let code_sha256 = code_path
        .as_ref()
        .map(|path| sha256_hex(&read(root, path, MAX_FILE).unwrap()));
    CheckReceipt {
        id: name.into(),
        stage: stage.into(),
        workspace_path: role.workspace_path.clone(),
        command_sha256: sha256_hex(&serde_json::to_vec(&command).unwrap()),
        command,
        source_sha256: source["source_sha256"].as_str().unwrap().into(),
        source_after_sha256: source["source_sha256"].as_str().unwrap().into(),
        exit_code: Some(0),
        timed_out: false,
        cancelled: false,
        output_path,
        output_sha256: sha256_hex(output),
        elapsed_ms: 1,
        code_path,
        code_sha256,
        code_after_sha256: None,
        output_artifacts: vec![],
        fresh: true,
    }
}

fn fixture() -> (Workspace, IntegrationBundle) {
    let workspace = Workspace::new();
    let root = &workspace.0;
    let old_statement = "all tested cases pass";
    let statement = "the sampled cases pass";
    let knowledge = json!({"schema":1,"meta":{"title":"Authored research","custom":"keep"},
        "frontier_history":[{"date":"2026-09-01","size":5,"resolved":0,"why":"author history"}],
        "nodes":[{"id":"ev.claim","kind":"evidence","tier":"T4","status":"observed",
            "title":"Original title","statement":old_statement,"links":[],"evidence":["authored-input"],
            "custom_author":{"keep":true},"review":{"state":"under-review","custom_note":"retain"}},
            {"id":"h.retained","kind":"hunch","tier":"T6","status":"live","title":"Untouched direction",
                "statement":"Try a new parameter","links":[],"custom_author":[1,2,3]}]});
    let sota = json!({"about":"Author table","groups":["bound"],"entries":[{
        "id":"row1","group":"bound","cls":"instances","result":"0 realized","status":"evidence",
        "refs":["older-label"],"lit":[],"updated":"2026-10-01",
        "previous":[{"date":"2026-09-01","was":"unknown"}],"custom_author":"keep"
    }]});
    let frontier = json!({"meta":{"size_name":"n","value_name":"value","custom":"keep"},"sizes":[{
        "size":6,"lo":0,"hi":2,"step":1,"realized":[],"intervals":[],
        "bounds":{"lower_proved":0,"upper":2,"upper_kind":"unknown","custom":"keep"},"custom":"keep"
    }]});
    write_json(root, "labyrinth/knowledge.json", &knowledge);
    write_json(root, "labyrinth/sota.json", &sota);
    write_json(root, "labyrinth/frontier.json", &frontier);
    write(root, "notes.md", format!("{old_statement}\n").as_bytes());
    let correction =
        json!({"id":"correction1","claim_id":"ev.claim","old":"all tested","new":"the sampled"});
    let original = json!({"id":"ev.claim","statement":old_statement,"status":"EVIDENCE","evidence":["artifacts/source.py"]});
    let corrected = json!({"id":"ev.claim","statement":statement,"status":"EVIDENCE"});
    let nodes = vec![
        json!({"id":"ev.claim","claim_id":"ev.claim","kind":"evidence","tier":"T4",
        "status":"observed","statement":statement}),
    ];
    let rows = vec![
        json!({"id":"row1","group":"bound","cls":"instances","result":"1 realized",
        "status":"evidence","kind":"computed","refs":["ev.claim"],"lit":[],"claim_id":"ev.claim",
        "frontier":{"size":6,"field":"upper","value":2}}),
    ];
    let new_frontier = json!({"sizes":[{"size":6,"realized":[[1,"sampled witness"]]}]});
    let proposed_edit = json!({"path":"notes.md","old":old_statement,"new":statement});
    let writer_result = json!({"claims":[corrected],"applied_corrections":["correction1"],
        "edits":[proposed_edit],"nodes":nodes,"sota":rows,"frontier":new_frontier});
    let author = role(
        root,
        "author",
        "attack",
        &json!({"claims":[original]}),
        &[
            ("notes.md", "all tested cases pass\n"),
            ("artifacts/source.py", "# author algorithm\nprint(1)\n"),
        ],
    );
    let literature = role(
        root,
        "literature",
        "literature",
        &json!({"leads":[{"source":"primary source","claim":"scope"}]}),
        &[("lead.md", "Primary-source citation and scope.\n")],
    );
    let referee = role(
        root,
        "referee",
        "referee",
        &json!({"verdicts":[{"claim_id":"ev.claim",
        "verdict":"ESTABLISHED WITH CORRECTIONS","reason":"only sampled instances were checked",
        "corrections":[correction]}],"independent_check":{"code":"artifacts/check.py","argv":["python3","-I","-S","artifacts/check.py"]}}),
        &[(
            "artifacts/check.py",
            "# independent checker\nassert sum([1]) == 1\n",
        )],
    );
    let writer_before = role(
        root,
        "writer-before",
        "writer",
        &writer_result,
        &[
            ("notes.md", "all tested cases pass\n"),
            ("tests/check.py", "# baseline reader document checker\n"),
        ],
    );
    let mut writer = role(
        root,
        "writer-checked",
        "writer",
        &writer_result,
        &[
            ("notes.md", "the sampled cases pass\n"),
            ("tests/check.py", "# writer reader document checker\n"),
        ],
    );
    let mut writer_metadata = json_file(root, &writer.result_path).unwrap();
    writer_metadata["draft_role_id"] = json!(writer_before.id);
    writer_metadata["draft_result_sha256"] = json!(writer_before.result_sha256);
    writer.result_sha256 = sha256_hex(&write_json(root, &writer.result_path, &writer_metadata));
    let coordinator = role(
        root,
        "coordinator",
        "coordinator",
        &json!({"check":"spot checked"}),
        &[
            ("notes.md", "the sampled cases pass\n"),
            ("tests/check.py", "# coordinator spot checker\n"),
        ],
    );
    let checks = vec![
        receipt(
            root,
            &referee,
            "refcheck",
            "referee",
            Some("artifacts/check.py"),
        ),
        receipt(root, &writer_before, "beforecheck", "writer-before", None),
        receipt(root, &writer, "writercheck1", "writer", None),
        receipt(root, &writer, "writercheck2", "writer", None),
        receipt(root, &coordinator, "coordcheck", "coordinator", None),
    ];
    let canonical_sources: Vec<_> = [
        "labyrinth/knowledge.json",
        "labyrinth/sota.json",
        "labyrinth/frontier.json",
    ]
    .into_iter()
    .map(|path| CanonicalSource {
        path: path.into(),
        sha256: Some(sha256_hex(&read(root, path, MAX_FILE).unwrap())),
    })
    .collect();
    let canonical_sources_path =
        "labyrinth/angel/campaigns/campaign/agents/writer-before/canonical-sources.json"
            .to_string();
    let source_bytes = write_json(
        root,
        &canonical_sources_path,
        &serde_json::to_value(&canonical_sources).unwrap(),
    );
    let mut normalized = corrected;
    normalized["referee"] =
        json!({"id":"referee","verdict":"ESTABLISHED WITH CORRECTIONS","reason":"sample scope"});
    let mut bundle = IntegrationBundle {
        schema: 1,
        id: "integration".into(),
        campaign_id: "campaign".into(),
        reviewed_at: "2026-10-09T00:00:00Z".into(),
        task: "check the sampled claim".into(),
        door_id: "ev.claim".into(),
        coordinator_id: "coordinator".into(),
        author,
        literature,
        referee,
        writer_before,
        writer,
        coordinator,
        canonical_sources,
        canonical_sources_path,
        canonical_sources_sha256: sha256_hex(&source_bytes),
        claims: vec![normalized],
        nodes,
        sota: rows,
        frontier: Some(new_frontier),
        edits: vec![DocumentEdit {
            path: "notes.md".into(),
            old: old_statement.into(),
            new: statement.into(),
            before_sha256: sha256_hex(b"all tested cases pass\n"),
            after_sha256: sha256_hex(b"the sampled cases pass\n"),
        }],
        corrections: vec![Correction {
            id: "correction1".into(),
            claim_id: "ev.claim".into(),
            field: None,
            old: "all tested".into(),
            new: "the sampled".into(),
        }],
        applied_corrections: vec!["correction1".into()],
        checks,
    };
    refresh_preview(root, &mut bundle);
    (workspace, bundle)
}

#[test]
fn source_manifests_use_component_order_when_a_directory_and_file_share_a_prefix() {
    let workspace = Workspace::new();
    let evidence = role(
        &workspace.0,
        "author",
        "attack",
        &json!({"claims":[]}),
        &[("cases.part.json", "file"), ("cases/member.json", "nested")],
    );
    let (_, manifest) = role_manifest(&workspace.0, "campaign", &evidence).unwrap();
    let native_pairs = [
        ("cases/member.json", sha256_hex(b"nested")),
        ("cases.part.json", sha256_hex(b"file")),
    ];
    assert_eq!(
        manifest.hash,
        sha256_hex(&serde_json::to_vec(&native_pairs).unwrap())
    );
    write(
        &workspace.0,
        &format!("{}/cases/member.json", evidence.workspace_path),
        b"changed",
    );
    assert!(role_manifest(&workspace.0, "campaign", &evidence).is_err());
}

fn refresh_preview(root: &Path, bundle: &mut IntegrationBundle) {
    let preview = preview_canonical(root, bundle).unwrap();
    let mut images = preview["files"].as_array().unwrap().clone();
    let mut documents = BTreeMap::new();
    for edit in &bundle.edits {
        let text = documents.entry(edit.path.clone()).or_insert_with(|| {
            String::from_utf8(read(root, &edit.path, MAX_FILE).unwrap()).unwrap()
        });
        assert_eq!(text.matches(&edit.old).count(), 1);
        *text = text.replacen(&edit.old, &edit.new, 1);
    }
    for (path, content) in documents {
        images.push(json!({"path":path,"content":content}));
    }
    for role in [&mut bundle.writer, &mut bundle.coordinator] {
        let archive = role.workspace_path.rsplit_once('/').unwrap().0;
        let manifest_path = format!("{archive}/source-manifest.json");
        let mut manifest = json_file(root, &manifest_path).unwrap();
        for file in &images {
            let source_path = file["path"].as_str().unwrap();
            let path = format!("{}/{source_path}", role.workspace_path);
            let bytes = file["content"].as_str().unwrap().as_bytes();
            write(root, &path, bytes);
            let files = manifest["files"].as_array_mut().unwrap();
            let entry = json!({"source_path":source_path,"path":path,"sha256":sha256_hex(bytes)});
            if let Some(existing) = files
                .iter_mut()
                .find(|file| file["source_path"] == source_path)
            {
                *existing = entry;
            } else {
                files.push(entry);
            }
        }
        let mut ordered: BTreeMap<String, String> = BTreeMap::new();
        for file in manifest["files"].as_array().unwrap() {
            ordered.insert(
                file["source_path"].as_str().unwrap().into(),
                file["sha256"].as_str().unwrap().into(),
            );
        }
        let mut pairs: Vec<_> = ordered.iter().collect();
        pairs.sort_by(|left, right| Path::new(left.0).cmp(Path::new(right.0)));
        let hash = sha256_hex(&serde_json::to_vec(&pairs).unwrap());
        manifest["source_sha256"] = json!(hash);
        manifest["source_after_sha256"] = json!(hash);
        write_json(root, &manifest_path, &manifest);
        let mut metadata = json_file(root, &role.result_path).unwrap();
        metadata["source_sha256"] = json!(hash);
        role.result_sha256 = sha256_hex(&write_json(root, &role.result_path, &metadata));
        for receipt in &mut bundle.checks {
            if receipt.workspace_path == role.workspace_path {
                receipt.source_sha256 = hash.clone();
                receipt.source_after_sha256 = hash.clone();
            }
        }
    }
}

fn canonical(root: &Path) -> Vec<Vec<u8>> {
    [
        "notes.md",
        "labyrinth/knowledge.json",
        "labyrinth/sota.json",
        "labyrinth/frontier.json",
    ]
    .into_iter()
    .map(|path| read(root, path, MAX_FILE).unwrap())
    .collect()
}

fn failure(root: &Path, bundle: &IntegrationBundle, code: &str) {
    let before = canonical(root);
    let err = integrate(root, bundle, &AtomicBool::new(false)).unwrap_err();
    let err: Value = serde_json::from_str(&err).unwrap();
    assert_eq!(err["code"], code, "{err}");
    assert_eq!(
        canonical(root),
        before,
        "failed integration changed canonical files"
    );
    ensure_no_pending(root).unwrap();
}

fn update_report(root: &Path, role: &mut RoleEvidence, mutate: impl FnOnce(&mut Value)) {
    let mut value = json_file(root, &role.report_path).unwrap();
    mutate(&mut value);
    let bytes = write_json(root, &role.report_path, &value);
    role.report_sha256 = sha256_hex(&bytes);
    let mut metadata = json_file(root, &role.result_path).unwrap();
    metadata["report_sha256"] = json!(role.report_sha256);
    role.result_sha256 = sha256_hex(&write_json(root, &role.result_path, &metadata));
}

fn update_writer(root: &Path, bundle: &mut IntegrationBundle, mutate: impl FnOnce(&mut Value)) {
    let mut value = json_file(root, &bundle.writer_before.report_path).unwrap();
    mutate(&mut value);
    for role in [&mut bundle.writer_before, &mut bundle.writer] {
        let bytes = write_json(root, &role.report_path, &value);
        role.report_sha256 = sha256_hex(&bytes);
        let mut metadata = json_file(root, &role.result_path).unwrap();
        metadata["report_sha256"] = json!(role.report_sha256);
        role.result_sha256 = sha256_hex(&write_json(root, &role.result_path, &metadata));
    }
    let mut metadata = json_file(root, &bundle.writer.result_path).unwrap();
    metadata["draft_result_sha256"] = json!(bundle.writer_before.result_sha256);
    bundle.writer.result_sha256 =
        sha256_hex(&write_json(root, &bundle.writer.result_path, &metadata));
}

#[test]
fn renamed_nodes_bind_unique_exact_reviewed_statements_without_rewriting_writer_reports() {
    let (workspace, mut bundle) = fixture();
    bundle.nodes[0]["id"] = json!("ev.renamed");
    bundle.nodes[0].as_object_mut().unwrap().remove("claim_id");
    let nodes = bundle.nodes.clone();
    update_writer(&workspace.0, &mut bundle, |report| {
        report["nodes"] = json!(nodes)
    });
    let raw_report = read(&workspace.0, &bundle.writer_before.report_path, MAX_FILE).unwrap();
    refresh_preview(&workspace.0, &mut bundle);
    assert_eq!(
        integrate(&workspace.0, &bundle, &AtomicBool::new(false)).unwrap()["integrated"],
        true
    );
    let knowledge = json_file(&workspace.0, "labyrinth/knowledge.json").unwrap();
    let node = knowledge["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "ev.renamed")
        .unwrap();
    assert_eq!(node["claim_id"], "ev.claim");
    assert_eq!(node["campaign_provenance"]["claim"], "ev.claim");
    assert_eq!(node["review"]["state"], "refereed");
    assert_eq!(node["tier"], "T4");
    assert_eq!(
        read(&workspace.0, &bundle.writer_before.report_path, MAX_FILE).unwrap(),
        raw_report
    );
    assert!(bundle.nodes[0].get("claim_id").is_none());
}

#[test]
fn exact_statement_binding_never_overrides_an_invalid_explicit_claim_id() {
    for invalid_id in [json!("unknown"), json!(""), Value::Null, json!(17)] {
        let (workspace, mut bundle) = fixture();
        bundle.nodes[0]["id"] = json!("ev.renamed");
        bundle.nodes[0]["claim_id"] = invalid_id;
        let nodes = bundle.nodes.clone();
        update_writer(&workspace.0, &mut bundle, |report| {
            report["nodes"] = json!(nodes)
        });
        failure(&workspace.0, &bundle, "node_provenance");
    }
}

#[test]
fn omitted_claim_ids_require_one_exact_statement_unless_the_node_id_matches() {
    for ambiguous in [false, true] {
        let (workspace, mut bundle) = fixture();
        let mut reviewed = validate_bundle(&workspace.0, &bundle, false).unwrap();
        let mut knowledge = json_file(&workspace.0, "labyrinth/knowledge.json").unwrap();
        bundle.nodes[0].as_object_mut().unwrap().remove("claim_id");
        if ambiguous {
            reviewed
                .claims
                .insert("ev.duplicate".into(), reviewed.claims["ev.claim"].clone());
            // An existing identity remains authoritative even when another statement matches.
            merge_nodes(&workspace.0, &mut knowledge, &bundle, &reviewed).unwrap();
        } else {
            bundle.nodes[0]["statement"] = json!("the sampled cases pass ");
        }
        bundle.nodes[0]["id"] = json!("ev.renamed");
        let error: Value = serde_json::from_str(
            &merge_nodes(&workspace.0, &mut knowledge, &bundle, &reviewed).unwrap_err(),
        )
        .unwrap();
        assert_eq!(error["code"], "node_provenance");
        assert_eq!(
            json_file(&workspace.0, "labyrinth/knowledge.json").unwrap()["nodes"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
}

#[test]
fn complete_reviewed_documents_above_the_old_limit_fit_the_retained_report_budget() {
    for size in [22497, DOCUMENT_EDIT_BYTES / 2] {
        let (workspace, mut bundle) = fixture();
        let mut new = bundle.claims[0]["statement"].as_str().unwrap().to_string();
        new.push_str(&"x".repeat(size - new.len()));
        bundle.edits[0].new = new.clone();
        bundle.edits[0].after_sha256 = sha256_hex(format!("{new}\n").as_bytes());
        update_writer(&workspace.0, &mut bundle, |report| {
            report["edits"][0]["new"] = json!(new)
        });
        refresh_preview(&workspace.0, &mut bundle);
        assert_eq!(
            integrate(&workspace.0, &bundle, &AtomicBool::new(false)).unwrap()["integrated"],
            true
        );
        assert_eq!(
            read(&workspace.0, "notes.md", MAX_FILE).unwrap(),
            format!("{new}\n").as_bytes()
        );
    }
}

#[test]
fn oversized_retained_edits_and_empty_anchors_preserve_canonical_files() {
    for (old, new, expected_error) in [
        (String::new(), "replacement".into(), "edit_budget"),
        (
            "x".repeat(DOCUMENT_EDIT_BYTES + 1),
            "replacement".into(),
            "byte_budget",
        ),
        (
            "all tested cases pass".into(),
            "x".repeat(DOCUMENT_EDIT_BYTES + 1),
            "byte_budget",
        ),
    ] {
        let (workspace, mut bundle) = fixture();
        bundle.edits[0].old = old.clone();
        bundle.edits[0].new = new.clone();
        update_writer(&workspace.0, &mut bundle, |report| {
            report["edits"][0]["old"] = json!(old);
            report["edits"][0]["new"] = json!(new);
        });
        failure(&workspace.0, &bundle, expected_error);
    }
}

#[test]
fn complete_bundle_checks_are_read_only_and_share_native_and_rl_dispatch() {
    use crate::agent::harness::Tool;
    let (workspace, mut bundle) = fixture();
    let path = "labyrinth/angel/campaigns/campaign/integrations/preflight.json";
    write_json(&workspace.0, path, &serde_json::to_value(&bundle).unwrap());
    let before = canonical(&workspace.0);
    let expected = check_integration(&workspace.0, path).unwrap();
    assert_eq!(expected["valid"], true);
    assert_eq!(expected["canonical_writes"], 0);
    assert_eq!(expected["integrated"], false);
    assert_eq!(expected["files"].as_array().unwrap().len(), 4);
    let tool = crate::agent::tools::labyrinth_campaign::LabyrinthCampaignTool::new(
        workspace.0.clone(),
        None,
    );
    let args = json!({"action":"check","bundle":path});
    assert!(!tool.workspace_write_scope_is_opaque(&args));
    assert_eq!(
        serde_json::from_str::<Value>(&tool.call(&args).unwrap()).unwrap(),
        expected
    );
    let rl = crate::agent::tools::loop_research::LoopResearchTool::new(
        workspace.0.clone(),
        std::sync::Arc::new(std::sync::Mutex::new(
            crate::drive::rl_ctl::RlState::default(),
        )),
    );
    let args = json!({"action":"campaign","campaign_action":"check","campaign_bundle":path});
    assert!(!rl.workspace_write_scope_is_opaque(&args));
    assert_eq!(
        serde_json::from_str::<Value>(&rl.call(&args).unwrap()).unwrap(),
        expected
    );
    assert_eq!(canonical(&workspace.0), before);
    bundle.checks[2].fresh = false;
    write_json(&workspace.0, path, &serde_json::to_value(&bundle).unwrap());
    let err: Value =
        serde_json::from_str(&check_integration(&workspace.0, path).unwrap_err()).unwrap();
    assert_eq!(err["code"], "check_failed");
    assert_eq!(canonical(&workspace.0), before);
    ensure_no_pending(&workspace.0).unwrap();
}

#[test]
fn integration_checks_exact_referee_test_and_lesson_corrections_and_rejects_writer_drift() {
    for drift in [false, true] {
        let (workspace, mut bundle) = fixture();
        let root = &workspace.0;
        update_report(root, &mut bundle.author, |report| {
            report["claims"][0]["test"] = json!("Original recorded test");
            report["claims"][0]["lesson"] = json!("Original recorded lesson");
        });
        let fixes = [
            json!({"id":"test-fix","claim_id":"ev.claim","field":"test","old":"Original recorded test","new":"Independently checked test"}),
            json!({"id":"lesson-fix","claim_id":"ev.claim","old":"Original recorded lesson","new":"Independently checked lesson"}),
        ];
        update_report(root, &mut bundle.referee, |report| {
            for fix in &fixes {
                report["verdicts"][0]["corrections"]
                    .as_array_mut()
                    .unwrap()
                    .push(fix.clone());
            }
        });
        for fix in fixes {
            bundle
                .corrections
                .push(serde_json::from_value(fix).unwrap());
        }
        bundle
            .applied_corrections
            .extend(["test-fix".into(), "lesson-fix".into()]);
        bundle.claims[0]["test"] = json!("Independently checked test");
        bundle.claims[0]["lesson"] = json!("Independently checked lesson");
        let claim = bundle.claims[0].clone();
        let applied = bundle.applied_corrections.clone();
        update_writer(root, &mut bundle, |report| {
            report["claims"][0] = claim;
            report["applied_corrections"] = json!(applied);
            if drift {
                report["claims"][0]["test"] = json!("Unreviewed writer claim");
            }
        });
        if drift {
            failure(root, &bundle, "writer_drift");
        } else {
            refresh_preview(root, &mut bundle);
            assert_eq!(
                integrate(root, &bundle, &AtomicBool::new(false)).unwrap()["integrated"],
                true
            );
        }
    }
}

fn refresh_bindings(root: &Path, bundle: &mut IntegrationBundle) {
    for source in &mut bundle.canonical_sources {
        source.sha256 = optional(root, &source.path, MAX_FILE)
            .unwrap()
            .map(|bytes| sha256_hex(&bytes));
    }
    let bytes = write_json(
        root,
        &bundle.canonical_sources_path,
        &serde_json::to_value(&bundle.canonical_sources).unwrap(),
    );
    bundle.canonical_sources_sha256 = sha256_hex(&bytes);
}

#[test]
fn preview_is_read_only_and_final_checks_must_cover_the_exact_generated_research_images() {
    let (workspace, mut bundle) = fixture();
    let before = canonical(&workspace.0);
    let preview = preview_canonical(&workspace.0, &bundle).unwrap();
    assert_eq!(preview["canonical_writes"], 0);
    assert_eq!(canonical(&workspace.0), before);
    assert_eq!(preview["files"].as_array().unwrap().len(), 3);
    for file in preview["files"].as_array().unwrap() {
        for role in [&bundle.writer, &bundle.coordinator] {
            let path = format!("{}/{}", role.workspace_path, file["path"].as_str().unwrap());
            assert_eq!(
                sha256_hex(&read(&workspace.0, &path, MAX_FILE).unwrap()),
                file["sha256"].as_str().unwrap()
            );
        }
    }
    // A valid proposal changed after those executions is still untested.
    // Schema/range validation cannot substitute for consuming its exact bytes.
    bundle.frontier = Some(json!({"sizes":[{"size":6,"realized":[0,1]}]}));
    let frontier = bundle.frontier.clone().unwrap();
    update_writer(&workspace.0, &mut bundle, |report| {
        report["frontier"] = frontier
    });
    failure(&workspace.0, &bundle, "stale_canonical_check");
}

#[test]
fn checked_gap_and_false_routes_reach_the_shared_map_without_closing_the_unresolved_question() {
    for verdict in ["GAP", "FALSE"] {
        let (workspace, mut bundle) = fixture();
        let root = &workspace.0;
        let mut knowledge = json_file(root, "labyrinth/knowledge.json").unwrap();
        let question = json!({"id":"q.tunnel","kind":"question","tier":null,"status":"open",
            "title":"Tunnel route","statement":"Which route works?","links":[],"author_note":"retain"});
        knowledge["nodes"]
            .as_array_mut()
            .unwrap()
            .push(question.clone());
        write_json(root, "labyrinth/knowledge.json", &knowledge);
        refresh_bindings(root, &mut bundle);
        bundle.door_id = "q.tunnel".into();
        update_report(root, &mut bundle.author, |report| {
            report["claims"][0]["route"] = json!("Sampled route leaves a boundary case untested");
            report["claims"][0]["test"] = json!("Check the omitted boundary case");
        });
        let reason = "the independently checked boundary case invalidates the attempted route";
        update_report(root, &mut bundle.referee, |report| {
            report["verdicts"][0]["verdict"] = json!(verdict);
            report["verdicts"][0]["reason"] = json!(reason);
            report["verdicts"][0]["corrections"] = json!([]);
        });
        bundle.claims[0]["statement"] = json!("all tested cases pass");
        bundle.claims[0]["status"] = json!(if verdict == "GAP" {
            "CONJECTURE"
        } else {
            "REFUTED"
        });
        bundle.claims[0]["route"] = json!("Sampled route leaves a boundary case untested");
        bundle.claims[0]["lesson"] = json!(reason);
        bundle.claims[0]["test"] = json!("Check the omitted boundary case");
        bundle.claims[0]["referee"]["verdict"] = json!(verdict);
        if verdict == "FALSE" {
            let archive = bundle.referee.workspace_path.rsplit_once('/').unwrap().0;
            let path = format!("{archive}/artifacts/counterexample.json");
            let bytes = write_json(
                root,
                &path,
                &json!({"input":0,"expected":true,"observed":false}),
            );
            let hash = sha256_hex(&bytes);
            bundle.referee.artifacts.push(ArtifactDigest {
                path: path.clone(),
                sha256: hash.clone(),
            });
            update_report(root, &mut bundle.referee, |report| {
                report["verdicts"][0]["counterexample"] = json!("artifacts/counterexample.json");
            });
            bundle.claims[0]["counterexample"] = json!({"path":path,"sha256":hash});
        }
        bundle.corrections.clear();
        bundle.applied_corrections.clear();
        bundle.edits.clear();
        bundle.nodes.clear();
        bundle.sota.clear();
        bundle.frontier = None;
        let claim = bundle.claims[0].clone();
        update_writer(root, &mut bundle, |report| {
            *report = json!({"claims":[claim],"applied_corrections":[],"edits":[],"nodes":[],"sota":[],"frontier":null});
        });
        refresh_preview(root, &mut bundle);
        let route = bundle.claims[0]["route"].clone();
        bundle.claims[0]["route"] = json!("an unrelated route invented after review");
        failure(root, &bundle, "writer_drift");
        bundle.claims[0]["route"] = route;
        bundle.claims[0]["lesson"] = json!("an unrelated lesson invented after review");
        failure(root, &bundle, "failed_route_provenance");
        bundle.claims[0]["lesson"] = json!(reason);
        integrate(root, &bundle, &AtomicBool::new(false)).unwrap();
        let knowledge = json_file(root, "labyrinth/knowledge.json").unwrap();
        let node = knowledge["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == "q.tunnel")
            .unwrap();
        assert_eq!(node["kind"], question["kind"]);
        assert_eq!(node["tier"], question["tier"]);
        assert_eq!(node["status"], question["status"]);
        assert_eq!(node["author_note"], question["author_note"]);
        assert_eq!(super::super::passage(node), super::super::Passage::Door);
        let route = &node["failed_routes"][0];
        assert_eq!(route["verdict"], verdict);
        assert_eq!(route["lesson"], reason);
        assert_eq!(route["campaign"], bundle.campaign_id);
        assert_eq!(route["check"]["sha256"], bundle.checks[0].output_sha256);
        assert_eq!(
            route["author_report"]["sha256"],
            bundle.author.report_sha256
        );
        if verdict == "FALSE" {
            assert!(route["counterexample"]["sha256"].is_string());
        }
    }
}

#[test]
fn reviewed_campaign_integrates_real_edits_and_preserves_curated_data_without_promoting_t4() {
    let (workspace, bundle) = fixture();
    let original = json_file(&workspace.0, "labyrinth/knowledge.json").unwrap();
    let untouched = original["nodes"][1].clone();
    let outcome = integrate(&workspace.0, &bundle, &AtomicBool::new(false)).unwrap();
    assert_eq!(outcome["integrated"], true);
    assert_eq!(
        read(&workspace.0, "notes.md", MAX_FILE).unwrap(),
        b"the sampled cases pass\n"
    );
    let knowledge = json_file(&workspace.0, "labyrinth/knowledge.json").unwrap();
    let node = &knowledge["nodes"][0];
    assert_eq!(node["tier"], "T4");
    assert_eq!(super::super::passage(node), super::super::Passage::Review);
    assert_eq!(node["review"]["state"], "refereed");
    assert_eq!(node["review"]["custom_note"], "retain");
    assert_eq!(
        node["review_history"][0]["review"],
        original["nodes"][0]["review"]
    );
    assert_eq!(node["title"], "Original title");
    assert_eq!(node["custom_author"], json!({"keep":true}));
    assert!(
        node["evidence"]
            .as_array()
            .unwrap()
            .contains(&json!("authored-input"))
    );
    assert_eq!(knowledge["nodes"][1], untouched);
    assert_eq!(knowledge["meta"], original["meta"]);
    assert_eq!(
        knowledge["frontier_history"][0],
        original["frontier_history"][0]
    );
    let sota = json_file(&workspace.0, "labyrinth/sota.json").unwrap();
    assert_eq!(sota["entries"][0]["previous"].as_array().unwrap().len(), 2);
    assert_eq!(sota["entries"][0]["previous"][1]["was"], "0 realized");
    assert_eq!(sota["entries"][0]["custom_author"], "keep");
    assert!(
        sota["entries"][0]["status"]
            .as_str()
            .unwrap()
            .contains("human check pending")
    );
    let frontier = json_file(&workspace.0, "labyrinth/frontier.json").unwrap();
    assert_eq!(frontier["sizes"][0]["bounds"]["custom"], "keep");
    assert_eq!(frontier["meta"]["custom"], "keep");
    let status = check(&workspace.0).unwrap();
    assert_eq!(status["frontier"]["sizes"][0]["resolved"], 1);
    assert_eq!(status["frontier"]["sizes"][0]["unknown"], 2);
    assert_eq!(status["events"], 1);
    assert_eq!(
        read(&workspace.0, "labyrinth/angel/.gitignore", 4096).unwrap(),
        b"*\n"
    );
}

#[test]
fn exact_completed_bundle_retry_returns_the_retained_receipt_without_reapplying_or_losing_user_edits()
 {
    let (workspace, bundle) = fixture();
    let first = integrate(&workspace.0, &bundle, &AtomicBool::new(false)).unwrap();
    let bytes = canonical(&workspace.0);
    let events = read(&workspace.0, EVENTS, MAX_EVENTS).unwrap();
    let replay = integrate(&workspace.0, &bundle, &AtomicBool::new(false)).unwrap();
    assert_eq!(replay["transaction"], first["transaction"]);
    assert_eq!(replay["already_integrated"], true);
    assert_eq!(replay["canonical_current"], true);
    assert_eq!(canonical(&workspace.0), bytes);
    assert_eq!(read(&workspace.0, EVENTS, MAX_EVENTS).unwrap(), events);
    write(
        &workspace.0,
        "notes.md",
        b"independent changes after integration\n",
    );
    let replay = integrate(&workspace.0, &bundle, &AtomicBool::new(false)).unwrap();
    assert_eq!(replay["already_integrated"], true);
    assert_eq!(replay["canonical_current"], false);
    assert_eq!(replay["changed_since_integration"], json!(["notes.md"]));
    assert_eq!(
        read(&workspace.0, "notes.md", MAX_FILE).unwrap(),
        b"independent changes after integration\n"
    );
    assert_eq!(read(&workspace.0, EVENTS, MAX_EVENTS).unwrap(), events);
}

#[test]
fn published_and_unreviewed_claims_cannot_be_reclassified_as_tools_to_evade_review() {
    for (tier, kind) in [("T1", "theorem"), ("T2", "theorem"), ("T4", "evidence")] {
        let (workspace, mut bundle) = fixture();
        let mut knowledge = json_file(&workspace.0, "labyrinth/knowledge.json").unwrap();
        knowledge["nodes"][0]["tier"] = json!(tier);
        knowledge["nodes"][0]["kind"] = json!(kind);
        knowledge["nodes"][0]["status"] = json!("established");
        write_json(&workspace.0, "labyrinth/knowledge.json", &knowledge);
        bundle.canonical_sources[0].sha256 = Some(sha256_hex(
            &read(&workspace.0, "labyrinth/knowledge.json", MAX_FILE).unwrap(),
        ));
        let source_bytes = write_json(
            &workspace.0,
            &bundle.canonical_sources_path,
            &serde_json::to_value(&bundle.canonical_sources).unwrap(),
        );
        bundle.canonical_sources_sha256 = sha256_hex(&source_bytes);
        bundle.nodes[0].as_object_mut().unwrap().remove("tier");
        bundle.nodes[0]["kind"] = json!("method");
        bundle.nodes[0]["statement"] = json!("An unchecked assertion disguised as a tool");
        let nodes = bundle.nodes.clone();
        update_writer(&workspace.0, &mut bundle, |v| v["nodes"] = json!(nodes));
        failure(&workspace.0, &bundle, "tier_discipline");
    }
}

#[test]
fn referee_success_must_execute_the_owned_code_instead_of_mentioning_it_to_true() {
    let (workspace, mut bundle) = fixture();
    update_report(&workspace.0, &mut bundle.referee, |v| {
        v["independent_check"]["argv"] = json!(["true", "artifacts/check.py"])
    });
    bundle.checks[0].command = vec!["true".into(), "artifacts/check.py".into()];
    bundle.checks[0].command_sha256 =
        sha256_hex(&serde_json::to_vec(&bundle.checks[0].command).unwrap());
    failure(&workspace.0, &bundle, "referee_independence");
    let (workspace, mut bundle) = fixture();
    let author = bundle.author.artifacts.first().unwrap();
    let bytes = read(&workspace.0, &author.path, MAX_FILE).unwrap();
    let referee = bundle.referee.artifacts.first_mut().unwrap();
    write(&workspace.0, &referee.path, &bytes);
    referee.sha256 = sha256_hex(&bytes);
    bundle.checks[0].code_sha256 = Some(referee.sha256.clone());
    failure(&workspace.0, &bundle, "referee_independence");
}

#[test]
fn false_verdict_cannot_close_a_route_without_a_retained_counterexample() {
    let (workspace, mut bundle) = fixture();
    update_report(&workspace.0, &mut bundle.referee, |report| {
        report["verdicts"][0]["verdict"] = json!("FALSE");
        report["verdicts"][0]["reason"] =
            json!("an independently reproduced counterexample was found");
        report["verdicts"][0]["corrections"] = json!([]);
    });
    bundle.corrections.clear();
    bundle.applied_corrections.clear();
    bundle.claims[0]["statement"] = json!("all tested cases pass");
    bundle.claims[0]["status"] = json!("REFUTED");
    bundle.claims[0]["lesson"] =
        json!("the claimed universal statement fails in the degenerate case");
    bundle.claims[0]["referee"]["verdict"] = json!("FALSE");
    let claim = bundle.claims[0].clone();
    update_writer(&workspace.0, &mut bundle, |report| {
        report["claims"] = json!([claim]);
        report["applied_corrections"] = json!([]);
    });
    failure(&workspace.0, &bundle, "claim_artifact");
}

#[test]
fn changed_canonical_source_and_nonunique_writer_anchors_never_partially_install() {
    let (workspace, bundle) = fixture();
    write(&workspace.0, "notes.md", b"human changes\n");
    failure(&workspace.0, &bundle, "stale_writer_sources");
    let (workspace, mut bundle) = fixture();
    let mut knowledge = json_file(&workspace.0, "labyrinth/knowledge.json").unwrap();
    knowledge["meta"]["human_edit"] = json!(true);
    write_json(&workspace.0, "labyrinth/knowledge.json", &knowledge);
    failure(&workspace.0, &bundle, "stale_canonical_sources");
    bundle.edits[0].old = "not present".into();
    update_writer(&workspace.0, &mut bundle, |v| {
        v["edits"][0]["old"] = json!("not present")
    });
    let sources = &mut bundle.canonical_sources;
    sources[0].sha256 = Some(sha256_hex(
        &read(&workspace.0, "labyrinth/knowledge.json", MAX_FILE).unwrap(),
    ));
    let bytes = write_json(
        &workspace.0,
        &bundle.canonical_sources_path,
        &serde_json::to_value(sources).unwrap(),
    );
    bundle.canonical_sources_sha256 = sha256_hex(&bytes);
    failure(&workspace.0, &bundle, "edit_anchor");
}

#[test]
fn stale_writer_receipts_and_unapplied_referee_corrections_are_rejected_before_any_edit() {
    for mutation in 0..4 {
        let (workspace, mut bundle) = fixture();
        let code = match mutation {
            0 => {
                bundle.checks[2].fresh = false;
                "check_failed"
            }
            1 => {
                bundle.checks[2].source_sha256 = "0".repeat(64);
                "stale_check"
            }
            2 => {
                bundle.applied_corrections.clear();
                "corrections_missing"
            }
            _ => {
                bundle.claims[0]["statement"] = json!("all tested cases pass");
                "writer_drift"
            }
        };
        failure(&workspace.0, &bundle, code);
    }
}

#[test]
fn independent_referee_identity_and_retained_check_output_are_required() {
    let (workspace, mut bundle) = fixture();
    bundle.referee.id = bundle.author.id.clone();
    failure(&workspace.0, &bundle, "referee_independence");
    let (workspace, bundle) = fixture();
    std::fs::remove_file(workspace.0.join(&bundle.checks[0].output_path)).unwrap();
    failure(&workspace.0, &bundle, "artifact_read");
    let (workspace, mut bundle) = fixture();
    bundle.checks[0].code_path = None;
    bundle.checks[0].code_sha256 = None;
    failure(&workspace.0, &bundle, "referee_independence");
}

#[test]
fn reviewed_evidence_cannot_silently_become_t1_t2_or_t3() {
    for tier in ["T1", "T2", "T3"] {
        let (workspace, mut bundle) = fixture();
        bundle.nodes[0]["tier"] = json!(tier);
        bundle.nodes[0]["kind"] = json!(if tier == "T3" {
            "exhaustive"
        } else {
            "theorem"
        });
        bundle.nodes[0]["status"] = json!("established");
        let nodes = bundle.nodes.clone();
        update_writer(&workspace.0, &mut bundle, |v| v["nodes"] = json!(nodes));
        failure(&workspace.0, &bundle, "tier_discipline");
    }
}

#[test]
fn numerical_frontier_uses_exact_counts_and_rejects_contradictions_or_stale_sota_bounds() {
    let frontier = json!({"sizes":[{"size":100,"lo":0,"hi":1_000_000_000_000i64,"step":2,
        "realized":[0],"intervals":[{"v0":2,"v1":998,"status":"impossible","why":"th.bound"},
        {"v0":1000,"v1":2000,"status":"gap-conditional","why":"named hypothesis"}]}]});
    let status = frontier_status(&frontier, None).unwrap();
    assert_eq!(status["sizes"][0]["admissible"], 500_000_000_001u64);
    assert_eq!(status["sizes"][0]["resolved"], 500);
    assert_eq!(status["sizes"][0]["conditional_or_conjectural"], 501);
    let mut contradiction = frontier.clone();
    contradiction["sizes"][0]["realized"] = json!([2]);
    assert!(
        frontier_status(&contradiction, None)
            .unwrap_err()
            .contains("frontier_contradiction")
    );
    contradiction["sizes"][0]["realized"] = json!([3]);
    assert!(
        frontier_status(&contradiction, None)
            .unwrap_err()
            .contains("frontier_grid")
    );
    let (workspace, mut bundle) = fixture();
    bundle.frontier = Some(json!({"sizes":[{"size":6,"realized":[1],"bounds":{"upper":1}}]}));
    let frontier = bundle.frontier.clone().unwrap();
    update_writer(&workspace.0, &mut bundle, |v| v["frontier"] = frontier);
    failure(&workspace.0, &bundle, "sota_frontier");
}

#[test]
fn t4_or_hunch_evidence_cannot_classify_a_new_impossible_range() {
    let (workspace, mut bundle) = fixture();
    bundle.frontier = Some(json!({"sizes":[{"size":6,"realized":[1],"intervals":[{
        "v0":2,"v1":2,"status":"impossible","why":"h.retained"}]}]}));
    let frontier = bundle.frontier.clone().unwrap();
    update_writer(&workspace.0, &mut bundle, |v| v["frontier"] = frontier);
    failure(&workspace.0, &bundle, "frontier_provenance");
}

#[test]
fn unrelated_frontier_updates_preserve_existing_authored_impossible_points() {
    let (workspace, mut bundle) = fixture();
    let mut frontier = json_file(&workspace.0, "labyrinth/frontier.json").unwrap();
    frontier["sizes"][0]["impossible"] = json!([2]);
    write_json(&workspace.0, "labyrinth/frontier.json", &frontier);
    bundle.canonical_sources[2].sha256 = Some(sha256_hex(
        &read(&workspace.0, "labyrinth/frontier.json", MAX_FILE).unwrap(),
    ));
    let sources = write_json(
        &workspace.0,
        &bundle.canonical_sources_path,
        &serde_json::to_value(&bundle.canonical_sources).unwrap(),
    );
    bundle.canonical_sources_sha256 = sha256_hex(&sources);
    refresh_preview(&workspace.0, &mut bundle);
    integrate(&workspace.0, &bundle, &AtomicBool::new(false)).unwrap();
    let frontier = json_file(&workspace.0, "labyrinth/frontier.json").unwrap();
    assert_eq!(frontier["sizes"][0]["impossible"], json!([2]));
    assert_eq!(
        check(&workspace.0).unwrap()["frontier"]["sizes"][0]["resolved"],
        2
    );
}

#[test]
fn event_journal_is_appended_during_work_and_report_archive_preserves_verbatim_bytes() {
    let (workspace, bundle) = fixture();
    let before = canonical(&workspace.0);
    let event =
        json!({"type":"question","summary":"opened a door","nodes":["ev.claim"],"evidence":[]});
    ArtifactIntegrator.event(&workspace.0, &event).unwrap();
    let first = read(&workspace.0, EVENTS, MAX_EVENTS).unwrap();
    append_event(&workspace.0,&json!({"type":"computed","summary":"sample checked (under review)","nodes":[],"evidence":[]})).unwrap();
    assert!(
        read(&workspace.0, EVENTS, MAX_EVENTS)
            .unwrap()
            .starts_with(&first)
    );
    assert_eq!(canonical(&workspace.0), before);
    assert_eq!(check(&workspace.0).unwrap()["events"], 2);
    let source = Workspace::new();
    write(&source.0, "own-code.py", b"print('owned code')\n");
    let report = b"Verbatim report.\n  Keep this whitespace.\n";
    let archive = archive_report(
        &workspace.0,
        &bundle.campaign_id,
        "extra-literature",
        "literature",
        &source.0,
        report,
        &["own-code.py".into()],
    )
    .unwrap();
    assert_eq!(
        read(
            &workspace.0,
            archive["report_path"].as_str().unwrap(),
            MAX_FILE
        )
        .unwrap(),
        report
    );
    assert_eq!(archive["report_sha256"], sha256_hex(report));
    assert_eq!(canonical(&workspace.0), before);
}

fn interrupted(root: &Path, independent_edit: bool) -> (String, Vec<u8>) {
    private(root).unwrap();
    let token = "a".repeat(64);
    let before = read(root, "notes.md", MAX_FILE).unwrap();
    let journal = json!({"schema":1,"id":token,"campaign":"campaign","changes":[
        {"path":"notes.md","before":std::str::from_utf8(&before).unwrap(),"after":"partly integrated\n"},
        {"path":"new-note.md","before":null,"after":"new reviewed note\n"}]});
    let prepared = serde_json::to_vec(&journal).unwrap();
    write(
        root,
        &format!("{TRANSACTIONS}/{token}.prepared.json"),
        &prepared,
    );
    write(
        root,
        "notes.md",
        if independent_edit {
            b"independent human edit\n"
        } else {
            b"partly integrated\n"
        },
    );
    write(root, "new-note.md", b"new reviewed note\n");
    (token, before)
}

#[test]
fn interrupted_batch_is_inert_until_exact_recovery_restores_old_files_and_removes_new_ones() {
    let (workspace, _) = fixture();
    let (token, before) = interrupted(&workspace.0, false);
    assert!(
        check(&workspace.0)
            .unwrap_err()
            .contains("integration_pending")
    );
    assert!(
        append_event(
            &workspace.0,
            &json!({"type":"monitor","summary":"unsafe pending append","nodes":[],"evidence":[]})
        )
        .unwrap_err()
        .contains("integration_pending")
    );
    let recovered = recover(&workspace.0).unwrap();
    assert_eq!(recovered["recovered"], json!([token]));
    assert_eq!(read(&workspace.0, "notes.md", MAX_FILE).unwrap(), before);
    assert!(!workspace.0.join("new-note.md").exists());
    ensure_ready(&workspace.0).unwrap();
    check(&workspace.0).unwrap();
}

#[test]
fn recovery_conflicts_preserve_all_independent_edits_and_keep_the_map_inert() {
    let (workspace, _) = fixture();
    interrupted(&workspace.0, true);
    let before = read(&workspace.0, "new-note.md", MAX_FILE).unwrap();
    assert!(
        recover(&workspace.0)
            .unwrap_err()
            .contains("recovery_conflict")
    );
    assert_eq!(
        read(&workspace.0, "notes.md", MAX_FILE).unwrap(),
        b"independent human edit\n"
    );
    assert_eq!(read(&workspace.0, "new-note.md", MAX_FILE).unwrap(), before);
    assert!(ensure_ready(&workspace.0).is_err());
}

#[test]
fn cooperating_readers_hold_a_shared_lease_and_refuse_active_canonical_writers() {
    let (workspace, bundle) = fixture();
    assert!(read_lease(&workspace.0).unwrap().is_none());
    let writer = Lease::acquire(&workspace.0).unwrap();
    assert!(
        read_lease(&workspace.0)
            .unwrap_err()
            .contains("workflow_busy")
    );
    drop(writer);
    let reader = read_lease(&workspace.0).unwrap().unwrap();
    assert!(
        integrate(&workspace.0, &bundle, &AtomicBool::new(false))
            .unwrap_err()
            .contains("workflow_busy")
    );
    drop(reader);
    integrate(&workspace.0, &bundle, &AtomicBool::new(false)).unwrap();
}

#[cfg(unix)]
#[test]
fn aliased_check_evidence_and_document_paths_are_never_followed() {
    let (workspace, bundle) = fixture();
    let output = workspace.0.join(&bundle.checks[0].output_path);
    std::fs::remove_file(&output).unwrap();
    std::os::unix::fs::symlink(workspace.0.join(&bundle.checks[1].output_path), output).unwrap();
    failure(&workspace.0, &bundle, "artifact_read");
    let (workspace, bundle) = fixture();
    write(&workspace.0, "other.md", b"all tested cases pass\n");
    std::fs::remove_file(workspace.0.join("notes.md")).unwrap();
    std::os::unix::fs::symlink(workspace.0.join("other.md"), workspace.0.join("notes.md")).unwrap();
    assert!(
        integrate(&workspace.0, &bundle, &AtomicBool::new(false))
            .unwrap_err()
            .contains("artifact_read")
    );
    assert_eq!(
        read(&workspace.0, "other.md", MAX_FILE).unwrap(),
        b"all tested cases pass\n"
    );
}

#[test]
fn lexical_aliases_cannot_reach_private_or_generated_files_through_writer_edits() {
    for path in [
        "labyrinth//angel/campaigns/other/agents/x/report.md",
        "labyrinth/./angel/campaigns/other/agents/x/report.md",
        "labyrinth//knowledge.json",
        "labyrinth/./sota.json",
        "./notes.md",
        "notes.md/",
        "docs//notes.md",
    ] {
        assert!(canonical_path(path).is_err(), "accepted alias: {path}");
    }
    assert!(canonical_path("docs/notes.md").is_ok());
    assert!(canonical_path("labyrinth/angel/campaigns/other/agents/x/report.md").is_err());
}
