# The three small pieces of whole-number math the rules lean on that Base does
# not spell. A word is a signed i32 read as its two's-complement bits, so +, - and
# * are the words' own, and order, division and absolute value are numeric.bend's
# signed spellings.
import Base
import ./n.bend as N

# a - b, held at zero.
def sat(+a: U32, +b: U32) -> U32: Bool.pick(U32, U32.is_ge(a, b), U32.sub(a, b), 0)

# a - 1, held at zero.
def dec(+a: U32) -> U32: Bool.pick(U32, U32.is_gt(a, 0), U32.sub(a, 1), 0)

# -1, 0 or 1 by the sign of a signed word.
def sign(+a: U32) -> U32: Bool.pick(U32, N.sgt(a, 0), 1, Bool.pick(U32, N.slt(a, 0), 4294967295, 0))
