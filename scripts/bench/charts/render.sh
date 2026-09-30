#!/usr/bin/env bash
# render.sh <data.json> <out-dir> — input tokens against agent time, pass and fail in the
# line color, in the site dot-matrix look: a 1200x1200 square (for posts) and a 1600x900 wide (for the
# README), both at 2x. Data from extract_tokens_time.py.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd); data=$1; out=$2; tmp=$(mktemp -d)
python3 -c 'import sys; open(sys.argv[3],"w").write(open(sys.argv[1]).read().replace("/*DATA*/null", open(sys.argv[2]).read()))' \
  "$here/tokens-time.html" "$data" "$tmp/chart.html"
for kind in square:1200,1200 wide:1600,900; do
  name=${kind%%:*}; size=${kind#*:}
  chromium --headless=new --disable-gpu --no-sandbox --hide-scrollbars --force-device-scale-factor=2 \
    --virtual-time-budget=8000 --window-size="$size" --screenshot="$out/tokens-time-deepseek-$name.png" "file://$tmp/chart.html#$name" >/dev/null 2>&1
done
echo "rendered to $out"
