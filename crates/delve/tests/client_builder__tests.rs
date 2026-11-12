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
