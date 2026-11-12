# Mage-VL-Cowork grounding probe — 2026-10-30

> **2026-11-02 self-verification** (`web_loop_verify.py`, 26 fixture targets,
> real clicks, plus one inert background click per target as the control): the
> hand can confirm its own click, but only through its coordinate channel.
> Asked to ground *the state the click produced* ("the control that is outlined
> in red"), the returned point lands inside the control that was clicked for
> **20/23 activations**, and on **0 of 28 non-activations** (3 failed clicks +
> 25 background clicks). With the page's visible answer switched off
> (`--mode none`) it confirms **0/23**: it reads the result, it does not
> reconstruct the click. The same question in language is unusable — 26/26
> replies degenerate into a `<think>` token loop in the runtime prompt format.
> Cost: one extra `ground` call, median 9.0 s. Combined with the pixel-diff rule
> from `web_loop_retry.py` (whose "no change" is the trustworthy half), the
> conjunction gives 20/20 precision on this scene. Table and caveats:
> [`reports/mage-selfverify-20261102/README.md`](../../reports/mage-selfverify-20261102/README.md).

> **2026-11-01 closed loop:** a real click into a real Chrome page, verdict
> read from the page (`web_loop_bench.py`): 1080p **28/32 target actuations**,
> but a 4K or 1.25× frame only **6/26–6/32**. The same pixels upscaled drift the
> model's answer by 2.5–5.5% of the frame, so `hands.rs` now caps the grounding
> frame at 1920×1080 (`fit_frame`/`uncap_point`) — capped 4K returns to 23/26.
> Cascade worth at 1080p: 1 look 24/32 (1.5 s), +2× 27/32, +3× 28/32 (5.4 s,
> median error 4.1 px).

> **2026-10-31 recovery:** The offline A/B was regenerated from all 618 paired
> records: held-out desktop **119/129 (92.2%)**, web **124/132 (93.9%)**.
> The fine-tuned **live** benchmark is still incomplete: all 24 archived attempts
> failed to launch with exit 127, not 24 model failures. Base live remains 23/24.
> Repaired scoring, raw-input hashes, dashboard data, and approval-gated rerun
> instructions: [recovery report](../../reports/mage-recovery-20261031/README.md).


Step 1 and step 2 of `docs/MAGE-PLAN.md`: can `microsoft/Mage-VL-Cowork`
be angel's GUI eye (`ground` → capsule-approved `click`) and its local
vision backend (`ANGEL_VISION_URL`)?

Baseline numbers for the integration, measured before any accuracy work:
- web pages: 46–51% hits;
- desktop apps: 34–41%;
- big rows and list items land, small icon buttons don't yet.

These numbers set what the `ground` action has to improve on. They are
not a gate on the plan.

The serving config is checked against Microsoft's own runtime: SGLang
source read out of `xiaoyizhang/mage-vl-cowork:0.1.0` layer 14, never run
(see "Config check").

## Setup

- Weights: `~/models/Mage-VL-Cowork` (HF snapshot of 2026-10-30, 2
  safetensors shards, 9.5 GB bf16). Remote code
  (`modeling_llava_onevision2.py`, `configuration_llava_onevision2.py`) read
  first: no network, subprocess or exec outside docstring examples.
- Serve: transformers 4.57.3 (the checkpoint's own version), torch 2.8.0
  cu128, RTX 3090, 11.4 GB VRAM, SDPA (no flash-attn). OpenAI-compatible
  shim `<scratch>/mage/bin/serve_mage.py` on 127.0.0.1:30001, run as user
  unit `mage-serve` (`<scratch>/mage/bin/start.sh`). vLLM was not tried:
  the card names SGLang and transformers only.
- **The card's transformers example cannot run images as published.** The
  vision tower needs `patch_positions` and dies on `None`
  (`'NoneType' object has no attribute 'device'`). The shim builds them
  from `image_grid_thw`: row-major `[t, h, w]` ids per patch, the same ids
  the model's own grid RoPE path makes. The merger ignores them
  (`use_patch_position_encoding` false).
- The tokenizer's "incorrect regex pattern" warning is a false positive:
  the pre-tokenizer is the stock Qwen2 split.

## Probe set

The agent display (workspace 90) was not used: `agent_start` creates a
headless output and focuses it on the operator's live Hyprland. Instead,
an isolated headless Chrome (own profile, no compositor) captured 20
public pages at 1920×1080 (`<scratch>/mage/bin/capture.mjs`). Ground
truth is each element's DOM bounding box, not hand marks. Two targets per
page, one from each size half, unique accessible name, fixed seed
(`bin/make_items.py`). The GNU Bash page rendered blank, and target 09-1
(a collapsed sidebar link) has an empty box, so both are excluded: **37
targets on 19 pages**, 10–81 px tall.

Prompt: the reference agent's `MAGE_GUI_SYSTEM_PROMPT` (read with
`ast.literal_eval`, not imported) and its first-step user turn, JPEG q95,
temperature 0. The first `<tool_call>` coordinate in [0, 999] maps to
pixels as `x / 999 · (W − 1)` (reference agent). "direct" appends: *The
target is visible on the current screen; click it directly in one
action.* A hit means the point lands inside the box.

## Config check against the reference runtime

The reference serve files were pulled from the registry and read, never
run:
- `sglang/srt/models/llava_onevision_2.py`;
- `multimodal/processors/llava_onevision_2.py`;
- `ov2_patch_positions.py`;
- the `serve` script.

| knob | reference | ours | evidence |
|---|---|---|---|
| vision `patch_positions` | row-major `meshgrid(arange t, h, w)` | same | block layout instead: 3/38 web hits vs 18/38 |
| vision RoPE | interleaved `rotate_half`, `cat([f, f])`, reordered by `grid_thw` | same code | official OV2 has the same pair |
| encoder output | last layer, optional post-norm, merger | same | |
| LM positions | 1-D (M-RoPE off: no `mrope_section`) | 1-D | |
| `lm_head` | untied, from checkpoint | untied, matches checkpoint | head ≠ embed_tokens; checked at load |
| image size | smart-resize to factor 32, max 4,000,000 px | same (1920×1080 → 1920×1088) | |
| patch size | 16 | 16 | weights `[1024, 3, 16, 16]` |
| request | JPEG q95, chat template, think on, T 0.7, top-p 0.8 | same, or greedy | T 0.7 + think: 18/38; greedy: 18/38 |

What remains different is kernel numerics only (SDPA vs triton/flashinfer).

**Run head to head (2026-10-30).** The reference container was run on the
3090: `xiaoyizhang/mage-vl-cowork:0.1.0`, Docker with
nvidia-container-toolkit 1.20.1 and a CDI spec. The probe went through
its native `/generate`, as the reference agent calls it
(`<scratch>/mage/bin/probe_ref.py`).

| set | mode | reference container | our shim |
|---|---|---|---|
| web (37) | first look, greedy | 19 | 18 |
| web (37) | `ground` cascade | 32, median 5.4 px | 32, ~5 px |
| web (37) | T 0.7 + think (eval settings) | 18 | 18 |
| desktop (29) | first look, greedy | 11 | 11 |
| desktop (29) | `ground` cascade | 17, median 63 px | 17, 63 px |
| desktop (29) | T 0.7 + think | 11 | 11 |

The shim was not costing accuracy.
- `ground`'s exact request works unchanged on the container's OpenAI
  endpoint (`/v1/chat/completions`, model `Mage-VL-Cowork`).
- About 0.6 s per look there vs 1.7 s on the shim, so roughly 2 s per
  three-look `ground`.

A whole screen downscaled to 960×540 gives 1/37 hits but no coordinate
shrink (answer/truth ratio 0.93, 0.99): 1080p web text is too small to
read at half size. A crop sent unscaled is what shrinks the answers, which
is why `ground` scales crops back up.

## Results

| mode | hits | ≥28 px targets | <28 px targets | median error | median error on hits | s/call |
|---|---|---|---|---|---|---|
| direct, no think | 18/37 (49%) | 12/18 | 6/19 | 24.6 px | 12.7 px | 1.8 |
| direct, think | 19/37 (51%) | 12/18 | 7/19 | 26.5 px | 15.2 px | 5.3 |
| plain, no think | 18/37 (49%) | 12/18 | 6/19 | 24.6 px | 14.6 px | 1.8 |
| plain, think | 17/37 (46%) | 11/18 | 6/19 | 30.4 px | 14.4 px | 5.7 |

- Every reply parsed (one in the plain-think run had no click).
- Image went in at full resolution: about 3,000 prompt tokens, about
  2,040 of them image.
- No systematic offset on near misses (median dx +0.8 px, dy −2.0 px), so
  the coordinate mapping is right. The misses are the model's.
- Miss kinds:
  - neighbour in a dense list ("Python 3.12" for "Python 3.1 (EOL)",
    "FAQs" one row above "Deprecations");
  - small targets missed by 10–40 px ("Home" 11 px left of its box,
    the 22 px colour-scheme icon);
  - wrong region entirely ("GIT" on empty margin, OSM "Legend").
- Thinking buys nothing for grounding and costs 3× the time.
- Without "direct", Mage sometimes plans instead of pointing (searched for
  "Dolores Huerta" rather than clicking the visible link).
- The card's 87.2% ScreenSpot-v2 is its Stage-1 checkpoint, not this one,
  and ScreenSpot targets are larger than 15 px text links on a 1080p page.

## Step 2 — as the vision backend

`ANGEL_VISION_URL=http://127.0.0.1:30001/v1
ANGEL_VISION_MODEL=microsoft/Mage-VL-Cowork angel --look-image shots/12.png
"What page is this and what are the main things a user can click?"`
(release binary, methack). Result:
- 41 s to the 2,048-token cap, no description;
- repeated `<think>` blocks planning a click on the "Description" tab,
  with doubled words ("this this", "PyTorcch").

Mage is an action policy, not a describer.

Also, `/see`'s auto-sidecar asks its question in braille
(`sidecar_question`, VISION pages) and offers the ledger reader as a tool.
Mage has neither. Keep the current vision backend.

## Desktop apps (own seat)

These ran in a private nested Hyprland seat at 1920×1080: its own
pointer, keyboard and clipboard, its window parked on the offscreen
GLASSCAP output (kit: `<artifacts>/glass-test-kit`). The operator's
focus and windows were not touched; GLASSCAP's removal moved focus once,
and it was put back on DP-3.

Isolation:
- Each app ran on a private session bus (`dbus-run-session`). Without it,
  `nautilus` hands its window to the operator's running copy.
- Throwaway HOME, Wayland only, `DISPLAY` unset.

Ground truth comes from AT-SPI widget extents.
- The stock a11y launcher needs dbus-broker plus systemd, which the
  private bus lacks. So the run brings up a plain `dbus-daemon` a11y bus
  and registryd, plus a small `org.a11y.Bus` shim
  (`<scratch>/mage/desk/run_capture.sh`, `a11y_shim.py`).
- Walks skip containers with more than 300 children, because Calc's sheet
  exposes millions of cells.
- Boxes were checked by eye and are pixel-exact.

The set: 11 apps, 3 targets each, 4 dropped, 29 kept:
- Nautilus ×2, Papers, Evince, Disks, Xournal++, Pinta, LibreOffice
  Writer/Calc/Impress, printer settings.
- Dropped: a dialog box whose coordinates were relative to the dialog,
  two internal widget ids as names, and a coordinate readout
  (`desk/probe/dropped.json`).

| mode | hits | ≥28 px targets | <28 px targets | median error | s/call |
|---|---|---|---|---|---|
| direct, no think | 11/29 (38%) | 9/24 | 2/5 | 75.5 px | 1.7 |
| plain, no think | 10/29 (34%) | — | — | 81.0 px | 1.7 |
| direct, think | 12/29 (41%) | — | — | 73.7 px | 6.0 |

The split is sharp:
- **Every list row and large item hit:** Nautilus sidebar and folders,
  Disks drives, Papers outline, Pinta close.
- **No icon toolbar button hit (0/9):** LibreOffice, Evince, printer
  settings. Its reasoning says it cannot find "Text Box" on the toolbar,
  then clicks a row lower.
- **No text menu hit (0/3):** Xournal++ "Edit" and "Tools", printers
  "Printer". It clicks the next label to the right; it placed "Tools" at
  x≈403 when the label spans 295–349.


## Step 3 — `ground` (built)

`desktop ground {target, agent?, output?}`. It is an observe action, so it
needs no capsule.
- Screenshots one output: the agent display with `agent=true`, else
  `output`, else the focused monitor.
- Sends the model its trained prompt (`hands_ground_prompt.txt`,
  verbatim, Apache-2.0) with thinking off and temperature 0.
- Reads the first `<tool_call>` point and maps [0, 999] to output pixels,
  then to layout coordinates (origin plus offset over scale).
- Returns coordinates for a capsule-approved `click`.
- Talks the reference agent's protocol (see "Live in angel"): SGLang
  `/generate` with Mage's own chat template, JPEG q95.
- Garbled or off-frame replies are errors, never guessed clicks.

The endpoint is `ANGEL_GROUND_URL`, the runtime root (+ `ANGEL_GROUND_KEY`). Tests in
`tests/cockpit/tools/hands__tests.rs` cover parsing, fail-closed cases,
layout mapping (offset monitors, HiDPI) and the prompt turn.

## Zoom cascade (in `ground`)

A bare crop sent as-is halves the model's answers: 3/38 web hits. Mage
reads its [0, 999] frame at the 1920×1080 size it trained on. Scaled back
up to full size, the crop makes small targets bigger instead.

| set | first look | 2× crop | 3× crop | 2× then 3× (shipped) |
|---|---|---|---|---|
| web (37 valid) | 18 | 28 | 31 | 32 (86%), median miss 5 px |
| desktop (29) | 11 | 18 | 17 | 17, median miss 63 px |

On desktop, 7 of 29 first looks land so far off that the target is
outside the 2× crop. Within the crop the cascade hits 18 of 22. The first
look is the remaining bottleneck.

Live through the Rust path, in a private nested seat with Writer
(toolbar pixel-identical to the AT-SPI capture), 3 looks and about 7.5 s
per target:
- hit: Bold, Spelling;
- miss: Save (landed on the neighbouring Open), Text Box, Outline Format.

## Live in angel: Muse Wonder brain, Mage hands

Task, run headless by angel (`--task-json --sandbox-profile sealed`,
`ANGEL_YOLO_SMART=1`, driver `meta`/`muse-wonder-1.0`) in a private nested
seat with Writer: type "Knights Wish", select it, click Bold via `ground`,
then verify. Script: `<scratch>/mage/bin/run_wonder_hands.sh`.

- **Wonder is the brain.** It plans, types, calls `ground` with a target,
  clicks the returned point, and checks with its own vision
  (`ANGEL_VISION_*` = Wonder).
- **Mage is the hands.** It only returns a point.

| run | `ground` transport | Mage's Bold point | outcome |
|---|---|---|---|
| 193601 | SGLang OpenAI `/v1/chat/completions` | (143,48) twice | Wonder found Bold with its own vision; 19 hops |
| 194343 | same (thinking off, as always) | (1211,546), (143,48), (1211,546) | same; 25 hops |
| 200344 | **native `/generate`, Mage's own template, JPEG q95** | **(675,86), first try** | hit; 12 hops, 85 s |

**Root cause.** SGLang's OpenAI endpoint renders Mage's prompt from its
own template, not `chat_template.jinja`. On the live screenshots that
path hit Bold 0/12; `/generate` with the model's template hit 6/12, and
5 more ended 1–3 px past the button edge. The reference agent only ever
uses `/generate`.

`ground` now does the same:
- `ground_prompt` is the template's exact render for one screenshot turn,
  thinking off, pinned by test;
- the screenshot goes as JPEG q95;
- `ANGEL_GROUND_URL` is the runtime root, and `/v1` is dropped.

Ruled out on the way, on the same screenshots:
- the theme: dark vs light, 18 vs 17 of 32 matched targets;
- quoting the label;
- PNG vs JPEG;
- the visible pointer.

## Full bench (2026-10-30 night)

Kit and raw data:
`<scratch>/mage/bench`:
- `results.jsonl`: every target and arm with Mage's raw reply;
- `angel-live/`: every live run with angel's transcript, before/after
  screenshots and the verdict.

Hypotheses and predictions were fixed before the runs.

Offline: 618 targets, 288 desktop (27 scenes, 15 app families) and 330 web
(55 pages). Each split by app family or web domain into dev and held-out
test. Reference container, `/generate`, temperature 0.

| method | web | desktop |
|---|---|---|
| first look | 57.3% | 34.0% |
| + 2× zoom | 74.2% | 51.4% |
| `ground` (2× then 3×) | 79.1% [74.4–83.1] | 51.0% [45.3–56.8] |
| OpenAI chat path | 0.3% | 0.7% |

- Zoom gain: +21.8 points web (p 1e-17), +17.0 desktop (p 1e-10).
- The 3× pass: +4.8 on web (p 0.005), −0.3 on desktop (p 1).
- Kind matters more than size: icon-only 33.9% against visible text
  78.3%. Under 24 px still hits 66.3% (H2 rejected).
- LibreOffice toolbars are the worst family: Writer 0 of 24, Calc 17%.
- Noise: 8 of 618 flips at temperature 0.

Live in angel, Muse Wonder brain plus Mage hands, 8 tasks × 3 runs:
- **23 of 24 passed**, judged by the app itself;
- `ground` on target 101 of 106;
- 4 runs recovered from a miss;
- the one failure was an Underline miss that Wonder did not catch.

What was fixed or set aside during the run:
- 3 Wonder API HTTP 400s (provider) were set aside and rerun; all passed.
- The Writer verifier wanted the state to be exactly `['checked']`.
  Verdicts were re-derived from the recorded AT-SPI state.
- The live target matcher matched "formatting" to "Format". Fixed with
  whole-word matching.
- Nautilus calls are unscored: the captured home differs from the live one.

## Fine-tune: Mage hands v1 (Modal B200, 2026-10-30 night)

Operator-approved $15 on Modal; about $9.20 used:
- smoke run $0.44;
- training $6.66;
- an evaluation stopped early, about $1.90;
- the CPU download, pennies.

All runs stopped.

Data (`<scratch>/mage/train`): 7,502 samples from 122 dev-split screens.
- 38 desktop scenes: bench dev scenes plus 24 new states of dev apps.
- 84 web pages on dev domains.
- The bench test apps (Disks, Printers, Pinta, Kdenlive, Zenity, Omacalc)
  and test web domains are excluded; the check found 0 bench test screens
  in training.
- Each sample is `ground`'s exact request: a full frame, or a 2× or 3× crop
  scaled back up with its centre jittered off the target. The answer is
  Mage's own format with the box centre on the 0–999 frame. Instructions
  are phrased several ways.

Recipe (`modal_mage.py`):
- LoRA r 64, α 128, on all 252 language-model linears (132M params).
- The vision→language merger trained in full; vision encoder frozen.
- Loss on answer tokens only; lr 1e-4, cosine over wall time, batch 8.
- One epoch on one B200: 937 steps in 63.9 min at 1.96 samples/s,
  13 GiB peak. Loss 0.53 → 0.09.

Merged locally (`merge_lora.py`, pure tensor math) into
`~/models/Mage-VL-Cowork-hands-v1`. Served by the same reference
container as the base numbers.

A/B, same 618 targets and same runtime as the bench, paired by target
(`bench/ab_analysis.json`):

| split | base `ground` | tuned `ground` | paired |
|---|---|---|---|
| **desktop test (129)** | 66.7% | **92.2%** | +35 / −2, p 1e-8 |
| **web test (132)** | 78.8% | **93.9%** | +22 / −2, p 4e-5 |
| desktop dev (trained on) | 38.4% | 91.8% | +85 / −0 |
| web dev (trained on) | 79.3% | 92.4% | +31 / −5 |

On the test split:
- First look on desktop: 41.9% → 80.6%.
- Desktop icon-only 24–39 px: 27.3% → 95.5%.
- Zenity dialogs, never seen in training: 16/29 → 29/29.
- Two apps side by side: 12/24 → 23/24.
- With the tuned weights the 3× pass helps on desktop (89.9% → 92.2%).

## Next accuracy levers

- A/B the shim against the reference container
  (`xiaoyizhang/mage-vl-cowork:0.1.0`, SGLang `/generate`) on both sets.

- First look for icons: tile the screen into upscaled quadrants. This
  targets the 7 desktop first looks that never reach the crop.
- A newer Mage checkpoint, or Mage-VL (the grounding-stage base), probed
  with the same two sets: `bin/probe.py` against `probe/` and
  `desk/probe/`.

## Reproduce

All under `<scratch>/mage`:
- `bin/start.sh` starts the server;
- `bin/run_all.sh` runs the four modes;
- results land in `probe/results-<mode>.json`;
- annotated pages are in `probe/pred-<mode>-NN.png` (green hit, red miss,
  magenta truth).
