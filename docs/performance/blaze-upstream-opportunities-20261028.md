# Blaze upstream opportunities: correctness before automatic acceleration

2026-10-28 · local investigation; no upstream issue/PR has been published.

> New bounded follow-up: [Blaze/V8 sandbox compatibility reduction](blaze-v8-sandbox-reduction-20261028.md). The full-host wrapper edit executed fresh output only after rustc fallback; a direct-Blaze stdlib-only fixture localizes the parent-of-OUT_DIR lock denial without disabling isolation.

Blaze here means `indexable-inc/blaze`, the Rust incremental/daemon compiler
integration, not a replacement application programming language. Useful language
support improvements are compiler/build-system changes that shorten Rust's edit
loop while preserving Cargo's observable behavior. Rewriting angelX into another
language is neither required nor supported by the measurements.

Pinned source examined:
`b383b0bcf5da33816809b00a37e1c36a0e761efb`, installed with
`nightly-2026-10-27`. The checkout is under
`cockpit/target/tools/blaze-src/`; compiler identities are in the receipts, not
inferred from PATH. This document does not modify that pinned installation.

## 1. Synchronous, generation-bound artifact publication (highest priority)

### Evidence

The full angelX host's semantic edit was observed, then the restore build exited
0 but its executable still contained the edited marker. Both executed outputs
had identical SHA-256 despite original source restoration. Raw evidence:
`reports/performance/dev-rebuild-baseline-20261028.json` and its semantic-edit /
restore logs. The independent launcher snapshots are copies, so a new launch
cannot repair already stale bytes in the published build output.

Relevant source paths in the pin:

- `crates/ra-jit/src/cargo/state.rs:518`, `State::cargo_has`: calls `place` to save
  rustc's output as `program.cargos`.
- `state.rs:585`, `State::give_back`: restores saved Cargo outputs.
- `state.rs:987`, `place`: on Linux attempts a hardlink then rename. This is not
  necessarily an immutable byte snapshot when another name can be mutated.
- `state.rs:1007`, `place_copy`: already provides isolated-byte publication for
  some paths. This is an available primitive, not evidence that replacing every
  link is correct or sufficient.
- `crates/ra-jit/src/cargo/wrapper.rs:276`, `stand_in_front_of_rustcs`, and `:292`,
  `warm_after_cargo`: save output then launch a detached settler after Cargo.
- `harness/cargo_ra_jit.py:295`, `Side.settled`: upstream tests explicitly wait
  for the asynchronous settler. Product callers need not make that wait.

### PR-sized deliverable

Add generation/source/output binding to saved artifacts and publication; reject
an obsolete settler publishing into a newer successful build. A successful
wrapper build must not return until the artifact that Cargo will run corresponds
to that invocation. Keep fast inode/link-based paths only where the producer
cannot mutate a supposed snapshot, or store immutable copies/content-addressed
artifacts. Keep diagnostics for the output generation and source/artifact hashes
so a caller can distinguish a fresh fallback from stale saved-image reuse.

This is a **proposal**, not a root-cause finding. The current evidence does not
isolate which function restored or retained the wrong bytes. Changing `place` to
`place_copy` everywhere without a regression would be speculative and could
regress performance without fixing the race.

### Tests/acceptance before merging

Extend the existing Cargo-wrapper harness (not a new orchestration framework):

1. Edit A→B→A and execute the published binary after **each returned command**,
   with no harness-only settling wait. Assert semantics, not just exit status.
2. Repeat with build scripts, include-generated Rust, fallback after daemon
   error, and rapid alternating edits while an old settler is paused at a
   deterministic publication barrier. Release it after the newer build exits.
3. Keep a private copy and a simultaneously running old image; a later build
   must not mutate that copy or the running inode. Check Cargo output and saved
   output identities rather than assuming a hardlink implies correctness.
4. Fail closed on compilation errors; never report stale output as successful.
5. Reproduce/fix the full-host case and compare paired latency before promoting
   automatic use. Full features and sandbox controls must match Cargo.

### New narrowing result

`scripts/perf/repro-blaze-image-freshness.py` supplies executable semantic
checks and bounded isolated daemon cleanup for two tiny wrapper fixtures:

```
python3 scripts/perf/repro-blaze-image-freshness.py <new-receipt.json>
python3 scripts/perf/repro-blaze-image-freshness.py <another-receipt.json> --build-script
```

Both passed prime/unchanged/edit/restore. Saved/published images had 2–3 hardlinks,
so links alone are not enough to reproduce the full-host failure. The simple
build-script case was also fresh. Receipts:
`blaze-tiny-freshness-v2-20261028.json` and
`blaze-tiny-build-script-freshness-v2-20261028.json` under `reports/performance/`.
The earlier same-named receipts without `v2` are retained: their supposed
unchanged step rewrote identical source, so it was not a true no-touch control.
The corrected probe also records its own digest and explicitly disables remote
Blaze work; neither version executes a remote command.

These probes wait for wrapper settlement **between** steps to keep them bounded
and easier to interpret, and execute the output **before** that wait. They do not
cover rapid overlapping generations. Logs show Cargo compilation but provide no
positive daemon/fallback provenance, so these runs cannot prove correct daemon
execution. Preserve the negative result instead of calling it a
fix or pretending the original bug has a minimal repro yet. Next reduction:
introduce nested build-script/Cargo work and controlled late publication into
this fixture; retain the full-host observation as the actual failing case.

## 2. Reentrant nested-Cargo build scripts with explicit capabilities

The full host could not obtain a clean daemon-host build: the nested WASM build
and its build-script wrapper hit read-only-cache / recursive-wrapper behavior,
then used the Cargo fallback. Existing compatibility controls
`RA_JIT_SCRIPTS_BY_RUSTC=1` and `RA_JIT_PROC_MACRO_SERVER=0` remain necessary for
this pin. The simple build-script probe passes, so the problem is not “all build
scripts are unsupported.”

Relevant surfaces: `cargo/own_script.rs`, `cargo/script.rs`,
`cargo/script_sandbox.rs`, `cargo/daemon.rs`; application reproducer is
`cockpit/build.rs` (`delve_web`, nested WASM Cargo). The host's
`cockpit/build_fast.rs` selects the actual toolchain Cargo/Rustc for nested work,
not a build-engine alias named `cargo`.

**PR deliverable:** preserve Cargo jobserver/target/toolchain semantics through
nested scripts, scope wrapper state per invocation, and declare precise allowed
cache/output directories. Report which capability/path was denied. Do not add
an “all build scripts unsandboxed” switch or silently fall back without naming
it. Verify a minimal nested Cargo→build script→generated Rust fixture, with a
read-only project plus writable declared target/cache dirs, explicit toolchain
paths containing spaces, and failure/cancellation. Then qualify the real WASM
host case under the same sandbox policy.

This is a compatibility route to test, not an implemented sandbox change.

## 3. Trustworthy phase/fallback telemetry and end-to-end edit benchmarks

The corrected tiny fixtures' true no-touch launches took about 19 ms, the first
edit took 2.17–2.35 s, and restore took 25–26 ms. The earlier v1 “unchanged” case
rewrote identical source and took 2.32–2.64 s: that was an invalid no-op control,
not evidence that genuine no-ops are slow. None are representative host numbers,
and ordinary Cargo logs do not identify the daemon/fallback path. A successful
build may include Cargo, daemon, fallback, settlement or publication work.

**PR deliverable:** emit a structured local receipt with compiler revision,
toolchain, cache state, fallback reason, and monotonic durations for discovery,
script work, analysis/codegen, link, settling and publication. Keep secrets,
environment values and source contents out of telemetry. Extend existing
`harness/cargo_ra_jit.py` / artifact-edit benchmarks to report both command-return
latency and semantic-fresh output latency. Gate advertised acceleration on
executed edit→restore behavior, not exit 0 or an “unchanged” message.

Acceptance: receipt phases agree with outer wall time within declared overhead;
forced daemon failure is labeled; fresh output checked on both paths; a bounded
multi-run Cargo comparison retains every result and does not select only daemon
successes. This would make downstream adoption faster and failures actionable.

## Adoption rule for angelX

The existing project-local installation and isolated Blaze target are useful
infrastructure; they are not qualification. Keep normal launch on qualified
Cargo and preserve the explicit experimental Blaze opt-in. Promote only after
repeated full-feature edit/restore/failure cycles pass, the daemon-host script
path works without sandbox weakening, and paired end-to-end latency beats Cargo.
No upstream API token calls, GitHub publication, global configuration changes or
provider jobs were used for this investigation.


## 4. Native static-library dependency freshness regression (new local result)

The native phase is now a normal `angel-native` path dependency with independent
Cargo invalidation. A matching tiny fixture on pinned Blaze `e8140ce0` kept Rust
unchanged and edited only C behind a dependency build script. Prime, untouched,
edit and restore executed `native-1`, `native-1`, `native-7`, `native-1`; all
matched after command return. Durations were 281.5, 17.3, 2378.1 and 476.7 ms.

Reproduce with `scripts/perf/repro-blaze-image-freshness.py OUTPUT.json
--native-dependency`; retained receipt:
`reports/performance/blaze-native-dependency-freshness-20261028.json`.
See [the detailed isolation/freshness report](native-fingerprint-20261028.md).

**PR opportunity:** port this executable edit/restore test to
`harness/cargo_ra_jit.py` / `harness/artifactedits.py`; include emitted static
link-search/link-lib metadata from a path dependency, no-touch control, forced
compiler failure, and daemon restart. Positive native-dependency results help
localize the earlier full-host failure rather than explain it away. The local
fixture covers positive freshness only; failure/restart/wrapper cases remain
unmeasured. This is a concrete regression-test proposal, not a new stale-output
bug report or full-host Blaze qualification. No upstream PR was submitted.

## 5. Readiness notification rather than fixed polling quanta (new source lead)

In angelX, replacing a synchronous child-exit loop's fixed 10 ms sleep with a
Linux pidfd readiness wait reduced median first terminal output from 36.947 to
28.536 ms (three warm PTY samples each). The eager startup/confined-tool policy
was preserved; lazy probing was rejected. Details and actual edit/restore are in
[the readiness report](startup-readiness-20261028.md). That measurement is for
angelX, **not** evidence of a Blaze speedup.

Source inspection of the installed Blaze checkout at
`b383b0bcf5da33816809b00a37e1c36a0e761efb` finds similar opportunities in
`crates/ra-jit/src/cargo/daemon.rs`: `POLL` is 20 ms (line 118),
`Worker::ended_within` retries child status after a full tick (line 602),
`Worker::connected` retries socket connect (line 728), and `Checker::ask`
retries checker connection (line 983). `cargo/client.rs` uses 5 ms `POLL` (line
28) for daemon shutdown/socket disappearance and startup (lines 95, 105, 212).
This is a different local checkout revision from the older `e8140ce0` fixtures
above; do not attribute source line findings to that earlier compiler pin.

**PR opportunity:** use pidfd/kqueue or an owned child-wait signal for process
termination, and an explicit bounded ready/error channel for newly spawned
workers/checkers/daemon listeners. Do not replace failed-connect polling with
polling an unconnected socket: listening readiness is a protocol event, not
child-exit readiness. Keep wire-version checks, timeouts, log diagnostics,
process-group cleanup, and old-kernel/non-Linux fallback. Avoid shortening ticks
into a CPU spin or holding worker/checkouts locks while waiting.

**Acceptance proposal:** retain every cold/warm/restart sample; timestamp worker
ready and client return separately; show latency beyond actual readiness falls
without raising CPU/FD use. Cover immediate child failure, malformed response,
never-ready child, timeout/cancellation, restart while clients wait, old-kernel
fallback and non-Linux behavior. Run semantic edit→execute→restore checks to
ensure readiness cannot be confused with artifact publication. A synthetic
delay spanning many tick phases distinguishes quantization from compiler work.

No Blaze source patch, benchmark, upstream API call or PR submission was made
for this lead. It remains a source-evidenced, **unmeasured** proposal; no latency
gain or daemon qualification is claimed. The duplicate compiler-probe launcher
lead was also inspected (~3 ms subprocess median) but left unimplemented to
avoid mixing shell selection changes into this verified startup patch.

## 6. Byte-identical cached build-script replay (new bounded result)

[Restore replay reduction and controls](blaze-restore-replay-20261028.md)
extracts `Kept::restore` from the pinned source. Comparing exact file bytes before
rewriting reduced an unchanged 12 MiB replay fixture by 66.7%, but a changed-file
control regressed 12.5%. This is a concrete, unintegrated PR opportunity with
negative controls, not whole-Blaze acceleration or qualification for auto mode.
