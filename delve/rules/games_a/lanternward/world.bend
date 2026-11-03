# crates/delve/src/rules/lanternward/world.rs — Lanternward's fixed ground:
# whole-number math, the five rooms' tiles, what a body may stand on.
#
# An i32 is a U32 holding its two's complement pattern; an i64 that the Rust
# keeps only in a product is computed on the unsigned magnitude (U32 when it
# fits, W otherwise) with the sign applied last, so the result is the same
# truncating division Rust's `i64 /` does. A position pair `(i32, i32)` is
# `U32 & U32`. Q is 256, TILE 16, TILE_SHIFT 12.

def lanternward.world.Q() -> U32:
  256

def lanternward.world.TILE() -> U32:
  16

def lanternward.world.TILE_SHIFT() -> U32:
  12

def lanternward.world.ROOMS() -> U32:
  5

# The integer square root of a u64, rounded down (Rust's `isqrt` on a
# non-negative i64): the root's bits from the top, each kept while its square
# stays under `n`.
def lanternward.world.isqrt.go(+k: Nat, +n: W.W, +res: U32) -> U32:
  match k:
    case 0n:
      res
    case 1n+p:
      +cand = U32.or(res, U32.shln(1, p))
      lanternward.world.isqrt.go(p, n, Bool.pick(U32, W.le(W.mul32(cand, cand), n), cand, res))

def lanternward.world.isqrt(+n: W.W) -> W.W:
  W.of_u32(lanternward.world.isqrt.go(32n, n, 0))

# Squares under 2^32 take the cheap root.
def lanternward.world.isqrt.fit(small: Bool, +n: W.W) -> U32:
  match small:
    case True{}:
      RS.u_isqrt(W.lo32(n))
    case False{}:
      lanternward.world.isqrt.go(32n, n, 0)

# floor(sqrt(n)) of a u64 as a U32 (the root of anything below 2^64 fits).
def lanternward.world.isqrt32(+n: W.W) -> U32:
  lanternward.world.isqrt.fit(U32.is_zero(W.hi32(n)), n)

# How long `(dx, dy)` is, in its own units.
def lanternward.world.dist(+dx: U32, +dy: U32) -> U32:
  +ax = RS.i_abs(dx)
  +ay = RS.i_abs(dy)
  lanternward.world.isqrt32(W.add(W.mul32(ax, ax), W.mul32(ay, ay)))

# `a * b / c` in i64, truncated to i32.
def lanternward.world.mul_div.fit(small: Bool, +ma: U32, +mb: U32, +mc: U32) -> U32:
  match small:
    case True{}:
      U32.div((ma * mb : U32), mc)
    case False{}:
      W.lo32(W.div(W.mul32(ma, mb), W.of_u32(mc)))

def lanternward.world.mul_div(+a: U32, +b: U32, +c: U32) -> U32:
  +ma = RS.i_abs(a)
  +mb = RS.i_abs(b)
  +q = lanternward.world.mul_div.fit(Bool.and(U32.is_lt(ma, 65536), U32.is_lt(mb, 65536)), ma, mb, RS.i_abs(c))
  Bool.pick(U32, Bool.xor(Bool.xor(RS.i_is_neg(a), RS.i_is_neg(b)), RS.i_is_neg(c)), RS.i_neg(q), q)

# `(dx, dy)` as a vector of length Q, scaled up first when short.
def lanternward.world.unit.mag(+m: U32, +l: U32, +neg: Bool) -> U32:
  +q = lanternward.world.unit.mag.fit(U32.is_lt(m, 16777216), m, l)
  Bool.pick(U32, neg, RS.i_neg(q), q)

def lanternward.world.unit.mag.fit(small: Bool, +m: U32, +l: U32) -> U32:
  match small:
    case True{}:
      U32.div((m * 256 : U32), l)
    case False{}:
      W.lo32(W.div_u32(W.mul32(m, 256), l))

def lanternward.world.unit.go(+ax: U32, +ay: U32, +nx: Bool, +ny: Bool) -> U32 & U32:
  +l = RS.u_max(lanternward.world.isqrt32(W.add(W.mul32(ax, ax), W.mul32(ay, ay))), 1)
  (lanternward.world.unit.mag(ax, l, nx), lanternward.world.unit.mag(ay, l, ny))

def lanternward.world.unit.shift(+dx: U32, +dy: U32, +sh: U32) -> U32 & U32:
  lanternward.world.unit.go(
    U32.shln(RS.i_abs(dx), U32.to_nat(sh)), U32.shln(RS.i_abs(dy), U32.to_nat(sh)),
    RS.i_is_neg(dx), RS.i_is_neg(dy))

def lanternward.world.unit.big(+big: U32, +dx: U32, +dy: U32) -> U32 & U32:
  match big:
    case 0:
      (0, 0)
    case _:
      lanternward.world.unit.shift(dx, dy, RS.u_sat_sub(24, W.bitlen32(big)))

def lanternward.world.unit(+dx: U32, +dy: U32) -> U32 & U32:
  lanternward.world.unit.big(RS.u_max(RS.i_abs(dx), RS.i_abs(dy)), dx, dy)

# The dot product of two Q vectors, in Q: (a.x*b.x + a.y*b.y) / Q in i64.
def lanternward.world.dot4.fit(small: Bool, +ax: U32, +ay: U32, +bx: U32, +by: U32) -> U32:
  match small:
    case True{}:
      RS.i_div((ax * bx + ay * by : U32), 256)
    case False{}:
      lanternward.world.dot4.big(ax, ay, bx, by)

def lanternward.world.dot4.big(+ax: U32, +ay: U32, +bx: U32, +by: U32) -> U32:
  lanternward.world.dot4.sum(
    W.mul32(RS.i_abs(ax), RS.i_abs(bx)), Bool.xor(RS.i_is_neg(ax), RS.i_is_neg(bx)),
    W.mul32(RS.i_abs(ay), RS.i_abs(by)), Bool.xor(RS.i_is_neg(ay), RS.i_is_neg(by)))

def lanternward.world.dot4.sum(+m1: W.W, +n1: Bool, +m2: W.W, +n2: Bool) -> U32:
  +same = Bool.not(Bool.xor(n1, n2))
  +first = W.ge(m1, m2)
  +mag = Bool.pick(W.W, same, W.add(m1, m2), Bool.pick(W.W, first, W.sub(m1, m2), W.sub(m2, m1)))
  +neg = Bool.pick(Bool, same, n1, Bool.pick(Bool, first, n1, n2))
  +q = W.lo32(W.div_u32(mag, 256))
  Bool.pick(U32, neg, RS.i_neg(q), q)

def lanternward.world.dot4(+ax: U32, +ay: U32, +bx: U32, +by: U32) -> U32:
  lanternward.world.dot4.fit(
    Bool.and(
      Bool.and(U32.is_le(RS.i_abs(ax), 23170), U32.is_le(RS.i_abs(ay), 23170)),
      Bool.and(U32.is_le(RS.i_abs(bx), 23170), U32.is_le(RS.i_abs(by), 23170))),
    ax, ay, bx, by)

# dot((ax, ay), unit(dx, dy)).
def lanternward.world.dot_unit.go(+ax: U32, +ay: U32, u: U32 & U32) -> U32:
  (ux, uy) = u
  lanternward.world.dot4(ax, ay, ux, uy)

def lanternward.world.dot_unit(+ax: U32, +ay: U32, +dx: U32, +dy: U32) -> U32:
  lanternward.world.dot_unit.go(ax, ay, lanternward.world.unit(dx, dy))

def lanternward.world.dot(a: U32 & U32, b: U32 & U32) -> U32:
  (ax, ay) = a
  (bx, by) = b
  lanternward.world.dot4(ax, ay, bx, by)

# Sine of a whole-degree angle in Q (Bhaskara's rational).
def lanternward.world.sin_q.go(+d: U32, +neg: Bool) -> U32:
  +p = (d * (180 - d : U32) : U32)
  +num = (1024 * p + RS.i_div((40500 - p : U32), 2) : U32)
  +q = RS.i_div(num, (40500 - p : U32))
  Bool.pick(U32, neg, RS.i_neg(q), q)

def lanternward.world.sin_q(+deg: U32) -> U32:
  +d = RS.i_rem_euclid(deg, 360)
  lanternward.world.sin_q.go(
    Bool.pick(U32, U32.is_ge(d, 180), (d - 180 : U32), d), U32.is_ge(d, 180))

def lanternward.world.cos_q(+deg: U32) -> U32:
  lanternward.world.sin_q((deg + 90 : U32))

# The Q direction of a whole-degree angle.
def lanternward.world.dir(+deg: U32) -> U32 & U32:
  (lanternward.world.cos_q(deg), lanternward.world.sin_q(deg))

# The whole degree whose direction lies closest to `u`: of those that point
# its way, the one with the least sideways error.
def lanternward.world.angle_of.go(+k: Nat, +d: U32, +ux: U32, +uy: U32, +bs: U32, +bd: U32) -> U32:
  match k:
    case 0n:
      bd
    case 1n+p:
      +cx = lanternward.world.cos_q(d)
      +cy = lanternward.world.sin_q(d)
      +side = RS.i_abs((cx * uy - cy * ux : U32))
      +take = Bool.and(RS.i_gt(lanternward.world.dot4(cx, cy, ux, uy), 0), RS.i_lt(side, bs))
      lanternward.world.angle_of.go(p, (d + 1 : U32), ux, uy, Bool.pick(U32, take, side, bs), Bool.pick(U32, take, d, bd))

def lanternward.world.angle_of.u(u: U32 & U32) -> U32:
  (+ux, +uy) = u
  Bool.pick(U32, Bool.and(U32.is_zero(ux), U32.is_zero(uy)), 0,
    lanternward.world.angle_of.go(360n, 0, ux, uy, 2147483647, 0))

# The whole-degree angle of `(dx, dy)`, 0 for no length.
def lanternward.world.angle_of(+dx: U32, +dy: U32) -> U32:
  lanternward.world.angle_of.u(lanternward.world.unit(dx, dy))

# The turn from angle `a` to angle `b`, -180 to 180.
def lanternward.world.turn(+a: U32, +b: U32) -> U32:
  (RS.i_rem_euclid((b - a + 180 : U32), 360) - 180 : U32)

# A tile's column or row of a position in Q pixels.
def lanternward.world.tile_of(+v: U32) -> U32:
  RS.i_shr(v, 12)

# The middle of tile `t`, in Q pixels.
def lanternward.world.middle(+t: U32) -> U32:
  (t * 4096 + 2048 : U32)

# The rooms. `plan(room)` clamps to the last room; a "Plan" is its room index.
def lanternward.world.plan(+room: U32) -> U32:
  RS.u_min(room, 4)

# STAIR
def lanternward.world.rows.stair() -> List<&2, String>:
  ["######################",
   "#..............==..\"\"#",
   "#.@.@.@.@.@.@..==..\"\"#",
   "#.C..F.........==....#",
   "#..x....\"\"\"\"...==.o..#",
   "#.......\"\"\"\"...==....#",
   "#..s...........==....#",
   "#.....S........==..c.#",
   "#..\"\"\".........==....#",
   "#..\"\"\".....o...==....#",
   "#..............==....#",
   "#.....o.....S..==..o.#",
   "#..o.........o.==....#",
   "#..............==....#",
   "#..............==....#",
   "#########GG###########",
   "#########QQ###########",
   "######################"]

# LAMP
def lanternward.world.rows.lamp() -> List<&2, String>:
  ["##############################",
   "#..........#.c.#......########",
   "#.oo.......#111#......#VbV####",
   "#.......S.............##V#####",
   "#....B.....................###",
   "#.......o..................###",
   "#....................B.....###",
   "#..........................###",
   "#.E.....\"\"..............S..GQ#",
   "#.......\"\"........\"\"\"......GQ#",
   "#.................\"\"\"......###",
   "#...s.S....................###",
   "#.............B....S.......###",
   "#.C..............o.......oN2c#",
   "#.@.@.@.@.@.@..............###",
   "##############################"]

# GALLERY
def lanternward.world.rows.gallery() -> List<&2, String>:
  ["######################################",
   "#..o.............#.................###",
   "#......s......o..#.................GQ#",
   "#.@.@.........Y..#...#.............GQ#",
   "#................#...#.............###",
   "#................#................VK##",
   "#.@.@....X....P..1.................###",
   "#................1.................###",
   "#................1...............T.###",
   "#.@.@..\"\"\"\"......#.....#.....#.....###",
   "#......\"\"\"\"......#.....#.....#.....###",
   "#.C....\"\"\"\".o..o.#..o.....#......T.###",
   "#................#.................###",
   "######################~###############",
   "#####################.c.##############",
   "######################################"]

# VAULT
def lanternward.world.rows.vault() -> List<&2, String>:
  ["################################",
   "#..........##.c..##..........###",
   "#..........##....##..........###",
   "#..........###LL###..........###",
   "#..o.....S..........S........###",
   "#............................###",
   "#............................###",
   "#............................###",
   "#..s.B..................B....MQ#",
   "#.............c..............MQ#",
   "#./..........................###",
   "#......S.................S...###",
   "#............................2c#",
   "#.............S.......S......###",
   "#.C.......................o..###",
   "#.@.@.@.@.@.@....o...........###",
   "#............................###",
   "################################"]

# WARDEN
def lanternward.world.rows.warden() -> List<&2, String>:
  ["##############################",
   "#............................#",
   "#.o............*...........o.#",
   "#............................#",
   "#............................#",
   "#...B..........W.........B...#",
   "#............................#",
   "#............................#",
   "#............................#",
   "#.......f............f.......#",
   "#............................#",
   "#..........#......#..........#",
   "#............................#",
   "#............................#",
   "#.......f............f.......#",
   "#............................#",
   "#...B....................B...#",
   "#..........#......#..........#",
   "#............................#",
   "#............................#",
   "#............................#",
   "#.o........@.@.@.@.@.@.....o.#",
   "#..............C.............#",
   "##############################"]

def lanternward.world.rows(+room: U32) -> List<&2, String>:
  match room:
    case 0:
      lanternward.world.rows.stair()
    case 1:
      lanternward.world.rows.lamp()
    case 2:
      lanternward.world.rows.gallery()
    case 3:
      lanternward.world.rows.vault()
    case _:
      lanternward.world.rows.warden()

# Whether the Gloam holds the room dark (the Key Vault).
def lanternward.world.Plan.dark(+room: U32) -> Bool:
  U32.is_eq(lanternward.world.plan(room), 3)

def lanternward.world.Plan.h.go(+rows: List<&2, String>) -> U32:
  U32.from_nat(List.length(&2, String, rows))

def lanternward.world.Plan.h(+room: U32) -> U32:
  lanternward.world.Plan.h.go(lanternward.world.rows(lanternward.world.plan(room)))

def lanternward.world.Plan.w.go(+rows: List<&2, String>) -> U32:
  match rows:
    case Con{+h, +t}:
      U32.from_nat(String.length(h))
    case Nil{}:
      0

def lanternward.world.Plan.w(+room: U32) -> U32:
  lanternward.world.Plan.w.go(lanternward.world.rows(lanternward.world.plan(room)))

def lanternward.world.row.go(+rows: List<&2, String>, k: Nat) -> String:
  match rows k:
    case Nil{} _:
      ""
    case Con{+h, +t} 0n:
      h
    case Con{+h, +t} 1n+p:
      lanternward.world.row.go(t, p)

def lanternward.world.char.go(s: String, k: Nat) -> U32:
  match s k:
    case SNil{} _:
      35
    case SCon{c, t} 0n:
      Char.to_u32(c)
    case SCon{c, t} 1n+p:
      lanternward.world.char.go(t, p)

def lanternward.world.Plan.at.in(+room: U32, +tx: U32, +ty: U32) -> U32:
  lanternward.world.char.go(
    lanternward.world.row.go(lanternward.world.rows(room), U32.to_nat(ty)), U32.to_nat(tx))

# The letter at a tile; wall beyond the edge.
def lanternward.world.Plan.at(+room: U32, +tx: U32, +ty: U32) -> U32:
  +r = lanternward.world.plan(room)
  Bool.pick(U32,
    Bool.or(Bool.or(RS.i_is_neg(tx), RS.i_is_neg(ty)), RS.i_ge(ty, lanternward.world.Plan.h(r))),
    35,
    lanternward.world.Plan.at.in(r, tx, ty))

# Every tile with this letter, row by row, each packed as x + 65536 * y
# (lanternward.world.packed_x / packed_y).
def lanternward.world.Plan.find.row(s: String, +c: U32, +x: U32, +y: U32) -> List<&2, U32>:
  match s:
    case SNil{}:
      Nil{}
    case SCon{h, t}:
      lanternward.world.Plan.find.hit(U32.is_eq(Char.to_u32(h), c), t, c, x, y)

def lanternward.world.Plan.find.hit(hit: Bool, s: String, +c: U32, +x: U32, +y: U32) -> List<&2, U32>:
  match hit:
    case True{}:
      Con{(x + y * 65536 : U32), lanternward.world.Plan.find.row(s, c, (x + 1 : U32), y)}
    case False{}:
      lanternward.world.Plan.find.row(s, c, (x + 1 : U32), y)

def lanternward.world.Plan.find.rows(+rows: List<&2, String>, +c: U32, +y: U32) -> List<&2, U32>:
  match rows:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      List.append(&2, U32, lanternward.world.Plan.find.row(h, c, 0, y),
        lanternward.world.Plan.find.rows(t, c, (y + 1 : U32)))

def lanternward.world.Plan.find(+room: U32, +c: U32) -> List<&2, U32>:
  lanternward.world.Plan.find.rows(lanternward.world.rows(lanternward.world.plan(room)), c, 0)

# Walls and bars stop a body for good; the rest is for the props.
def lanternward.world.wall(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 35), U32.is_eq(c, 86))

# The closet a secret hides, as tiles (x0, y0, x1, y1); none outside rooms 1-3.
def lanternward.world.closet(+room: U32) -> J.Val:
  match room:
    case 1:
      J.JA{[J.JU{28}, J.JU{13}, J.JU{28}, J.JU{13}]}
    case 2:
      J.JA{[J.JU{21}, J.JU{14}, J.JU{23}, J.JU{14}]}
    case 3:
      J.JA{[J.JU{30}, J.JU{12}, J.JU{30}, J.JU{12}]}
    case _:
      J.JNull{}

def lanternward.world.packed_x(+p: U32) -> U32:
  U32.and(p, 65535)

def lanternward.world.packed_y(+p: U32) -> U32:
  U32.shrn(p, 16n)

# ---- cases ----

# A pair as the JSON array [x, y] of two i32.
def lanternward.pair_j(p: U32 & U32) -> J.Val:
  (x, y) = p
  ga.pair_i(x, y)

def case.lanternward.world.isqrt(+input: J.Val) -> J.Val:
  W.to_j(lanternward.world.isqrt(W.of_j(J.get(input, "n"))))

def case.lanternward.world.dist(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.dist(ga.u(input, "dx"), ga.u(input, "dy")))

def case.lanternward.world.unit(+input: J.Val) -> J.Val:
  lanternward.pair_j(lanternward.world.unit(ga.u(input, "dx"), ga.u(input, "dy")))

def case.lanternward.world.dot(+input: J.Val) -> J.Val:
  +a = J.get(input, "a")
  +b = J.get(input, "b")
  J.of_i32(lanternward.world.dot4(ga.nu(a, 0), ga.nu(a, 1), ga.nu(b, 0), ga.nu(b, 1)))

def case.lanternward.world.angle_of(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.angle_of(ga.u(input, "dx"), ga.u(input, "dy")))

def case.lanternward.world.sin_q(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.sin_q(ga.u(input, "deg")))

def case.lanternward.world.cos_q(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.cos_q(ga.u(input, "deg")))

def case.lanternward.world.dir(+input: J.Val) -> J.Val:
  lanternward.pair_j(lanternward.world.dir(ga.u(input, "deg")))

def case.lanternward.world.turn(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.turn(ga.u(input, "a"), ga.u(input, "b")))

def case.lanternward.world.tile_of(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.tile_of(ga.u(input, "v")))

def case.lanternward.world.middle(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.middle(ga.u(input, "t")))

def case.lanternward.world.Plan.w(+input: J.Val) -> J.Val:
  J.JU{lanternward.world.Plan.w(ga.u(input, "room"))}

def case.lanternward.world.Plan.h(+input: J.Val) -> J.Val:
  J.JU{lanternward.world.Plan.h(ga.u(input, "room"))}

def case.lanternward.world.Plan.dark(+input: J.Val) -> J.Val:
  J.JBool{lanternward.world.Plan.dark(ga.u(input, "room"))}

def case.lanternward.world.closet(+input: J.Val) -> J.Val:
  lanternward.world.closet(ga.u(input, "room"))

def case.lanternward.world.Plan.at(+input: J.Val) -> J.Val:
  J.JU{lanternward.world.Plan.at(ga.u(input, "room"), ga.u(input, "tx"), ga.u(input, "ty"))}

def lanternward.packed_j(+xs: List<&2, U32>) -> J.Val:
  match xs:
    case Nil{}:
      J.JA{Nil{}}
    case Con{+h, +t}:
      J.JA{Con{ga.pair_i(lanternward.world.packed_x(h), lanternward.world.packed_y(h)), J.items(lanternward.packed_j(t))}}

def case.lanternward.world.Plan.find(+input: J.Val) -> J.Val:
  lanternward.packed_j(lanternward.world.Plan.find(ga.u(input, "room"), ga.u(input, "c")))

def case.lanternward.world.wall(+input: J.Val) -> J.Val:
  J.JBool{lanternward.world.wall(ga.u(input, "c"))}
