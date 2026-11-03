# crates/delve/src/rules/griffinwing/boss.rs — the Gloam Wyrm: three lamps in
# cold-light shields, three phases. A turned bolt opens the lit lamp's shield;
# a charged lock breaks the lamp open; its flame goes out and the Wyrm reels
# into the next phase.

# How far ahead of the formation the Wyrm's head holds.
def griffinwing.boss.HEAD() -> U32:
  griffinwing.types.dm(340)

# How long a lamp's shield stays open after a turned bolt, in ticks.
def griffinwing.boss.OPEN() -> U32:
  150

def griffinwing.boss.RISE_T() -> U32:
  90

def griffinwing.boss.HURT_T() -> U32:
  70

# The lamps' places about the head, across and up.
def griffinwing.boss.LAMP.x(+k: U32) -> U32:
  Bool.pick(U32, U32.is_eq(k, 0), gw.neg(griffinwing.types.dm(45)), Bool.pick(U32, U32.is_eq(k, 1), griffinwing.types.dm(45), 0))

def griffinwing.boss.LAMP.z(+k: U32) -> U32:
  Bool.pick(U32, U32.is_eq(k, 2), griffinwing.types.dm(42), griffinwing.types.dm(10))

# How far in front of its face the lamps hang.
def griffinwing.boss.FRONT() -> U32:
  griffinwing.types.dm(40)

# The head's height above the rail's line, resting.
def griffinwing.boss.HIGH() -> U32:
  griffinwing.types.dm(12)

# How wide the coils' gap and the roar's column are, half.
def griffinwing.boss.GAP_HALF() -> U32:
  griffinwing.types.dm(30)

def griffinwing.boss.COL_HALF() -> U32:
  griffinwing.types.dm(24)

def griffinwing.boss.COIL_T() -> U32:
  22

def griffinwing.boss.ROAR_T() -> U32:
  26

# What a coil or the roar does to a craft it catches.
def griffinwing.boss.BLOW() -> U32:
  35

# The Wyrm rises out of the roost (or, after a fall, is back in its place, its
# lamps as they were).
def griffinwing.boss.Session.rise(+s0: J.Val) -> J.Val:
  griffinwing.boss.Session.rise.go(ga.su(s0, "cp", griffinwing.rail.BOSS_AT()))

def griffinwing.boss.Session.rise.go(+s: J.Val) -> J.Val:
  gw.ifj(Bool.not(J.is_null(J.get(s, "boss"))),
    _ => ga.sv(s, "boss", ga.su(gw.sets(ga.sv(J.get(s, "boss"), "marks", J.JA{[]}), "st", "Rise"), "ticks", griffinwing.boss.RISE_T())),
    _ => griffinwing.boss.Session.rise.make(s))

# Lamp `k`, spawned: the new session in {"s", "r"}; the lamps' ids in `ids`.
def griffinwing.boss.Session.rise.make(+s: J.Val) -> J.Val:
  griffinwing.boss.Session.rise.fin(
    griffinwing.boss.Session.rise.lamp(
      griffinwing.boss.Session.rise.lamp(
        griffinwing.boss.Session.rise.lamp(J.set(J.set(J.obj(), "s", s), "ids", J.JA{[]}), 0),
        1),
      2))

def griffinwing.boss.Session.rise.lamp(+w: J.Val, +k: U32) -> J.Val:
  griffinwing.boss.Session.rise.lamp2(gw.ps(w), J.get(w, "ids"),
    griffinwing.combat.Session.spawn_foe(gw.ps(w), "Lamp",
      (griffinwing.Session.anchor(gw.ps(w)) + griffinwing.boss.HEAD() - griffinwing.boss.FRONT() : U32),
      griffinwing.boss.LAMP.x(k), (griffinwing.boss.HIGH() + griffinwing.boss.LAMP.z(k) : U32),
      0, 0, 0, "Shut", k, 0))

def griffinwing.boss.Session.rise.lamp2(+s0: J.Val, +ids: J.Val, +r: J.Val) -> J.Val:
  J.set(J.set(J.obj(), "s", gw.out_s(r)), "ids", J.push(ids, gw.ifj(J.is_null(gw.out_r(r)), _ => J.JU{0}, _ => gw.out_r(r))))

def griffinwing.boss.Session.rise.fin(+w: J.Val) -> J.Val:
  griffinwing.Session.sound(
    griffinwing.Session.say(
      ga.sv(gw.ps(w), "boss", griffinwing.boss.Beast.new(J.get(w, "ids"))),
      griffinwing.Session.voice(gw.ps(w)), "Wyrm"),
    "boss_rise")

def griffinwing.boss.Beast.new(+lamps: J.Val) -> J.Val:
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
def griffinwing.boss.Session.rest(+s: J.Val, +phase: U32) -> U32:
  U32.max((64 - (4 * (griffinwing.Session.pressure(s) - 1 : U32) : U32) - (8 * phase : U32) : U32), 34)

# The Wyrm's tick. The Wyrm is taken out of the session while it acts.
def griffinwing.boss.Session.step_boss(+s: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(J.is_null(J.get(s, "boss")),
    _ => s,
    _ => griffinwing.boss.Session.step_boss.sway(ga.sv(s, "boss", J.JNull{}), J.get(s, "boss"), n))

# The head holds its place ahead and sways; it rises from below; the lamps
# follow it and their shields time out.
def griffinwing.boss.Session.step_boss.sway(+s: J.Val, +b0: J.Val, +n: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.lamps(s,
    ga.si(ga.si(b0, "x", griffinwing.types.tri(ga.u(s, "t"), 260, griffinwing.types.dm(24))), "z",
      (griffinwing.boss.HIGH() - gw.ifu(gw.isn(b0, "st", "Rise"), _ => gw.div((ga.u(b0, "ticks") * griffinwing.types.dm(1) : U32), 2), _ => 0) : U32)),
    n)

def griffinwing.boss.Session.step_boss.lamps(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.lit(
    griffinwing.boss.Session.step_boss.lamp(griffinwing.boss.Session.step_boss.lamp(griffinwing.boss.Session.step_boss.lamp(s, b, 0), b, 1), b, 2),
    b, n)

# Lamp `k`: placed about the head; an open shield counts down.
def griffinwing.boss.Session.step_boss.lamp(+s: J.Val, +b: J.Val, +k: U32) -> J.Val:
  griffinwing.boss.Session.step_boss.lamp2(s, b, k, griffinwing.combat.Session.bombers_ram.at(ga.items(s, "foes"), J.u32(J.nth(J.get(b, "lamps"), k)), 0))

def griffinwing.boss.Session.step_boss.lamp2(+s: J.Val, +b: J.Val, +k: U32, +i: U32) -> J.Val:
  gw.ifj(U32.is_lt(i, gw.len(s, "foes")),
    _ => gw.put_item(s, "foes", i,
      griffinwing.boss.Session.step_boss.shield(
        ga.si(ga.si(ga.si(gw.item(s, "foes", i), "s", (griffinwing.Session.anchor(s) + griffinwing.boss.HEAD() - griffinwing.boss.FRONT() : U32)),
          "x", (ga.u(b, "x") + griffinwing.boss.LAMP.x(k) : U32)), "z", (ga.u(b, "z") + griffinwing.boss.LAMP.z(k) : U32)))),
    _ => s)

def griffinwing.boss.Session.step_boss.shield(+f: J.Val) -> J.Val:
  gw.ifj(gw.isn(f, "st", "Open"),
    _ => griffinwing.boss.Session.step_boss.shield2(gw.subs(f, "ticks", 1)),
    _ => f)

def griffinwing.boss.Session.step_boss.shield2(+f: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(f, "ticks")), _ => gw.sets(f, "st", "Shut"), _ => f)

# The lit lamp is out: the Wyrm reels (or dies), and its spits are gone.
def griffinwing.boss.Session.step_boss.lit(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(Bool.and(
      griffinwing.boss.Session.step_boss.out(ga.items(s, "foes"), J.u32(J.nth(J.get(b, "lamps"), U32.min(ga.u(b, "phase"), 2)))),
      Bool.not(Bool.or(gw.isn(b, "st", "Hurt"), gw.isn(b, "st", "Dying")))),
    _ => griffinwing.boss.Session.step_boss.reel(s, b),
    _ => griffinwing.boss.Session.step_boss.act(s, b, n))

# `foes.find(|f| f.id == lit).is_some_and(|f| f.st == Broken)`
def griffinwing.boss.Session.step_boss.out(+fs: List<&2, J.Val>, +lit: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      gw.ifb(U32.is_eq(ga.u(f, "id"), lit), _ => gw.isn(f, "st", "Broken"), _ => griffinwing.boss.Session.step_boss.out(t, lit))

def griffinwing.boss.Session.step_boss.reel(+s: J.Val, +b0: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.reel2(
    griffinwing.Session.sound(ga.sv(s, "bolts", J.JA{griffinwing.boss.Session.step_boss.owned(ga.items(s, "bolts"))}), "boss_roar"),
    ga.sv(b0, "marks", J.JA{[]}))

# bolts.retain(|x| x.owner != 0)
def griffinwing.boss.Session.step_boss.owned(+bs: List<&2, J.Val>) -> List<&2, J.Val>:
  match bs:
    case Nil{}:
      Nil{}
    case Con{+b, +t}:
      gw.ifjl(U32.is_zero(ga.u(b, "owner")),
        _ => griffinwing.boss.Session.step_boss.owned(t),
        _ => Con{b, griffinwing.boss.Session.step_boss.owned(t)})

def griffinwing.boss.Session.step_boss.reel2(+s: J.Val, +b: J.Val) -> J.Val:
  ga.sv(s, "boss",
    gw.ifj(U32.is_ge(ga.u(b, "phase"), 2),
      _ => ga.su(gw.sets(b, "st", "Dying"), "ticks", griffinwing.DYING()),
      _ => ga.su(gw.sets(b, "st", "Hurt"), "ticks", griffinwing.boss.HURT_T())))

# The attack pattern of the phase: [0], [1, 0], [2, 0, 1, 0].
def griffinwing.boss.Session.step_boss.pat(+phase: U32, +i: U32) -> U32:
  gw.ifu(U32.is_eq(phase, 0), _ => 0,
    _ => gw.ifu(U32.is_eq(phase, 1), _ => Bool.pick(U32, U32.is_eq(U32.mod(i, 2), 0), 1, 0),
      _ => Bool.pick(U32, U32.is_eq(U32.mod(i, 4), 0), 2, Bool.pick(U32, U32.is_eq(U32.mod(i, 4), 2), 1, 0))))

# The Wyrm's state: what it does this tick.
def griffinwing.boss.Session.step_boss.act(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
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

def griffinwing.boss.Session.step_boss.rise(+s: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(b, "ticks")),
    _ => ga.sv(griffinwing.Session.say(s, griffinwing.Session.voice(s), "Reflect"), "boss", ga.su(gw.sets(b, "st", "Idle"), "ticks", 50)),
    _ => ga.sv(s, "boss", b))

def griffinwing.boss.Session.step_boss.idle(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(b, "ticks")),
    _ => griffinwing.boss.Session.step_boss.idle2(s,
      ga.su(ga.su(b, "atk", griffinwing.boss.Session.step_boss.pat(ga.u(b, "phase"), ga.u(b, "seq"))), "seq", U32.and((ga.u(b, "seq") + 1 : U32), 255)), n),
    _ => ga.sv(s, "boss", b))

def griffinwing.boss.Session.step_boss.idle2(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.put(griffinwing.boss.Session.begin_tell(s, b, n))

# A pair {"s", "b"} put back as the session with its Wyrm.
def griffinwing.boss.Session.step_boss.put(+r: J.Val) -> J.Val:
  ga.sv(J.get(r, "s"), "boss", J.get(r, "b"))

def griffinwing.boss.Session.step_boss.tell(+s: J.Val, +b: J.Val) -> J.Val:
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
def griffinwing.boss.Session.step_boss.sweep(+s: J.Val, +b: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.sweep.done(
    griffinwing.boss.Session.step_boss.sweep.each(s, griffinwing.boss.Session.flying_ids(s, ga.ids(J.get(s, "seats"))), ga.u(b, "gap"), ga.u(b, "col"), gw.isn(b, "st", "Coil")),
    gw.subs(b, "ticks", 1))

# The ids of the knights flying, in seat order.
def griffinwing.boss.Session.flying_ids(+s: J.Val, +ids: List<&2, U32>) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gw.ifl(griffinwing.types.Craft.flying(gw.craft_of(s, id)),
        _ => Con{id, griffinwing.boss.Session.flying_ids(s, t)},
        _ => griffinwing.boss.Session.flying_ids(s, t))

def griffinwing.boss.Session.step_boss.sweep.each(+s: J.Val, +ids: List<&2, U32>, +gap: U32, +col: U32, +coil: Bool) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.boss.Session.step_boss.sweep.each(
        griffinwing.boss.Session.step_boss.sweep.one(s, id, gap, col, coil, ga.u(gw.craft_of(s, id), "x")),
        t, gap, col, coil)

def griffinwing.boss.Session.step_boss.sweep.one(+s: J.Val, +id: U32, +gap: U32, +col: U32, +coil: Bool, +x: U32) -> J.Val:
  gw.ifj(gw.ifb(coil,
      _ => gw.gt(gw.abs((x - gap : U32)), (griffinwing.boss.GAP_HALF() - gw.div(griffinwing.flight.CRAFT_R(), 2) : U32)),
      _ => gw.le(gw.abs((x - col : U32)), (griffinwing.boss.COL_HALF() + gw.div(griffinwing.flight.CRAFT_R(), 2) : U32))),
    _ => gw.out_s(griffinwing.combat.Session.hurt_craft(s, id, griffinwing.boss.BLOW())),
    _ => s)

def griffinwing.boss.Session.step_boss.sweep.done(+s: J.Val, +b: J.Val) -> J.Val:
  ga.sv(s, "boss",
    gw.ifj(U32.is_zero(ga.u(b, "ticks")),
      _ => ga.sv(ga.su(gw.sets(b, "st", "Idle"), "ticks", griffinwing.boss.Session.rest(s, ga.u(b, "phase"))), "marks", J.JA{[]}),
      _ => b))

# A lamp went out: it reels, then the next phase, and the Wyrm calls kites.
def griffinwing.boss.Session.step_boss.hurt(+s: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(b, "ticks")),
    _ => griffinwing.boss.Session.step_boss.hurt2(s, ga.su(b, "phase", (ga.u(b, "phase") + 1 : U32))),
    _ => ga.sv(s, "boss", b))

def griffinwing.boss.Session.step_boss.hurt2(+s: J.Val, +b0: J.Val) -> J.Val:
  griffinwing.boss.Session.step_boss.hurt3(s,
    ga.su(ga.su(gw.sets(b0, "st", "Idle"), "ticks", (griffinwing.boss.Session.rest(s, ga.u(b0, "phase")) + 20 : U32)), "seq", 0))

def griffinwing.boss.Session.step_boss.hurt3(+s: J.Val, +b: J.Val) -> J.Val:
  ga.sv(
    griffinwing.boss.Session.step_boss.hunters(
      griffinwing.Session.say(s, griffinwing.Session.voice(s), gw.ifs(U32.is_eq(ga.u(b, "phase"), 1), _ => "Coils", _ => "Roar")),
      0, (1 + U32.div(griffinwing.Session.pressure(s), 3) : U32)),
    "boss", b)

# for k in 0..hunters { act(Hunter { side: if k % 2 == 0 { -1 } else { 1 } }) }
def griffinwing.boss.Session.step_boss.hunters(+s: J.Val, +k: U32, +n: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, n),
    _ => griffinwing.boss.Session.step_boss.hunters(
      griffinwing.script.Session.act(s, ga.si(ga.ss(J.obj(), "k", "Hunter"), "side", gw.ifu(U32.is_eq(U32.mod(k, 2), 0), _ => gw.neg(1), _ => 1))),
      (k + 1 : U32), n),
    _ => s)

def griffinwing.boss.Session.step_boss.dying(+s: J.Val, +b: J.Val) -> J.Val:
  ga.sv(
    gw.ifj(U32.is_zero(U32.mod(ga.u(b, "ticks"), 12)),
      _ => griffinwing.Session.spark(s, "Pop", (griffinwing.Session.anchor(s) + griffinwing.boss.HEAD() : U32), ga.u(b, "x"), ga.u(b, "z"), 0),
      _ => s),
    "boss", b)

# An attack's footprints are locked and shown for the tell's ticks. {"s", "b"}.
def griffinwing.boss.Session.begin_tell(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(J.is_null(griffinwing.combat.Session.target(s, 0)),
    _ => J.set(J.set(J.obj(), "s", s), "b", ga.su(gw.sets(b, "st", "Idle"), "ticks", 30)),
    _ => griffinwing.boss.Session.begin_tell.kind(s, ga.sv(b, "marks", J.JA{[]}), n))

def griffinwing.boss.Session.begin_tell.kind(+s: J.Val, +b: J.Val, +n: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell.fin(
    gw.ifj(U32.is_zero(ga.u(b, "atk")),
      _ => griffinwing.boss.Session.begin_tell.spit(s, b),
      _ => gw.ifj(U32.is_eq(ga.u(b, "atk"), 1),
        _ => griffinwing.boss.Session.begin_tell.coil(s, b, n),
        _ => griffinwing.boss.Session.begin_tell.roar(s, b))),
    n)

def griffinwing.boss.Session.begin_tell.fin(+r: J.Val, +n: J.Val) -> J.Val:
  J.set(J.set(J.obj(), "s", griffinwing.Session.sound(J.get(r, "s"), "vent_fire")), "b",
    ga.su(gw.sets(J.get(r, "b"), "st", "Tell"), "ticks", U32.max(ga.u(n, "tell"), 40)))

# The spit: a footprint for each knight asked for, a knight asked for twice is
# aimed at to either side.
def griffinwing.boss.Session.begin_tell.spit(+s: J.Val, +b: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell.say(s,
    griffinwing.boss.Session.begin_tell.marks(s, b, 0,
      U32.min((2 + U32.div((griffinwing.Session.pressure(s) - 1 : U32), 2) + ga.u(b, "phase") : U32), 7)),
    b)

def griffinwing.boss.Session.begin_tell.marks(+s: J.Val, +b: J.Val, +k: U32, +count: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, count),
    _ => griffinwing.boss.Session.begin_tell.mark(s, b, k, count, griffinwing.combat.Session.target(s, (ga.u(b, "seq") + k : U32))),
    _ => b)

def griffinwing.boss.Session.begin_tell.mark(+s: J.Val, +b: J.Val, +k: U32, +count: U32, +to: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell.marks(s,
    gw.ifj(J.is_null(to),
      _ => b,
      _ => ga.push(b, "marks",
        ga.pair_i(
          (ga.nu(to, 0) + gw.ifu(U32.is_eq(U32.mod(k, 2), 0), _ => griffinwing.boss.Session.begin_tell.jitter(s, k), _ => gw.neg(griffinwing.boss.Session.begin_tell.jitter(s, k))) : U32),
          ga.nu(to, 1)))),
    (k + 1 : U32), count)

def griffinwing.boss.Session.begin_tell.jitter(+s: J.Val, +k: U32) -> U32:
  (U32.div(k, U32.max(gw.nseats(s), 1)) * griffinwing.types.dm(16) : U32)

def griffinwing.boss.Session.begin_tell.say(+s: J.Val, +b: J.Val, +unused: J.Val) -> J.Val:
  J.set(J.set(J.obj(), "s",
    gw.ifj(Bool.and(U32.is_zero(ga.u(b, "phase")), U32.is_eq(ga.u(b, "seq"), 1)),
      _ => griffinwing.Session.say(s, griffinwing.Session.voice(s), "Reflect"),
      _ => s)),
    "b", b)

# The coil: the gap it sweeps from the side its turn names.
def griffinwing.boss.Session.begin_tell.coil(+s: J.Val, +b0: J.Val, +n: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell.coil2(s,
    ga.si(b0, "side", gw.ifu(U32.is_eq(U32.mod(ga.u(b0, "seq"), 2), 0), _ => gw.neg(1), _ => 1)),
    n, gw.ifu(J.is_null(griffinwing.combat.Session.target(s, ga.u(b0, "seq"))), _ => 0, _ => ga.nu(griffinwing.combat.Session.target(s, ga.u(b0, "seq")), 0)))

def griffinwing.boss.Session.begin_tell.coil2(+s: J.Val, +b: J.Val, +n: J.Val, +gap: U32) -> J.Val:
  griffinwing.boss.Session.begin_tell.coil3(s, b,
    gw.clamp(gap, (gw.neg(griffinwing.Nums.wx(n)) + gw.div(griffinwing.boss.GAP_HALF(), 2) : U32), (griffinwing.Nums.wx(n) - gw.div(griffinwing.boss.GAP_HALF(), 2) : U32)))

def griffinwing.boss.Session.begin_tell.coil3(+s: J.Val, +b: J.Val, +gap: U32) -> J.Val:
  J.set(J.set(J.obj(), "s", s), "b", ga.push(ga.si(b, "gap", gap), "marks", ga.pair_i(gap, 0)))

# The roar: a column where the next knight in turn flies.
def griffinwing.boss.Session.begin_tell.roar(+s: J.Val, +b: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell.roar2(s, b,
    gw.ifu(J.is_null(griffinwing.combat.Session.target(s, (ga.u(b, "seq") + 1 : U32))), _ => 0, _ => ga.nu(griffinwing.combat.Session.target(s, (ga.u(b, "seq") + 1 : U32)), 0)))

def griffinwing.boss.Session.begin_tell.roar2(+s: J.Val, +b: J.Val, +col: U32) -> J.Val:
  J.set(J.set(J.obj(), "s", s), "b", ga.push(ga.si(b, "col", col), "marks", ga.pair_i(col, 0)))

# The Wyrm's spit: a Gloam bolt for each locked mark, from its mouth, each
# owing its return to the lit lamp.
def griffinwing.boss.Session.spit(+s: J.Val, +b: J.Val) -> J.Val:
  griffinwing.Session.sound(
    griffinwing.boss.Session.spit.each(s, b, ga.items(b, "marks"), J.u32(J.nth(J.get(b, "lamps"), U32.min(ga.u(b, "phase"), 2)))),
    "shot_crossbow")

def griffinwing.boss.Session.spit.each(+s: J.Val, +b: J.Val, +ms: List<&2, J.Val>, +lit: U32) -> J.Val:
  match ms:
    case Nil{}:
      s
    case Con{+m, +t}:
      griffinwing.boss.Session.spit.each(
        griffinwing.boss.Session.spit.one(s, b, lit, griffinwing.combat.Session.plane_shot(s, (griffinwing.Session.anchor(s) + griffinwing.boss.HEAD() : U32), ga.u(b, "x"), ga.u(b, "z"), ga.nu(m, 0), ga.nu(m, 1), griffinwing.types.dm(5))),
        b, t, lit)

def griffinwing.boss.Session.spit.one(+s: J.Val, +b: J.Val, +lit: U32, +v: GwV3) -> J.Val:
  griffinwing.flight.Session.push_bolt(s,
    griffinwing.types.Bolt.make("Gloam", 0, (griffinwing.Session.anchor(s) + griffinwing.boss.HEAD() : U32), ga.u(b, "x"), ga.u(b, "z"),
      gw.v3a(v), gw.v3b(v), gw.v3c(v), 25, 300, 0, lit, 0, False{}))
