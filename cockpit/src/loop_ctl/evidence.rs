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
mod tests {
    use super::super::DEFAULT_LOOP_MAX_ITERS;
    use super::*;

    fn block(dir: &str, findings: &[&str]) -> String {
        let mut s = format!("DIRECTION: {dir}\nFINDINGS:\n");
        for f in findings {
            s.push_str(&format!("- {f} [evidence: file:src/loop_ctl.rs:1]\n"));
        }
        s
    }

    #[test]
    fn submission_journal_preserves_attempts_without_counting_receipts_twice() {
        let _env = crate::tests::env_lock();
        let workspace = std::env::temp_dir().join(format!("angel-journal-{}", std::process::id()));
        std::fs::create_dir_all(&workspace).unwrap();
        let _owner =
            crate::harness::run_identity::LiveTurnScope::enter(Some("journal-fixture".into()));
        let _model =
            crate::harness::run_identity::LiveModelScope::enter(Some("deepseek-v4-flash".into()));
        let _ = crate::tools::submit_identity::drain_journal();
        for (exit, output) in [
            (2, "refused"),
            (0, "rejected"),
            (0, "not accepted"),
            (0, "Submission queued\n11111111-2222-3333-4444-555555555555"),
        ] {
            crate::tools::submit_identity::journal_execution(
                "shell",
                "yukon submit",
                Some(&workspace),
                Some(exit),
                output,
            );
        }
        let mut st = LoopState::default();
        drain_submission_journal(&mut st);
        assert!(
            st.submissions_log.is_empty(),
            "unbound loops cannot claim evidence"
        );
        st.workspace = Some(workspace.clone());
        drain_submission_journal(&mut st);
        assert!(
            st.submissions_log.is_empty(),
            "another loop in the same checkout cannot claim evidence"
        );
        st.id = "journal-fixture".into();
        drain_submission_journal(&mut st);
        assert_eq!(st.submissions, 0);
        assert_eq!(
            st.submissions_log
                .iter()
                .map(|row| row.outcome.as_str())
                .collect::<Vec<_>>(),
            ["refused", "rejected", "unknown", "dispatched"]
        );
        let receipts = vec!["submitted:11111111-2222-3333-4444-555555555555".into()];
        assert_eq!(
            register_verified_outcome_actions(&mut st, &receipts),
            (1, 1)
        );
        drain_submission_journal(&mut st);
        assert_eq!(
            register_verified_outcome_actions(&mut st, &receipts),
            (0, 0)
        );
        assert_eq!(st.submissions, 1);
        assert_eq!(st.submissions_log.len(), 4);
    }

    #[test]
    fn apply_reply_accumulates_distinct_and_tracks_stall() {
        let mut st = LoopState::default();
        assert_eq!(apply_reply(&mut st, &block("a", &["f1", "f2"])), 2);
        assert_eq!(apply_reply(&mut st, &block("b", &["f3"])), 1);
        // Repeat of a known finding → no fresh ground → stall climbs.
        assert_eq!(apply_reply(&mut st, &block("c", &["f1"])), 0);
        assert_eq!(st.findings.len(), 3);
        assert_eq!(st.iteration, 3);
        assert_eq!(st.stale_count, 1);
        assert_eq!(st.directions_tried, vec!["a", "b", "c"]);
    }

    #[test]
    fn unsupported_novelty_is_a_hypothesis_not_progress() {
        let mut st = LoopState::default();
        let reply =
            "DIRECTION: speculate\nFINDINGS:\n- top competitors probably use a library call";
        assert_eq!(apply_reply(&mut st, reply), 0);
        assert!(st.findings.is_empty());
        assert_eq!(st.hypotheses.len(), 1);
        assert_eq!(st.stale_count, 1);
        assert_eq!(st.log[0].unverified_findings, 1);
        assert!(
            st.last_setback
                .as_deref()
                .unwrap_or("")
                .contains("unverified")
        );
    }

    #[test]
    fn file_evidence_must_exist_and_prefix_matching_is_case_insensitive() {
        let mut st = LoopState::default();
        assert_eq!(
            apply_reply(
                &mut st,
                "DIRECTION: fake source\nFINDINGS:\n- claim [evidence: file:no-such-evidence.txt:1]",
            ),
            0
        );
        assert!(st.findings.is_empty());

        let mut st = LoopState::default();
        assert_eq!(
            apply_reply(
                &mut st,
                "DIRECTION: real source\nFINDINGS:\n- claim [evidence: File:src/loop_ctl.rs:1]",
            ),
            1
        );
        assert_eq!(st.findings.len(), 1);
    }

    #[test]
    fn coordinator_fallback_is_never_admitted_as_a_finding() {
        let mut st = LoopState::default();
        let reply = "Turbo driver is not reachable: club returned an empty reply (no text and no tool calls)";
        assert_eq!(apply_reply(&mut st, reply), 0);
        assert!(st.findings.is_empty());
        assert!(st.hypotheses.is_empty());
        assert_eq!(st.stale_count, 1);
        assert!(
            st.last_error
                .as_deref()
                .unwrap_or("")
                .contains("not reachable")
        );
    }

    #[test]
    fn error_heavy_tool_chain_cannot_reset_stall_with_fresh_prose() {
        let mut st = LoopState::default();
        let tools = ToolStripSnapshot {
            calls: 4,
            errors: 1,
            incomplete: 0,
            diagnostics: 0,
            costly_actions: Vec::new(),
            outcome_actions: Vec::new(),
            ..Default::default()
        };
        let reply = block("claim after errors", &["a source-backed fact"]);
        assert_eq!(apply_reply_with_tools(&mut st, &reply, &tools), 0);
        assert_eq!(st.findings.len(), 1, "fact remains in the evidence ledger");
        assert_eq!(st.stale_count, 1, "tool churn cannot manufacture progress");
        assert_eq!(st.log[0].tool_errors, 1);
    }

    #[test]
    fn repeated_costly_action_is_persisted_for_the_next_iteration() {
        let mut st = LoopState::default();
        let tools = ToolStripSnapshot {
            calls: 1,
            errors: 0,
            incomplete: 0,
            diagnostics: 0,
            costly_actions: vec!["shell:popcorn submit --mode benchmark candidate.py".into()],
            outcome_actions: vec!["shell:popcorn submit --mode benchmark candidate.py".into()],
            ..Default::default()
        };
        assert_eq!(
            apply_reply_with_tools(&mut st, &block("first", &["m1"]), &tools),
            1
        );
        assert_eq!(
            apply_reply_with_tools(&mut st, &block("repeat", &["m2"]), &tools),
            1
        );
        assert_eq!(st.log[1].duplicate_costly_actions, 1);
        assert!(
            st.last_setback
                .as_deref()
                .unwrap_or("")
                .contains("repeated 1 costly")
        );
    }

    #[test]
    fn podrace_receipts_record_execution_without_objective_progress() {
        let mut st = LoopState {
            podrace: true,
            stale_count: 2,
            ..Default::default()
        };
        let prose_only = ToolStripSnapshot::default();
        assert_eq!(
            apply_reply_with_tools(
                &mut st,
                &block("more reading", &["novel fact"]),
                &prose_only
            ),
            1
        );
        assert_eq!(
            st.stale_count, 3,
            "source-backed prose is not competition progress"
        );

        // An unverified outcome fingerprint (status poll / listing) is still
        // not a measured candidate.
        let polling = ToolStripSnapshot {
            calls: 1,
            outcome_actions: vec!["outcome:shell:git status:result=aa".into()],
            ..Default::default()
        };
        apply_reply_with_tools(&mut st, "DIRECTION: poll status", &polling);
        assert_eq!(st.stale_count, 4, "polling is not competition progress");

        let submitted = ToolStripSnapshot {
            calls: 1,
            verified_outcome_actions: vec![
                "submitted:shell:hilbert submit cand.py:result=deadbeef".into(),
            ],
            ..Default::default()
        };
        apply_reply_with_tools(&mut st, "DIRECTION: submit candidate", &submitted);
        assert_eq!(st.stale_count, 5);
        assert_eq!(st.submissions, 1);
        assert_eq!(
            st.measured_candidates, 0,
            "a submit is a submission, not a local measurement"
        );
        assert_eq!(st.log.last().unwrap().verified_outcome_actions, 1);

        // Replaying the identical receipt must not keep the run alive.
        apply_reply_with_tools(&mut st, "DIRECTION: submit again", &submitted);
        assert_eq!(st.stale_count, 6, "a repeated receipt is not new progress");
        assert_eq!(st.submissions, 1);
    }

    #[test]
    fn podrace_stall_climbs_through_findings_workspace_change_and_status_polls() {
        use std::path::PathBuf;
        let mut st = LoopState {
            podrace: true,
            workspace: Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
            ..Default::default()
        };
        // Seed the remembered fingerprint so the next observation can be
        // desynchronized into a forced workspace change.
        apply_reply_with_tools(&mut st, &block("seed", &[]), &ToolStripSnapshot::default());
        let current = st.last_workspace_fingerprint;
        st.last_workspace_fingerprint = current.map(|f| f.wrapping_add(1));

        // 5 fresh findings + a workspace change + a git status poll.
        let mut tools = ToolStripSnapshot {
            calls: 1,
            ..Default::default()
        };
        tools.outcome_actions = vec!["outcome:shell:git status --short:result=bb".into()];
        let reply = block(
            "micro-optimization direction",
            &["f1", "f2", "f3", "f4", "f5"],
        );
        assert_eq!(apply_reply_with_tools(&mut st, &reply, &tools), 5);
        assert!(
            st.log.last().unwrap().workspace_changed,
            "fixture must force a change"
        );
        assert_eq!(
            st.stale_count, 2,
            "findings, workspace edits, and status polls are not measured candidates"
        );

        // One verified benchmark receipt records execution, not improvement.
        let bench = ToolStripSnapshot {
            calls: 1,
            verified_outcome_actions: vec![
                "measured:shell:./benchmark.sh --local-iterate:result=cafe".into(),
            ],
            ..Default::default()
        };
        apply_reply_with_tools(&mut st, "DIRECTION: measure locally", &bench);
        assert_eq!(st.stale_count, 3);
        assert_eq!(st.measured_candidates, 1);
    }

    #[test]
    fn measured_candidate_row_and_state_round_trip() {
        let mut st = LoopState {
            podrace: true,
            ..Default::default()
        };
        let bench = ToolStripSnapshot {
            calls: 1,
            verified_outcome_actions: vec![
                "measured:shell:./benchmark.sh --local-iterate:result=cafe".into(),
            ],
            ..Default::default()
        };
        apply_reply_with_tools(&mut st, "DIRECTION: measure locally", &bench);
        assert_eq!(st.measured_candidates, 1);
        assert_eq!(st.measured_candidates_n, 1);
        assert_eq!(st.measured_candidates_log.len(), 1);
        assert!(
            st.measured_candidates_log[0]
                .verifier
                .command
                .contains("benchmark.sh")
        );
        let json = serde_json::to_string(&st).expect("ser");
        let back: LoopState = serde_json::from_str(&json).expect("de");
        assert_eq!(back.measured_candidates, 1);
        assert_eq!(back.measured_candidates_log.len(), 1);
        let mut bare = serde_json::to_value(LoopState::default()).expect("default");
        bare["measured_candidates"] = serde_json::json!(3);
        bare["tokens_spent"] = serde_json::json!(7);
        if let Some(obj) = bare.as_object_mut() {
            obj.remove("measured_candidates_log");
            obj.remove("measured_candidates_n");
            obj.remove("submissions_log");
            obj.remove("binary");
            obj.remove("tokens");
        }
        let loaded: LoopState = serde_json::from_value(bare).expect("old shape");
        assert_eq!(loaded.measured_candidates, 3);
        assert!(loaded.measured_candidates_log.is_empty());
        assert_eq!(loaded.tokens_spent, 7);
        assert_eq!(loaded.tokens.total, 0);
    }

    #[test]
    fn verifier_blocked_set_by_failure_and_checkpoint_cleared_by_receipt() {
        let failed = ToolStripSnapshot {
            calls: 1,
            errors: 1,
            verifier_failures: vec![(
                "shell:./benchmark.sh".into(),
                "benchctl measure-job: missing required --golden".into(),
            )],
            ..Default::default()
        };
        let mut st = LoopState {
            podrace: true,
            ..Default::default()
        };
        apply_reply_with_tools(&mut st, "DIRECTION: preflight", &failed);
        assert_eq!(
            st.verifier_blocked.as_deref(),
            Some("benchctl measure-job: missing required --golden")
        );

        // Prose novelty cannot clear a blocked verification path.
        apply_reply_with_tools(
            &mut st,
            &block("explore a new kernel direction", &["novel fact"]),
            &ToolStripSnapshot::default(),
        );
        assert_eq!(
            st.verifier_blocked.as_deref(),
            Some("benchctl measure-job: missing required --golden")
        );

        // A `VERIFY … blocked` / `DECISION … blocked` checkpoint refreshes the
        // diagnostic from the reply itself.
        apply_reply_with_tools(
            &mut st,
            "DIRECTION: report\nDECISION blocked: organizer must reconcile",
            &ToolStripSnapshot::default(),
        );
        assert_eq!(
            st.verifier_blocked.as_deref(),
            Some("blocked: organizer must reconcile")
        );
        apply_reply_with_tools(
            &mut st,
            "VERIFY blocked · benchctl requires --golden\nDIRECTION: x",
            &ToolStripSnapshot::default(),
        );
        assert_eq!(
            st.verifier_blocked.as_deref(),
            Some("blocked · benchctl requires --golden")
        );

        // A verified receipt clears it.
        let ok = ToolStripSnapshot {
            calls: 1,
            verified_outcome_actions: vec![
                "measured:shell:./benchmark.sh --local-iterate:result=11".into(),
            ],
            ..Default::default()
        };
        apply_reply_with_tools(&mut st, "DIRECTION: local benchmark", &ok);
        assert!(st.verifier_blocked.is_none());
        assert_eq!(st.measured_candidates, 1);

        // Ordinary loops keep today's behaviour: no blocker arming.
        let mut ordinary = LoopState::default();
        apply_reply_with_tools(&mut ordinary, "DIRECTION: preflight", &failed);
        assert!(ordinary.verifier_blocked.is_none());
    }

    #[test]
    fn ordinary_loop_credits_novel_tool_outcomes_but_repeated_polling_stays_stale() {
        let mut st = LoopState {
            stale_count: 3,
            stall_stop: 4,
            ..Default::default()
        };
        let outcome = ToolStripSnapshot {
            calls: 69,
            errors: 5,
            outcome_actions: vec!["outcome:status:submission 829".into()],
            ..Default::default()
        };

        apply_reply_with_tools(&mut st, "DIRECTION: inspect terminal score", &outcome);
        assert_eq!(st.stale_count, 0, "a new successful outcome is progress");
        assert_eq!(st.log[0].outcome_progress, 1);
        assert_eq!(st.log[0].novel_outcome_actions, 1);

        apply_reply_with_tools(&mut st, "DIRECTION: inspect terminal score", &outcome);
        assert_eq!(
            st.stale_count, 1,
            "replaying the identical status receipt must not mask a stall"
        );
        assert_eq!(st.log[1].outcome_progress, 1);
        assert_eq!(st.log[1].novel_outcome_actions, 0);
    }

    #[test]
    fn budget_trips_on_iters_and_tokens() {
        let mut st = LoopState {
            max_iters: 2,
            ..Default::default()
        };
        st.iteration = 1;
        assert!(budget_tripped(&st).is_none());
        st.iteration = 2;
        assert!(budget_tripped(&st).is_some(), "max iters trips");

        let st2 = LoopState {
            token_budget: 100,
            tokens_spent: 100,
            ..Default::default()
        };
        assert!(budget_tripped(&st2).is_some(), "token budget trips");

        // Deadline in the past trips; far future doesn't.
        let past = LoopState {
            deadline_secs: 1,
            started_ms: 0,
            ..Default::default()
        };
        assert!(
            budget_tripped(&past).is_some(),
            "elapsed past deadline trips"
        );
        let fresh = LoopState {
            deadline_secs: 100_000,
            started_ms: now_ms(),
            ..Default::default()
        };
        assert!(budget_tripped(&fresh).is_none());
    }

    #[test]
    fn default_loop_has_no_iteration_cap() {
        assert_eq!(
            DEFAULT_LOOP_MAX_ITERS, 0,
            "default loop should not have a hard 25-iteration cap"
        );
    }

    #[test]
    fn yolo_preserves_explicit_loop_runaway_budgets() {
        let _lock = crate::tests::env_lock();
        let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
        let _iters = crate::tests::TestEnvGuard::set("ANGEL_LOOP_MAX_ITERS", "7");
        let _deadline = crate::tests::TestEnvGuard::set("ANGEL_LOOP_DEADLINE_SECS", "123");
        let _tokens = crate::tests::TestEnvGuard::set("ANGEL_LOOP_TOKEN_BUDGET", "456");
        let _stall = crate::tests::TestEnvGuard::set("ANGEL_LOOP_STALL_STOP", "3");
        let _first = crate::tests::TestEnvGuard::set("ANGEL_LOOP_FIRST_CANDIDATE_ITERS", "9");

        let mut configured = LoopState::configured_from_env();
        assert_eq!(configured.max_iters, 7);
        assert_eq!(configured.deadline_secs, 123);
        assert_eq!(configured.token_budget, 456);
        assert_eq!(configured.stall_stop, 3);
        assert_eq!(configured.first_candidate_iters, 9);

        configured.iteration = 7;
        assert_eq!(
            budget_tripped(&configured).as_deref(),
            Some("max iterations (7)")
        );
        configured.iteration = 0;
        configured.tokens_spent = 456;
        assert_eq!(
            budget_tripped(&configured).as_deref(),
            Some("token budget (~456)")
        );
        configured.tokens_spent = 0;
        configured.started_ms = 0;
        assert_eq!(
            budget_tripped(&configured).as_deref(),
            Some("deadline (123s)")
        );
        configured.stale_count = 3;
        assert!(stall_limit_reached(&configured));
    }

    #[test]
    fn says_done_is_tolerant_but_anchored() {
        assert!(says_done("work done\nLOOP_DONE"));
        assert!(says_done("**LOOP_DONE**"));
        assert!(says_done("- LOOP_DONE."));
        assert!(says_done("loopdone")); // case/underscore tolerant
        // Must be its own line, not buried in prose.
        assert!(!says_done("I will write LOOP_DONE when finished"));
        assert!(!says_done("not finished yet"));
    }
}
