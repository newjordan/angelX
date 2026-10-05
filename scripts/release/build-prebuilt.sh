#!/usr/bin/env bash
# build-prebuilt.sh — build the two binaries a prebuilt angelX package ships.
#
#   scripts/release/build-prebuilt.sh MANIFEST TARGET OUT_DIR
#
# MANIFEST is the source release manifest `npm run release:evidence` wrote for
# this checkout's commit. TARGET is linux-x86_64, linux-arm64 or macos-arm64.
# OUT_DIR receives angel, angel-sandbox and build.json. The binaries carry the
# manifest's source and resource identities, so `angel --build-info --json`
# names the exact source release they were built from.
#
# Linux links against glibc 2.31 through cargo-zigbuild, so one binary runs on
# Ubuntu 22.04, Debian 11, RHEL 9 and the rolling distros alike. A plain host
# build needs the build host's own glibc (2.43 on Arch) and runs almost
# nowhere else. zig and cargo-zigbuild must be on PATH. macOS builds natively
# on Apple silicon, for macOS 13 and later.
#
# Source cartridges are never compiled in. build.rs embeds every source
# cartridge under ~/.angelX/cartridges, and the builder's own cartridges are
# private, so ANGEL_CARTRIDGES points at an empty folder and the binaries are
# checked for the builder's cartridge path afterwards.
#
# RUSTY_V8_ARCHIVE may name the pinned rusty_v8 static library listed in
# release/supply-chain-policy.json; without it the v8 build script downloads it.
set -euo pipefail

usage() { echo "usage: build-prebuilt.sh MANIFEST linux-x86_64|linux-arm64|macos-arm64 OUT_DIR" >&2; exit 2; }
[ $# -eq 3 ] || usage
manifest="$1"
target="$2"
out="$3"
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
glibc_floor=2.31
macos_floor=13.0
case "$target" in
  linux-x86_64) triple=x86_64-unknown-linux-gnu ;;
  linux-arm64) triple=aarch64-unknown-linux-gnu ;;
  macos-arm64) triple=aarch64-apple-darwin ;;
  *) usage ;;
esac
case "$target" in
  linux-*) [ "$(uname -s)" = Linux ] || { echo "build-prebuilt: $target builds on Linux" >&2; exit 2; } ;;
  macos-*) [ "$(uname -s)" = Darwin ] || { echo "build-prebuilt: $target builds on macOS" >&2; exit 2; } ;;
esac

# The checkout must hold exactly the manifest's tree, with clean build inputs.
[ -f "$manifest" ] || { echo "build-prebuilt: no manifest at $manifest" >&2; exit 2; }
release_field() { python3 -c 'import json,sys; m=json.load(open(sys.argv[1])); v=m
for k in sys.argv[2].split("."): v=v[k]
print(v)' "$manifest" "$1"; }
source_sha=$(release_field source.cockpit_source_sha256)
resource_sha=$(release_field entries_manifest_sha256)
commit=$(release_field source.commit)
tree=$(release_field source.tree)
[ "$(git -C "$root" rev-parse 'HEAD^{tree}')" = "$tree" ] || {
  echo "build-prebuilt: this checkout is not the manifest's tree $tree (commit $commit)" >&2; exit 1; }
bash "$root/scripts/check/cockpit-source-digest.sh" >/dev/null

# The pinned toolchain's own cargo and rustc, never a rustup proxy that may be
# broken or start a download mid-build (see bin/angelX).
channel=$(awk -F'"' '/^channel[[:space:]]*=/{print $2; exit}' "$root/rust-toolchain.toml")
host_triple=$(rustc -vV 2>/dev/null | awk '/^host:/{print $2}' || true)
[ -n "$host_triple" ] || host_triple=$( [ "$(uname -s)" = Darwin ] && echo aarch64-apple-darwin || echo "$(uname -m)-unknown-linux-gnu")
toolchain_bin="$HOME/.rustup/toolchains/$channel-$host_triple/bin"
[ -x "$toolchain_bin/cargo" ] || { echo "build-prebuilt: Rust $channel is not installed (rustup toolchain install $channel)" >&2; exit 1; }
[ -d "$HOME/.rustup/toolchains/$channel-$host_triple/lib/rustlib/$triple" ] || {
  echo "build-prebuilt: Rust $channel lacks $triple (rustup target add $triple --toolchain $channel)" >&2; exit 1; }
export PATH="$toolchain_bin:$PATH"
export RUSTC="$toolchain_bin/rustc"
export RUSTUP_AUTO_INSTALL=0
export ANGEL_BUILD_SOURCE_SHA256="$source_sha"
export ANGEL_BUILD_RESOURCE_SHA256="$resource_sha"
export ANGEL_BUILD_RUSTC="$("$RUSTC" -vV)"
export ANGEL_BUILD_PROFILE=release

empty_cartridges=$(mktemp -d)
trap 'rm -rf "$empty_cartridges"' EXIT
export ANGEL_CARTRIDGES="$empty_cartridges"

cargo_args=(--locked --release --no-default-features --bin angel --bin angel-sandbox)
cd "$root/cockpit"
case "$target" in
  linux-*)
    command -v zig >/dev/null && command -v cargo-zigbuild >/dev/null || {
      echo "build-prebuilt: Linux targets need zig and cargo-zigbuild on PATH" >&2; exit 1; }
    cargo zigbuild "${cargo_args[@]}" --target "$triple.$glibc_floor"
    ;;
  macos-*)
    MACOSX_DEPLOYMENT_TARGET="$macos_floor" cargo build "${cargo_args[@]}" --target "$triple"
    ;;
esac

built="$root/cockpit/target/$triple/release"
mkdir -p "$out"
for name in angel angel-sandbox; do
  [ -x "$built/$name" ] || { echo "build-prebuilt: missing $built/$name" >&2; exit 1; }
  # A builder's own cartridge source leaves its absolute path in the binary.
  if LC_ALL=C grep -aqF "$HOME/.angelX/cartridges/" "$built/$name"; then
    echo "build-prebuilt: $name carries a cartridge from $HOME/.angelX/cartridges; refusing to package it" >&2
    exit 1
  fi
  cp -f "$built/$name" "$out/$name"
done

floor=""
case "$target" in
  linux-*)
    # The newest glibc symbol version either binary needs must not pass the floor.
    for name in angel angel-sandbox; do
      need=$(objdump -T "$out/$name" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -1)
      if [ "$(printf '%s\n%s\n' "$need" "$glibc_floor" | sort -V | tail -1)" != "$glibc_floor" ]; then
        echo "build-prebuilt: $name needs glibc $need, above the $glibc_floor floor" >&2
        exit 1
      fi
    done
    floor="glibc $glibc_floor"
    ;;
  macos-*) floor="macOS $macos_floor" ;;
esac

# Run what this host can run: the binary must report the release it was bound to.
if [ "$triple" = "$host_triple" ]; then
  reported=$("$out/angel" --build-info --json | python3 -c 'import json,sys; b=json.load(sys.stdin); print(b["cockpit_source_sha256"], b["resources"]["sha256"])')
  [ "$reported" = "$source_sha $resource_sha" ] || {
    echo "build-prebuilt: binary reports $reported, expected $source_sha $resource_sha" >&2; exit 1; }
fi

sha() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | awk '{print $1}'; }
version=$(release_field version)
python3 - "$out/build.json" <<PY
import json, sys
json.dump({
    "schema": "angelX-prebuilt-build/v1",
    "version": "$version",
    "target": "$target",
    "rust_target": "$triple",
    "runs_on": "$floor",
    "commit": "$commit",
    "cockpit_source_sha256": "$source_sha",
    "resources_sha256": "$resource_sha",
    "rustc": """$ANGEL_BUILD_RUSTC""".splitlines()[0],
    "features": "no default features (no video decoding)",
    "binaries": {"angel": "$(sha "$out/angel")", "angel-sandbox": "$(sha "$out/angel-sandbox")"},
}, open(sys.argv[1], "w"), indent=2)
open(sys.argv[1], "a").write("\n")
PY
echo "build-prebuilt: $target $version ($floor) → $out"
