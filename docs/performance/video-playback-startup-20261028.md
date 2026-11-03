# File-video feature boundary: remove unused capture startup work

2026-10-28 · Linux x86-64, Rust 1.95.0, glibc 2.44, normal `fast` source build.
Local warm-cache measurements, not cold boot or a universal startup guarantee.

## Change installed

The cockpit uses Dotmax's file-video player, not its webcam API. Previously
`scryglass-video -> dotmax/video` also enabled FFmpeg's default features. In
particular `ffmpeg::init()` registered devices, retaining `libavdevice` and its
capture/filter/platform dependencies at process startup.

* New additive Dotmax `video-playback` feature: image rendering, FFmpeg format,
  codec and software scaling, with FFmpeg defaults disabled.
* Existing Dotmax `video` remains the webcam-inclusive API: it enables
  `video-playback` and FFmpeg `device`. Webcam exports/helpers/errors stay gated
  by `video`; playback module/helpers/`VideoError` use `video-playback`.
* The cockpit's existing default `scryglass-video` now selects `video-playback`.
  File-video decoding, stills, browser bundle and Glass remain in the normal app.
  No new operator setting or special benchmark application is needed.
* No sandbox-policy, helper-selection, launcher-lifetime, compiler-default or
  global Cargo configuration change is part of this treatment. Existing explicit
  helper pins and the sibling/self fallback remain unchanged.

This avoids eager linkage; it is not a new `dlopen` plugin system. FFmpeg's
remaining file-decoder libraries are still loaded normally. It removes 42
entries from this machine's `ldd` closure (**162 -> 120**, including the loader
and vDSO entries in both totals), with no new entries. `libavdevice`,
`libavfilter`, JACK/Pulse/ALSA capture dependencies and several graphics/filter
libraries disappear from that closure. These are platform/package-specific
numbers, not a promise that all graphics libraries are absent at runtime:
Glass can initialize graphics independently when actually used. The executable
itself shrank only **800 bytes**, not by the size of those shared libraries.

## Measurements

Both images were built through the same source launcher/profile/environment,
with defaults enabled, source binding off and no provider calls. Baseline sources
were saved before editing. Trusted images and every timed probe are SHA-256
identified. Repeated direct-image tests alternate B/C then C/B; three untimed
warmups per image/case are excluded. Every `--version` output must match; rejected
sandbox entry must report the missing policy, never silently execute a command.

| Measurement | Before median | After median | Scope |
|---|---:|---:|---|
| Direct `--version`, first comparison | 26.30 ms | 14.70 ms | 40 samples each, alternating |
| Direct cockpit rejected sandbox entry | 27.76 ms | 16.41 ms | 40 samples each, alternating |
| Source launcher including Cargo validation | 227.42 ms | 215.56 ms | 16 samples each, sequential batches |
| Offline PTY first output | 28.17 ms | 18.62 ms | 6 samples each, alternating |
| Offline PTY visible draft | 45.07 ms | 35.53 ms | 6 samples each, alternating |
| Offline PTY input echo | 0.627 ms | 0.701 ms | No demonstrated improvement |
| Final installed image `--version` | 27.79 ms | 16.77 ms | 24 samples each, alternating |
| Final installed image rejected sandbox entry | 28.92 ms | 17.61 ms | 24 samples each, alternating |

The sequential launcher batches suggest **11.85 ms / 5.2%** lower warm source
launch cost; they are less controlled than the alternating direct-image results.
The latter reproduce roughly **11 ms** of process-entry improvement on the final
installed source image. The PTY fixture uses isolated state, practice driver,
no live provider, terminal motion off and the existing terminal-ready probe;
it measures warm direct images, **not** ordinary Cargo-inclusive terminal launch
or Glass window readiness. Six pairs are a small readiness check.

Rejected entry is an entry-cost probe, **not** a complete confined command and
not the cost of every tool call. A deployed sibling `angel-sandbox` or explicit
helper pin avoids the cockpit loader entirely; only cockpit/self-helper fallback
can benefit from this loader reduction. No cold-cache or macOS result is claimed.

## Correctness and rebuild qualification

The existing focused Scryglass verifier passed **55/55** before final review
refinements and **55/55** on the final source. Actual fixture generation was
available; the MP4 tests did not take their skip path. The suite exercised:

* MP4 decode and EOF restart;
* nonempty video pixels, seek, resize, hide/reopen and decoder release;
* local network-playlist rejection before following its network reference;
* reveal/fault/pause/state behavior and source-rate sampling.

`cargo check --features dotmax/video` through the cockpit manifest also passed,
compiling the legacy webcam-inclusive path. Physical webcam capture was not
exercised. Standalone checking inside the trimmed vendor directory cannot be
used: its manifest declares examples absent from this checkout. The first
playback-only compile exposed a missed `VideoError` gate; it was fixed before
the successful build and retained test logs. Reviewer requests to qualify
self-helper costs, correct feature guidance and remove the new probe's Python
3.11-specific hashing API were incorporated.

A final real semantic host edit changed executed `--version` output, then source
restoration removed the marker from the executed image. Six launches succeeded:
prime **10.50 s**; untouched **213.76 / 217.96 / 229.97 ms**; edit **12.15 s**;
restore **11.11 s**. The restored executable's SHA matches the earlier candidate.
This verifies freshness and restoration, **not a compile-time improvement**.
First/profile-switch costs and the roughly 10–12-second host rebuild floor
remain. `cockpit/Cargo.lock` is byte-identical to the saved treatment baseline.

## Rejected helper-snapshot experiment

Before this treatment, an automatic private helper snapshot/pin was explored.
Its entry probe was much faster than cockpit re-exec, but review found detached
Glass ownership/lifetime and inherited-pin cleanup problems; an existing direct
helper process-ownership test also failed its reaping assertion. **That proposal
was withdrawn**, restoring only its own launcher/test edits from the saved
follow-up baselines. Earlier independent launcher work remains in the tree.
Rejected files and failure logs are retained separately, not installed or counted
as qualification of this feature change. Fixing helper ownership requires a
separate lifecycle/security design, not merely exporting a faster helper path.

## Reproduction and evidence

All files below are under `reports/performance/video-playback-boundary-iteration11/`:

* `baseline-sources.json` and `*.baseline.txt`: treatment input snapshots.
* `baseline-source-launch.json`, `candidate-source-launch.json`: sequential
  source-launch samples; three preceding warmups per batch excluded.
* `alternating-images.json`, `final-source-image.json`: paired samples, image and
  probe hashes. The first measured probe and the final measured probe are frozen
  separately because the hash implementation was made Python-3 compatible.
* `alternating-pty.json`: all 12 measured readiness rows and fixture switches.
* `library-closure.json`, `*-dynamic.txt`, `*-ldd.txt`: eager link closure.
* `final-scryglass-tests.log`, `legacy-video-check.log`: existing verifier output.
* `final-semantic-rebuild.json` and associated logs: executed edit/restore checks.
* `blaze-features.json` and step logs: separate compiler feature-graph fixture.

```sh
# Existing trusted images only; output must be a fresh path.
python3 scripts/perf/measure-launch-images.py NEW.json \
  --baseline /path/to/baseline/angel --candidate cockpit/target/fast/angel --runs 40

# From the repo root; use the same pinned toolchain/environment as bin/angelX.
ANGEL_BUILD_PROFILE=fast \
ANGEL_BUILD_RUSTC="$(rustc -vV)" \
cargo test --locked --manifest-path cockpit/Cargo.toml --profile fast \
  --bin angel ui::scryglass::tests:: -- --test-threads=1 --nocapture

python3 scripts/perf/measure-dev-rebuild.py NEW-REBUILD.json --backend cargo --case version
python3 scripts/perf/repro-blaze-image-freshness.py NEW-BLAZE.json --feature-dependency
```

See [the new Blaze feature-graph regression opportunity](blaze-feature-boundary-20261028.md).
Normal source launch remains Cargo-qualified; tiny fixture freshness does not
clear the previously observed full-host stale-output failure.
