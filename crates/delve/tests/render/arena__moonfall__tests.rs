//! Moonfall through the real arena: the town at its hours, the Ledger, the
//! bell, a conversation, the rewind and the end, one frame each.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_moonfall_review
//! -- --ignored` writes the pictures to open (native, 3x and the terminal's
//! cell size).
use super::super::frame_for;
use super::*;
use crate::rules::moonfall::{
    clock, folk,
    folk::Folk,
    ledger::{self, Fact},
    says::Topic,
    state::{Phase, Say, Wisp, WispMode},
};
use crate::rules::{MAX_PLAYERS, Run};
use std::collections::BTreeMap;

fn table(seats: u32) -> Run {
    let mut r = Run::moonfall_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

fn at_hour(r: &mut Run, day: u32, hh: u32, mm: u32) {
    let s = r.moonfall.as_mut().unwrap();
    // Hours after midnight belong to the night of the same day.
    let h2 = ((day - 1) * 24 + if hh < 6 { hh + 18 } else { hh - 6 }) * 2 + mm / 30;
    s.clock = clock::ticks(s.nums.day, h2);
}

fn put_knight(r: &mut Run, id: u32, tile: (i32, i32)) {
    let s = r.moonfall.as_mut().unwrap();
    let k = &mut s.seats.get_mut(&id).unwrap().knight;
    k.x = (tile.0 * 16 + 8) * 256;
    k.y = (tile.1 * 16 + 8) * 256;
}

fn base_and_fx(r: &Run, focus: u32) -> Img {
    let (mut base, fx) = layers(r, Some(focus));
    base.stamp(&fx, 0, 0);
    base
}

#[test]
fn the_town_draws_at_every_hour_and_changes_with_the_sky() {
    let mut r = table(2);
    let mut seen = std::collections::BTreeSet::new();
    for (day, hh) in [(1, 6), (1, 12), (1, 18), (1, 22), (2, 3), (3, 1)] {
        at_hour(&mut r, day, hh, 0);
        let (base, fx) = layers(&r, Some(1));
        assert_eq!((base.w, base.h, fx.w, fx.h), (384, 224, 384, 224));
        assert!(seen.insert(base.content_hash()), "day {day} {hh}:00 looks like another hour");
    }
    let (a, _) = layers(&r, Some(1));
    let (b, _) = layers(&r, Some(2));
    assert_ne!(a.content_hash(), b.content_hash(), "each seat sees its own camera");
}

#[test]
fn the_moon_hangs_lower_and_bigger_each_night() {
    use super::super::super::ink::ink;
    use super::sky::disc;
    let mut r = table(1);
    let pale = ['V', 'W', 'v', 'U'].map(|c| ink(c).unwrap());
    let (mut sizes, mut radii, mut lows) = (Vec::new(), Vec::new(), Vec::new());
    for day in 1..=3 {
        at_hour(&mut r, day, 23, 0);
        let (base, _) = layers(&r, Some(1));
        sizes.push(base.pixels().filter(|p| pale.contains(p)).count());
        let s = r.moonfall.as_ref().unwrap();
        let (_, _, cy, radius, _) = disc(s, s.shown_clock());
        (radii.push(radius), lows.push(cy));
    }
    // Whatever draws it: the disc is bigger and lower each night.
    assert!(radii[0] < radii[1] && radii[1] < radii[2] && lows[0] < lows[1] && lows[1] < lows[2], "{radii:?} {lows:?}");
    // The block's disc is pale: its pixels count the same growth.
    if bank::frame("moon", "far", 0).is_none() {
        assert!(sizes[0] < sizes[1] && sizes[1] < sizes[2], "{sizes:?}");
    }
}

#[test]
fn every_state_of_the_ritual_and_the_panels_draws() {
    let mut r = table(3);
    let day = r.moonfall.as_ref().unwrap().nums.day;
    r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t: 10, from: clock::ticks(day, 80), fell: false };
    let _ = layers(&r, Some(1));
    r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t: 20, from: clock::end(day), fell: true };
    let _ = layers(&r, Some(1));
    r.moonfall.as_mut().unwrap().phase = Phase::Skip { t: 12, from: 100, to: day / 2 };
    let _ = layers(&r, Some(1));
    r.moonfall.as_mut().unwrap().phase = Phase::Won { t: 50 };
    let _ = layers(&r, Some(1));
    r.moonfall.as_mut().unwrap().phase = Phase::Play;
    for page in 0..7 {
        r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.panel = Panel::Ledger(page);
        let _ = layers(&r, Some(1));
    }
    r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.panel = Panel::Bell(1);
    let _ = layers(&r, Some(1));
}

/// The Moonwell's pixels the stage draws: its middle on the screen, and the
/// cold bowl's mouth.
fn well_bowl(r: &Run) -> (i32, i32) {
    let cam = camera(r.moonfall.as_ref().unwrap().seats[&1].knight.px());
    (town::WELL.0 - cam.x - 11, town::WELL.1 - cam.y + VIEW_Y + 16 - 29)
}

#[test]
fn the_painted_well_follows_the_rules_states_and_keeps_its_ready_glow() {
    use super::super::super::ink::ink;
    if bank::frame("moonwell", "lit", 0).is_none() {
        return;
    }
    let mut r = table(1);
    put_knight(&mut r, 1, (18, 15));
    at_hour(&mut r, 1, 12, 0);
    let still = |r: &Run| layers(r, Some(1)).0.content_hash();
    let idle = still(&r);
    r.moonfall.as_mut().unwrap().memory.keeps |= ledger::OIL;
    let oiled = still(&r);
    assert_ne!(idle, oiled, "oil in the bowl is seen");
    {
        let s = r.moonfall.as_mut().unwrap();
        s.world.lit_until[2] = s.clock + 4000;
    }
    let lit = still(&r);
    assert_ne!(oiled, lit, "the lit well burns");
    // Lit, it plays four frames, six ticks each.
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..24 {
        seen.insert(still(&r));
        r.step(&BTreeMap::new());
    }
    assert!(seen.len() >= 4, "{} lit pictures", seen.len());
    // Oiled in the Final Hours with the Verse known, the cold bowl blinks the
    // signal ink, as the block's does: on at one tick, off ten ticks later.
    {
        let s = r.moonfall.as_mut().unwrap();
        s.world.lit_until[2] = 0;
        s.memory.learn(Fact::Verse);
    }
    at_hour(&mut r, 3, 4, 30);
    // From the north the well is low on the screen, clear of the Final Hours banner.
    put_knight(&mut r, 1, (17, 10));
    assert!(r.moonfall.as_ref().unwrap().tower_ready(2));
    let (bx, by) = well_bowl(&r);
    let glow = ink('4').unwrap();
    let mut on = Vec::new();
    for _ in 0..40 {
        on.push(layers(&r, Some(1)).0.get(bx, by) == Some(glow));
        r.step(&BTreeMap::new());
    }
    assert!(on.iter().any(|&x| x) && on.iter().any(|&x| !x), "it blinks: {on:?}");
}

/// The painted pictures of a building, when its day, night and lit ones are all there.
fn painted_pictures(id: &str) -> Option<[&'static Img; 3]> {
    Some([bank::frame(id, "day", 0)?, bank::frame(id, "night", 0)?, bank::frame(id, "lit", 0)?])
}

/// Where a painted building's top-left corner is on a seat's screen, from the
/// rules' own tile map: its bottom on the footprint's south edge, its door over
/// the doorstep tile under it when there is one, else centred.
fn painted_corner(r: &Run, focus: u32, hs: &town::House, pic: &Img) -> (i32, i32) {
    let cam = camera(r.moonfall.as_ref().unwrap().seats[&focus].knight.px());
    let door = (hs.x..hs.x + hs.w).find(|&c| town::tile(c, hs.y + hs.h) == town::Tile::Door);
    let middle = door.map_or(hs.x * 16 + hs.w * 8, |c| c * 16 + 8);
    (middle - pic.w / 2 - cam.x, (hs.y + hs.h) * 16 - pic.h - cam.y + VIEW_Y)
}

#[test]
fn the_painted_buildings_stand_on_their_footprints_and_follow_the_hour() {
    // By day the picture is stamped untinted: its upper part is on the screen
    // exactly where the map's south edge and doorstep put it (a villager at the
    // door may stand over its foot).
    // (letter, slot, a knight at the door, the knight whose seat frames it)
    for (ch, id, door, seat) in [
        ('A', "archive", (9, 15), (3, 13)),
        ('B', "bakery", (24, 15), (31, 13)),
        ('F', "forge", (28, 15), (31, 13)),
        ('I', "inn", (28, 17), (28, 17)),
        ('H', "herald-house", (32, 15), (31, 13)),
        ('S', "scriptorium", (5, 6), (6, 6)),
        ('C', "cottage", (2, 14), (4, 13)),
        ('U', "bridge-hut", (24, 7), (21, 7)),
        ('N', "tower-north", (18, 5), (19, 4)),
    ] {
        let Some([day, ..]) = painted_pictures(id) else { continue };
        let mut r = table(2);
        put_knight(&mut r, 1, door);
        put_knight(&mut r, 2, seat);
        at_hour(&mut r, 1, 12, 0);
        let hs = town::HOUSES.iter().find(|h| h.ch == ch).unwrap();
        let (x, y) = painted_corner(&r, 2, hs, day);
        let screen = layers(&r, Some(2)).0;
        let (mut same, mut total) = (0, 0);
        for py in 0..day.h - 30 {
            for px in 0..day.w {
                if let Some(c) = day.get(px, py) {
                    total += 1;
                    same += i32::from(screen.get(x + px, y + py) == Some(c));
                }
            }
        }
        assert!(total > 100 && same * 100 >= total * 97, "{id}: {same} of {total} pixels where the map puts it");
        // The picture stands on the south edge: its last row is painted.
        assert!((0..day.w).any(|px| day.get(px, day.h - 1).is_some()), "{id} stands on its footprint");
    }
    // The inn's windows follow the clock: the day's picture, the night's, then
    // the lit one when the evening is on.
    let Some(_) = painted_pictures("inn") else { return };
    let mut r = table(1);
    put_knight(&mut r, 1, (28, 17));
    let hs = town::HOUSES.iter().find(|h| h.ch == 'I').unwrap();
    let mut looks = Vec::new();
    for (hh, mm) in [(12, 0), (2, 0), (20, 30)] {
        at_hour(&mut r, 1, hh, mm);
        let cam = camera(r.moonfall.as_ref().unwrap().seats[&1].knight.px());
        let screen = layers(&r, Some(1)).0;
        let mut hash = 0u64;
        for py in (hs.y * 16 - cam.y + VIEW_Y)..((hs.y + hs.h) * 16 - cam.y + VIEW_Y) {
            for px in 0..384 {
                hash = hash.wrapping_mul(0x100_0000_01b3) ^ screen.get(px, py).map_or(0, |c| u64::from(c[0]) << 16 | u64::from(c[1]) << 8 | u64::from(c[2]));
            }
        }
        looks.push(hash);
    }
    assert!(looks[0] != looks[1] && looks[1] != looks[2] && looks[0] != looks[2], "day, night and lit differ: {looks:?}");
}

/// Whether the bell's picture `pic` is on the screen where the post's foot puts
/// it (a few pixels may be covered by a villager or the label of the ritual).
fn bell_shows(r: &Run, pic: &Img) -> bool {
    let cam = camera(r.moonfall.as_ref().unwrap().seats[&1].knight.px());
    let (bx, by) = (town::BELL_POST.0 - cam.x, town::BELL_POST.1 - cam.y + VIEW_Y);
    let screen = layers(r, Some(1)).0;
    let (mut same, mut total) = (0, 0);
    for py in 0..pic.h {
        for px in 0..pic.w {
            if let Some(c) = pic.get(px, py) {
                total += 1;
                same += i32::from(screen.get(bx - pic.w / 2 + px, by + 2 - pic.h + py) == Some(c));
            }
        }
    }
    same * 100 >= total * 90
}

#[test]
fn the_painted_bell_hangs_still_and_rings_with_the_ritual() {
    let (Some(idle), Some(_)) = (bank::frame("bell", "idle", 0), bank::frame("bell", "ring", 0)) else {
        return;
    };
    let mut r = table(1);
    put_knight(&mut r, 1, (14, 15));
    at_hour(&mut r, 1, 12, 0);
    assert!(bell_shows(&r, idle), "the bell hangs on its post");
    for _ in 0..3 {
        r.tick += 7;
        assert!(bell_shows(&r, idle), "it hangs still until the town is wound back");
    }
    // The ritual: six frames, four ticks each, from the first toll.
    let from = r.moonfall.as_ref().unwrap().clock;
    let mut rung = std::collections::BTreeSet::new();
    for t in [0u32, 4, 8, 12, 16, 20] {
        r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t, from, fell: false };
        let pic = bank::frame("bell", "ring", t).unwrap();
        assert!(bell_shows(&r, pic), "ring frame at tick {t}");
        rung.insert(pic.content_hash());
    }
    assert_eq!(rung.len(), 6, "six different rung frames");
    assert!(!rung.contains(&idle.content_hash()), "ringing is not hanging");
}

/// The pictures the tower's flame and gem are laid over.
fn north_tower_view(r: &mut Run) -> (Img, (i32, i32)) {
    put_knight(r, 1, (18, 5));
    put_knight(r, 2, (19, 4));
    let hs = town::HOUSES.iter().find(|h| h.ch == 'N').unwrap();
    let day = bank::frame("tower-north", "day", 0).expect("painted tower");
    let (x, y) = painted_corner(r, 2, hs, day);
    (layers(r, Some(2)).0, (x + day.w / 2, y + day.h))
}

#[test]
fn the_painted_north_tower_keeps_its_flame_and_its_lock_and_follows_its_door() {
    use super::super::super::ink::ink;
    if painted_pictures("tower-north").is_none() {
        return;
    }
    let mut r = table(2);
    at_hour(&mut r, 1, 12, 0);
    let (cold, foot) = north_tower_view(&mut r);
    // The lock's gem is gold while the door is locked, aqua once it is open,
    // and the picture shows its door ajar when it is.
    let gem = |img: &Img| img.get(foot.0 + 3, foot.1 - 17);
    assert_eq!(gem(&cold), Some(ink('6').unwrap()), "a locked door has a gold gem");
    r.moonfall.as_mut().unwrap().world.flags |= rules::state::NORTH_OPEN;
    let (open, _) = north_tower_view(&mut r);
    assert_eq!(gem(&open), Some(ink('2').unwrap()), "an open door has an aqua gem");
    assert_ne!(cold.content_hash(), open.content_hash(), "the open door shows in the picture");
    // The flame dances in the basket when the tower is lit, and not before.
    {
        let s = r.moonfall.as_mut().unwrap();
        s.world.flags &= !rules::state::NORTH_OPEN;
        s.world.lit_until[0] = s.clock + 4000;
    }
    let (lit, _) = north_tower_view(&mut r);
    let flames = [ink('@'), ink('5'), ink('w')];
    let burning = |img: &Img| {
        (-6..6)
            .flat_map(|dx| (-14..4).map(move |dy| (dx, dy)))
            .filter(|&(dx, dy)| flames.contains(&img.get(foot.0 + 1 + dx, foot.1 - 89 + dy)))
            .count()
    };
    assert!(burning(&lit) > 20, "a flame over the basket: {}", burning(&lit));
    assert!(burning(&cold) < 4, "none while the tower is cold: {}", burning(&cold));
}

#[test]
fn what_stands_up_over_the_sky_band_takes_the_light_of_the_night_too() {
    use super::super::super::ink::is_signal;
    let Some([_, night, _]) = painted_pictures("tower-north") else {
        return;
    };
    let mut r = table(2);
    at_hour(&mut r, 1, 22, 0);
    let (screen, foot) = north_tower_view(&mut r);
    // The tower's top stands over the sky band (screen rows 0 to 35): every
    // lit-by-day colour in it is darkened, as the rows below the band are.
    let (left, top) = (foot.0 - night.w / 2, foot.1 - night.h);
    let (mut over, mut changed) = (0, 0);
    for py in 0..night.h {
        let y = top + py;
        if !(0..VIEW_Y).contains(&y) {
            continue;
        }
        for px in 0..night.w {
            if let Some(c) = night.get(px, py).filter(|&c| !is_signal(c)) {
                over += 1;
                changed += i32::from(screen.get(left + px, y) != Some(c));
            }
        }
    }
    assert!(over > 300 && changed * 10 >= over * 9, "{changed} of {over} pixels over the sky band took the night");
}

/// The screen's pixels over the herald's podium, from a seat at the plaza.
fn podium_view(r: &Run) -> (Img, (i32, i32)) {
    let cam = camera(r.moonfall.as_ref().unwrap().seats[&1].knight.px());
    let (px, py) = town::Spot::Podium.px();
    (layers(r, Some(1)).0, (px - cam.x, py + 8 - cam.y + VIEW_Y))
}

#[test]
fn the_painted_podium_flaps_its_pennant_while_the_herald_cries_and_hangs_it_limp_otherwise() {
    let (Some(idle), Some(_)) = (bank::frame("podium", "idle", 0), bank::frame("podium", "cry", 0)) else {
        return;
    };
    let mut r = table(1);
    // An hour when the herald is not standing on it.
    let hour = (8..17)
        .find(|&hh| {
            at_hour(&mut r, 1, hh, 0);
            let s = r.moonfall.as_ref().unwrap();
            let who = folk::figure(Folk::Pella, s.clock, s.nums.day, s.world.shod);
            let (px, py) = town::Spot::Podium.px();
            !who.visible || (who.x - px).abs() + (who.y - py).abs() > 60
        })
        .expect("an hour of the day without the herald at the podium");
    at_hour(&mut r, 1, hour, 0);
    put_knight(&mut r, 1, (18, 16));
    // Standing: the limp picture whatever the tick, with its foot on the block's.
    let (screen, (px, py)) = podium_view(&r);
    let (x, y) = (px - idle.w / 2, py - idle.h);
    let same = |pic: &Img, screen: &Img| {
        let (mut same, mut total) = (0, 0);
        for qy in 0..pic.h - 4 {
            for qx in 0..pic.w {
                if let Some(c) = pic.get(qx, qy) {
                    total += 1;
                    same += i32::from(screen.get(x + qx, y + qy) == Some(c));
                }
            }
        }
        (same, total)
    };
    let (same_idle, total) = same(idle, &screen);
    assert!(total > 100 && same_idle * 100 >= total * 95, "idle: {same_idle} of {total}");
    // Crying: four beats of six ticks, each a different picture, none the idle one.
    r.moonfall.as_mut().unwrap().cry = Some((Folk::Pella, Topic::Cry(1), 60));
    let mut seen = std::collections::BTreeSet::new();
    for n in 0..4u32 {
        r.tick = u64::from(6 * n);
        let pic = bank::frame("podium", "cry", 6 * n).unwrap();
        let (screen, _) = podium_view(&r);
        let (same_cry, total) = same(pic, &screen);
        assert!(same_cry * 100 >= total * 95, "cry beat {n}: {same_cry} of {total}");
        assert!(seen.insert(pic.content_hash()) && pic.content_hash() != idle.content_hash());
    }
}

/// A fingerprint of the sixteen by thirty-four pixels over lantern `n`, from a
/// seat at the plaza's west (so no knight stands over it).
fn lamp_look(r: &Run, n: usize) -> u64 {
    let cam = camera(r.moonfall.as_ref().unwrap().seats[&1].knight.px());
    let (tx, ty) = town::LAMPS[n];
    let (x0, y0) = (tx * 16 - cam.x, ty * 16 + 14 - cam.y + VIEW_Y - 34);
    let screen = layers(r, Some(1)).0;
    let mut hash = 0u64;
    for y in y0..y0 + 34 {
        for x in x0..x0 + 16 {
            hash = hash.wrapping_mul(0x100_0000_01b3) ^ screen.get(x, y).map_or(0, |c| u64::from(c[0]) << 16 | u64::from(c[1]) << 8 | u64::from(c[2]));
        }
    }
    hash
}

#[test]
fn the_painted_lanterns_are_cold_by_day_flicker_through_three_flames_and_go_out_in_three_drawings() {
    if ["unlit", "lit", "snuffing"].iter().any(|state| bank::frame("lantern", state, 0).is_none()) {
        return;
    }
    let mut r = table(1);
    put_knight(&mut r, 1, (13, 15));
    at_hour(&mut r, 1, 12, 0);
    let cold = lamp_look(&r, 3);
    at_hour(&mut r, 1, 22, 0);
    let mut lit = std::collections::BTreeSet::new();
    for tick in [0u64, 7, 14] {
        r.tick = tick;
        lit.insert(lamp_look(&r, 3));
    }
    assert_eq!(lit.len(), 3, "three flames in three beats");
    assert!(!lit.contains(&cold), "a lit lantern is not the cold one");
    // A wisp leaning into the lamp: the drawings of its last eighteen ticks,
    // an early moment of the lean is still the flame.
    r.moonfall.as_mut().unwrap().world.wisps =
        vec![Wisp { x: 20 * 16 * 256, y: 20 * 16 * 256, mode: WispMode::Snuff(40), aim: 3, born: 0 }];
    r.tick = 0;
    assert!(lit.contains(&lamp_look(&r, 3)), "an early lean leaves the flame as it was");
    let mut out = std::collections::BTreeSet::new();
    for left in [16u32, 10, 4] {
        r.moonfall.as_mut().unwrap().world.wisps[0].mode = WispMode::Snuff(left);
        out.insert(lamp_look(&r, 3));
    }
    assert_eq!(out.len(), 3, "three drawings going out");
    assert!(out.is_disjoint(&lit) && !out.contains(&cold));
    // Put out, it is dark again, and the other lamps burn on.
    r.moonfall.as_mut().unwrap().world.wisps.clear();
    r.moonfall.as_mut().unwrap().world.snuffed = 1 << 3;
    assert!(!lit.contains(&lamp_look(&r, 3)) && !out.contains(&lamp_look(&r, 3)));
}

#[test]
fn the_painted_bridge_lies_flat_by_day_stands_upright_by_night_and_lifts_in_three_beats_after_the_hour() {
    use super::ground::bridge_leaf;
    if ["down", "rising", "up"].iter().any(|state| bank::frame("bridge", state, 0).is_none()) {
        return;
    }
    let mut r = table(1);
    let leaf = |r: &Run| {
        let s = r.moonfall.as_ref().unwrap();
        bridge_leaf(s, 0, s.shown_clock()).unwrap().content_hash()
    };
    let pic = |state: &str, t: u32| bank::frame("bridge", state, t).unwrap().content_hash();
    // It lies flat at the first dawn and through the day, upright through the night.
    r.moonfall.as_mut().unwrap().clock = 0;
    assert_eq!(leaf(&r), pic("down", 0), "the first dawn: flat, not lowering");
    at_hour(&mut r, 1, 12, 0);
    assert_eq!(leaf(&r), pic("down", 0));
    at_hour(&mut r, 1, 2, 0);
    assert_eq!(leaf(&r), pic("up", 0));
    // 20:00: the rules raise it; the leaf takes its three lifts, six ticks each, then stands.
    // The first tick the rules call it raised, found from just before.
    let flip = |r: &Run, up: bool| {
        let s = r.moonfall.as_ref().unwrap();
        (s.clock.saturating_sub(5)..).find(|&c| clock::bridge_up(c, s.nums.day) == up).unwrap()
    };
    at_hour(&mut r, 1, 19, 55);
    let dusk = flip(&r, true);
    for (after, state, t) in [(0, "rising", 0), (5, "rising", 5), (6, "rising", 6), (13, "rising", 13), (17, "rising", 17), (18, "up", 0)] {
        r.moonfall.as_mut().unwrap().clock = dusk + after;
        assert_eq!(leaf(&r), pic(state, t), "{after} ticks after eight in the evening");
    }
    let lifts: std::collections::BTreeSet<u64> = (0..3).map(|n| pic("rising", 6 * n)).collect();
    assert_eq!(lifts.len(), 3, "three different lifts");
    // 06:00: it comes down through the same three, the other way round.
    at_hour(&mut r, 2, 5, 55);
    let dawn = flip(&r, false);
    for (after, state, t) in [(0, "rising", 17), (6, "rising", 11), (12, "rising", 5), (18, "down", 0)] {
        r.moonfall.as_mut().unwrap().clock = dawn + after;
        assert_eq!(leaf(&r), pic(state, t), "{after} ticks after six in the morning");
    }
}

#[test]
fn the_painted_bridge_stands_on_the_south_edge_of_its_tiles_over_water() {
    if ["down", "rising", "up"].iter().any(|state| bank::frame("bridge", state, 0).is_none()) {
        return;
    }
    let mut r = table(1);
    // An hour of the bright day when nobody is crossing.
    let day = r.moonfall.as_ref().unwrap().nums.day;
    let hour = (8..17)
        .find(|&hh| {
            at_hour(&mut r, 1, hh, 0);
            let s = r.moonfall.as_ref().unwrap();
            folk::CAST.iter().all(|&f| {
                let who = folk::figure(f, s.clock, day, s.world.shod);
                !who.visible || (who.x - 296).abs() + (who.y - 144).abs() > 70
            })
        })
        .expect("an hour of the day with nobody on the bridge");
    at_hour(&mut r, 1, hour, 0);
    put_knight(&mut r, 1, (18, 7));
    let flat = bank::frame("bridge", "down", 0).unwrap();
    let cam = camera(r.moonfall.as_ref().unwrap().seats[&1].knight.px());
    // Bottom middle at the middle of its three columns, on the south edge of its two rows.
    let (x, y) = (296 - flat.w / 2 - cam.x, 160 - flat.h - cam.y + VIEW_Y);
    let screen = layers(&r, Some(1)).0;
    let (mut same, mut total) = (0, 0);
    for py in 0..flat.h {
        for px in 0..flat.w {
            if let Some(c) = flat.get(px, py) {
                total += 1;
                same += i32::from(screen.get(x + px, y + py) == Some(c));
            }
        }
    }
    assert!(total > 800 && same * 100 >= total * 97, "{same} of {total} pixels where the tiles put it");
    // Where the leaf does not reach, the canal: none of the old wooden tiles.
    let water = [ink_of('q'), ink_of('Q')];
    let (mut bare, mut wet) = (0, 0);
    for wy in 128..160 {
        for wx in 272..320 {
            let (px, py) = (wx - cam.x - x, wy - cam.y + VIEW_Y - y);
            if flat.get(px, py).is_none() {
                bare += 1;
                wet += i32::from(screen.get(wx - cam.x, wy - cam.y + VIEW_Y).is_some_and(|c| water.contains(&c)));
            }
        }
    }
    assert!(bare > 100 && wet * 100 >= bare * 97, "{wet} of {bare} uncovered pixels are the canal's water");
}

/// A sprite ink as a colour.
fn ink_of(ch: char) -> super::super::super::ink::Rgb {
    super::super::super::ink::ink(ch).unwrap()
}

#[test]
fn the_painted_moon_wears_a_face_for_each_day_its_fall_and_its_turning() {
    use super::sky::painted_face;
    let states = ["far", "near", "close", "falling", "turned"];
    if states.iter().any(|state| bank::frame("moon", state, 0).is_none()) {
        return;
    }
    let pic = |state: &str, t: u32| bank::frame("moon", state, t).unwrap().content_hash();
    let mut r = table(1);
    let day = r.moonfall.as_ref().unwrap().nums.day;
    let face = |r: &Run, tick: u32| {
        let s = r.moonfall.as_ref().unwrap();
        painted_face(s, s.shown_clock(), tick).unwrap().content_hash()
    };
    // The sulk the first day, the grimace the second, the snarl the third.
    for (d, hh, state) in [(1, 6, "far"), (1, 22, "far"), (2, 12, "near"), (2, 22, "near"), (3, 3, "close"), (3, 23, "close")] {
        at_hour(&mut r, d, hh, 0);
        assert_eq!(face(&r, 0), pic(state, 0), "day {d} {hh}:00");
    }
    // The fall: the roar and the fangs, six ticks each, while the moon comes down.
    at_hour(&mut r, 3, 5, 0);
    let ctl = crate::rules::moonfall::control(&r.moonfall.as_ref().unwrap().nums).max(1);
    r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t: 4, from: clock::end(day), fell: true };
    assert_eq!(face(&r, 0), pic("falling", 0));
    assert_eq!(face(&r, 6), pic("falling", 6));
    assert_ne!(pic("falling", 0), pic("falling", 6));
    // After the moon has landed the town winds back: the face of the day it
    // is wound back to, then the first dawn's.
    r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t: ctl / 2, from: clock::end(day), fell: true };
    assert_eq!(face(&r, 0), pic("near", 0));
    r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t: ctl, from: clock::end(day), fell: true };
    assert_eq!(face(&r, 0), pic("far", 0));
    // The lights hold: it turns, and stays turned.
    r.moonfall.as_mut().unwrap().phase = Phase::Won { t: 10 };
    assert_eq!(face(&r, 0), pic("turned", 0));
    {
        let s = r.moonfall.as_mut().unwrap();
        s.phase = Phase::Play;
        s.completed = Some(1);
    }
    assert_eq!(face(&r, 0), pic("turned", 0));
}

#[test]
fn the_painted_moon_is_the_discs_place_and_size_and_nothing_else_in_the_sky() {
    let Some(far) = bank::frame("moon", "far", 0) else {
        return;
    };
    if ["near", "close", "falling", "turned"].iter().any(|state| bank::frame("moon", state, 0).is_none()) {
        return;
    }
    // The first dawn: the moon is twenty-four pixels across at (312, 16), and
    // every pixel of it is the far face's, a plain pixel repeat or drop of it.
    let mut r = table(1);
    at_hour(&mut r, 1, 6, 0);
    put_knight(&mut r, 1, (20, 16));
    let (base, _) = layers(&r, Some(1));
    let (mut painted, mut same) = (0, 0);
    for y in 4..28 {
        for x in 300..324 {
            let (u, v) = ((2 * (x - 300) + 1) * far.w / 48, (2 * (y - 4) + 1) * far.h / 48);
            if let Some(c) = far.get(u, v) {
                painted += 1;
                same += i32::from(base.get(x, y) == Some(c));
            }
        }
    }
    assert!((400..=480).contains(&painted), "a disc 24 px across: {painted} pixels");
    assert_eq!(same, painted, "every pixel of the disc is the face's");
    // At the fall it fills the screen, drawn from the same pictures.
    let day = r.moonfall.as_ref().unwrap().nums.day;
    r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t: 14, from: clock::end(day), fell: true };
    let (wipe, _) = layers(&r, Some(1));
    let roar: std::collections::BTreeSet<_> = (0..2)
        .flat_map(|n| {
            let pic = bank::frame("moon", "falling", 6 * n).unwrap();
            (0..pic.h).flat_map(move |y| (0..pic.w).filter_map(move |x| pic.get(x, y)))
        })
        .collect();
    let inside = (60..324).flat_map(|x| (40..190).map(move |y| (x, y))).filter(|&(x, y)| wipe.get(x, y).is_some_and(|c| roar.contains(&c))).count();
    assert!(inside > 20_000, "the roaring faces fill the town: {inside}");
}

#[test]
#[ignore = "writes Moonfall's review pictures"]
fn write_moonfall_review() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let shot = |name: &str, r: &Run, focus: u32| {
        let both = base_and_fx(r, focus);
        save(&format!("{name}-1x"), &both, 1);
        save(&format!("{name}-3x"), &both, 3);
    };
    // Dawn, with the title card.
    let mut r = table(1);
    r.moonfall.as_mut().unwrap().card = 30;
    shot("moonfall-01-dawn-card", &r, 1);
    r.moonfall.as_mut().unwrap().card = 0;
    // Morning at the plaza, folk about; the knight by the Archive.
    at_hour(&mut r, 1, 9, 0);
    put_knight(&mut r, 1, (12, 16));
    shot("moonfall-02-morning-archive", &r, 1);
    // Noon in the plaza.
    at_hour(&mut r, 1, 12, 15);
    put_knight(&mut r, 1, (20, 16));
    shot("moonfall-03-noon-plaza", &r, 1);
    // North bank by day: scriptorium, reed bank, bridge.
    at_hour(&mut r, 1, 10, 0);
    put_knight(&mut r, 1, (14, 7));
    shot("moonfall-04-north-bank", &r, 1);
    // Dusk: lamps being lit.
    at_hour(&mut r, 1, 18, 30);
    put_knight(&mut r, 1, (18, 16));
    shot("moonfall-05-dusk-lamps", &r, 1);
    // Night with wisps.
    at_hour(&mut r, 1, 22, 0);
    {
        let s = r.moonfall.as_mut().unwrap();
        s.world.wisps = vec![
            Wisp { x: 21 * 16 * 256, y: 18 * 16 * 256, mode: WispMode::Hunt, aim: 3, born: 0 },
            Wisp { x: 13 * 16 * 256, y: 17 * 16 * 256, mode: WispMode::Snuff(20), aim: 1, born: 1 },
            Wisp { x: 25 * 16 * 256, y: 15 * 16 * 256, mode: WispMode::Shy(30), aim: 4, born: 2 },
        ];
    }
    put_knight(&mut r, 1, (17, 16));
    shot("moonfall-06-night-wisps", &r, 1);
    // The south causeway at night.
    put_knight(&mut r, 1, (17, 21));
    shot("moonfall-07-night-causeway", &r, 1);
    // Final hours.
    r.moonfall.as_mut().unwrap().world.wisps.clear();
    at_hour(&mut r, 3, 1, 0);
    put_knight(&mut r, 1, (18, 16));
    shot("moonfall-08-final-hours", &r, 1);
    at_hour(&mut r, 3, 4, 30);
    shot("moonfall-09-final-hours-late", &r, 1);
    // The three lights lit.
    {
        let s = r.moonfall.as_mut().unwrap();
        let c = s.clock;
        s.world.lit_until = [c + 400; 3];
    }
    put_knight(&mut r, 1, (18, 6));
    shot("moonfall-10-north-lit", &r, 1);
    // The rewind, three beats of it.
    let day = r.moonfall.as_ref().unwrap().nums.day;
    for (n, t) in [(1, 4), (2, 14), (3, 26)] {
        r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t, from: clock::ticks(day, 120), fell: false };
        shot(&format!("moonfall-11-rewind-{n}"), &r, 1);
    }
    for (n, t) in [(1, 8), (2, 16), (3, 24)] {
        r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t, from: clock::end(day), fell: true };
        shot(&format!("moonfall-12-fall-{n}"), &r, 1);
    }
    r.moonfall.as_mut().unwrap().phase = Phase::Skip { t: 18, from: 100, to: day / 2 };
    shot("moonfall-13-skip", &r, 1);
    r.moonfall.as_mut().unwrap().phase = Phase::Play;
    // The Ledger, written a little.
    let mut r = table(2);
    {
        let s = r.moonfall.as_mut().unwrap();
        s.memory.learn(Fact::OrinLost);
        s.memory.learn(Fact::TovinPass);
        s.memory.learn(Fact::AsterComing);
        s.memory.witness(Folk::Mira, 1);
        s.memory.witness(Folk::Mira, 4);
        s.memory.witness(Folk::Mira, 10);
        s.memory.witness(Folk::Orin, 1);
        s.memory.witness(Folk::Orin, 2);
        s.memory.tick(Folk::Pella);
        s.memory.masks = 1;
        s.memory.loops = 2;
    }
    at_hour(&mut r, 1, 14, 30);
    for page in [0u8, 1, 4, 6] {
        r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.panel = Panel::Ledger(page);
        shot(&format!("moonfall-14-ledger-{page}"), &r, 1);
    }
    // A full Ledger: every fact known, every routine seen, the longest pages.
    {
        let s = r.moonfall.as_mut().unwrap();
        for f in ledger::FACTS {
            s.memory.learn(f);
        }
        for who in folk::CAST {
            for id in 0..24 {
                s.memory.witness(who, id);
            }
        }
        s.memory.ticks = 0b11111;
    }
    for page in [3u8, 5, 1] {
        r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.panel = Panel::Ledger(page);
        shot(&format!("moonfall-14-ledger-full-{page}"), &r, 1);
    }
    r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.panel = Panel::Bell(0);
    shot("moonfall-15-bell", &r, 1);
    {
        let s = r.moonfall.as_mut().unwrap();
        let k = &mut s.seats.get_mut(&1).unwrap().knight;
        k.panel = Panel::Shut;
        k.say = Some(Say { folk: Folk::Mira, topic: Topic::Request, page: 0, age: 0 });
    }
    put_knight(&mut r, 1, (11, 16));
    shot("moonfall-16-talk", &r, 1);
    {
        let s = r.moonfall.as_mut().unwrap();
        s.seats.get_mut(&1).unwrap().knight.say = None;
        s.completed = Some(1);
        s.clock = clock::end(s.nums.day);
    }
    shot("moonfall-17-win", &r, 1);
    // The Moonwell through its states with four knights about it: cold, oiled,
    // lit through its four frames, lit by night, and asking for its flame.
    let mut r = table(4);
    at_hour(&mut r, 1, 12, 0);
    for (id, tile) in [(1, (17, 15)), (2, (20, 15)), (3, (16, 12)), (4, (21, 12))] {
        put_knight(&mut r, id, tile);
    }
    // Each state at six times its size, the well alone, in one strip.
    let mut strip: Vec<Img> = Vec::new();
    let well = |name: &str, r: &mut Run, strip: &mut Vec<Img>| {
        r.moonfall.as_mut().unwrap().card = 0;
        shot(name, r, 1);
        let s = r.moonfall.as_ref().unwrap();
        let cam = camera(s.seats[&1].knight.px());
        let (wx, wy) = (town::WELL.0 - cam.x, town::WELL.1 - cam.y + VIEW_Y);
        let full = base_and_fx(r, 1);
        let mut crop = Img::black(56, 64);
        for y in 0..64 {
            for x in 0..56 {
                if let Some(c) = full.get(wx - 28 + x, wy - 48 + y) {
                    crop.set(x, y, c);
                }
            }
        }
        let mut big = Img::black(56 * 5, 64 * 5);
        big.stamp_times(&crop, 0, 0, 5);
        strip.push(big);
    };
    well("moonfall-20-well-idle", &mut r, &mut strip);
    r.moonfall.as_mut().unwrap().memory.keeps |= ledger::OIL;
    well("moonfall-20-well-oiled-a", &mut r, &mut strip);
    for _ in 0..10 {
        r.step(&BTreeMap::new());
    }
    well("moonfall-20-well-oiled-b", &mut r, &mut strip);
    for n in 0..4 {
        let s = r.moonfall.as_mut().unwrap();
        s.world.lit_until[2] = s.clock + 4000;
        s.card = 0;
        if n == 1 {
            // The terminal's cell size, through the same fitter the cells use.
            save("moonfall-20-well-cells", &frame_for(&r, 96, 56, Some(1)), 6);
        }
        well(&format!("moonfall-20-well-lit-{n}"), &mut r, &mut strip);
        for _ in 0..6 {
            r.step(&BTreeMap::new());
        }
    }
    at_hour(&mut r, 1, 22, 0);
    {
        let s = r.moonfall.as_mut().unwrap();
        s.world.lit_until[2] = s.clock + 4000;
    }
    well("moonfall-20-well-lit-night", &mut r, &mut strip);
    {
        let s = r.moonfall.as_mut().unwrap();
        s.world.lit_until[2] = 0;
        s.memory.learn(Fact::Verse);
    }
    at_hour(&mut r, 3, 1, 0);
    while !(r.tick / 10).is_multiple_of(2) {
        r.step(&BTreeMap::new());
    }
    well("moonfall-20-well-ready", &mut r, &mut strip);
    let mut sheet = Img::black(strip.len() as i32 * (56 * 5 + 2), 64 * 5);
    for (i, f) in strip.iter().enumerate() {
        sheet.stamp(f, i as i32 * (56 * 5 + 2), 0);
    }
    save("moonfall-20-well-strip", &sheet, 1);
    // A full table.
    let mut r = table(MAX_PLAYERS);
    at_hour(&mut r, 1, 8, 0);
    shot("moonfall-18-six-seats", &r, 3);
    // The terminal's cell size, through the same fitter the cells use.
    let r = table(2);
    for (w, h) in [(96, 56), (384, 224)] {
        let img = frame_for(&r, w, h, Some(1));
        save(&format!("moonfall-19-frame-{w}"), &img, if w < 200 { 6 } else { 1 });
    }
}

/// The plaza's buildings and the bell at every hour, each from a seat that
/// frames them, with two knights standing at their doors for scale. Run
/// before and after a building is painted to compare the two:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve --lib
/// write_moonfall_review_buildings -- --ignored`.
#[test]
#[ignore = "writes Moonfall's building review pictures"]
fn write_moonfall_review_buildings() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    // (name, day, hour, minute)
    let hours = [
        ("a-dawn", 1, 6, 0),
        ("b-morning", 1, 9, 0),
        ("c-noon", 1, 12, 0),
        ("d-late", 1, 17, 0),
        ("e-dusk", 1, 18, 30),
        ("f-evening", 1, 20, 30),
        ("g-night", 1, 22, 0),
        ("h-deep", 1, 2, 0),
    ];
    // (name, the seat's own tile, the two knights' tiles)
    let views = [
        ("east", (31, 13), [(24, 15), (28, 15)]),
        ("inn", (28, 17), [(24, 17), (33, 17)]),
        ("archive", (9, 13), [(9, 15), (11, 15)]),
        ("bell", (14, 15), [(13, 14), (15, 14)]),
    ];
    let mut r = table(3);
    for (hour, day, hh, mm) in hours {
        at_hour(&mut r, day, hh, mm);
        for (view, mine, others) in views {
            put_knight(&mut r, 1, mine);
            put_knight(&mut r, 2, others[0]);
            put_knight(&mut r, 3, others[1]);
            let both = base_and_fx(&r, 1);
            save(&format!("{view}-{hour}-1x"), &both, 1);
            save(&format!("{view}-{hour}-3x"), &both, 3);
        }
    }
    // The bell through its ritual: the town winds back and the bell tolls.
    at_hour(&mut r, 1, 20, 0);
    let day = r.moonfall.as_ref().unwrap().nums.day;
    put_knight(&mut r, 1, (14, 15));
    put_knight(&mut r, 2, (13, 14));
    put_knight(&mut r, 3, (15, 14));
    let mut strip: Vec<Img> = Vec::new();
    for t in [0u32, 4, 8, 12, 16, 20, 24, 28] {
        r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t, from: clock::ticks(day, 80), fell: false };
        let cam = camera(r.moonfall.as_ref().unwrap().seats[&1].knight.px());
        let (bx, by) = (town::BELL_POST.0 - cam.x, town::BELL_POST.1 - cam.y + VIEW_Y);
        let full = base_and_fx(&r, 1);
        let mut crop = Img::black(48, 56);
        for y in 0..56 {
            for x in 0..48 {
                if let Some(c) = full.get(bx - 24 + x, by - 44 + y) {
                    crop.set(x, y, c);
                }
            }
        }
        let mut big = Img::black(48 * 4, 56 * 4);
        big.stamp_times(&crop, 0, 0, 4);
        strip.push(big);
        if t == 8 {
            save("bell-rewind-8-3x", &full, 3);
        }
    }
    let mut sheet = Img::black(strip.len() as i32 * (48 * 4 + 2), 56 * 4);
    for (i, f) in strip.iter().enumerate() {
        sheet.stamp(f, i as i32 * (48 * 4 + 2), 0);
    }
    save("bell-ritual-strip", &sheet, 1);
    // The east side while the clock winds back through the hours.
    put_knight(&mut r, 1, (31, 13));
    put_knight(&mut r, 2, (24, 15));
    put_knight(&mut r, 3, (28, 15));
    for t in [6u32, 20, 40] {
        r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t, from: clock::ticks(day, 80), fell: false };
        save(&format!("east-rewind-{t}-3x"), &base_and_fx(&r, 1), 3);
    }
}

/// The rest of the town at every hour, each from a seat that frames it, with
/// two knights near for scale: the scriptorium, the north tower and the bridge
/// hut, the bridge, the plaza's podium and lanterns, the cottage and the herald
/// house; then the sky (the moon at each stage of the loop, its fall and its
/// turning). Run before and after painting to compare the two:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve --lib
/// write_moonfall_review_town -- --ignored`.
#[test]
#[ignore = "writes Moonfall's town review pictures"]
fn write_moonfall_review_town() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let hours = [
        ("a-dawn", 1, 6, 0),
        ("b-morning", 1, 9, 0),
        ("c-noon", 1, 12, 0),
        ("d-late", 1, 17, 0),
        ("e-dusk", 1, 18, 30),
        ("f-evening", 1, 20, 30),
        ("g-night", 1, 22, 0),
        ("h-deep", 1, 2, 0),
    ];
    // (name, the seat's own tile, the two knights' tiles)
    let views = [
        ("northwest", (6, 6), [(4, 6), (7, 6)]),
        ("tower", (18, 4), [(17, 5), (19, 5)]),
        ("bridge", (18, 7), [(17, 7), (19, 7)]),
        ("plaza", (20, 16), [(19, 15), (23, 15)]),
        ("west", (4, 13), [(2, 14), (5, 14)]),
        ("herald", (32, 15), [(30, 15), (34, 15)]),
    ];
    let mut r = table(3);
    for (hour, day, hh, mm) in hours {
        at_hour(&mut r, day, hh, mm);
        for (view, mine, others) in views {
            put_knight(&mut r, 1, mine);
            put_knight(&mut r, 2, others[0]);
            put_knight(&mut r, 3, others[1]);
            save(&format!("{view}-{hour}-3x"), &base_and_fx(&r, 1), 3);
        }
    }
    // The bridge as it is raised at 20:00 and lowered at 06:00: the picture
    // every six ticks of its eighteen, at four times its size, in one strip.
    put_knight(&mut r, 1, (18, 7));
    let day = r.moonfall.as_ref().unwrap().nums.day;
    let mut lift = Vec::new();
    for (d, hh, mm) in [(1, 19, 55), (2, 5, 55)] {
        at_hour(&mut r, d, hh, mm);
        let first = (r.moonfall.as_ref().unwrap().clock..).find(|&c| clock::bridge_up(c, day) == (hh == 19)).unwrap();
        for after in [0u32, 4, 7, 10, 13, 16, 19] {
            r.moonfall.as_mut().unwrap().clock = first + after;
            let cam = camera(r.moonfall.as_ref().unwrap().seats[&1].knight.px());
            let full = base_and_fx(&r, 1);
            let (bx, by) = (296 - cam.x, 160 - cam.y + VIEW_Y);
            let mut crop = Img::black(64, 44);
            for y in 0..44 {
                for x in 0..64 {
                    if let Some(c) = full.get(bx - 32 + x, by - 38 + y) {
                        crop.set(x, y, c);
                    }
                }
            }
            let mut big = Img::black(64 * 4, 44 * 4);
            big.stamp_times(&crop, 0, 0, 4);
            lift.push(big);
        }
    }
    let mut sheet = Img::black(7 * (64 * 4 + 2), 2 * (44 * 4 + 2));
    for (i, f) in lift.iter().enumerate() {
        sheet.stamp(f, (i % 7) as i32 * (64 * 4 + 2), (i / 7) as i32 * (44 * 4 + 2));
    }
    save("bridge-lift-strip", &sheet, 1);
    // The plaza at night with a wisp leaning into each of three lamps at a
    // different moment of being put out.
    at_hour(&mut r, 1, 22, 0);
    {
        let s = r.moonfall.as_mut().unwrap();
        s.world.wisps = vec![
            Wisp { x: 13 * 16 * 256, y: 17 * 16 * 256, mode: WispMode::Snuff(16), aim: 1, born: 0 },
            Wisp { x: 20 * 16 * 256, y: 17 * 16 * 256, mode: WispMode::Snuff(10), aim: 3, born: 1 },
            Wisp { x: 24 * 16 * 256, y: 17 * 16 * 256, mode: WispMode::Snuff(3), aim: 4, born: 2 },
        ];
    }
    put_knight(&mut r, 1, (20, 16));
    put_knight(&mut r, 2, (12, 15));
    put_knight(&mut r, 3, (27, 15));
    save("plaza-snuffing-3x", &base_and_fx(&r, 1), 3);
    {
        let s = r.moonfall.as_mut().unwrap();
        s.world.wisps.clear();
        s.world.snuffed = 0b001010;
    }
    save("plaza-two-out-3x", &base_and_fx(&r, 1), 3);
    r.moonfall.as_mut().unwrap().world.snuffed = 0;
    // The podium while the herald cries, over the animation's four beats.
    at_hour(&mut r, 1, 12, 0);
    put_knight(&mut r, 1, (20, 16));
    put_knight(&mut r, 2, (19, 15));
    put_knight(&mut r, 3, (23, 15));
    for n in 0..4u32 {
        r.moonfall.as_mut().unwrap().cry = Some((Folk::Pella, Topic::Cry(1), 60));
        r.tick = 6 * u64::from(n);
        save(&format!("plaza-cry-{n}-3x"), &base_and_fx(&r, 1), 3);
    }
    r.moonfall.as_mut().unwrap().cry = None;
    // The sky: the moon at each stage of the loop, its fall and its turning.
    let day = r.moonfall.as_ref().unwrap().nums.day;
    for (name, d, hh, mm) in [
        ("m1-day1-dawn", 1, 6, 0),
        ("m2-day1-night", 1, 22, 0),
        ("m3-day2-noon", 2, 12, 0),
        ("m4-day2-night", 2, 22, 0),
        ("m5-day3-deep", 3, 3, 0),
        ("m6-day3-evening", 3, 20, 0),
        ("m7-day3-late", 3, 23, 30),
        ("m8-day3-last", 3, 5, 30),
    ] {
        at_hour(&mut r, d, hh, mm);
        put_knight(&mut r, 1, (20, 16));
        save(&format!("moon-{name}-3x"), &base_and_fx(&r, 1), 3);
    }
    for (n, t) in [(1, 2), (2, 6), (3, 10), (4, 14), (5, 18), (6, 24)] {
        r.moonfall.as_mut().unwrap().phase = Phase::Rewind { t, from: clock::end(day), fell: true };
        save(&format!("moon-fall-{n}-3x"), &base_and_fx(&r, 1), 3);
    }
    for (n, t) in [(1, 0), (2, 30), (3, 60)] {
        r.moonfall.as_mut().unwrap().phase = Phase::Won { t };
        save(&format!("moon-won-{n}-3x"), &base_and_fx(&r, 1), 3);
    }
    {
        let s = r.moonfall.as_mut().unwrap();
        s.phase = Phase::Play;
        s.completed = Some(1);
        s.clock = clock::end(s.nums.day);
    }
    save("moon-turned-3x", &base_and_fx(&r, 1), 3);
}

/// Where the knights stand for the label review: the seat's own tile, then
/// the others close about it (tags collide when knights huddle).
fn huddle(r: &mut Run, seats: u32, at: (i32, i32)) {
    let offsets = [(0, 0), (-1, 0), (1, 0), (0, -1), (-2, 1), (2, 1)];
    for id in 1..=seats {
        let (dx, dy) = offsets[(id - 1) as usize];
        put_knight(r, id, (at.0 + dx, at.1 + dy));
    }
}

/// The hours the labels are reviewed at: (name, day, hour, minute).
const LABEL_HOURS: [(&str, u32, u32, u32); 8] = [
    ("a-0600", 1, 6, 0),
    ("b-0900", 1, 9, 0),
    ("c-1215", 1, 12, 15),
    ("d-1700", 1, 17, 0),
    ("e-1830", 1, 18, 30),
    ("f-2030", 1, 20, 30),
    ("g-2200", 1, 22, 0),
    ("h-0200", 1, 2, 0),
];

/// The label review: the plaza, the archive, the east end and the nearest
/// villager, at every hour, with one, two, four and six knights huddled.
/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve --lib
/// write_moonfall_review_labels -- --ignored`.
#[test]
#[ignore = "writes Moonfall's label review pictures"]
fn write_moonfall_review_labels() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    for seats in [1u32, 2, 4, 6] {
        let mut r = table(seats);
        for (hour, day, hh, mm) in LABEL_HOURS {
            at_hour(&mut r, day, hh, mm);
            let s = r.moonfall.as_ref().unwrap();
            let shown = s.shown_clock();
            let near = folk::CAST
                .iter()
                .map(|&f| folk::figure(f, shown, s.nums.day, s.world.shod))
                .find(|f| f.visible)
                .map(|f| (f.x / 16, f.y / 16 + 1));
            let mut views = vec![("plaza", (20, 16)), ("archive", (9, 15)), ("east", (31, 15)), ("bell", (14, 15))];
            if let Some(at) = near {
                views.push(("folk", at));
            }
            for (view, at) in views {
                huddle(&mut r, seats, at);
                save(&format!("lab-{view}-s{seats}-{hour}"), &base_and_fx(&r, 1), 1);
            }
        }
    }
}

/// The HUD's states: banners, the bell's call, a held item, a mask, the
/// Final Hours, the ritual's words, with one, four and six knights.
/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve --lib
/// write_moonfall_review_hud -- --ignored`.
#[test]
#[ignore = "writes Moonfall's HUD review pictures"]
fn write_moonfall_review_hud() {
    use crate::rules::moonfall::state::{Call, Item, Mask, Note, Ring};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    for seats in [1u32, 4, 6] {
        let mut r = table(seats);
        at_hour(&mut r, 1, 12, 15);
        huddle(&mut r, seats, (17, 15));
        {
            let s = r.moonfall.as_mut().unwrap();
            s.card = 0;
            s.news = vec![
                (Note::Learned(Fact::OrinLost), 200),
                (Note::Learned(Fact::AsterLame), 150),
                (Note::Wrote(Folk::Mira, 3), 100),
                (Note::Ticked(Folk::Pella), 50),
            ];
            s.call = Some(Call { ring: Ring::Rewind, by: 1, ayes: 1, left: 100 });
            let k = &mut s.seats.get_mut(&1).unwrap().knight;
            k.mask = Mask::Swift;
            k.held = Item::Reeds;
        }
        save(&format!("hud-s{seats}-a-news"), &base_and_fx(&r, 1), 1);
        {
            let s = r.moonfall.as_mut().unwrap();
            let k = &mut s.seats.get_mut(&1).unwrap().knight;
            k.held = Item::Empty;
            k.mask = Mask::Stout;
            k.note = Some((Note::Learned(Fact::AsterLame), 100));
            s.call = None;
            s.news = vec![(Note::Learned(Fact::NessaCold), 100)];
            s.memory.loops = 11;
        }
        save(&format!("hud-s{seats}-b-note"), &base_and_fx(&r, 1), 1);
        let day = r.moonfall.as_ref().unwrap().nums.day;
        {
            let s = r.moonfall.as_mut().unwrap();
            s.seats.get_mut(&1).unwrap().knight.note = None;
            s.news = vec![(Note::Wrote(Folk::Pella, 2), 100), (Note::Candle(2), 80)];
        }
        at_hour(&mut r, 3, 2, 0);
        save(&format!("hud-s{seats}-c-final"), &base_and_fx(&r, 1), 1);
        // The ritual and the skip at the bell, then the dawn card and the hold bar.
        {
            let s = r.moonfall.as_mut().unwrap();
            s.news.clear();
        }
        at_hour(&mut r, 1, 20, 0);
        huddle(&mut r, seats, (14, 15));
        for (name, phase) in [
            ("d-ritual", Phase::Rewind { t: 8, from: clock::ticks(day, 80), fell: false }),
            ("e-skip", Phase::Skip { t: 12, from: 100, to: day / 2 }),
        ] {
            r.moonfall.as_mut().unwrap().phase = phase;
            save(&format!("hud-s{seats}-{name}"), &base_and_fx(&r, 1), 1);
        }
        r.moonfall.as_mut().unwrap().phase = Phase::Play;
        {
            let s = r.moonfall.as_mut().unwrap();
            s.card = 90;
            s.memory.loops = 1;
        }
        at_hour(&mut r, 1, 6, 0);
        save(&format!("hud-s{seats}-f-card"), &base_and_fx(&r, 1), 1);
        {
            let s = r.moonfall.as_mut().unwrap();
            s.card = 0;
            s.seats.get_mut(&1).unwrap().knight.hold = 10;
            s.seats.get_mut(&2.min(seats)).unwrap().knight.down = 40;
        }
        huddle(&mut r, seats, (18, 15));
        save(&format!("hud-s{seats}-g-hold-out"), &base_and_fx(&r, 1), 1);
        {
            let s = r.moonfall.as_mut().unwrap();
            s.seats.get_mut(&1).unwrap().knight.hold = 0;
            s.seats.get_mut(&1).unwrap().knight.down = 40;
        }
        save(&format!("hud-s{seats}-h-snuffed"), &base_and_fx(&r, 1), 1);
        {
            let s = r.moonfall.as_mut().unwrap();
            let k = &mut s.seats.get_mut(&1).unwrap().knight;
            k.down = 0;
            k.say = Some(Say { folk: Folk::Mira, topic: Topic::Request, page: 0, age: 0 });
        }
        save(&format!("hud-s{seats}-i-talk"), &base_and_fx(&r, 1), 1);
    }
}

// ---- where the words stand ----------------------------------------------

/// Where the seat stands for the label checks, the others huddled about it:
/// the plaza, both edges of the town (where plates meet the frame), the bell,
/// the north and south ends and the inn.
const LABEL_VIEWS: [(&str, (i32, i32)); 8] = [
    ("plaza", (20, 16)),
    ("archive", (9, 15)),
    ("west", (2, 14)),
    ("east", (34, 15)),
    ("bell", (14, 15)),
    ("north", (18, 5)),
    ("south", (17, 24)),
    ("inn", (28, 17)),
];

/// A table of `seats` at a moment, the dawn card gone.
fn quiet_table(seats: u32, (day, hh, mm): (u32, u32, u32)) -> Run {
    let mut r = table(seats);
    at_hour(&mut r, day, hh, mm);
    r.moonfall.as_mut().unwrap().card = 0;
    r
}

/// Every plate of every label in a scene, with its words.
fn plates(sc: &Scene) -> Vec<(String, labels::Rect)> {
    sc.labels.iter().flat_map(|p| p.parts.iter().map(|q| (q.text.clone(), q.rect))).collect()
}

/// What is wrong with the labels of a scene: a plate that leaves the town
/// view, touches another, or touches a place the HUD keeps.
fn label_faults(sc: &Scene) -> Vec<String> {
    let all = plates(sc);
    let mut bad = Vec::new();
    for (i, (text, r)) in all.iter().enumerate() {
        if !r.in_frame() {
            bad.push(format!("{text:?} {r:?} leaves the town view"));
        }
        // A pixel of the town between any two, so words never read as one.
        for (other, o) in &all[i + 1..] {
            if r.grown(1).hits(*o) {
                bad.push(format!("{text:?} {r:?} runs into {other:?} {o:?}"));
            }
        }
        for (k, _) in &sc.keep {
            if r.grown(1).hits(*k) {
                bad.push(format!("{text:?} {r:?} runs into the HUD's {k:?}"));
            }
        }
    }
    bad
}

/// The moments of the label checks: dawn, noon, the copper dusk, night, the
/// deep night and the Final Hours.
fn label_moments() -> Vec<(String, (u32, u32, u32))> {
    let mut v: Vec<_> = [LABEL_HOURS[0], LABEL_HOURS[2], LABEL_HOURS[4], LABEL_HOURS[6], LABEL_HOURS[7]]
        .iter()
        .map(|&(name, d, h, m)| (name.to_string(), (d, h, m)))
        .collect();
    v.push(("d3-0300".into(), (3, 3, 0)));
    v
}

/// The first villager in view at the moment, as a view of its own: the
/// seat stands one tile below.
fn folk_view(r: &Run) -> Option<(i32, i32)> {
    let s = r.moonfall.as_ref().unwrap();
    folk::CAST
        .iter()
        .map(|&f| folk::figure(f, s.shown_clock(), s.nums.day, s.world.shod))
        .find(|f| f.visible)
        .map(|f| ((f.x / 16).clamp(2, 35), (f.y / 16 + 1).clamp(2, 26)))
}

#[test]
fn every_label_is_whole_inside_the_town_view_and_touches_no_other_at_every_hour_and_seat_count() {
    // One to six knights huddled in the plaza, at both edges of the town,
    // at the bell, at the south end, and about the first villager in view; at
    // dawn, noon, dusk, night, the deep night and in the Final Hours.
    let mut checked = 0;
    let mut moved = 0;
    for seats in 1..=MAX_PLAYERS as u32 {
        let mut r = table(seats);
        for (hour, moment) in label_moments() {
            let (day, hh, mm) = moment;
            at_hour(&mut r, day, hh, mm);
            r.moonfall.as_mut().unwrap().card = 0;
            let mut views: Vec<_> = [0, 1, 2, 3, 4, 6].iter().map(|&i| LABEL_VIEWS[i]).collect();
            views.extend(folk_view(&r).map(|at| ("folk", at)));
            for (view, at) in views {
                huddle(&mut r, seats, at);
                let sc = scene(&r, Some(1));
                let at = format!("{seats} seats, {hour}, {view}");
                let bad = label_faults(&sc);
                assert!(bad.is_empty(), "{at}: {bad:#?}");
                assert!(sc.left_out.is_empty(), "{at}: nothing holds the words, yet {:?} is left out", sc.left_out);
                // Every knight has a number, however close they stand.
                let numbers = sc.labels.iter().filter(|p| p.kind == labels::Kind::Knight).count();
                assert_eq!(numbers, seats as usize, "{at}: a seat lost its number");
                // Every villager in view has a name.
                let s = r.moonfall.as_ref().unwrap();
                let cam = camera(s.seats[&1].knight.px());
                let seen = folk::CAST
                    .iter()
                    .map(|&f| folk::figure(f, s.shown_clock(), s.nums.day, s.world.shod))
                    .filter(|f| f.visible)
                    .filter(|f| {
                        let (x, y) = (f.x - cam.x, f.y - cam.y + VIEW_Y);
                        (14..=370).contains(&x) && (VIEW_Y + 44..=VIEW_Y + VIEW_H).contains(&y)
                    })
                    .count();
                let named = sc.labels.iter().filter(|p| p.kind == labels::Kind::Folk).count();
                assert!(named >= seen, "{at}: {named} names for {seen} villagers");
                // A plate stands where its building is, shifted into the frame;
                // it leaves that place (or shortens) only when the HUD, an
                // earlier plate or its own picture is in the way.
                let mut earlier: Vec<labels::Rect> = Vec::new();
                for p in &sc.labels {
                    if p.kind == labels::Kind::Plate {
                        let home = p.home.fitted();
                        if labels::bounds(&p.parts) != home {
                            moved += 1;
                            let held = sc.keep.iter().any(|&(k, _)| home.hits(k)) || earlier.iter().any(|&e| home.hits(e)) || home.hits(p.body);
                            assert!(held, "{at}: plate {:?} left {home:?} for {:?} with nothing in its way", p.parts[0].text, labels::bounds(&p.parts));
                        }
                    }
                    earlier.extend(p.parts.iter().map(|q| q.rect));
                }
                checked += 1;
            }
        }
    }
    assert!(checked >= 200, "{checked}");
    assert!(moved > 0, "some plate is held by another or by the edge, or the check does not bite");
}

#[test]
fn the_same_state_draws_the_same_words_in_the_same_places() {
    let r = quiet_table(4, (1, 12, 15));
    let (a, b) = (scene(&r, Some(1)), scene(&r, Some(1)));
    assert_eq!(plates(&a), plates(&b));
    assert_eq!(a.base.content_hash(), b.base.content_hash());
}

/// A scene with the herald crying at the podium, and the table at the plaza.
fn crying_table(seats: u32) -> Run {
    let mut r = quiet_table(seats, (1, 12, 0));
    huddle(&mut r, seats, (20, 16));
    r.moonfall.as_mut().unwrap().cry = Some((Folk::Pella, Topic::Cry(1), 60));
    r
}

/// How many lines the cry on the table has.
fn r_words(r: &Run) -> usize {
    let s = r.moonfall.as_ref().unwrap();
    let (who, topic, _) = s.cry.unwrap();
    s.words(who, topic).len().min(3)
}

#[test]
fn the_heralds_cry_stands_whole_clear_of_every_plate_and_name() {
    for seats in 1..=MAX_PLAYERS as u32 {
        let sc = scene(&crying_table(seats), Some(1));
        let said = sc.labels.iter().filter(|p| p.kind == labels::Kind::Speech).count();
        assert_eq!(said, 1, "{seats} seats: the cry shows");
        let words = r_words(&crying_table(seats));
        assert!(sc.labels.iter().filter(|p| p.kind == labels::Kind::Speech).all(|p| p.parts.len() == words), "every line of the cry shows");
        let bad = label_faults(&sc);
        assert!(bad.is_empty(), "{seats} seats: {bad:#?}");
    }
}

#[test]
fn the_edge_of_the_town_shifts_a_plate_in_and_never_clips_it() {
    // From the plaza's east side the archive's middle is off the left edge of
    // the town view and the herald's house's off the right: both plates whole,
    // against the edge, still over their buildings.
    let mut r = quiet_table(1, (1, 12, 0));
    put_knight(&mut r, 1, (22, 15));
    let sc = scene(&r, Some(1));
    let archive = plates(&sc).into_iter().find(|(t, _)| t == "ARCHIVE").expect("the archive's plate is up").1;
    assert_eq!(archive.x, 0, "against the left edge, not clipped by it: {archive:?}");
    put_knight(&mut r, 1, (21, 15));
    let sc = scene(&r, Some(1));
    let herald = plates(&sc).into_iter().find(|(t, _)| t.starts_with("HERALD")).expect("the herald house's plate is up").1;
    assert!(herald.in_frame() && herald.right() >= 380, "{herald:?}");
}

#[test]
fn a_plate_stands_only_for_a_building_with_a_tile_of_it_in_view() {
    // Looking north from the plaza the south bell tower is far below the view:
    // no plate hangs at the foot of the picture for it.
    let r = quiet_table(1, (1, 12, 0));
    let mut r = r;
    put_knight(&mut r, 1, (17, 14));
    let sc = scene(&r, Some(1));
    assert!(plates(&sc).iter().all(|(t, _)| t != "SOUTH BELL TOWER"), "{:?}", plates(&sc));
    put_knight(&mut r, 1, (17, 23));
    let sc = scene(&r, Some(1));
    assert!(plates(&sc).iter().any(|(t, _)| t.starts_with("SOUTH")), "in view, so named");
}

#[test]
fn the_words_keep_their_colours_at_every_hour_and_read_against_their_plates() {
    use super::super::super::ink::ink;
    let black = ink('k').unwrap();
    let mut checked = 0;
    for seats in [1u32, 6] {
        let mut r = table(seats);
        for (hour, moment) in label_moments() {
            let (day, hh, mm) = moment;
            at_hour(&mut r, day, hh, mm);
            r.moonfall.as_mut().unwrap().card = 0;
            for (view, at) in [LABEL_VIEWS[0], LABEL_VIEWS[1], LABEL_VIEWS[3], LABEL_VIEWS[4]] {
                huddle(&mut r, seats, at);
                let sc = scene(&r, Some(1));
                for part in sc.labels.iter().flat_map(|p| &p.parts) {
                    let (rect, edge) = (part.rect, part.edge.and_then(ink));
                    let mut words = Img::new(rect.w, rect.h);
                    words.text(part.pad.0, part.pad.1, &part.text, part.ink);
                    let colour = ink(part.ink).unwrap();
                    assert!(labels::contrast_of(colour, black) >= labels::MIN_CONTRAST, "{:?} in {:?} is too dim", part.text, part.ink);
                    for y in 0..rect.h {
                        for x in 0..rect.w {
                            let on_edge = edge.is_some() && (x == 0 || y == 0 || x == rect.w - 1 || y == rect.h - 1);
                            let want = if on_edge { edge } else if words.get(x, y).is_some() { Some(colour) } else { Some(black) };
                            assert_eq!(
                                sc.base.get(rect.x + x, rect.y + y),
                                want,
                                "{seats} seats, {hour}, {view}: {:?} at ({x}, {y}) of its plate",
                                part.text
                            );
                        }
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 400, "{checked}");
}

#[test]
fn a_dim_ink_is_drawn_pale_in_a_frame_of_itself() {
    use super::super::super::ink::ink;
    // Seat 4's red and Nessa's rust are under the contrast a small word needs
    // on black; every other name and number is its own colour.
    let dim: Vec<char> = (1..=6).map(ground::seat_ink).chain(folk::CAST.map(|f| f.ink())).filter(|&c| labels::contrast(c) < labels::MIN_CONTRAST).collect();
    assert!(!dim.is_empty());
    for c in dim {
        let part = labels::Part::plate("4", c, 10, 10);
        assert_eq!(part.edge, Some(c), "framed in its own ink");
        assert!(labels::contrast_of(ink(part.ink).unwrap(), ink('k').unwrap()) >= labels::MIN_CONTRAST, "pale word");
    }
    let plain = labels::Part::plate("BELL", 'T', 10, 10);
    assert_eq!((plain.ink, plain.edge), ('T', None));
}

#[test]
fn the_bells_banner_stands_clear_of_the_bell_the_ritual_glass_and_every_plate() {
    let day = quiet_table(1, (1, 20, 0)).moonfall.as_ref().unwrap().nums.day;
    let phases = [
        Phase::Rewind { t: 4, from: clock::ticks(day, 80), fell: false },
        Phase::Rewind { t: 14, from: clock::ticks(day, 80), fell: false },
        Phase::Rewind { t: 30, from: clock::end(day), fell: true },
        Phase::Skip { t: 12, from: 100, to: day / 2 },
        Phase::Skip { t: 12, from: day / 2, to: day + 100 },
    ];
    let mut shown_banner = 0;
    for seats in [1u32, 3, 6] {
        let mut r = quiet_table(seats, (1, 20, 0));
        for (view, at) in [("bell", (14, 15)), ("plaza", (17, 15)), ("archive", (10, 14)), ("north", (14, 9))] {
            huddle(&mut r, seats, at);
            for phase in phases {
                r.moonfall.as_mut().unwrap().phase = phase;
                let sc = scene(&r, Some(1));
                let bad = label_faults(&sc);
                assert!(bad.is_empty(), "{seats} seats, {view}, {phase:?}: {bad:#?}");
                let cam = camera(r.moonfall.as_ref().unwrap().seats[&1].knight.px());
                let (bx, by) = (town::BELL_POST.0 - cam.x, town::BELL_POST.1 - cam.y + VIEW_Y);
                let post = labels::Rect::new(bx - 12, by - 36, 24, 38);
                for p in sc.labels.iter().filter(|p| p.kind == labels::Kind::Banner) {
                    assert!(!p.parts[0].rect.hits(post), "{seats} seats, {view}, {phase:?}: the banner covers the bell {post:?} {:?}", p.parts[0].rect);
                    shown_banner += 1;
                }
            }
        }
    }
    assert!(shown_banner >= 40, "the banner shows ({shown_banner})");
}

#[test]
fn a_panel_card_or_conversation_hides_the_words_it_would_cover_and_leaves_none_half_covered() {
    for seats in [1u32, 4, 6] {
        let mut r = quiet_table(seats, (1, 12, 15));
        huddle(&mut r, seats, (17, 15));
        for panel in [Panel::Ledger(0), Panel::Ledger(3), Panel::Bell(1)] {
            r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.panel = panel;
            let sc = scene(&r, Some(1));
            let bad = label_faults(&sc);
            assert!(bad.is_empty(), "{seats} seats, {panel:?}: {bad:#?}");
        }
        r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.panel = Panel::Shut;
        r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.say = Some(Say { folk: Folk::Mira, topic: Topic::Request, page: 0, age: 0 });
        let bad = label_faults(&scene(&r, Some(1)));
        assert!(bad.is_empty(), "{seats} seats, talking: {bad:#?}");
        r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.say = None;
        r.moonfall.as_mut().unwrap().seats.get_mut(&1).unwrap().knight.down = 40;
        let bad = label_faults(&scene(&r, Some(1)));
        assert!(bad.is_empty(), "{seats} seats, snuffed: {bad:#?}");
    }
}

/// The HUD with everything on: the bell's call and four notes, the Final Hours
/// box, the hold bar, a note at the foot, a mask and a long loop count.
fn busy_hud(seats: u32) -> Run {
    use crate::rules::moonfall::state::{Call, Mask, Note, Ring};
    let mut r = quiet_table(seats, (3, 2, 0));
    huddle(&mut r, seats, (17, 15));
    let s = r.moonfall.as_mut().unwrap();
    s.news = vec![
        (Note::Learned(Fact::OrinLost), 200),
        (Note::Learned(Fact::AsterLame), 150),
        (Note::Wrote(Folk::Mira, 3), 100),
        (Note::Ticked(Folk::Pella), 50),
        (Note::Candle(2), 20),
    ];
    s.call = Some(Call { ring: Ring::Rewind, by: 1, ayes: 1, left: 100 });
    s.memory.loops = 998;
    let k = &mut s.seats.get_mut(&1).unwrap().knight;
    k.mask = Mask::Stout;
    k.hold = 10;
    k.note = Some((Note::Learned(Fact::Verse), 100));
    r
}

#[test]
fn the_huds_banner_lines_the_final_hours_box_and_the_hold_bar_stand_apart_and_the_call_comes_first() {
    for seats in [1u32, 4, 6] {
        let r = busy_hud(seats);
        let s = r.moonfall.as_ref().unwrap();
        let me = &s.seats[&1].knight;
        let lines = hud::news_lines(s, me);
        assert_eq!(lines.len(), 4, "four lines at most");
        assert!(lines[0].0.starts_with("KNIGHT 1 RINGS FOR REWIND"), "the call a knight must answer is on top, not cut off: {lines:?}");
        assert!(lines.iter().all(|(l, _)| l.chars().count() <= 62 && text_width(l) <= 372));
        let sc = scene(&r, Some(1));
        let open: Vec<_> = sc.keep.iter().filter(|(_, modal)| !modal).map(|&(k, _)| k).collect();
        assert!(open.len() >= 6, "four lines, the box and the bar: {open:?}");
        for (i, a) in open.iter().enumerate() {
            assert!(a.in_frame(), "{a:?}");
            for b in &open[i + 1..] {
                assert!(!a.hits(*b), "{seats} seats: {a:?} runs into {b:?}");
            }
        }
        let bad = label_faults(&sc);
        assert!(bad.is_empty(), "{seats} seats: {bad:#?}");
        // Under a curtain of banner lines a plate may be left out; a name or a
        // number never is.
        assert!(sc.left_out.iter().all(|(kind, _)| *kind == labels::Kind::Plate), "{:?}", sc.left_out);
    }
}

#[test]
fn a_note_is_cut_at_a_whole_word_with_two_dots_and_never_past_the_width_of_the_picture() {
    assert_eq!(hud::fit("SHORT NOTE", 62), "SHORT NOTE");
    let long = "LEDGER: ASTER - HIS HORSE HAS LOST A SHOE. THE IRON CART IS AT THE FORGE UNTIL EIGHT.";
    let cut = hud::fit(long, 62);
    assert!(cut.chars().count() <= 62 && cut.ends_with("..") && !cut.ends_with(" .."), "{cut:?}");
    assert!(long.starts_with(cut.trim_end_matches("..")), "only whole words are kept: {cut:?}");
    assert!(!cut.contains("IS AT") || cut.ends_with("AT.."), "{cut:?}");
    let worst = hud::fit(&"X".repeat(100), 62);
    assert_eq!(worst.chars().count(), 62 - 2 + 2);
}

#[test]
fn the_clock_bars_words_candles_seat_chips_and_track_never_touch() {
    use crate::rules::moonfall::state::{Item, Mask, Note};
    let mut checked = 0;
    for seats in 1..=MAX_PLAYERS as u32 {
        let mut r = table(seats);
        for (day, hh, mm) in [(1, 6, 0), (1, 12, 15), (2, 9, 0), (2, 22, 0), (3, 1, 0), (3, 5, 30)] {
            at_hour(&mut r, day, hh, mm);
            for loops in [0u32, 8, 98, 998] {
                for variant in 0..4 {
                    {
                        let s = r.moonfall.as_mut().unwrap();
                        s.memory.loops = loops;
                        let k = &mut s.seats.get_mut(&1).unwrap().knight;
                        k.mask = [Mask::Bare, Mask::Swift, Mask::Stout, Mask::Swift][variant];
                        k.held = if variant == 2 { Item::Shoe } else { Item::Empty };
                        k.note = (variant == 3).then_some((Note::Learned(Fact::AsterLame), 100));
                    }
                    let s = r.moonfall.as_ref().unwrap();
                    let me = &s.seats[&1].knight;
                    for tick in [0, 10, 25] {
                        let rows = hud::bar_rows(s, me, s.shown_clock(), tick);
                        for (i, (a, ra)) in rows.iter().enumerate() {
                            assert!(ra.x >= 0 && ra.right() <= 384 && ra.y >= 192 && ra.bottom() <= 224, "{a} {ra:?} leaves the bar");
                            for (b, rb) in &rows[i + 1..] {
                                assert!(!ra.hits(*rb), "{seats} seats, loop {loops}, {day} {hh}: {a} {ra:?} runs into {b} {rb:?}");
                            }
                        }
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 400);
}

#[test]
fn the_seat_chips_stand_in_the_clock_bar_not_over_the_town() {
    // The six chips of a full table were a row over the town's top right,
    // under the Ledger's banner lines and the plates of the east end.
    let r = table(MAX_PLAYERS as u32);
    let s = r.moonfall.as_ref().unwrap();
    let rows = hud::bar_rows(s, &s.seats[&1].knight, s.shown_clock(), 0);
    let (_, seats) = rows.iter().find(|(name, _)| *name == "seats").unwrap();
    assert!(seats.y >= VIEW_Y + VIEW_H, "{seats:?}");
    let (im, _) = layers(&r, Some(1));
    // Each chip's number is in its place in the bar.
    for (id, chip) in hud::chip_rects(6).iter().enumerate() {
        let mut want = Img::new(chip.w, chip.h);
        want.text(2, 1, &format!("{}", id + 1), ground::seat_ink(id as u32 + 1));
        assert!((0..chip.h).any(|y| (0..chip.w).any(|x| want.get(x, y).is_some() && im.get(chip.x + x, chip.y + y) == want.get(x, y))), "chip {} is drawn", id + 1);
    }
}

#[test]
fn layout_rules_in_the_small() {
    use labels::{Kind, Layout, Rect};
    let anchor = Rect::new(0, 72, 80, 60);
    // A plate against the left edge slides in whole.
    let mut l = Layout::default();
    l.plate(2, 60, "ARCHIVE", 'T', anchor);
    let placed = l.place().placed;
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].parts[0].rect.x, 0);
    // A plate holds its place; a name under it moves out of its way.
    let mut l = Layout::default();
    l.plate(100, 100, "BAKERY", 'T', Rect::new(80, 112, 40, 40));
    let body = Rect::new(94, 90, 12, 22);
    l.body(body);
    l.tag(Kind::Folk, 0, 100, 100, "NESSA", 'R', body);
    let placed = l.place().placed;
    let (plate, name) = (&placed[0], &placed[1]);
    assert_eq!(plate.kind, Kind::Plate);
    assert_eq!(plate.parts[0].rect, plate.home.fitted());
    assert!(!name.parts[0].rect.hits(plate.parts[0].rect), "{:?} {:?}", name.parts[0].rect, plate.parts[0].rect);
    // The viewer's own number keeps its home and the other's moves.
    let mut l = Layout::default();
    let body = Rect::new(94, 90, 12, 22);
    l.body(body);
    l.tag(Kind::Knight, 3, 100, 70, "3", 'C', body);
    l.tag(Kind::Knight, 0, 100, 70, "1", '2', body);
    let placed = l.place().placed;
    assert_eq!(placed[0].parts[0].text, "1");
    assert_eq!(placed[0].parts[0].rect, placed[0].home);
    assert_ne!(placed[1].parts[0].rect, placed[1].home);
    // A long plate that finds no room takes its fixed short form on its row.
    let mut l = Layout::default();
    l.plate(330, 100, "FORGE", 'T', Rect::new(300, 112, 40, 40));
    l.plate(372, 100, "HERALD HOUSE", 'T', Rect::new(340, 112, 80, 40));
    let placed = l.place().placed;
    assert_eq!(placed[1].parts[0].text, "HERALD");
    assert_eq!(placed[1].parts[0].rect.y, placed[1].home.y);
    // A word that would sit under a panel is left out, not half covered.
    let mut l = Layout::default();
    l.keep(Rect::new(0, 0, 384, 192), true);
    l.plate(100, 100, "BELL", 'T', Rect::new(80, 112, 40, 40));
    let body = Rect::new(94, 90, 12, 22);
    l.tag(Kind::Folk, 0, 100, 100, "MIRA", 'V', body);
    let out = l.place();
    assert!(out.placed.is_empty());
    assert_eq!(out.left_out, vec![(Kind::Plate, "BELL".to_string()), (Kind::Folk, "MIRA".to_string())]);
    // Six knights stacked on one tile all keep their numbers.
    let mut l = Layout::default();
    let body = Rect::new(186, 100, 14, 24);
    l.body(body);
    for id in 1..=6u32 {
        l.tag(Kind::Knight, id, 192, 90, &format!("{id}"), '2', body);
    }
    let placed = l.place().placed;
    assert_eq!(placed.len(), 6);
    for (i, a) in placed.iter().enumerate() {
        for b in &placed[i + 1..] {
            assert!(!a.parts[0].rect.grown(1).hits(b.parts[0].rect));
        }
    }
}



