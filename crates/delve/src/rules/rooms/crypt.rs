//! The Crypt's hand-drawn rooms (see `rooms`): quadrants, `.` floor, `#`
//! a block, `~` a hazard pool.

use super::super::layout::Template;

/// One-screen rooms: 11 by 6 quadrants.
pub const SMALL: &[Template] = &[
    // The Bone Chapel: twin nave pillars and a tomb pair by the altar; fight down the open aisle.
    [
        "...........",
        ".##..##..#.",
        ".##..##..#.",
        "...........",
        "...........",
        "...........",
    ],
    // The Slid Tombs: sarcophagi dragged out of line into a diagonal; slalom between them.
    [
        "...........",
        ".##........",
        "....####...",
        "....####...",
        "...........",
        "...........",
    ],
    // The Weeping Vault: a flooded vault in the corner, a pillar pair across it; shoot over the water.
    [
        "#..........",
        "#.~~....#..",
        "#.~~.....#.",
        "...........",
        "...........",
        "...........",
    ],
    // The Ossuary Wall: niched bones and loose stones in an open hall; a duel at range.
    [
        "##.........",
        "####...###.",
        "...........",
        "......#....",
        "...........",
        "...........",
    ],
];

/// Bigger rooms: quadrants of the sizes in `rooms::SIZES`.
pub const GREAT: &[&[&str]] = &[
    // The Chapel Apse: a broken apse arch off the corner, its floor flooded; the dead wait in the curve.
    &[
        "...............",
        "..###.##.......",
        ".#........#....",
        ".#........#....",
        ".#..~.~~..#....",
        ".#..~~~~..#....",
        "..........#....",
        "...............",
        "...............",
    ],
    // The Reliquary Aisle: three reliquaries a side down the aisle and a casket at the crossing; long lanes.
    &[
        "#...............",
        "####..##..##....",
        "####..##..##....",
        "................",
        ".......##.......",
        ".......##.......",
        "................",
        "................",
        "................",
    ],
    // The Drowned Ossuary: two sunken tombs and sliding water; wade the dry diagonal.
    &[
        "................",
        "..~~~...##......",
        "..~~~...##......",
        ".......###......",
        "......~~~.......",
        "..##..~~~.......",
        "................",
        "................",
        "................",
    ],
    // The Flooded Gallery: a moat straight across the gallery and shelf-butts beyond it; fight over the water.
    &[
        ".................",
        "..##.##.##.##....",
        ".....#...........",
        "....~~~~~~~......",
        "....~~.~~~~......",
        ".................",
        ".................",
        ".................",
        ".................",
    ],
    // The Sepulchre Field: tombs set on the half-step so the ranks interleave; no straight lane through.
    &[
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
    ],
    // The Processional: pillar pairs and casket butlers along the walk, pooled crossings between; circle the pillars.
    &[
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
    ],
    // The Crypt Beneath the Crypt: older tombs on the half-step and a black channel below; mind the water.
    &[
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
    ],
    // The Catacomb Shelves: shelf-racks stepping down the diagonal into a pooled undercroft; slide fight.
    &[
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
    ],
    // The Bone Cathedral: a tomb rank, a broken reliquary screen and a high altar at the crossing.
    &[
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
    ],
];
