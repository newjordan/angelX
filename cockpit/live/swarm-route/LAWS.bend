import Base
import ./swarm-route.bend as SwarmRoute

law moa_summons_from_the_front:
  for moa_prefix: Bool
  {SwarmRoute.wants(SwarmRoute.Feat{False{}, False{}, 0, 0, 0, 0, False{}, False{}, moa_prefix, False{}, False{}, False{}, False{}, False{}}) == SwarmRoute.Route{False{}, moa_prefix, False{}, False{}, False{}, False{}} : SwarmRoute.Decisions}

law blank_turn_never_routes:
  for open_word: Bool
  {SwarmRoute.wants(SwarmRoute.Feat{True{}, True{}, 900, 900, 4, 4, True{}, True{}, True{}, True{}, open_word, True{}, True{}, True{}}) == SwarmRoute.Route{True{}, True{}, False{}, False{}, False{}, True{}} : SwarmRoute.Decisions}
