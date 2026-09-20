#!/usr/bin/env bash
# cockpit-source-digest.sh — the source-identity digest that gets baked into the
# cockpit binary as ANGEL_BUILD_SOURCE_SHA256 and read back by
# `angel --build-info --json` as "cockpit_source_sha256".
#
# Current scheme (2026-09-12): angel-source-v2, covering both path dependencies
# and all three telemetry files embedded by Rust. Hash the scheme marker and
# ordered Git object IDs. --legacy-commit explicitly retains the incomplete
# 2026-09-08 scheme for historical inspection, never current-source binding.
# --scheme reports the authoritative inventory version to receipt producers.
#
# Historical convention (2026-09-08, content-bound): sha256 over the git object ids of the
# build inputs — `git rev-parse <commit>:cockpit <commit>:vendor/dotmax
# <commit>:rust-toolchain.toml | sha256sum` — so the digest changes only when
# those trees/blobs change. The 2026-09-07 scheme hashed `git archive` output,
# but git archive embeds the commit id (pax "comment" header) and the commit
# time, so every commit — docs included — still changed the baked value, forced
# a cockpit recompile at the next launch and made cohort drivers report a
# "drifting" binary after unrelated commits. A reviewer checks a v1 binary with:
#
#   git rev-parse <commit>:cockpit <commit>:vendor/dotmax <commit>:rust-toolchain.toml | sha256sum
#   <binary> --build-info --json      # cockpit_source_sha256 must equal it
#
# Binaries baked before 2026-09-08 19:40Z carry the old archive-based value;
# verify those with `git archive --format=tar <commit> -- <inputs> | sha256sum`.
#
# Usage:
#   scripts/cockpit-source-digest.sh              digest of HEAD, only if the build
#                                                 inputs are clean (exit 3 otherwise)
#   scripts/cockpit-source-digest.sh --commit X   v2 digest of any commit, no clean test
#   scripts/cockpit-source-digest.sh --legacy-commit X  historical v1 digest only
#   scripts/cockpit-source-digest.sh --dirty      list the dirty build-input paths
#
# Current build inputs are the explicit BUILD_INPUTS inventory below, including
# cockpit/Cargo.lock, both vendored path dependencies and embedded telemetry. A
# dirty path means the binary would not match HEAD, so no digest is printed and
# the caller builds unbound. Exit codes: 0 digest on stdout; 2 not a git
# checkout; 3 dirty build inputs (one explanatory line on stderr).
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
LEGACY_BUILD_INPUTS=(cockpit vendor/dotmax rust-toolchain.toml)
BUILD_INPUTS=(cockpit vendor/dotmax vendor/ureq rust-toolchain.toml
  docs/telemetry/model-calibration.toml docs/telemetry/store-caps.toml
  docs/telemetry/prices.toml)

source_digest() {
  local revision="$1"
  local object_ids
  local revs=()
  for input in "${BUILD_INPUTS[@]}"; do revs+=("$revision:$input"); done
  object_ids=$(git -C "$root" rev-parse "${revs[@]}") || return
  printf 'angel-source-v2\n%s\n' "$object_ids" | sha256sum | cut -d' ' -f1
}

if ! git -C "$root" rev-parse --verify HEAD >/dev/null 2>&1; then
  echo "build is unbound: $root is not a git checkout with a HEAD commit" >&2
  exit 2
fi

case "${1:-}" in
  --scheme)
    echo angel-source-v2
    exit 0
    ;;
  --legacy-commit)
    [ -n "${2:-}" ] || { echo "usage: $0 --legacy-commit <rev>" >&2; exit 64; }
    revs=(); for input in "${LEGACY_BUILD_INPUTS[@]}"; do revs+=("$2:$input"); done
    git -C "$root" rev-parse "${revs[@]}" | sha256sum | cut -d' ' -f1
    exit 0
    ;;
  --commit)
    [ -n "${2:-}" ] || { echo "usage: $0 --commit <rev>" >&2; exit 64; }
    source_digest "$2"
    exit 0
    ;;
  --dirty)
    git -C "$root" status --porcelain --untracked-files=all -- "${BUILD_INPUTS[@]}"
    exit 0
    ;;
  '') ;;
  *) echo "usage: $0 [--commit <rev> | --legacy-commit <rev> | --scheme | --dirty]" >&2; exit 64 ;;
esac

dirty=$(git -C "$root" status --porcelain --untracked-files=all -- "${BUILD_INPUTS[@]}" | wc -l | tr -d ' ')
if [ "$dirty" != "0" ]; then
  echo "build is unbound: $dirty dirty path(s) under ${BUILD_INPUTS[*]} (commit or stash them for a source-bound build; $0 --dirty lists them)" >&2
  exit 3
fi
source_digest HEAD
