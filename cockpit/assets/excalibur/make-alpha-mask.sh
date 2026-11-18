#!/bin/sh
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

magick -size "$(identify -format '%wx%h' "$source_gray")" xc:white "$source_gray" \
    -compose CopyOpacity -composite -define png:color-type=4 -depth 8 "$work/rise.png"

magick "$work/rise.png" -alpha extract "$work/alpha.png"
magick compare -metric AE "$work/alpha.png" "$source_gray" null: 2>"$work/ae" || true
ae=$(cat "$work/ae")
ae=${ae%% *}
ink=$(magick "$work/rise.png" -alpha off -format '%[fx:minima] %[fx:maxima]' info:)
desttype=$(identify -format '%[type]' "$work/rise.png")
if [ "$ae" != "0" ] || [ "$ink" != "1 1" ] || [ "$desttype" != "GrayscaleAlpha" ]; then
    echo "mask verification failed: alpha_diff=$ae ink=$ink type=$desttype" >&2
    exit 1
fi
mv "$work/rise.png" "$dest"
echo "$dest: $(identify -format '%wx%h' "$dest") graya, alpha==$source_gray ($ae differing pixels), ink=$ink"
