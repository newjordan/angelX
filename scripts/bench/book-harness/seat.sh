# seat.sh — sourced: seat_env <port> sets SEAT (the harness's env for the seat
# named by ROUTE) and exports PROXY_UPSTREAM / PROXY_KEY for proxy_log.py.
# Routes: local (needs PROXY_UPSTREAM, ANGEL_LOCAL_MODEL), glm-flash
# (ZAI_API_KEY), deepseek-flash and deepseek-messages (DEEPSEEK_API_KEY; the
# latter on DeepSeek's Messages API), muse (ANGEL_META_KEY), grok (grok-4.7 low
# through the operator's OAuth bearer, GROK_BEARER_CMD).
seat_env() {
  local url="http://127.0.0.1:$1/v1"
  case ${ROUTE:-local} in
    local) SEAT=(ANGEL_DRIVER=local ANGEL_LOCAL_URL="$url" ANGEL_LOCAL_MODEL="${ANGEL_LOCAL_MODEL:?set the model id}") ;;
    glm-flash)
      export PROXY_UPSTREAM=https://api.z.ai/api/coding/paas/v4 PROXY_KEY=${ZAI_API_KEY:?}
      SEAT=(ANGEL_DRIVER=glm-5.3-flash ANGEL_GLM_URL="$url" ANGEL_GLM_MODEL=glm-5.3-flash ANGEL_GLM_KEY="$PROXY_KEY") ;;
    deepseek-flash)
      export PROXY_UPSTREAM=https://api.deepseek.com/v1 PROXY_KEY=${DEEPSEEK_API_KEY:?}
      SEAT=(ANGEL_DRIVER=deepseek-flash ANGEL_DEEPSEEK_URL="$url" ANGEL_DEEPSEEK_KEY="$PROXY_KEY") ;;
    deepseek-messages)
      export PROXY_UPSTREAM=https://api.deepseek.com/anthropic/v1 PROXY_KEY=
      SEAT=(ANGEL_DRIVER=deepseek-flash ANGEL_DEEPSEEK_URL="$url" ANGEL_DEEPSEEK_API=messages
            ANGEL_DEEPSEEK_KEY="${DEEPSEEK_API_KEY:?}") ;;
    grok)
      # grok-4.7 at effort low on the grok-api seat; the proxy carries the
      # operator's Grok OAuth bearer (GROK_BEARER_CMD reads ~/.grok/auth.json),
      # the harness sees only a placeholder key.
      export PROXY_UPSTREAM=https://api.x.ai/v1 PROXY_KEY=
      export PROXY_KEY_CMD=${GROK_BEARER_CMD:?set GROK_BEARER_CMD to print the Grok OAuth bearer}
      SEAT=(ANGEL_DRIVER=grok-api ANGEL_GROK_URL="$url" ANGEL_GROK_KEY=proxy-held
            ANGEL_GROK_API_MODEL=grok-4.7 ANGEL_REASONING_EFFORT=low) ;;
    muse)
      export PROXY_UPSTREAM=https://api.meta.ai/v1 PROXY_KEY=${ANGEL_META_KEY:?}
      SEAT=(ANGEL_DRIVER=meta ANGEL_META_URL="$url" ANGEL_META_MODEL=muse-spark-1.3-contributor ANGEL_META_KEY="$PROXY_KEY") ;;
    *) echo "unknown ROUTE ${ROUTE}" >&2; return 2 ;;
  esac
}

# start_proxy <log>: proxy_log.py binds its own ephemeral port, then reports
# that exact listener through a private ready receipt. Sets PORT and PROXY.
start_proxy() {
  local log=$1 ready i
  seat_env 0 || return 2
  ready=$(mktemp "${RUN_TMP:-${TMPDIR:-/tmp}}/angel-proxy-ready.XXXXXX") || return 2
  PROXY_LOG=$log PROXY_PORT=0 PROXY_READY=$ready \
    python3 "$HERE/exec_session.py" python3 "$HERE/proxy_log.py" 2>"${log%.jsonl}.proxy.err" &
  PROXY=$!
  PORT=
  for ((i=0; i<50; i++)); do
    if ! kill -0 "$PROXY" 2>/dev/null; then break; fi
    if [[ -s "$ready" ]]; then
      read -r PORT < "$ready"
      [[ "$PORT" =~ ^[0-9]+$ ]] && break
    fi
    sleep 0.1
  done
  rm -f -- "$ready" "$ready.tmp"
  if [[ "$PORT" =~ ^[0-9]+$ ]]; then
    seat_env "$PORT" || return 2
    return 0
  fi
  kill "$PROXY" 2>/dev/null || true
  wait "$PROXY" 2>/dev/null || true
  echo "proxy did not become ready; see ${log%.jsonl}.proxy.err" >&2
  return 2
}
