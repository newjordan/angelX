# The Cavern's hand-drawn rooms (see `rooms-rules`): quadrants,
# `.` floor, `#` a block, `~` a hazard pool. Each was authored beside
# its pack in `crates/delve/src/rules/rooms/` and moved here whole.
import Base
import ./types.bend as T

# One-screen rooms: quadrants of an 11 by 6 half.
def small() -> List<&2, T.Quad>:
  [
    # The Boulder Rill: scattered boulder pairs around a sink pool, open
    # ground everywhere so a knight is never far from cover or a dodge.
    T.Quad{11, 6, [
        "...........",
        ".##....#...",
        ".....~~....",
        "...#.~~....",
        "..#..#.....",
        "...........",
    ]},
    # The Stalactite Garden: lone stalagmites with a shallow pool at the
    # foot, a dancing arena of single blocks and short hops.
    T.Quad{11, 6, [
        "...........",
        "...#....#..",
        "..~.#.~....",
        "..#....#...",
        "#....~~....",
        "...........",
    ]},
    # The Sunken Gallery: an S-shaped pool threading the middle, knights
    # shoot across it and circle either bank between rubble stubs.
    T.Quad{11, 6, [
        "...........",
        ".##.~~~....",
        "......~....",
        "....#.~~...",
        ".#.........",
        "...........",
    ]},
    # The Crystal Nook: a tight cluster of crystal spars by the wall with
    # a black pool at its foot; the rest is open floor for the fight.
    T.Quad{11, 6, [
        "......#....",
        "...#..#..#.",
        ".~~....#...",
        ".~~..#.....",
        "...........",
        "...........",
    ]},
  ]

# Bigger rooms: quadrants of the sizes in the sizes op.
def great() -> List<&2, T.Quad>:
  [
    # The Boulder Field (16x9): an open arena where two boulder clumps
    # flank a reed pool; long sight lines, hard cover to sprint between.
    T.Quad{16, 9, [
        "................",
        "..##....#.......",
        "....~~~.........",
        "..#.~~~....##...",
        "....~~~.........",
        "..#....##.......",
        "..~~~...........",
        "..#.............",
        "................",
    ]},
    # The Collapsed Gallery (16x9): fallen gallery columns lie in broken
    # rows; water gathers at their feet and knights fight down the aisles.
    T.Quad{16, 9, [
        "................",
        ".###...###..#...",
        "................",
        ".~~.#....#..~~..",
        "................",
        ".###...###..~~..",
        "................",
        "..#.....#.......",
        "................",
    ]},
    # The Underground Lake (20x11): a broad lake mid-room with two stone
    # islets; arrows fly over the water while the party skirts the shore.
    T.Quad{20, 11, [
        "....................",
        "....................",
        "....~~~~~~~~~~......",
        "....~~~~~~~~~~......",
        "....~~#~~#~~~~......",
        "....~~~~~~~~~~......",
        "....................",
        ".......##...##......",
        "....................",
        "....................",
        "....................",
    ]},
    # The Drowned Hall (23x13): two moat bands cut the great hall into
    # strips; boulder pairs stand on the dry causeways between.
    T.Quad{23, 13, [
        ".......................",
        ".......................",
        "...~~~~~~~~............",
        "...~~~~~~~~............",
        "...~~~~~~~~............",
        "...#...................",
        ".....##....##....##....",
        ".....##....##....##....",
        ".......................",
        "...~~~~~~~~............",
        "...~~~~~~~~............",
        ".......................",
        ".......................",
    ]},
    # The Rockslide (23x13): one shoulder of the hall has come down in a
    # diagonal scree with a pool caught at its foot; fight on the flat.
    T.Quad{23, 13, [
        ".......................",
        "..##...................",
        "..###..................",
        "...###.................",
        "...####................",
        "....####...............",
        "....#####..............",
        ".....#####.............",
        "......####....~~~~.....",
        ".......###....~~~~.....",
        "........##....~~~~.....",
        ".......................",
        ".......................",
    ]},
    # The Meander (19x11): an old stream bed winds corner to corner;
    # boulders bank its turns and both shores stay in touch round the ends.
    T.Quad{19, 11, [
        ".....#....#........",
        "..~~....#....#.....",
        "...~~..............",
        "....~~.....#.......",
        ".....~~~...........",
        "..#..~~~...........",
        "......~~~..#.......",
        ".......~~~.....#...",
        "........~~~........",
        "..........~~..#....",
        "...................",
    ]},
  ]
