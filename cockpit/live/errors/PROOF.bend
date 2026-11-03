import Base
import ./LAWS.bend as Laws

def Laws.veto_never_retries(kind, status, config, cert, auth, quota, rejected, context, answered, capped, attempt, bounded, budget, backoff, cap, elapsed):
  {==}

def Laws.rejected_certificate_never_retries(kind, status, veto, config, auth, quota, rejected, context, answered, capped, attempt, bounded, budget, backoff, cap, elapsed):
  match veto config:
    case True{} _:
      {==}
    case False{} True{}:
      {==}
    case False{} False{}:
      {==}

def Laws.no_replay_no_wait(backoff, step, cap):
  {==}
