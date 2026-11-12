#!/usr/bin/env bash
# Fetch the Mage base weights with resumable curl.
# huggingface_hub stalled on Xet at 268 MB while raw HTTPS sustains ~28 MB/s.
set -uo pipefail
repo="microsoft/Mage-VL-Cowork"
dest="${1:?destination dir required}"
mkdir -p "$dest"
api="https://huggingface.co/api/models/$repo"
files=$(curl -fsSL "$api" | python3 -c '
import json,sys
d=json.load(sys.stdin)
for s in d.get("siblings",[]):
    n=s["rfilename"]
    if n.endswith((".safetensors",".json",".jinja",".txt",".py",".model")):
        print(n)
')
[ -z "$files" ] && { echo "no files listed"; exit 1; }
echo "$files" | while read -r name; do
  [ -z "$name" ] && continue
  out="$dest/$name"
  mkdir -p "$(dirname "$out")"
  url="https://huggingface.co/$repo/resolve/main/$name"
  for attempt in 1 2 3 4 5; do
    if curl -fsSL -C - --retry 3 --retry-delay 2 --max-time 1800 -o "$out" "$url"; then
      break
    fi
    echo "retry $attempt: $name" >&2
    sleep 3
  done
  [ -f "$out" ] && echo "ok $(stat -c%s "$out") $name"
done
echo "DONE $dest"
du -sh "$dest"
