# Checked artifact selection without an interpreter-scale launch penalty

## Status

**Unapplied iteration-33 prototype. No live startup/compile speedup, application readiness qualification, or Blaze enablement is claimed.**

The current launcher always asks Cargo to check freshness but assumes the resulting image lives at `cockpit/target/$PROFILE/angel`. The earlier iteration-32 experiment demonstrated successful Cargo builds followed by stale image launch when target output is redirected. Its Python artifact-reader prototype corrected that error at a measured +9.15 ms paired pilot cost. This iteration tests a distinct native `jq` reader with synchronous diagnostic delivery, not an unchecked fast path.

## Evidence and scope

All evidence is in `reports/performance/launcher-jq-artifact-iteration33/`. Source is `candidate.patch`; baseline is frozen `baseline-angelX`. The real-Cargo fixture has a minimal Rust binary, no dependencies, offline Cargo 1.95.0, fast profile, source binding disabled, and no host/game build. The launcher is real; the application is not angelX.

Final real-Cargo matrix, nine attempts per arm:

- Seed default-output source A.
- Edit source; exercise absolute and relative `CARGO_TARGET_DIR`.
- Edit again; exercise `.cargo/config.toml` target directory and explicit host target.
- Revisit an output with no source changes.
- Inject a compile error; refuse stale launch.
- Explicit build-only operation; report the Cargo-selected image without launching.
- Explicit no-build operation; no compiler/artifact scan.

Baseline stale launches reproduced in exactly four redirected-output cases; candidate had none. The original nine-case driver checks its expected baseline failures as observations, not baseline correctness. Final 44 adapted existing Node launcher tests passed. The adaptations only teach fake Cargo to emit selected-artifact JSON and copy the candidate helper. These are not 44 new application tests. Both initial and review-amended suite runs are retained.

### What changed after separate-context review

Initial merged stdout/stderr incorrectly treated JSON-looking human text as metadata. Its field checks could silently ignore malformed irrelevant records, and jq 1.8.2's `stderr` could return zero when writing `/dev/full`.

Before any timing, the revised prototype:

1. Passes Cargo stdout alone through a synchronous raw-log tee into jq.
2. Sends Cargo stderr and jq diagnostics through a second synchronous, exit-checked tee.
3. Sends only the selected executable through a private command-substitution descriptor.
4. Uses inherited `pipefail`; all producers and sinks complete before selecting/launching the executable.
5. Validates artifact selection fields before filtering, requires exactly one matching binary plus a successful finish, and rejects ambiguous/duplicate/matching-name wrong-source/control-character paths.

31 same-author regression cases held: malformed/absent/ambiguous streams, malformed fields beside valid events, dependencies/tests, rendered compiler messages, raw-log and terminal `/dev/full`, JSON-looking human text and a partial final line. Final real warning/error launches preserved human terminal output and refused stale launch after compile failure.

The copied diagnostics verifier initially failed because it expected a human-only cargo.log; the amended design explicitly retains raw metadata there. Initial driver and failed warning receipt were preserved. Only the verifier's log expectation was corrected; the candidate and timings were not changed. Final two-case result held.

### Timing — one fixed 24-pair AB/BA series

| Metric | Observed |
|---|---:|
| Warm baseline launch median | 25.432146 ms |
| Warm jq prototype launch median | 25.949429 ms |
| Median paired increase | 0.537450 ms |
| Paired increase / baseline median | 2.1133% |
| Candidate faster pairs | 11/24 |
| Baseline-first median paired increase, 12 pairs | 0.386867 ms |
| Candidate-first median paired increase, 12 pairs | 0.537450 ms |

Two primes excluded; 48 warm launches included. Predeclared <=1 ms and <=5% paired overhead gate held. This is **not a speedup**. The previous Python reader's +9.15 ms was a different pilot/time window; this is not a directly paired jq/Python comparison. No cold-start, full cockpit readiness or edit-recompile acceleration measurement was performed. No threshold retuning or second timed implementation followed the observation.

### Integrity and protocol limitations

2,074 archival/arithmetic checks held, including identities of 1,920 earlier artifacts and 10 entry files/binaries, recomputed timing results, retained sources and clean upstream excerpts. These are not application tests or an independent evaluator. jq was version-recorded before use, but its file hash was only recorded after timing; do not claim pre/post jq binary equality.

The original <=24 non-timing real-Cargo launch budget was exceeded: initial/final matrices 18+18, initial warning 1, final diagnostics 2 = 39. A review-driven amendment documented rerunning corrected matrices but did not explicitly replace the numerical cap. This is a protocol deviation (+15), not a fully preregistered campaign. Including 2 primes and 48 warm launches, total tiny-Cargo launches were 89. No host build, download, provider call or resource expansion occurred.

## Adoption gates still open

- Candidate adds jq plus a deployed filter to normal checked Cargo launch. Missing jq fails closed. It does not qualify every currently supported development host or older jq release.
- This is **selection-field** validation, not a complete Cargo event schema validator. Some absent fields are equivalent to null, and unrelated build-script fields are not validated. A future design must scope its schema claims accordingly.
- Concurrent stdout/stderr tees append to the same log. The log is a mixed diagnostic transcript, **not guaranteed intact JSONL**, cross-stream ordering, or durable downstream delivery. Large/partial writes may interleave.
- Cargo gives a pathname. The later executable check and hardlink/copy do not bind selection to the exact inode of the completed Cargo build. A concurrent rebuild can replace it. This pre-existing pathname-to-image race remains unsolved; do not generalize single-writer fixtures into a transactional freshness guarantee.
- The protected `loop_ctl.rs` seal mismatch remains unchanged. No reseal/bypass or host acceptance is attempted.
- Blaze remains explicit-only. Previous success-with-stale-restored-executable result is not cleared by this Cargo fix.

No code was installed in `bin/angelX`, no protected source, manifest, tool image or host executable changed. A production patch needs a supported-host/dependency policy and an explicit single-writer/transactional image policy before broad acceptance.

## Concrete Blaze PR opportunity: extend existing output machinery

Read-only inspection of clean revision `b383b0bcf5da33816809b00a37e1c36a0e761efb` shows:

- `crates/ra-jit/src/cargo/messages.rs::Format` already parses JSON and `json-render-diagnostics`, and its check path emits `build-finished` success/failure.
- `whole.rs::refused` rejects every nonempty `flags.message_format` before whole build work.
- `whole.rs::Program` already carries `name`, `root`, `package`, `kind`, `test`, `executable` and object directory, so artifact provenance need not be reconstructed by launcher path guessing.
- Whole build calls its human `finished` status; the inspected build path has no matching machine artifact contract.

Therefore, extend `messages.rs` rather than creating a second JSON protocol. Proposed PR scope (design only, not applied/published):

1. Add typed artifact emitter using existing package/target identities; report exact materialized executable, profile/test and fresh/cache state.
2. Accept supported message formats in `whole.rs`, retaining refusal for unsupported combinations; separate machine stdout/human stderr.
3. Emit artifacts only after requested programs are materialized and current. Emit failed finish on all handled failure exits, never successful finish after a failed restore/link.
4. Add upstream regressions for changed output paths, explicit target, edited source, no-op/cache restore, multiple targets, failure and `json-render-diagnostics`; compare emitted executable hashes to actual output.
5. Keep a dedicated restored-host mutation regression. JSON reporting is not evidence the existing restore bug is fixed. Do not use this PR to promote auto-Blaze.

This source finding rules out a naive “use the same JSON flag for Cargo and Blaze” patch at the pinned revision. It is compatibility evidence, not a new runtime stale-image reproduction or an upstream benchmark. Exact source hashes/excerpts are retained in `blaze-output-source.json`.
