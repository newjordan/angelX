import Base

type Verdict is Data:
  Scored{decision: U32, mean_delta_bits: U32, lower_bound_bits: U32}
  Blank{decision: U32}

type Case is Data:
  Sample{complete: Bool, incumbent: List<&2, F32>, candidate: List<&2, F32>}

def total(+xs: List<&2, F32>, +acc: F32) -> F32:
  match xs:
    case Nil{}:
      acc
    case Con{head, tail}:
      total(tail, (acc + head : F32))

def count(+xs: List<&2, F32>, +acc: U32) -> U32:
  match xs:
    case Nil{}:
      acc
    case Con{+head, tail}:
      count(tail, (acc + 1 : U32))

def mean(+xs: List<&2, F32>) -> F32:
  (total(xs, (0.0 : F32)) / U32.to_f32(count(xs, 0)) : F32)

def sq_dev(+xs: List<&2, F32>, +m: F32, +acc: F32) -> F32:
  match xs:
    case Nil{}:
      acc
    case Con{head, tail}:
      +d = (head - m : F32)
      sq_dev(tail, m, (acc + (d * d : F32) : F32))

def variance(+xs: List<&2, F32>) -> F32:
  match xs:
    case Nil{}:
      (0.0 : F32)
    case Con{+head, +tail}:
      match tail:
        case Nil{}:
          (0.0 : F32)
        case Con{+more, +rest}:
          (sq_dev(xs, mean(xs), (0.0 : F32)) / U32.to_f32((count(xs, 0) - 1 : U32)) : F32)

def case_count(+cases: List<&2, Case>, +acc: U32) -> U32:
  match cases:
    case Nil{}:
      acc
    case Con{+head, tail}:
      case_count(tail, (acc + 1 : U32))

def all_complete(+cases: List<&2, Case>) -> Bool:
  match cases:
    case Nil{}:
      True{}
    case Con{head, tail}:
      match head:
        case Sample{complete, +incumbent, +candidate}:
          match complete:
            case True{}:
              all_complete(tail)
            case False{}:
              False{}

def choose(
  +under_floor: Bool,
  +regressed: Bool,
  +low: Bool,
  +mean_delta: F32,
  +lower_bound: F32
) -> Verdict:
  match under_floor:
    case True{}:
      Scored{4, F32.bits(mean_delta), F32.bits(lower_bound)}
    case False{}:
      match regressed:
        case True{}:
          Scored{5, F32.bits(mean_delta), F32.bits(lower_bound)}
        case False{}:
          match low:
            case True{}:
              Scored{6, F32.bits(mean_delta), F32.bits(lower_bound)}
            case False{}:
              Scored{0, F32.bits(mean_delta), F32.bits(lower_bound)}

def finish(
  +seen: U32,
  +delta_sum: F32,
  +var_sum: F32,
  +under_floor: Bool,
  +regressed: Bool,
  +z: F32,
  +min_delta: F32
) -> Verdict:
  +n = U32.to_f32(seen)
  +mean_delta = (delta_sum / n : F32)
  +mean_delta_variance = (var_sum / (n * n : F32) : F32)
  +lower_bound = (mean_delta - (z * F32.sqrt(mean_delta_variance) : F32) : F32)
  +low = (lower_bound < min_delta : F32)
  choose(under_floor, regressed, low, mean_delta, lower_bound)

def fold(
  +cases: List<&2, Case>,
  +samples: U32,
  +z: F32,
  +floor: F32,
  +max_regression: F32,
  +min_delta: F32,
  +seen: U32,
  +delta_sum: F32,
  +var_sum: F32,
  +under_floor: Bool,
  +regressed: Bool
) -> Verdict:
  match cases:
    case Nil{}:
      finish(seen, delta_sum, var_sum, under_floor, regressed, z, min_delta)
    case Con{head, tail}:
      match head:
        case Sample{+complete, +incumbent, +candidate}:
          +incumbent_mean = mean(incumbent)
          +candidate_mean = mean(candidate)
          +delta = (candidate_mean - incumbent_mean : F32)
          +n = U32.to_f32(samples)
          +delta_variance = (
            (variance(incumbent) / n : F32) + (variance(candidate) / n : F32) : F32)
          fold(
            tail,
            samples,
            z,
            floor,
            max_regression,
            min_delta,
            (seen + 1 : U32),
            (delta_sum + delta : F32),
            (var_sum + delta_variance : F32),
            Bool.or(under_floor, (candidate_mean < floor : F32)),
            Bool.or(regressed, (delta < F32.neg(max_regression) : F32))
          )

def gate(
  +empty: Bool,
  +undersized: Bool,
  +complete: Bool,
  +cases: List<&2, Case>,
  +samples: U32,
  +z: F32,
  +floor: F32,
  +max_regression: F32,
  +min_delta: F32
) -> Verdict:
  match empty:
    case True{}:
      Blank{1}
    case False{}:
      match undersized:
        case True{}:
          Blank{2}
        case False{}:
          match complete:
            case True{}:
              fold(
                cases, samples, z, floor, max_regression, min_delta, 0,
                (0.0 : F32), (0.0 : F32), False{}, False{})
            case False{}:
              Blank{3}

def judge(
  +cases: List<&2, Case>,
  +case_count: U32,
  +min_cases: U32,
  +samples: U32,
  +z: F32,
  +floor: F32,
  +max_regression: F32,
  +min_delta: F32
) -> Verdict:
  +empty = U32.is_eq(case_count, 0)
  +undersized = (case_count < min_cases : U32)
  +complete = all_complete(cases)
  gate(empty, undersized, complete, cases, samples, z, floor, max_regression, min_delta)
