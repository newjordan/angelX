# crates/delve/src/rules/griffinwing/combat.rs, continued: what a hit does
# (hit_foe, kill, break_lamp), Starfall, and the bolts' tick.

# A foe takes `dmg` from `by`; whether it fell. {"s", "r": bool}.
def griffinwing.combat.Session.hit_foe(+s: J.Val, +fi: U32, +dmg: U32, +by: U32) -> J.Val:
  griffinwing.combat.Session.hit_foe.struck(
    gw.put_item(s, "foes", fi,
      ga.su(ga.su(gw.item(s, "foes", fi), "hp", ga.ssub(ga.u(gw.item(s, "foes", fi), "hp"), dmg)), "flash", 5)),
    fi, by)

def griffinwing.combat.Session.hit_foe.struck(+s: J.Val, +fi: U32, +by: U32) -> J.Val:
  gw.ifj(U32.is_gt(ga.u(gw.item(s, "foes", fi), "hp"), 0),
    _ => gw.out(griffinwing.Session.sound(s, "hit_magic"), J.JBool{False{}}),
    _ => gw.out(
      gw.ifj(gw.isn(gw.item(s, "foes", fi), "kind", "Lamp"),
        _ => griffinwing.combat.Session.break_lamp(s, fi, by),
        _ => griffinwing.combat.Session.kill(s, fi, by)),
      J.JBool{True{}}))

# `seats.get_mut(&by)`'s craft changed by `f`, or the session at0 it is.
def gw.seat_has(+s: J.Val, +by: U32) -> Bool:
  ga.has_key(J.get(s, "seats"), by)

# A foe pops into cold smoke and a spark of returned light. A kite that had
# latched on is a rescue; a knight's chain of pops is counted, and pays more
# the longer it runs.
def griffinwing.combat.Session.kill(+s: J.Val, +fi: U32, +by: U32) -> J.Val:
  griffinwing.combat.Session.kill.a(gw.remove(s, "foes", fi), gw.item(s, "foes", fi), by)

def griffinwing.combat.Session.kill.a(+s: J.Val, +f: J.Val, +by: U32) -> J.Val:
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
def griffinwing.combat.Session.kill.b(+s: J.Val, +f: J.Val, +by: U32, +gain: U32) -> J.Val:
  griffinwing.combat.Session.kill.c(
    ga.sv(s, "beat", gw.addu(J.get(s, "beat"), "base", griffinwing.types.Kind.worth(ga.s(f, "kind")))),
    f, by, gain)

def griffinwing.combat.Session.kill.c(+s: J.Val, +f: J.Val, +by: U32, +gain: U32) -> J.Val:
  gw.ifj(gw.seat_has(s, by),
    _ => griffinwing.combat.Session.kill.d(s, f, by, gain,
      ga.su(ga.su(ga.su(gw.addu(gw.craft_of(s, by), "chain", 1), "chain_ticks", 75), "hits", (ga.u(gw.craft_of(s, by), "hits") + 1 : U32)), "pts", ga.u(gw.craft_of(s, by), "pts"))),
    _ => griffinwing.combat.Session.kill.e(s, f, by, gain, 0))

def griffinwing.combat.Session.kill.d(+s: J.Val, +f: J.Val, +by: U32, +gain: U32, +c: J.Val) -> J.Val:
  griffinwing.combat.Session.kill.d2(s, f, by, c, ga.u(c, "chain"),
    (gain + gw.ifu(U32.is_ge(ga.u(c, "chain"), 3), _ => (10 * (ga.u(c, "chain") - 2 : U32) : U32), _ => 0) : U32))

def griffinwing.combat.Session.kill.d2(+s: J.Val, +f: J.Val, +by: U32, +c: J.Val, +chain: U32, +gain: U32) -> J.Val:
  griffinwing.combat.Session.kill.e(gw.put_craft(s, by, ga.su(c, "pts", (ga.u(c, "pts") + gain : U32))), f, by, gain, chain)

def griffinwing.combat.Session.kill.e(+s: J.Val, +f: J.Val, +by: U32, +gain: U32, +chain: U32) -> J.Val:
  griffinwing.combat.Session.kill.fin(
    griffinwing.Session.sound(
      griffinwing.Session.spark(ga.sv(s, "beat", gw.addu(J.get(s, "beat"), "pts", gain)), "Pop", ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), U32.min(chain, 99)),
      "kill"),
    by, chain)

def griffinwing.combat.Session.kill.fin(+s: J.Val, +by: U32, +chain: U32) -> J.Val:
  gw.ifj(U32.is_eq(chain, 5), _ => griffinwing.Session.say(s, by, "Chain"), _ => s)

# A lamp's flame is out: points, a flash, and the Wyrm reels (the Wyrm's tick
# sees it).
def griffinwing.combat.Session.break_lamp(+s: J.Val, +fi: U32, +by: U32) -> J.Val:
  griffinwing.combat.Session.break_lamp.a(
    gw.put_item(s, "foes", fi, ga.su(gw.sets(gw.item(s, "foes", fi), "st", "Broken"), "hp", 0)),
    gw.item(s, "foes", fi), by)

def griffinwing.combat.Session.break_lamp.a(+s: J.Val, +f: J.Val, +by: U32) -> J.Val:
  griffinwing.Session.sound(
    griffinwing.Session.spark(
      gw.ifj(gw.seat_has(s, by),
        _ => gw.put_craft(griffinwing.combat.Session.break_lamp.beat(s), by,
          ga.su(ga.su(gw.craft_of(s, by), "pts", (ga.u(gw.craft_of(s, by), "pts") + 500 : U32)), "hits", (ga.u(gw.craft_of(s, by), "hits") + 1 : U32))),
        _ => griffinwing.combat.Session.break_lamp.beat(s)),
      "Lamp", ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), 2),
    "boss_rage")

def griffinwing.combat.Session.break_lamp.beat(+s: J.Val) -> J.Val:
  ga.sv(s, "beat", gw.addu(gw.addu(gw.addu(J.get(s, "beat"), "pts", 500), "base", 500), "kills", 1))

# Starfall: every hostile bolt is cleared, and everything small on the way is
# struck for what it can bear.
def griffinwing.combat.Session.starfall(+s: J.Val, +by: U32, +s_c: U32, +x: U32, +z: U32) -> J.Val:
  griffinwing.combat.Session.starfall.sweep(
    ga.sv(
      griffinwing.Session.sound(griffinwing.Session.spark(s, "Starfall", s_c, 0, 0, by), "bomb"),
      "bolts", J.JA{griffinwing.combat.Session.starfall.clear(ga.items(s, "bolts"))}),
    0, by, griffinwing.Session.anchor(s))

# bolts.retain(|b| !matches!(b.shot, Spark | Gloam))
def griffinwing.combat.Session.starfall.clear(+bs: List<&2, J.Val>) -> List<&2, J.Val>:
  match bs:
    case Nil{}:
      Nil{}
    case Con{+b, +t}:
      gw.ifjl(Bool.or(gw.isn(b, "shot", "Spark"), gw.isn(b, "shot", "Gloam")),
        _ => griffinwing.combat.Session.starfall.clear(t),
        _ => Con{b, griffinwing.combat.Session.starfall.clear(t)})

def griffinwing.combat.Session.starfall.sweep(+s: J.Val, +i: U32, +by: U32, +anchor: U32) -> J.Val:
  gw.ifj(U32.is_lt(i, gw.len(s, "foes")),
    _ => griffinwing.combat.Session.starfall.foe(s, i, by, anchor, gw.item(s, "foes", i)),
    _ => s)

def griffinwing.combat.Session.starfall.foe(+s: J.Val, +i: U32, +by: U32, +anchor: U32, +f: J.Val) -> J.Val:
  gw.ifj(Bool.and(gw.within((ga.u(f, "s") - anchor : U32), gw.neg(griffinwing.types.dm(50)), griffinwing.types.dm(600)),
      Bool.and(griffinwing.types.Foe.lockable(f), Bool.not(gw.isn(f, "kind", "Lamp")))),
    _ => griffinwing.combat.Session.starfall.hit(griffinwing.combat.Session.hit_foe(s, i, griffinwing.flight.STARFALL(), by), i, by, anchor),
    _ => griffinwing.combat.Session.starfall.sweep(s, (i + 1 : U32), by, anchor))

def griffinwing.combat.Session.starfall.hit(+r: J.Val, +i: U32, +by: U32, +anchor: U32) -> J.Val:
  griffinwing.combat.Session.starfall.sweep(gw.out_s(r), gw.ifu(J.bool(gw.out_r(r)), _ => i, _ => (i + 1 : U32)), by, anchor)

# ─── the bolts' tick ────────────────────────────────────────────────────────

# The bolts fly and home, then land.
def griffinwing.combat.Session.step_bolts(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.combat.Session.step_bolts.land(
    griffinwing.combat.Session.step_bolts.move(s, 0, gw.len(s, "bolts"), griffinwing.Session.anchor(s), J.JA{[]}, J.JA{[]}),
    n, griffinwing.Session.anchor(s))

# First pass: each bolt waits at its muzzle or homes and moves. The workspace
# {"s", "segs": null or [from, to] per bolt, "dead": bool per bolt}.
def griffinwing.combat.Session.step_bolts.move(+s: J.Val, +i: U32, +count: U32, +anchor: U32, +segs: J.Val, +dead: J.Val) -> J.Val:
  gw.ifj(U32.is_lt(i, count),
    _ => griffinwing.combat.Session.step_bolts.move.next(s, i, count, anchor, segs, dead,
      griffinwing.combat.Session.step_bolts.one(s, gw.item(s, "bolts", i), anchor)),
    _ => J.set(J.set(J.set(J.obj(), "s", s), "segs", segs), "dead", dead))

def griffinwing.combat.Session.step_bolts.move.next(+s: J.Val, +i: U32, +count: U32, +anchor: U32, +segs: J.Val, +dead: J.Val, +r: J.Val) -> J.Val:
  griffinwing.combat.Session.step_bolts.move(gw.put_item(s, "bolts", i, J.get(r, "b")), (i + 1 : U32), count, anchor, J.push(segs, J.get(r, "seg")), J.push(dead, J.get(r, "dead")))

def griffinwing.combat.Session.step_bolts.one(+s: J.Val, +b: J.Val, +anchor: U32) -> J.Val:
  gw.ifj(U32.is_gt(ga.u(b, "delay"), 0),
    _ => griffinwing.combat.Session.step_bolts.wait(b, griffinwing.flight.Session.craft_at(s, ga.u(b, "owner"))),
    _ => griffinwing.combat.Session.step_bolts.fly(s, griffinwing.combat.Session.step_bolts.home(s, b)))

# {"b", "seg", "dead"}
def gw.bolt_out(+b: J.Val, +seg: J.Val, +dead: Bool) -> J.Val:
  J.set(J.set(J.set(J.obj(), "b", b), "seg", seg), "dead", J.JBool{dead})

# A released starburst waits at its craft's muzzle for its turn.
def griffinwing.combat.Session.step_bolts.wait(+b: J.Val, +at: J.Val) -> J.Val:
  gw.ifj(Bool.and(gw.at_some(at), gw.at_ok(at)),
    _ => gw.bolt_out(
      ga.su(ga.si(ga.si(ga.si(b, "s", (gw.at_s(at) + griffinwing.types.dm(12) : U32)), "x", gw.at_x(at)), "z", gw.at_z(at)), "delay", (ga.u(b, "delay") - 1 : U32)),
      J.JNull{}, False{}),
    _ => gw.bolt_out(b, J.JNull{}, True{}))

# Homing: a starburst and a turned bolt close on their body.
def griffinwing.combat.Session.step_bolts.home(+s: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(Bool.and(U32.is_ne(ga.u(b, "aim"), 0), Bool.or(gw.isn(b, "shot", "Star"), gw.isn(b, "shot", "Turned"))),
    _ => griffinwing.combat.Session.step_bolts.home2(s, b, griffinwing.combat.Session.step_bolts.body(ga.items(s, "foes"), ga.u(b, "aim"))),
    _ => b)

# The live foe with this id: [s, x, z] or null.
def griffinwing.combat.Session.step_bolts.body(+fs: List<&2, J.Val>, +id: U32) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      gw.ifj(Bool.and(U32.is_eq(ga.u(f, "id"), id), griffinwing.types.Foe.alive(f)),
        _ => J.JA{[J.of_i32(ga.u(f, "s")), J.of_i32(ga.u(f, "x")), J.of_i32(ga.u(f, "z"))]},
        _ => griffinwing.combat.Session.step_bolts.body(t, id))

def griffinwing.combat.Session.step_bolts.home2(+s: J.Val, +b: J.Val, +p: J.Val) -> J.Val:
  gw.ifj(J.is_null(p),
    _ => ga.su(b, "aim", griffinwing.combat.Session.step_bolts.next_aim(ga.items(s, "foes"), b, J.JNull{}, 0)),
    _ => griffinwing.combat.Session.step_bolts.steer(b,
      griffinwing.types.scale3((ga.nu(p, 0) - ga.u(b, "s") : U32), (ga.nu(p, 1) - ga.u(b, "x") : U32), (ga.nu(p, 2) - ga.u(b, "z") : U32),
        gw.ifu(gw.isn(b, "shot", "Turned"), _ => griffinwing.combat.V_TURN(), _ => griffinwing.flight.Session.star_speed()))))

def griffinwing.combat.Session.step_bolts.steer(+b: J.Val, +want: GwV3) -> J.Val:
  ga.si(ga.si(ga.si(b, "vs", (ga.u(b, "vs") + gw.div((gw.v3a(want) - ga.u(b, "vs") : U32), 3) : U32)),
    "vx", (ga.u(b, "vx") + gw.div((gw.v3b(want) - ga.u(b, "vx") : U32), 3) : U32)),
    "vz", (ga.u(b, "vz") + gw.div((gw.v3c(want) - ga.u(b, "vz") : U32), 3) : U32))

# Its body is gone: on to the nearest ahead (lockable, not a lamp, within
# dm(20)..=dm(500) ahead of the bolt); the first of equals wins. The foe id, or 0.
def griffinwing.combat.Session.step_bolts.next_aim(+fs: List<&2, J.Val>, +b: J.Val, +best: J.Val, +bd: U32) -> U32:
  match fs:
    case Nil{}:
      gw.ifu(J.is_null(best), _ => 0, _ => J.u32(best))
    case Con{+f, +t}:
      gw.ifu(Bool.and(Bool.and(griffinwing.types.Foe.lockable(f), Bool.not(gw.isn(f, "kind", "Lamp"))),
          gw.within((ga.u(f, "s") - ga.u(b, "s") : U32), griffinwing.types.dm(20), griffinwing.types.dm(500))),
        _ => griffinwing.combat.Session.step_bolts.next_aim2(f, t, b, best, bd,
          griffinwing.types.norm((ga.u(f, "s") - ga.u(b, "s") : U32), (ga.u(f, "x") - ga.u(b, "x") : U32), (ga.u(f, "z") - ga.u(b, "z") : U32))),
        _ => griffinwing.combat.Session.step_bolts.next_aim(t, b, best, bd))

def griffinwing.combat.Session.step_bolts.next_aim2(+f: J.Val, +t: List<&2, J.Val>, +b: J.Val, +best: J.Val, +bd: U32, +d: U32) -> U32:
  gw.ifu(Bool.or(J.is_null(best), U32.is_lt(d, bd)),
    _ => griffinwing.combat.Session.step_bolts.next_aim(t, b, J.JU{ga.u(f, "id")}, d),
    _ => griffinwing.combat.Session.step_bolts.next_aim(t, b, best, bd))

# The move: from, then on its velocity; a tick of life gone.
def griffinwing.combat.Session.step_bolts.fly(+s: J.Val, +b0: J.Val) -> J.Val:
  griffinwing.combat.Session.step_bolts.fly2(b0,
    ga.su(ga.si(ga.si(ga.si(b0, "s", (ga.u(b0, "s") + ga.u(b0, "vs") : U32)), "x", (ga.u(b0, "x") + ga.u(b0, "vx") : U32)), "z", (ga.u(b0, "z") + ga.u(b0, "vz") : U32)),
      "life", ga.ssub(ga.u(b0, "life"), 1)))

def griffinwing.combat.Session.step_bolts.fly2(+b0: J.Val, +b: J.Val) -> J.Val:
  gw.bolt_out(b,
    J.JA{[J.of_i32(ga.u(b0, "s")), J.of_i32(ga.u(b0, "x")), J.of_i32(ga.u(b0, "z")), J.of_i32(ga.u(b, "s")), J.of_i32(ga.u(b, "x")), J.of_i32(ga.u(b, "z"))]},
    False{})

# Second pass: each bolt lands on what its swept path meets.
def griffinwing.combat.Session.step_bolts.land(+w: J.Val, +n: J.Val, +anchor: U32) -> J.Val:
  griffinwing.combat.Session.step_bolts.retain(
    griffinwing.combat.Session.step_bolts.hits(J.get(w, "s"), 0, gw.len(J.get(w, "s"), "bolts"), J.get(w, "segs"), J.get(w, "dead"), n),
    anchor)

# {"s", "dead"}
def griffinwing.combat.Session.step_bolts.hits(+s: J.Val, +i: U32, +count: U32, +segs: J.Val, +dead: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(U32.is_lt(i, count),
    _ => griffinwing.combat.Session.step_bolts.hit(s, i, count, segs, dead, n, J.nth(segs, i)),
    _ => J.set(J.set(J.obj(), "s", s), "dead", dead))

def griffinwing.combat.Session.step_bolts.hit(+s: J.Val, +i: U32, +count: U32, +segs: J.Val, +dead: J.Val, +n: J.Val, +seg: J.Val) -> J.Val:
  gw.ifj(Bool.or(J.is_null(seg), J.bool(J.nth(dead, i))),
    _ => griffinwing.combat.Session.step_bolts.hits(s, (i + 1 : U32), count, segs, dead, n),
    _ => griffinwing.combat.Session.step_bolts.hit2(s, i, count, segs, dead, n, seg, gw.item(s, "bolts", i)))

def griffinwing.combat.Session.step_bolts.hit2(+s: J.Val, +i: U32, +count: U32, +segs: J.Val, +dead: J.Val, +n: J.Val, +seg: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(Bool.or(gw.isn(b, "shot", "Spark"), gw.isn(b, "shot", "Gloam")),
    _ => griffinwing.combat.Session.step_bolts.hit3(
      i, count, segs, dead, n,
      griffinwing.combat.Session.hostile_bolt(s, b, ga.nu(seg, 0), ga.nu(seg, 1), ga.nu(seg, 2), ga.nu(seg, 3), ga.nu(seg, 4), ga.nu(seg, 5), n)),
    _ => griffinwing.combat.Session.step_bolts.hit4(
      i, count, segs, n,
      griffinwing.combat.Session.friendly_bolt(s, b, ga.nu(seg, 0), ga.nu(seg, 1), ga.nu(seg, 2), ga.nu(seg, 3), ga.nu(seg, 4), ga.nu(seg, 5), dead)))

# A hostile bolt flew (or not): bolts[i] = its turned self.
def griffinwing.combat.Session.step_bolts.hit3(+i: U32, +count: U32, +segs: J.Val, +dead: J.Val, +n: J.Val, +r: J.Val) -> J.Val:
  griffinwing.combat.Session.step_bolts.hits(
    gw.put_item(J.get(r, "s"), "bolts", i, J.get(r, "b")), (i + 1 : U32), count, segs,
    gw.ifj(J.bool(J.get(r, "r")), _ => dead, _ => J.set_nth(dead, i, J.JBool{True{}})), n)

def griffinwing.combat.Session.step_bolts.hit4(+i: U32, +count: U32, +segs: J.Val, +n: J.Val, +r: J.Val) -> J.Val:
  griffinwing.combat.Session.step_bolts.hits(
    J.get(r, "s"), (i + 1 : U32), count, segs,
    gw.ifj(J.bool(J.get(r, "r")), _ => J.get(r, "dead"), _ => J.set_nth(J.get(r, "dead"), i, J.JBool{True{}})), n)

# bolts.retain(|b| !dead[k] && b.life > 0 && anchor - dm(80) <= b.s <= anchor + dm(1200))
def griffinwing.combat.Session.step_bolts.retain(+w: J.Val, +anchor: U32) -> J.Val:
  ga.sv(J.get(w, "s"), "bolts", J.JA{griffinwing.combat.Session.step_bolts.keep(ga.items(J.get(w, "s"), "bolts"), ga.items(w, "dead"), anchor)})

def griffinwing.combat.Session.step_bolts.keep(+bs: List<&2, J.Val>, +dead: List<&2, J.Val>, +anchor: U32) -> List<&2, J.Val>:
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
def griffinwing.combat.Session.friendly_bolt(+s: J.Val, +b: J.Val, +fs: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +dead: J.Val) -> J.Val:
  griffinwing.combat.Session.friendly_bolt.land(s, b, fs, fx, fz, ts, tx, tz, dead,
    griffinwing.combat.Session.friendly_bolt.best(ga.items(s, "foes"), b, fs, fx, fz, ts, tx, tz, 0, J.JNull{}, 0))

# The foe the bolt lands on: its index, or null. `best` and `bd` carry the
# nearest so far (the first of equals wins).
def griffinwing.combat.Session.friendly_bolt.best(+fs: List<&2, J.Val>, +b: J.Val, +fs0: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +k: U32, +best: J.Val, +bd: U32) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      gw.ifj(Bool.and(griffinwing.types.Foe.alive(f),
          Bool.and(Bool.not(gw.isn(f, "st", "Hide")),
            Bool.not(Bool.and(gw.isn(b, "shot", "Star"), Bool.and(U32.is_ne(ga.u(b, "aim"), 0), U32.is_ne(ga.u(f, "id"), ga.u(b, "aim"))))))),
        _ => griffinwing.combat.Session.friendly_bolt.best2(f, t, b, fs0, fx, fz, ts, tx, tz, k, best, bd),
        _ => griffinwing.combat.Session.friendly_bolt.best(t, b, fs0, fx, fz, ts, tx, tz, (k + 1 : U32), best, bd))

def griffinwing.combat.Session.friendly_bolt.best2(+f: J.Val, +t: List<&2, J.Val>, +b: J.Val, +fs0: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +k: U32, +best: J.Val, +bd: U32) -> J.Val:
  gw.ifj(griffinwing.types.near(fs0, fx, fz, ts, tx, tz, ga.u(f, "s"), ga.u(f, "x"), ga.u(f, "z"), (griffinwing.types.Foe.radius(f) + griffinwing.combat.BOLT_R() : U32)),
    _ => griffinwing.combat.Session.friendly_bolt.best3(f, t, b, fs0, fx, fz, ts, tx, tz, k, best, bd,
      griffinwing.types.norm((ga.u(f, "s") - fs0 : U32), (ga.u(f, "x") - fx : U32), (ga.u(f, "z") - fz : U32))),
    _ => griffinwing.combat.Session.friendly_bolt.best(t, b, fs0, fx, fz, ts, tx, tz, (k + 1 : U32), best, bd))

def griffinwing.combat.Session.friendly_bolt.best3(+f: J.Val, +t: List<&2, J.Val>, +b: J.Val, +fs0: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +k: U32, +best: J.Val, +bd: U32, +d: U32) -> J.Val:
  gw.ifj(Bool.or(J.is_null(best), U32.is_lt(d, bd)),
    _ => griffinwing.combat.Session.friendly_bolt.best(t, b, fs0, fx, fz, ts, tx, tz, (k + 1 : U32), J.JU{k}, d),
    _ => griffinwing.combat.Session.friendly_bolt.best(t, b, fs0, fx, fz, ts, tx, tz, (k + 1 : U32), best, bd))

def griffinwing.combat.Session.friendly_bolt.land(+s: J.Val, +b: J.Val, +fs: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +dead: J.Val, +best: J.Val) -> J.Val:
  gw.ifj(Bool.not(J.is_null(best)),
    _ => J.set(J.set(J.set(J.obj(), "s", griffinwing.combat.Session.land(s, J.u32(best), b)), "r", J.JBool{False{}}), "dead", dead),
    _ => griffinwing.combat.Session.friendly_bolt.spark(s, dead,
      griffinwing.combat.Session.friendly_bolt.shoot(ga.items(s, "bolts"), dead, fs, fx, fz, ts, tx, tz, 0)))

# A bolt shoots a snuffer's spark out of the air: the first live Spark the path
# meets, its index or null.
def griffinwing.combat.Session.friendly_bolt.shoot(+bs: List<&2, J.Val>, +dead: J.Val, +fs: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +j: U32) -> J.Val:
  match bs:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      gw.ifj(Bool.and(Bool.not(J.bool(J.nth(dead, j))),
          Bool.and(gw.isn(h, "shot", "Spark"),
            griffinwing.types.near(fs, fx, fz, ts, tx, tz, ga.u(h, "s"), ga.u(h, "x"), ga.u(h, "z"), (griffinwing.combat.SPARK_R() + griffinwing.combat.BOLT_R() : U32)))),
        _ => J.JU{j},
        _ => griffinwing.combat.Session.friendly_bolt.shoot(t, dead, fs, fx, fz, ts, tx, tz, (j + 1 : U32)))

def griffinwing.combat.Session.friendly_bolt.spark(+s: J.Val, +dead: J.Val, +hit: J.Val) -> J.Val:
  gw.ifj(J.is_null(hit),
    _ => J.set(J.set(J.set(J.obj(), "s", s), "r", J.JBool{True{}}), "dead", dead),
    _ => J.set(J.set(J.set(J.obj(), "s",
        griffinwing.Session.spark(s, "Hit", ga.u(gw.item(s, "bolts", J.u32(hit)), "s"), ga.u(gw.item(s, "bolts", J.u32(hit)), "x"), ga.u(gw.item(s, "bolts", J.u32(hit)), "z"), 0)),
      "r", J.JBool{False{}}),
      "dead", J.set_nth(dead, J.u32(hit), J.JBool{True{}})))

# A friendly bolt lands on foe `i`.
def griffinwing.combat.Session.land(+s: J.Val, +i: U32, +b: J.Val) -> J.Val:
  gw.ifj(gw.isn(gw.item(s, "foes", i), "kind", "Lamp"),
    _ => griffinwing.combat.Session.land.lamp(s, i, b,
      Bool.and(Bool.not(J.is_null(J.get(s, "boss"))),
        U32.is_eq(J.u32(J.nth(J.get(J.get(s, "boss"), "lamps"), U32.min(ga.u(J.get(s, "boss"), "phase"), 2))), ga.u(gw.item(s, "foes", i), "id")))),
    _ => griffinwing.combat.Session.land.small(s, i, b))

def griffinwing.combat.Session.land.small(+s: J.Val, +i: U32, +b: J.Val) -> J.Val:
  gw.out_s(griffinwing.combat.Session.hit_foe(
    griffinwing.Session.spark(s, "Hit", ga.u(gw.item(s, "foes", i), "s"), ga.u(gw.item(s, "foes", i), "x"), ga.u(gw.item(s, "foes", i), "z"), 0),
    i, gw.ifu(gw.isn(b, "shot", "Turned"), _ => griffinwing.combat.TURNED(), _ => ga.u(b, "dmg")), ga.u(b, "owner")))

def griffinwing.combat.Session.land.lamp(+s: J.Val, +i: U32, +b: J.Val, +active: Bool) -> J.Val:
  griffinwing.combat.Session.land.lamp2(s, i, b, active, gw.item(s, "foes", i))

def griffinwing.combat.Session.land.lamp2(+s: J.Val, +i: U32, +b: J.Val, +active: Bool, +f: J.Val) -> J.Val:
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
def griffinwing.combat.Session.hostile_bolt(+s: J.Val, +b: J.Val, +fs: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +n: J.Val) -> J.Val:
  griffinwing.combat.Session.hostile_bolt.each(s, b, fs, fx, fz, ts, tx, tz, n,
    gw.ifu(gw.isn(b, "shot", "Gloam"), _ => griffinwing.combat.GLOAM_R(), _ => griffinwing.combat.SPARK_R()),
    griffinwing.combat.Session.hostile_bolt.crafts(s, ga.ids(J.get(s, "seats")), griffinwing.Nums.reflect(n)))

# (id, craft_s, x, z, reflecting, hurt > 0) of each knight flying.
def griffinwing.combat.Session.hostile_bolt.crafts(+s: J.Val, +ids: List<&2, U32>, +reflect: U32) -> List<&2, J.Val>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gw.ifjl(griffinwing.types.Craft.flying(gw.craft_of(s, id)),
        _ => Con{J.JA{[J.JU{id}, J.of_i32(griffinwing.flight.Session.craft_s(s, gw.craft_of(s, id))), J.of_i32(ga.u(gw.craft_of(s, id), "x")), J.of_i32(ga.u(gw.craft_of(s, id), "z")),
            J.JBool{griffinwing.flight.Craft.reflecting(gw.craft_of(s, id), reflect)}, J.JBool{U32.is_gt(ga.u(gw.craft_of(s, id), "hurt"), 0)}]},
          griffinwing.combat.Session.hostile_bolt.crafts(s, t, reflect)},
        _ => griffinwing.combat.Session.hostile_bolt.crafts(s, t, reflect))

def griffinwing.combat.Session.hostile_bolt.each(+s: J.Val, +b: J.Val, +fs: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32, +n: J.Val, +r: U32, +crafts: List<&2, J.Val>) -> J.Val:
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
def griffinwing.combat.Session.turn(+s: J.Val, +b: J.Val, +by: U32, +at0: U32, +ax: U32, +az: U32) -> J.Val:
  griffinwing.combat.Session.turn.go(s, b, by, at0,
    griffinwing.combat.Session.turn.target(s, b, at0, ax, az))

# The id the bolt heads for, or null: what fired it (if alive), else the lit
# lamp (if alive), else the nearest lockable foe ahead that is not a lamp.
def griffinwing.combat.Session.turn.target(+s: J.Val, +b: J.Val, +at0: U32, +ax: U32, +az: U32) -> J.Val:
  griffinwing.combat.Session.turn.target2(s, b, at0, ax, az, griffinwing.combat.Session.turn.alive(ga.items(s, "foes"), ga.u(b, "src")))

def griffinwing.combat.Session.turn.target2(+s: J.Val, +b: J.Val, +at0: U32, +ax: U32, +az: U32, +by_src: J.Val) -> J.Val:
  gw.ifj(Bool.not(J.is_null(by_src)),
    _ => by_src,
    _ => griffinwing.combat.Session.turn.target3(s, at0, ax, az,
      gw.ifj(J.is_null(J.get(s, "boss")),
        _ => J.JNull{},
        _ => griffinwing.combat.Session.turn.alive(ga.items(s, "foes"), J.u32(J.nth(J.get(J.get(s, "boss"), "lamps"), U32.min(ga.u(J.get(s, "boss"), "phase"), 2)))))))

def griffinwing.combat.Session.turn.target3(+s: J.Val, +at0: U32, +ax: U32, +az: U32, +lamp: J.Val) -> J.Val:
  gw.ifj(Bool.not(J.is_null(lamp)),
    _ => lamp,
    _ => griffinwing.combat.Session.turn.nearest(ga.items(s, "foes"), at0, ax, az, J.JNull{}, 0))

# The id of the foe with this id if it is alive, else null.
def griffinwing.combat.Session.turn.alive(+fs: List<&2, J.Val>, +id: U32) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      gw.ifj(Bool.and(U32.is_eq(ga.u(f, "id"), id), griffinwing.types.Foe.alive(f)),
        _ => J.JU{id},
        _ => griffinwing.combat.Session.turn.alive(t, id))

def griffinwing.combat.Session.turn.nearest(+fs: List<&2, J.Val>, +at0: U32, +ax: U32, +az: U32, +best: J.Val, +bd: U32) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      gw.ifj(Bool.and(Bool.and(griffinwing.types.Foe.lockable(f), Bool.not(gw.isn(f, "kind", "Lamp"))), gw.gt(ga.u(f, "s"), at0)),
        _ => griffinwing.combat.Session.turn.nearest2(f, t, at0, ax, az, best, bd,
          griffinwing.types.norm((ga.u(f, "s") - at0 : U32), (ga.u(f, "x") - ax : U32), (ga.u(f, "z") - az : U32))),
        _ => griffinwing.combat.Session.turn.nearest(t, at0, ax, az, best, bd))

def griffinwing.combat.Session.turn.nearest2(+f: J.Val, +t: List<&2, J.Val>, +at0: U32, +ax: U32, +az: U32, +best: J.Val, +bd: U32, +d: U32) -> J.Val:
  gw.ifj(Bool.or(J.is_null(best), U32.is_lt(d, bd)),
    _ => griffinwing.combat.Session.turn.nearest(t, at0, ax, az, J.JU{ga.u(f, "id")}, d),
    _ => griffinwing.combat.Session.turn.nearest(t, at0, ax, az, best, bd))

# The foe with this id (any state), or null.
def griffinwing.combat.Session.turn.find(+fs: List<&2, J.Val>, +id: U32) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      gw.ifj(U32.is_eq(ga.u(f, "id"), id), _ => f, _ => griffinwing.combat.Session.turn.find(t, id))

def griffinwing.combat.Session.turn.go(+s: J.Val, +b0: J.Val, +by: U32, +at0: U32, +target: J.Val) -> J.Val:
  griffinwing.combat.Session.turn.done(s, by,
    griffinwing.combat.Session.turn.aimed(
      ga.su(ga.su(ga.su(gw.sets(ga.su(b0, "owner", U32.and(by, 255)), "shot", "Turned"), "dmg", griffinwing.combat.TURNED()), "life", 90), "aim", gw.ifu(J.is_null(target), _ => 0, _ => J.u32(target))),
      gw.ifj(J.is_null(target), _ => J.JNull{}, _ => griffinwing.combat.Session.turn.find(ga.items(s, "foes"), J.u32(target)))))

def griffinwing.combat.Session.turn.aimed(+b: J.Val, +f: J.Val) -> J.Val:
  gw.ifj(J.is_null(f),
    _ => ga.si(ga.si(ga.si(b, "vs", griffinwing.combat.V_TURN()), "vx", 0), "vz", 0),
    _ => griffinwing.combat.Session.turn.vel(b, griffinwing.types.scale3((ga.u(f, "s") - ga.u(b, "s") : U32), (ga.u(f, "x") - ga.u(b, "x") : U32), (ga.u(f, "z") - ga.u(b, "z") : U32), griffinwing.combat.V_TURN())))

def griffinwing.combat.Session.turn.vel(+b: J.Val, +v: GwV3) -> J.Val:
  ga.si(ga.si(ga.si(b, "vs", gw.v3a(v)), "vx", gw.v3b(v)), "vz", gw.v3c(v))

# The roller scores; the beat too; a flash and a sound.
def griffinwing.combat.Session.turn.done(+s: J.Val, +by: U32, +b: J.Val) -> J.Val:
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
