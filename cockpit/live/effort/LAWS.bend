import Base
import ./effort.bend as Effort

law outside_levels_keep_native:
  for native: Maybe<&2, String>
  for allowed: List<&2, String>
  for facts: Effort.Facts
  for level: String
  for chose: {Effort.choose(Effort.Ask{native, allowed, facts}) == Some{level} : Maybe<&2, String>}
  for outside: {Effort.member(allowed, level) == False{} : Bool}
  {Effort.decide(Effort.Ask{native, allowed, facts}) == native : Maybe<&2, String>}

law inside_levels_are_kept:
  for native: Maybe<&2, String>
  for allowed: List<&2, String>
  for facts: Effort.Facts
  for level: String
  for chose: {Effort.choose(Effort.Ask{native, allowed, facts}) == Some{level} : Maybe<&2, String>}
  for inside: {Effort.member(allowed, level) == True{} : Bool}
  {Effort.decide(Effort.Ask{native, allowed, facts}) == Some{level} : Maybe<&2, String>}
