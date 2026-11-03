import Base

# Integer arithmetic for the ride, in Bend. Every signed 32-bit number travels
# as the same bits Rust keeps (a U32 holding the two's complement), so wrapping
# add, subtract and multiply are the plain U32 ones. A comparison flips the
# sign bit, then compares unsigned. A product Rust widens to i64 is computed
# in Nat magnitudes, which hold 2^48 - 1; every one in this game is under 2^46.
#
# A choice is its own def (`pk`), because Bend refuses to match a computed
# value and only a def's parameter can be matched. Both arms of a `pk` are
# computed, so an arm that is costly or may not exist is a def that matches.

def pk(+t: U32, +f: U32, +c: Bool) -> U32:
  match c:
    case True{}:
      t
    case False{}:
      f

def pkb(+c: Bool, +t: Bool, +f: Bool) -> Bool:
  match c:
    case True{}:
      t
    case False{}:
      f

def b2u(+c: Bool) -> U32:
  match c:
    case True{}:
      1
    case False{}:
      0

def nz(+a: U32) -> Bool:
  Bool.not(U32.is_eq(a, 0))

# ---- signed 32-bit on two's complement bits -------------------------------

def neg(+a: U32) -> U32:
  (0 - a : U32)

def isneg(+a: U32) -> Bool:
  (a >= 2147483648 : U32)

def lt(+a: U32, +b: U32) -> Bool:
  (U32.xor(a, 2147483648) < U32.xor(b, 2147483648) : U32)

def le(+a: U32, +b: U32) -> Bool:
  Bool.not(lt(b, a))

def gt(+a: U32, +b: U32) -> Bool:
  lt(b, a)

def ge(+a: U32, +b: U32) -> Bool:
  Bool.not(lt(a, b))

def eq(+a: U32, +b: U32) -> Bool:
  U32.is_eq(a, b)

def min(+a: U32, +b: U32) -> U32:
  pk(a, b, lt(a, b))

def max(+a: U32, +b: U32) -> U32:
  pk(a, b, gt(a, b))

def abs(+a: U32) -> U32:
  pk(neg(a), a, isneg(a))

# The sign of a number: -1, 0 or 1.
def sgn(+a: U32) -> U32:
  pk(4294967295, pk(1, 0, gt(a, 0)), isneg(a))

def clamp(+v: U32, +lo: U32, +hi: U32) -> U32:
  max(lo, min(hi, v))

# A saturating subtract on a number that is never negative.
def sat(+a: U32, +b: U32) -> U32:
  pk(0, (a - b : U32), (a < b : U32))

def sat1(+a: U32) -> U32:
  sat(a, 1)

def div_go(+negative: Bool, +q: U32) -> U32:
  pk(neg(q), q, negative)

# Rust's `/` truncates toward zero, and `%` carries the dividend's sign.
def div(+a: U32, +b: U32) -> U32:
  div_go(Bool.xor(isneg(a), isneg(b)), U32.div(abs(a), abs(b)))

def rem(+a: U32, +b: U32) -> U32:
  (a - (div(a, b) * b : U32) : U32)

def rem_euc_go(+r: U32, +m: U32) -> U32:
  pk((r + m : U32), r, isneg(r))

# Rust's `rem_euclid` for a positive modulus: never negative.
def rem_euc(+a: U32, +m: U32) -> U32:
  rem_euc_go(rem(a, m), m)

# ---- widened intermediates ------------------------------------------------

# `trunc(a * b / d)` for signed 32-bit inputs, the product under 2^48.
def md(+a: U32, +b: U32, +d: U32) -> U32:
  +p = Nat.mul(U32.to_nat(abs(a)), U32.to_nat(abs(b)))
  +qm = Nat.div(p, Nat.max(1n, U32.to_nat(abs(d))))
  div_go(Bool.xor(Bool.xor(isneg(a), isneg(b)), isneg(d)), U32.from_nat(qm))

# `dx * dx + dy * dy` in 48 bits: the length squared of a vector.
def len2(+dx: U32, +dy: U32) -> Nat:
  +x = U32.to_nat(abs(dx))
  +y = U32.to_nat(abs(dy))
  Nat.add(Nat.mul(x, x), Nat.mul(y, y))

# Newton's walk over a bounded number of steps: the count is the structural
# argument the checker follows, and it stops on its own condition exactly as
# the Rust loop does. Sixty-four steps settle any 48-bit input with room to
# spare, so the bound never changes an answer.
def isqrt.go(+steps: Nat, +more: Bool, +n: Nat, +x: Nat, +y: Nat) -> Nat:
  match steps more:
    case 0n _:
      x
    case 1n+k True{}:
      +next = Nat.div(Nat.add(y, Nat.div(n, y)), 2n)
      isqrt.go(k, Nat.is_lt(next, y), n, y, next)
    case 1n+k False{}:
      x

# The integer square root: the largest r with r * r <= n.
def isqrt(+n: Nat) -> Nat:
  match n:
    case 0n:
      0n
    case 1n:
      1n
    case _:
      isqrt.go(64n, Nat.is_lt(Nat.div(Nat.add(n, 1n), 2n), n), n, n, Nat.div(Nat.add(n, 1n), 2n))

# The length of the vector (dx, dy), as Rust's `isqrt(dx * dx + dy * dy)`.
def hyp(+dx: U32, +dy: U32) -> U32:
  U32.from_nat(isqrt(len2(dx, dy)))

# ---- the tables ----------------------------------------------------------

# A quarter of a sine wave, 65 steps of 90/64 degrees, in 12 bits. (Each arm adds
# `z`, which is always zero: the arms stay a chain of tests, since the compiler's
# table form leaves the module's last line unfinished.)
def sin_q(+k: U32, +z: U32) -> U32:
  match k:
    case 0:
      (0 + z : U32)
    case 1:
      (101 + z : U32)
    case 2:
      (201 + z : U32)
    case 3:
      (301 + z : U32)
    case 4:
      (401 + z : U32)
    case 5:
      (501 + z : U32)
    case 6:
      (601 + z : U32)
    case 7:
      (700 + z : U32)
    case 8:
      (799 + z : U32)
    case 9:
      (897 + z : U32)
    case 10:
      (995 + z : U32)
    case 11:
      (1092 + z : U32)
    case 12:
      (1189 + z : U32)
    case 13:
      (1285 + z : U32)
    case 14:
      (1380 + z : U32)
    case 15:
      (1474 + z : U32)
    case 16:
      (1567 + z : U32)
    case 17:
      (1660 + z : U32)
    case 18:
      (1751 + z : U32)
    case 19:
      (1842 + z : U32)
    case 20:
      (1931 + z : U32)
    case 21:
      (2019 + z : U32)
    case 22:
      (2106 + z : U32)
    case 23:
      (2191 + z : U32)
    case 24:
      (2276 + z : U32)
    case 25:
      (2359 + z : U32)
    case 26:
      (2440 + z : U32)
    case 27:
      (2520 + z : U32)
    case 28:
      (2598 + z : U32)
    case 29:
      (2675 + z : U32)
    case 30:
      (2751 + z : U32)
    case 31:
      (2824 + z : U32)
    case 32:
      (2896 + z : U32)
    case 33:
      (2967 + z : U32)
    case 34:
      (3035 + z : U32)
    case 35:
      (3102 + z : U32)
    case 36:
      (3166 + z : U32)
    case 37:
      (3229 + z : U32)
    case 38:
      (3290 + z : U32)
    case 39:
      (3349 + z : U32)
    case 40:
      (3406 + z : U32)
    case 41:
      (3461 + z : U32)
    case 42:
      (3513 + z : U32)
    case 43:
      (3564 + z : U32)
    case 44:
      (3612 + z : U32)
    case 45:
      (3659 + z : U32)
    case 46:
      (3703 + z : U32)
    case 47:
      (3745 + z : U32)
    case 48:
      (3784 + z : U32)
    case 49:
      (3822 + z : U32)
    case 50:
      (3857 + z : U32)
    case 51:
      (3889 + z : U32)
    case 52:
      (3920 + z : U32)
    case 53:
      (3948 + z : U32)
    case 54:
      (3973 + z : U32)
    case 55:
      (3996 + z : U32)
    case 56:
      (4017 + z : U32)
    case 57:
      (4036 + z : U32)
    case 58:
      (4052 + z : U32)
    case 59:
      (4065 + z : U32)
    case 60:
      (4076 + z : U32)
    case 61:
      (4085 + z : U32)
    case 62:
      (4091 + z : U32)
    case 63:
      (4095 + z : U32)
    case 64:
      (4096 + z : U32)
    case _:
      4096

# The arctangent of k/64 in brads, for k in 0..=64 (45 degrees is 128).
def atan_q(+k: U32, +z: U32) -> U32:
  match k:
    case 0:
      (0 + z : U32)
    case 1:
      (3 + z : U32)
    case 2:
      (5 + z : U32)
    case 3:
      (8 + z : U32)
    case 4:
      (10 + z : U32)
    case 5:
      (13 + z : U32)
    case 6:
      (15 + z : U32)
    case 7:
      (18 + z : U32)
    case 8:
      (20 + z : U32)
    case 9:
      (23 + z : U32)
    case 10:
      (25 + z : U32)
    case 11:
      (28 + z : U32)
    case 12:
      (30 + z : U32)
    case 13:
      (33 + z : U32)
    case 14:
      (35 + z : U32)
    case 15:
      (38 + z : U32)
    case 16:
      (40 + z : U32)
    case 17:
      (42 + z : U32)
    case 18:
      (45 + z : U32)
    case 19:
      (47 + z : U32)
    case 20:
      (49 + z : U32)
    case 21:
      (52 + z : U32)
    case 22:
      (54 + z : U32)
    case 23:
      (56 + z : U32)
    case 24:
      (58 + z : U32)
    case 25:
      (61 + z : U32)
    case 26:
      (63 + z : U32)
    case 27:
      (65 + z : U32)
    case 28:
      (67 + z : U32)
    case 29:
      (69 + z : U32)
    case 30:
      (71 + z : U32)
    case 31:
      (74 + z : U32)
    case 32:
      (76 + z : U32)
    case 33:
      (78 + z : U32)
    case 34:
      (80 + z : U32)
    case 35:
      (82 + z : U32)
    case 36:
      (84 + z : U32)
    case 37:
      (85 + z : U32)
    case 38:
      (87 + z : U32)
    case 39:
      (89 + z : U32)
    case 40:
      (91 + z : U32)
    case 41:
      (93 + z : U32)
    case 42:
      (95 + z : U32)
    case 43:
      (96 + z : U32)
    case 44:
      (98 + z : U32)
    case 45:
      (100 + z : U32)
    case 46:
      (102 + z : U32)
    case 47:
      (103 + z : U32)
    case 48:
      (105 + z : U32)
    case 49:
      (106 + z : U32)
    case 50:
      (108 + z : U32)
    case 51:
      (110 + z : U32)
    case 52:
      (111 + z : U32)
    case 53:
      (113 + z : U32)
    case 54:
      (114 + z : U32)
    case 55:
      (116 + z : U32)
    case 56:
      (117 + z : U32)
    case 57:
      (119 + z : U32)
    case 58:
      (120 + z : U32)
    case 59:
      (121 + z : U32)
    case 60:
      (123 + z : U32)
    case 61:
      (124 + z : U32)
    case 62:
      (125 + z : U32)
    case 63:
      (127 + z : U32)
    case 64:
      (128 + z : U32)
    case _:
      128

# The sine of `turns` (1/256 of a circle), in 12 bits.
def sin(+turns: U32) -> U32:
  +a = rem_euc(turns, 256)
  +k = U32.mod(a, 64)
  +quarter = U32.div(a, 64)
  +odd = U32.is_eq(U32.mod(quarter, 2), 1)
  +base = pk(sin_q((64 - k : U32), 0), sin_q(k, 0), odd)
  pk(neg(base), base, (quarter >= 2 : U32))

def cos(+turns: U32) -> U32:
  sin((turns + 64 : U32))

# The direction of the vector (dx, dy) as a 12-bit unit vector.
def unit_x(+dx: U32, +dy: U32) -> U32:
  +len = hyp(dx, dy)
  pk(md(dx, 4096, len), 4096, nz(len))

def unit_y(+dx: U32, +dy: U32) -> U32:
  +len = hyp(dx, dy)
  pk(md(dy, 4096, len), 0, nz(len))

# The first octant's angle for the smaller over the larger.
def octant(+small: U32, +large: U32) -> U32:
  +r = md(small, 4096, large)
  +k = U32.div(r, 64)
  +f = U32.mod(r, 64)
  +a = atan_q(k, 0)
  +b = atan_q((k + 1 : U32), 0)
  (a + div((((b - a) : U32) * f : U32), 64) : U32)

def atan_quadrant(+first: U32, +x_neg: Bool, +y_neg: Bool) -> U32:
  +east = pk((1024 - first : U32), first, y_neg)
  +west = pk((512 + first : U32), (512 - first : U32), y_neg)
  rem_euc(pk(west, east, x_neg), 1024)

def atan2_go(+x: U32, +y: U32) -> U32:
  +ax = abs(x)
  +ay = abs(y)
  +first = pk((256 - octant(ax, ay) : U32), octant(ay, ax), gt(ay, ax))
  atan_quadrant(first, isneg(x), isneg(y))

# The angle of the vector (x, y) in brads, 0 to 1023, measured on screen (so
# +y, down, is 256).
def atan2(+y: U32, +x: U32) -> U32:
  pk(0, atan2_go(x, y), Bool.and(eq(x, 0), eq(y, 0)))

# `to` reached from `from` by the shorter way round, each step at most `slew`,
# with the rest of the gap eased by `ease`.
def turn_toward(+from: U32, +to: U32, +ease: U32, +slew: U32) -> U32:
  +gap = (rem_euc((to + 512 - from : U32), 1024) - 512 : U32)
  +step = clamp(div(gap, max(ease, 1)), neg(slew), slew)
  +small = Bool.and(Bool.not(eq(gap, 0)), eq(step, 0))
  rem_euc((from + pk(sgn(gap), step, small) : U32), 1024)

# An S-shaped blend, 0 to 4096 over 0 to 4096, flat at both ends.
def smooth(+t: U32) -> U32:
  +c = clamp(t, 0, 4096)
  +cc = Nat.mul(U32.to_nat(c), U32.to_nat(c))
  +inner = Nat.sub(12288n, Nat.mul(2n, U32.to_nat(c)))
  U32.from_nat(Nat.div(Nat.mul(cc, inner), 16777216n))

# ---- values too wide for 32 bits --------------------------------------------

# A signed value that does not fit 32 bits rides as a Nat with a bias of 2^46,
# so the sum of two of them, the difference, and a comparison stay Nat
# arithmetic. Every such value in this game is under 2^40.
def zb() -> Nat:
  Nat.mul(16777216n, 4194304n)

# The product of two signed 32-bit values.
def zmul(+a: U32, +b: U32) -> Nat:
  +m = Nat.mul(U32.to_nat(abs(a)), U32.to_nat(abs(b)))
  Bool.pick(Nat, Bool.xor(isneg(a), isneg(b)), Nat.sub(zb(), m), Nat.add(zb(), m))

def zadd(+a: Nat, +b: Nat) -> Nat:
  Nat.sub(Nat.add(a, b), zb())

def zsub(+a: Nat, +b: Nat) -> Nat:
  Nat.sub(Nat.add(a, zb()), b)

# The magnitude of a biased value.
def zmag(+a: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(a, zb()), Nat.sub(zb(), a), Nat.sub(a, zb()))

# `trunc(a * 65536 / b)` for two biased values, held to 0..=65536 (a negative
# quotient is held to 0): how far along a line from `a` to `a - b` zero lies.
def zratio(+a: Nat, +b: Nat) -> U32:
  +same = Bool.not(Bool.xor(Nat.is_lt(a, zb()), Nat.is_lt(b, zb())))
  +q = Nat.div(Nat.mul(zmag(a), 65536n), Nat.max(1n, zmag(b)))
  pk(U32.from_nat(Nat.min(q, 65536n)), 0, Bool.and(same, Bool.not(Nat.is_eq(b, zb()))))
