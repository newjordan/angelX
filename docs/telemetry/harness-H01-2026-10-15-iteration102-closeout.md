# H01 closeout 102 — cancelled native snapshot preservation

## Hypothesis, phase and decision

**H01 hypothesis unchanged:** permanent-error classification bounds physical sends while preserving transient recovery, cancellation and actionable diagnostics.

**Phase:** read-only closeout of retained native experiment `1792073197859-3790707`, not a new H01 cell, efficacy experiment, independent audit, or renewed eight-iteration block. The native experiment had already stopped. No candidate is introduced and H02 remains unregistered.

**Decision: H01 BLOCKED / NO PROMOTION.** One previously unverified preservation claim is now supported for the retained snapshot files only. The initial block is exhausted ([iteration18](harness-H01-2026-10-15-iteration18.md)); no finite replacement grant appears in this continuation. This closeout does not authorize further experiments or settle the outer-loop contract.

## Question and bounded check

Question: do the cancelled native experiment's retained baseline and working trees still match its own baseline manifest, and can the retained tool calls be paired with results?

The protocol and preflight were read completely, including protocol §§4 and 7. Native frontier `status` reported 592 nodes; a targeted `plan` query returned no doors or recorded attempts for this closeout query. This is not evidence that no technical route exists or that execution is authorized. No game route was pursued.

Before the comparison, the assistant declared its scope: compare the 3,336 manifest-listed files in both retained trees and identify extras outside root `.git`. The local analysis script limits itself to one process, 30 seconds, 512 MiB hashed and 20,000 directory entries. These are finite closeout limits, not a replacement H01 product-execution grant. There were no subprocesses/network calls in the audit script, builds, product tests, model calls, remote jobs, or resource expansion. A small invocation wrapper used read-only git metadata commands and ran the script once.

## Exact identities and reproduction

Artifact directory, relative to the repository:

`artifacts/harness-research/20261015-local/H01/iteration102-cancelled-snapshot-closeout/`

Native source directory:

`~/.angelX/loop-experiments/1792073197859-3790707/`

Current checkout: `b7bf34a8a899f9bf20eb241eff45bfc63ae14bb1`.

Baseline and working native snapshot identity (no treatment difference):

`5f31a5b3ca44b9acb988f050fab3c623c7435c5ec6684cd8fea6a7dc12bbe4e5`

This is the native compact-JSON manifest hash, **not** an `angel-source-v3` digest, executable identity, resource qualification, or release baseline.

Exact analysis command (exit **0**, once):

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 GIT_OPTIONAL_LOCKS=0 \
/usr/bin/python3 -B artifacts/harness-research/20261015-local/H01/iteration102-cancelled-snapshot-closeout/audit.py
```

The flags describe this local invocation only. They do not establish ordinary product effective controls.

| Retained object | SHA256 |
|---|---|
| Native `baseline-manifest.json` (539,958 bytes) | `2b05e7feb0c4e17a03e8acb26cac80f32acd6115fbda4dfe53229f11e17074f5` |
| Native `tool-events.json` (5,545 bytes) | `49510495a691110e0ed9ca16c6d99c2af0bcf46adb9ef6f616794b0db4c03775` |
| Native `result.json` (856 bytes) | `5f29baf9187d60686d6084b0bd364150ffa8d751f905684031c6565ea2bc016f` |
| Local `audit.py` | `d5c819ad8cef2466cc5547aea9d49b8e126c443935f31b9a81232582da86d9e9` |
| Local `invocation.json` | `fff3c547c52466d4293f673ba4079789f4444f4710ca807982741f1d972a064c` |
| Local `audit.stdout.json` | `91b86be611a7c278856280525cd30c653b985b2f8cb6b00a0d4b43582ab1cd84` |
| Local `audit.stderr.txt` (empty) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| Local `native-source-excerpts.json` | `a9eb4f3e7566114dcfe1fcb5e6dc47348ffe2058bd075f20c4c65d17ae19be2c` |
| Local `receipt.json` | `44c0aea90db91be0a3f4c302d615581350bb7c19bbdebe490b91002f6096f9a4` |

The invocation retains argv, script hash, timestamp, HEAD, environment flags and git status. The receipt links stdout, stderr and source excerpts and retains the actual exit code. Native session tool receipts retain preliminary read/search/metadata operations; the first manifest-inspection command was accidentally oververbose (printed per-entry metadata) and its console output was truncated. A subsequent raw-event print was also truncated. Neither output was used as complete evidence: the bounded audit emitted the complete 3,118-byte machine result. No unsuccessful or truncated preliminary output was relabelled as a test pass.

## Observations

| Check | Observed result |
|---|---|
| Baseline files compared | 3,336; 209,227,014 bytes |
| Working files compared | 3,336; 209,227,014 bytes |
| Hash/executable-bit mismatches | 0 in each tree |
| Missing manifest-listed files | 0 in each tree |
| Extra files outside root `.git` | 0 in each tree |
| Canonical manifest hash vs native result | equal |
| Retained trace | 23 events: 9 calls, 9 results, 5 notices |
| Call types | 8 `read_file`, 1 `shell` |
| Call/result pairing | unique matching IDs, one result per call |
| Tool result execution status | all nine recorded `Succeeded`; verification `NotApplicable` in native trace |
| Event omissions reported by trace | 0; per-field character cap 4,096 |
| Audit resource accounting | 419,546,746 bytes hashed; 7,371 directory entries; 0.352457692 s elapsed |

The elapsed value describes one local file audit; it is **not** a product timing, benchmark speedup, or efficacy endpoint.

The sole recorded shell command in the cancelled native attempt was:

```sh
pwd; git status --short; git rev-parse HEAD; ls -a | head -50; date -u +%FT%TZ
```

The native terminal record remains `model_phase_entered: true`, `stop_reason: token_budget`, empty answer, null rollout/patch/verification/evidence paths. Its error is:

> tool batch not started: could not durably checkpoint intent: experiment reserved token estimate exhausted

`estimated_tokens: 48378` is an internal checkpoint estimate, not measured provider tokens, price, or billing.

## Source-path explanation, not runtime qualification

The two source files inspected match their retained native baseline bytes:

- `cockpit/src/agent/harness/loop_experiment.rs`: SHA256 `a57dbd1dd4c19a5e29e6e139ba8ff18ef64db82869aba8f5a79c31315f64bdd5`.
- `cockpit/src/agent/harness/turn/mod.rs`: SHA256 `cf8bbc34eef9d1ef3d395331d7400444a180f45ac6936df701aa3ebab712b176`.

At `loop_experiment.rs:970–985`, checkpoint accounting uses accumulated message-content bytes divided by four, rounds up, and sets `token_exhausted` and cancellation when the estimate exceeds the request's token budget. The comment explicitly allows conservative repeated checkpoint charging. At `turn/mod.rs:3957–3994`, checkpoint failure returns before that batch's tool dispatcher. At `loop_experiment.rs:1021–1049`, cancellation skips patch export and evaluator execution; at `1199–1206`, token exhaustion selects terminal `token_budget` unless the deadline flag has precedence. Thus this retained terminal label is consistent with the inspected source; a missing patch export alone would not have proved an unchanged snapshot. The file comparison supplies the narrower preservation evidence.

No original request limit was recovered here, no charge-correctness claim is made, and source equality does not prove which executable ran. This is not a retry-path fix or an H01 red/green result.

## Limits, preservation and next action

- Comparison covers file contents and executable bits, not empty directories, other permission bits, timestamps, Git internals, historical transient writes, or external effects.
- It is a sequential local audit, not a frozen atomic snapshot or independent reproduction. The script checks per-read file stability and rechecks small native metadata afterward, but cannot exclude every inter-file race.
- It proves neither parent-loop pause nor OS worker drain, successful global cancellation, or all unrelated-work preservation.
- Current source-v3/binary/helper/resources and effective product controls remain **unqualified**.
- New product edits/builds/tests/executions/provider sends are **0/0/0/0/0**. New per-case physical sends, recovery, cancellation and terminal classes remain **unmeasured/null**.
- The historical row425 report-hash mismatch is unresolved and was not repaired or retried.
- No earlier report/source was changed. The existing index receives a closeout evidence event, not a new/backdated preregistration. A local immutable settlement manifest may name this report; no host deposit, remote publication, submission or release is performed.

**Next action:** obtain a finite replacement local block and choose the H01 boundary (HTTP/turn-only containment versus parking the outer loop until repair/resume). A concrete proposal, **not authorization**, is one iteration / 20 minutes / at most two focused offline test executions and two builds / concurrency one / existing CPU resources only / no GPU, provider, remote or paid work. Within an authorized block, register the exact boundary and test, requalify identities/controls, and then execute the smallest discriminating product regression. Do not rerun this already-conclusive snapshot audit or idle-status queries as a substitute. No H02 registration, promotion, release qualification, holdout result or leadership claim follows from this closeout.
