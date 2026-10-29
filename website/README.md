# angelX public website

This static site restores the existing black-and-white, dot-matrix marketing page. Vercel project `angelx` uses `website` as its root directory and has no framework or build command. No runtime dependencies are required.

## Run locally

```sh
cd website
python3 -m http.server 8080
```

Open `http://localhost:8080`. The page starts at the bottom, with the sword entrance, and reads upward. The source order is therefore summit, preview, gallery, product description, entrance. `js/main.js` moves a new visit to the bottom; do not reorder sections without updating the ascent logic.

## Benchmarks

`benchmarks.html` opens with the angelX 0.1.9 harness comparison: a static table and `assets/bench/angelx-0.1.9-harness-comparison.png`, Jordan's chart from the tournament benchmark datasets. The table copies the chart's own table; keep the two in step. The machine-readable source for those runs is not yet in the public repository.

Below it, the page shows the *published* full 136-task cohorts, dated September 23 and 30, 2026. The exact values and model settings come from [`README.md` at the 0.2.3 public release commit](https://github.com/newjordan/angelX/blob/91456dc89efe8fa606a475b19fec1f0c0ffe26dd/README.md#benchmarks). The [full-cohort audit](https://github.com/newjordan/angelX/blob/91456dc89efe8fa606a475b19fec1f0c0ffe26dd/docs/telemetry/polyglot-full136-slowdown-20260926.md) explains timing and comparison limits. The interactive charts deliberately retain the [September 21 source cells](https://github.com/newjordan/angelX/blob/91456dc89efe8fa606a475b19fec1f0c0ffe26dd/docs/telemetry/polyglot-benchmark-20260921.json) in `js/bench-data.js` and are labeled historical. No full 136-task 0.2.3 result is published. The public reports do not identify benchmark host CPU, GPU, or memory.

Update numerical copy only alongside a dated evaluator receipt and source link. The dataset should retain failures, model settings, versions, and timing definition; don't combine partial cells into full-cohort totals.

## Knight Shift

The small star beside the creator credit opens a native, keyboard-accessible video dialog. Playback is user-initiated. `assets/knight-shift.mp4` is a 720p H.264/AAC web encode of `Knight_Shift_Final_v1.mp4` (226.36 seconds); the original master is preserved outside this repository. The master SHA-256 is `6054ebfa4471731162f0bf51b848986507af687f4cf82d7d437af50a6ae42b17`; the web encode SHA-256 is `e4a966f52ed6984ce390c4f05938f80e897b8e3bbb44a776e1d49823fa0b6431`.

## Deploy

The existing Vercel project is `frosty40s-projects/angelx`, with root directory `website`. Deploy from the repository root through that project or let its connected Git deployment run after the public source is updated. Check `/`, `/benchmarks.html`, and the Knight Shift asset on the resulting URL.
