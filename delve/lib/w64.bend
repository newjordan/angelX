import Base
import ./j.bend as J

# u64 arithmetic with Rust's release semantics (wrapping add/sub/mul, shift
# amounts masked to 6 bits, division by zero answers 0). A u64 is two U32
# halves. W is Data: every parameter here is reusable.

type W is Data:
  W{hi: U32, lo: U32}

# Construction and parts
# ----------------------

def zero() -> W:
  W{0, 0}

def one() -> W:
  W{0, 1}

def max_value() -> W:
  W{4294967295, 4294967295}

def of_u32(+v: U32) -> W:
  W{0, v}

def of_parts(+hi: U32, +lo: U32) -> W:
  W{hi, lo}

# `x as u32`: the low half.
def lo32(+a: W) -> U32:
  match a:
    case W{+h, +l}:
      l

def hi32(+a: W) -> U32:
  match a:
    case W{+h, +l}:
      h

# Bits
# ----

def xor(+a: W, +b: W) -> W:
  match a b:
    case W{+ah, +al} W{+bh, +bl}:
      W{U32.xor(ah, bh), U32.xor(al, bl)}

def and(+a: W, +b: W) -> W:
  match a b:
    case W{+ah, +al} W{+bh, +bl}:
      W{U32.and(ah, bh), U32.and(al, bl)}

def or(+a: W, +b: W) -> W:
  match a b:
    case W{+ah, +al} W{+bh, +bl}:
      W{U32.or(ah, bh), U32.or(al, bl)}

def not(+a: W) -> W:
  match a:
    case W{+h, +l}:
      W{U32.not(h), U32.not(l)}

# Comparing
# ---------

def eq(+a: W, +b: W) -> Bool:
  match a b:
    case W{+ah, +al} W{+bh, +bl}:
      Bool.and(U32.is_eq(ah, bh), U32.is_eq(al, bl))

def ne(+a: W, +b: W) -> Bool:
  Bool.not(eq(a, b))

def lt.hi(same: Bool, +ah: U32, +bh: U32, +al: U32, +bl: U32) -> Bool:
  match same:
    case True{}:
      U32.is_lt(al, bl)
    case False{}:
      U32.is_lt(ah, bh)

def lt(+a: W, +b: W) -> Bool:
  match a b:
    case W{+ah, +al} W{+bh, +bl}:
      lt.hi(U32.is_eq(ah, bh), ah, bh, al, bl)

def le(+a: W, +b: W) -> Bool:
  Bool.not(lt(b, a))

def gt(+a: W, +b: W) -> Bool:
  lt(b, a)

def ge(+a: W, +b: W) -> Bool:
  Bool.not(lt(a, b))

def is_zero(+a: W) -> Bool:
  eq(a, zero())

def min(+a: W, +b: W) -> W:
  Bool.pick(W, lt(b, a), b, a)

def max(+a: W, +b: W) -> W:
  Bool.pick(W, lt(a, b), b, a)

# Adding and multiplying
# ----------------------

def add(+a: W, +b: W) -> W:
  match a b:
    case W{+ah, +al} W{+bh, +bl}:
      +l = (al + bl : U32)
      W{(ah + bh + Bool.to_u32(U32.is_lt(l, al)) : U32), l}

def sub(+a: W, +b: W) -> W:
  match a b:
    case W{+ah, +al} W{+bh, +bl}:
      W{(ah - bh - Bool.to_u32(U32.is_lt(al, bl)) : U32), (al - bl : U32)}

def neg(+a: W) -> W:
  sub(zero(), a)

def add_u32(+a: W, +b: U32) -> W:
  add(a, W{0, b})

def sub_u32(+a: W, +b: U32) -> W:
  sub(a, W{0, b})

# The full 64-bit product of two U32, from 16-bit limbs.
def mul32.go(+p00: U32, +p01: U32, +p10: U32, +p11: U32) -> W:
  +mid = ((p00 >> 16n : U32) + U32.and(p01, 65535) + U32.and(p10, 65535) : U32)
  W{((p11 + (p01 >> 16n : U32) + (p10 >> 16n : U32) + (mid >> 16n : U32)) : U32),
    U32.or((mid << 16n : U32), U32.and(p00, 65535))}

def mul32(+x: U32, +y: U32) -> W:
  +x0 = U32.and(x, 65535)
  +x1 = (x >> 16n : U32)
  +y0 = U32.and(y, 65535)
  +y1 = (y >> 16n : U32)
  mul32.go((x0 * y0 : U32), (x0 * y1 : U32), (x1 * y0 : U32), (x1 * y1 : U32))

def mul.fin(+p: W, +cross: U32) -> W:
  match p:
    case W{+h, +l}:
      W{(h + cross : U32), l}

def mul(+a: W, +b: W) -> W:
  match a b:
    case W{+ah, +al} W{+bh, +bl}:
      mul.fin(mul32(al, bl), (al * bh + ah * bl : U32))

def mul_u32(+a: W, +b: U32) -> W:
  mul(a, W{0, b})

# Shifts: the amount is masked to 6 bits as Rust's release `<<` and `>>` do.
def shl32(+x: U32, +n: U32) -> U32:
  U32.shln(x, U32.to_nat(n))

def shr32(+x: U32, +n: U32) -> U32:
  U32.shrn(x, U32.to_nat(n))

def shl.go(wide: Bool, +h: U32, +l: U32, +n: U32) -> W:
  match wide:
    case True{}:
      W{shl32(l, (n - 32 : U32)), 0}
    case False{}:
      W{U32.or(shl32(h, n), shr32(l, (32 - n : U32))), shl32(l, n)}

def shl(+a: W, +n: U32) -> W:
  match a:
    case W{+h, +l}:
      shl.go(U32.is_ge(U32.and(n, 63), 32), h, l, U32.and(n, 63))

def shr.go(wide: Bool, +h: U32, +l: U32, +n: U32) -> W:
  match wide:
    case True{}:
      W{0, shr32(h, (n - 32 : U32))}
    case False{}:
      W{shr32(h, n), U32.or(shr32(l, n), shl32(h, (32 - n : U32)))}

def shr(+a: W, +n: U32) -> W:
  match a:
    case W{+h, +l}:
      shr.go(U32.is_ge(U32.and(n, 63), 32), h, l, U32.and(n, 63))

def rotl(+a: W, +n: U32) -> W:
  or(shl(a, n), shr(a, (64 - U32.and(n, 63) : U32)))

def rotr(+a: W, +n: U32) -> W:
  or(shr(a, n), shl(a, (64 - U32.and(n, 63) : U32)))

# Division
# --------

# One step of restoring division: bring the next bit of `a` into the
# remainder, and subtract the divisor when it fits. `carry` marks a remainder
# that overflowed 64 bits, which is certainly not below the divisor.
def divmod.step(+q: W, +r: W, +a: W, +b: W) -> W & W & W:
  +top = shr(a, 63)
  +carry = U32.is_ge(hi32(r), 2147483648)
  +r1 = or(shl(r, 1), top)
  +sub_it = Bool.or(carry, ge(r1, b))
  +q1 = or(shl(q, 1), Bool.pick(W, sub_it, one(), zero()))
  (q1, Bool.pick(W, sub_it, sub(r1, b), r1), shl(a, 1))

def divmod.go?(n: Nat, +q: W, +r: W, +a: W, +b: W) -> W & W:
  match n:
    case 0n:
      (q, r)
    case 1n+p:
      divmod.go.next(p, divmod.step(q, r, a, b), b)

def divmod.go.next?(n: Nat, t: W & W & W, +b: W) -> W & W:
  (q, r, a) = t
  divmod.go(n, q, r, a, b)

def divmod.small(+a: W, +b: W) -> W & W:
  (W{0, U32.div(lo32(a), lo32(b))}, W{0, U32.mod(lo32(a), lo32(b))})

def divmod.pick(small: Bool, +a: W, +b: W) -> W & W:
  match small:
    case True{}:
      divmod.small(a, b)
    case False{}:
      divmod.go(64n, zero(), zero(), a, b)

# Quotient and remainder together; a zero divisor answers (0, 0).
def divmod.zero(z: Bool, +a: W, +b: W) -> W & W:
  match z:
    case True{}:
      (zero(), zero())
    case False{}:
      divmod.pick(Bool.and(U32.is_zero(hi32(a)), U32.is_zero(hi32(b))), a, b)

def divmod(+a: W, +b: W) -> W & W:
  divmod.zero(is_zero(b), a, b)

def div.fin(qr: W & W) -> W:
  (q, r) = qr
  q

def div(+a: W, +b: W) -> W:
  div.fin(divmod(a, b))

def rem.fin(qr: W & W) -> W:
  (q, r) = qr
  r

def rem(+a: W, +b: W) -> W:
  rem.fin(divmod(a, b))

def div_u32(+a: W, +b: U32) -> W:
  div(a, W{0, b})

# `a % b` for a u64 and a u32 divisor: the remainder is below b, so a U32.
def rem_u32(+a: W, +b: U32) -> U32:
  lo32(rem(a, W{0, b}))

# Saturating ops, min/max, abs_diff
# ---------------------------------

def sat_add(+a: W, +b: W) -> W:
  +s = add(a, b)
  Bool.pick(W, lt(s, a), max_value(), s)

def sat_sub(+a: W, +b: W) -> W:
  Bool.pick(W, lt(a, b), zero(), sub(a, b))

def abs_diff(+a: W, +b: W) -> W:
  Bool.pick(W, lt(a, b), sub(b, a), sub(a, b))

# Bit counts
# ----------

def bitlen32.go(n: Nat, +x: U32, +acc: U32) -> U32:
  match n:
    case 0n:
      acc
    case 1n+p:
      bitlen32.go(p, (x >> 1n : U32), (acc + Bool.to_u32(U32.is_ne(x, 0)) : U32))

# The number of significant bits of a U32 (0 for 0).
def bitlen32(+x: U32) -> U32:
  bitlen32.go(32n, x, 0)

def bitlen(+a: W) -> U32:
  match a:
    case W{+h, +l}:
      Bool.pick(U32, U32.is_zero(h), bitlen32(l), (32 + bitlen32(h) : U32))

def leading_zeros(+a: W) -> U32:
  (64 - bitlen(a) : U32)

def count_ones32.go(n: Nat, +x: U32, +acc: U32) -> U32:
  match n:
    case 0n:
      acc
    case 1n+p:
      count_ones32.go(p, (x >> 1n : U32), (acc + U32.and(x, 1) : U32))

def count_ones(+a: W) -> U32:
  (count_ones32.go(32n, hi32(a), 0) + count_ones32.go(32n, lo32(a), 0) : U32)

def trailing_zeros(+a: W) -> U32:
  Bool.pick(U32, is_zero(a), 64, (bitlen(and(a, neg(a))) - 1 : U32))

# More u64
# --------

def is_multiple_of(+a: W, +b: W) -> Bool:
  Bool.pick(Bool, is_zero(b), is_zero(a), is_zero(rem(a, b)))

# floor(sqrt(a)): the root is below 2^32, so try each of its bits from the top.
def isqrt.go(n: Nat, +a: W, +res: U32, +bit: U32) -> U32:
  match n:
    case 0n:
      res
    case 1n+p:
      +cand = U32.or(res, bit)
      isqrt.go(p, a, Bool.pick(U32, le(mul32(cand, cand), a), cand, res), (bit >> 1n : U32))

def isqrt(+a: W) -> W:
  W{0, isqrt.go(32n, a, 0, 2147483648)}

# `a.pow(e)`, wrapping.
def pow.go(n: Nat, +a: W, +e: U32, +r: W) -> W:
  match n:
    case 0n:
      r
    case 1n+p:
      pow.go(p, mul(a, a), (e >> 1n : U32), Bool.pick(W, U32.is_ne(U32.and(e, 1), 0), mul(r, a), r))

def pow(+a: W, +e: U32) -> W:
  pow.go(32n, a, e, one())

def sat_mul(+a: W, +b: W) -> W:
  Bool.pick(W, Bool.and(Bool.not(is_zero(a)), gt(b, div(max_value(), a))), max_value(), mul(a, b))

def div_ceil(+a: W, +b: W) -> W:
  add(div(a, b), Bool.pick(W, ne(rem(a, b), zero()), one(), zero()))

def checked.go(overflow: Bool, +v: W) -> Maybe<&2, W>:
  match overflow:
    case True{}:
      None{}
    case False{}:
      Some{v}

def checked_add(+a: W, +b: W) -> Maybe<&2, W>:
  checked.go(lt(add(a, b), a), add(a, b))

def checked_sub(+a: W, +b: W) -> Maybe<&2, W>:
  checked.go(lt(a, b), sub(a, b))

def checked_mul(+a: W, +b: W) -> Maybe<&2, W>:
  checked.go(Bool.and(Bool.not(is_zero(a)), gt(b, div(max_value(), a))), mul(a, b))

# Signed 64-bit (two's complement in the same W)
# ---------------------------------------------

def s_is_neg(+a: W) -> Bool:
  U32.is_ge(hi32(a), 2147483648)

def s_of_i32(+v: U32) -> W:
  W{Bool.pick(U32, U32.is_ge(v, 2147483648), 4294967295, 0), v}

def s_abs(+a: W) -> W:
  Bool.pick(W, s_is_neg(a), neg(a), a)

def s_lt(+a: W, +b: W) -> Bool:
  lt(xor(a, W{2147483648, 0}), xor(b, W{2147483648, 0}))

def s_le(+a: W, +b: W) -> Bool:
  Bool.not(s_lt(b, a))

def s_gt(+a: W, +b: W) -> Bool:
  s_lt(b, a)

def s_ge(+a: W, +b: W) -> Bool:
  Bool.not(s_lt(a, b))

def s_min(+a: W, +b: W) -> W:
  Bool.pick(W, s_lt(b, a), b, a)

def s_max(+a: W, +b: W) -> W:
  Bool.pick(W, s_lt(a, b), b, a)

def s_clamp(+x: W, +lo: W, +hi: W) -> W:
  s_min(s_max(x, lo), hi)

# -1, 0 or 1.
def s_signum(+a: W) -> W:
  Bool.pick(W, s_is_neg(a), max_value(), Bool.pick(W, is_zero(a), zero(), one()))

def s_negif(+flip: Bool, +a: W) -> W:
  Bool.pick(W, flip, neg(a), a)

# Truncating division and remainder (a zero divisor answers 0).
def s_div(+a: W, +b: W) -> W:
  s_negif(Bool.xor(s_is_neg(a), s_is_neg(b)), div(s_abs(a), s_abs(b)))

def s_rem(+a: W, +b: W) -> W:
  s_negif(s_is_neg(a), rem(s_abs(a), s_abs(b)))

def s_div_euclid.fix(+q: W, +r: W, +b: W) -> W:
  Bool.pick(W, s_is_neg(r), Bool.pick(W, s_is_neg(b), add(q, one()), sub(q, one())), q)

def s_div_euclid(+a: W, +b: W) -> W:
  s_div_euclid.fix(s_div(a, b), s_rem(a, b), b)

def s_rem_euclid(+a: W, +b: W) -> W:
  +r = s_rem(a, b)
  Bool.pick(W, s_is_neg(r), add(r, s_abs(b)), r)

# `>>` on a signed value, the amount masked to 6 bits.
def s_sar(+a: W, +n: U32) -> W:
  Bool.pick(W, s_is_neg(a), not(shr(not(a), n)), shr(a, n))

# Casts
# -----

def as_f32.make(+w: U32) -> F32:
  match w:
    case U32{d}:
      F32{d}

# The mantissa carries the leading 1 at bit 23; rounding up past 2^24 lets the
# add carry into the exponent field, which is the right renormalization.
def as_f32.bits(+exp: U32, +m: U32) -> F32:
  as_f32.make(((exp << 23n : U32) + m - 8388608 : U32))

# `x as f32`, rounded to nearest even. Above 32 bits take the top 24 bits and
# round on the rest, so the value is rounded once.
def as_f32.big(+a: W, +len: U32) -> F32:
  +s = (len - 24 : U32)
  +m = shr(a, s)
  +rest = sub(a, shl(m, s))
  +half = shl(one(), (s - 1 : U32))
  +up = Bool.or(gt(rest, half), Bool.and(eq(rest, half), U32.is_ne(U32.and(lo32(m), 1), 0)))
  as_f32.bits((len - 1 + 127 : U32), (lo32(m) + Bool.to_u32(up) : U32))

def as_f32.pick(small: Bool, +a: W) -> F32:
  match small:
    case True{}:
      U32.to_f32(lo32(a))
    case False{}:
      as_f32.big(a, bitlen(a))

def as_f32(+a: W) -> F32:
  as_f32.pick(U32.is_zero(hi32(a)), a)

def s_as_f32(+a: W) -> F32:
  Bool.pick(F32, s_is_neg(a), F32.neg(as_f32(neg(a))), as_f32(a))

# The J tree
# ----------

def of_j.fin(hl: U32 & U32) -> W:
  (h, l) = hl
  W{h, l}

def of_j(+v: J.Val) -> W:
  of_j.fin(J.w64(v))

def to_j(+a: W) -> J.Val:
  match a:
    case W{+h, +l}:
      J.of_w64(h, l)
