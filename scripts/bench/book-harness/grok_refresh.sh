#!/usr/bin/env bash
# grok_refresh.sh <angel-bin-dir> — keep the operator's Grok OAuth token fresh
# for long benches: every 30 s, when under 100 s remain, one minimal call
# through angelX's own Grok OAuth seat (which refreshes within 120 s of expiry
# and persists the new token to ~/.grok/auth.json). Runs until killed.
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
