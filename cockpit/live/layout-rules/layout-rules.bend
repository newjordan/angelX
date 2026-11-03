import Base

# The Delve's authored floors (`layout`): the Undercroft and its three halls,
# Maud's tavern, and the wing Tobbin's crew digs out west of it — where each
# room stands, what it is built of, and which doorways it opens.
#
# Rust keeps the records (`Room`, `Floor`, `Secret`), their serde shapes, the
# tile enum (ids cross as 0 floor, 1 wall, 2 block, 3 hazard, 4 door, 5 stairs,
# 6 ledge, 7 bog), the door-lane arithmetic on a native room, and every
# rendering question. The adapter hands over the authored rectangle tables —
# inert art data the renderer reads too — and this module owns the layout.

type Rect is Data:
  Rect{col: Nat, row: Nat, w: Nat, h: Nat}

type Shape is Data:
  Home{
    cols: Nat,
    rows: Nat,
    north: Bool,
    east: Bool,
    south: Bool,
    west: Bool,
    spots: List<&2, Rect>,
    bounty: Rect,
    stairwell: Rect,
    stair_mouth: Rect
  }
  Hall{
    cols: Nat,
    rows: Nat,
    north: Bool,
    east: Bool,
    south: Bool,
    west: Bool,
    wheel_stand: Rect,
    coffer: Rect,
    benches: List<&2, Rect>
  }
  Yard{
    cols: Nat,
    rows: Nat,
    north: Bool,
    east: Bool,
    south: Bool,
    west: Bool,
    stall: Rect
  }
  Trophies{
    cols: Nat,
    rows: Nat,
    north: Bool,
    east: Bool,
    south: Bool,
    west: Bool,
    plinths: List<&2, Rect>
  }
  Tavern{
    cols: Nat,
    rows: Nat,
    north: Bool,
    east: Bool,
    south: Bool,
    west: Bool,
    bar: Rect,
    board: Rect,
    stage: Rect,
    tables: List<&2, Rect>
  }
  Bare{cols: Nat, rows: Nat, north: Bool, east: Bool, south: Bool, west: Bool}

type Room is Data:
  Room{
    cell_x: Nat,
    cell_y: Nat,
    kind: Nat,
    doors: List<&2, Bool>,
    cols: Nat,
    rows: Nat,
    tiles: List<&2, Nat>
  }

type Dig is Data:
  Dug{hall: Nat, room: Room}
  NoDig{}

type Found is Data:
  Found{hit: Bool, at: Nat, x: Nat, y: Nat}

# The room kinds this module names. The adapter maps each id onto its
# `RoomKind`; a save never sees these numbers.
def kind_home() -> Nat:
  0n

def kind_fortune() -> Nat:
  1n

def kind_yard() -> Nat:
  2n

def kind_trophies() -> Nat:
  3n

def kind_tavern() -> Nat:
  4n

def kind_workshop() -> Nat:
  5n

def kind_muster() -> Nat:
  6n

def tile_floor() -> Nat:
  0n

def tile_wall() -> Nat:
  1n

def tile_block() -> Nat:
  2n

def tile_door() -> Nat:
  4n

def tile_stairs() -> Nat:
  5n

def and(+a: Bool, +b: Bool) -> Bool:
  match a:
    case True{}:
      b
    case False{}:
      False{}

def negate(+value: Bool) -> Bool:
  match value:
    case True{}:
      False{}
    case False{}:
      True{}

# `Bool.pick` with the paint order the native fills use: the guard's tile, else
# the tile painted underneath it.
def layered(guard: Bool, tile: Nat, +below: Nat) -> Nat:
  Bool.pick(Nat, guard, tile, below)

def rect_inside(+rect: Rect, +col: Nat, +row: Nat) -> Bool:
  match rect:
    case Rect{c, r, w, h}:
      (
        Nat.is_ge(col, c)
          && Nat.is_lt(col, (Nat.add(c, w) : Nat))
          && Nat.is_ge(row, r)
          && Nat.is_lt(row, (Nat.add(r, h) : Nat)) : Bool
      )

def inner(+rect: Rect) -> Rect:
  match rect:
    case Rect{c, r, w, h}:
      Rect{
        (Nat.add(c, 1n) : Nat),
        (Nat.add(r, 1n) : Nat),
        (Nat.sub(w, 2n) : Nat),
        (Nat.sub(h, 2n) : Nat)
      }

def in_rects(+rects: List<&2, Rect>, +col: Nat, +row: Nat) -> Bool:
  match rects:
    case Nil{}:
      False{}
    case Con{+head, tail}:
      Bool.pick(
        Bool, rect_inside(head, col, row), True{}, in_rects(tail, col, row)
      )

def is_border(+cols: Nat, +rows: Nat, +col: Nat, +row: Nat) -> Bool:
  (
    Nat.is_eq(col, 0n)
      || Nat.is_eq(row, 0n)
      || Nat.is_eq(col, (Nat.sub(cols, 1n) : Nat))
      || Nat.is_eq(row, (Nat.sub(rows, 1n) : Nat)) : Bool
  )

# The two tiles `Room::open_door(dir)` writes: north and south take the middle
# columns, east and west the middle rows.
def lane_mid(+cols: Nat, +col: Nat) -> Bool:
  (
    Nat.is_eq(col, (Nat.sub(Nat.div(cols, 2n), 1n) : Nat))
      || Nat.is_eq(col, (Nat.div(cols, 2n) : Nat)) : Bool
  )

def lane_n(+cols: Nat, +col: Nat, +row: Nat) -> Bool:
  and(lane_mid(cols, col), Nat.is_eq(row, 0n))

def lane_s(+cols: Nat, +rows: Nat, +col: Nat, +row: Nat) -> Bool:
  and(lane_mid(cols, col), Nat.is_eq(row, (Nat.sub(rows, 1n) : Nat)))

def lane_e(+cols: Nat, +rows: Nat, +col: Nat, +row: Nat) -> Bool:
  and(lane_mid(rows, row), Nat.is_eq(col, (Nat.sub(cols, 1n) : Nat)))

def lane_w(+rows: Nat, +col: Nat, +row: Nat) -> Bool:
  and(lane_mid(rows, row), Nat.is_eq(col, 0n))

def lanes(
  +north: Bool,
  +east: Bool,
  +south: Bool,
  +west: Bool,
  +cols: Nat,
  +rows: Nat,
  +col: Nat,
  +row: Nat
) -> Bool:
  (
    (north && lane_n(cols, col, row))
      || (south && lane_s(cols, rows, col, row))
      || (east && lane_e(cols, rows, col, row))
      || (west && lane_w(rows, col, row)) : Bool
  )

def home_tile(
  lanes_open: Bool,
  +cols: Nat,
  +rows: Nat,
  +col: Nat,
  +row: Nat,
  +spots: List<&2, Rect>,
  +bounty: Rect,
  +stairwell: Rect,
  +stair_mouth: Rect
) -> Nat:
  layered(
    lanes_open,
    tile_door(),
    layered(
      rect_inside(stair_mouth, col, row),
      tile_floor(),
      layered(
        rect_inside(inner(stairwell), col, row),
        tile_stairs(),
        layered(
          in_rects(Con{stairwell, Con{bounty, spots}}, col, row),
          tile_block(),
          layered(is_border(cols, rows, col, row), tile_wall(), tile_floor())
        )
      )
    )
  )

def hall_tile(
  lanes_open: Bool,
  +cols: Nat,
  +rows: Nat,
  +col: Nat,
  +row: Nat,
  +wheel_stand: Rect,
  +coffer: Rect,
  +benches: List<&2, Rect>
) -> Nat:
  layered(
    lanes_open,
    tile_door(),
    layered(
      in_rects(Con{wheel_stand, Con{coffer, benches}}, col, row),
      tile_block(),
      layered(is_border(cols, rows, col, row), tile_wall(), tile_floor())
    )
  )

def yard_tile(
  lanes_open: Bool,
  +cols: Nat,
  +rows: Nat,
  +col: Nat,
  +row: Nat,
  +stall: Rect
) -> Nat:
  layered(
    lanes_open,
    tile_door(),
    layered(
      rect_inside(stall, col, row),
      tile_block(),
      layered(is_border(cols, rows, col, row), tile_wall(), tile_floor())
    )
  )

def trophies_tile(
  lanes_open: Bool,
  +cols: Nat,
  +rows: Nat,
  +col: Nat,
  +row: Nat,
  +plinths: List<&2, Rect>
) -> Nat:
  layered(
    lanes_open,
    tile_door(),
    layered(
      in_rects(plinths, col, row),
      tile_block(),
      layered(is_border(cols, rows, col, row), tile_wall(), tile_floor())
    )
  )

def tavern_tile(
  lanes_open: Bool,
  +cols: Nat,
  +rows: Nat,
  +col: Nat,
  +row: Nat,
  +bar: Rect,
  +board: Rect,
  +stage: Rect,
  +tables: List<&2, Rect>
) -> Nat:
  layered(
    lanes_open,
    tile_door(),
    layered(
      in_rects(Con{bar, Con{board, Con{stage, tables}}}, col, row),
      tile_block(),
      layered(is_border(cols, rows, col, row), tile_wall(), tile_floor())
    )
  )

def bare_tile(
  lanes_open: Bool, +cols: Nat, +rows: Nat, +col: Nat, +row: Nat
) -> Nat:
  layered(
    lanes_open,
    tile_door(),
    layered(is_border(cols, rows, col, row), tile_wall(), tile_floor())
  )

def tile(+shape: Shape, +col: Nat, +row: Nat) -> Nat:
  match shape:
    case Home{cols, rows, north, east, south, west, spots, bounty, stairwell, stair_mouth}:
      home_tile(
        lanes(north, east, south, west, cols, rows, col, row),
        cols,
        rows,
        col,
        row,
        spots,
        bounty,
        stairwell,
        stair_mouth
      )
    case Hall{cols, rows, north, east, south, west, wheel_stand, coffer, benches}:
      hall_tile(
        lanes(north, east, south, west, cols, rows, col, row),
        cols,
        rows,
        col,
        row,
        wheel_stand,
        coffer,
        benches
      )
    case Yard{cols, rows, north, east, south, west, stall}:
      yard_tile(
        lanes(north, east, south, west, cols, rows, col, row),
        cols,
        rows,
        col,
        row,
        stall
      )
    case Trophies{cols, rows, north, east, south, west, plinths}:
      trophies_tile(
        lanes(north, east, south, west, cols, rows, col, row),
        cols,
        rows,
        col,
        row,
        plinths
      )
    case Tavern{cols, rows, north, east, south, west, bar, board, stage, tables}:
      tavern_tile(
        lanes(north, east, south, west, cols, rows, col, row),
        cols,
        rows,
        col,
        row,
        bar,
        board,
        stage,
        tables
      )
    case Bare{cols, rows, north, east, south, west}:
      bare_tile(
        lanes(north, east, south, west, cols, rows, col, row), cols, rows, col, row
      )

def row_tiles(+shape: Shape, +cols: List<&2, Nat>, +at: Nat) -> List<&2, Nat>:
  match cols:
    case Nil{}:
      Nil{}
    case Con{+col, rest}:
      Con{tile(shape, col, at), row_tiles(shape, rest, at)}

def grid(
  +shape: Shape, +cols: List<&2, Nat>, +rows: List<&2, Nat>
) -> List<&2, List<&2, Nat>>:
  match rows:
    case Nil{}:
      Nil{}
    case Con{+at, rest}:
      Con{row_tiles(shape, cols, at), grid(shape, cols, rest)}

def tiles(+shape: Shape, +cols: Nat, +rows: Nat) -> List<&2, Nat>:
  List.concat(&2, Nat, grid(shape, List.range(cols), List.range(rows)))

def room(
  +cell_x: Nat,
  +cell_y: Nat,
  +kind: Nat,
  +doors: List<&2, Bool>,
  +shape: Shape,
  +cols: Nat,
  +rows: Nat
) -> Room:
  Room{cell_x, cell_y, kind, doors, cols, rows, tiles(shape, cols, rows)}

def doors_north() -> List<&2, Bool>:
  Con{True{}, Con{False{}, Con{False{}, Con{False{}, Nil{}}}}}

def doors_east() -> List<&2, Bool>:
  Con{False{}, Con{True{}, Con{False{}, Con{False{}, Nil{}}}}}

def doors_south() -> List<&2, Bool>:
  Con{False{}, Con{False{}, Con{True{}, Con{False{}, Nil{}}}}}

def doors_west() -> List<&2, Bool>:
  Con{False{}, Con{False{}, Con{False{}, Con{True{}, Nil{}}}}}

def doors_shut() -> List<&2, Bool>:
  Con{False{}, Con{False{}, Con{False{}, Con{False{}, Nil{}}}}}

def doors_home() -> List<&2, Bool>:
  Con{True{}, Con{True{}, Con{True{}, Con{False{}, Nil{}}}}}

def doors_loft() -> List<&2, Bool>:
  Con{True{}, Con{True{}, Con{False{}, Con{False{}, Nil{}}}}}

# --- the Undercroft -------------------------------------------------------

# The Home the party starts in with the Winding Stair in it, Dame Fortune's
# hall east through its door, the Training Yard north of it, and the Trophy
# Hall south down the runner.
def layout_undercroft(
  +spots: List<&2, Rect>,
  +bounty: Rect,
  +stairwell: Rect,
  +stair_mouth: Rect,
  +wheel_stand: Rect,
  +coffer: Rect,
  +benches: List<&2, Rect>,
  +stall: Rect,
  +plinths: List<&2, Rect>
) -> List<&2, Room>:
  Con{
    room(
      2n,
      2n,
      kind_home(),
      doors_home(),
      Home{
        24n,
        14n,
        True{},
        True{},
        True{},
        False{},
        spots,
        bounty,
        stairwell,
        stair_mouth
      },
      24n,
      14n
    ),
    Con{
      room(
        3n,
        2n,
        kind_fortune(),
        doors_west(),
        Hall{
          24n,
          14n,
          False{},
          False{},
          False{},
          True{},
          wheel_stand,
          coffer,
          benches
        },
        24n,
        14n
      ),
      Con{
        room(
          2n,
          1n,
          kind_yard(),
          doors_south(),
          Yard{24n, 14n, False{}, False{}, True{}, False{}, stall},
          24n,
          14n
        ),
        Con{
          room(
            2n,
            3n,
            kind_trophies(),
            doors_north(),
            Trophies{24n, 14n, True{}, False{}, False{}, False{}, plinths},
            24n,
            14n
          ),
          Nil{}
        }
      }
    }
  }

# --- Maud's tavern --------------------------------------------------------

def has_kind(+kinds: List<&2, Nat>, +kind: Nat) -> Bool:
  match kinds:
    case Nil{}:
      False{}
    case Con{+head, tail}:
      Bool.pick(Bool, Nat.is_eq(head, kind), True{}, has_kind(tail, kind))

def hall_pick(+rest: Found, +head: Nat, +here: Nat, +hx: Nat, +hy: Nat) -> Found:
  match rest:
    case Found{hit, at, x, y}:
      match hit:
        case True{}:
          Found{True{}, at, x, y}
        case False{}:
          Bool.pick(
            Found,
            Nat.is_eq(head, kind_trophies()),
            Found{True{}, here, hx, hy},
            Found{False{}, 0n, 0n, 0n}
          )

def hall_at(
  +kinds: List<&2, Nat>,
  +xs: List<&2, Nat>,
  +ys: List<&2, Nat>,
  +at: Nat
) -> Found:
  match kinds:
    case Nil{}:
      Found{False{}, 0n, 0n, 0n}
    case Con{+head, tail}:
      match xs:
        case Nil{}:
          Found{False{}, 0n, 0n, 0n}
        case Con{+x, rest_x}:
          match ys:
            case Nil{}:
              Found{False{}, 0n, 0n, 0n}
            case Con{+y, rest_y}:
              hall_pick(
                hall_at(tail, rest_x, rest_y, (Nat.add(at, 1n) : Nat)),
                head,
                at,
                x,
                y
              )

def find_hall(
  +kinds: List<&2, Nat>, +xs: List<&2, Nat>, +ys: List<&2, Nat>
) -> Found:
  hall_at(kinds, xs, ys, 0n)

def dig_from(
  ready: Bool,
  +hall: Found,
  +bar: Rect,
  +board: Rect,
  +stage: Rect,
  +tables: List<&2, Rect>
) -> Dig:
  match ready:
    case False{}:
      NoDig{}
    case True{}:
      match hall:
        case Found{hit, at, x, y}:
          match hit:
            case False{}:
              NoDig{}
            case True{}:
              Dug{
                at,
                room(
                  (Nat.sub(x, 1n) : Nat),
                  y,
                  kind_tavern(),
                  doors_east(),
                  Tavern{
                    24n,
                    14n,
                    False{},
                    True{},
                    False{},
                    False{},
                    bar,
                    board,
                    stage,
                    tables
                  },
                  24n,
                  14n
                )
              }

# Tobbin's crew digs out Maud's tavern west of the Trophy Hall: the hall's west
# wall opens and the tavern stands one cell west of it, its east door back into
# the hall. False if this is not the home floor, if a tavern already stands
# here, or if the floor has no Trophy Hall. Cells cross biased by 1024, so the
# tavern this answers with is biased the same way and the adapter decodes both.
def layout_tavern(
  +depth: Nat,
  +kinds: List<&2, Nat>,
  +xs: List<&2, Nat>,
  +ys: List<&2, Nat>,
  +bar: Rect,
  +board: Rect,
  +stage: Rect,
  +tables: List<&2, Rect>
) -> Dig:
  dig_from(
    and(Nat.is_eq(depth, 0n), negate(has_kind(kinds, kind_tavern()))),
    find_hall(kinds, xs, ys),
    bar,
    board,
    stage,
    tables
  )

# The bellwright's loft west of the tavern, and the siege muster north of it:
# each a bare room with its own doors, one cell on from the last.
def layout_tavern_wing(+cell_x: Nat, +cell_y: Nat) -> List<&2, Room>:
  Con{
    room(
      (Nat.sub(cell_x, 1n) : Nat),
      cell_y,
      kind_workshop(),
      doors_loft(),
      Bare{24n, 14n, True{}, True{}, False{}, False{}},
      24n,
      14n
    ),
    Con{
      room(
        (Nat.sub(cell_x, 1n) : Nat),
        (Nat.sub(cell_y, 1n) : Nat),
        kind_muster(),
        doors_south(),
        Bare{24n, 14n, False{}, False{}, True{}, False{}},
        24n,
        14n
      ),
      Nil{}
    }
  }
