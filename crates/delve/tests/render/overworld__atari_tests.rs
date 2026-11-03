use super::*;
use crate::rules::{Input, RoomKind};
use std::collections::BTreeMap;
use super::super::stage_frame::{self, StageMaker};

/// A run of one knight in the first room of `kind` on floor one, `ticks`
/// into its fight.
fn fight(seed: u64, kind: RoomKind, ticks: u32) -> Run {
    let mut run = Run::new(seed, 1, Some("Friend"));
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == kind && !r.great())
        .expect("room kind on floor one");
    run.enter_for_test(index);
    let aim = BTreeMap::from([(1, Input { aim_y: -1, ..Default::default() })]);
    for _ in 0..ticks {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&aim);
    }
    run
}

fn inks_of(img: &Img) -> std::collections::BTreeSet<Rgb> {
    img.pixels().collect()
}

#[test]
fn the_board_is_off_unless_the_switch_is_on() {
    let run = fight(11, RoomKind::Fight, 0);
    assert!(!on(), "experimental: off by default, ANGEL_DELVE_ATARI=1 turns it on");
    assert!(!here(&run));
    set(true);
    assert!(here(&run));
    set(false);
}

#[test]
fn floor_one_is_drawn_in_the_boards_few_flat_inks() {
    set(true);
    let run = fight(11, RoomKind::Fight, 0);
    let field = scenery(&run);
    let inks = inks_of(&field);
    assert!(inks.iter().all(|c| INKS.contains(c)), "only the board's inks: {inks:?}");
    assert!(inks.len() <= INKS.len());
    // A wall that meets the floor is a plain bar; the floor is the field.
    let room = run.room();
    let (col, row) = (room.cols as i32 / 2 + 3, 0);
    assert_eq!(room.tile(col, row), Tile::Wall);
    assert_eq!(field.get(col * TILE + 5, row * TILE + 5), Some(BAR));
    let (c, r) = (3, 3);
    assert_eq!(room.tile(c, r), Tile::Floor);
    assert_eq!(field.get(c * TILE + 5, r * TILE + 5), Some(FIELD));
    // Pong's net down the middle.
    let x = (room.cols as i32 * TILE / 2 - 1) & !1;
    let dashes = (0..room.rows as i32 * TILE).filter(|&y| field.get(x, y) == Some(GREY)).count();
    assert!(dashes > 40, "a dashed centre line, {dashes} pixels");
    set(false);
    let plain = scenery(&run);
    assert_ne!(plain, field, "off, the floor keeps today's look");
}

#[test]
fn the_board_frame_keeps_to_the_realm_palette() {
    set(true);
    let run = fight(11, RoomKind::Fight, 90);
    let img = frame(&run, NATIVE_W, NATIVE_H);
    assert!(img.pixels().all(|c| c == BLACK || super::super::super::ink::palette_index(c).is_some()));
    set(false);
}

#[test]
fn a_foe_on_the_board_is_a_blocky_sprite_in_one_hue() {
    set(true);
    let run = fight(11, RoomKind::Fight, 30);
    let enemy = run.enemies.first().expect("a foe in the fight room");
    let im = foe_figure(&run, enemy, 0);
    let inks = inks_of(&im);
    assert!(inks.iter().all(|c| [FOE, PONG, FIELD].contains(c)), "{inks:?}");
    assert!(inks.contains(&FOE));
    // Two pixels to a block.
    for y in (0..im.h - 1).step_by(2) {
        for x in (0..im.w - 1).step_by(2) {
            let c = im.get(x, y);
            assert!([im.get(x + 1, y), im.get(x, y + 1), im.get(x + 1, y + 1)].iter().all(|&o| o == c));
        }
    }
    set(false);
    let plain = foe_figure(&run, enemy, 0);
    assert!(inks_of(&plain).len() > 3, "off, a foe keeps its own inks");
}

#[test]
fn a_window_lays_the_board_flat_and_unlit_and_floor_two_tips_again() {
    set(true);
    let mut run = fight(11, RoomKind::Fight, 30);
    let scene = std::sync::Arc::new(scene_for(&run, NATIVE_W, NATIVE_H, Some(1)));
    assert!(scene.board);
    assert!(scene.lights.is_empty() && scene.emitters.is_empty() && scene.pickups.is_empty());
    let frame = StageMaker::default().make(&scene, |_| None);
    assert_eq!(frame.tilt, 0.0);
    // Down the stairs the world gains its first rung back.
    run.descend_for_test();
    assert!(!here(&run));
    let scene = std::sync::Arc::new(scene_for(&run, NATIVE_W, NATIVE_H, Some(1)));
    assert!(!scene.board);
    assert_eq!(StageMaker::default().make(&scene, |_| None).tilt, stage_frame::TILT);
    set(false);
}

#[test]
fn a_page_takes_the_board_from_its_hosts_welcome() {
    use crate::wire::Params;
    set(false);
    let params = Params { atari: true, ..Params::default() };
    let _client = crate::client::DelveClient::new(Some(1), Params::decode(&params.encode()).unwrap());
    assert!(on());
    set(false);
}

/// The identity start as a page sees it: the host's welcome turns the board
/// on, and each of floor one's three rooms reaches the page as the board,
/// tick after tick. A page bakes the next torch step a stage a frame, from
/// the ordinary dungeon's art (`scenery::DungeonBake`), so the board must
/// never have a next torch step; floor two's torches step again.
#[test]
fn the_identity_floor_reaches_a_page_as_the_board() {
    use super::super::scenery::torch_step;
    use crate::wire::Params;
    // The host (ANGEL_DELVE_IDENTITY=1) says so in its welcome.
    set(true);
    let welcome = Params::with_mode(crate::world3d::room::Mode::Off).encode();
    set(false);
    let _page = crate::client::DelveClient::new(Some(1), Params::decode(&welcome).unwrap());
    assert!(on(), "the welcome's flag turns the page's board on");
    let mut run = Run::at_home(7, 1, None, crate::rules::home::Home::default(), Default::default());
    run.naked_start = true;
    run.identity = true;
    run.descend_for_test();
    assert_eq!(run.dungeon.rooms.len(), crate::rules::identity::ROOMS);
    let still = BTreeMap::from([(1, Input::default())]);
    for index in 0..crate::rules::identity::ROOMS {
        if index > 0 {
            run.calm_for_test();
            run.enemies.clear();
            run.enter(index, Some(2));
        }
        assert!(here(&run), "room {index} is the board");
        let step = torch_step(&run);
        for _ in 0..40 {
            run.step(&still);
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            assert_eq!(torch_step(&run), step, "room {index}: no torch step to bake over the board");
            let scene = scene_for_live(&run, NATIVE_W, NATIVE_H, Some(1));
            assert!(scene.board, "room {index}");
            let ground = scene.ground.expect("the board's field");
            let inks = inks_of(&ground);
            assert!(inks.iter().all(|c| INKS.contains(c)), "room {index}: only the board's inks, {inks:?}");
            // The wall along the foot is a plain bar, not the dungeon's bricks.
            let room = run.room();
            let (col, row) = (3, room.rows as i32 - 1);
            assert_eq!(room.tile(col, row), Tile::Wall);
            assert_eq!(ground.get(col * TILE + 5, row * TILE + 5), Some(BAR), "room {index}");
        }
        // What stands and moves in the room besides the knights and its foes
        // (a frog hopping about) is held to the board's inks too, on a page
        // and in a terminal alike. (A foe is the foe's hue,
        // `a_foe_on_the_board_is_a_blocky_sprite_in_one_hue`; a guardian's
        // nameplate and bar are its marks.)
        let mut bare = run.clone();
        bare.players.clear();
        bare.enemies.clear();
        let (w, h) = (bare.room().width(), bare.room().height());
        bare.frogs.wild.push(crate::rules::frogs::Wild { x: w / 3.0, y: h / 2.0, from: (w / 3.0, h / 2.0), to: (w / 3.0, h / 2.0), hop: 0, rest: 20, leap: None, face: 1 });
        let (_, standing) = room_list(&bare, None, false).split(Layer::Ground);
        let (standing, _) = standing.split(Layer::Over);
        let mut layer = Img::new(room_px(bare.room()).0, room_px(bare.room()).1);
        standing.paint(&mut layer);
        let inks = inks_of(&layer);
        assert!(!inks.is_empty(), "room {index}: the frog is drawn");
        assert!(inks.iter().all(|c| INKS.contains(c)), "room {index}: the room's creatures in the board's inks, {inks:?}");
    }
    // Floor two: the dungeon's own look, its torches stepping.
    run.descend_for_test();
    assert!(!here(&run));
    let step = torch_step(&run);
    for _ in 0..12 {
        run.step(&still);
    }
    assert_ne!(torch_step(&run), step);
    set(false);
}

/// The board's proof shots, as a terminal draws them (a pixel of the frame
/// is half a cell): `ANGEL_ATARI_SHOTS=<dir> cargo test -p angel-delve --lib
/// write_atari_shots`.
#[test]
fn write_atari_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ATARI_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let step = |run: &mut Run, ticks: u32, aim: i8| {
        let input = BTreeMap::from([(1, Input { aim_y: aim, ..Default::default() }), (2, Input { aim_x: 1, ..Default::default() }), (3, Input { aim_x: -1, ..Default::default() })]);
        for _ in 0..ticks {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            run.step(&input);
        }
    };
    let both = |name: &str, run: &Run| {
        set(true);
        save(&format!("{name}-a0.png"), &frame(run, NATIVE_W, NATIVE_H), 3);
        // A terminal of 200 x 58 cells: the frame boiled down to its grain.
        save(&format!("{name}-a0-term200.png"), &frame(run, 200, 116), 6);
        set(false);
        save(&format!("{name}-today.png"), &frame(run, NATIVE_W, NATIVE_H), 3);
    };
    // A party of three in a fight on floor one: the doors barred, foes in.
    for seed in [11u64, 23] {
        let mut run = Run::new(seed, 1, Some("Friend"));
        run.join_seat(3, "Goose Inspector");
        let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
        run.enter_for_test(index);
        // Loot on the floor, in the board's second hue.
        let (w, h) = (run.room().width(), run.room().height());
        for (k, card) in ["gold", "potion", "gold"].iter().enumerate() {
            run.dungeon.rooms[run.at].items.push(crate::rules::Item {
                card: (*card).into(),
                x: w / 2.0 + 5.0 + 3.0 * k as f32,
                y: h * 0.7,
                held_off: None, took: Vec::new(), dropped: 0, owner: None,
            });
        }
        step(&mut run, 70, -1);
        both(&format!("fight-{seed}"), &run);
    }
    // The knights stood together in the middle of the room, the view on them.
    let gather = |run: &mut Run| {
        let (w, h) = (run.room().width(), run.room().height());
        for (k, hero) in run.players.values_mut().enumerate() {
            (hero.x, hero.y) = (w / 2.0 - 2.0 + 1.6 * k as f32, h * 0.62);
            hero.camera = Some((hero.x, hero.y - 1.0));
        }
    };
    // The floor's guardian, in the stairs room it holds.
    let mut run = Run::new(11, 1, Some("Friend"));
    run.join_seat(3, "Goose Inspector");
    let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    run.enter_for_test(index);
    gather(&mut run);
    step(&mut run, 90, -1);
    // The view on the guardian, the knights below it.
    if let Some((bx, by)) = run.enemies.iter().find(|e| e.is_boss()).map(|e| (e.x, e.y)) {
        for (k, hero) in run.players.values_mut().enumerate() {
            (hero.x, hero.y) = (bx - 2.0 + 1.6 * k as f32, by + 4.5);
            hero.camera = Some((bx, by + 2.5));
        }
        step(&mut run, 2, -1);
    }
    both("guardian", &run);
    // The way down once it falls: the stairs open.
    run.clear_for_test();
    gather(&mut run);
    step(&mut run, 20, 0);
    both("stairs-open", &run);
    // Down the stairs: floor two keeps today's black look, the first rung.
    let mut run = Run::new(11, 1, Some("Friend"));
    run.descend_for_test();
    let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight).unwrap();
    run.enter_for_test(index);
    step(&mut run, 70, -1);
    set(true);
    save("floor-two-with-the-switch-on.png", &frame(&run, NATIVE_W, NATIVE_H), 3);
    set(false);
}

/// The identity start's streamlined first floor shows the whole board: grey
/// walls at the head, the foot and both sides, all on the view above the
/// HUD's band, and a gap in the head wall where the stairs stand.
#[test]
fn the_streamlined_room_shows_its_whole_frame_with_the_stairs_gap() {
    set(true);
    crate::rules::identity::set_simple(true);
    let mut run = Run::at_home(7, 1, None, crate::rules::home::Home::default(), Default::default());
    run.naked_start = true;
    run.identity = true;
    run.descend_for_test();
    assert!(run.simple_creation() && here(&run));
    let field = playfield(&run);
    let room = run.room();
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    assert!(rows * TILE <= NATIVE_H - 32 && cols * TILE <= NATIVE_W, "the room fits the view above the band");
    let px = |c: i32, r: i32| field.get(c * TILE + 5, r * TILE + 5);
    for c in [1, 3, cols - 2] {
        assert_eq!(px(c, 0), Some(BAR), "head wall at {c}");
        assert_eq!(px(c, rows - 1), Some(BAR), "foot wall at {c}");
    }
    for r in [1, rows / 2, rows - 2] {
        assert_eq!(px(0, r), Some(BAR), "west wall at {r}");
        assert_eq!(px(cols - 1, r), Some(BAR), "east wall at {r}");
    }
    let stairs: Vec<i32> = (0..cols).filter(|&c| room.tile(c, 1) == Tile::Stairs).collect();
    assert_eq!(stairs.len(), 2, "the stairs against the head wall");
    for &c in &stairs {
        assert_eq!(room.tile(c, 0), Tile::Wall, "the gap is drawn, not walked");
        assert_eq!(px(c, 0), Some(FIELD), "the gap over the stairs at {c}");
    }
    crate::rules::identity::set_simple(false);
    set(false);
}
