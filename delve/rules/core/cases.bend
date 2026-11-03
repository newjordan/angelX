# Case wrappers for the core group's recorded Rust cases
# (crates/delve/tests/rules/core_bend_cases__tests.rs): each unpacks `in`,
# calls the port and packs the answer in the shape of `out`.

def case.rules.Input.valid(+input: J.Val) -> J.Val:
  J.JBool{rules.Input.valid(input)}

def case.rules.Input.pressing(+input: J.Val) -> J.Val:
  J.JBool{rules.Input.pressing(input)}

def case.command.lay_over(+input: J.Val) -> J.Val:
  command.lay_over(J.get(input, "under"), J.get(input, "over"))

def case.rules.Run.active(+input: J.Val) -> J.Val:
  J.JBool{rules.Run.active(input)}

def case.again.Run.over(+input: J.Val) -> J.Val:
  J.JBool{again.Run.over(input)}

def case.rules.Run.floor(+input: J.Val) -> J.Val:
  J.JU{rules.Run.floor(input)}

def case.rules.Run.room(+input: J.Val) -> J.Val:
  rules.Run.room(input)

def case.rules.Run.at_entrance(+input: J.Val) -> J.Val:
  J.JBool{rules.Run.at_entrance(input)}

def case.rules.Run.foes_left(+input: J.Val) -> J.Val:
  J.JBool{rules.Run.foes_left(input)}

def case.rules.Run.party_len(+input: J.Val) -> J.Val:
  J.JU{rules.Run.party_len(input)}

def case.rules.Run.party_size(+input: J.Val) -> J.Val:
  J.JU{rules.Run.party_size(input)}

def case.rules.Run.scaled(+input: J.Val) -> J.Val:
  J.JU{rules.Run.scaled(input)}

def case.rules.Run.away(+input: J.Val) -> J.Val:
  J.JBool{rules.Run.away(J.get(input, "run"), core.u(input, "id"))}

# --- hero ---------------------------------------------------------------------

def case.hero.Hero.hurt(+input: J.Val) -> J.Val:
  hero.Hero.hurt(J.get(input, "h"), core.u(input, "damage"))

def case.hero.Hero.hurt_most(+input: J.Val) -> J.Val:
  hero.Hero.hurt_most(J.get(input, "h"), core.u(input, "damage"), core.u(input, "most"))

def cc.hero.Hero.blow.go(r: J.Val & Bool) -> J.Val:
  (+h, +struck) = r
  J.set(J.set(J.obj(), "h", h), "struck", J.JBool{struck})

def case.hero.Hero.blow(+input: J.Val) -> J.Val:
  cc.hero.Hero.blow.go(hero.Hero.blow(J.get(input, "h"), core.u(input, "damage"), core.u(input, "tag")))

# --- hitstop ------------------------------------------------------------------

# A run holding just the fields the hold reads: its state, its tune, its seats.
def cc.hitstop.run(+input: J.Val) -> J.Val:
  +r0 = J.set(J.obj(), "players", cc.hitstop.seats(J.items(J.get(input, "ids")), J.obj()))
  +r1 = cc.hitstop.opt(r0, J.get(input, "run"), "hitstop")
  cc.hitstop.opt(r1, J.get(input, "run"), "tune")

def cc.hitstop.opt(+o: J.Val, +src: J.Val, +k: String) -> J.Val:
  cc.hitstop.opt.go(J.has(src, k), o, src, k)

def cc.hitstop.opt.go(has: Bool, +o: J.Val, +src: J.Val, +k: String) -> J.Val:
  match has:
    case True{}:
      J.set(o, k, J.get(src, k))
    case False{}:
      o

def cc.hitstop.seats(+ids: List<&2, J.Val>, +acc: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      acc
    case Con{+id, +t}:
      cc.hitstop.seats(t, J.set(acc, U32.show(J.u32(id)), J.obj()))

def cc.hitstop.resume(r: J.Val & HitstopResume) -> J.Val:
  (+run, +res) = r
  cc.hitstop.pack(res, cc.hitstop.opt(J.obj(), run, "hitstop"))

def cc.hitstop.pack(res: HitstopResume, +state: J.Val) -> J.Val:
  match res:
    case HitWait{}:
      J.set(J.set(J.obj(), "state", state), "resume", J.JS{"wait"})
    case HitSame{}:
      J.set(J.set(J.obj(), "state", state), "resume", J.JS{"same"})
    case HitFresh{+map}:
      J.set(J.set(J.obj(), "state", state), "resume", J.set(J.obj(), "fresh", map))

def case.hitstop.State.inputs(+input: J.Val) -> J.Val:
  cc.hitstop.resume(hitstop.State.inputs(cc.hitstop.run(input), J.get(input, "inputs")))

def case.hitstop.Run.finish_hitstop(+input: J.Val) -> J.Val:
  +out = hitstop.Run.finish_hitstop(input)
  cc.hitstop.opt(core.set_u(J.obj(), "shake", core.u(out, "shake")), out, "hitstop")

# --- the step's small phases --------------------------------------------------

def case.rules.Run.count_idle(+input: J.Val) -> J.Val:
  +out = rules.Run.count_idle(J.get(input, "run"), J.get(input, "inputs"))
  J.set(J.obj(), "idle", J.get(out, "idle"))

def case.rules.Run.rescale_party(+input: J.Val) -> J.Val:
  +out = rules.Run.rescale_party(J.get(input, "run"), core.u(input, "before"))
  cc.hitstop.opt(J.set(J.obj(), "enemies", J.get(out, "enemies")), out, "scaled_for")

def case.fortune.Run.turbo(+input: J.Val) -> J.Val:
  J.JBool{fortune.Run.turbo(input)}

def case.fortune.Run.harden(+input: J.Val) -> J.Val:
  J.set(J.obj(), "players", J.get(fortune.Run.harden(input), "players"))

def case.fortune.Run.tick_collapse(+input: J.Val) -> J.Val:
  +out = fortune.Run.tick_collapse(input)
  +o0 = J.set(J.obj(), "rocks", J.get(out, "rocks"))
  +o1 = J.set(o0, "rng", J.get(out, "rng"))
  +o2 = J.set(o1, "fx", J.get(out, "fx"))
  J.set(o2, "cues", J.get(out, "cues"))

def case.trophies.Run.mark_trophies(+input: J.Val) -> J.Val:
  +out = trophies.Run.mark_trophies(J.get(input, "run"), core.u(input, "heard"))
  J.set(J.obj(), "marks", J.get(out, "marks"))

def case.fx.Run.show_fx(+input: J.Val) -> J.Val:
  +out = fx.Run.show_fx(input)
  J.set(J.set(J.set(J.obj(), "shake", J.get(out, "shake")), "fx", J.get(out, "fx")), "sparks", J.get(out, "sparks"))

def case.fx.Run.present(+input: J.Val) -> J.Val:
  +out = fx.Run.present(input)
  +o0 = J.set(J.set(J.set(J.obj(), "shake", J.get(out, "shake")), "fx", J.get(out, "fx")), "sparks", J.get(out, "sparks"))
  J.set(J.set(J.set(o0, "gore", J.get(out, "gore")), "feel", J.get(out, "feel")), "sounds", J.get(out, "sounds"))

def case.desperate.Run.mark_hits(+input: J.Val) -> J.Val:
  J.set(J.obj(), "enemies", J.get(desperate.Run.mark_hits(input), "enemies"))
