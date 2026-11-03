import Base
import ./j.bend as J
import ./w64.bend as W

# Rust-exact scalar operations (Rust 1.95 release semantics, x86_64 Linux).
# An i32 is a U32 holding the two's complement pattern. Names: `i_*` signed
# 32-bit, `u_*` unsigned 32-bit, `f_*` f32. Division by zero never happens in
# the Rust code; it answers 0 here.

# i32
# ---

def i_neg(+a: U32) -> U32:
  (0 - a : U32)

def i_add(+a: U32, +b: U32) -> U32:
  (a + b : U32)

def i_sub(+a: U32, +b: U32) -> U32:
  (a - b : U32)

def i_mul(+a: U32, +b: U32) -> U32:
  (a * b : U32)

def i_is_neg(+a: U32) -> Bool:
  U32.is_ge(a, 2147483648)

# Wrapping, as a release build's `abs` is: i32::MIN stays MIN.
def i_abs(+a: U32) -> U32:
  Bool.pick(U32, i_is_neg(a), i_neg(a), a)

def i_unsigned_abs(+a: U32) -> U32:
  i_abs(a)

def i_lt(+a: U32, +b: U32) -> Bool:
  U32.is_lt(U32.xor(a, 2147483648), U32.xor(b, 2147483648))

def i_le(+a: U32, +b: U32) -> Bool:
  U32.is_le(U32.xor(a, 2147483648), U32.xor(b, 2147483648))

def i_gt(+a: U32, +b: U32) -> Bool:
  i_lt(b, a)

def i_ge(+a: U32, +b: U32) -> Bool:
  i_le(b, a)

def i_eq(+a: U32, +b: U32) -> Bool:
  U32.is_eq(a, b)

def i_ne(+a: U32, +b: U32) -> Bool:
  U32.is_ne(a, b)

def i_min(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, i_lt(b, a), b, a)

def i_max(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, i_lt(a, b), b, a)

# Rust's `clamp` panics when min > max; the rules never do that.
def i_clamp(+x: U32, +lo: U32, +hi: U32) -> U32:
  i_min(i_max(x, lo), hi)

# -1, 0 or 1.
def i_signum(+a: U32) -> U32:
  Bool.pick(U32, i_is_neg(a), 4294967295, Bool.to_u32(U32.is_ne(a, 0)))

# Truncating division: the quotient's sign is the product of the signs.
def i_div.sign(+unlike: Bool, +q: U32) -> U32:
  Bool.pick(U32, unlike, i_neg(q), q)

def i_div(+a: U32, +b: U32) -> U32:
  i_div.sign(Bool.xor(i_is_neg(a), i_is_neg(b)), U32.div(i_abs(a), i_abs(b)))

# Truncating remainder takes the sign of the dividend.
def i_rem(+a: U32, +b: U32) -> U32:
  i_div.sign(i_is_neg(a), U32.mod(i_abs(a), i_abs(b)))

def i_div_euclid.fix(+q: U32, +r: U32, +b: U32) -> U32:
  Bool.pick(U32, i_is_neg(r), Bool.pick(U32, i_is_neg(b), (q + 1 : U32), (q - 1 : U32)), q)

def i_div_euclid(+a: U32, +b: U32) -> U32:
  i_div_euclid.fix(i_div(a, b), i_rem(a, b), b)

def i_rem_euclid(+a: U32, +b: U32) -> U32:
  +r = i_rem(a, b)
  Bool.pick(U32, i_is_neg(r), (r + i_abs(b) : U32), r)

def i_abs_diff(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, i_lt(a, b), (b - a : U32), (a - b : U32))

# `>>` on a signed value shifts the sign in; the amount is masked to 5 bits.
def i_shr.go(+a: U32, +n: U32) -> U32:
  Bool.pick(U32, i_is_neg(a),
    U32.not(U32.shrn(U32.not(a), U32.to_nat(n))),
    U32.shrn(a, U32.to_nat(n)))

def i_shr(+a: U32, +n: U32) -> U32:
  i_shr.go(a, U32.and(n, 31))

def i_shl(+a: U32, +n: U32) -> U32:
  U32.shln(a, U32.to_nat(U32.and(n, 31)))

# Overflow only happens when the operands agree in sign and the sum does not.
def i_sat.fix(+an: Bool, +bn: Bool, +s: U32) -> U32:
  Bool.pick(U32, Bool.and(Bool.not(Bool.xor(an, bn)), Bool.xor(an, i_is_neg(s))),
    Bool.pick(U32, an, 2147483648, 2147483647), s)

def i_sat_add(+a: U32, +b: U32) -> U32:
  i_sat.fix(i_is_neg(a), i_is_neg(b), (a + b : U32))

def i_sat_sub.fix(+an: Bool, +bn: Bool, +s: U32) -> U32:
  Bool.pick(U32, Bool.and(Bool.xor(an, bn), Bool.xor(an, i_is_neg(s))),
    Bool.pick(U32, an, 2147483648, 2147483647), s)

def i_sat_sub(+a: U32, +b: U32) -> U32:
  i_sat_sub.fix(i_is_neg(a), i_is_neg(b), (a - b : U32))

# Narrow casts: `as i8`, `as i16`, `as u8`, `as u16` of a 32-bit pattern.
def as_u8(+a: U32) -> U32:
  U32.and(a, 255)

def as_u16(+a: U32) -> U32:
  U32.and(a, 65535)

def as_i8.fix(+b: U32) -> U32:
  Bool.pick(U32, U32.is_ge(b, 128), (b + 4294967040 : U32), b)

def as_i8(+a: U32) -> U32:
  as_i8.fix(U32.and(a, 255))

def as_i16.fix(+b: U32) -> U32:
  Bool.pick(U32, U32.is_ge(b, 32768), (b + 4294901760 : U32), b)

def as_i16(+a: U32) -> U32:
  as_i16.fix(U32.and(a, 65535))

def u_sat_add.fix(+s: U32, +a: U32) -> U32:
  Bool.pick(U32, U32.is_lt(s, a), 4294967295, s)

# u32 / usize
# -----------

def u_sat_add(+a: U32, +b: U32) -> U32:
  u_sat_add.fix((a + b : U32), a)

def u_sat_sub(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_lt(a, b), 0, (a - b : U32))

def u_sat_mul(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ne(a, 0), U32.is_gt(b, U32.div(4294967295, a))), 4294967295, (a * b : U32))

def u_min(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_lt(b, a), b, a)

def u_max(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_lt(a, b), b, a)

def u_clamp(+x: U32, +lo: U32, +hi: U32) -> U32:
  u_min(u_max(x, lo), hi)

def u_abs_diff(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_lt(a, b), (b - a : U32), (a - b : U32))

def u_shl(+a: U32, +n: U32) -> U32:
  U32.shln(a, U32.to_nat(U32.and(n, 31)))

def u_shr(+a: U32, +n: U32) -> U32:
  U32.shrn(a, U32.to_nat(U32.and(n, 31)))

def u_rotl(+a: U32, +n: U32) -> U32:
  U32.or(u_shl(a, n), U32.shrn(a, U32.to_nat((32 - U32.and(n, 31) : U32))))

def u_rotr(+a: U32, +n: U32) -> U32:
  U32.or(u_shr(a, n), U32.shln(a, U32.to_nat((32 - U32.and(n, 31) : U32))))

def u_count_ones(+a: U32) -> U32:
  W.count_ones32.go(32n, a, 0)

def u_bitlen(+a: U32) -> U32:
  W.bitlen32(a)

def u_leading_zeros(+a: U32) -> U32:
  (32 - W.bitlen32(a) : U32)

def u_trailing_zeros(+a: U32) -> U32:
  Bool.pick(U32, U32.is_zero(a), 32, (W.bitlen32(U32.and(a, (0 - a : U32))) - 1 : U32))

# floor(sqrt(a)): try each bit of the root from the top.
def u_isqrt.go(n: Nat, +a: U32, +res: U32, +bit: U32) -> U32:
  match n:
    case 0n:
      res
    case 1n+p:
      +cand = U32.or(res, bit)
      u_isqrt.go(p, a, Bool.pick(U32, U32.is_le((cand * cand : U32), a), cand, res), (bit >> 1n : U32))

def u_isqrt(+a: U32) -> U32:
  u_isqrt.go(16n, a, 0, 32768)

# `a.pow(n)`, wrapping.
def u_pow.go(n: Nat, +a: U32, +e: U32, +r: U32) -> U32:
  match n:
    case 0n:
      r
    case 1n+p:
      u_pow.go(p, (a * a : U32), (e >> 1n : U32),
        Bool.pick(U32, U32.is_ne(U32.and(e, 1), 0), (r * a : U32), r))

def u_pow(+a: U32, +e: U32) -> U32:
  u_pow.go(32n, a, e, 1)

def i_pow(+a: U32, +e: U32) -> U32:
  u_pow(a, e)

# f32
# ---

def f_from_bits(+w: U32) -> F32:
  match w:
    case U32{d}:
      F32{d}

def f_bits(+x: F32) -> U32:
  F32.bits(x)

def f_is_nan(+x: F32) -> Bool:
  F32.is_ne(x, x)

def f_is_finite(+x: F32) -> Bool:
  U32.is_lt(U32.and(F32.bits(x), 2147483647), 2139095040)

def f_abs(+x: F32) -> F32:
  f_from_bits(U32.and(F32.bits(x), 2147483647))

def f_neg(+x: F32) -> F32:
  f_from_bits(U32.xor(F32.bits(x), 2147483648))

def f_copysign(+x: F32, +s: F32) -> F32:
  f_from_bits(U32.or(U32.and(F32.bits(x), 2147483647), U32.and(F32.bits(s), 2147483648)))

# NaN stays NaN; any other value gives 1.0 with its sign (so -0.0 gives -1.0).
def f_signum(+x: F32) -> F32:
  Bool.pick(F32, f_is_nan(x), x, f_from_bits(U32.or(1065353216, U32.and(F32.bits(x), 2147483648))))

# Rust's `min` and `max` answer the other operand when one is NaN.
def f_min(+a: F32, +b: F32) -> F32:
  Bool.pick(F32, f_is_nan(a), b, Bool.pick(F32, f_is_nan(b), a, Bool.pick(F32, F32.is_lt(b, a), b, a)))

def f_max(+a: F32, +b: F32) -> F32:
  Bool.pick(F32, f_is_nan(a), b, Bool.pick(F32, f_is_nan(b), a, Bool.pick(F32, F32.is_lt(a, b), b, a)))

def f_clamp.hi(+x: F32, +hi: F32) -> F32:
  Bool.pick(F32, F32.is_gt(x, hi), hi, x)

# `if x < lo { x = lo }; if x > hi { x = hi }`: NaN stays NaN.
def f_clamp(+x: F32, +lo: F32, +hi: F32) -> F32:
  f_clamp.hi(Bool.pick(F32, F32.is_lt(x, lo), lo, x), hi)

def f_rem_euclid.fix(+r: F32, +y: F32) -> F32:
  Bool.pick(F32, F32.is_lt(r, 0.0), F32.add(r, f_abs(y)), r)

def f_rem_euclid(+x: F32, +y: F32) -> F32:
  f_rem_euclid.fix(F32.mod(x, y), y)

def f_div_euclid.fix(+q: F32, +r: F32, +y: F32) -> F32:
  Bool.pick(F32, F32.is_lt(r, 0.0), Bool.pick(F32, F32.is_gt(y, 0.0), F32.sub(q, 1.0), F32.add(q, 1.0)), q)

def f_div_euclid(+x: F32, +y: F32) -> F32:
  f_div_euclid.fix(F32.trunc(F32.div(x, y)), F32.mod(x, y), y)

def f_fract(+x: F32) -> F32:
  F32.sub(x, F32.trunc(x))

# Half away from zero. trunc is exact, so is the remainder.
def f_round.fix(+x: F32, +t: F32) -> F32:
  Bool.pick(F32, F32.is_ge(f_abs(F32.sub(x, t)), 0.5), F32.add(t, f_signum(x)), t)

def f_round(+x: F32) -> F32:
  f_round.fix(x, F32.trunc(x))

def f_powi.next?(done: Bool, n: Nat, +a: F32, +e: U32, +r: F32) -> F32:
  match done:
    case True{}:
      r
    case False{}:
      f_powi.go(n, F32.mul(a, a), (e >> 1n : U32), r)

# compiler-rt's __powisf2: square-and-multiply, then one reciprocal.
def f_powi.go?(n: Nat, +a: F32, +e: U32, +r: F32) -> F32:
  match n:
    case 0n:
      r
    case 1n+p:
      f_powi.next(U32.is_zero((e >> 1n : U32)), p, a, e, Bool.pick(F32, U32.is_ne(U32.and(e, 1), 0), F32.mul(r, a), r))

def f_powi.recip(+neg: Bool, +r: F32) -> F32:
  Bool.pick(F32, neg, F32.div(1.0, r), r)

def f_powi(+x: F32, +n: U32) -> F32:
  f_powi.recip(i_is_neg(n), f_powi.go(33n, x, i_abs(n), 1.0))

# `to_degrees` multiplies by a constant; `to_radians` by (PI / 180) as an f32.
def f_pi() -> F32:
  f_from_bits(1078530011)

def f_to_degrees(+x: F32) -> F32:
  F32.mul(x, f_from_bits(1113927393))

def f_to_radians(+x: F32) -> F32:
  F32.mul(x, F32.div(f_pi(), 180.0))

def f_lerp(+a: F32, +b: F32, +t: F32) -> F32:
  F32.add(a, F32.mul(F32.sub(b, a), t))

# Casts
# -----

# `x as u32`: truncates toward zero, saturates, NaN is 0. Base's to_u32 already
# answers 0 for NaN and anything outside [0, 2^32).
def f_as_u32(+x: F32) -> U32:
  Bool.pick(U32, F32.is_ge(x, 4294967296.0), 4294967295, F32.to_u32(x))

# usize is held in a U32 by the port.
def f_as_usize(+x: F32) -> U32:
  f_as_u32(x)

def f_as_i32.neg(+x: F32) -> U32:
  Bool.pick(U32, F32.is_le(x, F32.neg(2147483648.0)), 2147483648, i_neg(F32.to_u32(F32.neg(x))))

def f_as_i32(+x: F32) -> U32:
  Bool.pick(U32, F32.is_ge(x, 2147483648.0), 2147483647,
    Bool.pick(U32, F32.is_lt(x, 0.0), f_as_i32.neg(x), F32.to_u32(x)))

# `x as u64`: x = m * 2^(e - 150) for a 24-bit m once x is 2^32 or more.
def f_as_u64.big(+x: F32) -> W.W:
  +bits = F32.bits(x)
  W.shl(W.of_u32(U32.or(U32.and(bits, 8388607), 8388608)), (U32.and(U32.shrn(bits, 23n), 255) - 150 : U32))

def f_as_u64(+x: F32) -> W.W:
  Bool.pick(W.W, F32.is_ge(x, f_from_bits(1602224128)), W.max_value(),
    Bool.pick(W.W, F32.is_ge(x, 4294967296.0), f_as_u64.big(x), W.of_u32(F32.to_u32(x))))

def u_as_f32(+a: U32) -> F32:
  U32.to_f32(a)

def i_as_f32(+a: U32) -> F32:
  Bool.pick(F32, i_is_neg(a), F32.neg(U32.to_f32(i_neg(a))), U32.to_f32(a))

# libm-backed methods. Base's F32.sin/cos/exp/log/pow call the DOUBLE libm and
# round to f32, which is not what Rust's f32::sin/cos/exp/ln/powf (glibc's
# sinf/cosf/expf/logf/powf) give in about 1 call in 1000. Port code must call
# these RS names; the ones that match Base bit for bit are plain aliases, the
# rest are exact glibc emulations (see the cases in delve/rules/numerics).
def f_sqrt(+x: F32) -> F32:
  F32.sqrt(x)

def f_floor(+x: F32) -> F32:
  F32.floor(x)

def f_ceil(+x: F32) -> F32:
  F32.ceil(x)

def f_trunc(+x: F32) -> F32:
  F32.trunc(x)

def f_tan(+x: F32) -> F32:
  F32.tan(x)

def f_atan(+x: F32) -> F32:
  F32.atan(x)

def f_asin(+x: F32) -> F32:
  F32.asin(x)

def f_acos(+x: F32) -> F32:
  F32.acos(x)

# hypotf, exactly as glibc 2.35+ does it on x86_64:
#   (float) sqrt((double) x * x + (double) y * y)
# with every rounding (the double sum, the double sqrt, the float result)
# done in integers. A finite nonzero |x| is m * 2^e with m below 2^24.
type HypSq is Data:
  HypSq{n: W.W, e: U32}

def f_hyp.mant(+b: U32) -> U32:
  Bool.pick(U32, U32.is_lt(b, 8388608), b, U32.or(U32.and(b, 8388607), 8388608))

# The binary exponent of the unit bit, as an i32 pattern.
def f_hyp.exp(+b: U32) -> U32:
  Bool.pick(U32, U32.is_lt(b, 8388608), (0 - 149 : U32), ((b >> 23n : U32) - 150 : U32))

# x*x with its top bit moved to bit 63: value = n * 2^e.
def f_hyp.sq.go(+sq: W.W, +e2: U32) -> HypSq:
  +sh = (64 - W.bitlen(sq) : U32)
  HypSq{W.shl(sq, sh), (e2 * 2 - sh : U32)}

def f_hyp.sq(+b: U32) -> HypSq:
  f_hyp.sq.go(W.mul32(f_hyp.mant(b), f_hyp.mant(b)), f_hyp.exp(b))

# Round a 64-bit value to 53 bits: m is the kept part, rb the next bit, st
# anything below it.
def f_hyp.renorm(+over: Bool, +m: W.W, +e: U32) -> HypSq:
  match over:
    case True{}:
      HypSq{W.shl(W.one(), 52), (e + 1 : U32)}
    case False{}:
      HypSq{m, e}

def f_hyp.round(+m: W.W, +rb: Bool, +st: Bool, +e: U32) -> HypSq:
  +up = Bool.and(rb, Bool.or(st, U32.is_ne(U32.and(W.lo32(m), 1), 0)))
  +m2 = W.add(m, Bool.pick(W.W, up, W.one(), W.zero()))
  f_hyp.renorm(W.eq(m2, W.shl(W.one(), 53)), m2, e)

def f_hyp.sum2(+carry: Bool, +lost: Bool, +s: W.W, +he: U32) -> HypSq:
  match carry:
    case True{}:
      f_hyp.round(W.or(W.shr(s, 12), W.shl(W.one(), 52)),
        W.ne(W.and(s, W.shl(W.one(), 11)), W.zero()),
        Bool.or(lost, W.ne(W.and(s, W.of_u32(2047)), W.zero())), (he + 12 : U32))
    case False{}:
      f_hyp.round(W.shr(s, 11),
        W.ne(W.and(s, W.shl(W.one(), 10)), W.zero()),
        Bool.or(lost, W.ne(W.and(s, W.of_u32(1023)), W.zero())), (he + 11 : U32))

# hn * 2^he plus ln * 2^(he - k): align, add, round to a double.
def f_hyp.sum1(+k: U32, +hn: W.W, +he: U32, +ln: W.W) -> HypSq:
  +small = Bool.pick(W.W, U32.is_lt(k, 64), W.shr(ln, k), W.zero())
  +lost = Bool.pick(Bool, U32.is_lt(k, 64), W.ne(W.shl(small, k), ln), True{})
  +s = W.add(hn, small)
  f_hyp.sum2(W.lt(s, hn), lost, s, he)

def f_hyp.sum(+hi: HypSq, +lo: HypSq) -> HypSq:
  match hi lo:
    case HypSq{+hn, +he} HypSq{+ln, +le}:
      f_hyp.sum1((he - le : U32), hn, he, ln)

# floor(sqrt(m)) for m below 2^54: 27 bits, from the top.
def f_isqrt54.go(n: Nat, +m: W.W, +res: U32, +bit: U32) -> U32:
  match n:
    case 0n:
      res
    case 1n+p:
      +cand = U32.or(res, bit)
      f_isqrt54.go(p, m, Bool.pick(U32, W.le(W.mul32(cand, cand), m), cand, res), (bit >> 1n : U32))

def f_isqrt54(+m: W.W) -> U32:
  f_isqrt54.go(27n, m, 0, 67108864)

# Round s * 2^e (plus a sliver when `sticky`) to f32 bits, nearest even, into
# subnormals and up to infinity. s has 27 significant bits.
def f_roundpack.go(+shift: U32, +q: U32, +rb: Bool, +st: Bool, +e: U32) -> U32:
  +up = Bool.and(rb, Bool.or(st, U32.is_ne(U32.and(q, 1), 0)))
  +bits = (((e + shift + 150 : U32) << 23n : U32) + q + Bool.to_u32(up) - 8388608 : U32)
  Bool.pick(U32, U32.is_ge(bits, 2139095040), 2139095040, bits)

def f_roundpack(+s: U32, +e: U32, +sticky: Bool) -> U32:
  +shift = i_max(3, (0 - e - 149 : U32))
  +sm = U32.to_nat((shift - 1 : U32))
  f_roundpack.go(shift, U32.shrn(s, U32.to_nat(shift)),
    U32.is_ne(U32.and(U32.shrn(s, sm), 1), 0),
    Bool.or(sticky, U32.is_ne(U32.and(s, (U32.shln(1, sm) - 1 : U32)), 0)), e)

def f_hyp.sqrt2(+mq: W.W, +ee: U32) -> U32:
  +s = f_isqrt54(mq)
  f_roundpack(s, i_shr(ee, 1), Bool.not(W.eq(W.mul32(s, s), mq)))

def f_hyp.sqrt1(+odd: Bool, +m: W.W, +e: U32) -> U32:
  f_hyp.sqrt2(Bool.pick(W.W, odd, W.shl(m, 1), m), Bool.pick(U32, odd, (e - 1 : U32), e))

def f_hyp.sqrt(+d: HypSq) -> U32:
  match d:
    case HypSq{+m, +e}:
      f_hyp.sqrt1(U32.is_ne(U32.and(e, 1), 0), m, e)

def f_hyp.e(+a: HypSq) -> U32:
  match a:
    case HypSq{+n, +e}:
      e

def f_hyp.ord(+a_first: Bool, +a: HypSq, +b: HypSq) -> U32:
  match a_first:
    case True{}:
      f_hyp.sqrt(f_hyp.sum(a, b))
    case False{}:
      f_hyp.sqrt(f_hyp.sum(b, a))

def f_hyp.nonzero(+bx: U32, +by: U32) -> U32:
  f_hyp.ord(i_ge(f_hyp.e(f_hyp.sq(bx)), f_hyp.e(f_hyp.sq(by))), f_hyp.sq(bx), f_hyp.sq(by))

def f_hyp.zero.y(+y_zero: Bool, +bx: U32, +by: U32) -> U32:
  match y_zero:
    case True{}:
      bx
    case False{}:
      f_hyp.nonzero(bx, by)

def f_hyp.zero(+x_zero: Bool, +y_zero: Bool, +bx: U32, +by: U32) -> U32:
  match x_zero:
    case True{}:
      by
    case False{}:
      f_hyp.zero.y(y_zero, bx, by)

# Infinity or NaN in: infinity unless a signaling NaN is involved.
def f_hyp.special(+bx: U32, +by: U32) -> U32:
  +inf = Bool.or(U32.is_eq(bx, 2139095040), U32.is_eq(by, 2139095040))
  +snan = Bool.or(
    Bool.and(U32.is_gt(bx, 2139095040), U32.is_eq(U32.and(bx, 4194304), 0)),
    Bool.and(U32.is_gt(by, 2139095040), U32.is_eq(U32.and(by, 4194304), 0)))
  Bool.pick(U32, Bool.and(inf, Bool.not(snan)), 2139095040, 2143289344)

def f_hyp.top(+special: Bool, +bx: U32, +by: U32) -> U32:
  match special:
    case True{}:
      f_hyp.special(bx, by)
    case False{}:
      f_hyp.zero(U32.is_zero(bx), U32.is_zero(by), bx, by)

def f_hypot(+x: F32, +y: F32) -> F32:
  +bx = U32.and(F32.bits(x), 2147483647)
  +by = U32.and(F32.bits(y), 2147483647)
  f_from_bits(f_hyp.top(Bool.or(U32.is_ge(bx, 2139095040), U32.is_ge(by, 2139095040)), bx, by))

# ==== soft double ====
# IEEE binary64 arithmetic in integers, rounding to nearest even, for finite
# normal values and zero. glibc's sinf/cosf/expf/logf/powf compute in double
# (with fused multiply-adds on x86_64), so matching their last bit needs the
# same operations rounded the same way. A double is (-1)^neg * m * 2^e with
# 2^52 <= m < 2^53, or m = 0 for zero; e is an i32 pattern.

# A 128-bit register
# ------------------

type Q is Data:
  Q{h: W.W, l: W.W}

def q_zero() -> Q:
  Q{W.zero(), W.zero()}

def q_of_w(+a: W.W) -> Q:
  Q{W.zero(), a}

def q_lo(+a: Q) -> W.W:
  match a:
    case Q{+h, +l}:
      l

def q_is_zero(+a: Q) -> Bool:
  match a:
    case Q{+h, +l}:
      Bool.and(W.is_zero(h), W.is_zero(l))

def q_eq(+a: Q, +b: Q) -> Bool:
  match a b:
    case Q{+ah, +al} Q{+bh, +bl}:
      Bool.and(W.eq(ah, bh), W.eq(al, bl))

def q_lt.go(same: Bool, +ah: W.W, +bh: W.W, +al: W.W, +bl: W.W) -> Bool:
  match same:
    case True{}:
      W.lt(al, bl)
    case False{}:
      W.lt(ah, bh)

def q_lt(+a: Q, +b: Q) -> Bool:
  match a b:
    case Q{+ah, +al} Q{+bh, +bl}:
      q_lt.go(W.eq(ah, bh), ah, bh, al, bl)

def q_add(+a: Q, +b: Q) -> Q:
  match a b:
    case Q{+ah, +al} Q{+bh, +bl}:
      +l = W.add(al, bl)
      Q{W.add(W.add(ah, bh), Bool.pick(W.W, W.lt(l, al), W.one(), W.zero())), l}

def q_sub(+a: Q, +b: Q) -> Q:
  match a b:
    case Q{+ah, +al} Q{+bh, +bl}:
      Q{W.sub(W.sub(ah, bh), Bool.pick(W.W, W.lt(al, bl), W.one(), W.zero())), W.sub(al, bl)}

def q_bitlen(+a: Q) -> U32:
  match a:
    case Q{+h, +l}:
      Bool.pick(U32, W.is_zero(h), W.bitlen(l), (64 + W.bitlen(h) : U32))

# Shifts by 0..=127 bits.
def q_shl.go(wide: Bool, +h: W.W, +l: W.W, +s: U32) -> Q:
  match wide:
    case True{}:
      Q{W.shl(l, (s - 64 : U32)), W.zero()}
    case False{}:
      Q{W.or(W.shl(h, s), W.shr(W.shr(l, 1), (63 - s : U32))), W.shl(l, s)}

def q_shl(+a: Q, +s: U32) -> Q:
  match a:
    case Q{+h, +l}:
      q_shl.go(U32.is_ge(s, 64), h, l, s)

def q_shr.go(wide: Bool, +h: W.W, +l: W.W, +s: U32) -> Q:
  match wide:
    case True{}:
      Q{W.zero(), W.shr(h, (s - 64 : U32))}
    case False{}:
      Q{W.shr(h, s), W.or(W.shr(l, s), W.shl(W.shl(h, 1), (63 - s : U32)))}

def q_shr(+a: Q, +s: U32) -> Q:
  match a:
    case Q{+h, +l}:
      q_shr.go(U32.is_ge(s, 64), h, l, s)

def q_lost.go(all: Bool, +a: Q, +s: U32) -> Bool:
  match all:
    case True{}:
      Bool.not(q_is_zero(a))
    case False{}:
      Bool.not(q_eq(q_shl(q_shr(a, s), s), a))

# Whether the low s bits (0..=128) of a hold anything.
def q_lost(+a: Q, +s: U32) -> Bool:
  q_lost.go(U32.is_ge(s, 128), a, s)

# The 106-bit product of two numbers below 2^53.
def q_mul53(+a: W.W, +b: W.W) -> Q:
  +p00 = W.mul32(W.lo32(a), W.lo32(b))
  +p01 = W.mul32(W.lo32(a), W.hi32(b))
  +p10 = W.mul32(W.hi32(a), W.lo32(b))
  +p11 = W.mul32(W.hi32(a), W.hi32(b))
  +mid = W.add(p01, p10)
  q_add(q_add(Q{W.zero(), p00}, Q{W.shr(mid, 32), W.shl(mid, 32)}), Q{p11, W.zero()})

# Doubles
# -------

type D is Data:
  D{neg: Bool, m: W.W, e: U32}

def d_zero() -> D:
  D{False{}, W.zero(), 0}

def d_one() -> D:
  D{False{}, W.shl(W.one(), 52), (0 - 52 : U32)}

def d_neg(+d: D) -> D:
  match d:
    case D{+n, +m, +e}:
      D{Bool.not(n), m, e}

# Negated when `flip`.
def d_negif(+flip: Bool, +d: D) -> D:
  match flip:
    case True{}:
      d_neg(d)
    case False{}:
      d

def d_is_zero(+d: D) -> Bool:
  match d:
    case D{+n, +m, +e}:
      W.is_zero(m)

def d_is_neg(+d: D) -> Bool:
  match d:
    case D{+n, +m, +e}:
      n

# From the IEEE bit pattern (hi, lo): normal numbers and zero only.
def d_of_bits.go(+nz: Bool, +neg: Bool, +field: U32, +frac: W.W) -> D:
  match nz:
    case True{}:
      D{neg, W.or(frac, W.shl(W.one(), 52)), (field - 1075 : U32)}
    case False{}:
      d_zero()

def d_of_bits(+hi: U32, +lo: U32) -> D:
  d_of_bits.go(U32.is_ne(U32.and((hi >> 20n : U32), 2047), 0), U32.is_ge(hi, 2147483648),
    U32.and((hi >> 20n : U32), 2047), W.of_parts(U32.and(hi, 1048575), lo))

def d_of_w(+w: W.W) -> D:
  d_of_bits(W.hi32(w), W.lo32(w))

# The bit pattern of a normal double or zero.
def d_hi(+d: D) -> U32:
  match d:
    case D{+n, +m, +e}:
      Bool.pick(U32, W.is_zero(m), 0,
        U32.or(Bool.pick(U32, n, 2147483648, 0), U32.or(((e + 1075 : U32) << 20n : U32), U32.and(W.hi32(m), 1048575))))

def d_lo(+d: D) -> U32:
  match d:
    case D{+n, +m, +e}:
      Bool.pick(U32, W.is_zero(m), 0, W.lo32(m))

def d_to_w(+d: D) -> W.W:
  W.of_parts(d_hi(d), d_lo(d))

# Round r * 2^e (a sliver more when `st`) to 53 bits, nearest even.
def d_round.up(+neg: Bool, +m: W.W, +rb: Bool, +st: Bool, +e: U32) -> D:
  +up = Bool.and(rb, Bool.or(st, U32.is_ne(U32.and(W.lo32(m), 1), 0)))
  +m2 = W.add(m, Bool.pick(W.W, up, W.one(), W.zero()))
  Bool.pick(D, W.eq(m2, W.shl(W.one(), 53)), D{neg, W.shl(W.one(), 52), (e + 1 : U32)}, D{neg, m2, e})

# r has more than 53 bits: keep the top 53, s is how many bits go.
def d_round.wide(+neg: Bool, +r: Q, +e: U32, +st: Bool, +s: U32) -> D:
  d_round.up(neg, q_lo(q_shr(r, s)), U32.is_ne(U32.and(W.lo32(q_lo(q_shr(r, (s - 1 : U32)))), 1), 0),
    Bool.or(st, q_lost(r, (s - 1 : U32))), (e + s : U32))

def d_round.exact(+neg: Bool, +r: Q, +e: U32, +len: U32) -> D:
  D{neg, W.shl(q_lo(r), (53 - len : U32)), (e - (53 - len) : U32)}

def d_round.pick(+big: Bool, +neg: Bool, +r: Q, +e: U32, +st: Bool, +len: U32) -> D:
  match big:
    case True{}:
      d_round.wide(neg, r, e, st, (len - 53 : U32))
    case False{}:
      d_round.exact(neg, r, e, len)

def d_round.z(+z: Bool, +neg: Bool, +r: Q, +e: U32, +st: Bool) -> D:
  match z:
    case True{}:
      d_zero()
    case False{}:
      d_round.pick(U32.is_gt(q_bitlen(r), 53), neg, r, e, st, q_bitlen(r))

def d_round(+neg: Bool, +r: Q, +e: U32, +st: Bool) -> D:
  d_round.z(q_is_zero(r), neg, r, e, st)

def d_mul.go(+neg: Bool, +p: Q, +e: U32) -> D:
  d_round(neg, p, e, False{})

def d_mul(+a: D, +b: D) -> D:
  match a b:
    case D{+an, +am, +ae} D{+bn, +bm, +be}:
      Bool.pick(D, Bool.or(W.is_zero(am), W.is_zero(bm)), d_zero(),
        d_mul.go(Bool.xor(an, bn), q_mul53(am, bm), (ae + be : U32)))

# fma, last step: big and small are aligned at bit 126 (small shifted down by
# diff); the signs differ or agree.
def d_fma.sub(+bn: Bool, +big: Q, +e: U32, +sn: Bool, +small: Q, +lost: Bool) -> D:
  d_round(bn, q_sub(q_sub(big, small), Bool.pick(Q, lost, Q{W.zero(), W.one()}, q_zero())), e, lost)

def d_fma.sub0(+swap: Bool, +bn: Bool, +big: Q, +e: U32, +sn: Bool, +small: Q) -> D:
  match swap:
    case True{}:
      d_round(sn, q_sub(small, big), e, False{})
    case False{}:
      d_round(bn, q_sub(big, small), e, False{})

def d_fma.sub1(+level: Bool, +bn: Bool, +big: Q, +e: U32, +sn: Bool, +small: Q, +lost: Bool) -> D:
  match level:
    case True{}:
      d_fma.sub0(q_lt(big, small), bn, big, e, sn, small)
    case False{}:
      d_fma.sub(bn, big, e, sn, small, lost)

def d_fma.add(+same: Bool, +bn: Bool, +big: Q, +e: U32, +sn: Bool, +small: Q, +lost: Bool, +diff: U32) -> D:
  match same:
    case True{}:
      d_round(bn, q_add(big, small), e, lost)
    case False{}:
      d_fma.sub1(U32.is_zero(diff), bn, big, e, sn, small, lost)

def d_fma.align.go(+far: Bool, +bn: Bool, +big: Q, +e: U32, +sn: Bool, +small: Q, +diff: U32) -> D:
  match far:
    case True{}:
      d_fma.add(Bool.not(Bool.xor(bn, sn)), bn, big, e, sn, q_zero(), True{}, diff)
    case False{}:
      d_fma.add(Bool.not(Bool.xor(bn, sn)), bn, big, e, sn, q_shr(small, diff), q_lost(small, diff), diff)

def d_fma.align(+bn: Bool, +big: Q, +e: U32, +sn: Bool, +small: Q, +diff: U32) -> D:
  d_fma.align.go(U32.is_ge(diff, 128), bn, big, e, sn, small, diff)

# Order the product (pn, P at 2^ep) and the addend (cn, C at 2^ec) by exponent.
def d_fma.order(+pe_ge: Bool, +pn: Bool, +p: Q, +ep: U32, +cn: Bool, +c: Q, +ec: U32) -> D:
  match pe_ge:
    case True{}:
      d_fma.align(pn, p, ep, cn, c, (ep - ec : U32))
    case False{}:
      d_fma.align(cn, c, ec, pn, p, (ec - ep : U32))

def d_fma.norm.go(+pn: Bool, +p: Q, +ep: U32, +cn: Bool, +c: Q, +ec: U32) -> D:
  d_fma.order(i_ge(ep, ec), pn, p, ep, cn, c, ec)

def d_fma.norm(+pn: Bool, +p: Q, +ep: U32, +cn: Bool, +cm: W.W, +ec: U32) -> D:
  d_fma.norm.go(pn, q_shl(p, (127 - q_bitlen(p) : U32)), (ep - (127 - q_bitlen(p)) : U32), cn, q_shl(Q{W.zero(), cm}, 74), (ec - 74 : U32))

def d_fma.zc.go(+pn: Bool, +p: Q, +ep: U32, +c: D) -> D:
  match c:
    case D{+cn, +cm, +ce}:
      d_fma.norm(pn, p, ep, cn, cm, ce)

def d_fma.zc(+czero: Bool, +pn: Bool, +p: Q, +ep: U32, +c: D) -> D:
  match czero:
    case True{}:
      d_round(pn, p, ep, False{})
    case False{}:
      d_fma.zc.go(pn, p, ep, c)

def d_fma.pz(+pzero: Bool, +pn: Bool, +p: Q, +ep: U32, +c: D) -> D:
  match pzero:
    case True{}:
      c
    case False{}:
      d_fma.zc(d_is_zero(c), pn, p, ep, c)

# a * b + c with one rounding.
def d_fma(+a: D, +b: D, +c: D) -> D:
  match a b:
    case D{+an, +am, +ae} D{+bn, +bm, +be}:
      d_fma.pz(Bool.or(W.is_zero(am), W.is_zero(bm)), Bool.xor(an, bn), q_mul53(am, bm), (ae + be : U32), c)

def d_add(+a: D, +b: D) -> D:
  d_fma(a, d_one(), b)

def d_sub(+a: D, +b: D) -> D:
  d_fma(a, d_one(), d_neg(b))

# Conversions
# -----------

def d_of_f32.sub(+neg: Bool, +frac: U32) -> D:
  +sh = (53 - W.bitlen32(frac) : U32)
  D{neg, W.shl(W.of_u32(frac), sh), (0 - 149 - sh : U32)}

def d_of_f32.norm(+neg: Bool, +field: U32, +frac: U32) -> D:
  D{neg, W.shl(W.of_u32(U32.or(frac, 8388608)), 29), (field - 179 : U32)}

def d_of_f32.field(+z: Bool, +neg: Bool, +field: U32, +frac: U32) -> D:
  match z:
    case True{}:
      d_of_f32.sub(neg, frac)
    case False{}:
      d_of_f32.norm(neg, field, frac)

def d_of_f32.nz(+nz: Bool, +neg: Bool, +a: U32) -> D:
  match nz:
    case True{}:
      d_of_f32.field(U32.is_lt(a, 8388608), neg, (a >> 23n : U32), U32.and(a, 8388607))
    case False{}:
      d_zero()

# A finite f32, exactly.
def d_of_f32(+x: F32) -> D:
  d_of_f32.nz(U32.is_ne(U32.and(F32.bits(x), 2147483647), 0), U32.is_ge(F32.bits(x), 2147483648), U32.and(F32.bits(x), 2147483647))

def d_of_i32.go(+neg: Bool, +a: U32) -> D:
  +sh = (53 - W.bitlen32(a) : U32)
  D{neg, W.shl(W.of_u32(a), sh), (0 - sh : U32)}

# An i32 (pattern) as a double.
def d_of_i32(+n: U32) -> D:
  Bool.pick(D, U32.is_zero(n), d_zero(), d_of_i32.go(i_is_neg(n), i_abs(n)))

def d_of_i64.go(+neg: Bool, +w: W.W) -> D:
  d_round(neg, Q{W.zero(), Bool.pick(W.W, neg, W.neg(w), w)}, 0, False{})

# A two's complement 64-bit value as a double, rounded to nearest even.
def d_of_i64(+w: W.W) -> D:
  d_of_i64.go(U32.is_ge(W.hi32(w), 2147483648), w)

# `d as i32`, truncating toward zero (for |d| below 2^31).
def d_trunc_i32.go(+neg: Bool, +m: W.W, +s: U32) -> U32:
  Bool.pick(U32, neg, i_neg(W.lo32(W.shr(m, s))), W.lo32(W.shr(m, s)))

def d_trunc_i32(+d: D) -> U32:
  match d:
    case D{+n, +m, +e}:
      Bool.pick(U32, U32.is_ge((0 - e : U32), 64), 0, d_trunc_i32.go(n, m, (0 - e : U32)))

# f32 magnitude bits of m * 2^e, nearest even, into subnormals and infinity.
def d_f32.go(+shift: U32, +q: U32, +rb: Bool, +st: Bool, +e: U32) -> U32:
  +up = Bool.and(rb, Bool.or(st, U32.is_ne(U32.and(q, 1), 0)))
  +bits = (((e + shift + 150 : U32) << 23n : U32) + q + Bool.to_u32(up) - 8388608 : U32)
  Bool.pick(U32, U32.is_ge(bits, 2139095040), 2139095040, bits)

def d_f32.bits2(+m: W.W, +e: U32, +shift: U32) -> U32:
  d_f32.go(shift, W.lo32(W.shr(m, shift)),
    U32.is_ne(U32.and(W.lo32(W.shr(m, (shift - 1 : U32))), 1), 0),
    W.ne(W.shl(W.shr(m, (shift - 1 : U32)), (shift - 1 : U32)), m), e)

def d_f32.bits1(+shift: U32, +m: W.W, +e: U32) -> U32:
  Bool.pick(U32, U32.is_ge(shift, 64), 0, d_f32.bits2(m, e, shift))

def d_f32.bits(+m: W.W, +e: U32) -> U32:
  d_f32.bits1(i_max(29, (0 - e - 149 : U32)), m, e)

# `d as f32`.
def d_to_f32(+d: D) -> F32:
  match d:
    case D{+n, +m, +e}:
      f_from_bits(Bool.pick(U32, W.is_zero(m), Bool.pick(U32, n, 2147483648, 0),
        U32.or(Bool.pick(U32, n, 2147483648, 0), d_f32.bits(m, e))))

# Constants are written as their bit patterns.
def d_k(+hi: U32, +lo: U32) -> D:
  d_of_bits(hi, lo)

# ==== sinf and cosf ====
# glibc 2.44's __sinf_fma / __cosf_fma (sysdeps/ieee754/flt-32/s_sinf.c, the
# ARM optimized routines), transcribed from the disassembly with the fused
# multiply-adds where the compiler put them. The two functions share one body.

def f_sc.hpi_inv() -> D:
  d_k(1097097008, 1841940611)

def f_sc.hpi() -> D:
  d_k(1073291771, 1413754136)

def f_sc.pio4() -> D:
  d_k(1008280059, 1413754136)

def f_sc.c0() -> D:
  d_one()

def f_sc.c1() -> D:
  d_k(3219128319, 4245447196)

def f_sc.s1() -> D:
  d_k(3217380692, 1502979587)

def f_sc.c2() -> D:
  d_k(1067799891, 3775303449)

def f_sc.s2() -> D:
  d_k(1065422966, 86182852)

def f_sc.c3() -> D:
  d_k(3210133639, 3902420381)

def f_sc.s3() -> D:
  d_k(3207173355, 930402084)

def f_sc.c4() -> D:
  d_k(1056543555, 41679043)

# The 2/pi bits for large arguments (__inv_pio4).
def f_sc.arr(+i: U32) -> U32:
  Bool.pick(U32, U32.is_eq(i, 0), 162, Bool.pick(U32, U32.is_eq(i, 1), 41721, Bool.pick(U32, U32.is_eq(i, 2), 10680707, Bool.pick(U32, U32.is_eq(i, 3), 2734261102, Bool.pick(U32, U32.is_eq(i, 4), 4186140238, Bool.pick(U32, U32.is_eq(i, 5), 2205044292, Bool.pick(U32, U32.is_eq(i, 6), 1850622997, Bool.pick(U32, U32.is_eq(i, 7), 1313084713, Bool.pick(U32, U32.is_eq(i, 8), 1142237692, Bool.pick(U32, U32.is_eq(i, 9), 355073063, Bool.pick(U32, U32.is_eq(i, 10), 704390999, Bool.pick(U32, U32.is_eq(i, 11), 4230436817, Bool.pick(U32, U32.is_eq(i, 12), 660066805, Bool.pick(U32, U32.is_eq(i, 13), 1473377588, Bool.pick(U32, U32.is_eq(i, 14), 3522507997, Bool.pick(U32, U32.is_eq(i, 15), 4113882560, Bool.pick(U32, U32.is_eq(i, 16), 886948059, Bool.pick(U32, U32.is_eq(i, 17), 3720403810, Bool.pick(U32, U32.is_eq(i, 18), 3235603093, Bool.pick(U32, U32.is_eq(i, 19), 3680671129, Bool.pick(U32, U32.is_eq(i, 20), 1653971260, Bool.pick(U32, U32.is_eq(i, 21), 2509847619, Bool.pick(U32, U32.is_eq(i, 22), 2570863504, 1011060801)))))))))))))))))))))))

# sinf_poly for even n: the sine polynomial of x (already signed) and x2.
def f_sc.sin_even(+x: D, +x2: D) -> D:
  +x3 = d_mul(x, x2)
  +s1 = d_fma(x2, f_sc.s3(), f_sc.s2())
  +x7 = d_mul(x3, x2)
  +s = d_fma(x3, f_sc.s1(), x)
  d_fma(s1, x7, s)

# sinf_poly for odd n: the cosine polynomial; table 1 has c0..c4 negated.
def f_sc.cos_odd(+x2: D, +neg: Bool) -> D:
  +x4 = d_mul(x2, x2)
  +c2 = d_fma(x2, d_negif(neg, f_sc.c4()), d_negif(neg, f_sc.c3()))
  +c1 = d_fma(x2, d_negif(neg, f_sc.c1()), d_negif(neg, f_sc.c0()))
  +x6 = d_mul(x4, x2)
  +c = d_fma(x4, d_negif(neg, f_sc.c2()), c1)
  d_fma(c2, x6, c)

def f_sc.poly(+odd: Bool, +xs: D, +x2: D, +neg: Bool) -> D:
  match odd:
    case True{}:
      f_sc.cos_odd(x2, neg)
    case False{}:
      f_sc.sin_even(xs, x2)

# Reduced x, the index n + sign for the signs, and n for the parity.
def f_sc.finish(+is_sin: Bool, +xr: D, +ns: U32, +n: U32) -> F32:
  d_to_f32(f_sc.poly(U32.is_ne(U32.and(Bool.pick(U32, is_sin, n, U32.xor(n, 1)), 1), 0),
    d_negif(U32.is_ne(U32.and(U32.xor(ns, (ns >> 1n : U32)), 1), 0), xr), d_mul(xr, xr),
    U32.is_ne(U32.and(ns, 2), 0)))

def f_sc.fast.go(+is_sin: Bool, +xd: D, +n: U32) -> F32:
  f_sc.finish(is_sin, d_fma(d_neg(d_of_i32(n)), f_sc.hpi(), xd), n, n)

def f_sc.fast(+is_sin: Bool, +x: F32) -> F32:
  f_sc.fast.go(is_sin, d_of_f32(x), i_shr(i_add(d_trunc_i32(d_mul(d_of_f32(x), f_sc.hpi_inv())), 8388608), 24))

def f_sc.large.n(+is_sin: Bool, +sign: U32, +n: U32, +r0: W.W) -> F32:
  f_sc.finish(is_sin, d_mul(d_of_i64(W.sub(r0, W.shl(W.of_u32(n), 62))), f_sc.pio4()), (n + sign : U32), n)

def f_sc.large.r(+is_sin: Bool, +sign: U32, +r0: W.W) -> F32:
  f_sc.large.n(is_sin, sign, W.lo32(W.shr(W.add(r0, W.shl(W.one(), 61)), 62)), r0)

# reduce_large: n in the low bits of res0, the rest is the fraction of pi/4.
def f_sc.large.go(+is_sin: Bool, +sign: U32, +xi: U32, +k: U32) -> F32:
  f_sc.large.r(is_sin, sign,
    W.add(W.or(W.shr(W.mul32(xi, f_sc.arr((k + 8 : U32))), 32), W.shl(W.of_u32((xi * f_sc.arr(k) : U32)), 32)),
      W.mul32(xi, f_sc.arr((k + 4 : U32)))))

def f_sc.large(+is_sin: Bool, +x: F32) -> F32:
  f_sc.large.go(is_sin, (F32.bits(x) >> 31n : U32),
    U32.shln(U32.or(U32.and(F32.bits(x), 8388607), 8388608), U32.to_nat(U32.and((F32.bits(x) >> 23n : U32), 7))),
    U32.and((F32.bits(x) >> 26n : U32), 15))

def f_sc.small.go(+is_sin: Bool, +xd: D) -> F32:
  match is_sin:
    case True{}:
      d_to_f32(f_sc.sin_even(xd, d_mul(xd, xd)))
    case False{}:
      d_to_f32(f_sc.cos_odd(d_mul(xd, xd), False{}))

def f_sc.small.t(+tiny: Bool, +is_sin: Bool, +x: F32) -> F32:
  match tiny:
    case True{}:
      Bool.pick(F32, is_sin, x, 1.0)
    case False{}:
      f_sc.small.go(is_sin, d_of_f32(x))

def f_sc.small(+is_sin: Bool, +x: F32, +at: U32) -> F32:
  f_sc.small.t(U32.is_le(at, 919), is_sin, x)

def f_sc.big.t(+fast: Bool, +is_sin: Bool, +x: F32, +at: U32) -> F32:
  match fast:
    case True{}:
      f_sc.fast(is_sin, x)
    case False{}:
      Bool.pick(F32, U32.is_le(at, 2039), f_sc.large(is_sin, x), f_from_bits(4290772992))

def f_sc.big(+is_sin: Bool, +x: F32, +at: U32) -> F32:
  f_sc.big.t(U32.is_le(at, 1070), is_sin, x, at)

def f_sc.top.t(+small: Bool, +is_sin: Bool, +x: F32, +at: U32) -> F32:
  match small:
    case True{}:
      f_sc.small(is_sin, x, at)
    case False{}:
      f_sc.big(is_sin, x, at)

def f_sc.top(+is_sin: Bool, +x: F32, +at: U32) -> F32:
  f_sc.top.t(U32.is_le(at, 1011), is_sin, x, at)

def f_sin(+x: F32) -> F32:
  f_sc.top(True{}, x, U32.and((F32.bits(x) >> 20n : U32), 2047))

def f_cos(+x: F32) -> F32:
  f_sc.top(False{}, x, U32.and((F32.bits(x) >> 20n : U32), 2047))

# ==== expf, logf, powf ====
# glibc 2.44's __expf_fma, __logf_fma and __powf_fma (the ARM optimized
# routines), transcribed from the disassembly. Their tables are the libm's
# own: __exp2f_data, __logf_data, __powf_log2_data.

# exp2f_data.tab: 2^(i/32) as the bit patterns the exponent is added to.
def f_t.exp2(+i: U32) -> W.W:
  W.of_parts(Bool.pick(U32, U32.is_eq(i, 0), 1072693248, Bool.pick(U32, U32.is_eq(i, 1), 1072683440, Bool.pick(U32, U32.is_eq(i, 2), 1072674136, Bool.pick(U32, U32.is_eq(i, 3), 1072665345, Bool.pick(U32, U32.is_eq(i, 4), 1072657080, Bool.pick(U32, U32.is_eq(i, 5), 1072649351, Bool.pick(U32, U32.is_eq(i, 6), 1072642170, Bool.pick(U32, U32.is_eq(i, 7), 1072635549, Bool.pick(U32, U32.is_eq(i, 8), 1072629502, Bool.pick(U32, U32.is_eq(i, 9), 1072624039, Bool.pick(U32, U32.is_eq(i, 10), 1072619174, Bool.pick(U32, U32.is_eq(i, 11), 1072614920, Bool.pick(U32, U32.is_eq(i, 12), 1072611290, Bool.pick(U32, U32.is_eq(i, 13), 1072608299, Bool.pick(U32, U32.is_eq(i, 14), 1072605959, Bool.pick(U32, U32.is_eq(i, 15), 1072604286, Bool.pick(U32, U32.is_eq(i, 16), 1072603294, Bool.pick(U32, U32.is_eq(i, 17), 1072602997, Bool.pick(U32, U32.is_eq(i, 18), 1072603412, Bool.pick(U32, U32.is_eq(i, 19), 1072604553, Bool.pick(U32, U32.is_eq(i, 20), 1072606437, Bool.pick(U32, U32.is_eq(i, 21), 1072609079, Bool.pick(U32, U32.is_eq(i, 22), 1072612497, Bool.pick(U32, U32.is_eq(i, 23), 1072616707, Bool.pick(U32, U32.is_eq(i, 24), 1072621727, Bool.pick(U32, U32.is_eq(i, 25), 1072627574, Bool.pick(U32, U32.is_eq(i, 26), 1072634267, Bool.pick(U32, U32.is_eq(i, 27), 1072641824, Bool.pick(U32, U32.is_eq(i, 28), 1072650264, Bool.pick(U32, U32.is_eq(i, 29), 1072659607, Bool.pick(U32, U32.is_eq(i, 30), 1072669871, 1072681078))))))))))))))))))))))))))))))), Bool.pick(U32, U32.is_eq(i, 0), 0, Bool.pick(U32, U32.is_eq(i, 1), 3541402996, Bool.pick(U32, U32.is_eq(i, 2), 1828292879, Bool.pick(U32, U32.is_eq(i, 3), 3490863953, Bool.pick(U32, U32.is_eq(i, 4), 1014845819, Bool.pick(U32, U32.is_eq(i, 5), 828946858, Bool.pick(U32, U32.is_eq(i, 6), 1853186616, Bool.pick(U32, U32.is_eq(i, 7), 4112506593, Bool.pick(U32, U32.is_eq(i, 8), 171030293, Bool.pick(U32, U32.is_eq(i, 9), 926591435, Bool.pick(U32, U32.is_eq(i, 10), 1276261410, Bool.pick(U32, U32.is_eq(i, 11), 1617004845, Bool.pick(U32, U32.is_eq(i, 12), 3577096743, Bool.pick(U32, U32.is_eq(i, 13), 1453150082, Bool.pick(U32, U32.is_eq(i, 14), 3712504873, Bool.pick(U32, U32.is_eq(i, 15), 2956612997, Bool.pick(U32, U32.is_eq(i, 16), 1719614413, Bool.pick(U32, U32.is_eq(i, 17), 3907805044, Bool.pick(U32, U32.is_eq(i, 18), 1944781191, Bool.pick(U32, U32.is_eq(i, 19), 2571947539, Bool.pick(U32, U32.is_eq(i, 20), 1110089947, Bool.pick(U32, U32.is_eq(i, 21), 2966275557, Bool.pick(U32, U32.is_eq(i, 22), 2191782032, Bool.pick(U32, U32.is_eq(i, 23), 2990417245, Bool.pick(U32, U32.is_eq(i, 24), 2572866477, Bool.pick(U32, U32.is_eq(i, 25), 4076559943, Bool.pick(U32, U32.is_eq(i, 26), 3716502172, Bool.pick(U32, U32.is_eq(i, 27), 3706687593, Bool.pick(U32, U32.is_eq(i, 28), 3707479175, Bool.pick(U32, U32.is_eq(i, 29), 863738719, Bool.pick(U32, U32.is_eq(i, 30), 2728693978, 1533953344))))))))))))))))))))))))))))))))

# __logf_data.tab: invc and logc for the 16 subintervals.
def f_t.invc(+i: U32) -> D:
  d_k(Bool.pick(U32, U32.is_eq(i, 0), 1073111532, Bool.pick(U32, U32.is_eq(i, 1), 1073050093, Bool.pick(U32, U32.is_eq(i, 2), 1072993593, Bool.pick(U32, U32.is_eq(i, 3), 1072941461, Bool.pick(U32, U32.is_eq(i, 4), 1072893209, Bool.pick(U32, U32.is_eq(i, 5), 1072848418, Bool.pick(U32, U32.is_eq(i, 6), 1072806730, Bool.pick(U32, U32.is_eq(i, 7), 1072767832, Bool.pick(U32, U32.is_eq(i, 8), 1072731455, Bool.pick(U32, U32.is_eq(i, 9), 1072693248, Bool.pick(U32, U32.is_eq(i, 10), 1072586892, Bool.pick(U32, U32.is_eq(i, 11), 1072473267, Bool.pick(U32, U32.is_eq(i, 12), 1072373814, Bool.pick(U32, U32.is_eq(i, 13), 1072284369, Bool.pick(U32, U32.is_eq(i, 14), 1072203494, 1072130012))))))))))))))), Bool.pick(U32, U32.is_eq(i, 0), 2046358462, Bool.pick(U32, U32.is_eq(i, 1), 1253017661, Bool.pick(U32, U32.is_eq(i, 2), 4042264752, Bool.pick(U32, U32.is_eq(i, 3), 2964849541, Bool.pick(U32, U32.is_eq(i, 4), 210265253, Bool.pick(U32, U32.is_eq(i, 5), 2064354976, Bool.pick(U32, U32.is_eq(i, 6), 1243231295, Bool.pick(U32, U32.is_eq(i, 7), 4035634618, Bool.pick(U32, U32.is_eq(i, 8), 1100546215, Bool.pick(U32, U32.is_eq(i, 9), 0, Bool.pick(U32, U32.is_eq(i, 10), 4254746540, Bool.pick(U32, U32.is_eq(i, 11), 520252064, Bool.pick(U32, U32.is_eq(i, 12), 1466629350, Bool.pick(U32, U32.is_eq(i, 13), 1671539245, Bool.pick(U32, U32.is_eq(i, 14), 58212845, 4115875938))))))))))))))))

def f_t.logc(+i: U32) -> D:
  d_k(Bool.pick(U32, U32.is_eq(i, 0), 3218439159, Bool.pick(U32, U32.is_eq(i, 1), 3218259696, Bool.pick(U32, U32.is_eq(i, 2), 3218087598, Bool.pick(U32, U32.is_eq(i, 3), 3217764824, Bool.pick(U32, U32.is_eq(i, 4), 3217446735, Bool.pick(U32, U32.is_eq(i, 5), 3217140267, Bool.pick(U32, U32.is_eq(i, 6), 3216658038, Bool.pick(U32, U32.is_eq(i, 7), 3216086844, Bool.pick(U32, U32.is_eq(i, 8), 3215086324, Bool.pick(U32, U32.is_eq(i, 9), 0, Bool.pick(U32, U32.is_eq(i, 10), 1068148138, Bool.pick(U32, U32.is_eq(i, 11), 1069309523, Bool.pick(U32, U32.is_eq(i, 12), 1069885157, Bool.pick(U32, U32.is_eq(i, 13), 1070318214, Bool.pick(U32, U32.is_eq(i, 14), 1070663051, 1070859312))))))))))))))), Bool.pick(U32, U32.is_eq(i, 0), 2156702430, Bool.pick(U32, U32.is_eq(i, 1), 2814406107, Bool.pick(U32, U32.is_eq(i, 2), 2136029799, Bool.pick(U32, U32.is_eq(i, 3), 2793546985, Bool.pick(U32, U32.is_eq(i, 4), 180385624, Bool.pick(U32, U32.is_eq(i, 5), 3348922624, Bool.pick(U32, U32.is_eq(i, 6), 3465285214, Bool.pick(U32, U32.is_eq(i, 7), 1516313804, Bool.pick(U32, U32.is_eq(i, 8), 954272798, Bool.pick(U32, U32.is_eq(i, 9), 0, Bool.pick(U32, U32.is_eq(i, 10), 1576163716, Bool.pick(U32, U32.is_eq(i, 11), 2855677620, Bool.pick(U32, U32.is_eq(i, 12), 1998641928, Bool.pick(U32, U32.is_eq(i, 13), 220350320, Bool.pick(U32, U32.is_eq(i, 14), 3365961441, 1471606281))))))))))))))))

# __powf_log2_data.tab: logc (invc is the logf one).
def f_t.plogc(+i: U32) -> D:
  d_k(Bool.pick(U32, U32.is_eq(i, 0), 3219062470, Bool.pick(U32, U32.is_eq(i, 1), 3218803560, Bool.pick(U32, U32.is_eq(i, 2), 3218555275, Bool.pick(U32, U32.is_eq(i, 3), 3218316777, Bool.pick(U32, U32.is_eq(i, 4), 3218087323, Bool.pick(U32, U32.is_eq(i, 5), 3217652765, Bool.pick(U32, U32.is_eq(i, 6), 3217226211, Bool.pick(U32, U32.is_eq(i, 7), 3216597195, Bool.pick(U32, U32.is_eq(i, 8), 3215617949, Bool.pick(U32, U32.is_eq(i, 9), 0, Bool.pick(U32, U32.is_eq(i, 10), 1068710090, Bool.pick(U32, U32.is_eq(i, 11), 1069840041, Bool.pick(U32, U32.is_eq(i, 12), 1070498827, Bool.pick(U32, U32.is_eq(i, 13), 1070859845, Bool.pick(U32, U32.is_eq(i, 14), 1071156892, 1071440036))))))))))))))), Bool.pick(U32, U32.is_eq(i, 0), 1536569369, Bool.pick(U32, U32.is_eq(i, 1), 852819108, Bool.pick(U32, U32.is_eq(i, 2), 169850747, Bool.pick(U32, U32.is_eq(i, 3), 443404155, Bool.pick(U32, U32.is_eq(i, 4), 4092769841, Bool.pick(U32, U32.is_eq(i, 5), 456882928, Bool.pick(U32, U32.is_eq(i, 6), 2477735071, Bool.pick(U32, U32.is_eq(i, 7), 3212348764, Bool.pick(U32, U32.is_eq(i, 8), 3058130894, Bool.pick(U32, U32.is_eq(i, 9), 0, Bool.pick(U32, U32.is_eq(i, 10), 2669999421, Bool.pick(U32, U32.is_eq(i, 11), 1412993466, Bool.pick(U32, U32.is_eq(i, 12), 1254417618, Bool.pick(U32, U32.is_eq(i, 13), 4039533852, Bool.pick(U32, U32.is_eq(i, 14), 740007928, 1320254412))))))))))))))))

# asdouble((T[ki % 32] + (ski << 47))) for ki, ski as the 64-bit patterns.
def f_t.scale(+ki: W.W, +ski: W.W) -> D:
  d_of_w(W.add(f_t.exp2(U32.and(W.lo32(ki), 31)), W.shl(ski, 47)))

def f_exp.invln2n() -> D:
  d_k(1078400327, 1697350398)

def f_exp.shift() -> D:
  d_k(1127743488, 0)

def f_exp.ps0() -> D:
  d_k(1052535544, 1267803028)

def f_exp.ps1() -> D:
  d_k(1060028366, 1358611699)

def f_exp.ps2() -> D:
  d_k(1066806850, 4278997718)

def f_exp.tail(+kd0: D, +r: D) -> F32:
  d_to_f32(d_mul(
    d_fma(d_fma(r, f_exp.ps0(), f_exp.ps1()), d_mul(r, r), d_fma(r, f_exp.ps2(), d_one())),
    f_t.scale(d_to_w(kd0), d_to_w(kd0))))

def f_exp.main2(+xd: D, +kd0: D) -> F32:
  f_exp.tail(kd0, d_fma(f_exp.invln2n(), xd, d_neg(d_sub(kd0, f_exp.shift()))))

def f_exp.main(+x: F32) -> F32:
  f_exp.main2(d_of_f32(x), d_fma(f_exp.invln2n(), d_of_f32(x), f_exp.shift()))

# expf
# ----

# The past-the-end cases of exp: |x| >= 88 or NaN, with their early results.
def f_exp.big(+x: F32, +ix: U32) -> F32:
  Bool.pick(F32, U32.is_eq(ix, 4286578688), 0.0,
    Bool.pick(F32, U32.is_ge(U32.and((ix >> 20n : U32), 2047), 2040), F32.add(x, x),
      Bool.pick(F32, F32.is_gt(x, f_from_bits(1118925335)), f_from_bits(2139095040),
        Bool.pick(F32, F32.is_lt(x, f_from_bits(3268407732)), 0.0,
          Bool.pick(F32, F32.is_lt(x, f_from_bits(3268316879)), f_from_bits(1), f_exp.main(x))))))

def f_exp.top(+big: Bool, +x: F32, +ix: U32) -> F32:
  match big:
    case True{}:
      f_exp.big(x, ix)
    case False{}:
      f_exp.main(x)

def f_exp(+x: F32) -> F32:
  f_exp.top(U32.is_ge(U32.and((F32.bits(x) >> 20n : U32), 2047), 1067), x, F32.bits(x))

# logf
# ----

def f_ln.a0() -> D:
  d_k(3218083491, 1220051764)

def f_ln.a1() -> D:
  d_k(1070946139, 199232362)

def f_ln.a2() -> D:
  d_k(3219128318, 4060758307)

def f_ln.ln2() -> D:
  d_k(1072049730, 4277811695)

def f_ln.fin(+r: D, +y0: D) -> F32:
  d_to_f32(d_fma(d_mul(r, r), d_fma(d_mul(r, r), f_ln.a0(), d_fma(r, f_ln.a1(), f_ln.a2())), d_add(y0, r)))

def f_ln.r(+z: D, +i: U32, +k: U32) -> F32:
  f_ln.fin(d_fma(z, f_t.invc(i), d_neg(d_one())), d_fma(d_of_i32(k), f_ln.ln2(), f_t.logc(i)))

# x = 2^k z with z in [OFF, 2 OFF): the table index and the exponent.
def f_ln.main(+ix: U32) -> F32:
  f_ln.r(d_of_f32(f_from_bits((ix - U32.and((ix - 1060306944 : U32), 4286578688) : U32))),
    U32.and(((ix - 1060306944 : U32) >> 19n : U32), 15), i_shr((ix - 1060306944 : U32), 23))

def f_ln.sub(+x: F32) -> F32:
  f_ln.main(((F32.bits(F32.mul(x, 8388608.0))) - 192937984 : U32))

def f_ln.special2(+ix: U32, +x: F32) -> F32:
  Bool.pick(F32, U32.is_eq(ix, 2139095040), x,
    Bool.pick(F32, Bool.or(U32.is_ge(ix, 2147483648), U32.is_ge((ix * 2 : U32), 4278190080)), f_from_bits(4290772992), f_ln.sub(x)))

def f_ln.special(+ix: U32, +x: F32) -> F32:
  Bool.pick(F32, U32.is_zero((ix * 2 : U32)), f_from_bits(4286578688), f_ln.special2(ix, x))

def f_ln.top2(+small: Bool, +ix: U32, +x: F32) -> F32:
  match small:
    case True{}:
      f_ln.special(ix, x)
    case False{}:
      f_ln.main(ix)

def f_ln(+x: F32) -> F32:
  Bool.pick(F32, U32.is_eq(F32.bits(x), 1065353216), 0.0, f_ln.top2(U32.is_ge((F32.bits(x) - 8388608 : U32), 2130706432), F32.bits(x), x))

# powf
# ----

def f_pw.a0() -> D:
  d_k(1070757398, 3377032715)

def f_pw.a1() -> D:
  d_k(3218545001, 2692073082)

def f_pw.a2() -> D:
  d_k(1071564554, 1822931677)

def f_pw.a3() -> D:
  d_k(3219592519, 1220474568)

def f_pw.a4() -> D:
  d_k(1073157447, 1697298475)

def f_pw.shift() -> D:
  d_k(1122500608, 0)

def f_pw.c0() -> D:
  d_k(1068264184, 1267803028)

def f_pw.c1() -> D:
  d_k(1070514126, 1358611699)

def f_pw.c2() -> D:
  d_k(1072049730, 4278997718)

# exp2_inline: 2^xd with the sign of the result in sign_bias (0 or 0x10000).
def f_pw.exp2.fin(+kd0: D, +r: D, +sign_bias: U32) -> F32:
  d_to_f32(d_mul(
    d_fma(d_fma(r, f_pw.c0(), f_pw.c1()), d_mul(r, r), d_fma(r, f_pw.c2(), d_one())),
    f_t.scale(d_to_w(kd0), W.add(d_to_w(kd0), W.of_u32(sign_bias)))))

def f_pw.exp2.go(+xd: D, +kd0: D, +sign_bias: U32) -> F32:
  f_pw.exp2.fin(kd0, d_sub(xd, d_sub(kd0, f_pw.shift())), sign_bias)

def f_pw.exp2(+xd: D, +sign_bias: U32) -> F32:
  f_pw.exp2.go(xd, d_add(xd, f_pw.shift()), sign_bias)

def f_pw.signed(+sign_bias: U32, +bits: U32) -> F32:
  f_from_bits(U32.or(bits, Bool.pick(U32, U32.is_ne(sign_bias, 0), 2147483648, 0)))

# |ylogx| >= 126: results that are too small, too big or near the float limits.
def f_pw.edge3(+neg: Bool, +mag: W.W, +xd: D, +sign_bias: U32) -> F32:
  Bool.pick(F32, Bool.and(Bool.not(neg), W.gt(mag, W.of_parts(1080033279, 4291941745))), f_pw.signed(sign_bias, 2139095040),
    Bool.pick(F32, Bool.and(Bool.not(neg), W.gt(mag, W.of_parts(1080033279, 4288916194))), f_pw.signed(sign_bias, 2139095039),
      f_pw.exp2(xd, sign_bias)))

def f_pw.edge2(+neg: Bool, +mag: W.W, +xd: D, +sign_bias: U32) -> F32:
  Bool.pick(F32, Bool.and(neg, W.ge(mag, W.of_parts(1080213504, 0))), f_pw.signed(sign_bias, 0),
    Bool.pick(F32, Bool.and(neg, W.gt(mag, W.of_parts(1080205312, 0))), f_pw.signed(sign_bias, 1),
      f_pw.edge3(neg, mag, xd, sign_bias)))

def f_pw.edge(+xd: D, +sign_bias: U32) -> F32:
  f_pw.edge2(d_is_neg(xd), W.and(d_to_w(xd), W.of_parts(2147483647, 4294967295)), xd, sign_bias)

def f_pw.range(+big: Bool, +xd: D, +sign_bias: U32) -> F32:
  match big:
    case True{}:
      f_pw.edge(xd, sign_bias)
    case False{}:
      f_pw.exp2(xd, sign_bias)

def f_pw.log2.fin2(+ylogx: D, +sign_bias: U32) -> F32:
  f_pw.range(U32.is_ge(U32.and(d_hi(ylogx), 2147483647), 1080000512), ylogx, sign_bias)

def f_pw.log2.fin(+r: D, +y0: D, +yd: D, +sign_bias: U32) -> F32:
  f_pw.log2.fin2(d_mul(d_fma(d_fma(r, f_pw.a0(), f_pw.a1()), d_mul(d_mul(r, r), d_mul(r, r)),
    d_fma(d_mul(r, r), d_fma(r, f_pw.a2(), f_pw.a3()), d_fma(r, f_pw.a4(), y0))), yd), sign_bias)

def f_pw.log2.r(+z: D, +i: U32, +k: U32, +yd: D, +sign_bias: U32) -> F32:
  f_pw.log2.fin(d_fma(z, f_t.invc(i), d_neg(d_one())), d_add(f_t.plogc(i), d_of_i32(k)), yd, sign_bias)

def f_pw.main(+ix: U32, +y: F32, +sign_bias: U32) -> F32:
  f_pw.log2.r(d_of_f32(f_from_bits((ix - U32.and((ix - 1060306944 : U32), 4286578688) : U32))),
    U32.and(((ix - 1060306944 : U32) >> 19n : U32), 15), i_shr(U32.and((ix - 1060306944 : U32), 4286578688), 23), d_of_f32(y), sign_bias)

# checkint: 0 not an integer, 1 odd, 2 even (iy the bits of a nonzero finite float).
def f_pw.checkint.m(+iy: U32, +sh: U32) -> U32:
  Bool.pick(U32, U32.is_ne(U32.and(iy, (U32.shln(1, U32.to_nat(sh)) - 1 : U32)), 0), 0,
    Bool.pick(U32, U32.is_ne(U32.and(iy, U32.shln(1, U32.to_nat(sh))), 0), 1, 2))

def f_pw.checkint(+iy: U32) -> U32:
  Bool.pick(U32, U32.is_lt(U32.and((iy >> 23n : U32), 255), 127), 0,
    Bool.pick(U32, U32.is_gt(U32.and((iy >> 23n : U32), 255), 150), 2,
      f_pw.checkint.m(iy, (150 - U32.and((iy >> 23n : U32), 255) : U32))))

def f_pw.zeroinfnan(+i: U32) -> Bool:
  U32.is_ge(((i * 2 : U32) - 1 : U32), 4278190079)

def f_pw.issnan(+x: F32) -> Bool:
  U32.is_gt((U32.xor(F32.bits(x), 4194304) * 2 : U32), 4286578688)

# y is zero, infinite or NaN.
def f_pw.yspecial2(+x: F32, +y: F32, +ix: U32, +iy: U32) -> F32:
  Bool.pick(F32, Bool.or(U32.is_gt((ix * 2 : U32), 4278190080), U32.is_gt((iy * 2 : U32), 4278190080)), F32.add(x, y),
    Bool.pick(F32, U32.is_eq((ix * 2 : U32), 2130706432), 1.0,
      Bool.pick(F32, Bool.xor(U32.is_lt((ix * 2 : U32), 2130706432), U32.is_ge(iy, 2147483648)), 0.0, F32.mul(y, y))))

def f_pw.yspecial(+x: F32, +y: F32, +ix: U32, +iy: U32) -> F32:
  Bool.pick(F32, U32.is_zero((iy * 2 : U32)), Bool.pick(F32, f_pw.issnan(x), F32.add(x, y), 1.0),
    Bool.pick(F32, U32.is_eq(ix, 1065353216), Bool.pick(F32, f_pw.issnan(y), F32.add(x, y), 1.0),
      f_pw.yspecial2(x, y, ix, iy)))

def f_pw.xspecial3(+x2: F32, +iy: U32) -> F32:
  Bool.pick(F32, U32.is_ge(iy, 2147483648), F32.div(1.0, x2), x2)

def f_pw.xspecial2(+x2: F32, +odd: Bool, +iy: U32) -> F32:
  f_pw.xspecial3(Bool.pick(F32, odd, F32.neg(x2), x2), iy)

# x is zero, infinite or NaN (y is finite and nonzero).
def f_pw.xspecial(+x: F32, +ix: U32, +iy: U32) -> F32:
  f_pw.xspecial2(F32.mul(x, x), Bool.and(U32.is_ge(ix, 2147483648), U32.is_eq(f_pw.checkint(iy), 1)), iy)

# Normalize a subnormal x so the exponent goes negative.
def f_pw.sub(+ix: U32, +x: F32, +y: F32, +sign_bias: U32) -> F32:
  Bool.pick(F32, U32.is_lt(ix, 8388608),
    f_pw.main((U32.and(F32.bits(F32.mul(x, 8388608.0)), 2147483647) - 192937984 : U32), y, sign_bias),
    f_pw.main(ix, y, sign_bias))

# Negative finite x: only an integer y, and odd flips the sign.
def f_pw.neg.go(+yint: U32, +x: F32, +y: F32) -> F32:
  Bool.pick(F32, U32.is_zero(yint), f_from_bits(4290772992),
    f_pw.sub(U32.and(F32.bits(x), 2147483647), x, y, Bool.pick(U32, U32.is_eq(yint, 1), 65536, 0)))

def f_pw.finite(+x: F32, +y: F32, +ix: U32, +iy: U32) -> F32:
  Bool.pick(F32, U32.is_ge(ix, 2147483648), f_pw.neg.go(f_pw.checkint(iy), x, y), f_pw.sub(ix, x, y, 0))

def f_pw.xcheck(+x: F32, +y: F32, +ix: U32, +iy: U32) -> F32:
  Bool.pick(F32, f_pw.zeroinfnan(ix), f_pw.xspecial(x, ix, iy), f_pw.finite(x, y, ix, iy))

def f_pw.slow(+x: F32, +y: F32, +ix: U32, +iy: U32) -> F32:
  Bool.pick(F32, f_pw.zeroinfnan(iy), f_pw.yspecial(x, y, ix, iy), f_pw.xcheck(x, y, ix, iy))

def f_pw.top(+slow: Bool, +x: F32, +y: F32, +ix: U32, +iy: U32) -> F32:
  match slow:
    case True{}:
      f_pw.slow(x, y, ix, iy)
    case False{}:
      f_pw.main(ix, y, 0)

def f_powf(+x: F32, +y: F32) -> F32:
  f_pw.top(Bool.or(U32.is_ge((F32.bits(x) - 8388608 : U32), 2130706432), f_pw.zeroinfnan(F32.bits(y))), x, y, F32.bits(x), F32.bits(y))

# atan2f
# ------

# glibc 2.43's atan2f is correctly rounded; Base's double atan2 rounded to f32
# agrees except where the result is subnormal and the quotient y/x lands exactly
# between two floats: the true value is just under y/x (atan t < t), so the
# round-to-even float is one too high when it is the upper neighbour.
def d_eq(+a: D, +b: D) -> Bool:
  match a b:
    case D{+an, +am, +ae} D{+bn, +bm, +be}:
      Bool.and(W.eq(am, bm), Bool.or(W.is_zero(am), U32.is_eq(ae, be)))

def f_atan2.tie(+ay: D, +ax: D, +b: U32) -> Bool:
  d_eq(ay, d_mul(d_mul(d_of_i32((b * 2 - 1 : U32)), D{False{}, W.shl(W.one(), 52), (0 - 202 : U32)}), ax))

def f_atan2.fix(+ay: D, +ax: D, +r: F32) -> F32:
  Bool.pick(F32, f_atan2.tie(ay, ax, U32.and(F32.bits(r), 2147483647)),
    f_from_bits((F32.bits(r) - 1 : U32)), r)

def d_pos(+d: D) -> D:
  match d:
    case D{+n, +m, +e}:
      D{False{}, m, e}

def f_atan2.small(+small: Bool, +y: F32, +x: F32, +r: F32) -> F32:
  match small:
    case True{}:
      f_atan2.fix(d_pos(d_of_f32(y)), d_pos(d_of_f32(x)), r)
    case False{}:
      r

def f_atan2.top(+r: F32, +y: F32, +x: F32) -> F32:
  f_atan2.small(U32.is_lt((U32.and(F32.bits(r), 2147483647) - 1 : U32), 8388608), y, x, r)

def f_atan2(+y: F32, +x: F32) -> F32:
  f_atan2.top(F32.atan2(y, x), y, x)

# More integer helpers
# --------------------

def u_div_ceil(+a: U32, +b: U32) -> U32:
  (U32.div(a, b) + Bool.to_u32(U32.is_ne((a - U32.div(a, b) * b : U32), 0)) : U32)

# `a.is_multiple_of(b)`: zero is a multiple of zero only.
def u_is_multiple_of(+a: U32, +b: U32) -> Bool:
  Bool.pick(Bool, U32.is_zero(b), U32.is_zero(a), U32.is_zero(U32.mod(a, b)))

# `a.saturating_add_signed(b)` for a u32 and an i32 pattern.
def u_sat_add_signed(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, i_is_neg(b), u_sat_sub(a, i_unsigned_abs(b)), u_sat_add(a, b))

def u_checked.go(overflow: Bool, +v: U32) -> Maybe<&2, U32>:
  match overflow:
    case True{}:
      None{}
    case False{}:
      Some{v}

def u_checked_add(+a: U32, +b: U32) -> Maybe<&2, U32>:
  u_checked.go(U32.is_lt((a + b : U32), a), (a + b : U32))

def u_checked_sub(+a: U32, +b: U32) -> Maybe<&2, U32>:
  u_checked.go(U32.is_lt(a, b), (a - b : U32))

def u_checked_mul(+a: U32, +b: U32) -> Maybe<&2, U32>:
  u_checked.go(Bool.and(U32.is_ne(a, 0), U32.is_gt(b, U32.div(4294967295, a))), (a * b : U32))

# f32::total_cmp: the order of the keys as unsigned words is IEEE's total order.
def f_total_key(+x: F32) -> U32:
  U32.xor(U32.xor(F32.bits(x), Bool.pick(U32, U32.is_ge(F32.bits(x), 2147483648), 2147483647, 0)), 2147483648)

def f_total_cmp(+a: F32, +b: F32) -> Cmp:
  U32.cmp(f_total_key(a), f_total_key(b))

def f_total_lt(+a: F32, +b: F32) -> Bool:
  U32.is_lt(f_total_key(a), f_total_key(b))

def f_total_le(+a: F32, +b: F32) -> Bool:
  U32.is_le(f_total_key(a), f_total_key(b))

# Saturating casts to the narrow integer types (NaN is 0).
def f_as_u8(+x: F32) -> U32:
  u_min(f_as_u32(x), 255)

def f_as_u16(+x: F32) -> U32:
  u_min(f_as_u32(x), 65535)

def f_as_i8(+x: F32) -> U32:
  i_clamp(f_as_i32(x), 4294967168, 127)

def f_as_i16(+x: F32) -> U32:
  i_clamp(f_as_i32(x), 4294934528, 32767)
