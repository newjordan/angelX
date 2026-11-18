import Base
import ./rewards.bend as Rewards

law an_empty_ledger_selects_nothing:
  {Rewards.select(Nil{}, Nil{}) == Rewards.Absent{} : Rewards.Selection}

law a_binding_outranks_a_shaping_event:
  {Rewards.select(Con{True{}, Nil{}}, Con{True{}, Nil{}}) == Rewards.Official{0} : Rewards.Selection}

law the_newest_shaping_event_wins:
  {Rewards.select(Nil{}, Con{True{}, Con{False{}, Con{True{}, Nil{}}}}) == Rewards.Provisional{2} : Rewards.Selection}

law an_empty_shaping_ledger_appends:
  {Rewards.record_shaping(Nil{}) == Rewards.Append{} : Rewards.Shaping}

law a_same_id_row_that_differs_is_a_conflict:
  {Rewards.record_shaping(Con{Rewards.ShapingRow{True{}, False{}}, Nil{}}) == Rewards.Conflict{0} : Rewards.Shaping}
