use super::*;

fn arena(kit: &str) -> Run {
    let mut run = Run::new(21, 1, None);
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(fight);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    assert!(run.take_up(1, kit));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.aim_x, hero.aim_y) = (12.0, 14.0, 1.0, 0.0);
    hero.invulnerable = 1000;
    hero.wishes = 3;
    run.phase = Phase::Fighting;
    run
}

fn foe(run: &mut Run, x: f32, y: f32) {
    run.spawn_at_for_test(EnemyKind::Demon, x, y);
    let enemy = run.enemies.last_mut().unwrap();
    enemy.hp = 10000;
    enemy.max_hp = 10000;
    enemy.seen_hp = 10000;
    enemy.age = TELEGRAPH;
}

fn climb(run: &mut Run, id: u32, name: &str) {
    run.offer_wish(id, &format!("upgrade {name}"), None)
        .unwrap();
    run.choose(id, 0).unwrap();
}

#[test]
fn spell_upgrades_offer_every_builtin_spell_and_climb_only_that_knight() {
    let mut run = arena("gravecaller");
    run.join_seat(2, "Bea");
    assert!(run.take_up(2, "gravecaller"));
    let book = run.book.clone();
    for kit in knights::COMPANY {
        assert!(run.take_up(1, kit.id));
        for spell in run.players[&1].spells.iter().flatten() {
            let card = run.book.get(spell).unwrap();
            assert!(
                run.upgrades(1)
                    .iter()
                    .any(|u| u.name == card.name && u.next == Some(1)),
                "{}",
                card.name
            );
        }
    }
    let spells: Vec<_> = book
        .cards
        .iter()
        .filter(|c| c.kind == cards::Kind::Spell)
        .cloned()
        .collect();
    assert!(spells.len() >= 16);
    for card in spells {
        let hero = run.players.get_mut(&1).unwrap();
        hero.spells = [Some(card.id.clone()), None, None, None];
        hero.wishes = 2;
        hero.spell_cooldowns[0] = 17;
        let up = run
            .upgrades(1)
            .into_iter()
            .find(|u| u.name == card.name)
            .unwrap();
        assert_eq!(up.tiers(), "I → II");
        run.offer_wish(1, &up.words(), None).unwrap();
        assert_eq!(run.players[&1].wishes, 2);
        assert_eq!(
            run.players[&1].offer.as_ref().unwrap().choices[0].gives,
            format!("{} II", card.name)
        );
        run.choose(1, 0).unwrap();
        assert_eq!(run.players[&1].spell_tier(&card.id), 1);
        assert_eq!(run.players[&1].spell_cooldowns[0], 17);
        climb(&mut run, 1, &card.name);
        assert_eq!(run.players[&1].spell_tier(&card.id), 2);
        assert_eq!(run.players[&1].wishes, 0);
        let up = run
            .upgrades(1)
            .into_iter()
            .find(|u| u.name == card.name)
            .unwrap();
        assert_eq!(
            (up.tiers(), up.next, up.available),
            ("III".into(), None, false)
        );
        assert!(run.players[&2].spell_tiers.is_empty());
        assert!(run.asked.is_empty(), "built-in magic never needs a learner");
    }
    assert_eq!(
        run.book.cards, book.cards,
        "upgrades never alter another knight's card rules"
    );
}

#[test]
fn spell_upgrades_keep_wish_ladders_and_spend_super_wishes_the_usual_way() {
    let mut run = arena("gravecaller");
    run.players.get_mut(&1).unwrap().great_wishes = 1;
    let plain = run.players[&1].wishes;
    climb(&mut run, 1, "Soul Rip");
    assert_eq!(
        (run.players[&1].wishes, run.players[&1].great_wishes),
        (plain, 0)
    );
    assert_eq!(run.players[&1].spell_tier("soul-rip"), 1);
    let wish = phrasebook::get("firebolt").unwrap();
    run.grant(1, &wish).unwrap();
    let up = run
        .upgrades(1)
        .into_iter()
        .find(|u| u.name == wish.name)
        .unwrap();
    run.offer_wish(1, &up.words(), Some(&wish)).unwrap();
    run.choose(1, 0).unwrap();
    assert!(run.players[&1].deck.contains(&wish.card_id(1)));
    assert!(!run.players[&1].spell_tiers.contains_key(&wish.card_id(0)));
}

#[test]
fn spell_upgrades_refuse_missing_max_stale_and_unfunded_picks() {
    let mut run = arena("gravecaller");
    run.offer_wish(1, "upgrade Soul Rip", None).unwrap();
    let stale = run.players[&1].offer.clone();
    run.choose(1, 0).unwrap();
    let wishes = run.players[&1].wishes;
    run.players.get_mut(&1).unwrap().offer = stale;
    assert!(run.choose(1, 0).is_err());
    assert_eq!(run.players[&1].wishes, wishes);
    run.offer_wish(1, "upgrade Soul Rip", None).unwrap();
    run.players.get_mut(&1).unwrap().spells[3] = None;
    assert!(run.choose(1, 0).is_err());
    assert_eq!(run.players[&1].wishes, wishes);
    run.players.get_mut(&1).unwrap().spells[3] = Some("soul-rip".into());
    climb(&mut run, 1, "Soul Rip");
    assert!(run.offer_wish(1, "upgrade Soul Rip", None).is_err());
    run.players.get_mut(&1).unwrap().wishes = 0;
    assert!(run.offer_wish(1, "upgrade Raise Ghoul", None).is_err());
    assert!(run.upgrades(1).iter().all(|u| !u.available));
}

#[test]
fn spell_upgrades_survive_save_old_saves_and_slot_moves() {
    let mut run = arena("gravecaller");
    climb(&mut run, 1, "Soul Rip");
    let saved = serde_json::to_value(&run).unwrap();
    let mut restored: Run = serde_json::from_value(saved.clone()).unwrap();
    restored.players.get_mut(&1).unwrap().spells.swap(0, 3);
    assert_eq!(restored.players[&1].spell_tier("soul-rip"), 1);
    assert_eq!(
        restored.hud(1, &hud::Keys::FIRST).unwrap().spells[0]
            .as_ref()
            .unwrap()
            .name,
        "Soul Rip II"
    );
    assert!(
        restored
            .tray(1, &hud::Keys::FIRST)
            .iter()
            .any(|i| i.name == "Soul Rip II")
    );
    let mut old = saved;
    old["players"]["1"]
        .as_object_mut()
        .unwrap()
        .remove("spell_tiers");
    let restored: Run = serde_json::from_value(old).expect("old saves load at tier I");
    assert_eq!(restored.players[&1].spell_tier("soul-rip"), 0);
}

#[test]
fn spell_upgrades_change_custom_projectiles_heal_nova_and_actual_recharge() {
    for (tier, damage, heal, nova, wait) in [
        (0, 32, 35, 24, 240),
        (1, 43, 47, 32, 204),
        (2, 54, 60, 41, 168),
    ] {
        let mut run = arena("percival");
        let card = cards::check(
            "silver-bolt",
            "name Silver Bolt\nkind spell\ncooldown 8\ncast bolt\nheal 35\nnova 24\n",
        )
        .unwrap()
        .card;
        assert!(run.add_card(card, 1));
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = 10;
        hero.spell_tiers.insert("silver-bolt".into(), tier);
        foe(&mut run, 30.0, 20.0);
        run.step(&BTreeMap::from([(
            1,
            Input {
                cast: 1,
                ..Default::default()
            },
        )]));
        assert_eq!(run.players[&1].spell_cooldowns[0], wait);
        assert_eq!(run.players[&1].hp, 10 + heal);
        assert!(
            run.projectiles
                .iter()
                .any(|p| !p.hostile && p.damage == damage)
        );
        assert_eq!(10000 - run.enemies[0].hp, nova);
    }
}

#[test]
fn spell_upgrades_soul_rip_hits_wider_harder_and_mends_more() {
    for (tier, damage, mend_dead, reach) in [(0, 24, 15, 3.0), (1, 32, 20, 3.5), (2, 41, 26, 4.0)] {
        let mut run = arena("gravecaller");
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = 10;
        hero.spell_tiers.insert("soul-rip".into(), tier);
        foe(&mut run, 17.0, 14.0);
        run.call_minion(1, "raise-ghoul");
        run.minions.raised[0].hp = 1;
        run.call_minion(1, "soul-rip");
        assert_eq!(10000 - run.enemies[0].hp, damage);
        assert_eq!(
            run.players[&1].hp, 12,
            "the existing lifesteal cap still applies"
        );
        assert_eq!(run.minions.raised[0].hp, 1 + mend_dead);
        assert_eq!(run.players[&1].spell_step("soul-rip").rip_reach, reach);
    }
}

#[test]
fn spell_upgrades_garden_counts_durability_and_decoy_duration() {
    for (tier, count, damage, hp, duration) in [
        (0, 6, 34, 90, 150),
        (1, 7, 46, 122, 180),
        (2, 8, 58, 153, 210),
    ] {
        let mut run = arena("percival");
        let hero = run.players.get_mut(&1).unwrap();
        for spell in ["cabbage-storm", "hedge-maze", "the-imposture"] {
            hero.spell_tiers.insert(spell.into(), tier);
        }
        let bounds = (run.room().width(), run.room().height());
        let hero = &run.players[&1];
        spells::cabbage_storm(&mut run.strikes, hero, "cabbage-storm", 6, bounds, 1);
        assert_eq!(run.strikes.len(), count);
        assert!(run.strikes.iter().all(|s| s.damage == damage));
        spells::hedge_maze(&mut run.works.barricades, hero, "hedge-maze", 1, bounds);
        assert_eq!(run.works.barricades.len(), 3);
        assert!(run.works.barricades.iter().all(|b| b.hp == hp));
        spells::call_imposture(&mut run.phantoms, hero, "the-imposture", 1);
        assert_eq!(run.phantoms[0].left, duration);
        spells::call_imposture(&mut run.phantoms, hero, "the-imposture", 1);
        assert_eq!(run.phantoms.len(), 1, "recasting replaces the decoy");
    }
}

#[test]
fn spell_upgrades_minion_claws_and_totem_bolts_use_the_callers_tier() {
    for (tier, weight) in [(0, 14), (1, 19), (2, 24)] {
        let mut run = arena("gravecaller");
        for spell in ["raise-ghoul", "barrow-warden", "bone-totem"] {
            run.players
                .get_mut(&1)
                .unwrap()
                .spell_tiers
                .insert(spell.into(), tier);
        }
        for kind in [
            hireling::AllyKind::Ghoul,
            hireling::AllyKind::Warden,
            hireling::AllyKind::Totem,
        ] {
            let minion = hireling::Hireling::minion(kind, 1, 1, (14.0, 14.0), 1);
            assert_eq!(run.minion_spell_power(&minion, 14), weight);
        }
        foe(&mut run, 20.0, 14.0);
        run.call_minion(1, "bone-totem");
        for _ in 0..40 {
            run.tick_minions();
        }
        assert!(
            run.projectiles
                .iter()
                .any(|p| p.damage == weight && !p.hostile)
        );
    }
}

#[test]
fn spell_upgrades_champion_damage_heal_and_durations_land() {
    for (tier, slam, elixir, fury) in [(0, 45, 35, 180), (1, 61, 47, 210), (2, 77, 60, 240)] {
        let mut run = arena("hammerer");
        run.players
            .get_mut(&1)
            .unwrap()
            .spell_tiers
            .insert("groundslam".into(), tier);
        run.players
            .get_mut(&1)
            .unwrap()
            .spell_tiers
            .insert("brute-force".into(), tier);
        foe(&mut run, 14.0, 14.0);
        run.champion_feat(1, "groundslam");
        for _ in 0..champions::SLAM_WINDUP {
            run.tick_champions();
        }
        assert_eq!(10000 - run.enemies[0].hp, slam);
        run.champion_feat(1, "brute-force");
        assert_eq!(run.players[&1].champ.fury, fury);
        run.players
            .get_mut(&1)
            .unwrap()
            .spell_tiers
            .insert("elixir".into(), tier);
        run.players.get_mut(&1).unwrap().hp = 10;
        run.champion_feat(1, "elixir");
        assert_eq!(run.players[&1].hp, 10 + elixir);
    }
}

#[test]
fn spell_upgrades_wardens_gain_health_and_a_longer_roar() {
    for (tier, hp, taunt) in [(0, 240, 45), (1, 324, 61), (2, 408, 77)] {
        let mut run = arena("gravecaller");
        run.players
            .get_mut(&1)
            .unwrap()
            .spell_tiers
            .insert("barrow-warden".into(), tier);
        run.call_minion(1, "barrow-warden");
        foe(&mut run, 15.0, 14.0);
        run.fit_dead();
        assert_eq!(run.minions.raised[0].max_hp, hp);
        assert_eq!(run.minions.raised[0].hp, hp);
        for _ in 0..40 {
            run.tick_minions();
        }
        assert_eq!(
            run.minions.raised[0].taunt + 24,
            taunt,
            "the roar starts after rising, then runs down"
        );
    }
}

#[test]
fn a_spell_casts_in_its_familys_voice_and_its_bolt_breaks_the_same_way() {
    let mut run = arena("percival");
    let card = cards::check("ember-lance", "name Ember Lance\nkind spell\ncooldown 8\ncast bolt\n").unwrap().card;
    assert!(run.add_card(card, 1));
    foe(&mut run, 18.0, 14.0);
    run.sounds.clear();
    run.step(&BTreeMap::from([(1, Input { cast: 1, ..Default::default() })]));
    assert!(run.sounds.contains(&"cast_fire") && !run.sounds.contains(&"spell_cast"), "{:?}", run.sounds);
    let mut heard = Vec::new();
    for _ in 0..30 {
        run.sounds.clear();
        run.step(&BTreeMap::new());
        heard.extend(run.sounds.iter().copied());
        if run.enemies[0].hp < 10000 {
            break;
        }
    }
    assert!(run.enemies[0].hp < 10000, "the bolt struck");
    // It breaks as fire, and the bow's own hit is not rung over it.
    assert!(heard.contains(&"impact_fire"), "{heard:?}");
    assert!(!heard.iter().any(|s| ["hit", "hit_arrow", "hit_magic", "hit_bolt"].contains(s)), "{heard:?}");
}
