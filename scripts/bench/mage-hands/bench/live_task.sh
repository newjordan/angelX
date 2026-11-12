#!/usr/bin/env bash
# One live bench run, inside an isolated private bus (session-isolated.conf):
# a11y bus + registry + shim, the task's app on the nested seat, a headless
# angel turn (explicitly selected brain, Mage hands via `ground`, sealed sandbox),
# then an independent verifier. Writes $R/verdict.json.
#   live_task.sh <task> <run-dir> <seat-socket> <seat-signature>
set -uo pipefail
HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
. "$HERE/preflight.sh"
mage_check_launch || exit 1
TASK=$1; R=$2; SOCK=$3; SIG=$4
mage_check_seat "$SOCK" "$SIG" || exit 1
[[ $R = /* ]] || { echo 'run directory must be absolute' >&2; exit 2; }
[[ -f $HOME/.angelX/angel.env ]] || { echo 'missing provider configuration' >&2; exit 1; }
set -a; . "$HOME/.angelX/angel.env"; set +a
case "$MAGE_BRAIN" in
  muse-wonder-1.0) [[ -n ${ANGEL_META_KEY:-} ]] || { echo 'missing ANGEL_META_KEY' >&2; exit 1; } ;;
  grok-4.7) [[ -s $HOME/.grok/auth.json ]] || {
    echo 'missing native Grok OAuth credentials; no fallback' >&2; exit 1;
  } ;;
esac
if [[ $MAGE_BRAIN = grok-4.7 && -z ${MAGE_GROK_VISION_URL:-} ]]; then
  echo 'missing explicitly pinned Grok 4.7 image endpoint; no fallback' >&2; exit 1
fi
mage_brain_env || exit 1
APP=""; SHIM=""; REG=""; BUS=""
cleanup() {
  # Only processes launched by this run; no broad pgrep of other benchmark runs.
  if [[ -n $APP ]]; then pkill -P "$APP" 2>/dev/null || true; kill "$APP" 2>/dev/null || true; fi
  for p in "$SHIM" "$REG" "$BUS"; do [[ -z $p ]] || kill "$p" 2>/dev/null || true; done
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -p "$R"/{work,sessions,home,tmp}
exec 3>"$R/a11y.addr"
dbus-daemon --config-file=/usr/share/defaults/at-spi2/accessibility.conf --nofork --print-address=3 & BUS=$!
for _ in $(seq 50); do [ -s "$R/a11y.addr" ] && break; sleep 0.1; done
export AT_SPI_BUS_ADDRESS=$(head -1 "$R/a11y.addr")
/usr/lib/at-spi2-registryd 2>/dev/null & REG=$!
/usr/bin/python3 -I "$HERE/a11y_shim.py" 2>/dev/null & SHIM=$!
sleep 1
APPENV=(env -i PATH=/usr/bin:/bin HOME="$R/home" LANG=en_US.UTF-8 XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR"
  WAYLAND_DISPLAY="$SOCK" DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" AT_SPI_BUS_ADDRESS="$AT_SPI_BUS_ADDRESS"
  XDG_CONFIG_HOME="$R/home/.config" XDG_CACHE_HOME="$R/home/.cache" GDK_BACKEND=wayland QT_QPA_PLATFORM=wayland
  QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 SAL_USE_VCLPLUGIN=gtk3 GTK_A11Y=atspi GTK_THEME=Adwaita:dark
  QT_QUICK_BACKEND=software GSK_RENDERER=cairo SAL_SKIA=raster)
mkdir -p "$R/home/Projects/knights-wish/art" "$R/home/Projects/angel" "$R/home/Documents" "$R/home/Pictures"
USE_GROUND="Use the desktop tool: screenshot to look, ground to locate each control you need, click at the coordinates ground returns, and screenshot to verify. Work on the current screen; do not start the agent display."
if [[ $MAGE_BRAIN = grok-4.7 ]]; then
  USE_GROUND+=" Screenshot returns a saved image path, not image pixels in your context: call vision_look on that path to inspect it. That tool is explicitly pinned to the same Grok 4.7 brain. Use only desktop, vision_look, and tool_search to discover those exact tool names. Do not search for video or editing tools. For every click, use the coordinates returned by desktop ground, never guessed coordinates. You make all decisions; Mage only locates the control you request."
fi
NOKEYS="Use only the calculator's on-screen buttons: click them, do not type on the keyboard."
case "$TASK" in
  calc-*)
    "${APPENV[@]}" omacalc > "$R/app.out" 2>&1 & APP=$!
    case "$TASK" in
      calc-8008) EXPR="1001 × 8"; WANT=8008 ;;
      calc-58008) EXPR="7251 × 8"; WANT=58008 ;;
      calc-5318008) EXPR="2659004 × 2"; WANT=5318008 ;;
      calc-hello) EXPR="7734 ÷ 10000"; WANT=0.7734 ;;
    esac
    PROMPT="Omacalc, a calculator, is open on the current screen (a private test display). Compute $EXPR and press equals. $NOKEYS $USE_GROUND" ;;
  writer-bold|writer-iu)
    "${APPENV[@]}" libreoffice -env:UserInstallation=file://"$R"/lo --norestore --nologo --writer > "$R/app.out" 2>&1 & APP=$!
    if [ "$TASK" = writer-bold ]; then
      PROMPT="LibreOffice Writer is open and focused on the current screen (a private test display). Type the line \"Knights Wish\", select all of it with ctrl+a, then make it bold by clicking the Bold button on the formatting toolbar. $USE_GROUND"
    else
      PROMPT="LibreOffice Writer is open and focused on the current screen (a private test display). Type the line \"Knights Wish\", select all of it with ctrl+a, then make it italic and underlined by clicking the Italic and Underline buttons on the formatting toolbar. $USE_GROUND"
    fi ;;
  zenity-list)
    "${APPENV[@]}" zenity --list --checklist --title=Wishes --column=Grant --column=Wish --column=Cost \
      FALSE "Purple armour" 3 TRUE "Dragon steed" 9 FALSE "Lantern of truth" 5 FALSE "Bell of the keep" 2 > "$R/app.out" 2>&1 & APP=$!
    PROMPT="A dialog titled Wishes is open on the current screen (a private test display). Make sure exactly two wishes are ticked, Lantern of truth and Bell of the keep, with every other wish unticked, then click OK. $USE_GROUND" ;;
  nautilus-open)
    "${APPENV[@]}" nautilus --new-window "$R/home" > "$R/app.out" 2>&1 & APP=$!
    PROMPT="The Files app is open on the current screen (a private test display), showing a home folder. Open the Projects folder, then open the knights-wish folder inside it. $USE_GROUND" ;;
  *) echo "unknown task $TASK" >&2; exit 2 ;;
esac
READY=0
for _ in $(seq 60); do
  sleep 0.5
  if HYPRLAND_INSTANCE_SIGNATURE=$SIG hyprctl -j clients | python3 -I -c "import json,sys;sys.exit(0 if json.load(sys.stdin) else 1)"; then READY=1; break; fi
done
[[ $READY = 1 ]] || { echo 'task app never opened on the private seat' >&2; exit 1; }
sleep 4
mage_check_seat "$SOCK" "$SIG" || exit 1
WAYLAND_DISPLAY=$SOCK grim "$R/before.png" || exit 1

T0=$(date +%s.%N)
( cd "$R/work" && env "${BRAIN_UNSET[@]}" PATH=$MAGE_ROOT/seatbin:$HOME/.local/bin:$HOME/.cargo/bin \
    HYPRLAND_INSTANCE_SIGNATURE="$SIG" WAYLAND_DISPLAY="$SOCK" TMPDIR="$R/tmp" \
    ANGEL_YOLO_SMART=1 "${BRAIN_ENV[@]}" ANGEL_HANDS=1 \
    ANGEL_GROUND_URL="${ANGEL_GROUND_URL:-http://127.0.0.1:30001}" \
    ANGEL_SESSION_DIR="$R/sessions" ANGEL_MEMORY_FILE="$R/memory.json" ANGEL_GOAL_FILE="$R/goal.json" \
    ANGEL_LOOP_FILE="$R/loop.json" ANGEL_SCAN=0 ANGEL_LSP=0 ANGEL_TURN_DEADLINE_SECS=0 ANGEL_MAX_HOPS=0 \
    "$BIN" --task-json --sandbox-profile sealed --max-hops 0 --deadline-secs 0 --workspace "$R/work" "$PROMPT" > "$R/task.json" 2> "$R/task.err" )
ANGEL_RC=$?
T1=$(date +%s.%N)
mage_check_seat "$SOCK" "$SIG" || exit 1
WAYLAND_DISPLAY=$SOCK grim "$R/after.png" || exit 1

# Independent verifier.
OBS=""; PASS=false
case "$TASK" in
  calc-*)
    WAYLAND_DISPLAY=$SOCK wtype -M ctrl c -m ctrl; sleep 0.5
    OBS=$(WAYLAND_DISPLAY=$SOCK timeout 3 wl-paste -n 2>/dev/null)
    [ "$OBS" = "$WANT" ] && PASS=true ;;
  writer-*)
    OBS=$(/usr/bin/python3 -I "$HERE/atspi_dump.py" soffice 2>/dev/null | grep -E "toggle button \| '(Bold|Italic|Underline)'|paragraph" | tr -s ' ' | tr '\n' ';')
    if [ "$TASK" = writer-bold ]; then
      echo "$OBS" | grep -qE "toggle button \| 'Bold' \| text='' \| \[[^]]*'checked'" && echo "$OBS" | grep -q "Knights Wish" && PASS=true
    else
      echo "$OBS" | grep -qE "'Italic' \| text='' \| \[[^]]*'checked'" && echo "$OBS" | grep -qE "'Underline' \| text='' \| \[[^]]*'checked'" && echo "$OBS" | grep -q "Knights Wish" && PASS=true
    fi ;;
  zenity-list)
    sleep 1; OBS=$(head -c 200 "$R/app.out" | tr -d '\n')
    [ "$OBS" = "Lantern of truth|Bell of the keep" ] && PASS=true ;;
  nautilus-open)
    OBS=$(HYPRLAND_INSTANCE_SIGNATURE=$SIG hyprctl -j clients | python3 -I -c "import json,sys;print('|'.join(c['title'] for c in json.load(sys.stdin)))")
    echo "$OBS" | grep -q "knights-wish" && PASS=true ;;
esac
python3 -I -c "
import json,sys
json.dump({'task':sys.argv[1],'pass':sys.argv[2]=='true','observed':sys.argv[3],'angel_rc':int(sys.argv[4]),
           'wall_s':round(float(sys.argv[6])-float(sys.argv[5]),1)},open(sys.argv[7],'w'))" \
  "$TASK" "$PASS" "$OBS" "$ANGEL_RC" "$T0" "$T1" "$R/verdict.json"
echo "$TASK pass=$PASS observed=[$OBS] angel_rc=$ANGEL_RC"
