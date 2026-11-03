# `angelX`: current-source startup and retained browser functionality

## Contract

The normal installed `angelX` command must check the current checkout with Cargo
before opening the application. A compilation failure must fail closed rather
than launch an old image. `--version`, terminal and desktop launch use the same
application/features. A different optimization profile is not a second product;
an omitted feature or stale image is not an acceptable speed optimization.

`ANGEL_NO_BUILD=1` remains an explicit operator opt-out, not a default or a valid
current-source benchmark. Background compilation is explicit and cannot waive
an ordinary launch's source check. Prebuilt installations retain their separate
no-source install contract.

## Changes

- Ordinary source launches perform the Cargo freshness check even if an image
  already exists; a failed check never opens the cached image.
- The incremental `fast` profile is the source-launch default. Shipping
  optimization remains available through `angelX build --release`.
- Machine commands no longer perform interactive log-rotation process scans.
- Desktop, terminal and inspection launches retain the default media/Glass
  features instead of compiling smaller inspection-only images.
- Every normal profile includes the browser bundle. The former working-profile
  default that omitted it is removed. `ANGEL_DEV_SKIP_WEB_BUNDLE=1` is an explicit
  development-only opt-out and shipping profiles reject it.
- Browser assets are stored losslessly in the executable when compression helps.
  Small or incompressible files stay plain. Asset names, MIME types, index and
  decoded bytes are retained. Music remains in its existing beside-binary files.
- The build verifies packed assets against the original bytes. The shared runtime
  decoder bounds output by the recorded original size and rejects corruption,
  truncation and length mismatches. Decoding occurs only when opening a browser
  hosting bundle, not at cockpit startup. It reuses `miniz_oxide`, already in the
  image/HTTP dependency graph; no external compressor or new package version.
- Existing native C modules can still be rebuilt atomically with
  `angelX build --modules`, without a Cargo host rebuild. Reload workers remain
  opt-in (`ANGEL_NATIVE_MODULES=1`); this is not general Rust-host hot reload.

## Measurements and limitations

The installed entrypoint was `~/.local/bin/angelX`; no alternate
binary, no-build flag or smaller feature set was used for the current-source
machine measurements.

Before lossless packing, with browser packaging restored:

| Observation | Time | Meaning |
|---|---:|---|
| First `angelX --version` | 84.611 s | Actual source rebuild including browser packaging; **not** unchanged startup |
| Subsequent `angelX --version` | 318.3 ms, 268.4 ms | Unchanged source, Cargo freshness enabled; **not** composer readiness |

Raw data: `reports/performance/angelX-full-current-source-version.json` and its
three adjacent launch logs. These samples do not establish a general Rust edit
to composer latency or a controlled improvement over the former host rebuild.

Lossless packing has a production regression comparing the decoded embedded
`delve.wasm` byte-for-byte with the actual nested browser build output. Final
packing, executable-size and normal-command measurements are recorded in
`reports/performance/angelX-packed-bundle.json`.

| Post-packing observation | Result | Scope |
|---|---:|---|
| Normal host before packing | 217,567,432 bytes (207.5 MiB) | Browser bundle retained |
| Normal host after packing | 204,638,728 bytes (195.2 MiB) | Same default functionality |
| Executable size saved | 12,928,704 bytes (12.3 MiB / 5.94%) | No asset bytes discarded |
| Targeted test gate including compilation | 55.81 s | 10 tests passed; not an edit-to-load timing |
| First post-test `angelX --version` | 99.929 s | Includes rebuild after test-only dependency features |
| Subsequent `angelX --version` | 457.5 ms, 317.6 ms | Unchanged source, freshness enabled |

**These timings do not show an improvement in unchanged startup or rebuild
latency.** The verified improvement is executable size. Do not compare the
cold/rebuild sample with unchanged samples, or the test gate's compilation time
with a controlled host-edit benchmark. The normal source image was restored
through the installed command after the test gate; the receipt distinguishes
its size from the intermediate test-feature image.

The earlier 62 ms machine and 187 ms PTY results in
[startup-native-build-20261027.md](startup-native-build-20261027.md) used
no-build/feature/practice controls and do **not** satisfy this current-source,
full-application contract. The approximately 497 ms native-module result is a
production-loader isolation measurement for four existing C modules, not all
host edits or a full interactive model turn.

### Desktop probe: not accepted

The desktop probes did not establish a reliable current-source composer-ready
timing. OCR first misread `Write` as `Hrite`; subsequent captures did not confirm
the composer. A ready marker or mapped window is not a completed composer frame.
No desktop startup timing is accepted from these probes. The final probe used
process/executable ownership, closed its test instance, and confirmed the two
pre-existing operator Glass processes remained. No prompt or model API call was
made.

### Renderer toolchain and Blaze

The browser page and WASM are included, but the build reports that optional
browser GPU renderer generation lacks `wasm-bindgen 0.2.129` and uses its existing
presenter fallback. That is not evidence of all-media/renderer parity, nor a
Glass-versus-Kitty performance result.

At the time of the measurements above, neither `blaze` nor `blazer` was
available. A subsequent project-local pinned Blaze integration now builds and
launches the full host through its Cargo workspace wrapper. Its separate
[compiler/launcher measurement](blaze-host-cost-20261027.md) distinguishes
25 ms discovery from an 80.58 s host edit with Rust fallback. Daemon acceleration
is not qualified; the integration preserves isolated ABI/build directories and
never opens a stale image after a failed build.

## Verification

- `node --test tests/scripts/native-launch.test.mjs`: **26 passed**. Covers
  ordinary current-source launch, changed-source rebuild requests, failure
  refusing stale fallback, feature parity, strict no-build and explicit
  background/module-build behavior.
- Direct build-policy unit gate before packing: **3 passed**.
- `cargo test --manifest-path cockpit/Cargo.toml --locked --profile fast --test build_fast`:
  **10 passed** after packing. Covers normal bundle inclusion, profile/watch
  policy, exact lossless round trips, small/incompressible inputs, invalid data,
  and the generated embedded WASM against real build output. Its normal binary
  target also compiles the actual lazy runtime loader.

Remaining work is real host edit-to-load reduction, a reliable completed desktop
composer measurement and renderer/toolchain qualification—not another name,
stale binary, or reduced-functionality build.
