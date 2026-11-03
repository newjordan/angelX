# crates/delve/src/rules/lanternward/props.rs — what stands in a room, what
# lies on the floor, the puffs and sparks. (The render-only `lights`,
# `light_at`, `light_in` and `Item::name/how` are not ported.)

def lanternward.NAME_IN(+k: String, +names: List<&2, String>) -> Bool:
  match names:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(String.eq(k, h), lanternward.NAME_IN(k, t))

# `Pk::of`: the kind a plan letter makes, or null.
def lanternward.props.Pk.of(+c: U32) -> J.Val:
  match c:
    case 111:
      J.JS{"Pot"}
    case 120:
      J.JS{"Pot"}
    case 34:
      J.JS{"Grass"}
    case 115:
      J.JS{"Sign"}
    case 66:
      J.JS{"Brazier"}
    case 67:
      J.JS{"Rest"}
    case 69:
      J.JS{"Ember"}
    case 98:
      J.JS{"Great"}
    case 99:
      J.JS{"Chest"}
    case 71:
      J.JS{"Gate"}
    case 80:
      J.JS{"Plate"}
    case 89:
      J.JS{"Plate"}
    case 88:
      J.JS{"Block"}
    case 75:
      J.JS{"Crystal"}
    case 126:
      J.JS{"Cracked"}
    case 78:
      J.JS{"Sigil"}
    case 76:
      J.JS{"Lock"}
    case 77:
      J.JS{"Boss"}
    case 42:
      J.JS{"Beacon"}
    case 102:
      J.JS{"Fissure"}
    case 70:
      J.JS{"Folk"}
    case 47:
      J.JS{"Lever"}
    case _:
      lanternward.props.Pk.of.digit(Bool.and(U32.is_ge(c, 49), U32.is_le(c, 57)))

def lanternward.props.Pk.of.digit(d: Bool) -> J.Val:
  match d:
    case True{}:
      J.JS{"Gate"}
    case False{}:
      J.JNull{}

def lanternward.props.Prop.new(+kind: String, +tx: U32, +ty: U32, +id: U32) -> J.Val:
  +p0 = J.set(J.obj(), "kind", J.JS{kind})
  +p1 = ga.su(p0, "tx", U32.and(tx, 255))
  +p2 = ga.su(p1, "ty", U32.and(ty, 255))
  +p3 = ga.su(p2, "id", id)
  +p4 = ga.su(p3, "on", 0)
  +p5 = ga.su(p4, "t", 0)
  +p6 = ga.si(p5, "x", lanternward.world.middle(tx))
  ga.si(p6, "y", lanternward.world.middle(ty))

# Whether it is lit now.
def lanternward.props.Prop.lit(+p: J.Val) -> Bool:
  +k = ga.s(p, "kind")
  +on = ga.u(p, "on")
  Bool.pick(Bool, lanternward.NAME_IN(k, ["Rest", "Ember"]), True{},
    Bool.pick(Bool, String.eq(k, "Brazier"), Bool.or(U32.is_gt(ga.u(p, "t"), 0), U32.is_eq(on, 1)),
      Bool.pick(Bool, lanternward.NAME_IN(k, ["Great", "Beacon"]), U32.is_eq(on, 1), False{})))

# Whether a body at this point (Q pixels) is stopped by it.
def lanternward.props.Prop.blocks(+p: J.Val, +x: U32, +y: U32) -> Bool:
  +k = ga.s(p, "kind")
  +on = ga.u(p, "on")
  +here = Bool.and(
    U32.is_eq(lanternward.world.tile_of(x), ga.u(p, "tx")),
    U32.is_eq(lanternward.world.tile_of(y), ga.u(p, "ty")))
  +near = Bool.and(
    RS.i_lt(RS.i_abs((x - ga.u(p, "x") : U32)), 2048),
    RS.i_lt(RS.i_abs((y - ga.u(p, "y") : U32)), 2048))
  Bool.pick(Bool, lanternward.NAME_IN(k, ["Grass", "Plate", "Sigil", "Fissure"]), False{},
    Bool.pick(Bool, lanternward.NAME_IN(k, ["Pot", "Gate", "Lock", "Boss", "Cracked"]), Bool.and(U32.is_eq(on, 0), here),
      Bool.pick(Bool, String.eq(k, "Chest"), Bool.and(U32.is_ne(on, 2), here),
        Bool.pick(Bool, String.eq(k, "Block"), near, here))))

# Whether a bolt flies through it.
def lanternward.props.Prop.lets_bolts(+p: J.Val) -> Bool:
  lanternward.NAME_IN(ga.s(p, "kind"), ["Gate", "Sigil", "Grass", "Plate", "Fissure", "Pot"])

# Whether the knight in front of it can lock on to it.
def lanternward.props.Prop.target(+p: J.Val) -> Bool:
  +k = ga.s(p, "kind")
  Bool.pick(Bool, lanternward.NAME_IN(k, ["Sign", "Folk"]), True{},
    Bool.pick(Bool, lanternward.NAME_IN(k, ["Crystal", "Great", "Lever"]), U32.is_eq(ga.u(p, "on"), 0),
      Bool.pick(Bool, String.eq(k, "Brazier"), Bool.not(lanternward.props.Prop.lit(p)), False{})))

# What a chest holds, by its room and its place in it.
def lanternward.props.chest_item(+room: U32, +id: U32) -> J.Val:
  match room:
    case 0:
      J.JS{"Map"}
    case 1:
      lanternward.props.chest_item.r1(U32.is_eq(id, 0))
    case 2:
      lanternward.variant("Gems", J.JU{20})
    case 3:
      lanternward.props.chest_item.r3(id)
    case _:
      lanternward.variant("Gems", J.JU{10})

def lanternward.props.chest_item.r1(first: Bool) -> J.Val:
  match first:
    case True{}:
      J.JS{"Lantern"}
    case False{}:
      lanternward.variant("Gems", J.JU{25})

def lanternward.props.chest_item.r3(+id: U32) -> J.Val:
  match id:
    case 0:
      J.JS{"BossKey"}
    case 1:
      J.JS{"SmallKey"}
    case _:
      lanternward.variant("Gems", J.JU{25})

# The room's things, in the order the plan's rows list them. `acc` carries the
# list and the chest, sign and folk counters.
def lanternward.props.build.id(+kind: String, +c: U32, +acc: J.Val) -> U32:
  Bool.pick(U32, String.eq(kind, "Gate"), Bool.pick(U32, U32.is_eq(c, 71), 0, (c - 48 : U32)),
    Bool.pick(U32, String.eq(kind, "Chest"), ga.u(acc, "chests"),
      Bool.pick(U32, String.eq(kind, "Sign"), ga.u(acc, "signs"),
        Bool.pick(U32, String.eq(kind, "Folk"), ga.u(acc, "folk"),
          Bool.pick(U32, lanternward.NAME_IN(kind, ["Pot", "Plate"]), Bool.to_u32(Bool.or(U32.is_eq(c, 120), U32.is_eq(c, 89))), 0)))))

def lanternward.props.build.count(+kind: String, +acc: J.Val) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Chest"), ga.su(acc, "chests", (ga.u(acc, "chests") + 1 : U32)),
    Bool.pick(J.Val, String.eq(kind, "Sign"), ga.su(acc, "signs", (ga.u(acc, "signs") + 1 : U32)),
      Bool.pick(J.Val, String.eq(kind, "Folk"), ga.su(acc, "folk", (ga.u(acc, "folk") + 1 : U32)), acc)))

def lanternward.props.build.cell(+kind: J.Val, +room: U32, +c: U32, +x: U32, +y: U32, +acc: J.Val) -> J.Val:
  match kind:
    case J.JS{+kn}:
      lanternward.props.build.make(kn, room, c, x, y, lanternward.props.build.id(kn, c, acc), lanternward.props.build.count(kn, acc))
    case _:
      acc

def lanternward.props.build.make(+kn: String, +room: U32, +c: U32, +x: U32, +y: U32, +id: U32, +acc: J.Val) -> J.Val:
  +p = lanternward.props.Prop.new(kn, x, y, id)
  +on = Bool.pick(U32, Bool.and(String.eq(kn, "Chest"), Bool.and(U32.is_eq(room, 3), U32.is_eq(id, 1))), 2,
    Bool.pick(U32, String.eq(kn, "Rest"), 1, 0))
  ga.push(acc, "out", ga.su(p, "on", on))

def lanternward.props.build.row(s: String, +room: U32, +x: U32, +y: U32, +acc: J.Val) -> J.Val:
  match s:
    case SNil{}:
      acc
    case SCon{+h, t}:
      lanternward.props.build.row(t, room, (x + 1 : U32), y,
        lanternward.props.build.cell(lanternward.props.Pk.of(Char.to_u32(h)), room, Char.to_u32(h), x, y, acc))

def lanternward.props.build.rows(+rows: List<&2, String>, +room: U32, +y: U32, +acc: J.Val) -> J.Val:
  match rows:
    case Nil{}:
      acc
    case Con{+h, +t}:
      lanternward.props.build.rows(t, room, (y + 1 : U32), lanternward.props.build.row(h, room, 0, y, acc))

def lanternward.props.build(+room: U32) -> J.Val:
  J.get(
    lanternward.props.build.rows(lanternward.world.rows(lanternward.world.plan(room)), room, 0,
      ga.sv(ga.su(ga.su(ga.su(J.obj(), "chests", 0), "signs", 0), "folk", 0), "out", J.JA{Nil{}})),
    "out")

# ---- impl Session ----

def lanternward.props.Session.fx(+s: J.Val, +kind: String, +x: U32, +y: U32, +a: U32) -> J.Val:
  lanternward.props.Session.fx.push(U32.is_lt(lanternward.len(s, "fx"), 80), s, kind, x, y, a)

def lanternward.props.Session.fx.push(ok: Bool, +s: J.Val, +kind: String, +x: U32, +y: U32, +a: U32) -> J.Val:
  match ok:
    case True{}:
      lanternward.push(s, "fx",
        ga.si(ga.su(ga.si(ga.si(ga.sv(J.obj(), "kind", J.JS{kind}), "x", x), "y", y), "t", 0), "a", a))
    case False{}:
      s

def lanternward.props.Session.drop_at(+s: J.Val, +kind: String, +x: U32, +y: U32) -> J.Val:
  lanternward.props.Session.drop_at.push(U32.is_lt(lanternward.len(s, "drops"), 32), s, kind, x, y)

def lanternward.props.Session.drop_at.push(ok: Bool, +s: J.Val, +kind: String, +x: U32, +y: U32) -> J.Val:
  match ok:
    case True{}:
      lanternward.push(s, "drops",
        ga.su(ga.si(ga.si(ga.sv(J.obj(), "kind", J.JS{kind}), "x", x), "y", y), "age", 0))
    case False{}:
      s

# The fixed dice, in two halves: `roll` advances the seed, `rolled` reads the
# value `% n` off the new seed (Rust's `roll(n)` does both).
def lanternward.props.Session.roll(+s: J.Val) -> J.Val:
  ga.sv(s, "seed", W.to_j(W.add(W.mul(W.of_j(J.get(s, "seed")), W.W{1481765933, 1284865837}), W.W{335903614, 4150755663})))

def lanternward.props.Session.rolled(+s: J.Val, +n: U32) -> U32:
  W.rem_u32(W.shr(W.of_j(J.get(s, "seed")), 33), RS.u_max(n, 1))

# A pot or a tuft of grass breaks: what it held lies on the floor.
def lanternward.props.Session.shatter(+s: J.Val, +i: U32) -> J.Val:
  lanternward.props.Session.shatter.go(
    s, i, ga.s(lanternward.el(s, "props", i), "kind"), lanternward.eu(s, "props", i, "id"),
    lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"))

def lanternward.props.Session.shatter.go(+s: J.Val, +i: U32, +kind: String, +id: U32, +x: U32, +y: U32) -> J.Val:
  +s1 = lanternward.props.Session.fx(lanternward.es(s, "props", i, "on", 1), "Dust", x, y, 0)
  +pot = String.eq(kind, "Pot")
  lanternward.sound(
    lanternward.props.Session.shatter.drops(Bool.and(pot, U32.is_eq(id, 1)), s1, kind, x, y),
    Bool.pick(String, pot, "rock_land", "step_moss"))

def lanternward.props.Session.shatter.drops(bombs: Bool, +s: J.Val, +kind: String, +x: U32, +y: U32) -> J.Val:
  match bombs:
    case True{}:
      lanternward.props.Session.drop_at(
        lanternward.props.Session.drop_at(
          lanternward.props.Session.drop_at(s, "Bomb", (x - 1280 : U32), (y + 768 : U32)),
          "Bomb", x, (y + 768 : U32)),
        "Bomb", (x + 1280 : U32), (y + 768 : U32))
    case False{}:
      lanternward.props.Session.scatter(s, kind, x, y)

# What a cut or broken thing lets fall.
def lanternward.props.Session.scatter(+s: J.Val, +kind: String, +x: U32, +y: U32) -> J.Val:
  +s1 = lanternward.props.Session.roll(s)
  lanternward.props.Session.scatter.go(s1, lanternward.props.Session.scatter.pick(kind, lanternward.props.Session.rolled(s1, 100)), x, y)

def lanternward.props.Session.scatter.pick(+kind: String, +r: U32) -> String:
  Bool.pick(String, String.eq(kind, "Grass"),
    Bool.pick(String, U32.is_le(r, 11), "Gem", Bool.pick(String, U32.is_le(r, 21), "Heart", "")),
    Bool.pick(String, String.eq(kind, "Pot"),
      Bool.pick(String, U32.is_le(r, 17), "Heart", Bool.pick(String, U32.is_le(r, 44), "Gem", Bool.pick(String, U32.is_le(r, 54), "Bomb", ""))),
      Bool.pick(String, U32.is_le(r, 29), "Heart", Bool.pick(String, U32.is_le(r, 64), "Gem", ""))))

def lanternward.props.Session.scatter.go(+s: J.Val, +drop: String, +x: U32, +y: U32) -> J.Val:
  match drop:
    case SNil{}:
      s
    case _:
      lanternward.props.Session.drop_at(s, drop, x, y)

# Whether any closed thing in the room stops a body at this point.
def lanternward.props.Session.any_blocks(+ps: List<&2, J.Val>, +x: U32, +y: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(lanternward.props.Prop.blocks(h, x, y), lanternward.props.Session.any_blocks(t, x, y))

def lanternward.props.Session.blocked(+s: J.Val, +x: U32, +y: U32) -> Bool:
  Bool.or(
    lanternward.world.wall(lanternward.world.Plan.at(ga.u(s, "room"), lanternward.world.tile_of(x), lanternward.world.tile_of(y))),
    lanternward.props.Session.any_blocks(ga.items(s, "props"), x, y))

def lanternward.props.Session.secret_open.any(+ps: List<&2, J.Val>) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      +k = ga.s(h, "kind")
      Bool.or(
        Bool.and(
          Bool.or(String.eq(k, "Cracked"), Bool.and(String.eq(k, "Gate"), U32.is_eq(ga.u(h, "id"), 2))),
          U32.is_eq(ga.u(h, "on"), 1)),
        lanternward.props.Session.secret_open.any(t))

# Whether the room's secret closet has been opened.
def lanternward.props.Session.secret_open(+s: J.Val) -> Bool:
  lanternward.props.Session.secret_open.any(ga.items(s, "props"))

# Whether a body of radius `r` (Q pixels) fits at this point.
def lanternward.props.Session.clear(+s: J.Val, +x: U32, +y: U32, +r: U32) -> Bool:
  Bool.not(Bool.or(
    Bool.or(lanternward.props.Session.blocked(s, (x - r : U32), (y - r : U32)),
      lanternward.props.Session.blocked(s, (x + r : U32), (y - r : U32))),
    Bool.or(lanternward.props.Session.blocked(s, (x - r : U32), (y + r : U32)),
      lanternward.props.Session.blocked(s, (x + r : U32), (y + r : U32)))))

# Whether this point is over a pit.
def lanternward.props.Session.pit(+s: J.Val, +x: U32, +y: U32) -> Bool:
  U32.is_eq(lanternward.world.Plan.at(ga.u(s, "room"), lanternward.world.tile_of(x), lanternward.world.tile_of(y)), 61)

# Whether any prop that stops iron (not one that lets bolts pass) blocks (x, y).
def lanternward.props.Session.any_stops(+ps: List<&2, J.Val>, +x: U32, +y: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(
        Bool.and(Bool.not(lanternward.props.Prop.lets_bolts(h)), lanternward.props.Prop.blocks(h, x, y)),
        lanternward.props.Session.any_stops(t, x, y))

# One step `i` of the line: true when nothing stops a bolt there.
def lanternward.props.Session.line_open.step(+s: J.Val, +ax: U32, +ay: U32, +dx: U32, +dy: U32, +steps: U32, +i: U32, +ta: U32, +tb: U32) -> Bool:
  lanternward.props.Session.line_open.at(
    s, RS.i_add(ax, RS.i_div(RS.i_mul(dx, i), steps)), RS.i_add(ay, RS.i_div(RS.i_mul(dy, i), steps)), ta, tb)

def lanternward.props.Session.line_open.at(+s: J.Val, +x: U32, +y: U32, +ta: U32, +tb: U32) -> Bool:
  +t = (lanternward.world.tile_of(x) + 65536 * lanternward.world.tile_of(y) : U32)
  Bool.pick(Bool, Bool.or(U32.is_eq(t, ta), U32.is_eq(t, tb)), True{},
    Bool.and(
      U32.is_ne(lanternward.world.Plan.at(ga.u(s, "room"), lanternward.world.tile_of(x), lanternward.world.tile_of(y)), 35),
      Bool.not(lanternward.props.Session.any_stops(ga.items(s, "props"), x, y))))

def lanternward.props.Session.line_open.go(+s: J.Val, +ax: U32, +ay: U32, +dx: U32, +dy: U32, +steps: U32, +i: U32, +ta: U32, +tb: U32) -> Bool:
  lanternward.props.Session.line_open.next(
    U32.is_lt(i, steps), s, ax, ay, dx, dy, steps, i, ta, tb)

def lanternward.props.Session.line_open.next(more: Bool, +s: J.Val, +ax: U32, +ay: U32, +dx: U32, +dy: U32, +steps: U32, +i: U32, +ta: U32, +tb: U32) -> Bool:
  match more:
    case False{}:
      True{}
    case True{}:
      lanternward.props.Session.line_open.cont(
        lanternward.props.Session.line_open.step(s, ax, ay, dx, dy, steps, i, ta, tb),
        s, ax, ay, dx, dy, steps, i, ta, tb)

def lanternward.props.Session.line_open.cont(ok: Bool, +s: J.Val, +ax: U32, +ay: U32, +dx: U32, +dy: U32, +steps: U32, +i: U32, +ta: U32, +tb: U32) -> Bool:
  match ok:
    case False{}:
      False{}
    case True{}:
      lanternward.props.Session.line_open.go(s, ax, ay, dx, dy, steps, (i + 1 : U32), ta, tb)

# Whether the way from `a` to `b` is open to a bolt: only walls stop it.
def lanternward.props.Session.line_open(+s: J.Val, +ax: U32, +ay: U32, +bx: U32, +by: U32) -> Bool:
  +dx = (bx - ax : U32)
  +dy = (by - ay : U32)
  lanternward.props.Session.line_open.go(
    s, ax, ay, dx, dy,
    RS.i_max(RS.i_div(lanternward.world.dist(dx, dy), 1536), 1), 1,
    (lanternward.world.tile_of(ax) + 65536 * lanternward.world.tile_of(ay) : U32),
    (lanternward.world.tile_of(bx) + 65536 * lanternward.world.tile_of(by) : U32))

# Seat.flaring (kit.rs, a pure read of one seat).
def lanternward.kit.Seat.flaring(+k: J.Val) -> Bool:
  Bool.and(
    Bool.and(U32.is_ge(ga.u(k, "lamp"), 8), U32.is_gt(ga.u(k, "flame"), 0)),
    Bool.not(lanternward.is(k, "act", "Down")))

def lanternward.props.Session.burning.seats(+ids: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case Con{+id, +t}:
      Bool.or(
        Bool.and(
          lanternward.kit.Seat.flaring(ga.at(seats, id)),
          RS.i_lt(lanternward.world.dist((ga.u(ga.at(seats, id), "x") - x : U32), (ga.u(ga.at(seats, id), "y") - y : U32)), 19968)),
        lanternward.props.Session.burning.seats(t, seats, x, y))

def lanternward.props.Session.burning.props(+ps: List<&2, J.Val>, +x: U32, +y: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(
        Bool.and(
          lanternward.props.Prop.lit(h),
          RS.i_lt(lanternward.world.dist((ga.u(h, "x") - x : U32), (ga.u(h, "y") - y : U32)), 16384)),
        lanternward.props.Session.burning.props(t, x, y))

# Whether a point is held lit by a flare or a burning brazier.
def lanternward.props.Session.burning(+s: J.Val, +x: U32, +y: U32) -> Bool:
  Bool.or(
    lanternward.props.Session.burning.seats(ga.ids(J.get(s, "seats")), J.get(s, "seats"), x, y),
    lanternward.props.Session.burning.props(ga.items(s, "props"), x, y))

# The gate with this number, opened or shut. `$ret` is whether any changed.
def lanternward.props.Session.set_gate.go(+s: J.Val, +i: U32, +id: U32, +want: U32, +changed: Bool) -> J.Val:
  lanternward.props.Session.set_gate.next(U32.is_lt(i, lanternward.len(s, "props")), s, i, id, want, changed)

def lanternward.props.Session.set_gate.next(more: Bool, +s: J.Val, +i: U32, +id: U32, +want: U32, +changed: Bool) -> J.Val:
  match more:
    case False{}:
      ga.sb(s, "$ret", changed)
    case True{}:
      lanternward.props.Session.set_gate.at(
        Bool.and(lanternward.eis(s, "props", i, "kind", "Gate"), U32.is_eq(lanternward.eu(s, "props", i, "id"), id)),
        s, i, id, want, changed)

def lanternward.props.Session.set_gate.at(hit: Bool, +s: J.Val, +i: U32, +id: U32, +want: U32, +changed: Bool) -> J.Val:
  match hit:
    case False{}:
      lanternward.props.Session.set_gate.go(s, (i + 1 : U32), id, want, changed)
    case True{}:
      lanternward.props.Session.set_gate.set(U32.is_ne(lanternward.eu(s, "props", i, "on"), want), s, i, id, want, changed)

def lanternward.props.Session.set_gate.set(diff: Bool, +s: J.Val, +i: U32, +id: U32, +want: U32, +changed: Bool) -> J.Val:
  match diff:
    case True{}:
      lanternward.props.Session.set_gate.go(lanternward.es(s, "props", i, "on", want), (i + 1 : U32), id, want, True{})
    case False{}:
      lanternward.props.Session.set_gate.go(s, (i + 1 : U32), id, want, changed)

def lanternward.props.Session.set_gate(+s: J.Val, +id: U32, +open: Bool) -> J.Val:
  lanternward.props.Session.set_gate.go(s, 0, id, Bool.to_u32(open), False{})

# A puzzle came right: the chime, the star burst, a cue on the screen.
def lanternward.props.Session.solved(+s: J.Val, +x: U32, +y: U32) -> J.Val:
  lanternward.sound(
    lanternward.props.Session.solved.cue(J.is_null(J.get(s, "cue")),
      lanternward.props.Session.fx(s, "Solved", x, y, 0), x, y),
    "wish_grant")

def lanternward.props.Session.solved.cue(none: Bool, +s: J.Val, +x: U32, +y: U32) -> J.Val:
  match none:
    case True{}:
      ga.sv(s, "cue", J.JA{[J.of_i32(x), J.of_i32(y), J.JU{44}]})
    case False{}:
      s
