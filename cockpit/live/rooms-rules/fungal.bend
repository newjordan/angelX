# The Fungal's hand-drawn rooms (see `rooms-rules`): quadrants,
# `.` floor, `#` a block, `~` a hazard pool. Each was authored beside
# its pack in `crates/delve/src/rules/rooms/` and moved here whole.
import Base
import ./types.bend as T

# One-screen rooms: quadrants of an 11 by 6 half.
def small() -> List<&2, T.Quad>:
  [
    # The Fairy Ring: a broken ring of mushrooms round a spore pool;
    # knights circle the ring and shoot across the spores.
    T.Quad{11, 6, [
        "...........",
        "..#.....#..",
        "...~~~.....",
        "...~~..#...",
        "..#....#...",
        "...........",
    ]},
    # The Spore Sump: a sunken sump of spores by the wall with stumps
    # scattered on the dry ground; approach from either side.
    T.Quad{11, 6, [
        "...........",
        ".#...~~....",
        ".....~~....",
        "..#..~~#...",
        ".##........",
        "...........",
    ]},
    # The Root Web: braided roots in short runs with wide gaps between
    # them; room to slip a whole party through at speed.
    T.Quad{11, 6, [
        "...........",
        ".##..##..#.",
        "...........",
        ".#...#.#...",
        "....~~.....",
        "...........",
    ]},
    # The Toadstool Nook: two giant toadstools crowd a corner over a
    # spore slick; the open floor beside them is where the fight goes.
    T.Quad{11, 6, [
        "....##.....",
        ".##.##.~~..",
        ".#.....~...",
        "....##.....",
        "...........",
        "...........",
    ]},
  ]

# Bigger rooms: quadrants of the sizes in the sizes op.
def great() -> List<&2, T.Quad>:
  [
    # The Spore Flats (16x9): open flats pocked with sumps and stumps;
    # a running fight with little cover and much dodging.
    T.Quad{16, 9, [
        "................",
        ".#....#....#....",
        "..~~~....~~~....",
        "..~~~....~~~....",
        ".#....#....#....",
        "....~~..~~......",
        "..##......##....",
        "................",
        "................",
    ]},
    # The Ring Garden (20x11): mushroom rings dotted round the chamber,
    # spore pools between them; weave the rings or hold the open rim.
    T.Quad{20, 11, [
        "....................",
        "...##........##.....",
        "..#....#....#....#..",
        "..###..........###..",
        "....~~~......~~~....",
        "......#......#......",
        "..#....#....#....#..",
        "...##........##.....",
        "....................",
        "....~~......~~......",
        "....................",
    ]},
    # The Bloom Meadow (19x11): blooming mushrooms in five-petal clusters
    # across a meadow of spore puddles; a bright, open shooting gallery.
    T.Quad{19, 11, [
        "...................",
        "...#....#....#.....",
        "..###..###..###....",
        "...................",
        "....~~~....~~~.....",
        "....~~~....~~~.....",
        "...................",
        "..###..###..###....",
        "...#....#....#.....",
        "...................",
        "...................",
    ]},
  ]
