import Base
import ./patterns-reducer.bend as Reducer

# These claims are about the counter fold alone: no ids, no bucket ordering, no
# canonical index selection and no validation, because Rust owns all of those.
# Each one is checked against literal evidence rows.

# One operational tie adds exactly two counters and leaves the other eight.
law an_operational_tie_counts_only_its_two_counters:
  {Reducer.reduce_pattern(Reducer.Pat{Con{Reducer.Ev{2, 1, 0, 0, 0}, Nil{}}}) == Reducer.Agg{Reducer.Count{0, 0, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 1, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 1, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 0, False{}}} : Reducer.Aggregate}

# A saturated low half carries into the high half rather than losing the step.
law a_full_low_half_carries_into_the_high_half:
  {Reducer.counted(Reducer.Count{0, 4294967295, False{}}, True{}) == Reducer.Count{1, 0, False{}} : Reducer.Count}

# An exhausted u64 refuses the step instead of wrapping back to zero.
law an_exhausted_u64_refuses_instead_of_wrapping:
  {Reducer.counted(Reducer.Count{4294967295, 4294967295, False{}}, True{}) == Reducer.Count{0, 0, True{}} : Reducer.Count}
