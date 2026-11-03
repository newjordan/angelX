# Cases for altar.bend (recorded by altar_bend_cases__tests.rs).

def case.altar.stone_blocks(+input: J.Val) -> J.Val:
  J.JBool{altar.stone_blocks(ga.u(input, "c"), ga.u(input, "r"), ga.u(input, "tc"), ga.u(input, "tr"))}

def case.altar.keeps_an_altar(+input: J.Val) -> J.Val:
  J.JBool{altar.keeps_an_altar(J.get(input, "floor"), ga.u(input, "index"), ga.b(input, "identity"))}

def case.altar.place_on(+input: J.Val) -> J.Val:
  cx.altar.altar_at(altar.place_on(J.get(input, "floor"), ga.b(input, "identity")))

def cx.altar.altar_at(+floor: J.Val) -> J.Val:
  ga.sv(J.obj(), "altar_at", ga.pv(J.has(floor, "altar_at"), J.get(floor, "altar_at"), J.obj()))

def case.altar.place(+input: J.Val) -> J.Val:
  altar.place(J.get(input, "room"), J.get(input, "arrival"), J.get(input, "extra"))

def case.altar.spot(+input: J.Val) -> J.Val:
  altar.spot(J.get(input, "room"), J.get(input, "arrival"))

def case.altar.stone_cuts_a_passage(+input: J.Val) -> J.Val:
  J.JBool{altar.stone_cuts_a_passage(J.get(input, "room"), ga.u(input, "c"), ga.u(input, "r"))}

def case.altar.route_tiles(+input: J.Val) -> J.Val:
  altar.route_tiles(J.get(input, "room"), J.get(input, "from"), J.get(input, "extra"))

def case.altar.stone_stands_on_a_route(+input: J.Val) -> J.Val:
  J.JBool{altar.stone_stands_on_a_route(J.get(input, "on"), ga.u(input, "cols"), ga.u(input, "c"), ga.u(input, "r"))}

def case.altar.Run.altar_due(+input: J.Val) -> J.Val:
  J.JBool{altar.Run.altar_due(J.get(input, "run"))}

def case.altar.Run.wish_altar(+input: J.Val) -> J.Val:
  altar.Run.wish_altar(J.get(input, "run"))

def case.altar.Run.near_altar(+input: J.Val) -> J.Val:
  J.JBool{altar.Run.near_altar(J.get(input, "run"), ga.u(input, "id"))}

def case.altar.Run.choose_altars(+input: J.Val) -> J.Val:
  cx.altar.altar_at(J.get(altar.Run.choose_altars(J.get(input, "run")), "dungeon"))

def case.altar.Run.tick_wish_altar(+input: J.Val) -> J.Val:
  ga.pick(altar.Run.tick_wish_altar(J.get(input, "run"), J.get(input, "inputs")), ["players", "lamp", "found", "sounds", "dungeon"])

def case.altar.Run.lamp_sent(+input: J.Val) -> J.Val:
  ga.pick(altar.Run.lamp_sent(J.get(input, "run"), ga.u(input, "id"), ga.s(input, "words")), ["lamp"])

def case.altar.Run.lamp_landed(+input: J.Val) -> J.Val:
  ga.pick(altar.Run.lamp_landed(J.get(input, "run"), ga.u(input, "id"), ga.s(input, "words"), ga.s(input, "name")), ["lamp"])

def case.altar.Run.lamp_dropped(+input: J.Val) -> J.Val:
  ga.pick(altar.Run.lamp_dropped(J.get(input, "run"), ga.u(input, "id"), ga.s(input, "words")), ["lamp"])

def case.altar.Lamp.quiet(+input: J.Val) -> J.Val:
  J.JBool{altar.Lamp.quiet(J.get(input, "lamp"))}

def case.altar.Lamp.flaring(+input: J.Val) -> J.Val:
  altar.Lamp.flaring(J.get(input, "lamp"), W.of_j(J.get(input, "tick")))

def case.altar.Lamp.pending(+input: J.Val) -> J.Val:
  altar.Lamp.pending(J.get(input, "lamp"), ga.u(input, "id"))
