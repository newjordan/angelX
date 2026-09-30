# The parent campaign and every route chain each own their current Bash jobs.
# Wait asynchronously so a PID-only signal can interrupt the wait immediately.
CAMPAIGN_GRACE=12
run_trial() {
  local campaign_trial campaign_result
  "$@" &
  campaign_trial=$!
  if wait "$campaign_trial"; then return 0; else campaign_result=$?; fi
  return "$campaign_result"
}

cleanup_campaign() {
  local campaign_status=$? campaign_child
  local -a campaign_children=()
  trap '' INT TERM
  mapfile -t campaign_children < <(jobs -pr)
  if (( ${#campaign_children[@]} )); then
    # pidfd signals cannot hit an unrelated process after a numeric PID is
    # reused. The helper also checks that each target belongs to this Bash.
    python3 "$HERE/signal_children.py" --grace-secs "$CAMPAIGN_GRACE" "$BASHPID" "${campaign_children[@]}"
    for campaign_child in "${campaign_children[@]}"; do
      wait "$campaign_child" 2>/dev/null || true
    done
  fi
  return "$campaign_status"
}

campaign_traps() {
  trap cleanup_campaign EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
}
campaign_traps
