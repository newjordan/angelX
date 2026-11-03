# Browser asset metadata boundary — iteration 11

2026-10-28. Local Rust 1.95.0 / fast-profile measurements. **A metadata and
artifact-size improvement, not an established startup or edit/reload speedup.**

## What changed

`cockpit/web-bundle/build.rs` used to emit a private `static BUNDLE` containing
`include_str!`/`include_bytes!` references. A non-inlined public getter returned
that static. On the pinned compiler, the reachable static still caused the
asset payload to be serialized into cross-crate metadata.

The generator now emits `#[inline(never)] pub fn embedded()` directly, with the
same return type and expression but function-local promoted immutable constants.
The explicit skip/missing-input branch emits the identical signature returning
`None`. `PACKED`, `SOUNDS` and `BESIDE` retain their existing private tables and
non-inlined accessors. No runtime asset loader, new dependency, unsafe code,
source-freshness shortcut, compiler flag or feature change was introduced.

The host continues to rebuild/relink when Cargo says the dependency changed.
This change is **not** an ABI-stability promise and does not skip host compilation.
Recheck metadata under future compiler/profile/inlining changes.

## Results

Controlled B/C/C/B comparison, two CSS edit/restoration pairs per phase:

| Measurement | Baseline | Candidate |
|---|---:|---:|
| Library `.rmeta` | 14,851,188 B | 25,265 B |
| Library `.rlib` | 29,776,186 B | 14,949,970 B |
| CSS edit/restoration source launch, median (8 each) | 11.3969 s | 11.6260 s |
| Unchanged source launch, median (8 each) | 227.16 ms | 238.08 ms |

Metadata was **99.83% smaller**, the library archive **49.79% smaller**. The
candidate did **not** establish an end-to-end latency benefit: edit/restoration
median was 0.229 s higher and unchanged median 10.92 ms higher. Do not describe
this experiment as a faster-startup result. Keep the small boundary correction
for its measured storage/metadata reduction; the bottleneck must be elsewhere.

There were 37 successful source launches: 32 warm observations, four variant
primes and one final restoration. Variant-switch costs were recorded but not
pooled with warm timings. The baseline included a 19.04 s edit outlier. No
confidence interval or cold-cache/clean-build claim is made. These are local
wall times, not controlled CPU-isolated observations.

Final library comments were narrowed to this compiler/profile and the existing
page-contract test was strengthened after independent review. Final restored
inventory is **24,820 B rmeta / 14,949,522 B rlib**; that small additional change
is not a newly timed treatment. The generated asset hashes still match the
controlled experiment's final restoration.

## Correctness and evidence boundaries

* The existing `angel-web-bundle` test entry point passed 12 tests initially.
  After review, the existing page-contract test also compares the compiled
  `embedded()` page, decoded CSS, boot script and WASM to current generated
  output. All 12 tests passed in each normal, real-CSS-edit and restored run.
  This checks bytes in a linked **test binary**, not a live browser or host
  serving endpoint. It does not prove whole-application behavior.
* All decoded generated assets/index hashes were recorded throughout the
  comparison. Only CSS changed in edit steps; restoration matched every hash.
* Existing Node boundary/policy checks: 3 passed. An earlier run was deliberately
  taken while the comparison had installed the baseline and failed the new
  candidate-only structural assertion; the final candidate passed.
* Source-launch PTY fixture: three normal-launch samples, median visible draft
  **252.84 ms**, median input echo **0.676 ms**. Priming was **10.00 s**, separately
  recorded/excluded. No baseline PTY comparison was run in this iteration.
  One malformed `--output` invocation failed before measurement; corrected
  positional invocation succeeded.
* A fresh semantic host-root edit/restoration executed the expected versions,
  then removed the marker. Prime 46.97 s, no-touch 219.8–224.5 ms, semantic edit
  10.95 s, restore 12.19 s. A diagnostic wrapper added `-Ztime-passes` only to
  host rustc; those times are attribution evidence, not ordinary-build A/B data.
  One earlier command used invalid `--case root --output` and did not run;
  corrected `--case version` with positional output succeeded.
* Normal build logs say no pinned `wasm-bindgen` and omit the optional GPU
  renderer. No feature/assets were stripped by this patch, and all current
  generated asset hashes agree. This is **not** qualification of a configuration
  containing unavailable renderer tooling, nor browser FPS work.
* The measured driver selected the newest generated output and required exactly
  one library metadata artifact. Independent review correctly flagged that this
  is not a general artifact association. The refined driver now binds the
  generated source through the selected rmeta's rustc dep-info and fails on
  ambiguity/escaped paths. It was exercised on final restored output; the
  original measurement driver is frozen under its recorded SHA-256. Do not
  retroactively upgrade the original measurement's association guarantee.

## Fresh host attribution: do not chase only the linker

For the diagnostic semantic edit (10.704 s total rustc) / restoration (11.933 s):

| Phase | Edit | Restore |
|---|---:|---:|
| Macro expansion | 1.309 s | 1.374 s |
| Name resolution | 0.760 s | 0.744 s |
| Codegen crate | 2.052 s | 2.123 s |
| Incremental dep-graph serialization | 1.423 s | 1.390 s |
| Link binary | 1.117 s | 2.246 s |

These phases contain nested work; do not sum overlapping phase lines.
Type checking and borrow checking were about 0.29–0.36 s each. LLVM passes were
only 0.076–0.078 s. Peak RSS in these reported phases approached 3 GB. Removing
embedded metadata did not remove the host's parsing/expansion/monomorphization
and incremental-state costs. A next experiment should reduce a real host
compilation boundary with public non-generic interfaces, or measure incremental
state persistence separately—not assume that further asset compression or more
codegen units will fix this 10–12-second floor.

## Reproduction / retained evidence

```sh
python3 scripts/perf/measure-web-metadata.py NEW.json \
  --baseline-prefix reports/performance/web-metadata-boundary-20261028 --pairs 2
# Optional --candidate-first reverses the order; NEW.json must not exist.

TC="$HOME/.rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/bin"
RUSTC="$TC/rustc" RUSTDOC="$TC/rustdoc" "$TC/cargo" test \
  --manifest-path cockpit/Cargo.toml --offline --locked --profile fast \
  -p angel-web-bundle

python3 scripts/perf/measure-dev-rebuild.py NEW-PHASES.json \
  --backend cargo --compiler-phases --case version
python3 scripts/perf/measure-terminal-ready.py NEW-PTY.json --launcher
```

Do not edit/build this checkout concurrently with the source-mutating probes.
They own only the files they selected, check content before overwriting, terminate
timed-out children and restore their candidate/CSS in `finally`.

Explicit receipts under `reports/performance/`:

* `web-metadata-controlled-iteration11.json` and named source/log siblings;
  `web-metadata-controlled-iteration11-measured-probe.py` preserves the exact
  measured driver (before artifact-association refinement).
* `web-metadata-boundary-20261028-baseline-{build,lib}.rs` preserve the earlier
  iteration10 sources; `web-metadata-boundary-20261028-tests.log` is the initial
  crate verifier.
* `web-metadata-linked-freshness-iteration11.json` and its three test logs.
* `web-metadata-final-inventory-iteration11.json` and
  `web-metadata-final-launch-iteration11.log` bind/check restored source/assets.
* `web-metadata-host-phases-iteration11.json`, wrapper and named logs.
* `web-metadata-pty-iteration11.json` and
  `web-metadata-final-policy-iteration11.log`.

See [the Blaze embedded-dependency regression proposal](blaze-embedded-dependency-20261028.md)
for a distinct, executed upstream test opportunity. No upstream publication or
installed compiler modification occurred.
