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

def griffinwing.boss.HEAD?() -> U32:
  griffinwing.types.dm(340)

# How long a lamp's shield stays open after a turned bolt, in ticks.

def griffinwing.boss.OPEN?() -> U32:
  150

def griffinwing.boss.RISE_T?() -> U32:
  90

def griffinwing.boss.HURT_T?() -> U32:
  70

# The lamps' places about the head, across and up.

def griffinwing.boss.LAMP.x?(+k: U32) -> U32:
  Bool.pick(U32, U32.is_eq(k, 0), gw.neg(griffinwing.types.dm(45)), Bool.pick(U32, U32.is_eq(k, 1), griffinwing.types.dm(45), 0))

def griffinwing.boss.LAMP.z?(+k: U32) -> U32:
  Bool.pick(U32, U32.is_eq(k, 2), griffinwing.types.dm(42), griffinwing.types.dm(10))

# How far in front of its face the lamps hang.

def griffinwing.boss.FRONT?() -> U32:
  griffinwing.types.dm(40)

# The head's height above the rail's line, resting.

def griffinwing.boss.HIGH?() -> U32:
  griffinwing.types.dm(12)

# How wide the coils' gap and the roar's column are, half.

def griffinwing.boss.GAP_HALF?() -> U32:
  griffinwing.types.dm(30)

def griffinwing.boss.COL_HALF?() -> U32:
  griffinwing.types.dm(24)

def griffinwing.boss.COIL_T?() -> U32:
  22

def griffinwing.boss.ROAR_T?() -> U32:
  26

# What a coil or the roar does to a craft it catches.

def griffinwing.boss.BLOW?() -> U32:
  35

# The Wyrm rises out of the roost (or, after a fall, is back in its place, its
# lamps as they were).

def griffinwing.boss.Session.rise?(+s0: J.Val) -> J.Val:
  griffinwing.boss.Session.rise.go(ga.su(s0, "cp", griffinwing.rail.BOSS_AT()))

def griffinwing.boss.Session.rise.go?(+s: J.Val) -> J.Val:
  gw.ifj(Bool.not(J.is_null(J.get(s, "boss"))),
    _ => ga.sv(s, "boss", ga.su(gw.sets(ga.sv(J.get(s, "boss"), "marks", J.JA{[]}), "st", "Rise"), "ticks", griffinwing.boss.RISE_T())),
    _ => griffinwing.boss.Session.rise.make(s))

# Lamp `k`, spawned: the new session in {"s", "r"}; the lamps' ids in `ids`.

def griffinwing.boss.Session.rise.make?(+s: J.Val) -> J.Val:
  griffinwing.boss.Session.rise.fin(
    griffinwing.boss.Session.rise.lamp(
      griffinwing.boss.Session.rise.lamp(
        griffinwing.boss.Session.rise.lamp(J.set(J.set(J.obj(), "s", s), "ids", J.JA{[]}), 0),
        1),
      2))

def griffinwing.boss.Session.rise.lamp?(+w: J.Val, +k: U32) -> J.Val:
  griffinwing.boss.Session.rise.lamp2(gw.ps(w), J.get(w, "ids"),
    griffinwing.combat.Session.spawn_foe(gw.ps(w), "Lamp",
      (griffinwing.Session.anchor(gw.ps(w)) + griffinwing.boss.HEAD() - griffinwing.boss.FRONT() : U32),
      griffinwing.boss.LAMP.x(k), (griffinwing.boss.HIGH() + griffinwing.boss.LAMP.z(k) : U32),
      0, 0, 0, "Shut", k, 0))

def griffinwing.boss.Session.rise.lamp2?(+s0: J.Val, +ids: J.Val, +r: J.Val) -> J.Val:
  J.set(J.set(J.obj(), "s", gw.out_s(r)), "ids", J.push(ids, gw.ifj(J.is_null(gw.out_r(r)), _ => J.JU{0}, _ => gw.out_r(r))))

def griffinwing.boss.Session.rise.fin?(+w: J.Val) -> J.Val:
  griffinwing.Session.sound(
    griffinwing.Session.say(
      ga.sv(gw.ps(w), "boss", griffinwing.boss.Beast.new(J.get(w, "ids"))),
      griffinwing.Session.voice(gw.ps(w)), "Wyrm"),
    "boss_rise")

def griffinwing.boss.Beast.new?(+lamps: J.Val) -> J.Val:
  +b0 = J.obj()
  +b1 = ga.su(b0, "phase", 0)
  +b2 = ga.ss(b1, "st", "Rise")
  +b3 = ga.su(b2, "ticks", griffinwing.boss.RISE_T())
  +b4 = ga.si(b3, "x", 0)
  +b5 = ga.si(b4, "z", griffinwing.boss.HIGH())
  +b6 = ga.sv(b5, "lamps", lamps)
  +b7 = ga.sv(b6, "marks", J.JA{[]})
  +b8 = ga.si(b7, "gap", 0)
  +b9 = ga.si(b8, "side", 1)
  +b10 = ga.si(b9, "col", 0)
  +b11 = ga.su(b10, "atk", 0)
  ga.su(b11, "seq", 0)

# How long the Wyrm rests between attacks: shorter the more knights fly and the
# later the phase.

def griffinwing.boss.Session.rest?(+s: J.Val, +phase: U32) -> U32:
  U32.max((64 - (4 * (griffinwing.Session.pressure(s) - 1 : U32) : U32) - (8 * phase : U32) : U32), 34)

# The Wyrm's tick. The Wyrm is taken out of the session while it acts.

def griffinwing.boss.Session.step_boss?(+s: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(J.is_null(J.get(s, "boss")),
    _ => s,
    _ => griffinwing.boss.Session.step_boss.sway(ga.sv(s, "boss", J.JNull{}), J.get(s, "boss"), n))

# The head holds its place ahead and sways; it rises from below; the lamps
# follow it and their shields time out.

def griffinwing.boss.Session.step_boss.sway?(+s: J.Val, +b0: J.Val, +n: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.lamps(s,
    ga.si(ga.si(b0, "x", griffinwing.types.tri(ga.u(s, "t"), 260, griffinwing.types.dm(24))), "z",
      (griffinwing.boss.HIGH() - gw.ifu(gw.isn(b0, "st", "Rise"), _ => gw.div((ga.u(b0, "ticks") * griffinwing.types.dm(1) : U32), 2), _ => 0) : U32)),
    n)

def griffinwing.boss.Session.step_boss.lamps?(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.lit(
    griffinwing.boss.Session.step_boss.lamp(griffinwing.boss.Session.step_boss.lamp(griffinwing.boss.Session.step_boss.lamp(s, b, 0), b, 1), b, 2),
    b, n)

# Lamp `k`: placed about the head; an open shield counts down.

def griffinwing.boss.Session.step_boss.lamp?(+s: J.Val, +b: J.Val, +k: U32) -> J.Val:
  griffinwing.boss.Session.step_boss.lamp2(s, b, k, griffinwing.combat.Session.bombers_ram.at(ga.items(s, "foes"), J.u32(J.nth(J.get(b, "lamps"), k)), 0))

def griffinwing.boss.Session.step_boss.lamp2?(+s: J.Val, +b: J.Val, +k: U32, +i: U32) -> J.Val:
  gw.ifj(U32.is_lt(i, gw.len(s, "foes")),
    _ => gw.put_item(s, "foes", i,
      griffinwing.boss.Session.step_boss.shield(
        ga.si(ga.si(ga.si(gw.item(s, "foes", i), "s", (griffinwing.Session.anchor(s) + griffinwing.boss.HEAD() - griffinwing.boss.FRONT() : U32)),
          "x", (ga.u(b, "x") + griffinwing.boss.LAMP.x(k) : U32)), "z", (ga.u(b, "z") + griffinwing.boss.LAMP.z(k) : U32)))),
    _ => s)

def griffinwing.boss.Session.step_boss.shield?(+f: J.Val) -> J.Val:
  gw.ifj(gw.isn(f, "st", "Open"),
    _ => griffinwing.boss.Session.step_boss.shield2(gw.subs(f, "ticks", 1)),
    _ => f)

def griffinwing.boss.Session.step_boss.shield2?(+f: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(f, "ticks")), _ => gw.sets(f, "st", "Shut"), _ => f)

# The lit lamp is out: the Wyrm reels (or dies), and its spits are gone.

def griffinwing.boss.Session.step_boss.lit?(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(Bool.and(
      griffinwing.boss.Session.step_boss.out(ga.items(s, "foes"), J.u32(J.nth(J.get(b, "lamps"), U32.min(ga.u(b, "phase"), 2)))),
      Bool.not(Bool.or(gw.isn(b, "st", "Hurt"), gw.isn(b, "st", "Dying")))),
    _ => griffinwing.boss.Session.step_boss.reel(s, b),
    _ => griffinwing.boss.Session.step_boss.act(s, b, n))

# `foes.find(|f| f.id == lit).is_some_and(|f| f.st == Broken)`

def griffinwing.boss.Session.step_boss.out?(+fs: List<&2, J.Val>, +lit: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      gw.ifb(U32.is_eq(ga.u(f, "id"), lit), _ => gw.isn(f, "st", "Broken"), _ => griffinwing.boss.Session.step_boss.out(t, lit))

def griffinwing.boss.Session.step_boss.reel?(+s: J.Val, +b0: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.reel2(
    griffinwing.Session.sound(ga.sv(s, "bolts", J.JA{griffinwing.boss.Session.step_boss.owned(ga.items(s, "bolts"))}), "boss_roar"),
    ga.sv(b0, "marks", J.JA{[]}))

# bolts.retain(|x| x.owner != 0)

def griffinwing.boss.Session.step_boss.owned?(+bs: List<&2, J.Val>) -> List<&2, J.Val>:
  match bs:
    case Nil{}:
      Nil{}
    case Con{+b, +t}:
      gw.ifjl(U32.is_zero(ga.u(b, "owner")),
        _ => griffinwing.boss.Session.step_boss.owned(t),
        _ => Con{b, griffinwing.boss.Session.step_boss.owned(t)})

def griffinwing.boss.Session.step_boss.reel2?(+s: J.Val, +b: J.Val) -> J.Val:
  ga.sv(s, "boss",
    gw.ifj(U32.is_ge(ga.u(b, "phase"), 2),
      _ => ga.su(gw.sets(b, "st", "Dying"), "ticks", griffinwing.DYING()),
      _ => ga.su(gw.sets(b, "st", "Hurt"), "ticks", griffinwing.boss.HURT_T())))

# The attack pattern of the phase: [0], [1, 0], [2, 0, 1, 0].

def griffinwing.boss.Session.step_boss.pat?(+phase: U32, +i: U32) -> U32:
  gw.ifu(U32.is_eq(phase, 0), _ => 0,
    _ => gw.ifu(U32.is_eq(phase, 1), _ => Bool.pick(U32, U32.is_eq(U32.mod(i, 2), 0), 1, 0),
      _ => Bool.pick(U32, U32.is_eq(U32.mod(i, 4), 0), 2, Bool.pick(U32, U32.is_eq(U32.mod(i, 4), 2), 1, 0))))

# The Wyrm's state: what it does this tick.

def griffinwing.boss.Session.step_boss.act?(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(gw.isn(b, "st", "Rise"),
    _ => griffinwing.boss.Session.step_boss.rise(s, gw.subs(b, "ticks", 1)),
    _ => gw.ifj(gw.isn(b, "st", "Idle"),
      _ => griffinwing.boss.Session.step_boss.idle(s, gw.subs(b, "ticks", 1), n),
      _ => gw.ifj(gw.isn(b, "st", "Tell"),
        _ => griffinwing.boss.Session.step_boss.tell(s, gw.subs(b, "ticks", 1)),
        _ => gw.ifj(Bool.or(gw.isn(b, "st", "Coil"), gw.isn(b, "st", "Roar")),
          _ => griffinwing.boss.Session.step_boss.sweep(s, b),
          _ => gw.ifj(gw.isn(b, "st", "Hurt"),
            _ => griffinwing.boss.Session.step_boss.hurt(s, gw.subs(b, "ticks", 1)),
            _ => griffinwing.boss.Session.step_boss.dying(s, gw.subs(b, "ticks", 1)))))))

def griffinwing.boss.Session.step_boss.rise?(+s: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(b, "ticks")),
    _ => ga.sv(griffinwing.Session.say(s, griffinwing.Session.voice(s), "Reflect"), "boss", ga.su(gw.sets(b, "st", "Idle"), "ticks", 50)),
    _ => ga.sv(s, "boss", b))

def griffinwing.boss.Session.step_boss.idle?(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(b, "ticks")),
    _ => griffinwing.boss.Session.step_boss.idle2(s,
      ga.su(ga.su(b, "atk", griffinwing.boss.Session.step_boss.pat(ga.u(b, "phase"), ga.u(b, "seq"))), "seq", U32.and((ga.u(b, "seq") + 1 : U32), 255)), n),
    _ => ga.sv(s, "boss", b))

def griffinwing.boss.Session.step_boss.idle2?(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.put(griffinwing.boss.Session.begin_tell(s, b, n))

# A pair {"s", "b"} put back as the session with its Wyrm.

def griffinwing.boss.Session.step_boss.put?(+r: J.Val) -> J.Val:
  ga.sv(J.get(r, "s"), "boss", J.get(r, "b"))

def griffinwing.boss.Session.step_boss.tell?(+s: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(b, "ticks")),
    _ => gw.ifj(U32.is_zero(ga.u(b, "atk")),
      _ => ga.sv(griffinwing.boss.Session.spit(s, b), "boss",
        ga.sv(ga.su(gw.sets(b, "st", "Idle"), "ticks", griffinwing.boss.Session.rest(s, ga.u(b, "phase"))), "marks", J.JA{[]})),
      _ => gw.ifj(U32.is_eq(ga.u(b, "atk"), 1),
        _ => ga.sv(griffinwing.Session.sound(s, "boss_rage"), "boss", ga.su(gw.sets(b, "st", "Coil"), "ticks", griffinwing.boss.COIL_T())),
        _ => ga.sv(griffinwing.Session.sound(s, "boss_roar"), "boss", ga.su(gw.sets(b, "st", "Roar"), "ticks", griffinwing.boss.ROAR_T())))),
    _ => ga.sv(s, "boss", b))

# The coils sweep the window / the roar's column: each knight flying and
# caught is struck, then the attack's ticks run down.

def griffinwing.boss.Session.step_boss.sweep?(+s: J.Val, +b: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.sweep.done(
    griffinwing.boss.Session.step_boss.sweep.each(s, griffinwing.boss.Session.flying_ids(s, ga.ids(J.get(s, "seats"))), ga.u(b, "gap"), ga.u(b, "col"), gw.isn(b, "st", "Coil")),
    gw.subs(b, "ticks", 1))

# The ids of the knights flying, in seat order.

def griffinwing.boss.Session.flying_ids?(+s: J.Val, +ids: List<&2, U32>) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gw.ifl(griffinwing.types.Craft.flying(gw.craft_of(s, id)),
        _ => Con{id, griffinwing.boss.Session.flying_ids(s, t)},
        _ => griffinwing.boss.Session.flying_ids(s, t))

def griffinwing.boss.Session.step_boss.sweep.each?(+s: J.Val, +ids: List<&2, U32>, +gap: U32, +col: U32, +coil: Bool) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.boss.Session.step_boss.sweep.each(
        griffinwing.boss.Session.step_boss.sweep.one(s, id, gap, col, coil, ga.u(gw.craft_of(s, id), "x")),
        t, gap, col, coil)

def griffinwing.boss.Session.step_boss.sweep.one?(+s: J.Val, +id: U32, +gap: U32, +col: U32, +coil: Bool, +x: U32) -> J.Val:
  gw.ifj(gw.ifb(coil,
      _ => gw.gt(gw.abs((x - gap : U32)), (griffinwing.boss.GAP_HALF() - gw.div(griffinwing.flight.CRAFT_R(), 2) : U32)),
      _ => gw.le(gw.abs((x - col : U32)), (griffinwing.boss.COL_HALF() + gw.div(griffinwing.flight.CRAFT_R(), 2) : U32))),
    _ => gw.out_s(griffinwing.combat.Session.hurt_craft(s, id, griffinwing.boss.BLOW())),
    _ => s)

def griffinwing.boss.Session.step_boss.sweep.done?(+s: J.Val, +b: J.Val) -> J.Val:
  ga.sv(s, "boss",
    gw.ifj(U32.is_zero(ga.u(b, "ticks")),
      _ => ga.sv(ga.su(gw.sets(b, "st", "Idle"), "ticks", griffinwing.boss.Session.rest(s, ga.u(b, "phase"))), "marks", J.JA{[]}),
      _ => b))

# A lamp went out: it reels, then the next phase, and the Wyrm calls kites.

def griffinwing.boss.Session.step_boss.hurt?(+s: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(b, "ticks")),
    _ => griffinwing.boss.Session.step_boss.hurt2(s, ga.su(b, "phase", (ga.u(b, "phase") + 1 : U32))),
    _ => ga.sv(s, "boss", b))

def griffinwing.boss.Session.step_boss.hurt2?(+s: J.Val, +b0: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.hurt3(s,
    ga.su(ga.su(gw.sets(b0, "st", "Idle"), "ticks", (griffinwing.boss.Session.rest(s, ga.u(b0, "phase")) + 20 : U32)), "seq", 0))

def griffinwing.boss.Session.step_boss.hurt3?(+s: J.Val, +b: J.Val) -> J.Val:
  ga.sv(
    griffinwing.boss.Session.step_boss.hunters(
      griffinwing.Session.say(s, griffinwing.Session.voice(s), gw.ifs(U32.is_eq(ga.u(b, "phase"), 1), _ => "Coils", _ => "Roar")),
      0, (1 + U32.div(griffinwing.Session.pressure(s), 3) : U32)),
    "boss", b)

# for k in 0..hunters { act(Hunter { side: if k % 2 == 0 { -1 } else { 1 } }) }

def griffinwing.boss.Session.step_boss.hunters?(+s: J.Val, +k: U32, +n: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, n),
    _ => griffinwing.boss.Session.step_boss.hunters(
      griffinwing.script.Session.act(s, ga.si(ga.ss(J.obj(), "k", "Hunter"), "side", gw.ifu(U32.is_eq(U32.mod(k, 2), 0), _ => gw.neg(1), _ => 1))),
      (k + 1 : U32), n),
    _ => s)

def griffinwing.boss.Session.step_boss.dying?(+s: J.Val, +b: J.Val) -> J.Val:
  ga.sv(
    gw.ifj(U32.is_zero(U32.mod(ga.u(b, "ticks"), 12)),
      _ => griffinwing.Session.spark(s, "Pop", (griffinwing.Session.anchor(s) + griffinwing.boss.HEAD() : U32), ga.u(b, "x"), ga.u(b, "z"), 0),
      _ => s),
    "boss", b)

# An attack's footprints are locked and shown for the tell's ticks. {"s", "b"}.

def griffinwing.boss.Session.begin_tell?(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(J.is_null(griffinwing.combat.Session.target(s, 0)),
    _ => J.set(J.set(J.obj(), "s", s), "b", ga.su(gw.sets(b, "st", "Idle"), "ticks", 30)),
    _ => griffinwing.boss.Session.begin_tell.kind(s, ga.sv(b, "marks", J.JA{[]}), n))

def griffinwing.boss.Session.begin_tell.kind?(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell.fin(
    gw.ifj(U32.is_zero(ga.u(b, "atk")),
      _ => griffinwing.boss.Session.begin_tell.spit(s, b),
      _ => gw.ifj(U32.is_eq(ga.u(b, "atk"), 1),
        _ => griffinwing.boss.Session.begin_tell.coil(s, b, n),
        _ => griffinwing.boss.Session.begin_tell.roar(s, b))),
    n)

def griffinwing.boss.Session.begin_tell.fin?(+r: J.Val, +n: J.Val) -> J.Val:
  J.set(J.set(J.obj(), "s", griffinwing.Session.sound(J.get(r, "s"), "vent_fire")), "b",
    ga.su(gw.sets(J.get(r, "b"), "st", "Tell"), "ticks", U32.max(ga.u(n, "tell"), 40)))

# The spit: a footprint for each knight asked for, a knight asked for twice is
# aimed at to either side.

def griffinwing.boss.Session.begin_tell.spit?(+s: J.Val, +b: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell.say(s,
    griffinwing.boss.Session.begin_tell.marks(s, b, 0,
      U32.min((2 + U32.div((griffinwing.Session.pressure(s) - 1 : U32), 2) + ga.u(b, "phase") : U32), 7)),
    b)

def griffinwing.boss.Session.begin_tell.marks?(+s: J.Val, +b: J.Val, +k: U32, +count: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, count),
    _ => griffinwing.boss.Session.begin_tell.mark(s, b, k, count, griffinwing.combat.Session.target(s, (ga.u(b, "seq") + k : U32))),
    _ => b)

def griffinwing.boss.Session.begin_tell.mark?(+s: J.Val, +b: J.Val, +k: U32, +count: U32, +to: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell.marks(s,
    gw.ifj(J.is_null(to),
      _ => b,
      _ => ga.push(b, "marks",
        ga.pair_i(
          (ga.nu(to, 0) + gw.ifu(U32.is_eq(U32.mod(k, 2), 0), _ => griffinwing.boss.Session.begin_tell.jitter(s, k), _ => gw.neg(griffinwing.boss.Session.begin_tell.jitter(s, k))) : U32),
          ga.nu(to, 1)))),
    (k + 1 : U32), count)

def griffinwing.boss.Session.begin_tell.jitter?(+s: J.Val, +k: U32) -> U32:
  (U32.div(k, U32.max(gw.nseats(s), 1)) * griffinwing.types.dm(16) : U32)

def griffinwing.boss.Session.begin_tell.say?(+s: J.Val, +b: J.Val, +unused: J.Val) -> J.Val:
  J.set(J.set(J.obj(), "s",
    gw.ifj(Bool.and(U32.is_zero(ga.u(b, "phase")), U32.is_eq(ga.u(b, "seq"), 1)),
      _ => griffinwing.Session.say(s, griffinwing.Session.voice(s), "Reflect"),
      _ => s)),
    "b", b)

# The coil: the gap it sweeps from the side its turn names.

def griffinwing.boss.Session.begin_tell.coil?(+s: J.Val, +b0: J.Val, +n: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell.coil2(s,
    ga.si(b0, "side", gw.ifu(U32.is_eq(U32.mod(ga.u(b0, "seq"), 2), 0), _ => gw.neg(1), _ => 1)),
    n, gw.ifu(J.is_null(griffinwing.combat.Session.target(s, ga.u(b0, "seq"))), _ => 0, _ => ga.nu(griffinwing.combat.Session.target(s, ga.u(b0, "seq")), 0)))

def griffinwing.boss.Session.begin_tell.coil2?(+s: J.Val, +b: J.Val, +n: J.Val, +gap: U32) -> J.Val:
  griffinwing.boss.Session.begin_tell.coil3(s, b,
    gw.clamp(gap, (gw.neg(griffinwing.Nums.wx(n)) + gw.div(griffinwing.boss.GAP_HALF(), 2) : U32), (griffinwing.Nums.wx(n) - gw.div(griffinwing.boss.GAP_HALF(), 2) : U32)))

def griffinwing.boss.Session.begin_tell.coil3?(+s: J.Val, +b: J.Val, +gap: U32) -> J.Val:
  J.set(J.set(J.obj(), "s", s), "b", ga.push(ga.si(b, "gap", gap), "marks", ga.pair_i(gap, 0)))

# The roar: a column where the next knight in turn flies.

def griffinwing.boss.Session.begin_tell.roar?(+s: J.Val, +b: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell.roar2(s, b,
    gw.ifu(J.is_null(griffinwing.combat.Session.target(s, (ga.u(b, "seq") + 1 : U32))), _ => 0, _ => ga.nu(griffinwing.combat.Session.target(s, (ga.u(b, "seq") + 1 : U32)), 0)))

def griffinwing.boss.Session.begin_tell.roar2?(+s: J.Val, +b: J.Val, +col: U32) -> J.Val:
  J.set(J.set(J.obj(), "s", s), "b", ga.push(ga.si(b, "col", col), "marks", ga.pair_i(col, 0)))

# The Wyrm's spit: a Gloam bolt for each locked mark, from its mouth, each
# owing its return to the lit lamp.

def griffinwing.boss.Session.spit?(+s: J.Val, +b: J.Val) -> J.Val:
  griffinwing.Session.sound(
    griffinwing.boss.Session.spit.each(s, b, ga.items(b, "marks"), J.u32(J.nth(J.get(b, "lamps"), U32.min(ga.u(b, "phase"), 2)))),
    "shot_crossbow")

def griffinwing.boss.Session.spit.each?(+s: J.Val, +b: J.Val, +ms: List<&2, J.Val>, +lit: U32) -> J.Val:
  match ms:
    case Nil{}:
      s
    case Con{+m, +t}:
      griffinwing.boss.Session.spit.each(
        griffinwing.boss.Session.spit.one(s, b, lit, griffinwing.combat.Session.plane_shot(s, (griffinwing.Session.anchor(s) + griffinwing.boss.HEAD() : U32), ga.u(b, "x"), ga.u(b, "z"), ga.nu(m, 0), ga.nu(m, 1), griffinwing.types.dm(5))),
        b, t, lit)

def griffinwing.boss.Session.spit.one?(+s: J.Val, +b: J.Val, +lit: U32, +v: GwV3) -> J.Val:
  griffinwing.flight.Session.push_bolt(s,
    griffinwing.types.Bolt.make("Gloam", 0, (griffinwing.Session.anchor(s) + griffinwing.boss.HEAD() : U32), ga.u(b, "x"), ga.u(b, "z"),
      gw.v3a(v), gw.v3b(v), gw.v3c(v), 25, 300, 0, lit, 0, False{}))

def griffinwing.bot.HOLD_AT_BOT?() -> U32:
  7

# A steady scatter in `-span..=span` from a knight, a tick and a salt.

def griffinwing.bot.noise?(+id: U32, +t: U32, +salt: U32, +span: U32) -> U32:
  griffinwing.bot.noise.mix(U32.xor(U32.xor((id * 2654435761 : U32), (t * 2246822507 : U32)), (salt * 3266489909 : U32)), span)

def griffinwing.bot.noise.mix?(+h0: U32, +span: U32) -> U32:
  griffinwing.bot.noise.fin(U32.xor(h0, U32.shrn(h0, 15n)), span)

def griffinwing.bot.noise.fin?(+h1: U32, +span: U32) -> U32:
  griffinwing.bot.noise.fin2((h1 * 740637549 : U32), span)

def griffinwing.bot.noise.fin2?(+h2: U32, +span: U32) -> U32:
  griffinwing.bot.noise.fin3(U32.xor(h2, U32.shrn(h2, 13n)), span)

def griffinwing.bot.noise.fin3?(+h: U32, +span: U32) -> U32:
  (U32.mod(h, (2 * span + 1 : U32)) - span : U32)

# `steer`: -1, 0 or 1 toward `to`, with a dead zone.

def griffinwing.bot.steer?(+from: U32, +to: U32, +dead: U32) -> U32:
  Bool.pick(U32, U32.is_le(gw.abs((to - from : U32)), dead), 0, gw.sig((to - from : U32)))

# What knight `id` presses this tick, with a sure hand.

def griffinwing.bot.Session.bot_input?(+s: J.Val, +id: U32) -> J.Val:
  griffinwing.bot.Session.bot_with(s, id, griffinwing.Nums.default(), False{})

# The Input J: the default, with the keys set.

def griffinwing.bot.Input.make?(+mx: U32, +my: U32, +ax: U32, +ay: U32, +fire: Bool, +dash: Bool, +bomb: Bool, +swing: Bool) -> J.Val:
  i0 = rules.Input.default()
  i1 = ga.si(i0, "move_x", mx)
  i2 = ga.si(i1, "move_y", my)
  i3 = ga.si(i2, "aim_x", ax)
  i4 = ga.si(i3, "aim_y", ay)
  i5 = ga.sb(i4, "fire", fire)
  i6 = ga.sb(i5, "dash", dash)
  i7 = ga.sb(i6, "bomb", bomb)
  ga.sb(i7, "swing", swing)

# What knight `id` presses this tick, for the numbers `n` stand at.

def griffinwing.bot.Session.bot_with?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool) -> J.Val:
  gw.ifj(Bool.not(ga.has_key(J.get(s, "seats"), id)),
    _ => rules.Input.default(),
    _ => griffinwing.bot.Session.bot_with.seated(s, id, n, novice, gw.craft_of(s, id)))

def griffinwing.bot.Session.bot_with.seated?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val) -> J.Val:
  gw.ifj(griffinwing.Session.Mode.is(s, "Tally"),
    _ => gw.ifj(U32.is_zero(U32.mod(ga.u(s, "mode_t"), 2)), _ => ga.sb(rules.Input.default(), "interact", True{}), _ => rules.Input.default()),
    _ => gw.ifj(Bool.or(griffinwing.Session.Mode.is(s, "Retry"), U32.is_gt(ga.u(c, "down"), 0)),
      _ => rules.Input.default(),
      _ => griffinwing.bot.Session.bot_with.plan(s, id, n, novice, c, griffinwing.flight.Session.craft_s(s, c))))

# Where to be, whether to roll, whether to brake: the plan, then the hands.

def griffinwing.bot.Session.bot_with.plan?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32) -> J.Val:
  griffinwing.bot.Session.bot_with.hands(s, id, n, novice, c, s_c,
    griffinwing.bot.Session.plan(s, id, n, novice, c, s_c))

# The plan: {"want": null | [x, z], "rolls", "brake", "threat": null | [t, px, pz, gloam]}.

def griffinwing.bot.Session.plan?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32) -> J.Val:
  griffinwing.bot.Session.plan.react(s, id, n, novice, c, s_c,
    Bool.and(griffinwing.combat.Session.latched(s, id), Bool.and(U32.is_zero(ga.u(c, "rest")), U32.is_zero(ga.u(c, "roll")))),
    griffinwing.bot.Session.plan.chaser(ga.items(s, "foes"), id, s_c),
    griffinwing.bot.Session.threat(s, c, s_c),
    gw.ifj(griffinwing.bot.Session.reflector(s, id), _ => griffinwing.bot.Session.intercept(s, s_c), _ => J.JNull{}))

# A friend with a kite on their tail, level with this knight: brake until the
# chaser is in front.

def griffinwing.bot.Session.plan.chaser?(+fs: List<&2, J.Val>, +id: U32, +s_c: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      gw.ifb(Bool.and(gw.isn(f, "st", "Latched"), Bool.and(U32.is_le(ga.u(f, "on"), 6), Bool.and(U32.is_ne(ga.u(f, "on"), id), U32.is_ne(ga.u(f, "on"), 0)))),
        _ => gw.lt((ga.u(f, "s") - s_c : U32), griffinwing.types.dm(30)),
        _ => griffinwing.bot.Session.plan.chaser(t, id, s_c))

def griffinwing.bot.Session.plan.react?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +rolls0: Bool, +brake0: Bool, +threat: J.Val, +seek: J.Val) -> J.Val:
  gw.ifj(Bool.not(J.is_null(seek)),
    _ => griffinwing.bot.Session.plan.seek(s, id, n, novice, c, s_c, brake0, threat, seek),
    _ => gw.ifj(Bool.not(J.is_null(threat)),
      _ => griffinwing.bot.Session.plan.dodge(s, id, n, novice, c, s_c, rolls0, brake0, threat),
      _ => griffinwing.bot.Session.plan.spots(s, id, n, c, s_c, J.JNull{}, rolls0, brake0, threat)))

# The Wyrm's lamp is shut: be where its spit will be, and roll. A novice meets
# the Wyrm's spit half the time.

def griffinwing.bot.Session.plan.seek?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +brake0: Bool, +threat: J.Val, +seek: J.Val) -> J.Val:
  griffinwing.bot.Session.plan.spots(s, id, n, c, s_c, J.JA{[J.of_i32(ga.nu(seek, 1)), J.of_i32(ga.nu(seek, 2))]},
    Bool.and(Bool.or(Bool.not(novice), gw.ge(griffinwing.bot.noise(id, U32.div(ga.u(s, "t"), 60), 5, 1), 0)),
      Bool.and(U32.is_zero(ga.u(c, "rest")), Bool.and(U32.is_zero(ga.u(c, "roll")), U32.is_le(ga.nu(seek, 0), gw.ifu(novice, _ => 6, _ => 8))))),
    brake0, threat)

# A hostile bolt will cross: roll into a Gloam bolt, or make room.

def griffinwing.bot.Session.plan.dodge?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +rolls0: Bool, +brake0: Bool, +threat: J.Val) -> J.Val:
  griffinwing.bot.Session.plan.dodge2(s, id, n, novice, c, s_c, rolls0, brake0, threat,
    Bool.and(Bool.and(U32.is_zero(ga.u(c, "rest")), U32.is_zero(ga.u(c, "roll"))),
      Bool.and(J.bool(J.nth(threat, 3)),
        Bool.and(Bool.or(griffinwing.bot.Session.reflector(s, id), U32.is_le(ga.nu(threat, 0), 6)),
          Bool.and(U32.is_le(ga.nu(threat, 0), gw.ifu(novice, _ => 6, _ => 9)),
            Bool.not(Bool.and(novice, U32.is_eq(griffinwing.bot.noise(id, U32.div(ga.u(s, "t"), 45), 9, 2), 0))))))))

def griffinwing.bot.Session.plan.dodge2?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +rolls0: Bool, +brake0: Bool, +threat: J.Val, +roll: Bool) -> J.Val:
  gw.ifj(roll,
    _ => griffinwing.bot.Session.plan.spots(s, id, n, c, s_c, J.JNull{}, True{}, brake0, threat),
    _ => gw.ifj(rolls0,
      _ => griffinwing.bot.Session.plan.spots(s, id, n, c, s_c, J.JNull{}, True{}, brake0, threat),
      _ => gw.ifj(U32.is_le(ga.nu(threat, 0), gw.ifu(novice, _ => 22, _ => 40)),
        _ => griffinwing.bot.Session.plan.spots(s, id, n, c, s_c,
          griffinwing.bot.Session.room_from(c, ga.nu(threat, 1), ga.nu(threat, 2), n), rolls0, brake0, threat),
        _ => griffinwing.bot.Session.plan.spots(s, id, n, c, s_c, J.JNull{}, rolls0, brake0, threat))))

# Still no place to be: away from a footprint, a corridor, the next prop.

def griffinwing.bot.Session.plan.spots?(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +s_c: U32, +want: J.Val, +rolls: Bool, +brake: Bool, +threat: J.Val) -> J.Val:
  griffinwing.bot.Session.plan.fin(
    griffinwing.bot.Session.plan.props(s, id, n, c, s_c,
      gw.ifj(J.is_null(want),
        _ => gw.ifj(J.is_null(griffinwing.bot.Session.footprint_away(s, c, n)),
          _ => griffinwing.bot.Session.boss_corridor(s, c, n),
          _ => griffinwing.bot.Session.footprint_away(s, c, n)),
        _ => want),
      brake),
    rolls, threat)

def griffinwing.bot.Session.plan.fin?(+w: J.Val, +rolls: Bool, +threat: J.Val) -> J.Val:
  ga.sv(ga.sb(w, "rolls", rolls), "threat", threat)

# The next thing on the rail to thread or take, when nothing else says where to
# be. {"want", "brake"}.

def griffinwing.bot.Session.plan.props?(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +s_c: U32, +want: J.Val, +brake: Bool) -> J.Val:
  gw.ifj(J.is_null(want),
    _ => griffinwing.bot.Session.plan.prop(s, id, n, c, s_c, ga.items(s, "props"), brake),
    _ => ga.sb(ga.sv(J.obj(), "want", want), "brake", brake))

def griffinwing.bot.Session.plan.prop?(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +s_c: U32, +ps: List<&2, J.Val>, +brake: Bool) -> J.Val:
  match ps:
    case Nil{}:
      ga.sb(ga.sv(J.obj(), "want", J.JNull{}), "brake", brake)
    case Con{+p, +t}:
      gw.ifj(gw.within((ga.u(p, "s") - s_c : U32), 0, griffinwing.types.dm(300)),
        _ => griffinwing.bot.Session.plan.take(s, id, n, c, s_c, p, t, brake),
        _ => griffinwing.bot.Session.plan.prop(s, id, n, c, s_c, t, brake))

def griffinwing.bot.Session.plan.take?(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +s_c: U32, +p: J.Val, +t: List<&2, J.Val>, +brake: Bool) -> J.Val:
  gw.ifj(Bool.and(gw.isn(p, "thing", "Arch"), gw.lt((ga.u(p, "s") - s_c : U32), griffinwing.types.dm(260))),
    _ => ga.sb(ga.sv(J.obj(), "want", ga.pair_i(ga.u(p, "x"), ga.u(p, "z"))), "brake", brake),
    _ => gw.ifj(Bool.and(Bool.or(gw.isn(p, "thing", "Gate"), gw.isn(p, "thing", "Ring")),
          Bool.and(U32.is_zero(U32.and(ga.u(p, "taken"), griffinwing.bit(id))), Bool.or(gw.isn(p, "thing", "Gate"), gw.lt(ga.u(c, "shield"), 80)))),
      _ => ga.sb(ga.sv(J.obj(), "want", ga.pair_i(ga.u(p, "x"), ga.u(p, "z"))), "brake", brake),
      _ => gw.ifj(Bool.and(gw.isn(p, "thing", "Shutter"), gw.lt((ga.u(p, "s") - s_c : U32), griffinwing.types.dm(220))),
        _ => ga.sb(ga.sv(J.obj(), "want", ga.pair_i(ga.u(p, "x"), ga.u(c, "z"))), "brake",
          Bool.or(brake, gw.lt(griffinwing.things.shutter_gap(p, (ga.u(s, "t") + gw.max(gw.div((ga.u(p, "s") - s_c : U32), griffinwing.types.PACE()), 0) : U32), griffinwing.Nums.wx(n)), griffinwing.types.dm(16)))),
        _ => gw.ifj(Bool.and(gw.isn(p, "thing", "Ridge"),
              Bool.and(gw.lt(gw.abs((ga.u(p, "x") - ga.u(c, "x") : U32)), (ga.u(p, "w") + griffinwing.types.dm(16) : U32)), gw.lt((ga.u(p, "s") - s_c : U32), griffinwing.types.dm(200)))),
          _ => ga.sb(ga.sv(J.obj(), "want", ga.pair_i(gw.neg((gw.sig(ga.u(p, "x")) * griffinwing.types.dm(20) : U32)), ga.u(c, "z"))), "brake", brake),
          _ => griffinwing.bot.Session.plan.prop(s, id, n, c, s_c, t, brake)))))

# The hands: steer, roll, aim, fire, bomb.

def griffinwing.bot.Session.bot_with.hands?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +plan: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_with.roll(s, id, n, novice, c, s_c, plan,
    gw.ifj(J.is_null(J.get(plan, "want")), _ => ga.pair_i(0, 0), _ => J.get(plan, "want")))

def griffinwing.bot.Session.bot_with.roll?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +plan: J.Val, +to: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_with.roll2(s, id, n, novice, c, s_c, plan,
    griffinwing.bot.steer(ga.u(c, "x"), ga.nu(to, 0), griffinwing.types.dm(8)),
    gw.neg(griffinwing.bot.steer(ga.u(c, "z"), ga.nu(to, 1), griffinwing.types.dm(8))))

def griffinwing.bot.Session.bot_with.roll2?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +plan: J.Val, +mx: U32, +my: U32) -> J.Val:
  gw.ifj(Bool.and(ga.b(plan, "rolls"), Bool.and(griffinwing.combat.Session.latched(s, id), J.is_null(J.get(plan, "threat")))),
    _ => griffinwing.bot.Input.make(mx, my, 0, 0, False{}, U32.is_zero(U32.and(ga.u(c, "keys"), griffinwing.flight.K_DASH())), False{}, ga.b(plan, "brake")),
    _ => gw.ifj(ga.b(plan, "rolls"),
      _ => griffinwing.bot.Input.make(
        gw.ifu(Bool.and(Bool.not(J.is_null(J.get(plan, "threat"))), gw.ge(ga.nu(J.get(plan, "threat"), 1), ga.u(c, "x"))), _ => gw.neg(1), _ => 1),
        my, 0, 0, False{}, U32.is_zero(U32.and(ga.u(c, "keys"), griffinwing.flight.K_DASH())), False{}, ga.b(plan, "brake")),
      _ => griffinwing.bot.Session.bot_with.shoot(s, id, n, novice, c, s_c, mx, my, ga.b(plan, "brake"))))

# What to shoot: a kite on a friend's tail, a lamp that is open, a flight to
# sweep, the nearest foe.

def griffinwing.bot.Session.bot_with.shoot?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool) -> J.Val:
  griffinwing.bot.Session.bot_with.shoot2(s, id, n, novice, c, s_c, mx, my, brake,
    griffinwing.bot.Session.lockable(ga.items(s, "foes"), s_c))

# foes.filter(|f| near = if Latched { dm(15) } else { dm(40) }; f.lockable() && (near..=dm(480)).contains(f.s - s_c))

def griffinwing.bot.Session.lockable?(+fs: List<&2, J.Val>, +s_c: U32) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      gw.ifjl(Bool.and(griffinwing.types.Foe.lockable(f),
          gw.within((ga.u(f, "s") - s_c : U32), gw.ifu(gw.isn(f, "st", "Latched"), _ => griffinwing.types.dm(15), _ => griffinwing.types.dm(40)), griffinwing.types.dm(480))),
        _ => Con{f, griffinwing.bot.Session.lockable(t, s_c)},
        _ => griffinwing.bot.Session.lockable(t, s_c))

# The foes of the list that are not lamps.

def griffinwing.bot.Session.fliers?(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      gw.ifjl(gw.isn(f, "kind", "Lamp"), _ => griffinwing.bot.Session.fliers(t), _ => Con{f, griffinwing.bot.Session.fliers(t)})

# The first foe of the list in this state / kind, or null.

def griffinwing.bot.Session.first_kind?(+fs: List<&2, J.Val>, +kind: String) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      gw.ifj(gw.isn(f, "kind", kind), _ => f, _ => griffinwing.bot.Session.first_kind(t, kind))

# `lockable.filter(Latched).min_by_key(|f| f.s)`: the first of the least `s`.

def griffinwing.bot.Session.latched_first?(+fs: List<&2, J.Val>, +best: J.Val) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      griffinwing.bot.Session.latched_first(t,
        gw.ifj(Bool.and(gw.isn(f, "st", "Latched"), Bool.or(J.is_null(best), gw.lt(ga.u(f, "s"), ga.u(best, "s")))), _ => f, _ => best))

# `fliers.filter(!in_locks).min_by_key(|f| |f.x - c.x| + |f.z - c.z|)`

def griffinwing.bot.Session.sweep_next?(+fs: List<&2, J.Val>, +c: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      griffinwing.bot.Session.sweep_next2(f, t, c, best, bd,
        (gw.abs((ga.u(f, "x") - ga.u(c, "x") : U32)) + gw.abs((ga.u(f, "z") - ga.u(c, "z") : U32)) : U32),
        griffinwing.bot.Session.in_locks(ga.items(c, "locks"), ga.u(f, "id")))

def griffinwing.bot.Session.sweep_next2?(+f: J.Val, +t: List<&2, J.Val>, +c: J.Val, +best: J.Val, +bd: U32, +d: U32, +locked: Bool) -> J.Val:
  gw.ifj(Bool.and(Bool.not(locked), Bool.or(J.is_null(best), gw.lt(d, bd))),
    _ => griffinwing.bot.Session.sweep_next(t, c, f, d),
    _ => griffinwing.bot.Session.sweep_next(t, c, best, bd))

def griffinwing.bot.Session.in_locks?(+ls: List<&2, J.Val>, +id: U32) -> Bool:
  match ls:
    case Nil{}:
      False{}
    case Con{+l, +t}:
      Bool.or(U32.is_eq(ga.u(l, "foe"), id), griffinwing.bot.Session.in_locks(t, id))

# `fliers.min_by_key(|f| (f.s - s_c, f.id))`

def griffinwing.bot.Session.nearest?(+fs: List<&2, J.Val>, +s_c: U32, +best: J.Val) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      griffinwing.bot.Session.nearest(t, s_c,
        gw.ifj(Bool.or(J.is_null(best),
            Bool.or(gw.lt(ga.u(f, "s"), ga.u(best, "s")), Bool.and(U32.is_eq(ga.u(f, "s"), ga.u(best, "s")), U32.is_lt(ga.u(f, "id"), ga.u(best, "id"))))),
          _ => f, _ => best))

def griffinwing.bot.Session.bot_with.shoot2?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool, +lockable: List<&2, J.Val>) -> J.Val:
  griffinwing.bot.Session.bot_with.shoot3(s, id, n, novice, c, s_c, mx, my, brake, lockable,
    griffinwing.bot.Session.first_kind(lockable, "Lamp"),
    griffinwing.bot.Session.latched_first(lockable, J.JNull{}),
    griffinwing.bot.Session.fliers(lockable))

def griffinwing.bot.Session.bot_with.shoot3?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool, +lockable: List<&2, J.Val>, +lamp_open: J.Val, +latched: J.Val, +fliers: List<&2, J.Val>) -> J.Val:
  griffinwing.bot.Session.bot_with.shoot4(s, id, n, novice, c, s_c, mx, my, brake, lamp_open, latched, fliers,
    U32.is_gt(ga.u(c, "held"), griffinwing.bot.HOLD_AT_BOT()),
    gw.ifj(Bool.not(J.is_null(latched)), _ => latched,
      _ => gw.ifj(Bool.not(J.is_null(lamp_open)), _ => lamp_open,
        _ => gw.ifj(U32.is_gt(ga.u(c, "held"), griffinwing.bot.HOLD_AT_BOT()),
          _ => griffinwing.bot.Session.sweep_next(fliers, c, J.JNull{}, 0),
          _ => griffinwing.bot.Session.nearest(fliers, s_c, J.JNull{})))))

def griffinwing.bot.Session.bot_with.shoot4?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool, +lamp_open: J.Val, +latched: J.Val, +fliers: List<&2, J.Val>, +held: Bool, +target: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_with.fire(s, id, n, novice, c, s_c, mx, my, brake, lamp_open, latched, fliers, held, target,
    griffinwing.bot.Session.aim(s, id, novice, c, s_c, target))

# The reticle's key presses toward the target: [aim_x, aim_y].

def griffinwing.bot.Session.aim?(+s: J.Val, +id: U32, +novice: Bool, +c: J.Val, +s_c: U32, +target: J.Val) -> J.Val:
  gw.ifj(J.is_null(target),
    _ => ga.pair_i(griffinwing.bot.steer(ga.u(c, "rx"), 0, griffinwing.flight.RET_V()), gw.neg(griffinwing.bot.steer(ga.u(c, "rz"), 0, griffinwing.flight.RET_V()))),
    _ => griffinwing.bot.Session.aim2(c, target,
      gw.max((ga.u(target, "s") - s_c : U32), griffinwing.types.dm(30)),
      gw.ifu(novice, _ => griffinwing.bot.noise(id, U32.div(ga.u(s, "t"), 14), 1, griffinwing.types.dm(15)), _ => 0),
      gw.ifu(novice, _ => griffinwing.bot.noise(id, U32.div(ga.u(s, "t"), 14), 2, griffinwing.types.dm(10)), _ => 0)))

def griffinwing.bot.Session.aim2?(+c: J.Val, +f: J.Val, +ds: U32, +jx: U32, +jz: U32) -> J.Val:
  griffinwing.bot.Session.aim3(c,
    gw.clamp(gw.muldiv((ga.u(f, "x") - ga.u(c, "x") + jx : U32), griffinwing.types.FAR(), ds), gw.neg(griffinwing.flight.RX_MAX()), griffinwing.flight.RX_MAX()),
    gw.clamp(gw.muldiv((ga.u(f, "z") - ga.u(c, "z") + jz : U32), griffinwing.types.FAR(), ds), gw.neg(griffinwing.flight.RZ_MAX()), griffinwing.flight.RZ_MAX()))

def griffinwing.bot.Session.aim3?(+c: J.Val, +rx: U32, +rz: U32) -> J.Val:
  ga.pair_i(
    griffinwing.bot.steer(ga.u(c, "rx"), rx, U32.div((griffinwing.flight.RET_V() * 3 : U32), 4)),
    gw.neg(griffinwing.bot.steer(ga.u(c, "rz"), rz, U32.div((griffinwing.flight.RET_V() * 3 : U32), 4))))

def griffinwing.bot.Session.bot_with.fire?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool, +lamp_open: J.Val, +latched: J.Val, +fliers: List<&2, J.Val>, +held: Bool, +target: J.Val, +aim: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_with.bomb(s, c, s_c, mx, my, brake, fliers, aim,
    griffinwing.bot.Session.fire_key(s, id, n, novice, c, lamp_open, latched, fliers, held, target))

# Whether F is held down this tick.

def griffinwing.bot.Session.fire_key?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +lamp_open: J.Val, +latched: J.Val, +fliers: List<&2, J.Val>, +held: Bool, +target: J.Val) -> Bool:
  gw.ifb(held,
    _ => Bool.not(griffinwing.bot.Session.fire_done(s, id, n, novice, c, lamp_open, latched, fliers)),
    _ => gw.ifb(U32.is_gt(ga.u(c, "held"), 0),
      _ => True{},
      _ => gw.ifb(Bool.or(Bool.not(J.is_null(lamp_open)), U32.is_ge(gw.len_j(fliers), 3)),
        _ => U32.is_zero(U32.and(ga.u(c, "keys"), griffinwing.flight.K_FIRE())),
        _ => gw.ifb(Bool.not(J.is_null(target)),
          _ => U32.is_zero(U32.and(ga.u(c, "keys"), griffinwing.flight.K_FIRE())),
          _ => False{}))))

def gw.len_j?(+xs: List<&2, J.Val>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, +t}:
      (1 + gw.len_j(t) : U32)

# Hold until the locks are in or the charge is full, then let go. A lamp wants
# the whole charge; a flight, whatever is in hand. A novice lets go early now and
# then, a lamp's charge unfinished.

def griffinwing.bot.Session.fire_done?(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +lamp_open: J.Val, +latched: J.Val, +fliers: List<&2, J.Val>) -> Bool:
  griffinwing.bot.Session.fire_done2(n, c, griffinwing.types.Craft.lock_total(ga.items(c, "locks")),
    Bool.and(novice, U32.is_eq(griffinwing.bot.noise(id, U32.div(ga.u(s, "t"), 90), 3, 2), 0)),
    Bool.and(Bool.not(J.is_null(lamp_open)), J.is_null(latched)),
    U32.min(gw.len_j(fliers), griffinwing.types.MAX_LOCKS()))

def griffinwing.bot.Session.fire_done2?(+n: J.Val, +c: J.Val, +total: U32, +early: Bool, +lamp: Bool, +want_locks: U32) -> Bool:
  gw.ifb(lamp,
    _ => Bool.or(Bool.and(U32.is_ge(ga.u(c, "charge"), ga.u(n, "charge")), U32.is_ge(total, 2)), Bool.and(early, U32.is_ge(ga.u(c, "charge"), U32.div(ga.u(n, "charge"), 2)))),
    _ => Bool.or(Bool.and(U32.is_ge(total, want_locks), U32.is_ge(ga.u(c, "charge"), U32.div(ga.u(n, "charge"), 2))), U32.is_ge(ga.u(c, "charge"), ga.u(n, "charge"))))

# Starfall when worn down in a crowded sky.

def griffinwing.bot.Session.bot_with.bomb?(+s: J.Val, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool, +fliers: List<&2, J.Val>, +aim: J.Val, +fire: Bool) -> J.Val:
  griffinwing.bot.Input.make(mx, my, ga.nu(aim, 0), ga.nu(aim, 1), fire, False{},
    Bool.and(gw.le(ga.u(c, "shield"), 35),
      Bool.and(U32.is_ge(griffinwing.bot.Session.crowd(fliers, s_c), 3),
        Bool.and(U32.is_gt(ga.u(c, "bombs"), 0), U32.is_zero(U32.and(ga.u(c, "keys"), griffinwing.flight.K_BOMB()))))),
    brake)

def griffinwing.bot.Session.crowd?(+fs: List<&2, J.Val>, +s_c: U32) -> U32:
  match fs:
    case Nil{}:
      0
    case Con{+f, +t}:
      (Bool.to_u32(gw.lt((ga.u(f, "s") - s_c : U32), griffinwing.types.dm(260))) + griffinwing.bot.Session.crowd(t, s_c) : U32)

# Whether this knight is one of the rollers: the even seats, and any knight at
# all when no even seat is flying.

def griffinwing.bot.Session.reflector?(+s: J.Val, +id: U32) -> Bool:
  Bool.or(U32.is_zero(U32.mod(id, 2)), Bool.not(griffinwing.bot.Session.even_flying(s, ga.ids(J.get(s, "seats")))))

def griffinwing.bot.Session.even_flying?(+s: J.Val, +ids: List<&2, U32>) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case Con{+k, +t}:
      Bool.or(Bool.and(U32.is_zero(U32.mod(k, 2)), griffinwing.types.Craft.flying(gw.craft_of(s, k))), griffinwing.bot.Session.even_flying(s, t))

# While the lit lamp is shut, the soonest of the Wyrm's spits that is owed to
# it: [ticks, x, z] or null.

def griffinwing.bot.Session.intercept?(+s: J.Val, +s_c: U32) -> J.Val:
  gw.ifj(J.is_null(J.get(s, "boss")),
    _ => J.JNull{},
    _ => griffinwing.bot.Session.intercept2(s, s_c, J.u32(J.nth(J.get(J.get(s, "boss"), "lamps"), U32.min(ga.u(J.get(s, "boss"), "phase"), 2)))))

def griffinwing.bot.Session.intercept2?(+s: J.Val, +s_c: U32, +lit: U32) -> J.Val:
  gw.ifj(griffinwing.bot.Session.shut(ga.items(s, "foes"), lit),
    _ => griffinwing.bot.Session.intercept3(ga.items(s, "bolts"), s_c, lit, J.JNull{}),
    _ => J.JNull{})

def griffinwing.bot.Session.shut?(+fs: List<&2, J.Val>, +lit: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      Bool.or(Bool.and(U32.is_eq(ga.u(f, "id"), lit), gw.isn(f, "st", "Shut")), griffinwing.bot.Session.shut(t, lit))

# The first of the least ticks.

def griffinwing.bot.Session.intercept3?(+bs: List<&2, J.Val>, +s_c: U32, +lit: U32, +best: J.Val) -> J.Val:
  match bs:
    case Nil{}:
      best
    case Con{+b, +t}:
      gw.ifj(Bool.and(gw.isn(b, "shot", "Gloam"), Bool.and(U32.is_eq(ga.u(b, "src"), lit), gw.gt(ga.u(b, "s"), s_c))),
        _ => griffinwing.bot.Session.intercept4(b, t, s_c, lit, best,
          gw.max(gw.div((ga.u(b, "s") - s_c : U32), gw.max((griffinwing.types.PACE() - ga.u(b, "vs") : U32), 1)), 0)),
        _ => griffinwing.bot.Session.intercept3(t, s_c, lit, best))

def griffinwing.bot.Session.intercept4?(+b: J.Val, +t: List<&2, J.Val>, +s_c: U32, +lit: U32, +best: J.Val, +ticks: U32) -> J.Val:
  gw.ifj(Bool.or(J.is_null(best), U32.is_lt(ticks, ga.nu(best, 0))),
    _ => griffinwing.bot.Session.intercept3(t, s_c, lit,
      J.JA{[J.JU{ticks}, J.of_i32((ga.u(b, "x") + (ga.u(b, "vx") * ticks : U32) : U32)), J.of_i32((ga.u(b, "z") + (ga.u(b, "vz") * ticks : U32) : U32))]}),
    _ => griffinwing.bot.Session.intercept3(t, s_c, lit, best))

# The soonest hostile bolt that will pass near this craft's plane: [ticks to it,
# x, z where it will cross, whether it is a Gloam bolt] or null.

def griffinwing.bot.Session.threat?(+s: J.Val, +c: J.Val, +s_c: U32) -> J.Val:
  griffinwing.bot.Session.threat2(ga.items(s, "bolts"), c, s_c, J.JNull{})

def griffinwing.bot.Session.threat2?(+bs: List<&2, J.Val>, +c: J.Val, +s_c: U32, +best: J.Val) -> J.Val:
  match bs:
    case Nil{}:
      best
    case Con{+b, +t}:
      gw.ifj(Bool.and(Bool.or(gw.isn(b, "shot", "Spark"), gw.isn(b, "shot", "Gloam")),
          gw.within((ga.u(b, "s") - s_c : U32), 1, griffinwing.types.dm(220))),
        _ => griffinwing.bot.Session.threat3(b, t, c, s_c, best,
          gw.max(gw.div((ga.u(b, "s") - s_c : U32), gw.max((griffinwing.types.PACE() - ga.u(b, "vs") : U32), 1)), 0)),
        _ => griffinwing.bot.Session.threat2(t, c, s_c, best))

def griffinwing.bot.Session.threat3?(+b: J.Val, +t: List<&2, J.Val>, +c: J.Val, +s_c: U32, +best: J.Val, +ticks: U32) -> J.Val:
  griffinwing.bot.Session.threat4(b, t, c, s_c, best, ticks,
    (ga.u(b, "x") + (ga.u(b, "vx") * ticks : U32) : U32), (ga.u(b, "z") + (ga.u(b, "vz") * ticks : U32) : U32))

def griffinwing.bot.Session.threat4?(+b: J.Val, +t: List<&2, J.Val>, +c: J.Val, +s_c: U32, +best: J.Val, +ticks: U32, +px: U32, +pz: U32) -> J.Val:
  gw.ifj(Bool.and(gw.nlt(griffinwing.types.flat(px, pz, ga.u(c, "x"), ga.u(c, "z")), griffinwing.types.dm(26)),
      Bool.or(J.is_null(best), U32.is_lt(ticks, ga.nu(best, 0)))),
    _ => griffinwing.bot.Session.threat2(t, c, s_c, J.JA{[J.JU{ticks}, J.of_i32(px), J.of_i32(pz), J.JBool{gw.isn(b, "shot", "Gloam")}]}),
    _ => griffinwing.bot.Session.threat2(t, c, s_c, best))

# Where to go to be clear of the point `(ox, oz)`: four ways out of it (left,
# right, up, down), those the steer box allows, the one that ends farthest from
# it; the last of equals wins (`max_by_key`).

def griffinwing.bot.Session.room_from?(+c: J.Val, +ox: U32, +oz: U32, +n: J.Val) -> J.Val:
  griffinwing.bot.Session.room_from.pick(c, ox, oz, n,
    ga.pair_i(0, 0), 0, 0)

def griffinwing.bot.Session.room_from.pick?(+c: J.Val, +ox: U32, +oz: U32, +n: J.Val, +best: J.Val, +bd: U32, +k: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, 4),
    _ => griffinwing.bot.Session.room_from.cand(c, ox, oz, n, best, bd, k,
      gw.clamp((ga.u(c, "x") + griffinwing.bot.Session.room_from.dx(k) : U32), gw.neg(griffinwing.Nums.wx(n)), griffinwing.Nums.wx(n)),
      gw.clamp((ga.u(c, "z") + griffinwing.bot.Session.room_from.dz(k) : U32), gw.neg(griffinwing.Nums.wz(n)), griffinwing.Nums.wz(n))),
    _ => best)

def griffinwing.bot.Session.room_from.dx?(+k: U32) -> U32:
  Bool.pick(U32, U32.is_eq(k, 0), griffinwing.types.dm(40), Bool.pick(U32, U32.is_eq(k, 1), gw.neg(griffinwing.types.dm(40)), 0))

def griffinwing.bot.Session.room_from.dz?(+k: U32) -> U32:
  Bool.pick(U32, U32.is_eq(k, 2), griffinwing.types.dm(30), Bool.pick(U32, U32.is_eq(k, 3), gw.neg(griffinwing.types.dm(30)), 0))

def griffinwing.bot.Session.room_from.cand?(+c: J.Val, +ox: U32, +oz: U32, +n: J.Val, +best: J.Val, +bd: U32, +k: U32, +x: U32, +z: U32) -> J.Val:
  griffinwing.bot.Session.room_from.cmp(c, ox, oz, n, best, bd, k, x, z, griffinwing.types.flat(x, z, ox, oz))

def griffinwing.bot.Session.room_from.cmp?(+c: J.Val, +ox: U32, +oz: U32, +n: J.Val, +best: J.Val, +bd: U32, +k: U32, +x: U32, +z: U32, +d: U32) -> J.Val:
  gw.ifj(Bool.or(U32.is_zero(k), U32.is_ge(d, bd)),
    _ => griffinwing.bot.Session.room_from.pick(c, ox, oz, n, ga.pair_i(x, z), d, (k + 1 : U32)),
    _ => griffinwing.bot.Session.room_from.pick(c, ox, oz, n, best, bd, (k + 1 : U32)))

# A footprint locked on this craft (a turret's, a bomber's lane): the place to
# be instead. [x, z] or null.

def griffinwing.bot.Session.footprint_away?(+s: J.Val, +c: J.Val, +n: J.Val) -> J.Val:
  griffinwing.bot.Session.footprint_away.foes(s, c, n, ga.items(s, "foes"))

def griffinwing.bot.Session.footprint_away.foes?(+s: J.Val, +c: J.Val, +n: J.Val, +fs: List<&2, J.Val>) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      gw.ifj(Bool.and(gw.isn(f, "st", "Tell"), Bool.or(gw.isn(f, "kind", "Turret"), Bool.or(gw.isn(f, "kind", "Bomber"), gw.isn(f, "kind", "Wisp")))),
        _ => griffinwing.bot.Session.footprint_away.marks(s, c, n, t, J.items(griffinwing.combat.Session.marks(f, griffinwing.Session.pressure(s)))),
        _ => griffinwing.bot.Session.footprint_away.foes(s, c, n, t))

def griffinwing.bot.Session.footprint_away.marks?(+s: J.Val, +c: J.Val, +n: J.Val, +t: List<&2, J.Val>, +ms: List<&2, J.Val>) -> J.Val:
  match ms:
    case Nil{}:
      griffinwing.bot.Session.footprint_away.foes(s, c, n, t)
    case Con{+m, +mt}:
      gw.ifj(gw.nlt(griffinwing.types.flat(ga.u(c, "x"), ga.u(c, "z"), ga.nu(m, 0), ga.nu(m, 1)), griffinwing.types.dm(24)),
        _ => griffinwing.bot.Session.room_from(c, ga.nu(m, 0), ga.nu(m, 1), n),
        _ => griffinwing.bot.Session.footprint_away.marks(s, c, n, t, mt))

# The Wyrm's coil wants the gap, its roar wants the column left. [x, z] or null.

def griffinwing.bot.Session.boss_corridor?(+s: J.Val, +c: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(J.is_null(J.get(s, "boss")),
    _ => J.JNull{},
    _ => griffinwing.bot.Session.boss_corridor2(c, n, J.get(s, "boss")))

def griffinwing.bot.Session.boss_corridor2?(+c: J.Val, +n: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(Bool.or(gw.isn(b, "st", "Tell"), Bool.or(gw.isn(b, "st", "Coil"), gw.isn(b, "st", "Roar"))),
    _ => gw.ifj(U32.is_eq(ga.u(b, "atk"), 1),
      _ => ga.pair_i(ga.u(b, "gap"), ga.u(c, "z")),
      _ => gw.ifj(U32.is_eq(ga.u(b, "atk"), 2),
        _ => griffinwing.bot.Session.boss_corridor3(c, n, b),
        _ => J.JNull{})),
    _ => J.JNull{})

def griffinwing.bot.Session.boss_corridor3?(+c: J.Val, +n: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(gw.lt(gw.abs((ga.u(c, "x") - ga.u(b, "col") : U32)), (griffinwing.boss.COL_HALF() + griffinwing.types.dm(16) : U32)),
    _ => ga.pair_i(
      gw.clamp((ga.u(b, "col") + (gw.ifu(gw.ge(ga.u(c, "x"), ga.u(b, "col")), _ => 1, _ => gw.neg(1)) * (griffinwing.boss.COL_HALF() + griffinwing.types.dm(30)) : U32) : U32),
        gw.neg(griffinwing.Nums.wx(n)), griffinwing.Nums.wx(n)),
      ga.u(c, "z")),
    _ => J.JNull{})

def case.griffinwing.types.norm?(+i: J.Val) -> J.Val:
  J.JU{griffinwing.types.norm(ga.nu(i, 0), ga.nu(i, 1), ga.nu(i, 2))}

def case.griffinwing.types.flat?(+i: J.Val) -> J.Val:
  J.JU{griffinwing.types.flat(ga.nu(i, 0), ga.nu(i, 1), ga.nu(i, 2), ga.nu(i, 3))}

def case.griffinwing.types.scale3?(+i: J.Val) -> J.Val:
  gw.v3j(griffinwing.types.scale3(ga.nu(J.get(i, "d"), 0), ga.nu(J.get(i, "d"), 1), ga.nu(J.get(i, "d"), 2), ga.u(i, "len")))

def case.griffinwing.types.tri?(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.types.tri(ga.u(i, "t"), ga.u(i, "period"), ga.u(i, "amp")))

def case.griffinwing.types.near?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.near(
    ga.nu(J.get(i, "a"), 0), ga.nu(J.get(i, "a"), 1), ga.nu(J.get(i, "a"), 2),
    ga.nu(J.get(i, "b"), 0), ga.nu(J.get(i, "b"), 1), ga.nu(J.get(i, "b"), 2),
    ga.nu(J.get(i, "p"), 0), ga.nu(J.get(i, "p"), 1), ga.nu(J.get(i, "p"), 2), ga.u(i, "r"))}

def case.griffinwing.types.Foe.alive?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.Foe.alive(i)}

def case.griffinwing.types.Foe.lockable?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.Foe.lockable(i)}

def case.griffinwing.types.Craft.new?(+i: J.Val) -> J.Val:
  griffinwing.types.Craft.new(ga.u(i, "x"), ga.u(i, "z"))

def case.griffinwing.types.Craft.valid?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.Craft.valid(i)}

def case.griffinwing.flight.Craft.reflecting?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.flight.Craft.reflecting(J.get(i, "c"), ga.u(i, "reflect"))}

def case.griffinwing.types.Beast.valid?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.Beast.valid(i)}

def case.griffinwing.types.Tally.valid?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.Tally.valid(i)}

def case.griffinwing.types.Tally.bank?(+i: J.Val) -> J.Val:
  griffinwing.types.Tally.bank(J.get(i, "a"), J.get(i, "b"))

def case.griffinwing.things.shutter_gap?(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.things.shutter_gap(J.get(i, "p"), ga.u(i, "t"), ga.u(i, "wx")))

def case.griffinwing.flight.bits?(+i: J.Val) -> J.Val:
  J.JU{griffinwing.flight.bits(i)}

def case.griffinwing.flight.slot?(+i: J.Val) -> J.Val:
  griffinwing.flight.slot(J.u32(i))

def case.griffinwing.flight.rider_slot?(+i: J.Val) -> J.Val:
  griffinwing.flight.rider_slot(J.u32(i))

def case.griffinwing.bit?(+i: J.Val) -> J.Val:
  J.JU{griffinwing.bit(J.u32(i))}

def case.griffinwing.Nums.of?(+i: J.Val) -> J.Val:
  griffinwing.Nums.of(J.get(i, "tune"))

def case.griffinwing.Nums.wx?(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.Nums.wx(i))

def case.griffinwing.Nums.wz?(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.Nums.wz(i))

def case.griffinwing.Nums.reflect?(+i: J.Val) -> J.Val:
  J.JU{griffinwing.Nums.reflect(i)}

def case.griffinwing.Session.new?(+i: J.Val) -> J.Val:
  griffinwing.Session.new(J.get(i, "players"))

def case.griffinwing.Session.sit?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.sit(J.get(i, "s"), ga.u(i, "id"), ga.u(i, "joined")))

# {"s": s}

def gw.out_only?(+s: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", s)

def case.griffinwing.Session.anchor?(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.Session.anchor(i))

def case.griffinwing.Session.pressure?(+i: J.Val) -> J.Val:
  J.JU{griffinwing.Session.pressure(i)}

def case.griffinwing.Session.next_id?(+i: J.Val) -> J.Val:
  griffinwing.Session.next_id(J.get(i, "s"))

def case.griffinwing.Session.sound?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.sound(J.get(i, "s"), ga.s(i, "name")))

def case.griffinwing.Session.say?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.say(J.get(i, "s"), ga.u(i, "who"), ga.s(i, "line")))

def case.griffinwing.Session.spark?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.spark(J.get(i, "s"), ga.s(i, "what"), ga.nu(J.get(i, "at"), 0), ga.nu(J.get(i, "at"), 1), ga.nu(J.get(i, "at"), 2), ga.u(i, "n")))

def case.griffinwing.Session.voice?(+i: J.Val) -> J.Val:
  J.JU{griffinwing.Session.voice(i)}

def case.griffinwing.Session.fill_wing?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.fill_wing(J.get(i, "s")))

def case.griffinwing.Session.muster?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.muster(J.get(i, "s"), J.get(i, "heroes")))

def case.griffinwing.Session.step?(+i: J.Val) -> J.Val:
  griffinwing.Session.step(J.get(i, "s"), J.get(i, "inputs"), J.get(i, "heroes"), J.get(i, "n"))

def case.griffinwing.Session.tick_lists?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.tick_lists(J.get(i, "s")))

def case.griffinwing.Session.end_beat?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.end_beat(J.get(i, "s")))

def case.griffinwing.Session.next_beat?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.next_beat(J.get(i, "s")))

def case.griffinwing.Session.restart?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.restart(J.get(i, "s")))

def case.griffinwing.Session.finish?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.finish(J.get(i, "s")))

def case.griffinwing.Session.medal_now?(+i: J.Val) -> J.Val:
  J.JS{griffinwing.Session.medal_now(i)}

def case.griffinwing.Session.purse?(+i: J.Val) -> J.Val:
  J.JU{griffinwing.Session.purse(i)}

def case.griffinwing.Session.finished?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.Session.finished(i)}

def case.griffinwing.Session.permille?(+i: J.Val) -> J.Val:
  J.JU{griffinwing.Session.permille(i)}

def case.griffinwing.flight.Session.craft_s?(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.flight.Session.craft_s(J.get(i, "s"), J.get(i, "c")))

def case.griffinwing.flight.Session.craft_at?(+i: J.Val) -> J.Val:
  griffinwing.flight.Session.craft_at(J.get(i, "s"), ga.u(i, "id"))

def case.griffinwing.flight.Session.fly?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.flight.Session.fly(J.get(i, "s"), ga.u(i, "id"), J.get(i, "i"), J.get(i, "n")))

def case.griffinwing.flight.Session.wing_fly?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.flight.Session.wing_fly(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.flight.Session.step_craft?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.flight.Session.step_craft(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.combat.Session.step_foes?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.step_foes(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.combat.Session.step_bolts?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.step_bolts(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.boss.Session.step_boss?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.boss.Session.step_boss(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.script.Session.run_script?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.script.Session.run_script(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.things.Session.step_props?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.things.Session.step_props(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.Session.valid?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.Session.valid(i)}

def case.griffinwing.Session.repair?(+i: J.Val) -> J.Val:
  griffinwing.Session.repair(i)

def case.griffinwing.bot.Session.bot_with?(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_with(J.get(i, "s"), ga.u(i, "id"), J.get(i, "n"), ga.b(i, "novice"))

def case.griffinwing.bot.Session.bot_input?(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_input(J.get(i, "s"), ga.u(i, "id"))

def case.griffinwing.bot.noise?(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.bot.noise(ga.u(i, "id"), ga.u(i, "t"), ga.u(i, "salt"), ga.u(i, "span")))

def case.griffinwing.bot.Session.reflector?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.bot.Session.reflector(J.get(i, "s"), ga.u(i, "id"))}

def case.griffinwing.bot.Session.intercept?(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.intercept(J.get(i, "s"), ga.u(i, "s_c"))

def case.griffinwing.bot.Session.threat?(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.threat(J.get(i, "s"), J.get(i, "c"), ga.u(i, "s_c"))

def case.griffinwing.bot.Session.room_from?(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.room_from(J.get(i, "c"), ga.u(i, "ox"), ga.u(i, "oz"), J.get(i, "n"))

def case.griffinwing.bot.Session.footprint_away?(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.footprint_away(J.get(i, "s"), J.get(i, "c"), J.get(i, "n"))

def case.griffinwing.bot.Session.boss_corridor?(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.boss_corridor(J.get(i, "s"), J.get(i, "c"), J.get(i, "n"))

# {"s": ws.s, "c": ws.c} with the thing under another name.

def gw.named?(+w: J.Val, +k: String) -> J.Val:
  ga.sv(ga.sv(J.obj(), "s", gw.ps(w)), k, gw.pc(w))

# ─── flight ─────────────────────────────────────────────────────────────────

def case.griffinwing.flight.Session.push_bolt?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.flight.Session.push_bolt(J.get(i, "s"), J.get(i, "b")))

def case.griffinwing.flight.Session.tap?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.flight.Session.tap(J.get(i, "s"), ga.u(i, "owner"), J.get(i, "c"), ga.u(i, "s_c")))

def case.griffinwing.flight.Session.acquire?(+i: J.Val) -> J.Val:
  griffinwing.flight.Session.acquire(J.get(i, "s"), J.get(i, "c"), ga.u(i, "s_c"))

def case.griffinwing.flight.Session.release?(+i: J.Val) -> J.Val:
  griffinwing.flight.Session.release(J.get(i, "s"), ga.u(i, "owner"), J.get(i, "c"), ga.u(i, "s_c"), J.get(i, "n"))

# ─── combat ─────────────────────────────────────────────────────────────────

def case.griffinwing.combat.Session.spawn_foe?(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.spawn_foe(J.get(i, "s"), ga.s(i, "kind"),
    ga.nu(J.get(i, "at"), 0), ga.nu(J.get(i, "at"), 1), ga.nu(J.get(i, "at"), 2),
    ga.nu(J.get(i, "vel"), 0), ga.nu(J.get(i, "vel"), 1), ga.nu(J.get(i, "vel"), 2),
    ga.s(i, "st"), ga.u(i, "mode"), ga.u(i, "phase"))

def case.griffinwing.combat.Session.hurt_craft?(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.hurt_craft(J.get(i, "s"), ga.u(i, "id"), ga.u(i, "dmg"))

def case.griffinwing.combat.Session.drain?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.drain(J.get(i, "s"), ga.u(i, "id"), ga.u(i, "amount")))

def case.griffinwing.combat.Session.latched?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.combat.Session.latched(J.get(i, "s"), ga.u(i, "id"))}

def case.griffinwing.combat.Session.victim?(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.victim(J.get(i, "s"), ga.u(i, "hunter"))

def case.griffinwing.combat.Session.target?(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.target(J.get(i, "s"), ga.u(i, "turn"))

def case.griffinwing.combat.Session.plane_shot?(+i: J.Val) -> J.Val:
  gw.v3j(griffinwing.combat.Session.plane_shot(J.get(i, "s"),
    ga.nu(J.get(i, "o"), 0), ga.nu(J.get(i, "o"), 1), ga.nu(J.get(i, "o"), 2),
    ga.nu(J.get(i, "to"), 0), ga.nu(J.get(i, "to"), 1), ga.u(i, "closing")))

def case.griffinwing.combat.Session.marks?(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.marks(J.get(i, "f"), ga.u(i, "seats"))

def case.griffinwing.combat.Session.fire_marks?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.fire_marks(J.get(i, "s"), J.get(i, "f")))

def case.griffinwing.combat.Session.step_foe?(+i: J.Val) -> J.Val:
  gw.named(griffinwing.combat.Session.step_foe(J.get(i, "s"), J.get(i, "f"), ga.u(i, "anchor"), J.get(i, "n")), "f")

def case.griffinwing.combat.Session.hunt?(+i: J.Val) -> J.Val:
  gw.named(griffinwing.combat.Session.hunt(J.get(i, "s"), J.get(i, "f")), "f")

def case.griffinwing.combat.Session.hang?(+i: J.Val) -> J.Val:
  gw.named(griffinwing.combat.Session.hang(J.get(i, "s"), J.get(i, "f")), "f")

def case.griffinwing.combat.Session.bombers_ram?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.bombers_ram(J.get(i, "s")))

def case.griffinwing.combat.Session.hit_foe?(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.hit_foe(J.get(i, "s"), ga.u(i, "fi"), ga.u(i, "dmg"), ga.u(i, "by"))

def case.griffinwing.combat.Session.kill?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.kill(J.get(i, "s"), ga.u(i, "fi"), ga.u(i, "by")))

def case.griffinwing.combat.Session.break_lamp?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.break_lamp(J.get(i, "s"), ga.u(i, "fi"), ga.u(i, "by")))

def case.griffinwing.combat.Session.starfall?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.starfall(J.get(i, "s"), ga.u(i, "by"), ga.u(i, "s_c"), ga.u(i, "x"), ga.u(i, "z")))

def case.griffinwing.combat.Session.friendly_bolt?(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.friendly_bolt(J.get(i, "s"), J.get(i, "b"),
    ga.nu(J.get(i, "from"), 0), ga.nu(J.get(i, "from"), 1), ga.nu(J.get(i, "from"), 2),
    ga.nu(J.get(i, "to"), 0), ga.nu(J.get(i, "to"), 1), ga.nu(J.get(i, "to"), 2), J.get(i, "dead"))

def case.griffinwing.combat.Session.land?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.land(J.get(i, "s"), ga.u(i, "i"), J.get(i, "b")))

def case.griffinwing.combat.Session.hostile_bolt?(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.hostile_bolt(J.get(i, "s"), J.get(i, "b"),
    ga.nu(J.get(i, "from"), 0), ga.nu(J.get(i, "from"), 1), ga.nu(J.get(i, "from"), 2),
    ga.nu(J.get(i, "to"), 0), ga.nu(J.get(i, "to"), 1), ga.nu(J.get(i, "to"), 2), J.get(i, "n"))

def case.griffinwing.combat.Session.turn?(+i: J.Val) -> J.Val:
  case.griffinwing.combat.Session.turn.drop(griffinwing.combat.Session.turn(J.get(i, "s"), J.get(i, "b"), ga.u(i, "by"),
    ga.nu(J.get(i, "at"), 0), ga.nu(J.get(i, "at"), 1), ga.nu(J.get(i, "at"), 2)))

def case.griffinwing.combat.Session.turn.drop?(+r: J.Val) -> J.Val:
  J.del(r, "r")

# ─── boss ───────────────────────────────────────────────────────────────────

def case.griffinwing.boss.Session.rise?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.boss.Session.rise(J.get(i, "s")))

def case.griffinwing.boss.Session.rest?(+i: J.Val) -> J.Val:
  J.JU{griffinwing.boss.Session.rest(J.get(i, "s"), ga.u(i, "phase"))}

def case.griffinwing.boss.Session.begin_tell?(+i: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell(J.get(i, "s"), J.get(i, "b"), J.get(i, "n"))

def case.griffinwing.boss.Session.spit?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.boss.Session.spit(J.get(i, "s"), J.get(i, "b")))

# ─── script ─────────────────────────────────────────────────────────────────

def case.griffinwing.script.script?(+i: J.Val) -> J.Val:
  griffinwing.script.script(J.u32(i))

def case.griffinwing.script.resume?(+i: J.Val) -> J.Val:
  J.JU{griffinwing.script.resume(ga.u(i, "stage"), ga.u(i, "cp"))}

def case.griffinwing.script.Session.spread?(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.script.Session.spread(ga.u(i, "k"), ga.u(i, "count")))

def case.griffinwing.script.Session.act?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.script.Session.act(J.get(i, "s"), J.get(i, "act")))

def case.griffinwing.script.Session.victory_gates?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.script.Session.victory_gates(J.get(i, "s")))

# ─── things ─────────────────────────────────────────────────────────────────

def case.griffinwing.things.Session.take?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.things.Session.take(J.get(i, "s"), ga.u(i, "id"), J.get(i, "p"), ga.b(i, "boosting")))

def case.griffinwing.things.Session.scrape?(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.things.Session.scrape(J.get(i, "s"), ga.u(i, "id"), ga.u(i, "at"), ga.u(i, "x"), ga.u(i, "z")))

# ─── the Run's hooks ────────────────────────────────────────────────────────

def case.griffinwing.Run.griffinwing_showing?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.Run.griffinwing_showing(J.get(i, "run"))}

def case.griffinwing.Run.start_griffinwing?(+i: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", griffinwing.Run.start_griffinwing(J.get(i, "run")))

def case.griffinwing.Run.tick_griffinwing?(+i: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", griffinwing.Run.tick_griffinwing(J.get(i, "run"), J.get(i, "inputs")))

def case.griffinwing.Run.settle_griffinwing?(+i: J.Val) -> J.Val:
  case.griffinwing.Run.settle.pack(griffinwing.Run.settle_griffinwing(J.get(i, "run"), J.get(i, "s")))

def case.griffinwing.Run.settle.pack?(+w: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "run", gw.ps(w)), "s", gw.pc(w))

def case.griffinwing.ADVENTURE.valid?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.ADVENTURE.valid(J.get(i, "run"))}

def case.griffinwing.ADVENTURE.clear?(+i: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", griffinwing.ADVENTURE.clear(J.get(i, "run")))

def case.griffinwing.ADVENTURE.seated?(+i: J.Val) -> J.Val:
  griffinwing.ADVENTURE.seated(J.get(i, "run"))

def case.griffinwing.ADVENTURE.bot?(+i: J.Val) -> J.Val:
  griffinwing.ADVENTURE.bot(J.get(i, "run"), ga.u(i, "id"))

def case.griffinwing.ACTIVITY.on?(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.ACTIVITY.on(J.get(i, "run"))}

def case.griffinwing.ACTIVITY.ride?(+i: J.Val) -> J.Val:
  griffinwing.ACTIVITY.ride(J.get(i, "run"), J.get(i, "inputs"))

def case.griffinwing.ACTIVITY.end?(+i: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", griffinwing.ACTIVITY.end(J.get(i, "run")))

def case.griffinwing.settle?(+i: J.Val) -> J.Val:
  griffinwing.settle(J.get(i, "realm"), J.get(i, "marks"))

def griffinwing.combat.HUNT_V?() -> U32:
  griffinwing.types.dm(10)

def griffinwing.combat.LATCH_BACK?() -> U32:
  griffinwing.types.dm(18)

def griffinwing.combat.GLOAM?() -> U32:
  25

def griffinwing.combat.SPARK?() -> U32:
  10

def griffinwing.combat.GLOAM_R?() -> U32:
  griffinwing.types.dm(6)

def griffinwing.combat.SPARK_R?() -> U32:
  griffinwing.types.dm(5)

def griffinwing.combat.BOLT_R?() -> U32:
  griffinwing.types.dm(5)

# What a turned bolt does, and its pace home.

def griffinwing.combat.TURNED?() -> U32:
  60

def griffinwing.combat.V_TURN?() -> U32:
  griffinwing.types.dm(24)

# A dive-bomber's blow.

def griffinwing.combat.RAM?() -> U32:
  35

# How long a shield stays down after a hit lands on it, in ticks.

def griffinwing.combat.HURT?() -> U32:
  36

# Put a foe on the rail, its health provisioned for the knights at the table;
# what it is worth is offered to the medal. {"s", "r": id or null}.

def griffinwing.combat.Session.spawn_foe?(+s: J.Val, +kind: String, +at_s: U32, +at_x: U32, +at_z: U32, +vs: U32, +vx: U32, +vz: U32, +st: String, +mode: U32, +phase: U32) -> J.Val:
  gw.ifj(U32.is_ge(gw.len(s, "foes"), griffinwing.types.MAX_FOES()),
    _ => gw.out(s, J.JNull{}),
    _ => griffinwing.combat.Session.spawn_foe.in(griffinwing.Session.next_id(s), kind, at_s, at_x, at_z, vs, vx, vz, st, mode, phase, griffinwing.Session.pressure(s)))

def griffinwing.combat.Session.spawn_foe.in?(+r: J.Val, +kind: String, +at_s: U32, +at_x: U32, +at_z: U32, +vs: U32, +vx: U32, +vz: U32, +st: String, +mode: U32, +phase: U32, +seats: U32) -> J.Val:
  griffinwing.combat.Session.spawn_foe.push(gw.out_s(r), ga.u(gw.out_r(r), "id"), gw.out_r(r), kind, at_s, at_x, at_z, vs, vx, vz, st, mode, phase, seats)

def griffinwing.combat.Session.spawn_foe.push?(+s: J.Val, +id: U32, +idj: J.Val, +kind: String, +at_s: U32, +at_x: U32, +at_z: U32, +vs: U32, +vx: U32, +vz: U32, +st: String, +mode: U32, +phase: U32, +seats: U32) -> J.Val:
  gw.out(
    ga.sv(
      ga.push(s, "foes", griffinwing.combat.Foe.make(id, kind, at_s, at_x, at_z, vs, vx, vz, (griffinwing.types.Kind.health(kind) * seats : U32), seats, st, mode, phase)),
      "beat", gw.addu(J.get(s, "beat"), "offered", griffinwing.types.Kind.worth(kind))),
    idj)

def griffinwing.combat.Foe.make?(+id: U32, +kind: String, +s: U32, +x: U32, +z: U32, +vs: U32, +vx: U32, +vz: U32, +full: U32, +seats: U32, +st: String, +mode: U32, +phase: U32) -> J.Val:
  +f0 = J.obj()
  +f1 = ga.su(f0, "id", id)
  +f2 = ga.ss(f1, "kind", kind)
  +f3 = ga.si(f2, "s", s)
  +f4 = ga.si(f3, "x", x)
  +f5 = ga.si(f4, "z", z)
  +f6 = ga.si(f5, "vs", vs)
  +f7 = ga.si(f6, "vx", vx)
  +f8 = ga.si(f7, "vz", vz)
  +f9 = ga.su(f8, "hp", full)
  +f10 = ga.su(f9, "full", full)
  +f11 = ga.su(f10, "seats", seats)
  +f12 = ga.ss(f11, "st", st)
  +f13 = ga.su(f12, "ticks", 0)
  +f14 = ga.si(f13, "fx", 0)
  +f15 = ga.si(f14, "fz", 0)
  +f16 = ga.su(f15, "on", 0)
  +f17 = ga.su(f16, "flash", 0)
  +f18 = ga.su(f17, "mode", mode)
  ga.su(f18, "phase", phase)

# A craft's shield takes a blow. A knight is spared for a moment after one; a
# rider is only worn down (by what latches on). {"s", "r": whether it landed}.

def griffinwing.combat.Session.hurt_craft?(+s: J.Val, +id: U32, +dmg: U32) -> J.Val:
  gw.ifj(gw.within(id, 1, 6),
    _ => griffinwing.combat.Session.hurt_craft.seat(s, id, dmg, griffinwing.flight.Session.craft_at(s, id), J.get(J.get(s, "seats"), U32.show(id))),
    _ => griffinwing.combat.Session.hurt_craft.rider(s, dmg, griffinwing.flight.Session.wing_find(ga.items(s, "wing"), id)))

def griffinwing.combat.Session.hurt_craft.seat?(+s: J.Val, +id: U32, +dmg: U32, +at: J.Val, +seat: J.Val) -> J.Val:
  gw.ifj(J.is_null(seat),
    _ => gw.out(s, J.JBool{False{}}),
    _ => griffinwing.combat.Session.hurt_craft.seat2(s, id, dmg, at, J.get(seat, "craft")))

def griffinwing.combat.Session.hurt_craft.seat2?(+s: J.Val, +id: U32, +dmg: U32, +at: J.Val, +c: J.Val) -> J.Val:
  gw.ifj(Bool.or(U32.is_gt(ga.u(c, "hurt"), 0), U32.is_gt(ga.u(c, "down"), 0)),
    _ => gw.out(s, J.JBool{False{}}),
    _ => griffinwing.combat.Session.hurt_craft.hit(
      gw.put_craft(s, id, ga.su(ga.si(c, "shield", gw.max((ga.u(c, "shield") - dmg : U32), 0)), "hurt", griffinwing.combat.HURT())),
      id, dmg, at))

def griffinwing.combat.Session.hurt_craft.hit?(+s: J.Val, +id: U32, +dmg: U32, +at: J.Val) -> J.Val:
  griffinwing.combat.Session.hurt_craft.low(
    griffinwing.Session.sound(
      gw.ifj(gw.at_some(at), _ => griffinwing.Session.spark(s, "Hurt", gw.at_s(at), gw.at_x(at), gw.at_z(at), id), _ => s),
      gw.ifs(U32.is_ge(dmg, 20), _ => "knight_hit_big", _ => "hero_hurt")),
    id)

def gw.ifs?(c: Bool, a: Unit -> String, b: Unit -> String) -> String:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def griffinwing.combat.Session.hurt_craft.low?(+s: J.Val, +id: U32) -> J.Val:
  gw.out(
    gw.ifj(gw.le(ga.u(gw.craft_of(s, id), "shield"), 30), _ => griffinwing.Session.say(s, id, "Low"), _ => s),
    J.JBool{True{}})

def griffinwing.combat.Session.hurt_craft.rider?(+s: J.Val, +dmg: U32, +w: J.Val) -> J.Val:
  gw.ifj(J.is_null(w),
    _ => gw.out(s, J.JBool{False{}}),
    _ => griffinwing.combat.Session.hurt_craft.rider2(s, dmg, w))

def griffinwing.combat.Session.hurt_craft.rider2?(+s: J.Val, +dmg: U32, +w: J.Val) -> J.Val:
  gw.ifj(ga.b(w, "lost"),
    _ => gw.out(s, J.JBool{False{}}),
    _ => gw.out(
      griffinwing.combat.Session.wing_put(s, ga.u(w, "id"),
        ga.sv(w, "craft", ga.si(J.get(w, "craft"), "shield", gw.max((ga.u(J.get(w, "craft"), "shield") - dmg : U32), 0)))),
      J.JBool{True{}}))

# The rider by this id put back.

def griffinwing.combat.Session.wing_put?(+s: J.Val, +id: U32, +w: J.Val) -> J.Val:
  gw.put_item(s, "wing", griffinwing.combat.Session.wing_at(ga.items(s, "wing"), id, 0), w)

def griffinwing.combat.Session.wing_at?(+ws: List<&2, J.Val>, +id: U32, +k: U32) -> U32:
  match ws:
    case Nil{}:
      k
    case Con{+w, +t}:
      gw.ifu(U32.is_eq(ga.u(w, "id"), id), _ => k, _ => griffinwing.combat.Session.wing_at(t, id, (k + 1 : U32)))

# A drip from a latched kite: no spare moment, no flash.

def griffinwing.combat.Session.drain?(+s: J.Val, +id: U32, +amount: U32) -> J.Val:
  gw.ifj(gw.within(id, 1, 6),
    _ => griffinwing.combat.Session.drain.seat(s, id, amount, J.get(J.get(s, "seats"), U32.show(id))),
    _ => griffinwing.combat.Session.drain.rider(s, amount, griffinwing.flight.Session.wing_find(ga.items(s, "wing"), id)))

def griffinwing.combat.Session.drain.seat?(+s: J.Val, +id: U32, +amount: U32, +seat: J.Val) -> J.Val:
  gw.ifj(Bool.or(J.is_null(seat), U32.is_gt(ga.u(J.get(seat, "craft"), "down"), 0)),
    _ => s,
    _ => gw.put_craft(s, id, ga.si(J.get(seat, "craft"), "shield", gw.max((ga.u(J.get(seat, "craft"), "shield") - amount : U32), 0))))

def griffinwing.combat.Session.drain.rider?(+s: J.Val, +amount: U32, +w: J.Val) -> J.Val:
  gw.ifj(Bool.or(J.is_null(w), ga.b(w, "lost")),
    _ => s,
    _ => griffinwing.combat.Session.wing_put(s, ga.u(w, "id"),
      ga.sv(w, "craft", ga.si(J.get(w, "craft"), "shield", gw.max((ga.u(J.get(w, "craft"), "shield") - amount : U32), 0)))))

# Whether some kite hangs on this craft.

def griffinwing.combat.Session.latched?(+s: J.Val, +id: U32) -> Bool:
  griffinwing.combat.Session.latched.go(ga.items(s, "foes"), id)

def griffinwing.combat.Session.latched.go?(+fs: List<&2, J.Val>, +id: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      Bool.or(Bool.and(gw.isn(f, "st", "Latched"), U32.is_eq(ga.u(f, "on"), id)), griffinwing.combat.Session.latched.go(t, id))

# Whether this craft is flying and no kite hangs on it.

def griffinwing.combat.Session.victim.free?(+s: J.Val, +id: U32) -> Bool:
  Bool.and(
    Bool.and(gw.at_some(griffinwing.flight.Session.craft_at(s, id)), gw.at_ok(griffinwing.flight.Session.craft_at(s, id))),
    Bool.not(griffinwing.combat.Session.latched(s, id)))

# The ids of the free ones among the riders / the knights, in order.

def griffinwing.combat.Session.victim.riders?(+s: J.Val, +ws: List<&2, J.Val>) -> List<&2, U32>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      gw.ifl(griffinwing.combat.Session.victim.free(s, ga.u(w, "id")),
        _ => Con{ga.u(w, "id"), griffinwing.combat.Session.victim.riders(s, t)},
        _ => griffinwing.combat.Session.victim.riders(s, t))

def griffinwing.combat.Session.victim.knights?(+s: J.Val, +ids: List<&2, U32>) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gw.ifl(griffinwing.combat.Session.victim.free(s, id),
        _ => Con{id, griffinwing.combat.Session.victim.knights(s, t)},
        _ => griffinwing.combat.Session.victim.knights(s, t))

def gw.ifl?(c: Bool, a: Unit -> List<&2, U32>, b: Unit -> List<&2, U32>) -> List<&2, U32>:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

# The nth item of a list of ids.

def gw.nth_id?(+xs: List<&2, U32>, +n: U32) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, +t}:
      gw.ifu(U32.is_zero(n), _ => h, _ => gw.nth_id(t, (n - 1 : U32)))

def gw.len_id?(+xs: List<&2, U32>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, +t}:
      (1 + gw.len_id(t) : U32)

# Who a hunting kite goes for: a rider not yet latched, else a knight not yet
# latched, else anyone flying. The same hunter always finds the same victim.
# Answers an id, or null.

def griffinwing.combat.Session.victim?(+s: J.Val, +hunter: U32) -> J.Val:
  griffinwing.combat.Session.victim.pick(s, hunter,
    griffinwing.combat.Session.victim.riders(s, ga.items(s, "wing")))

def griffinwing.combat.Session.victim.pick?(+s: J.Val, +hunter: U32, +riders: List<&2, U32>) -> J.Val:
  gw.ifj(U32.is_zero(gw.len_id(riders)),
    _ => griffinwing.combat.Session.victim.seats(s, hunter, griffinwing.combat.Session.victim.knights(s, ga.ids(J.get(s, "seats")))),
    _ => J.JU{gw.nth_id(riders, U32.mod(hunter, gw.len_id(riders)))})

def griffinwing.combat.Session.victim.seats?(+s: J.Val, +hunter: U32, +knights: List<&2, U32>) -> J.Val:
  gw.ifj(U32.is_zero(gw.len_id(knights)),
    _ => griffinwing.combat.Session.victim.any(s, ga.ids(J.get(s, "seats"))),
    _ => J.JU{gw.nth_id(knights, U32.mod(hunter, gw.len_id(knights)))})

def griffinwing.combat.Session.victim.any?(+s: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      J.JNull{}
    case Con{+id, +t}:
      gw.ifj(Bool.and(gw.at_some(griffinwing.flight.Session.craft_at(s, id)), gw.at_ok(griffinwing.flight.Session.craft_at(s, id))),
        _ => J.JU{id},
        _ => griffinwing.combat.Session.victim.any(s, t))

# The (x, z) of the knights flying, in seat order.

def griffinwing.combat.Session.flying?(+s: J.Val) -> List<&2, J.Val>:
  griffinwing.combat.Session.flying.go(ga.ids(J.get(s, "seats")), s)

def griffinwing.combat.Session.flying.go?(+ids: List<&2, U32>, +s: J.Val) -> List<&2, J.Val>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gw.ifjl(griffinwing.types.Craft.flying(gw.craft_of(s, id)),
        _ => Con{ga.pair_i(ga.u(gw.craft_of(s, id), "x"), ga.u(gw.craft_of(s, id), "z")), griffinwing.combat.Session.flying.go(t, s)},
        _ => griffinwing.combat.Session.flying.go(t, s))

def gw.ifjl?(c: Bool, a: Unit -> List<&2, J.Val>, b: Unit -> List<&2, J.Val>) -> List<&2, J.Val>:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def gw.nth_j?(+xs: List<&2, J.Val>, +n: U32) -> J.Val:
  match xs:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      gw.ifj(U32.is_zero(n), _ => h, _ => gw.nth_j(t, (n - 1 : U32)))

# Who a turret or the Wyrm fires at: the knights flying, in turn. [x, z] or null.

def griffinwing.combat.Session.target?(+s: J.Val, +turn: U32) -> J.Val:
  griffinwing.combat.Session.target.of(griffinwing.combat.Session.flying(s), turn)

def griffinwing.combat.Session.target.of?(+fl: List<&2, J.Val>, +turn: U32) -> J.Val:
  match fl:
    case Nil{}:
      J.JNull{}
    case _:
      gw.nth_j(fl, U32.mod(turn, U32.from_nat(List.length(&2, J.Val, fl))))

# A bolt from `o` that meets the formation's plane at `to` (across, up) as the
# rail brings the plane to it, closing at `closing` a tick.

def griffinwing.combat.Session.plane_shot?(+s: J.Val, +os: U32, +ox: U32, +oz: U32, +tx: U32, +tz: U32, +closing: U32) -> GwV3:
  griffinwing.combat.Session.plane_shot.go(ox, oz, tx, tz, closing,
    gw.max(gw.div(gw.max((os - griffinwing.Session.anchor(s) : U32), griffinwing.types.dm(40)), gw.max(closing, 1)), 1))

def griffinwing.combat.Session.plane_shot.go?(+ox: U32, +oz: U32, +tx: U32, +tz: U32, +closing: U32, +ticks: U32) -> GwV3:
  GwV3{(griffinwing.types.PACE() - closing : U32), gw.div((tx - ox : U32), ticks), gw.div((tz - oz : U32), ticks)}

# ─── the foes' tick ─────────────────────────────────────────────────────────

# The foes' tick: flights, tells, latching, dives. The foes are taken out of
# the session while each is stepped (so a hunter finds no latched kite), then
# put back; those gone by or fled out of sight go; the bombers meet craft.

def griffinwing.combat.Session.step_foes?(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.combat.Session.step_foes.after(
    griffinwing.combat.Session.step_foes.each(ga.sv(s, "foes", J.JA{[]}), ga.items(s, "foes"), griffinwing.Session.anchor(s), n),
    griffinwing.Session.anchor(s))

# w = {"s", "c": the foes stepped so far}

def griffinwing.combat.Session.step_foes.each?(+s: J.Val, +fs: List<&2, J.Val>, +anchor: U32, +n: J.Val) -> J.Val:
  griffinwing.combat.Session.step_foes.walk(fs, gw.pk(s, J.JA{[]}), anchor, n)

def griffinwing.combat.Session.step_foes.walk?(+fs: List<&2, J.Val>, +w: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  match fs:
    case Nil{}:
      w
    case Con{+f, +t}:
      griffinwing.combat.Session.step_foes.walk(t,
        griffinwing.combat.Session.step_foes.one(griffinwing.combat.Session.step_foe(gw.ps(w), f, anchor, n), gw.pc(w)),
        anchor, n)

def griffinwing.combat.Session.step_foes.one?(+r: J.Val, +done: J.Val) -> J.Val:
  gw.pk(gw.ps(r), J.push(done, gw.pc(r)))

def griffinwing.combat.Session.step_foes.after?(+w: J.Val, +anchor: U32) -> J.Val:
  griffinwing.combat.Session.bombers_ram(
    ga.sv(gw.ps(w), "foes", J.JA{griffinwing.combat.Session.step_foes.keep(J.items(gw.pc(w)), anchor)}))

def griffinwing.combat.Session.step_foes.keep?(+fs: List<&2, J.Val>, +anchor: U32) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      gw.ifjl(Bool.or(gw.isn(f, "kind", "Lamp"),
          Bool.and(U32.is_gt(ga.u(f, "hp"), 0),
            Bool.and(gw.gt(ga.u(f, "s"), (anchor - griffinwing.types.dm(300) : U32)), gw.lt(ga.u(f, "s"), (anchor + griffinwing.types.dm(1500) : U32))))),
        _ => Con{f, griffinwing.combat.Session.step_foes.keep(t, anchor)},
        _ => griffinwing.combat.Session.step_foes.keep(t, anchor))

# A foe moves on its velocity.

def gw.fmove?(+f: J.Val) -> J.Val:
  ga.si(ga.si(ga.si(f, "s", (ga.u(f, "s") + ga.u(f, "vs") : U32)), "x", (ga.u(f, "x") + ga.u(f, "vx") : U32)), "z", (ga.u(f, "z") + ga.u(f, "vz") : U32))

# The pair (kind, state) as the match arm that handles it, in the order the
# Rust match tries them: 0 Wisp|Kite Fly, 1 Wisp Tell, 2 Kite Hunt, 3 Kite
# Latched, 4 any Flee, 5 Turret Rest, 6 Turret Tell, 7 Bomber Hide, 8 Bomber
# Tell, 9 Bomber Dive, 10 none.

def griffinwing.combat.Session.step_foe.arm?(+f: J.Val) -> U32:
  griffinwing.combat.Session.step_foe.arm2(ga.s(f, "kind"), ga.s(f, "st"))

def griffinwing.combat.Session.step_foe.arm2?(+k: String, +st: String) -> U32:
  gw.ifu(Bool.and(Bool.or(String.eq(k, "Wisp"), String.eq(k, "Kite")), String.eq(st, "Fly")), _ => 0,
    _ => gw.ifu(Bool.and(String.eq(k, "Wisp"), String.eq(st, "Tell")), _ => 1,
      _ => gw.ifu(Bool.and(String.eq(k, "Kite"), String.eq(st, "Hunt")), _ => 2,
        _ => gw.ifu(Bool.and(String.eq(k, "Kite"), String.eq(st, "Latched")), _ => 3,
          _ => gw.ifu(String.eq(st, "Flee"), _ => 4,
            _ => gw.ifu(Bool.and(String.eq(k, "Turret"), String.eq(st, "Rest")), _ => 5,
              _ => gw.ifu(Bool.and(String.eq(k, "Turret"), String.eq(st, "Tell")), _ => 6,
                _ => gw.ifu(Bool.and(String.eq(k, "Bomber"), String.eq(st, "Hide")), _ => 7,
                  _ => gw.ifu(Bool.and(String.eq(k, "Bomber"), String.eq(st, "Tell")), _ => 8,
                    _ => gw.ifu(Bool.and(String.eq(k, "Bomber"), String.eq(st, "Dive")), _ => 9, _ => 10))))))))))

# One foe's tick. {"s", "c": the foe}.

def griffinwing.combat.Session.step_foe?(+s: J.Val, +f: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  gw.ifj(Bool.and(U32.is_ge(ga.u(f, "flash"), 4), Bool.or(gw.isn(f, "st", "Fly"), gw.isn(f, "st", "Hunt"))),
    _ => gw.pk(s, f),
    _ => griffinwing.combat.Session.step_foe.arm_do(s, f, anchor, n, griffinwing.combat.Session.step_foe.arm(f)))

def griffinwing.combat.Session.step_foe.arm_do?(+s: J.Val, +f: J.Val, +anchor: U32, +n: J.Val, +arm: U32) -> J.Val:
  gw.ifj(U32.is_eq(arm, 0), _ => griffinwing.combat.Session.step_foe.fly(s, gw.fmove(f), anchor, n),
    _ => gw.ifj(U32.is_eq(arm, 1), _ => griffinwing.combat.Session.step_foe.wisp_tell(s, gw.fmove(f)),
      _ => gw.ifj(U32.is_eq(arm, 2), _ => griffinwing.combat.Session.hunt(s, f),
        _ => gw.ifj(U32.is_eq(arm, 3), _ => griffinwing.combat.Session.hang(s, f),
          _ => gw.ifj(U32.is_eq(arm, 4), _ => gw.pk(s, griffinwing.combat.Session.step_foe.flee(gw.fmove(f))),
            _ => gw.ifj(U32.is_eq(arm, 5), _ => griffinwing.combat.Session.step_foe.turret_rest(s, gw.subs(f, "ticks", 1), anchor, n),
              _ => gw.ifj(U32.is_eq(arm, 6), _ => griffinwing.combat.Session.step_foe.turret_tell(s, gw.subs(f, "ticks", 1)),
                _ => gw.ifj(U32.is_eq(arm, 7), _ => griffinwing.combat.Session.step_foe.bomber_hide(s, f, anchor, n),
                  _ => gw.ifj(U32.is_eq(arm, 8), _ => griffinwing.combat.Session.step_foe.bomber_tell(s, f, anchor, n),
                    _ => gw.ifj(U32.is_eq(arm, 9), _ => gw.pk(s, gw.fmove(f)),
                      _ => gw.pk(s, f)))))))))))

# (Wisp | Kite, Fly), after the move: a weaving one turns, a shooting wisp
# counts down to a tell.

def griffinwing.combat.Session.step_foe.fly?(+s: J.Val, +f0: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  +f1 = gw.ifj(Bool.and(U32.is_eq(ga.u(f0, "mode"), 1), U32.is_zero(U32.mod((ga.u(s, "t") + ga.u(f0, "phase") : U32), 34))),
    _ => ga.si(f0, "vx", gw.neg(ga.u(f0, "vx"))),
    _ => f0)
  gw.ifj(Bool.and(U32.is_eq(ga.u(f1, "mode"), 3), gw.isn(f1, "kind", "Wisp")),
    _ => griffinwing.combat.Session.step_foe.shoot(s, gw.subs(f1, "ticks", 1), anchor, n),
    _ => gw.pk(s, f1))

# A shooting wisp counts down to its tell. `f` has had its tick taken off.

def griffinwing.combat.Session.step_foe.shoot?(+s: J.Val, +f: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  gw.ifj(Bool.and(U32.is_zero(ga.u(f, "ticks")), gw.within((ga.u(f, "s") - anchor : U32), griffinwing.types.dm(200), griffinwing.types.dm(460))),
    _ => griffinwing.combat.Session.step_foe.aim(s, f, griffinwing.combat.Session.target(s, ga.u(f, "id")), n),
    _ => gw.pk(s, f))

# A tell on the footprint `to` ([x, z] or null): the foe locks it.

def griffinwing.combat.Session.step_foe.aim?(+s: J.Val, +f: J.Val, +to: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(J.is_null(to),
    _ => gw.pk(s, f),
    _ => gw.pk(s, ga.su(gw.sets(ga.si(ga.si(f, "fx", ga.nu(to, 0)), "fz", ga.nu(to, 1)), "st", "Tell"), "ticks", ga.u(n, "tell"))))

# (Wisp, Tell), after the move: the spark leaves when the tell runs out.

def griffinwing.combat.Session.step_foe.wisp_tell?(+s: J.Val, +f0: J.Val) -> J.Val:
  griffinwing.combat.Session.step_foe.wisp_tell2(s, gw.subs(f0, "ticks", 1))

def griffinwing.combat.Session.step_foe.wisp_tell2?(+s: J.Val, +f: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(f, "ticks")),
    _ => griffinwing.combat.Session.step_foe.spark_shot(s, f,
      griffinwing.combat.Session.plane_shot(s, ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), ga.u(f, "fx"), ga.u(f, "fz"), griffinwing.types.dm(4))),
    _ => gw.pk(s, f))

def griffinwing.combat.Session.step_foe.spark_shot?(+s: J.Val, +f: J.Val, +v: GwV3) -> J.Val:
  gw.pk(
    griffinwing.flight.Session.push_bolt(s,
      griffinwing.types.Bolt.make("Spark", 0, ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), gw.v3a(v), gw.v3b(v), gw.v3c(v),
        griffinwing.combat.SPARK(), 220, 0, ga.u(f, "id"), 0, False{})),
    ga.su(gw.sets(f, "st", "Fly"), "ticks", 100))

# (_, Flee), after the move: gone when the ticks run out.

def griffinwing.combat.Session.step_foe.flee?(+f0: J.Val) -> J.Val:
  griffinwing.combat.Session.step_foe.flee2(gw.subs(f0, "ticks", 1))

def griffinwing.combat.Session.step_foe.flee2?(+f: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(f, "ticks")), _ => ga.su(f, "hp", 0), _ => f)

# (Turret, Rest), its tick taken off: a tower waiting for something in reach.

def griffinwing.combat.Session.step_foe.turret_rest?(+s: J.Val, +f: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  gw.ifj(Bool.and(U32.is_zero(ga.u(f, "ticks")), gw.within((ga.u(f, "s") - anchor : U32), griffinwing.types.dm(150), griffinwing.types.dm(560))),
    _ => griffinwing.combat.Session.step_foe.turret_aim(s, f, griffinwing.combat.Session.target(s, (ga.u(f, "id") + ga.u(f, "phase") : U32)), n),
    _ => gw.pk(s, f))

def griffinwing.combat.Session.step_foe.turret_aim?(+s: J.Val, +f: J.Val, +to: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(J.is_null(to),
    _ => gw.pk(s, f),
    _ => gw.pk(griffinwing.Session.sound(s, "vent_fire"),
      ga.su(gw.sets(ga.si(ga.si(f, "fx", ga.nu(to, 0)), "fz", ga.nu(to, 1)), "st", "Tell"), "ticks", ga.u(n, "tell"))))

# (Turret, Tell), its tick taken off: the shot goes.

def griffinwing.combat.Session.step_foe.turret_tell?(+s: J.Val, +f: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(f, "ticks")),
    _ => gw.pk(griffinwing.combat.Session.fire_marks(s, f),
      ga.su(gw.sets(ga.su(f, "phase", U32.and((ga.u(f, "phase") + 1 : U32), 255)), "st", "Rest"), "ticks", 26)),
    _ => gw.pk(s, f))

# (Bomber, Hide): behind its ridge until the formation is near.

def griffinwing.combat.Session.step_foe.bomber_hide?(+s: J.Val, +f: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  gw.ifj(gw.le((ga.u(f, "s") - anchor : U32), griffinwing.types.dm(330)),
    _ => griffinwing.combat.Session.step_foe.aim(s, f, griffinwing.combat.Session.target(s, ga.u(f, "id")), n),
    _ => gw.pk(s, f))

# (Bomber, Tell): it rises over the ridge, then dives down the locked lane.

def griffinwing.combat.Session.step_foe.bomber_tell?(+s: J.Val, +f0: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  griffinwing.combat.Session.step_foe.bomber_tell2(s,
    gw.subs(
      gw.ifj(gw.lt(ga.u(f0, "z"), (gw.max(ga.u(f0, "fz"), griffinwing.types.dm(10)) + griffinwing.types.dm(24) : U32)),
        _ => ga.si(f0, "z", (ga.u(f0, "z") + griffinwing.types.dm(1) : U32)),
        _ => f0),
      "ticks", 1),
    anchor)

def griffinwing.combat.Session.step_foe.bomber_tell2?(+s: J.Val, +f: J.Val, +anchor: U32) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(f, "ticks")),
    _ => gw.pk(s, griffinwing.combat.Session.step_foe.dive(f, gw.div(gw.max((ga.u(f, "s") - anchor : U32), griffinwing.types.dm(60)), 32))),
    _ => gw.pk(s, f))

# The dive down the locked lane: 32 ticks to it.

def griffinwing.combat.Session.step_foe.dive?(+f: J.Val, +closing: U32) -> J.Val:
  +f1 = ga.si(f, "vs", (griffinwing.types.PACE() - closing : U32))
  +f2 = ga.si(f1, "vx", gw.div((ga.u(f1, "fx") - ga.u(f1, "x") : U32), 32))
  +f3 = ga.si(f2, "vz", gw.div((ga.u(f2, "fz") - ga.u(f2, "z") : U32), 32))
  gw.sets(f3, "st", "Dive")

# The marks a turret's shot will land on, across and up: where it locked, and a
# pair more to each side for every three knights beyond the first. A J array of
# [x, z].

def griffinwing.combat.Session.marks?(+f: J.Val, +seats: U32) -> J.Val:
  J.JA{
    Con{ga.pair_i((ga.u(f, "fx") - griffinwing.types.dm(8) : U32), ga.u(f, "fz")),
    Con{ga.pair_i((ga.u(f, "fx") + griffinwing.types.dm(8) : U32), ga.u(f, "fz")),
      griffinwing.combat.Session.marks.extra(f, 1, U32.div(ga.ssub(seats, 1), 3))}}}

def griffinwing.combat.Session.marks.extra?(+f: J.Val, +k: U32, +extra: U32) -> List<&2, J.Val>:
  gw.ifjl(U32.is_gt(k, extra),
    _ => Nil{},
    _ => Con{ga.pair_i(((ga.u(f, "fx") - griffinwing.types.dm(8) : U32) - (k * griffinwing.types.dm(20) : U32) : U32), ga.u(f, "fz")),
      Con{ga.pair_i(((ga.u(f, "fx") + griffinwing.types.dm(8) : U32) + (k * griffinwing.types.dm(20) : U32) : U32), ga.u(f, "fz")),
        griffinwing.combat.Session.marks.extra(f, (k + 1 : U32), extra)}})

# A turret fires on its locked marks.

def griffinwing.combat.Session.fire_marks?(+s: J.Val, +f: J.Val) -> J.Val:
  griffinwing.Session.sound(
    griffinwing.combat.Session.fire_marks.go(s, f, J.items(griffinwing.combat.Session.marks(f, griffinwing.Session.pressure(s)))),
    "shot_crossbow")

def griffinwing.combat.Session.fire_marks.go?(+s: J.Val, +f: J.Val, +ms: List<&2, J.Val>) -> J.Val:
  match ms:
    case Nil{}:
      s
    case Con{+m, +t}:
      griffinwing.combat.Session.fire_marks.go(griffinwing.combat.Session.fire_marks.one(s, f, ga.nu(m, 0), ga.nu(m, 1)), f, t)

def griffinwing.combat.Session.fire_marks.one?(+s: J.Val, +f: J.Val, +mx: U32, +mz: U32) -> J.Val:
  griffinwing.combat.Session.fire_marks.shot(s, f,
    griffinwing.combat.Session.plane_shot(s, ga.u(f, "s"), ga.u(f, "x"), (ga.u(f, "z") + griffinwing.types.dm(12) : U32), mx, mz, griffinwing.types.dm(5)))

def griffinwing.combat.Session.fire_marks.shot?(+s: J.Val, +f: J.Val, +v: GwV3) -> J.Val:
  griffinwing.flight.Session.push_bolt(s,
    griffinwing.types.Bolt.make("Gloam", 0, ga.u(f, "s"), ga.u(f, "x"), (ga.u(f, "z") + griffinwing.types.dm(12) : U32),
      gw.v3a(v), gw.v3b(v), gw.v3c(v), griffinwing.combat.GLOAM(), 260, 0, ga.u(f, "id"), 0, False{}))

# A kite going for its victim: it picks one, closes, and latches onto the tail.

def griffinwing.combat.Session.hunt?(+s: J.Val, +f: J.Val) -> J.Val:
  gw.ifj(Bool.and(U32.is_ne(ga.u(f, "on"), 0), gw.at_ok_some(griffinwing.flight.Session.craft_at(s, ga.u(f, "on")))),
    _ => griffinwing.combat.Session.hunt.close(s, f),
    _ => griffinwing.combat.Session.hunt.pick(s, f, griffinwing.combat.Session.victim(s, ga.u(f, "id"))))

# `craft_at(id).is_some_and(|c| c.3)`.

def gw.at_ok_some?(+r: J.Val) -> Bool:
  Bool.and(gw.at_some(r), gw.at_ok(r))

def griffinwing.combat.Session.hunt.pick?(+s: J.Val, +f: J.Val, +v: J.Val) -> J.Val:
  gw.ifj(J.is_null(v),
    _ => gw.pk(s, ga.si(ga.su(gw.sets(f, "st", "Flee"), "ticks", 40), "vz", 60)),
    _ => griffinwing.combat.Session.hunt.close(s, ga.su(f, "on", J.u32(v))))

def griffinwing.combat.Session.hunt.close?(+s: J.Val, +f: J.Val) -> J.Val:
  griffinwing.combat.Session.hunt.at(s, f, griffinwing.flight.Session.craft_at(s, ga.u(f, "on")))

def griffinwing.combat.Session.hunt.at?(+s: J.Val, +f: J.Val, +at: J.Val) -> J.Val:
  gw.ifj(Bool.not(gw.at_some(at)),
    _ => gw.pk(s, f),
    _ => griffinwing.combat.Session.hunt.go(s, f,
      (gw.at_s(at) - griffinwing.combat.LATCH_BACK() : U32), gw.at_x(at), (gw.at_z(at) + griffinwing.types.dm(4) : U32)))

def griffinwing.combat.Session.hunt.go?(+s: J.Val, +f: J.Val, +ts: U32, +tx: U32, +tz: U32) -> J.Val:
  griffinwing.combat.Session.hunt.test(s, f, ts, tx, tz, (ts - ga.u(f, "s") : U32), (tx - ga.u(f, "x") : U32), (tz - ga.u(f, "z") : U32))

def griffinwing.combat.Session.hunt.test?(+s: J.Val, +f: J.Val, +ts: U32, +tx: U32, +tz: U32, +dx: U32, +dy: U32, +dz: U32) -> J.Val:
  gw.ifj(gw.nle(griffinwing.types.norm(dx, dy, dz), griffinwing.types.dm(9)),
    _ => gw.pk(
      griffinwing.Session.sound(griffinwing.Session.say(s, ga.u(f, "on"), "Latched"), "clang"),
      ga.si(ga.si(ga.si(ga.su(gw.sets(f, "st", "Latched"), "ticks", 0), "s", ts), "x", tx), "z", tz)),
    _ => gw.pk(s, griffinwing.combat.Session.hunt.lunge(f, griffinwing.types.scale3(dx, dy, dz, griffinwing.combat.HUNT_V()))))

def griffinwing.combat.Session.hunt.lunge?(+f: J.Val, +v: GwV3) -> J.Val:
  +f1 = ga.si(ga.si(ga.si(f, "vs", gw.v3a(v)), "vx", gw.v3b(v)), "vz", gw.v3c(v))
  ga.si(ga.si(ga.si(f1, "s", (ga.u(f, "s") + gw.v3a(v) : U32)), "x", (ga.u(f, "x") + gw.v3b(v) : U32)), "z", (ga.u(f, "z") + gw.v3c(v) : U32))

# A latched kite hangs on its victim's tail, draining its shield, until it is
# shot, or shaken off by a roll.

def griffinwing.combat.Session.hang?(+s: J.Val, +f: J.Val) -> J.Val:
  griffinwing.combat.Session.hang.at(s, f, griffinwing.flight.Session.craft_at(s, ga.u(f, "on")))

def griffinwing.combat.Session.hang.at?(+s: J.Val, +f: J.Val, +at: J.Val) -> J.Val:
  gw.ifj(Bool.not(gw.at_some(at)),
    _ => gw.pk(s, ga.su(gw.sets(f, "st", "Flee"), "ticks", 40)),
    _ => griffinwing.combat.Session.hang.roll(s, f, at,
      Bool.and(gw.within(ga.u(f, "on"), 1, 6),
        Bool.and(ga.has_key(J.get(s, "seats"), ga.u(f, "on")), U32.is_gt(ga.u(gw.craft_of(s, ga.u(f, "on")), "roll"), 0)))))

def griffinwing.combat.Session.hang.roll?(+s: J.Val, +f: J.Val, +at: J.Val, +rolling: Bool) -> J.Val:
  gw.ifj(Bool.or(Bool.not(gw.at_ok(at)), rolling),
    _ => gw.pk(s, ga.si(ga.su(ga.si(ga.si(gw.sets(ga.su(f, "ticks", 40), "st", "Flee"), "vs", 0), "vx",
        gw.ifu(gw.ge(gw.at_x(at), 0), _ => 90, _ => gw.neg(90))), "vz", 70), "ticks", 40)),
    _ => griffinwing.combat.Session.hang.drip(s,
      ga.su(ga.si(ga.si(ga.si(f, "s", (gw.at_s(at) - griffinwing.combat.LATCH_BACK() : U32)), "x", gw.at_x(at)), "z", (gw.at_z(at) + griffinwing.types.dm(4) : U32)), "ticks",
        U32.mod((ga.u(f, "ticks") + 1 : U32), 9996))))

def griffinwing.combat.Session.hang.drip?(+s: J.Val, +f: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(U32.mod(ga.u(f, "ticks"), 12)),
    _ => gw.pk(griffinwing.combat.Session.drain(s, ga.u(f, "on"), gw.ifu(U32.is_le(ga.u(f, "on"), 6), _ => 2, _ => 3)), f),
    _ => gw.pk(s, f))

# Dive-bombers meet craft: a blow for the knight, and the bomber is spent.

def griffinwing.combat.Session.bombers_ram?(+s: J.Val) -> J.Val:
  griffinwing.combat.Session.bombers_ram.spend(s,
    griffinwing.combat.Session.bombers_ram.scan(ga.items(s, "foes"), s, griffinwing.combat.Session.bombers_ram.crafts(s, ga.ids(J.get(s, "seats")))))

# (id, craft_s, x, z) of the knights flying.

def griffinwing.combat.Session.bombers_ram.crafts?(+s: J.Val, +ids: List<&2, U32>) -> List<&2, J.Val>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gw.ifjl(griffinwing.types.Craft.flying(gw.craft_of(s, id)),
        _ => Con{J.JA{[J.JU{id}, J.of_i32(griffinwing.flight.Session.craft_s(s, gw.craft_of(s, id))), J.of_i32(ga.u(gw.craft_of(s, id), "x")), J.of_i32(ga.u(gw.craft_of(s, id), "z"))]},
          griffinwing.combat.Session.bombers_ram.crafts(s, t)},
        _ => griffinwing.combat.Session.bombers_ram.crafts(s, t))

# The (foe id, craft id) pairs a diving bomber's swept path meets, one craft each.

def griffinwing.combat.Session.bombers_ram.scan?(+fs: List<&2, J.Val>, +s: J.Val, +crafts: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      gw.ifjl(Bool.and(gw.isn(f, "kind", "Bomber"), gw.isn(f, "st", "Dive")),
        _ => griffinwing.combat.Session.bombers_ram.hit(f, griffinwing.combat.Session.bombers_ram.meet(f, crafts), griffinwing.combat.Session.bombers_ram.scan(t, s, crafts)),
        _ => griffinwing.combat.Session.bombers_ram.scan(t, s, crafts))

def griffinwing.combat.Session.bombers_ram.hit?(+f: J.Val, +cid: J.Val, +rest: List<&2, J.Val>) -> List<&2, J.Val>:
  gw.ifjl(J.is_null(cid),
    _ => rest,
    _ => Con{J.JA{[J.JU{ga.u(f, "id")}, cid]}, rest})

# The first craft its path meets: its id, or null.

def griffinwing.combat.Session.bombers_ram.meet?(+f: J.Val, +crafts: List<&2, J.Val>) -> J.Val:
  match crafts:
    case Nil{}:
      J.JNull{}
    case Con{+c, +t}:
      gw.ifj(griffinwing.types.near(
          (ga.u(f, "s") - ga.u(f, "vs") : U32), (ga.u(f, "x") - ga.u(f, "vx") : U32), (ga.u(f, "z") - ga.u(f, "vz") : U32),
          ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"),
          ga.nu(c, 1), ga.nu(c, 2), ga.nu(c, 3),
          (griffinwing.types.Foe.radius(f) + griffinwing.flight.CRAFT_R() : U32)),
        _ => J.nth(c, 0),
        _ => griffinwing.combat.Session.bombers_ram.meet(f, t))

def griffinwing.combat.Session.bombers_ram.spend?(+s: J.Val, +pairs: List<&2, J.Val>) -> J.Val:
  match pairs:
    case Nil{}:
      s
    case Con{+p, +t}:
      griffinwing.combat.Session.bombers_ram.spend(
        griffinwing.combat.Session.bombers_ram.blow(s, ga.nu(p, 0), griffinwing.combat.Session.hurt_craft(s, ga.nu(p, 1), griffinwing.combat.RAM())),
        t)

# The blow landed: the bomber is spent, and pops. (The session is the one the
# blow was struck in.)

def griffinwing.combat.Session.bombers_ram.blow?(+s0: J.Val, +fid: U32, +r: J.Val) -> J.Val:
  gw.ifj(J.bool(gw.out_r(r)),
    _ => griffinwing.combat.Session.bombers_ram.pop(gw.out_s(r), fid, griffinwing.combat.Session.bombers_ram.at(ga.items(gw.out_s(r), "foes"), fid, 0)),
    _ => gw.out_s(r))

# The index of the foe with this id, or the length.

def griffinwing.combat.Session.bombers_ram.at?(+fs: List<&2, J.Val>, +id: U32, +k: U32) -> U32:
  match fs:
    case Nil{}:
      k
    case Con{+f, +t}:
      gw.ifu(U32.is_eq(ga.u(f, "id"), id), _ => k, _ => griffinwing.combat.Session.bombers_ram.at(t, id, (k + 1 : U32)))

def griffinwing.combat.Session.bombers_ram.pop?(+s: J.Val, +fid: U32, +i: U32) -> J.Val:
  gw.ifj(U32.is_lt(i, gw.len(s, "foes")),
    _ => griffinwing.Session.sound(
      griffinwing.Session.spark(gw.remove(s, "foes", i), "Pop", ga.u(gw.item(s, "foes", i), "s"), ga.u(gw.item(s, "foes", i), "x"), ga.u(gw.item(s, "foes", i), "z"), 0),
      "bomb_toss"),
    _ => s)

def griffinwing.combat.Session.hit_foe?(+s: J.Val, +fi: U32, +dmg: U32, +by: U32) -> J.Val:
  griffinwing.combat.Session.hit_foe.struck(
    gw.put_item(s, "foes", fi,
      ga.su(ga.su(gw.item(s, "foes", fi), "hp", ga.ssub(ga.u(gw.item(s, "foes", fi), "hp"), dmg)), "flash", 5)),
    fi, by)

def griffinwing.combat.Session.hit_foe.struck?(+s: J.Val, +fi: U32, +by: U32) -> J.Val:
  gw.ifj(U32.is_gt(ga.u(gw.item(s, "foes", fi), "hp"), 0),
    _ => gw.out(griffinwing.Session.sound(s, "hit_magic"), J.JBool{False{}}),
    _ => gw.out(
      gw.ifj(gw.isn(gw.item(s, "foes", fi), "kind", "Lamp"),
        _ => griffinwing.combat.Session.break_lamp(s, fi, by),
        _ => griffinwing.combat.Session.kill(s, fi, by)),
      J.JBool{True{}}))

# `seats.get_mut(&by)`'s craft changed by `f`, or the session at0 it is.

def gw.seat_has?(+s: J.Val, +by: U32) -> Bool:
  ga.has_key(J.get(s, "seats"), by)

# A foe pops into cold smoke and a spark of returned light. A kite that had
# latched on is a rescue; a knight's chain of pops is counted, and pays more
# the longer it runs.

def griffinwing.combat.Session.kill?(+s: J.Val, +fi: U32, +by: U32) -> J.Val:
  griffinwing.combat.Session.kill.a(gw.remove(s, "foes", fi), gw.item(s, "foes", fi), by)

def griffinwing.combat.Session.kill.a?(+s: J.Val, +f: J.Val, +by: U32) -> J.Val:
  gw.ifj(gw.isn(f, "st", "Latched"),
    _ => griffinwing.combat.Session.kill.b(
      gw.ifj(gw.seat_has(s, by),
        _ => gw.put_craft(
          griffinwing.Session.say(
            ga.sv(s, "beat", ga.su(gw.addu(J.get(s, "beat"), "kills", 1), "saved", RS.as_u16((ga.u(J.get(s, "beat"), "saved") + 1 : U32)))),
            ga.u(f, "on"), "Thanks"),
          by, ga.su(gw.craft_of(s, by), "saved", RS.as_u16((ga.u(gw.craft_of(s, by), "saved") + 1 : U32)))),
        _ => griffinwing.Session.say(
          ga.sv(s, "beat", ga.su(gw.addu(J.get(s, "beat"), "kills", 1), "saved", RS.as_u16((ga.u(J.get(s, "beat"), "saved") + 1 : U32)))),
          ga.u(f, "on"), "Thanks")),
      f, by, (griffinwing.types.Kind.worth(ga.s(f, "kind")) + 150 : U32)),
    _ => griffinwing.combat.Session.kill.b(
      ga.sv(s, "beat", gw.addu(J.get(s, "beat"), "kills", 1)),
      f, by, griffinwing.types.Kind.worth(ga.s(f, "kind"))))

# The base is banked at the worth, not at the bonus.

def griffinwing.combat.Session.kill.b?(+s: J.Val, +f: J.Val, +by: U32, +gain: U32) -> J.Val:
  griffinwing.combat.Session.kill.c(
    ga.sv(s, "beat", gw.addu(J.get(s, "beat"), "base", griffinwing.types.Kind.worth(ga.s(f, "kind")))),
    f, by, gain)

def griffinwing.combat.Session.kill.c?(+s: J.Val, +f: J.Val, +by: U32, +gain: U32) -> J.Val:
  gw.ifj(gw.seat_has(s, by),
    _ => griffinwing.combat.Session.kill.d(s, f, by, gain,
      ga.su(ga.su(ga.su(gw.addu(gw.craft_of(s, by), "chain", 1), "chain_ticks", 75), "hits", (ga.u(gw.craft_of(s, by), "hits") + 1 : U32)), "pts", ga.u(gw.craft_of(s, by), "pts"))),
    _ => griffinwing.combat.Session.kill.e(s, f, by, gain, 0))

def griffinwing.combat.Session.kill.d?(+s: J.Val, +f: J.Val, +by: U32, +gain: U32, +c: J.Val) -> J.Val:
  griffinwing.combat.Session.kill.d2(s, f, by, c, ga.u(c, "chain"),
    (gain + gw.ifu(U32.is_ge(ga.u(c, "chain"), 3), _ => (10 * (ga.u(c, "chain") - 2 : U32) : U32), _ => 0) : U32))

def griffinwing.combat.Session.kill.d2?(+s: J.Val, +f: J.Val, +by: U32, +c: J.Val, +chain: U32, +gain: U32) -> J.Val:
  griffinwing.combat.Session.kill.e(gw.put_craft(s, by, ga.su(c, "pts", (ga.u(c, "pts") + gain : U32))), f, by, gain, chain)

def griffinwing.combat.Session.kill.e?(+s: J.Val, +f: J.Val, +by: U32, +gain: U32, +chain: U32) -> J.Val:
  griffinwing.combat.Session.kill.fin(
    griffinwing.Session.sound(
      griffinwing.Session.spark(ga.sv(s, "beat", gw.addu(J.get(s, "beat"), "pts", gain)), "Pop", ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), U32.min(chain, 99)),
      "kill"),
    by, chain)

def griffinwing.combat.Session.kill.fin?(+s: J.Val, +by: U32, +chain: U32) -> J.Val:
  gw.ifj(U32.is_eq(chain, 5), _ => griffinwing.Session.say(s, by, "Chain"), _ => s)

# A lamp's flame is out: points, a flash, and the Wyrm reels (the Wyrm's tick
# sees it).

def griffinwing.combat.Session.break_lamp?(+s: J.Val, +fi: U32, +by: U32) -> J.Val:
  griffinwing.combat.Session.break_lamp.a(
    gw.put_item(s, "foes", fi, ga.su(gw.sets(gw.item(s, "foes", fi), "st", "Broken"), "hp", 0)),
    gw.item(s, "foes", fi), by)

def griffinwing.combat.Session.break_lamp.a?(+s: J.Val, +f: J.Val, +by: U32) -> J.Val:
  griffinwing.Session.sound(
    griffinwing.Session.spark(
      gw.ifj(gw.seat_has(s, by),
        _ => gw.put_craft(griffinwing.combat.Session.break_lamp.beat(s), by,
          ga.su(ga.su(gw.craft_of(s, by), "pts", (ga.u(gw.craft_of(s, by), "pts") + 500 : U32)), "hits", (ga.u(gw.craft_of(s, by), "hits") + 1 : U32))),
        _ => griffinwing.combat.Session.break_lamp.beat(s)),
      "Lamp", ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), 2),
    "boss_rage")

def griffinwing.combat.Session.break_lamp.beat?(+s: J.Val) -> J.Val:
  ga.sv(s, "beat", gw.addu(gw.addu(gw.addu(J.get(s, "beat"), "pts", 500), "base", 500), "kills", 1))

# Starfall: every hostile bolt is cleared, and everything small on the way is
# struck for what it can bear.

def griffinwing.combat.Session.starfall?(+s: J.Val, +by: U32, +s_c: U32, +x: U32, +z: U32) -> J.Val:
  griffinwing.combat.Session.starfall.sweep(
    ga.sv(
      griffinwing.Session.sound(griffinwing.Session.spark(s, "Starfall", s_c, 0, 0, by), "bomb"),
      "bolts", J.JA{griffinwing.combat.Session.starfall.clear(ga.items(s, "bolts"))}),
    0, by, griffinwing.Session.anchor(s))

# bolts.retain(|b| !matches!(b.shot, Spark | Gloam))

def griffinwing.combat.Session.starfall.clear?(+bs: List<&2, J.Val>) -> List<&2, J.Val>:
  match bs:
    case Nil{}:
      Nil{}
    case Con{+b, +t}:
      gw.ifjl(Bool.or(gw.isn(b, "shot", "Spark"), gw.isn(b, "shot", "Gloam")),
        _ => griffinwing.combat.Session.starfall.clear(t),
        _ => Con{b, griffinwing.combat.Session.starfall.clear(t)})

def griffinwing.combat.Session.starfall.sweep?(+s: J.Val, +i: U32, +by: U32, +anchor: U32) -> J.Val:
  gw.ifj(U32.is_lt(i, gw.len(s, "foes")),
    _ => griffinwing.combat.Session.starfall.foe(s, i, by, anchor, gw.item(s, "foes", i)),
    _ => s)

def griffinwing.combat.Session.starfall.foe?(+s: J.Val, +i: U32, +by: U32, +anchor: U32, +f: J.Val) -> J.Val:
  gw.ifj(Bool.and(gw.within((ga.u(f, "s") - anchor : U32), gw.neg(griffinwing.types.dm(50)), griffinwing.types.dm(600)),
      Bool.and(griffinwing.types.Foe.lockable(f), Bool.not(gw.isn(f, "kind", "Lamp")))),
    _ => griffinwing.combat.Session.starfall.hit(griffinwing.combat.Session.hit_foe(s, i, griffinwing.flight.STARFALL(), by), i, by, anchor),
    _ => griffinwing.combat.Session.starfall.sweep(s, (i + 1 : U32), by, anchor))

def griffinwing.combat.Session.starfall.hit?(+r: J.Val, +i: U32, +by: U32, +anchor: U32) -> J.Val:
  griffinwing.combat.Session.starfall.sweep(gw.out_s(r), gw.ifu(J.bool(gw.out_r(r)), _ => i, _ => (i + 1 : U32)), by, anchor)

# ─── the bolts' tick ────────────────────────────────────────────────────────

# The bolts fly and home, then land.

def griffinwing.combat.Session.step_bolts?(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.combat.Session.step_bolts.land(
    griffinwing.combat.Session.step_bolts.move(s, 0, gw.len(s, "bolts"), griffinwing.Session.anchor(s), J.JA{[]}, J.JA{[]}),
    n, griffinwing.Session.anchor(s))

# First pass: each bolt waits at its muzzle or homes and moves. The workspace
# {"s", "segs": null or [from, to] per bolt, "dead": bool per bolt}.

def griffinwing.combat.Session.step_bolts.move?(+s: J.Val, +i: U32, +count: U32, +anchor: U32, +segs: J.Val, +dead: J.Val) -> J.Val:
  gw.ifj(U32.is_lt(i, count),
    _ => griffinwing.combat.Session.step_bolts.move.next(s, i, count, anchor, segs, dead,
      griffinwing.combat.Session.step_bolts.one(s, gw.item(s, "bolts", i), anchor)),
    _ => J.set(J.set(J.set(J.obj(), "s", s), "segs", segs), "dead", dead))

def griffinwing.combat.Session.step_bolts.move.next?(+s: J.Val, +i: U32, +count: U32, +anchor: U32, +segs: J.Val, +dead: J.Val, +r: J.Val) -> J.Val:
  griffinwing.combat.Session.step_bolts.move(gw.put_item(s, "bolts", i, J.get(r, "b")), (i + 1 : U32), count, anchor, J.push(segs, J.get(r, "seg")), J.push(dead, J.get(r, "dead")))

def griffinwing.combat.Session.step_bolts.one?(+s: J.Val, +b: J.Val, +anchor: U32) -> J.Val:
  gw.ifj(U32.is_gt(ga.u(b, "delay"), 0),
    _ => griffinwing.combat.Session.step_bolts.wait(b, griffinwing.flight.Session.craft_at(s, ga.u(b, "owner"))),
    _ => griffinwing.combat.Session.step_bolts.fly(s, griffinwing.combat.Session.step_bolts.home(s, b)))

# {"b", "seg", "dead"}

def gw.bolt_out?(+b: J.Val, +seg: J.Val, +dead: Bool) -> J.Val:
  J.set(J.set(J.set(J.obj(), "b", b), "seg", seg), "dead", J.JBool{dead})

# A released starburst waits at its craft's muzzle for its turn.

def griffinwing.combat.Session.step_bolts.wait?(+b: J.Val, +at: J.Val) -> J.Val:
  gw.ifj(Bool.and(gw.at_some(at), gw.at_ok(at)),
    _ => gw.bolt_out(
      ga.su(ga.si(ga.si(ga.si(b, "s", (gw.at_s(at) + griffinwing.types.dm(12) : U32)), "x", gw.at_x(at)), "z", gw.at_z(at)), "delay", (ga.u(b, "delay") - 1 : U32)),
      J.JNull{}, False{}),
    _ => gw.bolt_out(b, J.JNull{}, True{}))

# Homing: a starburst and a turned bolt close on their body.

def griffinwing.combat.Session.step_bolts.home?(+s: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(Bool.and(U32.is_ne(ga.u(b, "aim"), 0), Bool.or(gw.isn(b, "shot", "Star"), gw.isn(b, "shot", "Turned"))),
    _ => griffinwing.combat.Session.step_bolts.home2(s, b, griffinwing.combat.Session.step_bolts.body(ga.items(s, "foes"), ga.u(b, "aim"))),
    _ => b)

# The live foe with this id: [s, x, z] or null.

def griffinwing.combat.Session.step_bolts.body?(+fs: List<&2, J.Val>, +id: U32) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      gw.ifj(Bool.and(U32.is_eq(ga.u(f, "id"), id), griffinwing.types.Foe.alive(f)),
        _ => J.JA{[J.of_i32(ga.u(f, "s")), J.of_i32(ga.u(f, "x")), J.of_i32(ga.u(f, "z"))]},
        _ => griffinwing.combat.Session.step_bolts.body(t, id))

def griffinwing.combat.Session.step_bolts.home2?(+s: J.Val, +b: J.Val, +p: J.Val) -> J.Val:
  gw.ifj(J.is_null(p),
    _ => ga.su(b, "aim", griffinwing.combat.Session.step_bolts.next_aim(ga.items(s, "foes"), b, J.JNull{}, 0)),
    _ => griffinwing.combat.Session.step_bolts.steer(b,
      griffinwing.types.scale3((ga.nu(p, 0) - ga.u(b, "s") : U32), (ga.nu(p, 1) - ga.u(b, "x") : U32), (ga.nu(p, 2) - ga.u(b, "z") : U32),
        gw.ifu(gw.isn(b, "shot", "Turned"), _ => griffinwing.combat.V_TURN(), _ => griffinwing.flight.Session.star_speed()))))

def griffinwing.combat.Session.step_bolts.steer?(+b: J.Val, +want: GwV3) -> J.Val:
  ga.si(ga.si(ga.si(b, "vs", (ga.u(b, "vs") + gw.div((gw.v3a(want) - ga.u(b, "vs") : U32), 3) : U32)),
    "vx", (ga.u(b, "vx") + gw.div((gw.v3b(want) - ga.u(b, "vx") : U32), 3) : U32)),
    "vz", (ga.u(b, "vz") + gw.div((gw.v3c(want) - ga.u(b, "vz") : U32), 3) : U32))

# Its body is gone: on to the nearest ahead (lockable, not a lamp, within
# dm(20)..=dm(500) ahead of the bolt); the first of equals wins. The foe id, or 0.

def griffinwing.combat.Session.step_bolts.next_aim?(+fs: List<&2, J.Val>, +b: J.Val, +best: J.Val, +bd: U32) -> U32:
  match fs:
    case Nil{}:
      gw.ifu(J.is_null(best), _ => 0, _ => J.u32(best))
    case Con{+f, +t}:
      gw.ifu(Bool.and(Bool.and(griffinwing.types.Foe.lockable(f), Bool.not(gw.isn(f, "kind", "Lamp"))),
          gw.within((ga.u(f, "s") - ga.u(b, "s") : U32), griffinwing.types.dm(20), griffinwing.types.dm(500))),
        _ => griffinwing.combat.Session.step_bolts.next_aim2(f, t, b, best, bd,
          griffinwing.types.norm((ga.u(f, "s") - ga.u(b, "s") : U32), (ga.u(f, "x") - ga.u(b, "x") : U32), (ga.u(f, "z") - ga.u(b, "z") : U32))),
        _ => griffinwing.combat.Session.step_bolts.next_aim(t, b, best, bd))

def griffinwing.combat.Session.step_bolts.next_aim2?(+f: J.Val, +t: List<&2, J.Val>, +b: J.Val, +best: J.Val, +bd: U32, +d: U32) -> U32:
  gw.ifu(Bool.or(J.is_null(best), U32.is_lt(d, bd)),
    _ => griffinwing.combat.Session.step_bolts.next_aim(t, b, J.JU{ga.u(f, "id")}, d),
    _ => griffinwing.combat.Session.step_bolts.next_aim(t, b, best, bd))

# The move: from, then on its velocity; a tick of life gone.

def griffinwing.combat.Session.step_bolts.fly?(+s: J.Val, +b0: J.Val) -> J.Val:
  griffinwing.combat.Session.step_bolts.fly2(b0,
    ga.su(ga.si(ga.si(ga.si(b0, "s", (ga.u(b0, "s") + ga.u(b0, "vs") : U32)), "x", (ga.u(b0, "x") + ga.u(b0, "vx") : U32)), "z", (ga.u(b0, "z") + ga.u(b0, "vz") : U32)),
      "life", ga.ssub(ga.u(b0, "life"), 1)))

def griffinwing.combat.Session.step_bolts.fly2?(+b0: J.Val, +b: J.Val) -> J.Val:
  gw.bolt_out(b,
    J.JA{[J.of_i32(ga.u(b0, "s")), J.of_i32(ga.u(b0, "x")), J.of_i32(ga.u(b0, "z")), J.of_i32(ga.u(b, "s")), J.of_i32(ga.u(b, "x")), J.of_i32(ga.u(b, "z"))]},
    False{})

# Second pass: each bolt lands on what its swept path meets.

def griffinwing.combat.Session.step_bolts.land?(+w: J.Val, +n: J.Val, +anchor: U32) -> J.Val:
  griffinwing.combat.Session.step_bolts.retain(
    griffinwing.combat.Session.step_bolts.hits(J.get(w, "s"), 0, gw.len(J.get(w, "s"), "bolts"), J.get(w, "segs"), J.get(w, "dead"), n),
    anchor)

# {"s", "dead"}

def griffinwing.combat.Session.step_bolts.hits?(+s: J.Val, +i: U32, +count: U32, +segs: J.Val, +dead: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(U32.is_lt(i, count),
    _ => griffinwing.combat.Session.step_bolts.hit(s, i, count, segs, dead, n, J.nth(segs, i)),
    _ => J.set(J.set(J.obj(), "s", s), "dead", dead))

def griffinwing.combat.Session.step_bolts.hit?(+s: J.Val, +i: U32, +count: U32, +segs: J.Val, +dead: J.Val, +n: J.Val, +seg: J.Val) -> J.Val:
  gw.ifj(Bool.or(J.is_null(seg), J.bool(J.nth(dead, i))),
    _ => griffinwing.combat.Session.step_bolts.hits(s, (i + 1 : U32), count, segs, dead, n),
    _ => griffinwing.combat.Session.step_bolts.hit2(s, i, count, segs, dead, n, seg, gw.item(s, "bolts", i)))

def griffinwing.combat.Session.step_bolts.hit2?(+s: J.Val, +i: U32, +count: U32, +segs: J.Val, +dead: J.Val, +n: J.Val, +seg: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(Bool.or(gw.isn(b, "shot", "Spark"), gw.isn(b, "shot", "Gloam")),
    _ => griffinwing.combat.Session.step_bolts.hit3(
      i, count, segs, dead, n,
      griffinwing.combat.Session.hostile_bolt(s, b, ga.nu(seg, 0), ga.nu(seg, 1), ga.nu(seg, 2), ga.nu(seg, 3), ga.nu(seg, 4), ga.nu(seg, 5), n)),
    _ => griffinwing.combat.Session.step_bolts.hit4(
      i, count, segs, n,
      griffinwing.combat.Session.friendly_bolt(s, b, ga.nu(seg, 0), ga.nu(seg, 1), ga.nu(seg, 2), ga.nu(seg, 3), ga.nu(seg, 4), ga.nu(seg, 5), dead)))

# A hostile bolt flew (or not): bolts[i] = its turned self.

def griffinwing.combat.Session.step_bolts.hit3?(+i: U32, +count: U32, +segs: J.Val, +dead: J.Val, +n: J.Val, +r: J.Val) -> J.Val:
  griffinwing.combat.Session.step_bolts.hits(
    gw.put_item(J.get(r, "s"), "bolts", i, J.get(r, "b")), (i + 1 : U32), count, segs,
    gw.ifj(J.bool(J.get(r, "r")), _ => dead, _ => J.set_nth(dead, i, J.JBool{True{}})), n)

def griffinwing.combat.Session.step_bolts.hit4?(+i: U32, +count: U32, +segs: J.Val, +n: J.Val, +r: J.Val) -> J.Val:
  griffinwing.combat.Session.step_bolts.hits(
    J.get(r, "s"), (i + 1 : U32), count, segs,
    gw.ifj(J.bool(J.get(r, "r")), _ => J.get(r, "dead"), _ => J.set_nth(J.get(r, "dead"), i, J.JBool{True{}})), n)

# bolts.retain(|b| !dead[k] && b.life > 0 && anchor - dm(80) <= b.s <= anchor + dm(1200))

def griffinwing.combat.Session.step_bolts.retain?(+w: J.Val, +anchor: U32) -> J.Val:
  ga.sv(J.get(w, "s"), "bolts", J.JA{griffinwing.combat.Session.step_bolts.keep(ga.items(J.get(w, "s"), "bolts"), ga.items(w, "dead"), anchor)})

def griffinwing.combat.Session.step_bolts.keep?(+bs: List<&2, J.Val>, +dead: List<&2, J.Val>, +anchor: U32) -> List<&2, J.Val>:
  match bs dead:
    case Nil{} _:
      Nil{}
    case Con{+b, +t} Nil{}:
      Nil{}
    case Con{+b, +t} Con{+d, +dt}:
      gw.ifjl(Bool.and(Bool.not(J.bool(d)),
          Bool.and(U32.is_gt(ga.u(b, "life"), 0),
            Bool.and(gw.ge(ga.u(b, "s"), (anchor - griffinwing.types.dm(80) : U32)), gw.le(ga.u(b, "s"), (anchor + griffinwing.types.dm(1200) : U32))))),
        _ => Con{b, griffinwing.combat.Session.step_bolts.keep(t, dt, anchor)},
        _ => griffinwing.combat.Session.step_bolts.keep(t, dt, anchor))

# A friendly bolt against the foes and the hostile sparks; whether it flies on.
# {"s", "r", "dead"}.

def griffinwing.combat.Session.friendly_bolt?(+s: J.Val, +b: J.Val, +fs: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +dead: J.Val) -> J.Val:
  griffinwing.combat.Session.friendly_bolt.land(s, b, fs, fx, fz, ts, tx, tz, dead,
    griffinwing.combat.Session.friendly_bolt.best(ga.items(s, "foes"), b, fs, fx, fz, ts, tx, tz, 0, J.JNull{}, 0))

# The foe the bolt lands on: its index, or null. `best` and `bd` carry the
# nearest so far (the first of equals wins).

def griffinwing.combat.Session.friendly_bolt.best?(+fs: List<&2, J.Val>, +b: J.Val, +fs0: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +k: U32, +best: J.Val, +bd: U32) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      gw.ifj(Bool.and(griffinwing.types.Foe.alive(f),
          Bool.and(Bool.not(gw.isn(f, "st", "Hide")),
            Bool.not(Bool.and(gw.isn(b, "shot", "Star"), Bool.and(U32.is_ne(ga.u(b, "aim"), 0), U32.is_ne(ga.u(f, "id"), ga.u(b, "aim"))))))),
        _ => griffinwing.combat.Session.friendly_bolt.best2(f, t, b, fs0, fx, fz, ts, tx, tz, k, best, bd),
        _ => griffinwing.combat.Session.friendly_bolt.best(t, b, fs0, fx, fz, ts, tx, tz, (k + 1 : U32), best, bd))

def griffinwing.combat.Session.friendly_bolt.best2?(+f: J.Val, +t: List<&2, J.Val>, +b: J.Val, +fs0: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +k: U32, +best: J.Val, +bd: U32) -> J.Val:
  gw.ifj(griffinwing.types.near(fs0, fx, fz, ts, tx, tz, ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), (griffinwing.types.Foe.radius(f) + griffinwing.combat.BOLT_R() : U32)),
    _ => griffinwing.combat.Session.friendly_bolt.best3(f, t, b, fs0, fx, fz, ts, tx, tz, k, best, bd,
      griffinwing.types.norm((ga.u(f, "s") - fs0 : U32), (ga.u(f, "x") - fx : U32), (ga.u(f, "z") - fz : U32))),
    _ => griffinwing.combat.Session.friendly_bolt.best(t, b, fs0, fx, fz, ts, tx, tz, (k + 1 : U32), best, bd))

def griffinwing.combat.Session.friendly_bolt.best3?(+f: J.Val, +t: List<&2, J.Val>, +b: J.Val, +fs0: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +k: U32, +best: J.Val, +bd: U32, +d: U32) -> J.Val:
  gw.ifj(Bool.or(J.is_null(best), U32.is_lt(d, bd)),
    _ => griffinwing.combat.Session.friendly_bolt.best(t, b, fs0, fx, fz, ts, tx, tz, (k + 1 : U32), J.JU{k}, d),
    _ => griffinwing.combat.Session.friendly_bolt.best(t, b, fs0, fx, fz, ts, tx, tz, (k + 1 : U32), best, bd))

def griffinwing.combat.Session.friendly_bolt.land?(+s: J.Val, +b: J.Val, +fs: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +dead: J.Val, +best: J.Val) -> J.Val:
  gw.ifj(Bool.not(J.is_null(best)),
    _ => J.set(J.set(J.set(J.obj(), "s", griffinwing.combat.Session.land(s, J.u32(best), b)), "r", J.JBool{False{}}), "dead", dead),
    _ => griffinwing.combat.Session.friendly_bolt.spark(s, dead,
      griffinwing.combat.Session.friendly_bolt.shoot(ga.items(s, "bolts"), dead, fs, fx, fz, ts, tx, tz, 0)))

# A bolt shoots a snuffer's spark out of the air: the first live Spark the path
# meets, its index or null.

def griffinwing.combat.Session.friendly_bolt.shoot?(+bs: List<&2, J.Val>, +dead: J.Val, +fs: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +j: U32) -> J.Val:
  match bs:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      gw.ifj(Bool.and(Bool.not(J.bool(J.nth(dead, j))),
          Bool.and(gw.isn(h, "shot", "Spark"),
            griffinwing.types.near(fs, fx, fz, ts, tx, tz, ga.u(h, "s"), ga.u(h, "x"), ga.u(h, "z"), (griffinwing.combat.SPARK_R() + griffinwing.combat.BOLT_R() : U32)))),
        _ => J.JU{j},
        _ => griffinwing.combat.Session.friendly_bolt.shoot(t, dead, fs, fx, fz, ts, tx, tz, (j + 1 : U32)))

def griffinwing.combat.Session.friendly_bolt.spark?(+s: J.Val, +dead: J.Val, +hit: J.Val) -> J.Val:
  gw.ifj(J.is_null(hit),
    _ => J.set(J.set(J.set(J.obj(), "s", s), "r", J.JBool{True{}}), "dead", dead),
    _ => J.set(J.set(J.set(J.obj(), "s",
        griffinwing.Session.spark(s, "Hit", ga.u(gw.item(s, "bolts", J.u32(hit)), "s"), ga.u(gw.item(s, "bolts", J.u32(hit)), "x"), ga.u(gw.item(s, "bolts", J.u32(hit)), "z"), 0)),
      "r", J.JBool{False{}}),
      "dead", J.set_nth(dead, J.u32(hit), J.JBool{True{}})))

# A friendly bolt lands on foe `i`.

def griffinwing.combat.Session.land?(+s: J.Val, +i: U32, +b: J.Val) -> J.Val:
  gw.ifj(gw.isn(gw.item(s, "foes", i), "kind", "Lamp"),
    _ => griffinwing.combat.Session.land.lamp(s, i, b,
      Bool.and(Bool.not(J.is_null(J.get(s, "boss"))),
        U32.is_eq(J.u32(J.nth(J.get(J.get(s, "boss"), "lamps"), U32.min(ga.u(J.get(s, "boss"), "phase"), 2))), ga.u(gw.item(s, "foes", i), "id")))),
    _ => griffinwing.combat.Session.land.small(s, i, b))

def griffinwing.combat.Session.land.small?(+s: J.Val, +i: U32, +b: J.Val) -> J.Val:
  gw.out_s(griffinwing.combat.Session.hit_foe(
    griffinwing.Session.spark(s, "Hit", ga.u(gw.item(s, "foes", i), "s"), ga.u(gw.item(s, "foes", i), "x"), ga.u(gw.item(s, "foes", i), "z"), 0),
    i, gw.ifu(gw.isn(b, "shot", "Turned"), _ => griffinwing.combat.TURNED(), _ => ga.u(b, "dmg")), ga.u(b, "owner")))

def griffinwing.combat.Session.land.lamp?(+s: J.Val, +i: U32, +b: J.Val, +active: Bool) -> J.Val:
  griffinwing.combat.Session.land.lamp2(s, i, b, active, gw.item(s, "foes", i))

def griffinwing.combat.Session.land.lamp2?(+s: J.Val, +i: U32, +b: J.Val, +active: Bool, +f: J.Val) -> J.Val:
  gw.ifj(Bool.and(Bool.and(gw.isn(b, "shot", "Turned"), gw.isn(f, "st", "Shut")), active),
    _ => griffinwing.Session.say(
      griffinwing.Session.sound(
        griffinwing.Session.spark(gw.put_item(s, "foes", i, ga.su(gw.sets(f, "st", "Open"), "ticks", griffinwing.boss.OPEN())), "Lamp", ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), 1),
        "shield_up"),
      ga.u(b, "owner"), "Open"),
    _ => gw.ifj(Bool.and(gw.isn(b, "shot", "Turned"), gw.isn(f, "st", "Open")),
      _ => gw.put_item(s, "foes", i, ga.su(f, "ticks", griffinwing.boss.OPEN())),
      _ => gw.ifj(Bool.and(Bool.and(gw.isn(b, "shot", "Star"), gw.isn(f, "st", "Open")), Bool.and(ga.b(b, "full"), active)),
        _ => griffinwing.Session.spark(gw.out_s(griffinwing.combat.Session.hit_foe(s, i, ga.u(b, "dmg"), ga.u(b, "owner"))), "Hit", ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), 1),
        _ => griffinwing.Session.sound(
          griffinwing.Session.spark(gw.put_item(s, "foes", i, ga.su(f, "flash", 4)), "Hit", ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), 0),
          "clang"))))

# A hostile bolt against the knights: a roll's shield turns it round, a bare
# craft takes it. {"s", "b", "r": whether it flies on}.

def griffinwing.combat.Session.hostile_bolt?(+s: J.Val, +b: J.Val, +fs: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +n: J.Val) -> J.Val:
  griffinwing.combat.Session.hostile_bolt.each(s, b, fs, fx, fz, ts, tx, tz, n,
    gw.ifu(gw.isn(b, "shot", "Gloam"), _ => griffinwing.combat.GLOAM_R(), _ => griffinwing.combat.SPARK_R()),
    griffinwing.combat.Session.hostile_bolt.crafts(s, ga.ids(J.get(s, "seats")), griffinwing.Nums.reflect(n)))

# (id, craft_s, x, z, reflecting, hurt > 0) of each knight flying.

def griffinwing.combat.Session.hostile_bolt.crafts?(+s: J.Val, +ids: List<&2, U32>, +reflect: U32) -> List<&2, J.Val>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gw.ifjl(griffinwing.types.Craft.flying(gw.craft_of(s, id)),
        _ => Con{J.JA{[J.JU{id}, J.of_i32(griffinwing.flight.Session.craft_s(s, gw.craft_of(s, id))), J.of_i32(ga.u(gw.craft_of(s, id), "x")), J.of_i32(ga.u(gw.craft_of(s, id), "z")),
            J.JBool{griffinwing.flight.Craft.reflecting(gw.craft_of(s, id), reflect)}, J.JBool{U32.is_gt(ga.u(gw.craft_of(s, id), "hurt"), 0)}]},
          griffinwing.combat.Session.hostile_bolt.crafts(s, t, reflect)},
        _ => griffinwing.combat.Session.hostile_bolt.crafts(s, t, reflect))

def griffinwing.combat.Session.hostile_bolt.each?(+s: J.Val, +b: J.Val, +fs: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +n: J.Val, +r: U32, +crafts: List<&2, J.Val>) -> J.Val:
  match crafts:
    case Nil{}:
      J.set(J.set(J.set(J.obj(), "s", s), "b", b), "r", J.JBool{True{}})
    case Con{+c, +t}:
      gw.ifj(Bool.and(J.bool(J.nth(c, 4)), griffinwing.types.near(fs, fx, fz, ts, tx, tz, ga.nu(c, 1), ga.nu(c, 2), ga.nu(c, 3), (griffinwing.flight.REFLECT_R() + r : U32))),
        _ => griffinwing.combat.Session.turn(s, b, ga.nu(c, 0), ga.nu(c, 1), ga.nu(c, 2), ga.nu(c, 3)),
        _ => gw.ifj(Bool.and(Bool.not(J.bool(J.nth(c, 5))), griffinwing.types.near(fs, fx, fz, ts, tx, tz, ga.nu(c, 1), ga.nu(c, 2), ga.nu(c, 3), (griffinwing.flight.CRAFT_R() + r : U32))),
          _ => J.set(J.set(J.set(J.obj(), "s",
              griffinwing.Session.spark(gw.out_s(griffinwing.combat.Session.hurt_craft(s, ga.nu(c, 0), ga.u(b, "dmg"))), "Hit", ga.u(b, "s"), ga.u(b, "x"), ga.u(b, "z"), 0)),
            "b", b), "r", J.JBool{False{}}),
          _ => griffinwing.combat.Session.hostile_bolt.each(s, b, fs, fx, fz, ts, tx, tz, n, r, t)))

# A rolling craft's shield turns a bolt round: it is the roller's now, faster,
# and heads for what fired it (or the Wyrm's lit lamp). {"s", "b", "r": true}.

def griffinwing.combat.Session.turn?(+s: J.Val, +b: J.Val, +by: U32, +at0: U32, +ax: U32, +az: U32) -> J.Val:
  griffinwing.combat.Session.turn.go(s, b, by, at0,
    griffinwing.combat.Session.turn.target(s, b, at0, ax, az))

# The id the bolt heads for, or null: what fired it (if alive), else the lit
# lamp (if alive), else the nearest lockable foe ahead that is not a lamp.

def griffinwing.combat.Session.turn.target?(+s: J.Val, +b: J.Val, +at0: U32, +ax: U32, +az: U32) -> J.Val:
  griffinwing.combat.Session.turn.target2(s, b, at0, ax, az, griffinwing.combat.Session.turn.alive(ga.items(s, "foes"), ga.u(b, "src")))

def griffinwing.combat.Session.turn.target2?(+s: J.Val, +b: J.Val, +at0: U32, +ax: U32, +az: U32, +by_src: J.Val) -> J.Val:
  gw.ifj(Bool.not(J.is_null(by_src)),
    _ => by_src,
    _ => griffinwing.combat.Session.turn.target3(s, at0, ax, az,
      gw.ifj(J.is_null(J.get(s, "boss")),
        _ => J.JNull{},
        _ => griffinwing.combat.Session.turn.alive(ga.items(s, "foes"), J.u32(J.nth(J.get(J.get(s, "boss"), "lamps"), U32.min(ga.u(J.get(s, "boss"), "phase"), 2)))))))

def griffinwing.combat.Session.turn.target3?(+s: J.Val, +at0: U32, +ax: U32, +az: U32, +lamp: J.Val) -> J.Val:
  gw.ifj(Bool.not(J.is_null(lamp)),
    _ => lamp,
    _ => griffinwing.combat.Session.turn.nearest(ga.items(s, "foes"), at0, ax, az, J.JNull{}, 0))

# The id of the foe with this id if it is alive, else null.

def griffinwing.combat.Session.turn.alive?(+fs: List<&2, J.Val>, +id: U32) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      gw.ifj(Bool.and(U32.is_eq(ga.u(f, "id"), id), griffinwing.types.Foe.alive(f)),
        _ => J.JU{id},
        _ => griffinwing.combat.Session.turn.alive(t, id))

def griffinwing.combat.Session.turn.nearest?(+fs: List<&2, J.Val>, +at0: U32, +ax: U32, +az: U32, +best: J.Val, +bd: U32) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      gw.ifj(Bool.and(Bool.and(griffinwing.types.Foe.lockable(f), Bool.not(gw.isn(f, "kind", "Lamp"))), gw.gt(ga.u(f, "s"), at0)),
        _ => griffinwing.combat.Session.turn.nearest2(f, t, at0, ax, az, best, bd,
          griffinwing.types.norm((ga.u(f, "s") - at0 : U32), (ga.u(f, "x") - ax : U32), (ga.u(f, "z") - az : U32))),
        _ => griffinwing.combat.Session.turn.nearest(t, at0, ax, az, best, bd))

def griffinwing.combat.Session.turn.nearest2?(+f: J.Val, +t: List<&2, J.Val>, +at0: U32, +ax: U32, +az: U32, +best: J.Val, +bd: U32, +d: U32) -> J.Val:
  gw.ifj(Bool.or(J.is_null(best), U32.is_lt(d, bd)),
    _ => griffinwing.combat.Session.turn.nearest(t, at0, ax, az, J.JU{ga.u(f, "id")}, d),
    _ => griffinwing.combat.Session.turn.nearest(t, at0, ax, az, best, bd))

# The foe with this id (any state), or null.

def griffinwing.combat.Session.turn.find?(+fs: List<&2, J.Val>, +id: U32) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      gw.ifj(U32.is_eq(ga.u(f, "id"), id), _ => f, _ => griffinwing.combat.Session.turn.find(t, id))

def griffinwing.combat.Session.turn.go?(+s: J.Val, +b0: J.Val, +by: U32, +at0: U32, +target: J.Val) -> J.Val:
  griffinwing.combat.Session.turn.done(s, by,
    griffinwing.combat.Session.turn.aimed(
      ga.su(ga.su(ga.su(gw.sets(ga.su(b0, "owner", U32.and(by, 255)), "shot", "Turned"), "dmg", griffinwing.combat.TURNED()), "life", 90), "aim", gw.ifu(J.is_null(target), _ => 0, _ => J.u32(target))),
      gw.ifj(J.is_null(target), _ => J.JNull{}, _ => griffinwing.combat.Session.turn.find(ga.items(s, "foes"), J.u32(target)))))

def griffinwing.combat.Session.turn.aimed?(+b: J.Val, +f: J.Val) -> J.Val:
  gw.ifj(J.is_null(f),
    _ => ga.si(ga.si(ga.si(b, "vs", griffinwing.combat.V_TURN()), "vx", 0), "vz", 0),
    _ => griffinwing.combat.Session.turn.vel(b, griffinwing.types.scale3((ga.u(f, "s") - ga.u(b, "s") : U32), (ga.u(f, "x") - ga.u(b, "x") : U32), (ga.u(f, "z") - ga.u(b, "z") : U32), griffinwing.combat.V_TURN())))

def griffinwing.combat.Session.turn.vel?(+b: J.Val, +v: GwV3) -> J.Val:
  ga.si(ga.si(ga.si(b, "vs", gw.v3a(v)), "vx", gw.v3b(v)), "vz", gw.v3c(v))

# The roller scores; the beat too; a flash and a sound.

def griffinwing.combat.Session.turn.done?(+s: J.Val, +by: U32, +b: J.Val) -> J.Val:
  J.set(J.set(J.set(J.obj(), "s",
    griffinwing.Session.sound(
      griffinwing.Session.spark(
        ga.sv(
          gw.ifj(gw.seat_has(s, by),
            _ => gw.put_craft(s, by, ga.su(gw.craft_of(s, by), "pts", (ga.u(gw.craft_of(s, by), "pts") + 25 : U32))),
            _ => s),
          "beat", gw.addu(J.get(s, "beat"), "pts", 25)),
        "Turn", ga.u(b, "s"), ga.u(b, "x"), ga.u(b, "z"), U32.and(by, 255)),
      "shield_block")),
    "b", b), "r", J.JBool{True{}})

def gw.pk?(+s: J.Val, +c: J.Val) -> J.Val:
  J.set(J.set(J.obj(), "s", s), "c", c)

def gw.ps?(+w: J.Val) -> J.Val:
  J.get(w, "s")

def gw.pc?(+w: J.Val) -> J.Val:
  J.get(w, "c")

def griffinwing.flight.K_FIRE?() -> U32:
  1

def griffinwing.flight.K_DASH?() -> U32:
  2

def griffinwing.flight.K_BOMB?() -> U32:
  4

def griffinwing.flight.K_SPUR?() -> U32:
  8

def griffinwing.flight.bits?(+i: J.Val) -> U32:
  U32.or(
    U32.or(Bool.to_u32(ga.b(i, "fire")), U32.shln(Bool.to_u32(ga.b(i, "dash")), 1n)),
    U32.or(U32.shln(Bool.to_u32(ga.b(i, "bomb")), 2n), U32.or(U32.shln(Bool.to_u32(ga.b(i, "interact")), 3n), U32.shln(Bool.to_u32(ga.b(i, "swing")), 4n))))

# A seat's place in the formation: across and up, tenths of a unit.

def griffinwing.flight.slot.x?(+id: U32) -> U32:
  Bool.pick(U32, U32.is_eq(id, 1), 0,
    Bool.pick(U32, U32.is_eq(id, 2), gw.neg(griffinwing.types.dm(22)),
      Bool.pick(U32, U32.is_eq(id, 3), griffinwing.types.dm(22),
        Bool.pick(U32, U32.is_eq(id, 4), gw.neg(griffinwing.types.dm(40)),
          Bool.pick(U32, U32.is_eq(id, 5), griffinwing.types.dm(40), 0)))))

def griffinwing.flight.slot.z?(+id: U32) -> U32:
  Bool.pick(U32, U32.is_eq(id, 1), 0,
    Bool.pick(U32, Bool.or(U32.is_eq(id, 2), U32.is_eq(id, 3)), gw.neg(griffinwing.types.dm(4)),
      Bool.pick(U32, Bool.or(U32.is_eq(id, 4), U32.is_eq(id, 5)), gw.neg(griffinwing.types.dm(8)), griffinwing.types.dm(12))))

def griffinwing.flight.slot?(+id: U32) -> J.Val:
  ga.pair_i(griffinwing.flight.slot.x(id), griffinwing.flight.slot.z(id))

# A rider's place: across, up and ahead of the knights.

def griffinwing.flight.rider_slot.x?(+k: U32) -> U32:
  Bool.pick(U32, U32.is_eq(k, 0), gw.neg(griffinwing.types.dm(30)),
    Bool.pick(U32, U32.is_eq(k, 1), griffinwing.types.dm(30),
      Bool.pick(U32, U32.is_eq(k, 2), 0, gw.neg(griffinwing.types.dm(62)))))

def griffinwing.flight.rider_slot.z?(+k: U32) -> U32:
  Bool.pick(U32, Bool.or(U32.is_eq(k, 0), U32.is_eq(k, 1)), griffinwing.types.dm(8),
    Bool.pick(U32, U32.is_eq(k, 2), griffinwing.types.dm(18), griffinwing.types.dm(2)))

def griffinwing.flight.rider_slot.back?(+k: U32) -> U32:
  Bool.pick(U32, Bool.or(U32.is_eq(k, 0), U32.is_eq(k, 1)), griffinwing.types.dm(60),
    Bool.pick(U32, U32.is_eq(k, 2), griffinwing.types.dm(78), griffinwing.types.dm(50)))

def griffinwing.flight.rider_slot?(+k: U32) -> J.Val:
  ga.trio(J.of_i32(griffinwing.flight.rider_slot.x(k)), J.of_i32(griffinwing.flight.rider_slot.z(k)), J.of_i32(griffinwing.flight.rider_slot.back(k)))

# How fast a craft steers, across and up, and how quickly it gets there.

def griffinwing.flight.V_X?() -> U32:
  62

def griffinwing.flight.V_Z?() -> U32:
  50

def griffinwing.flight.ACC?() -> U32:
  14

def griffinwing.flight.RET_V?() -> U32:
  100

def griffinwing.flight.RX_MAX?() -> U32:
  griffinwing.types.dm(120)

def griffinwing.flight.RZ_MAX?() -> U32:
  griffinwing.types.dm(80)

def griffinwing.flight.TAP_REST?() -> U32:
  6

def griffinwing.flight.HOLD_AT?() -> U32:
  7

def griffinwing.flight.REL_REST?() -> U32:
  12

def griffinwing.flight.LOCK_R?() -> U32:
  griffinwing.types.dm(16)

def griffinwing.flight.V_BOLT?() -> U32:
  griffinwing.types.dm(22)

def griffinwing.flight.V_STAR?() -> U32:
  griffinwing.types.dm(26)

def griffinwing.flight.BOLT_LIFE?() -> U32:
  26

def griffinwing.flight.STAR_LIFE?() -> U32:
  46

def griffinwing.flight.LANCE?() -> U32:
  10

def griffinwing.flight.STAR_MIN?() -> U32:
  16

def griffinwing.flight.STAR_MAX?() -> U32:
  36

def griffinwing.flight.BOOST_BACK?() -> U32:
  griffinwing.types.dm(55)

def griffinwing.flight.BRAKE_BACK?() -> U32:
  griffinwing.types.dm(45)

def griffinwing.flight.BACK_V?() -> U32:
  28

def griffinwing.flight.ROLL_REST?() -> U32:
  12

def griffinwing.flight.ROLL_V?() -> U32:
  84

def griffinwing.flight.TAP_WINDOW?() -> U32:
  9

# The craft's body, for the bolts that hit it.

def griffinwing.flight.CRAFT_R?() -> U32:
  griffinwing.types.dm(10)

# How far from a craft a rolling shield turns a bolt round.

def griffinwing.flight.REFLECT_R?() -> U32:
  griffinwing.types.dm(26)

# What Starfall does to every foe it reaches.

def griffinwing.flight.STARFALL?() -> U32:
  120

# The bolts' home pace (starbursts steer at it).

def griffinwing.flight.Session.star_speed?() -> U32:
  griffinwing.flight.V_STAR()

# ─── where a craft is ───────────────────────────────────────────────────────

# The rail point a craft is at.

def griffinwing.flight.Session.craft_s?(+s: J.Val, +c: J.Val) -> U32:
  (griffinwing.Session.anchor(s) + ga.u(c, "back") : U32)

# A craft by the id a bolt or a foe holds: a seat's, a rider's. Its place along,
# across and up, and whether it is flying. null: none by that id.

def griffinwing.flight.Session.craft_at?(+s: J.Val, +id: U32) -> J.Val:
  gw.ifj(gw.within(id, 1, 6),
    _ => griffinwing.flight.Session.craft_at.seat(s, J.get(J.get(s, "seats"), U32.show(id))),
    _ => griffinwing.flight.Session.craft_at.rider(s, griffinwing.flight.Session.wing_find(ga.items(s, "wing"), id)))

def griffinwing.flight.Session.craft_at.seat?(+s: J.Val, +seat: J.Val) -> J.Val:
  gw.ifj(J.is_null(seat),
    _ => J.JNull{},
    _ => griffinwing.flight.Session.craft_at.of(s, J.get(seat, "craft")))

def griffinwing.flight.Session.craft_at.of?(+s: J.Val, +c: J.Val) -> J.Val:
  J.JA{[J.of_i32(griffinwing.flight.Session.craft_s(s, c)), J.of_i32(ga.u(c, "x")), J.of_i32(ga.u(c, "z")), J.JBool{griffinwing.types.Craft.flying(c)}]}

def griffinwing.flight.Session.craft_at.rider?(+s: J.Val, +w: J.Val) -> J.Val:
  gw.ifj(J.is_null(w),
    _ => J.JNull{},
    _ => J.JA{[J.of_i32(griffinwing.flight.Session.craft_s(s, J.get(w, "craft"))), J.of_i32(ga.u(J.get(w, "craft"), "x")), J.of_i32(ga.u(J.get(w, "craft"), "z")),
      J.JBool{Bool.and(Bool.not(ga.b(w, "lost")), U32.is_zero(ga.u(J.get(w, "craft"), "down")))}]})

# `wing.iter().find(|w| w.id == id)`: the rider, or null.

def griffinwing.flight.Session.wing_find?(+ws: List<&2, J.Val>, +id: U32) -> J.Val:
  match ws:
    case Nil{}:
      J.JNull{}
    case Con{+w, +t}:
      gw.ifj(U32.is_eq(ga.u(w, "id"), id), _ => w, _ => griffinwing.flight.Session.wing_find(t, id))

# The parts of a `craft_at` answer.

def gw.at_some?(+r: J.Val) -> Bool:
  Bool.not(J.is_null(r))

def gw.at_s?(+r: J.Val) -> U32:
  ga.nu(r, 0)

def gw.at_x?(+r: J.Val) -> U32:
  ga.nu(r, 1)

def gw.at_z?(+r: J.Val) -> U32:
  ga.nu(r, 2)

def gw.at_ok?(+r: J.Val) -> Bool:
  J.bool(J.nth(r, 3))

# ─── one knight's tick in the air ───────────────────────────────────────────

def gw.put_craft?(+s: J.Val, +id: U32, +c: J.Val) -> J.Val:
  ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id, ga.sv(ga.at(J.get(s, "seats"), id), "craft", c)))

def gw.craft_of?(+s: J.Val, +id: U32) -> J.Val:
  J.get(ga.at(J.get(s, "seats"), id), "craft")

# Whether a key bit went down this tick: `on && old & key == 0`.

def gw.edge?(+old: U32, +key: U32, +on: Bool) -> Bool:
  Bool.and(on, U32.is_zero(U32.and(old, key)))

def griffinwing.flight.Session.fly?(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(Bool.not(ga.has_key(J.get(s, "seats"), id)),
    _ => s,
    _ => griffinwing.flight.Session.fly.seated(s, id, i, n, gw.craft_of(s, id)))

# The counters wind down; a fallen craft rises or stays down.

def griffinwing.flight.Session.fly.seated?(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c0: J.Val) -> J.Val:
  +c1 = gw.subs(c0, "hurt", 1)
  +c2 = gw.subs(c1, "cool", 1)
  +c3 = gw.subs(c2, "rest", 1)
  +c4 = gw.subs(c3, "chain_ticks", 1)
  +c5 = gw.ifj(U32.is_zero(ga.u(c4, "chain_ticks")), _ => ga.su(c4, "chain", 0), _ => c4)
  +c6 = gw.subs(c5, "tap_ticks", 1)
  gw.ifj(U32.is_gt(ga.u(c6, "down"), 0),
    _ => gw.put_craft(s, id, griffinwing.flight.Session.fly.down(id, c6)),
    _ => griffinwing.flight.Session.fly.up(s, id, i, n, c6))

def griffinwing.flight.Session.fly.down?(+id: U32, +c0: J.Val) -> J.Val:
  +c1 = ga.su(c0, "down", (ga.u(c0, "down") - 1 : U32))
  +c2 = ga.si(c1, "ds", griffinwing.types.PACE())
  +c3 = ga.su(c2, "held", 0)
  +c4 = ga.su(c3, "charge", 0)
  +c5 = ga.sv(c4, "locks", J.JA{[]})
  gw.ifj(U32.is_zero(ga.u(c5, "down")),
    _ => griffinwing.flight.Session.fly.rise(id, c5),
    _ => c5)

# Back in the wing, mended a little, unhurt for a moment.

def griffinwing.flight.Session.fly.rise?(+id: U32, +c0: J.Val) -> J.Val:
  +c1 = ga.si(c0, "x", griffinwing.flight.slot.x(id))
  +c2 = ga.si(c1, "z", griffinwing.flight.slot.z(id))
  +c3 = ga.si(c2, "vx", 0)
  +c4 = ga.si(c3, "vz", 0)
  +c5 = ga.si(c4, "shield", 60)
  +c6 = ga.su(c5, "hurt", (2 * rules.HZ() : U32))
  ga.su(c6, "roll", 0)

def griffinwing.flight.Session.fly.up?(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c0: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.up.roll(s, id, i, n, c0, ga.u(c0, "keys"), gw.double_tap(c0, ga.u(i, "move_x")))

# A second tap of a direction is a roll that way: the craft's tap state and
# whether it was a double.

def gw.double_tap?(+c: J.Val, +mx: U32) -> J.Val:
  gw.ifj(Bool.and(U32.is_ne(mx, 0), U32.is_ne(ga.u(c, "last_x"), mx)),
    _ => gw.double_tap.in(c, mx),
    _ => gw.pk(J.JBool{False{}}, c))

def gw.double_tap.in?(+c: J.Val, +mx: U32) -> J.Val:
  gw.ifj(Bool.and(U32.is_eq(ga.u(c, "tap_dir"), mx), U32.is_gt(ga.u(c, "tap_ticks"), 0)),
    _ => gw.pk(J.JBool{True{}}, ga.su(c, "tap_ticks", 0)),
    _ => gw.pk(J.JBool{False{}}, ga.su(ga.si(c, "tap_dir", mx), "tap_ticks", griffinwing.flight.TAP_WINDOW())))

def griffinwing.flight.Session.fly.up.roll?(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c0: J.Val, +old: U32, +dt: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.up.roll2(s, id, i, n, ga.si(gw.pc(dt), "last_x", ga.u(i, "move_x")), old, J.bool(gw.ps(dt)))

def griffinwing.flight.Session.fly.up.roll2?(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c: J.Val, +old: U32, +double: Bool) -> J.Val:
  gw.ifj(Bool.and(Bool.or(gw.edge(old, griffinwing.flight.K_DASH(), ga.b(i, "dash")), double),
      Bool.and(U32.is_zero(ga.u(c, "rest")), U32.is_zero(ga.u(c, "roll")))),
    _ => griffinwing.flight.Session.fly.up.step(
      griffinwing.Session.sound(s, "roll"), id, i, n, griffinwing.flight.Session.fly.rolled(c, i, n), old),
    _ => griffinwing.flight.Session.fly.up.step(s, id, i, n, c, old))

# The roll: a spinning shield, and a throw to the side.

def griffinwing.flight.Session.fly.rolled?(+c0: J.Val, +i: J.Val, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.rolled.dir(c0, n,
    gw.ifu(U32.is_ne(ga.u(i, "move_x"), 0),
      _ => gw.sig(ga.u(i, "move_x")),
      _ => gw.ifu(U32.is_gt(gw.abs(ga.u(c0, "vx")), 8), _ => gw.sig(ga.u(c0, "vx")), _ => ga.u(c0, "dir"))))

def griffinwing.flight.Session.fly.rolled.dir?(+c0: J.Val, +n: J.Val, +dir: U32) -> J.Val:
  +c1 = ga.si(c0, "dir", dir)
  +c2 = ga.su(c1, "roll", ga.u(n, "roll"))
  +c3 = ga.su(c2, "roll_len", ga.u(n, "roll"))
  +c4 = ga.su(c3, "rest", (ga.u(n, "roll") + griffinwing.flight.ROLL_REST() : U32))
  +c5 = ga.si(c4, "vx", (dir * griffinwing.flight.ROLL_V() : U32))
  +c6 = ga.su(c5, "held", 0)
  +c7 = ga.su(c6, "charge", 0)
  ga.sv(c7, "locks", J.JA{[]})

# From the roll's end to the end of the tick: the reins, the spur, the
# reticle, the fire key, Starfall.

def griffinwing.flight.Session.fly.up.step?(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c0: J.Val, +old: U32) -> J.Val:
  +c1 = ga.su(c0, "roll", ga.ssub(ga.u(c0, "roll"), 1))
  +c2 = griffinwing.flight.Session.fly.reins(c1, i, n)
  +c3 = griffinwing.flight.Session.fly.spur(c2, i, n)
  +c4 = griffinwing.flight.Session.fly.reticle(c3, i)
  +w5 = griffinwing.flight.Session.fly.fire(s, id, i, n, c4, old, griffinwing.flight.Session.craft_s(s, c4))
  +w6 = griffinwing.flight.Session.fly.bomb(gw.ps(w5), id, i, gw.pc(w5), old, griffinwing.flight.Session.craft_s(s, c4))
  gw.put_craft(gw.ps(w6), id, gw.pc(w6))

# The reins: speed toward the stick, the window's walls, the lean.

def griffinwing.flight.Session.fly.reins?(+c0: J.Val, +i: J.Val, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.reins.move(
    c0, n,
    (ga.u(i, "move_x") * griffinwing.flight.V_X() : U32),
    gw.neg((ga.u(i, "move_y") * griffinwing.flight.V_Z() : U32)))

def griffinwing.flight.Session.fly.reins.move?(+c0: J.Val, +n: J.Val, +tx: U32, +tz: U32) -> J.Val:
  +c1 = ga.si(c0, "vx", (ga.u(c0, "vx") + gw.clamp((tx - ga.u(c0, "vx") : U32), gw.neg(griffinwing.flight.ACC()), griffinwing.flight.ACC()) : U32))
  +c2 = ga.si(c1, "vz", (ga.u(c1, "vz") + gw.clamp((tz - ga.u(c1, "vz") : U32), gw.neg(griffinwing.flight.ACC()), griffinwing.flight.ACC()) : U32))
  +c3 = ga.si(c2, "x", (ga.u(c2, "x") + ga.u(c2, "vx") : U32))
  +c4 = ga.si(c3, "z", (ga.u(c3, "z") + ga.u(c3, "vz") : U32))
  +c5 = gw.ifj(U32.is_gt(gw.abs(ga.u(c4, "x")), griffinwing.Nums.wx(n)),
    _ => ga.si(ga.si(c4, "x", (griffinwing.Nums.wx(n) * gw.sig(ga.u(c4, "x")) : U32)), "vx", 0),
    _ => c4)
  +c6 = gw.ifj(U32.is_gt(gw.abs(ga.u(c5, "z")), griffinwing.Nums.wz(n)),
    _ => ga.si(ga.si(c5, "z", (griffinwing.Nums.wz(n) * gw.sig(ga.u(c5, "z")) : U32)), "vz", 0),
    _ => c5)
  +c7 = ga.si(c6, "bank", (ga.u(c6, "bank") + gw.div((gw.div((ga.u(c6, "vx") * griffinwing.types.Q() : U32), griffinwing.flight.V_X()) - ga.u(c6, "bank") : U32), 4) : U32))
  ga.si(c7, "lift", (ga.u(c7, "lift") + gw.div((gw.div((ga.u(c7, "vz") * griffinwing.types.Q() : U32), griffinwing.flight.V_Z()) - ga.u(c7, "lift") : U32), 4) : U32))

# The spur on a short meter, and the brake.

def griffinwing.flight.Session.fly.spur?(+c0: J.Val, +i: J.Val, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.spur.go(
    c0, i,
    gw.div(griffinwing.types.METER(), U32.max(ga.u(n, "spur"), 1)),
    gw.div(griffinwing.types.METER(), gw.max(gw.div((U32.max(ga.u(n, "spur"), 1) * 5 : U32), 2), 1)))

def griffinwing.flight.Session.fly.spur.go?(+c0: J.Val, +i: J.Val, +drain: U32, +refill: U32) -> J.Val:
  griffinwing.flight.Session.fly.spur.back(
    griffinwing.flight.Session.fly.spur.meter(
      ga.sb(
        ga.sb(c0, "boost", Bool.and(ga.b(i, "interact"), Bool.and(Bool.not(ga.b(c0, "spent")),
          Bool.and(gw.gt(ga.u(c0, "meter"), 0), U32.is_zero(ga.u(c0, "roll")))))),
        "brake", False{}),
      i, drain, refill),
    i)

def griffinwing.flight.Session.fly.spur.meter?(+c0: J.Val, +i: J.Val, +drain: U32, +refill: U32) -> J.Val:
  +c1 = ga.sb(c0, "brake", Bool.and(ga.b(i, "swing"), Bool.not(ga.b(c0, "boost"))))
  gw.ifj(ga.b(c1, "boost"),
    _ => griffinwing.flight.Session.fly.spur.burn(ga.si(c1, "meter", gw.max((ga.u(c1, "meter") - drain : U32), 0))),
    _ => griffinwing.flight.Session.fly.spur.fill(ga.si(c1, "meter", gw.min((ga.u(c1, "meter") + refill : U32), griffinwing.types.METER()))))

def griffinwing.flight.Session.fly.spur.burn?(+c: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(c, "meter")), _ => ga.sb(c, "spent", True{}), _ => c)

def griffinwing.flight.Session.fly.spur.fill?(+c: J.Val) -> J.Val:
  gw.ifj(Bool.and(ga.b(c, "spent"), gw.ge(ga.u(c, "meter"), gw.div(griffinwing.types.METER(), 4))),
    _ => ga.sb(c, "spent", False{}),
    _ => c)

# How far the spur or the brake slides the craft along the rail.

def griffinwing.flight.Session.fly.spur.back?(+c0: J.Val, +i: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.spur.slide(c0,
    gw.ifu(ga.b(c0, "boost"), _ => griffinwing.flight.BOOST_BACK(),
      _ => gw.ifu(ga.b(c0, "brake"), _ => gw.neg(griffinwing.flight.BRAKE_BACK()), _ => 0)))

def griffinwing.flight.Session.fly.spur.slide?(+c0: J.Val, +want: U32) -> J.Val:
  griffinwing.flight.Session.fly.spur.slide2(c0,
    (ga.u(c0, "back") + gw.clamp((want - ga.u(c0, "back") : U32), gw.neg(griffinwing.flight.BACK_V()), griffinwing.flight.BACK_V()) : U32))

def griffinwing.flight.Session.fly.spur.slide2?(+c0: J.Val, +back: U32) -> J.Val:
  ga.si(ga.si(c0, "ds", (griffinwing.types.PACE() + back - ga.u(c0, "back") : U32)), "back", back)

# The reticle: instant, and it stays where it is put.

def griffinwing.flight.Session.fly.reticle?(+c0: J.Val, +i: J.Val) -> J.Val:
  +c1 = ga.si(c0, "rx", gw.clamp((ga.u(c0, "rx") + (ga.u(i, "aim_x") * griffinwing.flight.RET_V() : U32) : U32), gw.neg(griffinwing.flight.RX_MAX()), griffinwing.flight.RX_MAX()))
  ga.si(c1, "rz", gw.clamp((ga.u(c1, "rz") - (ga.u(i, "aim_y") * griffinwing.flight.RET_V() : U32) : U32), gw.neg(griffinwing.flight.RZ_MAX()), griffinwing.flight.RZ_MAX()))

# Fire: a tap is a twin bolt; a hold charges and locks; letting go sends the
# locks.

def griffinwing.flight.Session.fly.fire?(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c: J.Val, +old: U32, +s_c: U32) -> J.Val:
  gw.ifj(Bool.and(U32.is_zero(ga.u(c, "roll")), ga.b(i, "fire")),
    _ => griffinwing.flight.Session.fly.fire.on(s, id, n, c, old, s_c),
    _ => gw.ifj(U32.is_gt(ga.u(c, "held"), 0),
      _ => griffinwing.flight.Session.fly.fire.off(s, id, n, c, s_c),
      _ => gw.pk(s, c)))

def griffinwing.flight.Session.fly.fire.on?(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +old: U32, +s_c: U32) -> J.Val:
  gw.ifj(U32.is_zero(U32.and(old, griffinwing.flight.K_FIRE())),
    _ => griffinwing.flight.Session.fly.fire.press(s, id, ga.su(c, "held", 1), s_c),
    _ => gw.ifj(U32.is_gt(ga.u(c, "held"), 0),
      _ => griffinwing.flight.Session.fly.fire.hold(s, ga.su(c, "held", U32.min((ga.u(c, "held") + 1 : U32), 30000)), n, s_c),
      _ => gw.pk(s, c)))

def griffinwing.flight.Session.fly.fire.press?(+s: J.Val, +id: U32, +c: J.Val, +s_c: U32) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(c, "cool")),
    _ => gw.pk(griffinwing.flight.Session.tap(s, id, c, s_c), ga.su(c, "cool", griffinwing.flight.TAP_REST())),
    _ => gw.pk(s, c))

def griffinwing.flight.Session.fly.fire.hold?(+s: J.Val, +c: J.Val, +n: J.Val, +s_c: U32) -> J.Val:
  gw.ifj(U32.is_gt(ga.u(c, "held"), griffinwing.flight.HOLD_AT()),
    _ => griffinwing.flight.Session.acquire(s, ga.su(c, "charge", U32.min((ga.u(c, "charge") + 1 : U32), ga.u(n, "charge"))), s_c),
    _ => gw.pk(s, c))

def griffinwing.flight.Session.fly.fire.off?(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +s_c: U32) -> J.Val:
  gw.ifj(Bool.and(U32.is_gt(ga.u(c, "charge"), 0), U32.is_zero(ga.u(c, "roll"))),
    _ => griffinwing.flight.Session.fly.fire.let_go(griffinwing.flight.Session.release(s, id, c, s_c, n)),
    _ => griffinwing.flight.Session.fly.fire.let_go(gw.pk(s, c)))

def griffinwing.flight.Session.fly.fire.let_go?(+w: J.Val) -> J.Val:
  +c0 = gw.pc(w)
  +c1 = ga.su(c0, "held", 0)
  +c2 = ga.su(c1, "charge", 0)
  +c3 = ga.su(c2, "dwell", 0)
  gw.pk(gw.ps(w), ga.sv(c3, "locks", J.JA{[]}))

# Starfall.

def griffinwing.flight.Session.fly.bomb?(+s: J.Val, +id: U32, +i: J.Val, +c: J.Val, +old: U32, +s_c: U32) -> J.Val:
  gw.ifj(Bool.and(gw.edge(old, griffinwing.flight.K_BOMB(), ga.b(i, "bomb")), Bool.and(U32.is_gt(ga.u(c, "bombs"), 0), U32.is_zero(ga.u(c, "roll")))),
    _ => gw.pk(griffinwing.combat.Session.starfall(s, id, s_c, ga.u(c, "x"), ga.u(c, "z")), ga.su(c, "bombs", (ga.u(c, "bombs") - 1 : U32))),
    _ => gw.pk(s, c))

# ─── the bolts a craft fires ────────────────────────────────────────────────

def griffinwing.types.Bolt.make?(+shot: String, +owner: U32, +s: U32, +x: U32, +z: U32, +vs: U32, +vx: U32, +vz: U32, +dmg: U32, +life: U32, +aim: U32, +src: U32, +delay: U32, +full: Bool) -> J.Val:
  +b0 = J.obj()
  +b1 = ga.ss(b0, "shot", shot)
  +b2 = ga.su(b1, "owner", owner)
  +b3 = ga.si(b2, "s", s)
  +b4 = ga.si(b3, "x", x)
  +b5 = ga.si(b4, "z", z)
  +b6 = ga.si(b5, "vs", vs)
  +b7 = ga.si(b6, "vx", vx)
  +b8 = ga.si(b7, "vz", vz)
  +b9 = ga.su(b8, "dmg", dmg)
  +b10 = ga.su(b9, "life", life)
  +b11 = ga.su(b10, "aim", aim)
  +b12 = ga.su(b11, "src", src)
  +b13 = ga.su(b12, "delay", delay)
  ga.sb(b13, "full", full)

def griffinwing.flight.Session.push_bolt?(+s: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(U32.is_lt(gw.len(s, "bolts"), griffinwing.types.MAX_BOLTS()),
    _ => ga.push(s, "bolts", b),
    _ => s)

# A tap: two bolts, side by side, both to where the reticle points.

def griffinwing.flight.Session.tap?(+s: J.Val, +owner: U32, +c: J.Val, +s_c: U32) -> J.Val:
  griffinwing.Session.sound(
    griffinwing.flight.Session.tap.side(
      griffinwing.flight.Session.tap.side(s, owner, c, s_c, gw.neg(1)),
      owner, c, s_c, 1),
    "shot_bolt")

def griffinwing.flight.Session.tap.side?(+s: J.Val, +owner: U32, +c: J.Val, +s_c: U32, +side: U32) -> J.Val:
  griffinwing.flight.Session.tap.fire(s, owner, c, s_c, side, (s_c + griffinwing.types.dm(12) : U32), (ga.u(c, "x") + (side * griffinwing.types.dm(4) : U32) : U32))

def griffinwing.flight.Session.tap.fire?(+s: J.Val, +owner: U32, +c: J.Val, +s_c: U32, +side: U32, +fs: U32, +fx: U32) -> J.Val:
  griffinwing.flight.Session.tap.bolt(s, owner, fs, fx, ga.u(c, "z"),
    griffinwing.types.scale3(
      ((s_c + griffinwing.types.FAR() : U32) - fs : U32),
      ((ga.u(c, "x") + ga.u(c, "rx") : U32) - fx : U32),
      ((ga.u(c, "z") + ga.u(c, "rz") : U32) - ga.u(c, "z") : U32),
      griffinwing.flight.V_BOLT()))

def griffinwing.flight.Session.tap.bolt?(+s: J.Val, +owner: U32, +fs: U32, +fx: U32, +fz: U32, +v: GwV3) -> J.Val:
  griffinwing.flight.Session.push_bolt(s,
    griffinwing.types.Bolt.make("Lance", owner, fs, fx, fz, (griffinwing.types.PACE() + gw.v3a(v) : U32), gw.v3b(v), gw.v3c(v),
      griffinwing.flight.LANCE(), griffinwing.flight.BOLT_LIFE(), 0, 0, 0, False{}))



# Everything the reticle sweeps over while F is held is locked: a bracket on
# each, snapping on. A great body (a lamp) takes up to three if the reticle
# dwells on it. Returns the workspace {"s","c"}; while it runs the workspace
# also carries "total" (the locks' count) and "dwelt".

def griffinwing.flight.Session.acquire?(+s: J.Val, +c0: J.Val, +s_c: U32) -> J.Val:
  +c1 = ga.sv(c0, "locks", J.JA{griffinwing.flight.Session.acquire.keep(ga.items(c0, "locks"), ga.items(s, "foes"), s_c)})
  griffinwing.flight.Session.acquire.done(
    griffinwing.flight.Session.acquire.go(ga.items(s, "foes"),
      J.set(J.set(gw.pk(s, c1), "total", J.JU{griffinwing.types.Craft.lock_total(ga.items(c1, "locks"))}), "dwelt", J.JBool{False{}}), s_c))

def griffinwing.flight.Session.acquire.done?(+w: J.Val) -> J.Val:
  gw.pk(gw.ps(w), gw.ifj(ga.b(w, "dwelt"), _ => gw.pc(w), _ => ga.su(gw.pc(w), "dwell", 0)))

def gw.ifj_l?(c: Bool, a: Unit -> List<&2, J.Val>, b: Unit -> List<&2, J.Val>) -> List<&2, J.Val>:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

# c.locks.retain(|l| a foe with that id is lockable and within reach)

def griffinwing.flight.Session.acquire.keep?(+ls: List<&2, J.Val>, +foes: List<&2, J.Val>, +s_c: U32) -> List<&2, J.Val>:
  match ls:
    case Nil{}:
      Nil{}
    case Con{+l, +t}:
      gw.ifj_l(griffinwing.flight.Session.acquire.held(foes, ga.u(l, "foe"), s_c),
        _ => Con{l, griffinwing.flight.Session.acquire.keep(t, foes, s_c)},
        _ => griffinwing.flight.Session.acquire.keep(t, foes, s_c))

def griffinwing.flight.Session.acquire.held?(+foes: List<&2, J.Val>, +id: U32, +s_c: U32) -> Bool:
  match foes:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      Bool.or(
        Bool.and(U32.is_eq(ga.u(f, "id"), id), Bool.and(griffinwing.types.Foe.lockable(f),
          gw.within((ga.u(f, "s") - s_c : U32), griffinwing.types.LOCK_MIN(), (griffinwing.types.LOCK_MAX() + griffinwing.types.dm(40) : U32)))),
        griffinwing.flight.Session.acquire.held(t, id, s_c))

def griffinwing.flight.Session.acquire.go?(+foes: List<&2, J.Val>, +w: J.Val, +s_c: U32) -> J.Val:
  match foes:
    case Nil{}:
      w
    case Con{+f, +t}:
      griffinwing.flight.Session.acquire.go(t, griffinwing.flight.Session.acquire.foe(f, w, s_c), s_c)

# One foe under the reticle's ray.

def griffinwing.flight.Session.acquire.foe?(+f: J.Val, +w: J.Val, +s_c: U32) -> J.Val:
  griffinwing.flight.Session.acquire.foe2(f, w, (ga.u(f, "s") - s_c : U32))

def griffinwing.flight.Session.acquire.foe2?(+f: J.Val, +w: J.Val, +ds: U32) -> J.Val:
  gw.ifj(Bool.or(Bool.not(griffinwing.types.Foe.lockable(f)), Bool.not(gw.within(ds, griffinwing.types.LOCK_MIN(), griffinwing.types.LOCK_MAX()))),
    _ => w,
    _ => griffinwing.flight.Session.acquire.foe3(f, w,
      gw.muldiv(ga.u(gw.pc(w), "rx"), ds, griffinwing.types.FAR()),
      gw.muldiv(ga.u(gw.pc(w), "rz"), ds, griffinwing.types.FAR())))

def griffinwing.flight.Session.acquire.foe3?(+f: J.Val, +w: J.Val, +ax: U32, +az: U32) -> J.Val:
  gw.ifj(gw.ngt(
      griffinwing.types.flat((ga.u(f, "x") - ga.u(gw.pc(w), "x") : U32), (ga.u(f, "z") - ga.u(gw.pc(w), "z") : U32), ax, az),
      (griffinwing.types.Foe.radius(f) + griffinwing.flight.LOCK_R() : U32)),
    _ => w,
    _ => griffinwing.flight.Session.acquire.lock(f, w, griffinwing.flight.Session.acquire.find(ga.items(gw.pc(w), "locks"), ga.u(f, "id"), 0)))

# The index of the lock on this foe, or the length.

def griffinwing.flight.Session.acquire.find?(+ls: List<&2, J.Val>, +id: U32, +k: U32) -> U32:
  match ls:
    case Nil{}:
      k
    case Con{+l, +t}:
      gw.ifu(U32.is_eq(ga.u(l, "foe"), id), _ => k, _ => griffinwing.flight.Session.acquire.find(t, id, (k + 1 : U32)))

def griffinwing.flight.Session.acquire.lock?(+f: J.Val, +w: J.Val, +at: U32) -> J.Val:
  gw.ifj(U32.is_lt(at, gw.len(gw.pc(w), "locks")),
    _ => griffinwing.flight.Session.acquire.dwell(f, w, at),
    _ => griffinwing.flight.Session.acquire.snap(f, w))

# The reticle dwells on a lamp already locked: up to three on one body.

def griffinwing.flight.Session.acquire.dwell?(+f: J.Val, +w: J.Val, +at: U32) -> J.Val:
  gw.ifj(Bool.and(gw.isn(f, "kind", "Lamp"),
      Bool.and(U32.is_lt(ga.u(gw.item(gw.pc(w), "locks", at), "n"), griffinwing.types.MAX_PER_BODY()),
        U32.is_lt(ga.u(w, "total"), griffinwing.types.MAX_LOCKS()))),
    _ => griffinwing.flight.Session.acquire.dwell2(J.set(w, "dwelt", J.JBool{True{}}), at, (ga.u(gw.pc(w), "dwell") + 1 : U32)),
    _ => w)

def griffinwing.flight.Session.acquire.dwell2?(+w: J.Val, +at: U32, +dwell: U32) -> J.Val:
  gw.ifj(U32.is_ge(dwell, 8),
    _ => gw.bump_total(
      J.set(
        J.set(w, "s", griffinwing.Session.sound(gw.ps(w), "spoil_pickup")),
        "c", gw.put_item(ga.su(gw.pc(w), "dwell", 0), "locks", at,
          ga.su(gw.item(gw.pc(w), "locks", at), "n", (ga.u(gw.item(gw.pc(w), "locks", at), "n") + 1 : U32))))),
    _ => J.set(w, "c", ga.su(gw.pc(w), "dwell", dwell)))

def gw.bump_total?(+w: J.Val) -> J.Val:
  ga.su(w, "total", (ga.u(w, "total") + 1 : U32))

# A foe not yet locked: a bracket snaps on.

def griffinwing.flight.Session.acquire.snap?(+f: J.Val, +w: J.Val) -> J.Val:
  gw.ifj(U32.is_lt(ga.u(w, "total"), griffinwing.types.MAX_LOCKS()),
    _ => griffinwing.flight.Session.acquire.snap2(f, gw.bump_total(w)),
    _ => w)

def griffinwing.flight.Session.acquire.snap2?(+f: J.Val, +w: J.Val) -> J.Val:
  J.set(
    J.set(w, "s", griffinwing.Session.spark(
      griffinwing.Session.sound(gw.ps(w), "spoil_pickup"),
      "Lock", ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), ga.u(w, "total"))),
    "c", ga.su(ga.push(gw.pc(w), "locks", griffinwing.flight.Lock.make(ga.u(f, "id"), 1)), "dwell", 0))

def griffinwing.flight.Lock.make?(+foe: U32, +n: U32) -> J.Val:
  ga.su(ga.su(J.obj(), "foe", foe), "n", n)

# ─── letting go ─────────────────────────────────────────────────────────────

# The spread a starburst leaves with, by the order it was released in.

def gw.spread_x?(+k: U32) -> U32:
  gw.spread_pick(U32.mod(k, 6), gw.neg(70), 70, 0, gw.neg(40), 40, 0)

def gw.spread_z?(+k: U32) -> U32:
  gw.spread_pick(U32.mod(k, 6), 40, 40, 90, gw.neg(40), gw.neg(40), 0)

def gw.spread_pick?(+m: U32, +a: U32, +b: U32, +c: U32, +d: U32, +e: U32, +f: U32) -> U32:
  Bool.pick(U32, U32.is_eq(m, 0), a, Bool.pick(U32, U32.is_eq(m, 1), b, Bool.pick(U32, U32.is_eq(m, 2), c,
    Bool.pick(U32, U32.is_eq(m, 3), d, Bool.pick(U32, U32.is_eq(m, 4), e, f)))))

# Returns the workspace {"s","c"}.

def griffinwing.flight.Session.release?(+s: J.Val, +owner: U32, +c: J.Val, +s_c: U32, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.release.done(
    c,
    griffinwing.flight.Session.release.locks(
      ga.items(c, "locks"), s, 0, c, s_c, owner,
      U32.is_ge(ga.u(c, "charge"), ga.u(n, "charge")),
      (griffinwing.flight.STAR_MIN() + U32.div(((griffinwing.flight.STAR_MAX() - griffinwing.flight.STAR_MIN() : U32) * U32.min(ga.u(c, "charge"), ga.u(n, "charge")) : U32), U32.max(ga.u(n, "charge"), 1)) : U32)))

# w = {"s", "k"}: if k > 0 { c.cool = REL_REST; sound }

def griffinwing.flight.Session.release.done?(+c: J.Val, +w: J.Val) -> J.Val:
  gw.ifj(U32.is_gt(ga.u(w, "k"), 0),
    _ => gw.pk(griffinwing.Session.sound(gw.ps(w), "shot_spark"), ga.su(c, "cool", griffinwing.flight.REL_REST())),
    _ => gw.pk(gw.ps(w), c))

def griffinwing.flight.Session.release.locks?(+ls: List<&2, J.Val>, +s: J.Val, +k: U32, +c: J.Val, +s_c: U32, +owner: U32, +full: Bool, +dmg: U32) -> J.Val:
  match ls:
    case Nil{}:
      J.set(J.set(J.obj(), "s", s), "k", J.JU{k})
    case Con{+l, +t}:
      griffinwing.flight.Session.release.next(t, griffinwing.flight.Session.release.each(s, k, ga.u(l, "n"), ga.u(l, "foe"), c, s_c, owner, full, dmg), c, s_c, owner, full, dmg)

def griffinwing.flight.Session.release.next?(+t: List<&2, J.Val>, +w: J.Val, +c: J.Val, +s_c: U32, +owner: U32, +full: Bool, +dmg: U32) -> J.Val:
  griffinwing.flight.Session.release.locks(t, gw.ps(w), ga.u(w, "k"), c, s_c, owner, full, dmg)

# `for _ in 0..l.n`: one starburst each, k counting them all.

def griffinwing.flight.Session.release.each?(+s: J.Val, +k: U32, +m: U32, +foe: U32, +c: J.Val, +s_c: U32, +owner: U32, +full: Bool, +dmg: U32) -> J.Val:
  gw.ifj(U32.is_zero(m),
    _ => J.set(J.set(J.obj(), "s", s), "k", J.JU{k}),
    _ => griffinwing.flight.Session.release.each(
      griffinwing.flight.Session.push_bolt(s,
        griffinwing.types.Bolt.make("Star", owner, (s_c + griffinwing.types.dm(12) : U32), ga.u(c, "x"), ga.u(c, "z"),
          (griffinwing.types.PACE() + 120 : U32), gw.spread_x(k), gw.spread_z(k), dmg, griffinwing.flight.STAR_LIFE(), foe, 0,
          U32.and((k * 2 : U32), 255), full)),
      (k + 1 : U32), (m - 1 : U32), foe, c, s_c, owner, full, dmg))

# ─── the wingmen ────────────────────────────────────────────────────────────

# The wingmen: formation flying with a little drift, a bolt now and then at
# what is nearest ahead.

def griffinwing.flight.Session.wing_fly?(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.wing_fly.go(s, 0)

def griffinwing.flight.Session.wing_fly.go?(+s: J.Val, +k: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, gw.len(s, "wing")),
    _ => griffinwing.flight.Session.wing_fly.go(griffinwing.flight.Session.wing_fly.one(s, k, gw.item(s, "wing", k)), (k + 1 : U32)),
    _ => s)

def griffinwing.flight.Session.wing_fly.one?(+s: J.Val, +k: U32, +w: J.Val) -> J.Val:
  gw.ifj(ga.b(w, "lost"),
    _ => gw.put_item(s, "wing", k, ga.sv(w, "craft", ga.si(J.get(w, "craft"), "z", gw.max((ga.u(J.get(w, "craft"), "z") - 20 : U32), gw.neg(griffinwing.types.dm(110)))))),
    _ => griffinwing.flight.Session.wing_fly.drift(s, k, w,
      (griffinwing.flight.rider_slot.x(k) + griffinwing.types.tri((ga.u(s, "t") + (41 * k : U32) : U32), (130 + (16 * k : U32) : U32), griffinwing.types.dm(16)) : U32),
      (griffinwing.flight.rider_slot.z(k) + griffinwing.types.tri((ga.u(s, "t") + (29 * k : U32) : U32), (96 + (12 * k : U32) : U32), griffinwing.types.dm(6)) : U32)))

def griffinwing.flight.Session.wing_fly.drift?(+s: J.Val, +k: U32, +w: J.Val, +tx: U32, +tz: U32) -> J.Val:
  griffinwing.flight.Session.wing_fly.lean(s, k, w, ga.u(J.get(w, "craft"), "x"), ga.u(J.get(w, "craft"), "z"), tx, tz)

def griffinwing.flight.Session.wing_fly.lean?(+s: J.Val, +k: U32, +w: J.Val, +ox: U32, +oz: U32, +tx: U32, +tz: U32) -> J.Val:
  griffinwing.flight.Session.wing_fly.fire(s, k, w,
    griffinwing.flight.Session.wing_fly.craft(J.get(w, "craft"), k,
      (ox + gw.div((tx - ox : U32), 8) : U32), (oz + gw.div((tz - oz : U32), 8) : U32), ox, oz))

# The craft after its drift, its lean and its tick (the rider's bolt follows).

def griffinwing.flight.Session.wing_fly.craft?(+c0: J.Val, +k: U32, +x: U32, +z: U32, +ox: U32, +oz: U32) -> J.Val:
  +c1 = ga.si(c0, "x", x)
  +c2 = ga.si(c1, "z", z)
  +c3 = ga.si(c2, "vx", (x - ox : U32))
  +c4 = ga.si(c3, "vz", (z - oz : U32))
  +c5 = ga.si(c4, "bank", (ga.u(c4, "bank") + gw.div((gw.div((ga.u(c4, "vx") * griffinwing.types.Q() : U32), griffinwing.flight.V_X()) - ga.u(c4, "bank") : U32), 4) : U32))
  +c6 = ga.si(c5, "lift", (ga.u(c5, "lift") + gw.div((gw.div((ga.u(c5, "vz") * griffinwing.types.Q() : U32), griffinwing.flight.V_Z()) - ga.u(c5, "lift") : U32), 4) : U32))
  +c7 = ga.si(c6, "bank", gw.clamp(ga.u(c6, "bank"), gw.neg(griffinwing.types.Q()), griffinwing.types.Q()))
  +c8 = ga.si(c7, "lift", gw.clamp(ga.u(c7, "lift"), gw.neg(griffinwing.types.Q()), griffinwing.types.Q()))
  +c9 = ga.si(c8, "back", griffinwing.flight.rider_slot.back(k))
  gw.subs(c9, "hurt", 1)

def griffinwing.flight.Session.wing_fly.fire?(+s: J.Val, +k: U32, +w: J.Val, +c: J.Val) -> J.Val:
  griffinwing.flight.Session.wing_fly.aim(s, k, ga.sv(w, "craft", c), ga.ssub(ga.u(w, "fire"), 1))

def griffinwing.flight.Session.wing_fly.aim?(+s: J.Val, +k: U32, +w: J.Val, +fire: U32) -> J.Val:
  gw.ifj(U32.is_zero(fire),
    _ => griffinwing.flight.Session.wing_fly.shoot(s, k, w,
      (griffinwing.Session.anchor(s) + ga.u(J.get(w, "craft"), "back") : U32),
      griffinwing.flight.Session.wing_fly.pick(ga.items(s, "foes"),
        (griffinwing.Session.anchor(s) + ga.u(J.get(w, "craft"), "back") : U32), ga.u(J.get(w, "craft"), "x"), ga.u(J.get(w, "craft"), "z"), J.JNull{}, 0)),
    _ => gw.put_item(s, "wing", k, ga.su(w, "fire", fire)))

# The nearest foe ahead, within range: [s, x, z] or null. `best` and `bd` carry
# the least so far; the first of equals wins.

def griffinwing.flight.Session.wing_fly.pick?(+foes: List<&2, J.Val>, +ms: U32, +mx: U32, +mz: U32, +best: J.Val, +bd: U32) -> J.Val:
  match foes:
    case Nil{}:
      best
    case Con{+f, +t}:
      griffinwing.flight.Session.wing_fly.pick2(f, t, ms, mx, mz, best, bd,
        Bool.and(Bool.and(griffinwing.types.Foe.lockable(f), Bool.not(gw.isn(f, "kind", "Lamp"))),
          gw.within((ga.u(f, "s") - ms : U32), griffinwing.types.dm(60), griffinwing.types.dm(400))))

def griffinwing.flight.Session.wing_fly.pick2?(+f: J.Val, +t: List<&2, J.Val>, +ms: U32, +mx: U32, +mz: U32, +best: J.Val, +bd: U32, +ok: Bool) -> J.Val:
  gw.ifj(ok,
    _ => griffinwing.flight.Session.wing_fly.pick3(f, t, ms, mx, mz, best, bd,
      griffinwing.types.norm((ga.u(f, "s") - ms : U32), (ga.u(f, "x") - mx : U32), (ga.u(f, "z") - mz : U32))),
    _ => griffinwing.flight.Session.wing_fly.pick(t, ms, mx, mz, best, bd))

def griffinwing.flight.Session.wing_fly.pick3?(+f: J.Val, +t: List<&2, J.Val>, +ms: U32, +mx: U32, +mz: U32, +best: J.Val, +bd: U32, +d: U32) -> J.Val:
  gw.ifj(Bool.or(J.is_null(best), U32.is_lt(d, bd)),
    _ => griffinwing.flight.Session.wing_fly.pick(t, ms, mx, mz,
      J.JA{[J.of_i32(ga.u(f, "s")), J.of_i32(ga.u(f, "x")), J.of_i32(ga.u(f, "z"))]}, d),
    _ => griffinwing.flight.Session.wing_fly.pick(t, ms, mx, mz, best, bd))

def griffinwing.flight.Session.wing_fly.shoot?(+s: J.Val, +k: U32, +w: J.Val, +ms: U32, +pick: J.Val) -> J.Val:
  gw.ifj(J.is_null(pick),
    _ => gw.put_item(s, "wing", k, ga.su(w, "fire", 6)),
    _ => griffinwing.flight.Session.wing_fly.shoot2(s, k, w, ms, pick,
      griffinwing.types.scale3(
        (ga.nu(pick, 0) - ms : U32), (ga.nu(pick, 1) - ga.u(J.get(w, "craft"), "x") : U32), (ga.nu(pick, 2) - ga.u(J.get(w, "craft"), "z") : U32),
        griffinwing.flight.V_BOLT())))

def griffinwing.flight.Session.wing_fly.shoot2?(+s: J.Val, +k: U32, +w: J.Val, +ms: U32, +pick: J.Val, +v: GwV3) -> J.Val:
  gw.put_item(
    griffinwing.flight.Session.push_bolt(s,
      griffinwing.types.Bolt.make("Ally", ga.u(w, "id"), (ms + griffinwing.types.dm(12) : U32), ga.u(J.get(w, "craft"), "x"), ga.u(J.get(w, "craft"), "z"),
        (griffinwing.types.PACE() + gw.v3a(v) : U32), gw.v3b(v), gw.v3c(v), 6, griffinwing.flight.BOLT_LIFE(), 0, 0, 0, False{})),
    "wing", k, ga.su(w, "fire", (14 + (3 * k : U32) : U32)))

# ─── the fall ───────────────────────────────────────────────────────────────

# After the beat's blows: a craft with no shield falls and rises again a
# moment later; a rider with none is lost to the beat.

def griffinwing.flight.Session.step_craft?(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.step_craft.riders(
    griffinwing.flight.Session.step_craft.seats(s, ga.ids(J.get(s, "seats")), griffinwing.Session.anchor(s)),
    0, griffinwing.Session.anchor(s))

def griffinwing.flight.Session.step_craft.seats?(+s: J.Val, +ids: List<&2, U32>, +anchor: U32) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.flight.Session.step_craft.seats(
        griffinwing.flight.Session.step_craft.seat(s, id, anchor, gw.craft_of(s, id)), t, anchor)

def griffinwing.flight.Session.step_craft.seat?(+s: J.Val, +id: U32, +anchor: U32, +c: J.Val) -> J.Val:
  gw.ifj(Bool.and(gw.le(ga.u(c, "shield"), 0), U32.is_zero(ga.u(c, "down"))),
    _ => griffinwing.flight.Session.step_craft.fall(s, id, anchor, c),
    _ => s)

def griffinwing.flight.Session.step_craft.fall?(+s: J.Val, +id: U32, +anchor: U32, +c: J.Val) -> J.Val:
  griffinwing.flight.Session.step_craft.release(
    griffinwing.Session.sound(
      griffinwing.Session.spark(
        gw.put_craft(s, id, griffinwing.flight.Session.step_craft.down(c)),
        "Fall", (anchor + ga.u(c, "back") : U32), ga.u(c, "x"), ga.u(c, "z"), id),
      "knight_fall"),
    id)

def griffinwing.flight.Session.step_craft.down?(+c0: J.Val) -> J.Val:
  +c1 = ga.su(c0, "down", griffinwing.RISE())
  +c2 = ga.si(c1, "shield", 0)
  +c3 = ga.su(c2, "roll", 0)
  +c4 = ga.su(c3, "held", 0)
  +c5 = ga.su(c4, "charge", 0)
  ga.sv(c5, "locks", J.JA{[]})

# Whatever hung on it lets go: `for f in foes { if Latched && f.on == id { Flee, ticks 40 } }`.

def griffinwing.flight.Session.step_craft.release?(+s: J.Val, +id: U32) -> J.Val:
  ga.sv(s, "foes", J.JA{griffinwing.flight.Session.step_craft.let_go(ga.items(s, "foes"), id)})

def griffinwing.flight.Session.step_craft.let_go?(+fs: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Con{
        gw.ifj(Bool.and(gw.isn(f, "st", "Latched"), U32.is_eq(ga.u(f, "on"), id)),
          _ => ga.su(gw.sets(f, "st", "Flee"), "ticks", 40),
          _ => f),
        griffinwing.flight.Session.step_craft.let_go(t, id)}

def griffinwing.flight.Session.step_craft.riders?(+s: J.Val, +k: U32, +anchor: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, gw.len(s, "wing")),
    _ => griffinwing.flight.Session.step_craft.riders(
      griffinwing.flight.Session.step_craft.rider(s, k, anchor, gw.item(s, "wing", k)), (k + 1 : U32), anchor),
    _ => s)

def griffinwing.flight.Session.step_craft.rider?(+s: J.Val, +k: U32, +anchor: U32, +w: J.Val) -> J.Val:
  gw.ifj(Bool.and(Bool.not(ga.b(w, "lost")), gw.le(ga.u(J.get(w, "craft"), "shield"), 0)),
    _ => griffinwing.flight.Session.step_craft.gone(s, k, anchor, w),
    _ => s)

def griffinwing.flight.Session.step_craft.gone?(+s: J.Val, +k: U32, +anchor: U32, +w: J.Val) -> J.Val:
  griffinwing.flight.Session.step_craft.release(
    griffinwing.Session.sound(
      griffinwing.Session.spark(
        griffinwing.Session.say(
          ga.sv(gw.put_item(s, "wing", k, ga.sb(w, "lost", True{})), "beat",
            ga.su(J.get(s, "beat"), "lost", RS.as_u16((ga.u(J.get(s, "beat"), "lost") + 1 : U32)))),
          ga.u(w, "id"), "Lost"),
        "Fall", (anchor + ga.u(J.get(w, "craft"), "back") : U32), ga.u(J.get(w, "craft"), "x"), ga.u(J.get(w, "craft"), "z"), ga.u(w, "id")),
      "knight_fall"),
    ga.u(w, "id"))

def gw.ifj?(c: Bool, a: Unit -> J.Val, b: Unit -> J.Val) -> J.Val:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def gw.ifu?(c: Bool, a: Unit -> U32, b: Unit -> U32) -> U32:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def gw.ifb?(c: Bool, a: Unit -> Bool, b: Unit -> Bool) -> Bool:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

# Signed (i32) compare / min / max / abs / signum, thin names over RS.

def gw.lt?(+a: U32, +b: U32) -> Bool:
  RS.i_lt(a, b)

def gw.le?(+a: U32, +b: U32) -> Bool:
  RS.i_le(a, b)

def gw.gt?(+a: U32, +b: U32) -> Bool:
  RS.i_gt(a, b)

def gw.ge?(+a: U32, +b: U32) -> Bool:
  RS.i_ge(a, b)

def gw.neg?(+a: U32) -> U32:
  (0 - a : U32)

def gw.abs?(+a: U32) -> U32:
  RS.i_abs(a)

def gw.min?(+a: U32, +b: U32) -> U32:
  RS.i_min(a, b)

def gw.max?(+a: U32, +b: U32) -> U32:
  RS.i_max(a, b)

def gw.clamp?(+x: U32, +lo: U32, +hi: U32) -> U32:
  RS.i_clamp(x, lo, hi)

def gw.sig?(+a: U32) -> U32:
  RS.i_signum(a)

def gw.div?(+a: U32, +b: U32) -> U32:
  RS.i_div(a, b)

def gw.rem?(+a: U32, +b: U32) -> U32:
  RS.i_rem(a, b)

# `lo <= v && v <= hi` on i32.

def gw.within?(+v: U32, +lo: U32, +hi: U32) -> Bool:
  Bool.and(RS.i_le(lo, v), RS.i_le(v, hi))

# `(-m..=m).contains(&v)`.

def gw.far?(+v: U32, +m: U32) -> Bool:
  gw.within(v, gw.neg(m), m)

# An i64 compared with a non-negative-or-negative i32 radius: `n <= r`, `n < r`
# for n a non-negative i64 held in a U32.

def gw.nle?(+n: U32, +r: U32) -> Bool:
  Bool.and(Bool.not(RS.i_is_neg(r)), U32.is_le(n, r))

def gw.nlt?(+n: U32, +r: U32) -> Bool:
  Bool.and(Bool.not(RS.i_is_neg(r)), U32.is_lt(n, r))

def gw.ngt?(+n: U32, +r: U32) -> Bool:
  Bool.or(RS.i_is_neg(r), U32.is_gt(n, r))

# Field readers: a string field equals a name; a unit-enum field.

def gw.isn?(+o: J.Val, +k: String, +name: String) -> Bool:
  String.eq(ga.s(o, k), name)

def gw.sets?(+o: J.Val, +k: String, +name: String) -> J.Val:
  J.set(o, k, J.JS{name})

# o.k += d / o.k -= d (wrapping), signed and unsigned writers.

def gw.addi?(+o: J.Val, +k: String, +d: U32) -> J.Val:
  ga.si(o, k, (ga.u(o, k) + d : U32))

def gw.addu?(+o: J.Val, +k: String, +d: U32) -> J.Val:
  ga.su(o, k, (ga.u(o, k) + d : U32))

# o.k = o.k.saturating_sub(d), unsigned.

def gw.subs?(+o: J.Val, +k: String, +d: U32) -> J.Val:
  ga.su(o, k, ga.ssub(ga.u(o, k), d))

# o.k |= bits (u8 flags).

def gw.orbits?(+o: J.Val, +k: String, +bits: U32) -> J.Val:
  ga.su(o, k, U32.or(ga.u(o, k), bits))

# A bit set: `1 << n`.

def gw.bitn?(+n: U32) -> U32:
  U32.shln(1, U32.to_nat(n))

# Array of an object field, its length, an item, a replaced item.

def gw.len?(+o: J.Val, +k: String) -> U32:
  J.len(J.get(o, k))

def gw.item?(+o: J.Val, +k: String, +i: U32) -> J.Val:
  J.nth(J.get(o, k), i)

def gw.put_item?(+o: J.Val, +k: String, +i: U32, +v: J.Val) -> J.Val:
  J.set(o, k, J.set_nth(J.get(o, k), i, v))

# `v.remove(i)` on an object's array field; `v.remove(0)`.

def gw.remove?(+o: J.Val, +k: String, +i: U32) -> J.Val:
  J.set(o, k, J.JA{ga.remove_nth(J.items(J.get(o, k)), U32.to_nat(i))})

# `v.truncate(n)` on an object's array field.

def gw.trunc?(+o: J.Val, +k: String, +n: U32) -> J.Val:
  J.set(o, k, J.truncate(J.get(o, k), n))

def gw.empty?() -> J.Val:
  J.JA{[]}

# `[a, b, c]` as J numbers (an i32 triple).

def gw.tri3?(+a: U32, +b: U32, +c: U32) -> J.Val:
  J.JA{[J.of_i32(a), J.of_i32(b), J.of_i32(c)]}

# A boolean list item, as the case runners write it.

def gw.jb?(+b: Bool) -> J.Val:
  J.JBool{b}

# `Option<u16>` and friends: an id or null.

def gw.opt_u?(+some: Bool, +v: U32) -> J.Val:
  Bool.pick(J.Val, some, J.JU{v}, J.JNull{})

# A pair output {s, r} and its parts.

def gw.out?(+s: J.Val, +r: J.Val) -> J.Val:
  J.set(J.set(J.obj(), "s", s), "r", r)

def gw.out_s?(+o: J.Val) -> J.Val:
  J.get(o, "s")

def gw.out_r?(+o: J.Val) -> J.Val:
  J.get(o, "r")

# `a.max(b)` / `a.min(b)` on u32.

def gw.umax?(+a: U32, +b: U32) -> U32:
  U32.max(a, b)

def gw.umin?(+a: U32, +b: U32) -> U32:
  U32.min(a, b)

# `seats.len()`: the seats are an object keyed by id.

def gw.nseats?(+s: J.Val) -> U32:
  gw.len_id(ga.ids(J.get(s, "seats")))

def griffinwing.rail.LEN?(+i: U32) -> U32:
  Bool.pick(U32, U32.is_eq(i, 0), 1800, 2400)

# Where the Wyrm rises in the third beat: its checkpoint.

def griffinwing.rail.BOSS_AT?() -> U32:
  480

# A beat's first point on the rail, as fixed point: [0, 1800 * PACE, (1800 + 2400) * PACE].

def griffinwing.rail.ORIGIN?(+i: U32) -> U32:
  Bool.pick(U32, U32.is_eq(i, 0), 0, Bool.pick(U32, U32.is_eq(i, 1), (1800 * griffinwing.types.PACE() : U32), (4200 * griffinwing.types.PACE() : U32)))

# The beat a point on the rail falls in.

def griffinwing.rail.beat_of?(+s: U32) -> U32:
  Bool.pick(U32, gw.ge(s, griffinwing.rail.ORIGIN(2)), 2, Bool.pick(U32, gw.ge(s, griffinwing.rail.ORIGIN(1)), 1, 0))

# The third beat's canyon is long enough that its fight never runs out of it.

def griffinwing.rail.FIGHT_LEN?() -> U32:
  (3 * 60 * 30 : U32)

def griffinwing.NAME?() -> String:
  "Griffinwing"

# The Dark Forest's room is the one an adventure runs in.

def griffinwing.Run.in_forest?(+run: J.Val) -> Bool:
  String.eq(J.str(J.get(rules.Run.room(run), "kind")), "forest")

def griffinwing.Run.griffinwing_showing?(+run: J.Val) -> Bool:
  Bool.and(griffinwing.Run.in_forest(run), Bool.not(J.is_null(J.get(run, "griffinwing"))))

# `home.roundtable[SLOT]`: the flags settled into the Home.

def griffinwing.Run.flags?(+run: J.Val) -> U32:
  ga.nu(J.get(J.get(run, "home"), "roundtable"), griffinwing.SLOT())

# `home.countryside.games.roundtable`: five records (best, wins, plays).

def griffinwing.Run.records?(+run: J.Val) -> J.Val:
  J.get(J.get(J.get(J.get(run, "home"), "countryside"), "games"), "roundtable")

def griffinwing.Score.default?() -> J.Val:
  ga.su(ga.su(ga.su(J.obj(), "best", 0), "wins", 0), "plays", 0)

# The record of this game.

def griffinwing.Run.record?(+run: J.Val) -> J.Val:
  gw.ifj(J.is_null(J.nth(griffinwing.Run.records(run), griffinwing.SLOT())),
    _ => griffinwing.Score.default(),
    _ => J.nth(griffinwing.Run.records(run), griffinwing.SLOT()))

# The five records, with this game's replaced (all five are written: absent
# means none had one).

def griffinwing.Run.put_record?(+run: J.Val, +rec: J.Val) -> J.Val:
  ga.sv(run, "home",
    ga.sv(J.get(run, "home"), "countryside",
      ga.sv(J.get(J.get(run, "home"), "countryside"), "games",
        ga.sv(J.get(J.get(J.get(run, "home"), "countryside"), "games"), "roundtable",
          J.set_nth(
            gw.ifj(J.is_null(griffinwing.Run.records(run)),
              _ => J.JA{[griffinwing.Score.default(), griffinwing.Score.default(), griffinwing.Score.default(), griffinwing.Score.default(), griffinwing.Score.default()]},
              _ => griffinwing.Run.records(run)),
            griffinwing.SLOT(), rec)))))

# `home.roundtable[SLOT] |= flags`; the array is left out when all zero.

def griffinwing.Run.put_flags?(+run: J.Val, +flags: U32) -> J.Val:
  ga.sv(run, "home", griffinwing.Run.put_flags.home(J.get(run, "home"), griffinwing.Run.put_flags.words(J.get(J.get(run, "home"), "roundtable"), flags)))

def griffinwing.Run.put_flags.words?(+rt: J.Val, +flags: U32) -> J.Val:
  J.set_nth(
    gw.ifj(J.is_null(rt), _ => J.JA{[J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}]}, _ => rt),
    griffinwing.SLOT(), J.JU{U32.or(ga.nu(rt, griffinwing.SLOT()), flags)})

def griffinwing.Run.put_flags.home?(+home: J.Val, +words: J.Val) -> J.Val:
  gw.ifj(griffinwing.Run.all_zero(J.items(words)),
    _ => J.del(home, "roundtable"),
    _ => ga.sv(home, "roundtable", words))

def griffinwing.Run.all_zero?(+ws: List<&2, J.Val>) -> Bool:
  match ws:
    case Nil{}:
      True{}
    case Con{+w, +t}:
      Bool.and(U32.is_zero(J.u32(w)), griffinwing.Run.all_zero(t))

# The notice line: `found = Some((tick, 0, text))`.

def griffinwing.Run.found?(+run: J.Val, +text: String) -> J.Val:
  ga.sv(run, "found", J.JA{[J.get(run, "tick"), J.JU{0}, J.JS{text}]})

def griffinwing.Run.sound?(+run: J.Val, +name: String) -> J.Val:
  ga.push(run, "sounds", J.JS{name})

# The party at the door, let in.

def griffinwing.Run.start_griffinwing?(+run: J.Val) -> J.Val:
  gw.ifj(Bool.not(griffinwing.Run.in_forest(run)),
    _ => run,
    _ => griffinwing.Run.start_griffinwing.go(run))

def griffinwing.Run.start_griffinwing.go?(+run: J.Val) -> J.Val:
  griffinwing.Run.start_griffinwing.fin(
    ga.sv(run, "forest", forest.Forest.leave(J.get(run, "forest"))))

def griffinwing.Run.start_griffinwing.fin?(+run: J.Val) -> J.Val:
  griffinwing.Run.sound(
    griffinwing.Run.found(
      griffinwing.Run.put_record(
        ga.sv(run, "griffinwing",
          ga.sb(griffinwing.Session.new(J.get(run, "players")), "first", U32.is_zero(U32.and(griffinwing.Run.flags(run), griffinwing.BEGUN())))),
        ga.su(griffinwing.Run.record(run), "plays", ga.sadd32(ga.u(griffinwing.Run.record(run), "plays"), 1))),
      griffinwing.NAME() ++ ": hold Esc to return."),
    "play_card")

# `present`: the heroes not outside (those who left the run for the kingdom).

def griffinwing.Run.present?(+players: J.Val, +outside: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      J.obj()
    case Con{+id, +t}:
      gw.ifj(ga.has_key(outside, id),
        _ => griffinwing.Run.present(players, outside, t),
        _ => ga.put_at(griffinwing.Run.present(players, outside, t), id, ga.at(players, id)))

def griffinwing.Run.tick_griffinwing?(+run: J.Val, +inputs: J.Val) -> J.Val:
  gw.ifj(J.is_null(J.get(run, "griffinwing")),
    _ => run,
    _ => griffinwing.Run.tick_griffinwing.step(run, griffinwing.Session.step(J.get(run, "griffinwing"), inputs,
      griffinwing.Run.present(J.get(run, "players"), J.get(J.get(run, "exits"), "outside"), ga.ids(J.get(run, "players"))),
      griffinwing.Nums.of(J.get(run, "tune")))))

# r = {"s": the stepped session, "out": {"sounds", "leave"}}

def griffinwing.Run.tick_griffinwing.step?(+run: J.Val, +r: J.Val) -> J.Val:
  griffinwing.Run.tick_griffinwing.settle(
    griffinwing.Run.tick_griffinwing.sounds(run, ga.items(J.get(r, "out"), "sounds")),
    J.get(r, "s"), ga.items(J.get(r, "out"), "leave"))

def griffinwing.Run.tick_griffinwing.sounds?(+run: J.Val, +names: List<&2, J.Val>) -> J.Val:
  match names:
    case Nil{}:
      run
    case Con{+n, +t}:
      griffinwing.Run.tick_griffinwing.sounds(ga.push(run, "sounds", n), t)

def griffinwing.Run.tick_griffinwing.settle?(+run: J.Val, +s: J.Val, +leave: List<&2, J.Val>) -> J.Val:
  gw.ifj(griffinwing.Session.finished(s),
    _ => griffinwing.Run.tick_griffinwing.put(griffinwing.Run.settle_griffinwing(run, s), leave),
    _ => griffinwing.Run.tick_griffinwing.put(gw.pk(run, s), leave))

# w = {"s": run, "c": session}

def griffinwing.Run.tick_griffinwing.put?(+w: J.Val, +leave: List<&2, J.Val>) -> J.Val:
  griffinwing.Run.tick_griffinwing.leaves(ga.sv(gw.ps(w), "griffinwing", gw.pc(w)), leave)

def griffinwing.Run.tick_griffinwing.leaves?(+run: J.Val, +ids: List<&2, J.Val>) -> J.Val:
  match ids:
    case Nil{}:
      run
    case Con{+id, +t}:
      griffinwing.Run.tick_griffinwing.leaves(exits.Run.leave_activity(run, J.u32(id)), t)

# The Wyrm is down: the party's Home is marked and the purse is paid, once for
# each knight who flew it home, however often this is called. Answers
# {"s": the run, "c": the session}.

def griffinwing.Run.settle_griffinwing?(+run: J.Val, +s: J.Val) -> J.Val:
  griffinwing.Run.settle_griffinwing.pay(run, s,
    gw.ifs(J.is_null(J.get(s, "medal")), _ => "Bronze", _ => ga.s(s, "medal")),
    griffinwing.Session.purse(s), Bool.not(ga.b(s, "settled")),
    ga.ids(J.get(s, "seats")), gw.ifu(J.is_null(J.get(s, "done")), _ => 0, _ => J.u32(J.get(s, "done"))))

# for id in the seats that sat before the Wyrm fell: pay once.

def griffinwing.Run.settle_griffinwing.pay?(+run: J.Val, +s: J.Val, +medal: String, +purse: U32, +first: Bool, +ids: List<&2, U32>, +flown: U32) -> J.Val:
  match ids:
    case Nil{}:
      griffinwing.Run.settle_griffinwing.first(run, s, medal, first)
    case Con{+id, +t}:
      gw.ifj(Bool.and(U32.is_le(ga.u(ga.at(J.get(s, "seats"), id), "joined"), flown), U32.is_zero(U32.and(ga.u(s, "paid"), griffinwing.bit(id)))),
        _ => griffinwing.Run.settle_griffinwing.pay(
          griffinwing.Run.settle_griffinwing.hero(run, id, medal, purse),
          ga.su(s, "paid", U32.or(ga.u(s, "paid"), griffinwing.bit(id))),
          medal, purse, first, t, flown),
        _ => griffinwing.Run.settle_griffinwing.pay(run, s, medal, purse, first, t, flown))

# hero.carried.add(Gold, purse); hero.laurel = medal == Gold

def griffinwing.Run.settle_griffinwing.hero?(+run: J.Val, +id: U32, +medal: String, +purse: U32) -> J.Val:
  gw.ifj(ga.has_key(J.get(run, "players"), id),
    _ => ga.sv(run, "players", ga.put_at(J.get(run, "players"), id,
      ga.sb(
        ga.sv(ga.at(J.get(run, "players"), id), "carried", realm.Spoils.add(ga.v(ga.at(J.get(run, "players"), id), "carried"), "gold", purse)),
        "laurel", String.eq(medal, "Gold")))),
    _ => run)

def griffinwing.Run.settle_griffinwing.first?(+run: J.Val, +s: J.Val, +medal: String, +first: Bool) -> J.Val:
  gw.ifj(first,
    _ => gw.pk(griffinwing.Run.settle_griffinwing.begun(run, s, medal), ga.sb(s, "settled", True{})),
    _ => gw.pk(run, s))

# The news and the Home's marks: once a run.

def griffinwing.Run.settle_griffinwing.begun?(+run: J.Val, +s: J.Val, +medal: String) -> J.Val:
  griffinwing.Run.settle_griffinwing.begun2(
    gw.ifj(String.eq(medal, "Gold"),
      _ => griffinwing.Run.put_flags(ga.sv(run, "marks", ga.su(ga.su(J.get(run, "marks"), griffinwing.CAMPAIGN_MARK(), 1), griffinwing.GOLD_MARK(), 1)),
        U32.or(griffinwing.BEGUN(), griffinwing.GOLD())),
      _ => griffinwing.Run.put_flags(ga.sv(run, "marks", ga.su(J.get(run, "marks"), griffinwing.CAMPAIGN_MARK(), 1)), griffinwing.BEGUN())),
    s, medal)

def griffinwing.Run.settle_griffinwing.begun2?(+run: J.Val, +s: J.Val, +medal: String) -> J.Val:
  griffinwing.Run.settle_griffinwing.begun3(run, medal,
    ga.su(
      ga.su(griffinwing.Run.record(run), "wins", U32.min(ga.sadd32(ga.u(griffinwing.Run.record(run), "wins"), 1), ga.u(griffinwing.Run.record(run), "plays"))),
      "best", U32.max(ga.u(griffinwing.Run.record(run), "best"), griffinwing.Session.permille(s))))

def griffinwing.Run.settle_griffinwing.begun3?(+run: J.Val, +medal: String, +rec: J.Val) -> J.Val:
  ga.push(
    griffinwing.Run.sound(
      griffinwing.Run.found(griffinwing.Run.put_record(run, rec), griffinwing.NAME() ++ ": " ++ griffinwing.types.Medal.name(medal) ++ " medal. The last beacon is lit!"),
      "play_card"),
    "cues", J.JS{"pastime_won"})

# ─── ADVENTURE ──────────────────────────────────────────────────────────────

# Whether its state, if any, is one a run may hold.

def griffinwing.ADVENTURE.valid?(+run: J.Val) -> Bool:
  gw.ifb(J.is_null(J.get(run, "griffinwing")), _ => True{}, _ => griffinwing.Session.valid(J.get(run, "griffinwing")))

# End its run: the state cleared.

def griffinwing.ADVENTURE.clear?(+run: J.Val) -> J.Val:
  J.del(run, "griffinwing")

# The knights its run seats.

def griffinwing.ADVENTURE.seated?(+run: J.Val) -> J.Val:
  J.JA{griffinwing.ADVENTURE.seated.ids(ga.ids(J.get(J.get(run, "griffinwing"), "seats")))}

def griffinwing.ADVENTURE.seated.ids?(+ids: List<&2, U32>) -> List<&2, J.Val>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      Con{J.JU{id}, griffinwing.ADVENTURE.seated.ids(t)}

# What a bot presses for knight `id`.

def griffinwing.ADVENTURE.bot?(+run: J.Val, +id: U32) -> J.Val:
  gw.ifj(Bool.and(Bool.not(J.is_null(J.get(run, "griffinwing"))), ga.has_key(J.get(run, "players"), id)),
    _ => griffinwing.bot.Session.bot_with(J.get(run, "griffinwing"), id, griffinwing.Nums.of(J.get(run, "tune")), False{}),
    _ => rules.Input.default())

# ─── ACTIVITY ───────────────────────────────────────────────────────────────

def griffinwing.ACTIVITY.on?(+run: J.Val) -> Bool:
  griffinwing.Run.griffinwing_showing(run)

# A ride's tick: it plays, then gives the keys the rest of the tick sees (an
# empty set: it took them all). {"run", "inputs"}.

def griffinwing.ACTIVITY.ride?(+run: J.Val, +inputs: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "run", griffinwing.Run.tick_griffinwing(run, inputs)), "inputs", J.obj())

# It is over: its state cleared.

def griffinwing.ACTIVITY.end?(+run: J.Val) -> J.Val:
  J.del(run, "griffinwing")

# What a finished run pays into the realm at the save, once: the marks become
# flags in the Home. `realm` is {"home": {"roundtable": [..]}}; answers
# {"realm", "settled": {"changed", "said"}}.

def griffinwing.settle?(+realm: J.Val, +marks: J.Val) -> J.Val:
  griffinwing.settle.flags(realm,
    U32.or(gw.ifu(J.has(marks, griffinwing.CAMPAIGN_MARK()), _ => griffinwing.BEGUN(), _ => 0),
      gw.ifu(J.has(marks, griffinwing.GOLD_MARK()), _ => griffinwing.GOLD(), _ => 0)))

def griffinwing.settle.flags?(+realm: J.Val, +flags: U32) -> J.Val:
  griffinwing.settle.done(realm, flags,
    U32.is_ne(U32.and(flags, U32.not(ga.nu(J.get(J.get(realm, "home"), "roundtable"), griffinwing.SLOT()))), 0))

def griffinwing.settle.done?(+realm: J.Val, +flags: U32, +changed: Bool) -> J.Val:
  ga.sv(
    ga.sv(J.obj(), "realm",
      gw.ifj(U32.is_zero(flags),
        _ => realm,
        _ => ga.sv(realm, "home", griffinwing.Run.put_flags.home(J.get(realm, "home"), griffinwing.Run.put_flags.words(J.get(J.get(realm, "home"), "roundtable"), flags))))),
    "settled",
    ga.sv(ga.sb(J.obj(), "changed", changed), "said",
      gw.ifj(changed,
        _ => J.JA{[J.JA{[J.JS{"griffinwing"}, J.JS{griffinwing.NAME() ++ ": the last beacon is lit."}]}]},
        _ => J.JA{[]})))

# The shell's `showing` and `start` entries are the Run's methods themselves.

def griffinwing.ADVENTURE.showing?(+run: J.Val) -> Bool:
  griffinwing.Run.griffinwing_showing(run)

def griffinwing.ADVENTURE.start?(+run: J.Val) -> J.Val:
  griffinwing.Run.start_griffinwing(run)

def gw.ev?(+at: U32, +act: J.Val) -> J.Val:
  ga.sv(ga.su(J.obj(), "at", at), "act", act)

def gw.act_wisps?(+n: U32, +plus: U32, +shape: U32, +x: U32, +z: U32, +mode: U32) -> J.Val:
  +a0 = ga.ss(J.obj(), "k", "Wisps")
  +a1 = ga.su(a0, "n", n)
  +a2 = ga.su(a1, "plus", plus)
  +a3 = ga.su(a2, "shape", shape)
  +a4 = ga.si(a3, "x", x)
  +a5 = ga.si(a4, "z", z)
  ga.su(a5, "mode", mode)

def gw.act_convoy?(+n: U32, +plus: U32, +from: U32, +z: U32) -> J.Val:
  ga.si(ga.si(ga.su(ga.su(ga.ss(J.obj(), "k", "Convoy"), "n", n), "plus", plus), "from", from), "z", z)

def gw.act_hunter?(+side: U32) -> J.Val:
  ga.si(ga.ss(J.obj(), "k", "Hunter"), "side", side)

def gw.act_turret?(+x: U32, +z: U32) -> J.Val:
  ga.si(ga.si(ga.ss(J.obj(), "k", "Turret"), "x", x), "z", z)

def gw.act_ridge?(+x: U32, +bombers: U32, +plus: U32) -> J.Val:
  ga.su(ga.su(ga.si(ga.ss(J.obj(), "k", "Ridge"), "x", x), "bombers", bombers), "plus", plus)

def gw.act_gate?(+rel: U32, +x: U32, +z: U32, +bonus: Bool) -> J.Val:
  ga.sb(ga.si(ga.si(ga.si(ga.ss(J.obj(), "k", "Gate"), "rel", rel), "x", x), "z", z), "bonus", bonus)

def gw.act_ring?(+rel: U32, +x: U32, +z: U32) -> J.Val:
  ga.si(ga.si(ga.si(ga.ss(J.obj(), "k", "Ring"), "rel", rel), "x", x), "z", z)

def gw.act_arch?(+rel: U32, +x: U32, +z: U32, +w: U32, +h: U32) -> J.Val:
  ga.si(ga.si(ga.si(ga.si(ga.si(ga.ss(J.obj(), "k", "Arch"), "rel", rel), "x", x), "z", z), "w", w), "h", h)

def gw.act_shutter?(+rel: U32, +phase: U32) -> J.Val:
  ga.su(ga.si(ga.ss(J.obj(), "k", "Shutter"), "rel", rel), "phase", phase)

def gw.act_say?(+line: String) -> J.Val:
  ga.ss(ga.ss(J.obj(), "k", "Say"), "line", line)

def gw.act_beast?() -> J.Val:
  ga.ss(J.obj(), "k", "Beast")

# How many ticks the rail takes to carry a thing from where it appears to the
# formation.

def griffinwing.script.LEAD_TICKS?() -> U32:
  U32.div(griffinwing.types.LEAD(), griffinwing.types.PACE())

# The scripts, built once (here: by stage).

def griffinwing.script.list?(+stage: U32) -> List<&2, J.Val>:
  gw.ifjl(U32.is_eq(U32.min(stage, 2), 0), _ => griffinwing.script.thunderhead(),
    _ => gw.ifjl(U32.is_eq(U32.min(stage, 2), 1), _ => griffinwing.script.causeway(), _ => griffinwing.script.roost()))

def griffinwing.script.script?(+stage: U32) -> J.Val:
  J.JA{griffinwing.script.list(stage)}

# Where a beat's script stands when it comes back to the tick `cp`: how many of
# its events come before it.

def griffinwing.script.resume?(+stage: U32, +cp: U32) -> U32:
  griffinwing.script.resume.go(griffinwing.script.list(stage), cp)

def griffinwing.script.resume.go?(+es: List<&2, J.Val>, +cp: U32) -> U32:
  match es:
    case Nil{}:
      0
    case Con{+e, +t}:
      gw.ifu(U32.is_lt(ga.u(e, "at"), cp), _ => (1 + griffinwing.script.resume.go(t, cp) : U32), _ => 0)

# Run the events that are due.

def griffinwing.script.Session.run_script?(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.script.Session.run_script.go(s, griffinwing.script.script(ga.u(s, "stage")))

def griffinwing.script.Session.run_script.go?(+s: J.Val, +script: J.Val) -> J.Val:
  griffinwing.script.Session.run_script.ev(s, script, J.nth(script, ga.u(s, "next")))

def griffinwing.script.Session.run_script.ev?(+s: J.Val, +script: J.Val, +ev: J.Val) -> J.Val:
  gw.ifj(Bool.or(J.is_null(ev), U32.is_gt(ga.u(ev, "at"), ga.u(s, "t"))),
    _ => s,
    _ => griffinwing.script.Session.run_script.go(
      griffinwing.script.Session.act(ga.su(s, "next", U32.and((ga.u(s, "next") + 1 : U32), 65535)), J.get(ev, "act")),
      script))

# A centred spread: the `k`th of `count`, in half steps.

def griffinwing.script.Session.spread?(+k: U32, +count: U32) -> U32:
  ((2 * k : U32) - (count - 1 : U32) : U32)

# How many of a wave: `n + (seats - 1) * plus / 2`.

def griffinwing.script.Session.count?(+s: J.Val, +n: U32, +plus: U32) -> U32:
  (n + gw.div(((griffinwing.Session.pressure(s) - 1 : U32) * plus : U32), 2) : U32)

def griffinwing.script.Session.act?(+s: J.Val, +act: J.Val) -> J.Val:
  griffinwing.script.Session.act.kind(s, act, ga.s(act, "k"), (griffinwing.Session.anchor(s) + griffinwing.types.LEAD() : U32), griffinwing.rail.ORIGIN(U32.min(ga.u(s, "stage"), 2)))

def griffinwing.script.Session.act.kind?(+s: J.Val, +act: J.Val, +k: String, +far: U32, +origin: U32) -> J.Val:
  gw.ifj(String.eq(k, "Wisps"), _ => griffinwing.script.Session.act.wisps(s, act, far),
    _ => gw.ifj(String.eq(k, "Convoy"), _ => griffinwing.script.Session.act.convoy(s, act, far),
      _ => gw.ifj(String.eq(k, "Hunter"), _ => griffinwing.script.Session.act.hunter(s, act, far),
        _ => gw.ifj(String.eq(k, "Turret"), _ => griffinwing.script.Session.act.turret(s, act, far),
          _ => gw.ifj(String.eq(k, "Ridge"), _ => griffinwing.script.Session.act.ridge(s, act, far),
            _ => gw.ifj(String.eq(k, "Gate"), _ => griffinwing.script.Session.act.gate(s, act, origin),
              _ => gw.ifj(String.eq(k, "Ring"), _ => griffinwing.script.Session.act.ring(s, act, origin),
                _ => gw.ifj(String.eq(k, "Arch"), _ => griffinwing.script.Session.act.arch(s, act, origin),
                  _ => gw.ifj(String.eq(k, "Shutter"), _ => griffinwing.script.Session.act.shutter(s, act, origin),
                    _ => gw.ifj(String.eq(k, "Say"), _ => griffinwing.Session.say(s, griffinwing.Session.voice(s), ga.s(act, "line")),
                      _ => griffinwing.boss.Session.rise(s)))))))))))

# A flight of snuffers.

def griffinwing.script.Session.act.wisps?(+s: J.Val, +act: J.Val, +far: U32) -> J.Val:
  griffinwing.script.Session.act.wisps.pin(
    griffinwing.script.Session.act.wisps.go(s, act, far, 0, griffinwing.script.Session.count(s, ga.u(act, "n"), ga.u(act, "plus"))),
    ga.u(act, "mode"))

# if mode == 3 { for f in foes where f.mode == 3 && f.ticks == 0 { f.ticks = 60 } }

def griffinwing.script.Session.act.wisps.pin?(+s: J.Val, +mode: U32) -> J.Val:
  gw.ifj(U32.is_eq(mode, 3),
    _ => ga.sv(s, "foes", J.JA{griffinwing.script.Session.act.wisps.pin.go(ga.items(s, "foes"))}),
    _ => s)

def griffinwing.script.Session.act.wisps.pin.go?(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Con{
        gw.ifj(Bool.and(U32.is_eq(ga.u(f, "mode"), 3), U32.is_zero(ga.u(f, "ticks"))), _ => ga.su(f, "ticks", 60), _ => f),
        griffinwing.script.Session.act.wisps.pin.go(t)}

def griffinwing.script.Session.act.wisps.go?(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +count: U32) -> J.Val:
  gw.ifj(gw.lt(k, count),
    _ => griffinwing.script.Session.act.wisps.go(
      griffinwing.script.Session.act.wisps.one(s, act, far, k, griffinwing.script.Session.spread(k, count)),
      act, far, (k + 1 : U32), count),
    _ => s)

# The wisp's offset by shape: (dx, dz, ds, vx).

def griffinwing.script.Session.act.wisps.one?(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +sp: U32) -> J.Val:
  griffinwing.script.Session.act.wisps.spawn(s, act, far, k, ga.u(act, "shape"), sp)

def griffinwing.script.Session.act.wisps.spawn?(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +shape: U32, +sp: U32) -> J.Val:
  gw.ifj(U32.is_eq(shape, 0),
    _ => griffinwing.script.Session.act.wisps.put(s, act, far, k, (sp * griffinwing.types.dm(16) : U32), 0, 0, 0),
    _ => gw.ifj(U32.is_eq(shape, 1),
      _ => griffinwing.script.Session.act.wisps.put(s, act, far, k, (sp * griffinwing.types.dm(13) : U32),
        gw.neg((gw.abs(sp) * griffinwing.types.dm(3) : U32)), (gw.abs(sp) * griffinwing.types.dm(10) : U32), 0),
      _ => gw.ifj(U32.is_eq(shape, 2),
        _ => griffinwing.script.Session.act.wisps.put(s, act, far, k, 0, 0, (k * griffinwing.types.dm(34) : U32), 0),
        _ => gw.ifj(U32.is_eq(shape, 3),
          _ => griffinwing.script.Session.act.wisps.side(s, act, far, k, gw.ifu(U32.is_eq(U32.mod(k, 2), 0), _ => 1, _ => gw.neg(1))),
          _ => griffinwing.script.Session.act.wisps.put(s, act, far, k, (sp * griffinwing.types.dm(15) : U32),
            (gw.rem(k, 2) * griffinwing.types.dm(12) : U32), (k * griffinwing.types.dm(16) : U32), 0)))))

def griffinwing.script.Session.act.wisps.side?(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +side: U32) -> J.Val:
  griffinwing.script.Session.act.wisps.put(s, act, far, k, (side * griffinwing.types.dm(95) : U32),
    (gw.rem(k, 3) * griffinwing.types.dm(10) : U32), (k * griffinwing.types.dm(22) : U32), gw.neg((side * 44 : U32)))

def griffinwing.script.Session.act.wisps.put?(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +dx: U32, +dz: U32, +ds: U32, +vx: U32) -> J.Val:
  gw.out_s(griffinwing.combat.Session.spawn_foe(s, "Wisp",
    (far + ds : U32), (griffinwing.types.dm(ga.u(act, "x")) + dx : U32), (griffinwing.types.dm(ga.u(act, "z")) + dz : U32),
    gw.neg(15), vx, 0, "Fly", ga.u(act, "mode"), U32.and((k * 7 : U32), 255)))

# Kites crossing in a file from one side.

def griffinwing.script.Session.act.convoy?(+s: J.Val, +act: J.Val, +far: U32) -> J.Val:
  griffinwing.script.Session.act.convoy.go(s, act, far, 0, griffinwing.script.Session.count(s, ga.u(act, "n"), ga.u(act, "plus")))

def griffinwing.script.Session.act.convoy.go?(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +count: U32) -> J.Val:
  gw.ifj(gw.lt(k, count),
    _ => griffinwing.script.Session.act.convoy.go(
      gw.out_s(griffinwing.combat.Session.spawn_foe(s, "Kite",
        (far + (k * griffinwing.types.dm(26) : U32) : U32), (ga.u(act, "from") * griffinwing.types.dm(95) : U32), griffinwing.types.dm(ga.u(act, "z")),
        gw.neg(15), gw.neg((ga.u(act, "from") * 46 : U32)), 0, "Fly", 0, U32.and((k * 5 : U32), 255))),
      act, far, (k + 1 : U32), count),
    _ => s)

# A kite that hunts a wingman and latches onto its tail.

def griffinwing.script.Session.act.hunter?(+s: J.Val, +act: J.Val, +far: U32) -> J.Val:
  gw.out_s(griffinwing.combat.Session.spawn_foe(s, "Kite",
    (far - griffinwing.types.dm(40) : U32), (ga.u(act, "side") * griffinwing.types.dm(90) : U32), griffinwing.types.dm(40),
    0, 0, 0, "Hunt", 1, 0))

# A lantern tower on a ledge.

def griffinwing.script.Session.act.turret?(+s: J.Val, +act: J.Val, +far: U32) -> J.Val:
  griffinwing.script.Session.act.turret.set(
    griffinwing.combat.Session.spawn_foe(s, "Turret", far, griffinwing.types.dm(ga.u(act, "x")), griffinwing.types.dm(ga.u(act, "z")),
      0, 0, 0, "Rest", 0, U32.mod(ga.u(s, "next"), 5)))

def griffinwing.script.Session.act.turret.set?(+r: J.Val) -> J.Val:
  gw.ifj(J.is_null(gw.out_r(r)),
    _ => gw.out_s(r),
    _ => griffinwing.script.Session.act.turret.ticks(gw.out_s(r), griffinwing.combat.Session.bombers_ram.at(ga.items(gw.out_s(r), "foes"), J.u32(gw.out_r(r)), 0)))

def griffinwing.script.Session.act.turret.ticks?(+s: J.Val, +i: U32) -> J.Val:
  gw.ifj(U32.is_lt(i, gw.len(s, "foes")),
    _ => gw.put_item(s, "foes", i, ga.su(gw.item(s, "foes", i), "ticks", 30)),
    _ => s)

def griffinwing.types.Prop.make?(+thing: String, +s: U32, +x: U32, +z: U32, +w: U32, +h: U32, +len: U32, +phase: U32) -> J.Val:
  +p0 = ga.ss(J.obj(), "thing", thing)
  +p1 = ga.si(p0, "s", s)
  +p2 = ga.si(p1, "x", x)
  +p3 = ga.si(p2, "z", z)
  +p4 = ga.si(p3, "w", w)
  +p5 = ga.si(p4, "h", h)
  +p6 = ga.si(p5, "len", len)
  +p7 = ga.su(p6, "taken", 0)
  ga.su(p7, "phase", phase)

# `if self.props.len() < MAX_PROPS { push }`.

def gw.push_prop?(+s: J.Val, +p: J.Val) -> J.Val:
  gw.ifj(U32.is_lt(gw.len(s, "props"), griffinwing.types.MAX_PROPS()), _ => ga.push(s, "props", p), _ => s)

# A ridge with dive-bombers waiting behind it.

def griffinwing.script.Session.act.ridge?(+s: J.Val, +act: J.Val, +far: U32) -> J.Val:
  griffinwing.script.Session.act.ridge.bombers(
    gw.push_prop(s, griffinwing.types.Prop.make("Ridge", far, griffinwing.types.dm(ga.u(act, "x")), gw.neg(griffinwing.types.dm(30)),
      griffinwing.types.dm(30), griffinwing.types.dm(55), griffinwing.types.dm(60), 0)),
    act, far, 0, griffinwing.script.Session.count(s, ga.u(act, "bombers"), ga.u(act, "plus")))

def griffinwing.script.Session.act.ridge.bombers?(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +count: U32) -> J.Val:
  gw.ifj(gw.lt(k, count),
    _ => griffinwing.script.Session.act.ridge.bombers(
      gw.out_s(griffinwing.combat.Session.spawn_foe(s, "Bomber",
        (far + griffinwing.types.dm(10) + (k * griffinwing.types.dm(8) : U32) : U32),
        (griffinwing.types.dm(ga.u(act, "x")) + (gw.rem(k, 3) - 1 : U32) * griffinwing.types.dm(14) : U32),
        gw.neg(griffinwing.types.dm(6)), 0, 0, 0, "Hide", 0, U32.and((k * 3 : U32), 255))),
      act, far, (k + 1 : U32), count),
    _ => s)

# A bonus gate (or a victory gate), a hearth ring, a rock arch, a pair of
# shutters: where the rail brings them.

def griffinwing.script.Session.act.gate?(+s: J.Val, +act: J.Val, +origin: U32) -> J.Val:
  gw.ifj(U32.is_lt(gw.len(s, "props"), griffinwing.types.MAX_PROPS()),
    _ => griffinwing.script.Session.act.gate.offer(
      ga.push(s, "props", griffinwing.types.Prop.make("Gate", (origin + ga.u(act, "rel") : U32), griffinwing.types.dm(ga.u(act, "x")), griffinwing.types.dm(ga.u(act, "z")),
        griffinwing.types.dm(20), 0, 0, gw.ifu(ga.b(act, "bonus"), _ => 0, _ => 1))),
      ga.b(act, "bonus")),
    _ => s)

def griffinwing.script.Session.act.gate.offer?(+s: J.Val, +bonus: Bool) -> J.Val:
  gw.ifj(bonus,
    _ => ga.sv(s, "beat", gw.addu(J.get(s, "beat"), "offered", (100 * griffinwing.Session.pressure(s) : U32))),
    _ => s)

def griffinwing.script.Session.act.ring?(+s: J.Val, +act: J.Val, +origin: U32) -> J.Val:
  gw.push_prop(s, griffinwing.types.Prop.make("Ring", (origin + ga.u(act, "rel") : U32), griffinwing.types.dm(ga.u(act, "x")), griffinwing.types.dm(ga.u(act, "z")),
    griffinwing.types.dm(22), 0, 0, 0))

def griffinwing.script.Session.act.arch?(+s: J.Val, +act: J.Val, +origin: U32) -> J.Val:
  gw.push_prop(s, griffinwing.types.Prop.make("Arch", (origin + ga.u(act, "rel") : U32), griffinwing.types.dm(ga.u(act, "x")), griffinwing.types.dm(ga.u(act, "z")),
    griffinwing.types.dm(ga.u(act, "w")), griffinwing.types.dm(ga.u(act, "h")), griffinwing.types.dm(30), 0))

def griffinwing.script.Session.act.shutter?(+s: J.Val, +act: J.Val, +origin: U32) -> J.Val:
  gw.push_prop(s, griffinwing.types.Prop.make("Shutter", (origin + ga.u(act, "rel") : U32), 0, 0, 0, 0, griffinwing.types.dm(30), ga.u(act, "phase")))

# The warm fly-through after the Wyrm: a procession of gold gates and hearth
# rings, lit for the way home. It pays nothing and offers nothing.

def griffinwing.script.Session.victory_gates?(+s: J.Val) -> J.Val:
  griffinwing.script.Session.victory_gates.go(s, (griffinwing.Session.anchor(s) + griffinwing.types.dm(150) : U32), 0)

def griffinwing.script.Session.victory_gates.go?(+s: J.Val, +origin: U32, +k: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, 9),
    _ => griffinwing.script.Session.victory_gates.go(
      gw.push_prop(s, griffinwing.types.Prop.make(gw.ifs(U32.is_eq(U32.mod(k, 3), 2), _ => "Ring", _ => "Gate"),
        (origin + (k * griffinwing.types.dm(75) : U32) : U32),
        griffinwing.types.tri((k * 40 : U32), 120, griffinwing.types.dm(30)),
        (griffinwing.types.dm(30) + griffinwing.types.tri((k * 25 : U32), 90, griffinwing.types.dm(18)) : U32),
        griffinwing.types.dm(24), 0, 0, 1)),
      origin, (k + 1 : U32)),
    _ => s)

def griffinwing.script.thunderhead?() -> List<&2, J.Val>:
  [
    gw.ev(30, gw.act_say("Hello")),
    gw.ev(110, gw.act_wisps(3, 1, 0, 0, 20, 0)),
    gw.ev(138, gw.act_gate(28050, 4294967270, 30, True{})),
    gw.ev(153, gw.act_gate(29325, 0, 38, True{})),
    gw.ev(168, gw.act_gate(30600, 26, 30, True{})),
    gw.ev(330, gw.act_wisps(5, 2, 1, 0, 25, 0)),
    gw.ev(350, gw.act_say("Hold")),
    gw.ev(508, gw.act_ring(59500, 4294967266, 40)),
    gw.ev(508, gw.act_ring(59500, 30, 40)),
    gw.ev(560, gw.act_say("Ring")),
    gw.ev(560, gw.act_wisps(4, 2, 2, 0, 20, 0)),
    gw.ev(668, gw.act_arch(73100, 0, 20, 24, 15)),
    gw.ev(700, gw.act_say("Arch")),
    gw.ev(800, gw.act_wisps(4, 2, 3, 0, 15, 0)),
    gw.ev(850, gw.act_wisps(4, 2, 3, 0, 30, 0)),
    gw.ev(1010, gw.act_hunter(4294967295)),
    gw.ev(1100, gw.act_wisps(4, 2, 0, 0, 25, 3)),
    gw.ev(1180, gw.act_wisps(6, 2, 4, 0, 20, 1)),
    gw.ev(1240, gw.act_wisps(6, 2, 4, 0, 32, 1)),
    gw.ev(1308, gw.act_gate(127500, 0, 36, True{})),
    gw.ev(1323, gw.act_gate(128775, 30, 15, True{})),
    gw.ev(1338, gw.act_gate(130050, 0, 4294967286, True{})),
    gw.ev(1353, gw.act_gate(131325, 4294967266, 15, True{})),
    gw.ev(1368, gw.act_gate(132600, 0, 36, True{})),
    gw.ev(1380, gw.act_hunter(1)),
    gw.ev(1400, gw.act_wisps(3, 2, 0, 0, 25, 0)),
    gw.ev(1500, gw.act_hunter(4294967295)),
    gw.ev(1508, gw.act_ring(144500, 0, 40)),
    gw.ev(1560, gw.act_wisps(3, 1, 0, 0, 25, 3))
  ]

def griffinwing.script.causeway?() -> List<&2, J.Val>:
  [
    gw.ev(30, gw.act_say("Ruins")),
    gw.ev(80, gw.act_say("Turret")),
    gw.ev(90, gw.act_turret(4294967201, 4294967276)),
    gw.ev(150, gw.act_say("Roll")),
    gw.ev(330, gw.act_turret(95, 4294967286)),
    gw.ev(345, gw.act_turret(4294967196, 0)),
    gw.ev(500, gw.act_say("Convoy")),
    gw.ev(520, gw.act_convoy(6, 2, 1, 25)),
    gw.ev(608, gw.act_gate(68000, 4294967270, 25, True{})),
    gw.ev(620, gw.act_wisps(4, 2, 0, 0, 25, 3)),
    gw.ev(623, gw.act_gate(69275, 0, 36, True{})),
    gw.ev(638, gw.act_gate(70550, 26, 25, True{})),
    gw.ev(708, gw.act_arch(76500, 4294967286, 0, 22, 14)),
    gw.ev(750, gw.act_say("Bombers")),
    gw.ev(760, gw.act_ridge(4294967226, 2, 2)),
    gw.ev(980, gw.act_say("Shutter")),
    gw.ev(998, gw.act_shutter(101150, 0)),
    gw.ev(1150, gw.act_convoy(5, 2, 4294967295, 15)),
    gw.ev(1170, gw.act_turret(95, 4294967276)),
    gw.ev(1198, gw.act_shutter(118150, 75)),
    gw.ev(1300, gw.act_wisps(5, 2, 1, 0, 25, 3)),
    gw.ev(1400, gw.act_ridge(70, 2, 2)),
    gw.ev(1450, gw.act_ridge(4294967226, 1, 2)),
    gw.ev(1558, gw.act_ring(148750, 4294967266, 30)),
    gw.ev(1558, gw.act_ring(148750, 30, 30)),
    gw.ev(1650, gw.act_turret(4294967196, 4294967286)),
    gw.ev(1670, gw.act_turret(100, 4294967281)),
    gw.ev(1780, gw.act_hunter(1)),
    gw.ev(1850, gw.act_hunter(4294967295)),
    gw.ev(1900, gw.act_wisps(4, 2, 0, 0, 28, 3)),
    gw.ev(1990, gw.act_hunter(1)),
    gw.ev(2050, gw.act_hunter(4294967295)),
    gw.ev(2058, gw.act_gate(191250, 0, 36, True{})),
    gw.ev(2073, gw.act_gate(192525, 4294967266, 18, True{})),
    gw.ev(2088, gw.act_gate(193800, 0, 0, True{})),
    gw.ev(2103, gw.act_gate(195075, 30, 18, True{}))
  ]

def griffinwing.script.roost?() -> List<&2, J.Val>:
  [
    gw.ev(30, gw.act_say("Roost")),
    gw.ev(110, gw.act_wisps(4, 2, 0, 0, 25, 3)),
    gw.ev(250, gw.act_hunter(4294967295)),
    gw.ev(280, gw.act_hunter(1)),
    gw.ev(308, gw.act_ring(42500, 0, 30)),
    gw.ev(328, gw.act_gate(44200, 4294967276, 25, True{})),
    gw.ev(480, gw.act_beast())
  ]

def griffinwing.SLOT?() -> U32:
  3

# A run never runs past an hour; a longer one is not a run to play.

def griffinwing.LIMIT?() -> U32:
  (60 * 60 * rules.HZ() : U32)

# A fall puts the beat back this many ticks after the last knight goes down.

def griffinwing.RETRY?() -> U32:
  24

def griffinwing.DYING?() -> U32:
  60

def griffinwing.VICTORY?() -> U32:
  210

def griffinwing.TALLY_HOLD?() -> U32:
  45

# A knight who falls rises again this many ticks later, at the formation.

def griffinwing.RISE?() -> U32:
  75

def griffinwing.PURSE?() -> U32:
  10

def griffinwing.PURSE_MEDAL?() -> U32:
  5

def griffinwing.REPLAY?() -> U32:
  5

def griffinwing.REPLAY_MEDAL?() -> U32:
  2

def griffinwing.BEGUN?() -> U32:
  1

def griffinwing.GOLD?() -> U32:
  2

def griffinwing.CAMPAIGN_MARK?() -> String:
  "griffinwing:campaign-begun"

def griffinwing.GOLD_MARK?() -> String:
  "griffinwing:gold-medal"

# A seat's bit.

def griffinwing.bit?(+id: U32) -> U32:
  U32.shln(1, U32.to_nat((RS.u_clamp(id, 1, 8) - 1 : U32)))

# ─── the five numbers ───────────────────────────────────────────────────────

# `Nums::default()`: where the knobs start.

def griffinwing.Nums.default?() -> J.Val:
  griffinwing.Nums.make(40, 30, 64, 75, 50)

def griffinwing.Nums.make?(+charge: U32, +roll: U32, +window: U32, +spur: U32, +tell: U32) -> J.Val:
  +n0 = J.obj()
  +n1 = ga.su(n0, "charge", charge)
  +n2 = ga.su(n1, "roll", roll)
  +n3 = ga.su(n2, "window", window)
  +n4 = ga.su(n3, "spur", spur)
  ga.su(n4, "tell", tell)

# `Nums::of(&tune)`: `Tune::roundtable[SLOT]`, each clamped to its bounds.

def griffinwing.Nums.of?(+tune: J.Val) -> J.Val:
  griffinwing.Nums.of.row(J.nth(J.get(tune, "roundtable"), griffinwing.SLOT()))

def griffinwing.Nums.of.row?(+row: J.Val) -> J.Val:
  griffinwing.Nums.make(
    RS.u_clamp(ga.nu(row, 0), 20, 90),
    RS.u_clamp(ga.nu(row, 1), 18, 60),
    RS.u_clamp(ga.nu(row, 2), 40, 90),
    RS.u_clamp(ga.nu(row, 3), 30, 180),
    RS.u_clamp(ga.nu(row, 4), 40, 120))

# The steer box's half width and half height.

def griffinwing.Nums.wx?(+n: J.Val) -> U32:
  griffinwing.types.dm(ga.u(n, "window"))

def griffinwing.Nums.wz?(+n: J.Val) -> U32:
  RS.i_div((griffinwing.types.dm(ga.u(n, "window")) * 5 : U32), 8)

# How much of a roll reflects: all but its last ten ticks.

def griffinwing.Nums.reflect?(+n: J.Val) -> U32:
  U32.max(ga.ssub(ga.u(n, "roll"), 10), 8)

# ─── the session ────────────────────────────────────────────────────────────

# The rail's point the formation flies at now.

def griffinwing.Session.anchor?(+s: J.Val) -> U32:
  (griffinwing.rail.ORIGIN(U32.min(ga.u(s, "stage"), 2)) + (ga.u(s, "t") * griffinwing.types.PACE() : U32) : U32)

# How many knights fly: the pressure on the foes.

def griffinwing.Session.pressure?(+s: J.Val) -> U32:
  RS.u_clamp(gw.nseats(s), 1, rules.MAX_PLAYERS())

# A fresh id for a foe.

def griffinwing.Session.next_id?(+s: J.Val) -> J.Val:
  gw.out(
    ga.su(s, "ids", Bool.pick(U32, U32.is_ge(ga.u(s, "ids"), 60000), 1, (ga.u(s, "ids") + 1 : U32))),
    J.JU{ga.u(s, "ids")})

# `snd.contains(name)`.

def gw.has_str?(+xs: List<&2, J.Val>, +name: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(String.eq(J.str(h), name), gw.has_str(t, name))

def griffinwing.Session.sound?(+s: J.Val, +name: String) -> J.Val:
  gw.ifj(Bool.and(U32.is_lt(gw.len(s, "snd"), 12), Bool.not(gw.has_str(ga.items(s, "snd"), name))),
    _ => ga.push(s, "snd", J.JS{name}),
    _ => s)

# `calls.iter().any(|c| c.line == line && c.ticks > 40)`.

def griffinwing.Session.said?(+cs: List<&2, J.Val>, +line: String) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case Con{+c, +t}:
      Bool.or(Bool.and(String.eq(ga.s(c, "line"), line), U32.is_gt(ga.u(c, "ticks"), 40)), griffinwing.Session.said(t, line))

# A rider's short shout, shown as a portrait for a moment. The same line is
# not said twice in a row, and four show at most.

def griffinwing.Session.say?(+s: J.Val, +who: U32, +line: String) -> J.Val:
  gw.ifj(griffinwing.Session.said(ga.items(s, "calls"), line),
    _ => s,
    _ => griffinwing.Session.say.push(s, who, line))

def griffinwing.Session.say.push?(+s: J.Val, +who: U32, +line: String) -> J.Val:
  ga.push(
    gw.ifj(U32.is_ge(gw.len(s, "calls"), griffinwing.types.MAX_CALLS()), _ => gw.remove(s, "calls", 0), _ => s),
    "calls", griffinwing.Session.Call.make(who, line, 100))

def griffinwing.Session.Call.make?(+who: U32, +line: String, +ticks: U32) -> J.Val:
  +c0 = J.obj()
  +c1 = ga.su(c0, "who", who)
  +c2 = ga.ss(c1, "line", line)
  ga.su(c2, "ticks", ticks)

def griffinwing.Session.spark?(+s: J.Val, +what: String, +at: U32, +x: U32, +z: U32, +n: U32) -> J.Val:
  griffinwing.Session.spark.push(
    gw.ifj(U32.is_ge(gw.len(s, "fx"), griffinwing.types.MAX_FX()), _ => gw.remove(s, "fx", 0), _ => s),
    what, at, x, z, n)

def griffinwing.Session.spark.push?(+s: J.Val, +what: String, +at: U32, +x: U32, +z: U32, +n: U32) -> J.Val:
  ga.push(s, "fx", griffinwing.Session.Flare.make(what, at, x, z, griffinwing.Session.spark.ticks(what), n))

def griffinwing.Session.spark.ticks?(+what: String) -> U32:
  Bool.pick(U32, String.eq(what, "Starfall"), 36,
    Bool.pick(U32, String.eq(what, "Fall"), 40,
      Bool.pick(U32, String.eq(what, "Lock"), 8, 14)))

def griffinwing.Session.Flare.make?(+what: String, +at: U32, +x: U32, +z: U32, +ticks: U32, +n: U32) -> J.Val:
  +f0 = J.obj()
  +f1 = ga.ss(f0, "what", what)
  +f2 = ga.si(f1, "s", at)
  +f3 = ga.si(f2, "x", x)
  +f4 = ga.si(f3, "z", z)
  +f5 = ga.su(f4, "ticks", ticks)
  ga.su(f5, "n", n)

# Who calls the hints: the first of the riders still flying, and when a full
# table leaves none (or all are lost), Aster, the wing's lead.

def griffinwing.Session.voice?(+s: J.Val) -> U32:
  griffinwing.Session.voice.go(ga.items(s, "wing"))

def griffinwing.Session.voice.go?(+ws: List<&2, J.Val>) -> U32:
  match ws:
    case Nil{}:
      7
    case Con{+w, +t}:
      Bool.pick(U32, ga.b(w, "lost"), griffinwing.Session.voice.go(t), ga.u(w, "id"))

# Fill the wing up to four with riders: the seats first, riders in the seats
# nobody sits in.

def griffinwing.Session.fill_wing?(+s: J.Val) -> J.Val:
  griffinwing.Session.fill_wing.add(
    griffinwing.Session.fill_wing.pop(s, ga.ssub(griffinwing.types.WING(), gw.nseats(s))),
    ga.ssub(griffinwing.types.WING(), gw.nseats(s)))

# while wing.len() > want { wing.pop() }

def griffinwing.Session.fill_wing.pop?(+s: J.Val, +want: U32) -> J.Val:
  gw.ifj(U32.is_gt(gw.len(s, "wing"), want),
    _ => griffinwing.Session.fill_wing.pop(gw.trunc(s, "wing", (gw.len(s, "wing") - 1 : U32)), want),
    _ => s)

# while wing.len() < want { push a rider }

def griffinwing.Session.fill_wing.add?(+s: J.Val, +want: U32) -> J.Val:
  gw.ifj(U32.is_lt(gw.len(s, "wing"), want),
    _ => griffinwing.Session.fill_wing.add(ga.push(s, "wing", griffinwing.Session.Wingman.make(gw.len(s, "wing"))), want),
    _ => s)

def griffinwing.Session.Wingman.make?(+k: U32) -> J.Val:
  griffinwing.Session.Wingman.of(k, griffinwing.flight.rider_slot.x(k), griffinwing.flight.rider_slot.z(k), griffinwing.flight.rider_slot.back(k))

def griffinwing.Session.Wingman.of?(+k: U32, +x: U32, +z: U32, +back: U32) -> J.Val:
  +w0 = J.obj()
  +w1 = ga.su(w0, "id", (7 + k : U32))
  +w2 = ga.sv(w1, "craft", ga.si(griffinwing.types.Craft.new(x, z), "back", back))
  +w3 = ga.sb(w2, "lost", False{})
  ga.su(w3, "fire", (12 * k : U32))

# Seat a knight at the formation, unhurt for a moment.

def griffinwing.Session.sit?(+s: J.Val, +id: U32, +joined: U32) -> J.Val:
  gw.ifj(Bool.or(Bool.not(U32.is_eq(RS.u_clamp(id, 1, rules.MAX_PLAYERS()), id)), ga.has_key(J.get(s, "seats"), id)),
    _ => s,
    _ => griffinwing.Session.sit.in(s, id, joined))

def griffinwing.Session.sit.in?(+s: J.Val, +id: U32, +joined: U32) -> J.Val:
  ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id,
    griffinwing.Session.Seat.make(joined,
      ga.su(griffinwing.types.Craft.new(griffinwing.flight.slot.x(id), griffinwing.flight.slot.z(id)), "hurt", (2 * rules.HZ() : U32)))))

def griffinwing.Session.Seat.make?(+joined: U32, +craft: J.Val) -> J.Val:
  ga.sv(ga.su(J.obj(), "joined", joined), "craft", craft)

# The kingdom tile its door stands on: walk up and press E.

def griffinwing.ENTRY?() -> J.Val:
  ga.pair_i(105, 9)

# Every mark this adventure may write.

def griffinwing.MARKS?() -> J.Val:
  J.JA{[J.JS{griffinwing.CAMPAIGN_MARK()}, J.JS{griffinwing.GOLD_MARK()}]}

# The five numbers (charge, roll, window, spur, tell) as the knob panel shows
# them: {name, label, constant, min, max, start}.

def griffinwing.NUMBERS?() -> J.Val:
  J.JA{[
    griffinwing.NUMBERS.row("griffinwing_lock", "Charge ticks", "griffinwing::NUMBERS[0]", 20, 90, 40),
    griffinwing.NUMBERS.row("griffinwing_roll", "Roll ticks", "griffinwing::NUMBERS[1]", 18, 60, 30),
    griffinwing.NUMBERS.row("griffinwing_window", "Steer box", "griffinwing::NUMBERS[2]", 40, 90, 64),
    griffinwing.NUMBERS.row("griffinwing_spur", "Spur ticks", "griffinwing::NUMBERS[3]", 30, 180, 75),
    griffinwing.NUMBERS.row("griffinwing_tell", "Tell ticks", "griffinwing::NUMBERS[4]", 40, 120, 50)]}

def griffinwing.NUMBERS.row?(+name: String, +label: String, +constant: String, +min: U32, +max: U32, +start: U32) -> J.Val:
  +n0 = ga.ss(J.obj(), "name", name)
  +n1 = ga.ss(n0, "label", label)
  +n2 = ga.ss(n1, "constant", constant)
  +n3 = ga.su(n2, "min", min)
  +n4 = ga.su(n3, "max", max)
  ga.su(n4, "start", start)

def griffinwing.Session.Mode.is?(+s: J.Val, +name: String) -> Bool:
  gw.isn(s, "mode", name)

# A fresh session for the knights at the table.

def griffinwing.Session.new?(+players: J.Val) -> J.Val:
  griffinwing.Session.fill_wing(
    griffinwing.Session.new.sit(griffinwing.Session.new.empty(), players, ga.ids(players)))

def griffinwing.Session.new.empty?() -> J.Val:
  +s0 = ga.sv(J.obj(), "seats", J.obj())
  +s1 = ga.su(s0, "elapsed", 0)
  +s2 = ga.ss(s1, "mode", "Play")
  +s3 = ga.su(s2, "mode_t", 0)
  +s4 = ga.su(s3, "stage", 0)
  +s5 = ga.su(s4, "t", 0)
  +s6 = ga.su(s5, "cp", 0)
  +s7 = ga.su(s6, "next", 0)
  +s8 = ga.sv(s7, "foes", J.JA{[]})
  +s9 = ga.sv(s8, "props", J.JA{[]})
  +s10 = ga.sv(s9, "bolts", J.JA{[]})
  +s11 = ga.sv(s10, "wing", J.JA{[]})
  +s12 = ga.sv(s11, "boss", J.JNull{})
  +s13 = ga.sv(s12, "calls", J.JA{[]})
  +s14 = ga.sv(s13, "fx", J.JA{[]})
  +s15 = ga.su(s14, "ids", 1)
  +s16 = ga.sv(s15, "bank", griffinwing.types.Tally.default())
  +s17 = ga.sv(s16, "beat", griffinwing.types.Tally.default())
  +s18 = ga.su(s17, "retries", 0)
  +s19 = ga.sv(s18, "medal", J.JNull{})
  +s20 = ga.su(s19, "paid", 0)
  +s21 = ga.sb(s20, "settled", False{})
  +s22 = ga.sv(s21, "done", J.JNull{})
  +s23 = ga.si(s22, "home", 0)
  +s24 = ga.sb(s23, "first", True{})
  +s25 = ga.sv(s24, "snd", J.JA{[]})
  ga.sv(s25, "leave", J.JA{[]})

# `for (&id, _) in players.iter().filter(|(_, h)| !h.away && !h.stone) { s.sit(id, 0) }`

def griffinwing.Session.new.sit?(+s: J.Val, +players: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.new.sit(
        gw.ifj(griffinwing.Session.here(ga.at(players, id)), _ => griffinwing.Session.sit(s, id, 0), _ => s),
        players, t)

# `!h.away && !h.stone`

def griffinwing.Session.here?(+h: J.Val) -> Bool:
  Bool.and(Bool.not(ga.b(h, "away")), Bool.not(ga.b(h, "stone")))

# Who is at the table now (`present`), seated and stood up, and the foes'
# health provisioned for the seats now flying: a knight who sits down scales
# the beat up by its share, and a seat already provisioned does not scale twice.

def griffinwing.Session.muster?(+s0: J.Val, +heroes: J.Val) -> J.Val:
  griffinwing.Session.fill_wing(
    griffinwing.Session.muster.foes(
      griffinwing.Session.muster.sit(
        ga.sv(s0, "seats", griffinwing.Session.muster.keep(J.get(s0, "seats"), heroes, ga.ids(J.get(s0, "seats")))),
        heroes, ga.ids(heroes)),
      0))

# seats.retain(|id, _| heroes.get(id).is_some_and(|h| !h.away && !h.stone))

def griffinwing.Session.muster.keep?(+seats: J.Val, +heroes: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      seats
    case Con{+id, +t}:
      griffinwing.Session.muster.keep(
        gw.ifj(Bool.and(ga.has_key(heroes, id), griffinwing.Session.here(ga.at(heroes, id))), _ => seats, _ => J.del(seats, U32.show(id))),
        heroes, t)

# for id in here { if !seats.contains(id) && (1..=MAX_PLAYERS).contains(id) { sit; join call } }

def griffinwing.Session.muster.sit?(+s: J.Val, +heroes: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.muster.sit(
        gw.ifj(Bool.and(griffinwing.Session.here(ga.at(heroes, id)),
            Bool.and(Bool.not(ga.has_key(J.get(s, "seats"), id)), gw.within(id, 1, rules.MAX_PLAYERS()))),
          _ => griffinwing.Session.muster.join(griffinwing.Session.sit(s, id, ga.u(s, "elapsed")), id),
          _ => s),
        heroes, t)

def griffinwing.Session.muster.join?(+s: J.Val, +id: U32) -> J.Val:
  gw.ifj(U32.is_gt(ga.u(s, "elapsed"), 0),
    _ => griffinwing.Session.sound(griffinwing.Session.say(s, id, "Join"), "play_card"),
    _ => s)

# for f in foes { if f.seats < now { hp = hp * now / was; full = full * now / was; seats = now } }

def griffinwing.Session.muster.foes?(+s: J.Val, +i: U32) -> J.Val:
  gw.ifj(U32.is_lt(i, gw.len(s, "foes")),
    _ => griffinwing.Session.muster.foes(
      gw.put_item(s, "foes", i, griffinwing.Session.muster.foe(gw.item(s, "foes", i), griffinwing.Session.pressure(s))),
      (i + 1 : U32)),
    _ => s)

def griffinwing.Session.muster.foe?(+f: J.Val, +now: U32) -> J.Val:
  gw.ifj(U32.is_lt(ga.u(f, "seats"), now),
    _ => griffinwing.Session.muster.foe2(f, now, U32.max(ga.u(f, "seats"), 1)),
    _ => f)

def griffinwing.Session.muster.foe2?(+f: J.Val, +now: U32, +was: U32) -> J.Val:
  ga.su(ga.su(ga.su(f, "hp", U32.div((ga.u(f, "hp") * now : U32), was)), "full", U32.div((ga.u(f, "full") * now : U32), was)), "seats", now)

# The keys of knight `id`: what was pressed if it is a valid Input, else nothing.

def griffinwing.Session.keys?(+inputs: J.Val, +id: U32) -> J.Val:
  griffinwing.Session.keys.of(J.get(inputs, U32.show(id)))

def griffinwing.Session.keys.of?(+i: J.Val) -> J.Val:
  gw.ifj(Bool.and(Bool.not(J.is_null(i)), rules.Input.valid(i)), _ => i, _ => rules.Input.default())

# One tick: the seats are whoever is here and awake, so a knight who leaves
# stands up and one who joins late sits down; then the beat. Answers
# {"s", "out": {"sounds", "leave"}}.

def griffinwing.Session.step?(+s0: J.Val, +inputs: J.Val, +heroes: J.Val, +n: J.Val) -> J.Val:
  griffinwing.Session.step.mode(
    griffinwing.Session.muster(ga.su(s0, "elapsed", U32.min((ga.u(s0, "elapsed") + 1 : U32), griffinwing.LIMIT())), heroes),
    inputs, n)

def griffinwing.Session.step.mode?(+s: J.Val, +inputs: J.Val, +n: J.Val) -> J.Val:
  griffinwing.Session.step.done(
    gw.ifj(griffinwing.Session.Mode.is(s, "Play"),
      _ => griffinwing.Session.step.play(s, inputs, n, ga.ids(J.get(s, "seats"))),
      _ => gw.ifj(griffinwing.Session.Mode.is(s, "Retry"),
        _ => griffinwing.Session.step.retry(s),
        _ => griffinwing.Session.step.fly(s, inputs, n, ga.ids(J.get(s, "seats"))))),
    inputs)

# for id in live { self.fly(id, keys(id), nums) }

def griffinwing.Session.step.flies?(+s: J.Val, +inputs: J.Val, +n: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.step.flies(griffinwing.flight.Session.fly(s, id, griffinwing.Session.keys(inputs, id), n), inputs, n, t)

def griffinwing.Session.step.play?(+s0: J.Val, +inputs: J.Val, +n: J.Val, +live: List<&2, U32>) -> J.Val:
  griffinwing.Session.end_beat(
    griffinwing.Session.tick_lists(
      griffinwing.flight.Session.step_craft(
        griffinwing.things.Session.step_props(
          griffinwing.combat.Session.step_bolts(
            griffinwing.boss.Session.step_boss(
              griffinwing.combat.Session.step_foes(
                griffinwing.script.Session.run_script(
                  griffinwing.flight.Session.wing_fly(
                    griffinwing.Session.step.flies(ga.su(s0, "t", U32.min((ga.u(s0, "t") + 1 : U32), griffinwing.LIMIT())), inputs, n, live),
                    n),
                  n),
                n),
              n),
            n),
          n),
        n)))

def griffinwing.Session.step.retry?(+s0: J.Val) -> J.Val:
  griffinwing.Session.step.retry2(ga.su(griffinwing.Session.tick_lists(s0), "mode_t", ga.ssub(ga.u(s0, "mode_t"), 1)))

def griffinwing.Session.step.retry2?(+s: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(s, "mode_t")), _ => griffinwing.Session.restart(s), _ => s)

# The rail carries them home; at the tally the wing hovers.

def griffinwing.Session.step.fly?(+s0: J.Val, +inputs: J.Val, +n: J.Val, +live: List<&2, U32>) -> J.Val:
  griffinwing.Session.step.fly2(
    griffinwing.Session.tick_lists(
      griffinwing.flight.Session.step_craft(
        griffinwing.things.Session.step_props(
          griffinwing.combat.Session.step_bolts(
            griffinwing.flight.Session.wing_fly(
              griffinwing.Session.step.flies(
                gw.ifj(griffinwing.Session.Mode.is(s0, "Victory"), _ => ga.su(s0, "t", U32.min((ga.u(s0, "t") + 1 : U32), griffinwing.LIMIT())), _ => s0),
                inputs, n, live),
              n),
            n),
          n),
        n)),
    inputs, live)

def griffinwing.Session.step.fly2?(+s0: J.Val, +inputs: J.Val, +live: List<&2, U32>) -> J.Val:
  griffinwing.Session.step.fly3(ga.su(s0, "mode_t", U32.min((ga.u(s0, "mode_t") + 1 : U32), griffinwing.LIMIT())), inputs, live)

def griffinwing.Session.step.fly3?(+s: J.Val, +inputs: J.Val, +live: List<&2, U32>) -> J.Val:
  gw.ifj(Bool.and(griffinwing.Session.Mode.is(s, "Victory"), U32.is_ge(ga.u(s, "mode_t"), griffinwing.VICTORY())),
    _ => griffinwing.Session.finish(s),
    _ => gw.ifj(Bool.and(griffinwing.Session.Mode.is(s, "Tally"), U32.is_gt(ga.u(s, "mode_t"), griffinwing.TALLY_HOLD())),
      _ => griffinwing.Session.step.leaves(s, inputs, live),
      _ => s))

# for id in live { fresh = spur key not held last tick; if keys(id).interact && fresh { leave.push(id) } }

def griffinwing.Session.step.leaves?(+s: J.Val, +inputs: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.step.leaves(
        gw.ifj(Bool.and(
            ga.b(griffinwing.Session.keys(inputs, id), "interact"),
            Bool.and(gw.seat_has(s, id), U32.is_zero(U32.and(ga.u(gw.craft_of(s, id), "keys"), griffinwing.flight.K_SPUR())))),
          _ => ga.push(s, "leave", J.JU{id}),
          _ => s),
        inputs, t)

# The keys of this tick stand for the next one's presses; the queues are drained.

def griffinwing.Session.step.done?(+s: J.Val, +inputs: J.Val) -> J.Val:
  griffinwing.Session.step.out(griffinwing.Session.step.keys(s, inputs, ga.ids(J.get(s, "seats"))))

def griffinwing.Session.step.keys?(+s: J.Val, +inputs: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.step.keys(
        gw.put_craft(s, id, ga.su(gw.craft_of(s, id), "keys", griffinwing.flight.bits(griffinwing.Session.keys(inputs, id)))),
        inputs, t)

def griffinwing.Session.step.out?(+s: J.Val) -> J.Val:
  ga.sv(
    ga.sv(J.obj(), "s", ga.sv(ga.sv(s, "snd", J.JA{[]}), "leave", J.JA{[]})),
    "out", ga.sv(ga.sv(J.obj(), "sounds", J.get(s, "snd")), "leave", J.get(s, "leave")))

# The calls, flashes and counters wind down.

def griffinwing.Session.tick_lists?(+s: J.Val) -> J.Val:
  ga.sv(
    ga.sv(
      ga.sv(s, "calls", J.JA{griffinwing.Session.tick_lists.wind(ga.items(s, "calls"), False{})}),
      "fx", J.JA{griffinwing.Session.tick_lists.wind(ga.items(s, "fx"), False{})}),
    "foes", J.JA{griffinwing.Session.tick_lists.flash(ga.items(s, "foes"))})

# Each item's ticks one less; those at nothing are dropped.

def griffinwing.Session.tick_lists.wind?(+xs: List<&2, J.Val>, +unused: Bool) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+x, +t}:
      gw.ifjl(U32.is_gt(ga.ssub(ga.u(x, "ticks"), 1), 0),
        _ => Con{ga.su(x, "ticks", ga.ssub(ga.u(x, "ticks"), 1)), griffinwing.Session.tick_lists.wind(t, False{})},
        _ => griffinwing.Session.tick_lists.wind(t, False{}))

def griffinwing.Session.tick_lists.flash?(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Con{ga.su(f, "flash", ga.ssub(ga.u(f, "flash"), 1)), griffinwing.Session.tick_lists.flash(t)}

# `!seats.is_empty() && seats.values().all(|s| s.craft.down > 0)`

def griffinwing.Session.wiped?(+s: J.Val, +ids: List<&2, U32>, +any: Bool) -> Bool:
  match ids:
    case Nil{}:
      any
    case Con{+id, +t}:
      gw.ifb(U32.is_gt(ga.u(gw.craft_of(s, id), "down"), 0), _ => griffinwing.Session.wiped(s, t, True{}), _ => False{})

# The beat's end, a wipe, or the Wyrm's fall.

def griffinwing.Session.end_beat?(+s: J.Val) -> J.Val:
  gw.ifj(griffinwing.Session.wiped(s, ga.ids(J.get(s, "seats")), False{}),
    _ => griffinwing.Session.sound(ga.su(ga.ss(s, "mode", "Retry"), "mode_t", griffinwing.RETRY()), "knight_fall"),
    _ => gw.ifj(Bool.and(U32.is_lt(ga.u(s, "stage"), 2), U32.is_ge(ga.u(s, "t"), griffinwing.rail.LEN(ga.u(s, "stage")))),
      _ => griffinwing.Session.next_beat(s),
      _ => gw.ifj(Bool.and(U32.is_eq(ga.u(s, "stage"), 2), griffinwing.Session.dying(J.get(s, "boss"))),
        _ => griffinwing.Session.victory(s),
        _ => s)))

# `boss.is_some_and(|b| b.st == Dying && b.ticks == 0)`

def griffinwing.Session.dying?(+b: J.Val) -> Bool:
  Bool.and(Bool.not(J.is_null(b)), Bool.and(gw.isn(b, "st", "Dying"), U32.is_zero(ga.u(b, "ticks"))))

# The Wyrm is down: the warm fly-through.

def griffinwing.Session.victory?(+s0: J.Val) -> J.Val:
  griffinwing.script.Session.victory_gates(
    griffinwing.Session.sound(
      griffinwing.Session.say(
        ga.sv(ga.sv(
          griffinwing.Session.victory.beat(
            ga.su(ga.sv(ga.ss(s0, "mode", "Victory"), "done", J.JU{ga.u(s0, "elapsed")}), "mode_t", 0)),
          "foes", J.JA{[]}), "bolts", J.JA{[]}),
        griffinwing.Session.voice(s0), "Home"),
      "boss_death"))

def griffinwing.Session.victory.beat?(+s: J.Val) -> J.Val:
  ga.si(
    ga.sv(s, "beat", gw.addu(gw.addu(gw.addu(J.get(s, "beat"), "pts", 1000), "base", 1000), "offered", 1000)),
    "home", griffinwing.Session.anchor(s))

# The next beat flies on from where this one ends, the party mended a little and
# a bomb richer; what the beat made is banked.

def griffinwing.Session.next_beat?(+s0: J.Val) -> J.Val:
  griffinwing.Session.next_beat.seats(
    griffinwing.Session.sound(
      griffinwing.Session.say(
        ga.su(ga.su(ga.su(ga.su(
          ga.sv(ga.sv(s0, "bank", griffinwing.types.Tally.bank(J.get(s0, "bank"), J.get(s0, "beat"))), "beat", griffinwing.types.Tally.default()),
          "stage", (ga.u(s0, "stage") + 1 : U32)), "t", 0), "cp", 0), "next", 0),
        griffinwing.Session.voice(s0), gw.ifs(U32.is_eq((ga.u(s0, "stage") + 1 : U32), 1), _ => "Ruins", _ => "Roost")),
      "play_card"),
    ga.ids(J.get(s0, "seats")), 0)

# seats: shield +40, a bomb, the meter full; the wing mended and back.

def griffinwing.Session.next_beat.seats?(+s: J.Val, +ids: List<&2, U32>, +k: U32) -> J.Val:
  match ids:
    case Nil{}:
      griffinwing.Session.next_beat.wing(s, 0)
    case Con{+id, +t}:
      griffinwing.Session.next_beat.seats(
        gw.put_craft(s, id,
          ga.sb(ga.si(ga.su(ga.si(gw.craft_of(s, id), "shield", gw.min((ga.u(gw.craft_of(s, id), "shield") + 40 : U32), griffinwing.types.SHIELD())),
              "bombs", U32.min((ga.u(gw.craft_of(s, id), "bombs") + 1 : U32), 3)), "meter", griffinwing.types.METER()), "spent", False{})),
        t, k)

def griffinwing.Session.next_beat.wing?(+s: J.Val, +k: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, gw.len(s, "wing")),
    _ => griffinwing.Session.next_beat.wing(
      gw.put_item(s, "wing", k,
        ga.sv(ga.sb(gw.item(s, "wing", k), "lost", False{}), "craft",
          ga.si(J.get(gw.item(s, "wing", k), "craft"), "shield", gw.min((ga.u(J.get(gw.item(s, "wing", k), "craft"), "shield") + 40 : U32), griffinwing.types.SHIELD())))),
      (k + 1 : U32)),
    _ => s)

# A beat starts over from its checkpoint: the foes the beat had are gone and its
# script runs again from there; what was banked stays, as do the Wyrm's lamps
# already out. Everyone is up, mended and unhurt for a moment.

def griffinwing.Session.restart?(+s0: J.Val) -> J.Val:
  griffinwing.Session.restart.seats(
    griffinwing.Session.restart.clear(s0), ga.ids(J.get(s0, "seats")))

def griffinwing.Session.restart.clear?(+s0: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(
    ga.sv(
      ga.su(ga.su(ga.su(s0, "retries", U32.min((ga.u(s0, "retries") + 1 : U32), 255)), "t", ga.u(s0, "cp")), "next", griffinwing.script.resume(ga.u(s0, "stage"), ga.u(s0, "cp"))),
      "beat", griffinwing.types.Tally.default()),
    "props", J.JA{[]}), "bolts", J.JA{[]}), "fx", J.JA{[]}), "calls", J.JA{[]}),
    "foes", J.JA{griffinwing.Session.restart.lamps(ga.items(s0, "foes"))})

# foes.retain(|f| f.kind == Lamp)

def griffinwing.Session.restart.lamps?(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      gw.ifjl(gw.isn(f, "kind", "Lamp"), _ => Con{f, griffinwing.Session.restart.lamps(t)}, _ => griffinwing.Session.restart.lamps(t))

def griffinwing.Session.restart.seats?(+s: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      griffinwing.Session.restart.fin(griffinwing.Session.restart.wing(griffinwing.Session.restart.boss(s), 0))
    case Con{+id, +t}:
      griffinwing.Session.restart.seats(
        gw.put_craft(s, id, griffinwing.Session.restart.craft(gw.craft_of(s, id), id)),
        t)

# A new craft at the slot, with the progress kept and at least two bombs.

def griffinwing.Session.restart.craft?(+old: J.Val, +id: U32) -> J.Val:
  ga.su(ga.su(ga.su(ga.su(ga.su(
    ga.su(griffinwing.types.Craft.new(griffinwing.flight.slot.x(id), griffinwing.flight.slot.z(id)), "hurt", (2 * rules.HZ() : U32)),
    "bombs", U32.max(ga.u(old, "bombs"), 2)),
    "pts", ga.u(old, "pts")), "hits", ga.u(old, "hits")), "saved", ga.u(old, "saved")), "gates", ga.u(old, "gates"))

def griffinwing.Session.restart.boss?(+s: J.Val) -> J.Val:
  gw.ifj(J.is_null(J.get(s, "boss")),
    _ => s,
    _ => ga.sv(s, "boss", ga.sv(ga.su(gw.sets(J.get(s, "boss"), "st", "Idle"), "ticks", (2 * rules.HZ() : U32)), "marks", J.JA{[]})))

def griffinwing.Session.restart.wing?(+s: J.Val, +k: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, gw.len(s, "wing")),
    _ => griffinwing.Session.restart.wing(
      gw.put_item(s, "wing", k,
        ga.sv(ga.sb(gw.item(s, "wing", k), "lost", False{}), "craft",
          ga.su(ga.si(J.get(gw.item(s, "wing", k), "craft"), "shield", griffinwing.types.SHIELD()), "down", 0))),
      (k + 1 : U32)),
    _ => s)

def griffinwing.Session.restart.fin?(+s: J.Val) -> J.Val:
  griffinwing.Session.say(ga.su(ga.ss(s, "mode", "Play"), "mode_t", 0), griffinwing.Session.voice(s), "Hello")

# The Wyrm is down and the fly-through is over: the medal, and the tally.

def griffinwing.Session.finish?(+s0: J.Val) -> J.Val:
  griffinwing.Session.sound(
    ga.su(ga.ss(
      ga.sv(ga.sv(
        ga.sv(s0, "bank", griffinwing.types.Tally.bank(J.get(s0, "bank"), J.get(s0, "beat"))),
        "beat", griffinwing.types.Tally.default()),
        "medal", J.JS{griffinwing.Session.medal_now(ga.sv(s0, "bank", griffinwing.types.Tally.bank(J.get(s0, "bank"), J.get(s0, "beat"))))}),
      "mode", "Tally"), "mode_t", 0),
    "play_card")

# The medal the party has earned: the share of what the beats offered that they
# took, less a little for every fall. A string: "Bronze", "Silver" or "Gold".

def griffinwing.Session.medal_now?(+s: J.Val) -> String:
  griffinwing.Session.medal_now.pct(
    (griffinwing.Session.medal_now.share(ga.u(J.get(s, "bank"), "base"), ga.u(J.get(s, "bank"), "offered")) - (5 * ga.u(s, "retries") : U32) - (4 * ga.u(J.get(s, "bank"), "lost") : U32) : U32))

def griffinwing.Session.medal_now.share?(+base: U32, +offered: U32) -> U32:
  W.lo32(W.div(W.mul(W.of_u32(base), W.of_u32(100)), W.of_u32(U32.max(offered, 1))))

def griffinwing.Session.medal_now.pct?(+pct: U32) -> String:
  gw.ifs(gw.ge(pct, 85), _ => "Gold", _ => gw.ifs(gw.ge(pct, 60), _ => "Silver", _ => "Bronze"))

# What the finish pays each knight: a purse and a share for the medal, less for
# a party that has lit the beacon before.

def griffinwing.Session.purse?(+s: J.Val) -> U32:
  griffinwing.Session.purse.of(ga.b(s, "first"), gw.ifu(J.is_null(J.get(s, "medal")), _ => 0, _ => griffinwing.types.Medal.rank(ga.s(s, "medal"))))

def griffinwing.Session.purse.of?(+first: Bool, +rank: U32) -> U32:
  gw.ifu(first,
    _ => (griffinwing.PURSE() + (griffinwing.PURSE_MEDAL() * rank : U32) : U32),
    _ => (griffinwing.REPLAY() + (griffinwing.REPLAY_MEDAL() * rank : U32) : U32))

# Whether the Wyrm has fallen and the run is over.

def griffinwing.Session.finished?(+s: J.Val) -> Bool:
  griffinwing.Session.Mode.is(s, "Tally")

# The share of what was offered that was taken, in tenths of a percent.

def griffinwing.Session.permille?(+s: J.Val) -> U32:
  U32.min(W.lo32(W.div(W.mul(W.of_u32(ga.u(J.get(s, "bank"), "base")), W.of_u32(1000)), W.of_u32(U32.max(ga.u(J.get(s, "bank"), "offered"), 1)))), 1000)

def griffinwing.things.SHUTTER_PERIOD?() -> U32:
  150

def griffinwing.things.SHUTTER_MIN?() -> U32:
  griffinwing.types.dm(5)

# What a scrape of rock takes off a shield, and what a hearth ring mends.

def griffinwing.things.SCRAPE?() -> U32:
  15

def griffinwing.things.MEND?() -> U32:
  50

# A bonus gate's points, doubled for a gate flown on the spur.

def griffinwing.things.GATE?() -> U32:
  100

# A shutter's half gap at tick `t`: open and closing and opening again over its
# cycle, the way it shows from the far end of the rail.

def griffinwing.things.shutter_gap?(+p: J.Val, +t: U32, +wx: U32) -> U32:
  griffinwing.things.shutter_gap.k(p, wx,
    (griffinwing.types.tri((t + ga.u(p, "phase") : U32), griffinwing.things.SHUTTER_PERIOD(), 1000) + 1000 : U32))

def griffinwing.things.shutter_gap.k?(+p: J.Val, +wx: U32, +k: U32) -> U32:
  (griffinwing.things.SHUTTER_MIN() + gw.div(((wx + griffinwing.types.dm(10) - griffinwing.things.SHUTTER_MIN() : U32) * k : U32), 2000) : U32)

# Props and the knights who cross them.

def griffinwing.things.Session.step_props?(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.things.Session.step_props.retain(
    griffinwing.things.Session.step_props.ids(s, ga.ids(J.get(s, "seats")), griffinwing.Session.anchor(s), ga.u(s, "t"), griffinwing.Nums.wx(n)),
    griffinwing.Session.anchor(s))

def griffinwing.things.Session.step_props.retain?(+s: J.Val, +anchor: U32) -> J.Val:
  ga.sv(s, "props", J.JA{griffinwing.things.Session.step_props.keep(ga.items(s, "props"), anchor)})

def griffinwing.things.Session.step_props.keep?(+ps: List<&2, J.Val>, +anchor: U32) -> List<&2, J.Val>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      gw.ifjl(gw.gt((ga.u(p, "s") + ga.u(p, "len") : U32), (anchor - griffinwing.types.dm(300) : U32)),
        _ => Con{p, griffinwing.things.Session.step_props.keep(t, anchor)},
        _ => griffinwing.things.Session.step_props.keep(t, anchor))

def griffinwing.things.Session.step_props.ids?(+s: J.Val, +ids: List<&2, U32>, +anchor: U32, +t: U32, +wx: U32) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +rest}:
      griffinwing.things.Session.step_props.ids(
        griffinwing.things.Session.step_props.seat(s, id, t, wx, gw.craft_of(s, id)),
        rest, anchor, t, wx)

# One knight: if flying, every prop in turn against where it crossed this tick.

def griffinwing.things.Session.step_props.seat?(+s: J.Val, +id: U32, +t: U32, +wx: U32, +c: J.Val) -> J.Val:
  gw.ifj(Bool.not(griffinwing.types.Craft.flying(c)),
    _ => s,
    _ => griffinwing.things.Session.step_props.walk(s, id, 0, t, wx,
      griffinwing.flight.Session.craft_s(s, c), ga.u(c, "ds"), ga.u(c, "x"), ga.u(c, "z"), ga.b(c, "boost")))

def griffinwing.things.Session.step_props.walk?(+s: J.Val, +id: U32, +pi: U32, +t: U32, +wx: U32, +s1: U32, +ds: U32, +x: U32, +z: U32, +boosting: Bool) -> J.Val:
  gw.ifj(U32.is_lt(pi, gw.len(s, "props")),
    _ => griffinwing.things.Session.step_props.walk(
      griffinwing.things.Session.step_props.prop(s, id, pi, t, wx, s1, (s1 - ds : U32), x, z, boosting, gw.item(s, "props", pi)),
      id, (pi + 1 : U32), t, wx, s1, ds, x, z, boosting),
    _ => s)

def griffinwing.things.Session.step_props.prop?(+s: J.Val, +id: U32, +pi: U32, +t: U32, +wx: U32, +s1: U32, +s0: U32, +x: U32, +z: U32, +boosting: Bool, +p: J.Val) -> J.Val:
  griffinwing.things.Session.step_props.thing(s, id, pi, t, wx, s1, s0, x, z, boosting, p,
    Bool.and(gw.lt(s0, (ga.u(p, "s") + ga.u(p, "len") : U32)), gw.gt(s1, ga.u(p, "s"))))

def griffinwing.things.Session.step_props.thing?(+s: J.Val, +id: U32, +pi: U32, +t: U32, +wx: U32, +s1: U32, +s0: U32, +x: U32, +z: U32, +boosting: Bool, +p: J.Val, +over: Bool) -> J.Val:
  gw.ifj(Bool.or(gw.isn(p, "thing", "Gate"), gw.isn(p, "thing", "Ring")),
    _ => gw.ifj(Bool.and(Bool.and(gw.lt(s0, ga.u(p, "s")), gw.le(ga.u(p, "s"), s1)),
          Bool.and(U32.is_zero(U32.and(ga.u(p, "taken"), griffinwing.bit(id))),
            gw.nle(griffinwing.types.flat(x, z, ga.u(p, "x"), ga.u(p, "z")), (ga.u(p, "w") + griffinwing.flight.CRAFT_R() : U32)))),
      _ => griffinwing.things.Session.take(
        gw.put_item(s, "props", pi, gw.orbits(p, "taken", griffinwing.bit(id))), id, p, boosting),
      _ => s),
    _ => gw.ifj(gw.isn(p, "thing", "Arch"),
      _ => gw.ifj(Bool.and(over, Bool.not(Bool.and(
            gw.le((gw.abs((x - ga.u(p, "x") : U32)) + griffinwing.flight.CRAFT_R() : U32), ga.u(p, "w")),
            gw.le((gw.abs((z - ga.u(p, "z") : U32)) + griffinwing.flight.CRAFT_R() : U32), ga.u(p, "h"))))),
        _ => griffinwing.things.Session.scrape(s, id, s1, x, z),
        _ => s),
      _ => gw.ifj(gw.isn(p, "thing", "Ridge"),
        _ => gw.ifj(Bool.and(over, Bool.and(
              gw.lt(gw.abs((x - ga.u(p, "x") : U32)), (ga.u(p, "w") + griffinwing.flight.CRAFT_R() : U32)),
              gw.lt(gw.abs((z - ga.u(p, "z") : U32)), (ga.u(p, "h") + griffinwing.flight.CRAFT_R() : U32)))),
          _ => griffinwing.things.Session.scrape(s, id, s1, x, z),
          _ => s),
        _ => gw.ifj(Bool.and(over, gw.gt(gw.abs((x - ga.u(p, "x") : U32)), (griffinwing.things.shutter_gap(p, t, wx) - griffinwing.flight.CRAFT_R() : U32))),
          _ => griffinwing.things.Session.scrape(s, id, s1, x, z),
          _ => s))))

# A gate or a ring flown through.

def griffinwing.things.Session.take?(+s: J.Val, +id: U32, +p: J.Val, +boosting: Bool) -> J.Val:
  griffinwing.Session.spark(
    gw.ifj(gw.isn(p, "thing", "Gate"),
      _ => griffinwing.things.Session.take.gate(s, id, p, boosting),
      _ => griffinwing.Session.sound(
        gw.put_craft(s, id, ga.si(gw.craft_of(s, id), "shield", gw.min((ga.u(gw.craft_of(s, id), "shield") + griffinwing.things.MEND() : U32), griffinwing.types.SHIELD()))),
        "shield_up")),
    "Gate", ga.u(p, "s"), ga.u(p, "x"), ga.u(p, "z"), U32.and(id, 255))

# A victory gate (`phase` 1) is a welcome, not a score.

def griffinwing.things.Session.take.gate?(+s: J.Val, +id: U32, +p: J.Val, +boosting: Bool) -> J.Val:
  griffinwing.Session.sound(
    gw.ifj(U32.is_zero(ga.u(p, "phase")),
      _ => griffinwing.things.Session.take.score(s, id, (griffinwing.things.GATE() * gw.ifu(boosting, _ => 2, _ => 1) : U32)),
      _ => s),
    "gold")

def griffinwing.things.Session.take.score?(+s: J.Val, +id: U32, +pts: U32) -> J.Val:
  ga.sv(
    gw.ifj(gw.seat_has(s, id),
      _ => gw.put_craft(s, id, ga.su(ga.su(gw.craft_of(s, id), "pts", (ga.u(gw.craft_of(s, id), "pts") + pts : U32)), "gates", RS.as_u16((ga.u(gw.craft_of(s, id), "gates") + 1 : U32)))),
      _ => s),
    "beat",
    ga.su(
      gw.addu(gw.addu(J.get(s, "beat"), "pts", U32.min(pts, griffinwing.things.GATE())), "base", griffinwing.things.GATE()),
      "gates", RS.as_u16((ga.u(J.get(s, "beat"), "gates") + 1 : U32))))

# Rock met: a blow, and the craft is spared a moment.

def griffinwing.things.Session.scrape?(+s: J.Val, +id: U32, +at: U32, +x: U32, +z: U32) -> J.Val:
  griffinwing.things.Session.scrape.hit(griffinwing.combat.Session.hurt_craft(s, id, griffinwing.things.SCRAPE()), id, at, x, z)

def griffinwing.things.Session.scrape.hit?(+r: J.Val, +id: U32, +at: U32, +x: U32, +z: U32) -> J.Val:
  gw.ifj(J.bool(gw.out_r(r)),
    _ => griffinwing.Session.sound(griffinwing.Session.spark(gw.out_s(r), "Scrape", at, x, z, U32.and(id, 255)), "rock_land"),
    _ => gw.out_s(r))

def griffinwing.types.Q?() -> U32:
  256

# A length in tenths of a unit, as fixed point: `n * Q / 10`, i32 division.

def griffinwing.types.dm?(+n: U32) -> U32:
  RS.i_div((n * 256 : U32), 10)

def griffinwing.types.PACE?() -> U32:
  85

def griffinwing.types.LEAD?() -> U32:
  griffinwing.types.dm(640)

def griffinwing.types.FAR?() -> U32:
  griffinwing.types.dm(340)

def griffinwing.types.NEAR?() -> U32:
  griffinwing.types.dm(150)

def griffinwing.types.LOCK_MAX?() -> U32:
  griffinwing.types.dm(520)

def griffinwing.types.LOCK_MIN?() -> U32:
  griffinwing.types.dm(15)

def griffinwing.types.MAX_LOCKS?() -> U32:
  6

def griffinwing.types.MAX_PER_BODY?() -> U32:
  3

def griffinwing.types.MAX_FOES?() -> U32:
  40

def griffinwing.types.MAX_PROPS?() -> U32:
  40

def griffinwing.types.MAX_BOLTS?() -> U32:
  160

def griffinwing.types.MAX_CALLS?() -> U32:
  4

def griffinwing.types.MAX_FX?() -> U32:
  48

def griffinwing.types.SHIELD?() -> U32:
  100

def griffinwing.types.WING?() -> U32:
  4

# The spur's meter, full.

def griffinwing.types.METER?() -> U32:
  7200

# Kind: its body's size, health and worth.

def griffinwing.types.Kind.pick?(+k: String, +wisp: U32, +kite: U32, +turret: U32, +bomber: U32, +lamp: U32) -> U32:
  Bool.pick(U32, String.eq(k, "Wisp"), wisp,
    Bool.pick(U32, String.eq(k, "Kite"), kite,
      Bool.pick(U32, String.eq(k, "Turret"), turret,
        Bool.pick(U32, String.eq(k, "Bomber"), bomber, lamp))))

def griffinwing.types.Kind.radius?(+k: String) -> U32:
  griffinwing.types.Kind.pick(k, griffinwing.types.dm(9), griffinwing.types.dm(8), griffinwing.types.dm(16), griffinwing.types.dm(12), griffinwing.types.dm(16))

def griffinwing.types.Kind.health?(+k: String) -> U32:
  griffinwing.types.Kind.pick(k, 20, 20, 60, 40, 120)

def griffinwing.types.Kind.worth?(+k: String) -> U32:
  griffinwing.types.Kind.pick(k, 10, 20, 60, 40, 500)

def griffinwing.types.Foe.radius?(+f: J.Val) -> U32:
  griffinwing.types.Kind.radius(ga.s(f, "kind"))

def griffinwing.types.Foe.alive?(+f: J.Val) -> Bool:
  Bool.and(U32.is_gt(ga.u(f, "hp"), 0), Bool.not(gw.isn(f, "st", "Broken")))

# Whether a lock may take it: alive, in the open and (a lamp) with its shield
# open.

def griffinwing.types.Foe.lockable?(+f: J.Val) -> Bool:
  Bool.and(griffinwing.types.Foe.alive(f),
    Bool.not(Bool.or(gw.isn(f, "st", "Hide"), Bool.or(gw.isn(f, "st", "Shut"), gw.isn(f, "st", "Flee")))))

def gw.alls?(+bs: List<&2, Bool>) -> Bool:
  match bs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Bool.and(h, gw.alls(t))

# A craft at its slot, shield full and three bombs.

def griffinwing.types.Craft.new?(+x: U32, +z: U32) -> J.Val:
  +c0 = J.obj()
  +c1 = ga.si(c0, "x", x)
  +c2 = ga.si(c1, "z", z)
  +c3 = ga.si(c2, "vx", 0)
  +c4 = ga.si(c3, "vz", 0)
  +c5 = ga.si(c4, "bank", 0)
  +c6 = ga.si(c5, "lift", 0)
  +c7 = ga.si(c6, "back", 0)
  +c8 = ga.si(c7, "ds", griffinwing.types.PACE())
  +c9 = ga.si(c8, "rx", 0)
  +c10 = ga.si(c9, "rz", 0)
  +c11 = ga.si(c10, "shield", griffinwing.types.SHIELD())
  +c12 = ga.su(c11, "hurt", 0)
  +c13 = ga.su(c12, "down", 0)
  +c14 = ga.su(c13, "roll", 0)
  +c15 = ga.su(c14, "roll_len", 0)
  +c16 = ga.si(c15, "dir", 1)
  +c17 = ga.su(c16, "rest", 0)
  +c18 = ga.su(c17, "held", 0)
  +c19 = ga.su(c18, "charge", 0)
  +c20 = ga.su(c19, "cool", 0)
  +c21 = ga.sv(c20, "locks", J.JA{[]})
  +c22 = ga.su(c21, "dwell", 0)
  +c23 = ga.si(c22, "meter", griffinwing.types.METER())
  +c24 = ga.sb(c23, "spent", False{})
  +c25 = ga.sb(c24, "boost", False{})
  +c26 = ga.sb(c25, "brake", False{})
  +c27 = ga.su(c26, "bombs", 3)
  +c28 = ga.su(c27, "keys", 0)
  +c29 = ga.si(c28, "tap_dir", 0)
  +c30 = ga.su(c29, "tap_ticks", 0)
  +c31 = ga.si(c30, "last_x", 0)
  +c32 = ga.su(c31, "chain", 0)
  +c33 = ga.su(c32, "chain_ticks", 0)
  +c34 = ga.su(c33, "pts", 0)
  +c35 = ga.su(c34, "hits", 0)
  +c36 = ga.su(c35, "saved", 0)
  ga.su(c36, "gates", 0)

def griffinwing.types.Craft.flying?(+c: J.Val) -> Bool:
  U32.is_zero(ga.u(c, "down"))

# The sum of the locks' counts.

def griffinwing.types.Craft.lock_total?(+ls: List<&2, J.Val>) -> U32:
  match ls:
    case Nil{}:
      0
    case Con{+l, +t}:
      (ga.u(l, "n") + griffinwing.types.Craft.lock_total(t) : U32)

def griffinwing.types.Craft.valid?(+c: J.Val) -> Bool:
  gw.alls([
    gw.far(ga.u(c, "x"), griffinwing.types.dm(160)),
    gw.far(ga.u(c, "z"), griffinwing.types.dm(120)),
    gw.far(ga.u(c, "vx"), 1024),
    gw.far(ga.u(c, "vz"), 1024),
    gw.far(ga.u(c, "bank"), 512),
    gw.far(ga.u(c, "lift"), 512),
    gw.far(ga.u(c, "back"), griffinwing.types.dm(80)),
    gw.within(ga.u(c, "ds"), 0, 1024),
    gw.far(ga.u(c, "rx"), griffinwing.types.dm(200)),
    gw.far(ga.u(c, "rz"), griffinwing.types.dm(200)),
    gw.within(ga.u(c, "shield"), 0, griffinwing.types.SHIELD()),
    U32.is_le(ga.u(c, "hurt"), 600),
    U32.is_le(ga.u(c, "down"), 600),
    U32.is_le(ga.u(c, "roll"), 120),
    U32.is_le(ga.u(c, "roll_len"), 120),
    Bool.or(U32.is_eq(ga.u(c, "dir"), 1), U32.is_eq(ga.u(c, "dir"), 4294967295)),
    U32.is_le(ga.u(c, "rest"), 240),
    U32.is_le(ga.u(c, "held"), 36000),
    U32.is_le(ga.u(c, "charge"), 240),
    U32.is_le(ga.u(c, "cool"), 240),
    U32.is_le(gw.len(c, "locks"), griffinwing.types.MAX_LOCKS()),
    U32.is_le(griffinwing.types.Craft.lock_total(ga.items(c, "locks")), griffinwing.types.MAX_LOCKS()),
    U32.is_le(ga.u(c, "dwell"), 600),
    gw.within(ga.u(c, "meter"), 0, griffinwing.types.METER()),
    U32.is_le(ga.u(c, "bombs"), 9),
    gw.within(ga.u(c, "tap_dir"), 4294967295, 1),
    gw.within(ga.u(c, "last_x"), 4294967295, 1),
    U32.is_le(ga.u(c, "chain_ticks"), 600)])

# Whether its roll turns bolts round just now.

def griffinwing.flight.Craft.reflecting?(+c: J.Val, +reflect: U32) -> Bool:
  Bool.and(U32.is_gt(ga.u(c, "roll"), 0), U32.is_lt((ga.u(c, "roll_len") - ga.u(c, "roll") : U32), reflect))

# Beast: the Wyrm's state, valid.

def griffinwing.types.Beast.marks_ok?(+ms: List<&2, J.Val>) -> Bool:
  match ms:
    case Nil{}:
      True{}
    case Con{+m, +t}:
      Bool.and(
        Bool.and(gw.far(ga.nu(m, 0), griffinwing.types.dm(300)), gw.far(ga.nu(m, 1), griffinwing.types.dm(300))),
        griffinwing.types.Beast.marks_ok(t))

def griffinwing.types.Beast.valid?(+b: J.Val) -> Bool:
  gw.alls([
    U32.is_le(ga.u(b, "phase"), 2),
    U32.is_le(ga.u(b, "atk"), 2),
    U32.is_le(ga.u(b, "ticks"), 3000),
    gw.far(ga.u(b, "x"), griffinwing.types.dm(200)),
    gw.far(ga.u(b, "z"), griffinwing.types.dm(200)),
    U32.is_le(gw.len(b, "marks"), 8),
    griffinwing.types.Beast.marks_ok(ga.items(b, "marks")),
    gw.far(ga.u(b, "gap"), griffinwing.types.dm(200)),
    gw.within(ga.u(b, "side"), 4294967295, 1),
    gw.far(ga.u(b, "col"), griffinwing.types.dm(200))])

# Tally: what the party has banked, and what the beat in hand has made.

def griffinwing.types.Tally.default?() -> J.Val:
  +t0 = J.obj()
  +t1 = ga.su(t0, "pts", 0)
  +t2 = ga.su(t1, "base", 0)
  +t3 = ga.su(t2, "offered", 0)
  +t4 = ga.su(t3, "kills", 0)
  +t5 = ga.su(t4, "saved", 0)
  +t6 = ga.su(t5, "lost", 0)
  ga.su(t6, "gates", 0)

def griffinwing.types.Tally.valid?(+t: J.Val) -> Bool:
  gw.alls([
    U32.is_le(ga.u(t, "pts"), 1000000),
    U32.is_le(ga.u(t, "base"), 1000000),
    U32.is_le(ga.u(t, "offered"), 1000000),
    U32.is_le(ga.u(t, "kills"), 10000),
    U32.is_le(ga.u(t, "saved"), 1000),
    U32.is_le(ga.u(t, "lost"), 1000),
    U32.is_le(ga.u(t, "gates"), 1000)])

# `self.bank(beat)`: add the beat's tally in (u16 fields wrap).

def griffinwing.types.Tally.bank?(+t: J.Val, +beat: J.Val) -> J.Val:
  +u0 = ga.su(t, "pts", (ga.u(t, "pts") + ga.u(beat, "pts") : U32))
  +u1 = ga.su(u0, "base", (ga.u(t, "base") + ga.u(beat, "base") : U32))
  +u2 = ga.su(u1, "offered", (ga.u(t, "offered") + ga.u(beat, "offered") : U32))
  +u3 = ga.su(u2, "kills", (ga.u(t, "kills") + ga.u(beat, "kills") : U32))
  +u4 = ga.su(u3, "saved", RS.as_u16((ga.u(t, "saved") + ga.u(beat, "saved") : U32)))
  +u5 = ga.su(u4, "lost", RS.as_u16((ga.u(t, "lost") + ga.u(beat, "lost") : U32)))
  ga.su(u5, "gates", RS.as_u16((ga.u(t, "gates") + ga.u(beat, "gates") : U32)))

# How many gold pieces the purse adds for the medal.

def griffinwing.types.Medal.rank?(+m: String) -> U32:
  Bool.pick(U32, String.eq(m, "Bronze"), 1, Bool.pick(U32, String.eq(m, "Silver"), 2, 3))

def griffinwing.types.Medal.name?(+m: String) -> String:
  gw.ifs(String.eq(m, "Bronze"), _ => "BRONZE", _ => gw.ifs(String.eq(m, "Silver"), _ => "SILVER", _ => "GOLD"))

# ─── the little arithmetic the sim shares ───────────────────────────────────

# An i32 sign-extended to an i64.

def gw.i64?(+x: U32) -> W.W:
  Bool.pick(W.W, RS.i_is_neg(x), W.of_parts(4294967295, x), W.of_u32(x))

def gw.w_is_neg?(+a: W.W) -> Bool:
  U32.is_ge(W.hi32(a), 2147483648)

def gw.w_abs?(+a: W.W) -> W.W:
  Bool.pick(W.W, gw.w_is_neg(a), W.neg(a), a)

def gw.w_signed_lt?(+a: W.W, +b: W.W) -> Bool:
  W.lt(W.xor(a, W.of_parts(2147483648, 0)), W.xor(b, W.of_parts(2147483648, 0)))

# Truncating i64 division (the quotient's sign is the product of the signs).

def gw.w_sdiv?(+a: W.W, +b: W.W) -> W.W:
  Bool.pick(W.W, Bool.xor(gw.w_is_neg(a), gw.w_is_neg(b)),
    W.neg(W.div(gw.w_abs(a), gw.w_abs(b))), W.div(gw.w_abs(a), gw.w_abs(b)))

# floor(sqrt(n)) of a u64 (exact; Rust's f64 sqrt agrees below 2^52).

def gw.w_isqrt.go?(+n: Nat, +a: W.W, +res: W.W) -> W.W:
  match n:
    case 0n:
      res
    case 1n+p:
      +cand = W.or(res, W.shl(W.one(), U32.from_nat(p)))
      gw.w_isqrt.go(p, a, Bool.pick(W.W, W.le(W.mul(cand, cand), a), cand, res))

def gw.w_isqrt?(+a: W.W) -> U32:
  W.lo32(gw.w_isqrt.go(32n, a, W.zero()))

def gw.sq64?(+v: U32) -> W.W:
  W.mul(gw.i64(v), gw.i64(v))

def griffinwing.types.norm.wide?(+x: U32, +y: U32, +z: U32) -> U32:
  gw.w_isqrt(W.add(W.add(gw.sq64(x), gw.sq64(y)), gw.sq64(z)))

def griffinwing.types.norm.small?(+x: U32, +y: U32, +z: U32) -> U32:
  RS.u_isqrt(((x * x : U32) + (y * y : U32) + (z * z : U32) : U32))

# A whole-number length of the vector, rounded down (an i64 that always fits a
# U32: sqrt of a non-negative i64).

def griffinwing.types.norm?(+x: U32, +y: U32, +z: U32) -> U32:
  gw.ifu(Bool.and(U32.is_lt(RS.i_abs(x), 32768), Bool.and(U32.is_lt(RS.i_abs(y), 32768), U32.is_lt(RS.i_abs(z), 32768))),
    _ => griffinwing.types.norm.small(x, y, z),
    _ => griffinwing.types.norm.wide(x, y, z))

# `(a as i64 * b as i64 / m) as i32` for a, b i32 and a positive m (a U32):
# the quotient truncates toward zero.

def gw.muldiv.sign?(+neg: Bool, +q: U32) -> U32:
  Bool.pick(U32, neg, (0 - q : U32), q)

def gw.muldiv.big?(+neg: Bool, +a: U32, +b: U32, +m: U32) -> U32:
  gw.muldiv.sign(neg, W.lo32(W.div(W.mul(W.of_u32(a), W.of_u32(b)), W.of_u32(m))))

def gw.muldiv?(+a: U32, +b: U32, +m: U32) -> U32:
  gw.ifu(Bool.and(U32.is_lt(RS.i_abs(a), 65536), U32.is_lt(RS.i_abs(b), 65536)),
    _ => gw.muldiv.sign(Bool.xor(RS.i_is_neg(a), RS.i_is_neg(b)), U32.div((RS.i_abs(a) * RS.i_abs(b) : U32), m)),
    _ => gw.muldiv.big(Bool.xor(RS.i_is_neg(a), RS.i_is_neg(b)), RS.i_abs(a), RS.i_abs(b), m))

# The three parts of a vector.

type GwV3 is Data:
  GwV3{a: U32, b: U32, c: U32}

def gw.v3a?(+v: GwV3) -> U32:
  match v:
    case GwV3{+a, +b, +c}:
      a

def gw.v3b?(+v: GwV3) -> U32:
  match v:
    case GwV3{+a, +b, +c}:
      b

def gw.v3c?(+v: GwV3) -> U32:
  match v:
    case GwV3{+a, +b, +c}:
      c

def gw.v3j?(+v: GwV3) -> J.Val:
  gw.tri3(gw.v3a(v), gw.v3b(v), gw.v3c(v))

# `d` turned to a length of `len`, along the same way (straight ahead for a
# zero vector).

def griffinwing.types.scale3?(+x: U32, +y: U32, +z: U32, +len: U32) -> GwV3:
  griffinwing.types.scale3.m(x, y, z, len, griffinwing.types.norm(x, y, z))

def griffinwing.types.scale3.m?(+x: U32, +y: U32, +z: U32, +len: U32, +m: U32) -> GwV3:
  match m:
    case 0:
      GwV3{len, 0, 0}
    case _:
      GwV3{gw.muldiv(x, len, m), gw.muldiv(y, len, m), gw.muldiv(z, len, m)}

# A triangle wave through `-amp..=amp` over `period` ticks (32-bit
# intermediates: the sim's periods are a few hundred and its amps a few
# thousand).

def griffinwing.types.tri?(+t: U32, +period: U32, +amp: U32) -> U32:
  griffinwing.types.tri.go(t, U32.max(period, 2), amp)

def griffinwing.types.tri.go?(+t: U32, +p: U32, +amp: U32) -> U32:
  griffinwing.types.tri.up(U32.mod(t, p), p, U32.max(U32.div(p, 2), 1), amp)

def griffinwing.types.tri.up?(+k: U32, +p: U32, +half: U32, +amp: U32) -> U32:
  gw.muldiv(((Bool.pick(U32, U32.is_lt(k, half), k, (p - k : U32)) * 2 : U32) - half : U32), amp, half)

# Whether the segment from `a` to `b` passes within `r` of `p`: the swept test
# every bolt, dive and latch is judged by. A cheap box test first: the nearest
# point on the segment lies inside its box, so a body farther than `r` from the
# box on any axis is out of reach.

def gw.gap?(+p: U32, +a: U32, +b: U32) -> U32:
  gw.gap.go(p, RS.i_min(a, b), RS.i_max(a, b))

def gw.gap.go?(+p: U32, +lo: U32, +hi: U32) -> U32:
  Bool.pick(U32, RS.i_lt(p, lo), (lo - p : U32), Bool.pick(U32, RS.i_gt(p, hi), (p - hi : U32), 0))

def griffinwing.types.near?(+ax: U32, +ay: U32, +az: U32, +bx: U32, +by: U32, +bz: U32, +px: U32, +py: U32, +pz: U32, +r: U32) -> Bool:
  gw.ifb(Bool.or(U32.is_gt(gw.gap(px, ax, bx), RS.i_abs(r)),
      Bool.or(U32.is_gt(gw.gap(py, ay, by), RS.i_abs(r)), U32.is_gt(gw.gap(pz, az, bz), RS.i_abs(r)))),
    _ => False{},
    _ => griffinwing.types.near.full(ax, ay, az, bx, by, bz, px, py, pz, r))

def griffinwing.types.near.full?(+ax: U32, +ay: U32, +az: U32, +bx: U32, +by: U32, +bz: U32, +px: U32, +py: U32, +pz: U32, +r: U32) -> Bool:
  griffinwing.types.near.d(
    W.sub(gw.i64(bx), gw.i64(ax)), W.sub(gw.i64(by), gw.i64(ay)), W.sub(gw.i64(bz), gw.i64(az)),
    W.sub(gw.i64(px), gw.i64(ax)), W.sub(gw.i64(py), gw.i64(ay)), W.sub(gw.i64(pz), gw.i64(az)), r)

def griffinwing.types.near.d?(+dx: W.W, +dy: W.W, +dz: W.W, +wx: W.W, +wy: W.W, +wz: W.W, +r: U32) -> Bool:
  griffinwing.types.near.t(dx, dy, dz, wx, wy, wz, r,
    W.add(W.add(W.mul(dx, dx), W.mul(dy, dy)), W.mul(dz, dz)),
    W.add(W.add(W.mul(wx, dx), W.mul(wy, dy)), W.mul(wz, dz)))

# t = (w . d).clamp(0, len2), or 0 for a point.

def griffinwing.types.near.t?(+dx: W.W, +dy: W.W, +dz: W.W, +wx: W.W, +wy: W.W, +wz: W.W, +r: U32, +len2: W.W, +dot: W.W) -> Bool:
  griffinwing.types.near.e(dx, dy, dz, wx, wy, wz, r, len2,
    Bool.pick(W.W, W.is_zero(len2), W.zero(),
      Bool.pick(W.W, gw.w_is_neg(dot), W.zero(), W.min(dot, len2))))

def griffinwing.types.near.e?(+dx: W.W, +dy: W.W, +dz: W.W, +wx: W.W, +wy: W.W, +wz: W.W, +r: U32, +len2: W.W, +t: W.W) -> Bool:
  griffinwing.types.near.cmp(
    griffinwing.types.near.c(dx, wx, len2, t),
    griffinwing.types.near.c(dy, wy, len2, t),
    griffinwing.types.near.c(dz, wz, len2, t), r)

# c(dv, wv) = if len2 == 0 { wv } else { wv - dv * t / len2 }

def griffinwing.types.near.c?(+dv: W.W, +wv: W.W, +len2: W.W, +t: W.W) -> W.W:
  Bool.pick(W.W, W.is_zero(len2), wv, W.sub(wv, gw.w_sdiv(W.mul(dv, t), len2)))

def griffinwing.types.near.cmp?(+ex: W.W, +ey: W.W, +ez: W.W, +r: U32) -> Bool:
  W.le(W.add(W.add(W.mul(ex, ex), W.mul(ey, ey)), W.mul(ez, ez)), W.mul(gw.i64(r), gw.i64(r)))

# How far two points are apart across and up (the plane's own distance).

def griffinwing.types.flat?(+ax: U32, +az: U32, +bx: U32, +bz: U32) -> U32:
  griffinwing.types.norm(0, (ax - bx : U32), (az - bz : U32))

def gw.wide?(+v: U32) -> Bool:
  gw.far(v, 40000000)

def griffinwing.Session.valid.seats?(+s: J.Val, +ids: List<&2, U32>) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+id, +t}:
      Bool.and(
        Bool.and(gw.within(id, 1, rules.MAX_PLAYERS()),
          Bool.and(U32.is_le(ga.u(ga.at(J.get(s, "seats"), id), "joined"), ga.u(s, "elapsed")), griffinwing.types.Craft.valid(gw.craft_of(s, id)))),
        griffinwing.Session.valid.seats(s, t))

def griffinwing.Session.valid.wing?(+ws: List<&2, J.Val>) -> Bool:
  match ws:
    case Nil{}:
      True{}
    case Con{+w, +t}:
      Bool.and(Bool.and(gw.within(ga.u(w, "id"), 7, 10), griffinwing.types.Craft.valid(J.get(w, "craft"))), griffinwing.Session.valid.wing(t))

def griffinwing.Session.valid.calls?(+cs: List<&2, J.Val>) -> Bool:
  match cs:
    case Nil{}:
      True{}
    case Con{+c, +t}:
      Bool.and(Bool.and(U32.is_le(ga.u(c, "ticks"), 200), U32.is_le(ga.u(c, "who"), 10)), griffinwing.Session.valid.calls(t))

def griffinwing.Session.valid.fx?(+fs: List<&2, J.Val>) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{+f, +t}:
      Bool.and(
        Bool.and(U32.is_le(ga.u(f, "ticks"), 60), Bool.and(gw.wide(ga.u(f, "s")), Bool.and(gw.wide(ga.u(f, "x")), gw.wide(ga.u(f, "z"))))),
        griffinwing.Session.valid.fx(t))

def griffinwing.Session.valid.foes?(+fs: List<&2, J.Val>) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{+f, +t}:
      Bool.and(griffinwing.Session.valid.foe(f), griffinwing.Session.valid.foes(t))

def griffinwing.Session.valid.foe?(+f: J.Val) -> Bool:
  gw.alls([
    U32.is_le(ga.u(f, "hp"), ga.u(f, "full")),
    U32.is_le(ga.u(f, "full"), (griffinwing.types.Kind.health("Lamp") * rules.MAX_PLAYERS() * 2 : U32)),
    gw.within(ga.u(f, "seats"), 1, rules.MAX_PLAYERS()),
    gw.wide(ga.u(f, "s")), gw.wide(ga.u(f, "x")), gw.wide(ga.u(f, "z")),
    gw.far(ga.u(f, "vs"), 1024), gw.far(ga.u(f, "vx"), 1024), gw.far(ga.u(f, "vz"), 1024),
    U32.is_le(ga.u(f, "ticks"), 100000),
    U32.is_le(ga.u(f, "on"), 10),
    gw.wide(ga.u(f, "fx")), gw.wide(ga.u(f, "fz"))])

def griffinwing.Session.valid.props?(+ps: List<&2, J.Val>) -> Bool:
  match ps:
    case Nil{}:
      True{}
    case Con{+p, +t}:
      Bool.and(
        gw.alls([
          gw.wide(ga.u(p, "s")), gw.wide(ga.u(p, "x")), gw.wide(ga.u(p, "z")),
          gw.far(ga.u(p, "w"), griffinwing.types.dm(400)), gw.far(ga.u(p, "h"), griffinwing.types.dm(400)), gw.far(ga.u(p, "len"), griffinwing.types.dm(400)),
          U32.is_lt(ga.u(p, "taken"), 64)]),
        griffinwing.Session.valid.props(t))

def griffinwing.Session.valid.bolts?(+bs: List<&2, J.Val>) -> Bool:
  match bs:
    case Nil{}:
      True{}
    case Con{+b, +t}:
      Bool.and(
        gw.alls([
          U32.is_le(ga.u(b, "owner"), 10),
          gw.wide(ga.u(b, "s")), gw.wide(ga.u(b, "x")), gw.wide(ga.u(b, "z")),
          gw.far(ga.u(b, "vs"), 2048), gw.far(ga.u(b, "vx"), 2048), gw.far(ga.u(b, "vz"), 2048),
          U32.is_le(ga.u(b, "life"), 600),
          U32.is_le(ga.u(b, "dmg"), 1000)]),
        griffinwing.Session.valid.bolts(t))

# `done.is_none_or(|d| d <= elapsed)`, `boss.is_none_or(Beast::valid)`

def griffinwing.Session.valid.done?(+d: J.Val, +elapsed: U32) -> Bool:
  Bool.or(J.is_null(d), U32.is_le(J.u32(d), elapsed))

def griffinwing.Session.valid.boss?(+b: J.Val) -> Bool:
  gw.ifb(J.is_null(b), _ => True{}, _ => griffinwing.types.Beast.valid(b))

def griffinwing.Session.valid?(+s: J.Val) -> Bool:
  gw.alls([
    U32.is_le(ga.u(s, "elapsed"), griffinwing.LIMIT()),
    U32.is_le(gw.nseats(s), rules.MAX_PLAYERS()),
    griffinwing.Session.valid.seats(s, ga.ids(J.get(s, "seats"))),
    U32.is_le(ga.u(s, "stage"), 2),
    U32.is_le(ga.u(s, "t"), griffinwing.LIMIT()),
    U32.is_le(ga.u(s, "cp"), U32.max(ga.u(s, "t"), griffinwing.rail.BOSS_AT())),
    U32.is_le(ga.u(s, "mode_t"), griffinwing.LIMIT()),
    U32.is_le(gw.len(s, "foes"), griffinwing.types.MAX_FOES()),
    U32.is_le(gw.len(s, "props"), griffinwing.types.MAX_PROPS()),
    U32.is_le(gw.len(s, "bolts"), griffinwing.types.MAX_BOLTS()),
    U32.is_le(gw.len(s, "wing"), griffinwing.types.WING()),
    U32.is_le(gw.len(s, "calls"), griffinwing.types.MAX_CALLS()),
    U32.is_le(gw.len(s, "fx"), griffinwing.types.MAX_FX()),
    griffinwing.types.Tally.valid(J.get(s, "bank")),
    griffinwing.types.Tally.valid(J.get(s, "beat")),
    U32.is_lt(ga.u(s, "paid"), 64),
    griffinwing.Session.valid.done(J.get(s, "done"), ga.u(s, "elapsed")),
    gw.ge(ga.u(s, "home"), 0),
    griffinwing.Session.valid.wing(ga.items(s, "wing")),
    griffinwing.Session.valid.boss(J.get(s, "boss")),
    griffinwing.Session.valid.calls(ga.items(s, "calls")),
    griffinwing.Session.valid.fx(ga.items(s, "fx")),
    griffinwing.Session.valid.foes(ga.items(s, "foes")),
    griffinwing.Session.valid.props(ga.items(s, "props")),
    griffinwing.Session.valid.bolts(ga.items(s, "bolts"))])

# Mend what a bad save broke.

def griffinwing.Session.repair?(+s0: J.Val) -> J.Val:
  griffinwing.Session.repair.lists(
    griffinwing.Session.repair.scalars(
      griffinwing.Session.repair.seats(
        ga.su(
          ga.sv(s0, "seats", griffinwing.Session.repair.keep(J.get(s0, "seats"), ga.ids(J.get(s0, "seats")))),
          "elapsed", U32.min(ga.u(s0, "elapsed"), griffinwing.LIMIT())))))

# seats.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id))

def griffinwing.Session.repair.keep?(+seats: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      seats
    case Con{+id, +t}:
      griffinwing.Session.repair.keep(gw.ifj(gw.within(id, 1, rules.MAX_PLAYERS()), _ => seats, _ => J.del(seats, U32.show(id))), t)

# for each seat: joined = joined.min(elapsed); a craft that is not valid is
# made anew at its slot.

def griffinwing.Session.repair.seats?(+s: J.Val) -> J.Val:
  griffinwing.Session.repair.seats.go(s, ga.ids(J.get(s, "seats")))

def griffinwing.Session.repair.seats.go?(+s: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.repair.seats.go(
        griffinwing.Session.repair.seat(s, id, ga.at(J.get(s, "seats"), id)),
        t)

def griffinwing.Session.repair.seat?(+s: J.Val, +id: U32, +seat: J.Val) -> J.Val:
  ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id,
    ga.sv(
      ga.su(seat, "joined", U32.min(ga.u(seat, "joined"), ga.u(s, "elapsed"))),
      "craft",
      gw.ifj(griffinwing.types.Craft.valid(J.get(seat, "craft")),
        _ => J.get(seat, "craft"),
        _ => griffinwing.types.Craft.new(griffinwing.flight.slot.x(id), griffinwing.flight.slot.z(id))))))

# stage, t, cp, mode_t held to their bounds.

def griffinwing.Session.repair.scalars?(+s0: J.Val) -> J.Val:
  griffinwing.Session.repair.scalars.cp(ga.su(ga.su(ga.su(s0, "stage", U32.min(ga.u(s0, "stage"), 2)), "t", U32.min(ga.u(s0, "t"), griffinwing.LIMIT())), "mode_t", U32.min(ga.u(s0, "mode_t"), griffinwing.LIMIT())))

def griffinwing.Session.repair.scalars.cp?(+s: J.Val) -> J.Val:
  ga.su(s, "cp", U32.min(ga.u(s, "cp"), U32.max(ga.u(s, "t"), griffinwing.rail.BOSS_AT())))

# The lists cut to their bounds, the riders and foes that cannot be kept dropped.

def griffinwing.Session.repair.lists?(+s0: J.Val) -> J.Val:
  +s1 = gw.trunc(gw.trunc(gw.trunc(gw.trunc(gw.trunc(s0, "foes", griffinwing.types.MAX_FOES()), "props", griffinwing.types.MAX_PROPS()), "bolts", griffinwing.types.MAX_BOLTS()), "calls", griffinwing.types.MAX_CALLS()), "fx", griffinwing.types.MAX_FX())
  +s2 = ga.sv(s1, "wing", J.truncate(J.JA{griffinwing.Session.repair.wing(ga.items(s1, "wing"), 0)}, griffinwing.types.WING()))
  +s3 = ga.sv(s2, "foes", J.JA{griffinwing.Session.repair.foes(ga.items(s2, "foes"))})
  griffinwing.Session.repair.checks(ga.su(s3, "paid", U32.and(ga.u(s3, "paid"), 63)))

# wing.retain(|w| (7..=10).contains(&w.id) && w.craft.valid())

def griffinwing.Session.repair.wing?(+ws: List<&2, J.Val>, +unused: U32) -> List<&2, J.Val>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      gw.ifjl(Bool.and(gw.within(ga.u(w, "id"), 7, 10), griffinwing.types.Craft.valid(J.get(w, "craft"))),
        _ => Con{w, griffinwing.Session.repair.wing(t, 0)},
        _ => griffinwing.Session.repair.wing(t, 0))

# foes.retain(|f| f.hp <= f.full && (1..=MAX_PLAYERS).contains(&f.seats))

def griffinwing.Session.repair.foes?(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      gw.ifjl(Bool.and(U32.is_le(ga.u(f, "hp"), ga.u(f, "full")), gw.within(ga.u(f, "seats"), 1, rules.MAX_PLAYERS())),
        _ => Con{f, griffinwing.Session.repair.foes(t)},
        _ => griffinwing.Session.repair.foes(t))

# A boss that is not valid goes; a tally that is not valid is cleared; and what
# is left, if the session is still not valid, is no beat to resume.

def griffinwing.Session.repair.checks?(+s0: J.Val) -> J.Val:
  griffinwing.Session.repair.last(
    ga.sv(ga.sv(
      gw.ifj(Bool.and(Bool.not(J.is_null(J.get(s0, "boss"))), Bool.not(griffinwing.types.Beast.valid(J.get(s0, "boss")))),
        _ => ga.sv(s0, "boss", J.JNull{}),
        _ => s0),
      "bank", gw.ifj(griffinwing.types.Tally.valid(J.get(s0, "bank")), _ => J.get(s0, "bank"), _ => griffinwing.types.Tally.default())),
      "beat", gw.ifj(griffinwing.types.Tally.valid(J.get(s0, "beat")), _ => J.get(s0, "beat"), _ => griffinwing.types.Tally.default())))

def griffinwing.Session.repair.last?(+s: J.Val) -> J.Val:
  gw.ifj(griffinwing.Session.valid(s),
    _ => s,
    _ => griffinwing.Session.repair.reset(s))

def griffinwing.Session.repair.reset?(+s: J.Val) -> J.Val:
  ga.su(ga.su(ga.su(
    ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(
      ga.sv(s, "done", gw.ifj(Bool.or(J.is_null(J.get(s, "done")), U32.is_le(J.u32(J.get(s, "done")), ga.u(s, "elapsed"))), _ => J.get(s, "done"), _ => J.JNull{})),
      "foes", J.JA{[]}), "props", J.JA{[]}), "bolts", J.JA{[]}), "fx", J.JA{[]}), "calls", J.JA{[]}), "boss", J.JNull{}),
    "t", 0), "cp", 0), "next", 0)

def rules.HZ?() -> U32:
  30

def rules.MAX_PLAYERS?() -> U32:
  6

# rules.rs `Input::default()`: nothing pressed (the skip-serialized fields are
# left out).

def rules.Input.default?() -> J.Val:
  i0 = J.obj()
  i1 = J.set(i0, "move_x", J.JU{0})
  i2 = J.set(i1, "move_y", J.JU{0})
  i3 = J.set(i2, "aim_x", J.JU{0})
  i4 = J.set(i3, "aim_y", J.JU{0})
  i5 = J.set(i4, "fire", J.JBool{False{}})
  i6 = J.set(i5, "dash", J.JBool{False{}})
  i7 = J.set(i6, "bomb", J.JBool{False{}})
  i8 = J.set(i7, "swing", J.JBool{False{}})
  i9 = J.set(i8, "play", J.JU{0})
  J.set(i9, "cast", J.JU{0})

# rules.rs `Input::valid()`: the axes in -1..=1, the slots in bounds (the vote
# is not read by Griffinwing's inputs).

def rules.Input.axis_ok?(+v: U32) -> Bool:
  Bool.or(U32.is_le(v, 1), U32.is_eq(v, 4294967295))

def rules.Input.valid?(+i: J.Val) -> Bool:
  Bool.and(
    Bool.and(
      Bool.and(rules.Input.axis_ok(J.u32(J.get(i, "move_x"))), rules.Input.axis_ok(J.u32(J.get(i, "move_y")))),
      Bool.and(rules.Input.axis_ok(J.u32(J.get(i, "aim_x"))), rules.Input.axis_ok(J.u32(J.get(i, "aim_y"))))),
    Bool.and(U32.is_le(J.u32(J.get(i, "play")), 4), Bool.and(U32.is_le(J.u32(J.get(i, "cast")), 4), U32.is_le(J.u32(J.get(i, "skill")), 5))))

# rules.rs `Run::room`: `self.dungeon.rooms[self.at]`.

def rules.Run.room?(+run: J.Val) -> J.Val:
  J.nth(J.get(J.get(run, "dungeon"), "rooms"), J.u32(J.get(run, "at")))

# forest.rs `Forest::leave`: the clearing's games stop.

def forest.Forest.leave?(+f: J.Val) -> J.Val:
  f0 = J.set(f, "hunt_left", J.JU{0})
  f1 = J.set(f0, "caught", J.JU{0})
  f2 = J.set(f1, "singing", J.JBool{False{}})
  f3 = J.set(f2, "show_left", J.JU{0})
  f4 = J.set(f3, "note", J.JU{0})
  J.set(f4, "held", J.JU{0})

# realm.rs `Spoils::add`: `if n > 0 { *entry(spoil).or_insert(0) = saturating_add }`.

def realm.Spoils.add?(+sp: J.Val, +spoil: String, +n: U32) -> J.Val:
  Bool.pick(J.Val, U32.is_zero(n), sp, J.set(sp, spoil, J.JU{ga.sadd32(J.u32(J.get(sp, spoil)), n)}))

# exits.rs `Run::leave_activity`: the run leaves for a knight who pressed to go.
# A stub: the Run is answered unchanged.

def exits.Run.leave_activity?(+run: J.Val, +id: U32) -> J.Val:
  run

def cases.run?(+name: String, +input: J.Val) -> J.Val:
  cases.run.n0(String.eq(name, "griffinwing.ACTIVITY.end"), name, input)

def cases.run.n0?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.ACTIVITY.end(input)
    case False{}:
      cases.run.n1(String.eq(name, "griffinwing.ACTIVITY.on"), name, input)

def cases.run.n1?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.ACTIVITY.on(input)
    case False{}:
      cases.run.n2(String.eq(name, "griffinwing.ACTIVITY.ride"), name, input)

def cases.run.n2?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.ACTIVITY.ride(input)
    case False{}:
      cases.run.n3(String.eq(name, "griffinwing.ADVENTURE.bot"), name, input)

def cases.run.n3?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.ADVENTURE.bot(input)
    case False{}:
      cases.run.n4(String.eq(name, "griffinwing.ADVENTURE.clear"), name, input)

def cases.run.n4?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.ADVENTURE.clear(input)
    case False{}:
      cases.run.n5(String.eq(name, "griffinwing.ADVENTURE.seated"), name, input)

def cases.run.n5?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.ADVENTURE.seated(input)
    case False{}:
      cases.run.n6(String.eq(name, "griffinwing.ADVENTURE.valid"), name, input)

def cases.run.n6?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.ADVENTURE.valid(input)
    case False{}:
      cases.run.n7(String.eq(name, "griffinwing.Nums.of"), name, input)

def cases.run.n7?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Nums.of(input)
    case False{}:
      cases.run.n8(String.eq(name, "griffinwing.Nums.reflect"), name, input)

def cases.run.n8?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Nums.reflect(input)
    case False{}:
      cases.run.n9(String.eq(name, "griffinwing.Nums.wx"), name, input)

def cases.run.n9?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Nums.wx(input)
    case False{}:
      cases.run.n10(String.eq(name, "griffinwing.Nums.wz"), name, input)

def cases.run.n10?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Nums.wz(input)
    case False{}:
      cases.run.n11(String.eq(name, "griffinwing.Run.griffinwing_showing"), name, input)

def cases.run.n11?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Run.griffinwing_showing(input)
    case False{}:
      cases.run.n12(String.eq(name, "griffinwing.Run.settle.pack"), name, input)

def cases.run.n12?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Run.settle.pack(input)
    case False{}:
      cases.run.n13(String.eq(name, "griffinwing.Run.settle_griffinwing"), name, input)

def cases.run.n13?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Run.settle_griffinwing(input)
    case False{}:
      cases.run.n14(String.eq(name, "griffinwing.Run.start_griffinwing"), name, input)

def cases.run.n14?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Run.start_griffinwing(input)
    case False{}:
      cases.run.n15(String.eq(name, "griffinwing.Run.tick_griffinwing"), name, input)

def cases.run.n15?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Run.tick_griffinwing(input)
    case False{}:
      cases.run.n16(String.eq(name, "griffinwing.Session.anchor"), name, input)

def cases.run.n16?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.anchor(input)
    case False{}:
      cases.run.n17(String.eq(name, "griffinwing.Session.end_beat"), name, input)

def cases.run.n17?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.end_beat(input)
    case False{}:
      cases.run.n18(String.eq(name, "griffinwing.Session.fill_wing"), name, input)

def cases.run.n18?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.fill_wing(input)
    case False{}:
      cases.run.n19(String.eq(name, "griffinwing.Session.finish"), name, input)

def cases.run.n19?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.finish(input)
    case False{}:
      cases.run.n20(String.eq(name, "griffinwing.Session.finished"), name, input)

def cases.run.n20?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.finished(input)
    case False{}:
      cases.run.n21(String.eq(name, "griffinwing.Session.medal_now"), name, input)

def cases.run.n21?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.medal_now(input)
    case False{}:
      cases.run.n22(String.eq(name, "griffinwing.Session.muster"), name, input)

def cases.run.n22?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.muster(input)
    case False{}:
      cases.run.n23(String.eq(name, "griffinwing.Session.new"), name, input)

def cases.run.n23?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.new(input)
    case False{}:
      cases.run.n24(String.eq(name, "griffinwing.Session.next_beat"), name, input)

def cases.run.n24?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.next_beat(input)
    case False{}:
      cases.run.n25(String.eq(name, "griffinwing.Session.next_id"), name, input)

def cases.run.n25?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.next_id(input)
    case False{}:
      cases.run.n26(String.eq(name, "griffinwing.Session.permille"), name, input)

def cases.run.n26?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.permille(input)
    case False{}:
      cases.run.n27(String.eq(name, "griffinwing.Session.pressure"), name, input)

def cases.run.n27?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.pressure(input)
    case False{}:
      cases.run.n28(String.eq(name, "griffinwing.Session.purse"), name, input)

def cases.run.n28?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.purse(input)
    case False{}:
      cases.run.n29(String.eq(name, "griffinwing.Session.repair"), name, input)

def cases.run.n29?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.repair(input)
    case False{}:
      cases.run.n30(String.eq(name, "griffinwing.Session.restart"), name, input)

def cases.run.n30?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.restart(input)
    case False{}:
      cases.run.n31(String.eq(name, "griffinwing.Session.say"), name, input)

def cases.run.n31?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.say(input)
    case False{}:
      cases.run.n32(String.eq(name, "griffinwing.Session.sit"), name, input)

def cases.run.n32?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.sit(input)
    case False{}:
      cases.run.n33(String.eq(name, "griffinwing.Session.sound"), name, input)

def cases.run.n33?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.sound(input)
    case False{}:
      cases.run.n34(String.eq(name, "griffinwing.Session.spark"), name, input)

def cases.run.n34?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.spark(input)
    case False{}:
      cases.run.n35(String.eq(name, "griffinwing.Session.step"), name, input)

def cases.run.n35?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.step(input)
    case False{}:
      cases.run.n36(String.eq(name, "griffinwing.Session.tick_lists"), name, input)

def cases.run.n36?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.tick_lists(input)
    case False{}:
      cases.run.n37(String.eq(name, "griffinwing.Session.valid"), name, input)

def cases.run.n37?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.valid(input)
    case False{}:
      cases.run.n38(String.eq(name, "griffinwing.Session.voice"), name, input)

def cases.run.n38?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.Session.voice(input)
    case False{}:
      cases.run.n39(String.eq(name, "griffinwing.bit"), name, input)

def cases.run.n39?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.bit(input)
    case False{}:
      cases.run.n40(String.eq(name, "griffinwing.boss.Session.begin_tell"), name, input)

def cases.run.n40?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.boss.Session.begin_tell(input)
    case False{}:
      cases.run.n41(String.eq(name, "griffinwing.boss.Session.rest"), name, input)

def cases.run.n41?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.boss.Session.rest(input)
    case False{}:
      cases.run.n42(String.eq(name, "griffinwing.boss.Session.rise"), name, input)

def cases.run.n42?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.boss.Session.rise(input)
    case False{}:
      cases.run.n43(String.eq(name, "griffinwing.boss.Session.spit"), name, input)

def cases.run.n43?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.boss.Session.spit(input)
    case False{}:
      cases.run.n44(String.eq(name, "griffinwing.boss.Session.step_boss"), name, input)

def cases.run.n44?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.boss.Session.step_boss(input)
    case False{}:
      cases.run.n45(String.eq(name, "griffinwing.bot.Session.boss_corridor"), name, input)

def cases.run.n45?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.bot.Session.boss_corridor(input)
    case False{}:
      cases.run.n46(String.eq(name, "griffinwing.bot.Session.bot_input"), name, input)

def cases.run.n46?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.bot.Session.bot_input(input)
    case False{}:
      cases.run.n47(String.eq(name, "griffinwing.bot.Session.bot_with"), name, input)

def cases.run.n47?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.bot.Session.bot_with(input)
    case False{}:
      cases.run.n48(String.eq(name, "griffinwing.bot.Session.footprint_away"), name, input)

def cases.run.n48?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.bot.Session.footprint_away(input)
    case False{}:
      cases.run.n49(String.eq(name, "griffinwing.bot.Session.intercept"), name, input)

def cases.run.n49?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.bot.Session.intercept(input)
    case False{}:
      cases.run.n50(String.eq(name, "griffinwing.bot.Session.reflector"), name, input)

def cases.run.n50?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.bot.Session.reflector(input)
    case False{}:
      cases.run.n51(String.eq(name, "griffinwing.bot.Session.room_from"), name, input)

def cases.run.n51?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.bot.Session.room_from(input)
    case False{}:
      cases.run.n52(String.eq(name, "griffinwing.bot.Session.threat"), name, input)

def cases.run.n52?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.bot.Session.threat(input)
    case False{}:
      cases.run.n53(String.eq(name, "griffinwing.bot.noise"), name, input)

def cases.run.n53?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.bot.noise(input)
    case False{}:
      cases.run.n54(String.eq(name, "griffinwing.combat.Session.bombers_ram"), name, input)

def cases.run.n54?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.bombers_ram(input)
    case False{}:
      cases.run.n55(String.eq(name, "griffinwing.combat.Session.break_lamp"), name, input)

def cases.run.n55?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.break_lamp(input)
    case False{}:
      cases.run.n56(String.eq(name, "griffinwing.combat.Session.drain"), name, input)

def cases.run.n56?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.drain(input)
    case False{}:
      cases.run.n57(String.eq(name, "griffinwing.combat.Session.fire_marks"), name, input)

def cases.run.n57?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.fire_marks(input)
    case False{}:
      cases.run.n58(String.eq(name, "griffinwing.combat.Session.friendly_bolt"), name, input)

def cases.run.n58?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.friendly_bolt(input)
    case False{}:
      cases.run.n59(String.eq(name, "griffinwing.combat.Session.hang"), name, input)

def cases.run.n59?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.hang(input)
    case False{}:
      cases.run.n60(String.eq(name, "griffinwing.combat.Session.hit_foe"), name, input)

def cases.run.n60?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.hit_foe(input)
    case False{}:
      cases.run.n61(String.eq(name, "griffinwing.combat.Session.hostile_bolt"), name, input)

def cases.run.n61?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.hostile_bolt(input)
    case False{}:
      cases.run.n62(String.eq(name, "griffinwing.combat.Session.hunt"), name, input)

def cases.run.n62?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.hunt(input)
    case False{}:
      cases.run.n63(String.eq(name, "griffinwing.combat.Session.hurt_craft"), name, input)

def cases.run.n63?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.hurt_craft(input)
    case False{}:
      cases.run.n64(String.eq(name, "griffinwing.combat.Session.kill"), name, input)

def cases.run.n64?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.kill(input)
    case False{}:
      cases.run.n65(String.eq(name, "griffinwing.combat.Session.land"), name, input)

def cases.run.n65?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.land(input)
    case False{}:
      cases.run.n66(String.eq(name, "griffinwing.combat.Session.latched"), name, input)

def cases.run.n66?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.latched(input)
    case False{}:
      cases.run.n67(String.eq(name, "griffinwing.combat.Session.marks"), name, input)

def cases.run.n67?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.marks(input)
    case False{}:
      cases.run.n68(String.eq(name, "griffinwing.combat.Session.plane_shot"), name, input)

def cases.run.n68?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.plane_shot(input)
    case False{}:
      cases.run.n69(String.eq(name, "griffinwing.combat.Session.spawn_foe"), name, input)

def cases.run.n69?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.spawn_foe(input)
    case False{}:
      cases.run.n70(String.eq(name, "griffinwing.combat.Session.starfall"), name, input)

def cases.run.n70?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.starfall(input)
    case False{}:
      cases.run.n71(String.eq(name, "griffinwing.combat.Session.step_bolts"), name, input)

def cases.run.n71?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.step_bolts(input)
    case False{}:
      cases.run.n72(String.eq(name, "griffinwing.combat.Session.step_foe"), name, input)

def cases.run.n72?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.step_foe(input)
    case False{}:
      cases.run.n73(String.eq(name, "griffinwing.combat.Session.step_foes"), name, input)

def cases.run.n73?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.step_foes(input)
    case False{}:
      cases.run.n74(String.eq(name, "griffinwing.combat.Session.target"), name, input)

def cases.run.n74?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.target(input)
    case False{}:
      cases.run.n75(String.eq(name, "griffinwing.combat.Session.turn"), name, input)

def cases.run.n75?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.turn(input)
    case False{}:
      cases.run.n76(String.eq(name, "griffinwing.combat.Session.turn.drop"), name, input)

def cases.run.n76?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.turn.drop(input)
    case False{}:
      cases.run.n77(String.eq(name, "griffinwing.combat.Session.victim"), name, input)

def cases.run.n77?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.combat.Session.victim(input)
    case False{}:
      cases.run.n78(String.eq(name, "griffinwing.flight.Craft.reflecting"), name, input)

def cases.run.n78?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.Craft.reflecting(input)
    case False{}:
      cases.run.n79(String.eq(name, "griffinwing.flight.Session.acquire"), name, input)

def cases.run.n79?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.Session.acquire(input)
    case False{}:
      cases.run.n80(String.eq(name, "griffinwing.flight.Session.craft_at"), name, input)

def cases.run.n80?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.Session.craft_at(input)
    case False{}:
      cases.run.n81(String.eq(name, "griffinwing.flight.Session.craft_s"), name, input)

def cases.run.n81?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.Session.craft_s(input)
    case False{}:
      cases.run.n82(String.eq(name, "griffinwing.flight.Session.fly"), name, input)

def cases.run.n82?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.Session.fly(input)
    case False{}:
      cases.run.n83(String.eq(name, "griffinwing.flight.Session.push_bolt"), name, input)

def cases.run.n83?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.Session.push_bolt(input)
    case False{}:
      cases.run.n84(String.eq(name, "griffinwing.flight.Session.release"), name, input)

def cases.run.n84?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.Session.release(input)
    case False{}:
      cases.run.n85(String.eq(name, "griffinwing.flight.Session.step_craft"), name, input)

def cases.run.n85?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.Session.step_craft(input)
    case False{}:
      cases.run.n86(String.eq(name, "griffinwing.flight.Session.tap"), name, input)

def cases.run.n86?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.Session.tap(input)
    case False{}:
      cases.run.n87(String.eq(name, "griffinwing.flight.Session.wing_fly"), name, input)

def cases.run.n87?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.Session.wing_fly(input)
    case False{}:
      cases.run.n88(String.eq(name, "griffinwing.flight.bits"), name, input)

def cases.run.n88?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.bits(input)
    case False{}:
      cases.run.n89(String.eq(name, "griffinwing.flight.rider_slot"), name, input)

def cases.run.n89?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.rider_slot(input)
    case False{}:
      cases.run.n90(String.eq(name, "griffinwing.flight.slot"), name, input)

def cases.run.n90?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.flight.slot(input)
    case False{}:
      cases.run.n91(String.eq(name, "griffinwing.script.Session.act"), name, input)

def cases.run.n91?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.script.Session.act(input)
    case False{}:
      cases.run.n92(String.eq(name, "griffinwing.script.Session.run_script"), name, input)

def cases.run.n92?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.script.Session.run_script(input)
    case False{}:
      cases.run.n93(String.eq(name, "griffinwing.script.Session.spread"), name, input)

def cases.run.n93?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.script.Session.spread(input)
    case False{}:
      cases.run.n94(String.eq(name, "griffinwing.script.Session.victory_gates"), name, input)

def cases.run.n94?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.script.Session.victory_gates(input)
    case False{}:
      cases.run.n95(String.eq(name, "griffinwing.script.resume"), name, input)

def cases.run.n95?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.script.resume(input)
    case False{}:
      cases.run.n96(String.eq(name, "griffinwing.script.script"), name, input)

def cases.run.n96?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.script.script(input)
    case False{}:
      cases.run.n97(String.eq(name, "griffinwing.settle"), name, input)

def cases.run.n97?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.settle(input)
    case False{}:
      cases.run.n98(String.eq(name, "griffinwing.things.Session.scrape"), name, input)

def cases.run.n98?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.things.Session.scrape(input)
    case False{}:
      cases.run.n99(String.eq(name, "griffinwing.things.Session.step_props"), name, input)

def cases.run.n99?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.things.Session.step_props(input)
    case False{}:
      cases.run.n100(String.eq(name, "griffinwing.things.Session.take"), name, input)

def cases.run.n100?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.things.Session.take(input)
    case False{}:
      cases.run.n101(String.eq(name, "griffinwing.things.shutter_gap"), name, input)

def cases.run.n101?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.things.shutter_gap(input)
    case False{}:
      cases.run.n102(String.eq(name, "griffinwing.types.Beast.valid"), name, input)

def cases.run.n102?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.Beast.valid(input)
    case False{}:
      cases.run.n103(String.eq(name, "griffinwing.types.Craft.new"), name, input)

def cases.run.n103?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.Craft.new(input)
    case False{}:
      cases.run.n104(String.eq(name, "griffinwing.types.Craft.valid"), name, input)

def cases.run.n104?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.Craft.valid(input)
    case False{}:
      cases.run.n105(String.eq(name, "griffinwing.types.Foe.alive"), name, input)

def cases.run.n105?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.Foe.alive(input)
    case False{}:
      cases.run.n106(String.eq(name, "griffinwing.types.Foe.lockable"), name, input)

def cases.run.n106?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.Foe.lockable(input)
    case False{}:
      cases.run.n107(String.eq(name, "griffinwing.types.Tally.bank"), name, input)

def cases.run.n107?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.Tally.bank(input)
    case False{}:
      cases.run.n108(String.eq(name, "griffinwing.types.Tally.valid"), name, input)

def cases.run.n108?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.Tally.valid(input)
    case False{}:
      cases.run.n109(String.eq(name, "griffinwing.types.flat"), name, input)

def cases.run.n109?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.flat(input)
    case False{}:
      cases.run.n110(String.eq(name, "griffinwing.types.near"), name, input)

def cases.run.n110?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.near(input)
    case False{}:
      cases.run.n111(String.eq(name, "griffinwing.types.norm"), name, input)

def cases.run.n111?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.norm(input)
    case False{}:
      cases.run.n112(String.eq(name, "griffinwing.types.scale3"), name, input)

def cases.run.n112?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.scale3(input)
    case False{}:
      cases.run.n113(String.eq(name, "griffinwing.types.tri"), name, input)

def cases.run.n113?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.griffinwing.types.tri(input)
    case False{}:
      J.JS{"no case wrapper"}
