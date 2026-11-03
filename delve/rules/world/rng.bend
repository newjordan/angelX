# The run's generator (loot.rs `Rng`, another group's) behind plain affine
# pairs. These four are the only place the world port names it, so a change
# in how its draws are paired is one edit here.

def layout.rng.next(+r: W.W) -> W.W & W.W:
  loot.Rng.next(r)

def layout.rng.below(+r: W.W, +n: U32) -> W.W & U32:
  loot.Rng.below(r, n)

def layout.rng.chance(+r: W.W, +percent: U32) -> W.W & Bool:
  loot.Rng.chance(r, percent)

def layout.rng.pick(+r: W.W, +weights: List<&2, U32>) -> W.W & U32:
  loot.Rng.pick(r, weights)
