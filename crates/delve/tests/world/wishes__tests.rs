//! The wish lots' fast search answers what the tile-by-tile one did.
use super::*;

/// A small deterministic stream, so the cases are the same every run.
struct Dice(u64);
impl Dice {
    fn next(&mut self, below: i32) -> i32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as i32).rem_euclid(below)
    }
}

#[test]
fn the_fast_search_finds_the_lot_the_tile_by_tile_search_did() {
    let mut dice = Dice(0x5eed);
    for case in 0..40 {
        let (tw, th) = (1 + dice.next(8), 2 + dice.next(7));
        let goal = (dice.next(MAP_W), dice.next(MAP_H));
        // Lots in the way: a few, then a crowd round the goal.
        let crowd = if case % 2 == 0 { 3 } else { 60 };
        let mut taken: Vec<Lot> = reserved();
        for _ in 0..crowd {
            let near = case % 3 == 0;
            let (x, y) = if near { (goal.0 - 14 + dice.next(28), goal.1 - 10 + dice.next(20)) } else { (dice.next(MAP_W), dice.next(MAP_H)) };
            taken.push(Lot { tx: x, ty: y, tw: 1 + dice.next(6), th: 2 + dice.next(5) });
        }
        for margin in [1, 0] {
            for nearest in [false, true] {
                assert_eq!(
                    find_in(tw, th, goal, &taken, margin, nearest),
                    find_in_slow(tw, th, goal, &taken, margin, nearest),
                    "case {case}: {tw}x{th} near {goal:?}, {crowd} lots, margin {margin}, nearest {nearest}",
                );
            }
        }
    }
}

#[test]
fn a_realm_full_of_lots_still_finds_ground_or_says_it_has_none() {
    // Every start taken: both searches say none.
    let all = vec![Lot { tx: 0, ty: 0, tw: MAP_W, th: MAP_H }];
    assert_eq!(find_in(4, 5, (10, 10), &all, 0, false), None);
    assert_eq!(find_in_slow(4, 5, (10, 10), &all, 0, false), None);
}

#[test]
fn forty_wishes_are_placed_in_the_time_one_used_to_take() {
    let wishes: Vec<Wish> = (0..40)
        .map(|i| Wish {
            id: format!("w{i}"),
            name: format!("Wish {i}"),
            words: String::new(),
            by: String::new(),
            status: crate::realm::Status::Built,
            price: Default::default(),
            art: vec!["#".repeat(32); 32],
            near: String::new(),
            paid: Vec::new(),
            raised: i as u32,
            lot: None,
        })
        .collect();
    let began = std::time::Instant::now();
    let lots = lots(&wishes);
    assert!(lots.iter().all(Option::is_some), "every wish stands");
    // Tile by tile this took about two seconds at this crate's test opt-level;
    // a fifth of that leaves the room a loaded machine needs.
    assert!(began.elapsed() < std::time::Duration::from_millis(400), "took {:?}", began.elapsed());
}
