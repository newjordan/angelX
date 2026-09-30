#!/usr/bin/env bash
# e5.sh <route>... — E5: E1's intro arm on the speed baseline (b-base/) and
# head (h/), n=5 per build, build order shuffled within each round.
# Labels: e5-<route>-<base|head>-<round>.
set -u
cd "$(dirname "$0")"
for route in "$@"; do
  (
    for r in 1 2 3 4 5; do
      for build in $(shuf -e base head); do
        dir=h; [ "$build" = base ] && dir=b-base
        ROUTE=$route ANGEL_BOOK_INTRO=1 ./task_run.sh $dir e5-$route-$build-$r twobug
      done
    done
  ) > out/e5-$route.log 2>&1 &
done
wait
