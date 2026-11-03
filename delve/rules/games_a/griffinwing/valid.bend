# crates/delve/src/rules/griffinwing.rs — `Session::valid` and `Session::repair`.

# A position bound: -40_000_000..=40_000_000.
def gw.wide(+v: U32) -> Bool:
  gw.far(v, 40000000)

def griffinwing.Session.valid.seats(+s: J.Val, +ids: List<&2, U32>) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+id, +t}:
      Bool.and(
        Bool.and(gw.within(id, 1, rules.MAX_PLAYERS()),
          Bool.and(U32.is_le(ga.u(ga.at(J.get(s, "seats"), id), "joined"), ga.u(s, "elapsed")), griffinwing.types.Craft.valid(gw.craft_of(s, id)))),
        griffinwing.Session.valid.seats(s, t))

def griffinwing.Session.valid.wing(+ws: List<&2, J.Val>) -> Bool:
  match ws:
    case Nil{}:
      True{}
    case Con{+w, +t}:
      Bool.and(Bool.and(gw.within(ga.u(w, "id"), 7, 10), griffinwing.types.Craft.valid(J.get(w, "craft"))), griffinwing.Session.valid.wing(t))

def griffinwing.Session.valid.calls(+cs: List<&2, J.Val>) -> Bool:
  match cs:
    case Nil{}:
      True{}
    case Con{+c, +t}:
      Bool.and(Bool.and(U32.is_le(ga.u(c, "ticks"), 200), U32.is_le(ga.u(c, "who"), 10)), griffinwing.Session.valid.calls(t))

def griffinwing.Session.valid.fx(+fs: List<&2, J.Val>) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{+f, +t}:
      Bool.and(
        Bool.and(U32.is_le(ga.u(f, "ticks"), 60), Bool.and(gw.wide(ga.u(f, "s")), Bool.and(gw.wide(ga.u(f, "x")), gw.wide(ga.u(f, "z"))))),
        griffinwing.Session.valid.fx(t))

def griffinwing.Session.valid.foes(+fs: List<&2, J.Val>) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{+f, +t}:
      Bool.and(griffinwing.Session.valid.foe(f), griffinwing.Session.valid.foes(t))

def griffinwing.Session.valid.foe(+f: J.Val) -> Bool:
  gw.alls([
    U32.is_le(ga.u(f, "hp"), ga.u(f, "full")),
    U32.is_le(ga.u(f, "full"), (griffinwing.types.Kind.health("Lamp") * rules.MAX_PLAYERS() * 2 : U32)),
    gw.within(ga.u(f, "seats"), 1, rules.MAX_PLAYERS()),
    gw.wide(ga.u(f, "s")), gw.wide(ga.u(f, "x")), gw.wide(ga.u(f, "z")),
    gw.far(ga.u(f, "vs"), 1024), gw.far(ga.u(f, "vx"), 1024), gw.far(ga.u(f, "vz"), 1024),
    U32.is_le(ga.u(f, "ticks"), 100000),
    U32.is_le(ga.u(f, "on"), 10),
    gw.wide(ga.u(f, "fx")), gw.wide(ga.u(f, "fz"))])

def griffinwing.Session.valid.props(+ps: List<&2, J.Val>) -> Bool:
  match ps:
    case Nil{}:
      True{}
    case Con{+p, +t}:
      Bool.and(
        gw.alls([
          gw.wide(ga.u(p, "s")), gw.wide(ga.u(p, "x")), gw.wide(ga.u(p, "z")),
          gw.far(ga.u(p, "w"), griffinwing.types.dm(400)), gw.far(ga.u(p, "h"), griffinwing.types.dm(400)), gw.far(ga.u(p, "len"), griffinwing.types.dm(400)),
          U32.is_lt(ga.u(p, "taken"), 64)]),
        griffinwing.Session.valid.props(t))

def griffinwing.Session.valid.bolts(+bs: List<&2, J.Val>) -> Bool:
  match bs:
    case Nil{}:
      True{}
    case Con{+b, +t}:
      Bool.and(
        gw.alls([
          U32.is_le(ga.u(b, "owner"), 10),
          gw.wide(ga.u(b, "s")), gw.wide(ga.u(b, "x")), gw.wide(ga.u(b, "z")),
          gw.far(ga.u(b, "vs"), 2048), gw.far(ga.u(b, "vx"), 2048), gw.far(ga.u(b, "vz"), 2048),
          U32.is_le(ga.u(b, "life"), 600),
          U32.is_le(ga.u(b, "dmg"), 1000)]),
        griffinwing.Session.valid.bolts(t))

# `done.is_none_or(|d| d <= elapsed)`, `boss.is_none_or(Beast::valid)`
def griffinwing.Session.valid.done(+d: J.Val, +elapsed: U32) -> Bool:
  Bool.or(J.is_null(d), U32.is_le(J.u32(d), elapsed))

def griffinwing.Session.valid.boss(+b: J.Val) -> Bool:
  gw.ifb(J.is_null(b), _ => True{}, _ => griffinwing.types.Beast.valid(b))

def griffinwing.Session.valid(+s: J.Val) -> Bool:
  gw.alls([
    U32.is_le(ga.u(s, "elapsed"), griffinwing.LIMIT()),
    U32.is_le(gw.nseats(s), rules.MAX_PLAYERS()),
    griffinwing.Session.valid.seats(s, ga.ids(J.get(s, "seats"))),
    U32.is_le(ga.u(s, "stage"), 2),
    U32.is_le(ga.u(s, "t"), griffinwing.LIMIT()),
    U32.is_le(ga.u(s, "cp"), U32.max(ga.u(s, "t"), griffinwing.rail.BOSS_AT())),
    U32.is_le(ga.u(s, "mode_t"), griffinwing.LIMIT()),
    U32.is_le(gw.len(s, "foes"), griffinwing.types.MAX_FOES()),
    U32.is_le(gw.len(s, "props"), griffinwing.types.MAX_PROPS()),
    U32.is_le(gw.len(s, "bolts"), griffinwing.types.MAX_BOLTS()),
    U32.is_le(gw.len(s, "wing"), griffinwing.types.WING()),
    U32.is_le(gw.len(s, "calls"), griffinwing.types.MAX_CALLS()),
    U32.is_le(gw.len(s, "fx"), griffinwing.types.MAX_FX()),
    griffinwing.types.Tally.valid(J.get(s, "bank")),
    griffinwing.types.Tally.valid(J.get(s, "beat")),
    U32.is_lt(ga.u(s, "paid"), 64),
    griffinwing.Session.valid.done(J.get(s, "done"), ga.u(s, "elapsed")),
    gw.ge(ga.u(s, "home"), 0),
    griffinwing.Session.valid.wing(ga.items(s, "wing")),
    griffinwing.Session.valid.boss(J.get(s, "boss")),
    griffinwing.Session.valid.calls(ga.items(s, "calls")),
    griffinwing.Session.valid.fx(ga.items(s, "fx")),
    griffinwing.Session.valid.foes(ga.items(s, "foes")),
    griffinwing.Session.valid.props(ga.items(s, "props")),
    griffinwing.Session.valid.bolts(ga.items(s, "bolts"))])

# Mend what a bad save broke.
def griffinwing.Session.repair(+s0: J.Val) -> J.Val:
  griffinwing.Session.repair.lists(
    griffinwing.Session.repair.scalars(
      griffinwing.Session.repair.seats(
        ga.su(
          ga.sv(s0, "seats", griffinwing.Session.repair.keep(J.get(s0, "seats"), ga.ids(J.get(s0, "seats")))),
          "elapsed", U32.min(ga.u(s0, "elapsed"), griffinwing.LIMIT())))))

# seats.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id))
def griffinwing.Session.repair.keep(+seats: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      seats
    case Con{+id, +t}:
      griffinwing.Session.repair.keep(gw.ifj(gw.within(id, 1, rules.MAX_PLAYERS()), _ => seats, _ => J.del(seats, U32.show(id))), t)

# for each seat: joined = joined.min(elapsed); a craft that is not valid is
# made anew at its slot.
def griffinwing.Session.repair.seats(+s: J.Val) -> J.Val:
  griffinwing.Session.repair.seats.go(s, ga.ids(J.get(s, "seats")))

def griffinwing.Session.repair.seats.go(+s: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      s
    case Con{+id, +t}:
      griffinwing.Session.repair.seats.go(
        griffinwing.Session.repair.seat(s, id, ga.at(J.get(s, "seats"), id)),
        t)

def griffinwing.Session.repair.seat(+s: J.Val, +id: U32, +seat: J.Val) -> J.Val:
  ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id,
    ga.sv(
      ga.su(seat, "joined", U32.min(ga.u(seat, "joined"), ga.u(s, "elapsed"))),
      "craft",
      gw.ifj(griffinwing.types.Craft.valid(J.get(seat, "craft")),
        _ => J.get(seat, "craft"),
        _ => griffinwing.types.Craft.new(griffinwing.flight.slot.x(id), griffinwing.flight.slot.z(id))))))

# stage, t, cp, mode_t held to their bounds.
def griffinwing.Session.repair.scalars(+s0: J.Val) -> J.Val:
  griffinwing.Session.repair.scalars.cp(ga.su(ga.su(ga.su(s0, "stage", U32.min(ga.u(s0, "stage"), 2)), "t", U32.min(ga.u(s0, "t"), griffinwing.LIMIT())), "mode_t", U32.min(ga.u(s0, "mode_t"), griffinwing.LIMIT())))

def griffinwing.Session.repair.scalars.cp(+s: J.Val) -> J.Val:
  ga.su(s, "cp", U32.min(ga.u(s, "cp"), U32.max(ga.u(s, "t"), griffinwing.rail.BOSS_AT())))

# The lists cut to their bounds, the riders and foes that cannot be kept dropped.
def griffinwing.Session.repair.lists(+s0: J.Val) -> J.Val:
  +s1 = gw.trunc(gw.trunc(gw.trunc(gw.trunc(gw.trunc(s0, "foes", griffinwing.types.MAX_FOES()), "props", griffinwing.types.MAX_PROPS()), "bolts", griffinwing.types.MAX_BOLTS()), "calls", griffinwing.types.MAX_CALLS()), "fx", griffinwing.types.MAX_FX())
  +s2 = ga.sv(s1, "wing", J.truncate(J.JA{griffinwing.Session.repair.wing(ga.items(s1, "wing"), 0)}, griffinwing.types.WING()))
  +s3 = ga.sv(s2, "foes", J.JA{griffinwing.Session.repair.foes(ga.items(s2, "foes"))})
  griffinwing.Session.repair.checks(ga.su(s3, "paid", U32.and(ga.u(s3, "paid"), 63)))

# wing.retain(|w| (7..=10).contains(&w.id) && w.craft.valid())
def griffinwing.Session.repair.wing(+ws: List<&2, J.Val>, +unused: U32) -> List<&2, J.Val>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      gw.ifjl(Bool.and(gw.within(ga.u(w, "id"), 7, 10), griffinwing.types.Craft.valid(J.get(w, "craft"))),
        _ => Con{w, griffinwing.Session.repair.wing(t, 0)},
        _ => griffinwing.Session.repair.wing(t, 0))

# foes.retain(|f| f.hp <= f.full && (1..=MAX_PLAYERS).contains(&f.seats))
def griffinwing.Session.repair.foes(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      gw.ifjl(Bool.and(U32.is_le(ga.u(f, "hp"), ga.u(f, "full")), gw.within(ga.u(f, "seats"), 1, rules.MAX_PLAYERS())),
        _ => Con{f, griffinwing.Session.repair.foes(t)},
        _ => griffinwing.Session.repair.foes(t))

# A boss that is not valid goes; a tally that is not valid is cleared; and what
# is left, if the session is still not valid, is no beat to resume.
def griffinwing.Session.repair.checks(+s0: J.Val) -> J.Val:
  griffinwing.Session.repair.last(
    ga.sv(ga.sv(
      gw.ifj(Bool.and(Bool.not(J.is_null(J.get(s0, "boss"))), Bool.not(griffinwing.types.Beast.valid(J.get(s0, "boss")))),
        _ => ga.sv(s0, "boss", J.JNull{}),
        _ => s0),
      "bank", gw.ifj(griffinwing.types.Tally.valid(J.get(s0, "bank")), _ => J.get(s0, "bank"), _ => griffinwing.types.Tally.default())),
      "beat", gw.ifj(griffinwing.types.Tally.valid(J.get(s0, "beat")), _ => J.get(s0, "beat"), _ => griffinwing.types.Tally.default())))

def griffinwing.Session.repair.last(+s: J.Val) -> J.Val:
  gw.ifj(griffinwing.Session.valid(s),
    _ => s,
    _ => griffinwing.Session.repair.reset(s))

def griffinwing.Session.repair.reset(+s: J.Val) -> J.Val:
  ga.su(ga.su(ga.su(
    ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(
      ga.sv(s, "done", gw.ifj(Bool.or(J.is_null(J.get(s, "done")), U32.is_le(J.u32(J.get(s, "done")), ga.u(s, "elapsed"))), _ => J.get(s, "done"), _ => J.JNull{})),
      "foes", J.JA{[]}), "props", J.JA{[]}), "bolts", J.JA{[]}), "fx", J.JA{[]}), "calls", J.JA{[]}), "boss", J.JNull{}),
    "t", 0), "cp", 0), "next", 0)
