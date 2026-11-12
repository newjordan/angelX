//! The Powder Room's show: calls, runs, chains and the sooting's blame.
use super::*;

#[test]
fn the_show_starts_quiet() {
    assert!(PowderShow::default().quiet());
    assert!(PowderShow::default().valid());
}

fn blast_at(show: &mut PowderShow, now: u32, who: u32, at: (i32, i32), crates: &[(i32, i32)], depth: u8) -> Weight {
    show.blast(now, who, at, [2; 4], crates.to_vec(), depth, 40)
}

#[test]
fn a_keg_that_breaks_nothing_is_a_graze_and_crates_and_links_make_it_heavier() {
    assert_eq!(weigh(0, 0, 0, false), Weight::Graze);
    assert_eq!(weigh(1, 0, 0, false), Weight::Solid);
    assert_eq!(weigh(2, 0, 0, false), Weight::Crushing);
    assert_eq!(weigh(0, 1, 0, false), Weight::Crushing);
    assert_eq!(weigh(1, 2, 0, false), Weight::Kill);
    assert_eq!(weigh(4, 0, 0, false), Weight::Kill);
    assert_eq!(weigh(0, 0, 1, false), Weight::Kill);
    assert_eq!(weigh(0, 0, 0, true), Weight::Finale);
}

#[test]
fn the_first_blast_is_called_once_and_a_haul_of_three_is_called() {
    let mut show = PowderShow::default();
    blast_at(&mut show, 10, 1, (3, 3), &[(4, 3)], 0);
    blast_at(&mut show, 40, 2, (5, 5), &[(6, 5), (4, 5), (5, 6)], 0);
    let firsts = show.shouts.iter().filter(|s| s.cry == Cry::First).count();
    assert_eq!(firsts, 1);
    assert!(show.shouts.iter().any(|s| s.cry == Cry::Haul(3) && s.who == 2));
    assert!(show.valid());
}

#[test]
fn a_run_of_crates_is_counted_and_goes_cold_after_three_seconds() {
    let mut show = PowderShow::default();
    show.broke(0, 1, 3, 100);
    show.broke(40, 1, 2, 97);
    assert_eq!(show.run_of(1, 40), 5);
    assert!(show.shouts.iter().any(|s| s.cry == Cry::Roll(5)));
    assert_eq!(show.run_of(1, 40 + CHAIN_GAP + 1), 0);
    show.broke(300, 1, 1, 95);
    assert_eq!(show.run_of(1, 300), 1, "a cold run starts over");
    assert_eq!(show.best[&1], 5);
    assert_eq!(show.team, 6);
}

#[test]
fn the_table_is_called_at_a_quarter_half_and_most_of_the_store() {
    let mut show = PowderShow::default();
    let mut left = 100usize;
    for now in 0..90u32 {
        show.broke(now * 3, 1 + now % 2, 1, left);
        left -= 1;
    }
    let stores: Vec<u8> = show.shouts.iter().filter_map(|s| if let Cry::Store(n) = s.cry { Some(n) } else { None }).collect();
    assert!(stores.contains(&25) || show.shouts.len() == MOST_SHOUTS, "{stores:?}");
    assert!(show.called & QUARTER != 0 && show.called & HALFWAY != 0 && show.called & MOST != 0);
}

#[test]
fn a_sooting_is_blamed_on_the_keg_whose_fire_it_stood_in() {
    let mut show = PowderShow::default();
    blast_at(&mut show, 100, 2, (5, 5), &[], 0);
    let w = show.sooted(101, 1, (6, 5), (1, 1), 3);
    assert_eq!(w, Weight::Kill);
    let s = show.soots.last().unwrap();
    assert_eq!((s.who, s.by, s.lost), (1, 2, 3));
    assert!(show.shouts.iter().any(|c| c.cry == Cry::Soot));
    // Sooting the one who sooted you is a payback.
    blast_at(&mut show, 200, 1, (9, 9), &[], 0);
    show.sooted(201, 2, (9, 10), (2, 2), 3);
    assert!(show.shouts.iter().any(|c| c.cry == Cry::Payback && c.who == 1));
    // Your own keg is an own keg.
    blast_at(&mut show, 300, 3, (7, 7), &[], 0);
    show.sooted(301, 3, (7, 7), (3, 3), 0);
    assert!(show.shouts.iter().any(|c| c.cry == Cry::OwnKeg));
}

#[test]
fn a_chain_is_called_and_its_links_wait_their_turn() {
    let mut show = PowderShow::default();
    show.chained(10, 1, 1);
    assert!(show.shouts.is_empty(), "one keg is no chain");
    show.chained(10, 1, 5);
    assert!(show.shouts.iter().any(|c| c.cry == Cry::Chain(5)));
    assert_eq!(show.chain, 5);
}

#[test]
fn blasts_are_let_go_when_old_and_the_lists_are_capped() {
    let mut show = PowderShow::default();
    for i in 0..60u32 {
        blast_at(&mut show, i, 1, (i as i32 % 20, 3), &[], 0);
        show.dropped(i, 1, (1, 1));
    }
    assert!(show.valid(), "capped");
    show.tick(5_000);
    assert!(show.blasts.is_empty() && show.drops.is_empty());
    assert!(!show.scorch.is_empty(), "the soot stays");
}

#[test]
fn the_picture_walks_a_keg_s_lanes_as_the_rules_do() {
    let mut run = super::powder_reel::fresh_for(2);
    let cols = run.room().cols as i32;
    let room = run.room().clone();
    let p = run.powder.as_mut().unwrap();
    p.t = crate::rules::folly::TITLE;
    // The corner knight's tile: a keg set there.
    let at = p.corners[&1];
    let (arms, hit) = cross(&room, &p.crates, at);
    let crates_before = p.crates.len();
    p.kegs.push(super::powder::Keg { at, owner: 1, fuse: 1 });
    run.step(&BTreeMap::new());
    let p = run.powder.as_ref().unwrap();
    let went = p.show.blasts.last().expect("the keg went off");
    assert_eq!(went.at, at);
    assert_eq!(went.arms, arms, "the picture's lanes are the rules'");
    assert_eq!(went.crates, hit);
    assert_eq!(p.crates.len(), crates_before - hit.len());
    assert!(p.show.scorch.contains(&((at.1 * cols + at.0) as u16)));
}

#[test]
fn the_show_rides_the_wire_and_an_old_snapshot_loads_quiet() {
    let mut run = super::powder_reel::fresh_for(2);
    let p = run.powder.as_mut().unwrap();
    p.t = crate::rules::folly::TITLE;
    let at = p.corners[&1];
    p.kegs.push(super::powder::Keg { at, owner: 1, fuse: 1 });
    run.step(&BTreeMap::new());
    let p = run.powder.as_ref().unwrap();
    assert!(p.valid() && !p.show.quiet());
    let back: super::powder::Powder = serde_json::from_slice(&serde_json::to_vec(p).unwrap()).unwrap();
    assert_eq!(serde_json::to_vec(&back).unwrap(), serde_json::to_vec(p).unwrap());
    let mut old: serde_json::Value = serde_json::to_value(p).unwrap();
    old.as_object_mut().unwrap().remove("show");
    let loaded: super::powder::Powder = serde_json::from_value(old).unwrap();
    assert!(loaded.show.quiet());
}

#[test]
fn kegs_that_set_one_another_off_are_a_chain_with_their_depth_noted() {
    let mut run = super::powder_reel::fresh_for(1);
    let cols = run.room().cols as i32;
    let at = run.at;
    {
        let p = run.powder.as_mut().unwrap();
        p.t = crate::rules::folly::TITLE;
        for x in 1..=6 {
            p.crates.remove(&((cols + x) as u16));
        }
        p.kegs = vec![
            super::powder::Keg { at: (1, 1), owner: 1, fuse: 1 },
            super::powder::Keg { at: (3, 1), owner: 1, fuse: 100 },
            super::powder::Keg { at: (5, 1), owner: 1, fuse: 100 },
        ];
    }
    for x in 1..=6usize {
        run.dungeon.rooms[at].set(x, 1, Tile::Floor);
    }
    run.step(&BTreeMap::new());
    let p = run.powder.as_ref().unwrap();
    let depths: Vec<u8> = p.show.blasts.iter().map(|b| b.depth).collect();
    assert_eq!(depths, vec![0, 1, 2], "each keg knows how far down the chain it was");
    assert!(p.show.shouts.iter().any(|s| s.cry == Cry::Chain(3)), "{:?}", p.show.shouts);
    assert!(p.show.blasts[2].weight >= Weight::Kill, "a deep link weighs");
    assert!(p.kegs.is_empty(), "all three went in the one tick, as before");
    assert_eq!(p.show.chain, 3);
    assert!(p.valid());
}
