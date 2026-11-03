#!/usr/bin/env bash
# Launch angelY seats as tabs of one Kitty window on the headless ANGELCAP workspace
# (66, no focus), then arm each seat's /loop on its standing orders.
#   seats.sh [seat...]       seats come from seats.conf (seat workspace driver MemoryMax)
#   seats.sh --no-loop ...   launch only
set -euo pipefail
KIT="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")" && pwd)"
ANGELY="$KIT/../../bin/angely"
SOCK=unix:/tmp/angelY.sock
arm=1; [[ "${1:-}" == --no-loop ]] && { arm=0; shift; }
mapfile -t rows < <(grep -vE '^\s*(#|$)' "$KIT/seats.conf")
want=("$@")
hyprctl eval 'hl.window_rule({ match = { class = "angelY" }, workspace = "66 silent", no_focus = true })' >/dev/null 2>&1 || true

launched=()
for row in "${rows[@]}"; do
  read -r seat ws driver mem <<<"$row"
  ws="${ws/#\~/$HOME}"
  if [[ ${#want[@]} -gt 0 ]] && ! printf '%s\n' "${want[@]}" | grep -qxF "$seat"; then continue; fi
  envs=(ANGELY_SEAT="$seat" ANGELY_MEM="$mem")
  if ! kitty @ --to "$SOCK" ls >/dev/null 2>&1; then
    kitty --detach --class angelY --title "angelY" -o allow_remote_control=socket-only --listen-on "$SOCK" \
      --directory "$ws" env "${envs[@]}" "$ANGELY" "$ws" "$driver"
    for _ in $(seq 1 50); do kitty @ --to "$SOCK" ls >/dev/null 2>&1 && break; sleep 0.2; done
  else
    kitty @ --to "$SOCK" launch --type=tab --tab-title "$seat" --cwd "$ws" \
      --env "ANGELY_SEAT=$seat" --env "ANGELY_MEM=$mem" "$ANGELY" "$ws" "$driver" >/dev/null
  fi
  kitty @ --to "$SOCK" set-tab-title --match "env:ANGELY_SEAT=$seat" "$seat" 2>/dev/null || true
  echo "launched $seat ($driver) in $ws"
  launched+=("$seat")
done
[[ $arm == 1 ]] || exit 0

TASK='Read .angelY/ORDERS.md in this workspace and follow it as your standing orders.'
for seat in "${launched[@]}"; do
  # A fresh cockpit draws its composer before it accepts commands; typed too early,
  # /loop never arms and the seat runs one plain turn.
  sleep 20
  m="env:ANGELY_SEAT=$seat"
  kitty @ --to "$SOCK" send-text --match "$m" "/loop now $TASK"
  kitty @ --to "$SOCK" send-key --match "$m" enter
  sleep 3
  kitty @ --to "$SOCK" send-text --match "$m" "/loop endless"
  kitty @ --to "$SOCK" send-key --match "$m" enter
  ok=""
  for _ in $(seq 1 30); do
    "$KIT/screen.sh" "$seat" | grep -qE 'loop started|loop is already driving|loop · iteration' && { ok=1; break; }
    sleep 1
  done
  [[ -n "$ok" ]] && echo "loop armed: $seat" || echo "WARNING: no loop confirmation on $seat"
done
