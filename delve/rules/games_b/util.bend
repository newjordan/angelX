# Small helpers every games_b fragment shares. Not Rust names: `gb.*`.

# Field readers: a missing field reads as its Rust Default.
def gb.u(+o: J.Val, +k: String) -> U32:
  J.u32(J.get(o, k))

def gb.f(+o: J.Val, +k: String) -> F32:
  J.f32(J.get(o, k))

def gb.b(+o: J.Val, +k: String) -> Bool:
  J.bool(J.get(o, k))

def gb.s(+o: J.Val, +k: String) -> String:
  J.str(J.get(o, k))

# Field writers.
def gb.su(o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.JU{v})

def gb.sf(o: J.Val, +k: String, +v: F32) -> J.Val:
  J.set(o, k, J.JF{v})

def gb.sb(o: J.Val, +k: String, +v: Bool) -> J.Val:
  J.set(o, k, J.JBool{v})

def gb.ss(o: J.Val, +k: String, +v: String) -> J.Val:
  J.set(o, k, J.JS{v})

def gb.si(o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.of_i32(v))

# Choose by a flag (both arms evaluated: keep them cheap and total).
def gb.pu(+c: Bool, +a: U32, +b: U32) -> U32:
  Bool.pick(U32, c, a, b)

def gb.pf(+c: Bool, +a: F32, +b: F32) -> F32:
  Bool.pick(F32, c, a, b)

def gb.pb(+c: Bool, +a: Bool, +b: Bool) -> Bool:
  Bool.pick(Bool, c, a, b)

# A decimal object key.
def gb.key(+n: U32) -> String:
  U32.show(n)

# Seat `id` of a map keyed by decimal id.
def gb.seat(+m: J.Val, +id: U32) -> J.Val:
  J.get(m, gb.key(id))

# Push a string onto an array field.
def gb.push_s(+o: J.Val, +k: String, +s: String) -> J.Val:
  J.set(o, k, J.push(J.get(o, k), J.JS{s}))

# u8 saturating add of two bytes.
def gb.sat8(+a: U32, +b: U32) -> U32:
  U32.min((a + b : U32), 255)

# a.saturating_sub(b) on unsigned.
def gb.ssub(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_ge(a, b), (a - b : U32), 0)

# a.saturating_add(b) on u32.
def gb.sadd(+a: U32, +b: U32) -> U32:
  gb.sadd.go((a + b : U32), a)

def gb.sadd.go(+s: U32, +a: U32) -> U32:
  Bool.pick(U32, U32.is_lt(s, a), 4294967295, s)

# The run's tick (u64) and writes that need it.
def gb.tick(+run: J.Val) -> W.W:
  W.of_j(J.get(run, "tick"))

def gb.found(+run: J.Val, +id: U32, +text: String) -> J.Val:
  J.set(run, "found", J.JA{[W.to_j(gb.tick(run)), J.JU{id}, J.JS{text}]})

def gb.sound(+run: J.Val, +name: String) -> J.Val:
  gb.push_s(run, "sounds", name)

def gb.cue(+run: J.Val, +name: String) -> J.Val:
  gb.push_s(run, "cues", name)

# n.count_ones() for a value under 2^32.
def gb.popcount(+n: U32) -> U32:
  gb.popcount.go(n, 32n)

def gb.popcount.go(+n: U32, k: Nat) -> U32:
  match k:
    case 0n:
      0
    case 1n+p:
      (U32.and(n, 1) + gb.popcount.go(U32.shrn(n, 1n), p) : U32)

# Bit i of n.
def gb.bit(+n: U32, +i: U32) -> Bool:
  U32.is_ne(U32.and(n, U32.shln(1, U32.to_nat(i))), 0)

# The items of a Run's `players` map, by seat id order.
def gb.hero_list(+run: J.Val) -> List<&2, Sigma<&2, &2, String, _ => J.Val>>:
  J.int_fields(J.get(run, "players"))

# A BTreeSet<u32> in the state: a sorted array of numbers.
def gb.set_has(+a: J.Val, +id: U32) -> Bool:
  gb.set_has.go(J.items(a), id)

def gb.set_has.go(+xs: List<&2, J.Val>, +id: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(U32.is_eq(J.u32(h), id), gb.set_has.go(t, id))

def gb.set_ins(+a: J.Val, +id: U32) -> J.Val:
  J.JA{gb.set_ins.go(J.items(a), id)}

def gb.set_ins.go(+xs: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      [J.JU{id}]
    case Con{+h, +t}:
      gb.set_ins.if(U32.cmp(id, J.u32(h)), id, h, t)

def gb.set_ins.if(c: Cmp, +id: U32, +h: J.Val, +t: List<&2, J.Val>) -> List<&2, J.Val>:
  match c:
    case LT{}:
      J.JU{id} <> (h <> t)
    case EQ{}:
      h <> t
    case GT{}:
      h <> gb.set_ins.go(t, id)

def gb.set_rm(+a: J.Val, +id: U32) -> J.Val:
  J.JA{gb.set_rm.go(J.items(a), id)}

def gb.set_rm.go(+xs: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      []
    case Con{+h, +t}:
      gb.set_rm.if(U32.is_eq(J.u32(h), id), h, t, id)

def gb.set_rm.if(same: Bool, +h: J.Val, +t: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match same:
    case True{}:
      t
    case False{}:
      h <> gb.set_rm.go(t, id)

# An Option<(f32, f32)> in the state: [x, y] or null.
def gb.pair(+x: F32, +y: F32) -> J.Val:
  J.JA{[J.JF{x}, J.JF{y}]}

# The item `n` of a list taken out the way Vec::swap_remove does: the last
# item takes its place.
def gb.swap_remove(+xs: List<&2, J.Val>, +n: U32) -> List<&2, J.Val>:
  gb.swap_remove.set(xs, n, gb.last(xs))

def gb.last(+xs: List<&2, J.Val>) -> J.Val:
  match xs:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      gb.last.go(h, t)

def gb.last.go(+h: J.Val, +t: List<&2, J.Val>) -> J.Val:
  match t:
    case Nil{}:
      h
    case Con{+h2, +t2}:
      gb.last.go(h2, t2)

# xs with item n replaced by `last`, and the final item dropped.
def gb.swap_remove.set(+xs: List<&2, J.Val>, +n: U32, +last: J.Val) -> List<&2, J.Val>:
  gb.drop_last(gb.replace_nth(xs, n, last))

def gb.replace_nth(+xs: List<&2, J.Val>, +n: U32, +v: J.Val) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      []
    case Con{+h, +t}:
      gb.replace_nth.if(U32.is_eq(n, 0), h, t, n, v)

def gb.replace_nth.if(here: Bool, +h: J.Val, +t: List<&2, J.Val>, +n: U32, +v: J.Val) -> List<&2, J.Val>:
  match here:
    case True{}:
      v <> t
    case False{}:
      h <> gb.replace_nth(t, (n - 1 : U32), v)

def gb.drop_last(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      []
    case Con{+h, +t}:
      gb.drop_last.go(h, t)

def gb.drop_last.go(+h: J.Val, +t: List<&2, J.Val>) -> List<&2, J.Val>:
  match t:
    case Nil{}:
      []
    case Con{+h2, +t2}:
      h <> gb.drop_last.go(h2, t2)

# xs with item n replaced by f(item) isn't a thing in Bend 2 without a
# closure per call site; `gb.map_nth` takes the replacement.
def gb.set_in(+xs: List<&2, J.Val>, +n: U32, +v: J.Val) -> List<&2, J.Val>:
  gb.replace_nth(xs, n, v)

# Writers for u64 fields, and a decimal object key read back as a number.
def gb.sw(+o: J.Val, +k: String, +w: W.W) -> J.Val:
  J.set(o, k, W.to_j(w))

def gb.id(+k: String) -> U32:
  Maybe.default(&2, U32, U32.read(k), 0)

# The Run's knights in seat order, each with its seat id in `$id` (never
# written back: `gb.bare` takes it off).
def gb.heroes(+run: J.Val) -> List<&2, J.Val>:
  gb.heroes.go(gb.hero_list(run))

def gb.heroes.go(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> List<&2, J.Val>:
  match hs:
    case Nil{}:
      []
    case Con{(+k, +h), +t}:
      J.set(h, "$id", J.JU{gb.id(k)}) <> gb.heroes.go(t)

def gb.bare(+h: J.Val) -> J.Val:
  J.del(h, "$id")

def gb.hid(+h: J.Val) -> U32:
  gb.u(h, "$id")

# The first knight standing (hp > 0, not stone), or null.
def gb.standing(+hs: List<&2, J.Val>) -> J.Val:
  match hs:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      gb.standing.if(Bool.and(U32.is_gt(gb.u(h, "hp"), 0), Bool.not(gb.b(h, "stone"))), h, t)

def gb.standing.if(up: Bool, +h: J.Val, +t: List<&2, J.Val>) -> J.Val:
  match up:
    case True{}:
      h
    case False{}:
      gb.standing(t)

# The first knight at all, or null.
def gb.first(+hs: List<&2, J.Val>) -> J.Val:
  match hs:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      h

# `self.dungeon.rooms[self.at] = room`.
def gb.set_room(+run: J.Val, +room: J.Val) -> J.Val:
  gb.set_room.go(run, room, J.get(run, "dungeon"))

def gb.set_room.go(+run: J.Val, +room: J.Val, +dungeon: J.Val) -> J.Val:
  J.set(run, "dungeon", J.set(dungeon, "rooms", J.set_nth(J.get(dungeon, "rooms"), gb.u(run, "at"), room)))

# `serde(skip_serializing_if = "BTreeMap::is_empty")` (or Vec): an empty
# object or array leaves the key out.
def gb.put_nonempty(+o: J.Val, +k: String, +v: J.Val) -> J.Val:
  gb.put_nonempty.if(gb.is_empty(v), o, k, v)

def gb.put_nonempty.if(empty: Bool, +o: J.Val, +k: String, +v: J.Val) -> J.Val:
  match empty:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, v)

def gb.is_empty(+v: J.Val) -> Bool:
  match v:
    case J.JO{+m}:
      List.is_empty(&2, Sigma<&2, &2, String, _ => J.Val>, Map.to_list(&2, J.Val, m))
    case J.JA{+xs}:
      List.is_empty(&2, J.Val, xs)
    case J.JNull{}:
      True{}
    case _:
      False{}

# The text up to the first ':' (all of it when there is none).
def gb.head(+s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+c, +t}:
      gb.head.if(Char.is_eq(c, ':'), c, t)

def gb.head.if(stop: Bool, +c: Char, +t: String) -> String:
  match stop:
    case True{}:
      ""
    case False{}:
      SCon{c, gb.head(t)}

# The elements of an array from index `from` on (none past the end).
def gb.from(+a: J.Val, +from: U32) -> List<&2, J.Val>:
  List.drop(&2, J.Val, J.items(a), U32.to_nat(from))

# How many of the strings in an array satisfy a test on their head.
def gb.heads_eq(+xs: List<&2, J.Val>, +name: String) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+x, +t}:
      (Bool.to_u32(String.eq(gb.head(J.str(x)), name)) + gb.heads_eq(t, name) : U32)

def gb.any_eq(+xs: List<&2, J.Val>, +name: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+x, +t}:
      Bool.or(String.eq(J.str(x), name), gb.any_eq(t, name))

def gb.any_head(+xs: List<&2, J.Val>, +name: String) -> Bool:
  Bool.not(U32.is_eq(gb.heads_eq(xs, name), 0))

# A field read as the value itself (an object / array / null).
def gb.field(+o: J.Val, +k: String) -> J.Val:
  J.get(o, k)

# i32 field (two's complement U32); the helpers for fixed arrays in the state.
def gb.i(+o: J.Val, +k: String) -> U32:
  J.i32(J.get(o, k))

def gb.ag(+a: J.Val, +i: U32) -> U32:
  J.u32(J.nth(a, i))

def gb.as(+a: J.Val, +i: U32, +v: U32) -> J.Val:
  J.set_nth(a, i, J.JU{v})

def gb.ais(+a: J.Val, +i: U32, +v: U32) -> J.Val:
  J.set_nth(a, i, J.of_i32(v))

# Every element of an array saturating_sub(1).
def gb.sat1(+a: J.Val) -> J.Val:
  J.JA{gb.sat1.go(J.items(a))}

def gb.sat1.go(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      []
    case Con{+h, +t}:
      J.JU{gb.ssub(J.u32(h), 1)} <> gb.sat1.go(t)

# `if a != 0 { a } else { b }` over two i8 inputs, as an i32.
def gb.axis(+input: J.Val, +a: String, +b: String) -> U32:
  Bool.pick(U32, U32.is_ne(gb.i(input, a), 0), gb.i(input, a), gb.i(input, b))

# a ? x : y over i32s.
def gb.pi(+c: Bool, +a: U32, +b: U32) -> U32:
  Bool.pick(U32, c, a, b)

def gb.min1(+a: U32, +b: U32) -> U32:
  U32.min(a, b)

# An object's fields by numeric key as a list of {k, v} objects (for the
# bx-written code, which cannot name the pair type).
def gb.entries(+o: J.Val) -> List<&2, J.Val>:
  gb.entries.go(J.int_fields(o))

def gb.entries.go(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      []
    case Con{(+k, +v), +t}:
      J.set(J.set(J.obj(), "k", J.JS{k}), "v", v) <> gb.entries.go(t)

# s.to_uppercase() for ASCII text.
def gb.upper(+s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+c, +t}:
      SCon{Char.to_upper(c), gb.upper(t)}

# How many fields an object has.
def gb.field_len(+o: J.Val) -> U32:
  U32.from_nat(List.length(&2, Sigma<&2, &2, String, _ => J.Val>, J.fields(o)))
