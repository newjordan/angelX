# if / else, lazily: only the chosen branch is run.
def home.util.ite(-A: Type, c: Bool, a: Unit -> A, b: Unit -> A) -> A:
  k = Bool.pick(Unit -> A, c, a, b)
  k(Unit{})

# Rust std behaviours the home group's modules lean on, as Bend defs. Names
# are `home.util.*`; nothing here is a Rust function of Delve's own.

def home.util.rng(+x: U32, +lo: U32, +hi: U32) -> Bool:
  Bool.and(U32.is_ge(x, lo), U32.is_le(x, hi))

# String.len(): UTF-8 bytes
def home.util.blen(+s: String) -> U32:
  B.utf8.len(s, 0)

# bytes ≤ n
def home.util.blen_le(+s: String, +n: U32) -> Bool:
  U32.is_le(B.utf8.len(s, 0), n)

# u8::is_ascii_lowercase etc. on a code point
def home.util.c_lower(+x: U32) -> Bool:
  home.util.rng(x, 97, 122)

def home.util.c_upper(+x: U32) -> Bool:
  home.util.rng(x, 65, 90)

def home.util.c_digit(+x: U32) -> Bool:
  home.util.rng(x, 48, 57)

def home.util.c_alnum(+x: U32) -> Bool:
  Bool.or(home.util.c_lower(x), Bool.or(home.util.c_upper(x), home.util.c_digit(x)))

# whether x lies in any [lo, hi] pair of the flat list
def home.util.in_ranges(+x: U32, +rs: List<&2, U32>) -> Bool:
  match rs:
    case Con{+lo, Con{+hi, +t}}:
      Bool.or(home.util.rng(x, lo, hi), home.util.in_ranges(x, t))
    case _:
      False{}

# char::is_whitespace
def home.util.c_space(+x: U32) -> Bool:
  home.util.in_ranges(x, [9, 13, 32, 32, 133, 133, 160, 160, 5760, 5760, 8192, 8202, 8232, 8233, 8239, 8239, 8287, 8287, 12288, 12288])

# char::is_control
def home.util.c_control(+x: U32) -> Bool:
  Bool.or(U32.is_lt(x, 32), home.util.rng(x, 127, 159))

# char::is_alphanumeric, ASCII exact; beyond ASCII every letter-ish code point
# counts (punctuation and symbol blocks do not).
def home.util.c_alnum_u(+x: U32) -> Bool:
  Bool.or(home.util.c_alnum(x), Bool.and(U32.is_ge(x, 170), Bool.not(home.util.c_punct_u(x))))

def home.util.c_punct_u(+x: U32) -> Bool:
  home.util.in_ranges(x, [160, 169, 171, 180, 182, 183, 187, 187, 191, 191, 215, 215, 247, 247, 8192, 8303, 8352, 8527, 8592, 11263, 12288, 12351, 65024, 65039])

# char::to_lowercase for the one-to-one cases (ASCII, Latin-1, Greek, Cyrillic)
def home.util.c_lower_of(+x: U32) -> U32:
  Bool.pick(U32, home.util.c_upper(x), (x + 32 : U32),
    Bool.pick(U32, Bool.and(home.util.rng(x, 192, 222), Bool.not(U32.is_eq(x, 215))), (x + 32 : U32),
      Bool.pick(U32, home.util.rng(x, 913, 937), (x + 32 : U32),
        Bool.pick(U32, home.util.rng(x, 1040, 1071), (x + 32 : U32),
          Bool.pick(U32, home.util.rng(x, 1024, 1039), (x + 80 : U32), x)))))

def home.util.lower_chars(+cs: List<&2, Char>) -> List<&2, Char>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Char.from_u32(home.util.c_lower_of(Char.to_u32(h))) <> home.util.lower_chars(t)

# str::to_lowercase
def home.util.lower(+s: String) -> String:
  String.from_list(home.util.lower_chars(String.to_list(s)))

# u8::to_ascii_lowercase over a string
def home.util.ascii_lower_chars(+cs: List<&2, Char>) -> List<&2, Char>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Char.from_u32(Bool.pick(U32, home.util.c_upper(Char.to_u32(h)), (Char.to_u32(h) + 32 : U32), Char.to_u32(h))) <> home.util.ascii_lower_chars(t)

def home.util.ascii_lower(+s: String) -> String:
  String.from_list(home.util.ascii_lower_chars(String.to_list(s)))

# a.eq_ignore_ascii_case(b)
def home.util.eq_ic(+a: String, +b: String) -> Bool:
  String.eq(home.util.ascii_lower(a), home.util.ascii_lower(b))

# str::trim_start / trim_end / trim (Unicode White_Space)
def home.util.trim_start(+s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+h, +t}:
      home.util.trim_start.if(home.util.c_space(Char.to_u32(h)), h, t)

def home.util.trim_start.if(sp: Bool, +h: Char, +t: String) -> String:
  match sp:
    case True{}:
      home.util.trim_start(t)
    case False{}:
      SCon{h, t}

def home.util.trim_end(+s: String) -> String:
  String.reverse(home.util.trim_start(String.reverse(s)))

def home.util.trim(+s: String) -> String:
  home.util.trim_end(home.util.trim_start(s))

def home.util.is_blank(+s: String) -> Bool:
  String.is_empty(home.util.trim_start(s))

# str::split_whitespace
def home.util.words(+s: String) -> List<&2, String>:
  home.util.words.go(String.to_list(s), "", Nil{})

def home.util.words.go(+cs: List<&2, Char>, +cur: String, +acc: List<&2, String>) -> List<&2, String>:
  match cs:
    case Nil{}:
      home.util.words.push(cur, acc, True{})
    case Con{+h, +t}:
      home.util.words.step(home.util.c_space(Char.to_u32(h)), h, t, cur, acc)

def home.util.words.step(sp: Bool, +h: Char, +t: List<&2, Char>, +cur: String, +acc: List<&2, String>) -> List<&2, String>:
  match sp:
    case True{}:
      home.util.words.go(t, "", home.util.words.push(cur, acc, False{}))
    case False{}:
      home.util.words.go(t, String.append(cur, SCon{h, SNil{}}), acc)

# the finished word onto the (reversed) list; at the end the list is reversed
def home.util.words.push(+cur: String, +acc: List<&2, String>, done: Bool) -> List<&2, String>:
  match done:
    case True{}:
      List.reverse(&2, String, home.util.words.push.w(cur, acc))
    case False{}:
      home.util.words.push.w(cur, acc)

def home.util.words.push.w(+cur: String, +acc: List<&2, String>) -> List<&2, String>:
  match cur:
    case SNil{}:
      acc
    case SCon{+h, +t}:
      SCon{h, t} <> acc

# [a, b, c].join(sep)
def home.util.join(+xs: List<&2, String>, +sep: String) -> String:
  String.join(xs, sep)

# s.contains(p)
def home.util.has(+s: String, +p: String) -> Bool:
  String.contains(s, p)

# s.starts_with(p)
def home.util.starts(+s: String, +p: String) -> Bool:
  String.starts_with(s, p)

# s.get(..n) as (head, rest) when n lands on a char boundary within s
def home.util.cut(+s: String, +n: U32) -> Maybe<&2, Sigma<&2, &2, String, _ => String>>:
  home.util.cut.go(s, n, "")

def home.util.cut.go(+s: String, +n: U32, +acc: String) -> Maybe<&2, Sigma<&2, &2, String, _ => String>>:
  match n:
    case 0:
      Some{(acc, s)}
    case _:
      home.util.cut.step(s, n, acc)

def home.util.cut.step(+s: String, +n: U32, +acc: String) -> Maybe<&2, Sigma<&2, &2, String, _ => String>>:
  match s:
    case SNil{}:
      None{}
    case SCon{+h, +t}:
      home.util.cut.take(U32.is_ge(n, B.utf8.width(Char.to_u32(h))), h, t, n, acc)

def home.util.cut.take(ok: Bool, +h: Char, +t: String, +n: U32, +acc: String) -> Maybe<&2, Sigma<&2, &2, String, _ => String>>:
  match ok:
    case True{}:
      home.util.cut.go(t, (n - B.utf8.width(Char.to_u32(h)) : U32), String.append(acc, SCon{h, SNil{}}))
    case False{}:
      None{}

# str::parse::<u32>(): digits with an optional leading `+`, no overflow
def home.util.parse_u32(+s: String) -> Maybe<&2, U32>:
  match s:
    case SNil{}:
      None{}
    case SCon{+h, +t}:
      home.util.parse_u32.sign(U32.is_eq(Char.to_u32(h), 43), s, t)

def home.util.parse_u32.sign(plus: Bool, +s: String, +t: String) -> Maybe<&2, U32>:
  match plus:
    case True{}:
      home.util.parse_u32.digits(t, 0, False{})
    case False{}:
      home.util.parse_u32.digits(s, 0, False{})

def home.util.parse_u32.digits(+s: String, +acc: U32, +any: Bool) -> Maybe<&2, U32>:
  match s:
    case SNil{}:
      home.util.parse_u32.end(any, acc)
    case SCon{+h, +t}:
      home.util.parse_u32.digit(home.util.c_digit(Char.to_u32(h)), (Char.to_u32(h) - 48 : U32), t, acc)

def home.util.parse_u32.end(any: Bool, +acc: U32) -> Maybe<&2, U32>:
  match any:
    case True{}:
      Some{acc}
    case False{}:
      None{}

def home.util.parse_u32.digit(ok: Bool, +d: U32, +t: String, +acc: U32) -> Maybe<&2, U32>:
  match ok:
    case True{}:
      home.util.parse_u32.fits(Bool.or(U32.is_gt(acc, 429496729), Bool.and(U32.is_eq(acc, 429496729), U32.is_gt(d, 5))), d, t, acc)
    case False{}:
      None{}

def home.util.parse_u32.fits(over: Bool, +d: U32, +t: String, +acc: U32) -> Maybe<&2, U32>:
  match over:
    case True{}:
      None{}
    case False{}:
      home.util.parse_u32.digits(t, (acc * 10 + d : U32), True{})

# Option<u32> with a default
def home.util.or(+m: Maybe<&2, U32>, +d: U32) -> U32:
  match m:
    case Some{+v}:
      v
    case None{}:
      d

# str::trim_end_matches(c) for a single char code
def home.util.trim_end_c(+s: String, +c: U32) -> String:
  String.reverse(home.util.trim_start_c(String.reverse(s), c))

def home.util.trim_start_c(+s: String, +c: U32) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+h, +t}:
      home.util.trim_start_c.if(U32.is_eq(Char.to_u32(h), c), h, t, c)

def home.util.trim_start_c.if(same: Bool, +h: Char, +t: String, +c: U32) -> String:
  match same:
    case True{}:
      home.util.trim_start_c(t, c)
    case False{}:
      SCon{h, t}

# str::chars().count()
def home.util.chars(+s: String) -> U32:
  U32.from_nat(String.length(s))

# Option<&str> of an object field: the string, or None when null/absent
def home.util.opt_str(+o: J.Val, +k: String) -> Maybe<&2, String>:
  home.util.opt_str.of(J.get(o, k))

def home.util.opt_str.of(+v: J.Val) -> Maybe<&2, String>:
  match v:
    case J.JS{+s}:
      Some{s}
    case _:
      None{}

# whether a J array holds the string
def home.util.arr_has(+xs: List<&2, J.Val>, +s: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(String.eq(J.str(h), s), home.util.arr_has(t, s))

def home.util.str_has(+v: J.Val, +s: String) -> Bool:
  home.util.arr_has(J.items(v), s)

# a J array's strings with `f` applied is rarely needed; these cover counts
def home.util.count(+xs: List<&2, J.Val>) -> U32:
  U32.from_nat(List.length(&2, J.Val, xs))

# every item of an array has a string within `n` bytes
def home.util.all_len(+xs: List<&2, J.Val>, +n: U32) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Bool.and(home.util.blen_le(J.str(h), n), home.util.all_len(t, n))

# 1 + digits of a u32 as decimal text
def home.util.dec_len(+n: U32) -> U32:
  Bool.pick(U32, U32.is_lt(n, 10), 1, Bool.pick(U32, U32.is_lt(n, 100), 2, Bool.pick(U32, U32.is_lt(n, 1000), 3, Bool.pick(U32, U32.is_lt(n, 10000), 4, Bool.pick(U32, U32.is_lt(n, 100000), 5, Bool.pick(U32, U32.is_lt(n, 1000000), 6, Bool.pick(U32, U32.is_lt(n, 10000000), 7, Bool.pick(U32, U32.is_lt(n, 100000000), 8, Bool.pick(U32, U32.is_lt(n, 1000000000), 9, 10)))))))))

# bytes of a string as serde_json writes it (quotes and escapes included)
def home.util.jstr_len(+s: String) -> U32:
  (2 + home.util.jstr_len.go(String.to_list(s), 0) : U32)

def home.util.jstr_len.go(+cs: List<&2, Char>, +acc: U32) -> U32:
  match cs:
    case Nil{}:
      acc
    case Con{+h, +t}:
      home.util.jstr_len.go(t, (acc + home.util.jchar_len(Char.to_u32(h)) : U32))

def home.util.jchar_len(+x: U32) -> U32:
  Bool.pick(U32, Bool.or(U32.is_eq(x, 34), U32.is_eq(x, 92)), 2,
    Bool.pick(U32, U32.is_lt(x, 32),
      Bool.pick(U32, Bool.or(U32.is_eq(x, 8), Bool.or(U32.is_eq(x, 9), Bool.or(U32.is_eq(x, 10), Bool.or(U32.is_eq(x, 12), U32.is_eq(x, 13))))), 2, 6),
      B.utf8.width(x)))

# bytes of a value as serde_json::to_vec writes it. A float counts as its
# shortest f32 text would for the common cases (whole numbers `N.0`, else 10).
def home.util.jlen(+v: J.Val) -> U32:
  match v:
    case J.JNull{}:
      4
    case J.JBool{+b}:
      Bool.pick(U32, b, 4, 5)
    case J.JU{+n}:
      home.util.dec_len(n)
    case J.JI{+n}:
      (1 + home.util.dec_len(n) : U32)
    case J.JW{+hi, +lo}:
      (home.util.dec_len(hi) + 9 : U32)
    case J.JF{+f}:
      home.util.jlen.f(f)
    case J.JS{+s}:
      home.util.jstr_len(s)
    case J.JA{+xs}:
      (2 + home.util.jlen.items(xs, 0) : U32)
    case J.JO{+m}:
      (2 + home.util.jlen.fields(Map.to_list(&2, J.Val, m), 0) : U32)

def home.util.jlen.f(+f: F32) -> U32:
  Bool.pick(U32, F32.is_lt(F32.abs(f), 1000000.0), (home.util.dec_len(F32.to_u32(F32.abs(f))) + Bool.pick(U32, F32.is_lt(f, 0.0), 3, 2) : U32), 10)

def home.util.jlen.items(+xs: List<&2, J.Val>, +acc: U32) -> U32:
  match xs:
    case Nil{}:
      acc
    case Con{+h, +t}:
      home.util.jlen.items(t, (acc + home.util.jlen(h) + Bool.pick(U32, String.is_empty(""), 1, 1) : U32))

def home.util.jlen.fields(+xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +acc: U32) -> U32:
  match xs:
    case Nil{}:
      acc
    case Con{(+k, +v), +t}:
      home.util.jlen.fields(t, (acc + home.util.jstr_len(k) + 1 + home.util.jlen(v) + 1 : U32))

# An object from key/value pairs (later keys win).
def home.util.obj(+kvs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> J.Val:
  home.util.obj.go(kvs, J.obj())

def home.util.obj.go(+kvs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +o: J.Val) -> J.Val:
  match kvs:
    case Nil{}:
      o
    case Con{(+k, +v), +t}:
      home.util.obj.go(t, J.set(o, k, v))

# `o` with `k` set to `v` only when `keep`.
def home.util.set_if(+keep: Bool, +o: J.Val, +k: String, +v: J.Val) -> J.Val:
  match keep:
    case True{}:
      J.set(o, k, v)
    case False{}:
      o

# {"Ok": v} and {"Err": text}, the system reply's result shape
def home.util.ok(+v: J.Val) -> J.Val:
  J.set(J.obj(), "Ok", v)

def home.util.err(+why: String) -> J.Val:
  J.set(J.obj(), "Err", J.JS{why})

# Option<String> as a J value (null when none)
def home.util.jopt(+m: Maybe<&2, String>) -> J.Val:
  match m:
    case Some{+s}:
      J.JS{s}
    case None{}:
      J.JNull{}

def home.util.jstrs(+xs: List<&2, String>) -> J.Val:
  J.JA{home.util.jstrs.go(xs)}

def home.util.jstrs.go(+xs: List<&2, String>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.JS{h} <> home.util.jstrs.go(t)

# usize/u32 length of a Bend list
def home.util.len(+xs: List<&2, J.Val>) -> U32:
  U32.from_nat(List.length(&2, J.Val, xs))

# `x.get(i)` of a J list
def home.util.get(+xs: List<&2, J.Val>, +i: U32) -> J.Val:
  home.util.get.of(List.get(&2, J.Val, xs, U32.to_nat(i)))

def home.util.get.of(+m: Maybe<&2, J.Val>) -> J.Val:
  match m:
    case Some{+v}:
      v
    case None{}:
      J.JNull{}

# ---- serde's refusals for a `deny_unknown_fields` struct -----------------

# What serde names a value it cannot read as a struct.
def home.util.kind_of(+v: J.Val) -> String:
  match v:
    case J.JNull{}:
      "null"
    case J.JBool{+b}:
      Bool.pick(String, b, "boolean `true`", "boolean `false`")
    case J.JU{+n}:
      "integer `" ++ U32.show(n) ++ "`"
    case J.JI{+n}:
      "integer `-" ++ U32.show(n) ++ "`"
    case J.JW{+hi, +lo}:
      "integer"
    case J.JF{+f}:
      "floating point"
    case J.JS{+s}:
      "string \"" ++ s ++ "\""
    case J.JA{+xs}:
      "sequence"
    case J.JO{+m}:
      "map"

def home.util.quoted(+xs: List<&2, String>, +sep: String) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+h, +t}:
      home.util.quoted.go(t, "`" ++ h ++ "`", sep)

def home.util.quoted.go(+xs: List<&2, String>, +acc: String, +sep: String) -> String:
  match xs:
    case Nil{}:
      acc
    case Con{+h, +t}:
      home.util.quoted.go(t, acc ++ sep ++ "`" ++ h ++ "`", sep)

def home.util.expected(+all: List<&2, String>) -> String:
  match all:
    case Nil{}:
      "there are no fields"
    case Con{+a, Nil{}}:
      "`" ++ a ++ "`"
    case Con{+a, Con{+b, Nil{}}}:
      "`" ++ a ++ "` or `" ++ b ++ "`"
    case _:
      "one of " ++ home.util.quoted(all, ", ")

def home.util.has_name(+xs: List<&2, String>, +k: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(String.eq(h, k), home.util.has_name(t, k))

# The first object key (in sorted order, as serde_json's map yields them)
# that the struct does not have.
def home.util.unknown(+keys: List<&2, String>, +all: List<&2, String>) -> Maybe<&2, String>:
  match keys:
    case Nil{}:
      None{}
    case Con{+k, +t}:
      home.util.unknown.if(home.util.has_name(all, k), k, t, all)

def home.util.unknown.if(known: Bool, +k: String, +t: List<&2, String>, +all: List<&2, String>) -> Maybe<&2, String>:
  match known:
    case True{}:
      home.util.unknown(t, all)
    case False{}:
      Some{k}

def home.util.missing(+o: J.Val, +need: List<&2, String>) -> Maybe<&2, String>:
  match need:
    case Nil{}:
      None{}
    case Con{+k, +t}:
      home.util.missing.if(J.has(o, k), o, k, t)

def home.util.missing.if(here: Bool, +o: J.Val, +k: String, +t: List<&2, String>) -> Maybe<&2, String>:
  match here:
    case True{}:
      home.util.missing(o, t)
    case False{}:
      Some{k}

# Why serde would refuse `v` as struct `name` with these fields (all) and
# these required (need, in declaration order), or None when it reads.
def home.util.shape(+v: J.Val, +name: String, +all: List<&2, String>, +need: List<&2, String>) -> Maybe<&2, String>:
  match v:
    case J.JO{+m}:
      home.util.shape.keys(Map.keys(&2, J.Val, m), J.JO{m}, all, need)
    case _:
      Some{"invalid type: " ++ home.util.kind_of(v) ++ ", expected struct " ++ name}

def home.util.shape.keys(+keys: List<&2, String>, +o: J.Val, +all: List<&2, String>, +need: List<&2, String>) -> Maybe<&2, String>:
  home.util.shape.unk(home.util.unknown(keys, all), o, all, need)

def home.util.shape.unk(+u: Maybe<&2, String>, +o: J.Val, +all: List<&2, String>, +need: List<&2, String>) -> Maybe<&2, String>:
  match u:
    case Some{+k}:
      Some{"unknown field `" ++ k ++ "`, expected " ++ home.util.expected(all)}
    case None{}:
      home.util.shape.miss(home.util.missing(o, need))

def home.util.shape.miss(+m: Maybe<&2, String>) -> Maybe<&2, String>:
  match m:
    case Some{+k}:
      Some{"missing field `" ++ k ++ "`"}
    case None{}:
      None{}

# str::lines(): the text split at \n (a last empty piece dropped), no \r handling
# (callers trim).
def home.util.lines(+s: String) -> List<&2, String>:
  home.util.lines.go(String.to_list(s), "", Nil{}, False{})

# `fresh` is true when the current piece has had no chars yet and the input ended with \n
def home.util.lines.go(+cs: List<&2, Char>, +cur: String, +acc: List<&2, String>, +open: Bool) -> List<&2, String>:
  match cs:
    case Nil{}:
      List.reverse(&2, String, home.util.lines.fin(open, cur, acc))
    case Con{+h, +t}:
      home.util.ite(List<&2, String>, U32.is_eq(Char.to_u32(h), 10),
        u => home.util.lines.go(t, "", cur <> acc, False{}),
        u => home.util.lines.go(t, String.append(cur, SCon{h, SNil{}}), acc, True{}))

def home.util.lines.fin(open: Bool, +cur: String, +acc: List<&2, String>) -> List<&2, String>:
  home.util.ite(List<&2, String>, open, u => cur <> acc, u => acc)

# s.split_once(char::is_whitespace): (before, after) or the whole with ""
def home.util.split_ws1(+s: String) -> Sigma<&2, &2, String, _ => String>:
  home.util.split_ws1.go(s, "")

def home.util.split_ws1.go(+s: String, +acc: String) -> Sigma<&2, &2, String, _ => String>:
  match s:
    case SNil{}:
      (acc, "")
    case SCon{+h, +t}:
      home.util.ite(Sigma<&2, &2, String, _ => String>, home.util.c_space(Char.to_u32(h)),
        u => (acc, t),
        u => home.util.split_ws1.go(t, String.append(acc, SCon{h, SNil{}})))

# s.split(pat).next(): everything before the first `pat`
def home.util.before(+s: String, +pat: String) -> String:
  home.util.before.go(s, pat, "")

def home.util.before.go(+s: String, +pat: String, +acc: String) -> String:
  match s:
    case SNil{}:
      acc
    case SCon{+h, +t}:
      home.util.ite(String, String.starts_with(s, pat),
        u => acc,
        u => home.util.before.go(t, pat, String.append(acc, SCon{h, SNil{}})))

# every char satisfies the code predicate in `ranges` or equals `extra`
def home.util.all_in(+s: String, +ranges: List<&2, U32>, +extra: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{+h, +t}:
      Bool.and(Bool.or(home.util.in_ranges(Char.to_u32(h), ranges), String.contains(extra, SCon{h, SNil{}})), home.util.all_in(t, ranges, extra))

# str::chars().take(n).collect()
def home.util.take_chars(+s: String, +n: U32) -> String:
  String.take(s, U32.to_nat(n))

# the chars that are not control characters
def home.util.no_control(+s: String) -> String:
  String.from_list(home.util.no_control.go(String.to_list(s)))

def home.util.no_control.go(+cs: List<&2, Char>) -> List<&2, Char>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      home.util.ite(List<&2, Char>, home.util.c_control(Char.to_u32(h)), u => home.util.no_control.go(t), u => h <> home.util.no_control.go(t))

# a J array of J.Val with one more item
def home.util.push(+a: J.Val, +v: J.Val) -> J.Val:
  J.push(a, v)

# `{:.1}` of a non-negative finite f32, round-half-even on the exact value
def home.util.fmt1(+f: F32) -> String:
  home.util.fmt1.bits(F32.bits(f))

def home.util.fmt1.bits(+b: U32) -> String:
  home.util.fmt1.parts(((b >> 23n) .&. 255 : U32), (b .&. 8388607 : U32))

def home.util.fmt1.parts(+exp: U32, +frac: U32) -> String:
  home.util.ite(String, U32.is_eq(exp, 0),
    u => "0.0",
    u => home.util.fmt1.tenths(home.util.fmt1.round((8388608 + frac : U32), exp)))

# round(10 * m * 2^(exp-150)) half to even, as a count of tenths
def home.util.fmt1.round(+m: U32, +exp: U32) -> U32:
  home.util.ite(U32, U32.is_ge(exp, 150),
    u => (U32.mul(10, m) << U32.to_nat((exp - 150 : U32)) : U32),
    u => home.util.fmt1.shift((10 * m : U32), (150 - exp : U32)))

def home.util.fmt1.shift(+x: U32, +k: U32) -> U32:
  home.util.ite(U32, U32.is_ge(k, 32), u => 0,
    u => home.util.fmt1.rnd((x >> U32.to_nat(k) : U32), (x .&. ((1 << U32.to_nat(k) : U32) - 1 : U32) : U32), (1 << U32.to_nat((k - 1 : U32)) : U32)))

def home.util.fmt1.rnd(+q: U32, +r: U32, +half: U32) -> U32:
  home.util.ite(U32, Bool.or(U32.is_gt(r, half), Bool.and(U32.is_eq(r, half), Bool.not(U32.is_even(q)))), u => (q + 1 : U32), u => q)

def home.util.fmt1.tenths(+q: U32) -> String:
  U32.show(U32.div(q, 10)) ++ "." ++ U32.show(U32.mod(q, 10))

# str::parse::<f32>() for the plain forms: [+-]digits[.digits][e[+-]digits]
def home.util.parse_f32(+s: String) -> Maybe<&2, F32>:
  match s:
    case SNil{}:
      None{}
    case SCon{+h, +t}:
      home.util.ite(Maybe<&2, F32>, U32.is_eq(Char.to_u32(h), 45),
        u => home.util.parse_f32.neg(home.util.parse_f32.int(t, 0.0, 0, False{})),
        u => home.util.ite(Maybe<&2, F32>, U32.is_eq(Char.to_u32(h), 43),
          u => home.util.parse_f32.int(t, 0.0, 0, False{}),
          u => home.util.parse_f32.int(s, 0.0, 0, False{})))

def home.util.parse_f32.neg(+m: Maybe<&2, F32>) -> Maybe<&2, F32>:
  match m:
    case Some{+f}:
      Some{F32.neg(f)}
    case None{}:
      None{}

# digits before the point: accumulates the mantissa
def home.util.parse_f32.int(+s: String, +m: F32, +dec: U32, +any: Bool) -> Maybe<&2, F32>:
  match s:
    case SNil{}:
      home.util.parse_f32.fin(any, m, dec, 0, False{})
    case SCon{+h, +t}:
      home.util.parse_f32.int.step(Char.to_u32(h), t, m, dec, any)

def home.util.parse_f32.int.step(+c: U32, +t: String, +m: F32, +dec: U32, +any: Bool) -> Maybe<&2, F32>:
  home.util.ite(Maybe<&2, F32>, home.util.c_digit(c),
    u => home.util.parse_f32.int(t, F32.add(F32.mul(m, 10.0), U32.to_f32((c - 48 : U32))), dec, True{}),
    u => home.util.ite(Maybe<&2, F32>, U32.is_eq(c, 46),
      u => home.util.parse_f32.frac(t, m, dec, any),
      u => home.util.ite(Maybe<&2, F32>, Bool.or(U32.is_eq(c, 101), U32.is_eq(c, 69)),
        u => home.util.parse_f32.exp(t, m, dec, any),
        u => None{})))

# digits after the point: each divides the scale by ten
def home.util.parse_f32.frac(+s: String, +m: F32, +dec: U32, +any: Bool) -> Maybe<&2, F32>:
  match s:
    case SNil{}:
      home.util.parse_f32.fin(any, m, dec, 0, False{})
    case SCon{+h, +t}:
      home.util.parse_f32.frac.step(Char.to_u32(h), t, m, dec, any)

def home.util.parse_f32.frac.step(+c: U32, +t: String, +m: F32, +dec: U32, +any: Bool) -> Maybe<&2, F32>:
  home.util.ite(Maybe<&2, F32>, home.util.c_digit(c),
    u => home.util.parse_f32.frac(t, F32.add(F32.mul(m, 10.0), U32.to_f32((c - 48 : U32))), (dec + 1 : U32), True{}),
    u => home.util.ite(Maybe<&2, F32>, Bool.or(U32.is_eq(c, 101), U32.is_eq(c, 69)),
      u => home.util.parse_f32.exp(t, m, dec, any),
      u => None{}))

# [+-]digits after e
def home.util.parse_f32.exp(+s: String, +m: F32, +dec: U32, +any: Bool) -> Maybe<&2, F32>:
  match s:
    case SNil{}:
      None{}
    case SCon{+h, +t}:
      home.util.ite(Maybe<&2, F32>, U32.is_eq(Char.to_u32(h), 45),
        u => home.util.parse_f32.exp.digits(t, m, dec, any, 0, True{}, False{}),
        u => home.util.ite(Maybe<&2, F32>, U32.is_eq(Char.to_u32(h), 43),
          u => home.util.parse_f32.exp.digits(t, m, dec, any, 0, False{}, False{}),
          u => home.util.parse_f32.exp.digits(s, m, dec, any, 0, False{}, False{})))

def home.util.parse_f32.exp.digits(+s: String, +m: F32, +dec: U32, +any: Bool, +e: U32, +neg: Bool, +seen: Bool) -> Maybe<&2, F32>:
  match s:
    case SNil{}:
      home.util.ite(Maybe<&2, F32>, seen, u => home.util.parse_f32.fin(any, m, dec, e, neg), u => None{})
    case SCon{+h, +t}:
      home.util.ite(Maybe<&2, F32>, home.util.c_digit(Char.to_u32(h)),
        u => home.util.parse_f32.exp.digits(t, m, dec, any, U32.min((e * 10 + (Char.to_u32(h) - 48 : U32) : U32), 1000), neg, True{}),
        u => None{})

# mantissa * 10^(e - dec)
def home.util.parse_f32.fin(+any: Bool, +m: F32, +dec: U32, +e: U32, +neg: Bool) -> Maybe<&2, F32>:
  home.util.ite(Maybe<&2, F32>, any,
    u => Some{home.util.pow10(m, dec, e, neg)},
    u => None{})

def home.util.pow10(+m: F32, +dec: U32, +e: U32, +neg: Bool) -> F32:
  home.util.ite(F32, neg,
    u => home.util.scale_down(m, (dec + e : U32)),
    u => home.util.ite(F32, U32.is_ge(e, dec), u => home.util.scale_up(m, (e - dec : U32)), u => home.util.scale_down(m, (dec - e : U32))))

def home.util.scale_up(+m: F32, +n: U32) -> F32:
  home.util.ite(F32, U32.is_eq(n, 0), u => m, u => home.util.scale_up(F32.mul(m, 10.0), (n - 1 : U32)))

def home.util.scale_down(+m: F32, +n: U32) -> F32:
  home.util.ite(F32, U32.is_eq(n, 0), u => m, u => home.util.scale_down(F32.div(m, 10.0), (n - 1 : U32)))

# `parse::<f32>().map_or(default, |v| v.round().max(0.0) as u32)`
def home.util.parse_round(+s: String, +default: U32) -> U32:
  home.util.parse_round.of(home.util.parse_f32(s), default)

def home.util.parse_round.of(+m: Maybe<&2, F32>, +default: U32) -> U32:
  match m:
    case Some{+x}:
      home.util.round_u32(x)
    case None{}:
      default

# x.round().max(0.0) as u32 (half away from zero, saturating)
def home.util.round_u32(+x: F32) -> U32:
  home.util.ite(U32, F32.is_le(x, 0.0), u => 0,
    u => home.util.round_u32.t(F32.trunc(x), x))

def home.util.round_u32.t(+t: F32, +x: F32) -> U32:
  home.util.round_u32.sat(home.util.ite(F32, F32.is_ge(F32.sub(x, t), 0.5), u => F32.add(t, 1.0), u => t))

def home.util.round_u32.sat(+r: F32) -> U32:
  home.util.ite(U32, F32.is_ge(r, 4294967040.0), u => 4294967295, u => F32.to_u32(r))

# an object with no fields
def home.util.obj_empty(+o: J.Val) -> Bool:
  home.util.obj_empty.of(J.fields(o))

def home.util.obj_empty.of(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      False{}

# a sorted copy of strings (byte order), insertion sort
def home.util.sort_str(+xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      home.util.sort_str.ins(h, home.util.sort_str(t))

def home.util.sort_str.ins(+x: String, +xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      [x]
    case Con{+h, +t}:
      home.util.ite(List<&2, String>, String.is_le(x, h), u => x <> (h <> t), u => h <> home.util.sort_str.ins(x, t))

# Vec::dedup on sorted strings
def home.util.dedup_str(+xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      home.util.dedup_str.go(h, t)

def home.util.dedup_str.go(+prev: String, +xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      [prev]
    case Con{+h, +t}:
      home.util.ite(List<&2, String>, String.eq(prev, h), u => home.util.dedup_str.go(prev, t), u => prev <> home.util.dedup_str.go(h, t))

# two string lists are equal
def home.util.eq_strs(+a: List<&2, String>, +b: List<&2, String>) -> Bool:
  match a b:
    case Nil{} Nil{}:
      True{}
    case Con{+x, +xt} Con{+y, +yt}:
      Bool.and(String.eq(x, y), home.util.eq_strs(xt, yt))
    case _ _:
      False{}

# {"Err": [text, ..]}
def home.util.err_list(+xs: List<&2, String>) -> J.Val:
  J.set(J.obj(), "Err", home.util.jstrs(xs))

# an object's keys, in order
def home.util.keys_of(+o: J.Val) -> List<&2, J.Val>:
  home.util.keys_of.go(J.fields(o))

def home.util.keys_of.go(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{(+k, +v), +t}:
      J.JS{k} <> home.util.keys_of.go(t)

# str::trim_matches(|c| c is one of `codes` or white space), both ends
def home.util.trim_set(+s: String, +codes: List<&2, U32>) -> String:
  String.reverse(home.util.trim_set.front(String.reverse(home.util.trim_set.front(s, codes)), codes))

def home.util.trim_set.front(+s: String, +codes: List<&2, U32>) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+h, +t}:
      home.util.ite(String, Bool.or(combos.set_has(codes, Char.to_u32(h)), home.util.c_space(Char.to_u32(h))), u => home.util.trim_set.front(t, codes), u => SCon{h, t})
