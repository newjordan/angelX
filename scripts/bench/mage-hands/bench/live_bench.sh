#!/usr/bin/env bash
# Live bench: every task x repeat on the private seat, each in its own isolated bus.
#   live_bench.sh <repeats> [task...]
set -u
REPS=${1:-3}; shift
TASKS=("$@"); [ ${#TASKS[@]} -gt 0 ] || TASKS=(calc-8008 calc-58008 calc-5318008 calc-hello writer-bold writer-iu zenity-list nautilus-open)
MAGE=${MAGE_ROOT:-$HOME/mage}  # the mage working folder: bench/, desk/, bin/, venv/
B=$MAGE/bench
read -r SOCK SIG < <(bash $B/seat.sh up) || exit 1
trap 'bash $B/seat.sh down' EXIT
for k in $(seq "$REPS"); do
  for t in "${TASKS[@]}"; do
    R=$B/${LIVE_DIR:-angel-live}/$t-r$k
    [ -s "$R/verdict.json" ] && continue
    rm -rf "$R"; mkdir -p "$R"
    env -u DISPLAY WAYLAND_DISPLAY="$SOCK" HYPRLAND_INSTANCE_SIGNATURE="$SIG" \
      dbus-run-session --config-file=$B/session-isolated.conf -- bash $B/live_task.sh "$t" "$R" "$SOCK" "$SIG" 2>>"$R/live.err" | tail -1
    # leave the seat empty for the next run
    for a in $(HYPRLAND_INSTANCE_SIGNATURE=$SIG hyprctl -j clients | python3 -I -c "import json,sys;print(' '.join(c['address'] for c in json.load(sys.stdin)))"); do
      HYPRLAND_INSTANCE_SIGNATURE=$SIG hyprctl eval "hl.dispatch(hl.dsp.window.close({ window = 'address:$a' }))" >/dev/null
    done
    sleep 2
  done
done
