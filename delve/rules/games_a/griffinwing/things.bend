# crates/delve/src/rules/griffinwing/things.rs — the things that stand on the
# rail: bonus gates and hearth rings to fly through, an arch to thread, a ridge
# to clear, shutters that close on a clock.

# A shutter's cycle, in ticks, and the narrowest its gap closes to (half).
def griffinwing.things.SHUTTER_PERIOD() -> U32:
  150

def griffinwing.things.SHUTTER_MIN() -> U32:
  griffinwing.types.dm(5)

# What a scrape of rock takes off a shield, and what a hearth ring mends.
def griffinwing.things.SCRAPE() -> U32:
  15

def griffinwing.things.MEND() -> U32:
  50

# A bonus gate's points, doubled for a gate flown on the spur.
def griffinwing.things.GATE() -> U32:
  100

# A shutter's half gap at tick `t`: open and closing and opening again over its
# cycle, the way it shows from the far end of the rail.
def griffinwing.things.shutter_gap(+p: J.Val, +t: U32, +wx: U32) -> U32:
  griffinwing.things.shutter_gap.k(p, wx,
    (griffinwing.types.tri((t + ga.u(p, "phase") : U32), griffinwing.things.SHUTTER_PERIOD(), 1000) + 1000 : U32))

def griffinwing.things.shutter_gap.k(+p: J.Val, +wx: U32, +k: U32) -> U32:
  (griffinwing.things.SHUTTER_MIN() + gw.div(((wx + griffinwing.types.dm(10) - griffinwing.things.SHUTTER_MIN() : U32) * k : U32), 2000) : U32)

# Props and the knights who cross them.
def griffinwing.things.Session.step_props(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.things.Session.step_props.retain(
    griffinwing.things.Session.step_props.ids(s, ga.ids(J.get(s, "seats")), griffinwing.Session.anchor(s), ga.u(s, "t"), griffinwing.Nums.wx(n)),
    griffinwing.Session.anchor(s))

def griffinwing.things.Session.step_props.retain(+s: J.Val, +anchor: U32) -> J.Val:
  ga.sv(s, "props", J.JA{griffinwing.things.Session.step_props.keep(ga.items(s, "props"), anchor)})

def griffinwing.things.Session.step_props.keep(+ps: List<&2, J.Val>, +anchor: U32) -> List<&2, J.Val>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      gw.ifjl(gw.gt((ga.u(p, "s") + ga.u(p, "len") : U32), (anchor - griffinwing.types.dm(300) : U32)),
        _ => Con{p, griffinwing.things.Session.step_props.keep(t, anchor)},
        _ => griffinwing.things.Session.step_props.keep(t, anchor))

def griffinwing.things.Session.step_props.ids(+s: J.Val, +ids: List<&2, U32>, +anchor: U32, +t: U32, +wx: U32) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +rest}:
      griffinwing.things.Session.step_props.ids(
        griffinwing.things.Session.step_props.seat(s, id, t, wx, gw.craft_of(s, id)),
        rest, anchor, t, wx)

# One knight: if flying, every prop in turn against where it crossed this tick.
def griffinwing.things.Session.step_props.seat(+s: J.Val, +id: U32, +t: U32, +wx: U32, +c: J.Val) -> J.Val:
  gw.ifj(Bool.not(griffinwing.types.Craft.flying(c)),
    _ => s,
    _ => griffinwing.things.Session.step_props.walk(s, id, 0, t, wx,
      griffinwing.flight.Session.craft_s(s, c), ga.u(c, "ds"), ga.u(c, "x"), ga.u(c, "z"), ga.b(c, "boost")))

def griffinwing.things.Session.step_props.walk(+s: J.Val, +id: U32, +pi: U32, +t: U32, +wx: U32, +s1: U32, +ds: U32, +x: U32, +z: U32, +boosting: Bool) -> J.Val:
  gw.ifj(U32.is_lt(pi, gw.len(s, "props")),
    _ => griffinwing.things.Session.step_props.walk(
      griffinwing.things.Session.step_props.prop(s, id, pi, t, wx, s1, (s1 - ds : U32), x, z, boosting, gw.item(s, "props", pi)),
      id, (pi + 1 : U32), t, wx, s1, ds, x, z, boosting),
    _ => s)

def griffinwing.things.Session.step_props.prop(+s: J.Val, +id: U32, +pi: U32, +t: U32, +wx: U32, +s1: U32, +s0: U32, +x: U32, +z: U32, +boosting: Bool, +p: J.Val) -> J.Val:
  griffinwing.things.Session.step_props.thing(s, id, pi, t, wx, s1, s0, x, z, boosting, p,
    Bool.and(gw.lt(s0, (ga.u(p, "s") + ga.u(p, "len") : U32)), gw.gt(s1, ga.u(p, "s"))))

def griffinwing.things.Session.step_props.thing(+s: J.Val, +id: U32, +pi: U32, +t: U32, +wx: U32, +s1: U32, +s0: U32, +x: U32, +z: U32, +boosting: Bool, +p: J.Val, +over: Bool) -> J.Val:
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
def griffinwing.things.Session.take(+s: J.Val, +id: U32, +p: J.Val, +boosting: Bool) -> J.Val:
  griffinwing.Session.spark(
    gw.ifj(gw.isn(p, "thing", "Gate"),
      _ => griffinwing.things.Session.take.gate(s, id, p, boosting),
      _ => griffinwing.Session.sound(
        gw.put_craft(s, id, ga.si(gw.craft_of(s, id), "shield", gw.min((ga.u(gw.craft_of(s, id), "shield") + griffinwing.things.MEND() : U32), griffinwing.types.SHIELD()))),
        "shield_up")),
    "Gate", ga.u(p, "s"), ga.u(p, "x"), ga.u(p, "z"), U32.and(id, 255))

# A victory gate (`phase` 1) is a welcome, not a score.
def griffinwing.things.Session.take.gate(+s: J.Val, +id: U32, +p: J.Val, +boosting: Bool) -> J.Val:
  griffinwing.Session.sound(
    gw.ifj(U32.is_zero(ga.u(p, "phase")),
      _ => griffinwing.things.Session.take.score(s, id, (griffinwing.things.GATE() * gw.ifu(boosting, _ => 2, _ => 1) : U32)),
      _ => s),
    "gold")

def griffinwing.things.Session.take.score(+s: J.Val, +id: U32, +pts: U32) -> J.Val:
  ga.sv(
    gw.ifj(gw.seat_has(s, id),
      _ => gw.put_craft(s, id, ga.su(ga.su(gw.craft_of(s, id), "pts", (ga.u(gw.craft_of(s, id), "pts") + pts : U32)), "gates", RS.as_u16((ga.u(gw.craft_of(s, id), "gates") + 1 : U32)))),
      _ => s),
    "beat",
    ga.su(
      gw.addu(gw.addu(J.get(s, "beat"), "pts", U32.min(pts, griffinwing.things.GATE())), "base", griffinwing.things.GATE()),
      "gates", RS.as_u16((ga.u(J.get(s, "beat"), "gates") + 1 : U32))))

# Rock met: a blow, and the craft is spared a moment.
def griffinwing.things.Session.scrape(+s: J.Val, +id: U32, +at: U32, +x: U32, +z: U32) -> J.Val:
  griffinwing.things.Session.scrape.hit(griffinwing.combat.Session.hurt_craft(s, id, griffinwing.things.SCRAPE()), id, at, x, z)

def griffinwing.things.Session.scrape.hit(+r: J.Val, +id: U32, +at: U32, +x: U32, +z: U32) -> J.Val:
  gw.ifj(J.bool(gw.out_r(r)),
    _ => griffinwing.Session.sound(griffinwing.Session.spark(gw.out_s(r), "Scrape", at, x, z, U32.and(id, 255)), "rock_land"),
    _ => gw.out_s(r))
