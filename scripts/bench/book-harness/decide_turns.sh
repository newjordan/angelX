#!/usr/bin/env bash
# decide_turns.sh <bin-dir> <label> <scenario> — a multi-turn session in the
# TUI (tmux) against the scripted stub: one prompt per entry of the scenario's
# "prompts", the stub following "turns" (decide/scenarios.json). Every request
# lands in out/<label>.jsonl. Arm: ANGEL_BOOK_INTRO (1, 0 or every).
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
WORK=$RUN_TMP/work; HOMEDIR=$RUN_TMP/home; SCRIPT=$RUN_TMP/script.json; PROMPTS=$RUN_TMP/prompts
mkdir "$WORK" "$HOMEDIR" "$RUN_TMP/tmp"
SESSION=decide-$LABEL-${RUN_TMP##*-}
SESSION_OWNED=0
SESSION_TARGET="=$SESSION:"
cleanup_session() {
  local proc
  trap '' INT TERM
  if (( SESSION_OWNED )); then
    tmux kill-session -t "$SESSION_TARGET" 2>/dev/null || true
    SESSION_OWNED=0
  fi
  # As in E3, stop jobs still inside this invocation's private workspace.
  for proc in /proc/[0-9]*; do
    case "$(readlink "$proc/cwd" 2>/dev/null)" in
      "$WORK"|"$WORK"/*) kill -9 "${proc#/proc/}" 2>/dev/null || true ;;
    esac
  done
  cleanup
}
trap cleanup_session EXIT
if ! python3 - "$HERE/decide/scenarios.json" "$SCENARIO" "$WORK" "$SCRIPT" "$PROMPTS" <<'PY'
import json, os, sys
spec = json.load(open(sys.argv[1]))[sys.argv[2]]
for path, text in spec["files"].items():
    full = os.path.join(sys.argv[3], path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    open(full, "w").write(text)
json.dump(spec["turns"], open(sys.argv[4], "w"))
open(sys.argv[5], "w").write("\n".join(spec["prompts"]) + "\n")
PY
then exit 2; fi
( cd "$WORK"
  env -i PATH="$PATH" HOME="$HOMEDIR" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git init -q --template=
  env -i PATH="$PATH" HOME="$HOMEDIR" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git add -A
  env -i PATH="$PATH" HOME="$HOMEDIR" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 \
    git -c user.name=b -c user.email=b@b.invalid -c core.hooksPath=/dev/null -c commit.gpgsign=false commit -qm init )
READY=$RUN_TMP/stub.ready
STUB_LOG=$LOG STUB_PORT=0 STUB_READY=$READY STUB_SCRIPT=$SCRIPT STUB_PROMPTS=$PROMPTS python3 "$HERE/stub_model.py" 2>"$OUT/$LABEL.stub.err" &
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
ENVS=(PATH="$PATH" HOME="$HOMEDIR" TERM=xterm-256color LANG=C.UTF-8 GIT_CONFIG_GLOBAL=/dev/null TMPDIR="$RUN_TMP/tmp"
      ANGEL_WORKSPACE="$WORK" ANGEL_ACTION_CAPSULES=0 ANGEL_TUI_MOTION=off ANGEL_IMAGE_PROTOCOL=halfblocks
      ANGEL_DRIVER=local ANGEL_LOCAL_URL="http://127.0.0.1:$PORT/v1" ANGEL_LOCAL_MODEL=stub
      ANGEL_SOTA_CAVEMAN=0 ANGEL_TASK_RECON=0 ANGEL_YOLO=0 ANGEL_BOOK_INTRO=${ANGEL_BOOK_INTRO:-1})
printf -v RUN_COMMAND '%s 2>%q' "$(printf '%q ' env -i "${ENVS[@]}" "$BIN/angel")" "$OUT/$LABEL.stderr"
SESSION_OWNED=1
tmux new-session -d -s "$SESSION" -x 200 -y 55 -c "$WORK" "$RUN_COMMAND" || exit 2
say() { tmux send-keys -t "$SESSION_TARGET" -l "$1"; sleep 0.5; tmux send-keys -t "$SESSION_TARGET" Enter; }
# Idle: the stub's log has not grown for 6 s.
wait_idle() { local last=-1 n still=0; for ((i=0; i<150; i++)); do sleep 2; n=0
  if [[ -f "$LOG" ]]; then n=$(wc -l < "$LOG"); fi
  if [ "$n" -gt 0 ] && [ "$n" = "$last" ]; then still=$((still + 1)); [ $still -ge 3 ] && return; else still=0; fi; last=$n; done
  printf 'Scenario did not become idle; see %s\n' "$LOG" >&2; return 124; }
sleep 4
while IFS= read -r prompt; do say "$prompt"; sleep 2; wait_idle; done < "$PROMPTS"
REQUESTS=unknown
if [[ -f "$LOG" ]]; then REQUESTS=$(wc -l < "$LOG"); fi
echo "$LABEL requests=$REQUESTS"
