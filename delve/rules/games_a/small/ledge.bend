# crates/delve/src/rules/ledge.rs — side-on rooms. A body's position is its
# middle; its feet are FOOT below. A `Grid` is the J object {room, barred}
# (the core group's `rules.Grid`); a tile is its serde name as a J string.

def ledge.FOOT() -> F32:
  1.0

def ledge.HALF_W() -> F32:
  0.45

def ledge.GRAVITY() -> F32:
  95.0

def ledge.JUMP() -> F32:
  31.0

def ledge.AIR_JUMP() -> F32:
  28.0

def ledge.FALL_MAX() -> F32:
  40.0

def ledge.AIR_JUMPS() -> U32:
  1

def ledge.DROP() -> U32:
  8

def ledge.PIT_HURT() -> U32:
  20

def ledge.WALKER_FALL() -> F32:
  0.8

def ledge.Grid.tile_side(+g: J.Val, +x: F32, +y: F32) -> J.Val:
  J.JS{layout.tile_name(rules.Grid.tile_at(J.get(g, "room"), x, y))}

# Stone to a body moving sideways or rising.
def ledge.Grid.solid.tile(+t: J.Val, +barred: Bool) -> Bool:
  Bool.or(ga.stone(t), Bool.and(ga.is(t, "door"), barred))

def ledge.Grid.solid(+g: J.Val, +x: F32, +y: F32) -> Bool:
  ledge.Grid.solid.tile(ledge.Grid.tile_side(g, x, y), ga.b(g, "barred"))

# One column of a body: the three heights `-FOOT + 0.05`, 0, `FOOT - 0.05`.
def ledge.Grid.body_col(+g: J.Val, +x: F32, +y: F32) -> Bool:
  Bool.or(
    ledge.Grid.solid(g, x, F32.add(y, F32.add(F32.neg(ledge.FOOT()), 0.05))),
    Bool.or(ledge.Grid.solid(g, x, F32.add(y, 0.0)), ledge.Grid.solid(g, x, F32.add(y, F32.sub(ledge.FOOT(), 0.05)))))

# A body centred at `(x, y)` overlaps stone.
def ledge.Grid.body_blocked(+g: J.Val, +x: F32, +y: F32) -> Bool:
  Bool.or(
    ledge.Grid.body_col(g, F32.add(x, F32.neg(ledge.HALF_W())), y),
    ledge.Grid.body_col(g, F32.add(x, ledge.HALF_W()), y))

# One foot of `landing`: the top it lands on.
def ledge.Grid.landing.foot(+g: J.Val, +xx: F32, +top: F32, +thru: Bool) -> Maybe<&2, F32>:
  ledge.Grid.landing.tile(ledge.Grid.tile_side(g, xx, F32.add(top, 0.01)), top, thru)

def ledge.Grid.landing.tile(+t: J.Val, +top: F32, +thru: Bool) -> Maybe<&2, F32>:
  ledge.Grid.landing.pick(Bool.or(ga.stone(t), Bool.and(ga.is(t, "ledge"), Bool.not(thru))), top)

def ledge.Grid.landing.pick(hit: Bool, +top: F32) -> Maybe<&2, F32>:
  match hit:
    case True{}:
      Some{top}
    case False{}:
      None{}

def ledge.first(m: Maybe<&2, F32>, n: Maybe<&2, F32>) -> Maybe<&2, F32>:
  match m:
    case Some{+v}:
      Some{v}
    case None{}:
      n

# Feet moving down from `from` to `to`: the top they land on, if one lies
# between. Planks hold only feet that come from above them.
def ledge.Grid.landing(+g: J.Val, +x: F32, +from: F32, +to: F32, +thru: Bool) -> Maybe<&2, F32>:
  +top = F32.mul(F32.floor(F32.div(to, layout.TILE_UNITS())), layout.TILE_UNITS())
  ledge.Grid.landing.in(Bool.or(F32.is_gt(from, F32.add(top, 0.001)), F32.is_lt(to, top)), g, x, top, thru)

def ledge.Grid.landing.in(none: Bool, +g: J.Val, +x: F32, +top: F32, +thru: Bool) -> Maybe<&2, F32>:
  match none:
    case True{}:
      None{}
    case False{}:
      ledge.first(
        ledge.Grid.landing.foot(g, F32.add(x, F32.neg(ledge.HALF_W())), top, thru),
        ledge.Grid.landing.foot(g, F32.add(x, ledge.HALF_W()), top, thru))

def ledge.is_some(m: Maybe<&2, F32>) -> Bool:
  match m:
    case Some{v}:
      True{}
    case None{}:
      False{}

# Something to stand on right under feet at `feet`.
def ledge.Grid.underfoot(+g: J.Val, +x: F32, +feet: F32, +thru: Bool) -> Bool:
  ledge.is_some(ledge.Grid.landing(g, x, feet, F32.add(feet, 0.02), thru))

def ledge.Grid.pit(+g: J.Val, +x: F32, +feet: F32) -> Bool:
  ga.is(ledge.Grid.tile_side(g, x, F32.sub(feet, 0.01)), "hazard")

# --- one knight's step side-on --------------------------------------------

# The jump key as last seen, and a jump press: the hero with `vy`, `grounded`
# and `air_jumps` as the press leaves them.
def ledge.walk.jump(+hero: J.Val, +jump: Bool, +rolling: Bool) -> J.Val:
  ledge.walk.jump.in(Bool.and(jump, Bool.and(Bool.not(ga.b(hero, "jump_held")), Bool.not(rolling))), hero)

def ledge.walk.jump.in(press: Bool, +hero: J.Val) -> J.Val:
  match press:
    case False{}:
      hero
    case True{}:
      ledge.walk.jump.air(ga.b(hero, "grounded"), hero)

def ledge.walk.jump.air(grounded: Bool, +hero: J.Val) -> J.Val:
  match grounded:
    case True{}:
      ga.sb(ga.sf(hero, "vy", F32.neg(ledge.JUMP())), "grounded", False{})
    case False{}:
      ledge.walk.jump.double(U32.is_gt(ga.u(hero, "air_jumps"), 0), hero)

def ledge.walk.jump.double(has: Bool, +hero: J.Val) -> J.Val:
  match has:
    case False{}:
      hero
    case True{}:
      ga.sf(ga.su(hero, "air_jumps", (ga.u(hero, "air_jumps") - 1 : U32)), "vy", F32.neg(ledge.AIR_JUMP()))

# Down on the move axis while standing on a plank (not stone) drops through it.
def ledge.walk.drop(drop: Bool, +hero: J.Val) -> J.Val:
  match drop:
    case False{}:
      hero
    case True{}:
      ga.sb(ga.su(hero, "dropping", ledge.DROP()), "grounded", False{})

# `self.vy`: the roll runs flat; otherwise gravity, capped.
def ledge.walk.gravity(+hero: J.Val, +rolling: Bool) -> J.Val:
  match rolling:
    case True{}:
      ga.sf(hero, "vy", 0.0)
    case False{}:
      ga.sf(hero, "vy", RS.f_min(F32.add(ga.f(hero, "vy"), F32.mul(ledge.GRAVITY(), rules.DT())), ledge.FALL_MAX()))

# Falling: a landing sets the knight on the top, none lets them drop.
def ledge.walk.down(m: Maybe<&2, F32>, +hero: J.Val, +dy: F32) -> J.Val:
  match m:
    case Some{+top}:
      ga.su(ga.sb(ga.sf(ga.sf(hero, "y", F32.sub(top, ledge.FOOT())), "vy", 0.0), "grounded", True{}), "air_jumps", ledge.AIR_JUMPS())
    case None{}:
      ga.sb(ga.sf(hero, "y", F32.add(ga.f(hero, "y"), dy)), "grounded", False{})

# Rising: a bonk when the head meets stone, otherwise the rise.
def ledge.walk.up(+g: J.Val, +hero: J.Val, +dy: F32) -> J.Val:
  ledge.walk.up.in(
    Bool.or(
      ledge.Grid.solid(g, F32.add(ga.f(hero, "x"), F32.neg(ledge.HALF_W())), F32.add(F32.sub(ga.f(hero, "y"), ledge.FOOT()), dy)),
      ledge.Grid.solid(g, F32.add(ga.f(hero, "x"), ledge.HALF_W()), F32.add(F32.sub(ga.f(hero, "y"), ledge.FOOT()), dy))),
    hero, dy)

def ledge.walk.up.in(bonk: Bool, +hero: J.Val, +dy: F32) -> J.Val:
  match bonk:
    case True{}:
      ga.sb(ga.sf(ga.sf(hero, "y",
        F32.add(F32.mul(F32.add(F32.floor(F32.div(F32.add(F32.sub(ga.f(hero, "y"), ledge.FOOT()), dy), layout.TILE_UNITS())), 1.0), layout.TILE_UNITS()), ledge.FOOT())),
        "vy", 0.0), "grounded", False{})
    case False{}:
      ga.sb(ga.sf(hero, "y", F32.add(ga.f(hero, "y"), dy)), "grounded", False{})

# Not moving vertically: grounded is whether something is underfoot.
def ledge.walk.rest(+g: J.Val, +hero: J.Val, +feet: F32) -> J.Val:
  ledge.walk.rest.in(ledge.Grid.underfoot(g, ga.f(hero, "x"), feet, U32.is_gt(ga.u(hero, "dropping"), 0)), hero)

def ledge.walk.rest.in(g: Bool, +hero: J.Val) -> J.Val:
  match g:
    case True{}:
      ga.su(ga.sb(hero, "grounded", True{}), "air_jumps", ledge.AIR_JUMPS())
    case False{}:
      ga.sb(hero, "grounded", False{})

# The vertical move by the sign of `dy`.
def ledge.walk.vertical(+g: J.Val, +hero: J.Val) -> J.Val:
  ledge.walk.vertical.in(g, hero, F32.mul(ga.f(hero, "vy"), rules.DT()), F32.add(ga.f(hero, "y"), ledge.FOOT()))

def ledge.walk.vertical.in(+g: J.Val, +hero: J.Val, +dy: F32, +feet: F32) -> J.Val:
  ledge.walk.vertical.pick(F32.is_gt(dy, 0.0), F32.is_lt(dy, 0.0), g, hero, dy, feet)

def ledge.walk.vertical.pick(pos: Bool, neg: Bool, +g: J.Val, +hero: J.Val, +dy: F32, +feet: F32) -> J.Val:
  match pos:
    case True{}:
      ledge.walk.down(ledge.Grid.landing(g, ga.f(hero, "x"), feet, F32.add(feet, dy), U32.is_gt(ga.u(hero, "dropping"), 0)), hero, dy)
    case False{}:
      ledge.walk.vertical.neg(neg, g, hero, dy, feet)

def ledge.walk.vertical.neg(neg: Bool, +g: J.Val, +hero: J.Val, +dy: F32, +feet: F32) -> J.Val:
  match neg:
    case True{}:
      ledge.walk.up(g, hero, dy)
    case False{}:
      ledge.walk.rest(g, hero, feet)

# Fell in a pit: the bite (unless graced), back on the last firm footing.
def ledge.walk.pit(+hero: J.Val) -> J.Val:
  +h1 = ga.su(hero, "hp", ga.pu(U32.is_zero(ga.u(hero, "invulnerable")), ga.ssub(ga.u(hero, "hp"), ledge.PIT_HURT()), ga.u(hero, "hp")))
  +h2 = ledge.walk.pit.back(J.get(h1, "footing"), h1)
  ga.su(ga.su(ga.sf(h2, "vy", 0.0), "dash_ticks", 0), "invulnerable", U32.max(ga.u(h2, "invulnerable"), rules.HZ()))

def ledge.walk.pit.back(+footing: J.Val, +hero: J.Val) -> J.Val:
  ledge.walk.pit.back.in(J.is_null(footing), footing, hero)

def ledge.walk.pit.back.in(none: Bool, +footing: J.Val, +hero: J.Val) -> J.Val:
  match none:
    case True{}:
      hero
    case False{}:
      ga.sf(ga.sf(hero, "x", ga.nf(footing, 0)), "y", ga.nf(footing, 1))

# Firm footing: both feet on something, for a pit to put you back on.
def ledge.walk.footing(+g: J.Val, +hero: J.Val, +feet: F32) -> J.Val:
  ledge.walk.footing.in(
    Bool.and(ga.b(hero, "grounded"),
      Bool.and(
        ledge.Grid.underfoot(g, F32.add(ga.f(hero, "x"), F32.mul(F32.neg(ledge.HALF_W()), 2.0)), feet, False{}),
        ledge.Grid.underfoot(g, F32.add(ga.f(hero, "x"), F32.mul(ledge.HALF_W(), 2.0)), feet, False{}))),
    hero)

def ledge.walk.footing.in(firm: Bool, +hero: J.Val) -> J.Val:
  match firm:
    case True{}:
      J.set(hero, "footing", ga.pair_f(ga.f(hero, "x"), ga.f(hero, "y")))
    case False{}:
      hero

# One knight's step side-on: `dx` across (running or rolling), then the jump,
# the fall and the landing. The knight, and whether they fell in a pit.
def ledge.walk(+hero: J.Val, +input: J.Val, +g: J.Val, +dx: F32, +rolling: Bool) -> J.Val & Bool:
  +jump = RS.i_lt(ga.u(input, "move_y"), 0)
  +h1 = ga.sb(ledge.walk.jump(hero, jump, rolling), "jump_held", jump)
  +h2 = ga.su(h1, "dropping", ga.ssub(ga.u(h1, "dropping"), 1))
  +h3 = ledge.walk.drop(
    Bool.and(RS.i_gt(ga.u(input, "move_y"), 0),
      Bool.and(ga.b(h2, "grounded"), Bool.not(ledge.Grid.underfoot(g, ga.f(h2, "x"), F32.add(ga.f(h2, "y"), ledge.FOOT()), True{})))),
    h2)
  +h4 = ledge.walk.sideways(ledge.Grid.body_blocked(g, F32.add(ga.f(h3, "x"), dx), ga.f(h3, "y")), h3, dx)
  +h5 = ledge.walk.vertical(g, ledge.walk.gravity(h4, rolling))
  +feet = F32.add(ga.f(h5, "y"), ledge.FOOT())
  ledge.walk.end(ledge.Grid.pit(g, ga.f(h5, "x"), feet), g, h5, feet)

def ledge.walk.sideways(blocked: Bool, +hero: J.Val, +dx: F32) -> J.Val:
  match blocked:
    case True{}:
      hero
    case False{}:
      ga.sf(hero, "x", F32.add(ga.f(hero, "x"), dx))

def ledge.walk.end(pit: Bool, +g: J.Val, +hero: J.Val, +feet: F32) -> J.Val & Bool:
  match pit:
    case True{}:
      (ledge.walk.pit(hero), True{})
    case False{}:
      (ledge.walk.footing(g, hero, feet), False{})

# A walker side-on: it keeps the ground it walks on, falls off what it walks
# past, and a pit is the end of it.
def ledge.fall(+enemy: J.Val, +g: J.Val) -> J.Val:
  +feet = F32.add(ga.f(enemy, "y"), ledge.FOOT())
  +e1 = ledge.fall.land(ledge.Grid.landing(g, ga.f(enemy, "x"), feet, F32.add(feet, ledge.WALKER_FALL()), False{}), enemy)
  ledge.fall.pit(ledge.Grid.pit(g, ga.f(e1, "x"), F32.add(ga.f(e1, "y"), ledge.FOOT())), e1)

def ledge.fall.land(m: Maybe<&2, F32>, +enemy: J.Val) -> J.Val:
  match m:
    case Some{+top}:
      ga.sf(enemy, "y", F32.sub(top, ledge.FOOT()))
    case None{}:
      ga.sf(enemy, "y", F32.add(ga.f(enemy, "y"), ledge.WALKER_FALL()))

def ledge.fall.pit(pit: Bool, +enemy: J.Val) -> J.Val:
  match pit:
    case True{}:
      ga.su(enemy, "hp", 0)
    case False{}:
      enemy

# The ground under column `x`: the top of the highest stone or plank below
# `from`, as a body's middle standing on it.
def ledge.stand_on(+room: J.Val, +x: F32, +from: F32) -> F32:
  ledge.stand_on.go(room, RS.f_as_i32(F32.floor(F32.div(x, layout.TILE_UNITS()))), RS.f_as_i32(F32.floor(F32.div(from, layout.TILE_UNITS()))), from, ga.u(room, "rows"))

def ledge.stand_on.go(+room: J.Val, +col: U32, +row: U32, +from: F32, +rows: U32) -> F32:
  ledge.stand_on.row(RS.i_lt(row, rows), room, col, row, from, rows)

def ledge.stand_on.row(inside: Bool, +room: J.Val, +col: U32, +row: U32, +from: F32, +rows: U32) -> F32:
  match inside:
    case False{}:
      from
    case True{}:
      ledge.stand_on.tile(
        Bool.and(Bool.or(ga.stone(layout.Room.tile(room, col, row)), ga.is(layout.Room.tile(room, col, row), "ledge")), RS.i_gt(row, 0)),
        room, col, row, from, rows)

def ledge.stand_on.tile(hit: Bool, +room: J.Val, +col: U32, +row: U32, +from: F32, +rows: U32) -> F32:
  match hit:
    case True{}:
      F32.sub(F32.mul(RS.i_as_f32(row), layout.TILE_UNITS()), ledge.FOOT())
    case False{}:
      ledge.stand_on.go(room, col, RS.i_add(row, 1), from, rows)

# --- the Run's side-on hooks ------------------------------------------------

# Whether the party stands in a side-on room.
def ledge.Run.side_on(+run: J.Val) -> Bool:
  Bool.or(ga.is(J.get(rules.Run.room(run), "kind"), "ledge"), ga.is(J.get(rules.Run.room(run), "kind"), "wanted"))

# The low west doorway's row: the first row whose tile in column 0 is a door,
# else the middle.
def ledge.Run.arrive_side_on.door(+room: J.Val, +r: U32, +rows: U32) -> U32:
  ledge.Run.arrive_side_on.door.in(RS.i_lt(r, rows), room, r, rows)

def ledge.Run.arrive_side_on.door.in(inside: Bool, +room: J.Val, +r: U32, +rows: U32) -> U32:
  match inside:
    case False{}:
      RS.i_div(rows, 2)
    case True{}:
      ledge.Run.arrive_side_on.door.hit(ga.is(layout.Room.tile(room, 0, r), "door"), room, r, rows)

def ledge.Run.arrive_side_on.door.hit(hit: Bool, +room: J.Val, +r: U32, +rows: U32) -> U32:
  match hit:
    case True{}:
      r
    case False{}:
      ledge.Run.arrive_side_on.door(room, RS.i_add(r, 1), rows)

# Each knight, in seat order, at x = 3.3 + slot * 1.6 standing on the ground.
def ledge.Run.arrive_side_on.seat(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +room: J.Val, +from: F32, +slot: U32) -> J.Val:
  match hs:
    case Nil{}:
      players
    case Con{(+k, +hero), +t}:
      ledge.Run.arrive_side_on.seat(t, J.set(players, k, ledge.Run.arrive_side_on.hero(hero, room, from, slot)), room, from, (slot + 1 : U32))

def ledge.Run.arrive_side_on.hero(+hero: J.Val, +room: J.Val, +from: F32, +slot: U32) -> J.Val:
  ledge.Run.arrive_side_on.hero.at(hero, F32.add(3.3, F32.mul(RS.u_as_f32(slot), 1.6)), room, from)

def ledge.Run.arrive_side_on.hero.at(+hero: J.Val, +x: F32, +room: J.Val, +from: F32) -> J.Val:
  ledge.Run.arrive_side_on.hero.put(hero, x, ledge.stand_on(room, x, from))

def ledge.Run.arrive_side_on.hero.put(+hero: J.Val, +x: F32, +y: F32) -> J.Val:
  +h1 = ga.sf(ga.sf(hero, "x", x), "y", y)
  +h2 = ga.su(ga.sb(ga.sf(h1, "vy", 0.0), "grounded", True{}), "air_jumps", ledge.AIR_JUMPS())
  ga.sf(ga.sf(J.set(h2, "footing", ga.pair_f(x, y)), "aim_x", 1.0), "aim_y", 0.0)

# Into a side-on hall: the party comes in at its low west doorway, on the
# ground, falling still.
def ledge.Run.arrive_side_on(+run: J.Val) -> J.Val:
  ledge.Run.arrive_side_on.in(run, rules.Run.room(run))

def ledge.Run.arrive_side_on.in(+run: J.Val, +room: J.Val) -> J.Val:
  ledge.Run.arrive_side_on.put(run, room, F32.mul(RS.i_as_f32(ledge.Run.arrive_side_on.door(room, 0, ga.u(room, "rows"))), layout.TILE_UNITS()))

def ledge.Run.arrive_side_on.put(+run: J.Val, +room: J.Val, +from: F32) -> J.Val:
  J.set(run, "players", ledge.Run.arrive_side_on.seat(J.int_fields(J.get(run, "players")), J.get(run, "players"), room, from, 0))

# `self.rng.below(n)`: the run with the generator moved on, and the draw.
def ledge.Run.below(+run: J.Val, +n: U32) -> J.Val & U32:
  ledge.Run.below.in(run, loot.Rng.below(W.of_j(J.get(run, "rng")), n))

def ledge.Run.below.in(+run: J.Val, rv: W.W & U32) -> J.Val & U32:
  (r, v) = rv
  (J.set(run, "rng", W.to_j(r)), v)

# A monster's place side-on: walkers on a ledge or the ground well away from
# the door, fliers in the open air above.
def ledge.Run.perch(+run: J.Val, +flies: Bool) -> J.Val & (F32 & F32):
  ledge.Run.perch.go(run, flies, rules.Run.room(run), 64n)

def ledge.Run.perch.go(+run: J.Val, +flies: Bool, +room: J.Val, +n: Nat) -> J.Val & (F32 & F32):
  match n:
    case 0n:
      (run, (F32.div(F32.mul(RS.u_as_f32(ga.u(room, "cols")), layout.TILE_UNITS()), 2.0), F32.mul(layout.TILE_UNITS(), 3.0)))
    case 1n+p:
      ledge.Run.perch.col(ledge.Run.below(run, (ga.u(room, "cols") - 14 : U32)), flies, room, p)

def ledge.Run.perch.col(rv: J.Val & U32, +flies: Bool, +room: J.Val, +p: Nat) -> J.Val & (F32 & F32):
  (run, v) = rv
  ledge.Run.perch.flier(flies, run, room, RS.i_add(12, v), p)

def ledge.Run.perch.flier(+flies: Bool, +run: J.Val, +room: J.Val, +col: U32, +p: Nat) -> J.Val & (F32 & F32):
  match flies:
    case True{}:
      ledge.Run.perch.air(ledge.Run.below(run, (U32.div(ga.u(room, "rows"), 2) - 1 : U32)), room, col, p)
    case False{}:
      ledge.Run.perch.stand(ledge.Run.below(run, (ga.u(room, "rows") - 3 : U32)), room, col, p)

def ledge.Run.perch.x(+col: U32) -> F32:
  F32.mul(F32.add(RS.i_as_f32(col), 0.5), layout.TILE_UNITS())

def ledge.Run.perch.air(rv: J.Val & U32, +room: J.Val, +col: U32, +p: Nat) -> J.Val & (F32 & F32):
  (run, +v) = rv
  ledge.Run.perch.air.row(ga.is(layout.Room.tile(room, col, RS.i_add(2, v)), "floor"), run, room, col, RS.i_add(2, v), p)

def ledge.Run.perch.air.row(open: Bool, +run: J.Val, +room: J.Val, +col: U32, +row: U32, +p: Nat) -> J.Val & (F32 & F32):
  match open:
    case True{}:
      (run, (ledge.Run.perch.x(col), F32.mul(F32.add(RS.i_as_f32(row), 0.5), layout.TILE_UNITS())))
    case False{}:
      ledge.Run.perch.go(run, True{}, room, p)

def ledge.Run.perch.stand(rv: J.Val & U32, +room: J.Val, +col: U32, +p: Nat) -> J.Val & (F32 & F32):
  (run, v) = rv
  ledge.Run.perch.stand.at(run, room, col, p, ledge.Run.perch.x(col), F32.mul(RS.u_as_f32((1 + v : U32)), layout.TILE_UNITS()))

def ledge.Run.perch.stand.at(+run: J.Val, +room: J.Val, +col: U32, +p: Nat, +x: F32, +start: F32) -> J.Val & (F32 & F32):
  ledge.Run.perch.stand.ground(run, room, col, p, x, ledge.stand_on(room, x, start))

def ledge.Run.perch.stand.ground(+run: J.Val, +room: J.Val, +col: U32, +p: Nat, +x: F32, +y: F32) -> J.Val & (F32 & F32):
  ledge.Run.perch.stand.ok(
    Bool.and(
      ga.is(layout.Room.tile(room, col, RS.f_as_i32(F32.div(y, layout.TILE_UNITS()))), "floor"),
      Bool.or(
        ga.is(layout.Room.tile(room, col, RS.f_as_i32(F32.div(F32.add(y, ledge.FOOT()), layout.TILE_UNITS()))), "block"),
        ga.is(layout.Room.tile(room, col, RS.f_as_i32(F32.div(F32.add(y, ledge.FOOT()), layout.TILE_UNITS()))), "ledge"))),
    run, room, p, x, y)

def ledge.Run.perch.stand.ok(ok: Bool, +run: J.Val, +room: J.Val, +p: Nat, +x: F32, +y: F32) -> J.Val & (F32 & F32):
  match ok:
    case True{}:
      (run, (x, y))
    case False{}:
      ledge.Run.perch.go(run, False{}, room, p)
