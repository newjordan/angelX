//! The grain a picture is drawn at: how many of its pixels go across one of
//! the game's. The game draws at 1, sixteen pixels a tile; an HD pack at 4,
//! sixty-four a tile.

/// Pixels per game pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Grain(pub u8);

impl Grain {
    /// The game's own: the terminal, the wire and the golden pictures.
    pub const GAME: Grain = Grain(1);
    /// An HD pack's: 64 px a tile.
    pub const HD: Grain = Grain(4);
    /// Every grain kiln draws at.
    pub const ALL: [Grain; 4] = [Grain(1), Grain(2), Grain(4), Grain(8)];

    /// A grain from its number, when kiln draws at it.
    pub fn of(n: u8) -> Option<Grain> {
        Grain::ALL.into_iter().find(|g| g.0 == n)
    }

    /// Pixels a game pixel spans.
    pub fn px(self) -> f32 {
        self.0 as f32
    }

    /// The model point (in game pixels) pixel `(x, y)` samples: its centre.
    pub fn sample(self, x: u32, y: u32) -> (f32, f32) {
        ((x as f32 + 0.5) / self.px(), (y as f32 + 0.5) / self.px())
    }

    /// The pixels a span of `game` game pixels takes, rounded up.
    pub fn span(self, game: f32) -> u32 {
        (game * self.px()).ceil().max(0.0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pixel_samples_its_centre_in_game_pixels() {
        assert_eq!(Grain::GAME.sample(0, 0), (0.5, 0.5));
        assert_eq!(Grain::HD.sample(0, 0), (0.125, 0.125));
        assert_eq!(Grain::HD.sample(4, 8), (1.125, 2.125));
        assert_eq!(Grain::HD.span(16.0), 64);
        assert_eq!(Grain::of(3), None);
    }
}
