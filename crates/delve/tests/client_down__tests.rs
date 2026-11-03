use super::*;
use crate::rules::{Pack, Phase};

fn fallen() -> Run {
    let mut run = fight();
    for (id, name) in [(2, "Lynette"), (3, "Gareth"), (4, "Galahad"), (5, "Gawain"), (6, "Kay")] {
        run.join_seat(id, name);
    }
    run.floor_plates.clear();
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp = 0;
    hero.winds = 0;
    hero.talisman = false;
    run
}

fn watch(c: &DelveClient) -> u32 { c.down.as_ref().unwrap().watching }

fn draw(c: &mut DelveClient, hands: Hands) -> Frame {
    let mut frame = Frame::default();
    c.draw(&view(c, &[], 0, hands, 0.0), &mut frame);
    frame
}

#[test]
fn fallen_camera_cycles_living_present_allies_both_ways_and_can_watch_its_body() {
    let mut run = fallen();
    run.players.get_mut(&3).unwrap().stone = true;
    run.players.get_mut(&4).unwrap().absent = true;
    let mut host = Host::new(run);
    let mut c = client(Some(0));
    host.send(&mut c);
    assert_eq!(watch(&c), 2);
    for expected in [5, 6, 2] {
        assert_eq!(c.command(&tap(button::AIM_RIGHT)), DelveCmd::neutral());
        assert_eq!(watch(&c), expected);
    }
    c.command(&tap(button::AIM_LEFT));
    assert_eq!(watch(&c), 6);
    c.command(&held(button::AIM_RIGHT));
    assert_eq!(watch(&c), 6, "held keys never repeat a camera change");
    c.command(&tap(button::JUMP));
    assert_eq!(watch(&c), 1);
    host.send(&mut c);
    assert_eq!(watch(&c), 1, "watching the body is a lasting choice");
    c.command(&tap(button::AIM_LEFT));
    assert_eq!(watch(&c), 6);
}

#[test]
fn a_fallen_or_departed_watch_target_is_replaced_and_no_allies_means_the_body() {
    let mut host = Host::new(fallen());
    let mut c = client(Some(0));
    host.send(&mut c);
    host.run.players.get_mut(&2).unwrap().hp = 0;
    host.send(&mut c);
    assert_eq!(watch(&c), 3);
    host.run.players.remove(&3);
    host.send(&mut c);
    assert_eq!(watch(&c), 4);
    for h in host.run.players.values_mut() { h.hp = 0; }
    host.send(&mut c);
    assert_eq!(watch(&c), 1);
    c.command(&tap(button::AIM_LEFT));
    assert_eq!(watch(&c), 1);
}

#[test]
fn down_keys_are_local_and_other_seats_keep_playing_and_predicting() {
    let run = fallen();
    let mut host = Host::new(run.clone());
    let mut c = client(Some(0));
    host.send(&mut c);
    let before = serde_json::to_value(c.newest().unwrap()).unwrap();
    for bits in [button::AIM_RIGHT, button::AIM_LEFT, button::JUMP, button::MENU,
        button::AIM_DOWN, button::OK, key::MAP_KEY, button::MENU, key::SPELLS[3]] {
        let cmd = c.command(&Hands { held: bits, pressed: bits, ..Default::default() });
        assert_eq!(cmd, DelveCmd::neutral());
        let mut bytes = Vec::new();
        cmd.encode(&mut bytes);
        let mut neutral = Vec::new();
        DelveCmd::neutral().encode(&mut neutral);
        assert_eq!(bytes, neutral);
    }
    draw(&mut c, held(button::AIM_RIGHT | button::FIRE));
    assert_eq!(serde_json::to_value(c.newest().unwrap()).unwrap(), before);
    let mut living = client(Some(1));
    Host::new(run.clone()).send(&mut living);
    assert!(living.down.is_none());
    let cmd = living.command(&held(button::RIGHT));
    assert_eq!(cmd.held.move_x, 1);
    let pending = [(1, cmd)];
    let mut frame = Frame::default();
    living.draw(&view(&living, &pending, 0, held(button::RIGHT), 0.5), &mut frame);
    assert!(living.drawn_at().is_some());
    let mut watcher = client(None);
    Host::new(run).send(&mut watcher);
    assert!(watcher.down.is_none());
}

#[test]
fn the_down_menu_uses_arrows_enter_and_numbers_and_reuses_the_floor_map() {
    let mut host = Host::new(fallen());
    let mut c = client(Some(0)).exact();
    host.send(&mut c);
    let banner = draw(&mut c, Hands::default()).px;
    c.command(&tap(button::MENU));
    assert_eq!(c.down.as_ref().unwrap().menu, Some(0));
    assert_ne!(draw(&mut c, Hands::default()).px, banner);
    c.command(&tap(button::AIM_DOWN));
    c.command(&tap(button::OK));
    assert_eq!(watch(&c), 1);
    assert!(c.down.as_ref().unwrap().menu.is_none());
    c.command(&tap(button::MENU));
    c.command(&tap(key::SPELLS[2]));
    let mapped = draw(&mut c, Hands::default()).px;
    assert!(c.map_open());
    assert_ne!(mapped, banner);
    c.command(&tap(button::MENU));
    draw(&mut c, Hands::default());
    assert!(!c.map_open());
    c.command(&tap(button::MENU));
    c.command(&tap(key::SPELLS[3]));
    assert!(c.down.as_ref().unwrap().hidden);
    c.command(&tap(button::MENU));
    c.command(&tap(key::SPELLS[3]));
    assert!(!c.down.as_ref().unwrap().hidden);
    c.command(&tap(button::MENU));
    c.command(&tap(button::MENU));
    assert!(c.down.as_ref().unwrap().menu.is_none());
}

#[test]
fn the_stairs_clear_every_down_choice_as_soon_as_the_revive_snapshot_arrives() {
    let mut host = Host::new(fallen());
    let mut c = client(Some(0));
    host.send(&mut c);
    c.command(&tap(button::MENU));
    c.command(&tap(key::SPELLS[3]));
    c.command(&tap(button::MENU));
    c.command(&tap(key::SPELLS[2]));
    draw(&mut c, Hands::default());
    assert!(c.map_open());
    host.run.descend_for_test();
    assert!(host.run.players[&1].hp > 0);
    host.send(&mut c);
    assert!(c.down.is_none() && !c.map_open() && !c.menu_open());
    host.run.floor_plates.clear();
    host.send(&mut c);
    assert_eq!(c.command(&held(button::RIGHT)).held.move_x, 1);
    // A fresh fall gets the banner back and starts by watching an ally.
    host.run.players.get_mut(&1).unwrap().hp = 0;
    host.send(&mut c);
    let down = c.down.as_ref().unwrap();
    assert!(!down.hidden && !down.map && down.menu.is_none());
    assert_eq!(down.watching, 2);
    host.run.phase = Phase::Wiped;
    host.send(&mut c);
    assert!(c.down.is_none());
    assert!(c.command(&tap(button::OK)).pressed.again);
}

#[test]
fn the_camera_follows_the_named_ally_and_the_body_in_both_render_paths() {
    let mut run = fallen();
    run.begin_in(Pack::Crypt);
    let at = run.dungeon.rooms.iter().enumerate().filter(|(_, r)| r.kind == RoomKind::Fight)
        .max_by_key(|(_, r)| r.cols * r.rows).unwrap().0;
    run.enter_for_test(at);
    run.floor_plates.clear();
    run.shake = 0;
    let own = run.players.get_mut(&1).unwrap();
    (own.hp, own.x, own.y) = (0, 4.0, 4.0);
    let friend = run.players.get_mut(&2).unwrap();
    (friend.x, friend.y) = (run.dungeon.rooms[at].width() - 4.0, run.dungeon.rooms[at].height() - 4.0);
    let mut host = Host::new(run);
    let mut c = client(Some(0)).exact();
    host.send(&mut c);
    let in_seat = |(x, y): (f32, f32)| (x, y - arena::BAND_H as f32);
    let expected = in_seat(arena::camera_exact(c.newest().unwrap(), Some(2)));
    let body = in_seat(arena::with_watch(Some(1), || arena::camera_exact(c.newest().unwrap(), Some(1))));
    assert_ne!(expected, body, "a hall big enough to scroll");
    arena::draw_parts_outside(true);
    draw(&mut c, Hands::default());
    assert_eq!(c.scene.as_ref().unwrap().camera, expected);
    c.command(&tap(button::JUMP));
    draw(&mut c, Hands::default());
    assert_eq!(c.scene.as_ref().unwrap().camera, body);
    let parts_screen = c.scene.as_ref().unwrap().screen.clone();
    arena::draw_parts_outside(false);
    let frame = draw(&mut c, Hands::default());
    // The browser's stage screen and the flat client share the entire banner.
    for y in 10..70 {
        for x in 12..372 {
            let y = y + arena::BAND_H;
            if let Some(color) = parts_screen.get(x, y) {
                assert_eq!(frame.px[(y * frame.w as i32 + x) as usize] & 0x00ff_ffff,
                    u32::from(color[0]) << 16 | u32::from(color[1]) << 8 | u32::from(color[2]));
            }
        }
    }
    c.command(&tap(button::AIM_RIGHT));
    arena::draw_parts_outside(true);
    draw(&mut c, Hands::default());
    assert_eq!(c.scene.as_ref().unwrap().camera, expected);
    host.run.players.get_mut(&1).unwrap().hp = 1;
    host.send(&mut c);
    arena::draw_parts_outside(true);
    draw(&mut c, Hands::default());
    assert_eq!(c.scene.as_ref().unwrap().camera, body, "revived camera follows its own knight");
    arena::draw_parts_outside(false);
}

#[test]
#[ignore = "writes the down-state review frames"]
fn down_state_shots() {
    let out = std::path::PathBuf::from(std::env::var("DELVE_DOWN_SHOTS")
        .unwrap_or_else(|_| "/tmp/delve-down".to_string()));
    std::fs::create_dir_all(&out).unwrap();
    let save = |name: &str, c: &mut DelveClient| {
        let frame = draw(c, Hands::default());
        image::RgbaImage::from_fn(frame.w, frame.h, |x, y| {
            let word = frame.px[(y * frame.w + x) as usize];
            image::Rgba([(word >> 16) as u8, (word >> 8) as u8, word as u8, 255])
        }).save(out.join(format!("{name}.png"))).unwrap();
    };
    let mut host = Host::new(fallen());
    let mut c = client(Some(0)).exact();
    host.send(&mut c);
    save("down", &mut c);
    c.command(&tap(button::MENU));
    save("menu", &mut c);
    c.command(&tap(key::SPELLS[1]));
    save("body", &mut c);
    c.command(&tap(button::MENU));
    c.command(&tap(key::SPELLS[2]));
    save("map", &mut c);
    c.command(&tap(button::MENU));
    c.command(&tap(button::MENU));
    c.command(&tap(key::SPELLS[3]));
    save("hidden", &mut c);
    host.run.players.get_mut(&1).unwrap().hp = 1;
    host.send(&mut c);
    save("revived", &mut c);
    let mut living = client(Some(1)).exact();
    Host::new(fallen()).send(&mut living);
    save("living-ally", &mut living);
}

#[test]
fn calm_screen_toggle_lives_beside_the_other_local_viewer_options() {
    let mut host = Host::new(fallen());
    let mut c = client(Some(0)).exact();
    host.send(&mut c);
    let before = serde_json::to_value(&host.run).unwrap();
    c.command(&tap(button::MENU));
    c.command(&tap(button::AIM_UP));
    assert_eq!(c.down.as_ref().unwrap().menu, Some(4));
    assert_eq!(c.command(&tap(button::OK)), DelveCmd::neutral());
    assert!(c.viewer.calm_screen);
    let mut hands = held(key::MAP_KEY | button::USE);
    hands.pressed = button::USE;
    assert_eq!(c.command(&hands), DelveCmd::neutral());
    assert!(!c.viewer.calm_screen);
    assert_eq!(serde_json::to_value(&host.run).unwrap(), before);
}
