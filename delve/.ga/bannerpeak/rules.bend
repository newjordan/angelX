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

def bannerpeak.body.Q?() -> U32:
  256

def bannerpeak.body.TQ?() -> U32:
  4096

def bannerpeak.body.TOP_ACCEL?() -> U32:
  56

def bannerpeak.body.AIR_ACCEL?() -> U32:
  44

def bannerpeak.body.AIR_DRAG?() -> U32:
  10

def bannerpeak.body.FRICTION?() -> U32:
  84

def bannerpeak.body.SKID_BRAKE?() -> U32:
  184

def bannerpeak.body.SKID_MIN?() -> U32:
  520

def bannerpeak.body.SLIDE_FRICTION?() -> U32:
  14

def bannerpeak.body.SLIDE_MIN?() -> U32:
  420

def bannerpeak.body.CROUCH_FRICTION?() -> U32:
  110

def bannerpeak.body.GRAV_UP?() -> U32:
  150

def bannerpeak.body.GRAV_CUT?() -> U32:
  340

def bannerpeak.body.GRAV_DOWN?() -> U32:
  210

def bannerpeak.body.GRAV_LONG?() -> U32:
  90

def bannerpeak.body.FALL_MAX?() -> U32:
  3584

def bannerpeak.body.JUMP_V?() -> U32:
  2200

def bannerpeak.body.JUMP_FROM_SPEED?() -> U32:
  5

def bannerpeak.body.CUT_V?() -> U32:
  820

def bannerpeak.body.HOP2_PERCENT?() -> U32:
  111

def bannerpeak.body.HOP3_PERCENT?() -> U32:
  133

def bannerpeak.body.HOP_MIN_SPEED?() -> U32:
  560

def bannerpeak.body.LONG_MIN_SPEED?() -> U32:
  760

def bannerpeak.body.LONG_V?() -> U32:
  1560

def bannerpeak.body.LONG_PERCENT?() -> U32:
  145

def bannerpeak.body.LONG_MAX?() -> U32:
  1900

def bannerpeak.body.BACK_V?() -> U32:
  2800

def bannerpeak.body.BACK_VX?() -> U32:
  300

def bannerpeak.body.KICK_V?() -> U32:
  1900

def bannerpeak.body.KICK_VX?() -> U32:
  1150

def bannerpeak.body.WALL_SLIDE?() -> U32:
  560

def bannerpeak.body.WIND?() -> U32:
  6

def bannerpeak.body.SLAM_V?() -> U32:
  3200

def bannerpeak.body.RECOVER?() -> U32:
  9

def bannerpeak.body.SLAM_JUMP_WINDOW?() -> U32:
  5

def bannerpeak.body.SLAM_JUMP_V?() -> U32:
  2700

def bannerpeak.body.DIVE_VX?() -> U32:
  1800

def bannerpeak.body.DIVE_VY?() -> U32:
  700

def bannerpeak.body.BELLY?() -> U32:
  16

def bannerpeak.body.BELLY_FRICTION?() -> U32:
  30

def bannerpeak.body.BOUNCE_V?() -> U32:
  1500

def bannerpeak.body.BOUNCE_HELD_V?() -> U32:
  2350

def bannerpeak.body.HURT_TICKS?() -> U32:
  14

def bannerpeak.body.BRACE_TICKS?() -> U32:
  5

def bannerpeak.body.LEDGE_LIP?() -> U32:
  6

def bannerpeak.body.SHIELD_ID?() -> U32:
  1000

def bannerpeak.body.Ev.jump?(+kind: String) -> J.Val:
  J.set(J.set(J.set(J.set(J.obj(), "k", J.JS{"Jump"}), "s", J.JS{kind}), "a", J.of_i32(0)), "b", J.of_i32(0))

def bannerpeak.body.Ev.land?(+fell: U32) -> J.Val:
  J.set(J.set(J.set(J.set(J.obj(), "k", J.JS{"Land"}), "s", J.JS{""}), "a", J.of_i32(fell)), "b", J.of_i32(0))

def bannerpeak.body.Ev.bump?(+x: U32, +y: U32) -> J.Val:
  J.set(J.set(J.set(J.set(J.obj(), "k", J.JS{"Bump"}), "s", J.JS{""}), "a", J.of_i32(x)), "b", J.of_i32(y))

def bannerpeak.body.Ev.slam?(+x: U32, +y: U32) -> J.Val:
  J.set(J.set(J.set(J.set(J.obj(), "k", J.JS{"Slam"}), "s", J.JS{""}), "a", J.of_i32(x)), "b", J.of_i32(y))

def bannerpeak.body.Ev.skid?() -> J.Val:
  J.set(J.set(J.set(J.set(J.obj(), "k", J.JS{"Skid"}), "s", J.JS{""}), "a", J.of_i32(0)), "b", J.of_i32(0))

def bannerpeak.body.Ev.spring?() -> J.Val:
  J.set(J.set(J.set(J.set(J.obj(), "k", J.JS{"Spring"}), "s", J.JS{""}), "a", J.of_i32(0)), "b", J.of_i32(0))

def bannerpeak.body.Ev.hatch?() -> J.Val:
  J.set(J.set(J.set(J.set(J.obj(), "k", J.JS{"Hatch"}), "s", J.JS{""}), "a", J.of_i32(0)), "b", J.of_i32(0))

def bannerpeak.body.approach?(+v: U32, +to: U32, +by: U32) -> U32:
  bannerpeak.body.approach.if1(RS.i_lt(v, to), v, to, by)

def bannerpeak.body.approach.if1?(bpc: Bool, +v: U32, +to: U32, +by: U32) -> U32:
  match bpc:
    case True{}:
      RS.i_min((v + by : U32), to)
    case False{}:
      RS.i_max((v - by : U32), to)

def bannerpeak.body.Body.new?(+x_px: U32, +y_px: U32) -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32((x_px * 256 : U32))), "y", J.of_i32((y_px * 256 : U32))), "vx", J.of_i32(0)), "vy", J.of_i32(0)), "face", J.of_i32(1)), "mode", J.JS{"Ground"}), "jump", J.JS{"None"}), "big", J.JBool{False{}}), "ground", J.JBool{True{}}), "plank", J.JNull{}), "coyote", J.JU{0}), "buffer", J.JU{0}), "hop_next", J.JU{0}), "hop_t", J.JU{0}), "hop", J.JU{0}), "crouch", J.JBool{False{}}), "wall", J.of_i32(0)), "wall_t", J.JU{0}), "t", J.JU{0}), "freeze", J.JU{0}), "squash", J.JU{0}), "cut", J.JBool{False{}}), "skid", J.JBool{False{}}), "was_skid", J.JBool{False{}}), "pose", J.JS{"Idle"}), "run_t", J.JU{0})

def bannerpeak.body.Body.valid?(+self: J.Val) -> Bool:
  Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(RS.i_ge(J.u32(J.get(self, "x")), RS.i_neg((8 * 4096 : U32))), RS.i_le(J.u32(J.get(self, "x")), (1200 * 4096 : U32))), Bool.and(RS.i_ge(J.u32(J.get(self, "y")), RS.i_neg((64 * 4096 : U32))), RS.i_le(J.u32(J.get(self, "y")), (80 * 4096 : U32)))), Bool.and(RS.i_ge(J.u32(J.get(self, "vx")), 4294961296), RS.i_le(J.u32(J.get(self, "vx")), 6000))), Bool.and(RS.i_ge(J.u32(J.get(self, "vy")), 4294961296), RS.i_le(J.u32(J.get(self, "vy")), 6000))), Bool.or(U32.is_eq(J.u32(J.get(self, "face")), 4294967295), U32.is_eq(J.u32(J.get(self, "face")), 1))), U32.is_le(J.u32(J.get(self, "coyote")), 12)), U32.is_le(J.u32(J.get(self, "buffer")), 12)), U32.is_le(J.u32(J.get(self, "hop_next")), 3)), U32.is_le(J.u32(J.get(self, "hop_t")), 40)), U32.is_le(J.u32(J.get(self, "hop")), 3)), Bool.and(RS.i_ge(J.u32(J.get(self, "wall")), 4294967295), RS.i_le(J.u32(J.get(self, "wall")), 1))), U32.is_le(J.u32(J.get(self, "wall_t")), 12)), U32.is_le(J.u32(J.get(self, "t")), 600)), U32.is_le(J.u32(J.get(self, "freeze")), 8)), U32.is_le(J.u32(J.get(self, "squash")), 8)), U32.is_lt(J.u32(J.get(self, "run_t")), 16777216))

def bannerpeak.body.Body.dims?(+self: J.Val) -> Sigma<&2, &2, U32, _ => U32>:
  +low : Bool = Bool.or(Bool.or(Bool.or(J.bool(J.get(self, "crouch")), String.eq(J.str(J.get(self, "mode")), "Dive")), String.eq(J.str(J.get(self, "mode")), "Belly")), String.eq(J.str(J.get(self, "mode")), "Recover"))
  bannerpeak.body.Body.dims.if3(Bool.not(J.bool(J.get(self, "big"))), low)

def bannerpeak.body.Body.dims.if1?(bpc: Bool) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      (12, 20)
    case False{}:
      (12, 12)

def bannerpeak.body.Body.dims.if2?(bpc: Bool) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      (14, 30)
    case False{}:
      (14, 18)

def bannerpeak.body.Body.dims.if3?(bpc: Bool, +low: Bool) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      bannerpeak.body.Body.dims.if1(Bool.not(low))
    case False{}:
      bannerpeak.body.Body.dims.if2(Bool.not(low))

def bannerpeak.body.Body.stand_height?(+self: J.Val) -> U32:
  bannerpeak.body.Body.stand_height.if1(J.bool(J.get(self, "big")))

def bannerpeak.body.Body.stand_height.if1?(bpc: Bool) -> U32:
  match bpc:
    case True{}:
      30
    case False{}:
      20

def bannerpeak.body.Body.rect?(+self: J.Val) -> Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.dims(self)
  +w = bannerpeak.u.fst(U32, U32, _t1)
  +h = bannerpeak.u.snd(U32, U32, _t1)
  ((J.u32(J.get(self, "x")) - RS.i_div((w * 256 : U32), 2) : U32), ((J.u32(J.get(self, "y")) - (h * 256 : U32) : U32), ((J.u32(J.get(self, "x")) + RS.i_div((w * 256 : U32), 2) : U32), J.u32(J.get(self, "y")))))

def bannerpeak.body.Body.foot?(+self: J.Val) -> Sigma<&2, &2, U32, _ => U32>:
  (RS.i_div_euclid(J.u32(J.get(self, "x")), 256), RS.i_div_euclid(J.u32(J.get(self, "y")), 256))

def bannerpeak.body.Body.slamming?(+self: J.Val) -> Bool:
  String.eq(J.str(J.get(self, "mode")), "Pound")

def bannerpeak.body.Body.shield?(+self: J.Val, +id: U32) -> J.Val:
  bannerpeak.body.Body.shield.if1(Bool.and(Bool.and(String.eq(J.str(J.get(self, "mode")), "Brace"), U32.is_ge(J.u32(J.get(self, "t")), 5)), J.bool(J.get(self, "ground"))), self, id)

def bannerpeak.body.Body.shield.if1?(bpc: Bool, +self: J.Val, +id: U32) -> J.Val:
  match bpc:
    case True{}:
      +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.dims(self)
      +w = bannerpeak.u.fst(U32, U32, _t1)
      +h = bannerpeak.u.snd(U32, U32, _t1)
      J.set(J.set(J.obj(), "id", J.JU{id}), "p", J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32((RS.i_div_euclid(J.u32(J.get(self, "x")), 256) - 9 : U32))), "y", J.of_i32(((RS.i_div_euclid(J.u32(J.get(self, "y")), 256) - h : U32) - 1 : U32))), "w", J.of_i32(18)), "lift", J.JBool{False{}}))
    case False{}:
      J.JNull{}

def bannerpeak.body.Body.hurt?(+self: J.Val, +from: U32) -> J.Val:
  +self = J.set(self, "mode", J.JS{"Hurt"})
  +self = J.set(self, "t", J.JU{0})
  +self = J.set(self, "vx", J.of_i32((RS.i_neg(RS.i_signum(from)) * 760 : U32)))
  +self = J.set(self, "vy", J.of_i32(4294965796))
  +self = J.set(self, "ground", J.JBool{False{}})
  +self = J.set(self, "plank", J.JNull{})
  +self = J.set(self, "crouch", J.JBool{False{}})
  +self = J.set(self, "jump", J.JS{"None"})
  self

def bannerpeak.body.Body.bounce?(+self: J.Val, +held: Bool) -> J.Val:
  +self = J.set(self, "vy", J.of_i32(bannerpeak.u.ife(U32, held, _ => RS.i_neg(2350), _ => RS.i_neg(1500))))
  +self = J.set(self, "mode", J.JS{"Air"})
  +self = J.set(self, "ground", J.JBool{False{}})
  +self = J.set(self, "plank", J.JNull{})
  +self = J.set(self, "jump", J.JS{"Bounce"})
  +self = J.set(self, "hop", J.JU{0})
  +self = J.set(self, "hop_t", J.JU{0})
  +self = J.set(self, "cut", J.JBool{True{}})
  self

def bannerpeak.body.Body.place?(+self: J.Val, +x_px: U32, +y_px: U32) -> J.Val:
  +self = J.set(self, "x", J.of_i32((x_px * 256 : U32)))
  +self = J.set(self, "y", J.of_i32((y_px * 256 : U32)))
  +self = J.set(self, "vx", J.of_i32(0))
  +self = J.set(self, "vy", J.of_i32(0))
  +self = J.set(self, "mode", J.JS{"Ground"})
  +self = J.set(self, "jump", J.JS{"None"})
  +self = J.set(self, "ground", J.JBool{True{}})
  +self = J.set(self, "plank", J.JNull{})
  +self = J.set(self, "crouch", J.JBool{False{}})
  +self = J.set(self, "hop_t", J.JU{0})
  +self = J.set(self, "hop", J.JU{0})
  +self = J.set(self, "t", J.JU{0})
  +self = J.set(self, "freeze", J.JU{0})
  +self = J.set(self, "buffer", J.JU{0})
  +self = J.set(self, "wall_t", J.JU{0})
  self

def bannerpeak.body.Body.blocked?(+g: J.Val, +l: U32, +t: U32, +r: U32, +b: U32) -> Bool:
  bannerpeak.body.Body.blocked.if1(Bool.or(RS.i_le(r, l), RS.i_le(b, t)), g, l, t, r, b)

def bannerpeak.body.Body.blocked.if1?(bpc: Bool, +g: J.Val, +l: U32, +t: U32, +r: U32, +b: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.body.Body.blocked_rows(g, RS.i_div_euclid(t, 4096), RS.i_div_euclid((b - 1 : U32), 4096), RS.i_div_euclid(l, 4096), RS.i_div_euclid((r - 1 : U32), 4096))

def bannerpeak.body.Body.blocked_rows?(+g: J.Val, +ty: U32, +ty1: U32, +tx0: U32, +tx1: U32) -> Bool:
  bannerpeak.body.Body.blocked_rows.if2(RS.i_gt(ty, ty1), g, ty, ty1, tx0, tx1)

def bannerpeak.body.Body.blocked_rows.if1?(bpc: Bool, +g: J.Val, +ty: U32, +ty1: U32, +tx0: U32, +tx1: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.body.Body.blocked_rows(g, (ty + 1 : U32), ty1, tx0, tx1)

def bannerpeak.body.Body.blocked_rows.if2?(bpc: Bool, +g: J.Val, +ty: U32, +ty1: U32, +tx0: U32, +tx1: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.body.Body.blocked_rows.if1(bannerpeak.body.Body.blocked_cols(g, ty, tx0, tx1), g, ty, ty1, tx0, tx1)

def bannerpeak.body.Body.blocked_cols?(+g: J.Val, +ty: U32, +tx: U32, +tx1: U32) -> Bool:
  bannerpeak.body.Body.blocked_cols.if2(RS.i_gt(tx, tx1), g, ty, tx, tx1)

def bannerpeak.body.Body.blocked_cols.if1?(bpc: Bool, +g: J.Val, +ty: U32, +tx: U32, +tx1: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.body.Body.blocked_cols(g, ty, (tx + 1 : U32), tx1)

def bannerpeak.body.Body.blocked_cols.if2?(bpc: Bool, +g: J.Val, +ty: U32, +tx: U32, +tx1: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.body.Body.blocked_cols.if1(bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, tx, ty)), g, ty, tx, tx1)

def bannerpeak.body.Body.regrow?(+self: J.Val, +g: J.Val) -> J.Val:
  +self = bannerpeak.body.Body.regrow.if1(Bool.not(bannerpeak.body.Body.can_stand(self, g)), self)
  self

def bannerpeak.body.Body.regrow.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "crouch", J.JBool{True{}})
      self
    case False{}:
      self

def bannerpeak.body.Body.can_stand?(+self: J.Val, +g: J.Val) -> Bool:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.dims(self)
  +w = bannerpeak.u.fst(U32, U32, _t1)
  +h = bannerpeak.u.snd(U32, U32, _t1)
  +top : U32 = (J.u32(J.get(self, "y")) - (bannerpeak.body.Body.stand_height(self) * 256 : U32) : U32)
  Bool.not(bannerpeak.body.Body.blocked(g, (J.u32(J.get(self, "x")) - RS.i_div((w * 256 : U32), 2) : U32), top, (J.u32(J.get(self, "x")) + RS.i_div((w * 256 : U32), 2) : U32), (J.u32(J.get(self, "y")) - (h * 256 : U32) : U32)))

def bannerpeak.body.Body.wall_side?(+self: J.Val, +g: J.Val, +toward: U32) -> U32:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.dims(self)
  +w = bannerpeak.u.fst(U32, U32, _t1)
  +h = bannerpeak.u.snd(U32, U32, _t1)
  +t : U32 = ((J.u32(J.get(self, "y")) - (h * 256 : U32) : U32) + (2 * 256 : U32) : U32)
  +b : U32 = (J.u32(J.get(self, "y")) - (2 * 256 : U32) : U32)
  +s1 : U32 = bannerpeak.body.Body.wall_probe(self, g, toward, w, t, b)
  bannerpeak.body.Body.wall_side.if1(U32.is_ne(s1, 0), self, g, toward, w, t, b, s1)

def bannerpeak.body.Body.wall_side.if1?(bpc: Bool, +self: J.Val, +g: J.Val, +toward: U32, +w: U32, +t: U32, +b: U32, +s1: U32) -> U32:
  match bpc:
    case True{}:
      s1
    case False{}:
      bannerpeak.body.Body.wall_probe(self, g, RS.i_neg(toward), w, t, b)

def bannerpeak.body.Body.wall_probe?(+self: J.Val, +g: J.Val, +side: U32, +w: U32, +t: U32, +b: U32) -> U32:
  bannerpeak.body.Body.wall_probe.if3(U32.is_eq(side, 0), self, g, side, w, t, b)

def bannerpeak.body.Body.wall_probe.if1?(bpc: Bool, +self: J.Val, +w: U32, +l: U32, +r: U32) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      +l : U32 = (J.u32(J.get(self, "x")) + RS.i_div((w * 256 : U32), 2) : U32)
      +r : U32 = (((J.u32(J.get(self, "x")) + RS.i_div((w * 256 : U32), 2) : U32) + 256 : U32) + RS.i_div(256, 2) : U32)
      (l, r)
    case False{}:
      +l : U32 = (((J.u32(J.get(self, "x")) - RS.i_div((w * 256 : U32), 2) : U32) - 256 : U32) - RS.i_div(256, 2) : U32)
      +r : U32 = (J.u32(J.get(self, "x")) - RS.i_div((w * 256 : U32), 2) : U32)
      (l, r)

def bannerpeak.body.Body.wall_probe.if2?(bpc: Bool, +side: U32) -> U32:
  match bpc:
    case True{}:
      side
    case False{}:
      0

def bannerpeak.body.Body.wall_probe.if3?(bpc: Bool, +self: J.Val, +g: J.Val, +side: U32, +w: U32, +t: U32, +b: U32) -> U32:
  match bpc:
    case True{}:
      0
    case False{}:
      +l : U32 = 0
      +r : U32 = 0
      +_t1 = bannerpeak.body.Body.wall_probe.if1(RS.i_gt(side, 0), self, w, l, r)
      +l = bannerpeak.u.fst(U32, U32, _t1)
      +r = bannerpeak.u.snd(U32, U32, _t1)
      bannerpeak.body.Body.wall_probe.if2(bannerpeak.body.Body.blocked(g, l, t, r, b), side)

def bannerpeak.body.Body.tile_below?(+self: J.Val, +g: J.Val) -> U32:
  bannerpeak.session.View.tile(g, RS.i_div_euclid(J.u32(J.get(self, "x")), 4096), RS.i_div_euclid(J.u32(J.get(self, "y")), 4096))

def bannerpeak.body.Body.find_mover?(+ms: J.Val, +id: U32, +n: U32) -> J.Val:
  bannerpeak.body.Body.find_mover.if2(U32.is_ge(n, J.len(ms)), ms, id, n)

def bannerpeak.body.Body.find_mover.if1?(bpc: Bool, +ms: J.Val, +id: U32, +n: U32, +m: J.Val) -> J.Val:
  match bpc:
    case True{}:
      m
    case False{}:
      bannerpeak.body.Body.find_mover(ms, id, (n + 1 : U32))

def bannerpeak.body.Body.find_mover.if2?(bpc: Bool, +ms: J.Val, +id: U32, +n: U32) -> J.Val:
  match bpc:
    case True{}:
      J.JNull{}
    case False{}:
      +m : J.Val = J.nth(ms, n)
      bannerpeak.body.Body.find_mover.if1(U32.is_eq(J.u32(J.get(m, "id")), id), ms, id, n, m)

def bannerpeak.body.Body.step?(+self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +now: J.Val, +before: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +self = bannerpeak.body.Body.step.if1(J.bool(J.get(c, "jump_edge")), self, k)
  +self = J.set(self, "hop_t", J.JU{RS.u_sat_sub(J.u32(J.get(self, "hop_t")), 1)})
  +self = J.set(self, "squash", J.JU{RS.u_sat_sub(J.u32(J.get(self, "squash")), 1)})
  bannerpeak.body.Body.step.if9(U32.is_gt(J.u32(J.get(self, "freeze")), 0), self, c, k, g, now, before, out)

def bannerpeak.body.Body.step.if1?(bpc: Bool, +self: J.Val, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "buffer", J.JU{(J.u32(J.get(k, "grace")) + 1 : U32)})
      self
    case False{}:
      self

def bannerpeak.body.Body.step.if2?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Ground"})
      +self = J.set(self, "t", J.JU{0})
      self
    case False{}:
      self

def bannerpeak.body.Body.step.if3?(bpc: Bool, +self: J.Val, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "coyote", J.JU{(J.u32(J.get(k, "grace")) + 2 : U32)})
      self
    case False{}:
      self

def bannerpeak.body.Body.step.if4?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Air"})
      self
    case False{}:
      self

def bannerpeak.body.Body.step.if5?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "coyote", J.JU{RS.u_sat_sub(J.u32(J.get(self, "coyote")), 1)})
      self
    case False{}:
      self

def bannerpeak.body.Body.step.if6?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Air"})
      self
    case False{}:
      self

def bannerpeak.body.Body.step.if7?(bpc: Bool, +self: J.Val, +k: J.Val, +out: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t6 = bannerpeak.body.Body.landed(self, k, out)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t6)
      +out : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t6)
      self
    case False{}:
      self

def bannerpeak.body.Body.step.if8?(bpc: Bool, +out: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +out = J.push(out, bannerpeak.body.Ev.hatch())
      out
    case False{}:
      out

def bannerpeak.body.Body.step.if9?(bpc: Bool, +self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +now: J.Val, +before: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "freeze", J.JU{(J.u32(J.get(self, "freeze")) - 1 : U32)})
      +self = J.set(self, "buffer", J.JU{RS.u_sat_sub(J.u32(J.get(self, "buffer")), 1)})
      (self, out)
    case False{}:
      +_t1 = bannerpeak.body.Body.carried(self, now, before)
      +self : J.Val = _t1
      +_t2 = bannerpeak.body.Body.unstick(self, g)
      +self : J.Val = _t2
      +self = J.set(self, "was_skid", J.JBool{J.bool(J.get(self, "skid"))})
      +self = J.set(self, "skid", J.JBool{False{}})
      +self = bannerpeak.body.Body.step.if2(Bool.and(String.eq(J.str(J.get(self, "mode")), "Brace"), Bool.or(Bool.or(Bool.not(Bool.and(J.bool(J.get(c, "brace")), J.bool(J.get(self, "ground")))), J.bool(J.get(c, "jump_edge"))), J.bool(J.get(c, "down")))), self)
      +was_ground : Bool = J.bool(J.get(self, "ground"))
      +self = bannerpeak.body.Body.step.if3(J.bool(J.get(self, "ground")), self, k)
      +_t3 = bannerpeak.body.Body.modes(self, c, k, g, out)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +out : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t3)
      +self = J.set(self, "buffer", J.JU{RS.u_sat_sub(J.u32(J.get(self, "buffer")), 1)})
      +dx : U32 = J.u32(J.get(self, "vx"))
      +dy : U32 = J.u32(J.get(self, "vy"))
      +_t4 = bannerpeak.body.Body.move_x(self, g, dx)
      +self : J.Val = bannerpeak.u.snd(Bool, J.Val, _t4)
      +blocked_x : Bool = bannerpeak.u.fst(Bool, J.Val, _t4)
      +self = bannerpeak.body.Body.step.if4(Bool.and(blocked_x, String.eq(J.str(J.get(self, "mode")), "Dive")), self)
      +_t5 = bannerpeak.body.Body.move_y(self, g, now, dy, k, J.u32(J.get(c, "x")), out)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t5)
      +out : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t5)
      +self = bannerpeak.body.Body.step.if5(Bool.and(Bool.not(J.bool(J.get(self, "ground"))), J.is_null(J.get(self, "plank"))), self)
      +self = J.set(self, "wall_t", J.JU{RS.u_sat_sub(J.u32(J.get(self, "wall_t")), 1)})
      +self = bannerpeak.body.Body.step.if6(Bool.and(Bool.not(J.bool(J.get(self, "ground"))), String.eq(J.str(J.get(self, "mode")), "Ground")), self)
      +self = bannerpeak.body.Body.step.if7(Bool.and(J.bool(J.get(self, "ground")), Bool.not(was_ground)), self, k, out)
      +out = bannerpeak.body.Body.step.if8(Bool.and(Bool.and(Bool.and(J.bool(J.get(self, "ground")), String.eq(J.str(J.get(self, "mode")), "Ground")), J.bool(J.get(c, "down_edge"))), U32.is_eq(bannerpeak.body.Body.tile_below_feet(self, g), 11)), out)
      +_t7 = bannerpeak.body.Body.settle_pose(self, c)
      +self : J.Val = _t7
      (self, out)

def bannerpeak.body.Body.carried?(+self: J.Val, +now: J.Val, +before: J.Val) -> J.Val:
  bannerpeak.body.Body.carried.if2(J.is_null(J.get(self, "plank")), self, now, before)

def bannerpeak.body.Body.carried.if1?(bpc: Bool, +self: J.Val, +a: J.Val, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +pa : J.Val = J.get(a, "p")
      +pb : J.Val = J.get(b, "p")
      +self = J.set(self, "x", J.of_i32((J.u32(J.get(self, "x")) + ((J.u32(J.get(pa, "x")) - J.u32(J.get(pb, "x")) : U32) * 256 : U32) : U32)))
      +self = J.set(self, "y", J.of_i32((J.u32(J.get(self, "y")) + ((J.u32(J.get(pa, "y")) - J.u32(J.get(pb, "y")) : U32) * 256 : U32) : U32)))
      self
    case False{}:
      +self = J.set(self, "plank", J.JNull{})
      +self = J.set(self, "ground", J.JBool{False{}})
      self

def bannerpeak.body.Body.carried.if2?(bpc: Bool, +self: J.Val, +now: J.Val, +before: J.Val) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +id : U32 = J.u32(J.get(self, "plank"))
      +a : J.Val = bannerpeak.body.Body.find_mover(now, id, 0)
      +b : J.Val = bannerpeak.body.Body.find_mover(before, id, 0)
      +self = bannerpeak.body.Body.carried.if1(Bool.and(Bool.not(J.is_null(a)), Bool.not(J.is_null(b))), self, a, b)
      self

def bannerpeak.body.Body.tile_below_feet?(+self: J.Val, +g: J.Val) -> U32:
  bannerpeak.body.Body.tile_below(self, g)

def bannerpeak.body.Body.modes?(+self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +_t15 = bannerpeak.body.Body.modes.if14(String.eq(J.str(J.get(self, "mode")), "Wind"), self, c, k, g, out)
  +out = bannerpeak.u.fst(J.Val, J.Val, _t15)
  +self = bannerpeak.u.snd(J.Val, J.Val, _t15)
  (self, out)

def bannerpeak.body.Body.modes.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Pound"})
      +self = J.set(self, "vy", J.of_i32(3200))
      +self = J.set(self, "t", J.JU{0})
      self
    case False{}:
      self

def bannerpeak.body.Body.modes.if2?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Ground"})
      self
    case False{}:
      self

def bannerpeak.body.Body.modes.if3?(bpc: Bool, +self: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_neg(2700)))
      +self = J.set(self, "mode", J.JS{"Air"})
      +self = J.set(self, "jump", J.JS{"Plain"})
      +self = J.set(self, "hop", J.JU{0})
      +self = J.set(self, "hop_t", J.JU{0})
      +self = J.set(self, "ground", J.JBool{False{}})
      +self = J.set(self, "plank", J.JNull{})
      +self = J.set(self, "buffer", J.JU{0})
      +self = J.set(self, "cut", J.JBool{True{}})
      +out = J.push(out, bannerpeak.body.Ev.jump("Plain"))
      (out, self)
    case False{}:
      +self = bannerpeak.body.Body.modes.if2(U32.is_ge(J.u32(J.get(self, "t")), 9), self)
      (out, self)

def bannerpeak.body.Body.modes.if4?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Ground"})
      self
    case False{}:
      +self = J.set(self, "mode", J.JS{"Air"})
      self

def bannerpeak.body.Body.modes.if5?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = bannerpeak.body.Body.modes.if4(J.bool(J.get(self, "ground")), self)
      self
    case False{}:
      self

def bannerpeak.body.Body.modes.if6?(bpc: Bool, +self: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_neg(RS.i_div((2200 * 70 : U32), 100))))
      +self = J.set(self, "mode", J.JS{"Air"})
      +self = J.set(self, "jump", J.JS{"Plain"})
      +self = J.set(self, "hop", J.JU{0})
      +self = J.set(self, "hop_t", J.JU{0})
      +self = J.set(self, "ground", J.JBool{False{}})
      +self = J.set(self, "plank", J.JNull{})
      +self = J.set(self, "buffer", J.JU{0})
      +out = J.push(out, bannerpeak.body.Ev.jump("Plain"))
      (out, self)
    case False{}:
      (out, self)

def bannerpeak.body.Body.modes.if7?(bpc: Bool, +self: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Ground"})
      (out, self)
    case False{}:
      +_t5 = bannerpeak.body.Body.modes.if6(Bool.and(U32.is_ge(J.u32(J.get(self, "t")), 3), U32.is_gt(J.u32(J.get(self, "buffer")), 0)), self, out)
      +out = bannerpeak.u.fst(J.Val, J.Val, _t5)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t5)
      (out, self)

def bannerpeak.body.Body.modes.if8?(bpc: Bool, +self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +out: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "t", J.JU{RS.u_min((J.u32(J.get(self, "t")) + 1 : U32), 5)})
      +self = J.set(self, "vx", J.of_i32(bannerpeak.body.approach(J.u32(J.get(self, "vx")), 0, 84)))
      +_t8 = bannerpeak.body.Body.gravity(self, c)
      +self : J.Val = _t8
      self
    case False{}:
      +_t9 = bannerpeak.body.Body.free(self, c, k, g, out)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t9)
      +out : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t9)
      self

def bannerpeak.body.Body.modes.if9?(bpc: Bool, +self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "t", J.JU{(J.u32(J.get(self, "t")) + 1 : U32)})
      +self = J.set(self, "vx", J.of_i32(bannerpeak.body.approach(J.u32(J.get(self, "vx")), 0, 30)))
      +_t6 = bannerpeak.body.Body.modes.if7(U32.is_ge(J.u32(J.get(self, "t")), 16), self, out)
      +out = bannerpeak.u.fst(J.Val, J.Val, _t6)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t6)
      +_t7 = bannerpeak.body.Body.gravity(self, c)
      +self : J.Val = _t7
      (out, self)
    case False{}:
      +self = bannerpeak.body.Body.modes.if8(String.eq(J.str(J.get(self, "mode")), "Brace"), self, c, k, g, out)
      (out, self)

def bannerpeak.body.Body.modes.if10?(bpc: Bool, +self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "t", J.JU{(J.u32(J.get(self, "t")) + 1 : U32)})
      +_t4 = bannerpeak.body.Body.gravity(self, c)
      +self : J.Val = _t4
      (out, self)
    case False{}:
      +_t10 = bannerpeak.body.Body.modes.if9(String.eq(J.str(J.get(self, "mode")), "Belly"), self, c, k, g, out)
      +out = bannerpeak.u.fst(J.Val, J.Val, _t10)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t10)
      (out, self)

def bannerpeak.body.Body.modes.if11?(bpc: Bool, +self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "t", J.JU{(J.u32(J.get(self, "t")) + 1 : U32)})
      +self = bannerpeak.body.Body.modes.if5(U32.is_ge(J.u32(J.get(self, "t")), 14), self)
      +_t3 = bannerpeak.body.Body.gravity(self, c)
      +self : J.Val = _t3
      (out, self)
    case False{}:
      +_t11 = bannerpeak.body.Body.modes.if10(String.eq(J.str(J.get(self, "mode")), "Dive"), self, c, k, g, out)
      +out = bannerpeak.u.fst(J.Val, J.Val, _t11)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t11)
      (out, self)

def bannerpeak.body.Body.modes.if12?(bpc: Bool, +self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "t", J.JU{(J.u32(J.get(self, "t")) + 1 : U32)})
      +self = J.set(self, "vx", J.of_i32(bannerpeak.body.approach(J.u32(J.get(self, "vx")), 0, 84)))
      +_t1 = bannerpeak.body.Body.modes.if3(Bool.and(U32.is_le(J.u32(J.get(self, "t")), 5), U32.is_gt(J.u32(J.get(self, "buffer")), 0)), self, out)
      +out = bannerpeak.u.fst(J.Val, J.Val, _t1)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t1)
      +_t2 = bannerpeak.body.Body.gravity(self, c)
      +self : J.Val = _t2
      (out, self)
    case False{}:
      +_t12 = bannerpeak.body.Body.modes.if11(String.eq(J.str(J.get(self, "mode")), "Hurt"), self, c, k, g, out)
      +out = bannerpeak.u.fst(J.Val, J.Val, _t12)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t12)
      (out, self)

def bannerpeak.body.Body.modes.if13?(bpc: Bool, +self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "vx", J.of_i32(0))
      +self = J.set(self, "vy", J.of_i32(3200))
      (out, self)
    case False{}:
      +_t13 = bannerpeak.body.Body.modes.if12(String.eq(J.str(J.get(self, "mode")), "Recover"), self, c, k, g, out)
      +out = bannerpeak.u.fst(J.Val, J.Val, _t13)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t13)
      (out, self)

def bannerpeak.body.Body.modes.if14?(bpc: Bool, +self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "vx", J.of_i32(0))
      +self = J.set(self, "vy", J.of_i32(0))
      +self = J.set(self, "t", J.JU{(J.u32(J.get(self, "t")) + 1 : U32)})
      +self = bannerpeak.body.Body.modes.if1(U32.is_ge(J.u32(J.get(self, "t")), 6), self)
      (out, self)
    case False{}:
      +_t14 = bannerpeak.body.Body.modes.if13(String.eq(J.str(J.get(self, "mode")), "Pound"), self, c, k, g, out)
      +out = bannerpeak.u.fst(J.Val, J.Val, _t14)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t14)
      (out, self)

def bannerpeak.body.Body.gravity?(+self: J.Val, +c: J.Val) -> J.Val:
  +g : U32 = 210
  +g = bannerpeak.body.Body.gravity.if4(Bool.or(String.eq(J.str(J.get(self, "mode")), "Dive"), String.eq(J.str(J.get(self, "mode")), "Hurt")), self, c, g)
  +self = bannerpeak.body.Body.gravity.if5(Bool.or(Bool.not(J.bool(J.get(self, "ground"))), RS.i_lt(J.u32(J.get(self, "vy")), 0)), self, g)
  self

def bannerpeak.body.Body.gravity.if1?(bpc: Bool, +g: U32) -> U32:
  match bpc:
    case True{}:
      +g : U32 = 150
      g
    case False{}:
      +g : U32 = 340
      g

def bannerpeak.body.Body.gravity.if2?(bpc: Bool, +self: J.Val, +c: J.Val, +g: U32) -> U32:
  match bpc:
    case True{}:
      +g = bannerpeak.body.Body.gravity.if1(Bool.or(J.bool(J.get(c, "jump")), Bool.not(bannerpeak.body.Body.cuttable(self))), g)
      g
    case False{}:
      g

def bannerpeak.body.Body.gravity.if3?(bpc: Bool, +self: J.Val, +c: J.Val, +g: U32) -> U32:
  match bpc:
    case True{}:
      +g : U32 = 90
      g
    case False{}:
      +g = bannerpeak.body.Body.gravity.if2(RS.i_lt(J.u32(J.get(self, "vy")), 0), self, c, g)
      g

def bannerpeak.body.Body.gravity.if4?(bpc: Bool, +self: J.Val, +c: J.Val, +g: U32) -> U32:
  match bpc:
    case True{}:
      +g : U32 = 210
      g
    case False{}:
      +g = bannerpeak.body.Body.gravity.if3(String.eq(J.str(J.get(self, "jump")), "Long"), self, c, g)
      g

def bannerpeak.body.Body.gravity.if5?(bpc: Bool, +self: J.Val, +g: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_min((J.u32(J.get(self, "vy")) + g : U32), 3584)))
      self
    case False{}:
      +self = J.set(self, "vy", J.of_i32(g))
      self

def bannerpeak.body.Body.cuttable?(+self: J.Val) -> Bool:
  Bool.or(String.eq(J.str(J.get(self, "jump")), "Plain"), String.eq(J.str(J.get(self, "jump")), "Second"))

def bannerpeak.body.Body.free?(+self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +_t1 = bannerpeak.body.Body.free_move(self, c, k, g, out)
  +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
  +out : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
  +braced : Bool = bannerpeak.u.fst(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t1)
  bannerpeak.body.Body.free.if4(braced, self, c, k, out)

def bannerpeak.body.Body.free.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_max(J.u32(J.get(self, "vy")), RS.i_neg(820))))
      +self = J.set(self, "cut", J.JBool{True{}})
      self
    case False{}:
      self

def bannerpeak.body.Body.free.if2?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_min(J.u32(J.get(self, "vy")), 560)))
      self
    case False{}:
      self

def bannerpeak.body.Body.free.if3?(bpc: Bool, +self: J.Val, +c: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      (self, out)
    case False{}:
      +self = bannerpeak.body.Body.free.if1(Bool.and(Bool.and(Bool.and(RS.i_lt(J.u32(J.get(self, "vy")), 0), Bool.not(J.bool(J.get(c, "jump")))), Bool.not(J.bool(J.get(self, "cut")))), bannerpeak.body.Body.cuttable(self)), self)
      +_t3 = bannerpeak.body.Body.gravity(self, c)
      +self : J.Val = _t3
      +self = bannerpeak.body.Body.free.if2(String.eq(J.str(J.get(self, "mode")), "Wall"), self)
      (self, out)

def bannerpeak.body.Body.free.if4?(bpc: Bool, +self: J.Val, +c: J.Val, +k: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      (self, out)
    case False{}:
      +_t2 = bannerpeak.body.Body.free_jump(self, c, k, out)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      +out : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      +stopped : Bool = bannerpeak.u.fst(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t2)
      bannerpeak.body.Body.free.if3(stopped, self, c, out)

def bannerpeak.body.Body.free_move?(+self: J.Val, +c: J.Val, +k: J.Val, +g: J.Val, +out: J.Val) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +top : U32 = J.u32(J.get(k, "run"))
  +slip_up : U32 = 1
  +slip_down : U32 = 1
  +_t1 = bannerpeak.body.Body.free_move.if1(Bool.and(J.bool(J.get(self, "ground")), bannerpeak.world.Tile.slippery(bannerpeak.body.Body.tile_below(self, g))), slip_up, slip_down)
  +slip_down = bannerpeak.u.fst(U32, U32, _t1)
  +slip_up = bannerpeak.u.snd(U32, U32, _t1)
  +dir : U32 = J.u32(J.get(c, "x"))
  bannerpeak.body.Body.free_move.if14(J.bool(J.get(self, "ground")), self, c, g, out, top, slip_up, slip_down, dir)

def bannerpeak.body.Body.free_move.if1?(bpc: Bool, +slip_up: U32, +slip_down: U32) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      +slip_up : U32 = 2
      +slip_down : U32 = 4
      (slip_down, slip_up)
    case False{}:
      (slip_down, slip_up)

def bannerpeak.body.Body.free_move.if2?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "crouch", J.JBool{False{}})
      self
    case False{}:
      self

def bannerpeak.body.Body.free_move.if3?(bpc: Bool, +self: J.Val, +c: J.Val, +g: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "crouch", J.JBool{True{}})
      self
    case False{}:
      +self = bannerpeak.body.Body.free_move.if2(Bool.and(Bool.and(Bool.not(J.bool(J.get(c, "down"))), J.bool(J.get(self, "crouch"))), bannerpeak.body.Body.can_stand(self, g)), self)
      self

def bannerpeak.body.Body.free_move.if4?(bpc: Bool, +fric: U32) -> U32:
  match bpc:
    case True{}:
      +fric : U32 = 14
      fric
    case False{}:
      fric

def bannerpeak.body.Body.free_move.if5?(bpc: Bool, +self: J.Val, +c: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "face", J.of_i32(J.u32(J.get(c, "x"))))
      self
    case False{}:
      self

def bannerpeak.body.Body.free_move.if6?(bpc: Bool, +out: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +out = J.push(out, bannerpeak.body.Ev.skid())
      out
    case False{}:
      out

def bannerpeak.body.Body.free_move.if7?(bpc: Bool, +self: J.Val, +c: J.Val, +out: J.Val, +top: U32, +slip_up: U32, +slip_down: U32, +dir: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "vx", J.of_i32(bannerpeak.body.approach(J.u32(J.get(self, "vx")), 0, RS.i_div(184, RS.i_min(slip_down, 3)))))
      +self = J.set(self, "skid", J.JBool{True{}})
      +out = bannerpeak.body.Body.free_move.if6(Bool.not(J.bool(J.get(self, "was_skid"))), out)
      (out, self)
    case False{}:
      +self = J.set(self, "vx", J.of_i32(bannerpeak.body.approach(J.u32(J.get(self, "vx")), (dir * top : U32), RS.i_div(56, slip_up))))
      +self = J.set(self, "face", J.of_i32(J.u32(J.get(c, "x"))))
      (out, self)

def bannerpeak.body.Body.free_move.if8?(bpc: Bool, +self: J.Val, +c: J.Val, +out: J.Val, +top: U32, +slip_up: U32, +slip_down: U32, +dir: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "vx", J.of_i32(bannerpeak.body.approach(J.u32(J.get(self, "vx")), 0, RS.i_div(84, slip_down))))
      (out, self)
    case False{}:
      +_t2 = bannerpeak.body.Body.free_move.if7(Bool.and(Bool.and(U32.is_ne(J.u32(J.get(self, "vx")), 0), U32.is_ne(RS.i_signum(J.u32(J.get(self, "vx"))), RS.i_signum(dir))), RS.i_gt(RS.i_abs(J.u32(J.get(self, "vx"))), 520)), self, c, out, top, slip_up, slip_down, dir)
      +out = bannerpeak.u.fst(J.Val, J.Val, _t2)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t2)
      (out, self)

def bannerpeak.body.Body.free_move.if9?(bpc: Bool, +self: J.Val, +c: J.Val, +out: J.Val, +top: U32, +slip_up: U32, +slip_down: U32, +dir: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +fric : U32 = 110
      +fric = bannerpeak.body.Body.free_move.if4(RS.i_ge(RS.i_abs(J.u32(J.get(self, "vx"))), 420), fric)
      +self = J.set(self, "vx", J.of_i32(bannerpeak.body.approach(J.u32(J.get(self, "vx")), 0, RS.i_div(fric, slip_down))))
      +self = bannerpeak.body.Body.free_move.if5(Bool.and(U32.is_ne(dir, 0), RS.i_lt(RS.i_abs(J.u32(J.get(self, "vx"))), 420)), self, c)
      (out, self)
    case False{}:
      +_t3 = bannerpeak.body.Body.free_move.if8(U32.is_eq(dir, 0), self, c, out, top, slip_up, slip_down, dir)
      +out = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t3)
      (out, self)

def bannerpeak.body.Body.free_move.if10?(bpc: Bool, +self: J.Val, +c: J.Val, +out: J.Val, +top: U32, +slip_up: U32, +slip_down: U32, +dir: U32) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Brace"})
      +self = J.set(self, "t", J.JU{0})
      +self = J.set(self, "crouch", J.JBool{False{}})
      +self = J.set(self, "vx", J.of_i32(0))
      +self = J.set(self, "vy", J.of_i32(210))
      (True{}, (self, out))
    case False{}:
      +_t4 = bannerpeak.body.Body.free_move.if9(J.bool(J.get(self, "crouch")), self, c, out, top, slip_up, slip_down, dir)
      +out = bannerpeak.u.fst(J.Val, J.Val, _t4)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t4)
      (False{}, (self, out))

def bannerpeak.body.Body.free_move.if11?(bpc: Bool, +self: J.Val, +top: U32, +dir: U32) -> J.Val:
  match bpc:
    case True{}:
      +target : U32 = (dir * top : U32)
      +self = J.set(self, "vx", J.of_i32(bannerpeak.body.approach(J.u32(J.get(self, "vx")), target, 44)))
      self
    case False{}:
      self

def bannerpeak.body.Body.free_move.if12?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vx", J.of_i32(bannerpeak.body.approach(J.u32(J.get(self, "vx")), 0, 10)))
      self
    case False{}:
      self

def bannerpeak.body.Body.free_move.if13?(bpc: Bool, +self: J.Val, +c: J.Val, +top: U32, +dir: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "face", J.of_i32(J.u32(J.get(c, "x"))))
      +aligned : Bool = U32.is_eq(RS.i_signum(J.u32(J.get(self, "vx"))), RS.i_signum(dir))
      +self = bannerpeak.body.Body.free_move.if11(Bool.or(Bool.not(aligned), RS.i_le(RS.i_abs(J.u32(J.get(self, "vx"))), top)), self, top, dir)
      self
    case False{}:
      +self = bannerpeak.body.Body.free_move.if12(Bool.not(Bool.or(Bool.or(String.eq(J.str(J.get(self, "jump")), "Long"), String.eq(J.str(J.get(self, "jump")), "Kick")), String.eq(J.str(J.get(self, "jump")), "Back"))), self)
      self

def bannerpeak.body.Body.free_move.if14?(bpc: Bool, +self: J.Val, +c: J.Val, +g: J.Val, +out: J.Val, +top: U32, +slip_up: U32, +slip_down: U32, +dir: U32) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +self = bannerpeak.body.Body.free_move.if3(Bool.and(J.bool(J.get(c, "down")), Bool.not(J.bool(J.get(self, "crouch")))), self, c, g)
      bannerpeak.body.Body.free_move.if10(Bool.and(Bool.and(Bool.and(Bool.and(J.bool(J.get(c, "brace")), Bool.not(J.bool(J.get(c, "down")))), RS.i_lt(RS.i_abs(J.u32(J.get(self, "vx"))), 260)), Bool.not(J.bool(J.get(c, "jump_edge")))), U32.is_eq(J.u32(J.get(self, "buffer")), 0)), self, c, out, top, slip_up, slip_down, dir)
    case False{}:
      +self = J.set(self, "crouch", J.JBool{False{}})
      +self = bannerpeak.body.Body.free_move.if13(U32.is_ne(dir, 0), self, c, top, dir)
      (False{}, (self, out))

def bannerpeak.body.Body.free_jump?(+self: J.Val, +c: J.Val, +k: J.Val, +out: J.Val) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +leave : String = ""
  +leave = bannerpeak.body.Body.free_jump.if4(U32.is_gt(J.u32(J.get(self, "buffer")), 0), self, k, leave)
  bannerpeak.body.Body.free_jump.if8(Bool.not(String.eq(leave, "")), self, c, k, out, leave)

def bannerpeak.body.Body.free_jump.if1?(bpc: Bool, +leave: String) -> String:
  match bpc:
    case True{}:
      +leave : String = "Kick"
      leave
    case False{}:
      leave

def bannerpeak.body.Body.free_jump.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +leave: String) -> String:
  match bpc:
    case True{}:
      +leave : String = bannerpeak.body.Body.pick_jump(self, k)
      leave
    case False{}:
      +leave = bannerpeak.body.Body.free_jump.if1(Bool.and(U32.is_gt(J.u32(J.get(self, "wall_t")), 0), U32.is_ne(J.u32(J.get(self, "wall")), 0)), leave)
      leave

def bannerpeak.body.Body.free_jump.if3?(bpc: Bool, +self: J.Val, +k: J.Val, +leave: String) -> String:
  match bpc:
    case True{}:
      +leave : String = "Kick"
      leave
    case False{}:
      +leave = bannerpeak.body.Body.free_jump.if2(Bool.or(J.bool(J.get(self, "ground")), U32.is_gt(J.u32(J.get(self, "coyote")), 0)), self, k, leave)
      leave

def bannerpeak.body.Body.free_jump.if4?(bpc: Bool, +self: J.Val, +k: J.Val, +leave: String) -> String:
  match bpc:
    case True{}:
      +leave = bannerpeak.body.Body.free_jump.if3(Bool.and(String.eq(J.str(J.get(self, "mode")), "Wall"), Bool.not(J.bool(J.get(self, "ground")))), self, k, leave)
      leave
    case False{}:
      leave

def bannerpeak.body.Body.free_jump.if5?(bpc: Bool, +self: J.Val, +out: J.Val) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Dive"})
      +self = J.set(self, "t", J.JU{0})
      +self = J.set(self, "vx", J.of_i32((J.u32(J.get(self, "face")) * 1800 : U32)))
      +self = J.set(self, "vy", J.of_i32((RS.i_max(J.u32(J.get(self, "vy")), 0) + 700 : U32)))
      +self = J.set(self, "wall_t", J.JU{0})
      (True{}, (self, out))
    case False{}:
      (False{}, (self, out))

def bannerpeak.body.Body.free_jump.if6?(bpc: Bool, +self: J.Val, +c: J.Val, +out: J.Val) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Wind"})
      +self = J.set(self, "t", J.JU{0})
      +self = J.set(self, "vx", J.of_i32(0))
      +self = J.set(self, "vy", J.of_i32(0))
      +self = J.set(self, "crouch", J.JBool{False{}})
      +self = J.set(self, "wall_t", J.JU{0})
      (True{}, (self, out))
    case False{}:
      bannerpeak.body.Body.free_jump.if5(J.bool(J.get(c, "dive_edge")), self, out)

def bannerpeak.body.Body.free_jump.if7?(bpc: Bool, +self: J.Val, +c: J.Val, +out: J.Val) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      bannerpeak.body.Body.free_jump.if6(J.bool(J.get(c, "down_edge")), self, c, out)
    case False{}:
      (False{}, (self, out))

def bannerpeak.body.Body.free_jump.if8?(bpc: Bool, +self: J.Val, +c: J.Val, +k: J.Val, +out: J.Val, +leave: String) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.body.Body.launch(self, leave, k)
      +self : J.Val = _t1
      +out = J.push(out, bannerpeak.body.Ev.jump(leave))
      (False{}, (self, out))
    case False{}:
      bannerpeak.body.Body.free_jump.if7(Bool.not(J.bool(J.get(self, "ground"))), self, c, out)

def bannerpeak.body.Body.pick_jump?(+self: J.Val, +k: J.Val) -> String:
  +speed : U32 = RS.i_abs(J.u32(J.get(self, "vx")))
  bannerpeak.body.Body.pick_jump.if5(Bool.and(Bool.not(J.is_null(J.get(self, "plank"))), U32.is_ge(J.u32(J.get(self, "plank")), 1000)), self, speed)

def bannerpeak.body.Body.pick_jump.if1?(bpc: Bool) -> String:
  match bpc:
    case True{}:
      "Long"
    case False{}:
      "Back"

def bannerpeak.body.Body.pick_jump.if2?(bpc: Bool) -> String:
  match bpc:
    case True{}:
      "Triple"
    case False{}:
      "Second"

def bannerpeak.body.Body.pick_jump.if3?(bpc: Bool, +self: J.Val) -> String:
  match bpc:
    case True{}:
      bannerpeak.body.Body.pick_jump.if2(U32.is_ge(J.u32(J.get(self, "hop_next")), 3))
    case False{}:
      "Plain"

def bannerpeak.body.Body.pick_jump.if4?(bpc: Bool, +self: J.Val, +speed: U32) -> String:
  match bpc:
    case True{}:
      bannerpeak.body.Body.pick_jump.if1(RS.i_ge(speed, 760))
    case False{}:
      bannerpeak.body.Body.pick_jump.if3(Bool.and(Bool.and(U32.is_gt(J.u32(J.get(self, "hop_t")), 0), U32.is_ge(J.u32(J.get(self, "hop_next")), 2)), RS.i_ge(speed, 560)), self)

def bannerpeak.body.Body.pick_jump.if5?(bpc: Bool, +self: J.Val, +speed: U32) -> String:
  match bpc:
    case True{}:
      "Spring"
    case False{}:
      bannerpeak.body.Body.pick_jump.if4(J.bool(J.get(self, "crouch")), self, speed)

def bannerpeak.body.Body.launch?(+self: J.Val, +kind: String, +k: J.Val) -> J.Val:
  +speed : U32 = RS.i_abs(J.u32(J.get(self, "vx")))
  +bonus : U32 = RS.i_div(speed, 5)
  +self = bannerpeak.body.Body.launch.if8(String.eq(kind, "Plain"), self, kind, k, speed, bonus)
  +self = J.set(self, "jump", J.JS{kind})
  +self = J.set(self, "mode", J.JS{"Air"})
  +self = J.set(self, "ground", J.JBool{False{}})
  +self = J.set(self, "plank", J.JNull{})
  +self = J.set(self, "crouch", J.JBool{False{}})
  +self = J.set(self, "coyote", J.JU{0})
  +self = J.set(self, "buffer", J.JU{0})
  +self = J.set(self, "hop_t", J.JU{0})
  +self = J.set(self, "hop_next", J.JU{0})
  +self = J.set(self, "cut", J.JBool{False{}})
  +self = J.set(self, "wall_t", J.JU{0})
  +self = J.set(self, "t", J.JU{0})
  self

def bannerpeak.body.Body.launch.if1?(bpc: Bool, +self: J.Val, +dir: U32) -> U32:
  match bpc:
    case True{}:
      +dir : U32 = RS.i_signum(J.u32(J.get(self, "vx")))
      dir
    case False{}:
      dir

def bannerpeak.body.Body.launch.if2?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_neg(1900)))
      +self = J.set(self, "vx", J.of_i32((RS.i_neg(J.u32(J.get(self, "wall"))) * 1150 : U32)))
      +self = J.set(self, "face", J.of_i32(RS.i_neg(J.u32(J.get(self, "wall")))))
      +self = J.set(self, "hop", J.JU{0})
      self
    case False{}:
      self

def bannerpeak.body.Body.launch.if3?(bpc: Bool, +self: J.Val, +kind: String) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_neg(2800)))
      +self = J.set(self, "vx", J.of_i32((RS.i_neg(J.u32(J.get(self, "face"))) * 300 : U32)))
      +self = J.set(self, "hop", J.JU{0})
      self
    case False{}:
      +self = bannerpeak.body.Body.launch.if2(String.eq(kind, "Kick"), self)
      self

def bannerpeak.body.Body.launch.if4?(bpc: Bool, +self: J.Val, +kind: String, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_neg(J.u32(J.get(k, "ledge")))))
      +self = J.set(self, "hop", J.JU{0})
      self
    case False{}:
      +self = bannerpeak.body.Body.launch.if3(String.eq(kind, "Back"), self, kind)
      self

def bannerpeak.body.Body.launch.if5?(bpc: Bool, +self: J.Val, +kind: String, +k: J.Val, +speed: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_neg(1560)))
      +dir : U32 = J.u32(J.get(self, "face"))
      +dir = bannerpeak.body.Body.launch.if1(U32.is_ne(J.u32(J.get(self, "vx")), 0), self, dir)
      +self = J.set(self, "vx", J.of_i32((dir * RS.i_min(RS.i_div((speed * 145 : U32), 100), 1900) : U32)))
      +self = J.set(self, "face", J.of_i32(dir))
      +self = J.set(self, "hop", J.JU{0})
      self
    case False{}:
      +self = bannerpeak.body.Body.launch.if4(String.eq(kind, "Spring"), self, kind, k)
      self

def bannerpeak.body.Body.launch.if6?(bpc: Bool, +self: J.Val, +kind: String, +k: J.Val, +speed: U32, +bonus: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_neg((RS.i_div((2200 * 133 : U32), 100) + bonus : U32))))
      +self = J.set(self, "hop", J.JU{3})
      self
    case False{}:
      +self = bannerpeak.body.Body.launch.if5(String.eq(kind, "Long"), self, kind, k, speed)
      self

def bannerpeak.body.Body.launch.if7?(bpc: Bool, +self: J.Val, +kind: String, +k: J.Val, +speed: U32, +bonus: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_neg((RS.i_div((2200 * 111 : U32), 100) + bonus : U32))))
      +self = J.set(self, "hop", J.JU{2})
      self
    case False{}:
      +self = bannerpeak.body.Body.launch.if6(String.eq(kind, "Triple"), self, kind, k, speed, bonus)
      self

def bannerpeak.body.Body.launch.if8?(bpc: Bool, +self: J.Val, +kind: String, +k: J.Val, +speed: U32, +bonus: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(RS.i_neg((2200 + bonus : U32))))
      +self = J.set(self, "hop", J.JU{1})
      self
    case False{}:
      +self = bannerpeak.body.Body.launch.if7(String.eq(kind, "Second"), self, kind, k, speed, bonus)
      self

def bannerpeak.body.Body.landed?(+self: J.Val, +k: J.Val, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +fell : U32 = RS.i_max(J.u32(J.get(self, "vy")), 0)
  +was : String = J.str(J.get(self, "mode"))
  +self = J.set(self, "vy", J.of_i32(0))
  +self = J.set(self, "coyote", J.JU{(J.u32(J.get(k, "grace")) + 2 : U32)})
  +self = J.set(self, "cut", J.JBool{False{}})
  +speed : U32 = RS.i_abs(J.u32(J.get(self, "vx")))
  +self = bannerpeak.body.Body.landed.if2(Bool.or(String.eq(J.str(J.get(self, "jump")), "Plain"), String.eq(J.str(J.get(self, "jump")), "Second")), self, k, speed)
  +self = J.set(self, "hop", J.JU{0})
  +self = J.set(self, "jump", J.JS{"None"})
  +self = J.set(self, "wall", J.of_i32(0))
  +_t1 = bannerpeak.body.Body.landed.if5(Bool.or(String.eq(was, "Pound"), String.eq(was, "Wind")), self, out, was)
  +out = bannerpeak.u.fst(J.Val, J.Val, _t1)
  +self = bannerpeak.u.snd(J.Val, J.Val, _t1)
  +self = bannerpeak.body.Body.landed.if6(RS.i_gt(fell, 1700), self)
  +out = J.push(out, bannerpeak.body.Ev.land(fell))
  (self, out)

def bannerpeak.body.Body.landed.if1?(bpc: Bool, +self: J.Val, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "hop_next", J.JU{(J.u32(J.get(self, "hop")) + 1 : U32)})
      +self = J.set(self, "hop_t", J.JU{(J.u32(J.get(k, "hop")) + 1 : U32)})
      self
    case False{}:
      +self = J.set(self, "hop_next", J.JU{0})
      +self = J.set(self, "hop_t", J.JU{0})
      self

def bannerpeak.body.Body.landed.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +speed: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = bannerpeak.body.Body.landed.if1(Bool.and(Bool.and(U32.is_ge(J.u32(J.get(self, "hop")), 1), U32.is_le(J.u32(J.get(self, "hop")), 2)), RS.i_ge(speed, 560)), self, k)
      self
    case False{}:
      +self = J.set(self, "hop_next", J.JU{0})
      +self = J.set(self, "hop_t", J.JU{0})
      self

def bannerpeak.body.Body.landed.if3?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "t", J.JU{J.u32(J.get(self, "t"))})
      self
    case False{}:
      +self = J.set(self, "mode", J.JS{"Ground"})
      +self = J.set(self, "t", J.JU{0})
      self

def bannerpeak.body.Body.landed.if4?(bpc: Bool, +self: J.Val, +was: String) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Belly"})
      +self = J.set(self, "t", J.JU{0})
      self
    case False{}:
      +self = bannerpeak.body.Body.landed.if3(String.eq(was, "Hurt"), self)
      self

def bannerpeak.body.Body.landed.if5?(bpc: Bool, +self: J.Val, +out: J.Val, +was: String) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Recover"})
      +self = J.set(self, "t", J.JU{0})
      +self = J.set(self, "freeze", J.JU{3})
      +self = J.set(self, "hop_next", J.JU{0})
      +self = J.set(self, "hop_t", J.JU{0})
      +out = J.push(out, bannerpeak.body.Ev.slam(RS.i_div_euclid(J.u32(J.get(self, "x")), 256), RS.i_div_euclid(J.u32(J.get(self, "y")), 256)))
      (out, self)
    case False{}:
      +self = bannerpeak.body.Body.landed.if4(String.eq(was, "Dive"), self, was)
      (out, self)

def bannerpeak.body.Body.landed.if6?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "squash", J.JU{4})
      self
    case False{}:
      self

def bannerpeak.body.Body.unstick?(+self: J.Val, +g: J.Val) -> J.Val:
  +_t1 = bannerpeak.body.Body.unstick_n(self, g, 16)
  +self : J.Val = _t1
  self

def bannerpeak.body.Body.unstick_n?(+self: J.Val, +g: J.Val, +n: U32) -> J.Val:
  bannerpeak.body.Body.unstick_n.if2(U32.is_eq(n, 0), self, g, n)

def bannerpeak.body.Body.unstick_n.if1?(bpc: Bool, +self: J.Val, +g: J.Val, +n: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +self = J.set(self, "y", J.of_i32((J.u32(J.get(self, "y")) - 256 : U32)))
      +_t2 = bannerpeak.body.Body.unstick_n(self, g, (n - 1 : U32))
      +self : J.Val = _t2
      self

def bannerpeak.body.Body.unstick_n.if2?(bpc: Bool, +self: J.Val, +g: J.Val, +n: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.body.Body.rect(self)
      +l = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)
      +t = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1))
      +r = bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
      +b = bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
      bannerpeak.body.Body.unstick_n.if1(Bool.not(bannerpeak.body.Body.blocked(g, l, (t + 1 : U32), r, (b - 1 : U32))), self, g, n)

def bannerpeak.body.Body.solid_rows?(+g: J.Val, +col: U32, +row: U32, +bottom: U32) -> Bool:
  bannerpeak.body.Body.solid_rows.if2(RS.i_gt(row, bottom), g, col, row, bottom)

def bannerpeak.body.Body.solid_rows.if1?(bpc: Bool, +g: J.Val, +col: U32, +row: U32, +bottom: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.body.Body.solid_rows(g, col, (row + 1 : U32), bottom)

def bannerpeak.body.Body.solid_rows.if2?(bpc: Bool, +g: J.Val, +col: U32, +row: U32, +bottom: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.body.Body.solid_rows.if1(bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, col, row)), g, col, row, bottom)

def bannerpeak.body.Body.move_x?(+self: J.Val, +g: J.Val, +dx: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  bannerpeak.body.Body.move_x.if4(U32.is_eq(dx, 0), self, g, dx)

def bannerpeak.body.Body.move_x.if1?(bpc: Bool, +half: U32, +nx: U32, +edge: U32) -> U32:
  match bpc:
    case True{}:
      +edge : U32 = ((nx + half : U32) - 1 : U32)
      edge
    case False{}:
      edge

def bannerpeak.body.Body.move_x.if2?(bpc: Bool, +self: J.Val, +half: U32, +col: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "x", J.of_i32(((col * 4096 : U32) - half : U32)))
      self
    case False{}:
      +self = J.set(self, "x", J.of_i32((((col + 1 : U32) * 4096 : U32) + half : U32)))
      self

def bannerpeak.body.Body.move_x.if3?(bpc: Bool, +self: J.Val, +dx: U32, +half: U32, +nx: U32, +col: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +self = bannerpeak.body.Body.move_x.if2(RS.i_gt(dx, 0), self, half, col)
      +self = J.set(self, "vx", J.of_i32(0))
      (True{}, self)
    case False{}:
      +self = J.set(self, "x", J.of_i32(nx))
      (False{}, self)

def bannerpeak.body.Body.move_x.if4?(bpc: Bool, +self: J.Val, +g: J.Val, +dx: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      (False{}, self)
    case False{}:
      +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.dims(self)
      +w = bannerpeak.u.fst(U32, U32, _t1)
      +h = bannerpeak.u.snd(U32, U32, _t1)
      +half : U32 = RS.i_div((w * 256 : U32), 2)
      +nx : U32 = (J.u32(J.get(self, "x")) + dx : U32)
      +edge : U32 = (nx - half : U32)
      +edge = bannerpeak.body.Body.move_x.if1(RS.i_gt(dx, 0), half, nx, edge)
      +col : U32 = RS.i_div_euclid(edge, 4096)
      +top : U32 = RS.i_div_euclid((J.u32(J.get(self, "y")) - (h * 256 : U32) : U32), 4096)
      +bottom : U32 = RS.i_div_euclid((J.u32(J.get(self, "y")) - 1 : U32), 4096)
      bannerpeak.body.Body.move_x.if3(bannerpeak.body.Body.solid_rows(g, col, top, bottom), self, dx, half, nx, col)

def bannerpeak.body.Body.stop_rows?(+g: J.Val, +row: U32, +last: U32, +c0: U32, +c1: U32, +old: U32) -> Sigma<&2, &2, Bool, _ => U32>:
  bannerpeak.body.Body.stop_rows.if2(RS.i_gt(row, last), g, row, last, c0, c1, old)

def bannerpeak.body.Body.stop_rows.if1?(bpc: Bool, +g: J.Val, +row: U32, +last: U32, +c0: U32, +c1: U32, +old: U32, +top: U32) -> Sigma<&2, &2, Bool, _ => U32>:
  match bpc:
    case True{}:
      (True{}, top)
    case False{}:
      bannerpeak.body.Body.stop_rows(g, (row + 1 : U32), last, c0, c1, old)

def bannerpeak.body.Body.stop_rows.if2?(bpc: Bool, +g: J.Val, +row: U32, +last: U32, +c0: U32, +c1: U32, +old: U32) -> Sigma<&2, &2, Bool, _ => U32>:
  match bpc:
    case True{}:
      (False{}, 0)
    case False{}:
      +top : U32 = (row * 4096 : U32)
      bannerpeak.body.Body.stop_rows.if1(bannerpeak.body.Body.stop_cols(g, row, c0, c1, old, top), g, row, last, c0, c1, old, top)

def bannerpeak.body.Body.stop_cols?(+g: J.Val, +row: U32, +col: U32, +c1: U32, +old: U32, +top: U32) -> Bool:
  bannerpeak.body.Body.stop_cols.if2(RS.i_gt(col, c1), g, row, col, c1, old, top)

def bannerpeak.body.Body.stop_cols.if1?(bpc: Bool, +g: J.Val, +row: U32, +col: U32, +c1: U32, +old: U32, +top: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.body.Body.stop_cols(g, row, (col + 1 : U32), c1, old, top)

def bannerpeak.body.Body.stop_cols.if2?(bpc: Bool, +g: J.Val, +row: U32, +col: U32, +c1: U32, +old: U32, +top: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +t : U32 = bannerpeak.session.View.tile(g, col, row)
      bannerpeak.body.Body.stop_cols.if1(Bool.or(Bool.and(bannerpeak.world.Tile.solid(t), RS.i_le(old, top)), Bool.and(bannerpeak.world.Tile.one_way(t), RS.i_le(old, (top + (6 * 256 : U32) : U32)))), g, row, col, c1, old, top)

def bannerpeak.body.Body.landing?(+ms: J.Val, +i: U32, +old_y: U32, +y: U32, +x: U32) -> U32:
  bannerpeak.body.Body.landing.if2(RS.i_lt(i, 0), ms, i, old_y, y, x)

def bannerpeak.body.Body.landing.if1?(bpc: Bool, +ms: J.Val, +i: U32, +old_y: U32, +y: U32, +x: U32) -> U32:
  match bpc:
    case True{}:
      i
    case False{}:
      bannerpeak.body.Body.landing(ms, (i - 1 : U32), old_y, y, x)

def bannerpeak.body.Body.landing.if2?(bpc: Bool, +ms: J.Val, +i: U32, +old_y: U32, +y: U32, +x: U32) -> U32:
  match bpc:
    case True{}:
      4294967295
    case False{}:
      +p : J.Val = J.get(J.nth(ms, i), "p")
      bannerpeak.body.Body.landing.if1(Bool.and(Bool.and(Bool.and(RS.i_le(old_y, (J.u32(J.get(p, "y")) * 256 : U32)), RS.i_ge(y, (J.u32(J.get(p, "y")) * 256 : U32))), RS.i_ge(x, (J.u32(J.get(p, "x")) - 4 : U32))), RS.i_lt(x, ((J.u32(J.get(p, "x")) + J.u32(J.get(p, "w")) : U32) + 4 : U32))), ms, i, old_y, y, x)

def bannerpeak.body.Body.move_y?(+self: J.Val, +g: J.Val, +now: J.Val, +dy: U32, +k: J.Val, +toward: U32, +out: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.dims(self)
  +w = bannerpeak.u.fst(U32, U32, _t1)
  +h = bannerpeak.u.snd(U32, U32, _t1)
  +half : U32 = RS.i_div((w * 256 : U32), 2)
  +old : U32 = J.u32(J.get(self, "y"))
  +new : U32 = (old + dy : U32)
  bannerpeak.body.Body.move_y.if7(RS.i_ge(dy, 0), self, g, now, dy, k, toward, out, h, half, old, new)

def bannerpeak.body.Body.move_y.if1?(bpc: Bool, +now: J.Val, +stop: Bool, +stop_top: U32, +on: J.Val, +i: U32, +ptop: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, Bool, _ => U32>>:
  match bpc:
    case True{}:
      +stop : Bool = True{}
      +stop_top : U32 = ptop
      +on : J.Val = J.JU{J.u32(J.get(J.nth(now, i), "id"))}
      (on, (stop, stop_top))
    case False{}:
      (on, (stop, stop_top))

def bannerpeak.body.Body.move_y.if2?(bpc: Bool, +now: J.Val, +stop: Bool, +stop_top: U32, +on: J.Val, +i: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, Bool, _ => U32>>:
  match bpc:
    case True{}:
      +ptop : U32 = (J.u32(J.get(J.get(J.nth(now, i), "p"), "y")) * 256 : U32)
      +_t3 = bannerpeak.body.Body.move_y.if1(Bool.or(Bool.not(stop), RS.i_lt(ptop, stop_top)), now, stop, stop_top, on, i, ptop)
      +on = bannerpeak.u.fst(J.Val, Sigma<&2, &2, Bool, _ => U32>, _t3)
      +stop = bannerpeak.u.fst(Bool, U32, bannerpeak.u.snd(J.Val, Sigma<&2, &2, Bool, _ => U32>, _t3))
      +stop_top = bannerpeak.u.snd(Bool, U32, bannerpeak.u.snd(J.Val, Sigma<&2, &2, Bool, _ => U32>, _t3))
      (on, (stop, stop_top))
    case False{}:
      (on, (stop, stop_top))

def bannerpeak.body.Body.move_y.if3?(bpc: Bool, +self: J.Val, +now: J.Val, +old: U32, +new: U32, +stop: Bool, +stop_top: U32, +on: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, Bool, _ => U32>>:
  match bpc:
    case True{}:
      +i : U32 = bannerpeak.body.Body.landing(now, (J.len(now) - 1 : U32), old, new, RS.i_div_euclid(J.u32(J.get(self, "x")), 256))
      +_t4 = bannerpeak.body.Body.move_y.if2(RS.i_ge(i, 0), now, stop, stop_top, on, i)
      +on = bannerpeak.u.fst(J.Val, Sigma<&2, &2, Bool, _ => U32>, _t4)
      +stop = bannerpeak.u.fst(Bool, U32, bannerpeak.u.snd(J.Val, Sigma<&2, &2, Bool, _ => U32>, _t4))
      +stop_top = bannerpeak.u.snd(Bool, U32, bannerpeak.u.snd(J.Val, Sigma<&2, &2, Bool, _ => U32>, _t4))
      (on, (stop, stop_top))
    case False{}:
      (on, (stop, stop_top))

def bannerpeak.body.Body.move_y.if4?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Ground"})
      self
    case False{}:
      self

def bannerpeak.body.Body.move_y.if5?(bpc: Bool, +self: J.Val, +g: J.Val, +k: J.Val, +toward: U32, +out: J.Val, +new: U32, +stop_top: U32, +on: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "y", J.of_i32(stop_top))
      +self = J.set(self, "ground", J.JBool{True{}})
      +self = J.set(self, "plank", on)
      +self = bannerpeak.body.Body.move_y.if4(String.eq(J.str(J.get(self, "mode")), "Wall"), self)
      (self, out)
    case False{}:
      +self = J.set(self, "y", J.of_i32(new))
      +self = J.set(self, "ground", J.JBool{False{}})
      +self = J.set(self, "plank", J.JNull{})
      +_t6 = bannerpeak.body.Body.wall_check(self, g, k, toward)
      +self : J.Val = _t6
      (self, out)

def bannerpeak.body.Body.move_y.if6?(bpc: Bool, +self: J.Val, +g: J.Val, +k: J.Val, +toward: U32, +new: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "y", J.of_i32(new))
      +_t8 = bannerpeak.body.Body.wall_check(self, g, k, toward)
      +self : J.Val = _t8
      self
    case False{}:
      self

def bannerpeak.body.Body.move_y.if7?(bpc: Bool, +self: J.Val, +g: J.Val, +now: J.Val, +dy: U32, +k: J.Val, +toward: U32, +out: J.Val, +h: U32, +half: U32, +old: U32, +new: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +c0 : U32 = RS.i_div_euclid((J.u32(J.get(self, "x")) - half : U32), 4096)
      +c1 : U32 = RS.i_div_euclid(((J.u32(J.get(self, "x")) + half : U32) - 1 : U32), 4096)
      +first : U32 = RS.i_div_euclid((((old - (6 * 256 : U32) : U32) + 4096 : U32) - 1 : U32), 4096)
      +last : U32 = RS.i_div_euclid(new, 4096)
      +_t2 : Sigma<&2, &2, Bool, _ => U32> = bannerpeak.body.Body.stop_rows(g, first, last, c0, c1, old)
      +found = bannerpeak.u.fst(Bool, U32, _t2)
      +top = bannerpeak.u.snd(Bool, U32, _t2)
      +stop : Bool = found
      +stop_top : U32 = top
      +on : J.Val = J.JNull{}
      +_t5 = bannerpeak.body.Body.move_y.if3(RS.i_ge(J.u32(J.get(self, "vy")), 0), self, now, old, new, stop, stop_top, on)
      +on = bannerpeak.u.fst(J.Val, Sigma<&2, &2, Bool, _ => U32>, _t5)
      +stop = bannerpeak.u.fst(Bool, U32, bannerpeak.u.snd(J.Val, Sigma<&2, &2, Bool, _ => U32>, _t5))
      +stop_top = bannerpeak.u.snd(Bool, U32, bannerpeak.u.snd(J.Val, Sigma<&2, &2, Bool, _ => U32>, _t5))
      bannerpeak.body.Body.move_y.if5(stop, self, g, k, toward, out, new, stop_top, on)
    case False{}:
      +head : U32 = (old - (h * 256 : U32) : U32)
      +new_head : U32 = (head + dy : U32)
      +self = J.set(self, "ground", J.JBool{False{}})
      +self = J.set(self, "plank", J.JNull{})
      +_t7 = bannerpeak.body.Body.rise_rows(self, g, RS.i_div_euclid((head - 1 : U32), 4096), RS.i_div_euclid(new_head, 4096), half, h, new, new_head, head, out)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t7))
      +out : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t7))
      +done : Bool = bannerpeak.u.fst(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t7)
      +self = bannerpeak.body.Body.move_y.if6(Bool.not(done), self, g, k, toward, new)
      (self, out)

def bannerpeak.body.Body.any_solid?(+g: J.Val, +col: U32, +c1: U32, +row: U32) -> Bool:
  bannerpeak.body.Body.any_solid.if2(RS.i_gt(col, c1), g, col, c1, row)

def bannerpeak.body.Body.any_solid.if1?(bpc: Bool, +g: J.Val, +col: U32, +c1: U32, +row: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.body.Body.any_solid(g, (col + 1 : U32), c1, row)

def bannerpeak.body.Body.any_solid.if2?(bpc: Bool, +g: J.Val, +col: U32, +c1: U32, +row: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.body.Body.any_solid.if1(bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, col, row)), g, col, c1, row)

def bannerpeak.body.Body.rise_rows?(+self: J.Val, +g: J.Val, +row: U32, +lo: U32, +half: U32, +h: U32, +new: U32, +new_head: U32, +head: U32, +out: J.Val) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  bannerpeak.body.Body.rise_rows.if16(RS.i_lt(row, lo), self, g, row, lo, half, h, new, new_head, head, out)

def bannerpeak.body.Body.rise_rows.if1?(bpc: Bool, +self: J.Val, +half: U32, +c0: U32, +slip: U32) -> U32:
  match bpc:
    case True{}:
      +slip : U32 = (((c0 + 1 : U32) * 4096 : U32) - (J.u32(J.get(self, "x")) - half : U32) : U32)
      slip
    case False{}:
      slip

def bannerpeak.body.Body.rise_rows.if2?(bpc: Bool, +slip: U32, +shift: U32) -> U32:
  match bpc:
    case True{}:
      +shift : U32 = slip
      shift
    case False{}:
      shift

def bannerpeak.body.Body.rise_rows.if3?(bpc: Bool, +c0: U32, +c1: U32, +col: U32) -> U32:
  match bpc:
    case True{}:
      +col : U32 = c0
      col
    case False{}:
      +col : U32 = c1
      col

def bannerpeak.body.Body.rise_rows.if4?(bpc: Bool, +c0: U32, +c1: U32, +l_col: Bool, +col: U32, +centre: U32) -> U32:
  match bpc:
    case True{}:
      +col : U32 = centre
      col
    case False{}:
      +col = bannerpeak.body.Body.rise_rows.if3(l_col, c0, c1, col)
      col

def bannerpeak.body.Body.rise_rows.if5?(bpc: Bool, +self: J.Val, +g: J.Val, +row: U32, +c0: U32, +c1: U32, +l_col: Bool, +col: U32) -> U32:
  match bpc:
    case True{}:
      +centre : U32 = RS.i_div_euclid(J.u32(J.get(self, "x")), 4096)
      +col = bannerpeak.body.Body.rise_rows.if4(bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, centre, row)), c0, c1, l_col, col, centre)
      col
    case False{}:
      col

def bannerpeak.body.Body.rise_rows.if6?(bpc: Bool, +self: J.Val, +g: J.Val, +row: U32, +h: U32, +new: U32, +out: J.Val, +c0: U32, +c1: U32, +l_col: Bool, +shift: U32) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +self = J.set(self, "y", J.of_i32(new))
      (True{}, (self, out))
    case False{}:
      +self = J.set(self, "x", J.of_i32((J.u32(J.get(self, "x")) - shift : U32)))
      +self = J.set(self, "y", J.of_i32((((row + 1 : U32) * 4096 : U32) + (h * 256 : U32) : U32)))
      +self = J.set(self, "vy", J.of_i32(0))
      +col : U32 = c0
      +col = bannerpeak.body.Body.rise_rows.if5(U32.is_ne(c0, c1), self, g, row, c0, c1, l_col, col)
      +out = J.push(out, bannerpeak.body.Ev.bump(col, row))
      (True{}, (self, out))

def bannerpeak.body.Body.rise_rows.if7?(bpc: Bool, +c0: U32, +c1: U32, +col: U32) -> U32:
  match bpc:
    case True{}:
      +col : U32 = c0
      col
    case False{}:
      +col : U32 = c1
      col

def bannerpeak.body.Body.rise_rows.if8?(bpc: Bool, +c0: U32, +c1: U32, +l_col: Bool, +col: U32, +centre: U32) -> U32:
  match bpc:
    case True{}:
      +col : U32 = centre
      col
    case False{}:
      +col = bannerpeak.body.Body.rise_rows.if7(l_col, c0, c1, col)
      col

def bannerpeak.body.Body.rise_rows.if9?(bpc: Bool, +self: J.Val, +g: J.Val, +row: U32, +c0: U32, +c1: U32, +l_col: Bool, +col: U32) -> U32:
  match bpc:
    case True{}:
      +centre : U32 = RS.i_div_euclid(J.u32(J.get(self, "x")), 4096)
      +col = bannerpeak.body.Body.rise_rows.if8(bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, centre, row)), c0, c1, l_col, col, centre)
      col
    case False{}:
      col

def bannerpeak.body.Body.rise_rows.if10?(bpc: Bool, +self: J.Val, +g: J.Val, +row: U32, +half: U32, +h: U32, +new: U32, +new_head: U32, +head: U32, +out: J.Val, +c0: U32, +c1: U32, +l_col: Bool, +slip: U32) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +shift : U32 = RS.i_neg(slip)
      +shift = bannerpeak.body.Body.rise_rows.if2(l_col, slip, shift)
      +self = J.set(self, "x", J.of_i32((J.u32(J.get(self, "x")) + shift : U32)))
      +free : Bool = Bool.not(bannerpeak.body.Body.blocked(g, (J.u32(J.get(self, "x")) - half : U32), new_head, (J.u32(J.get(self, "x")) + half : U32), head))
      bannerpeak.body.Body.rise_rows.if6(free, self, g, row, h, new, out, c0, c1, l_col, shift)
    case False{}:
      +self = J.set(self, "y", J.of_i32((((row + 1 : U32) * 4096 : U32) + (h * 256 : U32) : U32)))
      +self = J.set(self, "vy", J.of_i32(0))
      +col : U32 = c0
      +col = bannerpeak.body.Body.rise_rows.if9(U32.is_ne(c0, c1), self, g, row, c0, c1, l_col, col)
      +out = J.push(out, bannerpeak.body.Ev.bump(col, row))
      (True{}, (self, out))

def bannerpeak.body.Body.rise_rows.if11?(bpc: Bool, +c0: U32, +c1: U32, +col: U32) -> U32:
  match bpc:
    case True{}:
      +col : U32 = c0
      col
    case False{}:
      +col : U32 = c1
      col

def bannerpeak.body.Body.rise_rows.if12?(bpc: Bool, +c0: U32, +c1: U32, +l_col: Bool, +col: U32, +centre: U32) -> U32:
  match bpc:
    case True{}:
      +col : U32 = centre
      col
    case False{}:
      +col = bannerpeak.body.Body.rise_rows.if11(l_col, c0, c1, col)
      col

def bannerpeak.body.Body.rise_rows.if13?(bpc: Bool, +self: J.Val, +g: J.Val, +row: U32, +c0: U32, +c1: U32, +l_col: Bool, +col: U32) -> U32:
  match bpc:
    case True{}:
      +centre : U32 = RS.i_div_euclid(J.u32(J.get(self, "x")), 4096)
      +col = bannerpeak.body.Body.rise_rows.if12(bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, centre, row)), c0, c1, l_col, col, centre)
      col
    case False{}:
      col

def bannerpeak.body.Body.rise_rows.if14?(bpc: Bool, +self: J.Val, +g: J.Val, +row: U32, +half: U32, +h: U32, +new: U32, +new_head: U32, +head: U32, +out: J.Val, +c0: U32, +c1: U32, +l_col: Bool) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +slip : U32 = ((J.u32(J.get(self, "x")) + half : U32) - (c1 * 4096 : U32) : U32)
      +slip = bannerpeak.body.Body.rise_rows.if1(l_col, self, half, c0, slip)
      bannerpeak.body.Body.rise_rows.if10(RS.i_le(slip, (5 * 256 : U32)), self, g, row, half, h, new, new_head, head, out, c0, c1, l_col, slip)
    case False{}:
      +self = J.set(self, "y", J.of_i32((((row + 1 : U32) * 4096 : U32) + (h * 256 : U32) : U32)))
      +self = J.set(self, "vy", J.of_i32(0))
      +col : U32 = c0
      +col = bannerpeak.body.Body.rise_rows.if13(U32.is_ne(c0, c1), self, g, row, c0, c1, l_col, col)
      +out = J.push(out, bannerpeak.body.Ev.bump(col, row))
      (True{}, (self, out))

def bannerpeak.body.Body.rise_rows.if15?(bpc: Bool, +self: J.Val, +g: J.Val, +row: U32, +lo: U32, +half: U32, +h: U32, +new: U32, +new_head: U32, +head: U32, +out: J.Val, +c0: U32, +c1: U32) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.body.Body.rise_rows(self, g, (row - 1 : U32), lo, half, h, new, new_head, head, out)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      +out : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      (bannerpeak.u.fst(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t1), (self, out))
    case False{}:
      +l_col : Bool = bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, c0, row))
      +r_col : Bool = bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, c1, row))
      bannerpeak.body.Body.rise_rows.if14(Bool.and(U32.is_ne(c0, c1), Bool.xor(l_col, r_col)), self, g, row, half, h, new, new_head, head, out, c0, c1, l_col)

def bannerpeak.body.Body.rise_rows.if16?(bpc: Bool, +self: J.Val, +g: J.Val, +row: U32, +lo: U32, +half: U32, +h: U32, +new: U32, +new_head: U32, +head: U32, +out: J.Val) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (False{}, (self, out))
    case False{}:
      +c0 : U32 = RS.i_div_euclid((J.u32(J.get(self, "x")) - half : U32), 4096)
      +c1 : U32 = RS.i_div_euclid(((J.u32(J.get(self, "x")) + half : U32) - 1 : U32), 4096)
      bannerpeak.body.Body.rise_rows.if15(Bool.not(bannerpeak.body.Body.any_solid(g, c0, c1, row)), self, g, row, lo, half, h, new, new_head, head, out, c0, c1)

def bannerpeak.body.Body.wall_check?(+self: J.Val, +g: J.Val, +k: J.Val, +toward: U32) -> J.Val:
  bannerpeak.body.Body.wall_check.if5(Bool.or(Bool.or(Bool.or(Bool.or(RS.i_lt(J.u32(J.get(self, "vy")), 0), String.eq(J.str(J.get(self, "mode")), "Wind")), String.eq(J.str(J.get(self, "mode")), "Pound")), String.eq(J.str(J.get(self, "mode")), "Dive")), String.eq(J.str(J.get(self, "mode")), "Hurt")), self, g, k, toward)

def bannerpeak.body.Body.wall_check.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Air"})
      self
    case False{}:
      self

def bannerpeak.body.Body.wall_check.if2?(bpc: Bool, +self: J.Val, +g: J.Val, +toward: U32, +side: U32) -> U32:
  match bpc:
    case True{}:
      +side : U32 = bannerpeak.body.Body.wall_side(self, g, toward)
      side
    case False{}:
      side

def bannerpeak.body.Body.wall_check.if3?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Air"})
      self
    case False{}:
      self

def bannerpeak.body.Body.wall_check.if4?(bpc: Bool, +self: J.Val, +k: J.Val, +side: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "mode", J.JS{"Wall"})
      +self = J.set(self, "wall", J.of_i32(side))
      +self = J.set(self, "wall_t", J.JU{(J.u32(J.get(k, "grace")) + 1 : U32)})
      +self = J.set(self, "vy", J.of_i32(RS.i_min(J.u32(J.get(self, "vy")), 560)))
      self
    case False{}:
      +self = bannerpeak.body.Body.wall_check.if3(String.eq(J.str(J.get(self, "mode")), "Wall"), self)
      self

def bannerpeak.body.Body.wall_check.if5?(bpc: Bool, +self: J.Val, +g: J.Val, +k: J.Val, +toward: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = bannerpeak.body.Body.wall_check.if1(String.eq(J.str(J.get(self, "mode")), "Wall"), self)
      self
    case False{}:
      +side : U32 = 0
      +side = bannerpeak.body.Body.wall_check.if2(U32.is_ne(toward, 0), self, g, toward, side)
      +self = bannerpeak.body.Body.wall_check.if4(Bool.and(U32.is_ne(side, 0), U32.is_eq(side, toward)), self, k, side)
      self

def bannerpeak.body.Body.settle_pose?(+self: J.Val, +c: J.Val) -> J.Val:
  +self = J.set(self, "run_t", J.JU{((J.u32(J.get(self, "run_t")) + RS.i_div(RS.i_abs(J.u32(J.get(self, "vx"))), 128) : U32) + 1 : U32)})
  +pose : String = "Idle"
  +pose = bannerpeak.body.Body.settle_pose.if20(String.eq(J.str(J.get(self, "mode")), "Ground"), self, c, pose)
  +self = J.set(self, "pose", J.JS{pose})
  self

def bannerpeak.body.Body.settle_pose.if1?(bpc: Bool, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Slide"
      pose
    case False{}:
      +pose : String = "Crouch"
      pose

def bannerpeak.body.Body.settle_pose.if2?(bpc: Bool, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Run"
      pose
    case False{}:
      +pose : String = "RunB"
      pose

def bannerpeak.body.Body.settle_pose.if3?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose = bannerpeak.body.Body.settle_pose.if2(U32.is_eq(U32.mod(U32.div(J.u32(J.get(self, "run_t")), 24), 2), 0), pose)
      pose
    case False{}:
      +pose : String = "Idle"
      pose

def bannerpeak.body.Body.settle_pose.if4?(bpc: Bool, +self: J.Val, +c: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Skid"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if3(Bool.or(Bool.and(RS.i_gt(RS.i_abs(J.u32(J.get(self, "vx"))), 90), U32.is_ne(J.u32(J.get(c, "x")), 0)), RS.i_gt(RS.i_abs(J.u32(J.get(self, "vx"))), 200)), self, pose)
      pose

def bannerpeak.body.Body.settle_pose.if5?(bpc: Bool, +self: J.Val, +c: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose = bannerpeak.body.Body.settle_pose.if1(RS.i_gt(RS.i_abs(J.u32(J.get(self, "vx"))), RS.i_div(420, 2)), pose)
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if4(J.bool(J.get(self, "skid")), self, c, pose)
      pose

def bannerpeak.body.Body.settle_pose.if6?(bpc: Bool, +self: J.Val, +c: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Land"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if5(J.bool(J.get(self, "crouch")), self, c, pose)
      pose

def bannerpeak.body.Body.settle_pose.if7?(bpc: Bool, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Rise"
      pose
    case False{}:
      +pose : String = "Fall"
      pose

def bannerpeak.body.Body.settle_pose.if8?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Back"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if7(RS.i_lt(J.u32(J.get(self, "vy")), 0), pose)
      pose

def bannerpeak.body.Body.settle_pose.if9?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Long"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if8(String.eq(J.str(J.get(self, "jump")), "Back"), self, pose)
      pose

def bannerpeak.body.Body.settle_pose.if10?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Second"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if9(String.eq(J.str(J.get(self, "jump")), "Long"), self, pose)
      pose

def bannerpeak.body.Body.settle_pose.if11?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Spin"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if10(String.eq(J.str(J.get(self, "jump")), "Second"), self, pose)
      pose

def bannerpeak.body.Body.settle_pose.if12?(bpc: Bool, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Hurt"
      pose
    case False{}:
      +pose : String = "Brace"
      pose

def bannerpeak.body.Body.settle_pose.if13?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Belly"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if12(String.eq(J.str(J.get(self, "mode")), "Hurt"), pose)
      pose

def bannerpeak.body.Body.settle_pose.if14?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Dive"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if13(String.eq(J.str(J.get(self, "mode")), "Belly"), self, pose)
      pose

def bannerpeak.body.Body.settle_pose.if15?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Land"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if14(String.eq(J.str(J.get(self, "mode")), "Dive"), self, pose)
      pose

def bannerpeak.body.Body.settle_pose.if16?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Slam"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if15(String.eq(J.str(J.get(self, "mode")), "Recover"), self, pose)
      pose

def bannerpeak.body.Body.settle_pose.if17?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Wind"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if16(String.eq(J.str(J.get(self, "mode")), "Pound"), self, pose)
      pose

def bannerpeak.body.Body.settle_pose.if18?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose : String = "Wall"
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if17(String.eq(J.str(J.get(self, "mode")), "Wind"), self, pose)
      pose

def bannerpeak.body.Body.settle_pose.if19?(bpc: Bool, +self: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose = bannerpeak.body.Body.settle_pose.if11(String.eq(J.str(J.get(self, "jump")), "Triple"), self, pose)
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if18(String.eq(J.str(J.get(self, "mode")), "Wall"), self, pose)
      pose

def bannerpeak.body.Body.settle_pose.if20?(bpc: Bool, +self: J.Val, +c: J.Val, +pose: String) -> String:
  match bpc:
    case True{}:
      +pose = bannerpeak.body.Body.settle_pose.if6(U32.is_gt(J.u32(J.get(self, "squash")), 0), self, c, pose)
      pose
    case False{}:
      +pose = bannerpeak.body.Body.settle_pose.if19(String.eq(J.str(J.get(self, "mode")), "Air"), self, pose)
      pose

def bannerpeak.body.Body.small?(+self: J.Val, +g: J.Val, +id: U32) -> J.Val:
  +_j1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.dims(self)
  +_j2 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.body.Body.rect(self)
  +_j3 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(self)
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "valid", J.JBool{bannerpeak.body.Body.valid(self)}), "dims", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j1)), J.of_i32(bannerpeak.u.snd(U32, U32, _j1))]}), "stand", J.of_i32(bannerpeak.body.Body.stand_height(self))), "rect", J.JA{[J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2)), J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2))), J.of_i32(bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2)))), J.of_i32(bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2))))]}), "foot", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j3)), J.of_i32(bannerpeak.u.snd(U32, U32, _j3))]}), "slamming", J.JBool{bannerpeak.body.Body.slamming(self)}), "shield", bannerpeak.body.Body.shield(self, (1000 + id : U32))), "can_stand", J.JBool{bannerpeak.body.Body.can_stand(self, g)}), "tile_below", J.JU{bannerpeak.body.Body.tile_below(self, g)})

def case.bannerpeak.body.approach?(+input: J.Val) -> J.Val:
  J.of_i32(bannerpeak.body.approach(J.u32(J.get(input, "v")), J.u32(J.get(input, "to")), J.u32(J.get(input, "by"))))

def case.bannerpeak.body.Body.new?(+input: J.Val) -> J.Val:
  bannerpeak.body.Body.new(J.u32(J.get(input, "x")), J.u32(J.get(input, "y")))

def case.bannerpeak.body.Body.valid?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.body.Body.valid(J.get(input, "body"))}

def case.bannerpeak.body.Body.small?(+input: J.Val) -> J.Val:
  bannerpeak.body.Body.small(J.get(input, "body"), J.get(input, "g"), J.u32(J.get(input, "id")))

def case.bannerpeak.body.Body.hurt?(+input: J.Val) -> J.Val:
  bannerpeak.body.Body.hurt(J.get(input, "body"), J.u32(J.get(input, "from")))

def case.bannerpeak.body.Body.bounce?(+input: J.Val) -> J.Val:
  bannerpeak.body.Body.bounce(J.get(input, "body"), J.bool(J.get(input, "held")))

def case.bannerpeak.body.Body.place?(+input: J.Val) -> J.Val:
  bannerpeak.body.Body.place(J.get(input, "body"), J.u32(J.get(input, "x")), J.u32(J.get(input, "y")))

def case.bannerpeak.body.Body.regrow?(+input: J.Val) -> J.Val:
  bannerpeak.body.Body.regrow(J.get(input, "body"), J.get(input, "g"))

def case.bannerpeak.body.Body.blocked?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.body.Body.blocked(J.get(input, "g"), J.u32(J.get(input, "l")), J.u32(J.get(input, "t")), J.u32(J.get(input, "r")), J.u32(J.get(input, "b")))}

def case.bannerpeak.body.Body.step?(+input: J.Val) -> J.Val:
  +r = bannerpeak.body.Body.step(J.get(input, "body"), J.get(input, "cmd"), J.get(input, "knobs"), J.get(input, "g"), bannerpeak.u.vec_or(J.get(input, "now")), bannerpeak.u.vec_or(J.get(input, "before")), bannerpeak.u.vec_or(J.get(input, "out")))
  J.set(J.set(J.obj(), "body", bannerpeak.u.fst(J.Val, J.Val, r)), "out", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.body.Body.gravity?(+input: J.Val) -> J.Val:
  bannerpeak.body.Body.gravity(J.get(input, "body"), J.get(input, "c"))

def case.bannerpeak.body.Body.settle_pose?(+input: J.Val) -> J.Val:
  bannerpeak.body.Body.settle_pose(J.get(input, "body"), J.get(input, "c"))

def case.bannerpeak.body.Body.pick_jump?(+input: J.Val) -> J.Val:
  J.JS{bannerpeak.body.Body.pick_jump(J.get(input, "body"), J.get(input, "k"))}

def case.bannerpeak.body.Body.launch?(+input: J.Val) -> J.Val:
  bannerpeak.body.Body.launch(J.get(input, "body"), J.str(J.get(input, "kind")), J.get(input, "k"))

def case.bannerpeak.body.Body.landed?(+input: J.Val) -> J.Val:
  +r = bannerpeak.body.Body.landed(J.get(input, "body"), J.get(input, "k"), bannerpeak.u.vec_or(J.get(input, "out")))
  J.set(J.set(J.obj(), "body", bannerpeak.u.fst(J.Val, J.Val, r)), "out", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.body.Body.unstick?(+input: J.Val) -> J.Val:
  bannerpeak.body.Body.unstick(J.get(input, "body"), J.get(input, "g"))

def case.bannerpeak.body.Body.move_x?(+input: J.Val) -> J.Val:
  +r = bannerpeak.body.Body.move_x(J.get(input, "body"), J.get(input, "g"), J.u32(J.get(input, "dx")))
  J.set(J.set(J.obj(), "blocked", J.JBool{bannerpeak.u.fst(Bool, J.Val, r)}), "body", bannerpeak.u.snd(Bool, J.Val, r))

def case.bannerpeak.body.Body.move_y?(+input: J.Val) -> J.Val:
  +r = bannerpeak.body.Body.move_y(J.get(input, "body"), J.get(input, "g"), bannerpeak.u.vec_or(J.get(input, "now")), J.u32(J.get(input, "dy")), J.get(input, "k"), J.u32(J.get(input, "toward")), bannerpeak.u.vec_or(J.get(input, "out")))
  J.set(J.set(J.obj(), "body", bannerpeak.u.fst(J.Val, J.Val, r)), "out", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.body.Body.wall_check?(+input: J.Val) -> J.Val:
  bannerpeak.body.Body.wall_check(J.get(input, "body"), J.get(input, "g"), J.get(input, "k"), J.u32(J.get(input, "toward")))

def case.bannerpeak.body.Body.wall_side?(+input: J.Val) -> J.Val:
  J.of_i32(bannerpeak.body.Body.wall_side(J.get(input, "body"), J.get(input, "g"), J.u32(J.get(input, "toward"))))

def case.bannerpeak.body.Body.free?(+input: J.Val) -> J.Val:
  +r = bannerpeak.body.Body.free(J.get(input, "body"), J.get(input, "c"), J.get(input, "k"), J.get(input, "g"), bannerpeak.u.vec_or(J.get(input, "out")))
  J.set(J.set(J.obj(), "body", bannerpeak.u.fst(J.Val, J.Val, r)), "out", bannerpeak.u.snd(J.Val, J.Val, r))

def bannerpeak.boss.BOSS_WIDTH?() -> U32:
  48

def bannerpeak.boss.BOSS_HEIGHT?() -> U32:
  64

def bannerpeak.boss.HEAD_UP?() -> U32:
  64

def bannerpeak.boss.HEAD_DOWN?() -> U32:
  44

def bannerpeak.boss.PHASES?() -> U32:
  3

def bannerpeak.boss.BOSS_ID?() -> U32:
  300

def bannerpeak.boss.REACH?() -> U32:
  100

def bannerpeak.boss.MAX_PLAYERS?() -> U32:
  6

def bannerpeak.boss.floor?() -> U32:
  ((10 + 12 : U32) * 16 : U32)

def bannerpeak.boss.home?() -> U32:
  ((bannerpeak.layout.info.cinder_x() * 16 : U32) + RS.i_div(16, 2) : U32)

def bannerpeak.boss.Hazard.rect?(+self: J.Val) -> Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>:
  (J.u32(J.get(self, "x")), (J.u32(J.get(self, "y")), ((J.u32(J.get(self, "x")) + J.u32(J.get(self, "w")) : U32), (J.u32(J.get(self, "y")) + J.u32(J.get(self, "h")) : U32))))

def bannerpeak.boss.Hazard.hurts?(+self: J.Val) -> Bool:
  Bool.and(U32.is_eq(J.u32(J.get(self, "tell")), 0), U32.is_gt(J.u32(J.get(self, "live")), 0))

def bannerpeak.boss.Boss.new?(+seats: U32) -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32(bannerpeak.boss.home())), "beat", J.JS{"Dormant"}), "kind", J.JS{"Stomp"}), "t", J.JU{0}), "phase", J.JU{0}), "plates", J.JU{RS.u_max(seats, 1)}), "count", J.JU{0}), "head", J.of_i32(64)), "head_before", J.of_i32(64)), "x_before", J.of_i32(bannerpeak.boss.home())), "flinch", J.JU{0}), "hazards", bannerpeak.u.vec_new())

def bannerpeak.boss.Hazard.valid?(+self: J.Val) -> Bool:
  Bool.and(Bool.and(Bool.and(Bool.and(RS.i_le(J.u32(J.get(self, "w")), 400), RS.i_le(J.u32(J.get(self, "h")), 400)), U32.is_le(J.u32(J.get(self, "tell")), 60)), U32.is_le(J.u32(J.get(self, "live")), 120)), RS.i_le(RS.i_abs(J.u32(J.get(self, "vx"))), 8))

def bannerpeak.boss.Boss.hazards_valid?(+hs: J.Val, +n: U32) -> Bool:
  bannerpeak.boss.Boss.hazards_valid.if2(U32.is_ge(n, J.len(hs)), hs, n)

def bannerpeak.boss.Boss.hazards_valid.if1?(bpc: Bool, +hs: J.Val, +n: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.boss.Boss.hazards_valid(hs, (n + 1 : U32))

def bannerpeak.boss.Boss.hazards_valid.if2?(bpc: Bool, +hs: J.Val, +n: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.boss.Boss.hazards_valid.if1(Bool.not(bannerpeak.boss.Hazard.valid(J.nth(hs, n))), hs, n)

def bannerpeak.boss.Boss.valid?(+self: J.Val, +seats_max: U32) -> Bool:
  +phase_ok : Bool = Bool.not(Bool.xor(U32.is_eq(J.u32(J.get(self, "phase")), 3), String.eq(J.str(J.get(self, "beat")), "Down")))
  Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(RS.i_ge(J.u32(J.get(self, "x")), (bannerpeak.boss.home() - 200 : U32)), RS.i_le(J.u32(J.get(self, "x")), (bannerpeak.boss.home() + 200 : U32))), Bool.and(RS.i_ge(J.u32(J.get(self, "x_before")), (bannerpeak.boss.home() - 200 : U32)), RS.i_le(J.u32(J.get(self, "x_before")), (bannerpeak.boss.home() + 200 : U32)))), U32.is_le(J.u32(J.get(self, "t")), 400)), U32.is_le(J.u32(J.get(self, "phase")), 3)), U32.is_le(J.u32(J.get(self, "plates")), seats_max)), U32.is_le(J.u32(J.get(self, "count")), 8)), Bool.and(RS.i_ge(J.u32(J.get(self, "head")), (44 - 1 : U32)), RS.i_le(J.u32(J.get(self, "head")), (64 + 1 : U32)))), Bool.and(RS.i_ge(J.u32(J.get(self, "head_before")), (44 - 1 : U32)), RS.i_le(J.u32(J.get(self, "head_before")), (64 + 1 : U32)))), U32.is_le(J.u32(J.get(self, "flinch")), 20)), U32.is_le(J.len(J.get(self, "hazards")), 16)), bannerpeak.boss.Boss.hazards_valid(J.get(self, "hazards"), 0)), phase_ok)

def bannerpeak.boss.Boss.defeated?(+self: J.Val) -> Bool:
  String.eq(J.str(J.get(self, "beat")), "Down")

def bannerpeak.boss.Boss.body?(+self: J.Val) -> Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>:
  +f : U32 = bannerpeak.boss.floor()
  (((J.u32(J.get(self, "x")) - RS.i_div(48, 2) : U32) + 4 : U32), (((f - J.u32(J.get(self, "head")) : U32) + 4 : U32), (((J.u32(J.get(self, "x")) + RS.i_div(48, 2) : U32) - 4 : U32), f)))

def bannerpeak.boss.Boss.plank?(+self: J.Val, +before: Bool) -> J.Val:
  +x : U32 = J.u32(J.get(self, "x"))
  +head : U32 = J.u32(J.get(self, "head"))
  +_t1 = bannerpeak.boss.Boss.plank.if1(before, self, x, head)
  +head = bannerpeak.u.fst(U32, U32, _t1)
  +x = bannerpeak.u.snd(U32, U32, _t1)
  J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32(((x - RS.i_div(48, 2) : U32) + 2 : U32))), "y", J.of_i32((bannerpeak.boss.floor() - head : U32))), "w", J.of_i32((48 - 4 : U32))), "lift", J.JBool{False{}})

def bannerpeak.boss.Boss.plank.if1?(bpc: Bool, +self: J.Val, +x: U32, +head: U32) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      +x : U32 = J.u32(J.get(self, "x_before"))
      +head : U32 = J.u32(J.get(self, "head_before"))
      (head, x)
    case False{}:
      (head, x)

def bannerpeak.boss.Boss.crown?(+self: J.Val) -> J.Val:
  bannerpeak.boss.Boss.crown.if1(String.eq(J.str(J.get(self, "beat")), "Wince"), self)

def bannerpeak.boss.Boss.crown.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +top : U32 = (bannerpeak.boss.floor() - J.u32(J.get(self, "head")) : U32)
      +_j1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = ((J.u32(J.get(self, "x")) - 18 : U32), ((top - 10 : U32), ((J.u32(J.get(self, "x")) + 18 : U32), top)))
      J.JA{[J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1)), J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1))), J.of_i32(bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1)))), J.of_i32(bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1))))]}
    case False{}:
      J.JNull{}

def bannerpeak.boss.Boss.seated?(+self: J.Val, +was: U32, +now: U32) -> J.Val:
  +self = bannerpeak.boss.Boss.seated.if1(Bool.and(U32.is_gt(now, was), U32.is_lt(J.u32(J.get(self, "phase")), 3)), self, was, now)
  self

def bannerpeak.boss.Boss.seated.if1?(bpc: Bool, +self: J.Val, +was: U32, +now: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "plates", J.JU{(J.u32(J.get(self, "plates")) + (now - was : U32) : U32)})
      self
    case False{}:
      self

def bannerpeak.boss.Boss.sound?(+self: J.Val) -> Bool:
  Bool.and(Bool.and(Bool.and(U32.is_le(J.u32(J.get(self, "phase")), 3), U32.is_le(J.u32(J.get(self, "plates")), 6)), U32.is_le(J.u32(J.get(self, "count")), 9)), U32.is_le(J.len(J.get(self, "hazards")), 64))

def bannerpeak.boss.Boss.step?(+self: J.Val, +at: J.Val, +seats: U32, +elapsed: U32) -> J.Val:
  +before : J.Val = self
  +_t1 = bannerpeak.oracle.step(self, at, seats, elapsed)
  +self : J.Val = _t1
  +self = bannerpeak.boss.Boss.step.if1(Bool.not(bannerpeak.boss.Boss.sound(self)), self, before)
  self

def bannerpeak.boss.Boss.step.if1?(bpc: Bool, +self: J.Val, +before: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self : J.Val = before
      self
    case False{}:
      self

def bannerpeak.boss.Boss.slam?(+self: J.Val, +x: U32, +y: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  +before : J.Val = self
  +_t1 = bannerpeak.oracle.slam(self, x, y)
  +self : J.Val = bannerpeak.u.snd(Bool, J.Val, _t1)
  +hit : Bool = bannerpeak.u.fst(Bool, J.Val, _t1)
  bannerpeak.boss.Boss.slam.if1(Bool.not(bannerpeak.boss.Boss.sound(self)), self, before, hit)

def bannerpeak.boss.Boss.slam.if1?(bpc: Bool, +self: J.Val, +before: J.Val, +hit: Bool) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +self : J.Val = before
      (False{}, self)
    case False{}:
      (hit, self)

def bannerpeak.boss.Boss.shape?(+self: J.Val) -> J.Val:
  +_j1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.boss.Boss.body(self)
  J.set(J.set(J.set(J.set(J.set(J.obj(), "defeated", J.JBool{bannerpeak.boss.Boss.defeated(self)}), "body", J.JA{[J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1)), J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1))), J.of_i32(bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1)))), J.of_i32(bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1))))]}), "plank", bannerpeak.boss.Boss.plank(self, False{})), "plank_before", bannerpeak.boss.Boss.plank(self, True{})), "crown", bannerpeak.boss.Boss.crown(self))

def bannerpeak.oracle.pattern_len?(+phase: U32) -> U32:
  bannerpeak.oracle.pattern_len.if2(U32.is_eq(phase, 0), phase)

def bannerpeak.oracle.pattern_len.if1?(bpc: Bool) -> U32:
  match bpc:
    case True{}:
      3
    case False{}:
      4

def bannerpeak.oracle.pattern_len.if2?(bpc: Bool, +phase: U32) -> U32:
  match bpc:
    case True{}:
      2
    case False{}:
      bannerpeak.oracle.pattern_len.if1(U32.is_eq(phase, 1))

def bannerpeak.oracle.pattern_at?(+phase: U32, +i: U32) -> String:
  bannerpeak.oracle.pattern_at.if8(U32.is_eq(phase, 0), phase, i)

def bannerpeak.oracle.pattern_at.if1?(bpc: Bool) -> String:
  match bpc:
    case True{}:
      "Stomp"
    case False{}:
      "Embers"

def bannerpeak.oracle.pattern_at.if2?(bpc: Bool) -> String:
  match bpc:
    case True{}:
      "Stomp"
    case False{}:
      "Embers"

def bannerpeak.oracle.pattern_at.if3?(bpc: Bool, +i: U32) -> String:
  match bpc:
    case True{}:
      "Sweep"
    case False{}:
      bannerpeak.oracle.pattern_at.if2(U32.is_eq(i, 1))

def bannerpeak.oracle.pattern_at.if4?(bpc: Bool) -> String:
  match bpc:
    case True{}:
      "Stomp"
    case False{}:
      "Embers"

def bannerpeak.oracle.pattern_at.if5?(bpc: Bool, +i: U32) -> String:
  match bpc:
    case True{}:
      "Sweep"
    case False{}:
      bannerpeak.oracle.pattern_at.if4(U32.is_eq(i, 2))

def bannerpeak.oracle.pattern_at.if6?(bpc: Bool, +i: U32) -> String:
  match bpc:
    case True{}:
      "Embers"
    case False{}:
      bannerpeak.oracle.pattern_at.if5(U32.is_eq(i, 1), i)

def bannerpeak.oracle.pattern_at.if7?(bpc: Bool, +i: U32) -> String:
  match bpc:
    case True{}:
      bannerpeak.oracle.pattern_at.if3(U32.is_eq(i, 0), i)
    case False{}:
      bannerpeak.oracle.pattern_at.if6(U32.is_eq(i, 0), i)

def bannerpeak.oracle.pattern_at.if8?(bpc: Bool, +phase: U32, +i: U32) -> String:
  match bpc:
    case True{}:
      bannerpeak.oracle.pattern_at.if1(U32.is_eq(i, 0))
    case False{}:
      bannerpeak.oracle.pattern_at.if7(U32.is_eq(phase, 1), i)

def bannerpeak.oracle.pattern?(+phase: U32) -> J.Val:
  bannerpeak.oracle.pattern_fill(phase, 0, bannerpeak.u.vec_new())

def bannerpeak.oracle.pattern_fill?(+phase: U32, +i: U32, +acc: J.Val) -> J.Val:
  bannerpeak.oracle.pattern_fill.if1(U32.is_ge(i, bannerpeak.oracle.pattern_len(phase)), phase, i, acc)

def bannerpeak.oracle.pattern_fill.if1?(bpc: Bool, +phase: U32, +i: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = J.push(acc, J.JS{bannerpeak.oracle.pattern_at(phase, i)})
      bannerpeak.oracle.pattern_fill(phase, (i + 1 : U32), acc)

def bannerpeak.oracle.arena?(+x: U32, +y: U32) -> Bool:
  Bool.and(Bool.and(RS.i_gt(x, ((392 + 3 : U32) * 16 : U32)), RS.i_lt(x, ((392 + 38 : U32) * 16 : U32))), RS.i_gt(y, (10 * 16 : U32)))

def bannerpeak.oracle.near_of?(+at: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  bannerpeak.oracle.near_of.if2(U32.is_ge(n, J.len(at)), at, n, acc)

def bannerpeak.oracle.near_of.if1?(bpc: Bool, +acc: J.Val, +p: Sigma<&2, &2, U32, _ => U32>) -> J.Val:
  match bpc:
    case True{}:
      +_j3 : Sigma<&2, &2, U32, _ => U32> = p
      +acc = J.push(acc, J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j3)), J.of_i32(bannerpeak.u.snd(U32, U32, _j3))]})
      acc
    case False{}:
      acc

def bannerpeak.oracle.near_of.if2?(bpc: Bool, +at: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +p : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.nth(at, n), 0)), J.u32(J.nth(J.nth(at, n), 1)))
      +_t1 : Sigma<&2, &2, U32, _ => U32> = p
      +_t2 : Sigma<&2, &2, U32, _ => U32> = p
      +acc = bannerpeak.oracle.near_of.if1(bannerpeak.oracle.arena(bannerpeak.u.fst(U32, U32, _t1), bannerpeak.u.snd(U32, U32, _t2)), acc, p)
      bannerpeak.oracle.near_of(at, (n + 1 : U32), acc)

def bannerpeak.oracle.tick_hazards?(+hs: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  bannerpeak.oracle.tick_hazards.if3(U32.is_ge(n, J.len(hs)), hs, n, acc)

def bannerpeak.oracle.tick_hazards.if1?(bpc: Bool, +h: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +h = J.set(h, "tell", J.JU{(J.u32(J.get(h, "tell")) - 1 : U32)})
      h
    case False{}:
      +h = J.set(h, "x", J.of_i32((J.u32(J.get(h, "x")) + J.u32(J.get(h, "vx")) : U32)))
      +h = J.set(h, "live", J.JU{RS.u_sat_sub(J.u32(J.get(h, "live")), 1)})
      h

def bannerpeak.oracle.tick_hazards.if2?(bpc: Bool, +acc: J.Val, +h: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, h)
      acc
    case False{}:
      acc

def bannerpeak.oracle.tick_hazards.if3?(bpc: Bool, +hs: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +h : J.Val = J.nth(hs, n)
      +h = bannerpeak.oracle.tick_hazards.if1(U32.is_gt(J.u32(J.get(h, "tell")), 0), h)
      +acc = bannerpeak.oracle.tick_hazards.if2(Bool.or(U32.is_gt(J.u32(J.get(h, "live")), 0), U32.is_gt(J.u32(J.get(h, "tell")), 0)), acc, h)
      bannerpeak.oracle.tick_hazards(hs, (n + 1 : U32), acc)

def bannerpeak.oracle.nearest?(+near: J.Val, +bx: U32, +n: U32, +best: U32, +best_d: U32) -> U32:
  bannerpeak.oracle.nearest.if2(U32.is_ge(n, J.len(near)), near, bx, n, best, best_d)

def bannerpeak.oracle.nearest.if1?(bpc: Bool, +near: J.Val, +bx: U32, +n: U32, +best: U32, +best_d: U32, +d: U32) -> U32:
  match bpc:
    case True{}:
      bannerpeak.oracle.nearest(near, bx, (n + 1 : U32), n, d)
    case False{}:
      bannerpeak.oracle.nearest(near, bx, (n + 1 : U32), best, best_d)

def bannerpeak.oracle.nearest.if2?(bpc: Bool, +near: J.Val, +bx: U32, +n: U32, +best: U32, +best_d: U32) -> U32:
  match bpc:
    case True{}:
      best
    case False{}:
      +p : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.nth(near, n), 0)), J.u32(J.nth(J.nth(near, n), 1)))
      +_t1 : Sigma<&2, &2, U32, _ => U32> = p
      +d : U32 = RS.i_abs((bannerpeak.u.fst(U32, U32, _t1) - bx : U32))
      bannerpeak.oracle.nearest.if1(Bool.or(U32.is_eq(n, 0), RS.i_lt(d, best_d)), near, bx, n, best, best_d, d)

def bannerpeak.oracle.step?(+boss: J.Val, +at: J.Val, +seats: U32, +elapsed: U32) -> J.Val:
  +boss = J.set(boss, "head_before", J.of_i32(J.u32(J.get(boss, "head"))))
  +boss = J.set(boss, "x_before", J.of_i32(J.u32(J.get(boss, "x"))))
  +boss = J.set(boss, "flinch", J.JU{RS.u_sat_sub(J.u32(J.get(boss, "flinch")), 1)})
  +boss = J.set(boss, "hazards", bannerpeak.oracle.tick_hazards(J.get(boss, "hazards"), 0, bannerpeak.u.vec_new()))
  +near : J.Val = bannerpeak.oracle.near_of(at, 0, bannerpeak.u.vec_new())
  +crowd : U32 = (RS.u_max(seats, 1) - 1 : U32)
  +boss = bannerpeak.oracle.step.if13(String.eq(J.str(J.get(boss, "beat")), "Dormant"), boss, seats, elapsed, near, crowd)
  boss

def bannerpeak.oracle.step.if1?(bpc: Bool, +boss: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "beat", J.JS{"Rest"})
      +boss = J.set(boss, "t", J.JU{60})
      boss
    case False{}:
      boss

def bannerpeak.oracle.step.if2?(bpc: Bool, +boss: J.Val, +seats: U32) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "plates", J.JU{RS.u_max(seats, 1)})
      +boss = J.set(boss, "count", J.JU{0})
      +boss = J.set(boss, "beat", J.JS{"Rest"})
      +boss = J.set(boss, "t", J.JU{40})
      boss
    case False{}:
      boss

def bannerpeak.oracle.step.if3?(bpc: Bool, +boss: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "t", J.JU{14})
      boss
    case False{}:
      +boss = J.set(boss, "t", J.JU{12})
      boss

def bannerpeak.oracle.step.if4?(bpc: Bool, +boss: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "t", J.JU{22})
      boss
    case False{}:
      +boss = bannerpeak.oracle.step.if3(String.eq(J.str(J.get(boss, "kind")), "Sweep"), boss)
      boss

def bannerpeak.oracle.step.if5?(bpc: Bool, +boss: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t2 = bannerpeak.oracle.strike(boss)
      +boss : J.Val = _t2
      +boss = J.set(boss, "beat", J.JS{"Strike"})
      +boss = bannerpeak.oracle.step.if4(String.eq(J.str(J.get(boss, "kind")), "Stomp"), boss)
      boss
    case False{}:
      boss

def bannerpeak.oracle.step.if6?(bpc: Bool, +boss: J.Val, +crowd: U32) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "count", J.JU{(J.u32(J.get(boss, "count")) + 1 : U32)})
      +boss = J.set(boss, "beat", J.JS{"Rest"})
      +boss = J.set(boss, "t", J.JU{RS.u_max(((44 - (6 * J.u32(J.get(boss, "phase")) : U32) : U32) - (3 * crowd : U32) : U32), 16)})
      boss
    case False{}:
      boss

def bannerpeak.oracle.step.if7?(bpc: Bool, +boss: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "count", J.JU{0})
      +boss = J.set(boss, "beat", J.JS{"Rest"})
      +boss = J.set(boss, "t", J.JU{30})
      boss
    case False{}:
      boss

def bannerpeak.oracle.step.if8?(bpc: Bool, +boss: J.Val, +crowd: U32) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "t", J.JU{RS.u_sat_sub(J.u32(J.get(boss, "t")), 1)})
      +boss = bannerpeak.oracle.step.if6(U32.is_eq(J.u32(J.get(boss, "t")), 0), boss, crowd)
      boss
    case False{}:
      +boss = J.set(boss, "t", J.JU{RS.u_sat_sub(J.u32(J.get(boss, "t")), 1)})
      +boss = J.set(boss, "head", J.of_i32(RS.i_max((J.u32(J.get(boss, "head")) - 2 : U32), 44)))
      +boss = bannerpeak.oracle.step.if7(U32.is_eq(J.u32(J.get(boss, "t")), 0), boss)
      boss

def bannerpeak.oracle.step.if9?(bpc: Bool, +boss: J.Val, +crowd: U32) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "t", J.JU{RS.u_sat_sub(J.u32(J.get(boss, "t")), 1)})
      +boss = bannerpeak.oracle.step.if5(U32.is_eq(J.u32(J.get(boss, "t")), 0), boss)
      boss
    case False{}:
      +boss = bannerpeak.oracle.step.if8(String.eq(J.str(J.get(boss, "beat")), "Strike"), boss, crowd)
      boss

def bannerpeak.oracle.step.if10?(bpc: Bool, +boss: J.Val, +seats: U32, +elapsed: U32, +near: J.Val, +crowd: U32) -> J.Val:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.oracle.rest(boss, near, seats, elapsed, crowd)
      +boss : J.Val = _t1
      boss
    case False{}:
      +boss = bannerpeak.oracle.step.if9(String.eq(J.str(J.get(boss, "beat")), "Tell"), boss, crowd)
      boss

def bannerpeak.oracle.step.if11?(bpc: Bool, +boss: J.Val, +seats: U32, +elapsed: U32, +near: J.Val, +crowd: U32) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "t", J.JU{RS.u_sat_sub(J.u32(J.get(boss, "t")), 1)})
      +boss = J.set(boss, "head", J.of_i32(RS.i_min((J.u32(J.get(boss, "head")) + 1 : U32), 64)))
      +boss = bannerpeak.oracle.step.if2(U32.is_eq(J.u32(J.get(boss, "t")), 0), boss, seats)
      boss
    case False{}:
      +boss = bannerpeak.oracle.step.if10(String.eq(J.str(J.get(boss, "beat")), "Rest"), boss, seats, elapsed, near, crowd)
      boss

def bannerpeak.oracle.step.if12?(bpc: Bool, +boss: J.Val, +seats: U32, +elapsed: U32, +near: J.Val, +crowd: U32) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "head", J.of_i32(RS.i_max((J.u32(J.get(boss, "head")) - 1 : U32), (44 - 1 : U32))))
      boss
    case False{}:
      +boss = bannerpeak.oracle.step.if11(String.eq(J.str(J.get(boss, "beat")), "Rage"), boss, seats, elapsed, near, crowd)
      boss

def bannerpeak.oracle.step.if13?(bpc: Bool, +boss: J.Val, +seats: U32, +elapsed: U32, +near: J.Val, +crowd: U32) -> J.Val:
  match bpc:
    case True{}:
      +boss = bannerpeak.oracle.step.if1(U32.is_gt(J.len(near), 0), boss)
      boss
    case False{}:
      +boss = bannerpeak.oracle.step.if12(String.eq(J.str(J.get(boss, "beat")), "Down"), boss, seats, elapsed, near, crowd)
      boss

def bannerpeak.oracle.rest?(+boss: J.Val, +near: J.Val, +seats: U32, +elapsed: U32, +crowd: U32) -> J.Val:
  +boss = J.set(boss, "t", J.JU{RS.u_sat_sub(J.u32(J.get(boss, "t")), 1)})
  +boss = J.set(boss, "head", J.of_i32(RS.i_min((J.u32(J.get(boss, "head")) + 1 : U32), 64)))
  +boss = bannerpeak.oracle.rest.if2(U32.is_gt(J.len(near), 0), boss, near, elapsed)
  +boss = bannerpeak.oracle.rest.if6(U32.is_eq(J.u32(J.get(boss, "t")), 0), boss, near, seats, elapsed, crowd)
  boss

def bannerpeak.oracle.rest.if1?(bpc: Bool, +boss: J.Val, +tx: U32) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "x", J.of_i32((J.u32(J.get(boss, "x")) + RS.i_clamp((tx - J.u32(J.get(boss, "x")) : U32), 4294967295, 1) : U32)))
      boss
    case False{}:
      boss

def bannerpeak.oracle.rest.if2?(bpc: Bool, +boss: J.Val, +near: J.Val, +elapsed: U32) -> J.Val:
  match bpc:
    case True{}:
      +i : U32 = bannerpeak.oracle.nearest(near, J.u32(J.get(boss, "x")), 0, 0, 0)
      +_t1 : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.nth(near, i), 0)), J.u32(J.nth(J.nth(near, i), 1)))
      +tx : U32 = bannerpeak.u.fst(U32, U32, _t1)
      +boss = bannerpeak.oracle.rest.if1(U32.is_eq(U32.mod(elapsed, 2), 0), boss, tx)
      +boss = J.set(boss, "x", J.of_i32(RS.i_clamp(J.u32(J.get(boss, "x")), (bannerpeak.boss.home() - 110 : U32), (bannerpeak.boss.home() + 110 : U32))))
      boss
    case False{}:
      boss

def bannerpeak.oracle.rest.if3?(bpc: Bool, +base: U32) -> U32:
  match bpc:
    case True{}:
      +base : U32 = 32
      base
    case False{}:
      base

def bannerpeak.oracle.rest.if4?(bpc: Bool, +boss: J.Val, +base: U32) -> U32:
  match bpc:
    case True{}:
      +base : U32 = 40
      base
    case False{}:
      +base = bannerpeak.oracle.rest.if3(String.eq(J.str(J.get(boss, "kind")), "Sweep"), base)
      base

def bannerpeak.oracle.rest.if5?(bpc: Bool, +boss: J.Val, +near: J.Val, +seats: U32, +elapsed: U32, +crowd: U32, +phase: U32) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "beat", J.JS{"Wince"})
      +boss = J.set(boss, "t", J.JU{RS.i_max(((150 - (20 * J.u32(J.get(boss, "phase")) : U32) : U32) - (3 * crowd : U32) : U32), 70)})
      boss
    case False{}:
      +boss = J.set(boss, "kind", J.JS{bannerpeak.oracle.pattern_at(phase, J.u32(J.get(boss, "count")))})
      +boss = J.set(boss, "beat", J.JS{"Tell"})
      +base : U32 = 30
      +base = bannerpeak.oracle.rest.if4(String.eq(J.str(J.get(boss, "kind")), "Stomp"), boss, base)
      +boss = J.set(boss, "t", J.JU{(base - (3 * J.u32(J.get(boss, "phase")) : U32) : U32)})
      +_t2 = bannerpeak.oracle.warn(boss, near, seats, elapsed)
      +boss : J.Val = _t2
      boss

def bannerpeak.oracle.rest.if6?(bpc: Bool, +boss: J.Val, +near: J.Val, +seats: U32, +elapsed: U32, +crowd: U32) -> J.Val:
  match bpc:
    case True{}:
      +phase : U32 = RS.u_min(J.u32(J.get(boss, "phase")), 2)
      +boss = bannerpeak.oracle.rest.if5(U32.is_ge(J.u32(J.get(boss, "count")), bannerpeak.oracle.pattern_len(phase)), boss, near, seats, elapsed, crowd, phase)
      boss
    case False{}:
      boss

def bannerpeak.oracle.warn?(+boss: J.Val, +near: J.Val, +seats: U32, +elapsed: U32) -> J.Val:
  +f : U32 = bannerpeak.boss.floor()
  +tell : U32 = J.u32(J.get(boss, "t"))
  +boss = bannerpeak.oracle.warn.if5(String.eq(J.str(J.get(boss, "kind")), "Sweep"), boss, near, seats, elapsed, f, tell)
  boss

def bannerpeak.oracle.warn.if1?(bpc: Bool, +toward: U32) -> U32:
  match bpc:
    case True{}:
      +toward : U32 = 4294967295
      toward
    case False{}:
      toward

def bannerpeak.oracle.warn.if2?(bpc: Bool, +boss: J.Val, +near: J.Val, +toward: U32) -> U32:
  match bpc:
    case True{}:
      +i : U32 = bannerpeak.oracle.nearest(near, J.u32(J.get(boss, "x")), 0, 0, 0)
      +_t1 : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.nth(near, i), 0)), J.u32(J.nth(J.nth(near, i), 1)))
      +toward = bannerpeak.oracle.warn.if1(RS.i_lt(bannerpeak.u.fst(U32, U32, _t1), J.u32(J.get(boss, "x"))), toward)
      toward
    case False{}:
      toward

def bannerpeak.oracle.warn.if3?(bpc: Bool, +boss: J.Val, +l: U32) -> U32:
  match bpc:
    case True{}:
      +l : U32 = (J.u32(J.get(boss, "x")) + 6 : U32)
      l
    case False{}:
      l

def bannerpeak.oracle.warn.if4?(bpc: Bool, +boss: J.Val, +near: J.Val, +seats: U32, +elapsed: U32, +f: U32, +tell: U32) -> J.Val:
  match bpc:
    case True{}:
      +columns : U32 = ((1 + J.u32(J.get(boss, "phase")) : U32) + RS.i_div(RS.u_sat_sub(seats, 1), 2) : U32)
      +aims : J.Val = bannerpeak.oracle.sorted_xs(near, 0, bannerpeak.u.vec_new())
      +top : U32 = ((10 + 1 : U32) * 16 : U32)
      +_t2 = bannerpeak.oracle.embers(boss, aims, 0, columns, elapsed, tell, f, top)
      +boss : J.Val = _t2
      boss
    case False{}:
      boss

def bannerpeak.oracle.warn.if5?(bpc: Bool, +boss: J.Val, +near: J.Val, +seats: U32, +elapsed: U32, +f: U32, +tell: U32) -> J.Val:
  match bpc:
    case True{}:
      +toward : U32 = 1
      +toward = bannerpeak.oracle.warn.if2(U32.is_gt(J.len(near), 0), boss, near, toward)
      +l : U32 = ((J.u32(J.get(boss, "x")) - 6 : U32) - 100 : U32)
      +l = bannerpeak.oracle.warn.if3(RS.i_gt(toward, 0), boss, l)
      +boss = J.set(boss, "hazards", J.push(J.get(boss, "hazards"), J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32(l)), "y", J.of_i32((f - 22 : U32))), "w", J.of_i32(100)), "h", J.of_i32(22)), "vx", J.of_i32(0)), "tell", J.JU{tell}), "live", J.JU{14})))
      boss
    case False{}:
      +boss = bannerpeak.oracle.warn.if4(String.eq(J.str(J.get(boss, "kind")), "Embers"), boss, near, seats, elapsed, f, tell)
      boss

def bannerpeak.oracle.embers?(+boss: J.Val, +aims: J.Val, +c: U32, +columns: U32, +elapsed: U32, +tell: U32, +f: U32, +top: U32) -> J.Val:
  bannerpeak.oracle.embers.if2(RS.i_ge(c, columns), boss, aims, c, columns, elapsed, tell, f, top)

def bannerpeak.oracle.embers.if1?(bpc: Bool, +aims: J.Val, +c: U32, +elapsed: U32, +pick: U32) -> U32:
  match bpc:
    case True{}:
      +pick : U32 = J.u32(J.nth(aims, U32.mod((elapsed + (c * 3 : U32) : U32), J.len(aims))))
      pick
    case False{}:
      pick

def bannerpeak.oracle.embers.if2?(bpc: Bool, +boss: J.Val, +aims: J.Val, +c: U32, +columns: U32, +elapsed: U32, +tell: U32, +f: U32, +top: U32) -> J.Val:
  match bpc:
    case True{}:
      boss
    case False{}:
      +pick : U32 = bannerpeak.boss.home()
      +pick = bannerpeak.oracle.embers.if1(U32.is_gt(J.len(aims), 0), aims, c, elapsed, pick)
      +x : U32 = RS.i_clamp((pick + ((RS.i_rem(c, 3) - 1 : U32) * 26 : U32) : U32), ((392 + 4 : U32) * 16 : U32), ((392 + 36 : U32) * 16 : U32))
      +boss = J.set(boss, "hazards", J.push(J.get(boss, "hazards"), J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32((x - 10 : U32))), "y", J.of_i32(top)), "w", J.of_i32(20)), "h", J.of_i32((f - top : U32))), "vx", J.of_i32(0)), "tell", J.JU{tell}), "live", J.JU{12})))
      +_t1 = bannerpeak.oracle.embers(boss, aims, (c + 1 : U32), columns, elapsed, tell, f, top)
      +boss : J.Val = _t1
      boss

def bannerpeak.oracle.sorted_xs?(+near: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  bannerpeak.oracle.sorted_xs.if1(U32.is_ge(n, J.len(near)), near, n, acc)

def bannerpeak.oracle.sorted_xs.if1?(bpc: Bool, +near: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +_t1 : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.nth(near, n), 0)), J.u32(J.nth(J.nth(near, n), 1)))
      +x : U32 = bannerpeak.u.fst(U32, U32, _t1)
      +p : U32 = bannerpeak.oracle.count_lt(acc, x, 0)
      bannerpeak.oracle.sorted_xs(near, (n + 1 : U32), bannerpeak.oracle.insert_at(acc, x, p, 0, bannerpeak.u.vec_new()))

def bannerpeak.oracle.count_lt?(+a: J.Val, +x: U32, +i: U32) -> U32:
  bannerpeak.oracle.count_lt.if2(U32.is_ge(i, J.len(a)), a, x, i)

def bannerpeak.oracle.count_lt.if1?(bpc: Bool, +a: J.Val, +x: U32, +i: U32) -> U32:
  match bpc:
    case True{}:
      bannerpeak.oracle.count_lt(a, x, (i + 1 : U32))
    case False{}:
      i

def bannerpeak.oracle.count_lt.if2?(bpc: Bool, +a: J.Val, +x: U32, +i: U32) -> U32:
  match bpc:
    case True{}:
      i
    case False{}:
      bannerpeak.oracle.count_lt.if1(RS.i_lt(J.u32(J.nth(a, i)), x), a, x, i)

def bannerpeak.oracle.insert_at?(+a: J.Val, +x: U32, +p: U32, +i: U32, +acc: J.Val) -> J.Val:
  bannerpeak.oracle.insert_at.if3(U32.is_gt(i, J.len(a)), a, x, p, i, acc)

def bannerpeak.oracle.insert_at.if1?(bpc: Bool, +x: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.of_i32(x))
      acc
    case False{}:
      acc

def bannerpeak.oracle.insert_at.if2?(bpc: Bool, +a: J.Val, +i: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.of_i32(J.u32(J.nth(a, i))))
      acc
    case False{}:
      acc

def bannerpeak.oracle.insert_at.if3?(bpc: Bool, +a: J.Val, +x: U32, +p: U32, +i: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = bannerpeak.oracle.insert_at.if1(U32.is_eq(i, p), x, acc)
      +acc = bannerpeak.oracle.insert_at.if2(U32.is_lt(i, J.len(a)), a, i, acc)
      bannerpeak.oracle.insert_at(a, x, p, (i + 1 : U32), acc)

def bannerpeak.oracle.strike?(+boss: J.Val) -> J.Val:
  +boss = bannerpeak.oracle.strike.if2(String.eq(J.str(J.get(boss, "kind")), "Stomp"), boss)
  boss

def bannerpeak.oracle.strike.if1?(bpc: Bool, +n: U32) -> U32:
  match bpc:
    case True{}:
      +n : U32 = 2
      n
    case False{}:
      n

def bannerpeak.oracle.strike.if2?(bpc: Bool, +boss: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +f : U32 = bannerpeak.boss.floor()
      +n : U32 = 1
      +n = bannerpeak.oracle.strike.if1(U32.is_ge(J.u32(J.get(boss, "phase")), 1), n)
      +_t1 = bannerpeak.oracle.strike_waves(boss, 0, n, f)
      +boss : J.Val = _t1
      boss
    case False{}:
      boss

def bannerpeak.oracle.strike_waves?(+boss: J.Val, +k: U32, +n: U32, +f: U32) -> J.Val:
  bannerpeak.oracle.strike_waves.if1(RS.i_ge(k, n), boss, k, n, f)

def bannerpeak.oracle.strike_waves.if1?(bpc: Bool, +boss: J.Val, +k: U32, +n: U32, +f: U32) -> J.Val:
  match bpc:
    case True{}:
      boss
    case False{}:
      +boss = J.set(boss, "hazards", J.push(J.get(boss, "hazards"), J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32(((J.u32(J.get(boss, "x")) - 8 : U32) - (k * 40 : U32) : U32))), "y", J.of_i32((f - 14 : U32))), "w", J.of_i32(16)), "h", J.of_i32(14)), "vx", J.of_i32(4294967293)), "tell", J.JU{(k * 14 : U32)}), "live", J.JU{70})))
      +boss = J.set(boss, "hazards", J.push(J.get(boss, "hazards"), J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32(((J.u32(J.get(boss, "x")) - 8 : U32) + (k * 40 : U32) : U32))), "y", J.of_i32((f - 14 : U32))), "w", J.of_i32(16)), "h", J.of_i32(14)), "vx", J.of_i32(3)), "tell", J.JU{(k * 14 : U32)}), "live", J.JU{70})))
      +_t1 = bannerpeak.oracle.strike_waves(boss, (k + 1 : U32), n, f)
      +boss : J.Val = _t1
      boss

def bannerpeak.oracle.slam?(+boss: J.Val, +x: U32, +y: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  bannerpeak.oracle.slam.if3(Bool.not(String.eq(J.str(J.get(boss, "beat")), "Wince")), boss, x, y)

def bannerpeak.oracle.slam.if1?(bpc: Bool, +boss: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.oracle.fall(boss)
      +boss : J.Val = _t1
      boss
    case False{}:
      boss

def bannerpeak.oracle.slam.if2?(bpc: Bool, +boss: J.Val) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +boss = J.set(boss, "plates", J.JU{(J.u32(J.get(boss, "plates")) - 1 : U32)})
      +boss = J.set(boss, "flinch", J.JU{14})
      +boss = bannerpeak.oracle.slam.if1(U32.is_eq(J.u32(J.get(boss, "plates")), 0), boss)
      (True{}, boss)
    case False{}:
      (False{}, boss)

def bannerpeak.oracle.slam.if3?(bpc: Bool, +boss: J.Val, +x: U32, +y: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      (False{}, boss)
    case False{}:
      +top : U32 = (bannerpeak.boss.floor() - J.u32(J.get(boss, "head")) : U32)
      bannerpeak.oracle.slam.if2(Bool.and(Bool.and(U32.is_gt(J.u32(J.get(boss, "plates")), 0), RS.i_le(RS.i_abs((x - J.u32(J.get(boss, "x")) : U32)), 24)), RS.i_le(RS.i_abs((y - top : U32)), 3)), boss)

def bannerpeak.oracle.fall?(+boss: J.Val) -> J.Val:
  +boss = J.set(boss, "phase", J.JU{(J.u32(J.get(boss, "phase")) + 1 : U32)})
  +boss = J.set(boss, "hazards", bannerpeak.u.vec_new())
  +boss = bannerpeak.oracle.fall.if1(U32.is_ge(J.u32(J.get(boss, "phase")), 3), boss)
  boss

def bannerpeak.oracle.fall.if1?(bpc: Bool, +boss: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +boss = J.set(boss, "beat", J.JS{"Down"})
      +boss = J.set(boss, "t", J.JU{0})
      boss
    case False{}:
      +boss = J.set(boss, "beat", J.JS{"Rage"})
      +boss = J.set(boss, "t", J.JU{54})
      boss

def case.bannerpeak.boss.Boss.new?(+input: J.Val) -> J.Val:
  bannerpeak.boss.Boss.new(J.u32(J.get(input, "seats")))

def case.bannerpeak.boss.Boss.valid?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.boss.Boss.valid(J.get(input, "boss"), J.u32(J.get(input, "seats_max")))}

def case.bannerpeak.boss.Boss.shape?(+input: J.Val) -> J.Val:
  bannerpeak.boss.Boss.shape(J.get(input, "boss"))

def case.bannerpeak.boss.Boss.seated?(+input: J.Val) -> J.Val:
  bannerpeak.boss.Boss.seated(J.get(input, "boss"), J.u32(J.get(input, "was")), J.u32(J.get(input, "now")))

def case.bannerpeak.boss.floor?(+input: J.Val) -> J.Val:
  J.of_i32(bannerpeak.boss.floor())

def case.bannerpeak.boss.home?(+input: J.Val) -> J.Val:
  J.of_i32(bannerpeak.boss.home())

def case.bannerpeak.oracle.pattern?(+input: J.Val) -> J.Val:
  bannerpeak.oracle.pattern(J.u32(J.get(input, "phase")))

def case.bannerpeak.oracle.step?(+input: J.Val) -> J.Val:
  bannerpeak.oracle.step(J.get(input, "boss"), bannerpeak.u.vec_or(J.get(input, "feet")), J.u32(J.get(input, "seats")), J.u32(J.get(input, "elapsed")))

def case.bannerpeak.oracle.slam?(+input: J.Val) -> J.Val:
  +r = bannerpeak.oracle.slam(J.get(input, "boss"), J.u32(J.get(input, "x")), J.u32(J.get(input, "y")))
  J.set(J.set(J.obj(), "hit", J.JBool{bannerpeak.u.fst(Bool, J.Val, r)}), "boss", bannerpeak.u.snd(Bool, J.Val, r))

def bannerpeak.bot.shipped_knobs?() -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.obj(), "run", J.of_i32(1152)), "hop", J.JU{10}), "grace", J.JU{5}), "slam", J.of_i32(36)), "ledge", J.of_i32(3000))

def bannerpeak.bot.Goal.of?(+k: String, +a: U32, +b: U32) -> J.Val:
  J.set(J.set(J.set(J.obj(), "k", J.JS{k}), "a", J.of_i32(a)), "b", J.of_i32(b))

def bannerpeak.bot.keys?(+x: U32, +jump: Bool, +down: Bool) -> J.Val:
  +i : J.Val = bannerpeak.u.input_default()
  +i = J.set(i, "move_x", J.of_i32(x))
  +i = J.set(i, "dash", J.JBool{jump})
  +i = bannerpeak.bot.keys.if1(down, i)
  i

def bannerpeak.bot.keys.if1?(bpc: Bool, +i: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +i = J.set(i, "move_y", J.of_i32(1))
      i
    case False{}:
      i

def bannerpeak.bot.centre_x?(+cx: U32) -> U32:
  ((cx * 16 : U32) + RS.i_div(16, 2) : U32)

def bannerpeak.bot.Session.bot_input?(+self: J.Val, +id: U32, +hero: J.Val) -> J.Val:
  bannerpeak.bot.Session.bot_input.if8(Bool.not(bannerpeak.u.has(J.get(self, "seats"), id)), self, id)

def bannerpeak.bot.Session.bot_input.if1?(bpc: Bool, +i: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +i = J.set(i, "interact", J.JBool{True{}})
      i
    case False{}:
      i

def bannerpeak.bot.Session.bot_input.if2?(bpc: Bool, +i: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +i = J.set(i, "interact", J.JBool{True{}})
      i
    case False{}:
      i

def bannerpeak.bot.Session.bot_input.if3?(bpc: Bool, +i: J.Val, +bx: U32, +by: U32, +x: U32, +y: U32) -> J.Val:
  match bpc:
    case True{}:
      +i = J.set(i, "move_x", J.of_i32(RS.i_signum((x - bx : U32))))
      +i = J.set(i, "move_y", J.of_i32(RS.i_signum(((y - 12 : U32) - by : U32))))
      i
    case False{}:
      i

def bannerpeak.bot.Session.bot_input.if4?(bpc: Bool, +self: J.Val, +k: J.Val, +i: J.Val) -> J.Val:
  match bpc:
    case True{}:
      i
    case False{}:
      bannerpeak.bot.Session.drive(self, k, bannerpeak.bot.Session.goal(self, k))

def bannerpeak.bot.Session.bot_input.if5?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +i: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +b : J.Val = J.get(k, "bubble")
      +bx : U32 = RS.i_div_euclid(J.u32(J.get(b, "x")), 256)
      +by : U32 = RS.i_div_euclid(J.u32(J.get(b, "y")), 256)
      +_t1 : Sigma<&2, &2, Bool, _ => Sigma<&2, &2, U32, _ => U32>> = bannerpeak.bot.Session.nearest_friend(self, bannerpeak.u.ids(J.get(self, "seats")), id, bx, by, False{}, 0, 0, 0)
      +found = bannerpeak.u.fst(Bool, Sigma<&2, &2, U32, _ => U32>, _t1)
      +x = bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(Bool, Sigma<&2, &2, U32, _ => U32>, _t1))
      +y = bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(Bool, Sigma<&2, &2, U32, _ => U32>, _t1))
      +i = bannerpeak.bot.Session.bot_input.if3(found, i, bx, by, x, y)
      i
    case False{}:
      bannerpeak.bot.Session.bot_input.if4(Bool.not(bannerpeak.knight.Knight.free(k)), self, k, i)

def bannerpeak.bot.Session.bot_input.if6?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +i: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +i = bannerpeak.bot.Session.bot_input.if2(Bool.and(Bool.not(J.is_null(J.get(self, "completed"))), U32.is_eq(U32.mod(J.u32(J.get(self, "elapsed")), 2), 0)), i)
      i
    case False{}:
      bannerpeak.bot.Session.bot_input.if5(Bool.not(J.is_null(J.get(k, "bubble"))), self, id, k, i)

def bannerpeak.bot.Session.bot_input.if7?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +i: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +i = bannerpeak.bot.Session.bot_input.if1(U32.is_eq(U32.mod(J.u32(J.get(self, "elapsed")), 2), 0), i)
      i
    case False{}:
      bannerpeak.bot.Session.bot_input.if6(String.eq(J.str(J.get(self, "phase")), "Finale"), self, id, k, i)

def bannerpeak.bot.Session.bot_input.if8?(bpc: Bool, +self: J.Val, +id: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.u.input_default()
    case False{}:
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +i : J.Val = bannerpeak.u.input_default()
      bannerpeak.bot.Session.bot_input.if7(String.eq(J.str(J.get(self, "phase")), "Select"), self, id, k, i)

def bannerpeak.bot.Session.nearest_friend?(+self: J.Val, +ids: List<&2, U32>, +id: U32, +bx: U32, +by: U32, +found: Bool, +best: U32, +fx: U32, +fy: U32) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, U32, _ => U32>>:
  bannerpeak.bot.Session.nearest_friend.if4(bannerpeak.u.is_empty(ids), self, ids, id, bx, by, found, best, fx, fy)

def bannerpeak.bot.Session.nearest_friend.if1?(bpc: Bool, +f: J.Val, +_b1: Bool) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +_b1 : Bool = bannerpeak.knight.Knight.free(f)
      (_b1, f)
    case False{}:
      (_b1, f)

def bannerpeak.bot.Session.nearest_friend.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +id: U32, +bx: U32, +by: U32, +found: Bool, +best: U32, +fx: U32, +fy: U32, +p: Sigma<&2, &2, U32, _ => U32>, +d: U32) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, U32, _ => U32>>:
  match bpc:
    case True{}:
      +_t5 : Sigma<&2, &2, U32, _ => U32> = p
      +_t6 : Sigma<&2, &2, U32, _ => U32> = p
      bannerpeak.bot.Session.nearest_friend(self, bannerpeak.u.tail(ids), id, bx, by, True{}, d, bannerpeak.u.fst(U32, U32, _t5), bannerpeak.u.snd(U32, U32, _t6))
    case False{}:
      bannerpeak.bot.Session.nearest_friend(self, bannerpeak.u.tail(ids), id, bx, by, found, best, fx, fy)

def bannerpeak.bot.Session.nearest_friend.if3?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +id: U32, +bx: U32, +by: U32, +found: Bool, +best: U32, +fx: U32, +fy: U32, +f: J.Val) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, U32, _ => U32>>:
  match bpc:
    case True{}:
      +p : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(f, "body"))
      +_t3 : Sigma<&2, &2, U32, _ => U32> = p
      +_t4 : Sigma<&2, &2, U32, _ => U32> = p
      +d : U32 = (RS.i_abs((bannerpeak.u.fst(U32, U32, _t3) - bx : U32)) + RS.i_abs((bannerpeak.u.snd(U32, U32, _t4) - by : U32)) : U32)
      bannerpeak.bot.Session.nearest_friend.if2(Bool.or(Bool.not(found), RS.i_lt(d, best)), self, ids, id, bx, by, found, best, fx, fy, p, d)
    case False{}:
      bannerpeak.bot.Session.nearest_friend(self, bannerpeak.u.tail(ids), id, bx, by, found, best, fx, fy)

def bannerpeak.bot.Session.nearest_friend.if4?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +id: U32, +bx: U32, +by: U32, +found: Bool, +best: U32, +fx: U32, +fy: U32) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, U32, _ => U32>>:
  match bpc:
    case True{}:
      (found, (fx, fy))
    case False{}:
      +o : U32 = bannerpeak.u.head(ids)
      +f : J.Val = bannerpeak.u.at(J.get(self, "seats"), o)
      +_b1 = Bool.and(U32.is_ne(o, id), bannerpeak.knight.Knight.alive(f))
      +_t2 = bannerpeak.bot.Session.nearest_friend.if1(_b1, f, _b1)
      +_b1 = bannerpeak.u.fst(Bool, J.Val, _t2)
      +f = bannerpeak.u.snd(Bool, J.Val, _t2)
      bannerpeak.bot.Session.nearest_friend.if3(_b1, self, ids, id, bx, by, found, best, fx, fy, f)

def bannerpeak.bot.Sec.row?(+oy: U32, +r: U32) -> U32:
  ((oy + r : U32) * 16 : U32)

def bannerpeak.bot.Session.goal?(+self: J.Val, +k: J.Val) -> J.Val:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +cx : U32 = RS.i_div_euclid(fx, 16)
  +b : U32 = J.u32(J.get(self, "banners"))
  +section : U32 = bannerpeak.layout.section_of(cx)
  bannerpeak.bot.Session.goal.if5(U32.is_eq(section, 0), self, k, fx, fy, cx, b, section)

def bannerpeak.bot.Session.goal.if1?(bpc: Bool, +self: J.Val, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.snow_goal(self, k)
    case False{}:
      bannerpeak.bot.Session.summit_goal(self, k)

def bannerpeak.bot.Session.goal.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +section: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.shaft_goal(self, k)
    case False{}:
      bannerpeak.bot.Session.goal.if1(U32.is_eq(section, 4), self, k)

def bannerpeak.bot.Session.goal.if3?(bpc: Bool, +self: J.Val, +k: J.Val, +section: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.mill_goal(self, k)
    case False{}:
      bannerpeak.bot.Session.goal.if2(U32.is_eq(section, 3), self, k, section)

def bannerpeak.bot.Session.goal.if4?(bpc: Bool, +self: J.Val, +k: J.Val, +fx: U32, +fy: U32, +cx: U32, +b: U32, +section: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.marsh_goal(self, k, fx, fy, cx, b)
    case False{}:
      bannerpeak.bot.Session.goal.if3(U32.is_eq(section, 2), self, k, section)

def bannerpeak.bot.Session.goal.if5?(bpc: Bool, +self: J.Val, +k: J.Val, +fx: U32, +fy: U32, +cx: U32, +b: U32, +section: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.hub_goal(self, k, fx, fy, cx, b)
    case False{}:
      bannerpeak.bot.Session.goal.if4(U32.is_eq(section, 1), self, k, fx, fy, cx, b, section)

def bannerpeak.bot.Session.hub_goal?(+self: J.Val, +k: J.Val, +fx: U32, +fy: U32, +cx: U32, +b: U32) -> J.Val:
  +in_cellar : Bool = Bool.and(RS.i_gt(fy, (bannerpeak.bot.Sec.row(32, 18) + 2 : U32)), Bool.and(RS.i_ge(cx, (0 + 13 : U32)), RS.i_lt(cx, (0 + 34 : U32))))
  bannerpeak.bot.Session.hub_goal.if7(Bool.and(U32.is_eq(U32.and(b, 1), 0), Bool.not(in_cellar)), fx, fy, b, in_cellar)

def bannerpeak.bot.Session.hub_goal.if1?(bpc: Bool, +bx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", bx, 0)
    case False{}:
      bannerpeak.bot.Goal.of("Walk", ((0 + 58 : U32) * 16 : U32), 0)

def bannerpeak.bot.Session.hub_goal.if2?(bpc: Bool, +bx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Down", bx, 0)
    case False{}:
      bannerpeak.bot.Goal.of("Down", (((0 + 13 : U32) * 16 : U32) + 8 : U32), 0)

def bannerpeak.bot.Session.hub_goal.if3?(bpc: Bool, +fy: U32) -> J.Val:
  match bpc:
    case True{}:
      +bx : U32 = bannerpeak.bot.centre_x(bannerpeak.layout.info.banner_x(1))
      bannerpeak.bot.Session.hub_goal.if2(RS.i_ge(fy, (bannerpeak.bot.Sec.row(32, 28) - 1 : U32)), bx)
    case False{}:
      bannerpeak.bot.Goal.of("Slam", ((0 + 24 : U32) * 16 : U32), 0)

def bannerpeak.bot.Session.hub_goal.if4?(bpc: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", (((0 + 13 : U32) * 16 : U32) + 6 : U32), 0)
    case False{}:
      bannerpeak.bot.Goal.of("Walk", ((0 + 30 : U32) * 16 : U32), 0)

def bannerpeak.bot.Session.hub_goal.if5?(bpc: Bool, +fx: U32, +fy: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.hub_goal.if4(Bool.and(RS.i_gt(fx, (((0 + 13 : U32) * 16 : U32) + 12 : U32)), RS.i_ge(fy, (bannerpeak.bot.Sec.row(32, 28) - 1 : U32))))
    case False{}:
      bannerpeak.bot.Goal.of("Walk", ((0 + 82 : U32) * 16 : U32), 0)

def bannerpeak.bot.Session.hub_goal.if6?(bpc: Bool, +fx: U32, +fy: U32, +in_cellar: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.hub_goal.if3(in_cellar, fy)
    case False{}:
      bannerpeak.bot.Session.hub_goal.if5(in_cellar, fx, fy)

def bannerpeak.bot.Session.hub_goal.if7?(bpc: Bool, +fx: U32, +fy: U32, +b: U32, +in_cellar: Bool) -> J.Val:
  match bpc:
    case True{}:
      +bx : U32 = bannerpeak.bot.centre_x(bannerpeak.layout.info.banner_x(0))
      +on_roof : Bool = Bool.and(RS.i_le(fy, (bannerpeak.bot.Sec.row(32, 11) + 2 : U32)), RS.i_ge(fx, ((0 + 56 : U32) * 16 : U32)))
      bannerpeak.bot.Session.hub_goal.if1(on_roof, bx)
    case False{}:
      bannerpeak.bot.Session.hub_goal.if6(U32.is_eq(U32.and(b, 2), 0), fx, fy, in_cellar)

def bannerpeak.bot.Session.marsh_spark?(+n: U32, +room: Bool) -> U32:
  bannerpeak.bot.Session.marsh_spark.if2(U32.is_ge(n, bannerpeak.layout.info.sparks_len()), n, room)

def bannerpeak.bot.Session.marsh_spark.if1?(bpc: Bool, +n: U32, +room: Bool) -> U32:
  match bpc:
    case True{}:
      n
    case False{}:
      bannerpeak.bot.Session.marsh_spark((n + 1 : U32), room)

def bannerpeak.bot.Session.marsh_spark.if2?(bpc: Bool, +n: U32, +room: Bool) -> U32:
  match bpc:
    case True{}:
      8
    case False{}:
      +sx : U32 = bannerpeak.layout.info.spark_x(n)
      +sy : U32 = bannerpeak.layout.info.spark_y(n)
      +in_marsh : Bool = Bool.and(RS.i_ge(sx, 80), RS.i_lt(sx, (80 + 105 : U32)))
      bannerpeak.bot.Session.marsh_spark.if1(Bool.and(in_marsh, Bool.or(Bool.and(room, RS.i_gt(sy, (36 + 18 : U32))), Bool.and(Bool.not(room), RS.i_le(sy, (36 + 18 : U32))))), n, room)

def bannerpeak.bot.Session.marsh_goal?(+self: J.Val, +k: J.Val, +fx: U32, +fy: U32, +cx: U32, +b: U32) -> J.Val:
  +room_spark : U32 = bannerpeak.bot.Session.marsh_spark(0, True{})
  +in_room : Bool = RS.i_gt(fy, bannerpeak.bot.Sec.row(36, 16))
  bannerpeak.bot.Session.marsh_goal.if13(U32.is_lt(room_spark, 8), self, k, fx, cx, b, room_spark, in_room)

def bannerpeak.bot.Session.marsh_goal.if1?(bpc: Bool, +room_spark: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", bannerpeak.bot.centre_x(bannerpeak.layout.info.spark_x(room_spark)), 0)
    case False{}:
      bannerpeak.bot.Goal.of("Hatch", bannerpeak.bot.centre_x((80 + 44 : U32)), 0)

def bannerpeak.bot.Session.marsh_goal.if2?(bpc: Bool, +px_: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", px_, 0)
    case False{}:
      bannerpeak.bot.Goal.of("Walk", (((80 + 105 : U32) + 4 : U32) * 16 : U32), 0)

def bannerpeak.bot.Session.marsh_goal.if3?(bpc: Bool, +k: J.Val, +b: U32, +sx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", (sx - (11 * 16 : U32) : U32), 0)
    case False{}:
      +px_ : U32 = bannerpeak.bot.centre_x(bannerpeak.layout.info.pole_x())
      bannerpeak.bot.Session.marsh_goal.if2(Bool.or(U32.is_eq(U32.and(b, 4), 0), U32.is_gt(J.u32(J.get(k, "pole")), 0)), px_)

def bannerpeak.bot.Session.marsh_goal.if4?(bpc: Bool, +px_: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", px_, 0)
    case False{}:
      bannerpeak.bot.Goal.of("Walk", (((80 + 105 : U32) + 4 : U32) * 16 : U32), 0)

def bannerpeak.bot.Session.marsh_goal.if5?(bpc: Bool, +self: J.Val, +k: J.Val, +fx: U32, +b: U32, +over_bog: U32) -> J.Val:
  match bpc:
    case True{}:
      +sx : U32 = bannerpeak.bot.centre_x(bannerpeak.layout.info.spark_x(over_bog))
      bannerpeak.bot.Session.marsh_goal.if3(Bool.and(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "sparks")), RS.u_shl(1, over_bog)), 0), U32.is_eq(J.u32(J.get(k, "pole")), 0)), RS.i_gt(fx, (sx + (3 * 16 : U32) : U32))), k, b, sx)
    case False{}:
      +px_ : U32 = bannerpeak.bot.centre_x(bannerpeak.layout.info.pole_x())
      bannerpeak.bot.Session.marsh_goal.if4(Bool.or(U32.is_eq(U32.and(b, 4), 0), U32.is_gt(J.u32(J.get(k, "pole")), 0)), px_)

def bannerpeak.bot.Session.marsh_goal.if6?(bpc: Bool, +self: J.Val, +k: J.Val, +fx: U32, +b: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Hatch", bannerpeak.bot.centre_x((80 + 56 : U32)), 0)
    case False{}:
      +over_bog : U32 = bannerpeak.bot.Session.marsh_spark(0, False{})
      bannerpeak.bot.Session.marsh_goal.if5(U32.is_lt(over_bog, 8), self, k, fx, b, over_bog)

def bannerpeak.bot.Session.marsh_goal.if7?(bpc: Bool, +self: J.Val, +k: J.Val, +fx: U32, +b: U32, +room_spark: U32, +in_room: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.marsh_goal.if1(in_room, room_spark)
    case False{}:
      bannerpeak.bot.Session.marsh_goal.if6(in_room, self, k, fx, b)

def bannerpeak.bot.Session.marsh_goal.if8?(bpc: Bool, +px_: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", px_, 0)
    case False{}:
      bannerpeak.bot.Goal.of("Walk", (((80 + 105 : U32) + 4 : U32) * 16 : U32), 0)

def bannerpeak.bot.Session.marsh_goal.if9?(bpc: Bool, +k: J.Val, +b: U32, +sx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", (sx - (11 * 16 : U32) : U32), 0)
    case False{}:
      +px_ : U32 = bannerpeak.bot.centre_x(bannerpeak.layout.info.pole_x())
      bannerpeak.bot.Session.marsh_goal.if8(Bool.or(U32.is_eq(U32.and(b, 4), 0), U32.is_gt(J.u32(J.get(k, "pole")), 0)), px_)

def bannerpeak.bot.Session.marsh_goal.if10?(bpc: Bool, +px_: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", px_, 0)
    case False{}:
      bannerpeak.bot.Goal.of("Walk", (((80 + 105 : U32) + 4 : U32) * 16 : U32), 0)

def bannerpeak.bot.Session.marsh_goal.if11?(bpc: Bool, +self: J.Val, +k: J.Val, +fx: U32, +b: U32, +over_bog: U32) -> J.Val:
  match bpc:
    case True{}:
      +sx : U32 = bannerpeak.bot.centre_x(bannerpeak.layout.info.spark_x(over_bog))
      bannerpeak.bot.Session.marsh_goal.if9(Bool.and(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "sparks")), RS.u_shl(1, over_bog)), 0), U32.is_eq(J.u32(J.get(k, "pole")), 0)), RS.i_gt(fx, (sx + (3 * 16 : U32) : U32))), k, b, sx)
    case False{}:
      +px_ : U32 = bannerpeak.bot.centre_x(bannerpeak.layout.info.pole_x())
      bannerpeak.bot.Session.marsh_goal.if10(Bool.or(U32.is_eq(U32.and(b, 4), 0), U32.is_gt(J.u32(J.get(k, "pole")), 0)), px_)

def bannerpeak.bot.Session.marsh_goal.if12?(bpc: Bool, +self: J.Val, +k: J.Val, +fx: U32, +b: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Hatch", bannerpeak.bot.centre_x((80 + 56 : U32)), 0)
    case False{}:
      +over_bog : U32 = bannerpeak.bot.Session.marsh_spark(0, False{})
      bannerpeak.bot.Session.marsh_goal.if11(U32.is_lt(over_bog, 8), self, k, fx, b, over_bog)

def bannerpeak.bot.Session.marsh_goal.if13?(bpc: Bool, +self: J.Val, +k: J.Val, +fx: U32, +cx: U32, +b: U32, +room_spark: U32, +in_room: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.marsh_goal.if7(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "sparks")), RS.u_shl(1, room_spark)), 0), Bool.or(in_room, RS.i_lt(cx, (80 + 61 : U32)))), self, k, fx, b, room_spark, in_room)
    case False{}:
      bannerpeak.bot.Session.marsh_goal.if12(in_room, self, k, fx, b)

def bannerpeak.bot.Session.spark_at?(+self: J.Val, +cx: U32, +cy: U32) -> U32:
  bannerpeak.layout.spark_index(cx, cy)

def bannerpeak.bot.Session.spark_missing?(+self: J.Val, +cx: U32, +cy: U32) -> Bool:
  U32.is_eq(U32.and(J.u32(J.get(self, "sparks")), RS.u_shl(1, bannerpeak.bot.Session.spark_at(self, cx, cy))), 0)

def bannerpeak.bot.Session.mill_goal?(+self: J.Val, +k: J.Val) -> J.Val:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +cx : U32 = RS.i_div_euclid(fx, 16)
  +ledge_x : U32 = (185 + 68 : U32)
  +ledge_y : U32 = (36 + 8 : U32)
  +on_wheel : Bool = Bool.and(Bool.not(J.is_null(J.get(J.get(k, "body"), "plank"))), U32.is_ge(J.u32(J.get(J.get(k, "body"), "plank")), 200))
  +past_house : Bool = Bool.and(RS.i_ge(fx, ((185 + 38 : U32) * 16 : U32)), RS.i_le(fy, (bannerpeak.bot.Sec.row(36, 8) + 2 : U32)))
  bannerpeak.bot.Session.mill_goal.if5(Bool.and(Bool.not(past_house), RS.i_lt(cx, (185 + 38 : U32))), self, fx, fy, cx, ledge_x, ledge_y, on_wheel, past_house)

def bannerpeak.bot.Session.mill_goal.if1?(bpc: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Wheel", 0, 0)
    case False{}:
      bannerpeak.bot.Goal.of("Walk", (((185 + 28 : U32) * 16 : U32) + 12 : U32), 0)

def bannerpeak.bot.Session.mill_goal.if2?(bpc: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", (((185 + 68 : U32) * 16 : U32) + 8 : U32), 0)
    case False{}:
      bannerpeak.bot.Goal.of("Backflip", (((185 + 71 : U32) * 16 : U32) + 8 : U32), 0)

def bannerpeak.bot.Session.mill_goal.if3?(bpc: Bool, +fx: U32, +fy: U32) -> J.Val:
  match bpc:
    case True{}:
      +on_ledge : Bool = Bool.and(RS.i_le(fy, (bannerpeak.bot.Sec.row(36, 9) + 2 : U32)), RS.i_lt(fx, ((185 + 70 : U32) * 16 : U32)))
      bannerpeak.bot.Session.mill_goal.if2(on_ledge)
    case False{}:
      bannerpeak.bot.Goal.of("Walk", (((185 + 88 : U32) + 3 : U32) * 16 : U32), 0)

def bannerpeak.bot.Session.mill_goal.if4?(bpc: Bool, +self: J.Val, +fx: U32, +fy: U32, +cx: U32, +ledge_x: U32, +ledge_y: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", bannerpeak.bot.centre_x(bannerpeak.layout.info.banner_x(3)), 0)
    case False{}:
      bannerpeak.bot.Session.mill_goal.if3(Bool.and(bannerpeak.bot.Session.spark_missing(self, ledge_x, ledge_y), RS.i_ge(cx, (185 + 46 : U32))), fx, fy)

def bannerpeak.bot.Session.mill_goal.if5?(bpc: Bool, +self: J.Val, +fx: U32, +fy: U32, +cx: U32, +ledge_x: U32, +ledge_y: U32, +on_wheel: Bool, +past_house: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.mill_goal.if1(Bool.or(on_wheel, Bool.and(RS.i_ge(cx, (185 + 24 : U32)), RS.i_ge(fy, bannerpeak.bot.Sec.row(36, 0)))))
    case False{}:
      bannerpeak.bot.Session.mill_goal.if4(Bool.and(past_house, U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 8), 0)), self, fx, fy, cx, ledge_x, ledge_y)

def bannerpeak.bot.Session.shaft_goal?(+self: J.Val, +k: J.Val) -> J.Val:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +top : U32 = ((16 + 6 : U32) * 16 : U32)
  +b : J.Val = J.get(k, "body")
  +up : Bool = Bool.and(Bool.and(J.bool(J.get(b, "ground")), RS.i_le(fy, top)), RS.i_ge(fx, ((273 + 10 : U32) * 16 : U32)))
  bannerpeak.bot.Session.shaft_goal.if4(up, self, fx, fy, b)

def bannerpeak.bot.Session.shaft_goal.if1?(bpc: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Climb", (273 + 10 : U32), (16 + 13 : U32))
    case False{}:
      bannerpeak.bot.Goal.of("Climb", bannerpeak.layout.info.banner_x(4), bannerpeak.layout.info.banner_y(4))

def bannerpeak.bot.Session.shaft_goal.if2?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Climb", (273 + 4 : U32), (16 + 26 : U32))
    case False{}:
      bannerpeak.bot.Session.shaft_goal.if1(bannerpeak.bot.Session.spark_missing(self, (273 + 10 : U32), (16 + 13 : U32)))

def bannerpeak.bot.Session.shaft_goal.if3?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", ((273 + 8 : U32) * 16 : U32), 0)
    case False{}:
      bannerpeak.bot.Session.shaft_goal.if2(bannerpeak.bot.Session.spark_missing(self, (273 + 4 : U32), (16 + 26 : U32)), self)

def bannerpeak.bot.Session.shaft_goal.if4?(bpc: Bool, +self: J.Val, +fx: U32, +fy: U32, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", (((273 + 14 : U32) + 3 : U32) * 16 : U32), 0)
    case False{}:
      +foot : U32 = ((16 + 34 : U32) * 16 : U32)
      bannerpeak.bot.Session.shaft_goal.if3(Bool.and(Bool.and(J.bool(J.get(b, "ground")), U32.is_eq(fy, foot)), RS.i_lt(fx, ((273 + 6 : U32) * 16 : U32))), self)

def bannerpeak.bot.Session.snow_goal?(+self: J.Val, +k: J.Val) -> J.Val:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +cx : U32 = RS.i_div_euclid(fx, 16)
  +shelf_x : U32 = (287 + 25 : U32)
  +shelf_y : U32 = (12 + 3 : U32)
  +chute_x : U32 = (287 + 73 : U32)
  +chute_y : U32 = (12 + 0 : U32)
  bannerpeak.bot.Session.snow_goal.if6(bannerpeak.bot.Session.spark_missing(self, shelf_x, shelf_y), self, k, fy, cx, shelf_x, chute_x, chute_y)

def bannerpeak.bot.Session.snow_goal.if1?(bpc: Bool, +shelf_x: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", bannerpeak.bot.centre_x(shelf_x), 0)
    case False{}:
      bannerpeak.bot.Goal.of("Walk", ((287 + 31 : U32) * 16 : U32), 0)

def bannerpeak.bot.Session.snow_goal.if2?(bpc: Bool, +chute_x: U32, +chute_y: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Climb", chute_x, chute_y)
    case False{}:
      bannerpeak.bot.Goal.of("Walk", bannerpeak.bot.centre_x((287 + 73 : U32)), 0)

def bannerpeak.bot.Session.snow_goal.if3?(bpc: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Board", bannerpeak.bot.centre_x((287 + 54 : U32)), 0)
    case False{}:
      bannerpeak.bot.Goal.of("Walk", ((392 + 6 : U32) * 16 : U32), 0)

def bannerpeak.bot.Session.snow_goal.if4?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", bannerpeak.bot.centre_x(bannerpeak.layout.info.banner_x(5)), 0)
    case False{}:
      bannerpeak.bot.Session.snow_goal.if3(U32.is_eq(U32.and(J.u32(J.get(self, "doors")), 2), 0))

def bannerpeak.bot.Session.snow_goal.if5?(bpc: Bool, +self: J.Val, +k: J.Val, +fy: U32, +cx: U32, +chute_x: U32, +chute_y: U32) -> J.Val:
  match bpc:
    case True{}:
      +in_cols : Bool = Bool.and(RS.i_ge(cx, (287 + 72 : U32)), RS.i_lt(cx, (287 + 76 : U32)))
      +inside : Bool = Bool.and(in_cols, Bool.or(RS.i_lt(fy, bannerpeak.bot.Sec.row(12, 8)), Bool.not(J.bool(J.get(J.get(k, "body"), "ground")))))
      bannerpeak.bot.Session.snow_goal.if2(Bool.or(inside, Bool.and(String.eq(J.str(J.get(J.get(k, "body"), "mode")), "Wall"), Bool.and(RS.i_ge(cx, (287 + 70 : U32)), RS.i_lt(cx, (287 + 78 : U32))))), chute_x, chute_y)
    case False{}:
      bannerpeak.bot.Session.snow_goal.if4(U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 32), 0), self)

def bannerpeak.bot.Session.snow_goal.if6?(bpc: Bool, +self: J.Val, +k: J.Val, +fy: U32, +cx: U32, +shelf_x: U32, +chute_x: U32, +chute_y: U32) -> J.Val:
  match bpc:
    case True{}:
      +up : Bool = Bool.and(RS.i_le(fy, (bannerpeak.bot.Sec.row(12, 7) + 1 : U32)), Bool.and(RS.i_ge(cx, (287 + 20 : U32)), RS.i_lt(cx, (287 + 37 : U32))))
      bannerpeak.bot.Session.snow_goal.if1(up, shelf_x)
    case False{}:
      bannerpeak.bot.Session.snow_goal.if5(bannerpeak.bot.Session.spark_missing(self, chute_x, chute_y), self, k, fy, cx, chute_x, chute_y)

def bannerpeak.bot.Session.summit_goal?(+self: J.Val, +k: J.Val) -> J.Val:
  bannerpeak.bot.Session.summit_goal.if1(Bool.and(bannerpeak.boss.Boss.defeated(J.get(self, "boss")), U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 128), 0)))

def bannerpeak.bot.Session.summit_goal.if1?(bpc: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Goal.of("Walk", bannerpeak.bot.centre_x(bannerpeak.layout.info.banner_x(7)), 0)
    case False{}:
      bannerpeak.bot.Goal.of("Fight", 0, 0)

def bannerpeak.bot.Session.drive?(+self: J.Val, +k: J.Val, +goal: J.Val) -> J.Val:
  +b : J.Val = J.get(k, "body")
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(b)
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +held_jump : Bool = U32.is_ne(U32.and(J.u32(J.get(k, "prev")), 1), 0)
  +held_down : Bool = U32.is_ne(U32.and(J.u32(J.get(k, "prev")), 2), 0)
  bannerpeak.bot.Session.drive.if29(Bool.or(String.eq(J.str(J.get(b, "mode")), "Recover"), String.eq(J.str(J.get(b, "mode")), "Hurt")), self, k, goal, b, fx, fy, held_jump, held_down)

def bannerpeak.bot.Session.drive.if1?(bpc: Bool, +goal: J.Val, +tx: U32) -> U32:
  match bpc:
    case True{}:
      +tx : U32 = J.u32(J.get(goal, "a"))
      tx
    case False{}:
      tx

def bannerpeak.bot.Session.drive.if2?(bpc: Bool, +goal: J.Val, +tx: U32) -> U32:
  match bpc:
    case True{}:
      +tx : U32 = bannerpeak.bot.centre_x(J.u32(J.get(goal, "a")))
      tx
    case False{}:
      +tx = bannerpeak.bot.Session.drive.if1(Bool.and(Bool.not(String.eq(J.str(J.get(goal, "k")), "Wheel")), Bool.not(String.eq(J.str(J.get(goal, "k")), "Fight"))), goal, tx)
      tx

def bannerpeak.bot.Session.drive.if3?(bpc: Bool, +dx: U32, +d: U32) -> U32:
  match bpc:
    case True{}:
      +d : U32 = RS.i_signum(dx)
      d
    case False{}:
      d

def bannerpeak.bot.Session.drive.if4?(bpc: Bool, +d: U32, +creep: U32) -> U32:
  match bpc:
    case True{}:
      +creep : U32 = d
      creep
    case False{}:
      creep

def bannerpeak.bot.Session.drive.if5?(bpc: Bool, +creep: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, False{}, False{})
    case False{}:
      bannerpeak.bot.keys(creep, False{}, False{})

def bannerpeak.bot.Session.drive.if6?(bpc: Bool, +b: J.Val, +held_jump: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, Bool.not(held_jump), False{})
    case False{}:
      +creep : U32 = 0
      +creep = bannerpeak.bot.Session.drive.if4(RS.i_lt(RS.i_abs(J.u32(J.get(b, "vx"))), 150), d, creep)
      bannerpeak.bot.Session.drive.if5(RS.i_le(RS.i_abs(dx), 5), creep)

def bannerpeak.bot.Session.drive.if7?(bpc: Bool, +self: J.Val, +k: J.Val, +b: J.Val, +held_jump: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.walk_with_hops(self, k, d)
    case False{}:
      bannerpeak.bot.Session.drive.if6(Bool.and(RS.i_le(RS.i_abs(dx), 5), RS.i_lt(RS.i_abs(J.u32(J.get(b, "vx"))), 120)), b, held_jump, dx, d)

def bannerpeak.bot.Session.drive.if8?(bpc: Bool, +b: J.Val, +held_jump: Bool, +held_down: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, False{}, Bool.not(held_down))
    case False{}:
      bannerpeak.bot.keys(0, Bool.and(held_jump, RS.i_lt(J.u32(J.get(b, "vy")), 0)), False{})

def bannerpeak.bot.Session.drive.if9?(bpc: Bool, +self: J.Val, +k: J.Val, +b: J.Val, +held_jump: Bool, +held_down: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive.if7(RS.i_gt(RS.i_abs(dx), 40), self, k, b, held_jump, dx, d)
    case False{}:
      bannerpeak.bot.Session.drive.if8(Bool.and(RS.i_gt(J.u32(J.get(b, "vy")), 0), String.eq(J.str(J.get(b, "mode")), "Air")), b, held_jump, held_down)

def bannerpeak.bot.Session.drive.if10?(bpc: Bool, +self: J.Val, +k: J.Val, +held_down: Bool, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.walk_with_hops(self, k, d)
    case False{}:
      bannerpeak.bot.keys(0, False{}, Bool.not(held_down))

def bannerpeak.bot.Session.drive.if11?(bpc: Bool, +self: J.Val, +k: J.Val, +b: J.Val, +held_jump: Bool, +held_down: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive.if10(RS.i_gt(RS.i_abs(dx), 4), self, k, held_down, d)
    case False{}:
      bannerpeak.bot.keys(d, Bool.and(held_jump, RS.i_lt(J.u32(J.get(b, "vy")), 0)), False{})

def bannerpeak.bot.Session.drive.if12?(bpc: Bool, +i: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +i = J.set(i, "interact", J.JBool{True{}})
      i
    case False{}:
      i

def bannerpeak.bot.Session.drive.if13?(bpc: Bool, +self: J.Val, +k: J.Val, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.walk_with_hops(self, k, d)
    case False{}:
      +i : J.Val = bannerpeak.u.input_default()
      +i = bannerpeak.bot.Session.drive.if12(U32.is_eq(U32.and(J.u32(J.get(k, "prev")), 8), 0), i)
      i

def bannerpeak.bot.Session.drive.if14?(bpc: Bool, +self: J.Val, +k: J.Val, +b: J.Val, +held_jump: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive.if13(RS.i_gt(RS.i_abs(dx), 8), self, k, d)
    case False{}:
      bannerpeak.bot.keys(d, Bool.and(held_jump, RS.i_lt(J.u32(J.get(b, "vy")), 0)), False{})

def bannerpeak.bot.Session.drive.if15?(bpc: Bool, +d: U32, +creep: U32) -> U32:
  match bpc:
    case True{}:
      +creep : U32 = d
      creep
    case False{}:
      creep

def bannerpeak.bot.Session.drive.if16?(bpc: Bool, +creep: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, False{}, False{})
    case False{}:
      bannerpeak.bot.keys(creep, False{}, False{})

def bannerpeak.bot.Session.drive.if17?(bpc: Bool, +b: J.Val, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, False{}, True{})
    case False{}:
      +creep : U32 = 0
      +creep = bannerpeak.bot.Session.drive.if15(RS.i_lt(RS.i_abs(J.u32(J.get(b, "vx"))), 150), d, creep)
      bannerpeak.bot.Session.drive.if16(RS.i_le(RS.i_abs(dx), 4), creep)

def bannerpeak.bot.Session.drive.if18?(bpc: Bool, +b: J.Val, +held_jump: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, Bool.not(held_jump), True{})
    case False{}:
      bannerpeak.bot.Session.drive.if17(Bool.and(RS.i_le(RS.i_abs(dx), 4), RS.i_lt(RS.i_abs(J.u32(J.get(b, "vx"))), 150)), b, dx, d)

def bannerpeak.bot.Session.drive.if19?(bpc: Bool, +self: J.Val, +k: J.Val, +b: J.Val, +held_jump: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.walk_with_hops(self, k, d)
    case False{}:
      bannerpeak.bot.Session.drive.if18(J.bool(J.get(b, "crouch")), b, held_jump, dx, d)

def bannerpeak.bot.Session.drive.if20?(bpc: Bool, +self: J.Val, +k: J.Val, +b: J.Val, +held_jump: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive.if19(RS.i_gt(RS.i_abs(dx), 40), self, k, b, held_jump, dx, d)
    case False{}:
      bannerpeak.bot.keys(0, Bool.and(held_jump, RS.i_lt(J.u32(J.get(b, "vy")), 0)), False{})

def bannerpeak.bot.Session.drive.if21?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +ground: Bool, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.fight(self, k)
    case False{}:
      bannerpeak.bot.Session.drive_walk(self, k, goal, fx, fy, d, ground, held_jump)

def bannerpeak.bot.Session.drive.if22?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +ground: Bool, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.climb(self, k, J.u32(J.get(goal, "a")), J.u32(J.get(goal, "b")))
    case False{}:
      bannerpeak.bot.Session.drive.if21(String.eq(J.str(J.get(goal, "k")), "Fight"), self, k, goal, fx, fy, held_jump, ground, d)

def bannerpeak.bot.Session.drive.if23?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +ground: Bool, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.ride(self, k)
    case False{}:
      bannerpeak.bot.Session.drive.if22(String.eq(J.str(J.get(goal, "k")), "Climb"), self, k, goal, fx, fy, held_jump, ground, d)

def bannerpeak.bot.Session.drive.if24?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +b: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +ground: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive.if20(ground, self, k, b, held_jump, dx, d)
    case False{}:
      bannerpeak.bot.Session.drive.if23(String.eq(J.str(J.get(goal, "k")), "Wheel"), self, k, goal, fx, fy, held_jump, ground, d)

def bannerpeak.bot.Session.drive.if25?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +b: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +ground: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive.if14(ground, self, k, b, held_jump, dx, d)
    case False{}:
      bannerpeak.bot.Session.drive.if24(String.eq(J.str(J.get(goal, "k")), "Backflip"), self, k, goal, b, fx, fy, held_jump, ground, dx, d)

def bannerpeak.bot.Session.drive.if26?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +b: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +held_down: Bool, +ground: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive.if11(ground, self, k, b, held_jump, held_down, dx, d)
    case False{}:
      bannerpeak.bot.Session.drive.if25(String.eq(J.str(J.get(goal, "k")), "Board"), self, k, goal, b, fx, fy, held_jump, ground, dx, d)

def bannerpeak.bot.Session.drive.if27?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +b: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +held_down: Bool, +ground: Bool, +dx: U32, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive.if9(ground, self, k, b, held_jump, held_down, dx, d)
    case False{}:
      bannerpeak.bot.Session.drive.if26(String.eq(J.str(J.get(goal, "k")), "Hatch"), self, k, goal, b, fx, fy, held_jump, held_down, ground, dx, d)

def bannerpeak.bot.Session.drive.if28?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +b: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +held_down: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, False{}, True{})
    case False{}:
      +ground : Bool = Bool.and(J.bool(J.get(b, "ground")), Bool.or(Bool.or(String.eq(J.str(J.get(b, "mode")), "Ground"), String.eq(J.str(J.get(b, "mode")), "Brace")), String.eq(J.str(J.get(b, "mode")), "Belly")))
      +tx : U32 = fx
      +tx = bannerpeak.bot.Session.drive.if2(String.eq(J.str(J.get(goal, "k")), "Climb"), goal, tx)
      +dx : U32 = (tx - fx : U32)
      +d : U32 = 0
      +d = bannerpeak.bot.Session.drive.if3(RS.i_ge(RS.i_abs(dx), 3), dx, d)
      bannerpeak.bot.Session.drive.if27(String.eq(J.str(J.get(goal, "k")), "Slam"), self, k, goal, b, fx, fy, held_jump, held_down, ground, dx, d)

def bannerpeak.bot.Session.drive.if29?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +b: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +held_down: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.u.input_default()
    case False{}:
      bannerpeak.bot.Session.drive.if28(Bool.or(String.eq(J.str(J.get(b, "mode")), "Wind"), String.eq(J.str(J.get(b, "mode")), "Pound")), self, k, goal, b, fx, fy, held_jump, held_down)

def bannerpeak.bot.Session.drive_walk?(+self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +ground: Bool, +held_jump: Bool) -> J.Val:
  +b : J.Val = J.get(k, "body")
  bannerpeak.bot.Session.drive_walk.if31(Bool.not(ground), self, k, goal, fx, fy, d, held_jump, b)

def bannerpeak.bot.Session.drive_walk.if1?(bpc: Bool, +over: U32) -> U32:
  match bpc:
    case True{}:
      +over : U32 = 0
      over
    case False{}:
      over

def bannerpeak.bot.Session.drive_walk.if2?(bpc: Bool, +d: U32, +held_jump: Bool, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +over : U32 = 1
      +over = bannerpeak.bot.Session.drive_walk.if1(RS.i_lt(J.u32(J.get(b, "vy")), 0), over)
      bannerpeak.bot.keys(over, Bool.and(held_jump, RS.i_lt(J.u32(J.get(b, "vy")), 0)), False{})
    case False{}:
      bannerpeak.bot.keys(d, Bool.and(held_jump, RS.i_lt(J.u32(J.get(b, "vy")), 0)), False{})

def bannerpeak.bot.Session.drive_walk.if3?(bpc: Bool, +held_jump: Bool, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(RS.i_neg(RS.i_signum(J.u32(J.get(b, "vx")))), False{}, False{})
    case False{}:
      bannerpeak.bot.keys(0, Bool.not(held_jump), False{})

def bannerpeak.bot.Session.drive_walk.if4?(bpc: Bool, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(d, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.bot.keys(d, False{}, False{})

def bannerpeak.bot.Session.drive_walk.if5?(bpc: Bool, +self: J.Val, +k: J.Val, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive_walk.if4(Bool.and(U32.is_ne(d, 0), bannerpeak.bot.Session.obstacle_ahead(self, k, d, False{})), d, held_jump)
    case False{}:
      bannerpeak.bot.Session.walk_with_hops(self, k, d)

def bannerpeak.bot.Session.drive_walk.if6?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(4294967295, False{}, False{})
    case False{}:
      bannerpeak.bot.Session.drive_walk.if5(String.eq(J.str(J.get(goal, "k")), "Down"), self, k, d, held_jump)

def bannerpeak.bot.Session.drive_walk.if7?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val, +lane: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(4294967295, False{}, False{})
    case False{}:
      bannerpeak.bot.Session.drive_walk.if6(Bool.and(Bool.and(Bool.and(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 1), 0), U32.is_eq(fy, lane)), RS.i_lt(J.u32(J.get(b, "face")), 0)), RS.i_gt(fx, ((0 + 27 : U32) * 16 : U32))), RS.i_lt(fx, ((0 + 56 : U32) * 16 : U32))), self, k, goal, d, held_jump)

def bannerpeak.bot.Session.drive_walk.if8?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(1, Bool.not(held_jump), False{})
    case False{}:
      +lane : U32 = ((32 + 18 : U32) * 16 : U32)
      bannerpeak.bot.Session.drive_walk.if7(Bool.and(Bool.and(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 1), 0), U32.is_eq(fy, lane)), RS.i_gt(fx, ((0 + 55 : U32) * 16 : U32))), RS.i_lt(fx, ((0 + 60 : U32) * 16 : U32))), self, k, goal, fx, fy, d, held_jump, b, lane)

def bannerpeak.bot.Session.drive_walk.if9?(bpc: Bool, +dir: U32) -> U32:
  match bpc:
    case True{}:
      +dir : U32 = 1
      dir
    case False{}:
      dir

def bannerpeak.bot.Session.drive_walk.if10?(bpc: Bool, +held_jump: Bool, +b: J.Val, +dir: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(dir, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.bot.keys(RS.i_neg(RS.i_signum(J.u32(J.get(b, "vx")))), False{}, False{})

def bannerpeak.bot.Session.drive_walk.if11?(bpc: Bool, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(d, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.bot.keys(d, False{}, False{})

def bannerpeak.bot.Session.drive_walk.if12?(bpc: Bool, +self: J.Val, +k: J.Val, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive_walk.if11(Bool.and(U32.is_ne(d, 0), bannerpeak.bot.Session.obstacle_ahead(self, k, d, False{})), d, held_jump)
    case False{}:
      bannerpeak.bot.Session.walk_with_hops(self, k, d)

def bannerpeak.bot.Session.drive_walk.if13?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(4294967295, False{}, False{})
    case False{}:
      bannerpeak.bot.Session.drive_walk.if12(String.eq(J.str(J.get(goal, "k")), "Down"), self, k, d, held_jump)

def bannerpeak.bot.Session.drive_walk.if14?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val, +lane: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(4294967295, False{}, False{})
    case False{}:
      bannerpeak.bot.Session.drive_walk.if13(Bool.and(Bool.and(Bool.and(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 1), 0), U32.is_eq(fy, lane)), RS.i_lt(J.u32(J.get(b, "face")), 0)), RS.i_gt(fx, ((0 + 27 : U32) * 16 : U32))), RS.i_lt(fx, ((0 + 56 : U32) * 16 : U32))), self, k, goal, d, held_jump)

def bannerpeak.bot.Session.drive_walk.if15?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val, +dir: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive_walk.if10(RS.i_lt(RS.i_abs(J.u32(J.get(b, "vx"))), 320), held_jump, b, dir)
    case False{}:
      +lane : U32 = ((32 + 18 : U32) * 16 : U32)
      bannerpeak.bot.Session.drive_walk.if14(Bool.and(Bool.and(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 1), 0), U32.is_eq(fy, lane)), RS.i_gt(fx, ((0 + 55 : U32) * 16 : U32))), RS.i_lt(fx, ((0 + 60 : U32) * 16 : U32))), self, k, goal, fx, fy, d, held_jump, b, lane)

def bannerpeak.bot.Session.drive_walk.if16?(bpc: Bool, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(d, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.bot.keys(d, False{}, False{})

def bannerpeak.bot.Session.drive_walk.if17?(bpc: Bool, +self: J.Val, +k: J.Val, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive_walk.if16(Bool.and(U32.is_ne(d, 0), bannerpeak.bot.Session.obstacle_ahead(self, k, d, False{})), d, held_jump)
    case False{}:
      bannerpeak.bot.Session.walk_with_hops(self, k, d)

def bannerpeak.bot.Session.drive_walk.if18?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(4294967295, False{}, False{})
    case False{}:
      bannerpeak.bot.Session.drive_walk.if17(String.eq(J.str(J.get(goal, "k")), "Down"), self, k, d, held_jump)

def bannerpeak.bot.Session.drive_walk.if19?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val, +lane: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(4294967295, False{}, False{})
    case False{}:
      bannerpeak.bot.Session.drive_walk.if18(Bool.and(Bool.and(Bool.and(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 1), 0), U32.is_eq(fy, lane)), RS.i_lt(J.u32(J.get(b, "face")), 0)), RS.i_gt(fx, ((0 + 27 : U32) * 16 : U32))), RS.i_lt(fx, ((0 + 56 : U32) * 16 : U32))), self, k, goal, d, held_jump)

def bannerpeak.bot.Session.drive_walk.if20?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(1, False{}, True{})
    case False{}:
      +lane : U32 = ((32 + 18 : U32) * 16 : U32)
      bannerpeak.bot.Session.drive_walk.if19(Bool.and(Bool.and(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 1), 0), U32.is_eq(fy, lane)), RS.i_gt(fx, ((0 + 55 : U32) * 16 : U32))), RS.i_lt(fx, ((0 + 60 : U32) * 16 : U32))), self, k, goal, fx, fy, d, held_jump, b, lane)

def bannerpeak.bot.Session.drive_walk.if21?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(1, Bool.not(held_jump), True{})
    case False{}:
      bannerpeak.bot.Session.drive_walk.if20(RS.i_ge(RS.i_abs(J.u32(J.get(b, "vx"))), 760), self, k, goal, fx, fy, d, held_jump, b)

def bannerpeak.bot.Session.drive_walk.if22?(bpc: Bool, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(d, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.bot.keys(d, False{}, False{})

def bannerpeak.bot.Session.drive_walk.if23?(bpc: Bool, +self: J.Val, +k: J.Val, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive_walk.if22(Bool.and(U32.is_ne(d, 0), bannerpeak.bot.Session.obstacle_ahead(self, k, d, False{})), d, held_jump)
    case False{}:
      bannerpeak.bot.Session.walk_with_hops(self, k, d)

def bannerpeak.bot.Session.drive_walk.if24?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(4294967295, False{}, False{})
    case False{}:
      bannerpeak.bot.Session.drive_walk.if23(String.eq(J.str(J.get(goal, "k")), "Down"), self, k, d, held_jump)

def bannerpeak.bot.Session.drive_walk.if25?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val, +lane: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(4294967295, False{}, False{})
    case False{}:
      bannerpeak.bot.Session.drive_walk.if24(Bool.and(Bool.and(Bool.and(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 1), 0), U32.is_eq(fy, lane)), RS.i_lt(J.u32(J.get(b, "face")), 0)), RS.i_gt(fx, ((0 + 27 : U32) * 16 : U32))), RS.i_lt(fx, ((0 + 56 : U32) * 16 : U32))), self, k, goal, d, held_jump)

def bannerpeak.bot.Session.drive_walk.if26?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive_walk.if21(J.bool(J.get(b, "crouch")), self, k, goal, fx, fy, d, held_jump, b)
    case False{}:
      +lane : U32 = ((32 + 18 : U32) * 16 : U32)
      bannerpeak.bot.Session.drive_walk.if25(Bool.and(Bool.and(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 1), 0), U32.is_eq(fy, lane)), RS.i_gt(fx, ((0 + 55 : U32) * 16 : U32))), RS.i_lt(fx, ((0 + 60 : U32) * 16 : U32))), self, k, goal, fx, fy, d, held_jump, b, lane)

def bannerpeak.bot.Session.drive_walk.if27?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val, +h: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(d, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.bot.Session.drive_walk.if26(Bool.and(U32.is_eq(h, 47), RS.i_gt(d, 0)), self, k, goal, fx, fy, d, held_jump, b)

def bannerpeak.bot.Session.drive_walk.if28?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val, +h: U32) -> J.Val:
  match bpc:
    case True{}:
      +dir : U32 = 4294967295
      +dir = bannerpeak.bot.Session.drive_walk.if9(U32.is_eq(h, 62), dir)
      bannerpeak.bot.Session.drive_walk.if15(Bool.or(U32.is_eq(d, dir), U32.is_eq(d, 0)), self, k, goal, fx, fy, d, held_jump, b, dir)
    case False{}:
      bannerpeak.bot.Session.drive_walk.if27(U32.is_eq(h, 124), self, k, goal, fx, fy, d, held_jump, b, h)

def bannerpeak.bot.Session.drive_walk.if29?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val, +h: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive_walk.if8(Bool.or(RS.i_ge(RS.i_abs(J.u32(J.get(b, "vx"))), 1100), Bool.and(U32.is_gt(J.u32(J.get(b, "hop_t")), 0), U32.is_ge(J.u32(J.get(b, "hop_next")), 2))), self, k, goal, fx, fy, d, held_jump, b)
    case False{}:
      bannerpeak.bot.Session.drive_walk.if28(Bool.or(U32.is_eq(h, 62), U32.is_eq(h, 60)), self, k, goal, fx, fy, d, held_jump, b, h)

def bannerpeak.bot.Session.drive_walk.if30?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive_walk.if3(RS.i_gt(RS.i_abs(J.u32(J.get(b, "vx"))), 200), held_jump, b)
    case False{}:
      +h : U32 = bannerpeak.bot.Session.hint_here(self, b)
      bannerpeak.bot.Session.drive_walk.if29(Bool.and(U32.is_eq(h, 38), RS.i_gt(d, 0)), self, k, goal, fx, fy, d, held_jump, b, h)

def bannerpeak.bot.Session.drive_walk.if31?(bpc: Bool, +self: J.Val, +k: J.Val, +goal: J.Val, +fx: U32, +fy: U32, +d: U32, +held_jump: Bool, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.drive_walk.if2(Bool.and(Bool.and(U32.is_ne(U32.and(J.u32(J.get(self, "banners")), 2), 0), Bool.and(RS.i_ge(fx, ((0 + 22 : U32) * 16 : U32)), RS.i_lt(fx, ((0 + 26 : U32) * 16 : U32)))), RS.i_gt(fy, ((32 + 17 : U32) * 16 : U32))), d, held_jump, b)
    case False{}:
      bannerpeak.bot.Session.drive_walk.if30(Bool.and(Bool.and(U32.is_ne(U32.and(J.u32(J.get(self, "banners")), 2), 0), U32.is_eq(fy, ((32 + 21 : U32) * 16 : U32))), Bool.and(RS.i_ge(RS.i_div_euclid(fx, 16), (0 + 23 : U32)), RS.i_lt(RS.i_div_euclid(fx, 16), (0 + 26 : U32)))), self, k, goal, fx, fy, d, held_jump, b)

def bannerpeak.bot.Session.foresee_step?(+self: J.Val, +b: J.Val, +prev: Bool, +n: U32, +jump_until: U32, +e: U32) -> J.Val:
  +jump : Bool = U32.is_lt(n, jump_until)
  +cmd : J.Val = J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32(1)), "jump", J.JBool{jump}), "jump_edge", J.JBool{Bool.and(jump, Bool.not(prev))}), "down", J.JBool{False{}}), "down_edge", J.JBool{False{}}), "dive_edge", J.JBool{False{}}), "brace", J.JBool{False{}})
  +now : J.Val = bannerpeak.layout.movers(((J.u32(J.get(self, "elapsed")) + 1 : U32) + n : U32))
  +before : J.Val = bannerpeak.layout.movers((J.u32(J.get(self, "elapsed")) + n : U32))
  +out : J.Val = bannerpeak.u.vec_new()
  +bb : J.Val = b
  +_t1 = bannerpeak.body.Body.step(bb, cmd, bannerpeak.bot.shipped_knobs(), J.get(self, "changed"), now, before, out)
  +bb : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t1)
  +out : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t1)
  bb

def bannerpeak.bot.Session.foresee_settle?(+self: J.Val, +b: J.Val, +n: U32, +m: U32) -> J.Val:
  bannerpeak.bot.Session.foresee_settle.if2(U32.is_ge(m, 16), self, b, n, m)

def bannerpeak.bot.Session.foresee_settle.if1?(bpc: Bool, +b: J.Val, +brake: U32) -> U32:
  match bpc:
    case True{}:
      +brake : U32 = RS.i_neg(RS.i_signum(J.u32(J.get(b, "vx"))))
      brake
    case False{}:
      brake

def bannerpeak.bot.Session.foresee_settle.if2?(bpc: Bool, +self: J.Val, +b: J.Val, +n: U32, +m: U32) -> J.Val:
  match bpc:
    case True{}:
      b
    case False{}:
      +brake : U32 = 0
      +brake = bannerpeak.bot.Session.foresee_settle.if1(RS.i_gt(RS.i_abs(J.u32(J.get(b, "vx"))), 150), b, brake)
      +cmd : J.Val = J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32(brake)), "jump", J.JBool{False{}}), "jump_edge", J.JBool{False{}}), "down", J.JBool{False{}}), "down_edge", J.JBool{False{}}), "dive_edge", J.JBool{False{}}), "brace", J.JBool{False{}})
      +step : U32 = ((n + 1 : U32) + m : U32)
      +now : J.Val = bannerpeak.layout.movers(((J.u32(J.get(self, "elapsed")) + 1 : U32) + step : U32))
      +before : J.Val = bannerpeak.layout.movers((J.u32(J.get(self, "elapsed")) + step : U32))
      +out : J.Val = bannerpeak.u.vec_new()
      +bb : J.Val = b
      +_t1 = bannerpeak.body.Body.step(bb, cmd, bannerpeak.bot.shipped_knobs(), J.get(self, "changed"), now, before, out)
      +bb : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t1)
      +out : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t1)
      bannerpeak.bot.Session.foresee_settle(self, bb, n, (m + 1 : U32))

def bannerpeak.bot.Session.foresee_want?(+b: J.Val, +want: U32) -> Bool:
  bannerpeak.bot.Session.foresee_want.if1(U32.is_eq(want, 0), b)

def bannerpeak.bot.Session.foresee_want.if1?(bpc: Bool, +b: J.Val) -> Bool:
  match bpc:
    case True{}:
      +roof : U32 = ((36 + 8 : U32) * 16 : U32)
      +foot : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(b)
      +_t1 : Sigma<&2, &2, U32, _ => U32> = foot
      +_t2 : Sigma<&2, &2, U32, _ => U32> = foot
      Bool.and(Bool.and(Bool.and(J.bool(J.get(b, "ground")), J.is_null(J.get(b, "plank"))), U32.is_eq(bannerpeak.u.snd(U32, U32, _t1), roof)), RS.i_gt(bannerpeak.u.fst(U32, U32, _t2), ((185 + 38 : U32) * 16 : U32)))
    case False{}:
      Bool.and(Bool.not(J.is_null(J.get(b, "plank"))), U32.is_ge(J.u32(J.get(b, "plank")), 200))

def bannerpeak.bot.Session.foresee_go?(+self: J.Val, +b: J.Val, +prev: Bool, +n: U32, +limit: U32, +jump_until: U32, +settle: Bool, +want: U32) -> Bool:
  bannerpeak.bot.Session.foresee_go.if4(U32.is_ge(n, limit), self, b, prev, n, limit, jump_until, settle, want)

def bannerpeak.bot.Session.foresee_go.if1?(bpc: Bool, +self: J.Val, +n: U32, +nb: J.Val, +rest: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +rest : J.Val = bannerpeak.bot.Session.foresee_settle(self, nb, n, 0)
      rest
    case False{}:
      rest

def bannerpeak.bot.Session.foresee_go.if2?(bpc: Bool, +self: J.Val, +n: U32, +limit: U32, +jump_until: U32, +settle: Bool, +want: U32, +jump: Bool, +nb: J.Val) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.bot.Session.foresee_go(self, nb, jump, (n + 1 : U32), limit, jump_until, settle, want)

def bannerpeak.bot.Session.foresee_go.if3?(bpc: Bool, +self: J.Val, +n: U32, +limit: U32, +jump_until: U32, +settle: Bool, +want: U32, +jump: Bool, +nb: J.Val) -> Bool:
  match bpc:
    case True{}:
      +rest : J.Val = nb
      +rest = bannerpeak.bot.Session.foresee_go.if1(settle, self, n, nb, rest)
      bannerpeak.bot.Session.foresee_want(rest, want)
    case False{}:
      +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(nb)
      bannerpeak.bot.Session.foresee_go.if2(RS.i_gt(bannerpeak.u.snd(U32, U32, _t1), (64 * 16 : U32)), self, n, limit, jump_until, settle, want, jump, nb)

def bannerpeak.bot.Session.foresee_go.if4?(bpc: Bool, +self: J.Val, +b: J.Val, +prev: Bool, +n: U32, +limit: U32, +jump_until: U32, +settle: Bool, +want: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +jump : Bool = U32.is_lt(n, jump_until)
      +nb : J.Val = bannerpeak.bot.Session.foresee_step(self, b, prev, n, jump_until, 0)
      bannerpeak.bot.Session.foresee_go.if3(Bool.and(U32.is_gt(n, 1), J.bool(J.get(nb, "ground"))), self, n, limit, jump_until, settle, want, jump, nb)

def bannerpeak.bot.Session.foresee?(+self: J.Val, +k: J.Val, +limit: U32, +jump_until: U32, +settle: Bool, +want: U32) -> Bool:
  bannerpeak.bot.Session.foresee_go(self, J.get(k, "body"), U32.is_ne(U32.and(J.u32(J.get(k, "prev")), 1), 0), 0, limit, jump_until, settle, want)

def bannerpeak.bot.Session.ride?(+self: J.Val, +k: J.Val) -> J.Val:
  +b : J.Val = J.get(k, "body")
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(b)
  +fx : U32 = bannerpeak.u.fst(U32, U32, _t1)
  +held_jump : Bool = U32.is_ne(U32.and(J.u32(J.get(k, "prev")), 1), 0)
  +cx : U32 = 3496
  +cy : U32 = 760
  +spark_x : U32 = (185 + 33 : U32)
  +spark_y : U32 = (36 + 5 : U32)
  bannerpeak.bot.Session.ride.if14(Bool.not(J.bool(J.get(b, "ground"))), self, k, b, fx, held_jump, cx, cy, spark_x, spark_y)

def bannerpeak.bot.Session.ride.if1?(bpc: Bool, +b: J.Val, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, Bool.and(held_jump, RS.i_lt(J.u32(J.get(b, "vy")), 0)), False{})
    case False{}:
      bannerpeak.bot.keys(1, Bool.and(held_jump, RS.i_lt(J.u32(J.get(b, "vy")), 0)), False{})

def bannerpeak.bot.Session.ride.if2?(bpc: Bool, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(d, False{}, False{})
    case False{}:
      bannerpeak.bot.keys(0, False{}, False{})

def bannerpeak.bot.Session.ride.if3?(bpc: Bool, +b: J.Val, +d: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.ride.if2(RS.i_lt(RS.i_abs(J.u32(J.get(b, "vx"))), 200), d)
    case False{}:
      bannerpeak.u.input_default()

def bannerpeak.bot.Session.ride.if4?(bpc: Bool, +b: J.Val, +fx: U32, +held_jump: Bool, +cx: U32, +p: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, Bool.not(held_jump), False{})
    case False{}:
      +d : U32 = RS.i_signum((cx - fx : U32))
      bannerpeak.bot.Session.ride.if3(Bool.and(RS.i_gt((fx + (d * 4 : U32) : U32), (J.u32(J.get(p, "x")) + 4 : U32)), RS.i_lt((fx + (d * 4 : U32) : U32), ((J.u32(J.get(p, "x")) + J.u32(J.get(p, "w")) : U32) - 4 : U32))), b, d)

def bannerpeak.bot.Session.ride.if5?(bpc: Bool, +b: J.Val, +fx: U32, +held_jump: Bool, +cx: U32, +p: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.ride.if4(RS.i_le(RS.i_abs((fx - cx : U32)), 9), b, fx, held_jump, cx, p)
    case False{}:
      bannerpeak.u.input_default()

def bannerpeak.bot.Session.ride.if6?(bpc: Bool, +b: J.Val, +fx: U32, +held_jump: Bool, +cx: U32, +cy: U32, +ms: J.Val, +pi: U32) -> J.Val:
  match bpc:
    case True{}:
      +p : J.Val = J.get(J.nth(ms, pi), "p")
      +near_top : Bool = Bool.and(RS.i_le(RS.i_abs(((J.u32(J.get(p, "x")) + RS.i_div(J.u32(J.get(p, "w")), 2) : U32) - cx : U32)), 24), RS.i_le(J.u32(J.get(p, "y")), (cy - 30 : U32)))
      bannerpeak.bot.Session.ride.if5(near_top, b, fx, held_jump, cx, p)
    case False{}:
      bannerpeak.u.input_default()

def bannerpeak.bot.Session.ride.if7?(bpc: Bool, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(1, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.u.input_default()

def bannerpeak.bot.Session.ride.if8?(bpc: Bool, +self: J.Val, +k: J.Val, +b: J.Val, +fx: U32, +held_jump: Bool, +cx: U32, +cy: U32) -> J.Val:
  match bpc:
    case True{}:
      +ms : J.Val = bannerpeak.layout.movers(J.u32(J.get(self, "elapsed")))
      +pi : U32 = bannerpeak.bot.Self_plank(ms, J.u32(J.get(b, "plank")), 0)
      bannerpeak.bot.Session.ride.if6(U32.is_lt(pi, J.len(ms)), b, fx, held_jump, cx, cy, ms, pi)
    case False{}:
      bannerpeak.bot.Session.ride.if7(bannerpeak.bot.Session.foresee(self, k, 50, 17, False{}, 0), held_jump)

def bannerpeak.bot.Session.ride.if9?(bpc: Bool, +self: J.Val, +k: J.Val, +b: J.Val, +fx: U32, +held_jump: Bool, +cx: U32, +cy: U32, +spark_x: U32, +spark_y: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(RS.i_neg(RS.i_signum(J.u32(J.get(b, "vx")))), False{}, False{})
    case False{}:
      bannerpeak.bot.Session.ride.if8(bannerpeak.bot.Session.spark_missing(self, spark_x, spark_y), self, k, b, fx, held_jump, cx, cy)

def bannerpeak.bot.Session.ride.if10?(bpc: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(4294967295, False{}, False{})
    case False{}:
      bannerpeak.bot.keys(1, False{}, False{})

def bannerpeak.bot.Session.ride.if11?(bpc: Bool, +b: J.Val, +fx: U32, +left: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(4294967295, False{}, False{})
    case False{}:
      bannerpeak.bot.Session.ride.if10(Bool.and(RS.i_lt(J.u32(J.get(b, "vx")), 4294967196), RS.i_gt(fx, (left + 14 : U32))))

def bannerpeak.bot.Session.ride.if12?(bpc: Bool, +b: J.Val, +fx: U32, +held_jump: Bool, +left: U32, +right: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(1, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.bot.Session.ride.if11(Bool.and(RS.i_ge(fx, (right - 15 : U32)), RS.i_gt(J.u32(J.get(b, "vx")), 300)), b, fx, left)

def bannerpeak.bot.Session.ride.if13?(bpc: Bool, +self: J.Val, +k: J.Val, +b: J.Val, +fx: U32, +held_jump: Bool, +cx: U32, +cy: U32, +spark_x: U32, +spark_y: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.ride.if9(RS.i_gt(RS.i_abs(J.u32(J.get(b, "vx"))), 450), self, k, b, fx, held_jump, cx, cy, spark_x, spark_y)
    case False{}:
      +left : U32 = ((185 + 24 : U32) * 16 : U32)
      +right : U32 = ((185 + 29 : U32) * 16 : U32)
      bannerpeak.bot.Session.ride.if12(Bool.and(RS.i_gt(J.u32(J.get(b, "vx")), 700), bannerpeak.bot.Session.foresee(self, k, 45, 16, True{}, 1)), b, fx, held_jump, left, right)

def bannerpeak.bot.Session.ride.if14?(bpc: Bool, +self: J.Val, +k: J.Val, +b: J.Val, +fx: U32, +held_jump: Bool, +cx: U32, +cy: U32, +spark_x: U32, +spark_y: U32) -> J.Val:
  match bpc:
    case True{}:
      +up : Bool = Bool.and(Bool.and(bannerpeak.bot.Session.spark_missing(self, spark_x, spark_y), RS.i_lt(RS.i_abs(J.u32(J.get(b, "vx"))), 300)), RS.i_lt(RS.i_abs((fx - cx : U32)), 30))
      bannerpeak.bot.Session.ride.if1(up, b, held_jump)
    case False{}:
      bannerpeak.bot.Session.ride.if13(Bool.and(Bool.not(J.is_null(J.get(b, "plank"))), U32.is_ge(J.u32(J.get(b, "plank")), 200)), self, k, b, fx, held_jump, cx, cy, spark_x, spark_y)

def bannerpeak.bot.Self_plank?(+ms: J.Val, +id: U32, +n: U32) -> U32:
  bannerpeak.bot.Self_plank.if2(U32.is_ge(n, J.len(ms)), ms, id, n)

def bannerpeak.bot.Self_plank.if1?(bpc: Bool, +ms: J.Val, +id: U32, +n: U32) -> U32:
  match bpc:
    case True{}:
      n
    case False{}:
      bannerpeak.bot.Self_plank(ms, id, (n + 1 : U32))

def bannerpeak.bot.Self_plank.if2?(bpc: Bool, +ms: J.Val, +id: U32, +n: U32) -> U32:
  match bpc:
    case True{}:
      n
    case False{}:
      bannerpeak.bot.Self_plank.if1(U32.is_eq(J.u32(J.get(J.nth(ms, n), "id")), id), ms, id, n)

def bannerpeak.bot.Session.climb?(+self: J.Val, +k: J.Val, +tcx: U32, +tcy: U32) -> J.Val:
  +b : J.Val = J.get(k, "body")
  +held_jump : Bool = U32.is_ne(U32.and(J.u32(J.get(k, "prev")), 1), 0)
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(b)
  +x : U32 = bannerpeak.u.fst(U32, U32, _t1)
  bannerpeak.bot.Session.climb.if10(J.bool(J.get(b, "ground")), self, k, tcx, tcy, b, held_jump, x)

def bannerpeak.bot.Session.climb.if1?(bpc: Bool, +lo: U32, +hi: U32) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      +lo : U32 = ((273 + 6 : U32) * 16 : U32)
      +hi : U32 = ((273 + 10 : U32) * 16 : U32)
      (hi, lo)
    case False{}:
      (hi, lo)

def bannerpeak.bot.Session.climb.if2?(bpc: Bool, +dd: U32) -> U32:
  match bpc:
    case True{}:
      +dd : U32 = 4294967295
      dd
    case False{}:
      dd

def bannerpeak.bot.Session.climb.if3?(bpc: Bool, +x: U32, +hi: U32, +dd: U32) -> U32:
  match bpc:
    case True{}:
      +dd : U32 = 1
      dd
    case False{}:
      +dd = bannerpeak.bot.Session.climb.if2(RS.i_gt(x, (hi - 6 : U32)), dd)
      dd

def bannerpeak.bot.Session.climb.if4?(bpc: Bool, +side: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(RS.i_neg(side), True{}, False{})
    case False{}:
      bannerpeak.bot.keys(side, False{}, False{})

def bannerpeak.bot.Session.climb.if5?(bpc: Bool, +side: U32, +wait: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.climb.if4(U32.is_eq(wait, 0), side)
    case False{}:
      bannerpeak.bot.keys(RS.i_neg(side), True{}, False{})

def bannerpeak.bot.Session.climb.if6?(bpc: Bool, +self: J.Val, +k: J.Val, +side: U32, +tx: U32, +ty: U32) -> J.Val:
  match bpc:
    case True{}:
      +wait : U32 = bannerpeak.bot.Session.kick_wait(self, k, 0, tx, ty)
      bannerpeak.bot.Session.climb.if5(U32.is_le(wait, 56), side, wait)
    case False{}:
      bannerpeak.bot.keys(RS.i_neg(side), True{}, False{})

def bannerpeak.bot.Session.climb.if7?(bpc: Bool, +self: J.Val, +k: J.Val, +b: J.Val, +side: U32, +tx: U32, +ty: U32, +fy: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(side, False{}, False{})
    case False{}:
      +_t4 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(b)
      bannerpeak.bot.Session.climb.if6(Bool.and(RS.i_lt(RS.i_abs((ty - fy : U32)), 90), RS.i_lt(RS.i_abs((tx - bannerpeak.u.fst(U32, U32, _t4) : U32)), 110)), self, k, side, tx, ty)

def bannerpeak.bot.Session.climb.if8?(bpc: Bool, +self: J.Val, +k: J.Val, +tcx: U32, +tcy: U32, +b: J.Val, +side: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(side, False{}, False{})
    case False{}:
      +tx : U32 = bannerpeak.bot.centre_x(tcx)
      +ty : U32 = ((tcy * 16 : U32) + RS.i_div(16, 2) : U32)
      +_t3 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(b)
      +fy : U32 = bannerpeak.u.snd(U32, U32, _t3)
      +spark : Bool = U32.is_lt(bannerpeak.layout.spark_index(tcx, tcy), 8)
      bannerpeak.bot.Session.climb.if7(Bool.and(spark, RS.i_lt((fy + 26 : U32), ty)), self, k, b, side, tx, ty, fy)

def bannerpeak.bot.Session.climb.if9?(bpc: Bool, +self: J.Val, +k: J.Val, +tcx: U32, +tcy: U32, +b: J.Val, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      +side : U32 = J.u32(J.get(b, "wall"))
      bannerpeak.bot.Session.climb.if8(held_jump, self, k, tcx, tcy, b, side)
    case False{}:
      bannerpeak.bot.keys(J.u32(J.get(b, "face")), Bool.and(held_jump, RS.i_lt(J.u32(J.get(b, "vy")), 0)), False{})

def bannerpeak.bot.Session.climb.if10?(bpc: Bool, +self: J.Val, +k: J.Val, +tcx: U32, +tcy: U32, +b: J.Val, +held_jump: Bool, +x: U32) -> J.Val:
  match bpc:
    case True{}:
      +lo : U32 = ((287 + 72 : U32) * 16 : U32)
      +hi : U32 = ((287 + 76 : U32) * 16 : U32)
      +_t2 = bannerpeak.bot.Session.climb.if1(Bool.and(RS.i_ge(RS.i_div_euclid(x, 16), 273), RS.i_lt(RS.i_div_euclid(x, 16), (273 + 14 : U32))), lo, hi)
      +hi = bannerpeak.u.fst(U32, U32, _t2)
      +lo = bannerpeak.u.snd(U32, U32, _t2)
      +dd : U32 = 1
      +dd = bannerpeak.bot.Session.climb.if3(RS.i_lt(x, (lo + 6 : U32)), x, hi, dd)
      bannerpeak.bot.keys(dd, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.bot.Session.climb.if9(String.eq(J.str(J.get(b, "mode")), "Wall"), self, k, tcx, tcy, b, held_jump)

def bannerpeak.bot.Session.kick_wait?(+self: J.Val, +k: J.Val, +wait: U32, +tx: U32, +ty: U32) -> U32:
  bannerpeak.bot.Session.kick_wait.if2(U32.is_gt(wait, 56), self, k, wait, tx, ty)

def bannerpeak.bot.Session.kick_wait.if1?(bpc: Bool, +self: J.Val, +k: J.Val, +wait: U32, +tx: U32, +ty: U32) -> U32:
  match bpc:
    case True{}:
      wait
    case False{}:
      bannerpeak.bot.Session.kick_wait(self, k, (wait + 1 : U32), tx, ty)

def bannerpeak.bot.Session.kick_wait.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +wait: U32, +tx: U32, +ty: U32) -> U32:
  match bpc:
    case True{}:
      wait
    case False{}:
      bannerpeak.bot.Session.kick_wait.if1(bannerpeak.bot.Session.foresee_kick(self, k, wait, tx, ty), self, k, wait, tx, ty)

def bannerpeak.bot.Session.foresee_kick?(+self: J.Val, +k: J.Val, +wait: U32, +tx: U32, +ty: U32) -> Bool:
  bannerpeak.bot.Session.kick_go(self, J.get(k, "body"), J.u32(J.get(J.get(k, "body"), "wall")), 0, wait, (wait + 36 : U32), tx, ty)

def bannerpeak.bot.Session.kick_go?(+self: J.Val, +b: J.Val, +side: U32, +n: U32, +wait: U32, +limit: U32, +tx: U32, +ty: U32) -> Bool:
  bannerpeak.bot.Session.kick_go.if4(U32.is_ge(n, limit), self, b, side, n, wait, limit, tx, ty)

def bannerpeak.bot.Session.kick_go.if1?(bpc: Bool, +side: U32, +x: U32) -> U32:
  match bpc:
    case True{}:
      +x : U32 = side
      x
    case False{}:
      x

def bannerpeak.bot.Session.kick_go.if2?(bpc: Bool, +self: J.Val, +side: U32, +n: U32, +wait: U32, +limit: U32, +tx: U32, +ty: U32, +bb: J.Val) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.bot.Session.kick_go(self, bb, side, (n + 1 : U32), wait, limit, tx, ty)

def bannerpeak.bot.Session.kick_go.if3?(bpc: Bool, +self: J.Val, +side: U32, +n: U32, +wait: U32, +limit: U32, +tx: U32, +ty: U32, +bb: J.Val) -> Bool:
  match bpc:
    case True{}:
      U32.is_ge(n, wait)
    case False{}:
      bannerpeak.bot.Session.kick_go.if2(Bool.and(U32.is_gt(n, wait), Bool.or(J.bool(J.get(bb, "ground")), String.eq(J.str(J.get(bb, "mode")), "Wall"))), self, side, n, wait, limit, tx, ty, bb)

def bannerpeak.bot.Session.kick_go.if4?(bpc: Bool, +self: J.Val, +b: J.Val, +side: U32, +n: U32, +wait: U32, +limit: U32, +tx: U32, +ty: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +kick : Bool = U32.is_eq(n, wait)
      +x : U32 = RS.i_neg(side)
      +x = bannerpeak.bot.Session.kick_go.if1(U32.is_lt(n, wait), side, x)
      +cmd : J.Val = J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32(x)), "jump", J.JBool{kick}), "jump_edge", J.JBool{kick}), "down", J.JBool{False{}}), "down_edge", J.JBool{False{}}), "dive_edge", J.JBool{False{}}), "brace", J.JBool{False{}})
      +out : J.Val = bannerpeak.u.vec_new()
      +bb : J.Val = b
      +_t1 = bannerpeak.body.Body.step(bb, cmd, bannerpeak.bot.shipped_knobs(), J.get(self, "changed"), bannerpeak.u.vec_new(), bannerpeak.u.vec_new(), out)
      +bb : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t1)
      +out : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t1)
      +_t2 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.body.Body.rect(bb)
      +l0 = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t2)
      +t0 = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t2))
      +r0 = bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t2)))
      +b0 = bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t2)))
      +l : U32 = RS.i_div(l0, 256)
      +t : U32 = RS.i_div(t0, 256)
      +r : U32 = RS.i_div(r0, 256)
      +bt : U32 = RS.i_div(b0, 256)
      bannerpeak.bot.Session.kick_go.if3(Bool.and(Bool.and(Bool.and(RS.i_lt(l, (tx + 8 : U32)), RS.i_gt(r, (tx - 8 : U32))), RS.i_lt(t, (ty + 8 : U32))), RS.i_gt(bt, (ty - 8 : U32))), self, side, n, wait, limit, tx, ty, bb)

def bannerpeak.bot.Session.fight?(+self: J.Val, +k: J.Val) -> J.Val:
  +bo : J.Val = J.get(self, "boss")
  +b : J.Val = J.get(k, "body")
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(b)
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +held_jump : Bool = U32.is_ne(U32.and(J.u32(J.get(k, "prev")), 1), 0)
  +held_down : Bool = U32.is_ne(U32.and(J.u32(J.get(k, "prev")), 2), 0)
  +floor : U32 = bannerpeak.boss.floor()
  +ground : Bool = Bool.and(J.bool(J.get(b, "ground")), Bool.or(String.eq(J.str(J.get(b, "mode")), "Ground"), String.eq(J.str(J.get(b, "mode")), "Belly")))
  +lo : U32 = ((392 + 5 : U32) * 16 : U32)
  +hi : U32 = ((392 + 36 : U32) * 16 : U32)
  +on_head : Bool = Bool.and(Bool.not(J.is_null(J.get(b, "plank"))), U32.is_eq(J.u32(J.get(b, "plank")), 300))
  bannerpeak.bot.Session.fight.if16(String.eq(J.str(J.get(bo, "beat")), "Dormant"), self, bo, b, fx, fy, held_jump, held_down, floor, ground, lo, hi, on_head)

def bannerpeak.bot.Session.fight.if1?(bpc: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(1, False{}, False{})
    case False{}:
      bannerpeak.bot.keys(0, False{}, False{})

def bannerpeak.bot.Session.fight.if2?(bpc: Bool, +dd: U32) -> U32:
  match bpc:
    case True{}:
      +dd : U32 = RS.i_neg(dd)
      dd
    case False{}:
      dd

def bannerpeak.bot.Session.fight.if3?(bpc: Bool, +b: J.Val, +held_jump: Bool, +dx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(RS.i_signum(dx), False{}, False{})
    case False{}:
      bannerpeak.bot.keys(RS.i_signum(dx), Bool.and(held_jump, RS.i_lt(J.u32(J.get(b, "vy")), 0)), False{})

def bannerpeak.bot.Session.fight.if4?(bpc: Bool, +b: J.Val, +fy: U32, +held_jump: Bool, +held_down: Bool, +floor: U32, +dx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, False{}, Bool.not(held_down))
    case False{}:
      bannerpeak.bot.Session.fight.if3(Bool.and(Bool.and(Bool.and(String.eq(J.str(J.get(b, "mode")), "Air"), RS.i_gt(J.u32(J.get(b, "vy")), 0)), RS.i_lt(fy, (floor - 4 : U32))), RS.i_gt(RS.i_abs(dx), 20)), b, held_jump, dx)

def bannerpeak.bot.Session.fight.if5?(bpc: Bool, +held_jump: Bool, +dx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(RS.i_signum(dx), False{}, False{})
    case False{}:
      bannerpeak.bot.keys(RS.i_signum(dx), Bool.not(held_jump), False{})

def bannerpeak.bot.Session.fight.if6?(bpc: Bool, +held_jump: Bool, +dx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.bot.Session.fight.if5(RS.i_gt(RS.i_abs(dx), 60), held_jump, dx)

def bannerpeak.bot.Session.fight.if7?(bpc: Bool, +b: J.Val, +fy: U32, +held_jump: Bool, +held_down: Bool, +floor: U32, +on_head: Bool, +dx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.fight.if4(Bool.and(Bool.and(String.eq(J.str(J.get(b, "mode")), "Air"), RS.i_gt(J.u32(J.get(b, "vy")), 0)), RS.i_le(RS.i_abs(dx), 20)), b, fy, held_jump, held_down, floor, dx)
    case False{}:
      bannerpeak.bot.Session.fight.if6(on_head, held_jump, dx)

def bannerpeak.bot.Session.fight.if8?(bpc: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(4294967295, False{}, False{})
    case False{}:
      bannerpeak.bot.keys(1, False{}, False{})

def bannerpeak.bot.Session.fight.if9?(bpc: Bool, +bo: J.Val, +want: U32) -> U32:
  match bpc:
    case True{}:
      +want : U32 = (J.u32(J.get(bo, "x")) + 120 : U32)
      want
    case False{}:
      want

def bannerpeak.bot.Session.fight.if10?(bpc: Bool, +dd: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, False{}, False{})
    case False{}:
      bannerpeak.bot.keys(dd, False{}, False{})

def bannerpeak.bot.Session.fight.if11?(bpc: Bool, +bo: J.Val, +fx: U32, +lo: U32, +hi: U32, +dx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.fight.if8(RS.i_ge(dx, 0))
    case False{}:
      +want : U32 = (J.u32(J.get(bo, "x")) - 120 : U32)
      +want = bannerpeak.bot.Session.fight.if9(RS.i_ge(fx, J.u32(J.get(bo, "x"))), bo, want)
      +want : U32 = RS.i_clamp(want, lo, hi)
      +dd : U32 = RS.i_signum((want - fx : U32))
      bannerpeak.bot.Session.fight.if10(RS.i_lt(RS.i_abs((want - fx : U32)), 6), dd)

def bannerpeak.bot.Session.fight.if12?(bpc: Bool, +bo: J.Val, +fx: U32, +lo: U32, +hi: U32, +on_head: Bool, +dx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, False{}, False{})
    case False{}:
      bannerpeak.bot.Session.fight.if11(on_head, bo, fx, lo, hi, dx)

def bannerpeak.bot.Session.fight.if13?(bpc: Bool, +bo: J.Val, +b: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +held_down: Bool, +floor: U32, +ground: Bool, +lo: U32, +hi: U32, +on_head: Bool, +dx: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.fight.if7(Bool.and(Bool.not(ground), Bool.not(on_head)), b, fy, held_jump, held_down, floor, on_head, dx)
    case False{}:
      bannerpeak.bot.Session.fight.if12(Bool.or(String.eq(J.str(J.get(bo, "beat")), "Rage"), String.eq(J.str(J.get(bo, "beat")), "Down")), bo, fx, lo, hi, on_head, dx)

def bannerpeak.bot.Session.fight.if14?(bpc: Bool, +bo: J.Val, +b: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +held_down: Bool, +floor: U32, +ground: Bool, +lo: U32, +hi: U32, +on_head: Bool, +away: U32) -> J.Val:
  match bpc:
    case True{}:
      +dd : U32 = away
      +dd = bannerpeak.bot.Session.fight.if2(Bool.or(Bool.and(RS.i_gt(dd, 0), RS.i_gt(fx, (hi - 10 : U32))), Bool.and(RS.i_lt(dd, 0), RS.i_lt(fx, (lo + 10 : U32)))), dd)
      bannerpeak.bot.keys(dd, False{}, False{})
    case False{}:
      +dx : U32 = (J.u32(J.get(bo, "x")) - fx : U32)
      bannerpeak.bot.Session.fight.if13(Bool.and(String.eq(J.str(J.get(bo, "beat")), "Wince"), U32.is_gt(J.u32(J.get(bo, "plates")), 0)), bo, b, fx, fy, held_jump, held_down, floor, ground, lo, hi, on_head, dx)

def bannerpeak.bot.Session.fight.if15?(bpc: Bool, +bo: J.Val, +b: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +held_down: Bool, +floor: U32, +ground: Bool, +lo: U32, +hi: U32, +on_head: Bool, +flee: Bool, +away: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(0, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.bot.Session.fight.if14(flee, bo, b, fx, fy, held_jump, held_down, floor, ground, lo, hi, on_head, away)

def bannerpeak.bot.Session.fight.if16?(bpc: Bool, +self: J.Val, +bo: J.Val, +b: J.Val, +fx: U32, +fy: U32, +held_jump: Bool, +held_down: Bool, +floor: U32, +ground: Bool, +lo: U32, +hi: U32, +on_head: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.fight.if1(RS.i_lt(fx, (bannerpeak.boss.home() - 120 : U32)))
    case False{}:
      +_t2 : Sigma<&2, &2, Bool, _ => Sigma<&2, &2, Bool, _ => U32>> = bannerpeak.bot.Session.flee(self, J.get(bo, "hazards"), 0, fx, fy, ground, floor, False{}, 0)
      +hop = bannerpeak.u.fst(Bool, Sigma<&2, &2, Bool, _ => U32>, _t2)
      +flee = bannerpeak.u.fst(Bool, U32, bannerpeak.u.snd(Bool, Sigma<&2, &2, Bool, _ => U32>, _t2))
      +away = bannerpeak.u.snd(Bool, U32, bannerpeak.u.snd(Bool, Sigma<&2, &2, Bool, _ => U32>, _t2))
      bannerpeak.bot.Session.fight.if15(hop, bo, b, fx, fy, held_jump, held_down, floor, ground, lo, hi, on_head, flee, away)

def bannerpeak.bot.Session.flee?(+self: J.Val, +hs: J.Val, +n: U32, +fx: U32, +fy: U32, +ground: Bool, +floor: U32, +flee: Bool, +away: U32) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, Bool, _ => U32>>:
  bannerpeak.bot.Session.flee.if6(U32.is_ge(n, J.len(hs)), self, hs, n, fx, fy, ground, floor, flee, away)

def bannerpeak.bot.Session.flee.if1?(bpc: Bool, +self: J.Val, +hs: J.Val, +n: U32, +fx: U32, +fy: U32, +ground: Bool, +floor: U32, +flee: Bool, +away: U32) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, Bool, _ => U32>>:
  match bpc:
    case True{}:
      (True{}, (flee, away))
    case False{}:
      bannerpeak.bot.Session.flee(self, hs, (n + 1 : U32), fx, fy, ground, floor, flee, away)

def bannerpeak.bot.Session.flee.if2?(bpc: Bool, +side: U32) -> U32:
  match bpc:
    case True{}:
      +side : U32 = 1
      side
    case False{}:
      side

def bannerpeak.bot.Session.flee.if3?(bpc: Bool, +self: J.Val, +hs: J.Val, +n: U32, +fx: U32, +fy: U32, +ground: Bool, +floor: U32, +flee: Bool, +away: U32, +r: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, Bool, _ => U32>>:
  match bpc:
    case True{}:
      +_t5 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = r
      +_t6 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = r
      +mid : U32 = RS.i_div((bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t5) + bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t6))) : U32), 2)
      +side : U32 = 4294967295
      +side = bannerpeak.bot.Session.flee.if2(RS.i_ge(fx, mid), side)
      bannerpeak.bot.Session.flee(self, hs, (n + 1 : U32), fx, fy, ground, floor, True{}, side)
    case False{}:
      bannerpeak.bot.Session.flee(self, hs, (n + 1 : U32), fx, fy, ground, floor, flee, away)

def bannerpeak.bot.Session.flee.if4?(bpc: Bool, +self: J.Val, +hs: J.Val, +n: U32, +fx: U32, +fy: U32, +ground: Bool, +floor: U32, +flee: Bool, +away: U32, +h: J.Val, +r: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, Bool, _ => U32>>:
  match bpc:
    case True{}:
      +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = r
      +_t2 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = r
      +c : U32 = RS.i_div((bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1) + bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t2))) : U32), 2)
      +toward : Bool = RS.i_gt(((fx - c : U32) * J.u32(J.get(h, "vx")) : U32), 0)
      bannerpeak.bot.Session.flee.if1(Bool.and(Bool.and(Bool.and(Bool.and(toward, RS.i_lt(RS.i_abs((fx - c : U32)), 40)), ground), RS.i_lt(RS.i_abs((fy - floor : U32)), 3)), U32.is_eq(J.u32(J.get(h, "tell")), 0)), self, hs, n, fx, fy, ground, floor, flee, away)
    case False{}:
      +_t3 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = r
      +_t4 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = r
      bannerpeak.bot.Session.flee.if3(Bool.and(RS.i_gt(fx, (bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t3) - 18 : U32)), RS.i_lt(fx, (bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t4))) + 18 : U32))), self, hs, n, fx, fy, ground, floor, flee, away, r)

def bannerpeak.bot.Session.flee.if5?(bpc: Bool, +self: J.Val, +hs: J.Val, +n: U32, +fx: U32, +fy: U32, +ground: Bool, +floor: U32, +flee: Bool, +away: U32, +h: J.Val) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, Bool, _ => U32>>:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.flee(self, hs, (n + 1 : U32), fx, fy, ground, floor, flee, away)
    case False{}:
      +r : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.boss.Hazard.rect(h)
      bannerpeak.bot.Session.flee.if4(U32.is_ne(J.u32(J.get(h, "vx")), 0), self, hs, n, fx, fy, ground, floor, flee, away, h, r)

def bannerpeak.bot.Session.flee.if6?(bpc: Bool, +self: J.Val, +hs: J.Val, +n: U32, +fx: U32, +fy: U32, +ground: Bool, +floor: U32, +flee: Bool, +away: U32) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, Bool, _ => U32>>:
  match bpc:
    case True{}:
      (False{}, (flee, away))
    case False{}:
      +h : J.Val = J.nth(hs, n)
      bannerpeak.bot.Session.flee.if5(Bool.and(U32.is_eq(J.u32(J.get(h, "live")), 0), U32.is_eq(J.u32(J.get(h, "tell")), 0)), self, hs, n, fx, fy, ground, floor, flee, away, h)

def bannerpeak.bot.Session.hint_here?(+self: J.Val, +b: J.Val) -> U32:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(b)
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +cx : U32 = RS.i_div_euclid(fx, 16)
  +cy : U32 = RS.i_div_euclid((fy - 1 : U32), 16)
  bannerpeak.bot.Session.hint_here.if2(Bool.and(Bool.and(Bool.and(RS.i_ge(cx, (0 + 13 : U32)), RS.i_lt(cx, (0 + 34 : U32))), RS.i_ge(cy, (32 + 20 : U32))), U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 2), 0)), cx, cy)

def bannerpeak.bot.Session.hint_here.if1?(bpc: Bool, +cx: U32, +cy: U32, +h: U32) -> U32:
  match bpc:
    case True{}:
      h
    case False{}:
      bannerpeak.layout.hint_at(cx, (cy - 1 : U32))

def bannerpeak.bot.Session.hint_here.if2?(bpc: Bool, +cx: U32, +cy: U32) -> U32:
  match bpc:
    case True{}:
      0
    case False{}:
      +h : U32 = bannerpeak.layout.hint_at(cx, cy)
      bannerpeak.bot.Session.hint_here.if1(U32.is_ne(h, 0), cx, cy, h)

def bannerpeak.bot.Session.walk_with_hops?(+self: J.Val, +k: J.Val, +d: U32) -> J.Val:
  +held_jump : Bool = U32.is_ne(U32.and(J.u32(J.get(k, "prev")), 1), 0)
  bannerpeak.bot.Session.walk_with_hops.if2(U32.is_eq(d, 0), self, k, d, held_jump)

def bannerpeak.bot.Session.walk_with_hops.if1?(bpc: Bool, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.keys(d, Bool.not(held_jump), False{})
    case False{}:
      bannerpeak.bot.keys(d, False{}, False{})

def bannerpeak.bot.Session.walk_with_hops.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +d: U32, +held_jump: Bool) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.u.input_default()
    case False{}:
      bannerpeak.bot.Session.walk_with_hops.if1(bannerpeak.bot.Session.obstacle_ahead(self, k, d, True{}), d, held_jump)

def bannerpeak.bot.Session.solid_or_ledge?(+self: J.Val, +x: U32, +fy: U32) -> Bool:
  +t : U32 = bannerpeak.session.View.tile(J.get(self, "changed"), RS.i_div_euclid(x, 16), RS.i_div_euclid(fy, 16))
  Bool.or(bannerpeak.world.Tile.solid(t), bannerpeak.world.Tile.one_way(t))

def bannerpeak.bot.Session.drop_floor?(+self: J.Val, +foot_ahead: U32, +fy: U32, +r: U32) -> Bool:
  bannerpeak.bot.Session.drop_floor.if2(RS.i_gt(r, 3), self, foot_ahead, fy, r)

def bannerpeak.bot.Session.drop_floor.if1?(bpc: Bool, +self: J.Val, +foot_ahead: U32, +fy: U32, +r: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.bot.Session.drop_floor(self, foot_ahead, fy, (r + 1 : U32))

def bannerpeak.bot.Session.drop_floor.if2?(bpc: Bool, +self: J.Val, +foot_ahead: U32, +fy: U32, +r: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.bot.Session.drop_floor.if1(bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(J.get(self, "changed"), RS.i_div_euclid(foot_ahead, 16), (RS.i_div_euclid(fy, 16) + r : U32))), self, foot_ahead, fy, r)

def bannerpeak.bot.Session.foe_ahead?(+self: J.Val, +n: U32, +fx: U32, +fy: U32, +d: U32) -> Bool:
  bannerpeak.bot.Session.foe_ahead.if3(U32.is_ge(n, J.len(J.get(self, "foes"))), self, n, fx, fy, d)

def bannerpeak.bot.Session.foe_ahead.if1?(bpc: Bool, +self: J.Val, +n: U32, +fx: U32, +fy: U32, +d: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.bot.Session.foe_ahead(self, (n + 1 : U32), fx, fy, d)

def bannerpeak.bot.Session.foe_ahead.if2?(bpc: Bool, +self: J.Val, +n: U32, +fx: U32, +fy: U32, +d: U32, +f: J.Val) -> Bool:
  match bpc:
    case True{}:
      +fxp : U32 = RS.i_div_euclid(J.u32(J.get(f, "x")), 256)
      +fyp : U32 = RS.i_div_euclid(J.u32(J.get(f, "y")), 256)
      +ahead : U32 = ((fxp - fx : U32) * d : U32)
      bannerpeak.bot.Session.foe_ahead.if1(Bool.and(Bool.and(RS.i_ge(ahead, 4), RS.i_lt(ahead, 42)), RS.i_lt(RS.i_abs((fyp - fy : U32)), 22)), self, n, fx, fy, d)
    case False{}:
      bannerpeak.bot.Session.foe_ahead(self, (n + 1 : U32), fx, fy, d)

def bannerpeak.bot.Session.foe_ahead.if3?(bpc: Bool, +self: J.Val, +n: U32, +fx: U32, +fy: U32, +d: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +f : J.Val = J.nth(J.get(self, "foes"), n)
      bannerpeak.bot.Session.foe_ahead.if2(bannerpeak.foes.Foe.alive(f), self, n, fx, fy, d, f)

def bannerpeak.bot.Session.boulder_ahead?(+self: J.Val, +n: U32, +fx: U32, +fy: U32, +d: U32) -> Bool:
  bannerpeak.bot.Session.boulder_ahead.if2(U32.is_ge(n, J.len(J.get(self, "boulders"))), self, n, fx, fy, d)

def bannerpeak.bot.Session.boulder_ahead.if1?(bpc: Bool, +self: J.Val, +n: U32, +fx: U32, +fy: U32, +d: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.bot.Session.boulder_ahead(self, (n + 1 : U32), fx, fy, d)

def bannerpeak.bot.Session.boulder_ahead.if2?(bpc: Bool, +self: J.Val, +n: U32, +fx: U32, +fy: U32, +d: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +bx : U32 = RS.i_div_euclid(J.u32(J.get(J.nth(J.get(self, "boulders"), n), "x")), 256)
      +ahead : U32 = ((bx - fx : U32) * d : U32)
      bannerpeak.bot.Session.boulder_ahead.if1(Bool.and(Bool.and(RS.i_ge(ahead, 0), RS.i_lt(ahead, 78)), RS.i_lt(RS.i_abs(((22 * 16 : U32) - fy : U32)), 8)), self, n, fx, fy, d)

def bannerpeak.bot.Session.obstacle_ahead?(+self: J.Val, +k: J.Val, +d: U32, +gaps: Bool) -> Bool:
  +b : J.Val = J.get(k, "body")
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(b)
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +g : J.Val = J.get(self, "changed")
  +_t2 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.dims(b)
  +half : U32 = RS.i_div(bannerpeak.u.fst(U32, U32, _t2), 2)
  +lead : U32 = (fx + (d * (half + 3 : U32) : U32) : U32)
  bannerpeak.bot.Session.obstacle_ahead.if6(Bool.or(Bool.or(bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, RS.i_div_euclid(lead, 16), RS.i_div_euclid((fy - 4 : U32), 16))), bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, RS.i_div_euclid(lead, 16), RS.i_div_euclid((fy - 12 : U32), 16)))), bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, RS.i_div_euclid(lead, 16), RS.i_div_euclid((fy - 22 : U32), 16)))), self, d, gaps, b, fx, fy, g, half)

def bannerpeak.bot.Session.obstacle_ahead.if1?(bpc: Bool, +self: J.Val, +d: U32, +fx: U32, +fy: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.bot.Session.boulder_ahead(self, 0, fx, fy, d)

def bannerpeak.bot.Session.obstacle_ahead.if2?(bpc: Bool, +self: J.Val, +d: U32, +fx: U32, +fy: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.bot.Session.obstacle_ahead.if1(bannerpeak.bot.Session.foe_ahead(self, 0, fx, fy, d), self, d, fx, fy)

def bannerpeak.bot.Session.obstacle_ahead.if3?(bpc: Bool, +self: J.Val, +d: U32, +fx: U32, +fy: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.bot.Session.boulder_ahead(self, 0, fx, fy, d)

def bannerpeak.bot.Session.obstacle_ahead.if4?(bpc: Bool, +self: J.Val, +d: U32, +fx: U32, +fy: U32, +foot_ahead: U32) -> Bool:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.obstacle_ahead.if2(Bool.not(bannerpeak.bot.Session.drop_floor(self, foot_ahead, fy, 1)), self, d, fx, fy)
    case False{}:
      bannerpeak.bot.Session.obstacle_ahead.if3(bannerpeak.bot.Session.foe_ahead(self, 0, fx, fy, d), self, d, fx, fy)

def bannerpeak.bot.Session.obstacle_ahead.if5?(bpc: Bool, +self: J.Val, +d: U32, +gaps: Bool, +b: J.Val, +fx: U32, +fy: U32, +half: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      +foot_ahead : U32 = (fx + (d * (half + 2 : U32) : U32) : U32)
      bannerpeak.bot.Session.obstacle_ahead.if4(Bool.and(Bool.and(Bool.and(Bool.and(gaps, J.bool(J.get(b, "ground"))), J.is_null(J.get(b, "plank"))), Bool.not(bannerpeak.bot.Session.solid_or_ledge(self, foot_ahead, fy))), Bool.not(bannerpeak.bot.Session.solid_or_ledge(self, (fx + (d * (half + 8 : U32) : U32) : U32), fy))), self, d, fx, fy, foot_ahead)

def bannerpeak.bot.Session.obstacle_ahead.if6?(bpc: Bool, +self: J.Val, +d: U32, +gaps: Bool, +b: J.Val, +fx: U32, +fy: U32, +g: J.Val, +half: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      +sy : U32 = RS.i_div_euclid((fy - 1 : U32), 16)
      bannerpeak.bot.Session.obstacle_ahead.if5(Bool.or(Bool.or(U32.is_eq(bannerpeak.session.View.tile(g, RS.i_div_euclid((fx + (d * 8 : U32) : U32), 16), sy), 10), U32.is_eq(bannerpeak.session.View.tile(g, RS.i_div_euclid((fx + (d * 16 : U32) : U32), 16), sy), 10)), U32.is_eq(bannerpeak.session.View.tile(g, RS.i_div_euclid((fx + (d * 24 : U32) : U32), 16), sy), 10)), self, d, gaps, b, fx, fy, half)

def bannerpeak.bot.Session.bot_input_c?(+self: J.Val, +id: U32) -> J.Val:
  bannerpeak.bot.Session.bot_input(self, id, J.set(J.set(J.set(J.obj(), "away", J.JBool{False{}}), "stone", J.JBool{False{}}), "carried", J.obj()))

def bannerpeak.bot.Session.goal_c?(+self: J.Val, +id: U32) -> J.Val:
  bannerpeak.bot.Session.goal(self, bannerpeak.u.at(J.get(self, "seats"), id))

def bannerpeak.bot.Session.drive_c?(+self: J.Val, +id: U32, +goal: J.Val) -> J.Val:
  bannerpeak.bot.Session.drive(self, bannerpeak.u.at(J.get(self, "seats"), id), goal)

def bannerpeak.bot.Session.mill_goal_c?(+self: J.Val, +id: U32) -> J.Val:
  bannerpeak.bot.Session.mill_goal(self, bannerpeak.u.at(J.get(self, "seats"), id))

def bannerpeak.bot.Session.shaft_goal_c?(+self: J.Val, +id: U32) -> J.Val:
  bannerpeak.bot.Session.shaft_goal(self, bannerpeak.u.at(J.get(self, "seats"), id))

def bannerpeak.bot.Session.snow_goal_c?(+self: J.Val, +id: U32) -> J.Val:
  bannerpeak.bot.Session.snow_goal(self, bannerpeak.u.at(J.get(self, "seats"), id))

def bannerpeak.bot.Session.summit_goal_c?(+self: J.Val, +id: U32) -> J.Val:
  bannerpeak.bot.Session.summit_goal(self, bannerpeak.u.at(J.get(self, "seats"), id))

def bannerpeak.bot.Session.ride_c?(+self: J.Val, +id: U32) -> J.Val:
  bannerpeak.bot.Session.ride(self, bannerpeak.u.at(J.get(self, "seats"), id))

def bannerpeak.bot.Session.fight_c?(+self: J.Val, +id: U32) -> J.Val:
  bannerpeak.bot.Session.fight(self, bannerpeak.u.at(J.get(self, "seats"), id))

def bannerpeak.bot.Session.climb_c?(+self: J.Val, +id: U32, +x: U32, +y: U32) -> J.Val:
  bannerpeak.bot.Session.climb(self, bannerpeak.u.at(J.get(self, "seats"), id), x, y)

def bannerpeak.bot.Session.foresee_kick_c?(+self: J.Val, +id: U32, +wait: U32, +tx: U32, +ty: U32) -> Bool:
  bannerpeak.bot.Session.foresee_kick(self, bannerpeak.u.at(J.get(self, "seats"), id), wait, tx, ty)

def bannerpeak.bot.Session.walk_with_hops_c?(+self: J.Val, +id: U32, +d: U32) -> J.Val:
  bannerpeak.bot.Session.walk_with_hops(self, bannerpeak.u.at(J.get(self, "seats"), id), d)

def bannerpeak.bot.Session.obstacle_ahead_c?(+self: J.Val, +id: U32, +d: U32, +gaps: Bool) -> Bool:
  bannerpeak.bot.Session.obstacle_ahead(self, bannerpeak.u.at(J.get(self, "seats"), id), d, gaps)

def bannerpeak.bot.Session.hint_here_c?(+self: J.Val, +id: U32) -> U32:
  bannerpeak.bot.Session.hint_here(self, J.get(bannerpeak.u.at(J.get(self, "seats"), id), "body"))

def bannerpeak.bot.Session.spark_missing_c?(+self: J.Val, +x: U32, +y: U32) -> Bool:
  bannerpeak.bot.Session.spark_missing(self, x, y)

def case.bannerpeak.bot.Session.bot_input?(+input: J.Val) -> J.Val:
  bannerpeak.bot.Session.bot_input_c(J.get(input, "session"), J.u32(J.get(input, "id")))

def case.bannerpeak.bot.Session.goal?(+input: J.Val) -> J.Val:
  bannerpeak.bot.Session.goal_c(J.get(input, "session"), J.u32(J.get(input, "id")))

def case.bannerpeak.bot.Session.drive?(+input: J.Val) -> J.Val:
  bannerpeak.bot.Session.drive_c(J.get(input, "session"), J.u32(J.get(input, "id")), J.get(input, "goal"))

def case.bannerpeak.bot.Session.mill_goal?(+input: J.Val) -> J.Val:
  bannerpeak.bot.Session.mill_goal_c(J.get(input, "session"), J.u32(J.get(input, "id")))

def case.bannerpeak.bot.Session.shaft_goal?(+input: J.Val) -> J.Val:
  bannerpeak.bot.Session.shaft_goal_c(J.get(input, "session"), J.u32(J.get(input, "id")))

def case.bannerpeak.bot.Session.snow_goal?(+input: J.Val) -> J.Val:
  bannerpeak.bot.Session.snow_goal_c(J.get(input, "session"), J.u32(J.get(input, "id")))

def case.bannerpeak.bot.Session.summit_goal?(+input: J.Val) -> J.Val:
  bannerpeak.bot.Session.summit_goal_c(J.get(input, "session"), J.u32(J.get(input, "id")))

def case.bannerpeak.bot.Session.ride?(+input: J.Val) -> J.Val:
  bannerpeak.bot.Session.ride_c(J.get(input, "session"), J.u32(J.get(input, "id")))

def case.bannerpeak.bot.Session.fight?(+input: J.Val) -> J.Val:
  bannerpeak.bot.Session.fight_c(J.get(input, "session"), J.u32(J.get(input, "id")))

def case.bannerpeak.bot.Session.climb?(+input: J.Val) -> J.Val:
  bannerpeak.bot.Session.climb_c(J.get(input, "session"), J.u32(J.get(input, "id")), J.u32(J.get(input, "x")), J.u32(J.get(input, "y")))

def case.bannerpeak.bot.Session.foresee_kick?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.bot.Session.foresee_kick_c(J.get(input, "session"), J.u32(J.get(input, "id")), J.u32(J.get(input, "wait")), J.u32(J.get(input, "tx")), J.u32(J.get(input, "ty")))}

def case.bannerpeak.bot.Session.walk_with_hops?(+input: J.Val) -> J.Val:
  bannerpeak.bot.Session.walk_with_hops_c(J.get(input, "session"), J.u32(J.get(input, "id")), J.u32(J.get(input, "d")))

def case.bannerpeak.bot.Session.obstacle_ahead?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.bot.Session.obstacle_ahead_c(J.get(input, "session"), J.u32(J.get(input, "id")), J.u32(J.get(input, "d")), J.bool(J.get(input, "gaps")))}

def case.bannerpeak.bot.Session.hint_here?(+input: J.Val) -> J.Val:
  J.JU{bannerpeak.bot.Session.hint_here_c(J.get(input, "session"), J.u32(J.get(input, "id")))}

def case.bannerpeak.bot.Session.spark_missing?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.bot.Session.spark_missing_c(J.get(input, "session"), J.u32(J.get(input, "x")), J.u32(J.get(input, "y")))}

def case.bannerpeak.layout.world?(+input: J.Val) -> J.Val:
  J.JA{bannerpeak.cases.world.rows(0)}

def bannerpeak.cases.world.rows?(+y: U32) -> List<&2, J.Val>:
  bannerpeak.u.ife(List<&2, J.Val>, U32.is_lt(y, 64), _ => Con{J.JA{bannerpeak.cases.world.cols(y, 0)}, bannerpeak.cases.world.rows((y + 1 : U32))}, _ => Nil{})

def bannerpeak.cases.world.cols?(+y: U32, +x: U32) -> List<&2, J.Val>:
  bannerpeak.u.ife(List<&2, J.Val>, U32.is_lt(x, 434), _ => Con{J.JU{bannerpeak.world.World.at(x, y)}, bannerpeak.cases.world.cols(y, (x + 1 : U32))}, _ => Nil{})

# Flat (x, y) pairs as an array of [x, y].

def bannerpeak.cases.pairs?(+xs: List<&2, U32>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+x, +r}:
      bannerpeak.cases.pairs.y(x, r)

def bannerpeak.cases.pairs.y?(+x: U32, +r: List<&2, U32>) -> List<&2, J.Val>:
  match r:
    case Nil{}:
      Nil{}
    case Con{+y, +t}:
      Con{J.JA{[J.JU{x}, J.JU{y}]}, bannerpeak.cases.pairs(t)}

def bannerpeak.cases.triples?(+xs: List<&2, U32>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+a, +r}:
      bannerpeak.cases.triples.b(a, r)

def bannerpeak.cases.triples.b?(+a: U32, +r: List<&2, U32>) -> List<&2, J.Val>:
  match r:
    case Nil{}:
      Nil{}
    case Con{+b, +s}:
      bannerpeak.cases.triples.c(a, b, s)

def bannerpeak.cases.triples.c?(+a: U32, +b: U32, +s: List<&2, U32>) -> List<&2, J.Val>:
  match s:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      Con{J.JA{[J.JU{a}, J.JU{b}, J.JU{c}]}, bannerpeak.cases.triples(t)}

# banners(): (x, y, has) triples as null or [x, y].

def bannerpeak.cases.banners?(+xs: List<&2, U32>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+a, +r}:
      bannerpeak.cases.banners.b(a, r)

def bannerpeak.cases.banners.b?(+a: U32, +r: List<&2, U32>) -> List<&2, J.Val>:
  match r:
    case Nil{}:
      Nil{}
    case Con{+b, +s}:
      bannerpeak.cases.banners.c(a, b, s)

def bannerpeak.cases.banners.c?(+a: U32, +b: U32, +s: List<&2, U32>) -> List<&2, J.Val>:
  match s:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      Con{bannerpeak.u.ife(J.Val, U32.is_eq(c, 1), _ => J.JA{[J.JU{a}, J.JU{b}]}, _ => J.JNull{}), bannerpeak.cases.banners(t)}

def case.bannerpeak.layout.info?(+input: J.Val) -> J.Val:
  +o0 = J.set(J.obj(), "start", J.JA{[J.JU{bannerpeak.layout.info.start_x()}, J.JU{bannerpeak.layout.info.start_y()}]})
  +o1 = J.set(o0, "gates", J.JA{bannerpeak.cases.pairs(bannerpeak.layout.info.gates())})
  +o2 = J.set(o1, "sparks", J.JA{bannerpeak.cases.pairs(bannerpeak.layout.info.sparks())})
  +o3 = J.set(o2, "coins", J.JA{bannerpeak.cases.pairs(bannerpeak.layout.info.coins())})
  +o4 = J.set(o3, "banners", J.JA{bannerpeak.cases.banners(bannerpeak.layout.info.banners())})
  +o5 = J.set(o4, "pole", J.JA{[J.JU{bannerpeak.layout.info.pole_x()}, J.JU{bannerpeak.layout.info.pole_y()}]})
  +o6 = J.set(o5, "great_pole", J.JA{[J.JU{bannerpeak.layout.info.great_pole_x()}, J.JU{bannerpeak.layout.info.great_pole_y()}]})
  +o7 = J.set(o6, "finale", J.JA{[J.JU{bannerpeak.layout.info.finale_x()}, J.JU{bannerpeak.layout.info.finale_y()}]})
  +o8 = J.set(o7, "arch", J.JA{[J.JU{bannerpeak.layout.info.arch_x()}, J.JU{bannerpeak.layout.info.arch_y()}]})
  +o9 = J.set(o8, "board", J.JA{[J.JU{bannerpeak.layout.info.board_x()}, J.JU{bannerpeak.layout.info.board_y()}]})
  +oa = J.set(o9, "cinder", J.JA{[J.JU{bannerpeak.layout.info.cinder_x()}, J.JU{bannerpeak.layout.info.cinder_y()}]})
  +ob = J.set(oa, "foes", J.JA{bannerpeak.cases.triples(bannerpeak.layout.info.foes())})
  J.set(ob, "hints", J.JA{bannerpeak.cases.triples(bannerpeak.layout.info.hints())})

def bannerpeak.foes.POP?() -> U32:
  12

def bannerpeak.foes.WALK?() -> U32:
  170

def bannerpeak.foes.CAPPED_WALK?() -> U32:
  130

def bannerpeak.foes.WISP_REACH?() -> U32:
  44

def bannerpeak.foes.WISP_PERIOD?() -> U32:
  150

def bannerpeak.foes.CARRY?() -> U32:
  150

def bannerpeak.foes.RISE?() -> U32:
  16

def bannerpeak.foes.BOLT_LIFE?() -> U32:
  70

def bannerpeak.foes.BOLT_SPEED?() -> U32:
  1900

def bannerpeak.foes.BOLT_MOST?() -> U32:
  2

def bannerpeak.foes.FoeKind.size?(+kind: String) -> Sigma<&2, &2, U32, _ => U32>:
  bannerpeak.foes.FoeKind.size.if2(String.eq(kind, "Snuffer"), kind)

def bannerpeak.foes.FoeKind.size.if1?(bpc: Bool) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      (14, 18)
    case False{}:
      (12, 12)

def bannerpeak.foes.FoeKind.size.if2?(bpc: Bool, +kind: String) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      (14, 14)
    case False{}:
      bannerpeak.foes.FoeKind.size.if1(String.eq(kind, "Capped"))

def bannerpeak.foes.Foe.new?(+kind: String, +cx: U32, +cy: U32, +mark: U32) -> J.Val:
  +x : U32 = ((cx * 16 : U32) + RS.i_div(16, 2) : U32)
  +y : U32 = ((cy + 1 : U32) * 16 : U32)
  +y = bannerpeak.foes.Foe.new.if1(String.eq(kind, "Wisp"), cy, y)
  +dir : U32 = 1
  +dir = bannerpeak.foes.Foe.new.if2(U32.is_eq(U32.mod(mark, 2), 0), dir)
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "kind", J.JS{kind}), "x", J.of_i32((x * 256 : U32))), "y", J.of_i32((y * 256 : U32))), "vx", J.of_i32(0)), "vy", J.of_i32(0)), "dir", J.of_i32(dir)), "t", J.JU{0}), "dead", J.JU{0}), "home", J.of_i32(x)), "mark", J.JU{mark})

def bannerpeak.foes.Foe.new.if1?(bpc: Bool, +cy: U32, +y: U32) -> U32:
  match bpc:
    case True{}:
      +y : U32 = (((cy * 16 : U32) + RS.i_div(16, 2) : U32) + 6 : U32)
      y
    case False{}:
      y

def bannerpeak.foes.Foe.new.if2?(bpc: Bool, +dir: U32) -> U32:
  match bpc:
    case True{}:
      +dir : U32 = 4294967295
      dir
    case False{}:
      dir

def bannerpeak.foes.Foe.alive?(+self: J.Val) -> Bool:
  U32.is_eq(J.u32(J.get(self, "dead")), 0)

def bannerpeak.foes.Foe.rect?(+self: J.Val) -> Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.foes.FoeKind.size(J.str(J.get(self, "kind")))
  +w = bannerpeak.u.fst(U32, U32, _t1)
  +h = bannerpeak.u.snd(U32, U32, _t1)
  +x : U32 = RS.i_div_euclid(J.u32(J.get(self, "x")), 256)
  +y : U32 = RS.i_div_euclid(J.u32(J.get(self, "y")), 256)
  ((x - RS.i_div(w, 2) : U32), ((y - h : U32), ((x + RS.i_div(w, 2) : U32), y)))

def bannerpeak.foes.Foe.defeat?(+self: J.Val) -> J.Val:
  +self = J.set(self, "dead", J.JU{12})
  +self = J.set(self, "vx", J.of_i32(0))
  self

def bannerpeak.foes.Foe.valid?(+self: J.Val, +width_px: U32, +height_px: U32) -> Bool:
  Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(RS.i_ge(J.u32(J.get(self, "x")), 0), RS.i_le(J.u32(J.get(self, "x")), (width_px * 256 : U32))), Bool.and(RS.i_ge(J.u32(J.get(self, "y")), (4294967292 * 4096 : U32)), RS.i_le(J.u32(J.get(self, "y")), ((height_px + 64 : U32) * 256 : U32)))), Bool.and(RS.i_ge(J.u32(J.get(self, "vx")), 4294963200), RS.i_le(J.u32(J.get(self, "vx")), 4096))), Bool.and(RS.i_ge(J.u32(J.get(self, "vy")), 4294961296), RS.i_le(J.u32(J.get(self, "vy")), 6000))), Bool.or(U32.is_eq(J.u32(J.get(self, "dir")), 4294967295), U32.is_eq(J.u32(J.get(self, "dir")), 1))), U32.is_le(J.u32(J.get(self, "dead")), 12)), U32.is_lt(J.u32(J.get(self, "t")), 16777216))

def bannerpeak.foes.Foe.on_floor?(+self: J.Val, +g: J.Val) -> Bool:
  Bool.and(U32.is_eq(RS.i_rem(J.u32(J.get(self, "y")), 4096), 0), bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, RS.i_div_euclid(J.u32(J.get(self, "x")), 4096), RS.i_div_euclid(J.u32(J.get(self, "y")), 4096))))

def bannerpeak.foes.Foe.step?(+self: J.Val, +g: J.Val, +near: Bool) -> J.Val:
  bannerpeak.foes.Foe.step.if4(U32.is_gt(J.u32(J.get(self, "dead")), 0), self, g, near)

def bannerpeak.foes.Foe.step.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "dir", J.of_i32(1))
      self
    case False{}:
      +self = J.set(self, "dir", J.of_i32(4294967295))
      self

def bannerpeak.foes.Foe.step.if2?(bpc: Bool, +self: J.Val, +g: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +swing : U32 = bannerpeak.layout.ping_pong(J.u32(J.get(self, "t")), 150, 0, RS.i_neg(44), 44)
      +x : U32 = (J.u32(J.get(self, "home")) + swing : U32)
      +self = bannerpeak.foes.Foe.step.if1(RS.i_ge((x * 256 : U32), J.u32(J.get(self, "x"))), self)
      +self = J.set(self, "x", J.of_i32((x * 256 : U32)))
      self
    case False{}:
      +_t1 = bannerpeak.foes.Foe.walker(self, g)
      +self : J.Val = _t1
      self

def bannerpeak.foes.Foe.step.if3?(bpc: Bool, +self: J.Val, +g: J.Val) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +self = J.set(self, "t", J.JU{(J.u32(J.get(self, "t")) + 1 : U32)})
      +self = bannerpeak.foes.Foe.step.if2(String.eq(J.str(J.get(self, "kind")), "Wisp"), self, g)
      self

def bannerpeak.foes.Foe.step.if4?(bpc: Bool, +self: J.Val, +g: J.Val, +near: Bool) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "dead", J.JU{(J.u32(J.get(self, "dead")) - 1 : U32)})
      self
    case False{}:
      bannerpeak.foes.Foe.step.if3(Bool.not(near), self, g)

def bannerpeak.foes.Foe.walker?(+self: J.Val, +g: J.Val) -> J.Val:
  +speed : U32 = 170
  +speed = bannerpeak.foes.Foe.walker.if1(String.eq(J.str(J.get(self, "kind")), "Capped"), speed)
  +self = J.set(self, "vx", J.of_i32((J.u32(J.get(self, "dir")) * speed : U32)))
  +self = J.set(self, "vy", J.of_i32(RS.i_min((J.u32(J.get(self, "vy")) + 210 : U32), 3000)))
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.foes.FoeKind.size(J.str(J.get(self, "kind")))
  +w = bannerpeak.u.fst(U32, U32, _t1)
  +h = bannerpeak.u.snd(U32, U32, _t1)
  +nx : U32 = (J.u32(J.get(self, "x")) + J.u32(J.get(self, "vx")) : U32)
  +edge : U32 = (nx - RS.i_div((w * 256 : U32), 2) : U32)
  +edge = bannerpeak.foes.Foe.walker.if2(RS.i_gt(J.u32(J.get(self, "dir")), 0), w, nx, edge)
  +col : U32 = RS.i_div_euclid(edge, 4096)
  +top : U32 = RS.i_div_euclid((J.u32(J.get(self, "y")) - (h * 256 : U32) : U32), 4096)
  +bottom : U32 = RS.i_div_euclid((J.u32(J.get(self, "y")) - 1 : U32), 4096)
  +wall : Bool = bannerpeak.body.Body.solid_rows(g, col, top, bottom)
  +ground : Bool = Bool.or(U32.is_eq(J.u32(J.get(self, "vy")), 0), bannerpeak.foes.Foe.on_floor(self, g))
  +under : U32 = bannerpeak.session.View.tile(g, col, RS.i_div_euclid(J.u32(J.get(self, "y")), 4096))
  +drop : Bool = Bool.and(Bool.and(ground, Bool.not(bannerpeak.world.Tile.solid(under))), Bool.not(bannerpeak.world.Tile.one_way(under)))
  +self = bannerpeak.foes.Foe.walker.if3(Bool.or(wall, drop), self, nx)
  +ny : U32 = (J.u32(J.get(self, "y")) + J.u32(J.get(self, "vy")) : U32)
  +foot_col : U32 = RS.i_div_euclid(J.u32(J.get(self, "x")), 4096)
  +row : U32 = RS.i_div_euclid(ny, 4096)
  +below : U32 = bannerpeak.session.View.tile(g, foot_col, row)
  +self = bannerpeak.foes.Foe.walker.if4(Bool.and(Bool.and(RS.i_gt(J.u32(J.get(self, "vy")), 0), Bool.or(bannerpeak.world.Tile.solid(below), bannerpeak.world.Tile.one_way(below))), RS.i_le(J.u32(J.get(self, "y")), (row * 4096 : U32))), self, ny, row)
  self

def bannerpeak.foes.Foe.walker.if1?(bpc: Bool, +speed: U32) -> U32:
  match bpc:
    case True{}:
      +speed : U32 = 130
      speed
    case False{}:
      speed

def bannerpeak.foes.Foe.walker.if2?(bpc: Bool, +w: U32, +nx: U32, +edge: U32) -> U32:
  match bpc:
    case True{}:
      +edge : U32 = (nx + RS.i_div((w * 256 : U32), 2) : U32)
      edge
    case False{}:
      edge

def bannerpeak.foes.Foe.walker.if3?(bpc: Bool, +self: J.Val, +nx: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "dir", J.of_i32(RS.i_neg(J.u32(J.get(self, "dir")))))
      +self = J.set(self, "vx", J.of_i32(0))
      self
    case False{}:
      +self = J.set(self, "x", J.of_i32(nx))
      self

def bannerpeak.foes.Foe.walker.if4?(bpc: Bool, +self: J.Val, +ny: U32, +row: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "y", J.of_i32((row * 4096 : U32)))
      +self = J.set(self, "vy", J.of_i32(0))
      self
    case False{}:
      +self = J.set(self, "y", J.of_i32(ny))
      self

def bannerpeak.foes.Foe.small?(+self: J.Val, +g: J.Val) -> J.Val:
  +_j1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.foes.FoeKind.size(J.str(J.get(self, "kind")))
  +_j2 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.foes.Foe.rect(self)
  J.set(J.set(J.set(J.set(J.obj(), "alive", J.JBool{bannerpeak.foes.Foe.alive(self)}), "size", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j1)), J.of_i32(bannerpeak.u.snd(U32, U32, _j1))]}), "rect", J.JA{[J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2)), J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2))), J.of_i32(bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2)))), J.of_i32(bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2))))]}), "on_floor", J.JBool{bannerpeak.foes.Foe.on_floor(self, g)})

def bannerpeak.foes.Item.new?(+kind: String, +x_px: U32, +y_px: U32) -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "kind", J.JS{kind}), "x", J.of_i32((x_px * 256 : U32))), "y", J.of_i32((y_px * 256 : U32))), "vx", J.of_i32(0)), "vy", J.of_i32(0)), "dir", J.of_i32(1)), "t", J.JU{0}), "cap", J.JS{"Rest"}), "owner", J.JU{0})

def bannerpeak.foes.Item.size?(+self: J.Val) -> Sigma<&2, &2, U32, _ => U32>:
  bannerpeak.foes.Item.size.if2(String.eq(J.str(J.get(self, "kind")), "Loaf"), self)

def bannerpeak.foes.Item.size.if1?(bpc: Bool) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      (10, 14)
    case False{}:
      (14, 10)

def bannerpeak.foes.Item.size.if2?(bpc: Bool, +self: J.Val) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      (12, 12)
    case False{}:
      bannerpeak.foes.Item.size.if1(String.eq(J.str(J.get(self, "kind")), "Flame"))

def bannerpeak.foes.Item.rect?(+self: J.Val) -> Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.foes.Item.size(self)
  +w = bannerpeak.u.fst(U32, U32, _t1)
  +h = bannerpeak.u.snd(U32, U32, _t1)
  +x : U32 = RS.i_div_euclid(J.u32(J.get(self, "x")), 256)
  +y : U32 = RS.i_div_euclid(J.u32(J.get(self, "y")), 256)
  ((x - RS.i_div(w, 2) : U32), ((y - h : U32), ((x + RS.i_div(w, 2) : U32), y)))

def bannerpeak.foes.Item.rising?(+self: J.Val) -> Bool:
  Bool.and(Bool.or(String.eq(J.str(J.get(self, "kind")), "Loaf"), String.eq(J.str(J.get(self, "kind")), "Flame")), U32.is_lt(J.u32(J.get(self, "t")), 16))

def bannerpeak.foes.Item.valid?(+self: J.Val, +width_px: U32, +height_px: U32) -> Bool:
  Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(RS.i_ge(J.u32(J.get(self, "x")), 0), RS.i_le(J.u32(J.get(self, "x")), (width_px * 256 : U32))), Bool.and(RS.i_ge(J.u32(J.get(self, "y")), (4294967292 * 4096 : U32)), RS.i_le(J.u32(J.get(self, "y")), ((height_px + 64 : U32) * 256 : U32)))), Bool.and(RS.i_ge(J.u32(J.get(self, "vx")), 4294963200), RS.i_le(J.u32(J.get(self, "vx")), 4096))), Bool.and(RS.i_ge(J.u32(J.get(self, "vy")), 4294961296), RS.i_le(J.u32(J.get(self, "vy")), 6000))), Bool.or(U32.is_eq(J.u32(J.get(self, "dir")), 4294967295), U32.is_eq(J.u32(J.get(self, "dir")), 1))), U32.is_lt(J.u32(J.get(self, "t")), 16777216)), U32.is_le(J.u32(J.get(self, "owner")), 8))

def bannerpeak.foes.Item.step?(+self: J.Val, +g: J.Val) -> J.Val:
  +self = J.set(self, "t", J.JU{RS.u_sat_add(J.u32(J.get(self, "t")), 1)})
  bannerpeak.foes.Item.step.if6(String.eq(J.str(J.get(self, "kind")), "Flame"), self, g)

def bannerpeak.foes.Item.step.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "y", J.of_i32((J.u32(J.get(self, "y")) - 256 : U32)))
      self
    case False{}:
      self

def bannerpeak.foes.Item.step.if2?(bpc: Bool, +self: J.Val, +g: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "y", J.of_i32((J.u32(J.get(self, "y")) - 256 : U32)))
      self
    case False{}:
      +_t1 = bannerpeak.foes.Item.walk(self, g, 384, True{})
      +self : J.Val = _t1
      self

def bannerpeak.foes.Item.step.if3?(bpc: Bool, +self: J.Val, +g: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t3 = bannerpeak.foes.Item.walk(self, g, 0, False{})
      +self : J.Val = _t3
      self
    case False{}:
      self

def bannerpeak.foes.Item.step.if4?(bpc: Bool, +self: J.Val, +g: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t2 = bannerpeak.foes.Item.walk(self, g, 1800, False{})
      +self : J.Val = _t2
      self
    case False{}:
      +self = bannerpeak.foes.Item.step.if3(String.eq(J.str(J.get(self, "cap")), "Rest"), self, g)
      self

def bannerpeak.foes.Item.step.if5?(bpc: Bool, +self: J.Val, +g: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.foes.Item.step.if2(U32.is_le(J.u32(J.get(self, "t")), 16), self, g)
    case False{}:
      +self = bannerpeak.foes.Item.step.if4(String.eq(J.str(J.get(self, "cap")), "Slide"), self, g)
      self

def bannerpeak.foes.Item.step.if6?(bpc: Bool, +self: J.Val, +g: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = bannerpeak.foes.Item.step.if1(U32.is_le(J.u32(J.get(self, "t")), 16), self)
      self
    case False{}:
      bannerpeak.foes.Item.step.if5(String.eq(J.str(J.get(self, "kind")), "Loaf"), self, g)

def bannerpeak.foes.Item.walk?(+self: J.Val, +g: J.Val, +speed: U32, +careful: Bool) -> J.Val:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.foes.Item.size(self)
  +w = bannerpeak.u.fst(U32, U32, _t1)
  +h = bannerpeak.u.snd(U32, U32, _t1)
  +self = J.set(self, "vy", J.of_i32(RS.i_min((J.u32(J.get(self, "vy")) + 210 : U32), 3000)))
  +self = bannerpeak.foes.Item.walk.if3(U32.is_ne(speed, 0), self, g, speed, careful, w, h)
  +ny : U32 = (J.u32(J.get(self, "y")) + J.u32(J.get(self, "vy")) : U32)
  +row : U32 = RS.i_div_euclid(ny, 4096)
  +below : U32 = bannerpeak.session.View.tile(g, RS.i_div_euclid(J.u32(J.get(self, "x")), 4096), row)
  +self = bannerpeak.foes.Item.walk.if4(Bool.and(Bool.and(RS.i_gt(J.u32(J.get(self, "vy")), 0), Bool.or(bannerpeak.world.Tile.solid(below), bannerpeak.world.Tile.one_way(below))), RS.i_le(J.u32(J.get(self, "y")), (row * 4096 : U32))), self, ny, row)
  self

def bannerpeak.foes.Item.walk.if1?(bpc: Bool, +w: U32, +nx: U32, +edge: U32) -> U32:
  match bpc:
    case True{}:
      +edge : U32 = (nx + RS.i_div((w * 256 : U32), 2) : U32)
      edge
    case False{}:
      edge

def bannerpeak.foes.Item.walk.if2?(bpc: Bool, +self: J.Val, +nx: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "dir", J.of_i32(RS.i_neg(J.u32(J.get(self, "dir")))))
      self
    case False{}:
      +self = J.set(self, "x", J.of_i32(nx))
      self

def bannerpeak.foes.Item.walk.if3?(bpc: Bool, +self: J.Val, +g: J.Val, +speed: U32, +careful: Bool, +w: U32, +h: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vx", J.of_i32((J.u32(J.get(self, "dir")) * speed : U32)))
      +nx : U32 = (J.u32(J.get(self, "x")) + J.u32(J.get(self, "vx")) : U32)
      +edge : U32 = (nx - RS.i_div((w * 256 : U32), 2) : U32)
      +edge = bannerpeak.foes.Item.walk.if1(RS.i_gt(J.u32(J.get(self, "dir")), 0), w, nx, edge)
      +col : U32 = RS.i_div_euclid(edge, 4096)
      +top : U32 = RS.i_div_euclid((J.u32(J.get(self, "y")) - (h * 256 : U32) : U32), 4096)
      +bottom : U32 = RS.i_div_euclid((J.u32(J.get(self, "y")) - 1 : U32), 4096)
      +wall : Bool = bannerpeak.body.Body.solid_rows(g, col, top, bottom)
      +under : U32 = bannerpeak.session.View.tile(g, col, RS.i_div_euclid(J.u32(J.get(self, "y")), 4096))
      +drop : Bool = Bool.and(Bool.and(Bool.and(careful, U32.is_eq(J.u32(J.get(self, "vy")), 0)), Bool.not(bannerpeak.world.Tile.solid(under))), Bool.not(bannerpeak.world.Tile.one_way(under)))
      +self = bannerpeak.foes.Item.walk.if2(Bool.or(wall, drop), self, nx)
      self
    case False{}:
      self

def bannerpeak.foes.Item.walk.if4?(bpc: Bool, +self: J.Val, +ny: U32, +row: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "y", J.of_i32((row * 4096 : U32)))
      +self = J.set(self, "vy", J.of_i32(0))
      self
    case False{}:
      +self = J.set(self, "y", J.of_i32(ny))
      self

def bannerpeak.foes.Item.small?(+self: J.Val) -> J.Val:
  +_j1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.foes.Item.size(self)
  +_j2 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.foes.Item.rect(self)
  J.set(J.set(J.set(J.obj(), "size", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j1)), J.of_i32(bannerpeak.u.snd(U32, U32, _j1))]}), "rect", J.JA{[J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2)), J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2))), J.of_i32(bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2)))), J.of_i32(bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j2))))]}), "rising", J.JBool{bannerpeak.foes.Item.rising(self)})

def bannerpeak.foes.Bolt.valid?(+self: J.Val, +width_px: U32, +height_px: U32) -> Bool:
  Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(RS.i_ge(J.u32(J.get(self, "x")), 0), RS.i_le(J.u32(J.get(self, "x")), (width_px * 256 : U32))), Bool.and(RS.i_ge(J.u32(J.get(self, "y")), (4294967292 * 4096 : U32)), RS.i_le(J.u32(J.get(self, "y")), ((height_px + 64 : U32) * 256 : U32)))), Bool.and(RS.i_ge(J.u32(J.get(self, "vx")), 4294963200), RS.i_le(J.u32(J.get(self, "vx")), 4096))), Bool.and(RS.i_ge(J.u32(J.get(self, "vy")), 4294961296), RS.i_le(J.u32(J.get(self, "vy")), 6000))), U32.is_le(J.u32(J.get(self, "t")), 70)), U32.is_le(J.u32(J.get(self, "owner")), 8))

def bannerpeak.foes.Bolt.rect?(+self: J.Val) -> Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>:
  +x : U32 = RS.i_div_euclid(J.u32(J.get(self, "x")), 256)
  +y : U32 = RS.i_div_euclid(J.u32(J.get(self, "y")), 256)
  ((x - 3 : U32), ((y - 3 : U32), ((x + 3 : U32), (y + 3 : U32))))

def bannerpeak.foes.Bolt.step?(+self: J.Val, +g: J.Val) -> Sigma<&2, &2, Bool, _ => J.Val>:
  +self = J.set(self, "t", J.JU{(J.u32(J.get(self, "t")) + 1 : U32)})
  +nx : U32 = (J.u32(J.get(self, "x")) + J.u32(J.get(self, "vx")) : U32)
  bannerpeak.foes.Bolt.step.if3(bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, RS.i_div_euclid(nx, 4096), RS.i_div_euclid(J.u32(J.get(self, "y")), 4096))), self, g, nx)

def bannerpeak.foes.Bolt.step.if1?(bpc: Bool, +self: J.Val, +ny: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(0))
      self
    case False{}:
      +self = J.set(self, "y", J.of_i32(ny))
      self

def bannerpeak.foes.Bolt.step.if2?(bpc: Bool, +self: J.Val, +g: J.Val, +ny: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "vy", J.of_i32(4294966396))
      self
    case False{}:
      +self = bannerpeak.foes.Bolt.step.if1(Bool.and(RS.i_lt(J.u32(J.get(self, "vy")), 0), bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, RS.i_div_euclid(J.u32(J.get(self, "x")), 4096), RS.i_div_euclid((ny - (3 * 256 : U32) : U32), 4096)))), self, ny)
      self

def bannerpeak.foes.Bolt.step.if3?(bpc: Bool, +self: J.Val, +g: J.Val, +nx: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      (False{}, self)
    case False{}:
      +self = J.set(self, "x", J.of_i32(nx))
      +self = J.set(self, "vy", J.of_i32(RS.i_min((J.u32(J.get(self, "vy")) + 130 : U32), 2400)))
      +ny : U32 = (J.u32(J.get(self, "y")) + J.u32(J.get(self, "vy")) : U32)
      +below : U32 = bannerpeak.session.View.tile(g, RS.i_div_euclid(J.u32(J.get(self, "x")), 4096), RS.i_div_euclid((ny + (3 * 256 : U32) : U32), 4096))
      +self = bannerpeak.foes.Bolt.step.if2(Bool.and(RS.i_gt(J.u32(J.get(self, "vy")), 0), Bool.or(bannerpeak.world.Tile.solid(below), bannerpeak.world.Tile.one_way(below))), self, g, ny)
      (U32.is_le(J.u32(J.get(self, "t")), 70), self)

def case.bannerpeak.foes.Foe.new?(+input: J.Val) -> J.Val:
  bannerpeak.foes.Foe.new(J.str(J.get(input, "kind")), J.u32(J.get(input, "cx")), J.u32(J.get(input, "cy")), J.u32(J.get(input, "mark")))

def case.bannerpeak.foes.Foe.step?(+input: J.Val) -> J.Val:
  bannerpeak.foes.Foe.step(J.get(input, "foe"), J.get(input, "g"), J.bool(J.get(input, "near")))

def case.bannerpeak.foes.Foe.valid?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.foes.Foe.valid(J.get(input, "foe"), J.u32(J.get(input, "w")), J.u32(J.get(input, "h")))}

def case.bannerpeak.foes.Foe.defeat?(+input: J.Val) -> J.Val:
  bannerpeak.foes.Foe.defeat(J.get(input, "foe"))

def case.bannerpeak.foes.Foe.small?(+input: J.Val) -> J.Val:
  bannerpeak.foes.Foe.small(J.get(input, "foe"), J.get(input, "g"))

def case.bannerpeak.foes.Item.new?(+input: J.Val) -> J.Val:
  bannerpeak.foes.Item.new(J.str(J.get(input, "kind")), J.u32(J.get(input, "x")), J.u32(J.get(input, "y")))

def case.bannerpeak.foes.Item.step?(+input: J.Val) -> J.Val:
  bannerpeak.foes.Item.step(J.get(input, "item"), J.get(input, "g"))

def case.bannerpeak.foes.Item.walk?(+input: J.Val) -> J.Val:
  bannerpeak.foes.Item.walk(J.get(input, "item"), J.get(input, "g"), J.u32(J.get(input, "speed")), J.bool(J.get(input, "careful")))

def case.bannerpeak.foes.Item.valid?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.foes.Item.valid(J.get(input, "item"), J.u32(J.get(input, "w")), J.u32(J.get(input, "h")))}

def case.bannerpeak.foes.Item.small?(+input: J.Val) -> J.Val:
  bannerpeak.foes.Item.small(J.get(input, "item"))

def case.bannerpeak.foes.Bolt.step?(+input: J.Val) -> J.Val:
  +r = bannerpeak.foes.Bolt.step(J.get(input, "bolt"), J.get(input, "g"))
  J.set(J.set(J.obj(), "alive", J.JBool{bannerpeak.u.fst(Bool, J.Val, r)}), "bolt", bannerpeak.u.snd(Bool, J.Val, r))

def case.bannerpeak.foes.Bolt.valid?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.foes.Bolt.valid(J.get(input, "bolt"), J.u32(J.get(input, "w")), J.u32(J.get(input, "h")))}

def case.bannerpeak.foes.Bolt.rect?(+input: J.Val) -> J.Val:
  +_j1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.foes.Bolt.rect(J.get(input, "bolt"))
  J.JA{[J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1)), J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1))), J.of_i32(bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1)))), J.of_i32(bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _j1))))]}

def bannerpeak.SLOT?() -> U32:
  2

def bannerpeak.BEGUN?() -> U32:
  1

def bannerpeak.FIRST_PURSE?() -> U32:
  60

def bannerpeak.REPEAT_PURSE?() -> U32:
  12

def bannerpeak.RUN?() -> U32:
  1152

def bannerpeak.HOP?() -> U32:
  10

def bannerpeak.COYOTE?() -> U32:
  5

def bannerpeak.SLAM?() -> U32:
  36

def bannerpeak.LEDGE?() -> U32:
  3000

def bannerpeak.Run.bannerpeak_showing?(+self: J.Val) -> Bool:
  Bool.and(String.eq(bannerpeak.u.room_kind(self), "forest"), Bool.not(J.is_null(J.get(self, "bannerpeak"))))

def bannerpeak.Run.start_bannerpeak?(+self: J.Val) -> J.Val:
  bannerpeak.Run.start_bannerpeak.if1(Bool.not(String.eq(bannerpeak.u.room_kind(self), "forest")), self)

def bannerpeak.Run.start_bannerpeak.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +_t1 = forest.Forest.leave(J.get(self, "forest"))
      +self = J.set(self, "forest", _t1)
      +self = J.set(self, "bannerpeak", bannerpeak.session.Session.new(J.get(self, "players")))
      +_t2 = bannerpeak.u.record_play(J.get(self, "home"))
      +self = J.set(self, "home", _t2)
      +self = J.set(self, "found", bannerpeak.u.found(J.get(self, "tick"), "Bannerpeak: raise eight banners. Hold Esc to return."))
      +self = J.set(self, "sounds", J.push(J.get(self, "sounds"), J.JS{"play_card"}))
      self

def bannerpeak.Run.bannerpeak_knobs?(+self: J.Val) -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.obj(), "run", J.of_i32(roundtable.number(J.get(self, "tune"), 2, 0))), "hop", J.JU{roundtable.number(J.get(self, "tune"), 2, 1)}), "grace", J.JU{roundtable.number(J.get(self, "tune"), 2, 2)}), "slam", J.of_i32(roundtable.number(J.get(self, "tune"), 2, 3))), "ledge", J.of_i32(roundtable.number(J.get(self, "tune"), 2, 4)))

def bannerpeak.Run.tick_bannerpeak?(+self: J.Val, +inputs: J.Val) -> J.Val:
  bannerpeak.Run.tick_bannerpeak.if3(J.is_null(J.get(self, "bannerpeak")), self, inputs)

def bannerpeak.Run.tick_bannerpeak.if1?(bpc: Bool, +self: J.Val, +s: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t2 = bannerpeak.Run.bannerpeak_pays(self, s)
      +self : J.Val = _t2
      self
    case False{}:
      self

def bannerpeak.Run.tick_bannerpeak.if2?(bpc: Bool, +self: J.Val, +inputs: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +returning : J.Val = bannerpeak.Run.returning(self, inputs, bannerpeak.u.ids(inputs), bannerpeak.u.vec_new())
      +_t3 = bannerpeak.Run.leave_all(self, returning, 0)
      +self : J.Val = _t3
      self
    case False{}:
      self

def bannerpeak.Run.tick_bannerpeak.if3?(bpc: Bool, +self: J.Val, +inputs: J.Val) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +s : J.Val = J.get(self, "bannerpeak")
      +present : J.Val = bannerpeak.u.present(self, bannerpeak.u.ids(J.get(self, "players")), J.obj())
      +knobs : J.Val = bannerpeak.Run.bannerpeak_knobs(self)
      +was_done : Bool = Bool.not(J.is_null(J.get(s, "completed")))
      +say : J.Val = bannerpeak.u.vec_new()
      +_t1 = bannerpeak.session.Session.step(s, inputs, present, knobs, say)
      +s : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t1)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t1)
      +self = J.set(self, "sounds", bannerpeak.u.vec_extend(J.get(self, "sounds"), say))
      +self = bannerpeak.Run.tick_bannerpeak.if1(Bool.and(Bool.not(was_done), Bool.not(J.is_null(J.get(s, "completed")))), self, s)
      +done : Bool = Bool.not(J.is_null(J.get(s, "completed")))
      +self = J.set(self, "bannerpeak", s)
      +self = bannerpeak.Run.tick_bannerpeak.if2(done, self, inputs)
      self

def bannerpeak.Run.returning?(+self: J.Val, +inputs: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  bannerpeak.Run.returning.if2(bannerpeak.u.is_empty(ids), self, inputs, ids, acc)

def bannerpeak.Run.returning.if1?(bpc: Bool, +acc: J.Val, +id: U32) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.JU{id})
      acc
    case False{}:
      acc

def bannerpeak.Run.returning.if2?(bpc: Bool, +self: J.Val, +inputs: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +i : J.Val = bannerpeak.u.at(inputs, id)
      +acc = bannerpeak.Run.returning.if1(Bool.and(Bool.and(J.bool(J.get(i, "interact")), rules.Input.valid(i)), Bool.not(bannerpeak.u.outside(self, id))), acc, id)
      bannerpeak.Run.returning(self, inputs, bannerpeak.u.tail(ids), acc)

def bannerpeak.Run.leave_all?(+self: J.Val, +returning: J.Val, +n: U32) -> J.Val:
  bannerpeak.Run.leave_all.if1(U32.is_ge(n, J.len(returning)), self, returning, n)

def bannerpeak.Run.leave_all.if1?(bpc: Bool, +self: J.Val, +returning: J.Val, +n: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +_t1 = bannerpeak.u.leave(self, J.u32(J.nth(returning, n)))
      +self : J.Val = _t1
      +_t2 = bannerpeak.Run.leave_all(self, returning, (n + 1 : U32))
      +self : J.Val = _t2
      self

def bannerpeak.Run.pay_seats?(+self: J.Val, +ids: List<&2, U32>, +purse: U32) -> J.Val:
  bannerpeak.Run.pay_seats.if2(bannerpeak.u.is_empty(ids), self, ids, purse)

def bannerpeak.Run.pay_seats.if1?(bpc: Bool, +self: J.Val, +purse: U32, +id: U32) -> J.Val:
  match bpc:
    case True{}:
      +h : J.Val = bannerpeak.u.at(J.get(self, "players"), id)
      +_t1 = bannerpeak.u.pay(h, purse)
      +h : J.Val = _t1
      +self = J.set(self, "players", bannerpeak.u.put(J.get(self, "players"), id, h))
      self
    case False{}:
      self

def bannerpeak.Run.pay_seats.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +purse: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +self = bannerpeak.Run.pay_seats.if1(bannerpeak.u.has(J.get(self, "players"), id), self, purse, id)
      +_t2 = bannerpeak.Run.pay_seats(self, bannerpeak.u.tail(ids), purse)
      +self : J.Val = _t2
      self

def bannerpeak.Run.bannerpeak_pays?(+self: J.Val, +s: J.Val) -> J.Val:
  +first : Bool = U32.is_eq(U32.and(bannerpeak.u.flags(J.get(self, "home")), 1), 0)
  +purse : U32 = 12
  +purse = bannerpeak.Run.bannerpeak_pays.if1(first, purse)
  +_t1 = bannerpeak.Run.pay_seats(self, bannerpeak.u.ids(J.get(s, "seats")), purse)
  +self : J.Val = _t1
  +seconds : U32 = U32.div(J.u32(J.get(s, "elapsed")), 30)
  +score : U32 = RS.u_max(RS.u_sat_sub(1000, RS.u_sat_sub(seconds, 120)), 100)
  +_t2 = bannerpeak.u.record_win(J.get(self, "home"), RS.u_min(score, 1000))
  +self = J.set(self, "home", _t2)
  +_t3 = bannerpeak.u.flags_put(J.get(self, "home"), U32.or(bannerpeak.u.flags(J.get(self, "home")), 1))
  +self = J.set(self, "home", _t3)
  +_t4 = bannerpeak.u.flag_set(J.get(self, "marks"), "bannerpeak:campaign-begun")
  +self = J.set(self, "marks", _t4)
  +self = J.set(self, "found", bannerpeak.u.found(J.get(self, "tick"), bannerpeak.u.pays_line(purse)))
  +self = J.set(self, "sounds", J.push(J.get(self, "sounds"), J.JS{"spoil_pickup"}))
  self

def bannerpeak.Run.bannerpeak_pays.if1?(bpc: Bool, +purse: U32) -> U32:
  match bpc:
    case True{}:
      +purse : U32 = 60
      purse
    case False{}:
      purse

def bannerpeak.ADVENTURE.valid?(+run: J.Val) -> Bool:
  Bool.or(J.is_null(J.get(run, "bannerpeak")), bannerpeak.session.Session.valid(J.get(run, "bannerpeak")))

def bannerpeak.ADVENTURE.clear?(+run: J.Val) -> J.Val:
  +run = J.set(run, "bannerpeak", J.JNull{})
  run

def bannerpeak.ADVENTURE.seated?(+run: J.Val) -> J.Val:
  bannerpeak.ADVENTURE.seated.if1(J.is_null(J.get(run, "bannerpeak")), run)

def bannerpeak.ADVENTURE.seated.if1?(bpc: Bool, +run: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.u.vec_new()
    case False{}:
      bannerpeak.u.ids_vec(bannerpeak.u.ids(J.get(J.get(run, "bannerpeak"), "seats")), bannerpeak.u.vec_new())

def bannerpeak.ADVENTURE.bot?(+run: J.Val, +id: U32) -> J.Val:
  bannerpeak.ADVENTURE.bot.if1(Bool.and(Bool.not(J.is_null(J.get(run, "bannerpeak"))), bannerpeak.u.has(J.get(run, "players"), id)), run, id)

def bannerpeak.ADVENTURE.bot.if1?(bpc: Bool, +run: J.Val, +id: U32) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.bot.Session.bot_input(J.get(run, "bannerpeak"), id, bannerpeak.u.at(J.get(run, "players"), id))
    case False{}:
      bannerpeak.u.input_default()

def bannerpeak.ACTIVITY.ride?(+run: J.Val, +inputs: J.Val) -> Sigma<&2, &2, Bool, _ => J.Val>:
  +_t1 = bannerpeak.Run.tick_bannerpeak(run, inputs)
  +run : J.Val = _t1
  (True{}, run)

def bannerpeak.ACTIVITY.end?(+run: J.Val) -> J.Val:
  +run = J.set(run, "bannerpeak", J.JNull{})
  run

def case.bannerpeak.Run.bannerpeak_showing?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.Run.bannerpeak_showing(J.get(input, "run"))}

def case.bannerpeak.Run.start_bannerpeak?(+input: J.Val) -> J.Val:
  bannerpeak.Run.start_bannerpeak(J.get(input, "run"))

def case.bannerpeak.Run.bannerpeak_knobs?(+input: J.Val) -> J.Val:
  bannerpeak.Run.bannerpeak_knobs(J.get(input, "run"))

def case.bannerpeak.Run.tick_bannerpeak?(+input: J.Val) -> J.Val:
  bannerpeak.Run.tick_bannerpeak(J.get(input, "run"), J.get(input, "inputs"))

def case.bannerpeak.Run.bannerpeak_pays?(+input: J.Val) -> J.Val:
  bannerpeak.Run.bannerpeak_pays(J.get(input, "run"), J.get(input, "session"))

def case.bannerpeak.Run.tick_bannerpeak.done?(+input: J.Val) -> J.Val:
  bannerpeak.Run.tick_bannerpeak(J.get(input, "run"), J.get(input, "inputs"))

def case.bannerpeak.ADVENTURE.valid?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.ADVENTURE.valid(J.get(input, "run"))}

def case.bannerpeak.ADVENTURE.clear?(+input: J.Val) -> J.Val:
  bannerpeak.ADVENTURE.clear(J.get(input, "run"))

def case.bannerpeak.ADVENTURE.seated?(+input: J.Val) -> J.Val:
  bannerpeak.ADVENTURE.seated(J.get(input, "run"))

def case.bannerpeak.ADVENTURE.bot?(+input: J.Val) -> J.Val:
  bannerpeak.ADVENTURE.bot(J.get(input, "run"), J.u32(J.get(input, "id")))

def case.bannerpeak.ACTIVITY.ride?(+input: J.Val) -> J.Val:
  +r = bannerpeak.ACTIVITY.ride(J.get(input, "run"), J.get(input, "inputs"))
  J.set(J.set(J.obj(), "some", J.JBool{bannerpeak.u.fst(Bool, J.Val, r)}), "run", bannerpeak.u.snd(Bool, J.Val, r))

def case.bannerpeak.ACTIVITY.end?(+input: J.Val) -> J.Val:
  bannerpeak.ACTIVITY.end(J.get(input, "run"))

def bannerpeak.knight.PIPS?() -> U32:
  8

def bannerpeak.knight.START_PIPS?() -> U32:
  6

def bannerpeak.knight.BACK_PIPS?() -> U32:
  3

def bannerpeak.knight.WIPE_PIPS?() -> U32:
  4

def bannerpeak.knight.HURT_GRACE?() -> U32:
  70

def bannerpeak.knight.BUBBLE_TICKS?() -> U32:
  330

def bannerpeak.knight.PIT_TICKS?() -> U32:
  14

def bannerpeak.knight.POSE_TICKS?() -> U32:
  54

def bannerpeak.knight.B_JUMP?() -> U32:
  1

def bannerpeak.knight.B_DOWN?() -> U32:
  2

def bannerpeak.knight.B_ACT?() -> U32:
  4

def bannerpeak.knight.B_BRACE?() -> U32:
  8

def bannerpeak.knight.B_DIVE?() -> U32:
  16

def bannerpeak.knight.B_USE?() -> U32:
  32

def bannerpeak.knight.CAM_W?() -> U32:
  384

def bannerpeak.knight.CAM_H?() -> U32:
  224

def bannerpeak.knight.Knight.at?(+joined: U32, +x: U32, +y: U32) -> J.Val:
  +_j1 : Sigma<&2, &2, U32, _ => U32> = (x, y)
  +_j2 : Sigma<&2, &2, U32, _ => U32> = (0, 0)
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "joined", J.JU{joined}), "body", bannerpeak.body.Body.new(x, y)), "size", J.JS{"Small"}), "pips", J.JU{6}), "coins", J.JU{0}), "score", J.JU{0}), "chain", J.JU{0}), "hurt", J.JU{0}), "bubble", J.JNull{}), "pit", J.JU{0}), "pose", J.JU{0}), "pole", J.JU{0}), "grip", J.of_i32(0)), "poled", J.JBool{False{}}), "safe", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j1)), J.of_i32(bannerpeak.u.snd(U32, U32, _j1))]}), "safe_t", J.JU{0}), "cam", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j2)), J.of_i32(bannerpeak.u.snd(U32, U32, _j2))]}), "prev", J.JU{0}), "cooldown", J.JU{0}), "carry", J.JBool{False{}}), "shock", J.JU{0}), "landed", J.JU{0})

def bannerpeak.knight.Knight.free?(+self: J.Val) -> Bool:
  Bool.and(Bool.and(Bool.and(J.is_null(J.get(self, "bubble")), U32.is_eq(J.u32(J.get(self, "pit")), 0)), U32.is_eq(J.u32(J.get(self, "pose")), 0)), U32.is_eq(J.u32(J.get(self, "pole")), 0))

def bannerpeak.knight.Knight.alive?(+self: J.Val) -> Bool:
  Bool.and(J.is_null(J.get(self, "bubble")), U32.is_eq(J.u32(J.get(self, "pit")), 0))

def bannerpeak.knight.Knight.big?(+self: J.Val) -> Bool:
  Bool.not(String.eq(J.str(J.get(self, "size")), "Small"))

def bannerpeak.knight.Knight.edge?(+self: J.Val, +bits: U32, +b: U32) -> Bool:
  Bool.and(U32.is_ne(U32.and(bits, b), 0), U32.is_eq(U32.and(J.u32(J.get(self, "prev")), b), 0))

def bannerpeak.knight.Knight.cmd?(+self: J.Val, +i: J.Val) -> Sigma<&2, &2, J.Val, _ => U32>:
  +up : Bool = RS.i_lt(J.u32(J.get(i, "move_y")), 0)
  +jump : Bool = Bool.or(J.bool(J.get(i, "dash")), up)
  +down : Bool = RS.i_gt(J.u32(J.get(i, "move_y")), 0)
  +bits : U32 = 0
  +bits = bannerpeak.knight.Knight.cmd.if1(jump, bits)
  +bits = bannerpeak.knight.Knight.cmd.if2(down, bits)
  +bits = bannerpeak.knight.Knight.cmd.if3(J.bool(J.get(i, "fire")), bits)
  +bits = bannerpeak.knight.Knight.cmd.if4(J.bool(J.get(i, "interact")), bits)
  +bits = bannerpeak.knight.Knight.cmd.if5(J.bool(J.get(i, "swing")), bits)
  +dive : Bool = Bool.or(bannerpeak.knight.Knight.edge(self, bits, 16), Bool.and(Bool.and(bannerpeak.knight.Knight.edge(self, bits, 4), Bool.not(String.eq(J.str(J.get(self, "size")), "Flame"))), Bool.not(J.bool(J.get(self, "carry")))))
  (J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32(J.u32(J.get(i, "move_x")))), "jump", J.JBool{jump}), "jump_edge", J.JBool{bannerpeak.knight.Knight.edge(self, bits, 1)}), "down", J.JBool{down}), "down_edge", J.JBool{bannerpeak.knight.Knight.edge(self, bits, 2)}), "dive_edge", J.JBool{dive}), "brace", J.JBool{J.bool(J.get(i, "interact"))}), bits)

def bannerpeak.knight.Knight.cmd.if1?(bpc: Bool, +bits: U32) -> U32:
  match bpc:
    case True{}:
      +bits : U32 = U32.or(bits, 1)
      bits
    case False{}:
      bits

def bannerpeak.knight.Knight.cmd.if2?(bpc: Bool, +bits: U32) -> U32:
  match bpc:
    case True{}:
      +bits : U32 = U32.or(bits, 2)
      bits
    case False{}:
      bits

def bannerpeak.knight.Knight.cmd.if3?(bpc: Bool, +bits: U32) -> U32:
  match bpc:
    case True{}:
      +bits : U32 = U32.or(bits, 4)
      bits
    case False{}:
      bits

def bannerpeak.knight.Knight.cmd.if4?(bpc: Bool, +bits: U32) -> U32:
  match bpc:
    case True{}:
      +bits : U32 = U32.or(bits, 8)
      bits
    case False{}:
      bits

def bannerpeak.knight.Knight.cmd.if5?(bpc: Bool, +bits: U32) -> U32:
  match bpc:
    case True{}:
      +bits : U32 = U32.or(bits, 16)
      bits
    case False{}:
      bits

def bannerpeak.knight.Knight.act_edge?(+self: J.Val, +bits: U32) -> Bool:
  Bool.and(U32.is_ne(U32.and(bits, 4), 0), U32.is_eq(U32.and(J.u32(J.get(self, "prev")), 4), 0))

def bannerpeak.knight.Knight.use_edge?(+self: J.Val, +bits: U32) -> Bool:
  Bool.and(U32.is_ne(U32.and(bits, 8), 0), U32.is_eq(U32.and(J.u32(J.get(self, "prev")), 8), 0))

def bannerpeak.knight.Knight.hit?(+self: J.Val, +from: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  +_b1 = U32.is_gt(J.u32(J.get(self, "hurt")), 0)
  +_t2 = bannerpeak.knight.Knight.hit.if1(Bool.not(_b1), self, _b1)
  +_b1 = bannerpeak.u.fst(Bool, J.Val, _t2)
  +self = bannerpeak.u.snd(Bool, J.Val, _t2)
  bannerpeak.knight.Knight.hit.if4(_b1, self, from)

def bannerpeak.knight.Knight.hit.if1?(bpc: Bool, +self: J.Val, +_b1: Bool) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +_b1 : Bool = Bool.not(bannerpeak.knight.Knight.free(self))
      (_b1, self)
    case False{}:
      (_b1, self)

def bannerpeak.knight.Knight.hit.if2?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "size", J.JS{"Small"})
      self
    case False{}:
      +self = J.set(self, "pips", J.JU{RS.u_sat_sub(J.u32(J.get(self, "pips")), 1)})
      self

def bannerpeak.knight.Knight.hit.if3?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "size", J.JS{"Big"})
      self
    case False{}:
      +self = bannerpeak.knight.Knight.hit.if2(String.eq(J.str(J.get(self, "size")), "Big"), self)
      self

def bannerpeak.knight.Knight.hit.if4?(bpc: Bool, +self: J.Val, +from: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      (False{}, self)
    case False{}:
      +self = J.set(self, "hurt", J.JU{70})
      +self = J.set(self, "chain", J.JU{0})
      +self = J.set(self, "carry", J.JBool{False{}})
      +self = bannerpeak.knight.Knight.hit.if3(String.eq(J.str(J.get(self, "size")), "Flame"), self)
      +self = J.set(self, "body", J.set(J.get(self, "body"), "big", J.JBool{bannerpeak.knight.Knight.big(self)}))
      +_t3 = bannerpeak.body.Body.hurt(J.get(self, "body"), from)
      +self = J.set(self, "body", _t3)
      (True{}, self)

def bannerpeak.knight.Bubble.valid?(+b: J.Val) -> Bool:
  Bool.and(Bool.and(Bool.and(RS.i_ge(J.u32(J.get(b, "x")), 0), RS.i_le(J.u32(J.get(b, "x")), ((434 * 16 : U32) * 256 : U32))), Bool.and(RS.i_ge(J.u32(J.get(b, "y")), ((4294967288 * 16 : U32) * 256 : U32)), RS.i_le(J.u32(J.get(b, "y")), (((64 + 4 : U32) * 16 : U32) * 256 : U32)))), U32.is_le(J.u32(J.get(b, "t")), 330))

def bannerpeak.knight.Knight.valid?(+self: J.Val) -> Bool:
  bannerpeak.knight.Knight.valid.if1(Bool.not(bannerpeak.body.Body.valid(J.get(self, "body"))), self)

def bannerpeak.knight.Knight.valid.if1?(bpc: Bool, +self: J.Val) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +_t1 : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.get(self, "safe"), 0)), J.u32(J.nth(J.get(self, "safe"), 1)))
      +_t2 : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.get(self, "safe"), 0)), J.u32(J.nth(J.get(self, "safe"), 1)))
      +_t3 : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.get(self, "cam"), 0)), J.u32(J.nth(J.get(self, "cam"), 1)))
      +_t4 : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.get(self, "cam"), 0)), J.u32(J.nth(J.get(self, "cam"), 1)))
      Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(U32.is_le(J.u32(J.get(self, "pips")), 8), U32.is_le(J.u32(J.get(self, "hurt")), 70)), U32.is_le(J.u32(J.get(self, "pit")), 14)), U32.is_le(J.u32(J.get(self, "pose")), 54)), U32.is_lt(J.u32(J.get(self, "pole")), 600)), Bool.and(RS.i_ge(J.u32(J.get(self, "grip")), 0), RS.i_le(J.u32(J.get(self, "grip")), 9))), U32.is_lt(J.u32(J.get(self, "safe_t")), 16777216)), U32.is_le(J.u32(J.get(self, "cooldown")), 60)), U32.is_le(J.u32(J.get(self, "shock")), 12)), U32.is_lt(J.u32(J.get(self, "landed")), 16777216)), Bool.not(Bool.xor(J.bool(J.get(J.get(self, "body"), "big")), bannerpeak.knight.Knight.big(self)))), U32.is_le(J.u32(J.get(self, "chain")), 12)), U32.is_lt(J.u32(J.get(self, "coins")), 1048576)), U32.is_lt(J.u32(J.get(self, "score")), 268435456)), Bool.and(RS.i_ge(bannerpeak.u.fst(U32, U32, _t1), 0), RS.i_le(bannerpeak.u.fst(U32, U32, _t1), (434 * 16 : U32)))), Bool.and(RS.i_ge(bannerpeak.u.snd(U32, U32, _t2), 4294967232), RS.i_le(bannerpeak.u.snd(U32, U32, _t2), ((64 * 16 : U32) + 64 : U32)))), Bool.and(RS.i_ge(bannerpeak.u.fst(U32, U32, _t3), 4294966784), RS.i_le(bannerpeak.u.fst(U32, U32, _t3), ((434 * 16 : U32) + 512 : U32)))), Bool.and(RS.i_ge(bannerpeak.u.snd(U32, U32, _t4), 4294966784), RS.i_le(bannerpeak.u.snd(U32, U32, _t4), ((64 * 16 : U32) + 512 : U32)))), Bool.or(J.is_null(J.get(self, "bubble")), bannerpeak.knight.Bubble.valid(J.get(self, "bubble"))))

def bannerpeak.knight.follow?(+cam: Sigma<&2, &2, U32, _ => U32>, +fx: U32, +fy: U32, +face: U32, +vx: U32, +vy: U32) -> Sigma<&2, &2, U32, _ => U32>:
  +lead : U32 = RS.i_clamp(((face * 26 : U32) + RS.i_div(vx, 40) : U32), 4294967226, 70)
  +want_x : U32 = RS.i_clamp(((fx + lead : U32) - RS.i_div(384, 2) : U32), 0, ((434 * 16 : U32) - 384 : U32))
  +look : U32 = 0
  +look = bannerpeak.knight.follow.if1(RS.i_gt(vy, 1500), vy, look)
  +want_y : U32 = RS.i_clamp(((fy - 158 : U32) + look : U32), 0, ((64 * 16 : U32) - 224 : U32))
  +most_y : U32 = 6
  +most_y = bannerpeak.knight.follow.if2(RS.i_gt(vy, 1500), most_y)
  +_t1 : Sigma<&2, &2, U32, _ => U32> = cam
  +_t2 : Sigma<&2, &2, U32, _ => U32> = cam
  +x : U32 = (bannerpeak.u.fst(U32, U32, _t1) + RS.i_clamp((want_x - bannerpeak.u.fst(U32, U32, _t2) : U32), 4294967289, 7) : U32)
  +_t3 : Sigma<&2, &2, U32, _ => U32> = cam
  +_t4 : Sigma<&2, &2, U32, _ => U32> = cam
  +y : U32 = (bannerpeak.u.snd(U32, U32, _t3) + RS.i_clamp((want_y - bannerpeak.u.snd(U32, U32, _t4) : U32), RS.i_neg(most_y), most_y) : U32)
  +x : U32 = RS.i_clamp(RS.i_clamp(x, (fx - 270 : U32), (fx - 114 : U32)), 0, ((434 * 16 : U32) - 384 : U32))
  +y : U32 = RS.i_clamp(RS.i_clamp(y, (fy - 200 : U32), (fy - 70 : U32)), 0, ((64 * 16 : U32) - 224 : U32))
  (x, y)

def bannerpeak.knight.follow.if1?(bpc: Bool, +vy: U32, +look: U32) -> U32:
  match bpc:
    case True{}:
      +look : U32 = RS.i_min(RS.i_div((vy - 1500 : U32), 40), 48)
      look
    case False{}:
      look

def bannerpeak.knight.follow.if2?(bpc: Bool, +most_y: U32) -> U32:
  match bpc:
    case True{}:
      +most_y : U32 = 14
      most_y
    case False{}:
      most_y

def bannerpeak.knight.Knight.small?(+self: J.Val, +bits: U32) -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "free", J.JBool{bannerpeak.knight.Knight.free(self)}), "alive", J.JBool{bannerpeak.knight.Knight.alive(self)}), "big", J.JBool{bannerpeak.knight.Knight.big(self)}), "act", J.JBool{bannerpeak.knight.Knight.act_edge(self, bits)}), "use", J.JBool{bannerpeak.knight.Knight.use_edge(self, bits)}), "bits", J.JU{bits})

def case.bannerpeak.knight.Knight.cmd?(+input: J.Val) -> J.Val:
  +_j1 : Sigma<&2, &2, J.Val, _ => U32> = bannerpeak.knight.Knight.cmd(J.get(input, "knight"), J.get(input, "input"))
  J.JA{[bannerpeak.u.fst(J.Val, U32, _j1), J.JU{bannerpeak.u.snd(J.Val, U32, _j1)}]}

def case.bannerpeak.knight.Knight.hit?(+input: J.Val) -> J.Val:
  +r = bannerpeak.knight.Knight.hit(J.get(input, "knight"), J.u32(J.get(input, "from")))
  J.set(J.set(J.obj(), "ok", J.JBool{bannerpeak.u.fst(Bool, J.Val, r)}), "knight", bannerpeak.u.snd(Bool, J.Val, r))

def case.bannerpeak.knight.Knight.valid?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.knight.Knight.valid(J.get(input, "knight"))}

def case.bannerpeak.knight.follow?(+input: J.Val) -> J.Val:
  +_j2 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.knight.follow((J.u32(J.nth(J.get(input, "cam"), 0)), J.u32(J.nth(J.get(input, "cam"), 1))), J.u32(J.get(input, "fx")), J.u32(J.get(input, "fy")), J.u32(J.get(input, "face")), J.u32(J.get(input, "vx")), J.u32(J.get(input, "vy")))
  J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j2)), J.of_i32(bannerpeak.u.snd(U32, U32, _j2))]}

def bannerpeak.layout.HUB_OX?() -> U32:
  0

def bannerpeak.layout.HUB_OY?() -> U32:
  32

def bannerpeak.layout.HUB_W?() -> U32:
  80

def bannerpeak.layout.HUB_H?() -> U32:
  30

def bannerpeak.layout.MARSH_OX?() -> U32:
  80

def bannerpeak.layout.MARSH_OY?() -> U32:
  36

def bannerpeak.layout.MARSH_W?() -> U32:
  105

def bannerpeak.layout.MARSH_H?() -> U32:
  28

def bannerpeak.layout.MILL_OX?() -> U32:
  185

def bannerpeak.layout.MILL_OY?() -> U32:
  36

def bannerpeak.layout.MILL_W?() -> U32:
  88

def bannerpeak.layout.MILL_H?() -> U32:
  18

def bannerpeak.layout.SHAFT_OX?() -> U32:
  273

def bannerpeak.layout.SHAFT_OY?() -> U32:
  16

def bannerpeak.layout.SHAFT_W?() -> U32:
  14

def bannerpeak.layout.SHAFT_H?() -> U32:
  38

def bannerpeak.layout.SNOW_OX?() -> U32:
  287

def bannerpeak.layout.SNOW_OY?() -> U32:
  12

def bannerpeak.layout.SNOW_W?() -> U32:
  105

def bannerpeak.layout.SNOW_H?() -> U32:
  14

def bannerpeak.layout.SUMMIT_OX?() -> U32:
  392

def bannerpeak.layout.SUMMIT_OY?() -> U32:
  10

def bannerpeak.layout.SUMMIT_W?() -> U32:
  42

def bannerpeak.layout.SUMMIT_H?() -> U32:
  14

def bannerpeak.layout.WIDTH?() -> U32:
  434

def bannerpeak.layout.HEIGHT?() -> U32:
  64

def bannerpeak.layout.SNOW_FLOOR?() -> U32:
  22

def bannerpeak.layout.WHEEL_X?() -> U32:
  3496

def bannerpeak.layout.WHEEL_Y?() -> U32:
  760

def bannerpeak.layout.WHEEL_RADIUS?() -> U32:
  40

def bannerpeak.layout.WHEEL_BUCKETS?() -> U32:
  6

def bannerpeak.layout.WHEEL_TURN?() -> U32:
  270

def bannerpeak.layout.WHEEL_ID?() -> U32:
  200

def bannerpeak.layout.LOG_ID?() -> U32:
  100

def bannerpeak.layout.section_of?(+x: U32) -> U32:
  bannerpeak.layout.section_of.if5(RS.i_ge(x, 392), x)

def bannerpeak.layout.section_of.if1?(bpc: Bool) -> U32:
  match bpc:
    case True{}:
      1
    case False{}:
      0

def bannerpeak.layout.section_of.if2?(bpc: Bool, +x: U32) -> U32:
  match bpc:
    case True{}:
      2
    case False{}:
      bannerpeak.layout.section_of.if1(RS.i_ge(x, 80))

def bannerpeak.layout.section_of.if3?(bpc: Bool, +x: U32) -> U32:
  match bpc:
    case True{}:
      3
    case False{}:
      bannerpeak.layout.section_of.if2(RS.i_ge(x, 185), x)

def bannerpeak.layout.section_of.if4?(bpc: Bool, +x: U32) -> U32:
  match bpc:
    case True{}:
      4
    case False{}:
      bannerpeak.layout.section_of.if3(RS.i_ge(x, 273), x)

def bannerpeak.layout.section_of.if5?(bpc: Bool, +x: U32) -> U32:
  match bpc:
    case True{}:
      5
    case False{}:
      bannerpeak.layout.section_of.if4(RS.i_ge(x, 287), x)

def bannerpeak.layout.cell_centre_x?(+cx: U32) -> U32:
  ((cx * 16 : U32) + RS.i_div(16, 2) : U32)

def bannerpeak.layout.cell_centre_y?(+cy: U32) -> U32:
  ((cy * 16 : U32) + RS.i_div(16, 2) : U32)

def bannerpeak.layout.cell_feet_x?(+cx: U32) -> U32:
  ((cx * 16 : U32) + RS.i_div(16, 2) : U32)

def bannerpeak.layout.cell_feet_y?(+cy: U32) -> U32:
  ((cy + 1 : U32) * 16 : U32)

def bannerpeak.layout.hatch_to?(+tx: U32, +ty: U32) -> J.Val:
  bannerpeak.layout.hatch_to.if2(Bool.and(U32.is_eq(tx, (80 + 44 : U32)), U32.is_eq(ty, (36 + 12 : U32))), tx, ty)

def bannerpeak.layout.hatch_to.if1?(bpc: Bool) -> J.Val:
  match bpc:
    case True{}:
      J.set(J.set(J.set(J.obj(), "some", J.JBool{True{}}), "x", J.of_i32((80 + 45 : U32))), "y", J.of_i32((36 + 11 : U32)))
    case False{}:
      J.set(J.set(J.set(J.obj(), "some", J.JBool{False{}}), "x", J.of_i32(0)), "y", J.of_i32(0))

def bannerpeak.layout.hatch_to.if2?(bpc: Bool, +tx: U32, +ty: U32) -> J.Val:
  match bpc:
    case True{}:
      J.set(J.set(J.set(J.obj(), "some", J.JBool{True{}}), "x", J.of_i32((80 + 50 : U32))), "y", J.of_i32((36 + 25 : U32)))
    case False{}:
      bannerpeak.layout.hatch_to.if1(Bool.and(U32.is_eq(tx, (80 + 56 : U32)), U32.is_eq(ty, (36 + 26 : U32))))

def bannerpeak.layout.cell?(+cx: U32, +cy: U32) -> J.Val:
  +_j1 : Sigma<&2, &2, U32, _ => U32> = (bannerpeak.layout.cell_centre_x(cx), bannerpeak.layout.cell_centre_y(cy))
  +_j2 : Sigma<&2, &2, U32, _ => U32> = (bannerpeak.layout.cell_feet_x(cx), bannerpeak.layout.cell_feet_y(cy))
  J.set(J.set(J.obj(), "centre", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j1)), J.of_i32(bannerpeak.u.snd(U32, U32, _j1))]}), "feet", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j2)), J.of_i32(bannerpeak.u.snd(U32, U32, _j2))]})

def bannerpeak.layout.sine_quarter?(+d: U32) -> U32:
  bannerpeak.layout.sine_table(d)

def bannerpeak.layout.sine?(+deg: U32) -> U32:
  +d : U32 = RS.i_rem_euclid(deg, 360)
  bannerpeak.layout.sine.if3(RS.i_le(d, 90), d)

def bannerpeak.layout.sine.if1?(bpc: Bool, +d: U32) -> U32:
  match bpc:
    case True{}:
      RS.i_neg(bannerpeak.layout.sine_quarter((d - 180 : U32)))
    case False{}:
      RS.i_neg(bannerpeak.layout.sine_quarter((360 - d : U32)))

def bannerpeak.layout.sine.if2?(bpc: Bool, +d: U32) -> U32:
  match bpc:
    case True{}:
      bannerpeak.layout.sine_quarter((180 - d : U32))
    case False{}:
      bannerpeak.layout.sine.if1(RS.i_le(d, 270), d)

def bannerpeak.layout.sine.if3?(bpc: Bool, +d: U32) -> U32:
  match bpc:
    case True{}:
      bannerpeak.layout.sine_quarter(d)
    case False{}:
      bannerpeak.layout.sine.if2(RS.i_le(d, 180), d)

def bannerpeak.layout.cosine?(+deg: U32) -> U32:
  bannerpeak.layout.sine((deg + 90 : U32))

def bannerpeak.layout.ping_pong?(+t: U32, +period: U32, +phase: U32, +lo: U32, +hi: U32) -> U32:
  +half : U32 = U32.div(period, 2)
  +k : U32 = U32.mod((t + phase : U32), period)
  +up : U32 = bannerpeak.u.ife(U32, U32.is_lt(k, half), _ => k, _ => (period - k : U32))
  (lo + RS.i_div(((hi - lo : U32) * up : U32), half) : U32)

def bannerpeak.layout.log?(+id: U32, +top: U32, +left: U32) -> J.Val:
  J.set(J.set(J.obj(), "id", J.JU{id}), "p", J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32(left)), "y", J.of_i32(top)), "w", J.of_i32(48)), "lift", J.JBool{False{}}))

def bannerpeak.layout.bucket?(+deg: U32, +i: U32) -> J.Val:
  +phi : U32 = (deg + RS.i_div((i * 360 : U32), 6) : U32)
  +x : U32 = (3496 + RS.i_div((40 * bannerpeak.layout.cosine(phi) : U32), 1024) : U32)
  +y : U32 = (760 + RS.i_div((40 * bannerpeak.layout.sine(phi) : U32), 1024) : U32)
  J.set(J.set(J.obj(), "id", J.JU{(200 + i : U32)}), "p", J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32((x - 14 : U32))), "y", J.of_i32(y)), "w", J.of_i32(28)), "lift", J.JBool{False{}}))

def bannerpeak.layout.buckets?(+deg: U32, +i: U32, +out: J.Val) -> J.Val:
  bannerpeak.layout.buckets.if1(RS.i_ge(i, 6), deg, i, out)

def bannerpeak.layout.buckets.if1?(bpc: Bool, +deg: U32, +i: U32, +out: J.Val) -> J.Val:
  match bpc:
    case True{}:
      out
    case False{}:
      +out = J.push(out, bannerpeak.layout.bucket(deg, i))
      +_t1 = bannerpeak.layout.buckets(deg, (i + 1 : U32), out)
      +out : J.Val = _t1
      out

def bannerpeak.layout.movers?(+t: U32) -> J.Val:
  +out : J.Val = bannerpeak.u.vec_new()
  +bank : U32 = ((36 + 12 : U32) * 16 : U32)
  +gap : U32 = ((80 + 62 : U32) * 16 : U32)
  +out = J.push(out, bannerpeak.layout.log(100, bank, bannerpeak.layout.ping_pong(t, 150, 0, (gap + 4 : U32), (gap + 52 : U32))))
  +out = J.push(out, bannerpeak.layout.log((100 + 1 : U32), bank, bannerpeak.layout.ping_pong(t, 150, 75, (gap + 68 : U32), (gap + 116 : U32))))
  +water : U32 = ((36 + 14 : U32) * 16 : U32)
  +race : U32 = ((185 + 19 : U32) * 16 : U32)
  +out = J.push(out, bannerpeak.layout.log((100 + 2 : U32), water, bannerpeak.layout.ping_pong(t, 120, 0, race, (race + 32 : U32))))
  +deg : U32 = RS.i_div((U32.mod(t, 270) * 360 : U32), 270)
  +_t1 = bannerpeak.layout.buckets(deg, 0, out)
  +out : J.Val = _t1
  out

def bannerpeak.layout.info.gate_x?(+i: U32) -> U32:
  bannerpeak.u.nth(bannerpeak.layout.info.gates(), (i * 2 : U32))

def bannerpeak.layout.info.gate_y?(+i: U32) -> U32:
  bannerpeak.u.nth(bannerpeak.layout.info.gates(), ((i * 2 : U32) + 1 : U32))

def bannerpeak.layout.info.spark_x?(+i: U32) -> U32:
  bannerpeak.u.nth(bannerpeak.layout.info.sparks(), (i * 2 : U32))

def bannerpeak.layout.info.spark_y?(+i: U32) -> U32:
  bannerpeak.u.nth(bannerpeak.layout.info.sparks(), ((i * 2 : U32) + 1 : U32))

def bannerpeak.layout.info.banner_has?(+n: U32) -> Bool:
  U32.is_eq(bannerpeak.u.nth(bannerpeak.layout.info.banners(), ((n * 3 : U32) + 2 : U32)), 1)

def bannerpeak.layout.info.banner_x?(+n: U32) -> U32:
  bannerpeak.u.nth(bannerpeak.layout.info.banners(), (n * 3 : U32))

def bannerpeak.layout.info.banner_y?(+n: U32) -> U32:
  bannerpeak.u.nth(bannerpeak.layout.info.banners(), ((n * 3 : U32) + 1 : U32))

def bannerpeak.layout.hint_at?(+x: U32, +y: U32) -> U32:
  bannerpeak.layout.hint_scan(bannerpeak.layout.info.hints(), x, y)

def bannerpeak.layout.hint_scan?(+xs: List<&2, U32>, +x: U32, +y: U32) -> U32:
  bannerpeak.layout.hint_scan.if2(bannerpeak.u.is_empty(xs), xs, x, y)

def bannerpeak.layout.hint_scan.if1?(bpc: Bool, +x: U32, +y: U32, +g: U32, +r2: List<&2, U32>) -> U32:
  match bpc:
    case True{}:
      g
    case False{}:
      bannerpeak.layout.hint_scan(bannerpeak.u.tail(r2), x, y)

def bannerpeak.layout.hint_scan.if2?(bpc: Bool, +xs: List<&2, U32>, +x: U32, +y: U32) -> U32:
  match bpc:
    case True{}:
      0
    case False{}:
      +g : U32 = bannerpeak.u.head(xs)
      +r1 : List<&2, U32> = bannerpeak.u.tail(xs)
      +hx : U32 = bannerpeak.u.head(r1)
      +r2 : List<&2, U32> = bannerpeak.u.tail(r1)
      +hy : U32 = bannerpeak.u.head(r2)
      bannerpeak.layout.hint_scan.if1(Bool.and(U32.is_eq(hx, x), U32.is_eq(hy, y)), x, y, g, r2)

def bannerpeak.layout.spark_index?(+x: U32, +y: U32) -> U32:
  bannerpeak.layout.spark_scan(0, x, y)

def bannerpeak.layout.spark_scan?(+n: U32, +x: U32, +y: U32) -> U32:
  bannerpeak.layout.spark_scan.if2(U32.is_ge(n, bannerpeak.layout.info.sparks_len()), n, x, y)

def bannerpeak.layout.spark_scan.if1?(bpc: Bool, +n: U32, +x: U32, +y: U32) -> U32:
  match bpc:
    case True{}:
      n
    case False{}:
      bannerpeak.layout.spark_scan((n + 1 : U32), x, y)

def bannerpeak.layout.spark_scan.if2?(bpc: Bool, +n: U32, +x: U32, +y: U32) -> U32:
  match bpc:
    case True{}:
      8
    case False{}:
      bannerpeak.layout.spark_scan.if1(Bool.and(U32.is_eq(bannerpeak.layout.info.spark_x(n), x), U32.is_eq(bannerpeak.layout.info.spark_y(n), y)), n, x, y)

def case.bannerpeak.layout.sine?(+input: J.Val) -> J.Val:
  J.of_i32(bannerpeak.layout.sine(J.u32(J.get(input, "deg"))))

def case.bannerpeak.layout.cosine?(+input: J.Val) -> J.Val:
  J.of_i32(bannerpeak.layout.cosine(J.u32(J.get(input, "deg"))))

def case.bannerpeak.layout.ping_pong?(+input: J.Val) -> J.Val:
  J.of_i32(bannerpeak.layout.ping_pong(J.u32(J.get(input, "t")), J.u32(J.get(input, "period")), J.u32(J.get(input, "phase")), J.u32(J.get(input, "lo")), J.u32(J.get(input, "hi"))))

def case.bannerpeak.layout.movers?(+input: J.Val) -> J.Val:
  bannerpeak.layout.movers(J.u32(J.get(input, "t")))

def case.bannerpeak.layout.section_of?(+input: J.Val) -> J.Val:
  J.JU{bannerpeak.layout.section_of(J.u32(J.get(input, "x")))}

def case.bannerpeak.layout.cell?(+input: J.Val) -> J.Val:
  bannerpeak.layout.cell(J.u32(J.get(input, "cx")), J.u32(J.get(input, "cy")))

def case.bannerpeak.layout.hatch_to?(+input: J.Val) -> J.Val:
  bannerpeak.layout.hatch_to(J.u32(J.get(input, "tx")), J.u32(J.get(input, "ty")))

def bannerpeak.u.room_kind?(+run: J.Val) -> String:
  J.str(J.get(rules.Run.room(run), "kind"))

# `found = Some((tick, 0, text))`: the line the party reads.

def bannerpeak.u.found?(+tick: J.Val, +text: String) -> J.Val:
  J.JA{[tick, J.JU{0}, J.JS{text}]}

# The Run's players' sounds: `sounds.extend(say)`.

def bannerpeak.u.vec_extend?(+a: J.Val, +b: J.Val) -> J.Val:
  J.JA{List.append(&2, J.Val, J.items(a), J.items(b))}

# `exits.outside.contains_key(id)`.

def bannerpeak.u.outside?(+run: J.Val, +id: U32) -> Bool:
  J.has(J.get(J.get(run, "exits"), "outside"), U32.show(id))

# `Run::leave_activity(id)` without its answer: the run after.

def bannerpeak.u.leave?(+run: J.Val, +id: U32) -> J.Val:
  bannerpeak.u.fst(J.Val, Bool, exits.Run.leave_activity(run, id))

# The record of this adventure: `home.countryside.games.roundtable[SLOT]`, five scores
# that are left out of the save while all are default.

def bannerpeak.u.score0?() -> J.Val:
  J.set(J.set(J.set(J.obj(), "best", J.JU{0}), "wins", J.JU{0}), "plays", J.JU{0})

def bannerpeak.u.scores?(+home: J.Val) -> J.Val:
  bannerpeak.u.scores.go(J.get(J.get(J.get(home, "countryside"), "games"), "roundtable"))

def bannerpeak.u.scores.go?(+a: J.Val) -> J.Val:
  match a:
    case J.JNull{}:
      J.JA{[bannerpeak.u.score0(), bannerpeak.u.score0(), bannerpeak.u.score0(), bannerpeak.u.score0(), bannerpeak.u.score0()]}
    case _:
      a

def bannerpeak.u.record?(+home: J.Val) -> J.Val:
  J.nth(bannerpeak.u.scores(home), 2)

def bannerpeak.u.record_put?(+home: J.Val, +rec: J.Val) -> J.Val:
  J.put(home, ["countryside", "games", "roundtable"], J.set_nth(bannerpeak.u.scores(home), 2, rec))

# A play begun: `record.plays = record.plays.saturating_add(1)`.

def bannerpeak.u.record_play?(+home: J.Val) -> J.Val:
  bannerpeak.u.record_put(home, ga.su(bannerpeak.u.record(home), "plays", ga.sadd32(ga.u(bannerpeak.u.record(home), "plays"), 1)))

# A win: `record.best = record.best.max(score.min(1000))`,
# `record.wins = record.wins.saturating_add(1).min(record.plays)`.

def bannerpeak.u.record_win?(+home: J.Val, +score: U32) -> J.Val:
  bannerpeak.u.record_win.go(home, bannerpeak.u.record(home), score)

def bannerpeak.u.record_win.go?(+home: J.Val, +rec: J.Val, +score: U32) -> J.Val:
  bannerpeak.u.record_put(home, ga.su(ga.su(rec, "best", U32.max(ga.u(rec, "best"), U32.min(score, 1000))), "wins", U32.min(ga.sadd32(ga.u(rec, "wins"), 1), ga.u(rec, "plays"))))

# `home.roundtable[SLOT]`: the flags this adventure settled (missing: none).

def bannerpeak.u.flags?(+home: J.Val) -> U32:
  J.u32(J.nth(J.get(home, "roundtable"), 2))

def bannerpeak.u.flags_put?(+home: J.Val, +bits: U32) -> J.Val:
  J.set(home, "roundtable", J.set_nth(bannerpeak.u.flags.arr(J.get(home, "roundtable")), 2, J.JU{bits}))

def bannerpeak.u.flags.arr?(+a: J.Val) -> J.Val:
  match a:
    case J.JNull{}:
      J.JA{[J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}]}
    case _:
      a

# A hero's gold: `carried.add(Spoil::Gold, n)`.

def bannerpeak.u.pay?(+h: J.Val, +n: U32) -> J.Val:
  J.set(h, "carried", realm.Spoils.add(J.get(h, "carried"), "gold", n))

def bannerpeak.u.flag_set?(+marks: J.Val, +key: String) -> J.Val:
  J.set(marks, key, J.JU{1})

# Heroes not outside: `players.iter().filter(!outside)`, as an object by id.

def bannerpeak.u.present?(+run: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      acc
    case Con{+id, +t}:
      bannerpeak.u.present(run, t, bannerpeak.u.present.one(bannerpeak.u.outside(run, id), id, J.get(J.get(run, "players"), U32.show(id)), acc))

def bannerpeak.u.present.one?(out: Bool, +id: U32, +h: J.Val, +acc: J.Val) -> J.Val:
  match out:
    case True{}:
      acc
    case False{}:
      J.set(acc, U32.show(id), h)

def bannerpeak.u.pays_line?(+purse: U32) -> String:
  "Bannerpeak: eight banners! " ++ U32.show(purse) ++ " gold each. E to return."

# A vector of ids as a J array of numbers.

def bannerpeak.u.ids_vec?(+ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      acc
    case Con{+h, +t}:
      bannerpeak.u.ids_vec(t, J.push(acc, J.JU{h}))

def bannerpeak.session.HZ?() -> U32:
  30

def bannerpeak.session.MAX_PLAYERS?() -> U32:
  6

def bannerpeak.session.LIMIT?() -> U32:
  108000

def bannerpeak.session.SUMMIT_BANNERS?() -> U32:
  7

def bannerpeak.session.WIPE_TICKS?() -> U32:
  20

def bannerpeak.session.SELECT_TICKS?() -> U32:
  240

def bannerpeak.session.FINALE_TICKS?() -> U32:
  210

def bannerpeak.session.ANNOUNCE_TICKS?() -> U32:
  110

def bannerpeak.session.FX_LIFE?() -> U32:
  14

def bannerpeak.session.BOARD_LEN?() -> U32:
  6

def bannerpeak.session.Cmd.default?() -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32(0)), "jump", J.JBool{False{}}), "jump_edge", J.JBool{False{}}), "down", J.JBool{False{}}), "down_edge", J.JBool{False{}}), "dive_edge", J.JBool{False{}}), "brace", J.JBool{False{}})

def bannerpeak.session.overlap?(+a: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +b: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>) -> Bool:
  +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = a
  +_t2 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = b
  +_t3 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = b
  +_t4 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = a
  +_t5 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = a
  +_t6 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = b
  +_t7 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = b
  +_t8 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = a
  Bool.and(Bool.and(Bool.and(RS.i_lt(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1), bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t2)))), RS.i_lt(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t3), bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t4))))), RS.i_lt(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t5)), bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t6))))), RS.i_lt(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t7)), bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t8)))))

def bannerpeak.session.px?(+b: J.Val) -> Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>:
  +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.body.Body.rect(b)
  +l = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)
  +t = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1))
  +r = bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
  +bt = bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
  (RS.i_div_euclid(l, 256), (RS.i_div_euclid(t, 256), ((RS.i_div_euclid((r - 1 : U32), 256) + 1 : U32), RS.i_div_euclid(bt, 256))))

def bannerpeak.session.Session.new?(+players: J.Val) -> J.Val:
  +s : J.Val = J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "seats", J.obj()), "elapsed", J.JU{0}), "banners", J.JU{0}), "sparks", J.JU{0}), "gates", J.JU{0}), "checkpoint", J.JU{0}), "doors", J.JU{0}), "phase", J.JS{"Play"}), "phase_t", J.JU{0}), "cursor", J.JU{0}), "foes", bannerpeak.u.vec_new()), "items", bannerpeak.u.vec_new()), "bolts", bannerpeak.u.vec_new()), "boulders", bannerpeak.u.vec_new()), "boss", bannerpeak.boss.Boss.new(1)), "fx", bannerpeak.u.vec_new()), "taken", bannerpeak.u.vec_new()), "changed", J.obj()), "announce", J.JNull{}), "wipe", J.JU{0}), "scaled", J.JU{0}), "seen", J.JBool{False{}}), "completed", J.JNull{})
  +_t1 = bannerpeak.session.Session.seat_all(s, players, bannerpeak.u.ids(players))
  +s : J.Val = _t1
  +_t2 = bannerpeak.session.Session.provision(s)
  +s : J.Val = _t2
  s

def bannerpeak.session.Session.seat_all?(+self: J.Val, +players: J.Val, +ids: List<&2, U32>) -> J.Val:
  bannerpeak.session.Session.seat_all.if2(bannerpeak.u.is_empty(ids), self, players, ids)

def bannerpeak.session.Session.seat_all.if1?(bpc: Bool, +self: J.Val, +id: U32) -> J.Val:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.session.Session.seat(self, id, 0)
      +self : J.Val = _t1
      self
    case False{}:
      self

def bannerpeak.session.Session.seat_all.if2?(bpc: Bool, +self: J.Val, +players: J.Val, +ids: List<&2, U32>) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +h : J.Val = bannerpeak.u.at(players, id)
      +self = bannerpeak.session.Session.seat_all.if1(Bool.and(Bool.not(J.bool(J.get(h, "away"))), Bool.not(J.bool(J.get(h, "stone")))), self, id)
      +_t2 = bannerpeak.session.Session.seat_all(self, players, bannerpeak.u.tail(ids))
      +self : J.Val = _t2
      self

def bannerpeak.session.Session.view?(+self: J.Val) -> J.Val:
  J.get(self, "changed")

def bannerpeak.session.Session.tile?(+self: J.Val, +tx: U32, +ty: U32) -> U32:
  bannerpeak.session.View.tile(J.get(self, "changed"), tx, ty)

def bannerpeak.session.Session.set_tile?(+self: J.Val, +tx: U32, +ty: U32, +code: U32) -> J.Val:
  +self = bannerpeak.session.Session.set_tile.if1(Bool.and(Bool.and(RS.i_ge(tx, 0), RS.i_lt(tx, 434)), Bool.and(RS.i_ge(ty, 0), RS.i_lt(ty, 64))), self, tx, ty, code)
  self

def bannerpeak.session.Session.set_tile.if1?(bpc: Bool, +self: J.Val, +tx: U32, +ty: U32, +code: U32) -> J.Val:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.u.view_insert(J.get(self, "changed"), ((ty * 434 : U32) + tx : U32), code)
      +self = J.set(self, "changed", _t1)
      self
    case False{}:
      self

def bannerpeak.session.Session.station?(+self: J.Val) -> Sigma<&2, &2, U32, _ => U32>:
  bannerpeak.session.Session.station.if1(U32.is_eq(J.u32(J.get(self, "checkpoint")), 0), self)

def bannerpeak.session.Session.station.if1?(bpc: Bool, +self: J.Val) -> Sigma<&2, &2, U32, _ => U32>:
  match bpc:
    case True{}:
      (bannerpeak.layout.cell_feet_x(bannerpeak.layout.info.start_x()), bannerpeak.layout.cell_feet_y(bannerpeak.layout.info.start_y()))
    case False{}:
      +n : U32 = (J.u32(J.get(self, "checkpoint")) - 1 : U32)
      (bannerpeak.layout.cell_feet_x(bannerpeak.layout.info.gate_x(n)), bannerpeak.layout.cell_feet_y(bannerpeak.layout.info.gate_y(n)))

def bannerpeak.session.Session.seat_count?(+self: J.Val) -> U32:
  bannerpeak.u.map_len(J.get(self, "seats"))

def bannerpeak.session.Session.seat?(+self: J.Val, +id: U32, +joined: U32) -> J.Val:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.session.Session.station(self)
  +x0 = bannerpeak.u.fst(U32, U32, _t1)
  +y = bannerpeak.u.snd(U32, U32, _t1)
  +n : U32 = bannerpeak.session.Session.seat_count(self)
  +sgn : U32 = 4294967295
  +sgn = bannerpeak.session.Session.seat.if1(U32.is_eq(RS.i_rem(n, 2), 0), sgn)
  +off : U32 = ((RS.i_div((n + 1 : U32), 2) * 7 : U32) * sgn : U32)
  +x : U32 = (x0 + off : U32)
  +g : J.Val = bannerpeak.session.Session.view(self)
  +cx : U32 = RS.i_div_euclid(x, 16)
  +cy : U32 = RS.i_div_euclid((y - 1 : U32), 16)
  +here : U32 = bannerpeak.session.View.tile(g, cx, cy)
  +below : U32 = bannerpeak.session.View.tile(g, cx, (cy + 1 : U32))
  +x = bannerpeak.session.Session.seat.if2(Bool.or(bannerpeak.world.Tile.solid(here), Bool.and(Bool.not(bannerpeak.world.Tile.solid(below)), Bool.not(bannerpeak.world.Tile.one_way(below)))), off, x)
  +k : J.Val = bannerpeak.knight.Knight.at(joined, x, y)
  +_j2 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.knight.follow((0, 0), x, y, 1, 0, 0)
  +k = J.set(k, "cam", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j2)), J.of_i32(bannerpeak.u.snd(U32, U32, _j2))]})
  +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), id, k))
  self

def bannerpeak.session.Session.seat.if1?(bpc: Bool, +sgn: U32) -> U32:
  match bpc:
    case True{}:
      +sgn : U32 = 1
      sgn
    case False{}:
      sgn

def bannerpeak.session.Session.seat.if2?(bpc: Bool, +off: U32, +x: U32) -> U32:
  match bpc:
    case True{}:
      +x : U32 = (x - off : U32)
      x
    case False{}:
      x

def bannerpeak.session.Session.provision?(+self: J.Val) -> J.Val:
  +seats : U32 = RS.u_max(bannerpeak.session.Session.seat_count(self), 1)
  +first : Bool = U32.is_eq(J.u32(J.get(self, "scaled")), 0)
  +was : U32 = bannerpeak.session.Session.tier(J.u32(J.get(self, "scaled")))
  +now : U32 = bannerpeak.session.Session.tier(seats)
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.session.Session.station(self)
  +station_x : U32 = bannerpeak.u.fst(U32, U32, _t1)
  +_t2 = bannerpeak.session.Session.provision_marks(self, bannerpeak.layout.info.foes(), 0, first, was, now, station_x)
  +self : J.Val = _t2
  +self = bannerpeak.session.Session.provision.if1(U32.is_eq(J.u32(J.get(self, "scaled")), 0), self, seats)
  +self = J.set(self, "scaled", J.JU{RS.u_max(J.u32(J.get(self, "scaled")), seats)})
  self

def bannerpeak.session.Session.provision.if1?(bpc: Bool, +self: J.Val, +seats: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "boss", J.set(J.get(self, "boss"), "plates", J.JU{seats}))
      self
    case False{}:
      +_t3 = bannerpeak.boss.Boss.seated(J.get(self, "boss"), J.u32(J.get(self, "scaled")), RS.u_max(seats, J.u32(J.get(self, "scaled"))))
      +self = J.set(self, "boss", _t3)
      self

def bannerpeak.session.Session.tier?(+s: U32) -> U32:
  (bannerpeak.u.b2u(U32.is_ge(s, 3)) + bannerpeak.u.b2u(U32.is_ge(s, 5)) : U32)

def bannerpeak.session.Session.provision_marks?(+self: J.Val, +ms: List<&2, U32>, +n: U32, +first: Bool, +was: U32, +now: U32, +station_x: U32) -> J.Val:
  bannerpeak.session.Session.provision_marks.if7(bannerpeak.u.is_empty(ms), self, ms, n, first, was, now, station_x)

def bannerpeak.session.Session.provision_marks.if1?(bpc: Bool, +was: U32, +now: U32, +kind: String, +wanted: Bool, +known: Bool) -> Sigma<&2, &2, String, _ => Sigma<&2, &2, Bool, _ => Bool>>:
  match bpc:
    case True{}:
      +kind : String = "Capped"
      +wanted : Bool = Bool.and(U32.is_lt(was, 2), U32.is_ge(now, 2))
      (kind, (known, wanted))
    case False{}:
      +known : Bool = False{}
      (kind, (known, wanted))

def bannerpeak.session.Session.provision_marks.if2?(bpc: Bool, +was: U32, +now: U32, +glyph: U32, +kind: String, +wanted: Bool, +known: Bool) -> Sigma<&2, &2, String, _ => Sigma<&2, &2, Bool, _ => Bool>>:
  match bpc:
    case True{}:
      +wanted : Bool = Bool.and(U32.is_lt(was, 1), U32.is_ge(now, 1))
      (kind, (known, wanted))
    case False{}:
      +_t1 = bannerpeak.session.Session.provision_marks.if1(U32.is_eq(glyph, 102), was, now, kind, wanted, known)
      +kind = bannerpeak.u.fst(String, Sigma<&2, &2, Bool, _ => Bool>, _t1)
      +known = bannerpeak.u.fst(Bool, Bool, bannerpeak.u.snd(String, Sigma<&2, &2, Bool, _ => Bool>, _t1))
      +wanted = bannerpeak.u.snd(Bool, Bool, bannerpeak.u.snd(String, Sigma<&2, &2, Bool, _ => Bool>, _t1))
      (kind, (known, wanted))

def bannerpeak.session.Session.provision_marks.if3?(bpc: Bool, +first: Bool, +was: U32, +now: U32, +glyph: U32, +kind: String, +wanted: Bool, +known: Bool) -> Sigma<&2, &2, String, _ => Sigma<&2, &2, Bool, _ => Bool>>:
  match bpc:
    case True{}:
      +kind : String = "Wisp"
      +wanted : Bool = first
      (kind, (known, wanted))
    case False{}:
      +_t2 = bannerpeak.session.Session.provision_marks.if2(U32.is_eq(glyph, 101), was, now, glyph, kind, wanted, known)
      +kind = bannerpeak.u.fst(String, Sigma<&2, &2, Bool, _ => Bool>, _t2)
      +known = bannerpeak.u.fst(Bool, Bool, bannerpeak.u.snd(String, Sigma<&2, &2, Bool, _ => Bool>, _t2))
      +wanted = bannerpeak.u.snd(Bool, Bool, bannerpeak.u.snd(String, Sigma<&2, &2, Bool, _ => Bool>, _t2))
      (kind, (known, wanted))

def bannerpeak.session.Session.provision_marks.if4?(bpc: Bool, +first: Bool, +was: U32, +now: U32, +glyph: U32, +kind: String, +wanted: Bool, +known: Bool) -> Sigma<&2, &2, String, _ => Sigma<&2, &2, Bool, _ => Bool>>:
  match bpc:
    case True{}:
      +kind : String = "Capped"
      +wanted : Bool = first
      (kind, (known, wanted))
    case False{}:
      +_t3 = bannerpeak.session.Session.provision_marks.if3(U32.is_eq(glyph, 119), first, was, now, glyph, kind, wanted, known)
      +kind = bannerpeak.u.fst(String, Sigma<&2, &2, Bool, _ => Bool>, _t3)
      +known = bannerpeak.u.fst(Bool, Bool, bannerpeak.u.snd(String, Sigma<&2, &2, Bool, _ => Bool>, _t3))
      +wanted = bannerpeak.u.snd(Bool, Bool, bannerpeak.u.snd(String, Sigma<&2, &2, Bool, _ => Bool>, _t3))
      (kind, (known, wanted))

def bannerpeak.session.Session.provision_marks.if5?(bpc: Bool, +first: Bool, +was: U32, +now: U32, +glyph: U32, +kind: String, +wanted: Bool, +known: Bool) -> Sigma<&2, &2, String, _ => Sigma<&2, &2, Bool, _ => Bool>>:
  match bpc:
    case True{}:
      +wanted : Bool = first
      (kind, (known, wanted))
    case False{}:
      +_t4 = bannerpeak.session.Session.provision_marks.if4(U32.is_eq(glyph, 99), first, was, now, glyph, kind, wanted, known)
      +kind = bannerpeak.u.fst(String, Sigma<&2, &2, Bool, _ => Bool>, _t4)
      +known = bannerpeak.u.fst(Bool, Bool, bannerpeak.u.snd(String, Sigma<&2, &2, Bool, _ => Bool>, _t4))
      +wanted = bannerpeak.u.snd(Bool, Bool, bannerpeak.u.snd(String, Sigma<&2, &2, Bool, _ => Bool>, _t4))
      (kind, (known, wanted))

def bannerpeak.session.Session.provision_marks.if6?(bpc: Bool, +self: J.Val, +n: U32, +mx: U32, +my: U32, +kind: String) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "foes", J.push(J.get(self, "foes"), bannerpeak.foes.Foe.new(kind, mx, my, n)))
      self
    case False{}:
      self

def bannerpeak.session.Session.provision_marks.if7?(bpc: Bool, +self: J.Val, +ms: List<&2, U32>, +n: U32, +first: Bool, +was: U32, +now: U32, +station_x: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +glyph : U32 = bannerpeak.u.head(ms)
      +r1 : List<&2, U32> = bannerpeak.u.tail(ms)
      +mx : U32 = bannerpeak.u.head(r1)
      +r2 : List<&2, U32> = bannerpeak.u.tail(r1)
      +my : U32 = bannerpeak.u.head(r2)
      +rest : List<&2, U32> = bannerpeak.u.tail(r2)
      +kind : String = "Snuffer"
      +wanted : Bool = False{}
      +known : Bool = True{}
      +_t5 = bannerpeak.session.Session.provision_marks.if5(U32.is_eq(glyph, 115), first, was, now, glyph, kind, wanted, known)
      +kind = bannerpeak.u.fst(String, Sigma<&2, &2, Bool, _ => Bool>, _t5)
      +known = bannerpeak.u.fst(Bool, Bool, bannerpeak.u.snd(String, Sigma<&2, &2, Bool, _ => Bool>, _t5))
      +wanted = bannerpeak.u.snd(Bool, Bool, bannerpeak.u.snd(String, Sigma<&2, &2, Bool, _ => Bool>, _t5))
      +behind : Bool = Bool.and(Bool.not(first), U32.is_lt((mx * 16 : U32), station_x))
      +self = bannerpeak.session.Session.provision_marks.if6(Bool.and(Bool.and(known, wanted), Bool.not(behind)), self, n, mx, my, kind)
      +_t6 = bannerpeak.session.Session.provision_marks(self, rest, (n + 1 : U32), first, was, now, station_x)
      +self : J.Val = _t6
      self

def bannerpeak.session.Session.fx?(+self: J.Val, +k: String, +x: U32, +y: U32) -> J.Val:
  +self = bannerpeak.session.Session.fx.if1(U32.is_ge(J.len(J.get(self, "fx")), 48), self)
  +self = J.set(self, "fx", J.push(J.get(self, "fx"), J.set(J.set(J.set(J.set(J.obj(), "k", J.JS{k}), "x", J.of_i32(x)), "y", J.of_i32(y)), "t", J.JU{14})))
  self

def bannerpeak.session.Session.fx.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "fx", bannerpeak.u.vec_remove(J.get(self, "fx"), 0))
      self
    case False{}:
      self

def bannerpeak.session.Session.seats_valid?(+self: J.Val, +ids: List<&2, U32>) -> Bool:
  bannerpeak.session.Session.seats_valid.if2(bannerpeak.u.is_empty(ids), self, ids)

def bannerpeak.session.Session.seats_valid.if1?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.seats_valid(self, bannerpeak.u.tail(ids))

def bannerpeak.session.Session.seats_valid.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      bannerpeak.session.Session.seats_valid.if1(Bool.not(Bool.and(Bool.and(Bool.and(U32.is_ge(id, 1), U32.is_le(id, 6)), bannerpeak.knight.Knight.valid(k)), U32.is_le(J.u32(J.get(k, "joined")), J.u32(J.get(self, "elapsed"))))), self, ids)

def bannerpeak.session.Session.foes_valid?(+foes: J.Val, +n: U32, +wp: U32, +hp: U32) -> Bool:
  bannerpeak.session.Session.foes_valid.if2(U32.is_ge(n, J.len(foes)), foes, n, wp, hp)

def bannerpeak.session.Session.foes_valid.if1?(bpc: Bool, +foes: J.Val, +n: U32, +wp: U32, +hp: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.foes_valid(foes, (n + 1 : U32), wp, hp)

def bannerpeak.session.Session.foes_valid.if2?(bpc: Bool, +foes: J.Val, +n: U32, +wp: U32, +hp: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.session.Session.foes_valid.if1(Bool.not(bannerpeak.foes.Foe.valid(J.nth(foes, n), wp, hp)), foes, n, wp, hp)

def bannerpeak.session.Session.items_valid?(+items: J.Val, +n: U32, +wp: U32, +hp: U32) -> Bool:
  bannerpeak.session.Session.items_valid.if2(U32.is_ge(n, J.len(items)), items, n, wp, hp)

def bannerpeak.session.Session.items_valid.if1?(bpc: Bool, +items: J.Val, +n: U32, +wp: U32, +hp: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.items_valid(items, (n + 1 : U32), wp, hp)

def bannerpeak.session.Session.items_valid.if2?(bpc: Bool, +items: J.Val, +n: U32, +wp: U32, +hp: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.session.Session.items_valid.if1(Bool.not(bannerpeak.foes.Item.valid(J.nth(items, n), wp, hp)), items, n, wp, hp)

def bannerpeak.session.Session.bolts_valid?(+bolts: J.Val, +n: U32, +wp: U32, +hp: U32) -> Bool:
  bannerpeak.session.Session.bolts_valid.if2(U32.is_ge(n, J.len(bolts)), bolts, n, wp, hp)

def bannerpeak.session.Session.bolts_valid.if1?(bpc: Bool, +bolts: J.Val, +n: U32, +wp: U32, +hp: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.bolts_valid(bolts, (n + 1 : U32), wp, hp)

def bannerpeak.session.Session.bolts_valid.if2?(bpc: Bool, +bolts: J.Val, +n: U32, +wp: U32, +hp: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.session.Session.bolts_valid.if1(Bool.not(bannerpeak.foes.Bolt.valid(J.nth(bolts, n), wp, hp)), bolts, n, wp, hp)

def bannerpeak.session.Session.boulders_valid?(+bs: J.Val, +n: U32, +wp: U32) -> Bool:
  bannerpeak.session.Session.boulders_valid.if2(U32.is_ge(n, J.len(bs)), bs, n, wp)

def bannerpeak.session.Session.boulders_valid.if1?(bpc: Bool, +bs: J.Val, +n: U32, +wp: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.boulders_valid(bs, (n + 1 : U32), wp)

def bannerpeak.session.Session.boulders_valid.if2?(bpc: Bool, +bs: J.Val, +n: U32, +wp: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      +b : J.Val = J.nth(bs, n)
      bannerpeak.session.Session.boulders_valid.if1(Bool.not(Bool.and(Bool.and(RS.i_ge(J.u32(J.get(b, "x")), 0), RS.i_le(J.u32(J.get(b, "x")), (wp * 256 : U32))), U32.is_lt(J.u32(J.get(b, "t")), 16777216))), bs, n, wp)

def bannerpeak.session.Session.fx_valid?(+fx: J.Val, +n: U32) -> Bool:
  bannerpeak.session.Session.fx_valid.if2(U32.is_ge(n, J.len(fx)), fx, n)

def bannerpeak.session.Session.fx_valid.if1?(bpc: Bool, +fx: J.Val, +n: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.fx_valid(fx, (n + 1 : U32))

def bannerpeak.session.Session.fx_valid.if2?(bpc: Bool, +fx: J.Val, +n: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.session.Session.fx_valid.if1(U32.is_gt(J.u32(J.get(J.nth(fx, n), "t")), 14), fx, n)

def bannerpeak.session.Session.taken_valid?(+taken: J.Val, +n: U32, +coins: U32) -> Bool:
  bannerpeak.session.Session.taken_valid.if2(U32.is_ge(n, J.len(taken)), taken, n, coins)

def bannerpeak.session.Session.taken_valid.if1?(bpc: Bool, +taken: J.Val, +n: U32, +coins: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.taken_valid(taken, (n + 1 : U32), coins)

def bannerpeak.session.Session.taken_valid.if2?(bpc: Bool, +taken: J.Val, +n: U32, +coins: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.session.Session.taken_valid.if1(U32.is_ge(J.u32(J.nth(taken, n)), coins), taken, n, coins)

def bannerpeak.session.Session.changed_valid?(+self: J.Val, +ids: List<&2, U32>) -> Bool:
  bannerpeak.session.Session.changed_valid.if2(bannerpeak.u.is_empty(ids), self, ids)

def bannerpeak.session.Session.changed_valid.if1?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.changed_valid(self, bannerpeak.u.tail(ids))

def bannerpeak.session.Session.changed_valid.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      +key : U32 = bannerpeak.u.head(ids)
      +code : U32 = bannerpeak.u.at_u32(J.get(self, "changed"), key)
      bannerpeak.session.Session.changed_valid.if1(Bool.not(Bool.and(RS.i_lt(key, (434 * 64 : U32)), bannerpeak.world.Tile.known(code))), self, ids)

def bannerpeak.session.Session.valid?(+self: J.Val) -> Bool:
  +wp : U32 = (434 * 16 : U32)
  +hp : U32 = (64 * 16 : U32)
  +gates_len : U32 = bannerpeak.layout.info.gates_len()
  bannerpeak.session.Session.valid.if7(Bool.not(Bool.and(Bool.and(U32.is_le(bannerpeak.session.Session.seat_count(self), 6), U32.is_le(J.u32(J.get(self, "elapsed")), 108000)), bannerpeak.session.Session.seats_valid(self, bannerpeak.u.ids(J.get(self, "seats"))))), self, wp, hp, gates_len)

def bannerpeak.session.Session.valid.if1?(bpc: Bool, +self: J.Val) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +announce_ok : Bool = Bool.or(J.is_null(J.get(self, "announce")), Bool.and(U32.is_lt(J.u32(J.get(J.get(self, "announce"), "banner")), 8), U32.is_le(J.u32(J.get(J.get(self, "announce"), "t")), 110)))
      +completed_ok : Bool = Bool.or(J.is_null(J.get(self, "completed")), U32.is_le(J.u32(J.get(self, "completed")), J.u32(J.get(self, "elapsed"))))
      Bool.and(Bool.and(Bool.and(Bool.and(announce_ok, U32.is_le(J.u32(J.get(self, "wipe")), 20)), U32.is_le(J.u32(J.get(self, "scaled")), 6)), completed_ok), Bool.or(Bool.not(String.eq(J.str(J.get(self, "phase")), "Finale")), U32.is_eq(J.u32(J.get(self, "banners")), 255)))

def bannerpeak.session.Session.valid.if2?(bpc: Bool, +self: J.Val) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.valid.if1(Bool.not(Bool.and(U32.is_le(bannerpeak.u.map_len(J.get(self, "changed")), 2048), bannerpeak.session.Session.changed_valid(self, bannerpeak.u.ids(J.get(self, "changed"))))), self)

def bannerpeak.session.Session.valid.if3?(bpc: Bool, +self: J.Val) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +coins : U32 = bannerpeak.layout.info.coins_len()
      bannerpeak.session.Session.valid.if2(Bool.not(Bool.and(U32.is_le(J.len(J.get(self, "taken")), coins), bannerpeak.session.Session.taken_valid(J.get(self, "taken"), 0, coins))), self)

def bannerpeak.session.Session.valid.if4?(bpc: Bool, +self: J.Val) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.valid.if3(Bool.not(Bool.and(Bool.and(bannerpeak.boss.Boss.valid(J.get(self, "boss"), 6), U32.is_le(J.len(J.get(self, "fx")), 48)), bannerpeak.session.Session.fx_valid(J.get(self, "fx"), 0))), self)

def bannerpeak.session.Session.valid.if5?(bpc: Bool, +self: J.Val, +wp: U32, +hp: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.valid.if4(Bool.not(Bool.and(Bool.and(Bool.and(U32.is_le(J.len(J.get(self, "bolts")), 16), bannerpeak.session.Session.bolts_valid(J.get(self, "bolts"), 0, wp, hp)), U32.is_le(J.len(J.get(self, "boulders")), 8)), bannerpeak.session.Session.boulders_valid(J.get(self, "boulders"), 0, wp))), self)

def bannerpeak.session.Session.valid.if6?(bpc: Bool, +self: J.Val, +wp: U32, +hp: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.valid.if5(Bool.not(Bool.and(Bool.and(Bool.and(U32.is_le(J.len(J.get(self, "foes")), 128), bannerpeak.session.Session.foes_valid(J.get(self, "foes"), 0, wp, hp)), U32.is_le(J.len(J.get(self, "items")), 48)), bannerpeak.session.Session.items_valid(J.get(self, "items"), 0, wp, hp))), self, wp, hp)

def bannerpeak.session.Session.valid.if7?(bpc: Bool, +self: J.Val, +wp: U32, +hp: U32, +gates_len: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.valid.if6(Bool.not(Bool.and(Bool.and(Bool.and(Bool.and(U32.is_le(J.u32(J.get(self, "checkpoint")), gates_len), U32.is_lt(J.u32(J.get(self, "gates")), RS.u_shl(1, gates_len))), U32.is_le(J.u32(J.get(self, "doors")), 3)), U32.is_le(J.u32(J.get(self, "phase_t")), 108000)), U32.is_lt(J.u32(J.get(self, "cursor")), 6))), self, wp, hp)

def bannerpeak.session.Session.count?(+self: J.Val) -> U32:
  RS.u_count_ones(J.u32(J.get(self, "banners")))

def bannerpeak.session.Session.door_count?(+self: J.Val) -> U32:
  RS.u_count_ones(U32.and(J.u32(J.get(self, "banners")), 127))

def bannerpeak.session.Session.alive?(+self: J.Val) -> J.Val:
  bannerpeak.session.Session.alive_from(self, bannerpeak.u.ids(J.get(self, "seats")), bannerpeak.u.vec_new())

def bannerpeak.session.Session.alive_from?(+self: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.alive_from.if2(bannerpeak.u.is_empty(ids), self, ids, acc)

def bannerpeak.session.Session.alive_from.if1?(bpc: Bool, +acc: J.Val, +id: U32) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.JU{id})
      acc
    case False{}:
      acc

def bannerpeak.session.Session.alive_from.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +acc = bannerpeak.session.Session.alive_from.if1(bannerpeak.knight.Knight.alive(bannerpeak.u.at(J.get(self, "seats"), id)), acc, id)
      bannerpeak.session.Session.alive_from(self, bannerpeak.u.tail(ids), acc)

def bannerpeak.session.Session.spark_gate?(+self: J.Val) -> U32:
  +n : U32 = bannerpeak.session.Session.first_missing(J.u32(J.get(self, "sparks")), 0)
  bannerpeak.session.Session.spark_gate.if4(U32.is_ge(n, bannerpeak.layout.info.sparks_len()), n)

def bannerpeak.session.Session.spark_gate.if1?(bpc: Bool) -> U32:
  match bpc:
    case True{}:
      3
    case False{}:
      5

def bannerpeak.session.Session.spark_gate.if2?(bpc: Bool, +section: U32) -> U32:
  match bpc:
    case True{}:
      2
    case False{}:
      bannerpeak.session.Session.spark_gate.if1(U32.is_eq(section, 3))

def bannerpeak.session.Session.spark_gate.if3?(bpc: Bool, +section: U32) -> U32:
  match bpc:
    case True{}:
      0
    case False{}:
      bannerpeak.session.Session.spark_gate.if2(U32.is_eq(section, 2), section)

def bannerpeak.session.Session.spark_gate.if4?(bpc: Bool, +n: U32) -> U32:
  match bpc:
    case True{}:
      0
    case False{}:
      +section : U32 = bannerpeak.layout.section_of(bannerpeak.layout.info.spark_x(n))
      bannerpeak.session.Session.spark_gate.if3(U32.is_eq(section, 1), section)

def bannerpeak.session.Session.first_missing?(+sparks: U32, +n: U32) -> U32:
  bannerpeak.session.Session.first_missing.if2(U32.is_ge(n, bannerpeak.layout.info.sparks_len()), sparks, n)

def bannerpeak.session.Session.first_missing.if1?(bpc: Bool, +sparks: U32, +n: U32) -> U32:
  match bpc:
    case True{}:
      n
    case False{}:
      bannerpeak.session.Session.first_missing(sparks, (n + 1 : U32))

def bannerpeak.session.Session.first_missing.if2?(bpc: Bool, +sparks: U32, +n: U32) -> U32:
  match bpc:
    case True{}:
      n
    case False{}:
      bannerpeak.session.Session.first_missing.if1(U32.is_eq(U32.and(sparks, RS.u_shl(1, n)), 0), sparks, n)

def bannerpeak.session.Session.crown_down?(+self: J.Val) -> Bool:
  bannerpeak.boss.Boss.defeated(J.get(self, "boss"))

def bannerpeak.session.Session.step?(+self: J.Val, +inputs: J.Val, +heroes: J.Val, +knobs: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +self = J.set(self, "elapsed", J.JU{RS.u_min((J.u32(J.get(self, "elapsed")) + 1 : U32), 108000)})
  +_t1 = bannerpeak.session.Session.retain_awake(self, heroes, bannerpeak.u.ids(J.get(self, "seats")))
  +self : J.Val = _t1
  +_t2 = bannerpeak.session.Session.seat_late(self, heroes, bannerpeak.u.ids(heroes), say)
  +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t2)
  +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t2)
  +self = bannerpeak.session.Session.step.if1(U32.is_gt(bannerpeak.session.Session.seat_count(self), J.u32(J.get(self, "scaled"))), self)
  +self = J.set(self, "fx", bannerpeak.session.Session.fx_tick(J.get(self, "fx"), 0, bannerpeak.u.vec_new()))
  +self = bannerpeak.session.Session.step.if3(Bool.not(J.is_null(J.get(self, "announce"))), self)
  bannerpeak.session.Session.step.if9(String.eq(J.str(J.get(self, "phase")), "Select"), self, inputs, knobs, say)

def bannerpeak.session.Session.step.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t3 = bannerpeak.session.Session.provision(self)
      +self : J.Val = _t3
      self
    case False{}:
      self

def bannerpeak.session.Session.step.if2?(bpc: Bool, +self: J.Val, +a: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "announce", J.JNull{})
      self
    case False{}:
      +self = J.set(self, "announce", a)
      self

def bannerpeak.session.Session.step.if3?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +a : J.Val = J.get(self, "announce")
      +a = J.set(a, "t", J.JU{RS.u_sat_sub(J.u32(J.get(a, "t")), 1)})
      +self = bannerpeak.session.Session.step.if2(U32.is_eq(J.u32(J.get(a, "t")), 0), self, a)
      self
    case False{}:
      self

def bannerpeak.session.Session.step.if4?(bpc: Bool, +self: J.Val, +now: J.Val, +before: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +now = J.push(now, J.set(J.set(J.obj(), "id", J.JU{300}), "p", bannerpeak.boss.Boss.plank(J.get(self, "boss"), False{})))
      +before = J.push(before, J.set(J.set(J.obj(), "id", J.JU{300}), "p", bannerpeak.boss.Boss.plank(J.get(self, "boss"), True{})))
      (before, now)
    case False{}:
      (before, now)

def bannerpeak.session.Session.step.if5?(bpc: Bool, +self: J.Val, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "wipe", J.JU{0})
      +_t12 = bannerpeak.session.Session.retry(self, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t12)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t12)
      self
    case False{}:
      self

def bannerpeak.session.Session.step.if6?(bpc: Bool, +self: J.Val, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "wipe", J.JU{(J.u32(J.get(self, "wipe")) + 1 : U32)})
      +self = bannerpeak.session.Session.step.if5(U32.is_ge(J.u32(J.get(self, "wipe")), 20), self, say)
      self
    case False{}:
      +self = J.set(self, "wipe", J.JU{0})
      self

def bannerpeak.session.Session.step.if7?(bpc: Bool, +self: J.Val, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t13 = bannerpeak.session.Session.start_finale(self, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t13)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t13)
      self
    case False{}:
      self

def bannerpeak.session.Session.step.if8?(bpc: Bool, +self: J.Val, +inputs: J.Val, +knobs: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +_t5 = bannerpeak.session.Session.finale_step(self, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t5)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t5)
      (self, say)
    case False{}:
      +now : J.Val = bannerpeak.layout.movers(J.u32(J.get(self, "elapsed")))
      +before : J.Val = bannerpeak.layout.movers(RS.u_sat_sub(J.u32(J.get(self, "elapsed")), 1))
      +_t6 = bannerpeak.session.Session.step.if4(Bool.not(bannerpeak.boss.Boss.defeated(J.get(self, "boss"))), self, now, before)
      +before = bannerpeak.u.fst(J.Val, J.Val, _t6)
      +now = bannerpeak.u.snd(J.Val, J.Val, _t6)
      +ids : List<&2, U32> = bannerpeak.u.ids(J.get(self, "seats"))
      +shields : J.Val = bannerpeak.session.Session.shields(self, ids, bannerpeak.u.vec_new())
      +_t7 = bannerpeak.session.Session.step_seats(self, ids, inputs, knobs, now, before, shields, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t7)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t7)
      +_t8 = bannerpeak.session.Session.step_things(self, knobs, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t8)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t8)
      +_t9 = bannerpeak.session.Session.step_rescue(self, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t9)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t9)
      +_t10 = bannerpeak.session.Session.step_doors(self, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t10)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t10)
      +_t11 = bannerpeak.session.Session.cameras(self, ids)
      +self : J.Val = _t11
      +self = bannerpeak.session.Session.step.if6(Bool.and(U32.is_gt(bannerpeak.session.Session.seat_count(self), 0), bannerpeak.session.Session.all_down(self, ids)), self, say)
      +self = bannerpeak.session.Session.step.if7(Bool.and(Bool.and(U32.is_eq(J.u32(J.get(self, "banners")), 255), String.eq(J.str(J.get(self, "phase")), "Play")), J.is_null(J.get(self, "announce"))), self, say)
      (self, say)

def bannerpeak.session.Session.step.if9?(bpc: Bool, +self: J.Val, +inputs: J.Val, +knobs: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +_t4 = bannerpeak.session.Session.select_step(self, inputs, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t4)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t4)
      (self, say)
    case False{}:
      bannerpeak.session.Session.step.if8(String.eq(J.str(J.get(self, "phase")), "Finale"), self, inputs, knobs, say)

def bannerpeak.session.Session.retain_awake?(+self: J.Val, +heroes: J.Val, +ids: List<&2, U32>) -> J.Val:
  bannerpeak.session.Session.retain_awake.if3(bannerpeak.u.is_empty(ids), self, heroes, ids)

def bannerpeak.session.Session.retain_awake.if1?(bpc: Bool, +heroes: J.Val, +id: U32, +keep: Bool) -> Bool:
  match bpc:
    case True{}:
      +h : J.Val = bannerpeak.u.at(heroes, id)
      +keep : Bool = Bool.and(Bool.not(J.bool(J.get(h, "away"))), Bool.not(J.bool(J.get(h, "stone"))))
      keep
    case False{}:
      keep

def bannerpeak.session.Session.retain_awake.if2?(bpc: Bool, +self: J.Val, +id: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "seats", bannerpeak.u.del(J.get(self, "seats"), id))
      self
    case False{}:
      self

def bannerpeak.session.Session.retain_awake.if3?(bpc: Bool, +self: J.Val, +heroes: J.Val, +ids: List<&2, U32>) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +keep : Bool = False{}
      +keep = bannerpeak.session.Session.retain_awake.if1(bannerpeak.u.has(heroes, id), heroes, id, keep)
      +self = bannerpeak.session.Session.retain_awake.if2(Bool.not(keep), self, id)
      +_t1 = bannerpeak.session.Session.retain_awake(self, heroes, bannerpeak.u.tail(ids))
      +self : J.Val = _t1
      self

def bannerpeak.session.Session.seat_late?(+self: J.Val, +heroes: J.Val, +ids: List<&2, U32>, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  bannerpeak.session.Session.seat_late.if2(bannerpeak.u.is_empty(ids), self, heroes, ids, say)

def bannerpeak.session.Session.seat_late.if1?(bpc: Bool, +self: J.Val, +say: J.Val, +id: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +at : U32 = J.u32(J.get(self, "elapsed"))
      +_t1 = bannerpeak.session.Session.seat(self, id, at)
      +self : J.Val = _t1
      +say = J.push(say, J.JS{"door_pass"})
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.seat_late.if2?(bpc: Bool, +self: J.Val, +heroes: J.Val, +ids: List<&2, U32>, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      (self, say)
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +h : J.Val = bannerpeak.u.at(heroes, id)
      +_t2 = bannerpeak.session.Session.seat_late.if1(Bool.and(Bool.and(Bool.not(J.bool(J.get(h, "away"))), Bool.not(J.bool(J.get(h, "stone")))), Bool.not(bannerpeak.u.has(J.get(self, "seats"), id))), self, say, id)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t2)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t2)
      +_t3 = bannerpeak.session.Session.seat_late(self, heroes, bannerpeak.u.tail(ids), say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t3)
      (self, say)

def bannerpeak.session.Session.fx_tick?(+fx: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.fx_tick.if2(U32.is_ge(n, J.len(fx)), fx, n, acc)

def bannerpeak.session.Session.fx_tick.if1?(bpc: Bool, +acc: J.Val, +f: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, f)
      acc
    case False{}:
      acc

def bannerpeak.session.Session.fx_tick.if2?(bpc: Bool, +fx: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +f : J.Val = J.nth(fx, n)
      +f = J.set(f, "t", J.JU{RS.u_sat_sub(J.u32(J.get(f, "t")), 1)})
      +acc = bannerpeak.session.Session.fx_tick.if1(U32.is_gt(J.u32(J.get(f, "t")), 0), acc, f)
      bannerpeak.session.Session.fx_tick(fx, (n + 1 : U32), acc)

def bannerpeak.session.Session.input_of?(+inputs: J.Val, +id: U32) -> J.Val:
  bannerpeak.session.Session.input_of.if2(bannerpeak.u.has(inputs, id), inputs, id)

def bannerpeak.session.Session.input_of.if1?(bpc: Bool, +i: J.Val) -> J.Val:
  match bpc:
    case True{}:
      i
    case False{}:
      bannerpeak.u.input_default()

def bannerpeak.session.Session.input_of.if2?(bpc: Bool, +inputs: J.Val, +id: U32) -> J.Val:
  match bpc:
    case True{}:
      +i : J.Val = bannerpeak.u.at(inputs, id)
      bannerpeak.session.Session.input_of.if1(rules.Input.valid(i), i)
    case False{}:
      bannerpeak.u.input_default()

def bannerpeak.session.Session.shields?(+self: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.shields.if2(bannerpeak.u.is_empty(ids), self, ids, acc)

def bannerpeak.session.Session.shields.if1?(bpc: Bool, +acc: J.Val, +sh: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, sh)
      acc
    case False{}:
      acc

def bannerpeak.session.Session.shields.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +sh : J.Val = bannerpeak.body.Body.shield(J.get(bannerpeak.u.at(J.get(self, "seats"), id), "body"), (1000 + id : U32))
      +acc = bannerpeak.session.Session.shields.if1(Bool.not(J.is_null(sh)), acc, sh)
      bannerpeak.session.Session.shields(self, bannerpeak.u.tail(ids), acc)

def bannerpeak.session.Session.with_shields?(+ms: J.Val, +shields: J.Val, +id: U32, +n: U32) -> J.Val:
  bannerpeak.session.Session.with_shields.if2(U32.is_ge(n, J.len(shields)), ms, shields, id, n)

def bannerpeak.session.Session.with_shields.if1?(bpc: Bool, +ms: J.Val, +s: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +ms = J.push(ms, s)
      ms
    case False{}:
      ms

def bannerpeak.session.Session.with_shields.if2?(bpc: Bool, +ms: J.Val, +shields: J.Val, +id: U32, +n: U32) -> J.Val:
  match bpc:
    case True{}:
      ms
    case False{}:
      +s : J.Val = J.nth(shields, n)
      +ms = bannerpeak.session.Session.with_shields.if1(U32.is_ne(J.u32(J.get(s, "id")), (1000 + id : U32)), ms, s)
      bannerpeak.session.Session.with_shields(ms, shields, id, (n + 1 : U32))

def bannerpeak.session.Session.step_seats?(+self: J.Val, +ids: List<&2, U32>, +inputs: J.Val, +knobs: J.Val, +now: J.Val, +before: J.Val, +shields: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  bannerpeak.session.Session.step_seats.if1(bannerpeak.u.is_empty(ids), self, ids, inputs, knobs, now, before, shields, say)

def bannerpeak.session.Session.step_seats.if1?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +inputs: J.Val, +knobs: J.Val, +now: J.Val, +before: J.Val, +shields: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      (self, say)
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +self = J.set(self, "seats", bannerpeak.u.del(J.get(self, "seats"), id))
      +pn : J.Val = bannerpeak.session.Session.with_shields(now, shields, id, 0)
      +pb : J.Val = bannerpeak.session.Session.with_shields(before, shields, id, 0)
      +_t1 = bannerpeak.session.Session.step_knight(self, id, k, bannerpeak.session.Session.input_of(inputs, id), knobs, pn, pb, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), id, k))
      +_t2 = bannerpeak.session.Session.step_seats(self, bannerpeak.u.tail(ids), inputs, knobs, now, before, shields, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t2)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t2)
      (self, say)

def bannerpeak.session.Session.cameras?(+self: J.Val, +ids: List<&2, U32>) -> J.Val:
  bannerpeak.session.Session.cameras.if3(bannerpeak.u.is_empty(ids), self, ids)

def bannerpeak.session.Session.cameras.if1?(bpc: Bool, +k: J.Val, +x: U32, +y: U32, +face: U32, +vx: U32, +vy: U32) -> Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>>:
  match bpc:
    case True{}:
      +b : J.Val = J.get(k, "bubble")
      +x : U32 = RS.i_div_euclid(J.u32(J.get(b, "x")), 256)
      +y : U32 = (RS.i_div_euclid(J.u32(J.get(b, "y")), 256) + 10 : U32)
      +face : U32 = 1
      +vx : U32 = 0
      +vy : U32 = 0
      (face, (vx, (vy, (x, y))))
    case False{}:
      (face, (vx, (vy, (x, y))))

def bannerpeak.session.Session.cameras.if2?(bpc: Bool, +k: J.Val, +x: U32, +y: U32, +face: U32, +vx: U32, +vy: U32) -> J.Val:
  match bpc:
    case True{}:
      +_j3 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.knight.follow((J.u32(J.nth(J.get(k, "cam"), 0)), J.u32(J.nth(J.get(k, "cam"), 1))), x, y, face, vx, vy)
      +k = J.set(k, "cam", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j3)), J.of_i32(bannerpeak.u.snd(U32, U32, _j3))]})
      k
    case False{}:
      k

def bannerpeak.session.Session.cameras.if3?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
      +fx = bannerpeak.u.fst(U32, U32, _t1)
      +fy = bannerpeak.u.snd(U32, U32, _t1)
      +x : U32 = fx
      +y : U32 = fy
      +face : U32 = J.u32(J.get(J.get(k, "body"), "face"))
      +vx : U32 = J.u32(J.get(J.get(k, "body"), "vx"))
      +vy : U32 = J.u32(J.get(J.get(k, "body"), "vy"))
      +_t2 = bannerpeak.session.Session.cameras.if1(Bool.not(J.is_null(J.get(k, "bubble"))), k, x, y, face, vx, vy)
      +face = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, _t2)
      +vx = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, _t2))
      +vy = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, _t2)))
      +x = bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, _t2))))
      +y = bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, _t2))))
      +k = bannerpeak.session.Session.cameras.if2(U32.is_eq(J.u32(J.get(k, "pit")), 0), k, x, y, face, vx, vy)
      +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), id, k))
      +_t4 = bannerpeak.session.Session.cameras(self, bannerpeak.u.tail(ids))
      +self : J.Val = _t4
      self

def bannerpeak.session.Session.all_down?(+self: J.Val, +ids: List<&2, U32>) -> Bool:
  bannerpeak.session.Session.all_down.if2(bannerpeak.u.is_empty(ids), self, ids)

def bannerpeak.session.Session.all_down.if1?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      bannerpeak.session.Session.all_down(self, bannerpeak.u.tail(ids))

def bannerpeak.session.Session.all_down.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.session.Session.all_down.if1(bannerpeak.knight.Knight.alive(bannerpeak.u.at(J.get(self, "seats"), bannerpeak.u.head(ids))), self, ids)

def bannerpeak.session.Session.retry?(+self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.session.Session.station(self)
  +x = bannerpeak.u.fst(U32, U32, _t1)
  +y = bannerpeak.u.snd(U32, U32, _t1)
  +_t2 = bannerpeak.session.Session.retry_seats(self, bannerpeak.u.ids(J.get(self, "seats")), 0, x, y)
  +self : J.Val = _t2
  +self = J.set(self, "boulders", bannerpeak.u.vec_new())
  +self = J.set(self, "bolts", bannerpeak.u.vec_new())
  +say = J.push(say, J.JS{"door_open"})
  (self, say)

def bannerpeak.session.Session.offset?(+n: U32) -> U32:
  +sgn : U32 = 4294967295
  +sgn = bannerpeak.session.Session.offset.if1(U32.is_eq(RS.i_rem(n, 2), 0), sgn)
  ((RS.i_div((n + 1 : U32), 2) * 7 : U32) * sgn : U32)

def bannerpeak.session.Session.offset.if1?(bpc: Bool, +sgn: U32) -> U32:
  match bpc:
    case True{}:
      +sgn : U32 = 1
      sgn
    case False{}:
      sgn

def bannerpeak.session.Session.retry_seats?(+self: J.Val, +ids: List<&2, U32>, +n: U32, +x: U32, +y: U32) -> J.Val:
  bannerpeak.session.Session.retry_seats.if1(bannerpeak.u.is_empty(ids), self, ids, n, x, y)

def bannerpeak.session.Session.retry_seats.if1?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +n: U32, +x: U32, +y: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +off : U32 = bannerpeak.session.Session.offset(n)
      +old : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +pips : U32 = RS.u_max(J.u32(J.get(old, "pips")), 4)
      +k : J.Val = bannerpeak.knight.Knight.at(J.u32(J.get(old, "joined")), (x + off : U32), y)
      +k = J.set(k, "coins", J.JU{J.u32(J.get(old, "coins"))})
      +k = J.set(k, "score", J.JU{J.u32(J.get(old, "score"))})
      +k = J.set(k, "pips", J.JU{pips})
      +k = J.set(k, "hurt", J.JU{70})
      +_j1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.knight.follow((J.u32(J.nth(J.get(k, "cam"), 0)), J.u32(J.nth(J.get(k, "cam"), 1))), x, y, 1, 0, 0)
      +k = J.set(k, "cam", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j1)), J.of_i32(bannerpeak.u.snd(U32, U32, _j1))]})
      +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), id, k))
      +_t2 = bannerpeak.session.Session.retry_seats(self, bannerpeak.u.tail(ids), (n + 1 : U32), x, y)
      +self : J.Val = _t2
      self

def bannerpeak.session.Session.step_knight?(+self: J.Val, +id: U32, +k: J.Val, +input: J.Val, +knobs: J.Val, +now: J.Val, +before: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +k = J.set(k, "hurt", J.JU{RS.u_sat_sub(J.u32(J.get(k, "hurt")), 1)})
  +k = J.set(k, "cooldown", J.JU{RS.u_sat_sub(J.u32(J.get(k, "cooldown")), 1)})
  +k = J.set(k, "shock", J.JU{RS.u_sat_sub(J.u32(J.get(k, "shock")), 1)})
  +k = J.set(k, "landed", J.JU{RS.u_sat_add(J.u32(J.get(k, "landed")), 1)})
  bannerpeak.session.Session.step_knight.if8(Bool.not(J.is_null(J.get(k, "bubble"))), self, id, k, input, knobs, now, before, say)

def bannerpeak.session.Session.step_knight.if1?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val, +b: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.session.Session.station(self)
      +x = bannerpeak.u.fst(U32, U32, _t1)
      +y = bannerpeak.u.snd(U32, U32, _t1)
      +joined : U32 = J.u32(J.get(k, "joined"))
      +coins : U32 = J.u32(J.get(k, "coins"))
      +score : U32 = J.u32(J.get(k, "score"))
      +k : J.Val = bannerpeak.knight.Knight.at(joined, x, y)
      +k = J.set(k, "coins", J.JU{coins})
      +k = J.set(k, "score", J.JU{score})
      +k = J.set(k, "pips", J.JU{3})
      +k = J.set(k, "hurt", J.JU{70})
      +_j2 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.knight.follow((J.u32(J.nth(J.get(k, "cam"), 0)), J.u32(J.nth(J.get(k, "cam"), 1))), x, y, 1, 0, 0)
      +k = J.set(k, "cam", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j2)), J.of_i32(bannerpeak.u.snd(U32, U32, _j2))]})
      +say = J.push(say, J.JS{"door_pass"})
      (k, say)
    case False{}:
      +k = J.set(k, "bubble", b)
      (k, say)

def bannerpeak.session.Session.step_knight.if2?(bpc: Bool, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t5 : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.get(k, "safe"), 0)), J.u32(J.nth(J.get(k, "safe"), 1)))
      +x = bannerpeak.u.fst(U32, U32, _t5)
      +y = bannerpeak.u.snd(U32, U32, _t5)
      +_t6 = bannerpeak.body.Body.place(J.get(k, "body"), x, y)
      +k = J.set(k, "body", _t6)
      +k = J.set(k, "hurt", J.JU{70})
      +_j7 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.knight.follow((J.u32(J.nth(J.get(k, "cam"), 0)), J.u32(J.nth(J.get(k, "cam"), 1))), x, y, J.u32(J.get(J.get(k, "body"), "face")), 0, 0)
      +k = J.set(k, "cam", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j7)), J.of_i32(bannerpeak.u.snd(U32, U32, _j7))]})
      k
    case False{}:
      k

def bannerpeak.session.Session.step_knight.if3?(bpc: Bool, +k: J.Val, +cmd: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +k = J.set(k, "pose", J.JU{(J.u32(J.get(k, "pose")) - 1 : U32)})
      +cmd : J.Val = bannerpeak.session.Cmd.default()
      (cmd, k)
    case False{}:
      (cmd, k)

def bannerpeak.session.Session.step_knight.if4?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +say: J.Val, +bits: U32, +was: U32) -> J.Val:
  match bpc:
    case True{}:
      +_t14 = bannerpeak.session.Session.touch(self, id, k, bits, was, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t14)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t14))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t14))
      self
    case False{}:
      self

def bannerpeak.session.Session.step_knight.if5?(bpc: Bool, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "chain", J.JU{0})
      k
    case False{}:
      k

def bannerpeak.session.Session.step_knight.if6?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +input: J.Val, +knobs: J.Val, +now: J.Val, +before: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t9 = bannerpeak.session.Session.step_pole(self, k, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t9)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t9))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t9))
      (self, (k, say))
    case False{}:
      +_t10 : Sigma<&2, &2, J.Val, _ => U32> = bannerpeak.knight.Knight.cmd(k, input)
      +cmd0 = bannerpeak.u.fst(J.Val, U32, _t10)
      +bits = bannerpeak.u.snd(J.Val, U32, _t10)
      +cmd : J.Val = cmd0
      +_t11 = bannerpeak.session.Session.step_knight.if3(U32.is_gt(J.u32(J.get(k, "pose")), 0), k, cmd)
      +cmd = bannerpeak.u.fst(J.Val, J.Val, _t11)
      +k = bannerpeak.u.snd(J.Val, J.Val, _t11)
      +k = J.set(k, "body", J.set(J.get(k, "body"), "big", J.JBool{bannerpeak.knight.Knight.big(k)}))
      +was : U32 = J.u32(J.get(J.get(k, "body"), "y"))
      +evs : J.Val = bannerpeak.u.vec_new()
      +_t12 = bannerpeak.body.Body.step(J.get(k, "body"), cmd, knobs, J.get(self, "changed"), now, before, evs)
      +k = J.set(k, "body", bannerpeak.u.fst(J.Val, J.Val, _t12))
      +evs : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t12)
      +_t13 = bannerpeak.session.Session.body_events(self, id, k, evs, 0, knobs, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t13)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t13))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t13))
      +self = bannerpeak.session.Session.step_knight.if4(U32.is_eq(J.u32(J.get(k, "pose")), 0), self, id, k, say, bits, was)
      +k = bannerpeak.session.Session.step_knight.if5(J.bool(J.get(J.get(k, "body"), "ground")), k)
      +_t15 = bannerpeak.session.Session.note_safe(self, k)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t15)
      +k : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t15)
      +k = J.set(k, "prev", J.JU{bits})
      (self, (k, say))

def bannerpeak.session.Session.step_knight.if7?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +input: J.Val, +knobs: J.Val, +now: J.Val, +before: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +k = J.set(k, "pit", J.JU{(J.u32(J.get(k, "pit")) - 1 : U32)})
      +k = bannerpeak.session.Session.step_knight.if2(U32.is_eq(J.u32(J.get(k, "pit")), 0), k)
      +_t8 : Sigma<&2, &2, J.Val, _ => U32> = bannerpeak.knight.Knight.cmd(k, input)
      +c1 = bannerpeak.u.fst(J.Val, U32, _t8)
      +bits1 = bannerpeak.u.snd(J.Val, U32, _t8)
      +k = J.set(k, "prev", J.JU{bits1})
      (self, (k, say))
    case False{}:
      bannerpeak.session.Session.step_knight.if6(U32.is_gt(J.u32(J.get(k, "pole")), 0), self, id, k, input, knobs, now, before, say)

def bannerpeak.session.Session.step_knight.if8?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +input: J.Val, +knobs: J.Val, +now: J.Val, +before: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +b : J.Val = J.get(k, "bubble")
      +b = J.set(b, "t", J.JU{(J.u32(J.get(b, "t")) + 1 : U32)})
      +dx : U32 = J.u32(J.get(input, "move_x"))
      +dy : U32 = J.u32(J.get(input, "move_y"))
      +b = J.set(b, "x", J.of_i32((J.u32(J.get(b, "x")) + (dx * 190 : U32) : U32)))
      +b = J.set(b, "y", J.of_i32((J.u32(J.get(b, "y")) + ((dy * 150 : U32) - 40 : U32) : U32)))
      +b = J.set(b, "x", J.of_i32(RS.i_clamp(J.u32(J.get(b, "x")), RS.i_div((8 * 4096 : U32), 16), (((434 * 16 : U32) - 8 : U32) * 256 : U32))))
      +b = J.set(b, "y", J.of_i32(RS.i_clamp(J.u32(J.get(b, "y")), 0, ((64 * 16 : U32) * 256 : U32))))
      +_t3 = bannerpeak.session.Session.step_knight.if1(U32.is_ge(J.u32(J.get(b, "t")), 330), self, k, say, b)
      +k = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +say = bannerpeak.u.snd(J.Val, J.Val, _t3)
      +_t4 : Sigma<&2, &2, J.Val, _ => U32> = bannerpeak.knight.Knight.cmd(k, input)
      +c0 = bannerpeak.u.fst(J.Val, U32, _t4)
      +bits0 = bannerpeak.u.snd(J.Val, U32, _t4)
      +k = J.set(k, "prev", J.JU{bits0})
      (self, (k, say))
    case False{}:
      bannerpeak.session.Session.step_knight.if7(U32.is_gt(J.u32(J.get(k, "pit")), 0), self, id, k, input, knobs, now, before, say)

def bannerpeak.session.Session.body_events?(+self: J.Val, +id: U32, +k: J.Val, +evs: J.Val, +n: U32, +knobs: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  bannerpeak.session.Session.body_events.if1(U32.is_ge(n, J.len(evs)), self, id, k, evs, n, knobs, say)

def bannerpeak.session.Session.body_events.if1?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +evs: J.Val, +n: U32, +knobs: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (self, (k, say))
    case False{}:
      +_t1 = bannerpeak.session.Session.body_event(self, id, k, J.nth(evs, n), knobs, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      +_t2 = bannerpeak.session.Session.body_events(self, id, k, evs, (n + 1 : U32), knobs, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      (self, (k, say))

def bannerpeak.session.Session.note_safe?(+self: J.Val, +k: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +b : J.Val = J.get(k, "body")
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(b)
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +g : J.Val = bannerpeak.session.Session.view(self)
  +ok : Bool = Bool.and(Bool.and(Bool.and(Bool.and(Bool.and(J.bool(J.get(b, "ground")), J.is_null(J.get(b, "plank"))), String.eq(J.str(J.get(b, "mode")), "Ground")), bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, RS.i_div_euclid((fx - 7 : U32), 16), RS.i_div_euclid(fy, 16)))), bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, RS.i_div_euclid((fx + 7 : U32), 16), RS.i_div_euclid(fy, 16)))), U32.is_ne(bannerpeak.session.View.tile(g, RS.i_div_euclid(fx, 16), RS.i_div_euclid((fy - 1 : U32), 16)), 10))
  +k = bannerpeak.session.Session.note_safe.if2(ok, k, fx, fy)
  (self, k)

def bannerpeak.session.Session.note_safe.if1?(bpc: Bool, +k: J.Val, +fx: U32, +fy: U32) -> J.Val:
  match bpc:
    case True{}:
      +_j2 : Sigma<&2, &2, U32, _ => U32> = (fx, fy)
      +k = J.set(k, "safe", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j2)), J.of_i32(bannerpeak.u.snd(U32, U32, _j2))]})
      k
    case False{}:
      k

def bannerpeak.session.Session.note_safe.if2?(bpc: Bool, +k: J.Val, +fx: U32, +fy: U32) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "safe_t", J.JU{(J.u32(J.get(k, "safe_t")) + 1 : U32)})
      +k = bannerpeak.session.Session.note_safe.if1(U32.is_ge(J.u32(J.get(k, "safe_t")), 8), k, fx, fy)
      k
    case False{}:
      +k = J.set(k, "safe_t", J.JU{0})
      k

def bannerpeak.session.Session.body_event?(+self: J.Val, +id: U32, +k: J.Val, +e: J.Val, +knobs: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +_t16 = bannerpeak.session.Session.body_event.if12(String.eq(J.str(J.get(e, "k")), "Jump"), self, id, k, e, knobs, say, fx, fy)
  +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t16)
  +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t16))
  +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t16))
  (self, (k, say))

def bannerpeak.session.Session.body_event.if1?(bpc: Bool, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"shield_up"})
      say
    case False{}:
      say

def bannerpeak.session.Session.body_event.if2?(bpc: Bool, +e: J.Val, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"roll"})
      say
    case False{}:
      +say = bannerpeak.session.Session.body_event.if1(String.eq(J.str(J.get(e, "s")), "Spring"), say)
      say

def bannerpeak.session.Session.body_event.if3?(bpc: Bool, +e: J.Val, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"spell_cast"})
      say
    case False{}:
      +say = bannerpeak.session.Session.body_event.if2(Bool.or(Bool.or(String.eq(J.str(J.get(e, "s")), "Long"), String.eq(J.str(J.get(e, "s")), "Back")), String.eq(J.str(J.get(e, "s")), "Kick")), e, say)
      say

def bannerpeak.session.Session.body_event.if4?(bpc: Bool, +e: J.Val, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"swing"})
      say
    case False{}:
      +say = bannerpeak.session.Session.body_event.if3(String.eq(J.str(J.get(e, "s")), "Triple"), e, say)
      say

def bannerpeak.session.Session.body_event.if5?(bpc: Bool, +self: J.Val, +say: J.Val, +fx: U32, +fy: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +_t2 = bannerpeak.session.Session.fx(self, "Dust", fx, fy)
      +self : J.Val = _t2
      +say = J.push(say, J.JS{"step_stone"})
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.body_event.if6?(bpc: Bool, +k: J.Val, +say: J.Val, +to: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +x : U32 = bannerpeak.layout.cell_feet_x(J.u32(J.get(to, "x")))
      +y : U32 = bannerpeak.layout.cell_feet_y(J.u32(J.get(to, "y")))
      +_t5 = bannerpeak.body.Body.place(J.get(k, "body"), x, y)
      +k = J.set(k, "body", _t5)
      +_j6 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.knight.follow((J.u32(J.nth(J.get(k, "cam"), 0)), J.u32(J.nth(J.get(k, "cam"), 1))), x, y, J.u32(J.get(J.get(k, "body"), "face")), 0, 0)
      +k = J.set(k, "cam", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j6)), J.of_i32(bannerpeak.u.snd(U32, U32, _j6))]})
      +_j7 : Sigma<&2, &2, U32, _ => U32> = (x, y)
      +k = J.set(k, "safe", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j7)), J.of_i32(bannerpeak.u.snd(U32, U32, _j7))]})
      +say = J.push(say, J.JS{"door_pass"})
      (k, say)
    case False{}:
      (k, say)

def bannerpeak.session.Session.body_event.if7?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +e: J.Val, +knobs: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +k = J.set(k, "shock", J.JU{10})
      +k = J.set(k, "chain", J.JU{0})
      +_t10 = bannerpeak.session.Session.slam(self, id, k, J.u32(J.get(e, "a")), J.u32(J.get(e, "b")), J.u32(J.get(knobs, "slam")), say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t10)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t10))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t10))
      (k, self)
    case False{}:
      (k, self)

def bannerpeak.session.Session.body_event.if8?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +e: J.Val, +knobs: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +_t9 = bannerpeak.session.Session.bump(self, id, k, J.u32(J.get(e, "a")), J.u32(J.get(e, "b")), say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t9)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t9))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t9))
      (k, self)
    case False{}:
      +_t11 = bannerpeak.session.Session.body_event.if7(String.eq(J.str(J.get(e, "k")), "Slam"), self, id, k, e, knobs, say)
      +k = bannerpeak.u.fst(J.Val, J.Val, _t11)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t11)
      (k, self)

def bannerpeak.session.Session.body_event.if9?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +e: J.Val, +knobs: J.Val, +say: J.Val, +fx: U32, +fy: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +to : J.Val = bannerpeak.layout.hatch_to(RS.i_div_euclid(fx, 16), RS.i_div_euclid(fy, 16))
      +_t8 = bannerpeak.session.Session.body_event.if6(J.bool(J.get(to, "some")), k, say, to)
      +k = bannerpeak.u.fst(J.Val, J.Val, _t8)
      +say = bannerpeak.u.snd(J.Val, J.Val, _t8)
      (k, (say, self))
    case False{}:
      +_t12 = bannerpeak.session.Session.body_event.if8(String.eq(J.str(J.get(e, "k")), "Bump"), self, id, k, e, knobs, say)
      +k = bannerpeak.u.fst(J.Val, J.Val, _t12)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t12)
      (k, (say, self))

def bannerpeak.session.Session.body_event.if10?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +e: J.Val, +knobs: J.Val, +say: J.Val, +fx: U32, +fy: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t4 = bannerpeak.session.Session.fx(self, "Dust", fx, fy)
      +self : J.Val = _t4
      (k, (say, self))
    case False{}:
      +_t13 = bannerpeak.session.Session.body_event.if9(String.eq(J.str(J.get(e, "k")), "Hatch"), self, id, k, e, knobs, say, fx, fy)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t13)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t13))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t13))
      (k, (say, self))

def bannerpeak.session.Session.body_event.if11?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +e: J.Val, +knobs: J.Val, +say: J.Val, +fx: U32, +fy: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +k = J.set(k, "landed", J.JU{0})
      +_t3 = bannerpeak.session.Session.body_event.if5(RS.i_gt(J.u32(J.get(e, "a")), 1700), self, say, fx, fy)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t3)
      (k, (say, self))
    case False{}:
      +_t14 = bannerpeak.session.Session.body_event.if10(String.eq(J.str(J.get(e, "k")), "Skid"), self, id, k, e, knobs, say, fx, fy)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t14)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t14))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t14))
      (k, (say, self))

def bannerpeak.session.Session.body_event.if12?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +e: J.Val, +knobs: J.Val, +say: J.Val, +fx: U32, +fy: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +say = bannerpeak.session.Session.body_event.if4(String.eq(J.str(J.get(e, "s")), "Second"), e, say)
      (k, (say, self))
    case False{}:
      +_t15 = bannerpeak.session.Session.body_event.if11(String.eq(J.str(J.get(e, "k")), "Land"), self, id, k, e, knobs, say, fx, fy)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t15)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t15))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t15))
      (k, (say, self))

def bannerpeak.session.Session.bump?(+self: J.Val, +id: U32, +k: J.Val, +tx: U32, +ty: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +t : U32 = bannerpeak.session.Session.tile(self, tx, ty)
  +cx : U32 = ((tx * 16 : U32) + RS.i_div(16, 2) : U32)
  +top : U32 = (ty * 16 : U32)
  +hit : Bool = True{}
  +_t20 = bannerpeak.session.Session.bump.if8(U32.is_eq(t, 4), self, id, k, tx, ty, say, t, cx, top, hit)
  +hit = bannerpeak.u.fst(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t20)
  +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t20))
  +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t20))
  bannerpeak.session.Session.bump.if9(Bool.not(hit), self, k, tx, say, top)

def bannerpeak.session.Session.bump.if1?(bpc: Bool, +self: J.Val, +tx: U32, +ty: U32, +n: U32) -> J.Val:
  match bpc:
    case True{}:
      +_t5 = bannerpeak.session.Session.set_tile(self, tx, ty, ((20 + n : U32) - 1 : U32))
      +self : J.Val = _t5
      self
    case False{}:
      +_t6 = bannerpeak.session.Session.set_tile(self, tx, ty, 6)
      +self : J.Val = _t6
      self

def bannerpeak.session.Session.bump.if2?(bpc: Bool, +kind: String) -> String:
  match bpc:
    case True{}:
      +kind : String = "Loaf"
      kind
    case False{}:
      kind

def bannerpeak.session.Session.bump.if3?(bpc: Bool, +item: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +item = J.set(item, "dir", J.of_i32(4294967295))
      item
    case False{}:
      +item = J.set(item, "dir", J.of_i32(1))
      item

def bannerpeak.session.Session.bump.if4?(bpc: Bool, +self: J.Val, +tx: U32, +ty: U32, +say: J.Val, +cx: U32, +top: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +_t12 = bannerpeak.session.Session.set_tile(self, tx, ty, 0)
      +self : J.Val = _t12
      +_t13 = bannerpeak.session.Session.fx(self, "Debris", (cx - 5 : U32), (top + 8 : U32))
      +self : J.Val = _t13
      +_t14 = bannerpeak.session.Session.fx(self, "Debris", (cx + 5 : U32), (top + 8 : U32))
      +self : J.Val = _t14
      +say = J.push(say, J.JS{"crack_open"})
      (say, self)
    case False{}:
      +_t15 = bannerpeak.session.Session.fx(self, "Bump", cx, top)
      +self : J.Val = _t15
      +say = J.push(say, J.JS{"clang"})
      (say, self)

def bannerpeak.session.Session.bump.if5?(bpc: Bool, +self: J.Val, +k: J.Val, +tx: U32, +ty: U32, +say: J.Val, +cx: U32, +top: U32, +hit: Bool) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t16 = bannerpeak.session.Session.bump.if4(bannerpeak.knight.Knight.big(k), self, tx, ty, say, cx, top)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t16)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t16)
      (hit, (say, self))
    case False{}:
      +hit : Bool = False{}
      (hit, (say, self))

def bannerpeak.session.Session.bump.if6?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +tx: U32, +ty: U32, +say: J.Val, +t: U32, +cx: U32, +top: U32, +hit: Bool) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t10 = bannerpeak.session.Session.set_tile(self, tx, ty, 6)
      +self : J.Val = _t10
      +kind : String = "Flame"
      +kind = bannerpeak.session.Session.bump.if2(String.eq(J.str(J.get(k, "size")), "Small"), kind)
      +item : J.Val = bannerpeak.foes.Item.new(kind, cx, (top + 16 : U32))
      +item = bannerpeak.session.Session.bump.if3(U32.is_eq(U32.mod(id, 2), 0), item)
      +self = J.set(self, "items", J.push(J.get(self, "items"), item))
      +_t11 = bannerpeak.session.Session.fx(self, "Bump", cx, top)
      +self : J.Val = _t11
      +say = J.push(say, J.JS{"clang"})
      (hit, (say, self))
    case False{}:
      +_t17 = bannerpeak.session.Session.bump.if5(U32.is_eq(t, 7), self, k, tx, ty, say, cx, top, hit)
      +hit = bannerpeak.u.fst(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t17)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t17))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t17))
      (hit, (say, self))

def bannerpeak.session.Session.bump.if7?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +tx: U32, +ty: U32, +say: J.Val, +t: U32, +cx: U32, +top: U32, +hit: Bool) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +n : U32 = (t - 20 : U32)
      +self = bannerpeak.session.Session.bump.if1(U32.is_gt(n, 1), self, tx, ty, n)
      +_t7 = bannerpeak.session.Session.coin(self, k, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t7)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t7))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t7))
      +_t8 = bannerpeak.session.Session.fx(self, "Coin", cx, top)
      +self : J.Val = _t8
      +_t9 = bannerpeak.session.Session.fx(self, "Bump", cx, top)
      +self : J.Val = _t9
      +say = J.push(say, J.JS{"clang"})
      (hit, (say, self))
    case False{}:
      +_t18 = bannerpeak.session.Session.bump.if6(U32.is_eq(t, 5), self, id, k, tx, ty, say, t, cx, top, hit)
      +hit = bannerpeak.u.fst(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t18)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t18))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t18))
      (hit, (say, self))

def bannerpeak.session.Session.bump.if8?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +tx: U32, +ty: U32, +say: J.Val, +t: U32, +cx: U32, +top: U32, +hit: Bool) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.session.Session.set_tile(self, tx, ty, 6)
      +self : J.Val = _t1
      +_t2 = bannerpeak.session.Session.coin(self, k, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      +_t3 = bannerpeak.session.Session.fx(self, "Coin", cx, top)
      +self : J.Val = _t3
      +_t4 = bannerpeak.session.Session.fx(self, "Bump", cx, top)
      +self : J.Val = _t4
      +say = J.push(say, J.JS{"clang"})
      (hit, (say, self))
    case False{}:
      +_t19 = bannerpeak.session.Session.bump.if7(Bool.and(U32.is_ge(t, 21), U32.is_le(t, 25)), self, id, k, tx, ty, say, t, cx, top, hit)
      +hit = bannerpeak.u.fst(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t19)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t19))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t19))
      (hit, (say, self))

def bannerpeak.session.Session.bump.if9?(bpc: Bool, +self: J.Val, +k: J.Val, +tx: U32, +say: J.Val, +top: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (self, (k, say))
    case False{}:
      +_t21 = bannerpeak.session.Session.knock_foes(self, 0, tx, top)
      +self : J.Val = bannerpeak.u.snd(U32, J.Val, _t21)
      +score : U32 = bannerpeak.u.fst(U32, J.Val, _t21)
      +k = J.set(k, "score", J.JU{(J.u32(J.get(k, "score")) + score : U32)})
      +_t22 = bannerpeak.session.Session.knock_items(self, 0, tx, top)
      +self : J.Val = _t22
      (self, (k, say))

def bannerpeak.session.Session.knock_foes?(+self: J.Val, +n: U32, +tx: U32, +top: U32) -> Sigma<&2, &2, U32, _ => J.Val>:
  bannerpeak.session.Session.knock_foes.if2(U32.is_ge(n, J.len(J.get(self, "foes"))), self, n, tx, top)

def bannerpeak.session.Session.knock_foes.if1?(bpc: Bool, +self: J.Val, +n: U32, +f: J.Val, +gained: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, U32, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t2 = bannerpeak.foes.Foe.defeat(f)
      +f : J.Val = _t2
      +self = J.set(self, "foes", J.set_nth(J.get(self, "foes"), n, f))
      +gained : U32 = 100
      (f, (gained, self))
    case False{}:
      (f, (gained, self))

def bannerpeak.session.Session.knock_foes.if2?(bpc: Bool, +self: J.Val, +n: U32, +tx: U32, +top: U32) -> Sigma<&2, &2, U32, _ => J.Val>:
  match bpc:
    case True{}:
      (0, self)
    case False{}:
      +f : J.Val = J.nth(J.get(self, "foes"), n)
      +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.foes.Foe.rect(f)
      +l = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)
      +ft = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1))
      +r = bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
      +b = bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
      +gained : U32 = 0
      +_t3 = bannerpeak.session.Session.knock_foes.if1(Bool.and(Bool.and(Bool.and(bannerpeak.foes.Foe.alive(f), U32.is_eq(b, top)), RS.i_lt(l, ((tx * 16 : U32) + 16 : U32))), RS.i_gt(r, (tx * 16 : U32))), self, n, f, gained)
      +f = bannerpeak.u.fst(J.Val, Sigma<&2, &2, U32, _ => J.Val>, _t3)
      +gained = bannerpeak.u.fst(U32, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, U32, _ => J.Val>, _t3))
      +self = bannerpeak.u.snd(U32, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, U32, _ => J.Val>, _t3))
      +_t4 = bannerpeak.session.Session.knock_foes(self, (n + 1 : U32), tx, top)
      +self : J.Val = bannerpeak.u.snd(U32, J.Val, _t4)
      ((gained + bannerpeak.u.fst(U32, J.Val, _t4) : U32), self)

def bannerpeak.session.Session.knock_items?(+self: J.Val, +n: U32, +tx: U32, +top: U32) -> J.Val:
  bannerpeak.session.Session.knock_items.if2(U32.is_ge(n, J.len(J.get(self, "items"))), self, n, tx, top)

def bannerpeak.session.Session.knock_items.if1?(bpc: Bool, +self: J.Val, +n: U32, +i: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +i = J.set(i, "vy", J.of_i32(4294965796))
      +self = J.set(self, "items", J.set_nth(J.get(self, "items"), n, i))
      (i, self)
    case False{}:
      (i, self)

def bannerpeak.session.Session.knock_items.if2?(bpc: Bool, +self: J.Val, +n: U32, +tx: U32, +top: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +i : J.Val = J.nth(J.get(self, "items"), n)
      +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.foes.Item.rect(i)
      +l = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)
      +it = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1))
      +r = bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
      +b = bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
      +_t2 = bannerpeak.session.Session.knock_items.if1(Bool.and(Bool.and(Bool.and(U32.is_eq(b, top), RS.i_lt(l, ((tx * 16 : U32) + 16 : U32))), RS.i_gt(r, (tx * 16 : U32))), String.eq(J.str(J.get(i, "kind")), "Cap")), self, n, i)
      +i = bannerpeak.u.fst(J.Val, J.Val, _t2)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t2)
      +_t3 = bannerpeak.session.Session.knock_items(self, (n + 1 : U32), tx, top)
      +self : J.Val = _t3
      self

def bannerpeak.session.Session.coin?(+self: J.Val, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +k = J.set(k, "coins", J.JU{(J.u32(J.get(k, "coins")) + 1 : U32)})
  +k = J.set(k, "score", J.JU{(J.u32(J.get(k, "score")) + 10 : U32)})
  +say = J.push(say, J.JS{"gold"})
  +k = bannerpeak.session.Session.coin.if1(U32.is_eq(U32.mod(J.u32(J.get(k, "coins")), 20), 0), k)
  +_t1 = bannerpeak.session.Session.coin.if2(U32.is_eq(U32.mod(J.u32(J.get(k, "coins")), 100), 0), k, say)
  +k = bannerpeak.u.fst(J.Val, J.Val, _t1)
  +say = bannerpeak.u.snd(J.Val, J.Val, _t1)
  (self, (k, say))

def bannerpeak.session.Session.coin.if1?(bpc: Bool, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "pips", J.JU{RS.u_min((J.u32(J.get(k, "pips")) + 1 : U32), 8)})
      k
    case False{}:
      k

def bannerpeak.session.Session.coin.if2?(bpc: Bool, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +k = J.set(k, "pips", J.JU{8})
      +k = J.set(k, "score", J.JU{(J.u32(J.get(k, "score")) + 500 : U32)})
      +say = J.push(say, J.JS{"wish_grant"})
      (k, say)
    case False{}:
      (k, say)

def bannerpeak.session.Session.slam?(+self: J.Val, +id: U32, +k: J.Val, +x: U32, +y: U32, +radius: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +_t1 = bannerpeak.session.Session.fx(self, "Ring", x, y)
  +self : J.Val = _t1
  +say = J.push(say, J.JS{"rock_land"})
  +phase : U32 = J.u32(J.get(J.get(self, "boss"), "phase"))
  +_t2 = bannerpeak.boss.Boss.slam(J.get(self, "boss"), x, y)
  +self = J.set(self, "boss", bannerpeak.u.snd(Bool, J.Val, _t2))
  +_t5 = bannerpeak.session.Session.slam.if5(bannerpeak.u.fst(Bool, J.Val, _t2), self, k, x, y, say, phase)
  +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t5)
  +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t5))
  +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t5))
  +row : U32 = RS.i_div_euclid(y, 16)
  +c0 : U32 = RS.i_div_euclid((x - radius : U32), 16)
  +c1 : U32 = RS.i_div_euclid((x + radius : U32), 16)
  +_t6 = bannerpeak.session.Session.crack_rows(self, row, (row + 1 : U32), c0, c1, x, radius)
  +self : J.Val = bannerpeak.u.snd(Bool, J.Val, _t6)
  +broke : Bool = bannerpeak.u.fst(Bool, J.Val, _t6)
  +say = bannerpeak.session.Session.slam.if6(broke, say)
  +_t7 = bannerpeak.session.Session.slam_foes(self, 0, x, y, radius, say)
  +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(U32, Sigma<&2, &2, J.Val, _ => J.Val>, _t7))
  +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(U32, Sigma<&2, &2, J.Val, _ => J.Val>, _t7))
  +k = J.set(k, "score", J.JU{(J.u32(J.get(k, "score")) + bannerpeak.u.fst(U32, Sigma<&2, &2, J.Val, _ => J.Val>, _t7) : U32)})
  +_t8 = bannerpeak.session.Session.slam_caps(self, 0, x, y, radius)
  +self : J.Val = _t8
  (self, (k, say))

def bannerpeak.session.Session.slam.if1?(bpc: Bool, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"boss_rage"})
      say
    case False{}:
      +say = J.push(say, J.JS{"boss_roar"})
      say

def bannerpeak.session.Session.slam.if2?(bpc: Bool, +self: J.Val, +say: J.Val, +phase: U32) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"boss_death"})
      say
    case False{}:
      +say = bannerpeak.session.Session.slam.if1(U32.is_gt(J.u32(J.get(J.get(self, "boss"), "phase")), phase), say)
      say

def bannerpeak.session.Session.slam.if3?(bpc: Bool, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"shield_block"})
      say
    case False{}:
      say

def bannerpeak.session.Session.slam.if4?(bpc: Bool, +self: J.Val, +x: U32, +y: U32, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +head : J.Val = bannerpeak.boss.Boss.plank(J.get(self, "boss"), False{})
      +say = bannerpeak.session.Session.slam.if3(Bool.and(RS.i_le(RS.i_abs((x - J.u32(J.get(J.get(self, "boss"), "x")) : U32)), 24), RS.i_le(RS.i_abs((y - J.u32(J.get(head, "y")) : U32)), 3)), say)
      say
    case False{}:
      say

def bannerpeak.session.Session.slam.if5?(bpc: Bool, +self: J.Val, +k: J.Val, +x: U32, +y: U32, +say: J.Val, +phase: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +k = J.set(k, "score", J.JU{(J.u32(J.get(k, "score")) + 500 : U32)})
      +_t3 = bannerpeak.session.Session.fx(self, "Debris", J.u32(J.get(J.get(self, "boss"), "x")), (y - 8 : U32))
      +self : J.Val = _t3
      +_t4 = bannerpeak.session.Session.fx(self, "Gleam", J.u32(J.get(J.get(self, "boss"), "x")), (y - 14 : U32))
      +self : J.Val = _t4
      +say = bannerpeak.session.Session.slam.if2(bannerpeak.boss.Boss.defeated(J.get(self, "boss")), self, say, phase)
      (k, (say, self))
    case False{}:
      +say = bannerpeak.session.Session.slam.if4(Bool.not(bannerpeak.boss.Boss.defeated(J.get(self, "boss"))), self, x, y, say)
      (k, (say, self))

def bannerpeak.session.Session.slam.if6?(bpc: Bool, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"crack_open"})
      say
    case False{}:
      say

def bannerpeak.session.Session.crack_rows?(+self: J.Val, +ty: U32, +last: U32, +c0: U32, +c1: U32, +x: U32, +radius: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  bannerpeak.session.Session.crack_rows.if1(RS.i_gt(ty, last), self, ty, last, c0, c1, x, radius)

def bannerpeak.session.Session.crack_rows.if1?(bpc: Bool, +self: J.Val, +ty: U32, +last: U32, +c0: U32, +c1: U32, +x: U32, +radius: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      (False{}, self)
    case False{}:
      +_t1 = bannerpeak.session.Session.crack_cols(self, ty, c0, c1, x, radius)
      +self : J.Val = bannerpeak.u.snd(Bool, J.Val, _t1)
      +a : Bool = bannerpeak.u.fst(Bool, J.Val, _t1)
      +_t2 = bannerpeak.session.Session.crack_rows(self, (ty + 1 : U32), last, c0, c1, x, radius)
      +self : J.Val = bannerpeak.u.snd(Bool, J.Val, _t2)
      +b : Bool = bannerpeak.u.fst(Bool, J.Val, _t2)
      (Bool.or(a, b), self)

def bannerpeak.session.Session.crack_cols?(+self: J.Val, +ty: U32, +tx: U32, +c1: U32, +x: U32, +radius: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  bannerpeak.session.Session.crack_cols.if3(RS.i_gt(tx, c1), self, ty, tx, c1, x, radius)

def bannerpeak.session.Session.crack_cols.if1?(bpc: Bool, +self: J.Val, +ty: U32, +tx: U32, +broke: Bool, +centre: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.session.Session.set_tile(self, tx, ty, 0)
      +self : J.Val = _t1
      +_t2 = bannerpeak.session.Session.fx(self, "Debris", (centre - 4 : U32), ((ty * 16 : U32) + 8 : U32))
      +self : J.Val = _t2
      +_t3 = bannerpeak.session.Session.fx(self, "Debris", (centre + 4 : U32), ((ty * 16 : U32) + 8 : U32))
      +self : J.Val = _t3
      +broke : Bool = True{}
      (broke, self)
    case False{}:
      (broke, self)

def bannerpeak.session.Session.crack_cols.if2?(bpc: Bool, +self: J.Val, +ty: U32, +tx: U32, +x: U32, +radius: U32, +broke: Bool) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +centre : U32 = ((tx * 16 : U32) + RS.i_div(16, 2) : U32)
      +_t4 = bannerpeak.session.Session.crack_cols.if1(RS.i_le(RS.i_abs((centre - x : U32)), (radius + RS.i_div(16, 2) : U32)), self, ty, tx, broke, centre)
      +broke = bannerpeak.u.fst(Bool, J.Val, _t4)
      +self = bannerpeak.u.snd(Bool, J.Val, _t4)
      (broke, self)
    case False{}:
      (broke, self)

def bannerpeak.session.Session.crack_cols.if3?(bpc: Bool, +self: J.Val, +ty: U32, +tx: U32, +c1: U32, +x: U32, +radius: U32) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      (False{}, self)
    case False{}:
      +broke : Bool = False{}
      +_t5 = bannerpeak.session.Session.crack_cols.if2(U32.is_eq(bannerpeak.session.Session.tile(self, tx, ty), 8), self, ty, tx, x, radius, broke)
      +broke = bannerpeak.u.fst(Bool, J.Val, _t5)
      +self = bannerpeak.u.snd(Bool, J.Val, _t5)
      +_t6 = bannerpeak.session.Session.crack_cols(self, ty, (tx + 1 : U32), c1, x, radius)
      +self : J.Val = bannerpeak.u.snd(Bool, J.Val, _t6)
      +rest : Bool = bannerpeak.u.fst(Bool, J.Val, _t6)
      (Bool.or(broke, rest), self)

def bannerpeak.session.Session.slam_foes?(+self: J.Val, +n: U32, +x: U32, +y: U32, +radius: U32, +say: J.Val) -> Sigma<&2, &2, U32, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  bannerpeak.session.Session.slam_foes.if2(U32.is_ge(n, J.len(J.get(self, "foes"))), self, n, x, y, radius, say)

def bannerpeak.session.Session.slam_foes.if1?(bpc: Bool, +self: J.Val, +n: U32, +say: J.Val, +f: J.Val, +gained: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  match bpc:
    case True{}:
      +_t2 = bannerpeak.foes.Foe.defeat(f)
      +f : J.Val = _t2
      +self = J.set(self, "foes", J.set_nth(J.get(self, "foes"), n, f))
      +gained : U32 = 200
      +say = J.push(say, J.JS{"kill"})
      (f, (gained, (say, self)))
    case False{}:
      (f, (gained, (say, self)))

def bannerpeak.session.Session.slam_foes.if2?(bpc: Bool, +self: J.Val, +n: U32, +x: U32, +y: U32, +radius: U32, +say: J.Val) -> Sigma<&2, &2, U32, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (0, (self, say))
    case False{}:
      +f : J.Val = J.nth(J.get(self, "foes"), n)
      +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.foes.Foe.rect(f)
      +l = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)
      +ft = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1))
      +r = bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
      +b = bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
      +cx : U32 = RS.i_div((l + r : U32), 2)
      +gained : U32 = 0
      +_t3 = bannerpeak.session.Session.slam_foes.if1(Bool.and(Bool.and(bannerpeak.foes.Foe.alive(f), RS.i_le(RS.i_abs((cx - x : U32)), radius)), RS.i_le(RS.i_abs((b - y : U32)), 8)), self, n, say, f, gained)
      +f = bannerpeak.u.fst(J.Val, Sigma<&2, &2, U32, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t3)
      +gained = bannerpeak.u.fst(U32, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, U32, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t3))
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(U32, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, U32, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t3)))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(U32, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, U32, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t3)))
      +_t4 = bannerpeak.session.Session.slam_foes(self, (n + 1 : U32), x, y, radius, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(U32, Sigma<&2, &2, J.Val, _ => J.Val>, _t4))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(U32, Sigma<&2, &2, J.Val, _ => J.Val>, _t4))
      ((gained + bannerpeak.u.fst(U32, Sigma<&2, &2, J.Val, _ => J.Val>, _t4) : U32), (self, say))

def bannerpeak.session.Session.slam_caps?(+self: J.Val, +n: U32, +x: U32, +y: U32, +radius: U32) -> J.Val:
  bannerpeak.session.Session.slam_caps.if3(U32.is_ge(n, J.len(J.get(self, "items"))), self, n, x, y, radius)

def bannerpeak.session.Session.slam_caps.if1?(bpc: Bool, +self: J.Val, +n: U32, +it: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +it = J.set(it, "cap", J.JS{"Rest"})
      +it = J.set(it, "vx", J.of_i32(0))
      +self = J.set(self, "items", J.set_nth(J.get(self, "items"), n, it))
      (it, self)
    case False{}:
      (it, self)

def bannerpeak.session.Session.slam_caps.if2?(bpc: Bool, +self: J.Val, +n: U32, +x: U32, +y: U32, +radius: U32, +it: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.foes.Item.rect(it)
      +l = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)
      +itop = bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1))
      +r = bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
      +b = bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)))
      +_t2 = bannerpeak.session.Session.slam_caps.if1(Bool.and(RS.i_le(RS.i_abs((RS.i_div((l + r : U32), 2) - x : U32)), radius), RS.i_le(RS.i_abs((b - y : U32)), 8)), self, n, it)
      +it = bannerpeak.u.fst(J.Val, J.Val, _t2)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t2)
      (it, self)
    case False{}:
      (it, self)

def bannerpeak.session.Session.slam_caps.if3?(bpc: Bool, +self: J.Val, +n: U32, +x: U32, +y: U32, +radius: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +it : J.Val = J.nth(J.get(self, "items"), n)
      +_t3 = bannerpeak.session.Session.slam_caps.if2(Bool.and(String.eq(J.str(J.get(it, "kind")), "Cap"), String.eq(J.str(J.get(it, "cap")), "Slide")), self, n, x, y, radius, it)
      +it = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t3)
      +_t4 = bannerpeak.session.Session.slam_caps(self, (n + 1 : U32), x, y, radius)
      +self : J.Val = _t4
      self

def bannerpeak.session.Session.small?(+self: J.Val) -> J.Val:
  +_j1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.session.Session.station(self)
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "station", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j1)), J.of_i32(bannerpeak.u.snd(U32, U32, _j1))]}), "count", J.JU{bannerpeak.session.Session.count(self)}), "door_count", J.JU{bannerpeak.session.Session.door_count(self)}), "alive", bannerpeak.session.Session.alive(self)), "spark_gate", J.JU{bannerpeak.session.Session.spark_gate(self)}), "crown_down", J.JBool{bannerpeak.session.Session.crown_down(self)}), "valid", J.JBool{bannerpeak.session.Session.valid(self)})

def case.bannerpeak.session.Session.step?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.step(J.get(input, "session"), J.get(input, "inputs"), J.get(input, "heroes"), J.get(input, "knobs"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "say", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.small?(+input: J.Val) -> J.Val:
  bannerpeak.session.Session.small(J.get(input, "session"))

def case.bannerpeak.session.Session.tile?(+input: J.Val) -> J.Val:
  J.JU{bannerpeak.session.Session.tile(J.get(input, "session"), J.u32(J.get(input, "tx")), J.u32(J.get(input, "ty")))}

def case.bannerpeak.session.Session.set_tile?(+input: J.Val) -> J.Val:
  bannerpeak.session.Session.set_tile(J.get(input, "session"), J.u32(J.get(input, "tx")), J.u32(J.get(input, "ty")), J.u32(J.get(input, "code")))

def case.bannerpeak.session.Session.seat?(+input: J.Val) -> J.Val:
  bannerpeak.session.Session.seat(J.get(input, "session"), J.u32(J.get(input, "id")), J.u32(J.get(input, "joined")))

def case.bannerpeak.session.Session.provision?(+input: J.Val) -> J.Val:
  bannerpeak.session.Session.provision(J.get(input, "session"))

def case.bannerpeak.session.Session.step_knight?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.step_knight(J.get(input, "session"), J.u32(J.get(input, "id")), J.get(input, "k"), J.get(input, "input"), J.get(input, "knobs"), bannerpeak.u.vec_or(J.get(input, "now")), bannerpeak.u.vec_or(J.get(input, "before")), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)), "k", bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r))), "say", bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)))

def case.bannerpeak.session.Session.note_safe?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.note_safe(J.get(input, "session"), J.get(input, "k"))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "k", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.body_event?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.body_event(J.get(input, "session"), J.u32(J.get(input, "id")), J.get(input, "k"), J.get(input, "ev"), J.get(input, "knobs"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)), "k", bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r))), "say", bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)))

def case.bannerpeak.session.Session.bump?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.bump(J.get(input, "session"), J.u32(J.get(input, "id")), J.get(input, "k"), J.u32(J.get(input, "tx")), J.u32(J.get(input, "ty")), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)), "k", bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r))), "say", bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)))

def case.bannerpeak.session.Session.coin?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.coin(J.get(input, "session"), J.get(input, "k"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)), "k", bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r))), "say", bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)))

def case.bannerpeak.session.Session.slam?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.slam(J.get(input, "session"), J.u32(J.get(input, "id")), J.get(input, "k"), J.u32(J.get(input, "x")), J.u32(J.get(input, "y")), J.u32(J.get(input, "radius")), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)), "k", bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r))), "say", bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)))

def case.bannerpeak.session.Session.new?(+input: J.Val) -> J.Val:
  bannerpeak.session.Session.new(J.get(input, "heroes"))

def bannerpeak.session.Session.touch?(+self: J.Val, +id: U32, +k: J.Val, +bits: U32, +was: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +me : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.session.px(J.get(k, "body"))
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +wp : U32 = (434 * 16 : U32)
  +hp : U32 = (64 * 16 : U32)
  bannerpeak.session.Session.touch.if4(Bool.or(Bool.or(RS.i_gt(fy, (hp + 24 : U32)), RS.i_lt(fx, 4294967288)), RS.i_gt(fx, (wp + 8 : U32))), self, id, k, bits, was, say, me, fx, fy)

def bannerpeak.session.Session.touch.if1?(bpc: Bool, +k: J.Val, +_b13: Bool) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +_t14 = bannerpeak.knight.Knight.hit(k, RS.i_neg(J.u32(J.get(J.get(k, "body"), "face"))))
      +k : J.Val = bannerpeak.u.snd(Bool, J.Val, _t14)
      +_b13 : Bool = bannerpeak.u.fst(Bool, J.Val, _t14)
      (_b13, k)
    case False{}:
      (_b13, k)

def bannerpeak.session.Session.touch.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"knight_hit"})
      +_t16 = bannerpeak.session.Session.after_hit(self, k)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t16)
      +k : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t16)
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.touch.if3?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +bits: U32, +was: U32, +say: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +fx: U32, +fy: U32, +pricked: Bool) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t12 = bannerpeak.session.Session.fall(self, k, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t12)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t12))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t12))
      (self, (k, say))
    case False{}:
      +_b13 = pricked
      +_t15 = bannerpeak.session.Session.touch.if1(_b13, k, _b13)
      +_b13 = bannerpeak.u.fst(Bool, J.Val, _t15)
      +k = bannerpeak.u.snd(Bool, J.Val, _t15)
      +_t17 = bannerpeak.session.Session.touch.if2(_b13, self, k, say)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t17)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t17)
      +_t18 = bannerpeak.session.Session.touch_coins(self, k, me, bannerpeak.layout.info.coins(), 0, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t18)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t18))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t18))
      +_t19 = bannerpeak.session.Session.touch_sparks(self, k, me, 0, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t19)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t19))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t19))
      +_t20 = bannerpeak.session.Session.touch_banners(self, k, me, 0, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t20)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t20))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t20))
      +_t21 = bannerpeak.session.Session.touch_gates(self, me, 0, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t21)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t21)
      +_t22 = bannerpeak.session.Session.touch_pole(self, k, me, fx, fy, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t22)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t22))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t22))
      +_t23 = bannerpeak.session.Session.touch_board(self, k, bits, fx, fy, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t23)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t23))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t23))
      +_t24 = bannerpeak.session.Session.touch_things(self, id, k, bits, was, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t24)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t24))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t24))
      (self, (k, say))

def bannerpeak.session.Session.touch.if4?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +bits: U32, +was: U32, +say: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +fx: U32, +fy: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t2 = bannerpeak.session.Session.fall(self, k, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      (self, (k, say))
    case False{}:
      +_t3 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = me
      +_t4 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = me
      +_t5 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = me
      +_t6 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = me
      +deep : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = ((bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t3) + 1 : U32), (bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t4)), ((bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t5))) - 1 : U32), (bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t6))) - 4 : U32))))
      +_t7 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = deep
      +_t8 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = me
      +_t9 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = me
      +_t10 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = me
      +_t11 : Sigma<&2, &2, Bool, _ => Bool> = bannerpeak.session.Session.water_spikes(self, me, deep, RS.i_div_euclid(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t7)), 16), RS.i_div_euclid((bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t8))) - 1 : U32), 16), RS.i_div_euclid(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t9), 16), RS.i_div_euclid((bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t10))) - 1 : U32), 16))
      +drowned = bannerpeak.u.fst(Bool, Bool, _t11)
      +pricked = bannerpeak.u.snd(Bool, Bool, _t11)
      bannerpeak.session.Session.touch.if3(drowned, self, id, k, bits, was, say, me, fx, fy, pricked)

def bannerpeak.session.Session.water_spikes?(+self: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +deep: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +ty: U32, +ty1: U32, +tx0: U32, +tx1: U32) -> Sigma<&2, &2, Bool, _ => Bool>:
  bannerpeak.session.Session.water_spikes.if1(RS.i_gt(ty, ty1), self, me, deep, ty, ty1, tx0, tx1)

def bannerpeak.session.Session.water_spikes.if1?(bpc: Bool, +self: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +deep: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +ty: U32, +ty1: U32, +tx0: U32, +tx1: U32) -> Sigma<&2, &2, Bool, _ => Bool>:
  match bpc:
    case True{}:
      (False{}, False{})
    case False{}:
      +_t1 : Sigma<&2, &2, Bool, _ => Bool> = bannerpeak.session.Session.water_spikes_row(self, me, deep, ty, tx0, tx1)
      +d0 = bannerpeak.u.fst(Bool, Bool, _t1)
      +p0 = bannerpeak.u.snd(Bool, Bool, _t1)
      +_t2 : Sigma<&2, &2, Bool, _ => Bool> = bannerpeak.session.Session.water_spikes(self, me, deep, (ty + 1 : U32), ty1, tx0, tx1)
      +d1 = bannerpeak.u.fst(Bool, Bool, _t2)
      +p1 = bannerpeak.u.snd(Bool, Bool, _t2)
      (Bool.or(d0, d1), Bool.or(p0, p1))

def bannerpeak.session.Session.water_spikes_row?(+self: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +deep: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +ty: U32, +tx: U32, +tx1: U32) -> Sigma<&2, &2, Bool, _ => Bool>:
  bannerpeak.session.Session.water_spikes_row.if3(RS.i_gt(tx, tx1), self, me, deep, ty, tx, tx1)

def bannerpeak.session.Session.water_spikes_row.if1?(bpc: Bool, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +ty: U32, +tx: U32, +pricked: Bool) -> Bool:
  match bpc:
    case True{}:
      +pricked : Bool = bannerpeak.session.overlap(me, (((tx * 16 : U32) + 3 : U32), (((ty * 16 : U32) + 6 : U32), ((((tx * 16 : U32) + 16 : U32) - 3 : U32), ((ty * 16 : U32) + 16 : U32)))))
      pricked
    case False{}:
      pricked

def bannerpeak.session.Session.water_spikes_row.if2?(bpc: Bool, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +deep: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +ty: U32, +tx: U32, +t: U32, +drowned: Bool, +pricked: Bool) -> Sigma<&2, &2, Bool, _ => Bool>:
  match bpc:
    case True{}:
      +drowned : Bool = bannerpeak.session.overlap(deep, ((tx * 16 : U32), ((ty * 16 : U32), (((tx * 16 : U32) + 16 : U32), ((ty * 16 : U32) + 16 : U32)))))
      (drowned, pricked)
    case False{}:
      +pricked = bannerpeak.session.Session.water_spikes_row.if1(U32.is_eq(t, 10), me, ty, tx, pricked)
      (drowned, pricked)

def bannerpeak.session.Session.water_spikes_row.if3?(bpc: Bool, +self: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +deep: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +ty: U32, +tx: U32, +tx1: U32) -> Sigma<&2, &2, Bool, _ => Bool>:
  match bpc:
    case True{}:
      (False{}, False{})
    case False{}:
      +t : U32 = bannerpeak.session.Session.tile(self, tx, ty)
      +drowned : Bool = False{}
      +pricked : Bool = False{}
      +_t1 = bannerpeak.session.Session.water_spikes_row.if2(U32.is_eq(t, 9), me, deep, ty, tx, t, drowned, pricked)
      +drowned = bannerpeak.u.fst(Bool, Bool, _t1)
      +pricked = bannerpeak.u.snd(Bool, Bool, _t1)
      +_t2 : Sigma<&2, &2, Bool, _ => Bool> = bannerpeak.session.Session.water_spikes_row(self, me, deep, ty, (tx + 1 : U32), tx1)
      +d1 = bannerpeak.u.fst(Bool, Bool, _t2)
      +p1 = bannerpeak.u.snd(Bool, Bool, _t2)
      (Bool.or(drowned, d1), Bool.or(pricked, p1))

def bannerpeak.session.Session.touch_coins?(+self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +cs: List<&2, U32>, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  bannerpeak.session.Session.touch_coins.if3(bannerpeak.u.is_empty(cs), self, k, me, cs, n, say)

def bannerpeak.session.Session.touch_coins.if1?(bpc: Bool, +self: J.Val, +k: J.Val, +n: U32, +say: J.Val, +cx: U32, +cy: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "taken", bannerpeak.u.set_insert(J.get(self, "taken"), n))
      +_t1 = bannerpeak.session.Session.coin(self, k, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      +_t2 = bannerpeak.session.Session.fx(self, "Coin", cx, cy)
      +self : J.Val = _t2
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_coins.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +n: U32, +say: J.Val, +x: U32, +y: U32) -> J.Val:
  match bpc:
    case True{}:
      +cx : U32 = ((x * 16 : U32) + RS.i_div(16, 2) : U32)
      +cy : U32 = ((y * 16 : U32) + RS.i_div(16, 2) : U32)
      +self = bannerpeak.session.Session.touch_coins.if1(bannerpeak.session.overlap(me, ((cx - 5 : U32), ((cy - 5 : U32), ((cx + 5 : U32), (cy + 5 : U32))))), self, k, n, say, cx, cy)
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_coins.if3?(bpc: Bool, +self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +cs: List<&2, U32>, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (self, (k, say))
    case False{}:
      +x : U32 = bannerpeak.u.head(cs)
      +r1 : List<&2, U32> = bannerpeak.u.tail(cs)
      +y : U32 = bannerpeak.u.head(r1)
      +rest : List<&2, U32> = bannerpeak.u.tail(r1)
      +self = bannerpeak.session.Session.touch_coins.if2(Bool.not(bannerpeak.u.set_has(J.get(self, "taken"), n)), self, k, me, n, say, x, y)
      +_t3 = bannerpeak.session.Session.touch_coins(self, k, me, rest, (n + 1 : U32), say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
      (self, (k, say))

def bannerpeak.session.Session.touch_sparks?(+self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  bannerpeak.session.Session.touch_sparks.if4(U32.is_ge(n, bannerpeak.layout.info.sparks_len()), self, k, me, n, say)

def bannerpeak.session.Session.touch_sparks.if1?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t2 = bannerpeak.session.Session.award(self, 6, k, True{}, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_sparks.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +n: U32, +say: J.Val, +cx: U32, +cy: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "sparks", J.JU{U32.or(J.u32(J.get(self, "sparks")), RS.u_shl(1, n))})
      +_t1 = bannerpeak.session.Session.fx(self, "Gleam", cx, cy)
      +self : J.Val = _t1
      +say = J.push(say, J.JS{"spell_cast"})
      +self = bannerpeak.session.Session.touch_sparks.if1(U32.is_eq(J.u32(J.get(self, "sparks")), 255), self, k, say)
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.touch_sparks.if3?(bpc: Bool, +self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +cx : U32 = ((bannerpeak.layout.info.spark_x(n) * 16 : U32) + RS.i_div(16, 2) : U32)
      +cy : U32 = ((bannerpeak.layout.info.spark_y(n) * 16 : U32) + RS.i_div(16, 2) : U32)
      +_t3 = bannerpeak.session.Session.touch_sparks.if2(bannerpeak.session.overlap(me, ((cx - 7 : U32), ((cy - 7 : U32), ((cx + 7 : U32), (cy + 7 : U32))))), self, k, n, say, cx, cy)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t3)
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.touch_sparks.if4?(bpc: Bool, +self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (self, (k, say))
    case False{}:
      +_t4 = bannerpeak.session.Session.touch_sparks.if3(U32.is_eq(U32.and(J.u32(J.get(self, "sparks")), RS.u_shl(1, n)), 0), self, k, me, n, say)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t4)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t4)
      +_t5 = bannerpeak.session.Session.touch_sparks(self, k, me, (n + 1 : U32), say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t5)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t5))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t5))
      (self, (k, say))

def bannerpeak.session.Session.touch_banners?(+self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +b: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  bannerpeak.session.Session.touch_banners.if3(U32.is_ge(b, 8), self, k, me, b, say)

def bannerpeak.session.Session.touch_banners.if1?(bpc: Bool, +self: J.Val, +k: J.Val, +b: U32, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.session.Session.award(self, b, k, True{}, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_banners.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +b: U32, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +cx : U32 = ((bannerpeak.layout.info.banner_x(b) * 16 : U32) + RS.i_div(16, 2) : U32)
      +cy : U32 = ((bannerpeak.layout.info.banner_y(b) * 16 : U32) + RS.i_div(16, 2) : U32)
      +self = bannerpeak.session.Session.touch_banners.if1(bannerpeak.session.overlap(me, ((cx - 9 : U32), ((cy - 12 : U32), ((cx + 9 : U32), (cy + 12 : U32))))), self, k, b, say)
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_banners.if3?(bpc: Bool, +self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +b: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (self, (k, say))
    case False{}:
      +self = bannerpeak.session.Session.touch_banners.if2(Bool.and(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "banners")), RS.u_shl(1, b)), 0), bannerpeak.layout.info.banner_has(b)), Bool.not(Bool.and(U32.is_eq(b, 7), Bool.not(bannerpeak.session.Session.crown_down(self))))), self, k, me, b, say)
      +_t2 = bannerpeak.session.Session.touch_banners(self, k, me, (b + 1 : U32), say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
      (self, (k, say))

def bannerpeak.session.Session.touch_gates?(+self: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  bannerpeak.session.Session.touch_gates.if4(U32.is_ge(n, bannerpeak.layout.info.gates_len()), self, me, n, say)

def bannerpeak.session.Session.touch_gates.if1?(bpc: Bool, +self: J.Val, +n: U32, +say: J.Val, +cx: U32, +cy: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "gates", J.JU{U32.or(J.u32(J.get(self, "gates")), RS.u_shl(1, n))})
      +_t1 = bannerpeak.session.Session.fx(self, "Gleam", cx, cy)
      +self : J.Val = _t1
      +say = J.push(say, J.JS{"shield_up"})
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.touch_gates.if2?(bpc: Bool, +self: J.Val, +n: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "checkpoint", J.JU{(n + 1 : U32)})
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_gates.if3?(bpc: Bool, +self: J.Val, +n: U32, +say: J.Val, +cx: U32, +cy: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +_t2 = bannerpeak.session.Session.touch_gates.if1(U32.is_eq(U32.and(J.u32(J.get(self, "gates")), RS.u_shl(1, n)), 0), self, n, say, cx, cy)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t2)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t2)
      +self = bannerpeak.session.Session.touch_gates.if2(U32.is_gt((n + 1 : U32), J.u32(J.get(self, "checkpoint"))), self, n)
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.touch_gates.if4?(bpc: Bool, +self: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      (self, say)
    case False{}:
      +cx : U32 = ((bannerpeak.layout.info.gate_x(n) * 16 : U32) + RS.i_div(16, 2) : U32)
      +cy : U32 = ((bannerpeak.layout.info.gate_y(n) * 16 : U32) + RS.i_div(16, 2) : U32)
      +_t3 = bannerpeak.session.Session.touch_gates.if3(bannerpeak.session.overlap(me, ((cx - 8 : U32), ((cy - 24 : U32), ((cx + 8 : U32), (cy + 8 : U32))))), self, n, say, cx, cy)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t3)
      +_t4 = bannerpeak.session.Session.touch_gates(self, me, (n + 1 : U32), say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t4)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t4)
      (self, say)

def bannerpeak.session.Session.touch_pole?(+self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +fx: U32, +fy: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +pole_x : U32 = ((bannerpeak.layout.info.pole_x() * 16 : U32) + RS.i_div(16, 2) : U32)
  +base : U32 = ((bannerpeak.layout.info.pole_y() + 1 : U32) * 16 : U32)
  +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = me
  +_t2 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = me
  +_t3 = bannerpeak.session.Session.touch_pole.if2(Bool.and(Bool.and(Bool.and(Bool.and(U32.is_eq(J.u32(J.get(k, "pole")), 0), Bool.not(J.bool(J.get(k, "poled")))), RS.i_le(RS.i_abs((fx - pole_x : U32)), 6)), RS.i_gt(bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1))), (base - (9 * 16 : U32) : U32))), RS.i_lt(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t2)), base)), self, k, fy, say, pole_x, base)
  +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3)
  +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
  +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
  (self, (k, say))

def bannerpeak.session.Session.touch_pole.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "banners", J.JU{U32.or(J.u32(J.get(self, "banners")), 4)})
      +self = J.set(self, "announce", J.set(J.set(J.obj(), "banner", J.JU{2}), "t", J.JU{110}))
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_pole.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +fy: U32, +say: J.Val, +pole_x: U32, +base: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +height : U32 = RS.i_clamp(RS.i_div((base - fy : U32), 16), 0, 9)
      +k = J.set(k, "grip", J.of_i32(height))
      +k = J.set(k, "pole", J.JU{1})
      +k = J.set(k, "score", J.JU{(J.u32(J.get(k, "score")) + bannerpeak.u.pole_score(height) : U32)})
      +k = J.set(k, "body", J.set(J.get(k, "body"), "x", J.of_i32(((pole_x - (6 * J.u32(J.get(J.get(k, "body"), "face")) : U32) : U32) * 256 : U32))))
      +k = J.set(k, "body", J.set(J.get(k, "body"), "vx", J.of_i32(0)))
      +k = J.set(k, "body", J.set(J.get(k, "body"), "vy", J.of_i32(0)))
      +k = J.set(k, "body", J.set(J.get(k, "body"), "mode", J.JS{"Air"}))
      +k = J.set(k, "body", J.set(J.get(k, "body"), "ground", J.JBool{False{}}))
      +k = J.set(k, "body", J.set(J.get(k, "body"), "plank", J.JNull{}))
      +say = J.push(say, J.JS{"wish_grant"})
      +self = bannerpeak.session.Session.touch_pole.if1(U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 4), 0), self)
      (k, (say, self))
    case False{}:
      (k, (say, self))

def bannerpeak.session.Session.near_gate?(+gx: U32, +gy: U32, +fx: U32, +fy: U32, +n: U32) -> Bool:
  bannerpeak.session.Session.near_gate.if2(U32.is_ge(n, bannerpeak.layout.info.gates_len()), gx, gy, fx, fy, n)

def bannerpeak.session.Session.near_gate.if1?(bpc: Bool, +gx: U32, +gy: U32, +fx: U32, +fy: U32, +n: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.session.Session.near_gate(gx, gy, fx, fy, (n + 1 : U32))

def bannerpeak.session.Session.near_gate.if2?(bpc: Bool, +gx: U32, +gy: U32, +fx: U32, +fy: U32, +n: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +cx : U32 = ((bannerpeak.layout.info.gate_x(n) * 16 : U32) + RS.i_div(16, 2) : U32)
      +cy : U32 = ((bannerpeak.layout.info.gate_y(n) * 16 : U32) + RS.i_div(16, 2) : U32)
      bannerpeak.session.Session.near_gate.if1(Bool.and(RS.i_lt(RS.i_abs((fx - cx : U32)), 20), RS.i_lt(RS.i_abs((fy - cy : U32)), 24)), gx, gy, fx, fy, n)

def bannerpeak.session.Session.touch_board?(+self: J.Val, +k: J.Val, +bits: U32, +fx: U32, +fy: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +ax : U32 = ((bannerpeak.layout.info.arch_x() * 16 : U32) + RS.i_div(16, 2) : U32)
  +ay : U32 = ((bannerpeak.layout.info.arch_y() * 16 : U32) + RS.i_div(16, 2) : U32)
  +bx : U32 = ((bannerpeak.layout.info.board_x() * 16 : U32) + RS.i_div(16, 2) : U32)
  +by : U32 = ((bannerpeak.layout.info.board_y() * 16 : U32) + RS.i_div(16, 2) : U32)
  +_b1 = bannerpeak.knight.Knight.use_edge(k, bits)
  +_t2 = bannerpeak.session.Session.touch_board.if1(_b1, k, _b1)
  +_b1 = bannerpeak.u.fst(Bool, J.Val, _t2)
  +k = bannerpeak.u.snd(Bool, J.Val, _t2)
  +self = bannerpeak.session.Session.touch_board.if3(_b1, self, fx, fy, say, ax, ay, bx, by)
  +self = bannerpeak.session.Session.touch_board.if5(Bool.and(Bool.not(J.bool(J.get(self, "seen"))), U32.is_ne(U32.and(J.u32(J.get(self, "doors")), 1), 0)), self, fx, fy, say, bx, by)
  (self, (k, say))

def bannerpeak.session.Session.touch_board.if1?(bpc: Bool, +k: J.Val, +_b1: Bool) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +_b1 : Bool = bannerpeak.knight.Knight.free(k)
      (_b1, k)
    case False{}:
      (_b1, k)

def bannerpeak.session.Session.touch_board.if2?(bpc: Bool, +self: J.Val, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t3 = bannerpeak.session.Session.open_board(self, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t3)
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_board.if3?(bpc: Bool, +self: J.Val, +fx: U32, +fy: U32, +say: J.Val, +ax: U32, +ay: U32, +bx: U32, +by: U32) -> J.Val:
  match bpc:
    case True{}:
      +near_arch : Bool = Bool.and(RS.i_lt(RS.i_abs((fx - ax : U32)), 26), RS.i_lt(RS.i_abs((fy - ay : U32)), 24))
      +near_board : Bool = Bool.and(RS.i_lt(RS.i_abs((fx - bx : U32)), 30), RS.i_lt(RS.i_abs((fy - by : U32)), 24))
      +near_gate : Bool = bannerpeak.session.Session.near_gate(0, 0, fx, fy, 0)
      +self = bannerpeak.session.Session.touch_board.if2(Bool.or(Bool.or(Bool.and(near_arch, U32.is_ne(U32.and(J.u32(J.get(self, "doors")), 1), 0)), near_board), near_gate), self, say)
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_board.if4?(bpc: Bool, +self: J.Val, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t4 = bannerpeak.session.Session.open_board(self, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t4)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t4)
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_board.if5?(bpc: Bool, +self: J.Val, +fx: U32, +fy: U32, +say: J.Val, +bx: U32, +by: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = bannerpeak.session.Session.touch_board.if4(Bool.and(RS.i_lt(RS.i_abs((fx - bx : U32)), 22), RS.i_lt(RS.i_abs((fy - by : U32)), 24)), self, say)
      self
    case False{}:
      self

def bannerpeak.session.Session.fall?(+self: J.Val, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  bannerpeak.session.Session.fall.if2(U32.is_gt(J.u32(J.get(k, "pit")), 0), self, k, say)

def bannerpeak.session.Session.fall.if1?(bpc: Bool, +self: J.Val, +k: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +_t3 = bannerpeak.session.Session.go_down(self, k)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +k : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t3)
      (k, self)
    case False{}:
      +k = J.set(k, "pit", J.JU{14})
      +k = J.set(k, "body", J.set(J.get(k, "body"), "vx", J.of_i32(0)))
      +k = J.set(k, "body", J.set(J.get(k, "body"), "vy", J.of_i32(0)))
      (k, self)

def bannerpeak.session.Session.fall.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (self, (k, say))
    case False{}:
      +k = J.set(k, "pips", J.JU{RS.u_sat_sub(J.u32(J.get(k, "pips")), 1)})
      +k = J.set(k, "chain", J.JU{0})
      +k = J.set(k, "carry", J.JBool{False{}})
      +say = J.push(say, J.JS{"hit"})
      +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
      +fx = bannerpeak.u.fst(U32, U32, _t1)
      +fy = bannerpeak.u.snd(U32, U32, _t1)
      +_t2 = bannerpeak.session.Session.fx(self, "Smoke", fx, RS.i_min(fy, ((64 * 16 : U32) - 8 : U32)))
      +self : J.Val = _t2
      +_t4 = bannerpeak.session.Session.fall.if1(U32.is_eq(J.u32(J.get(k, "pips")), 0), self, k)
      +k = bannerpeak.u.fst(J.Val, J.Val, _t4)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t4)
      (self, (k, say))

def bannerpeak.session.Session.after_hit?(+self: J.Val, +k: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +self = bannerpeak.session.Session.after_hit.if1(U32.is_eq(J.u32(J.get(k, "pips")), 0), self, k)
  (self, k)

def bannerpeak.session.Session.after_hit.if1?(bpc: Bool, +self: J.Val, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.session.Session.go_down(self, k)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t1)
      +k : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t1)
      self
    case False{}:
      self

def bannerpeak.session.Session.go_down?(+self: J.Val, +k: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy = bannerpeak.u.snd(U32, U32, _t1)
  +k = J.set(k, "bubble", J.set(J.set(J.set(J.obj(), "x", J.of_i32((fx * 256 : U32))), "y", J.of_i32(((RS.i_min(fy, ((64 * 16 : U32) - 24 : U32)) - 14 : U32) * 256 : U32))), "t", J.JU{0}))
  +k = J.set(k, "size", J.JS{"Small"})
  +k = J.set(k, "body", J.set(J.get(k, "body"), "big", J.JBool{False{}}))
  +k = J.set(k, "carry", J.JBool{False{}})
  +k = J.set(k, "pit", J.JU{0})
  +k = J.set(k, "pole", J.JU{0})
  +k = J.set(k, "pose", J.JU{0})
  (self, k)

def bannerpeak.session.Session.award?(+self: J.Val, +banner: U32, +k: J.Val, +who: Bool, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  bannerpeak.session.Session.award.if2(U32.is_ne(U32.and(J.u32(J.get(self, "banners")), RS.u_shl(1, banner)), 0), self, banner, k, who, say)

def bannerpeak.session.Session.award.if1?(bpc: Bool, +self: J.Val, +k: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +k = J.set(k, "pose", J.JU{54})
      +k = J.set(k, "score", J.JU{(J.u32(J.get(k, "score")) + 1000 : U32)})
      +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
      +fx = bannerpeak.u.fst(U32, U32, _t1)
      +fy = bannerpeak.u.snd(U32, U32, _t1)
      +_t2 = bannerpeak.session.Session.fx(self, "Gleam", fx, (fy - 24 : U32))
      +self : J.Val = _t2
      (k, self)
    case False{}:
      (k, self)

def bannerpeak.session.Session.award.if2?(bpc: Bool, +self: J.Val, +banner: U32, +k: J.Val, +who: Bool, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (self, (k, say))
    case False{}:
      +self = J.set(self, "banners", J.JU{U32.or(J.u32(J.get(self, "banners")), RS.u_shl(1, banner))})
      +self = J.set(self, "announce", J.set(J.set(J.obj(), "banner", J.JU{banner}), "t", J.JU{110}))
      +say = J.push(say, J.JS{"wish_grant"})
      +_t3 = bannerpeak.session.Session.award.if1(who, self, k)
      +k = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t3)
      (self, (k, say))

def bannerpeak.session.Session.open_board?(+self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  bannerpeak.session.Session.open_board.if1(Bool.not(String.eq(J.str(J.get(self, "phase")), "Play")), self, say)

def bannerpeak.session.Session.open_board.if1?(bpc: Bool, +self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      (self, say)
    case False{}:
      +self = J.set(self, "phase", J.JS{"Select"})
      +self = J.set(self, "phase_t", J.JU{0})
      +self = J.set(self, "seen", J.JBool{True{}})
      +self = J.set(self, "cursor", J.JU{bannerpeak.session.Session.first_line(J.u32(J.get(self, "banners")), 0)})
      +say = J.push(say, J.JS{"play_card"})
      (self, say)

def bannerpeak.session.Session.first_line?(+banners: U32, +line: U32) -> U32:
  bannerpeak.session.Session.first_line.if2(U32.is_ge(line, 6), banners, line)

def bannerpeak.session.Session.first_line.if1?(bpc: Bool, +banners: U32, +line: U32) -> U32:
  match bpc:
    case True{}:
      line
    case False{}:
      bannerpeak.session.Session.first_line(banners, (line + 1 : U32))

def bannerpeak.session.Session.first_line.if2?(bpc: Bool, +banners: U32, +line: U32) -> U32:
  match bpc:
    case True{}:
      0
    case False{}:
      bannerpeak.session.Session.first_line.if1(U32.is_eq(U32.and(banners, RS.u_shl(1, bannerpeak.u.board_banner(line))), 0), banners, line)

def bannerpeak.session.Session.select_step?(+self: J.Val, +inputs: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +self = J.set(self, "phase_t", J.JU{RS.u_min((J.u32(J.get(self, "phase_t")) + 1 : U32), 108000)})
  +_t1 = bannerpeak.session.Session.select_seats(self, bannerpeak.u.ids(J.get(self, "seats")), inputs, say)
  +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
  +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
  +go0 : Bool = bannerpeak.u.fst(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t1)
  +go : Bool = go0
  +go = bannerpeak.session.Session.select_step.if1(U32.is_ge(J.u32(J.get(self, "phase_t")), 240), go)
  bannerpeak.session.Session.select_step.if4(Bool.not(go), self, say)

def bannerpeak.session.Session.select_step.if1?(bpc: Bool, +go: Bool) -> Bool:
  match bpc:
    case True{}:
      +go : Bool = True{}
      go
    case False{}:
      go

def bannerpeak.session.Session.select_step.if2?(bpc: Bool, +self: J.Val, +gate: U32) -> U32:
  match bpc:
    case True{}:
      +gate : U32 = bannerpeak.session.Session.spark_gate(self)
      gate
    case False{}:
      gate

def bannerpeak.session.Session.select_step.if3?(bpc: Bool, +self: J.Val, +say: J.Val, +line: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "phase_t", J.JU{0})
      +say = J.push(say, J.JS{"mana_empty"})
      (self, say)
    case False{}:
      +gate : U32 = bannerpeak.u.board_gate(line)
      +gate = bannerpeak.session.Session.select_step.if2(U32.is_eq(line, 4), self, gate)
      +self = J.set(self, "checkpoint", J.JU{(gate + 1 : U32)})
      +self = J.set(self, "gates", J.JU{U32.or(J.u32(J.get(self, "gates")), RS.u_shl(1, gate))})
      +_t2 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.session.Session.station(self)
      +x = bannerpeak.u.fst(U32, U32, _t2)
      +y = bannerpeak.u.snd(U32, U32, _t2)
      +_t3 = bannerpeak.session.Session.place_seats(self, bannerpeak.u.ids(J.get(self, "seats")), 0, x, y)
      +self : J.Val = _t3
      +self = J.set(self, "boulders", bannerpeak.u.vec_new())
      +self = J.set(self, "bolts", bannerpeak.u.vec_new())
      +self = J.set(self, "phase", J.JS{"Play"})
      +self = J.set(self, "phase_t", J.JU{0})
      +say = J.push(say, J.JS{"door_open"})
      (self, say)

def bannerpeak.session.Session.select_step.if4?(bpc: Bool, +self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      (self, say)
    case False{}:
      +line : U32 = J.u32(J.get(self, "cursor"))
      bannerpeak.session.Session.select_step.if3(Bool.and(U32.is_eq(line, 5), U32.is_eq(U32.and(J.u32(J.get(self, "doors")), 2), 0)), self, say, line)

def bannerpeak.session.Session.select_seats?(+self: J.Val, +ids: List<&2, U32>, +inputs: J.Val, +say: J.Val) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  bannerpeak.session.Session.select_seats.if4(bannerpeak.u.is_empty(ids), self, ids, inputs, say)

def bannerpeak.session.Session.select_seats.if1?(bpc: Bool, +self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "cursor", J.JU{U32.mod(((J.u32(J.get(self, "cursor")) + 6 : U32) - 1 : U32), 6)})
      +say = J.push(say, J.JS{"card_pickup"})
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.select_seats.if2?(bpc: Bool, +self: J.Val, +say: J.Val, +i: J.Val, +k: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "cursor", J.JU{U32.mod((J.u32(J.get(self, "cursor")) + 1 : U32), 6)})
      +say = J.push(say, J.JS{"card_pickup"})
      (say, self)
    case False{}:
      +_t2 = bannerpeak.session.Session.select_seats.if1(Bool.and(RS.i_lt(J.u32(J.get(i, "move_y")), 0), U32.is_eq(U32.and(J.u32(J.get(k, "prev")), 1), 0)), self, say)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t2)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t2)
      (say, self)

def bannerpeak.session.Session.select_seats.if3?(bpc: Bool, +go: Bool) -> Bool:
  match bpc:
    case True{}:
      +go : Bool = True{}
      go
    case False{}:
      go

def bannerpeak.session.Session.select_seats.if4?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +inputs: J.Val, +say: J.Val) -> Sigma<&2, &2, Bool, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (False{}, (self, say))
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +i : J.Val = bannerpeak.session.Session.input_of(inputs, id)
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +_t1 : Sigma<&2, &2, J.Val, _ => U32> = bannerpeak.knight.Knight.cmd(k, i)
      +c0 = bannerpeak.u.fst(J.Val, U32, _t1)
      +bits = bannerpeak.u.snd(J.Val, U32, _t1)
      +go : Bool = False{}
      +_t3 = bannerpeak.session.Session.select_seats.if2(Bool.and(U32.is_ne(U32.and(bits, 2), 0), U32.is_eq(U32.and(J.u32(J.get(k, "prev")), 2), 0)), self, say, i, k)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t3)
      +go = bannerpeak.session.Session.select_seats.if3(Bool.and(U32.is_ne(U32.and(bits, 8), 0), U32.is_eq(U32.and(J.u32(J.get(k, "prev")), 8), 0)), go)
      +k = J.set(k, "prev", J.JU{bits})
      +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), id, k))
      +_t4 = bannerpeak.session.Session.select_seats(self, bannerpeak.u.tail(ids), inputs, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t4))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t4))
      +rest : Bool = bannerpeak.u.fst(Bool, Sigma<&2, &2, J.Val, _ => J.Val>, _t4)
      (Bool.or(go, rest), (self, say))

def bannerpeak.session.Session.place_seats?(+self: J.Val, +ids: List<&2, U32>, +n: U32, +x: U32, +y: U32) -> J.Val:
  bannerpeak.session.Session.place_seats.if2(bannerpeak.u.is_empty(ids), self, ids, n, x, y)

def bannerpeak.session.Session.place_seats.if1?(bpc: Bool, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "pips", J.JU{3})
      k
    case False{}:
      k

def bannerpeak.session.Session.place_seats.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +n: U32, +x: U32, +y: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +off : U32 = bannerpeak.session.Session.offset(n)
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +k = bannerpeak.session.Session.place_seats.if1(Bool.not(J.is_null(J.get(k, "bubble"))), k)
      +k = J.set(k, "bubble", J.JNull{})
      +k = J.set(k, "pit", J.JU{0})
      +k = J.set(k, "pole", J.JU{0})
      +_t1 = bannerpeak.body.Body.place(J.get(k, "body"), (x + off : U32), y)
      +k = J.set(k, "body", _t1)
      +k = J.set(k, "hurt", J.JU{70})
      +_j2 : Sigma<&2, &2, U32, _ => U32> = ((x + off : U32), y)
      +k = J.set(k, "safe", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j2)), J.of_i32(bannerpeak.u.snd(U32, U32, _j2))]})
      +_j3 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.knight.follow((J.u32(J.nth(J.get(k, "cam"), 0)), J.u32(J.nth(J.get(k, "cam"), 1))), x, y, 1, 0, 0)
      +k = J.set(k, "cam", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j3)), J.of_i32(bannerpeak.u.snd(U32, U32, _j3))]})
      +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), id, k))
      +_t4 = bannerpeak.session.Session.place_seats(self, bannerpeak.u.tail(ids), (n + 1 : U32), x, y)
      +self : J.Val = _t4
      self

def bannerpeak.session.Session.start_finale?(+self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +self = J.set(self, "phase", J.JS{"Finale"})
  +self = J.set(self, "phase_t", J.JU{0})
  +fx : U32 = bannerpeak.layout.cell_feet_x(bannerpeak.layout.info.finale_x())
  +fy : U32 = bannerpeak.layout.cell_feet_y(bannerpeak.layout.info.finale_y())
  +ids : List<&2, U32> = bannerpeak.u.ids(J.get(self, "seats"))
  +_t1 = bannerpeak.session.Session.finale_seats(self, ids, 0, bannerpeak.u.map_len(J.get(self, "seats")), fx, fy)
  +self : J.Val = _t1
  +self = J.set(self, "bolts", bannerpeak.u.vec_new())
  +self = J.set(self, "boulders", bannerpeak.u.vec_new())
  +say = J.push(say, J.JS{"play_card"})
  (self, say)

def bannerpeak.session.Session.finale_seats?(+self: J.Val, +ids: List<&2, U32>, +n: U32, +count: U32, +fx: U32, +fy: U32) -> J.Val:
  bannerpeak.session.Session.finale_seats.if2(bannerpeak.u.is_empty(ids), self, ids, n, count, fx, fy)

def bannerpeak.session.Session.finale_seats.if1?(bpc: Bool, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "body", J.set(J.get(k, "body"), "face", J.of_i32(1)))
      k
    case False{}:
      +k = J.set(k, "body", J.set(J.get(k, "body"), "face", J.of_i32(4294967295)))
      k

def bannerpeak.session.Session.finale_seats.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +n: U32, +count: U32, +fx: U32, +fy: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +off : U32 = ((n - RS.i_div(count, 2) : U32) * 14 : U32)
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +k = J.set(k, "bubble", J.JNull{})
      +k = J.set(k, "pit", J.JU{0})
      +k = J.set(k, "pole", J.JU{0})
      +k = J.set(k, "pose", J.JU{0})
      +_t1 = bannerpeak.body.Body.place(J.get(k, "body"), (fx + off : U32), fy)
      +k = J.set(k, "body", _t1)
      +k = bannerpeak.session.Session.finale_seats.if1(U32.is_eq(RS.i_rem(n, 2), 0), k)
      +_j2 : Sigma<&2, &2, U32, _ => U32> = ((fx + off : U32), fy)
      +k = J.set(k, "safe", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j2)), J.of_i32(bannerpeak.u.snd(U32, U32, _j2))]})
      +_j3 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.knight.follow((J.u32(J.nth(J.get(k, "cam"), 0)), J.u32(J.nth(J.get(k, "cam"), 1))), fx, fy, 1, 0, 0)
      +k = J.set(k, "cam", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j3)), J.of_i32(bannerpeak.u.snd(U32, U32, _j3))]})
      +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), id, k))
      +_t4 = bannerpeak.session.Session.finale_seats(self, bannerpeak.u.tail(ids), (n + 1 : U32), count, fx, fy)
      +self : J.Val = _t4
      self

def bannerpeak.session.Session.finale_step?(+self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +self = J.set(self, "phase_t", J.JU{RS.u_min((J.u32(J.get(self, "phase_t")) + 1 : U32), 108000)})
  +t : U32 = J.u32(J.get(self, "phase_t"))
  +say = bannerpeak.session.Session.finale_step.if1(Bool.or(Bool.or(U32.is_eq(t, 30), U32.is_eq(t, 60)), U32.is_eq(t, 90)), say)
  +_t2 = bannerpeak.session.Session.finale_step.if2(U32.is_eq(t, 120), self, say)
  +say = bannerpeak.u.fst(J.Val, J.Val, _t2)
  +self = bannerpeak.u.snd(J.Val, J.Val, _t2)
  +_t3 = bannerpeak.session.Session.finale_cams(self, bannerpeak.u.ids(J.get(self, "seats")))
  +self : J.Val = _t3
  +_t4 = bannerpeak.session.Session.finale_step.if3(Bool.and(U32.is_ge(t, 210), J.is_null(J.get(self, "completed"))), self, say)
  +say = bannerpeak.u.fst(J.Val, J.Val, _t4)
  +self = bannerpeak.u.snd(J.Val, J.Val, _t4)
  (self, say)

def bannerpeak.session.Session.finale_step.if1?(bpc: Bool, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"shield_up"})
      say
    case False{}:
      say

def bannerpeak.session.Session.finale_step.if2?(bpc: Bool, +self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"wish_grant"})
      +px_ : U32 = ((bannerpeak.layout.info.great_pole_x() * 16 : U32) + RS.i_div(16, 2) : U32)
      +py_ : U32 = ((bannerpeak.layout.info.great_pole_y() * 16 : U32) + RS.i_div(16, 2) : U32)
      +_t1 = bannerpeak.session.Session.fx(self, "Gleam", px_, (py_ - 150 : U32))
      +self : J.Val = _t1
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.finale_step.if3?(bpc: Bool, +self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "completed", J.JU{J.u32(J.get(self, "elapsed"))})
      +say = J.push(say, J.JS{"play_card"})
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.finale_cams?(+self: J.Val, +ids: List<&2, U32>) -> J.Val:
  bannerpeak.session.Session.finale_cams.if1(bannerpeak.u.is_empty(ids), self, ids)

def bannerpeak.session.Session.finale_cams.if1?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
      +fx = bannerpeak.u.fst(U32, U32, _t1)
      +fy = bannerpeak.u.snd(U32, U32, _t1)
      +_j2 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.knight.follow((J.u32(J.nth(J.get(k, "cam"), 0)), J.u32(J.nth(J.get(k, "cam"), 1))), fx, fy, J.u32(J.get(J.get(k, "body"), "face")), 0, 0)
      +k = J.set(k, "cam", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j2)), J.of_i32(bannerpeak.u.snd(U32, U32, _j2))]})
      +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), id, k))
      +_t3 = bannerpeak.session.Session.finale_cams(self, bannerpeak.u.tail(ids))
      +self : J.Val = _t3
      self

def bannerpeak.session.Session.step_doors?(+self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +_t2 = bannerpeak.session.Session.step_doors.if1(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "doors")), 1), 0), U32.is_eq(U32.and(J.u32(J.get(self, "banners")), 3), 3)), self, say)
  +say = bannerpeak.u.fst(J.Val, J.Val, _t2)
  +self = bannerpeak.u.snd(J.Val, J.Val, _t2)
  +_t4 = bannerpeak.session.Session.step_doors.if2(Bool.and(U32.is_eq(U32.and(J.u32(J.get(self, "doors")), 2), 0), U32.is_ge(bannerpeak.session.Session.door_count(self), 7)), self, say)
  +say = bannerpeak.u.fst(J.Val, J.Val, _t4)
  +self = bannerpeak.u.snd(J.Val, J.Val, _t4)
  (self, say)

def bannerpeak.session.Session.step_doors.if1?(bpc: Bool, +self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "doors", J.JU{U32.or(J.u32(J.get(self, "doors")), 1)})
      +_t1 = bannerpeak.session.Session.open_gates(self, 0)
      +self : J.Val = _t1
      +say = J.push(say, J.JS{"door_open"})
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.step_doors.if2?(bpc: Bool, +self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +self = J.set(self, "doors", J.JU{U32.or(J.u32(J.get(self, "doors")), 2)})
      +_t3 = bannerpeak.session.Session.open_gates(self, 1)
      +self : J.Val = _t3
      +say = J.push(say, J.JS{"door_open"})
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.open_gates?(+self: J.Val, +which: U32) -> J.Val:
  +_t1 = bannerpeak.session.Session.open_gates_row(self, which, 0)
  +self : J.Val = _t1
  self

def bannerpeak.session.Session.open_gates_row?(+self: J.Val, +which: U32, +ty: U32) -> J.Val:
  bannerpeak.session.Session.open_gates_row.if1(RS.i_ge(ty, 64), self, which, ty)

def bannerpeak.session.Session.open_gates_row.if1?(bpc: Bool, +self: J.Val, +which: U32, +ty: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +_t1 = bannerpeak.session.Session.open_gates_col(self, which, ty, 0)
      +self : J.Val = _t1
      +_t2 = bannerpeak.session.Session.open_gates_row(self, which, (ty + 1 : U32))
      +self : J.Val = _t2
      self

def bannerpeak.session.Session.open_gates_col?(+self: J.Val, +which: U32, +ty: U32, +tx: U32) -> J.Val:
  bannerpeak.session.Session.open_gates_col.if2(RS.i_ge(tx, 434), self, which, ty, tx)

def bannerpeak.session.Session.open_gates_col.if1?(bpc: Bool, +self: J.Val, +ty: U32, +tx: U32) -> J.Val:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.session.Session.set_tile(self, tx, ty, 0)
      +self : J.Val = _t1
      self
    case False{}:
      self

def bannerpeak.session.Session.open_gates_col.if2?(bpc: Bool, +self: J.Val, +which: U32, +ty: U32, +tx: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +self = bannerpeak.session.Session.open_gates_col.if1(U32.is_eq(bannerpeak.world.World.at(tx, ty), (12 + which : U32)), self, ty, tx)
      +_t2 = bannerpeak.session.Session.open_gates_col(self, which, ty, (tx + 1 : U32))
      +self : J.Val = _t2
      self

def bannerpeak.session.Session.step_pole?(+self: J.Val, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +base : U32 = (((bannerpeak.layout.info.pole_y() + 1 : U32) * 16 : U32) * 256 : U32)
  +k = J.set(k, "pole", J.JU{(J.u32(J.get(k, "pole")) + 1 : U32)})
  +k = J.set(k, "body", J.set(J.get(k, "body"), "vx", J.of_i32(0)))
  +_t2 = bannerpeak.session.Session.step_pole.if6(RS.i_lt(J.u32(J.get(J.get(k, "body"), "y")), base), self, k, say, base)
  +k = bannerpeak.u.fst(J.Val, J.Val, _t2)
  +say = bannerpeak.u.snd(J.Val, J.Val, _t2)
  (self, (k, say))

def bannerpeak.session.Session.step_pole.if1?(bpc: Bool, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "body", J.set(J.get(k, "body"), "ground", J.JBool{True{}}))
      +k = J.set(k, "body", J.set(J.get(k, "body"), "mode", J.JS{"Ground"}))
      +k = J.set(k, "body", J.set(J.get(k, "body"), "vy", J.of_i32(0)))
      +k = J.set(k, "pole", J.JU{RS.u_max(J.u32(J.get(k, "pole")), 40)})
      k
    case False{}:
      k

def bannerpeak.session.Session.step_pole.if2?(bpc: Bool, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "body", J.set(J.get(k, "body"), "x", J.of_i32((J.u32(J.get(J.get(k, "body"), "x")) - 560 : U32))))
      k
    case False{}:
      k

def bannerpeak.session.Session.step_pole.if3?(bpc: Bool, +self: J.Val, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "body", J.set(J.get(k, "body"), "face", J.of_i32(1)))
      +k = J.set(k, "body", J.set(J.get(k, "body"), "x", J.of_i32((J.u32(J.get(J.get(k, "body"), "x")) + 560 : U32))))
      +g : J.Val = bannerpeak.session.Session.view(self)
      +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
      +fx = bannerpeak.u.fst(U32, U32, _t1)
      +fy = bannerpeak.u.snd(U32, U32, _t1)
      +k = bannerpeak.session.Session.step_pole.if2(bannerpeak.world.Tile.solid(bannerpeak.session.View.tile(g, RS.i_div_euclid((fx + 8 : U32), 16), RS.i_div_euclid((fy - 4 : U32), 16))), k)
      k
    case False{}:
      k

def bannerpeak.session.Session.step_pole.if4?(bpc: Bool, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"door_pass"})
      say
    case False{}:
      say

def bannerpeak.session.Session.step_pole.if5?(bpc: Bool, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "pole", J.JU{0})
      +k = J.set(k, "poled", J.JBool{True{}})
      +k = J.set(k, "body", J.set(J.get(k, "body"), "mode", J.JS{"Ground"}))
      k
    case False{}:
      k

def bannerpeak.session.Session.step_pole.if6?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val, +base: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +k = J.set(k, "body", J.set(J.get(k, "body"), "y", J.of_i32(RS.i_min((J.u32(J.get(J.get(k, "body"), "y")) + 640 : U32), base))))
      +k = bannerpeak.session.Session.step_pole.if1(U32.is_eq(J.u32(J.get(J.get(k, "body"), "y")), base), k)
      (k, say)
    case False{}:
      +k = bannerpeak.session.Session.step_pole.if3(Bool.and(U32.is_ge(J.u32(J.get(k, "pole")), 60), U32.is_lt(J.u32(J.get(k, "pole")), 100)), self, k)
      +say = bannerpeak.session.Session.step_pole.if4(U32.is_eq(J.u32(J.get(k, "pole")), 60), say)
      +k = bannerpeak.session.Session.step_pole.if5(U32.is_ge(J.u32(J.get(k, "pole")), 100), k)
      (k, say)

def case.bannerpeak.session.Session.touch?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.touch(J.get(input, "session"), J.u32(J.get(input, "id")), J.get(input, "k"), J.u32(J.get(input, "bits")), J.u32(J.get(input, "was")), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)), "k", bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r))), "say", bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)))

def case.bannerpeak.session.Session.fall?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.fall(J.get(input, "session"), J.get(input, "k"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)), "k", bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r))), "say", bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)))

def case.bannerpeak.session.Session.after_hit?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.after_hit(J.get(input, "session"), J.get(input, "k"))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "k", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.go_down?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.go_down(J.get(input, "session"), J.get(input, "k"))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "k", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.award?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.award(J.get(input, "session"), J.u32(J.get(input, "banner")), J.get(input, "k"), J.bool(J.get(input, "who")), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)), "k", bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r))), "say", bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)))

def case.bannerpeak.session.Session.open_board?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.open_board(J.get(input, "session"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "say", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.select_step?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.select_step(J.get(input, "session"), J.get(input, "inputs"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "say", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.start_finale?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.start_finale(J.get(input, "session"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "say", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.finale_step?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.finale_step(J.get(input, "session"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "say", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.step_doors?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.step_doors(J.get(input, "session"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "say", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.open_gates?(+input: J.Val) -> J.Val:
  bannerpeak.session.Session.open_gates(J.get(input, "session"), J.u32(J.get(input, "which")))

def case.bannerpeak.session.Session.step_pole?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.step_pole(J.get(input, "session"), J.get(input, "k"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)), "k", bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r))), "say", bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)))

def bannerpeak.session.Session.alive_feet?(+self: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.alive_feet.if2(bannerpeak.u.is_empty(ids), self, ids, acc)

def bannerpeak.session.Session.alive_feet.if1?(bpc: Bool, +acc: J.Val, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_j1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
      +acc = J.push(acc, J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j1)), J.of_i32(bannerpeak.u.snd(U32, U32, _j1))]})
      acc
    case False{}:
      acc

def bannerpeak.session.Session.alive_feet.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), bannerpeak.u.head(ids))
      +acc = bannerpeak.session.Session.alive_feet.if1(bannerpeak.knight.Knight.alive(k), acc, k)
      bannerpeak.session.Session.alive_feet(self, bannerpeak.u.tail(ids), acc)

def bannerpeak.session.Session.close_to?(+near: J.Val, +n: U32, +fx: U32, +fy: U32) -> Bool:
  bannerpeak.session.Session.close_to.if2(U32.is_ge(n, J.len(near)), near, n, fx, fy)

def bannerpeak.session.Session.close_to.if1?(bpc: Bool, +near: J.Val, +n: U32, +fx: U32, +fy: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.session.Session.close_to(near, (n + 1 : U32), fx, fy)

def bannerpeak.session.Session.close_to.if2?(bpc: Bool, +near: J.Val, +n: U32, +fx: U32, +fy: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +p : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.nth(near, n), 0)), J.u32(J.nth(J.nth(near, n), 1)))
      +_t1 : Sigma<&2, &2, U32, _ => U32> = p
      +_t2 : Sigma<&2, &2, U32, _ => U32> = p
      bannerpeak.session.Session.close_to.if1(Bool.and(RS.i_lt(RS.i_abs((bannerpeak.u.fst(U32, U32, _t1) - fx : U32)), 340), RS.i_lt(RS.i_abs((bannerpeak.u.snd(U32, U32, _t2) - fy : U32)), 260)), near, n, fx, fy)

def bannerpeak.session.Session.step_things?(+self: J.Val, +knobs: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +near : J.Val = bannerpeak.session.Session.alive_feet(self, bannerpeak.u.ids(J.get(self, "seats")), bannerpeak.u.vec_new())
  +self = J.set(self, "foes", bannerpeak.session.Session.drop_popped(J.get(self, "foes"), 0, bannerpeak.u.vec_new()))
  +_t1 = bannerpeak.session.Session.step_foes(self, 0, near)
  +self : J.Val = _t1
  +_t2 = bannerpeak.session.Session.step_items(self, 0)
  +self : J.Val = _t2
  +self = J.set(self, "bolts", bannerpeak.session.Session.step_bolts(self, 0, bannerpeak.u.vec_new()))
  +limit : U32 = (((64 * 16 : U32) + 64 : U32) * 256 : U32)
  +self = J.set(self, "foes", bannerpeak.session.Session.above(J.get(self, "foes"), 0, limit, bannerpeak.u.vec_new()))
  +self = J.set(self, "items", bannerpeak.session.Session.above_items(J.get(self, "items"), 0, limit, bannerpeak.u.vec_new()))
  +_t3 : Sigma<&2, &2, J.Val, _ => J.Val> = bannerpeak.session.Session.bolt_hits(self, 0, 0, bannerpeak.u.vec_new(), bannerpeak.u.vec_new())
  +bks = bannerpeak.u.fst(J.Val, J.Val, _t3)
  +fks = bannerpeak.u.snd(J.Val, J.Val, _t3)
  +_t4 = bannerpeak.session.Session.apply_bolt_hits(self, bks, fks, 0, say)
  +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t4)
  +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t4)
  +self = J.set(self, "bolts", bannerpeak.session.Session.bolts_kept(J.get(self, "bolts"), 0, bannerpeak.u.vec_new()))
  +flipped : J.Val = bannerpeak.session.Session.cap_flips(self, 0, 0, bannerpeak.u.vec_new())
  +_t5 = bannerpeak.session.Session.apply_flips(self, flipped, 0, say)
  +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t5)
  +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t5)
  +feet : J.Val = bannerpeak.session.Session.alive_feet(self, bannerpeak.u.ids(J.get(self, "seats")), bannerpeak.u.vec_new())
  +seats : U32 = RS.u_max(bannerpeak.session.Session.seat_count(self), 1)
  +beat : String = J.str(J.get(J.get(self, "boss"), "beat"))
  +_t6 = bannerpeak.boss.Boss.step(J.get(self, "boss"), feet, seats, J.u32(J.get(self, "elapsed")))
  +self = J.set(self, "boss", _t6)
  +say = bannerpeak.session.Session.step_things.if1(Bool.and(String.eq(beat, "Dormant"), Bool.not(String.eq(J.str(J.get(J.get(self, "boss"), "beat")), "Dormant"))), say)
  +say = bannerpeak.session.Session.step_things.if2(Bool.and(String.eq(beat, "Rest"), String.eq(J.str(J.get(J.get(self, "boss"), "beat")), "Tell")), say)
  +say = bannerpeak.session.Session.step_things.if3(Bool.and(String.eq(J.str(J.get(J.get(self, "boss"), "beat")), "Strike"), String.eq(beat, "Tell")), say)
  +_t7 = bannerpeak.session.Session.step_boulders(self, feet, say)
  +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t7)
  +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t7)
  (self, say)

def bannerpeak.session.Session.step_things.if1?(bpc: Bool, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"boss_rise"})
      say
    case False{}:
      say

def bannerpeak.session.Session.step_things.if2?(bpc: Bool, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"boss_rage"})
      say
    case False{}:
      say

def bannerpeak.session.Session.step_things.if3?(bpc: Bool, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"rock_land"})
      say
    case False{}:
      say

def bannerpeak.session.Session.drop_popped?(+foes: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.drop_popped.if2(U32.is_ge(n, J.len(foes)), foes, n, acc)

def bannerpeak.session.Session.drop_popped.if1?(bpc: Bool, +foes: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.nth(foes, n))
      acc
    case False{}:
      acc

def bannerpeak.session.Session.drop_popped.if2?(bpc: Bool, +foes: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = bannerpeak.session.Session.drop_popped.if1(U32.is_ne(J.u32(J.get(J.nth(foes, n), "dead")), 1), foes, n, acc)
      bannerpeak.session.Session.drop_popped(foes, (n + 1 : U32), acc)

def bannerpeak.session.Session.step_foes?(+self: J.Val, +n: U32, +near: J.Val) -> J.Val:
  bannerpeak.session.Session.step_foes.if1(U32.is_ge(n, J.len(J.get(self, "foes"))), self, n, near)

def bannerpeak.session.Session.step_foes.if1?(bpc: Bool, +self: J.Val, +n: U32, +near: J.Val) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +f : J.Val = J.nth(J.get(self, "foes"), n)
      +fx : U32 = RS.i_div_euclid(J.u32(J.get(f, "x")), 256)
      +fy : U32 = RS.i_div_euclid(J.u32(J.get(f, "y")), 256)
      +close : Bool = bannerpeak.session.Session.close_to(near, 0, fx, fy)
      +_t1 = bannerpeak.foes.Foe.step(f, J.get(self, "changed"), close)
      +f : J.Val = _t1
      +self = J.set(self, "foes", J.set_nth(J.get(self, "foes"), n, f))
      +_t2 = bannerpeak.session.Session.step_foes(self, (n + 1 : U32), near)
      +self : J.Val = _t2
      self

def bannerpeak.session.Session.step_items?(+self: J.Val, +n: U32) -> J.Val:
  bannerpeak.session.Session.step_items.if1(U32.is_ge(n, J.len(J.get(self, "items"))), self, n)

def bannerpeak.session.Session.step_items.if1?(bpc: Bool, +self: J.Val, +n: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +i : J.Val = J.nth(J.get(self, "items"), n)
      +_t1 = bannerpeak.foes.Item.step(i, J.get(self, "changed"))
      +i : J.Val = _t1
      +self = J.set(self, "items", J.set_nth(J.get(self, "items"), n, i))
      +_t2 = bannerpeak.session.Session.step_items(self, (n + 1 : U32))
      +self : J.Val = _t2
      self

def bannerpeak.session.Session.step_bolts?(+self: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.step_bolts.if2(U32.is_ge(n, J.len(J.get(self, "bolts"))), self, n, acc)

def bannerpeak.session.Session.step_bolts.if1?(bpc: Bool, +acc: J.Val, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, b)
      acc
    case False{}:
      acc

def bannerpeak.session.Session.step_bolts.if2?(bpc: Bool, +self: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +b : J.Val = J.nth(J.get(self, "bolts"), n)
      +_t1 = bannerpeak.foes.Bolt.step(b, J.get(self, "changed"))
      +b : J.Val = bannerpeak.u.snd(Bool, J.Val, _t1)
      +alive : Bool = bannerpeak.u.fst(Bool, J.Val, _t1)
      +acc = bannerpeak.session.Session.step_bolts.if1(alive, acc, b)
      bannerpeak.session.Session.step_bolts(self, (n + 1 : U32), acc)

def bannerpeak.session.Session.above?(+foes: J.Val, +n: U32, +limit: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.above.if2(U32.is_ge(n, J.len(foes)), foes, n, limit, acc)

def bannerpeak.session.Session.above.if1?(bpc: Bool, +foes: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.nth(foes, n))
      acc
    case False{}:
      acc

def bannerpeak.session.Session.above.if2?(bpc: Bool, +foes: J.Val, +n: U32, +limit: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = bannerpeak.session.Session.above.if1(RS.i_lt(J.u32(J.get(J.nth(foes, n), "y")), limit), foes, n, acc)
      bannerpeak.session.Session.above(foes, (n + 1 : U32), limit, acc)

def bannerpeak.session.Session.above_items?(+items: J.Val, +n: U32, +limit: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.above_items.if2(U32.is_ge(n, J.len(items)), items, n, limit, acc)

def bannerpeak.session.Session.above_items.if1?(bpc: Bool, +items: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.nth(items, n))
      acc
    case False{}:
      acc

def bannerpeak.session.Session.above_items.if2?(bpc: Bool, +items: J.Val, +n: U32, +limit: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = bannerpeak.session.Session.above_items.if1(RS.i_lt(J.u32(J.get(J.nth(items, n), "y")), limit), items, n, acc)
      bannerpeak.session.Session.above_items(items, (n + 1 : U32), limit, acc)

def bannerpeak.session.Session.bolt_hits?(+self: J.Val, +bn: U32, +fnn: U32, +bs: J.Val, +fs: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  bannerpeak.session.Session.bolt_hits.if3(U32.is_ge(bn, J.len(J.get(self, "bolts"))), self, bn, fnn, bs, fs)

def bannerpeak.session.Session.bolt_hits.if1?(bpc: Bool, +bn: U32, +fnn: U32, +bs: J.Val, +fs: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +bs = J.push(bs, J.JU{bn})
      +fs = J.push(fs, J.JU{fnn})
      (bs, fs)
    case False{}:
      (bs, fs)

def bannerpeak.session.Session.bolt_hits.if2?(bpc: Bool, +self: J.Val, +bn: U32, +fnn: U32, +bs: J.Val, +fs: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      bannerpeak.session.Session.bolt_hits(self, (bn + 1 : U32), 0, bs, fs)
    case False{}:
      +f : J.Val = J.nth(J.get(self, "foes"), fnn)
      +_t1 = bannerpeak.session.Session.bolt_hits.if1(Bool.and(bannerpeak.foes.Foe.alive(f), bannerpeak.session.overlap(bannerpeak.foes.Bolt.rect(J.nth(J.get(self, "bolts"), bn)), bannerpeak.foes.Foe.rect(f))), bn, fnn, bs, fs)
      +bs = bannerpeak.u.fst(J.Val, J.Val, _t1)
      +fs = bannerpeak.u.snd(J.Val, J.Val, _t1)
      bannerpeak.session.Session.bolt_hits(self, bn, (fnn + 1 : U32), bs, fs)

def bannerpeak.session.Session.bolt_hits.if3?(bpc: Bool, +self: J.Val, +bn: U32, +fnn: U32, +bs: J.Val, +fs: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      (bs, fs)
    case False{}:
      bannerpeak.session.Session.bolt_hits.if2(U32.is_ge(fnn, J.len(J.get(self, "foes"))), self, bn, fnn, bs, fs)

def bannerpeak.session.Session.apply_bolt_hits?(+self: J.Val, +bs: J.Val, +fs: J.Val, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  bannerpeak.session.Session.apply_bolt_hits.if3(U32.is_ge(n, J.len(bs)), self, bs, fs, n, say)

def bannerpeak.session.Session.apply_bolt_hits.if1?(bpc: Bool, +self: J.Val, +owner: U32) -> J.Val:
  match bpc:
    case True{}:
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), owner)
      +k = J.set(k, "score", J.JU{(J.u32(J.get(k, "score")) + 100 : U32)})
      +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), owner, k))
      self
    case False{}:
      self

def bannerpeak.session.Session.apply_bolt_hits.if2?(bpc: Bool, +self: J.Val, +say: J.Val, +bn: U32, +fnn: U32, +f: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.foes.Foe.defeat(f)
      +f : J.Val = _t1
      +self = J.set(self, "foes", J.set_nth(J.get(self, "foes"), fnn, f))
      +_t2 = bannerpeak.session.Session.fx(self, "Pop", RS.i_div_euclid(J.u32(J.get(f, "x")), 256), RS.i_div_euclid(J.u32(J.get(f, "y")), 256))
      +self : J.Val = _t2
      +say = J.push(say, J.JS{"kill"})
      +owner : U32 = J.u32(J.get(J.nth(J.get(self, "bolts"), bn), "owner"))
      +self = bannerpeak.session.Session.apply_bolt_hits.if1(bannerpeak.u.has(J.get(self, "seats"), owner), self, owner)
      (f, (say, self))
    case False{}:
      (f, (say, self))

def bannerpeak.session.Session.apply_bolt_hits.if3?(bpc: Bool, +self: J.Val, +bs: J.Val, +fs: J.Val, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      (self, say)
    case False{}:
      +bn : U32 = J.u32(J.nth(bs, n))
      +fnn : U32 = J.u32(J.nth(fs, n))
      +f : J.Val = J.nth(J.get(self, "foes"), fnn)
      +_t3 = bannerpeak.session.Session.apply_bolt_hits.if2(bannerpeak.foes.Foe.alive(f), self, say, bn, fnn, f)
      +f = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
      +b : J.Val = J.nth(J.get(self, "bolts"), bn)
      +b = J.set(b, "t", J.JU{(70 + 1 : U32)})
      +self = J.set(self, "bolts", J.set_nth(J.get(self, "bolts"), bn, b))
      +_t4 = bannerpeak.session.Session.apply_bolt_hits(self, bs, fs, (n + 1 : U32), say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t4)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t4)
      (self, say)

def bannerpeak.session.Session.bolts_kept?(+bolts: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.bolts_kept.if2(U32.is_ge(n, J.len(bolts)), bolts, n, acc)

def bannerpeak.session.Session.bolts_kept.if1?(bpc: Bool, +bolts: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.nth(bolts, n))
      acc
    case False{}:
      acc

def bannerpeak.session.Session.bolts_kept.if2?(bpc: Bool, +bolts: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = bannerpeak.session.Session.bolts_kept.if1(U32.is_le(J.u32(J.get(J.nth(bolts, n), "t")), 70), bolts, n, acc)
      bannerpeak.session.Session.bolts_kept(bolts, (n + 1 : U32), acc)

def bannerpeak.session.Session.cap_flips?(+self: J.Val, +n: U32, +fnn: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.cap_flips.if3(U32.is_ge(n, J.len(J.get(self, "items"))), self, n, fnn, acc)

def bannerpeak.session.Session.cap_flips.if1?(bpc: Bool, +fnn: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.JU{fnn})
      acc
    case False{}:
      acc

def bannerpeak.session.Session.cap_flips.if2?(bpc: Bool, +self: J.Val, +n: U32, +fnn: U32, +acc: J.Val, +it: J.Val) -> J.Val:
  match bpc:
    case True{}:
      bannerpeak.session.Session.cap_flips(self, (n + 1 : U32), 0, acc)
    case False{}:
      +f : J.Val = J.nth(J.get(self, "foes"), fnn)
      +acc = bannerpeak.session.Session.cap_flips.if1(Bool.and(bannerpeak.foes.Foe.alive(f), bannerpeak.session.overlap(bannerpeak.foes.Item.rect(it), bannerpeak.foes.Foe.rect(f))), fnn, acc)
      bannerpeak.session.Session.cap_flips(self, n, (fnn + 1 : U32), acc)

def bannerpeak.session.Session.cap_flips.if3?(bpc: Bool, +self: J.Val, +n: U32, +fnn: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +it : J.Val = J.nth(J.get(self, "items"), n)
      bannerpeak.session.Session.cap_flips.if2(Bool.or(Bool.not(Bool.and(String.eq(J.str(J.get(it, "kind")), "Cap"), String.eq(J.str(J.get(it, "cap")), "Slide"))), U32.is_ge(fnn, J.len(J.get(self, "foes")))), self, n, fnn, acc, it)

def bannerpeak.session.Session.apply_flips?(+self: J.Val, +flipped: J.Val, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  bannerpeak.session.Session.apply_flips.if2(U32.is_ge(n, J.len(flipped)), self, flipped, n, say)

def bannerpeak.session.Session.apply_flips.if1?(bpc: Bool, +self: J.Val, +say: J.Val, +m: U32, +f: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.foes.Foe.defeat(f)
      +f : J.Val = _t1
      +self = J.set(self, "foes", J.set_nth(J.get(self, "foes"), m, f))
      +_t2 = bannerpeak.session.Session.fx(self, "Pop", RS.i_div_euclid(J.u32(J.get(f, "x")), 256), RS.i_div_euclid(J.u32(J.get(f, "y")), 256))
      +self : J.Val = _t2
      +say = J.push(say, J.JS{"kill"})
      (f, (say, self))
    case False{}:
      (f, (say, self))

def bannerpeak.session.Session.apply_flips.if2?(bpc: Bool, +self: J.Val, +flipped: J.Val, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      (self, say)
    case False{}:
      +m : U32 = J.u32(J.nth(flipped, n))
      +f : J.Val = J.nth(J.get(self, "foes"), m)
      +_t3 = bannerpeak.session.Session.apply_flips.if1(bannerpeak.foes.Foe.alive(f), self, say, m, f)
      +f = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
      +_t4 = bannerpeak.session.Session.apply_flips(self, flipped, (n + 1 : U32), say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t4)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t4)
      (self, say)

def bannerpeak.session.Session.step_boulders?(+self: J.Val, +feet: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +lo : U32 = ((287 + 6 : U32) * 16 : U32)
  +hi : U32 = ((287 + 97 : U32) * 16 : U32)
  +seats : U32 = RS.u_max(bannerpeak.session.Session.seat_count(self), 1)
  +interval : U32 = RS.u_max((300 - (24 * (seats - 1 : U32) : U32) : U32), 180)
  +speed : U32 = (800 + (32 * (seats - 1 : U32) : U32) : U32)
  +someone : Bool = bannerpeak.session.Session.someone_on_snow(feet, 0, lo, hi)
  +self = J.set(self, "boulders", bannerpeak.session.Session.roll(J.get(self, "boulders"), 0, speed, bannerpeak.u.vec_new()))
  +_t1 = bannerpeak.session.Session.step_boulders.if4(Bool.and(someone, String.eq(J.str(J.get(self, "phase")), "Play")), self, say, hi, seats, interval, someone)
  +say = bannerpeak.u.fst(J.Val, J.Val, _t1)
  +self = bannerpeak.u.snd(J.Val, J.Val, _t1)
  (self, say)

def bannerpeak.session.Session.step_boulders.if1?(bpc: Bool, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"circle_toll"})
      say
    case False{}:
      say

def bannerpeak.session.Session.step_boulders.if2?(bpc: Bool, +self: J.Val, +hi: U32) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "boulders", J.push(J.get(self, "boulders"), J.set(J.set(J.obj(), "x", J.of_i32((hi * 256 : U32))), "t", J.JU{0})))
      self
    case False{}:
      self

def bannerpeak.session.Session.step_boulders.if3?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "boulders", bannerpeak.u.vec_new())
      self
    case False{}:
      self

def bannerpeak.session.Session.step_boulders.if4?(bpc: Bool, +self: J.Val, +say: J.Val, +hi: U32, +seats: U32, +interval: U32, +someone: Bool) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +phase : U32 = U32.mod(J.u32(J.get(self, "elapsed")), interval)
      +second : Bool = Bool.and(U32.is_ge(seats, 4), U32.is_eq(phase, 80))
      +say = bannerpeak.session.Session.step_boulders.if1(Bool.or(U32.is_eq(phase, (interval - 36 : U32)), Bool.and(U32.is_ge(seats, 4), U32.is_eq(phase, (80 - 36 : U32)))), say)
      +self = bannerpeak.session.Session.step_boulders.if2(Bool.and(Bool.or(U32.is_eq(phase, 0), second), U32.is_lt(J.len(J.get(self, "boulders")), 4)), self, hi)
      (say, self)
    case False{}:
      +self = bannerpeak.session.Session.step_boulders.if3(Bool.not(someone), self)
      (say, self)

def bannerpeak.session.Session.someone_on_snow?(+feet: J.Val, +n: U32, +lo: U32, +hi: U32) -> Bool:
  bannerpeak.session.Session.someone_on_snow.if2(U32.is_ge(n, J.len(feet)), feet, n, lo, hi)

def bannerpeak.session.Session.someone_on_snow.if1?(bpc: Bool, +feet: J.Val, +n: U32, +lo: U32, +hi: U32) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.session.Session.someone_on_snow(feet, (n + 1 : U32), lo, hi)

def bannerpeak.session.Session.someone_on_snow.if2?(bpc: Bool, +feet: J.Val, +n: U32, +lo: U32, +hi: U32) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +p : Sigma<&2, &2, U32, _ => U32> = (J.u32(J.nth(J.nth(feet, n), 0)), J.u32(J.nth(J.nth(feet, n), 1)))
      +_t1 : Sigma<&2, &2, U32, _ => U32> = p
      +_t2 : Sigma<&2, &2, U32, _ => U32> = p
      +_t3 : Sigma<&2, &2, U32, _ => U32> = p
      +_t4 : Sigma<&2, &2, U32, _ => U32> = p
      bannerpeak.session.Session.someone_on_snow.if1(Bool.and(Bool.and(Bool.and(RS.i_gt(bannerpeak.u.fst(U32, U32, _t1), (lo - 80 : U32)), RS.i_lt(bannerpeak.u.fst(U32, U32, _t2), (hi + 80 : U32))), RS.i_gt(bannerpeak.u.snd(U32, U32, _t3), (12 * 16 : U32))), RS.i_lt(bannerpeak.u.snd(U32, U32, _t4), ((12 + 14 : U32) * 16 : U32))), feet, n, lo, hi)

def bannerpeak.session.Session.roll?(+bs: J.Val, +n: U32, +speed: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.roll.if2(U32.is_ge(n, J.len(bs)), bs, n, speed, acc)

def bannerpeak.session.Session.roll.if1?(bpc: Bool, +acc: J.Val, +b: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, b)
      acc
    case False{}:
      acc

def bannerpeak.session.Session.roll.if2?(bpc: Bool, +bs: J.Val, +n: U32, +speed: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +b : J.Val = J.nth(bs, n)
      +b = J.set(b, "x", J.of_i32((J.u32(J.get(b, "x")) - speed : U32)))
      +b = J.set(b, "t", J.JU{(J.u32(J.get(b, "t")) + 1 : U32)})
      +acc = bannerpeak.session.Session.roll.if1(RS.i_gt(J.u32(J.get(b, "x")), (((287 + 4 : U32) * 16 : U32) * 256 : U32)), acc, b)
      bannerpeak.session.Session.roll(bs, (n + 1 : U32), speed, acc)

def bannerpeak.session.Session.touch_things?(+self: J.Val, +id: U32, +k: J.Val, +bits: U32, +was: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +me : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.session.px(J.get(k, "body"))
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
  +fx = bannerpeak.u.fst(U32, U32, _t1)
  +fy0 = bannerpeak.u.snd(U32, U32, _t1)
  +_t2 = bannerpeak.session.Session.touch_foes(self, k, me, fx, bits, was, 0, say)
  +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2)
  +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
  +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t2))
  +_t3 = bannerpeak.session.Session.touch_boulders(self, k, me, bits, was, 0, say)
  +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3)
  +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
  +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
  +_t8 = bannerpeak.session.Session.touch_things.if5(Bool.and(Bool.not(String.eq(J.str(J.get(J.get(self, "boss"), "beat")), "Dormant")), Bool.not(bannerpeak.boss.Boss.defeated(J.get(self, "boss")))), self, k, say, me, fx)
  +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t8)
  +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t8))
  +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t8))
  +_t9 = bannerpeak.session.Session.touch_items(self, id, k, me, bits, was, 0, bannerpeak.u.vec_new(), say)
  +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t9))
  +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t9)))
  +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t9)))
  +gone : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t9)
  +_t10 = bannerpeak.session.Session.remove_gone(self, gone, J.len(gone), 0)
  +self : J.Val = _t10
  +self = bannerpeak.session.Session.touch_things.if6(J.bool(J.get(k, "carry")), self, id, k, bits, say)
  +_b12 = Bool.and(Bool.and(String.eq(J.str(J.get(k, "size")), "Flame"), bannerpeak.knight.Knight.act_edge(k, bits)), U32.is_eq(J.u32(J.get(k, "cooldown")), 0))
  +_t13 = bannerpeak.session.Session.touch_things.if7(_b12, k, _b12)
  +_b12 = bannerpeak.u.fst(Bool, J.Val, _t13)
  +k = bannerpeak.u.snd(Bool, J.Val, _t13)
  +_t14 = bannerpeak.session.Session.touch_things.if8(Bool.and(_b12, U32.is_lt(bannerpeak.session.Session.bolts_of(J.get(self, "bolts"), 0, id), 2)), self, id, k, say)
  +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t14)
  +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t14))
  +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t14))
  (self, (k, say))

def bannerpeak.session.Session.touch_things.if1?(bpc: Bool, +hurt: Bool) -> Bool:
  match bpc:
    case True{}:
      +hurt : Bool = True{}
      hurt
    case False{}:
      hurt

def bannerpeak.session.Session.touch_things.if2?(bpc: Bool, +from: U32) -> U32:
  match bpc:
    case True{}:
      +from : U32 = 1
      from
    case False{}:
      from

def bannerpeak.session.Session.touch_things.if3?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"knight_hit_big"})
      +_t5 = bannerpeak.session.Session.after_hit(self, k)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t5)
      +k : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t5)
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.touch_things.if4?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val, +fx: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +from : U32 = 4294967295
      +from = bannerpeak.session.Session.touch_things.if2(RS.i_gt(J.u32(J.get(J.get(self, "boss"), "x")), fx), from)
      +_t4 = bannerpeak.knight.Knight.hit(k, RS.i_neg(from))
      +k : J.Val = bannerpeak.u.snd(Bool, J.Val, _t4)
      +_t6 = bannerpeak.session.Session.touch_things.if3(bannerpeak.u.fst(Bool, J.Val, _t4), self, k, say)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t6)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t6)
      (k, (say, self))
    case False{}:
      (k, (say, self))

def bannerpeak.session.Session.touch_things.if5?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +fx: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +hurt : Bool = bannerpeak.session.Session.hazard_hits(J.get(J.get(self, "boss"), "hazards"), 0, me)
      +body : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.boss.Boss.body(J.get(self, "boss"))
      +hurt = bannerpeak.session.Session.touch_things.if1(Bool.and(Bool.not(String.eq(J.str(J.get(J.get(self, "boss"), "beat")), "Rage")), bannerpeak.session.overlap(me, body)), hurt)
      +_t7 = bannerpeak.session.Session.touch_things.if4(hurt, self, k, say, fx)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t7)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t7))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t7))
      (k, (say, self))
    case False{}:
      (k, (say, self))

def bannerpeak.session.Session.touch_things.if6?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +bits: U32, +say: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +_t11 = bannerpeak.session.Session.carried_cap(self, id, k, bits, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t11)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t11))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t11))
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_things.if7?(bpc: Bool, +k: J.Val, +_b12: Bool) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +_b12 : Bool = bannerpeak.knight.Knight.free(k)
      (_b12, k)
    case False{}:
      (_b12, k)

def bannerpeak.session.Session.touch_things.if8?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +face : U32 = J.u32(J.get(J.get(k, "body"), "face"))
      +self = J.set(self, "bolts", J.push(J.get(self, "bolts"), J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "x", J.of_i32((J.u32(J.get(J.get(k, "body"), "x")) + ((face * 10 : U32) * 256 : U32) : U32))), "y", J.of_i32((J.u32(J.get(J.get(k, "body"), "y")) - (12 * 256 : U32) : U32))), "vx", J.of_i32((face * 1900 : U32))), "vy", J.of_i32(200)), "t", J.JU{0}), "owner", J.JU{id})))
      +k = J.set(k, "cooldown", J.JU{9})
      +say = J.push(say, J.JS{"shot_spark"})
      (k, (say, self))
    case False{}:
      (k, (say, self))

def bannerpeak.session.Session.hazard_hits?(+hs: J.Val, +n: U32, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>) -> Bool:
  bannerpeak.session.Session.hazard_hits.if2(U32.is_ge(n, J.len(hs)), hs, n, me)

def bannerpeak.session.Session.hazard_hits.if1?(bpc: Bool, +hs: J.Val, +n: U32, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>) -> Bool:
  match bpc:
    case True{}:
      True{}
    case False{}:
      bannerpeak.session.Session.hazard_hits(hs, (n + 1 : U32), me)

def bannerpeak.session.Session.hazard_hits.if2?(bpc: Bool, +hs: J.Val, +n: U32, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>) -> Bool:
  match bpc:
    case True{}:
      False{}
    case False{}:
      +h : J.Val = J.nth(hs, n)
      bannerpeak.session.Session.hazard_hits.if1(Bool.and(bannerpeak.boss.Hazard.hurts(h), bannerpeak.session.overlap(me, bannerpeak.boss.Hazard.rect(h))), hs, n, me)

def bannerpeak.session.Session.bolts_of?(+bolts: J.Val, +n: U32, +id: U32) -> U32:
  bannerpeak.session.Session.bolts_of.if2(U32.is_ge(n, J.len(bolts)), bolts, n, id)

def bannerpeak.session.Session.bolts_of.if1?(bpc: Bool, +c: U32) -> U32:
  match bpc:
    case True{}:
      +c : U32 = 1
      c
    case False{}:
      c

def bannerpeak.session.Session.bolts_of.if2?(bpc: Bool, +bolts: J.Val, +n: U32, +id: U32) -> U32:
  match bpc:
    case True{}:
      0
    case False{}:
      +c : U32 = 0
      +c = bannerpeak.session.Session.bolts_of.if1(U32.is_eq(J.u32(J.get(J.nth(bolts, n), "owner")), id), c)
      (c + bannerpeak.session.Session.bolts_of(bolts, (n + 1 : U32), id) : U32)

def bannerpeak.session.Session.touch_foes?(+self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +fx: U32, +bits: U32, +was: U32, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  bannerpeak.session.Session.touch_foes.if8(U32.is_ge(n, J.len(J.get(self, "foes"))), self, k, me, fx, bits, was, n, say)

def bannerpeak.session.Session.touch_foes.if1?(bpc: Bool, +k: J.Val, +bits: U32) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "chain", J.JU{RS.u_min((J.u32(J.get(k, "chain")) + 1 : U32), 255)})
      +_t7 = bannerpeak.body.Body.bounce(J.get(k, "body"), U32.is_ne(U32.and(bits, 1), 0))
      +k = J.set(k, "body", _t7)
      k
    case False{}:
      k

def bannerpeak.session.Session.touch_foes.if2?(bpc: Bool, +cap: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +cap = J.set(cap, "dir", J.of_i32(1))
      cap
    case False{}:
      +cap = J.set(cap, "dir", J.of_i32(4294967295))
      cap

def bannerpeak.session.Session.touch_foes.if3?(bpc: Bool, +self: J.Val, +k: J.Val, +cx: U32, +cy: U32) -> J.Val:
  match bpc:
    case True{}:
      +cap : J.Val = bannerpeak.foes.Item.new("Cap", cx, cy)
      +cap = bannerpeak.session.Session.touch_foes.if2(RS.i_gt(J.u32(J.get(J.get(k, "body"), "face")), 0), cap)
      +self = J.set(self, "items", J.push(J.get(self, "items"), cap))
      self
    case False{}:
      self

def bannerpeak.session.Session.touch_foes.if4?(bpc: Bool, +from: U32) -> U32:
  match bpc:
    case True{}:
      +from : U32 = 1
      from
    case False{}:
      from

def bannerpeak.session.Session.touch_foes.if5?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"knight_hit"})
      +_t9 = bannerpeak.session.Session.after_hit(self, k)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t9)
      +k : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t9)
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.touch_foes.if6?(bpc: Bool, +self: J.Val, +k: J.Val, +fx: U32, +bits: U32, +n: U32, +say: J.Val, +foe: J.Val, +fr: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +falling: Bool, +striking: Bool) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  match bpc:
    case True{}:
      +kind : String = J.str(J.get(foe, "kind"))
      +_t2 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = fr
      +_t3 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = fr
      +cx : U32 = RS.i_div((bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t2) + bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t3))) : U32), 2)
      +_t4 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = fr
      +cy : U32 = bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t4)))
      +_t5 = bannerpeak.foes.Foe.defeat(foe)
      +foe : J.Val = _t5
      +self = J.set(self, "foes", J.set_nth(J.get(self, "foes"), n, foe))
      +_t6 = bannerpeak.session.Session.fx(self, "Pop", cx, cy)
      +self : J.Val = _t6
      +say = J.push(say, J.JS{"kill"})
      +value : U32 = RS.u_shl(100, RS.u_min(J.u32(J.get(k, "chain")), 6))
      +k = J.set(k, "score", J.JU{(J.u32(J.get(k, "score")) + value : U32)})
      +k = bannerpeak.session.Session.touch_foes.if1(Bool.and(falling, Bool.not(striking)), k, bits)
      +self = bannerpeak.session.Session.touch_foes.if3(Bool.and(String.eq(kind, "Capped"), Bool.not(striking)), self, k, cx, cy)
      (foe, (k, (say, self)))
    case False{}:
      +from : U32 = 4294967295
      +from = bannerpeak.session.Session.touch_foes.if4(RS.i_gt(RS.i_div_euclid(J.u32(J.get(foe, "x")), 256), fx), from)
      +_t8 = bannerpeak.knight.Knight.hit(k, RS.i_neg(from))
      +k : J.Val = bannerpeak.u.snd(Bool, J.Val, _t8)
      +_t10 = bannerpeak.session.Session.touch_foes.if5(bannerpeak.u.fst(Bool, J.Val, _t8), self, k, say)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t10)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t10)
      (foe, (k, (say, self)))

def bannerpeak.session.Session.touch_foes.if7?(bpc: Bool, +self: J.Val, +k: J.Val, +fx: U32, +bits: U32, +was: U32, +n: U32, +say: J.Val, +foe: J.Val, +fr: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  match bpc:
    case True{}:
      +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = fr
      +falling : Bool = Bool.and(RS.i_gt(J.u32(J.get(J.get(k, "body"), "vy")), 0), RS.i_le(was, ((bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)) + 4 : U32) * 256 : U32)))
      +striking : Bool = Bool.or(Bool.or(String.eq(J.str(J.get(J.get(k, "body"), "mode")), "Pound"), String.eq(J.str(J.get(J.get(k, "body"), "mode")), "Dive")), String.eq(J.str(J.get(J.get(k, "body"), "mode")), "Belly"))
      +_t11 = bannerpeak.session.Session.touch_foes.if6(Bool.or(striking, falling), self, k, fx, bits, n, say, foe, fr, falling, striking)
      +foe = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t11)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t11))
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t11)))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t11)))
      (foe, (k, (say, self)))
    case False{}:
      (foe, (k, (say, self)))

def bannerpeak.session.Session.touch_foes.if8?(bpc: Bool, +self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +fx: U32, +bits: U32, +was: U32, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (self, (k, say))
    case False{}:
      +foe : J.Val = J.nth(J.get(self, "foes"), n)
      +fr : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.foes.Foe.rect(foe)
      +_t12 = bannerpeak.session.Session.touch_foes.if7(Bool.and(bannerpeak.foes.Foe.alive(foe), bannerpeak.session.overlap(me, fr)), self, k, fx, bits, was, n, say, foe, fr)
      +foe = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t12)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t12))
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t12)))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t12)))
      +_t13 = bannerpeak.session.Session.touch_foes(self, k, me, fx, bits, was, (n + 1 : U32), say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t13)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t13))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t13))
      (self, (k, say))

def bannerpeak.session.Session.touch_boulders?(+self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +bits: U32, +was: U32, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  bannerpeak.session.Session.touch_boulders.if4(U32.is_ge(n, J.len(J.get(self, "boulders"))), self, k, me, bits, was, n, say)

def bannerpeak.session.Session.touch_boulders.if1?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"knight_hit_big"})
      +_t4 = bannerpeak.session.Session.after_hit(self, k)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t4)
      +k : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t4)
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.touch_boulders.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +bits: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t2 = bannerpeak.body.Body.bounce(J.get(k, "body"), U32.is_ne(U32.and(bits, 1), 0))
      +k = J.set(k, "body", _t2)
      +say = J.push(say, J.JS{"clang"})
      (k, (say, self))
    case False{}:
      +_t3 = bannerpeak.knight.Knight.hit(k, 1)
      +k : J.Val = bannerpeak.u.snd(Bool, J.Val, _t3)
      +_t5 = bannerpeak.session.Session.touch_boulders.if1(bannerpeak.u.fst(Bool, J.Val, _t3), self, k, say)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t5)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t5)
      (k, (say, self))

def bannerpeak.session.Session.touch_boulders.if3?(bpc: Bool, +self: J.Val, +k: J.Val, +bits: U32, +was: U32, +say: J.Val, +r: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = r
      +_t6 = bannerpeak.session.Session.touch_boulders.if2(Bool.and(RS.i_gt(J.u32(J.get(J.get(k, "body"), "vy")), 0), RS.i_le(was, ((bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)) + 4 : U32) * 256 : U32))), self, k, bits, say)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t6)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t6))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t6))
      (k, (say, self))
    case False{}:
      (k, (say, self))

def bannerpeak.session.Session.touch_boulders.if4?(bpc: Bool, +self: J.Val, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +bits: U32, +was: U32, +n: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (self, (k, say))
    case False{}:
      +floor : U32 = (22 * 16 : U32)
      +bx : U32 = RS.i_div_euclid(J.u32(J.get(J.nth(J.get(self, "boulders"), n), "x")), 256)
      +r : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = ((bx - 14 : U32), ((floor - 28 : U32), ((bx + 14 : U32), floor)))
      +_t7 = bannerpeak.session.Session.touch_boulders.if3(bannerpeak.session.overlap(me, r), self, k, bits, was, say, r)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t7)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t7))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t7))
      +_t8 = bannerpeak.session.Session.touch_boulders(self, k, me, bits, was, (n + 1 : U32), say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t8)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t8))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t8))
      (self, (k, say))

def bannerpeak.session.Session.touch_items?(+self: J.Val, +id: U32, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +bits: U32, +was: U32, +n: U32, +gone: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  bannerpeak.session.Session.touch_items.if5(U32.is_ge(n, J.len(J.get(self, "items"))), self, id, k, me, bits, was, n, gone, say)

def bannerpeak.session.Session.touch_items.if1?(bpc: Bool, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "size", J.JS{"Flame"})
      k
    case False{}:
      +k = J.set(k, "pips", J.JU{RS.u_min((J.u32(J.get(k, "pips")) + 2 : U32), 8)})
      k

def bannerpeak.session.Session.touch_items.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +it: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +k = J.set(k, "size", J.JS{"Big"})
      +k = J.set(k, "body", J.set(J.get(k, "body"), "big", J.JBool{True{}}))
      +_t2 = bannerpeak.body.Body.regrow(J.get(k, "body"), J.get(self, "changed"))
      +k = J.set(k, "body", _t2)
      k
    case False{}:
      +k = bannerpeak.session.Session.touch_items.if1(String.eq(J.str(J.get(it, "kind")), "Flame"), k)
      k

def bannerpeak.session.Session.touch_items.if3?(bpc: Bool, +self: J.Val, +k: J.Val, +n: U32, +gone: J.Val, +say: J.Val, +it: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +gone = J.push(gone, J.JU{n})
      +say = J.push(say, J.JS{"potion"})
      +k = J.set(k, "score", J.JU{(J.u32(J.get(k, "score")) + 1000 : U32)})
      +k = bannerpeak.session.Session.touch_items.if2(String.eq(J.str(J.get(k, "size")), "Small"), self, k, it)
      (gone, (k, say))
    case False{}:
      (gone, (k, say))

def bannerpeak.session.Session.touch_items.if4?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +bits: U32, +was: U32, +n: U32, +gone: J.Val, +say: J.Val, +it: J.Val, +r: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.session.Session.touch_cap(self, id, k, n, bits, was, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1)
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t1))
      (gone, (k, (say, self)))
    case False{}:
      +_t3 = bannerpeak.session.Session.touch_items.if3(Bool.and(Bool.not(bannerpeak.foes.Item.rising(it)), bannerpeak.session.overlap(me, r)), self, k, n, gone, say, it)
      +gone = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3)
      +k = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
      +say = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t3))
      (gone, (k, (say, self)))

def bannerpeak.session.Session.touch_items.if5?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +me: Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>>, +bits: U32, +was: U32, +n: U32, +gone: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  match bpc:
    case True{}:
      (gone, (self, (k, say)))
    case False{}:
      +it : J.Val = J.nth(J.get(self, "items"), n)
      +r : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.foes.Item.rect(it)
      +_t4 = bannerpeak.session.Session.touch_items.if4(String.eq(J.str(J.get(it, "kind")), "Cap"), self, id, k, me, bits, was, n, gone, say, it, r)
      +gone = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t4)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t4))
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t4)))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t4)))
      +_t5 = bannerpeak.session.Session.touch_items(self, id, k, me, bits, was, (n + 1 : U32), gone, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t5))
      +k : J.Val = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t5)))
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t5)))
      (bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t5), (self, (k, say)))

def bannerpeak.session.Session.remove_gone?(+self: J.Val, +gone: J.Val, +n: U32, +skip: U32) -> J.Val:
  bannerpeak.session.Session.remove_gone.if1(U32.is_eq(n, 0), self, gone, n, skip)

def bannerpeak.session.Session.remove_gone.if1?(bpc: Bool, +self: J.Val, +gone: J.Val, +n: U32, +skip: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +self = J.set(self, "items", bannerpeak.u.vec_remove(J.get(self, "items"), J.u32(J.nth(gone, (n - 1 : U32)))))
      +_t1 = bannerpeak.session.Session.remove_gone(self, gone, (n - 1 : U32), skip)
      +self : J.Val = _t1
      self

def bannerpeak.session.Session.held_cap?(+self: J.Val, +id: U32, +n: U32) -> U32:
  bannerpeak.session.Session.held_cap.if2(U32.is_ge(n, J.len(J.get(self, "items"))), self, id, n)

def bannerpeak.session.Session.held_cap.if1?(bpc: Bool, +self: J.Val, +id: U32, +n: U32) -> U32:
  match bpc:
    case True{}:
      n
    case False{}:
      bannerpeak.session.Session.held_cap(self, id, (n + 1 : U32))

def bannerpeak.session.Session.held_cap.if2?(bpc: Bool, +self: J.Val, +id: U32, +n: U32) -> U32:
  match bpc:
    case True{}:
      n
    case False{}:
      +it : J.Val = J.nth(J.get(self, "items"), n)
      bannerpeak.session.Session.held_cap.if1(Bool.and(Bool.and(String.eq(J.str(J.get(it, "kind")), "Cap"), String.eq(J.str(J.get(it, "cap")), "Carried")), U32.is_eq(J.u32(J.get(it, "owner")), id)), self, id, n)

def bannerpeak.session.Session.carried_cap?(+self: J.Val, +id: U32, +k: J.Val, +bits: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +n : U32 = bannerpeak.session.Session.held_cap(self, id, 0)
  bannerpeak.session.Session.carried_cap.if4(U32.is_ge(n, J.len(J.get(self, "items"))), self, k, bits, say, n)

def bannerpeak.session.Session.carried_cap.if1?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"knight_hit"})
      +_t2 = bannerpeak.session.Session.after_hit(self, k)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t2)
      +k : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t2)
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.carried_cap.if2?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val, +n: U32, +it: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +self = J.set(self, "items", bannerpeak.u.vec_remove(J.get(self, "items"), n))
      +k = J.set(k, "carry", J.JBool{False{}})
      +_t1 = bannerpeak.knight.Knight.hit(k, RS.i_neg(J.u32(J.get(J.get(k, "body"), "face"))))
      +k : J.Val = bannerpeak.u.snd(Bool, J.Val, _t1)
      +_t3 = bannerpeak.session.Session.carried_cap.if1(bannerpeak.u.fst(Bool, J.Val, _t1), self, k, say)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t3)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t3)
      (k, (say, self))
    case False{}:
      +self = J.set(self, "items", J.set_nth(J.get(self, "items"), n, it))
      (k, (say, self))

def bannerpeak.session.Session.carried_cap.if3?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val, +n: U32, +it: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  match bpc:
    case True{}:
      +it = J.set(it, "cap", J.JS{"Slide"})
      +it = J.set(it, "dir", J.of_i32(J.u32(J.get(J.get(k, "body"), "face"))))
      +it = J.set(it, "t", J.JU{0})
      +self = J.set(self, "items", J.set_nth(J.get(self, "items"), n, it))
      +k = J.set(k, "carry", J.JBool{False{}})
      +say = J.push(say, J.JS{"swing"})
      (it, (k, (say, self)))
    case False{}:
      +it = J.set(it, "x", J.of_i32((J.u32(J.get(J.get(k, "body"), "x")) + ((J.u32(J.get(J.get(k, "body"), "face")) * 10 : U32) * 256 : U32) : U32)))
      +it = J.set(it, "y", J.of_i32((J.u32(J.get(J.get(k, "body"), "y")) - (2 * 256 : U32) : U32)))
      +it = J.set(it, "t", J.JU{(J.u32(J.get(it, "t")) + 1 : U32)})
      +_t4 = bannerpeak.session.Session.carried_cap.if2(U32.is_gt(J.u32(J.get(it, "t")), 150), self, k, say, n, it)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t4)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t4))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t4))
      (it, (k, (say, self)))

def bannerpeak.session.Session.carried_cap.if4?(bpc: Bool, +self: J.Val, +k: J.Val, +bits: U32, +say: J.Val, +n: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +k = J.set(k, "carry", J.JBool{False{}})
      (self, (k, say))
    case False{}:
      +it : J.Val = J.nth(J.get(self, "items"), n)
      +_t5 = bannerpeak.session.Session.carried_cap.if3(U32.is_eq(U32.and(bits, 4), 0), self, k, say, n, it)
      +it = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t5)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t5))
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t5)))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t5)))
      (self, (k, say))

def bannerpeak.session.Session.touch_cap?(+self: J.Val, +id: U32, +k: J.Val, +n: U32, +bits: U32, +was: U32, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  +me : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.session.px(J.get(k, "body"))
  +it : J.Val = J.nth(J.get(self, "items"), n)
  bannerpeak.session.Session.touch_cap.if11(Bool.or(String.eq(J.str(J.get(it, "cap")), "Carried"), Bool.not(bannerpeak.session.overlap(me, bannerpeak.foes.Item.rect(it)))), self, id, k, n, bits, was, say, it)

def bannerpeak.session.Session.touch_cap.if1?(bpc: Bool, +from: U32) -> U32:
  match bpc:
    case True{}:
      +from : U32 = 1
      from
    case False{}:
      from

def bannerpeak.session.Session.touch_cap.if2?(bpc: Bool, +c: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +c = J.set(c, "dir", J.of_i32(1))
      c
    case False{}:
      +c = J.set(c, "dir", J.of_i32(4294967295))
      c

def bannerpeak.session.Session.touch_cap.if3?(bpc: Bool, +k: J.Val, +_b4: Bool) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +_b4 : Bool = bannerpeak.knight.Knight.free(k)
      (_b4, k)
    case False{}:
      (_b4, k)

def bannerpeak.session.Session.touch_cap.if4?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +n: U32, +say: J.Val, +from: U32, +c: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  match bpc:
    case True{}:
      +c = J.set(c, "cap", J.JS{"Carried"})
      +c = J.set(c, "owner", J.JU{id})
      +c = J.set(c, "t", J.JU{0})
      +self = J.set(self, "items", J.set_nth(J.get(self, "items"), n, c))
      +k = J.set(k, "carry", J.JBool{True{}})
      +say = J.push(say, J.JS{"shield_up"})
      (c, (k, (say, self)))
    case False{}:
      +c = J.set(c, "cap", J.JS{"Slide"})
      +c = J.set(c, "dir", J.of_i32(from))
      +c = J.set(c, "owner", J.JU{id})
      +c = J.set(c, "t", J.JU{0})
      +self = J.set(self, "items", J.set_nth(J.get(self, "items"), n, c))
      +say = J.push(say, J.JS{"clang"})
      (c, (k, (say, self)))

def bannerpeak.session.Session.touch_cap.if5?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +n: U32, +bits: U32, +say: J.Val, +from: U32, +c: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  match bpc:
    case True{}:
      +c = J.set(c, "cap", J.JS{"Slide"})
      +c = bannerpeak.session.Session.touch_cap.if2(RS.i_gt(J.u32(J.get(J.get(k, "body"), "face")), 0), c)
      +c = J.set(c, "owner", J.JU{id})
      +c = J.set(c, "t", J.JU{0})
      +self = J.set(self, "items", J.set_nth(J.get(self, "items"), n, c))
      +_t3 = bannerpeak.body.Body.bounce(J.get(k, "body"), U32.is_ne(U32.and(bits, 1), 0))
      +k = J.set(k, "body", _t3)
      +say = J.push(say, J.JS{"kill"})
      (c, (k, (say, self)))
    case False{}:
      +_b4 = Bool.and(U32.is_ne(U32.and(bits, 4), 0), Bool.not(J.bool(J.get(k, "carry"))))
      +_t5 = bannerpeak.session.Session.touch_cap.if3(_b4, k, _b4)
      +_b4 = bannerpeak.u.fst(Bool, J.Val, _t5)
      +k = bannerpeak.u.snd(Bool, J.Val, _t5)
      +_t6 = bannerpeak.session.Session.touch_cap.if4(_b4, self, id, k, n, say, from, c)
      +c = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t6)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t6))
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t6)))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t6)))
      (c, (k, (say, self)))

def bannerpeak.session.Session.touch_cap.if6?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +say = J.push(say, J.JS{"knight_hit"})
      +_t10 = bannerpeak.session.Session.after_hit(self, k)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t10)
      +k : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t10)
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.touch_cap.if7?(bpc: Bool, +self: J.Val, +k: J.Val, +say: J.Val, +from: U32) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      +_t9 = bannerpeak.knight.Knight.hit(k, RS.i_neg(from))
      +k : J.Val = bannerpeak.u.snd(Bool, J.Val, _t9)
      +_t11 = bannerpeak.session.Session.touch_cap.if6(bannerpeak.u.fst(Bool, J.Val, _t9), self, k, say)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t11)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t11)
      (k, (say, self))
    case False{}:
      (k, (say, self))

def bannerpeak.session.Session.touch_cap.if8?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +n: U32, +bits: U32, +say: J.Val, +it: J.Val, +from: U32, +c: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  match bpc:
    case True{}:
      +c = J.set(c, "cap", J.JS{"Rest"})
      +c = J.set(c, "vx", J.of_i32(0))
      +self = J.set(self, "items", J.set_nth(J.get(self, "items"), n, c))
      +_t8 = bannerpeak.body.Body.bounce(J.get(k, "body"), U32.is_ne(U32.and(bits, 1), 0))
      +k = J.set(k, "body", _t8)
      +say = J.push(say, J.JS{"kill"})
      (c, (k, (say, self)))
    case False{}:
      +_t12 = bannerpeak.session.Session.touch_cap.if7(Bool.or(U32.is_ne(J.u32(J.get(it, "owner")), id), U32.is_gt(J.u32(J.get(it, "t")), 14)), self, k, say, from)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t12)
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t12))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, _t12))
      (c, (k, (say, self)))

def bannerpeak.session.Session.touch_cap.if9?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +n: U32, +bits: U32, +say: J.Val, +it: J.Val, +on_top: Bool, +from: U32, +c: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  match bpc:
    case True{}:
      +_t13 = bannerpeak.session.Session.touch_cap.if8(on_top, self, id, k, n, bits, say, it, from, c)
      +c = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t13)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t13))
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t13)))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t13)))
      (c, (k, (say, self)))
    case False{}:
      (c, (k, (say, self)))

def bannerpeak.session.Session.touch_cap.if10?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +n: U32, +bits: U32, +say: J.Val, +it: J.Val, +on_top: Bool, +from: U32, +c: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>>:
  match bpc:
    case True{}:
      +_t7 = bannerpeak.session.Session.touch_cap.if5(on_top, self, id, k, n, bits, say, from, c)
      +c = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t7)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t7))
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t7)))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t7)))
      (c, (k, (say, self)))
    case False{}:
      +_t14 = bannerpeak.session.Session.touch_cap.if9(String.eq(J.str(J.get(it, "cap")), "Slide"), self, id, k, n, bits, say, it, on_top, from, c)
      +c = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t14)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t14))
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t14)))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t14)))
      (c, (k, (say, self)))

def bannerpeak.session.Session.touch_cap.if11?(bpc: Bool, +self: J.Val, +id: U32, +k: J.Val, +n: U32, +bits: U32, +was: U32, +say: J.Val, +it: J.Val) -> Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>:
  match bpc:
    case True{}:
      (self, (k, say))
    case False{}:
      +_t1 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.foes.Item.rect(it)
      +on_top : Bool = Bool.and(RS.i_gt(J.u32(J.get(J.get(k, "body"), "vy")), 0), RS.i_le(was, ((bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t1)) + 4 : U32) * 256 : U32)))
      +from : U32 = 4294967295
      +_t2 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
      +from = bannerpeak.session.Session.touch_cap.if1(RS.i_gt(RS.i_div_euclid(J.u32(J.get(it, "x")), 256), bannerpeak.u.fst(U32, U32, _t2)), from)
      +c : J.Val = it
      +_t15 = bannerpeak.session.Session.touch_cap.if10(String.eq(J.str(J.get(it, "cap")), "Rest"), self, id, k, n, bits, say, it, on_top, from, c)
      +c = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t15)
      +k = bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t15))
      +say = bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t15)))
      +self = bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => Sigma<&2, &2, J.Val, _ => J.Val>>, _t15)))
      (self, (k, say))

def bannerpeak.session.Session.mates?(+self: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.mates.if3(bannerpeak.u.is_empty(ids), self, ids, acc)

def bannerpeak.session.Session.mates.if1?(bpc: Bool, +k: J.Val, +_b1: Bool) -> Sigma<&2, &2, Bool, _ => J.Val>:
  match bpc:
    case True{}:
      +_b1 : Bool = bannerpeak.knight.Knight.free(k)
      (_b1, k)
    case False{}:
      (_b1, k)

def bannerpeak.session.Session.mates.if2?(bpc: Bool, +acc: J.Val, +id: U32, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +r : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = bannerpeak.session.px(J.get(k, "body"))
      +_t3 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.body.Body.foot(J.get(k, "body"))
      +fx = bannerpeak.u.fst(U32, U32, _t3)
      +fy = bannerpeak.u.snd(U32, U32, _t3)
      +_t4 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = r
      +_t5 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = r
      +_t6 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = r
      +_t7 : Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>> = r
      +acc = J.push(acc, J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "id", J.JU{id}), "l", J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t4))), "t", J.of_i32(bannerpeak.u.fst(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t5)))), "r", J.of_i32(bannerpeak.u.fst(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t6))))), "b", J.of_i32(bannerpeak.u.snd(U32, U32, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => U32>, bannerpeak.u.snd(U32, Sigma<&2, &2, U32, _ => Sigma<&2, &2, U32, _ => U32>>, _t7))))), "fx", J.of_i32(fx)), "fy", J.of_i32(fy)))
      acc
    case False{}:
      acc

def bannerpeak.session.Session.mates.if3?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +_b1 = bannerpeak.knight.Knight.alive(k)
      +_t2 = bannerpeak.session.Session.mates.if1(_b1, k, _b1)
      +_b1 = bannerpeak.u.fst(Bool, J.Val, _t2)
      +k = bannerpeak.u.snd(Bool, J.Val, _t2)
      +acc = bannerpeak.session.Session.mates.if2(_b1, acc, id, k)
      bannerpeak.session.Session.mates(self, bannerpeak.u.tail(ids), acc)

def bannerpeak.session.Session.mate_for?(+mates: J.Val, +n: U32, +bx: U32, +by: U32) -> U32:
  bannerpeak.session.Session.mate_for.if2(U32.is_ge(n, J.len(mates)), mates, n, bx, by)

def bannerpeak.session.Session.mate_for.if1?(bpc: Bool, +mates: J.Val, +n: U32, +bx: U32, +by: U32) -> U32:
  match bpc:
    case True{}:
      n
    case False{}:
      bannerpeak.session.Session.mate_for(mates, (n + 1 : U32), bx, by)

def bannerpeak.session.Session.mate_for.if2?(bpc: Bool, +mates: J.Val, +n: U32, +bx: U32, +by: U32) -> U32:
  match bpc:
    case True{}:
      n
    case False{}:
      +m : J.Val = J.nth(mates, n)
      bannerpeak.session.Session.mate_for.if1(bannerpeak.session.overlap((J.u32(J.get(m, "l")), (J.u32(J.get(m, "t")), (J.u32(J.get(m, "r")), J.u32(J.get(m, "b"))))), ((bx - 12 : U32), ((by - 12 : U32), ((bx + 12 : U32), (by + 12 : U32))))), mates, n, bx, by)

def bannerpeak.session.Session.step_rescue?(+self: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  +mates : J.Val = bannerpeak.session.Session.mates(self, bannerpeak.u.ids(J.get(self, "seats")), bannerpeak.u.vec_new())
  +ids : List<&2, U32> = bannerpeak.u.ids(J.get(self, "seats"))
  +_t1 = bannerpeak.session.Session.rescue_seats(self, ids, mates, say)
  +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t1)
  +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t1)
  +_t2 = bannerpeak.session.Session.drift_seats(self, ids, mates)
  +self : J.Val = _t2
  (self, say)

def bannerpeak.session.Session.rescue_seats?(+self: J.Val, +ids: List<&2, U32>, +mates: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  bannerpeak.session.Session.rescue_seats.if3(bannerpeak.u.is_empty(ids), self, ids, mates, say)

def bannerpeak.session.Session.rescue_seats.if1?(bpc: Bool, +self: J.Val, +mates: J.Val, +say: J.Val, +id: U32, +m: U32) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +fx : U32 = J.u32(J.get(J.nth(mates, m), "fx"))
      +fy : U32 = J.u32(J.get(J.nth(mates, m), "fy"))
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +k = J.set(k, "bubble", J.JNull{})
      +k = J.set(k, "pips", J.JU{3})
      +k = J.set(k, "hurt", J.JU{70})
      +_t1 = bannerpeak.body.Body.place(J.get(k, "body"), fx, fy)
      +k = J.set(k, "body", _t1)
      +_j2 : Sigma<&2, &2, U32, _ => U32> = (fx, fy)
      +k = J.set(k, "safe", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j2)), J.of_i32(bannerpeak.u.snd(U32, U32, _j2))]})
      +_j3 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.knight.follow((J.u32(J.nth(J.get(k, "cam"), 0)), J.u32(J.nth(J.get(k, "cam"), 1))), fx, fy, J.u32(J.get(J.get(k, "body"), "face")), 0, 0)
      +k = J.set(k, "cam", J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, _j3)), J.of_i32(bannerpeak.u.snd(U32, U32, _j3))]})
      +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), id, k))
      +_t4 = bannerpeak.session.Session.fx(self, "Gleam", fx, (fy - 12 : U32))
      +self : J.Val = _t4
      +say = J.push(say, J.JS{"wish_grant"})
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.rescue_seats.if2?(bpc: Bool, +self: J.Val, +mates: J.Val, +say: J.Val, +id: U32, +k0: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +b : J.Val = J.get(k0, "bubble")
      +bx : U32 = RS.i_div_euclid(J.u32(J.get(b, "x")), 256)
      +by : U32 = RS.i_div_euclid(J.u32(J.get(b, "y")), 256)
      +m : U32 = bannerpeak.session.Session.mate_for(mates, 0, bx, by)
      +_t5 = bannerpeak.session.Session.rescue_seats.if1(U32.is_lt(m, J.len(mates)), self, mates, say, id, m)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t5)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t5)
      (say, self)
    case False{}:
      (say, self)

def bannerpeak.session.Session.rescue_seats.if3?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +mates: J.Val, +say: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      (self, say)
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +k0 : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +_t6 = bannerpeak.session.Session.rescue_seats.if2(Bool.not(J.is_null(J.get(k0, "bubble"))), self, mates, say, id, k0)
      +say = bannerpeak.u.fst(J.Val, J.Val, _t6)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t6)
      +_t7 = bannerpeak.session.Session.rescue_seats(self, bannerpeak.u.tail(ids), mates, say)
      +self : J.Val = bannerpeak.u.fst(J.Val, J.Val, _t7)
      +say : J.Val = bannerpeak.u.snd(J.Val, J.Val, _t7)
      (self, say)

def bannerpeak.session.Session.mate_nearest?(+mates: J.Val, +bx: U32, +n: U32, +best: U32, +best_d: U32) -> U32:
  bannerpeak.session.Session.mate_nearest.if2(U32.is_ge(n, J.len(mates)), mates, bx, n, best, best_d)

def bannerpeak.session.Session.mate_nearest.if1?(bpc: Bool, +mates: J.Val, +bx: U32, +n: U32, +best: U32, +best_d: U32, +d: U32) -> U32:
  match bpc:
    case True{}:
      bannerpeak.session.Session.mate_nearest(mates, bx, (n + 1 : U32), n, d)
    case False{}:
      bannerpeak.session.Session.mate_nearest(mates, bx, (n + 1 : U32), best, best_d)

def bannerpeak.session.Session.mate_nearest.if2?(bpc: Bool, +mates: J.Val, +bx: U32, +n: U32, +best: U32, +best_d: U32) -> U32:
  match bpc:
    case True{}:
      best
    case False{}:
      +d : U32 = RS.i_abs((J.u32(J.get(J.nth(mates, n), "fx")) - bx : U32))
      bannerpeak.session.Session.mate_nearest.if1(Bool.or(U32.is_eq(n, 0), RS.i_lt(d, best_d)), mates, bx, n, best, best_d, d)

def bannerpeak.session.Session.drift_seats?(+self: J.Val, +ids: List<&2, U32>, +mates: J.Val) -> J.Val:
  bannerpeak.session.Session.drift_seats.if2(bannerpeak.u.is_empty(ids), self, ids, mates)

def bannerpeak.session.Session.drift_seats.if1?(bpc: Bool, +self: J.Val, +mates: J.Val, +id: U32, +k: J.Val) -> Sigma<&2, &2, J.Val, _ => J.Val>:
  match bpc:
    case True{}:
      +b : J.Val = J.get(k, "bubble")
      +m : U32 = bannerpeak.session.Session.mate_nearest(mates, RS.i_div_euclid(J.u32(J.get(b, "x")), 256), 0, 0, 0)
      +b = J.set(b, "x", J.of_i32((J.u32(J.get(b, "x")) + RS.i_clamp(((J.u32(J.get(J.nth(mates, m), "fx")) * 256 : U32) - J.u32(J.get(b, "x")) : U32), 4294967272, 24) : U32)))
      +b = J.set(b, "y", J.of_i32((J.u32(J.get(b, "y")) + RS.i_clamp((((J.u32(J.get(J.nth(mates, m), "fy")) - 20 : U32) * 256 : U32) - J.u32(J.get(b, "y")) : U32), 4294967280, 16) : U32)))
      +k = J.set(k, "bubble", b)
      +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), id, k))
      (k, self)
    case False{}:
      (k, self)

def bannerpeak.session.Session.drift_seats.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +mates: J.Val) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +_t1 = bannerpeak.session.Session.drift_seats.if1(Bool.and(Bool.not(J.is_null(J.get(k, "bubble"))), U32.is_gt(J.len(mates), 0)), self, mates, id, k)
      +k = bannerpeak.u.fst(J.Val, J.Val, _t1)
      +self = bannerpeak.u.snd(J.Val, J.Val, _t1)
      +_t2 = bannerpeak.session.Session.drift_seats(self, bannerpeak.u.tail(ids), mates)
      +self : J.Val = _t2
      self

def case.bannerpeak.session.Session.step_things?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.step_things(J.get(input, "session"), J.get(input, "knobs"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "say", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.step_boulders?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.step_boulders(J.get(input, "session"), bannerpeak.u.vec_or(J.get(input, "feet")), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "say", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.touch_things?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.touch_things(J.get(input, "session"), J.u32(J.get(input, "id")), J.get(input, "k"), J.u32(J.get(input, "bits")), J.u32(J.get(input, "was")), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)), "k", bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r))), "say", bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)))

def case.bannerpeak.session.Session.touch_cap?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.touch_cap(J.get(input, "session"), J.u32(J.get(input, "id")), J.get(input, "k"), J.u32(J.get(input, "n")), J.u32(J.get(input, "bits")), J.u32(J.get(input, "was")), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)), "k", bannerpeak.u.fst(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r))), "say", bannerpeak.u.snd(J.Val, J.Val, bannerpeak.u.snd(J.Val, Sigma<&2, &2, J.Val, _ => J.Val>, r)))

def case.bannerpeak.session.Session.step_rescue?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.step_rescue(J.get(input, "session"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "say", bannerpeak.u.snd(J.Val, J.Val, r))

def case.bannerpeak.session.Session.retry?(+input: J.Val) -> J.Val:
  +r = bannerpeak.session.Session.retry(J.get(input, "session"), bannerpeak.u.vec_or(J.get(input, "say")))
  J.set(J.set(J.obj(), "session", bannerpeak.u.fst(J.Val, J.Val, r)), "say", bannerpeak.u.snd(J.Val, J.Val, r))

def bannerpeak.session.Session.keep_ids?(+m: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.keep_ids.if2(bannerpeak.u.is_empty(ids), m, ids, acc)

def bannerpeak.session.Session.keep_ids.if1?(bpc: Bool, +m: J.Val, +acc: J.Val, +id: U32) -> J.Val:
  match bpc:
    case True{}:
      +acc = bannerpeak.u.put(acc, id, bannerpeak.u.at(m, id))
      acc
    case False{}:
      acc

def bannerpeak.session.Session.keep_ids.if2?(bpc: Bool, +m: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +acc = bannerpeak.session.Session.keep_ids.if1(Bool.and(U32.is_ge(id, 1), U32.is_le(id, 6)), m, acc, id)
      bannerpeak.session.Session.keep_ids(m, bannerpeak.u.tail(ids), acc)

def bannerpeak.session.Session.mend_seats?(+self: J.Val, +ids: List<&2, U32>, +elapsed: U32, +sx: U32, +sy: U32) -> J.Val:
  bannerpeak.session.Session.mend_seats.if2(bannerpeak.u.is_empty(ids), self, ids, elapsed, sx, sy)

def bannerpeak.session.Session.mend_seats.if1?(bpc: Bool, +self: J.Val, +elapsed: U32, +sx: U32, +sy: U32, +id: U32, +k: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +joined : U32 = RS.u_min(J.u32(J.get(k, "joined")), elapsed)
      +self = J.set(self, "seats", bannerpeak.u.put(J.get(self, "seats"), id, bannerpeak.knight.Knight.at(joined, sx, sy)))
      self
    case False{}:
      self

def bannerpeak.session.Session.mend_seats.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +elapsed: U32, +sx: U32, +sy: U32) -> J.Val:
  match bpc:
    case True{}:
      self
    case False{}:
      +id : U32 = bannerpeak.u.head(ids)
      +k : J.Val = bannerpeak.u.at(J.get(self, "seats"), id)
      +self = bannerpeak.session.Session.mend_seats.if1(Bool.or(Bool.not(bannerpeak.knight.Knight.valid(k)), U32.is_gt(J.u32(J.get(k, "joined")), elapsed)), self, elapsed, sx, sy, id, k)
      +_t1 = bannerpeak.session.Session.mend_seats(self, bannerpeak.u.tail(ids), elapsed, sx, sy)
      +self : J.Val = _t1
      self

def bannerpeak.session.Session.keep_foes?(+foes: J.Val, +n: U32, +wp: U32, +hp: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.keep_foes.if2(U32.is_ge(n, J.len(foes)), foes, n, wp, hp, acc)

def bannerpeak.session.Session.keep_foes.if1?(bpc: Bool, +foes: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.nth(foes, n))
      acc
    case False{}:
      acc

def bannerpeak.session.Session.keep_foes.if2?(bpc: Bool, +foes: J.Val, +n: U32, +wp: U32, +hp: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = bannerpeak.session.Session.keep_foes.if1(bannerpeak.foes.Foe.valid(J.nth(foes, n), wp, hp), foes, n, acc)
      bannerpeak.session.Session.keep_foes(foes, (n + 1 : U32), wp, hp, acc)

def bannerpeak.session.Session.keep_items?(+items: J.Val, +n: U32, +wp: U32, +hp: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.keep_items.if2(U32.is_ge(n, J.len(items)), items, n, wp, hp, acc)

def bannerpeak.session.Session.keep_items.if1?(bpc: Bool, +items: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.nth(items, n))
      acc
    case False{}:
      acc

def bannerpeak.session.Session.keep_items.if2?(bpc: Bool, +items: J.Val, +n: U32, +wp: U32, +hp: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = bannerpeak.session.Session.keep_items.if1(bannerpeak.foes.Item.valid(J.nth(items, n), wp, hp), items, n, acc)
      bannerpeak.session.Session.keep_items(items, (n + 1 : U32), wp, hp, acc)

def bannerpeak.session.Session.keep_bolts?(+bolts: J.Val, +n: U32, +wp: U32, +hp: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.keep_bolts.if2(U32.is_ge(n, J.len(bolts)), bolts, n, wp, hp, acc)

def bannerpeak.session.Session.keep_bolts.if1?(bpc: Bool, +bolts: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.nth(bolts, n))
      acc
    case False{}:
      acc

def bannerpeak.session.Session.keep_bolts.if2?(bpc: Bool, +bolts: J.Val, +n: U32, +wp: U32, +hp: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = bannerpeak.session.Session.keep_bolts.if1(bannerpeak.foes.Bolt.valid(J.nth(bolts, n), wp, hp), bolts, n, acc)
      bannerpeak.session.Session.keep_bolts(bolts, (n + 1 : U32), wp, hp, acc)

def bannerpeak.session.Session.keep_boulders?(+bs: J.Val, +n: U32, +wp: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.keep_boulders.if2(U32.is_ge(n, J.len(bs)), bs, n, wp, acc)

def bannerpeak.session.Session.keep_boulders.if1?(bpc: Bool, +bs: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.nth(bs, n))
      acc
    case False{}:
      acc

def bannerpeak.session.Session.keep_boulders.if2?(bpc: Bool, +bs: J.Val, +n: U32, +wp: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = bannerpeak.session.Session.keep_boulders.if1(Bool.and(RS.i_ge(J.u32(J.get(J.nth(bs, n), "x")), 0), RS.i_le(J.u32(J.get(J.nth(bs, n), "x")), (wp * 256 : U32))), bs, n, acc)
      bannerpeak.session.Session.keep_boulders(bs, (n + 1 : U32), wp, acc)

def bannerpeak.session.Session.keep_fx?(+fx: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.keep_fx.if2(U32.is_ge(n, J.len(fx)), fx, n, acc)

def bannerpeak.session.Session.keep_fx.if1?(bpc: Bool, +fx: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.nth(fx, n))
      acc
    case False{}:
      acc

def bannerpeak.session.Session.keep_fx.if2?(bpc: Bool, +fx: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = bannerpeak.session.Session.keep_fx.if1(U32.is_le(J.u32(J.get(J.nth(fx, n), "t")), 14), fx, n, acc)
      bannerpeak.session.Session.keep_fx(fx, (n + 1 : U32), acc)

def bannerpeak.session.Session.keep_taken?(+taken: J.Val, +n: U32, +coins: U32, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.keep_taken.if2(U32.is_ge(n, J.len(taken)), taken, n, coins, acc)

def bannerpeak.session.Session.keep_taken.if1?(bpc: Bool, +taken: J.Val, +n: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +acc = J.push(acc, J.JU{J.u32(J.nth(taken, n))})
      acc
    case False{}:
      acc

def bannerpeak.session.Session.keep_taken.if2?(bpc: Bool, +taken: J.Val, +n: U32, +coins: U32, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +acc = bannerpeak.session.Session.keep_taken.if1(U32.is_lt(J.u32(J.nth(taken, n)), coins), taken, n, acc)
      bannerpeak.session.Session.keep_taken(taken, (n + 1 : U32), coins, acc)

def bannerpeak.session.Session.keep_changed?(+self: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  bannerpeak.session.Session.keep_changed.if2(bannerpeak.u.is_empty(ids), self, ids, acc)

def bannerpeak.session.Session.keep_changed.if1?(bpc: Bool, +acc: J.Val, +key: U32, +code: U32) -> J.Val:
  match bpc:
    case True{}:
      +_t1 = bannerpeak.u.view_insert(acc, key, code)
      +acc : J.Val = _t1
      acc
    case False{}:
      acc

def bannerpeak.session.Session.keep_changed.if2?(bpc: Bool, +self: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  match bpc:
    case True{}:
      acc
    case False{}:
      +key : U32 = bannerpeak.u.head(ids)
      +code : U32 = bannerpeak.u.at_u32(J.get(self, "changed"), key)
      +acc = bannerpeak.session.Session.keep_changed.if1(Bool.and(RS.i_lt(key, (434 * 64 : U32)), bannerpeak.world.Tile.known(code)), acc, key, code)
      bannerpeak.session.Session.keep_changed(self, bannerpeak.u.tail(ids), acc)

def bannerpeak.session.Session.repair?(+self: J.Val) -> J.Val:
  +self = J.set(self, "seats", bannerpeak.session.Session.keep_ids(J.get(self, "seats"), bannerpeak.u.ids(J.get(self, "seats")), J.obj()))
  +self = J.set(self, "elapsed", J.JU{RS.u_min(J.u32(J.get(self, "elapsed")), 108000)})
  +elapsed : U32 = J.u32(J.get(self, "elapsed"))
  +wp : U32 = (434 * 16 : U32)
  +hp : U32 = (64 * 16 : U32)
  +_t1 : Sigma<&2, &2, U32, _ => U32> = bannerpeak.session.Session.station(self)
  +sx = bannerpeak.u.fst(U32, U32, _t1)
  +sy = bannerpeak.u.snd(U32, U32, _t1)
  +_t2 = bannerpeak.session.Session.mend_seats(self, bannerpeak.u.ids(J.get(self, "seats")), elapsed, sx, sy)
  +self : J.Val = _t2
  +gates_len : U32 = bannerpeak.layout.info.gates_len()
  +self = J.set(self, "checkpoint", J.JU{RS.u_min(J.u32(J.get(self, "checkpoint")), gates_len)})
  +self = J.set(self, "gates", J.JU{U32.and(J.u32(J.get(self, "gates")), (RS.u_shl(1, gates_len) - 1 : U32))})
  +self = J.set(self, "doors", J.JU{U32.and(J.u32(J.get(self, "doors")), 3)})
  +self = J.set(self, "phase_t", J.JU{RS.u_min(J.u32(J.get(self, "phase_t")), 108000)})
  +self = J.set(self, "cursor", J.JU{RS.u_min(J.u32(J.get(self, "cursor")), (6 - 1 : U32))})
  +self = J.set(self, "foes", bannerpeak.session.Session.keep_foes(J.get(self, "foes"), 0, wp, hp, bannerpeak.u.vec_new()))
  +self = J.set(self, "foes", J.truncate(J.get(self, "foes"), 128))
  +self = J.set(self, "items", bannerpeak.session.Session.keep_items(J.get(self, "items"), 0, wp, hp, bannerpeak.u.vec_new()))
  +self = J.set(self, "items", J.truncate(J.get(self, "items"), 48))
  +self = J.set(self, "bolts", bannerpeak.session.Session.keep_bolts(J.get(self, "bolts"), 0, wp, hp, bannerpeak.u.vec_new()))
  +self = J.set(self, "bolts", J.truncate(J.get(self, "bolts"), 16))
  +self = J.set(self, "boulders", bannerpeak.session.Session.keep_boulders(J.get(self, "boulders"), 0, wp, bannerpeak.u.vec_new()))
  +self = J.set(self, "boulders", J.truncate(J.get(self, "boulders"), 8))
  +self = bannerpeak.session.Session.repair.if1(Bool.not(bannerpeak.boss.Boss.valid(J.get(self, "boss"), 6)), self)
  +self = J.set(self, "fx", bannerpeak.session.Session.keep_fx(J.get(self, "fx"), 0, bannerpeak.u.vec_new()))
  +self = J.set(self, "fx", J.truncate(J.get(self, "fx"), 48))
  +self = J.set(self, "taken", bannerpeak.session.Session.keep_taken(J.get(self, "taken"), 0, bannerpeak.layout.info.coins_len(), bannerpeak.u.vec_new()))
  +self = J.set(self, "changed", bannerpeak.session.Session.keep_changed(self, bannerpeak.u.ids(J.get(self, "changed")), J.obj()))
  +self = bannerpeak.session.Session.repair.if2(U32.is_gt(bannerpeak.u.map_len(J.get(self, "changed")), 2048), self)
  +self = bannerpeak.session.Session.repair.if3(Bool.and(Bool.not(J.is_null(J.get(self, "announce"))), Bool.or(U32.is_ge(J.u32(J.get(J.get(self, "announce"), "banner")), 8), U32.is_gt(J.u32(J.get(J.get(self, "announce"), "t")), 110))), self)
  +self = J.set(self, "wipe", J.JU{RS.u_min(J.u32(J.get(self, "wipe")), 20)})
  +self = J.set(self, "scaled", J.JU{RS.u_min(J.u32(J.get(self, "scaled")), 6)})
  +self = bannerpeak.session.Session.repair.if4(Bool.and(Bool.not(J.is_null(J.get(self, "completed"))), U32.is_gt(J.u32(J.get(self, "completed")), elapsed)), self)
  +self = bannerpeak.session.Session.repair.if5(Bool.and(String.eq(J.str(J.get(self, "phase")), "Finale"), U32.is_ne(J.u32(J.get(self, "banners")), 255)), self)
  self

def bannerpeak.session.Session.repair.if1?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "boss", bannerpeak.boss.Boss.new(RS.u_max(bannerpeak.session.Session.seat_count(self), 1)))
      self
    case False{}:
      self

def bannerpeak.session.Session.repair.if2?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "changed", J.obj())
      self
    case False{}:
      self

def bannerpeak.session.Session.repair.if3?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "announce", J.JNull{})
      self
    case False{}:
      self

def bannerpeak.session.Session.repair.if4?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "completed", J.JNull{})
      self
    case False{}:
      self

def bannerpeak.session.Session.repair.if5?(bpc: Bool, +self: J.Val) -> J.Val:
  match bpc:
    case True{}:
      +self = J.set(self, "phase", J.JS{"Play"})
      self
    case False{}:
      self

def case.bannerpeak.session.Session.repair?(+input: J.Val) -> J.Val:
  bannerpeak.session.Session.repair(J.get(input, "session"))

def bannerpeak.session.View.tile?(+self: J.Val, +tx: U32, +ty: U32) -> U32:
  bannerpeak.session.View.tile.if3(Bool.and(Bool.and(RS.i_ge(tx, 0), RS.i_lt(tx, 434)), Bool.and(RS.i_ge(ty, 0), RS.i_lt(ty, 64))), self, tx, ty)

def bannerpeak.session.View.tile.if1?(bpc: Bool, +tx: U32, +ty: U32, +c: U32) -> U32:
  match bpc:
    case True{}:
      c
    case False{}:
      bannerpeak.world.World.at(tx, ty)

def bannerpeak.session.View.tile.if2?(bpc: Bool, +self: J.Val, +tx: U32, +ty: U32, +key: U32) -> U32:
  match bpc:
    case True{}:
      +c : U32 = bannerpeak.u.at_u32(self, key)
      bannerpeak.session.View.tile.if1(bannerpeak.world.Tile.known(c), tx, ty, c)
    case False{}:
      bannerpeak.world.World.at(tx, ty)

def bannerpeak.session.View.tile.if3?(bpc: Bool, +self: J.Val, +tx: U32, +ty: U32) -> U32:
  match bpc:
    case True{}:
      +key : U32 = ((ty * 434 : U32) + tx : U32)
      bannerpeak.session.View.tile.if2(bannerpeak.u.has(self, key), self, tx, ty, key)
    case False{}:
      bannerpeak.world.World.at(tx, ty)

def bannerpeak.layout.sine_table?(+d: U32) -> U32:
  bannerpeak.u.nth([0, 18, 36, 54, 71, 89, 107, 125, 143, 160, 178, 195, 213, 230, 248, 265, 282, 299, 316, 333, 350, 367, 384, 400, 416, 433, 449, 465, 481, 496, 512, 527, 543, 558, 573, 587, 602, 616, 630, 644, 658, 672, 685, 698, 711, 724, 737, 749, 761, 773, 784, 796, 807, 818, 828, 839, 849, 859, 868, 878, 887, 896, 904, 912, 920, 928, 935, 943, 949, 956, 962, 968, 974, 979, 984, 989, 994, 998, 1002, 1005, 1008, 1011, 1014, 1016, 1018, 1020, 1022, 1023, 1023, 1024, 1024], d)

def bannerpeak.u.ife?(-A: Type, c: Bool, a: Unit -> A, b: Unit -> A) -> A:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def bannerpeak.u.fst?(-A: Data, -B: Data, +p: Sigma<&2, &2, A, _ => B>) -> A:
  match p:
    case (+a, +b):
      a

def bannerpeak.u.snd?(-A: Data, -B: Data, +p: Sigma<&2, &2, A, _ => B>) -> B:
  match p:
    case (+a, +b):
      b

# A bool as 0 or 1.

def bannerpeak.u.b2u?(+b: Bool) -> U32:
  Bool.pick(U32, b, 1, 0)

# i32 helpers beyond RS.

def bannerpeak.u.sat_sub?(+a: U32, +b: U32) -> U32:
  RS.u_sat_sub(a, b)

# Lists of u32 (static tables, id lists).

def bannerpeak.u.head?(+xs: List<&2, U32>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, +t}:
      h

def bannerpeak.u.tail?(+xs: List<&2, U32>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      t

def bannerpeak.u.is_empty?(+xs: List<&2, U32>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      False{}

def bannerpeak.u.nth?(+xs: List<&2, U32>, +n: U32) -> U32:
  bannerpeak.u.nth.go(xs, U32.to_nat(n))

def bannerpeak.u.nth.go?(+xs: List<&2, U32>, +n: Nat) -> U32:
  match xs n:
    case Nil{} _:
      0
    case Con{+h, +t} 0n:
      h
    case Con{+h, +t} 1n+p:
      bannerpeak.u.nth.go(t, p)

# A two-int tuple read from / written to a J array [a, b].

def bannerpeak.u.ri2?(+j: J.Val) -> Sigma<&2, &2, U32, _ => U32>:
  (J.u32(J.nth(j, 0)), J.u32(J.nth(j, 1)))

def bannerpeak.u.ji2?(+p: Sigma<&2, &2, U32, _ => U32>) -> J.Val:
  J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, p)), J.of_i32(bannerpeak.u.snd(U32, U32, p))]}

# The ids of an object keyed by decimal id, ascending.

def bannerpeak.u.ids?(+m: J.Val) -> List<&2, U32>:
  ga.ids(m)

# Object entry by numeric id.

def bannerpeak.u.at?(+m: J.Val, +id: U32) -> J.Val:
  J.get(m, U32.show(id))

def bannerpeak.u.at_u32?(+m: J.Val, +id: U32) -> U32:
  J.u32(J.get(m, U32.show(id)))

def bannerpeak.u.put?(+m: J.Val, +id: U32, +v: J.Val) -> J.Val:
  J.set(m, U32.show(id), v)

def bannerpeak.u.has?(+m: J.Val, +id: U32) -> Bool:
  J.has(m, U32.show(id))

def bannerpeak.u.del?(+m: J.Val, +id: U32) -> J.Val:
  J.del(m, U32.show(id))

# Vec::remove(i)

def bannerpeak.u.vec_remove?(+a: J.Val, +i: U32) -> J.Val:
  J.JA{ga.remove_nth(J.items(a), U32.to_nat(i))}

def bannerpeak.u.vec_len?(+a: J.Val) -> U32:
  J.len(a)

def bannerpeak.u.vec_new?() -> J.Val:
  J.JA{Nil{}}

def bannerpeak.u.vec_clear?(+a: J.Val) -> J.Val:
  J.JA{Nil{}}

# BTreeSet<u16> kept as a sorted J array: contains / insert.

def bannerpeak.u.set_has?(+a: J.Val, +x: U32) -> Bool:
  bannerpeak.u.set_has.go(J.items(a), x)

def bannerpeak.u.set_has.go?(+xs: List<&2, J.Val>, +x: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(U32.is_eq(J.u32(h), x), bannerpeak.u.set_has.go(t, x))

def bannerpeak.u.set_insert?(+a: J.Val, +x: U32) -> J.Val:
  J.JA{bannerpeak.u.set_insert.go(J.items(a), x)}

def bannerpeak.u.set_insert.go?(+xs: List<&2, J.Val>, +x: U32) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      [J.JU{x}]
    case Con{+h, +t}:
      bannerpeak.u.set_insert.at(U32.is_lt(x, J.u32(h)), U32.is_eq(x, J.u32(h)), h, t, x)

def bannerpeak.u.set_insert.at?(+before: Bool, +same: Bool, +h: J.Val, +t: List<&2, J.Val>, +x: U32) -> List<&2, J.Val>:
  match before:
    case True{}:
      Con{J.JU{x}, Con{h, t}}
    case False{}:
      match same:
        case True{}:
          Con{h, t}
        case False{}:
          Con{h, bannerpeak.u.set_insert.go(t, x)}

# A J array, or a new empty one when the field is missing.

def bannerpeak.u.vec_or?(+j: J.Val) -> J.Val:
  match j:
    case J.JNull{}:
      J.JA{Nil{}}
    case _:
      j

# Input::default(): nothing pressed (serde writes the omitted-when-false fields only when set).

def bannerpeak.u.input_default?() -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "move_x", J.JU{0}), "move_y", J.JU{0}), "aim_x", J.JU{0}), "aim_y", J.JU{0}), "fire", J.JBool{False{}}), "dash", J.JBool{False{}}), "bomb", J.JBool{False{}}), "swing", J.JBool{False{}}), "play", J.JU{0}), "cast", J.JU{0})

# The number of entries of an object.

def bannerpeak.u.map_len?(+m: J.Val) -> U32:
  U32.from_nat(List.length(&2, Sigma<&2, &2, String, _ => J.Val>, J.fields(m)))

# The session's table of changed tiles: tile `key` becomes `code`.

def bannerpeak.u.view_insert?(+m: J.Val, +key: U32, +code: U32) -> J.Val:
  J.set(m, U32.show(key), J.JU{code})

# The tables of the course board: the banner and the gate of each line.

def bannerpeak.u.board_banner?(+i: U32) -> U32:
  bannerpeak.u.nth([2, 3, 4, 5, 6, 7], i)

def bannerpeak.u.board_gate?(+i: U32) -> U32:
  bannerpeak.u.nth([0, 2, 3, 5, 0, 7], i)

# Flagpole scores by height.

def bannerpeak.u.pole_score?(+i: U32) -> U32:
  bannerpeak.u.nth([100, 200, 400, 800, 1000, 2000, 4000, 5000, 8000, 10000], i)

def bannerpeak.world.TILE?() -> U32:
  16

def bannerpeak.world.AIR?() -> U32:
  0

def bannerpeak.world.STONE?() -> U32:
  1

def bannerpeak.world.SNOW?() -> U32:
  2

def bannerpeak.world.LEDGE?() -> U32:
  3

def bannerpeak.world.COIN?() -> U32:
  4

def bannerpeak.world.POWER?() -> U32:
  5

def bannerpeak.world.SPENT?() -> U32:
  6

def bannerpeak.world.BRICK?() -> U32:
  7

def bannerpeak.world.CRACKED?() -> U32:
  8

def bannerpeak.world.WATER?() -> U32:
  9

def bannerpeak.world.SPIKES?() -> U32:
  10

def bannerpeak.world.HATCH?() -> U32:
  11

def bannerpeak.world.GATE?() -> U32:
  12

def bannerpeak.world.MANY?() -> U32:
  20

def bannerpeak.world.Tile.solid?(+t: U32) -> Bool:
  Bool.not(Bool.or(Bool.or(Bool.or(U32.is_eq(t, 0), U32.is_eq(t, 3)), U32.is_eq(t, 9)), U32.is_eq(t, 10)))

def bannerpeak.world.Tile.one_way?(+t: U32) -> Bool:
  U32.is_eq(t, 3)

def bannerpeak.world.Tile.rune?(+t: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(t, 4), U32.is_eq(t, 5)), Bool.and(U32.is_ge(t, 21), U32.is_le(t, 25)))

def bannerpeak.world.Tile.slippery?(+t: U32) -> Bool:
  U32.is_eq(t, 2)

def bannerpeak.world.Tile.known?(+b: U32) -> Bool:
  Bool.or(Bool.or(Bool.or(U32.is_le(b, 11), U32.is_eq(b, 12)), U32.is_eq(b, 13)), Bool.and(U32.is_ge(b, 21), U32.is_le(b, 25)))

def bannerpeak.world.World.at?(+tx: U32, +ty: U32) -> U32:
  bannerpeak.world.World.at.if2(Bool.or(RS.i_lt(tx, 0), RS.i_ge(tx, 434)), tx, ty)

def bannerpeak.world.World.at.if1?(bpc: Bool, +tx: U32, +ty: U32) -> U32:
  match bpc:
    case True{}:
      0
    case False{}:
      bannerpeak.world.World.runs_code(bannerpeak.layout.row(ty), tx)

def bannerpeak.world.World.at.if2?(bpc: Bool, +tx: U32, +ty: U32) -> U32:
  match bpc:
    case True{}:
      1
    case False{}:
      bannerpeak.world.World.at.if1(Bool.or(RS.i_lt(ty, 0), RS.i_ge(ty, 64)), tx, ty)

def bannerpeak.world.World.runs_code?(+xs: List<&2, U32>, +tx: U32) -> U32:
  bannerpeak.world.World.runs_code.if3(bannerpeak.u.is_empty(xs), xs, tx)

def bannerpeak.world.World.runs_code.if1?(bpc: Bool, +tx: U32, +code: U32, +r2: List<&2, U32>) -> U32:
  match bpc:
    case True{}:
      code
    case False{}:
      bannerpeak.world.World.runs_code(bannerpeak.u.tail(r2), tx)

def bannerpeak.world.World.runs_code.if2?(bpc: Bool, +tx: U32, +col: U32, +code: U32, +r2: List<&2, U32>, +len: U32) -> U32:
  match bpc:
    case True{}:
      0
    case False{}:
      bannerpeak.world.World.runs_code.if1(U32.is_lt(tx, (col + len : U32)), tx, code, r2)

def bannerpeak.world.World.runs_code.if3?(bpc: Bool, +xs: List<&2, U32>, +tx: U32) -> U32:
  match bpc:
    case True{}:
      0
    case False{}:
      +col : U32 = bannerpeak.u.head(xs)
      +r1 : List<&2, U32> = bannerpeak.u.tail(xs)
      +code : U32 = bannerpeak.u.head(r1)
      +r2 : List<&2, U32> = bannerpeak.u.tail(r1)
      +len : U32 = bannerpeak.u.head(r2)
      bannerpeak.world.World.runs_code.if2(U32.is_lt(tx, col), tx, col, code, r2, len)

def bannerpeak.world.Tile.props?(+t: U32) -> J.Val:
  J.set(J.set(J.set(J.set(J.obj(), "solid", J.JBool{bannerpeak.world.Tile.solid(t)}), "one_way", J.JBool{bannerpeak.world.Tile.one_way(t)}), "rune", J.JBool{bannerpeak.world.Tile.rune(t)}), "slippery", J.JBool{bannerpeak.world.Tile.slippery(t)})

def case.bannerpeak.world.World.at?(+input: J.Val) -> J.Val:
  J.JU{bannerpeak.world.World.at(J.u32(J.get(input, "tx")), J.u32(J.get(input, "ty")))}

def case.bannerpeak.world.Tile.known?(+input: J.Val) -> J.Val:
  J.JBool{bannerpeak.world.Tile.known(J.u32(J.get(input, "b")))}

def case.bannerpeak.world.Tile.props?(+input: J.Val) -> J.Val:
  bannerpeak.world.Tile.props(J.u32(J.get(input, "t")))

def bannerpeak.layout.row_0?() -> List<&2, U32>:
  []

def bannerpeak.layout.row_1?() -> List<&2, U32>:
  []

def bannerpeak.layout.row_2?() -> List<&2, U32>:
  []

def bannerpeak.layout.row_3?() -> List<&2, U32>:
  []

def bannerpeak.layout.row_4?() -> List<&2, U32>:
  []

def bannerpeak.layout.row_5?() -> List<&2, U32>:
  []

def bannerpeak.layout.row_6?() -> List<&2, U32>:
  []

def bannerpeak.layout.row_7?() -> List<&2, U32>:
  []

def bannerpeak.layout.row_8?() -> List<&2, U32>:
  []

def bannerpeak.layout.row_9?() -> List<&2, U32>:
  []

def bannerpeak.layout.row_10?() -> List<&2, U32>:
  [392, 1, 42]

def bannerpeak.layout.row_11?() -> List<&2, U32>:
  [392, 1, 2, 430, 1, 4]

def bannerpeak.layout.row_12?() -> List<&2, U32>:
  [387, 1, 2, 392, 1, 2, 430, 1, 4]

def bannerpeak.layout.row_13?() -> List<&2, U32>:
  [357, 1, 2, 363, 1, 2, 387, 1, 2, 392, 1, 2, 430, 1, 4]

def bannerpeak.layout.row_14?() -> List<&2, U32>:
  [357, 1, 2, 363, 1, 2, 387, 1, 2, 392, 1, 2, 430, 1, 4]

def bannerpeak.layout.row_15?() -> List<&2, U32>:
  [357, 1, 2, 363, 1, 2, 387, 1, 2, 392, 1, 2, 430, 1, 4]

def bannerpeak.layout.row_16?() -> List<&2, U32>:
  [275, 1, 4, 310, 3, 5, 357, 1, 2, 363, 1, 2, 387, 1, 2, 392, 1, 2, 430, 1, 4]

def bannerpeak.layout.row_17?() -> List<&2, U32>:
  [275, 1, 4, 335, 5, 1, 357, 1, 2, 363, 1, 2, 387, 1, 2, 430, 1, 4]

def bannerpeak.layout.row_18?() -> List<&2, U32>:
  [275, 1, 4, 357, 1, 2, 363, 1, 2, 387, 13, 2, 398, 3, 4, 424, 3, 4, 430, 1, 4]

def bannerpeak.layout.row_19?() -> List<&2, U32>:
  [275, 1, 4, 299, 3, 6, 317, 3, 6, 333, 3, 6, 349, 3, 6, 357, 1, 2, 363, 1, 2, 367, 3, 6, 387, 13, 2, 430, 1, 4]

def bannerpeak.layout.row_20?() -> List<&2, U32>:
  [275, 1, 4, 387, 13, 2, 430, 1, 4]

def bannerpeak.layout.row_21?() -> List<&2, U32>:
  [275, 1, 4, 307, 10, 2, 351, 10, 2, 387, 13, 2, 430, 1, 4]

def bannerpeak.layout.row_22?() -> List<&2, U32>:
  [275, 1, 4, 283, 1, 4, 287, 2, 105, 392, 1, 42]

def bannerpeak.layout.row_23?() -> List<&2, U32>:
  [275, 1, 4, 283, 1, 151]

def bannerpeak.layout.row_24?() -> List<&2, U32>:
  [275, 1, 4, 283, 1, 109]

def bannerpeak.layout.row_25?() -> List<&2, U32>:
  [275, 1, 4, 283, 1, 109]

def bannerpeak.layout.row_26?() -> List<&2, U32>:
  [275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_27?() -> List<&2, U32>:
  [275, 1, 4, 285, 1, 2]

def bannerpeak.layout.row_28?() -> List<&2, U32>:
  [275, 1, 4, 285, 1, 2]

def bannerpeak.layout.row_29?() -> List<&2, U32>:
  [275, 1, 4, 285, 1, 2]

def bannerpeak.layout.row_30?() -> List<&2, U32>:
  [275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_31?() -> List<&2, U32>:
  [275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_32?() -> List<&2, U32>:
  [0, 1, 1, 76, 1, 2, 275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_33?() -> List<&2, U32>:
  [0, 1, 1, 76, 1, 2, 275, 1, 2, 283, 1, 4]

def bannerpeak.layout.row_34?() -> List<&2, U32>:
  [0, 1, 1, 76, 1, 2, 275, 1, 2, 283, 1, 4]

def bannerpeak.layout.row_35?() -> List<&2, U32>:
  [0, 1, 1, 76, 1, 2, 275, 1, 2, 283, 1, 4]

def bannerpeak.layout.row_36?() -> List<&2, U32>:
  [0, 1, 1, 76, 1, 2, 275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_37?() -> List<&2, U32>:
  [0, 1, 1, 76, 1, 2, 275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_38?() -> List<&2, U32>:
  [0, 1, 1, 76, 1, 2, 275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_39?() -> List<&2, U32>:
  [0, 1, 1, 76, 1, 2, 275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_40?() -> List<&2, U32>:
  [0, 1, 1, 76, 1, 2, 275, 1, 2, 283, 1, 4]

def bannerpeak.layout.row_41?() -> List<&2, U32>:
  [0, 1, 1, 76, 1, 2, 275, 1, 2, 283, 1, 4]

def bannerpeak.layout.row_42?() -> List<&2, U32>:
  [0, 1, 1, 76, 1, 2, 174, 1, 3, 275, 1, 2, 283, 1, 4]

def bannerpeak.layout.row_43?() -> List<&2, U32>:
  [0, 1, 1, 56, 1, 9, 76, 1, 2, 128, 4, 1, 130, 5, 1, 132, 4, 1, 173, 1, 4, 275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_44?() -> List<&2, U32>:
  [0, 1, 1, 61, 1, 4, 76, 1, 2, 172, 1, 5, 223, 1, 10, 233, 3, 13, 275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_45?() -> List<&2, U32>:
  [0, 1, 1, 61, 1, 4, 76, 1, 2, 90, 4, 1, 91, 5, 1, 92, 4, 1, 108, 25, 1, 171, 1, 6, 223, 1, 10, 246, 1, 1, 253, 1, 2, 275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_46?() -> List<&2, U32>:
  [0, 1, 1, 61, 1, 4, 76, 12, 2, 170, 1, 7, 189, 4, 1, 190, 5, 1, 191, 4, 1, 223, 1, 10, 246, 1, 2, 275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_47?() -> List<&2, U32>:
  [0, 1, 1, 61, 1, 4, 76, 12, 2, 169, 1, 8, 223, 1, 10, 246, 1, 3, 275, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_48?() -> List<&2, U32>:
  [0, 1, 1, 61, 1, 4, 76, 12, 2, 119, 1, 5, 124, 11, 1, 125, 1, 17, 151, 1, 10, 168, 1, 9, 223, 1, 10, 246, 1, 4, 283, 1, 4]

def bannerpeak.layout.row_49?() -> List<&2, U32>:
  [0, 1, 1, 61, 1, 4, 76, 12, 2, 119, 1, 23, 151, 1, 10, 167, 1, 10, 223, 1, 10, 246, 1, 5, 283, 1, 4]

def bannerpeak.layout.row_50?() -> List<&2, U32>:
  [0, 1, 22, 22, 8, 4, 26, 1, 72, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 23, 142, 9, 9, 151, 1, 44, 195, 9, 5, 200, 1, 4, 204, 9, 5, 209, 1, 5, 214, 9, 9, 223, 1, 10, 233, 9, 13, 246, 1, 41]

def bannerpeak.layout.row_51?() -> List<&2, U32>:
  [0, 1, 22, 22, 8, 4, 26, 1, 72, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 23, 142, 9, 9, 151, 1, 44, 195, 9, 5, 200, 1, 4, 204, 9, 5, 209, 1, 5, 214, 9, 9, 223, 1, 10, 233, 9, 13, 246, 1, 41]

def bannerpeak.layout.row_52?() -> List<&2, U32>:
  [0, 1, 13, 34, 1, 64, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 23, 142, 9, 9, 151, 1, 44, 195, 9, 5, 200, 1, 4, 204, 9, 5, 209, 1, 5, 214, 9, 9, 223, 1, 10, 233, 9, 13, 246, 1, 41]

def bannerpeak.layout.row_53?() -> List<&2, U32>:
  [0, 1, 13, 23, 3, 3, 34, 1, 64, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 23, 142, 9, 9, 151, 1, 44, 195, 9, 5, 200, 1, 4, 204, 9, 5, 209, 1, 5, 214, 9, 9, 223, 1, 10, 233, 9, 13, 246, 1, 41]

def bannerpeak.layout.row_54?() -> List<&2, U32>:
  [0, 1, 13, 34, 1, 64, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 23, 142, 9, 9, 151, 1, 34]

def bannerpeak.layout.row_55?() -> List<&2, U32>:
  [0, 1, 13, 28, 5, 1, 34, 1, 64, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 23, 142, 9, 9, 151, 1, 34]

def bannerpeak.layout.row_56?() -> List<&2, U32>:
  [0, 1, 13, 19, 3, 3, 34, 1, 64, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 9, 140, 1, 2, 142, 9, 9, 151, 1, 34]

def bannerpeak.layout.row_57?() -> List<&2, U32>:
  [0, 1, 13, 34, 1, 64, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 9, 130, 5, 1, 140, 1, 2, 142, 9, 9, 151, 1, 34]

def bannerpeak.layout.row_58?() -> List<&2, U32>:
  [0, 1, 13, 15, 3, 3, 34, 1, 64, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 9, 140, 1, 2, 142, 9, 9, 151, 1, 34]

def bannerpeak.layout.row_59?() -> List<&2, U32>:
  [0, 1, 13, 34, 1, 64, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 9, 140, 1, 2, 142, 9, 9, 151, 1, 34]

def bannerpeak.layout.row_60?() -> List<&2, U32>:
  [0, 1, 98, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 9, 140, 1, 2, 142, 9, 9, 151, 1, 34]

def bannerpeak.layout.row_61?() -> List<&2, U32>:
  [0, 1, 98, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 9, 140, 1, 2, 142, 9, 9, 151, 1, 34]

def bannerpeak.layout.row_62?() -> List<&2, U32>:
  [80, 1, 18, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 17, 136, 11, 1, 137, 1, 5, 142, 9, 9, 151, 1, 34]

def bannerpeak.layout.row_63?() -> List<&2, U32>:
  [80, 1, 18, 98, 9, 4, 102, 1, 13, 115, 9, 4, 119, 1, 23, 142, 9, 9, 151, 1, 34]

def bannerpeak.layout.row?(+ty: U32) -> List<&2, U32>:
  bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 32), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 16), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 8), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 4), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 2), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 1), _ => bannerpeak.layout.row_0(), _ => bannerpeak.layout.row_1()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 3), _ => bannerpeak.layout.row_2(), _ => bannerpeak.layout.row_3())), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 6), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 5), _ => bannerpeak.layout.row_4(), _ => bannerpeak.layout.row_5()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 7), _ => bannerpeak.layout.row_6(), _ => bannerpeak.layout.row_7()))), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 12), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 10), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 9), _ => bannerpeak.layout.row_8(), _ => bannerpeak.layout.row_9()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 11), _ => bannerpeak.layout.row_10(), _ => bannerpeak.layout.row_11())), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 14), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 13), _ => bannerpeak.layout.row_12(), _ => bannerpeak.layout.row_13()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 15), _ => bannerpeak.layout.row_14(), _ => bannerpeak.layout.row_15())))), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 24), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 20), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 18), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 17), _ => bannerpeak.layout.row_16(), _ => bannerpeak.layout.row_17()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 19), _ => bannerpeak.layout.row_18(), _ => bannerpeak.layout.row_19())), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 22), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 21), _ => bannerpeak.layout.row_20(), _ => bannerpeak.layout.row_21()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 23), _ => bannerpeak.layout.row_22(), _ => bannerpeak.layout.row_23()))), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 28), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 26), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 25), _ => bannerpeak.layout.row_24(), _ => bannerpeak.layout.row_25()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 27), _ => bannerpeak.layout.row_26(), _ => bannerpeak.layout.row_27())), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 30), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 29), _ => bannerpeak.layout.row_28(), _ => bannerpeak.layout.row_29()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 31), _ => bannerpeak.layout.row_30(), _ => bannerpeak.layout.row_31()))))), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 48), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 40), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 36), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 34), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 33), _ => bannerpeak.layout.row_32(), _ => bannerpeak.layout.row_33()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 35), _ => bannerpeak.layout.row_34(), _ => bannerpeak.layout.row_35())), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 38), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 37), _ => bannerpeak.layout.row_36(), _ => bannerpeak.layout.row_37()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 39), _ => bannerpeak.layout.row_38(), _ => bannerpeak.layout.row_39()))), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 44), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 42), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 41), _ => bannerpeak.layout.row_40(), _ => bannerpeak.layout.row_41()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 43), _ => bannerpeak.layout.row_42(), _ => bannerpeak.layout.row_43())), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 46), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 45), _ => bannerpeak.layout.row_44(), _ => bannerpeak.layout.row_45()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 47), _ => bannerpeak.layout.row_46(), _ => bannerpeak.layout.row_47())))), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 56), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 52), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 50), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 49), _ => bannerpeak.layout.row_48(), _ => bannerpeak.layout.row_49()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 51), _ => bannerpeak.layout.row_50(), _ => bannerpeak.layout.row_51())), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 54), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 53), _ => bannerpeak.layout.row_52(), _ => bannerpeak.layout.row_53()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 55), _ => bannerpeak.layout.row_54(), _ => bannerpeak.layout.row_55()))), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 60), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 58), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 57), _ => bannerpeak.layout.row_56(), _ => bannerpeak.layout.row_57()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 59), _ => bannerpeak.layout.row_58(), _ => bannerpeak.layout.row_59())), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 62), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 61), _ => bannerpeak.layout.row_60(), _ => bannerpeak.layout.row_61()), _ => bannerpeak.u.ife(List<&2, U32>, U32.is_lt(ty, 63), _ => bannerpeak.layout.row_62(), _ => bannerpeak.layout.row_63()))))))

# Marks as the lists layout::info() keeps, flattened: x, y pairs (foes and hints: glyph, x, y).

def bannerpeak.layout.info.gates?() -> List<&2, U32>:
  [82, 49, 132, 47, 187, 49, 281, 49, 277, 35, 289, 21, 341, 21, 396, 21]

def bannerpeak.layout.info.sparks?() -> List<&2, U32>:
  [131, 60, 146, 42, 218, 41, 253, 44, 277, 42, 283, 29, 312, 15, 360, 12]

def bannerpeak.layout.info.coins?() -> List<&2, U32>:
  [311, 14, 312, 14, 313, 14, 300, 18, 301, 18, 302, 18, 303, 18, 318, 18, 319, 18, 320, 18, 321, 18, 350, 18, 351, 18, 352, 18, 353, 18, 368, 18, 369, 18, 370, 18, 371, 18, 234, 42, 236, 42, 238, 42, 240, 42, 242, 42, 244, 42, 142, 43, 144, 43, 148, 43, 150, 43, 120, 45, 121, 45, 122, 45, 123, 45, 124, 45, 33, 46, 34, 46, 35, 46, 36, 46, 37, 46, 95, 46, 96, 46, 97, 46, 104, 46, 105, 46, 106, 46, 152, 46, 153, 46, 154, 46, 155, 46, 156, 46, 157, 46, 158, 46, 9, 47, 10, 47, 11, 47, 12, 47, 13, 47, 46, 47, 47, 47, 48, 47, 49, 47, 50, 47, 51, 47, 196, 47, 197, 47, 198, 47, 199, 47, 180, 48, 181, 48, 182, 48, 183, 48, 254, 48, 255, 48, 256, 48, 257, 48, 258, 48, 259, 48, 260, 48, 261, 48, 28, 56, 29, 56, 30, 56, 31, 56, 32, 56, 15, 59, 16, 59, 17, 59, 18, 59, 19, 59, 129, 61, 130, 61, 131, 61, 132, 61, 133, 61, 134, 61, 135, 61]

def bannerpeak.layout.info.foes?() -> List<&2, U32>:
  [115, 296, 21, 101, 303, 21, 115, 313, 21, 99, 321, 21, 115, 327, 21, 115, 345, 21, 101, 347, 21, 99, 353, 21, 115, 359, 21, 102, 371, 21, 99, 375, 21, 119, 280, 26, 119, 281, 38, 119, 237, 41, 119, 205, 45, 115, 125, 47, 101, 126, 47, 99, 128, 47, 115, 131, 47, 115, 154, 47, 101, 156, 47, 115, 158, 47, 115, 94, 49, 101, 102, 49, 115, 104, 49, 99, 107, 49, 115, 111, 49, 102, 113, 49, 99, 163, 49, 115, 191, 49, 115, 257, 49, 101, 263, 49, 99, 265, 49, 102, 269, 49, 115, 28, 59]

def bannerpeak.layout.info.hints?() -> List<&2, U32>:
  [62, 314, 15, 60, 317, 18, 62, 316, 21, 124, 360, 21, 47, 140, 47, 38, 34, 48, 38, 35, 48, 38, 36, 48, 38, 37, 48, 38, 38, 48, 38, 39, 48, 38, 40, 48, 38, 41, 48, 38, 42, 48, 38, 43, 48, 38, 44, 48, 38, 45, 48, 38, 46, 48, 38, 47, 48, 38, 48, 48, 38, 49, 48, 38, 50, 48, 38, 51, 48, 38, 52, 48, 38, 53, 48, 38, 54, 48, 34, 256, 49, 62, 19, 55, 62, 20, 55, 62, 21, 55, 62, 15, 57, 62, 16, 57, 62, 17, 57, 62, 13, 59, 62, 14, 59]

# Banner pickups by banner number: x, y, 1 when it has a place, else 0, 0, 0.

def bannerpeak.layout.info.banners?() -> List<&2, U32>:
  [60, 42, 1, 31, 59, 1, 0, 0, 0, 230, 43, 1, 284, 21, 1, 381, 21, 1, 0, 0, 0, 412, 20, 1]

def bannerpeak.layout.info.start_x?() -> U32:
  4

def bannerpeak.layout.info.start_y?() -> U32:
  49

def bannerpeak.layout.info.pole_x?() -> U32:
  179

def bannerpeak.layout.info.pole_y?() -> U32:
  49

def bannerpeak.layout.info.great_pole_x?() -> U32:
  40

def bannerpeak.layout.info.great_pole_y?() -> U32:
  49

def bannerpeak.layout.info.finale_x?() -> U32:
  44

def bannerpeak.layout.info.finale_y?() -> U32:
  49

def bannerpeak.layout.info.arch_x?() -> U32:
  74

def bannerpeak.layout.info.arch_y?() -> U32:
  49

def bannerpeak.layout.info.board_x?() -> U32:
  86

def bannerpeak.layout.info.board_y?() -> U32:
  49

def bannerpeak.layout.info.cinder_x?() -> U32:
  412

def bannerpeak.layout.info.cinder_y?() -> U32:
  21

def bannerpeak.layout.info.gates_len?() -> U32:
  8

def bannerpeak.layout.info.sparks_len?() -> U32:
  8

def bannerpeak.layout.info.coins_len?() -> U32:
  96

def bannerpeak.layout.info.foes_len?() -> U32:
  35

def rules.Input.valid?(+i: J.Val) -> Bool:
  True{}

# crates/delve/src/rules.rs `Run::room` (core): the party's room, `dungeon.rooms[at]`.

def rules.Run.room?(+run: J.Val) -> J.Val:
  J.nth(J.get(J.get(run, "dungeon"), "rooms"), ga.u(run, "at"))

# crates/delve/src/rules/roundtable.rs `number` (the F2 numbers): `tune.roundtable[slot][i]`.

def roundtable.number?(+tune: J.Val, +slot: U32, +i: U32) -> U32:
  J.u32(J.nth(J.nth(J.get(tune, "roundtable"), slot), i))

# crates/delve/src/rules/forest.rs `Forest::leave`: the hunt, the song and the pads let go.

def forest.Forest.leave?(+f: J.Val) -> J.Val:
  +f1 = ga.su(f, "hunt_left", 0)
  +f2 = ga.su(f1, "caught", 0)
  +f3 = ga.sb(f2, "singing", False{})
  +f4 = ga.su(f3, "show_left", 0)
  +f5 = ga.su(f4, "note", 0)
  ga.su(f5, "held", 0)

# crates/delve/src/rules/exits.rs `Run::leave_activity`: the knight steps out to the
# kingdom (the run, and whether it did). The stub leaves everything as it is.

def exits.Run.leave_activity?(+run: J.Val, +id: U32) -> Sigma<&2, &2, J.Val, _ => Bool>:
  (run, False{})

# crates/delve/src/realm.rs `Spoils::add`: `n` more of a spoil (the object keyed by its
# snake_case name), saturating.

def realm.Spoils.add?(+sp: J.Val, +spoil: String, +n: U32) -> J.Val:
  Bool.pick(J.Val, U32.is_zero(n), sp, J.set(sp, spoil, J.JU{ga.sadd32(ga.u(sp, spoil), n)}))

def cases.run?(+name: String, +input: J.Val) -> J.Val:
  cases.run.n0(String.eq(name, "bannerpeak.ACTIVITY.end"), name, input)

def cases.run.n0?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.ACTIVITY.end(input)
    case False{}:
      cases.run.n1(String.eq(name, "bannerpeak.ACTIVITY.ride"), name, input)

def cases.run.n1?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.ACTIVITY.ride(input)
    case False{}:
      cases.run.n2(String.eq(name, "bannerpeak.ADVENTURE.bot"), name, input)

def cases.run.n2?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.ADVENTURE.bot(input)
    case False{}:
      cases.run.n3(String.eq(name, "bannerpeak.ADVENTURE.clear"), name, input)

def cases.run.n3?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.ADVENTURE.clear(input)
    case False{}:
      cases.run.n4(String.eq(name, "bannerpeak.ADVENTURE.seated"), name, input)

def cases.run.n4?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.ADVENTURE.seated(input)
    case False{}:
      cases.run.n5(String.eq(name, "bannerpeak.ADVENTURE.valid"), name, input)

def cases.run.n5?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.ADVENTURE.valid(input)
    case False{}:
      cases.run.n6(String.eq(name, "bannerpeak.Run.bannerpeak_knobs"), name, input)

def cases.run.n6?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.Run.bannerpeak_knobs(input)
    case False{}:
      cases.run.n7(String.eq(name, "bannerpeak.Run.bannerpeak_pays"), name, input)

def cases.run.n7?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.Run.bannerpeak_pays(input)
    case False{}:
      cases.run.n8(String.eq(name, "bannerpeak.Run.bannerpeak_showing"), name, input)

def cases.run.n8?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.Run.bannerpeak_showing(input)
    case False{}:
      cases.run.n9(String.eq(name, "bannerpeak.Run.start_bannerpeak"), name, input)

def cases.run.n9?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.Run.start_bannerpeak(input)
    case False{}:
      cases.run.n10(String.eq(name, "bannerpeak.Run.tick_bannerpeak"), name, input)

def cases.run.n10?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.Run.tick_bannerpeak(input)
    case False{}:
      cases.run.n11(String.eq(name, "bannerpeak.Run.tick_bannerpeak.done"), name, input)

def cases.run.n11?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.Run.tick_bannerpeak.done(input)
    case False{}:
      cases.run.n12(String.eq(name, "bannerpeak.body.Body.blocked"), name, input)

def cases.run.n12?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.blocked(input)
    case False{}:
      cases.run.n13(String.eq(name, "bannerpeak.body.Body.bounce"), name, input)

def cases.run.n13?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.bounce(input)
    case False{}:
      cases.run.n14(String.eq(name, "bannerpeak.body.Body.free"), name, input)

def cases.run.n14?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.free(input)
    case False{}:
      cases.run.n15(String.eq(name, "bannerpeak.body.Body.gravity"), name, input)

def cases.run.n15?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.gravity(input)
    case False{}:
      cases.run.n16(String.eq(name, "bannerpeak.body.Body.hurt"), name, input)

def cases.run.n16?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.hurt(input)
    case False{}:
      cases.run.n17(String.eq(name, "bannerpeak.body.Body.landed"), name, input)

def cases.run.n17?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.landed(input)
    case False{}:
      cases.run.n18(String.eq(name, "bannerpeak.body.Body.launch"), name, input)

def cases.run.n18?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.launch(input)
    case False{}:
      cases.run.n19(String.eq(name, "bannerpeak.body.Body.move_x"), name, input)

def cases.run.n19?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.move_x(input)
    case False{}:
      cases.run.n20(String.eq(name, "bannerpeak.body.Body.move_y"), name, input)

def cases.run.n20?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.move_y(input)
    case False{}:
      cases.run.n21(String.eq(name, "bannerpeak.body.Body.new"), name, input)

def cases.run.n21?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.new(input)
    case False{}:
      cases.run.n22(String.eq(name, "bannerpeak.body.Body.pick_jump"), name, input)

def cases.run.n22?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.pick_jump(input)
    case False{}:
      cases.run.n23(String.eq(name, "bannerpeak.body.Body.place"), name, input)

def cases.run.n23?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.place(input)
    case False{}:
      cases.run.n24(String.eq(name, "bannerpeak.body.Body.regrow"), name, input)

def cases.run.n24?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.regrow(input)
    case False{}:
      cases.run.n25(String.eq(name, "bannerpeak.body.Body.settle_pose"), name, input)

def cases.run.n25?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.settle_pose(input)
    case False{}:
      cases.run.n26(String.eq(name, "bannerpeak.body.Body.small"), name, input)

def cases.run.n26?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.small(input)
    case False{}:
      cases.run.n27(String.eq(name, "bannerpeak.body.Body.step"), name, input)

def cases.run.n27?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.step(input)
    case False{}:
      cases.run.n28(String.eq(name, "bannerpeak.body.Body.unstick"), name, input)

def cases.run.n28?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.unstick(input)
    case False{}:
      cases.run.n29(String.eq(name, "bannerpeak.body.Body.valid"), name, input)

def cases.run.n29?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.valid(input)
    case False{}:
      cases.run.n30(String.eq(name, "bannerpeak.body.Body.wall_check"), name, input)

def cases.run.n30?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.wall_check(input)
    case False{}:
      cases.run.n31(String.eq(name, "bannerpeak.body.Body.wall_side"), name, input)

def cases.run.n31?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.Body.wall_side(input)
    case False{}:
      cases.run.n32(String.eq(name, "bannerpeak.body.approach"), name, input)

def cases.run.n32?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.body.approach(input)
    case False{}:
      cases.run.n33(String.eq(name, "bannerpeak.boss.Boss.new"), name, input)

def cases.run.n33?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.boss.Boss.new(input)
    case False{}:
      cases.run.n34(String.eq(name, "bannerpeak.boss.Boss.seated"), name, input)

def cases.run.n34?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.boss.Boss.seated(input)
    case False{}:
      cases.run.n35(String.eq(name, "bannerpeak.boss.Boss.shape"), name, input)

def cases.run.n35?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.boss.Boss.shape(input)
    case False{}:
      cases.run.n36(String.eq(name, "bannerpeak.boss.Boss.valid"), name, input)

def cases.run.n36?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.boss.Boss.valid(input)
    case False{}:
      cases.run.n37(String.eq(name, "bannerpeak.boss.floor"), name, input)

def cases.run.n37?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.boss.floor(input)
    case False{}:
      cases.run.n38(String.eq(name, "bannerpeak.boss.home"), name, input)

def cases.run.n38?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.boss.home(input)
    case False{}:
      cases.run.n39(String.eq(name, "bannerpeak.bot.Session.bot_input"), name, input)

def cases.run.n39?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.bot_input(input)
    case False{}:
      cases.run.n40(String.eq(name, "bannerpeak.bot.Session.climb"), name, input)

def cases.run.n40?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.climb(input)
    case False{}:
      cases.run.n41(String.eq(name, "bannerpeak.bot.Session.drive"), name, input)

def cases.run.n41?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.drive(input)
    case False{}:
      cases.run.n42(String.eq(name, "bannerpeak.bot.Session.fight"), name, input)

def cases.run.n42?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.fight(input)
    case False{}:
      cases.run.n43(String.eq(name, "bannerpeak.bot.Session.foresee_kick"), name, input)

def cases.run.n43?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.foresee_kick(input)
    case False{}:
      cases.run.n44(String.eq(name, "bannerpeak.bot.Session.goal"), name, input)

def cases.run.n44?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.goal(input)
    case False{}:
      cases.run.n45(String.eq(name, "bannerpeak.bot.Session.hint_here"), name, input)

def cases.run.n45?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.hint_here(input)
    case False{}:
      cases.run.n46(String.eq(name, "bannerpeak.bot.Session.mill_goal"), name, input)

def cases.run.n46?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.mill_goal(input)
    case False{}:
      cases.run.n47(String.eq(name, "bannerpeak.bot.Session.obstacle_ahead"), name, input)

def cases.run.n47?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.obstacle_ahead(input)
    case False{}:
      cases.run.n48(String.eq(name, "bannerpeak.bot.Session.ride"), name, input)

def cases.run.n48?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.ride(input)
    case False{}:
      cases.run.n49(String.eq(name, "bannerpeak.bot.Session.shaft_goal"), name, input)

def cases.run.n49?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.shaft_goal(input)
    case False{}:
      cases.run.n50(String.eq(name, "bannerpeak.bot.Session.snow_goal"), name, input)

def cases.run.n50?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.snow_goal(input)
    case False{}:
      cases.run.n51(String.eq(name, "bannerpeak.bot.Session.spark_missing"), name, input)

def cases.run.n51?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.spark_missing(input)
    case False{}:
      cases.run.n52(String.eq(name, "bannerpeak.bot.Session.summit_goal"), name, input)

def cases.run.n52?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.summit_goal(input)
    case False{}:
      cases.run.n53(String.eq(name, "bannerpeak.bot.Session.walk_with_hops"), name, input)

def cases.run.n53?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.bot.Session.walk_with_hops(input)
    case False{}:
      cases.run.n54(String.eq(name, "bannerpeak.foes.Bolt.rect"), name, input)

def cases.run.n54?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Bolt.rect(input)
    case False{}:
      cases.run.n55(String.eq(name, "bannerpeak.foes.Bolt.step"), name, input)

def cases.run.n55?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Bolt.step(input)
    case False{}:
      cases.run.n56(String.eq(name, "bannerpeak.foes.Bolt.valid"), name, input)

def cases.run.n56?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Bolt.valid(input)
    case False{}:
      cases.run.n57(String.eq(name, "bannerpeak.foes.Foe.defeat"), name, input)

def cases.run.n57?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Foe.defeat(input)
    case False{}:
      cases.run.n58(String.eq(name, "bannerpeak.foes.Foe.new"), name, input)

def cases.run.n58?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Foe.new(input)
    case False{}:
      cases.run.n59(String.eq(name, "bannerpeak.foes.Foe.small"), name, input)

def cases.run.n59?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Foe.small(input)
    case False{}:
      cases.run.n60(String.eq(name, "bannerpeak.foes.Foe.step"), name, input)

def cases.run.n60?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Foe.step(input)
    case False{}:
      cases.run.n61(String.eq(name, "bannerpeak.foes.Foe.valid"), name, input)

def cases.run.n61?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Foe.valid(input)
    case False{}:
      cases.run.n62(String.eq(name, "bannerpeak.foes.Item.new"), name, input)

def cases.run.n62?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Item.new(input)
    case False{}:
      cases.run.n63(String.eq(name, "bannerpeak.foes.Item.small"), name, input)

def cases.run.n63?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Item.small(input)
    case False{}:
      cases.run.n64(String.eq(name, "bannerpeak.foes.Item.step"), name, input)

def cases.run.n64?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Item.step(input)
    case False{}:
      cases.run.n65(String.eq(name, "bannerpeak.foes.Item.valid"), name, input)

def cases.run.n65?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Item.valid(input)
    case False{}:
      cases.run.n66(String.eq(name, "bannerpeak.foes.Item.walk"), name, input)

def cases.run.n66?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.foes.Item.walk(input)
    case False{}:
      cases.run.n67(String.eq(name, "bannerpeak.knight.Knight.cmd"), name, input)

def cases.run.n67?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.knight.Knight.cmd(input)
    case False{}:
      cases.run.n68(String.eq(name, "bannerpeak.knight.Knight.hit"), name, input)

def cases.run.n68?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.knight.Knight.hit(input)
    case False{}:
      cases.run.n69(String.eq(name, "bannerpeak.knight.Knight.valid"), name, input)

def cases.run.n69?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.knight.Knight.valid(input)
    case False{}:
      cases.run.n70(String.eq(name, "bannerpeak.knight.follow"), name, input)

def cases.run.n70?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.knight.follow(input)
    case False{}:
      cases.run.n71(String.eq(name, "bannerpeak.layout.cell"), name, input)

def cases.run.n71?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.layout.cell(input)
    case False{}:
      cases.run.n72(String.eq(name, "bannerpeak.layout.cosine"), name, input)

def cases.run.n72?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.layout.cosine(input)
    case False{}:
      cases.run.n73(String.eq(name, "bannerpeak.layout.hatch_to"), name, input)

def cases.run.n73?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.layout.hatch_to(input)
    case False{}:
      cases.run.n74(String.eq(name, "bannerpeak.layout.info"), name, input)

def cases.run.n74?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.layout.info(input)
    case False{}:
      cases.run.n75(String.eq(name, "bannerpeak.layout.movers"), name, input)

def cases.run.n75?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.layout.movers(input)
    case False{}:
      cases.run.n76(String.eq(name, "bannerpeak.layout.ping_pong"), name, input)

def cases.run.n76?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.layout.ping_pong(input)
    case False{}:
      cases.run.n77(String.eq(name, "bannerpeak.layout.section_of"), name, input)

def cases.run.n77?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.layout.section_of(input)
    case False{}:
      cases.run.n78(String.eq(name, "bannerpeak.layout.sine"), name, input)

def cases.run.n78?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.layout.sine(input)
    case False{}:
      cases.run.n79(String.eq(name, "bannerpeak.layout.world"), name, input)

def cases.run.n79?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.layout.world(input)
    case False{}:
      cases.run.n80(String.eq(name, "bannerpeak.oracle.pattern"), name, input)

def cases.run.n80?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.oracle.pattern(input)
    case False{}:
      cases.run.n81(String.eq(name, "bannerpeak.oracle.slam"), name, input)

def cases.run.n81?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.oracle.slam(input)
    case False{}:
      cases.run.n82(String.eq(name, "bannerpeak.oracle.step"), name, input)

def cases.run.n82?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.oracle.step(input)
    case False{}:
      cases.run.n83(String.eq(name, "bannerpeak.session.Session.after_hit"), name, input)

def cases.run.n83?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.after_hit(input)
    case False{}:
      cases.run.n84(String.eq(name, "bannerpeak.session.Session.award"), name, input)

def cases.run.n84?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.award(input)
    case False{}:
      cases.run.n85(String.eq(name, "bannerpeak.session.Session.body_event"), name, input)

def cases.run.n85?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.body_event(input)
    case False{}:
      cases.run.n86(String.eq(name, "bannerpeak.session.Session.bump"), name, input)

def cases.run.n86?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.bump(input)
    case False{}:
      cases.run.n87(String.eq(name, "bannerpeak.session.Session.coin"), name, input)

def cases.run.n87?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.coin(input)
    case False{}:
      cases.run.n88(String.eq(name, "bannerpeak.session.Session.fall"), name, input)

def cases.run.n88?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.fall(input)
    case False{}:
      cases.run.n89(String.eq(name, "bannerpeak.session.Session.finale_step"), name, input)

def cases.run.n89?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.finale_step(input)
    case False{}:
      cases.run.n90(String.eq(name, "bannerpeak.session.Session.go_down"), name, input)

def cases.run.n90?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.go_down(input)
    case False{}:
      cases.run.n91(String.eq(name, "bannerpeak.session.Session.new"), name, input)

def cases.run.n91?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.new(input)
    case False{}:
      cases.run.n92(String.eq(name, "bannerpeak.session.Session.note_safe"), name, input)

def cases.run.n92?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.note_safe(input)
    case False{}:
      cases.run.n93(String.eq(name, "bannerpeak.session.Session.open_board"), name, input)

def cases.run.n93?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.open_board(input)
    case False{}:
      cases.run.n94(String.eq(name, "bannerpeak.session.Session.open_gates"), name, input)

def cases.run.n94?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.open_gates(input)
    case False{}:
      cases.run.n95(String.eq(name, "bannerpeak.session.Session.provision"), name, input)

def cases.run.n95?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.provision(input)
    case False{}:
      cases.run.n96(String.eq(name, "bannerpeak.session.Session.repair"), name, input)

def cases.run.n96?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.repair(input)
    case False{}:
      cases.run.n97(String.eq(name, "bannerpeak.session.Session.retry"), name, input)

def cases.run.n97?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.retry(input)
    case False{}:
      cases.run.n98(String.eq(name, "bannerpeak.session.Session.seat"), name, input)

def cases.run.n98?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.seat(input)
    case False{}:
      cases.run.n99(String.eq(name, "bannerpeak.session.Session.select_step"), name, input)

def cases.run.n99?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.select_step(input)
    case False{}:
      cases.run.n100(String.eq(name, "bannerpeak.session.Session.set_tile"), name, input)

def cases.run.n100?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.set_tile(input)
    case False{}:
      cases.run.n101(String.eq(name, "bannerpeak.session.Session.slam"), name, input)

def cases.run.n101?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.slam(input)
    case False{}:
      cases.run.n102(String.eq(name, "bannerpeak.session.Session.small"), name, input)

def cases.run.n102?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.small(input)
    case False{}:
      cases.run.n103(String.eq(name, "bannerpeak.session.Session.start_finale"), name, input)

def cases.run.n103?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.start_finale(input)
    case False{}:
      cases.run.n104(String.eq(name, "bannerpeak.session.Session.step"), name, input)

def cases.run.n104?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.step(input)
    case False{}:
      cases.run.n105(String.eq(name, "bannerpeak.session.Session.step_boulders"), name, input)

def cases.run.n105?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.step_boulders(input)
    case False{}:
      cases.run.n106(String.eq(name, "bannerpeak.session.Session.step_doors"), name, input)

def cases.run.n106?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.step_doors(input)
    case False{}:
      cases.run.n107(String.eq(name, "bannerpeak.session.Session.step_knight"), name, input)

def cases.run.n107?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.step_knight(input)
    case False{}:
      cases.run.n108(String.eq(name, "bannerpeak.session.Session.step_pole"), name, input)

def cases.run.n108?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.step_pole(input)
    case False{}:
      cases.run.n109(String.eq(name, "bannerpeak.session.Session.step_rescue"), name, input)

def cases.run.n109?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.step_rescue(input)
    case False{}:
      cases.run.n110(String.eq(name, "bannerpeak.session.Session.step_things"), name, input)

def cases.run.n110?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.step_things(input)
    case False{}:
      cases.run.n111(String.eq(name, "bannerpeak.session.Session.tile"), name, input)

def cases.run.n111?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.tile(input)
    case False{}:
      cases.run.n112(String.eq(name, "bannerpeak.session.Session.touch"), name, input)

def cases.run.n112?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.touch(input)
    case False{}:
      cases.run.n113(String.eq(name, "bannerpeak.session.Session.touch_cap"), name, input)

def cases.run.n113?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.touch_cap(input)
    case False{}:
      cases.run.n114(String.eq(name, "bannerpeak.session.Session.touch_things"), name, input)

def cases.run.n114?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.session.Session.touch_things(input)
    case False{}:
      cases.run.n115(String.eq(name, "bannerpeak.world.Tile.known"), name, input)

def cases.run.n115?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.world.Tile.known(input)
    case False{}:
      cases.run.n116(String.eq(name, "bannerpeak.world.Tile.props"), name, input)

def cases.run.n116?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.world.Tile.props(input)
    case False{}:
      cases.run.n117(String.eq(name, "bannerpeak.world.World.at"), name, input)

def cases.run.n117?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.bannerpeak.world.World.at(input)
    case False{}:
      J.JS{"no case wrapper"}
