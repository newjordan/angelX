# Mage can check its own click — by pointing at the result, not by answering

2026-11-02. Third act of the `desktop ground` / mage-hands bench. Act one
(`web_loop_bench.py`) measured the click. Act two (`web_loop_retry.py`) measured
the only verifier available without a model — a pixel diff around the click —
and found it asymmetric: **an unchanged frame is evidence that a click failed;
a changed frame is not evidence that it worked** (a neighbour's focus ring
counts, 87 of 336 scored clicks changed the frame without reaching the
target). This act asks the open half: can the *model itself* close the loop,
and through which channel?

Answer: yes, but only through the channel it actually has. Asked in language
about a before/after pair, Mage produces no usable verdict at all. Asked to
**point at the control showing the activated state**, it confirms 20 of 23 real
activations, confirms nothing that did not happen, and — in the control where
the page's visible answer is switched off — confirms nothing at all, so the
confirmation is being read off the screen rather than guessed from the click.

## Design

Every target gets two real `Input.dispatchMouseEvent` clicks into a real
headless Chrome, so both classes are measured on the same scene:

| click | what it is | truth |
|---|---|---|
| **real** | the shipped `ground_point` cascade's answer (1920×1080 frame, zooms 2 and 3) | the page's own counter for that control |
| **decoy** | a point 60–170 px away on page background, probed and confirmed inert | nothing happened, by construction |

The page answers a click in two modes:

| mode | the page's visible answer | why |
|---|---|---|
| `ring` | the activated control gets a red outline (`.hit`) | a real app showing state |
| `none` | activation leaves no visible mark anywhere; DOM counters still move | **control**: a verifier that still confirms is inventing it |

Two verifier channels, same clicks:

- **`nl`** — one image, before-panel over after-panel of the clicked region,
  and the question "did that click activate *target*? YES or NO", in the
  runtime's own chat format (`<think>` pre-filled, as `hands.rs` does).
- **`reground`** — the model's native output channel, a coordinate. After the
  click, ground the *state the click was supposed to produce*: *"the control
  that is outlined in red"*. No target name in the question, so the answer
  cannot be the target from memory. The verdict is "the pointer landed inside
  the box of the control I clicked" — a confirmation, not an apology.

26 fixture targets (16 px and 24 px toolbar buttons, 40 px rows, checkboxes,
wide buttons, small links), temperature 0, viewport 1920×1080, DPR 1. Raw rows:
`reground-ring.jsonl`, `reground-none.jsonl`, `nl-ring.jsonl` (next to this
file), summaries beside them.

## Headline

| channel | confirms a real activation | confirms a failed click | confirms a background (decoy) click |
|---|---:|---:|---:|
| **`reground`, page shows state** | **20/23 (87 %)** | **0/3** | **0/25** |
| `reground`, page shows no state (control) | **0/23 (0 %)** | 0/3 | 0/26 |
| `nl`, runtime prompt format | 0/23 (0 %) | 0/3 | 0/25 |
| pixel diff > 20 px in the same crop (act two's rule) | 23/23 | 1/3 | 0/25 |

The control row is the finding. The same 23 clicks, the same counters moving,
the same model, and the pointer stops landing on the target the moment the
page's *visible* answer is removed. The pointer is not reconstructing the
click; it is reading the result.

The `nl` row is not "the model says the click failed"; it is worse and more
interesting: **26 of 26 replies are a `<think>` token loop**, the runtime-format
prompt for a non-grounding question degenerating before it reaches a verdict.
The chat-template framing was tried separately (5 saved pairs × 6 framings,
below): it produces 1 800–2 000 characters of narrative and hallucinates UI
that is not there ("the quick settings popover is visible"). Mage has exactly
one *usable* output channel, and it is a coordinate.

## The language channel can read the evidence — it cannot be made to say so

`web_loop_verify_framings.py` asks the same five saved before/after pairs under
six framings (raw output: `nl-framings.txt`):

| framing | verdict extracted | of the extracted, correct |
|---|---:|---:|
| A runtime format, thinking left open | 1/5 | 1/1 |
| B neutral system, thinking open | 5/5 | 3/5 — including YES on a background click and NO on a real activation |
| C runtime format, `<think>` pre-filled | **0/5** | — |
| D runtime format + BEFORE/AFTER labels | 0/5 | — |
| E neutral + labels | 2/5 | 2/2 |
| F neutral + labels, plain question | 3/5 | 3/3 — it caught the miss ("the Export report button is visibly outlined") |
| **total** | **11/30** | **9/11** |

So the model is not blind to what changed: when a verdict comes out, it is right
82 % of the time, and framing F read the one true failure correctly. But it
emits a verdict in only 11 of 30 trials, and in **0 of 5** under the format the
runtime actually uses. A channel that answers one time in three, and never in
production framing, is not a gate. Re-grounding answers 20/23 and never
refuses to answer.

## What the three unconfirmed activations were

| target | click | state pointer | why it was not confirmed |
|---|---|---|---|
| Sync on launch (checkbox) | (96, 443) | (225, 64), 400 px away | the click missed the 14 px input box and landed on its label, which toggles it — so the click *worked* but the state appeared on a control the pointer did not go to |
| Use compact rows (checkbox) | (113, 515) | (37, 468), 90 px away | same class: a label click that toggles the input beside it |
| release notes (small link) | (65, 661) | (227, 64), 618 px away | the fixture marks activated buttons and rows with `.hit`, and a link that navigates in-page shows no mark — there is no state to point at |

So the misses are exactly the two cases a confirmation channel cannot cover:
**the click landed on a control's label rather than the control** (the state is
real, on a different box) and **the control displays no state**. Neither is a
reading error, and both are visible in the record.

## Why "pointer near the click" is the wrong runtime rule

Distances from the state pointer to the click point, in CSS px:

| | min | median | max |
|---|---:|---:|---:|
| real activations (n=23) | 0.4 | 76.2 | 618 |
| decoys (n=26) | 1.6 | 340 | 673 |

A distance threshold is brittle in both directions: nine real activations point
40–100 px from the click because the click landed on a wide row's text and the
pointer went to the row's edge, while one decoy landed **1.6 px** from the click
point on an unrelated control. The rule that separates cleanly is the *box*:
is the pointer inside the control that was clicked. That is also what a runtime
can know without a DOM: the target's own grounding answer, re-asked on the
post-click frame, is the box — this bench uses the DOM's box, so the numbers
above are an upper bound on what a deployment gets.

## The two channels have complementary asymmetries

| | a *"changed"* answer means | a *"not changed"* answer means |
|---|---|---|
| pixel diff (act two) | nothing (neighbours, focus rings, robots) | **the click did not reach anything** |
| state pointer (this act) | **the click landed on the control I aimed at** | nothing (may be a label click, or a control with no visible state) |

Combining them on the same 26×2 pairs: **confirmed = pixel changed AND pointer
on target** gives 20/23 real activations confirmed, **0/28 non-activations**
(3 failed clicks + 25 decoys in this arm; the no-state control confirms 0 again) — precision
20/20. It also repairs the single dangerous cell from act two: the Save miss
that changed 932 px by hitting "Export report" is now *not* confirmed, because
the pointer went to the Export report button, not to Save.

Cost: one extra `ground` call per click, median 9.0 s in this arm (7.2 s in the
no-state control) on the same 3090 that serves `ground`. That is the price of a
confirmation a controller can act on, and it is the number to shrink next.

## Reproduce

```
$PY -I web_loop_verify.py --work <scratch>/mage/v-ring  --port 9473 \
    --mode ring --verifier reground --out reports/mage-selfverify-20261102/reground-ring.jsonl
$PY -I web_loop_verify.py --work <scratch>/mage/v-none  --port 9475 \
    --mode none --verifier reground --out reports/mage-selfverify-20261102/reground-none.jsonl
$PY -I web_loop_verify.py --work <scratch>/mage/v-nl    --port 9477 \
    --mode ring --verifier nl       --out reports/mage-selfverify-20261102/nl-ring.jsonl
```

Fixtures: 19/26 box hits at the click (`box_hit` 0.731), 23/26 actuated — the
same band as the frame-fit arms (21/30, 24/30), so this run is not a lucky
scene.

## Caveats

- The state cue is the fixture's red outline. It stands in for any UI that
  shows its state (checked box, selected tab, pressed button, toast); the
  *generic* question is "the control that is now in the activated state", and
  that wording is untested.
- `mode=none` is not perfectly silent: CSS cannot suppress a native checkbox
  tick, which is why the pixel rule still fires on 17 % of activations there.
  The pointer channel is unaffected (0/23).
- The activation truth is the page's own counter. A click that fires the wrong
  control is `actuated=false` for the intended one, which is the right truth
  for "did I do the thing I asked for".
- n=26 targets, 3 failed clicks. The failed class is small because the
  cascade is good; the decoys (25–26 per arm) carry the negative class.
- The `nl` arm measured the runtime's prompt format only; the chat-template
  comparison is small (5 pairs × 6 framings) and reported as exploratory.
