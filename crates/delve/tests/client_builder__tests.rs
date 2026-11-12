use super::*;
use crate::rules::builder_input::{self, BuilderAction};
use angel_net::cmd::Command;

fn village() -> Run {
    let mut run = Run::at_home(
        41,
        1,
        Some("Friend"),
        Default::default(),
        Default::default(),
    );
    run.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    run.step(&BTreeMap::from([(
        1,
        Input {
            cast: 4,
            ..Input::default()
        },
    )]));
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(
        1,
        Input {
            swing: true,
            ..Input::default()
        },
    )]));
    run.step(&BTreeMap::new());
    assert!(run.kingdom.industry[&1].open);
    assert_eq!(run.kingdom.industry[&1].plan, 1);
    run
}
fn frame_point(client: &DelveClient, lx: u32, ly: u32) -> (u32, u32) {
    let (w, h) = client.size();
    let vh = arena::view_rows(h as i32) as u32;
    let scale = (w as f32 / 768.).min(vh as f32 / 448.);
    let sw = (768. * scale) as u32;
    let sh = (448. * scale) as u32;
    (
        (w - sw) / 2 + lx * sw / 768,
        arena::BAND_H as u32 + (vh - sh) / 2 + ly * sh / 448,
    )
}

#[test]
fn pointer_click_is_one_authoritative_build_with_unchanged_replay_bytes() {
    let mut host = Host::new(village());
    let mut c = client(Some(0));
    host.send(&mut c);
    let (x, y) = frame_point(&c, 236, 239);
    assert!(c.pointer(x, y, 0));
    let cmd = c.command(&Hands::default());
    assert_eq!(
        cmd.pressed.builder,
        Some(BuilderAction::Plot {
            tile: (8, 8),
            build: true
        })
    );
    assert!(!cmd.input().fire && !cmd.input().interact);
    let mut bytes = Vec::new();
    cmd.encode(&mut bytes);
    let replay = DelveCmd::decode(&bytes).unwrap();
    assert_eq!(cmd, replay);
    host.step(&[(0, replay)]);
    assert_eq!(
        host.run
            .home
            .industry
            .buildings
            .values()
            .filter(|b| b.tile == (8, 8))
            .count(),
        1
    );
    let stock = host.run.home.industry.stock.clone();
    host.step(&[(0, replay.held_only())]);
    assert_eq!(host.run.home.industry.stock, stock);
    assert!(c.command(&Hands::default()).pressed.builder.is_none());
}

#[test]
fn pointer_inspection_tabs_and_closed_desk_never_leak_combat_input() {
    let mut host = Host::new(village());
    let mut c = client(Some(0));
    host.send(&mut c);
    let (x, y) = frame_point(&c, 236, 239);
    assert!(c.pointer(x, y, 2));
    assert_eq!(
        c.command(&Hands::default()).pressed.builder,
        Some(BuilderAction::Plot {
            tile: (8, 8),
            build: false
        })
    );
    assert!(c.pointer(x, y, 0));
    let (tx, ty) = frame_point(&c, 600, 72);
    assert!(c.pointer(tx, ty, 0));
    assert_eq!(
        c.command(&Hands::default()).pressed.builder,
        Some(BuilderAction::Page(7)),
        "newest click wins without replaying an older build"
    );
    assert!(!c.pointer(0, 0, 0), "the party band is not buildable");
    host.run.kingdom.industry.get_mut(&1).unwrap().open = false;
    host.send(&mut c);
    assert!(!c.pointer(x, y, 0));
    assert!(c.command(&Hands::default()).pressed.builder.is_none());
    assert!(builder_input::layout(&host.run, 1).is_none());
}

#[test]
fn displayed_c_space_and_e_shortcuts_use_actual_browser_control_mapping() {
    let controls: serde_json::Value = serde_json::from_str(CONTROLS).unwrap();
    let key = |name: &str| {
        angel_net::input::button::named(controls["keys"][name].as_str().unwrap()).unwrap()
    };
    let mut host = Host::new(village());
    let mut c = client(Some(0));
    host.send(&mut c);
    let old_zoom = host.run.kingdom.industry[&1].zoom;
    let zoom = c.command(&tap(key("Space")));
    assert!(zoom.input().dash);
    host.step(&[(0, zoom)]);
    host.step(&[]);
    host.send(&mut c);
    assert_ne!(host.run.kingdom.industry[&1].zoom, old_zoom);
    let build = c.command(&tap(key("KeyE")));
    assert!(build.input().interact);
    host.step(&[(0, build)]);
    host.step(&[]);
    host.send(&mut c);
    assert!(host.run.home.industry.at((8, 8)).is_some());
    let walk = c.command(&tap(key("KeyC")));
    assert!(
        walk.input().ride && !walk.input().team,
        "plain C walks; H owns the team chord"
    );
    host.step(&[(0, walk)]);
    assert!(host.run.kingdom.frontier);
    assert!(!host.run.kingdom.industry[&1].open);
}

/// A hillside run with the builder out in the walking world.
fn hillside() -> Run {
    let mut run = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(crate::kingdom::catalog(), 7);
    for (x, y) in [(158.5, 104.5)] {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (x * crate::rules::TILE_UNITS, y * crate::rules::TILE_UNITS);
        h.camera = Some((h.x, h.y - 1.0));
        h.invulnerable = 100_000;
        h.stone = false;
    }
    for _ in 0..3 {
        run.step(&BTreeMap::new());
    }
    run.step(&BTreeMap::from([(1, Input { cast: 4, ..Input::default() })]));
    run.step(&BTreeMap::new());
    assert!(crate::rules::hillside::building(&run, 1));
    run
}

#[test]
fn a_mouse_held_and_dragged_over_the_walking_world_is_a_press_the_tiles_crossed_and_a_release() {
    let mut host = Host::new(hillside());
    let mut c = client(Some(0));
    host.send(&mut c);
    let origin = arena::camera(c.newest().unwrap(), Some(1));
    let (w, h) = c.size();
    let point = move |lx: u32, ly: u32| {
        let vh = arena::view_rows(h as i32) as u32;
        let scale = (w as f32 / 768.).min(vh as f32 / 448.);
        let (sw, sh) = ((768. * scale) as u32, (448. * scale) as u32);
        ((w - sw) / 2 + lx * sw / 768, arena::BAND_H as u32 + (vh - sh) / 2 + ly * sh / 448)
    };
    let at = move |tile: (i32, i32)| point(((tile.0 * 16 - origin.0 + 8) * 2) as u32, ((tile.1 * 16 - origin.1 + 8) * 2) as u32);
    let (x, y) = at((158, 104));
    assert!(crate::rules::hillside::building(c.newest().unwrap(), 1), "the builder is out in the walking world the client sees");
    assert!(!crate::world3d::enabled(), "the flat view");
    assert!(c.knight.is_some(), "a seat");
    let lp = builder_input::logical_point(x, y, w, arena::view_rows(h as i32) as u32, arena::BAND_H as u32);
    assert!(lp.is_some(), "a point of the picture: {x},{y} of {w}x{h}");
    let (lx, ly) = lp.unwrap();
    assert!(arena::hill_tile_at(c.newest().unwrap(), 1, lx as i32, ly as i32).is_some(), "a tile at {lx},{ly}");
    assert!(c.pointer(x, y, 0));
    let first = c.command(&Hands::default()).pressed.builder;
    assert_eq!(first, Some(BuilderAction::Plot { tile: (158, 104), build: true }));
    // Moved a tile on: the next command says where.
    let (x, y) = at((159, 104));
    assert!(c.pointer_move(x, y));
    // Moved within the same tile: nothing new.
    assert!(!c.pointer_move(x + 1, y));
    let (x, y) = at((161, 104));
    assert!(c.pointer_move(x, y));
    // A run of moves is one, the last.
    assert_eq!(c.command(&Hands::default()).pressed.builder, Some(BuilderAction::Hold((161, 104))));
    assert!(c.pointer_up(x, y, 0));
    assert_eq!(c.command(&Hands::default()).pressed.builder, Some(BuilderAction::Let((161, 104))));
    assert!(c.command(&Hands::default()).pressed.builder.is_none());
    // A press and its release in one tick are two commands, in order.
    let (x, y) = at((160, 104));
    assert!(c.pointer(x, y, 0));
    assert!(c.pointer_up(x, y, 0));
    assert!(matches!(c.command(&Hands::default()).pressed.builder, Some(BuilderAction::Plot { .. })));
    assert!(matches!(c.command(&Hands::default()).pressed.builder, Some(BuilderAction::Let(_))));
    // A move with no button down is nothing; a right click is a press that is not dragged.
    assert!(!c.pointer_move(x, y));
    assert!(c.pointer(x, y, 2));
    assert!(!c.pointer_move(point(100, 100).0, point(100, 100).1));
    // The strip at the view's foot is the hotbar's: a slot is a click, the rest of it lays nothing.
    let strip = (arena::NATIVE_H - arena::hill_strip_height() - 2) * 2;
    let (x, y) = point(30, (strip + 14) as u32);
    c.command(&Hands::default());
    assert!(c.pointer(x, y, 0));
    assert_eq!(c.command(&Hands::default()).pressed.builder, Some(BuilderAction::Slot(0)));
    let (x, y) = point(500, (strip + 60) as u32);
    assert!(c.pointer(x, y, 0), "the strip takes the click");
    assert!(c.command(&Hands::default()).pressed.builder.is_none(), "and lays nothing");
}
