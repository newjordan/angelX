import Base
import ./LAWS.bend as Laws
import ./worker-health.bend as Wh

def Laws.healthy_phases_pass_through():
  {==}

def Laws.fenced_stays_healthy():
  {==}

def Laws.suspect_is_nudged():
  {==}

def Laws.progress_resets_staleness(stale_next):
  match stale_next:
    case 0n:
      {==}
    case 1n+p:
      %Laws.progress_resets_staleness(p) : {Wh.semantic(True{}, 1n+p) == Wh.semantic(True{}, stale_next) : Wh.Semantic}
      {==}
