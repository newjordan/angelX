# crates/delve/src/rules.rs — `Input`, one seat's keys for a tick. The J shape
# is serde's: unit fields are always written; the `skip_serializing_if`
# fields (leave_activity, leave_held, activity_menu, vote, latch_tick, vigil,
# ult, take, interact, skill, ride, cycle, skip_plate, again) only when set.

# `Input::default()`: nothing pressed.
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

# An axis, -1, 0 or 1 (i8 on the wire; JI for -1).
def rules.Input.axis_ok(+v: U32) -> Bool:
  Bool.or(U32.is_le(v, 1), U32.is_eq(v, 4294967295))

def rules.Input.valid(+i: J.Val) -> Bool:
  +axes = Bool.and(
    Bool.and(rules.Input.axis_ok(core.u(i, "move_x")), rules.Input.axis_ok(core.u(i, "move_y"))),
    Bool.and(rules.Input.axis_ok(core.u(i, "aim_x")), rules.Input.axis_ok(core.u(i, "aim_y"))))
  +slots = Bool.and(
    U32.is_le(core.u(i, "play"), cards.HAND()),
    Bool.and(U32.is_le(core.u(i, "cast"), cards.SPELL_SLOTS()), U32.is_le(core.u(i, "skill"), skills.SKILLS())))
  Bool.and(axes, Bool.and(slots, rules.Input.vote_ok(J.get(i, "vote"))))

# `self.vote.is_none_or(votes::Command::valid)`
def rules.Input.vote_ok(+v: J.Val) -> Bool:
  rules.Input.vote_ok.go(J.is_null(v), v)

def rules.Input.vote_ok.go(none: Bool, +v: J.Val) -> Bool:
  match none:
    case True{}:
      True{}
    case False{}:
      votes.Command.valid(v)

# `*i != Input::default()`: anything pressed, held or asked.
def rules.Input.pressing(+i: J.Val) -> Bool:
  +moved = Bool.or(
    Bool.or(U32.is_ne(core.u(i, "move_x"), 0), U32.is_ne(core.u(i, "move_y"), 0)),
    Bool.or(U32.is_ne(core.u(i, "aim_x"), 0), U32.is_ne(core.u(i, "aim_y"), 0)))
  +keys = Bool.or(
    Bool.or(Bool.or(core.b(i, "fire"), core.b(i, "dash")), Bool.or(core.b(i, "bomb"), core.b(i, "swing"))),
    Bool.or(Bool.or(core.b(i, "vigil"), core.b(i, "ult")), Bool.or(core.b(i, "take"), core.b(i, "interact"))))
  +more = Bool.or(
    Bool.or(Bool.or(core.b(i, "ride"), core.b(i, "cycle")), Bool.or(core.b(i, "skip_plate"), core.b(i, "again"))),
    Bool.or(Bool.or(core.b(i, "leave_activity"), core.b(i, "leave_held")), core.b(i, "activity_menu")))
  +nums = Bool.or(
    Bool.or(U32.is_ne(core.u(i, "play"), 0), U32.is_ne(core.u(i, "cast"), 0)),
    U32.is_ne(core.u(i, "skill"), 0))
  +opts = Bool.or(Bool.not(J.is_null(J.get(i, "latch_tick"))), Bool.not(J.is_null(J.get(i, "vote"))))
  Bool.or(Bool.or(moved, keys), Bool.or(Bool.or(more, nums), opts))
