import Base
import ./rewards.bend as Rewards

# These claims are about the row scans alone: no ids, receipts, scores or
# revisions to compare. They are checked over literal flag lists.

# Nothing recorded means nothing is effective.
law an_empty_ledger_selects_nothing:
  {Rewards.select(Nil{}, Nil{}) == Rewards.Absent{} : Rewards.Selection}

# A binding outranks a provisional shaping event for the same episode.
law a_binding_outranks_a_shaping_event:
  {Rewards.select(Con{True{}, Nil{}}, Con{True{}, Nil{}}) == Rewards.Official{0} : Rewards.Selection}

# When there is no binding, the newest shaping event is the effective reward.
law the_newest_shaping_event_wins:
  {Rewards.select(Nil{}, Con{True{}, Con{False{}, Con{True{}, Nil{}}}}) == Rewards.Provisional{2} : Rewards.Selection}

# With no shaping row recorded, a new event is appended rather than refused.
law an_empty_shaping_ledger_appends:
  {Rewards.record_shaping(Nil{}) == Rewards.Append{} : Rewards.Shaping}

# A row with the same shaping id but a different body is a conflicting identity.
law a_same_id_row_that_differs_is_a_conflict:
  {Rewards.record_shaping(Con{Rewards.ShapingRow{True{}, False{}}, Nil{}}) == Rewards.Conflict{0} : Rewards.Shaping}
