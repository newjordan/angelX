//! Small lantern seals left at a landmark when its tool reports back.
//!
//! These show public execution/verification receipts, never inferred reasoning.
//! One seal per place keeps a busy village legible; the host notes the
//! receipts and says which seal shows where.

use super::ink::Img;
use super::map::Place;
use super::scene::Motion;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Seal {
    Completed,
    Verified,
    Failed,
    Unresolved,
}

impl Seal {
    fn ink(self) -> char {
        match self {
            Self::Completed => '5',
            Self::Verified => '2',
            Self::Failed => '7',
            Self::Unresolved => 'v',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Echo {
    pub place: Place,
    pub seal: Seal,
    pub phase: u8,
}

/// A little metal lantern with a distinct seal inside. Reduced/off motion
/// retain the same receipt as a still shape: no motes, bob, pulse or flashing.
pub fn lantern(echo: Echo, motion: Motion) -> Img {
    let mut im = Img::new(23, 25);
    let color = echo.seal.ink();
    im.line(11, 16, 11, 23, 'G');
    im.line(8, 23, 14, 23, 'j');
    for dy in -6i32..=6 {
        let half = 6 - dy.abs();
        im.rect(11 - half, 10 + dy, half * 2 + 1, 1, 'k');
    }
    for (x0, y0, x1, y1) in [
        (11, 4, 17, 10),
        (17, 10, 11, 16),
        (11, 16, 5, 10),
        (5, 10, 11, 4),
    ] {
        im.line(x0, y0, x1, y1, 'G');
    }
    match echo.seal {
        Seal::Completed => {
            im.line(11, 7, 14, 10, color);
            im.line(14, 10, 11, 13, color);
            im.line(11, 13, 8, 10, color);
            im.line(8, 10, 11, 7, color);
            im.put(11, 10, '@');
        }
        Seal::Verified => {
            im.line(8, 10, 10, 12, color);
            im.line(10, 12, 14, 8, color);
            im.put(14, 7, '3');
        }
        Seal::Failed => {
            im.line(9, 8, 13, 12, color);
            im.line(13, 8, 9, 12, color);
        }
        Seal::Unresolved => {
            im.line(9, 8, 9, 12, color);
            im.line(13, 8, 13, 12, color);
        }
    }
    if motion == Motion::Full && echo.seal != Seal::Unresolved && echo.phase < 6 {
        let phase = i32::from(echo.phase);
        // Completed work lifts its motes; failures settle as embers. The
        // glyph stays put so meaning never depends on catching one frame.
        let drift = if echo.seal == Seal::Failed {
            phase / 2
        } else {
            -(phase / 2)
        };
        for (i, (x, y)) in [(2, 8), (20, 9), (6, 3), (17, 3)].into_iter().enumerate() {
            if phase < 3 || i % 2 == 0 {
                im.put(x, y + drift, color);
            }
        }
    }
    im
}

