import Base
import ./answer.bend as Ans
import ./boss.bend as Boss
import ./layout.bend as Layout
import ./map.bend as Map
import ./nums.bend as N
import ./tiles.bend as Tiles

# Bannerpeak's authored rules. Rust keeps the session and knight shapes, the
# saves and the wire; this leg owns what the world is made of and, from here,
# what a tick makes of it. The adapter hands plain JSON over and marshals the
# answer back: a cell is a code, a mover is a number, never a glyph.
#
# The ops are the game's entry points:
#   World{}        the world was read whole: its shape and every run in it
#   Tile{tx, ty}   a cell's code, and whether it is solid or one-way
#   Movers{t}      every moving platform at tick t
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

# King Cinder's tick and his slam. They take lists (his hazards), which a record
# cannot carry and a call may consume once, so they are their own entry points:
# the adapter hands plain JSON over and marshals the answer back.
def bannerpeak_boss(+elapsed: U32, +seats: U32, +at: Ans.Feet, +st: Ans.State, hazards: List<Ans.Hazard>) -> Ans.BossOut:
  Boss.step(elapsed, seats, at, st, hazards)

def bannerpeak_slam(+st: Ans.State, +x: U32, +y: U32, hazards: List<Ans.Hazard>) -> Ans.SlamOut:
  Boss.slam(st, x, y, hazards)
