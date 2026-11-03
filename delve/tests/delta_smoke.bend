import Base
import ../lib/j.bend as J
import ../lib/bj.bend as B
import ../lib/delta.bend as D

def val(r: B.Read) -> J.Val:
  match r:
    case B.Got{v, rest}:
      v
    case B.Bad{at}:
      J.JS{"BAD"}

def main() -> IO(Unit):
  +a = val(B.read("{\"a\":[1,2,3],\"b\":{\"x\":1,\"y\":2},\"ab\":5,\"gone\":true}"))
  +b = val(B.read("{\"a\":[1,9],\"b\":{\"x\":1,\"z\":3},\"ab\":5,\"new\":[f3f800000]}"))
  +c = val(B.read("{\"a\":[1,9,4,5],\"b\":{\"x\":1,\"y\":2},\"ab\":5}"))
  +d1 = D.diff(a, b)
  +d2 = D.diff(b, c)
  do IO<Unit>:
    IO.print(B.text(d1))
    IO.print(Bool.show(J.eq(D.patch(a, d1), b)))
    IO.print(B.text(d2))
    IO.print(Bool.show(J.eq(D.patch(b, d2), c)))
    IO.print(B.text(D.diff(c, c)))
