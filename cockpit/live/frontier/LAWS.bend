import Base
import ./frontier.bend as Frontier

# These claims are about the selection alone: no comparator, no scores to
# canonicalise, no receipts. They are checked over literal boards.

# Nothing to select from means no seat is filled.
law an_empty_board_has_no_frontier:
  {Frontier.choose(Nil{}) == Frontier.Choice{Frontier.Empty{}, Frontier.Empty{}, Frontier.Empty{}} : Frontier.Choice}

# A lone row with no personal or source claim fills the global target only.
law a_lone_row_only_fills_the_global_seat:
  {Frontier.choose(Con{Frontier.Row{3, 7, 0, 0, 0, False{}, False{}}, Nil{}}) == Frontier.Choice{Frontier.Seat{3, 7, 0, 0, 0}, Frontier.Empty{}, Frontier.Empty{}} : Frontier.Choice}

# `tie_key` puts a missing rank last, so it loses a score tie even though its
# entry id sorts first: rank 9 beats no rank, id 1 notwithstanding.
law a_missing_rank_loses_a_score_tie:
  {Frontier.choose(Con{Frontier.Row{0, 5, 4294967295, 4294967295, 0, False{}, False{}}, Con{Frontier.Row{1, 5, 0, 9, 1, False{}, False{}}, Nil{}}}) == Frontier.Choice{Frontier.Seat{1, 5, 0, 9, 1}, Frontier.Empty{}, Frontier.Empty{}} : Frontier.Choice}
