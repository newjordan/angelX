import Base
import ./board-reducer.bend as Reducer

# These claims are about the disposition alone: no journal, no adapter call, no
# hashes and no canonical board. Each one is checked over literal feature
# records, and none of them assumes arithmetic beyond the comparisons the leg
# itself runs.

# The identity check runs first, so a duplicate id cannot hide a mismatched
# competition.
law a_mismatched_identity_conflicts_before_a_duplicate:
  {Reducer.decide(Reducer.Obs{True{}, True{}, False{}, False{}, 0, 0, False{}, 0, 0, True{}, True{}}) == Reducer.Conflict{} : Reducer.Verdict}

# A delta two past the canonical sequence cannot be contiguous, whatever the
# refresh state is: it is a gap.
law a_skipped_delta_detects_a_gap:
  {Reducer.decide(Reducer.Obs{False{}, False{}, False{}, True{}, 0, 5, True{}, 0, 7, False{}, True{}}) == Reducer.Gap{} : Reducer.Verdict}

# A full snapshot rebuilds the board, so a pending refresh never turns one into
# a gap.
law a_full_snapshot_is_never_a_gap:
  for refresh: Bool
  {Reducer.decide(Reducer.Obs{False{}, False{}, True{}, False{}, 0, 0, True{}, 0, 9, refresh, True{}}) == Reducer.Replace{} : Reducer.Verdict}

# An already-applied observation id is ignored before any sequence is compared:
# it stays a duplicate however far ahead its sequence claims to be.
law a_duplicate_observation_is_ignored_first:
  for refresh: Bool
  {Reducer.decide(Reducer.Obs{False{}, True{}, False{}, True{}, 0, 5, True{}, 0, 40, refresh, True{}}) == Reducer.Duplicate{} : Reducer.Verdict}
