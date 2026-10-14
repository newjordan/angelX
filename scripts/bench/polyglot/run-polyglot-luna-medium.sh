#!/usr/bin/env bash
# angelX × GPT-6-Luna (reasoning medium) on polyglot-v1.
# Same Prime Verifiers 0.3.1 cell as the 2026-09-21 round: 136 tasks, one attempt,
# 600s wall, grade-any-exit, fresh HOME.
# Luna is the ChatGPT Codex Responses seat (gpt-6-luna), not an API-key HTTP club.
# Production Codex posts straight to chatgpt.com, so the interception proxy does
# not see those calls. Effort is pinned by ANGEL_OPENAI_REASONING_EFFORT, and
# token/cache numbers come from angel.usage (Responses usage), not proxy traces.
set -euo pipefail

RR=/home/frosty40/angel_tests/angelX-bench/polyglot-20260921
WS=/home/frosty40/angelX
CODEX_HOME_PIN=$RR/codex-home-luna
CFG=$RR/configs/luna-medium.toml
ANGEL_ROUTE=luna-direct
LUNA_MODEL=${LUNA_MODEL:-gpt-6-luna}
LUNA_EFFORT=${LUNA_EFFORT:-medium}
WALL_SECS=${WALL_SECS:-600}
SEED=${SEED:-0}
ONLY=${ONLY:-}
TASKS=${TASKS_FILE:-$RR/tasks-polyglot-v1.json}
GOLD=$RR/gold
AGENT_PATH="$RR/toolbin:/usr/local/bin:/usr/bin:/bin"
ANGEL_BIN=${ANGEL_BIN:-$WS/cockpit/target/release/angel}

if [[ ! -x "$ANGEL_BIN" ]]; then
  echo "missing angel binary: $ANGEL_BIN" >&2
  exit 2
fi

mkdir -p "$CODEX_HOME_PIN" "$RR/configs"
cp -f "$HOME/.codex/models_cache.json" "$CODEX_HOME_PIN/models_cache.json"
cp -f "$HOME/.codex/auth.json" "$CODEX_HOME_PIN/auth.json"
chmod 600 "$CODEX_HOME_PIN/auth.json"
cat >"$CODEX_HOME_PIN/config.toml" <<EOF
model = "$LUNA_MODEL"
model_reasoning_effort = "$LUNA_EFFORT"
EOF

python3 - "$RR" "$LUNA_MODEL" "$LUNA_EFFORT" "$CODEX_HOME_PIN" <<'PY'
import pathlib, sys
rr, model, effort, codex_home = sys.argv[1:]
cfg = pathlib.Path(rr) / "configs" / "luna-medium.toml"
cfg.write_text(
    f"""# {model}, reasoning {effort}. Codex talks to chatgpt.com directly;
# this client is only the eval record. Sampling on the agent is the effort pin.
model = "{model}"

[client]
base_url = "https://chatgpt.com/backend-api/codex"
api_key_var = "OPENAI_API_KEY"

[sampling]
temperature = 1.0
max_tokens = 8192
reasoning_effort = "{effort}"
"""
)
harness = pathlib.Path(rr) / "prime_v1" / "angel_action_v1" / "harness.py"
text = harness.read_text()
if "import os\n" not in text and "import os," not in text:
    text = text.replace("import json\n", "import json\nimport os\n", 1)
literal = '"grok-direct",\n        "sota-moa-longcat",'
if "luna-direct" not in text:
    if literal not in text:
        raise SystemExit("route literal marker missing")
    text = text.replace(
        literal,
        '"grok-direct",\n        "luna-direct",\n        "sota-moa-longcat",',
        1,
    )
    text = text.replace(
        '"grok-direct",\n            "sota-moa-longcat",',
        '"grok-direct",\n            "luna-direct",\n            "sota-moa-longcat",',
        1,
    )
    old_direct = '{"longcat-direct", "deepseek-direct", "glm-direct", "grok-direct"}'
    if old_direct not in text:
        raise SystemExit("direct-route set missing")
    text = text.replace(
        old_direct,
        '{"longcat-direct", "deepseek-direct", "glm-direct", "grok-direct", "luna-direct"}',
        1,
    )
old_roles = '''        elif self.config.route == "grok-direct":
            roles = "grok"
'''
new_roles = '''        elif self.config.route == "grok-direct":
            roles = "grok"
        elif self.config.route == "luna-direct":
            roles = "openai"
'''
if 'self.config.route == "luna-direct"' not in text:
    if old_roles not in text:
        raise SystemExit("roles block missing")
    text = text.replace(old_roles, new_roles, 1)
marker = 'elif self.config.route == "luna-direct":'
if "LUNA_CODEX_HOME" not in text:
    # Insert the env pin immediately after the grok env dict closes and before
    # the longcat else, if that shape is still there; otherwise after the roles line.
    needle = '''                    "ANGEL_GROK_API_STREAM_USAGE": "1",
                }
            )
            if self.config.moa_max_tokens is not None:'''
    insert = '''                    "ANGEL_GROK_API_STREAM_USAGE": "1",
                }
            )
        elif self.config.route == "luna-direct":
            env.update(
                {
                    "ANGEL_DRIVER": "openai",
                    "ANGEL_OPENAI_MODEL": os.environ.get("LUNA_MODEL", "gpt-6-luna"),
                    "ANGEL_OPENAI_REASONING_EFFORT": os.environ.get("LUNA_EFFORT", "medium"),
                    "ANGEL_REASONING_EFFORT": os.environ.get("LUNA_EFFORT", "medium"),
                    "CODEX_HOME": os.environ.get(
                        "LUNA_CODEX_HOME",
                        "/home/frosty40/angel_tests/angelX-bench/polyglot-20260921/codex-home-luna",
                    ),
                    "ANGEL_HTTP_TIMEOUT": "180",
                    "ANGEL_HTTP_RETRIES": "1",
                }
            )
            if self.config.moa_max_tokens is not None:'''
    if needle not in text:
        raise SystemExit("grok env block missing; cannot insert luna env")
    text = text.replace(needle, insert, 1)
harness.write_text(text)
print("harness luna-direct ready")
PY

BIN_SHA=$(sha256sum "$ANGEL_BIN" | cut -d' ' -f1)
if [[ -n "${ANGEL_COCKPIT_SOURCE_SHA256:-}" ]]; then
  SRC_SHA=$ANGEL_COCKPIT_SOURCE_SHA256
else
  SRC_SHA=$("$ANGEL_BIN" --build-info --json | python3 -c 'import json,re,sys; v=json.load(sys.stdin).get("cockpit_source_sha256") or ""; print(v if re.fullmatch(r"[0-9a-f]{64}", v) else "")')
fi

# The proxy is not on the Codex path. A placeholder keeps the eval client config valid.
KEY_VALUE=unused-luna-direct
N=${N_TASKS:-136}

FLAGS=(
  --env.taskset.id angel-action-v1
  --env.taskset.tasks-path "$TASKS"
  --env.taskset.gold-root "$GOLD"
  --env.agent.runtime.type subprocess
  --env.agent.harness.id angel-action-v1
  --env.agent.harness.angel-bin "$ANGEL_BIN"
  --env.agent.harness.angel-bin-sha256 "$BIN_SHA"
  --env.agent.harness.route "$ANGEL_ROUTE"
  --env.agent.harness.moa-max-tokens 8192
  --env.agent.harness.reasoning-effort "$LUNA_EFFORT"
  --env.agent.harness.max-hops "${ANGEL_MAX_HOPS:-60}"
  --env.agent.harness.env.ANGEL_SPIN_LIMIT "${ANGEL_SPIN_LIMIT:-4}"
  --env.agent.harness.env.ANGEL_UNPRODUCTIVE_STREAK_STOP "${ANGEL_UNPRODUCTIVE_STREAK_STOP:-16}"
  --env.agent.harness.env.ANGEL_ERROR_LIMIT "${ANGEL_ERROR_LIMIT:-6}"
  --env.agent.harness.env.ANGEL_VERIFY_BEFORE_DONE "${ANGEL_VERIFY_BEFORE_DONE:-0}"
  --env.agent.harness.env.ANGEL_VERIFY_NUDGES "${ANGEL_VERIFY_NUDGES:-4}"
  --env.agent.harness.env.ANGEL_TOOLCALL_STORM "${ANGEL_TOOLCALL_STORM:-1}"
)
if [[ -n "$SRC_SHA" ]]; then
  FLAGS+=(--env.agent.harness.angel-cockpit-source-sha256 "$SRC_SHA")
else
  echo "cockpit source sha unbound; omitting pin" >&2
fi
if [[ -n "$ONLY" ]]; then
  FLAGS+=(--env.taskset.only-task "$ONLY")
  N=1
fi

TAG=${ONLY:+smoke-}seed${SEED}
OUT=$RR/${OUT_ROOT:-runs}/luna/angelx
RUN_DIR=$TAG-$(date -u +%Y%m%dT%H%M%SZ)
# Per-call wire records (club/wire_log.rs) land beside the traces: rollout
# workdirs are deleted after grading, so this is the only copy.
FLAGS+=(--env.agent.harness.env.ANGEL_WIRE_LOG_DIR "$OUT/$RUN_DIR/wire")
mkdir -p "$OUT" "$RR/logs"
printf '%s\n' "$OUT/$RUN_DIR" >"$RR/logs/luna-angelx-current-run.txt"

echo "cell model=$LUNA_MODEL effort=$LUNA_EFFORT harness=angelx seed=$SEED n=$N wall=${WALL_SECS}s bin=$ANGEL_BIN out=$OUT/$RUN_DIR"
exec env -i \
  HOME=/home/frosty40 USER=frosty40 LOGNAME=frosty40 LANG=C.UTF-8 TERM=dumb \
  PATH="$AGENT_PATH:$RR/prime_v1/.venv/bin" \
  OPENAI_API_KEY="$KEY_VALUE" \
  LUNA_MODEL="$LUNA_MODEL" \
  LUNA_EFFORT="$LUNA_EFFORT" \
  LUNA_CODEX_HOME="$CODEX_HOME_PIN" \
  PRIME_AGENT_ISOLATION=fresh-home \
  PRIME_AGENT_PATH="$AGENT_PATH" \
  PRIME_REAL_HOME=/home/frosty40 \
  PRIME_AGENT_WALL_SECS=$WALL_SECS \
  PRIME_AGENT_GRADE_ANY_EXIT=1 \
  "$RR/prime_v1/.venv/bin/eval" angel-action-v1 @ "$CFG" \
  "${FLAGS[@]}" \
  -n "$N" -r 1 -c 1 --no-serve --no-rich --no-push \
  -o "$OUT" --run.dir "$RUN_DIR"
