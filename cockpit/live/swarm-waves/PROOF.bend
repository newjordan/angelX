import Base
import ./LAWS.bend as Laws

def Laws.width_stop_answers_first(waves_hit, nothing_back, novel_low, dissent_agrees, dissent_stable, room, next_raw):
  match waves_hit nothing_back novel_low dissent_agrees dissent_stable:
    case True{} _ _ _ _:
      {==}
    case False{} _ _ _ _:
      {==}

def Laws.dominant_pool_relaxes(escalate):
  match escalate:
    case True{}:
      {==}
    case False{}:
      {==}
