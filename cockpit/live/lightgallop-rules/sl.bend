import Base
import ./math.bend as M

# The ride's whole state is one persistent array of 32-bit slots: a binary tree
# of depth 11, 2048 slots, read and written by index. A slot holds a U32; a
# signed number is the same bits, a flag is 0 or 1. The host lays the session
# out into slots on the way in and reads it back on the way out (index.js), so
# a rule here is `Sl -> Sl`, and the Rust session never has a second owner.

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

# The slot at `i`, and the array with slot `i` set to `v`.
def g(+t: Sl, +i: U32) -> U32:
  get_go(11n, t, i)

def p(+t: Sl, +i: U32, +v: U32) -> Sl:
  set_go(11n, t, i, v)

# Add `d` to slot `i`.
def add(+t: Sl, +i: U32, +d: U32) -> Sl:
  p(t, i, (g(t, i) + d : U32))

# A choice between two arrays.
def pks(+c: Bool, +a: Sl, +b: Sl) -> Sl:
  Bool.pick(Sl, c, a, b)

# An array of zeros of the given depth.
def zeros(+n: Nat) -> Sl:
  match n:
    case 0n:
      Lf{0}
    case 1n+k:
      +z = zeros(k)
      Nd{z, z}
