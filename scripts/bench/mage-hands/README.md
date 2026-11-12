# Mage hands bench and fine-tune kit

The tools behind `desktop ground` (microsoft/Mage-VL-Cowork as angel's GUI
hands) and its numbers. Results and method: `docs/telemetry/mage-grounding-probe-20261030.md`.

The scripts run from a mage working folder, `$MAGE_ROOT` (default `~/mage`):
- bench data and results: `$MAGE_ROOT/bench`
- training data and adapters: `$MAGE_ROOT/train`
- the model weights: `$MAGE_MODELS` (default `~/models`)
- `live_task.sh` benches the binary named by `ANGEL_BIN`; the scripts read
  the `ground` prompt from this checkout unless `HANDS_GROUND_PROMPT` names one.
  `ANGEL_BENCH_BIN` names it too, and wins when both are set; with neither,
  `cockpit/target/fast/angel` in this checkout is used.

Run the live scripts directly from this repository: they resolve sibling scripts
relative to themselves and use `$MAGE_ROOT/bench` for data. Copies of the scripts kept
under `$MAGE_ROOT/bench` are older; do not use them for live reruns.

Recovery report and reanalyzed results:
[`reports/mage-recovery-20261031/README.md`](../../../reports/mage-recovery-20261031/README.md).
The offline A/B is complete; the tuned live benchmark has **0 valid runs**,
not a measured 0% task-success rate.

## serve/ — the runtime `ground` talks to

- `ground` needs `ANGEL_GROUND_URL` set to the runtime root, e.g.
  `http://127.0.0.1:30001` (in `~/.angelX/angel.env`).
- It posts Mage's own chat-template prompt and a JPEG q95 screenshot to
  SGLang's native `/generate`. SGLang's OpenAI chat endpoint builds a
  different prompt and grounds at 0.5%.
- The runtime is Microsoft's reference container,
  `xiaoyizhang/mage-vl-cowork:0.1.0`. It needs root for docker here and the
  NVIDIA Container Toolkit (`setup_gpu_docker.sh`).
- `start_ref_tuned_lowmem.sh` serves the fine-tuned weights
  (`$MAGE_MODELS/Mage-VL-Cowork-hands-v1`) and leaves ~3 GB of the 3090 free
  for the private seat.
- `start_ref.sh` serves the base weights.
- `serve_mage.py` / `start.sh` is a transformers shim. It speaks OpenAI chat
  only; numbers match the container.

## bench/ — offline and live

Desktop truth comes from the accessibility tree, on a private nested
Hyprland seat with its own D-Bus and no service activation. The operator's
screens, pointer and clipboard are never touched.

Web truth comes from page element boxes in an isolated headless Chrome.

Offline:
1. `capture_bench_desk.py` (via `run_bench_capture.sh` inside
   `dbus-run-session --config-file=session-isolated.conf`), plus `capture.mjs`
   for web.
2. `make_bench_items.py` makes the targets. Dev/test splits are by app family
   and web domain.
3. `bench_run.py` runs the offline arms: first look, 2×, cascade, repeat,
   chat path. `BENCH_FAST=1` and `BENCH_OUT` select a subset and an output.
4. `analyze.py` and `analyze_ab.py` do the statistics: Wilson intervals and
   exact McNemar.

Live:
- `live_bench.sh` runs each task in `live_task.sh`: angel headless,
  `--sandbox-profile sealed`, Muse Wonder as driver, Mage via `ground`, and
  an independent verifier per task.
- Both live entry points refuse Muse use unless the operator explicitly
  approves spending and the caller sets `MAGE_ALLOW_MUSE=1` for that run.
  There is no automatic alternate driver.
- The runner checks the binary and local Mage `/generate` before starting a
  private seat, selects that seat by exact PID/socket/signature, archives
  invalid attempts, and stops on infrastructure trouble. Test apps use
  software rendering. It never treats the brain's fallback vision as Mage.
- `live_score.py` separates launcher/provider/transport errors from model
  failures, rejects runs without Mage use, and lists missing repetitions.
  `--require-complete` exits nonzero unless every expected run is valid.
  It accepts `LIVE_DIR`, `LIVE_REPS` (default 3), and `LIVE_TASKS`.
- `build_ab_datasets.py` emits completeness and null pass rates for an empty
  live benchmark. Do not publish missing evidence as 0% accuracy.

After the operator starts the sudo-only tuned container **and approves Muse
spending**, use:

```bash
MAGE_ALLOW_MUSE=1 bash scripts/bench/mage-hands/bench/live_bench.sh --preflight
MAGE_ALLOW_MUSE=1 LIVE_DIR=angel-live-tuned \
  bash scripts/bench/mage-hands/bench/live_bench.sh 3
LIVE_DIR=angel-live-tuned python3 -I scripts/bench/mage-hands/bench/live_score.py \
  $MAGE_ROOT/bench --require-complete
```

Closed loop (`web_loop_bench.py` + `web_loop_driver.mjs`) — no Seat, no
Muse, no VRAM beyond the model already served:

- An isolated headless Chrome (own profile, CDP on a spare port) is asked for
  a screenshot; the scorer runs `hands.rs`'s exact cascade on it, then
  dispatches a **real** `Input.dispatchMouseEvent` at the answer and reads what
  the page did (`window.__act`, or the new URL for a link). Truth is the DOM
  box of the named element.
- `--scale` sets the display's device-pixel ratio (screenshots grow, CSS
  viewport does not); `--cap` applies the runtime's 1920×1080 frame cap before
  the look and maps the point back, so an arm can be compared against the
  fix.
- `--view WxH` sets the CSS viewport (layout size), so the same cascade can be
  measured on a laptop frame (1280×720), 4:3 (1024×768) or ultrawide
  (3440×1440) layout, not only on a 1920×1080 one.
- `--min-frame` applies the runtime's *lower* bound too (frames under 1920×1080
  are resampled up before the look); `--floor-mode contain|cover|stretch` picks
  how the box is met, which is how per-axis answer drift was separated from
  frame size.
- `absent_target_probe.py` asks the same runtime for controls the page does not
  have. It never refuses: 8/8 absent asks returned a point, 5/8 landed on a real
  control. Grounding is a pointer, not a judge — act, then screenshot to verify.
- Rows and summaries go to `--work` (`$MAGE_ROOT/loop/web-loop-*.jsonl`).

```bash
$MAGE_ROOT/venv/bin/python -I scripts/bench/mage-hands/bench/web_loop_bench.py \
  --work $MAGE_ROOT/loop --scale 2.0 --cap
```

Result: 1080p actuates 28/32 targets, a 4K frame without the cap 6/26, and the
same 4K frame capped 23/26 — the model's frame is not resolution-invariant, so
the runtime caps it. Numbers, probes and caveats:
[`reports/mage-web-loop-20261101/README.md`](../../../reports/mage-web-loop-20261101/README.md).

The mirror case is a frame *under* 1080p: a 1280×720 layout actuates 7/32, the
same frame resampled into 1920×1080 actuates 27/32, and a 1024×768 layout only
recovers when each axis is stretched onto 1920×1080 — the drift is per-axis and
linear in that axis's ratio, so the runtime now fits every shot to
`GROUND_FRAME`. Arms, medians and the 4:3/ultrawide/zoom-cascade discriminators:
[`reports/mage-frame-fit-20261101/README.md`](../../../reports/mage-frame-fit-20261101/README.md).

Does the click *work*, and does the hand know? `web_loop_retry.py` clicks, then
looks at the new frame and retries under four policies (`repeat`, `neutral`,
`feedback`, `annotated`). Measured: the frame as verifier is asymmetric (an
unchanged frame is evidence a click failed; a changed frame is not evidence it
worked), and retrying with the history in the prompt recovers nothing — 0/15 on
fixtures at three attempts. `web_loop_verify.py` then measures the two ways the
*model* could verify, on the same clicks plus a background decoy click per
target: `--verifier nl` (ask it in language over a before/after composite) and
`--verifier reground` (a pointing question — "the control that is outlined in
red" — scored by whether the pointer lands inside the control that was
clicked). `--mode none` switches the page's visible answer off as the control.
Result: the pointer confirms 20/23 real activations and 0/57 non-activations (both arms),
and drops to 0/23 when the page shows no state; the language channel produces a
`<think>` token loop on 26/26 replies. The pointer's "no" is not evidence, the
pixel diff's "no change" is — the two asymmetries are complementary, and their
conjunction gives 20/20 precision on this scene. Numbers and caveats:
[`reports/mage-selfverify-20261102/README.md`](../../../reports/mage-selfverify-20261102/README.md).

A controller is also asked for *states*, not clicks. `web_loop_state.py` puts
five renderings of a two-state control on one page (native checkbox, ARIA
switch, selected tab, filled field, disabled button) and measures reading them
two ways plus the goal loop that follows: "make sure <pref> is on", starting on
and starting off, truth from the DOM. Measured: a blind click meets that goal
6/12 and toggles **off** every control that was already on (6/6); the pointing
channel reads state at 12/12 with a 2.6–3.2 px median, at 1, 2 and 3 controls
in the asked state; the language channel returns no verdict in the runtime
format (0/8, and 0/6 re-run at 320 tokens — the budget lengthens the
`</think><think>` loop instead of ending it); and the pointer always answers,
naming the queried control 2/6 times when *no* control is in that state. Asking
both polarities and refusing the read when the two pointers name the same
control separates those cases 24/24, and the goal loop with that rule is 12/12
with no destructive toggle and no missed action (blind 6/12, each single-channel
policy 10/12). Numbers, raw replies and caveats:
[`reports/mage-state-read-20261102/README.md`](../../../reports/mage-state-read-20261102/README.md).

Act five of the same bench (dense pane): the frame-wide state read is
**positive-only** there (tp 0.50, fp 0.00) and the dense goal loop destroys 2-3 of
six already-on toggles; the compound "the X checkbox, if it is ticked" question
degrades to a pure identity read (fp 1.00), so identity and state cannot be had
from one pointer question. Numbers: same report, addendum at the end.

Regression tests (no model or provider calls):
`python3 -I scripts/bench/mage-hands/bench/test_live_bench.py`.

`seat.sh up|down` brings the private seat up and down and restores the
operator's focus. It needs a few GB of free VRAM: the NVIDIA EGL driver
SEGVs on the nested compositor's first frame when the 3090 is nearly full.

## train/ — local repair, not GPU-verified

**AngelX's primary driver does the thinking; Mage only grounds a named target.**
No Kimi fallback, Muse call, or separate image-analysis service belongs in this
regime. No serving or provider configuration was changed in the local repair.

The historical mixed-v2 pilot is not a completed live-navigation regime. It used
shopping/Store screens, labelled synthetic library views and desktop/web replay.
Later scripted navigation captures were not used in that paid run. Native Steam
and primary-driver live-task gates remain unpassed.

### Frozen local recovery datasets

Latest: `$MAGE_ROOT/regime-recovery-v1/dataset-20261101-native-v1`, with
**10,773 train / 1,468 validation / 1,258 test** rows and 806 screenshots.
The full artifact audit passed. This adds 53 click-verified native labels from
Calculator, Text Editor and Nautilus (107 rows including training crops), while
preserving every old split-file byte. Native Steam capture was blocked by a
`glXChooseVisual failed` fatal assertion in the isolated copied runtime; no error
dialog or blank screen was counted as native Steam data.

See the [native extension, evidence and limitations](../../../reports/mage-native-dataset-20261101/README.md).
`train/capture_native_atspi.py` records XTest clicks with window-anchored AT-SPI
labels and target-specific effects. `train/extend_native_dataset.py` rechecks raw
effects and appends to a new artifact only after the full audit. The private
capture session is stopped. No model training, evaluation, promotion or deployment
was performed for this extension; the primary-driver live gate remains unpassed.

Previous frozen baseline, retained unchanged:

`$MAGE_ROOT/regime-recovery-v1/dataset-20261031-v2` now exists and passed
the full artifact audit: **10,692 train / 1,454 validation / 1,246 test** rows,
753 screenshots, and checked boxes for all 2,400 replay examples. It includes
the saved public Store navigation screens and 18 newly captured local fake-shop
workflows; shared images/app identities were quarantined without moving holdouts.
Rows include crop variants, not independent tasks. Native Steam is **not** included.

See the [dataset, audit, coverage and exclusions](../../../reports/mage-dataset-recovery-20261031/README.md).
This artifact is local-only: no upload/training/deployment has occurred. Small
held-out transition coverage and GPU/live-model gates remain explicit limitations.

- `train/capture_shop_training.mjs`: isolated local-only scripted shop capture,
  with separate deterministic training/held-out modes and final order checks.
- `train/assemble_recovery.py`: assembles a new directory with original label
  provenance and cross-split quarantine; publishes only after the file audit.
- `train/audit_recovery.py`: reads the actual artifact, decodes/hashes all images,
  checks labels/crops against original annotations, and recomputes split overlap.

- `train/regime.py`: dataset/split checks plus production-parser and coordinate
  helpers, with raw-reply replay. Historical flag-only evaluations are rejected
  by the repaired verifier, not silently upgraded.
- `train/regime_batch.py`: padded multimodal minibatches, causal answer-only loss,
  token-weighted accumulation and short-window updates. CPU tests check these
  mechanics, not full Mage execution or B200 throughput.
- `train/modal_regime.py`: **unexecuted revised trainer**. Separate recovery
  paths, dry-run defaults, explicit approval/budget/scope/hash arguments; full
  frozen offline evaluation and fresh checkpoint reload before candidate eval.
  This is not permission to launch it. GPU batch-size/throughput preflight and
  confirmation of the navigation scope are still required.
- `train/summarize_regime.py`: replays raw replies, verifies frozen evaluation
  files and checkpoint/source hashes, and recomputes smoke numeric gates.
  These are artifact consistency checks, not independent execution attestation.
- `train/collect_regime.sh NEW_RUN NEW_DESTINATION`: an explicitly invoked cloud
  read, not part of CPU tests. Refuses existing destinations before cloud access.
  It collects repaired-run evidence only; historical reports stay intact.
- Verification and remaining gates:
  [`reports/mage-local-repair-20261031/README.md`](../../../reports/mage-local-repair-20261031/README.md).
- Historical pilot artifacts (not results of the revised trainer):
  [`reports/mage-regime-v2-20261031/README.md`](../../../reports/mage-regime-v2-20261031/README.md).

CPU-only regression suite (local torch/Pillow environment and cached Rust
`serde_json` build artifacts required; no weights or services):

```bash
CUDA_VISIBLE_DEVICES='' OMP_NUM_THREADS=1 $MAGE_ROOT/venv/bin/python -I - <<'PY'
import sys, unittest
from pathlib import Path
suite = unittest.defaultTestLoader.discover(
    str(Path('scripts/bench/mage-hands/train').resolve()), pattern='test_*.py')
result = unittest.TextTestRunner(verbosity=2).run(suite)
sys.exit(not result.wasSuccessful())
PY
```

## train/ — Mage hands v1 (historical)

1. `make_train.py`: dev-split screens only, in `ground`'s exact request
   format. Full frame plus jittered 2×/3× crops.
2. `modal_mage.py`: LoRA r64 on all 252 language-model linears plus the
   merger, one epoch on a Modal B200, about 64 min and $6.66.
3. `merge_lora.py`: pure tensor math into a copy of the checkpoint.

Held-out test: desktop 66.7% → 92.2%, web 78.8% → 93.9%.
