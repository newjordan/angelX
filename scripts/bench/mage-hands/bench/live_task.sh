#!/usr/bin/env bash
# One live bench run, inside an isolated private bus (session-isolated.conf):
# a11y bus + registry + shim, the task's app on the nested seat, a headless
# angel turn (Muse Wonder brain, Mage hands via `ground`, sealed sandbox),
# then an independent verifier. Writes $R/verdict.json.
#   live_task.sh <task> <run-dir> <seat-socket> <seat-signature>
set -u
TASK=$1; R=$2; SOCK=$3; SIG=$4
MAGE=${MAGE_ROOT:-$HOME/mage}  # the mage working folder: bench/, desk/, bin/, venv/
BIN=${ANGEL_BIN:?set ANGEL_BIN to the angel binary to bench (cockpit/target/fast/angel)}
mkdir -p "$R"/{work,sessions,home,tmp}
exec 3>"$R/a11y.addr"
dbus-daemon --config-file=/usr/share/defaults/at-spi2/accessibility.conf --nofork --print-address=3 & BUS=$!
for _ in $(seq 50); do [ -s "$R/a11y.addr" ] && break; sleep 0.1; done
export AT_SPI_BUS_ADDRESS=$(head -1 "$R/a11y.addr")
/usr/lib/at-spi2-registryd 2>/dev/null & REG=$!
/usr/bin/python3 -I $MAGE/desk/a11y_shim.py 2>/dev/null & SHIM=$!
sleep 1
APPENV=(env -i PATH=/usr/bin:/bin HOME="$R/home" LANG=en_US.UTF-8 XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR"
  WAYLAND_DISPLAY="$SOCK" DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" AT_SPI_BUS_ADDRESS="$AT_SPI_BUS_ADDRESS"
  XDG_CONFIG_HOME="$R/home/.config" XDG_CACHE_HOME="$R/home/.cache" GDK_BACKEND=wayland QT_QPA_PLATFORM=wayland
  QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 SAL_USE_VCLPLUGIN=gtk3 GTK_A11Y=atspi GTK_THEME=Adwaita:dark)
mkdir -p "$R/home/Projects/knights-wish/art" "$R/home/Projects/angel" "$R/home/Documents" "$R/home/Pictures"
USE_GROUND="Use the desktop tool: screenshot to look, ground to locate each control you need, click at the coordinates ground returns, and screenshot to verify. Work on the current screen; do not start the agent display."
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
for _ in $(seq 60); do
  sleep 0.5
  HYPRLAND_INSTANCE_SIGNATURE=$SIG hyprctl -j clients | python3 -I -c "import json,sys;sys.exit(0 if json.load(sys.stdin) else 1)" && break
done
sleep 4
WAYLAND_DISPLAY=$SOCK grim "$R/before.png"

set -a; . "$HOME/.angelX/angel.env"; set +a
T0=$(date +%s.%N)
( cd "$R/work" && timeout 600 env PATH=$MAGE/seatbin:$HOME/.local/bin:$HOME/.cargo/bin \
    HYPRLAND_INSTANCE_SIGNATURE="$SIG" WAYLAND_DISPLAY="$SOCK" TMPDIR="$R/tmp" \
    ANGEL_DRIVER=meta ANGEL_META_MODEL=muse-wonder-1.0 ANGEL_YOLO_SMART=1 ANGEL_HANDS=1 \
    ANGEL_GROUND_URL=http://127.0.0.1:30001 \
    ANGEL_VISION_URL=https://api.meta.ai/v1 ANGEL_VISION_MODEL=muse-wonder-1.0 ANGEL_VISION_KEY="$ANGEL_META_KEY" \
    ANGEL_SESSION_DIR="$R/sessions" ANGEL_MEMORY_FILE="$R/memory.json" ANGEL_GOAL_FILE="$R/goal.json" \
    ANGEL_LOOP_FILE="$R/loop.json" ANGEL_SCAN=0 ANGEL_LSP=0 ANGEL_TURN_DEADLINE_SECS=540 \
    "$BIN" --task-json --sandbox-profile sealed --workspace "$R/work" "$PROMPT" > "$R/task.json" 2> "$R/task.err" )
ANGEL_RC=$?
T1=$(date +%s.%N)
WAYLAND_DISPLAY=$SOCK grim "$R/after.png"

# Independent verifier.
OBS=""; PASS=false
case "$TASK" in
  calc-*)
    WAYLAND_DISPLAY=$SOCK wtype -M ctrl c -m ctrl; sleep 0.5
    OBS=$(WAYLAND_DISPLAY=$SOCK timeout 3 wl-paste -n 2>/dev/null)
    [ "$OBS" = "$WANT" ] && PASS=true ;;
  writer-*)
    OBS=$(/usr/bin/python3 -I $MAGE/bench/atspi_dump.py soffice 2>/dev/null | grep -E "toggle button \| '(Bold|Italic|Underline)'|paragraph" | tr -s ' ' | tr '\n' ';')
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
kill $APP 2>/dev/null; pkill -P $APP 2>/dev/null
for p in $(pgrep -f "[a]ngel-live/.*/lo"); do kill "$p" 2>/dev/null; done
sleep 2
kill $SHIM $REG $BUS 2>/dev/null
echo "$TASK pass=$PASS observed=[$OBS] angel_rc=$ANGEL_RC"
