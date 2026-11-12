//! What an open floor keeps and shows (`chronicle`): remains, cairns, the
//! guardian's ward, the doors' slam, a chamber's hush, a skull's call and a
//! low knight's heartbeat.

use super::super::*;
use super::{BEAT, CALL, DROP, FLARE, Fire, HUSH, KINDLE_STEP, SETTLE, SLAM, drawn_here, fire_of, over, under};
use crate::render::overworld::ink;
use crate::rules::chronicle::Trace;
use std::collections::BTreeMap;

fn party(depth: u32) -> Run {
    let mut run = Run::new(21, 7, None);
    run.join_seat(2, "Wren");
    run.join_seat(3, "Bors");
    run.open_floor_for_test(depth);
    run.tick = 1000;
    run
}

fn open(run: &Run) -> &crate::rules::layout::Open {
    run.dungeon.open.as_ref().unwrap()
}

fn open_mut(run: &mut Run) -> &mut crate::rules::layout::Open {
    run.dungeon.open.as_mut().unwrap()
}

/// The ground layer in a window about `spot` (arena units).
fn ground(run: &Run, spot: (f32, f32)) -> Img {
    let (pw, ph) = room_px(run.room());
    let (px, py) = at(spot.0, spot.1);
    let mut cv = Img::window(pw, ph, (px - 120, py - 90, 240, 180));
    under(&mut cv, run);
    cv
}

/// The lit pixels of one tile (`c`, `r`) of the ground layer.
fn tile_lit(run: &Run, (c, r): (u16, u16)) -> usize {
    let (pw, ph) = room_px(run.room());
    let mut cv = Img::window(pw, ph, (i32::from(c) * 16, i32::from(r) * 16, 16, 16));
    under(&mut cv, run);
    lit(&cv)
}

/// The same, a screen and a half wide: a chamber's whole hush.
fn wide(run: &Run, spot: (f32, f32)) -> Img {
    let (pw, ph) = room_px(run.room());
    let (px, py) = at(spot.0, spot.1);
    let mut cv = Img::window(pw, ph, (px - 300, py - 200, 600, 400));
    under(&mut cv, run);
    cv
}

fn lit(cv: &Img) -> usize {
    let (sx, sy, sw, sh) = cv.span();
    (sy..sy + sh).map(|y| (sx..sx + sw).filter(|&x| cv.get(x, y).is_some()).count()).sum()
}

fn has_ink(cv: &Img, ch: char) -> bool {
    let want = ink::ink(ch).unwrap();
    let (sx, sy, sw, sh) = cv.span();
    (sy..sy + sh).any(|y| (sx..sx + sw).any(|x| cv.get(x, y) == Some(want)))
}

/// A spot inside a fight chamber, off its middle, with no one near it.
fn quiet_spot(run: &Run) -> (f32, f32) {
    let o = open(run);
    let ch = o.chambers.iter().find(|c| c.kind == RoomKind::Fight).unwrap();
    let (cx, cy) = ch.centre();
    (cx + 6.0, cy)
}

fn remains(run: &mut Run, kind: EnemyKind, spot: (f32, f32), born: u64) {
    open_mut(run).traces.push(Trace::Remains { kind, x: spot.0, y: spot.1, born });
}

#[test]
fn a_bare_floor_draws_nothing_on_the_ground() {
    let run = party(2);
    assert_eq!(lit(&ground(&run, quiet_spot(&run))), 0);
}

#[test]
fn remains_lie_where_the_dead_fell_in_what_they_were_made_of() {
    let mut run = party(2);
    let spot = quiet_spot(&run);
    for (kind, ink_in) in [
        (EnemyKind::Goblin, '8'),
        (EnemyKind::Skeleton, 'g'),
        (EnemyKind::Imp, 'X'),
        (EnemyKind::Slime, 'F'),
    ] {
        open_mut(&mut run).traces.clear();
        let born = run.tick - 100;
        remains(&mut run, kind, spot, born);
        let cv = ground(&run, spot);
        let n = lit(&cv);
        assert!(n >= 4 && n < 900, "{kind:?} lies in {n} pixels");
        if kind != EnemyKind::Slime {
            assert!(has_ink(&cv, ink_in), "{kind:?} lies in ink {ink_in}");
        }
        // Dim on the black paper: never the pale inks.
        for c in ['w', 'W', 'H', 'c', '9'] {
            assert!(!has_ink(&cv, c), "{kind:?} is no stark {c}");
        }
    }
    // It is nowhere else on the map.
    let far = open(&run).chambers.last().unwrap().centre();
    assert_eq!(lit(&ground(&run, far)), 0);
}

#[test]
fn remains_spread_over_their_first_moments_and_the_great_scorch_wider() {
    let mut run = party(2);
    let spot = quiet_spot(&run);
    let at_age = |run: &mut Run, kind, age: u64| {
        open_mut(run).traces.clear();
        let born = run.tick - age;
        remains(run, kind, spot, born);
        lit(&ground(run, spot))
    };
    let (fresh, settled) = (at_age(&mut run, EnemyKind::Goblin, 0), at_age(&mut run, EnemyKind::Goblin, SETTLE));
    assert!(fresh < settled, "{fresh} then {settled}");
    assert_eq!(settled, at_age(&mut run, EnemyKind::Goblin, SETTLE * 9), "and then it lies");
    let great = at_age(&mut run, EnemyKind::Boss, SETTLE);
    assert!(great > settled, "a guardian's scorch {great} is wider than a goblin's {settled}");
}

#[test]
fn a_cairn_flies_its_knights_colours_and_names_them_only_when_a_knight_is_by() {
    let mut run = party(2);
    let spot = quiet_spot(&run);
    for (id, cloth) in [(2u32, '1'), (3, 'A'), (4, '4')] {
        open_mut(&mut run).traces.clear();
        let name = if id == 2 { "WREN" } else if id == 3 { "BORS" } else { "YSOLDE" };
        let born = run.tick - 100;
        open_mut(&mut run).traces.push(Trace::Cairn { knight: id, name: name.into(), x: spot.0, y: spot.1, born });
        let cv = ground(&run, spot);
        assert!(has_ink(&cv, cloth), "knight {id}'s pennon is ink {cloth}");
        assert!(has_ink(&cv, 'U') || has_ink(&cv, 'u'), "stone");
    }
    // The name: a knight stands by, or it does not show.
    let (pw, ph) = room_px(run.room());
    let (px, py) = at(spot.0, spot.1);
    let label = |run: &Run, legible: bool| {
        let mut cv = Img::window(pw, ph, (px - 120, py - 90, 240, 180));
        over(&mut cv, run, legible);
        lit(&cv)
    };
    let far = open(&run).arrival;
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = far;
    }
    assert_eq!(label(&run, true), 0, "no one near");
    run.players.get_mut(&1).unwrap().x = spot.0 + 3.0;
    run.players.get_mut(&1).unwrap().y = spot.1;
    assert!(label(&run, true) > 40, "a knight by it reads who");
    assert_eq!(label(&run, false), 0, "a reduced frame leaves the words out");
    run.players.get_mut(&1).unwrap().hp = 0;
    assert_eq!(label(&run, true), 0, "a knight down reads nothing");
}

#[test]
fn a_cairn_waits_while_its_knight_lies_on_the_spot() {
    let mut run = party(2);
    let spot = quiet_spot(&run);
    let born = run.tick - 100;
    open_mut(&mut run).traces.push(Trace::Cairn { knight: 2, name: "WREN".into(), x: spot.0, y: spot.1, born });
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (spot.0 - 40.0, spot.1);
    }
    assert!(lit(&ground(&run, spot)) > 0);
    // Wren lies by it, bones and skull: the bones are the mark.
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.x, hero.y, hero.hp, hero.fell) = (spot.0 + 1.0, spot.1, 0, run.tick - 5);
    }
    let cairn_only = {
        let mut cv = ground(&run, spot);
        // The skull's call may be on; the cairn's stones are not.
        !has_ink(&cv, 'U') && !has_ink(&cv, 'v') && { cv.untouch(); true }
    };
    assert!(cairn_only, "no stones while the bones lie");
    // Stood up again, it shows.
    run.players.get_mut(&2).unwrap().hp = 6;
    assert!(has_ink(&ground(&run, spot), 'U'));
}

#[test]
fn a_knight_raised_in_a_borrowed_body_is_ringed_for_the_length_of_the_ward() {
    let mut run = party(2);
    let spot = quiet_spot(&run);
    for id in [1, 3] {
        (run.players.get_mut(&id).unwrap().x, run.players.get_mut(&id).unwrap().y) = (spot.0 - 40.0, spot.1);
    }
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.x, hero.y, hero.possessed) = (spot.0, spot.1, Some(EnemyKind::Goblin));
    }
    let ward = crate::rules::skulls::RAISED_WARD;
    let ring = |run: &mut Run, left: u32| {
        run.players.get_mut(&2).unwrap().invulnerable = left;
        lit(&ground(run, spot))
    };
    assert!(ring(&mut run, ward) > 0 && ring(&mut run, ward / 2) > 0 && ring(&mut run, 3) > 0, "ringed while warded");
    assert_eq!(ring(&mut run, 0), 0, "the ward spent, the ring is gone");
    assert_eq!(ring(&mut run, ward + 1000), 0, "a longer invulnerability is not the raising's");
    run.players.get_mut(&2).unwrap().possessed = None;
    assert_eq!(ring(&mut run, ward / 2), 0, "a knight in their own body is not ringed");
}

/// The first gate of the guardian's chamber, in tiles.
fn gate(run: &Run) -> (u16, u16) {
    let o = open(run);
    o.chambers[usize::from(o.guardian)].gates[0]
}

fn at_gate(run: &Run) -> (f32, f32) {
    let (c, r) = gate(run);
    ((f32::from(c) + 0.5) * crate::rules::TILE_UNITS, (f32::from(r) + 0.5) * crate::rules::TILE_UNITS)
}

#[test]
fn a_ward_lies_across_the_guardians_mouths_until_it_shuts_or_falls() {
    let mut run = party(2);
    let g = usize::from(open(&run).guardian);
    let spot = at_gate(&run);
    let arrival = open(&run).arrival;
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = arrival;
    }
    assert!(lit(&ground(&run, spot)) > 6, "a ward while the guardian lives");
    // A knight nearer wakes it: more of it, brighter.
    let far = lit(&ground(&run, spot));
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = (spot.0, spot.1 + 10.0);
    assert!(lit(&ground(&run, spot)) > far);
    assert!(has_ink(&ground(&run, spot), '7'));
    // Shut: the ward is gone once the slam is (the bars, where the pack's
    // art leaves them to be drawn here, stand in grey).
    open_mut(&mut run).chambers[g].sealed = true;
    open_mut(&mut run).chambers[g].sealed_at = run.tick - SLAM - DROP - 1;
    let shut = ground(&run, spot);
    assert!(!has_ink(&shut, '7') && !has_ink(&shut, '@') && !has_ink(&shut, '6'), "no ward on a shut mouth after the slam");
    assert_eq!(has_ink(&shut, 'U'), drawn_here(&run), "the bars' jambs stand where they are drawn here");
    // Won: the way is plain.
    open_mut(&mut run).chambers[g].sealed = false;
    open_mut(&mut run).chambers[g].cleared = true;
    assert_eq!(lit(&ground(&run, spot)), 0);
}

#[test]
fn the_doors_slam_is_lit_then_cools_then_goes_and_the_bars_close_from_the_ends() {
    let mut run = party(2);
    let g = usize::from(open(&run).guardian);
    let spot = at_gate(&run);
    open_mut(&mut run).chambers[g].sealed = true;
    let count = |run: &mut Run, age: u64| {
        open_mut(run).chambers[g].sealed_at = run.tick - age;
        lit(&ground(run, spot))
    };
    let rest = count(&mut run, SLAM + DROP + 5);
    assert_eq!(rest > 0, drawn_here(&run), "after the slam only the bars stand, and only where they are drawn here");
    assert!(count(&mut run, 0) > 0, "lit at once");
    open_mut(&mut run).chambers[g].sealed_at = run.tick;
    assert!(has_ink(&ground(&run, spot), '6'), "hot at the first tick");
    assert!(count(&mut run, 10) > rest, "dust still hangs");
    assert!(count(&mut run, SLAM + DROP - 1) >= rest);
    assert_eq!(count(&mut run, SLAM + DROP), rest, "gone at the end");
    // A doorway's bars come down from its ends to its middle.
    let gates = open(&run).chambers[g].gates.clone();
    assert!(gates.len() >= 3);
    let (end, middle) = (gates[0], gates[gates.len() / 2]);
    open_mut(&mut run).chambers[g].sealed_at = run.tick - 1;
    let (first_end, first_middle) = (tile_lit(&run, end), tile_lit(&run, middle));
    assert!(first_end > 0, "the end is down at once");
    if drawn_here(&run) {
        assert_eq!(first_middle, 0, "the middle waits");
        open_mut(&mut run).chambers[g].sealed_at = run.tick - DROP;
        assert!(tile_lit(&run, middle) > 0, "and comes down by the last");
    }
}

#[test]
fn a_chambers_braziers_are_cold_until_a_knight_comes_catch_in_turn_and_end_as_embers() {
    let mut ch = open(&party(2)).chambers[0].clone();
    // No one has been in: cold iron.
    ch.seen = false;
    assert!((0..4).all(|k| fire_of(&ch, k, 5000) == Fire::Cold));
    // A knight steps in at tick 1000: round from the north-west.
    ch.seen = true;
    ch.seen_at = 1000;
    assert_eq!(fire_of(&ch, 0, 999), Fire::Cold);
    assert!(matches!(fire_of(&ch, 0, 1002), Fire::Catching(_)));
    assert_eq!(fire_of(&ch, 3, 1002), Fire::Cold, "the last waits its turn");
    assert!(matches!(fire_of(&ch, 3, 1000 + 3 * KINDLE_STEP + 2), Fire::Catching(_)));
    assert_eq!(fire_of(&ch, 0, 1000 + FLARE), Fire::Blazing);
    assert_eq!(fire_of(&ch, 3, 1000 + 3 * KINDLE_STEP + FLARE + 400), Fire::Blazing, "and burns while the chamber is not won");
    // A chamber no one fought is cleared and burns on.
    ch.cleared = true;
    assert_eq!(fire_of(&ch, 1, 5000), Fire::Blazing);
    // Won, fought: they burn through the hush, then lie in coals.
    ch.stirred = true;
    ch.cleared_at = 2000;
    assert_eq!(fire_of(&ch, 1, 2000 + HUSH - 1), Fire::Blazing);
    assert_eq!(fire_of(&ch, 1, 2000 + HUSH), Fire::Embers);
    // A floor saved before the tick was kept: seen, they burn; won, coals.
    let mut old = ch.clone();
    (old.seen_at, old.cleared, old.cleared_at) = (0, false, 0);
    assert!((0..4).all(|k| fire_of(&old, k, 5) == Fire::Blazing));
    old.cleared = true;
    assert!((0..4).all(|k| fire_of(&old, k, 5) == Fire::Embers));
}

#[test]
fn braziers_are_drawn_where_the_packs_art_leaves_them_out_and_burn_in_their_state() {
    let mut run = party(2);
    if !drawn_here(&run) {
        return;
    }
    let o = open(&run).clone();
    let fight = (0..o.chambers.len()).find(|&i| o.chambers[i].kind == RoomKind::Fight).unwrap();
    let (x, y, _, h) = o.chambers[fight].rect;
    let corner = ((f32::from(x) + 0.5) * crate::rules::TILE_UNITS, (f32::from(y) + 0.5) * crate::rules::TILE_UNITS);
    let _ = h;
    let now = run.tick;
    let at_state = |run: &mut Run, seen: bool, seen_at: u64, cleared: bool| {
        let ch = &mut open_mut(run).chambers[fight];
        (ch.seen, ch.seen_at, ch.cleared, ch.cleared_at) = (seen, seen_at, cleared, 0);
        lit(&ground(run, corner))
    };
    let cold = at_state(&mut run, false, 0, false);
    assert!(cold > 10, "cold iron stands there: {cold}");
    let blazing = at_state(&mut run, true, now - 400, false);
    assert!(blazing > cold + 100, "lit, it throws its light: {blazing} over {cold}");
    open_mut(&mut run).chambers[fight].stirred = true;
    let embers = at_state(&mut run, true, now - 400, true);
    assert!(embers > cold && embers < blazing, "won, it lies in coals: {embers}");
    let catching = at_state(&mut run, true, now - 8, false);
    assert!(catching > cold, "catching: {catching}");
}

#[test]
fn a_won_chamber_hushes_then_the_light_settles() {
    let mut run = party(2);
    let o = open(&run).clone();
    let fight = (0..o.chambers.len()).find(|&i| o.chambers[i].kind == RoomKind::Fight).unwrap();
    let (cx, cy) = o.chambers[fight].centre();
    open_mut(&mut run).chambers[fight].cleared = true;
    // A chamber cleared before this existed (no tick kept) shows only what
    // always stands there.
    open_mut(&mut run).chambers[fight].cleared_at = 0;
    let rest = lit(&wide(&run, (cx, cy)));
    let mut counts = Vec::new();
    for age in [1u64, 12, 30, 50, HUSH - 1, HUSH, HUSH + 20] {
        open_mut(&mut run).chambers[fight].cleared_at = run.tick - age;
        counts.push((age, lit(&wide(&run, (cx, cy)))));
    }
    assert!(counts[..3].iter().all(|c| c.1 > rest), "{counts:?} over {rest}");
    assert_eq!(counts[5].1, rest, "gone at the end: {counts:?}");
    assert_eq!(counts[6].1, rest);
}

#[test]
fn a_skull_calls_from_the_floor_and_a_low_knight_beats() {
    let mut run = party(2);
    let spot = quiet_spot(&run);
    for id in 1..=3 {
        (run.players.get_mut(&id).unwrap().x, run.players.get_mut(&id).unwrap().y) = (spot.0 - 30.0, spot.1);
    }
    // Knight 2 down at the spot.
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.x, hero.y, hero.hp, hero.fell) = (spot.0, spot.1, 0, run.tick - 60);
    }
    let around = |run: &mut Run| {
        let mut on = 0;
        for k in 0..CALL {
            run.tick = 1000 + k;
            on += usize::from(lit(&ground(run, spot)) > 0);
        }
        on
    };
    let calling = around(&mut run);
    assert!(calling > 8 && calling < CALL as usize, "a ring now and then: {calling} of {CALL}");
    // A friend carries it: the call stops.
    run.players.get_mut(&1).unwrap().carrying_skull = Some(2);
    assert_eq!(around(&mut run), 0, "carried, it needs no call");
    run.players.get_mut(&1).unwrap().carrying_skull = None;
    // A wiped party calls no one.
    run.phase = Phase::Wiped;
    assert_eq!(around(&mut run), 0);
    run.phase = Phase::Exploring;
    // A knight standing low beats; one standing well does not.
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.hp, hero.max_hp, hero.fell) = (3, 12, 0);
    }
    let low = {
        let mut on = 0;
        for k in 0..BEAT {
            run.tick = 1000 + k;
            on += usize::from(lit(&ground(&run, spot)) > 0);
        }
        on
    };
    assert!(low >= 8 && low < BEAT as usize, "a double beat each {BEAT} ticks: {low}");
    run.players.get_mut(&2).unwrap().hp = 12;
    let well = (0..BEAT).map(|k| { run.tick = 1000 + k; lit(&ground(&run, spot)) }).sum::<usize>();
    assert_eq!(well, 0, "a knight in health draws nothing");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_chronicle_shots -- --ignored`:
/// the aftermath of a fight, a cairn, the ward at the guardian's mouth, the
/// doors' slam, a chamber's hush, a skull's call and a heartbeat.
#[test]
#[ignore]
fn write_chronicle_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba).unwrap().save(dir.join(format!("{name}.png"))).unwrap();
    };
    let (w, h) = (NATIVE_W, NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H);
    let depth: u32 = std::env::var("CHRONICLE_SHOT_DEPTH").ok().and_then(|d| d.parse().ok()).unwrap_or(2);
    let mut run = Run::new(17, 7, None);
    run.join_seat(2, "Wren");
    run.join_seat(3, "Bors");
    run.open_floor_for_test(depth);
    let o = open(&run).clone();
    let aim = BTreeMap::new();
    // A fight chamber won: its company down, the knights standing in it.
    let fight = (0..o.chambers.len()).find(|&i| o.chambers[i].kind == RoomKind::Fight).unwrap();
    let (cx, cy) = o.chambers[fight].centre();
    let spots = run.open_spots(o.map, (cx, cy + 8.0), 3);
    for (k, id) in [1u32, 2, 3].into_iter().enumerate() {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = spots[k];
        hero.invulnerable = 100_000;
    }
    run.step(&aim);
    for e in run.enemies.iter_mut().filter(|e| e.home == Some(fight as u8)) {
        e.hp = 0;
    }
    for _ in 0..4 {
        run.step(&aim);
    }
    save("chronicle-hush-early", &seat_scene(&run, w, h, Some(1)).compose(), 2);
    for _ in 0..24 {
        run.step(&aim);
    }
    save("chronicle-hush-late", &seat_scene(&run, w, h, Some(1)).compose(), 2);
    for _ in 0..80 {
        run.step(&aim);
    }
    save("chronicle-aftermath", &seat_scene(&run, w, h, Some(1)).compose(), 2);
    // A knight goes down by the remains; the others stand by.
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.invulnerable, hero.hp) = (0, 2);
    }
    run.hurt_for_test(2, 50);
    for _ in 0..40 {
        run.step(&aim);
    }
    save("chronicle-knight-down", &seat_scene(&run, w, h, Some(1)).compose(), 2);
    // Up again; a cairn stays and says whose it is.
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.hp, hero.max_hp) = (3, 12);
        let (x, y) = (hero.x + 7.0, hero.y);
        (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = (x, y);
    }
    for _ in 0..20 {
        run.step(&aim);
    }
    save("chronicle-cairn-and-heartbeat", &seat_scene(&run, w, h, Some(1)).compose(), 2);
    // A skull calls: Wren down, far from the others; then raised in a body.
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.hp, hero.max_hp) = (12, 12);
    }
    let lone = run.open_spots(o.map, (cx - 10.0, cy + 10.0), 1)[0];
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.x, hero.y, hero.invulnerable) = (lone.0, lone.1, 0);
    }
    run.hurt_for_test(2, 500);
    for _ in 0..70 {
        run.step(&aim);
    }
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = (lone.0 + 9.0, lone.1);
    save("chronicle-skull-call", &seat_scene(&run, w, h, Some(1)).compose(), 2);
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.hp, hero.fell, hero.possessed, hero.invulnerable) = (6, 0, Some(EnemyKind::Goblin), crate::rules::skulls::RAISED_WARD - 10);
    }
    save("chronicle-raised", &seat_scene(&run, w, h, Some(1)).compose(), 2);
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.possessed, hero.invulnerable, hero.hp) = (None, 0, 12);
    }
    // A chamber's braziers kindle as a knight first comes in.
    // One away from the map's edge, so the camera has the whole corner.
    let fight = (0..o.chambers.len())
        .find(|&i| o.chambers[i].kind == RoomKind::Fight && o.chambers[i].rect.0 > 30 && o.chambers[i].rect.1 > 24)
        .unwrap_or(fight);
    let (kx, ky, kw, kh) = o.chambers[fight].rect;
    let _ = (kw, kh);
    let corner = run.open_spots(o.map, ((f32::from(kx) + 6.0) * crate::rules::TILE_UNITS, (f32::from(ky) + 5.0) * crate::rules::TILE_UNITS), 3);
    for (k, id) in [1u32, 2, 3].into_iter().enumerate() {
        (run.players.get_mut(&id).unwrap().x, run.players.get_mut(&id).unwrap().y) = corner[k];
    }
    for (name, seen, ticks) in [("cold", false, 0u64), ("catching-8", true, 8), ("catching-20", true, 20), ("blazing", true, 400)] {
        let ch = &mut run.dungeon.open.as_mut().unwrap().chambers[fight];
        (ch.seen, ch.seen_at) = (seen, run.tick.saturating_sub(ticks));
        save(&format!("chronicle-braziers-{name}"), &seat_scene(&run, w, h, Some(1)).compose(), 2);
    }
    // The ward at the guardian's mouth.
    let g = usize::from(o.guardian);
    let (gc, gr) = o.chambers[g].gates[0];
    let (gx, gy) = ((f32::from(gc) + 0.5) * crate::rules::TILE_UNITS, (f32::from(gr) + 0.5) * crate::rules::TILE_UNITS);
    let (x0, y0, w0, h0) = o.chambers[g].rect;
    let outward = if gr == y0 { (0.0, -5.0) } else if gr == y0 + h0 - 1 { (0.0, 5.0) } else if gc == x0 { (-5.0, 0.0) } else { (5.0, 0.0) };
    let _ = w0;
    let near = run.open_spots(o.map, (gx + outward.0, gy + outward.1), 3);
    for (k, id) in [1u32, 2, 3].into_iter().enumerate() {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.hp, hero.max_hp) = (near[k].0, near[k].1, 12, 12);
    }
    run.step(&aim);
    save("chronicle-ward", &seat_scene(&run, w, h, Some(1)).compose(), 2);
    // Step in: the doors slam.
    let inward = (-outward.0, -outward.1);
    let inside = run.open_spots(o.map, (gx + inward.0 * 0.8, gy + inward.1 * 0.8), 1)[0];
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = inside;
    run.step(&aim);
    save("chronicle-slam-1", &seat_scene(&run, w, h, Some(1)).compose(), 2);
    for (n, ticks) in [(2, 3), (3, 6), (4, 12)] {
        for _ in 0..ticks {
            run.step(&aim);
        }
        save(&format!("chronicle-slam-{n}"), &seat_scene(&run, w, h, Some(1)).compose(), 2);
    }
}
