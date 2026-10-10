use super::*;

/// A run whose first floor (of `pack`) hides a vault, standing in the room
/// with the crack, its fight calmed.
fn by_the_crack(pack: Pack) -> Run {
    let mut run = (0..200u64)
        .map(|seed| {
            let mut run = Run::new(seed, 1, None);
            run.begin_in(pack);
            run
        })
        .find(|run| run.dungeon.secret.is_some())
        .expect("a first floor with a vault");
    let host = run.dungeon.secret.unwrap().host;
    run.enter_for_test(host);
    run.calm_for_test();
    run.rune = None;
    run
}

fn press(run: &mut Run, input: Input) {
    run.step(&BTreeMap::from([(1, input)]));
}

#[test]
fn some_floors_hide_a_vault_behind_a_cracked_wall() {
    let mut vaults = 0;
    let mut floors = 0;
    for seed in 0..60u64 {
        for depth in 1..DEEPEST {
            let floor = layout::floor(depth, Pack::Crypt, &mut Rng::new(seed));
            floors += 1;
            let Some(secret) = floor.secret else {
                continue;
            };
            vaults += 1;
            let host = &floor.rooms[secret.host];
            let vault = &floor.rooms[secret.vault];
            assert_eq!(host.kind, RoomKind::Fight, "a fight room hides it");
            assert!(!host.doors[secret.side], "behind a wall, not a door");
            assert_eq!(vault.kind, RoomKind::Secret);
            assert!(
                vault.doors[(secret.side + 2) % 4],
                "its own door faces home"
            );
            let (dx, dy) = DIRS[secret.side];
            assert_eq!(vault.cell, (host.cell.0 + dx, host.cell.1 + dy));
            assert_eq!(floor.neighbour(secret.host, secret.side), None);
            assert!(vault.roster.is_empty(), "nobody guards it");
            if !secret.snibbet {
                assert!(vault.chest.is_some());
                assert_eq!(vault.items.iter().filter(|i| i.card == "gold").count(), 8);
            }
        }
    }
    assert!(
        (floors / 4..=floors * 3 / 4).contains(&vaults),
        "{vaults} vaults in {floors} floors"
    );
    // The bottom of the Unknown keeps no vault.
    for seed in 0..20u64 {
        let floor = layout::floor(DEEPEST, Pack::Unknown, &mut Rng::new(seed));
        assert!(floor.secret.is_none());
    }
}

#[test]
fn a_knights_bomb_brings_the_cracked_wall_down() {
    let mut run = by_the_crack(Pack::Crypt);
    let secret = run.dungeon.secret.unwrap();
    assert!(
        run.cues.iter().any(|c| c == "crack_seen"),
        "the Herald notices"
    );
    press(
        &mut run,
        Input {
            bomb: true,
            ..Default::default()
        },
    );
    let opened = run.dungeon.secret.unwrap();
    assert!(opened.found);
    assert_eq!(
        run.dungeon.neighbour(secret.host, secret.side),
        Some(secret.vault)
    );
    assert!(run.cues.iter().any(|c| c == "secret_found"));
    assert!(run.feats.contains(&"secret_room"));
    assert!(
        run.dungeon.crack().is_none(),
        "no crack once it is a doorway"
    );
}

#[test]
fn a_bomb_in_another_room_leaves_the_wall_standing() {
    let mut run = by_the_crack(Pack::Crypt);
    let secret = run.dungeon.secret.unwrap();
    let elsewhere = (0..run.dungeon.rooms.len())
        .find(|&i| i != secret.host && run.dungeon.rooms[i].kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(elsewhere);
    press(
        &mut run,
        Input {
            bomb: true,
            ..Default::default()
        },
    );
    assert!(!run.dungeon.secret.unwrap().found);
}

#[test]
fn a_keg_beside_the_crack_brings_it_down_and_one_across_the_room_does_not() {
    let blows = |near: bool| {
        let mut run = by_the_crack(Pack::Crypt);
        let (_, cx, cy) = run.dungeon.crack().unwrap();
        let (w, h) = (run.room().width(), run.room().height());
        let (x, y) = if near {
            (cx.clamp(3.0, w - 3.0), cy.clamp(3.0, h - 3.0))
        } else {
            (w - cx.clamp(3.0, w - 3.0), h - cy.clamp(3.0, h - 3.0))
        };
        run.kegs.push(foes::Keg { x, y, fuse: 1 });
        run.step(&BTreeMap::new());
        run.dungeon.secret.unwrap().found
    };
    assert!(blows(true), "a keg by the crack");
    assert!(!blows(false), "not one across the room");
}

#[test]
fn through_the_fallen_wall_lies_the_vault() {
    let mut run = by_the_crack(Pack::Cavern);
    let secret = run.dungeon.secret.unwrap();
    press(
        &mut run,
        Input {
            bomb: true,
            ..Default::default()
        },
    );
    run.clear_for_test();
    run.enter_for_test(secret.vault);
    assert_eq!(run.room().kind, RoomKind::Secret);
    assert_eq!(run.phase, Phase::Exploring, "nobody guards it");
    assert!(run.room().chest.is_some() || secret.snibbet);
}

#[test]
fn some_vaults_keep_snibbet_who_pays_the_party_to_forget_him_once() {
    let (mut vaults, mut snibbets) = (0, 0);
    for seed in 0..80u64 {
        for depth in 1..DEEPEST {
            let floor = layout::floor(depth, Pack::Cavern, &mut Rng::new(seed));
            if let Some(secret) = floor.secret {
                vaults += 1;
                if secret.snibbet {
                    snibbets += 1;
                    let vault = &floor.rooms[secret.vault];
                    assert!(
                        vault.chest.is_none() && vault.items.is_empty(),
                        "only Snibbet"
                    );
                }
            }
        }
    }
    assert!(
        (vaults / 6..=vaults / 2).contains(&snibbets),
        "{snibbets} of {vaults} vaults"
    );
    let mut run = (0..400u64)
        .map(|seed| {
            let mut run = Run::new(seed, 1, None);
            run.begin_in(Pack::Crypt);
            run
        })
        .find(|run| run.dungeon.secret.is_some_and(|s| s.snibbet))
        .expect("a first floor where Snibbet waits");
    let secret = run.dungeon.secret.unwrap();
    run.dungeon.open_secret();
    run.enter_for_test(secret.vault);
    let (sx, sy) = secrets::snibbet_at(run.room());
    // Beside him, clear of where the gold lands.
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (sx + 4.5, sy);
    run.step(&BTreeMap::new());
    assert!(run.dungeon.secret.unwrap().paid);
    assert!(run.cues.iter().any(|c| c == "snibbet"));
    let gold = run.room().items.iter().filter(|i| i.card == "gold").count();
    assert!(gold >= 6, "{gold} piles at their feet");
    assert_eq!(
        run.found.as_ref().map(|f| f.2.as_str()),
        Some("It's a secret to everybody.")
    );
    // Once.
    let heard = run.cues.len();
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    assert!(!run.cues[heard..].iter().any(|c| c == "snibbet"));
}
