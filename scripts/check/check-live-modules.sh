#!/usr/bin/env bash
# The small live-module loop uses the repo's compiler, even when a shell's
# cargo/rustc shims prefer a different default toolchain.
set -euo pipefail
live_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
live_channel=$(awk -F'"' '/^channel[[:space:]]*=/{print $2; exit}' "$live_root/rust-toolchain.toml")
if command -v rustup >/dev/null 2>&1; then
  live_cargo=$(rustup which --toolchain "$live_channel" cargo)
  export RUSTC="$(rustup which --toolchain "$live_channel" rustc)"
  export RUSTDOC="$(rustup which --toolchain "$live_channel" rustdoc)"
  export RUSTUP_TOOLCHAIN="$live_channel"
else
  live_cargo=$(command -v cargo)
fi
export RUSTUP_AUTO_INSTALL=0
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$live_root/crates/target/live}"
exec "$live_cargo" test --locked --manifest-path "$live_root/crates/Cargo.toml" -p angel-live "$@"
