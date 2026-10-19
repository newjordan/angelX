//! Bannerpeak through the real arena: the labelled blocks at every moment of
//! the way, for one to six knights. `ANGEL_ARENA_SHOTS=<dir> cargo test -p
//! angel-delve write_bannerpeak_review -- --ignored` writes the pictures to
//! open: native, three times over, and the terminal's layers.
use super::super::{art_v2, frame_for};
use super::*;
use super::super::super::ink::{ink, palette_index};
use crate::rules::bannerpeak::{
    knight::follow,
    boss, foes,
    layout::{self, HUB, MARSH, MILL, SHAFT, SNOW, SUMMIT, info},
    session::Phase,
    world::Tile,
};
use crate::rules::{MAX_PLAYERS, Run};
use std::collections::BTreeMap;

fn party(seats: u32) -> Run {
    let mut r = Run::bannerpeak_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

/// Knight `seat` standing in the air cell `cell`, the camera settled on them.
fn put(r: &mut Run, seat: u32, cell: (i32, i32)) {
    let s = r.bannerpeak.as_mut().unwrap();
    let k = s.seats.get_mut(&seat).unwrap();
    let (x, y) = layout::cell_feet(cell);
    k.body.place(x, y);
    k.safe = (x, y);
    for _ in 0..90 {
        k.cam = follow(k.cam, x, y, k.body.face, 0, 0);
    }
}

/// A named moment of the run, with `seats` knights at the table.
fn moment(name: &str, seats: u32) -> Run {
    let mut r = party(seats);
    let spots: [(&str, (i32, i32)); 23] = [
        ("hub-start", HUB.at(4, 17)),
        ("hub-pole", HUB.at(38, 17)),
        ("hub-tower", HUB.at(52, 17)),
        ("hub-cellar", HUB.at(22, 27)),
        ("marsh-start", MARSH.at(8, 13)),
        ("marsh-blocks", MARSH.at(14, 13)),
        ("marsh-gap", MARSH.at(60, 11)),
        ("mill-wheel", MILL.at(28, 13)),
        ("shaft", SHAFT.at(7, 33)),
        ("snow", SNOW.at(40, 9)),
        ("summit", SUMMIT.at(14, 11)),
        ("goal-pole", MARSH.at(96, 13)),
        ("poses", HUB.at(34, 17)),
        ("bubble", MARSH.at(30, 13)),
        ("boulder", SNOW.at(30, 9)),
        ("shaft-kick", SHAFT.at(7, 20)),
        ("pole-slide", MARSH.at(97, 13)),
        ("bump", MARSH.at(14, 13)),
        ("flame", MARSH.at(14, 13)),
        ("gates", MARSH.at(8, 13)),
        ("wheel-ride", MILL.at(28, 13)),
        ("marsh-hatch", MARSH.at(44, 11)),
        ("spikes", SNOW.at(17, 9)),
    ];
    let key = name.split(':').next().unwrap();
    let cell = spots.iter().find(|(n, _)| *n == key).map(|&(_, c)| c);
    if let Some(cell) = cell {
        for id in 1..=seats {
            // Spread the party a little along the ground.
            put(&mut r, id, (cell.0 + (id as i32 - 1) * 2, cell.1));
        }
    }
    let s = r.bannerpeak.as_mut().unwrap();
    // The wheel's moments name a tick of its turn (270 ticks); two turns in,
    // so the opening card is long gone.
    s.elapsed = name.split(':').nth(1).and_then(|t| t.parse::<u32>().ok()).map_or(400, |t| 540 + t);
    match name {
        "hub-pole" => s.banners = 0b0111_1111,
        "select" => {
            s.banners = 0b11;
            s.doors = 1;
            s.phase = Phase::Select;
        }
        "finale" => {
            s.banners = 0xFF;
            s.elapsed = 5000;
            let mut said = Vec::new();
            s.step(
                &BTreeMap::new(),
                &r.players.clone(),
                &crate::rules::bannerpeak::body::Knobs {
                    run: 1152,
                    hop: 10,
                    grace: 5,
                    slam: 36,
                    ledge: 3000,
                },
                &mut said,
            );
        }
        "poses" => {
            let all = [
                Pose::Spin,
                Pose::Long,
                Pose::Slam,
                Pose::Brace,
                Pose::Dive,
                Pose::Wall,
            ];
            for (n, (_, k)) in s.seats.iter_mut().enumerate() {
                k.body.pose = all[n % all.len()];
                if n % 3 == 1 {
                    k.body.big = true;
                    k.size = crate::rules::bannerpeak::knight::Size::Big;
                }
            }
        }
        "bubble" => {
            if let Some(k) = s.seats.get_mut(&2) {
                let (x, y) = k.body.foot();
                k.bubble = Some(crate::rules::bannerpeak::knight::Bubble {
                    x: (x + 40) * Q,
                    y: (y - 50) * Q,
                    t: 40,
                });
            }
        }
        "boulder" => {
            s.boulders.push(crate::rules::bannerpeak::foes::Boulder {
                x: (SNOW.ox + 40) * 16 * Q,
                t: 10,
            });
            s.boulders.push(crate::rules::bannerpeak::foes::Boulder {
                x: (SNOW.ox + 22) * 16 * Q,
                t: 90,
            });
        }
        "summit:tell" | "summit:wince" | "summit:strike" | "summit:rage" => {
            s.banners = 0x7F;
            s.doors = 3;
            let beat = name.split(':').nth(1).unwrap();
            s.boss.beat = match beat {
                "tell" => crate::rules::bannerpeak::boss::Beat::Tell,
                "strike" => crate::rules::bannerpeak::boss::Beat::Strike,
                "rage" => crate::rules::bannerpeak::boss::Beat::Rage,
                _ => crate::rules::bannerpeak::boss::Beat::Wince,
            };
            s.boss.t = 20;
            if beat == "wince" {
                s.boss.head = crate::rules::bannerpeak::boss::HEAD_DOWN;
            }
            if beat == "tell" || beat == "strike" {
                s.boss.kind = crate::rules::bannerpeak::boss::Attack::Embers;
                s.boss.hazards = vec![
                    crate::rules::bannerpeak::boss::Hazard {
                        x: boss::home() - 90,
                        y: (SUMMIT.oy + 1) * 16,
                        w: 20,
                        h: 11 * 16,
                        vx: 0,
                        tell: if beat == "tell" { 10 } else { 0 },
                        live: 12,
                    },
                    crate::rules::bannerpeak::boss::Hazard {
                        x: boss::home() + 70,
                        y: boss::floor() - 14,
                        w: 16,
                        h: 14,
                        vx: 3,
                        tell: 0,
                        live: 50,
                    },
                ];
            }
        }
        "bump" => {
            // The three rune blocks over the marsh's first stretch, struck at
            // three different moments of the eight-tick bump.
            for (n, age) in [0u32, 3, 6].into_iter().enumerate() {
                let (tx, ty) = MARSH.at(10 + n as i32, 9);
                s.fx.push(crate::rules::bannerpeak::session::Fx {
                    k: crate::rules::bannerpeak::session::FxKind::Bump,
                    x: tx * 16 + 8,
                    y: ty * 16,
                    t: crate::rules::bannerpeak::session::FX_LIFE - age,
                });
            }
        }
        "flame" => {
            // The lantern flame item at three moments: a bead rising out of
            // its block, half risen, and flickering at rest.
            let (fx, fy) = s.seats[&1].body.foot();
            for (n, t) in [1u32, 9, 40].into_iter().enumerate() {
                let mut item = crate::rules::bannerpeak::foes::Item::new(
                    crate::rules::bannerpeak::foes::ItemKind::Flame,
                    fx + 30 + n as i32 * 24,
                    fy,
                );
                item.t = t;
                s.items.push(item);
            }
        }
        "gates" => s.gates = 1,
        n if n.starts_with("wheel-ride") => {
            // A knight on each of four buckets: the platforms the rules own,
            // and what the wheel's picture shows under them.
            let buckets: Vec<_> = crate::rules::bannerpeak::layout::movers(s.elapsed)
                .into_iter()
                .filter(|m| m.id >= crate::rules::bannerpeak::layout::WHEEL_ID)
                .collect();
            for (n, (_, k)) in s.seats.iter_mut().enumerate() {
                let m = &buckets[n * 2 % buckets.len()];
                k.body.place(m.p.x + m.p.w / 2, m.p.y);
            }
        }
        "pole-slide" => {
            if let Some(k) = s.seats.get_mut(&1) {
                k.pole = 12;
                k.body.pose = Pose::Pole;
                k.body.y -= 60 * Q;
            }
        }
        _ => {}
    }
    r
}

const MOMENTS: [&str; 29] = [
    "hub-start", "hub-pole", "hub-tower", "hub-cellar", "marsh-start", "marsh-blocks",
    "marsh-gap", "goal-pole", "mill-wheel", "shaft", "snow", "summit", "select", "finale",
    "poses", "bubble", "boulder", "summit:tell", "summit:strike", "summit:wince", "summit:rage",
    "pole-slide", "shaft-kick", "bump", "flame", "gates", "wheel-ride", "marsh-hatch", "spikes",
];

/// The mill wheel at other ticks of its turn (270 ticks to a turn).
const WHEEL_TICKS: [u32; 5] = [30, 75, 140, 200, 250];

#[test]
fn every_moment_draws_on_both_routes_for_every_table() {
    let mut seen = std::collections::BTreeSet::new();
    for name in &MOMENTS {
        for seats in [1, 2, 4, MAX_PLAYERS] {
            let r = moment(name, seats);
            let (base, fx) = layers(&r, Some(1));
            assert_eq!((base.w, base.h, fx.w, fx.h), (384, 224, 384, 224), "{name}");
            // The terminal paints its own words over the cells, so the picture
            // leaves its pixel words out where it has them (the board, the
            // finale) and is the same picture otherwise.
            let (tbase, _) = terminal_layers(&r, Some(1));
            assert_eq!((tbase.w, tbase.h), (384, 224), "{name}");
            if matches!(*name, "select" | "finale") {
                assert_ne!(tbase.content_hash(), base.content_hash(), "{name} drops its words");
            } else {
                assert_eq!(tbase.content_hash(), base.content_hash(), "{name}");
            }
            assert!(art_v2::terminal_layers(&r, Some(1)).is_some());
            for (w, h) in [(384, 224), (768, 448), (96, 56)] {
                let img = frame_for(&r, w, h, Some(1));
                assert_eq!((img.w, img.h), (w, h), "{name}");
            }
            if seats == 1 {
                assert!(seen.insert(base.content_hash()), "{name} looks like another moment");
            }
        }
    }
}

#[test]
fn each_seat_sees_its_own_camera() {
    let mut r = moment("hub-start", 3);
    put(&mut r, 2, HUB.at(40, 17));
    put(&mut r, 3, MARSH.at(30, 13));
    let a = layers(&r, Some(1)).0.content_hash();
    let b = layers(&r, Some(2)).0.content_hash();
    let c = layers(&r, Some(3)).0.content_hash();
    assert!(a != b && b != c && a != c);
}

#[test]
fn the_door_is_drawn_at_its_tile_only_while_in_view() {
    let r = Run::at_home(41, 1, None, Default::default(), Default::default());
    let (ex, ey) = rules::ENTRY;
    let mut near = Img::new(384, 224);
    kingdom(&mut near, &r, (ex * 16 + 8 - 192, ey * 16 + 8 - 112));
    assert_ne!(near.content_hash(), Img::new(384, 224).content_hash());
    let mut far = Img::new(384, 224);
    kingdom(&mut far, &r, (ex * 16 + 2000, ey * 16 + 2000));
    assert_eq!(far.content_hash(), Img::new(384, 224).content_hash());
    let _ = info();
}


// ------------------------------------------------------- the painted slots

/// Whether every opaque pixel of `pic`, its anchor on screen `(x, y)`, is on
/// `screen`.
fn shows(screen: &Img, pic: &bank::Pic, x: i32, y: i32) -> bool {
    (0..pic.img.h).all(|py| {
        (0..pic.img.w).all(|px| {
            pic.img.get(px, py).is_none_or(|c| screen.get(x - pic.ax + px, y - pic.ay + py) == Some(c))
        })
    })
}

/// The first tile of `kind` in seat 1's view that is clear of the HUD, as the
/// screen position of its top left corner.
fn tile_on_screen(r: &Run, kind: Tile) -> Option<(i32, i32)> {
    let s = r.bannerpeak.as_ref().unwrap();
    let cam = s.seats[&1].cam;
    (0..16).flat_map(|row| (0..26).map(move |col| (col, row))).find_map(|(col, row)| {
        let (tx, ty) = (cam.0.div_euclid(16) + col, cam.1.div_euclid(16) + row);
        let (x, y) = (tx * 16 - cam.0, ty * 16 - cam.1);
        (s.tile(tx, ty) == kind && x >= 0 && x + 16 <= 384 && y >= 20 && y + 16 <= 212).then_some((x, y))
    })
}

#[test]
fn a_painted_tile_is_stamped_whole_where_the_world_has_it() {
    // (moment, tile, slot, state): at rest, tick 400.
    let cases = [
        ("hub-start", Tile::Stone, "tiles", "stone"),
        ("snow", Tile::Snow, "tiles", "snow"),
        ("hub-cellar", Tile::Ledge, "tiles", "ledge"),
        ("spikes", Tile::Spikes, "tiles", "spikes"),
        ("marsh-hatch", Tile::Hatch, "tiles", "hatch"),
        ("marsh-blocks", Tile::Coin, "rune-block", "idle"),
        ("marsh-blocks", Tile::Power, "rune-block", "idle"),
    ];
    for (name, kind, slot, state) in cases {
        let Some(pic) = bank::at(slot, state, 400) else { continue };
        let r = moment(name, 1);
        let (x, y) = tile_on_screen(&r, kind).unwrap_or_else(|| panic!("{name}: a {kind:?} on screen"));
        let screen = layers(&r, Some(1)).0;
        assert!(shows(&screen, pic, x + pic.ax, y + pic.ay), "{name}: the {kind:?} is the painted {slot} {state}");
    }
}

#[test]
fn a_struck_rune_block_flares_and_fades_over_its_eight_ticks() {
    // Three blocks over the marsh, struck 0, 3 and 6 ticks ago: they show the
    // bump frames the sheet's own durations pick, lifted as the rules lift.
    let r = moment("bump", 1);
    let s = r.bannerpeak.as_ref().unwrap();
    let cam = s.seats[&1].cam;
    let screen = layers(&r, Some(1)).0;
    for (n, age) in [0u32, 3, 6].into_iter().enumerate() {
        let Some(pic) = bank::at("rune-block", "bump", age) else { return };
        let (tx, ty) = MARSH.at(10 + n as i32, 9);
        let lift = [0, 2, 4, 5, 4, 2, 0, 0][age as usize];
        let (x, y) = (tx * 16 - cam.0, ty * 16 - cam.1 - lift);
        assert!(shows(&screen, pic, x + pic.ax, y + pic.ay), "bump at age {age}");
    }
}

#[test]
fn the_gate_is_dull_down_and_flies_once_raised() {
    let mut r = moment("marsh-start", 1);
    let (Some(down), Some(up)) = (bank::pic("gate", "down", 0), bank::at("gate", "up", 400)) else { return };
    let cam = r.bannerpeak.as_ref().unwrap().seats[&1].cam;
    let n = info()
        .gates
        .iter()
        .position(|&c| {
            let (cx, _) = layout::cell_centre(c);
            (40..344).contains(&(cx - cam.0))
        })
        .expect("a gate in view");
    let (cx, _) = layout::cell_centre(info().gates[n]);
    let (sx, sy) = (cx - cam.0, (info().gates[n].1 + 1) * 16 - cam.1);
    assert!(shows(&layers(&r, Some(1)).0, down, sx, sy), "a gate not yet raised hangs its dull pennon");
    r.bannerpeak.as_mut().unwrap().gates = 1 << n;
    assert!(shows(&layers(&r, Some(1)).0, up, sx, sy), "a raised gate's banner flies");
}

#[test]
fn a_bell_boulder_rolls_west_a_quarter_turn_counter_clockwise_each_frame() {
    let r = moment("boulder", 1);
    let s = r.bannerpeak.as_ref().unwrap();
    let cam = s.seats[&1].cam;
    let screen = layers(&r, Some(1)).0;
    let turns = bank::frames("boulder", "roll");
    if turns == 0 {
        return;
    }
    let mut checked = 0;
    for b in &s.boulders {
        let x = b.x.div_euclid(crate::rules::bannerpeak::body::Q) - cam.0;
        if !(40..344).contains(&x) {
            continue;
        }
        let seen = bank::index_at("boulder", "roll", b.t).unwrap();
        let pic = bank::pic("boulder", "roll", (turns - seen) % turns).unwrap();
        let foot = layout::SNOW_FLOOR * 16 - cam.1;
        assert!(shows(&screen, pic, x, foot), "the boulder at tick {}", b.t);
        checked += 1;
    }
    assert!(checked > 0, "a boulder in view");
}

#[test]
fn the_lantern_flame_kindles_over_the_ticks_it_rises_and_then_flickers() {
    let r = moment("flame", 1);
    let s = r.bannerpeak.as_ref().unwrap();
    let cam = s.seats[&1].cam;
    let screen = layers(&r, Some(1)).0;
    for it in s.items.iter().filter(|it| it.kind == foes::ItemKind::Flame) {
        let Some(pic) = (if it.rising() { bank::at("flame", "rise", it.t) } else { bank::at("flame", "idle", s.elapsed) }) else {
            return;
        };
        let (l, _, r_, b) = it.rect();
        assert!(shows(&screen, pic, (l + r_) / 2 - cam.0, b - cam.1), "flame item at t {}", it.t);
    }
}

#[test]
fn the_sheets_ticks_are_the_rules() {
    let slots: Vec<serde_json::Value> = serde_json::from_str(include_str!("../../assets/bannerpeak/slots.json")).unwrap();
    let ticks = |slot: &str, state: &str| -> u32 {
        let s = slots.iter().find(|s| s["id"] == format!("bannerpeak.{slot}")).unwrap();
        s["states"][state].as_array().unwrap().iter().map(|t| t.as_u64().unwrap() as u32).sum()
    };
    // The lantern kindles over the ticks the item is still rising, and a rune
    // block's bump is the eight ticks it lifts.
    assert_eq!(ticks("flame", "rise"), foes::RISE);
    assert_eq!(ticks("rune-block", "bump"), 8);
    // A bank is painted whole or not at all: a prop never changes look when it
    // changes state.
    for slot in &slots {
        let id = slot["id"].as_str().unwrap().trim_start_matches("bannerpeak.");
        let flags: Vec<bool> =
            slot["states"].as_object().unwrap().keys().map(|state| bank::pic(id, state, 0).is_some()).collect();
        assert!(flags.iter().all(|f| *f) || flags.iter().all(|f| !*f), "{id}: {flags:?}");
    }
}

#[test]
fn the_painted_sky_is_under_everything_and_goes_dark_under_the_street() {
    let mut day = Img::black(384, 224);
    if !sky::paint(&mut day, (0, 0)) {
        return;
    }
    // Every pixel of it is the palette's own ink, none opaque-black paper.
    assert!((0..224).all(|y| (0..384).all(|x| day.get(x, y).is_some_and(|c| palette_index(c).is_some()))));
    // A camera in the cellar sees only the flat deep.
    let deep = ink('s').unwrap();
    let mut cellar = Img::black(384, 224);
    sky::paint(&mut cellar, (0, 900));
    assert!((0..224).all(|y| (0..384).all(|x| cellar.get(x, y) == Some(deep))));
    // Above the street it is the picture: the same at any camera row.
    let mut a = Img::black(384, 224);
    let mut b = Img::black(384, 224);
    sky::paint(&mut a, (0, 100));
    sky::paint(&mut b, (400, 300));
    assert_eq!(a.content_hash(), b.content_hash());
    // And the stage draws the tiles and the knights over it, never under it:
    // a whole frame still shows its floor.
    let r = moment("hub-start", 4);
    let screen = layers(&r, Some(1)).0;
    let floor = tile_on_screen(&r, Tile::Stone).expect("floor in view");
    assert_ne!(screen.get(floor.0 + 8, floor.1 + 8), day.get(floor.0 + 8, floor.1 + 8));
}

#[test]
#[ignore = "writes Bannerpeak's review pictures"]
fn write_bannerpeak_review() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    for name in &MOMENTS {
        for seats in [1, 4] {
            let r = moment(name, seats);
            save(
                &format!("bannerpeak-{}-seats{seats}-native", name.replace(':', "-")),
                &frame_for(&r, 384, 224, Some(1)),
            );
        }
        let r = moment(name, 4);
        save(
            &format!("bannerpeak-{}-3x", name.replace(':', "-")),
            &frame_for(&r, 1152, 672, Some(2)),
        );
        let (base, fx) = art_v2::terminal_layers(&r, Some(1)).unwrap();
        let mut both = base.clone();
        both.stamp(&fx, 0, 0);
        save(&format!("bannerpeak-{}-terminal-layers", name.replace(':', "-")), &both);
    }
    // The wheel at five more ticks of its turn, native and three times over.
    for t in WHEEL_TICKS {
        let name = format!("mill-wheel:{t}");
        save(&format!("bannerpeak-wheel-t{t}-native"), &frame_for(&moment(&name, 4), 384, 224, Some(1)));
        save(&format!("bannerpeak-wheel-t{t}-3x"), &frame_for(&moment(&name, 4), 1152, 672, Some(2)));
        let ride = format!("wheel-ride:{t}");
        save(&format!("bannerpeak-wheel-ride-t{t}-native"), &frame_for(&moment(&ride, 4), 384, 224, Some(1)));
        save(&format!("bannerpeak-wheel-ride-t{t}-3x"), &frame_for(&moment(&ride, 4), 1152, 672, Some(2)));
    }
    let r = moment("marsh-start", MAX_PLAYERS);
    save("bannerpeak-seats6-native", &frame_for(&r, 384, 224, Some(1)));
    let mut k = Run::at_home(41, 1, Some("Gawain"), Default::default(), Default::default());
    k.walk_kingdom(Some((rules::ENTRY.0, rules::ENTRY.1 + 2)));
    k.follow_kingdom_cameras();
    save("bannerpeak-kingdom", &frame_for(&k, 384, 224, Some(1)));
}
