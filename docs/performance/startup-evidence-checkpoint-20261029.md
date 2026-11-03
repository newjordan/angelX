# Startup / Blaze evidence checkpoint — 2026-10-29

## Current decision

**Keep Cargo as the automatic source-launch backend. Keep Blaze explicitly experimental.** The fastest possible setup, record startup, current full-host interactive readiness and faster arbitrary edit-to-load are **not qualified**. This is a reconciled status page, not a new latency measurement.

The mandatory iteration-30 audit is at the full report (`reports/research-82abbb708cda5ae57589ee8a17aeaaaa-30.md`, not published). It supersedes unsupported interpretations of older timings without changing historical sources or receipts.

## What is actually available

- Normal source launches still ask Cargo about freshness and fail closed on build errors. Retain normal default features, browser assets and sandbox behavior.
- Existing `fast` profile and prior focused/native/browser build boundaries remain in the worktree. These have different measurement scopes; their savings cannot be added together.
- The invocation-local rustc metadata reuse from iteration 28 is **applied** to `bin/angelX`; its selected-compiler, provisioning and fallback regressions were exercised in that iteration. It is not a persistent compiler-info cache and does not skip Cargo's source check.
- Blaze installer/setup verification and explicit opt-in remain available. The later upstream answer/cache/harness/environment changes are **unapplied research candidates**, not installed compiler improvements. No upstream PR has been submitted by this work.

## Literal remaining delta

| Requested outcome | Evidence presently available | Still needed |
|---|---|---|
| Rapid ordinary startup | Historical source CLI launch around 224 ms; direct-image PTY samples and a later tiny-Cargo launcher pilot | Current full-feature source-launch and composer-ready distribution, with matching source/image identity; the 224 ms receipt predates the launcher change |
| Rapid load after Rust edits | Historical one-pair full-host semantic edit/restore at 11.966/11.869 s | Repeatable current-source edit/restore measurements and defined acceptance budget, without dropping features or stale-image execution |
| Rapid focused/native development | Focused V8 and native-module reload improvements within their explicit boundaries | Do not describe these as arbitrary host-Rust hot reload or a full-host compile multiplier |
| Fast first-time setup | Verified repeated Blaze setup around 179/184 ms normal/offline | Clean first-install baseline, network/download boundary, repeatability and failure recovery; no record-time setup comparison exists |
| Blaze across systems | Explicit pinned installation plus many isolated compatibility/answer controls | Fix and retest full-host stale restore; integrate/compose candidate patches and environment policy through actual callbacks; qualify daemon/fallback behavior, failures, cross-platform paths and actual runtime |
| Long-term bounded storage | Historical 16.41 GiB inode-deduplicated bitcode length outside pack-cache budget | Ownership/reachability-aware storage lifecycle, reviewed cleanup policy; no automatic deletion or guaranteed reclaim estimate |

There is no bound overall loop acceptance command or numeric operator startup/rebuild budget. No ratio in this work establishes global optimality, a cross-machine record, live-provider readiness, GUI/game performance or a fastest-possible guarantee.

## Hard gate before a full-host qualification build

The pre-existing gate is:

```sh
bash scripts/check/check-locks.sh
```

At this checkpoint it exits **1**. Only `cockpit/src/drive/loop_ctl.rs` differs from its seal:

- expected: `cd017dfde2178e21ba3f05855698b703a67f40101f442e99142ad4651dcce733`
- current: `c5fb1562ccad4ff15fd49d6aa399d5bebcb0b5739e951865a6733bce087d7148`

Current bytes equal HEAD. Iteration 16 attributed the change to committed `5071cce6c5fb7a25afe8f803e78dd3fa53d48129`, not a local performance edit. Exact diagnostic patch: `reports/performance/operator-lock-diagnosis-iteration16.patch`. Matching HEAD does not authorize changing the seal. An operator must approve/reconcile that protected state. This iteration does not restore protected source, invoke `--seal`, or bypass the check.

**The seal is not the only blocker:** even after operator reconciliation, historical Blaze full-host restore failure and missing integrated validation remain. Passing compiler-answer matrices does not clear executable publication/freshness.

## Reproduce the inexpensive evidence check

A new source tool reads explicitly named retained artifacts and runs only the existing protection gate plus read-only Git identity checks. It does not invoke Cargo/rustc/Blaze, launch angelX, benchmark, access a provider/network, clear caches or mutate a seal.

```sh
# Pick an unused workspace result path; output is exclusive-create.
python3 -B scripts/perf/audit-startup-evidence.py \
  --output reports/performance/startup-evidence-checkpoint-new.json
```

Exit **0** means the **archival checks** held, even while product qualification is blocked. Read `qualification` and `gate.exit_code`; a green archive audit is not a green product test. Changed/missing schema inputs fail rather than being counted as new performance observations. Output refuses an existing name.

Final retained run: `reports/performance/startup-evidence-checkpoint-iteration30-r3.json`, exit 0, **302/302** same-author archival checks. Raw stderr from the gate is embedded there; gate exit remains **1**. The separate-context review found three gaps in the first auditor; all three were corrected before r2. R3 additionally checks earlier exhibit preservation, the private-fixture/current-launcher boundary, all iteration-29 preservation inputs and a retained existing-entrypoint test receipt. No independent reviewer reran r2/r3 and no full-host build ran.

A fresh selection from the existing launcher regression entrypoint passed **6/6** tests using fake compiler/Cargo fixtures. This is source-launcher correctness coverage, not a real application build or latency measurement. The retained log is `reports/performance/startup-evidence-launcher-tests-iteration30.log`.

## Upstream work worth pursuing, in dependency order

1. Preserve synchronous generation-bound output semantics and reproduce the real full-host stale restore. Hardlinks and a detached settler are suspects, not a proven cause; a broad speculative copy replacement is not justified.
2. Address nested build-script interception / recognized wrapper composition using the retained controls, then existing full-crate integration checks. Separate candidates were not composed here.
3. Propagate explicit inheritance/clear policy through real command constructors and `project_model::Answerer`, or conservatively delegate unknown policy. Iteration29's API prototype alone still takes inherited-only production compatibility paths.
4. Compose the unwind/custom-target/file-input answer-cache fixes, validate together against rustc with persistent-cache controls and complete comparison accounting. Independent passing matrices against the same baseline do not prove composition.
5. Evaluate snapshot/readiness/storage optimizations at their actual call sites after correctness gates pass. Keep the rejected clone-first change rejected unless the mechanism/workload/threshold changes.
6. Once authorized and fresh, measure the same full-feature source workflow with source/output identities and before/after controls. Do not borrow tiny-fixture percentages as product startup savings.
