#!/usr/bin/env python3
"""Print the operator's Grok OAuth bearer from ~/.grok/auth.json (for
proxy_log.py's PROXY_KEY_CMD). Refuses only a missing or expired token: near
expiry the old token is still valid until grok_refresh.sh has angelX's own
OAuth seat refresh it."""
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

entry = next(iter(json.loads(Path.home().joinpath(".grok/auth.json").read_text()).values()))
token, expires = entry.get("key") or "", entry.get("expires_at") or ""
if expires:
    left = (datetime.fromisoformat(expires.replace("Z", "+00:00")) - datetime.now(timezone.utc)).total_seconds()
    print(f"grok_oauth_seconds_left={int(left)}", file=sys.stderr)
    if left <= 0:
        sys.exit("grok oauth token expired")
if len(token) < 20:
    sys.exit("grok oauth token missing")
sys.stdout.write(token)
