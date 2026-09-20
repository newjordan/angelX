use super::controller::{CampaignCommand, CampaignController};
use super::lens;
use super::schema::*;
use super::store::{CampaignStore, LoadedCampaign};
use crate::agent::club::{ChatMsg, ChatRole};
use crate::agent::harness::{
    AlignmentIndependence, AlignmentVerdict, CampaignAlignmentReceipt, CampaignBase,
    PreparedSwarmRun, SwarmRunOutcome, SwarmRunReceipt,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn scratch(label: &str) -> (PathBuf, PathBuf, PathBuf) {
    let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!(
        "angel-campaign-{label}-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    let workspace = base.join("workspace");
    let store = base.join("store");
    std::fs::create_dir_all(&workspace).unwrap();
    (base, workspace, store)
}

fn verifier(id: CriterionId) -> VerifierSpec {
    VerifierSpec {
        id: format!("{id}-v1"),
        kind: VerifierKind::Command,
        command: "cargo test --workspace".to_string(),
        network: NetworkPolicy::Offline,
        timeout_secs: 900,
    }
}

fn ready_record(workspace: &Path) -> CampaignRecord {
    let mut record = CampaignRecord::draft(
        "cmp-test-1".to_string(),
        workspace,
        "Ship proof".to_string(),
        1,
    );
    let id = CriterionId::new(1).unwrap();
    let mut criterion = AcceptanceCriterion::pending(id, "Tests pass".to_string());
    criterion.verifiers.push(verifier(id));
    criterion.test_scope.push(PathBuf::from("cockpit/src"));
    record.criteria.push(criterion);
    record.transition(CampaignStatus::Ready, 2).unwrap();
    record
}

fn reviewing_controller(
    workspace: &Path,
    root: PathBuf,
    record: CampaignRecord,
) -> CampaignController {
    let binding = record.project.clone();
    let store = CampaignStore::in_root(workspace, root.clone());
    let lease = store.acquire_lease().unwrap();
    store.save(&record).unwrap();
    drop(lease);
    let mut controller = CampaignController::open_in(workspace, root);
    let base_oid = "ba5e0123456789".to_string();
    let launch = controller
        .freeze_round(CampaignBase {
            workspace: workspace.canonicalize().unwrap(),
            repo_root: binding.canonical_root,
            workspace_rel: binding.workspace_rel.to_string_lossy().into_owned(),
            base_oid: base_oid.clone(),
        })
        .unwrap();
    let prepared = PreparedSwarmRun {
        run_id: "swr-campaign-review-test".to_string(),
        base_oid: base_oid.clone(),
    };
    controller
        .attach_prepared(&prepared, &launch.authorization)
        .unwrap();
    controller
        .finish_round(SwarmRunReceipt {
            run_id: prepared.run_id,
            outcome: SwarmRunOutcome::Verified,
            base_oid,
            parked_branch: Some("angel/swarm/review-candidate".to_string()),
            candidate_oid: Some("caad1da7e012345".to_string()),
            changed_paths: vec!["cockpit/src/drive/campaign/mod.rs".to_string()],
            technical_pass: true,
            code_review_pass: true,
            proof_path: PathBuf::from("/external/swarm/run.json"),
            error: None,
        })
        .unwrap();
    controller
}

fn alignment_receipt(
    request: &crate::agent::harness::CampaignAlignmentRequest,
    verdict: AlignmentVerdict,
    independence: AlignmentIndependence,
) -> CampaignAlignmentReceipt {
    CampaignAlignmentReceipt {
        schema: "campaign-review/v1".to_string(),
        verdict,
        reviewer_route: "alignment-reviewer".to_string(),
        reviewer_model_revision: "review-v2".to_string(),
        implementer_route: "implementer".to_string(),
        implementer_model_revision: "impl-v1".to_string(),
        technical_reviewer_route: "technical-reviewer".to_string(),
        technical_reviewer_model_revision: "tech-v1".to_string(),
        technical_review_independence: AlignmentIndependence::DifferentRouteAndRevision,
        independence,
        reviewed_contract_digest: request.contract_digest.clone(),
        reviewed_candidate_oid: request.candidate_oid.clone(),
        cited_criteria: request
            .criteria
            .iter()
            .map(|criterion| criterion.id.clone())
            .collect(),
        cited_proof_sha256: request
            .proofs
            .iter()
            .map(|proof| proof.sha256.clone())
            .collect(),
        summary: "candidate satisfies the exact criterion".to_string(),
        response_sha256: "ef".repeat(32),
    }
}

#[test]
fn criterion_ids_are_canonical_and_bounded() {
    assert_eq!(CriterionId::parse("ac-2").unwrap().to_string(), "AC-2");
    assert!(CriterionId::parse("2").is_err());
    assert!(CriterionId::parse("AC-0").is_err());
    assert!(CriterionId::parse("AC-65").is_err());

    let encoded = serde_json::to_string(&CriterionId::new(3).unwrap()).unwrap();
    assert_eq!(encoded, "\"AC-3\"");
    assert_eq!(
        serde_json::from_str::<CriterionId>("\"ac-3\"").unwrap(),
        CriterionId::new(3).unwrap()
    );
}

#[test]
fn campaign_transitions_and_contract_validation_fail_closed() {
    let (base, workspace, _) = scratch("transitions");
    let mut record = ready_record(&workspace);
    assert!(!CampaignStatus::Draft.allows(CampaignStatus::Running));
    assert!(CampaignStatus::Ready.allows(CampaignStatus::Running));
    assert!(!CampaignStatus::Integrated.allows(CampaignStatus::Running));

    record.transition(CampaignStatus::Running, 3).unwrap();
    record.paused_from = Some(CampaignStatus::Running);
    record.transition(CampaignStatus::Paused, 4).unwrap();
    record.paused_from = None;
    record.transition(CampaignStatus::Running, 5).unwrap();

    let mut contract = RoundContract {
        schema: ROUND_SCHEMA.to_string(),
        campaign_id: record.id.clone(),
        campaign_revision: record.revision,
        round: 1,
        base_oid: "0123456789abcdef".to_string(),
        objective: "Prove AC-1".to_string(),
        target_criteria: vec![CriterionId::new(1).unwrap()],
        targeted_test_cmd: "cargo test".to_string(),
        accept_cmd: "cargo test --workspace".to_string(),
        quality_cmds: vec!["cargo fmt --check".to_string()],
        test_scope: vec![PathBuf::from("cockpit/src")],
        network_policy: NetworkPolicy::Offline,
        digest: String::new(),
    };
    contract.digest = digest_round_contract(&contract).unwrap();
    record.active_contract = Some(contract.clone());
    record.validate().unwrap();
    record.active_contract.as_mut().unwrap().digest = "tampered".to_string();
    assert!(record.validate().unwrap_err().contains("digest mismatch"));

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn unknown_schema_fields_and_duplicate_criteria_are_rejected() {
    let (base, workspace, _) = scratch("schema");
    let mut record = ready_record(&workspace);
    let mut json = serde_json::to_value(&record).unwrap();
    json.as_object_mut()
        .unwrap()
        .insert("future_authority".to_string(), serde_json::json!(true));
    assert!(serde_json::from_value::<CampaignRecord>(json).is_err());

    record.status = CampaignStatus::Draft;
    record.criteria.push(record.criteria[0].clone());
    assert!(
        record
            .validate()
            .unwrap_err()
            .contains("duplicate criterion")
    );
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn command_parser_is_exact_and_does_not_fuzz_destructive_verbs() {
    assert_eq!(
        CampaignCommand::parse(None).unwrap(),
        CampaignCommand::Status
    );
    assert_eq!(
        CampaignCommand::parse(Some("new ship proofs")).unwrap(),
        CampaignCommand::New("ship proofs".to_string())
    );
    assert_eq!(
        CampaignCommand::parse(Some("criterion require ac-2")).unwrap(),
        CampaignCommand::CriterionRequire(CriterionId::new(2).unwrap())
    );
    assert_eq!(
        CampaignCommand::parse(Some("criterion verify AC-1 -- cargo test -q")).unwrap(),
        CampaignCommand::CriterionVerify {
            id: CriterionId::new(1).unwrap(),
            command: "cargo test -q".to_string()
        }
    );
    assert_eq!(
        CampaignCommand::parse(Some("advance")).unwrap(),
        CampaignCommand::Advance
    );
    assert_eq!(
        CampaignCommand::parse(Some("review")).unwrap(),
        CampaignCommand::Review
    );
    assert!(CampaignCommand::parse(Some("done")).is_err());
    assert!(CampaignCommand::parse(Some("criterion verify AC-1 cargo test")).is_err());
    assert!(CampaignCommand::parse(Some("abandon yes")).is_err());
}

#[test]
fn independent_alignment_accepts_and_parks_without_integration() {
    let (base, workspace, root) = scratch("alignment-pass");
    let mut record = ready_record(&workspace);
    record.status = CampaignStatus::Draft;
    let id = CriterionId::new(2).unwrap();
    let mut second = AcceptanceCriterion::pending(id, "Second proof passes".to_string());
    second.verifiers.push(verifier(id));
    second.test_scope.push(PathBuf::from("cockpit/tests"));
    record.criteria.push(second);
    record.status = CampaignStatus::Ready;
    record.validate().unwrap();
    let mut controller = reviewing_controller(&workspace, root, record);

    let request = controller.begin_alignment_review().unwrap();
    let message = controller
        .finish_alignment_review(alignment_receipt(
            &request,
            AlignmentVerdict::Pass,
            AlignmentIndependence::DifferentRouteAndRevision,
        ))
        .unwrap();
    assert!(message.contains("parked at Ready"));
    let record = controller.record().unwrap();
    assert_eq!(record.status, CampaignStatus::Ready);
    assert_eq!(record.criteria[0].status, CriterionStatus::Verified);
    assert_eq!(record.criteria[1].status, CriterionStatus::Pending);
    assert_eq!(record.rounds[0].state, RoundState::Accepted);
    assert!(record.rounds[0].code_review.is_some());
    assert!(record.rounds[0].alignment_review.is_some());
    assert_eq!(
        record.rounds[0]
            .proofs
            .iter()
            .filter(|proof| proof.kind == ProofKind::AlignmentReview)
            .count(),
        1
    );
    assert_eq!(record.frozen_base_oid.as_deref(), Some("ba5e0123456789"));
    assert_eq!(record.campaign_head_oid.as_deref(), Some("caad1da7e012345"));
    assert!(record.campaign_ref.is_none());

    let binding = record.project.clone();
    let next = controller
        .freeze_round(CampaignBase {
            workspace: workspace.canonicalize().unwrap(),
            repo_root: binding.canonical_root,
            workspace_rel: binding.workspace_rel.to_string_lossy().into_owned(),
            base_oid: "f00d0123456789".to_string(),
        })
        .unwrap();
    assert_eq!(
        next.authorization.base.base_oid, "caad1da7e012345",
        "the next round must compose from the accepted logical head"
    );
    assert_eq!(
        controller.record().unwrap().frozen_base_oid.as_deref(),
        Some("ba5e0123456789"),
        "starting a cumulative round must not rewrite the campaign origin"
    );
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn final_alignment_pass_stops_at_ready_to_integrate() {
    let (base, workspace, root) = scratch("alignment-final");
    let mut controller = reviewing_controller(&workspace, root, ready_record(&workspace));
    let request = controller.begin_alignment_review().unwrap();
    controller
        .finish_alignment_review(alignment_receipt(
            &request,
            AlignmentVerdict::Pass,
            AlignmentIndependence::DifferentRouteAndRevision,
        ))
        .unwrap();
    let record = controller.record().unwrap();
    assert_eq!(record.status, CampaignStatus::ReadyToIntegrate);
    assert_eq!(record.rounds[0].state, RoundState::Accepted);
    assert_eq!(
        record.rounds[0].candidate_branch.as_deref(),
        Some("angel/swarm/review-candidate")
    );
    assert!(record.campaign_ref.is_none());
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn same_route_or_mismatched_alignment_blocks_and_can_retry() {
    let (base, workspace, root) = scratch("alignment-block");
    let mut controller = reviewing_controller(&workspace, root, ready_record(&workspace));
    let request = controller.begin_alignment_review().unwrap();

    let mut mismatch = alignment_receipt(
        &request,
        AlignmentVerdict::Pass,
        AlignmentIndependence::DifferentRouteAndRevision,
    );
    mismatch.reviewed_candidate_oid = "deadbeef".to_string();
    assert!(
        controller
            .finish_alignment_review(mismatch)
            .unwrap_err()
            .contains("different contract or candidate")
    );
    controller
        .fail_alignment_review("candidate binding mismatch")
        .unwrap();
    assert_eq!(
        controller.record().unwrap().rounds[0].state,
        RoundState::Blocked
    );
    assert_eq!(
        controller.record().unwrap().status,
        CampaignStatus::ReviewingRound
    );

    let retry = controller.begin_alignment_review().unwrap();
    let blocked = alignment_receipt(
        &retry,
        AlignmentVerdict::Pass,
        AlignmentIndependence::SameRoute,
    );
    assert!(
        controller
            .finish_alignment_review(blocked)
            .unwrap()
            .contains("remains ReviewingRound")
    );
    assert_eq!(
        controller.record().unwrap().rounds[0].state,
        RoundState::Blocked
    );
    assert_eq!(
        controller.record().unwrap().criteria[0].status,
        CriterionStatus::TechnicallyVerified
    );
    assert!(controller.begin_alignment_review().is_ok());
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn one_round_freezes_base_attaches_proof_and_stops_before_alignment() {
    let (base, workspace, root) = scratch("technical-round");
    let store = CampaignStore::in_root(&workspace, root.clone());
    let record = ready_record(&workspace);
    let binding = record.project.clone();
    let _lease = store.acquire_lease().unwrap();
    store.save(&record).unwrap();
    drop(_lease);

    let mut controller = CampaignController::open_in(&workspace, root);
    let frozen_oid = "ba5e0123456789".to_string();
    let launch = controller
        .freeze_round(CampaignBase {
            workspace: workspace.canonicalize().unwrap(),
            repo_root: binding.canonical_root.clone(),
            workspace_rel: binding.workspace_rel.to_string_lossy().into_owned(),
            base_oid: frozen_oid.clone(),
        })
        .unwrap();
    assert_eq!(
        controller.record().unwrap().status,
        CampaignStatus::VerifyingRound
    );
    assert_eq!(launch.authorization.base.base_oid, frozen_oid);
    assert_eq!(launch.request.test_scope, vec!["cockpit/src"]);

    let prepared = PreparedSwarmRun {
        run_id: "swr-campaign-test".to_string(),
        base_oid: frozen_oid.clone(),
    };
    controller
        .attach_prepared(&prepared, &launch.authorization)
        .unwrap();
    controller
        .attach_prepared(&prepared, &launch.authorization)
        .unwrap();
    assert_eq!(controller.record().unwrap().rounds.len(), 1);

    let message = controller
        .finish_round(SwarmRunReceipt {
            run_id: prepared.run_id,
            outcome: SwarmRunOutcome::Verified,
            base_oid: frozen_oid.clone(),
            parked_branch: Some("angel/swarm/candidate".to_string()),
            candidate_oid: Some("caad1da7e012345".to_string()),
            changed_paths: vec!["cockpit/src/drive/campaign/mod.rs".to_string()],
            technical_pass: true,
            code_review_pass: true,
            proof_path: PathBuf::from("/external/swarm/run.json"),
            error: None,
        })
        .unwrap();
    assert!(message.contains("alignment review remains pending"));
    let record = controller.record().unwrap();
    assert_eq!(record.status, CampaignStatus::ReviewingRound);
    assert_eq!(
        record.criteria[0].status,
        CriterionStatus::TechnicallyVerified
    );
    assert_eq!(
        record.campaign_head_oid.as_deref(),
        Some(frozen_oid.as_str())
    );
    assert_eq!(record.rounds[0].state, RoundState::TechnicallyVerified);
    assert!(record.rounds[0].alignment_review.is_none());
    assert!(record.rounds[0].code_review.is_none());
    assert_eq!(record.rounds[0].proofs[0].sha256.len(), 64);
    assert!(
        controller
            .record_path()
            .parent()
            .unwrap()
            .join(&record.rounds[0].proofs[0].artifact_path)
            .is_file()
    );
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn active_round_error_is_durably_paused_for_explicit_resume() {
    let (base, workspace, root) = scratch("round-pause");
    let store = CampaignStore::in_root(&workspace, root.clone());
    let record = ready_record(&workspace);
    let binding = record.project.clone();
    let _lease = store.acquire_lease().unwrap();
    store.save(&record).unwrap();
    drop(_lease);

    let mut controller = CampaignController::open_in(&workspace, root.clone());
    controller
        .freeze_round(CampaignBase {
            workspace: workspace.canonicalize().unwrap(),
            repo_root: binding.canonical_root.clone(),
            workspace_rel: binding.workspace_rel.to_string_lossy().into_owned(),
            base_oid: "ba5e0123456789".to_string(),
        })
        .unwrap();
    controller
        .pause_execution("synthetic provider outage")
        .unwrap();
    assert_eq!(controller.record().unwrap().status, CampaignStatus::Paused);
    assert_eq!(
        controller.record().unwrap().paused_from,
        Some(CampaignStatus::VerifyingRound)
    );
    assert!(
        controller
            .command(Some("resume"), &workspace, None)
            .contains("campaign resumed")
    );
    let resumed = controller
        .freeze_round(CampaignBase {
            workspace: workspace.canonicalize().unwrap(),
            repo_root: binding.canonical_root,
            workspace_rel: binding.workspace_rel.to_string_lossy().into_owned(),
            base_oid: "caad1da7e012345".to_string(),
        })
        .unwrap();
    assert_eq!(
        resumed.authorization.base.base_oid, "ba5e0123456789",
        "a live branch advance must not move the campaign's frozen base"
    );

    let reopened = CampaignController::open_in(&workspace, root);
    assert_eq!(reopened.record().unwrap().status, CampaignStatus::Paused);
    assert!(
        reopened
            .startup_notice()
            .unwrap()
            .contains("saved campaign")
    );
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn authoring_persists_atomically_and_reopens_ready() {
    let (base, workspace, root) = scratch("persistence");
    let mut controller = CampaignController::open_in(&workspace, root.clone());
    assert!(
        controller
            .command(Some("new ship campaign state"), &workspace, None)
            .contains("campaign created")
    );
    assert!(
        controller
            .command(
                Some("criterion add the campaign unit tests pass"),
                &workspace,
                None
            )
            .contains("added AC-1")
    );
    assert!(
        controller
            .command(
                Some("criterion verify AC-1 -- cargo test campaign::tests"),
                &workspace,
                None
            )
            .contains("added verifier")
    );
    assert!(
        controller
            .command(
                Some("criterion scope AC-1 cockpit/src/drive/campaign"),
                &workspace,
                None
            )
            .contains("set scope")
    );
    assert!(
        controller
            .command(Some("start"), &workspace, None)
            .contains("campaign is ready")
    );
    assert_eq!(controller.record().unwrap().status, CampaignStatus::Ready);
    assert!(
        std::fs::read_dir(controller.record_path().parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".campaign.json.tmp")),
        "an atomic save must not leave a temp snapshot after publication"
    );

    let reopened = CampaignController::open_in(&workspace, root);
    assert_eq!(reopened.record().unwrap().status, CampaignStatus::Ready);
    assert_eq!(reopened.record().unwrap().criteria.len(), 1);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file_mode = std::fs::metadata(reopened.record_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let dir_mode = std::fs::metadata(reopened.record_path().parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);
        assert_eq!(dir_mode, 0o700);
    }
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn imported_goal_is_copied_without_mutating_goal() {
    let (base, workspace, root) = scratch("goal");
    let mut goal = crate::drive::goal::Goal::new("Ship the goal");
    goal.acceptance.push("The check is green".to_string());
    goal.accept_cmd = Some("cargo test".to_string());
    let identity = crate::platform::workspace_store::repo_identity(&workspace);
    goal.workspace = Some(identity.root);
    goal.project_key = Some(identity.key);
    let original = goal.clone();
    let mut controller = CampaignController::open_in(&workspace, root);
    assert!(
        controller
            .command(Some("import-goal"), &workspace, Some(&goal))
            .contains("imported active goal")
    );
    assert_eq!(goal.text, original.text);
    assert_eq!(goal.acceptance, original.acceptance);
    assert_eq!(
        controller.record().unwrap().criteria[0].verifiers[0].kind,
        VerifierKind::Aggregate
    );
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn foreign_and_corrupt_records_are_inert_and_never_overwritten() {
    let (base, workspace_a, root) = scratch("foreign");
    let workspace_b = base.join("workspace-b");
    std::fs::create_dir_all(&workspace_b).unwrap();

    let store_a = CampaignStore::in_root(&workspace_a, root.clone());
    let record = ready_record(&workspace_a);
    let _lease = store_a.acquire_lease().unwrap();
    store_a.save(&record).unwrap();
    drop(_lease);

    let probe_b = CampaignController::open_in(&workspace_b, root.clone());
    let foreign_path = probe_b.record_path().to_path_buf();
    std::fs::create_dir_all(foreign_path.parent().unwrap()).unwrap();
    std::fs::copy(store_a.record_path(), &foreign_path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&foreign_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut foreign = CampaignController::open_in(&workspace_b, root.clone());
    assert!(foreign.status_text(&workspace_b).contains("inert"));
    let before = std::fs::read(&foreign_path).unwrap();
    assert!(
        foreign
            .command(Some("new overwrite"), &workspace_b, None)
            .contains("inert")
    );
    assert_eq!(std::fs::read(&foreign_path).unwrap(), before);

    std::fs::write(&foreign_path, b"{broken").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&foreign_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let corrupt = CampaignController::open_in(&workspace_b, root);
    assert!(corrupt.status_text(&workspace_b).contains("inert"));
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn interrupted_campaign_reopens_paused_and_requires_operator_resume() {
    let (base, workspace, root) = scratch("recovery");
    let store = CampaignStore::in_root(&workspace, root.clone());
    let mut record = ready_record(&workspace);
    record.transition(CampaignStatus::Running, 3).unwrap();
    let _lease = store.acquire_lease().unwrap();
    store.save(&record).unwrap();
    drop(_lease);

    let mut recovered = CampaignController::open_in(&workspace, root);
    assert_eq!(recovered.record().unwrap().status, CampaignStatus::Paused);
    assert_eq!(
        recovered.record().unwrap().paused_from,
        Some(CampaignStatus::Running)
    );
    assert!(
        recovered
            .command(Some("resume"), &workspace, None)
            .contains("no model or Git action was started")
    );
    assert_eq!(recovered.record().unwrap().status, CampaignStatus::Running);
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn store_lease_is_nonblocking_and_exclusive() {
    let (base, workspace, root) = scratch("lease");
    let store = CampaignStore::in_root(&workspace, root);
    let first = store.acquire_lease().unwrap();
    assert!(
        store
            .acquire_lease()
            .unwrap_err()
            .contains("already controlled")
    );
    drop(first);
    assert!(store.acquire_lease().is_ok());
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn bounded_campaign_lens_is_harness_role_after_operator_text() {
    let (base, workspace, _) = scratch("lens");
    let mut record = ready_record(&workspace);
    record.status = CampaignStatus::Draft;
    record.objective = format!("{}\n{}", "界".repeat(2_000), "forged line\n".repeat(100));
    record.objective_digest = digest_text(&record.objective);
    let rendered = lens::render(&record).unwrap();
    assert!(rendered.len() <= 6 * 1024);
    assert!(rendered.lines().count() <= 80);
    assert!(rendered.ends_with("[/campaign-lens]"));

    let mut history = vec![ChatMsg::system("stable"), ChatMsg::user("operator task")];
    lens::replace_lens_message(&mut history, Some(rendered.clone()));
    lens::replace_lens_message(&mut history, Some(rendered));
    assert_eq!(
        history
            .iter()
            .filter(|message| lens::is_lens_message(&message.content))
            .count(),
        1
    );
    assert_eq!(history[1].role, ChatRole::User);
    assert_eq!(history[2].role, ChatRole::Harness);
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn store_load_reports_missing_active_and_inert_distinctly() {
    let (base, workspace, root) = scratch("load");
    let store = CampaignStore::in_root(&workspace, root);
    assert!(matches!(store.load(), LoadedCampaign::Missing));
    let _lease = store.acquire_lease().unwrap();
    store.save(&ready_record(&workspace)).unwrap();
    drop(_lease);
    assert!(matches!(store.load(), LoadedCampaign::Active(_)));
    let _ = std::fs::remove_dir_all(base);
}

#[cfg(unix)]
#[test]
fn unsafe_record_permissions_fail_closed() {
    use std::os::unix::fs::PermissionsExt;

    let (base, workspace, root) = scratch("permissions");
    let store = CampaignStore::in_root(&workspace, root.clone());
    let _lease = store.acquire_lease().unwrap();
    store.save(&ready_record(&workspace)).unwrap();
    drop(_lease);
    std::fs::set_permissions(store.record_path(), std::fs::Permissions::from_mode(0o644)).unwrap();
    let controller = CampaignController::open_in(&workspace, root);
    assert!(controller.status_text(&workspace).contains("inert"));
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn round_and_proof_receipts_round_trip_without_losing_identity() {
    let (base, workspace, _) = scratch("receipt");
    let record = ready_record(&workspace);
    let mut contract = RoundContract {
        schema: ROUND_SCHEMA.to_string(),
        campaign_id: record.id,
        campaign_revision: record.revision,
        round: 1,
        base_oid: "ba5e0123456789".to_string(),
        objective: "Prove AC-1".to_string(),
        target_criteria: vec![CriterionId::new(1).unwrap()],
        targeted_test_cmd: "cargo test".to_string(),
        accept_cmd: "cargo test".to_string(),
        quality_cmds: vec![],
        test_scope: vec![PathBuf::from("cockpit/src/drive/campaign")],
        network_policy: NetworkPolicy::Offline,
        digest: String::new(),
    };
    contract.digest = digest_round_contract(&contract).unwrap();
    let receipt = RoundReceipt {
        contract,
        state: RoundState::TechnicallyVerified,
        swarm_run_id: Some("swr-test".to_string()),
        candidate_branch: Some("angel/candidate".to_string()),
        candidate_oid: Some("caad1da7e012345".to_string()),
        changed_paths: vec![PathBuf::from("cockpit/src/drive/campaign/mod.rs")],
        proofs: vec![ProofRef {
            kind: ProofKind::TargetedTest,
            run_id: "swr-test".to_string(),
            artifact_path: PathBuf::from("proofs/targeted.json"),
            sha256: "0123456789abcdef".repeat(4),
            git_oid: "caad1da7e012345".to_string(),
            summary: "targeted verifier passed".to_string(),
        }],
        code_review: None,
        alignment_review: None,
        tokens: 42,
        elapsed_ms: 500,
        failure: None,
    };
    let round_trip: RoundReceipt =
        serde_json::from_slice(&serde_json::to_vec(&receipt).unwrap()).unwrap();
    assert_eq!(round_trip, receipt);
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn round_contract_digest_is_sha256_and_covers_every_field() {
    let (base, workspace, _) = scratch("contract-digest");
    let record = ready_record(&workspace);
    let mut contract = RoundContract {
        schema: ROUND_SCHEMA.to_string(),
        campaign_id: record.id,
        campaign_revision: record.revision,
        round: 1,
        base_oid: "ba5e0123456789".to_string(),
        objective: "Prove AC-1".to_string(),
        target_criteria: vec![CriterionId::new(1).unwrap()],
        targeted_test_cmd: "cargo test".to_string(),
        accept_cmd: "cargo test".to_string(),
        quality_cmds: vec!["cargo fmt --check".to_string()],
        test_scope: vec![PathBuf::from("cockpit/src/drive/campaign")],
        network_policy: NetworkPolicy::Offline,
        digest: String::new(),
    };
    let digest = digest_round_contract(&contract).unwrap();
    assert_eq!(digest.len(), 64);
    assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    contract.quality_cmds.push("cargo clippy".to_string());
    assert_ne!(digest_round_contract(&contract).unwrap(), digest);
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn accepted_round_requires_matching_independent_reviews() {
    let (base, workspace, _) = scratch("accepted-review");
    let mut record = ready_record(&workspace);
    let mut contract = RoundContract {
        schema: ROUND_SCHEMA.to_string(),
        campaign_id: record.id.clone(),
        campaign_revision: record.revision,
        round: 1,
        base_oid: "ba5e0123456789".to_string(),
        objective: "Prove AC-1".to_string(),
        target_criteria: vec![CriterionId::new(1).unwrap()],
        targeted_test_cmd: "cargo test".to_string(),
        accept_cmd: "cargo test".to_string(),
        quality_cmds: vec![],
        test_scope: vec![PathBuf::from("cockpit/src/drive/campaign")],
        network_policy: NetworkPolicy::Offline,
        digest: String::new(),
    };
    contract.digest = digest_round_contract(&contract).unwrap();
    let candidate_oid = "caad1da7e012345".to_string();
    let review = ReviewReceipt {
        route: "reviewer".to_string(),
        model_revision: "review-v1".to_string(),
        independence: ReviewIndependence::DifferentRouteAndRevision,
        verdict: ReviewVerdict::Pass,
        reviewed_contract_digest: contract.digest.clone(),
        reviewed_candidate_oid: candidate_oid.clone(),
    };
    record.rounds.push(RoundReceipt {
        contract,
        state: RoundState::Accepted,
        swarm_run_id: Some("swr-test".to_string()),
        candidate_branch: Some("angel/candidate".to_string()),
        candidate_oid: Some(candidate_oid),
        changed_paths: vec![PathBuf::from("cockpit/src/drive/campaign/mod.rs")],
        proofs: vec![ProofRef {
            kind: ProofKind::TargetedTest,
            run_id: "swr-test".to_string(),
            artifact_path: PathBuf::from("proofs/targeted.json"),
            sha256: "0123456789abcdef".repeat(4),
            git_oid: "caad1da7e012345".to_string(),
            summary: "targeted verifier passed".to_string(),
        }],
        code_review: Some(review.clone()),
        alignment_review: Some(review),
        tokens: 42,
        elapsed_ms: 500,
        failure: None,
    });
    record.validate().unwrap();
    record.rounds[0]
        .alignment_review
        .as_mut()
        .unwrap()
        .independence = ReviewIndependence::SameRoute;
    assert!(
        record
            .validate()
            .unwrap_err()
            .contains("independent alignment reviewer")
    );
    let _ = std::fs::remove_dir_all(base);
}
