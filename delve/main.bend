import Base
import ./lib/j.bend as J
import ./lib/bj.bend as B
import ./lib/delta.bend as D
import ./step.bend as S

# Delve's rules worker (docs/architecture/delve-in-bend.md). Frames on stdin
# and stdout: a 4-byte big-endian length, then the text. After `prepare` the
# text is BJ1 and the Run lives here between applies: the host sends what it
# changed and is answered with what the step changed.

type St is Data:
  St{staged: Bool, active: Bool, run: J.Val}

type Answer is Data:
  Answer{result: J.Val, st: St, stop: Bool}

def ok(+v: J.Val) -> J.Val:
  J.set(J.obj(), "Ok", v)

def err(+why: String) -> J.Val:
  J.set(J.obj(), "Err", J.JS{why})

def contracts() -> J.Val:
  c0 = J.obj()
  c1 = J.set(c0, "delve.scroll", J.JU{1})
  c2 = J.set(c1, "delve.card-paint", J.JU{1})
  c3 = J.set(c2, "delve.home", J.JU{1})
  c4 = J.set(c3, "delve.brew", J.JU{1})
  c5 = J.set(c4, "delve.reforge", J.JU{1})
  J.set(c5, "delve.wish", J.JU{1})

def hello() -> J.Val:
  h0 = J.obj()
  h1 = J.set(h0, "id", J.JS{"delve"})
  h2 = J.set(h1, "api", J.JU{1})
  h3 = J.set(h2, "state_schema", J.JU{6})
  h4 = J.set(h3, "contracts", contracts())
  h5 = J.set(h4, "codec", J.JS{"bj1"})
  J.set(h5, "resident", J.JBool{True{}})

# Lifecycle
# ---------

def restore.if(fits: Bool, +body: J.Val, st: St) -> Answer:
  match fits st:
    case True{} St{staged, active, run}:
      Answer{ok(body), St{True{}, active, body}, False{}}
    case False{} st:
      Answer{err("unsupported Delve state schema"), st, False{}}

def restore(+body: J.Val, st: St) -> Answer:
  restore.if(U32.is_eq(J.u32(J.get(body, "schema")), 6), body, st)

def activate(st: St) -> Answer:
  match st:
    case St{True{}, active, run}:
      Answer{ok(J.JNull{}), St{True{}, True{}, run}, False{}}
    case St{False{}, active, run}:
      Answer{err("invalid Delve system lifecycle request activate"), St{False{}, active, run}, False{}}

# The state as the request leaves it: the resident Run with the host's ops
# laid on, or the whole state it sent.
def given.if(resident: Bool, +body: J.Val, +run: J.Val) -> J.Val:
  match resident:
    case True{}:
      D.patch(run, J.get(body, "delta"))
    case False{}:
      J.get(body, "state")

def reply.if(resident: Bool, +before: J.Val, +after: J.Val) -> J.Val:
  match resident:
    case True{}:
      J.set(J.set(J.obj(), "output", J.JNull{}), "delta", D.diff(before, after))
    case False{}:
      J.set(J.set(J.obj(), "output", J.JNull{}), "state", after)

def applied(resident: Bool, +before: J.Val, s: S.Stepped, +staged: Bool) -> Answer:
  match s:
    case S.Ran{+after}:
      Answer{ok(reply.if(resident, before, after)), St{staged, True{}, after}, False{}}
    case S.Refused{why}:
      Answer{err(why), St{staged, True{}, before}, False{}}

def apply.on(+resident: Bool, +before: J.Val, +event: J.Val, +staged: Bool) -> Answer:
  applied(resident, before, S.step(before, event), staged)

def apply.go(+resident: Bool, +body: J.Val, st: St) -> Answer:
  match st:
    case St{+staged, active, +run}:
      apply.on(resident, given.if(resident, body, run), J.get(body, "event"), staged)

def apply(+body: J.Val, st: St) -> Answer:
  match st:
    case St{staged, True{}, run}:
      apply.go(J.has(body, "delta"), body, St{staged, True{}, run})
    case St{staged, False{}, run}:
      Answer{err("invalid Delve system lifecycle request apply"), St{staged, False{}, run}, False{}}

def compute.if(fits: Bool, +body: J.Val, st: St) -> Answer:
  match fits:
    case True{}:
      Answer{S.compute(J.str(J.get(body, "contract")), J.get(body, "input")), st, False{}}
    case False{}:
      Answer{err("unsupported Delve computation contract"), st, False{}}

def compute(+body: J.Val, st: St) -> Answer:
  match st:
    case St{staged, True{}, run}:
      compute.if(U32.is_eq(J.u32(J.get(body, "version")), 1), body, St{staged, True{}, run})
    case St{staged, False{}, run}:
      Answer{err("invalid Delve system lifecycle request compute"), St{staged, False{}, run}, False{}}

def dispose(st: St) -> Answer:
  match st:
    case St{staged, active, run}:
      Answer{ok(J.JNull{}), St{staged, False{}, run}, True{}}

def dispatch(+method: String, +body: J.Val, st: St) -> Answer:
  match method:
    case SCon{'p', SCon{'r', SCon{'e', _}}}:
      Answer{ok(hello()), st, False{}}
    case SCon{'r', _}:
      restore(body, st)
    case SCon{'a', SCon{'c', _}}:
      activate(st)
    case SCon{'a', SCon{'p', _}}:
      apply(body, st)
    case SCon{'c', _}:
      compute(body, st)
    case SCon{'d', _}:
      dispose(st)
    case _:
      Answer{err("invalid Delve system lifecycle request " ++ method), st, False{}}

# Frames
# ------

def envelope(+req: J.Val, +result: J.Val) -> J.Val:
  e0 = J.set(J.obj(), "api", J.JU{1})
  e1 = J.set(e0, "generation", J.get(req, "generation"))
  e2 = J.set(e1, "request", J.get(req, "request"))
  J.set(e2, "result", result)

def size(bs: List<&2, U32>) -> Maybe<&2, U32>:
  match bs:
    case Con{+a, Con{+b, Con{+c, Con{+d, Nil{}}}}}:
      Some{((a << 24n) .|. (b << 16n) .|. (c << 8n) .|. d : U32)}
    case _:
      None{}

def header(+n: U32) -> List<&2, U32>:
  [(n >> 24n : U32), ((n >> 16n) .&. 255 : U32), ((n >> 8n) .&. 255 : U32), (n .&. 255 : U32)]

def send.body(r: File & Result<&1, &1, U32 & String, Unit>, +text: String) -> IO(File):
  (o, x) = r
  do IO<File>:
    o2 : File & Result<&1, &1, U32 & String, Unit> <- File.write(o, text)
    return Pair.fst(File, Result<&1, &1, U32 & String, Unit>, o2)

def send(out: File, +v: J.Val) -> IO(File):
  +text = B.text(v)
  do IO<File>:
    o1 : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(out, header(B.utf8.len(text, 0)))
    o2 : File <- send.body(o1, text)
    return o2

def answered?(inp: File, out: File, +req: J.Val, a: Answer) -> IO(Unit):
  match a:
    case Answer{+result, st, +stop}:
      do IO<Unit>:
        o2 : File <- send(out, envelope(req, result))
        next(stop, inp, o2, st)

def next?(stop: Bool, inp: File, out: File, st: St) -> IO(Unit):
  match stop:
    case True{}:
      do IO<Unit>:
        File.close(inp)
        File.close(out)
    case False{}:
      serve(inp, out, st)

def framed?(r: File & Result<&1, &1, U32 & String, String>, out: File, st: St) -> IO(Unit):
  (inp, got) = r
  match got:
    case Done{text}:
      parsed(B.read(text), inp, out, st)
    case Fail{e}:
      IO.die(Unit, 1, "Delve system frame body unreadable")

def parsed?(r: B.Read, inp: File, out: File, st: St) -> IO(Unit):
  match r:
    case B.Got{+req, rest}:
      answered(inp, out, req, dispatch(J.str(J.get(req, "method")), J.get(req, "body"), st))
    case B.Bad{at}:
      IO.die(Unit, 1, "Delve system frame is not BJ1")

def sized?(n: Maybe<&2, U32>, inp: File, out: File, st: St) -> IO(Unit):
  match n:
    case Some{+len}:
      do IO<Unit>:
        r : File & Result<&1, &1, U32 & String, String> <- File.read(inp, len)
        framed(r, out, st)
    case None{}:
      do IO<Unit>:
        File.close(inp)
        File.close(out)

def headed?(r: File & Result<&1, &1, U32 & String, List<&2, U32>>, out: File, st: St) -> IO(Unit):
  (inp, got) = r
  match got:
    case Done{bs}:
      sized(size(bs), inp, out, st)
    case Fail{e}:
      do IO<Unit>:
        File.close(inp)
        File.close(out)

def serve?(inp: File, out: File, st: St) -> IO(Unit):
  do IO<Unit>:
    r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(inp, 4)
    headed(r, out, st)

def main() -> IO(Unit):
  do IO<Unit>:
    inp : File <- IO.try(File, File.open("/dev/stdin", "r"))
    out : File <- IO.try(File, File.open("/dev/stdout", "w"))
    serve(inp, out, St{False{}, False{}, J.JNull{}})
