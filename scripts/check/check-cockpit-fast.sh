#!/usr/bin/env bash
# Edit/test feedback shares the launcher's incremental fast profile, not a
# separate debug tree. Opt into final release qualification explicitly.
set -euo pipefail
check_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
# Pin the actual Cargo/toolchain before tests replace HOME in their fixtures.
# A direct Cargo installation also works without a rustup shim.
gate_cargo=$(command -v cargo)
if command -v rustup > /dev/null 2>&1; then
  gate_cargo=$(cd "$check_root" && rustup which cargo)
  gate_toolchain=$(cd "$check_root" && rustup show active-toolchain)
  export RUSTUP_TOOLCHAIN="${gate_toolchain%% *}"
fi
# Match the launcher's identity inputs: switching between launch and test must
# not invalidate the host crate just because one omitted compiler provenance.
gate_compiler="$(dirname "$gate_cargo")/rustc"
if [[ -x "$gate_compiler" ]]; then
  export RUSTC="$gate_compiler"
fi
export RUSTUP_AUTO_INSTALL=0
export ANGEL_BUILD_RUSTC="$("${RUSTC:-rustc}" -vV)"
qualify_release=0
if [[ "${1:-}" == --release ]]; then
  qualify_release=1
  shift
fi
# Fix source/toolchain identity before compiling anything in a final gate.
# Discovering an unbound image afterward otherwise forces another release build.
if (( qualify_release )); then
  gate_source=$(bash "$check_root/scripts/check/cockpit-source-digest.sh")
  if [[ -n "${ANGEL_BUILD_SOURCE_SHA256:-}" && "$ANGEL_BUILD_SOURCE_SHA256" != "$gate_source" ]]; then
    printf 'Build identity differs from the current committed source.\n' >&2
    exit 3
  fi
  export ANGEL_BUILD_SOURCE_SHA256="$gate_source"
fi
export ANGEL_BUILD_PROFILE=devfast
check_source() {
  [[ "$(bash "$check_root/scripts/check/cockpit-source-digest.sh")" == "$gate_source" ]] || {
    printf 'Source changed during qualification; candidate is not qualified.\n' >&2
    return 3
  }
}
printf 'Checking ordinary cockpit boundary...\n'
bash "$check_root/scripts/check/check-locks.sh"
bash "$check_root/scripts/check/check-plug-boundary.sh"
bash "$check_root/scripts/check/check-legacy-terminal-boundary.sh"
python3 "$check_root/scripts/check/check-active-connections.py"
# A gate owns its temporary fixtures, including stores left by test binaries.
# Preserve the caller's parent while removing only this invocation's directory.
gate_tmp=$(mktemp -d "${TMPDIR:-/tmp}/angelx-tests.XXXXXX")
trap 'rm -rf -- "$gate_tmp"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
export TMPDIR="$gate_tmp"
prepare_helper=0
if [[ -z "${ANGEL_T_SANDBOX_HELPER:-}" ]]; then
  prepare_helper=1
fi
# The development phase builds the `devfast` profile (cockpit/Cargo.toml):
# debug assertions and overflow checks stay on, but the code is optimized —
# at dev's opt-0 the CPU-bound tests alone take ~15x longer.
printf 'Development tests (devfast, no release LTO)...\n'
# Compile first, then keep the test image and CLI/helper siblings private. A
# concurrent Cargo build may unlink the shared target executable while tests
# use current_exe() to re-exec; the gate's own image stays available until exit.
build_log="$gate_tmp/build.jsonl"
test_log="$gate_tmp/tests.jsonl"
image="$gate_tmp/image"
if "$gate_cargo" build --locked --profile devfast --manifest-path "$check_root/cockpit/Cargo.toml" \
    --no-default-features --features delve --bins --message-format=json > "$build_log"; then
  :
else
  build_status=$?
  python3 "$check_root/scripts/check/copy-cockpit-test-image.py" --diagnostics "$build_log" || true
  exit "$build_status"
fi
if "$gate_cargo" test --locked --profile devfast --manifest-path "$check_root/cockpit/Cargo.toml" \
    --no-default-features --features delve --bin angel --no-run --message-format=json > "$test_log"; then
  :
else
  build_status=$?
  python3 "$check_root/scripts/check/copy-cockpit-test-image.py" --diagnostics "$test_log" || true
  exit "$build_status"
fi
"$gate_cargo" check --locked --profile devfast --manifest-path "$check_root/cockpit/Cargo.toml" \
  --no-default-features --bin angel
image_args=(--build "$build_log" --tests "$test_log" --out "$image" --profile devfast)
if (( ! prepare_helper )); then
  image_args+=(--helper "$ANGEL_T_SANDBOX_HELPER")
fi
python3 "$check_root/scripts/check/copy-cockpit-test-image.py" "${image_args[@]}"
# A caller's interactive authority is not a test fixture. Individual tests
# opt into their own authority explicitly. Cargo normally sets the package cwd
# when launching its test executable; preserve that contract for the copy.
(
  cd "$check_root/cockpit"
  ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 CARGO="$gate_cargo" \
    CARGO_BIN_EXE_angel="$image/devfast/angel" \
    ANGEL_T_SANDBOX_HELPER="$image/devfast/angel-sandbox" \
    "$image/devfast/deps/angel-tests" "$@"
)
# Dependency tests do not run with `--bin angel`: retain moved V8 coverage.
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 \
  "$gate_cargo" test --locked --profile devfast --manifest-path "$check_root/cockpit/Cargo.toml" \
    --package angel-code-mode --lib -- "$@"
ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 \
  "$gate_cargo" test --locked --profile devfast --manifest-path "$check_root/cockpit/Cargo.toml" \
    --package angel-delve-cart --lib -- "$@"
if (( qualify_release )); then
  check_source
  export ANGEL_BUILD_PROFILE=release
  export ANGEL_DELVE_WEB=1
  if (( prepare_helper )); then
    helper_target=$(cargo metadata --no-deps --format-version 1 \
      --manifest-path "$check_root/cockpit/Cargo.toml" | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
    "$gate_cargo" build --locked --manifest-path "$check_root/cockpit/Cargo.toml" \
      --release --no-default-features --bin angel-sandbox
    export ANGEL_T_SANDBOX_HELPER="$helper_target/release/angel-sandbox"
  fi
  printf 'Development checks passed; qualifying optimized tests...\n'
  ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 \
  cargo test --locked --manifest-path "$check_root/cockpit/Cargo.toml" \
    --release --no-default-features --features delve --bin angel -- "$@"
  ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 \
    "$gate_cargo" test --locked --manifest-path "$check_root/cockpit/Cargo.toml" \
      --release --package angel-code-mode --lib -- "$@"
  ANGEL_YOLO=0 ANGEL_YOLO_SMART=0 \
    "$gate_cargo" test --locked --manifest-path "$check_root/cockpit/Cargo.toml" \
      --release --package angel-delve-cart --lib -- "$@"
  check_source
  printf 'Optimized tests passed; building the candidate...\n'
  "$gate_cargo" build --locked --manifest-path "$check_root/cockpit/Cargo.toml" \
    --release --no-default-features --features delve --bin angel
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
