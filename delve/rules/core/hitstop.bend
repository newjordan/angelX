# crates/delve/src/rules/hitstop.rs — host-owned impact pauses. `Run::hitstop`
# is left out of the state while it is quiet (`State::quiet`), so a missing
# `hitstop` key is the all-zero state.

# The state as the Run holds it, an object with every field.
def hitstop.State.full(+s: J.Val) -> J.Val:
  +a = core.set_u(J.obj(), "left", core.u(s, "left"))
  +b = core.set_u(a, "cooldown", core.u(s, "cooldown"))
  +c = core.set_b(b, "held", core.b(s, "held"))
  +d = J.set(c, "buffered", core.or_else(J.get(s, "buffered"), J.obj()))
  core.set_u_skip(d, "pending", core.u(s, "pending"))

def hitstop.State.quiet(+s: J.Val) -> Bool:
  Bool.and(
    Bool.and(U32.is_zero(core.u(s, "left")), U32.is_zero(core.u(s, "cooldown"))),
    Bool.and(
      Bool.and(Bool.not(core.b(s, "held")), U32.is_zero(core.u(s, "pending"))),
      U32.is_zero(core.count(J.get(s, "buffered")))))

# `Run::hitstop` set to state `s`; a quiet state is left out.
def hitstop.State.put.go(quiet: Bool, +run: J.Val, +s: J.Val) -> J.Val:
  match quiet:
    case True{}:
      J.del(run, "hitstop")
    case False{}:
      J.set(run, "hitstop", hitstop.State.full(s))

def hitstop.State.put(+run: J.Val, +s: J.Val) -> J.Val:
  hitstop.State.put.go(hitstop.State.quiet(s), run, s)

# `self.left > 0 || self.held`
def hitstop.State.frozen(+s: J.Val) -> Bool:
  Bool.or(U32.is_ne(core.u(s, "left"), 0), core.b(s, "held"))

# `State::valid`
def hitstop.State.valid(+s: J.Val) -> Bool:
  Bool.and(
    Bool.and(U32.is_le(core.u(s, "left"), 12), U32.is_le(core.u(s, "cooldown"), 60)),
    Bool.and(
      Bool.and(U32.is_le(core.u(s, "pending"), 12), U32.is_le(core.count(J.get(s, "buffered")), rules.MAX_PLAYERS())),
      hitstop.State.valid.buffered(J.int_fields(J.get(s, "buffered")))))

def hitstop.State.valid.buffered(+bs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> Bool:
  match bs:
    case Nil{}:
      True{}
    case Con{(+k, +i), +t}:
      Bool.and(hitstop.State.valid.seat(k), Bool.and(rules.Input.valid(i), hitstop.State.valid.buffered(t)))

def hitstop.State.valid.seat(+k: String) -> Bool:
  hitstop.State.valid.seat.go(U32.read(k))

def hitstop.State.valid.seat.go(+id: Maybe<&2, U32>) -> Bool:
  match id:
    case Some{+n}:
      Bool.and(U32.is_ge(n, 1), U32.is_le(n, rules.MAX_PLAYERS()))
    case None{}:
      False{}

# `State::request`: the pause an impact asks for, held pending until the tick
# ends (`finish`). `tune` is the run's `tune` value; the state comes back.
def hitstop.State.request.ticks(+impact: HitstopImpact, +tune: J.Val) -> U32 & Bool:
  match impact:
    case HitHeavy{}:
      (tune.Tune.hitstop_heavy(tune), False{})
    case HitPhase{}:
      (tune.Tune.hitstop_phase(tune), False{})
    case HitEliteKill{}:
      (tune.Tune.hitstop_elite_kill(tune), True{})
    case HitBossKill{}:
      (tune.Tune.hitstop_boss_kill(tune), True{})
    case HitFinisher{}:
      (tune.Tune.hitstop_finisher(tune), True{})

def hitstop.State.request.go(on: Bool, r: U32 & Bool, +s: J.Val) -> J.Val:
  match on:
    case False{}:
      s
    case True{}:
      hitstop.State.request.kill(r, s)

def hitstop.State.request.kill(r: U32 & Bool, +s: J.Val) -> J.Val:
  (ticks, kill) = r
  hitstop.State.request.set(Bool.or(kill, U32.is_zero(core.u(s, "cooldown"))), ticks, s)

def hitstop.State.request.set(go: Bool, +ticks: U32, +s: J.Val) -> J.Val:
  match go:
    case True{}:
      core.set_u_skip(s, "pending", U32.max(core.u(s, "pending"), ticks))
    case False{}:
      s

def hitstop.State.request(+s: J.Val, +tune: J.Val, +impact: HitstopImpact) -> J.Val:
  hitstop.State.request.go(U32.is_ne(tune.Tune.hitstop(tune), 0), hitstop.State.request.ticks(impact, tune), hitstop.State.full(s))

# `State::finish`: the pending pause begins; whether one did. (state, began)
def hitstop.State.finish.go(+ticks: U32, +cooldown: U32, +s: J.Val) -> J.Val & Bool:
  match ticks:
    case 0:
      (s, False{})
    case _:
      (core.set_u(core.set_u_skip(core.set_u(s, "left", ticks), "pending", 0), "cooldown", cooldown), True{})

def hitstop.State.finish(+s: J.Val, +tune: J.Val) -> J.Val & Bool:
  hitstop.State.finish.go(core.u(s, "pending"), tune.Tune.hitstop_cooldown(tune), hitstop.State.full(s))

# `Run::finish_hitstop`
def hitstop.Run.finish_hitstop(+run: J.Val) -> J.Val:
  hitstop.Run.finish_hitstop.go(hitstop.State.finish(J.get(run, "hitstop"), J.get(run, "tune")), run)

def hitstop.Run.finish_hitstop.go(r: J.Val & Bool, +run: J.Val) -> J.Val:
  (s, began) = r
  match began:
    case False{}:
      hitstop.State.put(run, s)
    case True{}:
      core.set_u(hitstop.State.put(run, s), "shake", U32.max(core.u(run, "shake"), feel.SHAKE_SHOT()))

# `Input { move_x/move_y only while the dash is held, ..input }`
def hitstop.press(+input: J.Val) -> J.Val:
  hitstop.press.go(core.b(input, "dash"), input)

def hitstop.press.go(dash: Bool, +input: J.Val) -> J.Val:
  match dash:
    case True{}:
      input
    case False{}:
      J.set(J.set(input, "move_x", J.JU{0}), "move_y", J.JU{0})

# Buffer each valid press of a seat that is here: the held controls lay over it.
def hitstop.State.inputs.buffer(+xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +buffered: J.Val) -> J.Val:
  match xs:
    case Nil{}:
      buffered
    case Con{(+k, +input), +t}:
      hitstop.State.inputs.buffer(t, players, hitstop.State.inputs.buffer.one(Bool.and(J.has(players, k), rules.Input.valid(input)), k, hitstop.press(input), buffered))

def hitstop.State.inputs.buffer.one(take: Bool, +k: String, +press: J.Val, +buffered: J.Val) -> J.Val:
  match take:
    case False{}:
      buffered
    case True{}:
      hitstop.State.inputs.buffer.lay(rules.Input.pressing(press), k, press, buffered)

def hitstop.State.inputs.buffer.lay(go: Bool, +k: String, +press: J.Val, +buffered: J.Val) -> J.Val:
  match go:
    case False{}:
      buffered
    case True{}:
      J.set(buffered, k, command.lay_over(core.or_else(J.get(buffered, k), rules.Input.default()), press))

# The buffered presses of seats that are no longer here dropped.
def hitstop.State.inputs.retain(+xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +acc: J.Val) -> J.Val:
  match xs:
    case Nil{}:
      acc
    case Con{(+k, +i), +t}:
      hitstop.State.inputs.retain(t, players, hitstop.State.inputs.retain.one(J.has(players, k), k, i, acc))

def hitstop.State.inputs.retain.one(here: Bool, +k: String, +i: J.Val, +acc: J.Val) -> J.Val:
  match here:
    case True{}:
      J.set(acc, k, i)
    case False{}:
      acc

# Presses buffered through the hold laid over the latest map.
def hitstop.State.inputs.resume(+xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +resumed: J.Val) -> J.Val:
  match xs:
    case Nil{}:
      resumed
    case Con{(+k, +press), +t}:
      hitstop.State.inputs.resume(t, J.set(resumed, k, command.lay_over(press, hitstop.State.inputs.valid_or_default(J.get(resumed, k)))))

def hitstop.State.inputs.valid_or_default(+i: J.Val) -> J.Val:
  hitstop.State.inputs.valid_or_default.go(Bool.and(Bool.not(J.is_null(i)), rules.Input.valid(i)), i)

def hitstop.State.inputs.valid_or_default.go(ok: Bool, +i: J.Val) -> J.Val:
  match ok:
    case True{}:
      i
    case False{}:
      rules.Input.default()

# `State::inputs`: the hold's use of the caller's map. The run comes back with
# the hold's state updated, and what the step does next.
def hitstop.State.inputs(+run: J.Val, +inputs: J.Val) -> J.Val & HitstopResume:
  hitstop.State.inputs.a(hitstop.State.full(J.get(run, "hitstop")), run, inputs)

# `if tune.hitstop == 0`: a disabled hold releases itself.
def hitstop.State.inputs.a(+s: J.Val, +run: J.Val, +inputs: J.Val) -> J.Val & HitstopResume:
  hitstop.State.inputs.b(
    Bool.pick(J.Val, U32.is_zero(tune.Tune.hitstop(J.get(run, "tune"))),
      core.set_u_skip(core.set_b(core.set_u(core.set_u(s, "left", 0), "cooldown", 0), "held", False{}), "pending", 0), s),
    run, inputs)

def hitstop.State.inputs.b(+s: J.Val, +run: J.Val, +inputs: J.Val) -> J.Val & HitstopResume:
  +s1 = core.set_u(s, "cooldown", core.usub(core.u(s, "cooldown"), 1))
  +s2 = J.set(s1, "buffered", hitstop.State.inputs.retain(J.int_fields(J.get(s1, "buffered")), J.get(run, "players"), J.obj()))
  hitstop.State.inputs.c(U32.is_ne(core.u(s2, "left"), 0), s2, run, inputs)

def hitstop.State.inputs.c(holding: Bool, +s: J.Val, +run: J.Val, +inputs: J.Val) -> J.Val & HitstopResume:
  match holding:
    case True{}:
      (hitstop.State.put(run,
        core.set_b(core.set_u(
          J.set(s, "buffered", hitstop.State.inputs.buffer(J.int_fields(inputs), J.get(run, "players"), J.get(s, "buffered"))),
          "left", core.usub(core.u(s, "left"), 1)), "held", True{})),
       HitWait{})
    case False{}:
      hitstop.State.inputs.d(U32.is_zero(core.count(J.get(s, "buffered"))), core.set_b(s, "held", False{}), run, inputs)

def hitstop.State.inputs.d(empty: Bool, +s: J.Val, +run: J.Val, +inputs: J.Val) -> J.Val & HitstopResume:
  match empty:
    case True{}:
      (hitstop.State.put(run, s), HitSame{})
    case False{}:
      (hitstop.State.put(run, J.set(s, "buffered", J.obj())),
       HitFresh{hitstop.State.inputs.resume(J.int_fields(J.get(s, "buffered")), inputs)})

# Hitstop's `boss_phase`: how far through its chain of phases a boss is.
def hitstop.Run.boss_phase(+run: J.Val, +enemy: J.Val) -> U32:
  hitstop.Run.boss_phase.go(J.get(enemy, "boss"), run, enemy)

def hitstop.Run.boss_phase.go(+idx: J.Val, +run: J.Val, +enemy: J.Val) -> U32:
  hitstop.Run.boss_phase.phases(
    J.get(J.nth(J.get(run, "bosses"), J.u32(idx)), "phases"), J.is_null(idx), enemy)

def hitstop.Run.boss_phase.phases(+p: J.Val, none: Bool, +enemy: J.Val) -> U32:
  match none:
    case True{}:
      0
    case False{}:
      hitstop.Run.boss_phase.calc(J.is_null(p), p, enemy)

def hitstop.Run.boss_phase.calc(none: Bool, +p: J.Val, +enemy: J.Val) -> U32:
  match none:
    case True{}:
      0
    case False{}:
      U32.mod(U32.div(core.u(enemy, "pattern_volleys"), U32.max(core.u(p, "volleys"), 1)), U32.max(J.len(J.get(p, "chain")), 1))

# `elite`: a large body that is not a dummy or a ward.
def hitstop.elite(+enemy: J.Val) -> Bool:
  Bool.and(
    Bool.not(Bool.or(String.eq(core.s(enemy, "kind"), "dummy"), String.eq(core.s(enemy, "kind"), "ward"))),
    F32.is_ge(monsters.Enemy.radius(enemy), 1.0))
