# crates/delve/src/rules/griffinwing/script.rs — the three beats' scripts: what
# appears, and when, along the rail. The tables themselves are in
# script_table.bend (generated from the Rust's builders). An act is a J object
# {"k": kind, ...fields} (the Rust enum `Act`); an event is {"at", "act"}.

def gw.ev(+at: U32, +act: J.Val) -> J.Val:
  ga.sv(ga.su(J.obj(), "at", at), "act", act)

def gw.act_wisps(+n: U32, +plus: U32, +shape: U32, +x: U32, +z: U32, +mode: U32) -> J.Val:
  +a0 = ga.ss(J.obj(), "k", "Wisps")
  +a1 = ga.su(a0, "n", n)
  +a2 = ga.su(a1, "plus", plus)
  +a3 = ga.su(a2, "shape", shape)
  +a4 = ga.si(a3, "x", x)
  +a5 = ga.si(a4, "z", z)
  ga.su(a5, "mode", mode)

def gw.act_convoy(+n: U32, +plus: U32, +from: U32, +z: U32) -> J.Val:
  ga.si(ga.si(ga.su(ga.su(ga.ss(J.obj(), "k", "Convoy"), "n", n), "plus", plus), "from", from), "z", z)

def gw.act_hunter(+side: U32) -> J.Val:
  ga.si(ga.ss(J.obj(), "k", "Hunter"), "side", side)

def gw.act_turret(+x: U32, +z: U32) -> J.Val:
  ga.si(ga.si(ga.ss(J.obj(), "k", "Turret"), "x", x), "z", z)

def gw.act_ridge(+x: U32, +bombers: U32, +plus: U32) -> J.Val:
  ga.su(ga.su(ga.si(ga.ss(J.obj(), "k", "Ridge"), "x", x), "bombers", bombers), "plus", plus)

def gw.act_gate(+rel: U32, +x: U32, +z: U32, +bonus: Bool) -> J.Val:
  ga.sb(ga.si(ga.si(ga.si(ga.ss(J.obj(), "k", "Gate"), "rel", rel), "x", x), "z", z), "bonus", bonus)

def gw.act_ring(+rel: U32, +x: U32, +z: U32) -> J.Val:
  ga.si(ga.si(ga.si(ga.ss(J.obj(), "k", "Ring"), "rel", rel), "x", x), "z", z)

def gw.act_arch(+rel: U32, +x: U32, +z: U32, +w: U32, +h: U32) -> J.Val:
  ga.si(ga.si(ga.si(ga.si(ga.si(ga.ss(J.obj(), "k", "Arch"), "rel", rel), "x", x), "z", z), "w", w), "h", h)

def gw.act_shutter(+rel: U32, +phase: U32) -> J.Val:
  ga.su(ga.si(ga.ss(J.obj(), "k", "Shutter"), "rel", rel), "phase", phase)

def gw.act_say(+line: String) -> J.Val:
  ga.ss(ga.ss(J.obj(), "k", "Say"), "line", line)

def gw.act_beast() -> J.Val:
  ga.ss(J.obj(), "k", "Beast")

# How many ticks the rail takes to carry a thing from where it appears to the
# formation.
def griffinwing.script.LEAD_TICKS() -> U32:
  U32.div(griffinwing.types.LEAD(), griffinwing.types.PACE())

# The scripts, built once (here: by stage).
def griffinwing.script.list(+stage: U32) -> List<&2, J.Val>:
  gw.ifjl(U32.is_eq(U32.min(stage, 2), 0), _ => griffinwing.script.thunderhead(),
    _ => gw.ifjl(U32.is_eq(U32.min(stage, 2), 1), _ => griffinwing.script.causeway(), _ => griffinwing.script.roost()))

def griffinwing.script.script(+stage: U32) -> J.Val:
  J.JA{griffinwing.script.list(stage)}

# Where a beat's script stands when it comes back to the tick `cp`: how many of
# its events come before it.
def griffinwing.script.resume(+stage: U32, +cp: U32) -> U32:
  griffinwing.script.resume.go(griffinwing.script.list(stage), cp)

def griffinwing.script.resume.go(+es: List<&2, J.Val>, +cp: U32) -> U32:
  match es:
    case Nil{}:
      0
    case Con{+e, +t}:
      gw.ifu(U32.is_lt(ga.u(e, "at"), cp), _ => (1 + griffinwing.script.resume.go(t, cp) : U32), _ => 0)

# Run the events that are due.
def griffinwing.script.Session.run_script(+s: J.Val, +n: J.Val) -> J.Val:
  griffinwing.script.Session.run_script.go(s, griffinwing.script.script(ga.u(s, "stage")))

def griffinwing.script.Session.run_script.go(+s: J.Val, +script: J.Val) -> J.Val:
  griffinwing.script.Session.run_script.ev(s, script, J.nth(script, ga.u(s, "next")))

def griffinwing.script.Session.run_script.ev(+s: J.Val, +script: J.Val, +ev: J.Val) -> J.Val:
  gw.ifj(Bool.or(J.is_null(ev), U32.is_gt(ga.u(ev, "at"), ga.u(s, "t"))),
    _ => s,
    _ => griffinwing.script.Session.run_script.go(
      griffinwing.script.Session.act(ga.su(s, "next", U32.and((ga.u(s, "next") + 1 : U32), 65535)), J.get(ev, "act")),
      script))

# A centred spread: the `k`th of `count`, in half steps.
def griffinwing.script.Session.spread(+k: U32, +count: U32) -> U32:
  ((2 * k : U32) - (count - 1 : U32) : U32)

# How many of a wave: `n + (seats - 1) * plus / 2`.
def griffinwing.script.Session.count(+s: J.Val, +n: U32, +plus: U32) -> U32:
  (n + gw.div(((griffinwing.Session.pressure(s) - 1 : U32) * plus : U32), 2) : U32)

def griffinwing.script.Session.act(+s: J.Val, +act: J.Val) -> J.Val:
  griffinwing.script.Session.act.kind(s, act, ga.s(act, "k"), (griffinwing.Session.anchor(s) + griffinwing.types.LEAD() : U32), griffinwing.rail.ORIGIN(U32.min(ga.u(s, "stage"), 2)))

def griffinwing.script.Session.act.kind(+s: J.Val, +act: J.Val, +k: String, +far: U32, +origin: U32) -> J.Val:
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
def griffinwing.script.Session.act.wisps(+s: J.Val, +act: J.Val, +far: U32) -> J.Val:
  griffinwing.script.Session.act.wisps.pin(
    griffinwing.script.Session.act.wisps.go(s, act, far, 0, griffinwing.script.Session.count(s, ga.u(act, "n"), ga.u(act, "plus"))),
    ga.u(act, "mode"))

# if mode == 3 { for f in foes where f.mode == 3 && f.ticks == 0 { f.ticks = 60 } }
def griffinwing.script.Session.act.wisps.pin(+s: J.Val, +mode: U32) -> J.Val:
  gw.ifj(U32.is_eq(mode, 3),
    _ => ga.sv(s, "foes", J.JA{griffinwing.script.Session.act.wisps.pin.go(ga.items(s, "foes"))}),
    _ => s)

def griffinwing.script.Session.act.wisps.pin.go(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Con{
        gw.ifj(Bool.and(U32.is_eq(ga.u(f, "mode"), 3), U32.is_zero(ga.u(f, "ticks"))), _ => ga.su(f, "ticks", 60), _ => f),
        griffinwing.script.Session.act.wisps.pin.go(t)}

def griffinwing.script.Session.act.wisps.go(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +count: U32) -> J.Val:
  gw.ifj(gw.lt(k, count),
    _ => griffinwing.script.Session.act.wisps.go(
      griffinwing.script.Session.act.wisps.one(s, act, far, k, griffinwing.script.Session.spread(k, count)),
      act, far, (k + 1 : U32), count),
    _ => s)

# The wisp's offset by shape: (dx, dz, ds, vx).
def griffinwing.script.Session.act.wisps.one(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +sp: U32) -> J.Val:
  griffinwing.script.Session.act.wisps.spawn(s, act, far, k, ga.u(act, "shape"), sp)

def griffinwing.script.Session.act.wisps.spawn(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +shape: U32, +sp: U32) -> J.Val:
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

def griffinwing.script.Session.act.wisps.side(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +side: U32) -> J.Val:
  griffinwing.script.Session.act.wisps.put(s, act, far, k, (side * griffinwing.types.dm(95) : U32),
    (gw.rem(k, 3) * griffinwing.types.dm(10) : U32), (k * griffinwing.types.dm(22) : U32), gw.neg((side * 44 : U32)))

def griffinwing.script.Session.act.wisps.put(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +dx: U32, +dz: U32, +ds: U32, +vx: U32) -> J.Val:
  gw.out_s(griffinwing.combat.Session.spawn_foe(s, "Wisp",
    (far + ds : U32), (griffinwing.types.dm(ga.u(act, "x")) + dx : U32), (griffinwing.types.dm(ga.u(act, "z")) + dz : U32),
    gw.neg(15), vx, 0, "Fly", ga.u(act, "mode"), U32.and((k * 7 : U32), 255)))

# Kites crossing in a file from one side.
def griffinwing.script.Session.act.convoy(+s: J.Val, +act: J.Val, +far: U32) -> J.Val:
  griffinwing.script.Session.act.convoy.go(s, act, far, 0, griffinwing.script.Session.count(s, ga.u(act, "n"), ga.u(act, "plus")))

def griffinwing.script.Session.act.convoy.go(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +count: U32) -> J.Val:
  gw.ifj(gw.lt(k, count),
    _ => griffinwing.script.Session.act.convoy.go(
      gw.out_s(griffinwing.combat.Session.spawn_foe(s, "Kite",
        (far + (k * griffinwing.types.dm(26) : U32) : U32), (ga.u(act, "from") * griffinwing.types.dm(95) : U32), griffinwing.types.dm(ga.u(act, "z")),
        gw.neg(15), gw.neg((ga.u(act, "from") * 46 : U32)), 0, "Fly", 0, U32.and((k * 5 : U32), 255))),
      act, far, (k + 1 : U32), count),
    _ => s)

# A kite that hunts a wingman and latches onto its tail.
def griffinwing.script.Session.act.hunter(+s: J.Val, +act: J.Val, +far: U32) -> J.Val:
  gw.out_s(griffinwing.combat.Session.spawn_foe(s, "Kite",
    (far - griffinwing.types.dm(40) : U32), (ga.u(act, "side") * griffinwing.types.dm(90) : U32), griffinwing.types.dm(40),
    0, 0, 0, "Hunt", 1, 0))

# A lantern tower on a ledge.
def griffinwing.script.Session.act.turret(+s: J.Val, +act: J.Val, +far: U32) -> J.Val:
  griffinwing.script.Session.act.turret.set(
    griffinwing.combat.Session.spawn_foe(s, "Turret", far, griffinwing.types.dm(ga.u(act, "x")), griffinwing.types.dm(ga.u(act, "z")),
      0, 0, 0, "Rest", 0, U32.mod(ga.u(s, "next"), 5)))

def griffinwing.script.Session.act.turret.set(+r: J.Val) -> J.Val:
  gw.ifj(J.is_null(gw.out_r(r)),
    _ => gw.out_s(r),
    _ => griffinwing.script.Session.act.turret.ticks(gw.out_s(r), griffinwing.combat.Session.bombers_ram.at(ga.items(gw.out_s(r), "foes"), J.u32(gw.out_r(r)), 0)))

def griffinwing.script.Session.act.turret.ticks(+s: J.Val, +i: U32) -> J.Val:
  gw.ifj(U32.is_lt(i, gw.len(s, "foes")),
    _ => gw.put_item(s, "foes", i, ga.su(gw.item(s, "foes", i), "ticks", 30)),
    _ => s)

def griffinwing.types.Prop.make(+thing: String, +s: U32, +x: U32, +z: U32, +w: U32, +h: U32, +len: U32, +phase: U32) -> J.Val:
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
def gw.push_prop(+s: J.Val, +p: J.Val) -> J.Val:
  gw.ifj(U32.is_lt(gw.len(s, "props"), griffinwing.types.MAX_PROPS()), _ => ga.push(s, "props", p), _ => s)

# A ridge with dive-bombers waiting behind it.
def griffinwing.script.Session.act.ridge(+s: J.Val, +act: J.Val, +far: U32) -> J.Val:
  griffinwing.script.Session.act.ridge.bombers(
    gw.push_prop(s, griffinwing.types.Prop.make("Ridge", far, griffinwing.types.dm(ga.u(act, "x")), gw.neg(griffinwing.types.dm(30)),
      griffinwing.types.dm(30), griffinwing.types.dm(55), griffinwing.types.dm(60), 0)),
    act, far, 0, griffinwing.script.Session.count(s, ga.u(act, "bombers"), ga.u(act, "plus")))

def griffinwing.script.Session.act.ridge.bombers(+s: J.Val, +act: J.Val, +far: U32, +k: U32, +count: U32) -> J.Val:
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
def griffinwing.script.Session.act.gate(+s: J.Val, +act: J.Val, +origin: U32) -> J.Val:
  gw.ifj(U32.is_lt(gw.len(s, "props"), griffinwing.types.MAX_PROPS()),
    _ => griffinwing.script.Session.act.gate.offer(
      ga.push(s, "props", griffinwing.types.Prop.make("Gate", (origin + ga.u(act, "rel") : U32), griffinwing.types.dm(ga.u(act, "x")), griffinwing.types.dm(ga.u(act, "z")),
        griffinwing.types.dm(20), 0, 0, gw.ifu(ga.b(act, "bonus"), _ => 0, _ => 1))),
      ga.b(act, "bonus")),
    _ => s)

def griffinwing.script.Session.act.gate.offer(+s: J.Val, +bonus: Bool) -> J.Val:
  gw.ifj(bonus,
    _ => ga.sv(s, "beat", gw.addu(J.get(s, "beat"), "offered", (100 * griffinwing.Session.pressure(s) : U32))),
    _ => s)

def griffinwing.script.Session.act.ring(+s: J.Val, +act: J.Val, +origin: U32) -> J.Val:
  gw.push_prop(s, griffinwing.types.Prop.make("Ring", (origin + ga.u(act, "rel") : U32), griffinwing.types.dm(ga.u(act, "x")), griffinwing.types.dm(ga.u(act, "z")),
    griffinwing.types.dm(22), 0, 0, 0))

def griffinwing.script.Session.act.arch(+s: J.Val, +act: J.Val, +origin: U32) -> J.Val:
  gw.push_prop(s, griffinwing.types.Prop.make("Arch", (origin + ga.u(act, "rel") : U32), griffinwing.types.dm(ga.u(act, "x")), griffinwing.types.dm(ga.u(act, "z")),
    griffinwing.types.dm(ga.u(act, "w")), griffinwing.types.dm(ga.u(act, "h")), griffinwing.types.dm(30), 0))

def griffinwing.script.Session.act.shutter(+s: J.Val, +act: J.Val, +origin: U32) -> J.Val:
  gw.push_prop(s, griffinwing.types.Prop.make("Shutter", (origin + ga.u(act, "rel") : U32), 0, 0, 0, 0, griffinwing.types.dm(30), ga.u(act, "phase")))

# The warm fly-through after the Wyrm: a procession of gold gates and hearth
# rings, lit for the way home. It pays nothing and offers nothing.
def griffinwing.script.Session.victory_gates(+s: J.Val) -> J.Val:
  griffinwing.script.Session.victory_gates.go(s, (griffinwing.Session.anchor(s) + griffinwing.types.dm(150) : U32), 0)

def griffinwing.script.Session.victory_gates.go(+s: J.Val, +origin: U32, +k: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, 9),
    _ => griffinwing.script.Session.victory_gates.go(
      gw.push_prop(s, griffinwing.types.Prop.make(gw.ifs(U32.is_eq(U32.mod(k, 3), 2), _ => "Ring", _ => "Gate"),
        (origin + (k * griffinwing.types.dm(75) : U32) : U32),
        griffinwing.types.tri((k * 40 : U32), 120, griffinwing.types.dm(30)),
        (griffinwing.types.dm(30) + griffinwing.types.tri((k * 25 : U32), 90, griffinwing.types.dm(18)) : U32),
        griffinwing.types.dm(24), 0, 0, 1)),
      origin, (k + 1 : U32)),
    _ => s)
