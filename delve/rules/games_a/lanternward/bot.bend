# crates/delve/src/rules/lanternward/bot.rs — the scripted bot: what a knight
# presses when nobody is at the keys. It reads only the session. The keys are
# an Input J object: `rules.Input.default()` with the fields set; `ult` and
# `interact` are written only when true (serde skips them otherwise), which
# `lanternward.bot.norm` makes so at the end.

# How near a foe must be for the bot to take it on: 150 px.
def lanternward.bot.ENGAGE() -> U32:
  38400

# `sign(v, dead)` as an i8 held in a U32: 1, 0 or -1.
def lanternward.bot.sign(+v: U32, +dead: U32) -> U32:
  Bool.pick(U32, RS.i_gt(v, dead), 1, Bool.pick(U32, RS.i_lt(v, RS.i_neg(dead)), 4294967295, 0))

def lanternward.bot.i_move(+i: J.Val, +mx: U32, +my: U32) -> J.Val:
  ga.si(ga.si(i, "move_x", mx), "move_y", my)

def lanternward.bot.i_aim(+i: J.Val, +ax: U32, +ay: U32) -> J.Val:
  ga.si(ga.si(i, "aim_x", ax), "aim_y", ay)

# Whole-key set: drop `ult` / `interact` when false (serde's skip).
def lanternward.bot.norm(+i: J.Val) -> J.Val:
  ga.sb_skip(ga.sb_skip(i, "interact", ga.b(i, "interact")), "ult", ga.b(i, "ult"))

def lanternward.bot.idle() -> J.Val:
  rules.Input.default()

# The foe nearest the knight among living snuffers: null, or the foe.
def lanternward.bot.nearest_snuffer.go(+fs: List<&2, J.Val>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      lanternward.bot.nearest_snuffer.take(
        Bool.and(lanternward.foes.Foe.alive(f), lanternward.is(f, "kind", "Snuffer")),
        lanternward.world.dist((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)),
        f, t, me, best, bd)

def lanternward.bot.nearest_snuffer.take(+ok: Bool, +d: U32, +f: J.Val, +t: List<&2, J.Val>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  lanternward.bot.nearest_snuffer.go(t, me,
    Bool.pick(J.Val, Bool.and(ok, Bool.or(J.is_null(best), RS.i_lt(d, bd))), f, best),
    Bool.pick(U32, Bool.and(ok, Bool.or(J.is_null(best), RS.i_lt(d, bd))), d, bd))

def lanternward.bot.nearest_snuffer(+s: J.Val, +me: J.Val, +alive_only: Bool) -> J.Val:
  lanternward.bot.nearest_snuffer.go(ga.items(s, "foes"), me, J.JNull{}, 0)

# The living foe of any kind nearest the knight (room 0 and the dark).
def lanternward.bot.nearest_foe.go(+fs: List<&2, J.Val>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      lanternward.bot.nearest_foe.take(
        lanternward.foes.Foe.alive(f),
        lanternward.world.dist((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)),
        f, t, me, best, bd)

def lanternward.bot.nearest_foe.take(+ok: Bool, +d: U32, +f: J.Val, +t: List<&2, J.Val>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  lanternward.bot.nearest_foe.go(t, me,
    Bool.pick(J.Val, Bool.and(ok, Bool.or(J.is_null(best), RS.i_lt(d, bd))), f, best),
    Bool.pick(U32, Bool.and(ok, Bool.or(J.is_null(best), RS.i_lt(d, bd))), d, bd))

# What bot `id` presses this tick.
def lanternward.bot.Session.bot_input(+s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  lanternward.bot.bot_input.me(ga.has_key(J.get(s, "seats"), id), s, id, r)

def lanternward.bot.bot_input.me(has: Bool, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match has:
    case False{}:
      lanternward.bot.idle()
    case True{}:
      lanternward.bot.bot_input.up(ga.at(J.get(s, "seats"), id), s, id, r)

def lanternward.bot.bot_input.up(+me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  lanternward.bot.bot_input.up.go(
    Bool.or(Bool.not(lanternward.kit.Seat.up(me)), Bool.or(lanternward.is(me, "act", "Item"), lanternward.is(me, "act", "Fall"))),
    me, s, id, r)

def lanternward.bot.bot_input.up.go(down: Bool, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match down:
    case True{}:
      lanternward.bot.idle()
    case False{}:
      lanternward.bot.bot_input.done(J.is_null(J.get(s, "done")), me, s, id, r)

def lanternward.bot.bot_input.done(running: Bool, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match running:
    case False{}:
      lanternward.bot.bot_finale(s)
    case True{}:
      lanternward.bot.bot_input.boss(
        Bool.and(
          Bool.not(J.is_null(J.get(s, "boss"))),
          Bool.and(U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0), Bool.not(lanternward.is(J.get(s, "boss"), "st", "Asleep")))),
        me, s, id, r)

def lanternward.bot.bot_input.boss(awake: Bool, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match awake:
    case True{}:
      lanternward.bot.bot_boss(s, id, me, J.get(s, "boss"), r)
    case False{}:
      lanternward.bot.bot_input.foe(lanternward.bot.nearest_snuffer(s, me, True{}), me, s, id, r)

def lanternward.bot.bot_input.foe(+foe: J.Val, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match foe:
    case J.JNull{}:
      lanternward.bot.bot_input.friend(lanternward.bot.first_down(ga.ids(J.get(s, "seats")), J.get(s, "seats"), ga.u(me, "id")), me, s, id, r)
    case _:
      lanternward.bot.bot_input.engage(
        RS.i_lt(lanternward.world.dist((ga.u(foe, "x") - ga.u(me, "x") : U32), (ga.u(foe, "y") - ga.u(me, "y") : U32)), 38400),
        foe, me, s, id, r)

def lanternward.bot.bot_input.engage(near: Bool, +foe: J.Val, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match near:
    case True{}:
      lanternward.bot.bot_fight(s, id, me, foe, r)
    case False{}:
      lanternward.bot.bot_goto(s, me, ga.u(foe, "x"), ga.u(foe, "y"), 30, False{})

# The first other seat on the ground: null, or its seat object.
def lanternward.bot.first_down(+ids: List<&2, U32>, +seats: J.Val, +me_id: U32) -> J.Val:
  match ids:
    case Nil{}:
      J.JNull{}
    case Con{+k, +t}:
      lanternward.bot.first_down.hit(
        Bool.and(U32.is_ne(ga.u(ga.at(seats, k), "id"), me_id), lanternward.is(ga.at(seats, k), "act", "Down")),
        ga.at(seats, k), t, seats, me_id)

def lanternward.bot.first_down.hit(hit: Bool, +o: J.Val, +t: List<&2, U32>, +seats: J.Val, +me_id: U32) -> J.Val:
  match hit:
    case True{}:
      o
    case False{}:
      lanternward.bot.first_down(t, seats, me_id)

# A friend on the ground, and nothing about: lift them.
def lanternward.bot.bot_input.friend(+o: J.Val, +me: J.Val, +s: J.Val, +id: U32, +r: J.Val) -> J.Val:
  match o:
    case J.JNull{}:
      lanternward.bot.bot_objective(s, id, me, r)
    case _:
      lanternward.bot.bot_input.lift(
        lanternward.bot.bot_goto(s, me, ga.u(o, "x"), ga.u(o, "y"), 10, False{}),
        RS.i_lt(lanternward.world.dist((ga.u(o, "x") - ga.u(me, "x") : U32), (ga.u(o, "y") - ga.u(me, "y") : U32)), 4608))

def lanternward.bot.bot_input.lift(+i: J.Val, close: Bool) -> J.Val:
  lanternward.bot.norm(Bool.pick(J.Val, close, ga.sb(i, "interact", True{}), i))

# ---- walking ----

# Tiles a prop blocks (the Rust asks every prop of each tile's middle): the
# five-by-five tiles around each prop, `blocks` asked of each, as a set keyed
# by tile index.
def lanternward.bot.blocked_set.tile(+p: J.Val, +bs: J.Val, +x: U32, +y: U32, +w: U32, +h: U32) -> J.Val:
  Bool.pick(J.Val,
    Bool.and(
      Bool.and(RS.i_ge(x, 0), RS.i_lt(x, w)),
      Bool.and(
        Bool.and(RS.i_ge(y, 0), RS.i_lt(y, h)),
        lanternward.props.Prop.blocks(p, lanternward.world.middle(x), lanternward.world.middle(y)))),
    ga.sb(bs, U32.show((y * w + x : U32)), True{}),
    bs)

def lanternward.bot.blocked_set.around(+p: J.Val, +bs: J.Val, +k: U32, +w: U32, +h: U32) -> J.Val:
  lanternward.bot.blocked_set.around.go(U32.is_ge(k, 25), p, bs, k, w, h)

def lanternward.bot.blocked_set.around.go(done: Bool, +p: J.Val, +bs: J.Val, +k: U32, +w: U32, +h: U32) -> J.Val:
  match done:
    case True{}:
      bs
    case False{}:
      lanternward.bot.blocked_set.around(p,
        lanternward.bot.blocked_set.tile(p, bs,
          (ga.u(p, "tx") + U32.mod(k, 5) - 2 : U32), (ga.u(p, "ty") + U32.div(k, 5) - 2 : U32), w, h),
        (k + 1 : U32), w, h)

def lanternward.bot.blocked_set(+ps: List<&2, J.Val>, +bs: J.Val, +w: U32, +h: U32) -> J.Val:
  match ps:
    case Nil{}:
      bs
    case Con{+p, +t}:
      lanternward.bot.blocked_set(t, lanternward.bot.blocked_set.around(p, bs, 0, w, h), w, h)

# Whether a body can enter tile (x, y): inside the room, the goal itself, or
# open ground no prop stops.
def lanternward.bot.open(+env: J.Val, +x: U32, +y: U32) -> Bool:
  Bool.and(
    Bool.and(RS.i_ge(x, 0), RS.i_lt(x, ga.u(env, "w"))),
    Bool.and(
      Bool.and(RS.i_ge(y, 0), RS.i_lt(y, ga.u(env, "h"))),
      Bool.or(
        Bool.and(U32.is_eq(x, ga.u(env, "tx")), U32.is_eq(y, ga.u(env, "ty"))),
        lanternward.bot.open.ground(env, x, y))))

def lanternward.bot.open.ground(+env: J.Val, +x: U32, +y: U32) -> Bool:
  lanternward.bot.open.floor(lanternward.world.Plan.at(ga.u(env, "room"), x, y), env, x, y)

def lanternward.bot.open.floor(+c: U32, +env: J.Val, +x: U32, +y: U32) -> Bool:
  Bool.and(
    Bool.and(Bool.not(lanternward.world.wall(c)), U32.is_ne(c, 61)),
    Bool.not(J.bool(J.get(ga.v(env, "blocked"), U32.show((y * ga.u(env, "w") + x : U32))))))

# Visit neighbour (nx, ny) of tile `cur`: if unseen and open, remember where
# we came from and queue it.
def lanternward.bot.nb(+st: J.Val, +cur: U32, +nx: U32, +ny: U32, +env: J.Val) -> J.Val:
  lanternward.bot.nb.go(
    Bool.and(
      Bool.and(RS.i_ge(nx, 0), RS.i_lt(nx, ga.u(env, "w"))),
      Bool.and(RS.i_ge(ny, 0), RS.i_lt(ny, ga.u(env, "h")))),
    st, cur, nx, ny, env)

def lanternward.bot.nb.go(inside: Bool, +st: J.Val, +cur: U32, +nx: U32, +ny: U32, +env: J.Val) -> J.Val:
  match inside:
    case False{}:
      st
    case True{}:
      lanternward.bot.nb.seen(
        J.has(ga.v(st, "came"), U32.show((ny * ga.u(env, "w") + nx : U32))), st, cur, nx, ny, env)

def lanternward.bot.nb.seen(seen: Bool, +st: J.Val, +cur: U32, +nx: U32, +ny: U32, +env: J.Val) -> J.Val:
  match seen:
    case True{}:
      st
    case False{}:
      lanternward.bot.nb.push(lanternward.bot.open(env, nx, ny), st, cur, (ny * ga.u(env, "w") + nx : U32))

def lanternward.bot.nb.push(open: Bool, +st: J.Val, +cur: U32, +n: U32) -> J.Val:
  match open:
    case False{}:
      st
    case True{}:
      ga.sv(ga.sv(st, "came", ga.su(ga.v(st, "came"), U32.show(n), cur)), "back", J.JA{Con{J.JU{n}, ga.items(st, "back")}})

# Breadth-first from the queue's front; `came` maps a tile to the one before.
def lanternward.bot.bfs(+k: Nat, +front: List<&2, J.Val>, +st: J.Val, +env: J.Val) -> J.Val:
  match k:
    case 0n:
      J.get(st, "came")
    case 1n+p:
      lanternward.bot.bfs.pop(front, st, env, p)

def lanternward.bot.bfs.pop(+front: List<&2, J.Val>, +st: J.Val, +env: J.Val, +p: Nat) -> J.Val:
  match front:
    case Nil{}:
      lanternward.bot.bfs.refill(List.reverse(&2, J.Val, ga.items(st, "back")), ga.sv(st, "back", J.JA{Nil{}}), env, p)
    case Con{+h, +t}:
      lanternward.bot.bfs.expand(J.u32(h), t, st, env, p)

def lanternward.bot.bfs.refill(+front: List<&2, J.Val>, +st: J.Val, +env: J.Val, +p: Nat) -> J.Val:
  match front:
    case Nil{}:
      J.get(st, "came")
    case Con{+h, +t}:
      lanternward.bot.bfs.expand(J.u32(h), t, st, env, p)

def lanternward.bot.bfs.expand(+cur: U32, +t: List<&2, J.Val>, +st: J.Val, +env: J.Val, +p: Nat) -> J.Val:
  lanternward.bot.bfs.next(U32.is_eq(cur, ga.u(env, "to")), cur, t, st, env, p)

def lanternward.bot.bfs.next(done: Bool, +cur: U32, +t: List<&2, J.Val>, +st: J.Val, +env: J.Val, +p: Nat) -> J.Val:
  match done:
    case True{}:
      J.get(st, "came")
    case False{}:
      lanternward.bot.bfs(p, t,
        lanternward.bot.nb(
          lanternward.bot.nb(
            lanternward.bot.nb(
              lanternward.bot.nb(st, cur, (U32.mod(cur, ga.u(env, "w")) + 1 : U32), U32.div(cur, ga.u(env, "w")), env),
              cur, (U32.mod(cur, ga.u(env, "w")) - 1 : U32), U32.div(cur, ga.u(env, "w")), env),
            cur, U32.mod(cur, ga.u(env, "w")), (U32.div(cur, ga.u(env, "w")) + 1 : U32), env),
          cur, U32.mod(cur, ga.u(env, "w")), (U32.div(cur, ga.u(env, "w")) - 1 : U32), env),
        env)

# Back from the goal to the tile after ours.
def lanternward.bot.back_path(+came: J.Val, +at: U32, +from: U32, +k: Nat) -> U32:
  match k:
    case 0n:
      at
    case 1n+p:
      lanternward.bot.back_path.go(J.has(came, U32.show(at)), came, at, from, p)

def lanternward.bot.back_path.go(has: Bool, +came: J.Val, +at: U32, +from: U32, +p: Nat) -> U32:
  match has:
    case False{}:
      at
    case True{}:
      lanternward.bot.back_path.step(ga.u(came, U32.show(at)), came, at, from, p)

def lanternward.bot.back_path.step(+prev: U32, +came: J.Val, +at: U32, +from: U32, +p: Nat) -> U32:
  lanternward.bot.back_path.cont(U32.is_eq(prev, from), prev, came, at, from, p)

def lanternward.bot.back_path.cont(stop: Bool, +prev: U32, +came: J.Val, +at: U32, +from: U32, +p: Nat) -> U32:
  match stop:
    case True{}:
      at
    case False{}:
      lanternward.bot.back_path(came, prev, from, p)

# The way to walk now to get to tile `to` from where the knight is: [mx, my].
def lanternward.bot.bot_step(+s: J.Val, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.bot.bot_step.go(
    lanternward.world.Plan.w(ga.u(s, "room")), lanternward.world.Plan.h(ga.u(s, "room")),
    lanternward.world.tile_of(ga.u(me, "x")), lanternward.world.tile_of(ga.u(me, "y")), s, me, tx, ty)

def lanternward.bot.bot_step.go(+w: U32, +h: U32, +fx: U32, +fy: U32, +s: J.Val, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.bot.bot_step.inside(
    Bool.and(Bool.and(RS.i_ge(fx, 0), RS.i_ge(fy, 0)), Bool.and(RS.i_lt(fx, w), RS.i_lt(fy, h))),
    w, h, fx, fy, s, me, tx, ty)

def lanternward.bot.bot_step.inside(ok: Bool, +w: U32, +h: U32, +fx: U32, +fy: U32, +s: J.Val, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  match ok:
    case False{}:
      ga.pair_i(0, 0)
    case True{}:
      lanternward.bot.bot_step.same(Bool.and(U32.is_eq(fx, tx), U32.is_eq(fy, ty)), w, h, fx, fy, s, me, tx, ty)

def lanternward.bot.bot_step.same(same: Bool, +w: U32, +h: U32, +fx: U32, +fy: U32, +s: J.Val, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  match same:
    case True{}:
      ga.pair_i(
        lanternward.bot.sign((lanternward.world.middle(tx) - ga.u(me, "x") : U32), 512),
        lanternward.bot.sign((lanternward.world.middle(ty) - ga.u(me, "y") : U32), 512))
    case False{}:
      lanternward.bot.bot_step.search(w, h, fx, fy, me, tx, ty,
        ga.sv(ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(J.obj(), "room", ga.u(s, "room")), "w", w), "h", h), "tx", tx), "ty", ty), "to", (ty * w + tx : U32)), "from", (fy * w + fx : U32)),
          "blocked", lanternward.bot.blocked_set(ga.items(s, "props"), J.obj(), w, h)))

def lanternward.bot.bot_step.search(+w: U32, +h: U32, +fx: U32, +fy: U32, +me: J.Val, +tx: U32, +ty: U32, +env: J.Val) -> J.Val:
  lanternward.bot.bot_step.reached(
    lanternward.bot.bfs(U32.to_nat((w * h * 2 + 2 : U32)), [J.JU{(fy * w + fx : U32)}],
      ga.sv(ga.sv(J.obj(), "came", ga.su(J.obj(), U32.show((fy * w + fx : U32)), (fy * w + fx : U32))), "back", J.JA{Nil{}}), env),
    w, h, fx, fy, me, tx, ty)

def lanternward.bot.bot_step.reached(+came: J.Val, +w: U32, +h: U32, +fx: U32, +fy: U32, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.bot.bot_step.walk(
    Bool.and(
      Bool.and(RS.i_ge(tx, 0), RS.i_ge(ty, 0)),
      Bool.and(Bool.and(RS.i_lt(tx, w), RS.i_lt(ty, h)), J.has(came, U32.show((ty * w + tx : U32))))),
    came, w, fx, fy, me, tx, ty)

def lanternward.bot.bot_step.walk(ok: Bool, +came: J.Val, +w: U32, +fx: U32, +fy: U32, +me: J.Val, +tx: U32, +ty: U32) -> J.Val:
  match ok:
    case False{}:
      ga.pair_i(0, 0)
    case True{}:
      lanternward.bot.bot_step.aim(lanternward.bot.back_path(came, (ty * w + tx : U32), (fy * w + fx : U32), 2048n), w, me)

def lanternward.bot.bot_step.aim(+at: U32, +w: U32, +me: J.Val) -> J.Val:
  ga.pair_i(
    lanternward.bot.sign((lanternward.world.middle(U32.mod(at, w)) - ga.u(me, "x") : U32), 256),
    lanternward.bot.sign((lanternward.world.middle(U32.div(at, w)) - ga.u(me, "y") : U32), 256))

# Walk to a point; press E there when `press`.
def lanternward.bot.bot_goto(+s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach_px: U32, +press: Bool) -> J.Val:
  lanternward.bot.bot_goto.far(
    lanternward.world.dist((px - ga.u(me, "x") : U32), (py - ga.u(me, "y") : U32)),
    s, me, px, py, reach_px, press)

def lanternward.bot.bot_goto.far(+d: U32, +s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach_px: U32, +press: Bool) -> J.Val:
  lanternward.bot.bot_goto.aim(
    d, RS.i_gt(d, (reach_px * 256 : U32)), s, me, px, py, reach_px, press)

def lanternward.bot.bot_goto.aim(+d: U32, far: Bool, +s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach_px: U32, +press: Bool) -> J.Val:
  match far:
    case True{}:
      lanternward.bot.bot_goto.moved(
        lanternward.bot.bot_step(s, me, lanternward.world.tile_of(px), lanternward.world.tile_of(py)),
        d, s, me, px, py, reach_px, press)
    case False{}:
      lanternward.bot.bot_goto.face(rules.Input.default(), d, s, me, px, py, reach_px, press)

def lanternward.bot.bot_goto.moved(+mv: J.Val, +d: U32, +s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach_px: U32, +press: Bool) -> J.Val:
  lanternward.bot.bot_goto.face(
    Bool.pick(J.Val, Bool.and(U32.is_eq(ga.nu(mv, 0), 0), U32.is_eq(ga.nu(mv, 1), 0)),
      lanternward.bot.i_move(rules.Input.default(),
        lanternward.bot.sign((px - ga.u(me, "x") : U32), 256), lanternward.bot.sign((py - ga.u(me, "y") : U32), 256)),
      lanternward.bot.i_move(rules.Input.default(), ga.nu(mv, 0), ga.nu(mv, 1))),
    d, s, me, px, py, reach_px, press)

# Face it with the arrows, and press E every other tick (a press is an edge).
def lanternward.bot.bot_goto.face(+i: J.Val, +d: U32, +s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach_px: U32, +press: Bool) -> J.Val:
  lanternward.bot.norm(
    lanternward.bot.bot_goto.press(
      Bool.and(press, RS.i_le(d, ((reach_px + 6 : U32) * 256 : U32))),
      lanternward.bot.i_aim(i, lanternward.bot.sign((px - ga.u(me, "x") : U32), 768), lanternward.bot.sign((py - ga.u(me, "y") : U32), 768)),
      s))

def lanternward.bot.bot_goto.press(go: Bool, +i: J.Val, +s: J.Val) -> J.Val:
  match go:
    case True{}:
      ga.sb(i, "interact", U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0))
    case False{}:
      i

# ---- fighting a snuffer ----

def lanternward.bot.bot_fight(+s: J.Val, +id: U32, +me: J.Val, +f: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.bot_fight.go(
    lanternward.world.dist((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)),
    lanternward.world.unit((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)),
    s, id, me, f, r)

def lanternward.bot.bot_fight.go(+d: U32, toward: U32 & U32, +s: J.Val, +id: U32, +me: J.Val, +f: J.Val, +r: J.Val) -> J.Val:
  (+tx, +ty) = toward
  lanternward.bot.bot_fight.light(
    d, tx, ty, s, id, me, f, r,
    ga.sb(rules.Input.default(), "ult", RS.i_lt(d, ga.u(r, "lock"))))

# The hood: light first, then the blade.
def lanternward.bot.bot_fight.light(+d: U32, +tx: U32, +ty: U32, +s: J.Val, +id: U32, +me: J.Val, +f: J.Val, +r: J.Val, +i: J.Val) -> J.Val:
  lanternward.bot.bot_fight.line(
    Bool.and(
      Bool.and(Bool.and(ga.b(f, "hooded"), U32.is_lt(ga.u(f, "lit"), 12)), ga.b(ga.v(s, "inv"), "lantern")),
      Bool.and(U32.is_gt(ga.u(me, "flame"), 5), RS.i_lt(d, 19968))),
    d, tx, ty, s, id, me, f, r, i)

def lanternward.bot.bot_fight.line(+fire: Bool, +d: U32, +tx: U32, +ty: U32, +s: J.Val, +id: U32, +me: J.Val, +f: J.Val, +r: J.Val, +i: J.Val) -> J.Val:
  lanternward.bot.bot_fight.state(
    Bool.and(
      Bool.not(lanternward.is(f, "danger", "None")),
      lanternward.foes.Danger.hits(ga.v(f, "danger"), ga.u(me, "x"), ga.u(me, "y"), 1792)),
    d, tx, ty, s, id, me, f, r, Bool.pick(J.Val, fire, ga.sb(i, "fire", True{}), i))

def lanternward.bot.bot_fight.state(+in_line: Bool, +d: U32, +tx: U32, +ty: U32, +s: J.Val, +id: U32, +me: J.Val, +f: J.Val, +r: J.Val, +i: J.Val) -> J.Val:
  lanternward.bot.bot_fight.aim(
    Bool.pick(J.Val, Bool.and(in_line, lanternward.NAME_IN(ga.s(f, "st"), ["Tell", "Strike"])),
      lanternward.bot.bot_fight.dodge(Bool.or(lanternward.is(f, "st", "Strike"), U32.is_le(ga.u(f, "t"), 5)), tx, ty, id, i),
      lanternward.bot.bot_fight.close(d, tx, ty, s, id, f, i)),
    s, me, f)

# In the blow's line: raise the shield late, else step out sideways.
def lanternward.bot.bot_fight.dodge(+shield: Bool, +tx: U32, +ty: U32, +id: U32, +i: J.Val) -> J.Val:
  +sg = Bool.pick(U32, U32.is_eq(U32.mod(id, 2), 0), 1, 4294967295)
  Bool.pick(J.Val, shield, ga.sb(i, "dash", True{}),
    lanternward.bot.i_move(i, lanternward.bot.sign((RS.i_neg(ty) * sg : U32), 64), lanternward.bot.sign((tx * sg : U32), 64)))

def lanternward.bot.bot_fight.close(+d: U32, +tx: U32, +ty: U32, +s: J.Val, +id: U32, +f: J.Val, +i: J.Val) -> J.Val:
  Bool.pick(J.Val, RS.i_gt(d, 6144),
    lanternward.bot.i_move(i, lanternward.bot.sign(tx, 64), lanternward.bot.sign(ty, 64)),
    Bool.pick(J.Val, RS.i_lt(d, 3072),
      lanternward.bot.i_move(i, RS.i_neg(lanternward.bot.sign(tx, 64)), RS.i_neg(lanternward.bot.sign(ty, 64))),
      Bool.pick(J.Val, lanternward.foes.Foe.vulnerable(f),
        lanternward.bot.bot_fight.swing(lanternward.is(f, "st", "Drift"), tx, ty, s, id, ga.sb(i, "swing", U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0))),
        i)))

# A little circling keeps it from lunging into the face.
def lanternward.bot.bot_fight.swing(+drift: Bool, +tx: U32, +ty: U32, +s: J.Val, +id: U32, +i: J.Val) -> J.Val:
  +sg = Bool.pick(U32, U32.is_eq(U32.mod(id, 2), 0), 1, 4294967295)
  Bool.pick(J.Val, drift,
    lanternward.bot.i_move(i, lanternward.bot.sign((RS.i_neg(ty) * sg : U32), 128), lanternward.bot.sign((tx * sg : U32), 128)),
    i)

# Whatever is not walking faces its foe.
def lanternward.bot.bot_fight.aim(+i: J.Val, +s: J.Val, +me: J.Val, +f: J.Val) -> J.Val:
  lanternward.bot.norm(
    lanternward.bot.i_aim(i,
      lanternward.bot.sign((ga.u(f, "x") - ga.u(me, "x") : U32), 1024), lanternward.bot.sign((ga.u(f, "y") - ga.u(me, "y") : U32), 1024)))

# ---- after the Beacon: hold E and leave ----

def lanternward.bot.bot_finale(+s: J.Val) -> J.Val:
  lanternward.bot.norm(
    Bool.pick(J.Val, lanternward.Session.finale(s),
      ga.sb(rules.Input.default(), "interact", U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0)),
      rules.Input.default()))

# ---- the rooms ----

# The nearest of `tiles` (packed x + 65536 y) to the knight: packed, or null.
def lanternward.bot.nearest_tile.go(+ts: List<&2, U32>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  match ts:
    case Nil{}:
      best
    case Con{+t, +rest}:
      lanternward.bot.nearest_tile.take(
        lanternward.world.dist(
          (lanternward.world.middle(lanternward.world.packed_x(t)) - ga.u(me, "x") : U32),
          (lanternward.world.middle(lanternward.world.packed_y(t)) - ga.u(me, "y") : U32)),
        t, rest, me, best, bd)

def lanternward.bot.nearest_tile.take(+d: U32, +t: U32, +rest: List<&2, U32>, +me: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  lanternward.bot.nearest_tile.go(rest, me,
    Bool.pick(J.Val, Bool.or(J.is_null(best), RS.i_lt(d, bd)), J.JU{t}, best),
    Bool.pick(U32, Bool.or(J.is_null(best), RS.i_lt(d, bd)), d, bd))

def lanternward.bot.bot_exit(+s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.bot_exit.go(lanternward.bot.nearest_tile.go(lanternward.world.Plan.find(ga.u(s, "room"), 81), me, J.JNull{}, 0), s, me)

def lanternward.bot.bot_exit.go(+t: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  match t:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.bot_goto(s, me,
        lanternward.world.middle(lanternward.world.packed_x(J.u32(t))), lanternward.world.middle(lanternward.world.packed_y(J.u32(t))), 3, False{})

# ---- the Warden ----

# The seat's place among the seated ids (0 when absent).
def lanternward.bot.rank.go(+ids: List<&2, U32>, +id: U32, +i: U32) -> J.Val:
  match ids:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      Bool.pick(J.Val, U32.is_eq(h, id), J.JU{i}, lanternward.bot.rank.go(t, id, (i + 1 : U32)))

def lanternward.bot.rank(+s: J.Val, +id: U32) -> U32:
  J.u32(lanternward.bot.rank.go(ga.ids(J.get(s, "seats")), id, 0))

# The foe within `reach` of the knight nearest it (living snuffers), or null.
def lanternward.bot.snuffer_within(+s: J.Val, +me: J.Val, +reach: U32) -> J.Val:
  lanternward.bot.snuffer_within.go(lanternward.bot.nearest_snuffer(s, me, True{}), me, reach)

def lanternward.bot.snuffer_within.go(+f: J.Val, +me: J.Val, +reach: U32) -> J.Val:
  match f:
    case J.JNull{}:
      J.JNull{}
    case _:
      Bool.pick(J.Val,
        RS.i_lt(lanternward.world.dist((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)), reach),
        f, J.JNull{})

def lanternward.bot.bot_boss(+s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.bot_boss.fight(
    Bool.not(lanternward.boss.Session.boss_exposed(s)),
    lanternward.bot.snuffer_within(s, me, 11520), s, id, me, w, r)

# A snuffer on top of us comes first, unless the heart is open.
def lanternward.bot.bot_boss.fight(+dark: Bool, +f: J.Val, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.bot_boss.fight.go(Bool.and(dark, Bool.not(J.is_null(f))), f, s, id, me, w, r)

def lanternward.bot.bot_boss.fight.go(go: Bool, +f: J.Val, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.bot.bot_fight(s, id, me, f, r)
    case False{}:
      lanternward.bot.bot_boss.plan(s, id, me, w, r)

def lanternward.bot.bot_boss.plan(+s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  +seats = lanternward.seat_count(s)
  +rank = lanternward.bot.rank(s, id)
  +exposed = lanternward.boss.Session.boss_exposed(s)
  +lighter = U32.is_lt(rank, lanternward.boss.Warden.need(w, seats))
  +open_for = lanternward.boss.Warden.open_for(w, ga.u(s, "elapsed"), seats)
  +strike = Bool.and(Bool.and(exposed, Bool.or(Bool.not(lighter), U32.is_gt(open_for, 30))), Bool.not(lanternward.is(w, "st", "Dying")))
  lanternward.bot.bot_boss.walk(
    strike, lighter, exposed, open_for, rank, s, id, me, w, r,
    lanternward.boss.Warden.heart.x(w), lanternward.boss.Warden.heart.y(w))

def lanternward.bot.bot_boss.walk(+strike: Bool, +lighter: Bool, +exposed: Bool, +open_for: U32, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val, +hx: U32, +hy: U32) -> J.Val:
  lanternward.bot.bot_boss.walk.go(
    lanternward.world.dist((hx - ga.u(me, "x") : U32), (hy - ga.u(me, "y") : U32)),
    lanternward.world.unit((hx - ga.u(me, "x") : U32), (hy - ga.u(me, "y") : U32)),
    strike, lighter, exposed, open_for, rank, s, id, me, w, r, hx, hy)

def lanternward.bot.bot_boss.walk.go(+d: U32, toward: U32 & U32, +strike: Bool, +lighter: Bool, +exposed: Bool, +open_for: U32, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val, +hx: U32, +hy: U32) -> J.Val:
  (+tx, +ty) = toward
  lanternward.bot.bot_boss.moved(
    Bool.pick(U32, strike, 7680, 15872),
    d, tx, ty, strike, lighter, exposed, open_for, rank, s, id, me, w, r, hx, hy)

def lanternward.bot.bot_boss.moved(+stand: U32, +d: U32, +tx: U32, +ty: U32, +strike: Bool, +lighter: Bool, +exposed: Bool, +open_for: U32, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val, +hx: U32, +hy: U32) -> J.Val:
  +sg = Bool.pick(U32, U32.is_eq(U32.mod(id, 2), 0), 1, 4294967295)
  lanternward.bot.bot_boss.keys(
    Bool.pick(J.Val, RS.i_gt(d, (stand + 1536 : U32)),
      lanternward.bot.bot_boss.chase(lanternward.bot.bot_step(s, me, lanternward.world.tile_of(hx), lanternward.world.tile_of(hy)), tx, ty),
      Bool.pick(J.Val, RS.i_lt(d, (stand - 2048 : U32)),
        lanternward.bot.i_move(rules.Input.default(), RS.i_neg(lanternward.bot.sign(tx, 64)), RS.i_neg(lanternward.bot.sign(ty, 64))),
        lanternward.bot.i_move(rules.Input.default(), lanternward.bot.sign((RS.i_neg(ty) * sg : U32), 128), lanternward.bot.sign((tx * sg : U32), 128)))),
    d, strike, lighter, exposed, open_for, rank, s, id, me, w, r, hx, hy)

def lanternward.bot.bot_boss.chase(+mv: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.bot.i_move(rules.Input.default(),
    Bool.pick(U32, U32.is_ne(ga.nu(mv, 0), 0), ga.nu(mv, 0), lanternward.bot.sign(tx, 64)),
    Bool.pick(U32, U32.is_ne(ga.nu(mv, 1), 0), ga.nu(mv, 1), lanternward.bot.sign(ty, 64)))

def lanternward.bot.bot_boss.keys(+i: J.Val, +d: U32, +strike: Bool, +lighter: Bool, +exposed: Bool, +open_for: U32, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val, +hx: U32, +hy: U32) -> J.Val:
  lanternward.bot.bot_boss.light(
    lanternward.bot.i_aim(ga.sb(i, "ult", RS.i_lt(d, ga.u(r, "lock"))),
      lanternward.bot.sign((hx - ga.u(me, "x") : U32), 1024), lanternward.bot.sign((hy - ga.u(me, "y") : U32), 1024)),
    d, strike, lighter, exposed, open_for, rank, s, id, me, w, r, hx, hy)

# Light the heart: a tap of the lantern, every so often.
def lanternward.bot.bot_boss.light(+i: J.Val, +d: U32, +strike: Bool, +lighter: Bool, +exposed: Bool, +open_for: U32, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val, +hx: U32, +hy: U32) -> J.Val:
  lanternward.bot.bot_boss.swing(
    Bool.pick(J.Val,
      Bool.and(
        Bool.or(Bool.not(exposed), Bool.and(lighter, U32.is_lt(open_for, 24))),
        Bool.and(
          Bool.and(lighter, U32.is_ge(ga.u(me, "flame"), U32.div(ga.u(r, "flame"), 5))),
          Bool.and(
            Bool.and(
              U32.is_gt((RS.u_sat_sub(ga.u(s, "elapsed"), J.u32(J.nth(ga.v(w, "lit"), lanternward.kit.hit_slot(id)))) + 28 : U32), 90),
              RS.i_lt(d, 28160)),
            lanternward.props.Session.line_open(s, ga.u(me, "x"), ga.u(me, "y"), hx, hy)))),
      ga.sb(i, "fire", U32.is_lt(U32.mod(ga.u(s, "elapsed"), 6), 2)), i),
    d, strike, rank, s, id, me, w, r)

def lanternward.bot.bot_boss.swing(+i: J.Val, +d: U32, +strike: Bool, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.bot_boss.blows(
    Bool.pick(J.Val, Bool.and(strike, RS.i_le(d, 11264)), ga.sb(i, "swing", U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0)), i),
    rank, s, id, me, w, r)

# Its blows: up and over a ring; out of a beam's wedge; shield for a sweep.
def lanternward.bot.bot_boss.blows(+i: J.Val, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.norm(
    lanternward.bot.bot_boss.blow(
      lanternward.world.dist((ga.u(w, "x") - ga.u(me, "x") : U32), (ga.u(w, "y") - ga.u(me, "y") : U32)),
      i, rank, s, id, me, w, r))

def lanternward.bot.bot_boss.blow(+dm: U32, +i: J.Val, +rank: U32, +s: J.Val, +id: U32, +me: J.Val, +w: J.Val, +r: J.Val) -> J.Val:
  Bool.pick(J.Val, Bool.and(lanternward.is(w, "st", "Strike"), lanternward.is(w, "atk", "Slam")),
    lanternward.bot.bot_boss.slam(dm, i, s, me, w),
    Bool.pick(J.Val, Bool.and(Bool.and(lanternward.is(w, "st", "Tell"), lanternward.is(w, "atk", "Beam")), U32.is_gt(rank, 0)),
      lanternward.bot.bot_boss.wedge(dm, i, me, w),
      Bool.pick(J.Val, Bool.and(lanternward.is(w, "st", "Strike"), lanternward.is(w, "atk", "Beam")),
        lanternward.bot.bot_boss.sweep(dm, i, me, w),
        i)))

def lanternward.bot.bot_boss.slam(+dm: U32, +i: J.Val, +s: J.Val, +me: J.Val, +w: J.Val) -> J.Val:
  +elapsed = (30 - ga.u(w, "t") : U32)
  +ring = ((12 + 4 * elapsed : U32) * 256 : U32)
  Bool.pick(J.Val,
    Bool.and(
      Bool.and(lanternward.in_range((dm - ring : U32), 1536, 5120), U32.is_eq(ga.u(me, "z"), 0)),
      Bool.not(lanternward.NAME_IN(ga.s(me, "act"), ["Hop", "Roll", "Item"]))),
    lanternward.bot.i_move(
      ga.sb(ga.sb(ga.sb(i, "interact", U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0)), "ult", True{}), "swing", False{}), 0, 0),
    i)

def lanternward.bot.bot_boss.wedge(+dm: U32, +i: J.Val, +me: J.Val, +w: J.Val) -> J.Val:
  lanternward.bot.bot_boss.wedge.off(
    lanternward.world.turn(ga.u(w, "aim"), lanternward.world.angle_of((ga.u(me, "x") - ga.u(w, "x") : U32), (ga.u(me, "y") - ga.u(w, "y") : U32))),
    dm, i, me, w)

def lanternward.bot.bot_boss.wedge.off(+off: U32, +dm: U32, +i: J.Val, +me: J.Val, +w: J.Val) -> J.Val:
  Bool.pick(J.Val, Bool.and(RS.i_lt(RS.i_abs(off), 64), RS.i_lt(dm, 38400)),
    lanternward.bot.bot_boss.wedge.out(
      (ga.u(w, "aim") + Bool.pick(U32, RS.i_ge(off, 0), 74, 4294967222) : U32), i, me, w),
    i)

# Out of it by the nearer edge, round the Warden.
def lanternward.bot.bot_boss.wedge.out(+a: U32, +i: J.Val, +me: J.Val, +w: J.Val) -> J.Val:
  ga.sb(ga.sb(
    lanternward.bot.i_move(i,
      lanternward.bot.sign(((ga.u(w, "x") + lanternward.world.cos_q(a) * 70 : U32) - ga.u(me, "x") : U32), 256),
      lanternward.bot.sign(((ga.u(w, "y") + lanternward.world.sin_q(a) * 70 : U32) - ga.u(me, "y") : U32), 256)),
    "swing", False{}), "fire", False{})

def lanternward.bot.bot_boss.coming(+beam: U32, +mine: U32, +b: U32, +beams: U32) -> Bool:
  Bool.pick(Bool, U32.is_ge(b, beams), False{},
    Bool.or(
      lanternward.in_range(lanternward.world.turn((beam + 180 * b : U32), mine), 4294967286, 14),
      lanternward.bot.bot_boss.coming(beam, mine, (b + 1 : U32), beams)))

def lanternward.bot.bot_boss.sweep(+dm: U32, +i: J.Val, +me: J.Val, +w: J.Val) -> J.Val:
  Bool.pick(J.Val,
    Bool.and(
      lanternward.bot.bot_boss.coming(
        (ga.u(w, "aim") - 50 + RS.i_div((100 * (36 - ga.u(w, "t") : U32) : U32), 36) : U32),
        lanternward.world.angle_of((ga.u(me, "x") - ga.u(w, "x") : U32), (ga.u(me, "y") - ga.u(w, "y") : U32)),
        0, Bool.pick(U32, U32.is_ge(ga.u(w, "phase"), 2), 2, 1)),
      RS.i_lt(dm, 38400)),
    lanternward.bot.i_move(ga.sb(ga.sb(ga.sb(i, "dash", True{}), "swing", False{}), "fire", False{}), 0, 0),
    i)

# ---- the rooms' errands ----

# `(a, b)` of a prop's place, or null: prop_pos(kind, id).
def lanternward.bot.prop_pos(+s: J.Val, +kind: String, +id: U32) -> J.Val:
  lanternward.stage.prop_pos.opt(lanternward.stage.Session.prop_of(s, kind, id), s)

def lanternward.stage.prop_pos.opt(+i: J.Val, +s: J.Val) -> J.Val:
  match i:
    case J.JNull{}:
      J.JNull{}
    case _:
      ga.pair_i(lanternward.eu(s, "props", J.u32(i), "x"), lanternward.eu(s, "props", J.u32(i), "y"))

# Walk to a point `(px, py)`, then at the spot face `(ax, ay)` and press E.
def lanternward.bot.at_spot(+s: J.Val, +me: J.Val, +px: U32, +py: U32, +reach: U32, +near: U32, +ax: U32, +ay: U32, +press: Bool) -> J.Val:
  lanternward.bot.at_spot.go(lanternward.bot.bot_goto(s, me, px, py, reach, False{}), RS.i_lt(lanternward.world.dist((px - ga.u(me, "x") : U32), (py - ga.u(me, "y") : U32)), near), s, ax, ay, press)

def lanternward.bot.at_spot.go(+i: J.Val, close: Bool, +s: J.Val, +ax: U32, +ay: U32, +press: Bool) -> J.Val:
  match close:
    case False{}:
      i
    case True{}:
      lanternward.bot.norm(
        ga.sb(lanternward.bot.i_aim(i, ax, ay), "interact", Bool.or(Bool.not(press), U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0))))

def lanternward.bot.bot_objective(+s: J.Val, +id: U32, +me: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.objective.go(
    (ga.u(s, "room") * 10 + ga.u(s, "stage") : U32), s, id, me, r,
    U32.is_eq(lanternward.bot.rank(s, id), 0), lanternward.bot.rank(s, id))

def lanternward.bot.objective.go(+code: U32, +s: J.Val, +id: U32, +me: J.Val, +r: J.Val, +first: Bool, +rank: U32) -> J.Val:
  match code:
    case 0:
      lanternward.bot.objective.foe(lanternward.bot.nearest_foe.go(ga.items(s, "foes"), me, J.JNull{}, 0), 14, s, me)
    case 11:
      lanternward.bot.objective.chest(lanternward.bot.prop_pos(s, "Chest", 0), s, me, first)
    case 10:
      lanternward.bot.objective.braziers(s, id, me, rank)
    case 13:
      lanternward.bot.objective.lantern(s, me, r)
    case 12:
      lanternward.bot.idle()
    case 20:
      lanternward.bot.objective.plates(s, id, me, first, rank)
    case 21:
      lanternward.bot.bot_exit(s, me)
    case 30:
      lanternward.bot.objective.dark(lanternward.bot.nearest_foe.go(ga.items(s, "foes"), me, J.JNull{}, 0), s, me, r)
    case 31:
      lanternward.bot.objective.chest(lanternward.bot.prop_pos(s, "Chest", 1), s, me, first)
    case 32:
      lanternward.bot.objective.lock_door(first, s, me)
    case 33:
      lanternward.bot.objective.key_chest(lanternward.bot.prop_pos(s, "Chest", 0), s, me, first)
    case 34:
      lanternward.bot.objective.boss_door(s, id, me)
    case 42:
      lanternward.bot.objective.container(lanternward.bot.first_drop(ga.items(s, "drops")), s, me)
    case 43:
      lanternward.bot.objective.beacon(lanternward.find_prop(s, "Beacon", 4294967295), s, me)
    case _:
      lanternward.bot.objective.rest(code, s, id, me)

# Stages that just head for the way out, or to the gallery, or nowhere.
def lanternward.bot.objective.rest(+code: U32, +s: J.Val, +id: U32, +me: J.Val) -> J.Val:
  lanternward.bot.objective.rest.go(
    Bool.or(
      Bool.or(Bool.and(U32.is_ge(code, 1), U32.is_le(code, 9)), Bool.and(U32.is_ge(code, 14), U32.is_le(code, 19))),
      Bool.or(U32.is_eq(code, 21), Bool.and(U32.is_ge(code, 35), U32.is_le(code, 39)))),
    Bool.and(U32.is_ge(code, 22), U32.is_le(code, 29)),
    s, id, me)

def lanternward.bot.objective.rest.go(+exit: Bool, +gallery: Bool, +s: J.Val, +id: U32, +me: J.Val) -> J.Val:
  Bool.pick(J.Val, exit, lanternward.bot.bot_exit(s, me),
    Bool.pick(J.Val, gallery, lanternward.bot.bot_gallery(s, id, me), lanternward.bot.idle()))

def lanternward.bot.objective.foe(+f: J.Val, +reach: U32, +s: J.Val, +me: J.Val) -> J.Val:
  match f:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.bot_goto(s, me, ga.u(f, "x"), ga.u(f, "y"), reach, False{})

# The first of the party takes the chest; the rest wait south of it.
def lanternward.bot.objective.chest(+c: J.Val, +s: J.Val, +me: J.Val, +first: Bool) -> J.Val:
  match c:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      Bool.pick(J.Val, first,
        lanternward.bot.bot_goto(s, me, ga.nu(c, 0), ga.nu(c, 1), 14, True{}),
        lanternward.bot.bot_goto(s, me, ga.nu(c, 0), (ga.nu(c, 1) + 12288 : U32), 8, False{}))

# The unlit braziers' indices.
def lanternward.bot.unlit.go(+ps: List<&2, J.Val>, +i: U32) -> List<&2, U32>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      Bool.pick(List<&2, U32>, Bool.and(String.eq(ga.s(p, "kind"), "Brazier"), Bool.not(lanternward.props.Prop.lit(p))),
        Con{i, lanternward.bot.unlit.go(t, (i + 1 : U32))}, lanternward.bot.unlit.go(t, (i + 1 : U32)))

def lanternward.bot.nth_u32(+xs: List<&2, U32>, n: Nat) -> U32:
  match xs n:
    case Nil{} _:
      0
    case Con{+h, +t} 0n:
      h
    case Con{+h, +t} 1n+p:
      lanternward.bot.nth_u32(t, p)

# Three braziers, one ember: each fetches a flame, lights one.
def lanternward.bot.objective.braziers(+s: J.Val, +id: U32, +me: J.Val, +rank: U32) -> J.Val:
  lanternward.bot.objective.braziers.go(lanternward.bot.unlit.go(ga.items(s, "props"), 0), s, me, rank)

def lanternward.bot.objective.braziers.go(+unlit: List<&2, U32>, +s: J.Val, +me: J.Val, +rank: U32) -> J.Val:
  lanternward.bot.objective.braziers.n(U32.from_nat(List.length(&2, U32, unlit)), unlit, s, me, rank)

def lanternward.bot.objective.braziers.n(+n: U32, +unlit: List<&2, U32>, +s: J.Val, +me: J.Val, +rank: U32) -> J.Val:
  Bool.pick(J.Val, U32.is_eq(n, 0), lanternward.bot.idle(),
    Bool.pick(J.Val, U32.is_gt(ga.u(me, "taper"), 20),
      lanternward.bot.objective.braziers.at(lanternward.bot.nth_u32(unlit, U32.to_nat(U32.mod(rank, RS.u_max(n, 1)))), s, me),
      lanternward.bot.objective.ember(lanternward.find_prop(s, "Ember", 4294967295), s, me)))

def lanternward.bot.objective.braziers.at(+j: U32, +s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.bot_goto(s, me, lanternward.eu(s, "props", j, "x"), lanternward.eu(s, "props", j, "y"), 12, True{})

def lanternward.bot.objective.ember(+p: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  match p:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.bot_goto(s, me, ga.u(p, "x"), ga.u(p, "y"), 12, True{})

# South of the cage, the lantern thrown north through the bars.
def lanternward.bot.objective.lantern(+s: J.Val, +me: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.objective.lantern.go(
    lanternward.bot.i_aim(lanternward.bot.bot_goto(s, me, lanternward.world.middle(24), lanternward.world.middle(7), 4, False{}), 0, 4294967295),
    RS.i_lt(lanternward.world.dist((lanternward.world.middle(24) - ga.u(me, "x") : U32), (lanternward.world.middle(7) - ga.u(me, "y") : U32)), 2048),
    s, me, r)

def lanternward.bot.objective.lantern.go(+i: J.Val, +near: Bool, +s: J.Val, +me: J.Val, +r: J.Val) -> J.Val:
  lanternward.bot.norm(
    Bool.pick(J.Val,
      Bool.and(Bool.and(near, U32.is_ge(ga.u(me, "flame"), U32.div(ga.u(r, "flame"), 5))), U32.is_eq(ga.u(me, "lamp_cd"), 0)),
      ga.sb(i, "fire", U32.is_lt(U32.mod(ga.u(s, "elapsed"), 8), 3)), i))

# The Gallery's first zone: the block onto the plate, the second plate held.
def lanternward.bot.first_of(+s: J.Val, +kind: String) -> J.Val:
  lanternward.find_prop(s, kind, 4294967295)

def lanternward.bot.objective.plates(+s: J.Val, +id: U32, +me: J.Val, +first: Bool, +rank: U32) -> J.Val:
  lanternward.bot.objective.plates.gate(lanternward.stage.any_on(ga.items(s, "props"), "Gate", 1), s, id, me, first, rank)

def lanternward.bot.objective.plates.gate(open: Bool, +s: J.Val, +id: U32, +me: J.Val, +first: Bool, +rank: U32) -> J.Val:
  match open:
    case True{}:
      lanternward.bot.bot_gallery(s, id, me)
    case False{}:
      lanternward.bot.objective.plates.second(U32.is_eq(rank, 1), s, me, first)

# The second knight holds the second plate for the rest.
def lanternward.bot.objective.plates.second(second: Bool, +s: J.Val, +me: J.Val, +first: Bool) -> J.Val:
  match second:
    case True{}:
      lanternward.bot.objective.ember.plate(lanternward.find_prop(s, "Plate", 1), s, me)
    case False{}:
      lanternward.bot.objective.plates.push(lanternward.bot.first_of(s, "Block"), lanternward.bot.first_of(s, "Plate"), s, me, first)

def lanternward.bot.objective.ember.plate(+p: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  match p:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.bot_goto(s, me, ga.u(p, "x"), ga.u(p, "y"), 2, False{})

def lanternward.bot.objective.plates.push(+b: J.Val, +p: J.Val, +s: J.Val, +me: J.Val, +first: Bool) -> J.Val:
  match p:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      Bool.pick(J.Val,
        Bool.and(Bool.and(first, Bool.not(J.is_null(b))), U32.is_ne(ga.u(b, "tx"), ga.u(p, "tx"))),
        lanternward.bot.objective.behind(b, s, me),
        lanternward.bot.bot_goto(s, me, (ga.u(p, "x") + 4096 : U32), (ga.u(p, "y") + 8192 : U32), 10, False{}))

# Behind the block, then east into it.
def lanternward.bot.objective.behind(+b: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.norm(
    Bool.pick(J.Val,
      Bool.and(
        RS.i_lt(ga.u(me, "x"), (ga.u(b, "x") - 2560 : U32)),
        RS.i_gt(lanternward.world.dist(((ga.u(b, "x") - 4096 : U32) - ga.u(me, "x") : U32), (ga.u(b, "y") - ga.u(me, "y") : U32)), 1536)),
      lanternward.bot.bot_goto(s, me, (ga.u(b, "x") - 4096 : U32), ga.u(b, "y"), 3, False{}),
      lanternward.bot.i_aim(lanternward.bot.i_move(rules.Input.default(), 1, 0), 1, 0)))

# The dark: head for the nearest snuffer's glow.
def lanternward.bot.objective.dark(+f: J.Val, +s: J.Val, +me: J.Val, +r: J.Val) -> J.Val:
  match f:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.norm(
        ga.sb(lanternward.bot.bot_goto(s, me, ga.u(f, "x"), ga.u(f, "y"), 30, False{}), "ult",
          RS.i_lt(lanternward.world.dist((ga.u(f, "x") - ga.u(me, "x") : U32), (ga.u(f, "y") - ga.u(me, "y") : U32)), ga.u(r, "lock"))))

# The first knight goes to (middle(14), middle(4)) and presses E facing north.
def lanternward.bot.objective.lock_door(+first: Bool, +s: J.Val, +me: J.Val) -> J.Val:
  Bool.pick(J.Val, first,
    lanternward.bot.at_spot(s, me, lanternward.world.middle(14), lanternward.world.middle(4), 3, 1536, 0, 4294967295, True{}),
    lanternward.bot.bot_goto(s, me, lanternward.world.middle(14), lanternward.world.middle(8), 10, False{}))

def lanternward.bot.objective.key_chest(+c: J.Val, +s: J.Val, +me: J.Val, +first: Bool) -> J.Val:
  match c:
    case J.JNull{}:
      lanternward.bot.bot_goto(s, me, lanternward.world.middle(14), lanternward.world.middle(8), 10, False{})
    case _:
      Bool.pick(J.Val, first,
        lanternward.bot.at_spot(s, me, ga.nu(c, 0), (ga.nu(c, 1) + 4096 : U32), 5, 2048, 0, 4294967295, True{}),
        lanternward.bot.bot_goto(s, me, lanternward.world.middle(14), lanternward.world.middle(8), 10, False{}))

# The carrier opens the Boss Key door; the rest wait.
def lanternward.bot.objective.boss_door(+s: J.Val, +id: U32, +me: J.Val) -> J.Val:
  lanternward.bot.objective.boss_door.go(
    lanternward.bot.door_tile(lanternward.world.Plan.find(3, 77)),
    Bool.and(lanternward.tag_is(ga.v(ga.v(s, "inv"), "boss"), "Carried"), U32.is_eq(ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), id)),
    s, me)

def lanternward.bot.door_tile(+ms: List<&2, U32>) -> U32:
  match ms:
    case Nil{}:
      (29 + 65536 * 8 : U32)
    case Con{+h, +t}:
      h

def lanternward.bot.objective.boss_door.go(+door: U32, +carrier: Bool, +s: J.Val, +me: J.Val) -> J.Val:
  Bool.pick(J.Val, carrier,
    lanternward.bot.at_spot(s, me, lanternward.world.middle((lanternward.world.packed_x(door) - 1 : U32)), lanternward.world.middle(lanternward.world.packed_y(door)), 3, 2048, 1, 0, True{}),
    lanternward.bot.objective.boss_door.wait(lanternward.tag_is(ga.v(ga.v(s, "inv"), "boss"), "Lying"), door, s, me))

def lanternward.bot.objective.boss_door.wait(lying: Bool, +door: U32, +s: J.Val, +me: J.Val) -> J.Val:
  Bool.pick(J.Val, lying,
    lanternward.bot.bot_goto(s, me, ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 1), 4, False{}),
    lanternward.bot.bot_goto(s, me, lanternward.world.middle((lanternward.world.packed_x(door) - 4 : U32)), lanternward.world.middle(lanternward.world.packed_y(door)), 10, False{}))

def lanternward.bot.first_drop(+ds: List<&2, J.Val>) -> J.Val:
  match ds:
    case Nil{}:
      J.JNull{}
    case Con{+d, +t}:
      Bool.pick(J.Val, String.eq(ga.s(d, "kind"), "Container"), d, lanternward.bot.first_drop(t))

def lanternward.bot.objective.container(+d: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  match d:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.bot_goto(s, me, ga.u(d, "x"), ga.u(d, "y"), 4, False{})

def lanternward.bot.objective.beacon(+p: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  match p:
    case J.JNull{}:
      lanternward.bot.idle()
    case _:
      lanternward.bot.at_spot(s, me, ga.u(p, "x"), (ga.u(p, "y") + 4096 : U32), 8, 3584, 0, 4294967295, False{})

# The Gallery's zone two: one knight stands where the turret's bolt, met with
# a shield in its first ticks, goes to the crystal; the rest keep back.
def lanternward.bot.bot_gallery(+s: J.Val, +id: U32, +me: J.Val) -> J.Val:
  lanternward.bot.gallery.second(
    Bool.and(U32.is_eq(lanternward.bot.rank(s, id), 1), U32.is_eq(ga.u(s, "stage"), 0)),
    lanternward.find_prop(s, "Plate", 1), s, id, me, lanternward.bot.rank(s, id))

def lanternward.bot.gallery.second(+hold: Bool, +p: J.Val, +s: J.Val, +id: U32, +me: J.Val, +rank: U32) -> J.Val:
  Bool.pick(J.Val, Bool.and(hold, Bool.not(J.is_null(p))),
    lanternward.bot.bot_goto(s, me, ga.u(p, "x"), ga.u(p, "y"), 2, False{}),
    lanternward.bot.gallery.rank(rank, s, id, me))

def lanternward.bot.gallery.rank(+rank: U32, +s: J.Val, +id: U32, +me: J.Val) -> J.Val:
  Bool.pick(J.Val, U32.is_ne(rank, 0),
    lanternward.bot.bot_goto(s, me, lanternward.world.middle(19), lanternward.world.middle((2 + RS.i_rem((2 * rank : U32), 9) : U32)), 10, False{}),
    lanternward.bot.gallery.spot(s, me))

def lanternward.bot.gallery.spot(+s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.gallery.crystal(lanternward.bot.first_of(s, "Crystal"), s, me)

def lanternward.bot.gallery.crystal(+c: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.gallery.aim(
    Bool.pick(U32, J.is_null(c), lanternward.world.middle(35), ga.u(c, "x")),
    Bool.pick(U32, J.is_null(c), lanternward.world.middle(5), ga.u(c, "y")),
    s, me)

def lanternward.bot.gallery.aim(+cx: U32, +cy: U32, +s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.gallery.at(
    lanternward.bot.i_aim(
      lanternward.bot.bot_goto(s, me, lanternward.world.middle(29), lanternward.world.middle(5), 5, False{}),
      lanternward.bot.sign((cx - ga.u(me, "x") : U32), 768), lanternward.bot.sign((cy - ga.u(me, "y") : U32), 768)),
    RS.i_lt(lanternward.world.dist((lanternward.world.middle(29) - ga.u(me, "x") : U32), (lanternward.world.middle(5) - ga.u(me, "y") : U32)), 2304),
    s, me)

# At the spot: stand, lock the crystal (a tap of the arrow moves the lock off
# the turret), and meet the bolt with the shield.
def lanternward.bot.gallery.at(+i: J.Val, close: Bool, +s: J.Val, +me: J.Val) -> J.Val:
  match close:
    case False{}:
      lanternward.bot.norm(i)
    case True{}:
      lanternward.bot.gallery.hold(lanternward.bot.on_crystal(s, me), i, s, me)

def lanternward.bot.on_crystal(+s: J.Val, +me: J.Val) -> Bool:
  Bool.and(
    lanternward.tag_is(ga.v(me, "lock"), "Prop"),
    lanternward.bot.on_crystal.prop(J.u32(lanternward.payload(ga.v(me, "lock"))), s))

def lanternward.bot.on_crystal.prop(+pl: U32, +s: J.Val) -> Bool:
  Bool.and(U32.is_lt(pl, lanternward.len(s, "props")), lanternward.eis(s, "props", pl, "kind", "Crystal"))

def lanternward.bot.gallery.hold(+on: Bool, +i: J.Val, +s: J.Val, +me: J.Val) -> J.Val:
  lanternward.bot.norm(
    ga.sb(
      lanternward.bot.i_aim(
        lanternward.bot.i_move(ga.sb(i, "ult", True{}), 0, 0),
        0, Bool.pick(U32, Bool.and(Bool.not(on), U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0)), 4294967295, 0)),
      "dash",
      Bool.and(on, Bool.or(
        lanternward.bot.gallery.near(ga.items(s, "bolts"), me),
        Bool.and(U32.is_gt(ga.u(me, "shield"), 0),
          Bool.and(Bool.not(lanternward.bot.gallery.clear(ga.items(s, "bolts"), me)), U32.is_lt(ga.u(me, "shield"), 18)))))))

# A hostile bolt closing in on the knight.
def lanternward.bot.gallery.near(+bs: List<&2, J.Val>, +me: J.Val) -> Bool:
  match bs:
    case Nil{}:
      False{}
    case Con{+b, +t}:
      Bool.or(
        Bool.and(
          Bool.and(lanternward.foes.Bolt.hostile(b),
            RS.i_lt(lanternward.world.dist((ga.u(b, "x") - ga.u(me, "x") : U32), (ga.u(b, "y") - ga.u(me, "y") : U32)), 3840)),
          RS.i_gt(lanternward.bot.gallery.closing(b, me), 128)),
        lanternward.bot.gallery.near(t, me))

def lanternward.bot.gallery.closing(+b: J.Val, +me: J.Val) -> U32:
  lanternward.bot.gallery.closing.go(lanternward.world.unit(ga.u(b, "vx"), ga.u(b, "vy")), b, me)

def lanternward.bot.gallery.closing.go(v: U32 & U32, +b: J.Val, +me: J.Val) -> U32:
  (+vx, +vy) = v
  lanternward.world.dot_unit(vx, vy, (ga.u(me, "x") - ga.u(b, "x") : U32), (ga.u(me, "y") - ga.u(b, "y") : U32))

# No hostile bolt within 40 px.
def lanternward.bot.gallery.clear(+bs: List<&2, J.Val>, +me: J.Val) -> Bool:
  match bs:
    case Nil{}:
      True{}
    case Con{+b, +t}:
      Bool.and(
        Bool.or(Bool.not(lanternward.foes.Bolt.hostile(b)),
          RS.i_gt(lanternward.world.dist((ga.u(b, "x") - ga.u(me, "x") : U32), (ga.u(b, "y") - ga.u(me, "y") : U32)), 10240)),
        lanternward.bot.gallery.clear(t, me))
