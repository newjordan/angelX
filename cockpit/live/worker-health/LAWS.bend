import Base
import ./worker-health.bend as Wh

law healthy_phases_pass_through:
  {Wh.recovery(0n) == Wh.Step{0n, 0n} : Wh.Recovery}

law fenced_stays_healthy:
  {Wh.recovery(5n) == Wh.Step{0n, 5n} : Wh.Recovery}

law suspect_is_nudged:
  {Wh.recovery(2n) == Wh.Step{1n, 2n} : Wh.Recovery}

law progress_resets_staleness:
  for stale_next: Nat
  {Wh.semantic(True{}, stale_next) == Wh.Choose{0n, 0n} : Wh.Semantic}
