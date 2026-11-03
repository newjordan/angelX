# Lanternward's fixed ground: whole-number math. A position is pixels times Q
# (256), a tile is 16 pixels, an angle is a whole degree (0 east, 90 south, the
# screen's way round), a direction is a vector of length Q. Nothing here moves.
#
# Bend matches only parameters, so every choice between two numbers is a mask
# or `Bool.pick`, and vectors are passed as their two words (or as a V).
import Base
import ./n.bend as N

# A pair of words: a direction, a point.
type V is Data:
  V{x: U32, y: U32}

def vx(+v: V) -> U32:
  match v:
    case V{x, _}:
      x

def vy(+v: V) -> U32:
  match v:
    case V{_, y}:
      y

# A tile's column or row of a position in Q pixels.
def tile_of(+v: U32) -> U32: N.sar12(v)

# The middle of tile t, in Q pixels.
def middle(+t: U32) -> U32: U32.add(U32.mul(t, 4096), 2048)

# How long (dx, dy) is, in its own units.
def dist(+dx: U32, +dy: U32) -> U32: N.isqrt(N.u64_add(N.sq(dx), N.sq(dy)))

# n >> by, by a word.
def shr(+n: U32, +by: U32) -> U32: U32.shrn(n, U32.to_nat(by))

def shl(+n: U32, +by: U32) -> U32: U32.shln(n, U32.to_nat(by))

# How many bits it takes to write n (0 for 0).
def bits(+n: U32) -> U32:
  +a = U32.mul(N.flag(U32.is_ge(n, 65536)), 16)
  +n1 = shr(n, a)
  +b = U32.mul(N.flag(U32.is_ge(n1, 256)), 8)
  +n2 = shr(n1, b)
  +c = U32.mul(N.flag(U32.is_ge(n2, 16)), 4)
  +n3 = shr(n2, c)
  +d = U32.mul(N.flag(U32.is_ge(n3, 4)), 2)
  +n4 = shr(n3, d)
  +e = N.flag(U32.is_ge(n4, 2))
  U32.add(U32.add(a, b), U32.add(U32.add(c, d), U32.add(e, shr(n4, e))))

# How far (dx, dy) is grown so its longer side is 24 bits whole: a one-pixel
# step is then as true a direction as a hundred. Nothing for a longer side
# already past 24 bits.
def grow(+dx: U32, +dy: U32) -> U32:
  N.smax(U32.sub(24, bits(U32.max(N.sabs(dx), N.sabs(dy)))), 0)

# The grown length of (sx, sy), at least 1.
def glen(+sx: U32, +sy: U32) -> U32:
  U32.max(N.isqrt(N.u64_add(N.sq(sx), N.sq(sy))), 1)

# v scaled to Q over len: |v| * Q fits a word when v is under 24 bits, which is
# every direction this adventure takes; anything wider takes the 64-bit walk.
def scaled_go(fast: Bool, +v: U32, +len: U32) -> U32:
  match fast:
    case True{}:
      N.cond_neg(U32.div(U32.mul(N.sabs(v), 256), len), N.mask(v))
    case False{}:
      N.cond_neg(N.u64_div(N.mul32(N.sabs(v), 256), len), N.mask(v))

def scaled(+v: U32, +len: U32) -> U32: scaled_go(U32.is_lt(N.sabs(v), 16777216), v, len)

# (dx, dy) as a vector of length Q; nothing for no length.
def unit(+dx: U32, +dy: U32) -> V:
  +s = grow(dx, dy)
  +sx = shl(dx, s)
  +sy = shl(dy, s)
  +len = glen(sx, sy)
  V{scaled(sx, len), scaled(sy, len)}

# The dot product of two Q vectors, in Q.
def dot(+ax: U32, +ay: U32, +bx: U32, +by: U32) -> U32:
  N.sdiv(U32.add(U32.mul(ax, bx), U32.mul(ay, by)), 256)

# Sine of a whole-degree angle in Q (Bhaskara's rational, good to a fifth of
# a percent).
def sin_q(+deg: U32) -> U32:
  +d0 = N.rem_euclid(deg, 360)
  # d0 >= 180 folds into d0 - 180 and flips the sign; the sign bit says which.
  +d = U32.sub(d0, U32.mul(180, N.flag(N.sge(d0, 180))))
  +p = U32.mul(d, U32.sub(180, d))
  +den = U32.sub(40500, p)
  +mag = U32.div(U32.add(U32.mul(1024, p), U32.shrn(den, 1n)), den)
  N.cond_neg(mag, N.mask(N.sneg(N.flag(N.sge(d0, 180)))))

def cos_q(+deg: U32) -> U32: sin_q(U32.add(deg, 90))

# The whole-degree angle of (dx, dy), 0 for no length: of the degrees that
# point its way, the one with the least sideways error. The walk over all 360
# degrees moves by masks, so it is one tail call.
def angle_go(+fuel: Nat, +ux: U32, +uy: U32, +best_side: U32, +best_deg: U32) -> U32:
  match fuel:
    case 0n:
      best_deg
    case 1n+p:
      +d = U32.from_nat(Nat.sub(360n, fuel))
      +cx = cos_q(d)
      +cy = sin_q(d)
      +ahead = N.sgt(U32.add(U32.mul(cx, ux), U32.mul(cy, uy)), 0)
      +side = N.sabs(U32.sub(U32.mul(cx, uy), U32.mul(cy, ux)))
      +m = N.mask(N.sneg(N.flag(Bool.and(ahead, U32.is_lt(side, best_side)))))
      angle_go(p, ux, uy, N.cond(m, side, best_side), N.cond(m, d, best_deg))

def angle_of(+dx: U32, +dy: U32) -> U32:
  +u = unit(dx, dy)
  angle_go(360n, vx(u), vy(u), 4294967295, 0)

# The turn from angle a to angle b, -180 to 180.
def turn(+a: U32, +b: U32) -> U32:
  U32.sub(N.rem_euclid(U32.add(U32.sub(b, a), 180), 360), 180)
