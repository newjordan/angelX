#!/usr/bin/env bash
# run_one.sh <angel-binary> <sandbox-helper> <label> <plain|reader>
# One headless task against the stub model; the request log lands in out/<label>.jsonl.
set -u
BIN=$1; HELPER=$2; LABEL=$3; MODE=$4
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$HERE/out; mkdir -p "$OUT"
LOG=$OUT/$LABEL.jsonl; rm -f "$LOG" "$LOG.paths"
WORK=$(mktemp -d "${TMPDIR:-/tmp}"/angel-bench-ws-XXXX); HOMEDIR=$(mktemp -d "${TMPDIR:-/tmp}"/angel-bench-home-XXXX)
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
( cd "$WORK" && git init -q && git -c user.name=b -c user.email=b@b.invalid -c commit.gpgsign=false add -A \
  && git -c user.name=b -c user.email=b@b.invalid -c commit.gpgsign=false commit -qm init )
PORT=$((18000 + RANDOM % 1000))
STUB_LOG=$LOG STUB_MODE=$MODE STUB_PORT=$PORT STUB_CYCLES=${STUB_CYCLES:-0} python3 "$HERE/stub_model.py" 2>"$OUT/$LABEL.stub.err" &
STUB=$!
for _ in $(seq 50); do curl -s "http://127.0.0.1:$PORT/v1/models" >/dev/null && break; sleep 0.1; done
START=$(date +%s.%N)
env -i PATH="$PATH" HOME="$HOMEDIR" TERM=dumb LANG=C.UTF-8 \
  GIT_CONFIG_GLOBAL=/dev/null \
  ANGEL_DRIVER=local ANGEL_LOCAL_URL="http://127.0.0.1:$PORT/v1" ANGEL_LOCAL_MODEL=stub \
  ANGEL_SOTA_CAVEMAN=0 ANGEL_TASK_RECON=0 ANGEL_YOLO=0 ANGEL_T_SANDBOX_HELPER="$HELPER" \
  ANGEL_BOOK_INTRO=${ANGEL_BOOK_INTRO:-1} \
  "$BIN" --task-json --workspace "$WORK" --task-id bench-calc --max-hops ${MAX_HOPS:-40} \
  "The unittest suite in this repository fails. Fix the code so it passes." \
  > "$OUT/$LABEL.result.json" 2> "$OUT/$LABEL.stderr"
RC=$?
END=$(date +%s.%N)
kill $STUB 2>/dev/null; wait $STUB 2>/dev/null
echo "$LABEL rc=$RC wall=$(awk "BEGIN{print $END - $START}") requests=$(wc -l < "$LOG" 2>/dev/null || echo 0)"
rm -rf "$WORK" "$HOMEDIR"
