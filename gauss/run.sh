#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
# Reuse this checkout's existing build cache unless the caller chose another.
if [[ -z "${CARGO_TARGET_DIR:-}" && -d ../cockpit/target ]]; then
    export CARGO_TARGET_DIR="$(cd ../cockpit/target && pwd -P)"
fi
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
exec cargo run --offline -- "$@"
