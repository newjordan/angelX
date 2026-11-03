# The Unknown's hand-drawn rooms (see `rooms-rules`): quadrants,
# `.` floor, `#` a block, `~` a hazard pool. Each was authored beside
# its pack in `crates/delve/src/rules/rooms/` and moved here whole.
import Base
import ./types.bend as T

# One-screen rooms: quadrants of an 11 by 6 half.
def small() -> List<&2, T.Quad>:
  [
    # The Antechamber of the Spiral: outer pillars of a spiral stand
    # detached, its centre gone; a calm ring before the descent begins.
    T.Quad{11, 6, [
        "...........",
        ".##.##.##..",
        "...........",
        "..#..#.....",
        ".##.##.....",
        "...........",
    ]},
    # The Void Puddle: a sliver of the void sits mid-floor; two shards
    # lean against the wall. Cross the line of fire or go around.
    T.Quad{11, 6, [
        "...........",
        "..#..~~..#.",
        ".....~~....",
        ".#...~~....",
        "..#........",
        "...........",
    ]},
    # The Tilted Slab: one great slab tips into a void sink; the floor
    # slopes to a fight under its shadow.
    T.Quad{11, 6, [
        "..###......",
        "..###......",
        "..###.~~...",
        ".#....~~...",
        ".#.........",
        "...........",
    ]},
    # The Hollow Twin: two hollow columns hum off-axis, void seeping at
    # their feet; nothing here is where you left it.
    T.Quad{11, 6, [
        ".#....#....",
        ".#....#....",
        ".##...##...",
        ".....~~....",
        ".....~~....",
        "...........",
    ]},
  ]

# Bigger rooms: quadrants of the sizes in the sizes op.
def great() -> List<&2, T.Quad>:
  [
    # The Descent of Nine (16x9): pillar pairs walk in a slow spiral toward
    # the middle; every ring is two tiles apart so the maze never closes.
    T.Quad{16, 9, [
        "................",
        "..#....#....#...",
        "..##..##..##..##",
        "...#........#...",
        "..##...~~...##..",
        "................",
        "....##..##......",
        "......~~........",
        "................",
    ]},
    # The Void Moat (20x11): an island floats in a void moat, reached by
    # stepping stones; the far bank circles all the way round.
    T.Quad{20, 11, [
        "....................",
        "....................",
        "..~~~~~~~~~~~~......",
        "..~~~~~~~~~~~~......",
        "..............##....",
        "..~~~~~..~~.........",
        "......#.~~..........",
        "..~~~~~~~~~~........",
        "..~~~~~~~~~~........",
        "....................",
        "....................",
    ]},
    # The Geometry of the Deep (23x13): shards of some older room — a
    # colonnade, a ring, a moat — meet and refuse to agree.
    T.Quad{23, 13, [
        ".......................",
        "..#...#...#...#...#....",
        "......#........#.......",
        ".####....####....####..",
        ".......................",
        "....~~~....~~~....~~...",
        ".......................",
        ".####....####....####..",
        ".......................",
        "..#...#...#...#...#....",
        "......#........#.......",
        ".......................",
        ".......................",
    ]},
    # The Thin Orbit (15x9): sparse single pillars circle a void bead;
    # open space rules, cover is a rumour you duck behind once.
    T.Quad{15, 9, [
        "...............",
        "..#...#..#..#..",
        ".....#....#....",
        "..#..........#.",
        "....~~....~~...",
        "..#.........#..",
        "....~~~........",
        ".....#....#....",
        "...............",
    ]},
    # The Regolith Field (17x9): angular shards lie where they fell in
    # twin ranks; the void pools in the seam between the ranks.
    T.Quad{17, 9, [
        ".................",
        "...#..........#..",
        ".##...##...##....",
        ".##...##...##....",
        ".................",
        "....~~~~~~~......",
        ".................",
        ".##...##...##....",
        ".................",
    ]},
  ]
