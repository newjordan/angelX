import Base
import ./layout.bend as Layout

# What the leg answers. Every op the adapter may ask for gets one constructor
# here, so the entry stays a single match and the shape of an answer never
# depends on a string.
#
# King Cinder's shapes live here too, because the tick answers them: a knight's
# feet in fixed slots, his state as plain numbers, his hazards as a list a walk
# consumes once (Bend's `List` is a `Type`, not a `Data`, so a list is never
# copied — the tick filters his hazards and builds the new ones from nothing,
# which is the order the session has always done it in).
#
# A beat and an attack travel as indices, never as text: 0 Dormant, 1 Rest,
# 2 Tell, 3 Strike, 4 Wince, 5 Rage, 6 Down; and 0 Stomp, 1 Sweep, 2 Embers.
type Answer is Type:
  Whole{rows: U32, runs: U32, width: U32, height: U32}
  Code{code: U32, solid: Bool, one_way: Bool}
  Movers{list: List<Layout.Mv>}

# His own numbers: where he stands, what he is doing and how much of the crown
# is left.
type State is Data:
  State{
    x: U32,
    beat: U32,
    kind: U32,
    t: U32,
    phase: U32,
    plates: U32,
    count: U32,
    head: U32,
    head_before: U32,
    x_before: U32,
    flinch: U32
  }

# A thing that hurts: left, top, width, height in pixels, a wave's pixels a
# tick, the ticks of warning before it hurts and the ticks it hurts for.
type Hazard is Data:
  Hazard{x: U32, y: U32, w: U32, h: U32, vx: U32, tell: U32, live: U32}

# The feet of the seats standing, in seat order: six fixed slots and how many
# of them are in use, so every read of them is a copy.
type Feet is Data:
  Feet{
    n: U32,
    ax: U32,
    ay: U32,
    bx: U32,
    by: U32,
    cx: U32,
    cy: U32,
    dx: U32,
    dy: U32,
    ex: U32,
    ey: U32,
    fx: U32,
    fy: U32
  }

# A knight he steps toward, and whether there was one at all.
type Pick is Data:
  Pick{any: Bool, x: U32}

# One beat of the fight: his state on, and the hazards that beat raised.
type Branch is Type:
  Branch{next: State, extra: List<Hazard>}

# His tick, and (for a slam) whether a plate came off.
type BossOut is Type:
  BossOut{state: State, hazards: List<Hazard>}

type SlamOut is Type:
  SlamOut{state: State, hazards: List<Hazard>, hit: Bool}
