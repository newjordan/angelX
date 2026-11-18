#!/usr/bin/env bash
set -uo pipefail
HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
MAGE=${MAGE_ROOT:-$HOME/mage}  # the mage working folder: bench/, train/, seatbin/, venv/
STATE=$MAGE/bench/.seat-focus
case "${1:-}" in
up)
  if systemctl --user is-active --quiet mage-nested.service; then
    echo 'a Mage seat is already active; refusing to replace it' >&2; exit 1
  fi
  rm -f $MAGE/bench/.ready.png
  hyprctl monitors -j | python3 -I -c "import json,sys;print(next(m['name'] for m in json.load(sys.stdin) if m['focused']))" > "$STATE"
  hyprctl eval 'hl.workspace_rule({ workspace = "67", monitor = "GLASSCAP", default = true, persistent = true })' >/dev/null
  hyprctl output create headless GLASSCAP >/dev/null; sleep 0.5
  hyprctl eval 'hl.monitor({ output = "GLASSCAP", mode = "2000x1160@60", position = "30000x3000", scale = 1 })' >/dev/null
  hyprctl eval 'hl.window_rule({ match = { class = "aquamarine" }, workspace = "67 silent", no_focus = true, tag = "-default-opacity", opacity = "1 override 1 override", float = true, size = "1920 1080", move = "0 0" })' >/dev/null
  sleep 1
  if ! hyprctl workspaces -j | python3 -I -c "import json,sys;sys.exit(any(w['monitor']=='GLASSCAP' and w['id']!=67 for w in json.load(sys.stdin)))"; then
    echo "a user workspace landed on GLASSCAP" >&2; "$0" down; exit 1
  fi
  gpu_free_mib() { nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits 2>/dev/null | head -1 | tr -dc '0-9'; }
  gpu_refusing()  { journalctl -k --since "12 seconds ago" --no-pager 2>/dev/null | grep -q kchangrpapiConstruct; }
  gpu_wait() {
    command -v nvidia-smi >/dev/null 2>&1 || return 0   # no NVIDIA here, nothing to gate on
    local need=${MIN_FREE_MIB:-1200} free warned=0 i
    for i in $(seq "${GPU_WAIT_TRIES:-30}"); do
      free=$(gpu_free_mib); free=${free:-0}
      [ "$free" -ge "$need" ] && ! gpu_refusing && return 0
      [ "$warned" = 0 ] && { echo "seat: waiting for the 3090 (free=${free}MiB need=${need}MiB, channel allocs settling)" >&2; warned=1; }
      sleep 2
    done
    free=$(gpu_free_mib); free=${free:-0}
    echo "seat: 3090 not ready (free=${free}MiB need=${need}MiB)." >&2
    echo "seat: lower MEM_FRACTION_STATIC in the serve script, or close some GPU clients, then retry." >&2
    return 1
  }

  SOCK=""; SIG=""
  for attempt in 1 2 3; do
    gpu_wait || { "$0" down; exit 1; }
    systemctl --user reset-failed mage-nested 2>/dev/null
    systemd-run --user --collect --unit=mage-nested -E WAYLAND_DISPLAY="$WAYLAND_DISPLAY" -E XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
      Hyprland --config "$HERE/nested-1080.lua" >/dev/null 2>&1
    sleep 4
    PID=$(systemctl --user show mage-nested.service --property=MainPID --value)
    read -r SOCK SIG < <(hyprctl instances -j | python3 -I -c '
import json,sys
pid=int(sys.argv[1] or "0")
i=[x for x in json.load(sys.stdin) if x["pid"] == pid and pid > 0]
print(i[0]["wl_socket"],i[0]["instance"]) if len(i) == 1 else print("")
' "$PID")
    [ -n "$SIG" ] && break
    echo "nested seat start $attempt failed; retrying" >&2
    systemctl --user stop mage-nested 2>/dev/null
    sleep 2
  done
  [ -n "$SIG" ] || { echo "nested seat would not start" >&2; "$0" down; exit 1; }
  for i in $(seq 20); do
    WAYLAND_DISPLAY=$SOCK timeout 5 grim $MAGE/bench/.ready.png 2>/dev/null && break
    sleep 2
  done
  [ -s $MAGE/bench/.ready.png ] || { echo "seat never drew a frame" >&2; "$0" down; exit 1; }
  rm -f $MAGE/bench/.ready.png
  echo "$SOCK $SIG"
  ;;
down)
  systemctl --user stop mage-nested 2>/dev/null
  sleep 1
  hyprctl eval 'hl.workspace_rule({ workspace = "67", monitor = "GLASSCAP", persistent = false })' >/dev/null
  hyprctl output remove GLASSCAP >/dev/null 2>&1
  sleep 1
  [ -s "$STATE" ] && hyprctl eval "hl.dispatch(hl.dsp.focus({ monitor = '$(cat "$STATE")' }))" >/dev/null
  ;;
*) echo "usage: seat.sh up|down" >&2; exit 2 ;;
esac
