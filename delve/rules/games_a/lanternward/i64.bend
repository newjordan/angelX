# Signed 64-bit helpers for the few i64 places in Lanternward (invented,
# named lanternward.i64.<x>). An i64 is a W.W holding its two's complement.

def lanternward.i64.of_i32(+v: U32) -> W.W:
  W.W{Bool.pick(U32, RS.i_is_neg(v), 4294967295, 0), v}

def lanternward.i64.is_neg(+a: W.W) -> Bool:
  U32.is_ge(W.hi32(a), 2147483648)

def lanternward.i64.neg(+a: W.W) -> W.W:
  W.neg(a)

def lanternward.i64.abs(+a: W.W) -> W.W:
  Bool.pick(W.W, lanternward.i64.is_neg(a), W.neg(a), a)

def lanternward.i64.add(+a: W.W, +b: W.W) -> W.W:
  W.add(a, b)

def lanternward.i64.sub(+a: W.W, +b: W.W) -> W.W:
  W.sub(a, b)

# Two's complement multiplication is the unsigned wrapping product.
def lanternward.i64.mul(+a: W.W, +b: W.W) -> W.W:
  W.mul(a, b)

# Truncating division (the quotient's sign is the product of the signs).
def lanternward.i64.div(+a: W.W, +b: W.W) -> W.W:
  +q = W.div(lanternward.i64.abs(a), lanternward.i64.abs(b))
  Bool.pick(W.W, Bool.xor(lanternward.i64.is_neg(a), lanternward.i64.is_neg(b)), W.neg(q), q)

def lanternward.i64.lt(+a: W.W, +b: W.W) -> Bool:
  W.lt(W.xor(a, W.W{2147483648, 0}), W.xor(b, W.W{2147483648, 0}))

def lanternward.i64.le(+a: W.W, +b: W.W) -> Bool:
  W.le(W.xor(a, W.W{2147483648, 0}), W.xor(b, W.W{2147483648, 0}))

def lanternward.i64.clamp(+x: W.W, +lo: W.W, +hi: W.W) -> W.W:
  Bool.pick(W.W, lanternward.i64.lt(x, lo), lo, Bool.pick(W.W, lanternward.i64.lt(hi, x), hi, x))

# `x as i32`: the low 32 bits.
def lanternward.i64.to_i32(+a: W.W) -> U32:
  W.lo32(a)
