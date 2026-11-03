# The core group's types: declared first, since a constructor must be declared before it is matched.

type HitstopResume is Data:
  HitWait{}
  HitSame{}
  HitFresh{map: J.Val}

# `Impact`: what asked for the pause.
type HitstopImpact is Data:
  HitHeavy{}
  HitPhase{}
  HitEliteKill{}
  HitBossKill{}
  HitFinisher{}

