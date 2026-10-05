#!/usr/bin/env bash
# relay_run.sh <angel-binary> <sandbox-helper> <run-dir> [directive]
# Starts the relay and one headless angel task; retains receipts in a fresh run directory.
set -euo pipefail
if [[ $# -lt 3 || $# -gt 4 ]]; then
  printf 'Usage: %s <angel-binary> <sandbox-helper> <fresh-run-dir> [directive]\n' "$0" >&2
  exit 2
fi
BIN=$1; HELPER=$2; RUN=$3; DIRECTIVE=${4:-}
HERE=$(cd "$(dirname "$0")" && pwd)
# A manual run's request/reply files are evidence; never erase a previous run.
if [[ -z "$RUN" ]] || ! mkdir -p -- "$(dirname -- "$RUN")" || ! mkdir -- "$RUN"; then
  printf 'Relay output must be a fresh directory: %s\n' "$RUN" >&2
  exit 2
fi
RUN=$(cd -- "$RUN" && pwd)
RUN_TMP=
source "$HERE/lifecycle.sh"
relay_cleanup() {
  local status=$1
  cleanup
  # pending.py uses this as the completion marker, including cancellation and
  # startup failure. Publish it only after the task and relay have retired.
  printf '%s\n' "$status" > "$RUN/.rc.tmp"
  mv -f -- "$RUN/.rc.tmp" "$RUN/rc"
}
trap 'relay_cleanup "$?"' EXIT
RUN_TMP=$(mktemp -d "${TMPDIR:-/tmp}/angel-relay-run-XXXXXX")
WORK=$RUN_TMP/work; HOMEDIR=$RUN_TMP/home
mkdir "$WORK" "$HOMEDIR"
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
(
  git_fixture() {
    env -i PATH="$PATH" HOME="$HOMEDIR" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 \
      git -C "$WORK" -c core.hooksPath=/dev/null -c init.templateDir= "$@"
  }
  git_fixture init -q
  git_fixture add -A
  git_fixture -c user.name=b -c user.email=b@b.invalid -c commit.gpgsign=false commit -qm init
)
READY=$RUN_TMP/relay.ready
RELAY_DIR=$RUN RELAY_PORT=0 RELAY_READY=$READY RELAY_DIRECTIVE="$DIRECTIVE" \
  python3 "$HERE/exec_session.py" python3 "$HERE/relay.py" 2>"$RUN/relay.err" &
STUB=$!
PORT=
for ((i=0; i<50; i++)); do
  if ! kill -0 "$STUB" 2>/dev/null; then
    printf 'Relay failed to start; see %s\n' "$RUN/relay.err" >&2
    exit 2
  fi
  if [[ -s "$READY" ]]; then
    read -r PORT < "$READY"
    [[ "$PORT" =~ ^[0-9]+$ ]] && break
  fi
  sleep 0.1
done
if [[ ! "$PORT" =~ ^[0-9]+$ ]]; then
  printf 'Relay did not become ready; see %s\n' "$RUN/relay.err" >&2
  exit 2
fi
START=$(date +%s)
python3 "$HERE/exec_session.py" env -i PATH="$PATH" HOME="$HOMEDIR" TMPDIR="$RUN_TMP" TERM=dumb LANG=C.UTF-8 GIT_CONFIG_GLOBAL=/dev/null \
  ANGEL_DRIVER=local ANGEL_LOCAL_URL="http://127.0.0.1:$PORT/v1" ANGEL_LOCAL_MODEL=relay \
  ANGEL_SOTA_CAVEMAN=0 ANGEL_TASK_RECON=0 ANGEL_YOLO=0 ANGEL_T_SANDBOX_HELPER="$HELPER" \
  ANGEL_HTTP_TIMEOUT=3600 ANGEL_TURN_IDLE_TIMEOUT_SECS=3600 \
  "$BIN" --task-json --workspace "$WORK" --task-id relay-calc --max-hops 30 \
  "The unittest suite in this repository fails. Fix the code (not the tests) so it passes." \
  > "$RUN/result.json" 2> "$RUN/stderr" &
TASK=$!
if wait "$TASK"; then RC=0; else RC=$?; fi
stop_task
# Keep the manual interaction budget, but bound the final repository verifier.
( cd "$WORK" && exec python3 "$HERE/exec_session.py" --timeout "${VERIFY_TIMEOUT_SECS:-30}" \
    python3 -m unittest -q ) > "$RUN/verify.stdout" 2> "$RUN/verify.stderr" &
TASK=$!
if wait "$TASK"; then VERIFY_RC=0; else VERIFY_RC=$?; fi
stop_task
if [[ "$VERIFY_RC" == 0 ]]; then printf 'pass\n'; else printf 'fail\n'; fi > "$RUN/verify"
printf '%s\n' "$VERIFY_RC" > "$RUN/verify.rc"
echo "$(( $(date +%s) - START ))" > "$RUN/wall"
exit "$RC"
