//! The Archive's hand-drawn rooms (see `rooms`): quadrants, `.` floor, `#`
//! a block, `~` a hazard pool.

use super::super::layout::Template;

/// One-screen rooms: 11 by 6 quadrants.
pub const SMALL: &[Template] = &[
    // The Reading Alcove: shelf-butts and a reading desk in a small hall; corner the loremasters.
    [
        "...........",
        ".###.###...",
        "......##...",
        "......##...",
        "...........",
        "...........",
    ],
    // The Flooded Stacks: water in the corner seeping under the shelves; dry ground fights.
    [
        "~~~........",
        "~~~..##....",
        ".....##....",
        "...........",
        "~~~...#....",
        "...........",
    ],
    // The Map Table: one wide table with a clerk's stool; an open duel around it.
    [
        "...........",
        "......##...",
        "....####...",
        "....####...",
        "...........",
        "...........",
    ],
];

/// Bigger rooms: quadrants of the sizes in `rooms::SIZES`.
pub const GREAT: &[&[&str]] = &[
    // The Cartographer's Hall: the great table under its lamp, stools at the corners; plan the route.
    &[
        "...............",
        "...............",
        "....##.##......",
        "#...##.##......",
        "###............",
        "###.........#..",
        "###............",
        "...............",
        "...............",
    ],
    // The Sunken Reading Room: a flooded floor and desks hauled to the walls; shoot across the water.
    &[
        "................",
        ".####...........",
        ".####.......##..",
        "................",
        "......~~~~~.....",
        ".....~~~~~~.....",
        "................",
        "................",
        "................",
    ],
    // The Drowned Stacks: water down one wall, shelves marching away from it; slip-gaps everywhere.
    &[
        ".................",
        "~~~##............",
        "~~~###..##..##...",
        "~~~#.............",
        ".....#...##......",
        ".................",
        ".................",
        ".................",
        ".................",
    ],
    // The Flooded Aisle: water down the middle of the hall, shelves banked to either side.
    &[
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
    ],
    // The Index Hall: catalogue desks in clumps, water in the corners; many small fights.
    &[
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
    ],
    // The Scriptorium Deep: copyists' desks, an ink channel across, and water in the corner.
    &[
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
    ],
];
