# Case wrappers for lanternward (recorded from the Rust original).
#
# The recorders write {"fn", "in", "out"}. Session-level cases take {s, r}
# (and arguments) and answer {s, ev}; knight-level cases take {s, k, r, a} (the
# seat k taken out of the session, as Rust's step does) and answer
# {s, k, ev, ret}. Events are {sounds, stop, won}.

def lanternward.case.s2(+input: J.Val) -> J.Val:
  ga.sv(ga.sv(J.get(input, "s"), "$r", J.get(input, "r")), "$ev", lanternward.Events.default())

def lanternward.case.out2(+s: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "ev", J.get(s, "$ev")), "s", lanternward.Session.bare(s))

def lanternward.case.sk(+input: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(J.get(input, "s"), "$k", J.get(input, "k")), "$r", J.get(input, "r")), "$ev", lanternward.Events.default())

def lanternward.case.outk(+s: J.Val, +ret: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(ga.sv(J.obj(), "ret", ret), "ev", J.get(s, "$ev")), "k", J.get(s, "$k")), "s", lanternward.Session.bare(s))

def lanternward.case.outs(+s: J.Val, +ret: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(J.obj(), "ret", ret), "ev", J.get(s, "$ev")), "s", lanternward.Session.bare(s))

def lanternward.case.a(+input: J.Val) -> J.Val:
  J.get(input, "a")

def lanternward.case.pt(+v: J.Val) -> U32 & U32:
  (ga.nu(v, 0), ga.nu(v, 1))

# ---- session.rs ----

def case.lanternward.Session.new(+input: J.Val) -> J.Val:
  lanternward.Session.new(J.get(input, "players"))

def case.lanternward.Session.step(+input: J.Val) -> J.Val:
  lanternward.case.step.out(
    lanternward.Session.step(J.get(input, "s"), J.get(input, "inputs"), J.get(input, "heroes"), J.get(input, "r")))

def lanternward.case.step.out(+s: J.Val) -> J.Val:
  lanternward.case.out2(s)

def case.lanternward.Session.finale(+input: J.Val) -> J.Val:
  J.JBool{lanternward.Session.finale(J.get(input, "s"))}

def case.lanternward.Session.valid(+input: J.Val) -> J.Val:
  J.JBool{lanternward.Session.valid(J.get(input, "s"))}

def case.lanternward.Session.problem(+input: J.Val) -> J.Val:
  lanternward.Session.problem(J.get(input, "s"))

def case.lanternward.Session.repair(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.Session.repair(J.get(input, "s")))

def case.lanternward.Rules.tell_for(+input: J.Val) -> J.Val:
  J.JU{lanternward.Rules.tell_for(J.get(input, "r"), ga.u(input, "seats"))}

def case.lanternward.purse(+input: J.Val) -> J.Val:
  J.JU{lanternward.purse(ga.b(input, "first"), ga.u(input, "gems"))}

# ---- phases ----

def case.lanternward.foes.Session.provision(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.foes.Session.provision(lanternward.case.s2(input)))

def case.lanternward.kit.Session.step_seat(+input: J.Val) -> J.Val:
  lanternward.case.out2(
    lanternward.kit.Session.step_seat(lanternward.case.s2(input), ga.u(input, "id"), J.get(input, "inp")))

def case.lanternward.stage.Session.tick_props(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.tick_props(lanternward.case.s2(input)))

def case.lanternward.stage.Session.unstick(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.unstick(lanternward.case.s2(input)))

def case.lanternward.foes.Session.step_foes(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.foes.Session.step_foes(lanternward.case.s2(input)))

def case.lanternward.boss.Session.step_boss(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.boss.Session.step_boss(lanternward.case.s2(input)))

def case.lanternward.blow.Session.step_bolts(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.blow.Session.step_bolts(lanternward.case.s2(input)))

def case.lanternward.blow.Session.step_fuses(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.blow.Session.step_fuses(lanternward.case.s2(input)))

def case.lanternward.stage.Session.pickups(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.pickups(lanternward.case.s2(input)))

def case.lanternward.stage.Session.revive_pass(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.revive_pass(lanternward.case.s2(input)))

def case.lanternward.stage.Session.tick_stage(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.tick_stage(lanternward.case.s2(input)))

def case.lanternward.stage.Session.tick_wipe(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.tick_wipe(lanternward.case.s2(input)))

def case.lanternward.stage.Session.age(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.age(lanternward.case.s2(input)))

# ---- small public helpers on the session ----

def case.lanternward.props.Session.clear(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.clear(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "r"))}

def case.lanternward.props.Session.blocked(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.blocked(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"))}

def case.lanternward.props.Session.pit(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.pit(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"))}

def case.lanternward.props.Session.line_open(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.line_open(J.get(input, "s"),
    ga.nu(J.get(input, "a"), 0), ga.nu(J.get(input, "a"), 1), ga.nu(J.get(input, "b"), 0), ga.nu(J.get(input, "b"), 1))}

def case.lanternward.props.Session.burning(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.burning(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"))}

def case.lanternward.props.Session.secret_open(+input: J.Val) -> J.Val:
  J.JBool{lanternward.props.Session.secret_open(J.get(input, "s"))}

def case.lanternward.boss.Session.boss_exposed(+input: J.Val) -> J.Val:
  J.JBool{lanternward.boss.Session.boss_exposed(J.get(input, "s"))}

def case.lanternward.ctx.Session.beacon_ready(+input: J.Val) -> J.Val:
  J.JBool{lanternward.ctx.Session.beacon_ready(J.get(input, "s"))}

def case.lanternward.ctx.Session.burn_ticks(+input: J.Val) -> J.Val:
  J.JU{lanternward.ctx.Session.burn_ticks(J.get(input, "s"))}

def case.lanternward.stage.Session.exit_open(+input: J.Val) -> J.Val:
  J.JBool{lanternward.stage.Session.exit_open(J.get(input, "s"))}

def case.lanternward.ctx.Session.context(+input: J.Val) -> J.Val:
  lanternward.ctx.Session.context(J.get(input, "s"), J.get(input, "k"))

def case.lanternward.ctx.Session.carrying_boss_key(+input: J.Val) -> J.Val:
  J.JBool{lanternward.ctx.Session.carrying_boss_key(J.get(input, "s"), J.get(input, "k"))}

def case.lanternward.kit.Session.lock_point(+input: J.Val) -> J.Val:
  lanternward.kit.Session.lock_point(J.get(input, "s"), J.get(input, "l"))

def case.lanternward.foes.Session.nearest_knight(+input: J.Val) -> J.Val:
  lanternward.foes.Session.nearest_knight(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"))

def case.lanternward.stage.Session.start_of(+input: J.Val) -> J.Val:
  lanternward.stage.Session.start_of(J.get(input, "s"), ga.u(input, "id"))

def case.lanternward.foes.Session.nudge(+input: J.Val) -> J.Val:
  lanternward.foes.Session.nudge(J.get(input, "s"), ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "r"))

def case.lanternward.props.Session.roll(+input: J.Val) -> J.Val:
  lanternward.case.roll.out(lanternward.props.Session.roll(J.get(input, "s")), ga.u(input, "n"))

def lanternward.case.roll.out(+s: J.Val, +n: U32) -> J.Val:
  ga.sv(ga.su(J.obj(), "v", lanternward.props.Session.rolled(s, n)), "s", s)

def case.lanternward.props.Session.scatter(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.props.Session.scatter(J.get(input, "s"), ga.s(input, "kind"), ga.u(input, "x"), ga.u(input, "y")))

def case.lanternward.props.Session.fx(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.props.Session.fx(J.get(input, "s"), ga.s(input, "kind"), ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "a")))

def case.lanternward.props.Session.drop_at(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.props.Session.drop_at(J.get(input, "s"), ga.s(input, "kind"), ga.u(input, "x"), ga.u(input, "y")))

def case.lanternward.props.Session.solved(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.props.Session.solved(lanternward.case.sb(input), ga.u(input, "x"), ga.u(input, "y")))

def case.lanternward.props.Session.shatter(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.props.Session.shatter(lanternward.case.sb(input), ga.u(input, "i")))

def case.lanternward.props.Session.set_gate(+input: J.Val) -> J.Val:
  lanternward.case.set_gate.out(lanternward.props.Session.set_gate(J.get(input, "s"), ga.u(input, "id"), ga.b(input, "open")))

def lanternward.case.set_gate.out(+s: J.Val) -> J.Val:
  ga.sv(ga.sb(J.obj(), "changed", ga.b(s, "$ret")), "s", lanternward.Session.bare(s))

def lanternward.case.sb(+input: J.Val) -> J.Val:
  ga.sv(ga.sv(J.get(input, "s"), "$r", J.get(input, "r")), "$ev", lanternward.Events.default())

def case.lanternward.blow.Session.blow(+input: J.Val) -> J.Val:
  lanternward.case.blow.out(
    lanternward.blow.Session.blow(lanternward.case.sb(input), ga.u(input, "id"), ga.u(input, "dmg"),
      ga.nu(J.get(input, "from"), 0), ga.nu(J.get(input, "from"), 1), ga.s(input, "kind")))

def lanternward.case.blow.out(+s: J.Val) -> J.Val:
  ga.ss(lanternward.case.out2(s), "out", ga.s(s, "$ret"))

def case.lanternward.blow.Session.hurt_foe(+input: J.Val) -> J.Val:
  lanternward.case.hurt.out(
    lanternward.blow.Session.hurt_foe(
      ga.sv(ga.sv(J.get(input, "s"), "$ev", lanternward.Events.default()), "$r", J.obj()), ga.u(input, "i"), ga.u(input, "dmg"),
      ga.nu(J.get(input, "from"), 0), ga.nu(J.get(input, "from"), 1)))

def lanternward.case.hurt.out(+s: J.Val) -> J.Val:
  ga.sb(lanternward.case.out2(s), "hurt", ga.b(s, "$ret"))

def case.lanternward.boss.Session.hurt_boss(+input: J.Val) -> J.Val:
  lanternward.case.hurt.out(
    lanternward.boss.Session.hurt_boss(
      ga.sv(ga.sv(J.get(input, "s"), "$ev", lanternward.Events.default()), "$r", J.obj()), ga.u(input, "dmg"),
      ga.nu(J.get(input, "from"), 0), ga.nu(J.get(input, "from"), 1), ga.u(input, "owner")))

def case.lanternward.boss.Session.blast_boss(+input: J.Val) -> J.Val:
  lanternward.case.out2(
    lanternward.boss.Session.blast_boss(
      ga.sv(ga.sv(J.get(input, "s"), "$ev", lanternward.Events.default()), "$r", J.obj()),
      ga.nu(J.get(input, "at"), 0), ga.nu(J.get(input, "at"), 1)))

def case.lanternward.boss.Session.light_heart(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.boss.Session.light_heart(J.get(input, "s"), ga.u(input, "owner")))

def case.lanternward.boss.Session.reset_boss_phase(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.boss.Session.reset_boss_phase(J.get(input, "s")))

def case.lanternward.stage.Session.retry(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.stage.Session.retry(lanternward.case.s2(input)))

def case.lanternward.stage.Session.enter_room(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.Session.bare(lanternward.stage.Session.enter_room(lanternward.case.sb(input), ga.u(input, "room"))))

def case.lanternward.foes.Session.spawn_foe(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.foes.Session.spawn_foe(J.get(input, "s"), ga.s(input, "kind"), ga.u(input, "x"), ga.u(input, "y"), ga.b(input, "hooded")))

# Grant to the knight `id`, taken out of the table as the Rust did.
def case.lanternward.ctx.Session.grant(+input: J.Val) -> J.Val:
  lanternward.case.grant.go(ga.u(input, "id"), J.get(input, "item"), lanternward.case.sb(input))

def lanternward.case.grant.go(+id: U32, +item: J.Val, +s: J.Val) -> J.Val:
  lanternward.case.out2(
    lanternward.kit.put_k(
      lanternward.ctx.Session.grant(
        ga.sv(ga.sv(s, "$k", ga.at(J.get(s, "seats"), id)), "seats", J.del(J.get(s, "seats"), U32.show(id))), id, item),
      id))

def case.lanternward.Session.bot_input(+input: J.Val) -> J.Val:
  lanternward.bot.Session.bot_input(J.get(input, "s"), ga.u(input, "id"), J.get(input, "r"))

def case.lanternward.bot.Session.bot_input(+input: J.Val) -> J.Val:
  lanternward.bot.Session.bot_input(J.get(input, "s"), ga.u(input, "id"), J.get(input, "r"))

# ---- the Run hooks ----

def case.lanternward.Run.lanternward_showing(+input: J.Val) -> J.Val:
  J.JBool{lanternward.Run.lanternward_showing(J.get(input, "run"))}

def case.lanternward.Run.start_lanternward(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", lanternward.Run.start_lanternward(J.get(input, "run")))

def case.lanternward.Run.tick_lanternward(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", lanternward.Run.tick_lanternward(J.get(input, "run"), J.get(input, "inputs")))

def case.lanternward.Run.finish_lanternward(+input: J.Val) -> J.Val:
  lanternward.case.finish.out(lanternward.Run.finish_lanternward(J.get(input, "run"), J.get(input, "s")))

def lanternward.case.finish.out(+run: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "s", J.get(run, "$s")), "run", J.del(run, "$s"))

def case.lanternward.ADVENTURE.valid(+input: J.Val) -> J.Val:
  J.JBool{lanternward.ADVENTURE.valid(J.get(input, "run"))}

def case.lanternward.ADVENTURE.seated(+input: J.Val) -> J.Val:
  lanternward.ADVENTURE.seated(J.get(input, "run"))

def case.lanternward.ADVENTURE.bot(+input: J.Val) -> J.Val:
  lanternward.ADVENTURE.bot(J.get(input, "run"), ga.u(input, "id"))

def case.lanternward.ADVENTURE.clear(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", lanternward.ADVENTURE.clear(J.get(input, "run")))

def case.lanternward.ACTIVITY.on(+input: J.Val) -> J.Val:
  J.JBool{lanternward.ACTIVITY.on(J.get(input, "run"))}

def case.lanternward.ACTIVITY.ride(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", lanternward.ACTIVITY.ride(J.get(input, "run"), J.get(input, "inputs")))

def case.lanternward.ACTIVITY.end(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", lanternward.ACTIVITY.end(J.get(input, "run")))
