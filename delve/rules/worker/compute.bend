# The worker's compute contracts by name (delve/main.bend `compute`); each
# `compute.delve.<contract>` answers {"Ok": result} or {"Err": text}.

def compute.run(+contract: String, +input: J.Val) -> J.Val:
  compute.run.scroll(String.eq(contract, "delve.scroll"), contract, input)

def compute.run.scroll(hit: Bool, +contract: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      compute.delve.scroll(input)
    case False{}:
      compute.run.paint(String.eq(contract, "delve.card-paint"), contract, input)

def compute.run.paint(hit: Bool, +contract: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      compute.delve.card_paint(input)
    case False{}:
      compute.run.home(String.eq(contract, "delve.home"), contract, input)

def compute.run.home(hit: Bool, +contract: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      compute.delve.home(input)
    case False{}:
      compute.run.brew(String.eq(contract, "delve.brew"), contract, input)

def compute.run.brew(hit: Bool, +contract: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      compute.delve.brew(input)
    case False{}:
      compute.run.reforge(String.eq(contract, "delve.reforge"), contract, input)

def compute.run.reforge(hit: Bool, +contract: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      compute.delve.reforge(input)
    case False{}:
      compute.run.wish(String.eq(contract, "delve.wish"), contract, input)

def compute.run.wish(hit: Bool, +contract: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      compute.delve.wish(input)
    case False{}:
      J.set(J.obj(), "Err", J.JS{"unsupported Delve computation contract"})
