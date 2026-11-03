# Network / browser-stage performance

Branch: `angel/openai-2704093-1792902165535310152-0`.
Baseline: `775388f0e0a704a19529159e31ae410aedd1cdec` (from `delve/swap-f`).
Changes are confined to client/stage wire/render performance, the development
probe, and cockpit `together_seat` input transport. No squires, gameplay,
`hireling/home/rules.rs`, protocol version, dependencies, or live deployment changes.

## Root cause and changes

The alleged ~1 MB/frame is **client-to-presenter stage serialization**, not the
host-to-seat WebSocket stream. The compact projectile/course snapshot stream
was already below 1.5 Mbps/seat p95 at baseline. Treating stage bytes as network
bytes would misdiagnose this workload.

* Browser scenery uses a stable illumination bake and existing client-side
  stage lights for torch flicker. Its cache key includes wire mode; terminal
  lighting still advances normally, with a scoped guard restoring caller state.
* Browser tiles belong to room coordinates, not camera-window offsets. Unchanged
  interior tiles survive camera movement. Transparent tiles send no picture or
  quad; nonempty tiles send only their ink bounds. IDs remain bounded in the
  existing tile namespace; oversized spans fall back to whole pictures.
* Flat browser figures have independent cached pictures rather than one crowd
  atlas. Native/window atlas behavior is retained. No wire-format changes: all
  presenters already support the picture sizes and quad placements used here.
* Omit empty projectile `traits`; existing serde defaults restore them. Keep the
  existing compact course/delta path, rather than replacing it or sending pixels.
* Coalesce only consecutive identical cockpit held-key reports while stalled.
  Quick press/release and focus-loss transitions remain ordered. Existing
  per-seat snapshot pacing/budget machinery is retained, not replaced with an
  unmeasured transport policy.
* Add a bounded real-WebSocket headless client using the actual engine runtime,
  snapshot reader and input sampler, with an optional delayed-command/ACK mode.

## Measurement method

`net_perf_six_seat_bench` extends the original `floor3_bench` without removing
it. Same seed, heaviest fight per floor, six invulnerable heroes, enemies healed
per tick, 30 warmup ticks plus 300 measured ticks at 30 Hz; 384x296 frame. Default
bullets and `foe_shots=50`, `foe_shot_speed=1.5` ("50/1.5"). Identical benchmark
instrumentation was applied to a detached baseline worktree; baseline production
sources and dependency lockfile stayed unchanged. Profile `fast` in both runs.
Raw logs: `<scratch>/net-perf/baseline-final.log`, `after-final.log`.

Wire bytes below count compact snapshot **bodies**, excluding handshake, framing,
TCP/TLS overhead and initial warmup/full snapshots. Rolling one-second windows
contain 30 consecutive samples per seat. These are payload rates, not NIC rates.
Stage bytes are the encoded frame handed to the presenter, not GPU upload timing
and not network traffic. Decimal KB means 1,000 bytes. Timing is a single local
run, not a controlled statistical CPU speedup claim. Before → after throughout.

### Six-seat payload and stage matrix

| Floor | Bullets | Wire B/s p50 → | Wire B/s p95 → | Wire Mbps p95 → | Stage KB/frame mean → | Stage KB/frame p95 → | Mean reduction |
|---|---|---:|---:|---:|---:|---:|---:|
| 3 | default | 33,685 → 33,595 | 55,330 → 55,063 | 0.4426 → 0.4405 | 1034.6 → 299.7 | 1725.2 → 525.5 | 71.0% |
| 3 | 50/1.5 | 23,324 → 23,276 | 38,342 → 38,206 | 0.3067 → 0.3056 | 876.8 → 200.0 | 1523.6 → 373.7 | 77.2% |
| 4 | default | 27,046 → 27,046 | 39,256 → 39,256 | 0.3140 → 0.3140 | 736.7 → 198.5 | 1359.7 → 274.2 | 73.1% |
| 4 | 50/1.5 | 22,443 → 22,443 | 28,461 → 28,461 | 0.2277 → 0.2277 | 663.0 → 146.8 | 1320.9 → 186.2 | 77.9% |
| 5 | default | 27,679 → 27,494 | 34,510 → 34,368 | 0.2761 → 0.2749 | 805.4 → 222.1 | 1315.3 → 306.8 | 72.4% |
| 5 | 50/1.5 | 24,122 → 24,042 | 26,120 → 26,075 | 0.2090 → 0.2086 | 652.3 → 155.2 | 1144.6 → 198.4 | 76.2% |
| 6 | default | 42,576 → 41,932 | 63,595 → 63,459 | 0.5088 → 0.5077 | 983.4 → 376.8 | 1501.4 → 648.3 | 61.7% |
| 6 | 50/1.5 | 29,633 → 29,523 | 35,120 → 34,684 | 0.2810 → 0.2775 | 820.3 → 259.5 | 1320.1 → 511.8 | 68.4% |

Stage frame means fall **61.7–77.9%**. Network improvement is small (0–1.3% in
this deterministic matrix), as expected from omitting a small default field.
The worst after payload rate is **0.5077 Mbps/seat p95**, versus 0.5088 before.

### Local host work and client costs

Host-work sample = step + snapshot clone + publisher + all six seat diff/encodes
for the same tick, summed **before** sorting. This excludes socket scheduling
and I/O; real server telemetry is reported separately below. Read = snapshot
client decode/apply; draw = client preparation; stage = client stage encode.
All times are milliseconds, before → after. Tick budget is 33.33 ms at 30 Hz.

| Floor | Bullets | Host work p50 → | Host work p95 → | Host work max → | Client read p95 → | Client draw p95 → | Stage encode p95 → |
|---|---|---:|---:|---:|---:|---:|---:|
| 3 | default | 2.459 → 1.760 | 3.362 → 2.327 | 4.551 → 3.157 | 1.975 → 1.301 | 3.949 → 2.159 | 3.734 → 2.506 |
| 3 | 50/1.5 | 2.144 → 1.452 | 2.812 → 1.976 | 3.989 → 5.558 | 1.180 → 0.776 | 3.799 → 2.036 | 3.240 → 2.682 |
| 4 | default | 2.372 → 1.574 | 3.002 → 1.947 | 3.490 → 2.349 | 1.318 → 0.806 | 3.904 → 1.892 | 3.261 → 3.633 |
| 4 | 50/1.5 | 2.114 → 1.447 | 2.736 → 1.908 | 3.055 → 3.306 | 1.214 → 0.724 | 3.538 → 1.760 | 3.202 → 2.731 |
| 5 | default | 2.344 → 1.677 | 3.043 → 2.149 | 3.878 → 4.135 | 1.211 → 0.800 | 3.242 → 1.787 | 3.090 → 3.044 |
| 5 | 50/1.5 | 2.110 → 1.413 | 2.688 → 1.887 | 3.815 → 3.463 | 1.176 → 0.697 | 3.087 → 1.756 | 2.831 → 2.626 |
| 6 | default | 2.830 → 2.143 | 3.470 → 2.863 | 3.819 → 6.365 | 1.654 → 1.195 | 3.291 → 2.295 | 3.109 → 3.269 |
| 6 | 50/1.5 | 2.320 → 1.716 | 2.797 → 2.171 | 3.318 → 2.605 | 1.028 → 0.763 | 3.169 → 1.955 | 2.947 → 2.824 |

Sparse tile bounds/scans and independent pictures trade CPU/allocation work for
far fewer serialized pixels; this is not a universal CPU win. An earlier
implementation allocated a temporary RGBA Vec for every tile row; removing
that avoids those row allocations. Even in the final run some stage encode
costs regress. Do not infer mobile frame pacing or WAN quality from these local
numbers; renderer GPU time is not included. Full per-phase p50/p95/max are in
the raw benchmark logs. Baseline and after were sequential, not interleaved.

## Remote real-wire soak

SSH only to `root@108.240.82.27:54258`. Owned workspace
`/tmp/jordan-net-perf-18864`, listener **127.0.0.1:18864**. Live 8840 was never
restarted, rebound or modified. First dynamic binary could not run on the box's
older glibc; static `x86_64-unknown-linux-musl` binaries resolved this.

Six simultaneous real WebSocket seats for **45 seconds per cell** (16 cells:
baseline/after × floors 3–6 × bullet settings), plus 65 seconds after on floor 6
with seat 5's command/ACK transmission coalesced to every 300 ms. The first
three seconds are warmup. Total 4,710 seat-seconds including warmup. This uses
`--scene fight --depth N` on the existing dev host, not the identical seeded
benchmark fight: combat evolves with connection/input timing, so it is a
continuity/pressure soak, not a deterministic wire-byte comparison. Observed
78–231 peak shots at baseline and 76–231 after in the ordinary cells. The
headless clients decode and sample input; they do not render or play audio.
The supplied Python `/play` probe speaks the old protocol, hence the current
runtime-based `delve-net-probe` was used instead.

Below: worst per-seat one-second payload p95, worst decode p95, worst receive
gap p95 across six seats. Mbps counts encoded Snapshot messages including the
protocol envelope, but not WebSocket/TCP/TLS. Host tick p50 = median of one-second
reported p50 bins, **not an exact full-run median**; max is max of reported bin
maxima. Host logs expose p99 (not p95), so no exact remote tick p95 is claimed.

| Floor | Bullets | Mbps p95 → | Decode p95 ms → | Gap p95 ms → | Host tick bin-p50 ms → | Host tick max ms → |
|---|---|---:|---:|---:|---:|---:|
| 3 | default | 0.3100 → 0.3137 | 2.97 → 2.91 | 36.90 → 36.81 | 3.54 → 3.28 | 7.34 → 7.86 |
| 3 | 50/1.5 | 0.1936 → 0.1997 | 2.89 → 2.73 | 36.66 → 37.19 | 3.28 → 3.67 | 7.08 → 8.39 |
| 4 | default | 0.2652 → 0.2513 | 3.03 → 2.91 | 37.29 → 37.07 | 3.67 → 3.67 | 8.39 → 7.60 |
| 4 | 50/1.5 | 0.1552 → 0.1557 | 2.67 → 2.74 | 37.04 → 37.02 | 3.41 → 3.28 | 6.82 → 8.39 |
| 5 | default | 0.1812 → 0.1760 | 2.52 → 2.63 | 36.95 → 37.07 | 3.15 → 3.28 | 10.49 → 7.86 |
| 5 | 50/1.5 | 0.1256 → 0.1257 | 2.56 → 2.55 | 36.90 → 36.83 | 3.28 → 3.15 | 7.34 → 7.60 |
| 6 | default | 0.1748 → 0.1782 | 2.69 → 2.63 | 36.83 → 37.08 | 3.41 → 3.28 | 8.13 → 8.39 |
| 6 | 50/1.5 | 0.1285 → 0.1255 | 2.46 → 2.58 | 36.89 → 36.99 | 3.01 → 3.15 | 6.55 → 6.82 |

All cells: six seats, ~30 snapshots/s, **zero tick jumps and zero fatal/decode
failures**. Highest after per-seat payload p95: **0.3137 Mbps**. Highest after
reported host tick maximum: **8.39 ms**, comfortably below 33.33 ms; this is
not evidence of a big host speedup. Delayed seat: all six ~30 snapshots/s, zero
jumps/fatals, worst gap p95 36.86 ms and max 48.15 ms, worst payload p95 0.1682
Mbps, 215 peak shots. Delaying commands/ACKs is not real WAN packet loss.

Raw remote seat JSON, host logs and run log are copied to
`<scratch>/net-perf/remote/`. `remote-soak.sh` records exact launch commands.
Owned remote and local listeners and probe processes were checked stopped.
No probe host remains running; no deployment was performed.

## Verification and reproduction

* TDD: camera tile reuse, transparent-padding omission, empty projectile traits,
  independent figure pictures, and repeated-key coalescing failed before the
  corresponding fixes (`red-*.log`). Wire crowded-stream fingerprint updated
  intentionally for omitted empty traits; authoritative seat-view checks remain.
* Added tile pixel reconstruction across camera movement, ID wrap, whole-picture
  fallback and return; torch test checks Arc cache reuse, wire/terminal cache
  separation and nested guard restoration. The earlier torch integration check
  alone passed before the fix, so it is not claimed as a red regression.
* Final `cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast`:
  **2,837 passed, 0 failed, 304 ignored**. Original baseline verifier: 2,830 passed,
  0 failed, 303 ignored. The new ignored export adds the extra ignored test.
* `angel-stage`: **150 passed**. Scoped `angel-serve pace::tests`: **7 passed**.
* Standalone `rustc --test` wrapper imports only
  `cockpit/src/drive/together_seat/keys.rs`: **2 passed**. **No cockpit suite run**.
* Original ignored `floor3_bench`: passed before/after; new ignored six-seat
  eight-cell matrix: passed before/after. No historical tests were removed.
* WASM client check/build and renderer builds for WebGPU and WebGL succeeded
  using existing dependencies, `wasm32-unknown-unknown`, profile `web-fast`.
* Actual Chromium page smoke through `legacy`, `gpu`, `webgl`, `2d`: presenters
  `webgl2`, `gpu-webgpu`, `gpu-webgl`, `canvas2d`, respectively; 59–60 fps in
  this short local single-page fixture. These are **not** six rendered seats,
  baseline/after frame-rate measurements, or pixel-parity proofs. Screenshots
  differ because the fights/timing and renderer paths differ. No audio claim.
  `shots/browser-all/page-{legacy,gpu,webgl,2d}.png` and `browser-all.log`.
* Existing JS GL-stub probes `stage-alpha.mjs` and `stage-fuzz.mjs` fail their
  valid-frame fixtures. Both reproduce **identical failures at 775388f0e**;
  shell/probe files are unchanged. Do not report them as passing. Actual browser
  rendering above succeeded. Stub fixture repair is outside this patch.
* Screenshot export: `<scratch>/net-perf/shots/floor3-heavy-six.png`, plus
  repeatable serialized six-seat fixtures for all eight cells in that directory.

Commands (set CARGO_TARGET_DIR as desired):

```sh
cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast
cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast floor3_bench -- --ignored --nocapture
cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast net_perf_six_seat_bench -- --ignored --nocapture
DELVE_NET_FIXTURES=<scratch>/net-perf/shots cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast net_perf_export -- --ignored
cargo build --manifest-path crates/Cargo.toml -p angel-delve-dev-host --bins --profile fast --target x86_64-unknown-linux-musl
# Explicitly owned listener only; use six concurrent probes SEAT=0..5:
delve-dev-host --bind 127.0.0.1 --port 18864 --scene fight --depth 6 --key owned-key
delve-net-probe ws://127.0.0.1:18864/ws owned-key SEAT 45
# Optional last arg 300 coalesces commands/ACKs to simulate a slow sender.
```

## Residual / limits

* The data supports stage-byte reduction and local real-wire continuity, not
  proof that the original West Coast packet-loss/audio/tavern complaint is fixed.
  WAN loss, mobile GPU upload cost, long-session allocation/texture churn,
  audio quality and live tavern behavior remain unqualified here.
* Snapshot pacing and compact projectile courses already existed. No interest
  culling, lossy gameplay changes, projectile-count caps, new dependency, or
  wire-version migration was introduced. Empty-traits omission is a small gain.
* Cropping can change picture sizes and increase small-texture churn; tile bounds
  scans and cached per-figure pictures cost CPU. Stage bytes are still
  147–377 KB/frame on average, so there is room for further optimization.
* Remote metrics came from the post-fix build before a final allocation-only
  change (inline RGBA expansion instead of per-row temporary Vecs). The final
  deterministic matrix has byte-identical output. Final WASM/browser smoke also
  preceded that allocation-only change; byte/layout semantics are unchanged.
  Final Rust tests and benchmark exercise that final source.
* No modifications to squires or `hireling/home/rules.rs`; no cockpit suite,
  live 8840 restarts, or probe hosts left running.

