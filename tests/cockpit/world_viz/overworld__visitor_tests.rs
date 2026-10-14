use super::super::{castles, frame_at, pace, render_view, view_sized, wishes};
use super::*;
use crate::stage::houses;

/// Tick the world until the visitor stops, at most `limit` ticks.
fn settle(world: &mut World, limit: usize) {
    for _ in 0..limit {
        world.tick();
        if !world.visitor_busy() {
            break;
        }
    }
}

fn pose(world: &World) -> VisitorPose {
    world.overworld_scene().visitor.expect("the visitor is out")
}

/// A world with the visitor walked (by road, as a click sends him) to the
/// tile before `door`.
fn at_door(door: (i32, i32)) -> World {
    let mut world = World::new(7);
    world.start_walk();
    let (x, y) = feet(door);
    assert!(world.walk_visitor_to(x, y), "a road leads to {door:?}");
    settle(&mut world, 4000);
    let p = pose(&world);
    assert_eq!(tile_of(p.x, p.y), door);
    world
}

#[test]
fn every_door_stands_on_ground_the_roads_reach_and_the_table_names_each_once() {
    let mut seen = std::collections::HashSet::new();
    let from = Place::Keep.stand_world();
    for &(tile, door) in doors() {
        assert!(
            step_cost(tile.0, tile.1).is_some(),
            "{door:?} at {tile:?} is walkable"
        );
        assert!(
            tile == from
                || route(from, tile).len() > 1
                || tile.0.abs_diff(from.0) + tile.1.abs_diff(from.1) == 1,
            "a road leads to {door:?} at {tile:?}"
        );
        assert!(seen.insert(door), "{door:?} listed once");
        let (x, y) = feet(tile);
        assert_eq!(door_near(x, y), Some(door));
    }
    assert_eq!(
        doors().len(),
        Place::ALL.len() + houses::ids().count() + 2,
        "every place, every castle, the Delve and the College"
    );
}

#[test]
fn the_delve_gate_opens_from_the_near_bank_of_its_stream() {
    // The gate's own stand is in a pocket no road reaches.
    let stand = wishes::delve_stand();
    let from = Place::Keep.stand_world();
    assert_eq!(
        route(from, stand),
        vec![stand],
        "no road: the knight walks straight in"
    );
    let &(bank, _) = doors().iter().find(|(_, d)| *d == Door::Delve).unwrap();
    assert_ne!(bank, stand);
    assert!(route(from, bank).last() == Some(&bank));
    let gate = wishes::delve_gate();
    assert!(
        (gate.ty..gate.ty + gate.th).contains(&bank.1) && (gate.tx - bank.0) <= 6,
        "level with the gate, across the stream: {bank:?} for {gate:?}"
    );
    let (x, y) = ((gate.tx * TILE + 20) as f32, (gate.ty * TILE + 20) as f32);
    assert_eq!(
        click_goal(x, y),
        Some(bank),
        "a click on the gate walks to the bank"
    );
}

#[test]
fn the_doors_name_what_they_open() {
    let lists = Door::Place(Place::Lists);
    assert_eq!(lists.sign(), "THE LISTS");
    assert_eq!(lists.name(), "The Lists");
    assert_eq!(lists.opens(), Some(Opens::Lists));
    assert_eq!(Door::Place(Place::Tournament).opens(), Some(Opens::Stables));
    assert_eq!(Door::Delve.opens(), Some(Opens::Delve));
    assert_eq!(
        Door::Place(Place::Keep).opens(),
        Some(Opens::Room(Building::Keep))
    );
    assert_eq!(Door::Place(Place::Mines).opens(), None, "a name, no door");
    let id = houses::ids().next().unwrap();
    assert_eq!(Door::Castle(id).opens(), Some(Opens::House(id)));
    assert_eq!(Door::Castle(id).name(), houses::get(id).castle);
}

#[test]
fn the_visitor_steps_out_beside_the_knight_and_the_camera_follows_him() {
    let mut world = World::new(7);
    assert!(world.overworld_scene().visitor.is_none());
    let before = world.overworld_scene().key();
    world.start_walk();
    assert!(world.walking());
    let scene = world.overworld_scene();
    let (v, k) = (scene.visitor.unwrap(), scene.knight);
    assert_ne!((v.x, v.y), (k.x, k.y), "beside the knight, not on him");
    assert!((v.x - k.x).abs() + (v.y - k.y).abs() <= 2.0 * TILE as f32);
    assert_ne!(scene.key(), before, "he is drawn");
    world.steer_visitor((1, 0));
    for _ in 0..120 {
        world.tick();
    }
    let walked = world.overworld_scene();
    let w = walked.visitor.unwrap();
    assert!(w.x > v.x + 40.0, "east along the road: {} → {}", v.x, w.x);
    assert_ne!(walked.key(), scene.key(), "the map repaints as he walks");
    assert!(
        (walked.camera.0 - w.x).abs() <= 41.0,
        "the camera keeps him in view"
    );
    world.stop_walk();
    assert!(!world.walking());
    let home = world.overworld_scene();
    assert!(home.visitor.is_none());
    assert_eq!(home.camera, world.overworld.camera(), "back on the knight");
    world.start_walk();
    let again = pose(&world);
    assert_eq!(
        (again.x, again.y),
        (w.x, w.y),
        "the next walk starts where this one stopped"
    );
}

#[test]
fn a_structure_stops_him_and_his_feet_stay_on_open_ground() {
    let door = Place::Keep.stand_world();
    let mut world = at_door(door);
    let start = pose(&world);
    world.steer_visitor((0, -1));
    for _ in 0..80 {
        world.tick();
        let v = world.overworld_visitor.as_ref().unwrap();
        assert!(clear(v.x, v.y), "his feet never stand on closed ground");
    }
    let stopped = pose(&world);
    assert!(stopped.y <= start.y && stopped.y > start.y - TILE as f32);
    let (_, ahead) = tile_of(stopped.x, stopped.y - FOOT_H - PACE);
    assert!(
        step_cost(door.0, ahead).is_none(),
        "closed ground stops him"
    );
    let (kx, ky, kw, kh) = Place::Keep.footprint_world();
    assert!(
        (kx..kx + kw).contains(&door.0) && (ky..ky + kh).contains(&ahead),
        "the Keep's wall is what stops him"
    );
    // Along the road the way is open.
    world.steer_visitor((1, 0));
    for _ in 0..40 {
        world.tick();
    }
    assert!(pose(&world).x > stopped.x + 30.0);
}

#[test]
fn a_click_on_a_building_walks_him_to_its_door() {
    let (kx, ky, _, _) = Place::Keep.footprint_world();
    let (x, y) = ((kx * TILE + 20) as f32, (ky * TILE + 10) as f32);
    assert_eq!(click_goal(x, y), Some(Place::Keep.stand_world()));
    let house = houses::ids().next().unwrap();
    let (cx, cy, _, _) = castles::footprint(house);
    assert_eq!(
        click_goal((cx * TILE + 8) as f32, (cy * TILE + 8) as f32),
        Some(castles::stand(house))
    );
    let (sx, sy) = Place::Lists.stand_world();
    assert_eq!(
        click_goal((sx * TILE + 3) as f32, (sy * TILE + 3) as f32),
        Some((sx, sy)),
        "open ground is walked to as it is"
    );
}

#[test]
fn at_the_lists_and_the_delve_gate_the_sign_says_where_the_door_leads() {
    let door_of = |want: Door| doors().iter().find(|(_, d)| *d == want).unwrap().0;
    for want in [Door::Place(Place::Lists), Door::Delve] {
        let door = door_of(want);
        let world = at_door(door);
        assert_eq!(world.visitor_door(), Some(want));
        let scene = world.overworld_scene();
        let view = view_sized(&scene, 320, 180);
        let with = render_view(&scene, view);
        let mut bare = scene.clone();
        bare.visitor = None;
        assert_ne!(with.rgb_bytes(), render_view(&bare, view).rgb_bytes());
        // The sign's gold frame runs over his head.
        let gold = super::super::ink::ink('4').unwrap();
        let p = scene.visitor.unwrap();
        let (vx, top) = (p.x as i32 - view.x, p.y as i32 - view.y + 2 - 16 - 3 - 20);
        let framed = (vx - 70..vx + 70)
            .filter(|&x| with.get(x, top) == Some(gold))
            .count() as i32;
        let sign = kit::door_sign(&want.sign(), true);
        assert_eq!(framed, sign.w, "{want:?}: the sign over his head");
    }
}

#[test]
fn the_kitty_frame_and_the_half_block_frame_both_follow_him() {
    let mut world = World::new(7);
    world.start_walk();
    let mut first = world.overworld_scene();
    pace(&mut first, false);
    world.steer_visitor((1, 0));
    for _ in 0..8 {
        world.tick();
    }
    let mut moved = world.overworld_scene();
    pace(&mut moved, false);
    assert_ne!(first.key(), moved.key(), "the cache key moves with him");
    let native = |scene: &Scene| super::super::LazyFrame::new(scene.clone(), 160, 90, 2);
    assert_ne!(native(&first).as_ref(), native(&moved).as_ref());
    let half = |scene: &Scene| super::super::frame_cached(scene, 120, 96);
    assert_ne!(half(&first).rgb_bytes(), half(&moved).rgb_bytes());
}

/// Offline renders for review:
/// `ANGEL_OVERWORLD_SHOTS=<dir> cargo test write_visitor_shots -- --ignored`.
#[test]
#[ignore]
fn write_visitor_shots() {
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
    // A Kitty-sized view (a 64x30-cell pane at scale 2) and the half-block
    // view of a 64x24-cell pane.
    let shot = |world: &World, w: i32, h: i32| {
        let mut scene = world.overworld_scene();
        pace(&mut scene, false);
        frame_at(&scene, view_sized(&scene, w, h))
    };
    let mut world = World::new(7);
    world.start_walk();
    save("walk_start.ppm", &shot(&world, 320, 180));
    world.steer_visitor((1, 0));
    for frame in 0..4 {
        for _ in 0..12 {
            world.tick();
        }
        save(&format!("walk_east_{frame}.ppm"), &shot(&world, 320, 180));
    }
    // Out on the plaza, no door at hand: the mark over his head.
    let mut world = World::new(7);
    world.start_walk();
    world.steer_visitor((0, 1));
    for _ in 0..40 {
        world.tick();
    }
    assert_eq!(world.visitor_door(), None);
    save("walk_mark.ppm", &shot(&world, 320, 180));
    save("walk_mark_halfblock.ppm", &shot(&world, 128, 96));
    for (name, want) in [
        ("door_lists", Door::Place(Place::Lists)),
        ("door_delve", Door::Delve),
        ("door_keep", Door::Place(Place::Keep)),
        ("door_mines", Door::Place(Place::Mines)),
        (
            "door_castle",
            Door::Castle(houses::by_key("deepseek").unwrap()),
        ),
    ] {
        let &(door, _) = doors().iter().find(|(_, d)| *d == want).unwrap();
        let world = at_door(door);
        save(&format!("{name}.ppm"), &shot(&world, 320, 180));
        save(&format!("{name}_halfblock.ppm"), &shot(&world, 128, 96));
    }
}

#[test]
fn college_stands_beside_the_delve_with_a_distinct_reachable_map_door() {
    let college = super::super::college::lot();
    let gate = wishes::delve_gate();
    let dx = (college.tx - gate.tx - gate.tw)
        .max(gate.tx - college.tx - college.tw)
        .max(0);
    let dy = (college.ty - gate.ty - gate.th)
        .max(gate.ty - college.ty - college.th)
        .max(0);
    assert!(
        dx <= 4 && dy <= 3,
        "the school is by the gate: {college:?}, {gate:?}"
    );
    let &(tile, _) = doors()
        .iter()
        .find(|(_, door)| *door == Door::College)
        .unwrap();
    assert_eq!(
        tile,
        super::super::college::stand(),
        "the path reaches the actual front door"
    );
    assert!(!super::super::college::approach().is_empty());
    let world = at_door(tile);
    let p = pose(&world);
    assert_eq!(door_near(p.x, p.y), Some(Door::College));
    assert_eq!(Door::College.opens(), Some(Opens::College));
    assert_ne!(
        tile,
        doors()
            .iter()
            .find(|(_, door)| *door == Door::Delve)
            .unwrap()
            .0
    );
}
