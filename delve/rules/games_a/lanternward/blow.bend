# crates/delve/src/rules/lanternward/blow.rs — what blows do, and every bolt's
# flight. `Hit` is "Melee" | "Bolt" | "Beam" | "Low"; `Blow` is the string
# "Hit" | "Blocked" | "Parried" | "Dodged", left in `$ret` by `blow`.

# A blow lands on knight `id`, from (fx, fy), for `dmg` quarter hearts.
def lanternward.blow.Session.blow(+s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32, +kind: String) -> J.Val:
  lanternward.blow.Session.blow.look(ga.has_key(J.get(s, "seats"), id), s, id, dmg, fx, fy, kind)

def lanternward.blow.Session.blow.look(has: Bool, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32, +kind: String) -> J.Val:
  match has:
    case False{}:
      ga.ss(s, "$ret", "Dodged")
    case True{}:
      lanternward.blow.Session.blow.gate(ga.at(J.get(s, "seats"), id), s, id, dmg, fx, fy, kind)

def lanternward.blow.Session.blow.gate(+k: J.Val, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32, +kind: String) -> J.Val:
  lanternward.blow.Session.blow.gate.go(
    Bool.or(
      Bool.or(Bool.not(lanternward.kit.Seat.up(k)), U32.is_gt(ga.u(k, "inv"), 0)),
      Bool.or(
        Bool.or(lanternward.is(k, "act", "Item"), lanternward.is(k, "act", "Fall")),
        Bool.and(String.eq(kind, "Low"), RS.i_ge(ga.u(k, "z"), 1280)))),
    k, s, id, dmg, fx, fy)

def lanternward.blow.Session.blow.gate.go(dodged: Bool, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  match dodged:
    case True{}:
      ga.ss(s, "$ret", "Dodged")
    case False{}:
      lanternward.blow.Session.blow.aim(
        lanternward.world.unit((fx - ga.u(k, "x") : U32), (fy - ga.u(k, "y") : U32)), k, s, id, dmg, fx, fy)

def lanternward.blow.Session.blow.aim(toward: U32 & U32, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  (+tx, +ty) = toward
  lanternward.blow.Session.blow.guard(
    Bool.and(
      Bool.and(U32.is_gt(ga.u(k, "shield"), 0), lanternward.is(k, "act", "Free")),
      RS.i_ge(lanternward.world.dot4(ga.nu(ga.v(k, "face"), 0), ga.nu(ga.v(k, "face"), 1), tx, ty), 108)),
    tx, ty, k, s, id, dmg, fx, fy)

def lanternward.blow.Session.blow.guard(guarded: Bool, +tx: U32, +ty: U32, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  match guarded:
    case True{}:
      lanternward.blow.Session.blow.guarded(U32.is_le(ga.u(k, "shield"), lanternward.R(s, "parry")), tx, ty, k, s, id)
    case False{}:
      lanternward.blow.Session.blow.hit(k, s, id, dmg, fx, fy)

def lanternward.blow.put_seat(+s: J.Val, +id: U32, k: J.Val) -> J.Val:
  ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id, k))

# The shield met it: a parry freezes the knight and flashes; a late block
# pushes the knight back.
def lanternward.blow.Session.blow.guarded(parried: Bool, +tx: U32, +ty: U32, +k: J.Val, +s: J.Val, +id: U32) -> J.Val:
  match parried:
    case True{}:
      lanternward.blow.parry(k, s, id)
    case False{}:
      lanternward.blow.block(tx, ty, k, s, id)

def lanternward.blow.parry(+k: J.Val, +s: J.Val, +id: U32) -> J.Val:
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

def lanternward.blow.block(+tx: U32, +ty: U32, +k: J.Val, +s: J.Val, +id: U32) -> J.Val:
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
def lanternward.blow.Session.blow.hit(+k: J.Val, +s: J.Val, +id: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  lanternward.blow.hit.away(
    lanternward.world.unit((ga.u(k, "x") - fx : U32), (ga.u(k, "y") - fy : U32)), k, s, id, dmg)

def lanternward.blow.hit.away(away: U32 & U32, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32) -> J.Val:
  (+ax, +ay) = away
  lanternward.blow.hit.seat(RS.u_sat_sub(ga.u(k, "hp"), dmg), ax, ay, k, s, id, dmg)

def lanternward.blow.hit.seat(+hp: U32, +ax: U32, +ay: U32, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32) -> J.Val:
  +down = U32.is_eq(hp, 0)
  +k1 = ga.sv(ga.su(ga.su(ga.su(ga.su(k, "hp", hp), "inv", 36), "ward", 36), "t", 0), "act", J.JS{Bool.pick(String, down, "Down", "Hurt")})
  +k2 = ga.sb(ga.su(ga.su(ga.su(ga.su(ga.su(k1, "chain", 0), "step", 0), "charge", 0), "shield", 0), "lamp", 0), "queued", False{})
  +k3 = ga.sv(ga.si(k2, "z", 0), "push", ga.pair_i(RS.i_div((ax * 560 : U32), 256), RS.i_div((ay * 560 : U32), 256)))
  +k4 = ga.su(k3, "pusht", 6)
  +k5 = lanternward.blow.hit.down(down, k4)
  lanternward.blow.hit.fin(down, k5, s, id, dmg)

def lanternward.blow.hit.down(down: Bool, +k: J.Val) -> J.Val:
  match down:
    case False{}:
      k
    case True{}:
      ga.sv(ga.su(ga.sv(k, "lock", J.JNull{}), "rev", 0), "carry", J.JS{Bool.pick(String, lanternward.is(k, "carry", "Pot"), "Nothing", ga.s(k, "carry"))})

def lanternward.blow.hit.fin(down: Bool, +k: J.Val, +s: J.Val, +id: U32, +dmg: U32) -> J.Val:
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

def lanternward.blow.hit.fall(down: Bool, +s: J.Val) -> J.Val:
  match down:
    case True{}:
      lanternward.sound(s, "knight_fall")
    case False{}:
      s

# A blade, a bolt or a blast on foe `i`. `$ret` is true when it hurt.
def lanternward.blow.Session.hurt_foe(+s: J.Val, +i: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  lanternward.blow.hurt_foe.alive(lanternward.foes.Foe.alive(lanternward.el(s, "foes", i)), s, i, dmg, fx, fy)

def lanternward.blow.hurt_foe.alive(alive: Bool, +s: J.Val, +i: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  match alive:
    case False{}:
      ga.sb(s, "$ret", False{})
    case True{}:
      lanternward.blow.hurt_foe.kind(lanternward.el(s, "foes", i), s, i, dmg, fx, fy)

def lanternward.blow.hurt_foe.kind(+f: J.Val, +s: J.Val, +i: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  +x = ga.u(f, "x")
  +y = ga.u(f, "y")
  lanternward.blow.hurt_foe.code(
    Bool.pick(U32, lanternward.is(f, "kind", "Turret"), 1,
      Bool.pick(U32, Bool.and(lanternward.is(f, "kind", "Snuffer"), Bool.not(lanternward.foes.Foe.vulnerable(f))), 2, 0)),
    f, s, i, dmg, fx, fy)

def lanternward.blow.hurt_foe.code(+code: U32, +f: J.Val, +s: J.Val, +i: U32, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
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

def lanternward.blow.hurt_foe.hit(away: U32 & U32, +f: J.Val, +s: J.Val, +i: U32, +dmg: U32) -> J.Val:
  (+ax, +ay) = away
  +dm = Bool.pick(U32, ga.b(f, "open"), (dmg * 2 : U32), dmg)
  +hp = RS.u_sat_sub(ga.u(f, "hp"), dm)
  +f1 = ga.sv(ga.su(ga.su(ga.su(ga.sb(f, "open", Bool.and(ga.b(f, "open"), False{})), "hp", hp), "flinch", 8), "freeze", 3), "kb", ga.pair_i(RS.i_div((ax * 380 : U32), 256), RS.i_div((ay * 380 : U32), 256)))
  +f2 = ga.su(f1, "kbt", 5)
  +f3 = lanternward.blow.hurt_foe.wake(lanternward.is(f, "st", "Dormant"), f2)
  lanternward.blow.hurt_foe.fin(U32.is_eq(hp, 0), f3, s, i)

def lanternward.blow.hurt_foe.wake(dormant: Bool, +f: J.Val) -> J.Val:
  match dormant:
    case True{}:
      ga.su(ga.sv(f, "st", J.JS{"Drift"}), "t", 20)
    case False{}:
      f

def lanternward.blow.hurt_foe.fin(dead: Bool, +f: J.Val, +s: J.Val, +i: U32) -> J.Val:
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
def lanternward.blow.Session.light_foe(+s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.light_foe.go(
    Bool.and(lanternward.eis(s, "foes", i, "kind", "Snuffer"), lanternward.foes.Foe.alive(lanternward.el(s, "foes", i))),
    s, i)

def lanternward.blow.light_foe.go(ok: Bool, +s: J.Val, +i: U32) -> J.Val:
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

def lanternward.blow.light_foe.wake(dormant: Bool, +s: J.Val, +i: U32) -> J.Val:
  match dormant:
    case True{}:
      lanternward.es(lanternward.eset(s, "foes", i, "st", J.JS{"Drift"}), "foes", i, "t", 20)
    case False{}:
      s

# ---- bolts ----

# Fly every bolt one tick.
def lanternward.blow.Session.step_bolts(+s: J.Val) -> J.Val:
  lanternward.blow.step_bolts.retain(lanternward.blow.step_bolts.loop(s, 0))

def lanternward.blow.step_bolts.loop(+s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.step_bolts.loop.go(U32.is_lt(i, lanternward.len(s, "bolts")), s, i)

def lanternward.blow.step_bolts.loop.go(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.blow.step_bolts.loop(lanternward.blow.step_bolts.one(U32.is_eq(lanternward.eu(s, "bolts", i, "ttl"), 0), s, i), (i + 1 : U32))

def lanternward.blow.step_bolts.one(skip: Bool, +s: J.Val, +i: U32) -> J.Val:
  match skip:
    case True{}:
      s
    case False{}:
      lanternward.es(
        lanternward.blow.step_bolts.half(lanternward.blow.step_bolts.half(s, i), i),
        "bolts", i, "ttl", RS.u_sat_sub(lanternward.eu(s, "bolts", i, "ttl"), 1))

# Fast bolts go in two half steps so none skips a body.
def lanternward.blow.step_bolts.half(+s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.step_bolts.half.go(U32.is_eq(lanternward.eu(s, "bolts", i, "ttl"), 0), s, i)

def lanternward.blow.step_bolts.half.go(dead: Bool, +s: J.Val, +i: U32) -> J.Val:
  match dead:
    case True{}:
      s
    case False{}:
      lanternward.blow.Session.bolt_meets(
        lanternward.esi(
          lanternward.esi(s, "bolts", i, "x", (lanternward.eu(s, "bolts", i, "x") + RS.i_div(lanternward.eu(s, "bolts", i, "vx"), 2) : U32)),
          "bolts", i, "y", (lanternward.eu(s, "bolts", i, "y") + RS.i_div(lanternward.eu(s, "bolts", i, "vy"), 2) : U32)),
        i)

def lanternward.blow.step_bolts.retain.go(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+b, +t}:
      Bool.pick(List<&2, J.Val>, U32.is_gt(ga.u(b, "ttl"), 0),
        Con{b, lanternward.blow.step_bolts.retain.go(t)},
        lanternward.blow.step_bolts.retain.go(t))

def lanternward.blow.step_bolts.retain(+s: J.Val) -> J.Val:
  ga.sv(s, "bolts", J.JA{lanternward.blow.step_bolts.retain.go(ga.items(s, "bolts"))})

# A bolt is done: a pot breaks, light flickers, iron goes out.
def lanternward.blow.Session.bolt_ends(+s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.bolt_ends.kind(
    ga.s(lanternward.el(s, "bolts", i), "kind"), lanternward.es(s, "bolts", i, "ttl", 0),
    lanternward.eu(s, "bolts", i, "x"), lanternward.eu(s, "bolts", i, "y"))

def lanternward.blow.bolt_ends.kind(+kind: String, +s: J.Val, +x: U32, +y: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Pot"),
    lanternward.sound(
      lanternward.props.Session.scatter(lanternward.props.Session.fx(s, "Dust", x, y, 0), "Pot", x, y), "rock_land"),
    Bool.pick(J.Val, lanternward.NAME_IN(kind, ["Light", "Bright"]),
      lanternward.props.Session.fx(s, "Spark", x, y, 0),
      lanternward.props.Session.fx(s, "Dust", x, y, 0)))

# A shield turns an iron bolt: back where the knight's lock holds, else where
# the shield faces.
def lanternward.blow.Session.turn_bolt(+s: J.Val, +i: U32, +id: U32, +parried: Bool) -> J.Val:
  lanternward.blow.turn_bolt.has(ga.has_key(J.get(s, "seats"), id), s, i, id, parried)

def lanternward.blow.turn_bolt.has(has: Bool, +s: J.Val, +i: U32, +id: U32, +parried: Bool) -> J.Val:
  match has:
    case False{}:
      s
    case True{}:
      lanternward.blow.turn_bolt.lock(
        lanternward.kit.Session.lock_point(s, ga.v(ga.at(J.get(s, "seats"), id), "lock")), s, i, id, parried)

def lanternward.blow.turn_bolt.lock(+pt: J.Val, +s: J.Val, +i: U32, +id: U32, +parried: Bool) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.blow.turn_bolt.fin(
        ga.nu(ga.v(ga.at(J.get(s, "seats"), id), "face"), 0), ga.nu(ga.v(ga.at(J.get(s, "seats"), id), "face"), 1), s, i, id, parried)
    case _:
      lanternward.blow.turn_bolt.toward(
        lanternward.world.unit(
          (ga.nu(pt, 0) - ga.u(ga.at(J.get(s, "seats"), id), "x") : U32), (ga.nu(pt, 1) - ga.u(ga.at(J.get(s, "seats"), id), "y") : U32)),
        s, i, id, parried)

def lanternward.blow.turn_bolt.toward(d: U32 & U32, +s: J.Val, +i: U32, +id: U32, +parried: Bool) -> J.Val:
  (+dx, +dy) = d
  +face = ga.v(ga.at(J.get(s, "seats"), id), "face")
  +ok = RS.i_ge(lanternward.world.dot4(dx, dy, ga.nu(face, 0), ga.nu(face, 1)), 4294967232)
  lanternward.blow.turn_bolt.fin(
    Bool.pick(U32, ok, dx, ga.nu(face, 0)), Bool.pick(U32, ok, dy, ga.nu(face, 1)), s, i, id, parried)

def lanternward.blow.turn_bolt.fin(+ax: U32, +ay: U32, +s: J.Val, +i: U32, +id: U32, +parried: Bool) -> J.Val:
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
def lanternward.blow.Session.bolt_meets(+s: J.Val, +i: U32) -> J.Val:
  +c = lanternward.world.Plan.at(ga.u(s, "room"),
    lanternward.world.tile_of(lanternward.eu(s, "bolts", i, "x")), lanternward.world.tile_of(lanternward.eu(s, "bolts", i, "y")))
  lanternward.blow.bolt_meets.wall(Bool.or(U32.is_eq(c, 35), U32.is_eq(c, 0)), s, i)

def lanternward.blow.bolt_meets.wall(hit: Bool, +s: J.Val, +i: U32) -> J.Val:
  match hit:
    case True{}:
      lanternward.blow.Session.bolt_ends(s, i)
    case False{}:
      lanternward.blow.bolt_meets.props(s, i, 0)

# Things that stand in the way.
def lanternward.blow.bolt_meets.props(+s: J.Val, +i: U32, +j: U32) -> J.Val:
  lanternward.blow.bolt_meets.props.go(U32.is_lt(j, lanternward.len(s, "props")), s, i, j)

def lanternward.blow.bolt_meets.props.go(more: Bool, +s: J.Val, +i: U32, +j: U32) -> J.Val:
  match more:
    case False{}:
      lanternward.blow.bolt_meets.kind(ga.s(lanternward.el(s, "bolts", i), "kind"), s, i)
    case True{}:
      lanternward.blow.bolt_meets.prop(
        Bool.and(
          lanternward.props.Prop.blocks(lanternward.el(s, "props", j), lanternward.eu(s, "bolts", i, "x"), lanternward.eu(s, "bolts", i, "y")),
          Bool.not(lanternward.props.Prop.lets_bolts(lanternward.el(s, "props", j)))),
        s, i, j)

def lanternward.blow.bolt_meets.prop(stops: Bool, +s: J.Val, +i: U32, +j: U32) -> J.Val:
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
def lanternward.blow.bolt_meets.stop(+kind: String, +light: Bool, +s: J.Val, +j: U32) -> J.Val:
  Bool.pick(J.Val, Bool.and(light, Bool.and(String.eq(kind, "Crystal"), U32.is_eq(lanternward.eu(s, "props", j, "on"), 0))),
    lanternward.ctx.Session.light_crystal(s, j),
    Bool.pick(J.Val, Bool.and(light, Bool.and(String.eq(kind, "Brazier"), Bool.not(lanternward.props.Prop.lit(lanternward.el(s, "props", j))))),
      lanternward.sound(
        lanternward.props.Session.fx(lanternward.es(s, "props", j, "on", 1), "Light", lanternward.eu(s, "props", j, "x"), lanternward.eu(s, "props", j, "y"), 0),
        "spell_fire"),
      Bool.pick(J.Val, Bool.and(light, Bool.and(String.eq(kind, "Great"), U32.is_eq(lanternward.eu(s, "props", j, "on"), 0))),
        lanternward.props.Session.solved(lanternward.es(s, "props", j, "on", 1), lanternward.eu(s, "props", j, "x"), lanternward.eu(s, "props", j, "y")),
        s)))

def lanternward.blow.bolt_meets.kind(+kind: String, +s: J.Val, +i: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Iron"),
    lanternward.blow.bolt_meets.iron(ga.ids(J.get(s, "seats")), s, i),
    lanternward.blow.bolt_meets.foes(s, i, 0))

# Iron: the first knight it touches takes it (or turns it).
def lanternward.blow.bolt_meets.iron(+ids: List<&2, U32>, +s: J.Val, +i: U32) -> J.Val:
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

def lanternward.blow.bolt_meets.iron.near(near: Bool, +id: U32, +t: List<&2, U32>, +s: J.Val, +i: U32) -> J.Val:
  match near:
    case False{}:
      lanternward.blow.bolt_meets.iron(t, s, i)
    case True{}:
      lanternward.blow.bolt_meets.iron.after(
        lanternward.blow.Session.blow(s, id, 2,
          (lanternward.eu(s, "bolts", i, "x") - lanternward.eu(s, "bolts", i, "vx") : U32),
          (lanternward.eu(s, "bolts", i, "y") - lanternward.eu(s, "bolts", i, "vy") : U32), "Bolt"),
        id, t, i)

def lanternward.blow.bolt_meets.iron.after(+s: J.Val, +id: U32, +t: List<&2, U32>, +i: U32) -> J.Val:
  lanternward.blow.bolt_meets.iron.res(ga.s(s, "$ret"), J.del(s, "$ret"), id, t, i)

def lanternward.blow.bolt_meets.iron.res(+out: String, +s: J.Val, +id: U32, +t: List<&2, U32>, +i: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(out, "Dodged"), lanternward.blow.bolt_meets.iron(t, s, i),
    Bool.pick(J.Val, String.eq(out, "Hit"), lanternward.es(s, "bolts", i, "ttl", 0),
      lanternward.blow.Session.turn_bolt(s, i, id, String.eq(out, "Parried"))))

# Light, pot or a turned bolt: the first foe it touches, then the Warden.
def lanternward.blow.bolt_meets.foes(+s: J.Val, +i: U32, +j: U32) -> J.Val:
  lanternward.blow.bolt_meets.foes.go(U32.is_lt(j, lanternward.len(s, "foes")), s, i, j)

def lanternward.blow.bolt_meets.foes.go(more: Bool, +s: J.Val, +i: U32, +j: U32) -> J.Val:
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

def lanternward.blow.bolt_meets.foe(near: Bool, +s: J.Val, +i: U32, +j: U32) -> J.Val:
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

def lanternward.blow.bolt_meets.foe.hit(+kind: String, +s: J.Val, +i: U32, +j: U32, +fx: U32, +fy: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Light"), lanternward.blow.Session.light_foe(s, j),
    Bool.pick(J.Val, String.eq(kind, "Bright"),
      lanternward.blow.bolt_meets.foe.bright(lanternward.eis(s, "foes", j, "kind", "Turret"), s, j, fx, fy),
      Bool.pick(J.Val, String.eq(kind, "Dim"),
        lanternward.blow.Session.hurt_foe(s, j, 1, fx, fy),
        lanternward.blow.Session.hurt_foe(s, j, 2, fx, fy))))

def lanternward.blow.bolt_meets.foe.bright(turret: Bool, +s: J.Val, +j: U32, +fx: U32, +fy: U32) -> J.Val:
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
def lanternward.blow.bolt_meets.boss(+s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.bolt_meets.boss.go(
    Bool.and(Bool.not(J.is_null(J.get(s, "boss"))), U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0)), s, i)

def lanternward.blow.bolt_meets.boss.go(alive: Bool, +s: J.Val, +i: U32) -> J.Val:
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

def lanternward.blow.bolt_meets.boss.near(+heart: Bool, +body: Bool, +s: J.Val, +i: U32) -> J.Val:
  lanternward.blow.bolt_meets.boss.near.go(Bool.or(heart, body), heart, s, i)

def lanternward.blow.bolt_meets.boss.near.go(any: Bool, heart: Bool, +s: J.Val, +i: U32) -> J.Val:
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

def lanternward.blow.bolt_meets.boss.hit(+kind: String, heart: Bool, +s: J.Val, +fx: U32, +fy: U32, +owner: U32) -> J.Val:
  Bool.pick(J.Val, Bool.and(lanternward.NAME_IN(kind, ["Light", "Bright"]), heart),
    lanternward.boss.Session.light_heart(s, owner),
    Bool.pick(J.Val, String.eq(kind, "Pot"),
      lanternward.boss.Session.hurt_boss(s, 2, fx, fy, owner),
      s))

# ---- bombs ----

# Bombs: tick the fuses; a burst opens cracked walls, breaks pots and grass
# and hurts the Gloam, never the knights.
def lanternward.blow.fuses_tick(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Con{ga.su(f, "t", RS.u_sat_sub(ga.u(f, "t"), 1)), lanternward.blow.fuses_tick(t)}

def lanternward.blow.fuses_burst(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Bool.pick(List<&2, J.Val>, U32.is_eq(ga.u(f, "t"), 0),
        Con{f, lanternward.blow.fuses_burst(t)}, lanternward.blow.fuses_burst(t))

def lanternward.blow.fuses_keep(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Bool.pick(List<&2, J.Val>, U32.is_gt(ga.u(f, "t"), 0),
        Con{f, lanternward.blow.fuses_keep(t)}, lanternward.blow.fuses_keep(t))

def lanternward.blow.Session.step_fuses(+s: J.Val) -> J.Val:
  lanternward.blow.step_fuses.go(
    lanternward.blow.fuses_burst(lanternward.blow.fuses_tick(ga.items(s, "fuses"))),
    ga.sv(s, "fuses", J.JA{lanternward.blow.fuses_keep(lanternward.blow.fuses_tick(ga.items(s, "fuses")))}))

def lanternward.blow.step_fuses.go(+burst: List<&2, J.Val>, +s: J.Val) -> J.Val:
  match burst:
    case Nil{}:
      s
    case Con{+f, +t}:
      lanternward.blow.step_fuses.go(t, lanternward.blow.step_fuses.burst(s, ga.u(f, "x"), ga.u(f, "y")))

def lanternward.blow.step_fuses.burst(+s: J.Val, +x: U32, +y: U32) -> J.Val:
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

def lanternward.blow.step_fuses.foes(+s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
  lanternward.blow.step_fuses.foes.go(U32.is_lt(i, lanternward.len(s, "foes")), s, i, x, y)

def lanternward.blow.step_fuses.foes.go(more: Bool, +s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
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

def lanternward.blow.step_fuses.foe(hit: Bool, +s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.blow.step_fuses.foe.kind(lanternward.eis(s, "foes", i, "kind", "Turret"), s, i, x, y)

def lanternward.blow.step_fuses.foe.kind(turret: Bool, +s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
  match turret:
    case True{}:
      lanternward.eset(lanternward.es(lanternward.eset(s, "foes", i, "st", J.JS{"Stagger"}), "foes", i, "t", 90), "foes", i, "danger", J.JS{"None"})
    case False{}:
      lanternward.blow.Session.hurt_foe(lanternward.es(s, "foes", i, "lit", 60), i, 8, x, y)

def lanternward.blow.step_fuses.props(+s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
  lanternward.blow.step_fuses.props.go(U32.is_lt(i, lanternward.len(s, "props")), s, i, x, y)

def lanternward.blow.step_fuses.props.go(more: Bool, +s: J.Val, +i: U32, +x: U32, +y: U32) -> J.Val:
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

def lanternward.blow.step_fuses.prop(near: Bool, +s: J.Val, +i: U32) -> J.Val:
  match near:
    case False{}:
      s
    case True{}:
      lanternward.blow.step_fuses.prop.kind(
        ga.s(lanternward.el(s, "props", i), "kind"), U32.is_eq(lanternward.eu(s, "props", i, "on"), 0), s, i)

def lanternward.blow.step_fuses.prop.kind(+kind: String, +on0: Bool, +s: J.Val, +i: U32) -> J.Val:
  Bool.pick(J.Val, Bool.and(String.eq(kind, "Cracked"), on0),
    lanternward.sound(
      lanternward.props.Session.solved(
        lanternward.props.Session.fx(lanternward.es(s, "props", i, "on", 1), "Dust", lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 0),
        lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y")),
      "crack_open"),
    Bool.pick(J.Val, Bool.and(lanternward.NAME_IN(kind, ["Pot", "Grass"]), on0),
      lanternward.props.Session.shatter(s, i),
      s))

def lanternward.blow.step_fuses.boss(+s: J.Val, +x: U32, +y: U32) -> J.Val:
  lanternward.blow.step_fuses.boss.go(Bool.not(J.is_null(J.get(s, "boss"))), s, x, y)

def lanternward.blow.step_fuses.boss.go(has: Bool, +s: J.Val, +x: U32, +y: U32) -> J.Val:
  match has:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.blast_boss(s, x, y)
