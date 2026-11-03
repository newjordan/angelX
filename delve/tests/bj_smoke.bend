import Base
import ../lib/j.bend as J
import ../lib/bj.bend as B

def back(r: B.Read) -> String:
  match r:
    case B.Got{v, rest}:
      B.text(v)
    case B.Bad{at}:
      "BAD:" ++ at

def main() -> IO(Unit):
  do IO<Unit>:
    IO.print(back(B.read("{\"a\":[1,-2,w0000000100000002,f3f800000,true,false,null],\"s\":\"q\\\"\\u00e9\\ud83d\\ude00x\",\"o\":{}}")))
