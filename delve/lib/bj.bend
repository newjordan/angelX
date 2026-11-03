import Base
import ./j.bend as J

# BJ1, the worker's wire (docs/architecture/delve-in-bend.md): JSON whose
# floats are `f` and the 8 hex digits of their bits, and whose integers past
# 2^32 are `w` and 16 hex digits. Reading answers the value and the text after
# it; showing prepends a value's text onto the text that follows.

type Read is Data:
  Got{v: J.Val, rest: String}
  Bad{at: String}

# Reading
# -------

def ws?(s: String) -> String:
  match s:
    case SCon{' ', t}:
      ws(t)
    case SCon{'\n', t}:
      ws(t)
    case SCon{'\r', t}:
      ws(t)
    case SCon{'\t', t}:
      ws(t)
    case s:
      s

def hex.digit.pick(lt: Bool, +c: U32) -> U32:
  match lt:
    case True{}:
      (c - 48 : U32)
    case False{}:
      Bool.pick(U32, U32.is_lt(c, 71), (c - 55 : U32), (c - 87 : U32))

def hex.digit.at(+c: U32) -> U32:
  hex.digit.pick(U32.is_lt(c, 58), c)

# `n` hex digits as a word, and the text after them.
def hex.go(n: Nat, s: String, +acc: U32) -> U32 & String:
  match n s:
    case 0n s:
      (acc, s)
    case 1n+p SCon{Chr{+c}, t}:
      hex.go(p, t, ((acc << 4n) .|. hex.digit.at(c) : U32))
    case 1n+p SNil{}:
      (acc, SNil{})

def hex(n: Nat, s: String) -> U32 & String:
  hex.go(n, s, 0)

def is_digit(+c: Char) -> Bool:
  Char.is_digit(c)

# Decimal digits onto `acc`.
def dec?(s: String, +acc: U32) -> U32 & String:
  match s:
    case SCon{Chr{+c}, t}:
      dec.if(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), c, t, acc)
    case SNil{}:
      (acc, SNil{})

def dec.if?(digit: Bool, +c: U32, t: String, +acc: U32) -> U32 & String:
  match digit:
    case True{}:
      dec(t, (acc * 10 + (c - 48) : U32))
    case False{}:
      (acc, SCon{Chr{c}, t})

def f32.of_bits(w: U32) -> F32:
  match w:
    case U32{d}:
      F32{d}

def got.f(r: U32 & String) -> Read:
  (w, rest) = r
  Got{J.JF{f32.of_bits(w)}, rest}

def got.u(r: U32 & String) -> Read:
  (v, rest) = r
  Got{J.JU{v}, rest}

def got.i(r: U32 & String) -> Read:
  (v, rest) = r
  Got{J.JI{v}, rest}

def got.w.lo(+hi: U32, r: U32 & String) -> Read:
  (lo, rest) = r
  Got{J.JW{hi, lo}, rest}

def got.w(r: U32 & String) -> Read:
  (hi, rest) = r
  got.w.lo(hi, hex(8n, rest))

# A string's characters after its opening quote, reversed onto `acc`.
def str.go?(s: String, acc: String) -> Read:
  match s:
    case SCon{'"', t}:
      Got{J.JS{String.reverse(acc)}, t}
    case SCon{'\\', SCon{'n', t}}:
      str.go(t, SCon{'\n', acc})
    case SCon{'\\', SCon{'t', t}}:
      str.go(t, SCon{'\t', acc})
    case SCon{'\\', SCon{'r', t}}:
      str.go(t, SCon{'\r', acc})
    case SCon{'\\', SCon{'b', t}}:
      str.go(t, SCon{Chr{8}, acc})
    case SCon{'\\', SCon{'f', t}}:
      str.go(t, SCon{Chr{12}, acc})
    case SCon{'\\', SCon{'u', t}}:
      str.u(hex(4n, t), acc)
    case SCon{'\\', SCon{c, t}}:
      str.go(t, SCon{c, acc})
    case SCon{c, t}:
      str.go(t, SCon{c, acc})
    case SNil{}:
      Bad{SNil{}}

# \uXXXX, and its low surrogate when it is a high one.
def str.u?(r: U32 & String, acc: String) -> Read:
  (+hi, rest) = r
  str.u.if(Bool.and(U32.is_ge(hi, 55296), U32.is_lt(hi, 56320)), hi, rest, acc)

def str.u.if?(high: Bool, +hi: U32, rest: String, acc: String) -> Read:
  match high rest:
    case True{} SCon{'\\', SCon{'u', t}}:
      str.u.low(hi, hex(4n, t), acc)
    case _ rest:
      str.go(rest, SCon{Chr{hi}, acc})

def str.u.low?(+hi: U32, r: U32 & String, acc: String) -> Read:
  (+lo, rest) = r
  str.go(rest, SCon{Chr{(65536 + ((hi - 55296) << 10n) + (lo - 56320) : U32)}, acc})

def word?(s: String, w: String, v: J.Val) -> Read:
  match s w:
    case s SNil{}:
      Got{v, s}
    case SCon{Chr{+a}, t} SCon{Chr{+b}, u}:
      word.if(U32.is_eq(a, b), t, u, v)
    case _ _:
      Bad{s}

def word.if?(same: Bool, t: String, u: String, v: J.Val) -> Read:
  match same:
    case True{}:
      word(t, u, v)
    case False{}:
      Bad{t}

def value?(s: String) -> Read:
  value.at(ws(s))

def value.at?(s: String) -> Read:
  match s:
    case SCon{'{', t}:
      obj.open(ws(t))
    case SCon{'[', t}:
      arr.open(ws(t))
    case SCon{'"', t}:
      str.go(t, SNil{})
    case SCon{'n', t}:
      word(t, "ull", J.JNull{})
    case SCon{'t', t}:
      word(t, "rue", J.JBool{True{}})
    case SCon{'f', SCon{'a', SCon{'l', t}}}:
      word(t, "se", J.JBool{False{}})
    case SCon{'f', t}:
      got.f(hex(8n, t))
    case SCon{'w', t}:
      got.w(hex(8n, t))
    case SCon{'-', t}:
      got.i(dec(t, 0))
    case SCon{c, t}:
      got.u(dec(SCon{c, t}, 0))
    case SNil{}:
      Bad{SNil{}}

def arr.open?(s: String) -> Read:
  match s:
    case SCon{']', t}:
      Got{J.JA{Nil{}}, t}
    case s:
      arr.item(value(s), Nil{})

def arr.item?(r: Read, acc: List<&2, J.Val>) -> Read:
  match r:
    case Got{v, rest}:
      arr.next(ws(rest), v <> acc)
    case Bad{at}:
      Bad{at}

def arr.next?(s: String, acc: List<&2, J.Val>) -> Read:
  match s:
    case SCon{',', t}:
      arr.item(value(t), acc)
    case SCon{']', t}:
      Got{J.JA{List.reverse(&2, J.Val, acc)}, t}
    case s:
      Bad{s}

def obj.open?(s: String) -> Read:
  match s:
    case SCon{'}', t}:
      Got{J.obj(), t}
    case s:
      obj.key(s, Map.new(&2, J.Val))

def obj.key?(s: String, m: Map<&2, J.Val>) -> Read:
  obj.key.at(ws(s), m)

def obj.key.at?(s: String, m: Map<&2, J.Val>) -> Read:
  match s:
    case SCon{'"', t}:
      obj.colon(str.go(t, SNil{}), m)
    case s:
      Bad{s}

def obj.colon?(r: Read, m: Map<&2, J.Val>) -> Read:
  match r:
    case Got{J.JS{k}, rest}:
      obj.colon.go(ws(rest), k, m)
    case Got{v, rest}:
      Bad{rest}
    case Bad{at}:
      Bad{at}

def obj.colon.go?(s: String, k: String, m: Map<&2, J.Val>) -> Read:
  match s:
    case SCon{':', t}:
      obj.val(value(t), k, m)
    case s:
      Bad{s}

def obj.val?(r: Read, k: String, m: Map<&2, J.Val>) -> Read:
  match r:
    case Got{v, rest}:
      obj.next(ws(rest), Map.set(&2, J.Val, m, k, v))
    case Bad{at}:
      Bad{at}

def obj.next?(s: String, m: Map<&2, J.Val>) -> Read:
  match s:
    case SCon{',', t}:
      obj.key(t, m)
    case SCon{'}', t}:
      Got{J.JO{m}, t}
    case s:
      Bad{s}

# The whole text as one value.
def read(s: String) -> Read:
  value(s)

# Showing
# -------

def hex.char(+d: U32) -> Char:
  Chr{Bool.pick(U32, U32.is_lt(d, 10), (d + 48 : U32), (d + 87 : U32))}

# The low `n` hex digits of `w`, most significant first, before `rest`.
def hex.show(n: Nat, +w: U32, rest: String) -> String:
  match n:
    case 0n:
      rest
    case 1n+p:
      hex.show(p, (w >> 4n : U32), SCon{hex.char((w .&. 15 : U32)), rest})

def hex8(+w: U32, rest: String) -> String:
  hex.show(8n, w, rest)

def esc.hex(+c: U32, rest: String) -> String:
  String.append("\\u", hex.show(4n, c, rest))

def esc?(s: String, rest: String) -> String:
  match s:
    case SNil{}:
      rest
    case SCon{'"', t}:
      SCon{'\\', SCon{'"', esc(t, rest)}}
    case SCon{'\\', t}:
      SCon{'\\', SCon{'\\', esc(t, rest)}}
    case SCon{'\n', t}:
      SCon{'\\', SCon{'n', esc(t, rest)}}
    case SCon{'\r', t}:
      SCon{'\\', SCon{'r', esc(t, rest)}}
    case SCon{'\t', t}:
      SCon{'\\', SCon{'t', esc(t, rest)}}
    case SCon{Chr{+c}, t}:
      esc.if(U32.is_lt(c, 32), c, t, rest)

def esc.if?(ctl: Bool, +c: U32, t: String, rest: String) -> String:
  match ctl:
    case True{}:
      esc.hex(c, esc(t, rest))
    case False{}:
      SCon{Chr{c}, esc(t, rest)}

def show?(+v: J.Val, rest: String) -> String:
  match v:
    case J.JNull{}:
      String.append("null", rest)
    case J.JBool{True{}}:
      String.append("true", rest)
    case J.JBool{False{}}:
      String.append("false", rest)
    case J.JU{+n}:
      String.append(U32.show(n), rest)
    case J.JI{+n}:
      SCon{'-', String.append(U32.show(n), rest)}
    case J.JW{+hi, +lo}:
      SCon{'w', hex8(hi, hex8(lo, rest))}
    case J.JF{+x}:
      SCon{'f', hex8(F32.bits(x), rest)}
    case J.JS{+s}:
      SCon{'"', esc(s, SCon{'"', rest})}
    case J.JA{+xs}:
      SCon{'[', show.items(xs, SCon{']', rest})}
    case J.JO{+m}:
      SCon{'{', show.fields(Map.to_list(&2, J.Val, m), SCon{'}', rest})}

def show.items?(+xs: List<&2, J.Val>, rest: String) -> String:
  match xs:
    case Nil{}:
      rest
    case Con{+h, Nil{}}:
      show(h, rest)
    case Con{+h, +t}:
      show(h, SCon{',', show.items(t, rest)})

def show.fields?(+xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, rest: String) -> String:
  match xs:
    case Nil{}:
      rest
    case Con{(+k, +v), Nil{}}:
      SCon{'"', esc(k, String.append("\":", show(v, rest)))}
    case Con{(+k, +v), +t}:
      SCon{'"', esc(k, String.append("\":", show(v, SCon{',', show.fields(t, rest)})))}

def text(+v: J.Val) -> String:
  show(v, SNil{})

# UTF-8 bytes in a text, for a frame's length.
def utf8.width(+c: U32) -> U32:
  Bool.pick(U32, U32.is_lt(c, 128), 1,
    Bool.pick(U32, U32.is_lt(c, 2048), 2, Bool.pick(U32, U32.is_lt(c, 65536), 3, 4)))

def utf8.len?(s: String, +acc: U32) -> U32:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      utf8.len(t, (acc + utf8.width(c) : U32))
