import Base
import ../../lib/j.bend as J
import ../../lib/bj.bend as B
import ./rules.bend as R

# One case per stdin line, BJ1: {"fn": name, "in": input, "out": expected}.
# Answers one line per case: `ok <fn>` or `bad <fn> <what the port gave>`.

def verdict?(+fn: String, +got: J.Val, +want: J.Val) -> String:
  verdict.if(J.eq(got, want), fn, got)

def verdict.if(same: Bool, +fn: String, +got: J.Val) -> String:
  match same:
    case True{}:
      "ok " ++ fn
    case False{}:
      "bad " ++ fn ++ " " ++ B.text(got)

def judge?(r: B.Read) -> String:
  match r:
    case B.Got{+c, rest}:
      verdict(J.str(J.get(c, "fn")), R.cases.run(J.str(J.get(c, "fn")), J.get(c, "in")), J.get(c, "out"))
    case B.Bad{at}:
      "bad ? unreadable case"

def line?(r: File & Result<&1, &1, U32 & String, String>) -> IO(Unit):
  (f, got) = r
  match got:
    case Done{SNil{}}:
      File.close(f)
    case Done{text}:
      do IO<Unit>:
        IO.print(judge(B.read(text)))
        loop(f)
    case Fail{e}:
      File.close(f)

def loop?(f: File) -> IO(Unit):
  do IO<Unit>:
    r : File & Result<&1, &1, U32 & String, String> <- File.read_line(f, 268435456)
    line(r)

def main() -> IO(Unit):
  do IO<Unit>:
    f : File <- IO.try(File, File.open("/dev/stdin", "r"))
    loop(f)
