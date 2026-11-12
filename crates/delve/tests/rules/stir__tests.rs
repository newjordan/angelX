use super::*;
use crate::rules::claim_bosses::{self, Guardian};

/// A won seam three floors down, so a raid may reach it.
fn deep() -> (Run, industry::Deposit) {
    let (mut run, d) = claim_bosses::tests::fixture(Guardian::Ironroot);
    run.dungeon.depth = stir::RAID_FROM;
    run.dungeon.pack = Pack::Hellforge;
    let d = run.industry_deposits().into_iter().next().unwrap();
    run.home.industry.discover(
        crate::kingdom::catalog(),
        d.id.clone(),
        crate::kingdom::Claim {
            resource: d.resource.into(),
            name: d.name.clone(),
            source: d.seam.source(),
            secured: false,
            rate: d.seam.rate(),
            yields: d.seam.yields(), surface: false },
    );
    assert!(run.home.industry.secure(&d.id));
    run.enemies.clear();
    (run, d)
}
fn near(run: &mut Run, d: &industry::Deposit) {
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (d.x, d.y);
}
fn far(run: &mut Run, d: &industry::Deposit) {
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (d.x + 30.0, d.y);
}
/// The party stands by the claim, goes away for `away` ticks and returns.
fn leave_and_return(run: &mut Run, d: &industry::Deposit, away: u64) {
    near(run, d);
    run.tick_stir_claim(d);
    far(run, d);
    run.tick += away;
    near(run, d);
    run.tick_stir_claim(d);
}

#[test]
fn digging_and_working_the_shaft_stir_the_deep_and_it_settles_on_its_own() {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    assert_eq!(run.kingdom.stir.level, 0);
    run.stir_up(7);
    assert_eq!(run.kingdom.stir.level, 7);
    run.stir_up(5000);
    assert_eq!(run.kingdom.stir.level, stir::MAX, "it can never be more stirred than this");
    run.tick = stir::CALM_EVERY * 4;
    run.tick_stir();
    assert_eq!(run.kingdom.stir.level, stir::MAX - 1, "a point settles");
    // A shift with no mine at work stirs nothing.
    run.kingdom.stir.level = 0;
    run.home.industry.start(crate::kingdom::catalog());
    run.tick = industry::SHIFT_TICKS * 3;
    run.tick_stir();
    assert_eq!(run.kingdom.stir.level, 0, "no mine at work, nothing stirred");
    // A mine on a claim stirs it a point a shift, and the shaft's machinery
    // over it a little more.
    run.home.industry.buildings.insert(
        7,
        crate::kingdom::Building { plan: "mine".into(), claim: Some("seam".into()), ..Default::default() },
    );
    run.tick_stir();
    assert_eq!(run.kingdom.stir.level, 1);
    run.home.stockyard.tier = crate::rules::stockyard::Tier::WaterHoist;
    run.tick_stir();
    assert_eq!(run.kingdom.stir.level, 1 + 1 + 2);
    run.home.industry.buildings.get_mut(&7).unwrap().paused = true;
    run.tick_stir();
    assert_eq!(run.kingdom.stir.level, 4, "a mine that is stopped stirs nothing");
}

#[test]
fn the_deep_says_so_once_when_it_turns_restless() {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.stir_up(stir::RAID_AT - 1);
    assert!(run.notices.is_empty());
    run.stir_up(1);
    assert!(run.notices.last().is_some_and(|n| n.contains("restless")), "{:?}", run.notices);
    let said = run.notices.len();
    run.stir_up(10);
    assert_eq!(run.notices.len(), said, "said as it crosses, not every time");
}

#[test]
fn an_unguarded_claim_in_a_stirred_deep_gets_its_sentries_back() {
    let (mut run, d) = deep();
    run.kingdom.stir.level = stir::RAID_AT;
    leave_and_return(&mut run, &d, stir::UNGUARDED + 1);
    assert!(run.kingdom.stir.raiding(&d.id));
    assert!(run.notices.last().is_some_and(|n| n.starts_with("RAID")), "{:?}", run.notices);
    assert!(run.raided_claims().contains(d.id.as_str()));
    // The ordinary sentries stand up again, two of them, and are admitted.
    run.tick_ore_sentries(&d);
    assert_eq!(run.enemies.len(), 2);
    assert!(run.enemies.iter().all(|e| run.admitted_ore_sentry(e)));
    assert!(!run.ore_sentries_defeated(&d.id));
}

#[test]
fn a_broken_raid_quiets_the_deep_and_frees_the_face() {
    let (mut run, d) = deep();
    run.kingdom.stir.level = 60;
    leave_and_return(&mut run, &d, stir::UNGUARDED + 1);
    run.tick_ore_sentries(&d);
    for e in &mut run.enemies {
        e.hp = 0;
    }
    run.settle_kills();
    run.tick_stir_claim(&d);
    assert!(!run.kingdom.stir.raiding(&d.id));
    assert_eq!(run.kingdom.stir.level, 60 - stir::RELIEF);
    assert!(run.notices.last().is_some_and(|n| n.contains("broken")), "{:?}", run.notices);
    assert!(run.home.industry.claims[&d.id].secured, "a raid never takes the claim");
    assert!(run.ore_sentries_defeated(&d.id));
}

#[test]
fn a_raid_comes_once_to_a_claim_an_expedition() {
    let (mut run, d) = deep();
    run.kingdom.stir.level = 80;
    leave_and_return(&mut run, &d, stir::UNGUARDED + 1);
    run.kingdom.stir.raids.get_mut(&d.id).unwrap().live = false;
    leave_and_return(&mut run, &d, stir::UNGUARDED + 1);
    assert!(!run.kingdom.stir.raiding(&d.id), "no second raid on the same claim");
    assert_eq!(run.kingdom.stir.raids[&d.id].n, 1);
    run.kingdom.stir.new_expedition();
    leave_and_return(&mut run, &d, 1);
    near(&mut run, &d);
    run.tick += stir::UNGUARDED + 1;
    far(&mut run, &d);
    run.tick_stir_claim(&d);
    near(&mut run, &d);
    run.tick_stir_claim(&d);
    assert!(run.kingdom.stir.raiding(&d.id), "a new expedition can be raided again");
}

#[test]
fn nothing_raids_a_calm_deep_a_guarded_claim_or_the_first_two_floors() {
    // Calm: below the threshold.
    let (mut run, d) = deep();
    run.kingdom.stir.level = stir::RAID_AT - 1;
    leave_and_return(&mut run, &d, stir::UNGUARDED + 1);
    assert!(!run.kingdom.stir.raiding(&d.id));
    // Guarded: the party only stepped out a moment.
    let (mut run, d) = deep();
    run.kingdom.stir.level = stir::MAX;
    leave_and_return(&mut run, &d, stir::UNGUARDED - 30);
    assert!(!run.kingdom.stir.raiding(&d.id));
    // A new player's first two floors are never raided, however stirred.
    for floor in 1..stir::RAID_FROM {
        let (mut run, d) = deep();
        run.dungeon.depth = floor;
        run.kingdom.stir.level = stir::MAX;
        leave_and_return(&mut run, &d, stir::UNGUARDED * 4);
        assert!(!run.kingdom.stir.raiding(&d.id), "floor {floor}");
        assert!(run.kingdom.stir.raids.is_empty());
    }
    // Nobody is there: the sentries are not sent to an empty room.
    let (mut run, d) = deep();
    run.kingdom.stir.level = stir::MAX;
    near(&mut run, &d);
    run.tick_stir_claim(&d);
    far(&mut run, &d);
    run.tick += stir::UNGUARDED * 3;
    run.tick_stir_claim(&d);
    assert!(!run.kingdom.stir.raiding(&d.id));
}

#[test]
fn a_raid_cannot_lock_the_party_in_or_outlive_the_room() {
    let (mut run, d) = deep();
    run.kingdom.stir.level = stir::MAX;
    leave_and_return(&mut run, &d, stir::UNGUARDED + 1);
    run.tick_ore_sentries(&d);
    assert_eq!(run.enemies.len(), 2);
    // Leaving the room discards the bodies as any room does, the raid stays
    // live, and coming back raises exactly the same two, never more.
    for _ in 0..3 {
        run.enemies.clear();
        run.tick_ore_sentries(&d);
        assert_eq!(run.enemies.len(), 2, "two raiders, however often the room is entered");
    }
    // A crowded room waits, as for any seam, and never invents a body.
    run.enemies.clear();
    for _ in 0..boss_ecology::ENTITY_CAP - 1 {
        run.spawn_at_for_test(EnemyKind::Skeleton, 1.0, 1.0);
    }
    let before = run.enemies.len();
    run.tick_ore_sentries(&d);
    assert_eq!(run.enemies.len(), before);
    assert!(run.kingdom.stir.valid());
}

#[test]
fn the_stir_is_bounded_and_saved() {
    let (mut run, d) = deep();
    run.kingdom.stir.level = stir::MAX;
    leave_and_return(&mut run, &d, stir::UNGUARDED + 1);
    run.tick_ore_sentries(&d);
    let back: stir::Stir = serde_json::from_str(&serde_json::to_string(&run.kingdom.stir).unwrap()).unwrap();
    assert_eq!(back, run.kingdom.stir);
    assert!(back.raiding(&d.id) && back.valid());
    // The whole run, with a raid on, as a guest is sent it.
    let (mut plain, id) = {
        let (r, d) = claim_bosses::tests::fixture(Guardian::Ironroot);
        (r, d.id)
    };
    plain.kingdom.stir.level = 50;
    plain.kingdom.stir.cut.insert(id.clone(), 3);
    plain.kingdom.stir.raids.insert(id.clone(), stir::Raid { n: 1, live: true });
    let back: Run = serde_json::from_str(&serde_json::to_string(&plain).unwrap()).unwrap();
    assert!(back.valid_snapshot());
    assert_eq!(back.kingdom.stir, plain.kingdom.stir);
    let mut bad = back.clone();
    bad.kingdom.stir.level = stir::MAX + 1;
    assert!(!bad.valid_snapshot());
    let mut bad = back;
    for i in 0..70 {
        bad.kingdom.stir.cut.insert(format!("k{i}"), 1);
    }
    assert!(!bad.valid_snapshot(), "the books are bounded");
}
