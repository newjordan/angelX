//! The sites: how far the Keep has risen, who set what, and the ground
//! under it. These are pure (no map is stood up); the run's own ticking at
//! the site needs the blank-slate plan standing and is tested in the world
//! group (`ANGEL_REALM_PLAN=tabula`).

use super::*;
use crate::world::works::{GRID, ORIGIN};

fn tower() -> &'static Castle {
    works::castle(works::KEEP).expect("the great tower")
}

#[test]
fn a_run_starts_with_the_keep_partway_up_and_an_old_save_gets_the_same() {
    let sites = Sites::default();
    let keep = sites.keep().expect("the Keep's site");
    assert_eq!((keep.plan.as_str(), keep.laid), ("great-tower", START));
    assert!(sites.valid() && sites.is_default());
    // Partway: the first lift is done and the second is going up.
    let c = tower();
    let lift = |n: usize| c.stages.iter().filter(|s| s.count > 0).nth(n).expect("a lift");
    assert!(START >= lift(0).first + lift(0).count && START < lift(1).first + lift(1).count);
    // A run saved before sites has no such field: it loads with the Keep partway up.
    let run = Run::at_home(7, 1, None, Default::default(), Default::default());
    let mut saved = serde_json::to_value(&run).expect("a run writes");
    saved.as_object_mut().expect("a map").remove("sites");
    let loaded: Run = serde_json::from_value(saved).expect("an old save reads");
    assert_eq!(loaded.sites, Sites::default());
    // And a run whose Keep has risen carries it through a save.
    let mut run = run;
    run.sites.keep_mut().expect("site").lay(120, 2);
    let back: Run = serde_json::from_value(serde_json::to_value(&run).expect("writes")).expect("reads");
    assert_eq!(back.sites, run.sites);
    assert_eq!(back.sites.keep().expect("site").laid, START + 120);
}

#[test]
fn who_set_the_stones_is_kept_run_length_and_tallied() {
    let mut site = Site::keep();
    site.lay(100, 1);
    site.lay(50, 1);
    site.lay(30, 2);
    site.lay(10, 1);
    assert_eq!(site.laid, START + 190);
    // Runs of one seat merge; a change of seat is a new run.
    assert_eq!(site.marks, vec![(START, 0), (START + 150, 1), (START + 180, 2), (START + 190, 1)]);
    assert_eq!(site.setter(0), 0);
    assert_eq!(site.setter(START), 1);
    assert_eq!(site.setter(START + 149), 1);
    assert_eq!(site.setter(START + 150), 2);
    assert_eq!(site.setter(START + 189), 1);
    // Not yet set: no one's.
    assert_eq!(site.setter(START + 190), 0);
    assert_eq!(site.tally(), vec![(0, START), (1, 160), (2, 30)]);
    assert!(site.valid());
}

#[test]
fn the_castle_is_never_over_laid_and_a_bad_site_is_mended() {
    let mut site = Site::keep();
    site.lay(1_000_000, 1);
    assert_eq!(site.laid, tower().len());
    assert!(site.valid());
    let mut bad = Site { plan: "great-tower".into(), laid: u32::MAX, marks: vec![(50, 1), (40, 2), (60, 200), (9_999_999, 1)], trues: vec![(70, 60), (0, 9_000_000)], lost: vec![5_000, 3_000, 99_999_999], bands: Vec::new() };
    assert!(!bad.valid());
    bad.mend();
    assert!(bad.valid(), "{bad:?}");
    assert_eq!(bad.laid, tower().len());
    let mut sites = Sites((0..20).map(|_| Site::keep()).collect());
    assert!(!sites.valid());
    sites.mend();
    assert!(sites.valid());
}

#[test]
fn a_true_stone_is_kept_in_runs_and_a_stone_set_by_hand_carries_the_setters_mark() {
    let mut site = Site::keep();
    // A hired stone, a true one by seat 2, another true one beside it, a rough one by seat 1.
    site.lay_set(0, false);
    site.lay_set(2, true);
    site.lay_set(2, true);
    site.lay_set(1, false);
    assert_eq!(site.laid, START + 4);
    assert_eq!(site.trues, vec![(START + 1, START + 3)], "the two true stones are one run");
    assert!(!site.is_true(START) && site.is_true(START + 1) && site.is_true(START + 2) && !site.is_true(START + 3));
    assert!(!site.is_true(START + 4), "not set yet");
    assert_eq!(site.true_count(), 2);
    assert_eq!(site.setter(START + 1), 2);
    assert_eq!(site.setter(START + 3), 1);
    assert!(site.valid());
    // Saved and read back whole; a site that was never set true writes none.
    let back: Site = serde_json::from_value(serde_json::to_value(&site).expect("writes")).expect("reads");
    assert_eq!(back, site);
    assert!(serde_json::to_value(Site::keep()).expect("writes").get("trues").is_none());
    // An old save of a site has none.
    let mut old = serde_json::to_value(&site).expect("writes");
    old.as_object_mut().expect("a map").remove("trues");
    let loaded: Site = serde_json::from_value(old).expect("an old save reads");
    assert!(loaded.trues.is_empty() && loaded.laid == site.laid);
    // Set back to fewer stones, the true ones above go.
    let mut run = Run::at_home(7, 1, None, Default::default(), Default::default());
    run.sites.keep_mut().expect("site").lay_set(3, true);
    run.lay_to("great-tower", START);
    assert!(run.sites.keep().expect("site").trues.is_empty());
    // Past the castle's last stone nothing is set.
    let mut done = Site::keep();
    done.lay(1_000_000, 0);
    done.lay_set(1, true);
    assert_eq!(done.laid, tower().len());
    assert_eq!(done.true_count(), 0);
}

#[test]
fn the_map_holds_the_site_shut_and_the_works_opens_the_bailey_and_shuts_what_is_laid() {
    use crate::world::works::NEVER;
    let c = tower();
    let (cols, rows) = (240usize, 176usize);
    let mut room = Room::kingdom(vec![Tile::Wall; cols * rows], cols, rows);
    let gate = crate::world::plan::tabula::DELVE_GATE;
    // The Delve's gate is closed ground of its own and the patch leaves it so.
    let mut sites = Sites::default();
    assert!(patch_room(&mut room, &sites));
    assert!(!patch_room(&mut room, &sites), "patched is patched");
    let (sx, sy) = crate::world::plan::tabula::KEEP_STAND;
    assert_eq!(room.tile(sx, sy), Tile::Floor, "the spawn is open");
    assert_eq!(room.tile(gate.0, gate.1), Tile::Wall, "the Delve's gate is its own");
    assert_eq!(room.tile(gate.0 + 1, gate.1 + 1), Tile::Wall);
    let (stand_x, stand_y) = crate::world::plan::tabula::delve_stand();
    assert_eq!(room.tile(stand_x, stand_y), Tile::Floor, "its stand is open");
    // The ditch is a pit; the first course shuts a tile; nothing outside is touched.
    let ring = (ORIGIN.0 + 12, ORIGIN.1);
    assert_eq!(room.tile(ring.0, ring.1), Tile::Hazard);
    let (x0, y0, ..) = c.tiles();
    assert_eq!(room.tile(x0 - 1, y0), Tile::Wall, "off the site is the map's");
    // Finished, more is shut and the gate still opens.
    sites.keep_mut().expect("site").lay(1_000_000, 1);
    assert!(patch_room(&mut room, &sites));
    let shut = (0..GRID).flat_map(|j| (0..GRID).map(move |i| (x0 + i, y0 + j))).filter(|&(x, y)| room.tile(x, y) != Tile::Floor).count();
    assert!(shut > 400, "{shut}");
    for ty in ORIGIN.1 + 8..ORIGIN.1 + 14 {
        assert_eq!(room.tile(ORIGIN.0, ty), Tile::Floor, "the gate's way is open at {ty}");
        assert_eq!(room.tile(ORIGIN.0 - 1, ty), Tile::Floor);
    }
    // Set back to fewer stones, tiles open again.
    sites.keep_mut().expect("site").laid = 0;
    assert!(patch_room(&mut room, &sites));
    let in_gate = |x: i32, y: i32| x >= gate.0 && y >= gate.1 && x < gate.0 + gate.2 && y < gate.1 + gate.3;
    let still = (0..GRID).flat_map(|j| (0..GRID).map(move |i| (x0 + i, y0 + j))).filter(|&(x, y)| !in_gate(x, y) && c.need(x, y) != NEVER && c.need(x, y) > 0 && room.tile(x, y) == Tile::Wall).count();
    assert_eq!(still, 0);
}
