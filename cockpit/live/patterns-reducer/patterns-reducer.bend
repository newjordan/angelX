import Base

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

def carries(+lo: U32, +b_lo: U32) -> Bool:
  U32.is_gt(lo, U32.sub(4294967295, b_lo))

def low_sum(+lo: U32, +b_lo: U32, +carry: Bool) -> U32:
  match carry:
    case True{}:
      U32.sub(lo, U32.add(U32.sub(4294967295, b_lo), 1))
    case False{}:
      U32.add(lo, b_lo)

def carried(+hi: U32, +carry: Bool) -> U32:
  match carry:
    case True{}:
      low_sum(hi, 1, U32.is_eq(hi, 4294967295))
    case False{}:
      hi

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

def reduce(+buckets: List<&2, Bucket>) -> List<&2, Reduced>:
  match buckets:
    case Nil{}:
      Nil{}
    case Con{+head, +tail}:
      Con{reduce_bucket(head), reduce(tail)}
