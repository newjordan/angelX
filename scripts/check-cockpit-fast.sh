#!/usr/bin/env bash
# Edit/test feedback without release LTO. Opt into final qualification explicitly.
set -euo pipefail
check_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
qualify_release=0
if [[ "${1:-}" == --release ]]; then
  qualify_release=1
  shift
fi
# Fix source/toolchain identity before compiling anything in a final gate.
# Discovering an unbound image afterward otherwise forces another release build.
if (( qualify_release )); then
  gate_source=$(bash "$check_root/scripts/cockpit-source-digest.sh")
  if [[ -n "${ANGEL_BUILD_SOURCE_SHA256:-}" && "$ANGEL_BUILD_SOURCE_SHA256" != "$gate_source" ]]; then
    printf 'Build identity differs from the current committed source.\n' >&2
    exit 3
  fi
  export ANGEL_BUILD_SOURCE_SHA256="$gate_source"
  export ANGEL_BUILD_RUSTC="$(rustc -vV)"
  export ANGEL_BUILD_PROFILE=debug
fi
check_source() {
  [[ "$(bash "$check_root/scripts/cockpit-source-digest.sh")" == "$gate_source" ]] || {
    printf 'Source changed during qualification; candidate is not qualified.\n' >&2
    return 3
  }
}
printf 'Checking ordinary cockpit boundary...\n'
bash "$check_root/scripts/check-legacy-terminal-boundary.sh"
python3 "$check_root/scripts/check-active-connections.py"
prepare_helper=0
if [[ -z "${ANGEL_T_SANDBOX_HELPER:-}" ]]; then
  # Prepare the tiny helper before timed tests. Lazy helper compilation inside
  # the first process test can exhaust its observation deadline or wait on Cargo.
  prepare_helper=1
  helper_target=$(cargo metadata --no-deps --format-version 1 \
    --manifest-path "$check_root/cockpit/Cargo.toml" | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
  cargo build --locked --manifest-path "$check_root/cockpit/Cargo.toml" \
    --no-default-features --bin angel-sandbox
  export ANGEL_T_SANDBOX_HELPER="$helper_target/debug/angel-sandbox"
fi
printf 'Development tests (no release LTO)...\n'
# A caller's interactive authority is not a test fixture. Inherited YOLO
# bypasses timeout/effect checks and turns bounded process tests into long
# waits. Individual tests can still opt into either profile with EnvGuard.
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 \
cargo test --locked --manifest-path "$check_root/cockpit/Cargo.toml" \
  --no-default-features --bin angel -- "$@"
if (( qualify_release )); then
  check_source
  export ANGEL_BUILD_PROFILE=release
  if (( prepare_helper )); then
    cargo build --locked --manifest-path "$check_root/cockpit/Cargo.toml" \
      --release --no-default-features --bin angel-sandbox
    export ANGEL_T_SANDBOX_HELPER="$helper_target/release/angel-sandbox"
  fi
  printf 'Development checks passed; qualifying optimized tests...\n'
  ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 \
  cargo test --locked --manifest-path "$check_root/cockpit/Cargo.toml" \
    --release --no-default-features --bin angel -- "$@"
  check_source
  printf 'Optimized tests passed; building the candidate...\n'
  cargo build --locked --manifest-path "$check_root/cockpit/Cargo.toml" \
    --release --no-default-features --bin angel
  check_source
  candidate_target=${helper_target:-$(cargo metadata --no-deps --format-version 1 \
    --manifest-path "$check_root/cockpit/Cargo.toml" | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')}
  "$candidate_target/release/angel" --build-info --json | python3 -c '
import json,os,sys
info=json.load(sys.stdin)
if info.get("cockpit_source_sha256") != os.environ["ANGEL_BUILD_SOURCE_SHA256"] or info.get("toolchain",{}).get("profile") != "release":
    sys.exit("Built executable identity does not match qualification")
print(json.dumps(info))'
fi
