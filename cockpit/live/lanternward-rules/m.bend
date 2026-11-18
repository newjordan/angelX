import Base
import ./n.bend as N

def sat(+a: U32, +b: U32) -> U32: Bool.pick(U32, U32.is_ge(a, b), U32.sub(a, b), 0)

def dec(+a: U32) -> U32: Bool.pick(U32, U32.is_gt(a, 0), U32.sub(a, 1), 0)

def sign(+a: U32) -> U32: Bool.pick(U32, N.sgt(a, 0), 1, Bool.pick(U32, N.slt(a, 0), 4294967295, 0))
