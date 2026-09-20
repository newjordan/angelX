use super::schema::{CampaignRecord, CampaignStatus, CriterionStatus};

const LENS_PREFIX: &str = "[campaign-lens/v1 ";
const LENS_SENTINEL: &str = "[/campaign-lens]";
const MAX_LENS_BYTES: usize = 6 * 1024;
const MAX_LENS_LINES: usize = 80;

pub(crate) fn is_lens_message(content: &str) -> bool {
    content.starts_with(LENS_PREFIX)
}

pub(crate) fn replace_lens_message(history: &mut Vec<crate::club::ChatMsg>, lens: Option<String>) {
    history.retain(|message| {
        !(message.role == crate::club::ChatRole::Harness && is_lens_message(&message.content))
    });
    if let Some(lens) = lens {
        history.push(crate::club::ChatMsg::harness(lens));
    }
}

pub(crate) fn render(record: &CampaignRecord) -> Option<String> {
    if record.status.is_terminal() {
        return None;
    }
    let mut lines = Vec::new();
    lines.push(format!(
        "{LENS_PREFIX}id={} revision={} status={}]",
        record.id,
        record.revision,
        status_label(record.status)
    ));
    lines.push(format!(
        "objective receipt: {} · {}",
        &record.objective_digest[..record.objective_digest.len().min(16)],
        bounded(&record.objective, 1_200)
    ));

    let required = record
        .criteria
        .iter()
        .filter(|criterion| criterion.required)
        .count();
    let verified = record
        .criteria
        .iter()
        .filter(|criterion| criterion.status == CriterionStatus::Verified)
        .count();
    lines.push(format!(
        "criterion truth: {verified}/{required} required verified · {} total",
        record.criteria.len()
    ));

    let visible = record
        .criteria
        .iter()
        .filter(|criterion| {
            criterion.required
                && !matches!(
                    criterion.status,
                    CriterionStatus::Verified | CriterionStatus::Deferred
                )
        })
        .take(2)
        .collect::<Vec<_>>();
    for criterion in &visible {
        lines.push(format!(
            "target {} · {} · {} verifier(s): {}",
            criterion.id,
            criterion_label(criterion.status),
            criterion.verifiers.len(),
            bounded(&criterion.text, 1_200)
        ));
    }
    let hidden = record
        .criteria
        .iter()
        .filter(|criterion| {
            criterion.required
                && !matches!(
                    criterion.status,
                    CriterionStatus::Verified | CriterionStatus::Deferred
                )
        })
        .count()
        .saturating_sub(visible.len());
    if hidden > 0 {
        lines.push(format!(
            "target omissions: {hidden} additional required criteria"
        ));
    }

    if let Some(contract) = &record.active_contract {
        lines.push(format!(
            "active round: {} · contract {} · base {} · targets {}",
            contract.round,
            bounded(&contract.digest, 32),
            bounded(&contract.base_oid, 16),
            contract
                .target_criteria
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    } else {
        lines.push(
            "active round: none · no model, verifier, or Git action is authorized by this lens"
                .to_string(),
        );
    }

    let accepted = record
        .rounds
        .iter()
        .filter(|round| round.state == super::schema::RoundState::Accepted)
        .count();
    let proofs = record
        .criteria
        .iter()
        .map(|criterion| criterion.proof_refs.len())
        .sum::<usize>();
    lines.push(format!(
        "accepted evidence: {accepted} round(s) · {proofs} bounded proof reference(s)"
    ));
    lines.push(
        "authority: operator objective and criteria are immutable here; model prose cannot verify or waive them"
            .to_string(),
    );
    Some(cap_lines(lines))
}

fn cap_lines(lines: Vec<String>) -> String {
    let reserve = LENS_SENTINEL.len() + 1;
    let mut out = String::new();
    for line in lines.into_iter().take(MAX_LENS_LINES.saturating_sub(1)) {
        if out.len() + reserve >= MAX_LENS_BYTES {
            break;
        }
        let remaining = MAX_LENS_BYTES - out.len() - reserve;
        let line = utf8_prefix(&line, remaining.saturating_sub(1));
        if line.is_empty() {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(LENS_SENTINEL);
    debug_assert!(out.len() <= MAX_LENS_BYTES);
    debug_assert!(out.lines().count() <= MAX_LENS_LINES);
    out
}

fn utf8_prefix(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes.min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn bounded(value: &str, max_chars: usize) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let mut chars = normalized.chars();
    let mut out = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        out.push_str(" …");
    }
    out
}

fn status_label(status: CampaignStatus) -> &'static str {
    match status {
        CampaignStatus::Draft => "draft",
        CampaignStatus::Ready => "ready",
        CampaignStatus::Running => "running",
        CampaignStatus::VerifyingRound => "verifying_round",
        CampaignStatus::ReviewingRound => "reviewing_round",
        CampaignStatus::FinalVerifying => "final_verifying",
        CampaignStatus::AwaitingOperator => "awaiting_operator",
        CampaignStatus::Paused => "paused",
        CampaignStatus::ReadyToIntegrate => "ready_to_integrate",
        CampaignStatus::Integrated => "integrated",
        CampaignStatus::Stopped => "stopped",
        CampaignStatus::Failed => "failed",
    }
}

fn criterion_label(status: CriterionStatus) -> &'static str {
    match status {
        CriterionStatus::Pending => "pending",
        CriterionStatus::Active => "active",
        CriterionStatus::Blocked => "blocked",
        CriterionStatus::TechnicallyVerified => "technically_verified",
        CriterionStatus::Verified => "verified",
        CriterionStatus::Deferred => "deferred",
        CriterionStatus::Failed => "failed",
    }
}
