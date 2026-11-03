# The worker's entry points into the rules, refusing until their groups land
# them (delve/GROUPS.md: core owns rules.Run.step, home the compute contracts).

def rules.Run.step(+run: J.Val, +inputs: J.Val) -> J.Val:
  J.set(run, "$refused", J.JS{"the Bend rules do not step a Run yet"})

def compute.delve.scroll(+input: J.Val) -> J.Val:
  J.set(J.obj(), "Err", J.JS{"delve.scroll is not ported yet"})

def compute.delve.card_paint(+input: J.Val) -> J.Val:
  J.set(J.obj(), "Err", J.JS{"delve.card-paint is not ported yet"})

def compute.delve.home(+input: J.Val) -> J.Val:
  J.set(J.obj(), "Err", J.JS{"delve.home is not ported yet"})

def compute.delve.brew(+input: J.Val) -> J.Val:
  J.set(J.obj(), "Err", J.JS{"delve.brew is not ported yet"})

def compute.delve.reforge(+input: J.Val) -> J.Val:
  J.set(J.obj(), "Err", J.JS{"delve.reforge is not ported yet"})

def compute.delve.wish(+input: J.Val) -> J.Val:
  J.set(J.obj(), "Err", J.JS{"delve.wish is not ported yet"})
