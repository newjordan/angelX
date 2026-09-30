#!/usr/bin/env bash
# e5.sh <route>... — E5: E1's intro arm on the speed baseline (b-base/) and
# head (h/), n=5 per build, build order shuffled within each round.
# Labels: e5-<route>-<base|head>-<round>.
set -u
if (( $# == 0 )); then
  printf 'Usage: %s <route>...\n' "$0" >&2
  exit 2
fi
declare -A SEEN_ROUTES=()
for route in "$@"; do
  [[ "$route" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]] || { echo 'Invalid route label' >&2; exit 2; }
  [[ ! ${SEEN_ROUTES[$route]+present} ]] || { echo "Duplicate route label: $route" >&2; exit 2; }
  SEEN_ROUTES[$route]=1
done
HERE=$(cd "$(dirname "$0")" && pwd) || exit 2
. "$HERE/campaign_lifecycle.sh"
cd "$HERE" || exit 2
mkdir -p out || exit 2
PIDS=()
for route in "$@"; do
  (
    CAMPAIGN_GRACE=5
    campaign_traps
    chain_status=0
    for r in 1 2 3 4 5; do
      if order=$(shuf -e base head); then :; else
        rc=$?; (( chain_status != 0 )) || chain_status=$rc
        exit "$chain_status"
      fi
      read -r -a builds <<< "${order//$'\n'/ }"
      if [[ ${#builds[@]} != 2 || " ${builds[*]} " != *" base "* || " ${builds[*]} " != *" head "* ]]; then
        echo 'Invalid shuffled build order' >&2
        (( chain_status != 0 )) || chain_status=2
        exit "$chain_status"
      fi
      for build in "${builds[@]}"; do
        dir=h; [ "$build" = base ] && dir=b-base
        if ROUTE=$route ANGEL_BOOK_INTRO=1 run_trial ./task_run.sh "$dir" "e5-$route-$build-$r" twobug; then
          :
        else
          rc=$?
          (( chain_status != 0 )) || chain_status=$rc
        fi
      done
    done
    exit "$chain_status"
  ) > "out/e5-$route.log" 2>&1 &
  PIDS+=("$!")
done
status=0
for pid in "${PIDS[@]}"; do
  if wait "$pid"; then :; else rc=$?; (( status != 0 )) || status=$rc; fi
done
exit "$status"
