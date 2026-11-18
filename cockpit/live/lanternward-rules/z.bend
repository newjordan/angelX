import Base
import ./r.bend as R

def lz(c: Bool, a: Unit -> R.W, b: Unit -> R.W) -> R.W:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def lz64(c: Bool, a: Unit -> R.T8, b: Unit -> R.T8) -> R.T8:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

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
