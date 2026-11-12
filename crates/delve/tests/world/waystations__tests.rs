use super::*;
use crate::rules::{RoomKind, Tile};
use crate::world::kingdom;
use std::collections::VecDeque;

fn chebyshev(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

#[test]
fn the_halts_stand_where_the_search_finds_them() {
    let found: Vec<(usize, (i32, i32), (i32, i32))> = search().iter().map(|p| (p.stop, (p.rect.tx, p.rect.ty), p.road)).collect();
    let table: Vec<(usize, (i32, i32), (i32, i32))> = placed().iter().map(|p| (p.stop, (p.rect.tx, p.rect.ty), p.road)).collect();
    assert_eq!(found, table, "the map moved a halt: put what the search found in SPOTS");
}

#[test]
fn every_stop_of_the_ring_has_its_halt_in_ring_order() {
    let all = placed();
    assert_eq!(all.len(), STOPS.len(), "a stop found no ground: {:?}", all.iter().map(|p| p.stop).collect::<Vec<_>>());
    for (i, p) in all.iter().enumerate() {
        assert_eq!(p.stop, i);
        assert_eq!(onward(back(i)), i);
    }
    // A name is read on the signs and the keeper's board: no two alike.
    let mut names: Vec<&str> = STOPS.iter().map(|s| s.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), STOPS.len());
}

#[test]
fn a_halt_stands_on_open_ground_beside_a_road_it_can_be_walked_to() {
    let geo = kingdom::geography(&[]);
    let keep = crate::world::kingdom::keep_stand();
    // Every road tile the Keep's roads reach.
    let n = (crate::world::map::MAP_W * crate::world::map::MAP_H) as usize;
    let idx = |(x, y): (i32, i32)| (y * crate::world::map::MAP_W + x) as usize;
    let mut seen = vec![false; n];
    let mut queue = VecDeque::from([keep]);
    seen[idx(keep)] = true;
    while let Some((x, y)) = queue.pop_front() {
        for next in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if kingdom::is_road(&geo, next) && !seen[idx(next)] {
                seen[idx(next)] = true;
                queue.push_back(next);
            }
        }
    }
    let mut bad = Vec::new();
    for p in placed() {
        let name = STOPS[p.stop].name;
        for y in p.rect.ty..p.rect.ty + H {
            for x in p.rect.tx..p.rect.tx + W {
                if geo.room.tile(x, y) != Tile::Floor {
                    bad.push(format!("{name}: ({x},{y}) is not open ground"));
                }
                if kingdom::is_road(&geo, (x, y)) {
                    bad.push(format!("{name}: ({x},{y}) is road"));
                }
            }
        }
        for spot in [p.keeper, p.coach, p.rally] {
            if !p.rect.contains(spot.0, spot.1) {
                bad.push(format!("{name}: {spot:?} is off its ground"));
            }
        }
        if !kingdom::is_road(&geo, p.road) {
            bad.push(format!("{name}: marks no road"));
        }
        if chebyshev(p.keeper, p.road) > 6 {
            bad.push(format!("{name} stands {} tiles off its road", chebyshev(p.keeper, p.road)));
        }
        if !seen[idx(p.road)] {
            bad.push(format!("{name}: the Keep's roads do not reach {:?}", p.road));
        }
        let (ax, ay) = p.apron();
        if geo.room.tile(ax as i32, ay as i32) != Tile::Floor {
            bad.push(format!("{name}: its apron is not open"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_halt_keeps_off_every_door_lot_sign_and_thing_of_the_kingdom() {
    let geo = kingdom::geography(&[]);
    let stones: Vec<(i32, i32)> = crate::rules::classes::stones(&geo).into_iter().map(|(_, p)| p).collect();
    let things: Vec<(i32, i32)> = crate::rules::trinkets::things(&geo).into_iter().map(|(_, p)| p).collect();
    let mut bad = Vec::new();
    for (i, p) in placed().iter().enumerate() {
        let name = STOPS[p.stop].name;
        let grown = Lot { tx: p.rect.tx - 2, ty: p.rect.ty - 2, tw: W + 4, th: H + 4 };
        for s in &geo.sites {
            if chebyshev(s.stand, p.keeper) < 6 {
                bad.push(format!("{name} stands by {}'s door", s.name));
            }
            if let Some(lot) = s.lot.filter(|l| grown.overlaps(l)) {
                bad.push(format!("{name} stands on {}'s lot {lot:?}", s.name));
            }
        }
        for post in crate::world::signs::posts() {
            if grown.contains(post.at.0, post.at.1) {
                bad.push(format!("{name} stands on a fingerpost at {:?}", post.at));
            }
        }
        for t in stones.iter().chain(&things) {
            if grown.contains(t.0, t.1) {
                bad.push(format!("{name} crowds a banner or a thing at {t:?}"));
            }
        }
        for other in &placed()[i + 1..] {
            if chebyshev(p.rect.tx_ty(), other.rect.tx_ty()) < SPACING {
                bad.push(format!("{name} crowds {}", STOPS[other.stop].name));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
    assert_eq!(geo.sites.iter().filter(|s| s.room == RoomKind::Gate).count(), 1);
}

trait TxTy {
    fn tx_ty(&self) -> (i32, i32);
}
impl TxTy for Lot {
    fn tx_ty(&self) -> (i32, i32) {
        (self.tx, self.ty)
    }
}
