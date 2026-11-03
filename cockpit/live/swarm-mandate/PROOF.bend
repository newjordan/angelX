import Base
import ./LAWS.bend as Laws

def Laws.a_failed_classifier_never_says_tight(saw_tight, saw_open):
  match saw_tight saw_open:
    case True{} True{}:
      {==}
    case True{} False{}:
      {==}
    case False{} True{}:
      {==}
    case False{} False{}:
      {==}

def Laws.always_forces_the_swarm(always):
  match always:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.a_clear_review_is_never_annotated(has_annotation):
  match has_annotation:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.a_turn_without_a_live_ask_never_routes_research(research_enabled, researcher_present):
  match research_enabled researcher_present:
    case True{} True{}:
      {==}
    case True{} False{}:
      {==}
    case False{} True{}:
      {==}
    case False{} False{}:
      {==}
