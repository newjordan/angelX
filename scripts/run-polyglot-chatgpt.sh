#!/usr/bin/env bash
# angelX or Codex CLI on a ChatGPT-plan model (default gpt-6.1-sol, reasoning low),
# polyglot-v1. Same Prime Verifiers 0.3.1 cell as run-polyglot-luna-medium.sh:
# 136 tasks, one attempt, 600s wall, grade-any-exit, fresh HOME.
# Both harnesses sit on the same ChatGPT Codex seat and post to chatgpt.com
# directly, so the interception proxy sees nothing. Tokens come from angel.usage
# (angelX) or the `codex exec --json` turn usage (Codex), recorded per trace.
#
#   scripts/run-polyglot-chatgpt.sh angelx|codex [dry]
#   MODEL=gpt-6.1-sol EFFORT=low ONLY=<task> ANGEL_BIN=<pin>/angel
set -euo pipefail

HARNESS=${1:?harness: angelx|codex}
DRY=${2:-}
RR=/home/frosty40/angel_tests/angelX-bench/polyglot-20260921
WS=/home/frosty40/angelX
MODEL=${MODEL:-gpt-6.1-sol}
EFFORT=${EFFORT:-low}
KEY=${KEY:-sol}
SEAT=$RR/codex-home-$KEY
CFG=$RR/configs/$KEY-$EFFORT.toml
WALL_SECS=${WALL_SECS:-600}
SEED=${SEED:-0}
ONLY=${ONLY:-}
TASKS=${TASKS_FILE:-$RR/tasks-polyglot-v1.json}
GOLD=$RR/gold
AGENT_PATH="$RR/toolbin:/usr/local/bin:/usr/bin:/bin"
ANGEL_BIN=${ANGEL_BIN:-$WS/cockpit/target/release/angel}
CODEX_BIN=${CODEX_BIN:-$(readlink -f /home/frosty40/.local/share/mise/installs/codex/latest/bin/codex)}

mkdir -p "$SEAT" "$RR/configs"
cp -f "$HOME/.codex/models_cache.json" "$SEAT/models_cache.json"
cp -f "$HOME/.codex/auth.json" "$SEAT/auth.json"
chmod 600 "$SEAT/auth.json"
cat >"$SEAT/config.toml" <<EOF
model = "$MODEL"
model_reasoning_effort = "$EFFORT"
EOF
cat >"$CFG" <<EOF
# $MODEL, reasoning $EFFORT. The agent talks to chatgpt.com directly;
# this client is only the eval record. The effort pin is on the agent.
model = "$MODEL"

[client]
base_url = "https://chatgpt.com/backend-api/codex"
api_key_var = "OPENAI_API_KEY"

[sampling]
temperature = 1.0
max_tokens = 8192
reasoning_effort = "$EFFORT"
EOF

N=${N_TASKS:-136}
FLAGS=(
  --env.taskset.id angel-action-v1
  --env.taskset.tasks-path "$TASKS"
  --env.taskset.gold-root "$GOLD"
  --env.agent.runtime.type subprocess
)
TAG=${ONLY:+smoke-}seed${SEED}
OUT=$RR/${OUT_ROOT:-runs}/$KEY/$HARNESS
RUN_DIR=$TAG-$(date -u +%Y%m%dT%H%M%SZ)

case "$HARNESS" in
  angelx)
    [[ -x "$ANGEL_BIN" ]] || { echo "missing angel binary: $ANGEL_BIN" >&2; exit 2; }
    # The luna-direct route (the ChatGPT seat) is patched into the kit by
    # run-polyglot-luna-medium.sh; it reads LUNA_MODEL/LUNA_EFFORT/LUNA_CODEX_HOME.
    grep -q '"luna-direct"' "$RR/prime_v1/angel_action_v1/harness.py" \
      || { echo "kit harness lacks luna-direct; run run-polyglot-luna-medium.sh once" >&2; exit 2; }
    BIN_SHA=$(sha256sum "$ANGEL_BIN" | cut -d' ' -f1)
    SRC_SHA=$("$ANGEL_BIN" --build-info --json | python3 -c 'import json,re,sys; v=json.load(sys.stdin).get("cockpit_source_sha256") or ""; print(v if re.fullmatch(r"[0-9a-f]{64}", v) else "")')
    FLAGS+=(
      --env.agent.harness.id angel-action-v1
      --env.agent.harness.angel-bin "$ANGEL_BIN"
      --env.agent.harness.angel-bin-sha256 "$BIN_SHA"
      --env.agent.harness.route luna-direct
      --env.agent.harness.moa-max-tokens 8192
      --env.agent.harness.reasoning-effort "$EFFORT"
      --env.agent.harness.max-hops "${ANGEL_MAX_HOPS:-60}"
      --env.agent.harness.env.ANGEL_SPIN_LIMIT "${ANGEL_SPIN_LIMIT:-4}"
      --env.agent.harness.env.ANGEL_UNPRODUCTIVE_STREAK_STOP "${ANGEL_UNPRODUCTIVE_STREAK_STOP:-16}"
      --env.agent.harness.env.ANGEL_ERROR_LIMIT "${ANGEL_ERROR_LIMIT:-6}"
      --env.agent.harness.env.ANGEL_VERIFY_BEFORE_DONE "${ANGEL_VERIFY_BEFORE_DONE:-0}"
      --env.agent.harness.env.ANGEL_VERIFY_NUDGES "${ANGEL_VERIFY_NUDGES:-4}"
      --env.agent.harness.env.ANGEL_TOOLCALL_STORM "${ANGEL_TOOLCALL_STORM:-1}"
      --env.agent.harness.env.ANGEL_WIRE_LOG_DIR "$OUT/$RUN_DIR/wire"
    )
    if [[ -n "$SRC_SHA" ]]; then
      FLAGS+=(--env.agent.harness.angel-cockpit-source-sha256 "$SRC_SHA")
    else
      echo "cockpit source sha unbound; omitting pin" >&2
    fi
    BIN_NOTE=$ANGEL_BIN ;;
  codex)
    [[ -x "$CODEX_BIN" ]] || { echo "missing codex binary: $CODEX_BIN" >&2; exit 2; }
    FLAGS+=(
      --env.agent.harness.id codex-action-v1
      --env.agent.harness.codex-bin "$CODEX_BIN"
      --env.agent.harness.chatgpt-home "$SEAT"
      --env.agent.harness.reasoning-effort "$EFFORT"
      --env.agent.harness.events-dir "$OUT/$RUN_DIR/events"
    )
    BIN_NOTE=$CODEX_BIN ;;
  *) echo "unknown harness: $HARNESS" >&2; exit 2 ;;
esac
if [[ -n "$ONLY" ]]; then
  FLAGS+=(--env.taskset.only-task "$ONLY")
  N=1
fi
[[ "$DRY" == dry ]] && FLAGS+=(--dry-run True)

mkdir -p "$OUT" "$RR/logs"
printf '%s\n' "$OUT/$RUN_DIR" >"$RR/logs/$KEY-$HARNESS-current-run.txt"
echo "cell model=$MODEL effort=$EFFORT harness=$HARNESS seed=$SEED n=$N wall=${WALL_SECS}s bin=$BIN_NOTE out=$OUT/$RUN_DIR"
exec env -i \
  HOME=/home/frosty40 USER=frosty40 LOGNAME=frosty40 LANG=C.UTF-8 TERM=dumb \
  PATH="$AGENT_PATH:$RR/prime_v1/.venv/bin" \
  OPENAI_API_KEY=unused-chatgpt-seat \
  LUNA_MODEL="$MODEL" \
  LUNA_EFFORT="$EFFORT" \
  LUNA_CODEX_HOME="$SEAT" \
  PRIME_AGENT_ISOLATION=fresh-home \
  PRIME_AGENT_PATH="$AGENT_PATH" \
  PRIME_REAL_HOME=/home/frosty40 \
  PRIME_AGENT_WALL_SECS=$WALL_SECS \
  PRIME_AGENT_GRADE_ANY_EXIT=1 \
  "$RR/prime_v1/.venv/bin/eval" angel-action-v1 @ "$CFG" \
  "${FLAGS[@]}" \
  -n "$N" -r 1 -c 1 --no-serve --no-rich --no-push \
  -o "$OUT" --run.dir "$RUN_DIR"
