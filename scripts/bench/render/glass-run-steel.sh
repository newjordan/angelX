#!/usr/bin/env bash
# Hermetic glass cockpit run on the GLASSCAP headless output.
# Vendored from the glass test kit's glass-run.sh with one change:
# KITTY_CONF may be overridden from the environment, so the render
# scorecard can race a steelmanned kitty (kitty-steel.conf).
# usage: glass-run.sh <binary> <name> [extra env assignments...]
# KITTY=1 runs the terminal cockpit inside Kitty instead (same environment).
set -u
BIN=$1; NAME=$2; shift 2
S=${GLASS_KIT_RUNS:-/work/tmp/glass-runs}
R=$S/$NAME; rm -rf "$R"; mkdir -p "$R"/{home,config,cache,state,work,sessions,work-context}
# The user's look, read-only: Kitty config + theme + fonts.
mkdir -p "$R/home/.local/state" "$R/home/.local/share" "$R/config"
ln -s ~/.local/state/omarchy "$R/home/.local/state/omarchy"
ln -s ~/.local/share/fonts "$R/home/.local/share/fonts"
ln -s ~/.config/kitty "$R/config/kitty"
cd "$R/work"
KITTY_CONF=${KITTY_CONF:-~/.config/kitty/kitty.conf}
env -i PATH="$PATH" KITTY="${KITTY:-}" KITTY_BG="${KITTY_BG:-}" KITTY_CONF="$KITTY_CONF" TERM=xterm-kitty LANG=C.UTF-8 LC_ALL=C.UTF-8 \
  WAYLAND_DISPLAY="$WAYLAND_DISPLAY" XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
  HOME="$R/home" XDG_CONFIG_HOME="$R/config" XDG_CACHE_HOME="$R/cache" XDG_STATE_HOME="$R/state" \
  ANGEL_WORKSPACE="$R/work" ANGEL_SESSION_DIR="$R/sessions" ANGEL_MEMORY_FILE="$R/memory.json" \
  ANGEL_GOAL_FILE="$R/goal.json" ANGEL_LOOP_FILE="$R/loop.json" ANGEL_WORK_CONTEXT_DIR="$R/work-context" \
  ANGEL_DRIVER=practice ANGEL_SCAN=0 ANGEL_SCAN_INTERVAL_SECS=0 ANGEL_BAG_PROBE=0 \
  ANGEL_HYDRA_DISCOVER=0 ANGEL_HYDRA_PUBLISH=0 ANGEL_LSP=0 ANGEL_EXPERIENCE=0 ANGEL_DOSSIER=0 \
  ANGEL_ROUTE_MEMORY=0 ANGEL_OVERWATCH_CMD=true \
  ANGEL_GLASS_APP_ID=angelcap-glass "$@" \
  bash -c 'if [ "${KITTY:-}" = 1 ]; then
      exec kitty --class angelcap-glass --config "$KITTY_CONF" $([ "${KITTY_BG:-}" = 1 ] || echo -o background_image=none) \
        -o cursor_blink_interval=0 -o remember_window_size=no \
        sh -c "exec \"\$0\" 2>>\"\$1/cockpit.err\"" "$0" "$1" 2>"$1/kitty.log"
    else
      ANGEL_GLASS=1 exec "$0"
    fi' "$BIN" "$R" > "$R/stdout.log" 2> "$R/stderr.log"
echo "exit=$?" >> "$R/stderr.log"
