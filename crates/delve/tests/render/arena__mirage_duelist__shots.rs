//! The Mirage Duelist on screen: seated in the tavern, standing, and in a
//! fight with its dash, its flurry, its swap, its echo and its hall.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_mirage_duelist_shots -- --ignored`.

use super::*;
use crate::rules::champions::mirage;
use crate::rules::{EnemyKind, Input, Phase, RoomKind, Run, TELEGRAPH, Tile, alehouse, champions, ults};
use std::collections::BTreeMap;

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + super::party::BAND_H + super::party::HUD_H)
}

fn whole(run: &Run) -> Img {
    let mut cv = (*scenery_cached(run)).clone();
    cv.stamp(&room_layer(run, None, true), 0, 0);
    cv
}

fn tavern() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.players.retain(|&id, _| id <= 1);
    r.walk_alehouse();
    r.floor_plates.clear();
    for _ in 0..100 {
        r.step(&BTreeMap::new());
    }
    r
}

/// A Mirage Duelist in a fight room, three tough sleeping demons before them.
fn fight() -> Run {
    let mut r = Run::new(21, 1, Some("Friend"));
    let room = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(room);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    assert!(r.take_up(1, "mirageduelist"));
    let (cx, cy) = (r.room().width() / 2.0, r.room().height() / 2.0);
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (cx - 8.0, cy, 1.0, 0.0);
    h.invulnerable = 0;
    h.ult_charge = ults::ULT_FULL;
    for (dx, dy) in [(8.0, 0.0), (9.5, -2.5), (9.0, 2.5)] {
        r.spawn_at_for_test(EnemyKind::Demon, cx - 8.0 + dx, cy + dy);
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

fn crop(im: &Img, x0: i32, y0: i32, w: i32, h: i32) -> Img {
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some([r, g, b]) = im.get(x0 + x, y0 + y) {
                out.set(x, y, [r, g, b]);
            }
        }
    }
    out
}

#[test]
#[ignore]
fn write_mirage_duelist_shots() {
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
        let (hx, hy) = super::at(run.players[&1].x, run.players[&1].y);
        let (x0, y0) = ((hx + 12 - 64).max(0), (hy + 40 - 56).max(0));
        save_img(&format!("{name}-zoom"), &crop(&img, x0, y0, 128, 96), 6);
    };

    // The tavern: the east side, where the three of them sit in a row.
    let run = tavern();
    let hall = whole(&run);
    let at = |kit: &str| alehouse::PATRONS.iter().find(|p| p.kit == kit).unwrap().at;
    let (lancer, phantom, mirage_at) = (at("lancer"), at("phantomlancer"), at("mirageduelist"));
    let unit = UNIT * 2.0;
    let (cx, cy) = (((lancer.0 + mirage_at.0) / 2.0 * unit) as i32, (lancer.1 * unit) as i32);
    let _ = phantom;
    save_img("mirage-01-three-seats", &crop(&hall, (cx - 80).max(0), (cy - 70).max(0), 160, 110), 5);
    save_img("mirage-02-hall", &hall, 2);
    let mut run = tavern();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (mirage_at.0 * 2.0, (mirage_at.1 - 1.0) * 2.0);
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    run.step(&BTreeMap::new());
    seat_shot("mirage-03-picked", &run);

    // Standing: front, across and away.
    let run = fight();
    {
        let mut sheet = Img::new(3 * 66, 40);
        sheet.rect(0, 0, sheet.w, sheet.h, 'S');
        let mut run = run.clone();
        for (k, (ax, ay)) in [(0.0, 1.0), (1.0, 0.0), (0.0, -1.0)].into_iter().enumerate() {
            let hero = run.players.get_mut(&1).unwrap();
            (hero.aim_x, hero.aim_y) = (ax, ay);
            let c = champions::of(&run.players[&1]).unwrap();
            sheet.stamp(&super::champions::figure(&run.players[&1], c), k as i32 * 66 + 2, 2);
        }
        save_img("mirage-04-standing", &sheet, 6);
    }

    // Swashbuckle: the dash, and the flurry.
    let mut run = fight();
    step(&mut run, Input { cast: 1, ..Default::default() });
    idle(&mut run, 4);
    seat_shot("mirage-05-swashbuckle-dash", &run);
    while run.players[&1].champ.swash.dashing() {
        idle(&mut run, 1);
    }
    seat_shot("mirage-06-flurry-slash-1", &run);
    idle(&mut run, mirage::SWASH_EVERY + 2);
    seat_shot("mirage-07-flurry-slash-2", &run);

    // A bouncing dash: walled into a narrow box, the path drawn over the room.
    let mut run = fight();
    let at_room = run.at;
    let (cols, rows) = (run.room().cols, run.room().rows);
    {
        let room = &mut run.dungeon.rooms[at_room];
        for col in 0..cols {
            for row in 0..rows {
                let inside = (cols / 2 - 10..cols / 2 + 10).contains(&col) && (rows / 2 - 3..rows / 2 + 3).contains(&row);
                room.set(col, row, if inside { Tile::Floor } else { Tile::Wall });
            }
        }
    }
    run.enemies.clear();
    let (cx, cy) = (cols as f32, rows as f32);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.aim_x, hero.aim_y) = (cx + 16.0, cy, 1.0, 0.5);
    step(&mut run, Input { cast: 1, ..Default::default() });
    let mut path = vec![(run.players[&1].x, run.players[&1].y, 0u8)];
    let mut frames = Vec::new();
    for t in 0..80 {
        idle(&mut run, 1);
        let s = run.players[&1].champ.swash;
        path.push((run.players[&1].x, run.players[&1].y, s.bounces));
        if t % 8 == 3 && s.dashing() {
            frames.push(whole(&run));
        }
        if !s.dashing() && s.slashes == 0 {
            break;
        }
    }
    let mut room = whole(&run);
    for &(x, y, bounces) in &path {
        let (px, py) = super::at(x, y);
        let ink = ['3', '4', '7'][usize::from(bounces).min(2)];
        room.rect(px - 1, py - 1, 3, 3, ink);
    }
    let (x0, y0) = (((cols as f32 / 2.0 - 12.0) * UNIT * 2.0) as i32, ((rows as f32 / 2.0 - 5.0) * UNIT * 2.0) as i32);
    save_img("mirage-08-bounce-path", &crop(&room, x0.max(0), y0.max(0), (24.0 * UNIT * 2.0) as i32, (10.0 * UNIT * 2.0) as i32), 4);
    for (k, f) in frames.iter().take(5).enumerate() {
        save_img(&format!("mirage-09-bounce-frame-{k}"), &crop(f, x0.max(0), y0.max(0), (24.0 * UNIT * 2.0) as i32, (10.0 * UNIT * 2.0) as i32), 3);
    }

    // Mirror Swap with an image out, and Echo Strike mid-echo.
    let mut run = fight();
    let (hx, hy) = (run.players[&1].x, run.players[&1].y);
    run.phantoms.push(ults::Phantom::standing(1, (hx - 2.0, hy - 5.0), 300));
    step(&mut run, Input { cast: 2, ..Default::default() });
    seat_shot("mirage-10-swap", &run);
    idle(&mut run, 6);
    seat_shot("mirage-11-swap-after", &run);
    let mut run = fight();
    step(&mut run, Input { cast: 3, ..Default::default() });
    let (hx, hy) = (run.players[&1].x, run.players[&1].y);
    run.spawn_at_for_test(EnemyKind::Demon, hx + 3.0, hy);
    let i = run.enemies.len() - 1;
    (run.enemies[i].hp, run.enemies[i].max_hp, run.enemies[i].sleep_until) = (4000, 4000, run.tick + 100_000);
    step(&mut run, Input { fire: true, ..Default::default() });
    seat_shot("mirage-12-echo-thrust", &run);
    idle(&mut run, mirage::ECHO_DELAY - 1);
    seat_shot("mirage-13-echo-lands", &run);

    // The ultimate: the call, then the three at the foes.
    let mut run = fight();
    step(&mut run, Input { ult: true, ..Default::default() });
    idle(&mut run, 3);
    seat_shot("mirage-14-hall-call", &run);
    idle(&mut run, 40);
    seat_shot("mirage-15-hall-charging", &run);
    idle(&mut run, 30);
    seat_shot("mirage-16-hall-striking", &run);
}

#[test]
fn the_mirage_duelist_and_its_things_draw_in_every_facing() {
    let mut run = fight();
    let c = champions::of(&run.players[&1]).unwrap();
    for (ax, ay) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        for stance in 0..4 {
            let h = run.players.get_mut(&1).unwrap();
            (h.aim_x, h.aim_y) = (ax, ay);
            h.champ = Default::default();
            (h.swing, h.stride) = (0, 0.0);
            match stance {
                1 => h.swing = 3,
                2 => (h.champ.cast, h.champ.spell) = (champions::CAST_POSE, 1),
                3 => h.stride = 2.0,
                _ => {}
            }
            let im = super::champions::figure(&run.players[&1], c);
            assert!(im.w > 40 && im.h > 32);
            assert!((0..im.w).any(|x| im.get(x, im.h - 1).is_some()), "feet on the bottom row");
        }
    }
    for key in 1..=3 {
        step(&mut run, Input { cast: key, ..Default::default() });
        idle(&mut run, 3);
    }
    step(&mut run, Input { ult: true, ..Default::default() });
    for _ in 0..80 {
        idle(&mut run, 1);
        let (w, h) = seat();
        let _ = seat_scene(&run, w, h, Some(1)).compose();
    }
}
