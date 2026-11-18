#!/usr/bin/env bash
set -euo pipefail
name=${1:?name}; src=${2:?source}; start=${3:-}; end=${4:-}
root=$(cd "$(dirname "$0")/../.." && pwd)
out="$root/cockpit/assets/dungeon/sfx/$name.mp3"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/sfx.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
trim=()
[ -n "$start" ] && trim+=(-ss "$start")
[ -n "$end" ] && trim+=(-to "$end")
ffmpeg -v error -y "${trim[@]}" -i "$src" -ac 1 -ar 44100 "$tmp/cut.wav"
len=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$tmp/cut.wav")
peak=$(ffmpeg -v info -i "$tmp/cut.wav" -af volumedetect -f null - 2>&1 | sed -n 's/.*max_volume: \(-\?[0-9.]*\) dB.*/\1/p')
gain=$(python3 -c "print(float('${PEAK:--2}') - float('$peak'))")
fade=$(python3 -c "print(max(0.0, float('$len') - 0.06))")
ffmpeg -v error -y -i "$tmp/cut.wav" \
  -af "volume=${gain}dB,afade=t=in:d=0.005,afade=t=out:st=${fade}:d=0.06" \
  -c:a libmp3lame -b:a 96k "$tmp/out.mp3"
if [ -f "$out" ]; then
  mkdir -p "${TMPDIR:-/tmp}/sfx-replaced"
  cp "$out" "${TMPDIR:-/tmp}/sfx-replaced/$name-$(date +%Y%m%d-%H%M%S).mp3"
fi
cp "$tmp/out.mp3" "$out"
echo "$name.mp3: $(printf '%.2f' "$len") s, peak moved ${gain} dB -> $out"
