# H01 iteration 14 — mandatory 15-iteration evidence review

## Hypothesis, phase and decision

**Author-only reconciliation, not new product execution or independent validation.** Native program `20261015-local`, loop `1792029422798-3790707`, worker iteration 14 / fifteenth local iteration. Technical Labyrinth door `q.angel-3d0199f7046f545e561ac9ea` was selected after status/plan/frontier inspection; no game door was pursued. Protocol and preflight were reread. All prior work remains unchanged.

**Decision: RECONCILED_WITH_KNOWN_DEFECTS_NO_PROMOTION.** The one registered analyzer invocation completed, deliberately **exit 2**, with **386/386 diagnostic assertions satisfied**, **four declared read-only child commands**, analyzer wall **2.201942 s**, parent-observed wall **2.244276 s**. These are audit guards, NOT product-test passes or an all-clear. There was no retry. Expected defects were enumerated separately rather than silently waived. The parent receipt wrapper returning normally does not make its nonzero child green.

Audit question A14: can the active results be reconciled to primary records, preserving every negative and uncertainty? All four v3 raw replay cells agree with their saved assessments; global integrity remains unqualified because of the historical report mismatch. Secondary hypothesis P14: the original helper remains at the mandatory pinned preflight path while the shared helper retains the previously observed different bytes. **P14 supported for these exact two current paths.** This does not recover the missing report, explain helper modification history, or establish past effective controls.

No production/fixture code changed in this iteration. H01's previously recorded `KEEP_LOCAL_MECHANISM_QUALIFIED` disposition is retained as a historical, narrowly scoped local-debug result. This audit is not another independent endorsement. **No release, task-speed, cost, solve-rate, holdout, matched-peer or leadership claim. H02 remains unregistered.**

## Design, controls and exact identities

Prospective `scope.md`, analyzer and pre-invocation logical review were appended to the existing native append-only index before the single analyzer invocation. Observations from earlier reports were disclosed; this is not a blinded or untouched confirmatory experiment. No new campaign/controller/orchestration layer was introduced.

Evidence root: `artifacts/harness-research/20261015-local/H01/iteration14-evidence-review/`.

- Baseline/common base: `7d711a031cc3fbdda9112c1be29a104289a06524`.
- Current candidate HEAD: `b7bf34a8a899f9bf20eb241eff45bfc63ae14bb1`; tracked diff empty at preparation and analyzer checks.
- Candidate production patch: `667f687d2dbe347bbfedbf3f70ff507edac2ae63cb3eac50cbb87e031e6e69b0`.
- Identical v3 fixture patch: `55009757374c6784b0db037e64204b6f1d3a3c7335de5e586688109640a26982`.
- Retained v3 runner: `667fee0625f7e7bcd7b01e85ada23a37da65c71fae7328f9ce04ff74f7de7b55`.
- Current native `angel-source-v3`: `17796a36ed95e3ca0499087bcfb5a9346ffa321cb89d8ded3d118c64d4617a31`, measured with existing `scripts/bench/polyglot/source-digest.py`.
- Analyzer: `3723d9ff8ee071e045fd92e34828f6ecd0e670676e3d73334cf65946b3510024`.
- Scope: `fe6afcf8d3b8064b309f87e8167a685431937471c0736f502bc43a1ca42e7fbf`.

Registered ceilings: one invocation, zero retries, 120 seconds, six direct read-only children, at most 1 GiB per named binary/helper / 3 GiB aggregate, at most 10,000 inventory members/tree, 3,000 resources, 128 capsule members / 16 MiB uncompressed. Actual named binary hashing: **2,091,372,504 bytes**. Two exact helper paths replaced iteration13's broad candidate-name search; its 128-name ceiling was NOT expanded and its previous failure was NOT retroactively passed. No helper execution/copy/chmod/build occurred.

Exact audit command (recorded argv uses `/usr/bin/python3`):

```sh
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 PYTHONDONTWRITEBYTECODE=1 \
  /usr/bin/python3 -B artifacts/harness-research/20261015-local/H01/iteration14-evidence-review/analyze.py
```

`invocation-start.json`, `invocation-receipt.json`, `commands.json`, raw stdout/stderr, `checks.json`, `result.json` and each primary reconciliation table retain commands, timestamps, identities and outcomes. Raw stdout SHA256 `53ecc57c545439c14a1489a16689b779591498cf7a7f07d99b488d4e01c44dda`; stderr SHA256 `148ab621ac72a291063fe98f30c7c09ee81ca91b8d2f19069ad8c009d71061a3`.

## Newly resolved identity and resource questions

### Original helper is available; the shared helper is not that image

`named-binary-identities.json` and `two-path-helper-endpoint.json` independently record each exact path, SHA256, size, mode, inode/device and timestamps. Both are regular non-symlink ELF files, **803,400 bytes**, but different content:

| Exact path | Current SHA256 | Meaning |
|---|---|---|
| `artifacts/research-preflight/20261015T015123Z/cell-config/bin/angel-sandbox` | `1bebf47d10e03b4144788f5e78b058914d73222afbf47b4bb9031cad5b0d9996` | Original pin available now; manifest `sandbox_sha256` agrees |
| `cockpit/target/release/angel-sandbox` | `f0441c4b5910457a231533c9922d6eb52a1b2087e77961a74997ad926b68b177` | Shared path differs from recorded v3 helper pin |

Pinned mode is `0500`; shared mode is `0755`. This is **not access-state-only drift**. No evidence attributes the edit, timestamp of the change, cause, or exact binary delta. Future execution cannot silently bind the shared path to the original hash. Availability of a matching pinned helper is not authorization to execute it and does not certify its runtime containment.

Both retained v3 test executables match their original images:

- A: `7fd61e4ed22dd90b28505cc389bc2c896f44cd5ab1e65c70785cdbab6a4d9d33`.
- B: `35433f86c0c987de1db1bd8a6e7c32514aec894b834d6bd5dd2b96b64736e1d2`.

Thus iteration11's compound assertion did NOT establish test-binary drift. Iteration12 isolated the helper conjunct for both arms; this audit confirms availability of the original elsewhere without rewriting those failed results.

Current release `cockpit/target/release/angel` still hashes to `b41f58c70a21cec95412b9bbc2a926cca46f53de54e811d1e4fcf0eca23bf9d4`. The retained build-info for those identical bytes records **old** source digest `5c384d7c1c456c2b00687dc570e19547fcebc8183041c401b8709ef5a27cb9bb`, release rustc1.95.0/x86_64, and resource SHA `unbound`. It is not the current candidate release. No release binary was executed or freshly queried.

### Source and resources are no longer merely planned checks

Both **3,284-file** frozen A/B inventories matched their respective on-disk trees; **all 3,284 B inventory paths also match current contents**. The three production and two fixture files were compared explicitly to frozen B. Thirteen newly tracked paths outside the inventories are separately listed in `added-tracked-paths.json`; matching inventory members is not a claim that new files were absent or bound by that inventory.

Existing native `run-polyglot-loop.py` inventory definitions were reused without its execution entry point. The original **1,677-resource** snapshot matches its native pin `8448e2a7e9c608135adc09b9a99843bd8dbccc723f5a46b0e06aedae6282033f`. Current **1,677-resource** native digest is `df14bd8ebe6d39ef2ea5bacad022572dd8f9c3cb21897161d7b7ceffc53bd7d9`. Only `http.rs`, `retry.rs`, `classify.rs` differ from that old snapshot.

The earlier `a6515caa…` current-resource digest is historical, not current: its `classify.rs` hash was `cfa8aab4…`, whereas the measured current/frozen-v3 hash is `1759a94611aa3f777b0ab8afee30826a529ea6014207c1159d8000c2fc8c53d4`. The anchored-envelope marker correction accounts for a changed treatment relative to the original candidate. Do not compare native spaced-JSON resource digests to iteration0's compact-JSON scheme.

Current source inspection confirms the mechanism: `retry.rs:142–150` classifies five typed permanent configuration kinds; `http.rs:3140–3158` suppresses their HTTP replay and emits a local marker; `turn/classify.rs:69–86` trusts that marker only in the numeric local transport envelope. Arbitrary transient response text is not sufficient. This is code inspection, not additional handshake testing.

## Reconciled physical and nonphysical outcomes

Primary files are v3 `*-receipt.json`, `*.stdout.log`, `*.stderr.log`, `*-assessment.json`, plus this audit's `raw-replay-reconciliation.json`. Historical v3 runs were ordered **A1/B1/B2/A2**, same no-default-features debug fixtures; existing build receipts use Cargo `test --offline --locked --no-default-features --bin angel --no-run --message-format=json`, two build successes. Exact test invocation filters were `h01_ provider_cancel_disconnects_ esc_during_the_resend_after_a_dead_kept_connection_ends_the_call --test-threads=1 --nocapture`. These are retained commands, **not rerun instructions authorized by this iteration**.

| Historical cell | Raw exit | Eight exact tests | Invalid URL / FTP admissions | Retry-After interval | Pooled peer EOF |
|---|---:|---|---|---:|---:|
| A1 | 101 | 7 pass; only endpoint regression fails | 6 / 6 | 1001 ms | 7 ms |
| B1 | 0 | 8 pass | 1 / 1 | 1001 ms | 7 ms |
| B2 | 0 | 8 pass | 1 / 1 | 1002 ms | 7 ms |
| A2 | 101 | 7 pass; only endpoint regression fails | 6 / 6 | 1001 ms | 7 ms |

The invalid endpoints terminate `ProviderError`; their **zero physical sends are derived from parser/scheme rejection**, not a wire counter. The 6→1 result is retry **admissions**, not paid requests avoided. Baseline exit101 is expected RED, not a green verifier.

Ten peer-observed complete-body POST controls match in every historical cell:

| Case | Physical POSTs per run | Recovery |
|---|---:|---|
| success | 1 | true |
| auth | 1 | false |
| malformed | 1 | false |
| veto | 1 | false |
| transient_503 | 2 | true |
| transient_429 | 2 | true |
| disconnect | 2 | true |
| outer_503 | 3 | true |
| outer_marker | 3 | true |
| retry_after | 2 | true |

Exactly **18 POSTs/run / 72 across four runs for these ten controls**. This excludes discovery GETs and cancellation-test traffic; it is not all network traffic. Retry-After exceeds the registered 900ms lower bound. Four direct physical cancellation tests pass per cell (**16 executions**); pooled cancellation passes per cell (**4 executions**), with actual peer EOF, cancellation-flag and ordering assertions, peer joined, asserted client/peer <2s. Only peer latency is separately printed. Six existing retry/deadline guard subcases pass per cell, but their mocked calls are NOT physical sends. No live TLS/proxy handshake or complete refresh/adaptation matrix is established.

Original iteration0 baseline/candidate used a broken discovery fixture and nonidentical network fixtures; their network A/B comparison remains inconclusive. The original candidate's seven controls total10 POSTs, not the later v3 18. Those denominators must not be conflated.

## Complete active-result / negative-result ledger

`attempt-and-decision-census.json` retains full primary result/disposition records, not only this summary. No original result is overwritten by the present interpretation.

| Block | Primary retained evidence and literal status | Reconciled scope |
|---|---|---|
| 0 | `baseline-decision.json`, `receipts.jsonl`, `baseline.log`, `candidate.log`, `result.json`; raw101→0; two original builds/executions | RED endpoint reproduced, but baseline discovery GET fixture invalid; candidate-only helper reference and changed fixture confound network comparison. Original marker-anywhere treatment later corrected. No promotion |
| 1 | `iteration1-audit/result.json`, observations and source-control review; analyzer0 | Candidate-only observations reconcile, but unbound/current-vs-release source and no matched replay/audit remained |
| 2 | `iteration2-provenance/source-provenance.json`, `closeout.json`; **no result.json exists** | Author source citations, `semantic_verifier=false`; no independent efficacy evidence |
| 3 | `iteration3-revalidation/result.json`; 166 checks | Resources/identities rechecked at that time; dirty source explicitly UNBOUND, no product replay |
| 4 | `iteration4-recovery-scope/result.json`;187 checks | Conditional request rebuild may occur despite generic retryable=false; no-action request distinct from modified request; physical outcomes unmeasured |
| 5 | `iteration5-mixed-budget/result.json`;240 checks,7 model cases,14 arm/case checks,18 budget arms | Abstract mixed429/transport trace:3 modeled admissions vs diagnostic2; shared preexisting behavior, not H01 efficacy or physical count |
| 6 | `iteration6-adaptation-scope/analysis-command.json`, `failed-checks.json`, traceback; exit1,193/194 guards | Stopped on stale vendor citation range; downstream planned analysis not executed; no result.json exists |
| 7 | `iteration7-adaptation-composition/result.json`;281 checks,10 model cases | Two sticky learners permit two modeled reentries, not a whole-call depth1 bound; identical baseline/candidate. Modified-request veto semantics still need a contract, no physical result |
| v2 | `replay-20261015T052307Z/receipts.jsonl`, result; two build0, A1/A2=101 B1/B2=0 | Later `preliminary-v2-disposition.json`: **PRELIMINARY_NOT_QUALIFIED** because permissive baseline-red oracle and pre-cancel-EOF possibility; never pool with v3 as qualified replication |
| v3 | corrected replay raw receipts, assessments, independent-context source/measurement review, `decision.json` | Historical **KEEP_LOCAL_MECHANISM_QUALIFIED**, release=false; two builds and four runs. Separate-context self review of same artifacts, not independent execution/cross-vendor/blinded reproduction. Reviewer output truncation remains explicit |
| 8 | result/receipt exit1,1/2 guards | Historical indexed report hash mismatch stops identity audit; capsule scan not reached |
| 9 | result/receipt exit1,9/10 guards | Four newline/prefix variants, capsule and bounded Git history did not recover original report; not exhaustive proof of absence |
| 10 | result/receipt exit1,14/15 guards | Patch serialization equality failed; later direct blob/controlled serialization agreement does not retroactively green it |
| 11 | result/receipt exit1,182/183 guards | Eight Git-blob comparisons and67 capsule checksummed members matched; zero native index records despite README claim; baseline3284 inventory matched. Compound image assertion failed without isolating conjunct |
| 12 | result/receipt exit1,12/13 guards; `image-conjuncts.json` | Executable/access/build-receipt/source-manifest predicates pass, helper hash fails BOTH arms. Refutes access-state-only explanation, not test-executable drift |
| 13 | result/receipt exit1,8/9 guards; final receipt |130 raw name candidates exceed128 ceiling; stopped BEFORE helper hashing, hypothesis unmeasured. Final673/674 reference result /683 index entries agrees with exact primary receipts |
| 14 | current invocation receipt, result, raw reconciliation tables; exit2 | Audit completes with known defects; exact helper pin found, raw v3/source/resources revalidated. No product execution or new independent endorsement |

No selected later green replaces any failed attempt or becomes pass@1. Historical model assertions remain model assertions. Pilot12/12, TB2 13/20 and selected5/7 in the charter are historical context, not evidence generated/revalidated here; no current speed/leadership inference uses them.

## Integrity contradictions that remain open

**Index:** the initial **683 references /682 matches** retain precisely historical row425 mismatch:

- Path: `docs/telemetry/harness-H01-2026-10-15-matched-replay.md`.
- Recorded: `a6443c81c3b8a0143f01e84609c0824cb417da626faaaca8e635402f0c6cdf5a`.
- Observed: `3a29573ba70d57bf453f197c345a23a852c15cc5923c6480496a3eaa1f782cb9`.

Known old index snapshots are exact prefixes of the initial index. Prior iteration13's673/674 check is substantiated against its674-entry table and final683-entry receipt; it is not the current683-reference denominator. Original indexed bytes have not been recovered, exact delta not fabricated, and old hash never replaced. Global index integrity cannot be declared clean even when every other reference agrees.

**Capsule:** outer SHA256 `f482ad440c1ae92b3c0b8caadf62fb0be6b3956958993cfff666e2ced5d85854` matches sidecar. All68 members are safe regular unique members; all67 payload checksums and their retained-original bytes match. Recursive parsing of every JSON/JSONL member yields **zero native index records** under the registered schema/path/hash/phase-or-event predicate, contradicting README's snapshot claim. File integrity does NOT establish description/coverage integrity. Capsule unchanged; no replacement package created or external publication performed.

**Release/helper:** current release binds old source and unbound resources, not candidate HEAD. Shared helper differs from original pin; pinned copy exists. Neither issue is solved by a clean Git tree or a matching test executable.

## Effective controls, resources and limitations

Analysis process explicitly recorded YOLO flags0 and PYTHONDONTWRITEBYTECODE1; historical replay overrides YOLO0/SMART0/Cargo jobs2 remain saved. Existing fixture source sets bounded retry/helper controls. **Ordinary product effective runtime controls remain null**: no ordinary task was executed. Python audit hooks are not kernel/transitive containment proof. No credentials were logged.

Host snapshot at07:13:29Z: load average3.74/4.98/4.75, MemAvailable67,761,944kB / MemTotal96,426,828kB; disk free122,402,283,520 bytes. No serving/job changes, GPU workload, resource expansion or performance benchmark. These are host observations, not a claim of matched timing conditions. Existing historical toolchain/build flags are retained; no new compiler version execution or rebuild was needed for this artifact audit.

**This iteration:** zero builds, product-test executions, initiated product sends, provider/model calls, delegation, remote jobs, services, submissions, external publication or resource expansion. New per-case send/recovery/cancellation outcomes remain **null/unmeasured**, not passing zeros. Host/local exhibit staging below is not external publishing or host admission.

Inspection negatives remain visible: ignore-policy listing/grep exclusions; parked/truncated read outputs; code_mode `text()` undefined (corrected read-only to console.log); two exploratory summaries incorrectly assumed result.json for2/6 and raised FileNotFoundError; a post-audit summary used nonexistent key `control_rows` and raised KeyError (correct key `observed_controls`). None ran the analyzer again or became product-test results. Analyzer stderr includes Python SyntaxWarning for return-in-finally; retained unchanged, not suppressed or repaired post-run. Its explicit exit2 is consistent with the completed result and wrapper receipt, but this analyzer is not promoted as a general-purpose tool.

## Literal target delta and next necessary action

The objective is **not met**. Current evidence is a scoped deterministic debug mechanism plus author evidence reconciliation, with four known integrity/qualification defects. No full task outcome/time/token/cost table, untouched holdout or matched-peer measurement exists for this treatment.

1. **Lineage:** exact original report bytes/provenance are missing. Recover only from an identified retained backup, or obtain explicit disposition accepting a permanently unresolved historical reference with versioned annotations. Do not guess more newline variants or rewrite old index rows.
2. **Evidence packaging:** obtain authorization for an explicitly versioned capsule correction that actually includes native index records (or corrects the claim) and is independently audited. Do not replace the original artifact or relabel its claim supported.
3. **Integration:** candidate source/test identities now match, but the shipped release image is old and resources unbound. A separately preregistered finite release/integration gate must pin exact helper path/hash, effective runtime controls, build features/toolchain/resources, required negative tests and independent review. No rebuild or qualification is authorized by this report. Matching pinned helper availability merely removes the need for an unbounded search.
4. **Coverage:** live certificate/proxy/refresh/modified-request semantics and task-efficacy effects are not established. Distinguish necessary release regressions from a broader research treatment; do not bundle preexisting adaptation work into H01.
5. **H02:** keep separate and unregistered until the operator accepts an H01 disposition and a finite preregistration. Its green-terminal effects cannot be inferred from H01 or old narration timing.
6. **Leadership:** only independently audited repeatable benefit on untouched workloads and appropriately matched peers, with all-attempt outcomes/uncertainty and exact route/model/resource budgets, could support a scoped comparison. None is inferred from synthetic counts or a selected subset.

The charter's original eight-iteration development block is already exceeded; this requested fifteenth-iteration checkpoint is reconciliation only, not renewed product/spend authorization. **Pause after preservation closeout at the integrity/integration/authorization gate.** Request an explicit next-block scope, attempt/time/spend ceiling and independent-review authorization before any build/benchmark/live/remote step. No further identical audit, helper-name expansion, speculative product patch or H02 launch is justified by this block.

Report, audit artifacts and immutable local exhibit manifest are appended to the existing index. `final-preservation.json` / `final-receipt.json` provide terminal counts/hashes, original-prefix and unrelated-file preservation checks; terminal closeout does not rerun the analyzer. Local exhibit status is `inconclusive`, never verified/pass/success. No host deposit, canonical-checkpoint success or ledger sync is claimed.
