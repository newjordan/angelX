#!/usr/bin/env bash
# Sourced by both live entry points. No provider call is made by preflight.
MAGE_BENCH_SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
MAGE_REPO=$(cd -- "$MAGE_BENCH_SCRIPT_DIR/../../../.." && pwd)
BIN=${ANGEL_BENCH_BIN:-${ANGEL_BIN:-$MAGE_REPO/cockpit/target/fast/angel}}
MAGE_ROOT=${MAGE_ROOT:-$HOME/mage}  # the mage working folder: bench/, train/, seatbin/, venv/
MAGE_BENCH_ROOT=${MAGE_BENCH_ROOT:-$MAGE_ROOT/bench}
MAGE_BRAIN=${MAGE_BRAIN:-muse-wonder-1.0}

mage_check_launch() {
  case "$MAGE_BRAIN" in
  muse-wonder-1.0)
    if [[ ${MAGE_ALLOW_MUSE:-} != 1 ]]; then
      echo 'Muse token use is disabled. Only after operator approval, set MAGE_ALLOW_MUSE=1 for this run.' >&2
      return 1
    fi ;;
  grok-4.7)
    if [[ ${MAGE_ALLOW_GROK:-} != 1 ]]; then
      echo 'Grok use is disabled. Only after operator approval, set MAGE_ALLOW_GROK=1 for this run.' >&2
      return 1
    fi ;;
  *) echo "unsupported Mage benchmark brain: $MAGE_BRAIN" >&2; return 1 ;;
  esac
  if [[ ! -x $BIN ]]; then
    echo "angel binary is not executable: $BIN (set ANGEL_BENCH_BIN to a built binary)" >&2
    return 1
  fi
}

mage_brain_env() {
  # An unset variable is required: ANGEL_FALLBACK=0 still enables fallback.
  BRAIN_UNSET=(-u ANGEL_FALLBACK -u ANGEL_FALLBACK_INCLUDE_SOTA)
  case "$MAGE_BRAIN" in
  grok-4.7)
    BRAIN_UNSET+=(-u ANGEL_META_KEY -u ANGEL_KIMI_KEY -u ANGEL_KIMI_URL
      -u ANGEL_VISION_URL -u ANGEL_VISION_MODEL -u ANGEL_VISION_KEY)
    BRAIN_ENV=(ANGEL_DRIVER=grok ANGEL_GROK_MODEL=grok-4.7
      ANGEL_YOLO=0 ANGEL_YOLO_SMART=0
      ANGEL_REASONING_EFFORT=low ANGEL_GROK_REASONING_EFFORT=low
      ANGEL_GROK_RESEARCH=0 ANGEL_SOTA_MOA_GROK_RESEARCH=0
      ANGEL_VISION_SIDECAR=0 ANGEL_VISION_TOOLS=1 ANGEL_VIDEO_TOOLS=0
      "ANGEL_VISION_URL=${MAGE_GROK_VISION_URL:-http://127.0.0.1:18847/v1}"
      ANGEL_VISION_MODEL=grok-4.7 ANGEL_VISION_KEY=proxy-held
      ANGEL_TOOL_BUBBLE=0 ANGEL_TOOL_SCHEMA_PROFILE=essential
      ANGEL_HTTP_RETRIES=1)
    ;;
  muse-wonder-1.0)
    BRAIN_ENV=(ANGEL_DRIVER=meta ANGEL_META_MODEL=muse-wonder-1.0
      ANGEL_VISION_URL=https://api.meta.ai/v1 ANGEL_VISION_MODEL=muse-wonder-1.0
      "ANGEL_VISION_KEY=${ANGEL_META_KEY:-}")
    ;;
  *) echo "unsupported Mage benchmark brain: $MAGE_BRAIN" >&2; return 1 ;;
  esac
}

mage_check_seat() {
  local sock=$1 sig=$2 pid
  pid=${MAGE_SEAT_PID:-}
  # Inside the isolated task bus systemctl cannot reach the user's systemd.
  # The parent runner supplies its observed PID; the identity is still checked
  # against Hyprland's live registry and the private socket on every use.
  if [[ -z $pid ]]; then
    pid=$(systemctl --user show mage-nested.service --property=MainPID --value) || return 1
  fi
  [[ $pid =~ ^[1-9][0-9]*$ && -S ${XDG_RUNTIME_DIR:?}/$sock ]] || {
    echo 'private Mage seat is not running; refusing desktop access' >&2; return 1;
  }
  hyprctl instances -j | python3 -I -c '
import json,sys
pid,sock,sig=int(sys.argv[1]),sys.argv[2],sys.argv[3]
assert any(i["pid"] == pid and i["wl_socket"] == sock and i["instance"] == sig for i in json.load(sys.stdin)), "seat identity mismatch"
' "$pid" "$sock" "$sig" || return 1
}

mage_preflight() {
  mage_check_launch || return 1
  local cmd py=${MAGE_PYTHON:-$MAGE_ROOT/venv/bin/python}
  for cmd in hyprctl grim wtype wl-paste dbus-run-session dbus-daemon systemctl timeout flock; do
    command -v "$cmd" >/dev/null || { echo "missing benchmark dependency: $cmd" >&2; return 1; }
  done
  [[ -x $py && -f $MAGE_BENCH_ROOT/items.json ]] || {
    echo 'missing Mage Python environment or captured benchmark items' >&2; return 1;
  }
  # A successful /get_model_info is insufficient: the degraded driver returned
  # HTTP 500 on /generate while the HTTP server was otherwise healthy. Warm up
  # on a captured benchmark image before touching a display. Local inference only.
  "$py" -I - "$MAGE_BENCH_ROOT" "${ANGEL_GROUND_URL:-http://127.0.0.1:30001}" "$MAGE_REPO" <<'PY'
import base64, io, json, pathlib, re, sys, urllib.request
from PIL import Image
root, url = pathlib.Path(sys.argv[1]), sys.argv[2].rstrip('/')
try:
    def get(path):
        with urllib.request.urlopen(url + path, timeout=10) as response:
            return json.load(response)
    info = get('/get_model_info')
    get('/get_server_info')
    item = json.loads((root / 'items.json').read_text())[0]
    image = Image.open(root / item['shot']).convert('RGB')
    buf = io.BytesIO(); image.save(buf, format='JPEG', quality=95)
    system = (pathlib.Path(sys.argv[3]) / 'cockpit/src/agent/tools/hands_ground_prompt.txt').read_text()
    direct = ' The target is visible on the current screen; click it directly in one action.'
    user = ('\nPlease generate the next move according to the UI screenshot, instruction and previous '
            f"actions.\n\nInstruction: {item['instruction'].rstrip('.')}.{direct}\n\nPrevious actions:\nNone")
    prompt = (f'<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n'
              f'<|vision_start|><|image_pad|><|vision_end|>{user}<|im_end|>\n'
              '<|im_start|>assistant\n<think>\n\n</think>\n\n')
    payload = {'text': prompt, 'image_data': [base64.b64encode(buf.getvalue()).decode()],
               'sampling_params': {'max_new_tokens': 1024, 'temperature': 0.0}}
    req = urllib.request.Request(url + '/generate', json.dumps(payload).encode(), {'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=120) as response:
        result = json.load(response)
    point = re.search(r'"coordinate"\s*:\s*\[\s*(-?\d+(?:\.\d+)?)\s*,\s*(-?\d+(?:\.\d+)?)\s*\]', result.get('text', ''))
    if not point or not all(0 <= float(value) <= 999 for value in point.groups()):
        raise ValueError('Mage did not return valid normalized coordinates: ' + repr(result.get('text')))
    # Keep server/runtime identity with the results for reproducibility.
    (root / 'preflight-model-info.json').write_text(json.dumps(info, indent=2))
    (root / 'preflight-server-info.json').write_text(json.dumps(get('/get_server_info'), indent=2))
    print('Mage local inference warmup OK:', result['text'][:200])
except Exception as exc:
    sys.exit(f'Mage preflight failed before desktop launch: {exc}')
PY
}
