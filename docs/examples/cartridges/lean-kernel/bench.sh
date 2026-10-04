#!/usr/bin/env bash
# One measured candidate: prove it, then replay it in the kernel. The local
# replay reports wall time, not an official score. Copy beside the challenge
# repository's README; angelX counts `bench*` scripts as measurements.
set -euo pipefail
problem=${1:?a problem id: fib, partition, mertens, primecount, permanent, ca-rule110, sha256 or polydisc}
(cd "problems/$problem" && lake build)
exec python3 evaluation/run.py --problem "$problem" --submission "problems/$problem/Submission.lean"
