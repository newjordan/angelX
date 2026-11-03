# Iteration35 — Blaze toolchain probes: correctness before faster selection

## New findings

Against upstream `b383b0bcf5da33816809b00a37e1c36a0e761efb`, failed/empty sysroot output becomes an empty `PathBuf`. If the working directory contains `lib/rustlib/src/rust/library`, exact extracted `settle` code accepts it as `Chosen::Given("")`. Checking child exit status alone does not solve successful-empty output. Related control-flow defects:

- `other_toolchain() -> Option<String>` uses `?` for probe failure, so failure means **no refusal**.
- Its `.exists()` source check accepts a regular file, unlike `with_library`'s `.is_dir()`.
- The answer accelerator can return synthetic status 0 and metadata without executing the selected compiler. A retained exact-module executable reproduced this with a metadata fixture and a compiler shim that exits 23. Both direct and fresh-cache own-answer-disabled controls returned 23.
- Trimming sysroot output changes valid pathnames ending in spaces. The candidate preserves path bytes while removing the output terminator.

## Candidate and evidence

Unapplied three-file upstream patch: `reports/performance/blaze-toolchain-probes-iteration35/candidate.patch`. It adds a direct checked probe for selection/linker validation; propagates version/sysroot failures, rejects empty/relative/multiline/NUL/non-UTF-8 sysroots, preserves named-compiler errors, permits unnamed fallback, uses a source-directory check, and makes `rust_lld` fallible with both callers updated. Six Unix-native regression tests are included in the patch.

Measured verification:

| Check | Observed result | Scope |
|---|---|---|
| Final control-flow candidate | 40/40 expected outcomes | exact source functions, explicit stubs |
| Baseline | 40 runs: 10 pinned bad-accept characterizations, 30 observational cases | not baseline correctness passes |
| Revised-candidate total | two sets of 40, with baseline executed once | 120 total case executions, 186 compiler-shim calls |
| Native regression tests | 6 passed, 0 failed | extracted module; not full crate |
| Existing launcher suite | 44 passed, 0 failed | actual `tests/scripts/native-launch.test.mjs` entry |
| Answer-module reproduction | 6/6 expected results | retained executable, not rebuilt this iteration |
| Source/patch/receipt audit | 147 checks held | worker-owned audit, not external certification |

All four stdlib Rust compilations completed with exit 0 and no diagnostics. An explicit budget amendment authorized the fourth compilation for native tests; a separate amendment bounded six answer-module runs. Installed source, launcher/test entry files, protected source/seal, 32 earlier reports and 41 earlier performance documents retain their entry identities. Exact patch application to a temporary baseline tree matches the three final source mirrors; upstream checkout remains clean and unchanged.

## Deployment decision

**Retain as an upstream PR candidate, do not install.** Direct validation can cost extra subprocess launches. No candidate startup latency, full-engine build, real linker execution, Windows, daemon behavior, or full-host restore was measured here. The answer-module fixture uses synthetic toolchain metadata, not a real installed compiler; its executable identity and module equality are checked but its historical build is not independently reproduced.

This closes an important gap in the prior status-only proposal; it does not qualify all metadata consumers or clear the earlier full-host restoration failure. Cargo remains the launcher default. Full startup performance, pathname-to-image race, jq deployment, mixed diagnostic logging and protected-file seal ownership remain separate open gates.

Next: on an independently authorized isolated full-Blaze integration run, compile the patched crate and run its native tests plus the actual selection/linker paths; then measure direct-validation cost against the accelerated answer path before considering rollout. Preserve the previous restore failure until a targeted full-host recovery test clears it.

Detailed result and commands: `reports/research-82abbb708cda5ae57589ee8a17aeaaaa-35.md`.
