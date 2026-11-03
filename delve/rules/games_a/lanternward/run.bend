# crates/delve/src/rules/lanternward.rs — `impl Run` hooks, the ADVENTURE and
# ACTIVITY closures, `settle`. The Run is its J object.

def lanternward.run.sound(+run: J.Val, +name: String) -> J.Val:
  J.set(run, "sounds", J.push(J.get(run, "sounds"), J.JS{name}))

def lanternward.run.found(+run: J.Val, +text: String) -> J.Val:
  J.set(run, "found", J.JA{[J.get(run, "tick"), J.JU{0}, J.JS{text}]})

# The Score of slot 0 in `home.countryside.games.roundtable` (default when the
# array is omitted).
def lanternward.run.score(+run: J.Val) -> J.Val:
  lanternward.run.score.of(J.nth(ga.v(ga.v(ga.v(ga.v(run, "home"), "countryside"), "games"), "roundtable"), 0))

def lanternward.run.score.of(+sc: J.Val) -> J.Val:
  match sc:
    case J.JNull{}:
      ga.su(ga.su(ga.su(J.obj(), "best", 0), "wins", 0), "plays", 0)
    case _:
      sc

def lanternward.run.score_set(+run: J.Val, sc: J.Val) -> J.Val:
  J.put(run, ["home", "countryside", "games", "roundtable"],
    J.set_nth(lanternward.run.roundtable(J.get(J.get(J.get(run, "home"), "countryside"), "games")), 0, sc))

# The `[Score; 5]`, filled with defaults when serde omitted it.
def lanternward.run.roundtable(+games: J.Val) -> J.Val:
  lanternward.run.roundtable.of(J.get(games, "roundtable"))

def lanternward.run.roundtable.of(+a: J.Val) -> J.Val:
  match a:
    case J.JNull{}:
      J.JA{[lanternward.run.score.of(J.JNull{}), lanternward.run.score.of(J.JNull{}), lanternward.run.score.of(J.JNull{}), lanternward.run.score.of(J.JNull{}), lanternward.run.score.of(J.JNull{})]}
    case _:
      a

# `Run::lanternward_showing`.
def lanternward.Run.lanternward_showing(+run: J.Val) -> Bool:
  Bool.and(lanternward.tag_is(ga.v(rules.Run.room(run), "kind"), "Forest"), Bool.not(J.is_null(J.get(run, "lanternward"))))

# `Run::start_lanternward`: the table sits down and the play is counted.
def lanternward.Run.start_lanternward(+run: J.Val) -> J.Val:
  lanternward.Run.start_lanternward.go(lanternward.tag_is(ga.v(rules.Run.room(run), "kind"), "Forest"), run)

def lanternward.Run.start_lanternward.go(forest: Bool, +run: J.Val) -> J.Val:
  match forest:
    case False{}:
      run
    case True{}:
      lanternward.Run.start_lanternward.sit(J.set(run, "forest", forest.Forest.leave(J.get(run, "forest"))))

def lanternward.Run.start_lanternward.sit(+run: J.Val) -> J.Val:
  lanternward.Run.start_lanternward.count(J.set(run, "lanternward", lanternward.Session.new(J.get(run, "players"))))

def lanternward.Run.start_lanternward.count(+run: J.Val) -> J.Val:
  lanternward.run.sound(
    lanternward.run.found(
      lanternward.run.score_set(run, ga.su(lanternward.run.score(run), "plays", RS.u_sat_add(ga.u(lanternward.run.score(run), "plays"), 1))),
      "Lanternward: R locks on. Hold Esc to leave."),
    "play_card")

# The players at the table: those not outside the room.
def lanternward.run.present(+ids: List<&2, U32>, +players: J.Val, +outside: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      J.obj()
    case Con{+id, +t}:
      Bool.pick(J.Val, ga.has_key(outside, id),
        lanternward.run.present(t, players, outside),
        ga.put_at(lanternward.run.present(t, players, outside), id, ga.at(players, id)))

# Sounds from the tick, then the weight of impact for the engine's hit-stop.
def lanternward.run.sounds(+run: J.Val, +xs: List<&2, J.Val>) -> J.Val:
  match xs:
    case Nil{}:
      run
    case Con{+h, +t}:
      lanternward.run.sounds(lanternward.run.sound(run, J.str(h)), t)

def lanternward.run.stop(+run: J.Val, +stop: J.Val) -> J.Val:
  match stop:
    case J.JNull{}:
      run
    case _:
      J.set(run, "hitstop", hitstop.State.request(J.get(run, "hitstop"), J.get(run, "tune"),
        Bool.pick(String, lanternward.tag_is(stop, "Heavy"), "Heavy",
          Bool.pick(String, lanternward.tag_is(stop, "Kill"), "EliteKill",
            Bool.pick(String, lanternward.tag_is(stop, "Phase"), "Phase", "BossKill")))))

# `Run::tick_lanternward(inputs)`: step the session; if the Beacon was relit,
# pay the table; whoever pressed E in the finale leaves.
def lanternward.Run.tick_lanternward(+run: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Run.tick_lanternward.go(J.is_null(J.get(run, "lanternward")), run, inputs)

def lanternward.Run.tick_lanternward.go(none: Bool, +run: J.Val, +inputs: J.Val) -> J.Val:
  match none:
    case True{}:
      run
    case False{}:
      lanternward.Run.tick_lanternward.step(
        lanternward.Session.step(
          J.get(run, "lanternward"), inputs,
          lanternward.run.present(ga.ids(J.get(run, "players")), J.get(run, "players"), ga.v(ga.v(run, "exits"), "outside")),
          lanternward.Rules.of(J.get(run, "tune"))),
        J.del(run, "lanternward"), inputs)

def lanternward.Run.tick_lanternward.step(+s: J.Val, +run: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Run.tick_lanternward.won(
    ga.b(J.get(s, "$ev"), "won"),
    lanternward.run.stop(
      lanternward.run.sounds(run, ga.items(J.get(s, "$ev"), "sounds")),
      ga.v(J.get(s, "$ev"), "stop")),
    lanternward.Session.bare(s), inputs)

def lanternward.Run.tick_lanternward.won(won: Bool, +run: J.Val, +s: J.Val, +inputs: J.Val) -> J.Val:
  match won:
    case False{}:
      lanternward.Run.tick_lanternward.leave(run, s, inputs)
    case True{}:
      lanternward.Run.tick_lanternward.finish(lanternward.Run.finish_lanternward(run, s), inputs)

# `finish_lanternward` leaves the paid session in `$s`; put it back and let
# those who asked to leave go.
def lanternward.Run.tick_lanternward.finish(+run: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Run.tick_lanternward.leave(J.del(run, "$s"), J.get(run, "$s"), inputs)

def lanternward.Run.tick_lanternward.leave(+run: J.Val, +s: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.run.leaving(
    lanternward.run.leavers(ga.ids(inputs), inputs, s), J.set(run, "lanternward", s))

def lanternward.run.leavers(+ids: List<&2, U32>, +inputs: J.Val, +s: J.Val) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      Bool.pick(List<&2, U32>,
        Bool.and(
          Bool.and(rules.Input.valid(ga.at(inputs, id)), ga.b(ga.at(inputs, id), "interact")),
          Bool.and(lanternward.Session.finale(s), ga.has_key(J.get(s, "seats"), id))),
        Con{id, lanternward.run.leavers(t, inputs, s)},
        lanternward.run.leavers(t, inputs, s))

def lanternward.run.leaving(+ids: List<&2, U32>, +run: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      run
    case Con{+id, +t}:
      lanternward.run.leaving(t, J.del(exits.Run.leave_activity(run, id), "$ret"))

# The Beacon burns: the campaign is marked, each knight at the table is paid
# once, and a gold crown goes to every one of them. The session (with `paid`
# updated) is returned in `$s`.
def lanternward.Run.finish_lanternward(+run: J.Val, +s: J.Val) -> J.Val:
  lanternward.run.found(
    lanternward.run.sound(
      lanternward.run.pay(
        ga.ids(J.get(s, "seats")),
        lanternward.run.record(
          J.put(run, ["marks", "lanternward:campaign-begun"], J.JU{1}),
          ga.u(s, "elapsed"),
          Bool.and(
            U32.is_eq(U32.and(ga.nu(ga.v(ga.v(run, "home"), "roundtable"), 0), 1), 0),
            Bool.not(J.has(ga.v(run, "marks"), "lanternward:campaign-begun")))),
        s, Bool.and(
          U32.is_eq(U32.and(ga.nu(ga.v(ga.v(run, "home"), "roundtable"), 0), 1), 0),
          Bool.not(J.has(ga.v(run, "marks"), "lanternward:campaign-begun")))),
      "spoil_pickup"),
    "Lanternward: the Beacon burns again.")

# wins +1 (never above plays); best = the better of it and the rating.
def lanternward.run.record(+run: J.Val, +elapsed: U32, +first: Bool) -> J.Val:
  lanternward.run.record.go(run, U32.div(elapsed, 30))

def lanternward.run.record.go(+run: J.Val, +secs: U32) -> J.Val:
  lanternward.run.record.set(
    lanternward.run.score(run),
    RS.u_clamp(RS.u_sat_sub(1000, (RS.u_sat_sub(secs, 180) * 2 : U32)), 1, 1000), run)

def lanternward.run.record.set(+sc: J.Val, +rating: U32, +run: J.Val) -> J.Val:
  lanternward.run.score_set(run,
    ga.su(ga.su(sc, "wins", RS.u_min(RS.u_sat_add(ga.u(sc, "wins"), 1), ga.u(sc, "plays"))), "best", RS.u_max(ga.u(sc, "best"), rating)))

# Each knight at the table not yet paid is paid: gold, a laurel, no booby prize.
def lanternward.run.pay(+ids: List<&2, U32>, +run: J.Val, +s: J.Val, +first: Bool) -> J.Val:
  match ids:
    case Nil{}:
      ga.sv(run, "$s", s)
    case Con{+id, +t}:
      lanternward.run.pay.one(U32.is_ne(U32.and(ga.u(s, "paid"), U32.shln(1, U32.to_nat(id))), 0), t, run, s, id, first)

def lanternward.run.pay.one(paid: Bool, +t: List<&2, U32>, +run: J.Val, +s: J.Val, +id: U32, +first: Bool) -> J.Val:
  match paid:
    case True{}:
      lanternward.run.pay(t, run, s, first)
    case False{}:
      lanternward.run.pay(t,
        lanternward.run.pay.hero(ga.has_key(ga.v(run, "players"), id), run, id, lanternward.purse(first, ga.u(ga.at(J.get(s, "seats"), id), "gems"))),
        ga.su(s, "paid", U32.or(ga.u(s, "paid"), U32.shln(1, U32.to_nat(id)))), first)

def lanternward.run.pay.hero(has: Bool, +run: J.Val, +id: U32, +gold: U32) -> J.Val:
  match has:
    case False{}:
      run
    case True{}:
      J.set(run, "players", ga.put_at(J.get(run, "players"), id,
        ga.sb(
          ga.sv(ga.sv(ga.at(J.get(run, "players"), id), "carried", realm.Spoils.add(ga.v(ga.at(J.get(run, "players"), id), "carried"), "gold", gold)), "booby", J.JNull{}),
          "laurel", True{})))

# ---- ADVENTURE ----

def lanternward.ADVENTURE.valid(+run: J.Val) -> Bool:
  lanternward.ADVENTURE.valid.go(J.get(run, "lanternward"))

def lanternward.ADVENTURE.valid.go(+s: J.Val) -> Bool:
  match s:
    case J.JNull{}:
      True{}
    case _:
      lanternward.Session.valid(s)

def lanternward.ADVENTURE.clear(+run: J.Val) -> J.Val:
  J.del(run, "lanternward")

def lanternward.ADVENTURE.seated(+run: J.Val) -> J.Val:
  lanternward.ADVENTURE.seated.go(J.get(run, "lanternward"))

def lanternward.ADVENTURE.seated.go(+s: J.Val) -> J.Val:
  match s:
    case J.JNull{}:
      J.JA{Nil{}}
    case _:
      J.JA{lanternward.ids_j(ga.ids(J.get(s, "seats")))}

def lanternward.ids_j(+ids: List<&2, U32>) -> List<&2, J.Val>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{J.JU{h}, lanternward.ids_j(t)}

# The scripted bot's keys for seat `id`.
def lanternward.ADVENTURE.bot(+run: J.Val, +id: U32) -> J.Val:
  lanternward.ADVENTURE.bot.go(
    Bool.and(Bool.not(J.is_null(J.get(run, "lanternward"))), ga.has_key(J.get(run, "players"), id)), run, id)

def lanternward.ADVENTURE.bot.go(both: Bool, +run: J.Val, +id: U32) -> J.Val:
  match both:
    case False{}:
      rules.Input.default()
    case True{}:
      lanternward.bot.Session.bot_input(J.get(run, "lanternward"), id, lanternward.Rules.of(J.get(run, "tune")))

# ---- ACTIVITY ----

def lanternward.ACTIVITY.on(+run: J.Val) -> Bool:
  lanternward.Run.lanternward_showing(run)

def lanternward.ACTIVITY.ride(+run: J.Val, +inputs: J.Val) -> J.Val:
  lanternward.Run.tick_lanternward(run, inputs)

def lanternward.ACTIVITY.end(+run: J.Val) -> J.Val:
  J.del(run, "lanternward")

# ---- settle ----

# `settle(realm, marks)`: a finished run's mark becomes a flag in the Home.
# In: {home, marks}; out: {home, changed, said}.
def lanternward.settle.flags(+home: J.Val) -> J.Val:
  lanternward.settle.flags.of(J.get(home, "roundtable"))

def lanternward.settle.flags.of(+a: J.Val) -> J.Val:
  match a:
    case J.JNull{}:
      J.JA{[J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}]}
    case _:
      a

def lanternward.settle(+realm: J.Val, +marks: J.Val) -> J.Val:
  lanternward.settle.go(
    Bool.and(
      J.has(marks, "lanternward:campaign-begun"),
      U32.is_eq(U32.and(ga.nu(lanternward.settle.flags(ga.v(realm, "home")), 0), 1), 0)),
    realm)

def lanternward.settle.go(changed: Bool, +realm: J.Val) -> J.Val:
  match changed:
    case False{}:
      ga.sv(ga.sb(J.obj(), "changed", False{}), "said", J.JA{Nil{}})
    case True{}:
      ga.sv(
        ga.sv(
          ga.sb(J.obj(), "changed", True{}), "said",
          J.JA{[ga.pair(J.JS{"lanternward"}, J.JS{"Lanternward: the Beacon burns again."})]}),
        "home",
        ga.sv(ga.v(realm, "home"), "roundtable",
          J.set_nth(lanternward.settle.flags(ga.v(realm, "home")), 0, J.JU{U32.or(ga.nu(lanternward.settle.flags(ga.v(realm, "home")), 0), 1)})))
