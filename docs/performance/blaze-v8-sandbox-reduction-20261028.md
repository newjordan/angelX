# Blaze/V8 sandbox compatibility: a reduced upstream test case

2026-10-28 · local investigation · no upstream issue or PR submitted

## Finding

The full-feature angelX workspace-wrapper attempt at pinned Blaze
`b383b0bcf5da33816809b00a37e1c36a0e761efb` returned semantically fresh
edit/restore executables, but **that was not a successful daemon build**.
The edit log explicitly reports a build-script failure followed by
`rustc links angel`. The semantic edit took 125.713 s, including this failed
attempt and fallback. The log identifies V8 150.0.0 attempting to write
`OUT_DIR/../../v8.fslock`, outside the sandbox's writable `OUT_DIR`.

This is a compatibility boundary, not evidence of a sandbox vulnerability or a
reason to disable isolation. Blaze deliberately enforces Cargo's documented
OUT_DIR convention. A global writable target directory or an unsandboxed V8
allowlist is **not** the proposed fix.

Full application evidence:

- `reports/performance/bundle-package-blaze-20261028.json`
- `reports/performance/bundle-package-blaze-20261028-semantic-edit.log`

The same failure log also reports the host build script trying to write
`~/.cache/ra-jit/store`. The parent-lock reduction below does **not** explain or
fix that separate nested-build interaction.

## Offline reduction, including output-bound controls

Run from this checkout with its already-installed pinned compiler:

```sh
python3 scripts/perf/repro-blaze-image-freshness.py \
  reports/performance/blaze-parent-lock-NEW.json --parent-lock
```

The output name must be new. The fixture uses only the Rust standard library,
`--offline`, an isolated temporary daemon/cache/project, and a checkout path
containing spaces. It stops its daemon before deleting temporary files.
It leaves the sandbox enabled and declares no sandbox opt-outs. Direct
`ra-jit cargo build`, not the Cargo workspace wrapper, prevents fallback from
turning a denial into an apparent Blaze success.

Each build script writes a case-specific Rust constant into OUT_DIR and the
binary includes it. Successful controls must execute and print the matching
constant, not just a shared fixed string from an old executable.

| Case | Engine | Lock destination | Result | Observed time |
|---|---|---|---|---:|
| `cargo-parent` | pinned toolchain Cargo | `OUT_DIR/../../v8.fslock` | exit 0, prints `cargo-parent` | 0.107 s |
| `blaze-out-control` | direct Blaze | `OUT_DIR/v8.fslock` | exit 0, prints `blaze-out-control` | 7.686 s |
| `blaze-parent` | direct Blaze | `OUT_DIR/../../v8.fslock` | exit 101, read-only denial naming `v8.fslock` | 0.131 s |

These are reduction timings, **not a compiler speed comparison**: initialization
and case order differ. Receipt (script/compiler/binary hashes and commands):
`reports/performance/blaze-parent-lock-bound-output-20261028.json`, with its
three adjacent logs. Daemon stop exited 0. An earlier reduction with a shared
fixed binary string is retained as preliminary evidence, but is superseded for
control freshness by this output-bound run.

## Source localization

At this pin:

- `crates/ra-jit/src/cargo/script_sandbox.rs:1-14` documents the intended
  OUT_DIR-only write policy; `plan` starts its writable list with `script.out_dir`
  at line 264. The entire target is readable, not writable.
- `crates/ra-jit/src/cargo/fetches.rs:126-145` already knows about V8 prebuilt
  downloads; its cited V8 version is **152.2.0**, not angelX's **150.0.0**.
  Merely having a fetch override does not prove compatibility with older V8
  script state/layout.
- In published V8 150.0.0 `build.rs:96-100`, the script comments that it stores
  state in a parent of OUT_DIR, then unconditionally acquires the lock before
  the normal build/download branch. `acquire_lock` at lines 126-139 opens
  `OUT_DIR/../../v8.fslock`. `build_dir` at line 617 also derives shared storage
  above OUT_DIR. Redirecting the lock alone may therefore expose the next write
  failure; a lock-only patch is not a complete proposed solution.

## PR-sized opportunities

1. **Regression/diagnostic PR in Blaze.** Add the stdlib-only parent-write
   fixture beside existing build-script sandbox harness coverage. Assert an
   output-bound OUT_DIR control and the exact parent-write denial with no rustc
   fallback. Preserve the deny-by-default policy. Add a version-aware regression
   for V8's download/state contract; a fetch override should not imply that a
   package's side-effect layout is supported.
2. **Compatibility fix, preferably coordinated with rusty_v8.** Move build
   state and lock ownership into a supported per-build OUT_DIR/private scratch
   layout, with correct concurrent-build semantics and invalidation. If Blaze
   needs an adapter, narrowly model/package-bind that state and its publication
   rather than granting the whole target or home cache writable. Test the actual
   V8 versions/features used; do not blindly reuse the 152.2.0 assumption.
3. **Keep image-publication work separate.** Earlier executable edit/restore
   regressions and the native-dependency fixture remain useful proposals in
   [the existing upstream assessment](blaze-upstream-opportunities-20261028.md).
   The new denial is not a new stale-image reproduction, nor does successful
   rustc fallback clear the previously observed daemon freshness risk.

Acceptance for an eventual compatibility PR: offline direct-Blaze build with
locked prebuilt inputs; explicit OUT_DIR and forbidden-parent controls; case-
specific generated output; repeated edit/restore/failure/restart cycles; no
sandbox expansion; full-application evidence distinguishing daemon success
from fallback. None of those unrun integration checks is claimed passed here.

## Decision

Keep Cargo as angelX's default. Retain explicit Blaze opt-in and its warning.
The reduced test and diagnosis are ready for an upstream discussion; there is
no submitted PR, no installed upstream fix, and no automatic Blaze promotion.
