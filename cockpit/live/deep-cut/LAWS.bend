import Base
import ./deep-cut.bend as DeepCut

# Small, honest claims about the direction leg: the fallback holds when no
# direction is offered, i64 saturation is a real fixed point at both ends, an
# evidence class only counts under its own mark, and the pick follows score then
# rank. Nothing here claims more than the checker computes.

law empty_offer_keeps_the_fallback:
  for fallback_id: String
  {DeepCut.allocate(DeepCut.Offer{fallback_id, (Nil{} : List<&2, DeepCut.Direction>)}) == DeepCut.Decision{fallback_id, 2} : DeepCut.Decision}

law the_largest_score_saturates:
  {DeepCut.add_sat(DeepCut.i64_max(), DeepCut.Bits{0, 4}) == DeepCut.i64_max() : DeepCut.W64}

law the_smallest_score_saturates:
  {DeepCut.add_sat(DeepCut.i64_min(), DeepCut.Bits{4294967295, 4294967291}) == DeepCut.i64_min() : DeepCut.W64}

law a_transfer_needs_its_mark:
  for provisional: Bool
  {DeepCut.compatible(DeepCut.MarkedProvisionalTransfer{}, provisional) == provisional : Bool}

law target_evidence_refuses_a_transfer:
  {DeepCut.compatible(DeepCut.TargetEvidence{}, True{}) == False{} : Bool}

law a_silent_transfer_is_ineligible:
  {DeepCut.score_direction(DeepCut.Direction{"silent-transfer", 0, (Con{DeepCut.Use{True{}, True{}, True{}, DeepCut.TargetEvidence{}, DeepCut.Improvement{}, 0, 0}, Nil{}} : List<&2, DeepCut.Use>)}) == DeepCut.Scored{False{}, DeepCut.Bits{0, 0}, 2} : DeepCut.Scored}

law a_marked_transfer_is_eligible:
  {DeepCut.score_direction(DeepCut.Direction{"marked-transfer", 0, (Con{DeepCut.Use{True{}, True{}, True{}, DeepCut.MarkedProvisionalTransfer{}, DeepCut.Improvement{}, 0, 0}, Nil{}} : List<&2, DeepCut.Use>)}) == DeepCut.Scored{True{}, DeepCut.Bits{0, 4}, 1} : DeepCut.Scored}

law the_better_score_wins:
  {DeepCut.allocate(DeepCut.Offer{"fresh-direction", (Con{DeepCut.Direction{"low", 0, (Con{DeepCut.Use{True{}, True{}, False{}, DeepCut.TargetEvidence{}, DeepCut.Tie{}, 0, 0}, Nil{}} : List<&2, DeepCut.Use>)}, Con{DeepCut.Direction{"high", 1, (Con{DeepCut.Use{True{}, True{}, False{}, DeepCut.TargetEvidence{}, DeepCut.Improvement{}, 0, 0}, Nil{}} : List<&2, DeepCut.Use>)}, Nil{}}} : List<&2, DeepCut.Direction>)}) == DeepCut.Decision{"high", 0} : DeepCut.Decision}

law the_smaller_rank_breaks_a_tie:
  {DeepCut.allocate(DeepCut.Offer{"fresh-direction", (Con{DeepCut.Direction{"beta", 1, (Con{DeepCut.Use{True{}, True{}, False{}, DeepCut.TargetEvidence{}, DeepCut.Tie{}, 0, 0}, Nil{}} : List<&2, DeepCut.Use>)}, Con{DeepCut.Direction{"alpha", 0, (Con{DeepCut.Use{True{}, True{}, False{}, DeepCut.TargetEvidence{}, DeepCut.Tie{}, 0, 0}, Nil{}} : List<&2, DeepCut.Use>)}, Nil{}}} : List<&2, DeepCut.Direction>)}) == DeepCut.Decision{"alpha", 0} : DeepCut.Decision}
