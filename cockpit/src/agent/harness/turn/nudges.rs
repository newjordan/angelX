use crate::agent::harness::TaskPace;

/// The plain anti-spin redirect: fired once before the hard stop when the model
/// keeps re-emitting the same tool batch. Used when perturbation is disabled.
pub(crate) const SPIN_NUDGE: &str = "[harness-telemetry] You've repeated the same tool call \
    several times with no new result. Change your approach, try a different tool, or give your \
    final answer.";

/// Perturbation injection: instead of a bland "change approach", knock the model
/// out of the local minimum with a concrete cognitive jolt — name the assumption
/// it's leaning on, try the *opposite* hypothesis, or reframe the problem by
/// analogy to a different domain. The default redirect (`ANGEL_SPIN_PERTURB=0`
/// restores [`SPIN_NUDGE`]). Mirrors the auto-research "perturbation on stall"
/// move: a stuck searcher escapes faster by inverting its premise than by being
/// told to try harder.
pub(crate) const SPIN_PERTURBATION: &str = "[harness-telemetry] You've repeated the same tool call several times with no \
    new result — you're stuck in a loop, not converging. Break the pattern deliberately: \
    (1) state the key assumption your current approach depends on, then test the OPPOSITE \
    hypothesis; (2) if that doesn't fit, reframe the problem by analogy to a different \
    domain and see what that suggests; (3) or attack it with a different tool entirely. Do \
    not repeat the previous tool call. If you genuinely cannot make progress, give your \
    best final answer and flag what's unresolved.";

/// Injected once when the model thrashes on errors: stop, read the actual error,
/// check the precondition, then retry differently. Distinct from the anti-spin
/// nudge (which fires on *identical* repeated calls) — this fires when calls keep
/// *failing* even as they change.
pub(crate) const ERROR_NUDGE: &str = "[harness-telemetry] Every tool call in your last several turns failed. Stop and read the \
    actual error messages — they usually name the cause (wrong path, missing file, bad arguments, \
    wrong state). Verify the precondition first (does the file/dir exist? `list_dir`/`find_files` \
    before reading; check the working state) and then retry differently. Repeating failing calls \
    won't help.";

/// Injected once when the model re-reads files it has already read without making
/// any progress (no edit, no command, nothing new examined). Distinct from
/// anti-spin (identical calls) and the error breaker (failing calls): this fires
/// on *succeeding-but-circling* read churn.
#[cfg(test)]
pub(crate) const NOPROGRESS_NUDGE: &str = "[harness-telemetry] You've spent several turns re-reading files you already pulled into \
    context, without making an edit, running a command, or examining anything new. The information \
    is already above — act on it: make the change, run the test/build, or give your answer. If \
    you're missing something specific, search for that one thing rather than re-reading the whole \
    file.";

pub(crate) const FIRST_WRITE_NUDGE: &str = "[harness-telemetry] ACTIONABLE CANDIDATE PROGRESS. Inspection has not yet produced candidate progress. The next \
    tool must mutate the candidate, run the narrow validation implied by current evidence, or \
    report the concrete blocker. Do not wrap another source read in a build/check. Submission is \
    never implied by this hop guard.";

pub(crate) const RAPID_COMPETITION_FIRST_WRITE_NUDGE: &str = "[harness-telemetry] RAPID COMPETITION CANDIDATE PROGRESS. Inspection has not yet produced candidate progress. The next \
    tool must mutate the candidate or follow the explicitly armed submission contract; a board \
    receipt remains legal. Do not wrap another source read in a build/check. If no defensible edit \
    exists, report the concrete blocker.";

pub(crate) const FIRST_WRITE_REJECT_RESULT: &str = "first-write inspection not started — the bounded reconnaissance budget is exhausted. \
    Mutate the candidate, run a narrow validation, read the competition board, or report a \
    concrete blocker; wrapping another read in a build/check does not make it progress. This \
    guard does not authorize or require submission.";

pub(crate) fn first_write_nudge(competition: bool, pace: TaskPace) -> &'static str {
    if competition && pace == TaskPace::Rapid {
        RAPID_COMPETITION_FIRST_WRITE_NUDGE
    } else {
        FIRST_WRITE_NUDGE
    }
}

pub(crate) const PASSIVE_POLL_NUDGE: &str = "[harness-telemetry] PASSIVE WAIT BLOCKED. Passive status/sleep calls in this batch were not \
    started; productive sibling calls still run. Status snapshots and shell sleeps are \
    observations, not candidate progress. If a submission is in flight, the harness watcher \
    already owns its status and will inject WATCHER NOTIFY. Mutate the candidate, run a local \
    preflight/benchmark, submit the current best, or report a concrete blocker before requesting \
    another status snapshot.";

pub(crate) const DEEP_PASSIVE_POLL_NUDGE: &str = "[harness-telemetry] DEEP-SOLVE PASSIVE WAIT BLOCKED. Passive status/sleep calls in this batch were not \
    started; productive sibling calls still run. Advance the evidence chain with analysis, a \
    targeted experiment, candidate work, or an honest blocker. This poll guard is not a request \
    to submit.";

pub(crate) fn passive_poll_nudge(pace: TaskPace) -> &'static str {
    if pace == TaskPace::Deep {
        DEEP_PASSIVE_POLL_NUDGE
    } else {
        PASSIVE_POLL_NUDGE
    }
}

pub(super) const PASSIVE_POLL_RESULT: &str = "passive status/sleep call not started — the passive-wait budget is \
    exhausted. Do useful task work before polling again; in-flight submission status is delivered \
    automatically by WATCHER NOTIFY.";

/// Shared anti-spin identity for suppressed all-passive batches. The deny→retry
/// treadmill varies its polls each hop (different sleep durations, different
/// status args), so per-batch hashing never accumulated and the spin stop
/// never fired — one behavior, one fingerprint.
pub(super) const PASSIVE_TREADMILL_SPIN_FINGERPRINT: u64 = 0x5041_5353_4956_4557; // "PASSIVEW"

pub(crate) const FINAL_VERIFY_NUDGE: &str = "[harness-telemetry] You edited the workspace but have not run a verifier since the latest \
    edit. Before claiming completion, run the smallest relevant `check`, `run_tests`, `lint`, \
    `fmt --check`, or equivalent repository command. One relevant green verifier is sufficient; \
    do not follow it with broader or overlapping checks unless the task explicitly requires them. \
    If verification cannot run, state the \
    concrete blocker and the unverified risk in your final answer. A real verifier attempt, even \
    when red or unavailable, is sufficient evidence for an honest blocker report; another \
    unsupported completion claim may be denied up to the configured bounded limit.";

pub(crate) const TASK_ACCEPT_RED_NUDGE: &str = "[harness-telemetry] The task's operator-pinned acceptance command is still RED. This is a \
    hard completion contract, not an advisory verifier: fix the reported failure before claiming \
    completion. Do not redefine, bypass, mask, or replace the command. If the contract cannot be \
    satisfied, report the concrete blocker; the harness will stop rather than publish a false-green \
    answer.";

pub(crate) const FINAL_MILE_NUDGE: &str = "[harness-telemetry] FINAL-MILE BUDGET ACTIVE. The workspace has changed and the bounded \
    turn is near its horizon. Stop broad inspection. Follow the operator's verification arrangement: \
    if local builds/checks are prohibited or verification is delegated to another host or evaluator, \
    preserve the candidate and report local verification as pending; do not create build manifests \
    or run setup to bypass that arrangement. Otherwise run the smallest relevant verifier now; \
    if it fails, make only the concrete fix supported by its diagnostics, verify again, then \
    return an honest final answer. Do not spend the remaining calls re-reading known context.";

pub(crate) use crate::agent::club::FINAL_MILE_ANSWER_NUDGE;

pub(crate) const POST_EDIT_LOGIC_NUDGE: &str = "[harness-telemetry] Review the edit logically before testing: trace the state transitions, \
    invariants, cleanup/empty cases, and error paths implied by the task. Prefer the code that \
    *implements or emits* the behavior (library/pkg/src machinery) over generated testdata, docs, \
    or example trees. If the bug names multiple surfaces (code + config/markdown/model file), check \
    whether each still needs a change. Respect explicit no-build and externally delegated verification \
    instructions; in those cases finish with the candidate and a truthful pending-verification note. \
    Otherwise, if the edit already covers them, run one smallest relevant \
    verifier from a *pre-existing* project test entry point and finish. Do not invent new tests as proof, stack broader \
    checks without a concrete diagnostic, or thrash the same edit.";

/// Fired when the same mutation signature is re-issued (including failed non-unique
/// multi_edit of identical short snippets). Distinct from anti-spin, which requires
/// the entire tool *batch* bytes to match.
#[cfg(test)]
pub(crate) const MUTATION_THRASH_NUDGE: &str = "[harness-telemetry] MUTATION THRASH. You re-issued the same edit signature multiple times \
    (same path and old/new payload, or the same non-unique short snippet). Stop replaying it. \
    If the tool said 'old is not unique', include the enclosing function or more unique context \
    in `old`. If the edit already applied, run a verifier or change approach. Do not spend the \
    remaining horizon re-applying an identical patch.";

/// Fired when a green verifier only exercises tests the agent created this turn.
pub(crate) const SELF_AUTHORED_VERIFY_NUDGE: &str = "[harness-telemetry] WEAK VERIFICATION. The green check only ran tests or files you created \
    or heavily rewrote this turn. That is not evidence the task's real acceptance tests pass. \
    Prefer pre-existing project test entry points (package test suites, named cases already in \
    the tree). Keep verifying against those, or disclose the residual risk honestly.";

/// Fired when many mutations land only under docs/testdata without core library hits.
#[cfg(test)]
pub(crate) const PERIPHERAL_FANOUT_NUDGE: &str = "[harness-telemetry] PERIPHERAL FAN-OUT. Several edits landed under docs/, testdata/, fixtures, \
    or generated examples without a corresponding change in the implementing library (src/, pkg/, \
    lib/, core/). Find the code that *produces* those artifacts and fix it at the source instead of \
    hand-patching every generated copy.";

/// Fired after a completion-grade green verifier on the current workspace.
/// Further verification is redundant, but protocol steps (commit, artifact
/// dumps) get a bounded grace window before the answer is forced (Roll 09
/// OpenCC: 3/3 tech, 0/3 protocol — the answer must not be lost to thrash).
pub(crate) const GREEN_VERIFY_DONE_NUDGE: &str = "[harness-telemetry] GREEN VERIFIER. A full project verifier just passed on this workspace. \
    Use this result as evidence. Complete every remaining requested deliverable, including \
    additional edits or distinct checks when needed, then give the final answer summarizing the work.";

/// Fired after a winning candidate verification passes in competition mode.
/// Stacking speculative changes onto an unbanked winner risks breaking it and burning slots.
pub(crate) const COMPETITION_WINNER_BANK_NUDGE: &str = "[harness-telemetry] COMPETITION CANDIDATE VERIFIED. A local check passed on this candidate. \
    Preserve the passing candidate and its evidence before speculative edits. If the operator has authorized submission and all required checks pass, \
    submit through the agreed workflow and record the receipt. A local pass is not proof of a leaderboard win; complete remaining requested checks first.";

/// Answer claimed done without any source mutation (Roll 09 portfolio false completion).
pub(crate) const NO_EDIT_ANSWER_NUDGE: &str = "[harness-telemetry] NO WORKSPACE MUTATION. You claimed progress or completion but no source \
    file was changed this turn. If the bug is real, make the smallest edit (or a dependency bump \
    in go.mod / package.json / Cargo.toml when the fix is an upstream library version). If you \
    truly cannot edit, name the concrete blocker. A bare claim that it is already fixed is not \
    enough.";

/// Injected once at the start of competition / handoff / leaderboard turns.
/// Overthinking without submit is the failure mode this guard exists to kill.
/// In-flight slot watching is the harness watcher's job — poll-only hops while
/// a slot is in flight are a competition failure. Wait/poll remains legal as
/// an optional receipt check after WATCHER NOTIFY.
pub(crate) const COMPETITION_ACTION_POSTURE: &str = "[harness-telemetry] COMPETITION CHALLENGE PACE — RAPID. ALWAYS BE IMPROVING. \
    Revolving door: mutate → local preflight → the current BEST goes up to bat (SUBMIT, receipt/ID) → immediately improve the next best. \
    Once a submission is in play the harness watcher owns that slot. A WATCHER NOTIFY arrives with id + status + score/reason; do not poll-wait. \
    Sitting on a prepped submission is a competition failure. Constant output: one bat in flight, the next best being prepped. \
    After notify, one receipt check is optional; then submit-next (best to bat) or improve-candidate. \
    Long monologues or passive waiting do not count. Never wrap builds, engine boots, or benchmarks in `timeout`: \
    the harness has no tool ceiling and reports long tools live; a self-imposed timeout that kills a load mid-flight \
    is the most common cause of \"no verifier result\".";

/// Compact always-on board digest template.
pub(crate) const COMPETITION_WORLD_CARD: &str = "\
[harness-telemetry] COMPETITION WORLD CARD — RAPID (fill from tools; do not invent):
tip: (board tip / living-handoff head — one line)
score: (last measured score or unknown)
slot: (in-flight UUID or none)
status: active | validating | candidate-ready
next action: improve-candidate | local-benchmark | submit-next | check-receipt
Doctrine: ALWAYS BE IMPROVING. Revolving door — best always bats. Watcher owns in-flight status. NEVER sit idle, poll-wait, or hold a prepped submission. Mutate + preflight the next best until WATCHER NOTIFY; then submit-next or improve-candidate.";

pub(crate) const DEEP_COMPETITION_POSTURE: &str = "[harness-telemetry] COMPETITION CHALLENGE PACE — DEEP. \
    This is a slow-burn solve: build a coherent evidence chain, test distinct hypotheses, and \
    converge only when the candidate is defensible. Hop count, first-write pressure, and watcher \
    state are never instructions to submit — but a candidate that passes the local gate IS \
    submitted (receipt/ID) and then improved; depth is a reason to measure more, never a reason \
    to withhold a measured candidate. Never wrap builds, engine boots, or benchmarks in `timeout`: \
    the harness has no tool ceiling, long tools are reported live, and a self-imposed timeout that \
    kills a load mid-flight is the most common cause of \"no verifier result\".";

pub(crate) const DEEP_COMPETITION_WORLD_CARD: &str = "\
[harness-telemetry] COMPETITION WORLD CARD — DEEP (fill from tools; do not invent):
hypothesis: (current falsifiable theory — one line)
evidence: (strongest measurement or unknown)
candidate: untouched | exploring | edited | validating | evidence-ready
remaining uncertainty: (largest unresolved risk)
next action: targeted-research | targeted-experiment | improve-candidate | local-benchmark | final-report
Doctrine: depth before cadence. Continue the evidence chain; never submit solely because the hop counter advanced — and never sit on a candidate that passed the local gate.";

pub(crate) fn competition_posture(pace: TaskPace) -> (&'static str, &'static str) {
    match pace {
        TaskPace::Rapid => (COMPETITION_ACTION_POSTURE, COMPETITION_WORLD_CARD),
        TaskPace::Deep => (DEEP_COMPETITION_POSTURE, DEEP_COMPETITION_WORLD_CARD),
    }
}
