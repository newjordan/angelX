//! The kingdom's eleven biomes, one to a chunk of 24x16 tiles: which biome
//! each chunk belongs to, what each is called, and the open green its name
//! is lettered on. The ground each grows is `super::wild`'s; the design is
//! `docs/KINGDOM-OPEN.md`.

use super::map::{CHUNK_H, CHUNK_W, CHUNKS_X, CHUNKS_Y};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Biome {
    CastleTown,
    Greenwood,
    Highlands,
    Wastes,
    Coast,
    Fen,
    TiltYards,
    Fairground,
    WindCountry,
    Longbow,
    March,
}

/// Each chunk's biome, row by row (`docs/KINGDOM-OPEN.md`).
const CHART: [&[u8; CHUNKS_X as usize]; CHUNKS_Y as usize] = [
    b"GGGHHHHWWC",
    b"GGGHHHWWWC",
    b"GGHHHHWWWC",
    b"FTTTTYYYCC",
    b"FFTTTYYYCC",
    b"FFTTTYYYYC",
    b"FFFADDDLLC",
    b"FFAADDDLLC",
    b"FAADDDLLLC",
    b"MMMMMMMMMM",
    b"MMMMMMMMMM",
];

impl Biome {
    pub const ALL: [Biome; 11] = [
        Biome::CastleTown,
        Biome::Greenwood,
        Biome::Highlands,
        Biome::Wastes,
        Biome::Coast,
        Biome::Fen,
        Biome::TiltYards,
        Biome::Fairground,
        Biome::WindCountry,
        Biome::Longbow,
        Biome::March,
    ];

    fn of_letter(b: u8) -> Biome {
        match b {
            b'G' => Biome::Greenwood,
            b'H' => Biome::Highlands,
            b'W' => Biome::Wastes,
            b'C' => Biome::Coast,
            b'F' => Biome::Fen,
            b'T' => Biome::CastleTown,
            b'Y' => Biome::TiltYards,
            b'A' => Biome::Fairground,
            b'D' => Biome::WindCountry,
            b'L' => Biome::Longbow,
            _ => Biome::March,
        }
    }

    /// Its name, as a signpost or the corner of a view writes it.
    pub fn name(self) -> &'static str {
        match self {
            Biome::CastleTown => "Castle Town",
            Biome::Greenwood => "Greenwood",
            Biome::Highlands => "The Highlands",
            Biome::Wastes => "The Wastes",
            Biome::Coast => "The Coast",
            Biome::Fen => "The Fen",
            Biome::TiltYards => "The Tilt-Yards",
            Biome::Fairground => "The Fairground",
            Biome::WindCountry => "Wind Country",
            Biome::Longbow => "Longbow Country",
            Biome::March => "The March",
        }
    }

    /// Its index in [`Biome::ALL`], for saving.
    pub fn index(self) -> u8 {
        Biome::ALL.iter().position(|&b| b == self).unwrap_or(0) as u8
    }

    /// Every chunk `(cx, cy)` that belongs to it.
    pub fn chunks(self) -> impl Iterator<Item = (i32, i32)> {
        (0..CHUNKS_Y).flat_map(move |cy| {
            (0..CHUNKS_X)
                .filter(move |&cx| chunk(cx, cy) == self)
                .map(move |cx| (cx, cy))
        })
    }
}

/// The biome of chunk `(cx, cy)`; off the chart reads the nearest edge.
pub fn chunk(cx: i32, cy: i32) -> Biome {
    let (cx, cy) = (cx.clamp(0, CHUNKS_X - 1), cy.clamp(0, CHUNKS_Y - 1));
    Biome::of_letter(CHART[cy as usize][cx as usize])
}

/// The biome tile `(x, y)` lies in.
pub fn at(x: i32, y: i32) -> Biome {
    chunk(x.div_euclid(CHUNK_W), y.div_euclid(CHUNK_H))
}
