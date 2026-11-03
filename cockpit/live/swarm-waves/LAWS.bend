import Base
import ./swarm-waves.bend as Waves

# The width ceiling outranks every later stop reason: once the fan-out budget
# is spent, the decision is Stop(MaxWidth) no matter what the other flags say.
law width_stop_answers_first:
  for waves_hit: Bool
  for nothing_back: Bool
  for novel_low: Bool
  for dissent_agrees: Bool
  for dissent_stable: Bool
  for room: U32
  for next_raw: U32
  {Waves.wave_branch(True{}, waves_hit, nothing_back, novel_low, dissent_agrees, dissent_stable, room, next_raw) == Waves.Stop{Waves.MaxWidth{}} : Waves.WaveOut}

# A pool with a dominant family always relaxes the gate, whatever the entropy
# spike says.
law dominant_pool_relaxes:
  for escalate: Bool
  {Waves.gate_of(True{}, escalate) == Waves.Relax{} : Waves.GateOut}
