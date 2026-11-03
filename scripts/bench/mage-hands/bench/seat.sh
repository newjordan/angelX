#!/usr/bin/env bash
# Private nested Hyprland seat for the bench (own output GLASSCAP / ws 67).
#   seat.sh up    -> prints "SOCK SIG" once the seat is drawing frames
#   seat.sh down  -> tears it down and gives focus back to the monitor that had it
set -u
MAGE=${MAGE_ROOT:-$HOME/mage}  # the mage working folder: bench/, desk/, bin/, venv/
STATE=$MAGE/bench/.seat-focus
case "${1:-}" in
up)
  hyprctl monitors -j | python3 -I -c "import json,sys;print(next(m['name'] for m in json.load(sys.stdin) if m['focused']))" > "$STATE"
  hyprctl eval 'hl.workspace_rule({ workspace = "67", monitor = "GLASSCAP", default = true, persistent = true })' >/dev/null
  hyprctl output create headless GLASSCAP >/dev/null; sleep 0.5
  hyprctl eval 'hl.monitor({ output = "GLASSCAP", mode = "2000x1160@60", position = "30000x3000", scale = 1 })' >/dev/null
  hyprctl eval 'hl.window_rule({ match = { class = "aquamarine" }, workspace = "67 silent", no_focus = true, tag = "-default-opacity", opacity = "1 override 1 override", float = true, size = "1920 1080", move = "0 0" })' >/dev/null
  sleep 1
  if ! hyprctl workspaces -j | python3 -I -c "import json,sys;sys.exit(any(w['monitor']=='GLASSCAP' and w['id']!=67 for w in json.load(sys.stdin)))"; then
    echo "a user workspace landed on GLASSCAP" >&2; "$0" down; exit 1
  fi
  # The nested compositor has crashed on its first frame once (SEGV in renderMonitor,
  # 2026-10-31 01:24); retry a failed start up to three times.
  SOCK=""; SIG=""
  for attempt in 1 2 3; do
    systemctl --user reset-failed mage-nested 2>/dev/null
    # NESTED_GPU=amd renders the seat on the AMD device so it does not compete with
    # the Mage container for the 3090's memory (the NVIDIA EGL driver SEGVs when it is short).
    GPUENV=()
    [ "${NESTED_GPU:-}" = amd ] && GPUENV=(-E AQ_DRM_DEVICES=/dev/dri/card0 -E __EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json)
    systemd-run --user --collect --unit=mage-nested -E WAYLAND_DISPLAY="$WAYLAND_DISPLAY" -E XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" "${GPUENV[@]}" \
      Hyprland --config $MAGE/desk/nested-1080.lua >/dev/null 2>&1
    sleep 4
    read -r SOCK SIG < <(hyprctl instances -j | python3 -I -c "
import json,sys
i=[x for x in json.load(sys.stdin) if x['pid']!=1246458]
print(i[0]['wl_socket'],i[0]['instance']) if i else print('')")
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
