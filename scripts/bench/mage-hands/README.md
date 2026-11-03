# Mage hands bench and fine-tune kit

The tools behind `desktop ground` (microsoft/Mage-VL-Cowork as angel's GUI
hands) and its numbers. Results and method: `docs/telemetry/mage-grounding-probe-20261030.md`.
Dashboard: https://claude.ai/artifact/944SBMshgRXiL5C3owWDT9

The scripts run from a mage working folder, `$MAGE_ROOT` (default `~/mage`):
- bench data and results: `$MAGE_ROOT/bench`
- training data and adapters: `$MAGE_ROOT/train`
- the model weights: `$MAGE_MODELS` (default `~/models`)
- `live_task.sh` benches the binary named by `ANGEL_BIN`; the scripts read
  the `ground` prompt from this checkout unless `HANDS_GROUND_PROMPT` names one.

Copy the scripts into that folder (`bench/`, `desk/`, `bin/`) before rerunning them.

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
- `live_score.py` scores the runs. Provider HTTP 400s are mulligans.

`seat.sh up|down` brings the private seat up and down and restores the
operator's focus. It needs a few GB of free VRAM: the NVIDIA EGL driver
SEGVs on the nested compositor's first frame when the 3090 is nearly full.

## train/ — Mage hands v1

1. `make_train.py`: dev-split screens only, in `ground`'s exact request
   format. Full frame plus jittered 2×/3× crops.
2. `modal_mage.py`: LoRA r64 on all 252 language-model linears plus the
   merger, one epoch on a Modal B200, about 64 min and $6.66.
3. `merge_lora.py`: pure tensor math into a copy of the checkpoint.

Held-out test: desktop 66.7% → 92.2%, web 78.8% → 93.9%.
