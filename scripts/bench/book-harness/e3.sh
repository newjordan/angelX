#!/usr/bin/env bash
# e3.sh <rounds> <route>... — E3: live /loop sessions on heesch. Each round
# runs the intro and bare arms of every route at the same time (same provider
# conditions), then waits. Labels: e3-<route>-<intro|bare>-<round>.
# BIN (default h) is the build directory; LABEL_TAG adds a tag to the labels.
set -u
cd "$(dirname "$0")"
ROUNDS=$1; shift
for r in $(seq 1 "$ROUNDS"); do
  for route in "$@"; do
    tag=${LABEL_TAG:-}
    ROUTE=$route ANGEL_BOOK_INTRO=1 ./e3_session.sh "${BIN:-h}" e3-$route$tag-intro-$r > out/e3-$route$tag-intro-$r.driver 2>&1 &
    ROUTE=$route ANGEL_BOOK_INTRO=0 ./e3_session.sh "${BIN:-h}" e3-$route$tag-bare-$r  > out/e3-$route$tag-bare-$r.driver 2>&1 &
  done
  wait
  echo "round $r done $(date -Is)"
done
