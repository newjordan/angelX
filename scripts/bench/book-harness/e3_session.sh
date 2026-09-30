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
BIN_DIR=$(cd "$1" && pwd); LABEL=$2
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/seat.sh"
OUT=$HERE/out; mkdir -p "$OUT"
LOG=$OUT/$LABEL.jsonl; rm -rf "$LOG" "$OUT/$LABEL.state"
ROOT=${WORKROOT:-/work/tmp}
WORK=$(mktemp -d "$ROOT/angel-e3-ws-XXXX"); HOMEDIR=$(mktemp -d "$ROOT/angel-e3-home-XXXX")
cp -a "${HEESCH:-$HOME/comps/heesch}/." "$WORK/"
rm -f "$WORK/score.json"
git -C "$WORK" remote remove origin 2>/dev/null
start_proxy "$LOG" || exit 2
SESSION=e3-$LABEL
ENVS=(PATH="$PATH" HOME="$HOMEDIR" TERM=xterm-256color LANG=C.UTF-8 GIT_CONFIG_GLOBAL=/dev/null TMPDIR="$ROOT"
      ANGEL_WORKSPACE="$WORK" ANGEL_BOOK_INTRO="${ANGEL_BOOK_INTRO:-1}" ANGEL_SOTA_CAVEMAN=0
      ANGEL_ACTION_CAPSULES=0 ANGEL_TUI_MOTION=off ANGEL_IMAGE_PROTOCOL=halfblocks
      ANGEL_LOOP_STALL_STOP="${STALL_STOP:-1}" "${SEAT[@]}")
printf '%q ' env -i "${ENVS[@]}" "$BIN_DIR/angel" > "$OUT/$LABEL.cmd"
tmux new-session -d -s "$SESSION" -x 200 -y 55 -c "$WORK" \
  "$(cat "$OUT/$LABEL.cmd") 2>'$OUT/$LABEL.stderr'"
say() { tmux send-keys -t "$SESSION" -l "$1"; sleep 0.5; tmux send-keys -t "$SESSION" Enter; sleep 2; }
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
  tmux has-session -t "$SESSION" 2>/dev/null || { STATUS=exited; break; }
  if [ $(( $(date +%s) - START )) -ge "${LIMIT:-7200}" ]; then say "/loop stop"; STATUS="limit:$STATUS"; break; fi
done
sleep 3
tmux capture-pane -p -t "$SESSION" > "$OUT/$LABEL.screen" 2>/dev/null
tmux kill-session -t "$SESSION" 2>/dev/null
sleep 1
mkdir -p "$OUT/$LABEL.state"
cp -r "$HOMEDIR/.angelX/loops" "$OUT/$LABEL.state/" 2>/dev/null
find "$HOMEDIR" "$WORK" -path '*learning*' -name state.json 2>/dev/null | while read -r f; do
  cp "$f" "$OUT/$LABEL.state/learning-$(basename "$(dirname "$f")" | cut -c1-12).json"; done
cp -r "$WORK/submission" "$OUT/$LABEL.state/submission" 2>/dev/null
git -C "$WORK" status --short > "$OUT/$LABEL.state/git-status.txt" 2>/dev/null
# Jobs the model started (solvers under angel-sandbox) outlive the TUI; stop
# every process still working inside this session's workspace.
for proc in /proc/[0-9]*; do
  case "$(readlink "$proc/cwd" 2>/dev/null)" in "$WORK"|"$WORK"/*) kill -9 "${proc#/proc/}" 2>/dev/null ;; esac
done
kill $PROXY 2>/dev/null; wait $PROXY 2>/dev/null
echo "$LABEL status=$STATUS secs=$(( $(date +%s) - START )) requests=$(wc -l < "$LOG" 2>/dev/null || echo 0)" | tee "$OUT/$LABEL.run"
rm -rf "$WORK" "$HOMEDIR"
