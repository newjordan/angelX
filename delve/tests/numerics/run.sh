#!/bin/bash
# Run the numerics cases in a private tree: this checkout's libs and numerics
# fragments, no other group's fragments or stubs, so a half-written fragment
# elsewhere cannot break the proof.
#
#   delve/tests/numerics/run.sh numerics
#   delve/tests/numerics/run.sh --cases <dir> RS.f_sin RS.f_cos
set -e
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
T=${TMPDIR:-/tmp}/numerics-run
mkdir -p "$T/delve/tests" "$T/delve/rules" "$T/delve/stubs"
ln -sfn "$ROOT/lib" "$T/delve/lib"
ln -sfn "$ROOT/rules/numerics" "$T/delve/rules/numerics"
cp "$ROOT/build.py" "$T/delve/build.py"
cp "$ROOT/tests/cases.py" "$ROOT/tests/cases_main.bend" "$T/delve/tests/"
cd "$T" && exec python3 delve/tests/cases.py "$@"
