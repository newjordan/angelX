# Case wrappers for lanternward/kit.rs private steps: {s, k, r, a} in,
# {s, k, ev, ret} out (see cases.bend).

def case.lanternward.kit.buttons(+input: J.Val) -> J.Val:
  J.JU{lanternward.kit.buttons(J.get(input, "i"))}

def case.lanternward.kit.Seat.new(+input: J.Val) -> J.Val:
  lanternward.kit.Seat.new(ga.u(input, "id"), ga.u(input, "joined"), ga.nu(J.get(input, "at"), 0), ga.nu(J.get(input, "at"), 1))

def case.lanternward.kit.Seat.valid(+input: J.Val) -> J.Val:
  J.JBool{lanternward.kit.Seat.valid(J.get(input, "k"), ga.u(input, "id"), ga.u(input, "elapsed"), ga.u(input, "w"), ga.u(input, "h"))}

def case.lanternward.kit.Seat.flaring(+input: J.Val) -> J.Val:
  J.JBool{lanternward.kit.Seat.flaring(J.get(input, "k"))}

def case.lanternward.kit.Seat.up(+input: J.Val) -> J.Val:
  J.JBool{lanternward.kit.Seat.up(J.get(input, "k"))}

def case.lanternward.kit.Session.recovering(+input: J.Val) -> J.Val:
  J.JBool{lanternward.kit.Session.recovering(J.get(input, "k"))}

def case.lanternward.kit.Session.targets(+input: J.Val) -> J.Val:
  J.JA{lanternward.kit.Session.targets(J.get(input, "s"))}

def lanternward.case.kn(+input: J.Val, +s: J.Val) -> J.Val:
  lanternward.case.outk(s, J.JNull{})

def case.lanternward.kit.Session.refill(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.refill(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.upkeep(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.upkeep(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.camera(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.camera(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.pick_target(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.case.sk(input), lanternward.kit.Session.pick_target(lanternward.case.sk(input), J.get(lanternward.case.sk(input), "$k"), J.JNull{}))

def case.lanternward.kit.Session.cycle_target(+input: J.Val) -> J.Val:
  lanternward.case.cycle(lanternward.case.sk(input), J.get(lanternward.case.a(input), "dir"))

def lanternward.case.cycle(+s: J.Val, +dir: J.Val) -> J.Val:
  lanternward.case.outk(s, lanternward.kit.Session.cycle_target(s, J.get(s, "$k"), ga.nu(dir, 0), ga.nu(dir, 1)))

def case.lanternward.kit.Session.lock_holds(+input: J.Val) -> J.Val:
  lanternward.case.holds(lanternward.case.sk(input), J.get(lanternward.case.a(input), "l"))

def lanternward.case.holds(+s: J.Val, +l: J.Val) -> J.Val:
  lanternward.case.outk(s, J.JBool{lanternward.kit.Session.lock_holds(s, J.get(s, "$k"), l)})

def case.lanternward.kit.Session.take_lock(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.take_lock(lanternward.case.sk(input), J.get(lanternward.case.a(input), "l")), J.JNull{})

def case.lanternward.kit.Session.lock_on(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.lock_on(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "mask"),
      ga.u(lanternward.case.a(input), "press"), J.get(lanternward.case.a(input), "inp")),
    J.JNull{})

def case.lanternward.kit.Session.face_and_move(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.face_and_move(lanternward.case.sk(input), J.get(lanternward.case.a(input), "inp")), J.JNull{})

def case.lanternward.kit.Session.shove(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.shove(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "dx"), ga.u(lanternward.case.a(input), "dy")), J.JNull{})

def case.lanternward.kit.Session.lean(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.lean(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "sx"), ga.u(lanternward.case.a(input), "sy")), J.JNull{})

def case.lanternward.kit.Session.orbit(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.orbit(lanternward.case.sk(input),
      ga.nu(J.get(lanternward.case.a(input), "t"), 0), ga.nu(J.get(lanternward.case.a(input), "t"), 1),
      ga.nu(J.get(lanternward.case.a(input), "v"), 0), ga.nu(J.get(lanternward.case.a(input), "v"), 1)),
    J.JNull{})

def case.lanternward.kit.Session.falling(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.falling(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.hurting(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.hurting(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.free(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.free(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "press"), J.get(lanternward.case.a(input), "inp")),
    J.JNull{})

def case.lanternward.kit.Session.swinging(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.swinging(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "press")),
    J.JNull{})

def case.lanternward.kit.Session.rolling(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.rolling(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.hopping(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.hopping(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "press")),
    J.JNull{})

def case.lanternward.kit.Session.plunging(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.plunging(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id")), J.JNull{})

def case.lanternward.kit.Session.lantern(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.lantern(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "mask")),
    J.JNull{})

def case.lanternward.kit.Session.verbs(+input: J.Val) -> J.Val:
  lanternward.case.outk(
    lanternward.kit.Session.verbs(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "mask"),
      ga.u(lanternward.case.a(input), "press"), J.get(lanternward.case.a(input), "inp")),
    J.JNull{})

def case.lanternward.kit.Session.begin_dodge(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.begin_dodge(lanternward.case.sk(input), J.get(lanternward.case.a(input), "inp")), J.JNull{})

def case.lanternward.kit.Session.begin_slash(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.begin_slash(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "step")), J.JNull{})

def case.lanternward.kit.Session.begin_lunge(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.begin_lunge(lanternward.case.sk(input)), J.JNull{})

def case.lanternward.kit.Session.begin_spin(+input: J.Val) -> J.Val:
  lanternward.case.outk(lanternward.kit.Session.begin_spin(lanternward.case.sk(input)), J.JNull{})

# sweep / lunge_hit leave whether they landed in `$ret`.
def lanternward.case.landed(+s: J.Val) -> J.Val:
  lanternward.case.outk(s, J.JBool{ga.b(s, "$ret")})

def case.lanternward.kit.Session.sweep(+input: J.Val) -> J.Val:
  lanternward.case.landed(
    lanternward.kit.Session.sweep(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id"), ga.u(lanternward.case.a(input), "reach"),
      ga.u(lanternward.case.a(input), "cos"), ga.u(lanternward.case.a(input), "dmg"), ga.b(lanternward.case.a(input), "heavy")))

def case.lanternward.kit.Session.lunge_hit(+input: J.Val) -> J.Val:
  lanternward.case.landed(lanternward.kit.Session.lunge_hit(lanternward.case.sk(input), ga.u(lanternward.case.a(input), "id")))
