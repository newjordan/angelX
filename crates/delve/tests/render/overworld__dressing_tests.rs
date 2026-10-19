use super::*;
use crate::render::overworld::ink::{is_signal, PALETTE, rgb};

#[test]
fn every_thing_has_a_sprite_in_quiet_inks() {
    for &item in &Item::ALL {
        for flip in [false, true] {
            let im = sprite(item, flip);
            assert!(im.w >= 8 && im.h >= 6, "{item:?} is a speck: {}x{}", im.w, im.h);
            assert!(im.w <= 56 && im.h <= 44, "{item:?} is too big: {}x{}", im.w, im.h);
            let mut lit = 0;
            for y in 0..im.h {
                for x in 0..im.w {
                    if let Some(c) = im.get(x, y) {
                        assert!(!is_signal(c), "{item:?} uses a signal ink at ({x}, {y})");
                        // Nothing past a quiet grey: no stark whites.
                        assert!(c.iter().map(|&v| u32::from(v)).sum::<u32>() < 3 * 0xd0, "{item:?} is too bright at ({x}, {y}): {c:?}");
                        lit += 1;
                    }
                }
            }
            assert!(lit > 40, "{item:?} draws only {lit} pixels");
        }
    }
    assert!(PALETTE.iter().all(|&v| rgb(v).len() == 3));
}

#[test]
fn a_clearing_is_a_loose_oval_of_worn_ground() {
    let worn = (0..60)
        .flat_map(|y| (0..80).map(move |x| (x, y)))
        .filter(|&(x, y)| bay(x, y, (40, 28)).is_some())
        .count();
    assert!((250..1400).contains(&worn), "{worn} pixels worn");
    assert!(bay(0, 0, (40, 28)).is_none(), "nothing worn far off");
}

/// Every thing at four times its size on black, in rows by biome, as a PNG:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_dressing_sheet -- --ignored`.
#[test]
#[ignore]
fn write_dressing_sheet() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (cols, cell) = (6, (60, 52));
    let rows = Item::ALL.len().div_ceil(cols);
    let mut sheet = Img::black(cols as i32 * cell.0, rows as i32 * cell.1);
    for (i, &item) in Item::ALL.iter().enumerate() {
        let im = sprite(item, false);
        let (cx, cy) = ((i % cols) as i32 * cell.0, (i / cols) as i32 * cell.1);
        sheet.stamp(im, cx + (cell.0 - im.w) / 2, cy + cell.1 - im.h - 4);
    }
    let mut big = Img::black(sheet.w * 4, sheet.h * 4);
    for y in 0..big.h {
        for x in 0..big.w {
            if let Some(c) = sheet.get(x / 4, y / 4) {
                big.set(x, y, c);
            }
        }
    }
    image::RgbaImage::from_raw(big.w as u32, big.h as u32, big.rgba_bytes()).unwrap().save(dir.join("dressing-sheet.png")).unwrap();
}
