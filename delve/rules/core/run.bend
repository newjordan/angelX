def core.hero(+run: J.Val, +id: U32) -> J.Val:
  J.get(J.get(run, "players"), U32.show(id))

def core.set_hero(+run: J.Val, +id: U32, +hero: J.Val) -> J.Val:
  J.set(run, "players", J.set(J.get(run, "players"), U32.show(id), hero))

def core.phase_is(+run: J.Val, +name: String) -> Bool:
  String.eq(J.str(J.get(run, "phase")), name)

def rules.Run.active(+run: J.Val) -> Bool:
  Bool.or(core.phase_is(run, "fighting"), core.phase_is(run, "exploring"))

def again.Run.over(+run: J.Val) -> Bool:
  Bool.or(core.phase_is(run, "won"), core.phase_is(run, "wiped"))

def rules.Run.floor(+run: J.Val) -> U32:
  core.u(J.get(run, "dungeon"), "depth")

def rules.Run.room(+run: J.Val) -> J.Val:
  J.nth(J.get(J.get(run, "dungeon"), "rooms"), core.u(run, "at"))

def rules.Run.at_entrance(+run: J.Val) -> Bool:
  rules.Run.at_entrance.go(core.u(J.get(run, "dungeon"), "depth"), core.u(run, "at"))

def rules.Run.at_entrance.go(+depth: U32, +at: U32) -> Bool:
  Bool.or(U32.is_zero(depth), Bool.and(U32.is_eq(depth, 1), U32.is_zero(at)))

def rules.Run.foes_left(+run: J.Val) -> Bool:
  rules.Run.foes_left.go(J.items(J.get(run, "enemies")))

def rules.Run.foes_left.go(+es: List<&2, J.Val>) -> Bool:
  match es:
    case Nil{}:
      False{}
    case Con{+e, +t}:
      Bool.or(Bool.not(String.eq(J.str(J.get(e, "kind")), "dummy")), rules.Run.foes_left.go(t))

def rules.Run.party_len(+run: J.Val) -> U32:
  rules.Run.party_len.go(J.int_fields(J.get(run, "players")), 0)

def rules.Run.party_len.go(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +n: U32) -> U32:
  match hs:
    case Nil{}:
      n
    case Con{(+k, +h), +t}:
      rules.Run.party_len.go(t, Bool.pick(U32, Bool.and(Bool.not(core.b(h, "away")), Bool.not(core.b(h, "absent"))), (n + 1 : U32), n))

def rules.Run.party_size(+run: J.Val) -> U32:
  rules.Run.party_size.go(J.int_fields(J.get(run, "players")), 0)

def rules.Run.party_size.go(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +n: U32) -> U32:
  match hs:
    case Nil{}:
      n
    case Con{(+k, +h), +t}:
      rules.Run.party_size.go(t, Bool.pick(U32, Bool.not(core.b(h, "absent")), (n + 1 : U32), n))

def rules.Run.scaled(+run: J.Val) -> U32:
  rules.Run.scaled.go(core.u(run, "scaled_for"), core.count(J.get(run, "players")))

def rules.Run.scaled.go(+scaled_for: U32, +n: U32) -> U32:
  Bool.pick(U32, U32.is_zero(scaled_for), U32.max(n, 1), scaled_for)

def rules.Run.away(+run: J.Val, +id: U32) -> Bool:
  rules.Run.away.go(J.get(J.get(run, "players"), U32.show(id)), J.get(J.get(run, "idle"), U32.show(id)))

def rules.Run.away.go(+hero: J.Val, +idle: J.Val) -> Bool:
  Bool.or(core.b(hero, "away"), Bool.and(Bool.not(J.is_null(idle)), U32.is_ge(J.u32(idle), rules.AWAY_TICKS())))
