#!/usr/bin/env bash
set -euo pipefail

quality_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"

bash "$quality_root/scripts/check/check-legacy-terminal-boundary.sh"
python3 "$quality_root/scripts/check/check-active-connections.py"
cargo fmt --manifest-path "$quality_root/cockpit/Cargo.toml" --check
cargo clippy --locked --offline --manifest-path "$quality_root/cockpit/Cargo.toml" \
  --all-targets -- -D warnings -A dead-code -A clippy::type-complexity
