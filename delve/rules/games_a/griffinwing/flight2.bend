# crates/delve/src/rules/griffinwing/flight.rs, continued: the lock's dwell and
# snap, the release, the wingmen, and the fall.

def griffinwing.flight.Session.acquire.dwell2(+w: J.Val, +at: U32, +dwell: U32) -> J.Val:
  gw.ifj(U32.is_ge(dwell, 8),
    _ => gw.bump_total(
      J.set(
        J.set(w, "s", griffinwing.Session.sound(gw.ps(w), "spoil_pickup")),
        "c", gw.put_item(ga.su(gw.pc(w), "dwell", 0), "locks", at,
          ga.su(gw.item(gw.pc(w), "locks", at), "n", (ga.u(gw.item(gw.pc(w), "locks", at), "n") + 1 : U32))))),
    _ => J.set(w, "c", ga.su(gw.pc(w), "dwell", dwell)))

def gw.bump_total(+w: J.Val) -> J.Val:
  ga.su(w, "total", (ga.u(w, "total") + 1 : U32))

# A foe not yet locked: a bracket snaps on.
def griffinwing.flight.Session.acquire.snap(+f: J.Val, +w: J.Val) -> J.Val:
  gw.ifj(U32.is_lt(ga.u(w, "total"), griffinwing.types.MAX_LOCKS()),
    _ => griffinwing.flight.Session.acquire.snap2(f, gw.bump_total(w)),
    _ => w)

def griffinwing.flight.Session.acquire.snap2(+f: J.Val, +w: J.Val) -> J.Val:
  J.set(
    J.set(w, "s", griffinwing.Session.spark(
      griffinwing.Session.sound(gw.ps(w), "spoil_pickup"),
      "Lock", ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), ga.u(w, "total"))),
    "c", ga.su(ga.push(gw.pc(w), "locks", griffinwing.flight.Lock.make(ga.u(f, "id"), 1)), "dwell", 0))

def griffinwing.flight.Lock.make(+foe: U32, +n: U32) -> J.Val:
  ga.su(ga.su(J.obj(), "foe", foe), "n", n)

# ─── letting go ─────────────────────────────────────────────────────────────

# The spread a starburst leaves with, by the order it was released in.
def gw.spread_x(+k: U32) -> U32:
  gw.spread_pick(U32.mod(k, 6), gw.neg(70), 70, 0, gw.neg(40), 40, 0)

def gw.spread_z(+k: U32) -> U32:
  gw.spread_pick(U32.mod(k, 6), 40, 40, 90, gw.neg(40), gw.neg(40), 0)

def gw.spread_pick(+m: U32, +a: U32, +b: U32, +c: U32, +d: U32, +e: U32, +f: U32) -> U32:
  Bool.pick(U32, U32.is_eq(m, 0), a, Bool.pick(U32, U32.is_eq(m, 1), b, Bool.pick(U32, U32.is_eq(m, 2), c,
    Bool.pick(U32, U32.is_eq(m, 3), d, Bool.pick(U32, U32.is_eq(m, 4), e, f)))))

# Returns the workspace {"s","c"}.
def griffinwing.flight.Session.release(+s: J.Val, +owner: U32, +c: J.Val, +s_c: U32, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.release.done(
    c,
    griffinwing.flight.Session.release.locks(
      ga.items(c, "locks"), s, 0, c, s_c, owner,
      U32.is_ge(ga.u(c, "charge"), ga.u(n, "charge")),
      (griffinwing.flight.STAR_MIN() + U32.div(((griffinwing.flight.STAR_MAX() - griffinwing.flight.STAR_MIN() : U32) * U32.min(ga.u(c, "charge"), ga.u(n, "charge")) : U32), U32.max(ga.u(n, "charge"), 1)) : U32)))

# w = {"s", "k"}: if k > 0 { c.cool = REL_REST; sound }
def griffinwing.flight.Session.release.done(+c: J.Val, +w: J.Val) -> J.Val:
  gw.ifj(U32.is_gt(ga.u(w, "k"), 0),
    _ => gw.pk(griffinwing.Session.sound(gw.ps(w), "shot_spark"), ga.su(c, "cool", griffinwing.flight.REL_REST())),
    _ => gw.pk(gw.ps(w), c))

def griffinwing.flight.Session.release.locks(+ls: List<&2, J.Val>, +s: J.Val, +k: U32, +c: J.Val, +s_c: U32, +owner: U32, +full: Bool, +dmg: U32) -> J.Val:
  match ls:
    case Nil{}:
      J.set(J.set(J.obj(), "s", s), "k", J.JU{k})
    case Con{+l, +t}:
      griffinwing.flight.Session.release.next(t, griffinwing.flight.Session.release.each(s, k, ga.u(l, "n"), ga.u(l, "foe"), c, s_c, owner, full, dmg), c, s_c, owner, full, dmg)

def griffinwing.flight.Session.release.next(+t: List<&2, J.Val>, +w: J.Val, +c: J.Val, +s_c: U32, +owner: U32, +full: Bool, +dmg: U32) -> J.Val:
  griffinwing.flight.Session.release.locks(t, gw.ps(w), ga.u(w, "k"), c, s_c, owner, full, dmg)

# `for _ in 0..l.n`: one starburst each, k counting them all.
def griffinwing.flight.Session.release.each(+s: J.Val, +k: U32, +m: U32, +foe: U32, +c: J.Val, +s_c: U32, +owner: U32, +full: Bool, +dmg: U32) -> J.Val:
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
def griffinwing.flight.Session.wing_fly(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.wing_fly.go(s, 0)

def griffinwing.flight.Session.wing_fly.go(+s: J.Val, +k: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, gw.len(s, "wing")),
    _ => griffinwing.flight.Session.wing_fly.go(griffinwing.flight.Session.wing_fly.one(s, k, gw.item(s, "wing", k)), (k + 1 : U32)),
    _ => s)

def griffinwing.flight.Session.wing_fly.one(+s: J.Val, +k: U32, +w: J.Val) -> J.Val:
  gw.ifj(ga.b(w, "lost"),
    _ => gw.put_item(s, "wing", k, ga.sv(w, "craft", ga.si(J.get(w, "craft"), "z", gw.max((ga.u(J.get(w, "craft"), "z") - 20 : U32), gw.neg(griffinwing.types.dm(110)))))),
    _ => griffinwing.flight.Session.wing_fly.drift(s, k, w,
      (griffinwing.flight.rider_slot.x(k) + griffinwing.types.tri((ga.u(s, "t") + (41 * k : U32) : U32), (130 + (16 * k : U32) : U32), griffinwing.types.dm(16)) : U32),
      (griffinwing.flight.rider_slot.z(k) + griffinwing.types.tri((ga.u(s, "t") + (29 * k : U32) : U32), (96 + (12 * k : U32) : U32), griffinwing.types.dm(6)) : U32)))

def griffinwing.flight.Session.wing_fly.drift(+s: J.Val, +k: U32, +w: J.Val, +tx: U32, +tz: U32) -> J.Val:
  griffinwing.flight.Session.wing_fly.lean(s, k, w, ga.u(J.get(w, "craft"), "x"), ga.u(J.get(w, "craft"), "z"), tx, tz)

def griffinwing.flight.Session.wing_fly.lean(+s: J.Val, +k: U32, +w: J.Val, +ox: U32, +oz: U32, +tx: U32, +tz: U32) -> J.Val:
  griffinwing.flight.Session.wing_fly.fire(s, k, w,
    griffinwing.flight.Session.wing_fly.craft(J.get(w, "craft"), k,
      (ox + gw.div((tx - ox : U32), 8) : U32), (oz + gw.div((tz - oz : U32), 8) : U32), ox, oz))

# The craft after its drift, its lean and its tick (the rider's bolt follows).
def griffinwing.flight.Session.wing_fly.craft(+c0: J.Val, +k: U32, +x: U32, +z: U32, +ox: U32, +oz: U32) -> J.Val:
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

def griffinwing.flight.Session.wing_fly.fire(+s: J.Val, +k: U32, +w: J.Val, +c: J.Val) -> J.Val:
  griffinwing.flight.Session.wing_fly.aim(s, k, ga.sv(w, "craft", c), ga.ssub(ga.u(w, "fire"), 1))

def griffinwing.flight.Session.wing_fly.aim(+s: J.Val, +k: U32, +w: J.Val, +fire: U32) -> J.Val:
  gw.ifj(U32.is_zero(fire),
    _ => griffinwing.flight.Session.wing_fly.shoot(s, k, w,
      (griffinwing.Session.anchor(s) + ga.u(J.get(w, "craft"), "back") : U32),
      griffinwing.flight.Session.wing_fly.pick(ga.items(s, "foes"),
        (griffinwing.Session.anchor(s) + ga.u(J.get(w, "craft"), "back") : U32), ga.u(J.get(w, "craft"), "x"), ga.u(J.get(w, "craft"), "z"), J.JNull{}, 0)),
    _ => gw.put_item(s, "wing", k, ga.su(w, "fire", fire)))

# The nearest foe ahead, within range: [s, x, z] or null. `best` and `bd` carry
# the least so far; the first of equals wins.
def griffinwing.flight.Session.wing_fly.pick(+foes: List<&2, J.Val>, +ms: U32, +mx: U32, +mz: U32, +best: J.Val, +bd: U32) -> J.Val:
  match foes:
    case Nil{}:
      best
    case Con{+f, +t}:
      griffinwing.flight.Session.wing_fly.pick2(f, t, ms, mx, mz, best, bd,
        Bool.and(Bool.and(griffinwing.types.Foe.lockable(f), Bool.not(gw.isn(f, "kind", "Lamp"))),
          gw.within((ga.u(f, "s") - ms : U32), griffinwing.types.dm(60), griffinwing.types.dm(400))))

def griffinwing.flight.Session.wing_fly.pick2(+f: J.Val, +t: List<&2, J.Val>, +ms: U32, +mx: U32, +mz: U32, +best: J.Val, +bd: U32, +ok: Bool) -> J.Val:
  gw.ifj(ok,
    _ => griffinwing.flight.Session.wing_fly.pick3(f, t, ms, mx, mz, best, bd,
      griffinwing.types.norm((ga.u(f, "s") - ms : U32), (ga.u(f, "x") - mx : U32), (ga.u(f, "z") - mz : U32))),
    _ => griffinwing.flight.Session.wing_fly.pick(t, ms, mx, mz, best, bd))

def griffinwing.flight.Session.wing_fly.pick3(+f: J.Val, +t: List<&2, J.Val>, +ms: U32, +mx: U32, +mz: U32, +best: J.Val, +bd: U32, +d: U32) -> J.Val:
  gw.ifj(Bool.or(J.is_null(best), U32.is_lt(d, bd)),
    _ => griffinwing.flight.Session.wing_fly.pick(t, ms, mx, mz,
      J.JA{[J.of_i32(ga.u(f, "s")), J.of_i32(ga.u(f, "x")), J.of_i32(ga.u(f, "z"))]}, d),
    _ => griffinwing.flight.Session.wing_fly.pick(t, ms, mx, mz, best, bd))

def griffinwing.flight.Session.wing_fly.shoot(+s: J.Val, +k: U32, +w: J.Val, +ms: U32, +pick: J.Val) -> J.Val:
  gw.ifj(J.is_null(pick),
    _ => gw.put_item(s, "wing", k, ga.su(w, "fire", 6)),
    _ => griffinwing.flight.Session.wing_fly.shoot2(s, k, w, ms, pick,
      griffinwing.types.scale3(
        (ga.nu(pick, 0) - ms : U32), (ga.nu(pick, 1) - ga.u(J.get(w, "craft"), "x") : U32), (ga.nu(pick, 2) - ga.u(J.get(w, "craft"), "z") : U32),
        griffinwing.flight.V_BOLT())))

def griffinwing.flight.Session.wing_fly.shoot2(+s: J.Val, +k: U32, +w: J.Val, +ms: U32, +pick: J.Val, +v: GwV3) -> J.Val:
  gw.put_item(
    griffinwing.flight.Session.push_bolt(s,
      griffinwing.types.Bolt.make("Ally", ga.u(w, "id"), (ms + griffinwing.types.dm(12) : U32), ga.u(J.get(w, "craft"), "x"), ga.u(J.get(w, "craft"), "z"),
        (griffinwing.types.PACE() + gw.v3a(v) : U32), gw.v3b(v), gw.v3c(v), 6, griffinwing.flight.BOLT_LIFE(), 0, 0, 0, False{})),
    "wing", k, ga.su(w, "fire", (14 + (3 * k : U32) : U32)))

# ─── the fall ───────────────────────────────────────────────────────────────

# After the beat's blows: a craft with no shield falls and rises again a
# moment later; a rider with none is lost to the beat.
def griffinwing.flight.Session.step_craft(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.flight.Session.step_craft.riders(
    griffinwing.flight.Session.step_craft.seats(s, ga.ids(J.get(s, "seats")), griffinwing.Session.anchor(s)),
    0, griffinwing.Session.anchor(s))

def griffinwing.flight.Session.step_craft.seats(+s: J.Val, +ids: List<&2, U32>, +anchor: U32) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.flight.Session.step_craft.seats(
        griffinwing.flight.Session.step_craft.seat(s, id, anchor, gw.craft_of(s, id)), t, anchor)

def griffinwing.flight.Session.step_craft.seat(+s: J.Val, +id: U32, +anchor: U32, +c: J.Val) -> J.Val:
  gw.ifj(Bool.and(gw.le(ga.u(c, "shield"), 0), U32.is_zero(ga.u(c, "down"))),
    _ => griffinwing.flight.Session.step_craft.fall(s, id, anchor, c),
    _ => s)

def griffinwing.flight.Session.step_craft.fall(+s: J.Val, +id: U32, +anchor: U32, +c: J.Val) -> J.Val:
  griffinwing.flight.Session.step_craft.release(
    griffinwing.Session.sound(
      griffinwing.Session.spark(
        gw.put_craft(s, id, griffinwing.flight.Session.step_craft.down(c)),
        "Fall", (anchor + ga.u(c, "back") : U32), ga.u(c, "x"), ga.u(c, "z"), id),
      "knight_fall"),
    id)

def griffinwing.flight.Session.step_craft.down(+c0: J.Val) -> J.Val:
  +c1 = ga.su(c0, "down", griffinwing.RISE())
  +c2 = ga.si(c1, "shield", 0)
  +c3 = ga.su(c2, "roll", 0)
  +c4 = ga.su(c3, "held", 0)
  +c5 = ga.su(c4, "charge", 0)
  ga.sv(c5, "locks", J.JA{[]})

# Whatever hung on it lets go: `for f in foes { if Latched && f.on == id { Flee, ticks 40 } }`.
def griffinwing.flight.Session.step_craft.release(+s: J.Val, +id: U32) -> J.Val:
  ga.sv(s, "foes", J.JA{griffinwing.flight.Session.step_craft.let_go(ga.items(s, "foes"), id)})

def griffinwing.flight.Session.step_craft.let_go(+fs: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Con{
        gw.ifj(Bool.and(gw.isn(f, "st", "Latched"), U32.is_eq(ga.u(f, "on"), id)),
          _ => ga.su(gw.sets(f, "st", "Flee"), "ticks", 40),
          _ => f),
        griffinwing.flight.Session.step_craft.let_go(t, id)}

def griffinwing.flight.Session.step_craft.riders(+s: J.Val, +k: U32, +anchor: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, gw.len(s, "wing")),
    _ => griffinwing.flight.Session.step_craft.riders(
      griffinwing.flight.Session.step_craft.rider(s, k, anchor, gw.item(s, "wing", k)), (k + 1 : U32), anchor),
    _ => s)

def griffinwing.flight.Session.step_craft.rider(+s: J.Val, +k: U32, +anchor: U32, +w: J.Val) -> J.Val:
  gw.ifj(Bool.and(Bool.not(ga.b(w, "lost")), gw.le(ga.u(J.get(w, "craft"), "shield"), 0)),
    _ => griffinwing.flight.Session.step_craft.gone(s, k, anchor, w),
    _ => s)

def griffinwing.flight.Session.step_craft.gone(+s: J.Val, +k: U32, +anchor: U32, +w: J.Val) -> J.Val:
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
