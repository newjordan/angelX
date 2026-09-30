#!/usr/bin/env bash
# e3_session.sh <bin-dir> <label> — one live /loop session in the TUI, driven
# through tmux, against the seat named by ROUTE (see seat.sh) behind
# proxy_log.py. Arm: ANGEL_BOOK_INTRO=0|1. The workspace is a copy of the
# Yukon heesch benchmark (HEESCH, default ~/comps/heesch, set up), with no git
# remote and no Yukon login; the verifier (e3/verify_heesch.py) is green only
# for hc_verified >= 5, a target that stays red, so the loop stalls and raises
# its routes. Nothing is submitted.
#   ITERS (default 6)       the loop's iteration cap (/loop now iters=N)
#   STALL_STOP (default 1)  ANGEL_LOOP_STALL_STOP: stalls before a pivot
#   LIMIT (default 7200)    seconds before the driver stops the loop
#   WORKROOT (default /work/tmp)  where workspaces, homes and TMPDIR live
# Out: out/<label>.jsonl (requests), .stderr, .screen (last frame),
# .state/ (loop and learner state), .run (one summary line).
set -u
if [[ $# != 2 || ! "$2" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]]; then
  printf 'Usage: %s <bin-dir> <label> (alphanumeric, dot, underscore or dash)\n' "$0" >&2
  exit 2
fi
BIN_DIR=$(cd "$1" && pwd) || exit 2; LABEL=$2
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/seat.sh"
OUT=$HERE/out; mkdir -p "$OUT"
LOG=$OUT/$LABEL.jsonl; rm -rf "$LOG" "$OUT/$LABEL.state"
ROOT=${WORKROOT:-/work/tmp}
RUN_TMP=$(mktemp -d "$ROOT/angel-e3-run-XXXXXX") || exit 2
. "$HERE/lifecycle.sh"
WORK=$RUN_TMP/work; HOMEDIR=$RUN_TMP/home
mkdir -p "$WORK" "$HOMEDIR" "$RUN_TMP/tmp" || exit 2
SESSION=e3-$LABEL-${RUN_TMP##*-}
SESSION_OWNED=0
SESSION_TARGET="=$SESSION:"
stop_workspace_jobs() {
  local proc
  # Work is a private snapshot: only processes still using this invocation's
  # directory are candidates for its workspace cleanup.
  for proc in /proc/[0-9]*; do
    case "$(readlink "$proc/cwd" 2>/dev/null)" in
      "$WORK"|"$WORK"/*) kill -9 "${proc#/proc/}" 2>/dev/null || true ;;
    esac
  done
}
cleanup_session() {
  trap '' INT TERM
  if (( SESSION_OWNED )); then
    tmux kill-session -t "$SESSION_TARGET" 2>/dev/null || true
    SESSION_OWNED=0
  fi
  stop_workspace_jobs
  cleanup
}
trap cleanup_session EXIT
python3 "$HERE/exec_session.py" python3 "$HERE/copy_fixture.py" \
  "${HEESCH:-$HOME/comps/heesch}" "$WORK" &
TASK=$!
if wait "$TASK"; then
  TASK=
else
  stop_task
  exit 2
fi
rm -f "$WORK/score.json"
start_proxy "$LOG" || exit 2
ENVS=(PATH="$PATH" HOME="$HOMEDIR" TERM=xterm-256color LANG=C.UTF-8 GIT_CONFIG_GLOBAL=/dev/null TMPDIR="$RUN_TMP/tmp"
      ANGEL_WORKSPACE="$WORK" ANGEL_BOOK_INTRO="${ANGEL_BOOK_INTRO:-1}" ANGEL_SOTA_CAVEMAN=0
      ANGEL_ACTION_CAPSULES=0 ANGEL_TUI_MOTION=off ANGEL_IMAGE_PROTOCOL=halfblocks
      ANGEL_LOOP_STALL_STOP="${STALL_STOP:-1}" "${SEAT[@]}")
printf '%q ' env -i "${ENVS[@]}" "$BIN_DIR/angel" > "$OUT/$LABEL.cmd"
printf -v RUN_COMMAND '%s 2>%q' "$(cat "$OUT/$LABEL.cmd")" "$OUT/$LABEL.stderr"
SESSION_OWNED=1
tmux new-session -d -s "$SESSION" -x 200 -y 55 -c "$WORK" \
  "$RUN_COMMAND" || exit 2
say() { tmux send-keys -t "$SESSION_TARGET" -l "$1"; sleep 0.5; tmux send-keys -t "$SESSION_TARGET" Enter; sleep 2; }
sleep 4
say "/goal submission/best.heesch is an unmarked polyform whose witness patch verifies Hc >= 5 under heesch_verify (the first survivor)"
say "/goal cmd python3 $HERE/e3/verify_heesch.py"
say "/loop now iters=${ITERS:-6} Work the Heesch challenge in this repository (README.md, AGENTS.md, docs/submitting.md; the venv is .venv-bench). submission/best.heesch holds the current best, a 15-cell polyhex with Hc = 4, the class record. Goal: a shape whose witness patch verifies Hc >= 5 under .venv-bench/bin/python -m heesch_verify submission/best.heesch. One direction worth testing: design candidates top-down, as slices of a higher-dimensional periodic structure (cut-and-project), choosing a defective slice whose consistent window closes after five coronas; the witness is still a 2D patch. Do not submit to Yukon or post to its Discussions."
START=$(date +%s)
STATUS=
while :; do
  sleep 20
  STATUS=$(python3 - "$HOMEDIR" <<'PY'
import glob, json, sys
for path in glob.glob(sys.argv[1] + "/.angelX/loops/*.json"):
    try:
        print(str(json.load(open(path)).get("status", "")).lower())
    except (OSError, ValueError):
        pass
PY
)
  case $STATUS in *done*|*stopped*|*failed*|*paused*) break ;; esac
  tmux has-session -t "$SESSION_TARGET" 2>/dev/null || { STATUS=exited; break; }
  if [ $(( $(date +%s) - START )) -ge "${LIMIT:-7200}" ]; then say "/loop stop"; STATUS="limit:$STATUS"; break; fi
done
sleep 3
tmux capture-pane -p -t "$SESSION_TARGET" > "$OUT/$LABEL.screen" 2>/dev/null
tmux kill-session -t "$SESSION_TARGET" 2>/dev/null
sleep 1
mkdir -p "$OUT/$LABEL.state"
cp -r "$HOMEDIR/.angelX/loops" "$OUT/$LABEL.state/" 2>/dev/null
find "$HOMEDIR" "$WORK" -path '*learning*' -name state.json 2>/dev/null | while read -r f; do
  cp "$f" "$OUT/$LABEL.state/learning-$(basename "$(dirname "$f")" | cut -c1-12).json"; done
cp -r "$WORK/submission" "$OUT/$LABEL.state/submission" 2>/dev/null
git -C "$WORK" status --short > "$OUT/$LABEL.state/git-status.txt" 2>/dev/null
stop_workspace_jobs
stop_group "$PROXY"; PROXY=
REQUESTS=unknown
[[ ! -f "$LOG" ]] || REQUESTS=$(wc -l < "$LOG")
echo "$LABEL status=$STATUS secs=$(( $(date +%s) - START )) requests=$REQUESTS" | tee "$OUT/$LABEL.run"
