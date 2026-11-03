# angelX fast development — iteration 45 qualification checkpoint

## Decision

**Use the existing Cargo-backed development path. Keep Blaze explicitly experimental.** The mandatory 15-iteration reconciliation found real, retained launcher improvements, but no current full-application qualification for record startup, fastest first installation, arbitrary Rust hot reload, or a universally faster rebuild backend.

This page supersedes interpretations, not the bytes of earlier reports. It is not a new timing study. Full audit report (`reports/research-82abbb708cda5ae57589ee8a17aeaaaa-45.md`) and machine-readable receipt (`reports/performance/startup-evidence-checkpoint-iteration45-final-r2.json`), both kept with the measurement and not published; [read-only auditor](../../scripts/perf/audit-startup-evidence-iteration45.py).

## Immediate qualification blocker — operator action required

The existing `bash scripts/check/check-locks.sh` exits **1**:

- File: `cockpit/src/drive/loop_ctl.rs`
- Seal expects: `cd017dfde2178e21ba3f05855698b703a67f40101f442e99142ad4651dcce733`
- Current source **and committed HEAD**: `c5fb1562ccad4ff15fd49d6aa399d5bebcb0b5739e951865a6733bce087d7148`

The source is not an uncommitted change to undo. Do not edit protected code, bypass the check, suppress its failure, or reseal it as a performance optimization. Only the operator can approve/dispose of the exact mismatch. No full application build, installation, deployment or benchmark was attempted in this checkpoint.

## What is retained versus experimental

| Surface | Current evidence and state |
|---|---|
| Normal source startup | Cargo freshness checking and build-error refusal remain. Automatic backend is Cargo, even with Blaze installed. |
| Fast Rust profile and focused/native boundaries | Existing worktree changes remain; focused tests or native-module reload are not arbitrary Rust-host hot reload. |
| Invocation-local compiler metadata | Earlier retained launcher change; not a persistent cache and not permission to skip Cargo. |
| Logging clock | Retained Bash builtin-clock optimization on modern Bash; older Bash uses the failure-tolerant `date` fallback. Warm tiny-Cargo pilot only. |
| Short-text launch admission | Retained reduction of Python admission overhead within the guarded input contract. Full R1 test coverage must not be attributed to final R2. |
| Pinned-image hardlink | Retained duplicate-hash elimination when hardlinking; copied-image fallback still verifies source and copy. Large sparse-image pilot, not ordinary rebuild timing. |
| Cargo artifact relocation | **Unresolved live boundary:** the existing launcher can guess a stale executable when Cargo output is redirected. The Python selector failed overhead limits; jq met the overhead bound but remains unqualified/unapplied. Do not infer that a Cargo success authenticates a separately guessed path. |
| Blaze toolchain/answer patches | Retained patches and regression opportunities, **not installed**, composed, or submitted upstream. Linux/reduced or selected upstream-test coverage is not full engine/platform qualification. |
| Blaze completed-root scan suppression | **Rejected:** stable parity and a good synthetic benchmark did not prevent a concurrent namespace schedule from omitting a read and reusing a stale supplied value. |
| Blaze path ownership transfer | **Uninstalled, performance gate failed.** Fewer allocation requests but more retained string capacity. No qualified end-to-end gain. |

## Interpreting the measured launcher gains

The historical pilot results below are separately measured on different fixtures/baselines. **Do not sum percentages, subtract their milliseconds from a historic app launch, or claim a cumulative acceleration.**

| Iteration | Boundary | Median paired reduction; wins | Decision |
|---|---|---|---|
| 31 | Real launcher + tiny warm dependency-free Cargo fixture | 1.778 ms / 6.986%; 19/24 | Retained logging optimization |
| 32 | Same kind of default-path fixture, Python artifact selector | **−9.151 ms / −32.218%; 0/24** | Rejected/unapplied |
| 33 | Same kind of fixture, jq artifact selector | **−0.537 ms / −2.113%; 11/24** | Overhead bound met; not a speedup; unapplied |
| 34 | Same kind of fixture, builtin compiler discovery | 0.461 ms / 1.885%; 14/24 | All three performance thresholds missed; restored |
| 37 | Real launcher + short-text tiny-Cargo fixture | 25.728 ms / 32.283%; 24/24 | Retained within measured scope |
| 38 | Real launcher + warm sparse 256 MiB pinned native image | 132.562 ms / 43.366%; 24/24 | Retained hardlink path optimization |

Median paired reduction is not the difference of arm medians. Iteration 33's passing limit means *acceptable pilot overhead*, not acceleration. A passing diagnostic that detects stale cache reuse means the diagnostic worked, not that the affected compiler candidate is safe.

## Literal delta to the operator's request

1. **Rapid actual startup:** after operator disposition of the seal, measure the current full-feature application from source invocation to composer/terminal-ready. Bind launcher, compiler, assets, features, selected binary and executed image identities. Separate CLI exit time, direct-image startup, warm source launch and genuinely cold launch.
2. **Rapid correct edit-to-load:** use repeatable semantic edit, add, remove and restore cases on the current full host. Check changed/restored output, not just successful compiler status; retain all attempts and define the latency/resource budget before timing. Blaze's previous full-host stale-restore gate is still open.
3. **Fast long-term setup:** measure clean first installation separately from repeated installer reuse, with download/network, offline/failure/recovery and storage boundaries. Existing ~179/184 ms reuse results are not first-install records.
4. **Blaze across systems:** integrate and compose the source-specific answer/toolchain/environment fixes in a clean, pinned upstream tree; exercise real callbacks, cache consumers, generated outputs, relocation, supported compilers and platforms. No default switch until freshness and whole-host gates hold.
5. **Correct artifact identity:** resolve redirected Cargo output selection without weakening diagnostics, feature parity, failure refusal, concurrency/image selection or dependency policy. The jq prototype is a lead, not an installed fix.
6. **Upstream contribution:** prioritize the inter-directive creation **must-MISS regression** and checked-probe/local-refusal tests. Submit only with their documented reduced/full-target scopes and remaining gates; do not market unsafe scan-elision as a speed PR.

There is no bound whole-product acceptance command or record-comparison baseline. The loop remains open; this page does not claim completion.

## Reproduce the archival check, without building

The output name must be unused. The auditor atomically publishes with no replacement, reads explicit files, checks arithmetic and diagnostic receipts, executes the existing iteration44 **read-only** verifier, and reports the existing protection failure separately.

```sh
python3 -B scripts/perf/audit-startup-evidence-iteration45.py \
  --output reports/performance/startup-evidence-iteration45-local-check.json
```

Exit 0 means the archival checks agree with the recorded withheld/rejected verdicts. It is **not** application acceptance. The saved checkpoint has 165 checks, zero archival failures, 91 hashed inputs, and `qualification.product = BLOCKED`. Do not reuse its frozen output filename or rewrite earlier experimental evidence.
