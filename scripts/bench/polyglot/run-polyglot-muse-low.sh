#!/usr/bin/env bash
# angelX × Muse Spark (reasoning low) on polyglot-v1.
# Same Prime Verifiers 0.3.1 cell as the 2026-09-21 round: 136 tasks, one attempt,
# 600s wall, grade-any-exit, fresh HOME, sampling imposed by the interception proxy.
# Muse speaks the Responses API (its Chat Completions endpoint redacts reasoning);
# the proxy relays /v1/responses to api.meta.ai and records every call. The agent
# only ever sees the proxy secret; META_API_KEY stays on the eval process.
# Temperature 1.0 is Meta's recommended setting for Muse Spark.
set -euo pipefail

RR=/home/frosty40/angel_tests/angelX-bench/polyglot-20260921
WS=/home/frosty40/angelX
CFG=$RR/configs/muse-low.toml
KEY_VAR=META_API_KEY
MUSE_MODEL=${MUSE_MODEL:-muse-spark-1.3}
MUSE_EFFORT=${MUSE_EFFORT:-low}
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

python3 - "$RR" "$MUSE_MODEL" "$MUSE_EFFORT" <<'PY'
import pathlib, sys
rr, model, effort = sys.argv[1:]
rr = pathlib.Path(rr)
(rr / "configs" / "muse-low.toml").write_text(
    f"""# Muse Spark, reasoning {effort} — imposed by the Verifiers interception proxy.
# Temperature 1.0 is Meta's recommended sampling for Muse Spark.
model = "{model}"

[client]
base_url = "https://api.meta.ai/v1"
api_key_var = "META_API_KEY"

[sampling]
temperature = 1.0
max_tokens = 8192
reasoning_effort = "{effort}"
"""
)
harness = rr / "prime_v1" / "angel_action_v1" / "harness.py"
text = harness.read_text()
if "meta-direct" not in text:
    for old, new in (
        ('"luna-direct",\n        "sota-moa-longcat",', '"luna-direct",\n        "meta-direct",\n        "sota-moa-longcat",'),
        ('"luna-direct",\n            "sota-moa-longcat",', '"luna-direct",\n            "meta-direct",\n            "sota-moa-longcat",'),
        (
            '{"longcat-direct", "deepseek-direct", "glm-direct", "grok-direct", "luna-direct"}',
            '{"longcat-direct", "deepseek-direct", "glm-direct", "grok-direct", "luna-direct", "meta-direct"}',
        ),
        (
            '''        elif self.config.route == "luna-direct":
            roles = "openai"
''',
            '''        elif self.config.route == "luna-direct":
            roles = "openai"
        elif self.config.route == "meta-direct":
            roles = "meta"
''',
        ),
    ):
        if old not in text:
            raise SystemExit(f"harness.py marker missing: {old[:60]!r}")
        text = text.replace(old, new, 1)
    needle = '''        elif self.config.route == "luna-direct":
            env.update('''
    insert = '''        elif self.config.route == "meta-direct":
            # Responses-API seat through the proxy's /v1/responses relay. The
            # agent holds only the proxy secret; the proxy imposes sampling.
            env.update(
                {
                    "ANGEL_DRIVER": "meta",
                    "ANGEL_API_CLUBS": "meta",
                    "ANGEL_META_URL": endpoint,
                    "ANGEL_META_KEY": secret,
                    "ANGEL_META_MODEL": model,
                    "ANGEL_META_REASONING_EFFORT": os.environ.get("MUSE_EFFORT", "low"),
                    "ANGEL_REASONING_EFFORT": os.environ.get("MUSE_EFFORT", "low"),
                    "ANGEL_HTTP_TIMEOUT": "180",
                    "ANGEL_HTTP_RETRIES": "1",
                }
            )
'''
    if needle not in text:
        raise SystemExit("luna env block missing; cannot insert meta env")
    text = text.replace(needle, insert + needle, 1)
    harness.write_text(text)
    print("patched harness with meta-direct")
else:
    print("harness already has meta-direct")
PY

python3 -m py_compile "$RR/prime_v1/angel_action_v1/harness.py"

N=$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))))' "$TASKS")
BIN_SHA=$(sha256sum "$ANGEL_BIN" | cut -d' ' -f1)
if [[ -n "${ANGEL_COCKPIT_SOURCE_SHA256:-}" ]]; then
  SRC_SHA=$ANGEL_COCKPIT_SOURCE_SHA256
else
  SRC_SHA=$("$ANGEL_BIN" --build-info --json | python3 -c 'import json,re,sys; v=json.load(sys.stdin).get("cockpit_source_sha256") or ""; print(v if re.fullmatch(r"[0-9a-f]{64}", v) else "")')
fi

# Read only the one upstream key, in a subshell: nothing else from the host
# key files reaches the eval (and so none of it can reach an agent).
KEY_VALUE=$(bash -c 'for f in "$HOME/.config/host_env/system.env" "$HOME/.config/host_env/api_keys.sh"; do [ -r "$f" ] && . "$f" >/dev/null 2>&1; done; printf %s "${ANGEL_META_KEY:-${META_API_KEY:-}}"')
if [[ -z "$KEY_VALUE" ]]; then echo "missing Meta API key (ANGEL_META_KEY)" >&2; exit 2; fi

FLAGS=(
  --env.taskset.id angel-action-v1
  --env.taskset.tasks-path "$TASKS"
  --env.taskset.gold-root "$GOLD"
  --env.agent.runtime.type subprocess
  --env.agent.harness.id angel-action-v1
  --env.agent.harness.angel-bin "$ANGEL_BIN"
  --env.agent.harness.angel-bin-sha256 "$BIN_SHA"
  --env.agent.harness.route meta-direct
  --env.agent.harness.moa-max-tokens 8192
  --env.agent.harness.reasoning-effort "$MUSE_EFFORT"
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
OUT=$RR/${OUT_ROOT:-runs}/muse/angelx
RUN_DIR=$TAG-$(date -u +%Y%m%dT%H%M%SZ)
# Per-call wire records land beside the traces: rollout workdirs are deleted
# after grading, so this is the only copy.
FLAGS+=(--env.agent.harness.env.ANGEL_WIRE_LOG_DIR "$OUT/$RUN_DIR/wire")
mkdir -p "$OUT" "$RR/logs"
printf '%s\n' "$OUT/$RUN_DIR" >"$RR/logs/muse-angelx-current-run.txt"

EXTRA=()
[[ "${DRY:-}" == "1" ]] && EXTRA=(--dry-run True)

echo "cell model=$MUSE_MODEL effort=$MUSE_EFFORT harness=angelx seed=$SEED n=$N wall=${WALL_SECS}s bin=$ANGEL_BIN out=$OUT/$RUN_DIR"
exec env -i \
  HOME=/home/frosty40 USER=frosty40 LOGNAME=frosty40 LANG=C.UTF-8 TERM=dumb \
  PATH="$AGENT_PATH" \
  "$KEY_VAR=$KEY_VALUE" \
  MUSE_EFFORT="$MUSE_EFFORT" \
  PRIME_AGENT_ISOLATION=fresh-home \
  PRIME_AGENT_PATH="$AGENT_PATH" \
  PRIME_REAL_HOME=/home/frosty40 \
  PRIME_AGENT_WALL_SECS=$WALL_SECS \
  PRIME_AGENT_GRADE_ANY_EXIT=1 \
  "$RR/prime_v1/.venv/bin/eval" angel-action-v1 @ "$CFG" \
  "${FLAGS[@]}" \
  -n "$N" -r 1 -c 1 --no-serve --no-rich --no-push \
  -o "$OUT" --run.dir "$RUN_DIR" "${EXTRA[@]}"
