# Lists of records: the places the world keeps its props, foes, bolts and the
# rest. A list is read by walking it, which at these sizes (160 props at most)
# is a few hundred steps, and written by rebuilding the part before the place.
import Base
import ./r.bend as R

def zero8() -> R.W8: R.W8{0, 0, 0, 0, 0, 0, 0, 0}

def zero64() -> R.T8: R.T8{zero8(), zero8(), zero8(), zero8(), zero8(), zero8(), zero8(), zero8()}

# ----- lists of leaves ------------------------------------------------------

def len8(+xs: List<&2, R.W8>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{_, t}:
      U32.add(1, len8(t))

# The n-th record, zeros past the end.
def get8(+xs: List<&2, R.W8>, +n: Nat) -> R.W8:
  match xs:
    case Nil{}:
      zero8()
    case Con{h, t}:
      match n:
        case 0n:
          h
        case 1n+p:
          get8(t, p)

def set8(+xs: List<&2, R.W8>, +n: Nat, +v: R.W8) -> List<&2, R.W8>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      match n:
        case 0n:
          Con{v, t}
        case 1n+p:
          Con{h, set8(t, p, v)}

def push8(+xs: List<&2, R.W8>, +v: R.W8) -> List<&2, R.W8>:
  match xs:
    case Nil{}:
      Con{v, Nil{}}
    case Con{h, t}:
      Con{h, push8(t, v)}

# Rust's swap_remove: the last record takes the removed one's place.
def last8(+xs: List<&2, R.W8>) -> R.W8:
  match xs:
    case Nil{}:
      zero8()
    case Con{h, t}:
      match t:
        case Nil{}:
          h
        case Con{_, _}:
          last8(t)

def pop8(+xs: List<&2, R.W8>) -> List<&2, R.W8>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      match t:
        case Nil{}:
          Nil{}
        case Con{_, _}:
          Con{h, pop8(t)}

def swap_remove8(+xs: List<&2, R.W8>, +n: Nat) -> List<&2, R.W8>:
  set8(pop8(xs), n, last8(xs))

def keep8_put(+h: R.W8, keep: Bool, +rest: List<&2, R.W8>) -> List<&2, R.W8>:
  match keep:
    case True{}:
      Con{h, rest}
    case False{}:
      rest

# Keep the records the closed test `f` passes, in order.
def keep8(~f: R.W8 -> Bool, +xs: List<&2, R.W8>) -> List<&2, R.W8>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      keep8_put(h, f(h), keep8(~f, t))

# ----- lists of trees -------------------------------------------------------

def len64(+xs: List<&2, R.T8>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{_, t}:
      U32.add(1, len64(t))

def get64(+xs: List<&2, R.T8>, +n: Nat) -> R.T8:
  match xs:
    case Nil{}:
      zero64()
    case Con{h, t}:
      match n:
        case 0n:
          h
        case 1n+p:
          get64(t, p)

def set64(+xs: List<&2, R.T8>, +n: Nat, +v: R.T8) -> List<&2, R.T8>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      match n:
        case 0n:
          Con{v, t}
        case 1n+p:
          Con{h, set64(t, p, v)}

def push64(+xs: List<&2, R.T8>, +v: R.T8) -> List<&2, R.T8>:
  match xs:
    case Nil{}:
      Con{v, Nil{}}
    case Con{h, t}:
      Con{h, push64(t, v)}

def keep64_put(+h: R.T8, keep: Bool, +rest: List<&2, R.T8>) -> List<&2, R.T8>:
  match keep:
    case True{}:
      Con{h, rest}
    case False{}:
      rest

def keep64(~f: R.T8 -> Bool, +xs: List<&2, R.T8>) -> List<&2, R.T8>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      keep64_put(h, f(h), keep64(~f, t))

# ----- lists of words -------------------------------------------------------

def has(+xs: List<&2, U32>, +v: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{h, t}:
      Bool.or(U32.is_eq(h, v), has(t, v))

def push(+xs: List<&2, U32>, +v: U32) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Con{v, Nil{}}
    case Con{h, t}:
      Con{h, push(t, v)}
