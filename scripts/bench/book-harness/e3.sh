#!/usr/bin/env bash
# e3.sh <rounds> <route>... — E3: live /loop sessions on heesch. Each round
# runs the intro and bare arms of every route at the same time (same provider
# conditions), then waits. Labels: e3-<route>-<intro|bare>-<round>.
# BIN (default h) is the build directory; LABEL_TAG adds a tag to the labels.
set -u
if (( $# < 2 )) || [[ ! "$1" =~ ^[1-9][0-9]*$ ]]; then
  printf 'Usage: %s <positive rounds> <route>...\n' "$0" >&2
  exit 2
fi
declare -A SEEN_ROUTES=()
for route in "${@:2}"; do
  [[ "$route" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]] || { echo 'Invalid route label' >&2; exit 2; }
  [[ ! ${SEEN_ROUTES[$route]+present} ]] || { echo "Duplicate route label: $route" >&2; exit 2; }
  SEEN_ROUTES[$route]=1
done
tag=${LABEL_TAG:-}
[[ "$tag" =~ ^[a-zA-Z0-9._-]*$ ]] || { echo 'Invalid label tag' >&2; exit 2; }
HERE=$(cd "$(dirname "$0")" && pwd) || exit 2
. "$HERE/campaign_lifecycle.sh"
cd "$HERE" || exit 2
mkdir -p out || exit 2
ROUNDS=$1; shift
status=0
exec {ROUND_FD}< <(seq 1 "$ROUNDS")
ROUND_GENERATOR=$!
while read -r r; do
  PIDS=()
  for route in "$@"; do
    ROUTE=$route ANGEL_BOOK_INTRO=1 ./e3_session.sh "${BIN:-h}" "e3-$route$tag-intro-$r" > "out/e3-$route$tag-intro-$r.driver" 2>&1 &
    PIDS+=("$!")
    ROUTE=$route ANGEL_BOOK_INTRO=0 ./e3_session.sh "${BIN:-h}" "e3-$route$tag-bare-$r" > "out/e3-$route$tag-bare-$r.driver" 2>&1 &
    PIDS+=("$!")
  done
  round_status=0
  for pid in "${PIDS[@]}"; do
    if wait "$pid"; then :; else rc=$?; (( round_status != 0 )) || round_status=$rc; fi
  done
  (( status != 0 )) || status=$round_status
  echo "round $r done status=$round_status $(date -Is)"
done <&"$ROUND_FD"
exec {ROUND_FD}<&-
if wait "$ROUND_GENERATOR"; then :; else rc=$?; (( status != 0 )) || status=$rc; fi
exit "$status"
