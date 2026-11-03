//! The garage: its catalogue, a knight's build and what it adds up to, the
//! ledger that keeps builds by name (and an old save), the shop room's
//! swaps, and that a frame is the same frame every time.

use super::ledger::{self, Ledger, SwapError};
use super::meshes::{self, RacerMeshes};
use super::parts::{self, PARTS, Slot};
use super::racer::{Build, KIT, Stats};
use super::room::{self, Event, SEATS, Shop, UNIT};
use super::scene::{Look, View};
use super::shop;
use super::store::Store;
use crate::render::world3d::mesh::Mesh;
use crate::rules::Input;
use std::collections::{BTreeMap, BTreeSet};

fn press(id: u32, input: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, input)])
}

fn e() -> Input {
    Input { interact: true, ..Default::default() }
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("garage-{}-{}-{}", name, std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Put a seat at a rack cell, as the host's walking would bring it.
fn stand(shop: &mut Shop, id: u32, slot: Slot, cell: usize) {
    let (x, y) = shop::station(slot).stand_at(cell);
    let s = shop.seats.iter_mut().find(|s| s.id == id).unwrap();
    s.x = (x * UNIT as f32) as i32;
    s.y = (y * UNIT as f32) as i32;
}

// ------------------------------------------------------------ the catalogue

#[test]
fn every_slot_has_parts_and_the_engine_has_a_choice_of_four_to_six() {
    for s in Slot::ALL {
        assert!(parts::count(s) >= 1, "{s:?} has no parts");
    }
    assert!((4..=6).contains(&parts::count(Slot::Engine)));
    let ids: BTreeSet<_> = PARTS.iter().map(|p| p.id).collect();
    assert_eq!(ids.len(), PARTS.len(), "part ids are unique");
    for p in PARTS {
        assert_eq!(parts::part(p.id).map(|q| q.id), Some(p.id));
        assert!(!p.name.is_empty() && !p.blurb.is_empty());
    }
}

#[test]
fn the_four_example_racers_are_the_starting_kit() {
    assert_eq!(KIT, ["Excalibur", "Bastion", "Wraith", "Crown"]);
    for i in 0..4 {
        let b = Build::kit(i);
        assert!(b.is_sound(), "kit {i} fits the catalogue");
        for p in b.worn() {
            assert!(p.kit, "{} is worn by a kit racer, so it is kit", p.id);
        }
    }
    // each starting racer has its own engine
    let engines: BTreeSet<_> = (0..4).map(|i| Build::kit(i).engine).collect();
    assert_eq!(engines.len(), 4);
    // and the kit is exactly what the four wear
    let worn: BTreeSet<_> = (0..4).flat_map(|i| Build::kit(i).worn().map(|p| p.id)).collect();
    let kit: BTreeSet<_> = PARTS.iter().filter(|p| p.kit).map(|p| p.id).collect();
    assert_eq!(worn.len(), kit.len(), "every kit part is worn by a kit racer: {worn:?} vs {kit:?}");
}

#[test]
fn a_new_knight_starts_in_one_of_the_four_and_always_the_same_one() {
    let a = Build::starting("Sir Kay");
    assert_eq!(a, Build::starting("  sir kay "), "the name is trimmed and case-blind");
    assert!((0..4).any(|i| a == Build::kit(i)));
    let spread: BTreeSet<_> = ["Sir Kay", "Bors", "Lady Enid", "Percival", "Dame Gwen", "Galahad", "Tristan", "Mordred"].iter().map(|n| Build::starting(n).engine).collect();
    assert!(spread.len() >= 3, "a roomful of knights are not all handed the same racer: {spread:?}");
}

// ------------------------------------------------------------ stats

#[test]
fn a_racers_stats_are_the_base_plus_its_parts_with_the_weight_taken_off() {
    // base 30/30/30/10/30; Gryphon + Lancer + hover + triple organ add
    // speed 23, accel 11, handling 14, armour 2, weight 28: 8 over neutral
    // takes 1 off speed (a fifth), 4 off accel (a half), 2 off handling (a third)
    assert_eq!(Build::kit(0).stats(), Stats { speed: 52, accel: 37, handling: 42, armour: 12, weight: 58 });
}

#[test]
fn heavy_armour_buys_defence_at_the_cost_of_pace() {
    let mut b = Build::kit(0);
    let light = b.stats();
    assert!(b.fit("bastion_plate"));
    let heavy = b.stats();
    assert!(heavy.armour > light.armour);
    assert!(heavy.weight > light.weight);
    assert!(heavy.accel < light.accel && heavy.speed < light.speed && heavy.handling < light.handling);
    let d = heavy.delta(&light);
    assert_eq!((d.armour, d.weight), (20, 20));
}

#[test]
fn stats_stay_between_zero_and_a_hundred_whatever_is_fitted() {
    for engine in parts::options(Slot::Engine) {
        for armour in parts::options(Slot::Armour) {
            let mut b = Build::kit(1);
            b.fit(engine.id);
            b.fit(armour.id);
            b.fit("iron_treads");
            let s = b.stats();
            for (_, v) in s.bars() {
                assert!((0..=100).contains(&v), "{engine:?} {armour:?}: {s:?}");
            }
        }
    }
}

// ------------------------------------------------------------ builds and the ledger

#[test]
fn a_swap_changes_one_slot_and_says_what_it_did() {
    let mut l = Ledger::new();
    let before = l.build("Sir Kay");
    let s = l.swap("Sir Kay", "hellhound_twin").unwrap();
    assert_eq!(s.slot, Slot::Engine);
    assert_eq!(s.to.id, "hellhound_twin");
    assert_eq!(s.from.id, before.engine);
    assert_eq!(s.before, before.stats());
    let after = l.build("Sir Kay");
    assert_eq!(after.stats(), s.after);
    assert_eq!(before.differs(&after), vec![Slot::Engine]);
    assert_eq!(l.swap("Sir Kay", "hellhound_twin"), Err(SwapError::Already));
    assert_eq!(l.swap("Sir Kay", "nothing_of_the_kind"), Err(SwapError::Unknown));
    // and back
    l.swap("Sir Kay", &before.engine).unwrap();
    assert_eq!(l.build("Sir Kay"), before);
}

#[test]
fn a_build_is_kept_by_the_name_however_it_is_written() {
    let mut l = Ledger::new();
    l.swap("Sir Kay", "black_sun").unwrap();
    assert_eq!(l.build("  SIR KAY ").engine, "black_sun");
    assert!(l.knows("sir kay"));
    assert!(!l.knows("Bors"));
    assert_eq!(l.knights.len(), 1);
    assert_eq!(l.knights["sir kay"].name, "Sir Kay");
    assert_eq!(l.knights["sir kay"].swaps, 1);
}

#[test]
fn a_swap_marks_the_ledger_for_saving_and_saving_clears_it() {
    let dir = temp("dirty");
    let store = Store::new(dir.join("garage.json"));
    let mut l = Ledger::new();
    assert!(!l.dirty());
    assert!(!store.save_if_dirty(&mut l).unwrap(), "nothing to write yet");
    l.swap("Bors", "wraith_needle").unwrap();
    assert!(l.dirty());
    assert!(store.save_if_dirty(&mut l).unwrap());
    assert!(!l.dirty());
    assert!(!store.save_if_dirty(&mut l).unwrap());
}

#[test]
fn the_ledger_is_forgetful_only_past_its_limit_and_forgets_the_oldest_swap() {
    let mut l = Ledger::new();
    for i in 0..ledger::MAX_KNIGHTS + 5 {
        l.swap(&format!("knight {i}"), "black_sun").unwrap();
    }
    assert_eq!(l.knights.len(), ledger::MAX_KNIGHTS);
    assert!(!l.knows("knight 0"), "the first to swap is the first forgotten");
    assert!(l.knows(&format!("knight {}", ledger::MAX_KNIGHTS + 4)));
}

// ------------------------------------------------------------ saving and loading

#[test]
fn a_ledger_survives_a_restart_through_its_file() {
    let dir = temp("restart");
    let store = Store::in_workspace(&dir);
    let mut l = Ledger::new();
    l.swap("Sir Kay", "hellhound_twin").unwrap();
    l.swap("Sir Kay", "violet_dusk").unwrap();
    l.swap("Dame Ysolde", "scale_mail").unwrap();
    store.save(&mut l).unwrap();
    assert!(dir.join(".angel/dungeon/garage.json").exists());
    let leftovers: Vec<_> = std::fs::read_dir(dir.join(".angel/dungeon")).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(leftovers.len(), 1, "no half-written file is left beside it: {leftovers:?}");
    // a new host reads it back
    let back = Store::in_workspace(&dir).load().unwrap();
    assert_eq!(back, l);
    assert_eq!(back.build("sir kay").engine, "hellhound_twin");
    assert_eq!(back.build("sir kay").paint, "violet_dusk");
    assert_eq!(back.build("Dame Ysolde").armour, "scale_mail");
}

#[test]
fn a_new_game_starts_the_knights_in_the_racers_they_built() {
    let dir = temp("games");
    let store = Store::in_workspace(&dir);
    {
        // one game: Sir Kay takes an engine at the rack
        let mut ledger = store.load().unwrap();
        let mut shop = Shop::new();
        shop.join(1, "Sir Kay", &ledger);
        stand(&mut shop, 1, Slot::Engine, 5);
        shop.step(&BTreeMap::new(), &mut ledger);
        let events = shop.step(&press(1, e()), &mut ledger);
        assert!(matches!(events.as_slice(), [Event::Swapped { seat: 1, .. }]));
        store.save_if_dirty(&mut ledger).unwrap();
    }
    // another game, another host process: a fresh shop, the ledger from disk
    let ledger = Store::in_workspace(&dir).load().unwrap();
    let mut shop = Shop::new();
    shop.join(7, "sir kay", &ledger);
    assert_eq!(shop.seat(7).unwrap().build.part(Slot::Engine).id, parts::nth(Slot::Engine, 5).unwrap().id);
}

#[test]
fn an_empty_workspace_has_an_empty_ledger() {
    let dir = temp("empty");
    let l = Store::in_workspace(&dir).load().unwrap();
    assert!(l.knights.is_empty());
}

#[test]
fn an_old_save_with_fewer_slots_and_fields_still_loads() {
    // written before the crest, armour, paint and trim slots, the swap count,
    // the version and the sequence number existed
    let old = r#"{"knights":{"sir kay":{"name":"Sir Kay","build":{"engine":"wraith_needle","chassis":"wraith_skiff","running":"hover_discs","exhaust":"low_gutter"}}}}"#;
    let l = Ledger::from_json(old).unwrap();
    let b = l.build("Sir Kay");
    assert_eq!(b.engine, "wraith_needle");
    assert_eq!(b.exhaust, "low_gutter");
    assert!(b.is_sound());
    let kit = Build::kit(0);
    assert_eq!((b.crest.as_str(), b.armour.as_str(), b.paint.as_str(), b.trim.as_str()), (kit.crest.as_str(), kit.armour.as_str(), kit.paint.as_str(), kit.trim.as_str()));
    assert_eq!(l.knights["sir kay"].swaps, 0);
    assert_eq!(l.v, 1);
}

#[test]
fn a_save_naming_parts_the_catalogue_has_dropped_loads_with_the_defaults_in_their_place() {
    let odd = r#"{"v":1,"seq":9,"knights":{"bors":{"name":"Bors","swaps":3,"seq":9,"build":{"engine":"a_part_long_gone","chassis":"black_iron","paint":"banner_red"}}}}"#;
    let l = Ledger::from_json(odd).unwrap();
    let b = l.build("Bors");
    assert!(b.is_sound(), "{b:?}");
    assert_eq!(b.engine, Build::kit(0).engine, "an unknown engine becomes the default's");
    assert_eq!(b.chassis, Build::kit(0).chassis, "a paint in the chassis slot does not fit it");
    assert_eq!(b.paint, "banner_red", "what is understood is kept");
}

#[test]
fn a_save_from_a_newer_garage_with_fields_this_one_does_not_know_loads() {
    let newer = r#"{"v":9,"seq":4,"future":{"a":1},"knights":{"x":{"name":"X","swaps":1,"seq":4,"colour":"green","build":{"engine":"black_sun","wings":"large"}}}}"#;
    let l = Ledger::from_json(newer).unwrap();
    assert_eq!(l.build("X").engine, "black_sun");
    assert_eq!(l.v, 9, "the newer number is kept, not rewritten down");
}

#[test]
fn a_ledger_that_cannot_be_read_is_set_aside_not_destroyed() {
    let dir = temp("corrupt");
    let file = dir.join("garage.json");
    std::fs::write(&file, b"{ this is not json").unwrap();
    let store = Store::new(&file);
    assert!(store.load().is_err());
    assert!(!file.exists());
    assert!(dir.join("garage.json.bad").exists(), "the bad file is kept beside it");
    assert!(store.load().unwrap().knights.is_empty(), "and the garage starts empty");
}

#[test]
fn the_same_ledger_is_the_same_bytes() {
    let mut a = Ledger::new();
    let mut b = Ledger::new();
    for l in [&mut a, &mut b] {
        l.swap("Sir Kay", "black_sun").unwrap();
        l.swap("Bors", "war_wheels").unwrap();
        l.swap("Sir Kay", "gilt_trim").unwrap();
    }
    assert_eq!(a.to_json(), b.to_json());
    assert_eq!(Ledger::from_json(&a.to_json()).unwrap(), a);
}

// ------------------------------------------------------------ the shop room

#[test]
fn knights_are_given_the_middle_lifts_first_and_the_shop_holds_six() {
    let l = Ledger::new();
    let mut s = Shop::new();
    let bays: Vec<_> = (1..=6).map(|i| s.join(i, &format!("K{i}"), &l).unwrap()).collect();
    assert_eq!(bays, shop::BAY_ORDER);
    assert_eq!(s.join(7, "Late", &l), None, "one lift to a knight");
    assert_eq!(SEATS, 6);
    assert_eq!(s.join(3, "Again", &l), None, "a seat is in once");
    assert!(s.leave(2));
    assert_eq!(s.join(8, "Next", &l), Some(shop::BAY_ORDER[1]), "a freed lift is given out again");
}

#[test]
fn e_at_a_rack_puts_that_part_on_the_knights_own_racer_and_writes_it_down() {
    let mut l = Ledger::new();
    let mut s = Shop::new();
    s.join(1, "Sir Kay", &l);
    s.join(2, "Bors", &l);
    let bors_before = s.seat(2).unwrap().build.clone();
    for (slot, cell) in [(Slot::Engine, 4), (Slot::Chassis, 2), (Slot::Running, 3), (Slot::Exhaust, 1), (Slot::Crest, 3), (Slot::Armour, 2), (Slot::Paint, 5), (Slot::Trim, 4)] {
        stand(&mut s, 1, slot, cell);
        s.step(&BTreeMap::new(), &mut l);
        let want = parts::nth(slot, cell).unwrap();
        assert_eq!(s.seat(1).unwrap().pick.map(|p| p.part.id), Some(want.id), "{slot:?} {cell}: the knight is at the part");
        let ev = s.step(&press(1, e()), &mut l);
        assert!(matches!(ev.as_slice(), [Event::Swapped { seat: 1, swap }] if swap.to.id == want.id), "{slot:?}: {ev:?}");
        assert_eq!(s.seat(1).unwrap().build.id(slot), want.id);
        assert_eq!(l.build("Sir Kay").id(slot), want.id, "the ledger has it on the swap");
        s.step(&BTreeMap::new(), &mut l);
    }
    assert_eq!(s.seat(2).unwrap().build, bors_before, "nobody else's racer changed");
}

#[test]
fn e_held_swaps_once_and_a_part_already_fitted_is_not_swapped_again() {
    let mut l = Ledger::new();
    let mut s = Shop::new();
    s.join(1, "Sir Kay", &l);
    stand(&mut s, 1, Slot::Engine, 5);
    let a = s.step(&press(1, e()), &mut l);
    assert_eq!(a.len(), 1);
    assert!(s.step(&press(1, e()), &mut l).is_empty(), "held: no second swap");
    s.step(&BTreeMap::new(), &mut l);
    let again = s.step(&press(1, e()), &mut l);
    assert!(matches!(again.as_slice(), [Event::Already { seat: 1, slot: Slot::Engine }]));
    assert_eq!(l.knights["sir kay"].swaps, 1);
}

#[test]
fn e_away_from_the_racks_does_nothing() {
    let mut l = Ledger::new();
    let mut s = Shop::new();
    s.join(1, "Sir Kay", &l);
    let (x, y) = shop::bay_door(shop::BAY_ORDER[0]);
    let (px, py) = s.seat(1).unwrap().pos();
    assert!((px - x).abs() < 0.02 && (py - y).abs() < 0.02);
    assert!(s.step(&press(1, e()), &mut l).is_empty());
    assert!(!l.dirty());
}

#[test]
fn f_puts_the_last_swap_back_and_again_puts_it_forward() {
    let mut l = Ledger::new();
    let mut s = Shop::new();
    s.join(1, "Sir Kay", &l);
    let first = s.seat(1).unwrap().build.clone();
    stand(&mut s, 1, Slot::Engine, 5);
    s.step(&press(1, e()), &mut l);
    let swapped = s.seat(1).unwrap().build.clone();
    assert_ne!(first, swapped);
    let f = Input { fire: true, ..Default::default() };
    s.step(&BTreeMap::new(), &mut l);
    s.step(&press(1, f), &mut l);
    assert_eq!(s.seat(1).unwrap().build, first);
    assert_eq!(l.build("Sir Kay"), first, "the undo is saved too");
    s.step(&BTreeMap::new(), &mut l);
    s.step(&press(1, f), &mut l);
    assert_eq!(s.seat(1).unwrap().build, swapped);
}

#[test]
fn the_number_keys_fit_the_starting_racers() {
    let mut l = Ledger::new();
    let mut s = Shop::new();
    s.join(1, "Sir Kay", &l);
    for k in 1..=4u8 {
        s.step(&BTreeMap::new(), &mut l);
        s.step(&press(1, Input { cast: k, ..Default::default() }), &mut l);
        assert_eq!(s.seat(1).unwrap().build, Build::kit(usize::from(k) - 1));
    }
}

#[test]
fn a_swap_plays_for_a_second_and_a_bit_and_the_stat_change_stays_up_longer() {
    let mut l = Ledger::new();
    let mut s = Shop::new();
    s.join(1, "Sir Kay", &l);
    stand(&mut s, 1, Slot::Engine, 5);
    s.step(&press(1, e()), &mut l);
    assert!(s.seat(1).unwrap().swap.is_some());
    let d = s.seat(1).unwrap().shown.unwrap().0;
    assert!(!d.is_zero());
    for _ in 0..room::SWAP_TICKS {
        s.step(&BTreeMap::new(), &mut l);
    }
    assert!(s.seat(1).unwrap().swap.is_none(), "the lift is done");
    assert!(s.seat(1).unwrap().shown.is_some(), "the numbers are still up");
    for _ in 0..room::DELTA_TICKS {
        s.step(&BTreeMap::new(), &mut l);
    }
    assert!(s.seat(1).unwrap().shown.is_none());
}

#[test]
fn knights_walk_with_the_arrows_and_stop_at_walls_lifts_and_racks() {
    let mut l = Ledger::new();
    let mut s = Shop::new();
    s.join(1, "Sir Kay", &l);
    let start = s.seat(1).unwrap().pos();
    let right = Input { move_x: 1, ..Default::default() };
    for _ in 0..20 {
        s.step(&press(1, right), &mut l);
    }
    let moved = s.seat(1).unwrap().pos();
    assert!(moved.0 > start.0 + 2.0, "twenty ticks of right is a few paces: {start:?} {moved:?}");
    // up (move_y -1) is away from the camera, into the shop, to the lift
    let up = Input { move_y: -1, ..Default::default() };
    for _ in 0..200 {
        s.step(&press(1, up), &mut l);
    }
    let (x, y) = s.seat(1).unwrap().pos();
    for (x0, y0, x1, y1) in shop::obstacles() {
        assert!(!(x > *x0 && x < *x1 && y > *y0 && y < *y1), "never inside {x0},{y0},{x1},{y1}: at {x},{y}");
    }
    assert!(y < shop::HY && x.abs() < shop::HX);
    // every seat's walk is the same walk
    let mut s2 = Shop::new();
    let mut l2 = Ledger::new();
    s2.join(1, "Sir Kay", &l2);
    for _ in 0..20 {
        s2.step(&press(1, right), &mut l2);
    }
    for _ in 0..200 {
        s2.step(&press(1, up), &mut l2);
    }
    assert_eq!(s2.seat(1).unwrap().pos(), s.seat(1).unwrap().pos());
}

#[test]
fn every_part_of_every_rack_can_be_stood_at_and_picked() {
    let mut l = Ledger::new();
    let mut s = Shop::new();
    s.join(1, "Sir Kay", &l);
    for st in shop::stations() {
        for i in 0..st.n {
            stand(&mut s, 1, st.slot, i);
            s.step(&BTreeMap::new(), &mut l);
            let (x, y) = s.seat(1).unwrap().pos();
            assert!(x.abs() < shop::HX && y > 0.0 && y < shop::HY, "{:?} {i} is in the shop", st.slot);
            for (x0, y0, x1, y1) in shop::obstacles() {
                assert!(!(x > *x0 && x < *x1 && y > *y0 && y < *y1), "{:?} {i}: a knight at {x},{y} would be inside a thing", st.slot);
            }
            let p = s.seat(1).unwrap().pick.unwrap_or_else(|| panic!("{:?} {i}: nothing to pick", st.slot));
            assert_eq!((p.slot, p.index), (st.slot, i), "the cell the knight stands at is the cell picked");
        }
    }
}

#[test]
fn the_racks_fit_the_walls_and_do_not_overlap() {
    let sts = shop::stations();
    assert_eq!(sts.len(), Slot::ALL.len());
    for s in sts {
        assert_eq!(s.n, parts::count(s.slot), "a cell for every part");
        let (ex, ey) = (s.origin.0 + s.along.0 * s.len(), s.origin.1 + s.along.1 * s.len());
        for (x, y) in [s.origin, (ex, ey)] {
            assert!(x.abs() <= shop::HX + 0.01 && (0.0..=shop::HY + 0.01).contains(&y), "{:?} ends at {x},{y}", s.slot);
        }
    }
    for (i, a) in sts.iter().enumerate() {
        for b in &sts[i + 1..] {
            let same_line = a.along == b.along && (a.origin.1 - b.origin.1).abs() < 0.01 && a.along.0 != 0.0 || a.along == b.along && (a.origin.0 - b.origin.0).abs() < 0.01 && a.along.1 != 0.0;
            if same_line {
                let t = |s: &super::shop::Station| {
                    let u = s.origin.0 * s.along.0 + s.origin.1 * s.along.1;
                    (u, u + s.len())
                };
                let (ta, tb) = (t(a), t(b));
                assert!(ta.1 <= tb.0 + 0.001 || tb.1 <= ta.0 + 0.001, "{:?} and {:?} overlap on their wall", a.slot, b.slot);
            }
        }
    }
}

#[test]
fn the_same_inputs_make_the_same_room() {
    let run = || {
        let mut l = Ledger::new();
        let mut s = Shop::new();
        for (i, n) in ["Sir Kay", "Bors", "Lady Enid"].iter().enumerate() {
            s.join(i as u32 + 1, n, &l);
        }
        let mut events = vec![];
        for t in 0..300u32 {
            let mut inp = BTreeMap::new();
            inp.insert(1, Input { move_x: if t % 90 < 45 { 1 } else { -1 }, move_y: -((t / 40 % 2) as i8), interact: t % 37 == 0, ..Default::default() });
            inp.insert(2, Input { move_y: 1, interact: t % 50 == 0, fire: t % 71 == 0, ..Default::default() });
            events.extend(s.step(&inp, &mut l));
        }
        (s.seats.iter().map(|k| (k.x, k.y, k.build.clone())).collect::<Vec<_>>(), events, l.to_json())
    };
    assert_eq!(run(), run());
}

// ------------------------------------------------------------ meshes: the hook the race reads

fn finite(m: &Mesh) -> bool {
    m.tris.iter().all(|t| t.v.iter().all(|p| p.x.is_finite() && p.y.is_finite() && p.z.is_finite()) && t.normal.x.is_finite())
}

#[test]
fn every_part_has_a_mesh() {
    for p in PARTS {
        let mut m = Mesh::new();
        meshes::display(p, 0.0, &mut m);
        assert!(m.tris.len() > 6, "{} has a mesh", p.id);
        assert!(finite(&m), "{} is finite", p.id);
        let (lo, hi) = super::prim::bounds(&m).unwrap();
        assert!(lo.z >= -0.01, "{} stands on its shelf", p.id);
        assert!(hi.z - lo.z < 8.0 && hi.x - lo.x < 9.0, "{} is a sane size", p.id);
    }
}

#[test]
fn a_racer_is_a_mesh_the_race_can_draw_and_it_follows_the_build() {
    let mut a = Mesh::new();
    let mut b = Mesh::new();
    meshes::racer(&Build::kit(0), 0.0, &mut a);
    assert!(a.tris.len() > 150 && finite(&a));
    let (lo, hi) = super::prim::bounds(&a).unwrap();
    assert!(lo.z >= -0.01 && lo.z < 0.4, "the racer stands on the floor: {lo:?}");
    assert!(((lo.x + hi.x) / 2.0).abs() < 0.01, "and is centred along its length");
    let mut other = Build::kit(0);
    other.fit("black_sun");
    other.fit("war_wheels");
    meshes::racer(&other, 0.0, &mut b);
    assert_ne!(a.tris.len(), b.tris.len(), "a different build is a different racer");
    // the same build is the same mesh
    let mut c = Mesh::new();
    meshes::racer(&Build::kit(0), 0.0, &mut c);
    assert_eq!(a.tris.len(), c.tris.len());
    assert!(a.tris.iter().zip(&c.tris).all(|(x, y)| x.v == y.v && x.mat == y.mat));
    // and the race's materials: the paint and trim of the build
    let paint = meshes::Ink::of(&Build::kit(1));
    let mut m = Mesh::new();
    meshes::racer(&Build::kit(1), 0.0, &mut m);
    assert!(m.tris.iter().any(|t| t.mat == paint.paint) && m.tris.iter().any(|t| t.mat == paint.trim));
}

#[test]
fn each_engine_has_its_own_silhouette() {
    let mut sizes = BTreeSet::new();
    for e in parts::options(Slot::Engine) {
        let mut b = Build::kit(0);
        b.fit(e.id);
        let mut parts = RacerMeshes::new();
        meshes::build_parts(&b, 0.0, &mut parts);
        let (lo, hi) = super::prim::bounds(&parts.slot[Slot::Engine.index()]).unwrap();
        let d = hi - lo;
        sizes.insert(((d.x * 10.0) as i32, (d.y * 10.0) as i32, (d.z * 10.0) as i32));
    }
    assert_eq!(sizes.len(), parts::count(Slot::Engine), "no two engines are the same shape: {sizes:?}");
}

#[test]
fn the_race_takes_each_knights_racer_stats_and_body_from_the_ledger() {
    let mut l = Ledger::new();
    l.swap("Sir Kay", "black_sun").unwrap();
    let a = l.entrant(" sir kay");
    assert_eq!(a.build.engine, "black_sun");
    assert_eq!(a.stats, a.build.stats());
    let mut m = Mesh::new();
    a.mesh(0.0, &mut m);
    assert!(m.tris.len() > 150 && finite(&m));
    let other = l.entrant("Bors");
    assert_ne!(other.build, a.build, "each knight has their own racer");
}

// ------------------------------------------------------------ a frame

fn busy() -> (Shop, Ledger) {
    let mut l = Ledger::new();
    let mut s = Shop::new();
    for (i, n) in ["Sir Kay", "Bors", "Lady Enid", "Percival"].iter().enumerate() {
        s.join(i as u32 + 1, n, &l);
    }
    stand(&mut s, 1, Slot::Engine, 3);
    stand(&mut s, 2, Slot::Paint, 2);
    s.step(&BTreeMap::new(), &mut l);
    s.step(&press(1, e()), &mut l);
    for _ in 0..16 {
        s.step(&BTreeMap::new(), &mut l);
    }
    (s, l)
}

#[test]
fn a_frame_is_drawn_lit_and_is_the_same_frame_every_time() {
    let (s, _) = busy();
    let mut a = View::new(160, 90);
    let mut b = View::new(160, 90);
    a.draw(&s, Some(1));
    b.draw(&s, Some(1));
    assert_eq!(a.pixels(), b.pixels());
    let lit = a.pixels().iter().filter(|p| p.iter().any(|&c| c > 40)).count();
    assert!(lit > 160 * 90 / 8, "a shop is lit in pools, not black: {lit} lit pixels");
    // drawn again into the same buffers it is still the same frame
    a.draw(&s, Some(1));
    assert_eq!(a.pixels(), b.pixels());
    // the arcade finish is its own, and also steady
    a.look = Look::Arcade;
    b.look = Look::Arcade;
    a.draw(&s, Some(1));
    b.draw(&s, Some(1));
    assert_eq!(a.pixels(), b.pixels());
}

#[test]
fn a_swap_changes_the_picture_and_the_empty_shop_has_no_knights_in_it() {
    let (mut s, mut l) = busy();
    let mut v = View::new(160, 90);
    v.draw(&s, Some(1));
    let mid = v.pixels().to_vec();
    for _ in 0..40 {
        s.step(&BTreeMap::new(), &mut l);
    }
    v.draw(&s, Some(1));
    assert_ne!(mid, v.pixels(), "the swap, and its lamp and sparks, are in the frame");
    let empty = Shop::new();
    v.draw(&empty, None);
    assert_eq!(v.movers(), 0, "nothing moves in an empty shop");
}

#[test]
fn drawing_a_frame_does_not_grow_its_buffers() {
    let (s, _) = busy();
    let mut v = View::new(160, 90);
    v.draw(&s, Some(1));
    v.draw(&s, Some(1));
    let n = v.movers();
    for _ in 0..5 {
        v.draw(&s, Some(1));
    }
    assert_eq!(v.movers(), n, "each frame starts from empty buffers");
}
