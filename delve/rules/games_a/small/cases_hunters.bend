# Cases for hunters.bend (recorded by hunters_bend_cases__tests.rs).

def cx.hunters.empty_deeds() -> J.Val:
  ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(J.obj(), "hooks", J.JA{[]}), "webs", J.JA{[]}), "rot", J.JA{[]}), "spawns", J.JA{[]}), "cues", J.JA{[]})

def case.hunters.act(+input: J.Val) -> J.Val:
  cx.hunters.act.out(hunters.act(J.get(input, "enemy"), ga.xys(J.get(input, "heroes")),
    ga.grid(J.get(input, "room"), ga.b(input, "barred")), cx.hunters.empty_deeds()))

def cx.hunters.act.out(r: J.Val & J.Val & Bool) -> J.Val:
  (e, d, handled) = r
  ga.sb(ga.sv(ga.sv(J.obj(), "enemy", e), "deeds", d), "handled", handled)

def case.hunters.Run.tick_hunters(+input: J.Val) -> J.Val:
  ga.pick(hunters.Run.tick_hunters(J.get(input, "run")), ["hooks", "webs", "players", "room_hurt", "fx", "sounds", "cues", "feats"])

def case.hunters.Run.hunt(+input: J.Val) -> J.Val:
  ga.pick(hunters.Run.hunt(J.get(input, "run"), J.get(input, "hooks"), J.get(input, "webs"), J.get(input, "rot")),
    ["hooks", "webs", "players", "room_hurt"])

def case.hunters.Run.brood_full(+input: J.Val) -> J.Val:
  J.JBool{hunters.Run.brood_full(J.get(input, "run"))}

def case.hunters.Run.webbed(+input: J.Val) -> J.Val:
  J.JBool{hunters.Run.webbed(J.get(input, "webs"), ga.f(input, "x"), ga.f(input, "y"))}
