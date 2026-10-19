//! The Unknown's hand-drawn rooms (see `rooms`): quadrants, `.` floor, `#`
//! a block, `~` a hazard pool.

use super::super::layout::Template;

/// One-screen rooms: 11 by 6 quadrants.
pub const SMALL: &[Template] = &[
    // The Antechamber of the Spiral: outer pillars of a spiral stand
    // detached, its centre gone; a calm ring before the descent begins.
    [
        "...........",
        ".##.##.##..",
        "...........",
        "..#..#.....",
        ".##.##.....",
        "...........",
    ],
    // The Void Puddle: a sliver of the void sits mid-floor; two shards
    // lean against the wall. Cross the line of fire or go around.
    [
        "...........",
        "..#..~~..#.",
        ".....~~....",
        ".#...~~....",
        "..#........",
        "...........",
    ],
    // The Tilted Slab: one great slab tips into a void sink; the floor
    // slopes to a fight under its shadow.
    [
        "..###......",
        "..###......",
        "..###.~~...",
        ".#....~~...",
        ".#.........",
        "...........",
    ],
    // The Hollow Twin: two hollow columns hum off-axis, void seeping at
    // their feet; nothing here is where you left it.
    [
        ".#....#....",
        ".#....#....",
        ".##...##...",
        ".....~~....",
        ".....~~....",
        "...........",
    ],
];

/// Bigger rooms: quadrants of the sizes in `rooms::SIZES`.
pub const GREAT: &[&[&str]] = &[
    // The Descent of Nine (16x9): pillar pairs walk in a slow spiral toward
    // the middle; every ring is two tiles apart so the maze never closes.
    &[
        "................",
        "..#....#....#...",
        "..##..##..##..##",
        "...#........#...",
        "..##...~~...##..",
        "................",
        "....##..##......",
        "......~~........",
        "................",
    ],
    // The Void Moat (20x11): an island floats in a void moat, reached by
    // stepping stones; the far bank circles all the way round.
    &[
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
    ],
    // The Geometry of the Deep (23x13): shards of some older room — a
    // colonnade, a ring, a moat — meet and refuse to agree.
    &[
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
    ],
    // The Thin Orbit (15x9): sparse single pillars circle a void bead;
    // open space rules, cover is a rumour you duck behind once.
    &[
        "...............",
        "..#...#..#..#..",
        ".....#....#....",
        "..#..........#.",
        "....~~....~~...",
        "..#.........#..",
        "....~~~........",
        ".....#....#....",
        "...............",
    ],
    // The Regolith Field (17x9): angular shards lie where they fell in
    // twin ranks; the void pools in the seam between the ranks.
    &[
        ".................",
        "...#..........#..",
        ".##...##...##....",
        ".##...##...##....",
        ".................",
        "....~~~~~~~......",
        ".................",
        ".##...##...##....",
        ".................",
    ],
];
