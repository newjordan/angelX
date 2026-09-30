#!/usr/bin/env bash
# relay_run.sh <angel-binary> <sandbox-helper> <run-dir> [directive]
# Starts the relay and one headless angel task; exits when angel does.
set -u
BIN=$1; HELPER=$2; RUN=$3; DIRECTIVE=${4:-}
HERE=$(cd "$(dirname "$0")" && pwd)
rm -rf "$RUN"; mkdir -p "$RUN"
WORK=$(mktemp -d /tmp/angel-relay-ws-XXXX); HOMEDIR=$(mktemp -d /tmp/angel-relay-home-XXXX)
cat > "$WORK/stats.py" <<'EOF'
def mean(xs):
    return sum(xs) / (len(xs) - 1)


def median(xs):
    s = sorted(xs)
    n = len(s)
    mid = n // 2
    if n % 2:
        return s[mid]
    return s[mid]
EOF
cat > "$WORK/test_stats.py" <<'EOF'
import unittest

import stats


class StatsTest(unittest.TestCase):
    def test_mean(self):
        self.assertEqual(stats.mean([1, 2, 3, 4]), 2.5)

    def test_median_odd(self):
        self.assertEqual(stats.median([3, 1, 2]), 2)

    def test_median_even(self):
        self.assertEqual(stats.median([4, 1, 3, 2]), 2.5)


if __name__ == "__main__":
    unittest.main()
EOF
( cd "$WORK" && git init -q && git -c user.name=b -c user.email=b@b.invalid -c commit.gpgsign=false add -A \
  && git -c user.name=b -c user.email=b@b.invalid -c commit.gpgsign=false commit -qm init )
PORT=$((19000 + RANDOM % 1000))
RELAY_DIR=$RUN RELAY_PORT=$PORT RELAY_DIRECTIVE="$DIRECTIVE" python3 "$HERE/relay.py" 2>"$RUN/relay.err" &
RELAY=$!
for _ in $(seq 50); do curl -s "http://127.0.0.1:$PORT/v1/models" >/dev/null && break; sleep 0.1; done
START=$(date +%s)
env -i PATH="$PATH" HOME="$HOMEDIR" TERM=dumb LANG=C.UTF-8 GIT_CONFIG_GLOBAL=/dev/null \
  ANGEL_DRIVER=local ANGEL_LOCAL_URL="http://127.0.0.1:$PORT/v1" ANGEL_LOCAL_MODEL=relay \
  ANGEL_SOTA_CAVEMAN=0 ANGEL_TASK_RECON=0 ANGEL_YOLO=0 ANGEL_T_SANDBOX_HELPER="$HELPER" \
  ANGEL_HTTP_TIMEOUT=3600 ANGEL_STREAM_STALL_SECS=3600 ANGEL_TURN_IDLE_TIMEOUT_SECS=3600 \
  "$BIN" --task-json --workspace "$WORK" --task-id relay-calc --max-hops 30 \
  "The unittest suite in this repository fails. Fix the code (not the tests) so it passes." \
  > "$RUN/result.json" 2> "$RUN/stderr"
RC=$?
( cd "$WORK" && python3 -m unittest -q >/dev/null 2>&1 && echo pass || echo fail ) > "$RUN/verify"
echo "$RC" > "$RUN/rc"
echo "$(( $(date +%s) - START ))" > "$RUN/wall"
kill $RELAY 2>/dev/null
rm -rf "$WORK" "$HOMEDIR"
