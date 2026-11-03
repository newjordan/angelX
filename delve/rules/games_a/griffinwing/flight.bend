# crates/delve/src/rules/griffinwing/flight.rs — the craft in the air: steering
# with a banking lean, the free reticle, the tap and the hold-to-charge lock,
# the roll, the spur and the brake, Starfall, and the wingmen.
#
# A stage that changes both the session and one craft returns the workspace
# {"s": session, "c": craft} (gw.pk / gw.ps / gw.pc).

def gw.pk(+s: J.Val, +c: J.Val) -> J.Val:
  J.set(J.set(J.obj(), "s", s), "c", c)

def gw.ps(+w: J.Val) -> J.Val:
  J.get(w, "s")

def gw.pc(+w: J.Val) -> J.Val:
  J.get(w, "c")

def griffinwing.flight.K_FIRE() -> U32:
  1

def griffinwing.flight.K_DASH() -> U32:
  2

def griffinwing.flight.K_BOMB() -> U32:
  4

def griffinwing.flight.K_SPUR() -> U32:
  8

def griffinwing.flight.bits(+i: J.Val) -> U32:
  U32.or(
    U32.or(Bool.to_u32(ga.b(i, "fire")), U32.shln(Bool.to_u32(ga.b(i, "dash")), 1n)),
    U32.or(U32.shln(Bool.to_u32(ga.b(i, "bomb")), 2n), U32.or(U32.shln(Bool.to_u32(ga.b(i, "interact")), 3n), U32.shln(Bool.to_u32(ga.b(i, "swing")), 4n))))

# A seat's place in the formation: across and up, tenths of a unit.
def griffinwing.flight.slot.x(+id: U32) -> U32:
  Bool.pick(U32, U32.is_eq(id, 1), 0,
    Bool.pick(U32, U32.is_eq(id, 2), gw.neg(griffinwing.types.dm(22)),
      Bool.pick(U32, U32.is_eq(id, 3), griffinwing.types.dm(22),
        Bool.pick(U32, U32.is_eq(id, 4), gw.neg(griffinwing.types.dm(40)),
          Bool.pick(U32, U32.is_eq(id, 5), griffinwing.types.dm(40), 0)))))

def griffinwing.flight.slot.z(+id: U32) -> U32:
  Bool.pick(U32, U32.is_eq(id, 1), 0,
    Bool.pick(U32, Bool.or(U32.is_eq(id, 2), U32.is_eq(id, 3)), gw.neg(griffinwing.types.dm(4)),
      Bool.pick(U32, Bool.or(U32.is_eq(id, 4), U32.is_eq(id, 5)), gw.neg(griffinwing.types.dm(8)), griffinwing.types.dm(12))))

def griffinwing.flight.slot(+id: U32) -> J.Val:
  ga.pair_i(griffinwing.flight.slot.x(id), griffinwing.flight.slot.z(id))

# A rider's place: across, up and ahead of the knights.
def griffinwing.flight.rider_slot.x(+k: U32) -> U32:
  Bool.pick(U32, U32.is_eq(k, 0), gw.neg(griffinwing.types.dm(30)),
    Bool.pick(U32, U32.is_eq(k, 1), griffinwing.types.dm(30),
      Bool.pick(U32, U32.is_eq(k, 2), 0, gw.neg(griffinwing.types.dm(62)))))

def griffinwing.flight.rider_slot.z(+k: U32) -> U32:
  Bool.pick(U32, Bool.or(U32.is_eq(k, 0), U32.is_eq(k, 1)), griffinwing.types.dm(8),
    Bool.pick(U32, U32.is_eq(k, 2), griffinwing.types.dm(18), griffinwing.types.dm(2)))

def griffinwing.flight.rider_slot.back(+k: U32) -> U32:
  Bool.pick(U32, Bool.or(U32.is_eq(k, 0), U32.is_eq(k, 1)), griffinwing.types.dm(60),
    Bool.pick(U32, U32.is_eq(k, 2), griffinwing.types.dm(78), griffinwing.types.dm(50)))

def griffinwing.flight.rider_slot(+k: U32) -> J.Val:
  ga.trio(J.of_i32(griffinwing.flight.rider_slot.x(k)), J.of_i32(griffinwing.flight.rider_slot.z(k)), J.of_i32(griffinwing.flight.rider_slot.back(k)))

# How fast a craft steers, across and up, and how quickly it gets there.
def griffinwing.flight.V_X() -> U32:
  62

def griffinwing.flight.V_Z() -> U32:
  50

def griffinwing.flight.ACC() -> U32:
  14

def griffinwing.flight.RET_V() -> U32:
  100

def griffinwing.flight.RX_MAX() -> U32:
  griffinwing.types.dm(120)

def griffinwing.flight.RZ_MAX() -> U32:
  griffinwing.types.dm(80)

def griffinwing.flight.TAP_REST() -> U32:
  6

def griffinwing.flight.HOLD_AT() -> U32:
  7

def griffinwing.flight.REL_REST() -> U32:
  12

def griffinwing.flight.LOCK_R() -> U32:
  griffinwing.types.dm(16)

def griffinwing.flight.V_BOLT() -> U32:
  griffinwing.types.dm(22)

def griffinwing.flight.V_STAR() -> U32:
  griffinwing.types.dm(26)

def griffinwing.flight.BOLT_LIFE() -> U32:
  26

def griffinwing.flight.STAR_LIFE() -> U32:
  46

def griffinwing.flight.LANCE() -> U32:
  10

def griffinwing.flight.STAR_MIN() -> U32:
  16

def griffinwing.flight.STAR_MAX() -> U32:
  36

def griffinwing.flight.BOOST_BACK() -> U32:
  griffinwing.types.dm(55)

def griffinwing.flight.BRAKE_BACK() -> U32:
  griffinwing.types.dm(45)

def griffinwing.flight.BACK_V() -> U32:
  28

def griffinwing.flight.ROLL_REST() -> U32:
  12

def griffinwing.flight.ROLL_V() -> U32:
  84

def griffinwing.flight.TAP_WINDOW() -> U32:
  9

# The craft's body, for the bolts that hit it.
def griffinwing.flight.CRAFT_R() -> U32:
  griffinwing.types.dm(10)

# How far from a craft a rolling shield turns a bolt round.
def griffinwing.flight.REFLECT_R() -> U32:
  griffinwing.types.dm(26)

# What Starfall does to every foe it reaches.
def griffinwing.flight.STARFALL() -> U32:
  120

# The bolts' home pace (starbursts steer at it).
def griffinwing.flight.Session.star_speed() -> U32:
  griffinwing.flight.V_STAR()

# ─── where a craft is ───────────────────────────────────────────────────────

# The rail point a craft is at.
def griffinwing.flight.Session.craft_s(+s: J.Val, +c: J.Val) -> U32:
  (griffinwing.Session.anchor(s) + ga.u(c, "back") : U32)

# A craft by the id a bolt or a foe holds: a seat's, a rider's. Its place along,
# across and up, and whether it is flying. null: none by that id.
def griffinwing.flight.Session.craft_at(+s: J.Val, +id: U32) -> J.Val:
  gw.ifj(gw.within(id, 1, 6),
    _ => griffinwing.flight.Session.craft_at.seat(s, J.get(J.get(s, "seats"), U32.show(id))),
    _ => griffinwing.flight.Session.craft_at.rider(s, griffinwing.flight.Session.wing_find(ga.items(s, "wing"), id)))

def griffinwing.flight.Session.craft_at.seat(+s: J.Val, +seat: J.Val) -> J.Val:
  gw.ifj(J.is_null(seat),
    _ => J.JNull{},
    _ => griffinwing.flight.Session.craft_at.of(s, J.get(seat, "craft")))

def griffinwing.flight.Session.craft_at.of(+s: J.Val, +c: J.Val) -> J.Val:
  J.JA{[J.of_i32(griffinwing.flight.Session.craft_s(s, c)), J.of_i32(ga.u(c, "x")), J.of_i32(ga.u(c, "z")), J.JBool{griffinwing.types.Craft.flying(c)}]}

def griffinwing.flight.Session.craft_at.rider(+s: J.Val, +w: J.Val) -> J.Val:
  gw.ifj(J.is_null(w),
    _ => J.JNull{},
    _ => J.JA{[J.of_i32(griffinwing.flight.Session.craft_s(s, J.get(w, "craft"))), J.of_i32(ga.u(J.get(w, "craft"), "x")), J.of_i32(ga.u(J.get(w, "craft"), "z")),
      J.JBool{Bool.and(Bool.not(ga.b(w, "lost")), U32.is_zero(ga.u(J.get(w, "craft"), "down")))}]})

# `wing.iter().find(|w| w.id == id)`: the rider, or null.
def griffinwing.flight.Session.wing_find(+ws: List<&2, J.Val>, +id: U32) -> J.Val:
  match ws:
    case Nil{}:
      J.JNull{}
    case Con{+w, +t}:
      gw.ifj(U32.is_eq(ga.u(w, "id"), id), _ => w, _ => griffinwing.flight.Session.wing_find(t, id))

# The parts of a `craft_at` answer.
def gw.at_some(+r: J.Val) -> Bool:
  Bool.not(J.is_null(r))

def gw.at_s(+r: J.Val) -> U32:
  ga.nu(r, 0)

def gw.at_x(+r: J.Val) -> U32:
  ga.nu(r, 1)

def gw.at_z(+r: J.Val) -> U32:
  ga.nu(r, 2)

def gw.at_ok(+r: J.Val) -> Bool:
  J.bool(J.nth(r, 3))

# ─── one knight's tick in the air ───────────────────────────────────────────

def gw.put_craft(+s: J.Val, +id: U32, +c: J.Val) -> J.Val:
  ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id, ga.sv(ga.at(J.get(s, "seats"), id), "craft", c)))

def gw.craft_of(+s: J.Val, +id: U32) -> J.Val:
  J.get(ga.at(J.get(s, "seats"), id), "craft")

# Whether a key bit went down this tick: `on && old & key == 0`.
def gw.edge(+old: U32, +key: U32, +on: Bool) -> Bool:
  Bool.and(on, U32.is_zero(U32.and(old, key)))

def griffinwing.flight.Session.fly(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(Bool.not(ga.has_key(J.get(s, "seats"), id)),
    _ => s,
    _ => griffinwing.flight.Session.fly.seated(s, id, i, n, gw.craft_of(s, id)))

# The counters wind down; a fallen craft rises or stays down.
def griffinwing.flight.Session.fly.seated(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c0: J.Val) -> J.Val:
  +c1 = gw.subs(c0, "hurt", 1)
  +c2 = gw.subs(c1, "cool", 1)
  +c3 = gw.subs(c2, "rest", 1)
  +c4 = gw.subs(c3, "chain_ticks", 1)
  +c5 = gw.ifj(U32.is_zero(ga.u(c4, "chain_ticks")), _ => ga.su(c4, "chain", 0), _ => c4)
  +c6 = gw.subs(c5, "tap_ticks", 1)
  gw.ifj(U32.is_gt(ga.u(c6, "down"), 0),
    _ => gw.put_craft(s, id, griffinwing.flight.Session.fly.down(id, c6)),
    _ => griffinwing.flight.Session.fly.up(s, id, i, n, c6))

def griffinwing.flight.Session.fly.down(+id: U32, +c0: J.Val) -> J.Val:
  +c1 = ga.su(c0, "down", (ga.u(c0, "down") - 1 : U32))
  +c2 = ga.si(c1, "ds", griffinwing.types.PACE())
  +c3 = ga.su(c2, "held", 0)
  +c4 = ga.su(c3, "charge", 0)
  +c5 = ga.sv(c4, "locks", J.JA{[]})
  gw.ifj(U32.is_zero(ga.u(c5, "down")),
    _ => griffinwing.flight.Session.fly.rise(id, c5),
    _ => c5)

# Back in the wing, mended a little, unhurt for a moment.
def griffinwing.flight.Session.fly.rise(+id: U32, +c0: J.Val) -> J.Val:
  +c1 = ga.si(c0, "x", griffinwing.flight.slot.x(id))
  +c2 = ga.si(c1, "z", griffinwing.flight.slot.z(id))
  +c3 = ga.si(c2, "vx", 0)
  +c4 = ga.si(c3, "vz", 0)
  +c5 = ga.si(c4, "shield", 60)
  +c6 = ga.su(c5, "hurt", (2 * rules.HZ() : U32))
  ga.su(c6, "roll", 0)

def griffinwing.flight.Session.fly.up(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c0: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.up.roll(s, id, i, n, c0, ga.u(c0, "keys"), gw.double_tap(c0, ga.u(i, "move_x")))

# A second tap of a direction is a roll that way: the craft's tap state and
# whether it was a double.
def gw.double_tap(+c: J.Val, +mx: U32) -> J.Val:
  gw.ifj(Bool.and(U32.is_ne(mx, 0), U32.is_ne(ga.u(c, "last_x"), mx)),
    _ => gw.double_tap.in(c, mx),
    _ => gw.pk(J.JBool{False{}}, c))

def gw.double_tap.in(+c: J.Val, +mx: U32) -> J.Val:
  gw.ifj(Bool.and(U32.is_eq(ga.u(c, "tap_dir"), mx), U32.is_gt(ga.u(c, "tap_ticks"), 0)),
    _ => gw.pk(J.JBool{True{}}, ga.su(c, "tap_ticks", 0)),
    _ => gw.pk(J.JBool{False{}}, ga.su(ga.si(c, "tap_dir", mx), "tap_ticks", griffinwing.flight.TAP_WINDOW())))

def griffinwing.flight.Session.fly.up.roll(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c0: J.Val, +old: U32, +dt: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.up.roll2(s, id, i, n, ga.si(gw.pc(dt), "last_x", ga.u(i, "move_x")), old, J.bool(gw.ps(dt)))

def griffinwing.flight.Session.fly.up.roll2(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c: J.Val, +old: U32, +double: Bool) -> J.Val:
  gw.ifj(Bool.and(Bool.or(gw.edge(old, griffinwing.flight.K_DASH(), ga.b(i, "dash")), double),
      Bool.and(U32.is_zero(ga.u(c, "rest")), U32.is_zero(ga.u(c, "roll")))),
    _ => griffinwing.flight.Session.fly.up.step(
      griffinwing.Session.sound(s, "roll"), id, i, n, griffinwing.flight.Session.fly.rolled(c, i, n), old),
    _ => griffinwing.flight.Session.fly.up.step(s, id, i, n, c, old))

# The roll: a spinning shield, and a throw to the side.
def griffinwing.flight.Session.fly.rolled(+c0: J.Val, +i: J.Val, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.rolled.dir(c0, n,
    gw.ifu(U32.is_ne(ga.u(i, "move_x"), 0),
      _ => gw.sig(ga.u(i, "move_x")),
      _ => gw.ifu(U32.is_gt(gw.abs(ga.u(c0, "vx")), 8), _ => gw.sig(ga.u(c0, "vx")), _ => ga.u(c0, "dir"))))

def griffinwing.flight.Session.fly.rolled.dir(+c0: J.Val, +n: J.Val, +dir: U32) -> J.Val:
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
def griffinwing.flight.Session.fly.up.step(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c0: J.Val, +old: U32) -> J.Val:
  +c1 = ga.su(c0, "roll", ga.ssub(ga.u(c0, "roll"), 1))
  +c2 = griffinwing.flight.Session.fly.reins(c1, i, n)
  +c3 = griffinwing.flight.Session.fly.spur(c2, i, n)
  +c4 = griffinwing.flight.Session.fly.reticle(c3, i)
  +w5 = griffinwing.flight.Session.fly.fire(s, id, i, n, c4, old, griffinwing.flight.Session.craft_s(s, c4))
  +w6 = griffinwing.flight.Session.fly.bomb(gw.ps(w5), id, i, gw.pc(w5), old, griffinwing.flight.Session.craft_s(s, c4))
  gw.put_craft(gw.ps(w6), id, gw.pc(w6))

# The reins: speed toward the stick, the window's walls, the lean.
def griffinwing.flight.Session.fly.reins(+c0: J.Val, +i: J.Val, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.reins.move(
    c0, n,
    (ga.u(i, "move_x") * griffinwing.flight.V_X() : U32),
    gw.neg((ga.u(i, "move_y") * griffinwing.flight.V_Z() : U32)))

def griffinwing.flight.Session.fly.reins.move(+c0: J.Val, +n: J.Val, +tx: U32, +tz: U32) -> J.Val:
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
def griffinwing.flight.Session.fly.spur(+c0: J.Val, +i: J.Val, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.spur.go(
    c0, i,
    gw.div(griffinwing.types.METER(), U32.max(ga.u(n, "spur"), 1)),
    gw.div(griffinwing.types.METER(), gw.max(gw.div((U32.max(ga.u(n, "spur"), 1) * 5 : U32), 2), 1)))

def griffinwing.flight.Session.fly.spur.go(+c0: J.Val, +i: J.Val, +drain: U32, +refill: U32) -> J.Val:
  griffinwing.flight.Session.fly.spur.back(
    griffinwing.flight.Session.fly.spur.meter(
      ga.sb(
        ga.sb(c0, "boost", Bool.and(ga.b(i, "interact"), Bool.and(Bool.not(ga.b(c0, "spent")),
          Bool.and(gw.gt(ga.u(c0, "meter"), 0), U32.is_zero(ga.u(c0, "roll")))))),
        "brake", False{}),
      i, drain, refill),
    i)

def griffinwing.flight.Session.fly.spur.meter(+c0: J.Val, +i: J.Val, +drain: U32, +refill: U32) -> J.Val:
  +c1 = ga.sb(c0, "brake", Bool.and(ga.b(i, "swing"), Bool.not(ga.b(c0, "boost"))))
  gw.ifj(ga.b(c1, "boost"),
    _ => griffinwing.flight.Session.fly.spur.burn(ga.si(c1, "meter", gw.max((ga.u(c1, "meter") - drain : U32), 0))),
    _ => griffinwing.flight.Session.fly.spur.fill(ga.si(c1, "meter", gw.min((ga.u(c1, "meter") + refill : U32), griffinwing.types.METER()))))

def griffinwing.flight.Session.fly.spur.burn(+c: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(c, "meter")), _ => ga.sb(c, "spent", True{}), _ => c)

def griffinwing.flight.Session.fly.spur.fill(+c: J.Val) -> J.Val:
  gw.ifj(Bool.and(ga.b(c, "spent"), gw.ge(ga.u(c, "meter"), gw.div(griffinwing.types.METER(), 4))),
    _ => ga.sb(c, "spent", False{}),
    _ => c)

# How far the spur or the brake slides the craft along the rail.
def griffinwing.flight.Session.fly.spur.back(+c0: J.Val, +i: J.Val) -> J.Val:
  griffinwing.flight.Session.fly.spur.slide(c0,
    gw.ifu(ga.b(c0, "boost"), _ => griffinwing.flight.BOOST_BACK(),
      _ => gw.ifu(ga.b(c0, "brake"), _ => gw.neg(griffinwing.flight.BRAKE_BACK()), _ => 0)))

def griffinwing.flight.Session.fly.spur.slide(+c0: J.Val, +want: U32) -> J.Val:
  griffinwing.flight.Session.fly.spur.slide2(c0,
    (ga.u(c0, "back") + gw.clamp((want - ga.u(c0, "back") : U32), gw.neg(griffinwing.flight.BACK_V()), griffinwing.flight.BACK_V()) : U32))

def griffinwing.flight.Session.fly.spur.slide2(+c0: J.Val, +back: U32) -> J.Val:
  ga.si(ga.si(c0, "ds", (griffinwing.types.PACE() + back - ga.u(c0, "back") : U32)), "back", back)

# The reticle: instant, and it stays where it is put.
def griffinwing.flight.Session.fly.reticle(+c0: J.Val, +i: J.Val) -> J.Val:
  +c1 = ga.si(c0, "rx", gw.clamp((ga.u(c0, "rx") + (ga.u(i, "aim_x") * griffinwing.flight.RET_V() : U32) : U32), gw.neg(griffinwing.flight.RX_MAX()), griffinwing.flight.RX_MAX()))
  ga.si(c1, "rz", gw.clamp((ga.u(c1, "rz") - (ga.u(i, "aim_y") * griffinwing.flight.RET_V() : U32) : U32), gw.neg(griffinwing.flight.RZ_MAX()), griffinwing.flight.RZ_MAX()))

# Fire: a tap is a twin bolt; a hold charges and locks; letting go sends the
# locks.
def griffinwing.flight.Session.fly.fire(+s: J.Val, +id: U32, +i: J.Val, +n: J.Val, +c: J.Val, +old: U32, +s_c: U32) -> J.Val:
  gw.ifj(Bool.and(U32.is_zero(ga.u(c, "roll")), ga.b(i, "fire")),
    _ => griffinwing.flight.Session.fly.fire.on(s, id, n, c, old, s_c),
    _ => gw.ifj(U32.is_gt(ga.u(c, "held"), 0),
      _ => griffinwing.flight.Session.fly.fire.off(s, id, n, c, s_c),
      _ => gw.pk(s, c)))

def griffinwing.flight.Session.fly.fire.on(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +old: U32, +s_c: U32) -> J.Val:
  gw.ifj(U32.is_zero(U32.and(old, griffinwing.flight.K_FIRE())),
    _ => griffinwing.flight.Session.fly.fire.press(s, id, ga.su(c, "held", 1), s_c),
    _ => gw.ifj(U32.is_gt(ga.u(c, "held"), 0),
      _ => griffinwing.flight.Session.fly.fire.hold(s, ga.su(c, "held", U32.min((ga.u(c, "held") + 1 : U32), 30000)), n, s_c),
      _ => gw.pk(s, c)))

def griffinwing.flight.Session.fly.fire.press(+s: J.Val, +id: U32, +c: J.Val, +s_c: U32) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(c, "cool")),
    _ => gw.pk(griffinwing.flight.Session.tap(s, id, c, s_c), ga.su(c, "cool", griffinwing.flight.TAP_REST())),
    _ => gw.pk(s, c))

def griffinwing.flight.Session.fly.fire.hold(+s: J.Val, +c: J.Val, +n: J.Val, +s_c: U32) -> J.Val:
  gw.ifj(U32.is_gt(ga.u(c, "held"), griffinwing.flight.HOLD_AT()),
    _ => griffinwing.flight.Session.acquire(s, ga.su(c, "charge", U32.min((ga.u(c, "charge") + 1 : U32), ga.u(n, "charge"))), s_c),
    _ => gw.pk(s, c))

def griffinwing.flight.Session.fly.fire.off(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +s_c: U32) -> J.Val:
  gw.ifj(Bool.and(U32.is_gt(ga.u(c, "charge"), 0), U32.is_zero(ga.u(c, "roll"))),
    _ => griffinwing.flight.Session.fly.fire.let_go(griffinwing.flight.Session.release(s, id, c, s_c, n)),
    _ => griffinwing.flight.Session.fly.fire.let_go(gw.pk(s, c)))

def griffinwing.flight.Session.fly.fire.let_go(+w: J.Val) -> J.Val:
  +c0 = gw.pc(w)
  +c1 = ga.su(c0, "held", 0)
  +c2 = ga.su(c1, "charge", 0)
  +c3 = ga.su(c2, "dwell", 0)
  gw.pk(gw.ps(w), ga.sv(c3, "locks", J.JA{[]}))

# Starfall.
def griffinwing.flight.Session.fly.bomb(+s: J.Val, +id: U32, +i: J.Val, +c: J.Val, +old: U32, +s_c: U32) -> J.Val:
  gw.ifj(Bool.and(gw.edge(old, griffinwing.flight.K_BOMB(), ga.b(i, "bomb")), Bool.and(U32.is_gt(ga.u(c, "bombs"), 0), U32.is_zero(ga.u(c, "roll")))),
    _ => gw.pk(griffinwing.combat.Session.starfall(s, id, s_c, ga.u(c, "x"), ga.u(c, "z")), ga.su(c, "bombs", (ga.u(c, "bombs") - 1 : U32))),
    _ => gw.pk(s, c))

# ─── the bolts a craft fires ────────────────────────────────────────────────

def griffinwing.types.Bolt.make(+shot: String, +owner: U32, +s: U32, +x: U32, +z: U32, +vs: U32, +vx: U32, +vz: U32, +dmg: U32, +life: U32, +aim: U32, +src: U32, +delay: U32, +full: Bool) -> J.Val:
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

def griffinwing.flight.Session.push_bolt(+s: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(U32.is_lt(gw.len(s, "bolts"), griffinwing.types.MAX_BOLTS()),
    _ => ga.push(s, "bolts", b),
    _ => s)

# A tap: two bolts, side by side, both to where the reticle points.
def griffinwing.flight.Session.tap(+s: J.Val, +owner: U32, +c: J.Val, +s_c: U32) -> J.Val:
  griffinwing.Session.sound(
    griffinwing.flight.Session.tap.side(
      griffinwing.flight.Session.tap.side(s, owner, c, s_c, gw.neg(1)),
      owner, c, s_c, 1),
    "shot_bolt")

def griffinwing.flight.Session.tap.side(+s: J.Val, +owner: U32, +c: J.Val, +s_c: U32, +side: U32) -> J.Val:
  griffinwing.flight.Session.tap.fire(s, owner, c, s_c, side, (s_c + griffinwing.types.dm(12) : U32), (ga.u(c, "x") + (side * griffinwing.types.dm(4) : U32) : U32))

def griffinwing.flight.Session.tap.fire(+s: J.Val, +owner: U32, +c: J.Val, +s_c: U32, +side: U32, +fs: U32, +fx: U32) -> J.Val:
  griffinwing.flight.Session.tap.bolt(s, owner, fs, fx, ga.u(c, "z"),
    griffinwing.types.scale3(
      ((s_c + griffinwing.types.FAR() : U32) - fs : U32),
      ((ga.u(c, "x") + ga.u(c, "rx") : U32) - fx : U32),
      ((ga.u(c, "z") + ga.u(c, "rz") : U32) - ga.u(c, "z") : U32),
      griffinwing.flight.V_BOLT()))

def griffinwing.flight.Session.tap.bolt(+s: J.Val, +owner: U32, +fs: U32, +fx: U32, +fz: U32, +v: GwV3) -> J.Val:
  griffinwing.flight.Session.push_bolt(s,
    griffinwing.types.Bolt.make("Lance", owner, fs, fx, fz, (griffinwing.types.PACE() + gw.v3a(v) : U32), gw.v3b(v), gw.v3c(v),
      griffinwing.flight.LANCE(), griffinwing.flight.BOLT_LIFE(), 0, 0, 0, False{}))



# Everything the reticle sweeps over while F is held is locked: a bracket on
# each, snapping on. A great body (a lamp) takes up to three if the reticle
# dwells on it. Returns the workspace {"s","c"}; while it runs the workspace
# also carries "total" (the locks' count) and "dwelt".
def griffinwing.flight.Session.acquire(+s: J.Val, +c0: J.Val, +s_c: U32) -> J.Val:
  +c1 = ga.sv(c0, "locks", J.JA{griffinwing.flight.Session.acquire.keep(ga.items(c0, "locks"), ga.items(s, "foes"), s_c)})
  griffinwing.flight.Session.acquire.done(
    griffinwing.flight.Session.acquire.go(ga.items(s, "foes"),
      J.set(J.set(gw.pk(s, c1), "total", J.JU{griffinwing.types.Craft.lock_total(ga.items(c1, "locks"))}), "dwelt", J.JBool{False{}}), s_c))

def griffinwing.flight.Session.acquire.done(+w: J.Val) -> J.Val:
  gw.pk(gw.ps(w), gw.ifj(ga.b(w, "dwelt"), _ => gw.pc(w), _ => ga.su(gw.pc(w), "dwell", 0)))

def gw.ifj_l(c: Bool, a: Unit -> List<&2, J.Val>, b: Unit -> List<&2, J.Val>) -> List<&2, J.Val>:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

# c.locks.retain(|l| a foe with that id is lockable and within reach)
def griffinwing.flight.Session.acquire.keep(+ls: List<&2, J.Val>, +foes: List<&2, J.Val>, +s_c: U32) -> List<&2, J.Val>:
  match ls:
    case Nil{}:
      Nil{}
    case Con{+l, +t}:
      gw.ifj_l(griffinwing.flight.Session.acquire.held(foes, ga.u(l, "foe"), s_c),
        _ => Con{l, griffinwing.flight.Session.acquire.keep(t, foes, s_c)},
        _ => griffinwing.flight.Session.acquire.keep(t, foes, s_c))

def griffinwing.flight.Session.acquire.held(+foes: List<&2, J.Val>, +id: U32, +s_c: U32) -> Bool:
  match foes:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      Bool.or(
        Bool.and(U32.is_eq(ga.u(f, "id"), id), Bool.and(griffinwing.types.Foe.lockable(f),
          gw.within((ga.u(f, "s") - s_c : U32), griffinwing.types.LOCK_MIN(), (griffinwing.types.LOCK_MAX() + griffinwing.types.dm(40) : U32)))),
        griffinwing.flight.Session.acquire.held(t, id, s_c))

def griffinwing.flight.Session.acquire.go(+foes: List<&2, J.Val>, +w: J.Val, +s_c: U32) -> J.Val:
  match foes:
    case Nil{}:
      w
    case Con{+f, +t}:
      griffinwing.flight.Session.acquire.go(t, griffinwing.flight.Session.acquire.foe(f, w, s_c), s_c)

# One foe under the reticle's ray.
def griffinwing.flight.Session.acquire.foe(+f: J.Val, +w: J.Val, +s_c: U32) -> J.Val:
  griffinwing.flight.Session.acquire.foe2(f, w, (ga.u(f, "s") - s_c : U32))

def griffinwing.flight.Session.acquire.foe2(+f: J.Val, +w: J.Val, +ds: U32) -> J.Val:
  gw.ifj(Bool.or(Bool.not(griffinwing.types.Foe.lockable(f)), Bool.not(gw.within(ds, griffinwing.types.LOCK_MIN(), griffinwing.types.LOCK_MAX()))),
    _ => w,
    _ => griffinwing.flight.Session.acquire.foe3(f, w,
      gw.muldiv(ga.u(gw.pc(w), "rx"), ds, griffinwing.types.FAR()),
      gw.muldiv(ga.u(gw.pc(w), "rz"), ds, griffinwing.types.FAR())))

def griffinwing.flight.Session.acquire.foe3(+f: J.Val, +w: J.Val, +ax: U32, +az: U32) -> J.Val:
  gw.ifj(gw.ngt(
      griffinwing.types.flat((ga.u(f, "x") - ga.u(gw.pc(w), "x") : U32), (ga.u(f, "z") - ga.u(gw.pc(w), "z") : U32), ax, az),
      (griffinwing.types.Foe.radius(f) + griffinwing.flight.LOCK_R() : U32)),
    _ => w,
    _ => griffinwing.flight.Session.acquire.lock(f, w, griffinwing.flight.Session.acquire.find(ga.items(gw.pc(w), "locks"), ga.u(f, "id"), 0)))

# The index of the lock on this foe, or the length.
def griffinwing.flight.Session.acquire.find(+ls: List<&2, J.Val>, +id: U32, +k: U32) -> U32:
  match ls:
    case Nil{}:
      k
    case Con{+l, +t}:
      gw.ifu(U32.is_eq(ga.u(l, "foe"), id), _ => k, _ => griffinwing.flight.Session.acquire.find(t, id, (k + 1 : U32)))

def griffinwing.flight.Session.acquire.lock(+f: J.Val, +w: J.Val, +at: U32) -> J.Val:
  gw.ifj(U32.is_lt(at, gw.len(gw.pc(w), "locks")),
    _ => griffinwing.flight.Session.acquire.dwell(f, w, at),
    _ => griffinwing.flight.Session.acquire.snap(f, w))

# The reticle dwells on a lamp already locked: up to three on one body.
def griffinwing.flight.Session.acquire.dwell(+f: J.Val, +w: J.Val, +at: U32) -> J.Val:
  gw.ifj(Bool.and(gw.isn(f, "kind", "Lamp"),
      Bool.and(U32.is_lt(ga.u(gw.item(gw.pc(w), "locks", at), "n"), griffinwing.types.MAX_PER_BODY()),
        U32.is_lt(ga.u(w, "total"), griffinwing.types.MAX_LOCKS()))),
    _ => griffinwing.flight.Session.acquire.dwell2(J.set(w, "dwelt", J.JBool{True{}}), at, (ga.u(gw.pc(w), "dwell") + 1 : U32)),
    _ => w)
