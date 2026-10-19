# H01 iteration 10 — committed patch serialization exposed a new stop

## Hypothesis and phase

Prospectively registered author-only, local identity/capsule follow-up to H01. H01's original retry candidate and deterministic regression already exist; this iteration did not reimplement them or repeat any product tests. H02 remains separate and unregistered. Phase: exploratory identity revalidation, **blocked**, not independent audit or release qualification.

Hypothesis I: the committed production patch is literally byte-identical to frozen v3, and the retained source/binary/helper/resources can be bound without unexplained differences. Hypothesis C: the capsule contains the native index snapshot its description claims. The latter was **not reached**.

Current candidate HEAD: `b7bf34a8a899f9bf20eb241eff45bfc63ae14bb1`; frozen common base: `7d711a031cc3fbdda9112c1be29a104289a06524`. Changed factor: analyzer scope only; the historical report-hash mismatch was explicitly left OPEN so a separate non-promoting identity check could proceed. No product factor, safety control, build or fixture changed. Exact delta: `artifacts/harness-research/20261015-local/H01/iteration10-binding-coverage/analyzer.delta.patch`.

## Preregistration, budget and exact invocation

Scope, expected cases, controls, literal-match thresholds, analysis and stop rules: `artifacts/harness-research/20261015-local/H01/iteration10-binding-coverage/scope.md`. One author analysis invocation, alarm60s, at most16 direct read-only child commands; no retry. Those limits do not describe transitive helper process counts. Both YOLO flags overridden to0, pycache disabled. No credential values recorded. No live/provider, remote, submission or external-publication authorization was used or inferred.

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 /usr/bin/python3 -B artifacts/harness-research/20261015-local/H01/iteration10-binding-coverage/run-once.py
```

Preregistration SHA256: `7e9fa145d1ca801aef1ac2af566ab3046ee75e48ab5c6d7703e7739f5f719b75`. Analyzer SHA256: `368d9a84c3b7a7dc0daaf73a9897844ae0c48ddad5500ddb42bdd2a7a6a32c75`. Invocation receipt: `invocation-receipt.json`; exact subprocess commands and hashes: `commands.json`; all stdout/stderr and traceback retained.

## Measured evidence and new finding

- Analyzer **exit 1**, **0.064052s** wrapper wall; **14/15** assertions satisfied. These are analysis assertions, **not product-test results**.
- **5** read-only direct child commands ran: HEAD, status, tracked diff, native source digest, committed production diff. All exited0. The next assertion failed: `production exact frozen patch matches committed diff`. **No retry or analyzer edit after registration.**
- Native current digest scheme `angel-source-v3`: `17796a36ed95e3ca0499087bcfb5a9346ffa321cb89d8ded3d118c64d4617a31`. HEAD matched and tracked diff was empty. This is a source digest observation, **not binary/helper/resource qualification**.
- Frozen production patch SHA256: `667f687d2dbe347bbfedbf3f70ff507edac2ae63cb3eac50cbb87e031e6e69b0`. Saved committed-diff SHA256: `c2913bbfa3060037b8c45842ce58c344563028675e816a9bb6c4e163f18c1776`. Literal equality: **False**.
- Post-stop inspection of those **already saved bytes** found **9 path-header lines** differing: frozen `i/` / `w/` versus committed diff `a/` / `b/`. Transforming only those explicitly enumerated header prefixes makes the saved copy equal the frozen patch: **True**. Diagnostic: `patch-byte-diagnostic.json` and `patch-header-diagnostic.diff`. No original was modified. This is **not a second verifier, a pass of the failed endpoint, or proof of the Git configuration that caused the serialization**.
- Consequently the byte-exact endpoint is confounded by patch presentation. The recorded failure alone does not establish production source-content drift. Conversely header equivalence alone does not prove full frozen source/blob, fixture, executable or resource identity. A future distinct preregistration would need explicit prefix controls plus per-path blob/content identities, retaining this negative result.

## Integrity, preservation and limits

At analyzer start **490** indexed references were checked: **489 matched**, only historical entry425 differed. Expected report SHA256 `a6443c81c3b8a0143f01e84609c0824cb417da626faaaca8e635402f0c6cdf5a`; observed `3a29573ba70d57bf453f197c345a23a852c15cc5923c6480496a3eaa1f782cb9`. This is **unresolved**, not repaired or waived into global integrity. Earlier failed recovery directions were not repeated.

Preservation closeout: **522/523** pre-closeout indexed references matched; only that known mismatch remained. Original index prefix preserved: **True**; HEAD unchanged: **True**; tracked diff unchanged: **True**. Exact evidence: `closeout-integrity.json`, `closeout-commands.json`. Prior reports, retained patches, product source/regressions and unrelated work were untouched. Index events are appended, never rewritten.

**Not completed after the stop:** fixture binding, frozen tree inventories, release/test binary and helper checks, resource inventories, load snapshot, raw replay re-extraction, fixture control validation, capsule member/recursive-index coverage. Do not infer any of these from the current source digest or from previous reports. Capsule coverage remains unmeasured, not falsified by this invocation. Preparation and post-stop path-discovery tool errors are retained as research diagnostics, not product failures.

New builds0; new product-test executions0; sockets/physical sends/provider calls initiated0; no model delegate, remote jobs, submissions, pushes or external publication. **New per-case send counts, recovery and cancellation are null/unmeasured**, not zero outcomes. Original regression/replay evidence remains historical, not independently reproduced here. Ordinary task effective controls were not measured. No task solves, speed/cost, holdout, matched-peer or leadership claims.

## Decision and next action

**INCONCLUSIVE / BLOCKED — no promotion.** Reject the literal patch-serialization equality assumption as an adequate source-content identity gate; retain the failed attempt. Global lineage remains unqualified. Do not retry this analyzer, restore fabricated report bytes, rewrite the capsule, or register H02.

Next necessary action is a **new, separately registered** local source-content binding check with fixed Git prefixes and direct blob/content identities, keeping the historical mismatch open, and then only the unreached binary/helper/resource/capsule checks if their prerequisites qualify. This is not authorized to proceed implicitly in this iteration. Independent audit and any live qualification still require their own controls and finite recorded authorization. No release or research-goal completion is claimed.
