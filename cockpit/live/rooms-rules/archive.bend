# The Archive's hand-drawn rooms (see `rooms-rules`): quadrants,
# `.` floor, `#` a block, `~` a hazard pool. Each was authored beside
# its pack in `crates/delve/src/rules/rooms/` and moved here whole.
import Base
import ./types.bend as T

# One-screen rooms: quadrants of an 11 by 6 half.
def small() -> List<&2, T.Quad>:
  [
    # The Reading Alcove: shelf-butts and a reading desk in a small hall; corner the loremasters.
    T.Quad{11, 6, [
        "...........",
        ".###.###...",
        "......##...",
        "......##...",
        "...........",
        "...........",
    ]},
    # The Flooded Stacks: water in the corner seeping under the shelves; dry ground fights.
    T.Quad{11, 6, [
        "~~~........",
        "~~~..##....",
        ".....##....",
        "...........",
        "~~~...#....",
        "...........",
    ]},
    # The Map Table: one wide table with a clerk's stool; an open duel around it.
    T.Quad{11, 6, [
        "...........",
        "......##...",
        "....####...",
        "....####...",
        "...........",
        "...........",
    ]},
  ]

# Bigger rooms: quadrants of the sizes in the sizes op.
def great() -> List<&2, T.Quad>:
  [
    # The Cartographer's Hall: the great table under its lamp, stools at the corners; plan the route.
    T.Quad{15, 9, [
        "...............",
        "...............",
        "....##.##......",
        "#...##.##......",
        "###............",
        "###.........#..",
        "###............",
        "...............",
        "...............",
    ]},
    # The Sunken Reading Room: a flooded floor and desks hauled to the walls; shoot across the water.
    T.Quad{16, 9, [
        "................",
        ".####...........",
        ".####.......##..",
        "................",
        "......~~~~~.....",
        ".....~~~~~~.....",
        "................",
        "................",
        "................",
    ]},
    # The Drowned Stacks: water down one wall, shelves marching away from it; slip-gaps everywhere.
    T.Quad{17, 9, [
        ".................",
        "~~~##............",
        "~~~###..##..##...",
        "~~~#.............",
        ".....#...##......",
        ".................",
        ".................",
        ".................",
        ".................",
    ]},
    # The Flooded Aisle: water down the middle of the hall, shelves banked to either side.
    T.Quad{20, 11, [
        "....................",
        "....................",
        ".##...####...##.....",
        "......####..........",
        "......~~~~~~........",
        "......~.~~~~........",
        "........##..........",
        ".##....##....##.....",
        "....................",
        "....................",
        "....................",
    ]},
    # The Index Hall: catalogue desks in clumps, water in the corners; many small fights.
    T.Quad{20, 11, [
        "####................",
        "~~~#.....##.........",
        "~~~#.....##.........",
        "~~~#.....##.........",
        "#...................",
        ".....##....##.......",
        "....................",
        "........##......##..",
        "....................",
        "....................",
        "....................",
    ]},
    # The Scriptorium Deep: copyists' desks, an ink channel across, and water in the corner.
    T.Quad{23, 13, [
        ".......................",
        ".......................",
        "..##...###..###..##....",
        ".......###..###........",
        ".......~~~~~~~~........",
        ".......~~~~~~~~........",
        ".......................",
        "..##...##...##.........",
        ".......................",
        "~~.....................",
        "~~.....................",
        ".......................",
        ".......................",
    ]},
  ]
