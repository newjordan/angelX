//! The dyes in play: the arcane, a brew, the void and a rose burn down
//! them, a colour wished purple or pink wears its dye, the mirror has
//! violet, plum and rose cloth and lilac and pink hair, and the Lawn
//! Flamingo is pink.
//!
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_dye_shots -- --ignored`
//! writes the swatch sheet and the dyes in play, a PNG apiece.

use super::*;
use crate::render::overworld::ink::{self, PINK_BANK, VIOLET, VIOLET_BANK, dye_bank};
use crate::rules::{Input, Run, bosses, phrasebook};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The pink dye's inks, deep to pale.
const PINKS: [char; 5] = ['!', '?', ';', '%', '+'];

#[test]
fn the_lawn_flamingo_is_pink() {
    let flamingo = bosses::lawn_flamingo();
    let inks: Vec<char> = flamingo.art.iter().flat_map(|r| r.chars()).filter(|&c| c != '.').collect();
    let pink = inks.iter().filter(|c| PINKS.contains(c)).count();
    assert!(pink * 10 >= inks.len() * 7, "{pink} of {} pink", inks.len());
    // No signal ink dresses it: its colour is cloth, not a cue.
    assert!(inks.iter().all(|&c| !ink::is_signal(ink::ink(c).unwrap())), "{inks:?}");
}

#[test]
fn purple_and_pink_words_burn_down_the_dyes() {
    use crate::rules::vfx::{self, Palette};
    let dyed = |p: Palette, dye: usize| {
        let ramp = wishfx::ramp(p);
        let n = ramp.0.iter().filter(|&&c| dye_bank(ink::ink(c).unwrap()) == Some(dye)).count();
        assert!(n >= 4, "{p:?}: {:?}", ramp.0);
    };
    dyed(Palette::Arcane, VIOLET_BANK);
    dyed(Palette::Brew, VIOLET_BANK);
    dyed(Palette::Void, VIOLET_BANK);
    dyed(Palette::Rose, PINK_BANK);
    for (name, want) in [
        ("Purple Drank Armor", Palette::Brew),
        ("Lavender Haze", Palette::Brew),
        ("Violet Bolts", Palette::Brew),
        ("Amethyst Shard", Palette::Brew),
        ("Magenta Bullets", Palette::Rose),
        ("Fuchsia Fury", Palette::Rose),
        ("Bright Pink", Palette::Rose),
        ("Arcane Nova", Palette::Arcane),
    ] {
        let card = crate::rules::cards::check("words", &format!("name {name}\nkind hold\ndamage 1\n")).unwrap().card;
        let got = vfx::generated(&card).layers[0].palette;
        assert_eq!(got, want, "{name}");
    }
}

/// A wish that is only a colour, its look drawn over the whole body.
fn colour_wish(name: &str, rows: &[&str]) -> crate::rules::cards::Card {
    let text = format!("name {name}\nkind hold\nmax_hp 10\ndraw worn at=body\n{}\n", rows.join("\n"));
    let card = crate::rules::cards::check("colour", &text).unwrap_or_else(|r| panic!("{:?}", r.errors)).card;
    assert!(crate::rules::dead_wishes::colours_body(&card), "{name} colours a body");
    card
}

#[test]
fn a_colour_wished_purple_or_pink_wears_its_dye() {
    // "Bright Pink" drawn before there was a pink, in coral and cream.
    let old = colour_wish("Bright Pink", &[".7777.", "7cccc7", "7c99c7", ".7777."]);
    let ramp = wear::dye_ramp(&old).expect("a pink wish drawn without its pink");
    assert!(ramp.iter().all(|&c| dye_bank(c) == Some(PINK_BANK)));
    // Drawn in the dye already, it keeps its own drawing.
    assert!(wear::dye_ramp(&colour_wish("Bright Pink", &["%%;;", "++%%"])).is_none());
    // A pale one keeps the light end, a deep one the dark end.
    let pale = wear::dye_ramp(&colour_wish("Pale Lilac", &["hh"])).unwrap();
    let deep = wear::dye_ramp(&colour_wish("Deep Purple", &["hh"])).unwrap();
    assert!(pale.iter().all(|&c| dye_bank(c) == Some(VIOLET_BANK)) && !pale.contains(&rgb(VIOLET[0])));
    assert!(deep.iter().all(|&c| dye_bank(c) == Some(VIOLET_BANK)) && !deep.contains(&rgb(VIOLET[5])));
    // A colour that is no dye wears its drawing.
    assert!(wear::dye_ramp(&colour_wish("Bright Green", &["ll"])).is_none());
    // Over a body, every pixel off the face is the dye.
    let body = Img::from_rows(&["hhhhhhhh", "hhhhhhhh", "JJJJJJJJ", "JJJJJJJJ", "GGGGGGGG", "GGGGGGGG", "jjjjjjjj", "jjjjjjjj"]);
    let fit = crate::rules::looks::Body::human((0, 0), (8, 8));
    let tinted = wear::tinted(&body, &old, fit);
    let pinks = tinted.pixels().filter(|&c| dye_bank(c) == Some(PINK_BANK)).count();
    assert!(pinks >= 48, "{pinks} of 64 pink");
}

// ─── the shots ───────────────────────────────────────────────────────────────

fn save(img: &Img, k: u32, path: &std::path::Path) {
    image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k)).unwrap().save(path).unwrap();
}

/// The window round arena point `at`, in native pixels.
fn crop(run: &Run, at: (f32, f32), (w, h): (i32, i32)) -> Img {
    let img = frame_for(run, NATIVE_W, NATIVE_H, Some(1));
    let (ox, oy) = view_origin(run, Some(1));
    let (cx, cy) = ((at.0 * UNIT) as i32 - ox, (at.1 * UNIT) as i32 - oy);
    let x0 = (cx - w / 2).clamp(0, (img.w - w).max(0));
    let y0 = (cy - h / 2).clamp(0, (img.h - h).max(0));
    Img::from_view(&img, x0, y0, w, h)
}

/// Every bank as a strip of swatches, the realm's five over the two dyes,
/// each ink's character under its colour.
fn swatches() -> Img {
    const CELL: i32 = 22;
    let banks: [(&str, &str); 7] = [
        ("STRUCTURE", "kKZXgjGJhiH"),
        ("FOLIAGE", "fDFEelNmLAMyCY"),
        ("TIMBER", "nbIBpPrRoOtT"),
        ("STONE", "sxSuUvVWqQz"),
        ("SIGNAL", "0123wa4@56$c987"),
        ("VIOLET", "d()[]^"),
        ("PINK", "!?;%+"),
    ];
    let mut im = Img::black(16 + 15 * CELL, 8 + banks.len() as i32 * (CELL + 18));
    for (row, (name, inks)) in banks.iter().enumerate() {
        let y = 6 + row as i32 * (CELL + 18);
        im.text(8, y, name, 'h');
        for (i, ch) in inks.chars().enumerate() {
            let x = 8 + i as i32 * CELL;
            im.rect(x, y + 9, CELL - 3, CELL - 6, ch);
            im.text(x + 6, y + 9 + CELL - 4, &ch.to_string(), 'i');
        }
    }
    im
}

/// A fight room with three skeletons east of the knight, who stands west
/// of the middle facing them, kept on their marks between frames.
fn range() -> Run {
    let mut run = Run::new(11, 1, Some("Knight"));
    let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    run.dungeon.rooms[i].roster = vec![EnemyKind::Skeleton; 3];
    run.enter_for_test(i);
    for _ in 0..60 {
        hold(&mut run);
        run.step(&BTreeMap::new());
    }
    run.enemies.truncate(3);
    run.projectiles.clear();
    run
}

fn marks(run: &Run) -> ((f32, f32), [(f32, f32); 3]) {
    let (w, h) = (run.room().width(), run.room().height());
    let me = (w * 0.3, h * 0.55);
    (me, [(me.0 + 11.0, me.1 - 3.0), (me.0 + 13.0, me.1), (me.0 + 10.0, me.1 + 3.5)])
}

fn hold(run: &mut Run) {
    let (me, foes) = marks(run);
    if let Some(h) = run.players.get_mut(&1) {
        (h.x, h.y) = me;
        h.hp = h.max_hp;
    }
    for (e, at) in run.enemies.iter_mut().zip(foes) {
        (e.x, e.y) = at;
        e.age = e.age.max(100);
    }
    run.projectiles.retain(|p| !p.hostile);
}

fn step_with(run: &mut Run, input: Input) {
    hold(run);
    run.step(&BTreeMap::from([(1, input)]));
}

/// The wishes the shots grant, in players' words.
const WISHES: &str = "\
wish purple-drank-armor | Purple Drank Armor
say purple drank armor
tier armor 1 :: A sip.
tier armor 1; max_hp 10 :: A cup.
tier armor 2; max_hp 10 :: A jug.
draw worn at=body
..)[[)..
.)[]][).
)[]^^][)
)[[]][[)
)([[[[()
.)(()()).
..(dd)..
...dd...

wish arcane-nova | Arcane Nova
kind spell
say arcane nova
tier nova 40; cooldown 5 :: The air turns violet.

wish lavender-bolts | Lavender Bolts
say lavender plum bolts
tier damage 5 :: Plum.
tier damage 10 :: Plum and lavender.
tier damage 15 :: A whole orchard.

wish magenta-hearts | Magenta Hearts
say magenta heart shots
tier damage 5 :: Love.
tier damage 10 :: More love.
tier damage 15 :: Much love.

wish bright-pink | Bright Pink
say make me bright pink
tier max_hp 10 :: Pink all over.
draw worn at=body
.7777.
7cccc7
7c99c7
7cccc7
.7777.
";

fn grant(run: &mut Run, id: &str, times: u32) {
    let (book, faults) = phrasebook::read_checked(WISHES);
    let wish = book.into_iter().find(|w| w.id == id).unwrap_or_else(|| panic!("{id}: {faults:?}"));
    for _ in 0..times {
        let _ = run.grant(1, &wish);
    }
}

#[test]
#[ignore]
fn write_dye_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    save(&swatches(), 3, &dir.join("01-swatches.png"));

    // The Lawn Flamingo beside a knight.
    let mut boss = Run::new(41, 1, None);
    boss.enemies.clear();
    let at = boss.flamingo().unwrap();
    let (x, y) = (boss.players[&1].x + 10.0, boss.players[&1].y - 2.0);
    boss.spawn_boss_at_for_test(at, x, y);
    for _ in 0..10 {
        boss.step(&BTreeMap::new());
    }
    save(&frame_for(&boss, 768, 448, Some(1)), 1, &dir.join("02-flamingo-room.png"));
    let e = boss.enemies.iter().find(|e| e.boss == Some(at as u8)).unwrap();
    save(&crop(&boss, (e.x, e.y), (160, 100)), 4, &dir.join("02-flamingo.png"));
    let mut sheet = Img::black(4 * 40, 40);
    let art = Img::from_rows(&bosses::lawn_flamingo().art.iter().map(String::as_str).collect::<Vec<_>>());
    sheet.stamp(&art, 4, 4);
    sheet.stamp(&foe_material::dress(&art, EnemyKind::Boss), 44, 4);
    save(&sheet, 6, &dir.join("02-flamingo-art.png"));

    // Purple drank armour on the knight, and a pink wished over a body.
    let mut run = range();
    grant(&mut run, "purple-drank-armor", 3);
    let (me, _) = marks(&run);
    save(&crop(&run, me, (96, 64)), 6, &dir.join("03-purple-drank-armor.png"));
    save(&frame_for(&run, 768, 448, Some(1)), 1, &dir.join("03-purple-drank-armor-room.png"));
    let mut run = range();
    grant(&mut run, "bright-pink", 1);
    save(&crop(&run, me, (96, 64)), 6, &dir.join("04-bright-pink.png"));

    // An arcane spell, cast and landing.
    let mut run = range();
    grant(&mut run, "arcane-nova", 1);
    if let Some(slot) = run.players[&1].spells.iter().position(Option::is_some) {
        let (me, foes) = marks(&run);
        let mid = ((me.0 + foes[1].0) / 2.0, me.1);
        step_with(&mut run, Input { aim_x: 1, cast: slot as u8 + 1, ..Default::default() });
        for t in 0..24u32 {
            if t % 4 == 0 {
                save(&crop(&run, mid, (192, 120)), 3, &dir.join(format!("05-arcane-nova-{t:02}.png")));
            }
            step_with(&mut run, Input::default());
        }
    }

    // Wished arms in purple and pink words, firing.
    for (id, name) in [("lavender-bolts", "06-lavender-bolts"), ("magenta-hearts", "07-magenta-hearts"), ("purple-drank-armor", "08-purple-drank-shots")] {
        let mut run = range();
        grant(&mut run, id, 3);
        let (me, foes) = marks(&run);
        let mid = ((me.0 + foes[1].0) / 2.0, me.1);
        for t in 0..24u32 {
            if t % 6 == 5 {
                save(&crop(&run, mid, (192, 120)), 3, &dir.join(format!("{name}-{t:02}.png")));
            }
            step_with(&mut run, Input { aim_x: 1, fire: t < 20, ..Default::default() });
        }
    }

    // The mirror's new cloth and hair on three knights in the tavern.
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.join_seat(3, "Seat 3");
    r.players.retain(|&id, _| id <= 3);
    r.walk_alehouse();
    r.floor_plates.clear();
    for _ in 0..100 {
        r.step(&BTreeMap::new());
    }
    let ids: Vec<u32> = r.players.keys().copied().collect();
    for (n, id) in ids.iter().enumerate() {
        let h = r.players.get_mut(id).unwrap();
        (h.x, h.y) = ((9.5 + n as f32 * 2.5) * 2.0, 30.0);
        (h.aim_x, h.aim_y) = (0.0, 1.0);
        h.look.custom = true;
        h.look.primary = [8, 9, 10][n % 3];
        h.look.trim = [10, 8, 9][n % 3];
        h.look.hair = [2, 4, 3][n % 3];
        h.look.hair_colour = [6, 7, 6][n % 3];
    }
    r.step(&BTreeMap::new());
    let (x, y) = (r.players[&ids[1]].x, r.players[&ids[1]].y);
    save(&crop(&r, (x, y), (112, 64)), 5, &dir.join("09-mirror-colours.png"));
    // Every cloth and hair the mirror has, the dyes' last, one knight each.
    let mut sheet = Img::black(11 * 20, 2 * 30);
    let hero = r.players[&ids[0]].clone();
    for n in 0..11u8 {
        for (row, hair) in [(0, n % 8), (1, (n + 6) % 8)] {
            let mut h = hero.clone();
            (h.aim_x, h.aim_y) = (0.0, 1.0);
            h.look.custom = true;
            (h.look.primary, h.look.trim, h.look.hair, h.look.hair_colour) = (n, if row == 0 { n } else { 10 - n }, 2, hair);
            let im = knights::figure(&h, 1);
            sheet.stamp(&im, 2 + i32::from(n) * 20, 2 + row * 30);
        }
    }
    save(&sheet, 4, &dir.join("09-mirror-sheet.png"));
}
