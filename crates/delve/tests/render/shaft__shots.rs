//! The shaft head at each tier and the stockyard, drawn as the flat view
//! draws them, written by an ignored test to `ANGEL_SHAFT_SHOTS`.
use super::*;
use crate::rules::stockyard::{Stockyard, Tier};
use std::path::Path;

pub(crate) fn save(im: &Img, path: &Path, scale: i32) {
    let mut big = Img::black(im.w * scale, im.h * scale);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                for dy in 0..scale {
                    for dx in 0..scale {
                        big.set(x * scale + dx, y * scale + dy, c);
                    }
                }
            }
        }
    }
    image::save_buffer(path, &big.rgba_bytes(), big.w as u32, big.h as u32, image::ColorType::Rgba8).unwrap();
}

#[test]
#[ignore = "writes the shaft sheet"]
fn write_shaft_sheet() {
    let dest = std::env::var("ANGEL_SHAFT_SHOTS").unwrap_or_else(|_| "/tmp/shaft-shots".into());
    let dest = Path::new(&dest);
    std::fs::create_dir_all(dest).unwrap();
    let mut sheet = Img::black(5 * (W + 6) + 6, H + 6);
    for (i, tier) in Tier::ALL.into_iter().enumerate() {
        sheet.stamp(&head_img(tier, 17), 6 + i as i32 * (W + 6), 3);
    }
    save(&sheet, &dest.join("01-tiers.png"), 5);
    let mut yard = Stockyard::default();
    yard.tier = Tier::HorseGin;
    for (s, n) in [("limestone", 7), ("flint", 3), ("basalt", 1), ("slate", 9)] {
        yard.stack(s, n);
    }
    let img = yard_img(&yard).unwrap();
    let mut sheet = Img::black(img.w + 12, img.h + 12);
    sheet.stamp(&img, 6, 6);
    save(&sheet, &dest.join("02-yard.png"), 6);
}
