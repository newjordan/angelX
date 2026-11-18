import Base
import ./promotion.bend as Promotion

law empty_cohort_never_promotes:
  for min_cases: U32
  for samples: U32
  for z: F32
  for floor: F32
  for max_regression: F32
  for min_delta: F32
  {Promotion.judge(Nil{}, 0, min_cases, samples, z, floor, max_regression, min_delta) == Promotion.Blank{1} : Promotion.Verdict}

law one_case_cannot_satisfy_a_two_case_cohort:
  for incumbent: List<&2, F32>
  for candidate: List<&2, F32>
  {Promotion.judge(Con{Promotion.Sample{True{}, incumbent, candidate}, Nil{}}, 1, 2, 2, 1.0, 0.0, 0.0, 0.1) == Promotion.Blank{2} : Promotion.Verdict}
