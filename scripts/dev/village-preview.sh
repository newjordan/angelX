#!/bin/sh
# Launch an isolated, persistent local village. Optional first argument: Run JSON.
# Build prerequisites are the native dev host and crates/web/dist/delve-village.
set -eu

village_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
village_host=${VILLAGE_HOST:-/tmp/angelx-preview-native-target/debug/delve-dev-host}
village_bundle=${VILLAGE_BUNDLE:-$village_root/crates/web/dist/delve-village}
village_state=${VILLAGE_STATE:-/tmp/angelx-village-session}
village_port=${VILLAGE_PORT:-8820}

if [ ! -x "$village_host" ] || [ ! -f "$village_bundle/delve-village.wasm" ]; then
    echo "Build delve-dev-host and the delve-village browser bundle first." >&2
    echo "Set VILLAGE_HOST / VILLAGE_BUNDLE to use other build locations." >&2
    exit 1
fi
if [ "$#" -gt 1 ]; then
    echo "Usage: $0 [snapshot.json]" >&2
    exit 1
fi
village_snapshot=${1:-}
set -- --bind 127.0.0.1 --port "$village_port" --bundle "$village_bundle" \
    --scene night --night-dir "$village_state"
if [ -n "$village_snapshot" ] && [ ! -f "$village_state/run.json" ]; then
    set -- "$@" --snapshot "$village_snapshot"
fi
echo "Village saves: $village_state (an existing save resumes unchanged)."
exec "$village_host" "$@"
