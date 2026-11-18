import Base
import ./swarm-mandate.bend as Mandate

law a_failed_classifier_never_says_tight:
  for saw_tight: Bool
  for saw_open: Bool
  {Mandate.classify(Mandate.Verdict{False{}, saw_tight, saw_open}) == Mandate.Open{} : Mandate.ClassifyOut}

law always_forces_the_swarm:
  for always: Bool
  {Mandate.pass(Mandate.Pass{False{}, False{}, True{}, True{}, False{}, False{}, always}) == Mandate.Passed{Bool.not(always)} : Mandate.PassOut}

law a_clear_review_is_never_annotated:
  for has_annotation: Bool
  {Mandate.annotate(Mandate.Note{True{}, has_annotation}) == Mandate.Annotate{False{}} : Mandate.AnnotateOut}

law a_turn_without_a_live_ask_never_routes_research:
  for research_enabled: Bool
  for researcher_present: Bool
  {Mandate.research_route(Mandate.Ask{False{}, research_enabled, researcher_present}) == Mandate.Route{False{}} : Mandate.ResearchOut}
