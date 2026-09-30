#!/usr/bin/env bash
# e1.sh <route>... — E1: intro / bare / prebook on the two-bug task, n=5 per
# arm, arm order shuffled within each round; one background chain per route.
# Labels: e1-<route>-<arm>-<round>.
set -u
cd "$(dirname "$0")"
for route in "$@"; do
  (
    for r in 1 2 3 4 5; do
      for arm in $(shuf -e intro bare prebook); do
        case $arm in
          intro)   ROUTE=$route ANGEL_BOOK_INTRO=1 ./task_run.sh h         e1-$route-intro-$r   twobug ;;
          bare)    ROUTE=$route ANGEL_BOOK_INTRO=0 ./task_run.sh h         e1-$route-bare-$r    twobug ;;
          prebook) ROUTE=$route                    ./task_run.sh b-prebook e1-$route-prebook-$r twobug ;;
        esac
      done
    done
  ) > out/e1-$route.log 2>&1 &
done
wait
