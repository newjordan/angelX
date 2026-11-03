# Reduce repeated freshness traversal, not freshness

2026-10-28 · focused startup iteration · dirty tree based on acb6562b0.

## Hypothesis and implemented change

Warm source launch still spends most of its time in Cargo's real freshness check.
A diagnostic Bash DEBUG hook recorded line numbers/times only, never expanded
commands or environment values. In that instrumented run, Cargo consumed 230 ms
of a 342 ms launch; launch-image work consumed another 68 ms. Instrumentation and
filesystem cache state affect those observations, so they are diagnostic, not a
before/after performance result. Raw receipt:
`reports/performance/launcher-phase-baseline-20261028.json`.

The browser build-script output had **2,860** `rerun-if-changed` entries, including
whole directories alongside their children. Cargo recursively inspects watched
directories. Additional canonical child paths add no freshness coverage but can
add repeated traversal. `cockpit/build_fast.rs::minimal_watch_paths` now removes
those children and exact duplicates after collecting inputs. The browser-package
builder emits the reduced set **after** its work returns, including early-return
paths. It retains external dependencies and aliases; missing, noncanonical,
symlink and `..` paths are not covered away. Source/dependency parsing and the
fail-closed nested-Cargo diagnostic are unchanged.

The generated input list now has **714 entries**: 2,146 fewer, **75.0% removed**.
The full generated inventory is retained locally under
`reports/performance/watch-minimization-candidate-inventory-20261028.txt`.

## Measurements and caveats

All application builds kept the browser bundle, media, Glass and source
freshness. No no-build/skip-browser comparison, background freshness shortcut,
provider calls, sandbox weakening or global toolchain/config change.

| Observation | Result | Interpretation |
|---|---:|---|
| Full-feature Cargo build, no source touch, baseline median after prime (6 samples) | 195.50 ms | `cargo-watch-baseline-20261028.json` |
| Same command, candidate median after prime (6 samples) | 178.72 ms | 8.6% less in this local before/after sample |
| Normal source launcher `--version`, 6 warm samples | 232.6–239.1 ms; median 236.0 ms | Cargo freshness included; not UI ready |
| Existing semantic-edit probe warm launches | 240.6 / 241.7 / 242.4 ms | independent later observations |
| Actual Rust edit → execute | 20.23 s | edited marker observed in executable JSON |
| Actual Rust restore → execute | 16.98 s | marker absent; source restored |
| Warm 140×40 offline-practice PTY via source launcher, draft visible | 263.2 / 267.1 / 287.1 ms | constrained fixture, not production-provider startup |
| Same PTY, input echo after draft | 0.61–0.68 ms | responsiveness, not total launch |

The older retained launcher sample was 286.6 / 288.9 / 299.3 ms, but was not
interleaved with these observations. Cache state, host load and launcher-image
copy/hardlink cost vary. Do not attribute that entire historical difference to
this change. The direct Cargo comparison isolates a narrower improvement, but
is still sequential, not a randomized paired benchmark or a universal bound.

**No Rust compile-speed gain is established this iteration.** The real edit and
restore remain about 17–20 seconds, slower than some earlier retained samples.
A freshness optimization must not be presented as a compiler optimization.

Initial candidate application refresh took 18.44 s. The semantic probe's prime
was 10.87 s, and the PTY fixture needed its own HOME/cartridge configuration
prime before warm samples. Restoring normal HOME afterwards took 10.58 s. Those
one-time rebuilds remain recorded and are not warm launches or cold-boot tests.

## Distinct verification gates and evidence

1. **Existing `build_fast` Cargo test target:** 17 passed, 0 failed, including
   shared build policy/job-budget checks and new minimizer coverage for nested
   children, missing files, symlink/`..` aliases, duplicates and empty input.
   Receipt: `watch-minimization-build-fast-20261028.json` and adjacent log.
2. **Actual browser/WASM invalidation:** the existing
   `scripts/perf/measure-web-bundle-invalidation.py` proved cartridge environment
   does not rebuild the browser; a boss-asset edit changed the packed WASM and
   restore recovered its original SHA-256. No generated-cache injection.
   Receipt: `watch-minimization-browser-invalidation-20261028.json`.
3. **Directory-only watch coverage:** new repeatable
   `scripts/perf/measure-web-watch-freshness.py` established `shell.css` no longer
   has an explicit watch but its shell directory does. Editing its comment,
   adding a temporary CSS member, removing it and restoring the original all
   updated the actual packed page and generated timestamp. Source and temporary
   member were restored/removed. Receipt:
   `watch-minimization-covered-inputs-20261028.json`.
4. **Executable freshness:** existing `measure-dev-rebuild.py` asserted edited
   and restored runtime JSON, not just a successful build exit.
   `watch-minimization-devloop-20261028.json` records both assertions true.
5. **PTY ready/input observations:** existing `measure-terminal-ready.py
   --launcher`; `watch-minimization-terminal-launcher-20261028.json`.
   Normal HOME was restored with an accounted build afterwards.
6. **Separate read-only reviewer:** no blocker in the scoped change;
   `watch-minimization-review-20261028.txt`. Not an independent runtime evaluator.

Receipts above are under `reports/performance/`; commands, exit statuses,
logs/hashes and exact scopes are retained. The application edits predate this
iteration as well; only the watch minimizer and browser input collection were
changed here. No unrelated dirty work was reverted.

## Blaze contribution and remaining work

A newly measured, distinct upstream PR opportunity is described in
[Blaze overlapping watch traversal](blaze-watch-overlap-20261028.md). The exact
pinned walker expands overlapping paths before a later deduplication step.
A two-line visited-set reduction preserves all distinct path spellings while
reducing repeated traversal; selected fixture timing improved 51.6%. The patch
and complete reduction harness are saved separately; upstream installation and
default backend are untouched. This is not an end-to-end Blaze acceleration
result, and existing publication/sandbox blockers remain.

Next useful work: qualify the upstream candidate with the existing pinned
ra-jit test target and nonoverlap cases, and profile the remaining Rust
incremental frontend/link split for real application edits. Avoid multiplying
new build caches: existing `target/fast` and `target/blaze` occupy tens of GiB;
this host has only about 6–7 GiB free. No operator caches were deleted.

No bound loop acceptance command exists, so this is measured incremental
progress, not a declaration of the fastest possible setup or loop completion.
