#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
python3 "$here/build.py"
exec timeout 900 "$HOME/.cache/angel/bend2/221a350c2f7f7134884740018e6e90001fb6d230/bin/bend" "$here/main.bend" -o "$1"
