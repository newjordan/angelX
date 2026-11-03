# Cargo artifact location: correctness before cache relocation

2026-10-29 · iteration 32 · experimental prototype **not applied**

## New, reproduced boundary

The launcher runs Cargo's freshness check but then chooses
`cockpit/target/<profile>/angel` independently of Cargo. With output redirected,
a successful Cargo build can leave that pathname pointing to an older version.
This is a distinct launcher bug, **not** the previously observed Blaze restore
failure.

Using the unmodified current launcher, real Cargo/Rust 1.95.0, a private HOME and
a dependency-free offline crate:

| Configuration after source A → B | Real Cargo output | Launcher output |
| --- | --- | --- |
| Absolute `CARGO_TARGET_DIR`, spaces in path | B | **A** |
| Relative `CARGO_TARGET_DIR` | B | **A** |
| `.cargo/config.toml` `build.target-dir` | B | **A** |
| Configured `build.target` equal to installed host | B | **A** |

Both subprocesses exit successfully in those cases. The build-only case also
reports the old default pathname rather than the directory where C was built.
A syntax-error build still fails closed; this result does not mean all Cargo
build failures run stale code. Reproduction is reduced real-tool evidence,
not a full host rebuild or an independently authored application test.

**Current safe setup:** keep Cargo's default output layout for `angelX` checked
source launches; do not introduce shared/relocated targets or explicit target
layouts as a speed optimization yet. Unsetting `CARGO_TARGET_DIR` alone is
insufficient if Cargo config still redirects the build. Installed prebuilt and
explicit no-build modes have separate contracts. This iteration has not changed
launcher policy, cleared the bug, or weakened source checks.

## Prototype and decision

An isolated launcher requests `--message-format=json` on the **same** Cargo build.
A streaming helper selects one non-test `angel` bin artifact with the expected
resolved `cockpit/src/main.rs`; it requires an executable path and one successful
build-finished message. It forwards rendered compiler diagnostics to stderr and
the human log. No second Cargo metadata/config/build query is added. Parser or
Cargo failure is fatal. Blaze and no-build branches are unchanged.

* Nine reduced real-Cargo cases in each arm: all four redirected launches return
  B in the prototype; build-only reports the real C path, compiler error does
  not launch, edit/restore and warm reuse return A.
* **44/44 existing launcher tests pass** in an isolated copy with fake Cargo
  extended to emit the requested JSON and the candidate helper copied. This is
  an adapted same-author compatibility suite, not 44 real builds. The live tests
  and launcher remain byte-identical to entry.
* **16/16 parser-fixture expectations hold**, including rejection of absent,
  ambiguous, duplicate, malformed, wrong-source and nonexecutable records and
  rendered-diagnostic forwarding. One test deliberately records a compatibility
  gap: arbitrary non-JSON stdout is rejected, not forwarded.
* Two additional real-Cargo warning/error launches show rendered diagnostics in
  stderr and `cargo.log`, no JSON leakage, and error fail-closed with an older
  executable retained. This does not cover all build-script stdout or rendering
  modes.

### Preregistered warm performance gate: rejected

24 alternating AB/BA pairs share one warmed tiny-crate target and HOME:

| Metric | Value |
| --- | ---: |
| Baseline median launch | 28.402 ms |
| Prototype median launch | 37.294 ms |
| Median paired **increase** | **9.151 ms** |
| Paired increase / baseline median | **32.22%** |
| Prototype faster pairs | **0/24** |

The bound was <=1 ms **and** <=5% regression. It failed both. Unequal first
primes are retained separately and excluded. The prototype remains **unapplied**;
no startup speedup or full-application performance claim follows. Do not subtract
this from older measurements made on different inputs. Python startup, JSON
processing and changed pipeline plumbing are not separately isolated here.

## Review / remaining gates

Separate-context read-only review confirmed the baseline evidence and advised
against adoption. It preceded candidate execution, so its missing-result comments
are historical, not evidence that later checks failed. Its substantive limitations
remain:

* Path selection is not atomic build-to-private-image acquisition. Concurrent
  replacement after Cargo publishes is not solved (also present in the live
  launcher); no concurrency qualification is claimed.
* Cargo stderr goes through asynchronous process substitution, unlike the old
  merged `tee` stream. Failure of that tee is not included in `pipefail`; this
  logging guarantee must be resolved before adopting this implementation.
* Non-JSON stdout, pathological paths/locale/large output, unusual binaries and
  all rendering modes need fuller compatibility handling. Parser metadata arrays
  can grow; this is a line-stream prototype, not a fully bounded protocol parser.
* Full-host/default-feature edit/restore and startup readiness remain unmeasured
  here. Preserve the existing Blaze restore blocker independently.

Next useful experiment: a native artifact/launch adapter already available with
the application or a separately measured low-overhead stream reader, preserving
Cargo config semantics and fail-closed behavior. An env-only path guess cannot
fix config target-dir plus target layouts. Alternatively evaluate an explicit,
documented launcher-owned Cargo output layout, but do not silently override
operator Cargo settings. Preregister before another timed candidate.

## Blaze contribution opportunity

The clean pinned source is revision
`b383b0bcf5da33816809b00a37e1c36a0e761efb` (indexable-inc/blaze).
`crates/ra-jit/src/cargo/whole.rs:43` explicitly refuses native build/run with
`--message-format`; `:46–48` refuses CLI `--target-dir` while stating that
`CARGO_TARGET_DIR` is read. `planned()` already uses
`metadata.target_directory`, the selected target triple, and profile to derive
the actual executable (`:745–805`). Thus this is **not evidence that Blaze has
this launcher's hardcoded directory bug**.

Concrete upstream PR scope: implement Cargo-compatible `compiler-artifact` and
`build-finished` emission on successful native build/test paths, using their
existing actual `Program.executable`, package, target and profile data; emit
fresh artifacts on warm reuse too. Retain human diagnostics on stderr, correct
JSON escaping, multi-bin/test filtering, failed-build semantics and unsupported
format rejection. Reuse `cargo/messages.rs` serialization machinery where
appropriate rather than add a second path query. Independent regressions should
compare clean/warm/edit/restore, absolute/relative target directories, host target,
multiple bins, warnings/errors, spaces and escaped path characters. Artifact
messages must describe actually published bytes, **not hide** the known daemon
restore failure. No upstream code patch, live Blaze call, daemon, or PR submission
was made this iteration; feasibility is source-inspected, not runtime-verified.

## Reproduce / retained sources

Preregistration: `reports/performance/launcher-artifact-preregister-iteration32.md`.
All receipts: `reports/performance/launcher-artifact-iteration32/`.

* `baseline-angelX`, `baseline-native-launch.test.mjs` — exact entry copies.
* `candidate/` — unapplied implementation plus adapted existing launcher suite.
* `candidate-api.patch` — changes against the live entry sources, not a PR.
* `repro-{baseline,candidate}/{inputs.json,raw.jsonl,summary.json}`.
* `timing/{inputs.json,raw.jsonl,summary.json}` — recompute paired medians from
  raw records, not rounded table values.
* `streams/{raw.jsonl,summary.json}` and `real-diagnostics/`.
* `review-*.txt`, `upstream-source.json`, `final-integrity.json` — reviews,
  source excerpts/identities, archival and protected-file checks respectively.

Executable drivers are the `reports/performance/*-launcher-artifact-*iteration32.py`
files. They use exclusive output directories; a retry requires a new result name,
never replacing these receipts. Fixtures remove their private outputs after exit.
No installed compiler, application image, locked source or earlier report is
intentionally edited; final hash checking is retained separately.
