#!/usr/bin/env bash
# task_run.sh <angel-binary-dir> <label> [onebug|twobug]
# One headless task against a REAL model through proxy_log.py; the request log
# lands in out/<label>.jsonl, the task receipt in out/<label>.result.json.
# The binary dir must hold `angel` and its `angel-sandbox` (the harness runs the
# helper beside its own binary). Requires: PROXY_UPSTREAM (OpenAI-compatible
# base URL ending /v1), PROXY_KEY (optional), ANGEL_LOCAL_MODEL (model id).
# Arms: ANGEL_BOOK_INTRO=0|1 (default 1). MAX_HOPS (default 40).
# ROUTE picks the harness seat the proxy sits in front of (seat.sh; default
# local): local (needs PROXY_UPSTREAM, ANGEL_LOCAL_MODEL), glm-flash,
# deepseek-flash or muse (each its native seat; keys from the environment).
set -u
BIN_DIR=$1; LABEL=$2; FIXTURE=${3:-twobug}
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/seat.sh"
OUT=$HERE/out; mkdir -p "$OUT"
LOG=$OUT/$LABEL.jsonl; rm -f "$LOG"
WORK=$(mktemp -d "${TMPDIR:-/tmp}"/angel-task-ws-XXXX); HOMEDIR=$(mktemp -d "${TMPDIR:-/tmp}"/angel-task-home-XXXX)
if [ "$FIXTURE" = onebug ]; then
  printf 'def add(a, b):\n    return a - b\n' > "$WORK/calc.py"
  printf 'import unittest\n\nimport calc\n\n\nclass CalcTest(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(calc.add(2, 3), 5)\n\n\nif __name__ == "__main__":\n    unittest.main()\n' > "$WORK/test_calc.py"
  PROMPT="The unittest suite in this repository fails. Fix the code so it passes."
else
  printf 'def mean(xs):\n    return sum(xs) / (len(xs) - 1)\n\n\ndef median(xs):\n    s = sorted(xs)\n    n = len(s)\n    mid = n // 2\n    if n %% 2:\n        return s[mid]\n    return s[mid]\n' > "$WORK/stats.py"
  printf 'import unittest\n\nimport stats\n\n\nclass StatsTest(unittest.TestCase):\n    def test_mean(self):\n        self.assertEqual(stats.mean([1, 2, 3, 4]), 2.5)\n\n    def test_median_odd(self):\n        self.assertEqual(stats.median([3, 1, 2]), 2)\n\n    def test_median_even(self):\n        self.assertEqual(stats.median([4, 1, 3, 2]), 2.5)\n\n\nif __name__ == "__main__":\n    unittest.main()\n' > "$WORK/test_stats.py"
  PROMPT="The unittest suite in this repository fails. Fix the code so it passes."
fi
( cd "$WORK" && git init -q && git -c user.name=b -c user.email=b@b.invalid -c commit.gpgsign=false add -A \
  && git -c user.name=b -c user.email=b@b.invalid -c commit.gpgsign=false commit -qm init )
start_proxy "$LOG" || exit 2
START=$(date +%s.%N)
env -i PATH="$PATH" HOME="$HOMEDIR" TERM=dumb LANG=C.UTF-8 GIT_CONFIG_GLOBAL=/dev/null \
  "${SEAT[@]}" \
  ANGEL_SOTA_CAVEMAN=0 ANGEL_TASK_RECON=0 ANGEL_BOOK_INTRO=${ANGEL_BOOK_INTRO:-1} \
  "$BIN_DIR/angel" --task-json --workspace "$WORK" --task-id "$LABEL" --max-hops ${MAX_HOPS:-40} "$PROMPT" \
  > "$OUT/$LABEL.result.json" 2> >(python3 "$HERE/ts.py" > "$OUT/$LABEL.stderr")
RC=$?
END=$(date +%s.%N)
( cd "$WORK" && python3 -m unittest -q >/dev/null 2>&1 ) && SOLVED=yes || SOLVED=no
kill $PROXY 2>/dev/null; wait $PROXY 2>/dev/null
echo "$LABEL rc=$RC solved=$SOLVED wall=$(awk "BEGIN{print $END - $START}") requests=$(wc -l < "$LOG" 2>/dev/null || echo 0)" | tee "$OUT/$LABEL.run"
rm -rf "$WORK" "$HOMEDIR"
