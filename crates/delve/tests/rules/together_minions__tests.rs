//! The Gravecaller keeps his own dead: a spell for each kind, three at
//! most and the oldest let go; the Tombstone raises its zombies a second
//! apart up to its five, and they crumble with it; the dead go for what
//! their caller marks, and come to heel at the vigil key mid-fight; a
//! warden takes the shots meant for his caller and roars foes onto
//! himself; a totem stands and breaks; a soul rip tears and mends; and a
//! save or a guest's copy carries all of it.
use super::*;
use crate::rules::hireling::testing::{fight_with, foe_at};
use crate::rules::ults::Ult;

/// The Gravecaller in an open first-floor fight room, at (12, 12), aiming
/// east, every spell ready.
fn gravecaller() -> Run {
    let mut run = fight_with(false, &[]);
    assert!(run.take_up(1, GRAVECALLER));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.aim_x, hero.aim_y) = (12.0, 12.0, 1.0, 0.0);
    hero.spell_cooldowns = [0; 4];
    run
}

/// A foe that holds still and will not die of a few blows; the room
/// fights while it stands.
fn post(run: &mut Run, kind: EnemyKind, at: (f32, f32)) -> u32 {
    let id = foe_at(run, kind, at, u32::MAX / 2);
    let foe = run.enemies.last_mut().unwrap();
    (foe.hp, foe.max_hp) = (10_000, 10_000);
    run.phase = Phase::Fighting;
    id
}

fn foe(run: &Run, id: u32) -> &Enemy {
    run.enemies.iter().find(|e| e.id == id).unwrap()
}

fn cast(run: &mut Run, slot: u8) {
    run.players.get_mut(&1).unwrap().spell_cooldowns = [0; 4];
    run.step(&BTreeMap::from([(1, Input { cast: slot, aim_x: 1, ..Default::default() })]));
}

/// Steps with nothing pressed, the knight kept whole.
fn wait(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = hero.max_hp;
        run.step(&BTreeMap::new());
    }
}

fn kinds(run: &Run) -> Vec<AllyKind> {
    run.minions.raised.iter().map(|m| m.kind).collect()
}

#[test]
fn the_gravecaller_carries_his_staff_his_four_spells_and_the_tombstone() {
    let run = gravecaller();
    let hero = &run.players[&1];
    assert!(summoner(hero));
    assert_eq!(hero.arm.as_deref(), Some("grave-staff"));
    let spells: Vec<&str> = hero.spells.iter().flatten().map(String::as_str).collect();
    assert_eq!(spells, ["raise-ghoul", "barrow-warden", "bone-totem", "soul-rip"]);
    assert_eq!(hero.ult(), Ult::Tombstone);
    assert_eq!(Ult::parse("tombstone"), Some(Ult::Tombstone));
    for id in ["grave-staff", "raise-ghoul", "barrow-warden", "bone-totem", "soul-rip"] {
        let card = run.book.get(id).unwrap_or_else(|| panic!("{id} is in the book"));
        assert_eq!((card.drop, card.chest), (0, 0), "{id} is his alone, never loot");
    }
    // Another kit at the table gives his spells back.
    let mut run = run;
    assert!(run.take_up(1, "percival"));
    assert!(run.players[&1].spells.iter().all(Option::is_none));
}

#[test]
fn each_spell_raises_its_own_kind_three_at_most_and_the_oldest_goes() {
    let mut run = gravecaller();
    post(&mut run, EnemyKind::Bat, (40.0, 4.0));
    cast(&mut run, 1);
    cast(&mut run, 2);
    cast(&mut run, 3);
    assert_eq!(kinds(&run), [AllyKind::Ghoul, AllyKind::Warden, AllyKind::Totem]);
    assert!(run.minions.raised.iter().all(|m| m.summoner == Some(1)));
    assert!(run.hireling.is_none(), "none of them takes the party's place");
    assert!(run.minions.raised.iter().all(|m| m.rising > 0), "each climbs out of the ground");
    // The totem is planted along the aim, the others come up beside him.
    let totem = &run.minions.raised[2];
    assert!(totem.x > 14.0, "planted ahead: {}", totem.x);
    let hp: Vec<u32> = run.minions.raised.iter().map(|m| m.max_hp).collect();
    assert_eq!(hp, [90, 240, 120], "as hardy on every floor");
    cast(&mut run, 1);
    assert_eq!(
        kinds(&run),
        [AllyKind::Warden, AllyKind::Totem, AllyKind::Ghoul],
        "a fourth lets the oldest go"
    );
    assert!(run.cues.iter().any(|c| c == "crumbled:ghoul"));
    assert!(run.valid_snapshot());
}

#[test]
fn beaumains_keeps_the_partys_place_beside_a_summoners_own() {
    let mut run = fight_with(true, &["hearth-bell"]);
    assert!(run.take_up(1, GRAVECALLER));
    run.players.get_mut(&1).unwrap().hand = vec!["hearth-bell".into()];
    crate::rules::hireling::testing::play(&mut run, 1);
    assert_eq!(run.hireling.as_ref().unwrap().kind, AllyKind::Beaumains);
    assert_eq!(kinds(&run), [AllyKind::Brownie], "a card's minion goes to his own side");
}

#[test]
fn the_tombstone_raises_a_zombie_a_second_up_to_five_at_once() {
    let mut run = gravecaller();
    // Far off and frozen, so the zombies shamble at it and nothing dies.
    post(&mut run, EnemyKind::Skeleton, (44.0, 24.0));
    run.plant_tombstone(1);
    let stone = run.minions.stones[0].clone();
    assert_eq!(stone.owner, 1);
    assert!(stone.x > 13.0, "a few steps along the aim: {}", stone.x);
    let zombies = |run: &Run| kinds(run).iter().filter(|k| **k == AllyKind::Zombie).count();
    let mut most = 0;
    let mut seen = Vec::new();
    for t in 0..TOMB_TICKS - 1 {
        wait(&mut run, 1);
        let n = zombies(&run);
        most = most.max(n);
        if seen.last() != Some(&n) {
            seen.push(n);
            assert!(t <= u32::try_from(n).unwrap() * HZ, "about a second apart: {n} at {t}");
        }
    }
    assert_eq!(most, TOMB_ZOMBIES, "up to five at once, never more");
    assert!(run.minions.raised.iter().filter(|m| m.kind == AllyKind::Zombie).all(|m| m.summoner == Some(1)));
}

#[test]
fn its_zombies_crumble_when_the_stone_is_broken_or_its_time_runs_out() {
    let mut run = gravecaller();
    post(&mut run, EnemyKind::Skeleton, (44.0, 24.0));
    run.plant_tombstone(1);
    wait(&mut run, 3 * HZ);
    assert!(kinds(&run).contains(&AllyKind::Zombie));
    run.minions.stones[0].hp = 1;
    // A foe's shot finds the grave.
    let (x, y) = (run.minions.stones[0].x, run.minions.stones[0].y);
    run.projectiles.push(Projectile {
        x: x - 0.5,
        y,
        vx: 4.0,
        vy: 0.0,
        hostile: true,
        look: None,
        kind: Shot::Orb,
        damage: 8,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: HZ,
    });
    wait(&mut run, 1);
    assert!(run.minions.stones.is_empty(), "broken");
    assert!(!kinds(&run).contains(&AllyKind::Zombie), "its zombies with it");
    assert!(run.cues.iter().any(|c| c == "tombstone_broken"));
    // A second grave, left to stand its time.
    run.plant_tombstone(1);
    wait(&mut run, 3 * HZ);
    assert!(kinds(&run).contains(&AllyKind::Zombie));
    wait(&mut run, TOMB_TICKS);
    assert!(run.minions.stones.is_empty() && !kinds(&run).contains(&AllyKind::Zombie), "spent");
}

#[test]
fn a_zombie_takes_hold_of_a_small_foe_and_bites() {
    let mut run = gravecaller();
    let skeleton = post(&mut run, EnemyKind::Skeleton, (16.0, 12.0));
    run.plant_tombstone(1);
    let mut held = false;
    for _ in 0..4 * HZ {
        wait(&mut run, 1);
        held |= foe(&run, skeleton).chilled > 0;
    }
    assert!(held, "a bitten foe walks slowly");
    assert!(foe(&run, skeleton).hp < 10_000, "bitten");
}

#[test]
fn the_dead_go_for_what_their_caller_marks_before_anything_nearer() {
    let mut run = gravecaller();
    cast(&mut run, 1);
    wait(&mut run, RISE + 1);
    // Both outside the aura: only the ghoul can hurt the nearer one.
    let near = post(&mut run, EnemyKind::Skeleton, (7.0, 18.0));
    let far = post(&mut run, EnemyKind::Skeleton, (22.0, 12.0));
    // He fires along his aim, east: the far one is marked.
    run.step(&BTreeMap::from([(1, Input { fire: true, aim_x: 1, ..Default::default() })]));
    assert_eq!(run.minions.quarry.get(&1), Some(&far));
    wait(&mut run, 3 * HZ);
    assert!(foe(&run, far).hp < 10_000, "the ghoul went for the mark");
    assert_eq!(foe(&run, near).hp, 10_000, "and left the nearer one be");
    // The mark fallen, it is forgotten.
    run.enemies.retain(|e| e.id != far);
    wait(&mut run, 1);
    assert!(run.minions.quarry.get(&1).is_none());
}

#[test]
fn mid_fight_the_vigil_key_calls_them_to_heel_and_sends_them_back() {
    let mut run = gravecaller();
    cast(&mut run, 1);
    wait(&mut run, RISE + 1);
    let away = post(&mut run, EnemyKind::Skeleton, (24.0, 12.0));
    let g = |run: &mut Run| {
        run.step(&BTreeMap::from([(1, Input { vigil: true, ..Default::default() })]));
        run.step(&BTreeMap::new());
    };
    g(&mut run);
    assert_eq!(run.minions.order(1), Order::Heel);
    assert!(!run.players[&1].vigil, "no knight kneels mid-fight");
    assert!(run.cues.iter().any(|c| c == "minions:heel"));
    wait(&mut run, 3 * HZ);
    let ghoul = &run.minions.raised[0];
    let hero = &run.players[&1];
    assert!((ghoul.x - hero.x).hypot(ghoul.y - hero.y) < 4.0, "at his side");
    assert_eq!(foe(&run, away).hp, 10_000, "nothing near him to fall on");
    g(&mut run);
    assert_eq!(run.minions.order(1), Order::Hunt);
    wait(&mut run, 3 * HZ);
    assert!(foe(&run, away).hp < 10_000, "back on the hunt");
}

#[test]
fn a_warden_takes_the_shots_meant_for_his_caller_and_roars_foes_onto_himself() {
    let mut run = gravecaller();
    let skeleton = post(&mut run, EnemyKind::Skeleton, (18.0, 12.0));
    cast(&mut run, 2);
    wait(&mut run, RISE + HZ);
    let warden = run.minions.raised[0].clone();
    let hero = &run.players[&1];
    assert!(warden.x > hero.x, "out before him, toward what comes");
    assert!(warden.taunt > 0, "he roared at the foe near him");
    // The foe sees only him.
    let mut seen = vec![(hero.x, hero.y)];
    run.minions.roared(foe(&run, skeleton), &mut seen);
    assert_eq!(seen, [(warden.x, warden.y)]);
    // A volley at the knight from the east: his shield takes it.
    let hp = run.players[&1].hp;
    let (wx, wy) = (warden.x, warden.y);
    for k in 0..3 {
        run.projectiles.push(Projectile {
            x: wx + 2.0 + k as f32,
            y: wy,
            vx: -10.0,
            vy: 0.0,
            hostile: true,
            look: None,
            kind: Shot::Orb,
            damage: 10,
            pierce: 0,
            last_hit: None,
            empowered: false,
            traits: Default::default(),
            ttl: HZ,
        });
    }
    for _ in 0..HZ / 2 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&1].hp, hp, "none reached his caller");
    let now = &run.minions.raised[0];
    assert_eq!(now.max_hp - now.hp, 15, "each at half its weight, without a grace between");
}

#[test]
fn a_totem_stands_where_it_was_planted_spits_grave_fire_and_breaks() {
    let mut run = gravecaller();
    post(&mut run, EnemyKind::Skeleton, (24.0, 12.0));
    cast(&mut run, 3);
    let at = (run.minions.raised[0].x, run.minions.raised[0].y);
    let mut bolts = 0;
    for _ in 0..2 * HZ {
        wait(&mut run, 1);
        bolts = bolts.max(
            run.projectiles
                .iter()
                .filter(|p| !p.hostile && p.look.as_ref().is_some_and(|(c, _)| c == "venom"))
                .count(),
        );
    }
    let totem = &run.minions.raised[0];
    assert_eq!((totem.x, totem.y), at, "it never moves");
    assert!(bolts > 0, "grave-fire at the foe in reach");
    run.minions.raised[0].hp = 0;
    wait(&mut run, 1);
    assert!(run.minions.raised.is_empty(), "broken, it is gone");
}

#[test]
fn a_soul_rip_tears_every_foe_near_the_aim_and_mends_him_and_his_dead() {
    let mut run = gravecaller();
    cast(&mut run, 1);
    wait(&mut run, RISE + 1);
    let torn: Vec<u32> = [(17.0, 12.0), (17.5, 13.0)].map(|at| post(&mut run, EnemyKind::Skeleton, at)).into();
    let spared = post(&mut run, EnemyKind::Skeleton, (30.0, 4.0));
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp = hero.max_hp / 2;
    let before = hero.hp;
    run.minions.raised[0].hp = 10;
    run.players.get_mut(&1).unwrap().spell_cooldowns = [0; 4];
    run.step(&BTreeMap::from([(1, Input { cast: 4, aim_x: 1, ..Default::default() })]));
    for id in torn {
        assert!(foe(&run, id).hp < 10_000, "torn");
    }
    assert_eq!(foe(&run, spared).hp, 10_000, "out of its reach");
    assert!(run.players[&1].hp > before, "what is torn mends him");
    assert!(run.minions.raised[0].hp > 10, "and his dead");
    assert!(run.cues.iter().any(|c| c == "soul_rip"));
}

#[test]
fn into_a_new_room_his_dead_follow_and_the_grave_and_totem_stay() {
    let mut run = gravecaller();
    post(&mut run, EnemyKind::Bat, (40.0, 4.0));
    cast(&mut run, 1);
    cast(&mut run, 3);
    run.plant_tombstone(1);
    wait(&mut run, 2 * HZ);
    run.hireling_follows();
    assert_eq!(kinds(&run), [AllyKind::Ghoul]);
    assert!(run.minions.stones.is_empty());
}

#[test]
fn a_save_and_a_guests_copy_carry_the_dead_and_a_run_without_them_is_as_it_was() {
    let mut run = gravecaller();
    let plain = serde_json::to_string(&run).unwrap();
    assert!(!plain.contains("\"minions\""), "nothing written while there are none");
    post(&mut run, EnemyKind::Skeleton, (30.0, 12.0));
    cast(&mut run, 1);
    cast(&mut run, 2);
    run.plant_tombstone(1);
    run.minions.orders.insert(1, Order::Heel);
    wait(&mut run, 2 * HZ);
    assert!(!run.minions.raised.is_empty() && !run.minions.stones.is_empty());
    // A save.
    let back: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert_eq!(back.minions, run.minions);
    // An older save, from before the dead: no field at all.
    let mut old: serde_json::Value = serde_json::from_str(&plain).unwrap();
    old.as_object_mut().unwrap().remove("minions");
    let old: Run = serde_json::from_value(old).unwrap();
    assert!(old.minions.is_empty());
    // A guest's copy over the wire reads back the same dead, and draws them.
    let tree = crate::wire::publish(&run).unwrap();
    let guest = crate::wire::read(&tree).unwrap();
    assert_eq!(guest.minions, run.minions);
    assert!(guest.valid_snapshot());
}

#[test]
fn a_hand_written_card_may_call_his_kinds_and_a_spell_may_rip() {
    let ghoul = "name Grave Bell\nkind play\ntext A ghoul answers.\nghoul 1\n";
    let card = cards::check("grave-bell", ghoul).expect("a play card may call a ghoul").card;
    assert_eq!(card.minion(), Some((AllyKind::Ghoul, 1)));
    let rip = "name Little Rip\nkind spell\ntext Tear.\ncooldown 9\nrip 20\n";
    let card = cards::check("little-rip", rip).expect("a spell may rip").card;
    assert_eq!(card.rip(), Some(20));
    let taken = "name Rip Charm\nkind take\ntext No.\nrip 20\n";
    assert!(cards::check("rip-charm", taken).is_err(), "a soul rip is a spell's alone");
}

#[test]
fn a_spell_wished_takes_the_soul_rips_slot_and_a_second_is_refused() {
    // His kit fills all four slots: every spell he wished used to bounce
    // ("all four spell slots are full") and come back as a refund.
    let mut run = gravecaller();
    run.players.get_mut(&1).unwrap().spell_cooldowns[3] = 40;
    let firebolt = phrasebook::get("firebolt").unwrap();
    assert_eq!(firebolt.kind, "spell");
    let name = run.grant(1, &firebolt).expect("a wished spell finds a slot");
    let hero = &run.players[&1];
    assert_eq!(hero.spells[..3], [Some("raise-ghoul".into()), Some("barrow-warden".into()), Some("bone-totem".into())]);
    assert!(hero.spells[3].as_deref().is_some_and(|c| c.starts_with("wish-firebolt-")), "{name}: {:?}", hero.spells);
    assert_eq!(hero.spell_cooldowns[3], 0, "the slot comes ready, not on the soul rip's recharge");
    // The same wish again climbs its ladder in that slot.
    run.grant(1, &firebolt).expect("its next tier");
    assert!(run.players[&1].spells[3].as_deref().is_some_and(|c| c.starts_with("wish-firebolt-")));
    // A second spell finds his slots full, as a champion's would be.
    let ring = phrasebook::get("spark-ring").unwrap();
    assert!(run.grant(1, &ring).is_err());
    assert_eq!(run.players[&1].spells[2].as_deref(), Some("bone-totem"));
}
