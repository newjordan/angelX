# Mage element-dropout positives — 2026-11-01

## Delivered artifact, not a model result

- Artifact: `<scratch>/mage/dropout-positives-20261101-v1`
- Artifact manifest SHA256: `e6c00259adc1efeb893a3d150a8f9f932725151dd260198a17934ee7eec43139`
- Branch: `angel/deepseek-flash-101012-1793494890348457847-0`
- Frozen source dataset (read only): `<scratch>/mage/regime-recovery-v1/dataset-20261101-native-v1`
- Frozen source manifest SHA256, expected and observed unchanged:
  `be4a519a04f6fee2d82259315eeebb1e5403016e74d1207220caa42aba4f20e0`
- This is a **train-only isolated positive add-on**. It does not change
  validation/test, does not emit negatives or abstentions, is not merged into the
  frozen baseline, and is not a model training, inference, GPU, cloud-upload,
  deployment, serving, prompt or parser result. Edited screenshots are synthetic
  augmentation; they are not new verified native app interactions.

## What was built

The source train split has 10,773 rows. Only the 3,706 **full-frame** rows are
parents; the 7,067 crop rows are existing crop duplicates and were not multiplied
or used as new parents. Every full-frame parent has a distinct `label_id`; no
full-frame label identity was counted twice.

For each selected parent, the artifact keeps the original full-frame geometry,
target, target box, click answer, label identity, group and train split. A
bounded subset of **other genuine source element boxes** (DOM/AT-SPI element
boxes from the frozen `sources/*-pages.json` provenance) is filled with a local
border-median background colour. Variants are:

- `compact`: one or two source-element distractors dropped.
- `context_group`: a small spatial cluster of up to five source-element
  distractors dropped.

No target outlines, arrows or target spotlights were added. No background-only
dropout regions were emitted. The target box and identifying target/context text
(element boxes sharing a non-role token with the target, or text touching the
target box) are protected and byte-identical. Ambiguous target matches are
rejected rather than guessed.

The artifact also contains 512 unchanged parent controls/provenance copies.
They are not counted as variants: `controls.jsonl` references the copied parent
images, and `parents/rows.jsonl`, `parents/labels.jsonl` and
`parents/frames.jsonl` hold the exact original records. Parent images are byte
copies, not links; the source dataset was not mutated.

## Measured counts

Actual numbers, not quotas:

| Family | Selected parents | Parents with variants | Variants | compact | context_group |
|---|---:|---:|---:|---:|---:|
| native-calculator | 22 | 22 | 44 | 22 | 22 |
| native-files | 1 | 1 | 2 | 1 | 1 |
| native-text-editor | 4 | 4 | 8 | 4 | 4 |
| replay-desk | 69 | 68 | 136 | 68 | 68 |
| replay-web | 70 | 70 | 140 | 70 | 70 |
| shopping | 70 | 69 | 134 | 65 | 69 |
| shopping-transition | 70 | 69 | 132 | 67 | 65 |
| steam-navigation | 70 | 70 | 137 | 69 | 68 |
| steam-store | 66 | 66 | 127 | 65 | 62 |
| steam-synthetic | 70 | 69 | 133 | 69 | 64 |
| **Total** | **512** | **508** | **993** | **500** | **493** |

- Distinct parent labels selected: **512** (bounded cap).
- Distinct parents with generated variants: **508**.
- Parents with exactly one variant: **23**; parents with zero variants: **4**.
- Generated variants: **993**. All 993 generated file hashes and all 993 pixel
  hashes are unique.
- Unchanged controls/provenance: **512**; 288 unique parent images because
  several labels share a source screen.
- Source full-frame parents considered: **3,706**; not selected by the bounded
  quota: **3,194**. No full-frame parent failed the ancestry/source/heldout
  safety checks; all 512 selected parents passed them.
- Native coverage: 27 parents / 54 variants across calculator, files and text
  editor. Non-native selection: 485 selected parents / 481 with variants / 939
  variants across replay, shopping and Steam families.
- Changed pixels across all variants: **10,146,186**; maximum changed fraction
  for one variant: **0.0547** of the screen; minimum remaining distractors: **2**.
- Artifact size: **1,302 files / 357,313,590 bytes** (~341 MiB), including 993
  generated screens, 288 copied parent screens, 8 copied source pages and the
  manifest/audit.

Control-type counts for generated variants (from the parent target/source
element role or target wording; `other` is an honest unclassified bucket for
plain named targets):

| Control type | Variants | Parents |
|---|---:|---:|
| button | 365 | 185 |
| link | 352 | 182 |
| other (unclassified named target) | 126 | 64 |
| text_field | 79 | 43 |
| dropdown | 30 | 17 |
| item | 16 | 8 |
| toggle | 15 | 8 |
| tab | 6 | 3 |
| menu | 2 | 1 |
| slider | 2 | 1 |

## Acceptance

Exact end-to-end artifact audit command:

```bash
<scratch>/mage/venv/bin/python \
  scripts/bench/mage-hands/train/dropout_positives/verify.py \
  --artifact <scratch>/mage/dropout-positives-20261101-v1 \
  --source <scratch>/mage/regime-recovery-v1/dataset-20261101-native-v1
```

Observed: **exit 0**, verdict **PASS**. The machine-readable audit is
`<scratch>/mage/dropout-positives-20261101-v1/AUDIT.json` (also summarized in
`audit-summary.json` in this report directory). It decoded and hashed all
generated and copied parent images and verified:

- source manifest SHA256 expected/unchanged and copied byte-identically;
- every artifact file hash against the manifest, no symlinks, exact file set;
- every selected parent, label record and frame against the frozen source train
  ancestry, with no validation/test ancestry;
- target pixels and protected target/context pixels byte-identical;
- every changed pixel inside a dropped source-element box, with dropped boxes
  disjoint from protected regions;
- original dimensions, box, target, answer and click contract (click projects
  inside the box);
- no blank/flat screens, no excessive screen edits, no duplicate generated
  images or pixels (stronger than the same-target uniqueness requirement);
- zero equality/leakage of generated images or pixels to baseline validation/test
  images/pixels;
- manifest counts, holdout digest sets, exclusions and source-page copies.

Focused regression tests (preserve-target, reject crop/heldout/non-click/
ambiguous/out-of-box cases):

```bash
<scratch>/mage/venv/bin/python -m unittest -v \
  scripts/bench/mage-hands/train/dropout_positives/test_dropout_positives.py
```

Observed: **exit 0**, 8 tests OK.

Determinism: a fresh rebuild to a temporary artifact produced the identical
manifest SHA256 `e6c00259adc1efeb893a3d150a8f9f932725151dd260198a17934ee7eec43139`;
the temporary artifact was removed. The final artifact is the canonical
`dropout-positives-20261101-v1` path above.

## Exclusions and limitations

- **30 variant-generation exclusions**: 29 `duplicate_generated_pixels` and 1
  `no_safe_dropout_candidate` (replay-desk target "Annotate document"). The
  global-duplicate rule is intentionally stronger than the required same-target
  uniqueness: a variant that would produce exactly the same pixels as an earlier
  generated image is skipped rather than emitted for a different target.
- Four selected parents yielded zero variants and remain only as
  controls/provenance: replay-desk "Annotate document", shopping "the quantity
  for Silver Mug 18 field", shopping-transition "the Checkout link", and
  steam-synthetic "the Pause button for Amber Workshop 58". Twenty-three parents
  yielded one variant. These are reported as actual counts, not hidden behind the
  512-parent quota.
- No parent-selection exclusions were needed: every full-frame train parent had a
  confident source target match and passed split/group/label/heldout checks. Crop
  rows were not used as parents.
- Dropped boxes are filled with a flat local border-median colour. This can look
  artificial and is explicitly synthetic augmentation, not a new environment or
  a verified live interaction. No background-only dropout variants were made.
- The control-type `other` bucket contains plain named targets without an
  explicit button/link/field word in the target text; it is not a claim that those
  controls are semantically unclassified in the source page.
- Selection is a bounded deterministic representative subset. It is not complete
  coverage, not a claim of balanced real-world control frequency, and not
  evidence of model improvement.

## Code and scope

New code only under `scripts/bench/mage-hands/train/dropout_positives/`:
`common.py`, `build.py`, `verify.py`, `test_dropout_positives.py`, plus this
report under `reports/mage-dropout-positives-20261101/`. No shared baseline
script, frozen dataset file, validation/test row, serving/runtime prompt,
parser or config was edited. Bulk screenshots live only in the external
artifact, not in the git commit.
