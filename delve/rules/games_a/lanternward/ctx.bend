# crates/delve/src/rules/lanternward/ctx.rs — the one context button, the
# party's keys, the banner, items granted.
#
# Ctx values are serde's: "None", {"Open": i}, {"Read": i}, {"Lift": i},
# "Throw", "Flame", {"Light": i}, {"Unlock": i}, {"Revive": id}, "Relight",
# {"Talk": i}, {"Pull": i}. Hold: "Unfound", {"Carried": [seat, x, y]},
# {"Lying": [x, y]}, "Spent".

def lanternward.ctx.Party.new() -> J.Val:
  ga.su(ga.sv(ga.su(ga.sb(ga.sb(J.obj(), "map", False{}), "lantern", False{}), "small_keys", 0), "boss", J.JS{"Unfound"}), "containers", 0)

# Whether the boss key is carried by this seat.
def lanternward.ctx.Session.carrying_boss_key(+s: J.Val, +k: J.Val) -> Bool:
  +b = ga.v(ga.v(s, "inv"), "boss")
  Bool.and(lanternward.tag_is(b, "Carried"), U32.is_eq(ga.nu(lanternward.payload(b), 0), ga.u(k, "id")))

def lanternward.ctx.Session.beacon_ready(+s: J.Val) -> Bool:
  Bool.and(U32.is_eq(ga.u(s, "room"), 4), U32.is_ge(ga.u(s, "stage"), 3))

# The first revive reach over the fallen: seats in id order.
def lanternward.ctx.Session.context.revive(+ids: List<&2, U32>, +s: J.Val, +k: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      J.JNull{}
    case Con{+d, +t}:
      lanternward.ctx.Session.context.revive.at(
        Bool.and(
          lanternward.is(ga.at(ga.v(s, "seats"), d), "act", "Down"),
          RS.i_lt(lanternward.world.dist(
            (ga.u(ga.at(ga.v(s, "seats"), d), "x") - ga.u(k, "x") : U32),
            (ga.u(ga.at(ga.v(s, "seats"), d), "y") - ga.u(k, "y") : U32)), 6144)),
        d, t, s, k)

def lanternward.ctx.Session.context.revive.at(hit: Bool, +d: U32, +t: List<&2, U32>, +s: J.Val, +k: J.Val) -> J.Val:
  match hit:
    case True{}:
      lanternward.variant("Revive", J.JU{U32.and(d, 255)})
    case False{}:
      lanternward.ctx.Session.context.revive(t, s, k)

# The Ctx a prop would give, or null for none (the `continue`s).
def lanternward.ctx.Session.context.kind(+s: J.Val, +k: J.Val, +p: J.Val, +i: U32) -> J.Val:
  +kd = ga.s(p, "kind")
  +on0 = U32.is_eq(ga.u(p, "on"), 0)
  Bool.pick(J.Val, Bool.and(String.eq(kd, "Chest"), on0), lanternward.variant("Open", J.JU{U32.and(i, 255)}),
    Bool.pick(J.Val, String.eq(kd, "Sign"), lanternward.variant("Read", J.JU{U32.and(i, 255)}),
      Bool.pick(J.Val, String.eq(kd, "Folk"), lanternward.variant("Talk", J.JU{U32.and(i, 255)}),
        Bool.pick(J.Val, Bool.and(String.eq(kd, "Lever"), on0), lanternward.variant("Pull", J.JU{U32.and(i, 255)}),
          Bool.pick(J.Val, Bool.and(String.eq(kd, "Pot"), Bool.and(on0, U32.is_eq(ga.u(p, "id"), 0))), lanternward.variant("Lift", J.JU{U32.and(i, 255)}),
            Bool.pick(J.Val, String.eq(kd, "Ember"), J.JS{"Flame"},
              Bool.pick(J.Val, Bool.and(String.eq(kd, "Brazier"), Bool.and(Bool.not(lanternward.props.Prop.lit(p)), U32.is_gt(ga.u(k, "taper"), 0))), lanternward.variant("Light", J.JU{U32.and(i, 255)}),
                Bool.pick(J.Val, Bool.and(String.eq(kd, "Lock"), Bool.and(on0, U32.is_gt(ga.u(ga.v(s, "inv"), "small_keys"), 0))), lanternward.variant("Unlock", J.JU{U32.and(i, 255)}),
                  Bool.pick(J.Val, Bool.and(String.eq(kd, "Boss"), Bool.and(on0, lanternward.ctx.Session.carrying_boss_key(s, k))), lanternward.variant("Unlock", J.JU{U32.and(i, 255)}),
                    Bool.pick(J.Val, Bool.and(String.eq(kd, "Beacon"), Bool.and(on0, lanternward.ctx.Session.beacon_ready(s))), J.JS{"Relight"},
                      J.JNull{}))))))))))

# Walks the props keeping the nearest in reach and in front. `bc` is null for
# no candidate yet, else the best Ctx at distance `bd`.
def lanternward.ctx.Session.context.props(+s: J.Val, +k: J.Val, +ps: List<&2, J.Val>, +i: U32, +bd: U32, +bc: J.Val) -> J.Val:
  match ps:
    case Nil{}:
      bc
    case Con{+p, +t}:
      lanternward.ctx.Session.context.cand(lanternward.ctx.Session.context.kind(s, k, p, i), s, k, p, t, i, bd, bc)

def lanternward.ctx.Session.context.cand(+c: J.Val, +s: J.Val, +k: J.Val, +p: J.Val, +t: List<&2, J.Val>, +i: U32, +bd: U32, +bc: J.Val) -> J.Val:
  match c:
    case J.JNull{}:
      lanternward.ctx.Session.context.props(s, k, t, (i + 1 : U32), bd, bc)
    case _:
      lanternward.ctx.Session.context.dist(
        (ga.u(p, "x") - ga.u(k, "x") : U32), (ga.u(p, "y") - ga.u(k, "y") : U32), c, s, k, t, i, bd, bc)

def lanternward.ctx.Session.context.dist(+dx: U32, +dy: U32, +c: J.Val, +s: J.Val, +k: J.Val, +t: List<&2, J.Val>, +i: U32, +bd: U32, +bc: J.Val) -> J.Val:
  lanternward.ctx.Session.context.take(
    lanternward.world.dist(dx, dy), dx, dy, c, s, k, t, i, bd, bc)

def lanternward.ctx.Session.context.take(+d: U32, +dx: U32, +dy: U32, +c: J.Val, +s: J.Val, +k: J.Val, +t: List<&2, J.Val>, +i: U32, +bd: U32, +bc: J.Val) -> J.Val:
  +reach = RS.i_gt(d, 5120)
  +face = ga.v(k, "face")
  +behind = Bool.and(
    RS.i_gt(d, 3072),
    RS.i_lt(lanternward.ctx.Session.context.facing(face, dx, dy), 88))
  +skip = Bool.or(reach, behind)
  +better = Bool.or(J.is_null(bc), RS.i_lt(d, bd))
  lanternward.ctx.Session.context.props(s, k, t, (i + 1 : U32),
    Bool.pick(U32, Bool.and(Bool.not(skip), better), d, bd),
    Bool.pick(J.Val, Bool.and(Bool.not(skip), better), c, bc))

def lanternward.ctx.Session.context.facing(+face: J.Val, +dx: U32, +dy: U32) -> U32:
  lanternward.world.dot_unit(ga.nu(face, 0), ga.nu(face, 1), dx, dy)

# The nearest thing in reach and in front of the knight that E would act on.
def lanternward.ctx.Session.context(+s: J.Val, +k: J.Val) -> J.Val:
  lanternward.ctx.Session.context.carry(lanternward.is(k, "carry", "Pot"), s, k)

def lanternward.ctx.Session.context.carry(pot: Bool, +s: J.Val, +k: J.Val) -> J.Val:
  match pot:
    case True{}:
      J.JS{"Throw"}
    case False{}:
      lanternward.ctx.Session.context.fin(
        lanternward.ctx.Session.context.revive(ga.ids(J.get(s, "seats")), s, k), s, k)

def lanternward.ctx.Session.context.fin(+rv: J.Val, +s: J.Val, +k: J.Val) -> J.Val:
  match rv:
    case J.JNull{}:
      lanternward.ctx.Session.context.none(
        lanternward.ctx.Session.context.props(s, k, ga.items(s, "props"), 0, 0, J.JNull{}))
    case _:
      rv

def lanternward.ctx.Session.context.none(+c: J.Val) -> J.Val:
  match c:
    case J.JNull{}:
      J.JS{"None"}
    case _:
      c

# The first prop of this kind (and id, unless `id` is 4294967295), or null.
def lanternward.find_prop.go(+ps: List<&2, J.Val>, +kind: String, +id: U32) -> J.Val:
  match ps:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      lanternward.find_prop.hit(
        Bool.and(String.eq(ga.s(h, "kind"), kind), Bool.or(U32.is_eq(id, 4294967295), U32.is_eq(ga.u(h, "id"), id))),
        h, t, kind, id)

def lanternward.find_prop.hit(hit: Bool, +h: J.Val, +t: List<&2, J.Val>, +kind: String, +id: U32) -> J.Val:
  match hit:
    case True{}:
      h
    case False{}:
      lanternward.find_prop.go(t, kind, id)

def lanternward.find_prop(+s: J.Val, +kind: String, +id: U32) -> J.Val:
  lanternward.find_prop.go(ga.items(s, "props"), kind, id)

# Ticks a brazier lit from an ember burns: shorter the more hands.
def lanternward.ctx.Session.burn_ticks(+s: J.Val) -> U32:
  RS.u_max(RS.u_sat_sub(600, (60 * (RS.u_max(lanternward.seat_count(s), 1) - 1 : U32) : U32)), 300)

def lanternward.ctx.Session.light_crystal(+s: J.Val, +i: U32) -> J.Val:
  lanternward.ctx.Session.light_crystal.go(U32.is_eq(lanternward.eu(s, "props", i, "on"), 1), s, i)

def lanternward.ctx.Session.light_crystal.go(done: Bool, +s: J.Val, +i: U32) -> J.Val:
  match done:
    case True{}:
      s
    case False{}:
      lanternward.props.Session.solved(
        lanternward.es(s, "props", i, "on", 1),
        lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"))

# Start a block sliding one tile the way it was pushed, if the place is open.
def lanternward.ctx.against(+room: U32, +dtx: U32, +dty: U32) -> Bool:
  Bool.or(
    Bool.or(lanternward.ctx.against.at(lanternward.world.Plan.at(room, (dtx + 1 : U32), dty)),
      lanternward.ctx.against.at(lanternward.world.Plan.at(room, (dtx - 1 : U32), dty))),
    Bool.or(lanternward.ctx.against.at(lanternward.world.Plan.at(room, dtx, (dty + 1 : U32))),
      lanternward.ctx.against.at(lanternward.world.Plan.at(room, dtx, (dty - 1 : U32)))))

def lanternward.ctx.against.at(+c: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(c, 35), U32.is_eq(c, 86)), Bool.or(U32.is_eq(c, 61), U32.is_eq(c, 81)))

# Whether a prop other than block `b` and the plates stops a body at (x, y).
def lanternward.ctx.others_block(+ps: List<&2, J.Val>, +j: U32, +b: U32, +x: U32, +y: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+q, +t}:
      Bool.or(
        Bool.and(U32.is_ne(j, b),
          Bool.and(Bool.not(String.eq(ga.s(q, "kind"), "Plate")), lanternward.props.Prop.blocks(q, x, y))),
        lanternward.ctx.others_block(t, (j + 1 : U32), b, x, y))

def lanternward.ctx.Session.start_slide(+s: J.Val, +b: U32) -> J.Val:
  +on = lanternward.eu(s, "props", b, "on")
  +dx = Bool.pick(U32, U32.is_eq(on, 1), 1, Bool.pick(U32, U32.is_eq(on, 2), 4294967295, 0))
  +dy = Bool.pick(U32, U32.is_eq(on, 3), 1, Bool.pick(U32, Bool.or(U32.is_eq(on, 1), U32.is_eq(on, 2)), 0, 4294967295))
  lanternward.ctx.Session.start_slide.go(
    s, b, (lanternward.eu(s, "props", b, "x") + dx * 4096 : U32), (lanternward.eu(s, "props", b, "y") + dy * 4096 : U32))

def lanternward.ctx.Session.start_slide.go(+s: J.Val, +b: U32, +nx: U32, +ny: U32) -> J.Val:
  +room = ga.u(s, "room")
  +dtx = lanternward.world.tile_of(nx)
  +dty = lanternward.world.tile_of(ny)
  +c = lanternward.world.Plan.at(room, dtx, dty)
  +open = Bool.and(
    Bool.and(Bool.not(lanternward.ctx.against(room, dtx, dty)), Bool.not(lanternward.world.wall(c))),
    Bool.and(Bool.and(U32.is_ne(c, 61), U32.is_ne(c, 81)),
      Bool.not(lanternward.ctx.others_block(ga.items(s, "props"), 0, b, nx, ny))))
  lanternward.ctx.Session.start_slide.set(open, s, b)

def lanternward.ctx.Session.start_slide.set(open: Bool, +s: J.Val, +b: U32) -> J.Val:
  match open:
    case True{}:
      lanternward.es(s, "props", b, "t", 16)
    case False{}:
      lanternward.es(lanternward.es(s, "props", b, "on", 0), "props", b, "t", 0)

# ---- items ----

# An item comes to the party; its finder holds it up. (`$k` is the finder.)
def lanternward.ctx.Session.grant.seats(+ids: List<&2, U32>, +s: J.Val, +item: String, +n: U32) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.ctx.Session.grant.seats(t, lanternward.ctx.Session.grant.one(s, id, item, n), item, n)

# One other seat's share of a lantern (flame) or a container (hearts).
def lanternward.ctx.Session.grant.one(+s: J.Val, +id: U32, +item: String, +n: U32) -> J.Val:
  +o = ga.at(J.get(s, "seats"), id)
  ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id,
    Bool.pick(J.Val, String.eq(item, "Lantern"), ga.su(o, "flame", lanternward.R(s, "flame")),
      Bool.pick(J.Val, String.eq(item, "Container"),
        ga.su(ga.su(o, "max_hp", (ga.u(o, "max_hp") + 4 : U32)), "hp", (ga.u(o, "max_hp") + 4 : U32)),
        o))))

def lanternward.ctx.Session.grant.effect.go(+code: U32, +s: J.Val, +id: U32, +item: String, +n: U32) -> J.Val:
  match code:
    case 1:
      ga.sv(s, "inv", ga.sb(ga.v(s, "inv"), "map", True{}))
    case 2:
      lanternward.ctx.Session.grant.seats(ga.ids(J.get(s, "seats")),
        lanternward.ks(ga.sv(s, "inv", ga.sb(ga.v(s, "inv"), "lantern", True{})), "flame", lanternward.R(s, "flame")),
        item, n)
    case 3:
      ga.sv(s, "inv", ga.su(ga.v(s, "inv"), "small_keys", RS.u_min((ga.u(ga.v(s, "inv"), "small_keys") + 1 : U32), 255)))
    case 4:
      ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss",
        lanternward.variant("Carried", J.JA{[J.JU{U32.and(id, 255)}, J.of_i32(lanternward.ku(s, "x")), J.of_i32(lanternward.ku(s, "y"))]})))
    case 5:
      lanternward.ctx.Session.grant.seats(ga.ids(J.get(s, "seats")),
        lanternward.ks(
          lanternward.ks(
            ga.sv(s, "inv", ga.su(ga.v(s, "inv"), "containers", RS.u_min((ga.u(ga.v(s, "inv"), "containers") + 1 : U32), 255))),
            "max_hp", (lanternward.ku(s, "max_hp") + 4 : U32)),
          "hp", (lanternward.ku(s, "max_hp") + 4 : U32)),
        item, n)
    case 6:
      lanternward.ks(s, "gems", (lanternward.ku(s, "gems") + n : U32))
    case _:
      lanternward.ks(s, "bombs", (lanternward.ku(s, "bombs") + n : U32))

def lanternward.ctx.Session.grant.code(+item: String) -> U32:
  Bool.pick(U32, String.eq(item, "Map"), 1,
    Bool.pick(U32, String.eq(item, "Lantern"), 2,
      Bool.pick(U32, String.eq(item, "SmallKey"), 3,
        Bool.pick(U32, String.eq(item, "BossKey"), 4,
          Bool.pick(U32, String.eq(item, "Container"), 5,
            Bool.pick(U32, String.eq(item, "Gems"), 6, 7))))))

def lanternward.ctx.Session.grant.fin(+s: J.Val, +id: U32, +item: J.Val, +tag: String) -> J.Val:
  lanternward.sound(
    ga.sv(
      lanternward.ks(
        lanternward.ks(
          lanternward.ks(
            lanternward.ks(
              lanternward.ksv(lanternward.ksv(lanternward.ksn(lanternward.ks(lanternward.ks(s, "t", 0), "lamp", 0), "act", "Item"), "item", item), "lock", J.JNull{}),
              "shield", 0),
            "chain", 0),
          "step", 0),
        "charge", 0),
      "banner",
      ga.su(ga.su(ga.sv(J.obj(), "item", item), "seat", U32.and(id, 255)), "t", 60)),
    Bool.pick(String, lanternward.NAME_IN(tag, ["Gems", "Bombs"]), "spoil_pickup", "wheel_land"))

def lanternward.ctx.Session.grant(+s: J.Val, +id: U32, +item: J.Val) -> J.Val:
  lanternward.ctx.Session.grant.fin(
    lanternward.ctx.Session.grant.effect.go(
      lanternward.ctx.Session.grant.code(lanternward.tag(item)), s, id, lanternward.tag(item), J.u32(lanternward.payload(item))),
    id, item, lanternward.tag(item))

def lanternward.ctx.Session.open_chest(+s: J.Val, +id: U32, +i: U32) -> J.Val:
  lanternward.ctx.Session.open_chest.go(
    lanternward.sound(lanternward.es(s, "props", i, "on", 1), "chest_open"),
    id, i, lanternward.props.chest_item(ga.u(s, "room"), lanternward.eu(s, "props", i, "id")))

def lanternward.ctx.Session.open_chest.go(+s: J.Val, +id: U32, +i: U32, +item: J.Val) -> J.Val:
  lanternward.ctx.Session.grant(
    lanternward.props.Session.fx(s, "Light", lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 0),
    id, item)

# The Ctx variant's code: 0 None, 1 Open, 2 Read, 3 Lift, 4 Throw, 5 Flame,
# 6 Light, 7 Unlock, 8 Revive, 9 Relight, 10 Talk, 11 Pull.
def lanternward.ctx.tag_code(+t: String) -> U32:
  Bool.pick(U32, String.eq(t, "Open"), 1,
    Bool.pick(U32, String.eq(t, "Read"), 2,
      Bool.pick(U32, String.eq(t, "Lift"), 3,
        Bool.pick(U32, String.eq(t, "Throw"), 4,
          Bool.pick(U32, String.eq(t, "Flame"), 5,
            Bool.pick(U32, String.eq(t, "Light"), 6,
              Bool.pick(U32, String.eq(t, "Unlock"), 7,
                Bool.pick(U32, String.eq(t, "Revive"), 8,
                  Bool.pick(U32, String.eq(t, "Relight"), 9,
                    Bool.pick(U32, String.eq(t, "Talk"), 10,
                      Bool.pick(U32, String.eq(t, "Pull"), 11, 0)))))))))))

def lanternward.ctx.Session.use_context.pull(has: Bool, +s: J.Val, +g: J.Val) -> J.Val:
  match has:
    case False{}:
      s
    case True{}:
      lanternward.sound(
        lanternward.props.Session.solved(
          lanternward.props.Session.set_gate(s, 2, True{}), ga.u(g, "x"), ga.u(g, "y")),
        "door_open")

def lanternward.ctx.Session.use_context.throw(ok: Bool, +s: J.Val, +id: U32) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.push(s, "bolts",
        ga.su(
          ga.su(
            ga.si(
              ga.si(
                ga.si(
                  ga.si(ga.sv(J.obj(), "kind", J.JS{"Pot"}), "x",
                    (lanternward.ku(s, "x") + lanternward.kface(s, 0) * 8 : U32)),
                  "y", (lanternward.ku(s, "y") + lanternward.kface(s, 1) * 8 : U32)),
                "vx", RS.i_div((lanternward.kface(s, 0) * 1400 : U32), 256)),
              "vy", RS.i_div((lanternward.kface(s, 1) * 1400 : U32), 256)),
            "ttl", 10),
          "owner", U32.and(id, 255)))

def lanternward.ctx.Session.use_context.unlock(lock: Bool, +s: J.Val, +i: U32) -> J.Val:
  match lock:
    case True{}:
      ga.sv(s, "inv", ga.su(ga.v(s, "inv"), "small_keys", RS.u_sat_sub(ga.u(ga.v(s, "inv"), "small_keys"), 1)))
    case False{}:
      ga.sv(s, "inv", ga.sv(ga.v(s, "inv"), "boss", J.JS{"Spent"}))

def lanternward.ctx.Session.use_context.go(+code: U32, +i: U32, +s: J.Val, +id: U32) -> J.Val:
  match code:
    case 0:
      ga.sb(s, "$ret", False{})
    case 1:
      ga.sb(lanternward.ctx.Session.open_chest(s, id, i), "$ret", True{})
    case 2:
      ga.sb(
        lanternward.sound(
          lanternward.ksv(s, "read", ga.pair(J.JU{lanternward.eu(s, "props", i, "id")}, J.JU{120})), "card_pickup"),
        "$ret", True{})
    case 10:
      ga.sb(
        lanternward.sound(
          lanternward.ksv(s, "read", ga.pair(J.JU{(100 + lanternward.eu(s, "props", i, "id") : U32)}, J.JU{120})), "card_pickup"),
        "$ret", True{})
    case 11:
      ga.sb(
        lanternward.ctx.Session.use_context.pull(
          Bool.not(J.is_null(lanternward.find_prop(s, "Gate", 2))),
          lanternward.sound(lanternward.es(s, "props", i, "on", 1), "wall_up"),
          lanternward.find_prop(s, "Gate", 2)),
        "$ret", True{})
    case 3:
      ga.sb(lanternward.sound(lanternward.ksn(lanternward.es(s, "props", i, "on", 1), "carry", "Pot"), "bomb_toss"), "$ret", True{})
    case 4:
      ga.sb(
        lanternward.sound(
          lanternward.ctx.Session.use_context.throw(U32.is_lt(lanternward.len(s, "bolts"), 40), lanternward.ksn(s, "carry", "Nothing"), id),
          "bomb_toss"),
        "$ret", True{})
    case 5:
      ga.sb(
        lanternward.sound(
          lanternward.props.Session.fx(lanternward.ks(s, "taper", 420), "Light", lanternward.ku(s, "x"), (lanternward.ku(s, "y") - 2048 : U32), 0),
          "spell_fire"),
        "$ret", True{})
    case 6:
      ga.sb(
        lanternward.sound(
          lanternward.props.Session.fx(
            lanternward.es(s, "props", i, "t", lanternward.ctx.Session.burn_ticks(s)),
            "Light", lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 0),
          "spell_fire"),
        "$ret", True{})
    case 7:
      ga.sb(
        lanternward.sound(
          lanternward.props.Session.solved(
            lanternward.es(
              lanternward.ctx.Session.use_context.unlock(lanternward.eis(s, "props", i, "kind", "Lock"), s, i),
              "props", i, "on", 1),
            lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y")),
          "door_open"),
        "$ret", True{})
    case _:
      ga.sb(s, "$ret", True{})

# E pressed with a thing in reach. `$ret` is false when there is nothing to do
# (the caller rolls or hops). `$k` is the pressing seat.
def lanternward.ctx.Session.use_context(+s: J.Val, +id: U32) -> J.Val:
  +c = lanternward.ctx.Session.context(s, J.get(s, "$k"))
  lanternward.ctx.Session.use_context.go(
    lanternward.ctx.tag_code(lanternward.tag(c)), J.u32(lanternward.payload(c)), s, id)

# The index of the first prop of this kind, or null.
def lanternward.find_prop_index.go(+ps: List<&2, J.Val>, +kind: String, +i: U32) -> J.Val:
  match ps:
    case Nil{}:
      J.JNull{}
    case Con{+h, +t}:
      lanternward.find_prop_index.hit(String.eq(ga.s(h, "kind"), kind), t, kind, i)

def lanternward.find_prop_index.hit(hit: Bool, +t: List<&2, J.Val>, +kind: String, +i: U32) -> J.Val:
  match hit:
    case True{}:
      J.JU{i}
    case False{}:
      lanternward.find_prop_index.go(t, kind, (i + 1 : U32))

def lanternward.find_prop_index(+s: J.Val, +kind: String) -> J.Val:
  lanternward.find_prop_index.go(ga.items(s, "props"), kind, 0)
