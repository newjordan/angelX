//! The mini-game audit: scripted, looping baselines of the games as they are.
//!
//! Each scene builds the real `Run`, seats 2-4 knights, drives them with the
//! game's own bot (or a plain script where it has none) and writes frames of
//! chosen seats' own screens, every second tick (15 fps), as
//! `<dir>/<game>/f0000.png`, 384 by 224 each, seats side by side. The GIFs are
//! made from them by `scripts/audit_gifs.py` (1x and 2x, nearest neighbour,
//! flattened on black). It changes no game code and writes only under the
//! directory it is given.
//!
//!   ANGEL_AUDIT_SHOTS=<dir> [AUDIT_ONLY=latch,cinder] \
//!     cargo test -p angel-delve --lib write_audit_shots -- --ignored --nocapture
//!
//! A game whose rules run in a Bend module with no native oracle in the test
//! build (Moonfall, Lanternward) is drawn from its recorded or posed states,
//! and its scene says so.
use super::*;
use crate::render::overworld::{Img, arena::frame_for};
use crate::rules::roundtable::ADVENTURES;
use std::path::{Path, PathBuf};

type Keys = BTreeMap<u32, Input>;

fn press(id: u32) -> Keys {
    BTreeMap::from([(id, Input { interact: true, ..Default::default() })])
}

/// The frames of one scene on disk.
struct Reel {
    dir: PathBuf,
    n: u32,
    seats: Vec<u32>,
}

impl Reel {
    fn new(root: &Path, game: &str, seats: &[u32]) -> Reel {
        let dir = root.join(game);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Reel { dir, n: 0, seats: seats.to_vec() }
    }

    fn shot(&mut self, run: &Run) {
        let mut sheet = Img::black(384 * self.seats.len() as i32, 224);
        for (i, &seat) in self.seats.iter().enumerate() {
            if !run.players.contains_key(&seat) {
                continue;
            }
            sheet.stamp(&frame_for(run, 384, 224, Some(seat)), 384 * i as i32, 0);
        }
        image::save_buffer(
            self.dir.join(format!("f{:04}.png", self.n)),
            &sheet.rgb_bytes(),
            sheet.w as u32,
            sheet.h as u32,
            image::ColorType::Rgb8,
        )
        .unwrap();
        self.n += 1;
    }
}

/// Step `skip` ticks unseen, then record `ticks` ticks, a frame every second one.
fn play(
    root: &Path,
    game: &str,
    run: &mut Run,
    seats: &[u32],
    skip: u32,
    ticks: u32,
    mut keys: impl FnMut(&Run, u32) -> Keys,
) {
    let mut reel = Reel::new(root, game, seats);
    for t in 0..skip + ticks {
        let k = keys(run, t);
        run.step(&k);
        if t >= skip && (t - skip) % 2 == 0 {
            reel.shot(run);
        }
    }
    println!("{game}: {} frames, {} seats on the table", reel.n, run.players.len());
}

fn join(r: &mut Run, seats: u32) {
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
}

/// A seated room game: the party joins, knight one presses E, the game opens.
fn room_game(mut r: Run, seats: u32, open: impl Fn(&Run) -> bool) -> Run {
    join(&mut r, seats);
    r.step(&press(1));
    assert!(open(&r), "the game did not open");
    r
}

fn want(game: &str) -> bool {
    std::env::var("AUDIT_ONLY").map_or(true, |o| o.split(',').any(|x| x == game))
}

fn hash(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 13)
}

/// A knight who wanders: a new heading every `hold` ticks.
fn wander(id: u32, t: u32, hold: u32) -> Input {
    let h = hash(id, t / hold);
    let (x, y) = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)][(h % 8) as usize];
    Input { move_x: x, move_y: y, ..Default::default() }
}

/// Aim at the nearest foe and shoot.
fn gunner(r: &Run, id: u32) -> Input {
    let Some(me) = r.players.get(&id) else { return Input::default() };
    let near = r.enemies.iter().min_by(|a, b| {
        let da = (a.x - me.x).hypot(a.y - me.y);
        let db = (b.x - me.x).hypot(b.y - me.y);
        da.total_cmp(&db)
    });
    let Some(e) = near else { return wander(id, r.tick as u32, 30) };
    let (dx, dy) = (e.x - me.x, e.y - me.y);
    let s = |v: f32, big: f32| if v.abs() < big * 0.4 { 0 } else { v.signum() as i8 };
    let big = dx.abs().max(dy.abs());
    Input { aim_x: s(dx, big), aim_y: s(dy, big), fire: true, ..Default::default() }
}

fn first_run(find: impl Fn(u64) -> Option<Run>) -> Run {
    (0..120u64).find_map(find).expect("a seed with the room")
}

#[test]
#[ignore = "writes the audit baselines"]
fn write_audit_shots() {
    let Some(root) = std::env::var_os("ANGEL_AUDIT_SHOTS") else {
        return;
    };
    let root = PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();

    // Latch Duel: read the hook, counter the beat.
    if want("latch") {
        let mut r = room_game(Run::latch_for_test(), 4, |r| r.latch.is_some());
        play(&root, "latch", &mut r, &[1, 2], 0, 240, |r, _| {
            r.latch.as_ref().map_or_else(Keys::new, |s| s.duels.iter().map(|(&id, d)| (id, d.bot_input(1 + id % 3))).collect())
        });
    }
    // Cinder Stack.
    if want("cinder") {
        let mut r = room_game(Run::cinder_for_test(), 4, |r| r.cinder.is_some());
        // The bot at an eighth of its speed: at full speed it cracks the kiln in four seconds.
        play(&root, "cinder", &mut r, &[1, 2], 0, 420, |r, t| {
            if t % 8 != 0 {
                return Keys::new();
            }
            r.cinder.as_ref().map_or_else(Keys::new, |s| s.boards.iter().map(|(&id, b)| (id, b.bot_input())).collect())
        });
    }
    // Mossbell.
    if want("mossbell") {
        let mut r = room_game(Run::mossbell_for_test(), 4, |r| r.mossbell.is_some());
        play(&root, "mossbell", &mut r, &[1, 2], 0, 300, |r, _| {
            r.mossbell.as_ref().map_or_else(Keys::new, |s| s.tables.iter().map(|(&id, t)| (id, t.bot_input())).collect())
        });
    }
    // Pennon Rush.
    if want("pennon") {
        use crate::rules::pennon::LENGTH;
        let mut r = room_game(Run::pennon_for_test(), 4, |r| r.pennon.is_some());
        play(&root, "pennon", &mut r, &[1, 2], 0, 300, |r, _| {
            r.pennon.as_ref().map_or_else(Keys::new, |s| s.lanes.iter().map(|(&id, l)| (id, l.bot_input(LENGTH - s.left))).collect())
        });
    }
    // Rafterwake.
    if want("rafterwake") {
        use crate::rules::rafterwake::LENGTH;
        let mut r = room_game(Run::rafterwake_for_test(), 4, |r| r.rafterwake.is_some());
        play(&root, "rafterwake", &mut r, &[1, 2], 0, 300, |r, _| {
            r.rafterwake.as_ref().map_or_else(Keys::new, |s| s.lanes.iter().map(|(&id, l)| (id, l.bot_input(LENGTH - s.left))).collect())
        });
    }
    // Brasswake.
    if want("brasswake") {
        use crate::rules::brasswake::LENGTH;
        let mut r = room_game(Run::brasswake_for_test(), 4, |r| r.brasswake.is_some());
        play(&root, "brasswake", &mut r, &[1, 2], 0, 300, |r, _| {
            r.brasswake.as_ref().map_or_else(Keys::new, |s| s.lanes.iter().map(|(&id, l)| (id, l.bot_input(LENGTH - s.left))).collect())
        });
    }

    // The Round Table: each adventure's own bot, every seat at once.
    for game in ["bannerpeak", "lightgallop", "griffinwing"] {
        if !want(game) {
            continue;
        }
        let a = ADVENTURES.iter().find(|a| a.slug == game).unwrap();
        let mut r = Run::roundtable_for_test(game).unwrap();
        join(&mut r, 4);
        r.step(&Keys::new());
        play(&root, game, &mut r, &[1, 2], 0, 360, |r, _| {
            (a.seated)(r).into_iter().map(|id| (id, (a.bot)(r, id))).collect()
        });
    }

    // Forest Road: four knights, the captors' machines.
    if want("forest_road") {
        let mut r = Run::forest_road_for_test();
        join(&mut r, 4);
        r.start_forest_road();
        play(&root, "forest_road", &mut r, &[1, 2], 0, 360, |r, _| {
            let Some(s) = r.forest_road.as_ref() else { return Keys::new() };
            r.players.iter().map(|(&id, h)| (id, s.bot_input(id, h))).collect()
        });
    }

    // The follies of the delve: a hall that plays like another game.
    // The Wax Maze (floor one's folly).
    if want("wax_maze") {
        let mut r = Run::new(41, 1, None);
        join(&mut r, 4);
        r.enemies.clear();
        r.pick_folly();
        for h in r.players.values_mut() {
            h.carried.add(Spoil::Gold, 50);
        }
        r.enter_for_test(r.folly_room.expect("a folly room"));
        play(&root, "wax_maze", &mut r, &[1, 2], 0, 360, |r, t| {
            r.players.keys().map(|&id| (id, wander(id, t, 14))).collect::<Keys>()
        });
    }
    // The Belfry Shoot (the even floors' folly).
    if want("belfry_shoot") {
        let mut r = Run::new(41, 1, None);
        r.descend_for_test();
        join(&mut r, 4);
        r.enemies.clear();
        r.pick_folly();
        for h in r.players.values_mut() {
            h.carried.add(Spoil::Gold, 40);
        }
        r.enter_for_test(r.folly_room.expect("a hall on floor two"));
        // Past the title, into the first flocks.
        play(&root, "belfry_shoot", &mut r, &[1, 2], folly::TITLE - 20, 330, |r, _| {
            r.players.keys().map(|&id| (id, gunner(r, id))).collect()
        });
    }
    // The Powder Room (the third floors' folly).
    if want("powder") {
        let mut r = first_run(|seed| {
            let mut r = Run::new(seed, 1, None);
            r.descend_for_test();
            r.descend_for_test();
            (r.dungeon.depth % 3 == 0).then_some(r)
        });
        join(&mut r, 4);
        r.enemies.clear();
        r.pick_folly();
        r.enter_for_test(r.folly_room.expect("a hall"));
        play(&root, "powder", &mut r, &[1, 2], 0, 420, |r, t| {
            // Each knight walks in from their corner, drops a keg, backs off, and goes round again.
            let (cx, cy) = (r.room().width() / 2.0, r.room().height() / 2.0);
            r.players
                .iter()
                .map(|(&id, h)| {
                    let (tx, ty) = ((cx - h.x).signum() as i8, (cy - h.y).signum() as i8);
                    let phase = (t + id * 9) % 90;
                    let (dx, dy) = if phase < 40 { (tx, 0) } else if phase < 55 { (0, ty) } else if phase < 75 { (-tx, 0) } else { (0, -ty) };
                    (id, Input { move_x: dx, move_y: dy, bomb: phase == 40 || phase == 12, ..Default::default() })
                })
                .collect::<Keys>()
        });
    }
    // Book Hop (the sixth floors' folly).
    if want("book_hop") {
        let mut r = first_run(|seed| {
            let mut r = Run::new(seed, 1, None);
            for _ in 0..5 {
                r.descend_for_test();
            }
            (r.dungeon.depth % 6 == 0).then_some(r)
        });
        join(&mut r, 4);
        r.enemies.clear();
        r.pick_folly();
        r.enter_for_test(r.folly_room.expect("a hall"));
        play(&root, "book_hop", &mut r, &[1, 2], folly::TITLE - 20, 360, |r, t| {
            r.players
                .keys()
                .map(|&id| {
                    let h = hash(id, t / 16);
                    (id, Input { move_y: if h % 3 == 0 { 0 } else { -1 }, move_x: [0, 1, -1][(h / 3 % 3) as usize], ..Default::default() })
                })
                .collect::<Keys>()
                
        });
    }
    // The Keg Gantry (the side-on hall).
    if want("keg_gantry") {
        let mut r = first_run(|seed| {
            let r = Run::new(seed, 1, None);
            r.dungeon.rooms.iter().any(|room| room.kind == RoomKind::Ledge && room.cols == 48).then_some(r)
        });
        join(&mut r, 4);
        r.enemies.clear();
        r.pick_gantry();
        r.enter_for_test(r.gantry_room.expect("the gantry hall"));
        // Seat one climbs at a steady pace, the rest dither: the game ends in seconds if all four climb well.
        play(&root, "keg_gantry", &mut r, &[1, 2], 0, 420, |r, t| {
            r.players
                .keys()
                .map(|&id| {
                    let u = t % 24;
                    let climber = id == 1 || (id == 2 && t > 120);
                    let up = climber && (u < 6 || (12..18).contains(&u));
                    (id, Input { move_y: if up { -1 } else { 0 }, move_x: if (t / 70 + id) % 2 == 0 { 1 } else { -1 }, ..Default::default() })
                })
                .collect::<Keys>()
        });
    }

    // The races: one hall, three 3D courses.
    for (game, course) in [
        ("race3d_ball", race3d::Course::Shaft),
        ("race3d_cart", race3d::Course::Cart),
        ("race3d_griffin", race3d::Course::Griffin),
    ] {
        if !want(game) {
            continue;
        }
        let mut r = first_run(|seed| {
            let mut r = Run::new(seed, 1, None);
            for _ in 0..6 {
                r.descend_for_test();
                r.race3d_hall_for_test();
                if race3d::Course::of_floor(r.dungeon.depth) == course && r.race3d_room.is_some() {
                    return Some(r);
                }
            }
            None
        });
        join(&mut r, 4);
        r.enemies.clear();
        for h in r.players.values_mut() {
            h.carried.add(Spoil::Gold, 60);
        }
        r.enter_for_test(r.race3d_room.unwrap());
        play(&root, game, &mut r, &[1, 2], 20, 330, |r, _| {
            if r.race3d.is_none() {
                return Keys::new();
            }
            r.players.keys().map(|&id| (id, autopilot(r, id))).collect()
        });
    }

    // The Hippodrome's chariot race (the stable's coach plate).
    if want("chariot") {
        let mut r = Run::at_home(7, 1, Some("Gawain"), Default::default(), Default::default());
        join(&mut r, 4);
        let hippo = world::room_of(&r.dungeon, RoomKind::Hippodrome).expect("the Hippodrome");
        let (c, row, _, _) = hippodrome::COACH;
        r.arrive(hippo, (c as f32 + 0.5, row as f32 + 0.5));
        r.step(&Keys::new());
        r.board_chariot(1);
        play(&root, "chariot", &mut r, &[1, 2], 0, 330, |r, t| {
            r.players
                .keys()
                .map(|&id| {
                    let h = hash(id, t / 40);
                    (id, Input { dash: (t / 20 + id) % 3 != 0, move_x: [0, 0, 1, -1][(h % 4) as usize], fire: t % 70 == id, ..Default::default() })
                })
                .collect::<Keys>()
                
        });
    }

    // The lists: two knights joust, the rest wager from the gallery.
    if want("joust") {
        let mut r = Run::at_home(7, 1, Some("Gawain"), Default::default(), Default::default());
        join(&mut r, 4);
        let lists = world::room_of(&r.dungeon, RoomKind::Lists).expect("the lists");
        r.arrive(lists, (1.9, 8.5));
        for id in 1..=4u32 {
            let h = r.players.get_mut(&id).unwrap();
            h.invulnerable = 0;
            (h.x, h.y) = match id {
                1 => (1.9 * TILE_UNITS, 8.5 * TILE_UNITS),
                n => ((3.0 + 5.0 * n as f32) * TILE_UNITS / 2.0, 21.0),
            };
        }
        r.step(&Keys::new());
        for _ in 0..home::BUY_HOLD + 1 {
            r.step(&press(1));
        }
        r.step(&Keys::new());
        assert!(r.joust.is_some(), "mounted at the plate");
        play(&root, "joust", &mut r, &[1, 2], 0, 360, |r, _| {
            let Some(j) = r.joust.as_ref() else { return Keys::new() };
            let foe = j.foe.as_ref().map(|f| f.id);
            let mut out = Keys::new();
            let ready = j.stage == joust::Stage::Ready;
            let charge = j.stage == joust::Stage::Charge;
            let next = j.t + 1;
            let strike = charge && next + 2 == j.meeting();
            let rider = Input { fire: ready || strike, move_y: -1, ..Default::default() };
            out.insert(1, rider);
            if let Some(id) = foe {
                out.insert(id, Input { fire: ready, move_y: 1, ..Default::default() });
            }
            out
        });
    }

    // The Griffin Eyrie: the yard, the perch, then a free flight down the gorge.
    if want("eyrie") {
        let mut home = home::Home::default();
        home.deepest = 1;
        let mut r = Run::at_home(53, 1, None, home, Default::default());
        join(&mut r, 4);
        let geo = map::geography(&r.kingdom.wishes);
        r.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
        r.floor_plates.clear();
        let site = geo.sites.iter().position(|s| s.room == RoomKind::Eyrie).unwrap();
        r.enter_kingdom_place(site);
        for _ in 0..HZ {
            r.step(&Keys::new());
        }
        for hero in r.players.values_mut() {
            (hero.x, hero.y) = (race3d::PERCH.0 * TILE_UNITS, (race3d::PERCH.1 + 1.0) * TILE_UNITS);
        }
        play(&root, "eyrie", &mut r, &[1, 2], 0, 330, |r, t| {
            if r.race3d.is_none() {
                return if t < 4 { Keys::new() } else { press(1) };
            }
            r.players.keys().map(|&id| (id, autopilot(r, id))).collect()
        });
    }

    // Fortune's wheel: the lever held, the wheel spun.
    if want("fortune") {
        let mut home = home::Home { deepest: 3, ..home::Home::default() };
        home.levels.insert(home::Station::Wheel, 3);
        let mut r = Run::at_home(7, 1, None, home, Default::default());
        join(&mut r, 4);
        r.enter_for_test(1);
        let (c, row, w, h) = fortune::LEVER;
        for hero in r.players.values_mut() {
            hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
            hero.y = (row as f32 + h as f32 / 2.0) * TILE_UNITS;
        }
        r.step(&Keys::new());
        play(&root, "fortune", &mut r, &[1, 2], 0, 300, |_, _| press(1));
    }

    // The Eastern high roads' pigeon post (`aerie.rs`).
    if want("pigeon_post") {
        use crate::rules::aerie as a;
        let mut r = Run::at_home(7, 1, None, Default::default(), Default::default());
        r.come_to_the_gate();
        join(&mut r, 3);
        r.home.countryside.post = 1;
        let (kind, at, _) = a::DOCUMENTS[0];
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
        play(&root, "pigeon_post", &mut r, &[1, 2], 0, 300, |r, t| {
            r.players
                .keys()
                .map(|&id| {
                    let mut i = wander(id, t, 40);
                    i.fire = t % 6 < 3;
                    i.aim_y = -1;
                    (id, i)
                })
                .collect::<Keys>()
                
        });
    }

    // The Swamp of Horrors: floor one, a bog hand and its friends.
    if want("swamp") {
        let mut r = Run::new(11, 1, None);
        r.begin_in(Pack::Swamp);
        join(&mut r, 4);
        let fight = r.dungeon.rooms.iter().position(|x| x.kind == RoomKind::Fight).unwrap();
        r.enter_for_test(fight);
        r.enemies.clear();
        for (i, kind) in [EnemyKind::BogHand, EnemyKind::Drowned].into_iter().cycle().take(5).enumerate() {
            let (cx, cy) = (r.room().width() / 2.0, r.room().height() / 2.0);
            r.spawn_at_for_test(kind, cx + (i as f32 - 2.0) * 40.0, cy - 60.0);
        }
        play(&root, "swamp", &mut r, &[1, 2], 0, 300, |r, _| {
            r.players.keys().map(|&id| (id, gunner(r, id))).collect()
        });
    }

    // The Ranch: the barn, the rail, the paddock; knights walk the yard.
    if want("ranch") {
        let mut r = Run::at_home(23, 1, Some("Rider"), home::Home::default(), Default::default());
        r.come_to_the_gate();
        join(&mut r, 4);
        r.arrive(world::room_of(&r.dungeon, RoomKind::Ranch).unwrap(), (12.0, 10.5));
        play(&root, "ranch", &mut r, &[1, 2], 0, 240, |r, t| {
            r.players.keys().map(|&id| (id, wander(id, t, 30))).collect::<Keys>()
        });
    }

    // The Sozzled Griffin: the hall, the dartboard, the arm-wrestling table.
    if want("alehouse") {
        let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
        join(&mut r, 4);
        r.walk_alehouse();
        r.floor_plates.clear();
        r.step(&Keys::new());
        play(&root, "alehouse", &mut r, &[1, 2], 0, 240, |r, t| {
            r.players.keys().map(|&id| (id, wander(id, t, 30))).collect::<Keys>()
        });
    }

    // The Deep: a village below the Unknown, the star fallen.
    if want("deep") {
        let mut r = Run::new(41, 1, None);
        join(&mut r, 4);
        let mut h = home::Home { deepest: DEEPEST, ..home::Home::default() };
        h.levels.insert(home::Station::Map, 4);
        r.home = h;
        r.deep_floor_for_test(deep::CHAPTER);
        for hero in r.players.values_mut() {
            hero.invulnerable = 0;
        }
        play(&root, "deep", &mut r, &[1, 2], 0, 300, |r, t| {
            r.players.keys().map(|&id| (id, wander(id, t, 30))).collect::<Keys>()
        });
    }

    // Lanternward: its rules run in the Bend module, so the test build plays
    // none. This is the module's recorded moments, a second of each.
    if want("lanternward") {
        let moments = [
            ("new", 1), ("slash3", 1), ("lamp-wave", 2), ("gallery-tell", 1), ("vault", 1),
            ("warden-open", 4), ("warden-slam", 2), ("warden-dark", 4), ("finale", 2),
        ];
        let mut reel = Reel::new(&root, "lanternward", &[1, 2]);
        for (m, seats) in moments {
            let mut r = Run::lanternward_at_for_test(m, seats);
            for _ in 0..8 {
                r.step(&Keys::new());
                r.step(&Keys::new());
                reel.shot(&r);
            }
        }
        println!("lanternward: {} frames (recorded moments)", reel.n);
    }

    // Moonfall: its rules run in the Bend module too, so this is the town
    // posed, the clock turned a half hour a frame and the knights walking.
    if want("moonfall") {
        use moonfall::{clock, state::{Knight, SPAWN, Seat}};
        let mut r = Run::moonfall_for_test();
        join(&mut r, 4);
        {
            let s = r.moonfall.as_mut().unwrap();
            for id in 2..=4u32 {
                s.seats.insert(id, Seat { joined: 1, knight: Knight::at(SPAWN[(id as usize + 5) % 6]) });
            }
            s.peak = s.seats.len() as u32;
        }
        let mut reel = Reel::new(&root, "moonfall", &[1, 2]);
        for f in 0..96u32 {
            let s = r.moonfall.as_mut().unwrap();
            s.clock = clock::ticks(s.nums.day, f);
            for (&id, seat) in s.seats.iter_mut() {
                let (cx, cy) = SPAWN[(id as usize + 5) % 6];
                let a = f as f32 / 9.0 + id as f32;
                seat.knight.x = ((cx * 16 + 8) as f32 + 22.0 * a.cos()) as i32 * 256;
                seat.knight.y = ((cy * 16 + 8) as f32 + 22.0 * a.sin()) as i32 * 256;
                seat.knight.stride = f * 3;
                seat.knight.face = [3, 0, 2, 1][((a / std::f32::consts::FRAC_PI_2) as usize + 1) % 4];
            }
            r.tick += 2;
            reel.shot(&r);
        }
        println!("moonfall: {} frames (posed)", reel.n);
    }
}

/// A knight who keys the obvious things: forward, middle, hop the drift,
/// switch off an ending line, jump what is ahead, fly round the stone
/// (`together_race3d__tests::autopilot`, which is private to that module).
fn autopilot(run: &Run, id: u32) -> Input {
    use race3d::{Course, Ground, ItemKind, drift_bot, laid, rotten, track, ROT};
    let r = run.race3d.as_ref().unwrap();
    let Some(me) = r.riders.get(&id) else { return Input::default() };
    let t = track(r.course);
    let mut input = Input::default();
    match r.course {
        Course::Touge => return drift_bot(r, id),
        Course::Shaft => {
            input.move_y = -1;
            let at = t.at(me.s);
            let aim = if at.ground == Ground::Lodestone { 0.6 } else { 0.0 };
            let drift = me.x + me.vx * 0.5;
            if drift > aim + 0.25 {
                input.move_x = -1;
            } else if drift < aim - 0.25 {
                input.move_x = 1;
            }
            let plank = ((me.s + 1.2) / 2.0) as u16;
            let gone = t.at(me.s + 1.2).ground == Ground::Collapse
                && rotten(plank)
                && r.planks.get(&plank).is_some_and(|&b| r.t + 3 >= b + ROT);
            input.fire = gone && me.z == 0.0;
            if (at.width < 3.0 || t.at(me.s + 6.0).width < 3.0) && me.vs > 5.0 {
                input.move_y = 1;
            }
        }
        Course::Cart => {
            if (1..=8).any(|d| !laid(t, me.s + d as f32, me.lane)) {
                input.interact = r.t % 2 == 0;
                input.move_x = if me.lane == 0 || (me.lane == 1 && laid(t, me.s + 8.0, 2)) { 1 } else { -1 };
            }
            let v = me.vs.max(1.0);
            let gap = t.gaps.iter().any(|g| g.lane == me.lane && (0.3..0.6 + 0.2 * v).contains(&(g.s.0 - me.s)));
            let timber = t.items.iter().any(|i| {
                i.kind == ItemKind::Timber && (i.x - me.x).abs() < 1.0 && (0.6 + 0.2 * v..0.6 + 0.4 * v).contains(&(i.s - me.s))
            });
            let threat = gap || timber;
            input.fire = threat && me.z == 0.0 && me.keys & 1 == 0;
        }
        Course::Griffin => {
            let mut best = (f32::MAX, 0.0f32, 0.0f32);
            for gx in [-3.0f32, -1.5, 0.0, 1.5, 3.0] {
                for gz in [-2.0f32, 0.0, 2.0] {
                    let blocked = t.blocks.iter().any(|b| {
                        b.s > me.s - 1.0 && b.s < me.s + 18.0 && gx + 1.0 > b.x.0 && gx - 1.0 < b.x.1 && gz + 0.9 > b.z.0 && gz - 0.9 < b.z.1
                    }) || t.items.iter().enumerate().any(|(i, g)| {
                        g.kind == ItemKind::Gargoyle && !r.taken.contains(&(i as u16)) && g.s > me.s && g.s < me.s + 12.0 && (g.x - gx).hypot(g.z - gz) < 1.6
                    });
                    let cost = if blocked { 1000.0 } else { 0.0 } + (gx - me.x).abs() + (gz - me.z).abs();
                    if cost < best.0 {
                        best = (cost, gx, gz);
                    }
                }
            }
            if best.1 > me.x + 0.3 {
                input.move_x = 1;
            } else if best.1 < me.x - 0.3 {
                input.move_x = -1;
            }
            if best.2 > me.z + 0.3 {
                input.move_y = -1;
            } else if best.2 < me.z - 0.3 {
                input.move_y = 1;
            }
            input.fire = r.t % 12 == 0;
        }
    }
    input
}
