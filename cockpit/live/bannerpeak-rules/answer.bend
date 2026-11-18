import Base
import ./layout.bend as Layout

type Answer is Type:
  Whole{rows: U32, runs: U32, width: U32, height: U32}
  Code{code: U32, solid: Bool, one_way: Bool}
  Movers{list: List<Layout.Mv>}

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

type Hazard is Data:
  Hazard{x: U32, y: U32, w: U32, h: U32, vx: U32, tell: U32, live: U32}

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

type Pick is Data:
  Pick{any: Bool, x: U32}

type Branch is Type:
  Branch{next: State, extra: List<Hazard>}

type BossOut is Type:
  BossOut{state: State, hazards: List<Hazard>}

type SlamOut is Type:
  SlamOut{state: State, hazards: List<Hazard>, hit: Bool}
