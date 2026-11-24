# polyglot-v1 kit

Aider's [polyglot benchmark](https://github.com/Aider-AI/polyglot-benchmark) as an
agent regression test: 136 Exercism exercises in JavaScript (48), Python (34), Rust (30)
and C++ (24). Each task is a blank stub. The agent writes the solution from scratch,
and the full test suite grades it. This is the task set behind the angelX polyglot
numbers. Use it to check that a harness change didn't lose tasks or get slower, or to
put any agent CLI next to the published runs.

Requirements: Python 3.10+, git, Node 20+ with npm, a Rust toolchain, make and g++.
The kit installs pytest, the Jest/Babel test dependencies and, if missing, cmake into its
work directory. Nothing is installed system-wide.

## Quick start

```sh
cd bench/polyglot
python3 polyglot.py setup      # toolchain into ./work, then a doctor check
python3 polyglot.py build      # clone the exercises at the pinned commit, build 136 tasks
python3 polyglot.py validate   # every stub must fail and every reference solution must pass

# angelX on DeepSeek V4.1 Flash, as in the published run
export DEEPSEEK_API_KEY=...
ANGEL_BIN=$(command -v angel) python3 polyglot.py run \
  --agent angelx --config deepseek-flash --pass-env ANGEL_BIN --label my-change

python3 polyglot.py compare baselines/angelx-0.1.91-deepseek-flash.jsonl runs/my-change
```

`build` prints `"matches_published": true` when your catalog is byte-for-byte the
published one (`catalog.sha256`). `validate` should report 136/136 before you run an
agent. A task that cannot go green there is a toolchain problem, not an agent failure.

Try a few tasks first with `--tasks js-affine-cipher,py-pov,rust-accumulate,cpp-clock`,
or a whole language with `--tasks cpp`.

## What a run does

For every task:

- **Fresh workspace.** The stub is copied in and committed to a new git repo. JS tasks get
  the shared `node_modules` linked in.
- **Fresh HOME.** No config, plugins, memory or login carries over from an earlier
  attempt. Toolchain homes (cargo, rustup) and download caches stay shared, so compiles
  are not penalized.
- **Minimal environment.** The agent gets PATH, HOME, locale, TMPDIR and the `BENCH_*`
  variables below, plus anything named with `--pass-env`. It gets no API keys.
- **Metering proxy.** The agent's model traffic goes through a local proxy. The proxy holds
  the real key and imposes the config's sampling (temperature, max tokens, thinking) on
  every request, so two harnesses on one config ask the model for the same thing. It logs
  each call's latency and token usage (OpenAI chat, Responses and Anthropic Messages,
  streamed or not) to `calls/<task>.jsonl`.
- **One wall clock, default 600 s.** It is enforced from outside: the agent's whole process
  group gets SIGTERM, then SIGKILL 10 s later.
- **One attempt, graded however the agent exits.** The grader rebuilds the exercise from the
  pristine stub and copies over only the solution files. Edited tests or build files never
  reach the grade. A task passes when the suite exits 0 and its own summary reports
  passing tests and no failures.

The build follows Aider's runner. The prompt is the exercise's docs plus Aider's
instructions addendum, plus one line telling the agent to run the tests. Every test is
enabled: JS `xtest` becomes `test`, Rust `#[ignore]` is removed, and C++ test files
define `EXERCISM_RUN_ALL_TESTS`. Without that define, a Catch2 suite runs only its
first test case. Three exercises are left out on every machine: `cpp-gigasecond` and
`cpp-meetup` (their references need Boost.DateTime), and `js-ledger` (its reference
fails under Node 26's Intl currency formatting).

## Reading the numbers

`summary.md` and `compare` report:

- **agent time**: the sum of each task's agent process time, from launch to exit. This is
  the number the published tables use. It leaves out workspace setup and grading.
- **elapsed wall clock**: real time for the whole run, including setup, agent and grading.
  A full run is always well above its agent time, so compare agent time with agent time.
- **grading time**: the grader's own builds and test runs.
- **tokens**: input (cached included), the cached share, output and model calls, from the
  proxy. Cost uses the config's `prices` when it has them.

Things that move the numbers:

- **`--jobs`.** The published runs used one task at a time. Parallel tasks compete for CPU
  during Rust and C++ builds, and they share the provider's rate limits.
- **The route to the model.** A different provider, or a gateway that spreads calls across
  backends, loses the prompt cache, and every call gets slower. For OpenCode Go, the
  `deepseek-flash-opencode-go` config sends one fixed `x-opencode-session` for the whole
  run, which keeps the cache warm.
- **Time of day.** Provider latency changes over the day. When the difference matters,
  run both arms side by side.
- **TMPDIR.** Rust and C++ builds need a few GB. A small or quota-limited `/tmp` fails
  builds with "Disk quota exceeded", and those failures look like agent failures. `doctor`
  warns under 5 GB.
- **`cpp-robot-name`.** Its random-name collision test is flaky. `compare` lists a flip
  there but does not fail on it unless you pass `--strict-flaky`.

## As a regression gate

```sh
python3 polyglot.py compare runs/main runs/my-change --max-time-ratio 1.15
```

This lists every task that flipped, compares time and tokens on the tasks both runs
attempted, and gives the median per-task time ratio on the tasks both runs solved. It exits
1 if head lost a task base solved (`--allow-regressions N` allows N), or if head's agent
time is more than 1.15× base's. Models are not deterministic even at temperature 0, so for
a close call, run each arm more than once and look at the flips, not only the totals.

`--resume` continues a stopped run, skipping finished tasks. A run directory holds
`results.jsonl` (one row per task), `run.json` (settings and timing), `summary.md`,
`calls/` (per-call usage), `logs/` (agent stdout/stderr and the grader's output),
`solutions/` (the agent's solution files) and `prompts/`.

## Your own agent

`--agent` takes any shell command, or the name of a preset in `agents/`. It runs once per
task, with the task workspace as its working directory and these variables set:

| variable | value |
|---|---|
| `BENCH_PROMPT` | the task prompt (also in the file `BENCH_PROMPT_FILE`) |
| `BENCH_TASK`, `BENCH_LANGUAGE` | e.g. `cpp-clock`, `cpp` |
| `BENCH_WORKSPACE` | the workspace path (also the working directory) |
| `BENCH_TEST_CMD` | the task's test command |
| `BENCH_BASE_URL` | the proxy, standing in for the config's upstream URL |
| `BENCH_API_KEY` | a placeholder; the proxy adds the real key |
| `BENCH_MODEL`, `BENCH_PROVIDER`, `BENCH_EFFORT`, `BENCH_MAX_TOKENS` | from the config |
| `BENCH_WALL_SECS`, `BENCH_RUN_ID` | the wall clock and a unique id for the attempt |

Point your agent's base URL and model at those and pass it the prompt headlessly.
`agents/angelx.sh` is the reference wrapper: it sets angelX's task-mode settings from the
published runs and maps each provider to angelX's matching model seat. If your agent needs
a login or settings file, put it in a directory and pass `--home-template DIR`. It is copied
into every fresh HOME.

Without `--config` there is no proxy. The agent then calls its provider directly, keys
come from `--pass-env`, and token columns are left out.

## Configs

`configs/*.json` fields: `upstream` (the provider's base URL), `key_env` (the variable
holding the key, read by the proxy only) or `key_cmd` (a command that prints it), `model`,
`provider` (`deepseek`, `grok`, `glm`, `anthropic` or `openai-compatible`; used by the
agent wrapper), `effort`, `sampling` (fields forced onto every generation request; `null`
removes a field), optional `headers`, `auth` (`bearer` or `x-api-key`) and `prices` (USD
per 1M tokens: `input_uncached`, `input_cached`, `output`).

## Baselines

`baselines/angelx-0.1.91-deepseek-flash.jsonl` is the published DeepSeek run: angelX
0.1.91 (c6793df) on DeepSeek V4.1 Flash with thinking off, 2026-09-30. It solved 136/136
in 25.7 agent minutes (median task 8.8 s), with 737 model calls and 7.28 M input tokens,
81% of them cached. It has one row per task. To sit your run directly against it, use
`--config deepseek-flash`, `--jobs 1` and the default wall clock.

## Files

| file | job |
|---|---|
| `polyglot.py` | the CLI |
| `build_tasks.py` | the task build (pinned upstream commit, exclusions, prompt) |
| `grade.py` | the grader |
| `validate.py` | stub red, reference green, for every task |
| `run.py` | the per-task loop: workspace, HOME, wall clock, proxy, grading |
| `proxy.py` | the metering proxy |
| `compare.py` | summaries and the regression gate |
| `setup_env.py` | `setup` and `doctor` |
| `js/` | the pinned JS test dependencies (one lockfile for all 48 JS exercises) |

The work directory defaults to `bench/polyglot/work`; set `POLYGLOT_WORK` to put it
elsewhere. Runs go to `bench/polyglot/runs/<label>` unless you pass `--out`.
