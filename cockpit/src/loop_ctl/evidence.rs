//! Pure loop state-machine helpers: evidence admission, reply folding, and
//! budget/stall policy (module-breakup step 3, extracted from `loop_ctl.rs`).
//!
//! Everything here is unit-testable without threads or an `App`: fold one
//! iteration's reply into [`LoopState`], decide which evidence citations are
//! admissible, register costly/outcome actions, and evaluate the budgets that
//! pause a loop. The parent re-exports the functions its `App` methods call.

use crate::iterate::{
    evidence_source, finding_claim, is_placeholder_evidence, normalize, parse_iteration_sections,
};
use crate::sandbox::process_owner::OwnedCommandExt;
use crate::toolstrip::ToolStripSnapshot;
use std::path::Path;

use super::{
    LOOP_EVIDENCE_REVIEW_INTERVAL, LoopBinaryIdentity, LoopIterLog, LoopState, LoopVerifierId,
    MeasuredCandidateRow, SubmissionLogRow, now_ms,
};

#[cfg(test)]
fn apply_reply(st: &mut LoopState, reply: &str) -> usize {
    apply_reply_with_tools(st, reply, &ToolStripSnapshot::default())
}

pub(crate) fn apply_reply_with_tools(
    st: &mut LoopState,
    reply: &str,
    tools: &ToolStripSnapshot,
) -> usize {
    // Whatever setback this reply's iteration was shown, it has had its chance
    // to act on it; a persisting failure re-arms via the verify/error paths.
    st.last_setback = None;
    let non_result = is_non_result_reply(reply);
    let (direction, mut reported, mut hypotheses) = parse_iteration_sections(reply);
    // Prose without the evidence protocol is retained as an unverified lead so
    // the next iteration can validate it, but novelty alone is not progress.
    if reported.is_empty() && hypotheses.is_empty() && direction.trim().is_empty() && !non_result {
        let trimmed = reply.trim();
        if !trimmed.is_empty() {
            hypotheses.push(trimmed.chars().take(1200).collect());
        }
    }

    let reported_count = reported.len();
    let mut fresh_verified = 0;
    let mut unverified = 0;
    if !non_result {
        for finding in reported.drain(..) {
            let key = normalize(finding_claim(&finding));
            if key.is_empty() {
                continue;
            }
            if evidence_is_admissible(&finding, st.workspace.as_deref(), tools) {
                if st.seen.insert(key) {
                    st.findings.push(finding);
                    fresh_verified += 1;
                }
            } else {
                hypotheses.push(finding);
            }
        }
        for hypothesis in hypotheses {
            let key = normalize(finding_claim(&hypothesis));
            if !key.is_empty() && st.seen_hypotheses.insert(key) {
                st.hypotheses.push(hypothesis);
                unverified += 1;
            }
        }
    }

    st.tool_calls_total = st.tool_calls_total.saturating_add(tools.calls);
    st.tool_errors_total = st.tool_errors_total.saturating_add(tools.errors);
    let duplicate_costly_actions = register_costly_actions(st, &tools.costly_actions);
    let unhealthy_tools =
        tools.calls >= 4 && tools.errors.saturating_add(tools.incomplete) * 4 >= tools.calls;
    let credited_fresh = if unhealthy_tools { 0 } else { fresh_verified };
    let workspace_changed = observe_workspace_change(st);
    let outcome_progress = if unhealthy_tools {
        0
    } else {
        tools.outcome_actions.len()
    };
    let novel_outcome_actions = if unhealthy_tools {
        0
    } else {
        register_outcome_actions(st, &tools.outcome_actions)
    };
    // Verified measurement/submission receipts land even when the wider tool
    // chain was error-heavy (a succeeded benchmark is a succeeded benchmark).
    // These execution receipts advance activity clocks, not objective progress.
    register_verified_outcome_actions(st, &tools.verified_outcome_actions);
    drain_submission_journal(st);
    // Blocker-first (podrace): a failed benchmark/verify/validate/submit
    // action or a `VERIFY/DECISION … blocked` checkpoint arms the diagnostic;
    // only a verified receipt clears it — research novelty never does.
    if st.podrace {
        if let Some((_, tail)) = tools.verifier_failures.last() {
            st.verifier_blocked = Some(tail.clone());
        }
        if let Some(diagnostic) = checkpoint_blocked_diagnostic(reply) {
            st.verifier_blocked = Some(diagnostic);
        }
        if !tools.verified_outcome_actions.is_empty() {
            st.verifier_blocked = None;
        }
    }
    // The current tool receipt binds a command and opaque result digest, not
    // candidate bytes, objective, comparable baseline, or an improvement.
    // A new submission id (or timing noise) must not reset objective staleness.
    // Keep execution evidence/counters above; only a future authoritative
    // objective comparison can supply progress credit for ongoing competition.
    let credited_progress = if st.podrace {
        0
    } else {
        credited_fresh + novel_outcome_actions + usize::from(workspace_changed)
    };
    if credited_progress == 0 {
        st.stale_count += 1;
    } else {
        st.stale_count = 0;
    }
    let direction = direction.trim();
    if !direction.is_empty() {
        // A model that repeats its DIRECTION line must not inflate the list —
        // it rides every future prompt (findings already dedup via `seen`).
        let key = normalize(direction);
        if !st.directions_tried.iter().any(|d| normalize(d) == key) {
            st.directions_tried.push(direction.to_string());
        }
    }
    st.iteration += 1;
    st.observe_verifier_failure(tools);
    st.tokens_spent += est_tokens(reply);
    let out = est_tokens(reply) as u64;
    st.tokens.output = st.tokens.output.saturating_add(out);
    st.tokens.total = st.tokens.total.saturating_add(out);
    let evidence_review = st.iteration.is_multiple_of(LOOP_EVIDENCE_REVIEW_INTERVAL);
    st.log.push(LoopIterLog {
        iteration: st.iteration,
        direction: direction.to_string(),
        new_findings: credited_fresh,
        reported_findings: reported_count,
        unverified_findings: unverified,
        tool_calls: tools.calls,
        tool_errors: tools.errors,
        duplicate_costly_actions,
        outcome_progress,
        novel_outcome_actions,
        verified_outcome_actions: tools.verified_outcome_actions.len(),
        workspace_changed,
        evidence_review,
        stale_count: st.stale_count,
        ts_ms: now_ms(),
    });

    if non_result {
        st.last_error = Some(reply.trim().chars().take(400).collect());
        st.last_setback = Some(
            "the previous coordinator reply was an error/status fallback, not a result; restore the driver before claiming progress"
                .to_string(),
        );
    } else if unhealthy_tools {
        st.last_setback = Some(format!(
            "the previous iteration had {} error/incomplete result(s) across {} tool call(s); its prose did not reset stall detection — resolve the failed evidence chain first",
            tools.errors.saturating_add(tools.incomplete),
            tools.calls
        ));
    } else if st.podrace && !tools.verified_outcome_actions.is_empty() {
        st.last_setback = Some(
            "measurement/submission execution recorded; a receipt is not comparable objective improvement. Compare the retained result with the fixed baseline using the available verifier. Repeated competitive submissions of unchanged candidates are banned; a new receipt ID, timestamp, or note does not authorize a redraw. Use the comparison to retire the hypothesis or choose the next concrete mechanism.".to_string(),
        );
    } else if credited_progress == 0 && unverified > 0 {
        st.last_setback = Some(format!(
            "the previous iteration reported {unverified} unverified claim(s); validate them with concrete file/artifact/command/test/URL evidence before treating them as findings"
        ));
    } else if st.podrace && duplicate_costly_actions > 0 {
        st.last_setback = Some(
            "repeated competitive submissions of unchanged candidates are banned; inspect the existing result and change the candidate mechanism before another eligible submission".to_string(),
        );
    } else if duplicate_costly_actions > 0 {
        st.last_setback = Some(format!(
            "the previous iteration repeated {duplicate_costly_actions} costly action(s); justify the replication and compare its result with the earlier artifact before another retry"
        ));
    }
    credited_fresh
}

fn evidence_is_admissible(
    finding: &str,
    workspace: Option<&Path>,
    tools: &ToolStripSnapshot,
) -> bool {
    let Some(source) = evidence_source(finding) else {
        return false;
    };
    if is_placeholder_evidence(source) {
        return false;
    }
    let lower = source.to_ascii_lowercase();
    let successful_tool = tools.calls > tools.errors.saturating_add(tools.incomplete);
    for prefix in ["file:", "artifact:", "benchmark:", "test:"] {
        if lower.starts_with(prefix) {
            let raw_path = &source[prefix.len()..];
            let path = strip_line_suffix(raw_path.trim());
            if path.is_empty() {
                return false;
            }
            let path = Path::new(path);
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                workspace.unwrap_or_else(|| Path::new(".")).join(path)
            };
            return resolved.exists();
        }
    }
    if let Some(url) = lower.strip_prefix("url:") {
        return successful_tool && (url.starts_with("https://") || url.starts_with("http://"));
    }
    ["command:", "tool:"].iter().any(|prefix| {
        lower.starts_with(prefix) && source[prefix.len()..].trim().len() >= 3 && successful_tool
    })
}

fn strip_line_suffix(path: &str) -> &str {
    let Some((base, suffix)) = path.rsplit_once(':') else {
        return path;
    };
    if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit() || c == '-') {
        base
    } else {
        path
    }
}

fn is_non_result_reply(reply: &str) -> bool {
    let lower = reply.to_ascii_lowercase();
    [
        "club returned an empty reply",
        "no text and no tool calls",
        "finish_reason=length",
        "driver is not reachable",
        "worker vanished",
        "stopped before another paid provider request",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub(crate) fn register_costly_actions(st: &mut LoopState, actions: &[String]) -> usize {
    const MAX_ACTIONS: usize = 512;
    let mut duplicates = 0;
    for action in actions {
        if st.costly_actions_seen.contains(action) {
            duplicates += 1;
        } else {
            st.costly_actions_seen.push(action.clone());
        }
    }
    if st.costly_actions_seen.len() > MAX_ACTIONS {
        let drop = st.costly_actions_seen.len() - MAX_ACTIONS;
        st.costly_actions_seen.drain(..drop);
    }
    duplicates
}

/// Admit each successful outcome fingerprint once across the run. The bounded
/// ledger lets a genuinely new mutation/verification/submission/status result
/// reset a stall while repeated polling of the same receipt remains stale.
pub(crate) fn register_outcome_actions(st: &mut LoopState, actions: &[String]) -> usize {
    const MAX_ACTIONS: usize = 512;
    let mut novel = 0;
    for action in actions {
        if !st.outcome_actions_seen.contains(action) {
            st.outcome_actions_seen.push(action.clone());
            novel += 1;
        }
    }
    if st.outcome_actions_seen.len() > MAX_ACTIONS {
        let drop = st.outcome_actions_seen.len() - MAX_ACTIONS;
        st.outcome_actions_seen.drain(..drop);
    }
    novel
}

/// Register verified measurement/submission receipts through the shared
/// outcome ledger (a repeated identical receipt is not a new execution) and
/// advance the run's `measured_candidates` / `submissions` counters. Returns
/// the novel receipts and how many of them were submissions.
pub(crate) fn register_verified_outcome_actions(
    st: &mut LoopState,
    receipts: &[String],
) -> (usize, usize) {
    const MAX_ACTIONS: usize = 512;
    let mut novel = 0;
    let mut novel_submissions = 0;
    for receipt in receipts {
        if st.outcome_actions_seen.contains(receipt) {
            continue;
        }
        st.outcome_actions_seen.push(receipt.clone());
        novel += 1;
        if receipt.starts_with("submitted:") {
            novel_submissions += 1;
        }
    }
    if st.outcome_actions_seen.len() > MAX_ACTIONS {
        let drop = st.outcome_actions_seen.len() - MAX_ACTIONS;
        st.outcome_actions_seen.drain(..drop);
    }
    let measured_new = novel - novel_submissions;
    st.measured_candidates = st.measured_candidates.saturating_add(measured_new);
    st.measured_candidates_n = st.measured_candidates;
    st.submissions = st.submissions.saturating_add(novel_submissions);
    if measured_new > 0 {
        for receipt in receipts {
            if receipt.starts_with("submitted:") {
                continue;
            }
            if st
                .measured_candidates_log
                .iter()
                .any(|row| row.verifier.command == *receipt)
            {
                continue;
            }
            st.measured_candidates_log
                .push(measured_row_from_receipt(st, receipt));
        }
    }
    (novel, novel_submissions)
}

pub(crate) fn loop_binary_identity() -> LoopBinaryIdentity {
    if let Some(id) = crate::harness::run_identity::current() {
        return LoopBinaryIdentity {
            sha256: id.build.executable_sha256.clone(),
            source_digest: id.build.cockpit_source_sha256.clone(),
            build_info: Some(format!(
                "{} {}",
                id.build.toolchain.rustc, id.build.toolchain.profile
            )),
        };
    }
    LoopBinaryIdentity {
        sha256: crate::harness::run_identity::source_sha256().to_string(),
        source_digest: crate::harness::run_identity::source_sha256().to_string(),
        build_info: None,
    }
}

pub(crate) fn refresh_binary(st: &mut LoopState) {
    let next = loop_binary_identity();
    match &st.binary {
        Some(cur) if cur.sha256 == next.sha256 && cur.source_digest == next.source_digest => {}
        _ => st.binary = Some(next),
    }
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output_owned()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

fn measured_row_from_receipt(st: &LoopState, receipt: &str) -> MeasuredCandidateRow {
    let ws = st.workspace.as_deref();
    let candidate_sha = ws
        .map(|d| git_out(d, &["write-tree"]))
        .filter(|s| !s.is_empty())
        .or_else(|| ws.map(|d| git_out(d, &["rev-parse", "HEAD"])))
        .unwrap_or_default();
    let base_sha = st.start_rev.clone().unwrap_or_else(|| {
        ws.map(|d| git_out(d, &["rev-parse", "HEAD"]))
            .unwrap_or_default()
    });
    let command = receipt
        .split(":result=")
        .next()
        .unwrap_or(receipt)
        .to_string();
    let digest = crate::cut::sha256_hex(command.as_bytes());
    MeasuredCandidateRow {
        iteration: st.iteration,
        utc: unix_utc(),
        candidate_sha,
        base_sha,
        local_score: None,
        units: Some("as_printed".into()),
        verifier: LoopVerifierId {
            command,
            sha256: digest,
        },
        binary: st.binary.clone().unwrap_or_else(loop_binary_identity),
    }
}

fn unix_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

pub(crate) fn drain_submission_journal(st: &mut LoopState) {
    let pending =
        crate::tools::submit_identity::drain_workspace_journal(st.workspace.as_deref(), &st.id);
    if pending.is_empty() {
        return;
    }
    for row in pending {
        st.submissions_log.push(SubmissionLogRow {
            iteration: st.iteration,
            utc: row.utc,
            tool: row.tool,
            commit_or_patch_sha: row.commit_or_patch_sha,
            note_file_sha256: row.note_file_sha256,
            model: row.model,
            harness: row.harness,
            exit_code: row.exit_code,
            platform_response_excerpt: row.platform_response_excerpt,
            outcome: row.outcome,
        });
        // Attempts are diagnostics, not verified receipts. The existing
        // register_verified_outcome_actions path owns deduplicated counting.
    }
}

/// The reply's checkpoint declared a blocked verification path — a line
/// starting `VERIFY` / `DECISION` (case-insensitive) that also says
/// `blocked`. The rest of the line (≤200 chars, squashed) is the diagnostic
/// the next iteration must resolve or escalate.
fn checkpoint_blocked_diagnostic(reply: &str) -> Option<String> {
    reply.lines().find_map(|line| {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if !lower.contains("blocked") {
            return None;
        }
        let rest = ["verify", "decision"]
            .iter()
            .find_map(|keyword| lower.strip_prefix(keyword))?;
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        // The keyword occupies `trimmed.len() - rest.len()` bytes; the
        // diagnostic is everything after it.
        let diagnostic = trimmed
            .get(trimmed.len() - rest.len()..)
            .unwrap_or(trimmed)
            .trim();
        let diagnostic = if diagnostic.is_empty() {
            trimmed
        } else {
            diagnostic
        };
        Some(
            diagnostic
                .split_whitespace()
                .collect::<Vec<&str>>()
                .join(" ")
                .chars()
                .take(200)
                .collect(),
        )
    })
}

/// Compare the actual Git-backed workspace state with the preceding iteration.
/// A model need not restate a source edit as a prose finding for the loop to
/// recognize that it changed the candidate. `None` is deliberately neutral for
/// non-Git workspaces, where typed outcome receipts remain the fallback.
pub(crate) fn observe_workspace_change(st: &mut LoopState) -> bool {
    let current = st
        .workspace
        .as_deref()
        .and_then(crate::harness::workspace_fingerprint);
    let changed = matches!(
        (st.last_workspace_fingerprint, current),
        (Some(before), Some(after)) if before != after
    );
    if current.is_some() {
        st.last_workspace_fingerprint = current;
    }
    changed
}

/// Bound a `git diff --stat` for prompt injection: keep the leading file
/// lines and the trailing summary line, eliding the middle — a wide refactor
/// must not flood the iteration prompt.
pub(crate) fn clip_diff_stat(stat: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = stat.lines().collect();
    if lines.len() <= max_lines {
        return stat.to_string();
    }
    let shown = max_lines.saturating_sub(2).max(1);
    let omitted = lines.len().saturating_sub(shown + 1);
    let mut out = lines[..shown].join("\n");
    out.push_str(&format!("\n … ({omitted} more files)\n"));
    out.push_str(lines[lines.len() - 1]);
    out
}

/// Which budget (if any) is exhausted — the loop pauses when one is hit.
pub(crate) fn budget_tripped(st: &LoopState) -> Option<String> {
    if st.max_iters > 0 && st.iteration >= st.max_iters {
        return Some(format!("max iterations ({})", st.max_iters));
    }
    if st.token_budget > 0 && st.tokens_spent >= st.token_budget {
        return Some(format!("token budget (~{})", st.token_budget));
    }
    if st.deadline_secs > 0 {
        let elapsed = now_ms().saturating_sub(st.started_ms) / 1000;
        if elapsed >= st.deadline_secs {
            return Some(format!("deadline ({}s)", st.deadline_secs));
        }
    }
    None
}

pub(crate) fn stall_limit_reached(st: &LoopState) -> bool {
    st.stall_stop > 0 && st.stale_count >= st.stall_stop
}

pub(crate) fn says_done(reply: &str) -> bool {
    reply.lines().any(|l| {
        let t = l
            .trim()
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        t.eq_ignore_ascii_case("LOOP_DONE") || t.eq_ignore_ascii_case("LOOPDONE")
    })
}

/// Rough token estimate (≈4 chars/token) for the cumulative budget guard.
pub(crate) fn est_tokens(s: &str) -> usize {
    s.len() / 4
}

#[cfg(test)]
#[path = "../../../tests/cockpit/loop_ctl/evidence__tests.rs"]
mod tests;
