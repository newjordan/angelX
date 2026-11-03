# crates/delve/src/rules/command.rs — what the step reaches: `lay_over`.

# An axis `over` gives, else `under`'s (i8, as its J value).
def command.lay_over.axis.go(set: Bool, +u: J.Val, +o: J.Val) -> J.Val:
  match set:
    case True{}:
      o
    case False{}:
      core.or_else(u, J.JU{0})

def command.lay_over.axis(+under: J.Val, +over: J.Val, +k: String) -> J.Val:
  command.lay_over.axis.go(U32.is_ne(core.u(over, k), 0), J.get(under, k), J.get(over, k))

# A slot `over` gives, else `under`'s.
def command.lay_over.slot.go(set: Bool, +u: U32, +o: U32) -> U32:
  match set:
    case True{}:
      o
    case False{}:
      u

def command.lay_over.slot(+under: J.Val, +over: J.Val, +k: String) -> U32:
  command.lay_over.slot.go(U32.is_ne(core.u(over, k), 0), core.u(under, k), core.u(over, k))

def command.lay_over.flag(+under: J.Val, +over: J.Val, +k: String) -> Bool:
  Bool.or(core.b(under, k), core.b(over, k))

# `over.x.or(under.x)` for an Option field, written only when Some.
def command.lay_over.opt(+out: J.Val, +under: J.Val, +over: J.Val, +k: String) -> J.Val:
  command.lay_over.opt.go(out, k, core.or_else(J.get(over, k), J.get(under, k)))

def command.lay_over.opt.go(+out: J.Val, +k: String, +v: J.Val) -> J.Val:
  command.lay_over.opt.some(J.is_null(v), out, k, v)

def command.lay_over.opt.some(none: Bool, +out: J.Val, +k: String, +v: J.Val) -> J.Val:
  match none:
    case True{}:
      out
    case False{}:
      J.set(out, k, v)

# `under` with every field `over` sets laid on top: a flag set in either, an
# axis or slot `over` gives, `over`'s vote if it has one.
def command.lay_over(+under: J.Val, +over: J.Val) -> J.Val:
  +i0 = J.obj()
  +i1 = command.lay_over.opt(i0, under, over, "latch_tick")
  +i2 = core.set_b_skip(i1, "leave_activity", command.lay_over.flag(under, over, "leave_activity"))
  +i3 = core.set_b_skip(i2, "leave_held", core.b(under, "leave_held"))
  +i4 = core.set_b_skip(i3, "activity_menu", command.lay_over.flag(under, over, "activity_menu"))
  +i5 = command.lay_over.opt(i4, under, over, "vote")
  +i6 = J.set(i5, "move_x", command.lay_over.axis(under, over, "move_x"))
  +i7 = J.set(i6, "move_y", command.lay_over.axis(under, over, "move_y"))
  +i8 = J.set(i7, "aim_x", command.lay_over.axis(under, over, "aim_x"))
  +i9 = J.set(i8, "aim_y", command.lay_over.axis(under, over, "aim_y"))
  +j0 = core.set_b(i9, "fire", command.lay_over.flag(under, over, "fire"))
  +j1 = core.set_b(j0, "dash", command.lay_over.flag(under, over, "dash"))
  +j2 = core.set_b(j1, "bomb", command.lay_over.flag(under, over, "bomb"))
  +j3 = core.set_b(j2, "swing", command.lay_over.flag(under, over, "swing"))
  +j4 = core.set_u(j3, "play", command.lay_over.slot(under, over, "play"))
  +j5 = core.set_u(j4, "cast", command.lay_over.slot(under, over, "cast"))
  +j6 = core.set_b_skip(j5, "vigil", command.lay_over.flag(under, over, "vigil"))
  +j7 = core.set_b_skip(j6, "ult", command.lay_over.flag(under, over, "ult"))
  +j8 = core.set_b_skip(j7, "take", command.lay_over.flag(under, over, "take"))
  +j9 = core.set_b_skip(j8, "interact", command.lay_over.flag(under, over, "interact"))
  +k0 = core.set_u_skip(j9, "skill", command.lay_over.slot(under, over, "skill"))
  +k1 = core.set_b_skip(k0, "ride", command.lay_over.flag(under, over, "ride"))
  +k2 = core.set_b_skip(k1, "cycle", command.lay_over.flag(under, over, "cycle"))
  +k3 = core.set_b_skip(k2, "skip_plate", command.lay_over.flag(under, over, "skip_plate"))
  core.set_b_skip(k3, "again", command.lay_over.flag(under, over, "again"))
