# H01 retry-veto provenance — committed native qualification and integration

## Status

**COMPLETE — qualified local native/release fix committed and integrated.**
The scoped gates below passed. No push, deployment, or live restart occurred.

## Authorization and scope

On 2026-10-24 the operator explicitly approved creating an isolated branch,
committing only the reviewed fix/tests, running final qualification, and integrating
if green. Standing goal: continue until all gates are complete. No push, deployment,
or live cockpit restart was authorized. Game development remains out of scope.

The original `durability/backup-peer-watch` worktree contained unrelated edits.
Those edits are not part of this patch and must not be stashed, committed, overwritten,
or represented as qualified by this run. An inventory binds all 63 preexisting dirty
or untracked files captured before this operation.

## Candidate

- Parent: `d0e4ccba11044657f2119f07ad41853dd00d6e29`.
- Branch: `fix/h01-veto-provenance`.
- Candidate commit: `c6a6dc8c749c5ff2495f30ed28bff58808745d98`.
- Isolated checkout: `<scratch>/h01-veto-qualification-20261024`.
- Source-v3 digest: `2150a7fe2c9ba6c1d4837615b0fc5002a91b1710c36a56f4e51cfc69b2253648`.
- Product change: require retry-veto provenance from the producer's anchored
  `HTTP <three ASCII digits> [x-should-retry: false]: <detail>` envelope instead of
  accepting a veto marker anywhere in a response body.
- Exactly two files committed: `cockpit/src/agent/harness/turn/classify.rs` and
  `tests/cockpit/harness/tests__run_turn.rs`. Diff: 105 additions, one deletion.

The classifier is byte-identical to the previously audited candidate. Test
formatting was normalized only for the newly added tests; unrelated formatting
churn was removed before the final commit. An independent read-only reviewer
confirmed the production identity, fixture/token equivalence, two-file scope,
producer/consumer contract, and expected 13-test selection. No correctness blocker
was reported. That review is not a substitute for an actual green gate.

## Qualification method

Run the repository's native final gate against the clean committed candidate:

```text
scripts/check/check-cockpit-fast.sh --release \
  h01_ \
  run_turn_does_not_retry_permanent_provider_failures \
  run_turn_permanent_error_with_stall_context_is_not_retried \
  provider_cancel_disconnects_ \
  a_finished_request_abort_never_reaches_the_pooled_connection \
  an_aborted_request_socket_is_never_pooled \
  --test-threads=1 --nocapture
```

The gate checks source cleanliness and digest stability before compilation,
ordinary product boundaries, development CLI/helper builds and focused tests,
release helper build, optimized focused tests, release CLI build, and release
build-info source/profile identity. No default features are enabled, as specified
by the existing repository gate. This is not an all-features or full-suite result.

The private Cargo target is `<build-cache>/h01-veto-qualified-20261024`,
seeded from existing caches without mutating shared build outputs. Controls:
`ANGEL_YOLO=0`, `ANGEL_YOLO_SMART=0`, `RUST_TEST_THREADS=1`,
`CARGO_NET_OFFLINE=true`, `CARGO_BUILD_JOBS=2`; systemd scope `CPUWeight=20`,
`MemoryMax=12G`; outer timeout two hours. Preexisting build-script asset compilation
can clear the environment for nested Cargo. Therefore offline/jobs settings are
not a demonstrated all-descendant network/concurrency guarantee. The systemd
resource cap and timeout apply to descendants. Asset compilation is a dependency,
not resumed game work. No live provider, GPU, remote job, or cockpit UI launch is
part of qualification; `--build-info --json` is a noninteractive identity probe.

Inherited `ANGEL_BUILD_SOURCE_SHA256`, `ANGEL_BUILD_RUSTC`,
`ANGEL_BUILD_PROFILE`, and `ANGEL_T_SANDBOX_HELPER` are cleared before the gate.
In particular, the helper override is absent, requiring a candidate helper build.

## Setup and preflight diagnostics

1. Release-cache reflinking crossed filesystems and failed before compilation;
   an ordinary private copy completed successfully. Debug cache was CoW-copied.
2. First gate preflight exited 3 before compilation because it inherited the
   running cockpit's build identity rather than the committed candidate identity.
   Logs are preserved in `preflight-attempt-1/`. The corrected launcher clears the
   inherited values and lets the existing gate derive the candidate identity.
   No product-source change or blind rebuild was used to resolve this diagnostic.
3. An ancillary temporary-log lookup initially assumed `/tmp`; actual gate
   fixtures used the configured `<scratch>`. The lookup was corrected without
   disturbing the build. Primary stdout/stderr were preserved throughout.

## Measurements

The corrected gate exited **0**. Its wall duration was 3149.25 seconds;
start `2026-10-24T01:15:07.985208+00:00`, finish `2026-10-24T02:07:37.258518+00:00`.

| Gate | Result |
|---|---|
| Clean committed source-v3 identity, before and after | PASS |
| Ordinary cockpit boundary / active source connections | PASS |
| Development standalone CLI and sandbox helper builds | PASS |
| Development focused tests | **13 passed, 0 failed** |
| Release standalone sandbox helper build | PASS |
| Optimized focused tests | **13 passed, 0 failed** |
| Release standalone CLI build and source/profile build-info | PASS |
| Exact selected-test set and retained test-image identities | PASS |
| Explicit candidate resource root and 2,623-file inventory | PASS |
| All 63 preexisting files preserved byte-for-byte | PASS |
| Fast-forward integration and two-file identity comparison | PASS |

Both profiles executed exactly the registered 13 test functions. Each profile
observed 11 loopback cases and 21 fixture POSTs. The quoted-veto body recovered
after **3 POSTs**; a genuine header veto remained terminal after **1 POST**.
Retry-After recovered after 2 POSTs. The independent 15-assertion holdout passed
in both profiles, alongside permanent-endpoint/permanent-error guards, four direct
physical cancellation tests, and two pooled abort isolation/reuse tests. This is
not a pooled cancellation-then-resend test or a full task/peer evaluation.

All builds use `--locked --no-default-features`; release uses repository profile
settings (thin LTO, one codegen unit, symbol stripping). These are focused native
qualification results, not all-features/all-tests certification. Cargo emits
preexisting unused-code/import warnings in release tests. The preexisting
build-script warning about unavailable wasm-bindgen 0.2.129 selects its browser
fallback presenter; no browser/game change or GPU-renderer qualification is claimed.

Standalone artifact identities:

| Artifact | SHA-256 |
|---|---|
| `release/angel` | `75ff481d95b8000ad26700d5c3e805bdb67d1f96a2861f473e1335c514ad2b89` |
| `release/angel-sandbox` | `4922a917455c6ec17b4a49f6aa2a069bb7d9233e96efbf0296ec022256180ccb` |
| `debug/angel` | `d6fd1f0679e907db00d6c19b37e487af765a650bb3fc8165f1fb7c523d5b492b` |
| `debug/angel-sandbox` | `8db03ba6c64c25f17d810594b1b86268833a7f9e7dbf66a05925ba2d676c8492` |

- Development test image: `43b3fea8f8a3693ac5468d2318ca7d249a3962344f408895fc9ca78c529299ac`.
- Release test image: `3ef691e353568fec3f5249b1b13b04b4d773185cb6ae1643eee23c4fca929788`.
- Local resource inventory: `b4e8f43575e5fa0c90f07d3017f945a10a3be7720c125eab1c2f202d30ecfedf`.
- Embedded resource bundle label: `unbound`.

The build's resource label is not promoted into a portable/distribution bundle
claim. Runtime availability was separately checked with `ANGEL_RESOURCE_DIR`
explicitly pointing to the committed candidate, and all externally inventoried
resource hashes were recomputed. The inherited baseline resource directory is
not used as candidate evidence. See `build-info-candidate-resources.json`.


## Decision

**Integrated** commit `c6a6dc8c749c5ff2495f30ed28bff58808745d98` by fast-forward into
`durability/backup-peer-watch` after all scoped gates passed. The original index remained empty,
the original dirty-worktree status was identical before and after integration,
and every preexisting captured file retained its hash. Both integrated files
match the qualified candidate exactly. No unrelated game or harness edits were
committed, stashed, overwritten, or included in the qualified patch.

The scoped fix is complete: the provenance classifier, regression fixtures,
clean committed native/release qualification, evidence checks, and authorized
source integration are done. No additional unchanged classifier runs are needed.
The original tree's unrelated modifications remain unqualified; its aggregate
source digest may still reject the dirty tree. That does not change the clean
candidate's qualification and was not bypassed or presented as green.

The running cockpit executable has **not** been replaced or restarted. No push
or deployment occurred. Distribution qualification, representative task effects,
matched peers, cost/leadership claims, and historical row425 hash repair remain
outside this narrowly authorized fix, not concealed failed acceptance gates.

## Evidence

Artifacts: `artifacts/harness-research/20261024-local/H01-veto-qualification/`.

Key files: `qualification-plan.json`, `candidate-commit.json`, `qualify.py`,
`qualification-invocation.json`, `gate.stdout.txt`, `gate.stderr.txt`,
`qualification-receipt.json`, `analyze-qualification.py`,
`unrelated-work-inventory.json`, `resource-file-inventory.json`,
`commit-review-summary.txt`, `commit-review-receipt.json`,
`preflight-correction.json`, `preflight-attempt-1/`,
`qualification-verdict.json`, `integration-receipt.json`,
`development-cargo/`, `development-test-image`, `release-test-image`,
`final-artifact-manifest.json`.

Prior mechanism evidence remains in
[the 2026-10-23 native gate](harness-H01-2026-10-23-veto-native.md), including
baseline expected-red behavior and its early-abort coverage limitations. This run
is native qualification of the accepted fix, not a new paired task experiment.
No solve-rate, cost, leadership, release-distribution, or live-runtime claim follows.
The historical row425 report-hash discrepancy remains outside this fix.
