# Browser packing: verified byte reuse (2026-10-28)

## Candidate and invariant

`cockpit/web-bundle/pack_asset.rs` avoids recompressing an unchanged large
browser asset when its previous zlib slot decodes **exactly** to the current
plain bytes. There is no timestamp/digest sidecar freshness authority. The
nested Cargo build and full input watches still run first. Missing or reordered slots, decode failures and differing decoded bytes
regenerate and verify the new encoding. A valid stream with trailing junk can
be reused if it decodes exactly; canonical zlib encoding is not required.
Stored reads and decoded output are bounded by the current plain length.
Non-regular fallback slots fail rather than write through a symlink or FIFO.
The build's trusted Cargo-owned OUT_DIR is assumed; hostile concurrent path
replacement inside that directory is not a supported security boundary.
Small/unhelpfully compressed files retain the existing plain representation.
No runtime protocol, asset contents, renderer policy or feature set changes.

## Controlled package measurement

Command:

```sh
python3 scripts/perf/measure-web-pack-reuse.py reports/performance/web-pack-controlled-iteration10.json
```

Candidate/baseline/baseline/candidate, three CSS edit/restore pairs per phase,
real locked/offline `cargo build --profile fast -p angel-web-bundle`. Priming
and source transitions excluded from warm samples; 12 rebuilds per variant.
Every sample decoded and hashed all `include_bytes!` assets (not the separately
embedded index page), required current CSS and
unchanged other assets, and checked exact restoration. Frozen build-script and
helper variants are retained beside the receipt. Baseline changes only the
packing loop back, not watch policy, profiles or nested Cargo commands.

| Warm browser package rebuild | Baseline | Candidate |
|---|---:|---:|
| Median | 1,634.70 ms | 770.21 ms |
| Range | 1,596.97–1,757.80 ms | 727.30–1,021.86 ms |

Reduction: **864.49 ms / 52.9%**, a package-stage result, not a full-launch win.
The later non-regular-slot guard is additionally covered by the full-launch
experiments and final tests. The original measured helper is frozen by hash.

## Full launch and readiness qualification

`web-pack-launch-edits-iteration10.json` includes the final guard and real
`bin/angelX --version` after CSS edits, including host rebuild/link. Eight edit
or restoration samples per variant. Baseline median **12.039 s**, candidate
**13.259 s**. The final candidate phase was markedly slower than the first;
this experiment does **not** establish an end-to-end speedup. Keep that result,
not just the favorable package-only result. The reverse-order follow-up
(`web-pack-launch-reverse-iteration10.json`, 8 each) measured baseline
**12.689 s**, candidate **10.787 s**. Pooled medians (16 each) were 12.363 vs
11.513 s, an exploratory 0.850 s reduction. Opposite run-level results and
concurrent external compiler activity prevent a stable full-launch claim.
The host rebuild/link dominates this path.

Unchanged launches in the package experiment were 229.72 vs 222.16 ms (8 each),
but the full-launch edit experiment's no-touch controls were 231.04 vs
233.35 ms (8 each). **No new startup gain is claimed.** A normal-launcher PTY
fixture, three samples, produced a median visible draft of 239.96 ms and
0.593 ms input echo; fixture priming cost 10.20 s. This is practice-route,
terminal-only with background discovery disabled, not cold setup or Glass.

## Correctness evidence and limits

* Existing web-bundle Cargo test entry point: **12 passed**, no failures.
* `web-pack-candidate-iteration10.json`: real CSS edit/add/remove/restore.
* `web-pack-wasm-iteration10.json`: embedded map edit changed the packed WASM;
  restoration recovered the exact original decoded hash. Real WASM rebuilds
  took 22.56/23.85 s; this is freshness evidence, not a WASM compile speed claim.
* All successful probes restored temporary edits. Two initial PTY invocations
  used unsupported options and exited 2; the corrected invocation succeeded.
* Matching wasm-bindgen was absent here. Renderer-enabled, other platforms,
  clean builds, portable/remap transitions, corrupted full bundles and cold
  caches are not separately qualified. Existing runtime decoder limits remain.
* Review flagged pre-existing duplicate sound-name, optional-input creation and
  renderer-tool tracking leads; those remain unverified, not silently repaired.

Blaze opportunity and negative controls: [restore replay](blaze-restore-replay-20261028.md).
