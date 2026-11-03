# Cases for ledge.bend (recorded by ledge_bend_cases__tests.rs).

def case.ledge.walk(+input: J.Val) -> J.Val:
  cx.ledge.walk.out(ledge.walk(J.get(input, "hero"), J.get(input, "input"),
    ga.grid(J.get(input, "room"), ga.b(input, "barred")), ga.f(input, "dx"), ga.b(input, "rolling")))

def cx.ledge.walk.out(r: J.Val & Bool) -> J.Val:
  (hero, fell) = r
  ga.sb(ga.sv(J.obj(), "hero", hero), "fell", fell)

def case.ledge.fall(+input: J.Val) -> J.Val:
  ledge.fall(J.get(input, "enemy"), ga.grid(J.get(input, "room"), ga.b(input, "barred")))

def case.ledge.stand_on(+input: J.Val) -> J.Val:
  J.JF{ledge.stand_on(J.get(input, "room"), ga.f(input, "x"), ga.f(input, "from"))}

def case.ledge.Run.side_on(+input: J.Val) -> J.Val:
  J.JBool{ledge.Run.side_on(J.get(input, "run"))}

def case.ledge.Run.arrive_side_on(+input: J.Val) -> J.Val:
  ga.pick(ledge.Run.arrive_side_on(J.get(input, "run")), ["players"])

def case.ledge.Run.perch(+input: J.Val) -> J.Val:
  cx.ledge.Run.perch.out(ledge.Run.perch(J.get(input, "run"), ga.b(input, "flies")))

def cx.ledge.Run.perch.out(r: J.Val & (F32 & F32)) -> J.Val:
  (run, xy) = r
  cx.ledge.Run.perch.pack(J.get(run, "rng"), xy)

def cx.ledge.Run.perch.pack(+rng: J.Val, xy: F32 & F32) -> J.Val:
  (x, y) = xy
  ga.sv(ga.sv(J.obj(), "rng", rng), "pos", ga.pair_f(x, y))
