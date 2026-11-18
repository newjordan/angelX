import Base
import ./board-reducer.bend as Reducer

law a_mismatched_identity_conflicts_before_a_duplicate:
  {Reducer.decide(Reducer.Obs{True{}, True{}, False{}, False{}, 0, 0, False{}, 0, 0, True{}, True{}}) == Reducer.Conflict{} : Reducer.Verdict}

law a_skipped_delta_detects_a_gap:
  {Reducer.decide(Reducer.Obs{False{}, False{}, False{}, True{}, 0, 5, True{}, 0, 7, False{}, True{}}) == Reducer.Gap{} : Reducer.Verdict}

law a_full_snapshot_is_never_a_gap:
  for refresh: Bool
  {Reducer.decide(Reducer.Obs{False{}, False{}, True{}, False{}, 0, 0, True{}, 0, 9, refresh, True{}}) == Reducer.Replace{} : Reducer.Verdict}

law a_duplicate_observation_is_ignored_first:
  for refresh: Bool
  {Reducer.decide(Reducer.Obs{False{}, True{}, False{}, True{}, 0, 5, True{}, 0, 40, refresh, True{}}) == Reducer.Duplicate{} : Reducer.Verdict}
