import Base

# The competition director's choice of which research direction takes the next
# deep cut. Eligibility, scoring and the pick are decided here; the adapter
# supplies ids, a lexicographic rank, evidence class tags and each use's cost
# already reduced to whole units. A score is a signed 64-bit value in two words,
# so it saturates exactly where the native i64 does.

type W64 is Data:
  Bits{hi: U32, lo: U32}

type UseKind is Data:
  TargetEvidence{}
  MarkedProvisionalTransfer{}

type Outcome is Data:
  Improvement{}
  Tie{}
  Regression{}
  Failure{}
  Timeout{}

type Use is Data:
  Use{active: Bool, matches_target: Bool, provisional: Bool, kind: UseKind, outcome: Outcome, cost_hi: U32, cost_lo: U32}

type Direction is Data:
  Direction{id: String, rank: U32, uses: List<&2, Use>}

type Offer is Data:
  Offer{fallback_id: String, directions: List<&2, Direction>}

type Tally is Data:
  Tally{ok: Bool, points: W64, target: Bool, transfer: Bool, seen: Bool}

type Scored is Data:
  Scored{eligible: Bool, points: W64, hint: U32}

type Best is Data:
  Empty{}
  Chosen{total: W64, order: U32, why: U32, name: String}

type Decision is Data:
  Decision{selected: String, basis: U32}

# --- signed 64-bit words -----------------------------------------------------

def sign(hi: U32) -> Bool:
  U32.is_ge(hi, 2147483648)

def i64_max() -> W64:
  Bits{2147483647, 4294967295}

def i64_min() -> W64:
  Bits{2147483648, 0}

def saturate_by(+negative: Bool) -> W64:
  match negative:
    case True{}:
      i64_min()
    case False{}:
      i64_max()

def neg_finish(+hi: U32, +lo: U32, +wrapped: Bool) -> W64:
  match wrapped:
    case True{}:
      Bits{U32.add(hi, 1), lo}
    case False{}:
      Bits{hi, lo}

def neg(+w: W64) -> W64:
  match w:
    case Bits{hi, lo}:
      neg_finish(U32.not(hi), U32.add(U32.not(lo), 1), U32.is_eq(U32.add(U32.not(lo), 1), 0))

def add_result(+a_hi: U32, +lo: U32, +hi: U32, +over: Bool) -> W64:
  match over:
    case True{}:
      saturate_by(sign(a_hi))
    case False{}:
      Bits{hi, lo}

def add_done(+a_hi: U32, +b_hi: U32, +lo: U32, +hi: U32) -> W64:
  same = Bool.not(Bool.xor(sign(a_hi), sign(b_hi)))
  add_result(a_hi, lo, hi, Bool.and(same, Bool.xor(sign(a_hi), sign(hi))))

def add_hi(+a_hi: U32, +b_hi: U32, +lo: U32, +hi0: U32, +carry: Bool) -> W64:
  match carry:
    case True{}:
      add_done(a_hi, b_hi, lo, U32.add(hi0, 1))
    case False{}:
      add_done(a_hi, b_hi, lo, hi0)

def add_sat_b(+a_hi: U32, +a_lo: U32, +b: W64) -> W64:
  match b:
    case Bits{hi, lo}:
      add_hi(a_hi, hi, U32.add(a_lo, lo), U32.add(a_hi, hi), U32.is_lt(U32.add(a_lo, lo), a_lo))

def add_sat(+a: W64, +b: W64) -> W64:
  match a:
    case Bits{hi, lo}:
      add_sat_b(hi, lo, b)

def unsigned_above_flat(+a_hi: U32, +a_lo: U32, +b_hi: U32, +b_lo: U32, +flat: Bool) -> Bool:
  match flat:
    case True{}:
      U32.is_gt(a_lo, b_lo)
    case False{}:
      U32.is_gt(a_hi, b_hi)

def above_negative(+a_hi: U32, +a_lo: U32, +b_hi: U32, +b_lo: U32, +sb: Bool) -> Bool:
  match sb:
    case True{}:
      unsigned_above_flat(a_hi, a_lo, b_hi, b_lo, U32.is_eq(a_hi, b_hi))
    case False{}:
      False{}

def above_positive(+a_hi: U32, +a_lo: U32, +b_hi: U32, +b_lo: U32, +sb: Bool) -> Bool:
  match sb:
    case True{}:
      True{}
    case False{}:
      unsigned_above_flat(a_hi, a_lo, b_hi, b_lo, U32.is_eq(a_hi, b_hi))

def above_sign(+a_hi: U32, +a_lo: U32, +b_hi: U32, +b_lo: U32, +sa: Bool, +sb: Bool) -> Bool:
  match sa:
    case True{}:
      above_negative(a_hi, a_lo, b_hi, b_lo, sb)
    case False{}:
      above_positive(a_hi, a_lo, b_hi, b_lo, sb)

def above_both(+a_hi: U32, +a_lo: U32, +b_hi: U32, +b_lo: U32) -> Bool:
  above_sign(a_hi, a_lo, b_hi, b_lo, sign(a_hi), sign(b_hi))

def above_b(+a_hi: U32, +a_lo: U32, +b: W64) -> Bool:
  match b:
    case Bits{hi, lo}:
      above_both(a_hi, a_lo, hi, lo)

def above(+a: W64, +b: W64) -> Bool:
  match a:
    case Bits{hi, lo}:
      above_b(hi, lo, b)

def same_word_b(+a_hi: U32, +a_lo: U32, +b: W64) -> Bool:
  match b:
    case Bits{hi, lo}:
      Bool.and(U32.is_eq(a_hi, hi), U32.is_eq(a_lo, lo))

def same_word(+a: W64, +b: W64) -> Bool:
  match a:
    case Bits{hi, lo}:
      same_word_b(hi, lo, b)

# --- one direction's evidence ------------------------------------------------

def weight(o: Outcome) -> W64:
  match o:
    case Improvement{}:
      Bits{0, 4}
    case Tie{}:
      Bits{0, 1}
    case Regression{}:
      Bits{4294967295, 4294967292}
    case Failure{}:
      Bits{4294967295, 4294967291}
    case Timeout{}:
      Bits{4294967295, 4294967293}

def marked(+k: UseKind) -> Bool:
  match k:
    case TargetEvidence{}:
      False{}
    case MarkedProvisionalTransfer{}:
      True{}

def compatible(+k: UseKind, +p: Bool) -> Bool:
  match k:
    case TargetEvidence{}:
      Bool.not(p)
    case MarkedProvisionalTransfer{}:
      p

def step_paid(+points: W64, +target: Bool, +transfer: Bool, +kind: UseKind, +outcome: Outcome, +cost_hi: U32, +cost_lo: U32, +provisional: Bool) -> Tally:
  paid = add_sat(add_sat(points, weight(outcome)), neg(Bits{cost_hi, cost_lo}))
  Tally{True{}, paid, Bool.or(target, Bool.not(marked(kind))), Bool.or(transfer, Bool.and(marked(kind), provisional)), True{}}

def step_live(+points: W64, +target: Bool, +transfer: Bool, +kind: UseKind, +outcome: Outcome, +cost_hi: U32, +cost_lo: U32, +provisional: Bool, +live: Bool) -> Tally:
  match live:
    case False{}:
      Tally{False{}, points, target, transfer, True{}}
    case True{}:
      step_paid(points, target, transfer, kind, outcome, cost_hi, cost_lo, provisional)

def step_use(+ok: Bool, +points: W64, +target: Bool, +transfer: Bool, +u: Use) -> Tally:
  match u:
    case Use{active, matches_target, provisional, kind, outcome, cost_hi, cost_lo}:
      live = Bool.and(ok, Bool.and(active, Bool.and(matches_target, compatible(kind, provisional))))
      step_live(points, target, transfer, kind, outcome, cost_hi, cost_lo, provisional, live)

def step(+acc: Tally, +u: Use) -> Tally:
  match acc:
    case Tally{ok, points, target, transfer, seen}:
      step_use(ok, points, target, transfer, u)

def fold_uses(+uses: List<&2, Use>, +acc: Tally) -> Tally:
  match uses:
    case Nil{}:
      acc
    case Con{h, t}:
      fold_uses(t, step(acc, h))

def basis_flags(+target: Bool, +transfer: Bool) -> U32:
  match target:
    case True{}:
      0
    case False{}:
      match transfer:
        case True{}:
          1
        case False{}:
          2

def score_tally(+t: Tally) -> Scored:
  match t:
    case Tally{ok, points, target, transfer, seen}:
      settled = Bool.or(Bool.or(target, transfer), Bool.not(seen))
      Scored{Bool.and(ok, settled), points, basis_flags(target, transfer)}

def score_uses(+uses: List<&2, Use>) -> Scored:
  score_tally(fold_uses(uses, Tally{True{}, Bits{0, 0}, False{}, False{}, False{}}))

def score_direction(+d: Direction) -> Scored:
  match d:
    case Direction{id, rank, uses}:
      score_uses(uses)

# --- the pick ----------------------------------------------------------------

def promote_pick(+points: W64, +hint: U32, +id: String, +rank: U32, +total: W64, +order: U32, +why: U32, +name: String, +choose: Bool) -> Best:
  match choose:
    case True{}:
      Chosen{points, rank, hint, id}
    case False{}:
      Chosen{total, order, why, name}

def promote_choose(+points: W64, +hint: U32, +id: String, +rank: U32, +total: W64, +order: U32, +why: U32, +name: String) -> Best:
  choose = Bool.or(above(points, total), Bool.and(same_word(points, total), U32.is_lt(rank, order)))
  promote_pick(points, hint, id, rank, total, order, why, name, choose)

def promote(+candidate: Best, +points: W64, +hint: U32, +id: String, +rank: U32) -> Best:
  match candidate:
    case Empty{}:
      Chosen{points, rank, hint, id}
    case Chosen{total, order, why, name}:
      promote_choose(points, hint, id, rank, total, order, why, name)

def step_pick_eligible(+candidate: Best, +eligible: Bool, +points: W64, +hint: U32, +id: String, +rank: U32) -> Best:
  match eligible:
    case False{}:
      candidate
    case True{}:
      promote(candidate, points, hint, id, rank)

def step_pick_rank(+candidate: Best, +s: Scored, +id: String, +rank: U32) -> Best:
  match s:
    case Scored{eligible, points, hint}:
      step_pick_eligible(candidate, eligible, points, hint, id, rank)

def step_pick_scored(+candidate: Best, +h: Direction, +s: Scored) -> Best:
  match h:
    case Direction{id, rank, uses}:
      step_pick_rank(candidate, s, id, rank)

def step_pick(+candidate: Best, +h: Direction) -> Best:
  step_pick_scored(candidate, h, score_direction(h))

def pick(+rest: List<&2, Direction>, +candidate: Best) -> Best:
  match rest:
    case Nil{}:
      candidate
    case Con{h, t}:
      pick(t, step_pick(candidate, h))

def allocate_winner(+fallback_id: String, +winner: Best) -> Decision:
  match winner:
    case Empty{}:
      Decision{fallback_id, 2}
    case Chosen{total, order, why, name}:
      Decision{name, why}

def allocate(+offer: Offer) -> Decision:
  match offer:
    case Offer{fallback_id, directions}:
      allocate_winner(fallback_id, pick(directions, Empty{}))
