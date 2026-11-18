#!/bin/bash
set -e
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
T=${TMPDIR:-/tmp}/numerics-run
mkdir -p "$T/delve/tests" "$T/delve/rules" "$T/delve/stubs"
ln -sfn "$ROOT/lib" "$T/delve/lib"
ln -sfn "$ROOT/rules/numerics" "$T/delve/rules/numerics"
cp "$ROOT/build.py" "$T/delve/build.py"
cp "$ROOT/tests/cases.py" "$ROOT/tests/cases_main.bend" "$T/delve/tests/"
cd "$T" && exec python3 delve/tests/cases.py "$@"
