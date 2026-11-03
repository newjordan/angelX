# Standalone native reload builds: bounded parallel compilation

2026-10-28 · iteration 14 · measured on Linux/Ryzen 9950X3D, host GCC 16.2.1.
This changes `scripts/runtime/build-native-modules.py`, the opt-in **standalone
reload builder**, not the earlier Cargo native build-script scheduler.

## New measured result

Seven alternating baseline/candidate pairs compiled the actual four production C
sources, loaded the resulting shared libraries, and executed all four ABI-version
exports. Every library matched the baseline SHA-256, every ABI returned 1, and
every unchanged second invocation preserved its image inode and mtime.

| Scope | Serial baseline median | Final candidate median |
|---|---:|---:|
| Compile/publish four real reload libraries | 908.6 ms | 348.7 ms |
| Unchanged second invocation | 22.7 ms | 22.2 ms |

Ratio of compile medians: **2.605×**, 559.9 ms saved. All 14 compilations and all
14 cache reuse checks succeeded. The benchmark includes Python process startup,
compiler selection, hashing, lock acquisition, compilation, and publication; it
excludes copying the fixture sources and loading/executing the resulting libraries.
It is not application startup, a Rust host-edit improvement, an application
reload-latency measurement, or a clean-machine installation result.

The first candidate measured 933.3 → 362.6 ms, but unchanged reuse regressed from
24.2 to 36.0 ms due to importing `ThreadPoolExecutor`. The final source imports
it only for changed work. Both receipts remain intact; only the second receipt
binds the final source hash. No dependency package or new external tool is needed.

## Implementation and safety

- Default limit: available process CPUs, at most four compilers. Explicit positive
  `ANGEL_NATIVE_BUILD_JOBS` is capped at four; `1` is the serial control. Invalid
  or nonpositive settings fail before publishing anything.
- An inherited Make/Cargo jobserver forces one slot. This standalone Python
  builder does not borrow tokens and must not oversubscribe a parent's budget.
- The existing exclusive build lock covers planning, compilation, input
  revalidation, and publication. The unchanged cache key still includes compiler
  identity, flags, source, builder bytes, platform, and actual output hash.
- All changed objects are staged before publication. The executor joins every
  compiler on success or failure; all source snapshots are checked again before
  any library publishes. Compiler failure or detected source change leaves the
  previous batch intact. Temporary files are cleaned after children finish.
- Images still publish by atomic rename. An already loaded generation's bytes
  are not modified. Output receipt follows the image, so interruption can cause
  recompilation but not a false cache hit.
- Publication remains **per-image**, not an all-library transaction: an I/O error
  during publication, a process crash, or a loader reading between renames can
  observe mixed generations. This change does not claim stronger transaction
  semantics or eliminate the existing edit-vs-publication race window.
- Native reload stays explicit: `angelX build --modules`,
  `ANGEL_NATIVE_MODULES=1 angelX`. Ordinary startup does not enable reload workers.
  Existing workers poll every 500 ms; these measurements do not include that wait.
- `ANGEL_NO_BUILD=1` still prohibits compilation. No feature, authorization,
  sandbox, source validation, or fallback behavior is disabled.

## First-setup correction

The fresh compiler installation branch of `bin/angelX` used
`rustup toolchain install CHANNEL --profile minimal` while disabling Rustup's
automatic installation for the subsequent build. It now adds
`--target wasm32-unknown-unknown` to that same install command. This matches the
required target in `rust-toolchain.toml` and avoids a late missing-target failure
inside the nested browser build. A fixture exercises the exact missing-pin branch
and requires the target before Cargo runs. It failed before the change and passed
afterward. No real toolchain was installed or downloaded in this experiment;
network/cold-setup time is unmeasured. Already installed compiler paths are not
repaired by this branch if someone manually removes their WASM target.

## Verification

All paths below are under `reports/performance/cargo-freshness-iteration14/`.

- `native-red.log`: existing serial implementation failed three new regressions
  (parallel overlap, failed-batch preservation, invalid job bound).
- `native-final-tests.log`: **9 Python tests passed**. Cases cover unchanged reuse,
  changed-only publish/old dlopen generation, corruption and flags, compiler
  failure cleanup, bounded overlap with a synchronization barrier, failed batch
  preservation/reaping, invalid budget, parent jobserver serial behavior, and
  source mutation aborting publication.
- `setup-red-2.log`: fresh pinned setup regression failed without WASM target.
  `setup-red.log` is a discarded first fixture that incorrectly modeled an already
  usable unpinned Cargo; retained rather than counted as a product regression.
- `launcher-final-tests.log`: **36 launcher tests passed**, including explicit
  no-build behavior, build-failure refusal, setup target, native path overrides,
  backend selection, and Blaze isolated launch semantics.
- `native-final-paired.json`: final seven-pair actual-source benchmark, byte/ABI
  checks, warm reuse checks, source/builder hashes and compiler identity.
  `native-paired.json` retains the initial import-cost finding.
- `final-source-identities.json`: changed implementation/test hashes and blocker
  identity. Documentation additions occurred afterward, not implementation edits.
- `reload-gate.log`: attempted normal Rust runtime gate:
  `bash scripts/check/check-cockpit-fast.sh reload --test-threads=1`.
  **Blocked before compilation by operator lock validation**, not a failing
  runtime test. `lock-blocker.json` shows `cockpit/src/drive/loop_ctl.rs` current
  bytes match HEAD (SHA-256 `c5fb1562ccad4ff15fd49d6aa399d5bebcb0b5739e951865a6733bce087d7148`)
  while `locks/locked.sha256` records
  `cd017dfde2178e21ba3f05855698b703a67f40101f442e99142ad4651dcce733`.
  Neither the source nor seals were changed. No bypass/reseal attempted. Rust
  reload-runtime and full gate acceptance remain pending operator reconciliation.

The advisory read-only implementation reviewer timed out with no answer; no
independent review approval is claimed. These are project-local tests and measured
results, not independent acceptance or proof of a globally fastest configuration.

## Startup controls that did NOT help

The new warm trace put ~212 ms in Cargo's freshness/build call. Trace logging is
intrusive; 547 fresh units were observed, not a causal per-phase cost allocation.
Thirty-five randomized warm version launches (five rounds, seven controls) all
executed the normal source launcher successfully. Medians:

| Option | Milliseconds |
|---|---:|
| Default | 218.1 |
| Cargo jobs 1 / 2 / 4 / 8 / 16 | 219.8 / 223.5 / 219.1 / 218.1 / 214.8 |
| Cargo offline | 230.2 |

These do not justify a new product default. `warm-controls.json`, `trace.json`,
`trace.log`, and `cargo-trace.log` preserve raw evidence. They are warm `--version`
measurements before the setup change (the installed-toolchain branch is unchanged),
not usable-PTY/cold-start timing. No new end-to-end host-edit timing is claimed.

## Reproduce

```sh
python3 -m unittest discover -s tests/python -p test_build_native_modules.py
node --test tests/scripts/native-launch.test.mjs
python3 scripts/perf/measure-native-reload.py NEW.json \
  --baseline reports/performance/cargo-freshness-iteration14/native-baseline.py --pairs 7
```

Further application-level verification must use the normal gate after the operator
resolves the existing lock mismatch. Do not reseal or run around that blocker.
