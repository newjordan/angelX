#!/usr/bin/env bash
# Board A runner for docs/RENDER_SCORECARD.md. Everything runs on the
# glass test kit's nested seat (wayland-2); instance 0 is the operator's
# real desktop and is never touched.
#
#   run_scorecard.sh kitty blast4 --transport shm
#   run_scorecard.sh glass blast4            (immediate present, 120fps pace)
#   run_scorecard.sh glass blast4 --vsync    (mailbox, 60fps: the A6 race)
#   run_scorecard.sh resources               idle RSS/CPU for both hosts
#   run_scorecard.sh keyframe /path/to/angel A1: glass, then kitty
#   run_scorecard.sh all /path/to/angel
#
# Results land in /work/tmp/render-scorecard/<timestamp>/.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../../.." && pwd)
# The nested seat only. Inheriting the caller's WAYLAND_DISPLAY once
# put bench windows (and keyframe's virtual keyboard) on the real
# desktop, so this is forced, with an explicit opt-out name.
export WAYLAND_DISPLAY=${SCORECARD_WAYLAND:-wayland-2}
if [ "$WAYLAND_DISPLAY" = "wayland-1" ] && [ "${SCORECARD_ALLOW_WAYLAND1:-}" != 1 ]; then
  echo "refusing wayland-1 (the operator's desktop); set SCORECARD_WAYLAND" >&2
  exit 2
fi
KIT=${GLASS_KIT:-/work/artifacts/glass-test-kit}
RUN=${SCORECARD_RUN:-/work/tmp/render-scorecard/$(date +%Y%m%d-%H%M%S)}
mkdir -p "$RUN"
echo "run dir: $RUN (load:$(cut -d' ' -f1-3 /proc/loadavg))" | tee "$RUN/environment.txt"
git -C "$ROOT" rev-parse HEAD >> "$RUN/environment.txt" 2>/dev/null
kitty --version >> "$RUN/environment.txt" 2>/dev/null

hypr_instance() { # instance id whose wl socket is $WAYLAND_DISPLAY
  hyprctl instances -j 2>/dev/null | python3 -c '
import json, os, sys
for i in json.load(sys.stdin):
    if i.get("wl_socket") == os.environ["WAYLAND_DISPLAY"]:
        print(i["instance"]); break'
}

close_class() { # close windows of a class on the nested instance only
  local inst; inst=$(hypr_instance)
  [ -n "$inst" ] && hyprctl -i "$inst" eval \
    "hl.dispatch(hl.dsp.window.close({ window = \"class:$1\" }))" >/dev/null 2>&1
  sleep 1
}

rss_kib() { awk '/^VmRSS/{print $2}' "/proc/$1/status" 2>/dev/null || echo 0; }
cpu_ticks() { awk '{print $14+$15}' "/proc/$1/stat" 2>/dev/null || echo 0; }

sample_resources() { # $1 pid, $2 label: writes peak RSS + CPU% over the wait
  local pid=$1 label=$2 t0 peak=0 rss last cap=${SAMPLE_SECS:-240}
  t0=$(cpu_ticks "$pid"); last=$t0
  local start=$SECONDS
  while kill -0 "$pid" 2>/dev/null && [ $((SECONDS - start)) -lt "$cap" ]; do
    rss=$(rss_kib "$pid"); [ "$rss" -gt "$peak" ] && peak=$rss
    last=$(cpu_ticks "$pid")
    sleep 1
  done
  local dt=$((SECONDS - start)); [ "$dt" -lt 1 ] && dt=1
  local used=$(( last - t0 ))
  echo "{\"host\":\"$label\",\"peak_rss_kib\":$peak,\"cpu_pct\":$(awk "BEGIN{printf \"%.1f\", $used/100.0/$dt*100}")}" \
    > "$RUN/resources-$label.json"
}

run_kitty() { # run_kitty <mode> [bench args...]
  local mode=$1; shift
  local out="$RUN/kitty-$mode-$*${SCORECARD_TAG:-}.json"; out=${out// /-}; out=${out//--/}
  rm -f "$out" "$out.exit" "$out.err" "$out.log"
  # Steelmanned: kitty's fastest legitimate settings (kitty-steel.conf).
  BENCH_OUT="$out" setsid -f kitty --class angelcap-glass --config "$HERE/kitty-steel.conf" \
    -o initial_window_width=1600 -o initial_window_height=950 \
    sh -c "python3 \"$HERE/kitty_gfx_bench.py\" $mode $* 2>\"$out.err\"; echo \$? >\"$out.exit\"; sleep 2" \
    >"$out.log" 2>&1 </dev/null
  local waited=0
  while [ ! -s "$out" ] && [ ! -e "$out.exit" ] && [ $waited -lt 180 ]; do sleep 1; waited=$((waited+1)); done
  close_class angelcap-glass
  pkill -f 'kitty_gfx_bench.py' 2>/dev/null; pkill -f '^kitty --class angelcap-glass' 2>/dev/null; true
  if [ -s "$out" ]; then cat "$out"; else echo "KITTY $mode FAILED"; tail -8 "$out.err" "$out.log" 2>/dev/null; fi
}

run_glass() { # run_glass <mode> [--vsync] [KEY=VAL ...]
  local mode=$1; shift
  local present=immediate fps=120
  if [ "${1:-}" = "--vsync" ]; then present=mailbox fps=60; shift; fi
  local bin="$ROOT/crates/target/release/examples/stream-bench"
  [ -x "$bin" ] || (cd "$ROOT/crates" && cargo build --offline --release -p angel-glass --example stream-bench)
  local log="$RUN/glass-$mode-$present${SCORECARD_TAG:-}.log"
  rm -f "$log.frames.jsonl" "$log.produce.json"
  ANGEL_GLASS_STATS=1 ANGEL_GLASS_PRESENT=$present ANGEL_GLASS_SIZE=${ANGEL_GLASS_SIZE:-228x60} \
    ANGEL_GLASS_FONT_SIZE=${ANGEL_GLASS_FONT_SIZE:-9} \
    ANGEL_GLASS_APP_ID=angelcap-glass ANGEL_GLASS_TRANSITION=0 \
    ANGEL_GLASS_STATS_RAW="$log.frames.jsonl" STREAM_BENCH_RAW="$log.produce.json" \
    STREAM_BENCH_MODE=$mode STREAM_BENCH_FPS=$fps env "$@" "$bin" >"$log" 2>&1 &
  local pid=$!
  sample_resources "$pid" "glass-$mode" &
  wait "$pid"
  python3 - "$log" "$mode" "$present" <<'PYEOF' | tee "$log.json"
import re, statistics, sys, json, os
log, mode, present = sys.argv[1:4]
stats, parts, bench = [], [], {}
for line in open(log, errors="replace"):
    kv = dict(re.findall(r"(\w+)=([\w.]+)", line))
    if line.startswith("GLASS_STATS"): stats.append(kv)
    elif line.startswith("GLASS_PARTS"): parts.append(kv)
    elif line.startswith("STREAM_BENCH"): bench = kv
stats, parts = stats[1:], parts[1:]  # first window is warm-up
def med(rows, key):
    vals = [float(r[key]) for r in rows if key in r]
    return round(statistics.median(vals), 3) if vals else None
out = {"host": "glass", "mode": mode, "present": present,
       "fps": med(stats, "fps"), "render_p50_ms": med(stats, "render_p50_ms"),
       "render_p95_ms": med(stats, "render_p95_ms"),
       "build_mean_ms": med(parts, "build_mean_ms"),
       "submit_mean_ms": med(parts, "submit_mean_ms"),
       "present_mean_ms": med(parts, "present_mean_ms"),
       "upload_ms_per_frame": None, "bench": bench}
if stats and out["fps"]:
    up = [float(s["upload_ms"]) for s in stats]
    out["upload_ms_per_frame"] = round(statistics.median(up) / max(out["fps"] * 2, 1), 3)
if bench.get("cols") and out["fps"]:
    out["cells"] = int(bench["cols"]) * int(bench["rows"])
    out["cells_per_s"] = round(out["cells"] * out["fps"])
# Raw series beside the summary: presented frame times (first stats
# window dropped as warm-up) and producer handover times.
series = {"host": "glass", "mode": mode, "present": present}
try:
    windows = [json.loads(x) for x in open(log + ".frames.jsonl") if x.strip()]
    series["frames_ms"] = [t for w in windows[1:] for t in w]
except OSError:
    series["frames_ms"] = []
try:
    series["produce_ms"] = json.load(open(log + ".produce.json"))
except OSError:
    series["produce_ms"] = []
with open(log + ".series.json", "w") as f:
    json.dump(series, f)
out["series"] = os.path.basename(log + ".series.json")
print(json.dumps(out, indent=1))
PYEOF
}

run_keyframe() { # run_keyframe <angel binary> ; KF_ORDER="kitty glass" flips order
  local bin=$1
  for side in ${KF_ORDER:-glass kitty}; do
    # Fresh run name per invocation: a reused name restores prior
    # session state, which once made angel exit cleanly mid-probe
    # and looked exactly like a cursor-seek failure.
    local name="scorecard-kf-$side-$(date +%s)"
    if [ "$side" = kitty ]; then export KITTY=1; else export KITTY=; fi
    # No assembly transition: its white blocks are what keyframe's
    # cursor hunt finds if it looks too early.
    KITTY_CONF="$HERE/kitty-steel.conf" setsid -f "$HERE/glass-run-steel.sh" "$bin" "$name" ANGEL_GLASS_TRANSITION=0 ${KF_EXTRA_ENV:-} \
      </dev/null >>"$RUN/keyframe-$side${SCORECARD_TAG:-}.launch.log" 2>&1
    # Wait for the real cursor (the startup intro's white blocks fool
    # the hunt under load), then name the cell explicitly.
    local cell= waited=0
    while [ -z "$cell" ] && [ $waited -lt 75 ]; do
      sleep 3; waited=$((waited + 3))
      WAYLAND_DISPLAY=$WAYLAND_DISPLAY grim "$RUN/kf-seek.png" 2>/dev/null
      cell=$(python3 "$HERE/find_cursor.py" "$RUN/kf-seek.png" 2>/dev/null) || cell=
    done
    echo "keyframe $side cell=${cell:-unfound} after ${waited}s"
    local try
    for try in 1 2; do
      if WAYLAND_DISPLAY=$WAYLAND_DISPLAY KF_CELL="$cell" "$KIT/keyframe/target/release/keyframe" \
        "render-scorecard-keyframe-check-0123456789" >"$RUN/keyframe-$side${SCORECARD_TAG:-}.txt" 2>&1; then
        break
      fi
      sleep 6
    done
    tail -1 "$RUN/keyframe-$side${SCORECARD_TAG:-}.txt"
    close_class angelcap-glass
    sleep 2
  done
}

run_resources() {
  # Idle: kitty sitting at a shell prompt vs glass at rest (1 fps tick).
  setsid -f kitty --class angelcap-glass --config "$HERE/kitty-steel.conf" \
    sleep 30 </dev/null >/dev/null 2>&1
  sleep 4
  local kpid; kpid=$(pgrep -n -f '^kitty --class angelcap-glass')
  echo "kitty idle pid: ${kpid:-none}"
  # Fixed 12 s window after a 4 s settle, same for both hosts.
  [ -n "$kpid" ] && SAMPLE_SECS=12 sample_resources "$kpid" kitty-idle
  close_class angelcap-glass
  pkill -f '^kitty --class angelcap-glass' 2>/dev/null
  ANGEL_GLASS_APP_ID=angelcap-glass ANGEL_GLASS_TRANSITION=0 \
    STREAM_BENCH_MODE=text STREAM_BENCH_FPS=1 STREAM_BENCH_SECONDS=20 \
    "$ROOT/crates/target/release/examples/stream-bench" >/dev/null 2>&1 &
  local gpid=$!; sleep 4; SAMPLE_SECS=12 sample_resources "$gpid" glass-idle; wait "$gpid"
  cat "$RUN"/resources-*.json 2>/dev/null
}

case "${1:-}" in
  kitty) shift; run_kitty "$@" ;;
  glass) shift; run_glass "$@" ;;
  keyframe) run_keyframe "$2" ;;
  resources) run_resources ;;
  all)
    angel=$2
    run_kitty blast4 --transport b64
    run_kitty blast4 --transport png
    run_kitty blast4 --transport shm
    run_glass blast4
    run_kitty big --w 1920 --h 1080 --transport shm --frames 120
    run_glass big STREAM_BENCH_W=1920 STREAM_BENCH_H=1080
    run_kitty big --w 3840 --h 2160 --transport shm --frames 60
    run_glass big STREAM_BENCH_W=3840 STREAM_BENCH_H=2160
    for n in 100 1000 5000; do
      run_kitty placements --n "$n" --frames 120
      run_glass placements STREAM_BENCH_N=$n
    done
    run_kitty text --frames 300
    run_glass text
    run_glass blast4 --vsync
    run_resources
    [ -n "$angel" ] && run_keyframe "$angel"
    echo "all results in $RUN" ;;
  *) sed -n '2,14p' "$0"; exit 2 ;;
esac
