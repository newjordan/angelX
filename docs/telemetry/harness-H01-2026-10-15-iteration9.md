# H01 iteration 9 — bounded lineage recovery failed

## Hypothesis and phase

Author-only post-decision lineage check; not independent review, H02, product verification, or a new treatment. Prospective scope and analyzer were appended to the existing native experiment index before one invocation. Baseline remains common base `7d711a031cc3fbdda9112c1be29a104289a06524`; expected candidate commit `b7bf34a8a899f9bf20eb241eff45bfc63ae14bb1`. No source/test change was made in this block.

**Hypothesis L:** recover the originally indexed matched-replay report by exact SHA256 from four bounded prefix/newline variants, any existing capsule member, or at most eight Git-history versions. **Hypothesis C:** after recovery, verify identity/resources and the capsule's advertised native-index coverage. L was a prerequisite; no repair, second invocation, or relaxed endpoint was allowed. Scope: `artifacts/harness-research/20261015-local/H01/iteration9-lineage-amendment/scope.md`.

## Invocation and concrete new evidence

Command (one attempt):

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 /usr/bin/python3 -B artifacts/harness-research/20261015-local/H01/iteration9-lineage-amendment/run-once.py
```

Actual analyzer **exit 1**, parent-observed wall **0.071681 s**; analyzer wall **0.028958 s**, **9/10 checks satisfied**, two read-only Git children. This is a failed analysis, not a green product verifier. The generic tool cue suggesting a pass contradicts the saved actual child receipt and is not used as evidence. Budget was one invocation, 60 seconds and at most 16 direct read-only child commands; no analyzer retry occurred.

New negative finding: the indexed report bytes were **not recovered within this candidate set**:

- Four prefixes before `## Portable evidence`, with 0–3 terminal newlines: all exact hashes differ from the indexed hash. Each candidate hash is retained in `recovery-attempts.json`.
- No capsule member matched the indexed report hash. The archive's outer hash matched its sidecar and the bounded regular/unique/safe-member checks succeeded. Internal member checksums and member/local equality were **not reached**; this is not full capsule verification.
- `git log -n 8 --format=%H -- <report>` returned one revision, candidate commit `b7bf34a8…`. `git show` returned the current report bytes, not the indexed version. Both children exited 0; exact argv, clocks, output hashes, stdout/stderr are retained.

Historical reference 425 remains unchanged:

- Path: `docs/telemetry/harness-H01-2026-10-15-matched-replay.md`
- Indexed SHA256: `a6443c81c3b8a0143f01e84609c0824cb417da626faaaca8e635402f0c6cdf5a`
- Current SHA256: `3a29573ba70d57bf453f197c345a23a852c15cc5923c6480496a3eaa1f782cb9`

The initial **452-entry** index snapshot had **451 matching references**, with only that exact historical mismatch. Snapshot SHA256: `ea3e0b300bd4eff92eb54c8f68ea0e6281cc94fa7fe5d10a9458d9d237ca378a`. Nothing was silently relabeled as repaired. Current report bytes are snapshotted under a new name; no recovered original or delta is claimed.

## Controls and unmeasured outcomes

The recorded invocation environment has `ANGEL_YOLO=0`, `ANGEL_YOLO_SMART=0`, `PYTHONDONTWRITEBYTECODE=1`; retry/Cargo-job variables were unset. Pre-invocation source inspection established that the fixture's `h01_controls` sets retry/helper EnvGuards but does not set YOLO itself. An incorrect draft analyzer assumption was removed **before registration/measurement**; replay YOLO declarations would have been checked separately. Neither source declarations nor those intended flags establish ordinary-task effective controls.

**Zero new** builds, product-test executions, product physical sends, provider calls, services, remote jobs, submissions, external publication, or resource expansion were initiated. **New per-case send counts, recovery and cancellation remain null/unmeasured**; historical replay results were not rerun or revalidated here. Python audit-hook isolation is not a kernel/transitive sandbox claim. No delegation or model-review call was requested.

## Decision, limitations and next action

**L: not supported by the bounded recovery search. C: untested. STOP / no retry / no promotion.** Failure to recover does not establish that the original never existed or that broader local backups lack it; four prefix variants are not an exhaustive reconstruction. No attribution of the edit's time, author, intent or exact delta follows. Current-source/binary/helper/resource/ordinary-control revalidation, internal capsule verification, and native-index coverage analysis were **not reached**. Keep those gates explicitly open rather than borrowing older successful checks.

Earlier product dispositions and all historical evidence remain untouched, but this block does not newly reaffirm them. H02 remains separate and unregistered; there is no release, task-speed, cost, holdout, peer-comparison or leadership claim.

**Next action:** obtain the exact originally indexed report from an identified existing backup/session receipt, with its original provenance, before claiming lineage recovery. If unavailable, retain a permanent unresolved historical-reference annotation and prospectively authorize a separate identity/capsule-coverage check that explicitly does not depend on or repair this missing version. Do not repeat nearby newline guesses or regenerate a report and call it the original. No live/remote gate is opened by either path.

All scripts, scope, pre-invocation logical review, start/terminal receipts, known mismatch, reconstruction hashes, current report snapshot and full traceback are retained at `artifacts/harness-research/20261015-local/H01/iteration9-lineage-amendment/`. `closeout-integrity.json` records the final prefix/hash and workspace-preservation checks separately; its numbers must be read from that receipt, not inferred from this report.
