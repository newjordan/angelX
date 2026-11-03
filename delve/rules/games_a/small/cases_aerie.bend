# Cases for aerie.bend (recorded by aerie_bend_cases__tests.rs).

def case.aerie.pigeon_at(+input: J.Val) -> J.Val:
  cx.aerie.xy(aerie.pigeon_at(J.get(input, "room"), ga.u(input, "i"), W.of_j(J.get(input, "tick"))))

def cx.aerie.xy(+p: F32 & F32) -> J.Val:
  (x, y) = p
  ga.pair_f(x, y)

def case.aerie.flying(+input: J.Val) -> J.Val:
  cx.aerie.index(aerie.flying(ga.s(input, "kind"), ga.u(input, "stage"), ga.u(input, "papers"), J.get(input, "pigeons")))

def cx.aerie.index(+i: U32) -> J.Val:
  ga.pv(U32.is_ge(i, 3), J.JNull{}, J.JU{i})

def case.aerie.progress(+input: J.Val) -> J.Val:
  cx.aerie.pair(aerie.progress(J.get(input, "country"), J.get(input, "marks")))

def cx.aerie.pair(+p: U32 & U32) -> J.Val:
  (a, b) = p
  J.JA{[J.JU{a}, J.JU{b}]}

def case.aerie.shape(+input: J.Val) -> J.Val:
  aerie.shape(J.get(input, "room"))

def case.aerie.Pigeons.valid(+input: J.Val) -> J.Val:
  J.JBool{aerie.Pigeons.valid(J.get(input, "pigeons"))}

def case.aerie.Run.tick_aerie(+input: J.Val) -> J.Val:
  ga.pick(aerie.Run.tick_aerie(J.get(input, "run"), J.get(input, "inputs")), ["players", "pigeons", "marks", "found", "sounds", "projectiles"])
