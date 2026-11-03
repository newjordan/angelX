import Base

# The progressive-wave decision for one finished wave, the next-wave size
# clamp, and the scale gate for a clustered draft pool. The host resolves every
# float comparison natively (Bend has no f64: the novelty floor, the dissent
# thresholds and the entropy/dominant flags arrive as booleans, and the grown
# wave size arrives precomputed as an integer); the stop order, the expand
# choice and every integer clamp live here.

type Reason is Data:
  MaxWidth{}
  MaxWaves{}
  Failed{}
  LowNovelty{}
  Agree{}
  Stable{}

type Inputs is Data:
  In{wave_index: U32, max_waves: U32, launched_total: U32, max_width: U32, returned: U32, novel_low: Bool, dissent_agrees: Bool, dissent_stable: Bool, room: U32, next_raw: U32}

type WaveOut is Data:
  Stop{reason: Reason}
  Grow{next_wave: U32}

type GateOut is Data:
  Relax{}
  Escalate{}
  Hold{}

# The next wave never shrinks below one and never passes the room left.
def size_of(remaining: U32, next_raw: U32) -> U32:
  match remaining:
    case 0:
      0
    case _:
      U32.max(1, U32.min(next_raw, remaining))

def grow_of(remaining: U32, next_raw: U32) -> WaveOut:
  Grow{size_of(remaining, next_raw)}

def hits(launched_total: U32, max_width: U32) -> Bool:
  (U32.is_ge(launched_total, max_width) : Bool)

def done(wave_index: U32, max_waves: U32) -> Bool:
  (U32.is_ge(U32.add(wave_index, 1), max_waves) : Bool)

def empty(returned: U32) -> Bool:
  (U32.is_zero(returned) : Bool)

def wave_branch(width_hit: Bool, waves_hit: Bool, nothing_back: Bool, novel_low: Bool, dissent_agrees: Bool, dissent_stable: Bool, room: U32, next_raw: U32) -> WaveOut:
  match width_hit waves_hit nothing_back novel_low dissent_agrees dissent_stable:
    case True{} _ _ _ _ _:
      Stop{MaxWidth{}}
    case False{} True{} _ _ _ _:
      Stop{MaxWaves{}}
    case False{} False{} True{} _ _ _:
      Stop{Failed{}}
    case False{} False{} False{} True{} _ _:
      Stop{LowNovelty{}}
    case False{} False{} False{} False{} True{} _:
      Stop{Agree{}}
    case False{} False{} False{} False{} False{} True{}:
      Stop{Stable{}}
    case False{} False{} False{} False{} False{} False{}:
      grow_of(room, next_raw)

def wave_decide(+i: Inputs) -> WaveOut:
  match i:
    case In{wave_index, max_waves, launched_total, max_width, returned, novel_low, dissent_agrees, dissent_stable, room, next_raw}:
      wave_branch(hits(launched_total, max_width), done(wave_index, max_waves), empty(returned), novel_low, dissent_agrees, dissent_stable, room, next_raw)

# Relax wins as soon as one family dominates, else an entropy spike escalates,
# else the gate holds.
def gate_of(relax: Bool, escalate: Bool) -> GateOut:
  match relax escalate:
    case True{} _:
      Relax{}
    case False{} True{}:
      Escalate{}
    case False{} False{}:
      Hold{}
