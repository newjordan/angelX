use super::*;
use crate::world::biome::{self, Biome};
use crate::world::houses;

#[test]
fn every_castle_stands_on_march_meadow_with_a_road_at_its_gate() {
    let realm = super::super::map::Realm::get();
    let geo = crate::world::kingdom::geography(&[]);
    let mut taken = std::collections::BTreeSet::new();
    for id in houses::ids() {
        let (tx, ty, tw, th) = footprint(id);
        assert_eq!(biome::at(tx, ty), Biome::March, "{id:?} is not in the March");
        for y in ty..ty + th {
            for x in tx..tx + tw {
                assert_eq!(realm.at(x, y), b'.', "{id:?} stands on {x},{y}");
                assert!(taken.insert((x, y)), "castles overlap at {x},{y}");
            }
        }
        let (sx, sy) = stand(id);
        assert!(crate::world::kingdom::is_road(&geo, (sx, sy)), "{id:?} gate has no road");
    }
    // Off the grid: never two castles side by side, and never a row of
    // them at one height.
    let sites: Vec<(i32, i32, i32, i32)> = houses::ids().map(footprint).collect();
    for (i, a) in sites.iter().enumerate() {
        for b in &sites[i + 1..] {
            assert!((a.0 - b.0).abs().max((a.1 - b.1).abs()) >= 12, "{a:?} crowds {b:?}");
        }
    }
    let heights: std::collections::BTreeSet<i32> = sites.iter().map(|s| s.1).collect();
    assert!(heights.len() >= 6, "castles stand in rows: {heights:?}");
}

#[test]
fn an_idle_castle_flies_nothing_and_a_serving_one_flies_its_banner() {
    for id in houses::ids() {
        let h = houses::get(id);
        let idle = sprite(h, false, 0);
        assert!(
            idle.pixels().all(|c| !super::super::ink::is_signal(c)),
            "{} idle castle shows live state",
            h.key
        );
        let up = sprite(h, true, 0);
        // The house's colours fly at dusk: plain dyes, never signal inks.
        let field = super::super::ink::ink(dusk_ink(h.field)).unwrap();
        let charge = super::super::ink::ink(dusk_ink(h.charge)).unwrap();
        assert!(!super::super::ink::is_signal(field), "{} banner field glows", h.key);
        assert!(!super::super::ink::is_signal(charge), "{} banner charge glows", h.key);
        assert!(up.pixels().any(|c| c == field), "{} banner field", h.key);
        assert!(up.pixels().any(|c| c == charge), "{} banner charge", h.key);
    }
}

#[test]
fn the_knight_wears_his_houses_colours() {
    let kimi = houses::by_key("kimi").unwrap();
    let h = houses::get(kimi);
    let dressed = dress(&super::super::kit::knight(), Some(kimi));
    let red = super::super::ink::ink('7').unwrap();
    assert!(dressed.pixels().all(|c| c != red || h.field == '7'));
    assert!(
        dressed
            .pixels()
            .any(|c| c == super::super::ink::ink(dusk_ink(h.field)).unwrap())
    );
    assert_eq!(
        dress(&super::super::kit::knight(), None).rgb_bytes(),
        super::super::kit::knight().rgb_bytes(),
        "the Keep's own knight keeps the red"
    );
}

