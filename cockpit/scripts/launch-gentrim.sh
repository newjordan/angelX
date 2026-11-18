#!/usr/bin/env bash
set -euo pipefail
RR=${ANGEL_BENCH_ROOT:-~/angel_tests/angelX-bench/polyglot-20260921}
cd "$RR"
export ANGEL_BIN="$RR/pin/angel-gen-trim-09a15bd/angel"
export OUT_ROOT=runs-gentrim-full
export ANGEL_TASK_COMPACT_PROMPT=1
export ANGEL_TASK_CODING_DISCIPLINE=1
while pgrep -f -- "^([^[:space:]]*/python[^[:space:]]* )?$RR/prime_v1/.venv/bin/eval .* -o $RR/runs-compact-full/qwen/angelx( |$)" >/dev/null; do
  sleep 60
done
exec bash run-cell.sh qwen angelx 0
