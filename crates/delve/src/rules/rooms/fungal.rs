//! The Fungal's hand-drawn rooms (see `rooms`): quadrants, `.` floor, `#`
//! a block, `~` a hazard pool.

use super::super::layout::Template;

/// One-screen rooms: 11 by 6 quadrants.
pub const SMALL: &[Template] = &[
    // The Fairy Ring: a broken ring of mushrooms round a spore pool;
    // knights circle the ring and shoot across the spores.
    [
        "...........",
        "..#.....#..",
        "...~~~.....",
        "...~~..#...",
        "..#....#...",
        "...........",
    ],
    // The Spore Sump: a sunken sump of spores by the wall with stumps
    // scattered on the dry ground; approach from either side.
    [
        "...........",
        ".#...~~....",
        ".....~~....",
        "..#..~~#...",
        ".##........",
        "...........",
    ],
    // The Root Web: braided roots in short runs with wide gaps between
    // them; room to slip a whole party through at speed.
    [
        "...........",
        ".##..##..#.",
        "...........",
        ".#...#.#...",
        "....~~.....",
        "...........",
    ],
    // The Toadstool Nook: two giant toadstools crowd a corner over a
    // spore slick; the open floor beside them is where the fight goes.
    [
        "....##.....",
        ".##.##.~~..",
        ".#.....~...",
        "....##.....",
        "...........",
        "...........",
    ],
];

/// Bigger rooms: quadrants of the sizes in `rooms::SIZES`.
pub const GREAT: &[&[&str]] = &[
    // The Spore Flats (16x9): open flats pocked with sumps and stumps;
    // a running fight with little cover and much dodging.
    &[
        "................",
        ".#....#....#....",
        "..~~~....~~~....",
        "..~~~....~~~....",
        ".#....#....#....",
        "....~~..~~......",
        "..##......##....",
        "................",
        "................",
    ],
    // The Ring Garden (20x11): mushroom rings dotted round the chamber,
    // spore pools between them; weave the rings or hold the open rim.
    &[
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
    ],
    // The Bloom Meadow (19x11): blooming mushrooms in five-petal clusters
    // across a meadow of spore puddles; a bright, open shooting gallery.
    &[
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
    ],
];
