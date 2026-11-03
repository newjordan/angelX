# Case wrappers for the private steps of blow, foes, boss and stage.
# Session-level: {s, r, a} in, {s, ev, ret} out. (See cases.bend.)

def lanternward.case.ss(+input: J.Val) -> J.Val:
  ga.sv(ga.sv(J.get(input, "s"), "$r", J.get(input, "r")), "$ev", lanternward.Events.default())

# ---- blow ----

def case.lanternward.blow.Session.bolt_meets(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.blow.Session.bolt_meets(lanternward.case.ss(input), ga.u(lanternward.case.a(input), "i")), J.JNull{})

def case.lanternward.blow.Session.bolt_ends(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.blow.Session.bolt_ends(lanternward.case.ss(input), ga.u(lanternward.case.a(input), "i")), J.JNull{})

def case.lanternward.blow.Session.turn_bolt(+input: J.Val) -> J.Val:
  lanternward.case.outs(
    lanternward.blow.Session.turn_bolt(lanternward.case.ss(input), ga.u(lanternward.case.a(input), "i"), ga.u(lanternward.case.a(input), "id"), ga.b(lanternward.case.a(input), "parried")),
    J.JNull{})

def case.lanternward.blow.Session.light_foe(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.blow.Session.light_foe(lanternward.case.ss(input), ga.u(lanternward.case.a(input), "i")), J.JNull{})

# ---- foes ----

def case.lanternward.foes.Danger.hits(+input: J.Val) -> J.Val:
  J.JBool{lanternward.foes.Danger.hits(J.get(input, "d"), ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "rad"))}

def case.lanternward.foes.Foe.tell(+input: J.Val) -> J.Val:
  J.JU{lanternward.foes.Foe.tell(J.get(input, "f"), J.get(input, "r"), ga.u(input, "seats"))}

def case.lanternward.foes.Foe.vulnerable(+input: J.Val) -> J.Val:
  J.JBool{lanternward.foes.Foe.vulnerable(J.get(input, "f"))}

def case.lanternward.foes.Foe.new(+input: J.Val) -> J.Val:
  lanternward.foes.Foe.new(ga.u(input, "id"), ga.s(input, "kind"), ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "seats"))

def case.lanternward.foes.step_snuffer(+input: J.Val) -> J.Val:
  lanternward.case.snuffer.out(
    lanternward.foes.step_snuffer(
      ga.sv(ga.sv(J.obj(), "$f", J.get(input, "f")), "$strikes", J.JA{Nil{}}), J.get(input, "target"), ga.u(input, "index")))

def lanternward.case.snuffer.out(+s: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "strikes", J.get(s, "$strikes")), "f", J.get(s, "$f"))

def case.lanternward.foes.step_turret(+input: J.Val) -> J.Val:
  lanternward.case.turret.out(
    lanternward.foes.step_turret(
      ga.sv(ga.sv(ga.sv(ga.sv(J.obj(), "$f", J.get(input, "f")), "$shots", J.JA{Nil{}}), "$r", J.get(input, "r")), "$ev", lanternward.Events.default()),
      J.get(input, "target"), ga.u(input, "seats")))

def lanternward.case.turret.out(+s: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(J.obj(), "ev", J.get(s, "$ev")), "shots", J.get(s, "$shots")), "f", J.get(s, "$f"))

def case.lanternward.foes.Session.strike_with(+input: J.Val) -> J.Val:
  lanternward.case.out2(
    lanternward.foes.Session.strike_with(lanternward.case.s2(input), ga.u(input, "i"), J.get(input, "danger"), ga.u(input, "dmg")))

# ---- boss ----

def case.lanternward.boss.Warden.new(+input: J.Val) -> J.Val:
  lanternward.boss.Warden.new(ga.u(input, "x"), ga.u(input, "y"), ga.u(input, "seats"))

def case.lanternward.boss.Warden.valid(+input: J.Val) -> J.Val:
  J.JBool{lanternward.boss.Warden.valid(J.get(input, "w"))}

def case.lanternward.boss.Warden.need(+input: J.Val) -> J.Val:
  J.JU{lanternward.boss.Warden.need(J.get(input, "w"), ga.u(input, "seats"))}

def case.lanternward.boss.Warden.exposed(+input: J.Val) -> J.Val:
  J.JBool{lanternward.boss.Warden.exposed(J.get(input, "w"), ga.u(input, "now"), ga.u(input, "seats"))}

def case.lanternward.boss.Warden.heart(+input: J.Val) -> J.Val:
  ga.pair_i(lanternward.boss.Warden.heart.x(J.get(input, "w")), lanternward.boss.Warden.heart.y(J.get(input, "w")))

def lanternward.case.sw(+input: J.Val) -> J.Val:
  ga.sv(ga.sv(ga.sv(J.get(input, "s"), "$w", J.get(input, "w")), "$r", J.get(input, "r")), "$ev", lanternward.Events.default())

def lanternward.case.outw(+s: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "w", J.get(s, "$w")), "s", lanternward.Session.bare(s))

def case.lanternward.boss.Session.begin_attack(+input: J.Val) -> J.Val:
  lanternward.case.outw(
    lanternward.boss.Session.begin_attack(lanternward.case.sw(input), ga.nu(J.get(input, "at"), 0), ga.nu(J.get(input, "at"), 1)))

def case.lanternward.boss.Session.warden_hits(+input: J.Val) -> J.Val:
  lanternward.case.warden_hits.out(
    lanternward.boss.Session.warden_hits(
      ga.sb(lanternward.case.sw(input), "$parried", False{}), J.get(input, "line"), ga.u(input, "dmg"), ga.s(input, "kind")))

def lanternward.case.warden_hits.out(+s: J.Val) -> J.Val:
  ga.sb(ga.sv(ga.sv(ga.sv(J.obj(), "ev", J.get(s, "$ev")), "w", J.get(s, "$w")), "s", lanternward.Session.bare(s)), "parried", ga.b(s, "$parried"))

def case.lanternward.boss.Session.summon(+input: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", lanternward.Session.bare(lanternward.boss.Session.summon(ga.sv(J.get(input, "s"), "$w", J.get(input, "w")))))

def case.lanternward.boss.Session.prop_at(+input: J.Val) -> J.Val:
  lanternward.boss.Session.prop_at(J.get(input, "s"), ga.s(input, "kind"), ga.u(input, "tx"), ga.u(input, "ty"))

def case.lanternward.boss.Session.stage_warden(+input: J.Val) -> J.Val:
  lanternward.case.out2(lanternward.boss.Session.stage_warden(lanternward.case.s2(input)))

# ---- stage ----

def case.lanternward.stage.Session.stage_lamp_hall(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.stage_lamp_hall(lanternward.case.ss(input)), J.JNull{})

def case.lanternward.stage.Session.stage_gallery(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.stage_gallery(lanternward.case.ss(input)), J.JNull{})

def case.lanternward.stage.Session.stage_vault(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.stage_vault(lanternward.case.ss(input)), J.JNull{})

def case.lanternward.stage.Session.open_exit(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.open_exit(lanternward.case.ss(input)), J.JNull{})

def case.lanternward.stage.Session.provision_room(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.provision_room(lanternward.case.ss(input), ga.u(lanternward.case.a(input), "room")), J.JNull{})

def case.lanternward.stage.Session.spawn_wave(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.stage.Session.spawn_wave(lanternward.case.ss(input)), J.JNull{})

def case.lanternward.stage.Session.all_lit(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.case.ss(input), J.JBool{lanternward.stage.Session.all_lit(lanternward.case.ss(input), ga.s(lanternward.case.a(input), "kind"))})

def case.lanternward.stage.Session.foes_down(+input: J.Val) -> J.Val:
  lanternward.case.outs(lanternward.case.ss(input), J.JBool{lanternward.stage.Session.foes_down(lanternward.case.ss(input))})

def case.lanternward.stage.Session.prop_of(+input: J.Val) -> J.Val:
  lanternward.case.outs(
    lanternward.case.ss(input),
    lanternward.stage.Session.prop_of(lanternward.case.ss(input), ga.s(lanternward.case.a(input), "kind"), ga.u(lanternward.case.a(input), "id")))
