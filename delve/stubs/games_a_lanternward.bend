# Names the lanternward port calls that other groups own, until they land them.

# roundtable::number(tune, slot, i): the Tune's number, its start when the
# tune is the default (omitted from the Run).
def roundtable.number(+tune: J.Val, +slot: U32, +i: U32) -> U32:
  roundtable.number.of(J.nth(J.nth(J.get(tune, "roundtable"), slot), i), i)

def roundtable.number.of(+v: J.Val, +i: U32) -> U32:
  match v:
    case J.JNull{}:
      Bool.pick(U32, U32.is_eq(i, 0), 112, Bool.pick(U32, U32.is_eq(i, 1), 8, Bool.pick(U32, U32.is_eq(i, 2), 14, Bool.pick(U32, U32.is_eq(i, 3), 150, 56))))
    case _:
      J.u32(v)

# Input::valid / Input::default (core).
def rules.Input.valid(+i: J.Val) -> Bool:
  Bool.and(
    Bool.and(
      Bool.or(U32.is_le(J.u32(J.get(i, "move_x")), 1), U32.is_eq(J.u32(J.get(i, "move_x")), 4294967295)),
      Bool.or(U32.is_le(J.u32(J.get(i, "move_y")), 1), U32.is_eq(J.u32(J.get(i, "move_y")), 4294967295))),
    Bool.and(
      Bool.or(U32.is_le(J.u32(J.get(i, "aim_x")), 1), U32.is_eq(J.u32(J.get(i, "aim_x")), 4294967295)),
      Bool.or(U32.is_le(J.u32(J.get(i, "aim_y")), 1), U32.is_eq(J.u32(J.get(i, "aim_y")), 4294967295))))

def rules.Input.default() -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(),
    "move_x", J.JU{0}), "move_y", J.JU{0}), "aim_x", J.JU{0}), "aim_y", J.JU{0}), "fire", J.JBool{False{}}),
    "dash", J.JBool{False{}}), "bomb", J.JBool{False{}}), "swing", J.JBool{False{}}), "play", J.JU{0}), "cast", J.JU{0})

# Run::room (the room the party stands in).
def rules.Run.room(+run: J.Val) -> J.Val:
  J.nth(J.get(J.get(run, "dungeon"), "rooms"), J.u32(J.get(run, "at")))

# forest::Forest::leave
def forest.Forest.leave(+forest: J.Val) -> J.Val:
  forest

# hitstop::State::request
def hitstop.State.request(+hs: J.Val, +tune: J.Val, +impact: String) -> J.Val:
  hs

# exits: Run::leave_activity
def exits.Run.leave_activity(+run: J.Val, +id: U32) -> J.Val:
  run

# realm::Spoils::add (the Spoils is an object keyed by spoil name).
def realm.Spoils.add(+sp: J.Val, +spoil: String, +n: U32) -> J.Val:
  Bool.pick(J.Val, U32.is_gt(n, 0), J.set(sp, spoil, J.JU{RS.u_sat_add(J.u32(J.get(sp, spoil)), n)}), sp)
