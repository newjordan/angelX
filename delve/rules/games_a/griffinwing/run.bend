# crates/delve/src/rules/griffinwing.rs — the Run's hooks: ADVENTURE and
# ACTIVITY (the shell's registrations), the door, the tick and the finish.
# `run` is the checkpoint's `run` object. Cross-group state it touches:
# `forest`, `home.roundtable`, `home.countryside.games.roundtable`, `marks`,
# `found`, `cues`, `sounds`, the heroes' `carried` and `laurel`.

def griffinwing.NAME() -> String:
  "Griffinwing"

# The Dark Forest's room is the one an adventure runs in.
def griffinwing.Run.in_forest(+run: J.Val) -> Bool:
  String.eq(J.str(J.get(rules.Run.room(run), "kind")), "forest")

def griffinwing.Run.griffinwing_showing(+run: J.Val) -> Bool:
  Bool.and(griffinwing.Run.in_forest(run), Bool.not(J.is_null(J.get(run, "griffinwing"))))

# `home.roundtable[SLOT]`: the flags settled into the Home.
def griffinwing.Run.flags(+run: J.Val) -> U32:
  ga.nu(J.get(J.get(run, "home"), "roundtable"), griffinwing.SLOT())

# `home.countryside.games.roundtable`: five records (best, wins, plays).
def griffinwing.Run.records(+run: J.Val) -> J.Val:
  J.get(J.get(J.get(J.get(run, "home"), "countryside"), "games"), "roundtable")

def griffinwing.Score.default() -> J.Val:
  ga.su(ga.su(ga.su(J.obj(), "best", 0), "wins", 0), "plays", 0)

# The record of this game.
def griffinwing.Run.record(+run: J.Val) -> J.Val:
  gw.ifj(J.is_null(J.nth(griffinwing.Run.records(run), griffinwing.SLOT())),
    _ => griffinwing.Score.default(),
    _ => J.nth(griffinwing.Run.records(run), griffinwing.SLOT()))

# The five records, with this game's replaced (all five are written: absent
# means none had one).
def griffinwing.Run.put_record(+run: J.Val, +rec: J.Val) -> J.Val:
  ga.sv(run, "home",
    ga.sv(J.get(run, "home"), "countryside",
      ga.sv(J.get(J.get(run, "home"), "countryside"), "games",
        ga.sv(J.get(J.get(J.get(run, "home"), "countryside"), "games"), "roundtable",
          J.set_nth(
            gw.ifj(J.is_null(griffinwing.Run.records(run)),
              _ => J.JA{[griffinwing.Score.default(), griffinwing.Score.default(), griffinwing.Score.default(), griffinwing.Score.default(), griffinwing.Score.default()]},
              _ => griffinwing.Run.records(run)),
            griffinwing.SLOT(), rec)))))

# `home.roundtable[SLOT] |= flags`; the array is left out when all zero.
def griffinwing.Run.put_flags(+run: J.Val, +flags: U32) -> J.Val:
  ga.sv(run, "home", griffinwing.Run.put_flags.home(J.get(run, "home"), griffinwing.Run.put_flags.words(J.get(J.get(run, "home"), "roundtable"), flags)))

def griffinwing.Run.put_flags.words(+rt: J.Val, +flags: U32) -> J.Val:
  J.set_nth(
    gw.ifj(J.is_null(rt), _ => J.JA{[J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}]}, _ => rt),
    griffinwing.SLOT(), J.JU{U32.or(ga.nu(rt, griffinwing.SLOT()), flags)})

def griffinwing.Run.put_flags.home(+home: J.Val, +words: J.Val) -> J.Val:
  gw.ifj(griffinwing.Run.all_zero(J.items(words)),
    _ => J.del(home, "roundtable"),
    _ => ga.sv(home, "roundtable", words))

def griffinwing.Run.all_zero(+ws: List<&2, J.Val>) -> Bool:
  match ws:
    case Nil{}:
      True{}
    case Con{+w, +t}:
      Bool.and(U32.is_zero(J.u32(w)), griffinwing.Run.all_zero(t))

# The notice line: `found = Some((tick, 0, text))`.
def griffinwing.Run.found(+run: J.Val, +text: String) -> J.Val:
  ga.sv(run, "found", J.JA{[J.get(run, "tick"), J.JU{0}, J.JS{text}]})

def griffinwing.Run.sound(+run: J.Val, +name: String) -> J.Val:
  ga.push(run, "sounds", J.JS{name})

# The party at the door, let in.
def griffinwing.Run.start_griffinwing(+run: J.Val) -> J.Val:
  gw.ifj(Bool.not(griffinwing.Run.in_forest(run)),
    _ => run,
    _ => griffinwing.Run.start_griffinwing.go(run))

def griffinwing.Run.start_griffinwing.go(+run: J.Val) -> J.Val:
  griffinwing.Run.start_griffinwing.fin(
    ga.sv(run, "forest", forest.Forest.leave(J.get(run, "forest"))))

def griffinwing.Run.start_griffinwing.fin(+run: J.Val) -> J.Val:
  griffinwing.Run.sound(
    griffinwing.Run.found(
      griffinwing.Run.put_record(
        ga.sv(run, "griffinwing",
          ga.sb(griffinwing.Session.new(J.get(run, "players")), "first", U32.is_zero(U32.and(griffinwing.Run.flags(run), griffinwing.BEGUN())))),
        ga.su(griffinwing.Run.record(run), "plays", ga.sadd32(ga.u(griffinwing.Run.record(run), "plays"), 1))),
      griffinwing.NAME() ++ ": hold Esc to return."),
    "play_card")

# `present`: the heroes not outside (those who left the run for the kingdom).
def griffinwing.Run.present(+players: J.Val, +outside: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      J.obj()
    case Con{+id, +t}:
      gw.ifj(ga.has_key(outside, id),
        _ => griffinwing.Run.present(players, outside, t),
        _ => ga.put_at(griffinwing.Run.present(players, outside, t), id, ga.at(players, id)))

def griffinwing.Run.tick_griffinwing(+run: J.Val, +inputs: J.Val) -> J.Val:
  gw.ifj(J.is_null(J.get(run, "griffinwing")),
    _ => run,
    _ => griffinwing.Run.tick_griffinwing.step(run, griffinwing.Session.step(J.get(run, "griffinwing"), inputs,
      griffinwing.Run.present(J.get(run, "players"), J.get(J.get(run, "exits"), "outside"), ga.ids(J.get(run, "players"))),
      griffinwing.Nums.of(J.get(run, "tune")))))

# r = {"s": the stepped session, "out": {"sounds", "leave"}}
def griffinwing.Run.tick_griffinwing.step(+run: J.Val, +r: J.Val) -> J.Val:
  griffinwing.Run.tick_griffinwing.settle(
    griffinwing.Run.tick_griffinwing.sounds(run, ga.items(J.get(r, "out"), "sounds")),
    J.get(r, "s"), ga.items(J.get(r, "out"), "leave"))

def griffinwing.Run.tick_griffinwing.sounds(+run: J.Val, +names: List<&2, J.Val>) -> J.Val:
  match names:
    case Nil{}:
      run
    case Con{+n, +t}:
      griffinwing.Run.tick_griffinwing.sounds(ga.push(run, "sounds", n), t)

def griffinwing.Run.tick_griffinwing.settle(+run: J.Val, +s: J.Val, +leave: List<&2, J.Val>) -> J.Val:
  gw.ifj(griffinwing.Session.finished(s),
    _ => griffinwing.Run.tick_griffinwing.put(griffinwing.Run.settle_griffinwing(run, s), leave),
    _ => griffinwing.Run.tick_griffinwing.put(gw.pk(run, s), leave))

# w = {"s": run, "c": session}
def griffinwing.Run.tick_griffinwing.put(+w: J.Val, +leave: List<&2, J.Val>) -> J.Val:
  griffinwing.Run.tick_griffinwing.leaves(ga.sv(gw.ps(w), "griffinwing", gw.pc(w)), leave)

def griffinwing.Run.tick_griffinwing.leaves(+run: J.Val, +ids: List<&2, J.Val>) -> J.Val:
  match ids:
    case Nil{}:
      run
    case Con{+id, +t}:
      griffinwing.Run.tick_griffinwing.leaves(exits.Run.leave_activity(run, J.u32(id)), t)

# The Wyrm is down: the party's Home is marked and the purse is paid, once for
# each knight who flew it home, however often this is called. Answers
# {"s": the run, "c": the session}.
def griffinwing.Run.settle_griffinwing(+run: J.Val, +s: J.Val) -> J.Val:
  griffinwing.Run.settle_griffinwing.pay(run, s,
    gw.ifs(J.is_null(J.get(s, "medal")), _ => "Bronze", _ => ga.s(s, "medal")),
    griffinwing.Session.purse(s), Bool.not(ga.b(s, "settled")),
    ga.ids(J.get(s, "seats")), gw.ifu(J.is_null(J.get(s, "done")), _ => 0, _ => J.u32(J.get(s, "done"))))

# for id in the seats that sat before the Wyrm fell: pay once.
def griffinwing.Run.settle_griffinwing.pay(+run: J.Val, +s: J.Val, +medal: String, +purse: U32, +first: Bool, +ids: List<&2, U32>, +flown: U32) -> J.Val:
  match ids:
    case Nil{}:
      griffinwing.Run.settle_griffinwing.first(run, s, medal, first)
    case Con{+id, +t}:
      gw.ifj(Bool.and(U32.is_le(ga.u(ga.at(J.get(s, "seats"), id), "joined"), flown), U32.is_zero(U32.and(ga.u(s, "paid"), griffinwing.bit(id)))),
        _ => griffinwing.Run.settle_griffinwing.pay(
          griffinwing.Run.settle_griffinwing.hero(run, id, medal, purse),
          ga.su(s, "paid", U32.or(ga.u(s, "paid"), griffinwing.bit(id))),
          medal, purse, first, t, flown),
        _ => griffinwing.Run.settle_griffinwing.pay(run, s, medal, purse, first, t, flown))

# hero.carried.add(Gold, purse); hero.laurel = medal == Gold
def griffinwing.Run.settle_griffinwing.hero(+run: J.Val, +id: U32, +medal: String, +purse: U32) -> J.Val:
  gw.ifj(ga.has_key(J.get(run, "players"), id),
    _ => ga.sv(run, "players", ga.put_at(J.get(run, "players"), id,
      ga.sb(
        ga.sv(ga.at(J.get(run, "players"), id), "carried", realm.Spoils.add(ga.v(ga.at(J.get(run, "players"), id), "carried"), "gold", purse)),
        "laurel", String.eq(medal, "Gold")))),
    _ => run)

def griffinwing.Run.settle_griffinwing.first(+run: J.Val, +s: J.Val, +medal: String, +first: Bool) -> J.Val:
  gw.ifj(first,
    _ => gw.pk(griffinwing.Run.settle_griffinwing.begun(run, s, medal), ga.sb(s, "settled", True{})),
    _ => gw.pk(run, s))

# The news and the Home's marks: once a run.
def griffinwing.Run.settle_griffinwing.begun(+run: J.Val, +s: J.Val, +medal: String) -> J.Val:
  griffinwing.Run.settle_griffinwing.begun2(
    gw.ifj(String.eq(medal, "Gold"),
      _ => griffinwing.Run.put_flags(ga.sv(run, "marks", ga.su(ga.su(J.get(run, "marks"), griffinwing.CAMPAIGN_MARK(), 1), griffinwing.GOLD_MARK(), 1)),
        U32.or(griffinwing.BEGUN(), griffinwing.GOLD())),
      _ => griffinwing.Run.put_flags(ga.sv(run, "marks", ga.su(J.get(run, "marks"), griffinwing.CAMPAIGN_MARK(), 1)), griffinwing.BEGUN())),
    s, medal)

def griffinwing.Run.settle_griffinwing.begun2(+run: J.Val, +s: J.Val, +medal: String) -> J.Val:
  griffinwing.Run.settle_griffinwing.begun3(run, medal,
    ga.su(
      ga.su(griffinwing.Run.record(run), "wins", U32.min(ga.sadd32(ga.u(griffinwing.Run.record(run), "wins"), 1), ga.u(griffinwing.Run.record(run), "plays"))),
      "best", U32.max(ga.u(griffinwing.Run.record(run), "best"), griffinwing.Session.permille(s))))

def griffinwing.Run.settle_griffinwing.begun3(+run: J.Val, +medal: String, +rec: J.Val) -> J.Val:
  ga.push(
    griffinwing.Run.sound(
      griffinwing.Run.found(griffinwing.Run.put_record(run, rec), griffinwing.NAME() ++ ": " ++ griffinwing.types.Medal.name(medal) ++ " medal. The last beacon is lit!"),
      "play_card"),
    "cues", J.JS{"pastime_won"})

# ─── ADVENTURE ──────────────────────────────────────────────────────────────

# Whether its state, if any, is one a run may hold.
def griffinwing.ADVENTURE.valid(+run: J.Val) -> Bool:
  gw.ifb(J.is_null(J.get(run, "griffinwing")), _ => True{}, _ => griffinwing.Session.valid(J.get(run, "griffinwing")))

# End its run: the state cleared.
def griffinwing.ADVENTURE.clear(+run: J.Val) -> J.Val:
  J.del(run, "griffinwing")

# The knights its run seats.
def griffinwing.ADVENTURE.seated(+run: J.Val) -> J.Val:
  J.JA{griffinwing.ADVENTURE.seated.ids(ga.ids(J.get(J.get(run, "griffinwing"), "seats")))}

def griffinwing.ADVENTURE.seated.ids(+ids: List<&2, U32>) -> List<&2, J.Val>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      Con{J.JU{id}, griffinwing.ADVENTURE.seated.ids(t)}

# What a bot presses for knight `id`.
def griffinwing.ADVENTURE.bot(+run: J.Val, +id: U32) -> J.Val:
  gw.ifj(Bool.and(Bool.not(J.is_null(J.get(run, "griffinwing"))), ga.has_key(J.get(run, "players"), id)),
    _ => griffinwing.bot.Session.bot_with(J.get(run, "griffinwing"), id, griffinwing.Nums.of(J.get(run, "tune")), False{}),
    _ => rules.Input.default())

# ─── ACTIVITY ───────────────────────────────────────────────────────────────

def griffinwing.ACTIVITY.on(+run: J.Val) -> Bool:
  griffinwing.Run.griffinwing_showing(run)

# A ride's tick: it plays, then gives the keys the rest of the tick sees (an
# empty set: it took them all). {"run", "inputs"}.
def griffinwing.ACTIVITY.ride(+run: J.Val, +inputs: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "run", griffinwing.Run.tick_griffinwing(run, inputs)), "inputs", J.obj())

# It is over: its state cleared.
def griffinwing.ACTIVITY.end(+run: J.Val) -> J.Val:
  J.del(run, "griffinwing")

# What a finished run pays into the realm at the save, once: the marks become
# flags in the Home. `realm` is {"home": {"roundtable": [..]}}; answers
# {"realm", "settled": {"changed", "said"}}.
def griffinwing.settle(+realm: J.Val, +marks: J.Val) -> J.Val:
  griffinwing.settle.flags(realm,
    U32.or(gw.ifu(J.has(marks, griffinwing.CAMPAIGN_MARK()), _ => griffinwing.BEGUN(), _ => 0),
      gw.ifu(J.has(marks, griffinwing.GOLD_MARK()), _ => griffinwing.GOLD(), _ => 0)))

def griffinwing.settle.flags(+realm: J.Val, +flags: U32) -> J.Val:
  griffinwing.settle.done(realm, flags,
    U32.is_ne(U32.and(flags, U32.not(ga.nu(J.get(J.get(realm, "home"), "roundtable"), griffinwing.SLOT()))), 0))

def griffinwing.settle.done(+realm: J.Val, +flags: U32, +changed: Bool) -> J.Val:
  ga.sv(
    ga.sv(J.obj(), "realm",
      gw.ifj(U32.is_zero(flags),
        _ => realm,
        _ => ga.sv(realm, "home", griffinwing.Run.put_flags.home(J.get(realm, "home"), griffinwing.Run.put_flags.words(J.get(J.get(realm, "home"), "roundtable"), flags))))),
    "settled",
    ga.sv(ga.sb(J.obj(), "changed", changed), "said",
      gw.ifj(changed,
        _ => J.JA{[J.JA{[J.JS{"griffinwing"}, J.JS{griffinwing.NAME() ++ ": the last beacon is lit."}]}]},
        _ => J.JA{[]})))

# The shell's `showing` and `start` entries are the Run's methods themselves.
def griffinwing.ADVENTURE.showing(+run: J.Val) -> Bool:
  griffinwing.Run.griffinwing_showing(run)

def griffinwing.ADVENTURE.start(+run: J.Val) -> J.Val:
  griffinwing.Run.start_griffinwing(run)
