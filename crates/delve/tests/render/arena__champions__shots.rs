//! The champions on screen: at the bar, and each in a fight with their
//! great arm, their three spells and their ultimate going.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_champion_shots -- --ignored`.

use super::*;
use crate::rules::{EnemyKind, Input, Phase, RoomKind, Run, TELEGRAPH, alehouse, champions, ults};
use std::collections::BTreeMap;

fn whole(run: &Run) -> Img {
    let mut cv = (*scenery_cached(run)).clone();
    cv.stamp(&room_layer(run, None, true), 0, 0);
    cv
}

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + super::party::BAND_H + super::party::HUD_H)
}

fn tavern(seats: u32) -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    for id in 3..=seats {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.players.retain(|&id, _| id <= seats.max(1));
    r.walk_alehouse();
    r.floor_plates.clear();
    for _ in 0..100 {
        r.step(&BTreeMap::new());
    }
    r
}

/// A champion of `kit` in a fight room, a ring of tough demons before them.
fn fight(kit: &str) -> Run {
    let mut r = Run::new(21, 1, Some("Friend"));
    let room = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(room);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    assert!(r.take_up(1, kit));
    assert!(r.take_up(2, "percival"));
    let (cx, cy) = (r.room().width() / 2.0, r.room().height() / 2.0);
    for (id, (x, y)) in [(1, (cx - 6.0, cy)), (2, (cx - 10.0, cy + 4.0))] {
        let h = r.players.get_mut(&id).unwrap();
        (h.x, h.y, h.aim_x, h.aim_y) = (x, y, 1.0, 0.0);
        h.invulnerable = 0;
        h.ult_charge = ults::ULT_FULL;
    }
    for (dx, dy) in [(3.5, 0.0), (6.0, -2.5), (8.0, 2.0), (11.0, -0.5), (4.0, 4.0)] {
        r.spawn_at_for_test(EnemyKind::Demon, cx - 6.0 + dx, cy + dy);
    }
    for e in &mut r.enemies {
        (e.hp, e.max_hp, e.seen_hp, e.age) = (4000, 4000, 4000, TELEGRAPH);
        e.sleep_until = r.tick + 100_000;
    }
    r.phase = Phase::Fighting;
    r
}

fn step(r: &mut Run, i: Input) {
    r.step(&BTreeMap::from([(1, i)]));
}

fn idle(r: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        step(r, Input::default());
    }
}

#[test]
#[ignore]
fn write_champion_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save_img = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let (w, h) = seat();
    let seat_shot = |name: &str, run: &Run| {
        let img = seat_scene(run, w, h, Some(1)).compose();
        save_img(name, &img, 3);
    };
    // The bar with the champions standing at it, and one walked up to.
    let mut run = tavern(2);
    save_img("champions-01-bar-hall", &whole(&run), 2);
    let at = alehouse::PATRONS.iter().find(|p| p.kit == "hammerer").unwrap().at;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (at.0 * 2.0, (at.1 - 1.0) * 2.0);
    run.step(&BTreeMap::new());
    seat_shot("champions-02-bar-pitch", &run);
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    run.step(&BTreeMap::new());
    seat_shot("champions-03-bar-picked", &run);
    // The Hammerer: the swing, the thrown hammer, the slam, the fury, the
    // Earthsplitter.
    let mut run = fight("hammerer");
    seat_shot("hammerer-01-stand", &run);
    step(&mut run, Input { fire: true, ..Default::default() });
    idle(&mut run, 2);
    seat_shot("hammerer-02-swing", &run);
    let mut run = fight("hammerer");
    (run.players.get_mut(&1).unwrap().aim_x, run.players.get_mut(&1).unwrap().aim_y) = (1.0, -0.5);
    step(&mut run, Input { cast: 1, aim_x: 1, aim_y: -1, ..Default::default() });
    idle(&mut run, 4);
    seat_shot("hammerer-03-storm-hammer", &run);
    let mut run = fight("hammerer");
    step(&mut run, Input { cast: 2, ..Default::default() });
    idle(&mut run, 6);
    seat_shot("hammerer-04-windup", &run);
    idle(&mut run, champions::SLAM_WINDUP - 4);
    seat_shot("hammerer-05-groundslam", &run);
    let mut run = fight("hammerer");
    step(&mut run, Input { cast: 3, ..Default::default() });
    step(&mut run, Input { fire: true, ..Default::default() });
    seat_shot("hammerer-06-brute-force", &run);
    let mut run = fight("hammerer");
    step(&mut run, Input { ult: true, ..Default::default() });
    idle(&mut run, champions::CRACK_RUN - 4);
    seat_shot("hammerer-07-earthsplitter-runs", &run);
    idle(&mut run, champions::CRACK_ERUPT - champions::CRACK_RUN + 10);
    seat_shot("hammerer-08-earthsplitter-bursts", &run);
    // The Butcher: the cleaver, the hook out and coming home, the rot, the
    // haul, Dismember.
    let mut run = fight("butcher");
    seat_shot("butcher-01-stand", &run);
    step(&mut run, Input { fire: true, ..Default::default() });
    idle(&mut run, 1);
    seat_shot("butcher-02-chop", &run);
    let mut run = fight("butcher");
    // The near ones out of the way, so the hook flies to the far one.
    run.enemies.retain(|e| e.x - run.players[&1].x > 7.0);
    step(&mut run, Input { cast: 1, ..Default::default() });
    idle(&mut run, 6);
    seat_shot("butcher-03-hook-out", &run);
    idle(&mut run, 6);
    seat_shot("butcher-04-hook-home", &run);
    let mut run = fight("butcher");
    step(&mut run, Input { cast: 2, ..Default::default() });
    idle(&mut run, 20);
    seat_shot("butcher-05-rot", &run);
    let mut run = fight("butcher");
    step(&mut run, Input { cast: 3, aim_x: 1, aim_y: 1, ..Default::default() });
    idle(&mut run, 2);
    seat_shot("butcher-06-chain-haul", &run);
    let mut run = fight("butcher");
    step(&mut run, Input { ult: true, ..Default::default() });
    idle(&mut run, 21);
    seat_shot("butcher-07-dismember", &run);
    // The Alchemist: a flask in the air, foul air, quickfire into it, the
    // elixir, the rain.
    let mut run = fight("alchemist");
    seat_shot("alchemist-01-stand", &run);
    step(&mut run, Input { fire: true, ..Default::default() });
    idle(&mut run, 6);
    seat_shot("alchemist-02-flask", &run);
    let mut run = fight("alchemist");
    step(&mut run, Input { cast: 1, ..Default::default() });
    idle(&mut run, champions::FLASK_FLIGHT + 8);
    seat_shot("alchemist-03-miasma", &run);
    step(&mut run, Input { cast: 2, ..Default::default() });
    idle(&mut run, champions::FLASK_FLIGHT + 1);
    seat_shot("alchemist-04-quickfire-flash", &run);
    let mut run = fight("alchemist");
    run.players.get_mut(&2).unwrap().hp = 40;
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    let friend = run.players.get_mut(&2).unwrap();
    (friend.x, friend.y) = (x - 3.0, y + 1.0);
    step(&mut run, Input { cast: 3, ..Default::default() });
    idle(&mut run, 4);
    seat_shot("alchemist-05-elixir", &run);
    let mut run = fight("alchemist");
    step(&mut run, Input { ult: true, ..Default::default() });
    idle(&mut run, 10);
    seat_shot("alchemist-06-rain", &run);
    idle(&mut run, 30);
    seat_shot("alchemist-07-rain-flashes", &run);
    // The Reaver: the chops, the axe out and home, the leap and its quake,
    // the bloodlust, the Red Harvest.
    let mut run = fight("reaver");
    seat_shot("reaver-01-stand", &run);
    step(&mut run, Input { fire: true, ..Default::default() });
    idle(&mut run, 2);
    seat_shot("reaver-02-chop", &run);
    let mut run = fight("reaver");
    run.enemies.retain(|e| e.x - run.players[&1].x > 5.0);
    step(&mut run, Input { cast: 1, ..Default::default() });
    idle(&mut run, 5);
    seat_shot("reaver-03-axe-out", &run);
    idle(&mut run, 12);
    seat_shot("reaver-04-axe-home", &run);
    let mut run = fight("reaver");
    step(&mut run, Input { cast: 2, ..Default::default() });
    idle(&mut run, 2);
    seat_shot("reaver-05-leap", &run);
    idle(&mut run, 8);
    seat_shot("reaver-06-leap-lands", &run);
    let mut run = fight("reaver");
    step(&mut run, Input { cast: 3, ..Default::default() });
    step(&mut run, Input { fire: true, ..Default::default() });
    seat_shot("reaver-07-bloodlust", &run);
    let mut run = fight("reaver");
    for e in run.enemies.iter_mut() {
        e.hp = e.max_hp / 5;
    }
    step(&mut run, Input { ult: true, ..Default::default() });
    idle(&mut run, 3);
    seat_shot("reaver-08-red-harvest", &run);
    // The Lancer: the thrust, the javelin, the vault, the brace, the drive.
    let mut run = fight("lancer");
    seat_shot("lancer-01-stand", &run);
    step(&mut run, Input { fire: true, ..Default::default() });
    idle(&mut run, 2);
    seat_shot("lancer-02-thrust", &run);
    let mut run = fight("lancer");
    step(&mut run, Input { cast: 1, ..Default::default() });
    idle(&mut run, 5);
    seat_shot("lancer-03-javelin", &run);
    let mut run = fight("lancer");
    step(&mut run, Input { cast: 2, ..Default::default() });
    idle(&mut run, 2);
    seat_shot("lancer-04-vault", &run);
    let mut run = fight("lancer");
    step(&mut run, Input { cast: 3, ..Default::default() });
    idle(&mut run, 8);
    seat_shot("lancer-05-brace", &run);
    let mut run = fight("lancer");
    step(&mut run, Input { ult: true, ..Default::default() });
    idle(&mut run, 4);
    seat_shot("lancer-06-skewer-drive", &run);
    idle(&mut run, 20);
    seat_shot("lancer-07-skewer-pinned", &run);
}

#[test]
fn every_champion_draws_in_every_facing_and_movement_and_their_things_draw() {
    for kit in ["hammerer", "butcher", "alchemist"] {
        let mut run = fight(kit);
        let c = champions::of(&run.players[&1]).unwrap();
        for (ax, ay) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            for stance in 0..6 {
                let h = run.players.get_mut(&1).unwrap();
                (h.aim_x, h.aim_y) = (ax, ay);
                h.champ = Default::default();
                (h.swing, h.stride) = (0, 0.0);
                match stance {
                    1 => h.champ.windup = champions::SLAM_WINDUP,
                    2 => h.swing = 3,
                    3 => (h.champ.cast, h.champ.spell) = (champions::CAST_POSE, 2),
                    4 => (h.champ.cast, h.champ.spell, h.champ.fury) = (champions::CAST_POSE, 1, 30),
                    5 => h.stride = 2.0,
                    _ => {}
                }
                let im = super::champions::figure(&run.players[&1], c);
                // Half a head over the company (22 tall to the knight's 16), the maul's headroom over that.
                assert!(im.w > 40 && im.h > 32, "{kit}: half a head over the company");
                let feet = (0..im.w).any(|x| im.get(x, im.h - 1).is_some());
                assert!(feet, "{kit} {stance}: feet on the bottom row");
            }
        }
        // Everything they throw and leave, drawn under and over.
        for key in 1..=3 {
            step(&mut run, Input { cast: key, ..Default::default() });
            idle(&mut run, 3);
        }
        step(&mut run, Input { ult: true, ..Default::default() });
        for _ in 0..40 {
            idle(&mut run, 1);
            let mut cv = Img::new(NATIVE_W, NATIVE_H);
            super::champions::under(&mut cv, &run);
            super::champions::over(&mut cv, &run);
        }
        let (w, h) = seat();
        let _ = seat_scene(&run, w, h, Some(1)).compose();
    }
}

/// The heroes beside the realm's knight, for their size: each champion and
/// the Gravecaller toward the viewer, across both ways and away, standing
/// and through every drawing of their stride and their arm, then the bar
/// with them standing at it, their busts in a seat's frame, and the
/// Gravecaller's staff head where the grave-fire flares.
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_hero_size_shots -- --ignored`.
#[test]
#[ignore]
fn write_hero_size_shots() {
    use super::class_looks::gravecaller;
    use super::knights::{self, Facing};
    use crate::rules::minions::GRAVECALLER;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save_img = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    // Rows of figures, each on the floor line, a cell apiece.
    let sheet = |rows: &[Vec<Img>]| {
        let (gap, cell) = (4, rows.iter().flatten().map(|f| f.w).max().unwrap());
        let tall: Vec<i32> = rows.iter().map(|r| r.iter().map(|f| f.h).max().unwrap_or(0)).collect();
        let wide = rows.iter().map(|r| r.len() as i32).max().unwrap() * (cell + gap) + gap;
        let high = tall.iter().map(|t| t + gap * 2).sum::<i32>();
        let mut im = Img::new(wide, high);
        im.rect(0, 0, wide, high, 'K');
        let mut y = 0;
        for (row, t) in rows.iter().zip(&tall) {
            let floor = y + gap + t;
            im.rect(0, floor, wide, 1, 'g');
            // The knight's sixteen pixels, and the heroes' twenty-two.
            for (k, ink) in [(16, 'X'), (22, 'Z')] {
                for x in (0..wide).step_by(3) {
                    im.put(x, floor - k, ink);
                }
            }
            for (i, f) in row.iter().enumerate() {
                let cx = gap + i as i32 * (cell + gap) + cell / 2;
                im.stamp(f, cx - f.w / 2, floor - f.h);
            }
            y += t + gap * 2;
        }
        im
    };
    let facings = [(0.0, 1.0), (1.0, 0.0), (-1.0, 0.0), (0.0, -1.0)];
    let draw = |h: &crate::rules::Hero, casting: bool| {
        let im = knights::figure_casting(h, 1, false, Default::default(), casting);
        if h.aim_x < -0.3 { im.flip_h() } else { im }
    };
    let mut plain = fight("hammerer").players[&2].clone();
    (plain.knight, plain.class) = (None, None);
    let knight_at = |ax: f32, ay: f32| {
        let mut k = plain.clone();
        (k.aim_x, k.aim_y) = (ax, ay);
        let im = knights::figure(&k, 1);
        if ax < -0.3 { im.flip_h() } else { im }
    };
    // Each hero standing beside the knight, all four ways.
    let mut line: Vec<Vec<Img>> = Vec::new();
    for kit in ["hammerer", "butcher", "alchemist", GRAVECALLER] {
        let mut r = fight(kit);
        let mut row = Vec::new();
        for (ax, ay) in facings {
            let h = r.players.get_mut(&1).unwrap();
            (h.aim_x, h.aim_y, h.fire_cooldown) = (ax, ay, 0);
            row.push(knight_at(ax, ay));
            row.push(draw(&r.players[&1], false));
        }
        line.push(row);
    }
    save_img("hero-size-01-lineup", &sheet(&line), 5);
    // Every drawing of each champion, a facing to a row, the knight first.
    for kit in ["hammerer", "butcher", "alchemist"] {
        let mut r = fight(kit);
        let mut rows = Vec::new();
        for (ax, ay) in facings {
            let mut row = vec![knight_at(ax, ay)];
            for stance in 0..12 {
                let h = r.players.get_mut(&1).unwrap();
                (h.aim_x, h.aim_y) = (ax, ay);
                h.champ = Default::default();
                (h.swing, h.stride, h.fire_cooldown) = (0, 0.0, 0);
                match stance {
                    1..=4 => h.stride = 0.1 + (stance - 1) as f32 * 0.85,
                    5 => h.champ.windup = champions::SLAM_WINDUP,
                    6 => h.swing = 5,
                    7 => h.swing = 3,
                    8 => h.swing = 1,
                    9 => (h.champ.cast, h.champ.spell) = (champions::CAST_POSE, 2),
                    10 => (h.champ.cast, h.champ.spell) = (champions::CAST_POSE, 1),
                    11 => (h.champ.cast, h.champ.spell, h.champ.fury) = (champions::CAST_POSE, 3, 30),
                    _ => {}
                }
                row.push(draw(&r.players[&1], false));
            }
            rows.push(row);
        }
        save_img(&format!("hero-size-02-{kit}-frames"), &sheet(&rows), 4);
    }
    // The Gravecaller: standing, his stride, the strike, casting, bare.
    let mut r = fight(GRAVECALLER);
    let mut rows = Vec::new();
    for (ax, ay) in facings {
        let mut row = vec![knight_at(ax, ay)];
        for stance in 0..8 {
            let h = r.players.get_mut(&1).unwrap();
            (h.aim_x, h.aim_y, h.swing, h.stride, h.fire_cooldown) = (ax, ay, 0, 0.0, 0);
            match stance {
                1..=4 => h.stride = 0.1 + (stance - 1) as f32 * 0.85,
                5 => h.fire_cooldown = 5,
                _ => {}
            }
            let casting = stance == 6;
            let mut im = draw(&r.players[&1], casting);
            if casting {
                // Where the grave-fire flares, against the feet's middle.
                let face = knights::facing(ax, ay);
                let (mut sx, sy) = gravecaller::staff_head(face, true);
                if ax < -0.3 && face == Facing::Side {
                    sx = -sx;
                }
                let (fx, fy) = (im.w / 2 + sx, im.h - 1 + sy);
                for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                    im.put(fx + dx, fy + dy, '9');
                }
            }
            if stance == 7 {
                im = gravecaller::figure(knights::facing(ax, ay), super::class_looks::Pose::Stand, false);
                if ax < -0.3 {
                    im = im.flip_h();
                }
            }
            row.push(im);
        }
        rows.push(row);
    }
    save_img("hero-size-02-gravecaller-frames", &sheet(&rows), 4);
    // The bar, the champions and the Gravecaller standing at it.
    let run_bar = tavern(2);
    let hall = whole(&run_bar);
    save_img("hero-size-03-bar-hall", &hall, 2);
    let (bx, by) = super::at(10.0 * 2.0, 4.0 * 2.0);
    let (ex, ey) = super::at(29.0 * 2.0, 15.5 * 2.0);
    save_img("hero-size-04-bar-close", &Img::from_view(&hall, bx, by, ex - bx, ey - by), 4);
    // Their busts as a seat's frame crops them, and as the intro shows them.
    let mut busts = Vec::new();
    for kit in ["hammerer", "butcher", "alchemist", GRAVECALLER] {
        let r = fight(kit);
        let p = knights::portrait(&r.players[&1], 1);
        busts.push(super::party::bust(&p));
        busts.push(p);
        busts.push(knights::of_kit(Some(kit), &[]));
    }
    busts.push(super::party::bust(&knights::portrait(&plain, 1)));
    busts.push(knights::portrait(&plain, 1));
    save_img("hero-size-05-busts", &sheet(&[busts]), 5);
    // Casting in the fight, the flare on the staff's head.
    let (w, h) = seat();
    let field = fight(GRAVECALLER);
    for (name, (ax, ay)) in [("down", (0.0, 1.0)), ("side", (1.0, 0.0)), ("up", (0.0, -1.0))] {
        let mut f = field.clone();
        let hero = f.players.get_mut(&1).unwrap();
        (hero.aim_x, hero.aim_y) = (ax, ay);
        hero.spell_cooldowns = [0; 4];
        f.step(&BTreeMap::from([(1, Input { cast: 1, aim_x: ax as i8, aim_y: ay as i8, ..Default::default() })]));
        f.step(&BTreeMap::new());
        save_img(&format!("hero-size-06-cast-{name}"), &seat_scene(&f, w, h, Some(1)).compose(), 3);
    }
}
