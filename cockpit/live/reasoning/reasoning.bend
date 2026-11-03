import Base

# Authored reasoning leg. Four native decisions live here, each a faithful port
# of a function that stays in Rust as the fallback and the parity oracle:
#
# - `ladder`    the adaptive per-hop effort: the configured (or highest)
#               baseline on intake and on any friction hop, the lowest viable
#               rung on a clean tool hop.
# - `demote`    the opt-in reasoning-burn demotion for locked-thinking seats:
#               once cumulative reasoning crosses the limit without a submit,
#               the next hop drops to `low`.
# - `milestone` the newest whole-million metered-input boundary a turn crossed.
# - `expired`   whether a turn has used up its wall-clock budget (`0` = off).
#
# Rust keeps the text work (which seats are GLM-locked, the club's identity) and
# hands this record numbers, booleans and the seat's own level list. Bend cannot
# scrutinize a computed value and forbids mutual recursion, so each comparison
# goes to a terminal helper that matches on it as a parameter, and every loop
# recurses on one constructor of the level list only.

type Ladder is Data:
  Ladder{levels: List<&2, String>, effort: Maybe<&2, String>, hop: U32, friction: Bool}

type Burn is Data:
  Burn{levels: List<&2, String>, cumulative: U32, submitted: Bool, limit: U32, locked: Bool}

type Crossing is Data:
  Crossing{previous_millions: U32, current_millions: U32}

type Budget is Data:
  Budget{elapsed: U32, budget: U32}

def first(levels: List<&2, String>) -> Maybe<&2, String>:
  match levels:
    case Nil{}:
      None{}
    case Con{head, tail}:
      Some{head}

def last(levels: List<&2, String>, +acc: String) -> String:
  match levels:
    case Nil{}:
      acc
    case Con{head, tail}:
      last(tail, head)

# `head` when `same`, else the hit the scan already found further down.
def here(same: Bool, +head: String, rest: Maybe<&2, String>) -> Maybe<&2, String>:
  match same:
    case True{}:
      Some{head}
    case False{}:
      rest

# The first level the seat accepts whose spelling matches `name`, keeping the
# seat's own spelling (the ladder hands it back to the provider verbatim).
def named(levels: List<&2, String>, +name: String) -> Maybe<&2, String>:
  match levels:
    case Nil{}:
      None{}
    case Con{+head, tail}:
      here(String.eq(String.to_lower(head), name), head, named(tail, name))

def here_high(is_top: Bool, +head: String, rest: Maybe<&2, String>) -> Maybe<&2, String>:
  match is_top:
    case True{}:
      Some{head}
    case False{}:
      rest

# The first `high`/`max` rung in the seat's own order.
def top(levels: List<&2, String>) -> Maybe<&2, String>:
  match levels:
    case Nil{}:
      None{}
    case Con{+head, tail}:
      here_high(Bool.or(String.eq(String.to_lower(head), "high"), String.eq(String.to_lower(head), "max")), head, top(tail))

def top_or(found: Maybe<&2, String>, levels: List<&2, String>) -> String:
  match found:
    case Some{+value}:
      value
    case None{}:
      last(levels, "")

# The configured effort, else the seat's highest rung, else its last one.
def baseline(effort: Maybe<&2, String>, +levels: List<&2, String>) -> String:
  match effort:
    case Some{+value}:
      value
    case None{}:
      top_or(top(levels), levels)

def low_only(found: Maybe<&2, String>, levels: List<&2, String>) -> Maybe<&2, String>:
  match found:
    case Some{+value}:
      Some{value}
    case None{}:
      first(levels)

def none_or(found: Maybe<&2, String>, levels: List<&2, String>) -> Maybe<&2, String>:
  match found:
    case Some{_}:
      Some{"none"}
    case None{}:
      first(levels)

def low_or(found: Maybe<&2, String>, +levels: List<&2, String>) -> Maybe<&2, String>:
  match found:
    case Some{_}:
      Some{"low"}
    case None{}:
      none_or(named(levels, "none"), levels)

# The lowest viable rung: `low` if the seat names it, else `none`, else its
# first level.
def downgrade(+levels: List<&2, String>) -> Maybe<&2, String>:
  low_or(named(levels, "low"), levels)

def nonempty(hit: Maybe<&2, String>, effort: Maybe<&2, String>, +levels: List<&2, String>) -> Maybe<&2, String>:
  match hit:
    case None{}:
      None{}
    case Some{_}:
      Some{baseline(effort, levels)}

def plan(base_hop: Bool, +levels: List<&2, String>, effort: Maybe<&2, String>) -> Maybe<&2, String>:
  match base_hop:
    case True{}:
      nonempty(first(levels), effort, levels)
    case False{}:
      downgrade(levels)

def ladder(ask: Ladder) -> Maybe<&2, String>:
  match ask:
    case Ladder{+levels, +effort, +hop, friction}:
      plan(Bool.or(U32.is_zero(hop), friction), levels, effort)

# `low` on the seat that names it, else the seat's first rung.
def demotion_level(+levels: List<&2, String>) -> Maybe<&2, String>:
  low_only(named(levels, "low"), levels)

def burn_locked(locked: Bool, levels: List<&2, String>) -> Maybe<&2, String>:
  match locked:
    case True{}:
      demotion_level(levels)
    case False{}:
      None{}

def burn_below(below: Bool, locked: Bool, levels: List<&2, String>) -> Maybe<&2, String>:
  match below:
    case True{}:
      None{}
    case False{}:
      burn_locked(locked, levels)

def burn_limit(disabled: Bool, cumulative: U32, limit: U32, locked: Bool, levels: List<&2, String>) -> Maybe<&2, String>:
  match disabled:
    case True{}:
      None{}
    case False{}:
      burn_below(U32.is_lt(cumulative, limit), locked, levels)

def burn_submitted(submitted: Bool, cumulative: U32, +limit: U32, locked: Bool, levels: List<&2, String>) -> Maybe<&2, String>:
  match submitted:
    case True{}:
      None{}
    case False{}:
      burn_limit(U32.is_zero(limit), cumulative, limit, locked, levels)

def demote(ask: Burn) -> Maybe<&2, String>:
  match ask:
    case Burn{+levels, cumulative, submitted, limit, +locked}:
      burn_submitted(submitted, cumulative, limit, locked, levels)

def crossed(ahead: Bool, current_millions: U32) -> Maybe<U32>:
  match ahead:
    case True{}:
      Some{current_millions}
    case False{}:
      None{}

# One receipt per crossed boundary, never one per million jumped.
def milestone(ask: Crossing) -> Maybe<U32>:
  match ask:
    case Crossing{previous_millions, +current_millions}:
      crossed(U32.is_gt(current_millions, previous_millions), current_millions)

def expired(ask: Budget) -> Bool:
  match ask:
    case Budget{elapsed, +budget}:
      Bool.and(Bool.not(U32.is_zero(budget)), U32.is_ge(elapsed, budget))
