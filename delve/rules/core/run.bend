# crates/delve/src/rules.rs — the Run's small helpers. `run` is the checkpoint's
# `run` object (the serde shape of `Run`).

# The seat `id`'s knight, or JNull; a seat's knight put back.
def core.hero(+run: J.Val, +id: U32) -> J.Val:
  J.get(J.get(run, "players"), U32.show(id))

def core.set_hero(+run: J.Val, +id: U32, +hero: J.Val) -> J.Val:
  J.set(run, "players", J.set(J.get(run, "players"), U32.show(id), hero))

# `Phase`, spelled snake_case.
def core.phase_is(+run: J.Val, +name: String) -> Bool:
  String.eq(J.str(J.get(run, "phase")), name)

def rules.Run.active(+run: J.Val) -> Bool:
  Bool.or(core.phase_is(run, "fighting"), core.phase_is(run, "exploring"))

# again.rs: the run has fallen or is won.
def again.Run.over(+run: J.Val) -> Bool:
  Bool.or(core.phase_is(run, "won"), core.phase_is(run, "wiped"))

def rules.Run.floor(+run: J.Val) -> U32:
  core.u(J.get(run, "dungeon"), "depth")

# `self.dungeon.rooms[self.at]` (JNull past the end; Rust would panic).
def rules.Run.room(+run: J.Val) -> J.Val:
  J.nth(J.get(J.get(run, "dungeon"), "rooms"), core.u(run, "at"))

def rules.Run.at_entrance(+run: J.Val) -> Bool:
  rules.Run.at_entrance.go(core.u(J.get(run, "dungeon"), "depth"), core.u(run, "at"))

def rules.Run.at_entrance.go(+depth: U32, +at: U32) -> Bool:
  Bool.or(U32.is_zero(depth), Bool.and(U32.is_eq(depth, 1), U32.is_zero(at)))

# Something in the room still fights (the yard's quintains never do).
def rules.Run.foes_left(+run: J.Val) -> Bool:
  rules.Run.foes_left.go(J.items(J.get(run, "enemies")))

def rules.Run.foes_left.go(+es: List<&2, J.Val>) -> Bool:
  match es:
    case Nil{}:
      False{}
    case Con{+e, +t}:
      Bool.or(Bool.not(String.eq(J.str(J.get(e, "kind")), "dummy")), rules.Run.foes_left.go(t))

# The knights who count as the party: every one whose player is here.
def rules.Run.party_len(+run: J.Val) -> U32:
  rules.Run.party_len.go(J.int_fields(J.get(run, "players")), 0)

def rules.Run.party_len.go(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +n: U32) -> U32:
  match hs:
    case Nil{}:
      n
    case Con{(+k, +h), +t}:
      rules.Run.party_len.go(t, Bool.pick(U32, Bool.and(Bool.not(core.b(h, "away")), Bool.not(core.b(h, "absent"))), (n + 1 : U32), n))

# How many knights the floors are laid out for: every seat with a knight in it.
def rules.Run.party_size(+run: J.Val) -> U32:
  rules.Run.party_size.go(J.int_fields(J.get(run, "players")), 0)

def rules.Run.party_size.go(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +n: U32) -> U32:
  match hs:
    case Nil{}:
      n
    case Con{(+k, +h), +t}:
      rules.Run.party_size.go(t, Bool.pick(U32, Bool.not(core.b(h, "absent")), (n + 1 : U32), n))

# The party size the monsters stand scaled for.
def rules.Run.scaled(+run: J.Val) -> U32:
  rules.Run.scaled.go(core.u(run, "scaled_for"), core.count(J.get(run, "players")))

def rules.Run.scaled.go(+scaled_for: U32, +n: U32) -> U32:
  Bool.pick(U32, U32.is_zero(scaled_for), U32.max(n, 1), scaled_for)

# Whether knight `id` is away: its player is not here, or has pressed nothing
# for `AWAY_TICKS`.
def rules.Run.away(+run: J.Val, +id: U32) -> Bool:
  rules.Run.away.go(J.get(J.get(run, "players"), U32.show(id)), J.get(J.get(run, "idle"), U32.show(id)))

def rules.Run.away.go(+hero: J.Val, +idle: J.Val) -> Bool:
  Bool.or(core.b(hero, "away"), Bool.and(Bool.not(J.is_null(idle)), U32.is_ge(J.u32(idle), rules.AWAY_TICKS())))
