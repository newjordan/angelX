#!/bin/sh
# Rebuild the shipped Excalibur atlas as a white-ink alpha mask.
#
# `rise.png` is never displayed: playback turns it into dot animation. It is
# therefore stored the way that animation actually looks — white ink on a
# transparent background — instead of a gray frame with an opaque black plate.
# The transform is exact and reversible: the ink plane is 255 everywhere and the
# alpha plane carries the frame luminance byte for byte, so the dot pipeline
# (crop, fit, level curve, dither) reads the identical levels it always did.
#
# Usage: make-alpha-mask.sh <gray-atlas.png>
#
# The source must be the 8-bit grayscale tile the ffmpeg command in README.md
# writes. The mask is written to `rise.png` next to this script and verified
# against the source before this script reports success.
set -eu

source_gray=${1:-}
if [ -z "$source_gray" ] || [ ! -f "$source_gray" ]; then
    echo "usage: $(basename "$0") <gray-atlas.png>" >&2
    exit 2
fi

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
dest="$here/rise.png"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

srctype=$(identify -format '%[type]' "$source_gray")
if [ "$srctype" != "Grayscale" ]; then
    echo "$source_gray: expected an 8-bit grayscale PNG" >&2
    exit 1
fi

# White ink, alpha copied straight from the grayscale plane.
magick -size "$(identify -format '%wx%h' "$source_gray")" xc:white "$source_gray" \
    -compose CopyOpacity -composite -define png:color-type=4 -depth 8 "$work/rise.png"

# Verify: the transported mask must be exactly the luminance the pipeline reads,
# with a uniform white ink plane and no opaque plate left anywhere.
magick "$work/rise.png" -alpha extract "$work/alpha.png"
magick compare -metric AE "$work/alpha.png" "$source_gray" null: 2>"$work/ae" || true
ae=$(cat "$work/ae")
# `compare` reports `<normalized> (<absolute>)`; the absolute count leads.
ae=${ae%% *}
ink=$(magick "$work/rise.png" -alpha off -format '%[fx:minima] %[fx:maxima]' info:)
desttype=$(identify -format '%[type]' "$work/rise.png")
if [ "$ae" != "0" ] || [ "$ink" != "1 1" ] || [ "$desttype" != "GrayscaleAlpha" ]; then
    echo "mask verification failed: alpha_diff=$ae ink=$ink type=$desttype" >&2
    exit 1
fi
mv "$work/rise.png" "$dest"
echo "$dest: $(identify -format '%wx%h' "$dest") graya, alpha==$source_gray ($ae differing pixels), ink=$ink"
