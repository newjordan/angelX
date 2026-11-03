# Startup and native edit-to-load measurements — 2026-10-27

> **Correction:** the warm startup timings below used `ANGEL_NO_BUILD=1` and
> do not measure the operator requirement that plain `angelX` opens current
> source. They are not acceptance results for that goal. The normal launcher
> now checks current source before opening and refuses stale fallback; see
> [the corrected current-source report](angelX-current-source-20261027.md).
> The native reload result applies only to four existing modules, not arbitrary
> Rust host edits or the general `angelX` startup path.

## Change

- `bin/angelX` skips the legacy process-table scan for machine commands. Those
  commands do not rotate cockpit stderr logs. Interactive startup retains the
  scan when modern per-instance run links have not already identified another
  instance.
- `angelX build --modules` builds only the four existing native ABIs, without
  Cargo or a cockpit restart. It rejects host build profile/background options
  and respects `ANGEL_NO_BUILD=1`.
- `ANGEL_NATIVE_MODULES=1` selects the existing 500 ms background reload workers.
  Default startup remains unchanged: no new watchers or runtime dependency.
  Explicit per-module paths, including empty opt-outs, win.
- The Python stdlib-only builder serializes concurrent builds with `flock`,
  fingerprints source, compiler identity, platform and flags, verifies output
  content before cache hits, and publishes complete libraries by atomic rename.
  Compilation failures retain the previous library. Publication is independent
  per ABI, not an all-modules transaction.

```sh
# Opt into native reloads on a new cockpit instance.
ANGEL_NATIVE_MODULES=1 angelX --term

# After editing cockpit/native/*.c, publish changes from another terminal.
angelX build --modules
```

Existing instances launched without the opt-in do not acquire reload workers.
Modules are operator-trusted native code, not sandboxed plugins. Compiler flags
match the existing native implementations' floating-point constraints.

## Measurements

Local warm-cache observations, not cross-machine guarantees:

| Measurement | Result | Scope |
|---|---:|---|
| Launcher `--version`, before | 85.2 ms median / 5 runs | Existing fast image, no builds, Glass disabled |
| Launcher `--version`, after | 62.1 ms median / 7 runs | Same fast image and launch controls; about 27% less total time |
| Direct fast binary `--version`, after | 38.2 ms median / 7 runs | Separates binary load from launcher work |
| PTY composer startup, after | 187.4 ms median / 3 runs | Fresh home, 140×40 terminal, practice driver, motion off |
| First four-module build, full CLI | 984.5 ms | Compilation portion: 938 ms |
| Unchanged four-module build, full CLI | 45.5 ms | Cache/check portion: 1 ms; no outputs replaced |
| One native edit → new active generation | 497.0 ms | Isolated production-loader probe; compile portion: 246 ms |
| Four compiled shared libraries | 136 KiB total | Existing host executable is not reduced by this change |

PTY acceptance uses the pre-existing smoke test's `angelX` and `Write a message`
startup markers and verifies ICANON/ECHO are off. This is a terminal composer
frame observation, not a Glass GPU presentation measurement. There was no
pre-change PTY baseline, so no PTY improvement percentage is claimed.

The edit-to-load probe includes the actual `lib/native/native_module.rs`
production loader and the actual skill router C source plus a generation marker.
It asserts that the new generation returns 2 while a pinned old generation still
returns 1. This measures production reload machinery in isolation, not a full
interactive model turn. No model API calls were made.

Raw samples, probe source and receipts:
`artifacts/startup-native-build-20261027/measurements.json`.

## Verification

- `node --test tests/scripts/native-launch.test.mjs`: **25 passed, 0 failed**.
  Host test execution was reported as reward-unlabeled; this is not an
  independently audited benchmark result.
- `python3 tests/python/test_build_native_modules.py -v`: **4 passed**.
  Covers unchanged builds, one-source publication, old-generation survival,
  failed compilation cleanup, flag changes and corrupted-output recovery.
- All four real libraries loaded with `dlopen` and reported ABI version 1.
- Production-loader probe: new generation adopted; old generation still callable.
- Python discovery through the cockpit wrapper failed before running tests
  (`tests/python` not importable in its selected context); direct file execution
  above completed the intended suite successfully.

## Remaining scope

Ordinary Rust host edits still require Cargo. This does not shrink the current
181 MiB fast host or turn all Rust behavior into reloadable code. Broad data or
host boundary extraction needs separate measurements and design work.

Blaze is not installed as a command or integrated as a compiler in this checkout.
No speculative Blaze dependency was added. A future compiler backend can target
these existing native ABIs; compare its end-to-end edit-to-load time and validate
ABI parity before replacing the current host C compiler.
