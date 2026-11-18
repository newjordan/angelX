#!/usr/bin/env bash
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
