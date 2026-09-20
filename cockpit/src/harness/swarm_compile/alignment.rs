use super::schema::{ReviewVerdict as SwarmReviewVerdict, RunState};
use super::store::compact_scrubbed;
use super::tool::{
    AlignmentIndependence, AlignmentVerdict, CampaignAlignmentReceipt, CampaignAlignmentRequest,
    SwarmCompilerEngine,
};
use crate::harness::{DelegateMode, normalize_club_name, run_git};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

const REVIEW_SCHEMA: &str = "campaign-review/v1";
const MAX_REVIEW_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewDocument {
    schema: String,
    contract_digest: String,
    candidate_oid: String,
    criteria: Vec<CriterionFinding>,
    cited_proof_sha256: Vec<String>,
    summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CriterionFinding {
    criterion_id: String,
    verdict: AlignmentVerdict,
    citations: Vec<String>,
}

#[derive(Debug)]
struct ParsedReview {
    verdict: AlignmentVerdict,
    cited_criteria: Vec<String>,
    cited_proof_sha256: Vec<String>,
    summary: String,
}

impl SwarmCompilerEngine {
    pub(crate) fn campaign_alignment_review(
        &self,
        request: CampaignAlignmentRequest,
        cancelled: &AtomicBool,
    ) -> Result<CampaignAlignmentReceipt, String> {
        validate_request(&request)?;
        ensure_not_cancelled(cancelled)?;
        let before = self.inspect_campaign_base()?;
        let run = self.store.load(&request.swarm_run_id)?;
        let ownership = run
            .campaign
            .as_ref()
            .ok_or_else(|| "swarm run is not owned by a campaign".to_string())?;
        if ownership.campaign_id != request.campaign_id
            || ownership.campaign_revision != request.campaign_revision
            || ownership.round != request.round
            || ownership.contract_digest != request.contract_digest
        {
            return Err("swarm ownership does not match the alignment request".to_string());
        }
        if run.state != RunState::Verified
            || !run.verification.technical_pass
            || !run.verification.review_pass
        {
            return Err("alignment review requires a technically verified swarm run".to_string());
        }
        if run.parked_branch.as_deref() != Some(request.candidate_branch.as_str()) {
            return Err("parked candidate branch does not match campaign state".to_string());
        }
        let branch_oid =
            resolve_exact_commit(Path::new(&run.repo_root), request.candidate_branch.as_str())?;
        let candidate_oid =
            resolve_exact_commit(Path::new(&run.repo_root), request.candidate_oid.as_str())?;
        if branch_oid != candidate_oid || candidate_oid != request.candidate_oid {
            return Err("parked candidate no longer resolves to the reviewed OID".to_string());
        }

        let implementer = run
            .contribution("implementer")
            .ok_or_else(|| "durable swarm run has no implementer contribution".to_string())?;
        let implementer_route = contribution_route(implementer)?;
        let implementer_model = implementer.model_revision.clone().ok_or_else(|| {
            "implementer model revision is unavailable; independent review cannot be proven"
                .to_string()
        })?;
        let technical_reviewer = run.contribution("reviewer").ok_or_else(|| {
            "durable swarm run has no technical reviewer contribution".to_string()
        })?;
        if technical_reviewer.review_verdict != Some(SwarmReviewVerdict::Pass)
            || technical_reviewer.base_oid != request.candidate_oid
        {
            return Err("technical reviewer proof is not bound to the exact candidate".to_string());
        }
        let technical_route = contribution_route(technical_reviewer)?;
        let technical_model = technical_reviewer.model_revision.clone().ok_or_else(|| {
            "technical reviewer model revision is unavailable; acceptance is blocked".to_string()
        })?;
        let technical_independence = classify_independence(
            &technical_route,
            &technical_model,
            &implementer_route,
            &implementer_model,
        );

        let selected = self.select_alignment_route(&implementer.club, &implementer_route)?;
        ensure_not_cancelled(cancelled)?;
        let prompt = alignment_prompt(&request);
        let delegate_result = self.delegate.run_from(
            &selected,
            &prompt,
            DelegateMode::ReadOnly,
            &request.candidate_oid,
        );
        let after = self.inspect_campaign_base();
        let outcome = match delegate_result {
            Ok(outcome) => outcome,
            Err(error) => {
                if after? != before {
                    return Err(
                        "shared workspace or HEAD changed during failed read-only review"
                            .to_string(),
                    );
                }
                return Err(error);
            }
        };
        let ephemeral_branch = outcome.branch.clone();
        let review_result = (|| {
            let after = after?;
            if after != before {
                return Err("shared workspace or HEAD changed during read-only review".to_string());
            }
            let branch_after =
                resolve_exact_commit(Path::new(&run.repo_root), &request.candidate_branch)?;
            if branch_after != request.candidate_oid {
                return Err("parked candidate ref moved during alignment review".to_string());
            }
            ensure_not_cancelled(cancelled)?;
            if outcome.tool_failures != 0 {
                return Err(format!(
                    "read-only alignment reviewer had {} failed tool attempt(s)",
                    outcome.tool_failures
                ));
            }
            if outcome.base_oid != request.candidate_oid || !outcome.diff.trim().is_empty() {
                return Err("read-only reviewer did not remain on the exact candidate".to_string());
            }
            let reviewer_route = outcome.resolved_route.trim().to_string();
            let reviewer_model = outcome.model_revision.clone().ok_or_else(|| {
                "alignment reviewer model revision is unavailable; acceptance is blocked"
                    .to_string()
            })?;
            if reviewer_route.is_empty() || reviewer_model.trim().is_empty() {
                return Err("alignment reviewer returned an incomplete route identity".to_string());
            }
            let independence = classify_independence(
                &reviewer_route,
                &reviewer_model,
                &implementer_route,
                &implementer_model,
            );
            let parsed = parse_alignment_output(&outcome.answer, &request)?;
            let verdict = if parsed.verdict == AlignmentVerdict::Pass
                && matches!(
                    independence,
                    AlignmentIndependence::DifferentRouteAndRevision
                        | AlignmentIndependence::DifferentRevision
                ) {
                AlignmentVerdict::Pass
            } else {
                AlignmentVerdict::Block
            };
            let summary = if verdict == parsed.verdict {
                parsed.summary
            } else {
                compact_scrubbed(
                    &format!(
                        "{}; acceptance blocked because reviewer independence is {independence:?}",
                        parsed.summary
                    ),
                    2_048,
                )
            };
            Ok(CampaignAlignmentReceipt {
                schema: REVIEW_SCHEMA.to_string(),
                verdict,
                reviewer_route,
                reviewer_model_revision: reviewer_model,
                implementer_route,
                implementer_model_revision: implementer_model,
                technical_reviewer_route: technical_route,
                technical_reviewer_model_revision: technical_model,
                technical_review_independence: technical_independence,
                independence,
                reviewed_contract_digest: request.contract_digest.clone(),
                reviewed_candidate_oid: request.candidate_oid.clone(),
                cited_criteria: parsed.cited_criteria,
                cited_proof_sha256: parsed.cited_proof_sha256,
                summary,
                response_sha256: crate::cut::sha256_hex(outcome.answer.as_bytes()),
            })
        })();
        let cleanup = self.delegate.discard_branch(&ephemeral_branch);
        match (review_result, cleanup) {
            (Ok(receipt), Ok(())) => Ok(receipt),
            (Ok(_), Err(error)) => Err(format!(
                "alignment review completed but ephemeral branch cleanup failed: {error}"
            )),
            (Err(error), Ok(())) => Err(error),
            (Err(error), Err(cleanup)) => Err(format!(
                "{error}; additionally failed to clean alignment branch: {cleanup}"
            )),
        }
    }

    fn select_alignment_route(
        &self,
        implementer_club: &str,
        implementer_route: &str,
    ) -> Result<String, String> {
        let mut candidates = self
            .clubs
            .iter()
            .filter(|(label, club)| {
                club.is_available()
                    && self
                        .self_label
                        .as_ref()
                        .is_none_or(|root| root.as_str() != label.as_str())
            })
            .map(|(label, _)| label.clone())
            .collect::<Vec<_>>();
        candidates.sort();
        if candidates.is_empty() {
            return Err("no available non-root route can perform alignment review".to_string());
        }
        let implementer_club = normalize_club_name(implementer_club);
        let implementer_route = normalize_club_name(implementer_route);
        let independent = candidates
            .iter()
            .filter(|route| {
                route.as_str() != implementer_club && route.as_str() != implementer_route
            })
            .cloned()
            .collect::<Vec<_>>();
        let pool = if independent.is_empty() {
            &candidates
        } else {
            &independent
        };
        self.policy
            .select("campaign-alignment", "reviewer", pool)
            .or_else(|_| {
                pool.first()
                    .cloned()
                    .ok_or_else(|| "no alignment review route remains".to_string())
            })
    }
}

fn validate_request(request: &CampaignAlignmentRequest) -> Result<(), String> {
    if !request.campaign_id.starts_with("cmp-")
        || request.campaign_id.len() > 96
        || !request
            .campaign_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
        || request.campaign_revision == 0
        || request.round == 0
        || request.contract_digest.len() != 64
        || !request.swarm_run_id.starts_with("swr-")
        || request.swarm_run_id.len() > 96
        || !request
            .swarm_run_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
        || request.candidate_branch.is_empty()
        || request.candidate_branch.len() > 255
        || request.candidate_branch.chars().any(char::is_whitespace)
        || request.candidate_branch.contains("..")
        || request.candidate_oid.len() < 7
        || request.candidate_oid.len() > 64
        || request.criteria.is_empty()
        || request.criteria.len() > 2
        || request.proofs.is_empty()
        || request.proofs.len() > 128
    {
        return Err("invalid campaign alignment request identity".to_string());
    }
    if !request
        .contract_digest
        .chars()
        .chain(request.candidate_oid.chars())
        .all(|character| character.is_ascii_hexdigit())
    {
        return Err("campaign alignment request contains a non-hex identity".to_string());
    }
    let criteria = request
        .criteria
        .iter()
        .map(|criterion| criterion.id.as_str())
        .collect::<BTreeSet<_>>();
    let proofs = request
        .proofs
        .iter()
        .map(|proof| proof.sha256.as_str())
        .collect::<BTreeSet<_>>();
    if criteria.len() != request.criteria.len()
        || proofs.len() != request.proofs.len()
        || request.criteria.iter().any(|criterion| {
            criterion.id.is_empty()
                || criterion.id.len() > 16
                || criterion.text.trim().is_empty()
                || criterion.text.len() > 4_096
        })
        || request.proofs.iter().any(|proof| {
            proof.sha256.len() != 64
                || !proof
                    .sha256
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
                || proof.summary.trim().is_empty()
        })
    {
        return Err("campaign alignment request has invalid criteria or proofs".to_string());
    }
    Ok(())
}

fn parse_alignment_output(
    raw: &str,
    request: &CampaignAlignmentRequest,
) -> Result<ParsedReview, String> {
    if raw.is_empty() || raw.len() > MAX_REVIEW_OUTPUT_BYTES || raw.contains('\0') {
        return Err("alignment review output is empty or oversized".to_string());
    }
    let lines = raw.lines().collect::<Vec<_>>();
    let anchored = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            let line: &str = line;
            matches!(line, "CAMPAIGN_REVIEW: PASS" | "CAMPAIGN_REVIEW: BLOCK")
        })
        .collect::<Vec<_>>();
    if anchored.len() != 1 {
        return Err("alignment review must contain exactly one anchored verdict".to_string());
    }
    let (verdict_index, verdict_line) = anchored[0];
    let last_nonempty = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .ok_or_else(|| "alignment review output is empty".to_string())?;
    if verdict_index != last_nonempty
        || lines.iter().any(|line| {
            line.trim_start().starts_with("CAMPAIGN_REVIEW:")
                && !matches!(
                    line.trim(),
                    "CAMPAIGN_REVIEW: PASS" | "CAMPAIGN_REVIEW: BLOCK"
                )
        })
    {
        return Err("alignment verdict must be the sole exact final marker".to_string());
    }
    let verdict = if *verdict_line == "CAMPAIGN_REVIEW: PASS" {
        AlignmentVerdict::Pass
    } else {
        AlignmentVerdict::Block
    };
    let json = lines[..verdict_index].join("\n");
    let document: ReviewDocument = serde_json::from_str(json.trim())
        .map_err(|error| format!("parse campaign-review/v1 JSON: {error}"))?;
    if document.schema != REVIEW_SCHEMA
        || document.contract_digest != request.contract_digest
        || document.candidate_oid != request.candidate_oid
    {
        return Err("alignment review identity does not match the authorized round".to_string());
    }
    if document.summary.trim().is_empty() || document.summary.len() > 2_048 {
        return Err("alignment review summary is missing or oversized".to_string());
    }

    let expected_criteria = request
        .criteria
        .iter()
        .map(|criterion| criterion.id.as_str())
        .collect::<BTreeSet<_>>();
    let cited_criteria = document
        .criteria
        .iter()
        .map(|finding| finding.criterion_id.as_str())
        .collect::<BTreeSet<_>>();
    if cited_criteria != expected_criteria || cited_criteria.len() != document.criteria.len() {
        return Err("alignment review must cite every targeted criterion exactly once".to_string());
    }
    if document.criteria.iter().any(|finding| {
        finding.citations.is_empty()
            || finding.citations.len() > 16
            || finding
                .citations
                .iter()
                .any(|citation| citation.trim().is_empty() || citation.len() > 512)
    }) {
        return Err("alignment criterion citations are missing or invalid".to_string());
    }
    let all_pass = document
        .criteria
        .iter()
        .all(|finding| finding.verdict == AlignmentVerdict::Pass);
    let any_block = document
        .criteria
        .iter()
        .any(|finding| finding.verdict == AlignmentVerdict::Block);
    if (verdict == AlignmentVerdict::Pass && !all_pass)
        || (verdict == AlignmentVerdict::Block && !any_block)
    {
        return Err("alignment criterion findings contradict the final verdict".to_string());
    }

    let expected_proofs = request
        .proofs
        .iter()
        .map(|proof| proof.sha256.as_str())
        .collect::<BTreeSet<_>>();
    let cited_proofs = document
        .cited_proof_sha256
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if cited_proofs != expected_proofs || cited_proofs.len() != document.cited_proof_sha256.len() {
        return Err("alignment review must cite every authorized proof exactly once".to_string());
    }
    Ok(ParsedReview {
        verdict,
        cited_criteria: document
            .criteria
            .into_iter()
            .map(|finding| finding.criterion_id)
            .collect(),
        cited_proof_sha256: document.cited_proof_sha256,
        summary: document.summary,
    })
}

fn contribution_route(contribution: &super::schema::Contribution) -> Result<String, String> {
    contribution
        .resolved_route
        .clone()
        .filter(|route| !route.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "{} route identity is unavailable; acceptance is blocked",
                contribution.role
            )
        })
}

fn classify_independence(
    reviewer_route: &str,
    reviewer_model: &str,
    implementer_route: &str,
    implementer_model: &str,
) -> AlignmentIndependence {
    let route_differs = !reviewer_route.eq_ignore_ascii_case(implementer_route);
    let revision_differs = reviewer_model != implementer_model;
    match (route_differs, revision_differs) {
        (true, true) => AlignmentIndependence::DifferentRouteAndRevision,
        (false, true) => AlignmentIndependence::DifferentRevision,
        (false, false) => AlignmentIndependence::SameRoute,
        (true, false) => AlignmentIndependence::Unavailable,
    }
}

fn resolve_exact_commit(repo: &Path, reference: &str) -> Result<String, String> {
    if reference.is_empty() || reference.starts_with('-') || reference.len() > 255 {
        return Err("invalid alignment candidate reference".to_string());
    }
    let commit = format!("{reference}^{{commit}}");
    Ok(run_git(
        repo,
        &["rev-parse", "--verify", "--end-of-options", commit.as_str()],
    )?
    .trim()
    .to_string())
}

fn alignment_prompt(request: &CampaignAlignmentRequest) -> String {
    let criteria = request
        .criteria
        .iter()
        .map(|criterion| format!("- {}: {}", criterion.id, criterion.text))
        .collect::<Vec<_>>()
        .join("\n");
    let proofs = request
        .proofs
        .iter()
        .map(|proof| format!("- {}: {}", proof.sha256, proof.summary))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Read and obey the repository scope instructions. Review only; never edit, create, delete, \
         format, commit, or change refs. You are the final alignment gate for the exact parked \
         candidate below.\nContract digest: {}\nCandidate OID: {}\nTarget criteria:\n{}\n\
         Authorized technical proofs:\n{}\n\nInspect the candidate and decide whether it actually \
         satisfies every criterion without scope regressions or proof gaming. Return exactly one \
         raw JSON object (no markdown fence or surrounding prose) with keys: schema, \
         contract_digest, candidate_oid, criteria, cited_proof_sha256, summary. `schema` must be \
         `campaign-review/v1`. Each criteria item must contain criterion_id, verdict (`pass` or \
         `block`), and non-empty concrete citations. Cite every listed criterion and every proof \
         SHA exactly once. Then end with exactly one final line `CAMPAIGN_REVIEW: PASS` only if \
         every criterion passes, otherwise `CAMPAIGN_REVIEW: BLOCK`.",
        request.contract_digest, request.candidate_oid, criteria, proofs
    )
}

fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Acquire) {
        Err("campaign alignment review cancelled by operator".to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/swarm_compile__alignment__tests.rs"]
mod tests;
