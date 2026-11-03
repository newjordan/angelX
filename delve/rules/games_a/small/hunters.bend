# crates/delve/src/rules/hunters.rs — the deep's hunters: the Flesher's hook and
# the Silkmother's webs. `Deeds` is the J object of foes.rs with the Rust field
# names (tuples as arrays); `heroes` is the slice of (x, y) as a list of pairs.

def hunters.HOOK_RANGE() -> F32:
  15.0

def hunters.HOOK_SPEED() -> F32:
  22.0

def hunters.REEL_SPEED() -> F32:
  16.0

def hunters.HOOK_WINDUP() -> U32:
  20

def hunters.HOOK_EVERY() -> U32:
  120

def hunters.HOOK_DAMAGE() -> U32:
  20

def hunters.ROT_REACH() -> F32:
  2.6

def hunters.ROT_EVERY() -> U32:
  8

def hunters.WEB_REACH() -> F32:
  2.6

def hunters.WEB_LASTS() -> U32:
  240

def hunters.WEB_PACE() -> F32:
  0.6

def hunters.BROOD() -> U32:
  6

# The nearest hero (first of equals) as a Maybe pair.
def hunters.act.nearest(+hs: List<&2, Sigma<&2, &2, F32, _ => F32>>, +ex: F32, +ey: F32, +best: Maybe<&2, Sigma<&2, &2, F32, _ => F32>>) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match hs:
    case Nil{}:
      best
    case Con{(+hx, +hy), +t}:
      hunters.act.nearest(t, ex, ey, hunters.act.nearest.pick(best, hx, hy, ex, ey))

def hunters.act.nearest.pick(b: Maybe<&2, Sigma<&2, &2, F32, _ => F32>>, +hx: F32, +hy: F32, +ex: F32, +ey: F32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match b:
    case None{}:
      Some{(hx, hy)}
    case Some{(+bx, +by)}:
      hunters.act.nearest.keep(
        F32.is_gt(RS.f_hypot(F32.sub(bx, ex), F32.sub(by, ey)), RS.f_hypot(F32.sub(hx, ex), F32.sub(hy, ey))),
        bx, by, hx, hy)

def hunters.act.nearest.keep(replace: Bool, +bx: F32, +by: F32, +hx: F32, +hy: F32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match replace:
    case True{}:
      Some{(hx, hy)}
    case False{}:
      Some{(bx, by)}

# `walk`: the enemy slides toward `(dx, dy)` at `speed`.
def hunters.act.walk(+enemy: J.Val, +g: J.Val, +dx: F32, +dy: F32, +speed: F32) -> J.Val:
  hunters.act.walk.put(enemy, rules.Grid.slide(J.get(g, "room"), ga.b(g, "barred"), (ga.f(enemy, "x"), ga.f(enemy, "y")),
    (F32.mul(F32.mul(dx, speed), rules.DT()), F32.mul(F32.mul(dy, speed), rules.DT())),
    monsters.Enemy.radius(enemy), 1))

def hunters.act.walk.put(+enemy: J.Val, p: F32 & F32) -> J.Val:
  (x, y) = p
  ga.sf(ga.sf(enemy, "x", x), "y", y)

# One of the third company's steps. The enemy and the deeds after it, and false
# (nothing changed) for any other monster.
def hunters.act(+enemy: J.Val, +heroes: List<&2, Sigma<&2, &2, F32, _ => F32>>, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  hunters.act.kind(ga.s(enemy, "kind"), enemy, heroes, g, deeds)

def hunters.act.kind(+k: String, +enemy: J.Val, +heroes: List<&2, Sigma<&2, &2, F32, _ => F32>>, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  hunters.act.kind.is(Bool.or(Bool.or(String.eq(k, "flesher"), String.eq(k, "silkmother")), String.eq(k, "spiderling")), k, enemy, heroes, g, deeds)

def hunters.act.kind.is(hunter: Bool, +k: String, +enemy: J.Val, +heroes: List<&2, Sigma<&2, &2, F32, _ => F32>>, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  match hunter:
    case False{}:
      (enemy, deeds, False{})
    case True{}:
      hunters.act.aim(hunters.act.nearest(heroes, ga.f(enemy, "x"), ga.f(enemy, "y"), None{}), k, enemy, g, deeds)

def hunters.act.aim(t: Maybe<&2, Sigma<&2, &2, F32, _ => F32>>, +k: String, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  match t:
    case None{}:
      hunters.act.go(False{}, ga.f(enemy, "x"), ga.f(enemy, "y"), k, enemy, g, deeds)
    case Some{(+tx, +ty)}:
      hunters.act.go(True{}, tx, ty, k, enemy, g, deeds)

def hunters.act.go(+has: Bool, +tx: F32, +ty: F32, +k: String, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  hunters.act.go2(has, tx, ty, rules.unit(F32.sub(tx, ga.f(enemy, "x")), F32.sub(ty, ga.f(enemy, "y"))),
    RS.f_hypot(F32.sub(tx, ga.f(enemy, "x")), F32.sub(ty, ga.f(enemy, "y"))), k,
    ga.su(enemy, "timer", (ga.u(enemy, "timer") + 1 : U32)), g, deeds)

def hunters.act.go2(+has: Bool, +tx: F32, +ty: F32, c: F32 & F32, +dist: F32, +k: String, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  (cx, cy) = c
  hunters.act.run(has, tx, ty, cx, cy, dist, k, enemy, (ga.u(enemy, "timer") + (ga.u(enemy, "id") * 7 : U32) : U32), g, deeds)

def hunters.act.run(+has: Bool, +tx: F32, +ty: F32, +cx: F32, +cy: F32, +dist: F32, +k: String, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  hunters.act.out(
    hunters.act.run.kind(String.eq(k, "flesher"), String.eq(k, "silkmother"), has, tx, ty, cx, cy, dist, enemy, beat, g, deeds))

def hunters.act.out(r: J.Val & J.Val) -> J.Val & J.Val & Bool:
  (e, d) = r
  (e, d, True{})

def hunters.act.run.kind(flesher: Bool, silk: Bool, +has: Bool, +tx: F32, +ty: F32, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match flesher:
    case True{}:
      hunters.act.flesher(has, cx, cy, dist, enemy, beat, g, deeds)
    case False{}:
      hunters.act.run.silk(silk, has, tx, ty, cx, cy, dist, enemy, beat, g, deeds)

def hunters.act.run.silk(silk: Bool, +has: Bool, +tx: F32, +ty: F32, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match silk:
    case True{}:
      hunters.act.silkmother(has, tx, ty, cx, cy, dist, enemy, beat, g, deeds)
    case False{}:
      hunters.act.spiderling(cx, cy, enemy, g, deeds)

# --- the Flesher -----------------------------------------------------------

def hunters.act.flesher(+has: Bool, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.flesher.stage(has, cx, cy, dist, enemy,
    hunters.act.flesher.rot(U32.is_zero(U32.mod(beat, hunters.ROT_EVERY())), enemy, deeds), g)

def hunters.act.flesher.rot(on: Bool, +enemy: J.Val, +deeds: J.Val) -> J.Val:
  match on:
    case True{}:
      ga.push(deeds, "rot", ga.pair_f(ga.f(enemy, "x"), ga.f(enemy, "y")))
    case False{}:
      deeds

def hunters.act.flesher.stage(+has: Bool, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +deeds: J.Val, +g: J.Val) -> J.Val & J.Val:
  hunters.act.flesher.stage.at(ga.u(enemy, "stage"), has, cx, cy, dist, enemy, deeds, g)

def hunters.act.flesher.stage.at(+stage: U32, +has: Bool, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +deeds: J.Val, +g: J.Val) -> J.Val & J.Val:
  hunters.act.flesher.stage.pick(U32.is_eq(stage, 0), U32.is_eq(stage, 1), has, cx, cy, dist, enemy, deeds, g)

def hunters.act.flesher.stage.pick(zero: Bool, one: Bool, +has: Bool, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +deeds: J.Val, +g: J.Val) -> J.Val & J.Val:
  match zero:
    case True{}:
      hunters.act.flesher.shamble(has, cx, cy, dist, hunters.act.walk(enemy, g, cx, cy, 1.6), deeds)
    case False{}:
      hunters.act.flesher.stare(one, enemy, deeds)

# Shamble after the party; when one is in reach, stare.
def hunters.act.flesher.shamble(+has: Bool, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.flesher.shamble.go(
    Bool.and(U32.is_ge(ga.u(enemy, "timer"), (hunters.HOOK_EVERY() + (U32.mod(ga.u(enemy, "id"), 4) * 9 : U32) : U32)),
      Bool.and(has, F32.is_lt(dist, F32.sub(hunters.HOOK_RANGE(), 1.0)))),
    cx, cy, enemy, deeds)

def hunters.act.flesher.shamble.go(ready: Bool, +cx: F32, +cy: F32, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match ready:
    case True{}:
      (J.set(ga.su(ga.su(enemy, "stage", 1), "timer", 0), "dir", ga.pair_f(cx, cy)), deeds)
    case False{}:
      (enemy, deeds)

# The stare: his line shows on the floor; then the throw, then reeling in.
def hunters.act.flesher.stare(one: Bool, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match one:
    case True{}:
      hunters.act.flesher.throw(U32.is_ge(ga.u(enemy, "timer"), hunters.HOOK_WINDUP()), enemy, deeds)
    case False{}:
      hunters.act.flesher.breath(U32.is_ge(ga.u(enemy, "timer"), (2 * rules.HZ() : U32)), enemy, deeds)

def hunters.act.flesher.throw(go: Bool, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match go:
    case True{}:
      (ga.su(ga.su(enemy, "stage", 2), "timer", 0),
        ga.push(ga.push(deeds, "hooks",
          ga.trio(J.JU{ga.u(enemy, "id")}, J.JF{ga.f(enemy, "x")}, J.JA{[J.JF{ga.f(enemy, "y")}, J.get(enemy, "dir")]})),
          "cues", J.JS{"hook_thrown"}))
    case False{}:
      (enemy, deeds)

def hunters.act.flesher.breath(done: Bool, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match done:
    case True{}:
      (ga.su(ga.su(enemy, "stage", 0), "timer", 0), deeds)
    case False{}:
      (enemy, deeds)

# --- the Silkmother --------------------------------------------------------

def hunters.act.silkmother(+has: Bool, +tx: F32, +ty: F32, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.silkmother.way(F32.is_lt(dist, 8.0), F32.is_gt(dist, 13.0),
    ga.pf(U32.is_zero(U32.mod(ga.u(enemy, "id"), 2)), 1.0, F32.neg(1.0)),
    has, tx, ty, cx, cy, dist, enemy, beat, g, deeds)

def hunters.act.silkmother.way(+near: Bool, +far: Bool, +side: F32, +has: Bool, +tx: F32, +ty: F32, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.silkmother.step(
    ga.pf(near, F32.neg(cx), ga.pf(far, cx, F32.mul(F32.neg(cy), side))),
    ga.pf(near, F32.neg(cy), ga.pf(far, cy, F32.mul(cx, side))),
    has, tx, ty, dist, enemy, beat, g, deeds)

def hunters.act.silkmother.step(+wx: F32, +wy: F32, +has: Bool, +tx: F32, +ty: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.silkmother.web(has, tx, ty, dist, beat, hunters.act.walk(enemy, g, wx, wy, 2.4), deeds)

def hunters.act.silkmother.web(+has: Bool, +tx: F32, +ty: F32, +dist: F32, +beat: U32, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.silkmother.brood(beat, enemy,
    hunters.act.silkmother.web.on(Bool.and(U32.is_eq(U32.mod(beat, 100), 50), Bool.and(has, F32.is_lt(dist, 15.0))), tx, ty, deeds))

def hunters.act.silkmother.web.on(on: Bool, +tx: F32, +ty: F32, +deeds: J.Val) -> J.Val:
  match on:
    case True{}:
      ga.push(deeds, "webs", ga.pair_f(tx, ty))
    case False{}:
      deeds

def hunters.act.silkmother.brood(+beat: U32, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.silkmother.brood.on(U32.is_eq(U32.mod(beat, 180), 90), enemy, deeds)

def hunters.act.silkmother.brood.on(hatch: Bool, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match hatch:
    case True{}:
      (enemy, ga.push(hunters.act.silkmother.hatch(enemy, deeds), "cues", J.JS{"brood_hatch"}))
    case False{}:
      (enemy, deeds)

def hunters.act.silkmother.hatch(+enemy: J.Val, +deeds: J.Val) -> J.Val:
  ga.push(ga.push(deeds, "spawns", hunters.act.silkmother.spawn(enemy, F32.neg(1.0))), "spawns", hunters.act.silkmother.spawn(enemy, 1.0))

def hunters.act.silkmother.spawn(+enemy: J.Val, +side: F32) -> J.Val:
  J.JA{[J.JS{"spiderling"}, J.JF{F32.add(ga.f(enemy, "x"), F32.mul(side, 1.4))}, J.JF{F32.add(ga.f(enemy, "y"), 0.8)}, J.JU{0}]}

# --- the spiderling scuttles straight in, zig-zagging ------------------------

def hunters.act.spiderling(+cx: F32, +cy: F32, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.spiderling.zig(F32.mul(F32.sin(F32.add(F32.mul(RS.u_as_f32(ga.u(enemy, "age")), 0.5), RS.u_as_f32(ga.u(enemy, "id")))), 0.6), cx, cy, enemy, g, deeds)

def hunters.act.spiderling.zig(+zig: F32, +cx: F32, +cy: F32, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.spiderling.walk(rules.unit(F32.sub(cx, F32.mul(cy, zig)), F32.add(cy, F32.mul(cx, zig))), enemy, g, deeds)

def hunters.act.spiderling.walk(u: F32 & F32, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  (ux, uy) = u
  (hunters.act.walk(enemy, g, ux, uy, 5.4), deeds)

# --- the Run's hooks -------------------------------------------------------

def hunters.Run.hunt.hooks(+run: J.Val, +hs: List<&2, J.Val>) -> J.Val:
  match hs:
    case Nil{}:
      run
    case Con{+h, +t}:
      hunters.Run.hunt.hooks(hunters.Run.hunt.hook(U32.is_lt(J.len(J.get(run, "hooks")), 4), run, h), t)

def hunters.Run.hunt.hook(room: Bool, +run: J.Val, +h: J.Val) -> J.Val:
  match room:
    case False{}:
      run
    case True{}:
      ga.push(run, "hooks", hunters.Hook.new(h))

# A fresh hook from a Deeds entry (by, x, y, dir).
def hunters.Hook.new(+h: J.Val) -> J.Val:
  ga.sv(ga.sb(ga.sf(ga.sv(ga.sf(ga.sf(ga.su(J.obj(), "by", J.u32(J.nth(h, 0))), "x", J.f32(J.nth(h, 1))), "y", J.f32(J.nth(h, 2))), "dir", J.nth(h, 3)), "out", 0.0), "back", False{}), "caught", J.JNull{})

def hunters.Run.hunt.webs(+run: J.Val, +ws: List<&2, J.Val>) -> J.Val:
  match ws:
    case Nil{}:
      run
    case Con{+w, +t}:
      hunters.Run.hunt.webs(hunters.Run.hunt.web(run, w), t)

def hunters.Run.hunt.web(+run: J.Val, +w: J.Val) -> J.Val:
  hunters.Run.hunt.web.put(
    ga.sv(run, "webs", hunters.Run.hunt.web.cap(U32.is_ge(J.len(J.get(run, "webs")), 6), J.get(run, "webs"))), w)

def hunters.Run.hunt.web.cap(full: Bool, +webs: J.Val) -> J.Val:
  match full:
    case True{}:
      J.JA{ga.remove_nth(J.items(webs), 0n)}
    case False{}:
      webs

def hunters.Run.hunt.web.put(+run: J.Val, +w: J.Val) -> J.Val:
  ga.push(run, "webs", ga.su(ga.sf(ga.sf(J.obj(), "x", J.f32(J.nth(w, 0))), "y", J.f32(J.nth(w, 1))), "left", hunters.WEB_LASTS()))

# The rot: every targetable knight above one health within reach loses one.
def hunters.Run.hunt.rot(+run: J.Val, +rs: List<&2, J.Val>) -> J.Val:
  match rs:
    case Nil{}:
      run
    case Con{+r, +t}:
      hunters.Run.hunt.rot(hunters.Run.hunt.rot.one(run, J.f32(J.nth(r, 0)), J.f32(J.nth(r, 1)), J.int_fields(J.get(run, "players")), J.get(run, "players")), t)

def hunters.Run.hunt.rot.one(+run: J.Val, +x: F32, +y: F32, +hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val) -> J.Val:
  match hs:
    case Nil{}:
      J.set(run, "players", players)
    case Con{(+k, +h), +t}:
      hunters.Run.hunt.rot.hero(
        Bool.and(U32.is_gt(ga.u(h, "hp"), 1), peasant.Hero.targetable(h)),
        F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)), hunters.ROT_REACH()),
        run, x, y, t, players, k, h)

def hunters.Run.hunt.rot.hero(+live: Bool, +near: Bool, +run: J.Val, +x: F32, +y: F32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +k: String, +h: J.Val) -> J.Val:
  hunters.Run.hunt.rot.hit(Bool.and(live, near), run, x, y, t, players, k, h)

def hunters.Run.hunt.rot.hit(hit: Bool, +run: J.Val, +x: F32, +y: F32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +k: String, +h: J.Val) -> J.Val:
  match hit:
    case True{}:
      hunters.Run.hunt.rot.one(ga.sb(run, "room_hurt", True{}), x, y, t, J.set(players, k, ga.su(h, "hp", (ga.u(h, "hp") - 1 : U32))))
    case False{}:
      hunters.Run.hunt.rot.one(run, x, y, t, players)

# Hooks thrown, webs spun, the rot: make what the hunters did so.
def hunters.Run.hunt(+run: J.Val, +hooks: J.Val, +webs: J.Val, +rot: J.Val) -> J.Val:
  hunters.Run.hunt.rot(hunters.Run.hunt.webs(hunters.Run.hunt.hooks(run, J.items(hooks)), J.items(webs)), J.items(rot))

# Webs fade: a tick off each, the spent ones go.
def hunters.Run.tick_hunters.webs(+ws: List<&2, J.Val>) -> List<&2, J.Val>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      hunters.Run.tick_hunters.webs.keep(ga.ssub(ga.u(w, "left"), 1), w, hunters.Run.tick_hunters.webs(t))

def hunters.Run.tick_hunters.webs.keep(+left: U32, +w: J.Val, rest: List<&2, J.Val>) -> List<&2, J.Val>:
  hunters.Run.tick_hunters.webs.keep.if(U32.is_zero(left), left, w, rest)

def hunters.Run.tick_hunters.webs.keep.if(gone: Bool, +left: U32, +w: J.Val, rest: List<&2, J.Val>) -> List<&2, J.Val>:
  match gone:
    case True{}:
      rest
    case False{}:
      Con{ga.su(w, "left", left), rest}

# The Flesher a hook is his: where he stands and how big he is.
def hunters.Run.find_enemy(+es: List<&2, J.Val>, +id: U32) -> Maybe<&2, Sigma<&2, &2, F32, _ => Sigma<&2, &2, F32, _ => F32>>>:
  match es:
    case Nil{}:
      None{}
    case Con{+e, +t}:
      hunters.Run.find_enemy.hit(Bool.and(U32.is_eq(ga.u(e, "id"), id), U32.is_gt(ga.u(e, "hp"), 0)), e, t, id)

def hunters.Run.find_enemy.hit(hit: Bool, +e: J.Val, +t: List<&2, J.Val>, +id: U32) -> Maybe<&2, Sigma<&2, &2, F32, _ => Sigma<&2, &2, F32, _ => F32>>>:
  match hit:
    case True{}:
      Some{(ga.f(e, "x"), ga.f(e, "y"), monsters.Enemy.radius(e))}
    case False{}:
      hunters.Run.find_enemy(t, id)

# One hook: the run, and the hook if it stays up (none when it is done).
def hunters.Run.tick_hunters.one(+run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.owner(hunters.Run.find_enemy(J.items(J.get(run, "enemies")), ga.u(hook, "by")), run, hook)

def hunters.Run.tick_hunters.owner(m: Maybe<&2, Sigma<&2, &2, F32, _ => Sigma<&2, &2, F32, _ => F32>>>, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  match m:
    case None{}:
      (run, None{})
    case Some{(+bx, +by, +br)}:
      hunters.Run.tick_hunters.way(ga.b(hook, "back"), run, hook, bx, by, br)

def hunters.Run.tick_hunters.way(back: Bool, +run: J.Val, +hook: J.Val, +bx: F32, +by: F32, +br: F32) -> J.Val & Maybe<&2, J.Val>:
  match back:
    case True{}:
      hunters.Run.tick_hunters.reel(run, hook, bx, by, br)
    case False{}:
      hunters.Run.tick_hunters.fly(run, hook)

# --- reeling in ---------------------------------------------------------------

def hunters.Run.tick_hunters.reel(+run: J.Val, +hook: J.Val, +bx: F32, +by: F32, +br: F32) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.reel.dir(rules.unit(F32.sub(bx, ga.f(hook, "x")), F32.sub(by, ga.f(hook, "y"))), run, hook, bx, by, br)

def hunters.Run.tick_hunters.reel.dir(u: F32 & F32, +run: J.Val, +hook: J.Val, +bx: F32, +by: F32, +br: F32) -> J.Val & Maybe<&2, J.Val>:
  (ux, uy) = u
  hunters.Run.tick_hunters.reel.next(F32.add(ga.f(hook, "x"), F32.mul(ux, F32.mul(hunters.REEL_SPEED(), rules.DT()))),
    F32.add(ga.f(hook, "y"), F32.mul(uy, F32.mul(hunters.REEL_SPEED(), rules.DT()))), run, hook, bx, by, br)

def hunters.Run.tick_hunters.reel.next(+nx: F32, +ny: F32, +run: J.Val, +hook: J.Val, +bx: F32, +by: F32, +br: F32) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.reel.done(F32.is_lt(RS.f_hypot(F32.sub(bx, nx), F32.sub(by, ny)), F32.add(br, 1.0)), nx, ny, run, hook)

def hunters.Run.tick_hunters.reel.done(+home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.reel.end(home, hunters.Run.tick_hunters.reel.caught(J.get(hook, "caught"), home, nx, ny, run, hook), nx, ny)

def hunters.Run.tick_hunters.reel.end(+home: Bool, rh: J.Val & J.Val, +nx: F32, +ny: F32) -> J.Val & Maybe<&2, J.Val>:
  (run, hook) = rh
  hunters.Run.tick_hunters.reel.fin(home, run, ga.sf(ga.sf(hook, "x", nx), "y", ny))

def hunters.Run.tick_hunters.reel.fin(home: Bool, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  match home:
    case True{}:
      (run, None{})
    case False{}:
      (run, Some{hook})

# A reeled knight stops at a barred door or a wall. The hook may go on.
def hunters.Run.tick_hunters.reel.caught(+caught: J.Val, +home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  hunters.Run.tick_hunters.reel.caught.id(J.is_null(caught), J.u32(caught), home, nx, ny, run, hook)

def hunters.Run.tick_hunters.reel.caught.id(none: Bool, +id: U32, +home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  match none:
    case True{}:
      (run, hook)
    case False{}:
      hunters.Run.tick_hunters.reel.hero(ga.at(J.get(run, "players"), id), id, home, nx, ny, run, hook)

def hunters.Run.tick_hunters.reel.hero(+hero: J.Val, +id: U32, +home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  hunters.Run.tick_hunters.reel.hero.in(J.is_null(hero), hero, id, home, nx, ny, run, hook)

def hunters.Run.tick_hunters.reel.hero.in(gone: Bool, +hero: J.Val, +id: U32, +home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  match gone:
    case True{}:
      (run, hook)
    case False{}:
      hunters.Run.tick_hunters.reel.halt(
        rules.Grid.halted(rules.Run.room(run), rules.Run.barred(run),
          (ga.f(hero, "x"), ga.f(hero, "y")), (F32.sub(nx, ga.f(hero, "x")), F32.sub(ny, ga.f(hero, "y")))),
        hero, id, home, nx, ny, run, hook)

def hunters.Run.tick_hunters.reel.halt(stopped: F32 & F32, +hero: J.Val, +id: U32, +home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  (+sx, +sy) = stopped
  hunters.Run.tick_hunters.reel.land(F32.is_lt(RS.f_hypot(F32.sub(sx, nx), F32.sub(sy, ny)), 0.08), ga.sf(ga.sf(hero, "x", sx), "y", sy), id, home, run, hook)

def hunters.Run.tick_hunters.reel.land(+followed: Bool, +hero: J.Val, +id: U32, +home: Bool, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  match followed:
    case True{}:
      hunters.Run.tick_hunters.reel.hit(home, hero, id, run, hook)
    case False{}:
      (ga.sv(run, "players", ga.put_at(J.get(run, "players"), id, hero)), ga.sv(hook, "caught", J.JNull{}))

# A ward after a hit holds against the hook too: no reel-in lands on a blow
# just taken.
def hunters.Run.tick_hunters.reel.hit(home: Bool, +hero: J.Val, +id: U32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  match home:
    case False{}:
      (ga.sv(run, "players", ga.put_at(J.get(run, "players"), id, hero)), hook)
    case True{}:
      (ga.push(ga.push(ga.sb(ga.sv(run, "players", ga.put_at(J.get(run, "players"), id, hero.Hero.hurt(hero, hunters.HOOK_DAMAGE()))), "room_hurt", True{}),
        "fx", ga.su(J.obj(), "Shake", 6)), "sounds", J.JS{"hero_hurt"}), hook)

# --- flying out ---------------------------------------------------------------

def hunters.Run.tick_hunters.fly(+run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.fly.next(F32.mul(hunters.HOOK_SPEED(), rules.DT()), run, hook)

def hunters.Run.tick_hunters.fly.next(+step: F32, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.fly.tile(
    F32.add(ga.f(hook, "x"), F32.mul(ga.nf(J.get(hook, "dir"), 0), step)),
    F32.add(ga.f(hook, "y"), F32.mul(ga.nf(J.get(hook, "dir"), 1), step)), step, run, hook)

def hunters.Run.tick_hunters.fly.tile(+nx: F32, +ny: F32, +step: F32, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.fly.stop(
    layout.Room.tile(rules.Run.room(run), RS.f_as_i32(F32.div(nx, layout.TILE_UNITS())), RS.f_as_i32(F32.div(ny, layout.TILE_UNITS()))),
    nx, ny, step, run, hook)

def hunters.Run.tick_hunters.fly.stop(+tile: J.Val, +nx: F32, +ny: F32, +step: F32, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.fly.go(
    Bool.or(Bool.or(F32.is_ge(F32.add(ga.f(hook, "out"), step), hunters.HOOK_RANGE()), ga.stone(tile)),
      Bool.and(rules.Run.barred(run), ga.is(tile, "door"))),
    nx, ny, step, run, hook)

def hunters.Run.tick_hunters.fly.go(blocked: Bool, +nx: F32, +ny: F32, +step: F32, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  match blocked:
    case True{}:
      hunters.Run.tick_hunters.fly.catch(run, ga.sb(hook, "back", True{}))
    case False{}:
      hunters.Run.tick_hunters.fly.catch(run, ga.sf(ga.sf(ga.sf(hook, "x", nx), "y", ny), "out", F32.add(ga.f(hook, "out"), step)))

# The first knight it touches, unless rolling clear.
def hunters.Run.tick_hunters.fly.catch(+run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.fly.caught(hunters.Run.tick_hunters.fly.touch(J.int_fields(J.get(run, "players")), ga.f(hook, "x"), ga.f(hook, "y")), run, hook)

def hunters.Run.tick_hunters.fly.touch(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32) -> Maybe<&2, U32>:
  match hs:
    case Nil{}:
      None{}
    case Con{(+k, +h), +t}:
      hunters.Run.tick_hunters.fly.touch.hit(
        Bool.and(Bool.and(peasant.Hero.targetable(h), U32.is_zero(ga.u(h, "dash_ticks"))), Bool.and(U32.is_zero(ga.u(h, "immune")),
          F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)), 1.1))),
        k, t, x, y)

def hunters.Run.tick_hunters.fly.touch.hit(hit: Bool, +k: String, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32) -> Maybe<&2, U32>:
  match hit:
    case True{}:
      Some{Maybe.default(&2, U32, U32.read(k), 0)}
    case False{}:
      hunters.Run.tick_hunters.fly.touch(t, x, y)

def hunters.Run.tick_hunters.fly.caught(m: Maybe<&2, U32>, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  match m:
    case None{}:
      (run, Some{hook})
    case Some{+id}:
      (feats.Run.notice(ga.push(ga.push(run, "cues", J.JS{"hooked"}), "sounds", J.JS{"chest_open"}), "fresh_meat"),
        Some{ga.sb(ga.sv(hook, "caught", J.JU{id}), "back", True{})})

# --- the loop over the hooks --------------------------------------------------

def hunters.Run.tick_hunters.hooks(+run: J.Val, +hs: List<&2, J.Val>, +kept: List<&2, J.Val>) -> J.Val:
  match hs:
    case Nil{}:
      J.set(run, "hooks", J.JA{List.reverse(&2, J.Val, kept)})
    case Con{+h, +t}:
      hunters.Run.tick_hunters.hooks.next(hunters.Run.tick_hunters.one(run, h), t, kept)

def hunters.Run.tick_hunters.hooks.next(rm: J.Val & Maybe<&2, J.Val>, +t: List<&2, J.Val>, +kept: List<&2, J.Val>) -> J.Val:
  (run, m) = rm
  match m:
    case None{}:
      hunters.Run.tick_hunters.hooks(run, t, kept)
    case Some{+h}:
      hunters.Run.tick_hunters.hooks(run, t, Con{h, kept})

# Hooks fly and reel in; webs fade.
def hunters.Run.tick_hunters(+run: J.Val) -> J.Val:
  hunters.Run.tick_hunters.hooks(
    J.set(run, "webs", J.JA{hunters.Run.tick_hunters.webs(J.items(J.get(run, "webs")))}),
    J.items(J.get(run, "hooks")), Nil{})

# Whether a knight standing at `(x, y)` is in a web.
def hunters.Run.webbed(+webs: J.Val, +x: F32, +y: F32) -> Bool:
  hunters.Run.webbed.go(J.items(webs), x, y)

def hunters.Run.webbed.go(+ws: List<&2, J.Val>, +x: F32, +y: F32) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case Con{+w, +t}:
      hunters.Run.webbed.hit(F32.is_lt(RS.f_hypot(F32.sub(ga.f(w, "x"), x), F32.sub(ga.f(w, "y"), y)), hunters.WEB_REACH()), t, x, y)

def hunters.Run.webbed.hit(hit: Bool, +t: List<&2, J.Val>, +x: F32, +y: F32) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      hunters.Run.webbed.go(t, x, y)

# The spiderlings a room holds now, for the brood's cap.
def hunters.Run.brood_full(+run: J.Val) -> Bool:
  U32.is_ge(hunters.Run.brood_full.count(J.items(J.get(run, "enemies")), 0), hunters.BROOD())

def hunters.Run.brood_full.count(+es: List<&2, J.Val>, +n: U32) -> U32:
  match es:
    case Nil{}:
      n
    case Con{+e, +t}:
      hunters.Run.brood_full.count(t, ga.pu(Bool.and(ga.is(J.get(e, "kind"), "spiderling"), U32.is_gt(ga.u(e, "hp"), 0)), (n + 1 : U32), n))
