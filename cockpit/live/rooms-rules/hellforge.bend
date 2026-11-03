# The Hellforge's hand-drawn rooms (see `rooms-rules`): quadrants,
# `.` floor, `#` a block, `~` a hazard pool. Each was authored beside
# its pack in `crates/delve/src/rules/rooms/` and moved here whole.
import Base
import ./types.bend as T

# One-screen rooms: quadrants of an 11 by 6 half.
def small() -> List<&2, T.Quad>:
  [
    # The Anvil Nook: an anvil and a slag block in a small yard; a tight two-cover scrap.
    T.Quad{11, 6, [
        ".###.......",
        ".####......",
        "....#......",
        "......##...",
        "...........",
        "...........",
    ]},
    # The Slag Heap: one cooling heap in the open; kite round it, dodge across the yard.
    T.Quad{11, 6, [
        "..####.....",
        "..####.....",
        "....##.....",
        "...........",
        "...........",
        "...........",
    ]},
    # The Lava Ford: a tongue of lava with a stone ford; shoot over it, walk the ford.
    T.Quad{11, 6, [
        "...........",
        ".~~~~.~~~..",
        ".~~.~.~~~..",
        "...........",
        "...........",
        "...........",
    ]},
    # The Forge Ring: a quenching ring open at both ends; step in, step out, never stay.
    T.Quad{11, 6, [
        "...........",
        ".###.##....",
        "..#..##....",
        ".....##....",
        "...........",
        "...........",
    ]},
  ]

# Bigger rooms: quadrants of the sizes in the sizes op.
def great() -> List<&2, T.Quad>:
  [
    # The Furnace Floor: furnace mouths banked along the wall and a lava moat below; heat between covers.
    T.Quad{15, 9, [
        "...............",
        ".##.##.##......",
        "....##.##......",
        "....~~~~~......",
        "....~.~~~......",
        "...............",
        "...............",
        "...............",
        "...............",
    ]},
    # The Lava Channel: a wide channel with three stone bridges and an anvil on the far side.
    T.Quad{16, 9, [
        "................",
        "................",
        "..~~~#~~~.~~~...",
        "..~~~.~~~.~~~...",
        "................",
        "......##........",
        "................",
        "................",
        "................",
    ]},
    # The Slag Yard: cooling heaps and a shifting lava pond; no lane stays safe.
    T.Quad{16, 9, [
        "................",
        "...###..........",
        "...###..#~~~....",
        "...###..#~~~....",
        "........~~~#....",
        "................",
        "................",
        "................",
        "................",
    ]},
    # The Great Anvil: the master's anvil at the centre and work blocks to the sides; circle the stem.
    T.Quad{19, 11, [
        "...................",
        "...................",
        "...####...##.......",
        "........####.......",
        "........####.......",
        "........####.......",
        "........##.........",
        "...####............",
        "...####...##.......",
        "...................",
        "...................",
    ]},
    # The Lava River Halls: a broad river with twin fords and open ends; hold the bridges.
    T.Quad{20, 11, [
        "....................",
        "....................",
        "..##....##....##....",
        "....................",
        ".~~~~..~~~~~.~~~~...",
        ".~~~~...~~~~.~~~~...",
        ".~~~~..~~~~~.~~~~...",
        "....................",
        "....................",
        "....................",
        "....................",
    ]},
    # The Furnace Hall: furnace banks along the wall, a moat before them, an anvil on the floor.
    T.Quad{20, 11, [
        "#...................",
        "###.##.##.##.##.....",
        "....................",
        "....................",
        "......~~~~~~........",
        "......~.~~~~........",
        "........####........",
        "........####........",
        "....................",
        "....................",
        "....................",
    ]},
    # The Great Forge Ring: a ring of anvils about a lava throat; the centre is the high ground.
    T.Quad{23, 13, [
        ".......................",
        ".......................",
        "..####.##.##.##........",
        ".......####............",
        ".......#~~~~~..........",
        ".......#~~~~~..........",
        ".........~~~~..........",
        "........####...........",
        "....##.##.##.##........",
        ".......................",
        ".......................",
        ".......................",
        ".......................",
    ]},
    # The Slag Flats: ridges of slag stepping across the flats and ponds between; a running fight.
    T.Quad{23, 13, [
        ".......................",
        ".......................",
        "..........##...........",
        "..............##.......",
        "....~~~................",
        "....~~~....##..........",
        "....###.......##.......",
        ".###~~~................",
        "####~~~................",
        "##.####................",
        "##.####................",
        ".......................",
        ".......................",
    ]},
  ]
