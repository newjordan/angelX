use super::*;
use crate::render::overworld::ink::{BLACK, ink, palette_index};

fn on_palette(img: &Img) -> bool {
    img.pixels().all(|c| c == BLACK || palette_index(c).is_some())
}

const TONES: [Tone; 17] = [
    Tone::SLATE,
    Tone::KEY,
    Tone::PLATE,
    Tone::PAINTED,
    Tone::NAMEPLATE,
    Tone::GLASS,
    Tone::PIP,
    Tone::CHALK,
    Tone::NIGHT_SIGN,
    Tone::BARE,
    Tone::KEYCAP,
    Tone::BRASS,
    Tone::FRAMED,
    Tone::SIGN,
    Tone::TIMBER,
    Tone::EMBER,
    Tone::WELL,
];

#[test]
fn every_box_is_on_the_realm_palette() {
    for tone in TONES {
        for (w, h) in [(3, 3), (9, 10), (40, 11), (60, 20), (120, 48), (368, 208)] {
            let mut im = Img::new(w + 2, h + 2);
            panel(&mut im, (0, 0, w, h), tone);
            assert!(on_palette(&im), "{tone:?} {w}x{h}");
        }
    }
    let mut im = Img::new(40, 20);
    band(&mut im, (0, 0, 40, 20), ink('B').unwrap());
    rule(&mut im, 2, 30, 4);
    highlight(&mut im, (2, 8, 30, 9));
    meter(&mut im, (2, 15, 30, 4), 12, ink('5').unwrap());
    keycap(&mut im, (32, 2, 7, 11));
    chip(&mut im, (32, 14, 7, 5));
    ring(&mut im, (0, 0, 40, 20), 2);
    assert!(on_palette(&im));
    let mut im = Img::new(40, 20);
    for (i, (lit, open)) in [(true, true), (false, true), (false, false)].into_iter().enumerate() {
        floor_plate(&mut im, (i as i32 * 13, 0, 12, 12), lit, open);
        studs(&mut im, (i as i32 * 13, 0, 12, 12), 'O');
        pip(&mut im, (i as i32 * 13, 13, 7, 5), lit);
    }
    assert!(on_palette(&im));
    for ch in "BbPrOo4a567gjGJhuUSxKq".chars() {
        let mut im = Img::new(20, 12);
        board(&mut im, (0, 0, 19, 11), ch);
        assert!(on_palette(&im), "a board framed in {ch}");
    }
}

#[test]
fn corners_are_cut_and_the_box_is_symmetric() {
    for (w, h) in [(9, 10), (40, 11), (60, 20), (120, 48)] {
        let mut im = Img::new(w, h);
        inset_panel(&mut im, (0, 0, w, h), Tone::SLATE);
        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            assert!(im.get(x, y).is_none(), "{w}x{h}: corner {x},{y} is cut");
        }
        // The middle of each edge is the box.
        for (x, y) in [(w / 2, 0), (w / 2, h - 1), (0, h / 2), (w - 1, h / 2)] {
            assert!(im.get(x, y).is_some(), "{w}x{h}: edge {x},{y}");
        }
        for y in 0..h {
            for x in 0..w {
                assert_eq!(
                    im.get(x, y).is_some(),
                    im.get(w - 1 - x, y).is_some(),
                    "{w}x{h}: mirrored at {x},{y}"
                );
                assert_eq!(
                    im.get(x, y).is_some(),
                    im.get(x, h - 1 - y).is_some(),
                    "{w}x{h}: flipped at {x},{y}"
                );
            }
        }
    }
}

#[test]
fn the_rim_is_lit_above_and_shaded_below() {
    let mut im = Img::new(60, 20);
    inset_panel(&mut im, (0, 0, 60, 20), Tone::BRASS);
    assert_eq!(im.get(30, 0), ink('O'));
    assert_eq!(im.get(0, 10), ink('o'));
    assert_eq!(im.get(30, 19), ink('r'));
    // The fill falls toward black: its top row lighter than its foot.
    let lum = |c: Rgb| c.iter().map(|&v| u32::from(v)).sum::<u32>();
    assert!(lum(im.get(30, 2).unwrap()) > lum(im.get(30, 17).unwrap()));
}

#[test]
fn the_shadow_falls_only_outside_the_box() {
    let mut bare = Img::new(50, 20);
    inset_panel(&mut bare, (2, 2, 40, 12), Tone::SLATE);
    let mut shaded = Img::new(50, 20);
    panel(&mut shaded, (2, 2, 40, 12), Tone::SLATE);
    for y in 0..20 {
        for x in 0..50 {
            if let Some(c) = bare.get(x, y) {
                assert_eq!(shaded.get(x, y), Some(c), "the box itself at {x},{y}");
            } else if shaded.get(x, y).is_some() {
                // Only below and to the right of the box, a pixel off it.
                assert!(x >= 3 && y >= 3 && x <= 42 && y <= 14, "shadow at {x},{y}");
            }
        }
    }
    assert!(shaded.get(42, 8).is_some() && shaded.get(20, 14).is_some());
}

#[test]
fn a_label_keeps_its_old_place_and_words() {
    let mut im = Img::new(80, 20);
    let rect = label(&mut im, (10, 5), "E: TALK", '6', Tone::KEY);
    assert_eq!(rect, (7, 3, text_width("E: TALK") + 6, 11));
    // The words are the 5x7 hand's, in the brass a prompt wears at dusk
    // (a bright gold letter steps down to it, `dusk_mark`).
    let mut words = Img::new(80, 20);
    words.text(10, 5, "E: TALK", '6');
    for y in 0..20 {
        for x in 0..80 {
            if words.get(x, y).is_some() {
                assert_eq!(im.get(x, y), ink('O'), "the letters at {x},{y}");
            }
        }
    }
}

#[test]
fn lettering_over_the_world_never_wears_a_bright_gold_or_white() {
    // Gold goes to brass and cream or white to grey; the quiet inks stay.
    for ch in ['4', '5', '6', '@', 'T', 't'] {
        assert_eq!(dusk_mark(ch), 'O', "{ch}");
    }
    for ch in ['9', 'c', 'w', 'H', 'W'] {
        assert_eq!(dusk_mark(ch), 'i', "{ch}");
    }
    for ch in ['O', 'o', 'i', 'h', 'J', '$', '7', 'r'] {
        assert_eq!(dusk_mark(ch), ch, "{ch}");
    }
    // Whatever it answers is no brighter than brass or the quiet grey.
    let top = |c: char| {
        let [r, g, b] = ink(c).unwrap();
        0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)
    };
    for ch in ['4', '5', '6', '@', 'T', 't', '9', 'c', 'w', 'H', 'W'] {
        assert!(top(dusk_mark(ch)) <= top('i') + 0.5, "{ch}");
    }
    // A label's letters wear it too.
    let mut im = Img::new(80, 20);
    label(&mut im, (10, 5), "E: TALK", '9', Tone::KEY);
    assert!(im.pixels().all(|p| p != ink('9').unwrap()));
}

#[test]
fn a_pictures_inks_step_down_with_its_lettering() {
    let icon = Img::from_rows(&["6.9", "4T."]);
    let at_dusk = dusk_icon(&icon);
    assert_eq!(at_dusk.get(0, 0), ink('O'));
    assert_eq!(at_dusk.get(2, 0), ink('i'));
    assert_eq!(at_dusk.get(0, 1), ink('O'));
    assert_eq!(at_dusk.get(1, 1), ink('O'));
    assert_eq!(at_dusk.get(1, 0), None);
}

#[test]
fn corners_put_back_round_a_square_fill() {
    let mut im = Img::new(20, 12);
    im.rect(0, 0, 20, 12, 'E');
    let kept = corners(&im, (2, 1, 14, 10), &[2, 1]);
    assert_eq!(kept.len(), 12, "three pixels at each of four corners");
    im.rect(2, 1, 14, 10, 'B');
    restore(&mut im, &kept);
    for (x, y) in [(2, 1), (3, 1), (2, 2), (15, 1), (15, 10), (2, 10)] {
        assert_eq!(im.get(x, y), ink('E'), "corner {x},{y} is the ground again");
    }
    assert_eq!(im.get(4, 1), ink('B'));
    assert_eq!(im.get(8, 5), ink('B'));
}

/// The kit on black paper and over scenery, three times up:
/// `ANGEL_CHROME_SHEET=<file.png> cargo test -p angel-delve write_chrome_sheet -- --ignored`.
#[test]
#[ignore]
fn write_chrome_sheet() {
    let Some(path) = std::env::var_os("ANGEL_CHROME_SHEET") else {
        return;
    };
    let (w, h) = (384, 224);
    let mut im = Img::new(w, h);
    // Half the sheet over grass and timber, the other half black paper.
    for y in 0..h {
        for x in w / 2..w {
            let c = match (x / 7 + y / 5) % 3 {
                0 => 'E',
                1 => 'l',
                _ => 'F',
            };
            im.put(x, y, if y > 150 && (x + y) % 9 < 4 { 'P' } else { c });
        }
    }
    for half in [0, w / 2] {
        let x = half + 6;
        let mut y = 4;
        for (i, tone) in TONES.iter().enumerate() {
            let text = ["THE DELVE", "E: TALK", "ENTER", "ENTER", "TOLL BRIDGE", "PARTY BEST", "DARE", "SPC"][i];
            label(&mut im, (x + 3 + (i as i32 % 2) * 92, y + 2), text, if i == 1 { '6' } else { 'O' }, *tone);
            y += if i % 2 == 1 { 14 } else { 0 };
        }
        y += 2;
        panel(&mut im, (x, y, 80, 20), Tone::FRAMED);
        im.text(x + 26, y + 2, "ENTER", '4');
        im.text(x + 6, y + 11, "HORSE RANCH", '9');
        panel(&mut im, (x + 92, y, 80, 30), Tone::TIMBER);
        im.text(x + 104, y + 4, "PARTY BEST", 'T');
        y += 36;
        panel(&mut im, (x, y, 172, 56), Tone::SIGN);
        im.text(x + 20, y + 4, "BLAISE'S HEARTH III", '9');
        im.text(x + 8, y + 13, "+30 HEALTH, ALL KNIGHTS", 'h');
        highlight(&mut im, (x + 4, y + 23, 164, 11));
        im.text(x + 8, y + 25, "> HOLD E", 'T');
        rule(&mut im, x + 6, x + 165, y + 37);
        meter(&mut im, (x + 60, y + 42, 50, 5), 20, ink('5').unwrap());
        y += 62;
        for (k, rim) in [BRASS_RIM, GOLD_RIM, COLD_RIM, BRASS_RIM].iter().enumerate() {
            panel(&mut im, (x + k as i32 * 22, y, 18, 18), Tone::WELL.rimmed(*rim));
        }
        panel(&mut im, (x + 96, y, 76, 18), Tone::EMBER);
        im.text(x + 104, y + 5, "FORTUNE", '6');
    }
    image::RgbaImage::from_raw(w as u32 * 3, h as u32 * 3, im.rgba_scaled(3))
        .unwrap()
        .save(path)
        .unwrap();
}
