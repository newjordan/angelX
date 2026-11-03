# crates/delve/src/rules/griffinwing.rs — the session's tick and what ends a
# beat: muster, step, tick_lists, end_beat, next_beat, restart, finish, the
# medal and the purse, valid and repair, and `Session::new`.

def griffinwing.Session.Mode.is(+s: J.Val, +name: String) -> Bool:
  gw.isn(s, "mode", name)

# A fresh session for the knights at the table.
def griffinwing.Session.new(+players: J.Val) -> J.Val:
  griffinwing.Session.fill_wing(
    griffinwing.Session.new.sit(griffinwing.Session.new.empty(), players, ga.ids(players)))

def griffinwing.Session.new.empty() -> J.Val:
  +s0 = ga.sv(J.obj(), "seats", J.obj())
  +s1 = ga.su(s0, "elapsed", 0)
  +s2 = ga.ss(s1, "mode", "Play")
  +s3 = ga.su(s2, "mode_t", 0)
  +s4 = ga.su(s3, "stage", 0)
  +s5 = ga.su(s4, "t", 0)
  +s6 = ga.su(s5, "cp", 0)
  +s7 = ga.su(s6, "next", 0)
  +s8 = ga.sv(s7, "foes", J.JA{[]})
  +s9 = ga.sv(s8, "props", J.JA{[]})
  +s10 = ga.sv(s9, "bolts", J.JA{[]})
  +s11 = ga.sv(s10, "wing", J.JA{[]})
  +s12 = ga.sv(s11, "boss", J.JNull{})
  +s13 = ga.sv(s12, "calls", J.JA{[]})
  +s14 = ga.sv(s13, "fx", J.JA{[]})
  +s15 = ga.su(s14, "ids", 1)
  +s16 = ga.sv(s15, "bank", griffinwing.types.Tally.default())
  +s17 = ga.sv(s16, "beat", griffinwing.types.Tally.default())
  +s18 = ga.su(s17, "retries", 0)
  +s19 = ga.sv(s18, "medal", J.JNull{})
  +s20 = ga.su(s19, "paid", 0)
  +s21 = ga.sb(s20, "settled", False{})
  +s22 = ga.sv(s21, "done", J.JNull{})
  +s23 = ga.si(s22, "home", 0)
  +s24 = ga.sb(s23, "first", True{})
  +s25 = ga.sv(s24, "snd", J.JA{[]})
  ga.sv(s25, "leave", J.JA{[]})

# `for (&id, _) in players.iter().filter(|(_, h)| !h.away && !h.stone) { s.sit(id, 0) }`
def griffinwing.Session.new.sit(+s: J.Val, +players: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.new.sit(
        gw.ifj(griffinwing.Session.here(ga.at(players, id)), _ => griffinwing.Session.sit(s, id, 0), _ => s),
        players, t)

# `!h.away && !h.stone`
def griffinwing.Session.here(+h: J.Val) -> Bool:
  Bool.and(Bool.not(ga.b(h, "away")), Bool.not(ga.b(h, "stone")))

# Who is at the table now (`present`), seated and stood up, and the foes'
# health provisioned for the seats now flying: a knight who sits down scales
# the beat up by its share, and a seat already provisioned does not scale twice.
def griffinwing.Session.muster(+s0: J.Val, +heroes: J.Val) -> J.Val:
  griffinwing.Session.fill_wing(
    griffinwing.Session.muster.foes(
      griffinwing.Session.muster.sit(
        ga.sv(s0, "seats", griffinwing.Session.muster.keep(J.get(s0, "seats"), heroes, ga.ids(J.get(s0, "seats")))),
        heroes, ga.ids(heroes)),
      0))

# seats.retain(|id, _| heroes.get(id).is_some_and(|h| !h.away && !h.stone))
def griffinwing.Session.muster.keep(+seats: J.Val, +heroes: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      seats
    case Con{+id, +t}:
      griffinwing.Session.muster.keep(
        gw.ifj(Bool.and(ga.has_key(heroes, id), griffinwing.Session.here(ga.at(heroes, id))), _ => seats, _ => J.del(seats, U32.show(id))),
        heroes, t)

# for id in here { if !seats.contains(id) && (1..=MAX_PLAYERS).contains(id) { sit; join call } }
def griffinwing.Session.muster.sit(+s: J.Val, +heroes: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.muster.sit(
        gw.ifj(Bool.and(griffinwing.Session.here(ga.at(heroes, id)),
            Bool.and(Bool.not(ga.has_key(J.get(s, "seats"), id)), gw.within(id, 1, rules.MAX_PLAYERS()))),
          _ => griffinwing.Session.muster.join(griffinwing.Session.sit(s, id, ga.u(s, "elapsed")), id),
          _ => s),
        heroes, t)

def griffinwing.Session.muster.join(+s: J.Val, +id: U32) -> J.Val:
  gw.ifj(U32.is_gt(ga.u(s, "elapsed"), 0),
    _ => griffinwing.Session.sound(griffinwing.Session.say(s, id, "Join"), "play_card"),
    _ => s)

# for f in foes { if f.seats < now { hp = hp * now / was; full = full * now / was; seats = now } }
def griffinwing.Session.muster.foes(+s: J.Val, +i: U32) -> J.Val:
  gw.ifj(U32.is_lt(i, gw.len(s, "foes")),
    _ => griffinwing.Session.muster.foes(
      gw.put_item(s, "foes", i, griffinwing.Session.muster.foe(gw.item(s, "foes", i), griffinwing.Session.pressure(s))),
      (i + 1 : U32)),
    _ => s)

def griffinwing.Session.muster.foe(+f: J.Val, +now: U32) -> J.Val:
  gw.ifj(U32.is_lt(ga.u(f, "seats"), now),
    _ => griffinwing.Session.muster.foe2(f, now, U32.max(ga.u(f, "seats"), 1)),
    _ => f)

def griffinwing.Session.muster.foe2(+f: J.Val, +now: U32, +was: U32) -> J.Val:
  ga.su(ga.su(ga.su(f, "hp", U32.div((ga.u(f, "hp") * now : U32), was)), "full", U32.div((ga.u(f, "full") * now : U32), was)), "seats", now)

# The keys of knight `id`: what was pressed if it is a valid Input, else nothing.
def griffinwing.Session.keys(+inputs: J.Val, +id: U32) -> J.Val:
  griffinwing.Session.keys.of(J.get(inputs, U32.show(id)))

def griffinwing.Session.keys.of(+i: J.Val) -> J.Val:
  gw.ifj(Bool.and(Bool.not(J.is_null(i)), rules.Input.valid(i)), _ => i, _ => rules.Input.default())

# One tick: the seats are whoever is here and awake, so a knight who leaves
# stands up and one who joins late sits down; then the beat. Answers
# {"s", "out": {"sounds", "leave"}}.
def griffinwing.Session.step(+s0: J.Val, +inputs: J.Val, +heroes: J.Val, +n: J.Val) -> J.Val:
  griffinwing.Session.step.mode(
    griffinwing.Session.muster(ga.su(s0, "elapsed", U32.min((ga.u(s0, "elapsed") + 1 : U32), griffinwing.LIMIT())), heroes),
    inputs, n)

def griffinwing.Session.step.mode(+s: J.Val, +inputs: J.Val, +n: J.Val) -> J.Val:
  griffinwing.Session.step.done(
    gw.ifj(griffinwing.Session.Mode.is(s, "Play"),
      _ => griffinwing.Session.step.play(s, inputs, n, ga.ids(J.get(s, "seats"))),
      _ => gw.ifj(griffinwing.Session.Mode.is(s, "Retry"),
        _ => griffinwing.Session.step.retry(s),
        _ => griffinwing.Session.step.fly(s, inputs, n, ga.ids(J.get(s, "seats"))))),
    inputs)

# for id in live { self.fly(id, keys(id), nums) }
def griffinwing.Session.step.flies(+s: J.Val, +inputs: J.Val, +n: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.step.flies(griffinwing.flight.Session.fly(s, id, griffinwing.Session.keys(inputs, id), n), inputs, n, t)

def griffinwing.Session.step.play(+s0: J.Val, +inputs: J.Val, +n: J.Val, +live: List<&2, U32>) -> J.Val:
  griffinwing.Session.end_beat(
    griffinwing.Session.tick_lists(
      griffinwing.flight.Session.step_craft(
        griffinwing.things.Session.step_props(
          griffinwing.combat.Session.step_bolts(
            griffinwing.boss.Session.step_boss(
              griffinwing.combat.Session.step_foes(
                griffinwing.script.Session.run_script(
                  griffinwing.flight.Session.wing_fly(
                    griffinwing.Session.step.flies(ga.su(s0, "t", U32.min((ga.u(s0, "t") + 1 : U32), griffinwing.LIMIT())), inputs, n, live),
                    n),
                  n),
                n),
              n),
            n),
          n),
        n)))

def griffinwing.Session.step.retry(+s0: J.Val) -> J.Val:
  griffinwing.Session.step.retry2(ga.su(griffinwing.Session.tick_lists(s0), "mode_t", ga.ssub(ga.u(s0, "mode_t"), 1)))

def griffinwing.Session.step.retry2(+s: J.Val) -> J.Val:
  gw.ifj(U32.is_zero(ga.u(s, "mode_t")), _ => griffinwing.Session.restart(s), _ => s)

# The rail carries them home; at the tally the wing hovers.
def griffinwing.Session.step.fly(+s0: J.Val, +inputs: J.Val, +n: J.Val, +live: List<&2, U32>) -> J.Val:
  griffinwing.Session.step.fly2(
    griffinwing.Session.tick_lists(
      griffinwing.flight.Session.step_craft(
        griffinwing.things.Session.step_props(
          griffinwing.combat.Session.step_bolts(
            griffinwing.flight.Session.wing_fly(
              griffinwing.Session.step.flies(
                gw.ifj(griffinwing.Session.Mode.is(s0, "Victory"), _ => ga.su(s0, "t", U32.min((ga.u(s0, "t") + 1 : U32), griffinwing.LIMIT())), _ => s0),
                inputs, n, live),
              n),
            n),
          n),
        n)),
    inputs, live)

def griffinwing.Session.step.fly2(+s0: J.Val, +inputs: J.Val, +live: List<&2, U32>) -> J.Val:
  griffinwing.Session.step.fly3(ga.su(s0, "mode_t", U32.min((ga.u(s0, "mode_t") + 1 : U32), griffinwing.LIMIT())), inputs, live)

def griffinwing.Session.step.fly3(+s: J.Val, +inputs: J.Val, +live: List<&2, U32>) -> J.Val:
  gw.ifj(Bool.and(griffinwing.Session.Mode.is(s, "Victory"), U32.is_ge(ga.u(s, "mode_t"), griffinwing.VICTORY())),
    _ => griffinwing.Session.finish(s),
    _ => gw.ifj(Bool.and(griffinwing.Session.Mode.is(s, "Tally"), U32.is_gt(ga.u(s, "mode_t"), griffinwing.TALLY_HOLD())),
      _ => griffinwing.Session.step.leaves(s, inputs, live),
      _ => s))

# for id in live { fresh = spur key not held last tick; if keys(id).interact && fresh { leave.push(id) } }
def griffinwing.Session.step.leaves(+s: J.Val, +inputs: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.step.leaves(
        gw.ifj(Bool.and(
            ga.b(griffinwing.Session.keys(inputs, id), "interact"),
            Bool.and(gw.seat_has(s, id), U32.is_zero(U32.and(ga.u(gw.craft_of(s, id), "keys"), griffinwing.flight.K_SPUR())))),
          _ => ga.push(s, "leave", J.JU{id}),
          _ => s),
        inputs, t)

# The keys of this tick stand for the next one's presses; the queues are drained.
def griffinwing.Session.step.done(+s: J.Val, +inputs: J.Val) -> J.Val:
  griffinwing.Session.step.out(griffinwing.Session.step.keys(s, inputs, ga.ids(J.get(s, "seats"))))

def griffinwing.Session.step.keys(+s: J.Val, +inputs: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.step.keys(
        gw.put_craft(s, id, ga.su(gw.craft_of(s, id), "keys", griffinwing.flight.bits(griffinwing.Session.keys(inputs, id)))),
        inputs, t)

def griffinwing.Session.step.out(+s: J.Val) -> J.Val:
  ga.sv(
    ga.sv(J.obj(), "s", ga.sv(ga.sv(s, "snd", J.JA{[]}), "leave", J.JA{[]})),
    "out", ga.sv(ga.sv(J.obj(), "sounds", J.get(s, "snd")), "leave", J.get(s, "leave")))

# The calls, flashes and counters wind down.
def griffinwing.Session.tick_lists(+s: J.Val) -> J.Val:
  ga.sv(
    ga.sv(
      ga.sv(s, "calls", J.JA{griffinwing.Session.tick_lists.wind(ga.items(s, "calls"), False{})}),
      "fx", J.JA{griffinwing.Session.tick_lists.wind(ga.items(s, "fx"), False{})}),
    "foes", J.JA{griffinwing.Session.tick_lists.flash(ga.items(s, "foes"))})

# Each item's ticks one less; those at nothing are dropped.
def griffinwing.Session.tick_lists.wind(+xs: List<&2, J.Val>, +unused: Bool) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+x, +t}:
      gw.ifjl(U32.is_gt(ga.ssub(ga.u(x, "ticks"), 1), 0),
        _ => Con{ga.su(x, "ticks", ga.ssub(ga.u(x, "ticks"), 1)), griffinwing.Session.tick_lists.wind(t, False{})},
        _ => griffinwing.Session.tick_lists.wind(t, False{}))

def griffinwing.Session.tick_lists.flash(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Con{ga.su(f, "flash", ga.ssub(ga.u(f, "flash"), 1)), griffinwing.Session.tick_lists.flash(t)}

# `!seats.is_empty() && seats.values().all(|s| s.craft.down > 0)`
def griffinwing.Session.wiped(+s: J.Val, +ids: List<&2, U32>, +any: Bool) -> Bool:
  match ids:
    case Nil{}:
      any
    case Con{+id, +t}:
      gw.ifb(U32.is_gt(ga.u(gw.craft_of(s, id), "down"), 0), _ => griffinwing.Session.wiped(s, t, True{}), _ => False{})

# The beat's end, a wipe, or the Wyrm's fall.
def griffinwing.Session.end_beat(+s: J.Val) -> J.Val:
  gw.ifj(griffinwing.Session.wiped(s, ga.ids(J.get(s, "seats")), False{}),
    _ => griffinwing.Session.sound(ga.su(ga.ss(s, "mode", "Retry"), "mode_t", griffinwing.RETRY()), "knight_fall"),
    _ => gw.ifj(Bool.and(U32.is_lt(ga.u(s, "stage"), 2), U32.is_ge(ga.u(s, "t"), griffinwing.rail.LEN(ga.u(s, "stage")))),
      _ => griffinwing.Session.next_beat(s),
      _ => gw.ifj(Bool.and(U32.is_eq(ga.u(s, "stage"), 2), griffinwing.Session.dying(J.get(s, "boss"))),
        _ => griffinwing.Session.victory(s),
        _ => s)))

# `boss.is_some_and(|b| b.st == Dying && b.ticks == 0)`
def griffinwing.Session.dying(+b: J.Val) -> Bool:
  Bool.and(Bool.not(J.is_null(b)), Bool.and(gw.isn(b, "st", "Dying"), U32.is_zero(ga.u(b, "ticks"))))

# The Wyrm is down: the warm fly-through.
def griffinwing.Session.victory(+s0: J.Val) -> J.Val:
  griffinwing.script.Session.victory_gates(
    griffinwing.Session.sound(
      griffinwing.Session.say(
        ga.sv(ga.sv(
          griffinwing.Session.victory.beat(
            ga.su(ga.sv(ga.ss(s0, "mode", "Victory"), "done", J.JU{ga.u(s0, "elapsed")}), "mode_t", 0)),
          "foes", J.JA{[]}), "bolts", J.JA{[]}),
        griffinwing.Session.voice(s0), "Home"),
      "boss_death"))

def griffinwing.Session.victory.beat(+s: J.Val) -> J.Val:
  ga.si(
    ga.sv(s, "beat", gw.addu(gw.addu(gw.addu(J.get(s, "beat"), "pts", 1000), "base", 1000), "offered", 1000)),
    "home", griffinwing.Session.anchor(s))

# The next beat flies on from where this one ends, the party mended a little and
# a bomb richer; what the beat made is banked.
def griffinwing.Session.next_beat(+s0: J.Val) -> J.Val:
  griffinwing.Session.next_beat.seats(
    griffinwing.Session.sound(
      griffinwing.Session.say(
        ga.su(ga.su(ga.su(ga.su(
          ga.sv(ga.sv(s0, "bank", griffinwing.types.Tally.bank(J.get(s0, "bank"), J.get(s0, "beat"))), "beat", griffinwing.types.Tally.default()),
          "stage", (ga.u(s0, "stage") + 1 : U32)), "t", 0), "cp", 0), "next", 0),
        griffinwing.Session.voice(s0), gw.ifs(U32.is_eq((ga.u(s0, "stage") + 1 : U32), 1), _ => "Ruins", _ => "Roost")),
      "play_card"),
    ga.ids(J.get(s0, "seats")), 0)

# seats: shield +40, a bomb, the meter full; the wing mended and back.
def griffinwing.Session.next_beat.seats(+s: J.Val, +ids: List<&2, U32>, +k: U32) -> J.Val:
  match ids:
    case Nil{}:
      griffinwing.Session.next_beat.wing(s, 0)
    case Con{+id, +t}:
      griffinwing.Session.next_beat.seats(
        gw.put_craft(s, id,
          ga.sb(ga.si(ga.su(ga.si(gw.craft_of(s, id), "shield", gw.min((ga.u(gw.craft_of(s, id), "shield") + 40 : U32), griffinwing.types.SHIELD())),
              "bombs", U32.min((ga.u(gw.craft_of(s, id), "bombs") + 1 : U32), 3)), "meter", griffinwing.types.METER()), "spent", False{})),
        t, k)

def griffinwing.Session.next_beat.wing(+s: J.Val, +k: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, gw.len(s, "wing")),
    _ => griffinwing.Session.next_beat.wing(
      gw.put_item(s, "wing", k,
        ga.sv(ga.sb(gw.item(s, "wing", k), "lost", False{}), "craft",
          ga.si(J.get(gw.item(s, "wing", k), "craft"), "shield", gw.min((ga.u(J.get(gw.item(s, "wing", k), "craft"), "shield") + 40 : U32), griffinwing.types.SHIELD())))),
      (k + 1 : U32)),
    _ => s)

# A beat starts over from its checkpoint: the foes the beat had are gone and its
# script runs again from there; what was banked stays, as do the Wyrm's lamps
# already out. Everyone is up, mended and unhurt for a moment.
def griffinwing.Session.restart(+s0: J.Val) -> J.Val:
  griffinwing.Session.restart.seats(
    griffinwing.Session.restart.clear(s0), ga.ids(J.get(s0, "seats")))

def griffinwing.Session.restart.clear(+s0: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(
    ga.sv(
      ga.su(ga.su(ga.su(s0, "retries", U32.min((ga.u(s0, "retries") + 1 : U32), 255)), "t", ga.u(s0, "cp")), "next", griffinwing.script.resume(ga.u(s0, "stage"), ga.u(s0, "cp"))),
      "beat", griffinwing.types.Tally.default()),
    "props", J.JA{[]}), "bolts", J.JA{[]}), "fx", J.JA{[]}), "calls", J.JA{[]}),
    "foes", J.JA{griffinwing.Session.restart.lamps(ga.items(s0, "foes"))})

# foes.retain(|f| f.kind == Lamp)
def griffinwing.Session.restart.lamps(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      gw.ifjl(gw.isn(f, "kind", "Lamp"), _ => Con{f, griffinwing.Session.restart.lamps(t)}, _ => griffinwing.Session.restart.lamps(t))

def griffinwing.Session.restart.seats(+s: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      griffinwing.Session.restart.fin(griffinwing.Session.restart.wing(griffinwing.Session.restart.boss(s), 0))
    case Con{+id, +t}:
      griffinwing.Session.restart.seats(
        gw.put_craft(s, id, griffinwing.Session.restart.craft(gw.craft_of(s, id), id)),
        t)

# A new craft at the slot, with the progress kept and at least two bombs.
def griffinwing.Session.restart.craft(+old: J.Val, +id: U32) -> J.Val:
  ga.su(ga.su(ga.su(ga.su(ga.su(
    ga.su(griffinwing.types.Craft.new(griffinwing.flight.slot.x(id), griffinwing.flight.slot.z(id)), "hurt", (2 * rules.HZ() : U32)),
    "bombs", U32.max(ga.u(old, "bombs"), 2)),
    "pts", ga.u(old, "pts")), "hits", ga.u(old, "hits")), "saved", ga.u(old, "saved")), "gates", ga.u(old, "gates"))

def griffinwing.Session.restart.boss(+s: J.Val) -> J.Val:
  gw.ifj(J.is_null(J.get(s, "boss")),
    _ => s,
    _ => ga.sv(s, "boss", ga.sv(ga.su(gw.sets(J.get(s, "boss"), "st", "Idle"), "ticks", (2 * rules.HZ() : U32)), "marks", J.JA{[]})))

def griffinwing.Session.restart.wing(+s: J.Val, +k: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, gw.len(s, "wing")),
    _ => griffinwing.Session.restart.wing(
      gw.put_item(s, "wing", k,
        ga.sv(ga.sb(gw.item(s, "wing", k), "lost", False{}), "craft",
          ga.su(ga.si(J.get(gw.item(s, "wing", k), "craft"), "shield", griffinwing.types.SHIELD()), "down", 0))),
      (k + 1 : U32)),
    _ => s)

def griffinwing.Session.restart.fin(+s: J.Val) -> J.Val:
  griffinwing.Session.say(ga.su(ga.ss(s, "mode", "Play"), "mode_t", 0), griffinwing.Session.voice(s), "Hello")

# The Wyrm is down and the fly-through is over: the medal, and the tally.
def griffinwing.Session.finish(+s0: J.Val) -> J.Val:
  griffinwing.Session.sound(
    ga.su(ga.ss(
      ga.sv(ga.sv(
        ga.sv(s0, "bank", griffinwing.types.Tally.bank(J.get(s0, "bank"), J.get(s0, "beat"))),
        "beat", griffinwing.types.Tally.default()),
        "medal", J.JS{griffinwing.Session.medal_now(ga.sv(s0, "bank", griffinwing.types.Tally.bank(J.get(s0, "bank"), J.get(s0, "beat"))))}),
      "mode", "Tally"), "mode_t", 0),
    "play_card")

# The medal the party has earned: the share of what the beats offered that they
# took, less a little for every fall. A string: "Bronze", "Silver" or "Gold".
def griffinwing.Session.medal_now(+s: J.Val) -> String:
  griffinwing.Session.medal_now.pct(
    (griffinwing.Session.medal_now.share(ga.u(J.get(s, "bank"), "base"), ga.u(J.get(s, "bank"), "offered")) - (5 * ga.u(s, "retries") : U32) - (4 * ga.u(J.get(s, "bank"), "lost") : U32) : U32))

def griffinwing.Session.medal_now.share(+base: U32, +offered: U32) -> U32:
  W.lo32(W.div(W.mul(W.of_u32(base), W.of_u32(100)), W.of_u32(U32.max(offered, 1))))

def griffinwing.Session.medal_now.pct(+pct: U32) -> String:
  gw.ifs(gw.ge(pct, 85), _ => "Gold", _ => gw.ifs(gw.ge(pct, 60), _ => "Silver", _ => "Bronze"))

# What the finish pays each knight: a purse and a share for the medal, less for
# a party that has lit the beacon before.
def griffinwing.Session.purse(+s: J.Val) -> U32:
  griffinwing.Session.purse.of(ga.b(s, "first"), gw.ifu(J.is_null(J.get(s, "medal")), _ => 0, _ => griffinwing.types.Medal.rank(ga.s(s, "medal"))))

def griffinwing.Session.purse.of(+first: Bool, +rank: U32) -> U32:
  gw.ifu(first,
    _ => (griffinwing.PURSE() + (griffinwing.PURSE_MEDAL() * rank : U32) : U32),
    _ => (griffinwing.REPLAY() + (griffinwing.REPLAY_MEDAL() * rank : U32) : U32))

# Whether the Wyrm has fallen and the run is over.
def griffinwing.Session.finished(+s: J.Val) -> Bool:
  griffinwing.Session.Mode.is(s, "Tally")

# The share of what was offered that was taken, in tenths of a percent.
def griffinwing.Session.permille(+s: J.Val) -> U32:
  U32.min(W.lo32(W.div(W.mul(W.of_u32(ga.u(J.get(s, "bank"), "base")), W.of_u32(1000)), W.of_u32(U32.max(ga.u(J.get(s, "bank"), "offered"), 1)))), 1000)
