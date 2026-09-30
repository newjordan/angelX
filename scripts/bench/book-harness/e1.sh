#!/usr/bin/env bash
# e1.sh <route>... — E1: intro / bare / prebook on the two-bug task, n=5 per
# arm, arm order shuffled within each round; one background chain per route.
# Labels: e1-<route>-<arm>-<round>.
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
      if order=$(shuf -e intro bare prebook); then :; else
        rc=$?; (( chain_status != 0 )) || chain_status=$rc
        exit "$chain_status"
      fi
      read -r -a arms <<< "${order//$'\n'/ }"
      if [[ ${#arms[@]} != 3 || " ${arms[*]} " != *" intro "* || " ${arms[*]} " != *" bare "* || " ${arms[*]} " != *" prebook "* ]]; then
        echo 'Invalid shuffled arm order' >&2
        (( chain_status != 0 )) || chain_status=2
        exit "$chain_status"
      fi
      for arm in "${arms[@]}"; do
        if case $arm in
          intro)   ROUTE=$route ANGEL_BOOK_INTRO=1 run_trial ./task_run.sh h         "e1-$route-intro-$r"   twobug ;;
          bare)    ROUTE=$route ANGEL_BOOK_INTRO=0 run_trial ./task_run.sh h         "e1-$route-bare-$r"    twobug ;;
          prebook) ROUTE=$route                    run_trial ./task_run.sh b-prebook "e1-$route-prebook-$r" twobug ;;
        esac; then
          :
        else
          rc=$?
          (( chain_status != 0 )) || chain_status=$rc
        fi
      done
    done
    exit "$chain_status"
  ) > "out/e1-$route.log" 2>&1 &
  PIDS+=("$!")
done
status=0
for pid in "${PIDS[@]}"; do
  if wait "$pid"; then :; else rc=$?; (( status != 0 )) || status=$rc; fi
done
exit "$status"
