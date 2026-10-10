#!/usr/bin/env bash
# decide_capture.sh <bin-dir> <label> <scenario> — walk the real harness to a
# scenario's decision point with the scripted stub model (decide/scenarios.json)
# and keep every request it sent: out/<label>.jsonl. Arm: ANGEL_BOOK_INTRO.
set -euo pipefail
if [[ $# != 3 || ! "$2" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]]; then
  printf 'Usage: %s <bin-dir> <label> <scenario>\n' "$0" >&2
  exit 2
fi
BIN=$(cd "$1" && pwd) || exit 2; LABEL=$2; SCENARIO=$3
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$HERE/out; mkdir -p "$OUT"
LOG=$OUT/$LABEL.jsonl; rm -f "$LOG" "$LOG.paths"
RUN_TMP=$(mktemp -d "${TMPDIR:-/tmp}/angel-decide-run-XXXXXX")
source "$HERE/lifecycle.sh"
WORK=$RUN_TMP/work; HOMEDIR=$RUN_TMP/home; SCRIPT=$RUN_TMP/script.json
mkdir "$WORK" "$HOMEDIR" "$RUN_TMP/tmp"
if ! python3 - "$HERE/decide/scenarios.json" "$SCENARIO" "$WORK" "$SCRIPT" "$RUN_TMP/prompt" <<'PY'
import json, os, sys
spec = json.load(open(sys.argv[1]))[sys.argv[2]]
for path, text in spec["files"].items():
    full = os.path.join(sys.argv[3], path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    open(full, "w").write(text)
json.dump(spec["script"], open(sys.argv[4], "w"))
open(sys.argv[5], "w").write(spec["prompt"])
PY
then exit 2; fi
PROMPT=$(cat "$RUN_TMP/prompt")
( cd "$WORK"
  env -i PATH="$PATH" HOME="$HOMEDIR" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git init -q --template=
  env -i PATH="$PATH" HOME="$HOMEDIR" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git add -A
  env -i PATH="$PATH" HOME="$HOMEDIR" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 \
    git -c user.name=b -c user.email=b@b.invalid -c core.hooksPath=/dev/null -c commit.gpgsign=false commit -qm init )
READY=$RUN_TMP/stub.ready
STUB_LOG=$LOG STUB_PORT=0 STUB_READY=$READY STUB_SCRIPT=$SCRIPT python3 "$HERE/stub_model.py" 2>"$OUT/$LABEL.stub.err" &
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
python3 "$HERE/exec_session.py" env -i PATH="$PATH" HOME="$HOMEDIR" TMPDIR="$RUN_TMP/tmp" TERM=dumb LANG=C.UTF-8 GIT_CONFIG_GLOBAL=/dev/null \
  ANGEL_DRIVER=local ANGEL_LOCAL_URL="http://127.0.0.1:$PORT/v1" ANGEL_LOCAL_MODEL=stub \
  ANGEL_SOTA_CAVEMAN=0 ANGEL_TASK_RECON=0 ANGEL_YOLO=0 ANGEL_BOOK_INTRO=${ANGEL_BOOK_INTRO:-1} \
  "$BIN/angel" --task-json --workspace "$WORK" --task-id "$LABEL" --max-hops 20 "$PROMPT" > "$OUT/$LABEL.result.json" 2>"$OUT/$LABEL.stderr" &
TASK=$!
if wait "$TASK"; then RC=0; else RC=$?; fi
stop_task
REQUESTS=unknown
if [[ -f "$LOG" ]]; then
  REQUESTS=$(wc -l < "$LOG")
  REQUESTS=$((REQUESTS))
fi
echo "$LABEL rc=$RC requests=$REQUESTS"
exit "$RC"
