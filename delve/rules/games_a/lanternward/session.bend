# crates/delve/src/rules/lanternward.rs — the session: new, step, finale,
# valid / problem / repair, the Rules, the purse.
#
# `lanternward.Session.step` returns the session with the tick's Events in
# `$ev` (and the working keys left in): `lanternward.Session.out` strips them
# to the plain session, `lanternward.Session.events` reads the Events.

# ---- constants ----

def lanternward.LIMIT() -> U32:
  108000

def lanternward.HEART() -> U32:
  4

def lanternward.START_HP() -> U32:
  20

def lanternward.SPAWN_INV() -> U32:
  60

def lanternward.REVIVE_HP() -> U32:
  12

def lanternward.KNIGHT_R() -> U32:
  1280

def lanternward.WALK() -> U32:
  440

def lanternward.PURSE_FIRST() -> U32:
  40

def lanternward.PURSE_AGAIN() -> U32:
  12

def lanternward.PURSE_GEMS() -> U32:
  20

def lanternward.BEGUN() -> U32:
  1

def lanternward.SLOT() -> U32:
  0

def lanternward.NAME() -> String:
  "Lanternward"

def lanternward.CAMPAIGN_MARK() -> String:
  "lanternward:campaign-begun"

# The five numbers as the rules read them: `Rules::default()`.
def lanternward.Rules.default() -> J.Val:
  ga.su(ga.su(ga.su(ga.si(J.obj(), "lock", 28672), "parry", 8), "combo", 14), "flame", 150)

def lanternward.Rules.default.full() -> J.Val:
  ga.su(lanternward.Rules.default(), "tell", 56)

# `Rules::of(tune)`: the Tune's numbers for this slot (tell never under the floor).
def lanternward.Rules.of(+tune: J.Val) -> J.Val:
  ga.su(
    ga.su(
      ga.su(
        ga.su(
          ga.si(J.obj(), "lock", (roundtable.number(tune, 0, 0) * 256 : U32)),
          "parry", roundtable.number(tune, 0, 1)),
        "combo", roundtable.number(tune, 0, 2)),
      "flame", roundtable.number(tune, 0, 3)),
    "tell", RS.u_max(roundtable.number(tune, 0, 4), 40))

# What one knight is paid at the Beacon.
def lanternward.purse(+first: Bool, +gems: U32) -> U32:
  (Bool.pick(U32, first, 40, 12) + RS.u_min(gems, 20) : U32)

# ---- new ----

# `Session::new(&players)`: every player not away or in stone sits down, then
# room 0 is entered.
def lanternward.Session.new(+players: J.Val) -> J.Val:
  lanternward.Session.new.sit(ga.ids(players), players, lanternward.Session.new.empty())

def lanternward.Session.new.empty() -> J.Val:
  lanternward.Session.new.fields(ga.sv(J.obj(), "seats", J.obj()))

def lanternward.Session.new.fields(+s: J.Val) -> J.Val:
  lanternward.Session.new.fields2(
    ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(s,
      "elapsed", 0), "room", 0), "stage", 0), "stage_t", 0), "shake", 0), "flash", 0),
      "props", J.JA{Nil{}}), "foes", J.JA{Nil{}}), "bolts", J.JA{Nil{}}), "drops", J.JA{Nil{}}), "fx", J.JA{Nil{}}), "fuses", J.JA{Nil{}}))

def lanternward.Session.new.fields2(+s: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(ga.sv(ga.su(ga.su(ga.su(ga.su(ga.sv(ga.sv(ga.sv(ga.sv(s,
    "boss", J.JNull{}), "inv", lanternward.ctx.Party.new()), "banner", J.JNull{}), "seed", W.to_j(W.W{1279937108, 1163152983})),
    "exiting", 0), "wipe", 0), "tries", 0), "next_foe", 0),
    "cue", J.JNull{}), "done", J.JNull{}), "paid", J.JU{0}), "kept", J.obj())

def lanternward.Session.new.sit(+ids: List<&2, U32>, +players: J.Val, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      lanternward.Session.bare(
        lanternward.stage.Session.enter_room(ga.sv(s, "$r", lanternward.Rules.default.full()), 0))
    case Con{+id, +t}:
      lanternward.Session.new.sit(t, players,
        lanternward.Session.new.one(
          Bool.and(Bool.not(ga.b(ga.at(players, id), "away")), Bool.not(ga.b(ga.at(players, id), "stone"))), s, id))

def lanternward.Session.new.one(here: Bool, +s: J.Val, +id: U32) -> J.Val:
  match here:
    case False{}:
      s
    case True{}:
      lanternward.Session.new.seat(lanternward.stage.Session.start_of(s, id), s, id)

def lanternward.Session.new.seat(+at: J.Val, +s: J.Val, +id: U32) -> J.Val:
  lanternward.blow.put_seat(s, id, lanternward.kit.Seat.new(id, 0, ga.nu(at, 0), ga.nu(at, 1)))

# Strip the working keys: the session as serde writes it.
def lanternward.Session.bare(+s: J.Val) -> J.Val:
  J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(J.del(s,
    "$ev"), "$r"), "$ret"), "$k"), "$f"), "$w"), "$strikes"), "$shots"), "$landed"), "$cut"), "$parried"), "$weights"), "$flares"), "$revealed"), "$b")

def lanternward.Session.events(+s: J.Val) -> J.Val:
  J.get(s, "$ev")

def lanternward.Session.out(+s: J.Val) -> J.Val:
  lanternward.Session.bare(s)

# ---- step ----

# `here(id)`: the hero is at the table (not away, not in stone).
def lanternward.Session.here(+heroes: J.Val, +id: U32) -> Bool:
  Bool.and(
    ga.has_key(heroes, id),
    Bool.and(Bool.not(ga.b(ga.at(heroes, id), "away")), Bool.not(ga.b(ga.at(heroes, id), "stone"))))

# A knight who left stands up and keeps their gems and bombs.
def lanternward.Session.step.stand(+ids: List<&2, U32>, +s: J.Val, +heroes: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.Session.step.stand(t,
        lanternward.Session.step.stand.one(lanternward.Session.here(heroes, id), s, id), heroes)

def lanternward.Session.step.stand.one(here: Bool, +s: J.Val, +id: U32) -> J.Val:
  match here:
    case True{}:
      s
    case False{}:
      ga.sv(
        ga.sv(s, "kept", ga.put_at(J.get(s, "kept"), id,
          ga.pair(J.get(ga.at(J.get(s, "seats"), id), "gems"), J.get(ga.at(J.get(s, "seats"), id), "bombs")))),
        "seats", J.del(J.get(s, "seats"), U32.show(id)))

# One who joins late sits down at the room's start and scales what is standing.
def lanternward.Session.step.sit(+ids: List<&2, U32>, +s: J.Val, +heroes: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.Session.step.sit(t,
        lanternward.Session.step.sit.one(
          Bool.and(lanternward.Session.here(heroes, id), Bool.not(ga.has_key(J.get(s, "seats"), id))), s, id), heroes)

def lanternward.Session.step.sit.one(join: Bool, +s: J.Val, +id: U32) -> J.Val:
  match join:
    case False{}:
      s
    case True{}:
      lanternward.Session.step.sit.seat(lanternward.stage.Session.start_of(s, id), s, id)

def lanternward.Session.step.sit.seat(+at: J.Val, +s: J.Val, +id: U32) -> J.Val:
  lanternward.Session.step.sit.kept(
    ga.at(J.get(s, "kept"), id), ga.has_key(J.get(s, "kept"), id),
    lanternward.kit.Seat.new(id, ga.u(s, "elapsed"), ga.nu(at, 0), ga.nu(at, 1)), s, id)

def lanternward.Session.step.sit.kept(+kept: J.Val, +has: Bool, +k: J.Val, +s: J.Val, +id: U32) -> J.Val:
  lanternward.Session.step.sit.hp(
    Bool.pick(J.Val, has, ga.su(ga.su(k, "gems", ga.nu(kept, 0)), "bombs", ga.nu(kept, 1)), k),
    Bool.pick(J.Val, has, ga.sv(s, "kept", J.del(J.get(s, "kept"), U32.show(id))), s), id)

def lanternward.Session.step.sit.hp(+k: J.Val, +s: J.Val, +id: U32) -> J.Val:
  +max = (20 + 4 * ga.u(ga.v(s, "inv"), "containers") : U32)
  lanternward.blow.put_seat(s, id,
    ga.su(
      Bool.pick(J.Val, ga.b(ga.v(s, "inv"), "lantern"), ga.su(ga.su(ga.su(k, "max_hp", max), "hp", max), "flame", lanternward.R(s, "flame")), ga.su(ga.su(k, "max_hp", max), "hp", max)),
      "max_hp", max))

# Each seated knight takes their input (a bad one is no input).
def lanternward.Session.step.seats(+ids: List<&2, U32>, +s: J.Val, +inputs: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      lanternward.Session.step.seats(t, lanternward.Session.step.seat(s, id, ga.at(inputs, id)), inputs)

def lanternward.Session.step.seat(+s: J.Val, +id: U32, +inp: J.Val) -> J.Val:
  lanternward.Session.step.seat.valid(
    Bool.and(Bool.not(J.is_null(inp)), rules.Input.valid(inp)), s, id, inp)

def lanternward.Session.step.seat.valid(ok: Bool, +s: J.Val, +id: U32, +inp: J.Val) -> J.Val:
  match ok:
    case True{}:
      lanternward.kit.Session.step_seat(s, id, inp)
    case False{}:
      lanternward.kit.Session.step_seat(s, id, rules.Input.default())

# One tick: whoever is here and awake sits at the table, then every body acts
# in its order. `r` is the Rules ({lock, parry, combo, flame, tell}).
def lanternward.Session.step(+s: J.Val, +inputs: J.Val, +heroes: J.Val, +r: J.Val) -> J.Val:
  lanternward.Session.step.go(
    lanternward.Session.step.sit(
      ga.ids(heroes),
      lanternward.Session.step.stand(
        ga.ids(J.get(s, "seats")),
        ga.su(ga.sv(ga.sv(s, "$r", r), "$ev", lanternward.Events.default()), "elapsed", RS.u_min((ga.u(s, "elapsed") + 1 : U32), 108000)),
        heroes),
      heroes),
    inputs)

def lanternward.Session.step.go(+s: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Session.step.go2(lanternward.foes.Session.provision(s), inputs)

def lanternward.Session.step.go2(+s: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Session.step.rest(lanternward.Session.step.seats(ga.ids(J.get(s, "seats")), s, inputs))

def lanternward.Session.step.rest(+s: J.Val) -> J.Val:
  lanternward.stage.Session.age(
    lanternward.stage.Session.tick_wipe(
      lanternward.stage.Session.tick_stage(
        lanternward.stage.Session.revive_pass(
          lanternward.stage.Session.pickups(
            lanternward.blow.Session.step_fuses(
              lanternward.blow.Session.step_bolts(
                lanternward.boss.Session.step_boss(
                  lanternward.foes.Session.step_foes(
                    lanternward.stage.Session.unstick(
                      lanternward.stage.Session.tick_props(s)))))))))))

# Whether the finale is on: the Beacon is lit and the party may leave.
def lanternward.Session.finale(+s: J.Val) -> Bool:
  lanternward.Session.finale.go(J.is_null(J.get(s, "done")), s)

def lanternward.Session.finale.go(none: Bool, +s: J.Val) -> Bool:
  match none:
    case True{}:
      False{}
    case False{}:
      U32.is_ge(ga.u(s, "elapsed"), (J.u32(J.get(s, "done")) + 120 : U32))

# ---- valid / problem / repair ----

def lanternward.problem.inside(+x: U32, +y: U32, +w: U32, +h: U32) -> Bool:
  Bool.and(lanternward.in_range(x, 0, w), lanternward.in_range(y, 0, h))

def lanternward.problem.knights(+ids: List<&2, U32>, +seats: J.Val, +elapsed: U32, +w: U32, +h: U32) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+id, +t}:
      Bool.and(
        Bool.and(U32.is_ge(id, 1), U32.is_le(id, 6)),
        Bool.and(lanternward.kit.Seat.valid(ga.at(seats, id), id, elapsed, w, h), lanternward.problem.knights(t, seats, elapsed, w, h)))

def lanternward.problem.props(+ps: List<&2, J.Val>, +pw: U32, +ph: U32, +w: U32, +h: U32) -> Bool:
  match ps:
    case Nil{}:
      True{}
    case Con{+q, +t}:
      Bool.and(
        Bool.and(Bool.and(RS.i_lt(ga.u(q, "tx"), pw), RS.i_lt(ga.u(q, "ty"), ph)), U32.is_le(ga.u(q, "on"), 4)),
        Bool.and(lanternward.problem.inside(ga.u(q, "x"), ga.u(q, "y"), w, h), lanternward.problem.props(t, pw, ph, w, h)))

def lanternward.problem.foes(+fs: List<&2, J.Val>, +w: U32, +h: U32) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{+f, +t}:
      Bool.and(
        Bool.and(
          Bool.and(U32.is_le(ga.u(f, "hp"), ga.u(f, "full")), U32.is_le(ga.u(f, "full"), 48)),
          Bool.and(Bool.and(U32.is_ge(ga.u(f, "prov"), 1), U32.is_le(ga.u(f, "prov"), 6)), lanternward.problem.inside(ga.u(f, "x"), ga.u(f, "y"), w, h))),
        Bool.and(
          Bool.and(U32.is_le(ga.u(f, "t"), 600), U32.is_le(ga.u(f, "dead"), 14)),
          Bool.and(RS.i_le(lanternward.world.dist(ga.u(f, "fx"), ga.u(f, "fy")), 264), lanternward.problem.foes(t, w, h))))

def lanternward.problem.bolts(+bs: List<&2, J.Val>, +w: U32, +h: U32) -> Bool:
  match bs:
    case Nil{}:
      True{}
    case Con{+b, +t}:
      Bool.and(
        Bool.and(
          lanternward.problem.inside(ga.u(b, "x"), ga.u(b, "y"), w, h),
          Bool.and(lanternward.in_range(ga.u(b, "vx"), 4294965248, 2048), lanternward.in_range(ga.u(b, "vy"), 4294965248, 2048))),
        Bool.and(Bool.and(U32.is_le(ga.u(b, "ttl"), 400), U32.is_le(ga.u(b, "owner"), 6)), lanternward.problem.bolts(t, w, h)))

def lanternward.problem.drops(+ds: List<&2, J.Val>, +w: U32, +h: U32) -> Bool:
  match ds:
    case Nil{}:
      True{}
    case Con{+d, +t}:
      Bool.and(lanternward.problem.inside(ga.u(d, "x"), ga.u(d, "y"), w, h), lanternward.problem.drops(t, w, h))

def lanternward.problem.fuses(+fs: List<&2, J.Val>, +w: U32, +h: U32) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{+f, +t}:
      Bool.and(
        Bool.and(lanternward.problem.inside(ga.u(f, "x"), ga.u(f, "y"), w, h), U32.is_le(ga.u(f, "t"), 75)),
        lanternward.problem.fuses(t, w, h))

def lanternward.problem.kept(+ids: List<&2, U32>, +kept: J.Val) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+id, +t}:
      Bool.and(
        Bool.and(Bool.and(U32.is_ge(id, 1), U32.is_le(id, 6)), Bool.and(U32.is_le(ga.nu(ga.at(kept, id), 0), 100000), U32.is_le(ga.nu(ga.at(kept, id), 1), 10))),
        lanternward.problem.kept(t, kept))

# The first named bound the session breaks, else null.
def lanternward.Session.problem(+s: J.Val) -> J.Val:
  lanternward.problem.go(s, lanternward.world.Plan.w(lanternward.world.plan(ga.u(s, "room"))), lanternward.world.Plan.h(lanternward.world.plan(ga.u(s, "room"))))

def lanternward.problem.go(+s: J.Val, +pw: U32, +ph: U32) -> J.Val:
  lanternward.problem.checks(s, pw, ph, (pw * 4096 : U32), (ph * 4096 : U32))

def lanternward.problem.pick(ok: Bool, +name: String, +next: J.Val) -> J.Val:
  Bool.pick(J.Val, ok, next, J.JS{name})

def lanternward.problem.checks(+s: J.Val, +pw: U32, +ph: U32, +w: U32, +h: U32) -> J.Val:
  lanternward.problem.pick(
    Bool.and(U32.is_le(ga.u(s, "elapsed"), 108000), U32.is_le(ga.u(s, "stage_t"), 108000)), "clock",
    lanternward.problem.pick(
      Bool.and(U32.is_lt(ga.u(s, "room"), 5), U32.is_le(ga.u(s, "stage"), 6)), "room",
      lanternward.problem.pick(
        U32.is_le(lanternward.seat_count(s), 6), "seats",
        lanternward.problem.pick(
          lanternward.problem.knights(ga.ids(J.get(s, "seats")), J.get(s, "seats"), ga.u(s, "elapsed"), w, h), "knight",
          lanternward.problem.pick(
            U32.is_le(lanternward.len(s, "props"), 160), "props",
            lanternward.problem.pick(
              lanternward.problem.props(ga.items(s, "props"), pw, ph, w, h), "prop",
              lanternward.problem.pick(
                U32.is_le(lanternward.len(s, "foes"), 16), "foes",
                lanternward.problem.pick(
                  lanternward.problem.foes(ga.items(s, "foes"), w, h), "foe",
                  lanternward.problem.pick(
                    U32.is_le(lanternward.len(s, "bolts"), 40), "bolts",
                    lanternward.problem.pick(
                      lanternward.problem.bolts(ga.items(s, "bolts"), w, h), "bolt",
                      lanternward.problem.checks2(s, w, h)))))))))))

def lanternward.problem.checks2(+s: J.Val, +w: U32, +h: U32) -> J.Val:
  lanternward.problem.pick(
    Bool.and(U32.is_le(lanternward.len(s, "drops"), 32), lanternward.problem.drops(ga.items(s, "drops"), w, h)), "drops",
    lanternward.problem.pick(
      U32.is_le(lanternward.len(s, "fx"), 80), "fx",
      lanternward.problem.pick(
        Bool.and(U32.is_le(lanternward.len(s, "fuses"), 8), lanternward.problem.fuses(ga.items(s, "fuses"), w, h)), "fuses",
        lanternward.problem.pick(
          lanternward.problem.boss_ok(J.get(s, "boss"), ga.u(s, "room"), w, h), "boss",
          lanternward.problem.pick(
            Bool.and(U32.is_le(ga.u(ga.v(s, "inv"), "small_keys"), 8), U32.is_le(ga.u(ga.v(s, "inv"), "containers"), 8)), "keys",
            lanternward.problem.pick(
              Bool.and(U32.is_le(ga.u(s, "shake"), 64), U32.is_le(ga.u(s, "flash"), 64)), "shake",
              lanternward.problem.pick(
                Bool.and(U32.is_le(ga.u(s, "exiting"), 14), U32.is_le(ga.u(s, "wipe"), 24)), "exit",
                lanternward.problem.checks3(s, w, h))))))))

def lanternward.problem.boss_ok(+b: J.Val, +room: U32, +w: U32, +h: U32) -> Bool:
  match b:
    case J.JNull{}:
      True{}
    case _:
      Bool.and(
        Bool.and(U32.is_eq(room, 4), lanternward.boss.Warden.valid(b)),
        lanternward.problem.inside(ga.u(b, "x"), ga.u(b, "y"), w, h))

def lanternward.problem.cue_ok(+c: J.Val, +w: U32, +h: U32) -> Bool:
  match c:
    case J.JNull{}:
      True{}
    case _:
      Bool.and(lanternward.problem.inside(ga.nu(c, 0), ga.nu(c, 1), w, h), U32.is_le(ga.nu(c, 2), 44))

def lanternward.problem.checks3(+s: J.Val, +w: U32, +h: U32) -> J.Val:
  lanternward.problem.pick(
    lanternward.problem.cue_ok(J.get(s, "cue"), w, h), "cue",
    lanternward.problem.pick(
      Bool.and(U32.is_eq(U32.and(ga.u(s, "paid"), 1), 0), U32.is_lt(ga.u(s, "paid"), 128)), "paid",
      lanternward.problem.pick(
        lanternward.problem.done_ok(J.get(s, "done"), ga.u(s, "elapsed")), "done",
        lanternward.problem.pick(
          lanternward.problem.banner_ok(J.get(s, "banner")), "banner",
          lanternward.problem.pick(
            U32.is_lt(ga.u(s, "tries"), 1000000), "tries",
            lanternward.problem.pick(
              Bool.and(
                U32.is_le(lanternward.fields_count(J.get(s, "kept")), 6),
                lanternward.problem.kept(ga.ids(J.get(s, "kept")), J.get(s, "kept"))), "kept",
              J.JNull{}))))))

def lanternward.problem.done_ok(+d: J.Val, +elapsed: U32) -> Bool:
  match d:
    case J.JNull{}:
      True{}
    case _:
      U32.is_le(J.u32(d), elapsed)

def lanternward.problem.banner_ok(+b: J.Val) -> Bool:
  match b:
    case J.JNull{}:
      True{}
    case _:
      U32.is_le(ga.u(b, "t"), 60)

def lanternward.fields_count(+o: J.Val) -> U32:
  U32.from_nat(List.length(&2, Sigma<&2, &2, String, _ => J.Val>, J.fields(o)))

def lanternward.Session.valid(+s: J.Val) -> Bool:
  J.is_null(lanternward.Session.problem(s))

# ---- repair ----

# Mend what a bad save broke.
def lanternward.Session.repair(+s: J.Val) -> J.Val:
  lanternward.repair.seats(ga.ids(J.get(s, "seats")), ga.su(s, "elapsed", RS.u_min(ga.u(s, "elapsed"), 108000)))

# Keep only seats 1..=6, and clamp each one's `joined` to the clock.
def lanternward.repair.seats(+ids: List<&2, U32>, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      lanternward.repair.rebuild(lanternward.Session.valid(s), s)
    case Con{+id, +t}:
      lanternward.repair.seats(t, lanternward.repair.seat(Bool.and(U32.is_ge(id, 1), U32.is_le(id, 6)), s, id))

def lanternward.repair.seat(ok: Bool, +s: J.Val, +id: U32) -> J.Val:
  match ok:
    case False{}:
      ga.sv(s, "seats", J.del(J.get(s, "seats"), U32.show(id)))
    case True{}:
      lanternward.blow.put_seat(s, id,
        ga.su(ga.at(J.get(s, "seats"), id), "joined", RS.u_min(ga.u(ga.at(J.get(s, "seats"), id), "joined"), ga.u(s, "elapsed"))))

def lanternward.repair.rebuild(good: Bool, +s: J.Val) -> J.Val:
  match good:
    case True{}:
      s
    case False{}:
      lanternward.repair.fresh(
        ga.ids(J.get(s, "seats")),
        lanternward.repair.clear(ga.su(s, "room", RS.u_min(ga.u(s, "room"), 4))))

# Whatever else is wrong, the room is rebuilt as it opens and the party stands
# at its start with full hearts.
def lanternward.repair.clear(+s: J.Val) -> J.Val:
  lanternward.repair.keys(
    ga.su(
      ga.su(ga.su(ga.su(ga.su(
        ga.sv(ga.sv(ga.sv(ga.sv(s, "fx", J.JA{Nil{}}), "banner", J.JNull{}), "cue", J.JNull{}), "done", lanternward.repair.done(J.get(s, "done"), ga.u(s, "elapsed"))),
        "shake", 0), "flash", 0), "exiting", 0), "wipe", 0),
      "paid", U32.and(ga.u(s, "paid"), 126)))

def lanternward.repair.done(+d: J.Val, +elapsed: U32) -> J.Val:
  match d:
    case J.JNull{}:
      J.JNull{}
    case _:
      Bool.pick(J.Val, U32.is_le(J.u32(d), elapsed), d, J.JNull{})

def lanternward.repair.kept(+ids: List<&2, U32>, +kept: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      kept
    case Con{+id, +t}:
      lanternward.repair.kept(t,
        Bool.pick(J.Val, Bool.and(U32.is_ge(id, 1), U32.is_le(id, 6)), kept, J.del(kept, U32.show(id))))

def lanternward.repair.keys(+s: J.Val) -> J.Val:
  ga.sv(
    ga.sv(s, "kept", lanternward.repair.kept(ga.ids(J.get(s, "kept")), J.get(s, "kept"))),
    "inv", ga.su(ga.su(ga.v(s, "inv"), "small_keys", RS.u_min(ga.u(ga.v(s, "inv"), "small_keys"), 8)), "containers", RS.u_min(ga.u(ga.v(s, "inv"), "containers"), 8)))

# Each seat is made fresh (keeping when it joined), then the room is entered.
def lanternward.repair.fresh(+ids: List<&2, U32>, +s: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      lanternward.Session.bare(lanternward.stage.Session.enter_room(ga.sv(s, "$r", lanternward.Rules.default.full()), ga.u(s, "room")))
    case Con{+id, +t}:
      lanternward.repair.fresh(t, lanternward.repair.fresh.one(lanternward.stage.Session.start_of(s, id), s, id))

def lanternward.repair.fresh.one(+at: J.Val, +s: J.Val, +id: U32) -> J.Val:
  +hearts = (20 + 4 * ga.u(ga.v(s, "inv"), "containers") : U32)
  lanternward.blow.put_seat(s, id,
    ga.su(ga.su(lanternward.kit.Seat.new(id, ga.u(ga.at(J.get(s, "seats"), id), "joined"), ga.nu(at, 0), ga.nu(at, 1)), "max_hp", hearts), "hp", hearts))
