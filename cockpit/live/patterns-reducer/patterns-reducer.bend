import Base

# What one piece of field evidence does to the aggregate it targets: which
# membership counter it bumps, which outcome counter it bumps, and the
# uncertainty and cost it adds. Rust keeps every id and every ordering — the
# field/benchmark bucket keys, the eight-field pattern keys, the canonical and
# promoted index selection and all validation — and hands this leg the buckets
# and patterns in sorted order with each active piece of evidence already
# sitting in its (bucket, pattern) slot. The fold below is the whole counter
# policy.
#
# `apply_evidence` and `apply_to_fields` in
# cockpit/src/drive/competition/patterns_reducer.rs stay as the native oracle.
# Restated here, one piece of evidence
#   1. bumps exactly one membership counter:
#        0 confirmed official, 1 provisional transfer, 2 operational outcome
#   2. bumps exactly one outcome counter:
#        0 improvement, 1 tie, 2 regression, 3 failure, 4 timeout
#   3. adds `uncertainty_millis` and `cost_microunits` whole.
# Every counter is a u64 in Rust and U32 here, so each one travels as (hi, lo)
# and every add carries explicitly: a step that would leave the u64 range sets
# `over` instead of wrapping, which the host reads as a refused reduction.

type Count is Data:
  Count{hi: U32, lo: U32, over: Bool}

type Evidence is Data:
  Ev{kind: U32, outcome: U32, uncertainty: U32, cost_hi: U32, cost_lo: U32}

type Pattern is Data:
  Pat{evidence: List<&2, Evidence>}

type Bucket is Data:
  Bucket{patterns: List<&2, Pattern>}

type Aggregate is Data:
  Agg{confirmed_official: Count, provisional_transfer: Count, operational: Count, improvements: Count, ties: Count, regressions: Count, failures: Count, timeouts: Count, uncertainty_millis: Count, cost_microunits: Count}

type Reduced is Data:
  Reduced{patterns: List<&2, Aggregate>}

def both(+a: Bool, +b: Bool) -> Bool:
  match a:
    case True{}:
      b
    case False{}:
      False{}

def either(+a: Bool, +b: Bool) -> Bool:
  match a:
    case True{}:
      True{}
    case False{}:
      b

# `lo + b_lo > 4294967295`, decided before anything is added so no step has to
# rely on a wrapped sum.
def carries(+lo: U32, +b_lo: U32) -> Bool:
  U32.is_gt(lo, U32.sub(4294967295, b_lo))

# `lo + b_lo` reduced into one half; the carry branch subtracts the modulus
# instead of adding past it.
def low_sum(+lo: U32, +b_lo: U32, +carry: Bool) -> U32:
  match carry:
    case True{}:
      U32.sub(lo, U32.add(U32.sub(4294967295, b_lo), 1))
    case False{}:
      U32.add(lo, b_lo)

# One into the high half, refused only when it is already saturated.
def carried(+hi: U32, +carry: Bool) -> U32:
  match carry:
    case True{}:
      low_sum(hi, 1, U32.is_eq(hi, 4294967295))
    case False{}:
      hi

# `a + (b_hi, b_lo)` with Rust's `checked_add`: an out-of-range step raises
# `over` and leaves the halves where the last in-range step put them.
def added(+a: Count, +b_hi: U32, +b_lo: U32) -> Count:
  match a:
    case Count{+hi, +lo, +over}:
      +low_carry = carries(lo, b_lo)
      +high_carry = carries(hi, b_hi)
      +high = low_sum(hi, b_hi, high_carry)
      Count{
        carried(high, low_carry),
        low_sum(lo, b_lo, low_carry),
        either(
          over,
          either(high_carry, both(low_carry, U32.is_eq(high, 4294967295)))
        )
      }

def counted(+count: Count, +on: Bool) -> Count:
  match on:
    case True{}:
      added(count, 0, 1)
    case False{}:
      count

# One evidence row: one membership counter, one outcome counter, two sums.
def step(+aggregate: Aggregate, +row: Evidence) -> Aggregate:
  match aggregate row:
    case Agg{+confirmed_official, +provisional_transfer, +operational, +improvements, +ties, +regressions, +failures, +timeouts, +uncertainty_millis, +cost_microunits} Ev{+kind, +outcome, +uncertainty, +cost_hi, +cost_lo}:
      Agg{
            counted(confirmed_official, U32.is_eq(kind, 0)),
            counted(provisional_transfer, U32.is_eq(kind, 1)),
            counted(operational, U32.is_eq(kind, 2)),
            counted(improvements, U32.is_eq(outcome, 0)),
            counted(ties, U32.is_eq(outcome, 1)),
            counted(regressions, U32.is_eq(outcome, 2)),
            counted(failures, U32.is_eq(outcome, 3)),
            counted(timeouts, U32.is_eq(outcome, 4)),
            added(uncertainty_millis, 0, uncertainty),
            added(cost_microunits, cost_hi, cost_lo)
          }

def fold(+rows: List<&2, Evidence>, +aggregate: Aggregate) -> Aggregate:
  match rows:
    case Nil{}:
      aggregate
    case Con{+head, +tail}:
      fold(tail, step(aggregate, head))

def zero() -> Count:
  Count{0, 0, False{}}

# A pattern with no active evidence keeps Rust's `PatternAggregateV1::default`.
def empty() -> Aggregate:
  Agg{zero(), zero(), zero(), zero(), zero(), zero(), zero(), zero(), zero(), zero()}

def reduce_pattern(+pattern: Pattern) -> Aggregate:
  match pattern:
    case Pat{+evidence}:
      fold(evidence, empty())

def reduce_patterns(+patterns: List<&2, Pattern>) -> List<&2, Aggregate>:
  match patterns:
    case Nil{}:
      Nil{}
    case Con{+head, +tail}:
      Con{reduce_pattern(head), reduce_patterns(tail)}

def reduce_bucket(+bucket: Bucket) -> Reduced:
  match bucket:
    case Bucket{+patterns}:
      Reduced{reduce_patterns(patterns)}

# The host reads the result in plan order: one `Reduced` per bucket, one
# aggregate per pattern, exactly the shape it handed over.
def reduce(+buckets: List<&2, Bucket>) -> List<&2, Reduced>:
  match buckets:
    case Nil{}:
      Nil{}
    case Con{+head, +tail}:
      Con{reduce_bucket(head), reduce(tail)}
