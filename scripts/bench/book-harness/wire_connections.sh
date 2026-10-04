#!/usr/bin/env bash
# wire_connections.sh <angel-binary> <label> — walk one build through a scripted
# fix and a scripted loop on every seat's real club, each pointed at the local
# stub model, and keep every request each seat sent: out/wire-<label>-<seat>-<scenario>.jsonl.
# wire_connections.py then checks each approved piece on each seat's wire.
# Seats: deepseek, glm, grok, muse (Responses API), local. No API is called.
set -euo pipefail
BIN=$(readlink -f "${1:?angel binary}"); LABEL=${2:?label}
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$HERE/out; mkdir -p "$OUT"
SEATS=${SEATS:-"deepseek glm grok muse local"}
SCENARIOS=${SCENARIOS:-"fix loop"}

seat_env() {  # seat url
  local u=$2
  case $1 in
    deepseek) echo ANGEL_DRIVER=deepseek ANGEL_API_CLUBS=deepseek ANGEL_DEEPSEEK_URL=$u ANGEL_DEEPSEEK_KEY=stub ANGEL_DEEPSEEK_MODEL=deepseek-flash ANGEL_DEEPSEEK_FLASH_MODEL=deepseek-flash ;;
    glm) echo ANGEL_DRIVER=glm-5.3 ANGEL_API_CLUBS=glm ANGEL_GLM_URL=$u ANGEL_GLM_KEY=stub ANGEL_GLM_MODEL=glm-5.3 ;;
    grok) echo ANGEL_DRIVER=grok-api ANGEL_API_CLUBS=grok ANGEL_GROK_API_URL=$u ANGEL_GROK_URL=$u ANGEL_GROK_KEY=stub ANGEL_GROK_API_MODEL=grok-4.7 ANGEL_GROK_MODEL=grok-4.7 ANGEL_GROK_RESEARCH=0 ;;
    muse) echo ANGEL_DRIVER=meta ANGEL_API_CLUBS=meta ANGEL_META_URL=$u ANGEL_META_KEY=stub ANGEL_META_MODEL=muse-spark-1.3 ;;
    local) echo ANGEL_DRIVER=local ANGEL_LOCAL_URL=$u ANGEL_LOCAL_MODEL=stub ;;
  esac
}

for SEAT in $SEATS; do for SC in $SCENARIOS; do
  RUN=$(mktemp -d "${TMPDIR:-/tmp}/angel-wire-XXXXXX")
  WORK=$RUN/work; HOMEDIR=$RUN/home; mkdir -p "$WORK" "$HOMEDIR" "$RUN/tmp"
  printf 'def add(a, b):\n    return a - b\n' > "$WORK/calc.py"
  printf 'import unittest\nfrom calc import add\n\n\nclass T(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(2, 3), 5)\n\n\nif __name__ == "__main__":\n    unittest.main()\n' > "$WORK/test_calc.py"
  ( cd "$WORK" && git init -q && git add -A && git -c user.name=b -c user.email=b@b.invalid commit -qm init )
  LOG=$OUT/wire-$LABEL-$SEAT-$SC.jsonl; rm -f "$LOG" "$LOG.paths"
  SCRIPT_ENV=()
  if [[ $SC == loop ]]; then
    # The same red run six times: the loop detectors and their own turns.
    python3 -c 'import json,sys; s=[["run_tests",{"runtime":"python","entrypoint":"unittest"}]]*6+[None]; json.dump(s,open(sys.argv[1],"w"))' "$RUN/script.json"
    SCRIPT_ENV=(STUB_SCRIPT="$RUN/script.json")
  fi
  READY=$RUN/ready
  env STUB_LOG="$LOG" STUB_PORT=0 STUB_READY="$READY" "${SCRIPT_ENV[@]}" python3 "$HERE/stub_model.py" 2>"$OUT/wire-$LABEL-$SEAT-$SC.stub.err" &
  STUB=$!
  for _ in $(seq 50); do [[ -s $READY ]] && break; sleep 0.1; done
  PORT=$(cat "$READY")
  # shellcheck disable=SC2046
  env -i PATH="$PATH" HOME="$HOMEDIR" TMPDIR="$RUN/tmp" TERM=dumb LANG=C.UTF-8 GIT_CONFIG_GLOBAL=/dev/null \
    $(seat_env "$SEAT" "http://127.0.0.1:$PORT/v1") ANGEL_HTTP_RETRIES=0 \
    "$BIN" --task-json --workspace "$WORK" --task-id "wire-$SEAT-$SC" --max-hops 14 \
    "The unittest suite in this repository fails. Fix the code so it passes." \
    > "$OUT/wire-$LABEL-$SEAT-$SC.result.json" 2>"$OUT/wire-$LABEL-$SEAT-$SC.stderr" || true
  kill "$STUB" 2>/dev/null || true; wait "$STUB" 2>/dev/null || true
  echo "$LABEL $SEAT $SC requests=$(wc -l < "$LOG" 2>/dev/null || echo 0) paths=$(sort -u "$LOG.paths" 2>/dev/null | tr '\n' ' ')"
  rm -rf "$RUN"
done; done
