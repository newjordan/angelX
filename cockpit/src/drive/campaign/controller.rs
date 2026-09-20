use super::lens;
use super::schema::{
    AcceptanceCriterion, CampaignRecord, CampaignStatus, CriterionId, CriterionStatus,
    ExecutionRequest, MAX_COMMAND_BYTES, MAX_CRITERION_BYTES, MAX_OBJECTIVE_BYTES, NetworkPolicy,
    OperatorWaiver, ProofKind, ProofRef, ROUND_SCHEMA, ReviewIndependence, ReviewReceipt,
    ReviewVerdict, RoundContract, RoundReceipt, RoundState, VerifierKind, VerifierSpec,
    digest_round_contract, validate_bounded,
};
use super::store::{CampaignStore, LoadedCampaign};
use crate::agent::harness::{
    AlignmentCriterion, AlignmentIndependence, AlignmentProof, AlignmentVerdict,
    AuthorizedCampaignBase, CampaignAlignmentReceipt, CampaignAlignmentRequest, CampaignBase,
    CampaignCompileRequest, PreparedSwarmRun, SwarmRunOutcome, SwarmRunReceipt,
};
use crate::drive::goal::Goal;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static CAMPAIGN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CampaignCommand {
    Status,
    New(String),
    ImportGoal,
    CriterionAdd(String),
    CriterionRequire(CriterionId),
    CriterionOptional(CriterionId),
    CriterionVerify {
        id: CriterionId,
        command: String,
    },
    CriterionScope {
        id: CriterionId,
        paths: Vec<PathBuf>,
    },
    Start,
    Advance,
    Review,
    Pause,
    Resume,
    Abandon,
}

impl CampaignCommand {
    pub(crate) fn parse(arg: Option<&str>) -> Result<Self, String> {
        let raw = arg.map(str::trim).unwrap_or("");
        if raw.is_empty() || matches!(raw, "status" | "show" | "report") {
            return Ok(Self::Status);
        }
        if let Some(objective) = raw.strip_prefix("new ") {
            let objective = objective.trim();
            validate_bounded("campaign objective", objective, 1, MAX_OBJECTIVE_BYTES)?;
            return Ok(Self::New(objective.to_string()));
        }
        match raw {
            "new" => return Err("usage: /campaign new <objective>".to_string()),
            "import-goal" | "import goal" => return Ok(Self::ImportGoal),
            "start" => return Ok(Self::Start),
            "advance" => return Ok(Self::Advance),
            "review" => return Ok(Self::Review),
            "pause" => return Ok(Self::Pause),
            "resume" => return Ok(Self::Resume),
            "abandon" | "stop" => return Ok(Self::Abandon),
            _ => {}
        }
        let Some(rest) = raw.strip_prefix("criterion ") else {
            return Err(usage());
        };
        let Some((verb, tail)) = rest.trim().split_once(char::is_whitespace) else {
            return Err(criterion_usage());
        };
        let tail = tail.trim();
        match verb {
            "add" => {
                validate_bounded("criterion", tail, 1, MAX_CRITERION_BYTES)?;
                Ok(Self::CriterionAdd(tail.to_string()))
            }
            "require" => Ok(Self::CriterionRequire(CriterionId::parse(tail)?)),
            "optional" => Ok(Self::CriterionOptional(CriterionId::parse(tail)?)),
            "verify" => {
                let Some((id, command)) = tail.split_once(" -- ") else {
                    return Err("usage: /campaign criterion verify <AC-N> -- <command>".to_string());
                };
                let command = command.trim();
                validate_bounded("verifier command", command, 1, MAX_COMMAND_BYTES)?;
                Ok(Self::CriterionVerify {
                    id: CriterionId::parse(id)?,
                    command: command.to_string(),
                })
            }
            "scope" => {
                let mut words = tail.split_whitespace();
                let id = words
                    .next()
                    .ok_or_else(criterion_usage)
                    .and_then(CriterionId::parse)?;
                let paths = words.map(PathBuf::from).collect::<Vec<_>>();
                if paths.is_empty() {
                    return Err(
                        "usage: /campaign criterion scope <AC-N> <path> [path...]".to_string()
                    );
                }
                Ok(Self::CriterionScope { id, paths })
            }
            _ => Err(criterion_usage()),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CampaignLaunch {
    pub(crate) request: CampaignCompileRequest,
    pub(crate) authorization: AuthorizedCampaignBase,
}

#[derive(Debug)]
pub(crate) struct CampaignController {
    enabled: bool,
    workspace: PathBuf,
    store: CampaignStore,
    record: Option<CampaignRecord>,
    fault: Option<String>,
}

impl CampaignController {
    pub(crate) fn open(workspace: &Path) -> Self {
        if !campaign_enabled() {
            return Self::disabled(workspace);
        }
        let store = CampaignStore::for_workspace(workspace);
        Self::from_store(workspace, store, true)
    }

    pub(crate) fn disabled(workspace: &Path) -> Self {
        let store = CampaignStore::for_workspace(workspace);
        Self {
            enabled: false,
            workspace: workspace.to_path_buf(),
            store,
            record: None,
            fault: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn open_in(workspace: &Path, root: PathBuf) -> Self {
        let store = CampaignStore::in_root(workspace, root);
        Self::from_store(workspace, store, true)
    }

    fn from_store(workspace: &Path, store: CampaignStore, enabled: bool) -> Self {
        let (record, fault) = load_state(&store);
        let mut controller = Self {
            enabled,
            workspace: workspace.to_path_buf(),
            store,
            record,
            fault,
        };
        controller.recover_interrupted();
        controller
    }

    pub(crate) fn startup_notice(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        if let Some(fault) = &self.fault {
            return Some(format!(
                "campaign state is inert for this project: {}",
                bounded(fault, 240)
            ));
        }
        self.record.as_ref().and_then(|record| {
            (record.status == CampaignStatus::Paused).then(|| {
                format!(
                    "saved campaign {} is paused · {} · /campaign status",
                    record.id,
                    bounded(&record.objective, 120)
                )
            })
        })
    }

    pub(crate) fn command(
        &mut self,
        arg: Option<&str>,
        workspace: &Path,
        goal: Option<&Goal>,
    ) -> String {
        if !self.enabled {
            return "campaigns are disabled (ANGEL_CAMPAIGN=0)".to_string();
        }
        if let Err(error) = self.rebind_if_needed(workspace) {
            return format!("campaign error: {error}");
        }
        let command = match CampaignCommand::parse(arg) {
            Ok(command) => command,
            Err(error) => return format!("campaign error: {error}"),
        };
        let result = match command {
            CampaignCommand::Status => return self.status_text(workspace),
            CampaignCommand::New(objective) => self.create(objective),
            CampaignCommand::ImportGoal => self.import_goal(goal),
            CampaignCommand::CriterionAdd(text) => {
                let detail = text.clone();
                self.mutate("criterion_added", &detail, move |record, now| {
                    require_draft(record)?;
                    let next = u16::try_from(record.criteria.len() + 1)
                        .map_err(|_| "too many criteria".to_string())
                        .and_then(CriterionId::new)?;
                    record
                        .criteria
                        .push(AcceptanceCriterion::pending(next, text));
                    touch(record, now);
                    Ok(format!("added {next}"))
                })
            }
            CampaignCommand::CriterionRequire(id) => {
                self.set_required(id, true, "criterion_required")
            }
            CampaignCommand::CriterionOptional(id) => {
                self.set_required(id, false, "criterion_optional")
            }
            CampaignCommand::CriterionVerify { id, command } => {
                self.mutate("verifier_added", &id.to_string(), move |record, now| {
                    require_draft(record)?;
                    let network = record.policy.default_network_policy;
                    let criterion = criterion_mut(record, id)?;
                    let verifier_id = format!("{}-v{}", id, criterion.verifiers.len() + 1);
                    criterion.verifiers.push(VerifierSpec {
                        id: verifier_id,
                        kind: VerifierKind::Command,
                        command,
                        network,
                        timeout_secs: 1_800,
                    });
                    touch(record, now);
                    Ok(format!("added verifier for {id}"))
                })
            }
            CampaignCommand::CriterionScope { id, paths } => self.mutate(
                "criterion_scope_set",
                &id.to_string(),
                move |record, now| {
                    require_draft(record)?;
                    let criterion = criterion_mut(record, id)?;
                    criterion.test_scope = paths;
                    touch(record, now);
                    Ok(format!("set scope for {id}"))
                },
            ),
            CampaignCommand::Start => self.prepare(),
            CampaignCommand::Advance => {
                return "campaign error: /campaign advance must run through the operator flight slot"
                    .to_string()
            }
            CampaignCommand::Review => {
                return "campaign error: /campaign review must run through the operator flight slot"
                    .to_string()
            }
            CampaignCommand::Pause => self.pause(),
            CampaignCommand::Resume => self.resume(),
            CampaignCommand::Abandon => self.abandon(),
        };
        match result {
            Ok(message) => format!("{message}\n{}", self.status_text(workspace)),
            Err(error) => format!("campaign error: {error}"),
        }
    }

    pub(crate) fn status_text(&self, workspace: &Path) -> String {
        if !self.enabled {
            return "campaign · disabled".to_string();
        }
        if !self.store.binding().matches(workspace) {
            return "campaign · detached from current project; run /campaign status to rebind"
                .to_string();
        }
        if let Some(error) = &self.fault {
            return format!("campaign · inert · {}", bounded(error, 300));
        }
        let Some(record) = &self.record else {
            return "campaign · none — /campaign new <objective>".to_string();
        };
        let verified = record
            .criteria
            .iter()
            .filter(|criterion| criterion.status == CriterionStatus::Verified)
            .count();
        let required = record
            .criteria
            .iter()
            .filter(|criterion| criterion.required)
            .count();
        let verifier_count = record
            .criteria
            .iter()
            .map(|criterion| criterion.verifiers.len())
            .sum::<usize>();
        format!(
            "campaign {} · {:?} · revision {} · AC {verified}/{required} verified · {} total · {} verifier(s)\nobjective: {}",
            record.id,
            record.status,
            record.revision,
            record.criteria.len(),
            verifier_count,
            bounded(&record.objective, 300)
        )
    }

    pub(crate) fn lens_for(&self, workspace: &Path) -> Option<String> {
        if !self.enabled || self.fault.is_some() {
            return None;
        }
        let record = self.record.as_ref()?;
        if !record.project.matches(workspace) || record.status.is_terminal() {
            return None;
        }
        lens::render(record)
    }

    #[allow(dead_code)]
    pub(crate) fn execution_request(&self) -> Result<ExecutionRequest, String> {
        if !self.enabled || self.fault.is_some() {
            return Err("campaign controller is inert".to_string());
        }
        self.record
            .as_ref()
            .ok_or_else(|| "no campaign exists".to_string())?
            .execution_request()
    }

    /// Freeze or recover the sole executable round. This consumes no model
    /// budget; the app persists this contract before preparing a swarm run.
    pub(crate) fn freeze_round(&mut self, base: CampaignBase) -> Result<CampaignLaunch, String> {
        let workspace = self.workspace.clone();
        self.mutate_value(
            "round_contract_frozen",
            "operator advance",
            move |_store, record, now| {
                let mut base = base;
                if !record.project.matches(&workspace) {
                    return Err("campaign and swarm engine are bound to different projects".into());
                }
                if base.base_oid.is_empty()
                    || base.repo_root != record.project.canonical_root
                    || base.workspace_rel != path_text(&record.project.workspace_rel)
                {
                    return Err("swarm base does not match the campaign project binding".into());
                }
                if let Some(head) = record
                    .campaign_head_oid
                    .as_deref()
                    .or(record.frozen_base_oid.as_deref())
                {
                    // The frozen base remains the campaign origin. Accepted
                    // rounds advance only this logical head, so later rounds
                    // compose from the exact parked candidate without moving a
                    // live ref or checkout.
                    base.base_oid = head.to_string();
                }

                if let Some(contract) = record.active_contract.clone() {
                    if !matches!(
                        record.status,
                        CampaignStatus::Running | CampaignStatus::VerifyingRound
                    ) {
                        return Err(format!(
                            "active round is parked in {:?}; resolve that gate before advancing",
                            record.status
                        ));
                    }
                    if contract.base_oid != base.base_oid {
                        return Err("active round authorization targets a different base".into());
                    }
                    if record.status == CampaignStatus::Running {
                        record.transition(CampaignStatus::VerifyingRound, now)?;
                    }
                    return launch_from_contract(record, contract, base);
                }

                if record.status != CampaignStatus::Ready {
                    return Err(format!(
                        "campaign must be Ready before a new round; current state is {:?}",
                        record.status
                    ));
                }
                if record.policy.max_rounds > 0
                    && record.rounds.len()
                        >= usize::try_from(record.policy.max_rounds).unwrap_or(usize::MAX)
                {
                    return Err(format!(
                        "campaign round budget is exhausted; operator policy.max_rounds={}",
                        record.policy.max_rounds
                    ));
                }
                let criterion = record
                    .criteria
                    .iter()
                    .find(|criterion| {
                        criterion.required && criterion.status == CriterionStatus::Pending
                    })
                    .cloned()
                    .ok_or_else(|| "no required pending criterion is eligible".to_string())?;
                if criterion.verifiers.is_empty() || criterion.test_scope.is_empty() {
                    return Err(format!(
                        "{} needs both a verifier and an explicit test scope",
                        criterion.id
                    ));
                }

                if record.frozen_base_oid.is_none() {
                    record.frozen_base_oid = Some(base.base_oid.clone());
                }
                record.campaign_head_oid = Some(base.base_oid.clone());
                record.transition(CampaignStatus::Running, now)?;
                let first = criterion
                    .verifiers
                    .first()
                    .ok_or_else(|| "eligible criterion lost its verifier".to_string())?;
                let mut contract = RoundContract {
                    schema: ROUND_SCHEMA.to_string(),
                    campaign_id: record.id.clone(),
                    campaign_revision: record.revision,
                    round: u32::try_from(record.rounds.len() + 1)
                        .map_err(|_| "too many campaign rounds".to_string())?,
                    base_oid: base.base_oid.clone(),
                    objective: format!(
                        "{} — {}: {}",
                        record.objective, criterion.id, criterion.text
                    ),
                    target_criteria: vec![criterion.id],
                    targeted_test_cmd: first.command.clone(),
                    accept_cmd: first.command.clone(),
                    quality_cmds: criterion
                        .verifiers
                        .iter()
                        .skip(1)
                        .take(4)
                        .map(|verifier| verifier.command.clone())
                        .collect(),
                    test_scope: criterion.test_scope.clone(),
                    network_policy: first.network,
                    digest: String::new(),
                };
                contract.digest = digest_round_contract(&contract)?;
                record.active_contract = Some(contract.clone());
                criterion_mut(record, criterion.id)?.status = CriterionStatus::Active;
                record.transition(CampaignStatus::VerifyingRound, now)?;
                launch_from_contract(record, contract, base)
            },
        )
    }

    /// Attach a prepared internal run to the durable round before any model is
    /// allowed to execute.
    pub(crate) fn attach_prepared(
        &mut self,
        prepared: &PreparedSwarmRun,
        authorization: &AuthorizedCampaignBase,
    ) -> Result<(), String> {
        let prepared = prepared.clone();
        let authorization = authorization.clone();
        self.mutate_value(
            "swarm_run_attached",
            &prepared.run_id.clone(),
            move |_store, record, now| {
                let contract = active_authorized_contract(record, &authorization)?;
                if prepared.base_oid != contract.base_oid {
                    return Err("prepared swarm run targets a different base".to_string());
                }
                if let Some(existing) = record
                    .rounds
                    .iter()
                    .find(|receipt| receipt.contract.digest == contract.digest)
                {
                    if existing.swarm_run_id.as_deref() != Some(prepared.run_id.as_str()) {
                        return Err(
                            "round is already attached to a different swarm run".to_string()
                        );
                    }
                    return Ok(());
                }
                record.rounds.push(RoundReceipt {
                    contract,
                    state: RoundState::Compiling,
                    swarm_run_id: Some(prepared.run_id),
                    candidate_branch: None,
                    candidate_oid: None,
                    changed_paths: Vec::new(),
                    proofs: Vec::new(),
                    code_review: None,
                    alignment_review: None,
                    tokens: 0,
                    elapsed_ms: 0,
                    failure: None,
                });
                touch(record, now);
                Ok(())
            },
        )
    }

    /// Fold the technical swarm authority into campaign evidence. Even a fully
    /// green swarm stops at ReviewingRound; alignment is a separate gate.
    pub(crate) fn finish_round(&mut self, receipt: SwarmRunReceipt) -> Result<String, String> {
        let detail = receipt.run_id.clone();
        self.mutate_value("swarm_run_finished", &detail, move |store, record, now| {
            let contract = record
                .active_contract
                .clone()
                .ok_or_else(|| "campaign has no active round".to_string())?;
            if contract.base_oid != receipt.base_oid {
                return Err("swarm receipt targets a different frozen base".to_string());
            }
            let round_index = record
                .rounds
                .iter()
                .position(|round| {
                    round.contract.digest == contract.digest
                        && round.swarm_run_id.as_deref() == Some(receipt.run_id.as_str())
                })
                .ok_or_else(|| "swarm receipt is not attached to this round".to_string())?;

            match receipt.outcome {
                SwarmRunOutcome::Verified => {
                    if !receipt.technical_pass || !receipt.code_review_pass {
                        return Err(
                            "swarm marked verified without technical and code-review proof"
                                .to_string(),
                        );
                    }
                    let candidate_oid = receipt
                        .candidate_oid
                        .clone()
                        .ok_or_else(|| "verified swarm receipt has no candidate OID".to_string())?;
                    let candidate_branch = receipt
                        .parked_branch
                        .clone()
                        .ok_or_else(|| "verified swarm receipt has no parked branch".to_string())?;
                    let (artifact_path, sha256) =
                        store.save_proof_snapshot(&receipt.run_id, &receipt)?;
                    let proof = ProofRef {
                        kind: ProofKind::FullTest,
                        run_id: receipt.run_id.clone(),
                        artifact_path,
                        sha256,
                        git_oid: candidate_oid.clone(),
                        summary: "swarm technical verification and adversarial code review passed"
                            .to_string(),
                    };
                    {
                        let round = &mut record.rounds[round_index];
                        round.state = RoundState::TechnicallyVerified;
                        round.candidate_branch = Some(candidate_branch);
                        round.candidate_oid = Some(candidate_oid);
                        round.changed_paths =
                            receipt.changed_paths.iter().map(PathBuf::from).collect();
                        round.proofs = vec![proof.clone()];
                        round.failure = None;
                    }
                    for id in &contract.target_criteria {
                        let criterion = criterion_mut(record, *id)?;
                        criterion.status = CriterionStatus::TechnicallyVerified;
                        if !criterion
                            .proof_refs
                            .iter()
                            .any(|item| item.run_id == proof.run_id)
                        {
                            criterion.proof_refs.push(proof.clone());
                        }
                    }
                    record.transition(CampaignStatus::ReviewingRound, now)?;
                    Ok(format!(
                        "round {} is technically verified on {}; alignment review remains pending",
                        contract.round, receipt.run_id
                    ))
                }
                SwarmRunOutcome::Paused | SwarmRunOutcome::Rejected => {
                    let failure = receipt.error.clone().unwrap_or_else(|| {
                        match receipt.outcome {
                            SwarmRunOutcome::Rejected => "swarm proof rejected",
                            _ => "swarm run paused",
                        }
                        .to_string()
                    });
                    record.rounds[round_index].state =
                        if receipt.outcome == SwarmRunOutcome::Rejected {
                            RoundState::Failed
                        } else {
                            RoundState::Paused
                        };
                    record.rounds[round_index].failure = Some(bounded(&failure, 4_096));
                    record.paused_from = Some(CampaignStatus::VerifyingRound);
                    record.transition(CampaignStatus::Paused, now)?;
                    Ok(format!(
                        "campaign paused after {}: {}",
                        receipt.run_id, failure
                    ))
                }
            }
        })
    }

    /// Persist the alignment-reviewing state and return the exact bounded
    /// evidence packet an internal non-root reviewer is authorized to inspect.
    pub(crate) fn begin_alignment_review(&mut self) -> Result<CampaignAlignmentRequest, String> {
        self.mutate_value(
            "alignment_review_started",
            "operator review",
            move |_store, record, now| {
                if record.status != CampaignStatus::ReviewingRound {
                    return Err(format!(
                        "campaign must be ReviewingRound; current state is {:?}",
                        record.status
                    ));
                }
                let contract = record
                    .active_contract
                    .clone()
                    .ok_or_else(|| "campaign has no active review contract".to_string())?;
                let (swarm_run_id, candidate_branch, candidate_oid, proofs) = {
                    let round = record
                        .rounds
                        .iter_mut()
                        .find(|round| round.contract.digest == contract.digest)
                        .ok_or_else(|| "active campaign round receipt is missing".to_string())?;
                    if !matches!(
                        round.state,
                        RoundState::TechnicallyVerified
                            | RoundState::AlignmentReviewing
                            | RoundState::Blocked
                    ) {
                        return Err(format!(
                            "round {} is not eligible for alignment review from {:?}",
                            contract.round, round.state
                        ));
                    }
                    let swarm_run_id = round
                        .swarm_run_id
                        .clone()
                        .ok_or_else(|| "reviewing round has no swarm run".to_string())?;
                    let candidate_branch = round
                        .candidate_branch
                        .clone()
                        .ok_or_else(|| "reviewing round has no parked branch".to_string())?;
                    let candidate_oid = round
                        .candidate_oid
                        .clone()
                        .ok_or_else(|| "reviewing round has no candidate OID".to_string())?;
                    let proofs = round
                        .proofs
                        .iter()
                        .filter(|proof| proof.kind != ProofKind::AlignmentReview)
                        .map(|proof| AlignmentProof {
                            sha256: proof.sha256.clone(),
                            summary: proof.summary.clone(),
                        })
                        .collect::<Vec<_>>();
                    round.state = RoundState::AlignmentReviewing;
                    round.failure = None;
                    (swarm_run_id, candidate_branch, candidate_oid, proofs)
                };
                let criteria = contract
                    .target_criteria
                    .iter()
                    .map(|id| {
                        record
                            .criteria
                            .iter()
                            .find(|criterion| criterion.id == *id)
                            .map(|criterion| AlignmentCriterion {
                                id: id.to_string(),
                                text: criterion.text.clone(),
                            })
                            .ok_or_else(|| format!("target criterion {id} disappeared"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if proofs.is_empty() {
                    return Err("alignment review has no technical proof to cite".to_string());
                }
                touch(record, now);
                Ok(CampaignAlignmentRequest {
                    campaign_id: contract.campaign_id,
                    campaign_revision: contract.campaign_revision,
                    round: contract.round,
                    contract_digest: contract.digest,
                    swarm_run_id,
                    candidate_branch,
                    candidate_oid,
                    criteria,
                    proofs,
                })
            },
        )
    }

    /// Record an independently-produced alignment result. Technical and
    /// alignment receipts remain separate and are both required for acceptance.
    pub(crate) fn finish_alignment_review(
        &mut self,
        receipt: CampaignAlignmentReceipt,
    ) -> Result<String, String> {
        let detail = receipt.reviewed_contract_digest.clone();
        self.mutate_value(
            "alignment_review_finished",
            &detail,
            move |store, record, now| {
                if receipt.schema != "campaign-review/v1"
                    || record.status != CampaignStatus::ReviewingRound
                {
                    return Err("alignment receipt arrived outside ReviewingRound".to_string());
                }
                let contract = record
                    .active_contract
                    .clone()
                    .ok_or_else(|| "campaign has no active review contract".to_string())?;
                let round_index = record
                    .rounds
                    .iter()
                    .position(|round| round.contract.digest == contract.digest)
                    .ok_or_else(|| "active campaign round receipt is missing".to_string())?;
                let candidate_oid = record.rounds[round_index]
                    .candidate_oid
                    .clone()
                    .ok_or_else(|| "reviewing round has no candidate OID".to_string())?;
                let swarm_run_id = record.rounds[round_index]
                    .swarm_run_id
                    .clone()
                    .ok_or_else(|| "reviewing round has no swarm run".to_string())?;
                if receipt.reviewed_contract_digest != contract.digest
                    || receipt.reviewed_candidate_oid != candidate_oid
                {
                    return Err("alignment receipt targets a different contract or candidate"
                        .to_string());
                }
                let expected_criteria = contract
                    .target_criteria
                    .iter()
                    .map(ToString::to_string)
                    .collect::<BTreeSet<_>>();
                let cited_criteria = receipt
                    .cited_criteria
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                let expected_proofs = record.rounds[round_index]
                    .proofs
                    .iter()
                    .filter(|proof| proof.kind != ProofKind::AlignmentReview)
                    .map(|proof| proof.sha256.clone())
                    .collect::<BTreeSet<_>>();
                let cited_proofs = receipt
                    .cited_proof_sha256
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                if cited_criteria != expected_criteria
                    || cited_criteria.len() != receipt.cited_criteria.len()
                    || cited_proofs != expected_proofs
                    || cited_proofs.len() != receipt.cited_proof_sha256.len()
                {
                    return Err("alignment receipt does not cite the authorized criteria/proofs"
                        .to_string());
                }
                let alignment_independence = map_independence(receipt.independence);
                let independently_passed = receipt.verdict == AlignmentVerdict::Pass
                    && matches!(
                        alignment_independence,
                        ReviewIndependence::DifferentRouteAndRevision
                            | ReviewIndependence::DifferentRevision
                    );
                if receipt.response_sha256.len() != 64
                    || !receipt
                        .response_sha256
                        .chars()
                        .all(|character| character.is_ascii_hexdigit())
                {
                    return Err("alignment response digest is invalid".to_string());
                }
                let (artifact_path, sha256) = store.save_alignment_snapshot(
                    &swarm_run_id,
                    &receipt.response_sha256,
                    &receipt,
                )?;
                let alignment_proof = ProofRef {
                    kind: ProofKind::AlignmentReview,
                    run_id: swarm_run_id,
                    artifact_path,
                    sha256,
                    git_oid: candidate_oid.clone(),
                    summary: bounded(&receipt.summary, 2_048),
                };
                let code_review = ReviewReceipt {
                    route: receipt.technical_reviewer_route.clone(),
                    model_revision: receipt.technical_reviewer_model_revision.clone(),
                    independence: map_independence(receipt.technical_review_independence),
                    verdict: ReviewVerdict::Pass,
                    reviewed_contract_digest: contract.digest.clone(),
                    reviewed_candidate_oid: candidate_oid.clone(),
                };
                let alignment_review = ReviewReceipt {
                    route: receipt.reviewer_route.clone(),
                    model_revision: receipt.reviewer_model_revision.clone(),
                    independence: alignment_independence,
                    verdict: if independently_passed {
                        ReviewVerdict::Pass
                    } else {
                        ReviewVerdict::Block
                    },
                    reviewed_contract_digest: contract.digest.clone(),
                    reviewed_candidate_oid: candidate_oid.clone(),
                };
                {
                    let round = &mut record.rounds[round_index];
                    if !round
                        .proofs
                        .iter()
                        .any(|proof| proof.artifact_path == alignment_proof.artifact_path)
                    {
                        round.proofs.push(alignment_proof.clone());
                    }
                    round.code_review = Some(code_review);
                    round.alignment_review = Some(alignment_review);
                    if independently_passed {
                        round.state = RoundState::Accepted;
                        round.failure = None;
                    } else {
                        round.state = RoundState::Blocked;
                        round.failure = Some(bounded(&receipt.summary, 4_096));
                    }
                }
                for id in &contract.target_criteria {
                    let criterion = criterion_mut(record, *id)?;
                    if !criterion
                        .proof_refs
                        .iter()
                        .any(|proof| proof.artifact_path == alignment_proof.artifact_path)
                    {
                        criterion.proof_refs.push(alignment_proof.clone());
                    }
                    if independently_passed {
                        criterion.status = CriterionStatus::Verified;
                    }
                }
                if independently_passed {
                    record.campaign_head_oid = Some(candidate_oid);
                    record.active_contract = None;
                    let has_pending_required = record.criteria.iter().any(|criterion| {
                        criterion.required
                            && !matches!(
                                criterion.status,
                                CriterionStatus::Verified | CriterionStatus::Deferred
                            )
                    });
                    let next = if has_pending_required {
                        CampaignStatus::Ready
                    } else {
                        CampaignStatus::ReadyToIntegrate
                    };
                    record.transition(next, now)?;
                    Ok(format!(
                        "alignment review passed independently; campaign is parked at {next:?}"
                    ))
                } else {
                    touch(record, now);
                    Ok(format!(
                        "alignment review blocked; campaign remains ReviewingRound for explicit retry · {}",
                        bounded(&receipt.summary, 240)
                    ))
                }
            },
        )
    }

    pub(crate) fn fail_alignment_review(&mut self, error: &str) -> Result<String, String> {
        let detail = bounded(error, 2_048);
        self.mutate_value(
            "alignment_review_blocked",
            &detail.clone(),
            move |_store, record, now| {
                if record.status != CampaignStatus::ReviewingRound {
                    return Err(format!(
                        "cannot block alignment review from {:?}",
                        record.status
                    ));
                }
                let contract = record
                    .active_contract
                    .as_ref()
                    .ok_or_else(|| "campaign has no active review contract".to_string())?;
                let round = record
                    .rounds
                    .iter_mut()
                    .find(|round| round.contract.digest == contract.digest)
                    .ok_or_else(|| "active campaign round receipt is missing".to_string())?;
                round.state = RoundState::Blocked;
                round.failure = Some(detail.clone());
                touch(record, now);
                Ok(format!(
                    "alignment review blocked fail-closed; /campaign review retries explicitly: {detail}"
                ))
            },
        )
    }

    pub(crate) fn pause_execution(&mut self, error: &str) -> Result<String, String> {
        let detail = bounded(error, 2_048);
        self.mutate_value(
            "campaign_execution_paused",
            &detail.clone(),
            move |_store, record, now| {
                if record.status == CampaignStatus::Paused {
                    return Ok("campaign is already paused".to_string());
                }
                if !matches!(
                    record.status,
                    CampaignStatus::Running | CampaignStatus::VerifyingRound
                ) {
                    return Err(format!(
                        "cannot pause execution from campaign state {:?}",
                        record.status
                    ));
                }
                record.paused_from = Some(record.status);
                record.transition(CampaignStatus::Paused, now)?;
                Ok(format!("campaign execution paused: {detail}"))
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn record(&self) -> Option<&CampaignRecord> {
        self.record.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn record_path(&self) -> &Path {
        self.store.record_path()
    }

    fn create(&mut self, objective: String) -> Result<String, String> {
        self.ensure_healthy()?;
        let _lease = self.store.acquire_lease()?;
        match self.store.load() {
            LoadedCampaign::Missing => {}
            LoadedCampaign::Active(_) => {
                return Err(
                    "a campaign already exists for this project; abandon preserves it".to_string(),
                );
            }
            LoadedCampaign::Inert(error) => {
                return Err(format!("campaign store is inert: {error}"));
            }
        }
        let now = now_ms();
        let sequence = CAMPAIGN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let id = format!("cmp-{now:x}-{:x}-{sequence:x}", std::process::id());
        let record = CampaignRecord::draft(id, &self.workspace, objective, now);
        record.validate_for(&self.workspace)?;
        self.store.save(&record)?;
        let event_error = self.store.append_event(
            &record,
            CampaignStatus::Draft,
            "campaign_created",
            "operator",
        );
        self.record = Some(record);
        append_warning("campaign created", event_error)
    }

    fn import_goal(&mut self, goal: Option<&Goal>) -> Result<String, String> {
        let goal = goal
            .and_then(|goal| goal.campaign_snapshot_for(&self.workspace))
            .ok_or_else(|| "no active goal is available to import".to_string())?;
        self.ensure_healthy()?;
        let _lease = self.store.acquire_lease()?;
        match self.store.load() {
            LoadedCampaign::Missing => {}
            LoadedCampaign::Active(_) => {
                return Err("a campaign already exists for this project".to_string());
            }
            LoadedCampaign::Inert(error) => {
                return Err(format!("campaign store is inert: {error}"));
            }
        }
        let now = now_ms();
        let sequence = CAMPAIGN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let id = format!("cmp-{now:x}-{:x}-{sequence:x}", std::process::id());
        let mut record =
            CampaignRecord::draft(id, &self.workspace, goal.text.trim().to_string(), now);
        for (index, text) in goal.acceptance.iter().enumerate() {
            let id = u16::try_from(index + 1)
                .map_err(|_| "goal has too many acceptance criteria".to_string())
                .and_then(CriterionId::new)?;
            let mut criterion = AcceptanceCriterion::pending(id, text.trim().to_string());
            if let Some(command) = goal.accept_cmd.as_ref() {
                criterion.verifiers.push(VerifierSpec {
                    id: format!("{id}-goal"),
                    kind: VerifierKind::Aggregate,
                    command: command.clone(),
                    network: NetworkPolicy::Inherited,
                    timeout_secs: 1_800,
                });
            }
            record.criteria.push(criterion);
        }
        record.validate_for(&self.workspace)?;
        self.store.save(&record)?;
        let event_error =
            self.store
                .append_event(&record, CampaignStatus::Draft, "goal_imported", "operator");
        let count = record.criteria.len();
        self.record = Some(record);
        append_warning(
            &format!("imported active goal with {count} criterion/criteria"),
            event_error,
        )
    }

    fn set_required(
        &mut self,
        id: CriterionId,
        required: bool,
        event: &'static str,
    ) -> Result<String, String> {
        self.mutate(event, &id.to_string(), move |record, now| {
            require_draft(record)?;
            let criterion = criterion_mut(record, id)?;
            criterion.required = required;
            if required {
                criterion.waiver = None;
                if criterion.status == CriterionStatus::Deferred {
                    criterion.status = CriterionStatus::Pending;
                }
            } else {
                criterion.waiver = Some(OperatorWaiver {
                    reason: "operator marked optional".to_string(),
                    authored_ms: now,
                });
            }
            touch(record, now);
            Ok(format!(
                "{id} is now {}",
                if required { "required" } else { "optional" }
            ))
        })
    }

    fn prepare(&mut self) -> Result<String, String> {
        self.mutate("campaign_prepared", "operator", |record, now| {
            if record.status == CampaignStatus::Ready {
                return Ok("campaign is already ready".to_string());
            }
            require_draft(record)?;
            if !record.criteria.iter().any(|criterion| criterion.required) {
                return Err("add at least one required criterion".to_string());
            }
            if record
                .criteria
                .iter()
                .filter(|criterion| criterion.required)
                .any(|criterion| criterion.verifiers.is_empty())
            {
                return Err("every required criterion needs a verifier".to_string());
            }
            record.transition(CampaignStatus::Ready, now)?;
            Ok("campaign is ready; /campaign advance authorizes one proof round".to_string())
        })
    }

    fn pause(&mut self) -> Result<String, String> {
        self.mutate("campaign_paused", "operator", |record, now| {
            if record.status == CampaignStatus::Paused {
                return Ok("campaign is already paused".to_string());
            }
            if record.status == CampaignStatus::Draft || record.status.is_terminal() {
                return Err("only a ready or active campaign can be paused".to_string());
            }
            record.paused_from = Some(record.status);
            record.transition(CampaignStatus::Paused, now)?;
            Ok("campaign paused; no work will resume automatically".to_string())
        })
    }

    fn resume(&mut self) -> Result<String, String> {
        self.mutate("campaign_resumed", "operator", |record, now| {
            if record.status != CampaignStatus::Paused {
                return Err("campaign is not paused".to_string());
            }
            let prior = record
                .paused_from
                .take()
                .ok_or_else(|| "paused campaign has no recovery state".to_string())?;
            let next = match prior {
                CampaignStatus::Ready => CampaignStatus::Ready,
                CampaignStatus::ReviewingRound => CampaignStatus::ReviewingRound,
                CampaignStatus::AwaitingOperator => CampaignStatus::AwaitingOperator,
                CampaignStatus::ReadyToIntegrate => CampaignStatus::ReadyToIntegrate,
                _ => CampaignStatus::Running,
            };
            record.transition(next, now)?;
            Ok(format!(
                "campaign resumed to {next:?}; no model or Git action was started"
            ))
        })
    }

    fn abandon(&mut self) -> Result<String, String> {
        self.mutate("campaign_abandoned", "operator", |record, now| {
            if record.status.is_terminal() {
                return Err("campaign is already terminal".to_string());
            }
            if record.status == CampaignStatus::Draft {
                record.status = CampaignStatus::Stopped;
                touch(record, now);
                record.validate()?;
            } else {
                if record.status == CampaignStatus::Paused {
                    record.paused_from = None;
                }
                record.transition(CampaignStatus::Stopped, now)?;
            }
            Ok(
                "campaign abandoned; its record and future branch evidence are preserved"
                    .to_string(),
            )
        })
    }

    fn mutate<F>(&mut self, event: &str, detail: &str, mutate: F) -> Result<String, String>
    where
        F: FnOnce(&mut CampaignRecord, u64) -> Result<String, String>,
    {
        self.ensure_healthy()?;
        let _lease = self.store.acquire_lease()?;
        let mut record = match self.store.load() {
            LoadedCampaign::Active(record) => record,
            LoadedCampaign::Missing => {
                return Err("no campaign exists for this project".to_string());
            }
            LoadedCampaign::Inert(error) => {
                self.record = None;
                self.fault = Some(error.clone());
                return Err(format!("campaign store is inert: {error}"));
            }
        };
        if self
            .record
            .as_ref()
            .is_some_and(|current| current.id != record.id)
        {
            self.record = None;
            self.fault = Some("campaign identity changed on disk".to_string());
            return Err("campaign identity changed on disk".to_string());
        }
        let old_status = record.status;
        let message = mutate(&mut record, now_ms())?;
        record.validate_for(&self.workspace)?;
        self.store.save(&record)?;
        let event_error = self.store.append_event(&record, old_status, event, detail);
        self.record = Some(*record);
        append_warning(&message, event_error)
    }

    fn mutate_value<T, F>(&mut self, event: &str, detail: &str, mutate: F) -> Result<T, String>
    where
        F: FnOnce(&CampaignStore, &mut CampaignRecord, u64) -> Result<T, String>,
    {
        self.ensure_healthy()?;
        let _lease = self.store.acquire_lease()?;
        let mut record = match self.store.load() {
            LoadedCampaign::Active(record) => record,
            LoadedCampaign::Missing => {
                return Err("no campaign exists for this project".to_string());
            }
            LoadedCampaign::Inert(error) => {
                self.record = None;
                self.fault = Some(error.clone());
                return Err(format!("campaign store is inert: {error}"));
            }
        };
        if self
            .record
            .as_ref()
            .is_some_and(|current| current.id != record.id)
        {
            self.record = None;
            self.fault = Some("campaign identity changed on disk".to_string());
            return Err("campaign identity changed on disk".to_string());
        }
        let old_status = record.status;
        let value = mutate(&self.store, &mut record, now_ms())?;
        record.validate_for(&self.workspace)?;
        self.store.save(&record)?;
        let _event_error = self.store.append_event(&record, old_status, event, detail);
        self.record = Some(*record);
        Ok(value)
    }

    fn ensure_healthy(&self) -> Result<(), String> {
        if let Some(error) = &self.fault {
            return Err(format!("campaign store is inert: {error}"));
        }
        Ok(())
    }

    fn rebind_if_needed(&mut self, workspace: &Path) -> Result<(), String> {
        if self.store.binding().matches(workspace) {
            self.workspace = workspace.to_path_buf();
            return Ok(());
        }
        let store = CampaignStore::for_workspace(workspace);
        let (record, fault) = load_state(&store);
        self.workspace = workspace.to_path_buf();
        self.store = store;
        self.record = record;
        self.fault = fault;
        self.recover_interrupted();
        self.fault.clone().map_or(Ok(()), Err)
    }

    fn recover_interrupted(&mut self) {
        if !self.enabled || self.fault.is_some() {
            return;
        }
        let Some(record) = self.record.as_ref() else {
            return;
        };
        if !record.status.is_running_like() {
            return;
        }
        let interrupted = record.status;
        let result = (|| {
            let _lease = self.store.acquire_lease()?;
            let mut record = match self.store.load() {
                LoadedCampaign::Active(record) => record,
                LoadedCampaign::Missing => {
                    return Err("campaign vanished during recovery".to_string());
                }
                LoadedCampaign::Inert(error) => return Err(error),
            };
            if !record.status.is_running_like() {
                self.record = Some(*record);
                return Ok(());
            }
            let old_status = record.status;
            record.paused_from = Some(record.status);
            record.transition(CampaignStatus::Paused, now_ms())?;
            self.store.save(&record)?;
            self.store.append_event(
                &record,
                old_status,
                "crash_recovered",
                "startup converted active state to paused",
            )?;
            self.record = Some(*record);
            Ok(())
        })();
        if let Err(error) = result {
            self.record = None;
            self.fault = Some(format!(
                "could not pause interrupted {interrupted:?} campaign: {error}"
            ));
        }
    }
}

fn load_state(store: &CampaignStore) -> (Option<CampaignRecord>, Option<String>) {
    match store.load() {
        LoadedCampaign::Missing => (None, None),
        LoadedCampaign::Active(record) => (Some(*record), None),
        LoadedCampaign::Inert(error) => (None, Some(error)),
    }
}

fn criterion_mut(
    record: &mut CampaignRecord,
    id: CriterionId,
) -> Result<&mut AcceptanceCriterion, String> {
    record
        .criteria
        .iter_mut()
        .find(|criterion| criterion.id == id)
        .ok_or_else(|| format!("unknown criterion {id}"))
}

fn require_draft(record: &CampaignRecord) -> Result<(), String> {
    (record.status == CampaignStatus::Draft)
        .then_some(())
        .ok_or_else(|| "campaign contract is immutable after it becomes ready".to_string())
}

fn touch(record: &mut CampaignRecord, now: u64) {
    record.revision = record.revision.saturating_add(1);
    record.updated_ms = now;
}

fn active_authorized_contract(
    record: &CampaignRecord,
    authorization: &AuthorizedCampaignBase,
) -> Result<RoundContract, String> {
    let contract = record
        .active_contract
        .clone()
        .ok_or_else(|| "campaign has no active round".to_string())?;
    if record.id != authorization.campaign_id
        || contract.campaign_revision != authorization.campaign_revision
        || contract.round != authorization.round
        || contract.digest != authorization.contract_digest
        || contract.base_oid != authorization.base.base_oid
    {
        return Err("campaign swarm authorization no longer matches active contract".to_string());
    }
    Ok(contract)
}

fn launch_from_contract(
    record: &CampaignRecord,
    contract: RoundContract,
    base: CampaignBase,
) -> Result<CampaignLaunch, String> {
    let target = contract
        .target_criteria
        .first()
        .copied()
        .ok_or_else(|| "round contract has no target criterion".to_string())?;
    let criterion = record
        .criteria
        .iter()
        .find(|criterion| criterion.id == target)
        .ok_or_else(|| "round target criterion disappeared".to_string())?;
    Ok(CampaignLaunch {
        request: CampaignCompileRequest {
            goal: format!(
                "{}\n\nRequired criterion {}: {}",
                record.objective, criterion.id, criterion.text
            ),
            task_type: "campaign".to_string(),
            targeted_test_cmd: contract.targeted_test_cmd.clone(),
            accept_cmd: contract.accept_cmd.clone(),
            quality_cmds: contract.quality_cmds.clone(),
            test_scope: contract
                .test_scope
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            default_route: "auto".to_string(),
            role_routes: BTreeMap::new(),
        },
        authorization: AuthorizedCampaignBase {
            campaign_id: contract.campaign_id.clone(),
            campaign_revision: contract.campaign_revision,
            round: contract.round,
            contract_digest: contract.digest.clone(),
            base,
        },
    })
}

fn path_text(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        path.to_string_lossy().into_owned()
    }
}

fn map_independence(independence: AlignmentIndependence) -> ReviewIndependence {
    match independence {
        AlignmentIndependence::DifferentRouteAndRevision => {
            ReviewIndependence::DifferentRouteAndRevision
        }
        AlignmentIndependence::DifferentRevision => ReviewIndependence::DifferentRevision,
        AlignmentIndependence::SameRoute => ReviewIndependence::SameRoute,
        AlignmentIndependence::Unavailable => ReviewIndependence::Unavailable,
    }
}

fn campaign_enabled() -> bool {
    std::env::var("ANGEL_CAMPAIGN").map_or(true, |value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        )
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(1)
        .max(1)
}

fn bounded(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let mut out = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        out.push_str(" …");
    }
    out
}

fn append_warning(message: &str, event_result: Result<(), String>) -> Result<String, String> {
    Ok(match event_result {
        Ok(()) => message.to_string(),
        Err(error) => format!(
            "{message} (state saved, but the event receipt is degraded: {})",
            bounded(&error, 240)
        ),
    })
}

fn usage() -> String {
    "usage: /campaign [status|new <objective>|import-goal|criterion ...|start|advance|review|pause|resume|abandon]"
        .to_string()
}

fn criterion_usage() -> String {
    "usage: /campaign criterion <add|require|optional|verify|scope> ...".to_string()
}
