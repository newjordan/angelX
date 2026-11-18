#!/usr/bin/env bash
set -u
BIN=$(cd "$1" && pwd); HERE=$(cd "$(dirname "$0")" && pwd)
while :; do
  left=$(python3 "$HERE/grok_bearer.py" 2>&1 >/dev/null | sed -n 's/grok_oauth_seconds_left=//p')
  if [ -n "$left" ] && [ "$left" -lt 100 ]; then
    ws=$(mktemp -d)
    ANGEL_DRIVER=grok "$BIN/angel" --task-json --workspace "$ws" --max-hops 1 "Reply with OK." >/dev/null 2>&1
    echo "$(date -Is) refreshed (was ${left}s left): $(python3 "$HERE/grok_bearer.py" 2>&1 >/dev/null)"
    rm -rf "$ws"
  fi
  sleep 30
done
