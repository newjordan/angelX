# H01 iteration 15 — outer-loop boundary: diagnostic stopped at resource guard

**Decision: INCONCLUSIVE_STOP_NO_RETRY. No product promotion.**

## Hypothesis and phase

This is a bounded, offline source-model diagnostic within H01, not a new H02 or H04 experiment, release qualification, live trial, or independent audit. The original eight-iteration development budget is exhausted; this iteration performs safe local analysis and stops at the next authorization gate. No product patch/build/test budget was renewed.

After reading the protocol, preflight, required background, latest report and recorded frontier, the technical research door `q.angel-3d0199f7046f545e561ac9ea` was selected; no game work was resumed. The native plan returned no narrower registered attempts. The existing append-only research index and native session/tool receipts remain the evidence authority. No orchestrator was added.

**H15, prospective classification check:** on thirteen frozen historical error strings, six permanent turn failures would not satisfy the current outer-loop `error_requires_provider_action` predicate; auth/billing/quota would remain positive controls and four transient/quoted-marker strings would remain negatives. Later iteration scheduling would be conditional on existing campaign/goal/other gates, not proof of new traffic. Exploratory source observations and case selection were disclosed before registration; this was not a blinded design.

The exact cases, expected vector, analysis unit, limitations, source identities, finite ceilings and stop rules are in [scope.md](../../artifacts/harness-research/20261015-local/H01/iteration15-outer-loop-boundary/scope.md). Registration was appended before the diagnostic invocation. Existing H01 preregistration, failing deterministic baseline, minimal candidate and matched replay were not re-created. H02 remains unregistered.

## New direction and directly inspected contract conflict

A previously unexamined boundary lies above the HTTP/turn retry mechanism:

- [`retry.rs:229–238`](../../cockpit/src/agent/club/retry.rs) documents stopping the outer loop for an account/configuration error, but its predicate only includes billing/auth/quota checks.
- [`loop_ctl.rs:2223–2236`](../../cockpit/src/drive/loop_ctl.rs) marks an errored iteration for paced continuation and computes a separate `provider_blocked` label using that predicate.
- [`loop_ctl.rs:2333–2346`](../../cockpit/src/drive/loop_ctl.rs) schedules another iteration even on the provider-blocked branch, with at least 60 seconds of pacing if the run remains Running.
- [`loop_ctl.rs:2796–2829`](../../cockpit/src/drive/loop_ctl.rs) checks campaign caps before scheduling; the error retry floor is 60 seconds. Arm, goal, single-flight and other checks still apply. There is no claim of unconditional or infinite sending.
- [`retry_tests.rs`](../../tests/cockpit/loop_ctl/retry_tests.rs), especially `account_and_auth_errors_retry_after_pacing_without_swarm_or_ceremony`, explicitly requires Running, a future wake, and `retry_after_error` after billing/auth/quota. Existing tests also cover continued uncapped error iterations.

The comment/implementation-test disagreement is a **direct exploratory source observation**, not a verified product defect or a mandate to replace the operator's continuation policy. The source excerpts and full-file hashes are retained in [exploratory-source-excerpts.json](../../artifacts/harness-research/20261015-local/H01/iteration15-outer-loop-boundary/exploratory-source-excerpts.json). The thirteen-case evaluation did **not** reach this section of the analyzer, so its expected classification vector remains unmeasured. No cross-iteration send count, recovery probability, route widening or measured loop state transition follows from these excerpts.

## Design, attempted command and result

One source-model diagnostic was registered: source-extracted pure string predicates evaluated by a restricted finite Boolean interpreter; exact historical v3 B1 log strings; unchanged A/B predicate bodies as an attribution limitation; source citations for the scheduling contract. No candidate intervention or new ablation was claimed. Interpreter/source agreement would not be a Rust execution result.

Exact command, recorded by the invocation receipt:

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 \
  /usr/bin/python3 -B \
  artifacts/harness-research/20261015-local/H01/iteration15-outer-loop-boundary/analyze.py
```

- **One invocation, exit 1, no retry.** Child wall time was **0.136808187 seconds**; analyzer-reported wall was **0.103326946 seconds**. Neither is a product-performance measurement.
- **18/19 diagnostic guards** were satisfied. These are bookkeeping/identity guard results, not product-test passes.
- Stop: `AssertionError('resource ceilings')`, before resource hashing/comparison and before predicate extraction or case evaluation.
- The failed guard combines `len(resource_paths) <= 3000` and aggregate resource size `<= 128 MiB`. It did **not** retain the component values. Exact path count, aggregate size and which conjunct failed are **unknown**. No post-stop recount, widened ceiling, repair or retry was performed.
- **0/13 classification cases evaluated.** H15 is **unmeasured**, not confirmed and not refuted.
- Three allowed read-only children completed: `git rev-parse HEAD`, `git diff HEAD`, and the existing `bash scripts/check/cockpit-source-digest.sh`. Their argv, environments, exits, output paths and SHA256s are retained.

[Invocation receipt](../../artifacts/harness-research/20261015-local/H01/iteration15-outer-loop-boundary/invocation-receipt.json), [result](../../artifacts/harness-research/20261015-local/H01/iteration15-outer-loop-boundary/result.json), [guards](../../artifacts/harness-research/20261015-local/H01/iteration15-outer-loop-boundary/checks.json), [traceback](../../artifacts/harness-research/20261015-local/H01/iteration15-outer-loop-boundary/traceback.log), [commands](../../artifacts/harness-research/20261015-local/H01/iteration15-outer-loop-boundary/commands.json), and [negative observations](../../artifacts/harness-research/20261015-local/H01/iteration15-outer-loop-boundary/negative-observations.json) retain the failed attempt. Frozen analyzer SHA256: `b2839b79265206cd7afd099d081645341b76ae3e848859723808ed917a5cd425`.

## Identities and integrity actually checked

| Identity | Observed |
|---|---|
| HEAD | `b7bf34a8a899f9bf20eb241eff45bfc63ae14bb1` |
| Tracked diff against HEAD | Empty; SHA256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| Source identity | `angel-source-v3`, `17796a36ed95e3ca0499087bcfb5a9346ffa321cb89d8ded3d118c64d4617a31` |
| Existing treatment baseline anchor, not re-tested | `7d711a031cc3fbdda9112c1be29a104289a06524` |
| Named release executable | SHA256 `b41f58c70a21cec95412b9bbc2a926cca46f53de54e811d1e4fcf0eca23bf9d4`, 112,760,176 bytes |
| Named preflight helper | SHA256 `1bebf47d10e03b4144788f5e78b058914d73222afbf47b4bb9031cad5b0d9996`, 803,400 bytes |
| Named shared release helper | SHA256 `f0441c4b5910457a231533c9922d6eb52a1b2087e77961a74997ad926b68b177`, 803,400 bytes |

The three named images match the prior exact-path hashes. They were **hashed, not executed**. Historical release build-info is retained as historical: embedded source `5c384d7c…`, release/video, resources `unbound`. The current release remains unqualified for the candidate. This iteration did not rehash the large A/B test executables or qualify their runtime.

At analyzer start, **743/744** indexed references matched. The sole mismatch is still entry 425 for the historical matched-replay report: indexed `a6443c81…`, current `3a29573b…`. No original bytes were recovered or reconstructed, and no old entry was rewritten. Current resource revalidation **did not complete**; iteration14's 1,677-resource result is historical evidence, not a new result. Final preservation and reference accounting are separate closeout records: [final-preservation.json](../../artifacts/harness-research/20261015-local/H01/iteration15-outer-loop-boundary/final-preservation.json) and [final-receipt.json](../../artifacts/harness-research/20261015-local/H01/iteration15-outer-loop-boundary/final-receipt.json).

## Controls, physical outcomes and limitations

The local analysis process explicitly received `ANGEL_YOLO=0`, `ANGEL_YOLO_SMART=0`, `PYTHONDONTWRITEBYTECODE=1`; child git commands also received `GIT_OPTIONAL_LOCKS=0`. These are selected process flags, **not ordinary product effective controls**. The analyzer's Python socket/direct-child audit is best-effort; no kernel or transitive containment proof is claimed. The host-load capture was after the failed guard and was not reached. No latency comparison was made, and no GPU capacity was reserved or expanded.

**New product builds/tests/provider calls/physical sends: zero.** New per-case send counts, recovery/cancellation outcomes, ordinary product controls, task efficacy, token/cost coverage, holdout results and matched-peer results are **null/unmeasured**, not successful zeros. Prior physical send/recovery/cancellation observations remain in the retained replay; they were not rerun. No product source or test fixture was edited, no service started, no credentials read, and no paid/remote/submission/external-publication work was performed. The failed diagnostic is not a new failing Rust regression and establishes no candidate-green result.

Additional limits: author self-review only; exploratory case selection; a source-model interpreter rather than compiled predicates; uncompleted native resource guard; unresolved historical report hash; previously established capsule index coverage defect not re-audited here; no independent validation or repeatability demonstrated for this direction. Neither H01's existing local mechanism qualification nor this failed diagnostic supports leadership claims.

## Decision and next action

**Retain the failure and the exploratory contract evidence. Do not promote, patch, rerun, widen the resource guard, or register H02.** The initial development block is exhausted. Pause further experimental execution at the authorization gate.

The next necessary action is an explicit, finite next-block authorization and an operator decision on the contract: should permanent failures stop only the HTTP/turn budget, or also park the outer loop until explicit repair/resume? Current implementation/tests intentionally retain paced outer continuation even for account errors, so silently treating that as a bug would change policy. A separately registered follow-up must record resource guard component values before stopping and choose an exact bounded input scope; it must not retrospectively repair this attempt. If a product change is then authorized, start with one deterministic regression through the existing outer-loop test entry point, one minimal candidate, transient/cancellation/cap negative controls, and an independently audited finite verifier. Live efficacy and broader qualification remain separately gated.

A local immutable exhibit may link this report with worker status **inconclusive**. That is not host admission, canonical checkpoint success, official verification, or an external publication. No host deposit was executed here.
