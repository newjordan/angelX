# crates/delve/src/rules/griffinwing/combat.rs — the fight: the foes' flights
# and tells, the kites that latch on. The bolts and what they hit are in
# combat2.bend. A stage that changes the session and a foe returns the
# workspace {"s": session, "c": foe}.

def griffinwing.combat.HUNT_V() -> U32:
  griffinwing.types.dm(10)

def griffinwing.combat.LATCH_BACK() -> U32:
  griffinwing.types.dm(18)

def griffinwing.combat.GLOAM() -> U32:
  25

def griffinwing.combat.SPARK() -> U32:
  10

def griffinwing.combat.GLOAM_R() -> U32:
  griffinwing.types.dm(6)

def griffinwing.combat.SPARK_R() -> U32:
  griffinwing.types.dm(5)

def griffinwing.combat.BOLT_R() -> U32:
  griffinwing.types.dm(5)

# What a turned bolt does, and its pace home.
def griffinwing.combat.TURNED() -> U32:
  60

def griffinwing.combat.V_TURN() -> U32:
  griffinwing.types.dm(24)

# A dive-bomber's blow.
def griffinwing.combat.RAM() -> U32:
  35

# How long a shield stays down after a hit lands on it, in ticks.
def griffinwing.combat.HURT() -> U32:
  36

# Put a foe on the rail, its health provisioned for the knights at the table;
# what it is worth is offered to the medal. {"s", "r": id or null}.
def griffinwing.combat.Session.spawn_foe(+s: J.Val, +kind: String, +at_s: U32, +at_x: U32, +at_z: U32, +vs: U32, +vx: U32, +vz: U32, +st: String, +mode: U32, +phase: U32) -> J.Val:
  gw.ifj(U32.is_ge(gw.len(s, "foes"), griffinwing.types.MAX_FOES()),
    _ => gw.out(s, J.JNull{}),
    _ => griffinwing.combat.Session.spawn_foe.in(griffinwing.Session.next_id(s), kind, at_s, at_x, at_z, vs, vx, vz, st, mode, phase, griffinwing.Session.pressure(s)))

def griffinwing.combat.Session.spawn_foe.in(+r: J.Val, +kind: String, +at_s: U32, +at_x: U32, +at_z: U32, +vs: U32, +vx: U32, +vz: U32, +st: String, +mode: U32, +phase: U32, +seats: U32) -> J.Val:
  griffinwing.combat.Session.spawn_foe.push(gw.out_s(r), ga.u(gw.out_r(r), "id"), gw.out_r(r), kind, at_s, at_x, at_z, vs, vx, vz, st, mode, phase, seats)

def griffinwing.combat.Session.spawn_foe.push(+s: J.Val, +id: U32, +idj: J.Val, +kind: String, +at_s: U32, +at_x: U32, +at_z: U32, +vs: U32, +vx: U32, +vz: U32, +st: String, +mode: U32, +phase: U32, +seats: U32) -> J.Val:
  gw.out(
    ga.sv(
      ga.push(s, "foes", griffinwing.combat.Foe.make(id, kind, at_s, at_x, at_z, vs, vx, vz, (griffinwing.types.Kind.health(kind) * seats : U32), seats, st, mode, phase)),
      "beat", gw.addu(J.get(s, "beat"), "offered", griffinwing.types.Kind.worth(kind))),
    idj)

def griffinwing.combat.Foe.make(+id: U32, +kind: String, +s: U32, +x: U32, +z: U32, +vs: U32, +vx: U32, +vz: U32, +full: U32, +seats: U32, +st: String, +mode: U32, +phase: U32) -> J.Val:
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
def griffinwing.combat.Session.hurt_craft(+s: J.Val, +id: U32, +dmg: U32) -> J.Val:
  gw.ifj(gw.within(id, 1, 6),
    _ => griffinwing.combat.Session.hurt_craft.seat(s, id, dmg, griffinwing.flight.Session.craft_at(s, id), J.get(J.get(s, "seats"), U32.show(id))),
    _ => griffinwing.combat.Session.hurt_craft.rider(s, dmg, griffinwing.flight.Session.wing_find(ga.items(s, "wing"), id)))

def griffinwing.combat.Session.hurt_craft.seat(+s: J.Val, +id: U32, +dmg: U32, +at: J.Val, +seat: J.Val) -> J.Val:
  gw.ifj(J.is_null(seat),
    _ => gw.out(s, J.JBool{False{}}),
    _ => griffinwing.combat.Session.hurt_craft.seat2(s, id, dmg, at, J.get(seat, "craft")))

def griffinwing.combat.Session.hurt_craft.seat2(+s: J.Val, +id: U32, +dmg: U32, +at: J.Val, +c: J.Val) -> J.Val:
  gw.ifj(Bool.or(U32.is_gt(ga.u(c, "hurt"), 0), U32.is_gt(ga.u(c, "down"), 0)),
    _ => gw.out(s, J.JBool{False{}}),
    _ => griffinwing.combat.Session.hurt_craft.hit(
      gw.put_craft(s, id, ga.su(ga.si(c, "shield", gw.max((ga.u(c, "shield") - dmg : U32), 0)), "hurt", griffinwing.combat.HURT())),
      id, dmg, at))

def griffinwing.combat.Session.hurt_craft.hit(+s: J.Val, +id: U32, +dmg: U32, +at: J.Val) -> J.Val:
  griffinwing.combat.Session.hurt_craft.low(
    griffinwing.Session.sound(
      gw.ifj(gw.at_some(at), _ => griffinwing.Session.spark(s, "Hurt", gw.at_s(at), gw.at_x(at), gw.at_z(at), id), _ => s),
      gw.ifs(U32.is_ge(dmg, 20), _ => "knight_hit_big", _ => "hero_hurt")),
    id)

def gw.ifs(c: Bool, a: Unit -> String, b: Unit -> String) -> String:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def griffinwing.combat.Session.hurt_craft.low(+s: J.Val, +id: U32) -> J.Val:
  gw.out(
    gw.ifj(gw.le(ga.u(gw.craft_of(s, id), "shield"), 30), _ => griffinwing.Session.say(s, id, "Low"), _ => s),
    J.JBool{True{}})

def griffinwing.combat.Session.hurt_craft.rider(+s: J.Val, +dmg: U32, +w: J.Val) -> J.Val:
  gw.ifj(J.is_null(w),
    _ => gw.out(s, J.JBool{False{}}),
    _ => griffinwing.combat.Session.hurt_craft.rider2(s, dmg, w))

def griffinwing.combat.Session.hurt_craft.rider2(+s: J.Val, +dmg: U32, +w: J.Val) -> J.Val:
  gw.ifj(ga.b(w, "lost"),
    _ => gw.out(s, J.JBool{False{}}),
    _ => gw.out(
      griffinwing.combat.Session.wing_put(s, ga.u(w, "id"),
        ga.sv(w, "craft", ga.si(J.get(w, "craft"), "shield", gw.max((ga.u(J.get(w, "craft"), "shield") - dmg : U32), 0)))),
      J.JBool{True{}}))

# The rider by this id put back.
def griffinwing.combat.Session.wing_put(+s: J.Val, +id: U32, +w: J.Val) -> J.Val:
  gw.put_item(s, "wing", griffinwing.combat.Session.wing_at(ga.items(s, "wing"), id, 0), w)

def griffinwing.combat.Session.wing_at(+ws: List<&2, J.Val>, +id: U32, +k: U32) -> U32:
  match ws:
    case Nil{}:
      k
    case Con{+w, +t}:
      gw.ifu(U32.is_eq(ga.u(w, "id"), id), _ => k, _ => griffinwing.combat.Session.wing_at(t, id, (k + 1 : U32)))

# A drip from a latched kite: no spare moment, no flash.
def griffinwing.combat.Session.drain(+s: J.Val, +id: U32, +amount: U32) -> J.Val:
  gw.ifj(gw.within(id, 1, 6),
    _ => griffinwing.combat.Session.drain.seat(s, id, amount, J.get(J.get(s, "seats"), U32.show(id))),
    _ => griffinwing.combat.Session.drain.rider(s, amount, griffinwing.flight.Session.wing_find(ga.items(s, "wing"), id)))

def griffinwing.combat.Session.drain.seat(+s: J.Val, +id: U32, +amount: U32, +seat: J.Val) -> J.Val:
  gw.ifj(Bool.or(J.is_null(seat), U32.is_gt(ga.u(J.get(seat, "craft"), "down"), 0)),
    _ => s,
    _ => gw.put_craft(s, id, ga.si(J.get(seat, "craft"), "shield", gw.max((ga.u(J.get(seat, "craft"), "shield") - amount : U32), 0))))

def griffinwing.combat.Session.drain.rider(+s: J.Val, +amount: U32, +w: J.Val) -> J.Val:
  gw.ifj(Bool.or(J.is_null(w), ga.b(w, "lost")),
    _ => s,
    _ => griffinwing.combat.Session.wing_put(s, ga.u(w, "id"),
      ga.sv(w, "craft", ga.si(J.get(w, "craft"), "shield", gw.max((ga.u(J.get(w, "craft"), "shield") - amount : U32), 0)))))

# Whether some kite hangs on this craft.
def griffinwing.combat.Session.latched(+s: J.Val, +id: U32) -> Bool:
  griffinwing.combat.Session.latched.go(ga.items(s, "foes"), id)

def griffinwing.combat.Session.latched.go(+fs: List<&2, J.Val>, +id: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      Bool.or(Bool.and(gw.isn(f, "st", "Latched"), U32.is_eq(ga.u(f, "on"), id)), griffinwing.combat.Session.latched.go(t, id))

# Whether this craft is flying and no kite hangs on it.
def griffinwing.combat.Session.victim.free(+s: J.Val, +id: U32) -> Bool:
  Bool.and(
    Bool.and(gw.at_some(griffinwing.flight.Session.craft_at(s, id)), gw.at_ok(griffinwing.flight.Session.craft_at(s, id))),
    Bool.not(griffinwing.combat.Session.latched(s, id)))

# The ids of the free ones among the riders / the knights, in order.
def griffinwing.combat.Session.victim.riders(+s: J.Val, +ws: List<&2, J.Val>) -> List<&2, U32>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      gw.ifl(griffinwing.combat.Session.victim.free(s, ga.u(w, "id")),
        _ => Con{ga.u(w, "id"), griffinwing.combat.Session.victim.riders(s, t)},
        _ => griffinwing.combat.Session.victim.riders(s, t))

def griffinwing.combat.Session.victim.knights(+s: J.Val, +ids: List<&2, U32>) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gw.ifl(griffinwing.combat.Session.victim.free(s, id),
        _ => Con{id, griffinwing.combat.Session.victim.knights(s, t)},
        _ => griffinwing.combat.Session.victim.knights(s, t))

def gw.ifl(c: Bool, a: Unit -> List<&2, U32>, b: Unit -> List<&2, U32>) -> List<&2, U32>:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

# The nth item of a list of ids.
def gw.nth_id(+xs: List<&2, U32>, +n: U32) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, +t}:
      gw.ifu(U32.is_zero(n), _ => h, _ => gw.nth_id(t, (n - 1 : U32)))

def gw.len_id(+xs: List<&2, U32>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, +t}:
      (1 + gw.len_id(t) : U32)

# Who a hunting kite goes for: a rider not yet latched, else a knight not yet
# latched, else anyone flying. The same hunter always finds the same victim.
# Answers an id, or null.
def griffinwing.combat.Session.victim(+s: J.Val, +hunter: U32) -> J.Val:
  griffinwing.combat.Session.victim.pick(s, hunter,
    griffinwing.combat.Session.victim.riders(s, ga.items(s, "wing")))

def griffinwing.combat.Session.victim.pick(+s: J.Val, +hunter: U32, +riders: List<&2, U32>) -> J.Val:
  gw.ifj(U32.is_zero(gw.len_id(riders)),
    _ => griffinwing.combat.Session.victim.seats(s, hunter, griffinwing.combat.Session.victim.knights(s, ga.ids(J.get(s, "seats")))),
    _ => J.JU{gw.nth_id(riders, U32.mod(hunter, gw.len_id(riders)))})

def griffinwing.combat.Session.victim.seats(+s: J.Val, +hunter: U32, +knights: List<&2, U32>) -> J.Val:
  gw.ifj(U32.is_zero(gw.len_id(knights)),
    _ => griffinwing.combat.Session.victim.any(s, ga.ids(J.get(s, "seats"))),
    _ => J.JU{gw.nth_id(knights, U32.mod(hunter, gw.len_id(knights)))})

def griffinwing.combat.Session.victim.any(+s: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      J.JNull{}
    case Con{+id, +t}:
      gw.ifj(Bool.and(gw.at_some(griffinwing.flight.Session.craft_at(s, id)), gw.at_ok(griffinwing.flight.Session.craft_at(s, id))),
        _ => J.JU{id},
        _ => griffinwing.combat.Session.victim.any(s, t))

# The (x, z) of the knights flying, in seat order.
def griffinwing.combat.Session.flying(+s: J.Val) -> List<&2, J.Val>:
  griffinwing.combat.Session.flying.go(ga.ids(J.get(s, "seats")), s)

def griffinwing.combat.Session.flying.go(+ids: List<&2, U32>, +s: J.Val) -> List<&2, J.Val>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gw.ifjl(griffinwing.types.Craft.flying(gw.craft_of(s, id)),
        _ => Con{ga.pair_i(ga.u(gw.craft_of(s, id), "x"), ga.u(gw.craft_of(s, id), "z")), griffinwing.combat.Session.flying.go(t, s)},
        _ => griffinwing.combat.Session.flying.go(t, s))

def gw.ifjl(c: Bool, a: Unit -> List<&2, J.Val>, b: Unit -> List<&2, J.Val>) -> List<&2, J.Val>:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def gw.nth_j(+xs: List<&2, J.Val>, +n: U32) -> J.Val:
  match xs:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      gw.ifj(U32.is_zero(n), _ => h, _ => gw.nth_j(t, (n - 1 : U32)))

# Who a turret or the Wyrm fires at: the knights flying, in turn. [x, z] or null.
def griffinwing.combat.Session.target(+s: J.Val, +turn: U32) -> J.Val:
  griffinwing.combat.Session.target.of(griffinwing.combat.Session.flying(s), turn)

def griffinwing.combat.Session.target.of(+fl: List<&2, J.Val>, +turn: U32) -> J.Val:
  match fl:
    case Nil{}:
      J.JNull{}
    case _:
      gw.nth_j(fl, U32.mod(turn, U32.from_nat(List.length(&2, J.Val, fl))))

# A bolt from `o` that meets the formation's plane at `to` (across, up) as the
# rail brings the plane to it, closing at `closing` a tick.
def griffinwing.combat.Session.plane_shot(+s: J.Val, +os: U32, +ox: U32, +oz: U32, +tx: U32, +tz: U32, +closing: U32) -> GwV3:
  griffinwing.combat.Session.plane_shot.go(ox, oz, tx, tz, closing,
    gw.max(gw.div(gw.max((os - griffinwing.Session.anchor(s) : U32), griffinwing.types.dm(40)), gw.max(closing, 1)), 1))

def griffinwing.combat.Session.plane_shot.go(+ox: U32, +oz: U32, +tx: U32, +tz: U32, +closing: U32, +ticks: U32) -> GwV3:
  GwV3{(griffinwing.types.PACE() - closing : U32), gw.div((tx - ox : U32), ticks), gw.div((tz - oz : U32), ticks)}

# ─── the foes' tick ─────────────────────────────────────────────────────────

# The foes' tick: flights, tells, latching, dives. The foes are taken out of
# the session while each is stepped (so a hunter finds no latched kite), then
# put back; those gone by or fled out of sight go; the bombers meet craft.
def griffinwing.combat.Session.step_foes(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.combat.Session.step_foes.after(
    griffinwing.combat.Session.step_foes.each(ga.sv(s, "foes", J.JA{[]}), ga.items(s, "foes"), griffinwing.Session.anchor(s), n),
    griffinwing.Session.anchor(s))

# w = {"s", "c": the foes stepped so far}
def griffinwing.combat.Session.step_foes.each(+s: J.Val, +fs: List<&2, J.Val>, +anchor: U32, +n: J.Val) -> J.Val:
  griffinwing.combat.Session.step_foes.walk(fs, gw.pk(s, J.JA{[]}), anchor, n)

def griffinwing.combat.Session.step_foes.walk(+fs: List<&2, J.Val>, +w: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  match fs:
    case Nil{}:
      w
    case Con{+f, +t}:
      griffinwing.combat.Session.step_foes.walk(t,
        griffinwing.combat.Session.step_foes.one(griffinwing.combat.Session.step_foe(gw.ps(w), f, anchor, n), gw.pc(w)),
        anchor, n)

def griffinwing.combat.Session.step_foes.one(+r: J.Val, +done: J.Val) -> J.Val:
  gw.pk(gw.ps(r), J.push(done, gw.pc(r)))

def griffinwing.combat.Session.step_foes.after(+w: J.Val, +anchor: U32) -> J.Val:
  griffinwing.combat.Session.bombers_ram(
    ga.sv(gw.ps(w), "foes", J.JA{griffinwing.combat.Session.step_foes.keep(J.items(gw.pc(w)), anchor)}))

def griffinwing.combat.Session.step_foes.keep(+fs: List<&2, J.Val>, +anchor: U32) -> List<&2, J.Val>:
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
def gw.fmove(+f: J.Val) -> J.Val:
  ga.si(ga.si(ga.si(f, "s", (ga.u(f, "s") + ga.u(f, "vs") : U32)), "x", (ga.u(f, "x") + ga.u(f, "vx") : U32)), "z", (ga.u(f, "z") + ga.u(f, "vz") : U32))

# The pair (kind, state) as the match arm that handles it, in the order the
# Rust match tries them: 0 Wisp|Kite Fly, 1 Wisp Tell, 2 Kite Hunt, 3 Kite
# Latched, 4 any Flee, 5 Turret Rest, 6 Turret Tell, 7 Bomber Hide, 8 Bomber
# Tell, 9 Bomber Dive, 10 none.
def griffinwing.combat.Session.step_foe.arm(+f: J.Val) -> U32:
  griffinwing.combat.Session.step_foe.arm2(ga.s(f, "kind"), ga.s(f, "st"))

def griffinwing.combat.Session.step_foe.arm2(+k: String, +st: String) -> U32:
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
def griffinwing.combat.Session.step_foe(+s: J.Val, +f: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  gw.ifj(Bool.and(U32.is_ge(ga.u(f, "flash"), 4), Bool.or(gw.isn(f, "st", "Fly"), gw.isn(f, "st", "Hunt"))),
    _ => gw.pk(s, f),
    _ => griffinwing.combat.Session.step_foe.arm_do(s, f, anchor, n, griffinwing.combat.Session.step_foe.arm(f)))

def griffinwing.combat.Session.step_foe.arm_do(+s: J.Val, +f: J.Val, +anchor: U32, +n: J.Val, +arm: U32) -> J.Val:
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
def griffinwing.combat.Session.step_foe.fly(+s: J.Val, +f0: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  +f1 = gw.ifj(Bool.and(U32.is_eq(ga.u(f0, "mode"), 1), U32.is_zero(U32.mod((ga.u(s, "t") + ga.u(f0, "phase") : U32), 34))),
    _ => ga.si(f0, "vx", gw.neg(ga.u(f0, "vx"))),
    _ => f0)
  gw.ifj(Bool.and(U32.is_eq(ga.u(f1, "mode"), 3), gw.isn(f1, "kind", "Wisp")),
    _ => griffinwing.combat.Session.step_foe.shoot(s, gw.subs(f1, "ticks", 1), anchor, n),
    _ => gw.pk(s, f1))

# A shooting wisp counts down to its tell. `f` has had its tick taken off.
def griffinwing.combat.Session.step_foe.shoot(+s: J.Val, +f: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  gw.ifj(Bool.and(U32.is_zero(ga.u(f, "ticks")), gw.within((ga.u(f, "s") - anchor : U32), griffinwing.types.dm(200), griffinwing.types.dm(460))),
    _ => griffinwing.combat.Session.step_foe.aim(s, f, griffinwing.combat.Session.target(s, ga.u(f, "id")), n),
    _ => gw.pk(s, f))

# A tell on the footprint `to` ([x, z] or null): the foe locks it.
def griffinwing.combat.Session.step_foe.aim(+s: J.Val, +f: J.Val, +to: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(J.is_null(to),
    _ => gw.pk(s, f),
    _ => gw.pk(s, ga.su(gw.sets(ga.si(ga.si(f, "fx", ga.nu(to, 0)), "fz", ga.nu(to, 1)), "st", "Tell"), "ticks", ga.u(n, "tell"))))

# (Wisp, Tell), after the move: the spark leaves when the tell runs out.
def griffinwing.combat.Session.step_foe.wisp_tell(+s: J.Val, +f0: J.Val) -> J.Val:
  griffinwing.combat.Session.step_foe.wisp_tell2(s, gw.subs(f0, "ticks", 1))

def griffinwing.combat.Session.step_foe.wisp_tell2(+s: J.Val, +f: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(f, "ticks")),
    _ => griffinwing.combat.Session.step_foe.spark_shot(s, f,
      griffinwing.combat.Session.plane_shot(s, ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), ga.u(f, "fx"), ga.u(f, "fz"), griffinwing.types.dm(4))),
    _ => gw.pk(s, f))

def griffinwing.combat.Session.step_foe.spark_shot(+s: J.Val, +f: J.Val, +v: GwV3) -> J.Val:
  gw.pk(
    griffinwing.flight.Session.push_bolt(s,
      griffinwing.types.Bolt.make("Spark", 0, ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), gw.v3a(v), gw.v3b(v), gw.v3c(v),
        griffinwing.combat.SPARK(), 220, 0, ga.u(f, "id"), 0, False{})),
    ga.su(gw.sets(f, "st", "Fly"), "ticks", 100))

# (_, Flee), after the move: gone when the ticks run out.
def griffinwing.combat.Session.step_foe.flee(+f0: J.Val) -> J.Val:
  griffinwing.combat.Session.step_foe.flee2(gw.subs(f0, "ticks", 1))

def griffinwing.combat.Session.step_foe.flee2(+f: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(f, "ticks")), _ => ga.su(f, "hp", 0), _ => f)

# (Turret, Rest), its tick taken off: a tower waiting for something in reach.
def griffinwing.combat.Session.step_foe.turret_rest(+s: J.Val, +f: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  gw.ifj(Bool.and(U32.is_zero(ga.u(f, "ticks")), gw.within((ga.u(f, "s") - anchor : U32), griffinwing.types.dm(150), griffinwing.types.dm(560))),
    _ => griffinwing.combat.Session.step_foe.turret_aim(s, f, griffinwing.combat.Session.target(s, (ga.u(f, "id") + ga.u(f, "phase") : U32)), n),
    _ => gw.pk(s, f))

def griffinwing.combat.Session.step_foe.turret_aim(+s: J.Val, +f: J.Val, +to: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(J.is_null(to),
    _ => gw.pk(s, f),
    _ => gw.pk(griffinwing.Session.sound(s, "vent_fire"),
      ga.su(gw.sets(ga.si(ga.si(f, "fx", ga.nu(to, 0)), "fz", ga.nu(to, 1)), "st", "Tell"), "ticks", ga.u(n, "tell"))))

# (Turret, Tell), its tick taken off: the shot goes.
def griffinwing.combat.Session.step_foe.turret_tell(+s: J.Val, +f: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(f, "ticks")),
    _ => gw.pk(griffinwing.combat.Session.fire_marks(s, f),
      ga.su(gw.sets(ga.su(f, "phase", U32.and((ga.u(f, "phase") + 1 : U32), 255)), "st", "Rest"), "ticks", 26)),
    _ => gw.pk(s, f))

# (Bomber, Hide): behind its ridge until the formation is near.
def griffinwing.combat.Session.step_foe.bomber_hide(+s: J.Val, +f: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  gw.ifj(gw.le((ga.u(f, "s") - anchor : U32), griffinwing.types.dm(330)),
    _ => griffinwing.combat.Session.step_foe.aim(s, f, griffinwing.combat.Session.target(s, ga.u(f, "id")), n),
    _ => gw.pk(s, f))

# (Bomber, Tell): it rises over the ridge, then dives down the locked lane.
def griffinwing.combat.Session.step_foe.bomber_tell(+s: J.Val, +f0: J.Val, +anchor: U32, +n: J.Val) -> J.Val:
  griffinwing.combat.Session.step_foe.bomber_tell2(s,
    gw.subs(
      gw.ifj(gw.lt(ga.u(f0, "z"), (gw.max(ga.u(f0, "fz"), griffinwing.types.dm(10)) + griffinwing.types.dm(24) : U32)),
        _ => ga.si(f0, "z", (ga.u(f0, "z") + griffinwing.types.dm(1) : U32)),
        _ => f0),
      "ticks", 1),
    anchor)

def griffinwing.combat.Session.step_foe.bomber_tell2(+s: J.Val, +f: J.Val, +anchor: U32) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(f, "ticks")),
    _ => gw.pk(s, griffinwing.combat.Session.step_foe.dive(f, gw.div(gw.max((ga.u(f, "s") - anchor : U32), griffinwing.types.dm(60)), 32))),
    _ => gw.pk(s, f))

# The dive down the locked lane: 32 ticks to it.
def griffinwing.combat.Session.step_foe.dive(+f: J.Val, +closing: U32) -> J.Val:
  +f1 = ga.si(f, "vs", (griffinwing.types.PACE() - closing : U32))
  +f2 = ga.si(f1, "vx", gw.div((ga.u(f1, "fx") - ga.u(f1, "x") : U32), 32))
  +f3 = ga.si(f2, "vz", gw.div((ga.u(f2, "fz") - ga.u(f2, "z") : U32), 32))
  gw.sets(f3, "st", "Dive")

# The marks a turret's shot will land on, across and up: where it locked, and a
# pair more to each side for every three knights beyond the first. A J array of
# [x, z].
def griffinwing.combat.Session.marks(+f: J.Val, +seats: U32) -> J.Val:
  J.JA{
    Con{ga.pair_i((ga.u(f, "fx") - griffinwing.types.dm(8) : U32), ga.u(f, "fz")),
    Con{ga.pair_i((ga.u(f, "fx") + griffinwing.types.dm(8) : U32), ga.u(f, "fz")),
      griffinwing.combat.Session.marks.extra(f, 1, U32.div(ga.ssub(seats, 1), 3))}}}

def griffinwing.combat.Session.marks.extra(+f: J.Val, +k: U32, +extra: U32) -> List<&2, J.Val>:
  gw.ifjl(U32.is_gt(k, extra),
    _ => Nil{},
    _ => Con{ga.pair_i(((ga.u(f, "fx") - griffinwing.types.dm(8) : U32) - (k * griffinwing.types.dm(20) : U32) : U32), ga.u(f, "fz")),
      Con{ga.pair_i(((ga.u(f, "fx") + griffinwing.types.dm(8) : U32) + (k * griffinwing.types.dm(20) : U32) : U32), ga.u(f, "fz")),
        griffinwing.combat.Session.marks.extra(f, (k + 1 : U32), extra)}})

# A turret fires on its locked marks.
def griffinwing.combat.Session.fire_marks(+s: J.Val, +f: J.Val) -> J.Val:
  griffinwing.Session.sound(
    griffinwing.combat.Session.fire_marks.go(s, f, J.items(griffinwing.combat.Session.marks(f, griffinwing.Session.pressure(s)))),
    "shot_crossbow")

def griffinwing.combat.Session.fire_marks.go(+s: J.Val, +f: J.Val, +ms: List<&2, J.Val>) -> J.Val:
  match ms:
    case Nil{}:
      s
    case Con{+m, +t}:
      griffinwing.combat.Session.fire_marks.go(griffinwing.combat.Session.fire_marks.one(s, f, ga.nu(m, 0), ga.nu(m, 1)), f, t)

def griffinwing.combat.Session.fire_marks.one(+s: J.Val, +f: J.Val, +mx: U32, +mz: U32) -> J.Val:
  griffinwing.combat.Session.fire_marks.shot(s, f,
    griffinwing.combat.Session.plane_shot(s, ga.u(f, "s"), ga.u(f, "x"), (ga.u(f, "z") + griffinwing.types.dm(12) : U32), mx, mz, griffinwing.types.dm(5)))

def griffinwing.combat.Session.fire_marks.shot(+s: J.Val, +f: J.Val, +v: GwV3) -> J.Val:
  griffinwing.flight.Session.push_bolt(s,
    griffinwing.types.Bolt.make("Gloam", 0, ga.u(f, "s"), ga.u(f, "x"), (ga.u(f, "z") + griffinwing.types.dm(12) : U32),
      gw.v3a(v), gw.v3b(v), gw.v3c(v), griffinwing.combat.GLOAM(), 260, 0, ga.u(f, "id"), 0, False{}))

# A kite going for its victim: it picks one, closes, and latches onto the tail.
def griffinwing.combat.Session.hunt(+s: J.Val, +f: J.Val) -> J.Val:
  gw.ifj(Bool.and(U32.is_ne(ga.u(f, "on"), 0), gw.at_ok_some(griffinwing.flight.Session.craft_at(s, ga.u(f, "on")))),
    _ => griffinwing.combat.Session.hunt.close(s, f),
    _ => griffinwing.combat.Session.hunt.pick(s, f, griffinwing.combat.Session.victim(s, ga.u(f, "id"))))

# `craft_at(id).is_some_and(|c| c.3)`.
def gw.at_ok_some(+r: J.Val) -> Bool:
  Bool.and(gw.at_some(r), gw.at_ok(r))

def griffinwing.combat.Session.hunt.pick(+s: J.Val, +f: J.Val, +v: J.Val) -> J.Val:
  gw.ifj(J.is_null(v),
    _ => gw.pk(s, ga.si(ga.su(gw.sets(f, "st", "Flee"), "ticks", 40), "vz", 60)),
    _ => griffinwing.combat.Session.hunt.close(s, ga.su(f, "on", J.u32(v))))

def griffinwing.combat.Session.hunt.close(+s: J.Val, +f: J.Val) -> J.Val:
  griffinwing.combat.Session.hunt.at(s, f, griffinwing.flight.Session.craft_at(s, ga.u(f, "on")))

def griffinwing.combat.Session.hunt.at(+s: J.Val, +f: J.Val, +at: J.Val) -> J.Val:
  gw.ifj(Bool.not(gw.at_some(at)),
    _ => gw.pk(s, f),
    _ => griffinwing.combat.Session.hunt.go(s, f,
      (gw.at_s(at) - griffinwing.combat.LATCH_BACK() : U32), gw.at_x(at), (gw.at_z(at) + griffinwing.types.dm(4) : U32)))

def griffinwing.combat.Session.hunt.go(+s: J.Val, +f: J.Val, +ts: U32, +tx: U32, +tz: U32) -> J.Val:
  griffinwing.combat.Session.hunt.test(s, f, ts, tx, tz, (ts - ga.u(f, "s") : U32), (tx - ga.u(f, "x") : U32), (tz - ga.u(f, "z") : U32))

def griffinwing.combat.Session.hunt.test(+s: J.Val, +f: J.Val, +ts: U32, +tx: U32, +tz: U32, +dx: U32, +dy: U32, +dz: U32) -> J.Val:
  gw.ifj(gw.nle(griffinwing.types.norm(dx, dy, dz), griffinwing.types.dm(9)),
    _ => gw.pk(
      griffinwing.Session.sound(griffinwing.Session.say(s, ga.u(f, "on"), "Latched"), "clang"),
      ga.si(ga.si(ga.si(ga.su(gw.sets(f, "st", "Latched"), "ticks", 0), "s", ts), "x", tx), "z", tz)),
    _ => gw.pk(s, griffinwing.combat.Session.hunt.lunge(f, griffinwing.types.scale3(dx, dy, dz, griffinwing.combat.HUNT_V()))))

def griffinwing.combat.Session.hunt.lunge(+f: J.Val, +v: GwV3) -> J.Val:
  +f1 = ga.si(ga.si(ga.si(f, "vs", gw.v3a(v)), "vx", gw.v3b(v)), "vz", gw.v3c(v))
  ga.si(ga.si(ga.si(f1, "s", (ga.u(f, "s") + gw.v3a(v) : U32)), "x", (ga.u(f, "x") + gw.v3b(v) : U32)), "z", (ga.u(f, "z") + gw.v3c(v) : U32))

# A latched kite hangs on its victim's tail, draining its shield, until it is
# shot, or shaken off by a roll.
def griffinwing.combat.Session.hang(+s: J.Val, +f: J.Val) -> J.Val:
  griffinwing.combat.Session.hang.at(s, f, griffinwing.flight.Session.craft_at(s, ga.u(f, "on")))

def griffinwing.combat.Session.hang.at(+s: J.Val, +f: J.Val, +at: J.Val) -> J.Val:
  gw.ifj(Bool.not(gw.at_some(at)),
    _ => gw.pk(s, ga.su(gw.sets(f, "st", "Flee"), "ticks", 40)),
    _ => griffinwing.combat.Session.hang.roll(s, f, at,
      Bool.and(gw.within(ga.u(f, "on"), 1, 6),
        Bool.and(ga.has_key(J.get(s, "seats"), ga.u(f, "on")), U32.is_gt(ga.u(gw.craft_of(s, ga.u(f, "on")), "roll"), 0)))))

def griffinwing.combat.Session.hang.roll(+s: J.Val, +f: J.Val, +at: J.Val, +rolling: Bool) -> J.Val:
  gw.ifj(Bool.or(Bool.not(gw.at_ok(at)), rolling),
    _ => gw.pk(s, ga.si(ga.su(ga.si(ga.si(gw.sets(ga.su(f, "ticks", 40), "st", "Flee"), "vs", 0), "vx",
        gw.ifu(gw.ge(gw.at_x(at), 0), _ => 90, _ => gw.neg(90))), "vz", 70), "ticks", 40)),
    _ => griffinwing.combat.Session.hang.drip(s,
      ga.su(ga.si(ga.si(ga.si(f, "s", (gw.at_s(at) - griffinwing.combat.LATCH_BACK() : U32)), "x", gw.at_x(at)), "z", (gw.at_z(at) + griffinwing.types.dm(4) : U32)), "ticks",
        U32.mod((ga.u(f, "ticks") + 1 : U32), 9996))))

def griffinwing.combat.Session.hang.drip(+s: J.Val, +f: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(U32.mod(ga.u(f, "ticks"), 12)),
    _ => gw.pk(griffinwing.combat.Session.drain(s, ga.u(f, "on"), gw.ifu(U32.is_le(ga.u(f, "on"), 6), _ => 2, _ => 3)), f),
    _ => gw.pk(s, f))

# Dive-bombers meet craft: a blow for the knight, and the bomber is spent.
def griffinwing.combat.Session.bombers_ram(+s: J.Val) -> J.Val:
  griffinwing.combat.Session.bombers_ram.spend(s,
    griffinwing.combat.Session.bombers_ram.scan(ga.items(s, "foes"), s, griffinwing.combat.Session.bombers_ram.crafts(s, ga.ids(J.get(s, "seats")))))

# (id, craft_s, x, z) of the knights flying.
def griffinwing.combat.Session.bombers_ram.crafts(+s: J.Val, +ids: List<&2, U32>) -> List<&2, J.Val>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gw.ifjl(griffinwing.types.Craft.flying(gw.craft_of(s, id)),
        _ => Con{J.JA{[J.JU{id}, J.of_i32(griffinwing.flight.Session.craft_s(s, gw.craft_of(s, id))), J.of_i32(ga.u(gw.craft_of(s, id), "x")), J.of_i32(ga.u(gw.craft_of(s, id), "z"))]},
          griffinwing.combat.Session.bombers_ram.crafts(s, t)},
        _ => griffinwing.combat.Session.bombers_ram.crafts(s, t))

# The (foe id, craft id) pairs a diving bomber's swept path meets, one craft each.
def griffinwing.combat.Session.bombers_ram.scan(+fs: List<&2, J.Val>, +s: J.Val, +crafts: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      gw.ifjl(Bool.and(gw.isn(f, "kind", "Bomber"), gw.isn(f, "st", "Dive")),
        _ => griffinwing.combat.Session.bombers_ram.hit(f, griffinwing.combat.Session.bombers_ram.meet(f, crafts), griffinwing.combat.Session.bombers_ram.scan(t, s, crafts)),
        _ => griffinwing.combat.Session.bombers_ram.scan(t, s, crafts))

def griffinwing.combat.Session.bombers_ram.hit(+f: J.Val, +cid: J.Val, +rest: List<&2, J.Val>) -> List<&2, J.Val>:
  gw.ifjl(J.is_null(cid),
    _ => rest,
    _ => Con{J.JA{[J.JU{ga.u(f, "id")}, cid]}, rest})

# The first craft its path meets: its id, or null.
def griffinwing.combat.Session.bombers_ram.meet(+f: J.Val, +crafts: List<&2, J.Val>) -> J.Val:
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

def griffinwing.combat.Session.bombers_ram.spend(+s: J.Val, +pairs: List<&2, J.Val>) -> J.Val:
  match pairs:
    case Nil{}:
      s
    case Con{+p, +t}:
      griffinwing.combat.Session.bombers_ram.spend(
        griffinwing.combat.Session.bombers_ram.blow(s, ga.nu(p, 0), griffinwing.combat.Session.hurt_craft(s, ga.nu(p, 1), griffinwing.combat.RAM())),
        t)

# The blow landed: the bomber is spent, and pops. (The session is the one the
# blow was struck in.)
def griffinwing.combat.Session.bombers_ram.blow(+s0: J.Val, +fid: U32, +r: J.Val) -> J.Val:
  gw.ifj(J.bool(gw.out_r(r)),
    _ => griffinwing.combat.Session.bombers_ram.pop(gw.out_s(r), fid, griffinwing.combat.Session.bombers_ram.at(ga.items(gw.out_s(r), "foes"), fid, 0)),
    _ => gw.out_s(r))

# The index of the foe with this id, or the length.
def griffinwing.combat.Session.bombers_ram.at(+fs: List<&2, J.Val>, +id: U32, +k: U32) -> U32:
  match fs:
    case Nil{}:
      k
    case Con{+f, +t}:
      gw.ifu(U32.is_eq(ga.u(f, "id"), id), _ => k, _ => griffinwing.combat.Session.bombers_ram.at(t, id, (k + 1 : U32)))

def griffinwing.combat.Session.bombers_ram.pop(+s: J.Val, +fid: U32, +i: U32) -> J.Val:
  gw.ifj(U32.is_lt(i, gw.len(s, "foes")),
    _ => griffinwing.Session.sound(
      griffinwing.Session.spark(gw.remove(s, "foes", i), "Pop", ga.u(gw.item(s, "foes", i), "s"), ga.u(gw.item(s, "foes", i), "x"), ga.u(gw.item(s, "foes", i), "z"), 0),
      "bomb_toss"),
    _ => s)
