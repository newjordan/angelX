#!/usr/bin/env bash
set -euo pipefail
round_table_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
for round_table_runtime in codex grok muse; do
  test -x "$HOME/.local/bin/$round_table_runtime"
done
/usr/bin/python3 "$round_table_dir/worker.py" --configure
cat > "$HOME/.config/systemd/user/round-table-worker@.service" <<EOF
[Unit]
Description=Round Table queued responder (%i)
After=round-table.service
Wants=round-table.service

[Service]
Type=simple
ExecStart=/usr/bin/python3 "$round_table_dir/worker.py" --team %i
Restart=on-failure
RestartSec=5
UMask=0077
Environment=PYTHONDONTWRITEBYTECODE=1
Environment=PATH=$HOME/.local/bin:/usr/local/bin:/usr/bin:/bin
KillMode=control-group

[Install]
WantedBy=default.target
EOF
systemctl --user daemon-reload
systemctl --user restart round-table
systemctl --user enable --now round-table-worker@v round-table-worker@grok round-table-worker@muse
