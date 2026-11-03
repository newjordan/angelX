# Blaze upstream opportunity: do not answer mutable file-input questions as immutable

Date: 2026-10-28. Iteration 27. Status: **reported, unapplied candidate**.
Upstream pin: `b383b0bcf5da33816809b00a37e1c36a0e761efb`.

## Why this matters for rapid development

An instant successful compiler answer is not useful if it hides a deleted or invalid source path. The installed Blaze wrapper can answer `rustc --print=cfg src/probe.rs` successfully after the file disappears or becomes a directory, while direct rustc fails. Cargo-shaped queries reproduce the problem. This is a new file-input dependency, distinct from iteration 26's custom **target** file validation.

The proposed upstream change delegates named-file queries to rustc rather than trying to encode source path, cwd, existence, readability, and diagnostic behavior into a persistent answer key. It leaves ordinary no-file and stdin fast answers in place. No startup/rebuild latency improvement is claimed; the file-input cases intentionally incur rustc execution for correctness. The overhead and distribution of those queries must be measured before adoption.

## Reproduction and recorded evidence

See `reports/performance/blaze-file-input-iteration27/`:

- `calibration.json`: 30 installed-wrapper/reference observations; answers enabled match 11/15, answers disabled match 15/15. Four enabled observations falsely succeed. Four separate ordinary fast-query controls match.
- `edge-calibration.json`: 24 follow-up installed-wrapper/reference observations. Enabled mode falsely succeeds after deletion for a version-looking option value, a source following `--`, and a hyphen-leading source following `--`. Disabled mode matches all 12 observations. `-V source.rs` succeeds with or without the source in rustc too; it is a conservative refusal case, not a corrected false success.
- `r2/differential.json`: exact reduced upstream query/key/parser modules against real rustc. Final candidate 58/58 exact `(exit status, stdout, stderr)` triples; unchanged baseline 35/58. The remaining 23 baseline observations all falsely succeed. Cases include missing files, directories, repaired files, two cwd contexts, option-value boundaries, extensionless paths, primed persistent caches, and no-file/stdin controls. These are observations of a selected matrix, not 23 independent defect classes.
- `r2/candidate-tests.log`: 12/12 reduced-module tests pass. `r2/test-only-tests.log` has two assertion failures against unchanged logic, not compilation failures. These fail first on ordinary-file assertions; edge-specific causal sensitivity is not independently isolated by those assertions.
- Candidate-only controls create a target-list cache entry, read an explicit sentinel from it, restore and read the real answer. A fresh candidate-only file query with own answers refused leaves no persistent entry. Shared-home snapshots alone are not a no-read proof.
- `integrity.json`: 133/133 same-author source/receipt consistency assertions held, including exact reconstruction of tested sources by applying the patch. Not independent acceptance.
- `review-r1.md` and `r2/review.md`: two separate-context advisory read-only reviews. The first caught two bypasses; the second found no blocker in the selected final scope.

## Minimal upstream design

Final patch: `reports/performance/blaze-file-input-iteration27/r2/candidate-upstream.patch`.
SHA-256: `f58c24f28c85d3e0c9609924f8bc9a11828468fc339cfc2172c755e294104d3f`.

1. Retain `is_question` wrapper dispatch behavior using the existing scanner in permissive mode.
2. Invoke that scanner in conservative mode before constructing a persistent answer key. Positional file inputs and `--` refuse a key. The any-token version shortcut only permits an unambiguous standalone version invocation for caching; mixed forms fall through to real rustc.
3. Refuse positional named-file inputs in `Given::of`, so own answers cannot bypass the compiler's file diagnostics before the key guard runs.
4. Keep `-` stdin and recognized valued-option operands distinct from named-file inputs. Do not rely on `.rs` suffixes or stat a file then claim an immutable answer.
5. Refusing the key happens before looking up or locking a persistent entry, so old entries for the selected mutable-source queries are not reused by the candidate. No cache directory deletion/migration is needed for those queries.

The patch changes only `crates/ra-jit/src/answers.rs` and `answers/own.rs`, including tests. It does **not** include the separate candidates from iterations 24–26 and is not an omnibus correctness fix.

## Necessary upstream acceptance gates

- Test on the real complete engine/wrapper, not just this exact-module reduction. Exercise wrapper-only questions, Cargo, compile-time and rust-analyzer consumers; verify stdout, stderr, exit status and stdin behavior.
- Include valued flags, end-of-options, real mixed version queries, multiple inputs, non-UTF8 paths, relative/absolute paths and response-file delegation. File permission behavior needs an unprivileged test; missing/directory tests do not establish unreadable-file handling.
- Re-run the existing ordinary question grid and qualified fixes from iterations 24–26 together. Selected source-file parity does not establish arbitrary flag validity, custom target/cache correctness or complete wrapper dispatch correctness.
- Measure source-query frequency and cold/warm startup/rebuild cost on representative Cargo workspaces. Ensure preserving no-file/stdin eligibility actually preserves their performance in the full engine; no timing threshold is met by this work.
- Real cache atomic publication, lock lifecycle and concurrent behavior remain outside the reduction's substituted store/discovery behavior.
- No upstream PR was sent; no host engine was rebuilt or installed; `auto` remains unchanged. Host promotion is separately blocked by the pre-existing `loop_ctl.rs` protected-file seal mismatch. Only operator approval can resolve that gate.

## Experimental limits

The original <=200 explicit query-process cap was exceeded during review-driven calibration. The prospective r2 gate records a bounded extension, not retroactive original-budget compliance: approximately 279 explicit calibration/r1 query processes (including invalid CLI attempts), then 180 r2 queries; six reduced Cargo commands total. Full details and failures are in the iteration report. No provider calls, live jobs, submission or backend promotion.
