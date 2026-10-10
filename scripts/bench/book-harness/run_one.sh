#!/usr/bin/env bash
# run_one.sh <angel-binary> <sandbox-helper> <label> <plain|reader>
# One headless task against the stub model; the request log lands in out/<label>.jsonl.
set -euo pipefail
if [[ $# != 4 ]]; then
  printf 'Usage: %s <angel-binary> <sandbox-helper> <label> <plain|reader>\n' "$0" >&2
  exit 2
fi
BIN=$1; HELPER=$2; LABEL=$3; MODE=$4
if [[ ! "$LABEL" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ || ! "$MODE" =~ ^(plain|reader)$ ]]; then
  printf 'Invalid benchmark label or mode.\n' >&2
  exit 2
fi
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$HERE/out; mkdir -p "$OUT"
LOG=$OUT/$LABEL.jsonl; rm -f "$LOG" "$LOG.paths"
RUN_TMP=$(mktemp -d "${TMPDIR:-/tmp}/angel-bench-run-XXXXXX")
source "$HERE/lifecycle.sh"
WORK=$RUN_TMP/work; HOMEDIR=$RUN_TMP/home
mkdir "$WORK" "$HOMEDIR"
cat > "$WORK/calc.py" <<'EOF'
def add(a, b):
    return a - b
EOF
cat > "$WORK/test_calc.py" <<'EOF'
import unittest

import calc


class CalcTest(unittest.TestCase):
    def test_add(self):
        self.assertEqual(calc.add(2, 3), 5)


if __name__ == "__main__":
    unittest.main()
EOF
( export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
  cd "$WORK" && git init -q && git -c user.name=b -c user.email=b@b.invalid -c commit.gpgsign=false add -A \
  && git -c user.name=b -c user.email=b@b.invalid -c commit.gpgsign=false commit -qm init )
READY=$RUN_TMP/stub.ready
STUB_LOG=$LOG STUB_MODE=$MODE STUB_PORT=0 STUB_READY=$READY STUB_CYCLES=${STUB_CYCLES:-0} python3 "$HERE/stub_model.py" 2>"$OUT/$LABEL.stub.err" &
STUB=$!
PORT=
for ((i=0; i<50; i++)); do
  if ! kill -0 "$STUB" 2>/dev/null; then
    printf 'Stub failed to start; see %s\n' "$OUT/$LABEL.stub.err" >&2
    exit 2
  fi
  if [[ -s "$READY" ]]; then
    read -r PORT < "$READY"
    [[ "$PORT" =~ ^[0-9]+$ ]] && break
  fi
  sleep 0.1
done
if [[ ! "$PORT" =~ ^[0-9]+$ ]]; then
  printf 'Stub did not become ready; see %s\n' "$OUT/$LABEL.stub.err" >&2
  exit 2
fi
START=$(date +%s.%N)
python3 "$HERE/exec_session.py" env -i PATH="$PATH" HOME="$HOMEDIR" TMPDIR="$RUN_TMP" TERM=dumb LANG=C.UTF-8 \
  GIT_CONFIG_GLOBAL=/dev/null \
  ANGEL_DRIVER=local ANGEL_LOCAL_URL="http://127.0.0.1:$PORT/v1" ANGEL_LOCAL_MODEL=stub \
  ANGEL_SOTA_CAVEMAN=0 ANGEL_TASK_RECON=0 ANGEL_YOLO=0 ANGEL_T_SANDBOX_HELPER="$HELPER" \
  ANGEL_BOOK_INTRO=${ANGEL_BOOK_INTRO:-1} \
  "$BIN" --task-json --workspace "$WORK" --task-id bench-calc --max-hops ${MAX_HOPS:-40} \
  "The unittest suite in this repository fails. Fix the code so it passes." \
  > "$OUT/$LABEL.result.json" 2> "$OUT/$LABEL.stderr" &
TASK=$!
if wait "$TASK"; then RC=0; else RC=$?; fi
END=$(date +%s.%N)
stop_task
finish_logger
REQUESTS=unknown
if [[ -f "$LOG" ]]; then
  REQUESTS=$(wc -l < "$LOG")
  REQUESTS=$((REQUESTS))
fi
echo "$LABEL rc=$RC wall=$(awk "BEGIN{print $END - $START}") requests=$REQUESTS"
exit "$RC"
