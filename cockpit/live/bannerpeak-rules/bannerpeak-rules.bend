import Base
import ./answer.bend as Ans
import ./boss.bend as Boss
import ./layout.bend as Layout
import ./map.bend as Map
import ./nums.bend as N
import ./tiles.bend as Tiles

type Op is Data:
  World{}
  Tile{tx: U32, ty: U32}
  Movers{t: U32}

def run(op: Op) -> Ans.Answer:
  match op:
    case World{}:
      Tiles.whole(Map.rows(), Map.width(), Map.height())
    case Tile{+tx, +ty}:
      Tiles.describe(Map.rows(), tx, ty)
    case Movers{+t}:
      Ans.Movers{Layout.movers(t)}

def bannerpeak_boss(+elapsed: U32, +seats: U32, +at: Ans.Feet, +st: Ans.State, hazards: List<Ans.Hazard>) -> Ans.BossOut:
  Boss.step(elapsed, seats, at, st, hazards)

def bannerpeak_slam(+st: Ans.State, +x: U32, +y: U32, hazards: List<Ans.Hazard>) -> Ans.SlamOut:
  Boss.slam(st, x, y, hazards)
