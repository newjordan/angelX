# The shapes the rooms module answers in. Rust reads them as plain JSON: an
# art answer is a list of quadrants, each with the size it was drawn at and
# its rows of `.` floor, `#` block, `~` hazard. Nothing else crosses.
import Base

# One quadrant of a room, drawn as the room's top-left half and mirrored both
# ways by the renderer. `w` and `h` are the size it is drawn for, so a size
# query is a comparison of numbers, not a measurement of art.
type Quad is Data:
  Quad{w: U32, h: U32, rows: List<&2, String>}

# The answer to a draw: the quadrants of one kind, in authored order.
type Art is Data:
  Art{quads: List<&2, Quad>}

type Size is Data:
  Size{w: U32, h: U32}

# The numbers the layout rules share with this module.
type Consts is Data:
  Consts{sizes: List<&2, Size>, share: U32, least_floor: U32}
