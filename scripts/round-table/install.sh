#!/usr/bin/env bash
set -euo pipefail
round_table_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
round_table_state="${XDG_STATE_HOME:-$HOME/.local/state}/round-table"
mkdir -p "$HOME/.local/bin" "$HOME/.config/systemd/user" "$round_table_state"
chmod 700 "$round_table_state"
if [[ -e "$HOME/.local/bin/round-table" || -L "$HOME/.local/bin/round-table" ]]; then
  cp -a "$HOME/.local/bin/round-table" "$HOME/.local/bin/round-table.bak.$(date +%s)"
fi
round_table_launcher="$(mktemp "$HOME/.local/bin/.round-table.XXXXXX")"
cat > "$round_table_launcher" <<EOF
#!/usr/bin/env bash
exec /usr/bin/python3 "$round_table_dir/table.py" "\$@"
EOF
chmod 755 "$round_table_launcher"
mv -f "$round_table_launcher" "$HOME/.local/bin/round-table"
if [[ -f "$HOME/.config/systemd/user/round-table.service" ]]; then
  cp -p "$HOME/.config/systemd/user/round-table.service" "$round_table_state/service.backup.$(date +%s)"
fi
cat > "$HOME/.config/systemd/user/round-table.service" <<EOF
[Unit]
Description=Local Round Table for Dots and Grok Bot
After=default.target

[Service]
Type=simple
ExecStart=/usr/bin/python3 "$round_table_dir/table.py" --state-dir "$round_table_state" serve
Restart=on-failure
RestartSec=2
UMask=0077
Environment=PYTHONDONTWRITEBYTECODE=1

[Install]
WantedBy=default.target
EOF
systemctl --user daemon-reload
systemctl --user enable --now round-table.service
"$HOME/.local/bin/round-table" invite dots > "$round_table_state/connect-dots.txt"
"$HOME/.local/bin/round-table" invite grok > "$round_table_state/connect-grok.txt"
"$HOME/.local/bin/round-table" invite v > "$round_table_state/connect-v.txt"
"$HOME/.local/bin/round-table" invite muse > "$round_table_state/connect-muse.txt"
"$HOME/.local/bin/round-table" links --base http://127.0.0.1:18791
