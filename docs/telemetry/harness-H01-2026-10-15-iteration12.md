# H01 iteration12 — helper identity mismatch isolated; access-state hypothesis refuted

## Hypothesis and phase

Author-only, preregistered offline evidence diagnostic under native loop `1792029422798-3790707`, program `20261015-local`, H01, worker iteration12. The optional settlement offer uses iteration13, a distinct identifier. This block changes the analysis endpoint only, not the product candidate, retry policy, original tests, expected identities or historical receipts. The existing H01 registration/index/regression and minimal candidate were retained rather than recreated. H02 remains separate and unregistered.

Expected candidate HEAD `b7bf34a8a899f9bf20eb241eff45bfc63ae14bb1` was verified; frozen common base is `7d711a031cc3fbdda9112c1be29a104289a06524`. Initial tracked diff was empty. The protocol/preflight, original H01 registration, last report and the failed assertion, frozen A/B metadata, native inventory/helper code and current retry gates were inspected. Labyrinth technical frontier was inspected; its plan returned no dedicated subdoors/attempts. Prerequisites and exact endpoints were charted in `scope.md`.

**H12-I:** both retained test-image content hashes, source-inventory and build-receipt hashes, and the current sandbox helper hash still match the recorded identities; the old compound failure is attributable to execute-access state. Alternative outcomes and a stop on content/hash mismatch were registered before new endpoint measurement. This is not an independent product replay and does not retroactively reinterpret iteration11's failure.

## Registration, controls and finite budget

Evidence: `artifacts/harness-research/20261015-local/H01/iteration12-image-conjuncts/`.

Eight prospective references appended to the existing native index at **2026-10-15T06:51:48.977720+00:00**, before invocation:

- `scope.md`: SHA256 `082a5cc14576c2c15465ab155cf5245fa7fb200617ed0e4d602f44f1e626588a`.
- `expected-identities.json`: SHA256 `a1e1f9994736e3acab2419f19eae413134a744b78046d66f4e4466be2eb3f8d3`.
- `analyze.py`: SHA256 `f07db3cf464f9da88760b9b678a2065b1ee63d95951b8d59fe1c64dddf4145b4`.
- `run-once.py`: SHA256 `922d428e69b85ee72c85fd2e003c89d0ee93eee20ce5f803ab0233fe7e0a51f1`.

Controls: existing metadata supplies exact expectations, not values inferred after inspection. Evaluate all five original predicates separately for **both arms**, retaining all ten results before an identity stop. No chmod, copying/replacing binaries, build, direct test-image execution or product change. Imported native runner inventory definitions and existing source helper were the intended conditional follow-ons, not a new orchestration layer. Author logical review is retained and is **not independent audit**.

Budget: exactly one analyzer invocation, no retries; analyzer alarm60s, parent65s; <=12 direct read-only child commands, <=10s each. Separate <=2 preservation-only closeouts, <=30s each, cannot rerun image endpoints. Compilation/product tests/providers/remote jobs/submissions/external publication/resource expansion: **zero authorized**. No model/campaign/delegate run or new service.

Both invocation-selected YOLO flags set to0, pycache disabled and Git optional locks disabled, as recorded in the receipt. Parent selected flags retained separately in preparation. These observed analysis-process controls do **not** establish ordinary product runtime controls. Python network audit denial is not kernel or transitive isolation. The release binary build-info gate and runtime controls were not reached.

## Exact invocation and outcome

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 python3 -B artifacts/harness-research/20261015-local/H01/iteration12-image-conjuncts/run-once.py
```

**Actual exit1; one invocation, no retry.** Parent-observed wall **0.964720s**; analyzer wall **0.896194s**. Analyzer reported **12/13 guard assertions satisfied**, not 13 product tests. Three direct children (`git rev-parse HEAD`, `git status --porcelain=v1 --untracked-files=all`, `git diff HEAD`) exited0. Scope, expected identities, analyzer and wrapper unchanged through invocation.

Exact command argv/clocks/status/output hashes: `invocation-receipt.json`, `commands.json` and per-child receipts/stdout/stderr. Overall failed status and traceback retained verbatim in `result.json`, `traceback.log`, `analysis.stderr.log`; not relabeled green. Raw endpoint table: `image-conjuncts.json`.

## New concrete finding

| Original predicate | A-baseline | B-candidate |
|---|---|---|
| Test executable SHA256 equals frozen image metadata | Match | Match |
| `os.access(executable, X_OK)` equals true | Match: true | Match: true |
| Original build-receipt SHA256 | Match | Match |
| Original source-inventory file SHA256 | Match | Match |
| Current `angel-sandbox` SHA256 equals original image helper pin | **Mismatch** | **Mismatch** |

**8/10 image predicates matched.** Both test executables were regular, nonsymlink files, mode0755, executable by this worker. Their byte hashes matched:

- A: `7fd61e4ed22dd90b28505cc389bc2c896f44cd5ab1e65c70785cdbab6a4d9d33`.
- B: `35433f86c0c987de1db1bd8a6e7c32514aec894b834d6bd5dd2b96b64736e1d2`.

The current shared path `cockpit/target/release/angel-sandbox` was read independently for each arm:

- Expected, both original image records: `1bebf47d10e03b4144788f5e78b058914d73222afbf47b4bb9031cad5b0d9996`.
- Actual, both reads: `f0441c4b5910457a231533c9922d6eb52a1b2087e77961a74997ad926b68b177`.

**The access-state explanation is refuted at this observation; a current shared-helper byte identity mismatch is established.** This does not prove that either retained test executable changed. The two helper comparisons target the same shared path: they are not independent replications or two distinct helper failures. No cause, writer, replacement time, functional consequence or malicious action was established. Most importantly, this current measurement does **not** identify retrospectively which condition failed at iteration11's earlier instant. That earlier failure still lacked individual results.

## Stop, unreached measurements and integrity

The registered `no image content/hash drift` guard failed immediately after the ten-predicate group. No analyzer edit, new diagnostic, rebuild, chmod or retry followed.

Unreached in this block: original release executable/native script revalidation; current angel-source-v3 computation; all frozen A/B/current source inventory contents; resource map validation; added build-input classification; release build-info/current-source comparison; host load/RAM/disk; historical raw send/recovery/cancellation extraction. Hashing the source-inventory **file** is not revalidating its thousands of member files. HEAD equality alone is not source/binary/resource qualification. Earlier measurements remain historical, not re-certified here.

Initial integrity check: **593/594** prior indexed references matched, with only known entry425 report mismatch. Its indexed hash remains `a6443c81c3b8a0143f01e84609c0824cb417da626faaaca8e635402f0c6cdf5a`; current report hash `3a29573ba70d57bf453f197c345a23a852c15cc5923c6480496a3eaa1f782cb9`. No original bytes reconstructed, no old row rewritten, no global-integrity pass. Capsule native-index omission remains the separately retained iteration11 finding, not repaired here.

New product-test executions, builds, initiated physical product sends and provider calls: **0**. New per-case physical-send counts, recovery and cancellation outcomes remain **null/unmeasured**, not measured zeros. No sockets were requested by this diagnostic. No live route, remote job, submission, push or external publication; no safety/resource change or unrelated/game edit. Preparation policy exclusion, elided output and nonexistent optional read path are retained in preparation records; they were not product tests or analyzer retries.

Final preservation-only checks and exact index counts are in `closeout-integrity.json`, `closeout-commands.json` and `final-receipt.json`. Those checks bind unchanged HEAD/diff, original index prefix and old evidence; they do not rerun the failed endpoint or promote the candidate. This report is written once and indexed; no closeout results are backfilled into earlier evidence.

## Decision, limitations and next action

**H12-I refuted; current helper identity discrepancy isolated. Overall qualification: INCONCLUSIVE / BLOCKED / NO PROMOTION.** Keep existing product candidate and negative results intact, without newly reaffirming historical qualification. No H02 registration, release, solve-rate/speed/cost, holdout, matched-peer or leadership claim. No independent audit has run.

Next necessary action, only under a distinct finite prospective block: establish helper provenance and whether an immutable copy of the original expected helper exists, comparing explicit retained build/identity receipts without execution or replacement. Do not rebuild or overwrite the current shared helper to make the old pin appear valid. A later authorized replay would need a hash-bound helper from the correct build profile and scoped effective-control revalidation, with every old attempt retained. Missing historical report bytes and defective capsule description remain independent blockers. Live/remote/paid/task/release work still requires recorded authorization and finite budgets; pause at that gate.
