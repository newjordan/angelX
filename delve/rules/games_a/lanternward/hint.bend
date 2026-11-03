# crates/delve/src/rules/lanternward/hint.rs (+ props::sign_text / folk_text
# and Ctx::label): the wisp's words. Strings, or null for none.

def lanternward.props.sign_text(+room: U32, +id: U32) -> String:
  match room:
    case 0:
      "HOLD R TO LOCK ON"
    case 1:
      "FLARE SHOWS WHAT IS HIDDEN"
    case 2:
      "SHIELD UP AS IT FLIES"
    case 3:
      "LIGHT BURNS THE DARK OFF"
    case _:
      ""

def lanternward.props.folk_text(+room: U32, +id: U32) -> String:
  match room:
    case 0:
      "THE KEEP DARKENS. FIND THE LANTERN."
    case _:
      ""

# What E's label says.
def lanternward.ctx.Ctx.label(+tag: String) -> String:
  Bool.pick(String, String.eq(tag, "Open"), "OPEN",
    Bool.pick(String, String.eq(tag, "Read"), "READ",
      Bool.pick(String, String.eq(tag, "Lift"), "LIFT",
        Bool.pick(String, String.eq(tag, "Throw"), "THROW",
          Bool.pick(String, String.eq(tag, "Flame"), "TAKE FLAME",
            Bool.pick(String, String.eq(tag, "Light"), "LIGHT",
              Bool.pick(String, String.eq(tag, "Unlock"), "UNLOCK",
                Bool.pick(String, String.eq(tag, "Revive"), "REVIVE",
                  Bool.pick(String, String.eq(tag, "Relight"), "RELIGHT",
                    Bool.pick(String, String.eq(tag, "Talk"), "TALK",
                      Bool.pick(String, String.eq(tag, "Pull"), "PULL", "")))))))))))

# What a target that is not a foe says about itself when it is locked.
def lanternward.hint.lock_hint(+s: J.Val, +k: J.Val) -> J.Val:
  lanternward.hint.lock_hint.go(lanternward.tag_is(ga.v(k, "lock"), "Prop"), J.u32(lanternward.payload(ga.v(k, "lock"))), s)

def lanternward.hint.lock_hint.go(prop: Bool, +i: U32, +s: J.Val) -> J.Val:
  match prop:
    case False{}:
      J.JNull{}
    case True{}:
      lanternward.hint.lock_hint.of(U32.is_lt(i, lanternward.len(s, "props")), i, s)

def lanternward.hint.lock_hint.of(has: Bool, +i: U32, +s: J.Val) -> J.Val:
  match has:
    case False{}:
      J.JNull{}
    case True{}:
      lanternward.hint.lock_hint.kind(lanternward.eu(s, "props", i, "id"), ga.s(lanternward.el(s, "props", i), "kind"), s)

def lanternward.hint.lock_hint.kind(+id: U32, +kind: String, +s: J.Val) -> J.Val:
  Bool.pick(J.Val, String.eq(kind, "Sign"), J.JS{lanternward.props.sign_text(ga.u(s, "room"), id)},
    Bool.pick(J.Val, String.eq(kind, "Folk"), J.JS{"A KINGSMAN: E TALKS"},
      Bool.pick(J.Val, String.eq(kind, "Crystal"), J.JS{"A CRYSTAL: LIGHT OR A PARRIED BOLT"},
        Bool.pick(J.Val, String.eq(kind, "Great"), J.JS{"THE GREAT BRAZIER: THROW LIGHT"},
          Bool.pick(J.Val, String.eq(kind, "Brazier"), J.JS{"A BRAZIER: LIGHT IT"},
            Bool.pick(J.Val, String.eq(kind, "Sigil"), J.JS{"A HIDDEN SIGIL: FLARE IT"},
              Bool.pick(J.Val, String.eq(kind, "Lever"), J.JS{"A LEVER: E PULLS"}, J.JNull{})))))))

def lanternward.hint.near_foe(+fs: List<&2, J.Val>, +k: J.Val, +lock: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      Bool.or(
        Bool.and(lanternward.foes.Foe.alive(f),
          RS.i_lt(lanternward.world.dist((ga.u(f, "x") - ga.u(k, "x") : U32), (ga.u(f, "y") - ga.u(k, "y") : U32)), lock)),
        lanternward.hint.near_foe(t, k, lock))

# The wisp's one line. Few words, always about what to do next.
def lanternward.hint.hint(+s: J.Val, +k: J.Val, +r: J.Val) -> J.Val:
  Bool.pick(J.Val, lanternward.is(k, "act", "Down"),
    J.JS{"WAIT FOR A FRIEND'S HAND"},
    lanternward.hint.hint.room(ga.u(s, "room"), ga.u(s, "stage"), s, k, lanternward.hint.near_foe(ga.items(s, "foes"), k, ga.u(r, "lock"))))

def lanternward.hint.hint.room(+room: U32, +stage: U32, +s: J.Val, +k: J.Val, +near: Bool) -> J.Val:
  match room:
    case 0:
      lanternward.hint.hint.r0(stage, s, k, near)
    case 1:
      lanternward.hint.hint.r1(stage, k)
    case 2:
      lanternward.hint.hint.r2(stage, s, k)
    case 3:
      lanternward.hint.hint.r3(stage)
    case 4:
      lanternward.hint.hint.r4(stage, s)
    case _:
      J.JNull{}

def lanternward.hint.hint.r0(+stage: U32, +s: J.Val, +k: J.Val, +near: Bool) -> J.Val:
  Bool.pick(J.Val, U32.is_ne(stage, 0), J.JS{"THE STAIR IS OPEN"},
    Bool.pick(J.Val, Bool.and(J.is_null(ga.v(k, "lock")), near), J.JS{"HOLD R TO LOCK ON"},
      Bool.pick(J.Val, Bool.and(Bool.and(Bool.not(J.is_null(ga.v(k, "lock"))), U32.is_eq(U32.and(ga.u(k, "held"), 4), 0)), U32.is_lt(ga.u(s, "elapsed"), 600)),
        J.JS{"LOCK ON, THEN HOP: E"},
        Bool.pick(J.Val, Bool.not(J.is_null(ga.v(k, "lock"))), J.JS{"Q SLASH  HOLD SPACE BLOCK"},
          Bool.pick(J.Val, U32.is_lt(ga.u(s, "elapsed"), 150), J.JS{"WALK TO THE SNUFFER"}, J.JS{"THE STAIR IS OPEN"})))))

def lanternward.hint.hint.r1(+stage: U32, +k: J.Val) -> J.Val:
  match stage:
    case 0:
      Bool.pick(J.Val, U32.is_eq(ga.u(k, "taper"), 0), J.JS{"E AT THE EMBER BOWL"}, J.JS{"LIGHT ALL THREE AT ONCE"})
    case 1:
      J.JS{"OPEN THE CHEST"}
    case 2:
      J.JS{"HOLD F TO FLARE"}
    case 3:
      J.JS{"TAP F AT THE GREAT BRAZIER"}
    case _:
      J.JNull{}

def lanternward.hint.hint.r2(+stage: U32, +s: J.Val, +k: J.Val) -> J.Val:
  match stage:
    case 0:
      Bool.pick(J.Val, lanternward.stage.any_on(ga.items(s, "props"), "Gate", 1), J.JS{"LOCK THE CRYSTAL, SHIELD UP"},
        Bool.pick(J.Val,
          Bool.and(U32.is_ge(lanternward.seat_count(s), 2), lanternward.stage.any_on(ga.items(s, "props"), "Plate", 0)),
          J.JS{"A FRIEND STANDS ON THE SECOND PLATE"}, J.JS{"PUSH THE BLOCK ONTO THE PLATE"}))
    case _:
      Bool.pick(J.Val, Bool.and(ga.b(ga.v(s, "inv"), "lantern"), J.is_null(ga.v(k, "lock"))),
        J.JS{"LOCK THE CRYSTAL, SHIELD UP"}, J.JS{"SPACE SHIELD TURNS THE BOLT"})

def lanternward.hint.hint.r3(+stage: U32) -> J.Val:
  match stage:
    case 0:
      J.JS{"HOLD F: BURN THE HOOD OFF"}
    case 1:
      J.JS{"TAKE THE MOON KEY"}
    case 2:
      J.JS{"E AT THE LOCKED DOOR"}
    case 3:
      J.JS{"OPEN THE BOSS KEY CHEST"}
    case _:
      J.JS{"CARRY THE KEY TO THE DOOR"}

def lanternward.hint.hint.r4(+stage: U32, +s: J.Val) -> J.Val:
  match stage:
    case 0:
      J.JS{"THROW LIGHT AT ITS HEART"}
    case 1:
      lanternward.hint.hint.warden(J.get(s, "boss"), lanternward.seat_count(s))
    case 2:
      J.JS{"TAKE THE HEART"}
    case 3:
      J.JS{"HOLD E AT THE BEACON"}
    case _:
      J.JNull{}

def lanternward.hint.hint.warden(+w: J.Val, +seats: U32) -> J.Val:
  match w:
    case J.JNull{}:
      J.JS{"LIGHT THE HEART, THEN STRIKE"}
    case _:
      Bool.pick(J.Val, Bool.and(U32.is_ge(ga.u(w, "phase"), 1), U32.is_ge(seats, 2)), J.JS{"TWO LIGHTS AT ONCE"},
        Bool.pick(J.Val, Bool.and(lanternward.is(w, "st", "Tell"), lanternward.is(w, "atk", "Slam")), J.JS{"HOP THE RING"},
          Bool.pick(J.Val, Bool.and(lanternward.is(w, "st", "Tell"), lanternward.is(w, "atk", "Beam")), J.JS{"SHIELD UP: PARRY THE BEAM"},
            J.JS{"LIGHT THE HEART, THEN STRIKE"})))

# The line at the foot of seat `k`'s screen: a sign or kingsman being read,
# else what a locked switch says, else the wisp.
def lanternward.hint.line(+s: J.Val, +k: J.Val, +r: J.Val) -> J.Val:
  lanternward.hint.line.read(ga.v(k, "read"), s, k, r)

def lanternward.hint.line.read(+rd: J.Val, +s: J.Val, +k: J.Val, +r: J.Val) -> J.Val:
  match rd:
    case J.JNull{}:
      lanternward.hint.line.lock(lanternward.hint.lock_hint(s, k), s, k, r)
    case _:
      Bool.pick(J.Val, U32.is_ge(ga.nu(rd, 0), 100),
        J.JS{lanternward.props.folk_text(ga.u(s, "room"), (ga.nu(rd, 0) - 100 : U32))},
        J.JS{lanternward.props.sign_text(ga.u(s, "room"), ga.nu(rd, 0))})

def lanternward.hint.line.lock(+h: J.Val, +s: J.Val, +k: J.Val, +r: J.Val) -> J.Val:
  match h:
    case J.JNull{}:
      lanternward.hint.hint(s, k, r)
    case _:
      h

# What E would do: its label, or the roll or hop it falls back to.
def lanternward.hint.context(+k: J.Val) -> J.Val:
  lanternward.hint.context.tag(lanternward.tag(ga.v(k, "ctx")), k)

def lanternward.hint.context.tag(+tag: String, +k: J.Val) -> J.Val:
  Bool.pick(J.Val, String.eq(tag, "None"),
    Bool.pick(J.Val, J.is_null(ga.v(k, "lock")), J.JS{"E ROLL"}, J.JS{"E HOP"}),
    J.JS{"E " ++ lanternward.ctx.Ctx.label(tag)})

def case.lanternward.hint.line(+input: J.Val) -> J.Val:
  lanternward.hint.line(J.get(input, "s"), J.get(input, "k"), J.get(input, "r"))

def case.lanternward.hint.hint(+input: J.Val) -> J.Val:
  lanternward.hint.hint(J.get(input, "s"), J.get(input, "k"), J.get(input, "r"))

def case.lanternward.hint.lock_hint(+input: J.Val) -> J.Val:
  lanternward.hint.lock_hint(J.get(input, "s"), J.get(input, "k"))

def case.lanternward.hint.context(+input: J.Val) -> J.Val:
  lanternward.hint.context(J.get(input, "k"))
