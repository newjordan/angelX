use super::*;

fn chunk_of(at: (i32, i32)) -> (i32, i32) {
    (at.0 / CHUNK_W, at.1 / CHUNK_H)
}

#[test]
fn every_thing_stands_on_open_ground_clear_of_roads_lots_and_doors() {
    let geo = kingdom::geography(&[]);
    let d = get();
    assert!(d.pieces.len() > 200, "{} things", d.pieces.len());
    for p in &d.pieces {
        let (x, y) = p.at;
        let reach = p.item.reach();
        for tx in x - reach..=x + reach {
            assert_eq!(geo.room.tile(tx, y), Tile::Floor, "{p:?} stands in the way");
            assert!(p.item.stands_on(Realm::get().at(tx, y)), "{p:?} on {}", Realm::get().at(tx, y) as char);
            assert!(!in_set_piece(tx, y), "{p:?} in a set piece");
        }
        for dy in -2..=2 {
            for dx in -2..=2 {
                assert!(!kingdom::is_road(&geo, (x + dx, y + dy)), "{p:?} within two tiles of a road");
            }
        }
        for site in &geo.sites {
            assert!(chebyshev(site.stand, p.at) > 2, "{p:?} crowds the door of {}", site.name);
            if let Some(l) = site.lot {
                assert!(!(l.tx - 2..l.tx + l.tw + 2).contains(&x) || !(l.ty - 2..l.ty + l.th + 2).contains(&y), "{p:?} on the lot of {}", site.name);
            }
        }
        for (tx, ty, tw, th) in super::super::castles::footprints() {
            assert!(!(tx - 2..tx + tw + 2).contains(&x) || !(ty - 2..ty + th + 2).contains(&y), "{p:?} at a castle");
        }
    }
}

#[test]
fn every_chunk_has_something_to_walk_to_and_none_is_crowded() {
    let d = get();
    let geo = kingdom::geography(&[]);
    let mut bare = Vec::new();
    for cy in 0..CHUNKS_Y {
        for cx in 0..CHUNKS_X {
            let (x0, y0) = (cx * CHUNK_W, cy * CHUNK_H);
            let within = |(x, y): (i32, i32)| (x0..x0 + CHUNK_W).contains(&x) && (y0..y0 + CHUNK_H).contains(&y);
            let camps = d.camps.iter().filter(|c| within(c.at)).count();
            let things = d.pieces.iter().filter(|p| within(p.at)).count();
            let place = set_pieces().any(|(_, o)| within(o))
                || geo.sites.iter().any(|s| s.lot.is_some() && within(s.stand))
                || super::super::houses::footprints().any(|(tx, ty, _, _)| within((tx, ty)));
            assert!(things <= 12, "chunk ({cx}, {cy}) holds {things} things");
            assert!(camps <= 4, "chunk ({cx}, {cy}) holds {camps} camps");
            if camps == 0 && !place {
                bare.push((cx, cy, biome::chunk(cx, cy)));
            }
        }
    }
    assert!(bare.is_empty(), "nothing to walk to in {bare:?}");
}

#[test]
fn footpaths_run_from_a_road_to_a_camp_over_open_ground() {
    let geo = kingdom::geography(&[]);
    let d = get();
    let trodden = d.trails.iter().filter(|&&t| t).count();
    assert!(trodden > 200, "{trodden} trail tiles");
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if !d.trail(x, y) {
                continue;
            }
            assert_eq!(geo.room.tile(x, y), Tile::Floor, "trail at ({x}, {y}) stands in the way");
            assert!(!kingdom::is_road(&geo, (x, y)), "trail at ({x}, {y}) on a road");
            assert!(matches!(Realm::get().at(x, y), b'.' | b'a' | b's'), "trail on {}", Realm::get().at(x, y) as char);
        }
    }
    // Every wild camp with a trail: the trail is one piece that touches a
    // road at one end.
    let wild: Vec<&Camp> = d.camps.iter().filter(|c| c.class == Class::Wild).collect();
    let mut linked = 0;
    for c in &wild {
        if !d.trail(c.at.0, c.at.1) {
            continue;
        }
        linked += 1;
        let mut seen = std::collections::HashSet::from([c.at]);
        let mut stack = vec![c.at];
        let mut at_road = false;
        while let Some((x, y)) = stack.pop() {
            for n in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if kingdom::is_road(&geo, n) {
                    at_road = true;
                } else if d.trail(n.0, n.1) && seen.insert(n) {
                    stack.push(n);
                }
            }
        }
        assert!(at_road, "the trail from {:?} never reaches a road", c.at);
    }
    assert!(linked * 10 >= wild.len() * 6, "{linked} of {} wild camps have a trail", wild.len());
}

#[test]
fn wild_camps_keep_apart_and_are_places_a_find_may_stand() {
    let d = get();
    let spots = super::super::finds::spots();
    let wild: Vec<&Camp> = d.camps.iter().filter(|c| c.class == Class::Wild).collect();
    for (i, a) in wild.iter().enumerate() {
        assert!(spots.contains(&a.at), "{a:?} is no place for a find");
        for b in &wild[i + 1..] {
            assert!(chebyshev(a.at, b.at) >= SPACING, "{a:?} crowds {b:?}");
        }
    }
    assert!(wild.len() >= 60, "{} wild camps", wild.len());
}

#[test]
fn the_dressing_is_the_same_every_time() {
    let again = build(&kingdom::geography(&[]));
    let d = get();
    assert_eq!(again.pieces, d.pieces);
    assert_eq!(again.camps, d.camps);
    assert_eq!(again.trails, d.trails);
    let _ = chunk_of((0, 0));
}

/// Walking the open kingdom, a screen's worth of country round any spot a
/// knight can stand on holds something to walk to: a cluster, a place, a
/// castle's wall or a fingerpost.
#[test]
fn nearly_every_screen_of_the_open_country_has_something_in_it() {
    let geo = kingdom::geography(&[]);
    let d = get();
    let mut pois: Vec<(i32, i32)> = d.pieces.iter().map(|p| p.at).collect();
    pois.extend(geo.sites.iter().map(|s| s.stand));
    pois.extend(set_pieces().map(|(_, (ox, oy))| (ox + SCREEN_W / 2, oy + SCREEN_H / 2)));
    pois.extend(super::super::signs::posts().iter().map(|p| p.at));
    // A castle is never seen whole: any of its wall in view counts.
    let castles: Vec<(i32, i32, i32, i32)> = super::super::castles::footprints().collect();
    let (mut total, mut empty) = (0, Vec::new());
    for wy in (8..MAP_H - 8).step_by(4) {
        for wx in (8..MAP_W - 8).step_by(4) {
            if geo.room.tile(wx, wy) != Tile::Floor {
                continue;
            }
            total += 1;
            // The kingdom's view is 24 x 14 tiles: ask for something in the
            // 21 x 13 round the spot.
            let walls = castles.iter().any(|&(x, y, w, h)| x <= wx + 10 && x + w > wx - 10 && y <= wy + 6 && y + h > wy - 6);
            if !walls && !pois.iter().any(|&(x, y)| (x - wx).abs() <= 10 && (y - wy).abs() <= 6) {
                empty.push((wx, wy, biome::at(wx, wy)));
            }
        }
    }
    assert!(empty.len() * 100 <= total, "{} of {total} screens are bare: {empty:?}", empty.len());
}

/// A screen is never crowded, and never says one thing over and over:
/// no view's worth of the country holds more than nine things, nor more than
/// four of one kind.
#[test]
fn no_screen_is_crowded_or_repeats_itself() {
    let d = get();
    let (mut most, mut same) = ((0, (0, 0)), (0, (0, 0), Item::Cairn));
    for wy in 7..MAP_H - 7 {
        for wx in 12..MAP_W - 12 {
            let seen: Vec<&Piece> = d.pieces.iter().filter(|p| (p.at.0 - wx).abs() <= 12 && (p.at.1 - wy).abs() <= 7).collect();
            if seen.len() > most.0 {
                most = (seen.len(), (wx, wy));
            }
            for &item in &Item::ALL {
                let n = seen.iter().filter(|p| p.item == item).count();
                if n > same.0 {
                    same = (n, (wx, wy), item);
                }
            }
        }
    }
    assert!(most.0 <= CROWD, "{} things in the view at {:?}", most.0, most.1);
    assert!(same.0 <= SAME, "{} of {:?} in the view at {:?}", same.0, same.2, same.1);
}
