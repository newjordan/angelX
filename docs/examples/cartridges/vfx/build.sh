#!/usr/bin/env bash
set -euo pipefail
src=$(cd "$(dirname "$0")" && pwd -P)
command -v clang >/dev/null || { echo "vfx needs clang 14+ to compile the native Bend renderer" >&2; exit 1; }
command -v node >/dev/null || { echo "vfx needs Node.js 22+" >&2; exit 1; }
node --input-type=module -e 'if(Number(process.versions.node.split(".")[0])<22)process.exit(1)'
bend_bin="${BEND:-}"
if [ -z "$bend_bin" ]; then
  if command -v bend >/dev/null && [ "$(bend version)" = "bend 2.0.39" ]; then
    bend_bin=$(command -v bend)
  else
    bend_bin=$(python3 "$src/toolchain.py")
  fi
fi
[ "$("$bend_bin" version)" = "bend 2.0.39" ] || { echo "vfx pins Bend 2.0.39; set BEND to that version" >&2; exit 1; }
mkdir -p "$src/.build"
temporary=$(mktemp -d "$src/.build/compile.XXXXXX")
trap 'rm -rf "$temporary"' EXIT
"$bend_bin" "$src/bend/engine.bend" --check-only
"$bend_bin" "$src/bend/engine.bend" -o "$temporary/vfx-render"
mv "$temporary/vfx-render" "$src/.build/vfx-render"
python3 - "$src" <<'PY'
import hashlib, pathlib, sys
root = pathlib.Path(sys.argv[1])
(root / '.build/source.sha256').write_text(hashlib.sha256((root / 'bend/engine.bend').read_bytes()).hexdigest() + '\n')
PY
echo "Built Bend 2.0.39 renderer at $src/.build/vfx-render" >&2
