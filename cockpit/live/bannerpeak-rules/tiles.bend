import Base
import ./answer.bend as Ans
import ./map.bend as Map
import ./nums.bend as N

def air() -> U32:
  0

def stone() -> U32:
  1

def nth_row(rows: List<List<Map.Run>>, n: Nat) -> List<Map.Run>:
  match rows n:
    case Nil{} _:
      Nil{}
    case Con{h, t} 0n:
      h
    case Con{h, t} 1n+p:
      nth_row(t, p)

def in_run(runs: List<Map.Run>, +at: U32) -> U32:
  match runs:
    case Nil{}:
      air()
    case Con{h, t}:
      match h:
        case Map.Run{+col, +code, +len}:
          Bool.pick(
            U32,
            U32.is_lt(at, col),
            air(),
            Bool.pick(U32, U32.is_lt(at, (col + len : U32)), code, in_run(t, at))
          )

def at(rows: List<List<Map.Run>>, +tx: U32, +ty: U32) -> U32:
  Bool.pick(
    U32,
    U32.is_ge(tx, Map.width()),
    stone(),
    Bool.pick(U32, U32.is_ge(ty, Map.height()), air(), in_run(nth_row(rows, U32.to_nat(ty)), tx))
  )

def solid(+code: U32) -> Bool:
  Bool.pick(
    Bool,
    U32.is_zero(code),
    False{},
    Bool.pick(
      Bool,
      U32.is_eq(code, 3),
      False{},
      Bool.pick(Bool, U32.is_eq(code, 9), False{}, U32.is_ne(code, 10))
    )
  )

def one_way(+code: U32) -> Bool:
  U32.is_eq(code, 3)

def slippery(+code: U32) -> Bool:
  U32.is_eq(code, 2)

def runs_of(runs: List<Map.Run>, +acc: U32) -> U32:
  match runs:
    case Nil{}:
      acc
    case Con{_, t}:
      runs_of(t, (acc + 1 : U32))

def runs(rows: List<List<Map.Run>>, +acc: U32) -> U32:
  match rows:
    case Nil{}:
      acc
    case Con{h, t}:
      runs(t, (acc + runs_of(h, 0) : U32))

def describe(rows: List<List<Map.Run>>, +tx: U32, +ty: U32) -> Ans.Answer:
  +code = at(rows, tx, ty)
  Ans.Code{code, solid(code), one_way(code)}

def whole(rows: List<List<Map.Run>>, +width: U32, +height: U32) -> Ans.Answer:
  Ans.Whole{height, runs(rows, 0), width, height}
