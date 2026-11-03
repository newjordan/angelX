import Base
import ./swarm-mandate.bend as Mandate

# A classifier call that failed can never take the single pass: the fan-out is
# the default, whatever the (empty) verdict text looked like.
law a_failed_classifier_never_says_tight:
  for saw_tight: Bool
  for saw_open: Bool
  {Mandate.classify(Mandate.Verdict{False{}, saw_tight, saw_open}) == Mandate.Open{} : Mandate.ClassifyOut}

# `ANGEL_SWARM_ALWAYS` is an override on its own: with a speed route that wants
# no deliberation and no research, the direct pass is exactly its complement.
law always_forces_the_swarm:
  for always: Bool
  {Mandate.pass(Mandate.Pass{False{}, False{}, True{}, True{}, False{}, False{}, always}) == Mandate.Passed{Bool.not(always)} : Mandate.PassOut}

# A clear advisor review returns the answer untouched.
law a_clear_review_is_never_annotated:
  for has_annotation: Bool
  {Mandate.annotate(Mandate.Note{True{}, has_annotation}) == Mandate.Annotate{False{}} : Mandate.AnnotateOut}

# A turn that does not ask for live context never routes live research, however
# the clubs and knobs are configured.
law a_turn_without_a_live_ask_never_routes_research:
  for research_enabled: Bool
  for researcher_present: Bool
  {Mandate.research_route(Mandate.Ask{False{}, research_enabled, researcher_present}) == Mandate.Route{False{}} : Mandate.ResearchOut}
