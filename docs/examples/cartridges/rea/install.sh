#!/bin/sh
# Plug the REA cartridge in: copy it to ~/.angelX/cartridges/rea (or $1),
# install the pinned rea-agents CLI beside it, and take REA's own skill from
# that package, so the skill always matches the CLI. REA_VERSION picks another
# release. Run it again to update; remove the folder to unplug.
set -eu
version="${REA_VERSION:-6.5.0}"
src=$(cd "$(dirname "$0")" && pwd -P)
dest="${1:-$HOME/.angelX/cartridges/rea}"
command -v npm >/dev/null 2>&1 || {
	echo "install.sh: REA needs Node.js (22.19+, 24.11+ or 26+) and npm" >&2
	exit 1
}
mkdir -p "$dest/skills" "$dest/bin"
dest=$(cd "$dest" && pwd -P)
if [ "$src" != "$dest" ]; then
	cp "$src/cartridge.toml" "$dest/cartridge.toml"
	rm -rf "$dest/skills/rea"
	cp -R "$src/skills/rea" "$dest/skills/rea"
fi
npm install --prefix "$dest/runtime" --no-audit --no-fund --omit=dev "rea-agents@$version"
rm -rf "$dest/skills/reverse-engineer-anything"
cp -R "$dest/runtime/node_modules/rea-agents/skills/reverse-engineer-anything" "$dest/skills/"
ln -sf ../runtime/node_modules/.bin/rea "$dest/bin/rea"
echo "rea $("$dest/bin/rea" --version) plugged in at $dest; in angelX: /rea <target> [question]"
