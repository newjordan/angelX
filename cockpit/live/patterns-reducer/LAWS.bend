import Base
import ./patterns-reducer.bend as Reducer

law an_operational_tie_counts_only_its_two_counters:
  {Reducer.reduce_pattern(Reducer.Pat{Con{Reducer.Ev{2, 1, 0, 0, 0}, Nil{}}}) == Reducer.Agg{Reducer.Count{0, 0, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 1, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 1, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 0, False{}}, Reducer.Count{0, 0, False{}}} : Reducer.Aggregate}

law a_full_low_half_carries_into_the_high_half:
  {Reducer.counted(Reducer.Count{0, 4294967295, False{}}, True{}) == Reducer.Count{1, 0, False{}} : Reducer.Count}

law an_exhausted_u64_refuses_instead_of_wrapping:
  {Reducer.counted(Reducer.Count{4294967295, 4294967295, False{}}, True{}) == Reducer.Count{0, 0, True{}} : Reducer.Count}
