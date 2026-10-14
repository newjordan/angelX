#!/usr/bin/env bash
# angelX × Grok 4.7 (reasoning low) on polyglot-v1.
# Same Prime Verifiers 0.3.1 cell as the 2026-09-21 round: 136 tasks, one attempt,
# 600s wall, grade-any-exit, fresh HOME, sampling imposed by the interception proxy.
# Upstream auth is the local Grok OAuth bearer (no XAI_API_KEY in host_env).
# The agent itself only sees the proxy secret, on the grok-api HTTP seat.
set -euo pipefail

RR=/home/frosty40/angel_tests/angelX-bench/polyglot-20260921
WS=/home/frosty40/angelX
CFG=$RR/configs/grok-4.7-low.toml
KEY_VAR=XAI_API_KEY
ANGEL_ROUTE=grok-direct
ANGEL_EFFORT=low
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

python3 - "$RR" <<'PY'
import pathlib, sys
rr = pathlib.Path(sys.argv[1])
cfg = rr / "configs" / "grok-4.7-low.toml"
cfg.write_text(
    """# Grok 4.7, reasoning low — imposed by the Verifiers interception proxy.
# Upstream credential is XAI_API_KEY (filled from ~/.grok/auth.json by the runner).
model = "grok-4.7"

[client]
base_url = "https://api.x.ai/v1"
api_key_var = "XAI_API_KEY"

[sampling]
temperature = 0.0
max_tokens = 8192
reasoning_effort = "low"
"""
)
harness = rr / "prime_v1" / "angel_action_v1" / "harness.py"
text = harness.read_text()
if "grok-direct" not in text:
    needle = '"glm-direct",\n        "sota-moa-longcat",'
    alt = '"glm-direct",\n            "sota-moa-longcat",'
    if needle not in text or alt not in text:
        raise SystemExit("route literal markers missing")
    text = text.replace(needle, '"glm-direct",\n        "grok-direct",\n        "sota-moa-longcat",', 1)
    text = text.replace(alt, '"glm-direct",\n            "grok-direct",\n            "sota-moa-longcat",', 1)
    old_direct = '{"longcat-direct", "deepseek-direct", "glm-direct"}'
    new_direct = '{"longcat-direct", "deepseek-direct", "glm-direct", "grok-direct"}'
    if old_direct not in text:
        raise SystemExit("direct-route set missing")
    text = text.replace(old_direct, new_direct, 1)
    old_roles = '''        elif self.config.route == "glm-direct":
            roles = "glm"
        else:
            roles = "longcat"'''
    new_roles = '''        elif self.config.route == "glm-direct":
            roles = "glm"
        elif self.config.route == "grok-direct":
            roles = "grok"
        else:
            roles = "longcat"'''
    if old_roles not in text:
        raise SystemExit("roles block missing")
    text = text.replace(old_roles, new_roles, 1)
    old_else = '''            if self.config.moa_max_tokens is not None:
                env["ANGEL_GLM_MAX_TOKENS"] = str(self.config.moa_max_tokens)
        else:
            env.update(
                {
                    "ANGEL_DRIVER": "sota-moa",'''
    new_else = '''            if self.config.moa_max_tokens is not None:
                env["ANGEL_GLM_MAX_TOKENS"] = str(self.config.moa_max_tokens)
        elif self.config.route == "grok-direct":
            # API-key seat. Fresh HOME has no ~/.grok OAuth, so this must be
            # grok-api (not the OAuth alias "grok"). The proxy secret is the
            # agent credential; XAI_API_KEY stays on the eval process.
            env.update(
                {
                    "ANGEL_DRIVER": "grok-api",
                    "ANGEL_API_CLUBS": "grok",
                    "ANGEL_GROK_API_URL": endpoint,
                    "ANGEL_GROK_URL": endpoint,
                    "ANGEL_GROK_KEY": secret,
                    "ANGEL_GROK_API_MODEL": model,
                    "ANGEL_GROK_MODEL": model,
                    "ANGEL_GROK_REASONING_EFFORT": "low",
                    "ANGEL_GROK_RESEARCH": "0",
                    "ANGEL_SOTA_MOA_GROK_RESEARCH": "0",
                    "ANGEL_HTTP_TIMEOUT": "180",
                    "ANGEL_HTTP_RETRIES": "1",
                }
            )
            if self.config.moa_max_tokens is not None:
                env["ANGEL_GROK_MAX_TOKENS"] = str(self.config.moa_max_tokens)
        else:
            env.update(
                {
                    "ANGEL_DRIVER": "sota-moa",'''
    if old_else not in text:
        raise SystemExit("glm/sota launch block missing")
    text = text.replace(old_else, new_else, 1)
    harness.write_text(text)
    print("patched harness with grok-direct")
else:
    print("harness already has grok-direct")
    pin = '"ANGEL_STREAM_USAGE": "1"'
    if pin not in text:
        old = '"ANGEL_HTTP_RETRIES": "1",'
        if old not in text:
            raise SystemExit("cannot pin ANGEL_STREAM_USAGE; grok env block missing")
        text = text.replace(old, old + "\n                    " + pin + ",", 1)
        harness.write_text(text)
        print("pinned ANGEL_STREAM_USAGE")
PY

python3 -m py_compile "$RR/prime_v1/angel_action_v1/harness.py"

N=$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))))' "$TASKS")
BIN_SHA=$(sha256sum "$ANGEL_BIN" | cut -d' ' -f1)
if [[ -n "${ANGEL_COCKPIT_SOURCE_SHA256:-}" ]]; then
  SRC_SHA=$ANGEL_COCKPIT_SOURCE_SHA256
else
  SRC_SHA=$("$ANGEL_BIN" --build-info --json | python3 -c 'import json,re,sys; v=json.load(sys.stdin).get("cockpit_source_sha256") or ""; print(v if re.fullmatch(r"[0-9a-f]{64}", v) else "")')
fi

# OAuth bearer from the Grok CLI login. Printed fields are expiry only.
KEY_VALUE=$(python3 - <<'PY'
import json, sys
from datetime import datetime, timezone
from pathlib import Path
data = json.loads(Path.home().joinpath(".grok/auth.json").read_text())
entry = next(iter(data.values()))
token = entry.get("key") or ""
expires = entry.get("expires_at") or ""
print(f"grok_oauth_expires_at={expires}", file=sys.stderr)
if expires:
    exp = datetime.fromisoformat(expires.replace("Z", "+00:00"))
    left = (exp - datetime.now(timezone.utc)).total_seconds()
    print(f"grok_oauth_seconds_left={int(left)}", file=sys.stderr)
    if left < 600:
        sys.exit("grok oauth token expires in under 10 minutes")
if len(token) < 20:
    sys.exit("grok oauth token missing")
sys.stdout.write(token)
PY
)

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
  --env.agent.harness.reasoning-effort "$ANGEL_EFFORT"
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
OUT=$RR/${OUT_ROOT:-runs}/grok/angelx
RUN_DIR=$TAG-$(date -u +%Y%m%dT%H%M%SZ)
# Per-call wire records (club/wire_log.rs) land beside the traces: rollout
# workdirs are deleted after grading, so this is the only copy.
FLAGS+=(--env.agent.harness.env.ANGEL_WIRE_LOG_DIR "$OUT/$RUN_DIR/wire")
mkdir -p "$OUT" "$RR/logs"
printf '%s\n' "$OUT/$RUN_DIR" >"$RR/logs/grok-angelx-current-run.txt"

echo "cell model=grok-4.7 effort=low harness=angelx seed=$SEED n=$N wall=${WALL_SECS}s bin=$ANGEL_BIN out=$OUT/$RUN_DIR"
exec env -i \
  HOME=/home/frosty40 USER=frosty40 LOGNAME=frosty40 LANG=C.UTF-8 TERM=dumb \
  PATH="$AGENT_PATH" \
  "$KEY_VAR=$KEY_VALUE" \
  PRIME_AGENT_ISOLATION=fresh-home \
  PRIME_AGENT_PATH="$AGENT_PATH" \
  PRIME_REAL_HOME=/home/frosty40 \
  PRIME_AGENT_WALL_SECS=$WALL_SECS \
  PRIME_AGENT_GRADE_ANY_EXIT=1 \
  "$RR/prime_v1/.venv/bin/eval" angel-action-v1 @ "$CFG" \
  "${FLAGS[@]}" \
  -n "$N" -r 1 -c 1 --no-serve --no-rich --no-push \
  -o "$OUT" --run.dir "$RUN_DIR"
