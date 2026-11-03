# Independent native fingerprints and executable freshness — 2026-10-28

## New result

Native C compilation no longer shares the host's browser/cartridge build-script
fingerprint. `cockpit/native` is now a small normal Cargo path dependency,
`angel-native`, with its own `build.rs`, bounded jobserver runner, static archive,
and tracked compiler/archiver/flag inputs. There is no new manual cache.

Before this refactor, `cockpit/build.rs::main` unconditionally called
`build_native_modules()` on every rerun, including changes to `HOME`,
`ANGEL_CARTRIDGES`, `WASM_BINDGEN`, and watched browser input. Four independent
compilers could run in parallel, but unrelated build-script changes still ran
all four. Removing that work is different from making that work parallel.

The host now imports the native crate. `--build-info --json` adds
`builtin_native_abi_versions`, obtained by actually calling all four statically
linked version functions, not by embedding an expected constant. The optional
hotloaded modules keep their existing loaders and FFI declarations.

## Preserved behavior

- All four native C sources remain checked in and compiled from current source.
- Native source, generator, compiler flags and portable-mode inputs remain
  registered with Cargo. Browser/cartridge inputs remain owned by the host.
- Target-specific tools, `TARGET_*`, generic tools, compiler paths containing
  spaces, cross-compilation rejection and portable Zig choices are preserved.
- Strict floating-point flags and argument ordering are unchanged from the
  immediately preceding native implementation. User CFLAGS precede the fixed
  ABI flags; this refactor does not silently change their precedence.
- The same bounded jobserver runner holds permits until every child is reaped,
  falls back to serial at one job, and removes the archive before recreating it.
- Default application features, source-freshness checks, and failure-on-build-error
  launch behavior are unchanged. Experimental Blaze is not promoted to default.

Release inventory now explicitly ships the native package and the host's build
helpers, and includes these inputs in release source identity. The runtime
`build-native-modules.py` helper is also required in source releases; previously
`build --modules` could name a helper absent from the archive. New package files
must still be staged/committed before an index-based source archive can ship them.

## Actual Cargo measurement

Receipt: `reports/performance/native-fingerprint-invalidation-v2-20261028.json`.
Driver: `scripts/perf/measure-native-invalidation.py`.

This uses the real cockpit manifest, lock, fast profile, normal features including
`glass`, Cargo 1.95.0 and `--jobs 1`. It builds and executes the actual host.
A logging compiler at a pathname containing spaces captures argv. Counts below
select only invocations for the four exact native source paths; other native
Cargo dependencies' invocations are retained separately in the JSON.

| Operation | Build command wall time | Native C compiles | Actual host ABI versions |
|---|---:|---:|---|
| Instrumented prime | 59.715 s | 4 | `[1,1,1,1]` |
| No-touch host build | 0.214 s | 0 | `[1,1,1,1]` |
| Change cartridge-root environment | 12.036 s | **0** | `[1,1,1,1]` |
| Edit native version function | 14.139 s | 4 | **`[7,1,1,1]`** |
| Restore native source | 13.435 s | 4 | **`[1,1,1,1]`** |
| Change native CFLAGS, native package only | 1.015 s | 4 | archive ABI `[1,1,1,1]` |

The cartridge-root test appends a nonexistent root, preserving the loaded
cartridges while changing a watched host input. It proves independent
invalidation, not a 12-second optimization. Archive checks independently compile
and execute a C caller as well. Archive selection follows the measured compiler's
`-o` OUT_DIR, rather than guessing the newest archive in the target tree.

The final environment-restoration build took 41.483 s and restored host ABI
`[1,1,1,1]`. Its compiler logger was intentionally removed: its reported zero
logger calls is **not** evidence of zero actual native compilation. Changing CC
invalidates other native Cargo dependencies and makes prime/restoration times
unrepresentative of an ordinary edit. These samples are not a paired end-to-end
speedup estimate. Their result is the avoided four compiles on unrelated host
invalidation, with retained native edit/restore freshness.

The initial v1 measurement is retained at
`reports/performance/native-fingerprint-invalidation-20261028.json`. It failed:
it counted all 91 host/dependency CC invocations against an expectation of four.
It did not establish native isolation. The v2 receipt corrects counting, retains
all calls, selects the exact archive, and observes the actual host ABI.

## Current normal launcher

Receipt: `reports/performance/native-fingerprint-launcher-final-20261028.json`.
Four full-feature ordinary `./bin/angelX --version` calls, Cargo backend,
development source binding off (the normal development default), no no-build or
skip-web override. The first call refreshed launcher compiler provenance and took
9.639 s. Subsequent calls took **249.4 / 246.1 / 242.0 ms**, median **246.1 ms**.
All returned `angelX-cockpit 0.2.6`.

This is version-command launch latency, not terminal/window first-frame time or
cold-cache setup time. It is not materially faster than the prior warm launch;
the benefit here is narrower invalidation on future changes.

## Verification and review

- Existing `build_fast` Cargo test target: **16 passed, 0 failed**;
  `reports/performance/native-fingerprint-build-fast-20261028.log`.
- Targeted release manifest/native-package checks: **2 passed, 0 failed**;
  `reports/performance/native-fingerprint-release-tests-20261028.log`.
- Native edit and restoration were executed through the real host, not merely
  inferred from successful compiler exit or changed archive hashes.
- Independent read-only final review found no regression blocker in this narrow
  package change; it explicitly limits the result to local Linux/fast/single-job
  measurement, not release/macOS/cross-target qualification.

A broader embedded-allowlist check was also attempted and failed for existing
unlisted dungeon card/wish-art/heart assets and a font, plus the existing test's
include of `cockpit/build.rs`. This patch admits that build-script include through
the embedded inventory. It does not broaden the artwork allowlist to hide the
other failures. See `reports/performance/native-fingerprint-release-final-20261028.log`
for the retained failing run. **Full source-release qualification is not claimed.**

## Blaze finding and next upstream contribution

The new independent native path-dependency pattern was exercised with pinned
Blaze `e8140ce0` using the bounded tiny fixture in
`scripts/perf/repro-blaze-image-freshness.py --native-dependency`.
Receipt: `reports/performance/blaze-native-dependency-freshness-20261028.json`.

| Step | Command-return wall time | Executed output |
|---|---:|---|
| Prime | 281.5 ms | `native-1` |
| True no-touch | 17.3 ms | `native-1` |
| C-source-only semantic edit | 2378.1 ms | `native-7` |
| Restore C source | 476.7 ms | `native-1` |

Every invocation exited zero and every executed output matched. This is positive
compatibility evidence for the isolated native-dependency pattern. It does not
reproduce or repair the earlier full-application stale-image failure; it does
not qualify the full application or compare Blaze's speed to the different host
Cargo measurement above.

A useful upstream PR is an executable static-library freshness regression in
Blaze's `harness/cargo_ra_jit.py`/artifact-edit suite: main Rust and dependency Rust
stay unchanged; a dependency build script owns `value.c`, emits `rustc-link-search`
and `rustc-link-lib`, and only the C file changes and is restored. Assert executed
output after command return, a real untouched control, and error refusal for
failed native compilation. Our fixture covers the positive edit/restore cycle;
forced native failure, workspace-wrapper mode and daemon restart remain to add.
No upstream PR was submitted, no global toolchain was changed, and no provider
API was called for this fixture.

## Remaining constraints

- Full-application Blaze promotion still needs repeated fresh edit/restore and
  failure-cycle proof, nested browser-build compatibility, and paired latency
  wins. A passing tiny dependency does not supersede prior negative evidence.
- A local wasm-bindgen 0.2.129 executable is still absent; the existing build
  warning says the browser uses its fallback presenter. This refactor neither
  adds that tool nor disables the browser.
- The filesystem had about 5.6 GiB free during measurement. Keep incremental
  artifacts; broad `cargo clean` would destroy the warm dev loop. A measured,
  opt-in artifact-retention policy is a separate next opportunity.
- No acceptance command is bound to the long-horizon objective. These are
  concrete local results, not a claim that startup is globally optimal or that
  the loop is complete.
