#!/usr/bin/env bash
# Three repetitions of eight live tasks. Requires explicit selected-provider approval.
# Example AFTER approval: MAGE_ALLOW_MUSE=1 LIVE_DIR=angel-live-tuned bash live_bench.sh 3
# --preflight checks the binary/dependencies and local Mage inference, no Muse call.
set -euo pipefail
HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
. "$HERE/preflight.sh"
if [[ ${1:-} = --preflight ]]; then mage_preflight; exit $?; fi
REPS=${1:-3}
[[ $REPS =~ ^[1-9][0-9]*$ ]] || { echo 'repetitions must be a positive integer' >&2; exit 2; }
shift || true
if [[ $# -gt 0 ]]; then export LIVE_TASKS="$*"; fi
export LIVE_REPS=$REPS
export MAGE_BRAIN
export LIVE_DIR=${LIVE_DIR:-angel-live}
export LIVE_TASKS=${LIVE_TASKS:-calc-8008 calc-58008 calc-5318008 calc-hello writer-bold writer-iu zenity-list nautilus-open}
[[ $LIVE_DIR =~ ^[a-zA-Z0-9_-]+$ ]] || { echo 'LIVE_DIR must be a directory name, not a path' >&2; exit 2; }
mage_check_launch || exit 1
mkdir -p "$MAGE_BENCH_ROOT/$LIVE_DIR"
exec 9>"$MAGE_BENCH_ROOT/.live-bench.lock"
flock -n 9 || { echo 'another Mage live benchmark is running' >&2; exit 1; }
mage_preflight
# Score existing evidence once; skip only validated runs, never launcher failures.
python3 -I "$HERE/live_score.py" "$MAGE_BENCH_ROOT"
SCORE="$MAGE_BENCH_ROOT/live_analysis-$LIVE_DIR.json"
[[ $LIVE_DIR != angel-live ]] || SCORE="$MAGE_BENCH_ROOT/live_analysis.json"
VALID=$(python3 -I -c 'import json,sys;print("\n".join(r["run"] for r in json.load(open(sys.argv[1]))["runs"]))' "$SCORE")
SEAT_OWNED=0
cleanup() {
  if [[ $SEAT_OWNED = 1 ]]; then bash "$HERE/seat.sh" down || true; SEAT_OWNED=0; unset MAGE_SEAT_PID; fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
for k in $(seq "$REPS"); do
  for task in $LIVE_TASKS; do
    name="$task-r$k"
    if grep -Fxq "$name" <<< "$VALID"; then echo "keep validated run: $name"; continue; fi
    run="$MAGE_BENCH_ROOT/$LIVE_DIR/$name"
    if [[ -e $run ]]; then
      archive="$MAGE_BENCH_ROOT/$LIVE_DIR-infra"
      mkdir -p "$archive"
      mv -- "$run" "$archive/$name-attempt-$(date +%s%N)"
    fi
    seat=$(bash "$HERE/seat.sh" up)
    SEAT_OWNED=1
    read -r SOCK SIG <<< "$seat"
    export MAGE_SEAT_PID
    MAGE_SEAT_PID=$(systemctl --user show mage-nested.service --property=MainPID --value)
    mage_check_seat "$SOCK" "$SIG"
    # Each run has a private bus. The task and runner both clean up their own
    # resources through EXIT traps.
    task_rc=0
    mkdir -p "$run"
    env -u DISPLAY WAYLAND_DISPLAY="$SOCK" HYPRLAND_INSTANCE_SIGNATURE="$SIG" \
      dbus-run-session --config-file="$HERE/session-isolated.conf" -- \
      bash "$HERE/live_task.sh" "$task" "$run" "$SOCK" "$SIG" >"$run/live.out" 2>"$run/live.err" || task_rc=$?
    cleanup
    python3 -I "$HERE/live_score.py" "$MAGE_BENCH_ROOT"
    if [[ $task_rc != 0 ]] || ! python3 -I -c \
      'import json,sys;sys.exit(0 if any(r["run"] == sys.argv[2] for r in json.load(open(sys.argv[1]))["runs"]) else 1)' "$SCORE" "$name"; then
      echo "benchmark stopped: $name has no valid Mage run; evidence preserved in $run" >&2
      exit 1
    fi
  done
done
python3 -I "$HERE/live_score.py" "$MAGE_BENCH_ROOT" --require-complete
