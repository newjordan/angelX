#!/bin/sh
# Plug the DigUp cartridge in: copy it to ~/.angelX/cartridges/dig (or $1),
# unpack the pinned release's command-line helper (digup, llama.cpp, PDFium,
# ffmpeg) beside it, and fetch the model (865 MB, once). DIG_HOME keeps the
# helper, the model and the index somewhere roomier, linked from the
# cartridge. Run it again to update; remove the folder to unplug.
set -eu
version="0.7.0-beta.2"
sha256="b5a2b9ccf710fe54065bedb6227e877d1e8a550de8e1fbff5e139aef9b36dbd0"
src=$(cd "$(dirname "$0")" && pwd -P)
dest="${1:-$HOME/.angelX/cartridges/dig}"
[ "$(uname -s)-$(uname -m)" = Linux-x86_64 ] || {
	echo "install.sh: this pin is DigUp's Linux x86_64 helper" >&2
	exit 1
}
for tool in curl ar tar sha256sum; do
	command -v "$tool" >/dev/null 2>&1 || {
		echo "install.sh: needs $tool" >&2
		exit 1
	}
done
mkdir -p "$dest/skills" "$dest/bin"
dest=$(cd "$dest" && pwd -P)
if [ "$src" != "$dest" ]; then
	cp "$src/cartridge.toml" "$dest/cartridge.toml"
	cp "$src/bin/digup" "$src/bin/dig-search" "$dest/bin/"
	rm -rf "$dest/skills/dig"
	cp -R "$src/skills/dig" "$dest/skills/dig"
fi
chmod +x "$dest/bin/digup" "$dest/bin/dig-search"
for part in runtime models index; do
	if [ -n "${DIG_HOME:-}" ]; then
		mkdir -p "$DIG_HOME/$part"
		[ -L "$dest/$part" ] || [ ! -e "$dest/$part" ] || {
			echo "install.sh: $dest/$part exists; move it to $DIG_HOME/$part first" >&2
			exit 1
		}
		ln -sfn "$(cd "$DIG_HOME/$part" && pwd -P)" "$dest/$part"
	else
		mkdir -p "$dest/$part"
	fi
done
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
deb="DigUp_${version}_amd64.deb"
curl -fsSL -o "$work/$deb" "https://github.com/ARahim3/DigUp/releases/download/v$version/$deb"
echo "$sha256  $work/$deb" | sha256sum -c --quiet
(cd "$work" && ar x "$deb" && mkdir data && tar -xf data.tar.* -C data)
rm -rf "$dest/runtime/helper"
cp -R "$work/data/usr/lib/DigUp/helper" "$dest/runtime/helper"
cp "$work/data/usr/lib/DigUp/THIRD-PARTY-NOTICES.txt" "$dest/runtime/"
"$dest/bin/digup" download-model --to "$dest/models" >/dev/null
"$dest/bin/digup" speed --model --if-needed >/dev/null
echo "DigUp $version plugged in at $dest"
echo "index folders with: $dest/bin/digup index <folder>...  (estimate first: digup estimate <folder>...)"
echo "in angelX: /dig <what you're looking for>"
