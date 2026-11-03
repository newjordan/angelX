# Hand-written Bend for the Run's hooks (bannerpeak.rs): the Run's records and fields as
# the J tree holds them.

def bannerpeak.u.room_kind(+run: J.Val) -> String:
  J.str(J.get(rules.Run.room(run), "kind"))

# `found = Some((tick, 0, text))`: the line the party reads.
def bannerpeak.u.found(+tick: J.Val, +text: String) -> J.Val:
  J.JA{[tick, J.JU{0}, J.JS{text}]}

# The Run's players' sounds: `sounds.extend(say)`.
def bannerpeak.u.vec_extend(+a: J.Val, +b: J.Val) -> J.Val:
  J.JA{List.append(&2, J.Val, J.items(a), J.items(b))}

# `exits.outside.contains_key(id)`.
def bannerpeak.u.outside(+run: J.Val, +id: U32) -> Bool:
  J.has(J.get(J.get(run, "exits"), "outside"), U32.show(id))

# `Run::leave_activity(id)` without its answer: the run after.
def bannerpeak.u.leave(+run: J.Val, +id: U32) -> J.Val:
  bannerpeak.u.fst(J.Val, Bool, exits.Run.leave_activity(run, id))

# The record of this adventure: `home.countryside.games.roundtable[SLOT]`, five scores
# that are left out of the save while all are default.
def bannerpeak.u.score0() -> J.Val:
  J.set(J.set(J.set(J.obj(), "best", J.JU{0}), "wins", J.JU{0}), "plays", J.JU{0})

def bannerpeak.u.scores(+home: J.Val) -> J.Val:
  bannerpeak.u.scores.go(J.get(J.get(J.get(home, "countryside"), "games"), "roundtable"))

def bannerpeak.u.scores.go(+a: J.Val) -> J.Val:
  match a:
    case J.JNull{}:
      J.JA{[bannerpeak.u.score0(), bannerpeak.u.score0(), bannerpeak.u.score0(), bannerpeak.u.score0(), bannerpeak.u.score0()]}
    case _:
      a

def bannerpeak.u.record(+home: J.Val) -> J.Val:
  J.nth(bannerpeak.u.scores(home), 2)

def bannerpeak.u.record_put(+home: J.Val, +rec: J.Val) -> J.Val:
  J.put(home, ["countryside", "games", "roundtable"], J.set_nth(bannerpeak.u.scores(home), 2, rec))

# A play begun: `record.plays = record.plays.saturating_add(1)`.
def bannerpeak.u.record_play(+home: J.Val) -> J.Val:
  bannerpeak.u.record_put(home, ga.su(bannerpeak.u.record(home), "plays", ga.sadd32(ga.u(bannerpeak.u.record(home), "plays"), 1)))

# A win: `record.best = record.best.max(score.min(1000))`,
# `record.wins = record.wins.saturating_add(1).min(record.plays)`.
def bannerpeak.u.record_win(+home: J.Val, +score: U32) -> J.Val:
  bannerpeak.u.record_win.go(home, bannerpeak.u.record(home), score)

def bannerpeak.u.record_win.go(+home: J.Val, +rec: J.Val, +score: U32) -> J.Val:
  bannerpeak.u.record_put(home, ga.su(ga.su(rec, "best", U32.max(ga.u(rec, "best"), U32.min(score, 1000))), "wins", U32.min(ga.sadd32(ga.u(rec, "wins"), 1), ga.u(rec, "plays"))))

# `home.roundtable[SLOT]`: the flags this adventure settled (missing: none).
def bannerpeak.u.flags(+home: J.Val) -> U32:
  J.u32(J.nth(J.get(home, "roundtable"), 2))

def bannerpeak.u.flags_put(+home: J.Val, +bits: U32) -> J.Val:
  J.set(home, "roundtable", J.set_nth(bannerpeak.u.flags.arr(J.get(home, "roundtable")), 2, J.JU{bits}))

def bannerpeak.u.flags.arr(+a: J.Val) -> J.Val:
  match a:
    case J.JNull{}:
      J.JA{[J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}, J.JU{0}]}
    case _:
      a

# A hero's gold: `carried.add(Spoil::Gold, n)`.
def bannerpeak.u.pay(+h: J.Val, +n: U32) -> J.Val:
  J.set(h, "carried", realm.Spoils.add(J.get(h, "carried"), "gold", n))

def bannerpeak.u.flag_set(+marks: J.Val, +key: String) -> J.Val:
  J.set(marks, key, J.JU{1})

# Heroes not outside: `players.iter().filter(!outside)`, as an object by id.
def bannerpeak.u.present(+run: J.Val, +ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      acc
    case Con{+id, +t}:
      bannerpeak.u.present(run, t, bannerpeak.u.present.one(bannerpeak.u.outside(run, id), id, J.get(J.get(run, "players"), U32.show(id)), acc))

def bannerpeak.u.present.one(out: Bool, +id: U32, +h: J.Val, +acc: J.Val) -> J.Val:
  match out:
    case True{}:
      acc
    case False{}:
      J.set(acc, U32.show(id), h)

def bannerpeak.u.pays_line(+purse: U32) -> String:
  "Bannerpeak: eight banners! " ++ U32.show(purse) ++ " gold each. E to return."

# A vector of ids as a J array of numbers.
def bannerpeak.u.ids_vec(+ids: List<&2, U32>, +acc: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      acc
    case Con{+h, +t}:
      bannerpeak.u.ids_vec(t, J.push(acc, J.JU{h}))
