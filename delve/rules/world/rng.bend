def layout.rng.next(+r: W.W) -> W.W & W.W:
  loot.Rng.next(r)

def layout.rng.below(+r: W.W, +n: U32) -> W.W & U32:
  loot.Rng.below(r, n)

def layout.rng.chance(+r: W.W, +percent: U32) -> W.W & Bool:
  loot.Rng.chance(r, percent)

def layout.rng.pick(+r: W.W, +weights: List<&2, U32>) -> W.W & U32:
  loot.Rng.pick(r, weights)
