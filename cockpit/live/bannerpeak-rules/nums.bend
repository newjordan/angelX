import Base

# Bannerpeak's rules are integer physics: a position is the feet's centre in
# 1/256 pixel, a speed is 1/256 pixel a tick, and both are signed. Bend's only
# integer is U32, so a signed value travels as its 32-bit two's complement
# pattern and every signed operation the game needs is written once, here, and
# pinned by the laws. Division and remainder follow Rust's `i32` operators:
# `div` truncates toward zero, `rem` takes the dividend's sign, and the euclid
# pair by a positive divisor is what a tile column or a degree is read through.

def neg(+a: U32) -> U32:
  (0 - a : U32)

def add(+a: U32, +b: U32) -> U32:
  (a + b : U32)

def sub(+a: U32, +b: U32) -> U32:
  (a - b : U32)

def mul(+a: U32, +b: U32) -> U32:
  (a * b : U32)

# Whether the sign bit is set: the whole of signed comparison starts here.
def negative(+a: U32) -> Bool:
  U32.is_ge(a, 2147483648)

def abs(+a: U32) -> U32:
  Bool.pick(U32, negative(a), neg(a), a)

def sign_of(+down: Bool, +zero: Bool) -> U32:
  match down:
    case True{}:
      neg(1)
    case False{}:
      Bool.pick(U32, zero, 0, 1)

# -1, 0 or 1, as `i8::signum` would say.
def sign(+a: U32) -> U32:
  sign_of(negative(a), U32.is_zero(a))

# Two's complement order is unsigned order once the signs agree, and a negative
# is under every non-negative.
def lt_signs(+a_neg: Bool, +b_neg: Bool, +a: U32, +b: U32) -> Bool:
  match a_neg b_neg:
    case True{} True{}:
      U32.is_lt(a, b)
    case True{} False{}:
      True{}
    case False{} True{}:
      False{}
    case False{} False{}:
      U32.is_lt(a, b)

def lt(+a: U32, +b: U32) -> Bool:
  lt_signs(negative(a), negative(b), a, b)

def gt(+a: U32, +b: U32) -> Bool:
  lt(b, a)

def le(+a: U32, +b: U32) -> Bool:
  Bool.not(gt(a, b))

def ge(+a: U32, +b: U32) -> Bool:
  Bool.not(lt(a, b))

def min(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, lt(a, b), a, b)

def max(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, lt(a, b), b, a)

def clamp(+x: U32, +lo: U32, +hi: U32) -> U32:
  min(max(x, lo), hi)

# Truncating division: the quotient takes the sign of the product of the signs.
def div_sign(+unlike: Bool, +a: U32, +b: U32) -> U32:
  match unlike:
    case True{}:
      neg(U32.div(abs(a), abs(b)))
    case False{}:
      U32.div(abs(a), abs(b))

def div(+a: U32, +b: U32) -> U32:
  div_sign(Bool.xor(negative(a), negative(b)), a, b)

# Truncating remainder: the sign of the dividend.
def rem_sign(+down: Bool, +a: U32, +b: U32) -> U32:
  match down:
    case True{}:
      neg(U32.mod(abs(a), abs(b)))
    case False{}:
      U32.mod(abs(a), abs(b))

def rem(+a: U32, +b: U32) -> U32:
  rem_sign(negative(a), a, b)

# Euclidean division by a positive divisor: the remainder is never negative.
def div_euclid(+a: U32, +b: U32) -> U32:
  +q = div(a, b)
  +r = rem(a, b)
  Bool.pick(U32, negative(r), sub(q, 1), q)

def rem_euclid(+a: U32, +b: U32) -> U32:
  +r = rem(a, b)
  Bool.pick(U32, negative(r), add(r, abs(b)), r)

# `u32::saturating_sub`, which the game's tick counters are kept in.
def sat_sub(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_lt(a, b), 0, sub(a, b))

# `u32::count_ones`, for the eight banners and the eight ring sparks.
def bits(n: Nat, +x: U32, +acc: U32) -> U32:
  match n:
    case 0n:
      acc
    case 1n+p:
      bits(p, U32.shrn(x, 1n), (acc + U32.and(x, 1) : U32))

def popcount(+x: U32) -> U32:
  bits(32n, x, 0)
