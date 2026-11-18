import Base
import ./math.bend as M

type Sl is Data:
  Lf{v: U32}
  Nd{lo: Sl, hi: Sl}

def bit(+i: U32, +d: Nat) -> Bool:
  U32.is_eq(U32.and(U32.shrn(i, d), 1), 1)

def get_go(+n: Nat, +t: Sl, +i: U32) -> U32:
  match n t:
    case 0n Lf{v}:
      v
    case 1n+k Nd{lo, hi}:
      get_go(k, Bool.pick(Sl, bit(i, k), hi, lo), i)
    case _ _:
      0

def set_go(+n: Nat, +t: Sl, +i: U32, +x: U32) -> Sl:
  match n t:
    case 0n Lf{v}:
      Lf{x}
    case 1n+k Nd{lo, hi}:
      +c = bit(i, k)
      +child = set_go(k, Bool.pick(Sl, c, hi, lo), i, x)
      Nd{Bool.pick(Sl, c, lo, child), Bool.pick(Sl, c, child, hi)}
    case _ _:
      t

def g(+t: Sl, +i: U32) -> U32:
  get_go(11n, t, i)

def p(+t: Sl, +i: U32, +v: U32) -> Sl:
  set_go(11n, t, i, v)

def add(+t: Sl, +i: U32, +d: U32) -> Sl:
  p(t, i, (g(t, i) + d : U32))

def pks(+c: Bool, +a: Sl, +b: Sl) -> Sl:
  Bool.pick(Sl, c, a, b)

def zeros(+n: Nat) -> Sl:
  match n:
    case 0n:
      Lf{0}
    case 1n+k:
      +z = zeros(k)
      Nd{z, z}
