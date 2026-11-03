# The Crypt's hand-drawn rooms (see `rooms-rules`): quadrants,
# `.` floor, `#` a block, `~` a hazard pool. Each was authored beside
# its pack in `crates/delve/src/rules/rooms/` and moved here whole.
import Base
import ./types.bend as T

# One-screen rooms: quadrants of an 11 by 6 half.
def small() -> List<&2, T.Quad>:
  [
    # The Bone Chapel: twin nave pillars and a tomb pair by the altar; fight down the open aisle.
    T.Quad{11, 6, [
        "...........",
        ".##..##..#.",
        ".##..##..#.",
        "...........",
        "...........",
        "...........",
    ]},
    # The Slid Tombs: sarcophagi dragged out of line into a diagonal; slalom between them.
    T.Quad{11, 6, [
        "...........",
        ".##........",
        "....####...",
        "....####...",
        "...........",
        "...........",
    ]},
    # The Weeping Vault: a flooded vault in the corner, a pillar pair across it; shoot over the water.
    T.Quad{11, 6, [
        "#..........",
        "#.~~....#..",
        "#.~~.....#.",
        "...........",
        "...........",
        "...........",
    ]},
    # The Ossuary Wall: niched bones and loose stones in an open hall; a duel at range.
    T.Quad{11, 6, [
        "##.........",
        "####...###.",
        "...........",
        "......#....",
        "...........",
        "...........",
    ]},
  ]

# Bigger rooms: quadrants of the sizes in the sizes op.
def great() -> List<&2, T.Quad>:
  [
    # The Chapel Apse: a broken apse arch off the corner, its floor flooded; the dead wait in the curve.
    T.Quad{15, 9, [
        "...............",
        "..###.##.......",
        ".#........#....",
        ".#........#....",
        ".#..~.~~..#....",
        ".#..~~~~..#....",
        "..........#....",
        "...............",
        "...............",
    ]},
    # The Reliquary Aisle: three reliquaries a side down the aisle and a casket at the crossing; long lanes.
    T.Quad{16, 9, [
        "#...............",
        "####..##..##....",
        "####..##..##....",
        "................",
        ".......##.......",
        ".......##.......",
        "................",
        "................",
        "................",
    ]},
    # The Drowned Ossuary: two sunken tombs and sliding water; wade the dry diagonal.
    T.Quad{16, 9, [
        "................",
        "..~~~...##......",
        "..~~~...##......",
        ".......###......",
        "......~~~.......",
        "..##..~~~.......",
        "................",
        "................",
        "................",
    ]},
    # The Flooded Gallery: a moat straight across the gallery and shelf-butts beyond it; fight over the water.
    T.Quad{17, 9, [
        ".................",
        "..##.##.##.##....",
        ".....#...........",
        "....~~~~~~~......",
        "....~~.~~~~......",
        ".................",
        ".................",
        ".................",
        ".................",
    ]},
    # The Sepulchre Field: tombs set on the half-step so the ranks interleave; no straight lane through.
    T.Quad{19, 11, [
        "...................",
        "...................",
        ".##...##...##......",
        ".##...##...##...##.",
        ".##...##...##......",
        "...................",
        "......~~~....~~~...",
        "...................",
        ".##...........##...",
        "...................",
        "...................",
    ]},
    # The Processional: pillar pairs and casket butlers along the walk, pooled crossings between; circle the pillars.
    T.Quad{20, 11, [
        "....................",
        "..#....#....#.......",
        "..#....#....#.......",
        "....................",
        "...####..##.........",
        "..~~~##..~~~........",
        "..##.##..##.........",
        "..##..##....#.......",
        "....................",
        "....................",
        "....................",
    ]},
    # The Crypt Beneath the Crypt: older tombs on the half-step and a black channel below; mind the water.
    T.Quad{20, 11, [
        "....................",
        ".##...##...##.......",
        ".####.####...##.....",
        ".##...##...##.......",
        "....................",
        "......~.~~~.........",
        "......~~~~~.........",
        "....................",
        "....................",
        "....................",
        "....................",
    ]},
    # The Catacomb Shelves: shelf-racks stepping down the diagonal into a pooled undercroft; slide fight.
    T.Quad{23, 13, [
        "####...................",
        "####.###...............",
        "###.####...............",
        "###.##.##..............",
        "..........##...........",
        ".............##........",
        ".......................",
        ".....~~~~~.............",
        ".....~~~~~.............",
        ".....###...............",
        ".......................",
        ".......................",
        ".......................",
    ]},
    # The Bone Cathedral: a tomb rank, a broken reliquary screen and a high altar at the crossing.
    T.Quad{23, 13, [
        "####...................",
        "####...................",
        "####...##...##...####..",
        "####.............####..",
        "..............##.###...",
        ".......................",
        ".........###...........",
        ".........###...........",
        ".........###...........",
        ".......................",
        ".......................",
        ".......................",
        ".......................",
    ]},
  ]
