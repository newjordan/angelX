# Blaze PR opportunity: generated embedded-asset dependency regression

2026-10-28 · measured local fixture, not a new stale-output bug report.

## Why this is distinct

angelX's browser bundle is a generated `include!` module in an independent Cargo
path dependency. The generator writes both Rust source and embedded data into
`OUT_DIR`; source Rust in the host remains untouched when CSS changes. A large
private static can also inflate downstream metadata, while a non-inlined getter
with local promoted constants avoids most of that overhead on the pinned
compiler. This is a useful embedded-assets/code-generation boundary for Blaze
that the earlier Rust-body and C-static-library probes did not cover.

## Executed fixture

The existing `scripts/perf/repro-blaze-image-freshness.py` now accepts the mutually
exclusive `--embedded-dependency static|promoted` mode. Both cases use:

* an independent generated path dependency, no third-party crates/network;
* 1 MiB `asset.bin`, copied by a real build script into `OUT_DIR`;
* `include!(concat!(env!("OUT_DIR"), "/bundle.rs"))` with generated
  `include_bytes!` data, exposed by a non-inlined getter;
* prime, unchanged, data-only edit, restoration; each successful command's
  artifact is executed immediately and its output/binary identity recorded;
* compiler/install SHA-256 validation, bounded subprocess timeouts, private
  temporary fixture, isolated Cargo target and daemon socket/cache paths, and
  a final `ra-jit cargo stop` in cleanup.

No sandbox controls were disabled. The installed compiler and source were not
patched. Fixture directories are deleted by the probe, but explicit JSON/log
receipts and the exact measured probe source remain in the repository.

Pinned installed revision: `b383b0bcf5da33816809b00a37e1c36a0e761efb`, toolchain
`nightly-2026-10-27`. These receipts, not a historical report's revision, identify
this experiment's compiler. The wrapper is `ra-jit-rustc` via Cargo's workspace
wrapper setting; this is not a direct full-application Blaze launch.

| Fixture | Prime | Unchanged | Data edit | Restore |
|---|---:|---:|---:|---:|
| Private static + getter | 125.67 ms | 19.64 ms | 2222.05 ms | 281.70 ms |
| Function-local promotion | 322.51 ms | 17.72 ms | 2625.56 ms | 279.77 ms |

**All eight executions returned expected output**, both daemons stopped with
exit 0. Each baseline/restoration printed `1048576-97-101711872`; edits printed
`1048576-98-102760448`. This length/first-byte/sum signature detects these
specific uniform-byte edits; it is not an exact byte digest. There is only one
run per form, no paired steady-state latency claim, and no claim that promotion
is faster in Blaze. Metadata arrays are empty in these receipts because this
wrapper configuration did not expose matching `.rmeta` paths; these results do
not measure Blaze metadata serialization.

```sh
python3 scripts/perf/repro-blaze-image-freshness.py NEW-PROMOTED.json \
  --embedded-dependency promoted
python3 scripts/perf/repro-blaze-image-freshness.py NEW-STATIC.json \
  --embedded-dependency static
```

Explicit retained sources/receipts:

* `reports/performance/blaze-embedded-{static,promoted}-iteration11.json`
  and the four named step logs per form;
* `reports/performance/blaze-embedded-iteration11-measured-probe.py`
  (SHA-256 `c9cc111cb680f0da5ec1d5dd1b0feb29605c60e50731d9e9e22b33a73ca84c99`).

## Concrete upstream PR scope and acceptance

Port this generated-data path-dependency case to the installed revision's
`harness/cargo_ra_jit.py` / `harness/artifactedits.py`. Existing `artifactedits`
steps edit Rust source; this proposal changes only an embedded input behind a
build-script dependency, and compares both supported storage forms.

Before calling the upstream regression complete:

1. Use a nonuniform byte pattern and an exact execution-time byte comparison,
   including same-length edits at the start, middle and end. Retain binary
   hashes separately from semantic equivalence; restored binaries need not be
   bit-identical.
2. Cover generated source unchanged while data changes, and data unchanged while
   generation reruns. Preserve build-script env/input/output freshness, rather
   than deciding from Rust token/API equality alone.
3. Test deletion/missing input, deliberately failing generation/compilation,
   process interruption and daemon restart. Failure must not report a stale
   linked artifact as success. Do not weaken path confinement to pass a fixture.
4. Run no-touch controls and bounded repeated Cargo/Blaze comparisons. Record
   script/analysis/link/publication phases separately; 2.2–2.6-second tiny edits
   here are an investigative lead, not proof of a performance defect.
5. Add a browser-scale generated blob case if claiming large-asset support; the
   1 MiB fixture does not establish scale behavior.

The positive fixture narrows the existing full-host freshness failure: generated
embedded data alone did not reproduce it here. It does **not** clear full-host
edit/restore, nested browser/V8 or parent-lock sandbox compatibility gates.
Normal angelX launch stays on qualified Cargo; Blaze remains an explicit,
project-local research backend. No PR was submitted and no remote provider/API
job was run.
