//! The hero on one sheet: the realm's knight in every seat, facing and
//! pose, every stage of dress and the shield brought round, the company's
//! colours, every class in every facing, walking and striking, and the
//! same knight in the saddle, the cab and the mini-viz.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_hero_sheet -- --ignored`
//! writes `hero-sheet.png` (four times, nearest neighbour), `hero-sheet-1x.png`
//! and `hero-sheet-3x.png`.

use super::super::super::castles;
use super::super::super::ink::Img;
use super::super::super::kit::{self, Heraldry};
use super::super::class_looks::{self, Look, Pose};
use super::super::{bones, frame, sprites, strike};
use super::*;
use crate::rules::{Run, classes::CLASSES, spectacle::STRUCK};

const CELL_W: i32 = 26;
const CELL_H: i32 = 32;
const LABEL: i32 = 74;
const COLS: i32 = 20;

/// A sheet of labelled rows of figures, each standing on its cell's floor.
struct Sheet {
    im: Img,
    row: i32,
    col: i32,
}

impl Sheet {
    fn new(rows: i32) -> Sheet {
        Sheet { im: Img::new(LABEL + COLS * CELL_W, rows * CELL_H + 10), row: -1, col: 0 }
    }

    fn line(&mut self, label: &str) {
        self.row += 1;
        self.col = 0;
        let y = self.row * CELL_H + 4;
        self.im.text(2, y + CELL_H / 2 - 6, label, 'J');
    }

    /// `im` standing with its feet on the cell floor, its middle at the
    /// cell's middle (as `stand` puts a knight on the room).
    fn put(&mut self, im: &Img) {
        let (cx, floor) = (LABEL + self.col * CELL_W + CELL_W / 2, self.row * CELL_H + 4 + CELL_H - 4);
        // The floor the knight stands on: a quiet slate line.
        for x in -10..10 {
            self.im.put(cx + x, floor + 1, 'g');
        }
        self.im.stamp(im, cx - im.w / 2, floor - im.h + 1);
        self.col += 1;
    }

    /// Something drawn straight onto the sheet at the next cell's feet.
    fn draw(&mut self, paint: impl FnOnce(&mut Img, i32, i32)) {
        let (cx, floor) = (LABEL + self.col * CELL_W + CELL_W / 2, self.row * CELL_H + 4 + CELL_H - 4);
        for x in -10..10 {
            self.im.put(cx + x, floor + 1, 'g');
        }
        paint(&mut self.im, cx, floor);
        self.col += 1;
    }

    fn skip(&mut self) {
        self.col += 1;
    }
}

#[test]
fn floating_live_equipment_keeps_body_fitted_and_pivots_with_aim() {
    // The wooden curve faces the aim; the string is pulled toward the
    // knight, not out in front of the bow. At rest it is carried upright.
    for view in [realm::View::Toward, realm::View::Across, realm::View::Away] {
        for strike in [false, true] {
            let (bow, _) = realm::equipment(view, Held::Bow, strike).unwrap();
            let mut wood = Vec::new();
            let mut string = Vec::new();
            for y in 0..bow.h { for x in 0..bow.w {
                match bow.get(x, y) {
                    Some(c) if Some(c) == super::super::super::ink::ink('r') => wood.push((x, y)),
                    Some(c) if Some(c) == super::super::super::ink::ink('g') || Some(c) == super::super::super::ink::ink('j') => string.push((x, y)),
                    _ => {},
                }
            } }
            assert!(!wood.is_empty() && !string.is_empty());
            let component = |(x, y): (i32, i32)| {
                if !strike || view == realm::View::Across { x }
                else if view == realm::View::Toward { y } else { -y }
            };
            let wood_sum: i32 = wood.iter().copied().map(component).sum();
            let string_sum: i32 = string.iter().copied().map(component).sum();
            assert!(wood_sum * string.len() as i32 > string_sum * wood.len() as i32,
                "{view:?}, strike={strike}: the string must be behind the wooden curve");
        }
    }
    let run = Run::new(7, 1, Some("Friend"));
    let mut hero = hero_of(&run);
    hero.weapon = crate::rules::Weapon::Bow;
    let a = figure_timed(&hero, 1, false, Struck::default(), false, Some(0));
    let b = figure_timed(&hero, 1, false, Struck::default(), false, Some(22));
    assert_eq!(a, b, "the grip must not bob independently of the hand");
    hero.aim_y = -1.0;
    let up = figure_timed(&hero, 1, false, Struck::default(), false, Some(22));
    assert_ne!(a, up, "the bow turns to point up with the aim");
    for image in [&a, &b, &up] {
        let fit = body_of(&hero, image);
        let native = figure(&hero, 1);
        let reference = body_of(&hero, &native);
        assert_eq!(fit.bounds[0] - image.w / 2, reference.bounds[0] - native.w / 2);
        assert_eq!(image.h - fit.bounds[1], native.h - reference.bounds[1]);
    }
}

#[test]
fn floating_all_classes_and_champions_keep_body_anchors() {
    let run = Run::new(7, 1, Some("Friend"));
    let mut hero = hero_of(&run);
    for name in ["Archer", "Shieldmaiden", "Gunner", "Swordsman", "Berserker", "Mage", "Healer", "Bard", "Peasant"] {
        hero.class = Some(name.into());
        for (ax, ay) in [(0.0, 1.0), (1.0, 0.0), (0.0, -1.0), (-1.0, 0.0), (1.0, -1.0), (-1.0, 1.0)] {
            (hero.aim_x, hero.aim_y) = (ax, ay);
            for strike in [false, true] {
                let image = figure_timed(&hero, 1, false, Struck::default(), strike, Some(22));
                let fit = body_of(&hero, &image);
                let native = figure(&hero, 1);
                let reference = body_of(&hero, &native);
                assert_eq!(fit.bounds[0] - image.w / 2, reference.bounds[0] - native.w / 2, "{name}");
                assert_eq!(image.h - fit.bounds[1], native.h - reference.bounds[1], "{name}");
            }
        }
    }
    hero.class = None;
    for kit in ["gravecaller", "ironbraid", "hammerer", "butcher", "alchemist"] {
        hero.knight = Some(kit.into());
        let image = figure_timed(&hero, 1, false, Struck::default(), false, Some(22));
        let fit = body_of(&hero, &image);
        assert_eq!(fit.bounds[0] + fit.bounds[2] / 2, image.w / 2, "{kit}");
        assert_eq!(fit.bounds[1] + fit.bounds[3], image.h, "{kit}");
    }
}

#[test]
#[ignore]
fn write_floating_weapon_sheet() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return; };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = Run::new(7, 1, Some("Friend"));
    let mut base = hero_of(&run);
    base.weapon = crate::rules::Weapon::Bow;
    let cases = [
        ("BOW", None, None),
        ("CROSSBOW", None, None),
        ("HANDGONNE", None, None),
        ("SWORD", None, None),
        ("ARCHER", Some("Archer"), None),
        ("GUNNER", Some("Gunner"), None),
        ("SWORDSMAN", Some("Swordsman"), None),
        ("MAGE", Some("Mage"), None),
        ("GRAVECALLER", None, Some("gravecaller")),
        ("DWARF", None, Some(DWARF)),
        ("HAMMERER", None, Some("hammerer")),
        ("BUTCHER", None, Some("butcher")),
        ("ALCHEMIST", None, Some("alchemist")),
    ];
    let poses = [
        ("UP", 22, (0.0, -1.0), false),
        ("UP RIGHT", 22, (1.0, -1.0), false),
        ("RIGHT", 22, (1.0, 0.0), false),
        ("DOWN RIGHT", 22, (1.0, 1.0), false),
        ("DOWN", 22, (0.0, 1.0), false),
        ("DOWN LEFT", 22, (-1.0, 1.0), false),
        ("LEFT", 22, (-1.0, 0.0), false),
        ("UP LEFT", 22, (-1.0, -1.0), false),
        ("FIRE UP", 22, (0.0, -1.0), true),
        ("FIRE SIDE", 22, (1.0, 0.0), true),
        ("FIRE DOWN", 22, (0.0, 1.0), true),
    ];
    const CELL: i32 = 70;
    const ROW: i32 = 58;
    const LEFT: i32 = 86;
    let mut sheet = Img::new(LEFT + poses.len() as i32 * CELL, 22 + cases.len() as i32 * ROW);
    for (col, (label, ..)) in poses.iter().enumerate() {
        sheet.text(LEFT + col as i32 * CELL + 4, 5, label, 'J');
    }
    for (row, (label, class, knight)) in cases.into_iter().enumerate() {
        let mut hero = base.clone();
        hero.class = class.map(str::to_string);
        hero.knight = knight.map(str::to_string);
        hero.weapon = match label {
            "CROSSBOW" => crate::rules::Weapon::Crossbow,
            "HANDGONNE" => crate::rules::Weapon::Handgonne,
            "SWORD" => crate::rules::Weapon::Fists,
            _ => crate::rules::Weapon::Bow,
        };
        hero.sword = label == "SWORD";
        sheet.text(2, 46 + row as i32 * ROW, label, 'J');
        for (col, (_, tick, aim, strike)) in poses.into_iter().enumerate() {
            (hero.aim_x, hero.aim_y) = aim;
            hero.swing = if strike { 3 } else { 0 };
            let im = figure_timed(&hero, 1, false, Struck::default(), strike, Some(tick));
            let im = if aim.0 < -0.3 { im.flip_h() } else { im };
            let x = LEFT + CELL / 2 + col as i32 * CELL;
            let y = 74 + row as i32 * ROW;
            sheet.line(x - 30, y + 1, x + 30, y + 1, 'g');
            sheet.stamp(&im, x - im.w / 2, y - im.h + 1);
        }
    }
    let png = image::RgbaImage::from_raw(sheet.w as u32, sheet.h as u32, sheet.rgba_bytes()).unwrap();
    png.save(dir.join("floating-weapons-1x.png")).unwrap();
    image::imageops::resize(&png, png.width() * 3, png.height() * 3, image::imageops::FilterType::Nearest)
        .save(dir.join("floating-weapons.png")).unwrap();
}

fn hero_of(run: &Run) -> crate::rules::Hero {
    let mut h = run.players[&1].clone();
    (h.aim_x, h.aim_y, h.stride, h.swing, h.fire_cooldown) = (0.0, 1.0, 0.0, 0, 0);
    h.shielding = false;
    h.struck = 0;
    h.invulnerable = 0;
    h
}

fn aimed(h: &crate::rules::Hero, (ax, ay): (f32, f32), stride: f32) -> crate::rules::Hero {
    let mut h = h.clone();
    (h.aim_x, h.aim_y, h.stride) = (ax, ay, stride);
    h
}

/// The figure as the arena draws it, turned for the left.
fn drawn(h: &crate::rules::Hero, id: u32) -> Img {
    let im = figure_with(h, id, false);
    if h.aim_x < -0.3 { im.flip_h() } else { im }
}

const DOWN: (f32, f32) = (0.0, 1.0);
const SIDE: (f32, f32) = (1.0, 0.0);
const LEFT: (f32, f32) = (-1.0, 0.0);
const UP: (f32, f32) = (0.0, -1.0);
/// Stand, then the stride: a foot planted, passing, the other foot.
const STRIDES: [f32; 4] = [0.0, 0.5, 1.2, 2.0];

fn realm_row(sheet: &mut Sheet, base: &crate::rules::Hero, id: u32) {
    for aim in [DOWN, SIDE, UP] {
        for s in STRIDES {
            sheet.put(&drawn(&aimed(base, aim, s), id));
        }
    }
    sheet.put(&drawn(&aimed(base, LEFT, 0.0), id));
    // Loosing a shot across.
    let mut h = aimed(base, SIDE, 0.0);
    h.fire_cooldown = 4;
    sheet.put(&drawn(&h, id));
    // Hit: white, then bent back from the blow.
    for age in [1, 7] {
        let mut h = aimed(base, SIDE, 0.0);
        h.struck = STRUCK - age;
        h.struck_dir = (-1, 0);
        let (posed, dx, _) = strike::pose(&figure_with(&h, id, false), &h);
        let mut cell = Img::new(posed.w + 8, posed.h);
        cell.stamp(&posed, 4 + dx, 0);
        sheet.put(&cell);
    }
    // The roll, a frame a quarter of the way through each turn.
    for left in [9, 6, 3] {
        sheet.put(&rolled(&figure_with(&aimed(base, SIDE, 0.0), id, false), left));
    }
}

#[test]
#[ignore]
fn write_hero_sheet() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut sheet = Sheet::new(40);
    let run = Run::new(7, 1, Some("Friend"));
    let base = hero_of(&run);

    // The realm's knight in each seat's colours.
    for id in 1..=4 {
        sheet.line(&format!("SEAT {id}"));
        realm_row(&mut sheet, &base, id);
    }

    // Stages of dress, the shield, and what flies off.
    sheet.line("DRESS");
    for (gear, helm) in [(0u8, true), (1, false), (2, false)] {
        for aim in [DOWN, SIDE, UP] {
            let mut h = aimed(&base, aim, 0.0);
            h.gear = gear;
            if !helm {
                h.hp = h.max_hp / 10;
            }
            sheet.put(&drawn(&h, 1));
        }
    }
    sheet.draw(|cv, x, y| {
        let mut h = aimed(&base, SIDE, 0.0);
        h.gear = 1;
        h.gear_was = 0;
        h.pop = crate::rules::gear::POP_TICKS - 3;
        flung(cv, &h, x - 2, y, 1);
    });
    sheet.draw(|cv, x, y| {
        let mut h = aimed(&base, SIDE, 0.0);
        h.gear = 2;
        h.gear_was = 1;
        h.pop = crate::rules::gear::POP_TICKS - 3;
        flung(cv, &h, x + 2, y, -1);
    });
    sheet.line("SHIELD");
    for gear in [0u8, 2] {
        for aim in [DOWN, SIDE, UP] {
            let mut h = aimed(&base, aim, 0.0);
            h.gear = gear;
            h.shielding = true;
            sheet.put(&drawn(&h, 1));
        }
    }
    for gear in [0u8, 2] {
        let mut h = aimed(&base, SIDE, 0.0);
        h.gear = gear;
        sheet.put(&figure_with(&h, 1, true));
    }
    let mut naked = aimed(&base, SIDE, 0.0);
    naked.owned = crate::rules::gear::Owned::default();
    naked.gear = 2;
    naked.hp = naked.max_hp / 10;
    sheet.put(&figure_with(&naked, 1, true));
    // Fallen: the bones settled where the knight stood.
    sheet.draw(|cv, x, y| {
        let mut h = aimed(&base, SIDE, 0.0);
        h.hp = 0;
        bones::draw(cv, &h, 1, 400, false, x, y - 8, true);
    });
    sheet.put(&sprites::frog(0));

    // What the realm's knight holds: each arm at rest and in its strike,
    // across, toward and away; the blade as it swings.
    for (label, aim) in [("HELD ACROSS", SIDE), ("HELD TOWARD", DOWN), ("HELD AWAY", UP)] {
        sheet.line(label);
        for weapon in [
            crate::rules::Weapon::Fists,
            crate::rules::Weapon::Bow,
            crate::rules::Weapon::Crossbow,
            crate::rules::Weapon::Handgonne,
        ] {
            for fire in [0, 4] {
                let mut h = aimed(&base, aim, 0.0);
                h.weapon = weapon;
                h.forged = None;
                h.fire_cooldown = fire;
                sheet.put(&drawn(&h, 1));
            }
        }
        let mut h = aimed(&base, aim, 0.0);
        h.sword = true;
        h.swing = 5;
        sheet.put(&drawn(&h, 1));
    }

    // What a game against friends leaves on the helm: the winner's crown,
    // then each booby prize; toward the viewer and across.
    sheet.line("HATS");
    {
        use crate::rules::pastimes::Game;
        let mut crowned = aimed(&base, DOWN, 0.0);
        crowned.laurel = true;
        sheet.put(&drawn(&crowned, 1));
        for game in [Game::Pot, Game::Turnip, Game::DuckShoot, Game::Cheese, Game::Longbow, Game::Surf, Game::Broadsheet] {
            for aim in [DOWN, SIDE] {
                let mut h = aimed(&base, aim, 0.0);
                h.booby = Some(game);
                sheet.put(&drawn(&h, 1));
            }
        }
    }

    // The company: each kit's colours on the realm's knight, and the dwarf.
    sheet.line("COMPANY");
    for kit_id in [
        "percival", "lynette", "gareth", "galahad", "composer", "dispatcher", "loop", "money",
        "competition", "scryglass",
    ] {
        let mut r = Run::new(7, 1, Some("Friend"));
        r.outfit(1, crate::rules::knights::knight(kit_id).unwrap());
        let h = aimed(&hero_of(&r), SIDE, 0.0);
        sheet.put(&drawn(&h, 1));
    }
    for gear in [0u8, 1, 2] {
        let mut h = aimed(&base, DOWN, 0.0);
        h.knight = Some(DWARF.into());
        h.gear = gear;
        if gear > 0 {
            h.hp = h.max_hp / 10;
        }
        sheet.put(&drawn(&h, 1));
    }

    // Every class, toward / across / away, standing, both steps, striking.
    for look in Look::ALL {
        sheet.line(&look.name().to_uppercase());
        for facing in [Facing::Down, Facing::Side, Facing::Up] {
            for pose in [Pose::Stand, Pose::Step(0), Pose::Step(1), Pose::Step(2), Pose::Strike] {
                sheet.put(&class_looks::figure(look, facing, pose, true));
            }
        }
        sheet.put(&class_looks::figure(look, Facing::Side, Pose::Stand, true).flip_h());
        if look.shielded() {
            let mut r = Run::new(7, 1, Some("Friend"));
            r.become_class(1, CLASSES.iter().find(|c| c.name == look.name()).unwrap());
            for aim in [DOWN, SIDE] {
                let mut h = aimed(&hero_of(&r), aim, 0.0);
                h.shielding = true;
                sheet.put(&drawn(&h, 1));
            }
        }
    }
    // Each class in the four seats' trim.
    for pair in Look::ALL.chunks(4) {
        sheet.line("TRIM");
        for look in pair {
            let mut r = Run::new(7, 1, Some("Friend"));
            r.become_class(1, CLASSES.iter().find(|c| c.name == look.name()).unwrap());
            let h = aimed(&hero_of(&r), DOWN, 0.0);
            for id in 1..=4 {
                sheet.put(&figure_with(&h, id, false));
            }
            sheet.skip();
        }
    }

    // The same knight elsewhere: in the saddle at the lists, the visitor,
    // the reader, the mini-viz in two houses' colours, and each class's
    // head and coat for the saddle.
    sheet.line("ELSEWHERE");
    sheet.put(&kit::rider(Heraldry::Red));
    sheet.skip();
    sheet.put(&kit::rider(Heraldry::Blue).flip_h());
    sheet.skip();
    sheet.put(&kit::visitor());
    sheet.put(&kit::reader());
    for key in ["kimi", "deepseek", "glm"] {
        if let Some(house) = crate::world::houses::by_key(key) {
            sheet.put(&castles::dress(&kit::knight(), Some(house)));
        }
    }
    sheet.line("SADDLE");
    for look in Look::ALL {
        let (head, coat) = class_looks::saddle(look, 1);
        let mut im = Img::new(9, head.h + coat.h);
        im.stamp(&head, 0, 0);
        im.stamp(&coat, 1, head.h);
        sheet.put(&im);
    }
    sheet.line("CAB");
    for look in Look::ALL {
        for facing in [Facing::Down, Facing::Side] {
            sheet.put(&class_looks::driver_head(look, facing, 1));
        }
    }

    // Cut to the rows drawn.
    let mut im = Img::new(sheet.im.w, (sheet.row + 1) * CELL_H + 8);
    im.stamp(&sheet.im, 0, 0);
    let im = &im;
    let save = |name: &str, k: u32| {
        image::RgbaImage::from_raw(im.w as u32 * k, im.h as u32 * k, im.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    save("hero-sheet.png", 4);
    save("hero-sheet-3x.png", 3);
    save("hero-sheet-1x.png", 1);
}

/// The knights where players meet them: a crypt fight with four knights
/// (the realm's knight in two seats, two classes in the others), the
/// kingdom at the ranch. `hero-scene-*.png` at the
/// browser's 1280x720.
#[test]
#[ignore]
fn write_hero_scenes() {
    use super::super::testing;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let crypt = || {
        let mut run = testing::fight();
        run.join_seat(2, "Second");
        run.join_seat(3, "Third");
        run.join_seat(4, "Fourth");
        for (class, id) in [("Shieldmaiden", 3), ("Mage", 4)] {
            run.become_class(id, CLASSES.iter().find(|c| c.name == class).unwrap());
        }
        for (id, x, y, ax, ay) in [
            (1, 21.0, 19.0, 1.0, 0.0),
            (2, 29.0, 21.0, 0.0, 1.0),
            (3, 16.0, 15.0, 1.0, -0.2),
            (4, 33.0, 17.0, -1.0, 0.0),
        ] {
            if let Some(h) = run.players.get_mut(&id) {
                (h.x, h.y, h.aim_x, h.aim_y) = (x, y, ax, ay);
                h.invulnerable = 0;
                h.struck = 0;
                h.hp = h.max_hp;
            }
        }
        run
    };
    // The same fight worn down: helm off, smallclothes, a shield raised
    // away, a company's colours.
    let hurt = || {
        let mut run = crypt();
        for (id, gear) in [(1, 1u8), (2, 2)] {
            if let Some(h) = run.players.get_mut(&id) {
                h.gear = gear;
                h.hp = h.max_hp / 10;
            }
        }
        if let Some(h) = run.players.get_mut(&3) {
            (h.aim_x, h.aim_y, h.shielding) = (0.0, -1.0, true);
        }
        run.outfit(4, crate::rules::knights::knight("composer").unwrap());
        if let Some(h) = run.players.get_mut(&4) {
            h.class = None;
            (h.aim_x, h.aim_y) = (-1.0, 0.2);
        }
        run
    };
    save("hero-scene-crypt-v1.png", &frame(&crypt(), 1280, 720));
    save("hero-scene-crypt-hurt-v1.png", &frame(&hurt(), 1280, 720));
    save("hero-scene-kingdom-v1.png", &frame(&testing::kingdom(), 1280, 720));
}

