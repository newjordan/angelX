# Blaze host compiler cost versus launcher cost — 2026-10-27

## Outcome

Blaze **0.20261027.0**, pinned to
`b383b0bcf5da33816809b00a37e1c36a0e761efb`, is installed as regular files under
`cockpit/target/tools/blaze/<revision>/`. The `selected` file names that revision.
No user-global Cargo configuration, Rust replacement, or compiler symlink is
required. Its ABI is tied to `nightly-2026-10-27`; host artifacts are isolated in
`cockpit/target/blaze` rather than mixed with stable Cargo artifacts.

Ordinary source launches select installed Blaze in `auto` mode. Explicit
`ANGEL_BUILD_BACKEND=blaze` requires it; `cargo` selects ordinary Cargo.
The default `ANGEL_BLAZE_MODE=wrapper` uses Blaze's supported Cargo
workspace-wrapper protocol. Cargo plans the full build and unsupported units
compile freshly with Rust. Compiler failure stops launch rather than opening a
cached executable.

**Full-feature build and real launch work. Accelerated host edits are not yet
qualified.** A completed semantic host edit used Rust fallback after the daemon
attempt. A successful wrapper build is not evidence that Blaze linked the edit.

## Measurement method

- Linux x86_64, `fast` profile, default media/window features plus `glass`.
- Browser WASM bundle retained; no headless feature reduction or no-build path.
- `ANGEL_GLASS=0` prevents opening a window, not compiling window features.
- Development source binding disabled for the controlled edit. No provider,
  model, delegate, or Muse call.
- `time.perf_counter()` measures subprocess wall time. These are not compiler
  CPU timings or completed desktop/composer-frame timings.
- Toolchain discovery (`blaze toolchain --path`) and `rustc -vV` are measured
  independently from the already-resolved direct compiler command.
- A real semantic edit in `cockpit/src/platform/runtime_paths.rs` adds a temporary
  JSON marker to the resource status returned by `--build-info --json`.
  The built image is executed and checked for that marker. Original source is
  then restored, rebuilt, and the executed image checked for the marker's absence.
- The direct build command is the resolved nightly Cargo binary with
  `RUSTC_WORKSPACE_WRAPPER=<installed>/ra-jit-rustc`, not an unrelated Cargo
  backend. Its edit time includes the daemon attempt and fresh Rust fallback.

The benchmark script is
`reports/performance/measure-blaze-host-cost.py`. It deliberately changes and
restores a source file and can rebuild the large host twice. Run only with no
concurrent editor on that file and adequate disk space. It retains the verifier
process while loading and probing the daemon, because this tool environment
terminates surviving child services when a tool job ends. That lifecycle is not
proof that the daemon cannot persist under an ordinary desktop launch.

## Measured separation

The latest completed run with Rust-backed macro/script loading has these results
(`reports/performance/blaze-host-cost.json`):

| Measurement | Wall time | Samples / meaning |
|---|---:|---|
| Blaze toolchain discovery | **23.42 ms median** | 23.42, 22.29, 23.74 ms; 3 successful calls |
| Resolved `rustc -vV` | **4.83 ms median** | 7.04, 4.40, 4.83 ms; 3 calls |
| Full wrapper launch before edit | **296.25 ms median** | 279.00, 296.25, 300.16 ms; all returned `angelX-cockpit 0.2.6` |
| Direct unchanged build check | **200.09 ms** | 1 call; no launcher discovery or executable copy |
| Daemon readiness wait after explicit load | **46.22 s** | Separate cold worker load, not included in direct edit time |
| Direct semantic host edit | **58.759 s** | 1 call; daemon attempted, then Rust linked current source |
| Launch after the edited image was built | **525.42 ms** | Freshness check plus launch; JSON marker verified |
| Restore original source and build | **94.058 s** | 1 call; not a no-op or a second equivalent edit sample |
| Launch restored image | **640.55 ms** | Marker absent; original source bytes restored |

After removing the scoped-policy experiments below, a fresh full-feature
`bin/angelX --version` through the wrapper completed in **8.793 s**, returned
`angelX-cockpit 0.2.6`, and exited 0. This included a build-script refresh; it is
not an unchanged-launch sample. Its log and receipt are
`reports/performance/blaze-final-current-launch.{log,json}`.

Earlier completed runs measured **80.582 s** with Rust-backed daemon
macro/script loads (`blaze-host-cost-before-script-policy.json`) and **64.458 s**
before those loads were enabled (`blaze-host-cost-before-daemon-compat.json`).
All verified the semantic edit and restored source/image. These are not a
controlled before/after speed comparison: worker state, failure path and host
load differ. They show that host recompilation is measured in tens of seconds,
while repeated discovery is measured in tens of milliseconds.

The earlier Cargo unchanged-launch baseline has median **270.22 ms** across five
samples. It is not a simultaneous paired comparison. No paired Cargo semantic
edit benchmark was completed, so neither a compiler speedup nor slowdown ratio
is established. Subtracting no-op build and discovery times from total launch
would also not give an exact residual: the samples were collected separately and
include scheduling/cache variation.

## Remaining daemon blockers

1. The initial daemon rebuild tried to compile FFmpeg's `clang-sys` build script
   itself and failed with `IncompleteExpr`. The wrapper now passes both
   `RA_JIT_SCRIPTS_BY_RUSTC=1` and `RA_JIT_PROC_MACRO_SERVER=0`, making script and
   macro loads Rust-backed. These variables are scoped to the compiler child,
   not exported into the launched cockpit or its tool shells.
2. Subsequent real edits encountered V8's archive-fetch policy
   (`Cargo.fetches.lock` required) and the host's nested browser Cargo build
   within the build-script sandbox (`Read-only file system`). A target-specific
   `cockpit/Cargo.fetches.lock` now records V8 150.0.0's Linux x86_64 archive with
   its SHA-256. This is not proof of a successful daemon reload on this or other
   targets. The wrapper reported load failures and Rust linked a fresh full host
   image; no daemon-linked host edit was accepted.
3. The retained browser-build isolation uses Cargo and rustc directly from the
   outer compiler's sysroot, clears the nested environment, and prefers an
   absolute `RUSTUP_TOOLCHAIN` over build-engine compiler aliases. This prevents
   nested compiler-protocol recursion; clearing environment variables cannot
   remove an already inherited OS sandbox.
4. Workspace and package-scoped sandbox-policy experiments were removed. The
   first exposed missing example/bench declarations in the trimmed vendored
   `dotmax`; removing those declarations exposed a further upstream blocker:
   Blaze's `stand_in_front` renamed a generated shell wrapper over its own
   `build_script_build.real`. Both files were identical 278-byte shell scripts,
   and executing the script recursively re-entered `ra-jit build-script` without
   starting the compiled Rust script. The probe was stopped and its temporary
   semantic marker restored. The vendored manifest is restored; no scoped script
   exceptions, explicit workspace experiment, global sandbox bypass, or Blaze
   source patch from those experiments remains. This cancelled probe is not a
   successful edit measurement.

Standalone mode still fails on the FFmpeg script. It remains explicit and
experimental, not the default or a smaller-functionality workaround.

## Isolation and verification

- `node --test tests/scripts/native-launch.test.mjs`: **32 passed** after the
  daemon-compatibility fix (earlier in this work). The regression first failed on the missing
  `RA_JIT_PROC_MACRO_SERVER=0`, then passed with the fix.
- A real full-feature wrapper build completed and produced both host binaries.
- Final real `bin/angelX --version` launches: **3/3 passed** through Blaze's
  wrapper; stdout `angelX-cockpit 0.2.6`. The additional post-rollback real
  launch also passed with default features plus `glass` and browser packaging.
- The completed semantic-edit probes verified changed executable behavior and
  verified original source and executable restoration.
- Blaze launch images are private `cp -p` copies, not hardlinks to the image
  Blaze may patch. The build/launch-copy transaction uses a per-target `flock`.
- A launcher was changed during an earlier active Bash build command, causing a
  shell failure despite successful compilation. Stable-file reruns passed;
  subsequent launcher edits were kept outside active launch measurements.

Raw evidence:

- `reports/performance/blaze-host-cost-summary.json`
- `reports/performance/blaze-host-cost-before-daemon-compat.json`
- `reports/performance/blaze-host-cost-before-script-policy.json`
- `reports/performance/blaze-host-cost.json` (completed compatible run)
- `reports/performance/blaze-final-launch.json`
- `reports/performance/blaze-final-current-launch.json` (post-rollback verification)
- `reports/performance/blaze-host-cost-double-wrapper-experiment.json` (cancelled)

The hookup is functional, but the remaining performance task is to make the
full host's daemon build-script reload succeed and then measure an actually
daemon-linked semantic edit against a paired Cargo edit. Discovery caching alone
cannot address a tens-of-seconds recompilation.
