import Base
import ./swarm-waves.bend as Waves

law width_stop_answers_first:
  for waves_hit: Bool
  for nothing_back: Bool
  for novel_low: Bool
  for dissent_agrees: Bool
  for dissent_stable: Bool
  for room: U32
  for next_raw: U32
  {Waves.wave_branch(True{}, waves_hit, nothing_back, novel_low, dissent_agrees, dissent_stable, room, next_raw) == Waves.Stop{Waves.MaxWidth{}} : Waves.WaveOut}

law dominant_pool_relaxes:
  for escalate: Bool
  {Waves.gate_of(True{}, escalate) == Waves.Relax{} : Waves.GateOut}
