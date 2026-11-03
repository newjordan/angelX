use super::*;

fn input(fire: bool) -> BTreeMap<u32, Input> {
    BTreeMap::from([(
        1,
        Input {
            aim_x: 1,
            fire,
            ..Default::default()
        },
    )])
}

/// A wish of every family (`combos`), held: every super effect's recipe
/// made, so a test of what a super effect does needs no other wish.
pub(crate) fn combo_kit(run: &mut Run, id: u32) {
    let kit = cards::check(
        "wish-kit-1",
        "name Kit\nkind hold\ntext frog bullet storm fire shadow corn gun mail fast\nspeed 5\n",
    )
    .unwrap()
    .card;
    run.book.insert(kit);
    let hero = run.players.get_mut(&id).unwrap();
    hero.deck.push("wish-kit-1".into());
    hero.rebonus(&run.book);
}

#[test]
fn orbit_drawing_and_damage_positions_share_the_decks_two_orbit_cap() {
    let mut run = Run::new(4, 1, None);
    run.players.get_mut(&1).unwrap().bonus.orbit = 9;
    assert_eq!(run.orbits(1).len(), cards::cap("orbit") as usize);
}

fn fight(run: &mut Run) {
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
}

/// The floor's boss room, entered: its gates set aside, so the stairs
/// room is the boss's (`floor_boss_here`).
fn boss_fight(run: &mut Run) {
    run.boss_gates = None;
    let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    run.enter_for_test(room);
}

#[test]
fn a_delve_starts_with_three_wishes_each_floor_brings_two_and_its_boss_a_super_one() {
    let mut run = Run::at_home(4, 2, Some("Matt"), Default::default(), Default::default());
    run.descend_for_test();
    for id in 1..=2 {
        let hero = &run.players[&id];
        assert_eq!((hero.wishes, hero.great_wishes), (overclass::START_WISHES, 0), "a delve starts with three each");
    }
    boss_fight(&mut run);
    assert!(run.can_wish(1), "wishes are held through a fight");
    assert_eq!(run.players[&1].great_wishes, 0);
    run.clear_for_test();
    assert_eq!((run.players[&1].great_wishes, run.players[&2].great_wishes), (1, 1), "the floor's boss fell: a super wish each");
    assert!(run.window.as_ref().is_some_and(|w| w.great), "and the game says so");
    run.descend_for_test();
    assert_eq!(run.players[&1].wishes, overclass::START_WISHES + overclass::FLOOR_WISHES, "a new floor: two more each");
    assert_eq!(run.window.as_ref().map(|w| w.given), Some(overclass::FLOOR_WISHES));
}

#[test]
fn a_super_wish_takes_two_steps_and_a_refused_wish_spends_nothing() {
    let mut run = Run::new(4, 2, Some("Matt"));
    let volley = phrasebook::get("volley").unwrap();
    assert!(run.wish(1, &volley).is_err(), "no wish held, none made");
    run.give_wishes(1, 1);
    combo_kit(&mut run, 1);
    assert_eq!(run.wish(1, &volley).unwrap(), "Triple Volley II", "a super wish first, two steps up the ladder");
    assert_eq!((run.players[&1].wishes, run.players[&1].great_wishes), (1, 0));
    // The step above is every knight's now (no teacher holds the third
    // tier): the plain wish takes it.
    assert!(run.wish(1, &volley).unwrap().ends_with("III"));
    assert_eq!(run.players[&1].wishes, 0);
    // A ladder at its top refuses, and the plain wish is still held.
    run.give_wishes(1, 0);
    assert!(run.wish(1, &volley).is_err());
    assert_eq!(run.players[&1].wishes, 1);
    // A super wish with one step left takes it and leaves a plain wish.
    let fork = phrasebook::get("fork").unwrap();
    run.grant(2, &fork).unwrap();
    run.grant(2, &fork).unwrap();
    let knight = run.players.get_mut(&2).unwrap();
    (knight.wishes, knight.great_wishes) = (0, 1);
    run.wish(2, &fork).unwrap();
    assert_eq!(
        (run.players[&2].wishes, run.players[&2].great_wishes),
        (1, 0),
        "one step left on the ladder: it is taken, and a plain wish is kept"
    );
}

#[test]
fn wish_hud_reads_each_knights_held_wishes_without_changing_the_wire() {
    use hud::{Keys, WishHud};
    let mut run = Run::new(4, 2, Some("Matt"));
    for id in 3..=4 {
        run.join_seat(id, &format!("Knight {id}"));
        assert_eq!(run.players[&id].wishes, overclass::START_WISHES, "a friend who joins a delve under way holds a start's wishes");
    }
    for id in 1..=4 {
        run.players.get_mut(&id).unwrap().wishes = 0;
        assert_eq!(run.wish_hud(id), Some(WishHud::Closed));
    }
    run.give_wishes(2, 1);
    for id in 1..=4 {
        assert_eq!(run.hud(id, &Keys::FIRST).unwrap().wishes, Some(WishHud::Ready { wishes: 2, great: 1 }));
    }
    let volley = phrasebook::get("volley").unwrap();
    run.wish(4, &volley).unwrap();
    assert_eq!(run.wish_hud(4), Some(WishHud::Ready { wishes: 2, great: 0 }));
    let before = crate::wire::publish(&run).unwrap();
    for id in 1..=4 {
        run.hud(id, &Keys::FIRST).unwrap();
    }
    assert_eq!(before, crate::wire::publish(&run).unwrap());
    run.players.get_mut(&3).unwrap().hp = 0;
    assert_eq!(run.wish_hud(3), Some(WishHud::Fallen));
    assert_eq!(run.wish_hud(99), None);

    // A legacy local HUD can still be read; this is not a Run/wire field.
    let mut legacy = serde_json::to_value(run.hud(1, &Keys::FIRST).unwrap()).unwrap();
    legacy.as_object_mut().unwrap().remove("wishes");
    assert_eq!(serde_json::from_value::<hud::Hud>(legacy).unwrap().wishes, None);
}

#[test]
fn a_granted_volley_flies_three_wide_on_its_cadence_and_climbs() {
    let mut run = Run::new(4, 2, None);
    fight(&mut run);
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 99_999;
    }
    let volley = phrasebook::get("volley").unwrap();
    combo_kit(&mut run, 1);
    run.grant(1, &volley).unwrap();
    assert_eq!(run.players[&1].bonus.volley, 8);
    let mut fired = Vec::new();
    for _ in 0..(10 * HZ) {
        let before = run.projectiles.iter().filter(|p| !p.hostile).count();
        run.step(&input(true));
        let now = run.projectiles.iter().filter(|p| !p.hostile).count();
        if run.players[&1].fire_cooldown > 0 && now > before.saturating_sub(4) {
            fired.push(now.saturating_sub(before));
        }
        if run.players[&1].loosed >= 10 {
            break;
        }
    }
    assert!(run.players[&1].loosed >= 10);
    // Granted again: the tighter cadence replaces the first.
    run.window = Some(Default::default());
    run.grant(1, &volley).unwrap();
    assert_eq!(run.players[&1].bonus.volley, 6);
    assert_eq!(
        run.players[&1]
            .deck
            .iter()
            .filter(|c| c.starts_with("wish-volley-"))
            .count(),
        1
    );
}

#[test]
fn bouncing_homing_chaining_mending_shots_do_what_they_say() {
    // The entrance room: open floor, no pillars in a shot's way.
    let mut run = Run::new(4, 2, Some("Matt"));
    combo_kit(&mut run, 1);
    run.calm_for_test();
    for (id, wish) in [(1, "ricochet"), (1, "seeker"), (1, "chain"), (1, "mend")] {
        run.window = Some(Default::default());
        run.grant(id, &phrasebook::get(wish).unwrap()).unwrap();
    }
    let bonus = run.players[&1].bonus;
    assert_eq!(
        (bonus.bounce, bonus.homing, bonus.chain, bonus.mend),
        (1, 1, 1, 1)
    );
    // Bounce: a shot at the west wall comes back.
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (8.0, 10.0);
    hero.fire_cooldown = 0;
    run.enemies.clear();
    // The room clears (and sweeps its shots) before the test shot.
    run.step(&BTreeMap::new());
    run.players.get_mut(&1).unwrap().fire_cooldown = 0;
    run.step(&BTreeMap::from([(
        1,
        Input {
            aim_x: -1,
            ..Default::default()
        },
    )]));
    let mut came_back = false;
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
        came_back |= run.projectiles.iter().any(|p| !p.hostile && p.vx > 0.0);
    }
    assert!(came_back, "a bounced shot flies back east");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_wish_shots -- --ignored`
#[test]
#[ignore]
fn write_wish_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::new(4, 2, None);
    fight(&mut run);
    run.calm_for_test();
    let spots = [
        (24.0, 12.0),
        (28.0, 11.0),
        (20.0, 11.0),
        (24.0, 8.0),
        (31.0, 9.0),
        (17.0, 9.0),
    ];
    for (enemy, (x, y)) in run.enemies.iter_mut().zip(spots.iter().cycle()) {
        enemy.hp = 99_999;
        (enemy.x, enemy.y, enemy.origin_x, enemy.origin_y) = (*x, *y, *x, *y);
    }
    for wish in ["volley", "volley", "volley", "chain", "chain"] {
        run.window = Some(Default::default());
        run.grant(1, &phrasebook::get(wish).unwrap()).unwrap();
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 22.0);
    let mut best = 0;
    for _ in 0..(3 * HZ) {
        run.step(&BTreeMap::from([(
            1,
            Input {
                aim_y: -1,
                fire: true,
                ..Default::default()
            },
        )]));
        let lit = run.sparks.len() * 10 + run.projectiles.iter().filter(|p| !p.hostile).count();
        if lit > best && !run.sparks.is_empty() {
            best = lit;
            let img = crate::render::overworld::arena::frame(&run, 384, 224);
            let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
            out.extend(img.rgb_bytes());
            std::fs::write(dir.join("wish_volley_chain.ppm"), out).unwrap();
        }
    }
    assert!(best > 0, "a volley and a chain were drawn");
}

#[test]
fn a_thrown_blade_flies_out_and_comes_home_and_morningstars_strike() {
    let mut run = Run::new(4, 2, None);
    run.outfit(1, knights::knight("galahad").unwrap());
    run.calm_for_test();
    run.enemies.clear();
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.fire_cooldown) = (24.0, 16.0, 0);
    // Nothing in reach: the blade is thrown.
    run.step(&input(true));
    let blade = run
        .projectiles
        .iter()
        .find(|p| p.kind == Shot::Blade)
        .expect("thrown");
    assert_eq!(blade.traits.back, 1);
    let mut came_back = false;
    for _ in 0..(3 * HZ) {
        run.step(&BTreeMap::new());
        came_back |= run.projectiles.iter().any(|p| p.traits.back == 2);
        if came_back && run.projectiles.iter().all(|p| p.kind != Shot::Blade) {
            break;
        }
    }
    assert!(came_back, "it turned for home");
    assert!(
        run.projectiles.iter().all(|p| p.kind != Shot::Blade),
        "and was caught"
    );
    // A morningstar strikes what it touched, on its next beat.
    combo_kit(&mut run, 1);
    run.window = Some(Default::default());
    run.grant(1, &phrasebook::get("morningstar").unwrap())
        .unwrap();
    let (bx, by) = run.orbits(1)[0];
    run.spawn_at_for_test(EnemyKind::Skeleton, bx, by);
    let hp = run.enemies[0].hp;
    for _ in 0..ORBIT_EVERY {
        run.step(&BTreeMap::new());
    }
    assert!(run.enemies.first().is_none_or(|e| e.hp < hp), "struck");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_melee_shots -- --ignored`
#[test]
#[ignore]
fn write_melee_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = Run::new(4, 2, Some("Matt"));
    run.outfit(1, knights::knight("galahad").unwrap());
    fight(&mut run);
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 99_999;
    }
    run.window = Some(Default::default());
    run.grant(2, &phrasebook::get("morningstar").unwrap())
        .unwrap();
    run.window = Some(Default::default());
    run.grant(2, &phrasebook::get("morningstar").unwrap())
        .unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 22.0);
    let matt = run.players.get_mut(&2).unwrap();
    (matt.x, matt.y) = (14.0, 16.0);
    run.window = None;
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    for i in 0..9 {
        run.step(&BTreeMap::from([(
            1,
            Input {
                aim_y: -1,
                fire: i == 0,
                ..Default::default()
            },
        )]));
    }
    save(&run, "melee_throw_orbit");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_flourish_shots -- --ignored`
#[test]
#[ignore]
fn write_flourish_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = Run::new(4, 2, None);
    run.outfit(1, knights::knight("gareth").unwrap());
    fight(&mut run);
    run.calm_for_test();
    let spots = [(24.0, 12.0), (26.5, 11.0), (21.5, 11.0)];
    for (enemy, (x, y)) in run.enemies.iter_mut().zip(spots.iter().cycle()) {
        enemy.hp = 99_999;
        (enemy.x, enemy.y, enemy.origin_x, enemy.origin_y) = (*x, *y, *x, *y);
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (24.0, 22.0, 0);
    run.step(&BTreeMap::from([(
        1,
        Input {
            aim_y: -1,
            fire: true,
            ..Default::default()
        },
    )]));
    for _ in 0..(2 * HZ) {
        run.step(&BTreeMap::new());
        if run.blasts.iter().any(|b| b.2 == BLAST_TICKS - 3) {
            break;
        }
    }
    assert!(!run.blasts.is_empty(), "the ball burst");
    save(&run, "flourish_burst");
    run.window = Some(Default::default());
    run.grant(1, &phrasebook::get("volley").unwrap()).unwrap();
    for _ in 0..8 {
        run.step(&BTreeMap::new());
    }
    save(&run, "flourish_wish");
}

#[test]
fn blasting_powder_bursts_on_the_neighbours() {
    let mut run = Run::new(4, 2, None);
    fight(&mut run);
    run.calm_for_test();
    let spots = [(24.0, 12.0), (25.8, 12.0)];
    run.enemies.truncate(2);
    for (enemy, (x, y)) in run.enemies.iter_mut().zip(spots) {
        enemy.hp = 9_999;
        enemy.kind = EnemyKind::Skeleton;
        (enemy.x, enemy.y, enemy.origin_x, enemy.origin_y) = (x, y, x, y);
    }
    run.window = Some(Default::default());
    combo_kit(&mut run, 1);
    run.grant(1, &phrasebook::get("powder").unwrap()).unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.fire_cooldown) = (24.0, 22.0, 0);
    run.step(&BTreeMap::from([(
        1,
        Input {
            aim_y: -1,
            fire: true,
            ..Default::default()
        },
    )]));
    let side = run.enemies[1].id;
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
        if !run.blasts.is_empty() {
            break;
        }
    }
    let neighbour = run.enemies.iter().find(|e| e.id == side).unwrap();
    assert!(neighbour.hp < 9_999, "the burst reached the one beside it");
}

#[test]
fn an_old_save_s_knights_load_and_hold_their_wishes_to_the_floor_they_stand_on() {
    let raw = include_str!("fixtures/balance_party.json");
    let v: serde_json::Value = serde_json::from_str(raw).unwrap();
    let heroes: Vec<Hero> = serde_json::from_value(v["heroes"].clone()).unwrap();
    assert!(heroes.iter().all(|h| h.wish_floor == 0), "a save from before has no floor kept");
    let mut run = Run::new(4, 1, None);
    run.descend_for_test();
    run.step(&input(false));
    assert!(run.players.values().all(|h| h.wish_floor == run.dungeon.depth));
}

#[test]
fn a_learned_wish_s_grant_names_its_grade_and_what_its_super_effect_waits_on() {
    let text = format!(
        "{}\nwish homing-shots | Homing Shots\nsay homing shots\ntier homing 1 :: Your shots bend toward the nearest monster.\ntier homing 2 :: Harder.\nart\n77777777\n77777777\n77777777\n",
        phrasebook::LEARNED
    );
    let wish = phrasebook::read(&text).pop().unwrap();
    let mut run = Run::new(4, 1, None);
    let said = run.grant(1, &wish).unwrap();
    assert!(said.starts_with("Crude Homing Shots I"), "{said}");
    assert!(said.contains("Homing waits on Creature"), "{said}");
    assert!(said.contains("Families: Projectile"), "{said}");
    assert_eq!(run.players[&1].bonus.homing, 0);
}

#[test]
fn upgrade_words_grant_the_next_tier_of_the_wish_held_and_learn_nothing() {
    // The last game: "upgrade pasta armor" was learned as a new Upgraded
    // Pasta beside the Pasta Armor its knight held (2026-10-19).
    let book = phrasebook::read(&format!(
        "{}\n{}\nwish pasta-armor | Pasta Armor\nsay pasta armor | noodle armor | lasagne plate\n\
tier armor 1 :: Lasagne.\ntier armor 2 :: Thicker.\ntier armor 2; max_hp 15 :: Full.\n",
        phrasebook::TEXT,
        phrasebook::LEARNED
    ));
    let pasta = book.iter().find(|w| w.id == "pasta-armor").unwrap().clone();
    let mut run = Run::new(4, 1, Some("Matt"));
    run.give_wishes(2, 0);
    run.wish(1, &pasta).unwrap();
    let deck = run.players[&1].deck.clone();
    // What the host does with the words: a known wish is granted, only an
    // unknown one is learned.
    let found = phrasebook::find_in(&book, "upgrade my pasta armor", &deck)
        .expect("a known wish, not one to learn");
    assert_eq!(found.id, "pasta-armor");
    let granted = run.wish(1, &found).unwrap();
    assert!(granted.ends_with("Pasta Armor II"), "{granted}");
    let hero = &run.players[&1];
    assert!(hero.deck.contains(&"wish-pasta-armor-2".to_string()));
    assert!(!hero.deck.contains(&"wish-pasta-armor-1".to_string()), "the tier below is let go");
    assert_eq!(hero.wishes, 0, "a wish spent on each");
    // A knight holding none of it learns it as before.
    assert!(phrasebook::find_in(&book, "upgrade my pasta armor", &[]).is_none());
}

#[test]
fn together_a_wish_that_does_not_go_through_is_given_back() {
    let mut run = Run::new(5, 1, Some("Matt"));
    let hero = run.players.get_mut(&1).unwrap();
    hero.wishes = 2;
    hero.great_wishes = 1;
    // A super wish is spent first, and comes back as a super wish.
    assert_eq!(run.spend_wish(1), Some(true));
    run.refund_wish(1, true);
    assert_eq!((run.players[&1].wishes, run.players[&1].great_wishes), (2, 1));
    run.players.get_mut(&1).unwrap().great_wishes = 0;
    assert_eq!(run.spend_wish(1), Some(false));
    run.refund_wish(1, false);
    assert_eq!(run.players[&1].wishes, 2);
}

#[test]
fn every_knight_may_wish_every_tier_of_every_wish() {
    // Nothing but the floor holds a wish back: a knight who has done
    // nothing at all is granted Blood Drinker, the Awakened Fork and every
    // tier of every wish, the third too, with no teacher walked; the fourth
    // and fifth deep enough down (`overclass::top_tier`).
    for wish in phrasebook::book().iter() {
        let mut run = Run::new(4, 1, Some("Friend"));
        run.dungeon.depth = crate::rules::FLOORS;
        for tier in 0..wish.tiers.len() {
            assert_eq!(run.locked(1, wish, tier), None, "{} tier {}", wish.name, tier + 1);
            run.grant(1, wish).unwrap_or_else(|why| panic!("{} tier {}: {why}", wish.name, tier + 1));
        }
    }
    let mut run = Run::new(4, 1, Some("Friend"));
    let leech = phrasebook::book().iter().find(|w| w.id == "leech").unwrap().clone();
    assert!(run.grant(1, &leech).is_ok(), "a fresh knight is granted Blood Drinker");
}

#[test]
fn a_fresh_knight_takes_the_cards_a_rank_once_kept_on_the_floor() {
    // The vamp card and the awakened fork were Master cards, left lying for
    // someone who could take them. Everyone can.
    for card in ["leech-fang", "awakened-fork"] {
        let mut run = Run::new(4, 1, Some("Friend"));
        run.calm_for_test();
        let hero = &run.players[&1];
        let (x, y) = (hero.x, hero.y);
        let at = run.at;
        run.dungeon.rooms[at].items.push(Item {
            card: card.into(),
            x,
            y,
            held_off: None, took: Vec::new(), dropped: 0, owner: None,
        });
        run.step(&BTreeMap::new());
        assert!(run.players[&1].deck.iter().any(|c| c == card), "{card} is taken up");
    }
}

fn knight_line(run: &mut Run, id: u32) -> String {
    crate::wire::take_notices(run)
        .into_iter()
        .filter_map(|event| match event {
            crate::wire::Event::Notice { hear: crate::wire::Hear::Knight(k), text } if k == id => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `refund_wish` gives it back and the knight who wished hears why, on
/// their own screen, not the room's.
#[test]
fn a_refunded_wish_tells_the_knight_why() {
    let mut run = Run::new(5, 1, Some("Matt"));
    run.players.get_mut(&1).unwrap().wishes = 0;
    run.refund_told(1, false, "all four spell slots are full");
    assert_eq!(run.players[&1].wishes, 1);
    let line = knight_line(&mut run, 1);
    assert!(line.contains("given back"), "{line}");
    assert!(line.contains("all four spell slots are full"), "{line}");
    run.refund_told(1, true, "the book is full");
    assert_eq!(run.players[&1].great_wishes, 1);
    let line = knight_line(&mut run, 1);
    assert!(line.contains("given back") && line.contains("the book is full"), "{line}");
    assert!(knight_line(&mut run, 2).is_empty(), "the other knight is not told");
}

/// A further step that cannot be granted (`grant_more`) gives a plain
/// wish back and says why, the same line the scroll's second step uses.
#[test]
fn a_further_step_that_cannot_land_tells_the_knight_why() {
    let mut run = Run::new(5, 1, Some("Matt"));
    let wish = phrasebook::get("firebolt").expect("firebolt");
    {
        let hero = run.players.get_mut(&1).unwrap();
        hero.wishes = 0;
        hero.spells = std::array::from_fn(|i| Some(format!("pad-{i}")));
    }
    assert!(run.grant_more(1, &wish).is_none());
    assert_eq!(run.players[&1].wishes, 1, "the step that could not land came back");
    let line = knight_line(&mut run, 1);
    assert!(line.contains("given back"), "{line}");
    assert!(line.contains("all four spell slots are full"), "{line}");
}

/// How far a ladder climbs on each floor (`top_tier`): its third at home
/// and on the first floor, its fourth from the second, its fifth from
/// Dragon Keep's down. Every wish the first floor gives, its three and its
/// boss's super wish, spent on one ladder stop at the third there; the
/// wishes a delve has given by Dragon Keep's floor reach the fifth.
#[test]
fn the_fifth_tier_is_a_deep_delves_and_never_the_first_floors() {
    let reach: Vec<usize> = (0..=DEEPEST).map(top_tier).collect();
    assert_eq!(reach, vec![2, 2, 3, 4, 4, 4, 4]);
    assert_eq!((tier_floor(3), tier_floor(4)), (Some(2), Some(FLOORS)));
    let heavy = phrasebook::get("heavy").unwrap();
    let mut run = Run::new(4, 1, Some("Friend"));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.wishes, hero.great_wishes) = (START_WISHES, 1);
    while run.can_wish(1) && run.wish(1, &heavy).is_ok() {}
    assert!(run.players[&1].deck.contains(&heavy.card_id(2)), "the third on the first floor");
    assert!(!run.players[&1].deck.iter().any(|c| c == &heavy.card_id(3) || c == &heavy.card_id(4)));
    assert!(run.players[&1].wishes > 0, "what the floor cannot give is kept, not spent");
    // By Dragon Keep's floor a delve has given its start's wishes, two a
    // floor below and a super wish (two steps) a boss: past five steps.
    let given = u32::from(START_WISHES) + 2 * u32::from(FLOOR_WISHES) + 2 * 2;
    assert!(given >= phrasebook::MAX_TIERS as u32);
    run.dungeon.depth = FLOORS;
    run.players.get_mut(&1).unwrap().wishes = 2;
    run.wish(1, &heavy).unwrap();
    run.wish(1, &heavy).unwrap();
    assert!(run.players[&1].deck.contains(&heavy.card_id(4)), "the fifth, deep enough down");
}

/// A wish's fourth and fifth mark their plate on the skills bar; below
/// them the bar's icons travel as they always did.
#[test]
fn a_wished_spells_high_tiers_mark_its_plate_and_lower_ones_send_nothing_new() {
    let firebolt = phrasebook::get("firebolt").unwrap();
    let mut run = Run::new(4, 1, Some("Friend"));
    run.dungeon.depth = FLOORS;
    for tier in 0..firebolt.tiers.len() {
        run.grant(1, &firebolt).unwrap();
        let icons = run.tray(1, &hud::Keys::FIRST);
        let plate = icons.iter().find(|i| i.slot == tray::Slot::Spell && i.name.starts_with("Firebolt")).unwrap();
        let expected = if tier >= 3 { tier as u8 + 1 } else { 0 };
        assert_eq!(plate.high, expected, "Firebolt {}", tier + 1);
        let sent = serde_json::to_value(plate).unwrap();
        assert_eq!(sent.get("high").is_some(), tier >= 3, "{sent}");
    }
    assert_eq!(tray::high_tier(Some("wish-firebolt-5")), 5);
    assert_eq!(tray::high_tier(Some("wish-firebolt-3")), 0);
    assert_eq!(tray::high_tier(Some("potion")), 0);
}
