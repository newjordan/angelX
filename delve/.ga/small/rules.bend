import Base
import ../../lib/j.bend as J
import ../../lib/bj.bend as B
import ../../lib/delta.bend as D
import ../../lib/w64.bend as W
import ../../lib/rs.bend as RS

type LayTile is Data:
  TileFloor{}
  TileWall{}
  TileBlock{}
  TileHazard{}
  TileDoor{}
  TileStairs{}
  TileLedge{}
  TileBog{}

# A room's ground as rows of tiles: LayGrid{cols, rows, tiles: row by row}.

type LayGrid is Data:
  LayGrid{cols: U32, rows: U32, tiles: List<&2, List<&2, LayTile>>}

type LayLane is Data:
  LayLane{c0: U32, r0: U32, c1: U32, r1: U32}

# A search over a room's cells that found a cell (ok) or did not.

type LayFind is Data:
  LayFind{ok: Bool, a: U32, b: U32}

# A cell on a floor's room grid (i32 values, two's complement).

type LayCell is Data:
  LayCell{x: U32, y: U32}

# (column, row, width, height) in tiles.

type LayRect is Data:
  LayRect{c: U32, r: U32, w: U32, h: U32}

# What a floor's cells become: doors from the entrance to each, the far room
# (the stairs), the treasure's dead end (usize::MAX if none) and the halls.

type LayPlan is Data:
  LayPlan{dist: List<&2, U32>, far: U32, treasure: U32, halls: List<&2, U32>}

# A room's size in tiles.

type LayDims is Data:
  LayDims{cols: U32, rows: U32}

# An open cell with its wetness: (wet, x, y).

type LayWet is Data:
  LayWet{wet: F32, x: U32, y: U32}

# (beside the guardian, beside a showdown, near the entrance, distance from the middle).

type LayChief is Data:
  LayChief{guard: Bool, show: Bool, early: Bool, mid: U32}

# One authored world room: its kind and its cell on the home floor.

type WorldRoom is Data:
  WorldRoom{kind: String, x: U32, y: U32}

# A way down or across: the room it stands in, its stair tiles, where it goes
# (a room at a point, or a delve of a pack when `to_room` is empty), its
# board's words and what must stand before it opens (empty for nothing).

type WorldEntrance is Data:
  WorldEntrance{id: String, room: String, c: U32, r: U32, w: U32, h: U32, to_room: String, to_x: F32, to_y: F32, delve: String, label: String, needs: String}

# What `travel` finds standing in the ways out: the last door side and its seat,
# the first stairs seat, whether any stairs, the entrance on them, how many
# knights count, who stands in which door, and on which stairs (the entrance id).

type FScan is Data:
  FScan{exit: U32, exit_seat: U32, stairs_seat: U32, stairs: Bool, entrance: Maybe<&2, WorldEntrance>, counted: U32, in_door: J.Val, on_stairs: J.Val}

# A mini-game's fold over the knights: the run, the game's state, and a marker
# (a seat id, usize::MAX for none).

type WS is Data:
  WS{run: J.Val, g: J.Val, mark: U32}

# The powder store's kegs going off: the run, the game's state, the tiles on fire.

type PowderS is Data:
  PowderS{run: J.Val, p: J.Val, blasts: List<&2, LayCell>}

def layout.Tile.eq?(+a: LayTile, +b: LayTile) -> Bool:
  match a b:
    case TileFloor{} TileFloor{}:
      True{}
    case TileWall{} TileWall{}:
      True{}
    case TileBlock{} TileBlock{}:
      True{}
    case TileHazard{} TileHazard{}:
      True{}
    case TileDoor{} TileDoor{}:
      True{}
    case TileStairs{} TileStairs{}:
      True{}
    case TileLedge{} TileLedge{}:
      True{}
    case TileBog{} TileBog{}:
      True{}
    case _ _:
      False{}
# Ground a body can be set down on: dry floor, or the swamp's bog.

def layout.Tile.ground?(+t: LayTile) -> Bool:
  match t:
    case TileFloor{}:
      True{}
    case TileBog{}:
      True{}
    case _:
      False{}

def layout.tile_glyph?(+t: LayTile) -> Char:
  match t:
    case TileFloor{}:
      '.'
    case TileWall{}:
      '#'
    case TileBlock{}:
      'B'
    case TileHazard{}:
      '~'
    case TileDoor{}:
      'D'
    case TileStairs{}:
      '>'
    case TileLedge{}:
      '='
    case TileBog{}:
      'o'
# The tile a glyph names; a glyph that names none reads as stone.

def layout.tile_from_glyph?(+c: Char) -> LayTile:
  match c:
    case '.':
      TileFloor{}
    case '#':
      TileWall{}
    case 'B':
      TileBlock{}
    case '~':
      TileHazard{}
    case 'D':
      TileDoor{}
    case '>':
      TileStairs{}
    case '=':
      TileLedge{}
    case 'o':
      TileBog{}
    case _:
      TileWall{}
# Tiles to the saved run string and back.

def layout.tiles_rle.flush?(+cur: LayTile, +n: U32) -> String:
  layout.tiles_rle.flush.k1(U32.is_eq(n, 1), cur, n)

def layout.tiles_rle.flush.k1?(cv: Bool, +cur: LayTile, +n: U32) -> String:
  match cv:
    case True{}:
      SCon{layout.tile_glyph(cur), SNil{}}
    case False{}:
      String.append(U32.show(n), SCon{layout.tile_glyph(cur), SNil{}})

def layout.tiles_rle.run?(+ts: List<&2, LayTile>, +cur: LayTile, +n: U32) -> String:
  layout.tiles_rle.run.k1(ts, ts, cur, n)

def layout.tiles_rle.run.k2?(cv: Bool, +cur: LayTile, +n: U32, +h: LayTile, +t: List<&2, LayTile>) -> String:
  match cv:
    case True{}:
      layout.tiles_rle.run(t, cur, (n + 1 : U32))
    case False{}:
      String.append(layout.tiles_rle.flush(cur, n), layout.tiles_rle.run(t, h, 1))

def layout.tiles_rle.run.k1?(sv0: List<&2, LayTile>, +ts: List<&2, LayTile>, +cur: LayTile, +n: U32) -> String:
  match sv0:
    case Nil{}:
      layout.tiles_rle.flush(cur, n)
    case Con{+h, +t}:
      layout.tiles_rle.run.k2(layout.Tile.eq(h, cur), cur, n, h, t)

def layout.tiles_rle?(+ts: List<&2, LayTile>) -> String:
  layout.tiles_rle.k1(ts, ts)

def layout.tiles_rle.k1?(sv0: List<&2, LayTile>, +ts: List<&2, LayTile>) -> String:
  match sv0:
    case Nil{}:
      ""
    case Con{+h, +t}:
      layout.tiles_rle.run(t, h, 1)

def layout.tiles_from_rle.rep?(+t: LayTile, +n: U32, +rest: List<&2, LayTile>) -> List<&2, LayTile>:
  layout.tiles_from_rle.rep.k1(U32.is_eq(n, 0), t, n, rest)

def layout.tiles_from_rle.rep.k1?(cv: Bool, +t: LayTile, +n: U32, +rest: List<&2, LayTile>) -> List<&2, LayTile>:
  match cv:
    case True{}:
      rest
    case False{}:
      Con{t, layout.tiles_from_rle.rep(t, (n - 1 : U32), rest)}

def layout.tiles_from_rle.go?(+s: String, +n: U32, +saw: Bool) -> List<&2, LayTile>:
  layout.tiles_from_rle.go.k1(s, s, n, saw)

def layout.tiles_from_rle.go.k2?(cv: Bool, +n: U32, +saw: Bool, +c: Char, +t: String) -> List<&2, LayTile>:
  match cv:
    case True{}:
      layout.tiles_from_rle.go(t, (n * 10 + (Char.to_u32(c) - 48) : U32), True{})
    case False{}:
      layout.tiles_from_rle.rep(layout.tile_from_glyph(c), Bool.pick(U32, saw, n, 1), layout.tiles_from_rle.go(t, 0, False{}))

def layout.tiles_from_rle.go.k1?(sv0: String, +s: String, +n: U32, +saw: Bool) -> List<&2, LayTile>:
  match sv0:
    case SNil{}:
      Nil{}
    case SCon{+c, +t}:
      layout.tiles_from_rle.go.k2(Char.is_digit(c), n, saw, c, t)

def layout.tiles_from_rle?(+s: String) -> List<&2, LayTile>:
  layout.tiles_from_rle.go(s, 0, False{})
# The tile at `idx` of the run string, read where it stands; past the end, stone.

def layout.rle.at?(+s: String, +idx: U32) -> LayTile:
  layout.rle.at.go(s, idx, 0, False{})

def layout.rle.at.go?(+s: String, +idx: U32, +n: U32, +saw: Bool) -> LayTile:
  layout.rle.at.go.k1(s, s, idx, n, saw)

def layout.rle.at.go.k2?(cv: Bool, +idx: U32, +n: U32, +saw: Bool, +c: Char, +t: String) -> LayTile:
  match cv:
    case True{}:
      layout.rle.at.go(t, idx, (n * 10 + (Char.to_u32(c) - 48) : U32), True{})
    case False{}:
      layout.rle.at.run(c, t, idx, Bool.pick(U32, saw, n, 1))

def layout.rle.at.go.k1?(sv0: String, +s: String, +idx: U32, +n: U32, +saw: Bool) -> LayTile:
  match sv0:
    case SNil{}:
      TileWall{}
    case SCon{+c, +t}:
      layout.rle.at.go.k2(Char.is_digit(c), idx, n, saw, c, t)

def layout.rle.at.run?(+c: Char, +t: String, +idx: U32, +count: U32) -> LayTile:
  layout.rle.at.run.k1(U32.is_lt(idx, count), c, t, idx, count)

def layout.rle.at.run.k1?(cv: Bool, +c: Char, +t: String, +idx: U32, +count: U32) -> LayTile:
  match cv:
    case True{}:
      layout.tile_from_glyph(c)
    case False{}:
      layout.rle.at.go(t, (idx - count : U32), 0, False{})
# A grid
# ------

def layout.row.fill?(+n: U32, +t: LayTile) -> List<&2, LayTile>:
  layout.row.fill.k1(U32.is_eq(n, 0), n, t)

def layout.row.fill.k1?(cv: Bool, +n: U32, +t: LayTile) -> List<&2, LayTile>:
  match cv:
    case True{}:
      Nil{}
    case False{}:
      Con{t, layout.row.fill((n - 1 : U32), t)}

def layout.grid.rows_fill?(+n: U32, +cols: U32, +t: LayTile) -> List<&2, List<&2, LayTile>>:
  layout.grid.rows_fill.k1(U32.is_eq(n, 0), n, cols, t)

def layout.grid.rows_fill.k1?(cv: Bool, +n: U32, +cols: U32, +t: LayTile) -> List<&2, List<&2, LayTile>>:
  match cv:
    case True{}:
      Nil{}
    case False{}:
      Con{layout.row.fill(cols, t), layout.grid.rows_fill((n - 1 : U32), cols, t)}
# `cols` by `rows` of `t`.

def layout.grid.new?(+cols: U32, +rows: U32, +t: LayTile) -> LayGrid:
  LayGrid{cols, rows, layout.grid.rows_fill(rows, cols, t)}

def layout.row.get?(+row: List<&2, LayTile>, +col: U32) -> LayTile:
  layout.row.get.k1(row, row, col)

def layout.row.get.k2?(cv: Bool, +col: U32, +h: LayTile, +t: List<&2, LayTile>) -> LayTile:
  match cv:
    case True{}:
      h
    case False{}:
      layout.row.get(t, (col - 1 : U32))

def layout.row.get.k1?(sv0: List<&2, LayTile>, +row: List<&2, LayTile>, +col: U32) -> LayTile:
  match sv0:
    case Nil{}:
      TileWall{}
    case Con{+h, +t}:
      layout.row.get.k2(U32.is_eq(col, 0), col, h, t)

def layout.rows.get?(+rows: List<&2, List<&2, LayTile>>, +r: U32, +col: U32) -> LayTile:
  layout.rows.get.k1(rows, rows, r, col)

def layout.rows.get.k2?(cv: Bool, +r: U32, +col: U32, +h: List<&2, LayTile>, +t: List<&2, List<&2, LayTile>>) -> LayTile:
  match cv:
    case True{}:
      layout.row.get(h, col)
    case False{}:
      layout.rows.get(t, (r - 1 : U32), col)

def layout.rows.get.k1?(sv0: List<&2, List<&2, LayTile>>, +rows: List<&2, List<&2, LayTile>>, +r: U32, +col: U32) -> LayTile:
  match sv0:
    case Nil{}:
      TileWall{}
    case Con{+h, +t}:
      layout.rows.get.k2(U32.is_eq(r, 0), r, col, h, t)
# The tile at (col, row); off the grid, stone (Rust's `Room::tile`).

def layout.grid.get?(+g: LayGrid, +col: U32, +row: U32) -> LayTile:
  layout.grid.get.k1(g, g, col, row)

def layout.grid.get.k2?(cv: Bool, +col: U32, +row: U32, +cols: U32, +rows: U32, +tiles: List<&2, List<&2, LayTile>>) -> LayTile:
  match cv:
    case True{}:
      TileWall{}
    case False{}:
      layout.rows.get(tiles, row, col)

def layout.grid.get.k1?(sv0: LayGrid, +g: LayGrid, +col: U32, +row: U32) -> LayTile:
  match sv0:
    case LayGrid{+cols, +rows, +tiles}:
      layout.grid.get.k2(Bool.or(U32.is_ge(col, cols), U32.is_ge(row, rows)), col, row, cols, rows, tiles)

def layout.row.set?(+row: List<&2, LayTile>, +col: U32, +t: LayTile) -> List<&2, LayTile>:
  layout.row.set.k1(row, row, col, t)

def layout.row.set.k2?(cv: Bool, +col: U32, +t: LayTile, +h: LayTile, +tl: List<&2, LayTile>) -> List<&2, LayTile>:
  match cv:
    case True{}:
      Con{t, tl}
    case False{}:
      Con{h, layout.row.set(tl, (col - 1 : U32), t)}

def layout.row.set.k1?(sv0: List<&2, LayTile>, +row: List<&2, LayTile>, +col: U32, +t: LayTile) -> List<&2, LayTile>:
  match sv0:
    case Nil{}:
      Nil{}
    case Con{+h, +tl}:
      layout.row.set.k2(U32.is_eq(col, 0), col, t, h, tl)

def layout.rows.set?(+rows: List<&2, List<&2, LayTile>>, +r: U32, +col: U32, +t: LayTile) -> List<&2, List<&2, LayTile>>:
  layout.rows.set.k1(rows, rows, r, col, t)

def layout.rows.set.k2?(cv: Bool, +r: U32, +col: U32, +t: LayTile, +h: List<&2, LayTile>, +tl: List<&2, List<&2, LayTile>>) -> List<&2, List<&2, LayTile>>:
  match cv:
    case True{}:
      Con{layout.row.set(h, col, t), tl}
    case False{}:
      Con{h, layout.rows.set(tl, (r - 1 : U32), col, t)}

def layout.rows.set.k1?(sv0: List<&2, List<&2, LayTile>>, +rows: List<&2, List<&2, LayTile>>, +r: U32, +col: U32, +t: LayTile) -> List<&2, List<&2, LayTile>>:
  match sv0:
    case Nil{}:
      Nil{}
    case Con{+h, +tl}:
      layout.rows.set.k2(U32.is_eq(r, 0), r, col, t, h, tl)
# Rust's `Room::set`: a cell off the grid is left alone (Rust would panic).

def layout.grid.set?(+g: LayGrid, +col: U32, +row: U32, +t: LayTile) -> LayGrid:
  layout.grid.set.k1(g, g, col, row, t)

def layout.grid.set.k2?(cv: Bool, +g: LayGrid, +col: U32, +row: U32, +t: LayTile, +cols: U32, +rows: U32, +tiles: List<&2, List<&2, LayTile>>) -> LayGrid:
  match cv:
    case True{}:
      g
    case False{}:
      LayGrid{cols, rows, layout.rows.set(tiles, row, col, t)}

def layout.grid.set.k1?(sv0: LayGrid, +g: LayGrid, +col: U32, +row: U32, +t: LayTile) -> LayGrid:
  match sv0:
    case LayGrid{+cols, +rows, +tiles}:
      layout.grid.set.k2(Bool.or(U32.is_ge(col, cols), U32.is_ge(row, rows)), g, col, row, t, cols, rows, tiles)

def layout.grid.flat?(+rows: List<&2, List<&2, LayTile>>) -> List<&2, LayTile>:
  List.concat(&2, LayTile, rows)

def layout.grid.rle?(+g: LayGrid) -> String:
  layout.grid.rle.k1(g, g)

def layout.grid.rle.k1?(sv0: LayGrid, +g: LayGrid) -> String:
  match sv0:
    case LayGrid{+cols, +rows, +tiles}:
      layout.tiles_rle(layout.grid.flat(tiles))

def layout.grid.cut?(+ts: List<&2, LayTile>, +cols: U32, +rows: U32) -> List<&2, List<&2, LayTile>>:
  layout.grid.cut.k1(U32.is_eq(rows, 0), ts, cols, rows)

def layout.grid.cut.k1?(cv: Bool, +ts: List<&2, LayTile>, +cols: U32, +rows: U32) -> List<&2, List<&2, LayTile>>:
  match cv:
    case True{}:
      Nil{}
    case False{}:
      Con{List.take(&2, LayTile, ts, U32.to_nat(cols)), layout.grid.cut(List.drop(&2, LayTile, ts, U32.to_nat(cols)), cols, (rows - 1 : U32))}
# A room's ground as a grid.

def layout.grid.of_room?(+room: J.Val) -> LayGrid:
  layout.grid.of_rle(J.str(J.get(room, "tiles")), J.u32(J.get(room, "cols")), J.u32(J.get(room, "rows")))

def layout.grid.of_rle?(+text: String, +cols: U32, +rows: U32) -> LayGrid:
  LayGrid{cols, rows, layout.grid.cut(layout.tiles_from_rle(text), cols, rows)}
# A room with `g` as its ground (its size follows the grid).

def layout.grid.put?(+room: J.Val, +g: LayGrid) -> J.Val:
  layout.grid.put.k1(g, room, g)

def layout.grid.put.k1?(sv0: LayGrid, +room: J.Val, +g: LayGrid) -> J.Val:
  match sv0:
    case LayGrid{+cols, +rows, +tiles}:
      J.set(J.set(J.set(room, "cols", J.JU{cols}), "rows", J.JU{rows}), "tiles", J.JS{layout.tiles_rle(layout.grid.flat(tiles))})
# A room
# ------

def layout.Room.cols?(+room: J.Val) -> U32:
  J.u32(J.get(room, "cols"))

def layout.Room.rows?(+room: J.Val) -> U32:
  J.u32(J.get(room, "rows"))

def layout.Room.kind?(+room: J.Val) -> String:
  J.str(J.get(room, "kind"))
# Whether the room is of this kind (the snake_case name, `"fight"`).

def layout.Room.is_kind?(+room: J.Val, +kind: String) -> Bool:
  String.eq(J.str(J.get(room, "kind")), kind)
# Door `dir` (north, east, south, west) is open.

def layout.Room.door?(+room: J.Val, +dir: U32) -> Bool:
  J.bool(J.nth(J.get(room, "doors"), dir))
# Rust's `Room::tile`: off the room, stone.

def layout.Room.tile_t?(+room: J.Val, +col: U32, +row: U32) -> LayTile:
  layout.Room.tile.in(room, col, row, J.u32(J.get(room, "cols")), J.u32(J.get(room, "rows")))

def layout.Room.tile.in?(+room: J.Val, +col: U32, +row: U32, +cols: U32, +rows: U32) -> LayTile:
  layout.Room.tile.in.k1(Bool.or(U32.is_ge(col, cols), U32.is_ge(row, rows)), room, col, row, cols, rows)

def layout.Room.tile.in.k1?(cv: Bool, +room: J.Val, +col: U32, +row: U32, +cols: U32, +rows: U32) -> LayTile:
  match cv:
    case True{}:
      TileWall{}
    case False{}:
      layout.rle.at(J.str(J.get(room, "tiles")), (row * cols + col : U32))
# Rust's `Room::set`.

def layout.Room.set_t?(+room: J.Val, +col: U32, +row: U32, +t: LayTile) -> J.Val:
  layout.grid.put(room, layout.grid.set(layout.grid.of_room(room), col, row, t))
# The same over the tile as the state holds it: its serde name ("floor").

def layout.Room.tile?(+room: J.Val, +col: U32, +row: U32) -> J.Val:
  J.JS{layout.tile_name(layout.Room.tile_t(room, col, row))}

def layout.Room.set?(+room: J.Val, +col: U32, +row: U32, +tile: J.Val) -> J.Val:
  layout.Room.set_t(room, col, row, layout.tile_from_name(J.str(tile)))

def layout.TILE_UNITS?() -> F32:
  2.0

def layout.COLS?() -> U32:
  24

def layout.ROWS?() -> U32:
  14

def layout.MAX_COLS?() -> U32:
  48

def layout.MAX_ROWS?() -> U32:
  28
# Size in arena units.

def layout.Room.width?(+room: J.Val) -> F32:
  (U32.to_f32(J.u32(J.get(room, "cols"))) * 2.0 : F32)

def layout.Room.height?(+room: J.Val) -> F32:
  (U32.to_f32(J.u32(J.get(room, "rows"))) * 2.0 : F32)
# Bigger than one screen: the camera follows the knights.

def layout.Room.great?(+room: J.Val) -> Bool:
  Bool.or(U32.is_gt(J.u32(J.get(room, "cols")), 24), U32.is_gt(J.u32(J.get(room, "rows")), 14))
# The side-on hall's way out: the first open doorway.

def layout.Room.way_back?(+room: J.Val) -> Maybe<&2, U32>:
  layout.Room.way_back.k1(Bool.or(layout.Room.is_kind(room, "ledge"), layout.Room.is_kind(room, "wanted")), room)

def layout.Room.way_back.k1?(cv: Bool, +room: J.Val) -> Maybe<&2, U32>:
  match cv:
    case True{}:
      layout.Room.first_door(J.get(room, "doors"), 0)
    case False{}:
      None{}

def layout.Room.first_door?(+doors: J.Val, +i: U32) -> Maybe<&2, U32>:
  layout.Room.first_door.k1(U32.is_ge(i, 4), doors, i)

def layout.Room.first_door.k2?(cv: Bool, +doors: J.Val, +i: U32) -> Maybe<&2, U32>:
  match cv:
    case True{}:
      Some{i}
    case False{}:
      layout.Room.first_door(doors, (i + 1 : U32))

def layout.Room.first_door.k1?(cv: Bool, +doors: J.Val, +i: U32) -> Maybe<&2, U32>:
  match cv:
    case True{}:
      None{}
    case False{}:
      layout.Room.first_door.k2(J.bool(J.nth(doors, i)), doors, i)

def layout.half?(+n: U32) -> U32:
  (n >> 1n : U32)

def layout.tile_name?(+t: LayTile) -> String:
  match t:
    case TileFloor{}:
      "floor"
    case TileWall{}:
      "wall"
    case TileBlock{}:
      "block"
    case TileHazard{}:
      "hazard"
    case TileDoor{}:
      "door"
    case TileStairs{}:
      "stairs"
    case TileLedge{}:
      "ledge"
    case TileBog{}:
      "bog"

def layout.tile_from_name?(+s: String) -> LayTile:
  layout.tile_from_name.k1(String.eq(s, "floor"), s)

def layout.tile_from_name.k7?(cv: Bool, +s: String) -> LayTile:
  match cv:
    case True{}:
      TileLedge{}
    case False{}:
      TileBog{}

def layout.tile_from_name.k6?(cv: Bool, +s: String) -> LayTile:
  match cv:
    case True{}:
      TileStairs{}
    case False{}:
      layout.tile_from_name.k7(String.eq(s, "ledge"), s)

def layout.tile_from_name.k5?(cv: Bool, +s: String) -> LayTile:
  match cv:
    case True{}:
      TileDoor{}
    case False{}:
      layout.tile_from_name.k6(String.eq(s, "stairs"), s)

def layout.tile_from_name.k4?(cv: Bool, +s: String) -> LayTile:
  match cv:
    case True{}:
      TileHazard{}
    case False{}:
      layout.tile_from_name.k5(String.eq(s, "door"), s)

def layout.tile_from_name.k3?(cv: Bool, +s: String) -> LayTile:
  match cv:
    case True{}:
      TileBlock{}
    case False{}:
      layout.tile_from_name.k4(String.eq(s, "hazard"), s)

def layout.tile_from_name.k2?(cv: Bool, +s: String) -> LayTile:
  match cv:
    case True{}:
      TileWall{}
    case False{}:
      layout.tile_from_name.k3(String.eq(s, "block"), s)

def layout.tile_from_name.k1?(cv: Bool, +s: String) -> LayTile:
  match cv:
    case True{}:
      TileFloor{}
    case False{}:
      layout.tile_from_name.k2(String.eq(s, "wall"), s)
# The room's two doorway cells on side `dir`.

def layout.Room.lane?(+cols: U32, +rows: U32, +dir: U32) -> LayLane:
  layout.Room.lane.k1(dir, cols, rows, dir)

def layout.Room.lane.k1?(sv0: U32, +cols: U32, +rows: U32, +dir: U32) -> LayLane:
  match sv0:
    case 0:
      LayLane{(layout.half(cols) - 1 : U32), 0, layout.half(cols), 0}
    case 1:
      LayLane{(cols - 1 : U32), (layout.half(rows) - 1 : U32), (cols - 1 : U32), layout.half(rows)}
    case 2:
      LayLane{(layout.half(cols) - 1 : U32), (rows - 1 : U32), layout.half(cols), (rows - 1 : U32)}
    case _:
      LayLane{0, (layout.half(rows) - 1 : U32), 0, layout.half(rows)}
# Open a doorway in side `dir`, where the floor's grid had none.

def layout.Room.open_door?(+room: J.Val, +dir: U32) -> J.Val:
  +g = layout.grid.of_room(room)
  layout.Room.open_door.k1(layout.Room.lane(J.u32(J.get(room, "cols")), J.u32(J.get(room, "rows")), dir), room, dir, g)

def layout.Room.open_door.k1?(pr: LayLane, +room: J.Val, +dir: U32, +g: LayGrid) -> J.Val:
  match pr:
    case LayLane{+c0, +r0, +c1, +r1}:
      +g1 = layout.grid.set(g, c0, r0, TileDoor{})
      +g2 = layout.grid.set(g1, c1, r1, TileDoor{})
      layout.grid.put(J.set(room, "doors", J.set_nth(J.get(room, "doors"), dir, J.JBool{True{}})), g2)
# Open every doorway the room's doors say, from side `i` on.

def layout.Room.open_doors?(+room: J.Val, +i: U32) -> J.Val:
  layout.Room.open_doors.k1(U32.is_ge(i, 4), room, i)

def layout.Room.open_doors.k2?(cv: Bool, +room: J.Val, +i: U32) -> J.Val:
  match cv:
    case True{}:
      layout.Room.open_doors(layout.Room.open_door(room, i), (i + 1 : U32))
    case False{}:
      layout.Room.open_doors(room, (i + 1 : U32))

def layout.Room.open_doors.k1?(cv: Bool, +room: J.Val, +i: U32) -> J.Val:
  match cv:
    case True{}:
      room
    case False{}:
      layout.Room.open_doors.k2(layout.Room.door(room, i), room, i)
# Open floor inside stone walls, `cols` by `rows`.

def layout.grid.walled?(+cols: U32, +rows: U32) -> LayGrid:
  LayGrid{cols, rows, layout.grid.walled.rows(0, cols, rows)}

def layout.grid.walled.rows?(+r: U32, +cols: U32, +rows: U32) -> List<&2, List<&2, LayTile>>:
  layout.grid.walled.rows.k1(U32.is_ge(r, rows), r, cols, rows)

def layout.grid.walled.rows.k1?(cv: Bool, +r: U32, +cols: U32, +rows: U32) -> List<&2, List<&2, LayTile>>:
  match cv:
    case True{}:
      Nil{}
    case False{}:
      Con{layout.grid.walled.row(Bool.or(U32.is_eq(r, 0), U32.is_eq(r, (rows - 1 : U32))), cols), layout.grid.walled.rows((r + 1 : U32), cols, rows)}

def layout.grid.walled.row?(+edge: Bool, +cols: U32) -> List<&2, LayTile>:
  layout.grid.walled.row.k1(edge, edge, cols)

def layout.grid.walled.row.k1?(cv: Bool, +edge: Bool, +cols: U32) -> List<&2, LayTile>:
  match cv:
    case True{}:
      layout.row.fill(cols, TileWall{})
    case False{}:
      Con{TileWall{}, List.append(&2, LayTile, layout.row.fill((cols - 2 : U32), TileFloor{}), Con{TileWall{}, Nil{}})}
# The room laid afresh at `cols` by `rows`: open floor inside stone walls, its
# doorways where they were. What it measured before is kept as `laid`.

def layout.Room.regrow?(+room: J.Val, +cols: U32, +rows: U32) -> J.Val:
  layout.Room.regrow.k1(Bool.and(U32.is_eq(cols, J.u32(J.get(room, "cols"))), U32.is_eq(rows, J.u32(J.get(room, "rows")))), room, cols, rows)

def layout.Room.regrow.k2?(cv: Bool, +room: J.Val) -> J.Val:
  match cv:
    case True{}:
      J.JA{[J.get(room, "cols"), J.get(room, "rows")]}
    case False{}:
      J.get(room, "laid")

def layout.Room.regrow.k1?(cv: Bool, +room: J.Val, +cols: U32, +rows: U32) -> J.Val:
  match cv:
    case True{}:
      room
    case False{}:
      +laid = layout.Room.regrow.k2(J.is_null(J.get(room, "laid")), room)
      layout.Room.open_doors(layout.grid.put(J.set(room, "laid", laid), layout.grid.walled(cols, rows)), 0)
# Stairs down in the middle of the room (a slain dragon's lair).

def layout.Room.open_stairs?(+room: J.Val) -> J.Val:
  +c = (layout.half(J.u32(J.get(room, "cols"))) - 1 : U32)
  +r = (layout.half(J.u32(J.get(room, "rows"))) - 1 : U32)
  +g0 = layout.grid.of_room(room)
  +g1 = layout.grid.set(g0, c, (r + 2 : U32), TileStairs{})
  +g2 = layout.grid.set(g1, (c + 1 : U32), (r + 2 : U32), TileStairs{})
  +g3 = layout.grid.set(g2, c, (r + 3 : U32), TileStairs{})
  +g4 = layout.grid.set(g3, (c + 1 : U32), (r + 3 : U32), TileStairs{})
  layout.grid.put(room, g4)

def layout.tile_is_floor?(+t: LayTile) -> Bool:
  match t:
    case TileFloor{}:
      True{}
    case _:
      False{}

def layout.grid.clear?(+g: LayGrid, +c: U32, +r: U32) -> Bool:
  layout.tile_is_floor(layout.grid.get(g, c, r))
# Where a Sanctuary's privy stands: the first clear two-by-two corner from the
# north-west (rows 2.., cols 2..), its door facing south.

def layout.Room.privy.col?(+g: LayGrid, +row: U32, +col: U32, +end: U32) -> LayFind:
  layout.Room.privy.col.k1(U32.is_ge(col, end), g, row, col, end)

def layout.Room.privy.col.k2?(cv: Bool, +g: LayGrid, +row: U32, +col: U32, +end: U32) -> LayFind:
  match cv:
    case True{}:
      LayFind{True{}, col, row}
    case False{}:
      layout.Room.privy.col(g, row, (col + 1 : U32), end)

def layout.Room.privy.col.k1?(cv: Bool, +g: LayGrid, +row: U32, +col: U32, +end: U32) -> LayFind:
  match cv:
    case True{}:
      LayFind{False{}, 0, 0}
    case False{}:
      layout.Room.privy.col.k2(Bool.and(Bool.and(layout.grid.clear(g, col, row), layout.grid.clear(g, (col + 1 : U32), row)), Bool.and(layout.grid.clear(g, col, (row + 1 : U32)), layout.grid.clear(g, (col + 1 : U32), (row + 1 : U32)))), g, row, col, end)

def layout.Room.privy.row?(+g: LayGrid, +row: U32, +rend: U32, +cend: U32) -> LayFind:
  layout.Room.privy.row.k1(U32.is_ge(row, rend), g, row, rend, cend)

def layout.Room.privy.row.k1?(cv: Bool, +g: LayGrid, +row: U32, +rend: U32, +cend: U32) -> LayFind:
  match cv:
    case True{}:
      LayFind{False{}, 0, 0}
    case False{}:
      +found = layout.Room.privy.col(g, row, 2, cend)
      layout.Room.find_or(found, g, row, rend, cend)

def layout.Room.find_or?(+found: LayFind, +g: LayGrid, +row: U32, +rend: U32, +cend: U32) -> LayFind:
  layout.Room.find_or.k1(found, found, g, row, rend, cend)

def layout.Room.find_or.k2?(cv: Bool, +found: LayFind, +g: LayGrid, +row: U32, +rend: U32, +cend: U32, +ok: Bool) -> LayFind:
  match cv:
    case True{}:
      found
    case False{}:
      layout.Room.privy.row(g, (row + 1 : U32), rend, cend)

def layout.Room.find_or.k1?(sv0: LayFind, +found: LayFind, +g: LayGrid, +row: U32, +rend: U32, +cend: U32) -> LayFind:
  match sv0:
    case LayFind{+ok, +a, +b}:
      layout.Room.find_or.k2(ok, found, g, row, rend, cend, ok)

def layout.Room.privy?(+room: J.Val) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  layout.Room.privy.k1(layout.Room.is_kind(room, "sanctuary"), room)

def layout.Room.privy.k1?(cv: Bool, +room: J.Val) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match cv:
    case True{}:
      +cols = J.u32(J.get(room, "cols"))
      +rows = J.u32(J.get(room, "rows"))
      +found = layout.Room.privy.row(layout.grid.of_room(room), 2, layout.half(rows), (layout.half(cols) - 2 : U32))
      layout.Room.privy.out(found)
    case False{}:
      None{}

def layout.Room.privy.out?(+found: LayFind) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  layout.Room.privy.out.k1(found, found)

def layout.Room.privy.out.k2?(cv: Bool, +ok: Bool, +a: U32, +b: U32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match cv:
    case True{}:
      Some{((U32.to_f32((a + 1 : U32)) * 2.0 : F32), (U32.to_f32((b + 2 : U32)) * 2.0 : F32))}
    case False{}:
      None{}

def layout.Room.privy.out.k1?(sv0: LayFind, +found: LayFind) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match sv0:
    case LayFind{+ok, +a, +b}:
      layout.Room.privy.out.k2(ok, ok, a, b)
# Where a Sanctuary's field banner stands: the first clear tile down the east
# side from the middle.

def layout.Room.field_banner.col?(+g: LayGrid, +row: U32, +col: U32, +low: U32) -> LayFind:
  layout.Room.field_banner.col.k1(U32.is_lt(col, low), g, row, col, low)

def layout.Room.field_banner.col.k2?(cv: Bool, +g: LayGrid, +row: U32, +col: U32, +low: U32) -> LayFind:
  match cv:
    case True{}:
      LayFind{True{}, col, row}
    case False{}:
      layout.Room.field_banner.col(g, row, (col - 1 : U32), low)

def layout.Room.field_banner.col.k1?(cv: Bool, +g: LayGrid, +row: U32, +col: U32, +low: U32) -> LayFind:
  match cv:
    case True{}:
      LayFind{False{}, 0, 0}
    case False{}:
      layout.Room.field_banner.col.k2(Bool.and(layout.grid.clear(g, col, row), layout.grid.clear(g, col, (row + 1 : U32))), g, row, col, low)

def layout.Room.field_banner.row?(+g: LayGrid, +row: U32, +rend: U32, +hi: U32, +low: U32) -> LayFind:
  layout.Room.field_banner.row.k1(U32.is_ge(row, rend), g, row, rend, hi, low)

def layout.Room.field_banner.row.k1?(cv: Bool, +g: LayGrid, +row: U32, +rend: U32, +hi: U32, +low: U32) -> LayFind:
  match cv:
    case True{}:
      LayFind{False{}, 0, 0}
    case False{}:
      +found = layout.Room.field_banner.col(g, row, hi, low)
      layout.Room.field_banner.next(found, g, row, rend, hi, low)

def layout.Room.field_banner.next?(+found: LayFind, +g: LayGrid, +row: U32, +rend: U32, +hi: U32, +low: U32) -> LayFind:
  layout.Room.field_banner.next.k1(found, found, g, row, rend, hi, low)

def layout.Room.field_banner.next.k2?(cv: Bool, +found: LayFind, +g: LayGrid, +row: U32, +rend: U32, +hi: U32, +low: U32, +ok: Bool) -> LayFind:
  match cv:
    case True{}:
      found
    case False{}:
      layout.Room.field_banner.row(g, (row + 1 : U32), rend, hi, low)

def layout.Room.field_banner.next.k1?(sv0: LayFind, +found: LayFind, +g: LayGrid, +row: U32, +rend: U32, +hi: U32, +low: U32) -> LayFind:
  match sv0:
    case LayFind{+ok, +a, +b}:
      layout.Room.field_banner.next.k2(ok, found, g, row, rend, hi, low, ok)

def layout.Room.field_banner?(+room: J.Val) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  layout.Room.field_banner.k1(layout.Room.is_kind(room, "sanctuary"), room)

def layout.Room.field_banner.k1?(cv: Bool, +room: J.Val) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match cv:
    case True{}:
      +cols = J.u32(J.get(room, "cols"))
      +rows = J.u32(J.get(room, "rows"))
      +found = layout.Room.field_banner.row(layout.grid.of_room(room), (layout.half(rows) - 1 : U32), (rows - 3 : U32), (cols - 3 : U32), (layout.half(cols) + 2 : U32))
      layout.Room.field_banner.out(found)
    case False{}:
      None{}

def layout.Room.field_banner.out?(+found: LayFind) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  layout.Room.field_banner.out.k1(found, found)

def layout.Room.field_banner.out.k2?(cv: Bool, +ok: Bool, +a: U32, +b: U32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match cv:
    case True{}:
      Some{(((U32.to_f32(a) + 0.5 : F32) * 2.0 : F32), ((U32.to_f32(b) + 1.0 : F32) * 2.0 : F32))}
    case False{}:
      None{}

def layout.Room.field_banner.out.k1?(sv0: LayFind, +found: LayFind) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match sv0:
    case LayFind{+ok, +a, +b}:
      layout.Room.field_banner.out.k2(ok, ok, a, b)
# Whether any tile of the run string is stairs.

def layout.rle.has_stairs?(+s: String) -> Bool:
  layout.rle.has_stairs.k1(s, s)

def layout.rle.has_stairs.k2?(cv: Bool, +c: Char, +t: String) -> Bool:
  match cv:
    case True{}:
      True{}
    case False{}:
      layout.rle.has_stairs(t)

def layout.rle.has_stairs.k1?(sv0: String, +s: String) -> Bool:
  match sv0:
    case SNil{}:
      False{}
    case SCon{+c, +t}:
      layout.rle.has_stairs.k2(Char.is_eq(c, '>'), c, t)
# Where a Sanctuary's altar stands: the middle, or above the stairs when the
# room keeps them.

def layout.Room.altar?(+room: J.Val) -> Sigma<&2, &2, F32, _ => F32>:
  +x = (layout.Room.width(room) / 2.0 : F32)
  +y = (layout.Room.height(room) / 2.0 : F32)
  layout.Room.altar.k1(layout.rle.has_stairs(J.str(J.get(room, "tiles"))), room, x, y)

def layout.Room.altar.k1?(cv: Bool, +room: J.Val, +x: F32, +y: F32) -> Sigma<&2, &2, F32, _ => F32>:
  match cv:
    case True{}:
      (x, (y - 6.0 : F32))
    case False{}:
      (x, y)

def rules.unit?(+x: F32, +y: F32) -> F32 & F32:
  +length = {RS.f_hypot(x, y) : F32}
  rules.unit.if2(F32.is_gt(length, 0.0), x, y, length)

def rules.unit.k1?() -> F32 & F32:
  (0.0, 0.0)

def rules.unit.if2?(_c: Bool, +x: F32, +y: F32, +length: F32) -> F32 & F32:
  match _c:
    case True{}:
      ((x / length : F32), (y / length : F32))
    case False{}:
      rules.unit.k1()

# Off the edge reads as the edge tile, so an open doorway lets a knight step

# through it and out of the room.

def rules.Grid.tile_at?(+room: J.Val, +x: F32, +y: F32) -> LayTile:
  +cols = {layout.Room.cols(room) : U32}
  +rows = {layout.Room.rows(room) : U32}
  +col = {RS.i_clamp(RS.f_as_i32(F32.floor((x / layout.TILE_UNITS() : F32))), 0, RS.i_sub(cols, 1)) : U32}
  +row = {RS.i_clamp(RS.f_as_i32(F32.floor((y / layout.TILE_UNITS() : F32))), 0, RS.i_sub(rows, 1)) : U32}
  layout.Room.tile_t(room, col, row)

def rules.Grid.open_tile?(+t: LayTile, +barred: Bool, +mover: U32) -> Bool:
  rules.Grid.open_tile.m1(t, barred, mover)

def rules.Grid.open_tile.m1?(+s: LayTile, +barred: Bool, +mover: U32) -> Bool:
  match s:
    case TileFloor{}:
      True{}
    case TileStairs{}:
      True{}
    case TileLedge{}:
      True{}
    case TileBog{}:
      True{}
    case TileWall{}:
      False{}
    case TileBlock{}:
      False{}
    case TileHazard{}:
      U32.is_eq(mover, 2)
    case TileDoor{}:
      Bool.and(U32.is_zero(mover), Bool.not(barred))

def rules.Grid.open?(+room: J.Val, +barred: Bool, +x: F32, +y: F32, +mover: U32) -> Bool:
  rules.Grid.open_tile(rules.Grid.tile_at(room, x, y), barred, mover)

def rules.Grid.clear?(+room: J.Val, +barred: Bool, +x: F32, +y: F32, +r: F32, +mover: U32) -> Bool:
  +a = {rules.Grid.open(room, barred, (x + F32.neg(r) : F32), (y + F32.neg(r) : F32), mover) : Bool}
  +b = {rules.Grid.open(room, barred, (x + r : F32), (y + F32.neg(r) : F32), mover) : Bool}
  +c = {rules.Grid.open(room, barred, (x + F32.neg(r) : F32), (y + r : F32), mover) : Bool}
  +d = {rules.Grid.open(room, barred, (x + r : F32), (y + r : F32), mover) : Bool}
  Bool.and(Bool.and(a, b), Bool.and(c, d))

# Whether the tile is the bog (a mover wading it is slowed).

def rules.Grid.is_bog?(+t: LayTile) -> Bool:
  rules.Grid.is_bog.m1(t)

def rules.Grid.is_bog.m1?(+s: LayTile) -> Bool:
  match s:
    case TileBog{}:
      True{}
    case _:
      False{}

def rules.Grid.is_door?(+t: LayTile) -> Bool:
  rules.Grid.is_door.m1(t)

def rules.Grid.is_door.m1?(+s: LayTile) -> Bool:
  match s:
    case TileDoor{}:
      True{}
    case _:
      False{}

def rules.Grid.is_block?(+t: LayTile) -> Bool:
  rules.Grid.is_block.m1(t)

def rules.Grid.is_block.m1?(+s: LayTile) -> Bool:
  match s:
    case TileBlock{}:
      True{}
    case _:
      False{}

# Move with wall sliding, one axis at a time. Feet in the bog go at

# `swamp::BOG_PACE`; wings and the swamp's own waders do not.

def rules.Grid.slide?(+room: J.Val, +barred: Bool, p: F32 & F32, d: F32 & F32, +r: F32, +mover: U32) -> F32 & F32:
  rules.Grid.slide.k1(p, room, barred, d, r, mover)

def rules.Grid.slide.k6?(+x: F32, +y: F32) -> F32 & F32:
  (x, y)

def rules.Grid.slide.if7?(_c: Bool, +x: F32, +y: F32, +dy: F32) -> F32 & F32:
  match _c:
    case True{}:
      +y = {(y + dy : F32) : F32}
      rules.Grid.slide.k6(x, y)
    case False{}:
      rules.Grid.slide.k6(x, y)

def rules.Grid.slide.k5?(+room: J.Val, +barred: Bool, +r: F32, +mover: U32, +x: F32, +y: F32, +dy: F32) -> F32 & F32:
  rules.Grid.slide.if7(rules.Grid.clear(room, barred, x, (y + dy : F32), r, mover), x, y, dy)

def rules.Grid.slide.if8?(_c: Bool, +room: J.Val, +barred: Bool, +r: F32, +mover: U32, +x: F32, +y: F32, +dx: F32, +dy: F32) -> F32 & F32:
  match _c:
    case True{}:
      +x = {(x + dx : F32) : F32}
      rules.Grid.slide.k5(room, barred, r, mover, x, y, dy)
    case False{}:
      rules.Grid.slide.k5(room, barred, r, mover, x, y, dy)

def rules.Grid.slide.k4?(+room: J.Val, +barred: Bool, +r: F32, +mover: U32, +x: F32, +y: F32, +dx: F32, +dy: F32) -> F32 & F32:
  rules.Grid.slide.if8(rules.Grid.clear(room, barred, (x + dx : F32), y, r, mover), room, barred, r, mover, x, y, dx, dy)

def rules.Grid.slide.k11?(+room: J.Val, +barred: Bool, +r: F32, +mover: U32, +x: F32, +y: F32, +dx: F32, +dy: F32) -> F32 & F32:
  (x, y)

def rules.Grid.slide.if12?(_c: Bool, +room: J.Val, +barred: Bool, +r: F32, +mover: U32, +x: F32, +y: F32, +dx: F32, +dy: F32) -> F32 & F32:
  match _c:
    case True{}:
      +y = {(y + dy : F32) : F32}
      rules.Grid.slide.k11(room, barred, r, mover, x, y, dx, dy)
    case False{}:
      rules.Grid.slide.k11(room, barred, r, mover, x, y, dx, dy)

def rules.Grid.slide.k10?(+room: J.Val, +barred: Bool, +r: F32, +mover: U32, +x: F32, +y: F32, +dx: F32, +dy: F32, +cy: F32) -> F32 & F32:
  rules.Grid.slide.if12(F32.is_gt((dy * (cy - y : F32) : F32), 0.0), room, barred, r, mover, x, y, dx, dy)

def rules.Grid.slide.if13?(_c: Bool, +room: J.Val, +barred: Bool, +r: F32, +mover: U32, +x: F32, +y: F32, +dx: F32, +dy: F32, +cy: F32) -> F32 & F32:
  match _c:
    case True{}:
      +x = {(x + dx : F32) : F32}
      rules.Grid.slide.k10(room, barred, r, mover, x, y, dx, dy, cy)
    case False{}:
      rules.Grid.slide.k10(room, barred, r, mover, x, y, dx, dy, cy)

def rules.Grid.slide.if9?(_c: Bool, +room: J.Val, +barred: Bool, +r: F32, +mover: U32, +x: F32, +y: F32, +dx: F32, +dy: F32) -> F32 & F32:
  match _c:
    case True{}:
      +cx = {(U32.to_f32(layout.Room.cols(room)) * layout.TILE_UNITS() / 2.0 : F32) : F32}
      +cy = {(U32.to_f32(layout.Room.rows(room)) * layout.TILE_UNITS() / 2.0 : F32) : F32}
      rules.Grid.slide.if13(F32.is_gt((dx * (cx - x : F32) : F32), 0.0), room, barred, r, mover, x, y, dx, dy, cy)
    case False{}:
      rules.Grid.slide.k4(room, barred, r, mover, x, y, dx, dy)

def rules.Grid.slide.k3?(+room: J.Val, +barred: Bool, +r: F32, +mover: U32, +x: F32, +y: F32, +dx: F32, +dy: F32) -> F32 & F32:
  rules.Grid.slide.if9(Bool.and(U32.is_zero(mover), Bool.not(rules.Grid.clear(room, barred, x, y, r, mover))), room, barred, r, mover, x, y, dx, dy)

def rules.Grid.slide.if14?(_c: Bool, +room: J.Val, +barred: Bool, +r: F32, +mover: U32, +x: F32, +y: F32, +dx: F32, +dy: F32) -> F32 & F32:
  match _c:
    case True{}:
      +dx = {(dx * swamp.BOG_PACE() : F32) : F32}
      +dy = {(dy * swamp.BOG_PACE() : F32) : F32}
      rules.Grid.slide.k3(room, barred, r, mover, x, y, dx, dy)
    case False{}:
      rules.Grid.slide.k3(room, barred, r, mover, x, y, dx, dy)

def rules.Grid.slide.k2?(t: F32 & F32, +room: J.Val, +barred: Bool, +r: F32, +mover: U32, +x: F32, +y: F32) -> F32 & F32:
  (+dx, +dy) = t
  rules.Grid.slide.if14(Bool.and(Bool.or(U32.is_zero(mover), U32.is_eq(mover, 1)), rules.Grid.is_bog(rules.Grid.tile_at(room, x, y))), room, barred, r, mover, x, y, dx, dy)

def rules.Grid.slide.k1?(t: F32 & F32, +room: J.Val, +barred: Bool, d: F32 & F32, +r: F32, +mover: U32) -> F32 & F32:
  (+x, +y) = t
  rules.Grid.slide.k2(d, room, barred, r, mover, x, y)

# A knight's displacement, stopped at walls and barred doors the way walking is.

def rules.Grid.halted?(+room: J.Val, +barred: Bool, from: F32 & F32, d: F32 & F32) -> F32 & F32:
  rules.Grid.halted.k1(from, room, barred, d)

def rules.Grid.halted.k5?(+ax: F32, +ay: F32) -> F32 & F32:
  (ax, ay)

def rules.Grid.halted.k9?(+room: J.Val, +barred: Bool, +dist: F32, +ux: F32, +uy: F32, +ax: F32, +ay: F32, +gone: F32, +hop: F32, +nx: F32, +ny: F32) -> F32 & F32:
  +ax = {nx : F32}
  +ay = {ny : F32}
  +gone = {(gone + hop : F32) : F32}
  rules.Grid.halted.loop6(room, barred, dist, ux, uy, ax, ay, gone)

def rules.Grid.halted.if10?(_c: Bool, +room: J.Val, +barred: Bool, +dist: F32, +ux: F32, +uy: F32, +ax: F32, +ay: F32, +gone: F32, +hop: F32, +nx: F32, +ny: F32) -> F32 & F32:
  match _c:
    case True{}:
      (ax, ay)
    case False{}:
      rules.Grid.halted.k9(room, barred, dist, ux, uy, ax, ay, gone, hop, nx, ny)

def rules.Grid.halted.k8?(t: F32 & F32, +room: J.Val, +barred: Bool, +dist: F32, +ux: F32, +uy: F32, +ax: F32, +ay: F32, +gone: F32, +hop: F32) -> F32 & F32:
  (+nx, +ny) = t
  rules.Grid.halted.if10(Bool.and(F32.is_lt(F32.abs((nx - ax : F32)), 0.00001), F32.is_lt(F32.abs((ny - ay : F32)), 0.00001)), room, barred, dist, ux, uy, ax, ay, gone, hop, nx, ny)

def rules.Grid.halted.if7?(_c: Bool, +room: J.Val, +barred: Bool, +dist: F32, +ux: F32, +uy: F32, +ax: F32, +ay: F32, +gone: F32) -> F32 & F32:
  match _c:
    case True{}:
      +hop = {RS.f_min(0.4, (dist - gone : F32)) : F32}
      rules.Grid.halted.k8(rules.Grid.slide(room, barred, (ax, ay), ((ux * hop : F32), (uy * hop : F32)), rules.HERO_RADIUS(), 0), room, barred, dist, ux, uy, ax, ay, gone, hop)
    case False{}:
      rules.Grid.halted.k5(ax, ay)

def rules.Grid.halted.loop6?(+room: J.Val, +barred: Bool, +dist: F32, +ux: F32, +uy: F32, +ax: F32, +ay: F32, +gone: F32) -> F32 & F32:
  rules.Grid.halted.if7(F32.is_lt(gone, (dist - 0.0001 : F32)), room, barred, dist, ux, uy, ax, ay, gone)

def rules.Grid.halted.k4?(+room: J.Val, +barred: Bool, +fx: F32, +fy: F32, +dx: F32, +dy: F32, +dist: F32) -> F32 & F32:
  +ux = {(dx / dist : F32) : F32}
  +uy = {(dy / dist : F32) : F32}
  +ax = {fx : F32}
  +ay = {fy : F32}
  +gone = {0.0 : F32}
  rules.Grid.halted.loop6(room, barred, dist, ux, uy, ax, ay, gone)

def rules.Grid.halted.if11?(_c: Bool, +room: J.Val, +barred: Bool, +fx: F32, +fy: F32, +dx: F32, +dy: F32, +dist: F32) -> F32 & F32:
  match _c:
    case True{}:
      rules.Grid.slide(room, barred, (fx, fy), (dx, dy), rules.HERO_RADIUS(), 0)
    case False{}:
      rules.Grid.halted.k4(room, barred, fx, fy, dx, dy, dist)

def rules.Grid.halted.k3?(+room: J.Val, +barred: Bool, +fx: F32, +fy: F32, +dx: F32, +dy: F32, +dist: F32) -> F32 & F32:
  rules.Grid.halted.if11(Bool.not(rules.Grid.clear(room, barred, (fx + dx : F32), (fy + dy : F32), rules.HERO_RADIUS(), 0)), room, barred, fx, fy, dx, dy, dist)

def rules.Grid.halted.if12?(_c: Bool, +room: J.Val, +barred: Bool, +fx: F32, +fy: F32, +dx: F32, +dy: F32, +dist: F32) -> F32 & F32:
  match _c:
    case True{}:
      (fx, fy)
    case False{}:
      rules.Grid.halted.k3(room, barred, fx, fy, dx, dy, dist)

def rules.Grid.halted.k2?(t: F32 & F32, +room: J.Val, +barred: Bool, +fx: F32, +fy: F32) -> F32 & F32:
  (+dx, +dy) = t
  +dist = {RS.f_hypot(dx, dy) : F32}
  rules.Grid.halted.if12(F32.is_lt(dist, 0.000001), room, barred, fx, fy, dx, dy, dist)

def rules.Grid.halted.k1?(t: F32 & F32, +room: J.Val, +barred: Bool, d: F32 & F32) -> F32 & F32:
  (+fx, +fy) = t
  rules.Grid.halted.k2(d, room, barred, fx, fy)

# Whether a point in a doorway tile is met a step along the way (for `across_bar`).

def rules.Grid.in_door?(+room: J.Val, +nx: F32, +ny: F32, +r: F32) -> Bool:
  +a = {rules.Grid.is_door(rules.Grid.tile_at(room, nx, ny)) : Bool}
  +b = {rules.Grid.is_door(rules.Grid.tile_at(room, (nx + r : F32), ny)) : Bool}
  +c = {rules.Grid.is_door(rules.Grid.tile_at(room, (nx + F32.neg(r) : F32), ny)) : Bool}
  +d = {rules.Grid.is_door(rules.Grid.tile_at(room, nx, (ny + r : F32))) : Bool}
  +e = {rules.Grid.is_door(rules.Grid.tile_at(room, nx, (ny + F32.neg(r) : F32))) : Bool}
  Bool.or(Bool.or(Bool.or(a, b), Bool.or(c, d)), e)

# Where a placed knight lands: a rider sits on its carrier and a risen knight

# takes the body's place, but a barred door on the straight way stops them.

def rules.Grid.across_bar?(+room: J.Val, +barred: Bool, from: F32 & F32, to: F32 & F32) -> F32 & F32:
  rules.Grid.across_bar.k1(from, room, barred, to)

def rules.Grid.across_bar.k6?(+ax: F32, +ay: F32) -> F32 & F32:
  (ax, ay)

def rules.Grid.across_bar.k9?(+room: J.Val, +dist: F32, +ux: F32, +uy: F32, +gone: F32, +ax: F32, +ay: F32, +hop: F32, +nx: F32, +ny: F32) -> F32 & F32:
  +ax = {nx : F32}
  +ay = {ny : F32}
  +gone = {(gone + hop : F32) : F32}
  rules.Grid.across_bar.loop7(room, dist, ux, uy, gone, ax, ay)

def rules.Grid.across_bar.if10?(_c: Bool, +room: J.Val, +dist: F32, +ux: F32, +uy: F32, +gone: F32, +ax: F32, +ay: F32, +hop: F32, +nx: F32, +ny: F32) -> F32 & F32:
  match _c:
    case True{}:
      (ax, ay)
    case False{}:
      rules.Grid.across_bar.k9(room, dist, ux, uy, gone, ax, ay, hop, nx, ny)

def rules.Grid.across_bar.if8?(_c: Bool, +room: J.Val, +dist: F32, +ux: F32, +uy: F32, +gone: F32, +ax: F32, +ay: F32) -> F32 & F32:
  match _c:
    case True{}:
      +hop = {RS.f_min(0.2, (dist - gone : F32)) : F32}
      +nx = {(ax + ux * hop : F32) : F32}
      +ny = {(ay + uy * hop : F32) : F32}
      rules.Grid.across_bar.if10(rules.Grid.in_door(room, nx, ny, rules.HERO_RADIUS()), room, dist, ux, uy, gone, ax, ay, hop, nx, ny)
    case False{}:
      rules.Grid.across_bar.k6(ax, ay)

def rules.Grid.across_bar.loop7?(+room: J.Val, +dist: F32, +ux: F32, +uy: F32, +gone: F32, +ax: F32, +ay: F32) -> F32 & F32:
  rules.Grid.across_bar.if8(F32.is_lt(gone, (dist - 0.0001 : F32)), room, dist, ux, uy, gone, ax, ay)

def rules.Grid.across_bar.k5?(+room: J.Val, +fx: F32, +fy: F32, +dist: F32, +ux: F32, +uy: F32, +gone: F32) -> F32 & F32:
  +ax = {fx : F32}
  +ay = {fy : F32}
  +gone = {0.0 : F32}
  rules.Grid.across_bar.loop7(room, dist, ux, uy, gone, ax, ay)

def rules.Grid.across_bar.if11?(_c: Bool, +room: J.Val, +fx: F32, +fy: F32, +tx: F32, +ty: F32, +dist: F32, +ux: F32, +uy: F32, +gone: F32) -> F32 & F32:
  match _c:
    case True{}:
      (tx, ty)
    case False{}:
      rules.Grid.across_bar.k5(room, fx, fy, dist, ux, uy, gone)

def rules.Grid.across_bar.k4?(+room: J.Val, +fx: F32, +fy: F32, +tx: F32, +ty: F32, +dist: F32, +ux: F32, +uy: F32, +gone: F32, +hits: Bool) -> F32 & F32:
  rules.Grid.across_bar.if11(Bool.not(hits), room, fx, fy, tx, ty, dist, ux, uy, gone)

def rules.Grid.across_bar.k14?(+room: J.Val, +fx: F32, +fy: F32, +tx: F32, +ty: F32, +dist: F32, +ux: F32, +uy: F32, +gone: F32, +hits: Bool) -> F32 & F32:
  rules.Grid.across_bar.loop12(room, fx, fy, tx, ty, dist, ux, uy, gone, hits)

def rules.Grid.across_bar.if15?(_c: Bool, +room: J.Val, +fx: F32, +fy: F32, +tx: F32, +ty: F32, +dist: F32, +ux: F32, +uy: F32, +gone: F32, +hits: Bool) -> F32 & F32:
  match _c:
    case True{}:
      +hits = {True{} : Bool}
      rules.Grid.across_bar.k4(room, fx, fy, tx, ty, dist, ux, uy, gone, hits)
    case False{}:
      rules.Grid.across_bar.k14(room, fx, fy, tx, ty, dist, ux, uy, gone, hits)

def rules.Grid.across_bar.if13?(_c: Bool, +room: J.Val, +fx: F32, +fy: F32, +tx: F32, +ty: F32, +dist: F32, +ux: F32, +uy: F32, +gone: F32, +hits: Bool) -> F32 & F32:
  match _c:
    case True{}:
      +gone = {(gone + RS.f_min(0.2, (dist - gone : F32)) : F32) : F32}
      rules.Grid.across_bar.if15(rules.Grid.is_door(rules.Grid.tile_at(room, (fx + ux * gone : F32), (fy + uy * gone : F32))), room, fx, fy, tx, ty, dist, ux, uy, gone, hits)
    case False{}:
      rules.Grid.across_bar.k4(room, fx, fy, tx, ty, dist, ux, uy, gone, hits)

def rules.Grid.across_bar.loop12?(+room: J.Val, +fx: F32, +fy: F32, +tx: F32, +ty: F32, +dist: F32, +ux: F32, +uy: F32, +gone: F32, +hits: Bool) -> F32 & F32:
  rules.Grid.across_bar.if13(F32.is_lt(gone, (dist - 0.0001 : F32)), room, fx, fy, tx, ty, dist, ux, uy, gone, hits)

def rules.Grid.across_bar.k3?(+room: J.Val, +fx: F32, +fy: F32, +tx: F32, +ty: F32, +dx: F32, +dy: F32, +dist: F32) -> F32 & F32:
  +ux = {(dx / dist : F32) : F32}
  +uy = {(dy / dist : F32) : F32}
  +gone = {0.0 : F32}
  +hits = {False{} : Bool}
  rules.Grid.across_bar.loop12(room, fx, fy, tx, ty, dist, ux, uy, gone, hits)

def rules.Grid.across_bar.if16?(_c: Bool, +room: J.Val, +fx: F32, +fy: F32, +tx: F32, +ty: F32, +dx: F32, +dy: F32, +dist: F32) -> F32 & F32:
  match _c:
    case True{}:
      (tx, ty)
    case False{}:
      rules.Grid.across_bar.k3(room, fx, fy, tx, ty, dx, dy, dist)

def rules.Grid.across_bar.k2?(t: F32 & F32, +room: J.Val, +barred: Bool, +fx: F32, +fy: F32) -> F32 & F32:
  (+tx, +ty) = t
  +dx = {(tx - fx : F32) : F32}
  +dy = {(ty - fy : F32) : F32}
  +dist = {RS.f_hypot(dx, dy) : F32}
  rules.Grid.across_bar.if16(Bool.or(Bool.not(barred), F32.is_lt(dist, 0.000001)), room, fx, fy, tx, ty, dx, dy, dist)

def rules.Grid.across_bar.k1?(t: F32 & F32, +room: J.Val, +barred: Bool, to: F32 & F32) -> F32 & F32:
  (+fx, +fy) = t
  rules.Grid.across_bar.k2(to, room, barred, fx, fy)

def rules.Grid.stops_shot?(+room: J.Val, +barred: Bool, +x: F32, +y: F32) -> Bool:
  rules.Grid.stops_shot.if3(Bool.or(Bool.or(F32.is_lt(x, 0.0), F32.is_ge(x, layout.Room.width(room))), Bool.or(F32.is_lt(y, 0.0), F32.is_ge(y, layout.Room.height(room)))), room, barred, x, y)

def rules.Grid.stops_shot.m2?(+s: LayTile, +barred: Bool) -> Bool:
  match s:
    case TileWall{}:
      True{}
    case TileBlock{}:
      True{}
    case TileDoor{}:
      barred
    case _:
      False{}

def rules.Grid.stops_shot.k1?(+room: J.Val, +barred: Bool, +x: F32, +y: F32) -> Bool:
  rules.Grid.stops_shot.m2(rules.Grid.tile_at(room, x, y), barred)

def rules.Grid.stops_shot.if3?(_c: Bool, +room: J.Val, +barred: Bool, +x: F32, +y: F32) -> Bool:
  match _c:
    case True{}:
      True{}
    case False{}:
      rules.Grid.stops_shot.k1(room, barred, x, y)

# A shot whose body is `body` units round its middle: stopped by stone at its

# middle, and by a prop wherever its body touches one.

def rules.Grid.stops_body?(+room: J.Val, +barred: Bool, +x: F32, +y: F32, +body: F32) -> Bool:
  rules.Grid.stops_body.if4(rules.Grid.stops_shot(room, barred, x, y), room, x, y, body)

def rules.Grid.stops_body.k2?(+room: J.Val, +x: F32, +y: F32, +body: F32) -> Bool:
  +a = {rules.Grid.is_block(rules.Grid.tile_at(room, (x + body : F32), y)) : Bool}
  +b = {rules.Grid.is_block(rules.Grid.tile_at(room, (x + F32.neg(body) : F32), y)) : Bool}
  +c = {rules.Grid.is_block(rules.Grid.tile_at(room, x, (y + body : F32))) : Bool}
  +d = {rules.Grid.is_block(rules.Grid.tile_at(room, x, (y + F32.neg(body) : F32))) : Bool}
  Bool.or(Bool.or(a, b), Bool.or(c, d))

def rules.Grid.stops_body.if3?(_c: Bool, +room: J.Val, +x: F32, +y: F32, +body: F32) -> Bool:
  match _c:
    case True{}:
      False{}
    case False{}:
      rules.Grid.stops_body.k2(room, x, y, body)

def rules.Grid.stops_body.k1?(+room: J.Val, +x: F32, +y: F32, +body: F32) -> Bool:
  rules.Grid.stops_body.if3(Bool.not(F32.is_gt(body, 0.0)), room, x, y, body)

def rules.Grid.stops_body.if4?(_c: Bool, +room: J.Val, +x: F32, +y: F32, +body: F32) -> Bool:
  match _c:
    case True{}:
      True{}
    case False{}:
      rules.Grid.stops_body.k1(room, x, y, body)

# `shot_stop_body`: where it first touches stone or a prop on its way from `from` to `to`.

def rules.Grid.shot_stop_body?(+room: J.Val, +barred: Bool, from: F32 & F32, to: F32 & F32, +body: F32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  rules.Grid.shot_stop_body.k1(from, room, barred, to, body)

def rules.Grid.shot_stop_body.k3?() -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  None{}

def rules.Grid.shot_stop_body.k6?(+room: J.Val, +barred: Bool, +body: F32, +fx: F32, +fy: F32, +dx: F32, +dy: F32, +steps: U32, +i: U32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  +i = {(i + 1 : U32) : U32}
  rules.Grid.shot_stop_body.loop4(room, barred, body, fx, fy, dx, dy, steps, i)

def rules.Grid.shot_stop_body.if7?(_c: Bool, +room: J.Val, +barred: Bool, +body: F32, +fx: F32, +fy: F32, +dx: F32, +dy: F32, +steps: U32, +i: U32, +ax: F32, +ay: F32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match _c:
    case True{}:
      Some{(ax, ay)}
    case False{}:
      rules.Grid.shot_stop_body.k6(room, barred, body, fx, fy, dx, dy, steps, i)

def rules.Grid.shot_stop_body.if5?(_c: Bool, +room: J.Val, +barred: Bool, +body: F32, +fx: F32, +fy: F32, +dx: F32, +dy: F32, +steps: U32, +i: U32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match _c:
    case True{}:
      +t = {(RS.u_as_f32(i) / RS.u_as_f32(steps) : F32) : F32}
      +ax = {(fx + dx * t : F32) : F32}
      +ay = {(fy + dy * t : F32) : F32}
      rules.Grid.shot_stop_body.if7(rules.Grid.stops_body(room, barred, ax, ay, body), room, barred, body, fx, fy, dx, dy, steps, i, ax, ay)
    case False{}:
      rules.Grid.shot_stop_body.k3()

def rules.Grid.shot_stop_body.loop4?(+room: J.Val, +barred: Bool, +body: F32, +fx: F32, +fy: F32, +dx: F32, +dy: F32, +steps: U32, +i: U32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  rules.Grid.shot_stop_body.if5(U32.is_le(i, steps), room, barred, body, fx, fy, dx, dy, steps, i)

def rules.Grid.shot_stop_body.k2?(t: F32 & F32, +room: J.Val, +barred: Bool, +body: F32, +fx: F32, +fy: F32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  (+tx, +ty) = t
  +dx = {(tx - fx : F32) : F32}
  +dy = {(ty - fy : F32) : F32}
  +steps = {RS.f_as_u32(RS.f_clamp(F32.ceil((RS.f_hypot(dx, dy) / 0.25 : F32)), 1.0, 256.0)) : U32}
  +i = {1 : U32}
  rules.Grid.shot_stop_body.loop4(room, barred, body, fx, fy, dx, dy, steps, i)

def rules.Grid.shot_stop_body.k1?(t: F32 & F32, +room: J.Val, +barred: Bool, to: F32 & F32, +body: F32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  (+fx, +fy) = t
  rules.Grid.shot_stop_body.k2(to, room, barred, body, fx, fy)

def rules.Grid.shot_stop?(+room: J.Val, +barred: Bool, from: F32 & F32, to: F32 & F32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  rules.Grid.shot_stop_body(room, barred, from, to, 0.0)

def rules.WIDTH?() -> F32:
  48.0

def rules.HEIGHT?() -> F32:
  28.0

def rules.HZ?() -> U32:
  30

def rules.FLOORS?() -> U32:
  3

def rules.DEEPEST?() -> U32:
  6

def rules.DEEP_FLOOR?() -> U32:
  4

def rules.MAX_PROJECTILES?() -> U32:
  384

def rules.KNIGHT_SHOTS?() -> U32:
  48

def rules.KNIGHT_CHILDREN?() -> U32:
  32

def rules.MAX_BOMBS?() -> U32:
  5

def rules.MAX_ARMOR?() -> U32:
  2

def rules.MAX_PLAYERS?() -> U32:
  6

def rules.AWAY_TICKS?() -> U32:
  600

def rules.MAX_ITEMS?() -> U32:
  24

def rules.DT?() -> F32:
  0.03333333507180214

def rules.HERO_RADIUS?() -> F32:
  0.41999998688697815

def rules.HERO_SPEED?() -> F32:
  13.0

def rules.ROLL_TICKS?() -> U32:
  10

def rules.ROLL_SPEED?() -> F32:
  27.0

def rules.ROLL_REARM?() -> U32:
  36

def rules.SHIELD_PACE?() -> F32:
  0.44999998807907104

def rules.MAX_MANA?() -> F32:
  100.0

def rules.MANA_REST?() -> U32:
  15

def rules.MANA_FLOW?() -> F32:
  22.0

def rules.WALL_REACH?() -> F32:
  1.7000000476837158

def rules.WALL_PACE?() -> F32:
  0.6000000238418579

def rules.TURN_TO_WALK?() -> U32:
  15

def rules.BOSS_ALONE_TO?() -> U32:
  3

def rules.BOSS_CLEARANCE?() -> F32:
  12.0

def rules.BOMB_DAMAGE?() -> U32:
  60

def rules.REVIVE_SHARE?() -> U32:
  25

def rules.REVIVE_WARD?() -> U32:
  60

def rules.ENTRY_WARD?() -> U32:
  30

def rules.BOMB_CLEAR?() -> F32:
  9.0

def rules.BOMB_REARM?() -> U32:
  30

def rules.VIGIL_REACH?() -> F32:
  7.0

def rules.VIGIL_MEND_EVERY?() -> U32:
  10

def rules.VIGIL_SETTLE?() -> U32:
  30

def rules.VIGIL_REARM?() -> U32:
  150

def rules.PICKUP_REACH?() -> F32:
  1.2999999523162842

def rules.TAKE_REACH?() -> F32:
  2.4000000953674316

def rules.HAND_FULL?() -> String:
  "hand full"

def rules.PLAY_REARM?() -> U32:
  15

def rules.FAN?() -> F32:
  0.20000000298023224

def rules.VOLLEY_FAN?() -> F32:
  0.3499999940395355

def rules.CHAIN_REACH?() -> F32:
  6.5

def rules.BLAST_TICKS?() -> U32:
  9

def rules.BURST_REACH?() -> F32:
  2.5

def rules.BLADE_SPEED?() -> F32:
  20.0

def rules.ORBIT_RADIUS?() -> F32:
  2.4000000953674316

def rules.ORBIT_SPIN?() -> F32:
  5.0

def rules.ORBIT_EVERY?() -> U32:
  10

def rules.ORBIT_DAMAGE?() -> U32:
  12

def rules.HOMING_TURN?() -> F32:
  2.200000047683716

def rules.SWORD_DAMAGE?() -> U32:
  30

def rules.SWORD_REACH?() -> F32:
  2.200000047683716

def rules.SWORD_ARC?() -> F32:
  130.0

def rules.SWORD_REARM?() -> U32:
  18

def rules.PRIVY_REACH?() -> F32:
  1.2000000476837158

def rules.TELEGRAPH?() -> U32:
  30

def rules.FORK_TURN?() -> F32:
  0.6000000238418579

def rules.FORK_TTL?() -> U32:
  30

def hero.HIT_SHARE?() -> U32:
  200

def hero.BLOW_MOST?() -> U32:
  30

def hero.ONE_HIT_MOST?() -> U32:
  90

def hero.HIT_WARD?() -> U32:
  45

def skills.SKILLS?() -> U32:
  5

def skills.GAS?() -> U32:
  0

def skills.FIRE?() -> U32:
  1

def skills.MINE?() -> U32:
  2

def skills.WALL?() -> U32:
  3

def skills.SHIELD?() -> U32:
  4

def skills.THROW?() -> F32:
  6.0

def skills.SIDE_RISE?() -> F32:
  1.7000000476837158

def skills.GAS_LIFE?() -> U32:
  210

def skills.GAS_RADIUS?() -> F32:
  3.0

def skills.GAS_SWELL?() -> U32:
  15

def skills.GAS_DRIFT?() -> F32:
  1.100000023841858

def skills.POISON_BASE?() -> U32:
  4

def skills.POISON_EVERY?() -> U32:
  15

def skills.POISON_LINGER?() -> U32:
  90

def skills.FART_LIFE?() -> U32:
  90

def skills.FART_REST?() -> U32:
  60

def skills.FART_CLOUDS?() -> U32:
  32

def skills.BOSS_SHRUG?() -> U32:
  50

def skills.FIRE_BURST?() -> U32:
  20

def skills.FIRE_LIFE?() -> U32:
  150

def skills.FIRE_RADIUS?() -> F32:
  2.200000047683716

def skills.BURN_BASE?() -> U32:
  3

def skills.BURN_EVERY?() -> U32:
  10

def skills.BURN_LINGER?() -> U32:
  60

def skills.FLASH_DAMAGE?() -> U32:
  30

def skills.MINE_MAX?() -> U32:
  3

def skills.MINE_ARM?() -> U32:
  45

def skills.MINE_TRIGGER?() -> F32:
  1.2999999523162842

def skills.MINE_BLAST?() -> F32:
  2.799999952316284

def skills.MINE_DAMAGE?() -> U32:
  70

def skills.MINE_LIFE?() -> U32:
  2700

def skills.MINE_CHAIN?() -> U32:
  4

def skills.WALL_SPAN?() -> F32:
  4.0

def skills.WALL_HEIGHT?() -> F32:
  2.799999952316284

def skills.WALL_REACH?() -> F32:
  2.4000000953674316

def skills.WALL_HP?() -> U32:
  150

def skills.WALL_LIFE?() -> U32:
  360

def skills.WALL_MAX?() -> U32:
  2

def skills.WALL_CRUMBLE?() -> U32:
  16

def skills.WALL_THICK?() -> F32:
  0.5

def skills.GNAW?() -> U32:
  8

def skills.GNAW_EVERY?() -> W.W:
  W.W{0, 10}

def skills.SHIELD_SPEED?() -> F32:
  22.0

def skills.SHIELD_RANGE?() -> F32:
  13.0

def skills.SHIELD_DAMAGE?() -> U32:
  28

def skills.SHIELD_STUN?() -> U32:
  6

def skills.SHIELD_RICOCHETS?() -> U32:
  2

def skills.SHIELD_LINKS?() -> U32:
  2

def skills.SHIELD_JUMP?() -> F32:
  7.0

def skills.SHIELD_LIFE?() -> U32:
  120

def skills.SHIELD_CATCH?() -> F32:
  1.100000023841858

def skills.SHIELD_REACH?() -> F32:
  0.8999999761581421

def skills.DUCK_DROP?() -> F32:
  0.75

def phrasebook.LEARNED?() -> String:
  "## learned in this workspace"

def phrasebook.MAX_ID?() -> U32:
  24

def phrasebook.MAX_TIERS?() -> U32:
  5

def feel.JOLT_HIT?() -> U32:
  2

def feel.FLINCH?() -> U32:
  15

def feel.DEATH_HOLD?() -> U32:
  4

def feel.DEATH_TICKS?() -> U32:
  14

def feel.SHAKE_SHOT?() -> U32:
  2

def feel.SHAKE_KILL?() -> U32:
  3

def feel.RECOIL?() -> F32:
  0.6000000238418579

def feel.COIN_GOLD?() -> U32:
  1

def feel.COINS?() -> U32:
  24

def feel.COIN_SPEED?() -> F32:
  22.0

def feel.COIN_CATCH?() -> F32:
  0.8999999761581421

def feel.VALOUR_PIP?() -> U32:
  600

def feel.VALOUR_HIT?() -> U32:
  120

def feel.GRAZE_VALOUR?() -> U32:
  120

def feel.PIPS?() -> U32:
  3

def feel.GRAZE?() -> F32:
  0.5

def feel.TEMPER_MAX?() -> U32:
  3

def feel.FIRE_ARROWS?() -> U32:
  120

def feel.IMPACTS?() -> U32:
  48

def feel.STUCK?() -> U32:
  24

def feel.PUFFS?() -> U32:
  32

def feel.DEATHS?() -> U32:
  24

def feel.TRACERS?() -> U32:
  16

def feel.BLOCKS?() -> U32:
  16

def feel.GLINTS?() -> U32:
  12

def feel.TAKES?() -> U32:
  8

def feel.BOLTS?() -> U32:
  16

def feel.IMPACT_TICKS?() -> U32:
  6

def feel.TRACER_TICKS?() -> U32:
  8

def feel.FLASH_TICKS?() -> U32:
  3

def feel.SMOKE_TICKS?() -> U32:
  14

def feel.BLOCK_TICKS?() -> U32:
  10

def feel.DUST_TICKS?() -> U32:
  12

def feel.GLINT_TICKS?() -> U32:
  16

def feel.TAKE_TICKS?() -> U32:
  45

def feel.BOLT_TICKS?() -> U32:
  18

def feel.KICK?() -> U32:
  1

def feel.TRAIL?() -> U32:
  2

def feel.LAND?() -> U32:
  3

def looks.MAX_FRAMES?() -> U32:
  4

def looks.TICKS_PER_FRAME?() -> W.W:
  W.W{0, 6}

def looks.CARD_LOOK?() -> String:
  "card"

def ults.ULT_FULL?() -> U32:
  1000

def ults.PER_TICK?() -> U32:
  1

def ults.STRIKE_FALL?() -> U32:
  36

def ults.STRIKE_REACH?() -> F32:
  3.200000047683716

def ults.PHANTOM_TICKS?() -> U32:
  240

def ults.PHANTOM_SHARE?() -> U32:
  40

def shoulders.MAX_RIDERS?() -> U32:
  5

def shoulders.RIDE_REACH?() -> F32:
  2.799999952316284

def shoulders.PACE_PER_RIDER?() -> F32:
  0.05999999865889549

def shoulders.RIDER_SHARE?() -> U32:
  33

def shoulders.THROW_REACH?() -> F32:
  24.0

def shoulders.THROW_TICKS?() -> U32:
  22

def shoulders.THROW_LEAST?() -> U32:
  7

def shoulders.THROW_PEAK?() -> F32:
  6.0

def shoulders.THROW_REARM?() -> U32:
  90

def shoulders.LAND_WARD?() -> U32:
  15

def shoulders.SLAM_REACH?() -> F32:
  4.5

def shoulders.SLAM_DAMAGE?() -> U32:
  45

def shoulders.SLAM_KNOCK?() -> F32:
  4.0

def shoulders.SLAM_STUN?() -> U32:
  12

def shoulders.SLAM_SHOWN?() -> U32:
  14

def shoulders.SOUND_CLIMB?() -> String:
  "shoulder_climb"

def shoulders.SOUND_HOP?() -> String:
  "shoulder_hop"

def shoulders.SOUND_THROW?() -> String:
  "shoulder_throw"

def shoulders.SOUND_SLAM?() -> String:
  "shoulder_slam"

def stakes.PASSIVE_PER_SECOND?() -> U32:
  1

def stakes.PASSIVE_CALM_PER_SECOND?() -> U32:
  2

def stakes.LEECH_PER_SECOND?() -> U32:
  2

def stakes.DRAUGHT_REST?() -> U32:
  240

def stakes.DRAUGHT_AGAIN?() -> U32:
  30

def stakes.WARD_REST?() -> U32:
  450

def stakes.HP_OVER_KIT?() -> U32:
  50

def stakes.POWER_HEALTH?() -> F32:
  25.0

def stakes.POWER_HEALTH_MAX?() -> F32:
  120.0

def stakes.RAMP_FIRST?() -> U32:
  40

def stakes.POWER_HIT?() -> F32:
  4.0

def stakes.POWER_HIT_MAX?() -> F32:
  25.0

def stakes.EXTRA_PER_KNIGHT?() -> U32:
  2

def stakes.GENTLE_FIRST_TO?() -> U32:
  2

def stakes.BOSS_HIT?() -> U32:
  115

def stakes.FAN_SHOTS?() -> U32:
  12

def stakes.FAN_ANGLE?() -> F32:
  0.20000000298023224

def fortune.SPIN_TICKS?() -> U32:
  120

def mirror.BLEND_TICKS?() -> W.W:
  W.W{0, 2}

def mirror.JUMP?() -> F32:
  4.0

def gore.HOLD?() -> W.W:
  W.W{0, 3}

def gore.LIFE?() -> W.W:
  W.W{0, 240}

def gore.BODY?() -> W.W:
  W.W{0, 120}

def gore.MAX_LIVE?() -> U32:
  14

def gore.MAX_NEW?() -> U32:
  5

def gore.POINT_BLANK?() -> F32:
  4.0

def gore.KICK_REACH?() -> F32:
  5.0

def gore.SKID?() -> F32:
  2.5999999046325684

def tune.HIT_WARD_MOST?() -> F32:
  2.5

def tune.LONGEST_STEP?() -> F32:
  1.0

def identity.BALL?() -> String:
  "the-ball"

def identity.MADE_PARTS?() -> U32:
  1

def identity.ROOMS?() -> U32:
  3

def identity.MOST_WORN?() -> U32:
  7

def identity.SHAPE_HP?() -> U32:
  100

def floor_stats.SETTLE?() -> W.W:
  W.W{0, 150}

def floor_stats.AIMED?() -> U32:
  20

def floor_stats.TRUE_AIM?() -> U32:
  50

def floor_stats.WISHES?() -> U32:
  32

def floor_stats.SHOWN?() -> U32:
  200

def floor_stats.VERDICT_FIRST?() -> U32:
  45

def floor_stats.DEEDS_HOLD?() -> U32:
  240

def tray.SIZE?() -> U32:
  12

def swamp.FLOORS?() -> U32:
  3

def swamp.BOG_PACE?() -> F32:
  0.550000011920929

def swamp.FOG_REACH?() -> F32:
  7.0

def swamp.FOG_DOOR_CLEAR?() -> F32:
  8.0

def swamp.NOTICE_REACH?() -> F32:
  2.0

def swamp.HORSE?() -> String:
  "white-horse"

def swamp.RESCUE_MARK?() -> String:
  "rescue:white-horse"

def swamp.NOTICE_MARK?() -> String:
  "country:swamp:notice"

def swamp.MOMENT?() -> String:
  "white_horse"

def swamp.KING?() -> String:
  "the-mire-king"

def swamp.HORSE_NAME?() -> String:
  "Blanchefleur"

def swamp.HORSE_HOUSE?() -> String:
  "keep"

def swamp.PEN_HELD?() -> String:
  "THE MIRE KING HOLDS THE PEN."

def artistry.SIZE?() -> U32:
  15

def artistry.SHADING?() -> U32:
  15

def artistry.DETAIL?() -> U32:
  15

def artistry.SILHOUETTE?() -> U32:
  15

def artistry.OUTLINE?() -> U32:
  15

def artistry.FILL?() -> U32:
  5

def artistry.FINE?() -> U32:
  58

def artistry.MASTERWORK?() -> U32:
  78

def artistry.LEGENDARY?() -> U32:
  92

def command.NO_AXIS?() -> U32:
  3

def command.FLAGS?() -> U32:
  12

def command.HAS_SLOTS?() -> W.W:
  W.W{0, 4096}

def command.HAS_VOTE?() -> W.W:
  W.W{0, 8192}

def command.HAS_LATCH_TICK?() -> W.W:
  W.W{0, 16384}

def command.LEAVE_HELD?() -> W.W:
  W.W{0, 32768}

def command.RIDE?() -> W.W:
  W.W{0, 65536}

def command.CYCLE?() -> W.W:
  W.W{0, 131072}

def command.SLOT_BITS?() -> U32:
  3

def command.MAX_BYTES?() -> U32:
  54

def classes.ARCANE?() -> String:
  "arcane"

def classes.HOLY?() -> String:
  "holy"

def classes.REACH?() -> F32:
  1.600000023841858

def classes.FIELD_PRICE?() -> U32:
  20

def gear.BARE_BODY?() -> U32:
  20

def gear.BARE_HEAD?() -> U32:
  10

def gear.HELM_OFF?() -> U32:
  40

def gear.SMALLCLOTHES?() -> U32:
  15

def gear.POP_TICKS?() -> U32:
  24

def skulls.SKULL_REACH?() -> F32:
  1.600000023841858

def skulls.RAISE_REACH?() -> F32:
  2.4000000953674316

def skulls.SKULL_SETTLE?() -> W.W:
  W.W{0, 30}

def skulls.RAISED_SHARE?() -> U32:
  50

def skulls.RAISED_WARD?() -> U32:
  60

def mend.POS_LIMIT?() -> F32:
  8192.0

def mend.HP_ABS?() -> U32:
  100000

def mend.FOE_HP_ABS?() -> U32:
  1000000

def mend.BOON_ABS?() -> U32:
  1000

def wishcraft.SIZE_REACH?() -> F32:
  0.30000001192092896

def wishcraft.BASE_PCT?() -> U32:
  55

def wishcraft.BEAT_REF?() -> U32:
  12

def wishcraft.RATE_MIN_PCT?() -> U32:
  50

def wishcraft.RATE_MAX_PCT?() -> U32:
  150

def wishcraft.FLOOR_PCT?() -> U32:
  30

def wishcraft.SIZE_STEP_PCT?() -> U32:
  50

def wishcraft.HIT_REACH?() -> F32:
  1.2000000476837158

def wishcraft.KNOCK_STEP?() -> F32:
  0.6000000238418579

def wishcraft.BOOMERANG_HOME?() -> U32:
  60

def spells.CABBAGE_FALL?() -> U32:
  26

def spells.CABBAGE_STEP?() -> U32:
  7

def spells.CABBAGE_DAMAGE?() -> U32:
  34

def spells.CABBAGE_SPREAD?() -> F32:
  3.200000047683716

def spells.IMPOSTURE_TICKS?() -> U32:
  150

def spells.IMPOSTURE_MAX?() -> U32:
  4

def spells.HEDGE_HALF?() -> F32:
  3.0

def spells.HEDGE_HP?() -> U32:
  90

def spells.HEDGE_ROOM?() -> U32:
  21

def desperate.FROM?() -> F32:
  0.3499999940395355

def desperate.HIT_FLASH?() -> U32:
  6

def desperate.CYCLE?() -> U32:
  165

def desperate.RING_WARN?() -> U32:
  36

def desperate.ARMS_WARN?() -> U32:
  66

def desperate.ARMS_FROM?() -> U32:
  90

def desperate.ARMS_TO?() -> U32:
  132

def desperate.ARMS_AT?() -> F32:
  0.5

def desperate.RING_SHOTS?() -> U32:
  18

def desperate.RING_LANE?() -> U32:
  3

def trophies.GILDED?() -> U32:
  10

def hexer.HEX_TICKS?() -> U32:
  75

def hexer.HEX_IMMUNE?() -> U32:
  90

def hexer.FROG_PACE?() -> F32:
  0.699999988079071

def hexer.HEX_EVERY?() -> U32:
  120

def hexer.SPIT_EVERY?() -> U32:
  60

def hexer.RIBBIT?() -> U32:
  3

def fx.MAX_SPARKS?() -> U32:
  32

def lich.CHILL?() -> U32:
  45

def lich.CHILL_PACE?() -> F32:
  0.6000000238418579

def lich.BOUNCES?() -> U32:
  5

def lich.BOUNCE_REACH?() -> F32:
  10.0

def lich.BOLT_EVERY?() -> U32:
  75

def lich.CHAIN_EVERY?() -> U32:
  180

def campaign.SCHEMA?() -> String:
  "vec.board/1"

def campaign.PREFIX?() -> String:
  "vec_"

def campaign.MAX_BYTES?() -> U32:
  262144

def campaign.MAX_COUNT?() -> W.W:
  W.W{0, 1000000}

def script.INKS?() -> String:
  "kKZXgjGJhiHfDFEelNmLAMyCYnbIBpPrRoOtTsxSuUvVWqQz0123wa4@56$c987"

def aoe.FULL?() -> U32:
  3

def aoe.REST?() -> U32:
  25

def again.AGAIN_WAIT?() -> U32:
  150

def again.VERDICT_HOLD?() -> U32:
  300

def items.CENSER_REACH?() -> F32:
  8.0

def peasant.PACE?() -> F32:
  1.149999976158142

def peasant.SPRINT_PACE?() -> F32:
  1.7999999523162842

def peasant.SPRINT_TICKS?() -> U32:
  60

def peasant.DECOY_TICKS?() -> U32:
  90

def peasant.COLLAPSE_TICKS?() -> U32:
  9

def peasant.DECOY_RADIUS?() -> F32:
  0.6000000238418579

def peasant.STONE_KNOCK?() -> F32:
  0.2800000011920929

def peasant.STONE_HEIGHT?() -> F32:
  1.5

def spin.TICKS?() -> U32:
  14

def spin.REACH?() -> F32:
  3.0

def spin.DAMAGE?() -> U32:
  26

def spin.REARM?() -> U32:
  75

def spin.PACE?() -> F32:
  0.800000011920929

def bard.SONG_TICKS?() -> U32:
  60

def bard.SLEEP_TICKS?() -> U32:
  60

def bard.BOSS_SLEEP_TICKS?() -> U32:
  15

def bard.SONG_REACH?() -> F32:
  7.0

def risen.FOLLOW?() -> U32:
  240

def risen.RISE_AGAIN?() -> U32:
  1350

def loot.Rng.new?(+seed: W.W) -> W.W:
  W.or(W.xor(seed, W.W{2654435769, 2135587861}), W.W{0, 1})

def loot.Rng.next?(+r: W.W) -> W.W & W.W:
  +x1 = W.xor(r, W.shr(r, 12))
  +x2 = W.xor(x1, W.shl(x1, 25))
  +x3 = W.xor(x2, W.shr(x2, 27))
  (x3, W.mul(x3, W.W{625341585, 1332534557}))

def loot.Rng.below.go?(rv: W.W & W.W, +n: U32) -> W.W & U32:
  (+r2, +o) = rv
  (r2, W.rem_u32(o, loot.max1(n)))

def loot.max1?(+n: U32) -> U32:
  loot.max1.if(U32.is_eq(n, 0), n)

def loot.max1.if?(zero: Bool, +n: U32) -> U32:
  match zero:
    case True{}:
      1
    case False{}:
      n

def loot.Rng.below?(+r: W.W, +n: U32) -> W.W & U32:
  loot.Rng.below.go(loot.Rng.next(r), n)

def loot.Rng.chance.go?(rv: W.W & U32, +percent: U32) -> W.W & Bool:
  (+r2, +v) = rv
  (r2, U32.is_lt(v, percent))

def loot.Rng.chance?(+r: W.W, +percent: U32) -> W.W & Bool:
  loot.Rng.chance.go(loot.Rng.below(r, 100), percent)

def loot.sum?(+ws: List<&2, U32>) -> U32:
  match ws:
    case Nil{}:
      0
    case Con{+h, +t}:
      (h + loot.sum(t) : U32)

# The index the roll lands on, walking the weights as Rust does: a roll under
# the weight is the entry, otherwise the weight comes off the roll.

def loot.Rng.pick.walk?(+ws: List<&2, U32>, +roll: U32, +i: U32) -> U32:
  match ws:
    case Nil{}:
      0
    case Con{+w, +t}:
      loot.Rng.pick.walk.if(U32.is_lt(roll, w), t, roll, w, i)

def loot.Rng.pick.walk.if?(hit: Bool, +t: List<&2, U32>, +roll: U32, +w: U32, +i: U32) -> U32:
  match hit:
    case True{}:
      i
    case False{}:
      loot.Rng.pick.walk(t, (roll - w : U32), (i + 1 : U32))

def loot.Rng.pick.go?(rv: W.W & U32, +ws: List<&2, U32>) -> W.W & U32:
  (+r2, +roll) = rv
  (r2, loot.Rng.pick.walk(ws, roll, 0))

def loot.Rng.pick?(+r: W.W, +ws: List<&2, U32>) -> W.W & U32:
  loot.Rng.pick.go(loot.Rng.below(r, loot.sum(ws)), ws)

# Cases (recorded from the Rust original: tests/rules/loot_bend_cases__tests.rs)

def case.loot.Rng.new?(+input: J.Val) -> J.Val:
  W.to_j(loot.Rng.new(W.of_j(J.get(input, "seed"))))

def cb.case.loot.Rng.next.go?(rv: W.W & W.W) -> J.Val:
  (+r, +o) = rv
  J.set(J.set(J.obj(), "rng", W.to_j(r)), "out", W.to_j(o))

def case.loot.Rng.next?(+input: J.Val) -> J.Val:
  cb.case.loot.Rng.next.go(loot.Rng.next(W.of_j(J.get(input, "rng"))))

def cb.case.loot.Rng.below.go?(rv: W.W & U32) -> J.Val:
  (+r, +v) = rv
  J.set(J.set(J.obj(), "rng", W.to_j(r)), "v", J.of_u32(v))

def case.loot.Rng.below?(+input: J.Val) -> J.Val:
  cb.case.loot.Rng.below.go(loot.Rng.below(W.of_j(J.get(input, "rng")), J.u32(J.get(input, "n"))))

def cb.case.loot.Rng.chance.go?(rv: W.W & Bool) -> J.Val:
  (+r, +b) = rv
  J.set(J.set(J.obj(), "rng", W.to_j(r)), "hit", J.of_bool(b))

def case.loot.Rng.chance?(+input: J.Val) -> J.Val:
  cb.case.loot.Rng.chance.go(loot.Rng.chance(W.of_j(J.get(input, "rng")), J.u32(J.get(input, "percent"))))

def cb.case.loot.Rng.pick.go?(rv: W.W & U32) -> J.Val:
  (+r, +i) = rv
  J.set(J.set(J.obj(), "rng", W.to_j(r)), "index", J.of_u32(i))

def cb.case.loot.Rng.pick.weights?(+xs: List<&2, J.Val>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.u32(h) <> cb.case.loot.Rng.pick.weights(t)

def case.loot.Rng.pick?(+input: J.Val) -> J.Val:
  cb.case.loot.Rng.pick.go(loot.Rng.pick(W.of_j(J.get(input, "rng")), cb.case.loot.Rng.pick.weights(J.items(J.get(input, "weights")))))

def ga.v?(+o: J.Val, +k: String) -> J.Val:
  J.get(o, k)

def ga.u?(+o: J.Val, +k: String) -> U32:
  J.u32(J.get(o, k))

def ga.f?(+o: J.Val, +k: String) -> F32:
  J.f32(J.get(o, k))

def ga.b?(+o: J.Val, +k: String) -> Bool:
  J.bool(J.get(o, k))

def ga.s?(+o: J.Val, +k: String) -> String:
  J.str(J.get(o, k))

# Item `i` of array `a`.

def ga.nu?(+a: J.Val, +i: U32) -> U32:
  J.u32(J.nth(a, i))

def ga.nf?(+a: J.Val, +i: U32) -> F32:
  J.f32(J.nth(a, i))

# Writers: field `k` of `o` set to a u8/u16/u32, an i32, an f32, a bool,
# a string, any J value.

def ga.su?(o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.JU{v})

def ga.si?(o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.of_i32(v))

def ga.sf?(o: J.Val, +k: String, +v: F32) -> J.Val:
  J.set(o, k, J.JF{v})

def ga.sb?(o: J.Val, +k: String, +v: Bool) -> J.Val:
  J.set(o, k, J.JBool{v})

def ga.ss?(o: J.Val, +k: String, +v: String) -> J.Val:
  J.set(o, k, J.JS{v})

def ga.sv?(o: J.Val, +k: String, v: J.Val) -> J.Val:
  J.set(o, k, v)

# `#[serde(skip_serializing_if = ...)]`: a zero / false / null value is left
# out of the object.

def ga.su_skip.if?(zero: Bool, o: J.Val, +k: String, +v: U32) -> J.Val:
  match zero:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, J.JU{v})

def ga.su_skip?(o: J.Val, +k: String, +v: U32) -> J.Val:
  ga.su_skip.if(U32.is_zero(v), o, k, v)

def ga.sb_skip.if?(off: Bool, o: J.Val, +k: String) -> J.Val:
  match off:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, J.JBool{True{}})

def ga.sb_skip?(o: J.Val, +k: String, +v: Bool) -> J.Val:
  ga.sb_skip.if(Bool.not(v), o, k)

def ga.sv_skip.if?(none: Bool, o: J.Val, +k: String, v: J.Val) -> J.Val:
  match none:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, v)

def ga.sv_skip?(o: J.Val, +k: String, +v: J.Val) -> J.Val:
  ga.sv_skip.if(J.is_null(v), o, k, v)

# Pickers (both arms are evaluated: keep them cheap and total).

def ga.pu?(+c: Bool, +a: U32, +b: U32) -> U32:
  Bool.pick(U32, c, a, b)

def ga.pf?(+c: Bool, +a: F32, +b: F32) -> F32:
  Bool.pick(F32, c, a, b)

def ga.pb?(+c: Bool, +a: Bool, +b: Bool) -> Bool:
  Bool.pick(Bool, c, a, b)

def ga.pv?(+c: Bool, a: J.Val, b: J.Val) -> J.Val:
  Bool.pick(J.Val, c, a, b)

# A decimal object key, and entry `id` of an object keyed by decimal id
# (a BTreeMap<u32, _>).

def ga.key?(+n: U32) -> String:
  U32.show(n)

def ga.at?(+m: J.Val, +id: U32) -> J.Val:
  J.get(m, U32.show(id))

def ga.put_at?(m: J.Val, +id: U32, v: J.Val) -> J.Val:
  J.set(m, U32.show(id), v)

# Whether an object has the decimal key.

def ga.has_key?(+m: J.Val, +id: U32) -> Bool:
  J.has(m, U32.show(id))

# The ids of an object keyed by decimal id, ascending (BTreeMap order).

def ga.ids.go?(xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{(+k, +v), t}:
      Con{Maybe.default(&2, U32, U32.read(k), 0), ga.ids.go(t)}

def ga.ids?(+m: J.Val) -> List<&2, U32>:
  ga.ids.go(J.int_fields(m))

# A two-item array / three-item array.

def ga.pair?(a: J.Val, b: J.Val) -> J.Val:
  J.JA{[a, b]}

def ga.pair_f?(+x: F32, +y: F32) -> J.Val:
  J.JA{[J.JF{x}, J.JF{y}]}

def ga.pair_i?(+x: U32, +y: U32) -> J.Val:
  J.JA{[J.of_i32(x), J.of_i32(y)]}

def ga.trio?(a: J.Val, b: J.Val, c: J.Val) -> J.Val:
  J.JA{[a, b, c]}

# a.saturating_sub(b) on unsigned.

def ga.ssub?(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_ge(a, b), (a - b : U32), 0)

# a.saturating_add(b) on u8 / u16 / u32.

def ga.sadd8?(+a: U32, +b: U32) -> U32:
  U32.min((a + b : U32), 255)

def ga.sadd16?(+a: U32, +b: U32) -> U32:
  U32.min((a + b : U32), 65535)

def ga.sadd32?(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_lt((a + b : U32), a), 4294967295, (a + b : U32))

# Array helpers over the J array of an object field.

def ga.items?(+o: J.Val, +k: String) -> List<&2, J.Val>:
  J.items(J.get(o, k))

def ga.push?(+o: J.Val, +k: String, v: J.Val) -> J.Val:
  J.set(o, k, J.push(J.get(o, k), v))

# `Vec::remove(i)` / `Vec::retain` by position: drop item `i` of a list.

def ga.remove_nth?(+xs: List<&2, J.Val>, +n: Nat) -> List<&2, J.Val>:
  match xs n:
    case Nil{} _:
      Nil{}
    case Con{h, t} 0n:
      t
    case Con{h, t} 1n+p:
      Con{h, ga.remove_nth(t, p)}

# Number of items of a list as a U32.

def ga.count?(+xs: List<&2, J.Val>) -> U32:
  U32.from_nat(List.length(&2, J.Val, xs))

# `s` is the J string `name` (a unit enum variant in serde shape).

def ga.is?(+s: J.Val, +name: String) -> Bool:
  String.eq(J.str(s), name)

# A tile name (`Tile`, serde snake_case) is one of the stones that stop a body.

def ga.stone?(+t: J.Val) -> Bool:
  Bool.or(ga.is(t, "wall"), ga.is(t, "block"))

# `a.max(b)` / `a.min(b)` on u32 and the Option's `unwrap_or`.

def ga.umax?(+a: U32, +b: U32) -> U32:
  U32.max(a, b)

def ga.umin?(+a: U32, +b: U32) -> U32:
  U32.min(a, b)

# The object with only the keys `ks` of `o` (those it has), for comparing the
# part of a run a function changes.

def ga.pick?(+o: J.Val, +ks: List<&2, String>) -> J.Val:
  ga.pick.go(o, ks, J.obj())

def ga.pick.go?(+o: J.Val, +ks: List<&2, String>, +acc: J.Val) -> J.Val:
  match ks:
    case Nil{}:
      acc
    case Con{+k, +t}:
      ga.pick.go(o, t, ga.pick.put(J.has(o, k), o, k, acc))

def ga.pick.put?(has: Bool, +o: J.Val, +k: String, +acc: J.Val) -> J.Val:
  match has:
    case True{}:
      J.set(acc, k, J.get(o, k))
    case False{}:
      acc

# {room, barred}: a `Grid`.

def ga.grid?(+room: J.Val, +barred: Bool) -> J.Val:
  ga.sv(ga.sb(J.obj(), "barred", barred), "room", room)

# A J array of [x, y] pairs as a list of Bend pairs.

def ga.xys?(+a: J.Val) -> List<&2, Sigma<&2, &2, F32, _ => F32>>:
  ga.xys.go(J.items(a))

def ga.xys.go?(+xs: List<&2, J.Val>) -> List<&2, Sigma<&2, &2, F32, _ => F32>>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{(J.f32(J.nth(h, 0)), J.f32(J.nth(h, 1))), ga.xys.go(t)}

def ga.ps?(+c: Bool, +a: String, +b: String) -> String:
  Bool.pick(String, c, a, b)

# The number of fields of an object.

def ga.nfields?(+o: J.Val) -> U32:
  U32.from_nat(List.length(&2, Sigma<&2, &2, String, _ => J.Val>, J.fields(o)))

def aerie.LAP?() -> U32:
  240

def aerie.BONK?() -> F32:
  1.3

def aerie.POOP_EVERY?() -> U32:
  90

def aerie.POOP_REACH?() -> F32:
  1.2

def aerie.SWING_REACH?() -> F32:
  2.2

def aerie.TAU?() -> F32:
  6.2831855

# CLERK = (5.0, 10.0).

def aerie.CLERK_X?() -> F32:
  5.0

def aerie.CLERK_Y?() -> F32:
  10.0

# The paper whose room is `kind`: 0, 1, 2, else 3.

def aerie.doc_index?(+kind: String) -> U32:
  ga.pu(String.eq(kind, "belfry"), 0, ga.pu(String.eq(kind, "aqueduct"), 1, ga.pu(String.eq(kind, "aviary"), 2, 3)))

def aerie.doc_x?(+i: U32) -> F32:
  ga.pf(U32.is_eq(i, 0), 20.0, ga.pf(U32.is_eq(i, 1), 5.0, 19.0))

def aerie.doc_y?(+i: U32) -> F32:
  ga.pf(U32.is_eq(i, 0), 10.0, ga.pf(U32.is_eq(i, 1), 8.5, 6.5))

def aerie.doc_name?(+i: U32) -> String:
  ga.ps(U32.is_eq(i, 0), "BELL TAX", ga.ps(U32.is_eq(i, 1), "WATER TAX", "SEED TAX"))

def aerie.doc_room?(+i: U32) -> String:
  ga.ps(U32.is_eq(i, 0), "belfry", ga.ps(U32.is_eq(i, 1), "aqueduct", "aviary"))

def aerie.quest_mark?(+i: U32) -> String:
  ga.ps(U32.is_eq(i, 0), "country:minutes:accept", ga.ps(U32.is_eq(i, 1), "country:minutes:0",
    ga.ps(U32.is_eq(i, 2), "country:minutes:1", ga.ps(U32.is_eq(i, 3), "country:minutes:2", "country:minutes:deliver"))))

def aerie.visit_mark?(+i: U32) -> String:
  ga.ps(U32.is_eq(i, 0), "country:aerie:0", ga.ps(U32.is_eq(i, 1), "country:aerie:1", "country:aerie:2"))

# The pigeons' state when there is none: `Pigeons::default()`.

def aerie.Pigeons.default?() -> J.Val:
  ga.sv(ga.sv(ga.sv(J.obj(), "dropped", J.JNull{}), "bonks", J.obj()), "poop", J.obj())

def aerie.Pigeons.valid?(+p: J.Val) -> Bool:
  Bool.and(Bool.and(U32.is_le(ga.nfields(J.get(p, "bonks")), rules.MAX_PLAYERS()), U32.is_le(ga.nfields(J.get(p, "poop")), rules.MAX_PLAYERS())),
    aerie.Pigeons.valid.dropped(J.get(p, "dropped")))

def aerie.Pigeons.valid.dropped?(+d: J.Val) -> Bool:
  aerie.Pigeons.valid.dropped.in(J.is_null(d), d)

def aerie.Pigeons.valid.dropped.in?(+none: Bool, +d: J.Val) -> Bool:
  match none:
    case True{}:
      True{}
    case False{}:
      Bool.and(U32.is_lt(ga.nu(d, 0), 3), Bool.and(
        Bool.and(F32.is_ge(ga.nf(d, 1), 0.0), F32.is_lt(ga.nf(d, 1), 200.0)),
        Bool.and(F32.is_ge(ga.nf(d, 2), 0.0), F32.is_lt(ga.nf(d, 2), 200.0))))

# Where paper `i`'s pigeon flies at `tick`, in units: a figure of eight over the
# middle of its room.

def aerie.pigeon_at?(+room: J.Val, +i: U32, +tick: W.W) -> F32 & F32:
  aerie.pigeon_at.t(room, F32.mul(F32.div(RS.u_as_f32(W.rem_u32(W.add(tick, W.div_u32(W.mul_u32(W.of_u32(i), 240), 3)), 240)), 240.0), aerie.TAU()))

def aerie.pigeon_at.t?(+room: J.Val, +t: F32) -> F32 & F32:
  (F32.mul(F32.add(F32.div(RS.u_as_f32(ga.u(room, "cols")), 2.0), F32.mul(F32.mul(F32.sin(t), RS.u_as_f32(ga.u(room, "cols"))), 0.32)), layout.TILE_UNITS()),
    F32.mul(F32.add(F32.div(RS.u_as_f32(ga.u(room, "rows")), 2.0), F32.mul(F32.mul(F32.sin(F32.mul(2.0, t)), RS.u_as_f32(ga.u(room, "rows"))), 0.22)), layout.TILE_UNITS()))

# The paper whose pigeon is up in this room, if one is: its index, or 3.

def aerie.flying?(+kind: String, +stage: U32, +papers: U32, +pigeons: J.Val) -> U32:
  aerie.flying.in(aerie.doc_index(kind), stage, papers, pigeons)

def aerie.flying.in?(+i: U32, +stage: U32, +papers: U32, +pigeons: J.Val) -> U32:
  aerie.flying.up(Bool.and(U32.is_lt(i, 3), Bool.and(U32.is_eq(stage, 1),
    Bool.and(U32.is_zero(U32.and(papers, RS.u_shl(1, i))), aerie.flying.free(pigeons, i)))), i)

def aerie.flying.free?(+pigeons: J.Val, +i: U32) -> Bool:
  aerie.flying.free.in(J.is_null(pigeons), J.get(pigeons, "dropped"), i)

def aerie.flying.free.in?(+none: Bool, +dropped: J.Val, +i: U32) -> Bool:
  match none:
    case True{}:
      True{}
    case False{}:
      Bool.or(J.is_null(dropped), U32.is_ne(ga.nu(dropped, 0), i))

def aerie.flying.up?(+yes: Bool, +i: U32) -> U32:
  ga.pu(yes, i, 3)

# The commission stage and found-paper mask, with pending private intents.

def aerie.progress?(+country: J.Val, +marks: J.Val) -> U32 & U32:
  aerie.progress.mask(
    ga.pu(Bool.and(U32.is_zero(ga.u(country, "post")), J.has(marks, aerie.quest_mark(0))), 1, ga.u(country, "post")),
    ga.u(country, "minutes"), marks)

def aerie.progress.mask?(+stage: U32, +papers: U32, +marks: J.Val) -> U32 & U32:
  aerie.progress.deliver(stage,
    ga.pu(U32.is_gt(stage, 0),
      U32.or(U32.or(U32.or(papers, ga.pu(J.has(marks, aerie.quest_mark(1)), 1, 0)), ga.pu(J.has(marks, aerie.quest_mark(2)), 2, 0)), ga.pu(J.has(marks, aerie.quest_mark(3)), 4, 0)),
      papers), marks)

def aerie.progress.deliver?(+stage: U32, +papers: U32, +marks: J.Val) -> U32 & U32:
  (ga.pu(Bool.and(U32.is_gt(stage, 0), Bool.and(U32.is_eq(papers, 7), J.has(marks, aerie.quest_mark(4)))), 2, stage), papers)

# --- shape: the sculptures and the stairs ------------------------------------------

# The block rectangles (col, row, w, h) of a kind, as a flat list.

def aerie.blocks?(+kind: String) -> List<&2, U32>:
  aerie.blocks.in(String.eq(kind, "belfry"), String.eq(kind, "aqueduct"), String.eq(kind, "aviary"))

def aerie.blocks.in?(+belfry: Bool, +aqueduct: Bool, +aviary: Bool) -> List<&2, U32>:
  match belfry:
    case True{}:
      [3, 2, 2, 3, 19, 2, 2, 3]
    case False{}:
      aerie.blocks.aq(aqueduct, aviary)

def aerie.blocks.aq?(+aqueduct: Bool, +aviary: Bool) -> List<&2, U32>:
  match aqueduct:
    case True{}:
      [7, 2, 2, 3, 15, 2, 2, 3, 7, 10, 2, 2, 15, 10, 2, 2]
    case False{}:
      aerie.blocks.av(aviary)

def aerie.blocks.av?(+aviary: Bool) -> List<&2, U32>:
  match aviary:
    case True{}:
      [3, 2, 5, 2]
    case False{}:
      Nil{}

# Fill the rectangle with `tile`.

def aerie.fill?(+room: J.Val, +c: U32, +r: U32, +w: U32, +h: U32, +tile: String) -> J.Val:
  aerie.fill.row(room, c, r, w, r, (r + h : U32), tile)

def aerie.fill.row?(+room: J.Val, +c: U32, +r: U32, +w: U32, +y: U32, +end: U32, +tile: String) -> J.Val:
  aerie.fill.row.m(U32.is_ge(y, end), room, c, r, w, y, end, tile)

def aerie.fill.row.m?(+flag: Bool, +room: J.Val, +c: U32, +r: U32, +w: U32, +y: U32, +end: U32, +tile: String) -> J.Val:
  match flag:
    case True{}:
      room
    case False{}:
      aerie.fill.row(aerie.fill.col(room, c, w, y, tile, c, (c + w : U32)), c, r, w, (y + 1 : U32), end, tile)

def aerie.fill.col?(+room: J.Val, +c: U32, +w: U32, +y: U32, +tile: String, +x: U32, +end: U32) -> J.Val:
  aerie.fill.col.m(U32.is_ge(x, end), room, c, w, y, tile, x, end)

def aerie.fill.col.m?(+flag: Bool, +room: J.Val, +c: U32, +w: U32, +y: U32, +tile: String, +x: U32, +end: U32) -> J.Val:
  match flag:
    case True{}:
      room
    case False{}:
      aerie.fill.col(layout.Room.set(room, x, y, J.JS{tile}), c, w, y, tile, (x + 1 : U32), end)

def aerie.shape.blocks?(+room: J.Val, +bs: List<&2, U32>) -> J.Val:
  match bs:
    case Nil{}:
      room
    case Con{+c, Con{+r, Con{+w, Con{+h, +t}}}}:
      aerie.shape.blocks(aerie.fill(room, c, r, w, h, "block"), t)
    case _:
      room

def aerie.shape?(+room: J.Val) -> J.Val:
  aerie.shape.stairs(aerie.shape.blocks(room, aerie.blocks(ga.s(room, "kind"))), ga.s(room, "kind"))

def aerie.shape.stairs?(+room: J.Val, +kind: String) -> J.Val:
  aerie.shape.stairs.in(String.eq(kind, "aviary"), String.eq(kind, "rookery"), room)

def aerie.shape.stairs.in?(+aviary: Bool, +rookery: Bool, +room: J.Val) -> J.Val:
  match aviary:
    case True{}:
      aerie.fill(room, 19, 10, 2, 1, "stairs")
    case False{}:
      aerie.shape.stairs.rook(rookery, room)

def aerie.shape.stairs.rook?(+rookery: Bool, +room: J.Val) -> J.Val:
  match rookery:
    case True{}:
      aerie.fill(room, 4, 10, 2, 1, "stairs")
    case False{}:
      room

# --- the Run -------------------------------------------------------------------------------

# Whether a living, awake knight is here and not in the privy.

def aerie.living?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case Con{(+k, +h), +t}:
      Bool.or(Bool.and(U32.is_gt(ga.u(h, "hp"), 0), Bool.and(Bool.not(ga.b(h, "stone")), U32.is_zero(ga.u(h, "privy")))), aerie.living(t))

def aerie.Run.tick_aerie?(+run: J.Val, +inputs: J.Val) -> J.Val:
  aerie.Run.tick_aerie.kind(ga.s(rules.Run.room(run), "kind"), run, inputs)

def aerie.Run.tick_aerie.kind?(+kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  aerie.Run.tick_aerie.place(aerie.doc_index(kind), kind, run, inputs)

def aerie.Run.tick_aerie.place?(+i: U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  aerie.Run.tick_aerie.place.m(U32.is_ge(i, 3), i, kind, run, inputs)

def aerie.Run.tick_aerie.place.m?(+flag: Bool, +i: U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  match flag:
    case True{}:
      run
    case False{}:
      aerie.Run.tick_aerie.awake(aerie.living(J.int_fields(J.get(run, "players"))), i, kind, run, inputs)

def aerie.Run.tick_aerie.awake?(+awake: Bool, +i: U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  match awake:
    case False{}:
      run
    case True{}:
      aerie.Run.tick_aerie.round(
        aerie.Run.tick_aerie.visit(U32.is_zero(U32.and(ga.u(J.get(J.get(run, "home"), "countryside"), "aerie_visited"), RS.u_shl(1, i))), i, run),
        i, kind, inputs)

# A first visit marks the place.

def aerie.Run.tick_aerie.visit?(+first: Bool, +i: U32, +run: J.Val) -> J.Val:
  match first:
    case True{}:
      J.set(run, "marks", J.set(J.get(run, "marks"), aerie.visit_mark(i), J.JU{1}))
    case False{}:
      run

# No quest actions while an arcade round is playing.

def aerie.Run.tick_aerie.round?(+run: J.Val, +i: U32, +kind: String, +inputs: J.Val) -> J.Val:
  aerie.Run.tick_aerie.round.m(J.is_null(J.get(J.get(run, "pastimes"), "round")), run, i, kind, inputs)

def aerie.Run.tick_aerie.round.m?(+flag: Bool, +run: J.Val, +i: U32, +kind: String, +inputs: J.Val) -> J.Val:
  match flag:
    case False{}:
      run
    case True{}:
      aerie.Run.tick_aerie.stage(aerie.progress(J.get(J.get(run, "home"), "countryside"), J.get(run, "marks")), kind, run, inputs)

def aerie.Run.tick_aerie.stage?(+sp: U32 & U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  (stage, papers) = sp
  aerie.Run.tick_aerie.stage.in(U32.is_eq(stage, 2), stage, papers, kind, run, inputs)

def aerie.Run.tick_aerie.stage.in?(+done: Bool, +stage: U32, +papers: U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  match done:
    case True{}:
      J.set(run, "pigeons", J.JNull{})
    case False{}:
      aerie.Run.tick_aerie.act(stage, papers, kind, aerie.Run.tick_aerie.pigeon(U32.is_eq(stage, 1), kind, papers, run), inputs)

def aerie.Run.tick_aerie.pigeon?(+on: Bool, +kind: String, +papers: U32, +run: J.Val) -> J.Val:
  match on:
    case True{}:
      aerie.Run.tick_pigeon(run, kind, papers)
    case False{}:
      run

# The first knight to press E at the clerk or on a dropped paper.

def aerie.Run.tick_aerie.act?(+stage: U32, +papers: U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  aerie.Run.tick_aerie.done(aerie.action(J.int_fields(J.get(run, "players")), stage, papers, kind, J.get(run, "pigeons"), inputs), run)

# (id, action) or null.

def aerie.action?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  match hs:
    case Nil{}:
      J.JNull{}
    case Con{(+k, +h), +t}:
      aerie.action.knight(Maybe.default(&2, U32, U32.read(k), 0), h, t, stage, papers, kind, pigeons, inputs)

def aerie.action.knight?(+id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  aerie.action.press(
    Bool.or(Bool.or(U32.is_zero(ga.u(h, "hp")), ga.b(h, "stone")), Bool.or(U32.is_ne(ga.u(h, "privy"), 0),
      Bool.not(Bool.and(rules.Input.valid(J.get(inputs, U32.show(id))), ga.b(J.get(inputs, U32.show(id)), "interact"))))),
    id, h, t, stage, papers, kind, pigeons, inputs)

def aerie.action.press?(+skip: Bool, +id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  match skip:
    case True{}:
      aerie.action(t, stage, papers, kind, pigeons, inputs)
    case False{}:
      aerie.action.clerk(
        Bool.and(String.eq(kind, "aviary"), Bool.and(
          F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), F32.mul(aerie.CLERK_X(), layout.TILE_UNITS())), F32.sub(ga.f(h, "y"), F32.mul(aerie.CLERK_Y(), layout.TILE_UNITS()))), 1.4),
          Bool.or(U32.is_zero(stage), U32.is_eq(papers, 7)))),
        id, h, t, stage, papers, kind, pigeons, inputs)

def aerie.action.clerk?(+at_clerk: Bool, +id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  match at_clerk:
    case True{}:
      ga.pair(J.JU{id}, J.JU{ga.pu(U32.is_zero(stage), 0, 4)})
    case False{}:
      aerie.action.paper(J.get(pigeons, "dropped"), id, h, t, stage, papers, kind, pigeons, inputs)

def aerie.action.paper?(+dropped: J.Val, +id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  aerie.action.paper.m(J.is_null(dropped), dropped, id, h, t, stage, papers, kind, pigeons, inputs)

def aerie.action.paper.m?(+flag: Bool, +dropped: J.Val, +id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  match flag:
    case True{}:
      aerie.action(t, stage, papers, kind, pigeons, inputs)
    case False{}:
      aerie.action.pick(
        Bool.and(String.eq(aerie.doc_room(ga.nu(dropped, 0)), kind),
          F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), ga.nf(dropped, 1)), F32.sub(ga.f(h, "y"), ga.nf(dropped, 2))), F32.mul(1.4, layout.TILE_UNITS()))),
        ga.nu(dropped, 0), id, t, stage, papers, kind, pigeons, inputs)

def aerie.action.pick?(+hit: Bool, +i: U32, +id: U32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  match hit:
    case True{}:
      ga.pair(J.JU{id}, J.JU{(i + 1 : U32)})
    case False{}:
      aerie.action(t, stage, papers, kind, pigeons, inputs)

def aerie.Run.tick_aerie.done?(+act: J.Val, +run: J.Val) -> J.Val:
  aerie.Run.tick_aerie.done.m(J.is_null(act), act, run)

def aerie.Run.tick_aerie.done.m?(+flag: Bool, +act: J.Val, +run: J.Val) -> J.Val:
  match flag:
    case True{}:
      run
    case False{}:
      aerie.Run.tick_aerie.marked(ga.nu(act, 0), ga.nu(act, 1), run)

def aerie.Run.tick_aerie.marked?(+id: U32, +action: U32, +run: J.Val) -> J.Val:
  aerie.Run.tick_aerie.said(id, action,
    aerie.Run.tick_aerie.drop(Bool.and(U32.is_ge(action, 1), U32.is_lt(action, 4)),
      ga.push(J.set(run, "marks", J.set(J.get(run, "marks"), aerie.quest_mark(action), J.JU{1})), "sounds", J.JS{"spoil_pickup"})))

# A paper picked up: the pigeons' dropped paper is cleared.

def aerie.Run.tick_aerie.drop?(+pick: Bool, +run: J.Val) -> J.Val:
  aerie.Run.tick_aerie.drop.m(Bool.and(pick, Bool.not(J.is_null(J.get(run, "pigeons")))), pick, run)

def aerie.Run.tick_aerie.drop.m?(+flag: Bool, +pick: Bool, +run: J.Val) -> J.Val:
  match flag:
    case True{}:
      J.set(run, "pigeons", ga.sv(J.get(run, "pigeons"), "dropped", J.JNull{}))
    case False{}:
      run

def aerie.Run.tick_aerie.said?(+id: U32, +action: U32, +run: J.Val) -> J.Val:
  aerie.Run.tick_aerie.said.m(U32.is_ge(action, 4), id, action, run)

def aerie.Run.tick_aerie.said.m?(+flag: Bool, +id: U32, +action: U32, +run: J.Val) -> J.Val:
  match flag:
    case True{}:
      aerie.Run.tick_aerie.crown(aerie.Run.crown_the_bonker(run, id), id)
    case False{}:
      aerie.Run.tick_aerie.found(run, id, aerie.message(action))

def aerie.message?(+action: U32) -> String:
  ga.ps(U32.is_eq(action, 0), "Lost minutes! Pigeons took them. Bonk them.",
    ga.ps(U32.is_eq(action, 1), "BELL TAX back. It taxes complaints too.",
      ga.ps(U32.is_eq(action, 2), "WATER TAX back. Damp paperwork.", "SEED TAX back. Slightly eaten.")))

def aerie.Run.tick_aerie.crown?(+rm: J.Val & String, +id: U32) -> J.Val:
  (run, message) = rm
  aerie.Run.tick_aerie.found(run, id, message)

def aerie.Run.tick_aerie.found?(+run: J.Val, +id: U32, +message: String) -> J.Val:
  J.set(run, "found", J.JA{[J.get(run, "tick"), J.JU{id}, J.JS{message}]})

# The minutes in: the best bonker is crowned and everyone is cleaned.

def aerie.Run.crown_the_bonker?(+run: J.Val, +giver: U32) -> J.Val & String:
  aerie.Run.crown_the_bonker.best(aerie.best(J.int_fields(J.get(ga.pv(J.is_null(J.get(run, "pigeons")), aerie.Pigeons.default(), J.get(run, "pigeons")), "bonks")), 0, 0, False{}), J.set(run, "pigeons", J.JNull{}), giver)

# The id with the most bonks, the lowest id of equals: (found, id).

def aerie.best?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +best_id: U32, +best_n: U32, +any: Bool) -> U32 & Bool:
  match hs:
    case Nil{}:
      (best_id, any)
    case Con{(+k, +n), +t}:
      aerie.best.pick(Bool.or(Bool.not(any), U32.is_gt(J.u32(n), best_n)), Maybe.default(&2, U32, U32.read(k), 0), J.u32(n), t, best_id, best_n)

def aerie.best.pick?(+better: Bool, +id: U32, +n: U32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +best_id: U32, +best_n: U32) -> U32 & Bool:
  aerie.best(t, ga.pu(better, id, best_id), ga.pu(better, n, best_n), True{})

def aerie.Run.crown_the_bonker.best?(+r: U32 & Bool, +run: J.Val, +giver: U32) -> J.Val & String:
  (id, any) = r
  aerie.Run.crown_the_bonker.name(ga.pu(any, id, giver), run)

def aerie.Run.crown_the_bonker.name?(+best: U32, +run: J.Val) -> J.Val & String:
  aerie.Run.crown_the_bonker.laurel(U32.is_gt(rules.Run.party_len(run), 1), best, run)

def aerie.Run.crown_the_bonker.laurel?(+crown: Bool, +best: U32, +run: J.Val) -> J.Val & String:
  match crown:
    case True{}:
      aerie.Run.crown_the_bonker.say(best, J.set(run, "players", aerie.laurels(J.int_fields(J.get(run, "players")), J.get(run, "players"), best)))
    case False{}:
      aerie.Run.crown_the_bonker.say(best, run)

def aerie.Run.crown_the_bonker.say?(+best: U32, +run: J.Val) -> J.Val & String:
  (run, "MINUTES IN! " ++ String.to_upper(ga.s(ga.at(J.get(run, "players"), best), "name")) ++ " BONKED BEST.")

def aerie.laurels?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +best: U32) -> J.Val:
  match hs:
    case Nil{}:
      players
    case Con{(+k, +h), +t}:
      aerie.laurels(t, J.set(players, k, ga.sb(h, "laurel", U32.is_eq(Maybe.default(&2, U32, U32.read(k), 0), best))), best)

# --- this room's pigeon ---------------------------------------------------------------------

# The first hostile-free shot with life left that is on the pigeon: it is spent,
# and its owner answers.

def aerie.shoot?(+ps: List<&2, J.Val>, +x: F32, +y: F32) -> List<&2, J.Val> & Maybe<&2, U32>:
  match ps:
    case Nil{}:
      (Nil{}, None{})
    case Con{+s, +t}:
      aerie.shoot.hit(
        Bool.and(Bool.and(Bool.not(ga.b(s, "hostile")), U32.is_gt(ga.u(s, "ttl"), 0)),
          F32.is_lt(RS.f_hypot(F32.sub(ga.f(s, "x"), x), F32.sub(ga.f(s, "y"), y)), aerie.BONK())),
        s, t, x, y)

def aerie.shoot.hit?(+hit: Bool, +s: J.Val, +t: List<&2, J.Val>, +x: F32, +y: F32) -> List<&2, J.Val> & Maybe<&2, U32>:
  match hit:
    case True{}:
      (Con{ga.su(s, "ttl", 0), t}, Some{ga.u(J.get(s, "traits"), "owner")})
    case False{}:
      aerie.shoot.rest(s, aerie.shoot(t, x, y))

def aerie.shoot.rest?(+s: J.Val, +r: List<&2, J.Val> & Maybe<&2, U32>) -> List<&2, J.Val> & Maybe<&2, U32>:
  (t, owner) = r
  (Con{s, t}, owner)

# The first living knight swinging within reach.

def aerie.swung?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32) -> Maybe<&2, U32>:
  match hs:
    case Nil{}:
      None{}
    case Con{(+k, +h), +t}:
      aerie.swung.hit(Bool.and(Bool.and(U32.is_gt(ga.u(h, "hp"), 0), U32.is_gt(ga.u(h, "swing"), 0)),
        F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)), aerie.SWING_REACH())), k, t, x, y)

def aerie.swung.hit?(+hit: Bool, +k: String, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32) -> Maybe<&2, U32>:
  match hit:
    case True{}:
      Some{Maybe.default(&2, U32, U32.read(k), 0)}
    case False{}:
      aerie.swung(t, x, y)

# The nearest knight to (x, y), the first of equals; 0 for an empty table.

def aerie.nearest?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32, +best: U32, +bd: F32, +any: Bool) -> U32:
  match hs:
    case Nil{}:
      best
    case Con{(+k, +h), +t}:
      aerie.nearest.pick(Bool.or(Bool.not(any), F32.is_gt(bd, RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)))),
        Maybe.default(&2, U32, U32.read(k), 0), RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)), t, x, y, best, bd)

def aerie.nearest.pick?(+better: Bool, +id: U32, +d: F32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32, +best: U32, +bd: F32) -> U32:
  aerie.nearest(t, x, y, ga.pu(better, id, best), ga.pf(better, d, bd), True{})

# Knights under the pigeon (hp > 0, within POOP_REACH), in seat order.

def aerie.under?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32) -> List<&2, U32>:
  match hs:
    case Nil{}:
      Nil{}
    case Con{(+k, +h), +t}:
      aerie.under.keep(Bool.and(U32.is_gt(ga.u(h, "hp"), 0), F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)), aerie.POOP_REACH())),
        Maybe.default(&2, U32, U32.read(k), 0), aerie.under(t, x, y))

def aerie.under.keep?(+hit: Bool, +id: U32, +rest: List<&2, U32>) -> List<&2, U32>:
  match hit:
    case True{}:
      Con{id, rest}
    case False{}:
      rest

# A poop on each knight underneath: their count (capped at 9) and the line.

def aerie.splat?(+run: J.Val, +p: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      J.set(run, "pigeons", p)
    case Con{+id, +t}:
      aerie.splat(
        J.set(ga.push(run, "sounds", J.JS{"hit_magic"}), "found", J.JA{[J.get(run, "tick"), J.JU{id}, J.JS{"SPLAT! THE PIGEON VOTED."}]}),
        ga.sv(p, "poop", J.set(J.get(p, "poop"), U32.show(id), J.JU{U32.min(ga.sadd8(ga.u(J.get(p, "poop"), U32.show(id)), 1), 9)})),
        t)

def aerie.Run.tick_pigeon?(+run: J.Val, +kind: String, +papers: U32) -> J.Val:
  aerie.Run.tick_pigeon.in(run, kind, papers, ga.pv(J.is_null(J.get(run, "pigeons")), aerie.Pigeons.default(), J.get(run, "pigeons")))

def aerie.Run.tick_pigeon.in?(+run: J.Val, +kind: String, +papers: U32, +p: J.Val) -> J.Val:
  aerie.Run.tick_pigeon.up(aerie.flying(kind, 1, papers, p), run, p)

def aerie.Run.tick_pigeon.up?(+i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  aerie.Run.tick_pigeon.up.m(U32.is_ge(i, 3), i, run, p)

def aerie.Run.tick_pigeon.up.m?(+flag: Bool, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  match flag:
    case True{}:
      J.set(run, "pigeons", p)
    case False{}:
      aerie.Run.tick_pigeon.at(aerie.pigeon_at(rules.Run.room(run), i, W.of_j(J.get(run, "tick"))), i, run, p)

def aerie.Run.tick_pigeon.at?(+xy: F32 & F32, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  (+x, +y) = xy
  aerie.Run.tick_pigeon.shot(aerie.shoot(J.items(J.get(run, "projectiles")), x, y), x, y, i, run, p)

def aerie.Run.tick_pigeon.shot?(+sh: List<&2, J.Val> & Maybe<&2, U32>, +x: F32, +y: F32, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  (ps, shot) = sh
  aerie.Run.tick_pigeon.owner(aerie.first_some(shot, aerie.swung(J.int_fields(J.get(run, "players")), x, y)), x, y, i, J.set(run, "projectiles", J.JA{ps}), p)

def aerie.first_some?(+a: Maybe<&2, U32>, +b: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match a:
    case Some{+v}:
      Some{v}
    case None{}:
      b

def aerie.Run.tick_pigeon.owner?(+o: Maybe<&2, U32>, +x: F32, +y: F32, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  match o:
    case Some{+owner}:
      aerie.Run.tick_pigeon.bonk(ga.pu(ga.has_key(J.get(run, "players"), owner), owner, aerie.nearest(J.int_fields(J.get(run, "players")), x, y, 0, 0.0, False{})), x, y, i, run, p)
    case None{}:
      aerie.Run.tick_pigeon.poop(U32.is_zero(W.rem_u32(W.of_j(J.get(run, "tick")), aerie.POOP_EVERY())), x, y, run, p)

def aerie.Run.tick_pigeon.bonk?(+by: U32, +x: F32, +y: F32, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  aerie.Run.tick_pigeon.land(ga.is(layout.Room.tile(rules.Run.room(run), RS.f_as_i32(F32.div(x, layout.TILE_UNITS())), RS.f_as_i32(F32.div(y, layout.TILE_UNITS()))), "floor"), by, x, y, i, run,
    ga.sv(p, "bonks", J.set(J.get(p, "bonks"), U32.show(by), J.JU{RS.as_u8((ga.u(J.get(p, "bonks"), U32.show(by)) + 1 : U32))})))

def aerie.Run.tick_pigeon.land?(+floor: Bool, +by: U32, +x: F32, +y: F32, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  aerie.Run.tick_pigeon.dropped(by, i, run, p, ga.pf(floor, x, F32.mul(aerie.doc_x(i), layout.TILE_UNITS())), ga.pf(floor, y, F32.mul(aerie.doc_y(i), layout.TILE_UNITS())))

def aerie.Run.tick_pigeon.dropped?(+by: U32, +i: U32, +run: J.Val, +p: J.Val, +ax: F32, +ay: F32) -> J.Val:
  J.set(J.set(ga.push(run, "sounds", J.JS{"hit_arrow"}), "found", J.JA{[J.get(run, "tick"), J.JU{by}, J.JS{"BONK! " ++ aerie.doc_name(i) ++ " DROPPED."}]}),
    "pigeons", ga.sv(p, "dropped", J.JA{[J.JU{i}, J.JF{ax}, J.JF{ay}]}))

def aerie.Run.tick_pigeon.poop?(+poop: Bool, +x: F32, +y: F32, +run: J.Val, +p: J.Val) -> J.Val:
  match poop:
    case True{}:
      aerie.splat(run, p, aerie.under(J.int_fields(J.get(run, "players")), x, y))
    case False{}:
      J.set(run, "pigeons", p)

def altar.REACH?() -> F32:
  2.4

def altar.EDGE_SLIDE?() -> F32:
  0.35

def altar.CORNER?() -> F32:
  0.9

def altar.FLARE_TICKS?() -> W.W:
  W.of_u32(60)

def altar.MOST_PENDING?() -> U32:
  24

def altar.MOST_LANDED?() -> U32:
  6

# --- the lamp ------------------------------------------------------------------

# Nothing has happened at the lamp.

def altar.Lamp.quiet?(+lamp: J.Val) -> Bool:
  Bool.and(U32.is_zero(J.len(J.get(lamp, "rubbed"))), Bool.and(U32.is_zero(J.len(J.get(lamp, "wishing"))), U32.is_zero(J.len(J.get(lamp, "landed")))))

# Knight `id`'s wishes in flight, oldest first.

def altar.Lamp.pending?(+lamp: J.Val, +id: U32) -> J.Val:
  J.JA{altar.Lamp.pending.go(J.items(J.get(lamp, "wishing")), id)}

def altar.Lamp.pending.go?(+ps: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      altar.Lamp.pending.keep(U32.is_eq(ga.u(p, "knight"), id), p, altar.Lamp.pending.go(t, id))

def altar.Lamp.pending.keep?(+mine: Bool, +p: J.Val, +rest: List<&2, J.Val>) -> List<&2, J.Val>:
  match mine:
    case True{}:
      Con{p, rest}
    case False{}:
      rest

# The newest grant, while it still flares at `tick`.

def altar.Lamp.flaring?(+lamp: J.Val, +tick: W.W) -> J.Val:
  altar.Lamp.flaring.last(List.reverse(&2, J.Val, J.items(J.get(lamp, "landed"))), tick)

def altar.Lamp.flaring.last?(+rev: List<&2, J.Val>, +tick: W.W) -> J.Val:
  match rev:
    case Nil{}:
      J.JNull{}
    case Con{+l, +t}:
      altar.Lamp.flaring.pick(W.lt(W.sat_sub(tick, W.of_j(J.get(l, "tick"))), altar.FLARE_TICKS()), l)

def altar.Lamp.flaring.pick?(+on: Bool, +l: J.Val) -> J.Val:
  ga.pv(on, l, J.JNull{})

# --- the stone's geometry --------------------------------------------------------

# Tiles a knight cannot cross once the altar stands at column `c`, row `r`.

def altar.stone_blocks?(+c: U32, +r: U32, +tc: U32, +tr: U32) -> Bool:
  altar.stone_blocks.box(
    F32.mul(F32.add(RS.i_as_f32(c), 0.5), layout.TILE_UNITS()),
    F32.mul(F32.add(RS.i_as_f32(r), 1.0), layout.TILE_UNITS()),
    F32.mul(F32.add(RS.i_as_f32(tc), 0.5), layout.TILE_UNITS()),
    F32.mul(F32.add(RS.i_as_f32(tr), 0.5), layout.TILE_UNITS()))

def altar.stone_blocks.box?(+ax: F32, +ay: F32, +cx: F32, +cy: F32) -> Bool:
  altar.stone_blocks.gap(
    F32.sub(ax, F32.mul(1.5, layout.TILE_UNITS())), F32.add(ax, F32.mul(1.5, layout.TILE_UNITS())),
    F32.sub(ay, F32.mul(1.4, layout.TILE_UNITS())), F32.add(ay, 0.6), cx, cy)

def altar.stone_blocks.gap?(+left: F32, +right: F32, +top: F32, +foot: F32, +cx: F32, +cy: F32) -> Bool:
  altar.stone_blocks.pick(Bool.and(F32.is_gt(cx, left), F32.is_lt(cx, right)), Bool.and(F32.is_gt(cy, top), F32.is_lt(cy, foot)),
    ga.pf(F32.is_le(cy, top), F32.sub(top, cy), ga.pf(F32.is_ge(cy, foot), F32.sub(cy, foot), 0.0)),
    ga.pf(F32.is_le(cx, left), F32.sub(left, cx), ga.pf(F32.is_ge(cx, right), F32.sub(cx, right), 0.0)))

def altar.stone_blocks.pick?(+xin: Bool, +yin: Bool, +gap_y: F32, +gap_x: F32) -> Bool:
  Bool.or(Bool.and(xin, yin),
    ga.pb(xin, F32.is_lt(gap_y, 0.45), ga.pb(yin, F32.is_lt(gap_x, 0.45), False{})))

# The four ways, in `layout::DIRS` order: north, east, south, west.

def altar.dir_dx?(+i: U32) -> U32:
  ga.pu(U32.is_eq(i, 1), 1, ga.pu(U32.is_eq(i, 3), 4294967295, 0))

def altar.dir_dy?(+i: U32) -> U32:
  ga.pu(U32.is_eq(i, 0), 4294967295, ga.pu(U32.is_eq(i, 2), 1, 0))

# Walkable ground: floor, door, stairs, bog.

def altar.ground?(+t: J.Val) -> Bool:
  Bool.or(Bool.or(ga.is(t, "floor"), ga.is(t, "door")), Bool.or(ga.is(t, "stairs"), ga.is(t, "bog")))

# --- stone_cuts_a_passage ------------------------------------------------------------

# The window round the stone: the room, the stone, and the box (c0, r0)-(c1, r1).

type AltarWin is Data:
  AltarWin{room: J.Val, c: U32, r: U32, c0: U32, r0: U32, c1: U32, r1: U32}

def altar.win.walk?(+win: AltarWin, +tc: U32, +tr: U32) -> Bool:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      altar.win.walk.in(
        Bool.and(Bool.and(RS.i_ge(tc, c0), RS.i_ge(tr, r0)), Bool.and(RS.i_le(tc, c1), RS.i_le(tr, r1))),
        Bool.and(Bool.and(RS.i_ge(tc, 0), RS.i_ge(tr, 0)), Bool.and(U32.is_lt(tc, ga.u(room, "cols")), U32.is_lt(tr, ga.u(room, "rows")))),
        room, tc, tr)

def altar.win.walk.in?(+inside: Bool, +ok: Bool, +room: J.Val, +tc: U32, +tr: U32) -> Bool:
  altar.win.walk.in.m(Bool.and(inside, ok), inside, ok, room, tc, tr)

def altar.win.walk.in.m?(+flag: Bool, +inside: Bool, +ok: Bool, +room: J.Val, +tc: U32, +tr: U32) -> Bool:
  match flag:
    case False{}:
      False{}
    case True{}:
      altar.ground(layout.Room.tile(room, tc, tr))

def altar.win.blocks?(+win: AltarWin, +tc: U32, +tr: U32) -> Bool:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      altar.stone_blocks(c, r, tc, tr)

# A walkable neighbour that the stone blocks.

def altar.win.touches?(+win: AltarWin, +tc: U32, +tr: U32, +i: U32) -> Bool:
  altar.win.touches.m(U32.is_eq(i, 4), win, tc, tr, i)

def altar.win.touches.m?(+flag: Bool, +win: AltarWin, +tc: U32, +tr: U32, +i: U32) -> Bool:
  match flag:
    case True{}:
      False{}
    case False{}:
      altar.win.touches.at(win, tc, tr, i, RS.i_add(tc, altar.dir_dx(i)), RS.i_add(tr, altar.dir_dy(i)))

def altar.win.touches.at?(+win: AltarWin, +tc: U32, +tr: U32, +i: U32, +nx: U32, +ny: U32) -> Bool:
  altar.win.touches.or(Bool.and(altar.win.walk(win, nx, ny), altar.win.blocks(win, nx, ny)), win, tc, tr, i)

def altar.win.touches.or?(+hit: Bool, +win: AltarWin, +tc: U32, +tr: U32, +i: U32) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      altar.win.touches(win, tc, tr, (i + 1 : U32))

# The seed of the flood: the first walkable tile the stone touches, in the scan
# rows `r-2 ..= r+3`, columns `c-2 ..= c+2`; else the first walkable one.

def altar.win.seed?(+win: AltarWin, +k: U32, +first: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>) -> Maybe<&2, Sigma<&2, &2, U32, _ => U32>>:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      altar.win.seed.at(U32.is_ge(k, 30), win, k, RS.i_add(c, RS.i_sub(U32.mod(k, 5), 2)), RS.i_add(r, RS.i_sub(U32.div(k, 5), 2)), first)

def altar.win.seed.at?(+done: Bool, +win: AltarWin, +k: U32, +tc: U32, +tr: U32, +first: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>) -> Maybe<&2, Sigma<&2, &2, U32, _ => U32>>:
  match done:
    case True{}:
      first
    case False{}:
      altar.win.seed.cand(Bool.and(altar.win.walk(win, tc, tr), Bool.not(altar.win.blocks(win, tc, tr))), win, k, tc, tr, first)

def altar.win.seed.cand?(+open: Bool, +win: AltarWin, +k: U32, +tc: U32, +tr: U32, +first: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>) -> Maybe<&2, Sigma<&2, &2, U32, _ => U32>>:
  match open:
    case False{}:
      altar.win.seed(win, (k + 1 : U32), first)
    case True{}:
      altar.win.seed.touch(altar.win.touches(win, tc, tr, 0), win, k, tc, tr, first)

def altar.win.seed.touch?(+touch: Bool, +win: AltarWin, +k: U32, +tc: U32, +tr: U32, +first: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>) -> Maybe<&2, Sigma<&2, &2, U32, _ => U32>>:
  match touch:
    case True{}:
      Some{(tc, tr)}
    case False{}:
      altar.win.seed(win, (k + 1 : U32), altar.win.seed.keep(first, tc, tr))

def altar.win.seed.keep?(+first: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>, +tc: U32, +tr: U32) -> Maybe<&2, Sigma<&2, &2, U32, _ => U32>>:
  match first:
    case Some{+p}:
      Some{p}
    case None{}:
      Some{(tc, tr)}

# The cell index within the window: (tr - r0) * 25 + (tc - c0).

def altar.win.idx?(+win: AltarWin, +tc: U32, +tr: U32) -> U32:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      (RS.i_sub(tr, r0) * 25 + RS.i_sub(tc, c0) : U32)

# The flood of walkable cells from the seed; `block` makes the stone a wall.
# The stack holds cell indices; `seen` is a set of index keys.

def altar.flood?(+win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +fuel: Nat) -> J.Val:
  match fuel:
    case 0n:
      seen
    case 1n+p:
      altar.flood.pop(stack, win, block, seen, p)

def altar.flood.pop?(+stack: List<&2, U32>, +win: AltarWin, +block: Bool, +seen: J.Val, +p: Nat) -> J.Val:
  match stack:
    case Nil{}:
      seen
    case Con{+i, +t}:
      altar.flood.dirs(0, win, block, t, seen, p, RS.i_add(altar.win.c0(win), U32.mod(i, 25)), RS.i_add(altar.win.r0(win), U32.div(i, 25)))

def altar.win.c0?(+win: AltarWin) -> U32:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      c0

def altar.win.r0?(+win: AltarWin) -> U32:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      r0

def altar.flood.dirs?(+d: U32, +win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +p: Nat, +x: U32, +y: U32) -> J.Val:
  altar.flood.dirs.m(U32.is_eq(d, 4), d, win, block, stack, seen, p, x, y)

def altar.flood.dirs.m?(+flag: Bool, +d: U32, +win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +p: Nat, +x: U32, +y: U32) -> J.Val:
  match flag:
    case True{}:
      altar.flood(win, block, stack, seen, p)
    case False{}:
      altar.flood.step(d, win, block, stack, seen, p, x, y, RS.i_add(x, altar.dir_dx(d)), RS.i_add(y, altar.dir_dy(d)))

def altar.flood.step?(+d: U32, +win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +p: Nat, +x: U32, +y: U32, +nx: U32, +ny: U32) -> J.Val:
  altar.flood.visit(
    Bool.and(altar.win.walk(win, nx, ny), Bool.not(Bool.and(block, altar.win.blocks(win, nx, ny)))),
    d, win, block, stack, seen, p, x, y, nx, ny)

def altar.flood.visit?(+ok: Bool, +d: U32, +win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +p: Nat, +x: U32, +y: U32, +nx: U32, +ny: U32) -> J.Val:
  match ok:
    case False{}:
      altar.flood.dirs((d + 1 : U32), win, block, stack, seen, p, x, y)
    case True{}:
      altar.flood.mark(altar.win.idx(win, nx, ny), J.has(seen, U32.show(altar.win.idx(win, nx, ny))), d, win, block, stack, seen, p, x, y)

def altar.flood.mark?(+j: U32, +was: Bool, +d: U32, +win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +p: Nat, +x: U32, +y: U32) -> J.Val:
  match was:
    case True{}:
      altar.flood.dirs((d + 1 : U32), win, block, stack, seen, p, x, y)
    case False{}:
      altar.flood.dirs((d + 1 : U32), win, block, Con{j, stack}, J.set(seen, U32.show(j), J.JBool{True{}}), p, x, y)

# Some cell reachable without the stone that is no longer reachable around it.

def altar.cuts.any?(+win: AltarWin, +open: J.Val, +around: J.Val, +i: U32) -> Bool:
  altar.cuts.any.m(U32.is_ge(i, 625), win, open, around, i)

def altar.cuts.any.m?(+flag: Bool, +win: AltarWin, +open: J.Val, +around: J.Val, +i: U32) -> Bool:
  match flag:
    case True{}:
      False{}
    case False{}:
      altar.cuts.any.at(win, open, around, i, Bool.and(J.has(open, U32.show(i)), Bool.not(J.has(around, U32.show(i)))))

def altar.cuts.any.at?(+win: AltarWin, +open: J.Val, +around: J.Val, +i: U32, +hit: Bool) -> Bool:
  match hit:
    case False{}:
      altar.cuts.any(win, open, around, (i + 1 : U32))
    case True{}:
      altar.cuts.any.free(Bool.not(altar.win.blocks(win, RS.i_add(altar.win.c0(win), U32.mod(i, 25)), RS.i_add(altar.win.r0(win), U32.div(i, 25)))), win, open, around, i)

def altar.cuts.any.free?(+free: Bool, +win: AltarWin, +open: J.Val, +around: J.Val, +i: U32) -> Bool:
  match free:
    case True{}:
      True{}
    case False{}:
      altar.cuts.any(win, open, around, (i + 1 : U32))

# The stone at (`c`, `r`) would seal a walk the party could take.

def altar.stone_cuts_a_passage?(+room: J.Val, +c: U32, +r: U32) -> Bool:
  altar.stone_cuts_a_passage.seed(AltarWin{room, c, r, RS.i_sub(c, 12), RS.i_sub(r, 12), RS.i_add(c, 12), RS.i_add(r, 12)})

def altar.stone_cuts_a_passage.seed?(+win: AltarWin) -> Bool:
  altar.stone_cuts_a_passage.flood(altar.win.seed(win, 0, None{}), win)

def altar.stone_cuts_a_passage.flood?(+seed: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>, +win: AltarWin) -> Bool:
  match seed:
    case None{}:
      False{}
    case Some{(+tc, +tr)}:
      altar.stone_cuts_a_passage.sets(altar.win.idx(win, tc, tr), win)

def altar.stone_cuts_a_passage.sets?(+s: U32, +win: AltarWin) -> Bool:
  altar.cuts.any(win,
    altar.flood(win, False{}, [s], J.set(J.obj(), U32.show(s), J.JBool{True{}}), 700n),
    altar.flood(win, True{}, [s], J.set(J.obj(), U32.show(s), J.JBool{True{}}), 700n), 0)

# --- route_tiles ---------------------------------------------------------------------------

# Whether tile index `k` (col = k % cols, row = k / cols) is walkable ground.

def altar.route.walk?(+room: J.Val, +k: U32, +cols: U32) -> Bool:
  altar.ground(layout.Room.tile(room, U32.mod(k, cols), U32.div(k, cols)))

# The breadth-first search. `front`/`rear` are the queue (rear reversed); `back`
# maps a tile index to the one it was reached from; `goals` collects doors and
# stairs in the order met (reversed).

def altar.route.bfs?(+room: J.Val, +cols: U32, +rows: U32, +start: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +back: J.Val, +goals: List<&2, U32>, +fuel: Nat) -> J.Val & List<&2, U32>:
  match fuel:
    case 0n:
      (back, goals)
    case 1n+p:
      altar.route.bfs.pop(front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.pop?(+front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match front:
    case Con{+k, +t}:
      altar.route.bfs.visit(k, t, rear, room, cols, rows, start, back, goals, p)
    case Nil{}:
      altar.route.bfs.flip(List.reverse(&2, U32, rear), room, cols, rows, start, back, goals, p)

def altar.route.bfs.flip?(+front: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match front:
    case Nil{}:
      (back, goals)
    case Con{+k, +t}:
      altar.route.bfs.visit(k, t, Nil{}, room, cols, rows, start, back, goals, p)

def altar.route.bfs.visit?(+k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  altar.route.bfs.goal(
    Bool.and(U32.is_ne(k, start), Bool.or(ga.is(layout.Room.tile(room, U32.mod(k, cols), U32.div(k, cols)), "door"), ga.is(layout.Room.tile(room, U32.mod(k, cols), U32.div(k, cols)), "stairs"))),
    k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.goal?(+is_goal: Bool, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match is_goal:
    case True{}:
      altar.route.bfs.dirs(0, k, front, rear, room, cols, rows, start, back, Con{k, goals}, p)
    case False{}:
      altar.route.bfs.dirs(0, k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.dirs?(+d: U32, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  altar.route.bfs.dirs.m(U32.is_eq(d, 4), d, k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.dirs.m?(+flag: Bool, +d: U32, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match flag:
    case True{}:
      altar.route.bfs(room, cols, rows, start, front, rear, back, goals, p)
    case False{}:
      altar.route.bfs.next(RS.i_add(U32.mod(k, cols), altar.dir_dx(d)), RS.i_add(U32.div(k, cols), altar.dir_dy(d)), d, k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.next?(+nx: U32, +ny: U32, +d: U32, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  altar.route.bfs.take(
    Bool.and(Bool.and(RS.i_ge(nx, 0), RS.i_lt(nx, cols)), Bool.and(RS.i_ge(ny, 0), RS.i_lt(ny, rows))),
    nx, ny, d, k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.take?(+inb: Bool, +nx: U32, +ny: U32, +d: U32, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match inb:
    case False{}:
      altar.route.bfs.dirs((d + 1 : U32), k, front, rear, room, cols, rows, start, back, goals, p)
    case True{}:
      altar.route.bfs.fresh(Bool.and(Bool.not(J.has(back, U32.show((ny * cols + nx : U32)))), altar.route.walk(room, (ny * cols + nx : U32), cols)),
        (ny * cols + nx : U32), d, k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.fresh?(+new: Bool, +j: U32, +d: U32, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match new:
    case False{}:
      altar.route.bfs.dirs((d + 1 : U32), k, front, rear, room, cols, rows, start, back, goals, p)
    case True{}:
      altar.route.bfs.dirs((d + 1 : U32), k, front, Con{j, rear}, room, cols, rows, start, J.set(back, U32.show(j), J.JU{k}), goals, p)

# Mark the walk from a goal back toward the start.

def altar.route.mark?(+k: U32, +home: U32, +back: J.Val, +on: J.Val, +fuel: Nat) -> J.Val:
  match fuel:
    case 0n:
      on
    case 1n+p:
      altar.route.mark.at(U32.is_eq(k, home), k, home, back, on, p)

def altar.route.mark.at?(+done: Bool, +k: U32, +home: U32, +back: J.Val, +on: J.Val, +p: Nat) -> J.Val:
  match done:
    case True{}:
      on
    case False{}:
      altar.route.mark.prev(J.has(back, U32.show(k)), k, home, back, J.set(on, U32.show(k), J.JBool{True{}}), p)

def altar.route.mark.prev?(+has: Bool, +k: U32, +home: U32, +back: J.Val, +on: J.Val, +p: Nat) -> J.Val:
  match has:
    case False{}:
      on
    case True{}:
      altar.route.mark(J.u32(J.get(back, U32.show(k))), home, back, on, p)

def altar.route.goals?(+gs: List<&2, U32>, +home: U32, +back: J.Val, +on: J.Val, +fuel: Nat) -> J.Val:
  match gs:
    case Nil{}:
      on
    case Con{+g, +t}:
      altar.route.goals(t, home, back, altar.route.mark(g, home, back, on, fuel), fuel)

# The extra goals (the open floor's chamber centres) that the search reached.

def altar.route.extra?(+ex: List<&2, J.Val>, +room: J.Val, +cols: U32, +rows: U32, +back: J.Val, +goals: List<&2, U32>) -> List<&2, U32>:
  match ex:
    case Nil{}:
      goals
    case Con{+e, +t}:
      altar.route.extra(t, room, cols, rows, back, altar.route.extra.one(RS.f_as_i32(F32.div(J.f32(J.nth(e, 0)), layout.TILE_UNITS())), RS.f_as_i32(F32.div(J.f32(J.nth(e, 1)), layout.TILE_UNITS())), cols, rows, back, goals))

def altar.route.extra.one?(+c: U32, +r: U32, +cols: U32, +rows: U32, +back: J.Val, +goals: List<&2, U32>) -> List<&2, U32>:
  altar.route.extra.in(Bool.and(Bool.and(RS.i_ge(c, 0), RS.i_lt(c, cols)), Bool.and(RS.i_ge(r, 0), RS.i_lt(r, rows))), c, r, cols, back, goals)

def altar.route.extra.in?(+inb: Bool, +c: U32, +r: U32, +cols: U32, +back: J.Val, +goals: List<&2, U32>) -> List<&2, U32>:
  match inb:
    case False{}:
      goals
    case True{}:
      altar.route.extra.add(J.has(back, U32.show((r * cols + c : U32))), (r * cols + c : U32), goals)

def altar.route.extra.add?(+reached: Bool, +j: U32, +goals: List<&2, U32>) -> List<&2, U32>:
  match reached:
    case True{}:
      Con{j, goals}
    case False{}:
      goals

# The route tiles as the set of tile indices on a shortest walk (before it is
# turned into the flag vector).

def altar.route.set?(+room: J.Val, +from: J.Val, +extra: J.Val) -> J.Val:
  altar.route.set.start(room, ga.u(room, "cols"), ga.u(room, "rows"), RS.f_as_i32(F32.div(J.f32(J.nth(from, 0)), layout.TILE_UNITS())), RS.f_as_i32(F32.div(J.f32(J.nth(from, 1)), layout.TILE_UNITS())), extra)

def altar.route.set.start?(+room: J.Val, +cols: U32, +rows: U32, +sx: U32, +sy: U32, +extra: J.Val) -> J.Val:
  altar.route.set.ok(Bool.and(Bool.and(RS.i_ge(sx, 0), RS.i_lt(sx, cols)), Bool.and(RS.i_ge(sy, 0), RS.i_lt(sy, rows))), room, cols, rows, sx, sy, extra)

def altar.route.set.ok?(+inb: Bool, +room: J.Val, +cols: U32, +rows: U32, +sx: U32, +sy: U32, +extra: J.Val) -> J.Val:
  match inb:
    case False{}:
      J.obj()
    case True{}:
      altar.route.set.walk(altar.route.walk(room, (sy * cols + sx : U32), cols), room, cols, rows, (sy * cols + sx : U32), extra)

def altar.route.set.walk?(+ok: Bool, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +extra: J.Val) -> J.Val:
  match ok:
    case False{}:
      J.obj()
    case True{}:
      altar.route.set.done(altar.route.bfs(room, cols, rows, start, [start], Nil{}, J.set(J.obj(), U32.show(start), J.JU{start}), Nil{}, U32.to_nat((cols * rows + 1 : U32))), room, cols, rows, start, extra)

def altar.route.set.done?(+r: J.Val & List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +extra: J.Val) -> J.Val:
  (back, goals) = r
  altar.route.set.mark(List.reverse(&2, U32, goals), room, cols, rows, start, back, J.items(extra))

def altar.route.set.mark?(+goals: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +ex: List<&2, J.Val>) -> J.Val:
  altar.route.goals(altar.route.extra(ex, room, cols, rows, back, goals) , start, back, J.obj(), U32.to_nat((cols * rows + 1 : U32)))

def altar.route.flags?(+on: J.Val, +i: U32, +n: U32) -> List<&2, J.Val>:
  altar.route.flags.m(U32.is_ge(i, n), on, i, n)

def altar.route.flags.m?(+flag: Bool, +on: J.Val, +i: U32, +n: U32) -> List<&2, J.Val>:
  match flag:
    case True{}:
      Nil{}
    case False{}:
      Con{J.JBool{J.has(on, U32.show(i))}, altar.route.flags(on, (i + 1 : U32), n)}

# Tiles that lie on a shortest walk from `from` to a door, the stairs, or one
# of `extra`: the flag vector.

def altar.route_tiles?(+room: J.Val, +from: J.Val, +extra: J.Val) -> J.Val:
  J.JA{altar.route.flags(altar.route.set(room, from, extra), 0, (ga.u(room, "cols") * ga.u(room, "rows") : U32))}

def altar.stone_stands_on_a_route?(+on: J.Val, +cols: U32, +c: U32, +r: U32) -> Bool:
  altar.route.stands(on, cols, c, r, 0)

def altar.route.stands?(+on: J.Val, +cols: U32, +c: U32, +r: U32, +k: U32) -> Bool:
  altar.route.stands.m(U32.is_ge(k, 9), on, cols, c, r, k)

def altar.route.stands.m?(+flag: Bool, +on: J.Val, +cols: U32, +c: U32, +r: U32, +k: U32) -> Bool:
  match flag:
    case True{}:
      False{}
    case False{}:
      altar.route.stands.at(on, cols, c, r, k, RS.i_add(c, RS.i_sub(U32.mod(k, 3), 1)), RS.i_add(r, RS.i_sub(U32.div(k, 3), 1)))

def altar.route.stands.at?(+on: J.Val, +cols: U32, +c: U32, +r: U32, +k: U32, +tc: U32, +tr: U32) -> Bool:
  altar.route.stands.hit(
    Bool.and(Bool.and(altar.stone_blocks(c, r, tc, tr), Bool.not(RS.i_lt(tc, 0))), Bool.not(RS.i_lt(tr, 0))),
    on, cols, c, r, k, (tr * cols + tc : U32))

def altar.route.stands.hit?(+live: Bool, +on: J.Val, +cols: U32, +c: U32, +r: U32, +k: U32, +i: U32) -> Bool:
  match live:
    case False{}:
      altar.route.stands(on, cols, c, r, (k + 1 : U32))
    case True{}:
      altar.route.stands.on(Bool.and(U32.is_lt(i, J.len(on)), J.bool(J.nth(on, i))), on, cols, c, r, k)

def altar.route.stands.on?(+on_route: Bool, +on: J.Val, +cols: U32, +c: U32, +r: U32, +k: U32) -> Bool:
  match on_route:
    case True{}:
      True{}
    case False{}:
      altar.route.stands(on, cols, c, r, (k + 1 : U32))

# --- where the altar stands -------------------------------------------------------

# A tile as a number of tiles: `(u / TILE_UNITS).floor() as i32`.

def altar.tile?(+u: F32) -> U32:
  RS.f_as_i32(F32.floor(F32.div(u, layout.TILE_UNITS())))

# The anchor of the search: (col, row), or null for a room that keeps none.

def altar.place.anchor?(+room: J.Val, +arrival: J.Val) -> J.Val:
  altar.place.anchor.in(J.is_null(arrival), ga.s(room, "kind"), RS.i_div(ga.u(room, "cols"), 2), RS.i_div(ga.u(room, "rows"), 2), arrival)

def altar.place.anchor.in?(+open: Bool, +kind: String, +hc: U32, +hr: U32, +arrival: J.Val) -> J.Val:
  altar.place.anchor.pick(open, String.eq(kind, "start"), String.eq(kind, "sanctuary"), hc, hr, arrival)

def altar.place.anchor.pick?(+no_arrival: Bool, +start: Bool, +sanct: Bool, +hc: U32, +hr: U32, +arrival: J.Val) -> J.Val:
  match no_arrival:
    case False{}:
      ga.pair_i(RS.i_sub(altar.tile(J.f32(J.nth(arrival, 0))), 6), RS.i_sub(altar.tile(J.f32(J.nth(arrival, 1))), 2))
    case True{}:
      altar.place.anchor.kind(start, sanct, hc, hr)

def altar.place.anchor.kind?(+start: Bool, +sanct: Bool, +hc: U32, +hr: U32) -> J.Val:
  match start:
    case True{}:
      ga.pair_i(RS.i_sub(hc, 7), RS.i_sub(hr, 3))
    case False{}:
      altar.place.anchor.sanct(sanct, hc, hr)

def altar.place.anchor.sanct?(+sanct: Bool, +hc: U32, +hr: U32) -> J.Val:
  match sanct:
    case True{}:
      ga.pair_i(RS.i_sub(hc, 8), hr)
    case False{}:
      J.JNull{}

# The 3x2 patch (c-1..=c+1, r..=r+1) is all floor.

def altar.place.floor3?(+room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  altar.place.floor3.m(U32.is_ge(k, 6), room, c, r, k)

def altar.place.floor3.m?(+flag: Bool, +room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  match flag:
    case True{}:
      True{}
    case False{}:
      altar.place.floor3.at(ga.is(layout.Room.tile(room, RS.i_add(c, RS.i_sub(U32.mod(k, 3), 1)), RS.i_add(r, U32.div(k, 3))), "floor"), room, c, r, k)

def altar.place.floor3.at?(+f: Bool, +room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  match f:
    case True{}:
      altar.place.floor3(room, c, r, (k + 1 : U32))
    case False{}:
      False{}

# No door in c-3..=c+3 x r-2..=r+3.

def altar.place.nodoor?(+room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  altar.place.nodoor.m(U32.is_ge(k, 42), room, c, r, k)

def altar.place.nodoor.m?(+flag: Bool, +room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  match flag:
    case True{}:
      True{}
    case False{}:
      altar.place.nodoor.at(ga.is(layout.Room.tile(room, RS.i_add(c, RS.i_sub(U32.mod(k, 7), 3)), RS.i_add(r, RS.i_sub(U32.div(k, 7), 2))), "door"), room, c, r, k)

def altar.place.nodoor.at?(+d: Bool, +room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  match d:
    case True{}:
      False{}
    case False{}:
      altar.place.nodoor(room, c, r, (k + 1 : U32))

# A spot the stone may stand: clear floor, off the doors, sealing no walk.

def altar.place.clear?(+room: J.Val, +c: U32, +r: U32) -> Bool:
  altar.place.clear.a(altar.place.floor3(room, c, r, 0), room, c, r)

def altar.place.clear.a?(+f: Bool, +room: J.Val, +c: U32, +r: U32) -> Bool:
  match f:
    case False{}:
      False{}
    case True{}:
      altar.place.clear.b(altar.place.nodoor(room, c, r, 0), room, c, r)

def altar.place.clear.b?(+f: Bool, +room: J.Val, +c: U32, +r: U32) -> Bool:
  match f:
    case False{}:
      False{}
    case True{}:
      Bool.not(altar.stone_cuts_a_passage(room, c, r))

# The spiral: rings 0..12, rows then columns within a ring. `aside` is the first
# spot that sat on a route (null when none). Answers the place, or null.

def altar.place.scan?(+room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  altar.place.scan.m(U32.is_ge(ring, 12), room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.m?(+flag: Bool, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match flag:
    case True{}:
      altar.place.at(aside)
    case False{}:
      altar.place.scan.row(RS.i_gt(dr, ring), room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.row?(+past: Bool, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match past:
    case True{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, (ring + 1 : U32), RS.i_neg((ring + 1 : U32)), RS.i_neg((ring + 1 : U32)), aside)
    case False{}:
      altar.place.scan.col(RS.i_gt(dc, ring), room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.col?(+past: Bool, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match past:
    case True{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, ring, RS.i_add(dr, 1), RS.i_neg(ring), aside)
    case False{}:
      altar.place.scan.edge(U32.is_eq(RS.i_max(RS.i_abs(dr), RS.i_abs(dc)), ring), room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.edge?(+on_ring: Bool, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match on_ring:
    case False{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, ring, dr, RS.i_add(dc, 1), aside)
    case True{}:
      altar.place.scan.cand(RS.i_add(ac, dc), RS.i_add(ar, dr), room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.cand?(+c: U32, +r: U32, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  altar.place.scan.ok(
    Bool.or(Bool.or(RS.i_lt(c, 2), RS.i_lt(r, 2)), Bool.or(RS.i_gt(c, RS.i_sub(cols, 3)), RS.i_gt(r, RS.i_sub(rows, 3)))),
    c, r, room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.ok?(+skip: Bool, +c: U32, +r: U32, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match skip:
    case True{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, ring, dr, RS.i_add(dc, 1), aside)
    case False{}:
      altar.place.scan.clear(altar.place.clear(room, c, r), c, r, room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.clear?(+clear: Bool, +c: U32, +r: U32, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match clear:
    case False{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, ring, dr, RS.i_add(dc, 1), aside)
    case True{}:
      altar.place.scan.route(Bool.and(Bool.not(J.is_null(routes)), altar.stone_stands_on_a_route(routes, cols, c, r)), c, r, room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.route?(+on_route: Bool, +c: U32, +r: U32, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match on_route:
    case False{}:
      altar.place.at(ga.pair_i(c, r))
    case True{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, ring, dr, RS.i_add(dc, 1), altar.place.first(aside, c, r))

def altar.place.first?(+aside: J.Val, +c: U32, +r: U32) -> J.Val:
  ga.pv(J.is_null(aside), ga.pair_i(c, r), aside)

# The stone's spot for tile (c, r): (c + 0.5, r + 1.0) tiles in units.

def altar.place.at?(+cr: J.Val) -> J.Val:
  altar.place.at.in(J.is_null(cr), cr)

def altar.place.at.in?(+none: Bool, +cr: J.Val) -> J.Val:
  match none:
    case True{}:
      J.JNull{}
    case False{}:
      ga.pair_f(F32.mul(F32.add(RS.i_as_f32(J.i32(J.nth(cr, 0))), 0.5), layout.TILE_UNITS()), F32.mul(F32.add(RS.i_as_f32(J.i32(J.nth(cr, 1))), 1.0), layout.TILE_UNITS()))

# Where the altar stands in `room`, if it has one: a clear stretch of floor
# three tiles wide and two deep, found by a spiral out from the spot that suits
# the room. `arrival` is null or [x, y]; `extra` an array of [x, y].

def altar.place?(+room: J.Val, +arrival: J.Val, +extra: J.Val) -> J.Val:
  altar.place.go(altar.place.anchor(room, arrival), room, arrival, extra)

def altar.place.go?(+anchor: J.Val, +room: J.Val, +arrival: J.Val, +extra: J.Val) -> J.Val:
  altar.place.go.in(J.is_null(anchor), anchor, room, arrival, extra)

def altar.place.go.in?(+none: Bool, +anchor: J.Val, +room: J.Val, +arrival: J.Val, +extra: J.Val) -> J.Val:
  match none:
    case True{}:
      J.JNull{}
    case False{}:
      altar.place.go.routes(ga.pv(J.is_null(arrival), J.JNull{}, altar.route_tiles(room, arrival, extra)), anchor, room)

def altar.place.go.routes?(+routes: J.Val, +anchor: J.Val, +room: J.Val) -> J.Val:
  altar.place.scan(room, routes, ga.u(room, "cols"), ga.u(room, "rows"), J.i32(J.nth(anchor, 0)), J.i32(J.nth(anchor, 1)), 0, 0, 0, J.JNull{})

def altar.spot?(+room: J.Val, +arrival: J.Val) -> J.Val:
  altar.place(room, arrival, J.JA{[]})

# A room that keeps a wish altar: the open floor's map, a start room, or a
# Sanctuary. The identity start and the home floor do not.

def altar.keeps_an_altar?(+floor: J.Val, +index: U32, +identity: Bool) -> Bool:
  altar.keeps_an_altar.in(Bool.or(Bool.or(identity, U32.is_zero(ga.u(floor, "depth"))), U32.is_ge(index, J.len(J.get(floor, "rooms")))), floor, index)

def altar.keeps_an_altar.in?(+no: Bool, +floor: J.Val, +index: U32) -> Bool:
  match no:
    case True{}:
      False{}
    case False{}:
      Bool.or(
        Bool.and(Bool.not(J.is_null(J.get(floor, "open"))), U32.is_eq(ga.u(J.get(floor, "open"), "map"), index)),
        Bool.or(ga.is(J.get(J.nth(J.get(floor, "rooms"), index), "kind"), "start"), ga.is(J.get(J.nth(J.get(floor, "rooms"), index), "kind"), "sanctuary")))

# The middle of an open map's chamber (rect = (col, row, cols, rows), u16).

def altar.chamber.centre?(+ch: J.Val) -> J.Val:
  ga.pair_f(
    F32.div(F32.add(F32.mul(RS.u_as_f32(ga.nu(J.get(ch, "rect"), 0)), layout.TILE_UNITS()), F32.mul(RS.u_as_f32(RS.as_u16((ga.nu(J.get(ch, "rect"), 0) + ga.nu(J.get(ch, "rect"), 2) : U32))), layout.TILE_UNITS())), 2.0),
    F32.div(F32.add(F32.mul(RS.u_as_f32(ga.nu(J.get(ch, "rect"), 1)), layout.TILE_UNITS()), F32.mul(RS.u_as_f32(RS.as_u16((ga.nu(J.get(ch, "rect"), 1) + ga.nu(J.get(ch, "rect"), 3) : U32))), layout.TILE_UNITS())), 2.0))

def altar.centres?(+chs: List<&2, J.Val>) -> List<&2, J.Val>:
  match chs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      Con{altar.chamber.centre(c), altar.centres(t)}

# Choose the wish altar's spot for every room on `floor` that keeps one and
# does not yet have it. A room already kept is left where it was stood.

def altar.place_on?(+floor: J.Val, +identity: Bool) -> J.Val:
  altar.place_on.go(floor, identity, 0, J.len(J.get(floor, "rooms")))

def altar.place_on.go?(+floor: J.Val, +identity: Bool, +i: U32, +n: U32) -> J.Val:
  altar.place_on.go.m(U32.is_ge(i, n), floor, identity, i, n)

def altar.place_on.go.m?(+flag: Bool, +floor: J.Val, +identity: Bool, +i: U32, +n: U32) -> J.Val:
  match flag:
    case True{}:
      floor
    case False{}:
      altar.place_on.room(Bool.or(J.has(J.get(floor, "altar_at"), U32.show(i)), Bool.not(altar.keeps_an_altar(floor, i, identity))), floor, identity, i, n)

def altar.place_on.room?(+skip: Bool, +floor: J.Val, +identity: Bool, +i: U32, +n: U32) -> J.Val:
  match skip:
    case True{}:
      altar.place_on.go(floor, identity, (i + 1 : U32), n)
    case False{}:
      altar.place_on.put(altar.place_on.args(floor, i), floor, identity, i, n)

# (arrival, extra) for room `i`: the open map's arrival and chamber centres.

def altar.place_on.args?(+floor: J.Val, +i: U32) -> J.Val:
  altar.place_on.args.in(Bool.and(Bool.not(J.is_null(J.get(floor, "open"))), U32.is_eq(ga.u(J.get(floor, "open"), "map"), i)), floor)

def altar.place_on.args.in?(+open: Bool, +floor: J.Val) -> J.Val:
  match open:
    case True{}:
      ga.pair(J.get(J.get(floor, "open"), "arrival"), J.JA{altar.centres(J.items(J.get(J.get(floor, "open"), "chambers")))})
    case False{}:
      ga.pair(J.JNull{}, J.JA{[]})

def altar.place_on.put?(+args: J.Val, +floor: J.Val, +identity: Bool, +i: U32, +n: U32) -> J.Val:
  altar.place_on.go(
    J.set(floor, "altar_at", J.set(ga.pv(J.has(floor, "altar_at"), J.get(floor, "altar_at"), J.obj()), U32.show(i),
      altar.place(J.nth(J.get(floor, "rooms"), i), J.nth(args, 0), J.nth(args, 1)))),
    identity, (i + 1 : U32), n)

# --- the Run ---------------------------------------------------------------------------

# Stand the wish altar in every room that has one and does not yet keep a spot.

def altar.Run.choose_altars?(+run: J.Val) -> J.Val:
  J.set(run, "dungeon", altar.place_on(J.get(run, "dungeon"), ga.b(run, "identity")))

# The room the party is in keeps an altar whose spot is not yet chosen.

def altar.Run.altar_due?(+run: J.Val) -> Bool:
  Bool.and(altar.keeps_an_altar(J.get(run, "dungeon"), ga.u(run, "at"), ga.b(run, "identity")),
    Bool.not(J.has(J.get(J.get(run, "dungeon"), "altar_at"), U32.show(ga.u(run, "at")))))

# Where the wish altar stands in the room the party is in: none in a room
# without one, or under the identity start. Chosen once, and kept on the floor.

def altar.Run.wish_altar?(+run: J.Val) -> J.Val:
  altar.Run.wish_altar.in(altar.keeps_an_altar(J.get(run, "dungeon"), ga.u(run, "at"), ga.b(run, "identity")), run)

def altar.Run.wish_altar.in?(+keeps: Bool, +run: J.Val) -> J.Val:
  match keeps:
    case False{}:
      J.JNull{}
    case True{}:
      altar.Run.wish_altar.kept(J.has(J.get(J.get(run, "dungeon"), "altar_at"), U32.show(ga.u(run, "at"))), run)

def altar.Run.wish_altar.kept?(+has: Bool, +run: J.Val) -> J.Val:
  match has:
    case True{}:
      J.get(J.get(J.get(run, "dungeon"), "altar_at"), U32.show(ga.u(run, "at")))
    case False{}:
      altar.Run.wish_altar.open(J.get(J.get(run, "dungeon"), "open"), run)

def altar.Run.wish_altar.open?(+open: J.Val, +run: J.Val) -> J.Val:
  altar.Run.wish_altar.here(Bool.and(Bool.not(J.is_null(open)), U32.is_eq(ga.u(open, "map"), ga.u(run, "at"))), open, run)

def altar.Run.wish_altar.here?(+here: Bool, +open: J.Val, +run: J.Val) -> J.Val:
  match here:
    case True{}:
      altar.place(rules.Run.room(run), J.get(open, "arrival"), J.JA{altar.centres(J.items(J.get(open, "chambers")))})
    case False{}:
      altar.place(rules.Run.room(run), J.JNull{}, J.JA{[]})

# Knight `id` stands within reach of the lamp.

def altar.Run.near_altar?(+run: J.Val, +id: U32) -> Bool:
  altar.Run.near_altar.in(altar.Run.wish_altar(run), ga.at(J.get(run, "players"), id))

def altar.Run.near_altar.in?(+spot: J.Val, +hero: J.Val) -> Bool:
  altar.Run.near_altar.go(Bool.or(J.is_null(spot), J.is_null(hero)), spot, hero)

def altar.Run.near_altar.go?(+no: Bool, +spot: J.Val, +hero: J.Val) -> Bool:
  match no:
    case True{}:
      False{}
    case False{}:
      F32.is_lt(
        RS.f_hypot(F32.sub(ga.f(hero, "x"), J.f32(J.nth(spot, 0))), F32.sub(ga.f(hero, "y"), F32.sub(J.f32(J.nth(spot, 1)), F32.mul(0.5, layout.TILE_UNITS())))),
        F32.mul(altar.REACH(), layout.TILE_UNITS()))

# --- the lamp's hooks from the host -------------------------------------------------------

# A lamp field that serde skips when empty: an empty array is left out.

def altar.Lamp.put?(+lamp: J.Val, +k: String, +items: List<&2, J.Val>) -> J.Val:
  altar.Lamp.put.in(U32.is_zero(ga.count(items)), lamp, k, items)

def altar.Lamp.put.in?(+empty: Bool, +lamp: J.Val, +k: String, +items: List<&2, J.Val>) -> J.Val:
  match empty:
    case True{}:
      J.del(lamp, k)
    case False{}:
      J.set(lamp, k, J.JA{items})

def altar.Lamp.has_words?(+ps: List<&2, J.Val>, +id: U32, +words: String) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+p, +t}:
      Bool.or(Bool.and(U32.is_eq(ga.u(p, "knight"), id), String.eq(ga.s(p, "words"), words)), altar.Lamp.has_words(t, id, words))

# The host handed knight `id`'s words to a grant runner: the lamp smokes until
# it lands or is let go.

def altar.Run.lamp_sent?(+run: J.Val, +id: U32, +words: String) -> J.Val:
  altar.Run.lamp_sent.in(altar.Lamp.has_words(J.items(J.get(J.get(run, "lamp"), "wishing")), id, words), run, id, words)

def altar.Run.lamp_sent.in?(+same: Bool, +run: J.Val, +id: U32, +words: String) -> J.Val:
  match same:
    case True{}:
      run
    case False{}:
      altar.Run.lamp_sent.push(J.items(J.get(J.get(run, "lamp"), "wishing")), run, id, words)

def altar.Run.lamp_sent.push?(+ws: List<&2, J.Val>, +run: J.Val, +id: U32, +words: String) -> J.Val:
  J.set(run, "lamp", altar.Lamp.put(J.get(run, "lamp"), "wishing",
    List.append(&2, J.Val, altar.Lamp.cap(ws, altar.MOST_PENDING()), [ga.sv(ga.ss(ga.su(J.obj(), "knight", id), "words", words), "since", J.get(run, "tick"))])))

# `if len >= most { remove(0) }`.

def altar.Lamp.cap?(+ws: List<&2, J.Val>, +most: U32) -> List<&2, J.Val>:
  altar.Lamp.cap.in(U32.is_ge(ga.count(ws), most), ws)

def altar.Lamp.cap.in?(+full: Bool, +ws: List<&2, J.Val>) -> List<&2, J.Val>:
  match full:
    case True{}:
      ga.remove_nth(ws, 0n)
    case False{}:
      ws

# Knight `id`'s wish in those words is no longer in flight (the first match).

def altar.Lamp.drop?(+ps: List<&2, J.Val>, +id: U32, +words: String) -> List<&2, J.Val>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      altar.Lamp.drop.at(Bool.and(U32.is_eq(ga.u(p, "knight"), id), String.eq(ga.s(p, "words"), words)), p, t, id, words)

def altar.Lamp.drop.at?(+hit: Bool, +p: J.Val, +t: List<&2, J.Val>, +id: U32, +words: String) -> List<&2, J.Val>:
  match hit:
    case True{}:
      t
    case False{}:
      Con{p, altar.Lamp.drop(t, id, words)}

def altar.Run.lamp_dropped?(+run: J.Val, +id: U32, +words: String) -> J.Val:
  J.set(run, "lamp", altar.Lamp.put(J.get(run, "lamp"), "wishing", altar.Lamp.drop(J.items(J.get(J.get(run, "lamp"), "wishing")), id, words)))

# Knight `id`'s wish in those words came back granted as `name`: the lamp
# flares and the knight is told, wherever they are.

def altar.Run.lamp_landed?(+run: J.Val, +id: U32, +words: String, +name: String) -> J.Val:
  altar.Run.lamp_landed.in(altar.Run.lamp_dropped(run, id, words), id, name)

def altar.Run.lamp_landed.in?(+run: J.Val, +id: U32, +name: String) -> J.Val:
  J.set(run, "lamp", altar.Lamp.put(J.get(run, "lamp"), "landed",
    List.append(&2, J.Val, altar.Lamp.cap(J.items(J.get(J.get(run, "lamp"), "landed")), altar.MOST_LANDED()),
      [ga.sv(ga.ss(ga.su(J.obj(), "knight", id), "name", name), "tick", J.get(run, "tick"))])))

# --- E at the lamp ---------------------------------------------------------------------------

# `held` is a BTreeSet<u32>: a sorted array of ids.

def altar.held.has?(+held: J.Val, +id: U32) -> Bool:
  altar.held.has.go(J.items(held), id)

def altar.held.has.go?(+xs: List<&2, J.Val>, +id: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+x, +t}:
      Bool.or(U32.is_eq(J.u32(x), id), altar.held.has.go(t, id))

def altar.held.insert?(+xs: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      [J.JU{id}]
    case Con{+x, +t}:
      altar.held.insert.at(U32.is_lt(id, J.u32(x)), x, t, id)

def altar.held.insert.at?(+before: Bool, +x: J.Val, +t: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match before:
    case True{}:
      Con{J.JU{id}, Con{x, t}}
    case False{}:
      Con{x, altar.held.insert(t, id)}

def altar.held.remove?(+xs: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+x, +t}:
      altar.held.remove.at(U32.is_eq(J.u32(x), id), x, t, id)

def altar.held.remove.at?(+same: Bool, +x: J.Val, +t: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match same:
    case True{}:
      t
    case False{}:
      Con{x, altar.held.remove(t, id)}

def altar.Run.set_held?(+run: J.Val, +held: List<&2, J.Val>) -> J.Val:
  J.set(run, "lamp", J.set(J.get(run, "lamp"), "held", J.JA{held}))

# The stone is stone: a knight standing in it is set on the nearest edge, and
# a walk into a corner slides round the end. One hero.

def altar.slide?(+hero: J.Val, +input: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  altar.slide.in(
    Bool.and(Bool.and(F32.is_gt(ga.f(hero, "x"), left), F32.is_lt(ga.f(hero, "x"), right)), Bool.and(F32.is_gt(ga.f(hero, "y"), top), F32.is_lt(ga.f(hero, "y"), foot))),
    hero, input, left, right, top, foot)

def altar.slide.in?(+inside: Bool, +hero: J.Val, +input: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  match inside:
    case False{}:
      hero
    case True{}:
      altar.slide.side(
        altar.slide.nearest(F32.sub(ga.f(hero, "x"), left), F32.sub(right, ga.f(hero, "x")), F32.sub(ga.f(hero, "y"), top), F32.sub(foot, ga.f(hero, "y"))),
        hero, RS.i_as_f32(ga.u(input, "move_x")), RS.i_as_f32(ga.u(input, "move_y")), left, right, top, foot)

# The side whose distance is least, the first of equals: 0 west, 1 east,
# 2 north, 3 south.

def altar.slide.nearest?(+d0: F32, +d1: F32, +d2: F32, +d3: F32) -> U32:
  altar.slide.nearest.keep(F32.is_gt(d0, d1), d0, d1, d2, d3)

def altar.slide.nearest.keep?(+gt: Bool, +d0: F32, +d1: F32, +d2: F32, +d3: F32) -> U32:
  altar.slide.nearest.two(ga.pu(gt, 1, 0), ga.pf(gt, d1, d0), d2, d3)

def altar.slide.nearest.two?(+side: U32, +best: F32, +d2: F32, +d3: F32) -> U32:
  altar.slide.nearest.three(ga.pu(F32.is_gt(best, d2), 2, side), ga.pf(F32.is_gt(best, d2), d2, best), d3)

def altar.slide.nearest.three?(+side: U32, +best: F32, +d3: F32) -> U32:
  ga.pu(F32.is_gt(best, d3), 3, side)

def altar.slide.side?(+side: U32, +hero: J.Val, +mx: F32, +my: F32, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  altar.slide.put(side,
    ga.pf(U32.is_lt(side, 2), my, mx),
    ga.pb(U32.is_eq(side, 0), F32.is_gt(mx, 0.0), ga.pb(U32.is_eq(side, 1), F32.is_lt(mx, 0.0), ga.pb(U32.is_eq(side, 2), F32.is_gt(my, 0.0), F32.is_lt(my, 0.0)))),
    ga.pf(U32.is_lt(side, 2), F32.sub(ga.f(hero, "y"), top), F32.sub(ga.f(hero, "x"), left)),
    ga.pf(U32.is_lt(side, 2), F32.sub(foot, ga.f(hero, "y")), F32.sub(right, ga.f(hero, "x"))),
    hero, left, right, top, foot)

def altar.slide.put?(+side: U32, +along: F32, +deeper: Bool, +span_near: F32, +span_far: F32, +hero: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  altar.slide.edge(side,
    ga.pf(F32.is_ne(along, 0.0), F32.mul(RS.f_signum(along), altar.EDGE_SLIDE()),
      ga.pf(Bool.and(deeper, F32.is_lt(RS.f_min(span_near, span_far), altar.CORNER())),
        ga.pf(F32.is_lt(span_near, span_far), F32.neg(altar.EDGE_SLIDE()), altar.EDGE_SLIDE()), 0.0)),
    hero, left, right, top, foot)

def altar.slide.edge?(+side: U32, +slide: F32, +hero: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  altar.slide.along(side, slide,
    ga.sf(ga.sf(hero, "x", ga.pf(U32.is_eq(side, 0), left, ga.pf(U32.is_eq(side, 1), right, ga.f(hero, "x")))),
      "y", ga.pf(U32.is_eq(side, 2), top, ga.pf(U32.is_eq(side, 3), foot, ga.f(hero, "y")))))

def altar.slide.along?(+side: U32, +slide: F32, +hero: J.Val) -> J.Val:
  altar.slide.along.in(U32.is_lt(side, 2), slide, hero)

def altar.slide.along.in?(+vertical: Bool, +slide: F32, +hero: J.Val) -> J.Val:
  match vertical:
    case True{}:
      ga.sf(hero, "y", F32.add(ga.f(hero, "y"), slide))
    case False{}:
      ga.sf(hero, "x", F32.add(ga.f(hero, "x"), slide))

# Every knight standing in the stone, set on its edge.

def altar.slides?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +inputs: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  match hs:
    case Nil{}:
      players
    case Con{(+k, +h), +t}:
      altar.slides(t, J.set(players, k, altar.slide(h, altar.input_of(J.get(inputs, k)), left, right, top, foot)), inputs, left, right, top, foot)

# An absent seat's keys are `Input::default()`.

def altar.input_of?(+i: J.Val) -> J.Val:
  ga.pv(J.is_null(i), rules.Input.default(), i)

# Who rubs: the knights newly pressing E in reach of the lamp, in seat order.
# The run (with `held` updated) and the rubbers.

def altar.rubs?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  match hs:
    case Nil{}:
      (run, List.reverse(&2, U32, acc))
    case Con{(+k, +h), +t}:
      altar.rubs.id(Maybe.default(&2, U32, U32.read(k), 0), h, t, run, inputs, acc)

def altar.rubs.id?(+id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  altar.rubs.down(Bool.and(rules.Input.valid(J.get(inputs, U32.show(id))), ga.b(J.get(inputs, U32.show(id)), "interact")), id, h, t, run, inputs, acc)

def altar.rubs.down?(+down: Bool, +id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  match down:
    case False{}:
      altar.rubs(t, altar.Run.set_held(run, altar.held.remove(J.items(J.get(J.get(run, "lamp"), "held")), id)), inputs, acc)
    case True{}:
      altar.rubs.live(Bool.or(U32.is_zero(ga.u(h, "hp")), ga.b(h, "absent")), id, t, run, inputs, acc)

def altar.rubs.live?(+dead: Bool, +id: U32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  match dead:
    case True{}:
      altar.rubs(t, run, inputs, acc)
    case False{}:
      altar.rubs.fresh(altar.held.has(J.get(J.get(run, "lamp"), "held"), id), id, t, run, inputs, acc)

def altar.rubs.fresh?(+held: Bool, +id: U32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  match held:
    case True{}:
      altar.rubs(t, run, inputs, acc)
    case False{}:
      altar.rubs.near(altar.Run.near_altar(run, id), id, t, altar.Run.set_held(run, altar.held.insert(J.items(J.get(J.get(run, "lamp"), "held")), id)), inputs, acc)

def altar.rubs.near?(+near: Bool, +id: U32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  match near:
    case True{}:
      altar.rubs(t, run, inputs, Con{id, acc})
    case False{}:
      altar.rubs(t, run, inputs, acc)

# Each rub: the lamp rubbed, or the knight told they hold no wish.

def altar.rub?(+run: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      run
    case Con{+id, +t}:
      altar.rub(altar.rub.one(J.is_null(J.get(ga.at(J.get(run, "players"), id), "offer")), run, id), t)

def altar.rub.one?(+open: Bool, +run: J.Val, +id: U32) -> J.Val:
  match open:
    case False{}:
      run
    case True{}:
      altar.rub.wish(overclass.Run.can_wish(run, id), run, id)

def altar.rub.wish?(+can: Bool, +run: J.Val, +id: U32) -> J.Val:
  match can:
    case True{}:
      ga.push(J.set(run, "lamp", J.set(J.get(run, "lamp"), "rubbed", J.set(ga.pv(J.has(J.get(run, "lamp"), "rubbed"), J.get(J.get(run, "lamp"), "rubbed"), J.obj()), U32.show(id), J.get(run, "tick")))), "sounds", J.JS{"spell_cast"})
    case False{}:
      J.set(run, "found", J.JA{[J.get(run, "tick"), J.JU{id}, J.JS{"NO WISH HELD: EACH FLOOR BRINGS TWO"}]})

def altar.Run.tick_wish_altar.stone?(+run: J.Val, +spot: J.Val, +inputs: J.Val) -> J.Val:
  altar.Run.tick_wish_altar.rubs(
    altar.Run.tick_wish_altar.slid(run, inputs,
      F32.sub(J.f32(J.nth(spot, 0)), F32.mul(1.5, layout.TILE_UNITS())), F32.add(J.f32(J.nth(spot, 0)), F32.mul(1.5, layout.TILE_UNITS())),
      F32.sub(J.f32(J.nth(spot, 1)), F32.mul(1.4, layout.TILE_UNITS())), F32.add(J.f32(J.nth(spot, 1)), 0.6)),
    inputs)

def altar.Run.tick_wish_altar.slid?(+run: J.Val, +inputs: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  J.set(run, "players", altar.slides(J.int_fields(J.get(run, "players")), J.get(run, "players"), inputs, left, right, top, foot))

def altar.Run.tick_wish_altar.rubs?(+run: J.Val, +inputs: J.Val) -> J.Val:
  altar.Run.tick_wish_altar.rubbed(altar.rubs(J.int_fields(J.get(run, "players")), run, inputs, Nil{}))

def altar.Run.tick_wish_altar.rubbed?(+r: J.Val & List<&2, U32>) -> J.Val:
  (run, ids) = r
  altar.rub(run, ids)

# E at the lamp: a knight who holds a wish rubs it, and their wish line opens;
# one who holds none is told so. `inputs` is the object of seat id -> Input.

def altar.Run.tick_wish_altar?(+run: J.Val, +inputs: J.Val) -> J.Val:
  altar.Run.tick_wish_altar.due(altar.Run.altar_due(run), run, inputs)

def altar.Run.tick_wish_altar.due?(+due: Bool, +run: J.Val, +inputs: J.Val) -> J.Val:
  match due:
    case True{}:
      altar.Run.tick_wish_altar.spot(altar.Run.choose_altars(run), inputs)
    case False{}:
      altar.Run.tick_wish_altar.spot(run, inputs)

def altar.Run.tick_wish_altar.spot?(+run: J.Val, +inputs: J.Val) -> J.Val:
  altar.Run.tick_wish_altar.have(altar.Run.wish_altar(run), run, inputs)

def altar.Run.tick_wish_altar.have?(+spot: J.Val, +run: J.Val, +inputs: J.Val) -> J.Val:
  altar.Run.tick_wish_altar.have.m(J.is_null(spot), spot, run, inputs)

def altar.Run.tick_wish_altar.have.m?(+flag: Bool, +spot: J.Val, +run: J.Val, +inputs: J.Val) -> J.Val:
  match flag:
    case True{}:
      altar.Run.set_held(run, Nil{})
    case False{}:
      altar.Run.tick_wish_altar.stone(run, spot, inputs)

def case.aerie.pigeon_at?(+input: J.Val) -> J.Val:
  cx.aerie.xy(aerie.pigeon_at(J.get(input, "room"), ga.u(input, "i"), W.of_j(J.get(input, "tick"))))

def cx.aerie.xy?(+p: F32 & F32) -> J.Val:
  (x, y) = p
  ga.pair_f(x, y)

def case.aerie.flying?(+input: J.Val) -> J.Val:
  cx.aerie.index(aerie.flying(ga.s(input, "kind"), ga.u(input, "stage"), ga.u(input, "papers"), J.get(input, "pigeons")))

def cx.aerie.index?(+i: U32) -> J.Val:
  ga.pv(U32.is_ge(i, 3), J.JNull{}, J.JU{i})

def case.aerie.progress?(+input: J.Val) -> J.Val:
  cx.aerie.pair(aerie.progress(J.get(input, "country"), J.get(input, "marks")))

def cx.aerie.pair?(+p: U32 & U32) -> J.Val:
  (a, b) = p
  J.JA{[J.JU{a}, J.JU{b}]}

def case.aerie.shape?(+input: J.Val) -> J.Val:
  aerie.shape(J.get(input, "room"))

def case.aerie.Pigeons.valid?(+input: J.Val) -> J.Val:
  J.JBool{aerie.Pigeons.valid(J.get(input, "pigeons"))}

def case.aerie.Run.tick_aerie?(+input: J.Val) -> J.Val:
  ga.pick(aerie.Run.tick_aerie(J.get(input, "run"), J.get(input, "inputs")), ["players", "pigeons", "marks", "found", "sounds", "projectiles"])

def case.altar.stone_blocks?(+input: J.Val) -> J.Val:
  J.JBool{altar.stone_blocks(ga.u(input, "c"), ga.u(input, "r"), ga.u(input, "tc"), ga.u(input, "tr"))}

def case.altar.keeps_an_altar?(+input: J.Val) -> J.Val:
  J.JBool{altar.keeps_an_altar(J.get(input, "floor"), ga.u(input, "index"), ga.b(input, "identity"))}

def case.altar.place_on?(+input: J.Val) -> J.Val:
  cx.altar.altar_at(altar.place_on(J.get(input, "floor"), ga.b(input, "identity")))

def cx.altar.altar_at?(+floor: J.Val) -> J.Val:
  ga.sv(J.obj(), "altar_at", ga.pv(J.has(floor, "altar_at"), J.get(floor, "altar_at"), J.obj()))

def case.altar.place?(+input: J.Val) -> J.Val:
  altar.place(J.get(input, "room"), J.get(input, "arrival"), J.get(input, "extra"))

def case.altar.spot?(+input: J.Val) -> J.Val:
  altar.spot(J.get(input, "room"), J.get(input, "arrival"))

def case.altar.stone_cuts_a_passage?(+input: J.Val) -> J.Val:
  J.JBool{altar.stone_cuts_a_passage(J.get(input, "room"), ga.u(input, "c"), ga.u(input, "r"))}

def case.altar.route_tiles?(+input: J.Val) -> J.Val:
  altar.route_tiles(J.get(input, "room"), J.get(input, "from"), J.get(input, "extra"))

def case.altar.stone_stands_on_a_route?(+input: J.Val) -> J.Val:
  J.JBool{altar.stone_stands_on_a_route(J.get(input, "on"), ga.u(input, "cols"), ga.u(input, "c"), ga.u(input, "r"))}

def case.altar.Run.altar_due?(+input: J.Val) -> J.Val:
  J.JBool{altar.Run.altar_due(J.get(input, "run"))}

def case.altar.Run.wish_altar?(+input: J.Val) -> J.Val:
  altar.Run.wish_altar(J.get(input, "run"))

def case.altar.Run.near_altar?(+input: J.Val) -> J.Val:
  J.JBool{altar.Run.near_altar(J.get(input, "run"), ga.u(input, "id"))}

def case.altar.Run.choose_altars?(+input: J.Val) -> J.Val:
  cx.altar.altar_at(J.get(altar.Run.choose_altars(J.get(input, "run")), "dungeon"))

def case.altar.Run.tick_wish_altar?(+input: J.Val) -> J.Val:
  ga.pick(altar.Run.tick_wish_altar(J.get(input, "run"), J.get(input, "inputs")), ["players", "lamp", "found", "sounds", "dungeon"])

def case.altar.Run.lamp_sent?(+input: J.Val) -> J.Val:
  ga.pick(altar.Run.lamp_sent(J.get(input, "run"), ga.u(input, "id"), ga.s(input, "words")), ["lamp"])

def case.altar.Run.lamp_landed?(+input: J.Val) -> J.Val:
  ga.pick(altar.Run.lamp_landed(J.get(input, "run"), ga.u(input, "id"), ga.s(input, "words"), ga.s(input, "name")), ["lamp"])

def case.altar.Run.lamp_dropped?(+input: J.Val) -> J.Val:
  ga.pick(altar.Run.lamp_dropped(J.get(input, "run"), ga.u(input, "id"), ga.s(input, "words")), ["lamp"])

def case.altar.Lamp.quiet?(+input: J.Val) -> J.Val:
  J.JBool{altar.Lamp.quiet(J.get(input, "lamp"))}

def case.altar.Lamp.flaring?(+input: J.Val) -> J.Val:
  altar.Lamp.flaring(J.get(input, "lamp"), W.of_j(J.get(input, "tick")))

def case.altar.Lamp.pending?(+input: J.Val) -> J.Val:
  altar.Lamp.pending(J.get(input, "lamp"), ga.u(input, "id"))

def cx.hunters.empty_deeds?() -> J.Val:
  ga.sv(ga.sv(ga.sv(ga.sv(ga.sv(J.obj(), "hooks", J.JA{[]}), "webs", J.JA{[]}), "rot", J.JA{[]}), "spawns", J.JA{[]}), "cues", J.JA{[]})

def case.hunters.act?(+input: J.Val) -> J.Val:
  cx.hunters.act.out(hunters.act(J.get(input, "enemy"), ga.xys(J.get(input, "heroes")),
    ga.grid(J.get(input, "room"), ga.b(input, "barred")), cx.hunters.empty_deeds()))

def cx.hunters.act.out?(r: J.Val & J.Val & Bool) -> J.Val:
  (e, d, handled) = r
  ga.sb(ga.sv(ga.sv(J.obj(), "enemy", e), "deeds", d), "handled", handled)

def case.hunters.Run.tick_hunters?(+input: J.Val) -> J.Val:
  ga.pick(hunters.Run.tick_hunters(J.get(input, "run")), ["hooks", "webs", "players", "room_hurt", "fx", "sounds", "cues", "feats"])

def case.hunters.Run.hunt?(+input: J.Val) -> J.Val:
  ga.pick(hunters.Run.hunt(J.get(input, "run"), J.get(input, "hooks"), J.get(input, "webs"), J.get(input, "rot")),
    ["hooks", "webs", "players", "room_hurt"])

def case.hunters.Run.brood_full?(+input: J.Val) -> J.Val:
  J.JBool{hunters.Run.brood_full(J.get(input, "run"))}

def case.hunters.Run.webbed?(+input: J.Val) -> J.Val:
  J.JBool{hunters.Run.webbed(J.get(input, "webs"), ga.f(input, "x"), ga.f(input, "y"))}

def case.ledge.walk?(+input: J.Val) -> J.Val:
  cx.ledge.walk.out(ledge.walk(J.get(input, "hero"), J.get(input, "input"),
    ga.grid(J.get(input, "room"), ga.b(input, "barred")), ga.f(input, "dx"), ga.b(input, "rolling")))

def cx.ledge.walk.out?(r: J.Val & Bool) -> J.Val:
  (hero, fell) = r
  ga.sb(ga.sv(J.obj(), "hero", hero), "fell", fell)

def case.ledge.fall?(+input: J.Val) -> J.Val:
  ledge.fall(J.get(input, "enemy"), ga.grid(J.get(input, "room"), ga.b(input, "barred")))

def case.ledge.stand_on?(+input: J.Val) -> J.Val:
  J.JF{ledge.stand_on(J.get(input, "room"), ga.f(input, "x"), ga.f(input, "from"))}

def case.ledge.Run.side_on?(+input: J.Val) -> J.Val:
  J.JBool{ledge.Run.side_on(J.get(input, "run"))}

def case.ledge.Run.arrive_side_on?(+input: J.Val) -> J.Val:
  ga.pick(ledge.Run.arrive_side_on(J.get(input, "run")), ["players"])

def case.ledge.Run.perch?(+input: J.Val) -> J.Val:
  cx.ledge.Run.perch.out(ledge.Run.perch(J.get(input, "run"), ga.b(input, "flies")))

def cx.ledge.Run.perch.out?(r: J.Val & (F32 & F32)) -> J.Val:
  (run, xy) = r
  cx.ledge.Run.perch.pack(J.get(run, "rng"), xy)

def cx.ledge.Run.perch.pack?(+rng: J.Val, xy: F32 & F32) -> J.Val:
  (x, y) = xy
  ga.sv(ga.sv(J.obj(), "rng", rng), "pos", ga.pair_f(x, y))

def hunters.HOOK_RANGE?() -> F32:
  15.0

def hunters.HOOK_SPEED?() -> F32:
  22.0

def hunters.REEL_SPEED?() -> F32:
  16.0

def hunters.HOOK_WINDUP?() -> U32:
  20

def hunters.HOOK_EVERY?() -> U32:
  120

def hunters.HOOK_DAMAGE?() -> U32:
  20

def hunters.ROT_REACH?() -> F32:
  2.6

def hunters.ROT_EVERY?() -> U32:
  8

def hunters.WEB_REACH?() -> F32:
  2.6

def hunters.WEB_LASTS?() -> U32:
  240

def hunters.WEB_PACE?() -> F32:
  0.6

def hunters.BROOD?() -> U32:
  6

# The nearest hero (first of equals) as a Maybe pair.

def hunters.act.nearest?(+hs: List<&2, Sigma<&2, &2, F32, _ => F32>>, +ex: F32, +ey: F32, +best: Maybe<&2, Sigma<&2, &2, F32, _ => F32>>) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match hs:
    case Nil{}:
      best
    case Con{(+hx, +hy), +t}:
      hunters.act.nearest(t, ex, ey, hunters.act.nearest.pick(best, hx, hy, ex, ey))

def hunters.act.nearest.pick?(b: Maybe<&2, Sigma<&2, &2, F32, _ => F32>>, +hx: F32, +hy: F32, +ex: F32, +ey: F32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match b:
    case None{}:
      Some{(hx, hy)}
    case Some{(+bx, +by)}:
      hunters.act.nearest.keep(
        F32.is_gt(RS.f_hypot(F32.sub(bx, ex), F32.sub(by, ey)), RS.f_hypot(F32.sub(hx, ex), F32.sub(hy, ey))),
        bx, by, hx, hy)

def hunters.act.nearest.keep?(replace: Bool, +bx: F32, +by: F32, +hx: F32, +hy: F32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match replace:
    case True{}:
      Some{(hx, hy)}
    case False{}:
      Some{(bx, by)}

# `walk`: the enemy slides toward `(dx, dy)` at `speed`.

def hunters.act.walk?(+enemy: J.Val, +g: J.Val, +dx: F32, +dy: F32, +speed: F32) -> J.Val:
  hunters.act.walk.put(enemy, rules.Grid.slide(J.get(g, "room"), ga.b(g, "barred"), (ga.f(enemy, "x"), ga.f(enemy, "y")),
    (F32.mul(F32.mul(dx, speed), rules.DT()), F32.mul(F32.mul(dy, speed), rules.DT())),
    monsters.Enemy.radius(enemy), 1))

def hunters.act.walk.put?(+enemy: J.Val, p: F32 & F32) -> J.Val:
  (x, y) = p
  ga.sf(ga.sf(enemy, "x", x), "y", y)

# One of the third company's steps. The enemy and the deeds after it, and false
# (nothing changed) for any other monster.

def hunters.act?(+enemy: J.Val, +heroes: List<&2, Sigma<&2, &2, F32, _ => F32>>, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  hunters.act.kind(ga.s(enemy, "kind"), enemy, heroes, g, deeds)

def hunters.act.kind?(+k: String, +enemy: J.Val, +heroes: List<&2, Sigma<&2, &2, F32, _ => F32>>, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  hunters.act.kind.is(Bool.or(Bool.or(String.eq(k, "flesher"), String.eq(k, "silkmother")), String.eq(k, "spiderling")), k, enemy, heroes, g, deeds)

def hunters.act.kind.is?(hunter: Bool, +k: String, +enemy: J.Val, +heroes: List<&2, Sigma<&2, &2, F32, _ => F32>>, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  match hunter:
    case False{}:
      (enemy, deeds, False{})
    case True{}:
      hunters.act.aim(hunters.act.nearest(heroes, ga.f(enemy, "x"), ga.f(enemy, "y"), None{}), k, enemy, g, deeds)

def hunters.act.aim?(t: Maybe<&2, Sigma<&2, &2, F32, _ => F32>>, +k: String, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  match t:
    case None{}:
      hunters.act.go(False{}, ga.f(enemy, "x"), ga.f(enemy, "y"), k, enemy, g, deeds)
    case Some{(+tx, +ty)}:
      hunters.act.go(True{}, tx, ty, k, enemy, g, deeds)

def hunters.act.go?(+has: Bool, +tx: F32, +ty: F32, +k: String, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  hunters.act.go2(has, tx, ty, rules.unit(F32.sub(tx, ga.f(enemy, "x")), F32.sub(ty, ga.f(enemy, "y"))),
    RS.f_hypot(F32.sub(tx, ga.f(enemy, "x")), F32.sub(ty, ga.f(enemy, "y"))), k,
    ga.su(enemy, "timer", (ga.u(enemy, "timer") + 1 : U32)), g, deeds)

def hunters.act.go2?(+has: Bool, +tx: F32, +ty: F32, c: F32 & F32, +dist: F32, +k: String, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  (cx, cy) = c
  hunters.act.run(has, tx, ty, cx, cy, dist, k, enemy, (ga.u(enemy, "timer") + (ga.u(enemy, "id") * 7 : U32) : U32), g, deeds)

def hunters.act.run?(+has: Bool, +tx: F32, +ty: F32, +cx: F32, +cy: F32, +dist: F32, +k: String, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val & Bool:
  hunters.act.out(
    hunters.act.run.kind(String.eq(k, "flesher"), String.eq(k, "silkmother"), has, tx, ty, cx, cy, dist, enemy, beat, g, deeds))

def hunters.act.out?(r: J.Val & J.Val) -> J.Val & J.Val & Bool:
  (e, d) = r
  (e, d, True{})

def hunters.act.run.kind?(flesher: Bool, silk: Bool, +has: Bool, +tx: F32, +ty: F32, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match flesher:
    case True{}:
      hunters.act.flesher(has, cx, cy, dist, enemy, beat, g, deeds)
    case False{}:
      hunters.act.run.silk(silk, has, tx, ty, cx, cy, dist, enemy, beat, g, deeds)

def hunters.act.run.silk?(silk: Bool, +has: Bool, +tx: F32, +ty: F32, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match silk:
    case True{}:
      hunters.act.silkmother(has, tx, ty, cx, cy, dist, enemy, beat, g, deeds)
    case False{}:
      hunters.act.spiderling(cx, cy, enemy, g, deeds)

# --- the Flesher -----------------------------------------------------------

def hunters.act.flesher?(+has: Bool, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.flesher.stage(has, cx, cy, dist, enemy,
    hunters.act.flesher.rot(U32.is_zero(U32.mod(beat, hunters.ROT_EVERY())), enemy, deeds), g)

def hunters.act.flesher.rot?(on: Bool, +enemy: J.Val, +deeds: J.Val) -> J.Val:
  match on:
    case True{}:
      ga.push(deeds, "rot", ga.pair_f(ga.f(enemy, "x"), ga.f(enemy, "y")))
    case False{}:
      deeds

def hunters.act.flesher.stage?(+has: Bool, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +deeds: J.Val, +g: J.Val) -> J.Val & J.Val:
  hunters.act.flesher.stage.at(ga.u(enemy, "stage"), has, cx, cy, dist, enemy, deeds, g)

def hunters.act.flesher.stage.at?(+stage: U32, +has: Bool, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +deeds: J.Val, +g: J.Val) -> J.Val & J.Val:
  hunters.act.flesher.stage.pick(U32.is_eq(stage, 0), U32.is_eq(stage, 1), has, cx, cy, dist, enemy, deeds, g)

def hunters.act.flesher.stage.pick?(zero: Bool, one: Bool, +has: Bool, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +deeds: J.Val, +g: J.Val) -> J.Val & J.Val:
  match zero:
    case True{}:
      hunters.act.flesher.shamble(has, cx, cy, dist, hunters.act.walk(enemy, g, cx, cy, 1.6), deeds)
    case False{}:
      hunters.act.flesher.stare(one, enemy, deeds)

# Shamble after the party; when one is in reach, stare.

def hunters.act.flesher.shamble?(+has: Bool, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.flesher.shamble.go(
    Bool.and(U32.is_ge(ga.u(enemy, "timer"), (hunters.HOOK_EVERY() + (U32.mod(ga.u(enemy, "id"), 4) * 9 : U32) : U32)),
      Bool.and(has, F32.is_lt(dist, F32.sub(hunters.HOOK_RANGE(), 1.0)))),
    cx, cy, enemy, deeds)

def hunters.act.flesher.shamble.go?(ready: Bool, +cx: F32, +cy: F32, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match ready:
    case True{}:
      (J.set(ga.su(ga.su(enemy, "stage", 1), "timer", 0), "dir", ga.pair_f(cx, cy)), deeds)
    case False{}:
      (enemy, deeds)

# The stare: his line shows on the floor; then the throw, then reeling in.

def hunters.act.flesher.stare?(one: Bool, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match one:
    case True{}:
      hunters.act.flesher.throw(U32.is_ge(ga.u(enemy, "timer"), hunters.HOOK_WINDUP()), enemy, deeds)
    case False{}:
      hunters.act.flesher.breath(U32.is_ge(ga.u(enemy, "timer"), (2 * rules.HZ() : U32)), enemy, deeds)

def hunters.act.flesher.throw?(go: Bool, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match go:
    case True{}:
      (ga.su(ga.su(enemy, "stage", 2), "timer", 0),
        ga.push(ga.push(deeds, "hooks",
          ga.trio(J.JU{ga.u(enemy, "id")}, J.JF{ga.f(enemy, "x")}, J.JA{[J.JF{ga.f(enemy, "y")}, J.get(enemy, "dir")]})),
          "cues", J.JS{"hook_thrown"}))
    case False{}:
      (enemy, deeds)

def hunters.act.flesher.breath?(done: Bool, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match done:
    case True{}:
      (ga.su(ga.su(enemy, "stage", 0), "timer", 0), deeds)
    case False{}:
      (enemy, deeds)

# --- the Silkmother --------------------------------------------------------

def hunters.act.silkmother?(+has: Bool, +tx: F32, +ty: F32, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.silkmother.way(F32.is_lt(dist, 8.0), F32.is_gt(dist, 13.0),
    ga.pf(U32.is_zero(U32.mod(ga.u(enemy, "id"), 2)), 1.0, F32.neg(1.0)),
    has, tx, ty, cx, cy, dist, enemy, beat, g, deeds)

def hunters.act.silkmother.way?(+near: Bool, +far: Bool, +side: F32, +has: Bool, +tx: F32, +ty: F32, +cx: F32, +cy: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.silkmother.step(
    ga.pf(near, F32.neg(cx), ga.pf(far, cx, F32.mul(F32.neg(cy), side))),
    ga.pf(near, F32.neg(cy), ga.pf(far, cy, F32.mul(cx, side))),
    has, tx, ty, dist, enemy, beat, g, deeds)

def hunters.act.silkmother.step?(+wx: F32, +wy: F32, +has: Bool, +tx: F32, +ty: F32, +dist: F32, +enemy: J.Val, +beat: U32, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.silkmother.web(has, tx, ty, dist, beat, hunters.act.walk(enemy, g, wx, wy, 2.4), deeds)

def hunters.act.silkmother.web?(+has: Bool, +tx: F32, +ty: F32, +dist: F32, +beat: U32, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.silkmother.brood(beat, enemy,
    hunters.act.silkmother.web.on(Bool.and(U32.is_eq(U32.mod(beat, 100), 50), Bool.and(has, F32.is_lt(dist, 15.0))), tx, ty, deeds))

def hunters.act.silkmother.web.on?(on: Bool, +tx: F32, +ty: F32, +deeds: J.Val) -> J.Val:
  match on:
    case True{}:
      ga.push(deeds, "webs", ga.pair_f(tx, ty))
    case False{}:
      deeds

def hunters.act.silkmother.brood?(+beat: U32, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.silkmother.brood.on(U32.is_eq(U32.mod(beat, 180), 90), enemy, deeds)

def hunters.act.silkmother.brood.on?(hatch: Bool, +enemy: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  match hatch:
    case True{}:
      (enemy, ga.push(hunters.act.silkmother.hatch(enemy, deeds), "cues", J.JS{"brood_hatch"}))
    case False{}:
      (enemy, deeds)

def hunters.act.silkmother.hatch?(+enemy: J.Val, +deeds: J.Val) -> J.Val:
  ga.push(ga.push(deeds, "spawns", hunters.act.silkmother.spawn(enemy, F32.neg(1.0))), "spawns", hunters.act.silkmother.spawn(enemy, 1.0))

def hunters.act.silkmother.spawn?(+enemy: J.Val, +side: F32) -> J.Val:
  J.JA{[J.JS{"spiderling"}, J.JF{F32.add(ga.f(enemy, "x"), F32.mul(side, 1.4))}, J.JF{F32.add(ga.f(enemy, "y"), 0.8)}, J.JU{0}]}

# --- the spiderling scuttles straight in, zig-zagging ------------------------

def hunters.act.spiderling?(+cx: F32, +cy: F32, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.spiderling.zig(F32.mul(F32.sin(F32.add(F32.mul(RS.u_as_f32(ga.u(enemy, "age")), 0.5), RS.u_as_f32(ga.u(enemy, "id")))), 0.6), cx, cy, enemy, g, deeds)

def hunters.act.spiderling.zig?(+zig: F32, +cx: F32, +cy: F32, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  hunters.act.spiderling.walk(rules.unit(F32.sub(cx, F32.mul(cy, zig)), F32.add(cy, F32.mul(cx, zig))), enemy, g, deeds)

def hunters.act.spiderling.walk?(u: F32 & F32, +enemy: J.Val, +g: J.Val, +deeds: J.Val) -> J.Val & J.Val:
  (ux, uy) = u
  (hunters.act.walk(enemy, g, ux, uy, 5.4), deeds)

# --- the Run's hooks -------------------------------------------------------

def hunters.Run.hunt.hooks?(+run: J.Val, +hs: List<&2, J.Val>) -> J.Val:
  match hs:
    case Nil{}:
      run
    case Con{+h, +t}:
      hunters.Run.hunt.hooks(hunters.Run.hunt.hook(U32.is_lt(J.len(J.get(run, "hooks")), 4), run, h), t)

def hunters.Run.hunt.hook?(room: Bool, +run: J.Val, +h: J.Val) -> J.Val:
  match room:
    case False{}:
      run
    case True{}:
      ga.push(run, "hooks", hunters.Hook.new(h))

# A fresh hook from a Deeds entry (by, x, y, dir).

def hunters.Hook.new?(+h: J.Val) -> J.Val:
  ga.sv(ga.sb(ga.sf(ga.sv(ga.sf(ga.sf(ga.su(J.obj(), "by", J.u32(J.nth(h, 0))), "x", J.f32(J.nth(h, 1))), "y", J.f32(J.nth(h, 2))), "dir", J.nth(h, 3)), "out", 0.0), "back", False{}), "caught", J.JNull{})

def hunters.Run.hunt.webs?(+run: J.Val, +ws: List<&2, J.Val>) -> J.Val:
  match ws:
    case Nil{}:
      run
    case Con{+w, +t}:
      hunters.Run.hunt.webs(hunters.Run.hunt.web(run, w), t)

def hunters.Run.hunt.web?(+run: J.Val, +w: J.Val) -> J.Val:
  hunters.Run.hunt.web.put(
    ga.sv(run, "webs", hunters.Run.hunt.web.cap(U32.is_ge(J.len(J.get(run, "webs")), 6), J.get(run, "webs"))), w)

def hunters.Run.hunt.web.cap?(full: Bool, +webs: J.Val) -> J.Val:
  match full:
    case True{}:
      J.JA{ga.remove_nth(J.items(webs), 0n)}
    case False{}:
      webs

def hunters.Run.hunt.web.put?(+run: J.Val, +w: J.Val) -> J.Val:
  ga.push(run, "webs", ga.su(ga.sf(ga.sf(J.obj(), "x", J.f32(J.nth(w, 0))), "y", J.f32(J.nth(w, 1))), "left", hunters.WEB_LASTS()))

# The rot: every targetable knight above one health within reach loses one.

def hunters.Run.hunt.rot?(+run: J.Val, +rs: List<&2, J.Val>) -> J.Val:
  match rs:
    case Nil{}:
      run
    case Con{+r, +t}:
      hunters.Run.hunt.rot(hunters.Run.hunt.rot.one(run, J.f32(J.nth(r, 0)), J.f32(J.nth(r, 1)), J.int_fields(J.get(run, "players")), J.get(run, "players")), t)

def hunters.Run.hunt.rot.one?(+run: J.Val, +x: F32, +y: F32, +hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val) -> J.Val:
  match hs:
    case Nil{}:
      J.set(run, "players", players)
    case Con{(+k, +h), +t}:
      hunters.Run.hunt.rot.hero(
        Bool.and(U32.is_gt(ga.u(h, "hp"), 1), peasant.Hero.targetable(h)),
        F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)), hunters.ROT_REACH()),
        run, x, y, t, players, k, h)

def hunters.Run.hunt.rot.hero?(+live: Bool, +near: Bool, +run: J.Val, +x: F32, +y: F32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +k: String, +h: J.Val) -> J.Val:
  hunters.Run.hunt.rot.hit(Bool.and(live, near), run, x, y, t, players, k, h)

def hunters.Run.hunt.rot.hit?(hit: Bool, +run: J.Val, +x: F32, +y: F32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +k: String, +h: J.Val) -> J.Val:
  match hit:
    case True{}:
      hunters.Run.hunt.rot.one(ga.sb(run, "room_hurt", True{}), x, y, t, J.set(players, k, ga.su(h, "hp", (ga.u(h, "hp") - 1 : U32))))
    case False{}:
      hunters.Run.hunt.rot.one(run, x, y, t, players)

# Hooks thrown, webs spun, the rot: make what the hunters did so.

def hunters.Run.hunt?(+run: J.Val, +hooks: J.Val, +webs: J.Val, +rot: J.Val) -> J.Val:
  hunters.Run.hunt.rot(hunters.Run.hunt.webs(hunters.Run.hunt.hooks(run, J.items(hooks)), J.items(webs)), J.items(rot))

# Webs fade: a tick off each, the spent ones go.

def hunters.Run.tick_hunters.webs?(+ws: List<&2, J.Val>) -> List<&2, J.Val>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      hunters.Run.tick_hunters.webs.keep(ga.ssub(ga.u(w, "left"), 1), w, hunters.Run.tick_hunters.webs(t))

def hunters.Run.tick_hunters.webs.keep?(+left: U32, +w: J.Val, rest: List<&2, J.Val>) -> List<&2, J.Val>:
  hunters.Run.tick_hunters.webs.keep.if(U32.is_zero(left), left, w, rest)

def hunters.Run.tick_hunters.webs.keep.if?(gone: Bool, +left: U32, +w: J.Val, rest: List<&2, J.Val>) -> List<&2, J.Val>:
  match gone:
    case True{}:
      rest
    case False{}:
      Con{ga.su(w, "left", left), rest}

# The Flesher a hook is his: where he stands and how big he is.

def hunters.Run.find_enemy?(+es: List<&2, J.Val>, +id: U32) -> Maybe<&2, Sigma<&2, &2, F32, _ => Sigma<&2, &2, F32, _ => F32>>>:
  match es:
    case Nil{}:
      None{}
    case Con{+e, +t}:
      hunters.Run.find_enemy.hit(Bool.and(U32.is_eq(ga.u(e, "id"), id), U32.is_gt(ga.u(e, "hp"), 0)), e, t, id)

def hunters.Run.find_enemy.hit?(hit: Bool, +e: J.Val, +t: List<&2, J.Val>, +id: U32) -> Maybe<&2, Sigma<&2, &2, F32, _ => Sigma<&2, &2, F32, _ => F32>>>:
  match hit:
    case True{}:
      Some{(ga.f(e, "x"), ga.f(e, "y"), monsters.Enemy.radius(e))}
    case False{}:
      hunters.Run.find_enemy(t, id)

# One hook: the run, and the hook if it stays up (none when it is done).

def hunters.Run.tick_hunters.one?(+run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.owner(hunters.Run.find_enemy(J.items(J.get(run, "enemies")), ga.u(hook, "by")), run, hook)

def hunters.Run.tick_hunters.owner?(m: Maybe<&2, Sigma<&2, &2, F32, _ => Sigma<&2, &2, F32, _ => F32>>>, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  match m:
    case None{}:
      (run, None{})
    case Some{(+bx, +by, +br)}:
      hunters.Run.tick_hunters.way(ga.b(hook, "back"), run, hook, bx, by, br)

def hunters.Run.tick_hunters.way?(back: Bool, +run: J.Val, +hook: J.Val, +bx: F32, +by: F32, +br: F32) -> J.Val & Maybe<&2, J.Val>:
  match back:
    case True{}:
      hunters.Run.tick_hunters.reel(run, hook, bx, by, br)
    case False{}:
      hunters.Run.tick_hunters.fly(run, hook)

# --- reeling in ---------------------------------------------------------------

def hunters.Run.tick_hunters.reel?(+run: J.Val, +hook: J.Val, +bx: F32, +by: F32, +br: F32) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.reel.dir(rules.unit(F32.sub(bx, ga.f(hook, "x")), F32.sub(by, ga.f(hook, "y"))), run, hook, bx, by, br)

def hunters.Run.tick_hunters.reel.dir?(u: F32 & F32, +run: J.Val, +hook: J.Val, +bx: F32, +by: F32, +br: F32) -> J.Val & Maybe<&2, J.Val>:
  (ux, uy) = u
  hunters.Run.tick_hunters.reel.next(F32.add(ga.f(hook, "x"), F32.mul(ux, F32.mul(hunters.REEL_SPEED(), rules.DT()))),
    F32.add(ga.f(hook, "y"), F32.mul(uy, F32.mul(hunters.REEL_SPEED(), rules.DT()))), run, hook, bx, by, br)

def hunters.Run.tick_hunters.reel.next?(+nx: F32, +ny: F32, +run: J.Val, +hook: J.Val, +bx: F32, +by: F32, +br: F32) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.reel.done(F32.is_lt(RS.f_hypot(F32.sub(bx, nx), F32.sub(by, ny)), F32.add(br, 1.0)), nx, ny, run, hook)

def hunters.Run.tick_hunters.reel.done?(+home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.reel.end(home, hunters.Run.tick_hunters.reel.caught(J.get(hook, "caught"), home, nx, ny, run, hook), nx, ny)

def hunters.Run.tick_hunters.reel.end?(+home: Bool, rh: J.Val & J.Val, +nx: F32, +ny: F32) -> J.Val & Maybe<&2, J.Val>:
  (run, hook) = rh
  hunters.Run.tick_hunters.reel.fin(home, run, ga.sf(ga.sf(hook, "x", nx), "y", ny))

def hunters.Run.tick_hunters.reel.fin?(home: Bool, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  match home:
    case True{}:
      (run, None{})
    case False{}:
      (run, Some{hook})

# A reeled knight stops at a barred door or a wall. The hook may go on.

def hunters.Run.tick_hunters.reel.caught?(+caught: J.Val, +home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  hunters.Run.tick_hunters.reel.caught.id(J.is_null(caught), J.u32(caught), home, nx, ny, run, hook)

def hunters.Run.tick_hunters.reel.caught.id?(none: Bool, +id: U32, +home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  match none:
    case True{}:
      (run, hook)
    case False{}:
      hunters.Run.tick_hunters.reel.hero(ga.at(J.get(run, "players"), id), id, home, nx, ny, run, hook)

def hunters.Run.tick_hunters.reel.hero?(+hero: J.Val, +id: U32, +home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  hunters.Run.tick_hunters.reel.hero.in(J.is_null(hero), hero, id, home, nx, ny, run, hook)

def hunters.Run.tick_hunters.reel.hero.in?(gone: Bool, +hero: J.Val, +id: U32, +home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  match gone:
    case True{}:
      (run, hook)
    case False{}:
      hunters.Run.tick_hunters.reel.halt(
        rules.Grid.halted(rules.Run.room(run), rules.Run.barred(run),
          (ga.f(hero, "x"), ga.f(hero, "y")), (F32.sub(nx, ga.f(hero, "x")), F32.sub(ny, ga.f(hero, "y")))),
        hero, id, home, nx, ny, run, hook)

def hunters.Run.tick_hunters.reel.halt?(stopped: F32 & F32, +hero: J.Val, +id: U32, +home: Bool, +nx: F32, +ny: F32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  (+sx, +sy) = stopped
  hunters.Run.tick_hunters.reel.land(F32.is_lt(RS.f_hypot(F32.sub(sx, nx), F32.sub(sy, ny)), 0.08), ga.sf(ga.sf(hero, "x", sx), "y", sy), id, home, run, hook)

def hunters.Run.tick_hunters.reel.land?(+followed: Bool, +hero: J.Val, +id: U32, +home: Bool, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  match followed:
    case True{}:
      hunters.Run.tick_hunters.reel.hit(home, hero, id, run, hook)
    case False{}:
      (ga.sv(run, "players", ga.put_at(J.get(run, "players"), id, hero)), ga.sv(hook, "caught", J.JNull{}))

# A ward after a hit holds against the hook too: no reel-in lands on a blow
# just taken.

def hunters.Run.tick_hunters.reel.hit?(home: Bool, +hero: J.Val, +id: U32, +run: J.Val, +hook: J.Val) -> J.Val & J.Val:
  match home:
    case False{}:
      (ga.sv(run, "players", ga.put_at(J.get(run, "players"), id, hero)), hook)
    case True{}:
      (ga.push(ga.push(ga.sb(ga.sv(run, "players", ga.put_at(J.get(run, "players"), id, hero.Hero.hurt(hero, hunters.HOOK_DAMAGE()))), "room_hurt", True{}),
        "fx", ga.su(J.obj(), "Shake", 6)), "sounds", J.JS{"hero_hurt"}), hook)

# --- flying out ---------------------------------------------------------------

def hunters.Run.tick_hunters.fly?(+run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.fly.next(F32.mul(hunters.HOOK_SPEED(), rules.DT()), run, hook)

def hunters.Run.tick_hunters.fly.next?(+step: F32, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.fly.tile(
    F32.add(ga.f(hook, "x"), F32.mul(ga.nf(J.get(hook, "dir"), 0), step)),
    F32.add(ga.f(hook, "y"), F32.mul(ga.nf(J.get(hook, "dir"), 1), step)), step, run, hook)

def hunters.Run.tick_hunters.fly.tile?(+nx: F32, +ny: F32, +step: F32, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.fly.stop(
    layout.Room.tile(rules.Run.room(run), RS.f_as_i32(F32.div(nx, layout.TILE_UNITS())), RS.f_as_i32(F32.div(ny, layout.TILE_UNITS()))),
    nx, ny, step, run, hook)

def hunters.Run.tick_hunters.fly.stop?(+tile: J.Val, +nx: F32, +ny: F32, +step: F32, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.fly.go(
    Bool.or(Bool.or(F32.is_ge(F32.add(ga.f(hook, "out"), step), hunters.HOOK_RANGE()), ga.stone(tile)),
      Bool.and(rules.Run.barred(run), ga.is(tile, "door"))),
    nx, ny, step, run, hook)

def hunters.Run.tick_hunters.fly.go?(blocked: Bool, +nx: F32, +ny: F32, +step: F32, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  match blocked:
    case True{}:
      hunters.Run.tick_hunters.fly.catch(run, ga.sb(hook, "back", True{}))
    case False{}:
      hunters.Run.tick_hunters.fly.catch(run, ga.sf(ga.sf(ga.sf(hook, "x", nx), "y", ny), "out", F32.add(ga.f(hook, "out"), step)))

# The first knight it touches, unless rolling clear.

def hunters.Run.tick_hunters.fly.catch?(+run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  hunters.Run.tick_hunters.fly.caught(hunters.Run.tick_hunters.fly.touch(J.int_fields(J.get(run, "players")), ga.f(hook, "x"), ga.f(hook, "y")), run, hook)

def hunters.Run.tick_hunters.fly.touch?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32) -> Maybe<&2, U32>:
  match hs:
    case Nil{}:
      None{}
    case Con{(+k, +h), +t}:
      hunters.Run.tick_hunters.fly.touch.hit(
        Bool.and(Bool.and(peasant.Hero.targetable(h), U32.is_zero(ga.u(h, "dash_ticks"))), Bool.and(U32.is_zero(ga.u(h, "immune")),
          F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)), 1.1))),
        k, t, x, y)

def hunters.Run.tick_hunters.fly.touch.hit?(hit: Bool, +k: String, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32) -> Maybe<&2, U32>:
  match hit:
    case True{}:
      Some{Maybe.default(&2, U32, U32.read(k), 0)}
    case False{}:
      hunters.Run.tick_hunters.fly.touch(t, x, y)

def hunters.Run.tick_hunters.fly.caught?(m: Maybe<&2, U32>, +run: J.Val, +hook: J.Val) -> J.Val & Maybe<&2, J.Val>:
  match m:
    case None{}:
      (run, Some{hook})
    case Some{+id}:
      (feats.Run.notice(ga.push(ga.push(run, "cues", J.JS{"hooked"}), "sounds", J.JS{"chest_open"}), "fresh_meat"),
        Some{ga.sb(ga.sv(hook, "caught", J.JU{id}), "back", True{})})

# --- the loop over the hooks --------------------------------------------------

def hunters.Run.tick_hunters.hooks?(+run: J.Val, +hs: List<&2, J.Val>, +kept: List<&2, J.Val>) -> J.Val:
  match hs:
    case Nil{}:
      J.set(run, "hooks", J.JA{List.reverse(&2, J.Val, kept)})
    case Con{+h, +t}:
      hunters.Run.tick_hunters.hooks.next(hunters.Run.tick_hunters.one(run, h), t, kept)

def hunters.Run.tick_hunters.hooks.next?(rm: J.Val & Maybe<&2, J.Val>, +t: List<&2, J.Val>, +kept: List<&2, J.Val>) -> J.Val:
  (run, m) = rm
  match m:
    case None{}:
      hunters.Run.tick_hunters.hooks(run, t, kept)
    case Some{+h}:
      hunters.Run.tick_hunters.hooks(run, t, Con{h, kept})

# Hooks fly and reel in; webs fade.

def hunters.Run.tick_hunters?(+run: J.Val) -> J.Val:
  hunters.Run.tick_hunters.hooks(
    J.set(run, "webs", J.JA{hunters.Run.tick_hunters.webs(J.items(J.get(run, "webs")))}),
    J.items(J.get(run, "hooks")), Nil{})

# Whether a knight standing at `(x, y)` is in a web.

def hunters.Run.webbed?(+webs: J.Val, +x: F32, +y: F32) -> Bool:
  hunters.Run.webbed.go(J.items(webs), x, y)

def hunters.Run.webbed.go?(+ws: List<&2, J.Val>, +x: F32, +y: F32) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case Con{+w, +t}:
      hunters.Run.webbed.hit(F32.is_lt(RS.f_hypot(F32.sub(ga.f(w, "x"), x), F32.sub(ga.f(w, "y"), y)), hunters.WEB_REACH()), t, x, y)

def hunters.Run.webbed.hit?(hit: Bool, +t: List<&2, J.Val>, +x: F32, +y: F32) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      hunters.Run.webbed.go(t, x, y)

# The spiderlings a room holds now, for the brood's cap.

def hunters.Run.brood_full?(+run: J.Val) -> Bool:
  U32.is_ge(hunters.Run.brood_full.count(J.items(J.get(run, "enemies")), 0), hunters.BROOD())

def hunters.Run.brood_full.count?(+es: List<&2, J.Val>, +n: U32) -> U32:
  match es:
    case Nil{}:
      n
    case Con{+e, +t}:
      hunters.Run.brood_full.count(t, ga.pu(Bool.and(ga.is(J.get(e, "kind"), "spiderling"), U32.is_gt(ga.u(e, "hp"), 0)), (n + 1 : U32), n))

def ledge.FOOT?() -> F32:
  1.0

def ledge.HALF_W?() -> F32:
  0.45

def ledge.GRAVITY?() -> F32:
  95.0

def ledge.JUMP?() -> F32:
  31.0

def ledge.AIR_JUMP?() -> F32:
  28.0

def ledge.FALL_MAX?() -> F32:
  40.0

def ledge.AIR_JUMPS?() -> U32:
  1

def ledge.DROP?() -> U32:
  8

def ledge.PIT_HURT?() -> U32:
  20

def ledge.WALKER_FALL?() -> F32:
  0.8

def ledge.Grid.tile_side?(+g: J.Val, +x: F32, +y: F32) -> J.Val:
  J.JS{layout.tile_name(rules.Grid.tile_at(J.get(g, "room"), x, y))}

# Stone to a body moving sideways or rising.

def ledge.Grid.solid.tile?(+t: J.Val, +barred: Bool) -> Bool:
  Bool.or(ga.stone(t), Bool.and(ga.is(t, "door"), barred))

def ledge.Grid.solid?(+g: J.Val, +x: F32, +y: F32) -> Bool:
  ledge.Grid.solid.tile(ledge.Grid.tile_side(g, x, y), ga.b(g, "barred"))

# One column of a body: the three heights `-FOOT + 0.05`, 0, `FOOT - 0.05`.

def ledge.Grid.body_col?(+g: J.Val, +x: F32, +y: F32) -> Bool:
  Bool.or(
    ledge.Grid.solid(g, x, F32.add(y, F32.add(F32.neg(ledge.FOOT()), 0.05))),
    Bool.or(ledge.Grid.solid(g, x, F32.add(y, 0.0)), ledge.Grid.solid(g, x, F32.add(y, F32.sub(ledge.FOOT(), 0.05)))))

# A body centred at `(x, y)` overlaps stone.

def ledge.Grid.body_blocked?(+g: J.Val, +x: F32, +y: F32) -> Bool:
  Bool.or(
    ledge.Grid.body_col(g, F32.add(x, F32.neg(ledge.HALF_W())), y),
    ledge.Grid.body_col(g, F32.add(x, ledge.HALF_W()), y))

# One foot of `landing`: the top it lands on.

def ledge.Grid.landing.foot?(+g: J.Val, +xx: F32, +top: F32, +thru: Bool) -> Maybe<&2, F32>:
  ledge.Grid.landing.tile(ledge.Grid.tile_side(g, xx, F32.add(top, 0.01)), top, thru)

def ledge.Grid.landing.tile?(+t: J.Val, +top: F32, +thru: Bool) -> Maybe<&2, F32>:
  ledge.Grid.landing.pick(Bool.or(ga.stone(t), Bool.and(ga.is(t, "ledge"), Bool.not(thru))), top)

def ledge.Grid.landing.pick?(hit: Bool, +top: F32) -> Maybe<&2, F32>:
  match hit:
    case True{}:
      Some{top}
    case False{}:
      None{}

def ledge.first?(m: Maybe<&2, F32>, n: Maybe<&2, F32>) -> Maybe<&2, F32>:
  match m:
    case Some{+v}:
      Some{v}
    case None{}:
      n

# Feet moving down from `from` to `to`: the top they land on, if one lies
# between. Planks hold only feet that come from above them.

def ledge.Grid.landing?(+g: J.Val, +x: F32, +from: F32, +to: F32, +thru: Bool) -> Maybe<&2, F32>:
  +top = F32.mul(F32.floor(F32.div(to, layout.TILE_UNITS())), layout.TILE_UNITS())
  ledge.Grid.landing.in(Bool.or(F32.is_gt(from, F32.add(top, 0.001)), F32.is_lt(to, top)), g, x, top, thru)

def ledge.Grid.landing.in?(none: Bool, +g: J.Val, +x: F32, +top: F32, +thru: Bool) -> Maybe<&2, F32>:
  match none:
    case True{}:
      None{}
    case False{}:
      ledge.first(
        ledge.Grid.landing.foot(g, F32.add(x, F32.neg(ledge.HALF_W())), top, thru),
        ledge.Grid.landing.foot(g, F32.add(x, ledge.HALF_W()), top, thru))

def ledge.is_some?(m: Maybe<&2, F32>) -> Bool:
  match m:
    case Some{v}:
      True{}
    case None{}:
      False{}

# Something to stand on right under feet at `feet`.

def ledge.Grid.underfoot?(+g: J.Val, +x: F32, +feet: F32, +thru: Bool) -> Bool:
  ledge.is_some(ledge.Grid.landing(g, x, feet, F32.add(feet, 0.02), thru))

def ledge.Grid.pit?(+g: J.Val, +x: F32, +feet: F32) -> Bool:
  ga.is(ledge.Grid.tile_side(g, x, F32.sub(feet, 0.01)), "hazard")

# --- one knight's step side-on --------------------------------------------

# The jump key as last seen, and a jump press: the hero with `vy`, `grounded`
# and `air_jumps` as the press leaves them.

def ledge.walk.jump?(+hero: J.Val, +jump: Bool, +rolling: Bool) -> J.Val:
  ledge.walk.jump.in(Bool.and(jump, Bool.and(Bool.not(ga.b(hero, "jump_held")), Bool.not(rolling))), hero)

def ledge.walk.jump.in?(press: Bool, +hero: J.Val) -> J.Val:
  match press:
    case False{}:
      hero
    case True{}:
      ledge.walk.jump.air(ga.b(hero, "grounded"), hero)

def ledge.walk.jump.air?(grounded: Bool, +hero: J.Val) -> J.Val:
  match grounded:
    case True{}:
      ga.sb(ga.sf(hero, "vy", F32.neg(ledge.JUMP())), "grounded", False{})
    case False{}:
      ledge.walk.jump.double(U32.is_gt(ga.u(hero, "air_jumps"), 0), hero)

def ledge.walk.jump.double?(has: Bool, +hero: J.Val) -> J.Val:
  match has:
    case False{}:
      hero
    case True{}:
      ga.sf(ga.su(hero, "air_jumps", (ga.u(hero, "air_jumps") - 1 : U32)), "vy", F32.neg(ledge.AIR_JUMP()))

# Down on the move axis while standing on a plank (not stone) drops through it.

def ledge.walk.drop?(drop: Bool, +hero: J.Val) -> J.Val:
  match drop:
    case False{}:
      hero
    case True{}:
      ga.sb(ga.su(hero, "dropping", ledge.DROP()), "grounded", False{})

# `self.vy`: the roll runs flat; otherwise gravity, capped.

def ledge.walk.gravity?(+hero: J.Val, +rolling: Bool) -> J.Val:
  match rolling:
    case True{}:
      ga.sf(hero, "vy", 0.0)
    case False{}:
      ga.sf(hero, "vy", RS.f_min(F32.add(ga.f(hero, "vy"), F32.mul(ledge.GRAVITY(), rules.DT())), ledge.FALL_MAX()))

# Falling: a landing sets the knight on the top, none lets them drop.

def ledge.walk.down?(m: Maybe<&2, F32>, +hero: J.Val, +dy: F32) -> J.Val:
  match m:
    case Some{+top}:
      ga.su(ga.sb(ga.sf(ga.sf(hero, "y", F32.sub(top, ledge.FOOT())), "vy", 0.0), "grounded", True{}), "air_jumps", ledge.AIR_JUMPS())
    case None{}:
      ga.sb(ga.sf(hero, "y", F32.add(ga.f(hero, "y"), dy)), "grounded", False{})

# Rising: a bonk when the head meets stone, otherwise the rise.

def ledge.walk.up?(+g: J.Val, +hero: J.Val, +dy: F32) -> J.Val:
  ledge.walk.up.in(
    Bool.or(
      ledge.Grid.solid(g, F32.add(ga.f(hero, "x"), F32.neg(ledge.HALF_W())), F32.add(F32.sub(ga.f(hero, "y"), ledge.FOOT()), dy)),
      ledge.Grid.solid(g, F32.add(ga.f(hero, "x"), ledge.HALF_W()), F32.add(F32.sub(ga.f(hero, "y"), ledge.FOOT()), dy))),
    hero, dy)

def ledge.walk.up.in?(bonk: Bool, +hero: J.Val, +dy: F32) -> J.Val:
  match bonk:
    case True{}:
      ga.sb(ga.sf(ga.sf(hero, "y",
        F32.add(F32.mul(F32.add(F32.floor(F32.div(F32.add(F32.sub(ga.f(hero, "y"), ledge.FOOT()), dy), layout.TILE_UNITS())), 1.0), layout.TILE_UNITS()), ledge.FOOT())),
        "vy", 0.0), "grounded", False{})
    case False{}:
      ga.sb(ga.sf(hero, "y", F32.add(ga.f(hero, "y"), dy)), "grounded", False{})

# Not moving vertically: grounded is whether something is underfoot.

def ledge.walk.rest?(+g: J.Val, +hero: J.Val, +feet: F32) -> J.Val:
  ledge.walk.rest.in(ledge.Grid.underfoot(g, ga.f(hero, "x"), feet, U32.is_gt(ga.u(hero, "dropping"), 0)), hero)

def ledge.walk.rest.in?(g: Bool, +hero: J.Val) -> J.Val:
  match g:
    case True{}:
      ga.su(ga.sb(hero, "grounded", True{}), "air_jumps", ledge.AIR_JUMPS())
    case False{}:
      ga.sb(hero, "grounded", False{})

# The vertical move by the sign of `dy`.

def ledge.walk.vertical?(+g: J.Val, +hero: J.Val) -> J.Val:
  ledge.walk.vertical.in(g, hero, F32.mul(ga.f(hero, "vy"), rules.DT()), F32.add(ga.f(hero, "y"), ledge.FOOT()))

def ledge.walk.vertical.in?(+g: J.Val, +hero: J.Val, +dy: F32, +feet: F32) -> J.Val:
  ledge.walk.vertical.pick(F32.is_gt(dy, 0.0), F32.is_lt(dy, 0.0), g, hero, dy, feet)

def ledge.walk.vertical.pick?(pos: Bool, neg: Bool, +g: J.Val, +hero: J.Val, +dy: F32, +feet: F32) -> J.Val:
  match pos:
    case True{}:
      ledge.walk.down(ledge.Grid.landing(g, ga.f(hero, "x"), feet, F32.add(feet, dy), U32.is_gt(ga.u(hero, "dropping"), 0)), hero, dy)
    case False{}:
      ledge.walk.vertical.neg(neg, g, hero, dy, feet)

def ledge.walk.vertical.neg?(neg: Bool, +g: J.Val, +hero: J.Val, +dy: F32, +feet: F32) -> J.Val:
  match neg:
    case True{}:
      ledge.walk.up(g, hero, dy)
    case False{}:
      ledge.walk.rest(g, hero, feet)

# Fell in a pit: the bite (unless graced), back on the last firm footing.

def ledge.walk.pit?(+hero: J.Val) -> J.Val:
  +h1 = ga.su(hero, "hp", ga.pu(U32.is_zero(ga.u(hero, "invulnerable")), ga.ssub(ga.u(hero, "hp"), ledge.PIT_HURT()), ga.u(hero, "hp")))
  +h2 = ledge.walk.pit.back(J.get(h1, "footing"), h1)
  ga.su(ga.su(ga.sf(h2, "vy", 0.0), "dash_ticks", 0), "invulnerable", U32.max(ga.u(h2, "invulnerable"), rules.HZ()))

def ledge.walk.pit.back?(+footing: J.Val, +hero: J.Val) -> J.Val:
  ledge.walk.pit.back.in(J.is_null(footing), footing, hero)

def ledge.walk.pit.back.in?(none: Bool, +footing: J.Val, +hero: J.Val) -> J.Val:
  match none:
    case True{}:
      hero
    case False{}:
      ga.sf(ga.sf(hero, "x", ga.nf(footing, 0)), "y", ga.nf(footing, 1))

# Firm footing: both feet on something, for a pit to put you back on.

def ledge.walk.footing?(+g: J.Val, +hero: J.Val, +feet: F32) -> J.Val:
  ledge.walk.footing.in(
    Bool.and(ga.b(hero, "grounded"),
      Bool.and(
        ledge.Grid.underfoot(g, F32.add(ga.f(hero, "x"), F32.mul(F32.neg(ledge.HALF_W()), 2.0)), feet, False{}),
        ledge.Grid.underfoot(g, F32.add(ga.f(hero, "x"), F32.mul(ledge.HALF_W(), 2.0)), feet, False{}))),
    hero)

def ledge.walk.footing.in?(firm: Bool, +hero: J.Val) -> J.Val:
  match firm:
    case True{}:
      J.set(hero, "footing", ga.pair_f(ga.f(hero, "x"), ga.f(hero, "y")))
    case False{}:
      hero

# One knight's step side-on: `dx` across (running or rolling), then the jump,
# the fall and the landing. The knight, and whether they fell in a pit.

def ledge.walk?(+hero: J.Val, +input: J.Val, +g: J.Val, +dx: F32, +rolling: Bool) -> J.Val & Bool:
  +jump = RS.i_lt(ga.u(input, "move_y"), 0)
  +h1 = ga.sb(ledge.walk.jump(hero, jump, rolling), "jump_held", jump)
  +h2 = ga.su(h1, "dropping", ga.ssub(ga.u(h1, "dropping"), 1))
  +h3 = ledge.walk.drop(
    Bool.and(RS.i_gt(ga.u(input, "move_y"), 0),
      Bool.and(ga.b(h2, "grounded"), Bool.not(ledge.Grid.underfoot(g, ga.f(h2, "x"), F32.add(ga.f(h2, "y"), ledge.FOOT()), True{})))),
    h2)
  +h4 = ledge.walk.sideways(ledge.Grid.body_blocked(g, F32.add(ga.f(h3, "x"), dx), ga.f(h3, "y")), h3, dx)
  +h5 = ledge.walk.vertical(g, ledge.walk.gravity(h4, rolling))
  +feet = F32.add(ga.f(h5, "y"), ledge.FOOT())
  ledge.walk.end(ledge.Grid.pit(g, ga.f(h5, "x"), feet), g, h5, feet)

def ledge.walk.sideways?(blocked: Bool, +hero: J.Val, +dx: F32) -> J.Val:
  match blocked:
    case True{}:
      hero
    case False{}:
      ga.sf(hero, "x", F32.add(ga.f(hero, "x"), dx))

def ledge.walk.end?(pit: Bool, +g: J.Val, +hero: J.Val, +feet: F32) -> J.Val & Bool:
  match pit:
    case True{}:
      (ledge.walk.pit(hero), True{})
    case False{}:
      (ledge.walk.footing(g, hero, feet), False{})

# A walker side-on: it keeps the ground it walks on, falls off what it walks
# past, and a pit is the end of it.

def ledge.fall?(+enemy: J.Val, +g: J.Val) -> J.Val:
  +feet = F32.add(ga.f(enemy, "y"), ledge.FOOT())
  +e1 = ledge.fall.land(ledge.Grid.landing(g, ga.f(enemy, "x"), feet, F32.add(feet, ledge.WALKER_FALL()), False{}), enemy)
  ledge.fall.pit(ledge.Grid.pit(g, ga.f(e1, "x"), F32.add(ga.f(e1, "y"), ledge.FOOT())), e1)

def ledge.fall.land?(m: Maybe<&2, F32>, +enemy: J.Val) -> J.Val:
  match m:
    case Some{+top}:
      ga.sf(enemy, "y", F32.sub(top, ledge.FOOT()))
    case None{}:
      ga.sf(enemy, "y", F32.add(ga.f(enemy, "y"), ledge.WALKER_FALL()))

def ledge.fall.pit?(pit: Bool, +enemy: J.Val) -> J.Val:
  match pit:
    case True{}:
      ga.su(enemy, "hp", 0)
    case False{}:
      enemy

# The ground under column `x`: the top of the highest stone or plank below
# `from`, as a body's middle standing on it.

def ledge.stand_on?(+room: J.Val, +x: F32, +from: F32) -> F32:
  ledge.stand_on.go(room, RS.f_as_i32(F32.floor(F32.div(x, layout.TILE_UNITS()))), RS.f_as_i32(F32.floor(F32.div(from, layout.TILE_UNITS()))), from, ga.u(room, "rows"))

def ledge.stand_on.go?(+room: J.Val, +col: U32, +row: U32, +from: F32, +rows: U32) -> F32:
  ledge.stand_on.row(RS.i_lt(row, rows), room, col, row, from, rows)

def ledge.stand_on.row?(inside: Bool, +room: J.Val, +col: U32, +row: U32, +from: F32, +rows: U32) -> F32:
  match inside:
    case False{}:
      from
    case True{}:
      ledge.stand_on.tile(
        Bool.and(Bool.or(ga.stone(layout.Room.tile(room, col, row)), ga.is(layout.Room.tile(room, col, row), "ledge")), RS.i_gt(row, 0)),
        room, col, row, from, rows)

def ledge.stand_on.tile?(hit: Bool, +room: J.Val, +col: U32, +row: U32, +from: F32, +rows: U32) -> F32:
  match hit:
    case True{}:
      F32.sub(F32.mul(RS.i_as_f32(row), layout.TILE_UNITS()), ledge.FOOT())
    case False{}:
      ledge.stand_on.go(room, col, RS.i_add(row, 1), from, rows)

# --- the Run's side-on hooks ------------------------------------------------

# Whether the party stands in a side-on room.

def ledge.Run.side_on?(+run: J.Val) -> Bool:
  Bool.or(ga.is(J.get(rules.Run.room(run), "kind"), "ledge"), ga.is(J.get(rules.Run.room(run), "kind"), "wanted"))

# The low west doorway's row: the first row whose tile in column 0 is a door,
# else the middle.

def ledge.Run.arrive_side_on.door?(+room: J.Val, +r: U32, +rows: U32) -> U32:
  ledge.Run.arrive_side_on.door.in(RS.i_lt(r, rows), room, r, rows)

def ledge.Run.arrive_side_on.door.in?(inside: Bool, +room: J.Val, +r: U32, +rows: U32) -> U32:
  match inside:
    case False{}:
      RS.i_div(rows, 2)
    case True{}:
      ledge.Run.arrive_side_on.door.hit(ga.is(layout.Room.tile(room, 0, r), "door"), room, r, rows)

def ledge.Run.arrive_side_on.door.hit?(hit: Bool, +room: J.Val, +r: U32, +rows: U32) -> U32:
  match hit:
    case True{}:
      r
    case False{}:
      ledge.Run.arrive_side_on.door(room, RS.i_add(r, 1), rows)

# Each knight, in seat order, at x = 3.3 + slot * 1.6 standing on the ground.

def ledge.Run.arrive_side_on.seat?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +room: J.Val, +from: F32, +slot: U32) -> J.Val:
  match hs:
    case Nil{}:
      players
    case Con{(+k, +hero), +t}:
      ledge.Run.arrive_side_on.seat(t, J.set(players, k, ledge.Run.arrive_side_on.hero(hero, room, from, slot)), room, from, (slot + 1 : U32))

def ledge.Run.arrive_side_on.hero?(+hero: J.Val, +room: J.Val, +from: F32, +slot: U32) -> J.Val:
  ledge.Run.arrive_side_on.hero.at(hero, F32.add(3.3, F32.mul(RS.u_as_f32(slot), 1.6)), room, from)

def ledge.Run.arrive_side_on.hero.at?(+hero: J.Val, +x: F32, +room: J.Val, +from: F32) -> J.Val:
  ledge.Run.arrive_side_on.hero.put(hero, x, ledge.stand_on(room, x, from))

def ledge.Run.arrive_side_on.hero.put?(+hero: J.Val, +x: F32, +y: F32) -> J.Val:
  +h1 = ga.sf(ga.sf(hero, "x", x), "y", y)
  +h2 = ga.su(ga.sb(ga.sf(h1, "vy", 0.0), "grounded", True{}), "air_jumps", ledge.AIR_JUMPS())
  ga.sf(ga.sf(J.set(h2, "footing", ga.pair_f(x, y)), "aim_x", 1.0), "aim_y", 0.0)

# Into a side-on hall: the party comes in at its low west doorway, on the
# ground, falling still.

def ledge.Run.arrive_side_on?(+run: J.Val) -> J.Val:
  ledge.Run.arrive_side_on.in(run, rules.Run.room(run))

def ledge.Run.arrive_side_on.in?(+run: J.Val, +room: J.Val) -> J.Val:
  ledge.Run.arrive_side_on.put(run, room, F32.mul(RS.i_as_f32(ledge.Run.arrive_side_on.door(room, 0, ga.u(room, "rows"))), layout.TILE_UNITS()))

def ledge.Run.arrive_side_on.put?(+run: J.Val, +room: J.Val, +from: F32) -> J.Val:
  J.set(run, "players", ledge.Run.arrive_side_on.seat(J.int_fields(J.get(run, "players")), J.get(run, "players"), room, from, 0))

# `self.rng.below(n)`: the run with the generator moved on, and the draw.

def ledge.Run.below?(+run: J.Val, +n: U32) -> J.Val & U32:
  ledge.Run.below.in(run, loot.Rng.below(W.of_j(J.get(run, "rng")), n))

def ledge.Run.below.in?(+run: J.Val, rv: W.W & U32) -> J.Val & U32:
  (r, v) = rv
  (J.set(run, "rng", W.to_j(r)), v)

# A monster's place side-on: walkers on a ledge or the ground well away from
# the door, fliers in the open air above.

def ledge.Run.perch?(+run: J.Val, +flies: Bool) -> J.Val & (F32 & F32):
  ledge.Run.perch.go(run, flies, rules.Run.room(run), 64n)

def ledge.Run.perch.go?(+run: J.Val, +flies: Bool, +room: J.Val, +n: Nat) -> J.Val & (F32 & F32):
  match n:
    case 0n:
      (run, (F32.div(F32.mul(RS.u_as_f32(ga.u(room, "cols")), layout.TILE_UNITS()), 2.0), F32.mul(layout.TILE_UNITS(), 3.0)))
    case 1n+p:
      ledge.Run.perch.col(ledge.Run.below(run, (ga.u(room, "cols") - 14 : U32)), flies, room, p)

def ledge.Run.perch.col?(rv: J.Val & U32, +flies: Bool, +room: J.Val, +p: Nat) -> J.Val & (F32 & F32):
  (run, v) = rv
  ledge.Run.perch.flier(flies, run, room, RS.i_add(12, v), p)

def ledge.Run.perch.flier?(+flies: Bool, +run: J.Val, +room: J.Val, +col: U32, +p: Nat) -> J.Val & (F32 & F32):
  match flies:
    case True{}:
      ledge.Run.perch.air(ledge.Run.below(run, (U32.div(ga.u(room, "rows"), 2) - 1 : U32)), room, col, p)
    case False{}:
      ledge.Run.perch.stand(ledge.Run.below(run, (ga.u(room, "rows") - 3 : U32)), room, col, p)

def ledge.Run.perch.x?(+col: U32) -> F32:
  F32.mul(F32.add(RS.i_as_f32(col), 0.5), layout.TILE_UNITS())

def ledge.Run.perch.air?(rv: J.Val & U32, +room: J.Val, +col: U32, +p: Nat) -> J.Val & (F32 & F32):
  (run, +v) = rv
  ledge.Run.perch.air.row(ga.is(layout.Room.tile(room, col, RS.i_add(2, v)), "floor"), run, room, col, RS.i_add(2, v), p)

def ledge.Run.perch.air.row?(open: Bool, +run: J.Val, +room: J.Val, +col: U32, +row: U32, +p: Nat) -> J.Val & (F32 & F32):
  match open:
    case True{}:
      (run, (ledge.Run.perch.x(col), F32.mul(F32.add(RS.i_as_f32(row), 0.5), layout.TILE_UNITS())))
    case False{}:
      ledge.Run.perch.go(run, True{}, room, p)

def ledge.Run.perch.stand?(rv: J.Val & U32, +room: J.Val, +col: U32, +p: Nat) -> J.Val & (F32 & F32):
  (run, v) = rv
  ledge.Run.perch.stand.at(run, room, col, p, ledge.Run.perch.x(col), F32.mul(RS.u_as_f32((1 + v : U32)), layout.TILE_UNITS()))

def ledge.Run.perch.stand.at?(+run: J.Val, +room: J.Val, +col: U32, +p: Nat, +x: F32, +start: F32) -> J.Val & (F32 & F32):
  ledge.Run.perch.stand.ground(run, room, col, p, x, ledge.stand_on(room, x, start))

def ledge.Run.perch.stand.ground?(+run: J.Val, +room: J.Val, +col: U32, +p: Nat, +x: F32, +y: F32) -> J.Val & (F32 & F32):
  ledge.Run.perch.stand.ok(
    Bool.and(
      ga.is(layout.Room.tile(room, col, RS.f_as_i32(F32.div(y, layout.TILE_UNITS()))), "floor"),
      Bool.or(
        ga.is(layout.Room.tile(room, col, RS.f_as_i32(F32.div(F32.add(y, ledge.FOOT()), layout.TILE_UNITS()))), "block"),
        ga.is(layout.Room.tile(room, col, RS.f_as_i32(F32.div(F32.add(y, ledge.FOOT()), layout.TILE_UNITS()))), "ledge"))),
    run, room, p, x, y)

def ledge.Run.perch.stand.ok?(ok: Bool, +run: J.Val, +room: J.Val, +p: Nat, +x: F32, +y: F32) -> J.Val & (F32 & F32):
  match ok:
    case True{}:
      (run, (x, y))
    case False{}:
      ledge.Run.perch.go(run, False{}, room, p)

def rules.Run.room?(+run: J.Val) -> J.Val:
  J.nth(J.get(J.get(run, "dungeon"), "rooms"), ga.u(run, "at"))

# The doors stay barred while anything in the room still lives (a stub: the
# fights, not the wave and mini-game cases).

def rules.Run.barred?(+run: J.Val) -> Bool:
  rules.Run.barred.live(J.items(J.get(run, "enemies")))

def rules.Run.barred.live?(+es: List<&2, J.Val>) -> Bool:
  match es:
    case Nil{}:
      False{}
    case Con{+e, +t}:
      Bool.or(Bool.not(ga.is(J.get(e, "kind"), "dummy")), rules.Run.barred.live(t))

# monsters.rs: `Enemy::radius` (the hunters' three; `size` overrides).

def monsters.Enemy.radius?(+e: J.Val) -> F32:
  monsters.Enemy.radius.size(J.get(e, "size"), ga.s(e, "kind"))

def monsters.Enemy.radius.size?(+size: J.Val, +kind: String) -> F32:
  monsters.Enemy.radius.if(J.is_null(size), size, kind)

def monsters.Enemy.radius.if?(none: Bool, +size: J.Val, +kind: String) -> F32:
  match none:
    case False{}:
      J.f32(size)
    case True{}:
      ga.pf(String.eq(kind, "flesher"), 1.15, ga.pf(String.eq(kind, "silkmother"), 1.0, 0.45))

# peasant.rs: `Hero::targetable`.

def peasant.Hero.targetable?(+h: J.Val) -> Bool:
  Bool.and(Bool.and(U32.is_gt(ga.u(h, "hp"), 0), Bool.not(ga.b(h, "stone"))), Bool.and(U32.is_zero(ga.u(h, "flee_ticks")), Bool.not(ga.b(h, "aloft"))))

# hero.rs: `Hero::hurt` (core owns the armour, curio and ward logic).

def hero.Hero.hurt?(+h: J.Val, +damage: U32) -> J.Val:
  h

# feats.rs: `Run::notice` (kept once, in order).

def feats.Run.notice?(+run: J.Val, +id: String) -> J.Val:
  feats.Run.notice.put(feats.Run.notice.has(J.items(J.get(run, "feats")), id), run, id)

def feats.Run.notice.has?(+fs: List<&2, J.Val>, +id: String) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      Bool.or(String.eq(J.str(f), id), feats.Run.notice.has(t, id))

def feats.Run.notice.put?(has: Bool, +run: J.Val, +id: String) -> J.Val:
  match has:
    case True{}:
      run
    case False{}:
      ga.push(run, "feats", J.JS{id})

# rules.rs: `Input::default()` and `Input::valid` (core).

def rules.Input.default?() -> J.Val:
  ga.su(ga.su(ga.sb(ga.sb(ga.sb(ga.sb(ga.su(ga.su(ga.su(ga.su(J.obj(), "move_x", 0), "move_y", 0), "aim_x", 0), "aim_y", 0), "fire", False{}), "dash", False{}), "bomb", False{}), "swing", False{}), "play", 0), "cast", 0)

def rules.Input.valid?(+i: J.Val) -> Bool:
  True{}

# overclass.rs: `Run::can_wish`.

def overclass.Run.can_wish?(+run: J.Val, +id: U32) -> Bool:
  overclass.Run.can_wish.of(ga.at(J.get(run, "players"), id))

def overclass.Run.can_wish.of?(+h: J.Val) -> Bool:
  Bool.and(U32.is_gt(ga.u(h, "hp"), 0), U32.is_gt(RS.as_u8((ga.u(h, "wishes") + ga.u(h, "great_wishes") : U32)), 0))

# rules.rs: `Run::party_len`.

def rules.Run.party_len?(+run: J.Val) -> U32:
  rules.Run.party_len.go(J.int_fields(J.get(run, "players")), 0)

def rules.Run.party_len.go?(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +n: U32) -> U32:
  match hs:
    case Nil{}:
      n
    case Con{(+k, +h), +t}:
      rules.Run.party_len.go(t, ga.pu(Bool.and(Bool.not(ga.b(h, "away")), Bool.not(ga.b(h, "absent"))), (n + 1 : U32), n))

def roundtable.number?(+tune: J.Val, +slot: U32, +i: U32) -> U32:
  J.u32(J.nth(J.nth(J.get(tune, "roundtable"), slot), i))

# crates/delve/src/rules/forest.rs `Forest::leave`: the hunt, the song and the pads let go.

def forest.Forest.leave?(+f: J.Val) -> J.Val:
  +f1 = ga.su(f, "hunt_left", 0)
  +f2 = ga.su(f1, "caught", 0)
  +f3 = ga.sb(f2, "singing", False{})
  +f4 = ga.su(f3, "show_left", 0)
  +f5 = ga.su(f4, "note", 0)
  ga.su(f5, "held", 0)

# crates/delve/src/rules/exits.rs `Run::leave_activity`: the knight steps out to the
# kingdom (the run, and whether it did). The stub leaves everything as it is.

def exits.Run.leave_activity?(+run: J.Val, +id: U32) -> Sigma<&2, &2, J.Val, _ => Bool>:
  (run, False{})

# crates/delve/src/realm.rs `Spoils::add`: `n` more of a spoil (the object keyed by its
# snake_case name), saturating.

def realm.Spoils.add?(+sp: J.Val, +spoil: String, +n: U32) -> J.Val:
  Bool.pick(J.Val, U32.is_zero(n), sp, J.set(sp, spoil, J.JU{ga.sadd32(ga.u(sp, spoil), n)}))

def rules.Input.axis_ok?(+v: U32) -> Bool:
  Bool.or(U32.is_le(v, 1), U32.is_eq(v, 4294967295))

def roundtable.number.of?(+v: J.Val, +i: U32) -> U32:
  match v:
    case J.JNull{}:
      Bool.pick(U32, U32.is_eq(i, 0), 112, Bool.pick(U32, U32.is_eq(i, 1), 8, Bool.pick(U32, U32.is_eq(i, 2), 14, Bool.pick(U32, U32.is_eq(i, 3), 150, 56))))
    case _:
      J.u32(v)

# Input::valid / Input::default (core).

def hitstop.State.request?(+hs: J.Val, +tune: J.Val, +impact: String) -> J.Val:
  hs

# exits: Run::leave_activity

def cases.run?(+name: String, +input: J.Val) -> J.Val:
  cases.run.n0(String.eq(name, "aerie.Pigeons.valid"), name, input)

def cases.run.n0?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.aerie.Pigeons.valid(input)
    case False{}:
      cases.run.n1(String.eq(name, "aerie.Run.tick_aerie"), name, input)

def cases.run.n1?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.aerie.Run.tick_aerie(input)
    case False{}:
      cases.run.n2(String.eq(name, "aerie.flying"), name, input)

def cases.run.n2?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.aerie.flying(input)
    case False{}:
      cases.run.n3(String.eq(name, "aerie.pigeon_at"), name, input)

def cases.run.n3?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.aerie.pigeon_at(input)
    case False{}:
      cases.run.n4(String.eq(name, "aerie.progress"), name, input)

def cases.run.n4?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.aerie.progress(input)
    case False{}:
      cases.run.n5(String.eq(name, "aerie.shape"), name, input)

def cases.run.n5?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.aerie.shape(input)
    case False{}:
      cases.run.n6(String.eq(name, "altar.Lamp.flaring"), name, input)

def cases.run.n6?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.Lamp.flaring(input)
    case False{}:
      cases.run.n7(String.eq(name, "altar.Lamp.pending"), name, input)

def cases.run.n7?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.Lamp.pending(input)
    case False{}:
      cases.run.n8(String.eq(name, "altar.Lamp.quiet"), name, input)

def cases.run.n8?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.Lamp.quiet(input)
    case False{}:
      cases.run.n9(String.eq(name, "altar.Run.altar_due"), name, input)

def cases.run.n9?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.Run.altar_due(input)
    case False{}:
      cases.run.n10(String.eq(name, "altar.Run.choose_altars"), name, input)

def cases.run.n10?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.Run.choose_altars(input)
    case False{}:
      cases.run.n11(String.eq(name, "altar.Run.lamp_dropped"), name, input)

def cases.run.n11?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.Run.lamp_dropped(input)
    case False{}:
      cases.run.n12(String.eq(name, "altar.Run.lamp_landed"), name, input)

def cases.run.n12?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.Run.lamp_landed(input)
    case False{}:
      cases.run.n13(String.eq(name, "altar.Run.lamp_sent"), name, input)

def cases.run.n13?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.Run.lamp_sent(input)
    case False{}:
      cases.run.n14(String.eq(name, "altar.Run.near_altar"), name, input)

def cases.run.n14?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.Run.near_altar(input)
    case False{}:
      cases.run.n15(String.eq(name, "altar.Run.tick_wish_altar"), name, input)

def cases.run.n15?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.Run.tick_wish_altar(input)
    case False{}:
      cases.run.n16(String.eq(name, "altar.Run.wish_altar"), name, input)

def cases.run.n16?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.Run.wish_altar(input)
    case False{}:
      cases.run.n17(String.eq(name, "altar.keeps_an_altar"), name, input)

def cases.run.n17?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.keeps_an_altar(input)
    case False{}:
      cases.run.n18(String.eq(name, "altar.place"), name, input)

def cases.run.n18?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.place(input)
    case False{}:
      cases.run.n19(String.eq(name, "altar.place_on"), name, input)

def cases.run.n19?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.place_on(input)
    case False{}:
      cases.run.n20(String.eq(name, "altar.route_tiles"), name, input)

def cases.run.n20?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.route_tiles(input)
    case False{}:
      cases.run.n21(String.eq(name, "altar.spot"), name, input)

def cases.run.n21?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.spot(input)
    case False{}:
      cases.run.n22(String.eq(name, "altar.stone_blocks"), name, input)

def cases.run.n22?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.stone_blocks(input)
    case False{}:
      cases.run.n23(String.eq(name, "altar.stone_cuts_a_passage"), name, input)

def cases.run.n23?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.stone_cuts_a_passage(input)
    case False{}:
      cases.run.n24(String.eq(name, "altar.stone_stands_on_a_route"), name, input)

def cases.run.n24?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.altar.stone_stands_on_a_route(input)
    case False{}:
      cases.run.n25(String.eq(name, "hunters.Run.brood_full"), name, input)

def cases.run.n25?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.hunters.Run.brood_full(input)
    case False{}:
      cases.run.n26(String.eq(name, "hunters.Run.hunt"), name, input)

def cases.run.n26?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.hunters.Run.hunt(input)
    case False{}:
      cases.run.n27(String.eq(name, "hunters.Run.tick_hunters"), name, input)

def cases.run.n27?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.hunters.Run.tick_hunters(input)
    case False{}:
      cases.run.n28(String.eq(name, "hunters.Run.webbed"), name, input)

def cases.run.n28?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.hunters.Run.webbed(input)
    case False{}:
      cases.run.n29(String.eq(name, "hunters.act"), name, input)

def cases.run.n29?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.hunters.act(input)
    case False{}:
      cases.run.n30(String.eq(name, "ledge.Run.arrive_side_on"), name, input)

def cases.run.n30?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.ledge.Run.arrive_side_on(input)
    case False{}:
      cases.run.n31(String.eq(name, "ledge.Run.perch"), name, input)

def cases.run.n31?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.ledge.Run.perch(input)
    case False{}:
      cases.run.n32(String.eq(name, "ledge.Run.side_on"), name, input)

def cases.run.n32?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.ledge.Run.side_on(input)
    case False{}:
      cases.run.n33(String.eq(name, "ledge.fall"), name, input)

def cases.run.n33?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.ledge.fall(input)
    case False{}:
      cases.run.n34(String.eq(name, "ledge.stand_on"), name, input)

def cases.run.n34?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.ledge.stand_on(input)
    case False{}:
      cases.run.n35(String.eq(name, "ledge.walk"), name, input)

def cases.run.n35?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.ledge.walk(input)
    case False{}:
      cases.run.n36(String.eq(name, "loot.Rng.below"), name, input)

def cases.run.n36?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.loot.Rng.below(input)
    case False{}:
      cases.run.n37(String.eq(name, "loot.Rng.chance"), name, input)

def cases.run.n37?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.loot.Rng.chance(input)
    case False{}:
      cases.run.n38(String.eq(name, "loot.Rng.new"), name, input)

def cases.run.n38?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.loot.Rng.new(input)
    case False{}:
      cases.run.n39(String.eq(name, "loot.Rng.next"), name, input)

def cases.run.n39?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.loot.Rng.next(input)
    case False{}:
      cases.run.n40(String.eq(name, "loot.Rng.pick"), name, input)

def cases.run.n40?(hit: Bool, +name: String, +input: J.Val) -> J.Val:
  match hit:
    case True{}:
      case.loot.Rng.pick(input)
    case False{}:
      J.JS{"no case wrapper"}
