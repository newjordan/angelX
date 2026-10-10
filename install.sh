#!/bin/sh
# angelX installer: downloads the prebuilt angelX for this machine and puts the
# `angelX` command on your PATH. No Rust, compiler or root access needed.
#
#   curl -fsSL https://raw.githubusercontent.com/newjordan/angelX/main/install.sh | sh
#
# Settings (environment, all optional):
#   ANGELX_VERSION=v0.2.0   install that release instead of the latest
#   ANGELX_HOME=~/.angelX    where releases are unpacked
#   ANGELX_BIN_DIR=~/.local/bin
#                            where the angelX command is linked
#   ANGELX_ARCHIVE=FILE      install a package file you already downloaded
#
# Each release unpacks into ~/.angelX/releases/<version>-<platform>, and
# ~/.angelX/current points at the active one. Running this again upgrades in
# place. Your keys (~/.angelX/angel.env), sessions and cartridges stay put.
set -eu

repo="newjordan/angelX"
home_dir="${ANGELX_HOME:-$HOME/.angelX}"
bin_dir="${ANGELX_BIN_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
die() { printf 'angelX install: %s\n' "$*" >&2; exit 1; }

# --- platform --------------------------------------------------------------
os=$(uname -s)
arch=$(uname -m)
case "$os/$arch" in
  Linux/x86_64 | Linux/amd64) platform=linux-x86_64 ;;
  Linux/aarch64 | Linux/arm64) platform=linux-arm64 ;;
  Darwin/arm64) platform=macos-arm64 ;;
  Darwin/x86_64)
    # An Apple-silicon Mac running this shell under Rosetta still gets arm64.
    if [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 1 ]; then
      platform=macos-arm64
    else
      die "no prebuilt angelX for Intel Macs yet. Build from source: https://github.com/$repo#readme"
    fi
    ;;
  *) die "no prebuilt angelX for $os $arch. Build from source: https://github.com/$repo#readme" ;;
esac

if [ "$os" = Linux ]; then
  # The Linux build needs glibc 2.31 or newer (Ubuntu 22.04, Debian 11, RHEL 9
  # and later). musl systems such as Alpine have no GNU_LIBC_VERSION.
  glibc=$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{print $2}')
  [ -n "$glibc" ] || die "this Linux has no glibc (musl?). Build from source: https://github.com/$repo#readme"
  lowest=$(printf '%s\n%s\n' "$glibc" 2.31 | sort -t. -k1,1n -k2,2n | head -n 1)
  [ "$lowest" = 2.31 ] || die "glibc $glibc is older than the 2.31 the prebuilt needs. Build from source: https://github.com/$repo#readme"
fi

command -v bash >/dev/null 2>&1 || die "angelX needs bash"
command -v tar >/dev/null 2>&1 || die "angelX install needs tar"

fetch() { # url file
  if command -v curl >/dev/null 2>&1; then
    curl -fL --proto '=https' --tlsv1.2 --retry 3 --progress-bar -o "$2" "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$2" "$1"
  else
    die "angelX install needs curl or wget"
  fi
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    die "angelX install needs sha256sum or shasum to check the download"
  fi
}

# --- download and check ----------------------------------------------------
asset="angelX-$platform.tar.gz"
mkdir -p "$home_dir/releases"
work=$(mktemp -d "$home_dir/releases/.install.XXXXXX")
trap 'rm -rf "$work"' EXIT
trap 'exit 1' INT TERM

if [ -n "${ANGELX_ARCHIVE:-}" ]; then
  [ -f "$ANGELX_ARCHIVE" ] || die "no package at $ANGELX_ARCHIVE"
  cp "$ANGELX_ARCHIVE" "$work/$asset"
  say "Installing angelX from $ANGELX_ARCHIVE"
else
  if [ -n "${ANGELX_VERSION:-}" ]; then
    base="https://github.com/$repo/releases/download/$ANGELX_VERSION"
  else
    base="https://github.com/$repo/releases/latest/download"
  fi
  say "Downloading angelX for $platform…"
  fetch "$base/$asset" "$work/$asset" || die "download failed: $base/$asset"
  fetch "$base/SHA256SUMS" "$work/SHA256SUMS" || die "download failed: $base/SHA256SUMS"
  want=$(awk -v f="$asset" '$2 == f || $2 == "*" f {print $1}' "$work/SHA256SUMS")
  [ -n "$want" ] || die "SHA256SUMS lists no $asset"
  [ "$(sha256_of "$work/$asset")" = "$want" ] || die "checksum mismatch for $asset; nothing was installed"
fi

# --- unpack and switch -----------------------------------------------------
mkdir "$work/unpack"
tar -xzf "$work/$asset" -C "$work/unpack"
top=$(ls "$work/unpack")
[ "$(printf '%s\n' "$top" | wc -l | tr -d ' ')" = 1 ] && [ -f "$work/unpack/$top/PREBUILT.json" ] \
  || die "$asset is not an angelX prebuilt package"
pkg="$work/unpack/$top"
version=$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$pkg/PREBUILT.json" | head -n 1)
[ -n "$version" ] || die "$asset does not say its version"

# Prove the binary runs here before replacing anything.
"$pkg/cockpit/target/release/angel" --version >/dev/null 2>&1 \
  || die "the $platform binary does not run on this machine; nothing was installed"

dest="$home_dir/releases/$version-$platform"
rm -rf "$dest"
mv "$pkg" "$dest"
if [ "$os" = Darwin ] && command -v xattr >/dev/null 2>&1; then
  xattr -dr com.apple.quarantine "$dest" 2>/dev/null || true
fi
touch "$dest"
ln -sfn "releases/$version-$platform" "$home_dir/current"

mkdir -p "$bin_dir"
ln -sfn "$home_dir/current/bin/angelX" "$bin_dir/angelX"
ln -sfn "$home_dir/current/bin/angelX" "$bin_dir/angelx"

# Keep the two newest releases; older ones only take space.
ls -1dt "$home_dir/releases/"*-"$platform" 2>/dev/null | tail -n +3 | while IFS= read -r old; do
  [ "$old" = "$dest" ] || rm -rf "$old"
done

say ""
say "angelX $version is installed."
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *)
    say ""
    say "Add $bin_dir to your PATH first:"
    case "${SHELL:-}" in
      */zsh) say "  echo 'export PATH=\"$bin_dir:\$PATH\"' >> ~/.zshrc && exec zsh" ;;
      */fish) say "  fish_add_path $bin_dir" ;;
      *) say "  echo 'export PATH=\"$bin_dir:\$PATH\"' >> ~/.bashrc && exec bash" ;;
    esac
    ;;
esac
say ""
say "Then start it inside a project:"
say "  cd your-project && angelX"
for tool in git python3; do
  command -v "$tool" >/dev/null 2>&1 || say "(angelX works best with $tool installed.)"
done
