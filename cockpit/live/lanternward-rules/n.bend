# Lanternward's numbers. The adventure is whole numbers on the 30 Hz step: a
# position is pixels times Q, an angle is a whole degree. Bend's U32 is the
# two's-complement word, so a signed i32 is that word read with the sign bit
# biased; the one signed i64 (a squared length) is a hi/lo pair.
#
# Bend only matches on parameters, so a choice between two values is a mask or
# `Bool.pick`, never a match on a computed flag.
import Base

# a < b, signed.
def slt(+a: U32, +b: U32) -> Bool:
  U32.is_lt(U32.xor(a, 2147483648), U32.xor(b, 2147483648))

def sle(+a: U32, +b: U32) -> Bool:
  U32.is_le(U32.xor(a, 2147483648), U32.xor(b, 2147483648))

def sgt(+a: U32, +b: U32) -> Bool:
  U32.is_gt(U32.xor(a, 2147483648), U32.xor(b, 2147483648))

def sge(+a: U32, +b: U32) -> Bool:
  U32.is_ge(U32.xor(a, 2147483648), U32.xor(b, 2147483648))

def smin(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, slt(a, b), a, b)

def smax(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, sgt(a, b), a, b)

def sclamp(+x: U32, +lo: U32, +hi: U32) -> U32:
  smin(smax(x, lo), hi)

# -a, wrapping at the smallest word.
def sneg(+a: U32) -> U32:
  U32.sub(0, a)

def sabs(+a: U32) -> U32:
  Bool.pick(U32, slt(a, 0), sneg(a), a)

# Every bit, when a's sign bit is set: the mask a condition applies.
def mask(+a: U32) -> U32: U32.sub(0, U32.shrn(a, 31n))

# a >> 12, arithmetic: the sign bit fills in the top twelve.
def sar12(+a: U32) -> U32: U32.or(U32.shrn(a, 12n), U32.mul(U32.shrn(a, 31n), 4293918720))

# v unless `m` is every bit, then -v: the branchless if.
def cond_neg(+v: U32, +m: U32) -> U32:
  U32.sub(U32.xor(v, m), m)

# `a` when `m` is every bit, else `b`.
def cond(+m: U32, +a: U32, +b: U32) -> U32:
  U32.or(U32.and(m, a), U32.and(U32.not(m), b))

# `c` as the U32 1 or 0.
def flag(c: Bool) -> U32:
  Bool.to_u32(c)

# a / b and a % b, truncated toward zero like Rust's `/` and `%`.
def sdiv(+a: U32, +b: U32) -> U32:
  cond_neg(U32.div(sabs(a), sabs(b)), mask(U32.xor(a, b)))

def srem(+a: U32, +b: U32) -> U32:
  cond_neg(U32.mod(sabs(a), sabs(b)), mask(a))

# a.rem_euclid(b) for a positive b: the remainder that is never negative.
def rem_euclid(+a: U32, +b: U32) -> U32:
  +r = srem(a, b)
  U32.sub(r, U32.and(mask(r), sneg(b)))

# ----- u64 as hi/lo, for lengths only -------------------------------------

type U64 is Data:
  U64{+hi: U32, +lo: U32}

def u64_hi(+a: U64) -> U32:
  match a:
    case U64{hi, _}:
      hi

def u64_lo(+a: U64) -> U32:
  match a:
    case U64{_, lo}:
      lo

def u64_zero() -> U64:
  U64{0, 0}

def u64_ge(+a: U64, +b: U64) -> Bool:
  match a b:
    case U64{ahi, alo} U64{bhi, blo}:
      Bool.or(U32.is_gt(ahi, bhi), Bool.and(U32.is_eq(ahi, bhi), U32.is_ge(alo, blo)))

def u64_add(+a: U64, +b: U64) -> U64:
  match a b:
    case U64{ahi, alo} U64{bhi, blo}:
      +lo = U32.add(alo, blo)
      +carry = flag(U32.is_lt(lo, alo))
      U64{U32.add(U32.add(ahi, bhi), carry), lo}

def u64_sub(+a: U64, +b: U64) -> U64:
  match a b:
    case U64{ahi, alo} U64{bhi, blo}:
      +borrow = flag(U32.is_lt(alo, blo))
      U64{U32.sub(U32.sub(ahi, bhi), borrow), U32.sub(alo, blo)}

# a & m, both limbs: the mask a u64 condition applies to its value.
def u64_andm(+a: U64, +m: U32) -> U64:
  match a:
    case U64{hi, lo}:
      U64{U32.and(hi, m), U32.and(lo, m)}

def u64_shr1(+a: U64) -> U64:
  match a:
    case U64{hi, lo}:
      U64{U32.shr(hi), U32.or(U32.shr(lo), U32.shln(U32.and(hi, 1), 31n))}

def u64_shr2(+a: U64) -> U64:
  match a:
    case U64{hi, lo}:
      U64{U32.shrn(hi, 2n), U32.or(U32.shrn(lo, 2n), U32.shln(U32.and(hi, 3), 30n))}

# a, or its two's complement when m is every bit.
def u64_cneg(+a: U64, +m: U32) -> U64:
  match a:
    case U64{hi, lo}:
      u64_add(U64{U32.xor(hi, m), U32.xor(lo, m)}, U64{0, flag(U32.is_ne(m, 0))})

# a * b for two words, as the 64-bit product.
def mul32(+a: U32, +b: U32) -> U64:
  +al = U32.and(a, 65535)
  +ah = U32.shrn(a, 16n)
  +bl = U32.and(b, 65535)
  +bh = U32.shrn(b, 16n)
  +p = U32.mul(al, bl)
  +q = U32.mul(ah, bl)
  +r = U32.mul(al, bh)
  +s = U32.mul(ah, bh)
  +mid = U32.add(U32.add(U32.shrn(p, 16n), U32.and(q, 65535)), U32.and(r, 65535))
  U64{
    U32.add(U32.add(s, U32.shrn(q, 16n)), U32.add(U32.shrn(r, 16n), U32.shrn(mid, 16n))),
    U32.or(U32.and(p, 65535), U32.shln(U32.and(mid, 65535), 16n))
  }

# The square of a signed word, as a u64.
def sq(+a: U32) -> U64:
  mul32(sabs(a), sabs(a))

# The square root of a 64-bit number, rounded down, by the restoring
# bit-by-bit method: no division, and the one choice is a mask, so the walk is
# a single tail call. Every length squared here stays under 2^48, so the walk
# starts at 2^50 with room to spare and runs exactly one step per even power
# down to 2^0: a step past zero would halve the answer again.
def isqrt_go(+fuel: Nat, +n: U64, +res: U64, +bit: U64) -> U32:
  match fuel:
    case 0n:
      match res:
        case U64{_, lo}:
          lo
    case 1n+p:
      +m = mask(sneg(flag(u64_ge(n, u64_add(res, bit)))))
      isqrt_go(
        p,
        u64_sub(n, u64_andm(u64_add(res, bit), m)),
        u64_add(u64_shr1(res), u64_andm(bit, m)),
        u64_shr2(bit)
      )

def isqrt_slow(+n: U64) -> U32:
  isqrt_go(32n, n, u64_zero(), U64{1073741824, 0})

# The square root of a 64-bit number, rounded down: an f32 estimate, which is
# within a few of the answer for every length this adventure squares, then exact
# steps either way (r*r > n is too far, (r+1)*(r+1) <= n is not far enough) until
# neither holds. The fuel only bounds the walk; the estimate never needs it all.
def u64_gt(+a: U64, +b: U64) -> Bool:
  Bool.not(u64_ge(b, a))

def isqrt_fix(+fuel: Nat, +n: U64, +r: U32, +done: Bool) -> U32:
  match fuel:
    case 0n:
      r
    case 1n+p:
      match done:
        case True{}:
          r
        case False{}:
          +big = u64_gt(mul32(r, r), n)
          +small = u64_ge(n, mul32(U32.inc(r), U32.inc(r)))
          +r2 = U32.sub(U32.add(r, flag(small)), flag(big))
          isqrt_fix(p, n, r2, U32.is_eq(r2, r))

def isqrt_est(+n: U64) -> U32:
  match n:
    case U64{hi, lo}:
      F32.to_u32(F32.sqrt(F32.add(F32.mul(U32.to_f32(hi), 4294967296.0), U32.to_f32(lo))))

def isqrt_fast(fast: Bool, +n: U64) -> U32:
  match fast:
    case True{}:
      isqrt_fix(8n, n, isqrt_est(n), False{})
    case False{}:
      isqrt_slow(n)

# The estimate holds to a few under 2^50; past that the restoring walk is exact.
def isqrt(+n: U64) -> U32:
  match n:
    case U64{hi, _}:
      isqrt_fast(U32.is_lt(hi, 262144), n)

def u64_shl1(a: U64) -> U64:
  match a:
    case U64{hi, lo}:
      U64{U32.or(U32.shl(hi), U32.shrn(lo, 31n)), U32.shl(lo)}

# a / d for a 64-bit dividend and a word divisor, by the restoring method over
# the dividend's bits. The quotient is the low word, which is every quotient
# this adventure asks for: a scaled direction's numerator over its length.
def udiv_go(fuel: Nat, rem: U32, a: U64, q: U32, +d: U32) -> U32:
  match fuel:
    case 0n:
      q
    case 1n+p:
      match a:
        case U64{ahi, alo}:
          +r2 = U32.or(U32.shl(rem), U32.shrn(ahi, 31n))
          +ge = flag(U32.is_ge(r2, d))
          m = mask(sneg(ge))
          udiv_go(
            p,
            U32.sub(r2, U32.and(m, d)),
            u64_shl1(U64{ahi, alo}),
            U32.or(U32.shl(q), ge),
            d
          )

def u64_div(+a: U64, d: U32) -> U32:
  udiv_go(64n, 0, a, 0, d)
