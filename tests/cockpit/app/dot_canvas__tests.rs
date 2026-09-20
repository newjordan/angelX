use super::*;
use crate::ui::term::art::ColoredBrailleCell;

const INK: [u8; 3] = [200, 40, 255];
const CLAMPED: [u8; 4] = [190, 40, 190, 255];

fn cell(glyph: char, fg: [u8; 3]) -> ColoredBrailleCell {
    ColoredBrailleCell { glyph, fg }
}

fn grid(width: usize, height: usize, cells: Vec<ColoredBrailleCell>) -> ColoredBrailleImage {
    ColoredBrailleImage {
        width,
        height,
        cells,
    }
}

fn single(glyph: char, fg: [u8; 3]) -> ColoredBrailleImage {
    grid(1, 1, vec![cell(glyph, fg)])
}

fn ink_pixels(raster: &image::RgbaImage) -> Vec<(u32, u32)> {
    raster
        .enumerate_pixels()
        .filter(|(_, _, pixel)| pixel.0 != BG)
        .map(|(x, y, _)| (x, y))
        .collect()
}

/// Standard 8-dot Braille: columns 0,1 and rows 0..4, bits 0..=7.
const DOT_MAP: [(u32, u32, char); 8] = [
    (0, 0, '\u{2801}'),
    (0, 1, '\u{2802}'),
    (0, 2, '\u{2804}'),
    (1, 0, '\u{2808}'),
    (1, 1, '\u{2810}'),
    (1, 2, '\u{2820}'),
    (0, 3, '\u{2840}'),
    (1, 3, '\u{2880}'),
];

#[test]
fn eight_dot_positions_and_clamped_rgb() {
    let pitch = 4u16;
    let geometry = DotGeometry::new(1, 1, (8, 16), pitch).expect("1x1 cell canvas");
    assert_eq!(geometry.width, 8);
    assert_eq!(geometry.height, 16);
    assert_eq!(geometry.grid_width, 1);
    assert_eq!(geometry.grid_height, 1);
    assert_eq!(geometry.pitch, 4);
    let (pad_x, pad_y) = geometry.origin().expect("origin");
    assert_eq!((pad_x, pad_y), (0, 0));

    for (local_x, local_y, glyph) in DOT_MAP {
        let raster = geometry
            .rasterize(&single(glyph, INK))
            .expect("rasterize one dot");
        assert_eq!(raster.dimensions(), (8, 16));
        let expected = (
            pad_x + local_x * u32::from(pitch),
            pad_y + local_y * u32::from(pitch),
        );
        let lit = ink_pixels(&raster);
        assert_eq!(lit, vec![expected], "glyph {glyph:?}");
        assert_eq!(raster.get_pixel(expected.0, expected.1).0, CLAMPED);
    }

    let all = geometry
        .rasterize(&single('\u{28FF}', [255, 207, 92]))
        .expect("all eight dots");
    let lit = ink_pixels(&all);
    assert_eq!(lit.len(), 8);
    for (local_x, local_y, _) in DOT_MAP {
        let x = pad_x + local_x * u32::from(pitch);
        let y = pad_y + local_y * u32::from(pitch);
        assert_eq!(all.get_pixel(x, y).0, [190, 190, 92, 255]);
    }
    assert_eq!(all.get_pixel(1, 0).0, BG);
    assert_eq!(all.get_pixel(0, 1).0, BG);
}

#[test]
fn equal_pitch_across_cell_and_row_boundaries() {
    let pitch = 3u32;
    let geometry = DotGeometry::new(1, 1, (12, 24), 3).expect("2x2 virtual cells");
    assert_eq!(
        (geometry.grid_width, geometry.grid_height, geometry.pitch),
        (2, 2, pitch)
    );
    let (pad_x, pad_y) = geometry.origin().expect("origin");
    assert_eq!((pad_x, pad_y), (0, 0));

    let cells = vec![
        cell('\u{28FF}', [80, 90, 100]),
        cell('\u{28FF}', [80, 90, 100]),
        cell('\u{28FF}', [80, 90, 100]),
        cell('\u{28FF}', [80, 90, 100]),
    ];
    let raster = geometry.rasterize(&grid(2, 2, cells)).expect("2x2 raster");
    let lit = ink_pixels(&raster);
    assert_eq!(lit.len(), 32);

    let xs: Vec<u32> = {
        let mut values: Vec<u32> = lit.iter().map(|(x, _)| *x).collect();
        values.sort_unstable();
        values.dedup();
        values
    };
    let ys: Vec<u32> = {
        let mut values: Vec<u32> = lit.iter().map(|(_, y)| *y).collect();
        values.sort_unstable();
        values.dedup();
        values
    };
    assert_eq!(xs, vec![0, 3, 6, 9]);
    assert_eq!(ys, vec![0, 3, 6, 9, 12, 15, 18, 21]);
    for window in xs.windows(2) {
        assert_eq!(window[1] - window[0], pitch);
    }
    for window in ys.windows(2) {
        assert_eq!(window[1] - window[0], pitch);
    }

    let right_of_first_cell = pad_x + pitch;
    let first_of_next_cell = pad_x + 2 * pitch;
    assert_eq!(first_of_next_cell - right_of_first_cell, pitch);
    let bottom_of_first_row = pad_y + 3 * pitch;
    let first_of_next_row = pad_y + 4 * pitch;
    assert_eq!(first_of_next_row - bottom_of_first_row, pitch);
    assert_eq!(raster.get_pixel(right_of_first_cell + 1, 0).0, BG);
}

#[test]
fn centering_and_aspect_at_different_cell_sizes() {
    let compact = DotGeometry::new(10, 5, (8, 16), 2).expect("8x16 cells");
    assert_eq!((compact.width, compact.height), (80, 80));
    assert_eq!((compact.grid_width, compact.grid_height), (20, 10));
    assert_eq!(compact.origin(), Some((0, 0)));

    let padded = DotGeometry::new(10, 5, (9, 18), 2).expect("9x18 cells");
    assert_eq!((padded.width, padded.height), (90, 90));
    assert_eq!((padded.grid_width, padded.grid_height), (22, 11));
    assert_eq!(padded.origin(), Some((1, 1)));
    assert_eq!(
        padded.width as usize,
        padded.grid_width * 2 * padded.pitch as usize + 2
    );
    assert_eq!(
        padded.height as usize,
        padded.grid_height * 4 * padded.pitch as usize + 2
    );

    let wide = DotGeometry::new(4, 2, (16, 8), 2).expect("wide cells");
    let tall = DotGeometry::new(4, 2, (8, 16), 2).expect("tall cells");
    assert_eq!((wide.width, wide.height), (64, 16));
    assert_eq!((tall.width, tall.height), (32, 32));
    assert_ne!(
        wide.width * tall.height,
        tall.width * wide.height,
        "canvas aspect follows terminal cell size, not a warped square"
    );

    let odd = DotGeometry::new(1, 1, (11, 19), 4).expect("odd leftover");
    assert_eq!((odd.grid_width, odd.grid_height), (1, 1));
    assert_eq!(odd.origin(), Some((1, 1)));
    let raster = odd
        .rasterize(&single('\u{2801}', [10, 20, 30]))
        .expect("padded raster");
    assert_eq!(raster.dimensions(), (11, 19));
    assert_eq!(ink_pixels(&raster), vec![(1, 1)]);
    assert_eq!(raster.get_pixel(0, 1).0, BG);
    assert_eq!(raster.get_pixel(10, 18).0, BG);

    let host = DotGeometry::new(1, 1, (16, 32), 2).expect("4x4 host grid");
    assert_eq!((host.grid_width, host.grid_height), (4, 4));
    let small = grid(
        2,
        1,
        vec![
            cell('\u{2801}', [12, 34, 56]),
            cell('\u{2801}', [12, 34, 56]),
        ],
    );
    let centered = host.rasterize(&small).expect("centered smaller grid");
    let (pad_x, pad_y) = host.origin().expect("host origin");
    let cell_origin_x = (4 - 2) / 2;
    let cell_origin_y = (4 - 1) / 2;
    let left = pad_x + cell_origin_x as u32 * 2 * host.pitch;
    let right = pad_x + (cell_origin_x as u32 + 1) * 2 * host.pitch;
    let y = pad_y + cell_origin_y as u32 * 4 * host.pitch;
    assert_eq!(ink_pixels(&centered), vec![(left, y), (right, y)]);
    assert_eq!(centered.dimensions(), (host.width, host.height));
}

#[test]
fn malformed_zero_and_huge_dimensions_reject() {
    assert!(DotGeometry::new(0, 4, (8, 16), 2).is_none());
    assert!(DotGeometry::new(4, 0, (8, 16), 2).is_none());
    assert!(DotGeometry::new(4, 4, (0, 16), 2).is_none());
    assert!(DotGeometry::new(4, 4, (8, 0), 2).is_none());
    assert!(DotGeometry::new(4, 4, (8, 16), 0).is_none());
    assert!(DotGeometry::new(4, 4, (8, 16), 1).is_none());
    assert!(DotGeometry::new(4, 4, (8, 16), 9).is_none());
    assert!(DotGeometry::new(257, 1, (8, 16), 2).is_none());
    assert!(DotGeometry::new(1, 97, (8, 16), 2).is_none());
    assert!(DotGeometry::new(2048, 977, (1, 1), 2).is_none());
    assert!(DotGeometry::new(1, 1, (8, 16), 8).is_none());
    assert!(DotGeometry::new(u16::MAX, u16::MAX, (u16::MAX, u16::MAX), 2).is_none());

    let ok = DotGeometry::new(2, 2, (8, 16), 2).expect("valid");
    let mismatch = ColoredBrailleImage {
        width: 2,
        height: 2,
        cells: vec![cell('\u{28FF}', INK)],
    };
    assert!(ok.rasterize(&mismatch).is_none());
    let oversized = grid(
        ok.grid_width + 1,
        1,
        vec![cell('\u{2801}', INK); ok.grid_width + 1],
    );
    assert!(ok.rasterize(&oversized).is_none());

    let huge = DotGeometry {
        width: 10_000,
        height: 8_000,
        grid_width: 10,
        grid_height: 10,
        pitch: 4,
    };
    assert!(huge.rasterize(&single('\u{28FF}', INK)).is_none());
    let zero_pitch = DotGeometry {
        width: 16,
        height: 16,
        grid_width: 1,
        grid_height: 1,
        pitch: 0,
    };
    assert!(zero_pitch.rasterize(&single('\u{28FF}', INK)).is_none());
}

#[test]
fn non_braille_glyphs_never_become_solid_blocks() {
    let geometry = DotGeometry::new(2, 2, (8, 16), 2).expect("canvas");
    let glyphs = ['A', '█', ' ', '#', '\u{FFFD}', '\u{2593}', '\u{25A0}'];
    let cells: Vec<_> = glyphs
        .into_iter()
        .cycle()
        .take(geometry.grid_width * geometry.grid_height)
        .map(|glyph| cell(glyph, [255, 255, 255]))
        .collect();
    let raster = geometry
        .rasterize(&grid(geometry.grid_width, geometry.grid_height, cells))
        .expect("non-braille raster");
    assert!(
        ink_pixels(&raster).is_empty(),
        "non-Braille glyphs must not light any pixels"
    );
    assert!(raster.pixels().all(|pixel| pixel.0 == BG));

    let mixed = grid(
        2,
        1,
        vec![cell('█', [255, 0, 0]), cell('\u{2801}', [20, 30, 40])],
    );
    let mixed_raster = geometry.rasterize(&mixed).expect("mixed");
    assert_eq!(ink_pixels(&mixed_raster).len(), 1);
}
