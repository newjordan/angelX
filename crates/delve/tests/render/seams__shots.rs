//! The rock faces, drawn as the game draws them: a sheet of every stone at
//! every grade, then one of each won, worked and raided. Written to
//! `ANGEL_SEAM_SHOTS` (default `/tmp/seam-shots`) by an ignored test.
use super::*;
use crate::rules::geology::{Face, Grade, ROWS};
use std::path::Path;

fn save(im: &Img, path: &Path, scale: i32) {
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
#[ignore = "writes the seam sheets"]
fn write_seam_sheets() {
    let dest = std::env::var("ANGEL_SEAM_SHOTS").unwrap_or_else(|_| "/tmp/seam-shots".into());
    let dest = Path::new(&dest);
    std::fs::create_dir_all(dest).unwrap();
    // Every stone, three grades across.
    let mut sheet = Img::black(3 * 52 + 8, ROWS.len() as i32 * 42 + 8);
    for (r, row) in ROWS.iter().enumerate() {
        for (g, grade) in [Grade::Rough, Grade::Sound, Grade::Prime].into_iter().enumerate() {
            let st = State { tick: 3, ..State::default() };
            paint(&mut sheet, row.face, grade, 11 + r as u32, 30 + g as i32 * 52, 36 + r as i32 * 42, st);
        }
    }
    save(&sheet, &dest.join("01-stones-by-grade.png"), 4);
    // Each won, then worked with a camp, then struck and cut, then raided.
    let faces = [Face::Limestone, Face::Flint, Face::Basalt, Face::Slate, Face::Tufa, Face::Clay, Face::Void];
    let mut sheet = Img::black(faces.len() as i32 * 76 + 8, 4 * 42 + 8);
    for (i, face) in faces.into_iter().enumerate() {
        let x = 34 + i as i32 * 76;
        let states = [
            State { secured: true, tick: 5, ..State::default() },
            State { secured: true, camp: true, tick: 5, ..State::default() },
            State { secured: true, camp: true, cut: 4, struck: Some(3), tick: 5, ..State::default() },
            State { secured: true, raided: true, tick: 5, ..State::default() },
        ];
        for (k, st) in states.into_iter().enumerate() {
            paint(&mut sheet, face, Grade::Sound, 3 + i as u32, x, 36 + k as i32 * 42, st);
        }
    }
    save(&sheet, &dest.join("02-won-worked-raided.png"), 4);
}
