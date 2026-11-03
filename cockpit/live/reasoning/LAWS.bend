import Base
import ./reasoning.bend as Reasoning

# A disabled burn limit never demotes: the policy is opt-in, so the leg is the
# identity on that seat whatever the burn has reached.
law zero_limit_never_demotes:
  for levels: List<&2, String>
  for cumulative: U32
  for submitted: Bool
  for locked: Bool
  {Reasoning.demote(Reasoning.Burn{levels, cumulative, submitted, 0, locked}) == None{} : Maybe<&2, String>}

# A zero budget is "off", so it never expires a turn (the interactive default).
law zero_budget_never_expires:
  for elapsed: U32
  {Reasoning.expired(Reasoning.Budget{elapsed, 0}) == False{} : Bool}

# A seat that names no reasoning level is never given one, on intake or after.
law an_empty_seat_never_gets_an_effort:
  for effort: Maybe<&2, String>
  for friction: Bool
  {Reasoning.ladder(Reasoning.Ladder{Nil{}, effort, 0, friction}) == None{} : Maybe<&2, String>}
