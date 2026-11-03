//! The dyes: the violet and pink banks the realm palette came without.
//!
//! Every reader of ink rows takes them (the atlas, a card's and a boss's
//! `art`, `draw` strips, a phrasebook, the coarse and plain steps, a tray
//! icon, a painting's cut, the hex export pxl and the painter read), and
//! nothing drawn before them moves: no realm colour lights, warms, shades
//! or quantizes onto a dye, and a dye shades only inside its own ramp.

use super::*;
use crate::render::overworld::ink::{
    self, BANKS, PALETTE, PINK, PINK_BANK, VIOLET, VIOLET_BANK, bank_colours, bank_of, dye_bank, every_colour,
    nearest_plain, palette_index,
};
use crate::render::overworld::{light, material};
use crate::rules::looks::{Slot, Sprite};
use crate::rules::{phrasebook, script};

/// The violet dye's inks, deep to pale, and the pink's.
const VIOLETS: [char; 6] = ['d', '(', ')', '[', ']', '^'];
const PINKS: [char; 5] = ['!', '?', ';', '%', '+'];

/// The sixty-three inks as they were before the dyes, in the table's order.
const REALM_INKS: [(char, u32); 63] = [
    ('k', 0x0b0a08), ('K', 0x151614), ('Z', 0x25140b), ('X', 0x322b2b), ('g', 0x3d3a36), ('j', 0x515349),
    ('G', 0x63655c), ('J', 0x7a7c72), ('h', 0x94968c), ('i', 0xb0b2a8), ('H', 0xcdcfc6), ('f', 0x0d1a0c),
    ('D', 0x182414), ('F', 0x263b1c), ('E', 0x335022), ('e', 0x35543f), ('l', 0x3f6428), ('N', 0x4b591f),
    ('m', 0x5a7330), ('L', 0x6a7b24), ('A', 0x6f8738), ('M', 0x768e39), ('y', 0x8aa34a), ('C', 0x6fa46d),
    ('Y', 0xa8c05f), ('n', 0x1a0e07), ('b', 0x331a0c), ('I', 0x452712), ('B', 0x582b13), ('p', 0x652c10),
    ('P', 0x684629), ('r', 0x784620), ('R', 0x924f1d), ('o', 0xa97639), ('O', 0xbe914b), ('t', 0xd8a95e),
    ('T', 0xe2cb8b), ('s', 0x192934), ('x', 0x22303a), ('S', 0x2f4a52), ('u', 0x456070), ('U', 0x6e8a8e),
    ('v', 0x8fa3a8), ('V', 0xb3c2c4), ('W', 0xd5dee0), ('q', 0x12516a), ('Q', 0x527597), ('z', 0x639dc2),
    ('0', 0x12414f), ('1', 0x2079a6), ('2', 0x50c6b3), ('3', 0x8fe8dc), ('w', 0xe4fff8), ('a', 0x7a4a12),
    ('4', 0xecb64a), ('@', 0xf6b450), ('5', 0xf7ca58), ('6', 0xfbd070), ('$', 0xc7b48f), ('c', 0xe3d2c3),
    ('9', 0xf4e1cb), ('8', 0x431e0f), ('7', 0xa8341f),
];

fn lum(c: Rgb) -> u32 {
    299 * u32::from(c[0]) + 587 * u32::from(c[1]) + 114 * u32::from(c[2])
}

#[test]
fn the_dyes_are_new_inks_and_every_realm_ink_keeps_its_colour() {
    for &(ch, v) in &REALM_INKS {
        assert_eq!(ink::ink(ch), Some(rgb(v)), "realm ink {ch}");
    }
    for (ch, v) in VIOLETS.iter().zip(VIOLET).chain(PINKS.iter().zip(PINK)) {
        assert_eq!(ink::ink(*ch), Some(rgb(v)), "dye {ch}");
    }
    // The realm palette is as it was: five banks of fifteen.
    assert_eq!(PALETTE.len(), 75);
    assert_eq!(every_colour().count(), 75 + VIOLET.len() + PINK.len());
    // Each dye runs from its darkest to its palest, and each step is a
    // visible one.
    for ramp in [&VIOLET[..], &PINK[..]] {
        for pair in ramp.windows(2) {
            assert!(lum(rgb(pair[1])) > lum(rgb(pair[0])) + 18_000, "{:06x} then {:06x}", pair[0], pair[1]);
        }
    }
    // Purple is purple and pink is pink: blue and red over green in the
    // middle of each ramp, red over blue in the pink.
    for &v in &VIOLET[1..5] {
        let [r, g, b] = rgb(v);
        assert!(b > r && r > g, "violet {v:06x}");
    }
    for &v in &PINK {
        let [r, g, b] = rgb(v);
        assert!(r > b && b > g, "pink {v:06x}");
    }
    assert_eq!(bank_colours(VIOLET_BANK), &VIOLET);
    assert_eq!(bank_colours(PINK_BANK), &PINK);
    assert!(bank_colours(BANKS).is_empty());
}

#[test]
fn the_ink_lists_name_every_ink_once_and_nothing_a_reader_keeps() {
    let mut seen = std::collections::BTreeSet::new();
    for ch in script::INKS.chars() {
        assert!(seen.insert(ch), "{ch} listed twice");
        assert!(ink::ink(ch).is_some(), "{ch} is listed but no ink");
        assert!(ch.is_ascii_graphic(), "{ch} is one byte, typed as itself");
    }
    // Every character with a colour is listed.
    for b in 0u8..=127 {
        let ch = char::from(b);
        assert_eq!(ink::ink(ch).is_some(), script::INKS.contains(ch), "{ch:?}");
    }
    assert_eq!(seen.len(), 63 + 11);
    // Paper, frames, icon rows, comments, pxl's own, a header's words,
    // fences and quotes are never inks.
    for ch in ['.', '|', '/', '#', '_', ' ', ',', ':', '=', '`', '"', '\'', '\\', '{', '}', '<', '>', '&', '*', '~', '-'] {
        assert!(ink::ink(ch).is_none(), "{ch:?} must stay free");
    }
}

/// The arms of `fn ink`, read from its source the way
/// `scripts/delve/inks_hex.py` reads them: `'c' => 0xRRGGBB,` a line, the
/// bank in the comment above.
#[test]
fn the_hex_export_reads_every_ink_from_the_table() {
    let src = include_str!("../../src/render/overworld/ink.rs");
    let body = &src[src.find("pub fn ink(ch: char)").unwrap()..];
    let body = &body[..body.find("_ => return None").unwrap()];
    let mut arms = Vec::new();
    let mut bank = "";
    for line in body.lines().map(str::trim) {
        if let Some(comment) = line.strip_prefix("// ") {
            bank = comment;
            continue;
        }
        let Some(rest) = line.strip_prefix('\'') else { continue };
        let mut chars = rest.chars();
        let ch = chars.next().unwrap();
        let rest = chars.as_str();
        let hex = rest.strip_prefix("' => 0x").and_then(|h| h.strip_suffix(',')).unwrap_or_else(|| panic!("{line}"));
        assert_eq!(hex.len(), 6, "{line}");
        arms.push((ch, u32::from_str_radix(hex, 16).unwrap(), bank));
    }
    assert_eq!(arms.len(), script::INKS.chars().count());
    for (ch, v, bank) in &arms {
        assert_eq!(ink::ink(*ch), Some(rgb(*v)));
        let dye = dye_bank(rgb(*v));
        assert_eq!(*bank == "violet", dye == Some(VIOLET_BANK), "{ch} under {bank}");
        assert_eq!(*bank == "pink", dye == Some(PINK_BANK), "{ch} under {bank}");
    }
    // The palette file the art tools read is the table's, dyes and all.
    let hex = include_str!("../../assets/demon-hand/inks.hex");
    let rows: Vec<(String, char)> = hex
        .lines()
        .map(|l| {
            let mut words = l.split_whitespace();
            (words.next().unwrap().to_string(), words.next().unwrap().chars().next().unwrap())
        })
        .collect();
    assert_eq!(rows.len(), arms.len());
    for ((hex, key), (ch, v, _)) in rows.iter().zip(&arms) {
        assert_eq!((hex.as_str(), *key), (format!("{v:06x}").as_str(), *ch));
    }
}

#[test]
fn the_palette_file_carries_the_dyes_as_banks_five_and_six() {
    let master: serde_json::Value =
        serde_json::from_str(include_str!("../../../../cockpit/assets/realm/palette.json")).unwrap();
    for (name, index, ramp) in [("violet", VIOLET_BANK, &VIOLET[..]), ("pink", PINK_BANK, &PINK[..])] {
        let bank = &master["banks"][name];
        assert_eq!(bank["index"].as_u64(), Some(index as u64), "{name}");
        let colours: Vec<u32> = bank["colors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| u32::from_str_radix(c.as_str().unwrap().trim_start_matches('#'), 16).unwrap())
            .collect();
        assert_eq!(colours, ramp, "{name}");
    }
}

#[test]
fn dye_rows_round_trip_through_every_reader() {
    let violet = "d()[]^";
    let pink = "!?;%+.";
    // The atlas reads them.
    let img = Img::from_rows(&[violet, pink]);
    for (y, row) in [violet, pink].iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            assert_eq!(img.get(x as i32, y as i32), ink::ink(ch), "{ch}");
        }
    }
    // A card's `art` and `draw` blocks keep every row, `|` between frames.
    let card = format!(
        "name Lavender Lantern\nkind hold\ntext A lamp of plum glass.\ndamage 2\nart\n.{violet}.\n.{pink}.\n..((((..\ndraw worn at=body\n{violet}|{violet}\n{pink}|{pink}\n"
    );
    let read = script::read(&card);
    assert_eq!(read.art, [format!(".{violet}."), format!(".{pink}."), "..((((..".to_string()]);
    assert_eq!(read.sprites.len(), 1);
    assert_eq!(read.sprites[0].2, [format!("{violet}|{violet}"), format!("{pink}|{pink}")]);
    let checked = crate::rules::cards::check("lavender-lantern", &card).unwrap_or_else(|r| panic!("{:?}", r.errors));
    assert_eq!(checked.card.art[0], format!(".{violet}."));
    let worn = checked.card.looks.worn.as_ref().expect("its worn look");
    assert_eq!(worn.frames.len(), 2);
    assert!(worn.frames.iter().flatten().any(|r| r.contains('^') && r.contains('d')));
    // A boss file's `art`.
    let lawn = crate::rules::bosses::LAWN_FLAMINGO;
    let boss = format!("{}art\n{violet}\n{pink}\n", &lawn[..lawn.find("art\n").unwrap()]);
    let (boss, _) = crate::rules::bosses::check("the-plum", &boss).unwrap_or_else(|r| panic!("{:?}", r.errors));
    assert_eq!(boss.art, [violet, pink]);
    // A strip the looks reader cuts.
    let strip = Sprite::from_strip(Slot::Shot, &[format!("{violet}|{pink}")]).unwrap();
    assert_eq!(strip.frames, vec![vec![violet.to_string()], vec![pink.to_string()]]);
    // A phrasebook's art and drawn look.
    let book = format!(
        "wish plum-cloak | Plum Cloak\nsay a plum cloak\ntier armor 1 :: A cloak.\nart\n{violet}\n{pink}\n\ndraw worn at=back\n{violet}\n{pink}\n"
    );
    let (wishes, faults) = phrasebook::read_checked(&book);
    assert!(faults.is_empty(), "{faults:?}");
    assert_eq!(wishes[0].art, [violet, pink]);
    assert!(wishes[0].looks[0].2.contains(violet) && wishes[0].looks[0].2.contains(pink));
    // Shown coarse or plain, a dyed picture keeps its dyes.
    for step in [crate::rules::fidelity::Step::Coarse, crate::rules::fidelity::Step::Plain] {
        let rows = crate::rules::fidelity::rows(&["((((%%%%", "((((%%%%", "))))++++", "))))++++"], step);
        assert!(rows.iter().any(|r| r.contains('(') || r.contains(')')), "{step:?}: {rows:?}");
        assert!(rows.iter().any(|r| r.contains('%') || r.contains('+')), "{step:?}: {rows:?}");
    }
    // A card icon in the tray, its rows parted by `/`.
    let fitted = crate::rules::tray::fit(&[violet, pink]);
    assert_eq!(fitted.split('/').collect::<Vec<_>>(), [violet, pink]);
    let big: Vec<String> = (0..24).map(|y| if y % 2 == 0 { "[".repeat(24) } else { "%".repeat(24) }).collect();
    let shrunk = crate::rules::tray::fit(&big.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(shrunk.chars().any(|c| c == '[' || c == '%'), "a shrunk dye icon stays inked: {shrunk}");
    // A bare `draw` or `art` is a block's word, not a row, though spelt in inks.
    let words = script::read("name X\nkind hold\ndraw worn\n((\ndraw\nart\n))\n");
    assert_eq!(words.sprites[0].2, ["(("]);
}

/// A painting is cut to the ink nearest each pixel over every ink the
/// scripts take (`together_sheets::nearest_ink`, in the cockpit): a purple
/// one is violet now, not the steel blue it had to be, a pink one pink,
/// and a face stays a face.
#[test]
fn a_purple_painting_cuts_to_violet_a_pink_one_to_pink_and_skin_stays_skin() {
    let nearest = |c: [u8; 3]| {
        script::INKS
            .chars()
            .min_by_key(|&ch| {
                let p = ink::ink(ch).unwrap();
                (0..3).map(|i| (i32::from(p[i]) - i32::from(c[i])).pow(2)).sum::<i32>()
            })
            .unwrap()
    };
    for c in [[0x7b, 0x3f, 0xa0], [0x9b, 0x59, 0xb6], [0x5e, 0x2a, 0x7e], [0x80, 0x00, 0x80], [0xc3, 0x9b, 0xd3]] {
        assert!(VIOLETS.contains(&nearest(c)), "{c:02x?} cut to {}", nearest(c));
    }
    for c in [[0xff, 0x69, 0xb4], [0xe8, 0x7a, 0xa0], [0xc7, 0x15, 0x85], [0xff, 0xb6, 0xc1]] {
        assert!(PINKS.contains(&nearest(c)), "{c:02x?} cut to {}", nearest(c));
    }
    for c in [[0xe0, 0xac, 0x69], [0xc6, 0x86, 0x42], [0x8d, 0x55, 0x24], [0xf1, 0xc2, 0x7d], [0xd8, 0xa0, 0x80]] {
        let ch = nearest(c);
        assert!(!VIOLETS.contains(&ch) && !PINKS.contains(&ch), "skin {c:02x?} cut to {ch}");
    }
}

#[test]
fn nothing_quantized_reaches_for_a_dye_and_a_dye_shades_inside_its_own() {
    // Every derived tone stays the realm's.
    for r in (0..=255).step_by(15) {
        for g in (0..=255).step_by(15) {
            for b in (0..=255).step_by(15) {
                let c = nearest_plain([r as f32, g as f32, b as f32]);
                assert!(dye_bank(c).is_none() && palette_index(c).is_some_and(|i| i < 60));
            }
        }
    }
    // Every realm colour lights, warms and darkens as it did, onto realm
    // colours.
    for &v in &PALETTE {
        let c = rgb(v);
        for n in 0..7 {
            assert!(dye_bank(light::step_down(c, n)).is_none());
        }
        assert!(dye_bank(light::warm(c)).is_none());
        assert!(dye_bank(material::lighter(c)).is_none() && dye_bank(material::darker(c)).is_none());
        assert!(dye_bank(ink::shade(c, 0.6)).is_none());
    }
    // A dye steps down its own ramp to black, keeps its hue by the fire,
    // and a shade of it is a deeper one of the same.
    for bank in [VIOLET_BANK, PINK_BANK] {
        for &v in bank_colours(bank) {
            let c = rgb(v);
            assert_eq!(bank_of(c), Some(bank));
            for n in 0..7 {
                let d = light::step_down(c, n);
                assert!(d == BLACK || bank_of(d) == Some(bank), "{v:06x} {n}");
            }
            assert_eq!(light::warm(c), c);
            assert!(lum(light::step_down(c, 1)) < lum(c) || light::step_down(c, 1) == BLACK);
            let (up, down) = (material::lighter(c), material::darker(c));
            assert!(bank_of(up) == Some(bank) && (down == BLACK || bank_of(down) == Some(bank)));
            let s = ink::shade(c, 0.6);
            assert!(s == BLACK || bank_of(s) == Some(bank), "{v:06x}");
        }
    }
}
