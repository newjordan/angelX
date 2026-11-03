import Base
import ./lib/j.bend as J
import ./build/rules.bend as R

# One apply of the rules: the Run checkpoint and the host's event
# ({inputs, settings, phrasebook, step}) in, the stepped checkpoint out. The
# Rust worker installs settings and the phrasebook as globals before stepping;
# here they ride on the run for the tick (`$settings`, `$phrasebook`) and are
# taken off before the reply. `rules.Run.step` is `Run::step`
# (crates/delve/src/rules.rs), assembled by delve/build.py from the fragments.

type Stepped is Data:
  Ran{run: J.Val}
  Refused{why: String}

def tick(+run: J.Val, +event: J.Val) -> J.Val:
  +lit = J.set(J.set(run, "$settings", J.get(event, "settings")), "$phrasebook", J.get(event, "phrasebook"))
  J.del(J.del(R.rules.Run.step(lit, J.get(event, "inputs")), "$settings"), "$phrasebook")

def stepped.if(refused: Bool, +checkpoint: J.Val, +after: J.Val) -> Stepped:
  match refused:
    case True{}:
      Refused{J.str(J.get(after, "$refused"))}
    case False{}:
      Ran{J.set(checkpoint, "run", after)}

def stepped(+checkpoint: J.Val, +after: J.Val) -> Stepped:
  stepped.if(J.has(after, "$refused"), checkpoint, after)

def step.if(go: Bool, +checkpoint: J.Val, +event: J.Val) -> Stepped:
  match go:
    case False{}:
      Ran{checkpoint}
    case True{}:
      stepped(checkpoint, tick(J.get(checkpoint, "run"), event))

def step(+checkpoint: J.Val, +event: J.Val) -> Stepped:
  step.if(J.bool(J.get(event, "step")), checkpoint, event)

# A computation the worker serves (`compute` in the protocol): `{"Ok": v}` or
# `{"Err": text}`.
def compute(+contract: String, +input: J.Val) -> J.Val:
  R.compute.run(contract, input)
