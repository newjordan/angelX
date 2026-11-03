# crates/delve/src/rules/lanternward/boss.rs — the Warden. The Warden being
# stepped rides in `s["$w"]` (and `s["boss"]` is null meanwhile, as Rust's
# `take()` leaves it). `$parried` collects whether a beam or ring was parried.

# The lamp-heart's place: [x, y] (in its chest, toward where it faces).
def lanternward.boss.Warden.heart.x(+w: J.Val) -> U32:
  (ga.u(w, "x") + ga.nu(ga.v(w, "face"), 0) * 9 : U32)

def lanternward.boss.Warden.heart.y(+w: J.Val) -> U32:
  (ga.u(w, "y") + ga.nu(ga.v(w, "face"), 1) * 9 : U32)

def lanternward.boss.Warden.radius() -> U32:
  4608

def lanternward.boss.Warden.new(+x: U32, +y: U32, +seats: U32) -> J.Val:
  lanternward.boss.Warden.new.go(x, y, RS.u_max(seats, 1), (90 * RS.u_max(seats, 1) : U32), RS.u_clamp(seats, 1, 6))

def lanternward.boss.Warden.new.go(+x: U32, +y: U32, +seats: U32, +full: U32, +prov: U32) -> J.Val:
  +w0 = ga.si(ga.si(J.obj(), "x", x), "y", y)
  +w1 = ga.su(ga.su(ga.su(ga.su(w0, "hp", full), "full", full), "prov", prov), "phase", 0)
  +w2 = ga.su(ga.sv(ga.sv(w1, "st", J.JS{"Asleep"}), "atk", J.JS{"Slam"}), "t", 0)
  +w3 = ga.si(ga.sv(ga.su(w2, "n", 0), "danger", J.JS{"None"}), "aim", 90)
  +w4 = ga.sv(ga.sv(ga.si(w3, "sweep", 0), "lit", lanternward.zeros6()), "hit", lanternward.zeros6())
  ga.su(ga.sv(ga.su(ga.su(ga.su(w4, "struck", 0), "dead", 0), "start", full), "face", ga.pair_i(0, 256)), "flinch", 0)

def lanternward.zeros6() -> J.Val:
  J.JA{[J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}]}

# Whether every number is one a save or the wire may carry.
def lanternward.boss.Warden.valid(+w: J.Val) -> Bool:
  Bool.and(
    Bool.and(
      Bool.and(U32.is_le(ga.u(w, "hp"), ga.u(w, "full")), U32.is_le(ga.u(w, "full"), 540)),
      Bool.and(U32.is_ge(ga.u(w, "prov"), 1), U32.is_le(ga.u(w, "prov"), 6))),
    Bool.and(
      Bool.and(U32.is_le(ga.u(w, "phase"), 2), U32.is_le(ga.u(w, "t"), 600)),
      Bool.and(
        lanternward.in_range(ga.u(w, "sweep"), 4294966936, 360),
        Bool.and(
          RS.i_le(lanternward.world.dist(ga.nu(ga.v(w, "face"), 0), ga.nu(ga.v(w, "face"), 1)), 264),
          Bool.and(U32.is_le(ga.u(w, "dead"), 600), U32.is_le(ga.u(w, "start"), ga.u(w, "full")))))))

# How many knights' light the shutter wants.
def lanternward.boss.Warden.need(+w: J.Val, +seats: U32) -> U32:
  Bool.pick(U32, U32.is_ge(ga.u(w, "phase"), 1), RS.u_min(seats, 2), 1)

def lanternward.boss.lit_count(+xs: List<&2, J.Val>, +now: U32) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, +t}:
      (Bool.to_u32(Bool.and(U32.is_gt(J.u32(h), 0), U32.is_gt((J.u32(h) + 90 : U32), now))) + lanternward.boss.lit_count(t, now) : U32)

# Whether the shutter is open at tick `now`.
def lanternward.boss.Warden.exposed(+w: J.Val, +now: U32, +seats: U32) -> Bool:
  Bool.or(
    lanternward.is(w, "st", "Stagger"),
    U32.is_ge(lanternward.boss.lit_count(ga.items(w, "lit"), now), lanternward.boss.Warden.need(w, seats)))

# Light fell on the lamp-heart from seat `owner`.
def lanternward.boss.Session.light_heart(+s: J.Val, +owner: U32) -> J.Val:
  lanternward.boss.Session.light_heart.go(
    Bool.and(Bool.not(J.is_null(J.get(s, "boss"))), Bool.and(U32.is_ge(owner, 1), U32.is_le(owner, 6))), s, owner)

def lanternward.boss.Session.light_heart.go(ok: Bool, +s: J.Val, +owner: U32) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      ga.sv(s, "boss", ga.sv(J.get(s, "boss"), "lit", J.set_nth(ga.v(J.get(s, "boss"), "lit"), (owner - 1 : U32), J.JU{ga.u(s, "elapsed")})))

def lanternward.boss.Session.boss_exposed(+s: J.Val) -> Bool:
  Bool.and(
    Bool.not(J.is_null(J.get(s, "boss"))),
    lanternward.boss.Warden.exposed(J.get(s, "boss"), ga.u(s, "elapsed"), lanternward.seat_count(s)))

def lanternward.boss.Session.boss_open(+s: J.Val) -> Bool:
  lanternward.boss.Session.boss_exposed(s)

# A blade or a pot on the Warden. Only the open heart can be hurt. `$ret` is
# whether it hurt.
def lanternward.boss.Session.hurt_boss(+s: J.Val, +dmg: U32, +fx: U32, +fy: U32, +owner: U32) -> J.Val:
  lanternward.boss.Session.hurt_boss.open(lanternward.boss.Session.boss_exposed(s), s, dmg, fx, fy)

def lanternward.boss.Session.hurt_boss.open(open: Bool, +s: J.Val, +dmg: U32, +fx: U32, +fy: U32) -> J.Val:
  match open:
    case False{}:
      lanternward.boss.Session.hurt_boss.shut(J.get(s, "boss"), s, fx, fy)
    case True{}:
      lanternward.boss.Session.hurt_boss.hit(
        Bool.or(U32.is_eq(ga.u(J.get(s, "boss"), "hp"), 0), lanternward.is(J.get(s, "boss"), "st", "Dying")), s, dmg)

def lanternward.boss.Session.hurt_boss.shut(+w: J.Val, +s: J.Val, +fx: U32, +fy: U32) -> J.Val:
  +bx = ga.u(w, "x")
  +by = ga.u(w, "y")
  ga.sb(
    lanternward.sound(
      lanternward.props.Session.fx(s, "Ring",
        (bx + RS.i_div((fx - bx : U32), 6) : U32), (by + RS.i_div((fy - by : U32), 6) : U32), 12),
      "clang"),
    "$ret", False{})

def lanternward.boss.Session.hurt_boss.hit(already: Bool, +s: J.Val, +dmg: U32) -> J.Val:
  match already:
    case True{}:
      ga.sb(s, "$ret", False{})
    case False{}:
      lanternward.boss.Session.hurt_boss.dmg(J.get(s, "boss"), s, dmg)

def lanternward.boss.Session.hurt_boss.dmg(+w: J.Val, +s: J.Val, +dmg: U32) -> J.Val:
  +dm = Bool.pick(U32, lanternward.is(w, "st", "Stagger"), (dmg * 2 : U32), dmg)
  +hp = RS.u_sat_sub(ga.u(w, "hp"), dm)
  +third = U32.div(ga.u(w, "full"), 3)
  +phase = Bool.pick(U32, U32.is_le(hp, third), 2, Bool.pick(U32, U32.is_le(hp, (third * 2 : U32)), 1, 0))
  +w1 = ga.su(ga.su(w, "hp", hp), "flinch", 8)
  lanternward.boss.Session.hurt_boss.after(U32.is_eq(hp, 0), phase, ga.u(w, "phase"), w1, s)

def lanternward.boss.Session.hurt_boss.after(dead: Bool, +phase: U32, +before: U32, +w: J.Val, +s: J.Val) -> J.Val:
  match dead:
    case True{}:
      lanternward.boss.Session.hurt_boss.die(w, s)
    case False{}:
      lanternward.boss.Session.hurt_boss.live(U32.is_gt(phase, before), phase, w, s)

# It dies: its summons go out with it.
def lanternward.boss.Session.hurt_boss.die(+w: J.Val, +s: J.Val) -> J.Val:
  ga.sb(
    ga.su(
      lanternward.props.Session.fx(
        lanternward.stop(
          lanternward.sound(
            lanternward.boss.Session.hurt_boss.foes(
              ga.sv(s, "boss", ga.sv(ga.su(ga.sv(w, "st", J.JS{"Dying"}), "dead", 0), "danger", J.JS{"None"})), 0),
            "boss_death"),
          "Boss"),
        "Puff", ga.u(w, "x"), ga.u(w, "y"), 0),
      "shake", 16),
    "$ret", True{})

def lanternward.boss.Session.hurt_boss.foes(+s: J.Val, +i: U32) -> J.Val:
  lanternward.boss.Session.hurt_boss.foes.go(U32.is_lt(i, lanternward.len(s, "foes")), s, i)

def lanternward.boss.Session.hurt_boss.foes.go(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.hurt_boss.foes(
        lanternward.boss.Session.hurt_boss.foe(lanternward.foes.Foe.alive(lanternward.el(s, "foes", i)), s, i), (i + 1 : U32))

def lanternward.boss.Session.hurt_boss.foe(alive: Bool, +s: J.Val, +i: U32) -> J.Val:
  match alive:
    case False{}:
      s
    case True{}:
      lanternward.es(lanternward.es(lanternward.eset(lanternward.es(s, "foes", i, "hp", 0), "foes", i, "st", J.JS{"Dying"}), "foes", i, "dead", 0), "foes", i, "dead", 0)

# Not dead: a new phase rears it and slams the shutter; else a plain hit.
def lanternward.boss.Session.hurt_boss.live(rise: Bool, +phase: U32, +w: J.Val, +s: J.Val) -> J.Val:
  match rise:
    case True{}:
      lanternward.boss.Session.hurt_boss.spark(
        lanternward.boss.Session.hurt_boss.rise(phase, w, s))
    case False{}:
      lanternward.boss.Session.hurt_boss.spark(
        lanternward.sound(ga.sv(s, "boss", w), "hit_axe"))

def lanternward.boss.Session.hurt_boss.rise(+phase: U32, +w: J.Val, +s: J.Val) -> J.Val:
  +w1 = ga.sv(ga.sv(ga.su(ga.sv(ga.su(ga.su(w, "phase", phase), "start", ga.u(w, "hp")), "st", J.JS{"Recover"}), "t", 48), "danger", J.JS{"None"}), "lit", lanternward.zeros6())
  ga.su(
    ga.su(
      lanternward.stop(
        lanternward.boss.Session.hurt_boss.dark(U32.is_eq(phase, 2), lanternward.sound(ga.sv(s, "boss", w1), "boss_rage")),
        "Phase"),
      "shake", 8),
    "flash", RS.u_max(ga.u(s, "flash"), 6))

# The Gloam falls: every lamp in the arena is snuffed.
def lanternward.boss.Session.hurt_boss.dark(last: Bool, +s: J.Val) -> J.Val:
  match last:
    case False{}:
      s
    case True{}:
      lanternward.sound(lanternward.boss.Session.hurt_boss.braziers(s, 0), "step_ash")

def lanternward.boss.Session.hurt_boss.braziers(+s: J.Val, +i: U32) -> J.Val:
  lanternward.boss.Session.hurt_boss.braziers.go(U32.is_lt(i, lanternward.len(s, "props")), s, i)

def lanternward.boss.Session.hurt_boss.braziers.go(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.hurt_boss.braziers(
        lanternward.boss.Session.hurt_boss.brazier(lanternward.eis(s, "props", i, "kind", "Brazier"), s, i), (i + 1 : U32))

def lanternward.boss.Session.hurt_boss.brazier(yes: Bool, +s: J.Val, +i: U32) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.es(lanternward.es(s, "props", i, "on", 0), "props", i, "t", 0)

def lanternward.boss.Session.hurt_boss.spark(+s: J.Val) -> J.Val:
  ga.sb(
    lanternward.props.Session.fx(s, "Spark",
      lanternward.boss.Warden.heart.x(J.get(s, "boss")), lanternward.boss.Warden.heart.y(J.get(s, "boss")), 0),
    "$ret", True{})

# A bomb's blast near the Warden: it counts only on an open heart.
def lanternward.boss.Session.blast_boss(+s: J.Val, +x: U32, +y: U32) -> J.Val:
  lanternward.boss.Session.blast_boss.go(
    Bool.not(J.is_null(J.get(s, "boss"))), s, x, y)

def lanternward.boss.Session.blast_boss.go(has: Bool, +s: J.Val, +x: U32, +y: U32) -> J.Val:
  match has:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.blast_boss.hit(
        RS.i_le(
          lanternward.world.dist((ga.u(J.get(s, "boss"), "x") - x : U32), (ga.u(J.get(s, "boss"), "y") - y : U32)),
          (9728 + 4608 : U32)),
        s, x, y)

def lanternward.boss.Session.blast_boss.hit(near: Bool, +s: J.Val, +x: U32, +y: U32) -> J.Val:
  match near:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.hurt_boss(s, 8, x, y, 0)

# A retry puts the Warden back at the start of the phase it was in, at the
# health the table gave it.
def lanternward.boss.Session.reset_boss_phase(+s: J.Val) -> J.Val:
  lanternward.boss.Session.reset_boss_phase.go(
    Bool.and(Bool.not(J.is_null(J.get(s, "boss"))), U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0)),
    s, lanternward.world.Plan.find(4, 87))

def lanternward.boss.Session.reset_boss_phase.go(ok: Bool, +s: J.Val, +marks: List<&2, U32>) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.reset_boss_phase.at(s, marks)

def lanternward.boss.Session.reset_boss_phase.at(+s: J.Val, +marks: List<&2, U32>) -> J.Val:
  match marks:
    case Nil{}:
      lanternward.boss.Session.reset_boss_phase.set(s, 0, 0)
    case Con{+m, +t}:
      lanternward.boss.Session.reset_boss_phase.set(
        s, lanternward.world.middle(lanternward.world.packed_x(m)), lanternward.world.middle(lanternward.world.packed_y(m)))

def lanternward.boss.Session.reset_boss_phase.set(+s: J.Val, +px: U32, +py: U32) -> J.Val:
  +w = J.get(s, "boss")
  +w1 = ga.su(ga.sv(ga.su(w, "hp", RS.u_max(RS.u_min(ga.u(w, "start"), ga.u(w, "full")), 1)), "st", J.JS{"Rest"}), "t", 60)
  +w2 = ga.sv(ga.sv(ga.si(ga.si(w1, "x", px), "y", py), "danger", J.JS{"None"}), "lit", lanternward.zeros6())
  ga.sv(s, "boss", ga.su(ga.su(w2, "struck", 0), "n", 0))

# ---- the Warden's tick ----

# Flares held on the heart keep it lit.
def lanternward.boss.step_boss.flares(+s: J.Val, +ids: List<&2, U32>, +hx: U32, +hy: U32) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.boss.step_boss.flares(
        lanternward.boss.step_boss.flare(lanternward.boss.step_boss.flares.ok(s, id, hx, hy), s, id), t, hx, hy)

def lanternward.boss.step_boss.flares.ok(+s: J.Val, +id: U32, +hx: U32, +hy: U32) -> Bool:
  +k = ga.at(J.get(s, "seats"), id)
  Bool.and(
    Bool.and(lanternward.kit.Seat.flaring(k), lanternward.kit.Seat.up(k)),
    Bool.and(
      RS.i_le(lanternward.world.dist((ga.u(k, "x") - hx : U32), (ga.u(k, "y") - hy : U32)), 17920),
      lanternward.props.Session.line_open(s, ga.u(k, "x"), ga.u(k, "y"), hx, hy)))

def lanternward.boss.step_boss.flare(ok: Bool, +s: J.Val, +id: U32) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.light_heart(s, id)

# One tick of the Warden.
def lanternward.boss.Session.step_boss(+s: J.Val) -> J.Val:
  lanternward.boss.step_boss.go(J.is_null(J.get(s, "boss")), s)

def lanternward.boss.step_boss.go(none: Bool, +s: J.Val) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.boss.step_boss.take(
        lanternward.boss.step_boss.flares(
          s, ga.ids(J.get(s, "seats")), lanternward.boss.Warden.heart.x(J.get(s, "boss")), lanternward.boss.Warden.heart.y(J.get(s, "boss"))))

def lanternward.boss.step_boss.take(+s: J.Val) -> J.Val:
  lanternward.boss.step_boss.fin(
    lanternward.boss.step_boss.st(
      ga.s(J.get(s, "boss"), "st"),
      ga.sv(ga.sv(ga.sv(s, "$w", lanternward.wsub(J.get(s, "boss"))), "boss", J.JNull{}), "$parried", J.JBool{False{}}),
      lanternward.foes.Session.nearest_knight(s, ga.u(J.get(s, "boss"), "x"), ga.u(J.get(s, "boss"), "y"))))

# w.flinch = w.flinch.saturating_sub(1), applied as the Warden is taken.
def lanternward.wsub(+w: J.Val) -> J.Val:
  ga.su(w, "flinch", RS.u_sat_sub(ga.u(w, "flinch"), 1))

def lanternward.boss.step_boss.fin(+s: J.Val) -> J.Val:
  J.del(ga.sv(lanternward.boss.step_boss.shove(s, ga.ids(J.get(s, "seats"))), "boss", J.get(s, "$w")), "$w")

# It is a body: a knight cannot stand in it.
def lanternward.boss.step_boss.shove(+s: J.Val, +ids: List<&2, U32>) -> J.Val:
  lanternward.boss.step_boss.shove.go(Bool.not(lanternward.wis(s, "st", "Dying")), s, ids)

def lanternward.boss.step_boss.shove.go(live: Bool, +s: J.Val, +ids: List<&2, U32>) -> J.Val:
  match live:
    case False{}:
      s
    case True{}:
      lanternward.boss.step_boss.shove.ids(s, ids)

def lanternward.boss.step_boss.shove.ids(+s: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.boss.step_boss.shove.ids(lanternward.boss.step_boss.shove.one(s, id), t)

def lanternward.boss.step_boss.shove.one(+s: J.Val, +id: U32) -> J.Val:
  +k = ga.at(J.get(s, "seats"), id)
  +dx = (ga.u(k, "x") - lanternward.wu(s, "x") : U32)
  +dy = (ga.u(k, "y") - lanternward.wu(s, "y") : U32)
  +d = lanternward.world.dist(dx, dy)
  +min = (4608 + 1280 : U32)
  lanternward.boss.step_boss.shove.push(
    Bool.and(lanternward.kit.Seat.up(k), Bool.and(RS.i_lt(d, min), RS.i_gt(d, 0))),
    lanternward.world.unit(dx, dy), s, id, min)

def lanternward.boss.step_boss.shove.push(yes: Bool, u: U32 & U32, +s: J.Val, +id: U32, +min: U32) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.boss.step_boss.shove.set(u, s, id, min)

def lanternward.boss.step_boss.shove.set(u: U32 & U32, +s: J.Val, +id: U32, +min: U32) -> J.Val:
  (+ux, +uy) = u
  lanternward.blow.put_seat(s, id,
    ga.si(ga.si(ga.at(J.get(s, "seats"), id), "x", (lanternward.wu(s, "x") + RS.i_div((ux * min : U32), 256) : U32)),
      "y", (lanternward.wu(s, "y") + RS.i_div((uy * min : U32), 256) : U32)))

def lanternward.boss.step_boss.st(+st: String, +s: J.Val, +target: J.Val) -> J.Val:
  lanternward.boss.step_boss.code(
    Bool.pick(U32, String.eq(st, "Dying"), 1,
      Bool.pick(U32, String.eq(st, "Asleep"), 2,
        Bool.pick(U32, String.eq(st, "Rest"), 3,
          Bool.pick(U32, String.eq(st, "Tell"), 4,
            Bool.pick(U32, String.eq(st, "Strike"), 5,
              Bool.pick(U32, String.eq(st, "Recover"), 6, 7)))))),
    s, target)

def lanternward.boss.step_boss.code(+code: U32, +s: J.Val, +target: J.Val) -> J.Val:
  match code:
    case 1:
      lanternward.boss.step_boss.dying(
        lanternward.ws(s, "dead", (lanternward.wu(s, "dead") + 1 : U32)))
    case 2:
      lanternward.boss.step_boss.asleep(
        Bool.or(
          Bool.and(Bool.not(J.is_null(target)), RS.i_lt(ga.nu(target, 3), 38400)),
          U32.is_gt(ga.u(s, "stage_t"), 150)),
        s)
    case 3:
      lanternward.boss.step_boss.rest(J.is_null(target), s, target)
    case 4:
      lanternward.boss.step_boss.tell(U32.is_le(lanternward.wu(s, "t"), 1), lanternward.ws(s, "t", RS.u_sat_sub(lanternward.wu(s, "t"), 1)))
    case 5:
      lanternward.boss.step_boss.strike(s)
    case 6:
      lanternward.boss.step_boss.recover(U32.is_le(lanternward.wu(s, "t"), 1), lanternward.ws(s, "t", RS.u_sat_sub(lanternward.wu(s, "t"), 1)))
    case _:
      lanternward.boss.step_boss.stagger(U32.is_le(lanternward.wu(s, "t"), 1), lanternward.ws(s, "t", RS.u_sat_sub(lanternward.wu(s, "t"), 1)))

def lanternward.boss.step_boss.dying(+s: J.Val) -> J.Val:
  lanternward.boss.step_boss.dying.go(U32.is_eq(lanternward.wu(s, "dead"), 40), s)

def lanternward.boss.step_boss.dying.go(drop: Bool, +s: J.Val) -> J.Val:
  match drop:
    case True{}:
      ga.su(lanternward.props.Session.drop_at(s, "Container", lanternward.wu(s, "x"), lanternward.wu(s, "y")), "stage", 2)
    case False{}:
      s

def lanternward.boss.step_boss.asleep(woke: Bool, +s: J.Val) -> J.Val:
  match woke:
    case False{}:
      s
    case True{}:
      ga.su(lanternward.sound(ga.su(lanternward.ws(lanternward.wsn(s, "st", "Rest"), "t", 70), "stage", 1), "boss_roar"), "shake", 8)

def lanternward.boss.step_boss.rest(none: Bool, +s: J.Val, +target: J.Val) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.boss.step_boss.rest.go(
        lanternward.world.unit((ga.nu(target, 1) - lanternward.wu(s, "x") : U32), (ga.nu(target, 2) - lanternward.wu(s, "y") : U32)),
        s, target)

def lanternward.boss.step_boss.rest.go(face: U32 & U32, +s: J.Val, +target: J.Val) -> J.Val:
  (+fx, +fy) = face
  lanternward.boss.step_boss.rest.walk(
    RS.i_gt(ga.nu(target, 3), (4608 + 6656 : U32)),
    lanternward.wsv(s, "face", ga.pair_i(fx, fy)), fx, fy, target)

def lanternward.boss.step_boss.rest.walk(far: Bool, +s: J.Val, +fx: U32, +fy: U32, +target: J.Val) -> J.Val:
  match far:
    case False{}:
      lanternward.boss.step_boss.rest.tick(s, target)
    case True{}:
      lanternward.boss.step_boss.rest.step(
        (lanternward.wu(s, "x") + RS.i_div((fx * 128 : U32), 256) : U32),
        (lanternward.wu(s, "y") + RS.i_div((fy * 128 : U32), 256) : U32), s, target)

def lanternward.boss.step_boss.rest.step(+nx: U32, +ny: U32, +s: J.Val, +target: J.Val) -> J.Val:
  lanternward.boss.step_boss.rest.tick(
    lanternward.boss.step_boss.rest.move(lanternward.props.Session.clear(s, nx, ny, 2304), s, nx, ny), target)

def lanternward.boss.step_boss.rest.move(ok: Bool, +s: J.Val, +nx: U32, +ny: U32) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.wsi(lanternward.wsi(s, "x", nx), "y", ny)

def lanternward.boss.step_boss.rest.tick(+s: J.Val, +target: J.Val) -> J.Val:
  lanternward.boss.step_boss.rest.attack(
    U32.is_le(lanternward.wu(s, "t"), 1), lanternward.ws(s, "t", RS.u_sat_sub(lanternward.wu(s, "t"), 1)), target)

def lanternward.boss.step_boss.rest.attack(zero: Bool, +s: J.Val, +target: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.begin_attack(s, ga.nu(target, 1), ga.nu(target, 2))

def lanternward.boss.step_boss.tell(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.boss.step_boss.tell.go(
        lanternward.boss.atk_ticks(ga.s(J.get(s, "$w"), "atk")),
        lanternward.wsi(lanternward.ws(lanternward.ws(lanternward.wsn(s, "st", "Strike"), "struck", 0), "t", lanternward.boss.atk_ticks(ga.s(J.get(s, "$w"), "atk"))), "sweep", 0))

def lanternward.boss.atk_ticks(+atk: String) -> U32:
  Bool.pick(U32, String.eq(atk, "Beam"), 36, Bool.pick(U32, String.eq(atk, "Slam"), 30, 12))

def lanternward.boss.step_boss.tell.go(+total: U32, +s: J.Val) -> J.Val:
  lanternward.sound(
    lanternward.boss.step_boss.tell.summon(lanternward.wis(s, "atk", "Summon"), s),
    Bool.pick(String, lanternward.wis(s, "atk", "Beam"), "shot_arcane", Bool.pick(String, lanternward.wis(s, "atk", "Slam"), "rock_land", "crack_open")))

def lanternward.boss.step_boss.tell.summon(yes: Bool, +s: J.Val) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.boss.Session.summon(s)

def lanternward.boss.step_boss.recover(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.ws(
        lanternward.ws(lanternward.wsn(s, "st", "Rest"), "t", (60 - 12 * lanternward.wu(s, "phase") : U32)),
        "n", U32.and((lanternward.wu(s, "n") + 1 : U32), 255))

def lanternward.boss.step_boss.stagger(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.ws(lanternward.wsn(s, "st", "Rest"), "t", 30)

# The strike: sweep a beam, or send a ring along the ground.
def lanternward.boss.step_boss.strike(+s: J.Val) -> J.Val:
  lanternward.boss.step_boss.strike.go(
    (lanternward.boss.atk_ticks(ga.s(J.get(s, "$w"), "atk")) - lanternward.wu(s, "t") : U32), s)

def lanternward.boss.step_boss.strike.go(+elapsed: U32, +s: J.Val) -> J.Val:
  +s1 = lanternward.boss.step_boss.strike.kind(
    Bool.pick(U32, lanternward.wis(s, "atk", "Beam"), 1, Bool.pick(U32, lanternward.wis(s, "atk", "Slam"), 2, 0)),
    elapsed, s)
  lanternward.boss.step_boss.strike.end(ga.b(s1, "$parried"), s1)

def lanternward.boss.step_boss.strike.kind(+code: U32, +elapsed: U32, +s: J.Val) -> J.Val:
  match code:
    case 1:
      lanternward.boss.step_boss.beam(s, elapsed, 0)
    case 2:
      lanternward.boss.step_boss.slam(s, elapsed)
    case _:
      s

# One beam of the sweep (and its twin in the last phase): `b` is 0 or 1.
def lanternward.boss.step_boss.beam(+s: J.Val, +elapsed: U32, +b: U32) -> J.Val:
  lanternward.boss.step_boss.beam.go(
    U32.is_lt(b, Bool.pick(U32, U32.is_ge(lanternward.wu(s, "phase"), 2), 2, 1)),
    s, elapsed, b)

def lanternward.boss.step_boss.beam.go(more: Bool, +s: J.Val, +elapsed: U32, +b: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.boss.step_boss.beam.one(
        (lanternward.wu(s, "aim") - 50 + RS.i_div((100 * elapsed : U32), 36) + 180 * b : U32), s, elapsed, b)

def lanternward.boss.step_boss.beam.one(+a: U32, +s: J.Val, +elapsed: U32, +b: U32) -> J.Val:
  lanternward.boss.step_boss.beam.hit(
    lanternward.world.dir(a), s, elapsed, b)

def lanternward.boss.step_boss.beam.hit(d: U32 & U32, +s: J.Val, +elapsed: U32, +b: U32) -> J.Val:
  (+dx, +dy) = d
  lanternward.boss.step_boss.beam.line(
    lanternward.variant("Capsule",
      ga.si(ga.si(ga.si(ga.si(ga.si(J.obj(), "ax", lanternward.wu(s, "x")), "ay", lanternward.wu(s, "y")),
        "bx", (lanternward.wu(s, "x") + dx * 150 : U32)), "by", (lanternward.wu(s, "y") + dy * 150 : U32)), "r", 1536)),
    s, elapsed, b)

def lanternward.boss.step_boss.beam.line(+line: J.Val, +s: J.Val, +elapsed: U32, +b: U32) -> J.Val:
  lanternward.boss.step_boss.beam(
    lanternward.boss.Session.warden_hits(
      lanternward.boss.step_boss.beam.danger(U32.is_eq(b, 0), s, line), line, 4, "Beam"),
    elapsed, (b + 1 : U32))

def lanternward.boss.step_boss.beam.danger(first: Bool, +s: J.Val, +line: J.Val) -> J.Val:
  match first:
    case True{}:
      lanternward.wsv(s, "danger", line)
    case False{}:
      s

def lanternward.boss.step_boss.slam(+s: J.Val, +elapsed: U32) -> J.Val:
  +s1 = lanternward.wsv(s, "danger", lanternward.variant("Circle", ga.si(ga.si(ga.si(J.obj(), "x", lanternward.wu(s, "x")), "y", lanternward.wu(s, "y")), "r", 33792)))
  lanternward.boss.step_boss.slam.wave(
    U32.is_eq(elapsed, 0),
    lanternward.boss.Session.warden_hits(
      s1,
      lanternward.variant("Ring",
        ga.si(ga.si(ga.si(ga.si(J.obj(), "x", lanternward.wu(s, "x")), "y", lanternward.wu(s, "y")),
          "r", ((12 + 4 * elapsed : U32) * 256 : U32)), "w", 1536)),
      4, "Low"))

def lanternward.boss.step_boss.slam.wave(first: Bool, +s: J.Val) -> J.Val:
  match first:
    case False{}:
      s
    case True{}:
      lanternward.props.Session.fx(s, "Wave", lanternward.wu(s, "x"), lanternward.wu(s, "y"), 0)

def lanternward.boss.step_boss.strike.end(parried: Bool, +s: J.Val) -> J.Val:
  match parried:
    case True{}:
      ga.su(
        lanternward.stop(
          lanternward.sound(
            lanternward.wsv(lanternward.wsv(lanternward.ws(lanternward.wsn(s, "st", "Stagger"), "t", 75), "danger", J.JS{"None"}), "lit", lanternward.zeros6()),
            "boss_rage"),
          "Phase"),
        "shake", 8)
    case False{}:
      lanternward.boss.step_boss.strike.tick(
        U32.is_le(lanternward.wu(s, "t"), 1), lanternward.ws(s, "t", RS.u_sat_sub(lanternward.wu(s, "t"), 1)))

def lanternward.boss.step_boss.strike.tick(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.wsv(
        lanternward.ws(lanternward.wsn(s, "st", "Recover"), "t", Bool.pick(U32, lanternward.wis(s, "atk", "Summon"), 30, 44)),
        "danger", J.JS{"None"})

# Choose the Warden's next attack and show where it will land.
def lanternward.boss.pattern(+phase: U32, +n: U32) -> String:
  match phase:
    case 0:
      lanternward.boss.pattern.pick(U32.mod(n, 4), "Slam", "Beam", "Beam", "Slam", "Slam")
    case 1:
      lanternward.boss.pattern.pick(U32.mod(n, 4), "Beam", "Summon", "Slam", "Beam", "Beam")
    case _:
      lanternward.boss.pattern.pick(U32.mod(n, 5), "Beam", "Slam", "Summon", "Beam", "Slam")

def lanternward.boss.pattern.pick(+i: U32, +a: String, +b: String, +c: String, +d: String, +e: String) -> String:
  match i:
    case 0:
      a
    case 1:
      b
    case 2:
      c
    case 3:
      d
    case _:
      e

def lanternward.boss.Session.begin_attack(+s: J.Val, +ax: U32, +ay: U32) -> J.Val:
  lanternward.boss.begin_attack.go(
    lanternward.boss.pattern(lanternward.wu(s, "phase"), lanternward.wu(s, "n")),
    lanternward.world.unit((ax - lanternward.wu(s, "x") : U32), (ay - lanternward.wu(s, "y") : U32)),
    s, ax, ay)

def lanternward.boss.begin_attack.go(+atk: String, face: U32 & U32, +s: J.Val, +ax: U32, +ay: U32) -> J.Val:
  (+fx, +fy) = face
  lanternward.boss.begin_attack.danger(
    atk,
    lanternward.wsv(
      lanternward.ws(
        lanternward.ws(
          lanternward.wsn(lanternward.wsn(s, "atk", atk), "st", "Tell"), "t",
          lanternward.Rules.tell_for(J.get(s, "$r"), lanternward.seat_count(s))),
        "struck", 0),
      "face", ga.pair_i(fx, fy)),
    ax, ay)

def lanternward.boss.begin_attack.danger(+atk: String, +s: J.Val, +ax: U32, +ay: U32) -> J.Val:
  lanternward.boss.begin_attack.danger.kind(
    Bool.pick(U32, String.eq(atk, "Beam"), 1, Bool.pick(U32, String.eq(atk, "Slam"), 2, 0)), s, ax, ay)

def lanternward.boss.begin_attack.danger.kind(+code: U32, +s: J.Val, +ax: U32, +ay: U32) -> J.Val:
  match code:
    case 1:
      lanternward.boss.begin_attack.fan(lanternward.world.angle_of((ax - lanternward.wu(s, "x") : U32), (ay - lanternward.wu(s, "y") : U32)), s)
    case 2:
      lanternward.wsv(s, "danger", lanternward.variant("Circle", ga.si(ga.si(ga.si(J.obj(), "x", lanternward.wu(s, "x")), "y", lanternward.wu(s, "y")), "r", 33792)))
    case _:
      lanternward.wsv(s, "danger", J.JS{"None"})

def lanternward.boss.begin_attack.fan(+aim: U32, +s: J.Val) -> J.Val:
  lanternward.wsv(
    lanternward.wsi(s, "aim", aim), "danger",
    lanternward.variant("Fan",
      ga.si(ga.si(ga.si(ga.si(ga.si(J.obj(), "x", lanternward.wu(s, "x")), "y", lanternward.wu(s, "y")), "deg", aim), "half", 56), "r", 38400)))

# The Warden's blow on every knight in `line` it has not yet struck. A parry
# sets `$parried`.
def lanternward.boss.Session.warden_hits(+s: J.Val, +line: J.Val, +dmg: U32, +kind: String) -> J.Val:
  lanternward.boss.warden_hits.ids(ga.ids(J.get(s, "seats")), s, line, dmg, kind)

def lanternward.boss.warden_hits.ids(+ids: List<&2, U32>, +s: J.Val, +line: J.Val, +dmg: U32, +kind: String) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.boss.warden_hits.ids(t, lanternward.boss.warden_hits.seat(s, id, line, dmg, kind), line, dmg, kind)

def lanternward.boss.warden_hits.seat(+s: J.Val, +id: U32, +line: J.Val, +dmg: U32, +kind: String) -> J.Val:
  +k = ga.at(J.get(s, "seats"), id)
  +bit = U32.shln(1, U32.to_nat(RS.u_min(id, 7)))
  lanternward.boss.warden_hits.hit(
    Bool.and(
      Bool.and(U32.is_eq(U32.and(lanternward.wu(s, "struck"), bit), 0), lanternward.kit.Seat.up(k)),
      lanternward.foes.Danger.hits(line, ga.u(k, "x"), ga.u(k, "y"), 1280)),
    s, id, dmg, kind, bit)

def lanternward.boss.warden_hits.hit(yes: Bool, +s: J.Val, +id: U32, +dmg: U32, +kind: String, +bit: U32) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.boss.warden_hits.after(
        lanternward.blow.Session.blow(s, id, dmg, lanternward.wu(s, "x"), lanternward.wu(s, "y"), kind), bit)

def lanternward.boss.warden_hits.after(+s: J.Val, +bit: U32) -> J.Val:
  lanternward.boss.warden_hits.res(ga.s(s, "$ret"), J.del(s, "$ret"), bit)

def lanternward.boss.warden_hits.res(+out: String, +s: J.Val, +bit: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(out, "Dodged"), s,
    Bool.pick(J.Val, String.eq(out, "Parried"),
      ga.sb(lanternward.ws(s, "struck", U32.or(lanternward.wu(s, "struck"), bit)), "$parried", True{}),
      lanternward.ws(s, "struck", U32.or(lanternward.wu(s, "struck"), bit))))

# Snuffers climb out of the fissures (hooded, in the last phase).
def lanternward.boss.Session.summon(+s: J.Val) -> J.Val:
  lanternward.boss.summon.marks(
    s, lanternward.world.Plan.find(4, 102),
    RS.u_min((2 + U32.div((RS.u_max(lanternward.seat_count(s), 1) - 1 : U32), 2) : U32), 4),
    U32.is_ge(lanternward.wu(s, "phase"), 2))

def lanternward.boss.summon.marks(+s: J.Val, +marks: List<&2, U32>, +n: U32, +hooded: Bool) -> J.Val:
  match marks:
    case Nil{}:
      s
    case Con{+m, +t}:
      lanternward.boss.summon.marks(
        lanternward.boss.summon.one(U32.is_gt(n, 0), s, m, hooded), t, RS.u_sat_sub(n, 1), hooded)

def lanternward.boss.summon.one(go: Bool, +s: J.Val, +m: U32, +hooded: Bool) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.boss.summon.crawl(
        lanternward.boss.summon.fissure(
          lanternward.foes.Session.spawn_foe(s, "Snuffer", lanternward.world.middle(lanternward.world.packed_x(m)), lanternward.world.middle(lanternward.world.packed_y(m)), hooded),
          lanternward.boss.Session.prop_at(s, "Fissure", lanternward.world.packed_x(m), lanternward.world.packed_y(m))))

def lanternward.boss.summon.fissure(+s: J.Val, +i: J.Val) -> J.Val:
  match i:
    case J.JNull{}:
      s
    case _:
      lanternward.es(s, "props", J.u32(i), "t", 40)

def lanternward.boss.summon.crawl(+s: J.Val) -> J.Val:
  lanternward.boss.summon.crawl.go(U32.is_gt(lanternward.len(s, "foes"), 0), s)

def lanternward.boss.summon.crawl.go(any: Bool, +s: J.Val) -> J.Val:
  match any:
    case False{}:
      s
    case True{}:
      lanternward.es(
        lanternward.eset(s, "foes", (lanternward.len(s, "foes") - 1 : U32), "st", J.JS{"Drift"}),
        "foes", (lanternward.len(s, "foes") - 1 : U32), "t", 30)

# The index of the first prop of this kind at this tile, or null.
def lanternward.boss.prop_at.go(+ps: List<&2, J.Val>, +kind: String, +tx: U32, +ty: U32, +i: U32) -> J.Val:
  match ps:
    case Nil{}:
      J.JNull{}
    case Con{+p, +t}:
      lanternward.boss.prop_at.hit(
        Bool.and(String.eq(ga.s(p, "kind"), kind), Bool.and(U32.is_eq(ga.u(p, "tx"), tx), U32.is_eq(ga.u(p, "ty"), ty))),
        t, kind, tx, ty, i)

def lanternward.boss.prop_at.hit(hit: Bool, +t: List<&2, J.Val>, +kind: String, +tx: U32, +ty: U32, +i: U32) -> J.Val:
  match hit:
    case True{}:
      J.JU{i}
    case False{}:
      lanternward.boss.prop_at.go(t, kind, tx, ty, (i + 1 : U32))

def lanternward.boss.Session.prop_at(+s: J.Val, +kind: String, +tx: U32, +ty: U32) -> J.Val:
  lanternward.boss.prop_at.go(ga.items(s, "props"), kind, tx, ty, 0)

# The last room: the heart container, then the Beacon.
def lanternward.boss.Session.stage_warden(+s: J.Val) -> J.Val:
  lanternward.boss.stage_warden.go(U32.is_eq(ga.u(s, "stage"), 3), s)

def lanternward.boss.stage_warden.go(on: Bool, +s: J.Val) -> J.Val:
  match on:
    case False{}:
      s
    case True{}:
      lanternward.boss.stage_warden.beacon(
        lanternward.boss.stage_warden.holders(ga.ids(J.get(s, "seats")), J.get(s, "seats")),
        s, lanternward.find_prop_index(s, "Beacon"))

def lanternward.boss.stage_warden.holders(+ids: List<&2, U32>, +seats: J.Val) -> U32:
  match ids:
    case Nil{}:
      0
    case Con{+id, +t}:
      (Bool.to_u32(
        Bool.and(
          Bool.and(lanternward.kit.Seat.up(ga.at(seats, id)), lanternward.is(ga.at(seats, id), "act", "Free")),
          Bool.and(
            U32.is_ne(U32.and(ga.u(ga.at(seats, id), "held"), 8), 0),
            lanternward.tag_is(ga.v(ga.at(seats, id), "ctx"), "Relight")))) + lanternward.boss.stage_warden.holders(t, seats) : U32)

def lanternward.boss.stage_warden.beacon(+holders: U32, +s: J.Val, +i: J.Val) -> J.Val:
  match i:
    case J.JNull{}:
      s
    case _:
      lanternward.boss.stage_warden.tick(
        lanternward.es(s, "props", J.u32(i), "t",
          Bool.pick(U32, U32.is_gt(holders, 0),
            RS.u_min((lanternward.eu(s, "props", J.u32(i), "t") + holders : U32), 65535),
            RS.u_sat_sub(lanternward.eu(s, "props", J.u32(i), "t"), 1))),
        J.u32(i))

def lanternward.boss.stage_warden.tick(+s: J.Val, +i: U32) -> J.Val:
  lanternward.boss.stage_warden.relit(
    Bool.and(U32.is_ge(lanternward.eu(s, "props", i, "t"), 90), U32.is_eq(lanternward.eu(s, "props", i, "on"), 0)), s, i)

def lanternward.boss.stage_warden.relit(yes: Bool, +s: J.Val, +i: U32) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.won(
        lanternward.sound(
          lanternward.sound(
            lanternward.props.Session.fx(
              ga.su(ga.su(ga.sv(ga.su(lanternward.es(s, "props", i, "on", 1), "stage", 4), "done", J.JU{ga.u(s, "elapsed")}), "flash", 12), "shake", 8),
              "Solved", lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 1),
            "circle_toll"),
          "wish_grant"))

# How long the shutter has left open, 0 if shut.
def lanternward.boss.min_open(+xs: List<&2, J.Val>, +now: U32, +best: J.Val) -> J.Val:
  match xs:
    case Nil{}:
      best
    case Con{+h, +t}:
      lanternward.boss.min_open(t, now,
        Bool.pick(J.Val, U32.is_gt(J.u32(h), 0),
          Bool.pick(J.Val, Bool.or(J.is_null(best), U32.is_lt(RS.u_sat_sub((J.u32(h) + 90 : U32), now), J.u32(best))),
            J.JU{RS.u_sat_sub((J.u32(h) + 90 : U32), now)}, best),
          best))

def lanternward.boss.Warden.open_for(+w: J.Val, +now: U32, +seats: U32) -> U32:
  Bool.pick(U32, lanternward.boss.Warden.exposed(w, now, seats),
    Bool.pick(U32, lanternward.is(w, "st", "Stagger"), ga.u(w, "t"),
      J.u32(lanternward.boss.min_open(ga.items(w, "lit"), now, J.JNull{}))),
    0)
