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

# start_proxy <log>: proxy_log.py on a port the OS hands out, retried until it
# binds (a random port once collided with another session's proxy, and the
# harness then talked to that session's model). Sets PORT and PROXY.
start_proxy() {
  local log=$1 attempt
  for attempt in 1 2 3 4 5; do
    PORT=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
    seat_env "$PORT" || return 2
    PROXY_LOG=$log PROXY_PORT=$PORT python3 "$HERE/proxy_log.py" 2>"${log%.jsonl}.proxy.err" &
    PROXY=$!
    sleep 0.5
    kill -0 "$PROXY" 2>/dev/null && return 0
  done
  echo "proxy never bound a port" >&2
  return 2
}
