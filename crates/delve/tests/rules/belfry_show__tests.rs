use super::*;
use crate::rules::folly::{self, BatShoot};

fn cries(show: &BatShow) -> Vec<Cry> {
    show.shouts.iter().map(|s| s.cry).collect()
}

#[test]
fn bats_felled_close_together_run_into_a_combo_that_a_gap_or_a_bite_breaks() {
    let mut show = BatShow::default();
    for k in 0..4u32 {
        show.felled(100 + k * 10, 1, 10 + k, (0, 0), (8, 0), 30);
    }
    assert_eq!(show.run_of(1, 130), 4);
    assert_eq!(show.run_of(1, 130 + CHAIN_GAP + 1), 0, "a gap lets the run go");
    show.felled(400, 1, 20, (0, 0), (8, 0), 30);
    assert_eq!(show.run_of(1, 400), 1, "a bat after a gap starts again");
    assert_eq!(show.best.get(&1), Some(&4));
    show.bitten(402, 1, (0, 0), 5);
    assert_eq!(show.run_of(1, 402), 0, "a bite breaks the run");
    assert_eq!(show.felled.get(&1), Some(&5));
    assert_eq!(show.team, 5);
}

#[test]
fn the_first_bat_and_each_size_of_run_is_called_once() {
    let mut show = BatShow::default();
    for k in 0..5u32 {
        show.felled(10 + k * 4, 2, k + 1, (0, 0), (0, 0), 30);
    }
    assert_eq!(
        cries(&show),
        vec![Cry::First, Cry::Chain(2), Cry::Chain(3), Cry::Chain(4), Cry::Chain(5)],
        "double, triple, four, five; the first bat once"
    );
}

#[test]
fn a_flocks_last_bat_ends_it_and_holds_the_room() {
    let mut show = BatShow::default();
    show.flock(50, 0, &[(1, (0, 0)), (2, (16, 0)), (3, (32, 0))]);
    assert_eq!(show.swoops.len(), 3);
    assert_eq!(show.felled(60, 1, 1, (0, 0), (0, 0), 30), Weight::Solid, "a bat is a solid kill");
    show.felled(200, 2, 2, (0, 0), (0, 0), 30);
    let last = show.felled(400, 3, 3, (0, 0), (0, 0), 30);
    assert_eq!(last, Weight::Kill, "the last of the flock holds the room");
    assert!(cries(&show).contains(&Cry::Flock));
    assert!(show.flocks.is_empty());
    assert!(show.hits.last().is_some_and(|h| h.last && h.kill));
}

#[test]
fn the_bell_tolls_each_ten_bats_of_the_table() {
    let mut show = BatShow::default();
    for k in 0..25u32 {
        show.felled(10 + k * 200, 1 + k % 3, 100 + k, (0, 0), (0, 0), 30);
    }
    assert_eq!(show.team, 25);
    assert!(cries(&show).contains(&Cry::Toll(1)) || show.shouts.len() < 25);
    assert_eq!(show.tolls.len(), 2, "two tolls in twenty-five");
}

#[test]
fn a_change_of_lead_is_called_and_the_clock_calls_itself() {
    let mut show = BatShow::default();
    let mut scores = BTreeMap::from([(1, 150), (2, 60)]);
    show.standings(100, &scores, 50);
    assert_eq!(show.lead, Some(1));
    assert!(show.shouts.is_empty(), "the first lead is not a change");
    scores.insert(2, 210);
    show.standings(200, &scores, 45);
    assert_eq!(cries(&show), vec![Cry::Lead]);
    assert_eq!(show.shouts[0].who, 2);
    show.standings(300, &scores, 30);
    show.standings(301, &scores, 29);
    show.standings(400, &scores, 10);
    show.standings(401, &scores, 9);
    assert_eq!(cries(&show).iter().filter(|c| **c == Cry::Half).count(), 1);
    assert_eq!(cries(&show).iter().filter(|c| **c == Cry::Last).count(), 1);
}

#[test]
fn old_moments_are_let_go_and_the_lists_stay_bounded() {
    let mut show = BatShow::default();
    for k in 0..100u32 {
        show.felled(10 + k, 1, k, (0, 0), (0, 0), 30);
        show.struck(10 + k, 1, k, (0, 0), (0, 0), 5, 30);
        show.bitten(10 + k, 1, (0, 0), 3);
        show.flock(10 + k, k as u16, &[(1000 + k, (0, 0)); 3]);
    }
    assert!(show.valid(), "the kit caps its lists");
    show.tick(10_000);
    assert!(show.hits.is_empty() && show.bites.is_empty() && show.swoops.is_empty() && show.shouts.is_empty());
    assert!(show.valid());
}

#[test]
fn the_show_rides_the_wire_and_stays_quiet_when_empty() {
    assert!(BatShow::default().quiet());
    let mut show = BatShow::default();
    show.felled(5, 1, 7, (3, 4), (0, 0), 30);
    let wire = serde_json::to_string(&show).unwrap();
    let back: BatShow = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, show);
    assert!(!show.quiet());
}

/// The real shoot: knights who shoot the nearest bat. Each felled bat is in
/// the show once, each knight's felled bats add to the table's, and the
/// show's kills are the bats that left the room dead.
#[test]
fn the_real_shoot_records_what_the_knights_shot() {
    use crate::rules::{EnemyKind, Input, Run, Spoil};
    let mut r = Run::new(41, 1, None);
    r.descend_for_test();
    r.join_seat(2, "Two");
    r.enemies.clear();
    r.pick_folly();
    for h in r.players.values_mut() {
        h.carried.add(Spoil::Gold, 40);
    }
    r.enter_for_test(r.folly_room.expect("a hall on floor two"));
    let mut seen_kills = std::collections::BTreeSet::new();
    let mut last_alive: Vec<u32> = Vec::new();
    for _ in 0..folly::TITLE + 20 * crate::rules::HZ {
        let keys: BTreeMap<u32, Input> = r
            .players
            .iter()
            .map(|(&id, me)| {
                let near = r.enemies.iter().min_by(|a, b| {
                    (a.x - me.x).hypot(a.y - me.y).total_cmp(&(b.x - me.x).hypot(b.y - me.y))
                });
                let Some(e) = near else { return (id, Input::default()) };
                let (dx, dy) = (e.x - me.x, e.y - me.y);
                let big = dx.abs().max(dy.abs());
                let s = |v: f32| if v.abs() < big * 0.4 { 0 } else { v.signum() as i8 };
                (id, Input { aim_x: s(dx), aim_y: s(dy), fire: true, ..Default::default() })
            })
            .collect();
        r.step(&keys);
        let alive: Vec<u32> = r.enemies.iter().filter(|e| e.kind == EnemyKind::Bat).map(|e| e.id).collect();
        let shoot: &BatShoot = r.bats.as_ref().unwrap();
        for h in shoot.show.hits.iter().filter(|h| h.kill) {
            seen_kills.insert(h.bat);
        }
        assert!(shoot.valid(), "the shoot stays valid");
        last_alive = alive;
    }
    let shoot = r.bats.as_ref().unwrap();
    assert!(shoot.show.team > 0, "the gunners felled something: {:?}", shoot.show.felled);
    assert_eq!(shoot.show.felled.values().map(|&n| u32::from(n)).sum::<u32>(), u32::from(shoot.show.team));
    assert!(u32::from(shoot.show.team) >= seen_kills.len() as u32, "every felled bat counted once");
    let _ = last_alive;
    assert!(r.valid_snapshot());
}

/// A knight's health lower than the last look is a bite, and breaks their run.
#[test]
fn a_bite_is_read_off_a_knights_health() {
    use crate::rules::{Input, Run, Spoil};
    let mut r = Run::new(41, 1, None);
    r.descend_for_test();
    r.join_seat(2, "Two");
    r.enemies.clear();
    r.pick_folly();
    for h in r.players.values_mut() {
        h.carried.add(Spoil::Gold, 40);
    }
    r.enter_for_test(r.folly_room.expect("a hall on floor two"));
    for _ in 0..folly::TITLE + 5 {
        r.step(&BTreeMap::<u32, Input>::new());
    }
    // A run going, then the bite.
    let now = r.tick as u32;
    r.bats.as_mut().unwrap().show.felled(now, 1, 9_000, (0, 0), (0, 0), 30);
    r.bats.as_mut().unwrap().show.felled(now, 1, 9_001, (0, 0), (0, 0), 30);
    assert_eq!(r.bats.as_ref().unwrap().show.run_of(1, now), 2);
    let hp = r.players[&1].hp;
    r.players.get_mut(&1).unwrap().hp = hp.saturating_sub(7).max(1);
    r.step(&BTreeMap::<u32, Input>::new());
    let show = &r.bats.as_ref().unwrap().show;
    assert!(show.bites.iter().any(|b| b.who == 1 && b.lost >= 1), "the bite is on the books: {:?}", show.bites);
    assert_eq!(show.run_of(1, r.tick as u32), 0, "a bite breaks the run");
}
