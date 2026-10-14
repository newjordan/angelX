//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;
use crate::render::overworld::Img;

/// A calm, empty fight room with the knight at its middle.
pub fn quiet() -> Run {
    let mut run = Run::new(4, 1, None);
    let room = fight_room(&run);
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run.window = None;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    hero.invulnerable = 100_000;
    run
}

pub fn idle(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        run.step(&BTreeMap::new());
    }
}

pub fn fire(aim_x: i8, aim_y: i8) -> Input {
    Input {
        aim_x,
        aim_y,
        fire: true,
        ..Input::default()
    }
}

pub fn bare(run: &mut Run) {
    let at = run.at;
    for row in 1..run.room().rows - 1 {
        for col in 1..run.room().cols - 1 {
            run.dungeon.rooms[at].set_for_test(col, row, Tile::Floor);
        }
    }
}

pub fn a_foe(run: &mut Run, kind: EnemyKind, x: f32, y: f32) -> u32 {
    run.spawn_at_for_test(kind, x, y);
    let e = run.enemies.last_mut().unwrap();
    (e.hp, e.max_hp) = (5000, 5000);
    e.id
}

pub fn save(img: &Img, dir: &std::path::Path, name: &str) {
    std::fs::create_dir_all(dir).unwrap();
    image::save_buffer(
        dir.join(format!("{name}.png")),
        &img.rgb_bytes(),
        img.w as u32,
        img.h as u32,
        image::ColorType::Rgb8,
    )
    .unwrap();
}

pub fn frame(run: &Run) -> Img {
    let mut run = run.clone();
    run.window = None;
    crate::render::overworld::arena::frame(&run, 768, 448)
}

pub fn fight_room(run: &Run) -> usize {
    run.dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap()
}
