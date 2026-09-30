#!/usr/bin/env bash
# decide_turns.sh <bin-dir> <label> <scenario> — a multi-turn session in the
# TUI (tmux) against the scripted stub: one prompt per entry of the scenario's
# "prompts", the stub following "turns" (decide/scenarios.json). Every request
# lands in out/<label>.jsonl. Arm: ANGEL_BOOK_INTRO (1, 0 or every).
set -u
BIN=$(cd "$1" && pwd); LABEL=$2; SCENARIO=$3
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$HERE/out; mkdir -p "$OUT"
LOG=$OUT/$LABEL.jsonl; rm -f "$LOG" "$LOG.paths"
WORK=$(mktemp -d); HOMEDIR=$(mktemp -d); SCRIPT=$(mktemp); PROMPTS=$(mktemp)
python3 - "$HERE/decide/scenarios.json" "$SCENARIO" "$WORK" "$SCRIPT" "$PROMPTS" <<'PY'
import json, os, sys
spec = json.load(open(sys.argv[1]))[sys.argv[2]]
for path, text in spec["files"].items():
    full = os.path.join(sys.argv[3], path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    open(full, "w").write(text)
json.dump(spec["turns"], open(sys.argv[4], "w"))
open(sys.argv[5], "w").write("\n".join(spec["prompts"]) + "\n")
PY
( cd "$WORK" && git init -q && git -c user.name=b -c user.email=b@b.invalid add -A && git -c user.name=b -c user.email=b@b.invalid commit -qm init )
PORT=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
STUB_LOG=$LOG STUB_PORT=$PORT STUB_SCRIPT=$SCRIPT STUB_PROMPTS=$PROMPTS python3 "$HERE/stub_model.py" 2>"$OUT/$LABEL.stub.err" &
STUB=$!
for _ in $(seq 50); do curl -s "http://127.0.0.1:$PORT/v1/models" >/dev/null && break; sleep 0.1; done
ENVS=(PATH="$PATH" HOME="$HOMEDIR" TERM=xterm-256color LANG=C.UTF-8 GIT_CONFIG_GLOBAL=/dev/null TMPDIR="${TMPDIR:-/tmp}"
      ANGEL_WORKSPACE="$WORK" ANGEL_ACTION_CAPSULES=0 ANGEL_TUI_MOTION=off ANGEL_IMAGE_PROTOCOL=halfblocks
      ANGEL_DRIVER=local ANGEL_LOCAL_URL="http://127.0.0.1:$PORT/v1" ANGEL_LOCAL_MODEL=stub
      ANGEL_SOTA_CAVEMAN=0 ANGEL_TASK_RECON=0 ANGEL_BOOK_INTRO=${ANGEL_BOOK_INTRO:-1})
tmux new-session -d -s "$LABEL" -x 200 -y 55 -c "$WORK" "$(printf '%q ' env -i "${ENVS[@]}" "$BIN/angel") 2>/dev/null"
say() { tmux send-keys -t "$LABEL" -l "$1"; sleep 0.5; tmux send-keys -t "$LABEL" Enter; }
# Idle: the stub's log has not grown for 6 s.
wait_idle() { local last=-1 n still=0; for _ in $(seq 150); do sleep 2; n=$(wc -l < "$LOG" 2>/dev/null || echo 0)
  if [ "$n" -gt 0 ] && [ "$n" = "$last" ]; then still=$((still + 1)); [ $still -ge 3 ] && return; else still=0; fi; last=$n; done; }
sleep 4
while IFS= read -r prompt; do say "$prompt"; sleep 2; wait_idle; done < "$PROMPTS"
tmux kill-session -t "$LABEL" 2>/dev/null; kill $STUB 2>/dev/null; wait $STUB 2>/dev/null
echo "$LABEL requests=$(wc -l < "$LOG")"
rm -rf "$WORK" "$HOMEDIR" "$SCRIPT" "$PROMPTS"
