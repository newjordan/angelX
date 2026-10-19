//! Where a frame goes on the canvas: the same box `shell/present.js` fits.
//!
//! A game draws at its own native size (the muster 320x180, the Delve 384x224
//! or 768x448); on the canvas it is scaled by the largest whole number that
//! fits, black around it. Where a whole number would leave too much of the
//! window black (a large frame on a small screen: the Delve's second art at
//! 768 wide in a 1280 window is only 1x), or not even 1x fits, it is fitted
//! instead, still nearest, rather than shown small or cropped.

/// A whole-number scale is kept while it fills at least this share of what
/// fitting the frame would.
pub const MIN_FILL: f64 = 0.8;

/// The dev panel's `scale` knob: 0 as above, 1 whole numbers whenever 1x
/// fits, 2 always fitted. Anything else is 0.
pub fn mode(m: u32) -> u8 {
    if m == 1 || m == 2 { m as u8 } else { 0 }
}

/// Where a `frame`-sized picture goes in a `screen`-sized canvas: x, y (from
/// the top), width, height, in the canvas's pixels. Never empty, never past
/// the canvas.
pub fn fit(screen: (u32, u32), frame: (u32, u32), mode: u8) -> [u32; 4] {
    let (cw, ch) = (screen.0.max(1), screen.1.max(1));
    let (w, h) = (frame.0.max(1), frame.1.max(1));
    let k = (cw / w).min(ch / h);
    let fitted = (f64::from(cw) / f64::from(w)).min(f64::from(ch) / f64::from(h));
    let whole = match mode {
        1 => k >= 1,
        2 => false,
        _ => k >= 1 && f64::from(k) >= MIN_FILL * fitted,
    };
    let (dw, dh) = if whole {
        (w * k, h * k)
    } else {
        (
            ((f64::from(w) * fitted).floor() as u32).clamp(1, cw),
            ((f64::from(h) * fitted).floor() as u32).clamp(1, ch),
        )
    };
    [(cw - dw) >> 1, (ch - dh) >> 1, dw, dh]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `present.js`'s `fit` gives for the same inputs, taken from that
    /// function itself (node, 2026-10-16): screen, frame, scale knob, box.
    #[test]
    fn the_box_is_the_one_present_js_fits() {
        type Case = ((u32, u32), (u32, u32), u8, [u32; 4]);
        #[rustfmt::skip]
        let table: &[Case] = &[
        ((1280, 800), (384, 224), 0, [64, 64, 1152, 672]),
        ((2880, 1800), (320, 180), 0, [0, 90, 2880, 1620]),
        ((1280, 800), (768, 448), 0, [0, 27, 1280, 746]),
        ((300, 150), (384, 224), 0, [21, 0, 257, 150]),
        ((1600, 1000), (384, 224), 0, [32, 52, 1536, 896]),
        ((1440, 900), (320, 180), 0, [80, 90, 1280, 720]),
        ((390, 844), (384, 224), 0, [3, 310, 384, 224]),
        ((2560, 1440), (768, 448), 0, [128, 48, 2304, 1344]),
        ((1920, 1080), (384, 224), 0, [192, 92, 1536, 896]),
        ((800, 600), (320, 180), 0, [80, 120, 640, 360]),
        ((1280, 800), (384, 224), 1, [64, 64, 1152, 672]),
        ((2880, 1800), (320, 180), 1, [0, 90, 2880, 1620]),
        ((1280, 800), (768, 448), 1, [256, 176, 768, 448]),
        ((300, 150), (384, 224), 1, [21, 0, 257, 150]),
        ((1600, 1000), (384, 224), 1, [32, 52, 1536, 896]),
        ((1440, 900), (320, 180), 1, [80, 90, 1280, 720]),
        ((390, 844), (384, 224), 1, [3, 310, 384, 224]),
        ((2560, 1440), (768, 448), 1, [128, 48, 2304, 1344]),
        ((1920, 1080), (384, 224), 1, [192, 92, 1536, 896]),
        ((800, 600), (320, 180), 1, [80, 120, 640, 360]),
        ((1280, 800), (384, 224), 2, [0, 27, 1280, 746]),
        ((2880, 1800), (320, 180), 2, [0, 90, 2880, 1620]),
        ((1280, 800), (768, 448), 2, [0, 27, 1280, 746]),
        ((300, 150), (384, 224), 2, [21, 0, 257, 150]),
        ((1600, 1000), (384, 224), 2, [0, 33, 1600, 933]),
        ((1440, 900), (320, 180), 2, [0, 45, 1440, 810]),
        ((390, 844), (384, 224), 2, [0, 308, 390, 227]),
        ((2560, 1440), (768, 448), 2, [46, 0, 2468, 1440]),
        ((1920, 1080), (384, 224), 2, [34, 0, 1851, 1080]),
        ((800, 600), (320, 180), 2, [0, 75, 800, 450]),
        ];
        for &(screen, frame, mode, want) in table {
            assert_eq!(fit(screen, frame, mode), want, "{screen:?} {frame:?} mode {mode}");
        }
    }

    #[test]
    fn the_scale_knob_takes_zero_one_or_two() {
        assert_eq!((mode(0), mode(1), mode(2), mode(7)), (0, 1, 2, 0));
    }

    #[test]
    fn nothing_is_empty_or_outside() {
        for (screen, frame) in [((1, 1), (384, 224)), ((0, 0), (1, 1)), ((8192, 8192), (1, 1)), ((5, 9000), (9000, 5))] {
            for mode in 0..3 {
                let [x, y, w, h] = fit(screen, frame, mode);
                assert!(w >= 1 && h >= 1);
                assert!(x + w <= screen.0.max(1) && y + h <= screen.1.max(1), "{screen:?} {frame:?} {mode}");
            }
        }
    }
}
