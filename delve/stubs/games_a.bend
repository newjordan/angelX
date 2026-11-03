# Names other groups own that games_a (the small modules: ledge, hunters, altar,
# aerie) calls. The build keeps each only while no fragment defines it; where a
# faithful body is cheap it is given so the recorded cases can run. The owners'
# real defs replace these by defining the name.
#
# Interfaces assumed: a tile is its serde name as a J string (`layout.Room.tile`);
# a `Grid` is passed as (room, barred) as core's rules.Grid.* take it, a mover is
# 0 Hero, 1 Walker, 2 Flier, 3 Wader; Enemy, Hero, Room, Floor are their serde J
# objects; a tuple (f32, f32) is a Bend pair `F32 & F32`; `Deeds` is a J object
# with the Rust field names whose tuples are J arrays.

def rules.Run.room(+run: J.Val) -> J.Val:
  J.nth(J.get(J.get(run, "dungeon"), "rooms"), ga.u(run, "at"))

# The doors stay barred while anything in the room still lives (a stub: the
# fights, not the wave and mini-game cases).
def rules.Run.barred(+run: J.Val) -> Bool:
  rules.Run.barred.live(J.items(J.get(run, "enemies")))

def rules.Run.barred.live(+es: List<&2, J.Val>) -> Bool:
  match es:
    case Nil{}:
      False{}
    case Con{+e, +t}:
      Bool.or(Bool.not(ga.is(J.get(e, "kind"), "dummy")), rules.Run.barred.live(t))

# monsters.rs: `Enemy::radius` (the hunters' three; `size` overrides).
def monsters.Enemy.radius(+e: J.Val) -> F32:
  monsters.Enemy.radius.size(J.get(e, "size"), ga.s(e, "kind"))

def monsters.Enemy.radius.size(+size: J.Val, +kind: String) -> F32:
  monsters.Enemy.radius.if(J.is_null(size), size, kind)

def monsters.Enemy.radius.if(none: Bool, +size: J.Val, +kind: String) -> F32:
  match none:
    case False{}:
      J.f32(size)
    case True{}:
      ga.pf(String.eq(kind, "flesher"), 1.15, ga.pf(String.eq(kind, "silkmother"), 1.0, 0.45))

# peasant.rs: `Hero::targetable`.
def peasant.Hero.targetable(+h: J.Val) -> Bool:
  Bool.and(Bool.and(U32.is_gt(ga.u(h, "hp"), 0), Bool.not(ga.b(h, "stone"))), Bool.and(U32.is_zero(ga.u(h, "flee_ticks")), Bool.not(ga.b(h, "aloft"))))

# hero.rs: `Hero::hurt` (core owns the armour, curio and ward logic).
def hero.Hero.hurt(+h: J.Val, +damage: U32) -> J.Val:
  h

# feats.rs: `Run::notice` (kept once, in order).
def feats.Run.notice(+run: J.Val, +id: String) -> J.Val:
  feats.Run.notice.put(feats.Run.notice.has(J.items(J.get(run, "feats")), id), run, id)

def feats.Run.notice.has(+fs: List<&2, J.Val>, +id: String) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      Bool.or(String.eq(J.str(f), id), feats.Run.notice.has(t, id))

def feats.Run.notice.put(has: Bool, +run: J.Val, +id: String) -> J.Val:
  match has:
    case True{}:
      run
    case False{}:
      ga.push(run, "feats", J.JS{id})

# rules.rs: `Input::default()` and `Input::valid` (core).
def rules.Input.default() -> J.Val:
  ga.su(ga.su(ga.sb(ga.sb(ga.sb(ga.sb(ga.su(ga.su(ga.su(ga.su(J.obj(), "move_x", 0), "move_y", 0), "aim_x", 0), "aim_y", 0), "fire", False{}), "dash", False{}), "bomb", False{}), "swing", False{}), "play", 0), "cast", 0)

def rules.Input.valid(+i: J.Val) -> Bool:
  True{}

# overclass.rs: `Run::can_wish`.
def overclass.Run.can_wish(+run: J.Val, +id: U32) -> Bool:
  overclass.Run.can_wish.of(ga.at(J.get(run, "players"), id))

def overclass.Run.can_wish.of(+h: J.Val) -> Bool:
  Bool.and(U32.is_gt(ga.u(h, "hp"), 0), U32.is_gt(RS.as_u8((ga.u(h, "wishes") + ga.u(h, "great_wishes") : U32)), 0))

# rules.rs: `Run::party_len`.
def rules.Run.party_len(+run: J.Val) -> U32:
  rules.Run.party_len.go(J.int_fields(J.get(run, "players")), 0)

def rules.Run.party_len.go(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +n: U32) -> U32:
  match hs:
    case Nil{}:
      n
    case Con{(+k, +h), +t}:
      rules.Run.party_len.go(t, ga.pu(Bool.and(Bool.not(ga.b(h, "away")), Bool.not(ga.b(h, "absent"))), (n + 1 : U32), n))
