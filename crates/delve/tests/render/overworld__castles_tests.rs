use super::*;
use super::super::map::SCREEN_H;
use crate::world::houses;

#[test]
fn every_castle_stands_on_march_meadow_with_a_road_at_its_gate() {
    let realm = super::super::map::Realm::get();
    let mut taken = std::collections::BTreeSet::new();
    for id in houses::ids() {
        let (tx, ty, tw, th) = footprint(id);
        assert!(
            ty / SCREEN_H >= super::super::map::HEARTLAND_Y * 2,
            "{:?} is not in the March",
            id
        );
        for y in ty..ty + th {
            for x in tx..tx + tw {
                assert_eq!(realm.at(x, y), b'.', "{id:?} stands on {x},{y}");
                assert!(taken.insert((x, y)), "castles overlap at {x},{y}");
            }
        }
        let (sx, sy) = stand(id);
        assert_eq!(realm.at(sx, sy), b'=', "{id:?} gate has no road");
    }
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
        let field = super::super::ink::ink(h.field).unwrap();
        let charge = super::super::ink::ink(h.charge).unwrap();
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
            .any(|c| c == super::super::ink::ink(h.field).unwrap())
    );
    assert_eq!(
        dress(&super::super::kit::knight(), None).rgb_bytes(),
        super::super::kit::knight().rgb_bytes(),
        "the Keep's own knight keeps the red"
    );
}

