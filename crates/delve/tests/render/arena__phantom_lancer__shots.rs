//! The Phantom Lancer on screen: seated in the tavern, standing, and in a
//! fight with its images, its lance, its blink, its stance and its ultimate.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_phantom_lancer_shots -- --ignored`.

use super::*;
use crate::rules::{EnemyKind, Input, Phase, RoomKind, Run, TELEGRAPH, alehouse, champions, ults};
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

fn fight(kit: &str) -> Run {
    let mut r = Run::new(21, 1, Some("Friend"));
    let room = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(room);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    assert!(r.take_up(1, kit));
    let (cx, cy) = (r.room().width() / 2.0, r.room().height() / 2.0);
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (cx - 6.0, cy, 1.0, 0.0);
    h.invulnerable = 0;
    h.ult_charge = ults::ULT_FULL;
    for (dx, dy) in [(3.5, 0.0), (6.0, -2.5), (8.0, 2.0), (11.0, -0.5), (4.0, 4.0), (3.0, -3.5)] {
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
fn write_phantom_lancer_shots() {
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

    let run = tavern();
    let hall = whole(&run);
    save_img("phantom-01-hall", &hall, 2);
    let pl = alehouse::PATRONS.iter().find(|p| p.kit == "phantomlancer").unwrap().at;
    let lancer = alehouse::PATRONS.iter().find(|p| p.kit == "lancer").unwrap().at;
    let unit = UNIT * 2.0;
    let (cx, cy) = (((pl.0 + lancer.0) / 2.0 * unit) as i32, (pl.1 * unit) as i32);
    save_img("phantom-02-hall-seats", &crop(&hall, (cx - 90).max(0), (cy - 70).max(0), 180, 110), 5);
    let mut run = tavern();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (pl.0 * 2.0, (pl.1 - 1.0) * 2.0);
    run.step(&BTreeMap::new());
    seat_shot("phantom-03-pitch", &run);
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    run.step(&BTreeMap::new());
    seat_shot("phantom-04-picked", &run);

    let mut run = fight("phantomlancer");
    {
        let mut sheet = Img::new(3 * 66, 40);
        for (k, (ax, ay)) in [(0.0, 1.0), (1.0, 0.0), (0.0, -1.0)].into_iter().enumerate() {
            let hero = run.players.get_mut(&1).unwrap();
            (hero.aim_x, hero.aim_y) = (ax, ay);
            let c = champions::of(&run.players[&1]).unwrap();
            let im = super::champions::figure(&run.players[&1], c);
            sheet.stamp(&im, k as i32 * 66 + 2, 2);
        }
        let mut ground = Img::new(sheet.w, sheet.h);
        ground.rect(0, 0, sheet.w, sheet.h, 'S');
        ground.stamp(&sheet, 0, 0);
        save_img("phantom-05-standing", &ground, 6);
    }

    let mut run = fight("phantomlancer");
    seat_shot("phantom-06-stand", &run);
    let (hx, hy) = (run.players[&1].x, run.players[&1].y);
    run.phantoms.push(ults::Phantom::standing(1, (hx - 0.5, hy - 3.0), 300));
    run.phantoms.push(ults::Phantom::standing(1, (hx - 0.5, hy + 3.0), 300));
    run.phantoms.push(ults::Phantom::beside(1, (hx, hy), (-2.6, 0.0), 300));
    idle(&mut run, 6);
    step(&mut run, Input { fire: true, ..Default::default() });
    seat_shot("phantom-07-thrust-with-images-a", &run);
    idle(&mut run, 2);
    seat_shot("phantom-08-thrust-with-images-b", &run);

    let mut run = fight("phantomlancer");
    step(&mut run, Input { cast: 1, ..Default::default() });
    idle(&mut run, 4);
    seat_shot("phantom-09-spirit-lance-flies", &run);
    idle(&mut run, champions::SPIRIT_LIFE + 4);
    seat_shot("phantom-10-spirit-lance-image", &run);

    let mut run = fight("phantomlancer");
    step(&mut run, Input { cast: 2, ..Default::default() });
    seat_shot("phantom-11-doppelganger", &run);
    idle(&mut run, 10);
    seat_shot("phantom-12-doppelganger-after", &run);

    let mut run = fight("phantomlancer");
    step(&mut run, Input { cast: 3, ..Default::default() });
    idle(&mut run, 4);
    seat_shot("phantom-13-juxtapose", &run);
    for _ in 0..90 {
        step(&mut run, Input { fire: true, ..Default::default() });
        if !run.phantoms.is_empty() {
            break;
        }
    }
    idle(&mut run, 3);
    seat_shot("phantom-14-juxtapose-image", &run);

    let mut run = fight("phantomlancer");
    step(&mut run, Input { ult: true, ..Default::default() });
    idle(&mut run, 3);
    seat_shot("phantom-15-legion-call", &run);
    idle(&mut run, 24);
    seat_shot("phantom-16-legion-ring", &run);
    step(&mut run, Input { fire: true, ..Default::default() });
    idle(&mut run, 2);
    seat_shot("phantom-17-legion-thrust", &run);
}

#[test]
fn the_phantom_lancer_and_its_images_draw_in_every_facing() {
    let mut run = fight("phantomlancer");
    let c = champions::of(&run.players[&1]).unwrap();
    for (ax, ay) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        for stance in 0..5 {
            let h = run.players.get_mut(&1).unwrap();
            (h.aim_x, h.aim_y) = (ax, ay);
            h.champ = Default::default();
            (h.swing, h.stride) = (0, 0.0);
            match stance {
                1 => h.swing = 3,
                2 => (h.champ.cast, h.champ.spell) = (champions::CAST_POSE, 2),
                3 => h.champ.brace = 10,
                4 => h.stride = 2.0,
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
    for _ in 0..40 {
        idle(&mut run, 1);
        let (w, h) = seat();
        let _ = seat_scene(&run, w, h, Some(1)).compose();
    }
    assert!(!run.phantoms.is_empty());
}
