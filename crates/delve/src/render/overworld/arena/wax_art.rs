//! The Wax Maze's little green kings. Brass and faces stay crisp above a
//! translucent robe; the caller supplies the veil's half opacity.

use super::super::ink::{Img, ink, is_signal};
use crate::rules::folly::Temper;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KingMood {
    Hunt,
    Flee,
    FleeEnding,
    Rest,
}

const SIDE: i32 = 14;

fn rows(inks: &[&str]) -> Img {
    debug_assert!(
        inks.iter().flat_map(|row| row.chars()).all(|ch| {
            matches!(ch, '.' | ' ') || ink(ch).is_some_and(|colour| !is_signal(colour))
        })
    );
    Img::from_rows(inks)
}

// Light falls down the left cheek, then breaks into the beard's ribbons.
// The head sinks a pixel on B while the three tips curl outward: the wisps
// always reach row 13, so a caller can centre every pose the same way.
const ROBES: [[&str; 14]; 2] = [
    [
        "..............",
        "..............",
        "..............",
        ".....FFF......",
        "...FCCCCCFF...",
        "..FCYYYCCCeF..",
        "..FCYCCCCCeeF.",
        ".FCYCCCCCCeeF.",
        ".FCYCCYYCCeeF.",
        ".FCYCCYCYCeeF.",
        "..FCYCYCYCeF..",
        "..FCYC.CYCeF..",
        "..FCF..FCF.F..",
        "...C....C..e..",
    ],
    [
        "..............",
        "..............",
        "..............",
        "..............",
        ".....FFF......",
        "...FCCCCCFF...",
        "..FCYYYCCCeF..",
        ".FCYCCCCCeeF..",
        ".FCYCCYYCCeeF.",
        "..FCYCYCYCeF..",
        "..FCYCCYCeF...",
        "..FCYC.CYCeF..",
        ".FCF...FCF.F..",
        ".eC.....C..Ce.",
    ],
];

fn crown(temper: Temper, scared: bool, bob: i32) -> Img {
    let (x, y, drawing): (i32, i32, &[&str]) = match temper {
        // A squat, toothy circlet: four short points, a heavy brass band.
        Temper::Hound => (2, 2, &["T..t..t..O", "tOtTOtTOOn", ".nOOOOOOn."]),
        // A pinched, dagger-shaped crown with a single tall centre point.
        Temper::Cutter => (
            4,
            0,
            &["...T...", "..nTn..", "..tTO..", ".tOTOn.", ".nOOOn."],
        ),
        // The outlying points lean over the ears like little battlements.
        Temper::Flanker => (
            1,
            1,
            &[
                "Tt........Ot",
                ".tO..T..OOn.",
                "..nTtOOOOn..",
                "..nOOOOOOn..",
            ],
        ),
        // Already slipping over one eye, its band descends to the left.
        Temper::Drifter => (
            2,
            0,
            &[
                "..T.......",
                ".nTO..t...",
                "..tTOtOn..",
                "T..tOOOOn.",
                "nOtOnnnn..",
            ],
        ),
    };
    let mut out = Img::new(SIDE, SIDE);
    for (ry, row) in drawing.iter().enumerate() {
        // Fear tips the whole crown off balance without changing its shape
        // into another king's: high points right, the heavy band left.
        let lean = if scared {
            match ry {
                0 | 1 => 1,
                2 => 0,
                _ => -1,
            }
        } else {
            0
        };
        out.stamp(&rows(&[row]), x + lean, y + ry as i32 + bob);
    }
    out
}

fn eyes(temper: Temper, scared: bool) -> Img {
    if scared {
        let pupils = match temper {
            Temper::Hound => "WfW..WfW",
            Temper::Cutter => "WWf..WWf",
            Temper::Flanker => "fWW..fWW",
            Temper::Drifter => "WfW..WWf",
        };
        rows(&["WWW..WWW", pupils, ".WW..WW."])
    } else {
        rows(match temper {
            Temper::Hound => &["ff....ff", ".Wf..fW."],
            Temper::Cutter => &[".ff...ff", "..Wf.Wf."],
            Temper::Flanker => &[".ff..ff.", "fWH.fWH."],
            Temper::Drifter => &[".ff..ff.", ".Hf..Hf."],
        })
    }
}

/// A wraith king: (veil, solid). The veil (robe, body, wisps) is drawn at
/// half opacity by the caller; the solid (crown, eyes, facial lines and
/// Cutter's blade) at full. Both are 14x14, centred on the wraith's position;
/// living robes trail to the bottom row. Rest leaves the veil empty.
pub fn king(temper: Temper, mood: KingMood, tick: u32) -> (Img, Img) {
    let frame = (tick / 6 % 2) as usize;
    let bob = frame as i32;
    let scared = matches!(mood, KingMood::Flee | KingMood::FleeEnding);
    let mut solid = crown(temper, scared, bob);
    solid.stamp(&eyes(temper, scared), 3, 5 + bob);
    if mood == KingMood::Rest {
        return (Img::new(SIDE, SIDE), solid);
    }

    let mut veil = rows(&ROBES[frame]);
    let beard: &[&str] = match temper {
        Temper::Hound => &["eCYYYYCe", ".eCYCCe.", "..eYYe.."],
        Temper::Cutter => &["eCCCCYCe", ".eCCYCe.", "..eCYe..", "...Ye..."],
        Temper::Flanker => &["YCfCCfCY", "eYYCCYYe", ".eYCYCe."],
        Temper::Drifter => &["eCCYCCCe", ".eCYCCe.", "..CYYCe.", "...CYe.."],
    };
    veil.stamp(&rows(beard), 3, 8 + bob);

    if scared {
        // A small open scream under enormous eyes. The brass stays brass:
        // even a frightened king keeps his unmistakable crown.
        solid.stamp(&rows(&[".ff.", "fKKf", ".ff."]), 5, 8 + bob);
        veil = if mood == KingMood::FleeEnding && (tick / 4).is_multiple_of(2) {
            veil.recolor(&[('F', 'h'), ('e', 'i'), ('C', 'H'), ('Y', 'W'), ('f', 'h')])
        } else {
            // Swap the old shadow first: F's new green-grey contour must
            // not be swept up in the e -> h replacement.
            veil.recolor(&[('e', 'h'), ('F', 'e'), ('C', 'i'), ('Y', 'H'), ('f', 'j')])
        };
    } else {
        let mouth: &[&str] = match temper {
            Temper::Hound => &[".fHHf.", "..ff.."],
            Temper::Cutter => &["....f.", "..ff.."],
            Temper::Flanker => &["...ff."],
            Temper::Drifter => &["...f.."],
        };
        solid.stamp(&rows(mouth), 4, 8 + bob);
        if temper == Temper::Cutter {
            // A tiny straight blade, with its pale edge above a brass hilt.
            solid.stamp(&rows(&[".W", "V.", "tO", "n."]), 12, 7 + bob);
        }
    }
    (veil, solid)
}

/// A drop of candle wax, the pellet: a 4x4 tear with a cream glint.
pub fn wax(tick: u32) -> Img {
    rows(&[
        ".T..",
        if (tick / 12).is_multiple_of(2) {
            "THt."
        } else {
            "TTt."
        },
        "tTOn",
        ".nn.",
    ])
}

/// A vial of holy water, the power pellet: 8x10, with four bob/glint frames.
pub fn holy(tick: u32) -> Img {
    let frame = (tick / 8 % 4) as usize;
    let bottle = rows(&[
        "..nOn...", "..tTn...", "..vVs...", ".vWVVx..", "sVWzQVs.", "sVzQqVs.", "svzqQus.",
        ".svVvs..", "..sss...",
    ]);
    let mut out = Img::new(8, 10);
    let bob = [1, 0, 0, 1][frame];
    out.stamp(&bottle, 0, bob);
    // A cross-shaped etching in the glass, and a moving specular point.
    out.stamp(&rows(&[".H.", "HVH", ".H."]), 2, 4 + bob);
    let (x, y) = [(2, 3), (3, 2), (5, 4), (4, 6)][frame];
    out.put(x, y + bob, 'W');
    out
}

#[cfg(test)]
mod tests {
    use super::super::super::ink::{Rgb, palette_index};
    use super::*;

    const TEMPERS: [Temper; 4] = [
        Temper::Hound,
        Temper::Cutter,
        Temper::Flanker,
        Temper::Drifter,
    ];
    const MOODS: [KingMood; 4] = [
        KingMood::Hunt,
        KingMood::Flee,
        KingMood::FleeEnding,
        KingMood::Rest,
    ];

    fn pixels(img: &Img) -> usize {
        (0..img.h)
            .flat_map(|y| (0..img.w).filter_map(move |x| img.get(x, y)))
            .count()
    }

    fn check(img: &Img, w: i32, h: i32) {
        assert!(img.w > 0 && img.w <= w && img.h > 0 && img.h <= h);
        for y in 0..img.h {
            for x in 0..img.w {
                if let Some(colour) = img.get(x, y) {
                    assert!(
                        palette_index(colour).is_some(),
                        "off-palette pixel at {x},{y}"
                    );
                    assert!(!is_signal(colour), "signal ink at {x},{y}");
                }
            }
        }
    }

    #[test]
    fn wax_art_stays_small_and_in_its_inks() {
        for tick in (0..48).chain([u32::MAX]) {
            for temper in TEMPERS {
                for mood in MOODS {
                    let (veil, solid) = king(temper, mood, tick);
                    check(&veil, 14, 14);
                    check(&solid, 14, 14);
                    assert_eq!((veil.w, veil.h), (solid.w, solid.h));
                    assert!(pixels(&solid) > 0);
                    if mood == KingMood::Rest {
                        assert_eq!(pixels(&veil), 0);
                    } else {
                        assert!(pixels(&veil) > 0);
                        assert!((0..veil.w).any(|x| veil.solid(x, veil.h - 1)));
                    }
                    assert!(!veil.solid(0, 0) && !solid.solid(0, 0));
                    assert!(!veil.solid(13, 13) && !solid.solid(13, 13));
                }
            }
            for (img, w, h) in [(wax(tick), 4, 4), (holy(tick), 8, 10)] {
                check(&img, w, h);
                assert!(pixels(&img) > 0);
            }
        }
        for temper in TEMPERS {
            for mood in MOODS {
                assert_ne!(king(temper, mood, 0), king(temper, mood, 6));
            }
            // The warning's four pictures include both wisp poses in both
            // palettes. Compare equal bob phases to isolate the flash.
            assert_eq!(
                king(temper, KingMood::FleeEnding, 12),
                king(temper, KingMood::Flee, 0)
            );
            assert_ne!(
                king(temper, KingMood::FleeEnding, 0).0,
                king(temper, KingMood::Flee, 0).0
            );
        }
    }

    // The sprite inks themselves remain opaque palette entries. Only this
    // review compositor (and the eventual caller) blends the veil with floor.
    fn half_over(sheet: &mut Img, veil: &Img, x: i32, y: i32) {
        for sy in 0..veil.h {
            for sx in 0..veil.w {
                if let Some(top) = veil.get(sx, sy) {
                    let floor = sheet.get(x + sx, y + sy).unwrap();
                    let colour: Rgb = std::array::from_fn(|i| {
                        ((u16::from(top[i]) + u16::from(floor[i])) / 2) as u8
                    });
                    sheet.set(x + sx, y + sy, colour);
                }
            }
        }
    }

    /// Every temperament, mood and distinct frame, nearest-neighbour at 6x.
    #[test]
    #[ignore]
    fn write_wax_art_sheet() {
        let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut sheet = Img::new(326, 192);
        sheet.rect(0, 0, sheet.w, sheet.h, 'K');
        sheet.text(8, 7, "WAX MAZE / WRAITH KINGS", 'T');
        sheet.text(8, 18, "VEIL 50% / BRASS + EYES 100%", 'h');
        let groups: [(KingMood, &str, &[u32]); 4] = [
            (KingMood::Hunt, "HUNT", &[0, 6]),
            (KingMood::Flee, "FLEE", &[0, 6]),
            (KingMood::FleeEnding, "FEAR ENDING", &[12, 6, 0, 18]),
            (KingMood::Rest, "REST", &[0, 6]),
        ];
        let mut gx = 66;
        for (mood, label, ticks) in groups {
            sheet.text(gx, 33, label, 'T');
            for (frame, &tick) in ticks.iter().enumerate() {
                let x = gx + frame as i32 * 24;
                let label = if mood == KingMood::FleeEnding && frame >= 2 {
                    if frame == 2 { "FA" } else { "FB" }
                } else if frame % 2 == 0 {
                    "A"
                } else {
                    "B"
                };
                sheet.text(x + 6, 44, label, 'h');
                for (row, temper) in TEMPERS.into_iter().enumerate() {
                    let y = 61 + row as i32 * 24;
                    sheet.frame(x - 2, y - 3, 22, 20, 'X');
                    let (veil, solid) = king(temper, mood, tick);
                    half_over(&mut sheet, &veil, x + 2, y);
                    sheet.stamp(&solid, x + 2, y);
                }
            }
            gx += ticks.len() as i32 * 24 + 4;
        }
        for (row, label) in ["HOUND", "CUTTER", "FLANKER", "DRIFTER"]
            .into_iter()
            .enumerate()
        {
            sheet.text(8, 65 + row as i32 * 24, label, 'i');
        }
        sheet.line(8, 158, 317, 158, 'X');
        sheet.text(8, 171, "WAX", 'T');
        for (frame, tick) in [0, 12].into_iter().enumerate() {
            sheet.stamp(&wax(tick), 45 + frame as i32 * 18, 173);
        }
        sheet.text(98, 171, "HOLY WATER", 'T');
        for (frame, tick) in [0, 8, 16, 24].into_iter().enumerate() {
            sheet.stamp(&holy(tick), 169 + frame as i32 * 26, 168);
        }
        image::RgbaImage::from_raw(sheet.w as u32 * 6, sheet.h as u32 * 6, sheet.rgba_scaled(6))
            .unwrap()
            .save(dir.join("wax-art-sheet.png"))
            .unwrap();
    }
}
