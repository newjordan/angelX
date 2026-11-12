pub use angel_stage::braille::*;

/// Tone calibration for the moonlit frame → 1-bit dots. Tuned against the
/// moving and settled scene fixtures: while riding, the sky stays
/// almost empty (stars and moon only), near walls dither at roughly half, and
/// fog starves walls toward the horizon. Settled vistas deliberately use a
/// luminous-horizon painting curve instead.
const TONE_BLACK: f32 = 18.0;
const TONE_WHITE: f32 = 118.0;
const TONE_GAMMA: f32 = 0.82;
const VISTA_TONE_BLACK: f32 = 8.0;
const VISTA_TONE_WHITE: f32 = 112.0;
const VISTA_TONE_GAMMA: f32 = 0.78;

/// A cell's ink must clear this max-channel value to survive on a dark
/// terminal; depth still reads from dot density, not ink brightness.
const INK_FLOOR: f32 = 70.0;

/// The calibrated tone curve as a 256-entry luma LUT: the black/white points
/// and gamma only ever see 8-bit luma, so the per-dot `powf` (thousands per
/// re-march) collapses to one table build.
fn tone_lut(vista: bool) -> &'static [f32; 256] {
    static LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    static VISTA_LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    let (lut, black, white, gamma) = if vista {
        (
            &VISTA_LUT,
            VISTA_TONE_BLACK,
            VISTA_TONE_WHITE,
            VISTA_TONE_GAMMA,
        )
    } else {
        (&LUT, TONE_BLACK, TONE_WHITE, TONE_GAMMA)
    };
    lut.get_or_init(|| {
        let mut table = [0.0; 256];
        for (i, value) in table.iter_mut().enumerate() {
            *value = ((i as f32 - black) / (white - black))
                .clamp(0.0, 1.0)
                .powf(gamma);
        }
        table
    })
}

pub fn frame_to_braille_graded(
    frame: &image::RgbaImage,
    cells_w: usize,
    cells_h: usize,
    vista: bool,
) -> ColoredBrailleImage {
    frame_to_braille(frame, cells_w, cells_h, vista, false)
}

/// Tonal bridge for the realm's 3D views only. Native map art continues to
/// use `frame_to_braille_graded` and its existing floor and density screen.
pub fn frame_to_braille_world3d(
    frame: &image::RgbaImage,
    cells_w: usize,
    cells_h: usize,
    vista: bool,
) -> ColoredBrailleImage {
    frame_to_braille(frame, cells_w, cells_h, vista, true)
}

fn frame_to_braille(
    frame: &image::RgbaImage,
    cells_w: usize,
    cells_h: usize,
    vista: bool,
    tonal: bool,
) -> ColoredBrailleImage {
    // Structure first: the shared fitter picks each cell's dots from the
    // cell's own light/dark split, so outlines, wall edges and doorways keep
    // their line and dark material is drawn as shape, not dust.
    let lut = tone_lut(vista);
    let fit = fit_braille_tonal(
        cells_w,
        cells_h,
        |x, y| {
            let pixel = frame.get_pixel(x as u32, y as u32).0;
            let luma = 0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
            let tone = lut[(luma.round() as usize).min(255)];
            DotSample {
                luma: tone,
                level: tone,
                rgb: [pixel[0] as f32, pixel[1] as f32, pixel[2] as f32],
            }
        },
        tonal,
    );
    let mut cells = vec![ColoredBrailleCell::default(); cells_w * cells_h];
    for (i, (bits, ink)) in fit.into_iter().enumerate() {
        if bits == 0 {
            continue;
        }
        // The cell's ink is the mean of its lit pixels: saturated, and as
        // dark as the material is. A dim dot is lifted only enough to show.
        let mut fg = ink;
        let peak = fg[0].max(fg[1]).max(fg[2]);
        if !tonal && peak > 0.0 && peak < INK_FLOOR {
            let scale = INK_FLOOR / peak;
            fg = [fg[0] * scale, fg[1] * scale, fg[2] * scale];
        }
        let quantize_channel = |val: f32| {
            let channel = val.round().clamp(0.0, 255.0) as u16;
            let level = (channel * 31 + 127) / 255;
            ((level * 255 + 15) / 31) as u8
        };
        cells[i] = ColoredBrailleCell {
            glyph: braille_char(bits),
            fg: if tonal {
                fg.map(|c| c.round().clamp(0.0, 228.0) as u8)
            } else {
                [
                    quantize_channel(fg[0]),
                    quantize_channel(fg[1]),
                    quantize_channel(fg[2]),
                ]
            },
        };
    }
    ColoredBrailleImage {
        width: cells_w,
        height: cells_h,
        cells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_planes_keep_solid_tonal_ink_and_the_map_keeps_its_screen() {
        let frame = image::RgbaImage::from_pixel(12, 12, image::Rgba([25, 28, 34, 255]));
        let world = frame_to_braille_world3d(&frame, 6, 3, false);
        assert!(
            world
                .cells
                .iter()
                .all(|c| braille_bits(c.glyph) == 255 && c.fg == [25, 28, 34])
        );
        let map = frame_to_braille_graded(&frame, 6, 3, false);
        assert!(map.cells.iter().any(|c| braille_bits(c.glyph) != 255));
        assert!(
            map.cells
                .iter()
                .filter(|c| braille_bits(c.glyph) != 0)
                .all(|c| c.fg.iter().copied().max().unwrap() >= 70)
        );
    }

    #[test]
    fn world_ink_falls_to_black_without_a_white_floor_or_colour_bands() {
        let mut last = 0;
        for value in 32..100u8 {
            let frame = image::RgbaImage::from_pixel(2, 4, image::Rgba([value, value, value, 255]));
            let cell = frame_to_braille_world3d(&frame, 1, 1, false).cells[0];
            assert_eq!(braille_bits(cell.glyph), 255);
            assert_eq!(cell.fg, [value; 3]);
            assert!(cell.fg[0] > last);
            last = cell.fg[0];
        }
        let black = image::RgbaImage::from_pixel(2, 4, image::Rgba([4, 5, 8, 255]));
        assert_eq!(
            frame_to_braille_world3d(&black, 1, 1, false).cells[0],
            ColoredBrailleCell::default()
        );
    }
}
