import Base
import ./LAWS.bend as Laws

# Each law is a closed equality between literal verdicts, so its proof only
# re-runs the same disposition: no journal or adapter behaviour is claimed.

def Laws.a_mismatched_identity_conflicts_before_a_duplicate():
  {==}

def Laws.a_skipped_delta_detects_a_gap():
  {==}

def Laws.a_full_snapshot_is_never_a_gap(refresh):
  match refresh:
    case False{}:
      {==}
    case True{}:
      {==}

def Laws.a_duplicate_observation_is_ignored_first(refresh):
  match refresh:
    case False{}:
      {==}
    case True{}:
      {==}
