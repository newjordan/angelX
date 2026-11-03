# Names other groups own that Griffinwing calls. A stub answers a harmless
# default and drops out of the build as soon as a fragment defines the name.
# Where a real body is cheap and faithful it is given, so the cases can run.

# rules.rs consts (core/consts.bend).
def rules.HZ() -> U32:
  30

def rules.MAX_PLAYERS() -> U32:
  6

# rules.rs `Input::default()`: nothing pressed (the skip-serialized fields are
# left out).
def rules.Input.default() -> J.Val:
  i0 = J.obj()
  i1 = J.set(i0, "move_x", J.JU{0})
  i2 = J.set(i1, "move_y", J.JU{0})
  i3 = J.set(i2, "aim_x", J.JU{0})
  i4 = J.set(i3, "aim_y", J.JU{0})
  i5 = J.set(i4, "fire", J.JBool{False{}})
  i6 = J.set(i5, "dash", J.JBool{False{}})
  i7 = J.set(i6, "bomb", J.JBool{False{}})
  i8 = J.set(i7, "swing", J.JBool{False{}})
  i9 = J.set(i8, "play", J.JU{0})
  J.set(i9, "cast", J.JU{0})

# rules.rs `Input::valid()`: the axes in -1..=1, the slots in bounds (the vote
# is not read by Griffinwing's inputs).
def rules.Input.axis_ok(+v: U32) -> Bool:
  Bool.or(U32.is_le(v, 1), U32.is_eq(v, 4294967295))

def rules.Input.valid(+i: J.Val) -> Bool:
  Bool.and(
    Bool.and(
      Bool.and(rules.Input.axis_ok(J.u32(J.get(i, "move_x"))), rules.Input.axis_ok(J.u32(J.get(i, "move_y")))),
      Bool.and(rules.Input.axis_ok(J.u32(J.get(i, "aim_x"))), rules.Input.axis_ok(J.u32(J.get(i, "aim_y"))))),
    Bool.and(U32.is_le(J.u32(J.get(i, "play")), 4), Bool.and(U32.is_le(J.u32(J.get(i, "cast")), 4), U32.is_le(J.u32(J.get(i, "skill")), 5))))

# rules.rs `Run::room`: `self.dungeon.rooms[self.at]`.
def rules.Run.room(+run: J.Val) -> J.Val:
  J.nth(J.get(J.get(run, "dungeon"), "rooms"), J.u32(J.get(run, "at")))

# forest.rs `Forest::leave`: the clearing's games stop.
def forest.Forest.leave(+f: J.Val) -> J.Val:
  f0 = J.set(f, "hunt_left", J.JU{0})
  f1 = J.set(f0, "caught", J.JU{0})
  f2 = J.set(f1, "singing", J.JBool{False{}})
  f3 = J.set(f2, "show_left", J.JU{0})
  f4 = J.set(f3, "note", J.JU{0})
  J.set(f4, "held", J.JU{0})

# realm.rs `Spoils::add`: `if n > 0 { *entry(spoil).or_insert(0) = saturating_add }`.
def realm.Spoils.add(+sp: J.Val, +spoil: String, +n: U32) -> J.Val:
  Bool.pick(J.Val, U32.is_zero(n), sp, J.set(sp, spoil, J.JU{ga.sadd32(J.u32(J.get(sp, spoil)), n)}))

# exits.rs `Run::leave_activity`: the run leaves for a knight who pressed to go.
# A stub: the Run is answered unchanged.
def exits.Run.leave_activity(+run: J.Val, +id: U32) -> J.Val:
  run
