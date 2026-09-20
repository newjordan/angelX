#!/usr/bin/env bash
# Local ordinary-cockpit gate. Dependencies must already be available offline.
set -euo pipefail

quality_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"

# Reject forbidden active dependencies before Cargo walks the module graph.
bash "$quality_root/scripts/check-legacy-terminal-boundary.sh"
python3 "$quality_root/scripts/check-active-connections.py"
cargo fmt --manifest-path "$quality_root/cockpit/Cargo.toml" --check
cargo clippy --locked --offline --manifest-path "$quality_root/cockpit/Cargo.toml" \
  --all-targets -- -D warnings -A dead-code -A clippy::type-complexity
