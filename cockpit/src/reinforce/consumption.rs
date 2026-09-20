//! Context-bound evaluator replay and persistent receipt consumption.

use super::artifact::load_evaluator_artifact;
use super::{EvaluatorEvidence, Reward, RewardInput};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const LEDGER_SCHEMA: &str = "angel.rlvr.receipt-consumption/v1";
const LEDGER_FILE: &str = "receipt-consumption.jsonl";
const PAIR_LEDGER_SCHEMA: &str = "angel.rlvr.receipt-pair-consumption/v2";
const PAIR_LEDGER_FILE: &str = "receipt-pair-consumption.jsonl";
const LEDGER_LOCK_FILE: &str = "receipt-consumption.lock";
const TERMINAL_RELEASE_FILE: &str = "terminal-release.json";
const CAMPAIGN_EXECUTION_LOCK_FILE: &str = ".campaign-execution.lock";
const MAX_LEDGER_BYTES: u64 = 64 * 1024 * 1024;
static TERMINAL_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub struct ExpectedReceiptContext<'a> {
    pub subject_sha256: &'a str,
    pub command_sha256: &'a str,
    pub verifier_contract_sha256: &'a str,
    pub execution_policy_sha256: &'a str,
}

impl ExpectedReceiptContext<'_> {
    fn validate(&self, evidence: &EvaluatorEvidence) -> Result<(), String> {
        for (label, expected, actual) in [
            ("subject", self.subject_sha256, evidence.subject_sha256()),
            ("command", self.command_sha256, evidence.command_sha256()),
            (
                "verifier contract",
                self.verifier_contract_sha256,
                evidence.verifier_contract_sha256(),
            ),
            (
                "execution policy",
                self.execution_policy_sha256,
                evidence.execution_policy_sha256(),
            ),
        ] {
            if expected != actual {
                return Err(format!("evaluator artifact {label} context mismatch"));
            }
        }
        Ok(())
    }
}

pub struct ReceiptCountContext<'a> {
    pub run_id: &'a str,
    pub reproduction_id: &'a str,
    pub attempt_id: &'a str,
    pub retry_index: u32,
    pub prior_attempt_id: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConsumptionOutcome {
    ScoreOnly { reward: f32 },
    Counted { reward: f32 },
    IdempotentResume { reward: f32 },
}

pub struct ReceiptPairCountContext<'a> {
    pub campaign_id: &'a str,
    pub reproduction_id: &'a str,
    pub phase: &'a str,
    pub cohort_manifest_sha256: &'a str,
    pub case_id: &'a str,
    pub task_sha256: &'a str,
    pub prompt_sha256: &'a str,
    pub inventory_reward_contract_sha256: &'a str,
    pub outcome_reward_contract_sha256: &'a str,
    pub policy_version: u64,
    pub sample_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReceiptPairOutcome {
    pub counted: bool,
    pub inventory_reward: f32,
    pub outcome_reward: f32,
    pub receipt_sha256s: [String; 2],
}

/// Hydrate an already-counted logical sample slot without reopening evaluator
/// artifacts or calling the reward implementation. The hash-chained row was
/// validated before lookup and the canonical pair id binds the task identity
/// even though the human-readable task digest is not duplicated in the row.
pub(crate) fn load_receipt_pair(
    root: &Path,
    context: &ReceiptPairCountContext<'_>,
    expected_inventory_subject_sha256: &str,
    expected_outcome_subject_sha256: &str,
) -> Result<Option<ReceiptPairOutcome>, String> {
    validate_pair_context(context)?;
    ensure_ledger_root(root)?;
    let _lock = LedgerLock::acquire_root(root)?;
    let path = root.join(PAIR_LEDGER_FILE);
    reject_non_regular_existing(&path)?;
    if !path.exists() {
        return Ok(None);
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("could not open receipt pair ledger: {error}"))?;
    let pair_id = canonical_receipt_pair_id(context);
    let rows = read_pair_rows_recover_tail(&mut file)?;
    let Some(row) = rows
        .into_iter()
        .find(|row| row.campaign_id == context.campaign_id && row.pair_id == pair_id)
    else {
        return Ok(None);
    };
    if row.reproduction_id != context.reproduction_id
        || row.phase != context.phase
        || row.cohort_manifest_sha256 != context.cohort_manifest_sha256
        || row.case_id != context.case_id
        || row.prompt_sha256 != context.prompt_sha256
        || row.inventory_reward_contract_sha256 != context.inventory_reward_contract_sha256
        || row.outcome_reward_contract_sha256 != context.outcome_reward_contract_sha256
        || row.policy_version != context.policy_version
        || row.sample_index != context.sample_index
        || row.inventory.subject_sha256 != expected_inventory_subject_sha256
        || row.outcome.subject_sha256 != expected_outcome_subject_sha256
    {
        return Err("persisted receipt pair authority context mismatch".into());
    }
    let inventory_reward = f32::from_bits(row.inventory.reward_bits);
    let outcome_reward = f32::from_bits(row.outcome.reward_bits);
    if !inventory_reward.is_finite() || !outcome_reward.is_finite() {
        return Err("persisted receipt pair reward is non-finite".into());
    }
    Ok(Some(ReceiptPairOutcome {
        counted: false,
        inventory_reward,
        outcome_reward,
        receipt_sha256s: [row.inventory.receipt_sha256, row.outcome.receipt_sha256],
    }))
}

pub(crate) fn load_receipt_pair_poison(
    root: &Path,
    context: &ReceiptPairCountContext<'_>,
) -> Result<Option<String>, String> {
    validate_pair_context(context)?;
    let pair_id = canonical_receipt_pair_id(context);
    let relative = Path::new("poison").join(format!("{pair_id}.json"));
    load_immutable_record(root, &relative, "receipt pair poison")?
        .map(|bytes| decode_pair_poison(&bytes, context).map(|record| record.failure))
        .transpose()
}

pub(crate) fn install_or_adopt_receipt_pair_poison(
    root: &Path,
    context: &ReceiptPairCountContext<'_>,
    failure: &str,
) -> Result<String, String> {
    validate_pair_context(context)?;
    if failure.is_empty() || failure.len() > 16 * 1024 || failure.contains('\0') {
        return Err("receipt pair poison failure is invalid".into());
    }
    let pair_id = canonical_receipt_pair_id(context);
    let mut record = PairPoisonRecord {
        schema: PAIR_POISON_SCHEMA.to_string(),
        campaign_id: context.campaign_id.to_string(),
        reproduction_id: context.reproduction_id.to_string(),
        phase: context.phase.to_string(),
        cohort_manifest_sha256: context.cohort_manifest_sha256.to_string(),
        case_id: context.case_id.to_string(),
        task_sha256: context.task_sha256.to_string(),
        prompt_sha256: context.prompt_sha256.to_string(),
        inventory_reward_contract_sha256: context.inventory_reward_contract_sha256.to_string(),
        outcome_reward_contract_sha256: context.outcome_reward_contract_sha256.to_string(),
        policy_version: context.policy_version,
        sample_index: context.sample_index,
        pair_id: pair_id.clone(),
        failure: failure.to_string(),
        record_sha256: String::new(),
    };
    record.record_sha256 = pair_poison_sha256(&record)?;
    let mut proposed = serde_json::to_vec(&record)
        .map_err(|error| format!("could not encode receipt pair poison: {error}"))?;
    proposed.push(b'\n');

    ensure_ledger_root(root)?;
    let _lock = LedgerLock::acquire_root(root)?;
    let pair_path = root.join(PAIR_LEDGER_FILE);
    reject_non_regular_existing(&pair_path)?;
    if pair_path.exists() {
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options
            .open(&pair_path)
            .map_err(|error| format!("could not open receipt pair ledger: {error}"))?;
        if read_pair_rows_recover_tail(&mut file)?
            .iter()
            .any(|row| row.campaign_id == context.campaign_id && row.pair_id == pair_id)
        {
            return Err("cannot poison an already committed receipt pair slot".into());
        }
    }
    let directory = root.join("poison");
    match std::fs::symlink_metadata(&directory) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err("receipt pair poison parent must be a non-symlink directory".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&directory).map_err(|error| {
                format!("could not create receipt pair poison directory: {error}")
            })?;
            sync_directory(root, "receipt pair poison directory")?;
        }
        Err(error) => {
            return Err(format!(
                "could not inspect receipt pair poison directory: {error}"
            ));
        }
    }
    let path = directory.join(format!("{pair_id}.json"));
    reject_non_regular_existing(&path)?;
    let adopted = if path.exists() {
        read_bounded_record(&path, "receipt pair poison")?.ok_or_else(|| {
            "receipt pair poison disappeared while holding its authority lock".to_string()
        })?
    } else {
        reject_unsealed(root)?;
        write_immutable_record_locked(root, &path, &proposed)?;
        proposed
    };
    decode_pair_poison(&adopted, context).map(|record| record.failure)
}

fn decode_pair_poison(
    bytes: &[u8],
    context: &ReceiptPairCountContext<'_>,
) -> Result<PairPoisonRecord, String> {
    let record: PairPoisonRecord = serde_json::from_slice(bytes)
        .map_err(|error| format!("could not decode receipt pair poison: {error}"))?;
    if record.schema != PAIR_POISON_SCHEMA
        || record.campaign_id != context.campaign_id
        || record.reproduction_id != context.reproduction_id
        || record.phase != context.phase
        || record.cohort_manifest_sha256 != context.cohort_manifest_sha256
        || record.case_id != context.case_id
        || record.task_sha256 != context.task_sha256
        || record.prompt_sha256 != context.prompt_sha256
        || record.inventory_reward_contract_sha256 != context.inventory_reward_contract_sha256
        || record.outcome_reward_contract_sha256 != context.outcome_reward_contract_sha256
        || record.policy_version != context.policy_version
        || record.sample_index != context.sample_index
        || record.pair_id != canonical_receipt_pair_id(context)
        || record.failure.is_empty()
        || record.failure.len() > 16 * 1024
        || record.failure.contains('\0')
        || record.record_sha256 != pair_poison_sha256(&record)?
    {
        return Err("receipt pair poison authority context mismatch".into());
    }
    Ok(record)
}

fn pair_poison_sha256(record: &PairPoisonRecord) -> Result<String, String> {
    let mut unsigned = record.clone();
    unsigned.record_sha256.clear();
    serde_json::to_vec(&unsigned)
        .map(|bytes| crate::cut::sha256_hex(&bytes))
        .map_err(|error| format!("could not hash receipt pair poison: {error}"))
}

pub(crate) struct TerminalReleaseSnapshot {
    pub bytes: Vec<u8>,
    pub receipt_ledger_head_sha256: String,
}

pub(crate) struct CampaignExecutionLock(LedgerLock);

pub(crate) fn acquire_campaign_execution_lock(
    root: &Path,
) -> Result<CampaignExecutionLock, String> {
    ensure_ledger_root(root)?;
    let path = root.join(CAMPAIGN_EXECUTION_LOCK_FILE);
    reject_non_regular_existing(&path)?;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("could not open campaign execution lock: {error}"))?;
    LedgerLock::acquire(file).map(CampaignExecutionLock)
}

pub struct EvaluatorArtifactScoreRequest<'a> {
    pub path: &'a Path,
    pub reward: &'a dyn Reward,
    pub expected: &'a ExpectedReceiptContext<'a>,
}

pub struct ReceiptPairConsumptionRequest<'a> {
    pub inventory: EvaluatorArtifactScoreRequest<'a>,
    pub outcome: EvaluatorArtifactScoreRequest<'a>,
    pub count: &'a ReceiptPairCountContext<'a>,
    pub ledger_root: &'a Path,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct ConsumptionRow {
    schema: String,
    receipt_sha256: String,
    execution_id: String,
    subject_sha256: String,
    command_sha256: String,
    verifier_contract_sha256: String,
    execution_policy_sha256: String,
    run_id: String,
    reproduction_id: String,
    attempt_id: String,
    retry_index: u32,
    prior_attempt_id: Option<String>,
    reward_bits: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct PairEvidenceRow {
    receipt_sha256: String,
    execution_id: String,
    subject_sha256: String,
    command_sha256: String,
    verifier_contract_sha256: String,
    execution_policy_sha256: String,
    reward_bits: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct PairConsumptionRow {
    schema: String,
    sequence: u64,
    previous_sha256: String,
    campaign_id: String,
    reproduction_id: String,
    phase: String,
    cohort_manifest_sha256: String,
    case_id: String,
    prompt_sha256: String,
    inventory_reward_contract_sha256: String,
    outcome_reward_contract_sha256: String,
    policy_version: u64,
    sample_index: usize,
    pair_id: String,
    inventory: PairEvidenceRow,
    outcome: PairEvidenceRow,
    row_sha256: String,
}

const PAIR_POISON_SCHEMA: &str = "angel.rlvr.receipt-pair-poison/v2";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PairPoisonRecord {
    schema: String,
    campaign_id: String,
    reproduction_id: String,
    phase: String,
    cohort_manifest_sha256: String,
    case_id: String,
    task_sha256: String,
    prompt_sha256: String,
    inventory_reward_contract_sha256: String,
    outcome_reward_contract_sha256: String,
    policy_version: u64,
    sample_index: usize,
    pair_id: String,
    failure: String,
    record_sha256: String,
}

pub fn replay_evaluator_artifact_in_context(
    path: &Path,
    reward: &dyn Reward,
    expected: &ExpectedReceiptContext<'_>,
) -> Result<ConsumptionOutcome, String> {
    let evidence = load_evaluator_artifact(path)?;
    expected.validate(&evidence)?;
    let reward = reward.score(RewardInput::EvaluatorEvidence(&evidence))?;
    Ok(ConsumptionOutcome::ScoreOnly { reward })
}

pub fn consume_evaluator_artifact(
    path: &Path,
    reward: &dyn Reward,
    expected: &ExpectedReceiptContext<'_>,
    count: &ReceiptCountContext<'_>,
    ledger_root: &Path,
) -> Result<ConsumptionOutcome, String> {
    validate_count_context(count)?;
    let evidence = load_evaluator_artifact(path)?;
    expected.validate(&evidence)?;
    let reward = reward.score(RewardInput::EvaluatorEvidence(&evidence))?;
    if !reward.is_finite() {
        return Err("evaluator artifact reward is non-finite".into());
    }
    let row = ConsumptionRow {
        schema: LEDGER_SCHEMA.to_string(),
        receipt_sha256: evidence.manifest_sha256().to_string(),
        execution_id: evidence.execution_id().to_string(),
        subject_sha256: evidence.subject_sha256().to_string(),
        command_sha256: evidence.command_sha256().to_string(),
        verifier_contract_sha256: evidence.verifier_contract_sha256().to_string(),
        execution_policy_sha256: evidence.execution_policy_sha256().to_string(),
        run_id: count.run_id.to_string(),
        reproduction_id: count.reproduction_id.to_string(),
        attempt_id: count.attempt_id.to_string(),
        retry_index: count.retry_index,
        prior_attempt_id: count.prior_attempt_id.map(str::to_string),
        reward_bits: reward.to_bits(),
    };
    append_consumption(ledger_root, row).map(|counted| {
        if counted {
            ConsumptionOutcome::Counted { reward }
        } else {
            ConsumptionOutcome::IdempotentResume { reward }
        }
    })
}

/// Atomically count the inventory and outcome receipts for one immutable
/// logical sample slot. A resumed execution of the same candidate/context
/// returns the original receipt identities; a different candidate in the same
/// slot collides and fails closed instead of increasing the held-out N.
pub fn consume_evaluator_artifact_pair(
    request: ReceiptPairConsumptionRequest<'_>,
) -> Result<ReceiptPairOutcome, String> {
    let ReceiptPairConsumptionRequest {
        inventory: inventory_request,
        outcome: outcome_request,
        count,
        ledger_root,
    } = request;
    validate_pair_context(count)?;
    let inventory = load_evaluator_artifact(inventory_request.path)?;
    inventory_request.expected.validate(&inventory)?;
    let inventory_score = inventory_request
        .reward
        .score(RewardInput::EvaluatorEvidence(&inventory))?;
    let outcome = load_evaluator_artifact(outcome_request.path)?;
    outcome_request.expected.validate(&outcome)?;
    let outcome_score = outcome_request
        .reward
        .score(RewardInput::EvaluatorEvidence(&outcome))?;
    if !inventory_score.is_finite() || !outcome_score.is_finite() {
        return Err("evaluator receipt pair reward is non-finite".into());
    }
    if inventory.manifest_sha256() == outcome.manifest_sha256()
        || inventory.execution_id() == outcome.execution_id()
        || inventory.subject_sha256() == outcome.subject_sha256()
    {
        return Err("evaluator receipt pair roles must be distinct".into());
    }
    if inventory.workspace_path_sha256() != outcome.workspace_path_sha256()
        || inventory.workspace_before_sha256() != outcome.workspace_before_sha256()
    {
        return Err("evaluator receipt pair does not share initial source state".into());
    }
    let row = PairConsumptionRow {
        schema: PAIR_LEDGER_SCHEMA.to_string(),
        sequence: 0,
        previous_sha256: String::new(),
        campaign_id: count.campaign_id.to_string(),
        reproduction_id: count.reproduction_id.to_string(),
        phase: count.phase.to_string(),
        cohort_manifest_sha256: count.cohort_manifest_sha256.to_string(),
        case_id: count.case_id.to_string(),
        prompt_sha256: count.prompt_sha256.to_string(),
        inventory_reward_contract_sha256: count.inventory_reward_contract_sha256.to_string(),
        outcome_reward_contract_sha256: count.outcome_reward_contract_sha256.to_string(),
        policy_version: count.policy_version,
        sample_index: count.sample_index,
        pair_id: canonical_receipt_pair_id(count),
        inventory: pair_evidence_row(&inventory, inventory_score),
        outcome: pair_evidence_row(&outcome, outcome_score),
        row_sha256: String::new(),
    };
    let (counted, persisted) = append_pair_consumption(ledger_root, row)?;
    Ok(ReceiptPairOutcome {
        counted,
        inventory_reward: f32::from_bits(persisted.inventory.reward_bits),
        outcome_reward: f32::from_bits(persisted.outcome.reward_bits),
        receipt_sha256s: [
            persisted.inventory.receipt_sha256,
            persisted.outcome.receipt_sha256,
        ],
    })
}

fn pair_evidence_row(evidence: &EvaluatorEvidence, reward: f32) -> PairEvidenceRow {
    PairEvidenceRow {
        receipt_sha256: evidence.manifest_sha256().to_string(),
        execution_id: evidence.execution_id().to_string(),
        subject_sha256: evidence.subject_sha256().to_string(),
        command_sha256: evidence.command_sha256().to_string(),
        verifier_contract_sha256: evidence.verifier_contract_sha256().to_string(),
        execution_policy_sha256: evidence.execution_policy_sha256().to_string(),
        reward_bits: reward.to_bits(),
    }
}

fn validate_pair_context(context: &ReceiptPairCountContext<'_>) -> Result<(), String> {
    for (label, value) in [
        ("campaign", context.campaign_id),
        ("reproduction", context.reproduction_id),
        ("phase", context.phase),
        ("cohort manifest", context.cohort_manifest_sha256),
        ("case", context.case_id),
        ("task", context.task_sha256),
        ("prompt", context.prompt_sha256),
        (
            "inventory reward contract",
            context.inventory_reward_contract_sha256,
        ),
        (
            "outcome reward contract",
            context.outcome_reward_contract_sha256,
        ),
    ] {
        if value.is_empty() || value.len() > 256 || value.contains(['\n', '\r']) {
            return Err(format!("receipt pair {label} is invalid"));
        }
    }
    Ok(())
}

pub(crate) fn canonical_receipt_pair_id(context: &ReceiptPairCountContext<'_>) -> String {
    let mut slot = String::new();
    for value in [
        context.cohort_manifest_sha256.to_string(),
        context.phase.to_string(),
        context.case_id.to_string(),
        context.task_sha256.to_string(),
        context.prompt_sha256.to_string(),
        context.policy_version.to_string(),
        context.sample_index.to_string(),
    ] {
        slot.push_str(&value.len().to_string());
        slot.push(':');
        slot.push_str(&value);
        slot.push('\n');
    }
    crate::cut::sha256_hex(slot.as_bytes())
}

fn validate_count_context(context: &ReceiptCountContext<'_>) -> Result<(), String> {
    for (label, value) in [
        ("run", context.run_id),
        ("reproduction", context.reproduction_id),
        ("attempt", context.attempt_id),
    ] {
        if value.is_empty() || value.len() > 256 || value.contains(['\n', '\r']) {
            return Err(format!("receipt {label} id is invalid"));
        }
    }
    match (context.retry_index, context.prior_attempt_id) {
        (0, None) => Ok(()),
        (0, Some(_)) => Err("initial receipt attempt cannot name a prior attempt".into()),
        (_, None) => Err("retry receipt must name its prior attempt".into()),
        (_, Some(prior)) if prior == context.attempt_id => {
            Err("retry receipt cannot name itself as prior attempt".into())
        }
        (_, Some(prior))
            if prior.is_empty() || prior.len() > 256 || prior.contains(['\n', '\r']) =>
        {
            Err("receipt prior attempt id is invalid".into())
        }
        _ => Ok(()),
    }
}

fn append_pair_consumption(
    root: &Path,
    mut row: PairConsumptionRow,
) -> Result<(bool, PairConsumptionRow), String> {
    ensure_ledger_root(root)?;
    let path = root.join(PAIR_LEDGER_FILE);
    reject_non_regular_existing(&path)?;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&path)
        .map_err(|error| format!("could not open receipt pair ledger: {error}"))?;
    let _lock = LedgerLock::acquire_root(root)?;
    let rows = read_pair_rows_recover_tail(&mut file)?;
    if let Some(existing) = rows
        .iter()
        .find(|existing| existing.campaign_id == row.campaign_id && existing.pair_id == row.pair_id)
    {
        if pair_resume_context_matches(existing, &row) {
            return Ok((false, existing.clone()));
        }
        return Err("receipt pair slot is already frozen to different evidence".into());
    }
    let poison_path = root.join("poison").join(format!("{}.json", row.pair_id));
    reject_non_regular_existing(&poison_path)?;
    if poison_path.exists() {
        return Err("receipt pair slot is frozen to a prior evaluator failure".into());
    }
    reject_unsealed(root)?;
    if rows.iter().any(|existing| {
        [&existing.inventory, &existing.outcome]
            .into_iter()
            .any(|old| {
                [&row.inventory, &row.outcome].into_iter().any(|new| {
                    old.receipt_sha256 == new.receipt_sha256 || old.execution_id == new.execution_id
                })
            })
    }) {
        return Err("receipt or execution was already counted in a pair".into());
    }
    let legacy_path = root.join(LEDGER_FILE);
    reject_non_regular_existing(&legacy_path)?;
    if legacy_path.exists() {
        let mut legacy_options = std::fs::OpenOptions::new();
        legacy_options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            legacy_options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut legacy_file = legacy_options
            .open(&legacy_path)
            .map_err(|error| format!("could not open receipt ledger: {error}"))?;
        let legacy_rows = read_rows(&mut legacy_file)?;
        if legacy_rows.iter().any(|existing| {
            [&row.inventory, &row.outcome].into_iter().any(|new| {
                existing.receipt_sha256 == new.receipt_sha256
                    || existing.execution_id == new.execution_id
                    || existing.subject_sha256 == new.subject_sha256
            })
        }) {
            return Err("receipt evidence was already counted in the legacy ledger".into());
        }
    }
    if rows.iter().any(|existing| {
        [&existing.inventory, &existing.outcome]
            .into_iter()
            .any(|old| {
                [&row.inventory, &row.outcome]
                    .into_iter()
                    .any(|new| old.subject_sha256 == new.subject_sha256)
            })
    }) {
        return Err("receipt subject was already counted in another pair slot".into());
    }
    row.sequence = rows.len() as u64;
    row.previous_sha256 = rows
        .last()
        .map_or_else(|| "genesis".to_string(), |prior| prior.row_sha256.clone());
    row.row_sha256 = pair_row_sha256(&row)?;
    file.seek(SeekFrom::End(0))
        .map_err(|error| format!("could not seek receipt pair ledger: {error}"))?;
    let mut bytes = serde_json::to_vec(&row)
        .map_err(|error| format!("could not encode receipt pair consumption: {error}"))?;
    bytes.push(b'\n');
    if file.metadata().map_err(|error| error.to_string())?.len() + bytes.len() as u64
        > MAX_LEDGER_BYTES
    {
        return Err("receipt pair ledger exceeds its bounded size".into());
    }
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("could not append receipt pair consumption: {error}"))?;
    sync_directory(root, "receipt pair ledger")?;
    Ok((true, row))
}

fn pair_resume_context_matches(
    existing: &PairConsumptionRow,
    proposed: &PairConsumptionRow,
) -> bool {
    existing.phase == proposed.phase
        && existing.reproduction_id == proposed.reproduction_id
        && existing.cohort_manifest_sha256 == proposed.cohort_manifest_sha256
        && existing.case_id == proposed.case_id
        && existing.prompt_sha256 == proposed.prompt_sha256
        && existing.inventory_reward_contract_sha256 == proposed.inventory_reward_contract_sha256
        && existing.outcome_reward_contract_sha256 == proposed.outcome_reward_contract_sha256
        && existing.policy_version == proposed.policy_version
        && existing.sample_index == proposed.sample_index
        && pair_evidence_context_matches(&existing.inventory, &proposed.inventory)
        && pair_evidence_context_matches(&existing.outcome, &proposed.outcome)
}

fn pair_evidence_context_matches(existing: &PairEvidenceRow, proposed: &PairEvidenceRow) -> bool {
    existing.subject_sha256 == proposed.subject_sha256
        && existing.command_sha256 == proposed.command_sha256
        && existing.verifier_contract_sha256 == proposed.verifier_contract_sha256
        && existing.execution_policy_sha256 == proposed.execution_policy_sha256
        && existing.reward_bits == proposed.reward_bits
}

fn pair_row_sha256(row: &PairConsumptionRow) -> Result<String, String> {
    let mut unsigned = row.clone();
    unsigned.row_sha256.clear();
    serde_json::to_vec(&unsigned)
        .map(|bytes| crate::cut::sha256_hex(&bytes))
        .map_err(|error| format!("could not hash receipt pair row: {error}"))
}

fn read_pair_rows_recover_tail(
    file: &mut std::fs::File,
) -> Result<Vec<PairConsumptionRow>, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("could not seek receipt pair ledger: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_LEDGER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read receipt pair ledger: {error}"))?;
    if bytes.len() as u64 > MAX_LEDGER_BYTES {
        return Err("receipt pair ledger exceeds its bounded size".into());
    }
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        let complete_len = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        file.set_len(complete_len as u64)
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("could not recover torn receipt pair tail: {error}"))?;
        bytes.truncate(complete_len);
    }
    let mut rows = Vec::new();
    for (sequence, line) in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .enumerate()
    {
        let row: PairConsumptionRow = serde_json::from_slice(line)
            .map_err(|error| format!("invalid receipt pair ledger row: {error}"))?;
        if row.schema != PAIR_LEDGER_SCHEMA
            || row.sequence != sequence as u64
            || row.previous_sha256
                != rows.last().map_or("genesis", |prior: &PairConsumptionRow| {
                    prior.row_sha256.as_str()
                })
            || row.row_sha256 != pair_row_sha256(&row)?
        {
            return Err("receipt pair ledger hash chain is invalid".into());
        }
        rows.push(row);
    }
    Ok(rows)
}

pub(crate) fn receipt_pair_ledger_head(root: &Path) -> Result<String, String> {
    ensure_ledger_root(root)?;
    let _lock = LedgerLock::acquire_root(root)?;
    let path = root.join(PAIR_LEDGER_FILE);
    reject_non_regular_existing(&path)?;
    if !path.exists() {
        return Ok("genesis".to_string());
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("could not open receipt pair ledger: {error}"))?;
    let rows = read_pair_rows_recover_tail(&mut file)?;
    Ok(rows
        .last()
        .map_or_else(|| "genesis".to_string(), |row| row.row_sha256.clone()))
}

/// Seal the authority to one release under the same lock as receipt appends.
/// A competing append either lands first and changes the expected head, or the
/// release lands first and permanently fences all later appends.
pub(crate) fn commit_terminal_release(
    root: &Path,
    expected_head_sha256: &str,
    release_bytes: &[u8],
) -> Result<PathBuf, String> {
    ensure_ledger_root(root)?;
    let _lock = LedgerLock::acquire_root(root)?;
    let pair_path = root.join(PAIR_LEDGER_FILE);
    reject_non_regular_existing(&pair_path)?;
    let observed_head = if pair_path.exists() {
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options
            .open(&pair_path)
            .map_err(|error| format!("could not open receipt pair ledger: {error}"))?;
        read_pair_rows_recover_tail(&mut file)?
            .last()
            .map_or_else(|| "genesis".to_string(), |row| row.row_sha256.clone())
    } else {
        "genesis".to_string()
    };
    if observed_head != expected_head_sha256 {
        return Err("receipt ledger changed before terminal release commit".into());
    }
    let path = root.join(TERMINAL_RELEASE_FILE);
    reject_non_regular_existing(&path)?;
    if path.exists() {
        let existing = std::fs::read(&path)
            .map_err(|error| format!("could not read terminal release: {error}"))?;
        return (existing == release_bytes)
            .then_some(path)
            .ok_or_else(|| "receipt authority already has a different terminal release".into());
    }
    if release_bytes.is_empty() || release_bytes.len() as u64 > MAX_LEDGER_BYTES {
        return Err("terminal release exceeds its bounded size".into());
    }
    let temp = root.join(format!(
        ".terminal-release-{}-{}.tmp",
        std::process::id(),
        TERMINAL_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    reject_non_regular_existing(&temp)?;
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&temp)
        .map_err(|error| format!("could not create terminal release: {error}"))?;
    let result = file
        .write_all(release_bytes)
        .and_then(|_| file.sync_all())
        .and_then(|_| std::fs::rename(&temp, &path));
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("could not commit terminal release: {error}"));
    }
    sync_directory(root, "terminal release")?;
    Ok(path)
}

pub(crate) fn load_terminal_release(
    root: &Path,
) -> Result<Option<TerminalReleaseSnapshot>, String> {
    ensure_ledger_root(root)?;
    let _lock = LedgerLock::acquire_root(root)?;
    let terminal_path = root.join(TERMINAL_RELEASE_FILE);
    reject_non_regular_existing(&terminal_path)?;
    if !terminal_path.exists() {
        return Ok(None);
    }
    let mut terminal_options = std::fs::OpenOptions::new();
    terminal_options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        terminal_options.custom_flags(libc::O_NOFOLLOW);
    }
    let terminal = terminal_options
        .open(&terminal_path)
        .map_err(|error| format!("could not open terminal release: {error}"))?;
    let mut bytes = Vec::new();
    terminal
        .take(MAX_LEDGER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read terminal release: {error}"))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_LEDGER_BYTES {
        return Err("terminal release has invalid bounded size".into());
    }

    let pair_path = root.join(PAIR_LEDGER_FILE);
    reject_non_regular_existing(&pair_path)?;
    let receipt_ledger_head_sha256 = if pair_path.exists() {
        let mut pair_options = std::fs::OpenOptions::new();
        pair_options.read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            pair_options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut pair_file = pair_options
            .open(&pair_path)
            .map_err(|error| format!("could not open receipt pair ledger: {error}"))?;
        read_pair_rows_recover_tail(&mut pair_file)?
            .last()
            .map_or_else(|| "genesis".to_string(), |row| row.row_sha256.clone())
    } else {
        "genesis".to_string()
    };
    Ok(Some(TerminalReleaseSnapshot {
        bytes,
        receipt_ledger_head_sha256,
    }))
}

pub(crate) fn bind_campaign_authority(root: &Path, record: &[u8]) -> Result<(), String> {
    bind_immutable_record(root, Path::new("campaign.json"), record, false)
}

pub(crate) fn load_campaign_candidate(root: &Path) -> Result<Option<Vec<u8>>, String> {
    load_immutable_record(root, Path::new("candidate.plan"), "frozen candidate")
}

/// Install a candidate proposal exactly once and return the authoritative
/// bytes. Concurrent proposers adopt the first fully committed record instead
/// of turning a harmless race into campaign drift.
pub(crate) fn install_or_adopt_campaign_candidate(
    root: &Path,
    record: &[u8],
) -> Result<Vec<u8>, String> {
    if record.is_empty() || record.len() as u64 > MAX_LEDGER_BYTES {
        return Err("immutable campaign record exceeds its bounded size".into());
    }
    ensure_ledger_root(root)?;
    let _lock = LedgerLock::acquire_root(root)?;
    let path = root.join("candidate.plan");
    reject_non_regular_existing(&path)?;
    if path.exists() {
        return read_bounded_record(&path, "frozen candidate")?
            .ok_or_else(|| "frozen candidate disappeared while holding its authority lock".into());
    }
    reject_unsealed(root)?;
    write_immutable_record_locked(root, &path, record)?;
    Ok(record.to_vec())
}

fn load_immutable_record(
    root: &Path,
    relative_path: &Path,
    label: &str,
) -> Result<Option<Vec<u8>>, String> {
    ensure_ledger_root(root)?;
    let _lock = LedgerLock::acquire_root(root)?;
    let path = root.join(relative_path);
    reject_non_regular_existing(&path)?;
    read_bounded_record(&path, label)
}

fn read_bounded_record(path: &Path, label: &str) -> Result<Option<Vec<u8>>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("could not open {label}: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_LEDGER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read {label}: {error}"))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_LEDGER_BYTES {
        return Err(format!("{label} has invalid bounded size"));
    }
    Ok(Some(bytes))
}

fn write_immutable_record_locked(root: &Path, path: &Path, record: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        TERMINAL_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    reject_non_regular_existing(&temp)?;
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&temp)
        .map_err(|error| format!("could not create immutable campaign record: {error}"))?;
    let result = file
        .write_all(record)
        .and_then(|_| file.sync_all())
        .and_then(|_| std::fs::rename(&temp, path));
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("could not bind immutable campaign record: {error}"));
    }
    path.parent()
        .and_then(|parent| std::fs::File::open(parent).ok())
        .ok_or_else(|| "could not open immutable campaign record directory".to_string())?
        .sync_all()
        .map_err(|error| format!("could not sync immutable campaign record: {error}"))?;
    sync_directory(root, "immutable campaign record")
}

pub(crate) fn freeze_evaluation_plan(
    root: &Path,
    plan_id: &str,
    record: &[u8],
) -> Result<(), String> {
    if plan_id.len() != 64 || !plan_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("evaluation plan id is invalid".into());
    }
    bind_immutable_record(
        root,
        &Path::new("plans").join(format!("{plan_id}.plan")),
        record,
        true,
    )
}

pub(crate) fn load_evaluation_plan(root: &Path, plan_id: &str) -> Result<Option<Vec<u8>>, String> {
    validate_plan_id(plan_id)?;
    load_immutable_record(
        root,
        &Path::new("plans").join(format!("{plan_id}.plan")),
        "evaluation plan",
    )
}

pub(crate) fn install_or_adopt_evaluation_plan(
    root: &Path,
    plan_id: &str,
    record: &[u8],
) -> Result<Vec<u8>, String> {
    validate_plan_id(plan_id)?;
    if record.is_empty() || record.len() as u64 > MAX_LEDGER_BYTES {
        return Err("immutable campaign record exceeds its bounded size".into());
    }
    ensure_ledger_root(root)?;
    let _lock = LedgerLock::acquire_root(root)?;
    let plans = root.join("plans");
    match std::fs::symlink_metadata(&plans) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err("evaluation plan parent must be a non-symlink directory".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&plans)
                .map_err(|error| format!("could not create evaluation plan directory: {error}"))?;
            sync_directory(root, "evaluation plan directory")?;
        }
        Err(error) => {
            return Err(format!(
                "could not inspect evaluation plan directory: {error}"
            ));
        }
    }
    let path = plans.join(format!("{plan_id}.plan"));
    reject_non_regular_existing(&path)?;
    if path.exists() {
        return read_bounded_record(&path, "evaluation plan")?
            .ok_or_else(|| "evaluation plan disappeared while holding its authority lock".into());
    }
    reject_unsealed(root)?;
    write_immutable_record_locked(root, &path, record)?;
    Ok(record.to_vec())
}

pub(crate) fn load_phase_report(root: &Path, plan_id: &str) -> Result<Option<Vec<u8>>, String> {
    validate_plan_id(plan_id)?;
    load_immutable_record(
        root,
        &Path::new("reports").join(format!("{plan_id}.json")),
        "phase report",
    )
}

pub(crate) fn install_or_adopt_phase_report(
    root: &Path,
    plan_id: &str,
    record: &[u8],
) -> Result<Vec<u8>, String> {
    validate_plan_id(plan_id)?;
    if record.is_empty() || record.len() as u64 > MAX_LEDGER_BYTES {
        return Err("phase report exceeds its bounded size".into());
    }
    ensure_ledger_root(root)?;
    let _lock = LedgerLock::acquire_root(root)?;
    let reports = root.join("reports");
    match std::fs::symlink_metadata(&reports) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err("phase report parent must be a non-symlink directory".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&reports)
                .map_err(|error| format!("could not create phase report directory: {error}"))?;
            sync_directory(root, "phase report directory")?;
        }
        Err(error) => {
            return Err(format!("could not inspect phase report directory: {error}"));
        }
    }
    let path = reports.join(format!("{plan_id}.json"));
    reject_non_regular_existing(&path)?;
    if path.exists() {
        return read_bounded_record(&path, "phase report")?
            .ok_or_else(|| "phase report disappeared while holding its authority lock".into());
    }
    reject_unsealed(root)?;
    write_immutable_record_locked(root, &path, record)?;
    Ok(record.to_vec())
}

fn validate_plan_id(plan_id: &str) -> Result<(), String> {
    if plan_id.len() != 64 || !plan_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("evaluation plan id is invalid".into());
    }
    Ok(())
}

fn bind_immutable_record(
    root: &Path,
    relative_path: &Path,
    record: &[u8],
    fence_new_after_release: bool,
) -> Result<(), String> {
    if record.is_empty() || record.len() as u64 > MAX_LEDGER_BYTES {
        return Err("immutable campaign record exceeds its bounded size".into());
    }
    ensure_ledger_root(root)?;
    let _lock = LedgerLock::acquire_root(root)?;
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        match std::fs::symlink_metadata(parent) {
            Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
                return Err(
                    "immutable campaign record parent must be a non-symlink directory".into(),
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(parent).map_err(|error| {
                    format!("could not create immutable campaign record directory: {error}")
                })?;
                sync_directory(root, "immutable campaign record")?;
            }
            Err(error) => {
                return Err(format!(
                    "could not inspect immutable campaign record directory: {error}"
                ));
            }
        }
    }
    reject_non_regular_existing(&path)?;
    if path.exists() {
        let existing = std::fs::read(&path)
            .map_err(|error| format!("could not read immutable campaign record: {error}"))?;
        return (existing == record)
            .then_some(())
            .ok_or_else(|| "immutable campaign record drift".into());
    }
    if fence_new_after_release {
        reject_unsealed(root)?;
    }
    let temp = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        TERMINAL_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    reject_non_regular_existing(&temp)?;
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&temp)
        .map_err(|error| format!("could not create immutable campaign record: {error}"))?;
    let result = file
        .write_all(record)
        .and_then(|_| file.sync_all())
        .and_then(|_| std::fs::rename(&temp, &path));
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("could not bind immutable campaign record: {error}"));
    }
    path.parent()
        .and_then(|parent| std::fs::File::open(parent).ok())
        .ok_or_else(|| "could not open immutable campaign record directory".to_string())?
        .sync_all()
        .map_err(|error| format!("could not sync immutable campaign record: {error}"))
}

fn reject_unsealed(root: &Path) -> Result<(), String> {
    let terminal = root.join(TERMINAL_RELEASE_FILE);
    reject_non_regular_existing(&terminal)?;
    (!terminal.exists())
        .then_some(())
        .ok_or_else(|| "receipt authority is sealed by a terminal release".into())
}

fn ensure_ledger_root(root: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(root) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err("receipt ledger root must be a non-symlink directory".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("could not inspect receipt ledger root: {error}")),
    }
    std::fs::create_dir_all(root)
        .map_err(|error| format!("could not create receipt ledger root: {error}"))
}

fn sync_directory(root: &Path, label: &str) -> Result<(), String> {
    std::fs::File::open(root)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("could not sync {label} directory: {error}"))
}

fn append_consumption(root: &Path, row: ConsumptionRow) -> Result<bool, String> {
    ensure_ledger_root(root)?;
    let path = root.join(LEDGER_FILE);
    reject_non_regular_existing(&path)?;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&path)
        .map_err(|error| format!("could not open receipt ledger: {error}"))?;
    let _lock = LedgerLock::acquire_root(root)?;
    let rows = read_rows(&mut file)?;
    if let Some(existing) = rows.iter().find(|existing| {
        existing.run_id == row.run_id
            && existing.reproduction_id == row.reproduction_id
            && existing.attempt_id == row.attempt_id
    }) {
        if existing == &row {
            return Ok(false);
        }
        return Err("receipt attempt resume does not match its persisted row".into());
    }
    reject_unsealed(root)?;
    if rows.iter().any(|existing| {
        existing.receipt_sha256 == row.receipt_sha256 || existing.execution_id == row.execution_id
    }) {
        return Err("receipt or execution was already counted".into());
    }
    let pair_path = root.join(PAIR_LEDGER_FILE);
    reject_non_regular_existing(&pair_path)?;
    if pair_path.exists() {
        let mut pair_options = std::fs::OpenOptions::new();
        pair_options.read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            pair_options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut pair_file = pair_options
            .open(&pair_path)
            .map_err(|error| format!("could not open receipt pair ledger: {error}"))?;
        let pair_rows = read_pair_rows_recover_tail(&mut pair_file)?;
        if pair_rows.iter().any(|existing| {
            [&existing.inventory, &existing.outcome]
                .into_iter()
                .any(|old| {
                    old.receipt_sha256 == row.receipt_sha256
                        || old.execution_id == row.execution_id
                        || old.subject_sha256 == row.subject_sha256
                })
        }) {
            return Err("receipt evidence was already counted in the pair ledger".into());
        }
    }
    if row.retry_index > 0 {
        let prior = rows
            .iter()
            .find(|existing| {
                existing.run_id == row.run_id
                    && existing.reproduction_id == row.reproduction_id
                    && Some(existing.attempt_id.as_str()) == row.prior_attempt_id.as_deref()
            })
            .ok_or_else(|| "retry prior attempt is absent from the receipt ledger".to_string())?;
        if prior.retry_index.checked_add(1) != Some(row.retry_index) {
            return Err("retry index is not consecutive with its prior attempt".into());
        }
    }
    file.seek(SeekFrom::End(0))
        .map_err(|error| format!("could not seek receipt ledger: {error}"))?;
    let mut bytes = serde_json::to_vec(&row)
        .map_err(|error| format!("could not encode receipt consumption: {error}"))?;
    bytes.push(b'\n');
    if file.metadata().map_err(|error| error.to_string())?.len() + bytes.len() as u64
        > MAX_LEDGER_BYTES
    {
        return Err("receipt ledger exceeds its bounded size".into());
    }
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("could not append receipt consumption: {error}"))?;
    sync_directory(root, "receipt ledger")?;
    Ok(true)
}

fn read_rows(file: &mut std::fs::File) -> Result<Vec<ConsumptionRow>, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("could not seek receipt ledger: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_LEDGER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read receipt ledger: {error}"))?;
    if bytes.len() as u64 > MAX_LEDGER_BYTES {
        return Err("receipt ledger exceeds its bounded size".into());
    }
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            let row: ConsumptionRow = serde_json::from_slice(line)
                .map_err(|error| format!("invalid receipt ledger row: {error}"))?;
            (row.schema == LEDGER_SCHEMA)
                .then_some(row)
                .ok_or_else(|| "unsupported receipt ledger schema".to_string())
        })
        .collect()
}

fn reject_non_regular_existing(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            Err("receipt ledger must be a regular non-symlink file".into())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("could not inspect receipt ledger: {error}")),
    }
}

struct LedgerLock(std::fs::File);

impl LedgerLock {
    fn acquire(file: std::fs::File) -> Result<Self, String> {
        use std::os::fd::AsRawFd;
        let fd = file.as_raw_fd();
        // SAFETY: the returned guard owns `file` until after the unlock.
        let result = unsafe { libc::flock(fd, libc::LOCK_EX) };
        (result == 0).then_some(Self(file)).ok_or_else(|| {
            format!(
                "could not lock receipt ledger: {}",
                std::io::Error::last_os_error()
            )
        })
    }

    fn acquire_root(root: &Path) -> Result<Self, String> {
        let path = root.join(LEDGER_LOCK_FILE);
        reject_non_regular_existing(&path)?;
        let mut options = std::fs::OpenOptions::new();
        options.create(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let file = options
            .open(path)
            .map_err(|error| format!("could not open receipt ledger lock: {error}"))?;
        Self::acquire(file)
    }
}

impl Drop for LedgerLock {
    fn drop(&mut self) {
        // SAFETY: the guard owns the advisory lock while this descriptor lives.
        use std::os::fd::AsRawFd;
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}
