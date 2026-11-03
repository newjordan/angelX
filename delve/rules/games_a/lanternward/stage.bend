# crates/delve/src/rules/lanternward/stage.rs — the rooms, one after another:
# what each asks of the party, the way through, the fallen and the retry.

# Where seat `id` starts in this room: its own mark, wrapping: [x, y].
def lanternward.stage.Session.start_of(+s: J.Val, +id: U32) -> J.Val:
  lanternward.stage.start_of.marks(lanternward.world.Plan.find(ga.u(s, "room"), 64), id)

def lanternward.stage.start_of.marks(+marks: List<&2, U32>, +id: U32) -> J.Val:
  lanternward.stage.start_of.pick(marks, U32.mod((id - 1 : U32), U32.from_nat(List.length(&2, U32, marks))))

def lanternward.stage.start_of.pick(+marks: List<&2, U32>, +i: U32) -> J.Val:
  lanternward.stage.start_of.nth(marks, U32.to_nat(i))

def lanternward.stage.start_of.nth(+marks: List<&2, U32>, n: Nat) -> J.Val:
  match marks n:
    case Nil{} _:
      ga.pair_i(0, 0)
    case Con{+h, +t} 0n:
      ga.pair_i(lanternward.world.middle(lanternward.world.packed_x(h)), lanternward.world.middle(lanternward.world.packed_y(h)))
    case Con{+h, +t} 1n+p:
      lanternward.stage.start_of.nth(t, p)

# The party goes into room `room`: its things stand, its foes wait, and every
# seat is at its start (the fallen on their feet again).
def lanternward.stage.Session.enter_room(+s: J.Val, +room: U32) -> J.Val:
  lanternward.stage.enter_room.seats(
    ga.ids(J.get(s, "seats")),
    ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(s,
      "room", U32.and(room, 255)), "stage", 0), "stage_t", 0), "exiting", 0), "wipe", 0), "next_foe", 0), "boss", J.JNull{}),
      "props", lanternward.props.build(room)), "foes", J.JA{Nil{}}), "bolts", J.JA{Nil{}}), "drops", J.JA{Nil{}}), "fuses", J.JA{Nil{}}),
    room)

def lanternward.stage.enter_room.seats(+ids: List<&2, U32>, +s: J.Val, +room: U32) -> J.Val:
  match ids:
    case Nil{}:
      lanternward.stage.Session.provision_room(s, room)
    case Con{+id, +t}:
      lanternward.stage.enter_room.seats(t, lanternward.stage.enter_room.seat(s, id), room)

def lanternward.stage.enter_room.seat(+s: J.Val, +id: U32) -> J.Val:
  lanternward.stage.enter_room.seat.at(lanternward.stage.Session.start_of(s, id), ga.at(J.get(s, "seats"), id), s, id)

def lanternward.stage.enter_room.seat.at(+at: J.Val, +k: J.Val, +s: J.Val, +id: U32) -> J.Val:
  +k1 = ga.si(ga.si(ga.sv(ga.sv(k, "safe", at), "cam", at), "x", ga.nu(at, 0)), "y", ga.nu(at, 1))
  +k2 = ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(ga.sv(ga.sv(ga.si(k1, "z", 0), "lock", J.JNull{}), "act", J.JS{"Free"}), "t", 0), "shield", 0), "chain", 0), "step", 0), "charge", 0), "pusht", 0)
  +k3 = ga.sv(ga.su(ga.sv(k2, "push", ga.pair_i(0, 0)), "taper", 0), "act", J.JS{"Free"})
  +k4 = ga.su(Bool.pick(J.Val, U32.is_eq(ga.u(k3, "hp"), 0), ga.su(k3, "hp", RS.u_min(12, ga.u(k3, "max_hp"))), k3), "inv", 60)
  lanternward.blow.put_seat(s, id, k4)

# The foes (and the Warden) a room opens with.
def lanternward.stage.Session.provision_room(+s: J.Val, +room: U32) -> J.Val:
  lanternward.stage.provision_room.go(room, s, RS.u_max(lanternward.seat_count(s), 1))

def lanternward.stage.provision_room.go(+room: U32, +s: J.Val, +seats: U32) -> J.Val:
  match room:
    case 0:
      lanternward.stage.spawn_at(s, lanternward.world.Plan.find(0, 83), (1 + U32.div((seats - 1 : U32), 3) : U32), "Snuffer", False{})
    case 2:
      lanternward.stage.spawn_at(s, lanternward.world.Plan.find(2, 84), Bool.pick(U32, U32.is_ge(seats, 4), 2, 1), "Turret", False{})
    case 3:
      lanternward.stage.spawn_at(s, lanternward.world.Plan.find(3, 83), (2 + U32.div(seats, 2) : U32), "Snuffer", True{})
    case 4:
      lanternward.stage.provision_room.lamps(lanternward.stage.provision_room.boss(lanternward.world.Plan.find(4, 87), s, seats), 0)
    case _:
      s

# `for (tx, ty) in marks.take(n) { spawn_foe(kind, middle(tx), middle(ty), hooded) }`.
def lanternward.stage.spawn_at(+s: J.Val, +marks: List<&2, U32>, +n: U32, +kind: String, +hooded: Bool) -> J.Val:
  match marks:
    case Nil{}:
      s
    case Con{+m, +t}:
      lanternward.stage.spawn_at(
        Bool.pick(J.Val, U32.is_gt(n, 0),
          lanternward.foes.Session.spawn_foe(s, kind, lanternward.world.middle(lanternward.world.packed_x(m)), lanternward.world.middle(lanternward.world.packed_y(m)), hooded),
          s),
        t, RS.u_sat_sub(n, 1), kind, hooded)

# The Warden stands on the first W mark.
def lanternward.stage.provision_room.boss(+marks: List<&2, U32>, +s: J.Val, +seats: U32) -> J.Val:
  match marks:
    case Nil{}:
      s
    case Con{+m, +t}:
      ga.sv(s, "boss", lanternward.boss.Warden.new(lanternward.world.middle(lanternward.world.packed_x(m)), lanternward.world.middle(lanternward.world.packed_y(m)), seats))

# The arena's lamps burn until the Gloam falls.
def lanternward.stage.provision_room.lamps(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.provision_room.lamps.go(U32.is_lt(i, lanternward.len(s, "props")), s, i)

def lanternward.stage.provision_room.lamps.go(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.stage.provision_room.lamps(
        lanternward.stage.set_on(lanternward.eis(s, "props", i, "kind", "Brazier"), s, i), (i + 1 : U32))

def lanternward.stage.set_on(yes: Bool, +s: J.Val, +i: U32) -> J.Val:
  match yes:
    case True{}:
      lanternward.es(s, "props", i, "on", 1)
    case False{}:
      s

# The Lamp Hall's wave: the Gloam answers the Lantern.
def lanternward.stage.Session.spawn_wave(+s: J.Val) -> J.Val:
  lanternward.stage.spawn_at(
    s, lanternward.world.Plan.find(1, 83),
    RS.u_min((2 + U32.div((RS.u_max(lanternward.seat_count(s), 1) - 1 : U32), 2) : U32), 4), "Snuffer", False{})

# The index of the first prop of this kind and id, or null.
def lanternward.stage.prop_of.go(+ps: List<&2, J.Val>, +kind: String, +id: U32, +i: U32) -> J.Val:
  match ps:
    case Nil{}:
      J.JNull{}
    case Con{+p, +t}:
      lanternward.stage.prop_of.hit(
        Bool.and(String.eq(ga.s(p, "kind"), kind), U32.is_eq(ga.u(p, "id"), id)), t, kind, id, i)

def lanternward.stage.prop_of.hit(hit: Bool, +t: List<&2, J.Val>, +kind: String, +id: U32, +i: U32) -> J.Val:
  match hit:
    case True{}:
      J.JU{i}
    case False{}:
      lanternward.stage.prop_of.go(t, kind, id, (i + 1 : U32))

def lanternward.stage.Session.prop_of(+s: J.Val, +kind: String, +id: U32) -> J.Val:
  lanternward.stage.prop_of.go(ga.items(s, "props"), kind, id, 0)

# Whether the prop of this kind and id exists and has `on == 1`.
def lanternward.stage.prop_on(+s: J.Val, +kind: String, +id: U32) -> Bool:
  lanternward.stage.prop_on.go(lanternward.stage.Session.prop_of(s, kind, id), s)

def lanternward.stage.prop_on.go(+i: J.Val, +s: J.Val) -> Bool:
  match i:
    case J.JNull{}:
      False{}
    case _:
      U32.is_eq(lanternward.eu(s, "props", J.u32(i), "on"), 1)

# The position [x, y] of the prop at index `i`, or [0, 0] for none.
def lanternward.stage.prop_pos(+s: J.Val, +i: J.Val) -> J.Val:
  match i:
    case J.JNull{}:
      ga.pair_i(0, 0)
    case _:
      ga.pair_i(lanternward.eu(s, "props", J.u32(i), "x"), lanternward.eu(s, "props", J.u32(i), "y"))

# Every prop of this kind is lit (and there is at least one).
def lanternward.stage.all_lit.go(+ps: List<&2, J.Val>, +kind: String, +any: Bool) -> Bool:
  match ps:
    case Nil{}:
      any
    case Con{+p, +t}:
      lanternward.stage.all_lit.step(String.eq(ga.s(p, "kind"), kind), lanternward.props.Prop.lit(p), t, kind, any)

def lanternward.stage.all_lit.step(+mine: Bool, +lit: Bool, +t: List<&2, J.Val>, +kind: String, +any: Bool) -> Bool:
  Bool.pick(Bool, Bool.and(mine, Bool.not(lit)), False{},
    lanternward.stage.all_lit.go(t, kind, Bool.or(any, mine)))

def lanternward.stage.Session.all_lit(+s: J.Val, +kind: String) -> Bool:
  lanternward.stage.all_lit.go(ga.items(s, "props"), kind, False{})

def lanternward.stage.foes_down.go(+fs: List<&2, J.Val>) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{+f, +t}:
      Bool.and(Bool.not(lanternward.foes.Foe.alive(f)), lanternward.stage.foes_down.go(t))

def lanternward.stage.Session.foes_down(+s: J.Val) -> Bool:
  lanternward.stage.foes_down.go(ga.items(s, "foes"))

# Whether the way on is open.
def lanternward.stage.Session.exit_open(+s: J.Val) -> Bool:
  lanternward.stage.exit_open.go(ga.u(s, "room"), ga.u(s, "stage"))

def lanternward.stage.exit_open.go(+room: U32, +stage: U32) -> Bool:
  match room:
    case 0:
      U32.is_ge(stage, 1)
    case 1:
      U32.is_ge(stage, 4)
    case 2:
      U32.is_ge(stage, 1)
    case 3:
      U32.is_ge(stage, 4)
    case _:
      False{}

# ---- the things in the room ----

# Positions that press a plate: knights on the ground, then every block (a
# flat list x, y, x, y, ...).
def lanternward.stage.weights.seats(+ids: List<&2, U32>, +seats: J.Val) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      Bool.pick(List<&2, U32>,
        Bool.and(lanternward.kit.Seat.up(ga.at(seats, id)), U32.is_eq(ga.u(ga.at(seats, id), "z"), 0)),
        Con{ga.u(ga.at(seats, id), "x"), Con{ga.u(ga.at(seats, id), "y"), lanternward.stage.weights.seats(t, seats)}},
        lanternward.stage.weights.seats(t, seats))

def lanternward.stage.weights.blocks(+ps: List<&2, J.Val>) -> List<&2, U32>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      Bool.pick(List<&2, U32>, String.eq(ga.s(p, "kind"), "Block"),
        Con{ga.u(p, "x"), Con{ga.u(p, "y"), lanternward.stage.weights.blocks(t)}},
        lanternward.stage.weights.blocks(t))

# Positions of the knights holding a flare.
def lanternward.stage.flares(+ids: List<&2, U32>, +seats: J.Val) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      Bool.pick(List<&2, U32>, lanternward.kit.Seat.flaring(ga.at(seats, id)),
        Con{ga.u(ga.at(seats, id), "x"), Con{ga.u(ga.at(seats, id), "y"), lanternward.stage.flares(t, seats)}},
        lanternward.stage.flares(t, seats))

# Whether any (x, y) pair in the flat list lies within `reach` of (px, py).
def lanternward.stage.any_near(+xs: List<&2, U32>, +px: U32, +py: U32, +reach: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+x, +r1}:
      lanternward.stage.any_near.y(r1, x, px, py, reach)

def lanternward.stage.any_near.y(+r1: List<&2, U32>, +x: U32, +px: U32, +py: U32, +reach: U32) -> Bool:
  match r1:
    case Nil{}:
      False{}
    case Con{+y, +rest}:
      Bool.or(
        RS.i_lt(lanternward.world.dist((x - px : U32), (y - py : U32)), reach),
        lanternward.stage.any_near(rest, px, py, reach))

def lanternward.stage.u32s(+xs: List<&2, U32>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{J.of_i32(h), lanternward.stage.u32s(t)}

def lanternward.stage.j32s(+xs: List<&2, J.Val>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{J.u32(h), lanternward.stage.j32s(t)}

# One tick of the things in the room. `$weights` and `$flares` ride in `s`;
# a sigil found leaves its place in `$revealed`.
def lanternward.stage.Session.tick_props(+s: J.Val) -> J.Val:
  lanternward.stage.tick_props.reveal(
    lanternward.stage.tick_props.loop(
      ga.sv(ga.sv(ga.sv(s, "$weights", J.JA{lanternward.stage.u32s(
        List.append(&2, U32, lanternward.stage.weights.seats(ga.ids(J.get(s, "seats")), J.get(s, "seats")), lanternward.stage.weights.blocks(ga.items(s, "props"))))}),
        "$flares", J.JA{lanternward.stage.u32s(lanternward.stage.flares(ga.ids(J.get(s, "seats")), J.get(s, "seats")))}),
        "$revealed", J.JNull{}),
      0))

def lanternward.stage.tick_props.loop(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.loop.go(U32.is_lt(i, lanternward.len(s, "props")), s, i)

def lanternward.stage.tick_props.loop.go(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.stage.tick_props.loop(
        lanternward.stage.tick_props.kind(ga.s(lanternward.el(s, "props", i), "kind"), s, i), (i + 1 : U32))

def lanternward.stage.tick_props.kind(+kind: String, +s: J.Val, +i: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Brazier"), lanternward.stage.tick_props.brazier(U32.is_gt(lanternward.eu(s, "props", i, "t"), 0), s, i),
    Bool.pick(J.Val, String.eq(kind, "Plate"), lanternward.stage.tick_props.plate(s, i),
      Bool.pick(J.Val, String.eq(kind, "Block"), lanternward.stage.tick_props.block(s, i),
        Bool.pick(J.Val, String.eq(kind, "Sigil"), lanternward.stage.tick_props.sigil(U32.is_eq(lanternward.eu(s, "props", i, "on"), 0), s, i),
          Bool.pick(J.Val, String.eq(kind, "Fissure"), lanternward.es(s, "props", i, "t", RS.u_sat_sub(lanternward.eu(s, "props", i, "t"), 1)),
            s)))))

# A brazier gutters: its flame counts down and puffs out unless lit for good.
def lanternward.stage.tick_props.brazier(burning: Bool, +s: J.Val, +i: U32) -> J.Val:
  match burning:
    case False{}:
      s
    case True{}:
      lanternward.stage.tick_props.brazier.out(
        Bool.and(U32.is_eq(lanternward.eu(s, "props", i, "t"), 1), U32.is_eq(lanternward.eu(s, "props", i, "on"), 0)),
        lanternward.es(s, "props", i, "t", (lanternward.eu(s, "props", i, "t") - 1 : U32)), i)

def lanternward.stage.tick_props.brazier.out(puff: Bool, +s: J.Val, +i: U32) -> J.Val:
  match puff:
    case False{}:
      s
    case True{}:
      lanternward.sound(lanternward.props.Session.fx(s, "Puff", lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 0), "step_ash")

# A plate is pressed by a knight on the ground or a block on it.
def lanternward.stage.tick_props.plate(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.plate.set(
    lanternward.eu(s, "props", i, "on"),
    Bool.to_u32(lanternward.stage.any_near(lanternward.stage.j32s(ga.items(s, "$weights")), lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 2304)),
    s, i)

def lanternward.stage.tick_props.plate.set(+was: U32, +now: U32, +s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.plate.sound(U32.is_ne(was, now), now, lanternward.es(s, "props", i, "on", now))

def lanternward.stage.tick_props.plate.sound(changed: Bool, +now: U32, +s: J.Val) -> J.Val:
  match changed:
    case False{}:
      s
    case True{}:
      lanternward.sound(s, Bool.pick(String, U32.is_eq(now, 1), "wall_up", "step_stone"))

# A block slides a pixel a tick toward where it was pushed, then settles.
def lanternward.stage.tick_props.block(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.block.go(U32.is_gt(lanternward.eu(s, "props", i, "on"), 0), s, i)

def lanternward.stage.tick_props.block.go(sliding: Bool, +s: J.Val, +i: U32) -> J.Val:
  match sliding:
    case False{}:
      lanternward.es(s, "props", i, "t", RS.u_sat_sub(lanternward.eu(s, "props", i, "t"), 1))
    case True{}:
      lanternward.stage.tick_props.block.move(lanternward.eu(s, "props", i, "on"), s, i)

def lanternward.stage.tick_props.block.move(+on: U32, +s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.block.t(
    lanternward.esi(lanternward.esi(s, "props", i, "x",
      (lanternward.eu(s, "props", i, "x") + Bool.pick(U32, U32.is_eq(on, 1), 256, Bool.pick(U32, U32.is_eq(on, 2), 4294967040, 0)) : U32)),
      "props", i, "y",
      (lanternward.eu(s, "props", i, "y") + Bool.pick(U32, U32.is_eq(on, 3), 256, Bool.pick(U32, Bool.or(U32.is_eq(on, 1), U32.is_eq(on, 2)), 0, 4294967040)) : U32)),
    i)

def lanternward.stage.tick_props.block.t(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.tick_props.block.settle(
    U32.is_le(lanternward.eu(s, "props", i, "t"), 1), lanternward.es(s, "props", i, "t", RS.u_sat_sub(lanternward.eu(s, "props", i, "t"), 1)), i)

def lanternward.stage.tick_props.block.settle(done: Bool, +s: J.Val, +i: U32) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.stage.tick_props.block.snap(
        lanternward.world.middle(lanternward.world.tile_of(lanternward.eu(s, "props", i, "x"))),
        lanternward.world.middle(lanternward.world.tile_of(lanternward.eu(s, "props", i, "y"))), s, i)

def lanternward.stage.tick_props.block.snap(+x: U32, +y: U32, +s: J.Val, +i: U32) -> J.Val:
  lanternward.sound(
    lanternward.es(
      lanternward.es(
        lanternward.es(
          lanternward.esi(lanternward.esi(s, "props", i, "x", x), "props", i, "y", y),
          "props", i, "tx", U32.and(lanternward.world.tile_of(x), 255)),
        "props", i, "ty", U32.and(lanternward.world.tile_of(y), 255)),
      "props", i, "on", 0),
    "rock_land")

# A sigil under a flare fills; unlit it fades. Found, it names its place.
def lanternward.stage.tick_props.sigil(dark: Bool, +s: J.Val, +i: U32) -> J.Val:
  match dark:
    case False{}:
      s
    case True{}:
      lanternward.stage.tick_props.sigil.go(
        lanternward.stage.any_near(lanternward.stage.j32s(ga.items(s, "$flares")), lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 19968),
        s, i)

def lanternward.stage.tick_props.sigil.go(lit: Bool, +s: J.Val, +i: U32) -> J.Val:
  match lit:
    case False{}:
      lanternward.es(s, "props", i, "t", RS.u_sat_sub(lanternward.eu(s, "props", i, "t"), 1))
    case True{}:
      lanternward.stage.tick_props.sigil.found(
        U32.is_ge((lanternward.eu(s, "props", i, "t") + 1 : U32), 24),
        lanternward.es(s, "props", i, "t", (lanternward.eu(s, "props", i, "t") + 1 : U32)), i)

def lanternward.stage.tick_props.sigil.found(done: Bool, +s: J.Val, +i: U32) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      ga.sv(lanternward.es(s, "props", i, "on", 1), "$revealed", ga.pair_i(lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y")))

# A sigil found opens what it was hiding.
def lanternward.stage.tick_props.reveal(+s: J.Val) -> J.Val:
  lanternward.stage.tick_props.reveal.go(ga.v(s, "$revealed"), J.del(J.del(J.del(s, "$weights"), "$flares"), "$revealed"))

def lanternward.stage.tick_props.reveal.go(+at: J.Val, +s: J.Val) -> J.Val:
  match at:
    case J.JNull{}:
      s
    case _:
      lanternward.sound(
        lanternward.props.Session.solved(lanternward.props.Session.set_gate(s, 2, True{}), ga.nu(at, 0), ga.nu(at, 1)),
        "door_open")

# A gate that closed or a block that slid over a body puts it out on the
# nearest open ground: nobody is walled in.
def lanternward.stage.Session.unstick(+s: J.Val) -> J.Val:
  lanternward.stage.unstick.ids(ga.ids(J.get(s, "seats")), s)

def lanternward.stage.unstick.ids(+ids: List<&2, U32>, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.stage.unstick.ids(t, lanternward.stage.unstick.one(s, id))

def lanternward.stage.unstick.one(+s: J.Val, +id: U32) -> J.Val:
  lanternward.stage.unstick.go(
    Bool.not(lanternward.props.Session.clear(s, ga.u(ga.at(J.get(s, "seats"), id), "x"), ga.u(ga.at(J.get(s, "seats"), id), "y"), 640)),
    s, id)

def lanternward.stage.unstick.go(stuck: Bool, +s: J.Val, +id: U32) -> J.Val:
  match stuck:
    case False{}:
      s
    case True{}:
      lanternward.stage.unstick.set(
        lanternward.foes.Session.nudge(s, ga.u(ga.at(J.get(s, "seats"), id), "x"), ga.u(ga.at(J.get(s, "seats"), id), "y"), 1280), s, id)

def lanternward.stage.unstick.set(+p: J.Val, +s: J.Val, +id: U32) -> J.Val:
  lanternward.blow.put_seat(s, id, ga.si(ga.si(ga.at(J.get(s, "seats"), id), "x", ga.nu(p, 0)), "y", ga.nu(p, 1)))

# ---- what each room asks ----

# What each room asks, and what it gives.
def lanternward.stage.Session.tick_stage(+s: J.Val) -> J.Val:
  lanternward.stage.tick_stage.exit(
    lanternward.stage.tick_stage.room(ga.u(s, "room"), ga.su(s, "stage_t", RS.u_min((ga.u(s, "stage_t") + 1 : U32), 108000))))

def lanternward.stage.tick_stage.room(+room: U32, +s: J.Val) -> J.Val:
  match room:
    case 0:
      lanternward.stage.stair(Bool.and(U32.is_eq(ga.u(s, "stage"), 0), lanternward.stage.Session.foes_down(s)), s)
    case 1:
      lanternward.stage.Session.stage_lamp_hall(s)
    case 2:
      lanternward.stage.Session.stage_gallery(s)
    case 3:
      lanternward.stage.Session.stage_vault(s)
    case _:
      lanternward.boss.Session.stage_warden(s)

def lanternward.stage.stair(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.stage.Session.open_exit(ga.su(s, "stage", 1))

# The way on: stand in it.
def lanternward.stage.tick_stage.exit(+s: J.Val) -> J.Val:
  lanternward.stage.tick_stage.exit.go(
    Bool.and(U32.is_eq(ga.u(s, "exiting"), 0), Bool.and(lanternward.stage.Session.exit_open(s), U32.is_lt(ga.u(s, "room"), 4))),
    s)

def lanternward.stage.tick_stage.exit.go(opening: Bool, +s: J.Val) -> J.Val:
  match opening:
    case True{}:
      lanternward.stage.tick_stage.exit.hit(
        lanternward.stage.stands_in_exit(ga.ids(J.get(s, "seats")), J.get(s, "seats"), ga.u(s, "room")), s)
    case False{}:
      lanternward.stage.tick_stage.exit.count(U32.is_gt(ga.u(s, "exiting"), 0), s)

def lanternward.stage.stands_in_exit(+ids: List<&2, U32>, +seats: J.Val, +room: U32) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case Con{+id, +t}:
      Bool.or(
        Bool.and(
          Bool.and(lanternward.kit.Seat.up(ga.at(seats, id)), U32.is_eq(ga.u(ga.at(seats, id), "z"), 0)),
          U32.is_eq(lanternward.world.Plan.at(room, lanternward.world.tile_of(ga.u(ga.at(seats, id), "x")), lanternward.world.tile_of(ga.u(ga.at(seats, id), "y"))), 81)),
        lanternward.stage.stands_in_exit(t, seats, room))

def lanternward.stage.tick_stage.exit.hit(hit: Bool, +s: J.Val) -> J.Val:
  match hit:
    case True{}:
      lanternward.sound(ga.su(s, "exiting", 14), "door_pass")
    case False{}:
      s

def lanternward.stage.tick_stage.exit.count(counting: Bool, +s: J.Val) -> J.Val:
  match counting:
    case False{}:
      s
    case True{}:
      lanternward.stage.tick_stage.exit.leave(U32.is_eq((ga.u(s, "exiting") - 1 : U32), 0), ga.su(s, "exiting", (ga.u(s, "exiting") - 1 : U32)))

def lanternward.stage.tick_stage.exit.leave(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.sound(lanternward.stage.Session.enter_room(s, (ga.u(s, "room") + 1 : U32)), "descend")

# The exit gate swings open. (`set_gate` leaves whether any changed in `$ret`.)
def lanternward.stage.Session.open_exit(+s: J.Val) -> J.Val:
  lanternward.stage.open_exit.go(lanternward.props.Session.set_gate(s, 0, True{}))

def lanternward.stage.open_exit.go(+s: J.Val) -> J.Val:
  lanternward.stage.open_exit.solved(ga.b(s, "$ret"), J.del(s, "$ret"))

def lanternward.stage.open_exit.solved(changed: Bool, +s: J.Val) -> J.Val:
  match changed:
    case False{}:
      s
    case True{}:
      lanternward.stage.open_exit.at(lanternward.find_prop(s, "Gate", 0), s)

def lanternward.stage.open_exit.at(+g: J.Val, +s: J.Val) -> J.Val:
  match g:
    case J.JNull{}:
      lanternward.sound(lanternward.props.Session.solved(s, 0, 0), "door_open")
    case _:
      lanternward.sound(lanternward.props.Session.solved(s, ga.u(g, "x"), ga.u(g, "y")), "door_open")

# The Lamp Hall: braziers, the Lantern, the Gloam's wave, the great brazier.
def lanternward.stage.Session.stage_lamp_hall(+s: J.Val) -> J.Val:
  lanternward.stage.lamp_hall.go(ga.u(s, "stage"), s)

def lanternward.stage.lamp_hall.go(+stage: U32, +s: J.Val) -> J.Val:
  match stage:
    case 0:
      lanternward.stage.lamp_hall.braziers(lanternward.stage.Session.all_lit(s, "Brazier"), s)
    case 1:
      lanternward.stage.lamp_hall.chest(lanternward.stage.prop_on(s, "Chest", 0), s)
    case 2:
      lanternward.stage.lamp_hall.wave(Bool.and(U32.is_gt(ga.u(s, "stage_t"), 20), lanternward.stage.Session.foes_down(s)), s)
    case 3:
      lanternward.stage.lamp_hall.great(lanternward.stage.prop_on(s, "Great", 0), s)
    case _:
      s

# Three braziers, all alight at once, from one ember.
def lanternward.stage.lamp_hall.braziers(lit: Bool, +s: J.Val) -> J.Val:
  match lit:
    case False{}:
      s
    case True{}:
      lanternward.stage.lamp_hall.braziers.at(
        lanternward.stage.prop_pos(s, lanternward.stage.Session.prop_of(s, "Chest", 0)), ga.su(s, "stage", 1))

def lanternward.stage.lamp_hall.braziers.at(+at: J.Val, +s: J.Val) -> J.Val:
  lanternward.sound(
    lanternward.props.Session.solved(
      J.del(lanternward.props.Session.set_gate(s, 1, True{}), "$ret"), ga.nu(at, 0), ga.nu(at, 1)),
    "door_open")

# The niche stands open; the Lantern is in the chest.
def lanternward.stage.lamp_hall.chest(open: Bool, +s: J.Val) -> J.Val:
  match open:
    case False{}:
      s
    case True{}:
      lanternward.sound(lanternward.stage.Session.spawn_wave(ga.su(ga.su(s, "stage", 2), "stage_t", 0)), "wave_gate")

def lanternward.stage.lamp_hall.wave(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.sound(ga.su(s, "stage", 3), "play_card")

def lanternward.stage.lamp_hall.great(lit: Bool, +s: J.Val) -> J.Val:
  match lit:
    case False{}:
      s
    case True{}:
      lanternward.stage.Session.open_exit(ga.su(s, "stage", 4))

# The Arrow Gallery: two plates and a crystal.
def lanternward.stage.plate_held(+ps: List<&2, J.Val>, +id: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+p, +t}:
      Bool.or(
        Bool.and(String.eq(ga.s(p, "kind"), "Plate"), Bool.and(U32.is_eq(ga.u(p, "id"), id), U32.is_eq(ga.u(p, "on"), 1))),
        lanternward.stage.plate_held(t, id))

def lanternward.stage.any_on(+ps: List<&2, J.Val>, +kind: String, +id: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+p, +t}:
      Bool.or(
        Bool.and(String.eq(ga.s(p, "kind"), kind), Bool.and(Bool.or(U32.is_eq(id, 4294967295), U32.is_eq(ga.u(p, "id"), id)), U32.is_eq(ga.u(p, "on"), 1))),
        lanternward.stage.any_on(t, kind, id))

def lanternward.stage.Session.stage_gallery(+s: J.Val) -> J.Val:
  lanternward.stage.gallery.go(
    Bool.and(
      lanternward.stage.plate_held(ga.items(s, "props"), 0),
      Bool.or(lanternward.stage.plate_held(ga.items(s, "props"), 1), U32.is_lt(lanternward.seat_count(s), 2))),
    lanternward.stage.any_on(ga.items(s, "props"), "Crystal", 4294967295),
    lanternward.stage.any_on(ga.items(s, "props"), "Gate", 1), s)

def lanternward.stage.gallery.go(+plate: Bool, +crystal: Bool, +was: Bool, +s: J.Val) -> J.Val:
  lanternward.stage.gallery.gate(Bool.or(plate, crystal), was, crystal, lanternward.props.Session.set_gate(s, 1, Bool.or(plate, crystal)))

# The plate-gate holds open only while it is held, or for good once the
# crystal has woken.
def lanternward.stage.gallery.gate(+want: Bool, +was: Bool, +crystal: Bool, +s: J.Val) -> J.Val:
  lanternward.stage.gallery.after(
    Bool.and(ga.b(s, "$ret"), Bool.xor(want, was)), want, crystal, J.del(s, "$ret"))

def lanternward.stage.gallery.after(changed: Bool, +want: Bool, +crystal: Bool, +s: J.Val) -> J.Val:
  lanternward.stage.gallery.crystal(crystal, lanternward.stage.gallery.sound(changed, want, s))

def lanternward.stage.gallery.sound(yes: Bool, +want: Bool, +s: J.Val) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.stage.gallery.sound.at(lanternward.find_prop(s, "Gate", 1), want, s)

def lanternward.stage.gallery.sound.at(+g: J.Val, +want: Bool, +s: J.Val) -> J.Val:
  match g:
    case J.JNull{}:
      lanternward.stage.gallery.sound.dust(want, 0, 0, lanternward.sound(s, Bool.pick(String, want, "door_open", "door_seal")))
    case _:
      lanternward.stage.gallery.sound.dust(want, ga.u(g, "x"), ga.u(g, "y"), lanternward.sound(s, Bool.pick(String, want, "door_open", "door_seal")))

def lanternward.stage.gallery.sound.dust(want: Bool, +x: U32, +y: U32, +s: J.Val) -> J.Val:
  match want:
    case True{}:
      lanternward.props.Session.fx(s, "Dust", x, y, 0)
    case False{}:
      s

def lanternward.stage.gallery.crystal(+crystal: Bool, +s: J.Val) -> J.Val:
  lanternward.stage.gallery.open(Bool.and(U32.is_eq(ga.u(s, "stage"), 0), crystal), s)

def lanternward.stage.gallery.open(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.stage.Session.open_exit(ga.su(s, "stage", 1))

# The Key Vault: the dark beaten, the key chest, the small key's door, the
# boss key.
def lanternward.stage.Session.stage_vault(+s: J.Val) -> J.Val:
  lanternward.stage.vault.boss(lanternward.stage.vault.go(ga.u(s, "stage"), s))

def lanternward.stage.vault.go(+stage: U32, +s: J.Val) -> J.Val:
  match stage:
    case 0:
      lanternward.stage.vault.dark(lanternward.stage.Session.foes_down(s), s)
    case 1:
      lanternward.stage.vault.step(lanternward.stage.prop_on(s, "Chest", 1), 2, s)
    case 2:
      lanternward.stage.vault.step(lanternward.stage.any_on(ga.items(s, "props"), "Lock", 4294967295), 3, s)
    case 3:
      lanternward.stage.vault.step(lanternward.stage.prop_on(s, "Chest", 0), 4, s)
    case _:
      s

def lanternward.stage.vault.step(yes: Bool, +to: U32, +s: J.Val) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      ga.su(s, "stage", to)

# The vault's key chest is not there until the dark is beaten; the spark the
# last snuffer gave back lights the vault's braziers for good.
def lanternward.stage.vault.dark(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.stage.provision_room.lamps(
        lanternward.stage.vault.chest(lanternward.stage.Session.prop_of(s, "Chest", 1), ga.su(s, "stage", 1)), 0)

def lanternward.stage.vault.chest(+i: J.Val, +s: J.Val) -> J.Val:
  match i:
    case J.JNull{}:
      s
    case _:
      lanternward.props.Session.solved(
        lanternward.props.Session.fx(lanternward.es(s, "props", J.u32(i), "on", 0), "Light", lanternward.eu(s, "props", J.u32(i), "x"), lanternward.eu(s, "props", J.u32(i), "y"), 0),
        lanternward.eu(s, "props", J.u32(i), "x"), lanternward.eu(s, "props", J.u32(i), "y"))

def lanternward.stage.vault.boss(+s: J.Val) -> J.Val:
  lanternward.stage.vault.step(
    Bool.and(lanternward.stage.any_on(ga.items(s, "props"), "Boss", 4294967295), U32.is_ge(ga.u(s, "stage"), 4)), 5, s)

# ---- drops ----

# Drops: hearts mend, gems and bombs are kept, the Boss Key is carried.
def lanternward.stage.Session.pickups(+s: J.Val) -> J.Val:
  lanternward.stage.pickups.drops(lanternward.stage.pickups.lying(lanternward.stage.pickups.follow(s)), 0)

# The Boss Key follows its carrier; a carrier who falls or leaves puts it
# down where they stood.
def lanternward.stage.pickups.follow(+s: J.Val) -> J.Val:
  lanternward.stage.pickups.follow.go(lanternward.tag_is(ga.v(ga.v(s, "inv"), "boss"), "Carried"), s)

def lanternward.stage.pickups.follow.go(carried: Bool, +s: J.Val) -> J.Val:
  match carried:
    case False{}:
      s
    case True{}:
      lanternward.stage.pickups.follow.who(ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), s)

def lanternward.stage.pickups.follow.who(+id: U32, +s: J.Val) -> J.Val:
  lanternward.stage.pickups.follow.up(
    Bool.and(ga.has_key(J.get(s, "seats"), id), lanternward.kit.Seat.up(ga.at(J.get(s, "seats"), id))), id, s)

def lanternward.stage.pickups.follow.up(up: Bool, +id: U32, +s: J.Val) -> J.Val:
  match up:
    case True{}:
      ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss",
        lanternward.variant("Carried", J.JA{[J.JU{id}, J.of_i32(ga.u(ga.at(J.get(s, "seats"), id), "x")), J.of_i32(ga.u(ga.at(J.get(s, "seats"), id), "y"))]})))
    case False{}:
      ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss",
        lanternward.variant("Lying", J.JA{[J.nth(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 1), J.nth(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 2)]})))

# The first knight (by seat) standing on a lying key picks it up.
def lanternward.stage.pickups.lying(+s: J.Val) -> J.Val:
  lanternward.stage.pickups.lying.go(lanternward.tag_is(ga.v(ga.v(s, "inv"), "boss"), "Lying"), s)

def lanternward.stage.pickups.lying.go(lying: Bool, +s: J.Val) -> J.Val:
  match lying:
    case False{}:
      s
    case True{}:
      lanternward.stage.pickups.lying.find(
        lanternward.stage.first_near(ga.ids(J.get(s, "seats")), J.get(s, "seats"),
          ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 1)),
        s)

# The first up knight within pick-up reach of (x, y): the seat id, or null.
def lanternward.stage.first_near(+ids: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32) -> J.Val:
  match ids:
    case Nil{}:
      J.JNull{}
    case Con{+id, +t}:
      lanternward.stage.first_near.hit(
        Bool.and(lanternward.kit.Seat.up(ga.at(seats, id)),
          RS.i_lt(lanternward.world.dist((ga.u(ga.at(seats, id), "x") - x : U32), (ga.u(ga.at(seats, id), "y") - y : U32)), 2560)),
        id, t, seats, x, y)

def lanternward.stage.first_near.hit(hit: Bool, +id: U32, +t: List<&2, U32>, +seats: J.Val, +x: U32, +y: U32) -> J.Val:
  match hit:
    case True{}:
      J.JU{id}
    case False{}:
      lanternward.stage.first_near(t, seats, x, y)

def lanternward.stage.pickups.lying.find(+id: J.Val, +s: J.Val) -> J.Val:
  match id:
    case J.JNull{}:
      s
    case _:
      lanternward.sound(
        ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss",
          lanternward.variant("Carried", J.JA{[J.JU{U32.and(J.u32(id), 255)}, J.nth(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), J.nth(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 1)]}))),
        "card_pickup")

def lanternward.stage.pickups.drops(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.pickups.drops.go(U32.is_lt(i, lanternward.len(s, "drops")), s, i)

def lanternward.stage.pickups.drops.go(more: Bool, +s: J.Val, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.stage.pickups.drop(
        lanternward.es(s, "drops", i, "age", RS.u_min((lanternward.eu(s, "drops", i, "age") + 1 : U32), 65535)), i)

def lanternward.stage.pickups.drop(+s: J.Val, +i: U32) -> J.Val:
  lanternward.stage.pickups.drop.taker(
    lanternward.stage.first_near(ga.ids(J.get(s, "seats")), J.get(s, "seats"), lanternward.eu(s, "drops", i, "x"), lanternward.eu(s, "drops", i, "y")),
    ga.s(lanternward.el(s, "drops", i), "kind"),
    Bool.and(U32.is_gt(lanternward.eu(s, "drops", i, "age"), 900), Bool.not(lanternward.eis(s, "drops", i, "kind", "Container"))),
    s, i)

def lanternward.stage.pickups.drop.taker(+taker: J.Val, +kind: String, +old: Bool, +s: J.Val, +i: U32) -> J.Val:
  match taker:
    case J.JNull{}:
      lanternward.stage.pickups.drop.end(old, s, i)
    case _:
      lanternward.stage.pickups.drop.take(J.u32(taker), kind, old, s, i)

def lanternward.stage.pickups.drop.take(+id: U32, +kind: String, +old: Bool, +s: J.Val, +i: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Heart"),
    lanternward.stage.pickups.heart(
      U32.is_lt(ga.u(ga.at(J.get(s, "seats"), id), "hp"), ga.u(ga.at(J.get(s, "seats"), id), "max_hp")), id, old, s, i),
    Bool.pick(J.Val, String.eq(kind, "Gem"),
      lanternward.stage.pickups.drop.end(True{},
        lanternward.sound(
          lanternward.blow.put_seat(s, id, ga.su(ga.at(J.get(s, "seats"), id), "gems", (ga.u(ga.at(J.get(s, "seats"), id), "gems") + 1 : U32))),
          "gold"), i),
      Bool.pick(J.Val, String.eq(kind, "Bomb"),
        lanternward.stage.pickups.drop.end(True{},
          lanternward.sound(
            lanternward.blow.put_seat(s, id, ga.su(ga.at(J.get(s, "seats"), id), "bombs", RS.u_min((ga.u(ga.at(J.get(s, "seats"), id), "bombs") + 1 : U32), 10))),
            "spoil_pickup"), i),
        lanternward.stage.pickups.drop.end(True{}, lanternward.stage.pickups.container(s, id), i))))

def lanternward.stage.pickups.heart(mend: Bool, +id: U32, +old: Bool, +s: J.Val, +i: U32) -> J.Val:
  match mend:
    case False{}:
      lanternward.stage.pickups.drop.end(old, s, i)
    case True{}:
      lanternward.stage.pickups.drop.end(True{},
        lanternward.sound(
          lanternward.blow.put_seat(s, id,
            ga.su(ga.at(J.get(s, "seats"), id), "hp",
              RS.u_min((ga.u(ga.at(J.get(s, "seats"), id), "hp") + 4 : U32), ga.u(ga.at(J.get(s, "seats"), id), "max_hp")))),
          "potion"), i)

# A heart container: the finder takes it as a granted item, the stage moves on.
def lanternward.stage.pickups.container(+s: J.Val, +id: U32) -> J.Val:
  ga.su(
    lanternward.kit.put_k(
      lanternward.ctx.Session.grant(
        ga.sv(ga.sv(s, "$k", ga.at(J.get(s, "seats"), id)), "seats", J.del(J.get(s, "seats"), U32.show(id))),
        id, J.JS{"Container"}),
      id),
    "stage", RS.u_max(ga.u(s, "stage"), 3))

# A taken or aged-out drop is swap_removed (the last drop takes its place);
# else the next drop.
def lanternward.stage.pickups.drop.end(+gone: Bool, +s: J.Val, +i: U32) -> J.Val:
  match gone:
    case True{}:
      lanternward.stage.pickups.drops(lanternward.stage.pickups.swap_remove(s, i), i)
    case False{}:
      lanternward.stage.pickups.drops(s, (i + 1 : U32))

def lanternward.stage.pickups.swap_remove(+s: J.Val, +i: U32) -> J.Val:
  ga.sv(s, "drops",
    J.truncate(J.set_nth(J.get(s, "drops"), i, J.nth(J.get(s, "drops"), (lanternward.len(s, "drops") - 1 : U32))), (lanternward.len(s, "drops") - 1 : U32)))

# ---- the fallen ----

# The fallen: a friend holding E gets them up; a long wait brings them to the
# next brazier on their own.
def lanternward.stage.Session.revive_pass(+s: J.Val) -> J.Val:
  lanternward.stage.revive.each(ga.ids(J.get(s, "seats")), s)

def lanternward.stage.revive.each(+ids: List<&2, U32>, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+d, +t}:
      lanternward.stage.revive.each(t, lanternward.stage.revive.one(lanternward.is(ga.at(J.get(s, "seats"), d), "act", "Down"), s, d))

def lanternward.stage.revive.one(down: Bool, +s: J.Val, +d: U32) -> J.Val:
  match down:
    case False{}:
      s
    case True{}:
      lanternward.stage.revive.helped(
        lanternward.stage.revive.helpers(ga.ids(J.get(s, "seats")), J.get(s, "seats"), d, ga.u(ga.at(J.get(s, "seats"), d), "x"), ga.u(ga.at(J.get(s, "seats"), d), "y")),
        s, d)

# Other knights standing, free, holding E beside the fallen.
def lanternward.stage.revive.helpers(+ids: List<&2, U32>, +seats: J.Val, +d: U32, +x: U32, +y: U32) -> U32:
  match ids:
    case Nil{}:
      0
    case Con{+id, +t}:
      (Bool.to_u32(
        Bool.and(
          Bool.and(U32.is_ne(id, d), lanternward.kit.Seat.up(ga.at(seats, id))),
          Bool.and(
            Bool.and(U32.is_ne(U32.and(ga.u(ga.at(seats, id), "held"), 8), 0), lanternward.is(ga.at(seats, id), "act", "Free")),
            RS.i_lt(lanternward.world.dist((ga.u(ga.at(seats, id), "x") - x : U32), (ga.u(ga.at(seats, id), "y") - y : U32)), 6144))))
        + lanternward.stage.revive.helpers(t, seats, d, x, y) : U32)

def lanternward.stage.revive.helped(+helpers: U32, +s: J.Val, +d: U32) -> J.Val:
  lanternward.stage.revive.up(
    Bool.pick(U32, U32.is_gt(helpers, 0),
      (ga.u(ga.at(J.get(s, "seats"), d), "rev") + RS.u_min(helpers, 2) : U32),
      RS.u_sat_sub(ga.u(ga.at(J.get(s, "seats"), d), "rev"), 2)),
    s, d)

def lanternward.stage.revive.up(+rev: U32, +s: J.Val, +d: U32) -> J.Val:
  lanternward.stage.revive.done(
    U32.is_ge(ga.u(ga.at(J.get(s, "seats"), d), "t"), 300),
    rev, lanternward.blow.put_seat(s, d, ga.su(ga.at(J.get(s, "seats"), d), "rev", rev)), d)

def lanternward.stage.revive.done(+auto: Bool, +rev: U32, +s: J.Val, +d: U32) -> J.Val:
  lanternward.stage.revive.stand(Bool.or(U32.is_ge(rev, 45), auto), auto, s, d)

def lanternward.stage.revive.stand(go: Bool, +auto: Bool, +s: J.Val, +d: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.stage.revive.stand.go(
        auto,
        lanternward.blow.put_seat(s, d,
          ga.su(ga.su(ga.su(ga.su(ga.sv(ga.at(J.get(s, "seats"), d), "act", J.JS{"Free"}), "t", 0), "rev", 0), "inv", 60),
            "hp", RS.u_max(RS.u_min(Bool.pick(U32, auto, 6, 12), ga.u(ga.at(J.get(s, "seats"), d), "max_hp")), 1))),
        d)

def lanternward.stage.revive.stand.go(+auto: Bool, +s: J.Val, +d: U32) -> J.Val:
  match auto:
    case True{}:
      lanternward.stage.revive.carry(lanternward.stage.Session.start_of(s, d), s, d)
    case False{}:
      lanternward.sound(
        lanternward.props.Session.fx(s, "Light", ga.u(ga.at(J.get(s, "seats"), d), "x"), ga.u(ga.at(J.get(s, "seats"), d), "y"), 0),
        "potion")

# Carried to the room's resting brazier.
def lanternward.stage.revive.carry(+start: J.Val, +s: J.Val, +d: U32) -> J.Val:
  lanternward.sound(
    lanternward.props.Session.fx(
      lanternward.blow.put_seat(s, d,
        ga.sv(ga.si(ga.si(ga.at(J.get(s, "seats"), d), "x", ga.nu(start, 0)), "y", ga.nu(start, 1)), "safe", start)),
      "Light", ga.nu(start, 0), ga.nu(start, 1), 0),
    "potion")

# ---- the wipe and the retry ----

def lanternward.stage.all_down.go(+ids: List<&2, U32>, +seats: J.Val) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+id, +t}:
      Bool.and(Bool.not(lanternward.kit.Seat.up(ga.at(seats, id))), lanternward.stage.all_down.go(t, seats))

# Every knight down: a breath, then the party is back at the room's start.
def lanternward.stage.Session.tick_wipe(+s: J.Val) -> J.Val:
  lanternward.stage.tick_wipe.go(
    Bool.and(U32.is_gt(lanternward.seat_count(s), 0), lanternward.stage.all_down.go(ga.ids(J.get(s, "seats")), J.get(s, "seats"))), s)

def lanternward.stage.tick_wipe.go(all_down: Bool, +s: J.Val) -> J.Val:
  match all_down:
    case False{}:
      ga.su(s, "wipe", 0)
    case True{}:
      lanternward.stage.tick_wipe.tick(
        lanternward.sound_if(U32.is_eq((ga.u(s, "wipe") + 1 : U32), 1), ga.su(s, "wipe", (ga.u(s, "wipe") + 1 : U32)), "boss_fall"))

def lanternward.stage.tick_wipe.tick(+s: J.Val) -> J.Val:
  lanternward.stage.tick_wipe.retry(U32.is_ge(ga.u(s, "wipe"), 24), s)

def lanternward.stage.tick_wipe.retry(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.stage.Session.retry(s)
    case False{}:
      s

# Back to the room's start: hearts full, foes as the encounter opened, every
# solved thing still solved.
def lanternward.stage.Session.retry(+s: J.Val) -> J.Val:
  lanternward.stage.retry.room(
    lanternward.stage.retry.key(
      lanternward.stage.retry.seats(
        ga.ids(J.get(s, "seats")),
        ga.sv(ga.sv(ga.sv(ga.su(ga.su(s, "wipe", 0), "tries", RS.u_min((ga.u(s, "tries") + 1 : U32), 4294967295)), "foes", J.JA{Nil{}}), "bolts", J.JA{Nil{}}), "fuses", J.JA{Nil{}}))))

def lanternward.stage.retry.seats(+ids: List<&2, U32>, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.stage.retry.seats(t, lanternward.stage.retry.seat(lanternward.stage.Session.start_of(s, id), s, id))

def lanternward.stage.retry.seat(+at: J.Val, +s: J.Val, +id: U32) -> J.Val:
  +k = ga.at(J.get(s, "seats"), id)
  +k1 = ga.si(ga.si(ga.sv(ga.sv(k, "safe", at), "cam", at), "x", ga.nu(at, 0)), "y", ga.nu(at, 1))
  +k2 = ga.su(ga.sv(ga.sv(ga.si(ga.su(ga.sv(ga.su(k1, "hp", ga.u(k, "max_hp")), "act", J.JS{"Free"}), "t", 0), "z", 0), "lock", J.JNull{}), "carry", J.JS{"Nothing"}), "shield", 0)
  +k3 = ga.su(ga.su(ga.su(ga.su(ga.su(ga.sv(k2, "push", ga.pair_i(0, 0)), "chain", 0), "step", 0), "charge", 0), "rev", 0), "pusht", 0)
  lanternward.blow.put_seat(s, id,
    ga.su(ga.su(k3, "inv", 60), "flame", Bool.pick(U32, ga.b(ga.v(s, "inv"), "lantern"), lanternward.R(s, "flame"), 0)))

# Carried keys are put down at the start.
def lanternward.stage.retry.key(+s: J.Val) -> J.Val:
  lanternward.stage.retry.key.go(lanternward.tag_is(ga.v(ga.v(s, "inv"), "boss"), "Carried"), s)

def lanternward.stage.retry.key.go(carried: Bool, +s: J.Val) -> J.Val:
  match carried:
    case False{}:
      s
    case True{}:
      lanternward.stage.retry.key.at(ga.nu(lanternward.payload(ga.v(ga.v(s, "inv"), "boss")), 0), s)

def lanternward.stage.retry.key.at(+id: U32, +s: J.Val) -> J.Val:
  lanternward.stage.retry.key.set(
    Bool.pick(J.Val, ga.has_key(J.get(s, "seats"), id),
      ga.pair_i(ga.u(ga.at(J.get(s, "seats"), id), "x"), ga.u(ga.at(J.get(s, "seats"), id), "y")), ga.pair_i(0, 0)),
    id, s)

def lanternward.stage.retry.key.set(+at: J.Val, +id: U32, +s: J.Val) -> J.Val:
  ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss", lanternward.variant("Carried", J.JA{[J.JU{id}, J.nth(at, 0), J.nth(at, 1)]})))

def lanternward.stage.retry.room(+s: J.Val) -> J.Val:
  lanternward.sound(
    ga.su(
      lanternward.stage.retry.by(ga.u(s, "room"), ga.u(s, "stage"), s),
      "flash", RS.u_max(ga.u(s, "flash"), 8)),
    "play_card")

def lanternward.stage.retry.by(+room: U32, +stage: U32, +s: J.Val) -> J.Val:
  Bool.pick(J.Val, Bool.or(Bool.or(Bool.and(U32.is_eq(room, 0), U32.is_eq(stage, 0)), Bool.and(U32.is_eq(room, 3), U32.is_eq(stage, 0))), U32.is_eq(room, 2)),
    lanternward.stage.Session.provision_room(s, room),
    Bool.pick(J.Val, Bool.and(U32.is_eq(room, 1), U32.is_eq(stage, 2)),
      lanternward.stage.Session.spawn_wave(s),
      Bool.pick(J.Val, U32.is_eq(room, 4), lanternward.boss.Session.reset_boss_phase(s), s)))

# ---- ageing ----

def lanternward.stage.fx_life(+kind: String) -> U32:
  Bool.pick(U32, lanternward.NAME_IN(kind, ["Puff", "Light"]), 16,
    Bool.pick(U32, lanternward.NAME_IN(kind, ["Spark", "Tick"]), 8,
      Bool.pick(U32, lanternward.NAME_IN(kind, ["Ring", "Dust"]), 10,
        Bool.pick(U32, String.eq(kind, "Parry"), 12,
          Bool.pick(U32, String.eq(kind, "Solved"), 24,
            Bool.pick(U32, String.eq(kind, "Blast"), 14, 20))))))

def lanternward.stage.age.fx(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      lanternward.stage.age.fx.keep(RS.u_min((ga.u(f, "t") + 1 : U32), 255), f, t)

def lanternward.stage.age.fx.keep(+age: U32, +f: J.Val, +t: List<&2, J.Val>) -> List<&2, J.Val>:
  Bool.pick(List<&2, J.Val>, U32.is_lt(age, lanternward.stage.fx_life(ga.s(f, "kind"))),
    Con{ga.su(f, "t", age), lanternward.stage.age.fx(t)}, lanternward.stage.age.fx(t))

# The tick's leftovers age out.
def lanternward.stage.Session.age(+s: J.Val) -> J.Val:
  lanternward.stage.age.banner(
    lanternward.stage.age.cue(
      ga.su(ga.su(ga.sv(s, "fx", J.JA{lanternward.stage.age.fx(ga.items(s, "fx"))}), "shake", RS.u_sat_sub(ga.u(s, "shake"), 1)), "flash", RS.u_sat_sub(ga.u(s, "flash"), 1))))

def lanternward.stage.age.cue(+s: J.Val) -> J.Val:
  lanternward.stage.age.cue.go(J.is_null(J.get(s, "cue")), s)

def lanternward.stage.age.cue.go(none: Bool, +s: J.Val) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.stage.age.cue.tick(U32.is_gt(ga.nu(J.get(s, "cue"), 2), 1), s)

def lanternward.stage.age.cue.tick(live: Bool, +s: J.Val) -> J.Val:
  match live:
    case False{}:
      ga.sv(s, "cue", J.JNull{})
    case True{}:
      ga.sv(s, "cue", J.JA{[J.nth(J.get(s, "cue"), 0), J.nth(J.get(s, "cue"), 1), J.JU{(ga.nu(J.get(s, "cue"), 2) - 1 : U32)}]})

def lanternward.stage.age.banner(+s: J.Val) -> J.Val:
  lanternward.stage.age.banner.go(J.is_null(J.get(s, "banner")), s)

def lanternward.stage.age.banner.go(none: Bool, +s: J.Val) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.stage.age.banner.out(
        U32.is_eq(RS.u_sat_sub(ga.u(J.get(s, "banner"), "t"), 1), 0),
        ga.sv(s, "banner", ga.su(J.get(s, "banner"), "t", RS.u_sat_sub(ga.u(J.get(s, "banner"), "t"), 1))))

def lanternward.stage.age.banner.out(gone: Bool, +s: J.Val) -> J.Val:
  match gone:
    case True{}:
      ga.sv(s, "banner", J.JNull{})
    case False{}:
      s
