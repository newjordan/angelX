use super::*;
use crate::stage::houses::{self, HouseId};

#[test]
fn every_castle_stands_on_march_meadow_with_a_road_at_its_gate() {
    let realm = super::super::map::Realm::get();
    let mut taken = std::collections::BTreeSet::new();
    for id in houses::ids() {
        let (tx, ty, tw, th) = footprint(id);
        assert!(
            ty / SCREEN_H >= super::super::map::HEARTLAND_Y * 2,
            "{:?} is not in the March",
            id
        );
        for y in ty..ty + th {
            for x in tx..tx + tw {
                assert_eq!(realm.at(x, y), b'.', "{id:?} stands on {x},{y}");
                assert!(taken.insert((x, y)), "castles overlap at {x},{y}");
            }
        }
        let (sx, sy) = stand(id);
        assert_eq!(realm.at(sx, sy), b'=', "{id:?} gate has no road");
    }
}

#[test]
fn every_castle_is_reachable_from_the_keep() {
    let keep = super::super::map::Place::Keep.stand_world();
    for id in houses::ids() {
        let path = super::super::live::route(keep, stand(id));
        assert_eq!(path.last(), Some(&stand(id)), "{id:?} unreachable");
        for &(x, y) in &path[..path.len() - 1] {
            assert!(
                super::super::live::step_cost(x, y).is_some(),
                "{id:?} route crosses closed ground at {x},{y}"
            );
        }
    }
}

#[test]
fn an_idle_castle_flies_nothing_and_a_serving_one_flies_its_banner() {
    for id in houses::ids() {
        let h = houses::get(id);
        let idle = sprite(h, false, 0);
        assert!(
            idle.pixels().all(|c| !super::super::ink::is_signal(c)),
            "{} idle castle shows live state",
            h.key
        );
        let up = sprite(h, true, 0);
        let field = super::super::ink::ink(h.field).unwrap();
        let charge = super::super::ink::ink(h.charge).unwrap();
        assert!(up.pixels().any(|c| c == field), "{} banner field", h.key);
        assert!(up.pixels().any(|c| c == charge), "{} banner charge", h.key);
    }
}

#[test]
fn the_knight_wears_his_houses_colours() {
    let kimi = houses::by_key("kimi").unwrap();
    let h = houses::get(kimi);
    let dressed = dress(&super::super::kit::knight(), Some(kimi));
    let red = super::super::ink::ink('7').unwrap();
    assert!(dressed.pixels().all(|c| c != red || h.field == '7'));
    assert!(
        dressed
            .pixels()
            .any(|c| c == super::super::ink::ink(h.field).unwrap())
    );
    assert_eq!(
        dress(&super::super::kit::knight(), None).rgb_bytes(),
        super::super::kit::knight().rgb_bytes(),
        "the Keep's own knight keeps the red"
    );
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
        super::super::map::Place::Keep.stand_world()
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
        super::super::map::Place::Keep.stand_world()
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
    let keep = super::super::map::Place::Keep.stand_world();
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
    let mut s = super::super::Scene::resting();
    let lead = houses::by_key("deepseek").unwrap();
    s.march = March {
        raised: vec![
            lead,
            houses::by_key("kimi").unwrap(),
            houses::by_key("grok").unwrap(),
        ],
        lead: Some(lead),
        riders: Vec::new(),
    };
    let (sx, sy) = stand(lead);
    s.knight = super::super::Knight {
        x: (sx * TILE + 8) as f32,
        y: ((sy + 1) * TILE - 2) as f32,
        walking: false,
    };
    s.tick = 3;
    save(
        "realm.ppm",
        &super::super::render_view(&s, super::super::View::realm()),
    );
    for (name, row) in [("march_near", 8), ("march_far", 10)] {
        for half in 0..2 {
            let view = super::super::View {
                x: half * 4 * SCREEN_W * TILE - half * SCREEN_W * TILE / 2,
                y: row * SCREEN_H * TILE,
                w: 4 * SCREEN_W * TILE,
                h: SCREEN_H * TILE,
            };
            save(
                &format!("{name}_{half}.ppm"),
                &super::super::frame_at(&s, view),
            );
        }
    }
    for key in ["deepseek", "kimi", "longcat", "north", "inkling"] {
        let id = houses::by_key(key).unwrap();
        let (tx, ty, _, _) = footprint(id);
        let view = super::super::View::around(
            ((tx + 1) * TILE + 8) as f32,
            (ty * TILE + 16) as f32,
            128,
            96,
        );
        save(
            &format!("castle_{key}.ppm"),
            &super::super::frame_at(&s, view),
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
            &super::super::frame_sized(&scene, 256, 176),
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
            &super::super::frame_sized(&scene, 256, 176),
        );
    }
    ride(&mut world, 600);
    let mut scene = world.overworld_scene();
    scene.camera = (scene.knight.x, scene.knight.y);
    save(
        "formation_at_keep.ppm",
        &super::super::frame_sized(&scene, 256, 176),
    );
    let _ = HouseId(0);
}
