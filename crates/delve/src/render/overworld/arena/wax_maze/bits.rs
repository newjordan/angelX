//! The Wax Maze's small sprites, drawn once and kept: the wax drop, the
//! flask's stopper, the leader's crown, a coin and the candle's flame.

use super::super::super::ink::Img;
use std::sync::OnceLock;

/// A drop of candle wax, the pellet: a cream tear, a pale glint, a brown
/// shadow. Five wide, six tall; the second is the drop at its glint.
pub(super) fn drop(glint: bool) -> &'static Img {
    static DROPS: OnceLock<[Img; 2]> = OnceLock::new();
    let drops = DROPS.get_or_init(|| {
        let tear = |glint: bool| {
            Img::from_rows(&[
                "..T..",
                if glint { ".THt." } else { ".TTt." },
                ".TTtO",
                "TTTtO",
                ".tOOn",
                "..nn.",
            ])
        };
        [tear(false), tear(true)]
    });
    &drops[usize::from(glint)]
}

/// A small tear of wax in flight (spilt wax): three by four.
pub(super) fn spill() -> &'static Img {
    static SPILL: OnceLock<Img> = OnceLock::new();
    SPILL.get_or_init(|| Img::from_rows(&[".T.", "TtO", "tOo", ".n."]))
}

/// The leader's crown, seven by four.
pub(super) fn crown() -> &'static Img {
    static CROWN: OnceLock<Img> = OnceLock::new();
    CROWN.get_or_init(|| Img::from_rows(&["T.T.T.T", "TtTtTtT", "tOOOOOt", ".nnnnn."]))
}

/// A gold coin, five square, for the pot.
pub(super) fn coin() -> &'static Img {
    static COIN: OnceLock<Img> = OnceLock::new();
    COIN.get_or_init(|| Img::from_rows(&[".444.", "46TO4", "4TOa4", "4OaO4", ".444."]))
}

/// A candle's flame, two frames, three by five.
pub(super) fn flame(frame: u32) -> &'static Img {
    static FLAMES: OnceLock<[Img; 2]> = OnceLock::new();
    let f = FLAMES.get_or_init(|| {
        [
            Img::from_rows(&[".6.", "656", "6@6", "6@6", ".R."]),
            Img::from_rows(&["..6", ".66", "6@6", "6@6", ".R."]),
        ]
    });
    &f[(frame % 2) as usize]
}
