#!/usr/bin/env bash
# Put a sound in for one of the Delve's effects.
#
#     scripts/delve/sfx_replace.sh <name> <source> [start-seconds] [end-seconds]
#
# Trims the source to [start, end] (the whole file when left out), fades in
# 5 ms and out 60 ms, and writes it as the effects are kept: mono 44.1 kHz,
# 96 kbit/s mp3, its peak at -2 dB like door_open's (PEAK=<dB> for a sound the
# set keeps quieter, such as kill at -7.7). The result goes to
# cockpit/assets/dungeon/sfx/<name>.mp3, which the mixer reads every time it
# plays a sound, so a running game hears it on the next one. The file it
# replaces is copied to $TMPDIR/sfx-replaced/ first.
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
