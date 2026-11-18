import Base
import ./errors.bend as Errors

law veto_never_retries:
  for kind: Errors.Shape
  for status: U32
  for config: Bool
  for cert: Bool
  for auth: Bool
  for quota: Bool
  for rejected: Bool
  for context: Bool
  for answered: Bool
  for capped: Bool
  for attempt: U32
  for bounded: Bool
  for budget: U32
  for backoff: U32
  for cap: U32
  for elapsed: U32
  {Errors.retry_of(Errors.decide(Errors.Facts{kind, status, True{}, config, cert, auth, quota, rejected, context, answered, capped, attempt, bounded, budget, backoff, cap, elapsed})) == False{} : Bool}

law rejected_certificate_never_retries:
  for kind: Errors.Shape
  for status: U32
  for veto: Bool
  for config: Bool
  for auth: Bool
  for quota: Bool
  for rejected: Bool
  for context: Bool
  for answered: Bool
  for capped: Bool
  for attempt: U32
  for bounded: Bool
  for budget: U32
  for backoff: U32
  for cap: U32
  for elapsed: U32
  {Errors.retry_of(Errors.decide(Errors.Facts{kind, status, veto, config, True{}, auth, quota, rejected, context, answered, capped, attempt, bounded, budget, backoff, cap, elapsed})) == False{} : Bool}

law no_replay_no_wait:
  for backoff: U32
  for step: U32
  for cap: U32
  {Errors.wait(False{}, backoff, step, cap) == 0 : U32}
