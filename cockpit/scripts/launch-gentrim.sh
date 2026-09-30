#!/usr/bin/env bash
# B-run launcher (gen-trim cell): waits for the compact A-run to drain the
# single-stream server, then launches the generation-trim cell with compact
# prompt + coding discipline both enabled. Detached via setsid from the
# cockpit; polls once a minute so it never spins.
set -euo pipefail
RR=${ANGEL_BENCH_ROOT:-/home/frosty40/angel_tests/angelX-bench/polyglot-20260921}
cd "$RR"
export ANGEL_BIN="$RR/pin/angel-gen-trim-09a15bd/angel"
export OUT_ROOT=runs-gentrim-full
export ANGEL_TASK_COMPACT_PROMPT=1
export ANGEL_TASK_CODING_DISCIPLINE=1
# Match the evaluator command, with its optional Python interpreter, and exact
# output directory. A broad substring also matches this launcher or a shell
# inspecting it. Wait for process exit so old completed logs cannot bypass a
# currently active run, and a stopped partial run does not block the queue.
while pgrep -f -- "^([^[:space:]]*/python[^[:space:]]* )?$RR/prime_v1/.venv/bin/eval .* -o $RR/runs-compact-full/qwen/angelx( |$)" >/dev/null; do
  sleep 60
done
exec bash run-cell.sh qwen angelx 0
