#!/usr/bin/env bash
# decide_capture.sh <bin-dir> <label> <scenario> — walk the real harness to a
# scenario's decision point with the scripted stub model (decide/scenarios.json)
# and keep every request it sent: out/<label>.jsonl. Arm: ANGEL_BOOK_INTRO.
set -u
BIN=$(cd "$1" && pwd); LABEL=$2; SCENARIO=$3
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$HERE/out; mkdir -p "$OUT"
LOG=$OUT/$LABEL.jsonl; rm -f "$LOG" "$LOG.paths"
WORK=$(mktemp -d); HOMEDIR=$(mktemp -d); SCRIPT=$(mktemp)
python3 - "$HERE/decide/scenarios.json" "$SCENARIO" "$WORK" "$SCRIPT" <<'PY'
import json, os, sys
spec = json.load(open(sys.argv[1]))[sys.argv[2]]
for path, text in spec["files"].items():
    full = os.path.join(sys.argv[3], path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    open(full, "w").write(text)
json.dump(spec["script"], open(sys.argv[4], "w"))
PY
PROMPT=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]]["prompt"])' "$HERE/decide/scenarios.json" "$SCENARIO")
( cd "$WORK" && git init -q && git -c user.name=b -c user.email=b@b.invalid add -A && git -c user.name=b -c user.email=b@b.invalid commit -qm init )
PORT=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
STUB_LOG=$LOG STUB_PORT=$PORT STUB_SCRIPT=$SCRIPT python3 "$HERE/stub_model.py" 2>"$OUT/$LABEL.stub.err" &
STUB=$!
for _ in $(seq 50); do curl -s "http://127.0.0.1:$PORT/v1/models" >/dev/null && break; sleep 0.1; done
env -i PATH="$PATH" HOME="$HOMEDIR" TERM=dumb LANG=C.UTF-8 GIT_CONFIG_GLOBAL=/dev/null \
  ANGEL_DRIVER=local ANGEL_LOCAL_URL="http://127.0.0.1:$PORT/v1" ANGEL_LOCAL_MODEL=stub \
  ANGEL_SOTA_CAVEMAN=0 ANGEL_TASK_RECON=0 ANGEL_BOOK_INTRO=${ANGEL_BOOK_INTRO:-1} \
  "$BIN/angel" --task-json --workspace "$WORK" --task-id "$LABEL" --max-hops 20 "$PROMPT" > "$OUT/$LABEL.result.json" 2>"$OUT/$LABEL.stderr"
kill $STUB 2>/dev/null; wait $STUB 2>/dev/null
echo "$LABEL requests=$(wc -l < "$LOG")"
rm -rf "$WORK" "$HOMEDIR" "$SCRIPT"
