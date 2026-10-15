use super::spectacle::{Beat, CLAIM, GATE_RISE, LOOT_WAIT, STRUCK};
use super::*;
use crate::render::overworld::Img;

fn save(img: &Img, dir: &std::path::Path, name: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
}

fn shots_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from)
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

/// A calm, empty room with the knight at its middle.
fn quiet() -> Run {
    let mut run = Run::new(4, 1, None);
    fight(&mut run);
    run.calm_for_test();
    run.enemies.clear();
    run.window = None;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    run
}

/// Hurt knight 1 the way a monster does, inside a tick the watcher sees.
fn hurt(run: &mut Run, damage: u32) {
    run.hurt_for_test(1, damage);
}

fn idle(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        run.step(&BTreeMap::new());
    }
}

fn frame(run: &Run) -> Img {
    let mut run = run.clone();
    run.window = None;
    crate::render::overworld::arena::frame(&run, 384, 224)
}

/// A guardian standing in the room, struck dead on the next step.
fn slay_guardian(run: &mut Run) {
    run.spawn_boss(0);
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
}

#[test]
fn a_fallen_guardian_is_claimed_a_few_seconds_after_its_loot_drops() {
    let mut run = quiet();
    slay_guardian(&mut run);
    let fell = run.tick;
    let loot = run.room().items.len();
    assert!(loot > 0, "the guardian's loot drops at once");
    assert_eq!(run.spectacle.corpses.len(), 1, "its body is left behind");
    assert_eq!(run.spectacle.corpses[0].beat(run.tick), Beat::Lying);
    assert!(run.sounds.contains(&"boss_death"));
    // The loot lies about it for LOOT_WAIT ticks, nothing cracking yet.
    let mut heard: Vec<(u64, &'static str)> = Vec::new();
    let mut shook = 0;
    while run.tick < fell + u64::from(CLAIM) + 5 {
        run.players.get_mut(&1).unwrap().invulnerable = 100_000;
        run.sounds.clear();
        run.step(&BTreeMap::new());
        for &s in &run.sounds {
            heard.push((run.tick - fell, s));
        }
        shook = shook.max(run.shake);
        let age = run.tick - fell;
        let beat = run.spectacle.corpses.first().map(|c| c.beat(run.tick));
        if age < u64::from(LOOT_WAIT) {
            assert_eq!(beat, Some(Beat::Lying), "age {age}");
        }
    }
    let at = |name: &str| heard.iter().find(|(_, s)| *s == name).map(|(t, _)| *t);
    let order = [
        "crack_open",
        "hand_rise",
        "hand_grab",
        "hand_drag",
        "ground_close",
    ];
    let times: Vec<u64> = order.iter().map(|n| at(n).expect(n)).collect();
    assert_eq!(
        times[0],
        u64::from(LOOT_WAIT),
        "the crack comes after the loot"
    );
    assert!(times.windows(2).all(|w| w[0] < w[1]), "{times:?}");
    assert!(shook >= 4, "the ground shakes as it drags");
    assert!(
        run.spectacle.corpses.is_empty(),
        "the body is gone with the hand"
    );
    assert!(
        run.room().items.len() >= loot,
        "the loot stays: {} of {loot}",
        run.room().items.len()
    );
}

#[test]
fn leaving_the_room_ends_the_claim_unseen_and_it_never_comes_back() {
    let mut run = quiet();
    slay_guardian(&mut run);
    idle(&mut run, 10);
    let here = run.at;
    let other = run
        .dungeon
        .rooms
        .iter()
        .enumerate()
        .position(|(i, r)| r.kind == RoomKind::Fight && i != here)
        .unwrap();
    run.enter_for_test(other);
    run.calm_for_test();
    run.enemies.clear();
    idle(&mut run, 2);
    assert!(run.spectacle.corpses.is_empty());
}

#[test]
fn a_hit_knight_flinches_flashes_warm_and_a_heavy_hit_shakes_the_view() {
    let mut run = quiet();
    run.players.get_mut(&1).unwrap().invulnerable = 0;
    run.spawn_at_for_test(EnemyKind::Skeleton, 22.5, 14.0);
    let before = frame(&run);
    // A heavy blow.
    {
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = 100;
        hero.max_hp = 100;
    }
    hurt(&mut run, 40);
    // The tick's shake reaches the picture when the tick is presented.
    run.present();
    let hero = &run.players[&1];
    assert_eq!(hero.struck, STRUCK, "the flinch starts");
    assert!(hero.struck_big);
    assert!(
        hero.struck_dir.0 > 0,
        "thrown away from the skeleton on its left"
    );
    assert!(run.shake > 0, "a heavy hit shakes the view");
    assert!(run.sounds.contains(&"knight_hit_big"));
    // The flash retains the knight's volume; its warm highlights and the
    // chest's tiny bright sparks read the hit without a solid white figure.
    let hit = frame(&run);
    assert_ne!(before, hit);
    let count = |img: &Img, colour| img.pixels().filter(|&c|c==colour).count();
    let warm=[0xe3,0xd2,0xc3];
    let ivory=[0xe4,0xff,0xf8];
    assert!(count(&hit,warm)>count(&before,warm),"the flash's warm planes read the blow");
    assert!(count(&hit,ivory)<20,"no solid white silhouette");
    run.enemies.clear();
    idle(&mut run,8);
    let later=frame(&run);
    assert!(count(&later,warm)<count(&hit,warm),"the warm flash fades after its held frames");
    idle(&mut run, STRUCK);
    assert_eq!(run.players[&1].struck, 0);
}

#[test]
fn a_light_hit_is_not_heavy() {
    let mut run = quiet();
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    hero.hp = 100;
    hurt(&mut run, 4);
    assert!(run.players[&1].struck > 0);
    assert!(!run.players[&1].struck_big);
    assert!(run.sounds.contains(&"knight_hit"));
}

#[test]
fn a_fallen_knight_is_bones_that_scatter_and_stay_until_revived() {
    let mut run = quiet();
    run.players.get_mut(&1).unwrap().invulnerable = 0;
    let alive = frame(&run);
    run.players.get_mut(&1).unwrap().hp = 3;
    hurt(&mut run, 30);
    let hero = &run.players[&1];
    assert_eq!(hero.hp, 0);
    assert_eq!(hero.fell, run.tick.max(1), "the tick they fell is kept");
    assert!(run.sounds.contains(&"knight_fall"));
    let early = frame(&run);
    idle(&mut run, 30);
    let settled = frame(&run);
    idle(&mut run, 60);
    let later = frame(&run);
    assert_ne!(alive, early);
    assert_ne!(early, settled, "the bones are still falling apart");
    assert_eq!(settled, later, "then they lie where they fell");
    // The bones clatter once they land.
    let mut run2 = Run::new(4, 1, Some("Matt"));
    fight(&mut run2);
    run2.calm_for_test();
    run2.enemies.clear();
    run2.players.get_mut(&1).unwrap().invulnerable = 0;
    run2.players.get_mut(&1).unwrap().hp = 3;
    hurt(&mut run2, 30);
    let mut heard = false;
    for _ in 0..20 {
        run2.step(&BTreeMap::new());
        heard |= run2.sounds.contains(&"bones_clatter");
    }
    assert!(heard);
    // Revival (the stairs) puts the knight back on their feet, no bones.
    assert_eq!(run2.players[&1].hp, 0);
    run2.descend_for_test();
    idle(&mut run2, 1);
    assert!(run2.players[&1].hp > 0);
    assert_eq!(run2.players[&1].fell, 0);
}

#[test]
fn the_party_still_wipes_when_every_knight_has_fallen() {
    let mut run = quiet();
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    hero.hp = 1;
    hurt(&mut run, 50);
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Wiped);
}

fn stand_at_gate(run: &mut Run) {
    let g = run.spectacle.gate.expect("the gate");
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (g.x, g.y);
}

#[test]
fn the_dragons_death_raises_a_hell_gate_that_opens_the_first_circle() {
    let mut run = Run::new(4, 1, None);
    run.lair_for_test();
    run.calm_for_test();
    assert_eq!(run.circle(), 0);
    assert!(run.spectacle.gate.is_none());
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert!(run.light.is_some(), "the way home is still there");
    let gate = run.spectacle.gate.expect("a fiery gate rises");
    assert!(run.sounds.contains(&"hell_gate_rise"));
    // It takes a moment to rise; stepping in before then does nothing.
    stand_at_gate(&mut run);
    run.step(&BTreeMap::new());
    assert_eq!(run.circle(), 0);
    assert_eq!(run.dungeon.depth, FLOORS);
    // Step away and wait for it to stand.
    run.players.get_mut(&1).unwrap().x = 10.0;
    idle(&mut run, GATE_RISE);
    assert!(gate.risen(run.tick) >= 1.0);
    stand_at_gate(&mut run);
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    run.step(&BTreeMap::new());
    assert_eq!(run.circle(), 1, "through the gate");
    assert_eq!(run.dungeon.depth, 1, "the delve starts over, a circle down");
    assert!(run.spectacle.gate.is_none());
    let (circle, _) = run.circle_card().expect("the title card shows");
    assert_eq!(circle, 1);
    assert_eq!(spectacle::circle_name(1), "First Circle");
    assert_eq!(spectacle::circle_name(2), "Second Circle");
    assert_eq!(spectacle::circle_title(1), "Limbo");
    assert!(run.sounds.contains(&"hell_gate_enter"));
    assert!(run.valid_snapshot());
}

#[test]
fn a_new_circle_brings_new_privies_and_their_toll() {
    let mut run = Run::new(4, 1, None);
    run.lair_for_test();
    run.calm_for_test();
    // Relieved on the first floor's privy, the first time down.
    {
        let hero = run.players.get_mut(&1).unwrap();
        hero.relieved = Some((1, 0));
        hero.pit.visits = 2;
    }
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    run.players.get_mut(&1).unwrap().x = 10.0;
    idle(&mut run, GATE_RISE);
    stand_at_gate(&mut run);
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    run.step(&BTreeMap::new());
    assert_eq!(run.circle(), 1);
    let hero = &run.players[&1];
    assert_eq!(hero.relieved, None, "floor one's privy is open again");
    assert_eq!(hero.pit.visits, 0);
}

#[test]
fn the_end_of_each_circle_opens_the_next_gate() {
    let mut run = Run::new(4, 1, None);
    run.spectacle.circle = 1;
    run.lair_for_test();
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert!(
        run.spectacle.gate.is_some(),
        "a gate at the end of the circle too"
    );
    run.players.get_mut(&1).unwrap().x = 10.0;
    idle(&mut run, GATE_RISE);
    stand_at_gate(&mut run);
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    run.step(&BTreeMap::new());
    assert_eq!(run.circle(), 2);
    assert_eq!(run.dungeon.depth, 1);
    assert_eq!(run.circle_card().map(|c| c.0), Some(2));
    assert_eq!(spectacle::circle_name(2), "Second Circle");
}

#[test]
fn each_circle_is_a_little_harder() {
    let mut above = Run::new(4, 1, None);
    above.descend_for_test();
    fight(&mut above);
    idle(&mut above, 1);
    let plain: u32 = above.enemies.iter().map(|e| e.max_hp).sum();
    let mut below = Run::new(4, 1, None);
    below.spectacle.circle = 2;
    below.descend_for_test();
    fight(&mut below);
    idle(&mut below, 1);
    let hard: u32 = below.enemies.iter().map(|e| e.max_hp).sum();
    assert!(hard > plain, "{hard} against {plain}");
    // Their hit points are scaled once, not on every tick.
    idle(&mut below, 5);
    assert_eq!(hard, below.enemies.iter().map(|e| e.max_hp).sum::<u32>());
}

#[test]
fn the_tray_shows_icons_for_the_weapon_guard_bombs_and_cards() {
    let mut run = quiet();
    run.players.get_mut(&1).unwrap().hand = vec!["potion".into(), "holy-water".into()];
    let tray = run.tray(1, &hud::Keys::FIRST);
    let slots: Vec<_> = tray.iter().map(|i| i.slot).collect();
    use tray::Slot;
    // Weapon, guard, the spells on their keys, the ultimate, then the bombs
    // and the hand, the first card on the quaff key.
    assert_eq!(slots[0], Slot::Weapon);
    assert_eq!(slots[1], Slot::Guard);
    assert_eq!(slots[2], Slot::Ult);
    assert_eq!(slots[3], Slot::Bomb);
    assert_eq!(slots.iter().filter(|s| **s == Slot::Card).count(), 2);
    assert_eq!(*slots.last().unwrap(), Slot::Card);
    assert_eq!(tray[4].key, "Z");
    assert_eq!(tray[5].key, "", "the rest wait their turn");
    for icon in &tray {
        let rows = icon.rows();
        assert!(rows.len() <= tray::SIZE, "{}", icon.name);
        assert!(rows.iter().all(|r| r.chars().count() <= tray::SIZE));
        assert!(
            rows.iter().any(|r| r.chars().any(|c| c != '.')),
            "{} is blank",
            icon.name
        );
    }
    assert_eq!(tray[3].count, 2, "two bombs");
    // A card too big for a plate is shrunk, not cut.
    let big: Vec<String> = (0..16).map(|_| "7".repeat(16)).collect();
    let fitted = tray::fit(&big.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(fitted.split('/').count(), tray::SIZE);
}

#[test]
fn the_tray_is_in_the_bottom_corner_of_the_picture_and_not_when_fallen() {
    let mut run = quiet();
    let with = frame(&run);
    run.players.get_mut(&1).unwrap().hp = 0;
    let without = frame(&run);
    // Something differs in the bottom-right corner, where the tray sits.
    let mut changed = 0;
    for y in 150..224 {
        for x in 200..384 {
            if with.get(x, y) != without.get(x, y) {
                changed += 1;
            }
        }
    }
    assert!(changed > 100, "{changed}");
}

#[test]
fn walking_and_gear_and_landed_hits_make_their_sounds() {
    let mut run = quiet();
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    let go = Input {
        move_x: 1,
        ..Default::default()
    };
    let mut steps = 0;
    for _ in 0..40 {
        run.sounds.clear();
        run.step(&BTreeMap::from([(1, go)]));
        steps += run.sounds.iter().filter(|s| s.starts_with("step_")).count();
    }
    // Footsteps are off: they drowned out the shots.
    assert_eq!(steps, 0, "{steps}");
    assert!(
        run.sounds.iter().all(|s| !s.starts_with("step_stone")) || run.dungeon.pack == Pack::Crypt
    );
    // A shot landing on a monster sounds as its weapon.
    let mut run = quiet();
    run.spawn_at_for_test(EnemyKind::Skeleton, 28.0, 14.0);
    let e = run.enemies.last_mut().unwrap();
    e.hp = 900;
    e.max_hp = 900;
    e.age = 100;
    run.sounds.clear();
    let id = run.enemies[0].id;
    // Strike it the way a shot does, between two steps.
    let before = run.seen();
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().hp -= 30;
    run.watch(before);
    assert!(run.sounds.contains(&"hit_arrow"), "{:?}", run.sounds);
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_spectacle_shots -- --ignored`
#[test]
#[ignore]
fn write_spectacle_shots() {
    let Some(dir) = shots_dir() else {
        return;
    };
    // The player hit: white, thrown back, sparks; a light hit and a heavy one.
    for (name, damage, kit) in [
        ("hit_light", 8, "percival"),
        ("hit_heavy", 40, "percival"),
        ("hit_dwarf", 30, "ironbraid"),
    ] {
        let mut run = quiet();
        run.outfit(1, knights::knight(kit).unwrap());
        run.spawn_at_for_test(EnemyKind::Skeleton, 19.0, 13.0);
        {
            let hero = run.players.get_mut(&1).unwrap();
            hero.invulnerable = 0;
            hero.hp = 100;
            hero.max_hp = 100;
        }
        hurt(&mut run, damage);
        // The blow's shake reaches the picture as the tick is presented.
        run.present();
        save(&frame(&run), &dir, &format!("{name}_00"));
        for n in 1..STRUCK {
            run.step(&BTreeMap::new());
            run.enemies[0].frozen = 100;
            if [1, 3, 5, 8, 12].contains(&n) {
                save(&frame(&run), &dir, &format!("{name}_{n:02}"));
            }
        }
    }

    // Death: the skeleton comes apart (a friend stands by, so the party goes on).
    for (kit, name) in [("percival", "bones_knight"), ("ironbraid", "bones_dwarf")] {
        let mut run = Run::new(4, 1, Some("Matt"));
        fight(&mut run);
        run.calm_for_test();
        run.enemies.clear();
        run.window = None;
        run.outfit(1, knights::knight(kit).unwrap());
        run.players.get_mut(&1).unwrap().x = 16.0;
        run.players.get_mut(&1).unwrap().y = 14.0;
        run.players.get_mut(&2).unwrap().x = 34.0;
        run.players.get_mut(&2).unwrap().y = 18.0;
        run.players.get_mut(&2).unwrap().invulnerable = 100_000;
        {
            let hero = run.players.get_mut(&1).unwrap();
            hero.invulnerable = 0;
            hero.hp = 2;
        }
        hurt(&mut run, 40);
        run.present();
        for n in 0..40 {
            if [0, 3, 6, 10, 15, 22, 39].contains(&n) {
                save(&frame(&run), &dir, &format!("{name}_{n:02}"));
            }
            run.step(&BTreeMap::new());
        }
    }
    // The demon hand.
    let mut run = quiet();
    run.spawn_boss(0);
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    let fell = run.tick;
    let marks = [
        (30, "a_lying"),
        (LOOT_WAIT + 14, "b_cracking"),
        (LOOT_WAIT + 30, "c_cracked"),
        (LOOT_WAIT + 36 + 14, "d_rising_a"),
        (LOOT_WAIT + 36 + 32, "e_rising_b"),
        (LOOT_WAIT + 36 + 45 + 6, "f_grabbing"),
        (LOOT_WAIT + 36 + 45 + 18 + 12, "g_pulling_a"),
        (LOOT_WAIT + 36 + 45 + 18 + 36, "h_pulling_b"),
        (LOOT_WAIT + 36 + 45 + 18 + 58, "i_pulling_c"),
        (LOOT_WAIT + 36 + 45 + 18 + 66 + 12, "j_closing"),
        (CLAIM - 2, "k_shut"),
    ];
    while run.tick < fell + u64::from(CLAIM) {
        run.players.get_mut(&1).unwrap().invulnerable = 100_000;
        run.step(&BTreeMap::new());
        let age = (run.tick - fell) as u32;
        for (at, name) in marks {
            if age == at {
                save(&frame(&run), &dir, &format!("hand_{name}"));
            }
        }
    }

    // The hell gate rising, standing, and the title card.
    let mut run = Run::new(4, 1, None);
    run.lair_for_test();
    run.calm_for_test();
    run.dare = None;
    let (w, h) = (run.room().width(), run.room().height());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (w / 2.0 - 6.0, h / 2.0 + 5.0);
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    let born = run.tick;
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    while run.tick < born + u64::from(GATE_RISE) + 20 {
        run.players.get_mut(&1).unwrap().invulnerable = 100_000;
        run.step(&BTreeMap::new());
        let age = run.tick - born;
        if [5, 20, 40, 60, 80, 110].contains(&age) {
            save(&frame(&run), &dir, &format!("gate_{age:03}"));
        }
    }
    stand_at_gate(&mut run);
    run.step(&BTreeMap::new());
    assert_eq!(run.circle(), 1);
    for n in 0..(spectacle::CARD_TICKS) {
        if [3, 10, 60, 120, 160].contains(&n) {
            save(&frame(&run), &dir, &format!("circle_card_{n:03}"));
        }
        run.players.get_mut(&1).unwrap().invulnerable = 100_000;
        run.step(&BTreeMap::new());
    }
    save(&frame(&run), &dir, "circle_room");
    run.spectacle.circle = 2;
    run.spectacle.card = Some((run.tick, 2));
    save(&frame(&run), &dir, "circle_card_second");

    // The tray, with a hand and a spell and a cooldown part-run.
    let mut run = quiet();
    let spell =
        "name Ember Ring\nkind spell\nrarity rare\ncooldown 4\ncast ring\nward 1\ntext Fire.\n";
    run.book
        .insert(cards::check("ember-ring", spell).unwrap().card);
    {
        let hero = run.players.get_mut(&1).unwrap();
        hero.hand = vec!["potion".into(), "holy-water".into()];
        hero.spells[0] = Some("ember-ring".into());
        hero.spell_cooldowns[0] = 70;
        hero.ult_charge = 640;
        hero.dash_cooldown = 20;
    }
    idle(&mut run, 3);
    save(&frame(&run), &dir, "tray_room");
    let tray = crate::render::overworld::arena::tray::image(
        &run,
        1,
        &crate::rules::hud::Keys::FIRST,
        384,
    )
    .unwrap();
    let mut sheet = Img::black(tray.w + 8, tray.h + 8);
    sheet.stamp(&tray, 4, 4);
    save(&sheet, &dir, "tray_only");
}
