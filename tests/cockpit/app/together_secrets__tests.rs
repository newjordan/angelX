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

/// The first delve (of 400 seeds, its first floor of `pack`) that, walked
/// down to `depth`, has the room `pick` names; the party in it, calmed.
fn on_a_generated_floor(pack: Pack, depth: u32, pick: impl Fn(&Run) -> Option<usize>) -> Run {
    (0..400u64)
        .find_map(|seed| {
            let mut run = Run::new(seed, 1, None);
            run.begin_in(pack);
            while run.dungeon.depth < depth {
                run.descend_for_test();
            }
            let room = pick(&run)?;
            run.enter_for_test(room);
            run.calm_for_test();
            run.rune = None;
            Some(run)
        })
        .expect("a generated floor with the room")
}

fn first_of(kind: RoomKind) -> impl Fn(&Run) -> Option<usize> {
    move |run: &Run| run.dungeon.rooms.iter().position(|r| r.kind == kind)
}

/// Knight one, standing at `(x, y)`.
fn stand(run: &mut Run, (x, y): (f32, f32)) {
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.hp) = (x, y, hero.max_hp);
}

/// A card dropped at knight one's feet, and walked over.
fn pick_up(run: &mut Run, card: &str) {
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    let at = run.at;
    run.dungeon.rooms[at].items.push(Item {
        card: card.into(),
        x,
        y,
        held_off: None,
    });
    run.step(&BTreeMap::new());
}

fn lying(run: &Run, card: &str) -> usize {
    run.room().items.iter().filter(|i| i.card == card).count()
}

fn heard(run: &Run, cue: &str) -> usize {
    run.cues.iter().filter(|c| c.as_str() == cue).count()
}

/// The first press of the vigil key, and the steps it waits to answer again.
fn vigil(run: &mut Run) {
    press(
        run,
        Input {
            vigil: true,
            ..Default::default()
        },
    );
    for _ in 0..VIGIL_REARM + 1 {
        run.step(&BTreeMap::new());
    }
}

/// Once lifted, never again: more steps there leave one card.
fn lifts_once(run: &mut Run, card: &str, name: &str) {
    assert_eq!(lying(run, card), 1, "{name} lifted {card}: {:?}", run.cues);
    assert_eq!(heard(run, &format!("secret_found:{name}")), 1);
    let key = format!("{name}:{}", run.dungeon.depth);
    assert!(run.lifted.contains(&key), "{key} kept: {:?}", run.lifted);
    assert!(
        run.marks.get("cue:secret_found").is_some_and(|n| *n >= 1),
        "Wren's bounty counts it: {:?}",
        run.marks
    );
    let title = run.book.get(card).unwrap().name.clone();
    assert!(
        run.found.as_ref().is_some_and(|f| f.2.contains(&title)),
        "the notice line names it: {:?}",
        run.found
    );
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(
        heard(run, &format!("secret_found:{name}")),
        1,
        "{name} once"
    );
}

#[test]
fn a_vigil_at_a_deep_sanctuary_altar_lifts_the_empty_cup_once() {
    let mut run = on_a_generated_floor(Pack::Crypt, 2, first_of(RoomKind::Sanctuary));
    let (ax, ay) = run.room().altar();
    stand(&mut run, (ax + 1.0, ay));
    run.step(&BTreeMap::new());
    assert_eq!(heard(&run, "secret_hint:cup"), 1, "the Herald hints");
    assert_eq!(
        lying(&run, "hearth-bell"),
        0,
        "standing by it is not enough"
    );
    // A raised guard is not a vigil.
    press(
        &mut run,
        Input {
            dash: true,
            ..Default::default()
        },
    );
    assert_eq!(lying(&run, "hearth-bell"), 0);
    vigil(&mut run);
    lifts_once(&mut run, "hearth-bell", "cup");
    assert_eq!(heard(&run, "secret_hint:cup"), 1, "hinted once");
}

#[test]
fn a_shield_raised_at_a_deeper_altar_lifts_the_glass_needle_and_a_vigil_does_not() {
    let mut run = on_a_generated_floor(Pack::Crypt, 3, first_of(RoomKind::Sanctuary));
    let (ax, ay) = run.room().altar();
    stand(&mut run, (ax - 6.0, ay));
    pick_up(&mut run, "kite-shield");
    assert_eq!(run.players[&1].guard, cards::Guard::Shield);
    stand(&mut run, (ax + 1.0, ay));
    run.step(&BTreeMap::new());
    assert_eq!(heard(&run, "secret_hint:cup"), 1);
    assert_eq!(
        heard(&run, "secret_hint:glass"),
        0,
        "one hint at a time at one altar"
    );
    vigil(&mut run);
    assert_eq!(lying(&run, "glass-needle"), 0, "a vigil is the cup's");
    assert_eq!(lying(&run, "hearth-bell"), 1);
    // Awake again, the glass speaks up; a shield held up lifts it.
    vigil(&mut run);
    assert!(!run.players[&1].vigil);
    assert_eq!(heard(&run, "secret_hint:glass"), 1, "{:?}", run.cues);
    press(
        &mut run,
        Input {
            dash: true,
            ..Default::default()
        },
    );
    lifts_once(&mut run, "glass-needle", "glass");
}

#[test]
fn a_shield_raised_at_a_sealed_chest_lifts_the_rime_whistle_before_the_lid() {
    let mut run = on_a_generated_floor(Pack::Cavern, 2, |run| {
        run.dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Treasure && r.chest.is_some())
    });
    let chest = run.room().chest.unwrap();
    stand(&mut run, (chest.x - 8.0, chest.y));
    pick_up(&mut run, "kite-shield");
    stand(&mut run, (chest.x - 4.0, chest.y));
    run.step(&BTreeMap::new());
    assert_eq!(heard(&run, "secret_hint:seal"), 1);
    assert_eq!(lying(&run, "rime-whistle"), 0, "walking up is not enough");
    press(
        &mut run,
        Input {
            dash: true,
            ..Default::default()
        },
    );
    assert!(!run.room().chest.unwrap().open, "the seal holds");
    lifts_once(&mut run, "rime-whistle", "seal");
}

#[test]
fn a_seal_already_broken_hides_nothing() {
    let mut run = on_a_generated_floor(Pack::Cavern, 2, |run| {
        run.dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Treasure && r.chest.is_some())
    });
    let chest = run.room().chest.unwrap();
    stand(&mut run, (chest.x - 8.0, chest.y));
    pick_up(&mut run, "kite-shield");
    stand(&mut run, (chest.x, chest.y));
    run.step(&BTreeMap::new());
    assert!(
        run.room().chest.is_none_or(|c| c.open),
        "opened, or a mimic"
    );
    stand(&mut run, (chest.x - 4.0, chest.y));
    for _ in 0..5 {
        press(
            &mut run,
            Input {
                dash: true,
                ..Default::default()
            },
        );
    }
    assert_eq!(lying(&run, "rime-whistle"), 0);
}

#[test]
fn a_bomb_at_the_heart_of_the_floors_farthest_fight_lifts_the_salted_thread() {
    let mut run = on_a_generated_floor(Pack::Crypt, 2, |run| {
        secrets::farthest(&run.dungeon, RoomKind::Fight)
    });
    assert_eq!(run.room().kind, RoomKind::Fight);
    let (cx, cy) = (run.room().width() / 2.0, run.room().height() / 2.0);
    stand(&mut run, (cx, cy));
    run.step(&BTreeMap::new());
    assert_eq!(heard(&run, "secret_hint:salt"), 0, "not while it is fought");
    run.clear_for_test();
    stand(&mut run, (cx, cy));
    run.step(&BTreeMap::new());
    assert_eq!(heard(&run, "secret_hint:salt"), 1);
    assert_eq!(
        lying(&run, "salt-thread"),
        0,
        "standing on it is not enough"
    );
    press(
        &mut run,
        Input {
            bomb: true,
            ..Default::default()
        },
    );
    lifts_once(&mut run, "salt-thread", "salt");
}

#[test]
fn a_bomb_in_a_nearer_fight_finds_no_salt() {
    let mut run = on_a_generated_floor(Pack::Crypt, 2, |run| {
        let far = secrets::farthest(&run.dungeon, RoomKind::Fight)?;
        (0..run.dungeon.rooms.len())
            .find(|&i| i != far && run.dungeon.rooms[i].kind == RoomKind::Fight)
    });
    run.clear_for_test();
    let (cx, cy) = (run.room().width() / 2.0, run.room().height() / 2.0);
    stand(&mut run, (cx, cy));
    press(
        &mut run,
        Input {
            bomb: true,
            ..Default::default()
        },
    );
    assert_eq!(lying(&run, "salt-thread"), 0);
    assert_eq!(heard(&run, "secret_hint:salt"), 0);
}

#[test]
fn a_bomb_at_the_heart_of_the_slain_dragons_lair_wakes_the_banked_coal() {
    let mut run = Run::new(1, 1, None);
    run.begin_in(Pack::Crypt);
    run.lair_for_test();
    run.calm_for_test();
    let (cx, cy) = (run.room().width() / 2.0, run.room().height() / 2.0);
    // Carrying bombs past it, even standing on it, lifts nothing.
    stand(&mut run, (cx, cy));
    run.step(&BTreeMap::new());
    assert_eq!(
        heard(&run, "secret_hint:coal"),
        0,
        "not with the dragon awake"
    );
    run.clear_for_test();
    assert!(run.room().cleared);
    stand(&mut run, (cx, cy));
    run.step(&BTreeMap::new());
    assert!(run.players[&1].bombs > 0);
    assert_eq!(heard(&run, "secret_hint:coal"), 1);
    assert_eq!(
        lying(&run, "cinder-wick"),
        0,
        "a bomb carried is not a bomb set off"
    );
    press(
        &mut run,
        Input {
            bomb: true,
            ..Default::default()
        },
    );
    assert_eq!(run.phase, Phase::Exploring, "still in the lair");
    lifts_once(&mut run, "cinder-wick", "coal");
}

#[test]
fn a_draught_drunk_on_a_won_stairs_landing_lifts_the_lantern_mote() {
    let mut run = on_a_generated_floor(Pack::Crypt, 2, first_of(RoomKind::Stairs));
    run.clear_for_test();
    let landing = stair_landing(run.room());
    stand(&mut run, (landing.0 - 4.0, landing.1));
    pick_up(&mut run, "potion");
    assert!(run.players[&1].hand.iter().any(|c| c == "potion"));
    stand(&mut run, landing);
    run.step(&BTreeMap::new());
    assert_eq!(heard(&run, "secret_hint:lantern"), 1);
    assert_eq!(lying(&run, "lantern-mote"), 0, "a potion held is not drunk");
    let slot = run.players[&1]
        .hand
        .iter()
        .position(|c| c == "potion")
        .unwrap();
    press(
        &mut run,
        Input {
            play: slot as u8 + 1,
            ..Default::default()
        },
    );
    assert!(!run.players[&1].hand.iter().any(|c| c == "potion"), "drunk");
    lifts_once(&mut run, "lantern-mote", "lantern");
}

#[test]
fn a_draught_drunk_at_the_lip_of_a_won_pit_lifts_the_reed_flute() {
    let mut run = on_a_generated_floor(Pack::Cavern, 2, first_of(RoomKind::Pit));
    run.clear_for_test();
    let lip = sunk_reed_at(run.room());
    stand(&mut run, (lip.0 - 5.0, lip.1));
    pick_up(&mut run, "potion");
    stand(&mut run, lip);
    run.step(&BTreeMap::new());
    assert_eq!(heard(&run, "secret_hint:reed"), 1);
    assert_eq!(lying(&run, "reed-flute"), 0);
    let slot = run.players[&1]
        .hand
        .iter()
        .position(|c| c == "potion")
        .unwrap();
    press(
        &mut run,
        Input {
            play: slot as u8 + 1,
            ..Default::default()
        },
    );
    lifts_once(&mut run, "reed-flute", "reed");
}

#[test]
fn a_vigil_kept_with_crypt_wax_at_the_ledges_far_nook_lifts_the_choir_crumb() {
    let mut run = on_a_generated_floor(Pack::Crypt, 1, first_of(RoomKind::Ledge));
    run.clear_for_test();
    let wick = low_wick_at(run.room());
    // The way in is far from it.
    let door = run.players[&1].x;
    assert!(wick.0 - door > 60.0, "the nook is across the ledge");
    stand(&mut run, wick);
    run.step(&BTreeMap::new());
    assert!(run.players[&1].grounded, "a knight can stand there");
    assert_eq!(heard(&run, "secret_hint:wick"), 1);
    // A vigil with no wax lifts nothing.
    vigil(&mut run);
    assert_eq!(lying(&run, "choir-crumb"), 0);
    vigil(&mut run);
    pick_up(&mut run, "wax");
    assert_eq!(run.players[&1].carried.get(Spoil::Wax), 1);
    vigil(&mut run);
    assert_eq!(
        run.players[&1].carried.get(Spoil::Wax),
        0,
        "the wick took it"
    );
    lifts_once(&mut run, "choir-crumb", "wick");
}

#[test]
fn a_vigil_kept_with_mines_ore_at_the_floors_farthest_hall_lifts_the_marrow_sip() {
    let mut run = on_a_generated_floor(Pack::Cavern, 1, |run| {
        secrets::farthest(&run.dungeon, RoomKind::Hall)
    });
    run.clear_for_test();
    let sill = damp_sill_at(run.room());
    stand(&mut run, sill);
    run.step(&BTreeMap::new());
    assert_eq!(heard(&run, "secret_hint:sill"), 1);
    vigil(&mut run);
    assert_eq!(lying(&run, "marrow-sip"), 0, "no ore, nothing");
    vigil(&mut run);
    pick_up(&mut run, "ore");
    assert_eq!(run.players[&1].carried.get(Spoil::Ore), 1);
    vigil(&mut run);
    assert_eq!(
        run.players[&1].carried.get(Spoil::Ore),
        0,
        "the sill took it"
    );
    lifts_once(&mut run, "marrow-sip", "sill");
}

#[test]
fn the_wick_and_the_sill_wait_only_where_their_spoil_can_be_found() {
    // A Mines first floor sheds no wax: its ledge keeps no wick to hint at.
    let mut run = on_a_generated_floor(Pack::Cavern, 1, first_of(RoomKind::Ledge));
    run.clear_for_test();
    let wick = low_wick_at(run.room());
    stand(&mut run, wick);
    run.step(&BTreeMap::new());
    assert!(!run.cues.iter().any(|c| c.starts_with("secret_hint:")));
    // A Crypt floor sheds no ore: its halls keep no sill.
    let mut run = on_a_generated_floor(Pack::Crypt, 1, |run| {
        secrets::farthest(&run.dungeon, RoomKind::Hall)
    });
    run.clear_for_test();
    let sill = damp_sill_at(run.room());
    stand(&mut run, sill);
    run.step(&BTreeMap::new());
    assert!(!run.cues.iter().any(|c| c.starts_with("secret_hint:")));
}

#[test]
fn every_small_secrets_room_is_laid_out_on_real_floors() {
    // For each secret: how many of 40 delves of each first pack give it a
    // room, on the floor it waits on.
    let mut found = BTreeMap::<&str, u32>::new();
    for seed in 0..40u64 {
        for first in [Pack::Crypt, Pack::Cavern] {
            for depth in 1..DEEPEST {
                let floor = layout::floor(depth, Pack::at(depth, first), &mut Rng::new(seed));
                let has = |kind| floor.rooms.iter().any(|r| r.kind == kind);
                let mut count = |name, yes: bool| *found.entry(name).or_default() += u32::from(yes);
                count("cup", depth >= 2 && has(RoomKind::Sanctuary));
                count("glass", depth >= 3 && has(RoomKind::Sanctuary));
                count("seal", depth >= 2 && has(RoomKind::Treasure));
                count(
                    "salt",
                    depth >= 2 && secrets::farthest(&floor, RoomKind::Fight).is_some(),
                );
                count("coal", has(RoomKind::Lair));
                count("lantern", depth >= 2 && has(RoomKind::Stairs));
                count("reed", depth >= 2 && has(RoomKind::Pit));
                count("wick", first == Pack::Crypt && has(RoomKind::Ledge));
                count(
                    "sill",
                    floor.pack.kin() == Pack::Cavern
                        && secrets::farthest(&floor, RoomKind::Hall).is_some(),
                );
            }
        }
    }
    for name in [
        "cup", "glass", "seal", "salt", "coal", "lantern", "reed", "wick", "sill",
    ] {
        assert!(
            found.get(name).is_some_and(|n| *n >= 10),
            "{name}: {found:?}"
        );
    }
}

#[test]
fn a_hint_is_said_once_though_the_host_takes_the_marks_every_frame() {
    let _guard = crate::tests::env_lock();
    let dir = std::env::temp_dir().join(format!("delve-hints-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut a = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    a.dungeon.realm = Some(crate::drive::together_realm::Realm::beside(Some(
        &dir.join("island.json"),
    )));
    let mut run = on_a_generated_floor(Pack::Crypt, 2, first_of(RoomKind::Sanctuary));
    let (ax, ay) = run.room().altar();
    stand(&mut run, (ax + 1.0, ay));
    a.dungeon.shooter = Some(run);
    let mut hints = 0;
    for _ in 0..20 {
        let run = a.dungeon.shooter.as_mut().unwrap();
        let before = run.cues.len();
        run.step(&BTreeMap::new());
        hints += run.cues[before..]
            .iter()
            .filter(|c| c.starts_with("secret_hint:"))
            .count();
        assert!(a.settle_home());
        assert!(
            a.dungeon.shooter.as_ref().unwrap().marks.is_empty(),
            "taken"
        );
    }
    assert_eq!(hints, 1, "said once across twenty frames");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_lifted_secret_and_a_given_hint_stay_so_across_a_save() {
    let mut run = on_a_generated_floor(Pack::Crypt, 2, first_of(RoomKind::Sanctuary));
    let (ax, ay) = run.room().altar();
    stand(&mut run, (ax + 1.0, ay));
    run.step(&BTreeMap::new());
    vigil(&mut run);
    let at = run.at;
    run.dungeon.rooms[at].items.clear();
    let mut restored: Run = serde_json::from_value(serde_json::to_value(&run).unwrap()).unwrap();
    restored.cues.clear();
    for _ in 0..10 {
        restored.step(&BTreeMap::new());
    }
    assert_eq!(
        lying(&restored, "hearth-bell"),
        0,
        "a save cannot refill the cup"
    );
    assert!(
        !restored
            .cues
            .iter()
            .any(|c| c.starts_with("secret_hint:cup"))
    );
}

#[test]
fn an_older_save_gains_the_secret_cards_its_book_never_had() {
    let mut run = on_a_generated_floor(Pack::Crypt, 2, first_of(RoomKind::Sanctuary));
    let mut saved = serde_json::to_value(&run).unwrap();
    let cards = saved["book"]["cards"].as_array_mut().unwrap();
    let before = cards.len();
    cards.retain(|c| c["id"] != "hearth-bell" && c["id"] != "marrow-sip");
    assert_eq!(cards.len(), before - 2);
    let mut restored: Run = serde_json::from_value(saved).unwrap();
    assert!(restored.book.get("hearth-bell").is_some());
    assert!(restored.book.get("marrow-sip").is_some());
    assert_eq!(restored.book.cards.len(), run.book.cards.len());
    assert!(
        restored.book.cards.windows(2).all(|w| w[0].id < w[1].id),
        "still sorted, so rolls replay"
    );
    // Its secret lifts a card a knight can pick up.
    let (ax, ay) = restored.room().altar();
    stand(&mut restored, (ax + 1.0, ay));
    vigil(&mut restored);
    assert_eq!(lying(&restored, "hearth-bell"), 1);
    let bell = restored
        .room()
        .items
        .iter()
        .find(|i| i.card == "hearth-bell")
        .cloned()
        .unwrap();
    vigil(&mut restored);
    stand(&mut restored, (bell.x, bell.y));
    restored.step(&BTreeMap::new());
    assert!(restored.players[&1].hand.iter().any(|c| c == "hearth-bell"));
    // A friend's own cards in a save are kept as they were.
    run.book.insert(
        cards::check("odd", "name Odd\nkind play\nheal 5\n")
            .unwrap()
            .card,
    );
    let restored: Run = serde_json::from_value(serde_json::to_value(&run).unwrap()).unwrap();
    assert!(restored.book.get("odd").is_some());
}

#[test]
fn the_small_secrets_cards_are_found_nowhere_else() {
    let book = Book::builtin();
    let secret = [
        "hearth-bell",
        "rime-whistle",
        "salt-thread",
        "glass-needle",
        "cinder-wick",
        "lantern-mote",
        "reed-flute",
        "choir-crumb",
        "marrow-sip",
    ];
    for id in secret {
        let card = book.get(id).unwrap();
        assert_eq!((card.drop, card.chest), (0, 0), "{id} is a secret's alone");
        assert!(card.summon().is_some(), "{id} calls a minion");
    }
    let mut rng = Rng::new(7);
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive] {
        for _ in 0..2000 {
            let rolled = book
                .roll_drop(&mut rng, pack)
                .into_iter()
                .chain(book.roll_chest(&mut rng, pack));
            for id in rolled {
                assert!(!secret.contains(&id.as_str()), "{id} rolled");
            }
        }
    }
}
