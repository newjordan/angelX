import Base
import ../../lib/j.bend as J
import ../../lib/bj.bend as B
import ../../lib/delta.bend as D
import ../../lib/w64.bend as W
import ../../lib/rs.bend as RS

def ga.v?(+o: J.Val, +k: String) -> J.Val:
  J.get(o, k)

def ga.u?(+o: J.Val, +k: String) -> U32:
  J.u32(J.get(o, k))

def ga.f?(+o: J.Val, +k: String) -> F32:
  J.f32(J.get(o, k))

def ga.b?(+o: J.Val, +k: String) -> Bool:
  J.bool(J.get(o, k))

def ga.s?(+o: J.Val, +k: String) -> String:
  J.str(J.get(o, k))

# Item `i` of array `a`.

def ga.nu?(+a: J.Val, +i: U32) -> U32:
  J.u32(J.nth(a, i))

def ga.nf?(+a: J.Val, +i: U32) -> F32:
  J.f32(J.nth(a, i))

# Writers: field `k` of `o` set to a u8/u16/u32, an i32, an f32, a bool,
# a string, any J value.

def ga.su?(o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.JU{v})

def ga.si?(o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.of_i32(v))

def ga.sf?(o: J.Val, +k: String, +v: F32) -> J.Val:
  J.set(o, k, J.JF{v})

def ga.sb?(o: J.Val, +k: String, +v: Bool) -> J.Val:
  J.set(o, k, J.JBool{v})

def ga.ss?(o: J.Val, +k: String, +v: String) -> J.Val:
  J.set(o, k, J.JS{v})

def ga.sv?(o: J.Val, +k: String, v: J.Val) -> J.Val:
  J.set(o, k, v)

# `#[serde(skip_serializing_if = ...)]`: a zero / false / null value is left
# out of the object.

def ga.su_skip.if?(zero: Bool, o: J.Val, +k: String, +v: U32) -> J.Val:
  match zero:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, J.JU{v})

def ga.su_skip?(o: J.Val, +k: String, +v: U32) -> J.Val:
  ga.su_skip.if(U32.is_zero(v), o, k, v)

def ga.sb_skip.if?(off: Bool, o: J.Val, +k: String) -> J.Val:
  match off:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, J.JBool{True{}})

def ga.sb_skip?(o: J.Val, +k: String, +v: Bool) -> J.Val:
  ga.sb_skip.if(Bool.not(v), o, k)

def ga.sv_skip.if?(none: Bool, o: J.Val, +k: String, v: J.Val) -> J.Val:
  match none:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, v)

def ga.sv_skip?(o: J.Val, +k: String, +v: J.Val) -> J.Val:
  ga.sv_skip.if(J.is_null(v), o, k, v)

# Pickers (both arms are evaluated: keep them cheap and total).

def ga.pu?(+c: Bool, +a: U32, +b: U32) -> U32:
  Bool.pick(U32, c, a, b)

def ga.pf?(+c: Bool, +a: F32, +b: F32) -> F32:
  Bool.pick(F32, c, a, b)

def ga.pb?(+c: Bool, +a: Bool, +b: Bool) -> Bool:
  Bool.pick(Bool, c, a, b)

def ga.pv?(+c: Bool, a: J.Val, b: J.Val) -> J.Val:
  Bool.pick(J.Val, c, a, b)

# A decimal object key, and entry `id` of an object keyed by decimal id
# (a BTreeMap<u32, _>).

def ga.key?(+n: U32) -> String:
  U32.show(n)

def ga.at?(+m: J.Val, +id: U32) -> J.Val:
  J.get(m, U32.show(id))

def ga.put_at?(m: J.Val, +id: U32, v: J.Val) -> J.Val:
  J.set(m, U32.show(id), v)

# Whether an object has the decimal key.

def ga.has_key?(+m: J.Val, +id: U32) -> Bool:
  J.has(m, U32.show(id))

# The ids of an object keyed by decimal id, ascending (BTreeMap order).

def ga.ids.go?(xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{(+k, +v), t}:
      Con{Maybe.default(&2, U32, U32.read(k), 0), ga.ids.go(t)}

def ga.ids?(+m: J.Val) -> List<&2, U32>:
  ga.ids.go(J.int_fields(m))

# A two-item array / three-item array.

def ga.pair?(a: J.Val, b: J.Val) -> J.Val:
  J.JA{[a, b]}

def ga.pair_f?(+x: F32, +y: F32) -> J.Val:
  J.JA{[J.JF{x}, J.JF{y}]}

def ga.pair_i?(+x: U32, +y: U32) -> J.Val:
  J.JA{[J.of_i32(x), J.of_i32(y)]}

def ga.trio?(a: J.Val, b: J.Val, c: J.Val) -> J.Val:
  J.JA{[a, b, c]}

# a.saturating_sub(b) on unsigned.

def ga.ssub?(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_ge(a, b), (a - b : U32), 0)

# a.saturating_add(b) on u8 / u16 / u32.

def ga.sadd8?(+a: U32, +b: U32) -> U32:
  U32.min((a + b : U32), 255)

def ga.sadd16?(+a: U32, +b: U32) -> U32:
  U32.min((a + b : U32), 65535)

def ga.sadd32?(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_lt((a + b : U32), a), 4294967295, (a + b : U32))

# Array helpers over the J array of an object field.

def ga.items?(+o: J.Val, +k: String) -> List<&2, J.Val>:
  J.items(J.get(o, k))

def ga.push?(+o: J.Val, +k: String, v: J.Val) -> J.Val:
  J.set(o, k, J.push(J.get(o, k), v))

# `Vec::remove(i)` / `Vec::retain` by position: drop item `i` of a list.

def ga.remove_nth?(+xs: List<&2, J.Val>, +n: Nat) -> List<&2, J.Val>:
  match xs n:
    case Nil{} _:
      Nil{}
    case Con{h, t} 0n:
      t
    case Con{h, t} 1n+p:
      Con{h, ga.remove_nth(t, p)}

# Number of items of a list as a U32.

def ga.count?(+xs: List<&2, J.Val>) -> U32:
  U32.from_nat(List.length(&2, J.Val, xs))

# `s` is the J string `name` (a unit enum variant in serde shape).

def ga.is?(+s: J.Val, +name: String) -> Bool:
  String.eq(J.str(s), name)

# A tile name (`Tile`, serde snake_case) is one of the stones that stop a body.

def ga.stone?(+t: J.Val) -> Bool:
  Bool.or(ga.is(t, "wall"), ga.is(t, "block"))

# `a.max(b)` / `a.min(b)` on u32 and the Option's `unwrap_or`.

def ga.umax?(+a: U32, +b: U32) -> U32:
  U32.max(a, b)

def ga.umin?(+a: U32, +b: U32) -> U32:
  U32.min(a, b)

# The object with only the keys `ks` of `o` (those it has), for comparing the
# part of a run a function changes.

def ga.pick?(+o: J.Val, +ks: List<&2, String>) -> J.Val:
  ga.pick.go(o, ks, J.obj())

def ga.pick.go?(+o: J.Val, +ks: List<&2, String>, +acc: J.Val) -> J.Val:
  match ks:
    case Nil{}:
      acc
    case Con{+k, +t}:
      ga.pick.go(o, t, ga.pick.put(J.has(o, k), o, k, acc))

def ga.pick.put?(has: Bool, +o: J.Val, +k: String, +acc: J.Val) -> J.Val:
  match has:
    case True{}:
      J.set(acc, k, J.get(o, k))
    case False{}:
      acc

# {room, barred}: a `Grid`.

def ga.grid?(+room: J.Val, +barred: Bool) -> J.Val:
  ga.sv(ga.sb(J.obj(), "barred", barred), "room", room)

# A J array of [x, y] pairs as a list of Bend pairs.

def ga.xys?(+a: J.Val) -> List<&2, Sigma<&2, &2, F32, _ => F32>>:
  ga.xys.go(J.items(a))

def ga.xys.go?(+xs: List<&2, J.Val>) -> List<&2, Sigma<&2, &2, F32, _ => F32>>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{(J.f32(J.nth(h, 0)), J.f32(J.nth(h, 1))), ga.xys.go(t)}

def ga.ps?(+c: Bool, +a: String, +b: String) -> String:
  Bool.pick(String, c, a, b)

# The number of fields of an object.

def ga.nfields?(+o: J.Val) -> U32:
  U32.from_nat(List.length(&2, Sigma<&2, &2, String, _ => J.Val>, J.fields(o)))

def lanternward.blow.Session.blow?(+s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32, +kind: String) -> J.Val:
  lanternward.blow.Session.blow.look(ga.has_key(J.get(s, "seats"), id), s, id, dmg, fx, fy, kind)

def lanternward.blow.Session.blow.look?(has: Bool, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32, +kind: String) -> J.Val:
  match has:
    case False{}:
      ga.ss(s, "$ret", "Dodged")
    case True{}:
      lanternward.blow.Session.blow.gate(ga.at(J.get(s, "seats"), id), s, id, dmg, fx, fy, kind)

def lanternward.blow.Session.blow.gate?(+k: J.Val, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32, +kind: String) -> J.Val:
  lanternward.blow.Session.blow.gate.go(
    Bool.or(
      Bool.or(Bool.not(lanternward.kit.Seat.up(k)), U32.is_gt(ga.u(k, "inv"), 0)),
      Bool.or(
        Bool.or(lanternward.is(k, "act", "Item"), lanternward.is(k, "act", "Fall")),
        Bool.and(String.eq(kind, "Low"), RS.i_ge(ga.u(k, "z"), 1280)))),
    k, s, id, dmg, fx, fy)

def lanternward.blow.Session.blow.gate.go?(dodged: Bool, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  match dodged:
    case True{}:
      ga.ss(s, "$ret", "Dodged")
    case False{}:
      lanternward.blow.Session.blow.aim(
        lanternward.world.unit((fx - ga.u(k, "x") : U32), (fy - ga.u(k, "y") : U32)), k, s, id, dmg, fx, fy)

def lanternward.blow.Session.blow.aim?(toward: U32 & U32, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  (+tx, +ty) = toward
  lanternward.blow.Session.blow.guard(
    Bool.and(
      Bool.and(U32.is_gt(ga.u(k, "shield"), 0), lanternward.is(k, "act", "Free")),
      RS.i_ge(lanternward.world.dot4(ga.nu(ga.v(k, "face"), 0), ga.nu(ga.v(k, "face"), 1), tx, ty), 108)),
    tx, ty, k, s, id, dmg, fx, fy)

def lanternward.blow.Session.blow.guard?(guarded: Bool, +tx: U32, +ty: U32, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  match guarded:
    case True{}:
      lanternward.blow.Session.blow.guarded(U32.is_le(ga.u(k, "shield"), lanternward.R(s, "parry")), tx, ty, k, s, id)
    case False{}:
      lanternward.blow.Session.blow.hit(k, s, id, dmg, fx, fy)

def lanternward.blow.put_seat?(+s: J.Val, +id: U32, k: J.Val) -> J.Val:
  ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id, k))

# The shield met it: a parry freezes the knight and flashes; a late block
# pushes the knight back.

def lanternward.blow.Session.blow.guarded?(parried: Bool, +tx: U32, +ty: U32, +k: J.Val, +s: J.Val, +id: U32) -> J.Val:
  match parried:
    case True{}:
      lanternward.blow.parry(k, s, id)
    case False{}:
      lanternward.blow.block(tx, ty, k, s, id)

def lanternward.blow.parry?(+k: J.Val, +s: J.Val, +id: U32) -> J.Val:
  ga.ss(
    lanternward.stop(
      lanternward.sound(
        ga.su(
          lanternward.props.Session.fx(
            lanternward.blow.put_seat(s, id, ga.su(k, "freeze", RS.u_max(ga.u(k, "freeze"), 4))),
            "Parry",
            (ga.u(k, "x") + ga.nu(ga.v(k, "face"), 0) * 9 : U32),
            (ga.u(k, "y") + ga.nu(ga.v(k, "face"), 1) * 9 : U32), 0),
          "flash", RS.u_max(ga.u(s, "flash"), 6)),
        "holy_water"),
      "Phase"),
    "$ret", "Parried")

def lanternward.blow.block?(+tx: U32, +ty: U32, +k: J.Val, +s: J.Val, +id: U32) -> J.Val:
  ga.ss(
    lanternward.sound(
      lanternward.props.Session.fx(
        lanternward.blow.put_seat(s, id,
          ga.su(ga.sv(k, "push", ga.pair_i(RS.i_div((RS.i_neg(tx) * 520 : U32), 256), RS.i_div((RS.i_neg(ty) * 520 : U32), 256))), "pusht", 4)),
        "Ring",
        (ga.u(k, "x") + ga.nu(ga.v(k, "face"), 0) * 9 : U32),
        (ga.u(k, "y") + ga.nu(ga.v(k, "face"), 1) * 9 : U32), 10),
      "shield_block"),
    "$ret", "Blocked")

# The blow lands.

def lanternward.blow.Session.blow.hit?(+k: J.Val, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  lanternward.blow.hit.away(
    lanternward.world.unit((ga.u(k, "x") - fx : U32), (ga.u(k, "y") - fy : U32)), k, s, id, dmg)

def lanternward.blow.hit.away?(away: U32 & U32, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32) -> J.Val:
  (+ax, +ay) = away
  lanternward.blow.hit.seat(RS.u_sat_sub(ga.u(k, "hp"), dmg), ax, ay, k, s, id, dmg)

def lanternward.blow.hit.seat?(+hp: U32, +ax: U32, +ay: U32, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32) -> J.Val:
  +down = U32.is_eq(hp, 0)
  +k1 = ga.sv(ga.su(ga.su(ga.su(ga.su(k, "hp", hp), "inv", 36), "ward", 36), "t", 0), "act", J.JS{Bool.pick(String, down, "Down", "Hurt")})
  +k2 = ga.sb(ga.su(ga.su(ga.su(ga.su(ga.su(k1, "chain", 0), "step", 0), "charge", 0), "shield", 0), "lamp", 0), "queued", False{})
  +k3 = ga.sv(ga.si(k2, "z", 0), "push", ga.pair_i(RS.i_div((ax * 560 : U32), 256), RS.i_div((ay * 560 : U32), 256)))
  +k4 = ga.su(k3, "pusht", 6)
  +k5 = lanternward.blow.hit.down(down, k4)
  lanternward.blow.hit.fin(down, k5, s, id, dmg)

def lanternward.blow.hit.down?(down: Bool, +k: J.Val) -> J.Val:
  match down:
    case False{}:
      k
    case True{}:
      ga.sv(ga.su(ga.sv(k, "lock", J.JNull{}), "rev", 0), "carry", J.JS{Bool.pick(String, lanternward.is(k, "carry", "Pot"), "Nothing", ga.s(k, "carry"))})

def lanternward.blow.hit.fin?(down: Bool, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32) -> J.Val:
  +x = ga.u(ga.at(J.get(s, "seats"), id), "x")
  +y = ga.u(ga.at(J.get(s, "seats"), id), "y")
  lanternward.blow.hit.fall(
    down,
    ga.ss(
      lanternward.sound(
        ga.su(
          lanternward.props.Session.fx(lanternward.blow.put_seat(s, id, k), "Spark", x, (y - 1536 : U32), 0),
          "shake", RS.u_max(ga.u(s, "shake"), 5)),
        Bool.pick(String, U32.is_ge(dmg, 4), "knight_hit_big", "knight_hit")),
      "$ret", "Hit"))

def lanternward.blow.hit.fall?(down: Bool, +s: J.Val) -> J.Val:
  match down:
    case True{}:
      lanternward.sound(s, "knight_fall")
    case False{}:
      s

# A blade, a bolt or a blast on foe `i`. `$ret` is true when it hurt.

def lanternward.blow.Session.hurt_foe?(+s: J.Val, +i: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  lanternward.blow.hurt_foe.alive(lanternward.foes.Foe.alive(lanternward.el(s, "foes", i)), s, i, dmg, fx, fy)

def lanternward.blow.hurt_foe.alive?(alive: Bool, +s: J.Val, +i: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  match alive:
    case False{}:
      ga.sb(s, "$ret", False{})
    case True{}:
      lanternward.blow.hurt_foe.kind(lanternward.el(s, "foes", i), s, i, dmg, fx, fy)

def lanternward.blow.hurt_foe.kind?(+f: J.Val, +s: J.Val, +i: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  +x = ga.u(f, "x")
  +y = ga.u(f, "y")
  lanternward.blow.hurt_foe.code(
    Bool.pick(U32, lanternward.is(f, "kind", "Turret"), 1,
      Bool.pick(U32, Bool.and(lanternward.is(f, "kind", "Snuffer"), Bool.not(lanternward.foes.Foe.vulnerable(f))), 2, 0)),
    f, s, i, dmg, fx, fy)

def lanternward.blow.hurt_foe.code?(+code: U32, +f: J.Val, +s: J.Val, +i: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  match code:
    case 1:
      ga.sb(
        lanternward.props.Session.fx(lanternward.sound(s, "clang"), "Spark", ga.u(f, "x"), ga.u(f, "y"), 0),
        "$ret", False{})
    case 2:
      ga.sb(
        lanternward.props.Session.fx(lanternward.sound(s, "step_ash"), "Ring", ga.u(f, "x"), ga.u(f, "y"), 8),
        "$ret", False{})
    case _:
      lanternward.blow.hurt_foe.hit(
        lanternward.world.unit((ga.u(f, "x") - fx : U32), (ga.u(f, "y") - fy : U32)), f, s, i, dmg)

def lanternward.blow.hurt_foe.hit?(away: U32 & U32, +f: J.Val, +s: J.Val, +i: U32, +dmg: U32) -> J.Val:
  (+ax, +ay) = away
  +dm = Bool.pick(U32, ga.b(f, "open"), (dmg * 2 : U32), dmg)
  +hp = RS.u_sat_sub(ga.u(f, "hp"), dm)
  +f1 = ga.sv(ga.su(ga.su(ga.su(ga.sb(f, "open", Bool.and(ga.b(f, "open"), False{})), "hp", hp), "flinch", 8), "freeze", 3), "kb", ga.pair_i(RS.i_div((ax * 380 : U32), 256), RS.i_div((ay * 380 : U32), 256)))
  +f2 = ga.su(f1, "kbt", 5)
  +f3 = lanternward.blow.hurt_foe.wake(lanternward.is(f, "st", "Dormant"), f2)
  lanternward.blow.hurt_foe.fin(U32.is_eq(hp, 0), f3, s, i)

def lanternward.blow.hurt_foe.wake?(dormant: Bool, +f: J.Val) -> J.Val:
  match dormant:
    case True{}:
      ga.su(ga.sv(f, "st", J.JS{"Drift"}), "t", 20)
    case False{}:
      f

def lanternward.blow.hurt_foe.fin?(dead: Bool, +f: J.Val, +s: J.Val, +i: U32) -> J.Val:
  match dead:
    case True{}:
      ga.sb(
        lanternward.stop(
          lanternward.sound(
            lanternward.props.Session.scatter(
              lanternward.props.Session.fx(
                lanternward.props.Session.fx(
                  lanternward.eput(s, "foes", i,
                    ga.su(ga.sv(ga.su(ga.su(ga.sv(f, "st", J.JS{"Dying"}), "dead", 0), "kbt", 0), "danger", J.JS{"None"}), "kbt", 0)),
                  "Puff", ga.u(f, "x"), ga.u(f, "y"), 0),
                "Light", ga.u(f, "x"), ga.u(f, "y"), 0),
              "Fissure", ga.u(f, "x"), ga.u(f, "y")),
            "kill"),
          "Kill"),
        "$ret", True{})
    case False{}:
      ga.sb(
        lanternward.sound(
          lanternward.props.Session.fx(lanternward.eput(s, "foes", i, f), "Spark", ga.u(f, "x"), ga.u(f, "y"), 0),
          "hit_blade"),
        "$ret", True{})

# Light on foe `i`: a snuffer's hood burns off for a while, and it takes a
# little.

def lanternward.blow.Session.light_foe?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.light_foe.go(
    Bool.and(lanternward.eis(s, "foes", i, "kind", "Snuffer"), lanternward.foes.Foe.alive(lanternward.el(s, "foes", i))),
    s, i)

def lanternward.blow.light_foe.go?(ok: Bool, +s: J.Val, +i: U32) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.sound(
        lanternward.props.Session.fx(
          lanternward.blow.light_foe.wake(
            lanternward.eis(s, "foes", i, "st", "Dormant"),
            lanternward.es(lanternward.es(s, "foes", i, "lit", 120), "foes", i, "flinch", 8), i),
          "Light", lanternward.eu(s, "foes", i, "x"), lanternward.eu(s, "foes", i, "y"), 0),
        "hit_magic")

def lanternward.blow.light_foe.wake?(dormant: Bool, +s: J.Val, +i: U32) -> J.Val:
  match dormant:
    case True{}:
      lanternward.es(lanternward.eset(s, "foes", i, "st", J.JS{"Drift"}), "foes", i, "t", 20)
    case False{}:
      s

# ---- bolts ----

# Fly every bolt one tick.

def lanternward.blow.Session.step_bolts?(+s: J.Val) -> J.Val:
  lanternward.blow.step_bolts.retain(lanternward.blow.step_bolts.loop(s, 0))

def lanternward.blow.step_bolts.loop?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.step_bolts.loop.go(U32.is_lt(i, lanternward.len(s, "bolts")), s, i)

def lanternward.blow.step_bolts.loop.go?(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.blow.step_bolts.loop(lanternward.blow.step_bolts.one(U32.is_eq(lanternward.eu(s, "bolts", i, "ttl"), 0), s, i), (i + 1 : U32))

def lanternward.blow.step_bolts.one?(skip: Bool, +s: J.Val, +i: U32) -> J.Val:
  match skip:
    case True{}:
      s
    case False{}:
      lanternward.es(
        lanternward.blow.step_bolts.half(lanternward.blow.step_bolts.half(s, i), i),
        "bolts", i, "ttl", RS.u_sat_sub(lanternward.eu(s, "bolts", i, "ttl"), 1))

# Fast bolts go in two half steps so none skips a body.

def lanternward.blow.step_bolts.half?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.step_bolts.half.go(U32.is_eq(lanternward.eu(s, "bolts", i, "ttl"), 0), s, i)

def lanternward.blow.step_bolts.half.go?(dead: Bool, +s: J.Val, +i: U32) -> J.Val:
  match dead:
    case True{}:
      s
    case False{}:
      lanternward.blow.Session.bolt_meets(
        lanternward.esi(
          lanternward.esi(s, "bolts", i, "x", (lanternward.eu(s, "bolts", i, "x") + RS.i_div(lanternward.eu(s, "bolts", i, "vx"), 2) : U32)),
          "bolts", i, "y", (lanternward.eu(s, "bolts", i, "y") + RS.i_div(lanternward.eu(s, "bolts", i, "vy"), 2) : U32)),
        i)

def lanternward.blow.step_bolts.retain.go?(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+b, +t}:
      Bool.pick(List<&2, J.Val>, U32.is_gt(ga.u(b, "ttl"), 0),
        Con{b, lanternward.blow.step_bolts.retain.go(t)},
        lanternward.blow.step_bolts.retain.go(t))

def lanternward.blow.step_bolts.retain?(+s: J.Val) -> J.Val:
  ga.sv(s, "bolts", J.JA{lanternward.blow.step_bolts.retain.go(ga.items(s, "bolts"))})

# A bolt is done: a pot breaks, light flickers, iron goes out.

def lanternward.blow.Session.bolt_ends?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.bolt_ends.kind(
    ga.s(lanternward.el(s, "bolts", i), "kind"), lanternward.es(s, "bolts", i, "ttl", 0),
    lanternward.eu(s, "bolts", i, "x"), lanternward.eu(s, "bolts", i, "y"))

def lanternward.blow.bolt_ends.kind?(+kind: String, +s: J.Val, +x: U32, +y: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Pot"),
    lanternward.sound(
      lanternward.props.Session.scatter(lanternward.props.Session.fx(s, "Dust", x, y, 0), "Pot", x, y), "rock_land"),
    Bool.pick(J.Val, lanternward.NAME_IN(kind, ["Light", "Bright"]),
      lanternward.props.Session.fx(s, "Spark", x, y, 0),
      lanternward.props.Session.fx(s, "Dust", x, y, 0)))

# A shield turns an iron bolt: back where the knight's lock holds, else where
# the shield faces.

def lanternward.blow.Session.turn_bolt?(+s: J.Val, +i: U32, +id: U32, +parried: Bool) -> J.Val:
  lanternward.blow.turn_bolt.has(ga.has_key(J.get(s, "seats"), id), s, i, id, parried)

def lanternward.blow.turn_bolt.has?(has: Bool, +s: J.Val, +i: U32, +id: U32, +parried: Bool) -> J.Val:
  match has:
    case False{}:
      s
    case True{}:
      lanternward.blow.turn_bolt.lock(
        lanternward.kit.Session.lock_point(s, ga.v(ga.at(J.get(s, "seats"), id), "lock")), s, i, id, parried)

def lanternward.blow.turn_bolt.lock?(+pt: J.Val, +s: J.Val, +i: U32, +id: U32, +parried: Bool) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.blow.turn_bolt.fin(
        ga.nu(ga.v(ga.at(J.get(s, "seats"), id), "face"), 0), ga.nu(ga.v(ga.at(J.get(s, "seats"), id), "face"), 1), s, i, id, parried)
    case _:
      lanternward.blow.turn_bolt.toward(
        lanternward.world.unit(
          (ga.nu(pt, 0) - ga.u(ga.at(J.get(s, "seats"), id), "x") : U32), (ga.nu(pt, 1) - ga.u(ga.at(J.get(s, "seats"), id), "y") : U32)),
        s, i, id, parried)

def lanternward.blow.turn_bolt.toward?(d: U32 & U32, +s: J.Val, +i: U32, +id: U32, +parried: Bool) -> J.Val:
  (+dx, +dy) = d
  +face = ga.v(ga.at(J.get(s, "seats"), id), "face")
  +ok = RS.i_ge(lanternward.world.dot4(dx, dy, ga.nu(face, 0), ga.nu(face, 1)), 4294967232)
  lanternward.blow.turn_bolt.fin(
    Bool.pick(U32, ok, dx, ga.nu(face, 0)), Bool.pick(U32, ok, dy, ga.nu(face, 1)), s, i, id, parried)

def lanternward.blow.turn_bolt.fin?(+ax: U32, +ay: U32, +s: J.Val, +i: U32, +id: U32, +parried: Bool) -> J.Val:
  +k = ga.at(J.get(s, "seats"), id)
  +speed = Bool.pick(U32, parried, 930, 620)
  lanternward.eput(s, "bolts", i,
    ga.su(
      ga.su(
        ga.si(
          ga.si(
            ga.si(
              ga.si(
                ga.su(
                  ga.sv(lanternward.el(s, "bolts", i), "kind", J.JS{Bool.pick(String, parried, "Bright", "Dim")}),
                  "owner", U32.and(id, 255)),
                "x", (ga.u(k, "x") + ga.nu(ga.v(k, "face"), 0) * 9 : U32)),
              "y", (ga.u(k, "y") + ga.nu(ga.v(k, "face"), 1) * 9 : U32)),
            "vx", RS.i_div((ax * speed : U32), 256)),
          "vy", RS.i_div((ay * speed : U32), 256)),
        "ttl", Bool.pick(U32, parried, 160, 24)),
      "owner", U32.and(id, 255)))

# One bolt against what it meets this half step.

def lanternward.blow.Session.bolt_meets?(+s: J.Val, +i: U32) -> J.Val:
  +c = lanternward.world.Plan.at(ga.u(s, "room"),
    lanternward.world.tile_of(lanternward.eu(s, "bolts", i, "x")), lanternward.world.tile_of(lanternward.eu(s, "bolts", i, "y")))
  lanternward.blow.bolt_meets.wall(Bool.or(U32.is_eq(c, 35), U32.is_eq(c, 0)), s, i)

def lanternward.blow.bolt_meets.wall?(hit: Bool, +s: J.Val, +i: U32) -> J.Val:
  match hit:
    case True{}:
      lanternward.blow.Session.bolt_ends(s, i)
    case False{}:
      lanternward.blow.bolt_meets.props(s, i, 0)

# Things that stand in the way.

def lanternward.blow.bolt_meets.props?(+s: J.Val, +i: U32, +j: U32) -> J.Val:
  lanternward.blow.bolt_meets.props.go(U32.is_lt(j, lanternward.len(s, "props")), s, i, j)

def lanternward.blow.bolt_meets.props.go?(more: Bool, +s: J.Val, +i: U32, +j: U32) -> J.Val:
  match more:
    case False{}:
      lanternward.blow.bolt_meets.kind(ga.s(lanternward.el(s, "bolts", i), "kind"), s, i)
    case True{}:
      lanternward.blow.bolt_meets.prop(
        Bool.and(
          lanternward.props.Prop.blocks(lanternward.el(s, "props", j), lanternward.eu(s, "bolts", i, "x"), lanternward.eu(s, "bolts", i, "y")),
          Bool.not(lanternward.props.Prop.lets_bolts(lanternward.el(s, "props", j)))),
        s, i, j)

def lanternward.blow.bolt_meets.prop?(stops: Bool, +s: J.Val, +i: U32, +j: U32) -> J.Val:
  match stops:
    case False{}:
      lanternward.blow.bolt_meets.props(s, i, (j + 1 : U32))
    case True{}:
      lanternward.blow.Session.bolt_ends(
        lanternward.blow.bolt_meets.stop(
          ga.s(lanternward.el(s, "props", j), "kind"),
          lanternward.foes.Bolt.light(lanternward.el(s, "bolts", i)), s, j),
        i)

# The thing a bolt of light wakes: a crystal, a brazier, the great brazier.

def lanternward.blow.bolt_meets.stop?(+kind: String, +light: Bool, +s: J.Val, +j: U32) -> J.Val:
  Bool.pick(J.Val, Bool.and(light, Bool.and(String.eq(kind, "Crystal"), U32.is_eq(lanternward.eu(s, "props", j, "on"), 0))),
    lanternward.ctx.Session.light_crystal(s, j),
    Bool.pick(J.Val, Bool.and(light, Bool.and(String.eq(kind, "Brazier"), Bool.not(lanternward.props.Prop.lit(lanternward.el(s, "props", j))))),
      lanternward.sound(
        lanternward.props.Session.fx(lanternward.es(s, "props", j, "on", 1), "Light", lanternward.eu(s, "props", j, "x"), lanternward.eu(s, "props", j, "y"), 0),
        "spell_fire"),
      Bool.pick(J.Val, Bool.and(light, Bool.and(String.eq(kind, "Great"), U32.is_eq(lanternward.eu(s, "props", j, "on"), 0))),
        lanternward.props.Session.solved(lanternward.es(s, "props", j, "on", 1), lanternward.eu(s, "props", j, "x"), lanternward.eu(s, "props", j, "y")),
        s)))

def lanternward.blow.bolt_meets.kind?(+kind: String, +s: J.Val, +i: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Iron"),
    lanternward.blow.bolt_meets.iron(ga.ids(J.get(s, "seats")), s, i),
    lanternward.blow.bolt_meets.foes(s, i, 0))

# Iron: the first knight it touches takes it (or turns it).

def lanternward.blow.bolt_meets.iron?(+ids: List<&2, U32>, +s: J.Val, +i: U32) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.blow.bolt_meets.iron.near(
        Bool.and(
          lanternward.kit.Seat.up(ga.at(J.get(s, "seats"), id)),
          RS.i_le(
            lanternward.world.dist(
              (ga.u(ga.at(J.get(s, "seats"), id), "x") - lanternward.eu(s, "bolts", i, "x") : U32),
              (ga.u(ga.at(J.get(s, "seats"), id), "y") - lanternward.eu(s, "bolts", i, "y") : U32)), 2048)),
        id, t, s, i)

def lanternward.blow.bolt_meets.iron.near?(near: Bool, +id: U32, +t: List<&2, U32>, +s: J.Val, +i: U32) -> J.Val:
  match near:
    case False{}:
      lanternward.blow.bolt_meets.iron(t, s, i)
    case True{}:
      lanternward.blow.bolt_meets.iron.after(
        lanternward.blow.Session.blow(s, id, 2,
          (lanternward.eu(s, "bolts", i, "x") - lanternward.eu(s, "bolts", i, "vx") : U32),
          (lanternward.eu(s, "bolts", i, "y") - lanternward.eu(s, "bolts", i, "vy") : U32), "Bolt"),
        id, t, i)

def lanternward.blow.bolt_meets.iron.after?(+s: J.Val, +id: U32, +t: List<&2, U32>, +i: U32) -> J.Val:
  lanternward.blow.bolt_meets.iron.res(ga.s(s, "$ret"), J.del(s, "$ret"), id, t, i)

def lanternward.blow.bolt_meets.iron.res?(+out: String, +s: J.Val, +id: U32, +t: List<&2, U32>, +i: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(out, "Dodged"), lanternward.blow.bolt_meets.iron(t, s, i),
    Bool.pick(J.Val, String.eq(out, "Hit"), lanternward.es(s, "bolts", i, "ttl", 0),
      lanternward.blow.Session.turn_bolt(s, i, id, String.eq(out, "Parried"))))

# Light, pot or a turned bolt: the first foe it touches, then the Warden.

def lanternward.blow.bolt_meets.foes?(+s: J.Val, +i: U32, +j: U32) -> J.Val:
  lanternward.blow.bolt_meets.foes.go(U32.is_lt(j, lanternward.len(s, "foes")), s, i, j)

def lanternward.blow.bolt_meets.foes.go?(more: Bool, +s: J.Val, +i: U32, +j: U32) -> J.Val:
  match more:
    case False{}:
      lanternward.blow.bolt_meets.boss(s, i)
    case True{}:
      lanternward.blow.bolt_meets.foe(
        Bool.and(
          lanternward.foes.Foe.alive(lanternward.el(s, "foes", j)),
          RS.i_le(
            lanternward.world.dist(
              (lanternward.eu(s, "foes", j, "x") - lanternward.eu(s, "bolts", i, "x") : U32),
              (lanternward.eu(s, "foes", j, "y") - lanternward.eu(s, "bolts", i, "y") : U32)),
            (lanternward.foes.Foe.radius(lanternward.el(s, "foes", j)) + 512 : U32))),
        s, i, j)

def lanternward.blow.bolt_meets.foe?(near: Bool, +s: J.Val, +i: U32, +j: U32) -> J.Val:
  match near:
    case False{}:
      lanternward.blow.bolt_meets.foes(s, i, (j + 1 : U32))
    case True{}:
      lanternward.blow.Session.bolt_ends(
        lanternward.blow.bolt_meets.foe.hit(
          ga.s(lanternward.el(s, "bolts", i), "kind"), s, i, j,
          (lanternward.eu(s, "bolts", i, "x") - lanternward.eu(s, "bolts", i, "vx") : U32),
          (lanternward.eu(s, "bolts", i, "y") - lanternward.eu(s, "bolts", i, "vy") : U32)),
        i)

def lanternward.blow.bolt_meets.foe.hit?(+kind: String, +s: J.Val, +i: U32, +j: U32, +fx: U32, +fy: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Light"), lanternward.blow.Session.light_foe(s, j),
    Bool.pick(J.Val, String.eq(kind, "Bright"),
      lanternward.blow.bolt_meets.foe.bright(lanternward.eis(s, "foes", j, "kind", "Turret"), s, j, fx, fy),
      Bool.pick(J.Val, String.eq(kind, "Dim"),
        lanternward.blow.Session.hurt_foe(s, j, 1, fx, fy),
        lanternward.blow.Session.hurt_foe(s, j, 2, fx, fy))))

def lanternward.blow.bolt_meets.foe.bright?(turret: Bool, +s: J.Val, +j: U32, +fx: U32, +fy: U32) -> J.Val:
  match turret:
    case True{}:
      lanternward.sound(
        lanternward.props.Session.fx(
          lanternward.esb(
            lanternward.eset(
              lanternward.es(lanternward.eset(lanternward.es(s, "foes", j, "t", 90), "foes", j, "st", J.JS{"Stagger"}), "foes", j, "t", 90),
              "foes", j, "danger", J.JS{"None"}),
            "foes", j, "open", True{}),
          "Parry", lanternward.eu(s, "foes", j, "x"), lanternward.eu(s, "foes", j, "y"), 0),
        "clang")
    case False{}:
      lanternward.blow.Session.hurt_foe(s, j, 4, fx, fy)

# The Warden: its heart, or its body, meets the bolt.

def lanternward.blow.bolt_meets.boss?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.bolt_meets.boss.go(
    Bool.and(Bool.not(J.is_null(J.get(s, "boss"))), U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0)), s, i)

def lanternward.blow.bolt_meets.boss.go?(alive: Bool, +s: J.Val, +i: U32) -> J.Val:
  match alive:
    case False{}:
      s
    case True{}:
      lanternward.blow.bolt_meets.boss.near(
        RS.i_le(
          lanternward.world.dist(
            (lanternward.boss.Warden.heart.x(J.get(s, "boss")) - lanternward.eu(s, "bolts", i, "x") : U32),
            (lanternward.boss.Warden.heart.y(J.get(s, "boss")) - lanternward.eu(s, "bolts", i, "y") : U32)), 2304),
        RS.i_le(
          lanternward.world.dist(
            (ga.u(J.get(s, "boss"), "x") - lanternward.eu(s, "bolts", i, "x") : U32),
            (ga.u(J.get(s, "boss"), "y") - lanternward.eu(s, "bolts", i, "y") : U32)), 4608),
        s, i)

def lanternward.blow.bolt_meets.boss.near?(+heart: Bool, +body: Bool, +s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.bolt_meets.boss.near.go(Bool.or(heart, body), heart, s, i)

def lanternward.blow.bolt_meets.boss.near.go?(any: Bool, heart: Bool, +s: J.Val, +i: U32) -> J.Val:
  match any:
    case False{}:
      s
    case True{}:
      lanternward.blow.Session.bolt_ends(
        lanternward.blow.bolt_meets.boss.hit(
          ga.s(lanternward.el(s, "bolts", i), "kind"), heart, s,
          (lanternward.eu(s, "bolts", i, "x") - lanternward.eu(s, "bolts", i, "vx") : U32),
          (lanternward.eu(s, "bolts", i, "y") - lanternward.eu(s, "bolts", i, "vy") : U32),
          lanternward.eu(s, "bolts", i, "owner")),
        i)

def lanternward.blow.bolt_meets.boss.hit?(+kind: String, heart: Bool, +s: J.Val, +fx: U32, +fy: U32, +owner: U32) -> J.Val:
  Bool.pick(J.Val, Bool.and(lanternward.NAME_IN(kind, ["Light", "Bright"]), heart),
    lanternward.boss.Session.light_heart(s, owner),
    Bool.pick(J.Val, String.eq(kind, "Pot"),
      lanternward.boss.Session.hurt_boss(s, 2, fx, fy, owner),
      s))

# ---- bombs ----

# Bombs: tick the fuses; a burst opens cracked walls, breaks pots and grass
# and hurts the Gloam, never the knights.

def lanternward.blow.fuses_tick?(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Con{ga.su(f, "t", RS.u_sat_sub(ga.u(f, "t"), 1)), lanternward.blow.fuses_tick(t)}

def lanternward.blow.fuses_burst?(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Bool.pick(List<&2, J.Val>, U32.is_eq(ga.u(f, "t"), 0),
        Con{f, lanternward.blow.fuses_burst(t)}, lanternward.blow.fuses_burst(t))

def lanternward.blow.fuses_keep?(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Bool.pick(List<&2, J.Val>, U32.is_gt(ga.u(f, "t"), 0),
        Con{f, lanternward.blow.fuses_keep(t)}, lanternward.blow.fuses_keep(t))

def lanternward.blow.Session.step_fuses?(+s: J.Val) -> J.Val:
  lanternward.blow.step_fuses.go(
    lanternward.blow.fuses_burst(lanternward.blow.fuses_tick(ga.items(s, "fuses"))),
    ga.sv(s, "fuses", J.JA{lanternward.blow.fuses_keep(lanternward.blow.fuses_tick(ga.items(s, "fuses")))}))

def lanternward.blow.step_fuses.go?(+burst: List<&2, J.Val>, +s: J.Val) -> J.Val:
  match burst:
    case Nil{}:
      s
    case Con{+f, +t}:
      lanternward.blow.step_fuses.go(t, lanternward.blow.step_fuses.burst(s, ga.u(f, "x"), ga.u(f, "y")))

def lanternward.blow.step_fuses.burst?(+s: J.Val, +x: U32, +y: U32) -> J.Val:
  lanternward.blow.step_fuses.boss(
    lanternward.blow.step_fuses.props(
      lanternward.blow.step_fuses.foes(
        lanternward.stop(
          lanternward.sound(
            ga.su(lanternward.props.Session.fx(s, "Blast", x, y, 0), "shake", RS.u_max(ga.u(s, "shake"), 8)),
            "bomb"),
          "Heavy"),
        0, x, y),
      0, x, y),
    x, y)

def lanternward.blow.step_fuses.foes?(+s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
  lanternward.blow.step_fuses.foes.go(U32.is_lt(i, lanternward.len(s, "foes")), s, i, x, y)

def lanternward.blow.step_fuses.foes.go?(more: Bool, +s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.blow.step_fuses.foes(
        lanternward.blow.step_fuses.foe(
          Bool.and(
            lanternward.foes.Foe.alive(lanternward.el(s, "foes", i)),
            RS.i_le(
              lanternward.world.dist((lanternward.eu(s, "foes", i, "x") - x : U32), (lanternward.eu(s, "foes", i, "y") - y : U32)), 9728)),
          s, i, x, y),
        (i + 1 : U32), x, y)

def lanternward.blow.step_fuses.foe?(hit: Bool, +s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.blow.step_fuses.foe.kind(lanternward.eis(s, "foes", i, "kind", "Turret"), s, i, x, y)

def lanternward.blow.step_fuses.foe.kind?(turret: Bool, +s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
  match turret:
    case True{}:
      lanternward.eset(lanternward.es(lanternward.eset(s, "foes", i, "st", J.JS{"Stagger"}), "foes", i, "t", 90), "foes", i, "danger", J.JS{"None"})
    case False{}:
      lanternward.blow.Session.hurt_foe(lanternward.es(s, "foes", i, "lit", 60), i, 8, x, y)

def lanternward.blow.step_fuses.props?(+s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
  lanternward.blow.step_fuses.props.go(U32.is_lt(i, lanternward.len(s, "props")), s, i, x, y)

def lanternward.blow.step_fuses.props.go?(more: Bool, +s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.blow.step_fuses.props(
        lanternward.blow.step_fuses.prop(
          RS.i_le(
            lanternward.world.dist((lanternward.eu(s, "props", i, "x") - x : U32), (lanternward.eu(s, "props", i, "y") - y : U32)), 9728),
          s, i),
        (i + 1 : U32), x, y)

def lanternward.blow.step_fuses.prop?(near: Bool, +s: J.Val, +i: U32) -> J.Val:
  match near:
    case False{}:
      s
    case True{}:
      lanternward.blow.step_fuses.prop.kind(
        ga.s(lanternward.el(s, "props", i), "kind"), U32.is_eq(lanternward.eu(s, "props", i, "on"), 0), s, i)

def lanternward.blow.step_fuses.prop.kind?(+kind: String, +on0: Bool, +s: J.Val, +i: U32) -> J.Val:
  Bool.pick(J.Val, Bool.and(String.eq(kind, "Cracked"), on0),
    lanternward.sound(
      lanternward.props.Session.solved(
        lanternward.props.Session.fx(lanternward.es(s, "props", i, "on", 1), "Dust", lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 0),
        lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y")),
      "crack_open"),
    Bool.pick(J.Val, Bool.and(lanternward.NAME_IN(kind, ["Pot", "Grass"]), on0),
      lanternward.props.Session.shatter(s, i),
      s))

def lanternward.blow.step_fuses.boss?(+s: J.Val, +x: U32, +y: U32) -> J.Val:
  lanternward.blow.step_fuses.boss.go(Bool.not(J.is_null(J.get(s, "boss"))), s, x, y)

def lanternward.blow.step_fuses.boss.go?(has: Bool, +s: J.Val, +x: U32, +y: U32) -> J.Val:
  match has:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.blast_boss(s, x, y)

def lanternward.boss.Warden.heart.x?(+w: J.Val) -> U32:
  (ga.u(w, "x") + ga.nu(ga.v(w, "face"), 0) * 9 : U32)

def lanternward.boss.Warden.heart.y?(+w: J.Val) -> U32:
  (ga.u(w, "y") + ga.nu(ga.v(w, "face"), 1) * 9 : U32)

def lanternward.boss.Warden.radius?() -> U32:
  4608

def lanternward.boss.Warden.new?(+x: U32, +y: U32, +seats: U32) -> J.Val:
  lanternward.boss.Warden.new.go(x, y, RS.u_max(seats, 1), (90 * RS.u_max(seats, 1) : U32), RS.u_clamp(seats, 1, 6))

def lanternward.boss.Warden.new.go?(+x: U32, +y: U32, +seats: U32, +full: U32, +prov: U32) -> J.Val:
  +w0 = ga.si(ga.si(J.obj(), "x", x), "y", y)
  +w1 = ga.su(ga.su(ga.su(ga.su(w0, "hp", full), "full", full), "prov", prov), "phase", 0)
  +w2 = ga.su(ga.sv(ga.sv(w1, "st", J.JS{"Asleep"}), "atk", J.JS{"Slam"}), "t", 0)
  +w3 = ga.si(ga.sv(ga.su(w2, "n", 0), "danger", J.JS{"None"}), "aim", 90)
  +w4 = ga.sv(ga.sv(ga.si(w3, "sweep", 0), "lit", lanternward.zeros6()), "hit", lanternward.zeros6())
  ga.su(ga.sv(ga.su(ga.su(ga.su(w4, "struck", 0), "dead", 0), "start", full), "face", ga.pair_i(0, 256)), "flinch", 0)

def lanternward.zeros6?() -> J.Val:
  J.JA{[J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}]}

# Whether every number is one a save or the wire may carry.

def lanternward.boss.Warden.valid?(+w: J.Val) -> Bool:
  Bool.and(
    Bool.and(
      Bool.and(U32.is_le(ga.u(w, "hp"), ga.u(w, "full")), U32.is_le(ga.u(w, "full"), 540)),
      Bool.and(U32.is_ge(ga.u(w, "prov"), 1), U32.is_le(ga.u(w, "prov"), 6))),
    Bool.and(
      Bool.and(U32.is_le(ga.u(w, "phase"), 2), U32.is_le(ga.u(w, "t"), 600)),
      Bool.and(
        lanternward.in_range(ga.u(w, "sweep"), 4294966936, 360),
        Bool.and(
          RS.i_le(lanternward.world.dist(ga.nu(ga.v(w, "face"), 0), ga.nu(ga.v(w, "face"), 1)), 264),
          Bool.and(U32.is_le(ga.u(w, "dead"), 600), U32.is_le(ga.u(w, "start"), ga.u(w, "full")))))))

# How many knights' light the shutter wants.

def lanternward.boss.Warden.need?(+w: J.Val, +seats: U32) -> U32:
  Bool.pick(U32, U32.is_ge(ga.u(w, "phase"), 1), RS.u_min(seats, 2), 1)

def lanternward.boss.lit_count?(+xs: List<&2, J.Val>, +now: U32) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, +t}:
      (Bool.to_u32(Bool.and(U32.is_gt(J.u32(h), 0), U32.is_gt((J.u32(h) + 90 : U32), now))) + lanternward.boss.lit_count(t, now) : U32)

# Whether the shutter is open at tick `now`.

def lanternward.boss.Warden.exposed?(+w: J.Val, +now: U32, +seats: U32) -> Bool:
  Bool.or(
    lanternward.is(w, "st", "Stagger"),
    U32.is_ge(lanternward.boss.lit_count(ga.items(w, "lit"), now), lanternward.boss.Warden.need(w, seats)))

# Light fell on the lamp-heart from seat `owner`.

def lanternward.boss.Session.light_heart?(+s: J.Val, +owner: U32) -> J.Val:
  lanternward.boss.Session.light_heart.go(
    Bool.and(Bool.not(J.is_null(J.get(s, "boss"))), Bool.and(U32.is_ge(owner, 1), U32.is_le(owner, 6))), s, owner)

def lanternward.boss.Session.light_heart.go?(ok: Bool, +s: J.Val, +owner: U32) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      ga.sv(s, "boss", ga.sv(J.get(s, "boss"), "lit", J.set_nth(ga.v(J.get(s, "boss"), "lit"), (owner - 1 : U32), J.JU{ga.u(s, "elapsed")})))

def lanternward.boss.Session.boss_exposed?(+s: J.Val) -> Bool:
  Bool.and(
    Bool.not(J.is_null(J.get(s, "boss"))),
    lanternward.boss.Warden.exposed(J.get(s, "boss"), ga.u(s, "elapsed"), lanternward.seat_count(s)))

def lanternward.boss.Session.boss_open?(+s: J.Val) -> Bool:
  lanternward.boss.Session.boss_exposed(s)

# A blade or a pot on the Warden. Only the open heart can be hurt. `$ret` is
# whether it hurt.

def lanternward.boss.Session.hurt_boss?(+s: J.Val, +dmg: U32, +fx: U32, +fy: U32, +owner: U32) -> J.Val:
  lanternward.boss.Session.hurt_boss.open(lanternward.boss.Session.boss_exposed(s), s, dmg, fx, fy)

def lanternward.boss.Session.hurt_boss.open?(open: Bool, +s: J.Val, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  match open:
    case False{}:
      lanternward.boss.Session.hurt_boss.shut(J.get(s, "boss"), s, fx, fy)
    case True{}:
      lanternward.boss.Session.hurt_boss.hit(
        Bool.or(U32.is_eq(ga.u(J.get(s, "boss"), "hp"), 0), lanternward.is(J.get(s, "boss"), "st", "Dying")), s, dmg)

def lanternward.boss.Session.hurt_boss.shut?(+w: J.Val, +s: J.Val, +fx: U32, +fy: U32) -> J.Val:
  +bx = ga.u(w, "x")
  +by = ga.u(w, "y")
  ga.sb(
    lanternward.sound(
      lanternward.props.Session.fx(s, "Ring",
        (bx + RS.i_div((fx - bx : U32), 6) : U32), (by + RS.i_div((fy - by : U32), 6) : U32), 12),
      "clang"),
    "$ret", False{})

def lanternward.boss.Session.hurt_boss.hit?(already: Bool, +s: J.Val, +dmg: U32) -> J.Val:
  match already:
    case True{}:
      ga.sb(s, "$ret", False{})
    case False{}:
      lanternward.boss.Session.hurt_boss.dmg(J.get(s, "boss"), s, dmg)

def lanternward.boss.Session.hurt_boss.dmg?(+w: J.Val, +s: J.Val, +dmg: U32) -> J.Val:
  +dm = Bool.pick(U32, lanternward.is(w, "st", "Stagger"), (dmg * 2 : U32), dmg)
  +hp = RS.u_sat_sub(ga.u(w, "hp"), dm)
  +third = U32.div(ga.u(w, "full"), 3)
  +phase = Bool.pick(U32, U32.is_le(hp, third), 2, Bool.pick(U32, U32.is_le(hp, (third * 2 : U32)), 1, 0))
  +w1 = ga.su(ga.su(w, "hp", hp), "flinch", 8)
  lanternward.boss.Session.hurt_boss.after(U32.is_eq(hp, 0), phase, ga.u(w, "phase"), w1, s)

def lanternward.boss.Session.hurt_boss.after?(dead: Bool, +phase: U32, +before: U32, +w: J.Val, +s: J.Val) -> J.Val:
  match dead:
    case True{}:
      lanternward.boss.Session.hurt_boss.die(w, s)
    case False{}:
      lanternward.boss.Session.hurt_boss.live(U32.is_gt(phase, before), phase, w, s)

# It dies: its summons go out with it.

def lanternward.boss.Session.hurt_boss.die?(+w: J.Val, +s: J.Val) -> J.Val:
  ga.sb(
    ga.su(
      lanternward.props.Session.fx(
        lanternward.stop(
          lanternward.sound(
            lanternward.boss.Session.hurt_boss.foes(
              ga.sv(s, "boss", ga.sv(ga.su(ga.sv(w, "st", J.JS{"Dying"}), "dead", 0), "danger", J.JS{"None"})), 0),
            "boss_death"),
          "Boss"),
        "Puff", ga.u(w, "x"), ga.u(w, "y"), 0),
      "shake", 16),
    "$ret", True{})

def lanternward.boss.Session.hurt_boss.foes?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.boss.Session.hurt_boss.foes.go(U32.is_lt(i, lanternward.len(s, "foes")), s, i)

def lanternward.boss.Session.hurt_boss.foes.go?(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.hurt_boss.foes(
        lanternward.boss.Session.hurt_boss.foe(lanternward.foes.Foe.alive(lanternward.el(s, "foes", i)), s, i), (i + 1 : U32))

def lanternward.boss.Session.hurt_boss.foe?(alive: Bool, +s: J.Val, +i: U32) -> J.Val:
  match alive:
    case False{}:
      s
    case True{}:
      lanternward.es(lanternward.es(lanternward.eset(lanternward.es(s, "foes", i, "hp", 0), "foes", i, "st", J.JS{"Dying"}), "foes", i, "dead", 0), "foes", i, "dead", 0)

# Not dead: a new phase rears it and slams the shutter; else a plain hit.

def lanternward.boss.Session.hurt_boss.live?(rise: Bool, +phase: U32, +w: J.Val, +s: J.Val) -> J.Val:
  match rise:
    case True{}:
      lanternward.boss.Session.hurt_boss.spark(
        lanternward.boss.Session.hurt_boss.rise(phase, w, s))
    case False{}:
      lanternward.boss.Session.hurt_boss.spark(
        lanternward.sound(ga.sv(s, "boss", w), "hit_axe"))

def lanternward.boss.Session.hurt_boss.rise?(+phase: U32, +w: J.Val, +s: J.Val) -> J.Val:
  +w1 = ga.sv(ga.sv(ga.su(ga.sv(ga.su(ga.su(w, "phase", phase), "start", ga.u(w, "hp")), "st", J.JS{"Recover"}), "t", 48), "danger", J.JS{"None"}), "lit", lanternward.zeros6())
  ga.su(
    ga.su(
      lanternward.stop(
        lanternward.boss.Session.hurt_boss.dark(U32.is_eq(phase, 2), lanternward.sound(ga.sv(s, "boss", w1), "boss_rage")),
        "Phase"),
      "shake", 8),
    "flash", RS.u_max(ga.u(s, "flash"), 6))

# The Gloam falls: every lamp in the arena is snuffed.

def lanternward.boss.Session.hurt_boss.dark?(last: Bool, +s: J.Val) -> J.Val:
  match last:
    case False{}:
      s
    case True{}:
      lanternward.sound(lanternward.boss.Session.hurt_boss.braziers(s, 0), "step_ash")

def lanternward.boss.Session.hurt_boss.braziers?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.boss.Session.hurt_boss.braziers.go(U32.is_lt(i, lanternward.len(s, "props")), s, i)

def lanternward.boss.Session.hurt_boss.braziers.go?(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.hurt_boss.braziers(
        lanternward.boss.Session.hurt_boss.brazier(lanternward.eis(s, "props", i, "kind", "Brazier"), s, i), (i + 1 : U32))

def lanternward.boss.Session.hurt_boss.brazier?(yes: Bool, +s: J.Val, +i: U32) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.es(lanternward.es(s, "props", i, "on", 0), "props", i, "t", 0)

def lanternward.boss.Session.hurt_boss.spark?(+s: J.Val) -> J.Val:
  ga.sb(
    lanternward.props.Session.fx(s, "Spark",
      lanternward.boss.Warden.heart.x(J.get(s, "boss")), lanternward.boss.Warden.heart.y(J.get(s, "boss")), 0),
    "$ret", True{})

# A bomb's blast near the Warden: it counts only on an open heart.

def lanternward.boss.Session.blast_boss?(+s: J.Val, +x: U32, +y: U32) -> J.Val:
  lanternward.boss.Session.blast_boss.go(
    Bool.not(J.is_null(J.get(s, "boss"))), s, x, y)

def lanternward.boss.Session.blast_boss.go?(has: Bool, +s: J.Val, +x: U32, +y: U32) -> J.Val:
  match has:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.blast_boss.hit(
        RS.i_le(
          lanternward.world.dist((ga.u(J.get(s, "boss"), "x") - x : U32), (ga.u(J.get(s, "boss"), "y") - y : U32)),
          (9728 + 4608 : U32)),
        s, x, y)

def lanternward.boss.Session.blast_boss.hit?(near: Bool, +s: J.Val, +x: U32, +y: U32) -> J.Val:
  match near:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.hurt_boss(s, 8, x, y, 0)

# A retry puts the Warden back at the start of the phase it was in, at the
# health the table gave it.

def lanternward.boss.Session.reset_boss_phase?(+s: J.Val) -> J.Val:
  lanternward.boss.Session.reset_boss_phase.go(
    Bool.and(Bool.not(J.is_null(J.get(s, "boss"))), U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0)),
    s, lanternward.world.Plan.find(4, 87))

def lanternward.boss.Session.reset_boss_phase.go?(ok: Bool, +s: J.Val, +marks: List<&2, U32>) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.reset_boss_phase.at(s, marks)

def lanternward.boss.Session.reset_boss_phase.at?(+s: J.Val, +marks: List<&2, U32>) -> J.Val:
  match marks:
    case Nil{}:
      lanternward.boss.Session.reset_boss_phase.set(s, 0, 0)
    case Con{+m, +t}:
      lanternward.boss.Session.reset_boss_phase.set(
        s, lanternward.world.middle(lanternward.world.packed_x(m)), lanternward.world.middle(lanternward.world.packed_y(m)))

def lanternward.boss.Session.reset_boss_phase.set?(+s: J.Val, +px: U32, +py: U32) -> J.Val:
  +w = J.get(s, "boss")
  +w1 = ga.su(ga.sv(ga.su(w, "hp", RS.u_max(RS.u_min(ga.u(w, "start"), ga.u(w, "full")), 1)), "st", J.JS{"Rest"}), "t", 60)
  +w2 = ga.sv(ga.sv(ga.si(ga.si(w1, "x", px), "y", py), "danger", J.JS{"None"}), "lit", lanternward.zeros6())
  ga.sv(s, "boss", ga.su(ga.su(w2, "struck", 0), "n", 0))

# ---- the Warden's tick ----

# Flares held on the heart keep it lit.

def lanternward.boss.step_boss.flares?(+s: J.Val, +ids: List<&2, U32>, +hx: U32, +hy: U32) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.boss.step_boss.flares(
        lanternward.boss.step_boss.flare(lanternward.boss.step_boss.flares.ok(s, id, hx, hy), s, id), t, hx, hy)

def lanternward.boss.step_boss.flares.ok?(+s: J.Val, +id: U32, +hx: U32, +hy: U32) -> Bool:
  +k = ga.at(J.get(s, "seats"), id)
  Bool.and(
    Bool.and(lanternward.kit.Seat.flaring(k), lanternward.kit.Seat.up(k)),
    Bool.and(
      RS.i_le(lanternward.world.dist((ga.u(k, "x") - hx : U32), (ga.u(k, "y") - hy : U32)), 17920),
      lanternward.props.Session.line_open(s, ga.u(k, "x"), ga.u(k, "y"), hx, hy)))

def lanternward.boss.step_boss.flare?(ok: Bool, +s: J.Val, +id: U32) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.light_heart(s, id)

# One tick of the Warden.

def lanternward.boss.Session.step_boss?(+s: J.Val) -> J.Val:
  lanternward.boss.step_boss.go(J.is_null(J.get(s, "boss")), s)

def lanternward.boss.step_boss.go?(none: Bool, +s: J.Val) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.boss.step_boss.take(
        lanternward.boss.step_boss.flares(
          s, ga.ids(J.get(s, "seats")), lanternward.boss.Warden.heart.x(J.get(s, "boss")), lanternward.boss.Warden.heart.y(J.get(s, "boss"))))

def lanternward.boss.step_boss.take?(+s: J.Val) -> J.Val:
  lanternward.boss.step_boss.fin(
    lanternward.boss.step_boss.st(
      ga.s(J.get(s, "boss"), "st"),
      ga.sv(ga.sv(ga.sv(s, "$w", lanternward.wsub(J.get(s, "boss"))), "boss", J.JNull{}), "$parried", J.JBool{False{}}),
      lanternward.foes.Session.nearest_knight(s, ga.u(J.get(s, "boss"), "x"), ga.u(J.get(s, "boss"), "y"))))

# w.flinch = w.flinch.saturating_sub(1), applied as the Warden is taken.

def lanternward.wsub?(+w: J.Val) -> J.Val:
  ga.su(w, "flinch", RS.u_sat_sub(ga.u(w, "flinch"), 1))

def lanternward.boss.step_boss.fin?(+s: J.Val) -> J.Val:
  J.del(ga.sv(lanternward.boss.step_boss.shove(s, ga.ids(J.get(s, "seats"))), "boss", J.get(s, "$w")), "$w")

# It is a body: a knight cannot stand in it.

def lanternward.boss.step_boss.shove?(+s: J.Val, +ids: List<&2, U32>) -> J.Val:
  lanternward.boss.step_boss.shove.go(Bool.not(lanternward.wis(s, "st", "Dying")), s, ids)

def lanternward.boss.step_boss.shove.go?(live: Bool, +s: J.Val, +ids: List<&2, U32>) -> J.Val:
  match live:
    case False{}:
      s
    case True{}:
      lanternward.boss.step_boss.shove.ids(s, ids)

def lanternward.boss.step_boss.shove.ids?(+s: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.boss.step_boss.shove.ids(lanternward.boss.step_boss.shove.one(s, id), t)

def lanternward.boss.step_boss.shove.one?(+s: J.Val, +id: U32) -> J.Val:
  +k = ga.at(J.get(s, "seats"), id)
  +dx = (ga.u(k, "x") - lanternward.wu(s, "x") : U32)
  +dy = (ga.u(k, "y") - lanternward.wu(s, "y") : U32)
  +d = lanternward.world.dist(dx, dy)
  +min = (4608 + 1280 : U32)
  lanternward.boss.step_boss.shove.push(
    Bool.and(lanternward.kit.Seat.up(k), Bool.and(RS.i_lt(d, min), RS.i_gt(d, 0))),
    lanternward.world.unit(dx, dy), s, id, min)

def lanternward.boss.step_boss.shove.push?(yes: Bool, u: U32 & U32, +s: J.Val, +id: U32, +min: U32) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.boss.step_boss.shove.set(u, s, id, min)

def lanternward.boss.step_boss.shove.set?(u: U32 & U32, +s: J.Val, +id: U32, +min: U32) -> J.Val:
  (+ux, +uy) = u
  lanternward.blow.put_seat(s, id,
    ga.si(ga.si(ga.at(J.get(s, "seats"), id), "x", (lanternward.wu(s, "x") + RS.i_div((ux * min : U32), 256) : U32)),
      "y", (lanternward.wu(s, "y") + RS.i_div((uy * min : U32), 256) : U32)))

def lanternward.boss.step_boss.st?(+st: String, +s: J.Val, +target: J.Val) -> J.Val:
  lanternward.boss.step_boss.code(
    Bool.pick(U32, String.eq(st, "Dying"), 1,
      Bool.pick(U32, String.eq(st, "Asleep"), 2,
        Bool.pick(U32, String.eq(st, "Rest"), 3,
          Bool.pick(U32, String.eq(st, "Tell"), 4,
            Bool.pick(U32, String.eq(st, "Strike"), 5,
              Bool.pick(U32, String.eq(st, "Recover"), 6, 7)))))),
    s, target)

def lanternward.boss.step_boss.code?(+code: U32, +s: J.Val, +target: J.Val) -> J.Val:
  match code:
    case 1:
      lanternward.boss.step_boss.dying(
        lanternward.ws(s, "dead", (lanternward.wu(s, "dead") + 1 : U32)))
    case 2:
      lanternward.boss.step_boss.asleep(
        Bool.or(
          Bool.and(Bool.not(J.is_null(target)), RS.i_lt(ga.nu(target, 3), 38400)),
          U32.is_gt(ga.u(s, "stage_t"), 150)),
        s)
    case 3:
      lanternward.boss.step_boss.rest(J.is_null(target), s, target)
    case 4:
      lanternward.boss.step_boss.tell(U32.is_le(lanternward.wu(s, "t"), 1), lanternward.ws(s, "t", RS.u_sat_sub(lanternward.wu(s, "t"), 1)))
    case 5:
      lanternward.boss.step_boss.strike(s)
    case 6:
      lanternward.boss.step_boss.recover(U32.is_le(lanternward.wu(s, "t"), 1), lanternward.ws(s, "t", RS.u_sat_sub(lanternward.wu(s, "t"), 1)))
    case _:
      lanternward.boss.step_boss.stagger(U32.is_le(lanternward.wu(s, "t"), 1), lanternward.ws(s, "t", RS.u_sat_sub(lanternward.wu(s, "t"), 1)))

def lanternward.boss.step_boss.dying?(+s: J.Val) -> J.Val:
  lanternward.boss.step_boss.dying.go(U32.is_eq(lanternward.wu(s, "dead"), 40), s)

def lanternward.boss.step_boss.dying.go?(drop: Bool, +s: J.Val) -> J.Val:
  match drop:
    case True{}:
      ga.su(lanternward.props.Session.drop_at(s, "Container", lanternward.wu(s, "x"), lanternward.wu(s, "y")), "stage", 2)
    case False{}:
      s

def lanternward.boss.step_boss.asleep?(woke: Bool, +s: J.Val) -> J.Val:
  match woke:
    case False{}:
      s
    case True{}:
      ga.su(lanternward.sound(ga.su(lanternward.ws(lanternward.wsn(s, "st", "Rest"), "t", 70), "stage", 1), "boss_roar"), "shake", 8)

def lanternward.boss.step_boss.rest?(none: Bool, +s: J.Val, +target: J.Val) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.boss.step_boss.rest.go(
        lanternward.world.unit((ga.nu(target, 1) - lanternward.wu(s, "x") : U32), (ga.nu(target, 2) - lanternward.wu(s, "y") : U32)),
        s, target)

def lanternward.boss.step_boss.rest.go?(face: U32 & U32, +s: J.Val, +target: J.Val) -> J.Val:
  (+fx, +fy) = face
  lanternward.boss.step_boss.rest.walk(
    RS.i_gt(ga.nu(target, 3), (4608 + 6656 : U32)),
    lanternward.wsv(s, "face", ga.pair_i(fx, fy)), fx, fy, target)

def lanternward.boss.step_boss.rest.walk?(far: Bool, +s: J.Val, +fx: U32, +fy: U32, +target: J.Val) -> J.Val:
  match far:
    case False{}:
      lanternward.boss.step_boss.rest.tick(s, target)
    case True{}:
      lanternward.boss.step_boss.rest.step(
        (lanternward.wu(s, "x") + RS.i_div((fx * 128 : U32), 256) : U32),
        (lanternward.wu(s, "y") + RS.i_div((fy * 128 : U32), 256) : U32), s, target)

def lanternward.boss.step_boss.rest.step?(+nx: U32, +ny: U32, +s: J.Val, +target: J.Val) -> J.Val:
  lanternward.boss.step_boss.rest.tick(
    lanternward.boss.step_boss.rest.move(lanternward.props.Session.clear(s, nx, ny, 2304), s, nx, ny), target)

def lanternward.boss.step_boss.rest.move?(ok: Bool, +s: J.Val, +nx: U32, +ny: U32) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.wsi(lanternward.wsi(s, "x", nx), "y", ny)

def lanternward.boss.step_boss.rest.tick?(+s: J.Val, +target: J.Val) -> J.Val:
  lanternward.boss.step_boss.rest.attack(
    U32.is_le(lanternward.wu(s, "t"), 1), lanternward.ws(s, "t", RS.u_sat_sub(lanternward.wu(s, "t"), 1)), target)

def lanternward.boss.step_boss.rest.attack?(zero: Bool, +s: J.Val, +target: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.begin_attack(s, ga.nu(target, 1), ga.nu(target, 2))

def lanternward.boss.step_boss.tell?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.boss.step_boss.tell.go(
        lanternward.boss.atk_ticks(ga.s(J.get(s, "$w"), "atk")),
        lanternward.wsi(lanternward.ws(lanternward.ws(lanternward.wsn(s, "st", "Strike"), "struck", 0), "t", lanternward.boss.atk_ticks(ga.s(J.get(s, "$w"), "atk"))), "sweep", 0))

def lanternward.boss.atk_ticks?(+atk: String) -> U32:
  Bool.pick(U32, String.eq(atk, "Beam"), 36, Bool.pick(U32, String.eq(atk, "Slam"), 30, 12))

def lanternward.boss.step_boss.tell.go?(+total: U32, +s: J.Val) -> J.Val:
  lanternward.sound(
    lanternward.boss.step_boss.tell.summon(lanternward.wis(s, "atk", "Summon"), s),
    Bool.pick(String, lanternward.wis(s, "atk", "Beam"), "shot_arcane", Bool.pick(String, lanternward.wis(s, "atk", "Slam"), "rock_land", "crack_open")))

def lanternward.boss.step_boss.tell.summon?(yes: Bool, +s: J.Val) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.summon(s)

def lanternward.boss.step_boss.recover?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.ws(
        lanternward.ws(lanternward.wsn(s, "st", "Rest"), "t", (60 - 12 * lanternward.wu(s, "phase") : U32)),
        "n", U32.and((lanternward.wu(s, "n") + 1 : U32), 255))

def lanternward.boss.step_boss.stagger?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.ws(lanternward.wsn(s, "st", "Rest"), "t", 30)

# The strike: sweep a beam, or send a ring along the ground.

def lanternward.boss.step_boss.strike?(+s: J.Val) -> J.Val:
  lanternward.boss.step_boss.strike.go(
    (lanternward.boss.atk_ticks(ga.s(J.get(s, "$w"), "atk")) - lanternward.wu(s, "t") : U32), s)

def lanternward.boss.step_boss.strike.go?(+elapsed: U32, +s: J.Val) -> J.Val:
  +s1 = lanternward.boss.step_boss.strike.kind(
    Bool.pick(U32, lanternward.wis(s, "atk", "Beam"), 1, Bool.pick(U32, lanternward.wis(s, "atk", "Slam"), 2, 0)),
    elapsed, s)
  lanternward.boss.step_boss.strike.end(ga.b(s1, "$parried"), s1)

def lanternward.boss.step_boss.strike.kind?(+code: U32, +elapsed: U32, +s: J.Val) -> J.Val:
  match code:
    case 1:
      lanternward.boss.step_boss.beam(s, elapsed, 0)
    case 2:
      lanternward.boss.step_boss.slam(s, elapsed)
    case _:
      s

# One beam of the sweep (and its twin in the last phase): `b` is 0 or 1.

def lanternward.boss.step_boss.beam?(+s: J.Val, +elapsed: U32, +b: U32) -> J.Val:
  lanternward.boss.step_boss.beam.go(
    U32.is_lt(b, Bool.pick(U32, U32.is_ge(lanternward.wu(s, "phase"), 2), 2, 1)),
    s, elapsed, b)

def lanternward.boss.step_boss.beam.go?(more: Bool, +s: J.Val, +elapsed: U32, +b: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.boss.step_boss.beam.one(
        (lanternward.wu(s, "aim") - 50 + RS.i_div((100 * elapsed : U32), 36) + 180 * b : U32), s, elapsed, b)

def lanternward.boss.step_boss.beam.one?(+a: U32, +s: J.Val, +elapsed: U32, +b: U32) -> J.Val:
  lanternward.boss.step_boss.beam.hit(
    lanternward.world.dir(a), s, elapsed, b)

def lanternward.boss.step_boss.beam.hit?(d: U32 & U32, +s: J.Val, +elapsed: U32, +b: U32) -> J.Val:
  (+dx, +dy) = d
  lanternward.boss.step_boss.beam.line(
    lanternward.variant("Capsule",
      ga.si(ga.si(ga.si(ga.si(ga.si(J.obj(), "ax", lanternward.wu(s, "x")), "ay", lanternward.wu(s, "y")),
        "bx", (lanternward.wu(s, "x") + dx * 150 : U32)), "by", (lanternward.wu(s, "y") + dy * 150 : U32)), "r", 1536)),
    s, elapsed, b)

def lanternward.boss.step_boss.beam.line?(+line: J.Val, +s: J.Val, +elapsed: U32, +b: U32) -> J.Val:
  lanternward.boss.step_boss.beam(
    lanternward.boss.Session.warden_hits(
      lanternward.boss.step_boss.beam.danger(U32.is_eq(b, 0), s, line), line, 4, "Beam"),
    elapsed, (b + 1 : U32))

def lanternward.boss.step_boss.beam.danger?(first: Bool, +s: J.Val, +line: J.Val) -> J.Val:
  match first:
    case True{}:
      lanternward.wsv(s, "danger", line)
    case False{}:
      s

def lanternward.boss.step_boss.slam?(+s: J.Val, +elapsed: U32) -> J.Val:
  +s1 = lanternward.wsv(s, "danger", lanternward.variant("Circle", ga.si(ga.si(ga.si(J.obj(), "x", lanternward.wu(s, "x")), "y", lanternward.wu(s, "y")), "r", 33792)))
  lanternward.boss.step_boss.slam.wave(
    U32.is_eq(elapsed, 0),
    lanternward.boss.Session.warden_hits(
      s1,
      lanternward.variant("Ring",
        ga.si(ga.si(ga.si(ga.si(J.obj(), "x", lanternward.wu(s, "x")), "y", lanternward.wu(s, "y")),
          "r", ((12 + 4 * elapsed : U32) * 256 : U32)), "w", 1536)),
      4, "Low"))

def lanternward.boss.step_boss.slam.wave?(first: Bool, +s: J.Val) -> J.Val:
  match first:
    case False{}:
      s
    case True{}:
      lanternward.props.Session.fx(s, "Wave", lanternward.wu(s, "x"), lanternward.wu(s, "y"), 0)

def lanternward.boss.step_boss.strike.end?(parried: Bool, +s: J.Val) -> J.Val:
  match parried:
    case True{}:
      ga.su(
        lanternward.stop(
          lanternward.sound(
            lanternward.wsv(lanternward.wsv(lanternward.ws(lanternward.wsn(s, "st", "Stagger"), "t", 75), "danger", J.JS{"None"}), "lit", lanternward.zeros6()),
            "boss_rage"),
          "Phase"),
        "shake", 8)
    case False{}:
      lanternward.boss.step_boss.strike.tick(
        U32.is_le(lanternward.wu(s, "t"), 1), lanternward.ws(s, "t", RS.u_sat_sub(lanternward.wu(s, "t"), 1)))

def lanternward.boss.step_boss.strike.tick?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.wsv(
        lanternward.ws(lanternward.wsn(s, "st", "Recover"), "t", Bool.pick(U32, lanternward.wis(s, "atk", "Summon"), 30, 44)),
        "danger", J.JS{"None"})

# Choose the Warden's next attack and show where it will land.

def lanternward.boss.pattern?(+phase: U32, +n: U32) -> String:
  match phase:
    case 0:
      lanternward.boss.pattern.pick(U32.mod(n, 4), "Slam", "Beam", "Beam", "Slam", "Slam")
    case 1:
      lanternward.boss.pattern.pick(U32.mod(n, 4), "Beam", "Summon", "Slam", "Beam", "Beam")
    case _:
      lanternward.boss.pattern.pick(U32.mod(n, 5), "Beam", "Slam", "Summon", "Beam", "Slam")

def lanternward.boss.pattern.pick?(+i: U32, +a: String, +b: String, +c: String, +d: String, +e: String) -> String:
  match i:
    case 0:
      a
    case 1:
      b
    case 2:
      c
    case 3:
      d
    case _:
      e

def lanternward.boss.Session.begin_attack?(+s: J.Val, +ax: U32, +ay: U32) -> J.Val:
  lanternward.boss.begin_attack.go(
    lanternward.boss.pattern(lanternward.wu(s, "phase"), lanternward.wu(s, "n")),
    lanternward.world.unit((ax - lanternward.wu(s, "x") : U32), (ay - lanternward.wu(s, "y") : U32)),
    s, ax, ay)

def lanternward.boss.begin_attack.go?(+atk: String, face: U32 & U32, +s: J.Val, +ax: U32, +ay: U32) -> J.Val:
  (+fx, +fy) = face
  lanternward.boss.begin_attack.danger(
    atk,
    lanternward.wsv(
      lanternward.ws(
        lanternward.ws(
          lanternward.wsn(lanternward.wsn(s, "atk", atk), "st", "Tell"), "t",
          lanternward.Rules.tell_for(J.get(s, "$r"), lanternward.seat_count(s))),
        "struck", 0),
      "face", ga.pair_i(fx, fy)),
    ax, ay)

def lanternward.boss.begin_attack.danger?(+atk: String, +s: J.Val, +ax: U32, +ay: U32) -> J.Val:
  lanternward.boss.begin_attack.danger.kind(
    Bool.pick(U32, String.eq(atk, "Beam"), 1, Bool.pick(U32, String.eq(atk, "Slam"), 2, 0)), s, ax, ay)

def lanternward.boss.begin_attack.danger.kind?(+code: U32, +s: J.Val, +ax: U32, +ay: U32) -> J.Val:
  match code:
    case 1:
      lanternward.boss.begin_attack.fan(lanternward.world.angle_of((ax - lanternward.wu(s, "x") : U32), (ay - lanternward.wu(s, "y") : U32)), s)
    case 2:
      lanternward.wsv(s, "danger", lanternward.variant("Circle", ga.si(ga.si(ga.si(J.obj(), "x", lanternward.wu(s, "x")), "y", lanternward.wu(s, "y")), "r", 33792)))
    case _:
      lanternward.wsv(s, "danger", J.JS{"None"})

def lanternward.boss.begin_attack.fan?(+aim: U32, +s: J.Val) -> J.Val:
  lanternward.wsv(
    lanternward.wsi(s, "aim", aim), "danger",
    lanternward.variant("Fan",
      ga.si(ga.si(ga.si(ga.si(ga.si(J.obj(), "x", lanternward.wu(s, "x")), "y", lanternward.wu(s, "y")), "deg", aim), "half", 56), "r", 38400)))

# The Warden's blow on every knight in `line` it has not yet struck. A parry
# sets `$parried`.

def lanternward.boss.Session.warden_hits?(+s: J.Val, +line: J.Val, +dmg: U32, +kind: String) -> J.Val:
  lanternward.boss.warden_hits.ids(ga.ids(J.get(s, "seats")), s, line, dmg, kind)

def lanternward.boss.warden_hits.ids?(+ids: List<&2, U32>, +s: J.Val, +line: J.Val, +dmg: U32, +kind: String) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.boss.warden_hits.ids(t, lanternward.boss.warden_hits.seat(s, id, line, dmg, kind), line, dmg, kind)

def lanternward.boss.warden_hits.seat?(+s: J.Val, +id: U32, +line: J.Val, +dmg: U32, +kind: String) -> J.Val:
  +k = ga.at(J.get(s, "seats"), id)
  +bit = U32.shln(1, U32.to_nat(RS.u_min(id, 7)))
  lanternward.boss.warden_hits.hit(
    Bool.and(
      Bool.and(U32.is_eq(U32.and(lanternward.wu(s, "struck"), bit), 0), lanternward.kit.Seat.up(k)),
      lanternward.foes.Danger.hits(line, ga.u(k, "x"), ga.u(k, "y"), 1280)),
    s, id, dmg, kind, bit)

def lanternward.boss.warden_hits.hit?(yes: Bool, +s: J.Val, +id: U32, +dmg: U32, +kind: String, +bit: U32) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.boss.warden_hits.after(
        lanternward.blow.Session.blow(s, id, dmg, lanternward.wu(s, "x"), lanternward.wu(s, "y"), kind), bit)

def lanternward.boss.warden_hits.after?(+s: J.Val, +bit: U32) -> J.Val:
  lanternward.boss.warden_hits.res(ga.s(s, "$ret"), J.del(s, "$ret"), bit)

def lanternward.boss.warden_hits.res?(+out: String, +s: J.Val, +bit: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(out, "Dodged"), s,
    Bool.pick(J.Val, String.eq(out, "Parried"),
      ga.sb(lanternward.ws(s, "struck", U32.or(lanternward.wu(s, "struck"), bit)), "$parried", True{}),
      lanternward.ws(s, "struck", U32.or(lanternward.wu(s, "struck"), bit))))

# Snuffers climb out of the fissures (hooded, in the last phase).

def lanternward.boss.Session.summon?(+s: J.Val) -> J.Val:
  lanternward.boss.summon.marks(
    s, lanternward.world.Plan.find(4, 102),
    RS.u_min((2 + U32.div((RS.u_max(lanternward.seat_count(s), 1) - 1 : U32), 2) : U32), 4),
    U32.is_ge(lanternward.wu(s, "phase"), 2))

def lanternward.boss.summon.marks?(+s: J.Val, +marks: List<&2, U32>, +n: U32, +hooded: Bool) -> J.Val:
  match marks:
    case Nil{}:
      s
    case Con{+m, +t}:
      lanternward.boss.summon.marks(
        lanternward.boss.summon.one(U32.is_gt(n, 0), s, m, hooded), t, RS.u_sat_sub(n, 1), hooded)

def lanternward.boss.summon.one?(go: Bool, +s: J.Val, +m: U32, +hooded: Bool) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.boss.summon.crawl(
        lanternward.boss.summon.fissure(
          lanternward.foes.Session.spawn_foe(s, "Snuffer", lanternward.world.middle(lanternward.world.packed_x(m)), lanternward.world.middle(lanternward.world.packed_y(m)), hooded),
          lanternward.boss.Session.prop_at(s, "Fissure", lanternward.world.packed_x(m), lanternward.world.packed_y(m))))

def lanternward.boss.summon.fissure?(+s: J.Val, +i: J.Val) -> J.Val:
  match i:
    case J.JNull{}:
      s
    case _:
      lanternward.es(s, "props", J.u32(i), "t", 40)

def lanternward.boss.summon.crawl?(+s: J.Val) -> J.Val:
  lanternward.boss.summon.crawl.go(U32.is_gt(lanternward.len(s, "foes"), 0), s)

def lanternward.boss.summon.crawl.go?(any: Bool, +s: J.Val) -> J.Val:
  match any:
    case False{}:
      s
    case True{}:
      lanternward.es(
        lanternward.eset(s, "foes", (lanternward.len(s, "foes") - 1 : U32), "st", J.JS{"Drift"}),
        "foes", (lanternward.len(s, "foes") - 1 : U32), "t", 30)

# The index of the first prop of this kind at this tile, or null.

def lanternward.boss.prop_at.go?(+ps: List<&2, J.Val>, +kind: String, +tx: U32, +ty: U32, +i: U32) -> J.Val:
  match ps:
    case Nil{}:
      J.JNull{}
    case Con{+p, +t}:
      lanternward.boss.prop_at.hit(
        Bool.and(String.eq(ga.s(p, "kind"), kind), Bool.and(U32.is_eq(ga.u(p, "tx"), tx), U32.is_eq(ga.u(p, "ty"), ty))),
        t, kind, tx, ty, i)

def lanternward.boss.prop_at.hit?(hit: Bool, +t: List<&2, J.Val>, +kind: String, +tx: U32, +ty: U32, +i: U32) -> J.Val:
  match hit:
    case True{}:
      J.JU{i}
    case False{}:
      lanternward.boss.prop_at.go(t, kind, tx, ty, (i + 1 : U32))

def lanternward.boss.Session.prop_at?(+s: J.Val, +kind: String, +tx: U32, +ty: U32) -> J.Val:
  lanternward.boss.prop_at.go(ga.items(s, "props"), kind, tx, ty, 0)

# The last room: the heart container, then the Beacon.

def lanternward.boss.Session.stage_warden?(+s: J.Val) -> J.Val:
  lanternward.boss.stage_warden.go(U32.is_eq(ga.u(s, "stage"), 3), s)

def lanternward.boss.stage_warden.go?(on: Bool, +s: J.Val) -> J.Val:
  match on:
    case False{}:
      s
    case True{}:
      lanternward.boss.stage_warden.beacon(
        lanternward.boss.stage_warden.holders(ga.ids(J.get(s, "seats")), J.get(s, "seats")),
        s, lanternward.find_prop_index(s, "Beacon"))

def lanternward.boss.stage_warden.holders?(+ids: List<&2, U32>, +seats: J.Val) -> U32:
  match ids:
    case Nil{}:
      0
    case Con{+id, +t}:
      (Bool.to_u32(
        Bool.and(
          Bool.and(lanternward.kit.Seat.up(ga.at(seats, id)), lanternward.is(ga.at(seats, id), "act", "Free")),
          Bool.and(
            U32.is_ne(U32.and(ga.u(ga.at(seats, id), "held"), 8), 0),
            lanternward.tag_is(ga.v(ga.at(seats, id), "ctx"), "Relight")))) + lanternward.boss.stage_warden.holders(t, seats) : U32)

def lanternward.boss.stage_warden.beacon?(+holders: U32, +s: J.Val, +i: J.Val) -> J.Val:
  match i:
    case J.JNull{}:
      s
    case _:
      lanternward.boss.stage_warden.tick(
        lanternward.es(s, "props", J.u32(i), "t",
          Bool.pick(U32, U32.is_gt(holders, 0),
            RS.u_min((lanternward.eu(s, "props", J.u32(i), "t") + holders : U32), 65535),
            RS.u_sat_sub(lanternward.eu(s, "props", J.u32(i), "t"), 1))),
        J.u32(i))

def lanternward.boss.stage_warden.tick?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.boss.stage_warden.relit(
    Bool.and(U32.is_ge(lanternward.eu(s, "props", i, "t"), 90), U32.is_eq(lanternward.eu(s, "props", i, "on"), 0)), s, i)

def lanternward.boss.stage_warden.relit?(yes: Bool, +s: J.Val, +i: U32) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.won(
        lanternward.sound(
          lanternward.sound(
            lanternward.props.Session.fx(
              ga.su(ga.su(ga.sv(ga.su(lanternward.es(s, "props", i, "on", 1), "stage", 4), "done", J.JU{ga.u(s, "elapsed")}), "flash", 12), "shake", 8),
              "Solved", lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 1),
            "circle_toll"),
          "wish_grant"))

# How long the shutter has left open, 0 if shut.

def lanternward.boss.min_open?(+xs: List<&2, J.Val>, +now: U32, +best: J.Val) -> J.Val:
  match xs:
    case Nil{}:
      best
    case Con{+h, +t}:
      lanternward.boss.min_open(t, now,
        Bool.pick(J.Val, U32.is_gt(J.u32(h), 0),
          Bool.pick(J.Val, Bool.or(J.is_null(best), U32.is_lt(RS.u_sat_sub((J.u32(h) + 90 : U32), now), J.u32(best))),
            J.JU{RS.u_sat_sub((J.u32(h) + 90 : U32), now)}, best),
          best))

def lanternward.boss.Warden.open_for?(+w: J.Val, +now: U32, +seats: U32) -> U32:
  Bool.pick(U32, lanternward.boss.Warden.exposed(w, now, seats),
    Bool.pick(U32, lanternward.is(w, "st", "Stagger"), ga.u(w, "t"),
      J.u32(lanternward.boss.min_open(ga.items(w, "lit"), now, J.JNull{}))),
    0)

def lanternward.bot.ENGAGE?() -> U32:
  38400

# `sign(v, dead)` as an i8 held in a U32: 1, 0 or -1.

def lanternward.bot.sign?(+v: U32, +dead: U32) -> U32:
  Bool.pick(U32, RS.i_gt(v, dead), 1, Bool.pick(U32, RS.i_lt(v, RS.i_neg(dead)), 4294967295, 0))

def lanternward.bot.i_move?(+i: J.Val, +mx: U32, +my: U32) -> J.Val:
  ga.si(ga.si(i, "move_x", mx), "move_y", my)

def lanternward.bot.i_aim?(+i: J.Val, +ax: U32, +ay: U32) -> J.Val:
  ga.si(ga.si(i, "aim_x", ax), "aim_y", ay)

# Whole-key set: drop `ult` / `interact` when false (serde's skip).

def lanternward.bot.norm?(+i: J.Val) -> J.Val:
  ga.sb_skip(ga.sb_skip(i, "interact", ga.b(i, "interact")), "ult", ga.b(i, "ult"))

def lanternward.bot.idle?() -> J.Val:
  rules.Input.default()

# The foe nearest the knight among living snuffers: null, or the foe.

def lanternward.bot.nearest_snuffer.go?(+fs: List<&2, J.Val>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      lanternward.bot.nearest_snuffer.take(
        Bool.and(lanternward.foes.Foe.alive(f), lanternward.is(f, "kind", "Snuffer")),
        lanternward.world.dist((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)),
        f, t, me, best, bd)

def lanternward.bot.nearest_snuffer.take?(+ok: Bool, +d: U32, +f: J.Val, +t: List<&2, J.Val>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  lanternward.bot.nearest_snuffer.go(t, me,
    Bool.pick(J.Val, Bool.and(ok, Bool.or(J.is_null(best), RS.i_lt(d, bd))), f, best),
    Bool.pick(U32, Bool.and(ok, Bool.or(J.is_null(best), RS.i_lt(d, bd))), d, bd))

def lanternward.bot.nearest_snuffer?(+s: J.Val, +me: J.Val, +alive_only: Bool) -> J.Val:
  lanternward.bot.nearest_snuffer.go(ga.items(s, "foes"), me, J.JNull{}, 0)

# The living foe of any kind nearest the knight (room 0 and the dark).

def lanternward.bot.nearest_foe.go?(+fs: List<&2, J.Val>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      lanternward.bot.nearest_foe.take(
        lanternward.foes.Foe.alive(f),
        lanternward.world.dist((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)),
        f, t, me, best, bd)

def lanternward.bot.nearest_foe.take?(+ok: Bool, +d: U32, +f: J.Val, +t: List<&2, J.Val>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  lanternward.bot.nearest_foe.go(t, me,
    Bool.pick(J.Val, Bool.and(ok, Bool.or(J.is_null(best), RS.i_lt(d, bd))), f, best),
    Bool.pick(U32, Bool.and(ok, Bool.or(J.is_null(best), RS.i_lt(d, bd))), d, bd))

# What bot `id` presses this tick.

def lanternward.bot.Session.bot_input?(+s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  lanternward.bot.bot_input.me(ga.has_key(J.get(s, "seats"), id), s, id, r)

def lanternward.bot.bot_input.me?(has: Bool, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match has:
    case False{}:
      lanternward.bot.idle()
    case True{}:
      lanternward.bot.bot_input.up(ga.at(J.get(s, "seats"), id), s, id, r)

def lanternward.bot.bot_input.up?(+me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  lanternward.bot.bot_input.up.go(
    Bool.or(Bool.not(lanternward.kit.Seat.up(me)), Bool.or(lanternward.is(me, "act", "Item"), lanternward.is(me, "act", "Fall"))),
    me, s, id, r)

def lanternward.bot.bot_input.up.go?(down: Bool, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match down:
    case True{}:
      lanternward.bot.idle()
    case False{}:
      lanternward.bot.bot_input.done(J.is_null(J.get(s, "done")), me, s, id, r)

def lanternward.bot.bot_input.done?(running: Bool, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match running:
    case False{}:
      lanternward.bot.bot_finale(s)
    case True{}:
      lanternward.bot.bot_input.boss(
        Bool.and(
          Bool.not(J.is_null(J.get(s, "boss"))),
          Bool.and(U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0), Bool.not(lanternward.is(J.get(s, "boss"), "st", "Asleep")))),
        me, s, id, r)

def lanternward.bot.bot_input.boss?(awake: Bool, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match awake:
    case True{}:
      lanternward.bot.bot_boss(s, id, me, J.get(s, "boss"), r)
    case False{}:
      lanternward.bot.bot_input.foe(lanternward.bot.nearest_snuffer(s, me, True{}), me, s, id, r)

def lanternward.bot.bot_input.foe?(+foe: J.Val, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match foe:
    case J.JNull{}:
      lanternward.bot.bot_input.friend(lanternward.bot.first_down(ga.ids(J.get(s, "seats")), J.get(s, "seats"), ga.u(me, "id")), me, s, id, r)
    case _:
      lanternward.bot.bot_input.engage(
        RS.i_lt(lanternward.world.dist((ga.u(foe, "x") - ga.u(me, "x") : U32), (ga.u(foe, "y") - ga.u(me, "y") : U32)), 38400),
        foe, me, s, id, r)

def lanternward.bot.bot_input.engage?(near: Bool, +foe: J.Val, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match near:
    case True{}:
      lanternward.bot.bot_fight(s, id, me, foe, r)
    case False{}:
      lanternward.bot.bot_goto(s, me, ga.u(foe, "x"), ga.u(foe, "y"), 30, False{})

# The first other seat on the ground: null, or its seat object.

def lanternward.bot.first_down?(+ids: List<&2, U32>, +seats: J.Val, +me_id: U32) -> J.Val:
  match ids:
    case Nil{}:
      J.JNull{}
    case Con{+k, +t}:
      lanternward.bot.first_down.hit(
        Bool.and(U32.is_ne(ga.u(ga.at(seats, k), "id"), me_id), lanternward.is(ga.at(seats, k), "act", "Down")),
        ga.at(seats, k), t, seats, me_id)

def lanternward.bot.first_down.hit?(hit: Bool, +o: J.Val, +t: List<&2, U32>, +seats: J.Val, +me_id: U32) -> J.Val:
  match hit:
    case True{}:
      o
    case False{}:
      lanternward.bot.first_down(t, seats, me_id)

# A friend on the ground, and nothing about: lift them.

def lanternward.bot.bot_input.friend?(+o: J.Val, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match o:
    case J.JNull{}:
      lanternward.bot.bot_objective(s, id, me, r)
    case _:
      lanternward.bot.bot_input.lift(
        lanternward.bot.bot_goto(s, me, ga.u(o, "x"), ga.u(o, "y"), 10, False{}),
        RS.i_lt(lanternward.world.dist((ga.u(o, "x") - ga.u(me, "x") : U32), (ga.u(o, "y") - ga.u(me, "y") : U32)), 4608))

def lanternward.bot.bot_input.lift?(+i: J.Val, close: Bool) -> J.Val:
  lanternward.bot.norm(Bool.pick(J.Val, close, ga.sb(i, "interact", True{}), i))

# ---- walking ----

# Tiles a prop blocks (the Rust asks every prop of each tile's middle): the
# five-by-five tiles around each prop, `blocks` asked of each, as a set keyed
# by tile index.

def lanternward.bot.blocked_set.tile?(+p: J.Val, +bs: J.Val, +x: U32, +y: U32, +w: U32, +h: U32) -> J.Val:
  Bool.pick(J.Val,
    Bool.and(
      Bool.and(RS.i_ge(x, 0), RS.i_lt(x, w)),
      Bool.and(
        Bool.and(RS.i_ge(y, 0), RS.i_lt(y, h)),
        lanternward.props.Prop.blocks(p, lanternward.world.middle(x), lanternward.world.middle(y)))),
    ga.sb(bs, U32.show((y * w + x : U32)), True{}),
    bs)

def lanternward.bot.blocked_set.around?(+p: J.Val, +bs: J.Val, +k: U32, +w: U32, +h: U32) -> J.Val:
  lanternward.bot.blocked_set.around.go(U32.is_ge(k, 25), p, bs, k, w, h)

def lanternward.bot.blocked_set.around.go?(done: Bool, +p: J.Val, +bs: J.Val, +k: U32, +w: U32, +h: U32) -> J.Val:
  match done:
    case True{}:
      bs
    case False{}:
      lanternward.bot.blocked_set.around(p,
        lanternward.bot.blocked_set.tile(p, bs,
          (ga.u(p, "tx") + U32.mod(k, 5) - 2 : U32), (ga.u(p, "ty") + U32.div(k, 5) - 2 : U32), w, h),
        (k + 1 : U32), w, h)

def lanternward.bot.blocked_set?(+ps: List<&2, J.Val>, +bs: J.Val, +w: U32, +h: U32) -> J.Val:
  match ps:
    case Nil{}:
      bs
    case Con{+p, +t}:
      lanternward.bot.blocked_set(t, lanternward.bot.blocked_set.around(p, bs, 0, w, h), w, h)

# Whether a body can enter tile (x, y): inside the room, the goal itself, or
# open ground no prop stops.

def lanternward.bot.open?(+env: J.Val, +x: U32, +y: U32) -> Bool:
  Bool.and(
    Bool.and(RS.i_ge(x, 0), RS.i_lt(x, ga.u(env, "w"))),
    Bool.and(
      Bool.and(RS.i_ge(y, 0), RS.i_lt(y, ga.u(env, "h"))),
      Bool.or(
        Bool.and(U32.is_eq(x, ga.u(env, "tx")), U32.is_eq(y, ga.u(env, "ty"))),
        lanternward.bot.open.ground(env, x, y))))

def lanternward.bot.open.ground?(+env: J.Val, +x: U32, +y: U32) -> Bool:
  lanternward.bot.open.floor(lanternward.world.Plan.at(ga.u(env, "room"), x, y), env, x, y)

def lanternward.bot.open.floor?(+c: U32, +env: J.Val, +x: U32, +y: U32) -> Bool:
  Bool.and(
    Bool.and(Bool.not(lanternward.world.wall(c)), U32.is_ne(c, 61)),
    Bool.not(J.bool(J.get(ga.v(env, "blocked"), U32.show((y * ga.u(env, "w") + x : U32))))))

# Visit neighbour (nx, ny) of tile `cur`: if unseen and open, remember where
# we came from and queue it.

def lanternward.bot.nb?(+st: J.Val, +cur: U32, +nx: U32, +ny: U32, +env: J.Val) -> J.Val:
  lanternward.bot.nb.go(
    Bool.and(
      Bool.and(RS.i_ge(nx, 0), RS.i_lt(nx, ga.u(env, "w"))),
      Bool.and(RS.i_ge(ny, 0), RS.i_lt(ny, ga.u(env, "h")))),
    st, cur, nx, ny, env)

def lanternward.bot.nb.go?(inside: Bool, +st: J.Val, +cur: U32, +nx: U32, +ny: U32, +env: J.Val) -> J.Val:
  match inside:
    case False{}:
      st
    case True{}:
      lanternward.bot.nb.seen(
        J.has(ga.v(st, "came"), U32.show((ny * ga.u(env, "w") + nx : U32))), st, cur, nx, ny, env)

def lanternward.bot.nb.seen?(seen: Bool, +st: J.Val, +cur: U32, +nx: U32, +ny: U32, +env: J.Val) -> J.Val:
  match seen:
    case True{}:
      st
    case False{}:
      lanternward.bot.nb.push(lanternward.bot.open(env, nx, ny), st, cur, (ny * ga.u(env, "w") + nx : U32))

def lanternward.bot.nb.push?(open: Bool, +st: J.Val, +cur: U32, +n: U32) -> J.Val:
  match open:
    case False{}:
      st
    case True{}:
      ga.sv(ga.sv(st, "came", ga.su(ga.v(st, "came"), U32.show(n), cur)), "back", J.JA{Con{J.JU{n}, ga.items(st, "back")}})

# Breadth-first from the queue's front; `came` maps a tile to the one before.

def lanternward.bot.bfs?(+k: Nat, +front: List<&2, J.Val>, +st: J.Val, +env: J.Val) -> J.Val:
  match k:
    case 0n:
      J.get(st, "came")
    case 1n+p:
      lanternward.bot.bfs.pop(front, st, env, p)

def lanternward.bot.bfs.pop?(+front: List<&2, J.Val>, +st: J.Val, +env: J.Val, +p: Nat) -> J.Val:
  match front:
    case Nil{}:
      lanternward.bot.bfs.refill(List.reverse(&2, J.Val, ga.items(st, "back")), ga.sv(st, "back", J.JA{Nil{}}), env, p)
    case Con{+h, +t}:
      lanternward.bot.bfs.expand(J.u32(h), t, st, env, p)

def lanternward.bot.bfs.refill?(+front: List<&2, J.Val>, +st: J.Val, +env: J.Val, +p: Nat) -> J.Val:
  match front:
    case Nil{}:
      J.get(st, "came")
    case Con{+h, +t}:
      lanternward.bot.bfs.expand(J.u32(h), t, st, env, p)

def lanternward.bot.bfs.expand?(+cur: U32, +t: List<&2, J.Val>, +st: J.Val, +env: J.Val, +p: Nat) -> J.Val:
  lanternward.bot.bfs.next(U32.is_eq(cur, ga.u(env, "to")), cur, t, st, env, p)

def lanternward.bot.bfs.next?(done: Bool, +cur: U32, +t: List<&2, J.Val>, +st: J.Val, +env: J.Val, +p: Nat) -> J.Val:
  match done:
    case True{}:
      J.get(st, "came")
    case False{}:
      lanternward.bot.bfs(p, t,
        lanternward.bot.nb(
          lanternward.bot.nb(
            lanternward.bot.nb(
              lanternward.bot.nb(st, cur, (U32.mod(cur, ga.u(env, "w")) + 1 : U32), U32.div(cur, ga.u(env, "w")), env),
              cur, (U32.mod(cur, ga.u(env, "w")) - 1 : U32), U32.div(cur, ga.u(env, "w")), env),
            cur, U32.mod(cur, ga.u(env, "w")), (U32.div(cur, ga.u(env, "w")) + 1 : U32), env),
          cur, U32.mod(cur, ga.u(env, "w")), (U32.div(cur, ga.u(env, "w")) - 1 : U32), env),
        env)

# Back from the goal to the tile after ours.

def lanternward.bot.back_path?(+came: J.Val, +at: U32, +from: U32, +k: Nat) -> U32:
  match k:
    case 0n:
      at
    case 1n+p:
      lanternward.bot.back_path.go(J.has(came, U32.show(at)), came, at, from, p)

def lanternward.bot.back_path.go?(has: Bool, +came: J.Val, +at: U32, +from: U32, +p: Nat) -> U32:
  match has:
    case False{}:
      at
    case True{}:
      lanternward.bot.back_path.step(ga.u(came, U32.show(at)), came, at, from, p)

def lanternward.bot.back_path.step?(+prev: U32, +came: J.Val, +at: U32, +from: U32, +p: Nat) -> U32:
  lanternward.bot.back_path.cont(U32.is_eq(prev, from), prev, came, at, from, p)

def lanternward.bot.back_path.cont?(stop: Bool, +prev: U32, +came: J.Val, +at: U32, +from: U32, +p: Nat) -> U32:
  match stop:
    case True{}:
      at
    case False{}:
      lanternward.bot.back_path(came, prev, from, p)

# The way to walk now to get to tile `to` from where the knight is: [mx, my].

def lanternward.bot.bot_step?(+s: J.Val, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.bot.bot_step.go(
    lanternward.world.Plan.w(ga.u(s, "room")), lanternward.world.Plan.h(ga.u(s, "room")),
    lanternward.world.tile_of(ga.u(me, "x")), lanternward.world.tile_of(ga.u(me, "y")), s, me, tx, ty)

def lanternward.bot.bot_step.go?(+w: U32, +h: U32, +fx: U32, +fy: U32, +s: J.Val, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.bot.bot_step.inside(
    Bool.and(Bool.and(RS.i_ge(fx, 0), RS.i_ge(fy, 0)), Bool.and(RS.i_lt(fx, w), RS.i_lt(fy, h))),
    w, h, fx, fy, s, me, tx, ty)

def lanternward.bot.bot_step.inside?(ok: Bool, +w: U32, +h: U32, +fx: U32, +fy: U32, +s: J.Val, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  match ok:
    case False{}:
      ga.pair_i(0, 0)
    case True{}:
      lanternward.bot.bot_step.same(Bool.and(U32.is_eq(fx, tx), U32.is_eq(fy, ty)), w, h, fx, fy, s, me, tx, ty)

def lanternward.bot.bot_step.same?(same: Bool, +w: U32, +h: U32, +fx: U32, +fy: U32, +s: J.Val, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  match same:
    case True{}:
      ga.pair_i(
        lanternward.bot.sign((lanternward.world.middle(tx) - ga.u(me, "x") : U32), 512),
        lanternward.bot.sign((lanternward.world.middle(ty) - ga.u(me, "y") : U32), 512))
    case False{}:
      lanternward.bot.bot_step.search(w, h, fx, fy, me, tx, ty,
        ga.sv(ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(J.obj(), "room", ga.u(s, "room")), "w", w), "h", h), "tx", tx), "ty", ty), "to", (ty * w + tx : U32)), "from", (fy * w + fx : U32)),
          "blocked", lanternward.bot.blocked_set(ga.items(s, "props"), J.obj(), w, h)))

def lanternward.bot.bot_step.search?(+w: U32, +h: U32, +fx: U32, +fy: U32, +me: J.Val, +tx: U32, +ty: U32, +env: J.Val) -> J.Val:
  lanternward.bot.bot_step.reached(
    lanternward.bot.bfs(U32.to_nat((w * h * 2 + 2 : U32)), [J.JU{(fy * w + fx : U32)}],
      ga.sv(ga.sv(J.obj(), "came", ga.su(J.obj(), U32.show((fy * w + fx : U32)), (fy * w + fx : U32))), "back", J.JA{Nil{}}), env),
    w, h, fx, fy, me, tx, ty)

def lanternward.bot.bot_step.reached?(+came: J.Val, +w: U32, +h: U32, +fx: U32, +fy: U32, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.bot.bot_step.walk(
    Bool.and(
      Bool.and(RS.i_ge(tx, 0), RS.i_ge(ty, 0)),
      Bool.and(Bool.and(RS.i_lt(tx, w), RS.i_lt(ty, h)), J.has(came, U32.show((ty * w + tx : U32))))),
    came, w, fx, fy, me, tx, ty)

def lanternward.bot.bot_step.walk?(ok: Bool, +came: J.Val, +w: U32, +fx: U32, +fy: U32, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  match ok:
    case False{}:
      ga.pair_i(0, 0)
    case True{}:
      lanternward.bot.bot_step.aim(lanternward.bot.back_path(came, (ty * w + tx : U32), (fy * w + fx : U32), 2048n), w, me)

def lanternward.bot.bot_step.aim?(+at: U32, +w: U32, +me: J.Val) -> J.Val:
  ga.pair_i(
    lanternward.bot.sign((lanternward.world.middle(U32.mod(at, w)) - ga.u(me, "x") : U32), 256),
    lanternward.bot.sign((lanternward.world.middle(U32.div(at, w)) - ga.u(me, "y") : U32), 256))

# Walk to a point; press E there when `press`.

def lanternward.bot.bot_goto?(+s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach_px: U32, +press: Bool) -> J.Val:
  lanternward.bot.bot_goto.far(
    lanternward.world.dist((px - ga.u(me, "x") : U32), (py - ga.u(me, "y") : U32)),
    s, me, px, py, reach_px, press)

def lanternward.bot.bot_goto.far?(+d: U32, +s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach_px: U32, +press: Bool) -> J.Val:
  lanternward.bot.bot_goto.aim(
    d, RS.i_gt(d, (reach_px * 256 : U32)), s, me, px, py, reach_px, press)

def lanternward.bot.bot_goto.aim?(+d: U32, far: Bool, +s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach_px: U32, +press: Bool) -> J.Val:
  match far:
    case True{}:
      lanternward.bot.bot_goto.moved(
        lanternward.bot.bot_step(s, me, lanternward.world.tile_of(px), lanternward.world.tile_of(py)),
        d, s, me, px, py, reach_px, press)
    case False{}:
      lanternward.bot.bot_goto.face(rules.Input.default(), d, s, me, px, py, reach_px, press)

def lanternward.bot.bot_goto.moved?(+mv: J.Val, +d: U32, +s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach_px: U32, +press: Bool) -> J.Val:
  lanternward.bot.bot_goto.face(
    Bool.pick(J.Val, Bool.and(U32.is_eq(ga.nu(mv, 0), 0), U32.is_eq(ga.nu(mv, 1), 0)),
      lanternward.bot.i_move(rules.Input.default(),
        lanternward.bot.sign((px - ga.u(me, "x") : U32), 256), lanternward.bot.sign((py - ga.u(me, "y") : U32), 256)),
      lanternward.bot.i_move(rules.Input.default(), ga.nu(mv, 0), ga.nu(mv, 1))),
    d, s, me, px, py, reach_px, press)

# Face it with the arrows, and press E every other tick (a press is an edge).

def lanternward.bot.bot_goto.face?(+i: J.Val, +d: U32, +s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach_px: U32, +press: Bool) -> J.Val:
  lanternward.bot.norm(
    lanternward.bot.bot_goto.press(
      Bool.and(press, RS.i_le(d, ((reach_px + 6 : U32) * 256 : U32))),
      lanternward.bot.i_aim(i, lanternward.bot.sign((px - ga.u(me, "x") : U32), 768), lanternward.bot.sign((py - ga.u(me, "y") : U32), 768)),
      s))

def lanternward.bot.bot_goto.press?(go: Bool, +i: J.Val, +s: J.Val) -> J.Val:
  match go:
    case True{}:
      ga.sb(i, "interact", U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0))
    case False{}:
      i

# ---- fighting a snuffer ----

def lanternward.bot.bot_fight?(+s: J.Val, +id: U32, +me: J.Val, +f: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.bot_fight.go(
    lanternward.world.dist((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)),
    lanternward.world.unit((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)),
    s, id, me, f, r)

def lanternward.bot.bot_fight.go?(+d: U32, toward: U32 & U32, +s: J.Val, +id: U32, +me: J.Val, +f: J.Val, +r: J.Val) -> J.Val:
  (+tx, +ty) = toward
  lanternward.bot.bot_fight.light(
    d, tx, ty, s, id, me, f, r,
    ga.sb(rules.Input.default(), "ult", RS.i_lt(d, ga.u(r, "lock"))))

# The hood: light first, then the blade.

def lanternward.bot.bot_fight.light?(+d: U32, +tx: U32, +ty: U32, +s: J.Val, +id: U32, +me: J.Val, +f: J.Val, +r: J.Val, +i: J.Val) -> J.Val:
  lanternward.bot.bot_fight.line(
    Bool.and(
      Bool.and(Bool.and(ga.b(f, "hooded"), U32.is_lt(ga.u(f, "lit"), 12)), ga.b(ga.v(s, "inv"), "lantern")),
      Bool.and(U32.is_gt(ga.u(me, "flame"), 5), RS.i_lt(d, 19968))),
    d, tx, ty, s, id, me, f, r, i)

def lanternward.bot.bot_fight.line?(+fire: Bool, +d: U32, +tx: U32, +ty: U32, +s: J.Val, +id: U32, +me: J.Val, +f: J.Val, +r: J.Val, +i: J.Val) -> J.Val:
  lanternward.bot.bot_fight.state(
    Bool.and(
      Bool.not(lanternward.is(f, "danger", "None")),
      lanternward.foes.Danger.hits(ga.v(f, "danger"), ga.u(me, "x"), ga.u(me, "y"), 1792)),
    d, tx, ty, s, id, me, f, r, Bool.pick(J.Val, fire, ga.sb(i, "fire", True{}), i))

def lanternward.bot.bot_fight.state?(+in_line: Bool, +d: U32, +tx: U32, +ty: U32, +s: J.Val, +id: U32, +me: J.Val, +f: J.Val, +r: J.Val, +i: J.Val) -> J.Val:
  lanternward.bot.bot_fight.aim(
    Bool.pick(J.Val, Bool.and(in_line, lanternward.NAME_IN(ga.s(f, "st"), ["Tell", "Strike"])),
      lanternward.bot.bot_fight.dodge(Bool.or(lanternward.is(f, "st", "Strike"), U32.is_le(ga.u(f, "t"), 5)), tx, ty, id, i),
      lanternward.bot.bot_fight.close(d, tx, ty, s, id, f, i)),
    s, me, f)

# In the blow's line: raise the shield late, else step out sideways.

def lanternward.bot.bot_fight.dodge?(+shield: Bool, +tx: U32, +ty: U32, +id: U32, +i: J.Val) -> J.Val:
  +sg = Bool.pick(U32, U32.is_eq(U32.mod(id, 2), 0), 1, 4294967295)
  Bool.pick(J.Val, shield, ga.sb(i, "dash", True{}),
    lanternward.bot.i_move(i, lanternward.bot.sign((RS.i_neg(ty) * sg : U32), 64), lanternward.bot.sign((tx * sg : U32), 64)))

def lanternward.bot.bot_fight.close?(+d: U32, +tx: U32, +ty: U32, +s: J.Val, +id: U32, +f: J.Val, +i: J.Val) -> J.Val:
  Bool.pick(J.Val, RS.i_gt(d, 6144),
    lanternward.bot.i_move(i, lanternward.bot.sign(tx, 64), lanternward.bot.sign(ty, 64)),
    Bool.pick(J.Val, RS.i_lt(d, 3072),
      lanternward.bot.i_move(i, RS.i_neg(lanternward.bot.sign(tx, 64)), RS.i_neg(lanternward.bot.sign(ty, 64))),
      Bool.pick(J.Val, lanternward.foes.Foe.vulnerable(f),
        lanternward.bot.bot_fight.swing(lanternward.is(f, "st", "Drift"), tx, ty, s, id, ga.sb(i, "swing", U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0))),
        i)))

# A little circling keeps it from lunging into the face.

def lanternward.bot.bot_fight.swing?(+drift: Bool, +tx: U32, +ty: U32, +s: J.Val, +id: U32, +i: J.Val) -> J.Val:
  +sg = Bool.pick(U32, U32.is_eq(U32.mod(id, 2), 0), 1, 4294967295)
  Bool.pick(J.Val, drift,
    lanternward.bot.i_move(i, lanternward.bot.sign((RS.i_neg(ty) * sg : U32), 128), lanternward.bot.sign((tx * sg : U32), 128)),
    i)

# Whatever is not walking faces its foe.

def lanternward.bot.bot_fight.aim?(+i: J.Val, +s: J.Val, +me: J.Val, +f: J.Val) -> J.Val:
  lanternward.bot.norm(
    lanternward.bot.i_aim(i,
      lanternward.bot.sign((ga.u(f, "x") - ga.u(me, "x") : U32), 1024), lanternward.bot.sign((ga.u(f, "y") - ga.u(me, "y") : U32), 1024)))

# ---- after the Beacon: hold E and leave ----

def lanternward.bot.bot_finale?(+s: J.Val) -> J.Val:
  lanternward.bot.norm(
    Bool.pick(J.Val, lanternward.Session.finale(s),
      ga.sb(rules.Input.default(), "interact", U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0)),
      rules.Input.default()))

# ---- the rooms ----

# The nearest of `tiles` (packed x + 65536 y) to the knight: packed, or null.

def lanternward.bot.nearest_tile.go?(+ts: List<&2, U32>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  match ts:
    case Nil{}:
      best
    case Con{+t, +rest}:
      lanternward.bot.nearest_tile.take(
        lanternward.world.dist(
          (lanternward.world.middle(lanternward.world.packed_x(t)) - ga.u(me, "x") : U32),
          (lanternward.world.middle(lanternward.world.packed_y(t)) - ga.u(me, "y") : U32)),
        t, rest, me, best, bd)

def lanternward.bot.nearest_tile.take?(+d: U32, +t: U32, +rest: List<&2, U32>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  lanternward.bot.nearest_tile.go(rest, me,
    Bool.pick(J.Val, Bool.or(J.is_null(best), RS.i_lt(d, bd)), J.JU{t}, best),
    Bool.pick(U32, Bool.or(J.is_null(best), RS.i_lt(d, bd)), d, bd))

def lanternward.bot.bot_exit?(+s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.bot_exit.go(lanternward.bot.nearest_tile.go(lanternward.world.Plan.find(ga.u(s, "room"), 81), me, J.JNull{}, 0), s, me)

def lanternward.bot.bot_exit.go?(+t: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  match t:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.bot_goto(s, me,
        lanternward.world.middle(lanternward.world.packed_x(J.u32(t))), lanternward.world.middle(lanternward.world.packed_y(J.u32(t))), 3, False{})

# ---- the Warden ----

# The seat's place among the seated ids (0 when absent).

def lanternward.bot.rank.go?(+ids: List<&2, U32>, +id: U32, +i: U32) -> J.Val:
  match ids:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      Bool.pick(J.Val, U32.is_eq(h, id), J.JU{i}, lanternward.bot.rank.go(t, id, (i + 1 : U32)))

def lanternward.bot.rank?(+s: J.Val, +id: U32) -> U32:
  J.u32(lanternward.bot.rank.go(ga.ids(J.get(s, "seats")), id, 0))

# The foe within `reach` of the knight nearest it (living snuffers), or null.

def lanternward.bot.snuffer_within?(+s: J.Val, +me: J.Val, +reach: U32) -> J.Val:
  lanternward.bot.snuffer_within.go(lanternward.bot.nearest_snuffer(s, me, True{}), me, reach)

def lanternward.bot.snuffer_within.go?(+f: J.Val, +me: J.Val, +reach: U32) -> J.Val:
  match f:
    case J.JNull{}:
      J.JNull{}
    case _:
      Bool.pick(J.Val,
        RS.i_lt(lanternward.world.dist((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)), reach),
        f, J.JNull{})

def lanternward.bot.bot_boss?(+s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.bot_boss.fight(
    Bool.not(lanternward.boss.Session.boss_exposed(s)),
    lanternward.bot.snuffer_within(s, me, 11520), s, id, me, w, r)

# A snuffer on top of us comes first, unless the heart is open.

def lanternward.bot.bot_boss.fight?(+dark: Bool, +f: J.Val, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.bot_boss.fight.go(Bool.and(dark, Bool.not(J.is_null(f))), f, s, id, me, w, r)

def lanternward.bot.bot_boss.fight.go?(go: Bool, +f: J.Val, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.bot.bot_fight(s, id, me, f, r)
    case False{}:
      lanternward.bot.bot_boss.plan(s, id, me, w, r)

def lanternward.bot.bot_boss.plan?(+s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  +seats = lanternward.seat_count(s)
  +rank = lanternward.bot.rank(s, id)
  +exposed = lanternward.boss.Session.boss_exposed(s)
  +lighter = U32.is_lt(rank, lanternward.boss.Warden.need(w, seats))
  +open_for = lanternward.boss.Warden.open_for(w, ga.u(s, "elapsed"), seats)
  +strike = Bool.and(Bool.and(exposed, Bool.or(Bool.not(lighter), U32.is_gt(open_for, 30))), Bool.not(lanternward.is(w, "st", "Dying")))
  lanternward.bot.bot_boss.walk(
    strike, lighter, exposed, open_for, rank, s, id, me, w, r,
    lanternward.boss.Warden.heart.x(w), lanternward.boss.Warden.heart.y(w))

def lanternward.bot.bot_boss.walk?(+strike: Bool, +lighter: Bool, +exposed: Bool, +open_for: U32, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val, +hx: U32, +hy: U32) -> J.Val:
  lanternward.bot.bot_boss.walk.go(
    lanternward.world.dist((hx - ga.u(me, "x") : U32), (hy - ga.u(me, "y") : U32)),
    lanternward.world.unit((hx - ga.u(me, "x") : U32), (hy - ga.u(me, "y") : U32)),
    strike, lighter, exposed, open_for, rank, s, id, me, w, r, hx, hy)

def lanternward.bot.bot_boss.walk.go?(+d: U32, toward: U32 & U32, +strike: Bool, +lighter: Bool, +exposed: Bool, +open_for: U32, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val, +hx: U32, +hy: U32) -> J.Val:
  (+tx, +ty) = toward
  lanternward.bot.bot_boss.moved(
    Bool.pick(U32, strike, 7680, 15872),
    d, tx, ty, strike, lighter, exposed, open_for, rank, s, id, me, w, r, hx, hy)

def lanternward.bot.bot_boss.moved?(+stand: U32, +d: U32, +tx: U32, +ty: U32, +strike: Bool, +lighter: Bool, +exposed: Bool, +open_for: U32, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val, +hx: U32, +hy: U32) -> J.Val:
  +sg = Bool.pick(U32, U32.is_eq(U32.mod(id, 2), 0), 1, 4294967295)
  lanternward.bot.bot_boss.keys(
    Bool.pick(J.Val, RS.i_gt(d, (stand + 1536 : U32)),
      lanternward.bot.bot_boss.chase(lanternward.bot.bot_step(s, me, lanternward.world.tile_of(hx), lanternward.world.tile_of(hy)), tx, ty),
      Bool.pick(J.Val, RS.i_lt(d, (stand - 2048 : U32)),
        lanternward.bot.i_move(rules.Input.default(), RS.i_neg(lanternward.bot.sign(tx, 64)), RS.i_neg(lanternward.bot.sign(ty, 64))),
        lanternward.bot.i_move(rules.Input.default(), lanternward.bot.sign((RS.i_neg(ty) * sg : U32), 128), lanternward.bot.sign((tx * sg : U32), 128)))),
    d, strike, lighter, exposed, open_for, rank, s, id, me, w, r, hx, hy)

def lanternward.bot.bot_boss.chase?(+mv: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.bot.i_move(rules.Input.default(),
    Bool.pick(U32, U32.is_ne(ga.nu(mv, 0), 0), ga.nu(mv, 0), lanternward.bot.sign(tx, 64)),
    Bool.pick(U32, U32.is_ne(ga.nu(mv, 1), 0), ga.nu(mv, 1), lanternward.bot.sign(ty, 64)))

def lanternward.bot.bot_boss.keys?(+i: J.Val, +d: U32, +strike: Bool, +lighter: Bool, +exposed: Bool, +open_for: U32, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val, +hx: U32, +hy: U32) -> J.Val:
  lanternward.bot.bot_boss.light(
    lanternward.bot.i_aim(ga.sb(i, "ult", RS.i_lt(d, ga.u(r, "lock"))),
      lanternward.bot.sign((hx - ga.u(me, "x") : U32), 1024), lanternward.bot.sign((hy - ga.u(me, "y") : U32), 1024)),
    d, strike, lighter, exposed, open_for, rank, s, id, me, w, r, hx, hy)

# Light the heart: a tap of the lantern, every so often.

def lanternward.bot.bot_boss.light?(+i: J.Val, +d: U32, +strike: Bool, +lighter: Bool, +exposed: Bool, +open_for: U32, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val, +hx: U32, +hy: U32) -> J.Val:
  lanternward.bot.bot_boss.swing(
    Bool.pick(J.Val,
      Bool.and(
        Bool.or(Bool.not(exposed), Bool.and(lighter, U32.is_lt(open_for, 24))),
        Bool.and(
          Bool.and(lighter, U32.is_ge(ga.u(me, "flame"), U32.div(ga.u(r, "flame"), 5))),
          Bool.and(
            Bool.and(
              U32.is_gt((RS.u_sat_sub(ga.u(s, "elapsed"), J.u32(J.nth(ga.v(w, "lit"), lanternward.kit.hit_slot(id)))) + 28 : U32), 90),
              RS.i_lt(d, 28160)),
            lanternward.props.Session.line_open(s, ga.u(me, "x"), ga.u(me, "y"), hx, hy)))),
      ga.sb(i, "fire", U32.is_lt(U32.mod(ga.u(s, "elapsed"), 6), 2)), i),
    d, strike, rank, s, id, me, w, r)

def lanternward.bot.bot_boss.swing?(+i: J.Val, +d: U32, +strike: Bool, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.bot_boss.blows(
    Bool.pick(J.Val, Bool.and(strike, RS.i_le(d, 11264)), ga.sb(i, "swing", U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0)), i),
    rank, s, id, me, w, r)

# Its blows: up and over a ring; out of a beam's wedge; shield for a sweep.

def lanternward.bot.bot_boss.blows?(+i: J.Val, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.norm(
    lanternward.bot.bot_boss.blow(
      lanternward.world.dist((ga.u(w, "x") - ga.u(me, "x") : U32), (ga.u(w, "y") - ga.u(me, "y") : U32)),
      i, rank, s, id, me, w, r))

def lanternward.bot.bot_boss.blow?(+dm: U32, +i: J.Val, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  Bool.pick(J.Val, Bool.and(lanternward.is(w, "st", "Strike"), lanternward.is(w, "atk", "Slam")),
    lanternward.bot.bot_boss.slam(dm, i, s, me, w),
    Bool.pick(J.Val, Bool.and(Bool.and(lanternward.is(w, "st", "Tell"), lanternward.is(w, "atk", "Beam")), U32.is_gt(rank, 0)),
      lanternward.bot.bot_boss.wedge(dm, i, me, w),
      Bool.pick(J.Val, Bool.and(lanternward.is(w, "st", "Strike"), lanternward.is(w, "atk", "Beam")),
        lanternward.bot.bot_boss.sweep(dm, i, me, w),
        i)))

def lanternward.bot.bot_boss.slam?(+dm: U32, +i: J.Val, +s: J.Val, +me: J.Val, +w: J.Val) -> J.Val:
  +elapsed = (30 - ga.u(w, "t") : U32)
  +ring = ((12 + 4 * elapsed : U32) * 256 : U32)
  Bool.pick(J.Val,
    Bool.and(
      Bool.and(lanternward.in_range((dm - ring : U32), 1536, 5120), U32.is_eq(ga.u(me, "z"), 0)),
      Bool.not(lanternward.NAME_IN(ga.s(me, "act"), ["Hop", "Roll", "Item"]))),
    lanternward.bot.i_move(
      ga.sb(ga.sb(ga.sb(i, "interact", U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0)), "ult", True{}), "swing", False{}), 0, 0),
    i)

def lanternward.bot.bot_boss.wedge?(+dm: U32, +i: J.Val, +me: J.Val, +w: J.Val) -> J.Val:
  lanternward.bot.bot_boss.wedge.off(
    lanternward.world.turn(ga.u(w, "aim"), lanternward.world.angle_of((ga.u(me, "x") - ga.u(w, "x") : U32), (ga.u(me, "y") - ga.u(w, "y") : U32))),
    dm, i, me, w)

def lanternward.bot.bot_boss.wedge.off?(+off: U32, +dm: U32, +i: J.Val, +me: J.Val, +w: J.Val) -> J.Val:
  Bool.pick(J.Val, Bool.and(RS.i_lt(RS.i_abs(off), 64), RS.i_lt(dm, 38400)),
    lanternward.bot.bot_boss.wedge.out(
      (ga.u(w, "aim") + Bool.pick(U32, RS.i_ge(off, 0), 74, 4294967222) : U32), i, me, w),
    i)

# Out of it by the nearer edge, round the Warden.

def lanternward.bot.bot_boss.wedge.out?(+a: U32, +i: J.Val, +me: J.Val, +w: J.Val) -> J.Val:
  ga.sb(ga.sb(
    lanternward.bot.i_move(i,
      lanternward.bot.sign(((ga.u(w, "x") + lanternward.world.cos_q(a) * 70 : U32) - ga.u(me, "x") : U32), 256),
      lanternward.bot.sign(((ga.u(w, "y") + lanternward.world.sin_q(a) * 70 : U32) - ga.u(me, "y") : U32), 256)),
    "swing", False{}), "fire", False{})

def lanternward.bot.bot_boss.coming?(+beam: U32, +mine: U32, +b: U32, +beams: U32) -> Bool:
  Bool.pick(Bool, U32.is_ge(b, beams), False{},
    Bool.or(
      lanternward.in_range(lanternward.world.turn((beam + 180 * b : U32), mine), 4294967286, 14),
      lanternward.bot.bot_boss.coming(beam, mine, (b + 1 : U32), beams)))

def lanternward.bot.bot_boss.sweep?(+dm: U32, +i: J.Val, +me: J.Val, +w: J.Val) -> J.Val:
  Bool.pick(J.Val,
    Bool.and(
      lanternward.bot.bot_boss.coming(
        (ga.u(w, "aim") - 50 + RS.i_div((100 * (36 - ga.u(w, "t") : U32) : U32), 36) : U32),
        lanternward.world.angle_of((ga.u(me, "x") - ga.u(w, "x") : U32), (ga.u(me, "y") - ga.u(w, "y") : U32)),
        0, Bool.pick(U32, U32.is_ge(ga.u(w, "phase"), 2), 2, 1)),
      RS.i_lt(dm, 38400)),
    lanternward.bot.i_move(ga.sb(ga.sb(ga.sb(i, "dash", True{}), "swing", False{}), "fire", False{}), 0, 0),
    i)

# ---- the rooms' errands ----

# `(a, b)` of a prop's place, or null: prop_pos(kind, id).

def lanternward.bot.prop_pos?(+s: J.Val, +kind: String, +id: U32) -> J.Val:
  lanternward.stage.prop_pos.opt(lanternward.stage.Session.prop_of(s, kind, id), s)

def lanternward.stage.prop_pos.opt?(+i: J.Val, +s: J.Val) -> J.Val:
  match i:
    case J.JNull{}:
      J.JNull{}
    case _:
      ga.pair_i(lanternward.eu(s, "props", J.u32(i), "x"), lanternward.eu(s, "props", J.u32(i), "y"))

# Walk to a point `(px, py)`, then at the spot face `(ax, ay)` and press E.

def lanternward.bot.at_spot?(+s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach: U32, +near: U32, +ax: U32, +ay: U32, +press: Bool) -> J.Val:
  lanternward.bot.at_spot.go(lanternward.bot.bot_goto(s, me, px, py, reach, False{}), RS.i_lt(lanternward.world.dist((px - ga.u(me, "x") : U32), (py - ga.u(me, "y") : U32)), near), s, ax, ay, press)

def lanternward.bot.at_spot.go?(+i: J.Val, close: Bool, +s: J.Val, +ax: U32, +ay: U32, +press: Bool) -> J.Val:
  match close:
    case False{}:
      i
    case True{}:
      lanternward.bot.norm(
        ga.sb(lanternward.bot.i_aim(i, ax, ay), "interact", Bool.or(Bool.not(press), U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0))))

def lanternward.bot.bot_objective?(+s: J.Val, +id: U32, +me: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.objective.go(
    (ga.u(s, "room") * 10 + ga.u(s, "stage") : U32), s, id, me, r,
    U32.is_eq(lanternward.bot.rank(s, id), 0), lanternward.bot.rank(s, id))

def lanternward.bot.objective.go?(+code: U32, +s: J.Val, +id: U32, +me: J.Val, +r: J.Val, +first: Bool, +rank: U32) -> J.Val:
  match code:
    case 0:
      lanternward.bot.objective.foe(lanternward.bot.nearest_foe.go(ga.items(s, "foes"), me, J.JNull{}, 0), 14, s, me)
    case 11:
      lanternward.bot.objective.chest(lanternward.bot.prop_pos(s, "Chest", 0), s, me, first)
    case 10:
      lanternward.bot.objective.braziers(s, id, me, rank)
    case 13:
      lanternward.bot.objective.lantern(s, me, r)
    case 12:
      lanternward.bot.idle()
    case 20:
      lanternward.bot.objective.plates(s, id, me, first, rank)
    case 21:
      lanternward.bot.bot_exit(s, me)
    case 30:
      lanternward.bot.objective.dark(lanternward.bot.nearest_foe.go(ga.items(s, "foes"), me, J.JNull{}, 0), s, me, r)
    case 31:
      lanternward.bot.objective.chest(lanternward.bot.prop_pos(s, "Chest", 1), s, me, first)
    case 32:
      lanternward.bot.objective.lock_door(first, s, me)
    case 33:
      lanternward.bot.objective.key_chest(lanternward.bot.prop_pos(s, "Chest", 0), s, me, first)
    case 34:
      lanternward.bot.objective.boss_door(s, id, me)
    case 42:
      lanternward.bot.objective.container(lanternward.bot.first_drop(ga.items(s, "drops")), s, me)
    case 43:
      lanternward.bot.objective.beacon(lanternward.find_prop(s, "Beacon", 4294967295), s, me)
    case _:
      lanternward.bot.objective.rest(code, s, id, me)

# Stages that just head for the way out, or to the gallery, or nowhere.

def lanternward.bot.objective.rest?(+code: U32, +s: J.Val, +id: U32, +me: J.Val) -> J.Val:
  lanternward.bot.objective.rest.go(
    Bool.or(
      Bool.or(Bool.and(U32.is_ge(code, 1), U32.is_le(code, 9)), Bool.and(U32.is_ge(code, 14), U32.is_le(code, 19))),
      Bool.or(U32.is_eq(code, 21), Bool.and(U32.is_ge(code, 35), U32.is_le(code, 39)))),
    Bool.and(U32.is_ge(code, 22), U32.is_le(code, 29)),
    s, id, me)

def lanternward.bot.objective.rest.go?(+exit: Bool, +gallery: Bool, +s: J.Val, +id: U32, +me: J.Val) -> J.Val:
  Bool.pick(J.Val, exit, lanternward.bot.bot_exit(s, me),
    Bool.pick(J.Val, gallery, lanternward.bot.bot_gallery(s, id, me), lanternward.bot.idle()))

def lanternward.bot.objective.foe?(+f: J.Val, +reach: U32, +s: J.Val, +me: J.Val) -> J.Val:
  match f:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.bot_goto(s, me, ga.u(f, "x"), ga.u(f, "y"), reach, False{})

# The first of the party takes the chest; the rest wait south of it.

def lanternward.bot.objective.chest?(+c: J.Val, +s: J.Val, +me: J.Val, +first: Bool) -> J.Val:
  match c:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      Bool.pick(J.Val, first,
        lanternward.bot.bot_goto(s, me, ga.nu(c, 0), ga.nu(c, 1), 14, True{}),
        lanternward.bot.bot_goto(s, me, ga.nu(c, 0), (ga.nu(c, 1) + 12288 : U32), 8, False{}))

# The unlit braziers' indices.

def lanternward.bot.unlit.go?(+ps: List<&2, J.Val>, +i: U32) -> List<&2, U32>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      Bool.pick(List<&2, U32>, Bool.and(String.eq(ga.s(p, "kind"), "Brazier"), Bool.not(lanternward.props.Prop.lit(p))),
        Con{i, lanternward.bot.unlit.go(t, (i + 1 : U32))}, lanternward.bot.unlit.go(t, (i + 1 : U32)))

def lanternward.bot.nth_u32?(+xs: List<&2, U32>, n: Nat) -> U32:
  match xs n:
    case Nil{} _:
      0
    case Con{+h, +t} 0n:
      h
    case Con{+h, +t} 1n+p:
      lanternward.bot.nth_u32(t, p)

# Three braziers, one ember: each fetches a flame, lights one.

def lanternward.bot.objective.braziers?(+s: J.Val, +id: U32, +me: J.Val, +rank: U32) -> J.Val:
  lanternward.bot.objective.braziers.go(lanternward.bot.unlit.go(ga.items(s, "props"), 0), s, me, rank)

def lanternward.bot.objective.braziers.go?(+unlit: List<&2, U32>, +s: J.Val, +me: J.Val, +rank: U32) -> J.Val:
  lanternward.bot.objective.braziers.n(U32.from_nat(List.length(&2, U32, unlit)), unlit, s, me, rank)

def lanternward.bot.objective.braziers.n?(+n: U32, +unlit: List<&2, U32>, +s: J.Val, +me: J.Val, +rank: U32) -> J.Val:
  Bool.pick(J.Val, U32.is_eq(n, 0), lanternward.bot.idle(),
    Bool.pick(J.Val, U32.is_gt(ga.u(me, "taper"), 20),
      lanternward.bot.objective.braziers.at(lanternward.bot.nth_u32(unlit, U32.to_nat(U32.mod(rank, RS.u_max(n, 1)))), s, me),
      lanternward.bot.objective.ember(lanternward.find_prop(s, "Ember", 4294967295), s, me)))

def lanternward.bot.objective.braziers.at?(+j: U32, +s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.bot_goto(s, me, lanternward.eu(s, "props", j, "x"), lanternward.eu(s, "props", j, "y"), 12, True{})

def lanternward.bot.objective.ember?(+p: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  match p:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.bot_goto(s, me, ga.u(p, "x"), ga.u(p, "y"), 12, True{})

# South of the cage, the lantern thrown north through the bars.

def lanternward.bot.objective.lantern?(+s: J.Val, +me: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.objective.lantern.go(
    lanternward.bot.i_aim(lanternward.bot.bot_goto(s, me, lanternward.world.middle(24), lanternward.world.middle(7), 4, False{}), 0, 4294967295),
    RS.i_lt(lanternward.world.dist((lanternward.world.middle(24) - ga.u(me, "x") : U32), (lanternward.world.middle(7) - ga.u(me, "y") : U32)), 2048),
    s, me, r)

def lanternward.bot.objective.lantern.go?(+i: J.Val, +near: Bool, +s: J.Val, +me: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.norm(
    Bool.pick(J.Val,
      Bool.and(Bool.and(near, U32.is_ge(ga.u(me, "flame"), U32.div(ga.u(r, "flame"), 5))), U32.is_eq(ga.u(me, "lamp_cd"), 0)),
      ga.sb(i, "fire", U32.is_lt(U32.mod(ga.u(s, "elapsed"), 8), 3)), i))

# The Gallery's first zone: the block onto the plate, the second plate held.

def lanternward.bot.first_of?(+s: J.Val, +kind: String) -> J.Val:
  lanternward.find_prop(s, kind, 4294967295)

def lanternward.bot.objective.plates?(+s: J.Val, +id: U32, +me: J.Val, +first: Bool, +rank: U32) -> J.Val:
  lanternward.bot.objective.plates.gate(lanternward.stage.any_on(ga.items(s, "props"), "Gate", 1), s, id, me, first, rank)

def lanternward.bot.objective.plates.gate?(open: Bool, +s: J.Val, +id: U32, +me: J.Val, +first: Bool, +rank: U32) -> J.Val:
  match open:
    case True{}:
      lanternward.bot.bot_gallery(s, id, me)
    case False{}:
      lanternward.bot.objective.plates.second(U32.is_eq(rank, 1), s, me, first)

# The second knight holds the second plate for the rest.

def lanternward.bot.objective.plates.second?(second: Bool, +s: J.Val, +me: J.Val, +first: Bool) -> J.Val:
  match second:
    case True{}:
      lanternward.bot.objective.ember.plate(lanternward.find_prop(s, "Plate", 1), s, me)
    case False{}:
      lanternward.bot.objective.plates.push(lanternward.bot.first_of(s, "Block"), lanternward.bot.first_of(s, "Plate"), s, me, first)

def lanternward.bot.objective.ember.plate?(+p: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  match p:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.bot_goto(s, me, ga.u(p, "x"), ga.u(p, "y"), 2, False{})

def lanternward.bot.objective.plates.push?(+b: J.Val, +p: J.Val, +s: J.Val, +me: J.Val, +first: Bool) -> J.Val:
  match p:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      Bool.pick(J.Val,
        Bool.and(Bool.and(first, Bool.not(J.is_null(b))), U32.is_ne(ga.u(b, "tx"), ga.u(p, "tx"))),
        lanternward.bot.objective.behind(b, s, me),
        lanternward.bot.bot_goto(s, me, (ga.u(p, "x") + 4096 : U32), (ga.u(p, "y") + 8192 : U32), 10, False{}))

# Behind the block, then east into it.

def lanternward.bot.objective.behind?(+b: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.norm(
    Bool.pick(J.Val,
      Bool.and(
        RS.i_lt(ga.u(me, "x"), (ga.u(b, "x") - 2560 : U32)),
        RS.i_gt(lanternward.world.dist(((ga.u(b, "x") - 4096 : U32) - ga.u(me, "x") : U32), (ga.u(b, "y") - ga.u(me, "y") : U32)), 1536)),
      lanternward.bot.bot_goto(s, me, (ga.u(b, "x") - 4096 : U32), ga.u(b, "y"), 3, False{}),
      lanternward.bot.i_aim(lanternward.bot.i_move(rules.Input.default(), 1, 0), 1, 0)))

# The dark: head for the nearest snuffer's glow.

def lanternward.bot.objective.dark?(+f: J.Val, +s: J.Val, +me: J.Val, +r: J.Val) -> J.Val:
  match f:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.norm(
        ga.sb(lanternward.bot.bot_goto(s, me, ga.u(f, "x"), ga.u(f, "y"), 30, False{}), "ult",
          RS.i_lt(lanternward.world.dist((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)), ga.u(r, "lock"))))

# The first knight goes to (middle(14), middle(4)) and presses E facing north.

def lanternward.bot.objective.lock_door?(+first: Bool, +s: J.Val, +me: J.Val) -> J.Val:
  Bool.pick(J.Val, first,
    lanternward.bot.at_spot(s, me, lanternward.world.middle(14), lanternward.world.middle(4), 3, 1536, 0, 4294967295, True{}),
    lanternward.bot.bot_goto(s, me, lanternward.world.middle(14), lanternward.world.middle(8), 10, False{}))

def lanternward.bot.objective.key_chest?(+c: J.Val, +s: J.Val, +me: J.Val, +first: Bool) -> J.Val:
  match c:
    case J.JNull{}:
      lanternward.bot.bot_goto(s, me, lanternward.world.middle(14), lanternward.world.middle(8), 10, False{})
    case _:
      Bool.pick(J.Val, first,
        lanternward.bot.at_spot(s, me, ga.nu(c, 0), (ga.nu(c, 1) + 4096 : U32), 5, 2048, 0, 4294967295, True{}),
        lanternward.bot.bot_goto(s, me, lanternward.world.middle(14), lanternward.world.middle(8), 10, False{}))

# The carrier opens the Boss Key door; the rest wait.

def lanternward.bot.objective.boss_door?(+s: J.Val, +id: U32, +me: J.Val) -> J.Val:
  lanternward.bot.objective.boss_door.go(
    lanternward.bot.door_tile(lanternward.world.Plan.find(3, 77)),
    Bool.and(lanternward.tag_is(ga.v(ga.v(s, "inv"), "boss"), "Carried"), U32.is_eq(ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), id)),
    s, me)

def lanternward.bot.door_tile?(+ms: List<&2, U32>) -> U32:
  match ms:
    case Nil{}:
      (29 + 65536 * 8 : U32)
    case Con{+h, +t}:
      h

def lanternward.bot.objective.boss_door.go?(+door: U32, +carrier: Bool, +s: J.Val, +me: J.Val) -> J.Val:
  Bool.pick(J.Val, carrier,
    lanternward.bot.at_spot(s, me, lanternward.world.middle((lanternward.world.packed_x(door) - 1 : U32)), lanternward.world.middle(lanternward.world.packed_y(door)), 3, 2048, 1, 0, True{}),
    lanternward.bot.objective.boss_door.wait(lanternward.tag_is(ga.v(ga.v(s, "inv"), "boss"), "Lying"), door, s, me))

def lanternward.bot.objective.boss_door.wait?(lying: Bool, +door: U32, +s: J.Val, +me: J.Val) -> J.Val:
  Bool.pick(J.Val, lying,
    lanternward.bot.bot_goto(s, me, ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 1), 4, False{}),
    lanternward.bot.bot_goto(s, me, lanternward.world.middle((lanternward.world.packed_x(door) - 4 : U32)), lanternward.world.middle(lanternward.world.packed_y(door)), 10, False{}))

def lanternward.bot.first_drop?(+ds: List<&2, J.Val>) -> J.Val:
  match ds:
    case Nil{}:
      J.JNull{}
    case Con{+d, +t}:
      Bool.pick(J.Val, String.eq(ga.s(d, "kind"), "Container"), d, lanternward.bot.first_drop(t))

def lanternward.bot.objective.container?(+d: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  match d:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.bot_goto(s, me, ga.u(d, "x"), ga.u(d, "y"), 4, False{})

def lanternward.bot.objective.beacon?(+p: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  match p:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.at_spot(s, me, ga.u(p, "x"), (ga.u(p, "y") + 4096 : U32), 8, 3584, 0, 4294967295, False{})

# The Gallery's zone two: one knight stands where the turret's bolt, met with
# a shield in its first ticks, goes to the crystal; the rest keep back.

def lanternward.bot.bot_gallery?(+s: J.Val, +id: U32, +me: J.Val) -> J.Val:
  lanternward.bot.gallery.second(
    Bool.and(U32.is_eq(lanternward.bot.rank(s, id), 1), U32.is_eq(ga.u(s, "stage"), 0)),
    lanternward.find_prop(s, "Plate", 1), s, id, me, lanternward.bot.rank(s, id))

def lanternward.bot.gallery.second?(+hold: Bool, +p: J.Val, +s: J.Val, +id: U32, +me: J.Val, +rank: U32) -> J.Val:
  Bool.pick(J.Val, Bool.and(hold, Bool.not(J.is_null(p))),
    lanternward.bot.bot_goto(s, me, ga.u(p, "x"), ga.u(p, "y"), 2, False{}),
    lanternward.bot.gallery.rank(rank, s, id, me))

def lanternward.bot.gallery.rank?(+rank: U32, +s: J.Val, +id: U32, +me: J.Val) -> J.Val:
  Bool.pick(J.Val, U32.is_ne(rank, 0),
    lanternward.bot.bot_goto(s, me, lanternward.world.middle(19), lanternward.world.middle((2 + RS.i_rem((2 * rank : U32), 9) : U32)), 10, False{}),
    lanternward.bot.gallery.spot(s, me))

def lanternward.bot.gallery.spot?(+s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.gallery.crystal(lanternward.bot.first_of(s, "Crystal"), s, me)

def lanternward.bot.gallery.crystal?(+c: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.gallery.aim(
    Bool.pick(U32, J.is_null(c), lanternward.world.middle(35), ga.u(c, "x")),
    Bool.pick(U32, J.is_null(c), lanternward.world.middle(5), ga.u(c, "y")),
    s, me)

def lanternward.bot.gallery.aim?(+cx: U32, +cy: U32, +s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.gallery.at(
    lanternward.bot.i_aim(
      lanternward.bot.bot_goto(s, me, lanternward.world.middle(29), lanternward.world.middle(5), 5, False{}),
      lanternward.bot.sign((cx - ga.u(me, "x") : U32), 768), lanternward.bot.sign((cy - ga.u(me, "y") : U32), 768)),
    RS.i_lt(lanternward.world.dist((lanternward.world.middle(29) - ga.u(me, "x") : U32), (lanternward.world.middle(5) - ga.u(me, "y") : U32)), 2304),
    s, me)

# At the spot: stand, lock the crystal (a tap of the arrow moves the lock off
# the turret), and meet the bolt with the shield.

def lanternward.bot.gallery.at?(+i: J.Val, close: Bool, +s: J.Val, +me: J.Val) -> J.Val:
  match close:
    case False{}:
      lanternward.bot.norm(i)
    case True{}:
      lanternward.bot.gallery.hold(lanternward.bot.on_crystal(s, me), i, s, me)

def lanternward.bot.on_crystal?(+s: J.Val, +me: J.Val) -> Bool:
  Bool.and(
    lanternward.tag_is(ga.v(me, "lock"), "Prop"),
    lanternward.bot.on_crystal.prop(J.u32(lanternward.payload(ga.v(me, "lock"))), s))

def lanternward.bot.on_crystal.prop?(+pl: U32, +s: J.Val) -> Bool:
  Bool.and(U32.is_lt(pl, lanternward.len(s, "props")), lanternward.eis(s, "props", pl, "kind", "Crystal"))

def lanternward.bot.gallery.hold?(+on: Bool, +i: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.norm(
    ga.sb(
      lanternward.bot.i_aim(
        lanternward.bot.i_move(ga.sb(i, "ult", True{}), 0, 0),
        0, Bool.pick(U32, Bool.and(Bool.not(on), U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0)), 4294967295, 0)),
      "dash",
      Bool.and(on, Bool.or(
        lanternward.bot.gallery.near(ga.items(s, "bolts"), me),
        Bool.and(U32.is_gt(ga.u(me, "shield"), 0),
          Bool.and(Bool.not(lanternward.bot.gallery.clear(ga.items(s, "bolts"), me)), U32.is_lt(ga.u(me, "shield"), 18)))))))

# A hostile bolt closing in on the knight.

def lanternward.bot.gallery.near?(+bs: List<&2, J.Val>, +me: J.Val) -> Bool:
  match bs:
    case Nil{}:
      False{}
    case Con{+b, +t}:
      Bool.or(
        Bool.and(
          Bool.and(lanternward.foes.Bolt.hostile(b),
            RS.i_lt(lanternward.world.dist((ga.u(b, "x") - ga.u(me, "x") : U32), (ga.u(b, "y") - ga.u(me, "y") : U32)), 3840)),
          RS.i_gt(lanternward.bot.gallery.closing(b, me), 128)),
        lanternward.bot.gallery.near(t, me))

def lanternward.bot.gallery.closing?(+b: J.Val, +me: J.Val) -> U32:
  lanternward.bot.gallery.closing.go(lanternward.world.unit(ga.u(b, "vx"), ga.u(b, "vy")), b, me)

def lanternward.bot.gallery.closing.go?(v: U32 & U32, +b: J.Val, +me: J.Val) -> U32:
  (+vx, +vy) = v
  lanternward.world.dot_unit(vx, vy, (ga.u(me, "x") - ga.u(b, "x") : U32), (ga.u(me, "y") - ga.u(b, "y") : U32))

# No hostile bolt within 40 px.

def lanternward.bot.gallery.clear?(+bs: List<&2, J.Val>, +me: J.Val) -> Bool:
  match bs:
    case Nil{}:
      True{}
    case Con{+b, +t}:
      Bool.and(
        Bool.or(Bool.not(lanternward.foes.Bolt.hostile(b)),
          RS.i_gt(lanternward.world.dist((ga.u(b, "x") - ga.u(me, "x") : U32), (ga.u(b, "y") - ga.u(me, "y") : U32)), 10240)),
        lanternward.bot.gallery.clear(t, me))

def lanternward.case.s2?(+input: J.Val) -> J.Val:
  ga.sv(ga.sv(J.get(input, "s"), "$r", J.get(input, "r")), "$ev", lanternward.Events.default())

def lanternward.case.out2?(+s: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "ev", J.get(s, "$ev")), "s", lanternward.Session.bare(s))

def lanternward.case.sk?(+input: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(J.get(input, "s"), "$k", J.get(input, "k")), "$r", J.get(input, "r")), "$ev", lanternward.Events.default())

def lanternward.case.outk?(+s: J.Val, +ret: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(ga.sv(J.obj(), "ret", ret), "ev", J.get(s, "$ev")), "k", J.get(s, "$k")), "s", lanternward.Session.bare(s))

def lanternward.case.outs?(+s: J.Val, +ret: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(J.obj(), "ret", ret), "ev", J.get(s, "$ev")), "s", lanternward.Session.bare(s))

def lanternward.case.a?(+input: J.Val) -> J.Val:
  J.get(input, "a")

def lanternward.case.pt?(+v: J.Val) -> U32 & U32:
  (ga.nu(v, 0), ga.nu(v, 1))

# ---- session.rs ----

def case.lanternward.Session.new?(+input: J.Val) -> J.Val:
  lanternward.Session.new(J.get(input, "players"))

def case.lanternward.Session.step?(+input: J.Val) -> J.Val:
  lanternward.case.step.out(
    lanternward.Session.step(J.get(input, "s"), J.get(input, "inputs"), J.get(input, "heroes"), J.get(input, "r")))

def lanternward.case.step.out?(+s: J.Val) -> J.Val:
  lanternward.case.out2(s)

def case.lanternward.Session.finale?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.Session.finale(J.get(input, "s"))}

def case.lanternward.Session.valid?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.Session.valid(J.get(input, "s"))}

def case.lanternward.Session.problem?(+input: J.Val) -> J.Val:
  lanternward.Session.problem(J.get(input, "s"))

def case.lanternward.Session.repair?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.Session.repair(J.get(input, "s")))

def case.lanternward.Rules.tell_for?(+input: J.Val) -> J.Val:
  J.JU{lanternward.Rules.tell_for(J.get(input, "r"), ga.u(input, "seats"))}

def case.lanternward.purse?(+input: J.Val) -> J.Val:
  J.JU{lanternward.purse(ga.b(input, "first"), ga.u(input, "gems"))}

# ---- phases ----

def case.lanternward.foes.Session.provision?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.foes.Session.provision(lanternward.case.s2(input)))

def case.lanternward.kit.Session.step_seat?(+input: J.Val) -> J.Val:
  lanternward.case.out2(
    lanternward.kit.Session.step_seat(lanternward.case.s2(input), ga.u(input, "id"), J.get(input, "inp")))

def case.lanternward.stage.Session.tick_props?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.tick_props(lanternward.case.s2(input)))

def case.lanternward.stage.Session.unstick?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.unstick(lanternward.case.s2(input)))

def case.lanternward.foes.Session.step_foes?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.foes.Session.step_foes(lanternward.case.s2(input)))

def case.lanternward.boss.Session.step_boss?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.boss.Session.step_boss(lanternward.case.s2(input)))

def case.lanternward.blow.Session.step_bolts?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.blow.Session.step_bolts(lanternward.case.s2(input)))

def case.lanternward.blow.Session.step_fuses?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.blow.Session.step_fuses(lanternward.case.s2(input)))

def case.lanternward.stage.Session.pickups?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.pickups(lanternward.case.s2(input)))

def case.lanternward.stage.Session.revive_pass?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.revive_pass(lanternward.case.s2(input)))

def case.lanternward.stage.Session.tick_stage?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.tick_stage(lanternward.case.s2(input)))

def case.lanternward.stage.Session.tick_wipe?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.tick_wipe(lanternward.case.s2(input)))

def case.lanternward.stage.Session.age?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.age(lanternward.case.s2(input)))

# ---- small public helpers on the session ----

def case.lanternward.props.Session.clear?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.clear(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "r"))}

def case.lanternward.props.Session.blocked?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.blocked(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"))}

def case.lanternward.props.Session.pit?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.pit(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"))}

def case.lanternward.props.Session.line_open?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.line_open(J.get(input, "s"),
    ga.nu(J.get(input, "a"), 0), ga.nu(J.get(input, "a"), 1), ga.nu(J.get(input, "b"), 0), ga.nu(J.get(input, "b"), 1))}

def case.lanternward.props.Session.burning?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.burning(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"))}

def case.lanternward.props.Session.secret_open?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.secret_open(J.get(input, "s"))}

def case.lanternward.boss.Session.boss_exposed?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.boss.Session.boss_exposed(J.get(input, "s"))}

def case.lanternward.ctx.Session.beacon_ready?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.ctx.Session.beacon_ready(J.get(input, "s"))}

def case.lanternward.ctx.Session.burn_ticks?(+input: J.Val) -> J.Val:
  J.JU{lanternward.ctx.Session.burn_ticks(J.get(input, "s"))}

def case.lanternward.stage.Session.exit_open?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.stage.Session.exit_open(J.get(input, "s"))}

def case.lanternward.ctx.Session.context?(+input: J.Val) -> J.Val:
  lanternward.ctx.Session.context(J.get(input, "s"), J.get(input, "k"))

def case.lanternward.ctx.Session.carrying_boss_key?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.ctx.Session.carrying_boss_key(J.get(input, "s"), J.get(input, "k"))}

def case.lanternward.kit.Session.lock_point?(+input: J.Val) -> J.Val:
  lanternward.kit.Session.lock_point(J.get(input, "s"), J.get(input, "l"))

def case.lanternward.foes.Session.nearest_knight?(+input: J.Val) -> J.Val:
  lanternward.foes.Session.nearest_knight(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"))

def case.lanternward.stage.Session.start_of?(+input: J.Val) -> J.Val:
  lanternward.stage.Session.start_of(J.get(input, "s"), ga.u(input, "id"))

def case.lanternward.foes.Session.nudge?(+input: J.Val) -> J.Val:
  lanternward.foes.Session.nudge(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "r"))

def case.lanternward.props.Session.roll?(+input: J.Val) -> J.Val:
  lanternward.case.roll.out(lanternward.props.Session.roll(J.get(input, "s")), ga.u(input, "n"))

def lanternward.case.roll.out?(+s: J.Val, +n: U32) -> J.Val:
  ga.sv(ga.su(J.obj(), "v", lanternward.props.Session.rolled(s, n)), "s", s)

def case.lanternward.props.Session.scatter?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.props.Session.scatter(J.get(input, "s"), ga.s(input, "kind"), ga.u(input, "x"), ga.u(input, "y")))

def case.lanternward.props.Session.fx?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.props.Session.fx(J.get(input, "s"), ga.s(input, "kind"), ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "a")))

def case.lanternward.props.Session.drop_at?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.props.Session.drop_at(J.get(input, "s"), ga.s(input, "kind"), ga.u(input, "x"), ga.u(input, "y")))

def case.lanternward.props.Session.solved?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.props.Session.solved(lanternward.case.sb(input), ga.u(input, "x"), ga.u(input, "y")))

def case.lanternward.props.Session.shatter?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.props.Session.shatter(lanternward.case.sb(input), ga.u(input, "i")))

def case.lanternward.props.Session.set_gate?(+input: J.Val) -> J.Val:
  lanternward.case.set_gate.out(lanternward.props.Session.set_gate(J.get(input, "s"), ga.u(input, "id"), ga.b(input, "open")))

def lanternward.case.set_gate.out?(+s: J.Val) -> J.Val:
  ga.sv(ga.sb(J.obj(), "changed", ga.b(s, "$ret")), "s", lanternward.Session.bare(s))

def lanternward.case.sb?(+input: J.Val) -> J.Val:
  ga.sv(ga.sv(J.get(input, "s"), "$r", J.get(input, "r")), "$ev", lanternward.Events.default())

def case.lanternward.blow.Session.blow?(+input: J.Val) -> J.Val:
  lanternward.case.blow.out(
    lanternward.blow.Session.blow(lanternward.case.sb(input), ga.u(input, "id"), ga.u(input, "dmg"),
      ga.nu(J.get(input, "from"), 0), ga.nu(J.get(input, "from"), 1), ga.s(input, "kind")))

def lanternward.case.blow.out?(+s: J.Val) -> J.Val:
  ga.ss(lanternward.case.out2(s), "out", ga.s(s, "$ret"))

def case.lanternward.blow.Session.hurt_foe?(+input: J.Val) -> J.Val:
  lanternward.case.hurt.out(
    lanternward.blow.Session.hurt_foe(
      ga.sv(ga.sv(J.get(input, "s"), "$ev", lanternward.Events.default()), "$r", J.obj()), ga.u(input, "i"), ga.u(input, "dmg"),
      ga.nu(J.get(input, "from"), 0), ga.nu(J.get(input, "from"), 1)))

def lanternward.case.hurt.out?(+s: J.Val) -> J.Val:
  ga.sb(lanternward.case.out2(s), "hurt", ga.b(s, "$ret"))

def case.lanternward.boss.Session.hurt_boss?(+input: J.Val) -> J.Val:
  lanternward.case.hurt.out(
    lanternward.boss.Session.hurt_boss(
      ga.sv(ga.sv(J.get(input, "s"), "$ev", lanternward.Events.default()), "$r", J.obj()), ga.u(input, "dmg"),
      ga.nu(J.get(input, "from"), 0), ga.nu(J.get(input, "from"), 1), ga.u(input, "owner")))

def case.lanternward.boss.Session.blast_boss?(+input: J.Val) -> J.Val:
  lanternward.case.out2(
    lanternward.boss.Session.blast_boss(
      ga.sv(ga.sv(J.get(input, "s"), "$ev", lanternward.Events.default()), "$r", J.obj()),
      ga.nu(J.get(input, "at"), 0), ga.nu(J.get(input, "at"), 1)))

def case.lanternward.boss.Session.light_heart?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.boss.Session.light_heart(J.get(input, "s"), ga.u(input, "owner")))

def case.lanternward.boss.Session.reset_boss_phase?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.boss.Session.reset_boss_phase(J.get(input, "s")))

def case.lanternward.stage.Session.retry?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.retry(lanternward.case.s2(input)))

def case.lanternward.stage.Session.enter_room?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.Session.bare(lanternward.stage.Session.enter_room(lanternward.case.sb(input), ga.u(input, "room"))))

def case.lanternward.foes.Session.spawn_foe?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.foes.Session.spawn_foe(J.get(input, "s"), ga.s(input, "kind"), ga.u(input, "x"), ga.u(input, "y"), ga.b(input, "hooded")))

# Grant to the knight `id`, taken out of the table as the Rust did.

def case.lanternward.ctx.Session.grant?(+input: J.Val) -> J.Val:
  lanternward.case.grant.go(ga.u(input, "id"), J.get(input, "item"), lanternward.case.sb(input))

def lanternward.case.grant.go?(+id: U32, +item: J.Val, +s: J.Val) -> J.Val:
  lanternward.case.out2(
    lanternward.kit.put_k(
      lanternward.ctx.Session.grant(
        ga.sv(ga.sv(s, "$k", ga.at(J.get(s, "seats"), id)), "seats", J.del(J.get(s, "seats"), U32.show(id))), id, item),
      id))

def case.lanternward.Session.bot_input?(+input: J.Val) -> J.Val:
  lanternward.bot.Session.bot_input(J.get(input, "s"), ga.u(input, "id"), J.get(input, "r"))

def case.lanternward.bot.Session.bot_input?(+input: J.Val) -> J.Val:
  lanternward.bot.Session.bot_input(J.get(input, "s"), ga.u(input, "id"), J.get(input, "r"))

# ---- the Run hooks ----

def case.lanternward.Run.lanternward_showing?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.Run.lanternward_showing(J.get(input, "run"))}

def case.lanternward.Run.start_lanternward?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", lanternward.Run.start_lanternward(J.get(input, "run")))

def case.lanternward.Run.tick_lanternward?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", lanternward.Run.tick_lanternward(J.get(input, "run"), J.get(input, "inputs")))

def case.lanternward.Run.finish_lanternward?(+input: J.Val) -> J.Val:
  lanternward.case.finish.out(lanternward.Run.finish_lanternward(J.get(input, "run"), J.get(input, "s")))

def lanternward.case.finish.out?(+run: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "s", J.get(run, "$s")), "run", J.del(run, "$s"))

def case.lanternward.ADVENTURE.valid?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.ADVENTURE.valid(J.get(input, "run"))}

def case.lanternward.ADVENTURE.seated?(+input: J.Val) -> J.Val:
  lanternward.ADVENTURE.seated(J.get(input, "run"))

def case.lanternward.ADVENTURE.bot?(+input: J.Val) -> J.Val:
  lanternward.ADVENTURE.bot(J.get(input, "run"), ga.u(input, "id"))

def case.lanternward.ADVENTURE.clear?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", lanternward.ADVENTURE.clear(J.get(input, "run")))

def case.lanternward.ACTIVITY.on?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.ACTIVITY.on(J.get(input, "run"))}

def case.lanternward.ACTIVITY.ride?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", lanternward.ACTIVITY.ride(J.get(input, "run"), J.get(input, "inputs")))

def case.lanternward.ACTIVITY.end?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", lanternward.ACTIVITY.end(J.get(input, "run")))

def case.lanternward.kit.buttons?(+input: J.Val) -> J.Val:
  J.JU{lanternward.kit.buttons(J.get(input, "i"))}

def case.lanternward.kit.Seat.new?(+input: J.Val) -> J.Val:
  lanternward.kit.Seat.new(ga.u(input, "id"), ga.u(input, "joined"), ga.nu(J.get(input, "at"), 0), ga.nu(J.get(input, "at"), 1))

def case.lanternward.kit.Seat.valid?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.kit.Seat.valid(J.get(input, "k"), ga.u(input, "id"), ga.u(input, "elapsed"), ga.u(input, "w"), ga.u(input, "h"))}

def case.lanternward.kit.Seat.flaring?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.kit.Seat.flaring(J.get(input, "k"))}

def case.lanternward.kit.Seat.up?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.kit.Seat.up(J.get(input, "k"))}

def case.lanternward.kit.Session.recovering?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.kit.Session.recovering(J.get(input, "k"))}

def case.lanternward.kit.Session.targets?(+input: J.Val) -> J.Val:
  J.JA{lanternward.kit.Session.targets(J.get(input, "s"))}

def lanternward.case.kn?(+input: J.Val, +s: J.Val) -> J.Val:
  lanternward.case.outk(s, J.JNull{})

def case.lanternward.kit.Session.refill?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.refill(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.upkeep?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.upkeep(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.camera?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.camera(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.pick_target?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.case.sk(input), lanternward.kit.Session.pick_target(lanternward.case.sk(input), J.get(lanternward.case.sk(input), "$k"), J.JNull{}))

def case.lanternward.kit.Session.cycle_target?(+input: J.Val) -> J.Val:
  lanternward.case.cycle(lanternward.case.sk(input), J.get(lanternward.case.a(input), "dir"))

def lanternward.case.cycle?(+s: J.Val, +dir: J.Val) -> J.Val:
  lanternward.case.outk(s, lanternward.kit.Session.cycle_target(s, J.get(s, "$k"), ga.nu(dir, 0), ga.nu(dir, 1)))

def case.lanternward.kit.Session.lock_holds?(+input: J.Val) -> J.Val:
  lanternward.case.holds(lanternward.case.sk(input), J.get(lanternward.case.a(input), "l"))

def lanternward.case.holds?(+s: J.Val, +l: J.Val) -> J.Val:
  lanternward.case.outk(s, J.JBool{lanternward.kit.Session.lock_holds(s, J.get(s, "$k"), l)})

def case.lanternward.kit.Session.take_lock?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.take_lock(lanternward.case.sk(input), J.get(lanternward.case.a(input), "l")), J.JNull{})

def case.lanternward.kit.Session.lock_on?(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.lock_on(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "mask"),
      ga.u(lanternward.case.a(input), "press"), J.get(lanternward.case.a(input), "inp")),
    J.JNull{})

def case.lanternward.kit.Session.face_and_move?(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.face_and_move(lanternward.case.sk(input), J.get(lanternward.case.a(input), "inp")), J.JNull{})

def case.lanternward.kit.Session.shove?(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.shove(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "dx"), ga.u(lanternward.case.a(input), "dy")), J.JNull{})

def case.lanternward.kit.Session.lean?(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.lean(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "sx"), ga.u(lanternward.case.a(input), "sy")), J.JNull{})

def case.lanternward.kit.Session.orbit?(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.orbit(lanternward.case.sk(input),
      ga.nu(J.get(lanternward.case.a(input), "t"), 0), ga.nu(J.get(lanternward.case.a(input), "t"), 1),
      ga.nu(J.get(lanternward.case.a(input), "v"), 0), ga.nu(J.get(lanternward.case.a(input), "v"), 1)),
    J.JNull{})

def case.lanternward.kit.Session.falling?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.falling(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.hurting?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.hurting(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.free?(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.free(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "press"), J.get(lanternward.case.a(input), "inp")),
    J.JNull{})

def case.lanternward.kit.Session.swinging?(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.swinging(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "press")),
    J.JNull{})

def case.lanternward.kit.Session.rolling?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.rolling(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.hopping?(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.hopping(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "press")),
    J.JNull{})

def case.lanternward.kit.Session.plunging?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.plunging(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id")), J.JNull{})

def case.lanternward.kit.Session.lantern?(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.lantern(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "mask")),
    J.JNull{})

def case.lanternward.kit.Session.verbs?(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.verbs(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "mask"),
      ga.u(lanternward.case.a(input), "press"), J.get(lanternward.case.a(input), "inp")),
    J.JNull{})

def case.lanternward.kit.Session.begin_dodge?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.begin_dodge(lanternward.case.sk(input), J.get(lanternward.case.a(input), "inp")), J.JNull{})

def case.lanternward.kit.Session.begin_slash?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.begin_slash(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "step")), J.JNull{})

def case.lanternward.kit.Session.begin_lunge?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.begin_lunge(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.begin_spin?(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.begin_spin(lanternward.case.sk(input)), J.JNull{})

# sweep / lunge_hit leave whether they landed in `$ret`.

def lanternward.case.landed?(+s: J.Val) -> J.Val:
  lanternward.case.outk(s, J.JBool{ga.b(s, "$ret")})

def case.lanternward.kit.Session.sweep?(+input: J.Val) -> J.Val:
  lanternward.case.landed(
    lanternward.kit.Session.sweep(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "reach"),
      ga.u(lanternward.case.a(input), "cos"), ga.u(lanternward.case.a(input), "dmg"), ga.b(lanternward.case.a(input), "heavy")))

def case.lanternward.kit.Session.lunge_hit?(+input: J.Val) -> J.Val:
  lanternward.case.landed(lanternward.kit.Session.lunge_hit(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id")))

def lanternward.case.ss?(+input: J.Val) -> J.Val:
  ga.sv(ga.sv(J.get(input, "s"), "$r", J.get(input, "r")), "$ev", lanternward.Events.default())

# ---- blow ----

def case.lanternward.blow.Session.bolt_meets?(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.blow.Session.bolt_meets(lanternward.case.ss(input), ga.u(lanternward.case.a(input), "i")), J.JNull{})

def case.lanternward.blow.Session.bolt_ends?(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.blow.Session.bolt_ends(lanternward.case.ss(input), ga.u(lanternward.case.a(input), "i")), J.JNull{})

def case.lanternward.blow.Session.turn_bolt?(+input: J.Val) -> J.Val:
  lanternward.case.outs(
    lanternward.blow.Session.turn_bolt(lanternward.case.ss(input), ga.u(lanternward.case.a(input), "i"), ga.u(lanternward.case.a(input), "id"), ga.b(lanternward.case.a(input), "parried")),
    J.JNull{})

def case.lanternward.blow.Session.light_foe?(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.blow.Session.light_foe(lanternward.case.ss(input), ga.u(lanternward.case.a(input), "i")), J.JNull{})

# ---- foes ----

def case.lanternward.foes.Danger.hits?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.foes.Danger.hits(J.get(input, "d"), ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "rad"))}

def case.lanternward.foes.Foe.tell?(+input: J.Val) -> J.Val:
  J.JU{lanternward.foes.Foe.tell(J.get(input, "f"), J.get(input, "r"), ga.u(input, "seats"))}

def case.lanternward.foes.Foe.vulnerable?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.foes.Foe.vulnerable(J.get(input, "f"))}

def case.lanternward.foes.Foe.new?(+input: J.Val) -> J.Val:
  lanternward.foes.Foe.new(ga.u(input, "id"), ga.s(input, "kind"), ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "seats"))

def case.lanternward.foes.step_snuffer?(+input: J.Val) -> J.Val:
  lanternward.case.snuffer.out(
    lanternward.foes.step_snuffer(
      ga.sv(ga.sv(J.obj(), "$f", J.get(input, "f")), "$strikes", J.JA{Nil{}}), J.get(input, "target"), ga.u(input, "index")))

def lanternward.case.snuffer.out?(+s: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "strikes", J.get(s, "$strikes")), "f", J.get(s, "$f"))

def case.lanternward.foes.step_turret?(+input: J.Val) -> J.Val:
  lanternward.case.turret.out(
    lanternward.foes.step_turret(
      ga.sv(ga.sv(ga.sv(ga.sv(J.obj(), "$f", J.get(input, "f")), "$shots", J.JA{Nil{}}), "$r", J.get(input, "r")), "$ev", lanternward.Events.default()),
      J.get(input, "target"), ga.u(input, "seats")))

def lanternward.case.turret.out?(+s: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(J.obj(), "ev", J.get(s, "$ev")), "shots", J.get(s, "$shots")), "f", J.get(s, "$f"))

def case.lanternward.foes.Session.strike_with?(+input: J.Val) -> J.Val:
  lanternward.case.out2(
    lanternward.foes.Session.strike_with(lanternward.case.s2(input), ga.u(input, "i"), J.get(input, "danger"), ga.u(input, "dmg")))

# ---- boss ----

def case.lanternward.boss.Warden.new?(+input: J.Val) -> J.Val:
  lanternward.boss.Warden.new(ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "seats"))

def case.lanternward.boss.Warden.valid?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.boss.Warden.valid(J.get(input, "w"))}

def case.lanternward.boss.Warden.need?(+input: J.Val) -> J.Val:
  J.JU{lanternward.boss.Warden.need(J.get(input, "w"), ga.u(input, "seats"))}

def case.lanternward.boss.Warden.exposed?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.boss.Warden.exposed(J.get(input, "w"), ga.u(input, "now"), ga.u(input, "seats"))}

def case.lanternward.boss.Warden.heart?(+input: J.Val) -> J.Val:
  ga.pair_i(lanternward.boss.Warden.heart.x(J.get(input, "w")), lanternward.boss.Warden.heart.y(J.get(input, "w")))

def lanternward.case.sw?(+input: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(J.get(input, "s"), "$w", J.get(input, "w")), "$r", J.get(input, "r")), "$ev", lanternward.Events.default())

def lanternward.case.outw?(+s: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "w", J.get(s, "$w")), "s", lanternward.Session.bare(s))

def case.lanternward.boss.Session.begin_attack?(+input: J.Val) -> J.Val:
  lanternward.case.outw(
    lanternward.boss.Session.begin_attack(lanternward.case.sw(input), ga.nu(J.get(input, "at"), 0), ga.nu(J.get(input, "at"), 1)))

def case.lanternward.boss.Session.warden_hits?(+input: J.Val) -> J.Val:
  lanternward.case.warden_hits.out(
    lanternward.boss.Session.warden_hits(
      ga.sb(lanternward.case.sw(input), "$parried", False{}), J.get(input, "line"), ga.u(input, "dmg"), ga.s(input, "kind")))

def lanternward.case.warden_hits.out?(+s: J.Val) -> J.Val:
  ga.sb(ga.sv(ga.sv(ga.sv(J.obj(), "ev", J.get(s, "$ev")), "w", J.get(s, "$w")), "s", lanternward.Session.bare(s)), "parried", ga.b(s, "$parried"))

def case.lanternward.boss.Session.summon?(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.Session.bare(lanternward.boss.Session.summon(ga.sv(J.get(input, "s"), "$w", J.get(input, "w")))))

def case.lanternward.boss.Session.prop_at?(+input: J.Val) -> J.Val:
  lanternward.boss.Session.prop_at(J.get(input, "s"), ga.s(input, "kind"), ga.u(input, "tx"), ga.u(input, "ty"))

def case.lanternward.boss.Session.stage_warden?(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.boss.Session.stage_warden(lanternward.case.s2(input)))

# ---- stage ----

def case.lanternward.stage.Session.stage_lamp_hall?(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.stage_lamp_hall(lanternward.case.ss(input)), J.JNull{})

def case.lanternward.stage.Session.stage_gallery?(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.stage_gallery(lanternward.case.ss(input)), J.JNull{})

def case.lanternward.stage.Session.stage_vault?(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.stage_vault(lanternward.case.ss(input)), J.JNull{})

def case.lanternward.stage.Session.open_exit?(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.open_exit(lanternward.case.ss(input)), J.JNull{})

def case.lanternward.stage.Session.provision_room?(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.provision_room(lanternward.case.ss(input), ga.u(lanternward.case.a(input), "room")), J.JNull{})

def case.lanternward.stage.Session.spawn_wave?(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.spawn_wave(lanternward.case.ss(input)), J.JNull{})

def case.lanternward.stage.Session.all_lit?(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.case.ss(input), J.JBool{lanternward.stage.Session.all_lit(lanternward.case.ss(input), ga.s(lanternward.case.a(input), "kind"))})

def case.lanternward.stage.Session.foes_down?(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.case.ss(input), J.JBool{lanternward.stage.Session.foes_down(lanternward.case.ss(input))})

def case.lanternward.stage.Session.prop_of?(+input: J.Val) -> J.Val:
  lanternward.case.outs(
    lanternward.case.ss(input),
    lanternward.stage.Session.prop_of(lanternward.case.ss(input), ga.s(lanternward.case.a(input), "kind"), ga.u(lanternward.case.a(input), "id")))

def lanternward.ctx.Party.new?() -> J.Val:
  ga.su(ga.sv(ga.su(ga.sb(ga.sb(J.obj(), "map", False{}), "lantern", False{}), "small_keys", 0), "boss", J.JS{"Unfound"}), "containers", 0)

# Whether the boss key is carried by this seat.

def lanternward.ctx.Session.carrying_boss_key?(+s: J.Val, +k: J.Val) -> Bool:
  +b = ga.v(ga.v(s, "inv"), "boss")
  Bool.and(lanternward.tag_is(b, "Carried"), U32.is_eq(ga.nu(lanternward.payload(b), 0), ga.u(k, "id")))

def lanternward.ctx.Session.beacon_ready?(+s: J.Val) -> Bool:
  Bool.and(U32.is_eq(ga.u(s, "room"), 4), U32.is_ge(ga.u(s, "stage"), 3))

# The first revive reach over the fallen: seats in id order.

def lanternward.ctx.Session.context.revive?(+ids: List<&2, U32>, +s: J.Val, +k: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      J.JNull{}
    case Con{+d, +t}:
      lanternward.ctx.Session.context.revive.at(
        Bool.and(
          lanternward.is(ga.at(ga.v(s, "seats"), d), "act", "Down"),
          RS.i_lt(lanternward.world.dist(
            (ga.u(ga.at(ga.v(s, "seats"), d), "x") - ga.u(k, "x") : U32),
            (ga.u(ga.at(ga.v(s, "seats"), d), "y") - ga.u(k, "y") : U32)), 6144)),
        d, t, s, k)

def lanternward.ctx.Session.context.revive.at?(hit: Bool, +d: U32, +t: List<&2, U32>, +s: J.Val, +k: J.Val) -> J.Val:
  match hit:
    case True{}:
      lanternward.variant("Revive", J.JU{U32.and(d, 255)})
    case False{}:
      lanternward.ctx.Session.context.revive(t, s, k)

# The Ctx a prop would give, or null for none (the `continue`s).

def lanternward.ctx.Session.context.kind?(+s: J.Val, +k: J.Val, +p: J.Val, +i: U32) -> J.Val:
  +kd = ga.s(p, "kind")
  +on0 = U32.is_eq(ga.u(p, "on"), 0)
  Bool.pick(J.Val, Bool.and(String.eq(kd, "Chest"), on0), lanternward.variant("Open", J.JU{U32.and(i, 255)}),
    Bool.pick(J.Val, String.eq(kd, "Sign"), lanternward.variant("Read", J.JU{U32.and(i, 255)}),
      Bool.pick(J.Val, String.eq(kd, "Folk"), lanternward.variant("Talk", J.JU{U32.and(i, 255)}),
        Bool.pick(J.Val, Bool.and(String.eq(kd, "Lever"), on0), lanternward.variant("Pull", J.JU{U32.and(i, 255)}),
          Bool.pick(J.Val, Bool.and(String.eq(kd, "Pot"), Bool.and(on0, U32.is_eq(ga.u(p, "id"), 0))), lanternward.variant("Lift", J.JU{U32.and(i, 255)}),
            Bool.pick(J.Val, String.eq(kd, "Ember"), J.JS{"Flame"},
              Bool.pick(J.Val, Bool.and(String.eq(kd, "Brazier"), Bool.and(Bool.not(lanternward.props.Prop.lit(p)), U32.is_gt(ga.u(k, "taper"), 0))), lanternward.variant("Light", J.JU{U32.and(i, 255)}),
                Bool.pick(J.Val, Bool.and(String.eq(kd, "Lock"), Bool.and(on0, U32.is_gt(ga.u(ga.v(s, "inv"), "small_keys"), 0))), lanternward.variant("Unlock", J.JU{U32.and(i, 255)}),
                  Bool.pick(J.Val, Bool.and(String.eq(kd, "Boss"), Bool.and(on0, lanternward.ctx.Session.carrying_boss_key(s, k))), lanternward.variant("Unlock", J.JU{U32.and(i, 255)}),
                    Bool.pick(J.Val, Bool.and(String.eq(kd, "Beacon"), Bool.and(on0, lanternward.ctx.Session.beacon_ready(s))), J.JS{"Relight"},
                      J.JNull{}))))))))))

# Walks the props keeping the nearest in reach and in front. `bc` is null for
# no candidate yet, else the best Ctx at distance `bd`.

def lanternward.ctx.Session.context.props?(+s: J.Val, +k: J.Val, +ps: List<&2, J.Val>, +i: U32, +bd: U32, +bc: J.Val) -> J.Val:
  match ps:
    case Nil{}:
      bc
    case Con{+p, +t}:
      lanternward.ctx.Session.context.cand(lanternward.ctx.Session.context.kind(s, k, p, i), s, k, p, t, i, bd, bc)

def lanternward.ctx.Session.context.cand?(+c: J.Val, +s: J.Val, +k: J.Val, +p: J.Val, +t: List<&2, J.Val>, +i: U32, +bd: U32, +bc: J.Val) -> J.Val:
  match c:
    case J.JNull{}:
      lanternward.ctx.Session.context.props(s, k, t, (i + 1 : U32), bd, bc)
    case _:
      lanternward.ctx.Session.context.dist(
        (ga.u(p, "x") - ga.u(k, "x") : U32), (ga.u(p, "y") - ga.u(k, "y") : U32), c, s, k, t, i, bd, bc)

def lanternward.ctx.Session.context.dist?(+dx: U32, +dy: U32, +c: J.Val, +s: J.Val, +k: J.Val, +t: List<&2, J.Val>, +i: U32, +bd: U32, +bc: J.Val) -> J.Val:
  lanternward.ctx.Session.context.take(
    lanternward.world.dist(dx, dy), dx, dy, c, s, k, t, i, bd, bc)

def lanternward.ctx.Session.context.take?(+d: U32, +dx: U32, +dy: U32, +c: J.Val, +s: J.Val, +k: J.Val, +t: List<&2, J.Val>, +i: U32, +bd: U32, +bc: J.Val) -> J.Val:
  +reach = RS.i_gt(d, 5120)
  +face = ga.v(k, "face")
  +behind = Bool.and(
    RS.i_gt(d, 3072),
    RS.i_lt(lanternward.ctx.Session.context.facing(face, dx, dy), 88))
  +skip = Bool.or(reach, behind)
  +better = Bool.or(J.is_null(bc), RS.i_lt(d, bd))
  lanternward.ctx.Session.context.props(s, k, t, (i + 1 : U32),
    Bool.pick(U32, Bool.and(Bool.not(skip), better), d, bd),
    Bool.pick(J.Val, Bool.and(Bool.not(skip), better), c, bc))

def lanternward.ctx.Session.context.facing?(+face: J.Val, +dx: U32, +dy: U32) -> U32:
  lanternward.world.dot_unit(ga.nu(face, 0), ga.nu(face, 1), dx, dy)

# The nearest thing in reach and in front of the knight that E would act on.

def lanternward.ctx.Session.context?(+s: J.Val, +k: J.Val) -> J.Val:
  lanternward.ctx.Session.context.carry(lanternward.is(k, "carry", "Pot"), s, k)

def lanternward.ctx.Session.context.carry?(pot: Bool, +s: J.Val, +k: J.Val) -> J.Val:
  match pot:
    case True{}:
      J.JS{"Throw"}
    case False{}:
      lanternward.ctx.Session.context.fin(
        lanternward.ctx.Session.context.revive(ga.ids(J.get(s, "seats")), s, k), s, k)

def lanternward.ctx.Session.context.fin?(+rv: J.Val, +s: J.Val, +k: J.Val) -> J.Val:
  match rv:
    case J.JNull{}:
      lanternward.ctx.Session.context.none(
        lanternward.ctx.Session.context.props(s, k, ga.items(s, "props"), 0, 0, J.JNull{}))
    case _:
      rv

def lanternward.ctx.Session.context.none?(+c: J.Val) -> J.Val:
  match c:
    case J.JNull{}:
      J.JS{"None"}
    case _:
      c

# The first prop of this kind (and id, unless `id` is 4294967295), or null.

def lanternward.find_prop.go?(+ps: List<&2, J.Val>, +kind: String, +id: U32) -> J.Val:
  match ps:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      lanternward.find_prop.hit(
        Bool.and(String.eq(ga.s(h, "kind"), kind), Bool.or(U32.is_eq(id, 4294967295), U32.is_eq(ga.u(h, "id"), id))),
        h, t, kind, id)

def lanternward.find_prop.hit?(hit: Bool, +h: J.Val, +t: List<&2, J.Val>, +kind: String, +id: U32) -> J.Val:
  match hit:
    case True{}:
      h
    case False{}:
      lanternward.find_prop.go(t, kind, id)

def lanternward.find_prop?(+s: J.Val, +kind: String, +id: U32) -> J.Val:
  lanternward.find_prop.go(ga.items(s, "props"), kind, id)

# Ticks a brazier lit from an ember burns: shorter the more hands.

def lanternward.ctx.Session.burn_ticks?(+s: J.Val) -> U32:
  RS.u_max(RS.u_sat_sub(600, (60 * (RS.u_max(lanternward.seat_count(s), 1) - 1 : U32) : U32)), 300)

def lanternward.ctx.Session.light_crystal?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.ctx.Session.light_crystal.go(U32.is_eq(lanternward.eu(s, "props", i, "on"), 1), s, i)

def lanternward.ctx.Session.light_crystal.go?(done: Bool, +s: J.Val, +i: U32) -> J.Val:
  match done:
    case True{}:
      s
    case False{}:
      lanternward.props.Session.solved(
        lanternward.es(s, "props", i, "on", 1),
        lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"))

# Start a block sliding one tile the way it was pushed, if the place is open.

def lanternward.ctx.against?(+room: U32, +dtx: U32, +dty: U32) -> Bool:
  Bool.or(
    Bool.or(lanternward.ctx.against.at(lanternward.world.Plan.at(room, (dtx + 1 : U32), dty)),
      lanternward.ctx.against.at(lanternward.world.Plan.at(room, (dtx - 1 : U32), dty))),
    Bool.or(lanternward.ctx.against.at(lanternward.world.Plan.at(room, dtx, (dty + 1 : U32))),
      lanternward.ctx.against.at(lanternward.world.Plan.at(room, dtx, (dty - 1 : U32)))))

def lanternward.ctx.against.at?(+c: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(c, 35), U32.is_eq(c, 86)), Bool.or(U32.is_eq(c, 61), U32.is_eq(c, 81)))

# Whether a prop other than block `b` and the plates stops a body at (x, y).

def lanternward.ctx.others_block?(+ps: List<&2, J.Val>, +j: U32, +b: U32, +x: U32, +y: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+q, +t}:
      Bool.or(
        Bool.and(U32.is_ne(j, b),
          Bool.and(Bool.not(String.eq(ga.s(q, "kind"), "Plate")), lanternward.props.Prop.blocks(q, x, y))),
        lanternward.ctx.others_block(t, (j + 1 : U32), b, x, y))

def lanternward.ctx.Session.start_slide?(+s: J.Val, +b: U32) -> J.Val:
  +on = lanternward.eu(s, "props", b, "on")
  +dx = Bool.pick(U32, U32.is_eq(on, 1), 1, Bool.pick(U32, U32.is_eq(on, 2), 4294967295, 0))
  +dy = Bool.pick(U32, U32.is_eq(on, 3), 1, Bool.pick(U32, Bool.or(U32.is_eq(on, 1), U32.is_eq(on, 2)), 0, 4294967295))
  lanternward.ctx.Session.start_slide.go(
    s, b, (lanternward.eu(s, "props", b, "x") + dx * 4096 : U32), (lanternward.eu(s, "props", b, "y") + dy * 4096 : U32))

def lanternward.ctx.Session.start_slide.go?(+s: J.Val, +b: U32, +nx: U32, +ny: U32) -> J.Val:
  +room = ga.u(s, "room")
  +dtx = lanternward.world.tile_of(nx)
  +dty = lanternward.world.tile_of(ny)
  +c = lanternward.world.Plan.at(room, dtx, dty)
  +open = Bool.and(
    Bool.and(Bool.not(lanternward.ctx.against(room, dtx, dty)), Bool.not(lanternward.world.wall(c))),
    Bool.and(Bool.and(U32.is_ne(c, 61), U32.is_ne(c, 81)),
      Bool.not(lanternward.ctx.others_block(ga.items(s, "props"), 0, b, nx, ny))))
  lanternward.ctx.Session.start_slide.set(open, s, b)

def lanternward.ctx.Session.start_slide.set?(open: Bool, +s: J.Val, +b: U32) -> J.Val:
  match open:
    case True{}:
      lanternward.es(s, "props", b, "t", 16)
    case False{}:
      lanternward.es(lanternward.es(s, "props", b, "on", 0), "props", b, "t", 0)

# ---- items ----

# An item comes to the party; its finder holds it up. (`$k` is the finder.)

def lanternward.ctx.Session.grant.seats?(+ids: List<&2, U32>, +s: J.Val, +item: String, +n: U32) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.ctx.Session.grant.seats(t, lanternward.ctx.Session.grant.one(s, id, item, n), item, n)

# One other seat's share of a lantern (flame) or a container (hearts).

def lanternward.ctx.Session.grant.one?(+s: J.Val, +id: U32, +item: String, +n: U32) -> J.Val:
  +o = ga.at(J.get(s, "seats"), id)
  ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id,
    Bool.pick(J.Val, String.eq(item, "Lantern"), ga.su(o, "flame", lanternward.R(s, "flame")),
      Bool.pick(J.Val, String.eq(item, "Container"),
        ga.su(ga.su(o, "max_hp", (ga.u(o, "max_hp") + 4 : U32)), "hp", (ga.u(o, "max_hp") + 4 : U32)),
        o))))

def lanternward.ctx.Session.grant.effect.go?(+code: U32, +s: J.Val, +id: U32, +item: String, +n: U32) -> J.Val:
  match code:
    case 1:
      ga.sv(s, "inv", ga.sb(ga.v(s, "inv"), "map", True{}))
    case 2:
      lanternward.ctx.Session.grant.seats(ga.ids(J.get(s, "seats")),
        lanternward.ks(ga.sv(s, "inv", ga.sb(ga.v(s, "inv"), "lantern", True{})), "flame", lanternward.R(s, "flame")),
        item, n)
    case 3:
      ga.sv(s, "inv", ga.su(ga.v(s, "inv"), "small_keys", RS.u_min((ga.u(ga.v(s, "inv"), "small_keys") + 1 : U32), 255)))
    case 4:
      ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss",
        lanternward.variant("Carried", J.JA{[J.JU{U32.and(id, 255)}, J.of_i32(lanternward.ku(s, "x")), J.of_i32(lanternward.ku(s, "y"))]})))
    case 5:
      lanternward.ctx.Session.grant.seats(ga.ids(J.get(s, "seats")),
        lanternward.ks(
          lanternward.ks(
            ga.sv(s, "inv", ga.su(ga.v(s, "inv"), "containers", RS.u_min((ga.u(ga.v(s, "inv"), "containers") + 1 : U32), 255))),
            "max_hp", (lanternward.ku(s, "max_hp") + 4 : U32)),
          "hp", (lanternward.ku(s, "max_hp") + 4 : U32)),
        item, n)
    case 6:
      lanternward.ks(s, "gems", (lanternward.ku(s, "gems") + n : U32))
    case _:
      lanternward.ks(s, "bombs", (lanternward.ku(s, "bombs") + n : U32))

def lanternward.ctx.Session.grant.code?(+item: String) -> U32:
  Bool.pick(U32, String.eq(item, "Map"), 1,
    Bool.pick(U32, String.eq(item, "Lantern"), 2,
      Bool.pick(U32, String.eq(item, "SmallKey"), 3,
        Bool.pick(U32, String.eq(item, "BossKey"), 4,
          Bool.pick(U32, String.eq(item, "Container"), 5,
            Bool.pick(U32, String.eq(item, "Gems"), 6, 7))))))

def lanternward.ctx.Session.grant.fin?(+s: J.Val, +id: U32, +item: J.Val, +tag: String) -> J.Val:
  lanternward.sound(
    ga.sv(
      lanternward.ks(
        lanternward.ks(
          lanternward.ks(
            lanternward.ks(
              lanternward.ksv(lanternward.ksv(lanternward.ksn(lanternward.ks(lanternward.ks(s, "t", 0), "lamp", 0), "act", "Item"), "item", item), "lock", J.JNull{}),
              "shield", 0),
            "chain", 0),
          "step", 0),
        "charge", 0),
      "banner",
      ga.su(ga.su(ga.sv(J.obj(), "item", item), "seat", U32.and(id, 255)), "t", 60)),
    Bool.pick(String, lanternward.NAME_IN(tag, ["Gems", "Bombs"]), "spoil_pickup", "wheel_land"))

def lanternward.ctx.Session.grant?(+s: J.Val, +id: U32, +item: J.Val) -> J.Val:
  lanternward.ctx.Session.grant.fin(
    lanternward.ctx.Session.grant.effect.go(
      lanternward.ctx.Session.grant.code(lanternward.tag(item)), s, id, lanternward.tag(item), J.u32(lanternward.payload(item))),
    id, item, lanternward.tag(item))

def lanternward.ctx.Session.open_chest?(+s: J.Val, +id: U32, +i: U32) -> J.Val:
  lanternward.ctx.Session.open_chest.go(
    lanternward.sound(lanternward.es(s, "props", i, "on", 1), "chest_open"),
    id, i, lanternward.props.chest_item(ga.u(s, "room"), lanternward.eu(s, "props", i, "id")))

def lanternward.ctx.Session.open_chest.go?(+s: J.Val, +id: U32, +i: U32, +item: J.Val) -> J.Val:
  lanternward.ctx.Session.grant(
    lanternward.props.Session.fx(s, "Light", lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 0),
    id, item)

# The Ctx variant's code: 0 None, 1 Open, 2 Read, 3 Lift, 4 Throw, 5 Flame,
# 6 Light, 7 Unlock, 8 Revive, 9 Relight, 10 Talk, 11 Pull.

def lanternward.ctx.tag_code?(+t: String) -> U32:
  Bool.pick(U32, String.eq(t, "Open"), 1,
    Bool.pick(U32, String.eq(t, "Read"), 2,
      Bool.pick(U32, String.eq(t, "Lift"), 3,
        Bool.pick(U32, String.eq(t, "Throw"), 4,
          Bool.pick(U32, String.eq(t, "Flame"), 5,
            Bool.pick(U32, String.eq(t, "Light"), 6,
              Bool.pick(U32, String.eq(t, "Unlock"), 7,
                Bool.pick(U32, String.eq(t, "Revive"), 8,
                  Bool.pick(U32, String.eq(t, "Relight"), 9,
                    Bool.pick(U32, String.eq(t, "Talk"), 10,
                      Bool.pick(U32, String.eq(t, "Pull"), 11, 0)))))))))))

def lanternward.ctx.Session.use_context.pull?(has: Bool, +s: J.Val, +g: J.Val) -> J.Val:
  match has:
    case False{}:
      s
    case True{}:
      lanternward.sound(
        lanternward.props.Session.solved(
          lanternward.props.Session.set_gate(s, 2, True{}), ga.u(g, "x"), ga.u(g, "y")),
        "door_open")

def lanternward.ctx.Session.use_context.throw?(ok: Bool, +s: J.Val, +id: U32) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.push(s, "bolts",
        ga.su(
          ga.su(
            ga.si(
              ga.si(
                ga.si(
                  ga.si(ga.sv(J.obj(), "kind", J.JS{"Pot"}), "x",
                    (lanternward.ku(s, "x") + lanternward.kface(s, 0) * 8 : U32)),
                  "y", (lanternward.ku(s, "y") + lanternward.kface(s, 1) * 8 : U32)),
                "vx", RS.i_div((lanternward.kface(s, 0) * 1400 : U32), 256)),
              "vy", RS.i_div((lanternward.kface(s, 1) * 1400 : U32), 256)),
            "ttl", 10),
          "owner", U32.and(id, 255)))

def lanternward.ctx.Session.use_context.unlock?(lock: Bool, +s: J.Val, +i: U32) -> J.Val:
  match lock:
    case True{}:
      ga.sv(s, "inv", ga.su(ga.v(s, "inv"), "small_keys", RS.u_sat_sub(ga.u(ga.v(s, "inv"), "small_keys"), 1)))
    case False{}:
      ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss", J.JS{"Spent"}))

def lanternward.ctx.Session.use_context.go?(+code: U32, +i: U32, +s: J.Val, +id: U32) -> J.Val:
  match code:
    case 0:
      ga.sb(s, "$ret", False{})
    case 1:
      ga.sb(lanternward.ctx.Session.open_chest(s, id, i), "$ret", True{})
    case 2:
      ga.sb(
        lanternward.sound(
          lanternward.ksv(s, "read", ga.pair(J.JU{lanternward.eu(s, "props", i, "id")}, J.JU{120})), "card_pickup"),
        "$ret", True{})
    case 10:
      ga.sb(
        lanternward.sound(
          lanternward.ksv(s, "read", ga.pair(J.JU{(100 + lanternward.eu(s, "props", i, "id") : U32)}, J.JU{120})), "card_pickup"),
        "$ret", True{})
    case 11:
      ga.sb(
        lanternward.ctx.Session.use_context.pull(
          Bool.not(J.is_null(lanternward.find_prop(s, "Gate", 2))),
          lanternward.sound(lanternward.es(s, "props", i, "on", 1), "wall_up"),
          lanternward.find_prop(s, "Gate", 2)),
        "$ret", True{})
    case 3:
      ga.sb(lanternward.sound(lanternward.ksn(lanternward.es(s, "props", i, "on", 1), "carry", "Pot"), "bomb_toss"), "$ret", True{})
    case 4:
      ga.sb(
        lanternward.sound(
          lanternward.ctx.Session.use_context.throw(U32.is_lt(lanternward.len(s, "bolts"), 40), lanternward.ksn(s, "carry", "Nothing"), id),
          "bomb_toss"),
        "$ret", True{})
    case 5:
      ga.sb(
        lanternward.sound(
          lanternward.props.Session.fx(lanternward.ks(s, "taper", 420), "Light", lanternward.ku(s, "x"), (lanternward.ku(s, "y") - 2048 : U32), 0),
          "spell_fire"),
        "$ret", True{})
    case 6:
      ga.sb(
        lanternward.sound(
          lanternward.props.Session.fx(
            lanternward.es(s, "props", i, "t", lanternward.ctx.Session.burn_ticks(s)),
            "Light", lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 0),
          "spell_fire"),
        "$ret", True{})
    case 7:
      ga.sb(
        lanternward.sound(
          lanternward.props.Session.solved(
            lanternward.es(
              lanternward.ctx.Session.use_context.unlock(lanternward.eis(s, "props", i, "kind", "Lock"), s, i),
              "props", i, "on", 1),
            lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y")),
          "door_open"),
        "$ret", True{})
    case _:
      ga.sb(s, "$ret", True{})

# E pressed with a thing in reach. `$ret` is false when there is nothing to do
# (the caller rolls or hops). `$k` is the pressing seat.

def lanternward.ctx.Session.use_context?(+s: J.Val, +id: U32) -> J.Val:
  +c = lanternward.ctx.Session.context(s, J.get(s, "$k"))
  lanternward.ctx.Session.use_context.go(
    lanternward.ctx.tag_code(lanternward.tag(c)), J.u32(lanternward.payload(c)), s, id)

# The index of the first prop of this kind, or null.

def lanternward.find_prop_index.go?(+ps: List<&2, J.Val>, +kind: String, +i: U32) -> J.Val:
  match ps:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      lanternward.find_prop_index.hit(String.eq(ga.s(h, "kind"), kind), t, kind, i)

def lanternward.find_prop_index.hit?(hit: Bool, +t: List<&2, J.Val>, +kind: String, +i: U32) -> J.Val:
  match hit:
    case True{}:
      J.JU{i}
    case False{}:
      lanternward.find_prop_index.go(t, kind, (i + 1 : U32))

def lanternward.find_prop_index?(+s: J.Val, +kind: String) -> J.Val:
  lanternward.find_prop_index.go(ga.items(s, "props"), kind, 0)

def lanternward.foes.seg_dist2?(+px: U32, +py: U32, +ax: U32, +ay: U32, +bx: U32, +by: U32) -> W.W:
  lanternward.foes.seg_dist2.go(
    lanternward.i64.sub(lanternward.i64.of_i32(px), lanternward.i64.of_i32(ax)),
    lanternward.i64.sub(lanternward.i64.of_i32(py), lanternward.i64.of_i32(ay)),
    lanternward.i64.sub(lanternward.i64.of_i32(bx), lanternward.i64.of_i32(ax)),
    lanternward.i64.sub(lanternward.i64.of_i32(by), lanternward.i64.of_i32(ay)),
    lanternward.i64.of_i32(ax), lanternward.i64.of_i32(ay), lanternward.i64.of_i32(px), lanternward.i64.of_i32(py))

def lanternward.foes.seg_dist2.go?(+pax: W.W, +pay: W.W, +dx: W.W, +dy: W.W, +ax: W.W, +ay: W.W, +px: W.W, +py: W.W) -> W.W:
  +len2 = lanternward.i64.add(lanternward.i64.mul(dx, dx), lanternward.i64.mul(dy, dy))
  +t = Bool.pick(W.W, W.is_zero(len2), W.zero(),
    lanternward.i64.clamp(
      lanternward.i64.div(
        lanternward.i64.mul(lanternward.i64.add(lanternward.i64.mul(pax, dx), lanternward.i64.mul(pay, dy)), W.W{0, 1024}),
        len2),
      W.zero(), W.W{0, 1024}))
  +cx = lanternward.i64.add(ax, lanternward.i64.div(lanternward.i64.mul(dx, t), W.W{0, 1024}))
  +cy = lanternward.i64.add(ay, lanternward.i64.div(lanternward.i64.mul(dy, t), W.W{0, 1024}))
  +ex = lanternward.i64.sub(px, cx)
  +ey = lanternward.i64.sub(py, cy)
  lanternward.i64.add(lanternward.i64.mul(ex, ex), lanternward.i64.mul(ey, ey))

# Whether a body of radius `rad` at (x, y) stands in the footprint.

def lanternward.foes.Danger.hits?(+d: J.Val, +x: U32, +y: U32, +rad: U32) -> Bool:
  +p = lanternward.payload(d)
  +tag = lanternward.tag(d)
  Bool.pick(Bool, String.eq(tag, "Capsule"), lanternward.foes.Danger.hits.capsule(p, x, y, rad),
    Bool.pick(Bool, String.eq(tag, "Circle"),
      RS.i_le(lanternward.world.dist((x - ga.u(p, "x") : U32), (y - ga.u(p, "y") : U32)), (ga.u(p, "r") + rad : U32)),
      Bool.pick(Bool, String.eq(tag, "Fan"), lanternward.foes.Danger.hits.fan(p, x, y, rad),
        Bool.pick(Bool, String.eq(tag, "Ring"),
          RS.i_le(RS.i_abs((lanternward.world.dist((x - ga.u(p, "x") : U32), (y - ga.u(p, "y") : U32)) - ga.u(p, "r") : U32)), (ga.u(p, "w") + rad : U32)),
          False{}))))

def lanternward.foes.Danger.hits.capsule?(+p: J.Val, +x: U32, +y: U32, +rad: U32) -> Bool:
  +reach = lanternward.i64.of_i32((ga.u(p, "r") + rad : U32))
  lanternward.i64.le(
    lanternward.foes.seg_dist2(x, y, ga.u(p, "ax"), ga.u(p, "ay"), ga.u(p, "bx"), ga.u(p, "by")),
    lanternward.i64.mul(reach, reach))

def lanternward.foes.Danger.hits.fan?(+p: J.Val, +x: U32, +y: U32, +rad: U32) -> Bool:
  +d = lanternward.world.dist((x - ga.u(p, "x") : U32), (y - ga.u(p, "y") : U32))
  Bool.and(
    RS.i_le(d, (ga.u(p, "r") + rad : U32)),
    Bool.or(
      RS.i_lt(d, rad),
      RS.i_ge(
        lanternward.foes.Danger.hits.fan_dot((x - ga.u(p, "x") : U32), (y - ga.u(p, "y") : U32), ga.u(p, "deg")),
        lanternward.world.cos_q(ga.u(p, "half")))))

def lanternward.foes.Danger.hits.fan_dot?(+dx: U32, +dy: U32, +deg: U32) -> U32:
  lanternward.foes.Danger.hits.fan_dot.go(lanternward.world.unit(dx, dy), lanternward.world.cos_q(deg), lanternward.world.sin_q(deg))

def lanternward.foes.Danger.hits.fan_dot.go?(u: U32 & U32, +cx: U32, +cy: U32) -> U32:
  (+ux, +uy) = u
  lanternward.world.dot4(ux, uy, cx, cy)

def lanternward.foes.Foe.new?(+id: U32, +kind: String, +x: U32, +y: U32, +seats: U32) -> J.Val:
  lanternward.foes.Foe.new.go(id, kind, x, y, RS.u_max(seats, 1))

def lanternward.foes.Foe.new.go?(+id: U32, +kind: String, +x: U32, +y: U32, +n: U32) -> J.Val:
  +full = (Bool.pick(U32, String.eq(kind, "Snuffer"), 6, 8) * n : U32)
  +f0 = ga.sv(J.obj(), "id", J.JU{id})
  +f1 = ga.si(ga.si(ga.sv(f0, "kind", J.JS{kind}), "x", x), "y", y)
  +f2 = ga.su(ga.su(ga.su(f1, "hp", full), "full", full), "prov", RS.u_clamp(n, 1, 6))
  +f3 = ga.si(ga.si(ga.su(ga.sv(f2, "st", J.JS{"Dormant"}), "t", 0), "fx", 0), "fy", 256)
  +f4 = ga.sb(ga.sv(f3, "danger", J.JS{"None"}), "hooded", False{})
  +f5 = ga.su(ga.su(ga.su(ga.su(f4, "lit", 0), "flinch", 0), "freeze", 0), "dead", 0)
  +f6 = ga.sv(ga.su(ga.sb(f5, "open", False{}), "struck", 0), "hit", J.JA{[J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}]})
  ga.su(ga.su(ga.sv(f6, "kb", ga.pair_i(0, 0)), "kbt", 0), "n", 0)

def lanternward.foes.Foe.alive?(+f: J.Val) -> Bool:
  Bool.and(U32.is_gt(ga.u(f, "hp"), 0), Bool.not(lanternward.is(f, "st", "Dying")))

# Its body's radius in Q pixels.

def lanternward.foes.Foe.radius?(+f: J.Val) -> U32:
  Bool.pick(U32, lanternward.is(f, "kind", "Snuffer"), 1536, 2048)

# A big foe's tell for a table of `seats` (Rules::tell_for).

def lanternward.Rules.tell_for?(+r: J.Val, +seats: U32) -> U32:
  RS.u_max(RS.u_sat_sub(ga.u(r, "tell"), (3 * RS.u_sat_sub(seats, 1) : U32)), 40)

# Its tell in ticks (`r` is the Rules).

def lanternward.foes.Foe.tell?(+f: J.Val, +r: J.Val, +seats: U32) -> U32:
  Bool.pick(U32, lanternward.is(f, "kind", "Snuffer"), 24, U32.and(lanternward.Rules.tell_for(r, seats), 65535))

# Whether a blade may hurt it now.

def lanternward.foes.Foe.vulnerable?(+f: J.Val) -> Bool:
  Bool.pick(Bool, lanternward.is(f, "kind", "Snuffer"),
    Bool.not(Bool.and(ga.b(f, "hooded"), U32.is_eq(ga.u(f, "lit"), 0))), False{})

def lanternward.foes.Bolt.light?(+b: J.Val) -> Bool:
  Bool.or(lanternward.is(b, "kind", "Light"), lanternward.is(b, "kind", "Bright"))

def lanternward.foes.Bolt.hostile?(+b: J.Val) -> Bool:
  lanternward.is(b, "kind", "Iron")

# ---- impl Session ----

# A foe of `kind` where a snuffer or turret mark stands.

def lanternward.foes.Session.spawn_foe?(+s: J.Val, +kind: String, +x: U32, +y: U32, +hooded: Bool) -> J.Val:
  lanternward.foes.Session.spawn_foe.go(U32.is_ge(lanternward.len(s, "foes"), 16), s, kind, x, y, hooded)

def lanternward.foes.Session.spawn_foe.go?(full: Bool, +s: J.Val, +kind: String, +x: U32, +y: U32, +hooded: Bool) -> J.Val:
  match full:
    case True{}:
      s
    case False{}:
      lanternward.push(
        ga.su(s, "next_foe", U32.and((ga.u(s, "next_foe") + 1 : U32), 255)),
        "foes",
        ga.sb(lanternward.foes.Foe.new(ga.u(s, "next_foe"), kind, x, y, lanternward.seat_count(s)), "hooded", hooded))

# Scale a foe or the Warden already standing to a grown table.

def lanternward.foes.provision.one?(+o: J.Val, +now: U32, +boss: Bool) -> J.Val:
  lanternward.foes.provision.one.go(
    Bool.and(U32.is_gt(now, ga.u(o, "prov")), U32.is_gt(ga.u(o, "hp"), 0)), o, now, boss)

def lanternward.foes.provision.one.go?(grow: Bool, +o: J.Val, +now: U32, +boss: Bool) -> J.Val:
  match grow:
    case False{}:
      o
    case True{}:
      +had = ga.u(o, "prov")
      +o1 = ga.su(ga.su(o, "hp", W.lo32(W.div_u32(W.mul32(ga.u(o, "hp"), now), had))), "full", U32.div((ga.u(o, "full") * now : U32), had))
      +o2 = Bool.pick(J.Val, boss, ga.su(o1, "start", W.lo32(W.div_u32(W.mul32(ga.u(o, "start"), now), had))), o1)
      ga.su(o2, "prov", U32.and(now, 255))

def lanternward.foes.provision.foes?(+s: J.Val, +i: U32, +now: U32) -> J.Val:
  lanternward.foes.provision.foes.go(U32.is_lt(i, lanternward.len(s, "foes")), s, i, now)

def lanternward.foes.provision.foes.go?(more: Bool, +s: J.Val, +i: U32, +now: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.foes.provision.foes(
        lanternward.eput(s, "foes", i, lanternward.foes.provision.one(lanternward.el(s, "foes", i), now, False{})),
        (i + 1 : U32), now)

# Scale foes already standing to a table that has grown.

def lanternward.foes.Session.provision?(+s: J.Val) -> J.Val:
  lanternward.foes.Session.provision.boss(
    lanternward.foes.provision.foes(s, 0, RS.u_clamp(lanternward.seat_count(s), 1, 6)),
    RS.u_clamp(lanternward.seat_count(s), 1, 6))

def lanternward.foes.Session.provision.boss?(+s: J.Val, +now: U32) -> J.Val:
  lanternward.foes.Session.provision.boss.go(J.is_null(J.get(s, "boss")), s, now)

def lanternward.foes.Session.provision.boss.go?(none: Bool, +s: J.Val, +now: U32) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      ga.sv(s, "boss", lanternward.foes.provision.one(J.get(s, "boss"), now, True{}))

# The living knight nearest a point, by distance then seat: null, or
# [id, x, y, d].

def lanternward.foes.Session.nearest_knight.go?(+ids: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32, +best: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      best
    case Con{+id, +t}:
      lanternward.foes.Session.nearest_knight.cand(ga.at(seats, id), id, t, seats, x, y, best)

def lanternward.foes.Session.nearest_knight.cand?(+k: J.Val, +id: U32, +t: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32, +best: J.Val) -> J.Val:
  lanternward.foes.Session.nearest_knight.pick(
    lanternward.kit.Seat.up(k), lanternward.world.dist((ga.u(k, "x") - x : U32), (ga.u(k, "y") - y : U32)), id, k, t, seats, x, y, best)

def lanternward.foes.Session.nearest_knight.pick?(up: Bool, +d: U32, +id: U32, +k: J.Val, +t: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32, +best: J.Val) -> J.Val:
  lanternward.foes.Session.nearest_knight.go(t, seats, x, y,
    Bool.pick(J.Val, Bool.and(up, Bool.or(J.is_null(best), RS.i_lt(d, ga.nu(best, 3)))),
      J.JA{[J.JU{id}, J.of_i32(ga.u(k, "x")), J.of_i32(ga.u(k, "y")), J.of_i32(d)]}, best))

def lanternward.foes.Session.nearest_knight?(+s: J.Val, +x: U32, +y: U32) -> J.Val:
  lanternward.foes.Session.nearest_knight.go(ga.ids(J.get(s, "seats")), J.get(s, "seats"), x, y, J.JNull{})

# Standing and awake: can be hurt, can hurt, can be a foe's target.

def lanternward.kit.Seat.up?(+k: J.Val) -> Bool:
  Bool.and(U32.is_gt(ga.u(k, "hp"), 0), Bool.not(lanternward.is(k, "act", "Down")))

# The nearest open point to one that is in a wall.

def lanternward.foes.Session.nudge.dirs?(+n: U32) -> U32 & U32:
  match n:
    case 0:
      (1, 0)
    case 1:
      (4294967295, 0)
    case 2:
      (0, 1)
    case 3:
      (0, 4294967295)
    case 4:
      (1, 1)
    case 5:
      (4294967295, 4294967295)
    case 6:
      (1, 4294967295)
    case _:
      (4294967295, 1)

def lanternward.foes.Session.nudge.try?(d: U32 & U32, +s: J.Val, +x: U32, +y: U32, +ring: U32, +r: U32) -> J.Val:
  (+dx, +dy) = d
  lanternward.foes.Session.nudge.try.at(
    lanternward.props.Session.clear(s, (x + dx * ring * 1024 : U32), (y + dy * ring * 1024 : U32), r),
    (x + dx * ring * 1024 : U32), (y + dy * ring * 1024 : U32))

def lanternward.foes.Session.nudge.try.at?(ok: Bool, +px: U32, +py: U32) -> J.Val:
  match ok:
    case True{}:
      ga.pair_i(px, py)
    case False{}:
      J.JNull{}

# ring 1..=8, then the eight directions in order: the first clear point.

def lanternward.foes.Session.nudge.go?(+k: U32, +s: J.Val, +x: U32, +y: U32, +r: U32) -> J.Val:
  lanternward.foes.Session.nudge.next(U32.is_ge(k, 64), k, s, x, y, r)

def lanternward.foes.Session.nudge.next?(done: Bool, +k: U32, +s: J.Val, +x: U32, +y: U32, +r: U32) -> J.Val:
  match done:
    case True{}:
      ga.pair_i(x, y)
    case False{}:
      lanternward.foes.Session.nudge.some(
        lanternward.foes.Session.nudge.try(
          lanternward.foes.Session.nudge.dirs(U32.mod(k, 8)), s, x, y, (U32.div(k, 8) + 1 : U32), r),
        k, s, x, y, r)

def lanternward.foes.Session.nudge.some?(+hit: J.Val, +k: U32, +s: J.Val, +x: U32, +y: U32, +r: U32) -> J.Val:
  match hit:
    case J.JNull{}:
      lanternward.foes.Session.nudge.go((k + 1 : U32), s, x, y, r)
    case _:
      hit

def lanternward.foes.Session.nudge?(+s: J.Val, +x: U32, +y: U32, +r: U32) -> J.Val:
  lanternward.foes.Session.nudge.go(0, s, x, y, r)

# ---- one snuffer / turret tick, on `$f` ----

def lanternward.foes.drift_dir?(+d: U32, +ux: U32, +uy: U32, +id: U32) -> U32 & U32:
  Bool.pick(U32 & U32, RS.i_gt(d, 6656), (ux, uy),
    Bool.pick(U32 & U32, RS.i_lt(d, 4608), (RS.i_neg(ux), RS.i_neg(uy)),
      Bool.pick(U32 & U32, U32.is_eq(U32.mod(id, 2), 0), (RS.i_neg(uy), ux), (uy, RS.i_neg(ux)))))

def lanternward.foes.snuffer.drift?(+s: J.Val, +tx: U32, +ty: U32, +d: U32) -> J.Val:
  lanternward.foes.snuffer.drift.go(lanternward.world.unit((tx - lanternward.fu(s, "x") : U32), (ty - lanternward.fu(s, "y") : U32)), s, tx, ty, d)

def lanternward.foes.snuffer.drift.go?(u: U32 & U32, +s: J.Val, +tx: U32, +ty: U32, +d: U32) -> J.Val:
  (+ux, +uy) = u
  lanternward.foes.snuffer.drift.move(lanternward.foes.drift_dir(d, ux, uy, lanternward.fu(s, "id")),
    lanternward.fsi(lanternward.fsi(s, "fx", ux), "fy", uy), tx, ty, d)

def lanternward.foes.snuffer.drift.move?(m: U32 & U32, +s: J.Val, +tx: U32, +ty: U32, +d: U32) -> J.Val:
  (+mx, +my) = m
  lanternward.foes.snuffer.drift.lock(
    lanternward.fs(
      lanternward.fsi(lanternward.fsi(s, "x", (lanternward.fu(s, "x") + RS.i_div((mx * 200 : U32), 256) : U32)),
        "y", (lanternward.fu(s, "y") + RS.i_div((my * 200 : U32), 256) : U32)),
      "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)),
    tx, ty, d)

def lanternward.foes.snuffer.drift.lock?(+s: J.Val, +tx: U32, +ty: U32, +d: U32) -> J.Val:
  lanternward.foes.snuffer.drift.lock.at(
    Bool.and(U32.is_eq(lanternward.fu(s, "t"), 0), RS.i_lt(d, 12800)), U32.is_eq(lanternward.fu(s, "t"), 0), s, tx, ty)

def lanternward.foes.snuffer.drift.lock.at?(go: Bool, zero: Bool, +s: J.Val, +tx: U32, +ty: U32) -> J.Val:
  match go:
    case True{}:
      lanternward.foes.snuffer.drift.line(lanternward.world.unit((tx - lanternward.fu(s, "x") : U32), (ty - lanternward.fu(s, "y") : U32)), s)
    case False{}:
      lanternward.foes.snuffer.drift.wait(zero, s)

def lanternward.foes.snuffer.drift.wait?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case True{}:
      lanternward.fs(s, "t", 8)
    case False{}:
      s

def lanternward.foes.snuffer.drift.line?(l: U32 & U32, +s: J.Val) -> J.Val:
  (+lx, +ly) = l
  lanternward.fs(
    lanternward.fsn(
      lanternward.fs(
        lanternward.fsv(
          lanternward.fsi(lanternward.fsi(s, "fx", lx), "fy", ly), "danger",
          lanternward.variant("Capsule",
            ga.si(ga.si(ga.si(ga.si(ga.si(J.obj(), "ax", lanternward.fu(s, "x")), "ay", lanternward.fu(s, "y")),
              "bx", (lanternward.fu(s, "x") + lx * 44 : U32)), "by", (lanternward.fu(s, "y") + ly * 44 : U32)), "r", 1792))),
        "t", 24),
      "st", "Tell"),
    "struck", 0)

# One tick of a snuffer: drift to a ring about the nearest knight, show the
# line it will lunge along, lunge, tire. `target` is null or [id, x, y, d];
# a strike is pushed on `$strikes`.

def lanternward.foes.step_snuffer?(+s: J.Val, +target: J.Val, +index: U32) -> J.Val:
  match target:
    case J.JNull{}:
      s
    case _:
      lanternward.foes.step_snuffer.st(
        ga.s(J.get(s, "$f"), "st"), s, ga.nu(target, 1), ga.nu(target, 2), ga.nu(target, 3), index)

def lanternward.foes.step_snuffer.st?(+st: String, +s: J.Val, +tx: U32, +ty: U32, +d: U32, +index: U32) -> J.Val:
  lanternward.foes.step_snuffer.code(
    Bool.pick(U32, String.eq(st, "Dormant"), 1,
      Bool.pick(U32, String.eq(st, "Drift"), 2,
        Bool.pick(U32, String.eq(st, "Tell"), 3,
          Bool.pick(U32, String.eq(st, "Strike"), 4,
            Bool.pick(U32, String.eq(st, "Recover"), 5,
              Bool.pick(U32, String.eq(st, "Stagger"), 6, 0)))))),
    s, tx, ty, d, index)

def lanternward.foes.step_snuffer.code?(+code: U32, +s: J.Val, +tx: U32, +ty: U32, +d: U32, +index: U32) -> J.Val:
  match code:
    case 1:
      lanternward.foes.step_snuffer.wake(
        Bool.or(RS.i_lt(d, 30720), U32.is_lt(lanternward.fu(s, "hp"), lanternward.fu(s, "full"))), s)
    case 2:
      lanternward.foes.snuffer.drift(s, tx, ty, d)
    case 3:
      lanternward.foes.step_snuffer.tell(U32.is_gt(lanternward.fu(s, "flinch"), 0), s)
    case 4:
      lanternward.foes.step_snuffer.strike(
        lanternward.fsi(lanternward.fsi(s, "x", (lanternward.fu(s, "x") + RS.i_div((lanternward.fu(s, "fx") * 1400 : U32), 256) : U32)),
          "y", (lanternward.fu(s, "y") + RS.i_div((lanternward.fu(s, "fy") * 1400 : U32), 256) : U32)),
        index)
    case 5:
      lanternward.foes.step_snuffer.recover(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case 6:
      lanternward.foes.step_snuffer.stagger(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case _:
      s

def lanternward.foes.step_snuffer.wake?(wake: Bool, +s: J.Val) -> J.Val:
  match wake:
    case True{}:
      lanternward.fs(lanternward.fsn(s, "st", "Drift"), "t", (18 + U32.mod(lanternward.fu(s, "id"), 5) * 4 : U32))
    case False{}:
      s

def lanternward.foes.step_snuffer.tell?(flinch: Bool, +s: J.Val) -> J.Val:
  match flinch:
    case True{}:
      lanternward.fs(lanternward.fsv(lanternward.fsn(s, "st", "Drift"), "danger", J.JS{"None"}), "t", 30)
    case False{}:
      lanternward.foes.step_snuffer.tell.tick(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))

def lanternward.foes.step_snuffer.tell.tick?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case True{}:
      lanternward.fs(lanternward.fsn(s, "st", "Strike"), "t", 8)
    case False{}:
      s

def lanternward.foes.step_snuffer.strike?(+s: J.Val, +index: U32) -> J.Val:
  lanternward.foes.step_snuffer.strike.end(
    U32.is_le(lanternward.fu(s, "t"), 1),
    lanternward.fs(
      lanternward.push(s, "$strikes", ga.trio(J.JU{index}, lanternward.fv(s, "danger"), J.JU{2})),
      "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))

def lanternward.foes.step_snuffer.strike.end?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case True{}:
      lanternward.fs(lanternward.fsv(lanternward.fsn(s, "st", "Recover"), "danger", J.JS{"None"}), "t", 28)
    case False{}:
      s

def lanternward.foes.step_snuffer.recover?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case True{}:
      lanternward.fs(
        lanternward.fs(lanternward.fsn(s, "st", "Drift"), "t",
          (22 + U32.mod(lanternward.fu(s, "id"), 4) * 6 + U32.mod(lanternward.fu(s, "n"), 3) * 4 : U32)),
        "n", U32.and((lanternward.fu(s, "n") + 1 : U32), 255))
    case False{}:
      s

def lanternward.foes.step_snuffer.stagger?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case True{}:
      lanternward.fs(lanternward.fsb(lanternward.fsn(s, "st", "Drift"), "open", False{}), "t", 24)
    case False{}:
      s

# One tick of a turret: wait for a knight in sight, lock the line, loose an
# iron bolt along it. Shots go on `$shots`.

def lanternward.foes.step_turret?(+s: J.Val, +target: J.Val, +seats: U32) -> J.Val:
  lanternward.foes.step_turret.st(ga.s(J.get(s, "$f"), "st"), s, target, seats)

def lanternward.foes.step_turret.st?(+st: String, +s: J.Val, +target: J.Val, +seats: U32) -> J.Val:
  lanternward.foes.step_turret.code(
    Bool.pick(U32, lanternward.NAME_IN(st, ["Dormant", "Drift"]), 1,
      Bool.pick(U32, String.eq(st, "Tell"), 2,
        Bool.pick(U32, String.eq(st, "Strike"), 3,
          Bool.pick(U32, String.eq(st, "Recover"), 4,
            Bool.pick(U32, String.eq(st, "Stagger"), 5, 0))))),
    s, target, seats)

def lanternward.foes.step_turret.code?(+code: U32, +s: J.Val, +target: J.Val, +seats: U32) -> J.Val:
  match code:
    case 1:
      lanternward.foes.step_turret.look(J.is_null(target), s, target, seats)
    case 2:
      lanternward.foes.step_turret.tell(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case 3:
      lanternward.foes.step_turret.strike(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case 4:
      lanternward.foes.step_turret.recover(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case 5:
      lanternward.foes.step_turret.stagger(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case _:
      s

def lanternward.foes.step_turret.look?(none: Bool, +s: J.Val, +target: J.Val, +seats: U32) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.foes.step_turret.aim(
        lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)), ga.nu(target, 1), ga.nu(target, 2), ga.nu(target, 3), seats)

def lanternward.foes.step_turret.aim?(+s: J.Val, +tx: U32, +ty: U32, +d: U32, +seats: U32) -> J.Val:
  lanternward.foes.step_turret.aim.go(
    Bool.and(RS.i_lt(d, 66560), U32.is_eq(lanternward.fu(s, "t"), 0)),
    s, lanternward.world.unit((tx - lanternward.fu(s, "x") : U32), (ty - lanternward.fu(s, "y") : U32)), seats)

def lanternward.foes.step_turret.aim.go?(fire: Bool, +s: J.Val, u: U32 & U32, +seats: U32) -> J.Val:
  match fire:
    case False{}:
      s
    case True{}:
      lanternward.foes.step_turret.aim.fire(u, s, seats)

def lanternward.foes.step_turret.aim.fire?(u: U32 & U32, +s: J.Val, +seats: U32) -> J.Val:
  (+ux, +uy) = u
  lanternward.fs(
    lanternward.fsn(
      lanternward.fsv(
        lanternward.fsi(lanternward.fsi(s, "fx", ux), "fy", uy), "danger",
        lanternward.variant("Capsule",
          ga.si(ga.si(ga.si(ga.si(ga.si(J.obj(), "ax", lanternward.fu(s, "x")), "ay", lanternward.fu(s, "y")),
            "bx", (lanternward.fu(s, "x") + ux * 260 : U32)), "by", (lanternward.fu(s, "y") + uy * 260 : U32)), "r", 1280))),
      "st", "Tell"),
    "t", lanternward.foes.Foe.tell(J.get(s, "$f"), J.get(s, "$r"), seats))

def lanternward.foes.step_turret.tell?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.sound(
        lanternward.push(
          lanternward.fs(lanternward.fsn(s, "st", "Strike"), "t", 6), "$shots",
          ga.su(
            ga.su(
              ga.si(
                ga.si(
                  ga.si(
                    ga.si(ga.sv(J.obj(), "kind", J.JS{"Iron"}), "x", (lanternward.fu(s, "x") + lanternward.fu(s, "fx") * 10 : U32)),
                    "y", (lanternward.fu(s, "y") + lanternward.fu(s, "fy") * 10 : U32)),
                  "vx", RS.i_div((lanternward.fu(s, "fx") * 620 : U32), 256)),
                "vy", RS.i_div((lanternward.fu(s, "fy") * 620 : U32), 256)),
              "ttl", 160),
            "owner", 0)),
        "shot_bolt")

def lanternward.foes.step_turret.strike?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.fs(lanternward.fsv(lanternward.fsn(s, "st", "Recover"), "danger", J.JS{"None"}), "t", 60)

def lanternward.foes.step_turret.recover?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.fs(lanternward.fsn(s, "st", "Dormant"), "t", 0)

def lanternward.foes.step_turret.stagger?(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.fsb(lanternward.fsn(s, "st", "Dormant"), "open", False{})

# The per-foe preamble and dispatch of `step_foes`: foe `i` rides in `$f`.

def lanternward.foes.step_foes.live?(+s: J.Val, +i: U32, +target: J.Val, +burning: Bool, +seats: U32) -> J.Val:
  +s1 = lanternward.fs(s, "flinch", RS.u_sat_sub(lanternward.fu(s, "flinch"), 1))
  +s2 = lanternward.fs(s1, "lit", Bool.pick(U32, burning, 60, RS.u_sat_sub(lanternward.fu(s1, "lit"), 1)))
  +s3 = lanternward.foes.step_foes.kb(U32.is_gt(lanternward.fu(s2, "kbt"), 0), s2)
  lanternward.foes.step_foes.kind(lanternward.fis(s3, "kind", "Snuffer"), s3, i, target, seats)

def lanternward.foes.step_foes.kb?(moving: Bool, +s: J.Val) -> J.Val:
  match moving:
    case False{}:
      s
    case True{}:
      lanternward.fs(
        lanternward.fsi(
          lanternward.fsi(s, "x", (lanternward.fu(s, "x") + ga.nu(lanternward.fv(s, "kb"), 0) : U32)),
          "y", (lanternward.fu(s, "y") + ga.nu(lanternward.fv(s, "kb"), 1) : U32)),
        "kbt", (lanternward.fu(s, "kbt") - 1 : U32))

def lanternward.foes.step_foes.kind?(snuffer: Bool, +s: J.Val, +i: U32, +target: J.Val, +seats: U32) -> J.Val:
  match snuffer:
    case True{}:
      lanternward.foes.step_snuffer(s, target, i)
    case False{}:
      lanternward.foes.step_turret(s, target, seats)

# Foe `i`'s turn: sleeping, frozen and dying foes skip the rest.

def lanternward.foes.step_foes.one?(+s: J.Val, +i: U32, +seats: U32) -> J.Val:
  +f = lanternward.el(s, "foes", i)
  +target = lanternward.foes.Session.nearest_knight(s, ga.u(f, "x"), ga.u(f, "y"))
  +burning = lanternward.props.Session.burning(s, ga.u(f, "x"), ga.u(f, "y"))
  lanternward.foes.step_foes.stash(ga.sv(s, "$f", f), i, target, burning, seats)

def lanternward.foes.step_foes.stash?(+s: J.Val, +i: U32, +target: J.Val, +burning: Bool, +seats: U32) -> J.Val:
  lanternward.foes.step_foes.put(
    lanternward.foes.step_foes.pre(lanternward.fis(s, "st", "Dying"), s, i, target, burning, seats), i)

def lanternward.foes.step_foes.put?(+s: J.Val, +i: U32) -> J.Val:
  J.del(lanternward.eput(s, "foes", i, J.get(s, "$f")), "$f")

def lanternward.foes.step_foes.pre?(dying: Bool, +s: J.Val, +i: U32, +target: J.Val, +burning: Bool, +seats: U32) -> J.Val:
  match dying:
    case True{}:
      lanternward.fs(s, "dead", RS.u_min((lanternward.fu(s, "dead") + 1 : U32), 255))
    case False{}:
      lanternward.foes.step_foes.frozen(U32.is_gt(lanternward.fu(s, "freeze"), 0), s, i, target, burning, seats)

def lanternward.foes.step_foes.frozen?(frozen: Bool, +s: J.Val, +i: U32, +target: J.Val, +burning: Bool, +seats: U32) -> J.Val:
  match frozen:
    case True{}:
      lanternward.fs(s, "freeze", (lanternward.fu(s, "freeze") - 1 : U32))
    case False{}:
      lanternward.foes.step_foes.live(s, i, target, burning, seats)

def lanternward.foes.step_foes.loop?(+s: J.Val, +i: U32, +seats: U32) -> J.Val:
  lanternward.foes.step_foes.loop.go(U32.is_lt(i, lanternward.len(s, "foes")), s, i, seats)

def lanternward.foes.step_foes.loop.go?(more: Bool, +s: J.Val, +i: U32, +seats: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.foes.step_foes.loop(lanternward.foes.step_foes.one(s, i, seats), (i + 1 : U32), seats)

# Keep a snuffer out of the walls it drifts into.

def lanternward.foes.step_foes.unstick?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.foes.step_foes.unstick.go(U32.is_lt(i, lanternward.len(s, "foes")), s, i)

def lanternward.foes.step_foes.unstick.go?(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.foes.step_foes.unstick(
        lanternward.foes.step_foes.unstick.one(
          Bool.and(lanternward.eis(s, "foes", i, "kind", "Snuffer"),
            Bool.not(lanternward.props.Session.clear(s, lanternward.eu(s, "foes", i, "x"), lanternward.eu(s, "foes", i, "y"), 1024))),
          s, i),
        (i + 1 : U32))

def lanternward.foes.step_foes.unstick.one?(stuck: Bool, +s: J.Val, +i: U32) -> J.Val:
  match stuck:
    case False{}:
      s
    case True{}:
      lanternward.foes.step_foes.unstick.set(
        lanternward.foes.Session.nudge(s, lanternward.eu(s, "foes", i, "x"), lanternward.eu(s, "foes", i, "y"), 1024), s, i)

def lanternward.foes.step_foes.unstick.set?(+p: J.Val, +s: J.Val, +i: U32) -> J.Val:
  lanternward.esi(lanternward.esi(s, "foes", i, "x", ga.nu(p, 0)), "foes", i, "y", ga.nu(p, 1))

# New shots join the bolts while there is room.

def lanternward.foes.step_foes.shots?(+s: J.Val, +xs: List<&2, J.Val>) -> J.Val:
  match xs:
    case Nil{}:
      s
    case Con{+b, +t}:
      lanternward.foes.step_foes.shots(
        lanternward.foes.step_foes.shots.one(U32.is_lt(lanternward.len(s, "bolts"), 40), s, b), t)

def lanternward.foes.step_foes.shots.one?(room: Bool, +s: J.Val, +b: J.Val) -> J.Val:
  match room:
    case True{}:
      lanternward.push(s, "bolts", b)
    case False{}:
      s

# Each foe's strike lands on every knight standing in its footprint.

def lanternward.foes.step_foes.strikes?(+s: J.Val, +xs: List<&2, J.Val>) -> J.Val:
  match xs:
    case Nil{}:
      s
    case Con{+st, +t}:
      lanternward.foes.step_foes.strikes(
        lanternward.foes.Session.strike_with(s, ga.nu(st, 0), J.nth(st, 1), ga.nu(st, 2)), t)

# One tick of every foe.

def lanternward.foes.Session.step_foes?(+s: J.Val) -> J.Val:
  lanternward.foes.step_foes.fin(
    lanternward.foes.step_foes.unstick(
      lanternward.foes.step_foes.loop(
        ga.sv(ga.sv(s, "$strikes", J.JA{Nil{}}), "$shots", J.JA{Nil{}}), 0, lanternward.seat_count(s)),
      0))

def lanternward.foes.step_foes.fin?(+s: J.Val) -> J.Val:
  lanternward.foes.step_foes.retain(
    J.del(J.del(
      lanternward.foes.step_foes.strikes(
        lanternward.foes.step_foes.shots(s, ga.items(s, "$shots")), ga.items(s, "$strikes")),
      "$shots"), "$strikes"))

# Those that have puffed out are gone.

def lanternward.foes.step_foes.retain.go?(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Bool.pick(List<&2, J.Val>,
        Bool.and(lanternward.is(f, "st", "Dying"), U32.is_ge(ga.u(f, "dead"), 14)),
        lanternward.foes.step_foes.retain.go(t),
        Con{f, lanternward.foes.step_foes.retain.go(t)})

def lanternward.foes.step_foes.retain?(+s: J.Val) -> J.Val:
  ga.sv(s, "foes", J.JA{lanternward.foes.step_foes.retain.go(ga.items(s, "foes"))})

# A foe's blow lands on every knight standing in its footprint that it has
# not yet struck.

def lanternward.foes.Session.strike_with.ids?(+ids: List<&2, U32>, +s: J.Val, +i: U32, +danger: J.Val, +dmg: U32) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.foes.Session.strike_with.ids(t, lanternward.foes.Session.strike_with.seat(s, id, i, danger, dmg), i, danger, dmg)

def lanternward.foes.Session.strike_with.seat?(+s: J.Val, +id: U32, +i: U32, +danger: J.Val, +dmg: U32) -> J.Val:
  +k = ga.at(J.get(s, "seats"), id)
  +bit = U32.shln(1, U32.to_nat(RS.u_min(id, 7)))
  lanternward.foes.Session.strike_with.hit(
    Bool.and(
      Bool.and(U32.is_eq(U32.and(lanternward.eu(s, "foes", i, "struck"), bit), 0), lanternward.kit.Seat.up(k)),
      lanternward.foes.Danger.hits(danger, ga.u(k, "x"), ga.u(k, "y"), 1280)),
    s, id, i, bit, dmg)

def lanternward.foes.Session.strike_with.hit?(hit: Bool, +s: J.Val, +id: U32, +i: U32, +bit: U32, +dmg: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.foes.Session.strike_with.after(
        lanternward.blow.Session.blow(
          lanternward.es(s, "foes", i, "struck", U32.or(lanternward.eu(s, "foes", i, "struck"), bit)),
          id, dmg, lanternward.eu(s, "foes", i, "x"), lanternward.eu(s, "foes", i, "y"), "Melee"),
        i)

def lanternward.foes.Session.strike_with.after?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.foes.Session.strike_with.res(ga.s(s, "$ret"), J.del(s, "$ret"), i)

def lanternward.foes.Session.strike_with.res?(+out: String, +s: J.Val, +i: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(out, "Parried"),
    lanternward.stop(
      lanternward.es(
        lanternward.esb(
          lanternward.eset(lanternward.es(lanternward.eset(s, "foes", i, "st", J.JS{"Stagger"}), "foes", i, "t", 30), "foes", i, "danger", J.JS{"None"}),
          "foes", i, "open", True{}),
        "foes", i, "freeze", 3),
      "Phase"),
    Bool.pick(J.Val, String.eq(out, "Blocked"),
      lanternward.es(
        lanternward.eset(s, "foes", i, "kb",
          ga.pair_i(
            RS.i_div(RS.i_neg(lanternward.eu(s, "foes", i, "fx")), 40),
            RS.i_div(RS.i_neg(lanternward.eu(s, "foes", i, "fy")), 40))),
        "foes", i, "kbt", 6),
      s))

def lanternward.foes.Session.strike_with?(+s: J.Val, +i: U32, +danger: J.Val, +dmg: U32) -> J.Val:
  lanternward.foes.Session.strike_with.ids(ga.ids(J.get(s, "seats")), s, i, danger, dmg)

def lanternward.sl.u?(+s: J.Val, +slot: String, +key: String) -> U32:
  ga.u(J.get(s, slot), key)

def lanternward.sl.b?(+s: J.Val, +slot: String, +key: String) -> Bool:
  ga.b(J.get(s, slot), key)

def lanternward.sl.v?(+s: J.Val, +slot: String, +key: String) -> J.Val:
  J.get(J.get(s, slot), key)

def lanternward.sl.set?(+s: J.Val, +slot: String, +key: String, v: J.Val) -> J.Val:
  J.set(s, slot, J.set(J.get(s, slot), key, v))

def lanternward.ku?(+s: J.Val, +key: String) -> U32:
  lanternward.sl.u(s, "$k", key)

def lanternward.kb?(+s: J.Val, +key: String) -> Bool:
  lanternward.sl.b(s, "$k", key)

def lanternward.kv?(+s: J.Val, +key: String) -> J.Val:
  lanternward.sl.v(s, "$k", key)

def lanternward.ks?(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$k", key, J.JU{v})

def lanternward.ksi?(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$k", key, J.of_i32(v))

def lanternward.ksb?(+s: J.Val, +key: String, +v: Bool) -> J.Val:
  lanternward.sl.set(s, "$k", key, J.JBool{v})

def lanternward.ksv?(+s: J.Val, +key: String, v: J.Val) -> J.Val:
  lanternward.sl.set(s, "$k", key, v)

def lanternward.fu?(+s: J.Val, +key: String) -> U32:
  lanternward.sl.u(s, "$f", key)

def lanternward.fb?(+s: J.Val, +key: String) -> Bool:
  lanternward.sl.b(s, "$f", key)

def lanternward.fv?(+s: J.Val, +key: String) -> J.Val:
  lanternward.sl.v(s, "$f", key)

def lanternward.fs?(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$f", key, J.JU{v})

def lanternward.fsi?(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$f", key, J.of_i32(v))

def lanternward.fsb?(+s: J.Val, +key: String, +v: Bool) -> J.Val:
  lanternward.sl.set(s, "$f", key, J.JBool{v})

def lanternward.fsv?(+s: J.Val, +key: String, v: J.Val) -> J.Val:
  lanternward.sl.set(s, "$f", key, v)

def lanternward.wu?(+s: J.Val, +key: String) -> U32:
  lanternward.sl.u(s, "$w", key)

def lanternward.wb?(+s: J.Val, +key: String) -> Bool:
  lanternward.sl.b(s, "$w", key)

def lanternward.wv?(+s: J.Val, +key: String) -> J.Val:
  lanternward.sl.v(s, "$w", key)

def lanternward.ws?(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$w", key, J.JU{v})

def lanternward.wsi?(+s: J.Val, +key: String, +v: U32) -> J.Val:
  lanternward.sl.set(s, "$w", key, J.of_i32(v))

def lanternward.wsb?(+s: J.Val, +key: String, +v: Bool) -> J.Val:
  lanternward.sl.set(s, "$w", key, J.JBool{v})

def lanternward.wsv?(+s: J.Val, +key: String, v: J.Val) -> J.Val:
  lanternward.sl.set(s, "$w", key, v)

# Element `i` of the session's array `vec` (props, foes, bolts, drops, fx,
# fuses): reads and writes of one field.

def lanternward.eu?(+s: J.Val, +vec: String, +i: U32, +key: String) -> U32:
  ga.u(J.nth(J.get(s, vec), i), key)

def lanternward.eb?(+s: J.Val, +vec: String, +i: U32, +key: String) -> Bool:
  ga.b(J.nth(J.get(s, vec), i), key)

def lanternward.ev?(+s: J.Val, +vec: String, +i: U32, +key: String) -> J.Val:
  J.get(J.nth(J.get(s, vec), i), key)

def lanternward.el?(+s: J.Val, +vec: String, +i: U32) -> J.Val:
  J.nth(J.get(s, vec), i)

def lanternward.eset?(+s: J.Val, +vec: String, +i: U32, +key: String, v: J.Val) -> J.Val:
  J.set(s, vec, J.set_nth(J.get(s, vec), i, J.set(J.nth(J.get(s, vec), i), key, v)))

def lanternward.es?(+s: J.Val, +vec: String, +i: U32, +key: String, +v: U32) -> J.Val:
  lanternward.eset(s, vec, i, key, J.JU{v})

def lanternward.esi?(+s: J.Val, +vec: String, +i: U32, +key: String, +v: U32) -> J.Val:
  lanternward.eset(s, vec, i, key, J.of_i32(v))

def lanternward.esb?(+s: J.Val, +vec: String, +i: U32, +key: String, +v: Bool) -> J.Val:
  lanternward.eset(s, vec, i, key, J.JBool{v})

def lanternward.eis?(+s: J.Val, +vec: String, +i: U32, +key: String, +name: String) -> Bool:
  String.eq(ga.s(J.nth(J.get(s, vec), i), key), name)

# A whole element replaced.

def lanternward.eput?(+s: J.Val, +vec: String, +i: U32, v: J.Val) -> J.Val:
  J.set(s, vec, J.set_nth(J.get(s, vec), i, v))

# Length of the session's array `vec`.

def lanternward.len?(+s: J.Val, +vec: String) -> U32:
  J.len(J.get(s, vec))

# `vec.push(v)`.

def lanternward.push?(+s: J.Val, +vec: String, v: J.Val) -> J.Val:
  J.set(s, vec, J.push(J.get(s, vec), v))

# The session's `retain` of a list by a list of keep flags would be written per
# site; `lanternward.drop_nth` removes item `i` of the array `vec`.

def lanternward.drop_nth?(+s: J.Val, +vec: String, +i: U32) -> J.Val:
  J.set(s, vec, J.JA{ga.remove_nth(J.items(J.get(s, vec)), U32.to_nat(i))})

# Enum values in serde shape: a unit variant is its name; a tuple / struct
# variant is {Name: payload}. `is` compares a field's unit-variant name.

def lanternward.is?(+o: J.Val, +key: String, +name: String) -> Bool:
  String.eq(ga.s(o, key), name)

def lanternward.kis?(+s: J.Val, +key: String, +name: String) -> Bool:
  String.eq(ga.s(J.get(s, "$k"), key), name)

def lanternward.fis?(+s: J.Val, +key: String, +name: String) -> Bool:
  String.eq(ga.s(J.get(s, "$f"), key), name)

def lanternward.wis?(+s: J.Val, +key: String, +name: String) -> Bool:
  String.eq(ga.s(J.get(s, "$w"), key), name)

def lanternward.ksn?(+s: J.Val, +key: String, +name: String) -> J.Val:
  lanternward.sl.set(s, "$k", key, J.JS{name})

def lanternward.fsn?(+s: J.Val, +key: String, +name: String) -> J.Val:
  lanternward.sl.set(s, "$f", key, J.JS{name})

def lanternward.wsn?(+s: J.Val, +key: String, +name: String) -> J.Val:
  lanternward.sl.set(s, "$w", key, J.JS{name})

# The variant name of an enum value (the string, or the only key of the
# object), and its payload (null for a unit variant).

def lanternward.tag.first?(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> String:
  match fs:
    case Nil{}:
      ""
    case Con{(+k, +v), +t}:
      k

def lanternward.tag?(+v: J.Val) -> String:
  match v:
    case J.JS{+n}:
      n
    case J.JO{+m}:
      lanternward.tag.first(J.fields(v))
    case _:
      ""

def lanternward.payload.first?(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{(+k, +v), +t}:
      v

def lanternward.payload?(+v: J.Val) -> J.Val:
  match v:
    case J.JO{+m}:
      lanternward.payload.first(J.fields(v))
    case _:
      J.JNull{}

def lanternward.tag_is?(+v: J.Val, +name: String) -> Bool:
  String.eq(lanternward.tag(v), name)

# `{Name: payload}`.

def lanternward.variant?(+name: String, p: J.Val) -> J.Val:
  J.set(J.obj(), name, p)

# The Rules, read from the session.

def lanternward.R?(+s: J.Val, +key: String) -> U32:
  ga.u(J.get(s, "$r"), key)

# Events: `sound` appends a name once; `stop` keeps the heaviest.

def lanternward.sounds.has?(+xs: List<&2, J.Val>, +name: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(String.eq(J.str(h), name), lanternward.sounds.has(t, name))

def lanternward.sound.add?(has: Bool, +s: J.Val, +name: String) -> J.Val:
  match has:
    case True{}:
      s
    case False{}:
      J.set(s, "$ev", J.set(J.get(s, "$ev"), "sounds", J.push(J.get(J.get(s, "$ev"), "sounds"), J.JS{name})))

def lanternward.sound?(+s: J.Val, +name: String) -> J.Val:
  lanternward.sound.add(lanternward.sounds.has(ga.items(J.get(s, "$ev"), "sounds"), name), s, name)

# Heavy < Kill < Phase < Boss.

def lanternward.stop.rank?(+n: String) -> U32:
  Bool.pick(U32, String.eq(n, "Heavy"), 1,
    Bool.pick(U32, String.eq(n, "Kill"), 2,
      Bool.pick(U32, String.eq(n, "Phase"), 3,
        Bool.pick(U32, String.eq(n, "Boss"), 4, 0))))

def lanternward.stop?(+s: J.Val, +name: String) -> J.Val:
  lanternward.stop.set(
    U32.is_gt(lanternward.stop.rank(name), lanternward.stop.rank(ga.s(J.get(s, "$ev"), "stop"))), s, name)

def lanternward.stop.set?(up: Bool, +s: J.Val, +name: String) -> J.Val:
  match up:
    case True{}:
      J.set(s, "$ev", J.set(J.get(s, "$ev"), "stop", J.JS{name}))
    case False{}:
      s

def lanternward.won?(+s: J.Val) -> J.Val:
  J.set(s, "$ev", J.set(J.get(s, "$ev"), "won", J.JBool{True{}}))

# A fresh Events.

def lanternward.Events.default?() -> J.Val:
  J.set(J.set(J.set(J.obj(), "sounds", J.JA{Nil{}}), "stop", J.JNull{}), "won", J.JBool{False{}})

# The session's own fields.

def lanternward.su?(+s: J.Val, +key: String, +v: U32) -> J.Val:
  ga.su(s, key, v)

# Signed order and bounds helpers on i32 held in a U32.

def lanternward.in_range?(+v: U32, +lo: U32, +hi: U32) -> Bool:
  Bool.and(RS.i_ge(v, lo), RS.i_le(v, hi))

# Trunc-divide by Q (i32 `/ Q`).

def lanternward.divq?(+v: U32) -> U32:
  RS.i_div(v, 256)

# `a.min(b)` / `.max(b)` on unsigned values and `a.saturating_sub(b)`.

def lanternward.umin?(+a: U32, +b: U32) -> U32:
  RS.u_min(a, b)

def lanternward.umax?(+a: U32, +b: U32) -> U32:
  RS.u_max(a, b)

def lanternward.usub?(+a: U32, +b: U32) -> U32:
  RS.u_sat_sub(a, b)

# `self.seats.len()`: the fields of the seats object.

def lanternward.seat_count?(+s: J.Val) -> U32:
  U32.from_nat(List.length(&2, Sigma<&2, &2, String, _ => J.Val>, J.fields(J.get(s, "seats"))))

# The seat's face component (0 = x, 1 = y).

def lanternward.kface?(+s: J.Val, +n: U32) -> U32:
  ga.nu(J.get(J.get(s, "$k"), "face"), n)

# `sound` when the condition holds.

def lanternward.sound_if?(yes: Bool, +s: J.Val, +name: String) -> J.Val:
  match yes:
    case True{}:
      lanternward.sound(s, name)
    case False{}:
      s

def lanternward.props.sign_text?(+room: U32, +id: U32) -> String:
  match room:
    case 0:
      "HOLD R TO LOCK ON"
    case 1:
      "FLARE SHOWS WHAT IS HIDDEN"
    case 2:
      "SHIELD UP AS IT FLIES"
    case 3:
      "LIGHT BURNS THE DARK OFF"
    case _:
      ""

def lanternward.props.folk_text?(+room: U32, +id: U32) -> String:
  match room:
    case 0:
      "THE KEEP DARKENS. FIND THE LANTERN."
    case _:
      ""

# What E's label says.

def lanternward.ctx.Ctx.label?(+tag: String) -> String:
  Bool.pick(String, String.eq(tag, "Open"), "OPEN",
    Bool.pick(String, String.eq(tag, "Read"), "READ",
      Bool.pick(String, String.eq(tag, "Lift"), "LIFT",
        Bool.pick(String, String.eq(tag, "Throw"), "THROW",
          Bool.pick(String, String.eq(tag, "Flame"), "TAKE FLAME",
            Bool.pick(String, String.eq(tag, "Light"), "LIGHT",
              Bool.pick(String, String.eq(tag, "Unlock"), "UNLOCK",
                Bool.pick(String, String.eq(tag, "Revive"), "REVIVE",
                  Bool.pick(String, String.eq(tag, "Relight"), "RELIGHT",
                    Bool.pick(String, String.eq(tag, "Talk"), "TALK",
                      Bool.pick(String, String.eq(tag, "Pull"), "PULL", "")))))))))))

# What a target that is not a foe says about itself when it is locked.

def lanternward.hint.lock_hint?(+s: J.Val, +k: J.Val) -> J.Val:
  lanternward.hint.lock_hint.go(lanternward.tag_is(ga.v(k, "lock"), "Prop"), J.u32(lanternward.payload(ga.v(k, "lock"))), s)

def lanternward.hint.lock_hint.go?(prop: Bool, +i: U32, +s: J.Val) -> J.Val:
  match prop:
    case False{}:
      J.JNull{}
    case True{}:
      lanternward.hint.lock_hint.of(U32.is_lt(i, lanternward.len(s, "props")), i, s)

def lanternward.hint.lock_hint.of?(has: Bool, +i: U32, +s: J.Val) -> J.Val:
  match has:
    case False{}:
      J.JNull{}
    case True{}:
      lanternward.hint.lock_hint.kind(lanternward.eu(s, "props", i, "id"), ga.s(lanternward.el(s, "props", i), "kind"), s)

def lanternward.hint.lock_hint.kind?(+id: U32, +kind: String, +s: J.Val) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Sign"), J.JS{lanternward.props.sign_text(ga.u(s, "room"), id)},
    Bool.pick(J.Val, String.eq(kind, "Folk"), J.JS{"A KINGSMAN: E TALKS"},
      Bool.pick(J.Val, String.eq(kind, "Crystal"), J.JS{"A CRYSTAL: LIGHT OR A PARRIED BOLT"},
        Bool.pick(J.Val, String.eq(kind, "Great"), J.JS{"THE GREAT BRAZIER: THROW LIGHT"},
          Bool.pick(J.Val, String.eq(kind, "Brazier"), J.JS{"A BRAZIER: LIGHT IT"},
            Bool.pick(J.Val, String.eq(kind, "Sigil"), J.JS{"A HIDDEN SIGIL: FLARE IT"},
              Bool.pick(J.Val, String.eq(kind, "Lever"), J.JS{"A LEVER: E PULLS"}, J.JNull{})))))))

def lanternward.hint.near_foe?(+fs: List<&2, J.Val>, +k: J.Val, +lock: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      Bool.or(
        Bool.and(lanternward.foes.Foe.alive(f),
          RS.i_lt(lanternward.world.dist((ga.u(f, "x") - ga.u(k, "x") : U32), (ga.u(f, "y") - ga.u(k, "y") : U32)), lock)),
        lanternward.hint.near_foe(t, k, lock))

# The wisp's one line. Few words, always about what to do next.

def lanternward.hint.hint?(+s: J.Val, +k: J.Val, +r: J.Val) -> J.Val:
  Bool.pick(J.Val, lanternward.is(k, "act", "Down"),
    J.JS{"WAIT FOR A FRIEND'S HAND"},
    lanternward.hint.hint.room(ga.u(s, "room"), ga.u(s, "stage"), s, k, lanternward.hint.near_foe(ga.items(s, "foes"), k, ga.u(r, "lock"))))

def lanternward.hint.hint.room?(+room: U32, +stage: U32, +s: J.Val, +k: J.Val, +near: Bool) -> J.Val:
  match room:
    case 0:
      lanternward.hint.hint.r0(stage, s, k, near)
    case 1:
      lanternward.hint.hint.r1(stage, k)
    case 2:
      lanternward.hint.hint.r2(stage, s, k)
    case 3:
      lanternward.hint.hint.r3(stage)
    case 4:
      lanternward.hint.hint.r4(stage, s)
    case _:
      J.JNull{}

def lanternward.hint.hint.r0?(+stage: U32, +s: J.Val, +k: J.Val, +near: Bool) -> J.Val:
  Bool.pick(J.Val, U32.is_ne(stage, 0), J.JS{"THE STAIR IS OPEN"},
    Bool.pick(J.Val, Bool.and(J.is_null(ga.v(k, "lock")), near), J.JS{"HOLD R TO LOCK ON"},
      Bool.pick(J.Val, Bool.and(Bool.and(Bool.not(J.is_null(ga.v(k, "lock"))), U32.is_eq(U32.and(ga.u(k, "held"), 4), 0)), U32.is_lt(ga.u(s, "elapsed"), 600)),
        J.JS{"LOCK ON, THEN HOP: E"},
        Bool.pick(J.Val, Bool.not(J.is_null(ga.v(k, "lock"))), J.JS{"Q SLASH  HOLD SPACE BLOCK"},
          Bool.pick(J.Val, U32.is_lt(ga.u(s, "elapsed"), 150), J.JS{"WALK TO THE SNUFFER"}, J.JS{"THE STAIR IS OPEN"})))))

def lanternward.hint.hint.r1?(+stage: U32, +k: J.Val) -> J.Val:
  match stage:
    case 0:
      Bool.pick(J.Val, U32.is_eq(ga.u(k, "taper"), 0), J.JS{"E AT THE EMBER BOWL"}, J.JS{"LIGHT ALL THREE AT ONCE"})
    case 1:
      J.JS{"OPEN THE CHEST"}
    case 2:
      J.JS{"HOLD F TO FLARE"}
    case 3:
      J.JS{"TAP F AT THE GREAT BRAZIER"}
    case _:
      J.JNull{}

def lanternward.hint.hint.r2?(+stage: U32, +s: J.Val, +k: J.Val) -> J.Val:
  match stage:
    case 0:
      Bool.pick(J.Val, lanternward.stage.any_on(ga.items(s, "props"), "Gate", 1), J.JS{"LOCK THE CRYSTAL, SHIELD UP"},
        Bool.pick(J.Val,
          Bool.and(U32.is_ge(lanternward.seat_count(s), 2), lanternward.stage.any_on(ga.items(s, "props"), "Plate", 0)),
          J.JS{"A FRIEND STANDS ON THE SECOND PLATE"}, J.JS{"PUSH THE BLOCK ONTO THE PLATE"}))
    case _:
      Bool.pick(J.Val, Bool.and(ga.b(ga.v(s, "inv"), "lantern"), J.is_null(ga.v(k, "lock"))),
        J.JS{"LOCK THE CRYSTAL, SHIELD UP"}, J.JS{"SPACE SHIELD TURNS THE BOLT"})

def lanternward.hint.hint.r3?(+stage: U32) -> J.Val:
  match stage:
    case 0:
      J.JS{"HOLD F: BURN THE HOOD OFF"}
    case 1:
      J.JS{"TAKE THE MOON KEY"}
    case 2:
      J.JS{"E AT THE LOCKED DOOR"}
    case 3:
      J.JS{"OPEN THE BOSS KEY CHEST"}
    case _:
      J.JS{"CARRY THE KEY TO THE DOOR"}

def lanternward.hint.hint.r4?(+stage: U32, +s: J.Val) -> J.Val:
  match stage:
    case 0:
      J.JS{"THROW LIGHT AT ITS HEART"}
    case 1:
      lanternward.hint.hint.warden(J.get(s, "boss"), lanternward.seat_count(s))
    case 2:
      J.JS{"TAKE THE HEART"}
    case 3:
      J.JS{"HOLD E AT THE BEACON"}
    case _:
      J.JNull{}

def lanternward.hint.hint.warden?(+w: J.Val, +seats: U32) -> J.Val:
  match w:
    case J.JNull{}:
      J.JS{"LIGHT THE HEART, THEN STRIKE"}
    case _:
      Bool.pick(J.Val, Bool.and(U32.is_ge(ga.u(w, "phase"), 1), U32.is_ge(seats, 2)), J.JS{"TWO LIGHTS AT ONCE"},
        Bool.pick(J.Val, Bool.and(lanternward.is(w, "st", "Tell"), lanternward.is(w, "atk", "Slam")), J.JS{"HOP THE RING"},
          Bool.pick(J.Val, Bool.and(lanternward.is(w, "st", "Tell"), lanternward.is(w, "atk", "Beam")), J.JS{"SHIELD UP: PARRY THE BEAM"},
            J.JS{"LIGHT THE HEART, THEN STRIKE"})))

# The line at the foot of seat `k`'s screen: a sign or kingsman being read,
# else what a locked switch says, else the wisp.

def lanternward.hint.line?(+s: J.Val, +k: J.Val, +r: J.Val) -> J.Val:
  lanternward.hint.line.read(ga.v(k, "read"), s, k, r)

def lanternward.hint.line.read?(+rd: J.Val, +s: J.Val, +k: J.Val, +r: J.Val) -> J.Val:
  match rd:
    case J.JNull{}:
      lanternward.hint.line.lock(lanternward.hint.lock_hint(s, k), s, k, r)
    case _:
      Bool.pick(J.Val, U32.is_ge(ga.nu(rd, 0), 100),
        J.JS{lanternward.props.folk_text(ga.u(s, "room"), (ga.nu(rd, 0) - 100 : U32))},
        J.JS{lanternward.props.sign_text(ga.u(s, "room"), ga.nu(rd, 0))})

def lanternward.hint.line.lock?(+h: J.Val, +s: J.Val, +k: J.Val, +r: J.Val) -> J.Val:
  match h:
    case J.JNull{}:
      lanternward.hint.hint(s, k, r)
    case _:
      h

# What E would do: its label, or the roll or hop it falls back to.

def lanternward.hint.context?(+k: J.Val) -> J.Val:
  lanternward.hint.context.tag(lanternward.tag(ga.v(k, "ctx")), k)

def lanternward.hint.context.tag?(+tag: String, +k: J.Val) -> J.Val:
  Bool.pick(J.Val, String.eq(tag, "None"),
    Bool.pick(J.Val, J.is_null(ga.v(k, "lock")), J.JS{"E ROLL"}, J.JS{"E HOP"}),
    J.JS{"E " ++ lanternward.ctx.Ctx.label(tag)})

def case.lanternward.hint.line?(+input: J.Val) -> J.Val:
  lanternward.hint.line(J.get(input, "s"), J.get(input, "k"), J.get(input, "r"))

def case.lanternward.hint.hint?(+input: J.Val) -> J.Val:
  lanternward.hint.hint(J.get(input, "s"), J.get(input, "k"), J.get(input, "r"))

def case.lanternward.hint.lock_hint?(+input: J.Val) -> J.Val:
  lanternward.hint.lock_hint(J.get(input, "s"), J.get(input, "k"))

def case.lanternward.hint.context?(+input: J.Val) -> J.Val:
  lanternward.hint.context(J.get(input, "k"))

def lanternward.i64.of_i32?(+v: U32) -> W.W:
  W.W{Bool.pick(U32, RS.i_is_neg(v), 4294967295, 0), v}

def lanternward.i64.is_neg?(+a: W.W) -> Bool:
  U32.is_ge(W.hi32(a), 2147483648)

def lanternward.i64.neg?(+a: W.W) -> W.W:
  W.neg(a)

def lanternward.i64.abs?(+a: W.W) -> W.W:
  Bool.pick(W.W, lanternward.i64.is_neg(a), W.neg(a), a)

def lanternward.i64.add?(+a: W.W, +b: W.W) -> W.W:
  W.add(a, b)

def lanternward.i64.sub?(+a: W.W, +b: W.W) -> W.W:
  W.sub(a, b)

# Two's complement multiplication is the unsigned wrapping product.

def lanternward.i64.mul?(+a: W.W, +b: W.W) -> W.W:
  W.mul(a, b)

# Truncating division (the quotient's sign is the product of the signs).

def lanternward.i64.div?(+a: W.W, +b: W.W) -> W.W:
  +q = W.div(lanternward.i64.abs(a), lanternward.i64.abs(b))
  Bool.pick(W.W, Bool.xor(lanternward.i64.is_neg(a), lanternward.i64.is_neg(b)), W.neg(q), q)

def lanternward.i64.lt?(+a: W.W, +b: W.W) -> Bool:
  W.lt(W.xor(a, W.W{2147483648, 0}), W.xor(b, W.W{2147483648, 0}))

def lanternward.i64.le?(+a: W.W, +b: W.W) -> Bool:
  W.le(W.xor(a, W.W{2147483648, 0}), W.xor(b, W.W{2147483648, 0}))

def lanternward.i64.clamp?(+x: W.W, +lo: W.W, +hi: W.W) -> W.W:
  Bool.pick(W.W, lanternward.i64.lt(x, lo), lo, Bool.pick(W.W, lanternward.i64.lt(hi, x), hi, x))

# `x as i32`: the low 32 bits.

def lanternward.i64.to_i32?(+a: W.W) -> U32:
  W.lo32(a)

def lanternward.kit.buttons?(+i: J.Val) -> U32:
  (Bool.to_u32(ga.b(i, "ult")) * 1 + Bool.to_u32(ga.b(i, "dash")) * 2 + Bool.to_u32(ga.b(i, "swing")) * 4
    + Bool.to_u32(ga.b(i, "interact")) * 8 + Bool.to_u32(ga.b(i, "fire")) * 16 + Bool.to_u32(ga.b(i, "bomb")) * 32 : U32)

def lanternward.kit.Seat.new?(+id: U32, +joined: U32, +x: U32, +y: U32) -> J.Val:
  +k0 = ga.su(ga.su(ga.su(J.obj(), "id", U32.and(id, 255)), "joined", joined), "z", 0)
  +k1 = ga.si(ga.si(k0, "x", x), "y", y)
  +k2 = ga.si(ga.sv(k1, "face", ga.pair_i(0, 256)), "z", 0)
  +k3 = ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(k2, "hp", 20), "max_hp", 20), "gems", 0), "bombs", 0), "flame", 0), "taper", 0)
  +k4 = ga.sv(ga.su(ga.su(k3, "t", 0), "n", 0), "act", J.JS{"Free"})
  +k5 = ga.sv(ga.sv(k4, "mv", ga.pair_i(0, 0)), "push", ga.pair_i(0, 0))
  +k6 = ga.sb(ga.su(ga.su(ga.su(ga.su(ga.su(k5, "pusht", 0), "step", 0), "chain", 0), "charge", 0), "shield", 0), "queued", False{})
  +k7 = ga.su(ga.su(ga.su(ga.su(ga.sv(ga.su(k6, "guard_cd", 0), "lock", J.JNull{}), "lock_t", 0), "unlock_t", 0), "relock", 0), "inv", 60)
  +k8 = ga.su(ga.su(ga.su(ga.su(ga.su(k7, "ward", 0), "freeze", 0), "swing", 0), "lamp", 0), "lamp_cd", 0)
  +k9 = ga.sv(ga.sv(ga.sv(k8, "carry", J.JS{"Nothing"}), "ctx", J.JS{"None"}), "read", J.JNull{})
  +k10 = ga.su(ga.sv(k9, "aim", ga.pair_i(0, 0)), "held", 0)
  ga.sv(ga.su(ga.su(ga.sv(ga.sv(k10, "safe", ga.pair_i(x, y)), "cam", ga.pair_i(x, y)), "rev", 0), "beep", 0), "item", J.JNull{})

# Whether every number is one a save or the wire may carry.

def lanternward.kit.Seat.valid?(+k: J.Val, +id: U32, +elapsed: U32, +w: U32, +h: U32) -> Bool:
  Bool.and(
    Bool.and(lanternward.kit.Seat.valid.a(k, id, elapsed, w, h), lanternward.kit.Seat.valid.b(k)),
    Bool.and(lanternward.kit.Seat.valid.c(k, w, h), lanternward.kit.Seat.valid.d(k)))

def lanternward.kit.Seat.valid.a?(+k: J.Val, +id: U32, +elapsed: U32, +w: U32, +h: U32) -> Bool:
  Bool.and(
    Bool.and(U32.is_eq(ga.u(k, "id"), id), U32.is_le(ga.u(k, "joined"), elapsed)),
    Bool.and(lanternward.in_range(ga.u(k, "x"), 0, w), lanternward.in_range(ga.u(k, "y"), 0, h)))

def lanternward.kit.Seat.valid.b?(+k: J.Val) -> Bool:
  +face = ga.v(k, "face")
  Bool.and(
    Bool.and(lanternward.in_range(ga.u(k, "z"), 0, 4096), RS.i_le(lanternward.world.dist(ga.nu(face, 0), ga.nu(face, 1)), 264)),
    Bool.and(U32.is_le(ga.u(k, "max_hp"), 48), U32.is_le(ga.u(k, "hp"), ga.u(k, "max_hp"))))

def lanternward.kit.Seat.valid.c?(+k: J.Val, +w: U32, +h: U32) -> Bool:
  +mv = ga.v(k, "mv")
  +push = ga.v(k, "push")
  +cam = ga.v(k, "cam")
  +safe = ga.v(k, "safe")
  Bool.and(
    Bool.and(
      Bool.and(lanternward.in_range(ga.nu(mv, 0), 4294965248, 2048), lanternward.in_range(ga.nu(mv, 1), 4294965248, 2048)),
      Bool.and(lanternward.in_range(ga.nu(push, 0), 4294965248, 2048), lanternward.in_range(ga.nu(push, 1), 4294965248, 2048))),
    Bool.and(
      Bool.and(
        Bool.and(lanternward.in_range(ga.nu(cam, 0), 0, w), lanternward.in_range(ga.nu(cam, 1), 0, h)),
        Bool.and(lanternward.in_range(ga.nu(safe, 0), 0, w), lanternward.in_range(ga.nu(safe, 1), 0, h))),
      lanternward.kit.Seat.valid.read(ga.v(k, "read"))))

def lanternward.kit.Seat.valid.read?(+r: J.Val) -> Bool:
  Bool.or(J.is_null(r), U32.is_le(ga.nu(r, 1), 120))

def lanternward.kit.Seat.valid.d?(+k: J.Val) -> Bool:
  Bool.and(
    Bool.and(
      Bool.and(
        Bool.and(U32.is_le(ga.u(k, "gems"), 100000), U32.is_le(ga.u(k, "bombs"), 10)),
        Bool.and(U32.is_le(ga.u(k, "flame"), 300), U32.is_le(ga.u(k, "taper"), 420))),
      Bool.and(
        Bool.and(U32.is_le(ga.u(k, "t"), 4096), U32.is_le(ga.u(k, "n"), 3)),
        Bool.and(U32.is_le(ga.u(k, "step"), 2), U32.is_le(ga.u(k, "chain"), 64)))),
    Bool.and(
      Bool.and(
        Bool.and(U32.is_le(ga.u(k, "charge"), 255), U32.is_le(ga.u(k, "shield"), 255)),
        Bool.and(U32.is_le(ga.u(k, "guard_cd"), 64), U32.is_le(ga.u(k, "lock_t"), 255))),
      Bool.and(
        Bool.and(U32.is_le(ga.u(k, "unlock_t"), 64), U32.is_le(ga.u(k, "relock"), 16)),
        Bool.and(
          Bool.and(U32.is_le(ga.u(k, "inv"), 128), U32.is_le(ga.u(k, "ward"), 36)),
          Bool.and(
            Bool.and(U32.is_le(ga.u(k, "freeze"), 16), U32.is_le(ga.u(k, "lamp"), 255)),
            Bool.and(
              Bool.and(U32.is_le(ga.u(k, "lamp_cd"), 64), U32.is_le(ga.u(k, "rev"), 128)),
              U32.is_le(ga.u(k, "pusht"), 16)))))))

def lanternward.kit.Seat.charged?(+k: J.Val) -> Bool:
  U32.is_ge(ga.u(k, "charge"), 22)

def lanternward.kit.Seat.airborne?(+k: J.Val) -> Bool:
  RS.i_gt(ga.u(k, "z"), 0)

def lanternward.kit.Session.lantern_owned?(+s: J.Val) -> Bool:
  ga.b(ga.v(s, "inv"), "lantern")

# ---- lock-on targets ----

# Where a lock holds: null, or [x, y].

def lanternward.kit.Session.lock_point?(+s: J.Val, +l: J.Val) -> J.Val:
  lanternward.kit.lock_point.tag(lanternward.tag(l), J.u32(lanternward.payload(l)), s)

def lanternward.kit.lock_point.tag?(+tag: String, +n: U32, +s: J.Val) -> J.Val:
  Bool.pick(J.Val, String.eq(tag, "Foe"), lanternward.kit.lock_point.foe(ga.items(s, "foes"), n),
    Bool.pick(J.Val, String.eq(tag, "Boss"), lanternward.kit.lock_point.boss(J.get(s, "boss")),
      Bool.pick(J.Val, String.eq(tag, "Prop"), lanternward.kit.lock_point.prop(lanternward.el(s, "props", n)),
        J.JNull{})))

def lanternward.kit.lock_point.foe?(+fs: List<&2, J.Val>, +id: U32) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      lanternward.kit.lock_point.foe.hit(
        Bool.and(U32.is_eq(ga.u(f, "id"), id), lanternward.foes.Foe.alive(f)), f, t, id)

def lanternward.kit.lock_point.foe.hit?(hit: Bool, +f: J.Val, +t: List<&2, J.Val>, +id: U32) -> J.Val:
  match hit:
    case True{}:
      ga.pair_i(ga.u(f, "x"), ga.u(f, "y"))
    case False{}:
      lanternward.kit.lock_point.foe(t, id)

def lanternward.kit.lock_point.boss?(+w: J.Val) -> J.Val:
  match w:
    case J.JNull{}:
      J.JNull{}
    case _:
      lanternward.kit.lock_point.boss.alive(U32.is_gt(ga.u(w, "hp"), 0), w)

def lanternward.kit.lock_point.boss.alive?(alive: Bool, +w: J.Val) -> J.Val:
  match alive:
    case True{}:
      ga.pair_i(lanternward.boss.Warden.heart.x(w), lanternward.boss.Warden.heart.y(w))
    case False{}:
      J.JNull{}

def lanternward.kit.lock_point.prop?(+p: J.Val) -> J.Val:
  match p:
    case J.JNull{}:
      J.JNull{}
    case _:
      lanternward.kit.lock_point.prop.target(lanternward.props.Prop.target(p), p)

def lanternward.kit.lock_point.prop.target?(t: Bool, +p: J.Val) -> J.Val:
  match t:
    case True{}:
      ga.pair_i(ga.u(p, "x"), ga.u(p, "y"))
    case False{}:
      J.JNull{}

# Everything a lock may hold: [lock, x, y, hostile] in order: foes, the
# Warden's heart, then the props that can be targeted.

def lanternward.kit.targets.foes?(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Bool.pick(List<&2, J.Val>, lanternward.foes.Foe.alive(f),
        Con{J.JA{[lanternward.variant("Foe", J.JU{ga.u(f, "id")}), J.of_i32(ga.u(f, "x")), J.of_i32(ga.u(f, "y")), J.JBool{True{}}]},
          lanternward.kit.targets.foes(t)},
        lanternward.kit.targets.foes(t))

def lanternward.kit.targets.props?(+ps: List<&2, J.Val>, +i: U32) -> List<&2, J.Val>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      Bool.pick(List<&2, J.Val>, lanternward.props.Prop.target(p),
        Con{J.JA{[lanternward.variant("Prop", J.JU{U32.and(i, 255)}), J.of_i32(ga.u(p, "x")), J.of_i32(ga.u(p, "y")), J.JBool{False{}}]},
          lanternward.kit.targets.props(t, (i + 1 : U32))},
        lanternward.kit.targets.props(t, (i + 1 : U32)))

def lanternward.kit.targets.boss?(+w: J.Val) -> List<&2, J.Val>:
  match w:
    case J.JNull{}:
      Nil{}
    case _:
      Bool.pick(List<&2, J.Val>, U32.is_gt(ga.u(w, "hp"), 0),
        [J.JA{[J.JS{"Boss"}, J.of_i32(lanternward.boss.Warden.heart.x(w)), J.of_i32(lanternward.boss.Warden.heart.y(w)), J.JBool{True{}}]}],
        Nil{})

def lanternward.kit.Session.targets?(+s: J.Val) -> List<&2, J.Val>:
  List.append(&2, J.Val, lanternward.kit.targets.foes(ga.items(s, "foes")),
    List.append(&2, J.Val, lanternward.kit.targets.boss(J.get(s, "boss")), lanternward.kit.targets.props(ga.items(s, "props"), 0)))

# Whether the knight's lock is of something still there and in reach.

def lanternward.kit.Session.lock_holds?(+s: J.Val, +k: J.Val, +l: J.Val) -> Bool:
  lanternward.kit.lock_holds.pt(lanternward.kit.Session.lock_point(s, l), k, lanternward.R(s, "lock"))

def lanternward.kit.lock_holds.pt?(+p: J.Val, +k: J.Val, +lock: U32) -> Bool:
  match p:
    case J.JNull{}:
      False{}
    case _:
      RS.i_le(lanternward.world.dist((ga.nu(p, 0) - ga.u(k, "x") : U32), (ga.nu(p, 1) - ga.u(k, "y") : U32)), RS.i_div((lock * 3 : U32), 2))

# The nearest thing in front of the knight, a foe before a switch; with
# nothing in front, the nearest at all. The key is (not hostile, not front, d).

def lanternward.kit.pick_target.go?(+ts: List<&2, J.Val>, +s: J.Val, +k: J.Val, +skip: J.Val, +bk: J.Val, +bl: J.Val) -> J.Val:
  match ts:
    case Nil{}:
      bl
    case Con{+t, +rest}:
      lanternward.kit.pick_target.cand(
        Bool.not(J.eq(J.nth(t, 0), skip)),
        t, rest, s, k, skip, bk, bl)

def lanternward.kit.pick_target.cand?(ok: Bool, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +skip: J.Val, +bk: J.Val, +bl: J.Val) -> J.Val:
  match ok:
    case False{}:
      lanternward.kit.pick_target.go(rest, s, k, skip, bk, bl)
    case True{}:
      lanternward.kit.pick_target.dist(
        lanternward.world.dist((ga.nu(t, 1) - ga.u(k, "x") : U32), (ga.nu(t, 2) - ga.u(k, "y") : U32)),
        t, rest, s, k, skip, bk, bl)

def lanternward.kit.pick_target.dist?(+d: U32, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +skip: J.Val, +bk: J.Val, +bl: J.Val) -> J.Val:
  lanternward.kit.pick_target.reach(
    Bool.and(RS.i_le(d, lanternward.R(s, "lock")),
      lanternward.props.Session.line_open(s, ga.u(k, "x"), ga.u(k, "y"), ga.nu(t, 1), ga.nu(t, 2))),
    d, t, rest, s, k, skip, bk, bl)

def lanternward.kit.pick_target.reach?(ok: Bool, +d: U32, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +skip: J.Val, +bk: J.Val, +bl: J.Val) -> J.Val:
  match ok:
    case False{}:
      lanternward.kit.pick_target.go(rest, s, k, skip, bk, bl)
    case True{}:
      lanternward.kit.pick_target.key(
        Bool.to_u32(Bool.not(J.bool(J.nth(t, 3)))),
        Bool.to_u32(RS.i_lt(lanternward.world.dot_unit(ga.nu(ga.v(k, "face"), 0), ga.nu(ga.v(k, "face"), 1), (ga.nu(t, 1) - ga.u(k, "x") : U32), (ga.nu(t, 2) - ga.u(k, "y") : U32)), 0)),
        d, t, rest, s, k, skip, bk, bl)

# Lexicographic key (a, b, d) < best.

def lanternward.kit.pick_target.key?(+a: U32, +b: U32, +d: U32, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +skip: J.Val, +bk: J.Val, +bl: J.Val) -> J.Val:
  +kk = {J.JA{[J.JU{a}, J.JU{b}, J.of_i32(d)]} : J.Val}
  +better = Bool.or(J.is_null(bl), lanternward.kit.key_lt(a, b, d, bk))
  lanternward.kit.pick_target.go(rest, s, k, skip, Bool.pick(J.Val, better, kk, bk), Bool.pick(J.Val, better, J.nth(t, 0), bl))

def lanternward.kit.key_lt?(+a: U32, +b: U32, +d: U32, +bk: J.Val) -> Bool:
  Bool.or(
    U32.is_lt(a, ga.nu(bk, 0)),
    Bool.and(U32.is_eq(a, ga.nu(bk, 0)),
      Bool.or(U32.is_lt(b, ga.nu(bk, 1)), Bool.and(U32.is_eq(b, ga.nu(bk, 1)), RS.i_lt(d, ga.nu(bk, 2))))))

def lanternward.kit.Session.pick_target?(+s: J.Val, +k: J.Val, +skip: J.Val) -> J.Val:
  lanternward.kit.pick_target.go(lanternward.kit.Session.targets(s), s, k, skip, J.JNull{}, J.JNull{})

# The next target in the direction the knight pressed, as seen from the
# current one.

def lanternward.kit.cycle.go?(+ts: List<&2, J.Val>, +s: J.Val, +k: J.Val, +cx: U32, +cy: U32, +wx: U32, +wy: U32, +bd: U32, +bl: J.Val) -> J.Val:
  match ts:
    case Nil{}:
      bl
    case Con{+t, +rest}:
      lanternward.kit.cycle.cand(
        Bool.and(
          Bool.not(J.eq(J.nth(t, 0), ga.v(k, "lock"))),
          Bool.and(
            RS.i_le(lanternward.world.dist((ga.nu(t, 1) - ga.u(k, "x") : U32), (ga.nu(t, 2) - ga.u(k, "y") : U32)), lanternward.R(s, "lock")),
            lanternward.props.Session.line_open(s, ga.u(k, "x"), ga.u(k, "y"), ga.nu(t, 1), ga.nu(t, 2)))),
        t, rest, s, k, cx, cy, wx, wy, bd, bl)

def lanternward.kit.cycle.cand?(ok: Bool, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +cx: U32, +cy: U32, +wx: U32, +wy: U32, +bd: U32, +bl: J.Val) -> J.Val:
  match ok:
    case False{}:
      lanternward.kit.cycle.go(rest, s, k, cx, cy, wx, wy, bd, bl)
    case True{}:
      lanternward.kit.cycle.take(
        lanternward.world.dist((ga.nu(t, 1) - cx : U32), (ga.nu(t, 2) - cy : U32)),
        t, rest, s, k, cx, cy, wx, wy, bd, bl)

def lanternward.kit.cycle.take?(+d: U32, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +cx: U32, +cy: U32, +wx: U32, +wy: U32, +bd: U32, +bl: J.Val) -> J.Val:
  +good = Bool.and(
    Bool.and(RS.i_gt(d, 0), RS.i_gt(lanternward.kit.cycle.dot((ga.nu(t, 1) - cx : U32), (ga.nu(t, 2) - cy : U32), wx, wy), 128)),
    Bool.or(J.is_null(bl), RS.i_lt(d, bd)))
  lanternward.kit.cycle.go(rest, s, k, cx, cy, wx, wy, Bool.pick(U32, good, d, bd), Bool.pick(J.Val, good, J.nth(t, 0), bl))

def lanternward.kit.cycle.dot?(+vx: U32, +vy: U32, +wx: U32, +wy: U32) -> U32:
  lanternward.world.dot_unit(wx, wy, vx, vy)

def lanternward.kit.Session.cycle_target?(+s: J.Val, +k: J.Val, +dx: U32, +dy: U32) -> J.Val:
  lanternward.kit.cycle_target.cur(lanternward.kit.Session.lock_point(s, ga.v(k, "lock")), s, k, dx, dy)

def lanternward.kit.cycle_target.cur?(+cur: J.Val, +s: J.Val, +k: J.Val, +dx: U32, +dy: U32) -> J.Val:
  match cur:
    case J.JNull{}:
      J.JNull{}
    case _:
      lanternward.kit.cycle_target.want(lanternward.world.unit(dx, dy), cur, s, k)

def lanternward.kit.cycle_target.want?(want: U32 & U32, +cur: J.Val, +s: J.Val, +k: J.Val) -> J.Val:
  (+wx, +wy) = want
  lanternward.kit.cycle.go(lanternward.kit.Session.targets(s), s, k, ga.nu(cur, 0), ga.nu(cur, 1), wx, wy, 0, J.JNull{})

# ---- one tick of one knight ----

def lanternward.kit.act_code?(+a: String) -> U32:
  Bool.pick(U32, String.eq(a, "Free"), 0,
    Bool.pick(U32, String.eq(a, "Slash"), 1,
      Bool.pick(U32, String.eq(a, "Lunge"), 2,
        Bool.pick(U32, String.eq(a, "Spin"), 3,
          Bool.pick(U32, String.eq(a, "Roll"), 4,
            Bool.pick(U32, String.eq(a, "Hop"), 5,
              Bool.pick(U32, String.eq(a, "Plunge"), 6,
                Bool.pick(U32, String.eq(a, "Hurt"), 7,
                  Bool.pick(U32, String.eq(a, "Item"), 8,
                    Bool.pick(U32, String.eq(a, "Down"), 9, 10))))))))))

# Put the knight in `$k` back at the table.

def lanternward.kit.put_k?(+s: J.Val, +id: U32) -> J.Val:
  J.del(ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id, J.get(s, "$k"))), "$k")

# One tick of one knight.

def lanternward.kit.Session.step_seat?(+s: J.Val, +id: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.step_seat.has(ga.has_key(J.get(s, "seats"), id), s, id, inp)

def lanternward.kit.step_seat.has?(has: Bool, +s: J.Val, +id: U32, +inp: J.Val) -> J.Val:
  match has:
    case False{}:
      s
    case True{}:
      lanternward.kit.step_seat.take(
        ga.sv(ga.sv(s, "$k", ga.at(J.get(s, "seats"), id)), "seats", J.del(J.get(s, "seats"), U32.show(id))),
        id, inp, lanternward.kit.buttons(inp))

def lanternward.kit.step_seat.take?(+s: J.Val, +id: U32, +inp: J.Val, +mask: U32) -> J.Val:
  lanternward.kit.step_seat.pre(
    lanternward.ks(s, "held", mask), id, inp, mask, U32.and(mask, U32.not(lanternward.ku(s, "held"))))

def lanternward.kit.step_seat.pre?(+s: J.Val, +id: U32, +inp: J.Val, +mask: U32, +press: U32) -> J.Val:
  +s1 = lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(s,
    "inv", RS.u_sat_sub(lanternward.ku(s, "inv"), 1)),
    "ward", RS.u_sat_sub(lanternward.ku(s, "ward"), 1)),
    "taper", RS.u_sat_sub(lanternward.ku(s, "taper"), 1)),
    "guard_cd", RS.u_sat_sub(lanternward.ku(s, "guard_cd"), 1)),
    "lamp_cd", RS.u_sat_sub(lanternward.ku(s, "lamp_cd"), 1)),
    "chain", RS.u_sat_sub(lanternward.ku(s, "chain"), 1))
  +s2 = lanternward.kit.step_seat.unchain(Bool.and(U32.is_eq(lanternward.ku(s1, "chain"), 0), Bool.not(lanternward.kb(s1, "queued"))), s1)
  +s3 = lanternward.ks(lanternward.ks(s2, "unlock_t", RS.u_sat_sub(lanternward.ku(s2, "unlock_t"), 1)), "lock_t", RS.u_min((lanternward.ku(s2, "lock_t") + 1 : U32), 255))
  lanternward.kit.step_seat.freeze(
    lanternward.kit.Session.refill(lanternward.kit.step_seat.read(J.is_null(lanternward.kv(s3, "read")), s3)),
    id, inp, mask, press)

def lanternward.kit.step_seat.unchain?(yes: Bool, +s: J.Val) -> J.Val:
  match yes:
    case True{}:
      lanternward.ks(s, "step", 0)
    case False{}:
      s

# A sign being read counts down and is put away at zero.

def lanternward.kit.step_seat.read?(none: Bool, +s: J.Val) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.kit.step_seat.read.go(RS.u_sat_sub(ga.nu(lanternward.kv(s, "read"), 1), 1), s)

def lanternward.kit.step_seat.read.go?(+t: U32, +s: J.Val) -> J.Val:
  lanternward.ksv(s, "read",
    Bool.pick(J.Val, U32.is_eq(t, 0), J.JNull{}, ga.pair(J.nth(lanternward.kv(s, "read"), 0), J.JU{t})))

# A blade that landed holds its owner a moment.

def lanternward.kit.step_seat.freeze?(+s: J.Val, +id: U32, +inp: J.Val, +mask: U32, +press: U32) -> J.Val:
  lanternward.kit.step_seat.freeze.go(U32.is_gt(lanternward.ku(s, "freeze"), 0), s, id, inp, mask, press)

def lanternward.kit.step_seat.freeze.go?(held: Bool, +s: J.Val, +id: U32, +inp: J.Val, +mask: U32, +press: U32) -> J.Val:
  match held:
    case True{}:
      lanternward.kit.put_k(lanternward.kit.Session.camera(lanternward.ks(s, "freeze", (lanternward.ku(s, "freeze") - 1 : U32))), id)
    case False{}:
      lanternward.kit.step_seat.act(lanternward.kit.act_code(ga.s(J.get(s, "$k"), "act")), s, id, inp, mask, press)

def lanternward.kit.step_seat.act?(+code: U32, +s: J.Val, +id: U32, +inp: J.Val, +mask: U32, +press: U32) -> J.Val:
  lanternward.kit.step_seat.fin(
    lanternward.kit.step_seat.act.go(code, s, id, inp, mask, press), id)

def lanternward.kit.step_seat.act.go?(+code: U32, +s: J.Val, +id: U32, +inp: J.Val, +mask: U32, +press: U32) -> J.Val:
  match code:
    case 9:
      lanternward.ksv(lanternward.ks(lanternward.ks(s, "t", RS.u_min((lanternward.ku(s, "t") + 1 : U32), 4000)), "shield", 0), "lock", J.JNull{})
    case 8:
      lanternward.kit.step_seat.item(U32.is_ge((lanternward.ku(s, "t") + 1 : U32), 60), lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32)))
    case 10:
      lanternward.kit.Session.falling(s)
    case 7:
      lanternward.kit.Session.hurting(s)
    case _:
      lanternward.kit.Session.verbs(
        lanternward.kit.Session.face_and_move(lanternward.kit.Session.lock_on(s, id, mask, press, inp), inp),
        id, mask, press, inp)

# Holding an item overhead, then putting it down.

def lanternward.kit.step_seat.item?(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.ks(lanternward.ksv(lanternward.ksn(s, "act", "Free"), "item", J.JNull{}), "t", 0)

# Things that go on whatever the knight is doing, then the camera, then back
# to the table.

def lanternward.kit.step_seat.fin?(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.put_k(lanternward.kit.Session.camera(lanternward.kit.Session.upkeep(s)), id)

# The lantern's flame comes back: slowly anywhere, quickly at a resting
# brazier or beside a friend's flare.

def lanternward.kit.refill.rest?(+ps: List<&2, J.Val>, +kx: U32, +ky: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+p, +t}:
      Bool.or(
        Bool.and(String.eq(ga.s(p, "kind"), "Rest"),
          RS.i_lt(lanternward.world.dist((ga.u(p, "x") - kx : U32), (ga.u(p, "y") - ky : U32)), 16384)),
        lanternward.kit.refill.rest(t, kx, ky))

def lanternward.kit.refill.kindled?(+ids: List<&2, U32>, +seats: J.Val, +kid: U32, +kx: U32, +ky: U32) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case Con{+id, +t}:
      Bool.or(
        Bool.and(
          Bool.and(U32.is_ne(ga.u(ga.at(seats, id), "id"), kid), lanternward.kit.Seat.flaring(ga.at(seats, id))),
          RS.i_lt(lanternward.world.dist((ga.u(ga.at(seats, id), "x") - kx : U32), (ga.u(ga.at(seats, id), "y") - ky : U32)), 7168)),
        lanternward.kit.refill.kindled(t, seats, kid, kx, ky))

def lanternward.kit.Session.refill?(+s: J.Val) -> J.Val:
  lanternward.kit.refill.owned(lanternward.kit.Session.lantern_owned(s), s)

def lanternward.kit.refill.owned?(owned: Bool, +s: J.Val) -> J.Val:
  match owned:
    case False{}:
      s
    case True{}:
      lanternward.kit.refill.cap(
        lanternward.ks(s, "flame", RS.u_min(lanternward.ku(s, "flame"), lanternward.R(s, "flame"))))

def lanternward.kit.refill.cap?(+s: J.Val) -> J.Val:
  lanternward.kit.refill.flaring(lanternward.kit.Seat.flaring(J.get(s, "$k")), s)

def lanternward.kit.refill.flaring?(flaring: Bool, +s: J.Val) -> J.Val:
  match flaring:
    case True{}:
      s
    case False{}:
      lanternward.ks(
        s, "flame",
        RS.u_min(
          (lanternward.ku(s, "flame") + Bool.pick(U32, lanternward.kit.refill.rest(ga.items(s, "props"), lanternward.ku(s, "x"), lanternward.ku(s, "y")), 3,
            Bool.pick(U32, lanternward.kit.refill.kindled(ga.ids(J.get(s, "seats")), J.get(s, "seats"), lanternward.ku(s, "id"), lanternward.ku(s, "x"), lanternward.ku(s, "y")), 2,
              Bool.pick(U32, U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0), 1, 0))) : U32),
          lanternward.R(s, "flame")))

def lanternward.kit.Session.upkeep?(+s: J.Val) -> J.Val:
  lanternward.kit.upkeep.ctx(lanternward.kit.upkeep.beep(lanternward.kit.upkeep.safe(s)))

# The last spot the knight stood on solid ground not beside a ledge.

def lanternward.kit.upkeep.safe?(+s: J.Val) -> J.Val:
  lanternward.kit.upkeep.safe.go(
    Bool.and(
      Bool.and(lanternward.kis(s, "act", "Free"), U32.is_eq(lanternward.ku(s, "z"), 0)),
      Bool.not(lanternward.props.Session.pit(s, lanternward.ku(s, "x"), lanternward.ku(s, "y")))),
    s)

def lanternward.kit.upkeep.safe.go?(ok: Bool, +s: J.Val) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.kit.upkeep.safe.set(
        Bool.and(
          Bool.not(lanternward.kit.upkeep.ledge(ga.u(s, "room"), lanternward.world.tile_of(lanternward.ku(s, "x")), lanternward.world.tile_of(lanternward.ku(s, "y")))),
          lanternward.props.Session.clear(s, lanternward.ku(s, "x"), lanternward.ku(s, "y"), 1280)),
        s)

def lanternward.kit.upkeep.safe.set?(ok: Bool, +s: J.Val) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.ksv(s, "safe", ga.pair_i(lanternward.ku(s, "x"), lanternward.ku(s, "y")))

def lanternward.kit.upkeep.ledge?(+room: U32, +tx: U32, +ty: U32) -> Bool:
  Bool.or(
    Bool.or(
      Bool.or(lanternward.kit.upkeep.pit_at(room, (tx - 1 : U32), (ty - 1 : U32)), lanternward.kit.upkeep.pit_at(room, tx, (ty - 1 : U32))),
      Bool.or(lanternward.kit.upkeep.pit_at(room, (tx + 1 : U32), (ty - 1 : U32)), lanternward.kit.upkeep.pit_at(room, (tx - 1 : U32), ty))),
    Bool.or(
      Bool.or(lanternward.kit.upkeep.pit_at(room, tx, ty), lanternward.kit.upkeep.pit_at(room, (tx + 1 : U32), ty)),
      Bool.or(
        Bool.or(lanternward.kit.upkeep.pit_at(room, (tx - 1 : U32), (ty + 1 : U32)), lanternward.kit.upkeep.pit_at(room, tx, (ty + 1 : U32))),
        lanternward.kit.upkeep.pit_at(room, (tx + 1 : U32), (ty + 1 : U32)))))

def lanternward.kit.upkeep.pit_at?(+room: U32, +tx: U32, +ty: U32) -> Bool:
  U32.is_eq(lanternward.world.Plan.at(room, tx, ty), 61)

# The heart beep: a low knight chirps now and then.

def lanternward.kit.upkeep.beep?(+s: J.Val) -> J.Val:
  lanternward.kit.upkeep.beep.go(
    Bool.and(lanternward.kit.Seat.up(J.get(s, "$k")), U32.is_le(lanternward.ku(s, "hp"), 8)), s)

def lanternward.kit.upkeep.beep.go?(low: Bool, +s: J.Val) -> J.Val:
  match low:
    case False{}:
      lanternward.ks(s, "beep", 0)
    case True{}:
      lanternward.kit.upkeep.beep.chirp(
        U32.mod((lanternward.ku(s, "beep") + 1 : U32), 160), s)

def lanternward.kit.upkeep.beep.chirp?(+b: U32, +s: J.Val) -> J.Val:
  lanternward.kit.upkeep.beep.sound(U32.is_eq(U32.mod(b, 40), 1), lanternward.ks(s, "beep", b))

def lanternward.kit.upkeep.beep.sound?(yes: Bool, +s: J.Val) -> J.Val:
  match yes:
    case True{}:
      lanternward.sound(s, "mana_empty")
    case False{}:
      s

def lanternward.kit.upkeep.ctx?(+s: J.Val) -> J.Val:
  lanternward.kit.upkeep.ctx.go(lanternward.kis(s, "act", "Free"), s)

def lanternward.kit.upkeep.ctx.go?(free: Bool, +s: J.Val) -> J.Val:
  match free:
    case True{}:
      lanternward.ksv(s, "ctx", lanternward.ctx.Session.context(s, J.get(s, "$k")))
    case False{}:
      lanternward.ksv(s, "ctx", J.JS{"None"})

# ---- lock-on ----

def lanternward.kit.Session.lock_on?(+s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.lock_on.go(
    Bool.and(U32.is_ne(U32.and(mask, 1), 0), lanternward.kit.Seat.up(J.get(s, "$k"))), s, press, inp)

def lanternward.kit.lock_on.go?(holding: Bool, +s: J.Val, +press: U32, +inp: J.Val) -> J.Val:
  match holding:
    case False{}:
      lanternward.ksv(
        lanternward.ks(
          lanternward.kit.lock_on.drop(Bool.not(J.is_null(lanternward.kv(s, "lock"))), s), "relock", 0),
        "aim", ga.pair(J.get(inp, "aim_x"), J.get(inp, "aim_y")))
    case True{}:
      lanternward.kit.lock_on.keep(
        Bool.and(Bool.not(J.is_null(lanternward.kv(s, "lock"))),
          Bool.not(lanternward.kit.Session.lock_holds(s, J.get(s, "$k"), lanternward.kv(s, "lock")))),
        s, press, inp)

def lanternward.kit.lock_on.drop?(had: Bool, +s: J.Val) -> J.Val:
  lanternward.ksv(
    Bool.pick(J.Val, had, lanternward.ks(s, "unlock_t", 10), s), "lock", J.JNull{})

# A lock that no longer holds lets go and waits a few ticks to re-lock.

def lanternward.kit.lock_on.keep?(lost: Bool, +s: J.Val, +press: U32, +inp: J.Val) -> J.Val:
  match lost:
    case True{}:
      lanternward.kit.lock_on.fresh(lanternward.kit.lock_on.lost(s), press, inp)
    case False{}:
      lanternward.kit.lock_on.fresh(s, press, inp)

def lanternward.kit.lock_on.lost?(+s: J.Val) -> J.Val:
  lanternward.ks(lanternward.ksv(s, "lock", J.JNull{}), "relock", 6)

# A fresh press of an aim key while locked cycles to the next target that way.

def lanternward.kit.lock_on.fresh?(+s: J.Val, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.lock_on.cycle(
    Bool.and(
      Bool.or(U32.is_ne(ga.u(inp, "aim_x"), 0), U32.is_ne(ga.u(inp, "aim_y"), 0)),
      Bool.not(Bool.and(U32.is_eq(ga.u(inp, "aim_x"), ga.nu(lanternward.kv(s, "aim"), 0)), U32.is_eq(ga.u(inp, "aim_y"), ga.nu(lanternward.kv(s, "aim"), 1))))),
    ga.pair(J.get(inp, "aim_x"), J.get(inp, "aim_y")),
    s, press, inp)

def lanternward.kit.lock_on.cycle?(+fresh: Bool, +aim: J.Val, +s: J.Val, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.lock_on.branch(
    Bool.and(Bool.not(J.is_null(lanternward.kv(s, "lock"))), fresh),
    J.is_null(lanternward.kv(s, "lock")),
    ga.sv(s, "$k", J.set(J.get(s, "$k"), "aim", aim)), press, inp)

def lanternward.kit.lock_on.branch?(+cycling: Bool, +unlocked: Bool, +s: J.Val, +press: U32, +inp: J.Val) -> J.Val:
  Bool.pick(J.Val, cycling,
    lanternward.kit.lock_on.cycled(
      lanternward.kit.Session.cycle_target(s, J.get(s, "$k"), ga.u(inp, "aim_x"), ga.u(inp, "aim_y")), s),
    Bool.pick(J.Val, unlocked,
      lanternward.kit.lock_on.seek(Bool.or(U32.is_ne(U32.and(press, 1), 0), U32.is_eq(lanternward.ku(s, "relock"), 0)), s, press),
      s))

def lanternward.kit.lock_on.cycled?(+l: J.Val, +s: J.Val) -> J.Val:
  match l:
    case J.JNull{}:
      s
    case _:
      lanternward.kit.Session.take_lock(s, l)

def lanternward.kit.lock_on.seek?(look: Bool, +s: J.Val, +press: U32) -> J.Val:
  match look:
    case False{}:
      lanternward.ks(s, "relock", (lanternward.ku(s, "relock") - 1 : U32))
    case True{}:
      lanternward.kit.lock_on.found(
        lanternward.kit.Session.pick_target(s, J.get(s, "$k"), J.JNull{}), s, U32.is_ne(U32.and(press, 1), 0))

def lanternward.kit.lock_on.found?(+l: J.Val, +s: J.Val, +pressed: Bool) -> J.Val:
  match l:
    case J.JNull{}:
      Bool.pick(J.Val, pressed, lanternward.ks(s, "unlock_t", 10), s)
    case _:
      lanternward.kit.Session.take_lock(s, l)

def lanternward.kit.Session.take_lock?(+s: J.Val, +l: J.Val) -> J.Val:
  lanternward.kit.take_lock.at(
    lanternward.kit.Session.lock_point(s, l),
    lanternward.ks(lanternward.ksv(s, "lock", l), "lock_t", 0))

def lanternward.kit.take_lock.at?(+pt: J.Val, +s: J.Val) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.sound(s, "card_pickup")
    case _:
      lanternward.kit.take_lock.face(
        lanternward.world.unit((ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), (ga.nu(pt, 1) - lanternward.ku(s, "y") : U32)),
        lanternward.props.Session.fx(s, "Tick", ga.nu(pt, 0), ga.nu(pt, 1), 0))

def lanternward.kit.take_lock.face?(f: U32 & U32, +s: J.Val) -> J.Val:
  (+fx, +fy) = f
  lanternward.sound(
    Bool.pick(J.Val, Bool.and(U32.is_eq(fx, 0), U32.is_eq(fy, 0)), s, lanternward.ksv(s, "face", ga.pair_i(fx, fy))),
    "card_pickup")

# ---- facing and walking ----

# Face a locked target, else the aim keys, else the way walked.

def lanternward.kit.Session.face_and_move?(+s: J.Val, +inp: J.Val) -> J.Val:
  lanternward.kit.face_and_move.go(
    lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")), s, inp)

def lanternward.kit.face_and_move.go?(+pt: J.Val, +s: J.Val, +inp: J.Val) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.kit.face_and_move.aim(
        Bool.or(U32.is_ne(ga.u(inp, "aim_x"), 0), U32.is_ne(ga.u(inp, "aim_y"), 0)), s, inp)
    case _:
      lanternward.kit.face_and_move.lock(
        lanternward.world.unit((ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), (ga.nu(pt, 1) - lanternward.ku(s, "y") : U32)), s, inp)

def lanternward.kit.face_and_move.lock?(f: U32 & U32, +s: J.Val, +inp: J.Val) -> J.Val:
  (+fx, +fy) = f
  lanternward.kit.face_and_move.move(
    Bool.pick(J.Val, Bool.and(U32.is_eq(fx, 0), U32.is_eq(fy, 0)), s, lanternward.ksv(s, "face", ga.pair_i(fx, fy))), inp)

def lanternward.kit.face_and_move.aim?(has: Bool, +s: J.Val, +inp: J.Val) -> J.Val:
  match has:
    case True{}:
      lanternward.kit.face_and_move.face(lanternward.world.unit(ga.u(inp, "aim_x"), ga.u(inp, "aim_y")), s, inp)
    case False{}:
      lanternward.kit.face_and_move.walk(
        Bool.and(
          Bool.and(
            Bool.or(U32.is_ne(ga.u(inp, "move_x"), 0), U32.is_ne(ga.u(inp, "move_y"), 0)),
            U32.is_eq(lanternward.ku(s, "shield"), 0)),
          lanternward.kis(s, "act", "Free")),
        s, inp)

def lanternward.kit.face_and_move.walk?(go: Bool, +s: J.Val, +inp: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.kit.face_and_move.face(lanternward.world.unit(ga.u(inp, "move_x"), ga.u(inp, "move_y")), s, inp)
    case False{}:
      lanternward.kit.face_and_move.move(s, inp)

def lanternward.kit.face_and_move.face?(f: U32 & U32, +s: J.Val, +inp: J.Val) -> J.Val:
  (+fx, +fy) = f
  lanternward.kit.face_and_move.move(lanternward.ksv(s, "face", ga.pair_i(fx, fy)), inp)

def lanternward.kit.face_and_move.move?(+s: J.Val, +inp: J.Val) -> J.Val:
  lanternward.kit.face_and_move.push(lanternward.kis(s, "act", "Free"), s, inp)

# Only a free knight is carried by a push and walks.

def lanternward.kit.face_and_move.push?(free: Bool, +s: J.Val, +inp: J.Val) -> J.Val:
  match free:
    case False{}:
      s
    case True{}:
      lanternward.kit.face_and_move.walking(
        lanternward.kit.Session.shove_push(U32.is_gt(lanternward.ku(s, "pusht"), 0), s), inp)

def lanternward.kit.Session.shove_push?(yes: Bool, +s: J.Val) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.kit.Session.shove(
        lanternward.ks(s, "pusht", (lanternward.ku(s, "pusht") - 1 : U32)),
        ga.nu(lanternward.kv(s, "push"), 0), ga.nu(lanternward.kv(s, "push"), 1))

def lanternward.kit.face_and_move.walking?(+s: J.Val, +inp: J.Val) -> J.Val:
  lanternward.kit.face_and_move.speed(
    Bool.and(U32.is_eq(ga.u(inp, "move_x"), 0), U32.is_eq(ga.u(inp, "move_y"), 0)), s, inp)

def lanternward.kit.face_and_move.speed?(still: Bool, +s: J.Val, +inp: J.Val) -> J.Val:
  match still:
    case True{}:
      s
    case False{}:
      lanternward.kit.face_and_move.step(
        Bool.pick(U32, U32.is_gt(lanternward.ku(s, "shield"), 0), 230,
          Bool.pick(U32, Bool.or(lanternward.kis(s, "carry", "Pot"), U32.is_gt(lanternward.ku(s, "charge"), 12)), 260,
            Bool.pick(U32, Bool.not(J.is_null(lanternward.kv(s, "lock"))), 340, 440))),
        lanternward.world.unit(ga.u(inp, "move_x"), ga.u(inp, "move_y")), s)

def lanternward.kit.face_and_move.step?(+speed: U32, v: U32 & U32, +s: J.Val) -> J.Val:
  (+vx, +vy) = v
  lanternward.kit.face_and_move.go2(
    RS.i_div((vx * speed : U32), 256), RS.i_div((vy * speed : U32), 256), vx, vy, s,
    lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")))

def lanternward.kit.face_and_move.go2?(+wx: U32, +wy: U32, +vx: U32, +vy: U32, +s: J.Val, +pt: J.Val) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.kit.Session.shove(s, wx, wy)
    case _:
      lanternward.kit.Session.orbit(s, ga.nu(pt, 0), ga.nu(pt, 1), wx, wy)

# Walk about a locked target: the sideways part of the step goes round it at
# the same distance, the rest closes or opens it.

def lanternward.kit.Session.orbit?(+s: J.Val, +tx: U32, +ty: U32, +vx: U32, +vy: U32) -> J.Val:
  lanternward.kit.orbit.go(
    (lanternward.ku(s, "x") - tx : U32), (lanternward.ku(s, "y") - ty : U32), s, tx, ty, vx, vy)

def lanternward.kit.orbit.go?(+rx: U32, +ry: U32, +s: J.Val, +tx: U32, +ty: U32, +vx: U32, +vy: U32) -> J.Val:
  lanternward.kit.orbit.len(lanternward.world.dist(rx, ry), rx, ry, s, tx, ty, vx, vy)

def lanternward.kit.orbit.len?(+len: U32, +rx: U32, +ry: U32, +s: J.Val, +tx: U32, +ty: U32, +vx: U32, +vy: U32) -> J.Val:
  lanternward.kit.orbit.len.go(RS.i_lt(len, 256), len, rx, ry, s, tx, ty, vx, vy)

def lanternward.kit.orbit.len.go?(near: Bool, +len: U32, +rx: U32, +ry: U32, +s: J.Val, +tx: U32, +ty: U32, +vx: U32, +vy: U32) -> J.Val:
  match near:
    case True{}:
      lanternward.kit.Session.shove(s, vx, vy)
    case False{}:
      lanternward.kit.orbit.rhat(lanternward.world.unit(rx, ry), len, s, tx, ty, vx, vy)

def lanternward.kit.orbit.rhat?(rhat: U32 & U32, +len: U32, +s: J.Val, +tx: U32, +ty: U32, +vx: U32, +vy: U32) -> J.Val:
  (+hx, +hy) = rhat
  lanternward.kit.orbit.split(
    lanternward.world.dot4(vx, vy, hx, hy), lanternward.world.dot4(vx, vy, RS.i_neg(hy), hx),
    RS.i_neg(hy), hx, hx, hy, lanternward.world.dist(vx, vy), len, s, tx, ty)

def lanternward.kit.orbit.split?(+radial: U32, +tangent: U32, +thx: U32, +thy: U32, +hx: U32, +hy: U32, +speed: U32, +len: U32, +s: J.Val, +tx: U32, +ty: U32) -> J.Val:
  +side = RS.i_ge((RS.i_abs(tangent) * 10 : U32), (speed * 8 : U32))
  +fore = RS.i_ge((RS.i_abs(radial) * 10 : U32), (speed * 8 : U32))
  lanternward.kit.orbit.want(
    Bool.pick(U32, side, 0, radial), Bool.pick(U32, side, tangent, Bool.pick(U32, fore, 0, tangent)),
    thx, thy, hx, hy, len, s, tx, ty)

def lanternward.kit.orbit.want?(+radial: U32, +tangent: U32, +thx: U32, +thy: U32, +hx: U32, +hy: U32, +len: U32, +s: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.kit.orbit.off(
    RS.i_max(RS.i_add(len, radial), 3072),
    RS.i_add(lanternward.world.mul_div(hx, len, 256), RS.i_div((tangent * thx : U32), 256)),
    RS.i_add(lanternward.world.mul_div(hy, len, 256), RS.i_div((tangent * thy : U32), 256)),
    s, tx, ty)

def lanternward.kit.orbit.off?(+want: U32, +ox: U32, +oy: U32, +s: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.kit.orbit.to(lanternward.world.unit(ox, oy), want, s, tx, ty)

def lanternward.kit.orbit.to?(u: U32 & U32, +want: U32, +s: J.Val, +tx: U32, +ty: U32) -> J.Val:
  (+ux, +uy) = u
  lanternward.kit.Session.shove(
    s,
    (tx + lanternward.world.mul_div(ux, want, 256) - lanternward.ku(s, "x") : U32),
    (ty + lanternward.world.mul_div(uy, want, 256) - lanternward.ku(s, "y") : U32))

# Move a body by (dx, dy), a whole axis at a time so it slides along a wall;
# leans on a block it meets; falls into a pit it walks into.

def lanternward.kit.Session.shove?(+s: J.Val, +dx: U32, +dy: U32) -> J.Val:
  lanternward.kit.shove.fall(
    Bool.not(Bool.or(lanternward.kit.Seat.airborne(J.get(s, "$k")), lanternward.NAME_IN(ga.s(J.get(s, "$k"), "act"), ["Roll", "Hop", "Plunge"]))),
    lanternward.kit.shove.y(lanternward.kit.shove.x(s, dx), dy))

def lanternward.kit.shove.x?(+s: J.Val, +dx: U32) -> J.Val:
  lanternward.kit.shove.x.go(U32.is_ne(dx, 0), s, dx)

def lanternward.kit.shove.x.go?(moving: Bool, +s: J.Val, +dx: U32) -> J.Val:
  match moving:
    case False{}:
      s
    case True{}:
      lanternward.kit.shove.x.try(
        lanternward.props.Session.clear(s, (lanternward.ku(s, "x") + dx : U32), lanternward.ku(s, "y"), 1280), s, dx)

def lanternward.kit.shove.x.try?(ok: Bool, +s: J.Val, +dx: U32) -> J.Val:
  match ok:
    case True{}:
      lanternward.ksi(s, "x", (lanternward.ku(s, "x") + dx : U32))
    case False{}:
      lanternward.kit.Session.lean(s, RS.i_signum(dx), 0)

def lanternward.kit.shove.y?(+s: J.Val, +dy: U32) -> J.Val:
  lanternward.kit.shove.y.go(U32.is_ne(dy, 0), s, dy)

def lanternward.kit.shove.y.go?(moving: Bool, +s: J.Val, +dy: U32) -> J.Val:
  match moving:
    case False{}:
      s
    case True{}:
      lanternward.kit.shove.y.try(
        lanternward.props.Session.clear(s, lanternward.ku(s, "x"), (lanternward.ku(s, "y") + dy : U32), 1280), s, dy)

def lanternward.kit.shove.y.try?(ok: Bool, +s: J.Val, +dy: U32) -> J.Val:
  match ok:
    case True{}:
      lanternward.ksi(s, "y", (lanternward.ku(s, "y") + dy : U32))
    case False{}:
      lanternward.kit.Session.lean(s, 0, RS.i_signum(dy))

# `airborne` was decided before the move (Rust reads it first).

def lanternward.kit.shove.fall?(+ground: Bool, +s: J.Val) -> J.Val:
  lanternward.kit.shove.fall.go(
    Bool.and(Bool.and(ground, lanternward.props.Session.pit(s, lanternward.ku(s, "x"), lanternward.ku(s, "y"))), lanternward.kis(s, "act", "Free")),
    s)

def lanternward.kit.shove.fall.go?(drop: Bool, +s: J.Val) -> J.Val:
  match drop:
    case False{}:
      s
    case True{}:
      lanternward.ks(lanternward.ks(lanternward.ksv(lanternward.ks(lanternward.ksn(s, "act", "Fall"), "t", 0), "lock", J.JNull{}), "lamp", 0), "shield", 0)

# A body pressing on a block: after a moment of leaning, it slides.

def lanternward.kit.Session.lean?(+s: J.Val, +sx: U32, +sy: U32) -> J.Val:
  lanternward.kit.lean.find(
    lanternward.kit.lean.block(ga.items(s, "props"), 0,
      (lanternward.ku(s, "x") + sx * 2048 : U32), (lanternward.ku(s, "y") + sy * 2048 : U32)),
    s, sx, sy)

def lanternward.kit.lean.block?(+ps: List<&2, J.Val>, +i: U32, +x: U32, +y: U32) -> J.Val:
  match ps:
    case Nil{}:
      J.JNull{}
    case Con{+p, +t}:
      lanternward.kit.lean.block.hit(
        Bool.and(String.eq(ga.s(p, "kind"), "Block"), Bool.and(U32.is_eq(ga.u(p, "on"), 0), lanternward.props.Prop.blocks(p, x, y))),
        t, i, x, y)

def lanternward.kit.lean.block.hit?(hit: Bool, +t: List<&2, J.Val>, +i: U32, +x: U32, +y: U32) -> J.Val:
  match hit:
    case True{}:
      J.JU{i}
    case False{}:
      lanternward.kit.lean.block(t, (i + 1 : U32), x, y)

def lanternward.kit.lean.find?(+b: J.Val, +s: J.Val, +sx: U32, +sy: U32) -> J.Val:
  match b:
    case J.JNull{}:
      s
    case _:
      lanternward.kit.lean.push(J.u32(b), lanternward.es(s, "props", J.u32(b), "t", RS.u_min((lanternward.eu(s, "props", J.u32(b), "t") + 2 : U32), 65535)), sx, sy)

def lanternward.kit.lean.push?(+b: U32, +s: J.Val, +sx: U32, +sy: U32) -> J.Val:
  lanternward.kit.lean.slide(U32.is_ge(lanternward.eu(s, "props", b, "t"), 10), b, s, sx, sy)

def lanternward.kit.lean.slide?(go: Bool, +b: U32, +s: J.Val, +sx: U32, +sy: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.ctx.Session.start_slide(
        lanternward.es(lanternward.es(s, "props", b, "t", 0), "props", b, "on",
          Bool.pick(U32, U32.is_eq(sx, 1), 1, Bool.pick(U32, U32.is_eq(sx, 4294967295), 2, Bool.pick(U32, U32.is_eq(sy, 1), 3, 4)))),
        b)

def lanternward.kit.Session.falling?(+s: J.Val) -> J.Val:
  lanternward.kit.falling.go(U32.is_ge((lanternward.ku(s, "t") + 1 : U32), 18), lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32)))

def lanternward.kit.falling.go?(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.ks(
        lanternward.ks(
          lanternward.ks(
            lanternward.ksn(
              lanternward.ksi(lanternward.ksi(lanternward.ks(s, "hp", RS.u_max(RS.u_sat_sub(lanternward.ku(s, "hp"), 2), 1)),
                "x", ga.nu(lanternward.kv(s, "safe"), 0)), "y", ga.nu(lanternward.kv(s, "safe"), 1)),
              "act", "Free"),
            "t", 0),
          "inv", 36),
        "t", 0)

def lanternward.kit.Session.hurting?(+s: J.Val) -> J.Val:
  lanternward.kit.hurting.go(
    lanternward.kit.Session.shove_push(U32.is_gt(lanternward.ku(s, "pusht"), 0), lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32))))

def lanternward.kit.hurting.go?(+s: J.Val) -> J.Val:
  lanternward.kit.hurting.end(U32.is_ge(lanternward.ku(s, "t"), 10), s)

def lanternward.kit.hurting.end?(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0)

# ---- the verbs ----

def lanternward.kit.Session.verbs?(+s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.verbs.charge(lanternward.kit.verbs.shield(s, mask), id, mask, press, inp)

# The shield: held on the ground with a free arm.

def lanternward.kit.verbs.shield?(+s: J.Val, +mask: U32) -> J.Val:
  lanternward.kit.verbs.shield.go(
    Bool.and(
      Bool.and(U32.is_ne(U32.and(mask, 2), 0), lanternward.kis(s, "act", "Free")),
      Bool.and(
        Bool.and(lanternward.kis(s, "carry", "Nothing"), U32.is_eq(lanternward.ku(s, "z"), 0)),
        U32.is_eq(lanternward.ku(s, "guard_cd"), 0))),
    s)

def lanternward.kit.verbs.shield.go?(up: Bool, +s: J.Val) -> J.Val:
  match up:
    case True{}:
      lanternward.ks(
        lanternward.kit.verbs.shield.sound(U32.is_eq(lanternward.ku(s, "shield"), 0), s),
        "shield", RS.u_min((lanternward.ku(s, "shield") + 1 : U32), 255))
    case False{}:
      lanternward.ks(
        Bool.pick(J.Val, Bool.and(U32.is_gt(lanternward.ku(s, "shield"), 0), lanternward.kis(s, "act", "Free")),
          lanternward.ks(s, "guard_cd", 10), s),
        "shield", 0)

def lanternward.kit.verbs.shield.sound?(first: Bool, +s: J.Val) -> J.Val:
  match first:
    case True{}:
      lanternward.sound(s, "shield_up")
    case False{}:
      s

# The sword button held builds the spin.

def lanternward.kit.verbs.charge?(+s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.verbs.recover(
    lanternward.kit.verbs.charge.go(Bool.and(U32.is_ne(U32.and(mask, 4), 0), U32.is_eq(lanternward.ku(s, "shield"), 0)), s),
    id, mask, press, inp)

def lanternward.kit.verbs.charge.go?(on: Bool, +s: J.Val) -> J.Val:
  match on:
    case False{}:
      s
    case True{}:
      lanternward.kit.verbs.charge.mark(
        U32.is_eq(RS.u_min((lanternward.ku(s, "charge") + 1 : U32), 255), 22),
        lanternward.ks(s, "charge", RS.u_min((lanternward.ku(s, "charge") + 1 : U32), 255)))

def lanternward.kit.verbs.charge.mark?(full: Bool, +s: J.Val) -> J.Val:
  match full:
    case False{}:
      s
    case True{}:
      lanternward.props.Session.fx(lanternward.sound(s, "spell_cast"), "Spark", lanternward.ku(s, "x"), (lanternward.ku(s, "y") - 2048 : U32), 1)

# A swing's recovery gives way to a dodge or the shield.

def lanternward.kit.Session.recovering?(+k: J.Val) -> Bool:
  Bool.pick(Bool, String.eq(ga.s(k, "act"), "Slash"),
    U32.is_gt(ga.u(k, "t"), (lanternward.kit.slash_wind(RS.u_min(ga.u(k, "n"), 2)) + lanternward.kit.slash_active(RS.u_min(ga.u(k, "n"), 2)) : U32)),
    Bool.pick(Bool, String.eq(ga.s(k, "act"), "Lunge"), U32.is_ge(ga.u(k, "t"), 9), False{}))

def lanternward.kit.slash_wind?(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 4, 2)

def lanternward.kit.slash_active?(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 4, 3)

def lanternward.kit.slash_rest?(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 12, 6)

def lanternward.kit.slash_reach?(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 22, 18)

def lanternward.kit.slash_cos?(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 4294967252, 66)

def lanternward.kit.slash_damage?(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 4, 2)

def lanternward.kit.verbs.recover?(+s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.verbs.act(
    lanternward.kit.verbs.recover.go(
      Bool.and(lanternward.kit.Session.recovering(J.get(s, "$k")), Bool.or(U32.is_ne(U32.and(press, 8), 0), U32.is_ne(U32.and(mask, 2), 0))), s),
    id, mask, press, inp)

def lanternward.kit.verbs.recover.go?(yes: Bool, +s: J.Val) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.ksb(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0), "chain", 0), "step", 0), "queued", False{})

def lanternward.kit.verbs.act?(+s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.verbs.release(
    lanternward.kit.verbs.act.go(lanternward.kit.act_code(ga.s(J.get(s, "$k"), "act")), s, id, mask, press, inp),
    id, mask)

def lanternward.kit.verbs.act.go?(+code: U32, +s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  match code:
    case 0:
      lanternward.kit.Session.free(s, id, press, inp)
    case 1:
      lanternward.kit.Session.swinging(s, id, press)
    case 2:
      lanternward.kit.Session.swinging(s, id, press)
    case 3:
      lanternward.kit.Session.swinging(s, id, press)
    case 4:
      lanternward.kit.Session.rolling(s)
    case 5:
      lanternward.kit.Session.hopping(s, id, press)
    case 6:
      lanternward.kit.Session.plunging(s, id)
    case _:
      s

# Letting the sword go ends the winding; a wound spin goes off. Then the lamp.

def lanternward.kit.verbs.release?(+s: J.Val, +id: U32, +mask: U32) -> J.Val:
  lanternward.kit.Session.lantern(
    lanternward.kit.verbs.release.go(U32.is_eq(U32.and(mask, 4), 0), s), id, mask)

def lanternward.kit.verbs.release.go?(up: Bool, +s: J.Val) -> J.Val:
  match up:
    case False{}:
      s
    case True{}:
      lanternward.ks(
        lanternward.kit.verbs.release.spin(
          Bool.and(
            Bool.and(lanternward.kit.Seat.charged(J.get(s, "$k")), lanternward.kis(s, "act", "Free")),
            U32.is_eq(lanternward.ku(s, "shield"), 0)),
          s),
        "charge", 0)

def lanternward.kit.verbs.release.spin?(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.kit.Session.begin_spin(s)
    case False{}:
      s

def lanternward.kit.Session.free?(+s: J.Val, +id: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.free.shield(U32.is_gt(lanternward.ku(s, "shield"), 0), s, id, press, inp)

# A shield up trades the sword and the hop for a wall; E still meets the world.

def lanternward.kit.free.shield?(up: Bool, +s: J.Val, +id: U32, +press: U32, +inp: J.Val) -> J.Val:
  match up:
    case True{}:
      lanternward.kit.free.ctx_only(U32.is_ne(U32.and(press, 8), 0), s, id)
    case False{}:
      lanternward.kit.free.sword(
        Bool.and(U32.is_ne(U32.and(press, 4), 0), lanternward.kis(s, "carry", "Nothing")), s, id, press, inp)

def lanternward.kit.free.ctx_only?(pressed: Bool, +s: J.Val, +id: U32) -> J.Val:
  match pressed:
    case True{}:
      J.del(lanternward.ctx.Session.use_context(s, id), "$ret")
    case False{}:
      s

def lanternward.kit.free.sword?(go: Bool, +s: J.Val, +id: U32, +press: U32, +inp: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.kit.free.sword.lunge(
        lanternward.kit.free.forward(lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")), s, inp), s)
    case False{}:
      lanternward.kit.free.ctx(U32.is_ne(U32.and(press, 8), 0), s, id, press, inp)

# Whether the stick pushes toward the locked target, for a lunge.

def lanternward.kit.free.forward?(+pt: J.Val, +s: J.Val, +inp: J.Val) -> Bool:
  match pt:
    case J.JNull{}:
      False{}
    case _:
      lanternward.kit.free.forward.go(
        lanternward.world.unit(ga.u(inp, "move_x"), ga.u(inp, "move_y")), pt, s)

def lanternward.kit.free.forward.go?(v: U32 & U32, +pt: J.Val, +s: J.Val) -> Bool:
  (+vx, +vy) = v
  Bool.and(
    Bool.not(Bool.and(U32.is_eq(vx, 0), U32.is_eq(vy, 0))),
    RS.i_ge(lanternward.world.dot_unit(vx, vy, (ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), (ga.nu(pt, 1) - lanternward.ku(s, "y") : U32)), 128))

def lanternward.kit.free.sword.lunge?(+forward: Bool, +s: J.Val) -> J.Val:
  lanternward.kit.free.sword.lunge.go(Bool.and(forward, U32.is_eq(lanternward.ku(s, "chain"), 0)), s)

def lanternward.kit.free.sword.lunge.go?(lunge: Bool, +s: J.Val) -> J.Val:
  match lunge:
    case True{}:
      lanternward.kit.Session.begin_lunge(s)
    case False{}:
      lanternward.kit.Session.begin_slash(s, Bool.pick(U32, U32.is_gt(lanternward.ku(s, "chain"), 0), lanternward.ku(s, "step"), 0))

def lanternward.kit.free.ctx?(pressed: Bool, +s: J.Val, +id: U32, +press: U32, +inp: J.Val) -> J.Val:
  match pressed:
    case True{}:
      lanternward.kit.free.ctx.go(lanternward.ctx.Session.use_context(s, id), inp)
    case False{}:
      lanternward.kit.free.bomb(
        Bool.and(Bool.and(U32.is_ne(U32.and(press, 32), 0), U32.is_gt(lanternward.ku(s, "bombs"), 0)), lanternward.kis(s, "carry", "Nothing")),
        s, id)

def lanternward.kit.free.ctx.go?(+s: J.Val, +inp: J.Val) -> J.Val:
  lanternward.kit.free.ctx.then(ga.b(s, "$ret"), J.del(s, "$ret"), inp)

def lanternward.kit.free.ctx.then?(used: Bool, +s: J.Val, +inp: J.Val) -> J.Val:
  match used:
    case True{}:
      s
    case False{}:
      lanternward.kit.Session.begin_dodge(s, inp)

def lanternward.kit.free.bomb?(go: Bool, +s: J.Val, +id: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.sound(
        lanternward.kit.free.bomb.fuse(
          U32.is_lt(lanternward.len(s, "fuses"), 8),
          lanternward.ks(s, "bombs", (lanternward.ku(s, "bombs") - 1 : U32)), id),
        "bomb_toss")

def lanternward.kit.free.bomb.fuse?(room: Bool, +s: J.Val, +id: U32) -> J.Val:
  match room:
    case False{}:
      s
    case True{}:
      lanternward.push(s, "fuses",
        ga.su(ga.su(ga.si(ga.si(J.obj(), "x", (lanternward.ku(s, "x") + lanternward.kface(s, 0) * 12 : U32)),
          "y", (lanternward.ku(s, "y") + lanternward.kface(s, 1) * 12 : U32)), "t", 75), "owner", U32.and(id, 255)))

# ---- the sword ----

def lanternward.kit.Session.begin_slash?(+s: J.Val, +step: U32) -> J.Val:
  lanternward.sound(
    lanternward.ksv(
      lanternward.ksb(
        lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Slash"), "t", 0), "n", RS.u_min(step, 2)), "swing", (lanternward.ku(s, "swing") + 1 : U32)),
        "queued", False{}),
      "mv", ga.pair_i(0, 0)),
    "swing")

def lanternward.kit.Session.begin_lunge?(+s: J.Val) -> J.Val:
  lanternward.sound(
    lanternward.ksv(
      lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Lunge"), "t", 0), "swing", (lanternward.ku(s, "swing") + 1 : U32)), "chain", 0), "step", 0),
      "mv", ga.pair_i(RS.i_div((lanternward.kface(s, 0) * 1100 : U32), 256), RS.i_div((lanternward.kface(s, 1) * 1100 : U32), 256))),
    "swing")

def lanternward.kit.Session.begin_spin?(+s: J.Val) -> J.Val:
  lanternward.sound(
    lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Spin"), "t", 0), "swing", (lanternward.ku(s, "swing") + 1 : U32)), "charge", 0), "chain", 0), "step", 0),
    "axe_spin")

def lanternward.kit.Session.swinging?(+s: J.Val, +id: U32, +press: U32) -> J.Val:
  lanternward.kit.swinging.act(ga.s(J.get(s, "$k"), "act"), lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32)), id, press)

def lanternward.kit.swinging.act?(+act: String, +s: J.Val, +id: U32, +press: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(act, "Slash"), lanternward.kit.swinging.slash(s, id, press),
    Bool.pick(J.Val, String.eq(act, "Lunge"), lanternward.kit.swinging.lunge(s, id),
      Bool.pick(J.Val, String.eq(act, "Spin"), lanternward.kit.swinging.spin(s, id), s)))

def lanternward.kit.swinging.slash?(+s: J.Val, +id: U32, +press: U32) -> J.Val:
  lanternward.kit.swinging.slash.sweep(
    RS.u_min(lanternward.ku(s, "n"), 2),
    Bool.pick(J.Val, U32.is_ne(U32.and(press, 4), 0), lanternward.ksb(s, "queued", True{}), s), id)

def lanternward.kit.swinging.slash.sweep?(+n: U32, +s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.swinging.slash.sweep.go(
    Bool.and(U32.is_gt(lanternward.ku(s, "t"), lanternward.kit.slash_wind(n)), U32.is_le(lanternward.ku(s, "t"), (lanternward.kit.slash_wind(n) + lanternward.kit.slash_active(n) : U32))),
    n, s, id)

def lanternward.kit.swinging.slash.sweep.go?(active: Bool, +n: U32, +s: J.Val, +id: U32) -> J.Val:
  match active:
    case False{}:
      lanternward.kit.swinging.slash.chain(n, s)
    case True{}:
      lanternward.kit.swinging.slash.chain(n, lanternward.kit.swinging.slash.freeze(n, lanternward.kit.Session.sweep(s, id, lanternward.kit.slash_reach(n), lanternward.kit.slash_cos(n), lanternward.kit.slash_damage(n), U32.is_eq(n, 2))))

# A landed blade holds its owner a moment.

def lanternward.kit.swinging.slash.freeze?(+n: U32, +s: J.Val) -> J.Val:
  lanternward.kit.swinging.freeze(ga.b(s, "$ret"), Bool.pick(U32, U32.is_eq(n, 2), 4, 2), J.del(s, "$ret"))

def lanternward.kit.swinging.freeze?(landed: Bool, +amount: U32, +s: J.Val) -> J.Val:
  match landed:
    case False{}:
      s
    case True{}:
      lanternward.ks(s, "freeze", RS.u_max(lanternward.ku(s, "freeze"), amount))

def lanternward.kit.swinging.slash.chain?(+n: U32, +s: J.Val) -> J.Val:
  lanternward.kit.swinging.slash.end(
    n,
    Bool.pick(J.Val, U32.is_eq(lanternward.ku(s, "t"), (lanternward.kit.slash_wind(n) + lanternward.kit.slash_active(n) : U32)),
      lanternward.ks(
        lanternward.ks(s, "chain", (lanternward.kit.slash_rest(n) + lanternward.R(s, "combo") : U32)),
        "step", Bool.pick(U32, U32.is_eq(n, 2), 0, (n + 1 : U32))),
      s))

# The recovery ends: free again, and a queued press starts the next slash.

def lanternward.kit.swinging.slash.end?(+n: U32, +s: J.Val) -> J.Val:
  lanternward.kit.swinging.slash.end.go(
    U32.is_ge(lanternward.ku(s, "t"), (lanternward.kit.slash_wind(n) + lanternward.kit.slash_active(n) + lanternward.kit.slash_rest(n) : U32)), s)

def lanternward.kit.swinging.slash.end.go?(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.kit.swinging.slash.next(
        Bool.and(lanternward.kb(s, "queued"), Bool.and(U32.is_gt(lanternward.ku(s, "step"), 0), U32.is_gt(lanternward.ku(s, "chain"), 0))),
        lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0))

def lanternward.kit.swinging.slash.next?(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.kit.Session.begin_slash(s, lanternward.ku(s, "step"))

def lanternward.kit.swinging.lunge?(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.swinging.lunge.end(
    lanternward.kit.swinging.lunge.hit(
      Bool.and(U32.is_ge(lanternward.ku(s, "t"), 3), U32.is_lt(lanternward.ku(s, "t"), 9)), s, id))

def lanternward.kit.swinging.lunge.hit?(go: Bool, +s: J.Val, +id: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.kit.swinging.lunge.hit.go(
        lanternward.kit.Session.lunge_hit(lanternward.kit.Session.shove(s, ga.nu(lanternward.kv(s, "mv"), 0), ga.nu(lanternward.kv(s, "mv"), 1)), id))

def lanternward.kit.swinging.lunge.hit.go?(+s: J.Val) -> J.Val:
  lanternward.kit.swinging.freeze(ga.b(s, "$ret"), 3, J.del(s, "$ret"))

def lanternward.kit.swinging.lunge.end?(+s: J.Val) -> J.Val:
  lanternward.kit.swinging.lunge.end.go(U32.is_ge(lanternward.ku(s, "t"), 19), s)

def lanternward.kit.swinging.lunge.end.go?(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case True{}:
      lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0)
    case False{}:
      s

def lanternward.kit.swinging.spin?(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.swinging.spin.end(
    lanternward.kit.swinging.spin.hit(
      Bool.and(U32.is_ge(lanternward.ku(s, "t"), 2), U32.is_lt(lanternward.ku(s, "t"), 12)), s, id))

def lanternward.kit.swinging.spin.hit?(go: Bool, +s: J.Val, +id: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.kit.swinging.lunge.hit.go(lanternward.kit.Session.sweep(s, id, 22, 4294967040, 3, True{}))

def lanternward.kit.swinging.spin.end?(+s: J.Val) -> J.Val:
  lanternward.kit.swinging.spin.end.go(U32.is_ge(lanternward.ku(s, "t"), 14), s)

def lanternward.kit.swinging.spin.end.go?(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case True{}:
      lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0)
    case False{}:
      s

# A swing's arc: whether (x, y) lies within `reach` px (plus `extra`) of the
# knight and, unless the swing is a full circle, in front of the knight.

def lanternward.kit.inside?(+k: J.Val, +reach: U32, +cos: U32, +x: U32, +y: U32, +extra: U32) -> Bool:
  lanternward.kit.inside.d(lanternward.world.dist((x - ga.u(k, "x") : U32), (y - ga.u(k, "y") : U32)), k, reach, cos, x, y, extra)

def lanternward.kit.inside.d?(+d: U32, +k: J.Val, +reach: U32, +cos: U32, +x: U32, +y: U32, +extra: U32) -> Bool:
  Bool.and(
    RS.i_le(d, (reach * 256 + extra : U32)),
    Bool.or(
      Bool.or(RS.i_le(cos, 4294967040), RS.i_lt(d, 1536)),
      RS.i_ge(lanternward.world.dot_unit(ga.nu(ga.v(k, "face"), 0), ga.nu(ga.v(k, "face"), 1), (x - ga.u(k, "x") : U32), (y - ga.u(k, "y") : U32)), cos)))

def lanternward.kit.hit_slot?(+id: U32) -> U32:
  RS.u_min((id - 1 : U32), 5)

# Every foe, pot, blade-cut thing and iron bolt inside the arc is struck once.
# `$ret` is true when something was hurt.

def lanternward.kit.Session.sweep?(+s: J.Val, +id: U32, +reach: U32, +cos: U32, +dmg: U32, +heavy: Bool) -> J.Val:
  lanternward.kit.sweep.fin(
    lanternward.kit.sweep.bolts(
      lanternward.kit.sweep.props(
        lanternward.kit.sweep.boss(
          lanternward.kit.sweep.foes(ga.sb(ga.sb(s, "$landed", False{}), "$cut", False{}), id, reach, cos, dmg, 0),
          id, reach, cos, dmg),
        reach, cos, 0),
      reach, cos, 0),
    heavy)

def lanternward.kit.sweep.foes?(+s: J.Val, +id: U32, +reach: U32, +cos: U32, +dmg: U32, +i: U32) -> J.Val:
  lanternward.kit.sweep.foes.go(U32.is_lt(i, lanternward.len(s, "foes")), s, id, reach, cos, dmg, i)

def lanternward.kit.sweep.foes.go?(more: Bool, +s: J.Val, +id: U32, +reach: U32, +cos: U32, +dmg: U32, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.foes(
        lanternward.kit.sweep.foe(
          Bool.and(
            Bool.and(
              Bool.not(U32.is_eq(J.u32(J.nth(lanternward.ev(s, "foes", i, "hit"), lanternward.kit.hit_slot(id))), lanternward.ku(s, "swing"))),
              lanternward.foes.Foe.alive(lanternward.el(s, "foes", i))),
            lanternward.kit.inside(J.get(s, "$k"), reach, cos, lanternward.eu(s, "foes", i, "x"), lanternward.eu(s, "foes", i, "y"),
              lanternward.foes.Foe.radius(lanternward.el(s, "foes", i)))),
          s, id, dmg, i),
        id, reach, cos, dmg, (i + 1 : U32))

def lanternward.kit.sweep.foe?(hit: Bool, +s: J.Val, +id: U32, +dmg: U32, +i: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.landed(
        lanternward.blow.Session.hurt_foe(
          lanternward.eset(s, "foes", i, "hit", J.set_nth(lanternward.ev(s, "foes", i, "hit"), lanternward.kit.hit_slot(id), J.JU{lanternward.ku(s, "swing")})),
          i, dmg, lanternward.ku(s, "x"), lanternward.ku(s, "y")))

# hurt_foe / hurt_boss left whether they hurt in `$ret`: fold it into `$landed`.

def lanternward.kit.sweep.landed?(+s: J.Val) -> J.Val:
  lanternward.kit.sweep.landed.go(ga.b(s, "$ret"), J.del(s, "$ret"))

def lanternward.kit.sweep.landed.go?(hurt: Bool, +s: J.Val) -> J.Val:
  match hurt:
    case True{}:
      ga.sb(s, "$landed", True{})
    case False{}:
      s

def lanternward.kit.sweep.boss?(+s: J.Val, +id: U32, +reach: U32, +cos: U32, +dmg: U32) -> J.Val:
  lanternward.kit.sweep.boss.go(
    Bool.and(Bool.not(J.is_null(J.get(s, "boss"))), U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0)), s, id, reach, cos, dmg)

def lanternward.kit.sweep.boss.go?(alive: Bool, +s: J.Val, +id: U32, +reach: U32, +cos: U32, +dmg: U32) -> J.Val:
  match alive:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.boss.hit(
        Bool.and(
          lanternward.kit.inside(J.get(s, "$k"), reach, cos, ga.u(J.get(s, "boss"), "x"), ga.u(J.get(s, "boss"), "y"), 4608),
          Bool.not(U32.is_eq(J.u32(J.nth(ga.v(J.get(s, "boss"), "hit"), lanternward.kit.hit_slot(id))), lanternward.ku(s, "swing")))),
        s, id, dmg)

def lanternward.kit.sweep.boss.hit?(hit: Bool, +s: J.Val, +id: U32, +dmg: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.landed(
        lanternward.boss.Session.hurt_boss(
          ga.sv(s, "boss", ga.sv(J.get(s, "boss"), "hit", J.set_nth(ga.v(J.get(s, "boss"), "hit"), lanternward.kit.hit_slot(id), J.JU{lanternward.ku(s, "swing")}))),
          dmg, lanternward.ku(s, "x"), lanternward.ku(s, "y"), id))

def lanternward.kit.sweep.props?(+s: J.Val, +reach: U32, +cos: U32, +i: U32) -> J.Val:
  lanternward.kit.sweep.props.go(U32.is_lt(i, lanternward.len(s, "props")), s, reach, cos, i)

def lanternward.kit.sweep.props.go?(more: Bool, +s: J.Val, +reach: U32, +cos: U32, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.props(
        lanternward.kit.sweep.prop(
          Bool.and(
            Bool.and(lanternward.NAME_IN(ga.s(lanternward.el(s, "props", i), "kind"), ["Pot", "Grass", "Crystal"]),
              U32.is_eq(lanternward.eu(s, "props", i, "on"), 0)),
            lanternward.kit.inside(J.get(s, "$k"), reach, cos, lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 1536)),
          s, i),
        reach, cos, (i + 1 : U32))

def lanternward.kit.sweep.prop?(cut: Bool, +s: J.Val, +i: U32) -> J.Val:
  match cut:
    case False{}:
      s
    case True{}:
      Bool.pick(J.Val, lanternward.eis(s, "props", i, "kind", "Crystal"),
        lanternward.ctx.Session.light_crystal(s, i),
        lanternward.props.Session.shatter(s, i))

def lanternward.kit.sweep.bolts?(+s: J.Val, +reach: U32, +cos: U32, +i: U32) -> J.Val:
  lanternward.kit.sweep.bolts.go(U32.is_lt(i, lanternward.len(s, "bolts")), s, reach, cos, i)

def lanternward.kit.sweep.bolts.go?(more: Bool, +s: J.Val, +reach: U32, +cos: U32, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.bolts(
        lanternward.kit.sweep.bolt(
          Bool.and(
            lanternward.foes.Bolt.hostile(lanternward.el(s, "bolts", i)),
            lanternward.kit.inside(J.get(s, "$k"), reach, cos, lanternward.eu(s, "bolts", i, "x"), lanternward.eu(s, "bolts", i, "y"), 1024)),
          s, i),
        reach, cos, (i + 1 : U32))

def lanternward.kit.sweep.bolt?(cut: Bool, +s: J.Val, +i: U32) -> J.Val:
  match cut:
    case False{}:
      s
    case True{}:
      ga.sb(lanternward.es(s, "bolts", i, "ttl", 0), "$cut", True{})

def lanternward.kit.sweep.fin?(+s: J.Val, +heavy: Bool) -> J.Val:
  lanternward.kit.sweep.out(ga.b(s, "$landed"), heavy, lanternward.kit.sweep.clang(ga.b(s, "$cut"), s))

def lanternward.kit.sweep.clang?(cut: Bool, +s: J.Val) -> J.Val:
  match cut:
    case False{}:
      s
    case True{}:
      lanternward.props.Session.fx(
        lanternward.sound(s, "clang"), "Spark",
        (lanternward.ku(s, "x") + lanternward.kface(s, 0) * 18 : U32), (lanternward.ku(s, "y") + lanternward.kface(s, 1) * 18 : U32), 0)

def lanternward.kit.sweep.out?(+landed: Bool, +heavy: Bool, +s: J.Val) -> J.Val:
  ga.sb(J.del(J.del(lanternward.kit.sweep.heavy(Bool.and(landed, heavy), lanternward.kit.sweep.stop(landed, s)), "$landed"), "$cut"), "$ret", landed)

def lanternward.kit.sweep.stop?(landed: Bool, +s: J.Val) -> J.Val:
  match landed:
    case True{}:
      lanternward.stop(s, "Heavy")
    case False{}:
      s

def lanternward.kit.sweep.heavy?(hard: Bool, +s: J.Val) -> J.Val:
  match hard:
    case True{}:
      lanternward.sound(ga.su(s, "shake", RS.u_max(ga.u(s, "shake"), 8)), "hit_axe")
    case False{}:
      s

# A lunge's line: from the knight out ahead, narrow and long. `$ret` is true
# when it landed.

def lanternward.kit.Session.lunge_hit?(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.lunge_hit.go(
    lanternward.variant("Capsule",
      ga.si(ga.si(ga.si(ga.si(ga.si(J.obj(), "ax", lanternward.ku(s, "x")), "ay", lanternward.ku(s, "y")),
        "bx", (lanternward.ku(s, "x") + lanternward.kface(s, 0) * 30 : U32)), "by", (lanternward.ku(s, "y") + lanternward.kface(s, 1) * 30 : U32)), "r", 1280)),
    s, id)

def lanternward.kit.lunge_hit.go?(+line: J.Val, +s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.lunge_hit.fin(
    lanternward.kit.lunge_hit.boss(lanternward.kit.lunge_hit.foes(ga.sb(s, "$landed", False{}), line, id, 0), line, id), 0)

def lanternward.kit.lunge_hit.fin?(+s: J.Val, +z: U32) -> J.Val:
  lanternward.kit.lunge_hit.fin.go(ga.b(s, "$landed"), s)

def lanternward.kit.lunge_hit.fin.go?(+landed: Bool, +s: J.Val) -> J.Val:
  ga.sb(J.del(Bool.pick(J.Val, landed, lanternward.stop(s, "Heavy"), s), "$landed"), "$ret", landed)

def lanternward.kit.lunge_hit.foes?(+s: J.Val, +line: J.Val, +id: U32, +i: U32) -> J.Val:
  lanternward.kit.lunge_hit.foes.go(U32.is_lt(i, lanternward.len(s, "foes")), s, line, id, i)

def lanternward.kit.lunge_hit.foes.go?(more: Bool, +s: J.Val, +line: J.Val, +id: U32, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.kit.lunge_hit.foes(
        lanternward.kit.lunge_hit.foe(
          Bool.and(
            Bool.and(
              Bool.not(U32.is_eq(J.u32(J.nth(lanternward.ev(s, "foes", i, "hit"), lanternward.kit.hit_slot(id))), lanternward.ku(s, "swing"))),
              lanternward.foes.Foe.alive(lanternward.el(s, "foes", i))),
            lanternward.foes.Danger.hits(line, lanternward.eu(s, "foes", i, "x"), lanternward.eu(s, "foes", i, "y"),
              lanternward.foes.Foe.radius(lanternward.el(s, "foes", i)))),
          s, id, i),
        line, id, (i + 1 : U32))

def lanternward.kit.lunge_hit.foe?(hit: Bool, +s: J.Val, +id: U32, +i: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.landed(
        lanternward.blow.Session.hurt_foe(
          lanternward.eset(s, "foes", i, "hit", J.set_nth(lanternward.ev(s, "foes", i, "hit"), lanternward.kit.hit_slot(id), J.JU{lanternward.ku(s, "swing")})),
          i, 3, lanternward.ku(s, "x"), lanternward.ku(s, "y")))

def lanternward.kit.lunge_hit.boss?(+s: J.Val, +line: J.Val, +id: U32) -> J.Val:
  lanternward.kit.lunge_hit.boss.go(
    Bool.and(Bool.not(J.is_null(J.get(s, "boss"))), U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0)), s, line, id)

def lanternward.kit.lunge_hit.boss.go?(alive: Bool, +s: J.Val, +line: J.Val, +id: U32) -> J.Val:
  match alive:
    case False{}:
      s
    case True{}:
      lanternward.kit.lunge_hit.boss.hit(
        Bool.and(
          Bool.not(U32.is_eq(J.u32(J.nth(ga.v(J.get(s, "boss"), "hit"), lanternward.kit.hit_slot(id))), lanternward.ku(s, "swing"))),
          lanternward.foes.Danger.hits(line, ga.u(J.get(s, "boss"), "x"), ga.u(J.get(s, "boss"), "y"), 4608)),
        s, id)

def lanternward.kit.lunge_hit.boss.hit?(hit: Bool, +s: J.Val, +id: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.landed(
        lanternward.boss.Session.hurt_boss(
          ga.sv(s, "boss", ga.sv(J.get(s, "boss"), "hit", J.set_nth(ga.v(J.get(s, "boss"), "hit"), lanternward.kit.hit_slot(id), J.JU{lanternward.ku(s, "swing")}))),
          3, lanternward.ku(s, "x"), lanternward.ku(s, "y"), id))

# ---- dodges ----

# E with nothing to do: a roll along the walk, or, locked on, the hop the
# stick asks for.

def lanternward.kit.Session.begin_dodge?(+s: J.Val, +inp: J.Val) -> J.Val:
  lanternward.kit.begin_dodge.go(
    lanternward.world.unit(ga.u(inp, "move_x"), ga.u(inp, "move_y")),
    lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")), s)

def lanternward.kit.begin_dodge.go?(walk: U32 & U32, +pt: J.Val, +s: J.Val) -> J.Val:
  (+wx, +wy) = walk
  lanternward.kit.begin_dodge.done(
    lanternward.sound(
      Bool.pick(J.Val, J.is_null(pt),
        lanternward.kit.begin_dodge.roll(wx, wy, s),
        lanternward.kit.begin_dodge.hop(wx, wy, pt, s)),
      "roll"))

def lanternward.kit.begin_dodge.done?(+s: J.Val) -> J.Val:
  lanternward.ks(lanternward.ks(lanternward.ks(s, "chain", 0), "step", 0), "charge", 0)

def lanternward.kit.begin_dodge.roll?(+wx: U32, +wy: U32, +s: J.Val) -> J.Val:
  lanternward.kit.begin_dodge.roll.go(
    Bool.pick(U32, Bool.and(U32.is_eq(wx, 0), U32.is_eq(wy, 0)), lanternward.kface(s, 0), wx),
    Bool.pick(U32, Bool.and(U32.is_eq(wx, 0), U32.is_eq(wy, 0)), lanternward.kface(s, 1), wy), s)

def lanternward.kit.begin_dodge.roll.go?(+vx: U32, +vy: U32, +s: J.Val) -> J.Val:
  lanternward.ksv(
    lanternward.ksv(lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Roll"), "t", 0), "n", 0), "mv",
      ga.pair_i(RS.i_div((vx * 1100 : U32), 256), RS.i_div((vy * 1100 : U32), 256))),
    "face", ga.pair_i(vx, vy))

# Locked on: forward, back or to a side relative to the lock.

def lanternward.kit.begin_dodge.hop?(+wx: U32, +wy: U32, +pt: J.Val, +s: J.Val) -> J.Val:
  lanternward.kit.begin_dodge.hop.fwd(
    lanternward.world.unit((ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), (ga.nu(pt, 1) - lanternward.ku(s, "y") : U32)), wx, wy, s)

def lanternward.kit.begin_dodge.hop.fwd?(fwd: U32 & U32, +wx: U32, +wy: U32, +s: J.Val) -> J.Val:
  (+fx, +fy) = fwd
  +still = Bool.and(U32.is_eq(wx, 0), U32.is_eq(wy, 0))
  lanternward.kit.begin_dodge.hop.pick(
    Bool.pick(U32, still, 4294967040, lanternward.world.dot4(wx, wy, fx, fy)),
    Bool.pick(U32, still, 0, lanternward.world.dot4(wx, wy, RS.i_neg(fy), fx)),
    fx, fy, s)

def lanternward.kit.begin_dodge.hop.pick?(+f: U32, +sd: U32, +fx: U32, +fy: U32, +s: J.Val) -> J.Val:
  lanternward.kit.begin_dodge.hop.set(
    Bool.and(RS.i_gt(RS.i_abs(sd), RS.i_abs(f)), RS.i_gt(RS.i_abs(sd), 64)),
    RS.i_gt(f, 64), RS.i_signum(sd), f, fx, fy, s)

def lanternward.kit.begin_dodge.hop.set?(+side: Bool, +fwd: Bool, +sign: U32, +f: U32, +fx: U32, +fy: U32, +s: J.Val) -> J.Val:
  lanternward.kit.begin_dodge.hop.fin(
    Bool.pick(U32, side, 0, Bool.pick(U32, fwd, 2, 1)),
    Bool.pick(U32, side, (RS.i_neg(fy) * sign : U32), Bool.pick(U32, fwd, fx, RS.i_neg(fx))),
    Bool.pick(U32, side, (fx * sign : U32), Bool.pick(U32, fwd, fy, RS.i_neg(fy))),
    Bool.pick(U32, side, 640, Bool.pick(U32, fwd, 640, 683)),
    s)

def lanternward.kit.begin_dodge.hop.fin?(+kind: U32, +vx: U32, +vy: U32, +speed: U32, +s: J.Val) -> J.Val:
  lanternward.ks(
    lanternward.ksv(
      lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Hop"), "n", kind), "t", 0),
      "mv", ga.pair_i(RS.i_div((vx * speed : U32), 256), RS.i_div((vy * speed : U32), 256))),
    "lock_t", 0)

def lanternward.kit.Session.rolling?(+s: J.Val) -> J.Val:
  lanternward.kit.rolling.end(
    lanternward.kit.rolling.move(U32.is_lt((lanternward.ku(s, "t") + 1 : U32), 12), lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32))))

def lanternward.kit.rolling.move?(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.kit.Session.shove(s, ga.nu(lanternward.kv(s, "mv"), 0), ga.nu(lanternward.kv(s, "mv"), 1))

def lanternward.kit.rolling.end?(+s: J.Val) -> J.Val:
  lanternward.kit.rolling.inv(lanternward.kit.rolling.done(U32.is_ge(lanternward.ku(s, "t"), 15), s))

def lanternward.kit.rolling.done?(over: Bool, +s: J.Val) -> J.Val:
  match over:
    case False{}:
      s
    case True{}:
      lanternward.kit.rolling.pit(
        lanternward.props.Session.pit(s, lanternward.ku(s, "x"), lanternward.ku(s, "y")),
        lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0))

def lanternward.kit.rolling.pit?(pit: Bool, +s: J.Val) -> J.Val:
  match pit:
    case True{}:
      lanternward.ksn(s, "act", "Fall")
    case False{}:
      s

def lanternward.kit.rolling.inv?(+s: J.Val) -> J.Val:
  lanternward.ks(s, "inv", RS.u_max(lanternward.ku(s, "inv"), Bool.to_u32(Bool.and(U32.is_ge(lanternward.ku(s, "t"), 2), U32.is_le(lanternward.ku(s, "t"), 10)))))

def lanternward.kit.Session.hopping?(+s: J.Val, +id: U32, +press: U32) -> J.Val:
  lanternward.kit.hopping.move(
    lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32)), press)

def lanternward.kit.hopping.move?(+s: J.Val, +press: U32) -> J.Val:
  lanternward.kit.hopping.z(
    Bool.pick(J.Val, U32.is_lt(lanternward.ku(s, "t"), 16),
      lanternward.kit.Session.shove(s, ga.nu(lanternward.kv(s, "mv"), 0), ga.nu(lanternward.kv(s, "mv"), 1)), s),
    press)

# A parabola, peaking mid-hop.

def lanternward.kit.hopping.z?(+s: J.Val, +press: U32) -> J.Val:
  lanternward.kit.hopping.plunge(
    lanternward.ksi(
      s, "z",
      RS.i_max(RS.i_div((3584 * 4 * lanternward.ku(s, "t") * (18 - lanternward.ku(s, "t") : U32) : U32), 324), 0)),
    press)

def lanternward.kit.hopping.plunge?(+s: J.Val, +press: U32) -> J.Val:
  lanternward.kit.hopping.jump(
    Bool.and(U32.is_ne(U32.and(press, 4), 0), RS.i_gt(lanternward.ku(s, "z"), 512)),
    lanternward.ks(s, "inv",
      Bool.pick(U32, Bool.and(U32.is_ge(lanternward.ku(s, "t"), 2), U32.is_le(lanternward.ku(s, "t"), 14)),
        RS.u_max(lanternward.ku(s, "inv"), 1), lanternward.ku(s, "inv"))))

# The jump attack: straight down on whatever is below.

def lanternward.kit.hopping.jump?(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.sound(
        lanternward.ksv(
          lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Plunge"), "t", 0), "swing", (lanternward.ku(s, "swing") + 1 : U32)),
          "mv", ga.pair_i(0, 0)),
        "swing")
    case False{}:
      lanternward.kit.hopping.land(U32.is_ge(lanternward.ku(s, "t"), 18), s)

def lanternward.kit.hopping.land?(down: Bool, +s: J.Val) -> J.Val:
  match down:
    case False{}:
      s
    case True{}:
      lanternward.kit.rolling.pit(
        lanternward.props.Session.pit(s, lanternward.ku(s, "x"), lanternward.ku(s, "y")),
        lanternward.props.Session.fx(
          lanternward.ks(lanternward.ksn(lanternward.ksi(s, "z", 0), "act", "Free"), "t", 0),
          "Dust", lanternward.ku(s, "x"), lanternward.ku(s, "y"), 0))

def lanternward.kit.Session.plunging?(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.plunging.fall(
    lanternward.ksi(
      lanternward.ks(lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32)), "inv", RS.u_max(lanternward.ku(s, "inv"), 1)),
      "z", RS.i_max(RS.i_sub(lanternward.ku(s, "z"), 768), 0)),
    id)

def lanternward.kit.plunging.fall?(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.plunging.down(U32.is_eq(lanternward.ku(s, "z"), 0), s, id)

# Down: the blow lands in a ring about the knight.

def lanternward.kit.plunging.down?(grounded: Bool, +s: J.Val, +id: U32) -> J.Val:
  match grounded:
    case False{}:
      s
    case True{}:
      lanternward.kit.plunging.land(lanternward.kit.Session.sweep(s, id, 18, 4294967040, 4, True{}))

def lanternward.kit.plunging.land?(+s: J.Val) -> J.Val:
  lanternward.kit.plunging.end(
    ga.b(s, "$ret"),
    lanternward.props.Session.fx(J.del(s, "$ret"), "Ring", lanternward.ku(s, "x"), lanternward.ku(s, "y"), 24))

def lanternward.kit.plunging.end?(+hit: Bool, +s: J.Val) -> J.Val:
  lanternward.kit.rolling.pit(
    lanternward.props.Session.pit(s, lanternward.ku(s, "x"), lanternward.ku(s, "y")),
    lanternward.ks(
      lanternward.ks(
        lanternward.ksn(
          lanternward.ks(
            Bool.pick(J.Val, hit, lanternward.ks(lanternward.kit.plunging.thud(s), "freeze", 3), lanternward.kit.plunging.thud(s)),
            "t", 0),
          "act", "Free"),
        "t", 0),
      "guard_cd", 8))

def lanternward.kit.plunging.thud?(+s: J.Val) -> J.Val:
  ga.su(lanternward.sound(s, "rock_land"), "shake", RS.u_max(ga.u(s, "shake"), 8))

# ---- the lantern ----

def lanternward.kit.Session.lantern?(+s: J.Val, +id: U32, +mask: U32) -> J.Val:
  lanternward.kit.lantern.owned(
    Bool.and(lanternward.kit.Session.lantern_owned(s), lanternward.NAME_IN(ga.s(J.get(s, "$k"), "act"), ["Free", "Slash", "Hop"])),
    s, id, mask)

def lanternward.kit.lantern.owned?(go: Bool, +s: J.Val, +id: U32, +mask: U32) -> J.Val:
  match go:
    case False{}:
      lanternward.ks(s, "lamp", 0)
    case True{}:
      lanternward.kit.lantern.held(U32.is_ne(U32.and(mask, 16), 0), s, id)

def lanternward.kit.lantern.held?(held: Bool, +s: J.Val, +id: U32) -> J.Val:
  match held:
    case True{}:
      lanternward.kit.lantern.burn(
        lanternward.kit.lantern.cast(
          U32.is_eq(RS.u_min((lanternward.ku(s, "lamp") + 1 : U32), 255), 8),
          lanternward.ks(s, "lamp", RS.u_min((lanternward.ku(s, "lamp") + 1 : U32), 255))))
    case False{}:
      lanternward.ks(lanternward.kit.lantern.release(s, id), "lamp", 0)

def lanternward.kit.lantern.cast?(first: Bool, +s: J.Val) -> J.Val:
  match first:
    case True{}:
      Bool.pick(J.Val, U32.is_gt(lanternward.ku(s, "flame"), 0), lanternward.sound(s, "spell_cast"), s)
    case False{}:
      s

def lanternward.kit.lantern.burn?(+s: J.Val) -> J.Val:
  lanternward.kit.lantern.burn.go(lanternward.kit.Seat.flaring(J.get(s, "$k")), s)

def lanternward.kit.lantern.burn.go?(flaring: Bool, +s: J.Val) -> J.Val:
  match flaring:
    case False{}:
      s
    case True{}:
      lanternward.kit.lantern.out(
        U32.is_eq(RS.u_sat_sub(lanternward.ku(s, "flame"), 1), 0),
        lanternward.ks(s, "flame", RS.u_sat_sub(lanternward.ku(s, "flame"), 1)))

# Sputters out: the knight cannot flare again for a bit.

def lanternward.kit.lantern.out?(dead: Bool, +s: J.Val) -> J.Val:
  match dead:
    case False{}:
      s
    case True{}:
      lanternward.sound(lanternward.ks(lanternward.ks(s, "lamp", 0), "lamp_cd", 45), "mana_empty")

# A tap throws a bolt of light on the release.

def lanternward.kit.lantern.release?(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.lantern.tap(
    Bool.and(
      Bool.and(U32.is_gt(lanternward.ku(s, "lamp"), 0), U32.is_lt(lanternward.ku(s, "lamp"), 8)),
      U32.is_eq(lanternward.ku(s, "lamp_cd"), 0)),
    s, id)

def lanternward.kit.lantern.tap?(go: Bool, +s: J.Val, +id: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.kit.lantern.cost(U32.div(lanternward.R(s, "flame"), 5), s, id)

def lanternward.kit.lantern.cost?(+cost: U32, +s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.lantern.cost.go(U32.is_ge(lanternward.ku(s, "flame"), cost), cost, s, id)

def lanternward.kit.lantern.cost.go?(afford: Bool, +cost: U32, +s: J.Val, +id: U32) -> J.Val:
  match afford:
    case True{}:
      lanternward.kit.lantern.throw(
        lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")),
        lanternward.ks(lanternward.ks(s, "flame", (lanternward.ku(s, "flame") - cost : U32)), "lamp_cd", 14), id)
    case False{}:
      lanternward.sound(s, "mana_empty")

def lanternward.kit.lantern.throw?(+pt: J.Val, +s: J.Val, +id: U32) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.kit.lantern.bolt(lanternward.kface(s, 0), lanternward.kface(s, 1), s, id)
    case _:
      lanternward.kit.lantern.aim(
        lanternward.world.unit((ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), (ga.nu(pt, 1) - lanternward.ku(s, "y") : U32)), s, id)

def lanternward.kit.lantern.aim?(a: U32 & U32, +s: J.Val, +id: U32) -> J.Val:
  (+ax, +ay) = a
  lanternward.kit.lantern.bolt(ax, ay, s, id)

def lanternward.kit.lantern.bolt?(+ax: U32, +ay: U32, +s: J.Val, +id: U32) -> J.Val:
  lanternward.sound(
    Bool.pick(J.Val, U32.is_lt(lanternward.len(s, "bolts"), 40),
      lanternward.push(s, "bolts",
        ga.su(
          ga.su(
            ga.si(
              ga.si(
                ga.si(
                  ga.si(ga.sv(J.obj(), "kind", J.JS{"Light"}), "x", (lanternward.ku(s, "x") + ax * 8 : U32)),
                  "y", (lanternward.ku(s, "y") + ay * 8 : U32)),
                "vx", RS.i_div((ax * 1150 : U32), 256)),
              "vy", RS.i_div((ay * 1150 : U32), 256)),
            "ttl", 30),
          "owner", U32.and(id, 255))),
      s),
    "shot_spark")

# ---- camera ----

# The view eases to the knight, leaning toward a lock; a fresh lock swings it
# in fast, a let-go eases it back slowly.

def lanternward.kit.Session.camera?(+s: J.Val) -> J.Val:
  lanternward.kit.camera.lock(lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")), s)

def lanternward.kit.camera.lock?(+pt: J.Val, +s: J.Val) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.kit.camera.boss(lanternward.ku(s, "x"), lanternward.ku(s, "y"), s)
    case _:
      lanternward.kit.camera.boss(
        (lanternward.ku(s, "x") + RS.i_div((RS.i_mul((ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), 38)), 100) : U32),
        (lanternward.ku(s, "y") + RS.i_div((RS.i_mul((ga.nu(pt, 1) - lanternward.ku(s, "y") : U32), 38)), 100) : U32), s)

# The Warden stands head and shoulders over a knight: while it is up and not
# the lock, the view leans toward it, up to 52 px.

def lanternward.kit.camera.boss?(+tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.boss.go(
    Bool.and(
      Bool.not(lanternward.tag_is(lanternward.kv(s, "lock"), "Boss")),
      Bool.and(
        Bool.not(J.is_null(J.get(s, "boss"))),
        Bool.and(U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0), Bool.not(lanternward.is(J.get(s, "boss"), "st", "Asleep"))))),
    tx, ty, s)

def lanternward.kit.camera.boss.go?(pull: Bool, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  match pull:
    case False{}:
      lanternward.kit.camera.cue(tx, ty, s)
    case True{}:
      lanternward.kit.camera.pull(
        (ga.u(J.get(s, "boss"), "x") - tx : U32), (ga.u(J.get(s, "boss"), "y") - ty : U32), tx, ty, s)

def lanternward.kit.camera.pull?(+dx: U32, +dy: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.pull.go(RS.i_max(lanternward.world.dist(dx, dy), 1), dx, dy, tx, ty, s)

def lanternward.kit.camera.pull.go?(+d: U32, +dx: U32, +dy: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.pull.fin(RS.i_min(RS.i_div(RS.i_mul(d, 70), 100), 13312), d, dx, dy, tx, ty, s)

def lanternward.kit.camera.pull.fin?(+pull: U32, +d: U32, +dx: U32, +dy: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.cue(
    (tx + lanternward.world.mul_div(dx, pull, d) : U32), (ty + lanternward.world.mul_div(dy, pull, d) : U32), s)

# A door or a switch that came right: every view looks at it a moment.

def lanternward.kit.camera.cue?(+tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.cue.has(J.is_null(J.get(s, "cue")), tx, ty, s)

def lanternward.kit.camera.cue.has?(none: Bool, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  match none:
    case True{}:
      lanternward.kit.camera.ease(tx, ty, False{}, s)
    case False{}:
      lanternward.kit.camera.cue.on(
        ga.nu(J.get(s, "cue"), 2), ga.nu(J.get(s, "cue"), 0), ga.nu(J.get(s, "cue"), 1), tx, ty, s)

def lanternward.kit.camera.cue.on?(+t: U32, +cx: U32, +cy: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.cue.go(
    Bool.pick(U32, U32.is_gt(t, 36), RS.i_div((RS.i_mul((44 - t : U32), 100)), 8),
      Bool.pick(U32, U32.is_gt(t, 8), 100, RS.i_div((RS.i_mul(t, 100)), 8))),
    cx, cy, tx, ty, s)

def lanternward.kit.camera.cue.go?(+on: U32, +cx: U32, +cy: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.ease(
    (tx + RS.i_div(RS.i_mul((cx - tx : U32), on), 100) : U32), (ty + RS.i_div(RS.i_mul((cy - ty : U32), on), 100) : U32),
    RS.i_gt(on, 0), s)

def lanternward.kit.camera.ease?(+tx: U32, +ty: U32, +cued: Bool, +s: J.Val) -> J.Val:
  lanternward.kit.camera.clamp(
    Bool.pick(U32, cued, 3,
      Bool.pick(U32, Bool.and(Bool.not(J.is_null(lanternward.kv(s, "lock"))), U32.is_lt(lanternward.ku(s, "lock_t"), 6)), 6,
        Bool.pick(U32, Bool.not(J.is_null(lanternward.kv(s, "lock"))), 3,
          Bool.pick(U32, U32.is_gt(lanternward.ku(s, "unlock_t"), 0), 2, 4)))),
    tx, ty, s)

def lanternward.kit.camera.clamp?(+rate: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.fin(
    RS.i_add(ga.nu(lanternward.kv(s, "cam"), 0), RS.i_div(RS.i_mul((tx - ga.nu(lanternward.kv(s, "cam"), 0) : U32), rate), 8)),
    RS.i_add(ga.nu(lanternward.kv(s, "cam"), 1), RS.i_div(RS.i_mul((ty - ga.nu(lanternward.kv(s, "cam"), 1) : U32), rate), 8)),
    (lanternward.world.Plan.w(ga.u(s, "room")) * 4096 : U32), (lanternward.world.Plan.h(ga.u(s, "room")) * 4096 : U32), s)

def lanternward.kit.camera.fin?(+cx: U32, +cy: U32, +rw: U32, +rh: U32, +s: J.Val) -> J.Val:
  lanternward.ksv(s, "cam",
    ga.pair_i(
      Bool.pick(U32, RS.i_le(rw, 98304), RS.i_div(rw, 2), RS.i_clamp(cx, 49152, (rw - 49152 : U32))),
      Bool.pick(U32, RS.i_le(rh, 57344), RS.i_div(rh, 2), RS.i_clamp(cy, 28672, (rh - 28672 : U32)))))

def lanternward.NAME_IN?(+k: String, +names: List<&2, String>) -> Bool:
  match names:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(String.eq(k, h), lanternward.NAME_IN(k, t))

# `Pk::of`: the kind a plan letter makes, or null.

def lanternward.props.Pk.of?(+c: U32) -> J.Val:
  match c:
    case 111:
      J.JS{"Pot"}
    case 120:
      J.JS{"Pot"}
    case 34:
      J.JS{"Grass"}
    case 115:
      J.JS{"Sign"}
    case 66:
      J.JS{"Brazier"}
    case 67:
      J.JS{"Rest"}
    case 69:
      J.JS{"Ember"}
    case 98:
      J.JS{"Great"}
    case 99:
      J.JS{"Chest"}
    case 71:
      J.JS{"Gate"}
    case 80:
      J.JS{"Plate"}
    case 89:
      J.JS{"Plate"}
    case 88:
      J.JS{"Block"}
    case 75:
      J.JS{"Crystal"}
    case 126:
      J.JS{"Cracked"}
    case 78:
      J.JS{"Sigil"}
    case 76:
      J.JS{"Lock"}
    case 77:
      J.JS{"Boss"}
    case 42:
      J.JS{"Beacon"}
    case 102:
      J.JS{"Fissure"}
    case 70:
      J.JS{"Folk"}
    case 47:
      J.JS{"Lever"}
    case _:
      lanternward.props.Pk.of.digit(Bool.and(U32.is_ge(c, 49), U32.is_le(c, 57)))

def lanternward.props.Pk.of.digit?(d: Bool) -> J.Val:
  match d:
    case True{}:
      J.JS{"Gate"}
    case False{}:
      J.JNull{}

def lanternward.props.Prop.new?(+kind: String, +tx: U32, +ty: U32, +id: U32) -> J.Val:
  +p0 = J.set(J.obj(), "kind", J.JS{kind})
  +p1 = ga.su(p0, "tx", U32.and(tx, 255))
  +p2 = ga.su(p1, "ty", U32.and(ty, 255))
  +p3 = ga.su(p2, "id", id)
  +p4 = ga.su(p3, "on", 0)
  +p5 = ga.su(p4, "t", 0)
  +p6 = ga.si(p5, "x", lanternward.world.middle(tx))
  ga.si(p6, "y", lanternward.world.middle(ty))

# Whether it is lit now.

def lanternward.props.Prop.lit?(+p: J.Val) -> Bool:
  +k = ga.s(p, "kind")
  +on = ga.u(p, "on")
  Bool.pick(Bool, lanternward.NAME_IN(k, ["Rest", "Ember"]), True{},
    Bool.pick(Bool, String.eq(k, "Brazier"), Bool.or(U32.is_gt(ga.u(p, "t"), 0), U32.is_eq(on, 1)),
      Bool.pick(Bool, lanternward.NAME_IN(k, ["Great", "Beacon"]), U32.is_eq(on, 1), False{})))

# Whether a body at this point (Q pixels) is stopped by it.

def lanternward.props.Prop.blocks?(+p: J.Val, +x: U32, +y: U32) -> Bool:
  +k = ga.s(p, "kind")
  +on = ga.u(p, "on")
  +here = Bool.and(
    U32.is_eq(lanternward.world.tile_of(x), ga.u(p, "tx")),
    U32.is_eq(lanternward.world.tile_of(y), ga.u(p, "ty")))
  +near = Bool.and(
    RS.i_lt(RS.i_abs((x - ga.u(p, "x") : U32)), 2048),
    RS.i_lt(RS.i_abs((y - ga.u(p, "y") : U32)), 2048))
  Bool.pick(Bool, lanternward.NAME_IN(k, ["Grass", "Plate", "Sigil", "Fissure"]), False{},
    Bool.pick(Bool, lanternward.NAME_IN(k, ["Pot", "Gate", "Lock", "Boss", "Cracked"]), Bool.and(U32.is_eq(on, 0), here),
      Bool.pick(Bool, String.eq(k, "Chest"), Bool.and(U32.is_ne(on, 2), here),
        Bool.pick(Bool, String.eq(k, "Block"), near, here))))

# Whether a bolt flies through it.

def lanternward.props.Prop.lets_bolts?(+p: J.Val) -> Bool:
  lanternward.NAME_IN(ga.s(p, "kind"), ["Gate", "Sigil", "Grass", "Plate", "Fissure", "Pot"])

# Whether the knight in front of it can lock on to it.

def lanternward.props.Prop.target?(+p: J.Val) -> Bool:
  +k = ga.s(p, "kind")
  Bool.pick(Bool, lanternward.NAME_IN(k, ["Sign", "Folk"]), True{},
    Bool.pick(Bool, lanternward.NAME_IN(k, ["Crystal", "Great", "Lever"]), U32.is_eq(ga.u(p, "on"), 0),
      Bool.pick(Bool, String.eq(k, "Brazier"), Bool.not(lanternward.props.Prop.lit(p)), False{})))

# What a chest holds, by its room and its place in it.

def lanternward.props.chest_item?(+room: U32, +id: U32) -> J.Val:
  match room:
    case 0:
      J.JS{"Map"}
    case 1:
      lanternward.props.chest_item.r1(U32.is_eq(id, 0))
    case 2:
      lanternward.variant("Gems", J.JU{20})
    case 3:
      lanternward.props.chest_item.r3(id)
    case _:
      lanternward.variant("Gems", J.JU{10})

def lanternward.props.chest_item.r1?(first: Bool) -> J.Val:
  match first:
    case True{}:
      J.JS{"Lantern"}
    case False{}:
      lanternward.variant("Gems", J.JU{25})

def lanternward.props.chest_item.r3?(+id: U32) -> J.Val:
  match id:
    case 0:
      J.JS{"BossKey"}
    case 1:
      J.JS{"SmallKey"}
    case _:
      lanternward.variant("Gems", J.JU{25})

# The room's things, in the order the plan's rows list them. `acc` carries the
# list and the chest, sign and folk counters.

def lanternward.props.build.id?(+kind: String, +c: U32, +acc: J.Val) -> U32:
  Bool.pick(U32, String.eq(kind, "Gate"), Bool.pick(U32, U32.is_eq(c, 71), 0, (c - 48 : U32)),
    Bool.pick(U32, String.eq(kind, "Chest"), ga.u(acc, "chests"),
      Bool.pick(U32, String.eq(kind, "Sign"), ga.u(acc, "signs"),
        Bool.pick(U32, String.eq(kind, "Folk"), ga.u(acc, "folk"),
          Bool.pick(U32, lanternward.NAME_IN(kind, ["Pot", "Plate"]), Bool.to_u32(Bool.or(U32.is_eq(c, 120), U32.is_eq(c, 89))), 0)))))

def lanternward.props.build.count?(+kind: String, +acc: J.Val) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Chest"), ga.su(acc, "chests", (ga.u(acc, "chests") + 1 : U32)),
    Bool.pick(J.Val, String.eq(kind, "Sign"), ga.su(acc, "signs", (ga.u(acc, "signs") + 1 : U32)),
      Bool.pick(J.Val, String.eq(kind, "Folk"), ga.su(acc, "folk", (ga.u(acc, "folk") + 1 : U32)), acc)))

def lanternward.props.build.cell?(+kind: J.Val, +room: U32, +c: U32, +x: U32, +y: U32, +acc: J.Val) -> J.Val:
  match kind:
    case J.JS{+kn}:
      lanternward.props.build.make(kn, room, c, x, y, lanternward.props.build.id(kn, c, acc), lanternward.props.build.count(kn, acc))
    case _:
      acc

def lanternward.props.build.make?(+kn: String, +room: U32, +c: U32, +x: U32, +y: U32, +id: U32, +acc: J.Val) -> J.Val:
  +p = lanternward.props.Prop.new(kn, x, y, id)
  +on = Bool.pick(U32, Bool.and(String.eq(kn, "Chest"), Bool.and(U32.is_eq(room, 3), U32.is_eq(id, 1))), 2,
    Bool.pick(U32, String.eq(kn, "Rest"), 1, 0))
  ga.push(acc, "out", ga.su(p, "on", on))

def lanternward.props.build.row?(s: String, +room: U32, +x: U32, +y: U32, +acc: J.Val) -> J.Val:
  match s:
    case SNil{}:
      acc
    case SCon{+h, t}:
      lanternward.props.build.row(t, room, (x + 1 : U32), y,
        lanternward.props.build.cell(lanternward.props.Pk.of(Char.to_u32(h)), room, Char.to_u32(h), x, y, acc))

def lanternward.props.build.rows?(+rows: List<&2, String>, +room: U32, +y: U32, +acc: J.Val) -> J.Val:
  match rows:
    case Nil{}:
      acc
    case Con{+h, +t}:
      lanternward.props.build.rows(t, room, (y + 1 : U32), lanternward.props.build.row(h, room, 0, y, acc))

def lanternward.props.build?(+room: U32) -> J.Val:
  J.get(
    lanternward.props.build.rows(lanternward.world.rows(lanternward.world.plan(room)), room, 0,
      ga.sv(ga.su(ga.su(ga.su(J.obj(), "chests", 0), "signs", 0), "folk", 0), "out", J.JA{Nil{}})),
    "out")

# ---- impl Session ----

def lanternward.props.Session.fx?(+s: J.Val, +kind: String, +x: U32, +y: U32, +a: U32) -> J.Val:
  lanternward.props.Session.fx.push(U32.is_lt(lanternward.len(s, "fx"), 80), s, kind, x, y, a)

def lanternward.props.Session.fx.push?(ok: Bool, +s: J.Val, +kind: String, +x: U32, +y: U32, +a: U32) -> J.Val:
  match ok:
    case True{}:
      lanternward.push(s, "fx",
        ga.si(ga.su(ga.si(ga.si(ga.sv(J.obj(), "kind", J.JS{kind}), "x", x), "y", y), "t", 0), "a", a))
    case False{}:
      s

def lanternward.props.Session.drop_at?(+s: J.Val, +kind: String, +x: U32, +y: U32) -> J.Val:
  lanternward.props.Session.drop_at.push(U32.is_lt(lanternward.len(s, "drops"), 32), s, kind, x, y)

def lanternward.props.Session.drop_at.push?(ok: Bool, +s: J.Val, +kind: String, +x: U32, +y: U32) -> J.Val:
  match ok:
    case True{}:
      lanternward.push(s, "drops",
        ga.su(ga.si(ga.si(ga.sv(J.obj(), "kind", J.JS{kind}), "x", x), "y", y), "age", 0))
    case False{}:
      s

# The fixed dice, in two halves: `roll` advances the seed, `rolled` reads the
# value `% n` off the new seed (Rust's `roll(n)` does both).

def lanternward.props.Session.roll?(+s: J.Val) -> J.Val:
  ga.sv(s, "seed", W.to_j(W.add(W.mul(W.of_j(J.get(s, "seed")), W.W{1481765933, 1284865837}), W.W{335903614, 4150755663})))

def lanternward.props.Session.rolled?(+s: J.Val, +n: U32) -> U32:
  W.rem_u32(W.shr(W.of_j(J.get(s, "seed")), 33), RS.u_max(n, 1))

# A pot or a tuft of grass breaks: what it held lies on the floor.

def lanternward.props.Session.shatter?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.props.Session.shatter.go(
    s, i, ga.s(lanternward.el(s, "props", i), "kind"), lanternward.eu(s, "props", i, "id"),
    lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"))

def lanternward.props.Session.shatter.go?(+s: J.Val, +i: U32, +kind: String, +id: U32, +x: U32, +y: U32) -> J.Val:
  +s1 = lanternward.props.Session.fx(lanternward.es(s, "props", i, "on", 1), "Dust", x, y, 0)
  +pot = String.eq(kind, "Pot")
  lanternward.sound(
    lanternward.props.Session.shatter.drops(Bool.and(pot, U32.is_eq(id, 1)), s1, kind, x, y),
    Bool.pick(String, pot, "rock_land", "step_moss"))

def lanternward.props.Session.shatter.drops?(bombs: Bool, +s: J.Val, +kind: String, +x: U32, +y: U32) -> J.Val:
  match bombs:
    case True{}:
      lanternward.props.Session.drop_at(
        lanternward.props.Session.drop_at(
          lanternward.props.Session.drop_at(s, "Bomb", (x - 1280 : U32), (y + 768 : U32)),
          "Bomb", x, (y + 768 : U32)),
        "Bomb", (x + 1280 : U32), (y + 768 : U32))
    case False{}:
      lanternward.props.Session.scatter(s, kind, x, y)

# What a cut or broken thing lets fall.

def lanternward.props.Session.scatter?(+s: J.Val, +kind: String, +x: U32, +y: U32) -> J.Val:
  +s1 = lanternward.props.Session.roll(s)
  lanternward.props.Session.scatter.go(s1, lanternward.props.Session.scatter.pick(kind, lanternward.props.Session.rolled(s1, 100)), x, y)

def lanternward.props.Session.scatter.pick?(+kind: String, +r: U32) -> String:
  Bool.pick(String, String.eq(kind, "Grass"),
    Bool.pick(String, U32.is_le(r, 11), "Gem", Bool.pick(String, U32.is_le(r, 21), "Heart", "")),
    Bool.pick(String, String.eq(kind, "Pot"),
      Bool.pick(String, U32.is_le(r, 17), "Heart", Bool.pick(String, U32.is_le(r, 44), "Gem", Bool.pick(String, U32.is_le(r, 54), "Bomb", ""))),
      Bool.pick(String, U32.is_le(r, 29), "Heart", Bool.pick(String, U32.is_le(r, 64), "Gem", ""))))

def lanternward.props.Session.scatter.go?(+s: J.Val, +drop: String, +x: U32, +y: U32) -> J.Val:
  match drop:
    case SNil{}:
      s
    case _:
      lanternward.props.Session.drop_at(s, drop, x, y)

# Whether any closed thing in the room stops a body at this point.

def lanternward.props.Session.any_blocks?(+ps: List<&2, J.Val>, +x: U32, +y: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(lanternward.props.Prop.blocks(h, x, y), lanternward.props.Session.any_blocks(t, x, y))

def lanternward.props.Session.blocked?(+s: J.Val, +x: U32, +y: U32) -> Bool:
  Bool.or(
    lanternward.world.wall(lanternward.world.Plan.at(ga.u(s, "room"), lanternward.world.tile_of(x), lanternward.world.tile_of(y))),
    lanternward.props.Session.any_blocks(ga.items(s, "props"), x, y))

def lanternward.props.Session.secret_open.any?(+ps: List<&2, J.Val>) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      +k = ga.s(h, "kind")
      Bool.or(
        Bool.and(
          Bool.or(String.eq(k, "Cracked"), Bool.and(String.eq(k, "Gate"), U32.is_eq(ga.u(h, "id"), 2))),
          U32.is_eq(ga.u(h, "on"), 1)),
        lanternward.props.Session.secret_open.any(t))

# Whether the room's secret closet has been opened.

def lanternward.props.Session.secret_open?(+s: J.Val) -> Bool:
  lanternward.props.Session.secret_open.any(ga.items(s, "props"))

# Whether a body of radius `r` (Q pixels) fits at this point.

def lanternward.props.Session.clear?(+s: J.Val, +x: U32, +y: U32, +r: U32) -> Bool:
  Bool.not(Bool.or(
    Bool.or(lanternward.props.Session.blocked(s, (x - r : U32), (y - r : U32)),
      lanternward.props.Session.blocked(s, (x + r : U32), (y - r : U32))),
    Bool.or(lanternward.props.Session.blocked(s, (x - r : U32), (y + r : U32)),
      lanternward.props.Session.blocked(s, (x + r : U32), (y + r : U32)))))

# Whether this point is over a pit.

def lanternward.props.Session.pit?(+s: J.Val, +x: U32, +y: U32) -> Bool:
  U32.is_eq(lanternward.world.Plan.at(ga.u(s, "room"), lanternward.world.tile_of(x), lanternward.world.tile_of(y)), 61)

# Whether any prop that stops iron (not one that lets bolts pass) blocks (x, y).

def lanternward.props.Session.any_stops?(+ps: List<&2, J.Val>, +x: U32, +y: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(
        Bool.and(Bool.not(lanternward.props.Prop.lets_bolts(h)), lanternward.props.Prop.blocks(h, x, y)),
        lanternward.props.Session.any_stops(t, x, y))

# One step `i` of the line: true when nothing stops a bolt there.

def lanternward.props.Session.line_open.step?(+s: J.Val, +ax: U32, +ay: U32, +dx: U32, +dy: U32, +steps: U32, +i: U32, +ta: U32, +tb: U32) -> Bool:
  lanternward.props.Session.line_open.at(
    s, RS.i_add(ax, RS.i_div(RS.i_mul(dx, i), steps)), RS.i_add(ay, RS.i_div(RS.i_mul(dy, i), steps)), ta, tb)

def lanternward.props.Session.line_open.at?(+s: J.Val, +x: U32, +y: U32, +ta: U32, +tb: U32) -> Bool:
  +t = (lanternward.world.tile_of(x) + 65536 * lanternward.world.tile_of(y) : U32)
  Bool.pick(Bool, Bool.or(U32.is_eq(t, ta), U32.is_eq(t, tb)), True{},
    Bool.and(
      U32.is_ne(lanternward.world.Plan.at(ga.u(s, "room"), lanternward.world.tile_of(x), lanternward.world.tile_of(y)), 35),
      Bool.not(lanternward.props.Session.any_stops(ga.items(s, "props"), x, y))))

def lanternward.props.Session.line_open.go?(+s: J.Val, +ax: U32, +ay: U32, +dx: U32, +dy: U32, +steps: U32, +i: U32, +ta: U32, +tb: U32) -> Bool:
  lanternward.props.Session.line_open.next(
    U32.is_lt(i, steps), s, ax, ay, dx, dy, steps, i, ta, tb)

def lanternward.props.Session.line_open.next?(more: Bool, +s: J.Val, +ax: U32, +ay: U32, +dx: U32, +dy: U32, +steps: U32, +i: U32, +ta: U32, +tb: U32) -> Bool:
  match more:
    case False{}:
      True{}
    case True{}:
      lanternward.props.Session.line_open.cont(
        lanternward.props.Session.line_open.step(s, ax, ay, dx, dy, steps, i, ta, tb),
        s, ax, ay, dx, dy, steps, i, ta, tb)

def lanternward.props.Session.line_open.cont?(ok: Bool, +s: J.Val, +ax: U32, +ay: U32, +dx: U32, +dy: U32, +steps: U32, +i: U32, +ta: U32, +tb: U32) -> Bool:
  match ok:
    case False{}:
      False{}
    case True{}:
      lanternward.props.Session.line_open.go(s, ax, ay, dx, dy, steps, (i + 1 : U32), ta, tb)

# Whether the way from `a` to `b` is open to a bolt: only walls stop it.

def lanternward.props.Session.line_open?(+s: J.Val, +ax: U32, +ay: U32, +bx: U32, +by: U32) -> Bool:
  +dx = (bx - ax : U32)
  +dy = (by - ay : U32)
  lanternward.props.Session.line_open.go(
    s, ax, ay, dx, dy,
    RS.i_max(RS.i_div(lanternward.world.dist(dx, dy), 1536), 1), 1,
    (lanternward.world.tile_of(ax) + 65536 * lanternward.world.tile_of(ay) : U32),
    (lanternward.world.tile_of(bx) + 65536 * lanternward.world.tile_of(by) : U32))

# Seat.flaring (kit.rs, a pure read of one seat).

def lanternward.kit.Seat.flaring?(+k: J.Val) -> Bool:
  Bool.and(
    Bool.and(U32.is_ge(ga.u(k, "lamp"), 8), U32.is_gt(ga.u(k, "flame"), 0)),
    Bool.not(lanternward.is(k, "act", "Down")))

def lanternward.props.Session.burning.seats?(+ids: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case Con{+id, +t}:
      Bool.or(
        Bool.and(
          lanternward.kit.Seat.flaring(ga.at(seats, id)),
          RS.i_lt(lanternward.world.dist((ga.u(ga.at(seats, id), "x") - x : U32), (ga.u(ga.at(seats, id), "y") - y : U32)), 19968)),
        lanternward.props.Session.burning.seats(t, seats, x, y))

def lanternward.props.Session.burning.props?(+ps: List<&2, J.Val>, +x: U32, +y: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(
        Bool.and(
          lanternward.props.Prop.lit(h),
          RS.i_lt(lanternward.world.dist((ga.u(h, "x") - x : U32), (ga.u(h, "y") - y : U32)), 16384)),
        lanternward.props.Session.burning.props(t, x, y))

# Whether a point is held lit by a flare or a burning brazier.

def lanternward.props.Session.burning?(+s: J.Val, +x: U32, +y: U32) -> Bool:
  Bool.or(
    lanternward.props.Session.burning.seats(ga.ids(J.get(s, "seats")), J.get(s, "seats"), x, y),
    lanternward.props.Session.burning.props(ga.items(s, "props"), x, y))

# The gate with this number, opened or shut. `$ret` is whether any changed.

def lanternward.props.Session.set_gate.go?(+s: J.Val, +i: U32, +id: U32, +want: U32, +changed: Bool) -> J.Val:
  lanternward.props.Session.set_gate.next(U32.is_lt(i, lanternward.len(s, "props")), s, i, id, want, changed)

def lanternward.props.Session.set_gate.next?(more: Bool, +s: J.Val, +i: U32, +id: U32, +want: U32, +changed: Bool) -> J.Val:
  match more:
    case False{}:
      ga.sb(s, "$ret", changed)
    case True{}:
      lanternward.props.Session.set_gate.at(
        Bool.and(lanternward.eis(s, "props", i, "kind", "Gate"), U32.is_eq(lanternward.eu(s, "props", i, "id"), id)),
        s, i, id, want, changed)

def lanternward.props.Session.set_gate.at?(hit: Bool, +s: J.Val, +i: U32, +id: U32, +want: U32, +changed: Bool) -> J.Val:
  match hit:
    case False{}:
      lanternward.props.Session.set_gate.go(s, (i + 1 : U32), id, want, changed)
    case True{}:
      lanternward.props.Session.set_gate.set(U32.is_ne(lanternward.eu(s, "props", i, "on"), want), s, i, id, want, changed)

def lanternward.props.Session.set_gate.set?(diff: Bool, +s: J.Val, +i: U32, +id: U32, +want: U32, +changed: Bool) -> J.Val:
  match diff:
    case True{}:
      lanternward.props.Session.set_gate.go(lanternward.es(s, "props", i, "on", want), (i + 1 : U32), id, want, True{})
    case False{}:
      lanternward.props.Session.set_gate.go(s, (i + 1 : U32), id, want, changed)

def lanternward.props.Session.set_gate?(+s: J.Val, +id: U32, +open: Bool) -> J.Val:
  lanternward.props.Session.set_gate.go(s, 0, id, Bool.to_u32(open), False{})

# A puzzle came right: the chime, the star burst, a cue on the screen.

def lanternward.props.Session.solved?(+s: J.Val, +x: U32, +y: U32) -> J.Val:
  lanternward.sound(
    lanternward.props.Session.solved.cue(J.is_null(J.get(s, "cue")),
      lanternward.props.Session.fx(s, "Solved", x, y, 0), x, y),
    "wish_grant")

def lanternward.props.Session.solved.cue?(none: Bool, +s: J.Val, +x: U32, +y: U32) -> J.Val:
  match none:
    case True{}:
      ga.sv(s, "cue", J.JA{[J.of_i32(x), J.of_i32(y), J.JU{44}]})
    case False{}:
      s

def lanternward.run.sound?(+run: J.Val, +name: String) -> J.Val:
  J.set(run, "sounds", J.push(J.get(run, "sounds"), J.JS{name}))

def lanternward.run.found?(+run: J.Val, +text: String) -> J.Val:
  J.set(run, "found", J.JA{[J.get(run, "tick"), J.JU{0}, J.JS{text}]})

# The Score of slot 0 in `home.countryside.games.roundtable` (default when the
# array is omitted).

def lanternward.run.score?(+run: J.Val) -> J.Val:
  lanternward.run.score.of(J.nth(ga.v(ga.v(ga.v(ga.v(run, "home"), "countryside"), "games"), "roundtable"), 0))

def lanternward.run.score.of?(+sc: J.Val) -> J.Val:
  match sc:
    case J.JNull{}:
      ga.su(ga.su(ga.su(J.obj(), "best", 0), "wins", 0), "plays", 0)
    case _:
      sc

def lanternward.run.score_set?(+run: J.Val, sc: J.Val) -> J.Val:
  J.put(run, ["home", "countryside", "games", "roundtable"],
    J.set_nth(lanternward.run.roundtable(J.get(J.get(J.get(run, "home"), "countryside"), "games")), 0, sc))

# The `[Score; 5]`, filled with defaults when serde omitted it.

def lanternward.run.roundtable?(+games: J.Val) -> J.Val:
  lanternward.run.roundtable.of(J.get(games, "roundtable"))

def lanternward.run.roundtable.of?(+a: J.Val) -> J.Val:
  match a:
    case J.JNull{}:
      J.JA{[lanternward.run.score.of(J.JNull{}), lanternward.run.score.of(J.JNull{}), lanternward.run.score.of(J.JNull{}), lanternward.run.score.of(J.JNull{}), lanternward.run.score.of(J.JNull{})]}
    case _:
      a

# `Run::lanternward_showing`.

def lanternward.Run.lanternward_showing?(+run: J.Val) -> Bool:
  Bool.and(lanternward.tag_is(ga.v(rules.Run.room(run), "kind"), "Forest"), Bool.not(J.is_null(J.get(run, "lanternward"))))

# `Run::start_lanternward`: the table sits down and the play is counted.

def lanternward.Run.start_lanternward?(+run: J.Val) -> J.Val:
  lanternward.Run.start_lanternward.go(lanternward.tag_is(ga.v(rules.Run.room(run), "kind"), "Forest"), run)

def lanternward.Run.start_lanternward.go?(forest: Bool, +run: J.Val) -> J.Val:
  match forest:
    case False{}:
      run
    case True{}:
      lanternward.Run.start_lanternward.sit(J.set(run, "forest", forest.Forest.leave(J.get(run, "forest"))))

def lanternward.Run.start_lanternward.sit?(+run: J.Val) -> J.Val:
  lanternward.Run.start_lanternward.count(J.set(run, "lanternward", lanternward.Session.new(J.get(run, "players"))))

def lanternward.Run.start_lanternward.count?(+run: J.Val) -> J.Val:
  lanternward.run.sound(
    lanternward.run.found(
      lanternward.run.score_set(run, ga.su(lanternward.run.score(run), "plays", RS.u_sat_add(ga.u(lanternward.run.score(run), "plays"), 1))),
      "Lanternward: R locks on. Hold Esc to leave."),
    "play_card")

# The players at the table: those not outside the room.

def lanternward.run.present?(+ids: List<&2, U32>, +players: J.Val, +outside: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      J.obj()
    case Con{+id, +t}:
      Bool.pick(J.Val, ga.has_key(outside, id),
        lanternward.run.present(t, players, outside),
        ga.put_at(lanternward.run.present(t, players, outside), id, ga.at(players, id)))

# Sounds from the tick, then the weight of impact for the engine's hit-stop.

def lanternward.run.sounds?(+run: J.Val, +xs: List<&2, J.Val>) -> J.Val:
  match xs:
    case Nil{}:
      run
    case Con{+h, +t}:
      lanternward.run.sounds(lanternward.run.sound(run, J.str(h)), t)

def lanternward.run.stop?(+run: J.Val, +stop: J.Val) -> J.Val:
  match stop:
    case J.JNull{}:
      run
    case _:
      J.set(run, "hitstop", hitstop.State.request(J.get(run, "hitstop"), J.get(run, "tune"),
        Bool.pick(String, lanternward.tag_is(stop, "Heavy"), "Heavy",
          Bool.pick(String, lanternward.tag_is(stop, "Kill"), "EliteKill",
            Bool.pick(String, lanternward.tag_is(stop, "Phase"), "Phase", "BossKill")))))

# `Run::tick_lanternward(inputs)`: step the session; if the Beacon was relit,
# pay the table; whoever pressed E in the finale leaves.

def lanternward.Run.tick_lanternward?(+run: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Run.tick_lanternward.go(J.is_null(J.get(run, "lanternward")), run, inputs)

def lanternward.Run.tick_lanternward.go?(none: Bool, +run: J.Val, +inputs: J.Val) -> J.Val:
  match none:
    case True{}:
      run
    case False{}:
      lanternward.Run.tick_lanternward.step(
        lanternward.Session.step(
          J.get(run, "lanternward"), inputs,
          lanternward.run.present(ga.ids(J.get(run, "players")), J.get(run, "players"), ga.v(ga.v(run, "exits"), "outside")),
          lanternward.Rules.of(J.get(run, "tune"))),
        J.del(run, "lanternward"), inputs)

def lanternward.Run.tick_lanternward.step?(+s: J.Val, +run: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Run.tick_lanternward.won(
    ga.b(J.get(s, "$ev"), "won"),
    lanternward.run.stop(
      lanternward.run.sounds(run, ga.items(J.get(s, "$ev"), "sounds")),
      ga.v(J.get(s, "$ev"), "stop")),
    lanternward.Session.bare(s), inputs)

def lanternward.Run.tick_lanternward.won?(won: Bool, +run: J.Val, +s: J.Val, +inputs: J.Val) -> J.Val:
  match won:
    case False{}:
      lanternward.Run.tick_lanternward.leave(run, s, inputs)
    case True{}:
      lanternward.Run.tick_lanternward.finish(lanternward.Run.finish_lanternward(run, s), inputs)

# `finish_lanternward` leaves the paid session in `$s`; put it back and let
# those who asked to leave go.

def lanternward.Run.tick_lanternward.finish?(+run: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Run.tick_lanternward.leave(J.del(run, "$s"), J.get(run, "$s"), inputs)

def lanternward.Run.tick_lanternward.leave?(+run: J.Val, +s: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.run.leaving(
    lanternward.run.leavers(ga.ids(inputs), inputs, s), J.set(run, "lanternward", s))

def lanternward.run.leavers?(+ids: List<&2, U32>, +inputs: J.Val, +s: J.Val) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      Bool.pick(List<&2, U32>,
        Bool.and(
          Bool.and(rules.Input.valid(ga.at(inputs, id)), ga.b(ga.at(inputs, id), "interact")),
          Bool.and(lanternward.Session.finale(s), ga.has_key(J.get(s, "seats"), id))),
        Con{id, lanternward.run.leavers(t, inputs, s)},
        lanternward.run.leavers(t, inputs, s))

def lanternward.run.leaving?(+ids: List<&2, U32>, +run: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      run
    case Con{+id, +t}:
      lanternward.run.leaving(t, J.del(exits.Run.leave_activity(run, id), "$ret"))

# The Beacon burns: the campaign is marked, each knight at the table is paid
# once, and a gold crown goes to every one of them. The session (with `paid`
# updated) is returned in `$s`.

def lanternward.Run.finish_lanternward?(+run: J.Val, +s: J.Val) -> J.Val:
  lanternward.run.found(
    lanternward.run.sound(
      lanternward.run.pay(
        ga.ids(J.get(s, "seats")),
        lanternward.run.record(
          J.put(run, ["marks", "lanternward:campaign-begun"], J.JU{1}),
          ga.u(s, "elapsed"),
          Bool.and(
            U32.is_eq(U32.and(ga.nu(ga.v(ga.v(run, "home"), "roundtable"), 0), 1), 0),
            Bool.not(J.has(ga.v(run, "marks"), "lanternward:campaign-begun")))),
        s, Bool.and(
          U32.is_eq(U32.and(ga.nu(ga.v(ga.v(run, "home"), "roundtable"), 0), 1), 0),
          Bool.not(J.has(ga.v(run, "marks"), "lanternward:campaign-begun")))),
      "spoil_pickup"),
    "Lanternward: the Beacon burns again.")

# wins +1 (never above plays); best = the better of it and the rating.

def lanternward.run.record?(+run: J.Val, +elapsed: U32, +first: Bool) -> J.Val:
  lanternward.run.record.go(run, U32.div(elapsed, 30))

def lanternward.run.record.go?(+run: J.Val, +secs: U32) -> J.Val:
  lanternward.run.record.set(
    lanternward.run.score(run),
    RS.u_clamp(RS.u_sat_sub(1000, (RS.u_sat_sub(secs, 180) * 2 : U32)), 1, 1000), run)

def lanternward.run.record.set?(+sc: J.Val, +rating: U32, +run: J.Val) -> J.Val:
  lanternward.run.score_set(run,
    ga.su(ga.su(sc, "wins", RS.u_min(RS.u_sat_add(ga.u(sc, "wins"), 1), ga.u(sc, "plays"))), "best", RS.u_max(ga.u(sc, "best"), rating)))

# Each knight at the table not yet paid is paid: gold, a laurel, no booby prize.

def lanternward.run.pay?(+ids: List<&2, U32>, +run: J.Val, +s: J.Val, +first: Bool) -> J.Val:
  match ids:
    case Nil{}:
      ga.sv(run, "$s", s)
    case Con{+id, +t}:
      lanternward.run.pay.one(U32.is_ne(U32.and(ga.u(s, "paid"), U32.shln(1, U32.to_nat(id))), 0), t, run, s, id, first)

def lanternward.run.pay.one?(paid: Bool, +t: List<&2, U32>, +run: J.Val, +s: J.Val, +id: U32, +first: Bool) -> J.Val:
  match paid:
    case True{}:
      lanternward.run.pay(t, run, s, first)
    case False{}:
      lanternward.run.pay(t,
        lanternward.run.pay.hero(ga.has_key(ga.v(run, "players"), id), run, id, lanternward.purse(first, ga.u(ga.at(J.get(s, "seats"), id), "gems"))),
        ga.su(s, "paid", U32.or(ga.u(s, "paid"), U32.shln(1, U32.to_nat(id)))), first)

def lanternward.run.pay.hero?(has: Bool, +run: J.Val, +id: U32, +gold: U32) -> J.Val:
  match has:
    case False{}:
      run
    case True{}:
      J.set(run, "players", ga.put_at(J.get(run, "players"), id,
        ga.sb(
          ga.sv(ga.sv(ga.at(J.get(run, "players"), id), "carried", realm.Spoils.add(ga.v(ga.at(J.get(run, "players"), id), "carried"), "gold", gold)), "booby", J.JNull{}),
          "laurel", True{})))

# ---- ADVENTURE ----

def lanternward.ADVENTURE.valid?(+run: J.Val) -> Bool:
  lanternward.ADVENTURE.valid.go(J.get(run, "lanternward"))

def lanternward.ADVENTURE.valid.go?(+s: J.Val) -> Bool:
  match s:
    case J.JNull{}:
      True{}
    case _:
      lanternward.Session.valid(s)

def lanternward.ADVENTURE.clear?(+run: J.Val) -> J.Val:
  J.del(run, "lanternward")

def lanternward.ADVENTURE.seated?(+run: J.Val) -> J.Val:
  lanternward.ADVENTURE.seated.go(J.get(run, "lanternward"))

def lanternward.ADVENTURE.seated.go?(+s: J.Val) -> J.Val:
  match s:
    case J.JNull{}:
      J.JA{Nil{}}
    case _:
      J.JA{lanternward.ids_j(ga.ids(J.get(s, "seats")))}

def lanternward.ids_j?(+ids: List<&2, U32>) -> List<&2, J.Val>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{J.JU{h}, lanternward.ids_j(t)}

# The scripted bot's keys for seat `id`.

def lanternward.ADVENTURE.bot?(+run: J.Val, +id: U32) -> J.Val:
  lanternward.ADVENTURE.bot.go(
    Bool.and(Bool.not(J.is_null(J.get(run, "lanternward"))), ga.has_key(J.get(run, "players"), id)), run, id)

def lanternward.ADVENTURE.bot.go?(both: Bool, +run: J.Val, +id: U32) -> J.Val:
  match both:
    case False{}:
      rules.Input.default()
    case True{}:
      lanternward.bot.Session.bot_input(J.get(run, "lanternward"), id, lanternward.Rules.of(J.get(run, "tune")))

# ---- ACTIVITY ----

def lanternward.ACTIVITY.on?(+run: J.Val) -> Bool:
  lanternward.Run.lanternward_showing(run)

def lanternward.ACTIVITY.ride?(+run: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Run.tick_lanternward(run, inputs)

def lanternward.ACTIVITY.end?(+run: J.Val) -> J.Val:
  J.del(run, "lanternward")

# ---- settle ----

# `settle(realm, marks)`: a finished run's mark becomes a flag in the Home.
# In: {home, marks}; out: {home, changed, said}.

def lanternward.settle.flags?(+home: J.Val) -> J.Val:
  lanternward.settle.flags.of(J.get(home, "roundtable"))

def lanternward.settle.flags.of?(+a: J.Val) -> J.Val:
  match a:
    case J.JNull{}:
      J.JA{[J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}]}
    case _:
      a

def lanternward.settle?(+realm: J.Val, +marks: J.Val) -> J.Val:
  lanternward.settle.go(
    Bool.and(
      J.has(marks, "lanternward:campaign-begun"),
      U32.is_eq(U32.and(ga.nu(lanternward.settle.flags(ga.v(realm, "home")), 0), 1), 0)),
    realm)

def lanternward.settle.go?(changed: Bool, +realm: J.Val) -> J.Val:
  match changed:
    case False{}:
      ga.sv(ga.sb(J.obj(), "changed", False{}), "said", J.JA{Nil{}})
    case True{}:
      ga.sv(
        ga.sv(
          ga.sb(J.obj(), "changed", True{}), "said",
          J.JA{[ga.pair(J.JS{"lanternward"}, J.JS{"Lanternward: the Beacon burns again."})]}),
        "home",
        ga.sv(ga.v(realm, "home"), "roundtable",
          J.set_nth(lanternward.settle.flags(ga.v(realm, "home")), 0, J.JU{U32.or(ga.nu(lanternward.settle.flags(ga.v(realm, "home")), 0), 1)})))

def lanternward.LIMIT?() -> U32:
  108000

def lanternward.HEART?() -> U32:
  4

def lanternward.START_HP?() -> U32:
  20

def lanternward.SPAWN_INV?() -> U32:
  60

def lanternward.REVIVE_HP?() -> U32:
  12

def lanternward.KNIGHT_R?() -> U32:
  1280

def lanternward.WALK?() -> U32:
  440

def lanternward.PURSE_FIRST?() -> U32:
  40

def lanternward.PURSE_AGAIN?() -> U32:
  12

def lanternward.PURSE_GEMS?() -> U32:
  20

def lanternward.BEGUN?() -> U32:
  1

def lanternward.SLOT?() -> U32:
  0

def lanternward.NAME?() -> String:
  "Lanternward"

def lanternward.CAMPAIGN_MARK?() -> String:
  "lanternward:campaign-begun"

# The five numbers as the rules read them: `Rules::default()`.

def lanternward.Rules.default?() -> J.Val:
  ga.su(ga.su(ga.su(ga.si(J.obj(), "lock", 28672), "parry", 8), "combo", 14), "flame", 150)

def lanternward.Rules.default.full?() -> J.Val:
  ga.su(lanternward.Rules.default(), "tell", 56)

# `Rules::of(tune)`: the Tune's numbers for this slot (tell never under the floor).

def lanternward.Rules.of?(+tune: J.Val) -> J.Val:
  ga.su(
    ga.su(
      ga.su(
        ga.su(
          ga.si(J.obj(), "lock", (roundtable.number(tune, 0, 0) * 256 : U32)),
          "parry", roundtable.number(tune, 0, 1)),
        "combo", roundtable.number(tune, 0, 2)),
      "flame", roundtable.number(tune, 0, 3)),
    "tell", RS.u_max(roundtable.number(tune, 0, 4), 40))

# What one knight is paid at the Beacon.

def lanternward.purse?(+first: Bool, +gems: U32) -> U32:
  (Bool.pick(U32, first, 40, 12) + RS.u_min(gems, 20) : U32)

# ---- new ----

# `Session::new(&players)`: every player not away or in stone sits down, then
# room 0 is entered.

def lanternward.Session.new?(+players: J.Val) -> J.Val:
  lanternward.Session.new.sit(ga.ids(players), players, lanternward.Session.new.empty())

def lanternward.Session.new.empty?() -> J.Val:
  lanternward.Session.new.fields(ga.sv(J.obj(), "seats", J.obj()))

def lanternward.Session.new.fields?(+s: J.Val) -> J.Val:
  lanternward.Session.new.fields2(
    ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(s,
      "elapsed", 0), "room", 0), "stage", 0), "stage_t", 0), "shake", 0), "flash", 0),
      "props", J.JA{Nil{}}), "foes", J.JA{Nil{}}), "bolts", J.JA{Nil{}}), "drops", J.JA{Nil{}}), "fx", J.JA{Nil{}}), "fuses", J.JA{Nil{}}))

def lanternward.Session.new.fields2?(+s: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(ga.sv(ga.su(ga.su(ga.su(ga.su(ga.sv(ga.sv(ga.sv(ga.sv(s,
    "boss", J.JNull{}), "inv", lanternward.ctx.Party.new()), "banner", J.JNull{}), "seed", W.to_j(W.W{1279937108, 1163152983})),
    "exiting", 0), "wipe", 0), "tries", 0), "next_foe", 0),
    "cue", J.JNull{}), "done", J.JNull{}), "paid", J.JU{0}), "kept", J.obj())

def lanternward.Session.new.sit?(+ids: List<&2, U32>, +players: J.Val, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      lanternward.Session.bare(
        lanternward.stage.Session.enter_room(ga.sv(s, "$r", lanternward.Rules.default.full()), 0))
    case Con{+id, +t}:
      lanternward.Session.new.sit(t, players,
        lanternward.Session.new.one(
          Bool.and(Bool.not(ga.b(ga.at(players, id), "away")), Bool.not(ga.b(ga.at(players, id), "stone"))), s, id))

def lanternward.Session.new.one?(here: Bool, +s: J.Val, +id: U32) -> J.Val:
  match here:
    case False{}:
      s
    case True{}:
      lanternward.Session.new.seat(lanternward.stage.Session.start_of(s, id), s, id)

def lanternward.Session.new.seat?(+at: J.Val, +s: J.Val, +id: U32) -> J.Val:
  lanternward.blow.put_seat(s, id, lanternward.kit.Seat.new(id, 0, ga.nu(at, 0), ga.nu(at, 1)))

# Strip the working keys: the session as serde writes it.

def lanternward.Session.bare?(+s: J.Val) -> J.Val:
  J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(s,
    "$ev"), "$r"), "$ret"), "$k"), "$f"), "$w"), "$strikes"), "$shots"), "$landed"), "$cut"), "$parried"), "$weights"), "$flares"), "$revealed"), "$b")

def lanternward.Session.events?(+s: J.Val) -> J.Val:
  J.get(s, "$ev")

def lanternward.Session.out?(+s: J.Val) -> J.Val:
  lanternward.Session.bare(s)

# ---- step ----

# `here(id)`: the hero is at the table (not away, not in stone).

def lanternward.Session.here?(+heroes: J.Val, +id: U32) -> Bool:
  Bool.and(
    ga.has_key(heroes, id),
    Bool.and(Bool.not(ga.b(ga.at(heroes, id), "away")), Bool.not(ga.b(ga.at(heroes, id), "stone"))))

# A knight who left stands up and keeps their gems and bombs.

def lanternward.Session.step.stand?(+ids: List<&2, U32>, +s: J.Val, +heroes: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.Session.step.stand(t,
        lanternward.Session.step.stand.one(lanternward.Session.here(heroes, id), s, id), heroes)

def lanternward.Session.step.stand.one?(here: Bool, +s: J.Val, +id: U32) -> J.Val:
  match here:
    case True{}:
      s
    case False{}:
      ga.sv(
        ga.sv(s, "kept", ga.put_at(J.get(s, "kept"), id,
          ga.pair(J.get(ga.at(J.get(s, "seats"), id), "gems"), J.get(ga.at(J.get(s, "seats"), id), "bombs")))),
        "seats", J.del(J.get(s, "seats"), U32.show(id)))

# One who joins late sits down at the room's start and scales what is standing.

def lanternward.Session.step.sit?(+ids: List<&2, U32>, +s: J.Val, +heroes: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.Session.step.sit(t,
        lanternward.Session.step.sit.one(
          Bool.and(lanternward.Session.here(heroes, id), Bool.not(ga.has_key(J.get(s, "seats"), id))), s, id), heroes)

def lanternward.Session.step.sit.one?(join: Bool, +s: J.Val, +id: U32) -> J.Val:
  match join:
    case False{}:
      s
    case True{}:
      lanternward.Session.step.sit.seat(lanternward.stage.Session.start_of(s, id), s, id)

def lanternward.Session.step.sit.seat?(+at: J.Val, +s: J.Val, +id: U32) -> J.Val:
  lanternward.Session.step.sit.kept(
    ga.at(J.get(s, "kept"), id), ga.has_key(J.get(s, "kept"), id),
    lanternward.kit.Seat.new(id, ga.u(s, "elapsed"), ga.nu(at, 0), ga.nu(at, 1)), s, id)

def lanternward.Session.step.sit.kept?(+kept: J.Val, +has: Bool, +k: J.Val, +s: J.Val, +id: U32) -> J.Val:
  lanternward.Session.step.sit.hp(
    Bool.pick(J.Val, has, ga.su(ga.su(k, "gems", ga.nu(kept, 0)), "bombs", ga.nu(kept, 1)), k),
    Bool.pick(J.Val, has, ga.sv(s, "kept", J.del(J.get(s, "kept"), U32.show(id))), s), id)

def lanternward.Session.step.sit.hp?(+k: J.Val, +s: J.Val, +id: U32) -> J.Val:
  +max = (20 + 4 * ga.u(ga.v(s, "inv"), "containers") : U32)
  lanternward.blow.put_seat(s, id,
    ga.su(
      Bool.pick(J.Val, ga.b(ga.v(s, "inv"), "lantern"), ga.su(ga.su(ga.su(k, "max_hp", max), "hp", max), "flame", lanternward.R(s, "flame")), ga.su(ga.su(k, "max_hp", max), "hp", max)),
      "max_hp", max))

# Each seated knight takes their input (a bad one is no input).

def lanternward.Session.step.seats?(+ids: List<&2, U32>, +s: J.Val, +inputs: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.Session.step.seats(t, lanternward.Session.step.seat(s, id, ga.at(inputs, id)), inputs)

def lanternward.Session.step.seat?(+s: J.Val, +id: U32, +inp: J.Val) -> J.Val:
  lanternward.Session.step.seat.valid(
    Bool.and(Bool.not(J.is_null(inp)), rules.Input.valid(inp)), s, id, inp)

def lanternward.Session.step.seat.valid?(ok: Bool, +s: J.Val, +id: U32, +inp: J.Val) -> J.Val:
  match ok:
    case True{}:
      lanternward.kit.Session.step_seat(s, id, inp)
    case False{}:
      lanternward.kit.Session.step_seat(s, id, rules.Input.default())

# One tick: whoever is here and awake sits at the table, then every body acts
# in its order. `r` is the Rules ({lock, parry, combo, flame, tell}).

def lanternward.Session.step?(+s: J.Val, +inputs: J.Val, +heroes: J.Val, +r: J.Val) -> J.Val:
  lanternward.Session.step.go(
    lanternward.Session.step.sit(
      ga.ids(heroes),
      lanternward.Session.step.stand(
        ga.ids(J.get(s, "seats")),
        ga.su(ga.sv(ga.sv(s, "$r", r), "$ev", lanternward.Events.default()), "elapsed", RS.u_min((ga.u(s, "elapsed") + 1 : U32), 108000)),
        heroes),
      heroes),
    inputs)

def lanternward.Session.step.go?(+s: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Session.step.go2(lanternward.foes.Session.provision(s), inputs)

def lanternward.Session.step.go2?(+s: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Session.step.rest(lanternward.Session.step.seats(ga.ids(J.get(s, "seats")), s, inputs))

def lanternward.Session.step.rest?(+s: J.Val) -> J.Val:
  lanternward.stage.Session.age(
    lanternward.stage.Session.tick_wipe(
      lanternward.stage.Session.tick_stage(
        lanternward.stage.Session.revive_pass(
          lanternward.stage.Session.pickups(
            lanternward.blow.Session.step_fuses(
              lanternward.blow.Session.step_bolts(
                lanternward.boss.Session.step_boss(
                  lanternward.foes.Session.step_foes(
                    lanternward.stage.Session.unstick(
                      lanternward.stage.Session.tick_props(s)))))))))))

# Whether the finale is on: the Beacon is lit and the party may leave.

def lanternward.Session.finale?(+s: J.Val) -> Bool:
  lanternward.Session.finale.go(J.is_null(J.get(s, "done")), s)

def lanternward.Session.finale.go?(none: Bool, +s: J.Val) -> Bool:
  match none:
    case True{}:
      False{}
    case False{}:
      U32.is_ge(ga.u(s, "elapsed"), (J.u32(J.get(s, "done")) + 120 : U32))

# ---- valid / problem / repair ----

def lanternward.problem.inside?(+x: U32, +y: U32, +w: U32, +h: U32) -> Bool:
  Bool.and(lanternward.in_range(x, 0, w), lanternward.in_range(y, 0, h))

def lanternward.problem.knights?(+ids: List<&2, U32>, +seats: J.Val, +elapsed: U32, +w: U32, +h: U32) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+id, +t}:
      Bool.and(
        Bool.and(U32.is_ge(id, 1), U32.is_le(id, 6)),
        Bool.and(lanternward.kit.Seat.valid(ga.at(seats, id), id, elapsed, w, h), lanternward.problem.knights(t, seats, elapsed, w, h)))

def lanternward.problem.props?(+ps: List<&2, J.Val>, +pw: U32, +ph: U32, +w: U32, +h: U32) -> Bool:
  match ps:
    case Nil{}:
      True{}
    case Con{+q, +t}:
      Bool.and(
        Bool.and(Bool.and(RS.i_lt(ga.u(q, "tx"), pw), RS.i_lt(ga.u(q, "ty"), ph)), U32.is_le(ga.u(q, "on"), 4)),
        Bool.and(lanternward.problem.inside(ga.u(q, "x"), ga.u(q, "y"), w, h), lanternward.problem.props(t, pw, ph, w, h)))

def lanternward.problem.foes?(+fs: List<&2, J.Val>, +w: U32, +h: U32) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{+f, +t}:
      Bool.and(
        Bool.and(
          Bool.and(U32.is_le(ga.u(f, "hp"), ga.u(f, "full")), U32.is_le(ga.u(f, "full"), 48)),
          Bool.and(Bool.and(U32.is_ge(ga.u(f, "prov"), 1), U32.is_le(ga.u(f, "prov"), 6)), lanternward.problem.inside(ga.u(f, "x"), ga.u(f, "y"), w, h))),
        Bool.and(
          Bool.and(U32.is_le(ga.u(f, "t"), 600), U32.is_le(ga.u(f, "dead"), 14)),
          Bool.and(RS.i_le(lanternward.world.dist(ga.u(f, "fx"), ga.u(f, "fy")), 264), lanternward.problem.foes(t, w, h))))

def lanternward.problem.bolts?(+bs: List<&2, J.Val>, +w: U32, +h: U32) -> Bool:
  match bs:
    case Nil{}:
      True{}
    case Con{+b, +t}:
      Bool.and(
        Bool.and(
          lanternward.problem.inside(ga.u(b, "x"), ga.u(b, "y"), w, h),
          Bool.and(lanternward.in_range(ga.u(b, "vx"), 4294965248, 2048), lanternward.in_range(ga.u(b, "vy"), 4294965248, 2048))),
        Bool.and(Bool.and(U32.is_le(ga.u(b, "ttl"), 400), U32.is_le(ga.u(b, "owner"), 6)), lanternward.problem.bolts(t, w, h)))

def lanternward.problem.drops?(+ds: List<&2, J.Val>, +w: U32, +h: U32) -> Bool:
  match ds:
    case Nil{}:
      True{}
    case Con{+d, +t}:
      Bool.and(lanternward.problem.inside(ga.u(d, "x"), ga.u(d, "y"), w, h), lanternward.problem.drops(t, w, h))

def lanternward.problem.fuses?(+fs: List<&2, J.Val>, +w: U32, +h: U32) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{+f, +t}:
      Bool.and(
        Bool.and(lanternward.problem.inside(ga.u(f, "x"), ga.u(f, "y"), w, h), U32.is_le(ga.u(f, "t"), 75)),
        lanternward.problem.fuses(t, w, h))

def lanternward.problem.kept?(+ids: List<&2, U32>, +kept: J.Val) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+id, +t}:
      Bool.and(
        Bool.and(Bool.and(U32.is_ge(id, 1), U32.is_le(id, 6)), Bool.and(U32.is_le(ga.nu(ga.at(kept, id), 0), 100000), U32.is_le(ga.nu(ga.at(kept, id), 1), 10))),
        lanternward.problem.kept(t, kept))

# The first named bound the session breaks, else null.

def lanternward.Session.problem?(+s: J.Val) -> J.Val:
  lanternward.problem.go(s, lanternward.world.Plan.w(lanternward.world.plan(ga.u(s, "room"))), lanternward.world.Plan.h(lanternward.world.plan(ga.u(s, "room"))))

def lanternward.problem.go?(+s: J.Val, +pw: U32, +ph: U32) -> J.Val:
  lanternward.problem.checks(s, pw, ph, (pw * 4096 : U32), (ph * 4096 : U32))

def lanternward.problem.pick?(ok: Bool, +name: String, +next: J.Val) -> J.Val:
  Bool.pick(J.Val, ok, next, J.JS{name})

def lanternward.problem.checks?(+s: J.Val, +pw: U32, +ph: U32, +w: U32, +h: U32) -> J.Val:
  lanternward.problem.pick(
    Bool.and(U32.is_le(ga.u(s, "elapsed"), 108000), U32.is_le(ga.u(s, "stage_t"), 108000)), "clock",
    lanternward.problem.pick(
      Bool.and(U32.is_lt(ga.u(s, "room"), 5), U32.is_le(ga.u(s, "stage"), 6)), "room",
      lanternward.problem.pick(
        U32.is_le(lanternward.seat_count(s), 6), "seats",
        lanternward.problem.pick(
          lanternward.problem.knights(ga.ids(J.get(s, "seats")), J.get(s, "seats"), ga.u(s, "elapsed"), w, h), "knight",
          lanternward.problem.pick(
            U32.is_le(lanternward.len(s, "props"), 160), "props",
            lanternward.problem.pick(
              lanternward.problem.props(ga.items(s, "props"), pw, ph, w, h), "prop",
              lanternward.problem.pick(
                U32.is_le(lanternward.len(s, "foes"), 16), "foes",
                lanternward.problem.pick(
                  lanternward.problem.foes(ga.items(s, "foes"), w, h), "foe",
                  lanternward.problem.pick(
                    U32.is_le(lanternward.len(s, "bolts"), 40), "bolts",
                    lanternward.problem.pick(
                      lanternward.problem.bolts(ga.items(s, "bolts"), w, h), "bolt",
                      lanternward.problem.checks2(s, w, h)))))))))))

def lanternward.problem.checks2?(+s: J.Val, +w: U32, +h: U32) -> J.Val:
  lanternward.problem.pick(
    Bool.and(U32.is_le(lanternward.len(s, "drops"), 32), lanternward.problem.drops(ga.items(s, "drops"), w, h)), "drops",
    lanternward.problem.pick(
      U32.is_le(lanternward.len(s, "fx"), 80), "fx",
      lanternward.problem.pick(
        Bool.and(U32.is_le(lanternward.len(s, "fuses"), 8), lanternward.problem.fuses(ga.items(s, "fuses"), w, h)), "fuses",
        lanternward.problem.pick(
          lanternward.problem.boss_ok(J.get(s, "boss"), ga.u(s, "room"), w, h), "boss",
          lanternward.problem.pick(
            Bool.and(U32.is_le(ga.u(ga.v(s, "inv"), "small_keys"), 8), U32.is_le(ga.u(ga.v(s, "inv"), "containers"), 8)), "keys",
            lanternward.problem.pick(
              Bool.and(U32.is_le(ga.u(s, "shake"), 64), U32.is_le(ga.u(s, "flash"), 64)), "shake",
              lanternward.problem.pick(
                Bool.and(U32.is_le(ga.u(s, "exiting"), 14), U32.is_le(ga.u(s, "wipe"), 24)), "exit",
                lanternward.problem.checks3(s, w, h))))))))

def lanternward.problem.boss_ok?(+b: J.Val, +room: U32, +w: U32, +h: U32) -> Bool:
  match b:
    case J.JNull{}:
      True{}
    case _:
      Bool.and(
        Bool.and(U32.is_eq(room, 4), lanternward.boss.Warden.valid(b)),
        lanternward.problem.inside(ga.u(b, "x"), ga.u(b, "y"), w, h))

def lanternward.problem.cue_ok?(+c: J.Val, +w: U32, +h: U32) -> Bool:
  match c:
    case J.JNull{}:
      True{}
    case _:
      Bool.and(lanternward.problem.inside(ga.nu(c, 0), ga.nu(c, 1), w, h), U32.is_le(ga.nu(c, 2), 44))

def lanternward.problem.checks3?(+s: J.Val, +w: U32, +h: U32) -> J.Val:
  lanternward.problem.pick(
    lanternward.problem.cue_ok(J.get(s, "cue"), w, h), "cue",
    lanternward.problem.pick(
      Bool.and(U32.is_eq(U32.and(ga.u(s, "paid"), 1), 0), U32.is_lt(ga.u(s, "paid"), 128)), "paid",
      lanternward.problem.pick(
        lanternward.problem.done_ok(J.get(s, "done"), ga.u(s, "elapsed")), "done",
        lanternward.problem.pick(
          lanternward.problem.banner_ok(J.get(s, "banner")), "banner",
          lanternward.problem.pick(
            U32.is_lt(ga.u(s, "tries"), 1000000), "tries",
            lanternward.problem.pick(
              Bool.and(
                U32.is_le(lanternward.fields_count(J.get(s, "kept")), 6),
                lanternward.problem.kept(ga.ids(J.get(s, "kept")), J.get(s, "kept"))), "kept",
              J.JNull{}))))))

def lanternward.problem.done_ok?(+d: J.Val, +elapsed: U32) -> Bool:
  match d:
    case J.JNull{}:
      True{}
    case _:
      U32.is_le(J.u32(d), elapsed)

def lanternward.problem.banner_ok?(+b: J.Val) -> Bool:
  match b:
    case J.JNull{}:
      True{}
    case _:
      U32.is_le(ga.u(b, "t"), 60)

def lanternward.fields_count?(+o: J.Val) -> U32:
  U32.from_nat(List.length(&2, Sigma<&2, &2, String, _ => J.Val>, J.fields(o)))

def lanternward.Session.valid?(+s: J.Val) -> Bool:
  J.is_null(lanternward.Session.problem(s))

# ---- repair ----

# Mend what a bad save broke.

def lanternward.Session.repair?(+s: J.Val) -> J.Val:
  lanternward.repair.seats(ga.ids(J.get(s, "seats")), ga.su(s, "elapsed", RS.u_min(ga.u(s, "elapsed"), 108000)))

# Keep only seats 1..=6, and clamp each one's `joined` to the clock.

def lanternward.repair.seats?(+ids: List<&2, U32>, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      lanternward.repair.rebuild(lanternward.Session.valid(s), s)
    case Con{+id, +t}:
      lanternward.repair.seats(t, lanternward.repair.seat(Bool.and(U32.is_ge(id, 1), U32.is_le(id, 6)), s, id))

def lanternward.repair.seat?(ok: Bool, +s: J.Val, +id: U32) -> J.Val:
  match ok:
    case False{}:
      ga.sv(s, "seats", J.del(J.get(s, "seats"), U32.show(id)))
    case True{}:
      lanternward.blow.put_seat(s, id,
        ga.su(ga.at(J.get(s, "seats"), id), "joined", RS.u_min(ga.u(ga.at(J.get(s, "seats"), id), "joined"), ga.u(s, "elapsed"))))

def lanternward.repair.rebuild?(good: Bool, +s: J.Val) -> J.Val:
  match good:
    case True{}:
      s
    case False{}:
      lanternward.repair.fresh(
        ga.ids(J.get(s, "seats")),
        lanternward.repair.clear(ga.su(s, "room", RS.u_min(ga.u(s, "room"), 4))))

# Whatever else is wrong, the room is rebuilt as it opens and the party stands
# at its start with full hearts.

def lanternward.repair.clear?(+s: J.Val) -> J.Val:
  lanternward.repair.keys(
    ga.su(
      ga.su(ga.su(ga.su(ga.su(
        ga.sv(ga.sv(ga.sv(ga.sv(s, "fx", J.JA{Nil{}}), "banner", J.JNull{}), "cue", J.JNull{}), "done", lanternward.repair.done(J.get(s, "done"), ga.u(s, "elapsed"))),
        "shake", 0), "flash", 0), "exiting", 0), "wipe", 0),
      "paid", U32.and(ga.u(s, "paid"), 126)))

def lanternward.repair.done?(+d: J.Val, +elapsed: U32) -> J.Val:
  match d:
    case J.JNull{}:
      J.JNull{}
    case _:
      Bool.pick(J.Val, U32.is_le(J.u32(d), elapsed), d, J.JNull{})

def lanternward.repair.kept?(+ids: List<&2, U32>, +kept: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      kept
    case Con{+id, +t}:
      lanternward.repair.kept(t,
        Bool.pick(J.Val, Bool.and(U32.is_ge(id, 1), U32.is_le(id, 6)), kept, J.del(kept, U32.show(id))))

def lanternward.repair.keys?(+s: J.Val) -> J.Val:
  ga.sv(
    ga.sv(s, "kept", lanternward.repair.kept(ga.ids(J.get(s, "kept")), J.get(s, "kept"))),
    "inv", ga.su(ga.su(ga.v(s, "inv"), "small_keys", RS.u_min(ga.u(ga.v(s, "inv"), "small_keys"), 8)), "containers", RS.u_min(ga.u(ga.v(s, "inv"), "containers"), 8)))

# Each seat is made fresh (keeping when it joined), then the room is entered.

def lanternward.repair.fresh?(+ids: List<&2, U32>, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      lanternward.Session.bare(lanternward.stage.Session.enter_room(ga.sv(s, "$r", lanternward.Rules.default.full()), ga.u(s, "room")))
    case Con{+id, +t}:
      lanternward.repair.fresh(t, lanternward.repair.fresh.one(lanternward.stage.Session.start_of(s, id), s, id))

def lanternward.repair.fresh.one?(+at: J.Val, +s: J.Val, +id: U32) -> J.Val:
  +hearts = (20 + 4 * ga.u(ga.v(s, "inv"), "containers") : U32)
  lanternward.blow.put_seat(s, id,
    ga.su(ga.su(lanternward.kit.Seat.new(id, ga.u(ga.at(J.get(s, "seats"), id), "joined"), ga.nu(at, 0), ga.nu(at, 1)), "max_hp", hearts), "hp", hearts))

def lanternward.stage.Session.start_of?(+s: J.Val, +id: U32) -> J.Val:
  lanternward.stage.start_of.marks(lanternward.world.Plan.find(ga.u(s, "room"), 64), id)

def lanternward.stage.start_of.marks?(+marks: List<&2, U32>, +id: U32) -> J.Val:
  lanternward.stage.start_of.pick(marks, U32.mod((id - 1 : U32), U32.from_nat(List.length(&2, U32, marks))))

def lanternward.stage.start_of.pick?(+marks: List<&2, U32>, +i: U32) -> J.Val:
  lanternward.stage.start_of.nth(marks, U32.to_nat(i))

def lanternward.stage.start_of.nth?(+marks: List<&2, U32>, n: Nat) -> J.Val:
  match marks n:
    case Nil{} _:
      ga.pair_i(0, 0)
    case Con{+h, +t} 0n:
      ga.pair_i(lanternward.world.middle(lanternward.world.packed_x(h)), lanternward.world.middle(lanternward.world.packed_y(h)))
    case Con{+h, +t} 1n+p:
      lanternward.stage.start_of.nth(t, p)

# The party goes into room `room`: its things stand, its foes wait, and every
# seat is at its start (the fallen on their feet again).

def lanternward.stage.Session.enter_room?(+s: J.Val, +room: U32) -> J.Val:
  lanternward.stage.enter_room.seats(
    ga.ids(J.get(s, "seats")),
    ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(s,
      "room", U32.and(room, 255)), "stage", 0), "stage_t", 0), "exiting", 0), "wipe", 0), "next_foe", 0), "boss", J.JNull{}),
      "props", lanternward.props.build(room)), "foes", J.JA{Nil{}}), "bolts", J.JA{Nil{}}), "drops", J.JA{Nil{}}), "fuses", J.JA{Nil{}}),
    room)

def lanternward.stage.enter_room.seats?(+ids: List<&2, U32>, +s: J.Val, +room: U32) -> J.Val:
  match ids:
    case Nil{}:
      lanternward.stage.Session.provision_room(s, room)
    case Con{+id, +t}:
      lanternward.stage.enter_room.seats(t, lanternward.stage.enter_room.seat(s, id), room)

def lanternward.stage.enter_room.seat?(+s: J.Val, +id: U32) -> J.Val:
  lanternward.stage.enter_room.seat.at(lanternward.stage.Session.start_of(s, id), ga.at(J.get(s, "seats"), id), s, id)

def lanternward.stage.enter_room.seat.at?(+at: J.Val, +k: J.Val, +s: J.Val, +id: U32) -> J.Val:
  +k1 = ga.si(ga.si(ga.sv(ga.sv(k, "safe", at), "cam", at), "x", ga.nu(at, 0)), "y", ga.nu(at, 1))
  +k2 = ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(ga.sv(ga.sv(ga.si(k1, "z", 0), "lock", J.JNull{}), "act", J.JS{"Free"}), "t", 0), "shield", 0), "chain", 0), "step", 0), "charge", 0), "pusht", 0)
  +k3 = ga.sv(ga.su(ga.sv(k2, "push", ga.pair_i(0, 0)), "taper", 0), "act", J.JS{"Free"})
  +k4 = ga.su(Bool.pick(J.Val, U32.is_eq(ga.u(k3, "hp"), 0), ga.su(k3, "hp", RS.u_min(12, ga.u(k3, "max_hp"))), k3), "inv", 60)
  lanternward.blow.put_seat(s, id, k4)

# The foes (and the Warden) a room opens with.

def lanternward.stage.Session.provision_room?(+s: J.Val, +room: U32) -> J.Val:
  lanternward.stage.provision_room.go(room, s, RS.u_max(lanternward.seat_count(s), 1))

def lanternward.stage.provision_room.go?(+room: U32, +s: J.Val, +seats: U32) -> J.Val:
  match room:
    case 0:
      lanternward.stage.spawn_at(s, lanternward.world.Plan.find(0, 83), (1 + U32.div((seats - 1 : U32), 3) : U32), "Snuffer", False{})
    case 2:
      lanternward.stage.spawn_at(s, lanternward.world.Plan.find(2, 84), Bool.pick(U32, U32.is_ge(seats, 4), 2, 1), "Turret", False{})
    case 3:
      lanternward.stage.spawn_at(s, lanternward.world.Plan.find(3, 83), (2 + U32.div(seats, 2) : U32), "Snuffer", True{})
    case 4:
      lanternward.stage.provision_room.lamps(lanternward.stage.provision_room.boss(lanternward.world.Plan.find(4, 87), s, seats), 0)
    case _:
      s

# `for (tx, ty) in marks.take(n) { spawn_foe(kind, middle(tx), middle(ty), hooded) }`.

def lanternward.stage.spawn_at?(+s: J.Val, +marks: List<&2, U32>, +n: U32, +kind: String, +hooded: Bool) -> J.Val:
  match marks:
    case Nil{}:
      s
    case Con{+m, +t}:
      lanternward.stage.spawn_at(
        Bool.pick(J.Val, U32.is_gt(n, 0),
          lanternward.foes.Session.spawn_foe(s, kind, lanternward.world.middle(lanternward.world.packed_x(m)), lanternward.world.middle(lanternward.world.packed_y(m)), hooded),
          s),
        t, RS.u_sat_sub(n, 1), kind, hooded)

# The Warden stands on the first W mark.

def lanternward.stage.provision_room.boss?(+marks: List<&2, U32>, +s: J.Val, +seats: U32) -> J.Val:
  match marks:
    case Nil{}:
      s
    case Con{+m, +t}:
      ga.sv(s, "boss", lanternward.boss.Warden.new(lanternward.world.middle(lanternward.world.packed_x(m)), lanternward.world.middle(lanternward.world.packed_y(m)), seats))

# The arena's lamps burn until the Gloam falls.

def lanternward.stage.provision_room.lamps?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.provision_room.lamps.go(U32.is_lt(i, lanternward.len(s, "props")), s, i)

def lanternward.stage.provision_room.lamps.go?(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.stage.provision_room.lamps(
        lanternward.stage.set_on(lanternward.eis(s, "props", i, "kind", "Brazier"), s, i), (i + 1 : U32))

def lanternward.stage.set_on?(yes: Bool, +s: J.Val, +i: U32) -> J.Val:
  match yes:
    case True{}:
      lanternward.es(s, "props", i, "on", 1)
    case False{}:
      s

# The Lamp Hall's wave: the Gloam answers the Lantern.

def lanternward.stage.Session.spawn_wave?(+s: J.Val) -> J.Val:
  lanternward.stage.spawn_at(
    s, lanternward.world.Plan.find(1, 83),
    RS.u_min((2 + U32.div((RS.u_max(lanternward.seat_count(s), 1) - 1 : U32), 2) : U32), 4), "Snuffer", False{})

# The index of the first prop of this kind and id, or null.

def lanternward.stage.prop_of.go?(+ps: List<&2, J.Val>, +kind: String, +id: U32, +i: U32) -> J.Val:
  match ps:
    case Nil{}:
      J.JNull{}
    case Con{+p, +t}:
      lanternward.stage.prop_of.hit(
        Bool.and(String.eq(ga.s(p, "kind"), kind), U32.is_eq(ga.u(p, "id"), id)), t, kind, id, i)

def lanternward.stage.prop_of.hit?(hit: Bool, +t: List<&2, J.Val>, +kind: String, +id: U32, +i: U32) -> J.Val:
  match hit:
    case True{}:
      J.JU{i}
    case False{}:
      lanternward.stage.prop_of.go(t, kind, id, (i + 1 : U32))

def lanternward.stage.Session.prop_of?(+s: J.Val, +kind: String, +id: U32) -> J.Val:
  lanternward.stage.prop_of.go(ga.items(s, "props"), kind, id, 0)

# Whether the prop of this kind and id exists and has `on == 1`.

def lanternward.stage.prop_on?(+s: J.Val, +kind: String, +id: U32) -> Bool:
  lanternward.stage.prop_on.go(lanternward.stage.Session.prop_of(s, kind, id), s)

def lanternward.stage.prop_on.go?(+i: J.Val, +s: J.Val) -> Bool:
  match i:
    case J.JNull{}:
      False{}
    case _:
      U32.is_eq(lanternward.eu(s, "props", J.u32(i), "on"), 1)

# The position [x, y] of the prop at index `i`, or [0, 0] for none.

def lanternward.stage.prop_pos?(+s: J.Val, +i: J.Val) -> J.Val:
  match i:
    case J.JNull{}:
      ga.pair_i(0, 0)
    case _:
      ga.pair_i(lanternward.eu(s, "props", J.u32(i), "x"), lanternward.eu(s, "props", J.u32(i), "y"))

# Every prop of this kind is lit (and there is at least one).

def lanternward.stage.all_lit.go?(+ps: List<&2, J.Val>, +kind: String, +any: Bool) -> Bool:
  match ps:
    case Nil{}:
      any
    case Con{+p, +t}:
      lanternward.stage.all_lit.step(String.eq(ga.s(p, "kind"), kind), lanternward.props.Prop.lit(p), t, kind, any)

def lanternward.stage.all_lit.step?(+mine: Bool, +lit: Bool, +t: List<&2, J.Val>, +kind: String, +any: Bool) -> Bool:
  Bool.pick(Bool, Bool.and(mine, Bool.not(lit)), False{},
    lanternward.stage.all_lit.go(t, kind, Bool.or(any, mine)))

def lanternward.stage.Session.all_lit?(+s: J.Val, +kind: String) -> Bool:
  lanternward.stage.all_lit.go(ga.items(s, "props"), kind, False{})

def lanternward.stage.foes_down.go?(+fs: List<&2, J.Val>) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{+f, +t}:
      Bool.and(Bool.not(lanternward.foes.Foe.alive(f)), lanternward.stage.foes_down.go(t))

def lanternward.stage.Session.foes_down?(+s: J.Val) -> Bool:
  lanternward.stage.foes_down.go(ga.items(s, "foes"))

# Whether the way on is open.

def lanternward.stage.Session.exit_open?(+s: J.Val) -> Bool:
  lanternward.stage.exit_open.go(ga.u(s, "room"), ga.u(s, "stage"))

def lanternward.stage.exit_open.go?(+room: U32, +stage: U32) -> Bool:
  match room:
    case 0:
      U32.is_ge(stage, 1)
    case 1:
      U32.is_ge(stage, 4)
    case 2:
      U32.is_ge(stage, 1)
    case 3:
      U32.is_ge(stage, 4)
    case _:
      False{}

# ---- the things in the room ----

# Positions that press a plate: knights on the ground, then every block (a
# flat list x, y, x, y, ...).

def lanternward.stage.weights.seats?(+ids: List<&2, U32>, +seats: J.Val) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      Bool.pick(List<&2, U32>,
        Bool.and(lanternward.kit.Seat.up(ga.at(seats, id)), U32.is_eq(ga.u(ga.at(seats, id), "z"), 0)),
        Con{ga.u(ga.at(seats, id), "x"), Con{ga.u(ga.at(seats, id), "y"), lanternward.stage.weights.seats(t, seats)}},
        lanternward.stage.weights.seats(t, seats))

def lanternward.stage.weights.blocks?(+ps: List<&2, J.Val>) -> List<&2, U32>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      Bool.pick(List<&2, U32>, String.eq(ga.s(p, "kind"), "Block"),
        Con{ga.u(p, "x"), Con{ga.u(p, "y"), lanternward.stage.weights.blocks(t)}},
        lanternward.stage.weights.blocks(t))

# Positions of the knights holding a flare.

def lanternward.stage.flares?(+ids: List<&2, U32>, +seats: J.Val) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      Bool.pick(List<&2, U32>, lanternward.kit.Seat.flaring(ga.at(seats, id)),
        Con{ga.u(ga.at(seats, id), "x"), Con{ga.u(ga.at(seats, id), "y"), lanternward.stage.flares(t, seats)}},
        lanternward.stage.flares(t, seats))

# Whether any (x, y) pair in the flat list lies within `reach` of (px, py).

def lanternward.stage.any_near?(+xs: List<&2, U32>, +px: U32, +py: U32, +reach: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+x, +r1}:
      lanternward.stage.any_near.y(r1, x, px, py, reach)

def lanternward.stage.any_near.y?(+r1: List<&2, U32>, +x: U32, +px: U32, +py: U32, +reach: U32) -> Bool:
  match r1:
    case Nil{}:
      False{}
    case Con{+y, +rest}:
      Bool.or(
        RS.i_lt(lanternward.world.dist((x - px : U32), (y - py : U32)), reach),
        lanternward.stage.any_near(rest, px, py, reach))

def lanternward.stage.u32s?(+xs: List<&2, U32>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{J.of_i32(h), lanternward.stage.u32s(t)}

def lanternward.stage.j32s?(+xs: List<&2, J.Val>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{J.u32(h), lanternward.stage.j32s(t)}

# One tick of the things in the room. `$weights` and `$flares` ride in `s`;
# a sigil found leaves its place in `$revealed`.

def lanternward.stage.Session.tick_props?(+s: J.Val) -> J.Val:
  lanternward.stage.tick_props.reveal(
    lanternward.stage.tick_props.loop(
      ga.sv(ga.sv(ga.sv(s, "$weights", J.JA{lanternward.stage.u32s(
        List.append(&2, U32, lanternward.stage.weights.seats(ga.ids(J.get(s, "seats")), J.get(s, "seats")), lanternward.stage.weights.blocks(ga.items(s, "props"))))}),
        "$flares", J.JA{lanternward.stage.u32s(lanternward.stage.flares(ga.ids(J.get(s, "seats")), J.get(s, "seats")))}),
        "$revealed", J.JNull{}),
      0))

def lanternward.stage.tick_props.loop?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.loop.go(U32.is_lt(i, lanternward.len(s, "props")), s, i)

def lanternward.stage.tick_props.loop.go?(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.stage.tick_props.loop(
        lanternward.stage.tick_props.kind(ga.s(lanternward.el(s, "props", i), "kind"), s, i), (i + 1 : U32))

def lanternward.stage.tick_props.kind?(+kind: String, +s: J.Val, +i: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Brazier"), lanternward.stage.tick_props.brazier(U32.is_gt(lanternward.eu(s, "props", i, "t"), 0), s, i),
    Bool.pick(J.Val, String.eq(kind, "Plate"), lanternward.stage.tick_props.plate(s, i),
      Bool.pick(J.Val, String.eq(kind, "Block"), lanternward.stage.tick_props.block(s, i),
        Bool.pick(J.Val, String.eq(kind, "Sigil"), lanternward.stage.tick_props.sigil(U32.is_eq(lanternward.eu(s, "props", i, "on"), 0), s, i),
          Bool.pick(J.Val, String.eq(kind, "Fissure"), lanternward.es(s, "props", i, "t", RS.u_sat_sub(lanternward.eu(s, "props", i, "t"), 1)),
            s)))))

# A brazier gutters: its flame counts down and puffs out unless lit for good.

def lanternward.stage.tick_props.brazier?(burning: Bool, +s: J.Val, +i: U32) -> J.Val:
  match burning:
    case False{}:
      s
    case True{}:
      lanternward.stage.tick_props.brazier.out(
        Bool.and(U32.is_eq(lanternward.eu(s, "props", i, "t"), 1), U32.is_eq(lanternward.eu(s, "props", i, "on"), 0)),
        lanternward.es(s, "props", i, "t", (lanternward.eu(s, "props", i, "t") - 1 : U32)), i)

def lanternward.stage.tick_props.brazier.out?(puff: Bool, +s: J.Val, +i: U32) -> J.Val:
  match puff:
    case False{}:
      s
    case True{}:
      lanternward.sound(lanternward.props.Session.fx(s, "Puff", lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 0), "step_ash")

# A plate is pressed by a knight on the ground or a block on it.

def lanternward.stage.tick_props.plate?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.plate.set(
    lanternward.eu(s, "props", i, "on"),
    Bool.to_u32(lanternward.stage.any_near(lanternward.stage.j32s(ga.items(s, "$weights")), lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 2304)),
    s, i)

def lanternward.stage.tick_props.plate.set?(+was: U32, +now: U32, +s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.plate.sound(U32.is_ne(was, now), now, lanternward.es(s, "props", i, "on", now))

def lanternward.stage.tick_props.plate.sound?(changed: Bool, +now: U32, +s: J.Val) -> J.Val:
  match changed:
    case False{}:
      s
    case True{}:
      lanternward.sound(s, Bool.pick(String, U32.is_eq(now, 1), "wall_up", "step_stone"))

# A block slides a pixel a tick toward where it was pushed, then settles.

def lanternward.stage.tick_props.block?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.block.go(U32.is_gt(lanternward.eu(s, "props", i, "on"), 0), s, i)

def lanternward.stage.tick_props.block.go?(sliding: Bool, +s: J.Val, +i: U32) -> J.Val:
  match sliding:
    case False{}:
      lanternward.es(s, "props", i, "t", RS.u_sat_sub(lanternward.eu(s, "props", i, "t"), 1))
    case True{}:
      lanternward.stage.tick_props.block.move(lanternward.eu(s, "props", i, "on"), s, i)

def lanternward.stage.tick_props.block.move?(+on: U32, +s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.block.t(
    lanternward.esi(lanternward.esi(s, "props", i, "x",
      (lanternward.eu(s, "props", i, "x") + Bool.pick(U32, U32.is_eq(on, 1), 256, Bool.pick(U32, U32.is_eq(on, 2), 4294967040, 0)) : U32)),
      "props", i, "y",
      (lanternward.eu(s, "props", i, "y") + Bool.pick(U32, U32.is_eq(on, 3), 256, Bool.pick(U32, Bool.or(U32.is_eq(on, 1), U32.is_eq(on, 2)), 0, 4294967040)) : U32)),
    i)

def lanternward.stage.tick_props.block.t?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.block.settle(
    U32.is_le(lanternward.eu(s, "props", i, "t"), 1), lanternward.es(s, "props", i, "t", RS.u_sat_sub(lanternward.eu(s, "props", i, "t"), 1)), i)

def lanternward.stage.tick_props.block.settle?(done: Bool, +s: J.Val, +i: U32) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.stage.tick_props.block.snap(
        lanternward.world.middle(lanternward.world.tile_of(lanternward.eu(s, "props", i, "x"))),
        lanternward.world.middle(lanternward.world.tile_of(lanternward.eu(s, "props", i, "y"))), s, i)

def lanternward.stage.tick_props.block.snap?(+x: U32, +y: U32, +s: J.Val, +i: U32) -> J.Val:
  lanternward.sound(
    lanternward.es(
      lanternward.es(
        lanternward.es(
          lanternward.esi(lanternward.esi(s, "props", i, "x", x), "props", i, "y", y),
          "props", i, "tx", U32.and(lanternward.world.tile_of(x), 255)),
        "props", i, "ty", U32.and(lanternward.world.tile_of(y), 255)),
      "props", i, "on", 0),
    "rock_land")

# A sigil under a flare fills; unlit it fades. Found, it names its place.

def lanternward.stage.tick_props.sigil?(dark: Bool, +s: J.Val, +i: U32) -> J.Val:
  match dark:
    case False{}:
      s
    case True{}:
      lanternward.stage.tick_props.sigil.go(
        lanternward.stage.any_near(lanternward.stage.j32s(ga.items(s, "$flares")), lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 19968),
        s, i)

def lanternward.stage.tick_props.sigil.go?(lit: Bool, +s: J.Val, +i: U32) -> J.Val:
  match lit:
    case False{}:
      lanternward.es(s, "props", i, "t", RS.u_sat_sub(lanternward.eu(s, "props", i, "t"), 1))
    case True{}:
      lanternward.stage.tick_props.sigil.found(
        U32.is_ge((lanternward.eu(s, "props", i, "t") + 1 : U32), 24),
        lanternward.es(s, "props", i, "t", (lanternward.eu(s, "props", i, "t") + 1 : U32)), i)

def lanternward.stage.tick_props.sigil.found?(done: Bool, +s: J.Val, +i: U32) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      ga.sv(lanternward.es(s, "props", i, "on", 1), "$revealed", ga.pair_i(lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y")))

# A sigil found opens what it was hiding.

def lanternward.stage.tick_props.reveal?(+s: J.Val) -> J.Val:
  lanternward.stage.tick_props.reveal.go(ga.v(s, "$revealed"), J.del(J.del(J.del(s, "$weights"), "$flares"), "$revealed"))

def lanternward.stage.tick_props.reveal.go?(+at: J.Val, +s: J.Val) -> J.Val:
  match at:
    case J.JNull{}:
      s
    case _:
      lanternward.sound(
        lanternward.props.Session.solved(lanternward.props.Session.set_gate(s, 2, True{}), ga.nu(at, 0), ga.nu(at, 1)),
        "door_open")

# A gate that closed or a block that slid over a body puts it out on the
# nearest open ground: nobody is walled in.

def lanternward.stage.Session.unstick?(+s: J.Val) -> J.Val:
  lanternward.stage.unstick.ids(ga.ids(J.get(s, "seats")), s)

def lanternward.stage.unstick.ids?(+ids: List<&2, U32>, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.stage.unstick.ids(t, lanternward.stage.unstick.one(s, id))

def lanternward.stage.unstick.one?(+s: J.Val, +id: U32) -> J.Val:
  lanternward.stage.unstick.go(
    Bool.not(lanternward.props.Session.clear(s, ga.u(ga.at(J.get(s, "seats"), id), "x"), ga.u(ga.at(J.get(s, "seats"), id), "y"), 640)),
    s, id)

def lanternward.stage.unstick.go?(stuck: Bool, +s: J.Val, +id: U32) -> J.Val:
  match stuck:
    case False{}:
      s
    case True{}:
      lanternward.stage.unstick.set(
        lanternward.foes.Session.nudge(s, ga.u(ga.at(J.get(s, "seats"), id), "x"), ga.u(ga.at(J.get(s, "seats"), id), "y"), 1280), s, id)

def lanternward.stage.unstick.set?(+p: J.Val, +s: J.Val, +id: U32) -> J.Val:
  lanternward.blow.put_seat(s, id, ga.si(ga.si(ga.at(J.get(s, "seats"), id), "x", ga.nu(p, 0)), "y", ga.nu(p, 1)))

# ---- what each room asks ----

# What each room asks, and what it gives.

def lanternward.stage.Session.tick_stage?(+s: J.Val) -> J.Val:
  lanternward.stage.tick_stage.exit(
    lanternward.stage.tick_stage.room(ga.u(s, "room"), ga.su(s, "stage_t", RS.u_min((ga.u(s, "stage_t") + 1 : U32), 108000))))

def lanternward.stage.tick_stage.room?(+room: U32, +s: J.Val) -> J.Val:
  match room:
    case 0:
      lanternward.stage.stair(Bool.and(U32.is_eq(ga.u(s, "stage"), 0), lanternward.stage.Session.foes_down(s)), s)
    case 1:
      lanternward.stage.Session.stage_lamp_hall(s)
    case 2:
      lanternward.stage.Session.stage_gallery(s)
    case 3:
      lanternward.stage.Session.stage_vault(s)
    case _:
      lanternward.boss.Session.stage_warden(s)

def lanternward.stage.stair?(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.stage.Session.open_exit(ga.su(s, "stage", 1))

# The way on: stand in it.

def lanternward.stage.tick_stage.exit?(+s: J.Val) -> J.Val:
  lanternward.stage.tick_stage.exit.go(
    Bool.and(U32.is_eq(ga.u(s, "exiting"), 0), Bool.and(lanternward.stage.Session.exit_open(s), U32.is_lt(ga.u(s, "room"), 4))),
    s)

def lanternward.stage.tick_stage.exit.go?(opening: Bool, +s: J.Val) -> J.Val:
  match opening:
    case True{}:
      lanternward.stage.tick_stage.exit.hit(
        lanternward.stage.stands_in_exit(ga.ids(J.get(s, "seats")), J.get(s, "seats"), ga.u(s, "room")), s)
    case False{}:
      lanternward.stage.tick_stage.exit.count(U32.is_gt(ga.u(s, "exiting"), 0), s)

def lanternward.stage.stands_in_exit?(+ids: List<&2, U32>, +seats: J.Val, +room: U32) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case Con{+id, +t}:
      Bool.or(
        Bool.and(
          Bool.and(lanternward.kit.Seat.up(ga.at(seats, id)), U32.is_eq(ga.u(ga.at(seats, id), "z"), 0)),
          U32.is_eq(lanternward.world.Plan.at(room, lanternward.world.tile_of(ga.u(ga.at(seats, id), "x")), lanternward.world.tile_of(ga.u(ga.at(seats, id), "y"))), 81)),
        lanternward.stage.stands_in_exit(t, seats, room))

def lanternward.stage.tick_stage.exit.hit?(hit: Bool, +s: J.Val) -> J.Val:
  match hit:
    case True{}:
      lanternward.sound(ga.su(s, "exiting", 14), "door_pass")
    case False{}:
      s

def lanternward.stage.tick_stage.exit.count?(counting: Bool, +s: J.Val) -> J.Val:
  match counting:
    case False{}:
      s
    case True{}:
      lanternward.stage.tick_stage.exit.leave(U32.is_eq((ga.u(s, "exiting") - 1 : U32), 0), ga.su(s, "exiting", (ga.u(s, "exiting") - 1 : U32)))

def lanternward.stage.tick_stage.exit.leave?(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.sound(lanternward.stage.Session.enter_room(s, (ga.u(s, "room") + 1 : U32)), "descend")

# The exit gate swings open. (`set_gate` leaves whether any changed in `$ret`.)

def lanternward.stage.Session.open_exit?(+s: J.Val) -> J.Val:
  lanternward.stage.open_exit.go(lanternward.props.Session.set_gate(s, 0, True{}))

def lanternward.stage.open_exit.go?(+s: J.Val) -> J.Val:
  lanternward.stage.open_exit.solved(ga.b(s, "$ret"), J.del(s, "$ret"))

def lanternward.stage.open_exit.solved?(changed: Bool, +s: J.Val) -> J.Val:
  match changed:
    case False{}:
      s
    case True{}:
      lanternward.stage.open_exit.at(lanternward.find_prop(s, "Gate", 0), s)

def lanternward.stage.open_exit.at?(+g: J.Val, +s: J.Val) -> J.Val:
  match g:
    case J.JNull{}:
      lanternward.sound(lanternward.props.Session.solved(s, 0, 0), "door_open")
    case _:
      lanternward.sound(lanternward.props.Session.solved(s, ga.u(g, "x"), ga.u(g, "y")), "door_open")

# The Lamp Hall: braziers, the Lantern, the Gloam's wave, the great brazier.

def lanternward.stage.Session.stage_lamp_hall?(+s: J.Val) -> J.Val:
  lanternward.stage.lamp_hall.go(ga.u(s, "stage"), s)

def lanternward.stage.lamp_hall.go?(+stage: U32, +s: J.Val) -> J.Val:
  match stage:
    case 0:
      lanternward.stage.lamp_hall.braziers(lanternward.stage.Session.all_lit(s, "Brazier"), s)
    case 1:
      lanternward.stage.lamp_hall.chest(lanternward.stage.prop_on(s, "Chest", 0), s)
    case 2:
      lanternward.stage.lamp_hall.wave(Bool.and(U32.is_gt(ga.u(s, "stage_t"), 20), lanternward.stage.Session.foes_down(s)), s)
    case 3:
      lanternward.stage.lamp_hall.great(lanternward.stage.prop_on(s, "Great", 0), s)
    case _:
      s

# Three braziers, all alight at once, from one ember.

def lanternward.stage.lamp_hall.braziers?(lit: Bool, +s: J.Val) -> J.Val:
  match lit:
    case False{}:
      s
    case True{}:
      lanternward.stage.lamp_hall.braziers.at(
        lanternward.stage.prop_pos(s, lanternward.stage.Session.prop_of(s, "Chest", 0)), ga.su(s, "stage", 1))

def lanternward.stage.lamp_hall.braziers.at?(+at: J.Val, +s: J.Val) -> J.Val:
  lanternward.sound(
    lanternward.props.Session.solved(
      J.del(lanternward.props.Session.set_gate(s, 1, True{}), "$ret"), ga.nu(at, 0), ga.nu(at, 1)),
    "door_open")

# The niche stands open; the Lantern is in the chest.

def lanternward.stage.lamp_hall.chest?(open: Bool, +s: J.Val) -> J.Val:
  match open:
    case False{}:
      s
    case True{}:
      lanternward.sound(lanternward.stage.Session.spawn_wave(ga.su(ga.su(s, "stage", 2), "stage_t", 0)), "wave_gate")

def lanternward.stage.lamp_hall.wave?(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.sound(ga.su(s, "stage", 3), "play_card")

def lanternward.stage.lamp_hall.great?(lit: Bool, +s: J.Val) -> J.Val:
  match lit:
    case False{}:
      s
    case True{}:
      lanternward.stage.Session.open_exit(ga.su(s, "stage", 4))

# The Arrow Gallery: two plates and a crystal.

def lanternward.stage.plate_held?(+ps: List<&2, J.Val>, +id: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+p, +t}:
      Bool.or(
        Bool.and(String.eq(ga.s(p, "kind"), "Plate"), Bool.and(U32.is_eq(ga.u(p, "id"), id), U32.is_eq(ga.u(p, "on"), 1))),
        lanternward.stage.plate_held(t, id))

def lanternward.stage.any_on?(+ps: List<&2, J.Val>, +kind: String, +id: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+p, +t}:
      Bool.or(
        Bool.and(String.eq(ga.s(p, "kind"), kind), Bool.and(Bool.or(U32.is_eq(id, 4294967295), U32.is_eq(ga.u(p, "id"), id)), U32.is_eq(ga.u(p, "on"), 1))),
        lanternward.stage.any_on(t, kind, id))

def lanternward.stage.Session.stage_gallery?(+s: J.Val) -> J.Val:
  lanternward.stage.gallery.go(
    Bool.and(
      lanternward.stage.plate_held(ga.items(s, "props"), 0),
      Bool.or(lanternward.stage.plate_held(ga.items(s, "props"), 1), U32.is_lt(lanternward.seat_count(s), 2))),
    lanternward.stage.any_on(ga.items(s, "props"), "Crystal", 4294967295),
    lanternward.stage.any_on(ga.items(s, "props"), "Gate", 1), s)

def lanternward.stage.gallery.go?(+plate: Bool, +crystal: Bool, +was: Bool, +s: J.Val) -> J.Val:
  lanternward.stage.gallery.gate(Bool.or(plate, crystal), was, crystal, lanternward.props.Session.set_gate(s, 1, Bool.or(plate, crystal)))

# The plate-gate holds open only while it is held, or for good once the
# crystal has woken.

def lanternward.stage.gallery.gate?(+want: Bool, +was: Bool, +crystal: Bool, +s: J.Val) -> J.Val:
  lanternward.stage.gallery.after(
    Bool.and(ga.b(s, "$ret"), Bool.xor(want, was)), want, crystal, J.del(s, "$ret"))

def lanternward.stage.gallery.after?(changed: Bool, +want: Bool, +crystal: Bool, +s: J.Val) -> J.Val:
  lanternward.stage.gallery.crystal(crystal, lanternward.stage.gallery.sound(changed, want, s))

def lanternward.stage.gallery.sound?(yes: Bool, +want: Bool, +s: J.Val) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.stage.gallery.sound.at(lanternward.find_prop(s, "Gate", 1), want, s)

def lanternward.stage.gallery.sound.at?(+g: J.Val, +want: Bool, +s: J.Val) -> J.Val:
  match g:
    case J.JNull{}:
      lanternward.stage.gallery.sound.dust(want, 0, 0, lanternward.sound(s, Bool.pick(String, want, "door_open", "door_seal")))
    case _:
      lanternward.stage.gallery.sound.dust(want, ga.u(g, "x"), ga.u(g, "y"), lanternward.sound(s, Bool.pick(String, want, "door_open", "door_seal")))

def lanternward.stage.gallery.sound.dust?(want: Bool, +x: U32, +y: U32, +s: J.Val) -> J.Val:
  match want:
    case True{}:
      lanternward.props.Session.fx(s, "Dust", x, y, 0)
    case False{}:
      s

def lanternward.stage.gallery.crystal?(+crystal: Bool, +s: J.Val) -> J.Val:
  lanternward.stage.gallery.open(Bool.and(U32.is_eq(ga.u(s, "stage"), 0), crystal), s)

def lanternward.stage.gallery.open?(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.stage.Session.open_exit(ga.su(s, "stage", 1))

# The Key Vault: the dark beaten, the key chest, the small key's door, the
# boss key.

def lanternward.stage.Session.stage_vault?(+s: J.Val) -> J.Val:
  lanternward.stage.vault.boss(lanternward.stage.vault.go(ga.u(s, "stage"), s))

def lanternward.stage.vault.go?(+stage: U32, +s: J.Val) -> J.Val:
  match stage:
    case 0:
      lanternward.stage.vault.dark(lanternward.stage.Session.foes_down(s), s)
    case 1:
      lanternward.stage.vault.step(lanternward.stage.prop_on(s, "Chest", 1), 2, s)
    case 2:
      lanternward.stage.vault.step(lanternward.stage.any_on(ga.items(s, "props"), "Lock", 4294967295), 3, s)
    case 3:
      lanternward.stage.vault.step(lanternward.stage.prop_on(s, "Chest", 0), 4, s)
    case _:
      s

def lanternward.stage.vault.step?(yes: Bool, +to: U32, +s: J.Val) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      ga.su(s, "stage", to)

# The vault's key chest is not there until the dark is beaten; the spark the
# last snuffer gave back lights the vault's braziers for good.

def lanternward.stage.vault.dark?(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.stage.provision_room.lamps(
        lanternward.stage.vault.chest(lanternward.stage.Session.prop_of(s, "Chest", 1), ga.su(s, "stage", 1)), 0)

def lanternward.stage.vault.chest?(+i: J.Val, +s: J.Val) -> J.Val:
  match i:
    case J.JNull{}:
      s
    case _:
      lanternward.props.Session.solved(
        lanternward.props.Session.fx(lanternward.es(s, "props", J.u32(i), "on", 0), "Light", lanternward.eu(s, "props", J.u32(i), "x"), lanternward.eu(s, "props", J.u32(i), "y"), 0),
        lanternward.eu(s, "props", J.u32(i), "x"), lanternward.eu(s, "props", J.u32(i), "y"))

def lanternward.stage.vault.boss?(+s: J.Val) -> J.Val:
  lanternward.stage.vault.step(
    Bool.and(lanternward.stage.any_on(ga.items(s, "props"), "Boss", 4294967295), U32.is_ge(ga.u(s, "stage"), 4)), 5, s)

# ---- drops ----

# Drops: hearts mend, gems and bombs are kept, the Boss Key is carried.

def lanternward.stage.Session.pickups?(+s: J.Val) -> J.Val:
  lanternward.stage.pickups.drops(lanternward.stage.pickups.lying(lanternward.stage.pickups.follow(s)), 0)

# The Boss Key follows its carrier; a carrier who falls or leaves puts it
# down where they stood.

def lanternward.stage.pickups.follow?(+s: J.Val) -> J.Val:
  lanternward.stage.pickups.follow.go(lanternward.tag_is(ga.v(ga.v(s, "inv"), "boss"), "Carried"), s)

def lanternward.stage.pickups.follow.go?(carried: Bool, +s: J.Val) -> J.Val:
  match carried:
    case False{}:
      s
    case True{}:
      lanternward.stage.pickups.follow.who(ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), s)

def lanternward.stage.pickups.follow.who?(+id: U32, +s: J.Val) -> J.Val:
  lanternward.stage.pickups.follow.up(
    Bool.and(ga.has_key(J.get(s, "seats"), id), lanternward.kit.Seat.up(ga.at(J.get(s, "seats"), id))), id, s)

def lanternward.stage.pickups.follow.up?(up: Bool, +id: U32, +s: J.Val) -> J.Val:
  match up:
    case True{}:
      ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss",
        lanternward.variant("Carried", J.JA{[J.JU{id}, J.of_i32(ga.u(ga.at(J.get(s, "seats"), id), "x")), J.of_i32(ga.u(ga.at(J.get(s, "seats"), id), "y"))]})))
    case False{}:
      ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss",
        lanternward.variant("Lying", J.JA{[J.nth(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 1), J.nth(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 2)]})))

# The first knight (by seat) standing on a lying key picks it up.

def lanternward.stage.pickups.lying?(+s: J.Val) -> J.Val:
  lanternward.stage.pickups.lying.go(lanternward.tag_is(ga.v(ga.v(s, "inv"), "boss"), "Lying"), s)

def lanternward.stage.pickups.lying.go?(lying: Bool, +s: J.Val) -> J.Val:
  match lying:
    case False{}:
      s
    case True{}:
      lanternward.stage.pickups.lying.find(
        lanternward.stage.first_near(ga.ids(J.get(s, "seats")), J.get(s, "seats"),
          ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 1)),
        s)

# The first up knight within pick-up reach of (x, y): the seat id, or null.

def lanternward.stage.first_near?(+ids: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32) -> J.Val:
  match ids:
    case Nil{}:
      J.JNull{}
    case Con{+id, +t}:
      lanternward.stage.first_near.hit(
        Bool.and(lanternward.kit.Seat.up(ga.at(seats, id)),
          RS.i_lt(lanternward.world.dist((ga.u(ga.at(seats, id), "x") - x : U32), (ga.u(ga.at(seats, id), "y") - y : U32)), 2560)),
        id, t, seats, x, y)

def lanternward.stage.first_near.hit?(hit: Bool, +id: U32, +t: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32) -> J.Val:
  match hit:
    case True{}:
      J.JU{id}
    case False{}:
      lanternward.stage.first_near(t, seats, x, y)

def lanternward.stage.pickups.lying.find?(+id: J.Val, +s: J.Val) -> J.Val:
  match id:
    case J.JNull{}:
      s
    case _:
      lanternward.sound(
        ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss",
          lanternward.variant("Carried", J.JA{[J.JU{U32.and(J.u32(id), 255)}, J.nth(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), J.nth(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 1)]}))),
        "card_pickup")

def lanternward.stage.pickups.drops?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.pickups.drops.go(U32.is_lt(i, lanternward.len(s, "drops")), s, i)

def lanternward.stage.pickups.drops.go?(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.stage.pickups.drop(
        lanternward.es(s, "drops", i, "age", RS.u_min((lanternward.eu(s, "drops", i, "age") + 1 : U32), 65535)), i)

def lanternward.stage.pickups.drop?(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.pickups.drop.taker(
    lanternward.stage.first_near(ga.ids(J.get(s, "seats")), J.get(s, "seats"), lanternward.eu(s, "drops", i, "x"), lanternward.eu(s, "drops", i, "y")),
    ga.s(lanternward.el(s, "drops", i), "kind"),
    Bool.and(U32.is_gt(lanternward.eu(s, "drops", i, "age"), 900), Bool.not(lanternward.eis(s, "drops", i, "kind", "Container"))),
    s, i)

def lanternward.stage.pickups.drop.taker?(+taker: J.Val, +kind: String, +old: Bool, +s: J.Val, +i: U32) -> J.Val:
  match taker:
    case J.JNull{}:
      lanternward.stage.pickups.drop.end(old, s, i)
    case _:
      lanternward.stage.pickups.drop.take(J.u32(taker), kind, old, s, i)

def lanternward.stage.pickups.drop.take?(+id: U32, +kind: String, +old: Bool, +s: J.Val, +i: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Heart"),
    lanternward.stage.pickups.heart(
      U32.is_lt(ga.u(ga.at(J.get(s, "seats"), id), "hp"), ga.u(ga.at(J.get(s, "seats"), id), "max_hp")), id, old, s, i),
    Bool.pick(J.Val, String.eq(kind, "Gem"),
      lanternward.stage.pickups.drop.end(True{},
        lanternward.sound(
          lanternward.blow.put_seat(s, id, ga.su(ga.at(J.get(s, "seats"), id), "gems", (ga.u(ga.at(J.get(s, "seats"), id), "gems") + 1 : U32))),
          "gold"), i),
      Bool.pick(J.Val, String.eq(kind, "Bomb"),
        lanternward.stage.pickups.drop.end(True{},
          lanternward.sound(
            lanternward.blow.put_seat(s, id, ga.su(ga.at(J.get(s, "seats"), id), "bombs", RS.u_min((ga.u(ga.at(J.get(s, "seats"), id), "bombs") + 1 : U32), 10))),
            "spoil_pickup"), i),
        lanternward.stage.pickups.drop.end(True{}, lanternward.stage.pickups.container(s, id), i))))

def lanternward.stage.pickups.heart?(mend: Bool, +id: U32, +old: Bool, +s: J.Val, +i: U32) -> J.Val:
  match mend:
    case False{}:
      lanternward.stage.pickups.drop.end(old, s, i)
    case True{}:
      lanternward.stage.pickups.drop.end(True{},
        lanternward.sound(
          lanternward.blow.put_seat(s, id,
            ga.su(ga.at(J.get(s, "seats"), id), "hp",
              RS.u_min((ga.u(ga.at(J.get(s, "seats"), id), "hp") + 4 : U32), ga.u(ga.at(J.get(s, "seats"), id), "max_hp")))),
          "potion"), i)

# A heart container: the finder takes it as a granted item, the stage moves on.

def lanternward.stage.pickups.container?(+s: J.Val, +id: U32) -> J.Val:
  ga.su(
    lanternward.kit.put_k(
      lanternward.ctx.Session.grant(
        ga.sv(ga.sv(s, "$k", ga.at(J.get(s, "seats"), id)), "seats", J.del(J.get(s, "seats"), U32.show(id))),
        id, J.JS{"Container"}),
      id),
    "stage", RS.u_max(ga.u(s, "stage"), 3))

# A taken or aged-out drop is swap_removed (the last drop takes its place);
# else the next drop.

def lanternward.stage.pickups.drop.end?(+gone: Bool, +s: J.Val, +i: U32) -> J.Val:
  match gone:
    case True{}:
      lanternward.stage.pickups.drops(lanternward.stage.pickups.swap_remove(s, i), i)
    case False{}:
      lanternward.stage.pickups.drops(s, (i + 1 : U32))

def lanternward.stage.pickups.swap_remove?(+s: J.Val, +i: U32) -> J.Val:
  ga.sv(s, "drops",
    J.truncate(J.set_nth(J.get(s, "drops"), i, J.nth(J.get(s, "drops"), (lanternward.len(s, "drops") - 1 : U32))), (lanternward.len(s, "drops") - 1 : U32)))

# ---- the fallen ----

# The fallen: a friend holding E gets them up; a long wait brings them to the
# next brazier on their own.

def lanternward.stage.Session.revive_pass?(+s: J.Val) -> J.Val:
  lanternward.stage.revive.each(ga.ids(J.get(s, "seats")), s)

def lanternward.stage.revive.each?(+ids: List<&2, U32>, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+d, +t}:
      lanternward.stage.revive.each(t, lanternward.stage.revive.one(lanternward.is(ga.at(J.get(s, "seats"), d), "act", "Down"), s, d))

def lanternward.stage.revive.one?(down: Bool, +s: J.Val, +d: U32) -> J.Val:
  match down:
    case False{}:
      s
    case True{}:
      lanternward.stage.revive.helped(
        lanternward.stage.revive.helpers(ga.ids(J.get(s, "seats")), J.get(s, "seats"), d, ga.u(ga.at(J.get(s, "seats"), d), "x"), ga.u(ga.at(J.get(s, "seats"), d), "y")),
        s, d)

# Other knights standing, free, holding E beside the fallen.

def lanternward.stage.revive.helpers?(+ids: List<&2, U32>, +seats: J.Val, +d: U32, +x: U32, +y: U32) -> U32:
  match ids:
    case Nil{}:
      0
    case Con{+id, +t}:
      (Bool.to_u32(
        Bool.and(
          Bool.and(U32.is_ne(id, d), lanternward.kit.Seat.up(ga.at(seats, id))),
          Bool.and(
            Bool.and(U32.is_ne(U32.and(ga.u(ga.at(seats, id), "held"), 8), 0), lanternward.is(ga.at(seats, id), "act", "Free")),
            RS.i_lt(lanternward.world.dist((ga.u(ga.at(seats, id), "x") - x : U32), (ga.u(ga.at(seats, id), "y") - y : U32)), 6144))))
        + lanternward.stage.revive.helpers(t, seats, d, x, y) : U32)

def lanternward.stage.revive.helped?(+helpers: U32, +s: J.Val, +d: U32) -> J.Val:
  lanternward.stage.revive.up(
    Bool.pick(U32, U32.is_gt(helpers, 0),
      (ga.u(ga.at(J.get(s, "seats"), d), "rev") + RS.u_min(helpers, 2) : U32),
      RS.u_sat_sub(ga.u(ga.at(J.get(s, "seats"), d), "rev"), 2)),
    s, d)

def lanternward.stage.revive.up?(+rev: U32, +s: J.Val, +d: U32) -> J.Val:
  lanternward.stage.revive.done(
    U32.is_ge(ga.u(ga.at(J.get(s, "seats"), d), "t"), 300),
    rev, lanternward.blow.put_seat(s, d, ga.su(ga.at(J.get(s, "seats"), d), "rev", rev)), d)

def lanternward.stage.revive.done?(+auto: Bool, +rev: U32, +s: J.Val, +d: U32) -> J.Val:
  lanternward.stage.revive.stand(Bool.or(U32.is_ge(rev, 45), auto), auto, s, d)

def lanternward.stage.revive.stand?(go: Bool, +auto: Bool, +s: J.Val, +d: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.stage.revive.stand.go(
        auto,
        lanternward.blow.put_seat(s, d,
          ga.su(ga.su(ga.su(ga.su(ga.sv(ga.at(J.get(s, "seats"), d), "act", J.JS{"Free"}), "t", 0), "rev", 0), "inv", 60),
            "hp", RS.u_max(RS.u_min(Bool.pick(U32, auto, 6, 12), ga.u(ga.at(J.get(s, "seats"), d), "max_hp")), 1))),
        d)

def lanternward.stage.revive.stand.go?(+auto: Bool, +s: J.Val, +d: U32) -> J.Val:
  match auto:
    case True{}:
      lanternward.stage.revive.carry(lanternward.stage.Session.start_of(s, d), s, d)
    case False{}:
      lanternward.sound(
        lanternward.props.Session.fx(s, "Light", ga.u(ga.at(J.get(s, "seats"), d), "x"), ga.u(ga.at(J.get(s, "seats"), d), "y"), 0),
        "potion")

# Carried to the room's resting brazier.

def lanternward.stage.revive.carry?(+start: J.Val, +s: J.Val, +d: U32) -> J.Val:
  lanternward.sound(
    lanternward.props.Session.fx(
      lanternward.blow.put_seat(s, d,
        ga.sv(ga.si(ga.si(ga.at(J.get(s, "seats"), d), "x", ga.nu(start, 0)), "y", ga.nu(start, 1)), "safe", start)),
      "Light", ga.nu(start, 0), ga.nu(start, 1), 0),
    "potion")

# ---- the wipe and the retry ----

def lanternward.stage.all_down.go?(+ids: List<&2, U32>, +seats: J.Val) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+id, +t}:
      Bool.and(Bool.not(lanternward.kit.Seat.up(ga.at(seats, id))), lanternward.stage.all_down.go(t, seats))

# Every knight down: a breath, then the party is back at the room's start.

def lanternward.stage.Session.tick_wipe?(+s: J.Val) -> J.Val:
  lanternward.stage.tick_wipe.go(
    Bool.and(U32.is_gt(lanternward.seat_count(s), 0), lanternward.stage.all_down.go(ga.ids(J.get(s, "seats")), J.get(s, "seats"))), s)

def lanternward.stage.tick_wipe.go?(all_down: Bool, +s: J.Val) -> J.Val:
  match all_down:
    case False{}:
      ga.su(s, "wipe", 0)
    case True{}:
      lanternward.stage.tick_wipe.tick(
        lanternward.sound_if(U32.is_eq((ga.u(s, "wipe") + 1 : U32), 1), ga.su(s, "wipe", (ga.u(s, "wipe") + 1 : U32)), "boss_fall"))

def lanternward.stage.tick_wipe.tick?(+s: J.Val) -> J.Val:
  lanternward.stage.tick_wipe.retry(U32.is_ge(ga.u(s, "wipe"), 24), s)

def lanternward.stage.tick_wipe.retry?(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.stage.Session.retry(s)
    case False{}:
      s

# Back to the room's start: hearts full, foes as the encounter opened, every
# solved thing still solved.

def lanternward.stage.Session.retry?(+s: J.Val) -> J.Val:
  lanternward.stage.retry.room(
    lanternward.stage.retry.key(
      lanternward.stage.retry.seats(
        ga.ids(J.get(s, "seats")),
        ga.sv(ga.sv(ga.sv(ga.su(ga.su(s, "wipe", 0), "tries", RS.u_min((ga.u(s, "tries") + 1 : U32), 4294967295)), "foes", J.JA{Nil{}}), "bolts", J.JA{Nil{}}), "fuses", J.JA{Nil{}}))))

def lanternward.stage.retry.seats?(+ids: List<&2, U32>, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.stage.retry.seats(t, lanternward.stage.retry.seat(lanternward.stage.Session.start_of(s, id), s, id))

def lanternward.stage.retry.seat?(+at: J.Val, +s: J.Val, +id: U32) -> J.Val:
  +k = ga.at(J.get(s, "seats"), id)
  +k1 = ga.si(ga.si(ga.sv(ga.sv(k, "safe", at), "cam", at), "x", ga.nu(at, 0)), "y", ga.nu(at, 1))
  +k2 = ga.su(ga.sv(ga.sv(ga.si(ga.su(ga.sv(ga.su(k1, "hp", ga.u(k, "max_hp")), "act", J.JS{"Free"}), "t", 0), "z", 0), "lock", J.JNull{}), "carry", J.JS{"Nothing"}), "shield", 0)
  +k3 = ga.su(ga.su(ga.su(ga.su(ga.su(ga.sv(k2, "push", ga.pair_i(0, 0)), "chain", 0), "step", 0), "charge", 0), "rev", 0), "pusht", 0)
  lanternward.blow.put_seat(s, id,
    ga.su(ga.su(k3, "inv", 60), "flame", Bool.pick(U32, ga.b(ga.v(s, "inv"), "lantern"), lanternward.R(s, "flame"), 0)))

# Carried keys are put down at the start.

def lanternward.stage.retry.key?(+s: J.Val) -> J.Val:
  lanternward.stage.retry.key.go(lanternward.tag_is(ga.v(ga.v(s, "inv"), "boss"), "Carried"), s)

def lanternward.stage.retry.key.go?(carried: Bool, +s: J.Val) -> J.Val:
  match carried:
    case False{}:
      s
    case True{}:
      lanternward.stage.retry.key.at(ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), s)

def lanternward.stage.retry.key.at?(+id: U32, +s: J.Val) -> J.Val:
  lanternward.stage.retry.key.set(
    Bool.pick(J.Val, ga.has_key(J.get(s, "seats"), id),
      ga.pair_i(ga.u(ga.at(J.get(s, "seats"), id), "x"), ga.u(ga.at(J.get(s, "seats"), id), "y")), ga.pair_i(0, 0)),
    id, s)

def lanternward.stage.retry.key.set?(+at: J.Val, +id: U32, +s: J.Val) -> J.Val:
  ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss", lanternward.variant("Carried", J.JA{[J.JU{id}, J.nth(at, 0), J.nth(at, 1)]})))

def lanternward.stage.retry.room?(+s: J.Val) -> J.Val:
  lanternward.sound(
    ga.su(
      lanternward.stage.retry.by(ga.u(s, "room"), ga.u(s, "stage"), s),
      "flash", RS.u_max(ga.u(s, "flash"), 8)),
    "play_card")

def lanternward.stage.retry.by?(+room: U32, +stage: U32, +s: J.Val) -> J.Val:
  Bool.pick(J.Val, Bool.or(Bool.or(Bool.and(U32.is_eq(room, 0), U32.is_eq(stage, 0)), Bool.and(U32.is_eq(room, 3), U32.is_eq(stage, 0))), U32.is_eq(room, 2)),
    lanternward.stage.Session.provision_room(s, room),
    Bool.pick(J.Val, Bool.and(U32.is_eq(room, 1), U32.is_eq(stage, 2)),
      lanternward.stage.Session.spawn_wave(s),
      Bool.pick(J.Val, U32.is_eq(room, 4), lanternward.boss.Session.reset_boss_phase(s), s)))

# ---- ageing ----

def lanternward.stage.fx_life?(+kind: String) -> U32:
  Bool.pick(U32, lanternward.NAME_IN(kind, ["Puff", "Light"]), 16,
    Bool.pick(U32, lanternward.NAME_IN(kind, ["Spark", "Tick"]), 8,
      Bool.pick(U32, lanternward.NAME_IN(kind, ["Ring", "Dust"]), 10,
        Bool.pick(U32, String.eq(kind, "Parry"), 12,
          Bool.pick(U32, String.eq(kind, "Solved"), 24,
            Bool.pick(U32, String.eq(kind, "Blast"), 14, 20))))))

def lanternward.stage.age.fx?(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      lanternward.stage.age.fx.keep(RS.u_min((ga.u(f, "t") + 1 : U32), 255), f, t)

def lanternward.stage.age.fx.keep?(+age: U32, +f: J.Val, +t: List<&2, J.Val>) -> List<&2, J.Val>:
  Bool.pick(List<&2, J.Val>, U32.is_lt(age, lanternward.stage.fx_life(ga.s(f, "kind"))),
    Con{ga.su(f, "t", age), lanternward.stage.age.fx(t)}, lanternward.stage.age.fx(t))

# The tick's leftovers age out.

def lanternward.stage.Session.age?(+s: J.Val) -> J.Val:
  lanternward.stage.age.banner(
    lanternward.stage.age.cue(
      ga.su(ga.su(ga.sv(s, "fx", J.JA{lanternward.stage.age.fx(ga.items(s, "fx"))}), "shake", RS.u_sat_sub(ga.u(s, "shake"), 1)), "flash", RS.u_sat_sub(ga.u(s, "flash"), 1))))

def lanternward.stage.age.cue?(+s: J.Val) -> J.Val:
  lanternward.stage.age.cue.go(J.is_null(J.get(s, "cue")), s)

def lanternward.stage.age.cue.go?(none: Bool, +s: J.Val) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.stage.age.cue.tick(U32.is_gt(ga.nu(J.get(s, "cue"), 2), 1), s)

def lanternward.stage.age.cue.tick?(live: Bool, +s: J.Val) -> J.Val:
  match live:
    case False{}:
      ga.sv(s, "cue", J.JNull{})
    case True{}:
      ga.sv(s, "cue", J.JA{[J.nth(J.get(s, "cue"), 0), J.nth(J.get(s, "cue"), 1), J.JU{(ga.nu(J.get(s, "cue"), 2) - 1 : U32)}]})

def lanternward.stage.age.banner?(+s: J.Val) -> J.Val:
  lanternward.stage.age.banner.go(J.is_null(J.get(s, "banner")), s)

def lanternward.stage.age.banner.go?(none: Bool, +s: J.Val) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.stage.age.banner.out(
        U32.is_eq(RS.u_sat_sub(ga.u(J.get(s, "banner"), "t"), 1), 0),
        ga.sv(s, "banner", ga.su(J.get(s, "banner"), "t", RS.u_sat_sub(ga.u(J.get(s, "banner"), "t"), 1))))

def lanternward.stage.age.banner.out?(gone: Bool, +s: J.Val) -> J.Val:
  match gone:
    case True{}:
      ga.sv(s, "banner", J.JNull{})
    case False{}:
      s

def lanternward.world.Q?() -> U32:
  256

def lanternward.world.TILE?() -> U32:
  16

def lanternward.world.TILE_SHIFT?() -> U32:
  12

def lanternward.world.ROOMS?() -> U32:
  5

# The integer square root of a u64, rounded down (Rust's `isqrt` on a
# non-negative i64): the root's bits from the top, each kept while its square
# stays under `n`.

def lanternward.world.isqrt.go?(+k: Nat, +n: W.W, +res: U32) -> U32:
  match k:
    case 0n:
      res
    case 1n+p:
      +cand = U32.or(res, U32.shln(1, p))
      lanternward.world.isqrt.go(p, n, Bool.pick(U32, W.le(W.mul32(cand, cand), n), cand, res))

def lanternward.world.isqrt?(+n: W.W) -> W.W:
  W.of_u32(lanternward.world.isqrt.go(32n, n, 0))

# Squares under 2^32 take the cheap root.

def lanternward.world.isqrt.fit?(small: Bool, +n: W.W) -> U32:
  match small:
    case True{}:
      RS.u_isqrt(W.lo32(n))
    case False{}:
      lanternward.world.isqrt.go(32n, n, 0)

# floor(sqrt(n)) of a u64 as a U32 (the root of anything below 2^64 fits).

def lanternward.world.isqrt32?(+n: W.W) -> U32:
  lanternward.world.isqrt.fit(U32.is_zero(W.hi32(n)), n)

# How long `(dx, dy)` is, in its own units.

def lanternward.world.dist?(+dx: U32, +dy: U32) -> U32:
  +ax = RS.i_abs(dx)
  +ay = RS.i_abs(dy)
  lanternward.world.isqrt32(W.add(W.mul32(ax, ax), W.mul32(ay, ay)))

# `a * b / c` in i64, truncated to i32.

def lanternward.world.mul_div.fit?(small: Bool, +ma: U32, +mb: U32, +mc: U32) -> U32:
  match small:
    case True{}:
      U32.div((ma * mb : U32), mc)
    case False{}:
      W.lo32(W.div(W.mul32(ma, mb), W.of_u32(mc)))

def lanternward.world.mul_div?(+a: U32, +b: U32, +c: U32) -> U32:
  +ma = RS.i_abs(a)
  +mb = RS.i_abs(b)
  +q = lanternward.world.mul_div.fit(Bool.and(U32.is_lt(ma, 65536), U32.is_lt(mb, 65536)), ma, mb, RS.i_abs(c))
  Bool.pick(U32, Bool.xor(Bool.xor(RS.i_is_neg(a), RS.i_is_neg(b)), RS.i_is_neg(c)), RS.i_neg(q), q)

# `(dx, dy)` as a vector of length Q, scaled up first when short.

def lanternward.world.unit.mag?(+m: U32, +l: U32, +neg: Bool) -> U32:
  +q = lanternward.world.unit.mag.fit(U32.is_lt(m, 16777216), m, l)
  Bool.pick(U32, neg, RS.i_neg(q), q)

def lanternward.world.unit.mag.fit?(small: Bool, +m: U32, +l: U32) -> U32:
  match small:
    case True{}:
      U32.div((m * 256 : U32), l)
    case False{}:
      W.lo32(W.div_u32(W.mul32(m, 256), l))

def lanternward.world.unit.go?(+ax: U32, +ay: U32, +nx: Bool, +ny: Bool) -> U32 & U32:
  +l = RS.u_max(lanternward.world.isqrt32(W.add(W.mul32(ax, ax), W.mul32(ay, ay))), 1)
  (lanternward.world.unit.mag(ax, l, nx), lanternward.world.unit.mag(ay, l, ny))

def lanternward.world.unit.shift?(+dx: U32, +dy: U32, +sh: U32) -> U32 & U32:
  lanternward.world.unit.go(
    U32.shln(RS.i_abs(dx), U32.to_nat(sh)), U32.shln(RS.i_abs(dy), U32.to_nat(sh)),
    RS.i_is_neg(dx), RS.i_is_neg(dy))

def lanternward.world.unit.big?(+big: U32, +dx: U32, +dy: U32) -> U32 & U32:
  match big:
    case 0:
      (0, 0)
    case _:
      lanternward.world.unit.shift(dx, dy, RS.u_sat_sub(24, W.bitlen32(big)))

def lanternward.world.unit?(+dx: U32, +dy: U32) -> U32 & U32:
  lanternward.world.unit.big(RS.u_max(RS.i_abs(dx), RS.i_abs(dy)), dx, dy)

# The dot product of two Q vectors, in Q: (a.x*b.x + a.y*b.y) / Q in i64.

def lanternward.world.dot4.fit?(small: Bool, +ax: U32, +ay: U32, +bx: U32, +by: U32) -> U32:
  match small:
    case True{}:
      RS.i_div((ax * bx + ay * by : U32), 256)
    case False{}:
      lanternward.world.dot4.big(ax, ay, bx, by)

def lanternward.world.dot4.big?(+ax: U32, +ay: U32, +bx: U32, +by: U32) -> U32:
  lanternward.world.dot4.sum(
    W.mul32(RS.i_abs(ax), RS.i_abs(bx)), Bool.xor(RS.i_is_neg(ax), RS.i_is_neg(bx)),
    W.mul32(RS.i_abs(ay), RS.i_abs(by)), Bool.xor(RS.i_is_neg(ay), RS.i_is_neg(by)))

def lanternward.world.dot4.sum?(+m1: W.W, +n1: Bool, +m2: W.W, +n2: Bool) -> U32:
  +same = Bool.not(Bool.xor(n1, n2))
  +first = W.ge(m1, m2)
  +mag = Bool.pick(W.W, same, W.add(m1, m2), Bool.pick(W.W, first, W.sub(m1, m2), W.sub(m2, m1)))
  +neg = Bool.pick(Bool, same, n1, Bool.pick(Bool, first, n1, n2))
  +q = W.lo32(W.div_u32(mag, 256))
  Bool.pick(U32, neg, RS.i_neg(q), q)

def lanternward.world.dot4?(+ax: U32, +ay: U32, +bx: U32, +by: U32) -> U32:
  lanternward.world.dot4.fit(
    Bool.and(
      Bool.and(U32.is_le(RS.i_abs(ax), 23170), U32.is_le(RS.i_abs(ay), 23170)),
      Bool.and(U32.is_le(RS.i_abs(bx), 23170), U32.is_le(RS.i_abs(by), 23170))),
    ax, ay, bx, by)

# dot((ax, ay), unit(dx, dy)).

def lanternward.world.dot_unit.go?(+ax: U32, +ay: U32, u: U32 & U32) -> U32:
  (ux, uy) = u
  lanternward.world.dot4(ax, ay, ux, uy)

def lanternward.world.dot_unit?(+ax: U32, +ay: U32, +dx: U32, +dy: U32) -> U32:
  lanternward.world.dot_unit.go(ax, ay, lanternward.world.unit(dx, dy))

def lanternward.world.dot?(a: U32 & U32, b: U32 & U32) -> U32:
  (ax, ay) = a
  (bx, by) = b
  lanternward.world.dot4(ax, ay, bx, by)

# Sine of a whole-degree angle in Q (Bhaskara's rational).

def lanternward.world.sin_q.go?(+d: U32, +neg: Bool) -> U32:
  +p = (d * (180 - d : U32) : U32)
  +num = (1024 * p + RS.i_div((40500 - p : U32), 2) : U32)
  +q = RS.i_div(num, (40500 - p : U32))
  Bool.pick(U32, neg, RS.i_neg(q), q)

def lanternward.world.sin_q?(+deg: U32) -> U32:
  +d = RS.i_rem_euclid(deg, 360)
  lanternward.world.sin_q.go(
    Bool.pick(U32, U32.is_ge(d, 180), (d - 180 : U32), d), U32.is_ge(d, 180))

def lanternward.world.cos_q?(+deg: U32) -> U32:
  lanternward.world.sin_q((deg + 90 : U32))

# The Q direction of a whole-degree angle.

def lanternward.world.dir?(+deg: U32) -> U32 & U32:
  (lanternward.world.cos_q(deg), lanternward.world.sin_q(deg))

# The whole degree whose direction lies closest to `u`: of those that point
# its way, the one with the least sideways error.

def lanternward.world.angle_of.go?(+k: Nat, +d: U32, +ux: U32, +uy: U32, +bs: U32, +bd: U32) -> U32:
  match k:
    case 0n:
      bd
    case 1n+p:
      +cx = lanternward.world.cos_q(d)
      +cy = lanternward.world.sin_q(d)
      +side = RS.i_abs((cx * uy - cy * ux : U32))
      +take = Bool.and(RS.i_gt(lanternward.world.dot4(cx, cy, ux, uy), 0), RS.i_lt(side, bs))
      lanternward.world.angle_of.go(p, (d + 1 : U32), ux, uy, Bool.pick(U32, take, side, bs), Bool.pick(U32, take, d, bd))

def lanternward.world.angle_of.u?(u: U32 & U32) -> U32:
  (+ux, +uy) = u
  Bool.pick(U32, Bool.and(U32.is_zero(ux), U32.is_zero(uy)), 0,
    lanternward.world.angle_of.go(360n, 0, ux, uy, 2147483647, 0))

# The whole-degree angle of `(dx, dy)`, 0 for no length.

def lanternward.world.angle_of?(+dx: U32, +dy: U32) -> U32:
  lanternward.world.angle_of.u(lanternward.world.unit(dx, dy))

# The turn from angle `a` to angle `b`, -180 to 180.

def lanternward.world.turn?(+a: U32, +b: U32) -> U32:
  (RS.i_rem_euclid((b - a + 180 : U32), 360) - 180 : U32)

# A tile's column or row of a position in Q pixels.

def lanternward.world.tile_of?(+v: U32) -> U32:
  RS.i_shr(v, 12)

# The middle of tile `t`, in Q pixels.

def lanternward.world.middle?(+t: U32) -> U32:
  (t * 4096 + 2048 : U32)

# The rooms. `plan(room)` clamps to the last room; a "Plan" is its room index.

def lanternward.world.plan?(+room: U32) -> U32:
  RS.u_min(room, 4)

# STAIR

def lanternward.world.rows.stair?() -> List<&2, String>:
  ["######################",
   "#..............==..\"\"#",
   "#.@.@.@.@.@.@..==..\"\"#",
   "#.C..F.........==....#",
   "#..x....\"\"\"\"...==.o..#",
   "#.......\"\"\"\"...==....#",
   "#..s...........==....#",
   "#.....S........==..c.#",
   "#..\"\"\".........==....#",
   "#..\"\"\".....o...==....#",
   "#..............==....#",
   "#.....o.....S..==..o.#",
   "#..o.........o.==....#",
   "#..............==....#",
   "#..............==....#",
   "#########GG###########",
   "#########QQ###########",
   "######################"]

# LAMP

def lanternward.world.rows.lamp?() -> List<&2, String>:
  ["##############################",
   "#..........#.c.#......########",
   "#.oo.......#111#......#VbV####",
   "#.......S.............##V#####",
   "#....B.....................###",
   "#.......o..................###",
   "#....................B.....###",
   "#..........................###",
   "#.E.....\"\"..............S..GQ#",
   "#.......\"\"........\"\"\"......GQ#",
   "#.................\"\"\"......###",
   "#...s.S....................###",
   "#.............B....S.......###",
   "#.C..............o.......oN2c#",
   "#.@.@.@.@.@.@..............###",
   "##############################"]

# GALLERY

def lanternward.world.rows.gallery?() -> List<&2, String>:
  ["######################################",
   "#..o.............#.................###",
   "#......s......o..#.................GQ#",
   "#.@.@.........Y..#...#.............GQ#",
   "#................#...#.............###",
   "#................#................VK##",
   "#.@.@....X....P..1.................###",
   "#................1.................###",
   "#................1...............T.###",
   "#.@.@..\"\"\"\"......#.....#.....#.....###",
   "#......\"\"\"\"......#.....#.....#.....###",
   "#.C....\"\"\"\".o..o.#..o.....#......T.###",
   "#................#.................###",
   "######################~###############",
   "#####################.c.##############",
   "######################################"]

# VAULT

def lanternward.world.rows.vault?() -> List<&2, String>:
  ["################################",
   "#..........##.c..##..........###",
   "#..........##....##..........###",
   "#..........###LL###..........###",
   "#..o.....S..........S........###",
   "#............................###",
   "#............................###",
   "#............................###",
   "#..s.B..................B....MQ#",
   "#.............c..............MQ#",
   "#./..........................###",
   "#......S.................S...###",
   "#............................2c#",
   "#.............S.......S......###",
   "#.C.......................o..###",
   "#.@.@.@.@.@.@....o...........###",
   "#............................###",
   "################################"]

# WARDEN

def lanternward.world.rows.warden?() -> List<&2, String>:
  ["##############################",
   "#............................#",
   "#.o............*...........o.#",
   "#............................#",
   "#............................#",
   "#...B..........W.........B...#",
   "#............................#",
   "#............................#",
   "#............................#",
   "#.......f............f.......#",
   "#............................#",
   "#..........#......#..........#",
   "#............................#",
   "#............................#",
   "#.......f............f.......#",
   "#............................#",
   "#...B....................B...#",
   "#..........#......#..........#",
   "#............................#",
   "#............................#",
   "#............................#",
   "#.o........@.@.@.@.@.@.....o.#",
   "#..............C.............#",
   "##############################"]

def lanternward.world.rows?(+room: U32) -> List<&2, String>:
  match room:
    case 0:
      lanternward.world.rows.stair()
    case 1:
      lanternward.world.rows.lamp()
    case 2:
      lanternward.world.rows.gallery()
    case 3:
      lanternward.world.rows.vault()
    case _:
      lanternward.world.rows.warden()

# Whether the Gloam holds the room dark (the Key Vault).

def lanternward.world.Plan.dark?(+room: U32) -> Bool:
  U32.is_eq(lanternward.world.plan(room), 3)

def lanternward.world.Plan.h.go?(+rows: List<&2, String>) -> U32:
  U32.from_nat(List.length(&2, String, rows))

def lanternward.world.Plan.h?(+room: U32) -> U32:
  lanternward.world.Plan.h.go(lanternward.world.rows(lanternward.world.plan(room)))

def lanternward.world.Plan.w.go?(+rows: List<&2, String>) -> U32:
  match rows:
    case Con{+h, +t}:
      U32.from_nat(String.length(h))
    case Nil{}:
      0

def lanternward.world.Plan.w?(+room: U32) -> U32:
  lanternward.world.Plan.w.go(lanternward.world.rows(lanternward.world.plan(room)))

def lanternward.world.row.go?(+rows: List<&2, String>, k: Nat) -> String:
  match rows k:
    case Nil{} _:
      ""
    case Con{+h, +t} 0n:
      h
    case Con{+h, +t} 1n+p:
      lanternward.world.row.go(t, p)

def lanternward.world.char.go?(s: String, k: Nat) -> U32:
  match s k:
    case SNil{} _:
      35
    case SCon{c, t} 0n:
      Char.to_u32(c)
    case SCon{c, t} 1n+p:
      lanternward.world.char.go(t, p)

def lanternward.world.Plan.at.in?(+room: U32, +tx: U32, +ty: U32) -> U32:
  lanternward.world.char.go(
    lanternward.world.row.go(lanternward.world.rows(room), U32.to_nat(ty)), U32.to_nat(tx))

# The letter at a tile; wall beyond the edge.

def lanternward.world.Plan.at?(+room: U32, +tx: U32, +ty: U32) -> U32:
  +r = lanternward.world.plan(room)
  Bool.pick(U32,
    Bool.or(Bool.or(RS.i_is_neg(tx), RS.i_is_neg(ty)), RS.i_ge(ty, lanternward.world.Plan.h(r))),
    35,
    lanternward.world.Plan.at.in(r, tx, ty))

# Every tile with this letter, row by row, each packed as x + 65536 * y
# (lanternward.world.packed_x / packed_y).

def lanternward.world.Plan.find.row?(s: String, +c: U32, +x: U32, +y: U32) -> List<&2, U32>:
  match s:
    case SNil{}:
      Nil{}
    case SCon{h, t}:
      lanternward.world.Plan.find.hit(U32.is_eq(Char.to_u32(h), c), t, c, x, y)

def lanternward.world.Plan.find.hit?(hit: Bool, s: String, +c: U32, +x: U32, +y: U32) -> List<&2, U32>:
  match hit:
    case True{}:
      Con{(x + y * 65536 : U32), lanternward.world.Plan.find.row(s, c, (x + 1 : U32), y)}
    case False{}:
      lanternward.world.Plan.find.row(s, c, (x + 1 : U32), y)

def lanternward.world.Plan.find.rows?(+rows: List<&2, String>, +c: U32, +y: U32) -> List<&2, U32>:
  match rows:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      List.append(&2, U32, lanternward.world.Plan.find.row(h, c, 0, y),
        lanternward.world.Plan.find.rows(t, c, (y + 1 : U32)))

def lanternward.world.Plan.find?(+room: U32, +c: U32) -> List<&2, U32>:
  lanternward.world.Plan.find.rows(lanternward.world.rows(lanternward.world.plan(room)), c, 0)

# Walls and bars stop a body for good; the rest is for the props.

def lanternward.world.wall?(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 35), U32.is_eq(c, 86))

# The closet a secret hides, as tiles (x0, y0, x1, y1); none outside rooms 1-3.

def lanternward.world.closet?(+room: U32) -> J.Val:
  match room:
    case 1:
      J.JA{[J.JU{28}, J.JU{13}, J.JU{28}, J.JU{13}]}
    case 2:
      J.JA{[J.JU{21}, J.JU{14}, J.JU{23}, J.JU{14}]}
    case 3:
      J.JA{[J.JU{30}, J.JU{12}, J.JU{30}, J.JU{12}]}
    case _:
      J.JNull{}

def lanternward.world.packed_x?(+p: U32) -> U32:
  U32.and(p, 65535)

def lanternward.world.packed_y?(+p: U32) -> U32:
  U32.shrn(p, 16n)

# ---- cases ----

# A pair as the JSON array [x, y] of two i32.

def lanternward.pair_j?(p: U32 & U32) -> J.Val:
  (x, y) = p
  ga.pair_i(x, y)

def case.lanternward.world.isqrt?(+input: J.Val) -> J.Val:
  W.to_j(lanternward.world.isqrt(W.of_j(J.get(input, "n"))))

def case.lanternward.world.dist?(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.dist(ga.u(input, "dx"), ga.u(input, "dy")))

def case.lanternward.world.unit?(+input: J.Val) -> J.Val:
  lanternward.pair_j(lanternward.world.unit(ga.u(input, "dx"), ga.u(input, "dy")))

def case.lanternward.world.dot?(+input: J.Val) -> J.Val:
  +a = J.get(input, "a")
  +b = J.get(input, "b")
  J.of_i32(lanternward.world.dot4(ga.nu(a, 0), ga.nu(a, 1), ga.nu(b, 0), ga.nu(b, 1)))

def case.lanternward.world.angle_of?(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.angle_of(ga.u(input, "dx"), ga.u(input, "dy")))

def case.lanternward.world.sin_q?(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.sin_q(ga.u(input, "deg")))

def case.lanternward.world.cos_q?(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.cos_q(ga.u(input, "deg")))

def case.lanternward.world.dir?(+input: J.Val) -> J.Val:
  lanternward.pair_j(lanternward.world.dir(ga.u(input, "deg")))

def case.lanternward.world.turn?(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.turn(ga.u(input, "a"), ga.u(input, "b")))

def case.lanternward.world.tile_of?(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.tile_of(ga.u(input, "v")))

def case.lanternward.world.middle?(+input: J.Val) -> J.Val:
  J.of_i32(lanternward.world.middle(ga.u(input, "t")))

def case.lanternward.world.Plan.w?(+input: J.Val) -> J.Val:
  J.JU{lanternward.world.Plan.w(ga.u(input, "room"))}

def case.lanternward.world.Plan.h?(+input: J.Val) -> J.Val:
  J.JU{lanternward.world.Plan.h(ga.u(input, "room"))}

def case.lanternward.world.Plan.dark?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.world.Plan.dark(ga.u(input, "room"))}

def case.lanternward.world.closet?(+input: J.Val) -> J.Val:
  lanternward.world.closet(ga.u(input, "room"))

def case.lanternward.world.Plan.at?(+input: J.Val) -> J.Val:
  J.JU{lanternward.world.Plan.at(ga.u(input, "room"), ga.u(input, "tx"), ga.u(input, "ty"))}

def lanternward.packed_j?(+xs: List<&2, U32>) -> J.Val:
  match xs:
    case Nil{}:
      J.JA{Nil{}}
    case Con{+h, +t}:
      J.JA{Con{ga.pair_i(lanternward.world.packed_x(h), lanternward.world.packed_y(h)), J.items(lanternward.packed_j(t))}}

def case.lanternward.world.Plan.find?(+input: J.Val) -> J.Val:
  lanternward.packed_j(lanternward.world.Plan.find(ga.u(input, "room"), ga.u(input, "c")))

def case.lanternward.world.wall?(+input: J.Val) -> J.Val:
  J.JBool{lanternward.world.wall(ga.u(input, "c"))}

def roundtable.number?(+tune: J.Val, +slot: U32, +i: U32) -> U32:
  roundtable.number.of(J.nth(J.nth(J.get(tune, "roundtable"), slot), i), i)

def roundtable.number.of?(+v: J.Val, +i: U32) -> U32:
  match v:
    case J.JNull{}:
      Bool.pick(U32, U32.is_eq(i, 0), 112, Bool.pick(U32, U32.is_eq(i, 1), 8, Bool.pick(U32, U32.is_eq(i, 2), 14, Bool.pick(U32, U32.is_eq(i, 3), 150, 56))))
    case _:
      J.u32(v)

# Input::valid / Input::default (core).

def rules.Input.valid?(+i: J.Val) -> Bool:
  Bool.and(
    Bool.and(
      Bool.or(U32.is_le(J.u32(J.get(i, "move_x")), 1), U32.is_eq(J.u32(J.get(i, "move_x")), 4294967295)),
      Bool.or(U32.is_le(J.u32(J.get(i, "move_y")), 1), U32.is_eq(J.u32(J.get(i, "move_y")), 4294967295))),
    Bool.and(
      Bool.or(U32.is_le(J.u32(J.get(i, "aim_x")), 1), U32.is_eq(J.u32(J.get(i, "aim_x")), 4294967295)),
      Bool.or(U32.is_le(J.u32(J.get(i, "aim_y")), 1), U32.is_eq(J.u32(J.get(i, "aim_y")), 4294967295))))

def rules.Input.default?() -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(),
    "move_x", J.JU{0}), "move_y", J.JU{0}), "aim_x", J.JU{0}), "aim_y", J.JU{0}), "fire", J.JBool{False{}}),
    "dash", J.JBool{False{}}), "bomb", J.JBool{False{}}), "swing", J.JBool{False{}}), "play", J.JU{0}), "cast", J.JU{0})

# Run::room (the room the party stands in).

def rules.Run.room?(+run: J.Val) -> J.Val:
  J.nth(J.get(J.get(run, "dungeon"), "rooms"), J.u32(J.get(run, "at")))

# forest::Forest::leave

def forest.Forest.leave?(+forest: J.Val) -> J.Val:
  forest

# hitstop::State::request

def hitstop.State.request?(+hs: J.Val, +tune: J.Val, +impact: String) -> J.Val:
  hs

# exits: Run::leave_activity

def exits.Run.leave_activity?(+run: J.Val, +id: U32) -> J.Val:
  run

# realm::Spoils::add (the Spoils is an object keyed by spoil name).

def realm.Spoils.add?(+sp: J.Val, +spoil: String, +n: U32) -> J.Val:
  Bool.pick(J.Val, U32.is_gt(n, 0), J.set(sp, spoil, J.JU{RS.u_sat_add(J.u32(J.get(sp, spoil)), n)}), sp)

def cases.run?(+name: String, +input: J.Val) -> J.Val:
  cases.run.n0(String.eq(name, "lanternward.ACTIVITY.end"), name, input)

def cases.run.n0?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ACTIVITY.end(input)
    case False{}:
      cases.run.n1(String.eq(name, "lanternward.ACTIVITY.on"), name, input)

def cases.run.n1?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ACTIVITY.on(input)
    case False{}:
      cases.run.n2(String.eq(name, "lanternward.ACTIVITY.ride"), name, input)

def cases.run.n2?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ACTIVITY.ride(input)
    case False{}:
      cases.run.n3(String.eq(name, "lanternward.ADVENTURE.bot"), name, input)

def cases.run.n3?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ADVENTURE.bot(input)
    case False{}:
      cases.run.n4(String.eq(name, "lanternward.ADVENTURE.clear"), name, input)

def cases.run.n4?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ADVENTURE.clear(input)
    case False{}:
      cases.run.n5(String.eq(name, "lanternward.ADVENTURE.seated"), name, input)

def cases.run.n5?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ADVENTURE.seated(input)
    case False{}:
      cases.run.n6(String.eq(name, "lanternward.ADVENTURE.valid"), name, input)

def cases.run.n6?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ADVENTURE.valid(input)
    case False{}:
      cases.run.n7(String.eq(name, "lanternward.Rules.tell_for"), name, input)

def cases.run.n7?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Rules.tell_for(input)
    case False{}:
      cases.run.n8(String.eq(name, "lanternward.Run.finish_lanternward"), name, input)

def cases.run.n8?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Run.finish_lanternward(input)
    case False{}:
      cases.run.n9(String.eq(name, "lanternward.Run.lanternward_showing"), name, input)

def cases.run.n9?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Run.lanternward_showing(input)
    case False{}:
      cases.run.n10(String.eq(name, "lanternward.Run.start_lanternward"), name, input)

def cases.run.n10?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Run.start_lanternward(input)
    case False{}:
      cases.run.n11(String.eq(name, "lanternward.Run.tick_lanternward"), name, input)

def cases.run.n11?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Run.tick_lanternward(input)
    case False{}:
      cases.run.n12(String.eq(name, "lanternward.Session.bot_input"), name, input)

def cases.run.n12?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Session.bot_input(input)
    case False{}:
      cases.run.n13(String.eq(name, "lanternward.Session.finale"), name, input)

def cases.run.n13?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Session.finale(input)
    case False{}:
      cases.run.n14(String.eq(name, "lanternward.Session.new"), name, input)

def cases.run.n14?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Session.new(input)
    case False{}:
      cases.run.n15(String.eq(name, "lanternward.Session.problem"), name, input)

def cases.run.n15?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Session.problem(input)
    case False{}:
      cases.run.n16(String.eq(name, "lanternward.Session.repair"), name, input)

def cases.run.n16?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Session.repair(input)
    case False{}:
      cases.run.n17(String.eq(name, "lanternward.Session.step"), name, input)

def cases.run.n17?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Session.step(input)
    case False{}:
      cases.run.n18(String.eq(name, "lanternward.Session.valid"), name, input)

def cases.run.n18?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.Session.valid(input)
    case False{}:
      cases.run.n19(String.eq(name, "lanternward.blow.Session.blow"), name, input)

def cases.run.n19?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.blow.Session.blow(input)
    case False{}:
      cases.run.n20(String.eq(name, "lanternward.blow.Session.bolt_ends"), name, input)

def cases.run.n20?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.blow.Session.bolt_ends(input)
    case False{}:
      cases.run.n21(String.eq(name, "lanternward.blow.Session.bolt_meets"), name, input)

def cases.run.n21?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.blow.Session.bolt_meets(input)
    case False{}:
      cases.run.n22(String.eq(name, "lanternward.blow.Session.hurt_foe"), name, input)

def cases.run.n22?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.blow.Session.hurt_foe(input)
    case False{}:
      cases.run.n23(String.eq(name, "lanternward.blow.Session.light_foe"), name, input)

def cases.run.n23?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.blow.Session.light_foe(input)
    case False{}:
      cases.run.n24(String.eq(name, "lanternward.blow.Session.step_bolts"), name, input)

def cases.run.n24?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.blow.Session.step_bolts(input)
    case False{}:
      cases.run.n25(String.eq(name, "lanternward.blow.Session.step_fuses"), name, input)

def cases.run.n25?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.blow.Session.step_fuses(input)
    case False{}:
      cases.run.n26(String.eq(name, "lanternward.blow.Session.turn_bolt"), name, input)

def cases.run.n26?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.blow.Session.turn_bolt(input)
    case False{}:
      cases.run.n27(String.eq(name, "lanternward.boss.Session.begin_attack"), name, input)

def cases.run.n27?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Session.begin_attack(input)
    case False{}:
      cases.run.n28(String.eq(name, "lanternward.boss.Session.blast_boss"), name, input)

def cases.run.n28?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Session.blast_boss(input)
    case False{}:
      cases.run.n29(String.eq(name, "lanternward.boss.Session.boss_exposed"), name, input)

def cases.run.n29?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Session.boss_exposed(input)
    case False{}:
      cases.run.n30(String.eq(name, "lanternward.boss.Session.hurt_boss"), name, input)

def cases.run.n30?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Session.hurt_boss(input)
    case False{}:
      cases.run.n31(String.eq(name, "lanternward.boss.Session.light_heart"), name, input)

def cases.run.n31?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Session.light_heart(input)
    case False{}:
      cases.run.n32(String.eq(name, "lanternward.boss.Session.prop_at"), name, input)

def cases.run.n32?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Session.prop_at(input)
    case False{}:
      cases.run.n33(String.eq(name, "lanternward.boss.Session.reset_boss_phase"), name, input)

def cases.run.n33?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Session.reset_boss_phase(input)
    case False{}:
      cases.run.n34(String.eq(name, "lanternward.boss.Session.stage_warden"), name, input)

def cases.run.n34?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Session.stage_warden(input)
    case False{}:
      cases.run.n35(String.eq(name, "lanternward.boss.Session.step_boss"), name, input)

def cases.run.n35?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Session.step_boss(input)
    case False{}:
      cases.run.n36(String.eq(name, "lanternward.boss.Session.summon"), name, input)

def cases.run.n36?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Session.summon(input)
    case False{}:
      cases.run.n37(String.eq(name, "lanternward.boss.Session.warden_hits"), name, input)

def cases.run.n37?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Session.warden_hits(input)
    case False{}:
      cases.run.n38(String.eq(name, "lanternward.boss.Warden.exposed"), name, input)

def cases.run.n38?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Warden.exposed(input)
    case False{}:
      cases.run.n39(String.eq(name, "lanternward.boss.Warden.heart"), name, input)

def cases.run.n39?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Warden.heart(input)
    case False{}:
      cases.run.n40(String.eq(name, "lanternward.boss.Warden.need"), name, input)

def cases.run.n40?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Warden.need(input)
    case False{}:
      cases.run.n41(String.eq(name, "lanternward.boss.Warden.new"), name, input)

def cases.run.n41?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Warden.new(input)
    case False{}:
      cases.run.n42(String.eq(name, "lanternward.boss.Warden.valid"), name, input)

def cases.run.n42?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.boss.Warden.valid(input)
    case False{}:
      cases.run.n43(String.eq(name, "lanternward.bot.Session.bot_input"), name, input)

def cases.run.n43?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.bot.Session.bot_input(input)
    case False{}:
      cases.run.n44(String.eq(name, "lanternward.ctx.Session.beacon_ready"), name, input)

def cases.run.n44?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ctx.Session.beacon_ready(input)
    case False{}:
      cases.run.n45(String.eq(name, "lanternward.ctx.Session.burn_ticks"), name, input)

def cases.run.n45?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ctx.Session.burn_ticks(input)
    case False{}:
      cases.run.n46(String.eq(name, "lanternward.ctx.Session.carrying_boss_key"), name, input)

def cases.run.n46?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ctx.Session.carrying_boss_key(input)
    case False{}:
      cases.run.n47(String.eq(name, "lanternward.ctx.Session.context"), name, input)

def cases.run.n47?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ctx.Session.context(input)
    case False{}:
      cases.run.n48(String.eq(name, "lanternward.ctx.Session.grant"), name, input)

def cases.run.n48?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.ctx.Session.grant(input)
    case False{}:
      cases.run.n49(String.eq(name, "lanternward.foes.Danger.hits"), name, input)

def cases.run.n49?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.Danger.hits(input)
    case False{}:
      cases.run.n50(String.eq(name, "lanternward.foes.Foe.new"), name, input)

def cases.run.n50?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.Foe.new(input)
    case False{}:
      cases.run.n51(String.eq(name, "lanternward.foes.Foe.tell"), name, input)

def cases.run.n51?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.Foe.tell(input)
    case False{}:
      cases.run.n52(String.eq(name, "lanternward.foes.Foe.vulnerable"), name, input)

def cases.run.n52?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.Foe.vulnerable(input)
    case False{}:
      cases.run.n53(String.eq(name, "lanternward.foes.Session.nearest_knight"), name, input)

def cases.run.n53?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.Session.nearest_knight(input)
    case False{}:
      cases.run.n54(String.eq(name, "lanternward.foes.Session.nudge"), name, input)

def cases.run.n54?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.Session.nudge(input)
    case False{}:
      cases.run.n55(String.eq(name, "lanternward.foes.Session.provision"), name, input)

def cases.run.n55?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.Session.provision(input)
    case False{}:
      cases.run.n56(String.eq(name, "lanternward.foes.Session.spawn_foe"), name, input)

def cases.run.n56?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.Session.spawn_foe(input)
    case False{}:
      cases.run.n57(String.eq(name, "lanternward.foes.Session.step_foes"), name, input)

def cases.run.n57?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.Session.step_foes(input)
    case False{}:
      cases.run.n58(String.eq(name, "lanternward.foes.Session.strike_with"), name, input)

def cases.run.n58?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.Session.strike_with(input)
    case False{}:
      cases.run.n59(String.eq(name, "lanternward.foes.step_snuffer"), name, input)

def cases.run.n59?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.step_snuffer(input)
    case False{}:
      cases.run.n60(String.eq(name, "lanternward.foes.step_turret"), name, input)

def cases.run.n60?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.foes.step_turret(input)
    case False{}:
      cases.run.n61(String.eq(name, "lanternward.hint.context"), name, input)

def cases.run.n61?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.hint.context(input)
    case False{}:
      cases.run.n62(String.eq(name, "lanternward.hint.hint"), name, input)

def cases.run.n62?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.hint.hint(input)
    case False{}:
      cases.run.n63(String.eq(name, "lanternward.hint.line"), name, input)

def cases.run.n63?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.hint.line(input)
    case False{}:
      cases.run.n64(String.eq(name, "lanternward.hint.lock_hint"), name, input)

def cases.run.n64?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.hint.lock_hint(input)
    case False{}:
      cases.run.n65(String.eq(name, "lanternward.kit.Seat.flaring"), name, input)

def cases.run.n65?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Seat.flaring(input)
    case False{}:
      cases.run.n66(String.eq(name, "lanternward.kit.Seat.new"), name, input)

def cases.run.n66?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Seat.new(input)
    case False{}:
      cases.run.n67(String.eq(name, "lanternward.kit.Seat.up"), name, input)

def cases.run.n67?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Seat.up(input)
    case False{}:
      cases.run.n68(String.eq(name, "lanternward.kit.Seat.valid"), name, input)

def cases.run.n68?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Seat.valid(input)
    case False{}:
      cases.run.n69(String.eq(name, "lanternward.kit.Session.begin_dodge"), name, input)

def cases.run.n69?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.begin_dodge(input)
    case False{}:
      cases.run.n70(String.eq(name, "lanternward.kit.Session.begin_lunge"), name, input)

def cases.run.n70?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.begin_lunge(input)
    case False{}:
      cases.run.n71(String.eq(name, "lanternward.kit.Session.begin_slash"), name, input)

def cases.run.n71?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.begin_slash(input)
    case False{}:
      cases.run.n72(String.eq(name, "lanternward.kit.Session.begin_spin"), name, input)

def cases.run.n72?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.begin_spin(input)
    case False{}:
      cases.run.n73(String.eq(name, "lanternward.kit.Session.camera"), name, input)

def cases.run.n73?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.camera(input)
    case False{}:
      cases.run.n74(String.eq(name, "lanternward.kit.Session.cycle_target"), name, input)

def cases.run.n74?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.cycle_target(input)
    case False{}:
      cases.run.n75(String.eq(name, "lanternward.kit.Session.face_and_move"), name, input)

def cases.run.n75?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.face_and_move(input)
    case False{}:
      cases.run.n76(String.eq(name, "lanternward.kit.Session.falling"), name, input)

def cases.run.n76?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.falling(input)
    case False{}:
      cases.run.n77(String.eq(name, "lanternward.kit.Session.free"), name, input)

def cases.run.n77?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.free(input)
    case False{}:
      cases.run.n78(String.eq(name, "lanternward.kit.Session.hopping"), name, input)

def cases.run.n78?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.hopping(input)
    case False{}:
      cases.run.n79(String.eq(name, "lanternward.kit.Session.hurting"), name, input)

def cases.run.n79?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.hurting(input)
    case False{}:
      cases.run.n80(String.eq(name, "lanternward.kit.Session.lantern"), name, input)

def cases.run.n80?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.lantern(input)
    case False{}:
      cases.run.n81(String.eq(name, "lanternward.kit.Session.lean"), name, input)

def cases.run.n81?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.lean(input)
    case False{}:
      cases.run.n82(String.eq(name, "lanternward.kit.Session.lock_holds"), name, input)

def cases.run.n82?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.lock_holds(input)
    case False{}:
      cases.run.n83(String.eq(name, "lanternward.kit.Session.lock_on"), name, input)

def cases.run.n83?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.lock_on(input)
    case False{}:
      cases.run.n84(String.eq(name, "lanternward.kit.Session.lock_point"), name, input)

def cases.run.n84?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.lock_point(input)
    case False{}:
      cases.run.n85(String.eq(name, "lanternward.kit.Session.lunge_hit"), name, input)

def cases.run.n85?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.lunge_hit(input)
    case False{}:
      cases.run.n86(String.eq(name, "lanternward.kit.Session.orbit"), name, input)

def cases.run.n86?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.orbit(input)
    case False{}:
      cases.run.n87(String.eq(name, "lanternward.kit.Session.pick_target"), name, input)

def cases.run.n87?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.pick_target(input)
    case False{}:
      cases.run.n88(String.eq(name, "lanternward.kit.Session.plunging"), name, input)

def cases.run.n88?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.plunging(input)
    case False{}:
      cases.run.n89(String.eq(name, "lanternward.kit.Session.recovering"), name, input)

def cases.run.n89?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.recovering(input)
    case False{}:
      cases.run.n90(String.eq(name, "lanternward.kit.Session.refill"), name, input)

def cases.run.n90?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.refill(input)
    case False{}:
      cases.run.n91(String.eq(name, "lanternward.kit.Session.rolling"), name, input)

def cases.run.n91?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.rolling(input)
    case False{}:
      cases.run.n92(String.eq(name, "lanternward.kit.Session.shove"), name, input)

def cases.run.n92?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.shove(input)
    case False{}:
      cases.run.n93(String.eq(name, "lanternward.kit.Session.step_seat"), name, input)

def cases.run.n93?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.step_seat(input)
    case False{}:
      cases.run.n94(String.eq(name, "lanternward.kit.Session.sweep"), name, input)

def cases.run.n94?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.sweep(input)
    case False{}:
      cases.run.n95(String.eq(name, "lanternward.kit.Session.swinging"), name, input)

def cases.run.n95?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.swinging(input)
    case False{}:
      cases.run.n96(String.eq(name, "lanternward.kit.Session.take_lock"), name, input)

def cases.run.n96?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.take_lock(input)
    case False{}:
      cases.run.n97(String.eq(name, "lanternward.kit.Session.targets"), name, input)

def cases.run.n97?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.targets(input)
    case False{}:
      cases.run.n98(String.eq(name, "lanternward.kit.Session.upkeep"), name, input)

def cases.run.n98?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.upkeep(input)
    case False{}:
      cases.run.n99(String.eq(name, "lanternward.kit.Session.verbs"), name, input)

def cases.run.n99?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.Session.verbs(input)
    case False{}:
      cases.run.n100(String.eq(name, "lanternward.kit.buttons"), name, input)

def cases.run.n100?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.kit.buttons(input)
    case False{}:
      cases.run.n101(String.eq(name, "lanternward.props.Session.blocked"), name, input)

def cases.run.n101?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.blocked(input)
    case False{}:
      cases.run.n102(String.eq(name, "lanternward.props.Session.burning"), name, input)

def cases.run.n102?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.burning(input)
    case False{}:
      cases.run.n103(String.eq(name, "lanternward.props.Session.clear"), name, input)

def cases.run.n103?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.clear(input)
    case False{}:
      cases.run.n104(String.eq(name, "lanternward.props.Session.drop_at"), name, input)

def cases.run.n104?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.drop_at(input)
    case False{}:
      cases.run.n105(String.eq(name, "lanternward.props.Session.fx"), name, input)

def cases.run.n105?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.fx(input)
    case False{}:
      cases.run.n106(String.eq(name, "lanternward.props.Session.line_open"), name, input)

def cases.run.n106?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.line_open(input)
    case False{}:
      cases.run.n107(String.eq(name, "lanternward.props.Session.pit"), name, input)

def cases.run.n107?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.pit(input)
    case False{}:
      cases.run.n108(String.eq(name, "lanternward.props.Session.roll"), name, input)

def cases.run.n108?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.roll(input)
    case False{}:
      cases.run.n109(String.eq(name, "lanternward.props.Session.scatter"), name, input)

def cases.run.n109?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.scatter(input)
    case False{}:
      cases.run.n110(String.eq(name, "lanternward.props.Session.secret_open"), name, input)

def cases.run.n110?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.secret_open(input)
    case False{}:
      cases.run.n111(String.eq(name, "lanternward.props.Session.set_gate"), name, input)

def cases.run.n111?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.set_gate(input)
    case False{}:
      cases.run.n112(String.eq(name, "lanternward.props.Session.shatter"), name, input)

def cases.run.n112?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.shatter(input)
    case False{}:
      cases.run.n113(String.eq(name, "lanternward.props.Session.solved"), name, input)

def cases.run.n113?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.props.Session.solved(input)
    case False{}:
      cases.run.n114(String.eq(name, "lanternward.purse"), name, input)

def cases.run.n114?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.purse(input)
    case False{}:
      cases.run.n115(String.eq(name, "lanternward.stage.Session.age"), name, input)

def cases.run.n115?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.age(input)
    case False{}:
      cases.run.n116(String.eq(name, "lanternward.stage.Session.all_lit"), name, input)

def cases.run.n116?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.all_lit(input)
    case False{}:
      cases.run.n117(String.eq(name, "lanternward.stage.Session.enter_room"), name, input)

def cases.run.n117?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.enter_room(input)
    case False{}:
      cases.run.n118(String.eq(name, "lanternward.stage.Session.exit_open"), name, input)

def cases.run.n118?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.exit_open(input)
    case False{}:
      cases.run.n119(String.eq(name, "lanternward.stage.Session.foes_down"), name, input)

def cases.run.n119?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.foes_down(input)
    case False{}:
      cases.run.n120(String.eq(name, "lanternward.stage.Session.open_exit"), name, input)

def cases.run.n120?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.open_exit(input)
    case False{}:
      cases.run.n121(String.eq(name, "lanternward.stage.Session.pickups"), name, input)

def cases.run.n121?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.pickups(input)
    case False{}:
      cases.run.n122(String.eq(name, "lanternward.stage.Session.prop_of"), name, input)

def cases.run.n122?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.prop_of(input)
    case False{}:
      cases.run.n123(String.eq(name, "lanternward.stage.Session.provision_room"), name, input)

def cases.run.n123?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.provision_room(input)
    case False{}:
      cases.run.n124(String.eq(name, "lanternward.stage.Session.retry"), name, input)

def cases.run.n124?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.retry(input)
    case False{}:
      cases.run.n125(String.eq(name, "lanternward.stage.Session.revive_pass"), name, input)

def cases.run.n125?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.revive_pass(input)
    case False{}:
      cases.run.n126(String.eq(name, "lanternward.stage.Session.spawn_wave"), name, input)

def cases.run.n126?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.spawn_wave(input)
    case False{}:
      cases.run.n127(String.eq(name, "lanternward.stage.Session.stage_gallery"), name, input)

def cases.run.n127?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.stage_gallery(input)
    case False{}:
      cases.run.n128(String.eq(name, "lanternward.stage.Session.stage_lamp_hall"), name, input)

def cases.run.n128?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.stage_lamp_hall(input)
    case False{}:
      cases.run.n129(String.eq(name, "lanternward.stage.Session.stage_vault"), name, input)

def cases.run.n129?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.stage_vault(input)
    case False{}:
      cases.run.n130(String.eq(name, "lanternward.stage.Session.start_of"), name, input)

def cases.run.n130?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.start_of(input)
    case False{}:
      cases.run.n131(String.eq(name, "lanternward.stage.Session.tick_props"), name, input)

def cases.run.n131?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.tick_props(input)
    case False{}:
      cases.run.n132(String.eq(name, "lanternward.stage.Session.tick_stage"), name, input)

def cases.run.n132?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.tick_stage(input)
    case False{}:
      cases.run.n133(String.eq(name, "lanternward.stage.Session.tick_wipe"), name, input)

def cases.run.n133?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.tick_wipe(input)
    case False{}:
      cases.run.n134(String.eq(name, "lanternward.stage.Session.unstick"), name, input)

def cases.run.n134?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.stage.Session.unstick(input)
    case False{}:
      cases.run.n135(String.eq(name, "lanternward.world.Plan.at"), name, input)

def cases.run.n135?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.Plan.at(input)
    case False{}:
      cases.run.n136(String.eq(name, "lanternward.world.Plan.dark"), name, input)

def cases.run.n136?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.Plan.dark(input)
    case False{}:
      cases.run.n137(String.eq(name, "lanternward.world.Plan.find"), name, input)

def cases.run.n137?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.Plan.find(input)
    case False{}:
      cases.run.n138(String.eq(name, "lanternward.world.Plan.h"), name, input)

def cases.run.n138?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.Plan.h(input)
    case False{}:
      cases.run.n139(String.eq(name, "lanternward.world.Plan.w"), name, input)

def cases.run.n139?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.Plan.w(input)
    case False{}:
      cases.run.n140(String.eq(name, "lanternward.world.angle_of"), name, input)

def cases.run.n140?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.angle_of(input)
    case False{}:
      cases.run.n141(String.eq(name, "lanternward.world.closet"), name, input)

def cases.run.n141?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.closet(input)
    case False{}:
      cases.run.n142(String.eq(name, "lanternward.world.cos_q"), name, input)

def cases.run.n142?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.cos_q(input)
    case False{}:
      cases.run.n143(String.eq(name, "lanternward.world.dir"), name, input)

def cases.run.n143?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.dir(input)
    case False{}:
      cases.run.n144(String.eq(name, "lanternward.world.dist"), name, input)

def cases.run.n144?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.dist(input)
    case False{}:
      cases.run.n145(String.eq(name, "lanternward.world.dot"), name, input)

def cases.run.n145?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.dot(input)
    case False{}:
      cases.run.n146(String.eq(name, "lanternward.world.isqrt"), name, input)

def cases.run.n146?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.isqrt(input)
    case False{}:
      cases.run.n147(String.eq(name, "lanternward.world.middle"), name, input)

def cases.run.n147?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.middle(input)
    case False{}:
      cases.run.n148(String.eq(name, "lanternward.world.sin_q"), name, input)

def cases.run.n148?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.sin_q(input)
    case False{}:
      cases.run.n149(String.eq(name, "lanternward.world.tile_of"), name, input)

def cases.run.n149?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.tile_of(input)
    case False{}:
      cases.run.n150(String.eq(name, "lanternward.world.turn"), name, input)

def cases.run.n150?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.turn(input)
    case False{}:
      cases.run.n151(String.eq(name, "lanternward.world.unit"), name, input)

def cases.run.n151?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.unit(input)
    case False{}:
      cases.run.n152(String.eq(name, "lanternward.world.wall"), name, input)

def cases.run.n152?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.lanternward.world.wall(input)
    case False{}:
      J.JS{"no case wrapper"}
