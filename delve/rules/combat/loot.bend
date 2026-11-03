# crates/delve/src/rules/loot.rs — the run's generator and the drop tables.
#
# `Rng` is a newtype over u64: in the state it is the bare number, here the
# `W.W` itself. Every draw hands back the new generator beside the value:
#
#   loot.Rng.new(+seed: W.W) -> W.W
#   loot.Rng.next(+r: W.W) -> W.W & W.W                  (generator, output)
#   loot.Rng.below(+r: W.W, +n: U32) -> W.W & U32   (generator, 0 <= v < max(n, 1))
#   loot.Rng.chance(+r: W.W, +percent: U32) -> W.W & Bool
#   loot.Rng.pick(+r: W.W, +weights: List<&2, U32>) -> W.W & U32
#
# `pick` is Rust's generic `pick<T: Copy>(&[(T, u32)]) -> T`: it takes the
# weights alone and answers the index of the chosen entry (the caller indexes
# its own table; zero weights are never chosen, an empty table answers 0).

def loot.Rng.new(+seed: W.W) -> W.W:
  W.or(W.xor(seed, W.W{2654435769, 2135587861}), W.W{0, 1})

def loot.Rng.next(+r: W.W) -> W.W & W.W:
  +x1 = W.xor(r, W.shr(r, 12))
  +x2 = W.xor(x1, W.shl(x1, 25))
  +x3 = W.xor(x2, W.shr(x2, 27))
  (x3, W.mul(x3, W.W{625341585, 1332534557}))

def loot.Rng.below.go(rv: W.W & W.W, +n: U32) -> W.W & U32:
  (+r2, +o) = rv
  (r2, W.rem_u32(o, loot.max1(n)))

def loot.max1(+n: U32) -> U32:
  loot.max1.if(U32.is_eq(n, 0), n)

def loot.max1.if(zero: Bool, +n: U32) -> U32:
  match zero:
    case True{}:
      1
    case False{}:
      n

def loot.Rng.below(+r: W.W, +n: U32) -> W.W & U32:
  loot.Rng.below.go(loot.Rng.next(r), n)

def loot.Rng.chance.go(rv: W.W & U32, +percent: U32) -> W.W & Bool:
  (+r2, +v) = rv
  (r2, U32.is_lt(v, percent))

def loot.Rng.chance(+r: W.W, +percent: U32) -> W.W & Bool:
  loot.Rng.chance.go(loot.Rng.below(r, 100), percent)

def loot.sum(+ws: List<&2, U32>) -> U32:
  match ws:
    case Nil{}:
      0
    case Con{+h, +t}:
      (h + loot.sum(t) : U32)

# The index the roll lands on, walking the weights as Rust does: a roll under
# the weight is the entry, otherwise the weight comes off the roll.
def loot.Rng.pick.walk(+ws: List<&2, U32>, +roll: U32, +i: U32) -> U32:
  match ws:
    case Nil{}:
      0
    case Con{+w, +t}:
      loot.Rng.pick.walk.if(U32.is_lt(roll, w), t, roll, w, i)

def loot.Rng.pick.walk.if(hit: Bool, +t: List<&2, U32>, +roll: U32, +w: U32, +i: U32) -> U32:
  match hit:
    case True{}:
      i
    case False{}:
      loot.Rng.pick.walk(t, (roll - w : U32), (i + 1 : U32))

def loot.Rng.pick.go(rv: W.W & U32, +ws: List<&2, U32>) -> W.W & U32:
  (+r2, +roll) = rv
  (r2, loot.Rng.pick.walk(ws, roll, 0))

def loot.Rng.pick(+r: W.W, +ws: List<&2, U32>) -> W.W & U32:
  loot.Rng.pick.go(loot.Rng.below(r, loot.sum(ws)), ws)

# Cases (recorded from the Rust original: tests/rules/loot_bend_cases__tests.rs)
def case.loot.Rng.new(+input: J.Val) -> J.Val:
  W.to_j(loot.Rng.new(W.of_j(J.get(input, "seed"))))

def cb.case.loot.Rng.next.go(rv: W.W & W.W) -> J.Val:
  (+r, +o) = rv
  J.set(J.set(J.obj(), "rng", W.to_j(r)), "out", W.to_j(o))

def case.loot.Rng.next(+input: J.Val) -> J.Val:
  cb.case.loot.Rng.next.go(loot.Rng.next(W.of_j(J.get(input, "rng"))))

def cb.case.loot.Rng.below.go(rv: W.W & U32) -> J.Val:
  (+r, +v) = rv
  J.set(J.set(J.obj(), "rng", W.to_j(r)), "v", J.of_u32(v))

def case.loot.Rng.below(+input: J.Val) -> J.Val:
  cb.case.loot.Rng.below.go(loot.Rng.below(W.of_j(J.get(input, "rng")), J.u32(J.get(input, "n"))))

def cb.case.loot.Rng.chance.go(rv: W.W & Bool) -> J.Val:
  (+r, +b) = rv
  J.set(J.set(J.obj(), "rng", W.to_j(r)), "hit", J.of_bool(b))

def case.loot.Rng.chance(+input: J.Val) -> J.Val:
  cb.case.loot.Rng.chance.go(loot.Rng.chance(W.of_j(J.get(input, "rng")), J.u32(J.get(input, "percent"))))

def cb.case.loot.Rng.pick.go(rv: W.W & U32) -> J.Val:
  (+r, +i) = rv
  J.set(J.set(J.obj(), "rng", W.to_j(r)), "index", J.of_u32(i))

def cb.case.loot.Rng.pick.weights(+xs: List<&2, J.Val>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.u32(h) <> cb.case.loot.Rng.pick.weights(t)

def case.loot.Rng.pick(+input: J.Val) -> J.Val:
  cb.case.loot.Rng.pick.go(loot.Rng.pick(W.of_j(J.get(input, "rng")), cb.case.loot.Rng.pick.weights(J.items(J.get(input, "weights")))))
