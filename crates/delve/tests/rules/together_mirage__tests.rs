//! The Mirage Duelist: a fast fencer with few images and a great deal of
//! movement. Swashbuckle is a dash that bounces off the room's walls and then
//! slashes four times; Mirror Swap changes places with an image; Echo Strike
//! strikes a thrust twice; the Hall of Mirrors sends three images at the
//! nearest foes; and the dodge roll leaves a decoy.

use super::*;
use crate::rules::champions::mirage::{self, Swash};
use crate::rules::spells::IMPOSTURE_MAX;
use crate::rules::ults::{PHANTOM_SHARE, Phantom};

/// The tiles the tests' room is walled down to: a clean box of floor, so a
/// wall is where the test says it is. Columns and rows of floor from the
/// room's (2, 2) (a fight room is 24 by 14 tiles); a tile is `TILE_UNITS` (2)
/// across, so the floor of the usual box runs from x = 4 to 44 and from y = 4
/// to 24, and the knight (0.42 round) stands no nearer a wall than 4.42, 43.58,
/// 4.42 and 23.58.
const BOX: (usize, usize) = (20, 10);
const LEFT: f32 = 4.0;
const TOP: f32 = 4.0;

fn boxed(r: &mut Run, (cols, rows): (usize, usize)) {
    let at = r.at;
    let room = &mut r.dungeon.rooms[at];
    assert!(room.cols >= 2 + cols && room.rows >= 2 + rows, "the room is {} by {}", room.cols, room.rows);
    for col in 0..room.cols {
        for row in 0..room.rows {
            let inside = (2..2 + cols).contains(&col) && (2..2 + rows).contains(&row);
            room.set(col, row, if inside { Tile::Floor } else { Tile::Wall });
        }
    }
}

/// A knight of `kit` standing at `(x, y)` aiming along `(ax, ay)` in the usual box.
fn arena(kit: &str, at: (f32, f32), aim: (f32, f32)) -> Run {
    arena_in(kit, at, aim, BOX)
}

/// The same in a box of `cols` by `rows` tiles.
fn arena_in(kit: &str, (x, y): (f32, f32), (ax, ay): (f32, f32), size: (usize, usize)) -> Run {
    let mut r = Run::new(21, 1, None);
    let fight = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(fight);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    boxed(&mut r, size);
    assert!(r.take_up(1, kit));
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (x, y);
    (h.aim_x, h.aim_y) = unit(ax, ay);
    h.invulnerable = 0;
    r.phase = Phase::Fighting;
    r
}

/// A second knight in the room, so another knight's images are kept (an
/// image of a knight who is not there is gone at the tick's end).
fn with_friend(r: &mut Run) {
    r.join_seat(2, "Friend");
}

/// A tough foe asleep where it stands.
fn foe(r: &mut Run, x: f32, y: f32) -> usize {
    r.spawn_at_for_test(EnemyKind::Demon, x, y);
    let i = r.enemies.len() - 1;
    let e = &mut r.enemies[i];
    (e.hp, e.max_hp, e.seen_hp, e.age) = (10000, 10000, 10000, TELEGRAPH);
    e.sleep_until = r.tick + 1_000_000;
    i
}

fn step(r: &mut Run, i: Input) {
    r.step(&BTreeMap::from([(1, i)]));
}

fn idle(r: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        step(r, Input::default());
    }
}

fn cast(r: &mut Run, key: u8) {
    step(r, Input { cast: key, ..Default::default() });
}

fn lost(r: &Run, i: usize) -> u32 {
    r.enemies[i].max_hp - r.enemies[i].hp
}

fn hero(r: &Run) -> &Hero {
    &r.players[&1]
}

fn swash(r: &Run) -> Swash {
    r.players[&1].champ.swash
}

fn mine(r: &Run) -> Vec<Phantom> {
    r.phantoms.iter().copied().filter(|p| p.owner == 1).collect()
}

fn walkable(r: &Run, (x, y): (f32, f32)) -> bool {
    r.settlement_walkable_for_test(x, y)
}

fn voices(r: &Run) -> usize {
    r.sounds.iter().filter(|s| **s == "mirage_bing_bong").count()
}

/// No thrust of the foil gets in the way of what is being counted.
fn no_thrusts(r: &mut Run) {
    r.players.get_mut(&1).unwrap().fire_cooldown = 1_000_000;
}

/// Cast Swashbuckle and run it until the dash is over, tracking the path.
fn dash(r: &mut Run) -> Vec<(f32, f32)> {
    cast(r, 1);
    assert!(swash(r).dashing(), "the dash is set running");
    let mut path = vec![(hero(r).x, hero(r).y)];
    for _ in 0..200 {
        if !swash(r).dashing() {
            break;
        }
        idle(r, 1);
        path.push((hero(r).x, hero(r).y));
    }
    assert!(!swash(r).dashing(), "the dash ends");
    path
}

fn length(path: &[(f32, f32)]) -> f32 {
    path.windows(2).map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)).sum()
}

// ─── the hero ────────────────────────────────────────────────────────────────

#[test]
fn the_mirage_duelist_is_a_champion_with_a_foil_and_three_spells() {
    use champions::Champion;
    assert_eq!(Champion::of_kit("mirageduelist"), Some(Champion::MirageDuelist));
    assert_eq!(Champion::MirageDuelist.spells(), ["swashbuckle", "mirror-swap", "echo-strike"]);
    assert_eq!(Champion::ALL.len(), 7);
    assert_eq!(Champion::ALL[5], Champion::PhantomLancer, "appended after the Phantom Lancer");
    assert_eq!(Champion::ALL[6], Champion::MirageDuelist);
    let k = knights::knight("mirageduelist").unwrap();
    assert_eq!((k.arm, k.guard, k.max_hp), ("mirage-foil", "dodge-roll", 110));
    let r = Run::new(21, 1, None);
    let cards::Arm::Forged(foil) = r.book.get("mirage-foil").expect("the foil is in the book").arm.clone().unwrap() else {
        panic!("a forged arm")
    };
    let thrust = foil.melee.unwrap();
    assert_eq!((thrust.damage, thrust.reach, thrust.arc, thrust.every), (14, 3.2, 30.0, 9));
    for spell in Champion::MirageDuelist.spells() {
        let card = r.book.get(spell).unwrap_or_else(|| panic!("{spell} is in the book"));
        assert_eq!(card.kind, cards::Kind::Spell);
        assert_eq!((card.drop, card.chest), (0, 0), "{spell} is the champion's alone");
    }
    assert_eq!(r.book.get("swashbuckle").unwrap().cooldown, 10, "near the Lancer's vault");
}

#[test]
fn the_hall_of_mirrors_is_appended_so_saved_names_keep() {
    let all = ults::Ult::ALL;
    assert_eq!(all.len(), 24);
    assert_eq!(all[21], ults::Ult::SkewerDrive);
    assert_eq!(all[22], ults::Ult::PhantomLegion);
    assert_eq!(all[23], ults::Ult::HallOfMirrors);
    assert_eq!(ults::Ult::of(Some("mirageduelist")), ults::Ult::HallOfMirrors);
    assert_eq!(serde_json::to_string(&ults::Ult::HallOfMirrors).unwrap(), "\"hall_of_mirrors\"");
    assert_eq!(serde_json::to_string(&ults::Ult::PhantomLegion).unwrap(), "\"phantom_legion\"");
    assert_eq!(ults::Ult::parse("Hall of Mirrors"), Some(ults::Ult::HallOfMirrors));
}

#[test]
fn the_patron_is_pickable_sits_apart_and_arms_the_knight() {
    let mut r = Run::new(21, 1, None);
    let i = alehouse::PATRONS.iter().position(|p| p.kit == "mirageduelist").expect("a seat in the tavern");
    let p = &alehouse::PATRONS[i];
    assert_eq!(alehouse::patron_room(i), RoomKind::Alehouse);
    assert!(p.pitch.len() < 80 && !p.label.contains(' '));
    r.alehouse.company_page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, i)).unwrap();
    r.pick_patron(1, i);
    let h = hero(&r);
    assert_eq!(h.knight.as_deref(), Some("mirageduelist"));
    assert_eq!(h.arm.as_deref(), Some("mirage-foil"));
    assert!(h.forged.as_ref().is_some_and(|w| w.melee.is_some()));
    assert_eq!(h.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(h.ult(), ults::Ult::HallOfMirrors);
    assert_eq!(h.max_hp, 110 + h.home_hp);
    // The three of them stand in a row, clear of one another and of every other seat.
    let near: Vec<_> = ["lancer", "phantomlancer", "mirageduelist"]
        .iter()
        .map(|k| alehouse::PATRONS.iter().find(|p| p.kit == *k).unwrap())
        .collect();
    assert_eq!(near[0].at.1, near[2].at.1, "one row");
    for (_, q) in alehouse::PATRONS.iter().enumerate().filter(|(j, _)| *j != i && alehouse::patron_room(*j) == RoomKind::Alehouse) {
        assert!((p.at.0 - q.at.0).hypot(p.at.1 - q.at.1) > 1.2, "{} sits apart from {}", p.label, q.label);
    }
}

// ─── Swashbuckle ─────────────────────────────────────────────────────────────

#[test]
fn a_dash_in_the_open_runs_exactly_its_base_length_and_never_bounces() {
    let mut r = arena("mirageduelist", (16.0, 18.0), (1.0, 0.0));
    let path = dash(&mut r);
    let s = swash(&r);
    assert!((length(&path) - mirage::SWASH_REACH).abs() < 0.01, "5.5 tiles: {}", length(&path));
    assert!((path.last().unwrap().0 - (16.0 + mirage::SWASH_REACH)).abs() < 0.01);
    assert!((path.last().unwrap().1 - 18.0).abs() < 0.001, "straight");
    assert_eq!((s.bounces, s.tripled), (0, false));
    assert_eq!(mirage::SWASH_REACH, 11.0, "5.5 tiles of 2 units");
}

#[test]
fn a_dash_into_a_wall_at_an_angle_reflects_the_axis_it_met_and_carries_on() {
    // Toward the east wall, down and east: it meets it 4 units on.
    let mut r = arena("mirageduelist", (40.0, 16.0), (1.0, 0.5));
    cast(&mut r, 1);
    let (hx, hy) = (swash(&r).hx, swash(&r).hy);
    assert!(hx > 0.0 && hy > 0.0);
    let mut turned = false;
    for _ in 0..8 {
        idle(&mut r, 1);
        if swash(&r).bounces == 1 && !turned {
            turned = true;
            let s = swash(&r);
            assert!((s.hx + hx).abs() < 1e-4, "x flipped: {} from {hx}", s.hx);
            assert!((s.hy - hy).abs() < 1e-4, "y kept: {} from {hy}", s.hy);
            assert!(s.dashing(), "a wall does not stop it");
        }
    }
    assert!(turned, "it met the wall");
    let before = hero(&r).x;
    idle(&mut r, 3);
    assert!(hero(&r).x < before, "and went on, back along the room");
    assert!(hero(&r).x < 43.58 && walkable(&r, (hero(&r).x, hero(&r).y)));
}

#[test]
fn the_first_wall_triples_the_dash_once_and_the_distance_run_counts() {
    // Straight at the east wall from 3.58 short of it, with the whole box to
    // run back across: one bounce, then the budget.
    let mut r = arena("mirageduelist", (40.0, 18.0), (1.0, 0.0));
    dash(&mut r);
    let s = swash(&r);
    assert_eq!(s.bounces, 1);
    assert!(s.tripled);
    assert!((s.budget - 33.0).abs() < 1e-4 || s.budget == 0.0, "tripled once, not again");
    assert!((s.run - mirage::SWASH_REACH * mirage::SWASH_TRIPLE).abs() < 0.001, "three times the base in all: {}", s.run);
    let h = hero(&r);
    assert!((h.x - (43.58 - (33.0 - 3.58))).abs() < 0.25 && (h.y - 18.0).abs() < 0.001, "it came back the way it went: {}", h.x);
}

#[test]
fn a_third_wall_ends_the_dash_on_open_floor_after_two_rebounds() {
    // A narrow box (y from 4 to 12), so the walls come quickly: east at 4
    // units, south at 8, north at 24.
    let mut r = arena_in("mirageduelist", (40.0, 8.0), (1.0, 0.5), (20, 4));
    dash(&mut r);
    let s = swash(&r);
    assert_eq!(s.bounces, mirage::SWASH_REBOUNDS, "two rebounds and no more");
    assert!(s.tripled && (s.run - 24.0).abs() < 0.3, "the third wall cut it short of the 33: {}", s.run);
    let h = hero(&r);
    assert!(walkable(&r, (h.x, h.y)), "standing on floor, not in the wall: {} {}", h.x, h.y);
    assert!(h.y >= 4.42 - 0.001 && h.y <= 11.58 + 0.001, "{}", h.y);
    assert!(s.slashes > 0, "the flurry begins where it ends");
}

#[test]
fn a_corner_turns_both_components() {
    // Into the north-east corner on the diagonal.
    let mut r = arena("mirageduelist", (42.5, 5.5), (1.0, -1.0));
    cast(&mut r, 1);
    let (hx, hy) = (swash(&r).hx, swash(&r).hy);
    for _ in 0..40 {
        idle(&mut r, 1);
        if swash(&r).bounces > 0 {
            break;
        }
    }
    let s = swash(&r);
    assert_eq!(s.bounces, 1);
    assert!((s.hx + hx).abs() < 1e-3 && (s.hy + hy).abs() < 1e-3, "both flipped: ({}, {}) from ({hx}, {hy})", s.hx, s.hy);
}

#[test]
fn a_dash_is_on_floor_in_the_room_every_tick_from_every_start_and_aim() {
    let mut tried = 0;
    let spots = [(5.0, 5.0), (43.0, 5.0), (5.0, 23.0), (43.0, 23.0), (24.0, 14.0), (5.0, 14.0), (43.0, 14.0), (24.0, 5.0), (24.0, 23.0)];
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0), (2.0, 1.0), (-1.0, 3.0)] {
        for (x, y) in spots {
            let mut r = arena("mirageduelist", (x, y), (dx, dy));
            cast(&mut r, 1);
            let mut ran = 0;
            while swash(&r).dashing() && ran < 200 {
                idle(&mut r, 1);
                ran += 1;
                let h = hero(&r);
                assert!(walkable(&r, (h.x, h.y)), "in stone at {} {} from ({x}, {y}) aim ({dx}, {dy}), tick {ran}", h.x, h.y);
                assert!(h.x > LEFT && h.x < 44.0 && h.y > TOP && h.y < 24.0, "left the room at {} {}", h.x, h.y);
            }
            assert!(!swash(&r).dashing(), "every dash ends");
            tried += 1;
        }
    }
    assert_eq!(tried, 90);
}

#[test]
fn a_bouncing_dash_still_ends_in_its_four_slashes_from_where_it_ends() {
    let mut r = arena_in("mirageduelist", (40.0, 8.0), (1.0, 0.5), (20, 4));
    no_thrusts(&mut r);
    cast(&mut r, 1);
    let mut slashes = 0;
    let mut dashing = true;
    let mut ended_at = None;
    for _ in 0..120 {
        idle(&mut r, 1);
        if dashing && !swash(&r).dashing() {
            dashing = false;
            ended_at = Some((hero(&r).x, hero(&r).y));
        }
        if hero(&r).swing == 6 {
            slashes += 1;
        }
    }
    assert_eq!(slashes, 4, "four slashes after the dash");
    assert_eq!(ended_at.map(|(x, y)| ((x * 10.0).round(), (y * 10.0).round())), Some(((hero(&r).x * 10.0).round(), (hero(&r).y * 10.0).round())), "from where it ended: the knight stood there throughout");
    assert!(swash(&r).idle(), "and nothing is left of the dash on the knight");
}

#[test]
fn a_fast_dash_cannot_skip_a_one_tile_wall() {
    let mut r = arena("mirageduelist", (19.0, 18.0), (1.0, 0.0));
    // A single column of wall across the way (x 28 to 30), floor either side.
    let at = r.at;
    let room = &mut r.dungeon.rooms[at];
    for row in 2..2 + BOX.1 {
        room.set(14, row, Tile::Wall);
    }
    cast(&mut r, 1);
    let mut bounced = 0;
    for _ in 0..60 {
        idle(&mut r, 1);
        assert!(hero(&r).x < 28.0, "never past the wall: {}", hero(&r).x);
        bounced = bounced.max(swash(&r).bounces);
    }
    assert!(bounced >= 1, "and it bounced off it");
    assert!(LEFT < 28.0 && TOP < 28.0);
}

#[test]
fn the_aim_steers_a_dash_at_ninety_degrees_a_second_and_no_aim_leaves_it_straight() {
    let straight = {
        let mut r = arena("mirageduelist", (14.0, 18.0), (1.0, 0.0));
        cast(&mut r, 1);
        idle(&mut r, 8);
        (hero(&r).x, hero(&r).y)
    };
    assert!((straight.1 - 18.0).abs() < 1e-3, "no aim, no curve");
    let mut r = arena("mirageduelist", (14.0, 18.0), (1.0, 0.0));
    cast(&mut r, 1);
    for _ in 0..8 {
        step(&mut r, Input { aim_y: -1, ..Default::default() });
    }
    let curved = (hero(&r).x, hero(&r).y);
    assert!(curved.1 < straight.1 - 0.8, "the held aim pulled it up: {curved:?} against {straight:?}");
    // Eight ticks of 3 degrees is 24 degrees off east at most.
    let s = swash(&r);
    let turned = s.hy.atan2(s.hx).to_degrees();
    assert!((-24.5..-23.0).contains(&turned), "turned {turned} degrees, at most 3 a tick");
    assert!(swash(&r).dashing(), "still running at 8 ticks");
}

#[test]
fn a_bounce_is_reflected_first_and_the_steering_goes_on_from_the_reflection() {
    let mut r = arena("mirageduelist", (40.0, 18.0), (1.0, 0.0));
    cast(&mut r, 1);
    let north = (-std::f32::consts::FRAC_PI_2).to_degrees();
    let off = |s: &Swash| (s.hy.atan2(s.hx).to_degrees() - north + 540.0).rem_euclid(360.0) - 180.0;
    let mut at_bounce = None;
    for _ in 0..40 {
        step(&mut r, Input { aim_y: -1, ..Default::default() });
        let s = swash(&r);
        if s.bounces == 1 && at_bounce.is_none() {
            assert!(s.hx < 0.0, "reflected to the west at once: ({}, {})", s.hx, s.hy);
            at_bounce = Some(off(&s).abs());
            // Eight more ticks of the held aim: at most 24 degrees round, and from the reflection.
            for _ in 0..8 {
                step(&mut r, Input { aim_y: -1, ..Default::default() });
            }
            let later = off(&swash(&r)).abs();
            assert_eq!(swash(&r).bounces, 1, "no second wall yet");
            assert!(later < at_bounce.unwrap() - 20.0 && later > at_bounce.unwrap() - 24.5, "steered toward the aim, 3 degrees a tick: {} then {later}", at_bounce.unwrap());
            break;
        }
    }
    assert!(at_bounce.is_some(), "it bounced");
}

#[test]
fn the_dash_is_untouchable_for_a_second_and_no_longer() {
    let mut r = arena("mirageduelist", (40.0, 18.0), (1.0, 0.0));
    cast(&mut r, 1);
    let mut shielded = 0;
    let mut dashing_late = 0;
    for t in 1..=40 {
        idle(&mut r, 1);
        if hero(&r).invulnerable > 0 {
            shielded += 1;
        } else if swash(&r).dashing() {
            dashing_late += 1;
            assert!(t > mirage::SWASH_INVULN, "vulnerable only after the cap: tick {t}");
        }
    }
    assert_eq!(shielded, mirage::SWASH_INVULN, "one second of it");
    assert!(dashing_late > 0, "and still dashing after it");
}

#[test]
fn the_dash_leaves_one_image_where_it_began_and_keeps_the_cap() {
    let mut r = arena("mirageduelist", (16.0, 18.0), (1.0, 0.0));
    cast(&mut r, 1);
    let images = mine(&r);
    assert_eq!(images.len(), 1);
    assert!(images[0].fixed && (images[0].x, images[0].y) == (16.0, 18.0));
    assert!(images[0].left <= mirage::SWASH_IMAGE);
    let mut r = arena("mirageduelist", (16.0, 18.0), (1.0, 0.0));
    with_friend(&mut r);
    for k in 0..IMPOSTURE_MAX {
        r.phantoms.push(Phantom::standing(2, (30.0 + k as f32, 10.0), 900));
    }
    cast(&mut r, 1);
    assert!(mine(&r).is_empty() && r.phantoms.len() == IMPOSTURE_MAX, "a full room is humoured no more");
    assert!(swash(&r).dashing(), "the dash goes all the same");
}

#[test]
fn the_flurry_is_four_slashes_eight_ticks_apart_each_at_the_foils_blow_times_one_point_two() {
    let mut r = arena("mirageduelist", (14.0, 18.0), (1.0, 0.0));
    // 11 units of dash, ending at x = 25; the foe stands 3 beyond it.
    let a = foe(&mut r, 28.0, 18.0);
    no_thrusts(&mut r);
    cast(&mut r, 1);
    let blow = (hero(&r).scaled(14) * 12 + 5) / 10;
    assert_eq!(blow, 17, "14 and a fifth over");
    let mut hits = Vec::new();
    let mut last = 0;
    for _ in 0..120 {
        idle(&mut r, 1);
        if lost(&r, a) > last {
            hits.push((r.tick, lost(&r, a) - last));
            last = lost(&r, a);
        }
    }
    assert_eq!(hits.len(), 4, "four slashes: {hits:?}");
    for w in hits.windows(2) {
        assert_eq!(w[1].0 - w[0].0, u64::from(mirage::SWASH_EVERY), "eight ticks apart: {hits:?}");
    }
    assert!(hits.iter().all(|h| h.1 == blow), "each for {blow}: {hits:?}");
    assert_eq!(swash(&r).slashes, 0);
    assert_eq!(mirage::SWASH_SLASHES, 4);
}

#[test]
fn each_slash_follows_the_aim_held_that_tick() {
    let mut r = arena("mirageduelist", (14.0, 18.0), (1.0, 0.0));
    // The dash ends at x = 25: one foe 3 east of it, one 3 north.
    let east = foe(&mut r, 28.0, 18.0);
    let north = foe(&mut r, 25.0, 15.0);
    no_thrusts(&mut r);
    cast(&mut r, 1);
    while swash(&r).dashing() {
        idle(&mut r, 1);
    }
    // The first slash fell on the tick the dash ended, along the way it ended: east.
    let first = lost(&r, east);
    assert!(first > 0 && lost(&r, north) == 0, "slash 1 went east");
    let aim = |ax: i8, ay: i8| Input { aim_x: ax, aim_y: ay, ..Default::default() };
    // Swing the aim north for the second slash ...
    for _ in 0..mirage::SWASH_EVERY {
        step(&mut r, aim(0, -1));
    }
    let second = lost(&r, north);
    assert!(second > 0 && lost(&r, east) == first, "slash 2 went north: {} {}", lost(&r, east), second);
    // ... back east for the third ...
    for _ in 0..mirage::SWASH_EVERY {
        step(&mut r, aim(1, 0));
    }
    assert_eq!((lost(&r, east), lost(&r, north)), (2 * first, second), "slash 3 went east");
    // ... and north again for the fourth.
    for _ in 0..mirage::SWASH_EVERY {
        step(&mut r, aim(0, -1));
    }
    assert_eq!((lost(&r, east), lost(&r, north)), (2 * first, 2 * second), "slash 4 went north");
    // No fifth.
    for _ in 0..mirage::SWASH_EVERY {
        step(&mut r, aim(1, 0));
    }
    assert_eq!((lost(&r, east), lost(&r, north)), (2 * first, 2 * second));
}

#[test]
fn a_slash_hits_every_foe_in_its_arc_not_the_nearest_three() {
    let mut r = arena("mirageduelist", (14.0, 18.0), (1.0, 0.0));
    let line: Vec<usize> = [26.0, 26.6, 27.2, 27.8, 28.4].iter().map(|&x| foe(&mut r, x, 18.0)).collect();
    no_thrusts(&mut r);
    cast(&mut r, 1);
    while swash(&r).dashing() {
        idle(&mut r, 1);
    }
    let blow = (hero(&r).scaled(14) * 12 + 5) / 10;
    for &i in &line {
        assert_eq!(lost(&r, i), blow, "foe {i} takes the whole slash");
    }
}

#[test]
fn the_images_already_out_strike_with_each_slash_at_their_share() {
    let mut r = arena("mirageduelist", (14.0, 18.0), (1.0, 0.0));
    // An image whose own east reach finds a foe the knight's slash never will.
    let far = foe(&mut r, 27.0, 10.0);
    r.phantoms.push(Phantom::standing(1, (24.0, 10.0), 900));
    no_thrusts(&mut r);
    cast(&mut r, 1);
    idle(&mut r, 60);
    let slash = (hero(&r).scaled(14) * 12 + 5) / 10;
    let share = (slash * PHANTOM_SHARE / 100).max(1);
    assert_eq!(lost(&r, far), 4 * share, "one share of the slash for each of the four");
}

#[test]
fn swashbuckle_waits_its_cooldown_and_the_voice_is_one_per_cast() {
    let mut r = arena("mirageduelist", (14.0, 18.0), (1.0, 0.0));
    r.sounds.clear();
    cast(&mut r, 1);
    assert_eq!(voices(&r), 1, "bing bong, once, at the start");
    assert_eq!(hero(&r).spell_cooldowns[0], 10 * HZ);
    idle(&mut r, 150);
    assert_eq!(voices(&r), 1, "not once a bounce, not once a slash");
    // Still cooling: another press makes no dash and no sound.
    let before = (hero(&r).x, hero(&r).y, mine(&r).len());
    r.sounds.clear();
    cast(&mut r, 1);
    assert_eq!(voices(&r), 0);
    assert!(!swash(&r).dashing());
    assert_eq!((hero(&r).x, hero(&r).y, mine(&r).len()), before);
    // Ready again, it goes.
    idle(&mut r, 10 * HZ);
    r.sounds.clear();
    cast(&mut r, 1);
    assert_eq!(voices(&r), 1);
    assert!(swash(&r).dashing());
}

#[test]
fn the_voice_goes_once_through_a_whole_bouncing_dash_and_its_flurry() {
    let mut r = arena_in("mirageduelist", (40.0, 8.0), (1.0, 0.5), (20, 4));
    r.sounds.clear();
    cast(&mut r, 1);
    let mut bounced = 0;
    for _ in 0..120 {
        idle(&mut r, 1);
        bounced = bounced.max(swash(&r).bounces);
    }
    assert_eq!(bounced, 2);
    assert_eq!(voices(&r), 1);
}

#[test]
fn a_run_saved_in_the_middle_of_a_dash_loads_and_plays_on_the_same() {
    let mut r = arena("mirageduelist", (40.0, 16.0), (1.0, 0.5));
    foe(&mut r, 20.0, 22.0);
    cast(&mut r, 1);
    idle(&mut r, 5);
    assert!(swash(&r).dashing());
    let bytes = serde_json::to_vec(&r).unwrap();
    let mut back: Run = serde_json::from_slice(&bytes).expect("a run saved mid-dash loads");
    assert_eq!(back.players[&1].champ.swash, swash(&r));
    assert_eq!(back.phantoms, r.phantoms);
    for _ in 0..80 {
        idle(&mut r, 1);
        idle(&mut back, 1);
    }
    assert_eq!(serde_json::to_string(&r.players[&1]).unwrap(), serde_json::to_string(&back.players[&1]).unwrap());
    assert_eq!(r.enemies[0].hp, back.enemies[0].hp);
}

#[test]
fn old_saves_without_the_new_fields_still_load() {
    let stance: champions::Stance = serde_json::from_value(serde_json::json!({ "brace": 3, "juxta": 5 })).unwrap();
    assert!(stance.swash.idle() && stance.echoes == 0 && stance.echo_ticks == 0);
    let written = serde_json::to_value(champions::Stance { brace: 1, ..Default::default() }).unwrap();
    assert!(written.get("swash").is_none() && written.get("echoes").is_none() && written.get("echo_ticks").is_none(), "{written}");
    let phantom: Phantom = serde_json::from_value(serde_json::json!({ "owner": 1, "x": 3.0, "y": 4.0, "dx": 1.0, "dy": 0.0, "left": 9, "fixed": true, "life": 90 })).unwrap();
    assert!(!phantom.charge && phantom.strike == 0);
    let written = serde_json::to_value(phantom).unwrap();
    assert!(written.get("charge").is_none() && written.get("strike").is_none(), "{written}");
    let r = arena("lancer", (14.0, 18.0), (1.0, 0.0));
    let value = serde_json::to_value(&r).unwrap();
    assert!(value["champions"].get("echoes").is_none(), "an idle field writes no echoes");
    let _: Run = serde_json::from_value(value).expect("a run from before the Mirage Duelist loads");
}

#[test]
fn a_run_holding_the_new_state_round_trips() {
    let mut r = arena("mirageduelist", (14.0, 18.0), (1.0, 0.0));
    foe(&mut r, 18.0, 18.0);
    cast(&mut r, 3);
    step(&mut r, Input { fire: true, ..Default::default() });
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(&mut r, Input { ult: true, ..Default::default() });
    idle(&mut r, 3);
    assert!(!r.champions.echoes.is_empty(), "an echo is waiting");
    assert!(mine(&r).iter().any(|p| p.charge), "the Hall is up");
    let back: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).expect("loads");
    assert_eq!(back.champions.echoes, r.champions.echoes);
    assert_eq!(back.phantoms, r.phantoms);
    assert_eq!(back.players[&1].champ, r.players[&1].champ);
}

// ─── Mirror Swap ─────────────────────────────────────────────────────────────

#[test]
fn mirror_swap_changes_places_with_the_nearest_own_image_only() {
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    with_friend(&mut r);
    r.phantoms.push(Phantom::standing(1, (36.0, 18.0), 900));
    r.phantoms.push(Phantom::standing(1, (24.0, 22.0), 900));
    r.phantoms.push(Phantom::standing(2, (21.0, 18.0), 900));
    cast(&mut r, 2);
    let h = hero(&r);
    assert_eq!((h.x, h.y), (24.0, 22.0), "to the nearest of their own, not the other knight's nearer one");
    let at = |x: f32, y: f32| r.phantoms.iter().any(|p| (p.x, p.y) == (x, y));
    assert!(at(20.0, 18.0), "the image takes the knight's old spot");
    assert!(at(36.0, 18.0) && at(21.0, 18.0), "no other image moved");
    assert_eq!(r.phantoms.len(), 3, "none made, none lost");
    assert!(r.phantoms.iter().all(|p| p.fixed));
}

#[test]
fn a_following_image_that_is_swapped_with_stays_where_the_knight_stood() {
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    r.phantoms.push(Phantom::beside(1, (20.0, 18.0), (0.0, 3.0), 900));
    cast(&mut r, 2);
    assert_eq!((hero(&r).x, hero(&r).y), (20.0, 21.0));
    for _ in 0..5 {
        step(&mut r, Input { move_x: 1, ..Default::default() });
    }
    let image = r.phantoms[0];
    assert_eq!((image.x, image.y), (20.0, 18.0), "left standing, not following");
}

#[test]
fn mirror_swap_shields_the_knight_for_four_tenths_of_a_second() {
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    r.phantoms.push(Phantom::standing(1, (26.0, 18.0), 900));
    cast(&mut r, 2);
    let mut shielded = u32::from(hero(&r).invulnerable > 0);
    for _ in 0..40 {
        idle(&mut r, 1);
        shielded += u32::from(hero(&r).invulnerable > 0);
    }
    assert_eq!(shielded, mirage::SWAP_SHIELD, "12 ticks, 0.4 s");
    assert_eq!(mirage::SWAP_SHIELD, HZ * 4 / 10);
    assert_eq!(hero(&r).spell_cooldowns[1], 7 * HZ - 40, "and its wait is the card's");
}

#[test]
fn with_no_image_mirror_swap_blinks_along_the_aim_and_leaves_one() {
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    cast(&mut r, 2);
    let h = hero(&r);
    assert!((h.x - (20.0 + mirage::SWAP_BLINK)).abs() < 0.01 && (h.y - 18.0).abs() < 0.001);
    let images = mine(&r);
    assert_eq!(images.len(), 1);
    assert_eq!((images[0].x, images[0].y), (20.0, 18.0));
    assert!(h.invulnerable > 0, "shielded all the same");
    // A full room: the blink still goes, the image is not made.
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    with_friend(&mut r);
    for k in 0..IMPOSTURE_MAX {
        r.phantoms.push(Phantom::standing(2, (34.0 + k as f32, 12.0), 900));
    }
    cast(&mut r, 2);
    assert!((hero(&r).x - 24.0).abs() < 0.01);
    assert_eq!(r.phantoms.len(), IMPOSTURE_MAX);
}

#[test]
fn the_blink_never_ends_in_a_wall() {
    let mut r = arena("mirageduelist", (42.0, 18.0), (1.0, 0.0));
    cast(&mut r, 2);
    let h = hero(&r);
    assert!(walkable(&r, (h.x, h.y)) && h.x <= 43.58 + 0.001, "stopped short of the wall: {}", h.x);
}

// ─── Echo Strike ─────────────────────────────────────────────────────────────

#[test]
fn echo_strike_echoes_exactly_the_next_three_thrusts_nine_ticks_later_for_half() {
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    let a = foe(&mut r, 22.5, 18.0);
    cast(&mut r, 3);
    assert_eq!(hero(&r).champ.echoes, mirage::ECHO_THRUSTS);
    let blow = hero(&r).scaled(14);
    // Four thrusts, each on its own; only three are echoed.
    let mut hits: Vec<(u64, u32)> = Vec::new();
    let mut last = 0;
    let mut thrust_ticks = Vec::new();
    for _ in 0..4 {
        step(&mut r, Input { fire: true, ..Default::default() });
        thrust_ticks.push(r.tick);
        for _ in 0..30 {
            if lost(&r, a) > last {
                hits.push((r.tick, lost(&r, a) - last));
                last = lost(&r, a);
            }
            idle(&mut r, 1);
        }
    }
    let thrust_hits: Vec<_> = hits.iter().filter(|h| h.1 == blow).collect();
    let echo_hits: Vec<_> = hits.iter().filter(|h| h.1 == blow * mirage::ECHO_SHARE / 100).collect();
    assert_eq!(thrust_hits.len(), 4, "{hits:?}");
    assert_eq!(echo_hits.len(), 3, "three echoes and no more: {hits:?}");
    for (k, e) in echo_hits.iter().enumerate() {
        assert_eq!(e.0, thrust_ticks[k] + u64::from(mirage::ECHO_DELAY), "echo {k} lands nine ticks after its thrust: {hits:?}");
    }
    assert_eq!(hero(&r).champ.echoes, 0);
    assert_eq!(mirage::ECHO_DELAY, 9, "0.3 s");
}

#[test]
fn an_echo_strikes_from_where_the_thrust_was_made_even_if_the_knight_walked_off() {
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    let a = foe(&mut r, 22.5, 18.0);
    cast(&mut r, 3);
    step(&mut r, Input { fire: true, ..Default::default() });
    let first = lost(&r, a);
    assert!(first > 0);
    for _ in 0..mirage::ECHO_DELAY + 1 {
        step(&mut r, Input { move_x: -1, ..Default::default() });
    }
    assert!(hero(&r).x < 19.0, "walked off: {}", hero(&r).x);
    assert!(lost(&r, a) > first, "the echo still struck the foe at the old place");
}

#[test]
fn an_unspent_echo_lapses_and_the_stance_is_deterministic() {
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    let a = foe(&mut r, 22.5, 18.0);
    cast(&mut r, 3);
    idle(&mut r, mirage::ECHO_TICKS + 1);
    assert_eq!(hero(&r).champ.echo_ticks, 0);
    assert_eq!(hero(&r).champ.echoes, 0);
    step(&mut r, Input { fire: true, ..Default::default() });
    idle(&mut r, 20);
    let blow = hero(&r).scaled(14);
    assert_eq!(lost(&r, a), blow, "one thrust, no echo");
    let run = || {
        let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
        foe(&mut r, 22.5, 18.0);
        cast(&mut r, 3);
        for _ in 0..90 {
            step(&mut r, Input { fire: true, ..Default::default() });
        }
        serde_json::to_string(&(r.enemies[0].hp, &r.champions, &r.players[&1].champ)).unwrap()
    };
    assert_eq!(run(), run());
}

// ─── the Hall of Mirrors ─────────────────────────────────────────────────────

fn hall(r: &mut Run) {
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(r, Input { ult: true, ..Default::default() });
}

#[test]
fn the_hall_of_mirrors_makes_three_images_that_stand_for_six_seconds() {
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    hall(&mut r);
    let images = mine(&r);
    assert_eq!(images.len(), 3);
    assert_eq!(mirage::HALL_IMAGES, 3);
    assert!(images.iter().all(|p| p.charge && p.fixed && p.left <= mirage::HALL_TICKS && walkable(&r, (p.x, p.y))));
    assert_eq!(mirage::HALL_TICKS, 6 * HZ);
    idle(&mut r, mirage::HALL_TICKS - 2);
    assert_eq!(mine(&r).len(), 3, "still there a tick before");
    idle(&mut r, 1);
    assert!(mine(&r).is_empty(), "gone at six seconds");
}

#[test]
fn the_hall_sends_each_image_at_the_nearest_foe_to_strike_at_seventy_percent() {
    let mut r = arena("mirageduelist", (14.0, 18.0), (1.0, 0.0));
    let near = foe(&mut r, 26.0, 18.0);
    let far = foe(&mut r, 30.0, 22.0);
    hall(&mut r);
    let start: Vec<f32> = mine(&r).iter().map(|p| (p.x - 26.0).hypot(p.y - 18.0)).collect();
    idle(&mut r, 12);
    let later: Vec<f32> = mine(&r).iter().map(|p| (p.x - 26.0).hypot(p.y - 18.0)).collect();
    assert!(later.iter().zip(&start).all(|(l, s)| l < s), "all three run at the near foe: {start:?} then {later:?}");
    idle(&mut r, 100);
    assert!(lost(&r, near) > 0 && lost(&r, far) == 0, "the nearest takes it");
    let blow = hero(&r).scaled(14);
    let per = (blow * mirage::HALL_SHARE / 100).max(1);
    assert_eq!(lost(&r, near) % per, 0, "every strike is the share: {} in {per}s", lost(&r, near));
    assert!(lost(&r, near) / per >= 6, "they struck again and again: {}", lost(&r, near) / per);
    assert_eq!(mirage::HALL_SHARE, 70);
}

#[test]
fn the_knights_thrusts_during_the_hall_make_no_juxtapose_images_and_the_cap_holds() {
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    hall(&mut r);
    let grid = Grid { room: &r.dungeon.rooms[r.at], barred: false };
    let mut hero = r.players[&1].clone();
    hero.champ.juxta = 100;
    let mut phantoms = r.phantoms.clone();
    let before = phantoms.len();
    for salt in 0..500 {
        champions::juxtapose(&mut phantoms, &hero, 1, salt, &grid);
    }
    assert_eq!(phantoms.len(), before, "nothing extra while the Hall stands");
    assert!(phantoms.len() <= IMPOSTURE_MAX);
    // Without the Hall the same stance makes them.
    let mut none: Vec<Phantom> = Vec::new();
    for salt in 0..500 {
        champions::juxtapose(&mut none, &hero, 1, salt, &grid);
    }
    assert!(!none.is_empty());
    // And the Hall's own images take no part in the mirroring of a thrust.
    let struck = ults::phantom_strikes(&r.phantoms, 1, &mut r.enemies, &grid, (1.0, 0.0), (3.2, 30.0), 14);
    assert!(struck.is_empty());
}

// ─── the dodge ───────────────────────────────────────────────────────────────

#[test]
fn the_dodge_roll_leaves_a_decoy_for_a_second_and_a_half_if_a_place_is_free() {
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    step(&mut r, Input { dash: true, move_x: 1, ..Default::default() });
    let images = mine(&r);
    assert_eq!(images.len(), 1);
    assert_eq!((images[0].x, images[0].y), (20.0, 18.0), "where the roll began");
    assert!(images[0].fixed && images[0].left <= mirage::DECOY_TICKS);
    assert_eq!(mirage::DECOY_TICKS, 45);
    assert!(hero(&r).x > 20.0, "and the roll went on");
    idle(&mut r, mirage::DECOY_TICKS);
    assert!(mine(&r).is_empty(), "gone after 1.5 s");
    // No free place: no decoy.
    let mut r = arena("mirageduelist", (20.0, 18.0), (1.0, 0.0));
    with_friend(&mut r);
    for k in 0..IMPOSTURE_MAX {
        r.phantoms.push(Phantom::standing(2, (34.0 + k as f32, 12.0), 900));
    }
    step(&mut r, Input { dash: true, move_x: 1, ..Default::default() });
    assert_eq!(r.phantoms.len(), IMPOSTURE_MAX);
    assert!(mine(&r).is_empty());
    // Another champion's roll leaves nothing.
    let mut r = arena("lancer", (20.0, 18.0), (1.0, 0.0));
    step(&mut r, Input { dash: true, move_x: 1, ..Default::default() });
    assert!(mine(&r).is_empty());
}

// ─── the voice ───────────────────────────────────────────────────────────────

#[test]
fn the_voice_is_a_shipped_effect_with_a_gap_longer_than_itself() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/assets/dungeon/sfx/mirage_bing_bong.mp3");
    assert!(file.is_file(), "{file:?}");
    assert!(std::fs::metadata(&file).unwrap().len() > 1_000);
    assert!(crate::client::sound_gap("mirage_bing_bong") >= 1000, "never stacks");
}
