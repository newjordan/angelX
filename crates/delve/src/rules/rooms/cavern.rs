//! The Cavern's hand-drawn rooms (see `rooms`): quadrants, `.` floor, `#`
//! a block, `~` a hazard pool.

use super::super::layout::Template;

/// One-screen rooms: 11 by 6 quadrants.
pub const SMALL: &[Template] = &[
    // The Boulder Rill: scattered boulder pairs around a sink pool, open
    // ground everywhere so a knight is never far from cover or a dodge.
    [
        "...........",
        ".##....#...",
        ".....~~....",
        "...#.~~....",
        "..#..#.....",
        "...........",
    ],
    // The Stalactite Garden: lone stalagmites with a shallow pool at the
    // foot, a dancing arena of single blocks and short hops.
    [
        "...........",
        "...#....#..",
        "..~.#.~....",
        "..#....#...",
        "#....~~....",
        "...........",
    ],
    // The Sunken Gallery: an S-shaped pool threading the middle, knights
    // shoot across it and circle either bank between rubble stubs.
    [
        "...........",
        ".##.~~~....",
        "......~....",
        "....#.~~...",
        ".#.........",
        "...........",
    ],
    // The Crystal Nook: a tight cluster of crystal spars by the wall with
    // a black pool at its foot; the rest is open floor for the fight.
    [
        "......#....",
        "...#..#..#.",
        ".~~....#...",
        ".~~..#.....",
        "...........",
        "...........",
    ],
];

/// Bigger rooms: quadrants of the sizes in `rooms::SIZES`.
pub const GREAT: &[&[&str]] = &[
    // The Boulder Field (16x9): an open arena where two boulder clumps
    // flank a reed pool; long sight lines, hard cover to sprint between.
    &[
        "................",
        "..##....#.......",
        "....~~~.........",
        "..#.~~~....##...",
        "....~~~.........",
        "..#....##.......",
        "..~~~...........",
        "..#.............",
        "................",
    ],
    // The Collapsed Gallery (16x9): fallen gallery columns lie in broken
    // rows; water gathers at their feet and knights fight down the aisles.
    &[
        "................",
        ".###...###..#...",
        "................",
        ".~~.#....#..~~..",
        "................",
        ".###...###..~~..",
        "................",
        "..#.....#.......",
        "................",
    ],
    // The Underground Lake (20x11): a broad lake mid-room with two stone
    // islets; arrows fly over the water while the party skirts the shore.
    &[
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
    ],
    // The Drowned Hall (23x13): two moat bands cut the great hall into
    // strips; boulder pairs stand on the dry causeways between.
    &[
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
    ],
    // The Rockslide (23x13): one shoulder of the hall has come down in a
    // diagonal scree with a pool caught at its foot; fight on the flat.
    &[
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
    ],
    // The Meander (19x11): an old stream bed winds corner to corner;
    // boulders bank its turns and both shores stay in touch round the ends.
    &[
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
    ],
];
