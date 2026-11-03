import Base
import ./effort.bend as Effort
import ./LAWS.bend as Laws

def Laws.outside_levels_keep_native(native, allowed, facts, level, chose, outside):
  %Equal.sym(Maybe<&2, String>, Effort.choose(Effort.Ask{native, allowed, facts}), Some{level}, chose) : {Effort.guard(allowed, _, native) == native : Maybe<&2, String>}
  %Equal.sym(Bool, Effort.member(allowed, level), False{}, outside) : {Effort.keep(_, Some{level}, native) == native : Maybe<&2, String>}
  {==}

def Laws.inside_levels_are_kept(native, allowed, facts, level, chose, inside):
  %Equal.sym(Maybe<&2, String>, Effort.choose(Effort.Ask{native, allowed, facts}), Some{level}, chose) : {Effort.guard(allowed, _, native) == Some{level} : Maybe<&2, String>}
  %Equal.sym(Bool, Effort.member(allowed, level), True{}, inside) : {Effort.keep(_, Some{level}, native) == Some{level} : Maybe<&2, String>}
  {==}
