---
name: competition-loop
description: Run ranked coding and performance competitions through validated, distinct submissions and official score results. Use when the operator asks to compete, optimize a benchmark, submit candidates, improve a leaderboard score, or continue an unattended competition campaign.
---

# Competition Loop

Optimize for valid score improvement. Research, tool activity, and local claims
are not competition results.

## Bind the real interface

Before a long run, establish from the repository and competition CLI:

- the benchmark work directory and editable paths;
- the current best candidate and score;
- the required validation, submission, and status commands; and
- the operator's deadline, spend, and submission scope.

Exercise each required interface or report its exact failing command and error.
Work only in the authorized repository. Preserve unrelated changes. Credential
changes, destructive sync/reset, paid-resource expansion, and work outside the
named scope still require explicit approval.

Use the platform skill when available. For Hilbert, use `hilbert-cli`, retain
trace capture, and provide the exact model and harness attribution required by
the platform.

## Run the shortest closed loop

1. Select one bottleneck supported by source or measurements.
2. Make the smallest candidate that tests one causal hypothesis.
3. Run only the competition's required validation for that candidate.
4. Submit an eligible, distinct candidate promptly.
5. Capture its candidate identity, submission ID, terminal status, and score.
6. Keep or revert from the official result, then take the next concrete action.

While a submitted candidate is pending, prepare the next concrete candidate
instead of repeatedly polling. Reuse an exact valid local receipt when code and
inputs are unchanged. Preserve failed measurements and rejected mechanisms so
they are not repeated.

## Keep ground truth narrow

- Never resubmit unchanged code. Comments, new IDs, or repeated local timings do
  not make a new candidate.
- Never call a candidate a win without an attributable official score that
  beats the relevant baseline or frontier.
- A local benchmark is evidence for deciding whether to submit, not a board
  result.
- A queued submission is pending, not successful.
- Do not add broad tests, orchestration, agents, or reporting that the
  competition does not require.
- Treat a verified external failure as a blocker only when the next action
  needs operator or platform intervention; include the exact recovery action.

Report only the current candidate, validation receipt, submission ID/status,
score delta, decision, and next action. Say `ZERO SUBMISSIONS` when none exist.
Continue until the operator's score, submission, or deadline condition is met,
or an external blocker prevents further authorized work.
