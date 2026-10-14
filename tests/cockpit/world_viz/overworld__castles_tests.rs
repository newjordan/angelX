//! The March as the live world rides it: knights out of their castles and
//! home again.

use super::castles::*;
use super::{Img, SCREEN_H, SCREEN_W, TILE};
use crate::stage::houses::{self, HouseId};

#[test]
fn every_castle_is_reachable_from_the_keep() {
    let keep = super::map::Place::Keep.stand_world();
    for id in houses::ids() {
        let path = super::live::route(keep, stand(id));
        assert_eq!(path.last(), Some(&stand(id)), "{id:?} unreachable");
        for &(x, y) in &path[..path.len() - 1] {
            assert!(
                super::kingdom::step_cost(x, y).is_some(),
                "{id:?} route crosses closed ground at {x},{y}"
            );
        }
    }
}

fn serving(lead: &str, seated: &[&str], turn: bool) -> houses::Serving {
    houses::Serving {
        lead: houses::by_key(lead),
        seated: seated.iter().filter_map(|k| houses::by_key(k)).collect(),
        turn,
    }
}

fn ride(world: &mut crate::stage::world_viz::World, ticks: usize) {
    for _ in 0..ticks {
        world.tick();
    }
}

fn knight_tile(world: &crate::stage::world_viz::World) -> (i32, i32) {
    let k = world.overworld_scene().knight;
    ((k.x / TILE as f32) as i32, (k.y / TILE as f32) as i32)
}

#[test]
fn the_serving_knight_rides_out_of_his_castle_and_home_again() {
    let mut world = crate::stage::world_viz::World::new(7);
    let kimi = houses::by_key("kimi").unwrap();
    world.note_serving(&serving("kimi", &[], false));
    ride(&mut world, 4);
    assert_eq!(
        knight_tile(&world),
        stand(kimi),
        "he starts at his own gate"
    );
    let scene = world.overworld_scene();
    assert_eq!(scene.march.lead, Some(kimi));
    assert_eq!(scene.march.raised, vec![kimi]);
    // A turn calls him out to the Keep; when it ends he rides home.
    world.note_serving(&serving("kimi", &[], true));
    world.turn_started();
    ride(&mut world, 600);
    assert_eq!(
        knight_tile(&world),
        super::map::Place::Keep.stand_world()
    );
    world.note_serving(&serving("kimi", &[], false));
    world.turn_ended(true);
    ride(&mut world, 600);
    assert_eq!(knight_tile(&world), stand(kimi), "home at his castle");
}

#[test]
fn a_change_of_club_sends_the_old_knight_home_and_a_new_one_out() {
    let mut world = crate::stage::world_viz::World::new(7);
    let (kimi, grok) = (
        houses::by_key("kimi").unwrap(),
        houses::by_key("grok").unwrap(),
    );
    world.note_serving(&serving("kimi", &[], true));
    world.turn_started();
    ride(&mut world, 600);
    world.note_serving(&serving("grok", &[], true));
    ride(&mut world, 2);
    let scene = world.overworld_scene();
    assert_eq!(scene.march.lead, Some(grok));
    assert_eq!(scene.march.raised, vec![grok], "kimi's banner comes down");
    assert_eq!(
        scene
            .march
            .riders
            .iter()
            .map(|r| r.house)
            .collect::<Vec<_>>(),
        vec![kimi],
        "kimi's knight rides home"
    );
    ride(&mut world, 900);
    assert!(
        world.overworld_scene().march.riders.is_empty(),
        "and is gone at his gate"
    );
    assert_eq!(
        knight_tile(&world),
        super::map::Place::Keep.stand_world()
    );
}

#[test]
fn a_formation_rides_out_together_and_stands_down_home() {
    let mut world = crate::stage::world_viz::World::new(7);
    world.note_serving(&serving("deepseek", &["kimi", "grok"], true));
    world.turn_started();
    ride(&mut world, 2);
    let scene = world.overworld_scene();
    assert_eq!(scene.march.raised.len(), 3, "three banners fly");
    assert_eq!(scene.march.riders.len(), 2, "two seated knights ride out");
    ride(&mut world, 900);
    let keep = super::map::Place::Keep.stand_world();
    let scene = world.overworld_scene();
    for r in &scene.march.riders {
        let tile = ((r.x / TILE as f32) as i32, (r.y / TILE as f32) as i32);
        assert!(
            (tile.0 - keep.0).abs() + (tile.1 - keep.1).abs() <= 4,
            "{:?} drew up behind the lead, at {tile:?}",
            r.house
        );
    }
    world.note_serving(&serving("deepseek", &[], true));
    ride(&mut world, 900);
    assert!(world.overworld_scene().march.riders.is_empty());
}

/// `ANGEL_OVERWORLD_SHOTS=<dir> cargo test write_castle_shots -- --ignored`
#[test]
#[ignore]
fn write_castle_shots() {
    let Some(dir) = std::env::var_os("ANGEL_OVERWORLD_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(name), out).unwrap();
    };
    let mut s = super::Scene::resting();
    let lead = houses::by_key("deepseek").unwrap();
    s.march = March {
        raised: vec![
            lead,
            houses::by_key("kimi").unwrap(),
            houses::by_key("grok").unwrap(),
        ],
        lead: Some(lead),
        riders: Vec::new(),
        watch: Vec::new(),
    };
    let (sx, sy) = stand(lead);
    s.knight = super::scene::Knight {
        x: (sx * TILE + 8) as f32,
        y: ((sy + 1) * TILE - 2) as f32,
        walking: false,
    };
    s.tick = 3;
    save(
        "realm.ppm",
        &super::render_view(&s, super::View::realm()),
    );
    for (name, row) in [("march_near", 8), ("march_far", 10)] {
        for half in 0..2 {
            let view = super::View {
                x: half * 4 * SCREEN_W * TILE - half * SCREEN_W * TILE / 2,
                y: row * SCREEN_H * TILE,
                w: 4 * SCREEN_W * TILE,
                h: SCREEN_H * TILE,
            };
            save(
                &format!("{name}_{half}.ppm"),
                &super::frame_at(&s, view),
            );
        }
    }
    for key in ["deepseek", "kimi", "longcat", "north", "inkling"] {
        let id = houses::by_key(key).unwrap();
        let (tx, ty, _, _) = footprint(id);
        let view = super::View::around(
            ((tx + 1) * TILE + 8) as f32,
            (ty * TILE + 16) as f32,
            128,
            96,
        );
        save(
            &format!("castle_{key}.ppm"),
            &super::frame_at(&s, view),
        );
    }
    // A live ride: Kimi's knight out of his castle to the Keep, then a change
    // of club with a formation seated.
    let mut world = crate::stage::world_viz::World::new(7);
    world.note_serving(&serving("kimi", &[], false));
    ride(&mut world, 4);
    world.note_serving(&serving("kimi", &[], true));
    world.turn_started();
    for (i, t) in [0usize, 40, 80, 120, 160, 240].into_iter().enumerate() {
        ride(&mut world, if i == 0 { 2 } else { 40 });
        let _ = t;
        let mut scene = world.overworld_scene();
        scene.camera = (scene.knight.x, scene.knight.y);
        save(
            &format!("ride_{i}.ppm"),
            &super::frame_sized(&scene, 256, 176),
        );
    }
    ride(&mut world, 400);
    world.note_serving(&serving("grok", &["kimi", "luna"], true));
    for i in 0..6 {
        ride(&mut world, 40);
        let mut scene = world.overworld_scene();
        scene.camera = (scene.knight.x, scene.knight.y);
        save(
            &format!("swap_{i}.ppm"),
            &super::frame_sized(&scene, 256, 176),
        );
    }
    ride(&mut world, 600);
    let mut scene = world.overworld_scene();
    scene.camera = (scene.knight.x, scene.knight.y);
    save(
        "formation_at_keep.ppm",
        &super::frame_sized(&scene, 256, 176),
    );
    let _ = HouseId(0);
}
