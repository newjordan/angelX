# crates/delve/src/rules/lanternward/foes.rs — the snuffer, the turret, their
# bolts, and the foes' tick.
#
# A Danger is serde's: "None", {"Capsule": {ax, ay, bx, by, r}},
# {"Circle": {x, y, r}}, {"Fan": {x, y, deg, half, r}}, {"Ring": {x, y, r, w}}.
# The foe being stepped rides in `s["$f"]`; the strikes and shots it makes
# are collected in `s["$strikes"]` ([index, danger, dmg]) and `s["$shots"]`.

# Squared distance from a point to a segment, in i64 (the Rust's `seg_dist2`).
def lanternward.foes.seg_dist2(+px: U32, +py: U32, +ax: U32, +ay: U32, +bx: U32, +by: U32) -> W.W:
  lanternward.foes.seg_dist2.go(
    lanternward.i64.sub(lanternward.i64.of_i32(px), lanternward.i64.of_i32(ax)),
    lanternward.i64.sub(lanternward.i64.of_i32(py), lanternward.i64.of_i32(ay)),
    lanternward.i64.sub(lanternward.i64.of_i32(bx), lanternward.i64.of_i32(ax)),
    lanternward.i64.sub(lanternward.i64.of_i32(by), lanternward.i64.of_i32(ay)),
    lanternward.i64.of_i32(ax), lanternward.i64.of_i32(ay), lanternward.i64.of_i32(px), lanternward.i64.of_i32(py))

def lanternward.foes.seg_dist2.go(+pax: W.W, +pay: W.W, +dx: W.W, +dy: W.W, +ax: W.W, +ay: W.W, +px: W.W, +py: W.W) -> W.W:
  +len2 = lanternward.i64.add(lanternward.i64.mul(dx, dx), lanternward.i64.mul(dy, dy))
  +t = Bool.pick(W.W, W.is_zero(len2), W.zero(),
    lanternward.i64.clamp(
      lanternward.i64.div(
        lanternward.i64.mul(lanternward.i64.add(lanternward.i64.mul(pax, dx), lanternward.i64.mul(pay, dy)), W.W{0, 1024}),
        len2),
      W.zero(), W.W{0, 1024}))
  +cx = lanternward.i64.add(ax, lanternward.i64.div(lanternward.i64.mul(dx, t), W.W{0, 1024}))
  +cy = lanternward.i64.add(ay, lanternward.i64.div(lanternward.i64.mul(dy, t), W.W{0, 1024}))
  +ex = lanternward.i64.sub(px, cx)
  +ey = lanternward.i64.sub(py, cy)
  lanternward.i64.add(lanternward.i64.mul(ex, ex), lanternward.i64.mul(ey, ey))

# Whether a body of radius `rad` at (x, y) stands in the footprint.
def lanternward.foes.Danger.hits(+d: J.Val, +x: U32, +y: U32, +rad: U32) -> Bool:
  +p = lanternward.payload(d)
  +tag = lanternward.tag(d)
  Bool.pick(Bool, String.eq(tag, "Capsule"), lanternward.foes.Danger.hits.capsule(p, x, y, rad),
    Bool.pick(Bool, String.eq(tag, "Circle"),
      RS.i_le(lanternward.world.dist((x - ga.u(p, "x") : U32), (y - ga.u(p, "y") : U32)), (ga.u(p, "r") + rad : U32)),
      Bool.pick(Bool, String.eq(tag, "Fan"), lanternward.foes.Danger.hits.fan(p, x, y, rad),
        Bool.pick(Bool, String.eq(tag, "Ring"),
          RS.i_le(RS.i_abs((lanternward.world.dist((x - ga.u(p, "x") : U32), (y - ga.u(p, "y") : U32)) - ga.u(p, "r") : U32)), (ga.u(p, "w") + rad : U32)),
          False{}))))

def lanternward.foes.Danger.hits.capsule(+p: J.Val, +x: U32, +y: U32, +rad: U32) -> Bool:
  +reach = lanternward.i64.of_i32((ga.u(p, "r") + rad : U32))
  lanternward.i64.le(
    lanternward.foes.seg_dist2(x, y, ga.u(p, "ax"), ga.u(p, "ay"), ga.u(p, "bx"), ga.u(p, "by")),
    lanternward.i64.mul(reach, reach))

def lanternward.foes.Danger.hits.fan(+p: J.Val, +x: U32, +y: U32, +rad: U32) -> Bool:
  +d = lanternward.world.dist((x - ga.u(p, "x") : U32), (y - ga.u(p, "y") : U32))
  Bool.and(
    RS.i_le(d, (ga.u(p, "r") + rad : U32)),
    Bool.or(
      RS.i_lt(d, rad),
      RS.i_ge(
        lanternward.foes.Danger.hits.fan_dot((x - ga.u(p, "x") : U32), (y - ga.u(p, "y") : U32), ga.u(p, "deg")),
        lanternward.world.cos_q(ga.u(p, "half")))))

def lanternward.foes.Danger.hits.fan_dot(+dx: U32, +dy: U32, +deg: U32) -> U32:
  lanternward.foes.Danger.hits.fan_dot.go(lanternward.world.unit(dx, dy), lanternward.world.cos_q(deg), lanternward.world.sin_q(deg))

def lanternward.foes.Danger.hits.fan_dot.go(u: U32 & U32, +cx: U32, +cy: U32) -> U32:
  (+ux, +uy) = u
  lanternward.world.dot4(ux, uy, cx, cy)

def lanternward.foes.Foe.new(+id: U32, +kind: String, +x: U32, +y: U32, +seats: U32) -> J.Val:
  lanternward.foes.Foe.new.go(id, kind, x, y, RS.u_max(seats, 1))

def lanternward.foes.Foe.new.go(+id: U32, +kind: String, +x: U32, +y: U32, +n: U32) -> J.Val:
  +full = (Bool.pick(U32, String.eq(kind, "Snuffer"), 6, 8) * n : U32)
  +f0 = ga.sv(J.obj(), "id", J.JU{id})
  +f1 = ga.si(ga.si(ga.sv(f0, "kind", J.JS{kind}), "x", x), "y", y)
  +f2 = ga.su(ga.su(ga.su(f1, "hp", full), "full", full), "prov", RS.u_clamp(n, 1, 6))
  +f3 = ga.si(ga.si(ga.su(ga.sv(f2, "st", J.JS{"Dormant"}), "t", 0), "fx", 0), "fy", 256)
  +f4 = ga.sb(ga.sv(f3, "danger", J.JS{"None"}), "hooded", False{})
  +f5 = ga.su(ga.su(ga.su(ga.su(f4, "lit", 0), "flinch", 0), "freeze", 0), "dead", 0)
  +f6 = ga.sv(ga.su(ga.sb(f5, "open", False{}), "struck", 0), "hit", J.JA{[J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}]})
  ga.su(ga.su(ga.sv(f6, "kb", ga.pair_i(0, 0)), "kbt", 0), "n", 0)

def lanternward.foes.Foe.alive(+f: J.Val) -> Bool:
  Bool.and(U32.is_gt(ga.u(f, "hp"), 0), Bool.not(lanternward.is(f, "st", "Dying")))

# Its body's radius in Q pixels.
def lanternward.foes.Foe.radius(+f: J.Val) -> U32:
  Bool.pick(U32, lanternward.is(f, "kind", "Snuffer"), 1536, 2048)

# A big foe's tell for a table of `seats` (Rules::tell_for).
def lanternward.Rules.tell_for(+r: J.Val, +seats: U32) -> U32:
  RS.u_max(RS.u_sat_sub(ga.u(r, "tell"), (3 * RS.u_sat_sub(seats, 1) : U32)), 40)

# Its tell in ticks (`r` is the Rules).
def lanternward.foes.Foe.tell(+f: J.Val, +r: J.Val, +seats: U32) -> U32:
  Bool.pick(U32, lanternward.is(f, "kind", "Snuffer"), 24, U32.and(lanternward.Rules.tell_for(r, seats), 65535))

# Whether a blade may hurt it now.
def lanternward.foes.Foe.vulnerable(+f: J.Val) -> Bool:
  Bool.pick(Bool, lanternward.is(f, "kind", "Snuffer"),
    Bool.not(Bool.and(ga.b(f, "hooded"), U32.is_eq(ga.u(f, "lit"), 0))), False{})

def lanternward.foes.Bolt.light(+b: J.Val) -> Bool:
  Bool.or(lanternward.is(b, "kind", "Light"), lanternward.is(b, "kind", "Bright"))

def lanternward.foes.Bolt.hostile(+b: J.Val) -> Bool:
  lanternward.is(b, "kind", "Iron")

# ---- impl Session ----

# A foe of `kind` where a snuffer or turret mark stands.
def lanternward.foes.Session.spawn_foe(+s: J.Val, +kind: String, +x: U32, +y: U32, +hooded: Bool) -> J.Val:
  lanternward.foes.Session.spawn_foe.go(U32.is_ge(lanternward.len(s, "foes"), 16), s, kind, x, y, hooded)

def lanternward.foes.Session.spawn_foe.go(full: Bool, +s: J.Val, +kind: String, +x: U32, +y: U32, +hooded: Bool) -> J.Val:
  match full:
    case True{}:
      s
    case False{}:
      lanternward.push(
        ga.su(s, "next_foe", U32.and((ga.u(s, "next_foe") + 1 : U32), 255)),
        "foes",
        ga.sb(lanternward.foes.Foe.new(ga.u(s, "next_foe"), kind, x, y, lanternward.seat_count(s)), "hooded", hooded))

# Scale a foe or the Warden already standing to a grown table.
def lanternward.foes.provision.one(+o: J.Val, +now: U32, +boss: Bool) -> J.Val:
  lanternward.foes.provision.one.go(
    Bool.and(U32.is_gt(now, ga.u(o, "prov")), U32.is_gt(ga.u(o, "hp"), 0)), o, now, boss)

def lanternward.foes.provision.one.go(grow: Bool, +o: J.Val, +now: U32, +boss: Bool) -> J.Val:
  match grow:
    case False{}:
      o
    case True{}:
      +had = ga.u(o, "prov")
      +o1 = ga.su(ga.su(o, "hp", W.lo32(W.div_u32(W.mul32(ga.u(o, "hp"), now), had))), "full", U32.div((ga.u(o, "full") * now : U32), had))
      +o2 = Bool.pick(J.Val, boss, ga.su(o1, "start", W.lo32(W.div_u32(W.mul32(ga.u(o, "start"), now), had))), o1)
      ga.su(o2, "prov", U32.and(now, 255))

def lanternward.foes.provision.foes(+s: J.Val, +i: U32, +now: U32) -> J.Val:
  lanternward.foes.provision.foes.go(U32.is_lt(i, lanternward.len(s, "foes")), s, i, now)

def lanternward.foes.provision.foes.go(more: Bool, +s: J.Val, +i: U32, +now: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.foes.provision.foes(
        lanternward.eput(s, "foes", i, lanternward.foes.provision.one(lanternward.el(s, "foes", i), now, False{})),
        (i + 1 : U32), now)

# Scale foes already standing to a table that has grown.
def lanternward.foes.Session.provision(+s: J.Val) -> J.Val:
  lanternward.foes.Session.provision.boss(
    lanternward.foes.provision.foes(s, 0, RS.u_clamp(lanternward.seat_count(s), 1, 6)),
    RS.u_clamp(lanternward.seat_count(s), 1, 6))

def lanternward.foes.Session.provision.boss(+s: J.Val, +now: U32) -> J.Val:
  lanternward.foes.Session.provision.boss.go(J.is_null(J.get(s, "boss")), s, now)

def lanternward.foes.Session.provision.boss.go(none: Bool, +s: J.Val, +now: U32) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      ga.sv(s, "boss", lanternward.foes.provision.one(J.get(s, "boss"), now, True{}))

# The living knight nearest a point, by distance then seat: null, or
# [id, x, y, d].
def lanternward.foes.Session.nearest_knight.go(+ids: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32, +best: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      best
    case Con{+id, +t}:
      lanternward.foes.Session.nearest_knight.cand(ga.at(seats, id), id, t, seats, x, y, best)

def lanternward.foes.Session.nearest_knight.cand(+k: J.Val, +id: U32, +t: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32, +best: J.Val) -> J.Val:
  lanternward.foes.Session.nearest_knight.pick(
    lanternward.kit.Seat.up(k), lanternward.world.dist((ga.u(k, "x") - x : U32), (ga.u(k, "y") - y : U32)), id, k, t, seats, x, y, best)

def lanternward.foes.Session.nearest_knight.pick(up: Bool, +d: U32, +id: U32, +k: J.Val, +t: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32, +best: J.Val) -> J.Val:
  lanternward.foes.Session.nearest_knight.go(t, seats, x, y,
    Bool.pick(J.Val, Bool.and(up, Bool.or(J.is_null(best), RS.i_lt(d, ga.nu(best, 3)))),
      J.JA{[J.JU{id}, J.of_i32(ga.u(k, "x")), J.of_i32(ga.u(k, "y")), J.of_i32(d)]}, best))

def lanternward.foes.Session.nearest_knight(+s: J.Val, +x: U32, +y: U32) -> J.Val:
  lanternward.foes.Session.nearest_knight.go(ga.ids(J.get(s, "seats")), J.get(s, "seats"), x, y, J.JNull{})

# Standing and awake: can be hurt, can hurt, can be a foe's target.
def lanternward.kit.Seat.up(+k: J.Val) -> Bool:
  Bool.and(U32.is_gt(ga.u(k, "hp"), 0), Bool.not(lanternward.is(k, "act", "Down")))

# The nearest open point to one that is in a wall.
def lanternward.foes.Session.nudge.dirs(+n: U32) -> U32 & U32:
  match n:
    case 0:
      (1, 0)
    case 1:
      (4294967295, 0)
    case 2:
      (0, 1)
    case 3:
      (0, 4294967295)
    case 4:
      (1, 1)
    case 5:
      (4294967295, 4294967295)
    case 6:
      (1, 4294967295)
    case _:
      (4294967295, 1)

def lanternward.foes.Session.nudge.try(d: U32 & U32, +s: J.Val, +x: U32, +y: U32, +ring: U32, +r: U32) -> J.Val:
  (+dx, +dy) = d
  lanternward.foes.Session.nudge.try.at(
    lanternward.props.Session.clear(s, (x + dx * ring * 1024 : U32), (y + dy * ring * 1024 : U32), r),
    (x + dx * ring * 1024 : U32), (y + dy * ring * 1024 : U32))

def lanternward.foes.Session.nudge.try.at(ok: Bool, +px: U32, +py: U32) -> J.Val:
  match ok:
    case True{}:
      ga.pair_i(px, py)
    case False{}:
      J.JNull{}

# ring 1..=8, then the eight directions in order: the first clear point.
def lanternward.foes.Session.nudge.go(+k: U32, +s: J.Val, +x: U32, +y: U32, +r: U32) -> J.Val:
  lanternward.foes.Session.nudge.next(U32.is_ge(k, 64), k, s, x, y, r)

def lanternward.foes.Session.nudge.next(done: Bool, +k: U32, +s: J.Val, +x: U32, +y: U32, +r: U32) -> J.Val:
  match done:
    case True{}:
      ga.pair_i(x, y)
    case False{}:
      lanternward.foes.Session.nudge.some(
        lanternward.foes.Session.nudge.try(
          lanternward.foes.Session.nudge.dirs(U32.mod(k, 8)), s, x, y, (U32.div(k, 8) + 1 : U32), r),
        k, s, x, y, r)

def lanternward.foes.Session.nudge.some(+hit: J.Val, +k: U32, +s: J.Val, +x: U32, +y: U32, +r: U32) -> J.Val:
  match hit:
    case J.JNull{}:
      lanternward.foes.Session.nudge.go((k + 1 : U32), s, x, y, r)
    case _:
      hit

def lanternward.foes.Session.nudge(+s: J.Val, +x: U32, +y: U32, +r: U32) -> J.Val:
  lanternward.foes.Session.nudge.go(0, s, x, y, r)

# ---- one snuffer / turret tick, on `$f` ----

def lanternward.foes.drift_dir(+d: U32, +ux: U32, +uy: U32, +id: U32) -> U32 & U32:
  Bool.pick(U32 & U32, RS.i_gt(d, 6656), (ux, uy),
    Bool.pick(U32 & U32, RS.i_lt(d, 4608), (RS.i_neg(ux), RS.i_neg(uy)),
      Bool.pick(U32 & U32, U32.is_eq(U32.mod(id, 2), 0), (RS.i_neg(uy), ux), (uy, RS.i_neg(ux)))))

def lanternward.foes.snuffer.drift(+s: J.Val, +tx: U32, +ty: U32, +d: U32) -> J.Val:
  lanternward.foes.snuffer.drift.go(lanternward.world.unit((tx - lanternward.fu(s, "x") : U32), (ty - lanternward.fu(s, "y") : U32)), s, tx, ty, d)

def lanternward.foes.snuffer.drift.go(u: U32 & U32, +s: J.Val, +tx: U32, +ty: U32, +d: U32) -> J.Val:
  (+ux, +uy) = u
  lanternward.foes.snuffer.drift.move(lanternward.foes.drift_dir(d, ux, uy, lanternward.fu(s, "id")),
    lanternward.fsi(lanternward.fsi(s, "fx", ux), "fy", uy), tx, ty, d)

def lanternward.foes.snuffer.drift.move(m: U32 & U32, +s: J.Val, +tx: U32, +ty: U32, +d: U32) -> J.Val:
  (+mx, +my) = m
  lanternward.foes.snuffer.drift.lock(
    lanternward.fs(
      lanternward.fsi(lanternward.fsi(s, "x", (lanternward.fu(s, "x") + RS.i_div((mx * 200 : U32), 256) : U32)),
        "y", (lanternward.fu(s, "y") + RS.i_div((my * 200 : U32), 256) : U32)),
      "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)),
    tx, ty, d)

def lanternward.foes.snuffer.drift.lock(+s: J.Val, +tx: U32, +ty: U32, +d: U32) -> J.Val:
  lanternward.foes.snuffer.drift.lock.at(
    Bool.and(U32.is_eq(lanternward.fu(s, "t"), 0), RS.i_lt(d, 12800)), U32.is_eq(lanternward.fu(s, "t"), 0), s, tx, ty)

def lanternward.foes.snuffer.drift.lock.at(go: Bool, zero: Bool, +s: J.Val, +tx: U32, +ty: U32) -> J.Val:
  match go:
    case True{}:
      lanternward.foes.snuffer.drift.line(lanternward.world.unit((tx - lanternward.fu(s, "x") : U32), (ty - lanternward.fu(s, "y") : U32)), s)
    case False{}:
      lanternward.foes.snuffer.drift.wait(zero, s)

def lanternward.foes.snuffer.drift.wait(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case True{}:
      lanternward.fs(s, "t", 8)
    case False{}:
      s

def lanternward.foes.snuffer.drift.line(l: U32 & U32, +s: J.Val) -> J.Val:
  (+lx, +ly) = l
  lanternward.fs(
    lanternward.fsn(
      lanternward.fs(
        lanternward.fsv(
          lanternward.fsi(lanternward.fsi(s, "fx", lx), "fy", ly), "danger",
          lanternward.variant("Capsule",
            ga.si(ga.si(ga.si(ga.si(ga.si(J.obj(), "ax", lanternward.fu(s, "x")), "ay", lanternward.fu(s, "y")),
              "bx", (lanternward.fu(s, "x") + lx * 44 : U32)), "by", (lanternward.fu(s, "y") + ly * 44 : U32)), "r", 1792))),
        "t", 24),
      "st", "Tell"),
    "struck", 0)

# One tick of a snuffer: drift to a ring about the nearest knight, show the
# line it will lunge along, lunge, tire. `target` is null or [id, x, y, d];
# a strike is pushed on `$strikes`.
def lanternward.foes.step_snuffer(+s: J.Val, +target: J.Val, +index: U32) -> J.Val:
  match target:
    case J.JNull{}:
      s
    case _:
      lanternward.foes.step_snuffer.st(
        ga.s(J.get(s, "$f"), "st"), s, ga.nu(target, 1), ga.nu(target, 2), ga.nu(target, 3), index)

def lanternward.foes.step_snuffer.st(+st: String, +s: J.Val, +tx: U32, +ty: U32, +d: U32, +index: U32) -> J.Val:
  lanternward.foes.step_snuffer.code(
    Bool.pick(U32, String.eq(st, "Dormant"), 1,
      Bool.pick(U32, String.eq(st, "Drift"), 2,
        Bool.pick(U32, String.eq(st, "Tell"), 3,
          Bool.pick(U32, String.eq(st, "Strike"), 4,
            Bool.pick(U32, String.eq(st, "Recover"), 5,
              Bool.pick(U32, String.eq(st, "Stagger"), 6, 0)))))),
    s, tx, ty, d, index)

def lanternward.foes.step_snuffer.code(+code: U32, +s: J.Val, +tx: U32, +ty: U32, +d: U32, +index: U32) -> J.Val:
  match code:
    case 1:
      lanternward.foes.step_snuffer.wake(
        Bool.or(RS.i_lt(d, 30720), U32.is_lt(lanternward.fu(s, "hp"), lanternward.fu(s, "full"))), s)
    case 2:
      lanternward.foes.snuffer.drift(s, tx, ty, d)
    case 3:
      lanternward.foes.step_snuffer.tell(U32.is_gt(lanternward.fu(s, "flinch"), 0), s)
    case 4:
      lanternward.foes.step_snuffer.strike(
        lanternward.fsi(lanternward.fsi(s, "x", (lanternward.fu(s, "x") + RS.i_div((lanternward.fu(s, "fx") * 1400 : U32), 256) : U32)),
          "y", (lanternward.fu(s, "y") + RS.i_div((lanternward.fu(s, "fy") * 1400 : U32), 256) : U32)),
        index)
    case 5:
      lanternward.foes.step_snuffer.recover(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case 6:
      lanternward.foes.step_snuffer.stagger(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case _:
      s

def lanternward.foes.step_snuffer.wake(wake: Bool, +s: J.Val) -> J.Val:
  match wake:
    case True{}:
      lanternward.fs(lanternward.fsn(s, "st", "Drift"), "t", (18 + U32.mod(lanternward.fu(s, "id"), 5) * 4 : U32))
    case False{}:
      s

def lanternward.foes.step_snuffer.tell(flinch: Bool, +s: J.Val) -> J.Val:
  match flinch:
    case True{}:
      lanternward.fs(lanternward.fsv(lanternward.fsn(s, "st", "Drift"), "danger", J.JS{"None"}), "t", 30)
    case False{}:
      lanternward.foes.step_snuffer.tell.tick(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))

def lanternward.foes.step_snuffer.tell.tick(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case True{}:
      lanternward.fs(lanternward.fsn(s, "st", "Strike"), "t", 8)
    case False{}:
      s

def lanternward.foes.step_snuffer.strike(+s: J.Val, +index: U32) -> J.Val:
  lanternward.foes.step_snuffer.strike.end(
    U32.is_le(lanternward.fu(s, "t"), 1),
    lanternward.fs(
      lanternward.push(s, "$strikes", ga.trio(J.JU{index}, lanternward.fv(s, "danger"), J.JU{2})),
      "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))

def lanternward.foes.step_snuffer.strike.end(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case True{}:
      lanternward.fs(lanternward.fsv(lanternward.fsn(s, "st", "Recover"), "danger", J.JS{"None"}), "t", 28)
    case False{}:
      s

def lanternward.foes.step_snuffer.recover(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case True{}:
      lanternward.fs(
        lanternward.fs(lanternward.fsn(s, "st", "Drift"), "t",
          (22 + U32.mod(lanternward.fu(s, "id"), 4) * 6 + U32.mod(lanternward.fu(s, "n"), 3) * 4 : U32)),
        "n", U32.and((lanternward.fu(s, "n") + 1 : U32), 255))
    case False{}:
      s

def lanternward.foes.step_snuffer.stagger(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case True{}:
      lanternward.fs(lanternward.fsb(lanternward.fsn(s, "st", "Drift"), "open", False{}), "t", 24)
    case False{}:
      s

# One tick of a turret: wait for a knight in sight, lock the line, loose an
# iron bolt along it. Shots go on `$shots`.
def lanternward.foes.step_turret(+s: J.Val, +target: J.Val, +seats: U32) -> J.Val:
  lanternward.foes.step_turret.st(ga.s(J.get(s, "$f"), "st"), s, target, seats)

def lanternward.foes.step_turret.st(+st: String, +s: J.Val, +target: J.Val, +seats: U32) -> J.Val:
  lanternward.foes.step_turret.code(
    Bool.pick(U32, lanternward.NAME_IN(st, ["Dormant", "Drift"]), 1,
      Bool.pick(U32, String.eq(st, "Tell"), 2,
        Bool.pick(U32, String.eq(st, "Strike"), 3,
          Bool.pick(U32, String.eq(st, "Recover"), 4,
            Bool.pick(U32, String.eq(st, "Stagger"), 5, 0))))),
    s, target, seats)

def lanternward.foes.step_turret.code(+code: U32, +s: J.Val, +target: J.Val, +seats: U32) -> J.Val:
  match code:
    case 1:
      lanternward.foes.step_turret.look(J.is_null(target), s, target, seats)
    case 2:
      lanternward.foes.step_turret.tell(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case 3:
      lanternward.foes.step_turret.strike(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case 4:
      lanternward.foes.step_turret.recover(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case 5:
      lanternward.foes.step_turret.stagger(U32.is_le(lanternward.fu(s, "t"), 1), lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)))
    case _:
      s

def lanternward.foes.step_turret.look(none: Bool, +s: J.Val, +target: J.Val, +seats: U32) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.foes.step_turret.aim(
        lanternward.fs(s, "t", RS.u_sat_sub(lanternward.fu(s, "t"), 1)), ga.nu(target, 1), ga.nu(target, 2), ga.nu(target, 3), seats)

def lanternward.foes.step_turret.aim(+s: J.Val, +tx: U32, +ty: U32, +d: U32, +seats: U32) -> J.Val:
  lanternward.foes.step_turret.aim.go(
    Bool.and(RS.i_lt(d, 66560), U32.is_eq(lanternward.fu(s, "t"), 0)),
    s, lanternward.world.unit((tx - lanternward.fu(s, "x") : U32), (ty - lanternward.fu(s, "y") : U32)), seats)

def lanternward.foes.step_turret.aim.go(fire: Bool, +s: J.Val, u: U32 & U32, +seats: U32) -> J.Val:
  match fire:
    case False{}:
      s
    case True{}:
      lanternward.foes.step_turret.aim.fire(u, s, seats)

def lanternward.foes.step_turret.aim.fire(u: U32 & U32, +s: J.Val, +seats: U32) -> J.Val:
  (+ux, +uy) = u
  lanternward.fs(
    lanternward.fsn(
      lanternward.fsv(
        lanternward.fsi(lanternward.fsi(s, "fx", ux), "fy", uy), "danger",
        lanternward.variant("Capsule",
          ga.si(ga.si(ga.si(ga.si(ga.si(J.obj(), "ax", lanternward.fu(s, "x")), "ay", lanternward.fu(s, "y")),
            "bx", (lanternward.fu(s, "x") + ux * 260 : U32)), "by", (lanternward.fu(s, "y") + uy * 260 : U32)), "r", 1280))),
      "st", "Tell"),
    "t", lanternward.foes.Foe.tell(J.get(s, "$f"), J.get(s, "$r"), seats))

def lanternward.foes.step_turret.tell(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.sound(
        lanternward.push(
          lanternward.fs(lanternward.fsn(s, "st", "Strike"), "t", 6), "$shots",
          ga.su(
            ga.su(
              ga.si(
                ga.si(
                  ga.si(
                    ga.si(ga.sv(J.obj(), "kind", J.JS{"Iron"}), "x", (lanternward.fu(s, "x") + lanternward.fu(s, "fx") * 10 : U32)),
                    "y", (lanternward.fu(s, "y") + lanternward.fu(s, "fy") * 10 : U32)),
                  "vx", RS.i_div((lanternward.fu(s, "fx") * 620 : U32), 256)),
                "vy", RS.i_div((lanternward.fu(s, "fy") * 620 : U32), 256)),
              "ttl", 160),
            "owner", 0)),
        "shot_bolt")

def lanternward.foes.step_turret.strike(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.fs(lanternward.fsv(lanternward.fsn(s, "st", "Recover"), "danger", J.JS{"None"}), "t", 60)

def lanternward.foes.step_turret.recover(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.fs(lanternward.fsn(s, "st", "Dormant"), "t", 0)

def lanternward.foes.step_turret.stagger(zero: Bool, +s: J.Val) -> J.Val:
  match zero:
    case False{}:
      s
    case True{}:
      lanternward.fsb(lanternward.fsn(s, "st", "Dormant"), "open", False{})

# The per-foe preamble and dispatch of `step_foes`: foe `i` rides in `$f`.
def lanternward.foes.step_foes.live(+s: J.Val, +i: U32, +target: J.Val, +burning: Bool, +seats: U32) -> J.Val:
  +s1 = lanternward.fs(s, "flinch", RS.u_sat_sub(lanternward.fu(s, "flinch"), 1))
  +s2 = lanternward.fs(s1, "lit", Bool.pick(U32, burning, 60, RS.u_sat_sub(lanternward.fu(s1, "lit"), 1)))
  +s3 = lanternward.foes.step_foes.kb(U32.is_gt(lanternward.fu(s2, "kbt"), 0), s2)
  lanternward.foes.step_foes.kind(lanternward.fis(s3, "kind", "Snuffer"), s3, i, target, seats)

def lanternward.foes.step_foes.kb(moving: Bool, +s: J.Val) -> J.Val:
  match moving:
    case False{}:
      s
    case True{}:
      lanternward.fs(
        lanternward.fsi(
          lanternward.fsi(s, "x", (lanternward.fu(s, "x") + ga.nu(lanternward.fv(s, "kb"), 0) : U32)),
          "y", (lanternward.fu(s, "y") + ga.nu(lanternward.fv(s, "kb"), 1) : U32)),
        "kbt", (lanternward.fu(s, "kbt") - 1 : U32))

def lanternward.foes.step_foes.kind(snuffer: Bool, +s: J.Val, +i: U32, +target: J.Val, +seats: U32) -> J.Val:
  match snuffer:
    case True{}:
      lanternward.foes.step_snuffer(s, target, i)
    case False{}:
      lanternward.foes.step_turret(s, target, seats)

# Foe `i`'s turn: sleeping, frozen and dying foes skip the rest.
def lanternward.foes.step_foes.one(+s: J.Val, +i: U32, +seats: U32) -> J.Val:
  +f = lanternward.el(s, "foes", i)
  +target = lanternward.foes.Session.nearest_knight(s, ga.u(f, "x"), ga.u(f, "y"))
  +burning = lanternward.props.Session.burning(s, ga.u(f, "x"), ga.u(f, "y"))
  lanternward.foes.step_foes.stash(ga.sv(s, "$f", f), i, target, burning, seats)

def lanternward.foes.step_foes.stash(+s: J.Val, +i: U32, +target: J.Val, +burning: Bool, +seats: U32) -> J.Val:
  lanternward.foes.step_foes.put(
    lanternward.foes.step_foes.pre(lanternward.fis(s, "st", "Dying"), s, i, target, burning, seats), i)

def lanternward.foes.step_foes.put(+s: J.Val, +i: U32) -> J.Val:
  J.del(lanternward.eput(s, "foes", i, J.get(s, "$f")), "$f")

def lanternward.foes.step_foes.pre(dying: Bool, +s: J.Val, +i: U32, +target: J.Val, +burning: Bool, +seats: U32) -> J.Val:
  match dying:
    case True{}:
      lanternward.fs(s, "dead", RS.u_min((lanternward.fu(s, "dead") + 1 : U32), 255))
    case False{}:
      lanternward.foes.step_foes.frozen(U32.is_gt(lanternward.fu(s, "freeze"), 0), s, i, target, burning, seats)

def lanternward.foes.step_foes.frozen(frozen: Bool, +s: J.Val, +i: U32, +target: J.Val, +burning: Bool, +seats: U32) -> J.Val:
  match frozen:
    case True{}:
      lanternward.fs(s, "freeze", (lanternward.fu(s, "freeze") - 1 : U32))
    case False{}:
      lanternward.foes.step_foes.live(s, i, target, burning, seats)

def lanternward.foes.step_foes.loop(+s: J.Val, +i: U32, +seats: U32) -> J.Val:
  lanternward.foes.step_foes.loop.go(U32.is_lt(i, lanternward.len(s, "foes")), s, i, seats)

def lanternward.foes.step_foes.loop.go(more: Bool, +s: J.Val, +i: U32, +seats: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.foes.step_foes.loop(lanternward.foes.step_foes.one(s, i, seats), (i + 1 : U32), seats)

# Keep a snuffer out of the walls it drifts into.
def lanternward.foes.step_foes.unstick(+s: J.Val, +i: U32) -> J.Val:
  lanternward.foes.step_foes.unstick.go(U32.is_lt(i, lanternward.len(s, "foes")), s, i)

def lanternward.foes.step_foes.unstick.go(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.foes.step_foes.unstick(
        lanternward.foes.step_foes.unstick.one(
          Bool.and(lanternward.eis(s, "foes", i, "kind", "Snuffer"),
            Bool.not(lanternward.props.Session.clear(s, lanternward.eu(s, "foes", i, "x"), lanternward.eu(s, "foes", i, "y"), 1024))),
          s, i),
        (i + 1 : U32))

def lanternward.foes.step_foes.unstick.one(stuck: Bool, +s: J.Val, +i: U32) -> J.Val:
  match stuck:
    case False{}:
      s
    case True{}:
      lanternward.foes.step_foes.unstick.set(
        lanternward.foes.Session.nudge(s, lanternward.eu(s, "foes", i, "x"), lanternward.eu(s, "foes", i, "y"), 1024), s, i)

def lanternward.foes.step_foes.unstick.set(+p: J.Val, +s: J.Val, +i: U32) -> J.Val:
  lanternward.esi(lanternward.esi(s, "foes", i, "x", ga.nu(p, 0)), "foes", i, "y", ga.nu(p, 1))

# New shots join the bolts while there is room.
def lanternward.foes.step_foes.shots(+s: J.Val, +xs: List<&2, J.Val>) -> J.Val:
  match xs:
    case Nil{}:
      s
    case Con{+b, +t}:
      lanternward.foes.step_foes.shots(
        lanternward.foes.step_foes.shots.one(U32.is_lt(lanternward.len(s, "bolts"), 40), s, b), t)

def lanternward.foes.step_foes.shots.one(room: Bool, +s: J.Val, +b: J.Val) -> J.Val:
  match room:
    case True{}:
      lanternward.push(s, "bolts", b)
    case False{}:
      s

# Each foe's strike lands on every knight standing in its footprint.
def lanternward.foes.step_foes.strikes(+s: J.Val, +xs: List<&2, J.Val>) -> J.Val:
  match xs:
    case Nil{}:
      s
    case Con{+st, +t}:
      lanternward.foes.step_foes.strikes(
        lanternward.foes.Session.strike_with(s, ga.nu(st, 0), J.nth(st, 1), ga.nu(st, 2)), t)

# One tick of every foe.
def lanternward.foes.Session.step_foes(+s: J.Val) -> J.Val:
  lanternward.foes.step_foes.fin(
    lanternward.foes.step_foes.unstick(
      lanternward.foes.step_foes.loop(
        ga.sv(ga.sv(s, "$strikes", J.JA{Nil{}}), "$shots", J.JA{Nil{}}), 0, lanternward.seat_count(s)),
      0))

def lanternward.foes.step_foes.fin(+s: J.Val) -> J.Val:
  lanternward.foes.step_foes.retain(
    J.del(J.del(
      lanternward.foes.step_foes.strikes(
        lanternward.foes.step_foes.shots(s, ga.items(s, "$shots")), ga.items(s, "$strikes")),
      "$shots"), "$strikes"))

# Those that have puffed out are gone.
def lanternward.foes.step_foes.retain.go(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Bool.pick(List<&2, J.Val>,
        Bool.and(lanternward.is(f, "st", "Dying"), U32.is_ge(ga.u(f, "dead"), 14)),
        lanternward.foes.step_foes.retain.go(t),
        Con{f, lanternward.foes.step_foes.retain.go(t)})

def lanternward.foes.step_foes.retain(+s: J.Val) -> J.Val:
  ga.sv(s, "foes", J.JA{lanternward.foes.step_foes.retain.go(ga.items(s, "foes"))})

# A foe's blow lands on every knight standing in its footprint that it has
# not yet struck.
def lanternward.foes.Session.strike_with.ids(+ids: List<&2, U32>, +s: J.Val, +i: U32, +danger: J.Val, +dmg: U32) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.foes.Session.strike_with.ids(t, lanternward.foes.Session.strike_with.seat(s, id, i, danger, dmg), i, danger, dmg)

def lanternward.foes.Session.strike_with.seat(+s: J.Val, +id: U32, +i: U32, +danger: J.Val, +dmg: U32) -> J.Val:
  +k = ga.at(J.get(s, "seats"), id)
  +bit = U32.shln(1, U32.to_nat(RS.u_min(id, 7)))
  lanternward.foes.Session.strike_with.hit(
    Bool.and(
      Bool.and(U32.is_eq(U32.and(lanternward.eu(s, "foes", i, "struck"), bit), 0), lanternward.kit.Seat.up(k)),
      lanternward.foes.Danger.hits(danger, ga.u(k, "x"), ga.u(k, "y"), 1280)),
    s, id, i, bit, dmg)

def lanternward.foes.Session.strike_with.hit(hit: Bool, +s: J.Val, +id: U32, +i: U32, +bit: U32, +dmg: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.foes.Session.strike_with.after(
        lanternward.blow.Session.blow(
          lanternward.es(s, "foes", i, "struck", U32.or(lanternward.eu(s, "foes", i, "struck"), bit)),
          id, dmg, lanternward.eu(s, "foes", i, "x"), lanternward.eu(s, "foes", i, "y"), "Melee"),
        i)

def lanternward.foes.Session.strike_with.after(+s: J.Val, +i: U32) -> J.Val:
  lanternward.foes.Session.strike_with.res(ga.s(s, "$ret"), J.del(s, "$ret"), i)

def lanternward.foes.Session.strike_with.res(+out: String, +s: J.Val, +i: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(out, "Parried"),
    lanternward.stop(
      lanternward.es(
        lanternward.esb(
          lanternward.eset(lanternward.es(lanternward.eset(s, "foes", i, "st", J.JS{"Stagger"}), "foes", i, "t", 30), "foes", i, "danger", J.JS{"None"}),
          "foes", i, "open", True{}),
        "foes", i, "freeze", 3),
      "Phase"),
    Bool.pick(J.Val, String.eq(out, "Blocked"),
      lanternward.es(
        lanternward.eset(s, "foes", i, "kb",
          ga.pair_i(
            RS.i_div(RS.i_neg(lanternward.eu(s, "foes", i, "fx")), 40),
            RS.i_div(RS.i_neg(lanternward.eu(s, "foes", i, "fy")), 40))),
        "foes", i, "kbt", 6),
      s))

def lanternward.foes.Session.strike_with(+s: J.Val, +i: U32, +danger: J.Val, +dmg: U32) -> J.Val:
  lanternward.foes.Session.strike_with.ids(ga.ids(J.get(s, "seats")), s, i, danger, dmg)
