//! The kingdom's roads on screen: the waystations, the fingerposts and the
//! dungeon mouths. `ANGEL_ARENA_SHOTS=<dir>
//! cargo test write_roads_shots -- --ignored`.

use super::*;
use crate::rules::Run;
use std::collections::BTreeMap;

fn save(dir: &std::path::Path, name: &str, img: &Img) {
    image::RgbaImage::from_raw(img.w as u32 * 2, img.h as u32 * 2, img.rgba_scaled(2))
        .unwrap()
        .save(dir.join(format!("{name}.png")))
        .unwrap();
}

/// A lone knight standing at tile `(x, y)` on the kingdom, a moment settled.
fn at(tile: (f32, f32)) -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (tile.0 * 2.0, tile.1 * 2.0);
    hero.camera = Some((hero.x, hero.y - 1.0));
    for _ in 0..70 {
        run.step(&BTreeMap::new());
    }
    run
}

fn dir() -> Option<std::path::PathBuf> {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS")?);
    std::fs::create_dir_all(&dir).unwrap();
    Some(dir)
}

#[test]
#[ignore]
fn write_roads_shots() {
    let Some(dir) = dir() else {
        return;
    };
    halts(&dir);
    forks(&dir);
    arrival(&dir);
}

/// Four knights step down from the coach at the next halt: side by side on
/// the apron, the lantern burning, the dust still in the air.
fn arrival(dir: &std::path::Path) {
    let mut run = Run::at_home(41, 4, None, Default::default(), Default::default());
    for id in 2..=4 {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.walk_kingdom(None);
    for id in 1..=4 {
        beside(&mut run, id, "Midway Halt");
        let hero = run.players.get_mut(&id).unwrap();
        hero.x += id as f32 * 1.6 - 4.0;
    }
    for _ in 0..3 {
        run.step(&BTreeMap::new());
    }
    press(&mut run, 1);
    press(&mut run, 1);
    run.step(&BTreeMap::from([(1, crate::rules::Input { cast: 1, ..Default::default() })]));
    for _ in 0..4 {
        run.step(&BTreeMap::new());
    }
    save(dir, "roads-11-coach-arrival", &frame_for(&run, 384, 224, Some(1)));
}

/// The signposts at a fork by a way down, and the way down's own sign.
fn forks(dir: &std::path::Path) {
    let geo = crate::render::overworld::kingdom::geography(&[]);
    for (name, shot) in [("The Delve", "roads-08-fork-delve"), ("The Mines", "roads-09-fork-mines"), ("The Swamp", "roads-10-fork-swamp")] {
        let m = crate::rules::world::MOUTHS.iter().find(|m| m.name == name).unwrap();
        let site = geo.sites.iter().find(|s| s.room == m.room).unwrap();
        let mut run = at((site.stand.0 as f32 + 3.5, site.stand.1 as f32 + 4.5));
        run.home.deepest = 4;
        run.step(&BTreeMap::new());
        save(dir, shot, &frame_for(&run, 384, 224, Some(1)));
    }
}

fn press(run: &mut Run, id: u32) {
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(id, crate::rules::Input { interact: true, ..Default::default() })]));
}

/// Knight `id` at halt `name`'s apron, beside its keeper.
fn beside(run: &mut Run, id: u32, name: &str) {
    let i = crate::world::waystations::STOPS.iter().position(|s| s.name == name).unwrap();
    let p = crate::world::waystations::placed().iter().find(|p| p.stop == i).unwrap();
    let hero = run.players.get_mut(&id).unwrap();
    (hero.x, hero.y) = ((p.keeper.0 as f32 + 0.5) * 2.0 + 3.0, (p.keeper.1 as f32 + 1.9) * 2.0);
    hero.camera = Some((hero.x, hero.y - 1.0));
    hero.stone = false;
}

/// A halt as a party meets it: nobody near, the keeper's key, the greeting,
/// where to, then a rally and a coach's lantern.
fn halts(dir: &std::path::Path) {
    let mut run = Run::at_home(41, 2, None, Default::default(), Default::default());
    run.join_seat(2, "Sir Boots");
    run.walk_kingdom(None);
    let at = crate::world::waystations::STOPS.iter().position(|s| s.name == "Coastgate Halt").unwrap();
    let p = crate::world::waystations::placed()[at];
    beside(&mut run, 1, "Coastgate Halt");
    let hero = run.players.get_mut(&1).unwrap();
    hero.x -= 6.0;
    hero.y += 3.0;
    hero.camera = Some((hero.x, hero.y - 6.0));
    let cam = hero.camera;
    let hero = run.players.get_mut(&2).unwrap();
    (hero.x, hero.y) = ((p.rect.tx as f32 + 5.2) * 2.0, (p.rect.ty as f32 + 4.6) * 2.0);
    hero.camera = cam;
    for _ in 0..70 {
        run.step(&BTreeMap::new());
    }
    save(dir, "roads-02-halt", &frame_for(&run, 384, 224, Some(1)));
    beside(&mut run, 1, "Coastgate Halt");
    run.step(&BTreeMap::new());
    save(dir, "roads-03-halt-key", &frame_for(&run, 384, 224, Some(1)));
    press(&mut run, 1);
    save(dir, "roads-04-halt-greeting", &frame_for(&run, 384, 224, Some(1)));
    press(&mut run, 1);
    save(dir, "roads-05-halt-where-to", &frame_for(&run, 384, 224, Some(1)));
    let hero = run.players.get_mut(&2).unwrap();
    (hero.x, hero.y) = (hero.x - 150.0, hero.y + 60.0);
    hero.camera = Some((hero.x, hero.y - 1.0));
    run.step(&BTreeMap::from([(1, crate::rules::Input { cast: 3, ..Default::default() })]));
    for _ in 0..6 {
        run.step(&BTreeMap::new());
    }
    save(dir, "roads-06-halt-rallied", &frame_for(&run, 384, 224, Some(1)));
    // The party's other knight, far off, sees the arrow.
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    save(dir, "roads-07-rally-arrow", &frame_for(&run, 384, 224, Some(2)));
}
