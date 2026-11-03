# The worker's compute contracts (crates/delve-system/src/lib.rs `compute`):
# each `compute.delve.<contract>(input)` takes the request's `input` and answers
# {"Ok": result} or {"Err": text} as the Rust worker's reply `result` would —
# the result being the serialised policy answer, a refused request, or a
# violation of the contract.

# Refuse with `why` when there is one, else run `k`.
def compute.fail(+m: Maybe<&2, String>, k: Unit -> J.Val) -> J.Val:
  match m:
    case Some{+why}:
      home.util.err(why)
    case None{}:
      k(Unit{})

# ---- delve.scroll -----------------------------------------------------------

def compute.delve.scroll(+input: J.Val) -> J.Val:
  compute.fail(home.util.shape(input, "Request", ["facts", "said", "heard", "known", "whims"], ["facts", "said", "whims"]),
    u => compute.fail(scrolls.policy.Request.validate(input),
      u => compute.delve.scroll.decide(input)))

def compute.delve.scroll.decide(+input: J.Val) -> J.Val:
  compute.delve.scroll.check(input, scrolls.policy.Request.decide(input))

def compute.delve.scroll.check(+input: J.Val, +result: J.Val) -> J.Val:
  compute.fail(scrolls.policy.Request.validate_result(input, result),
    u => home.util.ok(result))

def case.compute.delve.scroll(+input: J.Val) -> J.Val:
  compute.delve.scroll(input)

# ---- delve.card-paint ---------------------------------------------------------

def compute.delve.card_paint(+input: J.Val) -> J.Val:
  compute.fail(home.util.shape(input, "Request", ["candidates"], ["candidates"]),
    u => compute.fail(card_paint.Request.validate(input),
      u => compute.delve.card_paint.check(input, card_paint.Request.evaluate(input))))

def compute.delve.card_paint.check(+input: J.Val, +decision: J.Val) -> J.Val:
  compute.fail(card_paint.Request.validate_result(input, decision),
    u => home.util.ok(decision))

def case.compute.delve.card_paint(+input: J.Val) -> J.Val:
  compute.delve.card_paint(input)

# ---- delve.wish -----------------------------------------------------------------

def compute.delve.wish(+input: J.Val) -> J.Val:
  compute.fail(home.util.shape(input, "PolicyRequest", ["work", "wish"], ["work", "wish"]),
    u => compute.fail(wish_jobs.PolicyRequest.validate(input),
      u => compute.delve.wish.check(input, wish_jobs.PolicyRequest.evaluate(input))))

def compute.delve.wish.check(+input: J.Val, +result: J.Val) -> J.Val:
  compute.fail(wish_jobs.PolicyRequest.validate_result(input, J.get(result, "Ok")),
    u => home.util.ok(result))

def case.compute.delve.wish(+input: J.Val) -> J.Val:
  compute.delve.wish(input)

# ---- delve.brew -----------------------------------------------------------------

def compute.delve.brew(+input: J.Val) -> J.Val:
  compute.fail(home.util.shape(input, "PolicyRequest", ["work", "card", "taken"], ["work", "card", "taken"]),
    u => compute.fail(brew_jobs.PolicyRequest.validate(input),
      u => compute.delve.brew.check(input, brew_jobs.PolicyRequest.evaluate(input))))

# `evaluate` answered {"Ok": result} or {"Err": why}; the worker then checks the result again.
def compute.delve.brew.check(+input: J.Val, +r: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(r, "Ok"),
    u => compute.fail(brew_jobs.PolicyRequest.validate_result(input, J.get(r, "Ok")), u => r),
    u => r)

def case.compute.delve.brew(+input: J.Val) -> J.Val:
  compute.delve.brew(input)

# ---- delve.reforge ----------------------------------------------------------------

def compute.delve.reforge(+input: J.Val) -> J.Val:
  compute.fail(home.util.shape(input, "PolicyRequest", ["work", "card"], ["work", "card"]),
    u => compute.fail(reforge_jobs.PolicyRequest.validate(input),
      u => compute.delve.reforge.check(input, reforge_jobs.PolicyRequest.evaluate(input))))

def compute.delve.reforge.check(+input: J.Val, +r: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(r, "Ok"),
    u => compute.fail(reforge_jobs.PolicyRequest.validate_result(input, J.get(r, "Ok")), u => r),
    u => r)

def case.compute.delve.reforge(+input: J.Val) -> J.Val:
  compute.delve.reforge(input)
