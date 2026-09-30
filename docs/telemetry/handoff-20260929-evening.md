# Handoff · 2026-09-29 evening

Read this first after the restart. `dev` is pushed at `9781b55`. Nothing from
this session is running on atlas or sparky. The `heesch` tmux session on
sparky (created Sep 24, angelX-017 on grok) is the operator's; leave it.

## Next: DeepSeek on the full 136

The operator wants the full polyglot-v1 136 on DeepSeek, run on the current
build and compared with the older DeepSeek 136 runs: "let's see how it really
does". It is approved to run after the restart. Nothing has been launched.

### The runs to beat (all atlas, `runs/deepseek/`)

| run | harness | pin | solved | agent total | median |
|---|---|---|---|---|---|
| `angelx/seed0-20260921T200947Z` | angelX | `pin/angel` (cb3f659b) | 133/136 | 34.5 min | 9.9 s |
| `angelx/seed0-20260923T195645Z` | angelX | `pin/angel-muse-0b3afe2e` | 134/136 | 51.4 min | 10.7 s |
| `opencode/…` | OpenCode | — | 132/136 | 58.7 min | 8.6 s |

The Sep 21 run left js-forth, js-rational-numbers and rust-decimal unsolved.
A single full run moves about ±3 tasks on its own: judge task by task against
both baselines, not by the headline alone. Timing and token breakdowns for all
of these are in `docs/telemetry/polyglot-full136-slowdown-20260926.md`.

### The same terms as the baselines

The kit's interception proxy sets DeepSeek's sampling for every harness
(`configs/deepseek-flash.toml`): `deepseek-flash`, temperature 0,
`max_tokens` 8192, reasoning `none`, thinking disabled. `run-cell.sh deepseek
angelx` keeps route `deepseek-direct`, max hops 60 and a 600 s wall. So
this session's DeepSeek changes that concern thinking and output size (the
256K cap, reasoning pass-back) are overridden, exactly as they were for the
baselines. What the new build does bring to the comparison:

- the reactive legend, on by default and now on every wire;
- the append-only prefix fixes (cache hits);
- the `/props` startup skip;
- process ownership (no orphaned helpers).

### Steps

Kit: `RR=~/angel_tests/angelX-bench/polyglot-20260921` (atlas, 8 GB).

1. **Pin the build.** It must be a directory pin, because this session changed
   the sandbox helper (process ownership) and the shared `pin/angel-sandbox`
   is older. Build from a clean checkout of `9781b55` with the source digest
   ([[bench-pin-needs-source-digest]]):
   ```
   ANGEL_BUILD_SOURCE_SHA256=$(scripts/check/cockpit-source-digest.sh) \
     cargo build --manifest-path cockpit/Cargo.toml --locked --release \
     --no-default-features --bin angel --bin angel-sandbox
   ```
   Then put `angel` and `angel-sandbox` in `$RR/pin/angel-legend-<first 8 of
   the angel sha256>/` (like `pin/angel-017/`), read-only. Check that
   `angel --build-info --json` shows a hex `cockpit_source_sha256`.
2. **Dry run** (no cost): `ANGEL_BIN=$RR/pin/angel-legend-XXXX/angel
   $RR/run-cell.sh deepseek angelx 0 "" dry`.
3. **One-task smoke:** the same with `OUT_ROOT=runs-legend` and a task id as
   the 4th argument. In `runs-legend/deepseek/angelx/<run>/wire/`, confirm
   three things:
   - the legend's English is on the wire;
   - the proxy's sampling holds (thinking off);
   - cache hits are reported.
4. **Full run:** `ANGEL_BIN=… OUT_ROOT=runs-legend nohup $RR/run-cell.sh
   deepseek angelx 0 > $RR/runs-legend.log 2>&1 &`. The baselines took
   35–51 agent minutes in total.
5. **Compare:**
   - `python3 $RR/analyze.py --glob 'runs*/deepseek/*/seed0-*'` gives per-cell
     aggregates and paired comparisons;
   - `python3 $RR/angelx_board.py --model deepseek` gives angelX against
     OpenCode and omp on DeepSeek;
   - add the task-by-task flips against both angelX baselines.

   Report solved, per language, agent total, median, and the token columns
   (input, cached, uncached, output). Benchmarks are marketing assets: plain
   copy, never touch the numbers.

**Host.** The operator's standing rule this session was "benchmarks on
sparky, not atlas". This kit, its fixtures, its venv (absolute paths) and
every baseline are on atlas. Sparky has none of it. Its toolchains
(`/usr/bin/node`, `g++`) are not the kit's (mise-installed omp and opencode,
though the angelX cell needs neither). Local compile and test time is part of
each task's wall. Recommendation: run this comparison on atlas, like for like
with the baselines, and say so; move the kit to sparky only if the operator
says so.

## What this session established

In `docs/telemetry/book-encoder-20260929.md`, log section, plus the report
at https://claude.ai/artifact/S1RkLVpyrTvFgpYeB2YPws (rebuild with
`scripts/bench/book-harness/report/build_report.py`):

- **Legend delivery:** the legend reaches every wire (`95c85d2`), and it is
  on by default.
- **Legend routing** (decision replay, 4 models, 440 replies): with the
  legend, 35/40 replies took the route's action at the batch and
  untested-edit points. Braille alone got 23/40 and 10/40, and no cue got
  21/40 at the batch point.
- **Legend vs English at every sighting** (`ANGEL_BOOK_INTRO=every`, a
  bench arm, DeepSeek):
  - the same cost within a turn;
  - the legend saves a fixed 176 prompt tokens per turn, +3,489 (+16%) by
    turn 20;
  - its stamp alone routes about as well at a repeat sighting (9 vs 11 of
    20);
  - the legend's own cost over braille alone is flat at 1,344.
- **DeepSeek conformance**, per its open-source harness and papers
  (`docs/telemetry/deepseek-blueprint-20260929.md`):
  - reasoning pass-back, 256K output, 678K compaction;
  - an append-only prefix: follow-up cache hits went 37% → 94%;
  - startup 1,169 → 403 ms;
  - the Messages API transport (`ANGEL_DEEPSEEK_API=messages`).
- Process ownership: attached helpers die with the cockpit; detached
  verifiers survive.

## Open items

- **⠧⠋ names a tool that isn't loaded.** Its action says `run_tests`, but
  `run_tests` is not among the nine tools a task-mode turn loads. Models run
  tests through `shell` instead, and Grok once searched for the tool. Fix the
  action text or load the tool.
- **Muse is not calibrated.** It chases the skills catalog's
  "systematic-debugging" hint (present on every wire). After a green run on
  braille it keeps exploring instead of answering.
- **The E3 heesch live fire never finished.** All rounds were stopped; there
  is no valid result. Long runs need the operator's approval.
- **atlas `/tmp` is at this user's quota** (37 GB of tmpfs, mostly leftover
  test-suite fixture dirs: `angel-research-embed-*`, `angelx-frames-*`,
  `angel-trajectories-test-*` …). Nothing was deleted. The bench scripts now
  use `$TMPDIR`. The test suites should clean up after themselves.
- The PTY latency smoke tests (25 ms thresholds) are flaky, as before this
  session.

## Where things are

- **Bench tools:** `scripts/bench/book-harness/`.
  - `decide_capture.sh` and `decide_turns.sh` capture decision points, single
    turn and multi-turn TUI.
  - `decide/decide.py` replays them; `decide/decide_table.py` scores them.
  - `decide/count_prompts.py` counts DeepSeek tokens.
  - The stub supports `STUB_SCRIPT` (per-turn) and `STUB_PROMPTS`.
- **Raw data:** `/work/artifacts/book-handoff-20260929/` (`decide/`,
  `sparky-out/`).
- **sparky:** `/srv/models-hdd/angelx-bench/`.
  - Builds `bin-dev`, `bin-prebook`, `bin-base`, `bin-ds`.
  - The bench copy is in `src-ds/scripts/bench/book-harness`.
  - Keys are in `~/.config/host_env/api_keys.sh`: ZAI, DeepSeek, Meta
    (Muse contributor).
  - Grok uses the operator's OAuth through `grok_bearer.py`.
