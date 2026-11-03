#!/usr/bin/env bash
# Copy a standalone tool cartridge, build it, then let angelX discover it next start.
set -euo pipefail
src=$(cd "$(dirname "$0")" && pwd -P)
dest="${1:-$HOME/.angelX/cartridges/vfx}"
mkdir -p "$dest"
dest=$(cd "$dest" && pwd -P)
if [ "$src" != "$dest" ]; then
  for path in bin lib bend skills examples licenses cartridge.toml package.json README.md LICENSE THIRD_PARTY_NOTICES.md build.sh toolchain.py; do
    cp -R "$src/$path" "$dest/"
  done
fi
"$dest/build.sh"
python3 - "$dest" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
config = {'mcpServers': {'vfx': {'command': str(root / 'bin/vfx'), 'args': ['mcp']}}}
(root / 'mcp.example.json').write_text(json.dumps(config, indent=2) + '\n')
PY
"$dest/bin/vfx" doctor
echo "VFX cartridge ready at $dest; restart angelX and use /vfx <effect>." >&2
