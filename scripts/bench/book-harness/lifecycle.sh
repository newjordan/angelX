STUB=
PROXY=
TASK=
LOGGER=

stop_group() {
  local owned=$1
  [[ "$owned" =~ ^[1-9][0-9]*$ ]] || return 0
  kill -TERM -- "-$owned" 2>/dev/null || true
  kill -TERM "$owned" 2>/dev/null || true
  for ((j=0; j<20; j++)); do
    if ! kill -0 -- "-$owned" 2>/dev/null && ! kill -0 "$owned" 2>/dev/null; then break; fi
    sleep 0.025
  done
  kill -KILL -- "-$owned" 2>/dev/null || true
  kill -KILL "$owned" 2>/dev/null || true
  wait "$owned" 2>/dev/null || true
}

stop_task() {
  [[ -n "$TASK" ]] || return 0
  stop_group "$TASK"
  TASK=
}

finish_logger() {
  [[ -n "$LOGGER" ]] || return 0
  for ((j=0; j<20; j++)); do
    if ! kill -0 "$LOGGER" 2>/dev/null; then break; fi
    sleep 0.05
  done
  kill -KILL "$LOGGER" 2>/dev/null || true
  wait "$LOGGER" 2>/dev/null || true
  LOGGER=
}

cleanup() {
  local service
  trap '' INT TERM
  stop_task
  finish_logger
  for service in "$STUB" "${PROXY:-}"; do
    [[ -z "$service" ]] || stop_group "$service"
  done
  rm -rf -- "$RUN_TMP"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
