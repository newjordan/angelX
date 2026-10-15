#!/usr/bin/env bash
# run_seat.sh <bin-dir> <label> [local|grok-api|meta]
# One headless one-bug task against the stub model on a chosen seat; the
# request log lands in out/<label>.jsonl and the task's result in
# out/<label>.result.json. <bin-dir> holds that build's angel and
# angel-sandbox. SEAT=grok-api drives the xAI API-key seat (the polyglot kit's
# grok-direct route) with the stub standing in for api.x.ai; SEAT=meta drives
# the Muse seat (the kit's meta-direct, the Responses API). STUB_KEEPALIVE=1
# makes the stub keep its connections open like a provider; STUB_CYCLES and
# MAX_HOPS as in run_ts.sh. Prints rc, wall, requests and the number of
# distinct connections the requests arrived on.
# RIG_TLS_DIR=<dir with ca.pem, srv.pem, srv.key (CN/SAN localhost)> serves the
# stub over TLS behind delay_relay.py with RIG_DELAY_MS one-way (default 10),
# so a fresh connection pays a TLS round trip as on a WAN; the harness trusts
# the rig's CA through SSL_CERT_FILE.
set -euo pipefail
if [[ $# -lt 2 || $# -gt 3 ]]; then
  printf 'Usage: %s <bin-dir> <label> [local|grok-api|meta]\n' "$0" >&2
  exit 2
fi
BIN_DIR=$1; LABEL=$2; SEAT=${3:-local}
if [[ ! "$LABEL" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ || ! "$SEAT" =~ ^(local|grok-api|meta)$ ]]; then
  printf 'Invalid benchmark label or seat.\n' >&2
  exit 2
fi
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$HERE/out; mkdir -p "$OUT"
LOG=$OUT/$LABEL.jsonl; rm -f "$LOG" "$LOG.paths" "$LOG.times" "$LOG.closes"
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
TLS_ENV=()
if [[ -n "${RIG_TLS_DIR:-}" ]]; then
  TLS_ENV=(STUB_TLS_CERT="$RIG_TLS_DIR/srv.pem" STUB_TLS_KEY="$RIG_TLS_DIR/srv.key")
fi
env "${TLS_ENV[@]}" STUB_LOG=$LOG STUB_MODE=${STUB_MODE:-plain} STUB_PORT=0 STUB_READY=$READY STUB_CYCLES=${STUB_CYCLES:-0} \
  STUB_KEEPALIVE=${STUB_KEEPALIVE:-0} python3 "$HERE/stub_model.py" 2>"$OUT/$LABEL.stub.err" &
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
URL="http://127.0.0.1:$PORT/v1"
CA_ENV=()
if [[ -n "${RIG_TLS_DIR:-}" ]]; then
  RELAY_READY=$RUN_TMP/relay.ready
  RELAY_UPSTREAM=127.0.0.1:$PORT RELAY_DELAY_MS=${RIG_DELAY_MS:-10} RELAY_LISTEN_READY=$RELAY_READY \
    RELAY_LOG=$OUT/$LABEL.relay.log python3 "$HERE/delay_relay.py" 2>"$OUT/$LABEL.relay.err" &
  PROXY=$!
  rm -f "$OUT/$LABEL.relay.log"
  for ((i=0; i<50; i++)); do [[ -s "$RELAY_READY" ]] && break; sleep 0.1; done
  read -r RELAY_PORT < "$RELAY_READY"
  URL="https://localhost:$RELAY_PORT/v1"
  # RIG_UNTRUSTED=1 leaves the rig's CA out: every handshake then fails
  # certificate verification (UnknownIssuer), as in a task image whose
  # trust store lacks the provider's issuer.
  if [[ "${RIG_UNTRUSTED:-0}" != 1 ]]; then
    CA_ENV=(SSL_CERT_FILE="$RIG_TLS_DIR/ca.pem")
  fi
fi
if [[ "$SEAT" == grok-api ]]; then
  # The polyglot kit's grok-direct seat env, the stub in place of its proxy.
  SEAT_ENV=(ANGEL_DRIVER=grok-api ANGEL_API_CLUBS=grok ANGEL_GROK_API_URL="$URL" ANGEL_GROK_URL="$URL"
            ANGEL_GROK_KEY=stub-key ANGEL_GROK_API_MODEL=grok-4.7 ANGEL_GROK_MODEL=grok-4.7
            ANGEL_GROK_REASONING_EFFORT=low ANGEL_GROK_RESEARCH=0 ANGEL_SOTA_MOA_GROK_RESEARCH=0
            ANGEL_HTTP_TIMEOUT=180 ANGEL_HTTP_RETRIES=1 ANGEL_STREAM_USAGE=1)
elif [[ "$SEAT" == meta ]]; then
  # The polyglot kit's meta-direct seat env (Responses API).
  SEAT_ENV=(ANGEL_DRIVER=meta ANGEL_API_CLUBS=meta ANGEL_META_URL="$URL" ANGEL_META_KEY=stub-key
            ANGEL_META_MODEL=muse-spark-1.3 ANGEL_META_REASONING_EFFORT=low ANGEL_REASONING_EFFORT=low
            ANGEL_HTTP_TIMEOUT=180 ANGEL_HTTP_RETRIES=1)
else
  SEAT_ENV=(ANGEL_DRIVER=local ANGEL_LOCAL_URL="$URL" ANGEL_LOCAL_MODEL=stub)
fi
# RELATIVE_WORKSPACE=1 runs the task from the workspace's parent with
# `--workspace work`, as the polyglot kit passes `ws/<task>`.
WORKSPACE_ARG=$WORK
if [[ "${RELATIVE_WORKSPACE:-0}" == 1 ]]; then
  cd "$RUN_TMP"
  WORKSPACE_ARG=work
fi
START=$(date +%s.%N)
echo "$START" > "$OUT/$LABEL.start"
python3 "$HERE/exec_session.py" env -i PATH="$PATH" HOME="$HOMEDIR" TMPDIR="$RUN_TMP" TERM=dumb LANG=C.UTF-8 \
  GIT_CONFIG_GLOBAL=/dev/null "${SEAT_ENV[@]}" "${CA_ENV[@]}" \
  ANGEL_SOTA_CAVEMAN=0 ANGEL_TASK_RECON=0 ANGEL_YOLO=0 ANGEL_T_SANDBOX_HELPER="$BIN_DIR/angel-sandbox" \
  ANGEL_BOOK_INTRO=${ANGEL_BOOK_INTRO:-1} \
  "$BIN_DIR/angel" --task-json --workspace "$WORKSPACE_ARG" --task-id bench-calc --max-hops ${MAX_HOPS:-40} \
  "The unittest suite in this repository fails. Fix the code so it passes." \
  > "$OUT/$LABEL.result.json" 2> "$OUT/$LABEL.stderr" &
TASK=$!
if wait "$TASK"; then RC=0; else RC=$?; fi
END=$(date +%s.%N)
stop_task
REQUESTS=unknown; CONNS=unknown
if [[ -f "$LOG" ]]; then
  REQUESTS=$(wc -l < "$LOG"); REQUESTS=$((REQUESTS))
  CONNS=$(python3 -c 'import json,sys; print(len({json.loads(l).get("conn") for l in open(sys.argv[1])}))' "$LOG")
fi
echo "$LABEL rc=$RC wall=$(awk "BEGIN{print $END - $START}") requests=$REQUESTS connections=$CONNS"
exit "$RC"
