import Base

# Provider-failure policy for one failed request. The host reduces the error to
# the flags below and owns every piece of text matching; this module only
# decides. The host checks each verdict: it may be stricter than the native
# policy, never looser.
type Shape is Data:
  Status{}
  Transport{}
  Stream{}
  Empty{}
  Other{}

type Class is Data:
  Permanent{}
  Transient{}
  Quota{}
  Context{}

type Facts is Data:
  Facts{kind: Shape, status: U32, veto: Bool, config: Bool, cert: Bool, auth: Bool, quota: Bool, rejected: Bool, context: Bool, answered: Bool, capped: Bool, attempt: U32, bounded: Bool, budget: U32, backoff: U32, cap: U32, elapsed: U32}

type Verdict is Data:
  Verdict{class: Class, retry: Bool, delay: U32}

def retry_of(verdict: Verdict) -> Bool:
  match verdict:
    case Verdict{class, retry, delay}:
      retry

# A 4xx says the request itself is wrong, except the timeout, too-early and
# rate-limit statuses.
def client_fault(+status: U32) -> Bool:
  (status >= 400 : U32) && (status <= 499 : U32) && Bool.not(U32.is_eq(status, 408) || U32.is_eq(status, 425) || U32.is_eq(status, 429))

# The same request would fail the same way again.
def permanent(veto: Bool, config: Bool, cert: Bool, auth: Bool, quota: Bool, rejected: Bool, status: U32) -> Bool:
  veto || config || cert || auth || quota || rejected || client_fault(status)

def classify(context: Bool, quota: Bool, permanent: Bool) -> Class:
  match context quota permanent:
    case True{} _ _:
      Context{}
    case False{} True{} _:
      Quota{}
    case False{} False{} True{}:
      Permanent{}
    case False{} False{} False{}:
      Transient{}

def is_stream(kind: Shape) -> Bool:
  match kind:
    case Stream{}:
      True{}
    case _:
      False{}

def room(bounded: Bool, attempt: U32, budget: U32) -> Bool:
  match bounded:
    case False{}:
      True{}
    case True{}:
      (attempt < budget : U32)

# Text that was already shown is only replayed after a cut stream, where the
# display is retracted first.
def replay(permanent: Bool, answered: Bool, stream: Bool, capped: Bool, room: Bool) -> Bool:
  Bool.not(permanent) && (Bool.not(answered) || stream) && Bool.not(capped) && room

# min(backoff * step, cap) with no wrap-around.
def pace_over(over: Bool, backoff: U32, step: U32, cap: U32) -> U32:
  match over:
    case True{}:
      cap
    case False{}:
      (backoff * step : U32)

def pace_zero(zero: Bool, +backoff: U32, +step: U32, +cap: U32) -> U32:
  match zero:
    case True{}:
      0
    case False{}:
      pace_over((step > (cap / backoff : U32) : U32), backoff, step, cap)

def pace(+backoff: U32, +step: U32, +cap: U32) -> U32:
  pace_zero(U32.is_zero(backoff), backoff, step, cap)

def wait(retry: Bool, backoff: U32, step: U32, cap: U32) -> U32:
  match retry:
    case False{}:
      0
    case True{}:
      pace(backoff, step, cap)

def conclude(class: Class, +retry: Bool, backoff: U32, +attempt: U32, cap: U32) -> Verdict:
  Verdict{class, retry, wait(retry, backoff, (attempt + 1 : U32), cap)}

def settle(+perm: Bool, context: Bool, quota: Bool, stream: Bool, answered: Bool, capped: Bool, bounded: Bool, +attempt: U32, budget: U32, backoff: U32, cap: U32) -> Verdict:
  conclude(classify(context, quota, perm), replay(perm, answered, stream, capped, room(bounded, attempt, budget)), backoff, attempt, cap)

def decide(facts: Facts) -> Verdict:
  match facts:
    case Facts{kind, +status, veto, config, cert, auth, +quota, rejected, context, answered, capped, +attempt, bounded, budget, backoff, cap, elapsed}:
      settle(permanent(veto, config, cert, auth, quota, rejected, status), context, quota, is_stream(kind), answered, capped, bounded, attempt, budget, backoff, cap)
