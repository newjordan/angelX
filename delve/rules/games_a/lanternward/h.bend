# Lanternward's working helpers (invented, named lanternward.<x>).
#
# The Session `s` is its J object. While a function works on a body that Rust
# hands round as `&mut` (the seat `k`, a foe `f`, the Warden `w`) or collects
# into (`ev`), that body rides inside `s` under a reserved key and is put
# back by the caller that took it out:
#   "$k" the seat being stepped      "$f" the foe being stepped
#   "$w" the Warden being stepped    "$ev" the Events: {sounds, stop, won}
#   "$r" the Rules: {lock, parry, combo, flame, tell}
#   "$ret" a function's returned value when it also changes `s`
# Reads: lanternward.ku/kb/kv (seat), fu/fb/fv (foe), wu/wb/wv (warden).
# Writes: ks (u8/u16/u32), ksi (i32), ksb, ksv (any J value), and fs.., ws...

def lanternward.sl.u(+s: J.Val, +slot: String, +key: String) -> U32:
  ga.u(J.get(s, slot), key)

def lanternward.sl.b(+s: J.Val, +slot: String, +key: String) -> Bool:
  ga.b(J.get(s, slot), key)

def lanternward.sl.v(+s: J.Val, +slot: String, +key: String) -> J.Val:
  J.get(J.get(s, slot), key)

def lanternward.sl.set(+s: J.Val, +slot: String, +key: String, v: J.Val) -> J.Val:
  J.set(s, slot, J.set(J.get(s, slot), key, v))

def lanternward.ku(+s: J.Val, +key: String) -> U32:
  lanternward.sl.u(s, "$k", key)

def lanternward.kb(+s: J.Val, +key: String) -> Bool:
  lanternward.sl.b(s, "$k", key)

def lanternward.kv(+s: J.Val, +key: String) -> J.Val:
  lanternward.sl.v(s, "$k", key)

def lanternward.ks(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$k", key, J.JU{v})

def lanternward.ksi(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$k", key, J.of_i32(v))

def lanternward.ksb(+s: J.Val, +key: String, +v: Bool) -> J.Val:
  lanternward.sl.set(s, "$k", key, J.JBool{v})

def lanternward.ksv(+s: J.Val, +key: String, v: J.Val) -> J.Val:
  lanternward.sl.set(s, "$k", key, v)

def lanternward.fu(+s: J.Val, +key: String) -> U32:
  lanternward.sl.u(s, "$f", key)

def lanternward.fb(+s: J.Val, +key: String) -> Bool:
  lanternward.sl.b(s, "$f", key)

def lanternward.fv(+s: J.Val, +key: String) -> J.Val:
  lanternward.sl.v(s, "$f", key)

def lanternward.fs(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$f", key, J.JU{v})

def lanternward.fsi(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$f", key, J.of_i32(v))

def lanternward.fsb(+s: J.Val, +key: String, +v: Bool) -> J.Val:
  lanternward.sl.set(s, "$f", key, J.JBool{v})

def lanternward.fsv(+s: J.Val, +key: String, v: J.Val) -> J.Val:
  lanternward.sl.set(s, "$f", key, v)

def lanternward.wu(+s: J.Val, +key: String) -> U32:
  lanternward.sl.u(s, "$w", key)

def lanternward.wb(+s: J.Val, +key: String) -> Bool:
  lanternward.sl.b(s, "$w", key)

def lanternward.wv(+s: J.Val, +key: String) -> J.Val:
  lanternward.sl.v(s, "$w", key)

def lanternward.ws(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$w", key, J.JU{v})

def lanternward.wsi(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$w", key, J.of_i32(v))

def lanternward.wsb(+s: J.Val, +key: String, +v: Bool) -> J.Val:
  lanternward.sl.set(s, "$w", key, J.JBool{v})

def lanternward.wsv(+s: J.Val, +key: String, v: J.Val) -> J.Val:
  lanternward.sl.set(s, "$w", key, v)

# Element `i` of the session's array `vec` (props, foes, bolts, drops, fx,
# fuses): reads and writes of one field.
def lanternward.eu(+s: J.Val, +vec: String, +i: U32, +key: String) -> U32:
  ga.u(J.nth(J.get(s, vec), i), key)

def lanternward.eb(+s: J.Val, +vec: String, +i: U32, +key: String) -> Bool:
  ga.b(J.nth(J.get(s, vec), i), key)

def lanternward.ev(+s: J.Val, +vec: String, +i: U32, +key: String) -> J.Val:
  J.get(J.nth(J.get(s, vec), i), key)

def lanternward.el(+s: J.Val, +vec: String, +i: U32) -> J.Val:
  J.nth(J.get(s, vec), i)

def lanternward.eset(+s: J.Val, +vec: String, +i: U32, +key: String, v: J.Val) -> J.Val:
  J.set(s, vec, J.set_nth(J.get(s, vec), i, J.set(J.nth(J.get(s, vec), i), key, v)))

def lanternward.es(+s: J.Val, +vec: String, +i: U32, +key: String, +v: U32) -> J.Val:
  lanternward.eset(s, vec, i, key, J.JU{v})

def lanternward.esi(+s: J.Val, +vec: String, +i: U32, +key: String, +v: U32) -> J.Val:
  lanternward.eset(s, vec, i, key, J.of_i32(v))

def lanternward.esb(+s: J.Val, +vec: String, +i: U32, +key: String, +v: Bool) -> J.Val:
  lanternward.eset(s, vec, i, key, J.JBool{v})

def lanternward.eis(+s: J.Val, +vec: String, +i: U32, +key: String, +name: String) -> Bool:
  String.eq(ga.s(J.nth(J.get(s, vec), i), key), name)

# A whole element replaced.
def lanternward.eput(+s: J.Val, +vec: String, +i: U32, v: J.Val) -> J.Val:
  J.set(s, vec, J.set_nth(J.get(s, vec), i, v))

# Length of the session's array `vec`.
def lanternward.len(+s: J.Val, +vec: String) -> U32:
  J.len(J.get(s, vec))

# `vec.push(v)`.
def lanternward.push(+s: J.Val, +vec: String, v: J.Val) -> J.Val:
  J.set(s, vec, J.push(J.get(s, vec), v))

# The session's `retain` of a list by a list of keep flags would be written per
# site; `lanternward.drop_nth` removes item `i` of the array `vec`.
def lanternward.drop_nth(+s: J.Val, +vec: String, +i: U32) -> J.Val:
  J.set(s, vec, J.JA{ga.remove_nth(J.items(J.get(s, vec)), U32.to_nat(i))})

# Enum values in serde shape: a unit variant is its name; a tuple / struct
# variant is {Name: payload}. `is` compares a field's unit-variant name.
def lanternward.is(+o: J.Val, +key: String, +name: String) -> Bool:
  String.eq(ga.s(o, key), name)

def lanternward.kis(+s: J.Val, +key: String, +name: String) -> Bool:
  String.eq(ga.s(J.get(s, "$k"), key), name)

def lanternward.fis(+s: J.Val, +key: String, +name: String) -> Bool:
  String.eq(ga.s(J.get(s, "$f"), key), name)

def lanternward.wis(+s: J.Val, +key: String, +name: String) -> Bool:
  String.eq(ga.s(J.get(s, "$w"), key), name)

def lanternward.ksn(+s: J.Val, +key: String, +name: String) -> J.Val:
  lanternward.sl.set(s, "$k", key, J.JS{name})

def lanternward.fsn(+s: J.Val, +key: String, +name: String) -> J.Val:
  lanternward.sl.set(s, "$f", key, J.JS{name})

def lanternward.wsn(+s: J.Val, +key: String, +name: String) -> J.Val:
  lanternward.sl.set(s, "$w", key, J.JS{name})

# The variant name of an enum value (the string, or the only key of the
# object), and its payload (null for a unit variant).
def lanternward.tag.first(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> String:
  match fs:
    case Nil{}:
      ""
    case Con{(+k, +v), +t}:
      k

def lanternward.tag(+v: J.Val) -> String:
  match v:
    case J.JS{+n}:
      n
    case J.JO{+m}:
      lanternward.tag.first(J.fields(v))
    case _:
      ""

def lanternward.payload.first(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{(+k, +v), +t}:
      v

def lanternward.payload(+v: J.Val) -> J.Val:
  match v:
    case J.JO{+m}:
      lanternward.payload.first(J.fields(v))
    case _:
      J.JNull{}

def lanternward.tag_is(+v: J.Val, +name: String) -> Bool:
  String.eq(lanternward.tag(v), name)

# `{Name: payload}`.
def lanternward.variant(+name: String, p: J.Val) -> J.Val:
  J.set(J.obj(), name, p)

# The Rules, read from the session.
def lanternward.R(+s: J.Val, +key: String) -> U32:
  ga.u(J.get(s, "$r"), key)

# Events: `sound` appends a name once; `stop` keeps the heaviest.
def lanternward.sounds.has(+xs: List<&2, J.Val>, +name: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(String.eq(J.str(h), name), lanternward.sounds.has(t, name))

def lanternward.sound.add(has: Bool, +s: J.Val, +name: String) -> J.Val:
  match has:
    case True{}:
      s
    case False{}:
      J.set(s, "$ev", J.set(J.get(s, "$ev"), "sounds", J.push(J.get(J.get(s, "$ev"), "sounds"), J.JS{name})))

def lanternward.sound(+s: J.Val, +name: String) -> J.Val:
  lanternward.sound.add(lanternward.sounds.has(ga.items(J.get(s, "$ev"), "sounds"), name), s, name)

# Heavy < Kill < Phase < Boss.
def lanternward.stop.rank(+n: String) -> U32:
  Bool.pick(U32, String.eq(n, "Heavy"), 1,
    Bool.pick(U32, String.eq(n, "Kill"), 2,
      Bool.pick(U32, String.eq(n, "Phase"), 3,
        Bool.pick(U32, String.eq(n, "Boss"), 4, 0))))

def lanternward.stop(+s: J.Val, +name: String) -> J.Val:
  lanternward.stop.set(
    U32.is_gt(lanternward.stop.rank(name), lanternward.stop.rank(ga.s(J.get(s, "$ev"), "stop"))), s, name)

def lanternward.stop.set(up: Bool, +s: J.Val, +name: String) -> J.Val:
  match up:
    case True{}:
      J.set(s, "$ev", J.set(J.get(s, "$ev"), "stop", J.JS{name}))
    case False{}:
      s

def lanternward.won(+s: J.Val) -> J.Val:
  J.set(s, "$ev", J.set(J.get(s, "$ev"), "won", J.JBool{True{}}))

# A fresh Events.
def lanternward.Events.default() -> J.Val:
  J.set(J.set(J.set(J.obj(), "sounds", J.JA{Nil{}}), "stop", J.JNull{}), "won", J.JBool{False{}})

# The session's own fields.
def lanternward.su(+s: J.Val, +key: String, +v: U32) -> J.Val:
  ga.su(s, key, v)

# Signed order and bounds helpers on i32 held in a U32.
def lanternward.in_range(+v: U32, +lo: U32, +hi: U32) -> Bool:
  Bool.and(RS.i_ge(v, lo), RS.i_le(v, hi))

# Trunc-divide by Q (i32 `/ Q`).
def lanternward.divq(+v: U32) -> U32:
  RS.i_div(v, 256)

# `a.min(b)` / `.max(b)` on unsigned values and `a.saturating_sub(b)`.
def lanternward.umin(+a: U32, +b: U32) -> U32:
  RS.u_min(a, b)

def lanternward.umax(+a: U32, +b: U32) -> U32:
  RS.u_max(a, b)

def lanternward.usub(+a: U32, +b: U32) -> U32:
  RS.u_sat_sub(a, b)

# `self.seats.len()`: the fields of the seats object.
def lanternward.seat_count(+s: J.Val) -> U32:
  U32.from_nat(List.length(&2, Sigma<&2, &2, String, _ => J.Val>, J.fields(J.get(s, "seats"))))

# The seat's face component (0 = x, 1 = y).
def lanternward.kface(+s: J.Val, +n: U32) -> U32:
  ga.nu(J.get(J.get(s, "$k"), "face"), n)

# `sound` when the condition holds.
def lanternward.sound_if(yes: Bool, +s: J.Val, +name: String) -> J.Val:
  match yes:
    case True{}:
      lanternward.sound(s, name)
    case False{}:
      s
