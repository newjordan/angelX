# gc.* — small helpers every games_c fragment shares. Not Rust names.

# Field readers: a missing field reads as its Rust Default.
def gc.u(+o: J.Val, +k: String) -> U32:
  J.u32(J.get(o, k))

def gc.f(+o: J.Val, +k: String) -> F32:
  J.f32(J.get(o, k))

def gc.b(+o: J.Val, +k: String) -> Bool:
  J.bool(J.get(o, k))

def gc.s(+o: J.Val, +k: String) -> String:
  J.str(J.get(o, k))

# Item `i` of an array read as a number / float / bool / text.
def gc.nu(+a: J.Val, +i: U32) -> U32:
  J.u32(J.nth(a, i))

def gc.nf(+a: J.Val, +i: U32) -> F32:
  J.f32(J.nth(a, i))

# Field writers.
def gc.su(o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.JU{v})

def gc.si(o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.of_i32(v))

def gc.sf(o: J.Val, +k: String, +v: F32) -> J.Val:
  J.set(o, k, J.JF{v})

def gc.sb(o: J.Val, +k: String, +v: Bool) -> J.Val:
  J.set(o, k, J.JBool{v})

def gc.ss(o: J.Val, +k: String, +v: String) -> J.Val:
  J.set(o, k, J.JS{v})

# `serde(skip_serializing_if = zero)`: a zero is left out; `Not::not`: false is.
def gc.sz.if(zero: Bool, o: J.Val, +k: String, +v: U32) -> J.Val:
  match zero:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, J.JU{v})

def gc.sz(o: J.Val, +k: String, +v: U32) -> J.Val:
  gc.sz.if(U32.is_zero(v), o, k, v)

def gc.sbz.if(off: Bool, o: J.Val, +k: String) -> J.Val:
  match off:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, J.JBool{True{}})

def gc.sbz(o: J.Val, +k: String, +v: Bool) -> J.Val:
  gc.sbz.if(Bool.not(v), o, k)

# Option fields: null.
def gc.snull(o: J.Val, +k: String) -> J.Val:
  J.set(o, k, J.JNull{})

# A two-item array.
def gc.pair(+a: J.Val, +b: J.Val) -> J.Val:
  J.JA{[a, b]}

def gc.pairf(+x: F32, +y: F32) -> J.Val:
  J.JA{[J.JF{x}, J.JF{y}]}

def gc.pairu(+x: U32, +y: U32) -> J.Val:
  J.JA{[J.JU{x}, J.JU{y}]}

# Choose by a flag (both arms are evaluated: keep them cheap and total).
def gc.pu(+c: Bool, +a: U32, +b: U32) -> U32:
  Bool.pick(U32, c, a, b)

def gc.pf(+c: Bool, +a: F32, +b: F32) -> F32:
  Bool.pick(F32, c, a, b)

def gc.pb(+c: Bool, +a: Bool, +b: Bool) -> Bool:
  Bool.pick(Bool, c, a, b)

def gc.pj(+c: Bool, +a: J.Val, +b: J.Val) -> J.Val:
  Bool.pick(J.Val, c, a, b)

def gc.ps(+c: Bool, +a: String, +b: String) -> String:
  Bool.pick(String, c, a, b)

# Booleans.
def gc.not(+a: Bool) -> Bool:
  Bool.not(a)

def gc.and(+a: Bool, +b: Bool) -> Bool:
  Bool.and(a, b)

def gc.or(+a: Bool, +b: Bool) -> Bool:
  Bool.or(a, b)

# Unsigned arithmetic: a.saturating_sub(b), a.saturating_add(b), min, max.
def gc.ssub(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_ge(a, b), (a - b : U32), 0)

def gc.sadd(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_lt((a + b : U32), a), 4294967295, (a + b : U32))

def gc.umin(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_le(a, b), a, b)

def gc.umax(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_ge(a, b), a, b)

# u8 / u16 saturating add and min.
def gc.sadd8(+a: U32, +b: U32) -> U32:
  gc.umin((a + b : U32), 255)

def gc.sadd16(+a: U32, +b: U32) -> U32:
  gc.umin((a + b : U32), 65535)

# Floats: min, max by Rust's NaN rules are RS.f_min / RS.f_max; these two are for
# values known to be finite.
def gc.fmin(+a: F32, +b: F32) -> F32:
  Bool.pick(F32, F32.is_lt(b, a), b, a)

def gc.fmax(+a: F32, +b: F32) -> F32:
  Bool.pick(F32, F32.is_gt(b, a), b, a)

# A decimal object key and back.
def gc.key(+n: U32) -> String:
  U32.show(n)

def gc.id.of(m: Maybe<&2, U32>) -> U32:
  match m:
    case Some{+v}:
      v
    case None{}:
      0

def gc.id(+k: String) -> U32:
  gc.id.of(U32.read(k))

# Seat `id` of an object keyed by decimal id.
def gc.seat(+m: J.Val, +id: U32) -> J.Val:
  J.get(m, gc.key(id))

def gc.has_seat(+m: J.Val, +id: U32) -> Bool:
  J.has(m, gc.key(id))

def gc.put_seat(m: J.Val, +id: U32, v: J.Val) -> J.Val:
  J.set(m, gc.key(id), v)

# The ids of an object keyed by decimal id, in numeric (BTreeMap) order.
def gc.ids.go(xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{(+k, +v), t}:
      gc.id(k) <> gc.ids.go(t)

def gc.ids(+o: J.Val) -> List<&2, U32>:
  gc.ids.go(J.int_fields(o))

# The values of an object keyed by decimal id, in numeric order.
def gc.vals.go(xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{(+k, +v), t}:
      v <> gc.vals.go(t)

def gc.vals(+o: J.Val) -> List<&2, J.Val>:
  gc.vals.go(J.int_fields(o))

# An array of numbers read to a list, and back.
def gc.us.go(xs: List<&2, J.Val>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, t}:
      J.u32(h) <> gc.us.go(t)

def gc.us(+a: J.Val) -> List<&2, U32>:
  gc.us.go(J.items(a))

def gc.of_us(xs: List<&2, U32>) -> J.Val:
  J.JA{gc.of_us.go(xs)}

def gc.of_us.go(xs: List<&2, U32>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, t}:
      J.JU{h} <> gc.of_us.go(t)

# A list's length, as U32.
def gc.len(xs: List<&2, J.Val>) -> U32:
  U32.from_nat(List.length(&2, J.Val, xs))

def gc.len_u(xs: List<&2, U32>) -> U32:
  U32.from_nat(List.length(&2, U32, xs))

# Does the list of numbers contain `x`?
def gc.has_u(+x: U32, +xs: List<&2, U32>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(U32.is_eq(h, x), gc.has_u(x, t))

# The numbers 0..n.
def gc.range.go(+i: U32, +n: U32) -> List<&2, U32>:
  gc.range.if(U32.is_lt(i, n), i, n)

def gc.range.if(more: Bool, +i: U32, +n: U32) -> List<&2, U32>:
  match more:
    case True{}:
      i <> gc.range.go((i + 1 : U32), n)
    case False{}:
      Nil{}

def gc.range(+n: U32) -> List<&2, U32>:
  gc.range.go(0, n)

# Array push of a string / J value onto field `k`.
def gc.push(+o: J.Val, +k: String, v: J.Val) -> J.Val:
  J.set(o, k, J.push(J.get(o, k), v))

def gc.push_s(o: J.Val, +k: String, +s: String) -> J.Val:
  gc.push(o, k, J.JS{s})

# Remove item `i` of an array.
def gc.remove.go(+xs: List<&2, J.Val>, +i: U32) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      gc.pj_list(U32.is_zero(i), t, h <> gc.remove.go(t, (i - 1 : U32)))

def gc.pj_list(+c: Bool, a: List<&2, J.Val>, b: List<&2, J.Val>) -> List<&2, J.Val>:
  match c:
    case True{}:
      a
    case False{}:
      b

def gc.remove(+a: J.Val, +i: U32) -> J.Val:
  J.JA{gc.remove.go(J.items(a), i)}

# The last `n` items of an array (the others dropped).
def gc.drop_front.go(+xs: List<&2, J.Val>, +n: U32) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      gc.pj_list(U32.is_zero(n), h <> t, gc.drop_front.go(t, (n - 1 : U32)))

def gc.drop_front(+a: J.Val, +n: U32) -> J.Val:
  J.JA{gc.drop_front.go(J.items(a), n)}

# The first `n` items.
def gc.take(+a: J.Val, +n: U32) -> J.Val:
  J.truncate(a, n)

# Rust's `x as f32 / y` style conversions come from RS; this is u32 -> f32.
def gc.uf(+v: U32) -> F32:
  U32.to_f32(v)

# self.sounds.push(name), self.cues.push(name).
def gc.sound(run: J.Val, +name: String) -> J.Val:
  gc.push_s(run, "sounds", name)

def gc.cue(run: J.Val, +name: String) -> J.Val:
  gc.push_s(run, "cues", name)

# self.found = Some((tick, who, text)).
def gc.found(+run: J.Val, +who: U32, +text: String) -> J.Val:
  J.set(run, "found", J.JA{[J.get(run, "tick"), J.JU{who}, J.JS{text}]})

# marks.insert(key, v) and marks.contains_key(key).
def gc.mark(+run: J.Val, +key: String, +v: U32) -> J.Val:
  J.set(run, "marks", J.set(J.get(run, "marks"), key, J.JU{v}))

def gc.marked(+run: J.Val, +key: String) -> Bool:
  J.has(J.get(run, "marks"), key)

# String equality as a Bool.
def gc.eq(+a: String, +b: String) -> Bool:
  String.eq(a, b)

# Table lookups: item `i` of a list (the last item stands for any index past the end of a table of one).
def gc.nth_s.go(+xs: List<&2, String>, +i: U32) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+h, +t}:
      gc.nth_s.if(U32.is_zero(i), h, t, i)

def gc.nth_s.if(+here: Bool, +h: String, +t: List<&2, String>, +i: U32) -> String:
  match here:
    case True{}:
      h
    case False{}:
      gc.nth_s.go(t, (i - 1 : U32))

def gc.nth_s(+xs: List<&2, String>, +i: U32) -> String:
  gc.nth_s.go(xs, i)

def gc.nth_u.go(+xs: List<&2, U32>, +i: U32) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, +t}:
      gc.nth_u.if(U32.is_zero(i), h, t, i)

def gc.nth_u.if(+here: Bool, +h: U32, +t: List<&2, U32>, +i: U32) -> U32:
  match here:
    case True{}:
      h
    case False{}:
      gc.nth_u.go(t, (i - 1 : U32))

def gc.nth_u(+xs: List<&2, U32>, +i: U32) -> U32:
  gc.nth_u.go(xs, i)

def gc.nth_f.go(+xs: List<&2, F32>, +i: U32) -> F32:
  match xs:
    case Nil{}:
      0.0
    case Con{+h, +t}:
      gc.nth_f.if(U32.is_zero(i), h, t, i)

def gc.nth_f.if(+here: Bool, +h: F32, +t: List<&2, F32>, +i: U32) -> F32:
  match here:
    case True{}:
      h
    case False{}:
      gc.nth_f.go(t, (i - 1 : U32))

def gc.nth_f(+xs: List<&2, F32>, +i: U32) -> F32:
  gc.nth_f.go(xs, i)

# 1 << i, a & (1 << i) != 0
def gc.bit(+i: U32) -> U32:
  U32.shln(1, U32.to_nat(i))

def gc.has_bit(+mask: U32, +i: U32) -> Bool:
  U32.is_ne(U32.and(mask, U32.shln(1, U32.to_nat(i))), 0)

# A lazy `if`: only the chosen thunk runs (Bool.pick runs both arms).
def gc.if(-A: Type, c: Bool, a: Unit -> A, b: Unit -> A) -> A:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def gc.ifj(c: Bool, a: Unit -> J.Val, b: Unit -> J.Val) -> J.Val:
  gc.if(J.Val, c, a, b)

def gc.ifu(c: Bool, a: Unit -> U32, b: Unit -> U32) -> U32:
  gc.if(U32, c, a, b)

def gc.iff(c: Bool, a: Unit -> F32, b: Unit -> F32) -> F32:
  gc.if(F32, c, a, b)

def gc.ifb(c: Bool, a: Unit -> Bool, b: Unit -> Bool) -> Bool:
  gc.if(Bool, c, a, b)

def gc.ifs(c: Bool, a: Unit -> String, b: Unit -> String) -> String:
  gc.if(String, c, a, b)

# knight `id`'s input for the tick: a valid one, else Input::default().
def gc.input(+inputs: J.Val, +id: U32) -> J.Val:
  gc.input.of(gc.seat(inputs, id))

def gc.input.of(+i: J.Val) -> J.Val:
  gc.ifj(Bool.and(Bool.not(J.is_null(i)), rules.Input.valid(i)), _ => i, _ => rules.Input.default())

# inputs.get(&id).is_some_and(|i| i.valid() && i.interact)
def gc.pressed_interact(+inputs: J.Val, +id: U32) -> Bool:
  gc.pressed_interact.of(gc.seat(inputs, id))

def gc.pressed_interact.of(+i: J.Val) -> Bool:
  gc.ifb(J.is_null(i), _ => False{}, _ => Bool.and(rules.Input.valid(i), gc.b(i, "interact")))

# An i8 axis is negative when its two's complement top bit is set.
def gc.neg(+v: U32) -> Bool:
  U32.is_ge(v, 2147483648)

# The tick as a u64.
def gc.tick(+run: J.Val) -> W.W:
  W.of_j(J.get(run, "tick"))

# Option<u32>: a number or null.
def gc.opt_u(+v: J.Val) -> U32:
  J.u32(v)

# Option<u32> equals Some(id).
def gc.opt_is(+v: J.Val, +id: U32) -> Bool:
  Bool.and(Bool.not(J.is_null(v)), U32.is_eq(J.u32(v), id))

def gc.ifj_us(c: Bool, a: Unit -> List<&2, U32>, b: Unit -> List<&2, U32>) -> List<&2, U32>:
  gc.if(List<&2, U32>, c, a, b)

# The ids of `players` not in `outside` (an object keyed by id), in order.
def gc.ids_outside.go(+ids: List<&2, U32>, +outside: J.Val) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gc.ifx_us(Bool.not(gc.has_seat(outside, id)), id, gc.ids_outside.go(t, outside))

def gc.ids_outside(+players: J.Val, +outside: J.Val) -> List<&2, U32>:
  gc.ids_outside.go(gc.ids(players), outside)

# The values of any object (key order), for maps whose keys are not ids.
def gc.vals_any.go(xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{(+k, +v), t}:
      v <> gc.vals_any.go(t)

def gc.vals_any(+o: J.Val) -> List<&2, J.Val>:
  gc.vals_any.go(J.fields(o))

# pastimes::Score is left out of its Records while empty (skip_serializing_if = Score::is_empty):
# the score named `name` of a Records object, and a Records with it set (or left out when empty).
def gc.score_put(+games: J.Val, +name: String, +score: J.Val) -> J.Val:
  gc.ifj(Bool.and(U32.is_zero(gc.u(score, "best")), Bool.and(U32.is_zero(gc.u(score, "wins")), U32.is_zero(gc.u(score, "plays")))),
    _ => J.del(games, name),
    _ => J.set(games, name, score))

# Every named field set to 0.
def gc.zeros(+o: J.Val, +names: List<&2, String>) -> J.Val:
  match names:
    case Nil{}:
      o
    case Con{+h, +t}:
      gc.zeros(J.set(o, h, J.JU{0}), t)

def gc.falses(+o: J.Val, +names: List<&2, String>) -> J.Val:
  match names:
    case Nil{}:
      o
    case Con{+h, +t}:
      gc.falses(J.set(o, h, J.JBool{False{}}), t)

# Option<u64> from its J value (null is None).
def gc.maybe_w(+j: J.Val) -> Maybe<&2, W.W>:
  gc.maybe_w.if(J.is_null(j), j)

def gc.maybe_w.if(none: Bool, +j: J.Val) -> Maybe<&2, W.W>:
  match none:
    case True{}:
      None{}
    case False{}:
      Some{W.of_j(j)}

# A sound ("" for none) as the case's `ret`: a string, or null.
def gc.ret_s(+s: String) -> J.Val:
  gc.ifj(String.is_empty(s), _ => J.JNull{}, _ => J.JS{s})

# i64 over W.W (two's complement): the few signed 64-bit operations the integer physics uses.
def gc.l_of(+v: U32) -> W.W:
  gc.ifw(U32.is_ge(v, 2147483648), W.W{4294967295, v}, W.of_u32(v))

def gc.l_neg_p(+a: W.W) -> Bool:
  match a:
    case W.W{+hi, +lo}:
      U32.is_ge(hi, 2147483648)

def gc.l_abs(+a: W.W) -> W.W:
  gc.ifw(gc.l_neg_p(a), W.neg(a), a)

# a < b, signed
def gc.l_lt(+a: W.W, +b: W.W) -> Bool:
  Bool.pick(Bool, Bool.and(gc.l_neg_p(a), Bool.not(gc.l_neg_p(b))), True{}, Bool.pick(Bool, Bool.and(Bool.not(gc.l_neg_p(a)), gc.l_neg_p(b)), False{}, W.lt(a, b)))

# a / b truncating toward zero (b != 0)
def gc.l_div(+a: W.W, +b: W.W) -> W.W:
  +q = W.div(gc.l_abs(a), gc.l_abs(b))
  gc.ifw(Bool.not(Bool.pick(Bool, gc.l_neg_p(a), gc.l_neg_p(b), Bool.not(gc.l_neg_p(b)))), W.neg(q), q)

def gc.l_clamp(+a: W.W, +lo: W.W, +hi: W.W) -> W.W:
  gc.ifw(gc.l_lt(a, lo), lo, gc.ifw(gc.l_lt(hi, a), hi, a))

# A pair of i32 values.
def gc.pairi(+x: U32, +y: U32) -> J.Val:
  J.JA{[J.of_i32(x), J.of_i32(y)]}

# h <> t over J values.
def gc.cons(+h: J.Val, +t: List<&2, J.Val>) -> List<&2, J.Val>:
  h <> t

# A sorted set of numbers as a J array: contains, insert.
def gc.set_has(+arr: J.Val, +v: U32) -> Bool:
  gc.has_u(v, gc.us(arr))

def gc.set_insert.go(+xs: List<&2, U32>, +v: U32) -> List<&2, U32>:
  match xs:
    case Nil{}:
      [v]
    case Con{+h, +t}:
      gc.set_insert.if(U32.is_eq(h, v), U32.is_lt(v, h), h, t, v)

def gc.set_insert.if(+same: Bool, +before: Bool, +h: U32, +t: List<&2, U32>, +v: U32) -> List<&2, U32>:
  match same:
    case True{}:
      h <> t
    case False{}:
      gc.set_insert.if2(before, h, t, v)

def gc.set_insert.if2(+before: Bool, +h: U32, +t: List<&2, U32>, +v: U32) -> List<&2, U32>:
  match before:
    case True{}:
      v <> (h <> t)
    case False{}:
      h <> gc.set_insert.go(t, v)

def gc.set_insert(+arr: J.Val, +v: U32) -> J.Val:
  gc.of_us(gc.set_insert.go(gc.us(arr), v))
