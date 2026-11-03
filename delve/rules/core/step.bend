# crates/delve/src/rules.rs — `Run::step`, phase by phase in the order Rust
# runs them. `run` is the checkpoint's `run` object, `inputs` the event's
# `inputs` object (`BTreeMap<u32, Input>` keyed by decimal seat id).

# One fixed step of the run, and of the show watching it.
def rules.Run.step(+run: J.Val, +inputs: J.Val) -> J.Val:
  rules.Run.step.cross(J.is_null(J.get(run, "crossroads")), run, inputs)

# `if self.crossroads.is_some() { count_idle; tick_crossroads; return }`
def rules.Run.step.cross(none: Bool, +run: J.Val, +inputs: J.Val) -> J.Val:
  match none:
    case False{}:
      crossroads.Run.tick_crossroads(rules.Run.count_idle(run, inputs), inputs)
    case True{}:
      rules.Run.step.leave(exits.Run.tick_leave_hold(run, inputs), inputs)

# Leaving follows transport time, even while an impact holds the scene: the
# seats the leave hold blanked press nothing this tick.
def rules.Run.step.leave(r: J.Val & List<&2, Maybe<&2, U32>>, +inputs: J.Val) -> J.Val:
  (+run, +blanks) = r
  rules.Run.step.hold(run, rules.Run.step.blank(blanks, inputs))

def rules.Run.step.blank(blanks: List<&2, Maybe<&2, U32>>, +inputs: J.Val) -> J.Val:
  match blanks:
    case Nil{}:
      inputs
    case Con{b, t}:
      rules.Run.step.blank(t, rules.Run.step.blank.one(b, inputs))

def rules.Run.step.blank.one(b: Maybe<&2, U32>, +inputs: J.Val) -> J.Val:
  match b:
    case Some{+id}:
      J.set(inputs, U32.show(id), rules.Input.default())
    case None{}:
      inputs

# The hitstop gate: still frozen, the caller's map, or the presses buffered
# through the hold laid over the latest.
def rules.Run.step.hold(+run: J.Val, +transport: J.Val) -> J.Val:
  rules.Run.step.resume(hitstop.State.inputs(run, transport), transport)

def rules.Run.step.resume(r: J.Val & HitstopResume, +transport: J.Val) -> J.Val:
  (+run, +res) = r
  match res:
    case HitWait{}:
      run
    case HitSame{}:
      rules.Run.step.keep(run, transport)
    case HitFresh{+map}:
      rules.Run.step.keep(run, map)

def rules.Run.step.keep(+run: J.Val, +inputs: J.Val) -> J.Val:
  # A knight's wishes are held to the budget of the floor they stand on.
  +r1 = overclass.Run.keep_wish_floor(run)
  # Who is here may have changed since the last tick.
  +before = rules.Run.scaled(r1)
  +r2 = rules.Run.rescale_party(r1, before)
  +r3 = rules.Run.count_idle(r2, inputs)
  rules.Run.step.retreat(r3, inputs)

# Retreat is processed before reins, overlays, or combat: a departing seat's
# controls, including a held interact key, are discarded.
def rules.Run.step.retreat(+run: J.Val, +inputs: J.Val) -> J.Val:
  rules.Run.step.retreat.go(J.int_fields(inputs), run, inputs)

def rules.Run.step.retreat.go(+xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +owned: J.Val) -> J.Val:
  match xs:
    case Nil{}:
      rules.Run.step.detach(run, owned)
    case Con{(+k, +input), +t}:
      rules.Run.step.retreat.seat(t, k, Bool.and(rules.Input.valid(input), core.b(input, "leave_activity")), run, owned)

def rules.Run.step.retreat.seat(+t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +k: String, leaving: Bool, +run: J.Val, +owned: J.Val) -> J.Val:
  match leaving:
    case False{}:
      rules.Run.step.retreat.go(t, run, owned)
    case True{}:
      rules.Run.step.retreat.said(t, k, exits.Run.leave_activity(run, rules.Run.step.id(k)), owned)

def rules.Run.step.retreat.said(+t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +k: String, r: J.Val & Bool, +owned: J.Val) -> J.Val:
  (+run, +left) = r
  rules.Run.step.retreat.go(t, run, Bool.pick(J.Val, left, J.set(owned, k, rules.Input.default()), owned))

# A seat's key as its id.
def rules.Run.step.id(+k: String) -> U32:
  rules.Run.step.id.go(U32.read(k))

def rules.Run.step.id.go(+n: Maybe<&2, U32>) -> U32:
  match n:
    case Some{+v}:
      v
    case None{}:
      0

def rules.Run.step.detach(+run: J.Val, +inputs: J.Val) -> J.Val:
  +r1 = exits.Run.close_behind_the_away(run)
  rules.Run.step.detached(exits.Run.detached_seats(r1, inputs), inputs)

# `if self.players.is_empty() { self.players.extend(detached); return }`
def rules.Run.step.detached(r: J.Val & J.Val, +inputs: J.Val) -> J.Val:
  (+run, +detached) = r
  rules.Run.step.detached.go(U32.is_zero(core.count(J.get(run, "players"))), run, detached, inputs)

def rules.Run.step.detached.go(empty: Bool, +run: J.Val, +detached: J.Val, +inputs: J.Val) -> J.Val:
  match empty:
    case True{}:
      rules.Run.extend_players(run, detached)
    case False{}:
      rules.Run.step.plates(floor_plates.Run.tick_floor_plates(run, inputs), detached, inputs)

# `self.players.extend(detached)`: each detached seat's knight put back.
def rules.Run.extend_players(+run: J.Val, +detached: J.Val) -> J.Val:
  rules.Run.extend_players.go(J.fields(detached), run)

def rules.Run.extend_players.go(+xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val) -> J.Val:
  match xs:
    case Nil{}:
      run
    case Con{(+k, +h), +t}:
      rules.Run.extend_players.go(t, J.set(run, "players", J.set(J.get(run, "players"), k, h)))

def rules.Run.step.plates(+run: J.Val, +detached: J.Val, +inputs: J.Val) -> J.Val:
  rules.Run.step.ballot(votes.Run.ballot_inputs(run, inputs), detached, inputs)

def rules.Run.step.ballot(r: J.Val & Maybe<&2, J.Val>, +detached: J.Val, +inputs: J.Val) -> J.Val:
  (+run, +m) = r
  match m:
    case Some{+map}:
      rules.Run.step.offer(genie.Run.offer_inputs(run, map), detached, map)
    case None{}:
      rules.Run.step.offer(genie.Run.offer_inputs(run, inputs), detached, inputs)

# NB: `offer_inputs` is handed the ballot map; its Some replaces it.
def rules.Run.step.offer(r: J.Val & Maybe<&2, J.Val>, +detached: J.Val, +inputs: J.Val) -> J.Val:
  (+run, +m) = r
  match m:
    case Some{+map}:
      rules.Run.step.play(run, detached, map)
    case None{}:
      rules.Run.step.play(run, detached, inputs)

# The tick itself: what was heard and seen before it, the (turbo) advances, the
# watchers, the verdicts.
def rules.Run.step.play(+run: J.Val, +detached: J.Val, +inputs: J.Val) -> J.Val:
  +live = rules.Run.active(run)
  +heard = J.len(J.get(run, "cues"))
  +sounded = J.len(J.get(run, "sounds"))
  +seen = Bool.pick(J.Val, live, spectacle.Run.seen(run), J.JNull{})
  +was_over = again.Run.over(run)
  +r1 = rules.Run.advance(run, inputs)
  +r2 = rules.Run.step.turbo(Bool.and(fortune.Run.turbo(r1), U32.is_zero(core.u(J.get(r1, "hitstop"), "left"))), r1, inputs)
  +r3 = fellowship.Run.call_fellowship(r2)
  +r4 = questline.Run.flush_questlines(r3)
  +r5 = questline.Run.tick_teachers(r4, inputs)
  +r6 = kingdom.Run.follow_kingdom_cameras(r5)
  rules.Run.step.after(live, heard, sounded, seen, was_over, r6, detached)

def rules.Run.step.turbo(on: Bool, +run: J.Val, +inputs: J.Val) -> J.Val:
  match on:
    case True{}:
      rules.Run.advance(run, inputs)
    case False{}:
      run

def rules.Run.step.after(live: Bool, +heard: U32, +sounded: U32, +seen: J.Val, was_over: Bool, +run: J.Val, +detached: J.Val) -> J.Val:
  match live:
    case True{}:
      rules.Run.step.watch(heard, sounded, seen, was_over, run, detached)
    case False{}:
      rules.Run.step.wiped(was_over, run, detached)

# `watch(seen)` and the `live` watchers.
def rules.Run.step.watch(+heard: U32, +sounded: U32, +seen: J.Val, was_over: Bool, +run: J.Val, +detached: J.Val) -> J.Val:
  +r1 = spectacle.Run.watch(run, seen)
  +r2 = dares.Run.watch_dare(r1, heard, sounded)
  +r3 = bounties.Run.mark_moments(r2, heard)
  +r4 = trophies.Run.mark_trophies(r3, heard)
  rules.Run.step.wiped(was_over, r4, detached)

# A wipe that a detached seat's living knight undoes.
def rules.Run.step.wiped(was_over: Bool, +run: J.Val, +detached: J.Val) -> J.Val:
  rules.Run.step.wiped.go(Bool.and(core.phase_is(run, "wiped"), rules.Run.step.any_alive(J.fields(detached))), was_over, run, detached)

def rules.Run.step.any_alive(+xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{(+k, +h), +t}:
      Bool.or(U32.is_ne(core.u(h, "hp"), 0), rules.Run.step.any_alive(t))

def rules.Run.step.wiped.go(undone: Bool, was_over: Bool, +run: J.Val, +detached: J.Val) -> J.Val:
  match undone:
    case True{}:
      rules.Run.step.close(was_over, rules.Run.extend_players(rules.Run.step.revive_phase(run), detached))
    case False{}:
      rules.Run.step.close(was_over, rules.Run.extend_players(run, detached))

# `self.phase = if self.barred() && !self.open_map() { Fighting } else { Exploring }`
def rules.Run.step.revive_phase(+run: J.Val) -> J.Val:
  rules.Run.step.revive_phase.go(Bool.and(rules.Run.barred(run), Bool.not(depths.Run.open_map(run))), run)

def rules.Run.step.revive_phase.go(fight: Bool, +run: J.Val) -> J.Val:
  match fight:
    case True{}:
      core.set_s(run, "phase", "fighting")
    case False{}:
      core.set_s(run, "phase", "exploring")

def rules.Run.step.close(was_over: Bool, +run: J.Val) -> J.Val:
  rules.Run.step.close.go(Bool.and(Bool.not(was_over), again.Run.over(run)), run)

def rules.Run.step.close.go(closing: Bool, +run: J.Val) -> J.Val:
  match closing:
    case True{}:
      rules.Run.step.verdict(floor_stats.Run.close_run(run))
    case False{}:
      rules.Run.step.verdict(run)

def rules.Run.step.verdict(+run: J.Val) -> J.Val:
  +r1 = floor_stats.Run.stand_verdict(run)
  +r2 = Bool.pick(J.Val, rules.Run.active(r1), r1, core.floor_plates_clear(r1))
  # What the step's watchers showed after the tick (`fx`).
  fx.Run.show_fx(r2)

# `self.floor_plates.clear()`
def core.floor_plates_clear(+run: J.Val) -> J.Val:
  J.set(run, "floor_plates", J.obj())

# Each knight's idle ticks: back to none on any key, one more on a tick with
# no key or no seat at all.
def rules.Run.count_idle(+run: J.Val, +inputs: J.Val) -> J.Val:
  J.set(run, "idle", rules.Run.count_idle.go(J.int_fields(J.get(run, "players")), J.get(run, "idle"), inputs, J.obj()))

def rules.Run.count_idle.go(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +old: J.Val, +inputs: J.Val, +acc: J.Val) -> J.Val:
  match hs:
    case Nil{}:
      acc
    case Con{(+k, +h), +t}:
      rules.Run.count_idle.go(t, old, inputs, J.set(acc, k, J.JU{rules.Run.count_idle.next(J.get(old, k), J.get(inputs, k))}))

def rules.Run.count_idle.next(+old: J.Val, +input: J.Val) -> U32:
  rules.Run.count_idle.next.go(Bool.and(Bool.not(J.is_null(input)), Bool.and(rules.Input.valid(input), rules.Input.pressing(input))), old)

def rules.Run.count_idle.next.go(pressing: Bool, +old: J.Val) -> U32:
  match pressing:
    case True{}:
      0
    case False{}:
      core.uadd(J.u32(old), 1)

# Monsters scaled from `before` knights to the party as it is now.
def rules.Run.rescale_party(+run: J.Val, +before: U32) -> J.Val:
  rules.Run.rescale_party.go(U32.max(rules.Run.party_len(run), 1), before, run)

def rules.Run.rescale_party.go(+now: U32, +before: U32, +run: J.Val) -> J.Val:
  rules.Run.rescale_party.fin(now, U32.is_ne(now, U32.max(before, 1)), before, run)

def rules.Run.rescale_party.fin(+now: U32, changed: Bool, +before: U32, +run: J.Val) -> J.Val:
  match changed:
    case True{}:
      rules.Run.rescale_party.set(now, rules.Run.rescale(run, before, now))
    case False{}:
      rules.Run.rescale_party.set(now, run)

def rules.Run.rescale_party.set(+now: U32, +run: J.Val) -> J.Val:
  core.set_u_skip(run, "scaled_for", Bool.pick(U32, U32.is_eq(now, core.count(J.get(run, "players"))), 0, now))

# Monster health follows the party size, keeping each one's share of damage
# already dealt.
def rules.Run.rescale(+run: J.Val, +before: U32, +now: U32) -> J.Val:
  rules.Run.rescale.go(U32.max(before, 1), U32.max(now, 1), run)

def rules.Run.rescale.go(+before: U32, +now: U32, +run: J.Val) -> J.Val:
  J.set(run, "enemies", J.JA{rules.Run.rescale.each(J.items(J.get(run, "enemies")), before, now, stakes.party_fury(before), stakes.party_fury(now))})

def rules.Run.rescale.each(+es: List<&2, J.Val>, +before: U32, +now: U32, +was: U32, +nowf: U32) -> List<&2, J.Val>:
  match es:
    case Nil{}:
      Nil{}
    case Con{+e, +t}:
      rules.Run.rescale.one(e, before, now, was, nowf) <> rules.Run.rescale.each(t, before, now, was, nowf)

def rules.Run.rescale.one(+e: J.Val, +before: U32, +now: U32, +was: U32, +nowf: U32) -> J.Val:
  rules.Run.rescale.one.go(U32.mul(monsters.Enemy.base_hp(e), now), e, before, now, was, nowf)

def rules.Run.rescale.one.go(+max_hp: U32, +e: J.Val, +before: U32, +now: U32, +was: U32, +nowf: U32) -> J.Val:
  +hp = RS.u_clamp(core.div_ceil(U32.mul(core.u(e, "hp"), now), before), 1, max_hp)
  +f = core.uadd(core.usub(core.u(e, "fury"), was), nowf)
  core.set_u(core.set_u(core.set_u(e, "max_hp", max_hp), "hp", hp), "fury", f)

def rules.Run.advance(+run: J.Val, +inputs: J.Val) -> J.Val:
  rules.Run.advance.go(rules.Run.active(run), run, inputs)

def rules.Run.advance.go(live: Bool, +run: J.Val, +inputs: J.Val) -> J.Val:
  match live:
    case False{}:
      run
    case True{}:
      rules.Run.advance.show(deathmatch.Run.deathmatch_showing(run), run, inputs)

def rules.Run.advance.show(showing: Bool, +run: J.Val, +inputs: J.Val) -> J.Val:
  match showing:
    case True{}:
      core.tick_inc(deathmatch.Run.tick_deathmatch(run, inputs))
    case False{}:
      hitstop.Run.finish_hitstop(fx.Run.present(rules.Run.play(run, inputs)))

# `self.tick += 1` (u64).
def core.tick_inc(+run: J.Val) -> J.Val:
  J.set(run, "tick", W.to_j(W.add_u32(W.of_j(J.get(run, "tick")), 1)))
