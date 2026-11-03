#!/usr/bin/env bash
# Build a source-bound release pin from this (clean) checkout and install it as
# angelY's binary in $ANGELY_HOME/pin. The previous pin is kept as pin.prev/.
#   scripts/angely/pin.sh
set -euo pipefail
src="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/../.." && pwd)"
KIT="${ANGELY_HOME:-$src/.angelY}"
mkdir -p "$KIT"
cd "$src"
[[ -z "$(git status --porcelain -- cockpit vendor rust-toolchain.toml)" ]] \
  || { echo "pin.sh: $src has uncommitted build inputs; commit first so the pin is source-bound" >&2; exit 2; }
export TMPDIR="${ANGELY_TMPDIR:-${TMPDIR:-/tmp}}" RUSTUP_AUTO_INSTALL=0 ANGEL_BUILD_PROFILE=release
export ANGEL_BUILD_RUSTC="$(rustc -vV)"
export ANGEL_BUILD_SOURCE_SHA256="$(bash scripts/check/cockpit-source-digest.sh)"
cargo build --locked --release --no-default-features --manifest-path cockpit/Cargo.toml --bin angel --bin angel-sandbox
target="$(cargo metadata --format-version 1 --no-deps --manifest-path cockpit/Cargo.toml | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
new="$KIT/pin.new"; rm -rf "$new"; mkdir -p "$new"
cp "$target/release/angel" "$target/release/angel-sandbox" "$new/"
sha256sum "$new/angel" | cut -d' ' -f1 > "$new/angel.sha256"
git log -1 --format='%H %s' > "$new/COMMIT"
"$new/angel" --build-info --json > "$new/build-info.json"
grep -q '"cockpit_source_sha256":"unbound"' "$new/build-info.json" && { echo "pin.sh: binary is unbound" >&2; exit 1; }
[[ -d "$KIT/pin" ]] && { rm -rf "$KIT/pin.prev"; mv "$KIT/pin" "$KIT/pin.prev"; }
mv "$new" "$KIT/pin"
echo "pinned $(cut -c1-12 "$KIT/pin/angel.sha256") from $(cat "$KIT/pin/COMMIT")"
