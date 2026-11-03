#!/usr/bin/env bash
set -euo pipefail
round_table_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
round_table_state="${XDG_STATE_HOME:-$HOME/.local/state}/round-table"
round_table_tunnel="$HOME/.local/opt/round-table/cloudflared"
"$round_table_tunnel" tunnel --no-autoupdate --url http://127.0.0.1:18791 2>&1 |
while IFS= read -r round_table_line; do
  printf '%s\n' "$round_table_line"
  if [[ "$round_table_line" =~ https://[a-z0-9-]+\.trycloudflare\.com ]]; then
    round_table_url="${BASH_REMATCH[0]}"
    /usr/bin/python3 "$round_table_dir/table.py" --state-dir "$round_table_state" public-url "$round_table_url" > "$round_table_state/shared-links.json"
    chmod 600 "$round_table_state/shared-links.json"
    for round_table_team in dots grok v muse; do
      /usr/bin/python3 "$round_table_dir/table.py" --state-dir "$round_table_state" invite "$round_table_team" > "$round_table_state/connect-$round_table_team.txt"
    done
  fi
done
