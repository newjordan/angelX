//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;
use crate::render::overworld::Img;

pub fn save(img: &Img, dir: &std::path::Path, name: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
}

pub fn fight(run: &mut Run) {
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
}

pub fn lay(run: &mut Run, card: &str, x: f32, y: f32) {
    let at = run.at;
    run.dungeon.rooms[at].items.push(Item {
        card: card.into(),
        x,
        y,
        held_off: None,
    });
}

/// A calm, cleared room with the knight at its middle and nothing in it.
pub fn quiet() -> Run {
    let mut run = Run::new(4, 1, None);
    fight(&mut run);
    run.calm_for_test();
    run.enemies.clear();
    run.window = None;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    run
}

pub fn frame(run: &Run) -> Img {
    // The wish board that follows a cleared room would cover the knight.
    let mut run = run.clone();
    run.window = None;
    crate::render::overworld::arena::frame(&run, 384, 224)
}
