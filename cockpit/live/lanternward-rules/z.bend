# A choice whose branches are not both done: Bend is strict, so `Bool.pick` works
# out both of its answers, and a branch that is a whole rule (a sweep, a nudge, a
# room being built) must wait for the choice. Each branch here is a function of
# nothing, called only if chosen.
import Base
import ./r.bend as R

def lz(c: Bool, a: Unit -> R.W, b: Unit -> R.W) -> R.W:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

# The same for a tree.
def lz64(c: Bool, a: Unit -> R.T8, b: Unit -> R.T8) -> R.T8:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

# A choice where only the first answer is worth waiting for: the other is already
# in hand.
def lw(c: Bool, a: Unit -> R.W, b: R.W) -> R.W:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b

def lw64(c: Bool, a: Unit -> R.T8, b: R.T8) -> R.T8:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b
