use super::*;
use crate::rules::cards::Guard;

#[test]
fn every_knight_of_the_company_carries_real_cards() {
    let book = crate::rules::Book::builtin();
    for k in COMPANY {
        assert!(
            book.get(k.arm).is_some_and(|c| c.arm.is_some()),
            "{} arm {}",
            k.id,
            k.arm
        );
        assert!(
            book.get(k.guard).is_some_and(|c| c.guard.is_some()),
            "{} guard {}",
            k.id,
            k.guard
        );
        assert!(k.hand.iter().all(|c| book.get(c).is_some()));
    }
    let ids: std::collections::BTreeSet<_> = COMPANY.iter().map(|k| k.id).collect();
    assert_eq!(ids.len(), COMPANY.len());
}

#[test]
fn outfitting_dresses_the_knight_and_begin_in_picks_the_first_delve() {
    let mut run = Run::new(3, 1, None);
    let lynette = knight("lynette").unwrap();
    run.outfit(1, lynette);
    let hero = &run.players[&1];
    assert_eq!(hero.guard, Guard::Shield);
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.colours(), lynette.colours);
    let other = if run.dungeon.pack == Pack::Crypt {
        Pack::Cavern
    } else {
        Pack::Crypt
    };
    run.begin_in(other);
    assert_eq!(run.dungeon.pack, other);
    assert_eq!(run.cues.first().map(String::as_str), Some("run_start"));
    let galahad = knight("galahad").unwrap();
    run.outfit(1, galahad);
    assert_eq!(run.players[&1].max_hp, 130);
    assert!(
        run.players[&1].forged.is_some(),
        "the blade is a forged melee arm"
    );
}

#[test]
fn a_kit_is_worn_by_a_knight_of_the_serving_house() {
    use crate::world::houses;
    let percival = knight("percival").unwrap();
    // Nothing serving (the stub route): the Keep's own household.
    houses::note_serving(&houses::Serving::default());
    assert_eq!(
        percival.styled(house_for_seat(1)),
        "Sir Percival, the Seeker"
    );
    let mut run = Run::new(3, 2, Some("Friend"));
    run.outfit(1, percival);
    assert_eq!(run.players[&1].house, None);
    assert_eq!(
        run.players[&1].knight_name().as_deref(),
        Some("Sir Percival")
    );

    // DeepSeek serves, Kimi sits beside it in a formation.
    let (deepseek, kimi) = (houses::by_key("deepseek"), houses::by_key("kimi"));
    houses::note_serving(&houses::Serving {
        lead: deepseek,
        seated: kimi.into_iter().collect(),
        turn: false,
    });
    let ds = houses::get(deepseek.unwrap());
    // Expanded kits rotate through the house's smaller name roster, just as
    // Knight::of_house does; kit slots are not house-roster indices.
    let first = ds.knight(percival.slot()).unwrap();
    assert_eq!(
        percival.styled(house_for_seat(1)),
        format!("{} of {}, the Seeker", first.name, ds.castle)
    );
    run.outfit(1, percival);
    let lynette = knight("lynette").unwrap();
    run.outfit(2, lynette);
    assert_eq!(run.players[&1].house.as_deref(), Some("deepseek"));
    assert_eq!(run.players[&1].knight_name(), Some(first.name.clone()));
    let km = houses::get(kimi.unwrap());
    assert_eq!(run.players[&2].house.as_deref(), Some("kimi"));
    assert_eq!(
        run.players[&2].knight_name(),
        Some(km.knight(lynette.slot()).unwrap().name.clone())
    );
    // The kit is unchanged: same arms, same ultimate, same saved id.
    assert_eq!(run.players[&2].knight.as_deref(), Some("lynette"));
    assert_eq!(run.players[&2].colours(), lynette.colours);

    // A save from before the houses has no `house`: it loads, and the
    // Keep's household names the knight.
    let mut saved = serde_json::to_value(&run.players[&1]).unwrap();
    saved.as_object_mut().unwrap().remove("house");
    let old: crate::rules::Hero = serde_json::from_value(saved).unwrap();
    assert_eq!(old.house, None);
    assert_eq!(old.knight_name().as_deref(), Some("Sir Percival"));
    houses::note_serving(&houses::Serving::default());
}

#[test]
fn twilight_company_has_valid_distinct_spell_and_item_art() {
    let book = crate::rules::Book::builtin();
    let ids = ["noon-shatter", "borrowed-minute", "dialkeepers-ring", "storm-receipt", "eye-of-the-squall", "fulgurite-spurs", "eclipse-toll", "mercy-lantern", "starfall-writ", "cinder-tithe",
        "keelbreaker", "pressure-seal", "undertow-coil",
        "last-light", "comet-glass", "bellkeepers-mail", "wayfarer-wick", "meteor-cordial"];
    let mut pictures = std::collections::BTreeSet::new();
    for id in ids {
        let card = book.get(id).unwrap_or_else(|| panic!("missing card {id}"));
        assert_eq!(card.art.len(), 16, "{id}");
        assert!(card.art.iter().all(|row| row.len() == 16), "{id}");
        let picture = card.art.join("\n");
        assert!(pictures.insert(picture.clone()), "reused icon: {id}");
        // Compare against the whole library, not just this release's cards.
        assert_eq!(book.cards.iter().filter(|other| other.art.join("\n") == picture).count(), 1,
            "icon collides with an existing card: {id}");
        let (_, source) = crate::rules::cards::BUILTIN.iter()
            .find(|(key, _)| *key == id).unwrap();
        assert!(crate::rules::cards::check(id, source).is_ok(), "invalid source: {id}");
    }
    for id in ["vesper", "orrin", "iona", "brine", "rill", "saffra"] {
        let kit = knight(id).unwrap();
        assert_eq!(kit.spells.len(), 2);
        assert_eq!(crate::rules::alehouse::PATRONS.iter().filter(|p| p.kit == id).count(), 1,
            "kit must have exactly one recruiter: {id}");
        for item in kit.hand {
            assert!(book.get(item).is_some(), "missing starting item {item} for {id}");
        }
        for spell in kit.spells {
            assert_eq!(book.get(spell).unwrap().kind, crate::rules::cards::Kind::Spell);
        }
        let mut run = Run::new(3, 1, None);
        run.outfit(1, kit);
        assert_eq!(run.players[&1].max_hp, kit.max_hp, "{id}");
    }
}

#[test]
fn widowseed_keeper_cards_are_valid_unique_and_recruitable() {
    let book = crate::rules::Book::builtin();
    let kit = knight("morrow").unwrap();
    assert_eq!(crate::rules::alehouse::PATRONS.iter().filter(|p| p.kit == kit.id).count(), 1);
    for id in kit.hand.iter().chain(kit.spells.iter()) {
        let card = book.get(id).unwrap();
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1, "duplicate art {id}");
        let (_, source) = crate::rules::cards::BUILTIN.iter().find(|(key, _)| key == id).unwrap();
        assert!(crate::rules::cards::check(id, source).is_ok(), "invalid {id}");
    }
    for id in kit.spells { assert_eq!(book.get(id).unwrap().kind, crate::rules::cards::Kind::Spell); }
    let mut run = Run::new(3, 1, None);
    run.outfit(1, kit);
    assert_eq!(run.players[&1].max_hp, kit.max_hp);
}

#[test]
fn salvage_reliquary_loot_has_valid_unique_hotbar_art() {
    let book = crate::rules::Book::builtin();
    for id in ["abyssal-compass", "powder-saint", "rustwing-locket"] {
        let (_, source) = crate::rules::cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        assert!(crate::rules::cards::check(id, source).is_ok(), "invalid {id}");
        let card = book.get(id).unwrap();
        assert_eq!(card.art.len(), 16, "{id}");
        assert!(card.art.iter().all(|row| row.len() == 16), "{id}");
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1, "duplicate art {id}");
        assert!(source.contains("drop 2\nchest 3\n"), "unobtainable {id}");
    }
}

#[test]
fn apothecary_consumables_are_valid_distinct_and_available() {
    let book = crate::rules::Book::builtin();
    for id in ["frostbite-ampoule", "choir-grenade", "phoenix-suture"] {
        let (_, source) = crate::rules::cards::BUILTIN.iter()
            .find(|(key, _)| *key == id).unwrap();
        assert!(crate::rules::cards::check(id, source).is_ok(), "invalid {id}");
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, crate::rules::cards::Kind::Play);
        assert_eq!(card.art.len(), 16, "{id}");
        assert!(card.art.iter().all(|row| row.len() == 16), "{id}");
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1,
            "duplicate icon {id}");
        assert!(card.drop > 0 && card.chest > 0, "unobtainable {id}");
        assert!(card.only_in.is_none(), "restricted loot {id}");
        assert!(!card.effects.is_empty(), "inert {id}");
    }
}

#[test]
fn spell_cache_is_valid_unique_and_world_available() {
    let book = crate::rules::Book::builtin();
    for id in ["gallows-bell", "moth-sanctuary", "crown-of-cinders"] {
        let (_, source) = crate::rules::cards::BUILTIN.iter()
            .find(|(key, _)| *key == id).unwrap();
        assert!(crate::rules::cards::check(id, source).is_ok(), "invalid {id}");
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, crate::rules::cards::Kind::Spell);
        assert_eq!(card.art.len(), 16, "{id}");
        assert!(card.art.iter().all(|row| row.len() == 16), "{id}");
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1,
            "duplicate icon {id}");
        assert!(card.drop > 0 && card.chest > 0, "unobtainable {id}");
        assert!(card.only_in.is_none(), "restricted {id}");
        assert!(!card.effects.is_empty(), "inert {id}");
    }
}

#[test]
fn powderworks_loot_is_valid_unique_and_world_available() {
    let book = crate::rules::Book::builtin();
    for id in ["railglass-prism", "emberwake-charge"] {
        let (_, source) = crate::rules::cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        assert!(crate::rules::cards::check(id, source).is_ok(), "invalid {id}");
        let card = book.get(id).unwrap();
        assert_eq!(card.art.len(), 16, "{id}");
        assert!(card.art.iter().all(|row| row.len() == 16), "{id}");
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1, "duplicate {id}");
        assert!(card.drop > 0 && card.chest > 0, "unobtainable {id}");
        assert!(card.only_in.is_none(), "restricted {id}");
        assert!(!card.effects.is_empty(), "inert {id}");
    }
}

#[test]
fn griffin_heirlooms_are_valid_distinct_and_world_available() {
    let book = crate::rules::Book::builtin();
    for (id, effects) in [
        ("gravewire-spool", ["armor 1", "rate 5"]),
        ("sunken-mint", ["armor 1", "max_hp 12"]),
    ] {
        let (_, source) = crate::rules::cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        assert!(crate::rules::cards::check(id, source).is_ok(), "invalid {id}");
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, crate::rules::cards::Kind::Hold);
        assert_eq!(card.art.len(), 16, "{id}");
        assert!(card.art.iter().all(|row| row.len() == 16), "{id}");
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1, "duplicate art {id}");
        assert!(source.contains("drop 2\nchest 3\n"), "unobtainable {id}");
        for effect in effects {
            assert!(source.lines().any(|line| line == effect), "missing {effect} on {id}");
        }
    }
}

#[test]
fn storm_orchard_spells_are_valid_distinct_and_available() {
    let book = crate::rules::Book::builtin();
    for (id, expected) in [
        ("stormglass-vespers", ["cooldown 24", "nova 28", "ward 1"]),
        ("orchard-of-ashes", ["cooldown 25", "heal 24", "ward 1"]),
    ] {
        let (_, source) = crate::rules::cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        assert!(crate::rules::cards::check(id, source).is_ok(), "invalid {id}");
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, crate::rules::cards::Kind::Spell);
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1, "duplicate {id}");
        assert!(card.drop > 0 && card.chest > 0);
        assert!(card.only_in.is_none());
        assert!(!card.effects.is_empty());
        for line in expected {
            assert!(source.lines().any(|actual| actual == line), "missing {line} on {id}");
        }
    }
}

#[test]
fn lantern_courier_loot_is_distinct_valid_and_available() {
    use crate::rules::cards::{self, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, lines) in [
        ("lastlight-postmark", Kind::Hold, vec!["armor 1", "max_hp 8"]),
        ("deadletter-flare", Kind::Spell, vec!["cooldown 23", "nova 30", "ward 1"]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        assert!(cards::check(id, source).is_ok(), "invalid {id}");
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, kind);
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1, "duplicate {id}");
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert!(!card.effects.is_empty());
        for line in lines {
            assert!(source.lines().any(|actual| actual == line), "missing {line} on {id}");
        }
    }
}

#[test]
fn blacktide_clockwork_loot_is_valid_unique_and_available() {
    use crate::rules::cards::{self, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, expected) in [
        ("blacktide-censer", Kind::Spell, vec!["cooldown 26", "nova 36", "ward 1"]),
        ("duelists-mainspring", Kind::Hold, vec!["rate 5", "max_hp 6"]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, kind);
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1, "duplicate {id}");
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert!(!card.effects.is_empty());
        for line in expected {
            assert!(source.lines().any(|actual| actual == line), "missing {line} on {id}");
        }
    }
}

#[test]
fn field_apothecary_consumables_are_valid_unique_and_available() {
    use crate::rules::cards::{self, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effects) in [
        ("saints-tourniquet", ["heal 28", "ward 1"]),
        ("thunderhead-phial", ["nova 24", "heal 10"]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        assert!(cards::check(id, source).is_ok(), "invalid {id}");
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Play);
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1, "duplicate {id}");
        assert!(card.drop > 0 && card.chest > 0);
        assert!(card.only_in.is_none());
        assert_eq!(card.effects.len(), 2);
        for effect in effects {
            assert!(source.lines().any(|line| line == effect), "missing {effect}");
        }
    }
}

#[test]
fn siege_procession_summons_are_valid_unique_and_world_available() {
    use crate::rules::cards::{self, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effect, cooldown) in [
        ("pallbearer-lanterns", "summon 2 guard", "cooldown 14"),
        ("rook-mortar", "summon 1 turret", "cooldown 18"),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        assert!(cards::check(id, source).is_ok(), "invalid {id}: {:?}", cards::check(id, source));
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Spell);
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1, "duplicate {id}");
        assert!(card.drop > 0 && card.chest > 0);
        assert!(card.only_in.is_none());
        assert_eq!(card.effects.len(), 1);
        for line in [effect, cooldown] {
            assert!(source.lines().any(|actual| actual == line), "missing {line}");
        }
    }
}

#[test]
fn glassway_cache_adds_valid_world_mobility() {
    use crate::rules::cards::{self, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, lines) in [
        ("foxstep-spindle", Kind::Play, vec!["blink 8", "immune 4"]),
        ("glassway-key", Kind::Play, vec!["blink 10", "ward 1"]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, kind);
        assert_eq!(card.effects.len(), 2);
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        for line in lines {
            assert!(source.lines().any(|actual| actual == line));
        }
    }
}

#[test]
fn wake_engineer_has_distinct_relic_and_reachable_siege_kit() {
    use crate::rules::{alehouse, cards::{self, Kind}};
    let book = crate::rules::Book::builtin();
    let kit = knight("tamsin").unwrap();
    assert_eq!(kit.spells, &["rook-mortar", "pallbearer-lanterns"]);
    assert_eq!(kit.hand, &["wake-engine"]);
    assert!(book.get(kit.arm).unwrap().arm.is_some());
    assert!(book.get(kit.guard).unwrap().guard.is_some());
    for id in kit.spells {
        let spell = book.get(id).unwrap();
        assert_eq!(spell.kind, Kind::Spell);
        assert!(!spell.effects.is_empty());
    }
    let (_, source) = cards::BUILTIN.iter().find(|(id, _)| *id == "wake-engine").unwrap();
    cards::check("wake-engine", source).unwrap();
    let relic = book.get("wake-engine").unwrap();
    assert_eq!(relic.kind, Kind::Hold);
    assert_eq!(relic.effects.len(), 2);
    assert!(relic.drop > 0 && relic.chest > 0 && relic.only_in.is_none());
    assert_eq!(relic.art.len(), 16);
    assert!(relic.art.iter().all(|row| row.len() == 16));
    assert_eq!(book.cards.iter().filter(|c| c.art == relic.art).count(), 1);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}

#[test]
fn stormrunner_has_valid_mobile_loadout_and_reachable_tavern_seat() {
    use crate::rules::{alehouse, cards::{self, Kind}};
    let book = crate::rules::Book::builtin();
    let kit = knight("nyra").unwrap();
    assert_eq!(kit.hand, &["switchglass-escapement", "foxstep-spindle", "glassway-key"]);
    assert!(book.get(kit.arm).unwrap().arm.is_some());
    assert!(book.get(kit.guard).unwrap().guard.is_some());
    for id in kit.hand {
        assert!(book.get(id).is_some(), "missing starting card {id}");
    }
    assert_eq!(kit.spells, &["stormglass-vespers"]);
    assert!(kit.hand.contains(&"foxstep-spindle"));
    assert!(kit.hand.contains(&"glassway-key"));
    for id in kit.spells {
        let spell = book.get(id).unwrap();
        assert_eq!(spell.kind, Kind::Spell);
        assert!(!spell.effects.is_empty());
    }
    let (_, source) = cards::BUILTIN.iter().find(|(id, _)| *id == "switchglass-escapement").unwrap();
    cards::check("switchglass-escapement", source).unwrap();
    let relic = book.get("switchglass-escapement").unwrap();
    assert_eq!(relic.kind, Kind::Hold);
    assert_eq!(relic.effects.len(), 2);
    assert!(relic.drop > 0 && relic.chest > 0 && relic.only_in.is_none());
    assert_eq!(relic.art.len(), 16);
    assert!(relic.art.iter().all(|row| row.len() == 16));
    assert_eq!(book.cards.iter().filter(|c| c.art == relic.art).count(), 1);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}

#[test]
fn astral_breach_spells_supply_trait_driven_world_attacks() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, shape, cooldown) in [
        ("starfall-writ", Shape::Meteor, "cooldown 13"),
        ("halo-breach", Shape::Ring, "cooldown 11"),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Spell);
        assert_eq!(card.effects, vec![Effect::Cast(shape)]);
        assert!(source.lines().any(|line| line == cooldown));
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn gallery_relics_add_valid_distinct_projectile_builds() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effects) in [
        ("bankshot-medallion", vec![Effect::Bounce(1), Effect::Velocity(15)]),
        ("wishbone-reticle", vec![Effect::Fork(1), Effect::Velocity(10)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Hold);
        assert_eq!(card.effects, effects);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn linebreaker_has_piercing_relic_and_reachable_tavern_seat() {
    use crate::rules::{alehouse, cards::{self, Effect, Kind}};
    let book = crate::rules::Book::builtin();
    let kit = knight("elian").unwrap();
    assert_eq!(kit.hand, &["needlewake-sight"]);
    assert!(book.get(kit.arm).unwrap().arm.is_some());
    assert!(book.get(kit.guard).unwrap().guard.is_some());
    for id in kit.hand {
        assert!(book.get(id).is_some(), "missing starting card {id}");
    }
    assert_eq!(kit.spells, &["halo-breach", "borrowed-minute"]);
    for id in kit.spells {
        let spell = book.get(id).unwrap();
        assert_eq!(spell.kind, Kind::Spell);
        assert!(!spell.effects.is_empty());
    }
    let (_, source) = cards::BUILTIN.iter().find(|(id, _)| *id == "needlewake-sight").unwrap();
    cards::check("needlewake-sight", source).unwrap();
    let relic = book.get("needlewake-sight").unwrap();
    assert_eq!(relic.kind, Kind::Hold);
    assert_eq!(relic.effects, vec![Effect::Pierce(1), Effect::Velocity(20)]);
    assert!(relic.drop > 0 && relic.chest > 0 && relic.only_in.is_none());
    assert_eq!(relic.art.len(), 16);
    assert!(relic.art.iter().all(|row| row.len() == 16));
    assert_eq!(book.cards.iter().filter(|c| c.art == relic.art).count(), 1);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}

#[test]
fn venom_and_whiteout_relics_offer_distinct_control_builds() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effects) in [
        ("vipers-hourglass", vec![Effect::Poison(1), Effect::Burst(20)]),
        ("whiteout-muzzle", vec![Effect::Chill(1), Effect::Knock(2)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Hold);
        assert_eq!(card.effects, effects);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn trigger_spells_supply_aimed_world_loot() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, cooldown, effects) in [
        ("copperhead-volley", "cooldown 6", vec![Effect::Cast(Shape::Bolt)]),
        ("mercy-trigger", "cooldown 10", vec![Effect::Cast(Shape::Bolt), Effect::Heal(8)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Spell);
        assert_eq!(card.effects, effects);
        assert!(source.lines().any(|line| line == cooldown));
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn procession_spells_combine_support_with_world_available_summons() {
    use crate::rules::cards::{self, Kind};
    let book = crate::rules::Book::builtin();
    for (id, lines) in [
        ("bastion-bell", ["cooldown 24", "summon 1 turret", "ward 1"]),
        ("mercy-procession", ["cooldown 22", "summon 2 guard", "heal 12"]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Spell);
        assert_eq!(card.effects.len(), 2);
        for line in lines { assert!(source.lines().any(|value| value == line)); }
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn emergency_supplies_offer_world_available_binding_and_escape_answers() {
    use crate::rules::cards::{self, Kind};
    let book = crate::rules::Book::builtin();
    for (id, lines) in [
        ("plague-divers-rebreather", ["immune 6", "ward 1"]),
        ("redshift-ripcord", ["blink 6", "heal 16"]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Play);
        assert_eq!(card.effects, checked.card.effects);
        assert_eq!(card.effects.len(), 2);
        for line in lines { assert!(source.lines().any(|value| value == line)); }
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn choirwire_and_mothglass_offer_chain_and_tracking_world_builds() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effects) in [
        ("choirwire-coil", vec![Effect::Chain(1)]),
        ("mothglass-lens", vec![Effect::Homing(1)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Hold);
        assert_eq!(card.effects, effects);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn night_entomancer_has_guided_venom_and_reachable_tavern_seat() {
    use crate::rules::{alehouse, cards::{Effect, Kind}};
    let book = crate::rules::Book::builtin();
    let kit = knight("iole").unwrap();
    assert_eq!(kit.max_hp, 75);
    assert_eq!(kit.bombs, 0);
    assert!(book.get(kit.arm).unwrap().arm.is_some());
    assert!(book.get(kit.guard).unwrap().guard.is_some());
    assert_eq!(kit.hand, &["mothglass-lens", "vipers-hourglass"]);
    assert!(book.get(kit.hand[0]).unwrap().effects.contains(&Effect::Homing(1)));
    assert!(book.get(kit.hand[1]).unwrap().effects.contains(&Effect::Poison(1)));
    assert_eq!(kit.spells, &["copperhead-volley", "pallbearer-lanterns"]);
    for id in kit.spells { assert_eq!(book.get(id).unwrap().kind, Kind::Spell); }
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}

#[test]
fn circuit_cantor_has_chain_support_and_reachable_tavern_seat() {
    use crate::rules::{alehouse, cards::{Effect, Kind}};
    let book = crate::rules::Book::builtin();
    let kit = knight("sorel").unwrap();
    assert_eq!(kit.max_hp, 90);
    assert_eq!(kit.bombs, 0);
    assert!(book.get(kit.arm).unwrap().arm.is_some());
    assert!(book.get(kit.guard).unwrap().guard.is_some());
    assert_eq!(kit.hand, &["choirwire-coil"]);
    assert!(book.get(kit.hand[0]).unwrap().effects.contains(&Effect::Chain(1)));
    assert_eq!(kit.spells, &["bastion-bell", "mercy-procession"]);
    for id in kit.spells { assert_eq!(book.get(id).unwrap().kind, Kind::Spell); }
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}

#[test]
fn stormrail_and_winterfang_offer_valid_hybrid_world_builds() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effects) in [
        ("stormrail-conductor", vec![Effect::Bounce(1), Effect::Chain(1)]),
        ("winterfang-bayonet", vec![Effect::Pierce(1), Effect::Chill(1)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Hold);
        assert_eq!(card.effects, effects);
        assert_eq!(checked.card.effects, card.effects);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn avalanche_marshal_has_piercing_chill_and_reachable_tavern_seat() {
    use crate::rules::{alehouse, cards::{Effect, Kind}};
    let book = crate::rules::Book::builtin();
    let kit = knight("veska").unwrap();
    assert_eq!(kit.max_hp, 80);
    assert_eq!(kit.bombs, 0);
    assert!(book.get(kit.arm).unwrap().arm.is_some());
    assert!(book.get(kit.guard).unwrap().guard.is_some());
    assert_eq!(kit.hand, &["winterfang-bayonet"]);
    assert!(book.get(kit.hand[0]).unwrap().effects.contains(&Effect::Pierce(1)));
    assert!(book.get(kit.hand[0]).unwrap().effects.contains(&Effect::Chill(1)));
    assert_eq!(kit.spells, &["starfall-writ", "halo-breach"]);
    for id in kit.spells { assert_eq!(book.get(id).unwrap().kind, Kind::Spell); }
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}

#[test]
fn crown_and_parasol_add_hybrid_projectile_world_spells() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, cooldown, effects) in [
        ("undertow-crown", 18, vec![Effect::Cast(Shape::Ring), Effect::Heal(10)]),
        ("cinder-parasol", 17, vec![Effect::Cast(Shape::Meteor), Effect::Ward(1)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Spell);
        assert_eq!(card.effects, effects);
        assert!(source.lines().any(|line| line == format!("cooldown {cooldown}")));
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn breakwater_corsair_has_hybrid_sustain_and_reachable_tavern_seat() {
    use crate::rules::{alehouse, cards::{Effect, Kind, Shape}};
    let book = crate::rules::Book::builtin();
    let kit = knight("mariel").unwrap();
    assert_eq!(kit.max_hp, 75);
    assert_eq!(kit.bombs, 0);
    assert!(book.get(kit.arm).unwrap().arm.is_some());
    assert!(book.get(kit.guard).unwrap().guard.is_some());
    assert_eq!(kit.hand, &["stormrail-conductor"]);
    let relic = book.get(kit.hand[0]).unwrap();
    assert!(relic.effects.contains(&Effect::Chain(1)));
    assert!(relic.effects.contains(&Effect::Bounce(1)));
    assert_eq!(kit.spells, &["undertow-crown", "cinder-parasol"]);
    for (id, expected) in [
        ("undertow-crown", vec![Effect::Cast(Shape::Ring), Effect::Heal(10)]),
        ("cinder-parasol", vec![Effect::Cast(Shape::Meteor), Effect::Ward(1)]),
    ] {
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Spell);
        assert_eq!(card.effects, expected);
    }
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    assert_eq!(alehouse::PATRONS.iter().filter(|p| p.kit == kit.id).count(), 1);
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}

#[test]
fn glacier_sexton_has_shielded_frost_support_and_reachable_tavern_seat() {
    use crate::rules::{alehouse, cards::{Effect, Kind}};
    let book = crate::rules::Book::builtin();
    let kit = knight("orren").unwrap();
    assert_eq!((kit.max_hp, kit.bombs), (105, 0));
    assert_eq!(kit.arm, "handgonne");
    assert_eq!(kit.guard, "kite-shield");
    assert!(book.get(kit.arm).unwrap().arm.is_some());
    assert!(book.get(kit.guard).unwrap().guard.is_some());
    assert_eq!(kit.hand, &["whiteout-muzzle"]);
    assert!(book.get(kit.hand[0]).unwrap().effects.contains(&Effect::Chill(1)));
    assert_eq!(kit.spells, &["bastion-bell", "mercy-procession"]);
    for id in kit.spells { assert_eq!(book.get(id).unwrap().kind, Kind::Spell); }
    assert_eq!(alehouse::PATRONS.iter().filter(|p| p.kit == kit.id).count(), 1);
    // Inserting a recruit must not shadow any older recruit on a shared page.
    for (index, patron) in alehouse::PATRONS.iter().enumerate() {
        let page = (0..alehouse::COMPANY_PAGES)
            .find(|&page| alehouse::patron_visible(page, index)).unwrap();
        assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), 
            patron.at.0 * crate::rules::TILE_UNITS,
            patron.at.1 * crate::rules::TILE_UNITS, 0, page),
            Some(alehouse::Spot::Patron(i)) if i == index), "{} is shadowed", patron.kit);
    }
}


#[test]
fn mobility_supplies_offer_world_escape_tools() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effects) in [
        ("vaultkey-spur", vec![Effect::Blink(4), Effect::Ward(1)]),
        ("wayfarer-stitch", vec![Effect::Blink(5), Effect::Heal(12)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Play);
        assert_eq!(card.effects, effects);
        assert_eq!(checked.card.effects, effects);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn prism_and_spindle_offer_distinct_targeting_builds() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effects) in [
        ("firefly-prism", vec![Effect::Fork(1), Effect::Homing(1)]),
        ("arcneedle-spindle", vec![Effect::Pierce(1), Effect::Chain(1)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Hold);
        assert_eq!(card.effects, effects);
        assert_eq!(checked.card.effects, effects);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn lantern_duelist_has_shielded_swarm_and_accessible_company() {
    use crate::rules::{alehouse, cards::{Effect, Kind, Shape}};
    let book = crate::rules::Book::builtin();
    let kit = knight("lys").unwrap();
    assert_eq!((kit.max_hp, kit.bombs), (80, 0));
    assert_eq!((kit.arm, kit.guard), ("crossbow", "kite-shield"));
    assert!(book.get(kit.arm).unwrap().arm.is_some());
    assert!(book.get(kit.guard).unwrap().guard.is_some());
    assert_eq!(kit.hand, &["firefly-prism"]);
    assert_eq!(book.get(kit.hand[0]).unwrap().effects, vec![Effect::Fork(1), Effect::Homing(1)]);
    assert_eq!(kit.spells, &["copperhead-volley", "undertow-crown"]);
    for (id, effects) in [
        ("copperhead-volley", vec![Effect::Cast(Shape::Bolt)]),
        ("undertow-crown", vec![Effect::Cast(Shape::Ring), Effect::Heal(10)]),
    ] {
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Spell);
        assert_eq!(card.effects, effects);
    }
    assert_eq!(alehouse::PATRONS.iter().filter(|p| p.kit == kit.id).count(), 1);
    for (index, patron) in alehouse::PATRONS.iter().enumerate() {
        let page = (0..alehouse::COMPANY_PAGES)
            .find(|&page| alehouse::patron_visible(page, index)).unwrap();
        assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), 
            patron.at.0 * crate::rules::TILE_UNITS,
            patron.at.1 * crate::rules::TILE_UNITS, 0, page),
            Some(alehouse::Spot::Patron(i)) if i == index), "unreachable {}", patron.kit);
    }
}


#[test]
fn expanded_company_can_be_named_by_every_serving_house() {
    use crate::world::houses;
    // Do not mutate the global serving roster: test the explicit house path
    // used by saved heroes as well as the recruit's display-name path.
    for house_id in houses::ids() {
        let house = houses::get(house_id);
        for kit in COMPANY {
            let wearer = kit.of_house(Some(house_id));
            if house.knights.is_empty() {
                assert!(wearer.is_none());
                assert_eq!(kit.name_for(Some(house_id)), kit.name);
                continue;
            }
            let expected = &house.knights[kit.slot() % house.knights.len()];
            assert_eq!(wearer.unwrap().name, expected.name, "{} / {}", house.key, kit.id);
            assert_eq!(kit.name_for(Some(house_id)), expected.name);
            assert!(kit.styled(Some(house_id)).contains(kit.class()));
            let mut run = Run::new(3, 1, None);
            run.outfit(1, kit);
            let hero = run.players.get_mut(&1).unwrap();
            hero.house = Some(house.key.to_string());
            let saved = serde_json::to_value(&*hero).unwrap();
            let restored: crate::rules::Hero = serde_json::from_value(saved).unwrap();
            assert_eq!(restored.knight_name(), Some(expected.name.clone()),
                "saved {} / {}", house.key, kit.id);
            assert_eq!(restored.knight.as_deref(), Some(kit.id));
            assert_eq!(restored.colours(), kit.colours);
        }
    }
}

#[test]
fn bard_songbook_recognizes_every_recruit_and_real_equipment() {
    let book = crate::rules::Book::builtin();
    let mut songs = std::collections::BTreeSet::new();
    for kit in COMPANY {
        let song = crate::rules::alehouse::company_ballad(kit.id, &book)
            .expect("every kit has a bard introduction");
        assert!(song.contains(&book.get(kit.arm).unwrap().name.to_uppercase()));
        if let Some(spell) = kit.spells.first() {
            assert!(song.contains(&book.get(spell).unwrap().name.to_uppercase()));
        }
        assert!(songs.insert(song), "duplicate ballad for {}", kit.id);
    }
    assert!(crate::rules::alehouse::company_ballad("missing-kit", &book).is_none());
}

#[test]
fn recruitment_receipts_cover_every_card_and_spell_control() {
    let book = crate::rules::Book::builtin();
    for kit in COMPANY {
        let receipt = crate::rules::alehouse::company_loadout(kit.id, &book)
            .expect("every recruit has a complete loadout receipt");
        for id in [kit.arm, kit.guard].into_iter().chain(kit.hand.iter().copied()) {
            assert!(receipt.contains(&book.get(id).unwrap().name.to_uppercase()), "{}: {}", kit.id, id);
        }
        for (slot, id) in kit.spells.iter().enumerate() {
            assert!(receipt.contains(&format!("SPELL {}: {}", slot + 1, book.get(id).unwrap().name.to_uppercase())));
        }
        assert!(receipt.contains(&format!("{} HEALTH; {} BOMBS", kit.max_hp, kit.bombs)));
    }
    assert!(crate::rules::alehouse::company_loadout("missing-kit", &book).is_none());
}

#[test]
fn celestial_spell_and_sugarglass_supply_are_world_loot() {
    use crate::rules::cards::{self, Effect, Shape};
    let book = crate::rules::Book::builtin();
    for (id, effects) in [
        ("orrery-collapse", vec![Effect::Cast(Shape::Ring)]),
        ("hearthglass-pear", vec![Effect::Heal(18), Effect::Ward(1)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn stormglass_and_cathedra_are_distinct_valid_world_rewards() {
    use crate::rules::cards::{self, Effect, Shape};
    let book = crate::rules::Book::builtin();
    for (id, effects) in [
        ("needle-cathedra", vec![Effect::Pierce(1), Effect::Homing(1)]),
        ("stormglass-writ", vec![Effect::Cast(Shape::Bolt)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn tavern_encores_follow_every_recruits_real_loadout() {
    use crate::rules::alehouse::{company_ballad, company_verse};
    let book = crate::rules::Book::builtin();
    for kit in COMPANY {
        let verses: Vec<_> = (0..8).map(|beat| company_verse(kit.id, &book, beat).unwrap()).collect();
        assert_eq!(verses[0], company_ballad(kit.id, &book).unwrap());
        assert!(verses[1].contains(&book.get(kit.guard).unwrap().name.to_uppercase()));
        if let Some(id) = kit.hand.first() {
            assert!(verses[2].contains(&book.get(id).unwrap().name.to_uppercase()));
        }
        if let Some(id) = kit.spells.last() {
            assert!(verses[3].contains(&book.get(id).unwrap().name.to_uppercase()));
        }
        assert!(verses[4].contains(&book.get(kit.arm).unwrap().name.to_uppercase()));
        assert!(verses[5].contains(&book.get(kit.guard).unwrap().name.to_uppercase()));
        if let Some(id) = kit.spells.first() {
            assert!(verses[6].contains(&book.get(id).unwrap().name.to_uppercase()));
        }
        if let Some(id) = kit.hand.last() {
            assert!(verses[7].contains(&book.get(id).unwrap().name.to_uppercase()));
        }
        for beat in 0..8 {
            assert_eq!(company_verse(kit.id, &book, beat + 8).as_ref(), Some(&verses[beat]));
            for other in beat + 1..8 { assert_ne!(verses[beat], verses[other]); }
        }
    }
    assert!(company_verse("missing-kit", &book, 3).is_none());
}

#[test]
fn votive_and_brineheart_are_defensive_world_rewards() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("votive-aegis", Kind::Spell, vec![Effect::Heal(8), Effect::Ward(2)]),
        ("brineheart-fig", Kind::Play, vec![Effect::Heal(12), Effect::Nova(20)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("votive-aegis").unwrap().cooldown, 18);
}

#[test]
fn reliquary_ranger_recruits_with_real_seeking_thunder_and_aegis() {
    use crate::rules::{alehouse, cards::{Effect, Kind}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("sabine").unwrap();
    assert_eq!(kit.hand, &["needle-cathedra"]);
    assert_eq!(kit.spells, &["stormglass-writ", "votive-aegis"]);
    let relic = run.book.get(kit.hand[0]).unwrap();
    assert!(relic.effects.contains(&Effect::Pierce(1)));
    assert!(relic.effects.contains(&Effect::Homing(1)));
    for id in kit.spells { assert_eq!(run.book.get(id).unwrap().kind, Kind::Spell); }
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.knight.as_deref(), Some("sabine"));
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["needle-cathedra"]);
    assert_eq!(hero.spells[0].as_deref(), Some("stormglass-writ"));
    assert_eq!(hero.spells[1].as_deref(), Some("votive-aegis"));
    assert_eq!(hero.hp, 75 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}

#[test]
fn vesper_and_moonmilk_are_distinct_warded_world_loot() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("vesper-knell", Kind::Spell, vec![Effect::Nova(24), Effect::Ward(2)]),
        ("moonmilk-flask", Kind::Play, vec![Effect::Heal(10), Effect::Ward(1)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn last_call_ricochet_has_a_playable_bankshot_kit_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect, Kind}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("ivo").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    let medal = run.book.get("bankshot-medallion").unwrap();
    assert_eq!(medal.kind, Kind::Hold);
    assert!(medal.effects.contains(&Effect::Bounce(1)));
    assert!(medal.effects.contains(&Effect::Velocity(15)));
    assert_eq!(run.book.get("vesper-knell").unwrap().kind, Kind::Spell);
    assert_eq!(run.book.get("moonmilk-flask").unwrap().kind, Kind::Play);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.knight.as_deref(), Some("ivo"));
    assert_eq!(hero.arm.as_deref(), Some("handgonne"));
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["bankshot-medallion", "moonmilk-flask"]);
    assert_eq!(hero.spells[0].as_deref(), Some("vesper-knell"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 85 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}

#[test]
fn orrery_warden_brings_trait_carrying_orbits_to_a_reachable_tavern_seat() {
    use crate::rules::{alehouse, cards::{Effect, Shape}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("maelis").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    let frost = run.book.get("whiteout-muzzle").unwrap();
    assert!(frost.effects.contains(&Effect::Chill(1)));
    assert!(frost.effects.contains(&Effect::Knock(2)));
    assert!(run.book.get("orrery-collapse").unwrap().effects.contains(&Effect::Cast(Shape::Ring)));
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["whiteout-muzzle", "hearthglass-pear"]);
    assert_eq!(hero.spells[0].as_deref(), Some("orrery-collapse"));
    assert_eq!(hero.spells[1].as_deref(), Some("votive-aegis"));
    assert!(hero.spells[2..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 80 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&p| alehouse::patron_visible(p, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "recruit shares a visible seat");
        }
    }
}

#[test]
fn rimewheel_and_kilnwing_are_distinct_tactical_world_loot() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("rimewheel-escapement", Kind::Hold, vec![Effect::Bounce(1), Effect::Chill(1)]),
        ("kilnwing-reprieve", Kind::Spell, vec![Effect::Nova(20), Effect::Ward(1)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("kilnwing-reprieve").unwrap().cooldown, 14);
}

#[test]
fn arcwell_and_fulgurite_are_distinct_trait_driven_world_loot() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("fulgurite-fork", Kind::Hold, vec![Effect::Chain(1)]),
        ("arcwell-salute", Kind::Spell, vec![Effect::Cast(Shape::Ring)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("arcwell-salute").unwrap().cooldown, 13);
}

#[test]
fn stormline_ferrier_has_chain_ring_shield_and_reachable_tavern_seat() {
    use crate::rules::{alehouse, cards::{Effect, Shape}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("nera").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("fulgurite-fork").unwrap().effects, vec![Effect::Chain(1)]);
    assert_eq!(run.book.get("arcwell-salute").unwrap().effects, vec![Effect::Cast(Shape::Ring)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard, Guard::Shield);
    assert_eq!(hero.guard_card.as_deref(), Some("kite-shield"));
    assert_eq!(hero.hand, vec!["fulgurite-fork", "moonmilk-flask"]);
    assert_eq!(hero.spells[0].as_deref(), Some("arcwell-salute"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 80 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}

#[test]
fn drowned_watch_rewards_have_valid_unique_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("wake-lantern", Kind::Hold, vec![Effect::Homing(1), Effect::Chill(1)]),
        ("undertow-benediction", Kind::Spell, vec![Effect::Heal(8), Effect::Ward(2)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("undertow-benediction").unwrap().cooldown, 17);
}

#[test]
fn drowned_watch_has_guided_chill_rescue_kit_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("bellweather").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("wake-lantern").unwrap().effects, vec![Effect::Homing(1), Effect::Chill(1)]);
    assert_eq!(run.book.get("undertow-benediction").unwrap().effects, vec![Effect::Heal(8), Effect::Ward(2)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard, Guard::Roll);
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["wake-lantern", "brineheart-fig"]);
    assert_eq!(hero.spells[0].as_deref(), Some("undertow-benediction"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 75 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}

#[test]
fn sundial_and_adder_are_distinct_artillery_world_rewards() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("sundial-mortar", Kind::Spell, vec![Effect::Cast(Shape::Meteor)]),
        ("adder-hourglass", Kind::Hold, vec![Effect::Pierce(1), Effect::Poison(1)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("sundial-mortar").unwrap().cooldown, 19);
}

#[test]
fn venom_bombardier_has_trait_driven_artillery_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("corvin").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("adder-hourglass").unwrap().effects, vec![Effect::Pierce(1), Effect::Poison(1)]);
    assert_eq!(run.book.get("sundial-mortar").unwrap().effects, vec![Effect::Cast(crate::rules::cards::Shape::Meteor)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard, Guard::Shield);
    assert_eq!(hero.guard_card.as_deref(), Some("kite-shield"));
    assert_eq!(hero.hand, vec!["adder-hourglass", "moonmilk-flask"]);
    assert_eq!(hero.spells[0].as_deref(), Some("sundial-mortar"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 80 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}

#[test]
fn signature_spells_can_be_discovered_without_recruiting_their_owner() {
    use crate::rules::cards::{self, Kind};
    let book = crate::rules::Book::builtin();
    let ids = ["borrowed-minute", "cinder-tithe", "eclipse-toll", "eye-of-the-squall", "glasshouse-mercy", "keelbreaker", "last-light", "mercy-lantern", "noon-shatter", "pressure-seal", "storm-receipt", "thorn-testament"];
    for id in ids {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id)
            .unwrap_or_else(|| panic!("missing built-in {id}"));
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Spell, "{id}");
        assert_eq!(card.drop, 1, "{id}: rare monster discovery");
        assert_eq!(card.chest, 2, "{id}: chest-favored discovery");
        assert!(card.only_in.is_none(), "{id}: region-independent discovery");
        assert_eq!(card.effects, checked.card.effects);
        assert!(!card.effects.is_empty(), "{id}");
        assert!(card.cooldown > 0, "{id}");
        assert_eq!(card.art.len(), 16, "{id}");
        assert!(card.art.iter().all(|row| row.len() == 16), "{id}");
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1, "{id}: distinct art");
    }
}

#[test]
fn widow_and_thunder_are_distinct_sustain_and_escape_rewards() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("widows-spindle", Kind::Hold, vec![Effect::Vamp(2), Effect::Pierce(1)]),
        ("last-call-thunder", Kind::Play, vec![Effect::Nova(28), Effect::Ward(2)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, kind);
        assert_eq!(card.effects, effects);
        assert_eq!(checked.card.effects, effects);
        assert!(card.drop > 0 && card.chest > 0);
        assert!(card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn velvet_breacher_has_piercing_sustain_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("mirelle").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("widows-spindle").unwrap().effects, vec![Effect::Vamp(2), Effect::Pierce(1)]);
    assert_eq!(run.book.get("vesper-knell").unwrap().effects, vec![Effect::Nova(24), Effect::Ward(2)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("handgonne"));
    assert_eq!(hero.guard, Guard::Roll);
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["widows-spindle", "last-call-thunder"]);
    assert_eq!(hero.spells[0].as_deref(), Some("vesper-knell"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 75 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}


#[test]
fn furnace_pair_has_valid_unique_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("cinderpin-escapement", Kind::Hold, vec![Effect::Bounce(1), Effect::Burn(1)]),
        ("furnace-curtain", Kind::Spell, vec![Effect::Cast(Shape::Ring), Effect::Ward(1)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("furnace-curtain").unwrap().cooldown, 18);
}


#[test]
fn furnace_usher_has_burning_bankshots_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("coalglass").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("cinderpin-escapement").unwrap().effects, vec![Effect::Bounce(1), Effect::Burn(1)]);
    assert_eq!(run.book.get("furnace-curtain").unwrap().effects, vec![Effect::Cast(crate::rules::cards::Shape::Ring), Effect::Ward(1)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("handgonne"));
    assert_eq!(hero.guard, Guard::Roll);
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["cinderpin-escapement", "moonmilk-flask"]);
    assert_eq!(hero.spells[0].as_deref(), Some("furnace-curtain"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 80 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}


#[test]
fn plague_artillery_pair_has_valid_unique_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("plague-switchboard", Kind::Hold, vec![Effect::Chain(1), Effect::Poison(1)]),
        ("quarantine-star", Kind::Spell, vec![Effect::Cast(Shape::Meteor), Effect::Ward(1)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("quarantine-star").unwrap().cooldown, 20);
}



#[test]
fn quarantine_signaler_has_chain_poison_artillery_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("sablewire").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("plague-switchboard").unwrap().effects, vec![Effect::Chain(1), Effect::Poison(1)]);
    assert_eq!(run.book.get("quarantine-star").unwrap().effects, vec![Effect::Cast(crate::rules::cards::Shape::Meteor), Effect::Ward(1)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_ne!(hero.guard, Guard::Roll);
    assert_eq!(hero.guard_card.as_deref(), Some("kite-shield"));
    assert_eq!(hero.hand, vec!["plague-switchboard", "moonmilk-flask"]);
    assert_eq!(hero.spells[0].as_deref(), Some("quarantine-star"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 80 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}


#[test]
fn redoubt_duelist_pair_has_valid_unique_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("redoubt-tuning-fork", Kind::Hold, vec![Effect::Knock(1), Effect::Vamp(1)]),
        ("velvet-rebuttal", Kind::Spell, vec![Effect::Cast(Shape::Bolt), Effect::Ward(1)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("velvet-rebuttal").unwrap().cooldown, 12);
}


#[test]
fn redoubt_second_has_spacing_sustain_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("oren").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("redoubt-tuning-fork").unwrap().effects, vec![Effect::Knock(1), Effect::Vamp(1)]);
    assert_eq!(run.book.get("velvet-rebuttal").unwrap().effects, vec![Effect::Cast(crate::rules::cards::Shape::Bolt), Effect::Ward(1)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard, Guard::Roll);
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["redoubt-tuning-fork", "last-call-thunder"]);
    assert_eq!(hero.spells[0].as_deref(), Some("velvet-rebuttal"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 75 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}

#[test]
fn whiteout_pair_has_valid_unique_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("rimewheel-bearing", Kind::Hold, vec![Effect::Bounce(1), Effect::Chill(1)]),
        ("whiteout-parasol", Kind::Spell, vec![Effect::Cast(Shape::Ring), Effect::Ward(2)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("whiteout-parasol").unwrap().cooldown, 16);
}



#[test]
fn whiteout_concierge_has_frost_bankshots_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("linet").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("rimewheel-bearing").unwrap().effects, vec![Effect::Bounce(1), Effect::Chill(1)]);
    assert_eq!(run.book.get("whiteout-parasol").unwrap().effects, vec![Effect::Cast(crate::rules::cards::Shape::Ring), Effect::Ward(2)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("handgonne"));
    assert_eq!(hero.guard, Guard::Roll);
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["rimewheel-bearing", "moonmilk-flask"]);
    assert_eq!(hero.spells[0].as_deref(), Some("whiteout-parasol"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 75 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}


#[test]
fn stormrail_pair_has_valid_unique_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("stormrail-ticket", Kind::Hold, vec![Effect::Pierce(1), Effect::Chain(1)]),
        ("thunderhead-gavel", Kind::Spell, vec![Effect::Cast(Shape::Meteor), Effect::Ward(1)]),
    ] {
        assert_eq!(cards::BUILTIN.iter().filter(|(key, _)| *key == id).count(), 1);
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("thunderhead-gavel").unwrap().cooldown, 18);
}

#[test]
fn stormrail_magistrate_has_chain_verdict_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("brasswrit").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("stormrail-ticket").unwrap().effects, vec![Effect::Pierce(1), Effect::Chain(1)]);
    assert_eq!(run.book.get("thunderhead-gavel").unwrap().effects, vec![Effect::Cast(crate::rules::cards::Shape::Meteor), Effect::Ward(1)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard, Guard::Roll);
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["stormrail-ticket", "moonmilk-flask"]);
    assert_eq!(hero.spells[0].as_deref(), Some("thunderhead-gavel"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 75 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}


#[test]
fn orrery_pair_has_valid_unique_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("orrery-heart", Kind::Hold, vec![Effect::Orbit(1), Effect::Homing(1)]),
        ("eventide-key", Kind::Play, vec![Effect::Blink(6), Effect::Ward(1)]),
    ] {
        assert_eq!(cards::BUILTIN.iter().filter(|(key, _)| *key == id).count(), 1);
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("eventide-key").unwrap().cooldown, 0);
}


#[test]
fn last_astronomer_has_orbital_escape_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("astrolabe").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("orrery-heart").unwrap().effects, vec![Effect::Orbit(1), Effect::Homing(1)]);
    assert_eq!(run.book.get("whiteout-parasol").unwrap().effects, vec![Effect::Cast(crate::rules::cards::Shape::Ring), Effect::Ward(2)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("handgonne"));
    assert_eq!(hero.guard, Guard::Roll);
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["orrery-heart", "eventide-key"]);
    assert_eq!(hero.spells[0].as_deref(), Some("whiteout-parasol"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 75 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}


#[test]
fn widowglass_pair_has_valid_unique_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("widowglass-ampoule", Kind::Hold, vec![Effect::Poison(1), Effect::Pierce(1)]),
        ("mothwake-lantern", Kind::Spell, vec![Effect::Cast(cards::Shape::Ring), Effect::Ward(1)]),
    ] {
        assert_eq!(cards::BUILTIN.iter().filter(|(key, _)| *key == id).count(), 1);
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("mothwake-lantern").unwrap().cooldown, 14);
}


#[test]
fn mothwake_apothecary_has_venom_ring_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("fenella").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("widowglass-ampoule").unwrap().effects, vec![Effect::Poison(1), Effect::Pierce(1)]);
    assert_eq!(run.book.get("mothwake-lantern").unwrap().effects, vec![Effect::Cast(crate::rules::cards::Shape::Ring), Effect::Ward(1)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard, Guard::Roll);
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["widowglass-ampoule", "moonmilk-flask"]);
    assert_eq!(hero.spells[0].as_deref(), Some("mothwake-lantern"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 75 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}



#[test]
fn keeltooth_pair_has_valid_unique_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("keeltooth-spur", Kind::Hold, vec![Effect::Fork(1), Effect::Knock(1)]),
        ("drydock-salute", Kind::Spell, vec![Effect::Cast(cards::Shape::Bolt), Effect::Ward(1)]),
    ] {
        assert_eq!(cards::BUILTIN.iter().filter(|(key, _)| *key == id).count(), 1);
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("drydock-salute").unwrap().cooldown, 12);
}


#[test]
fn summon_consumables_keep_their_effects() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effect) in [("choir-crumb", Effect::Linnet(1)), ("rime-whistle", Effect::Moth(45))] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        assert_eq!(checked.card.kind, Kind::Play);
        assert_eq!(checked.card.effects, vec![effect]);
        // Their reviewed art stands; the 16x16 redraws wait for review
        // (`refs/combine/astra-night`).
        let card = book.get(id).unwrap();
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

/// Check the whole staged company, not just the most recently added recruit.
#[test]
fn entire_tavern_roster_has_unique_reachable_wrapping_seats() {
    use crate::rules::{alehouse, heroes, TILE_UNITS};
    use std::collections::BTreeSet;
    let mut kits = BTreeSet::new();
    for (i, patron) in alehouse::PATRONS.iter().enumerate() {
        assert!(kits.insert(patron.kit), "duplicate patron kit {}", patron.kit);
        assert!(knight(patron.kit).is_some(), "unknown patron {}", patron.kit);
        let pages: Vec<_> = (0..alehouse::COMPANY_PAGES)
            .filter(|&page| alehouse::patron_visible(page, i)).collect();
        let permanent = heroes::KITS.contains(&patron.kit);
        // The whole company sits in the hall at once, on every page.
        assert_eq!(pages.len(), alehouse::COMPANY_PAGES,
            "wrong visibility count for {}", patron.kit);
        for page in pages {
            assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(i), 
                patron.at.0 * TILE_UNITS, patron.at.1 * TILE_UNITS, 0, page),
                Some(alehouse::Spot::Patron(index)) if index == i),
                "unreachable patron {} on page {}", patron.kit, page);
            if !permanent {
                assert!(alehouse::company_roll_call(page).contains(patron.label));
            }
            for (j, other) in alehouse::PATRONS.iter().enumerate() {
                if j != i && alehouse::patron_visible(page, j) {
                    assert_ne!(patron.at, other.at, "{} overlaps {}", patron.kit, other.kit);
                }
            }
        }
        for page in 0..alehouse::COMPANY_PAGES {
            assert_eq!(alehouse::patron_visible(page, i),
                alehouse::patron_visible(page + alehouse::COMPANY_PAGES, i));
        }
    }
    for page in 0..alehouse::COMPANY_PAGES {
        let rotating = alehouse::PATRONS.iter().enumerate()
            .filter(|(i, p)| !heroes::KITS.contains(&p.kit) && alehouse::patron_visible(page, *i)).count();
        assert_eq!(rotating, alehouse::PATRONS.iter().filter(|p| !heroes::KITS.contains(&p.kit)).count());
        assert!(!alehouse::patron_visible(page, alehouse::PATRONS.len()));
    }
}

#[test]
fn permafrost_pair_has_valid_unique_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("permafrost-flywheel", Kind::Hold, vec![Effect::Bounce(1), Effect::Chill(1)]),
        ("zero-hour-bell", Kind::Spell, vec![Effect::Cast(cards::Shape::Ring), Effect::Ward(1)]),
    ] {
        assert_eq!(cards::BUILTIN.iter().filter(|(key, _)| *key == id).count(), 1);
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("zero-hour-bell").unwrap().cooldown, 15);
}


#[test]
fn festival_contraband_has_distinct_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effects) in [
        ("masquerade-matchbox", vec![Effect::Blink(6), Effect::Ward(1)]),
        ("confetti-antidote", vec![Effect::Heal(8), Effect::Immune(4)]),
        ("velvet-rope-ticket", vec![Effect::Blink(8), Effect::Immune(4)]),
    ] {
        assert_eq!(cards::BUILTIN.iter().filter(|(key, _)| *key == id).count(), 1);
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        assert_eq!(checked.card.effects, effects);
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Play);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

/// Storm apiary is supplied as real world loot, not just gallery-only art.
#[test]
fn storm_apiary_has_distinct_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects, cooldown) in [
        ("thundercomb", Kind::Hold, vec![Effect::Homing(1), Effect::Chain(1)], 0),
        ("queenstorm-decree", Kind::Spell,
            vec![Effect::Cast(Shape::Ring), Effect::Ward(1)], 14),
        ("bottled-squall-honey", Kind::Play,
            vec![Effect::Heal(6), Effect::Nova(20)], 0),
    ] {
        assert_eq!(cards::BUILTIN.iter().filter(|(key, _)| *key == id).count(), 1);
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        assert_eq!(checked.card.effects, effects);
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, kind);
        if cooldown > 0 { assert_eq!(card.cooldown, cooldown); }
        assert_eq!((card.drop, card.chest), (2, 3));
        assert!(card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

/// Eclipse railway is supplied as real world loot, not just gallery-only art.
#[test]
fn eclipse_railway_has_distinct_art_and_world_supply() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects, cooldown) in [
        ("umbra-switchfork", Kind::Hold, vec![Effect::Pierce(1), Effect::Chain(1)], 0),
        ("last-platform-signal", Kind::Spell,
            vec![Effect::Cast(Shape::Ring), Effect::Ward(2)], 16),
        ("eclipse-punchcard", Kind::Play,
            vec![Effect::Blink(8), Effect::Immune(3)], 0),
    ] {
        assert_eq!(cards::BUILTIN.iter().filter(|(key, _)| *key == id).count(), 1);
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        assert_eq!(checked.card.effects, effects);
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, kind);
        if cooldown > 0 { assert_eq!(card.cooldown, cooldown); }
        assert_eq!((card.drop, card.chest), (2, 3));
        assert!(card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn eclipse_conductor_has_railway_kit_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("nocturne").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("umbra-switchfork").unwrap().effects, vec![Effect::Pierce(1), Effect::Chain(1)]);
    assert_eq!(run.book.get("last-platform-signal").unwrap().effects, vec![Effect::Cast(crate::rules::cards::Shape::Ring), Effect::Ward(2)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard, Guard::Roll);
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["umbra-switchfork", "eclipse-punchcard"]);
    assert_eq!(hero.spells[0].as_deref(), Some("last-platform-signal"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 75 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}

#[test]
fn storm_apiarist_has_swarm_kit_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("vespra").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("thundercomb").unwrap().effects, vec![Effect::Homing(1), Effect::Chain(1)]);
    assert_eq!(run.book.get("queenstorm-decree").unwrap().effects, vec![Effect::Cast(crate::rules::cards::Shape::Ring), Effect::Ward(1)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("handgonne"));
    assert_eq!(hero.guard, Guard::Roll);
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["thundercomb", "bottled-squall-honey"]);
    assert_eq!(hero.spells[0].as_deref(), Some("queenstorm-decree"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 80 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}

#[test]
fn zero_hour_millwright_has_banked_frost_kit_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("bront").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("permafrost-flywheel").unwrap().effects, vec![Effect::Bounce(1), Effect::Chill(1)]);
    assert_eq!(run.book.get("zero-hour-bell").unwrap().effects, vec![Effect::Cast(crate::rules::cards::Shape::Ring), Effect::Ward(1)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard_card.as_deref(), Some("kite-shield"));
    assert_eq!(hero.guard_card.as_deref(), Some("kite-shield"));
    assert_eq!(hero.hand, vec!["permafrost-flywheel", "confetti-antidote"]);
    assert_eq!(hero.spells[0].as_deref(), Some("zero-hour-bell"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 85 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}

#[test]
fn revel_marshal_has_escape_kit_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("maribel").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("masquerade-matchbox").unwrap().effects, vec![Effect::Blink(6), Effect::Ward(1)]);
    assert_eq!(run.book.get("velvet-rope-ticket").unwrap().effects, vec![Effect::Blink(8), Effect::Immune(4)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("handgonne"));
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["masquerade-matchbox", "velvet-rope-ticket", "confetti-antidote"]);
    assert_eq!(hero.spells[0].as_deref(), Some("rook-mortar"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 70 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}

#[test]
fn blacktide_quartermaster_has_broadside_kit_and_reachable_seat() {
    use crate::rules::{alehouse, cards::{Effect}};
    let mut run = Run::new(3, 1, None);
    let kit = knight("nerissa").unwrap();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    assert_eq!(run.book.get("abyssal-compass").unwrap().effects, vec![Effect::Rate(5), Effect::MaxHp(8)]);
    assert_eq!(run.book.get("blacktide-censer").unwrap().effects, vec![Effect::Nova(36), Effect::Ward(1)]);
    assert_eq!(run.book.get("brineheart-fig").unwrap().effects, vec![Effect::Heal(12), Effect::Nova(20)]);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.arm.as_deref(), Some("handgonne"));
    assert_eq!(hero.guard_card.as_deref(), Some("dodge-roll"));
    assert_eq!(hero.hand, vec!["abyssal-compass", "brineheart-fig"]);
    assert_eq!(hero.spells[0].as_deref(), Some("blacktide-censer"));
    assert!(hero.spells[1..].iter().all(Option::is_none));
    assert_eq!(hero.hp, 70 + hero.home_hp);
    assert_eq!(hero.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, index)).unwrap();
    assert!(alehouse::company_roll_call(page).contains(patron.label));
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert_ne!(other.at, patron.at, "shared visible seat with {}", other.kit);
        }
    }
}

#[test]
fn entire_company_has_unique_reachable_tavern_seats_across_page_wraps() {
    use crate::rules::{alehouse, heroes, TILE_UNITS};
    use std::collections::BTreeSet;

    let mut seated = BTreeSet::new();
    for patron in alehouse::PATRONS {
        assert!(knight(patron.kit).is_some(), "unknown patron kit {}", patron.kit);
        assert!(seated.insert(patron.kit), "duplicate patron {}", patron.kit);
    }
    for kit in COMPANY {
        assert!(seated.contains(kit.id), "{} has no tavern seat", kit.id);
    }
    let mut appearances = vec![0; alehouse::PATRONS.len()];
    for page in 0..alehouse::COMPANY_PAGES {
        let mut rotating = 0;
        for (index, patron) in alehouse::PATRONS.iter().enumerate() {
            let visible = alehouse::patron_visible(page, index);
            assert_eq!(visible, alehouse::patron_visible(page + alehouse::COMPANY_PAGES, index),
                "{} changes visibility after page wrap", patron.kit);
            if !visible { continue; }
            appearances[index] += 1;
            if !heroes::KITS.contains(&patron.kit) {
                rotating += 1;
                assert!(alehouse::company_roll_call(page).contains(patron.label));
            }
            // Sampling the moving cat across a full patrol also catches a
            // nearer interaction stealing a recruit's exact standing point.
            for tick in [0, 1, 60, 300, 900, 1800, 3600] {
                assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), 
                    patron.at.0 * TILE_UNITS, patron.at.1 * TILE_UNITS, tick, page),
                    Some(alehouse::Spot::Patron(found)) if found == index),
                    "{} unreachable on page {page}, tick {tick}", patron.kit);
            }
        }
        assert_eq!(rotating, alehouse::PATRONS.iter().filter(|p| !heroes::KITS.contains(&p.kit)).count(),
            "page {page} seats the whole company");
        assert!(!alehouse::patron_visible(page, alehouse::PATRONS.len()));
    }
    for (index, patron) in alehouse::PATRONS.iter().enumerate() {
        assert_eq!(appearances[index], alehouse::COMPANY_PAGES, "{} page coverage", patron.kit);
    }
}

#[test]
fn lantern_jury_and_blackberry_buckler_are_lootable_checked_assets() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, effects) in [
        ("lantern-jury", Kind::Spell, vec![Effect::Cast(Shape::Ring), Effect::Ward(2)]),
        ("blackberry-buckler", Kind::Play, vec![Effect::Heal(16), Effect::Nova(20)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.kind, kind);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
    assert_eq!(book.get("lantern-jury").unwrap().cooldown, 18);
}

#[test]
fn comet_oyster_and_censer_are_checked_loot_with_distinct_art() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, cooldown, effects) in [
        ("comet-siphon", Kind::Spell, 16, vec![Effect::Cast(Shape::Bolt), Effect::Blink(4)]),
        ("oyster-aegis", Kind::Play, 0, vec![Effect::Heal(12), Effect::Ward(3)]),
        ("last-call-censer", Kind::Spell, 22, vec![Effect::Cast(Shape::Ring), Effect::Heal(6)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.kind, kind);
        assert_eq!(card.cooldown, cooldown);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn comet_ferrier_is_outfitted_and_reachable_in_the_tavern() {
    use crate::rules::{alehouse, cards::Kind};
    let kit = knight("pearlrunner").unwrap();
    let book = crate::rules::Book::builtin();
    assert_eq!(kit.hand, &["oyster-aegis"]);
    assert_eq!(kit.spells, &["comet-siphon", "last-call-censer"]);
    for id in kit.spells { assert_eq!(book.get(id).unwrap().kind, Kind::Spell); }
    let mut run = Run::new(3, 1, None);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.max_hp, 80);
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard, Guard::Roll);
    assert_eq!(hero.colours(), kit.colours);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&p| alehouse::patron_visible(p, index)).unwrap();
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}

#[test]
fn railwake_and_bento_load_as_distinct_checked_loot() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, cooldown, effects) in [
        ("railwake-prism", Kind::Spell, 19, vec![Effect::Cast(Shape::Bolt), Effect::Ward(2)]),
        ("embercrab-bento", Kind::Play, 0, vec![Effect::Heal(10), Effect::Nova(24)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.kind, kind);
        assert_eq!(card.cooldown, cooldown);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn undertow_collection_is_checked_distinct_reachable_loot() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, cooldown, effects) in [
        ("undertow-carillon", Kind::Spell, 21, vec![Effect::Cast(Shape::Ring), Effect::Blink(4)]),
        ("cinder-votive", Kind::Play, 0, vec![Effect::Nova(18), Effect::Ward(2)]),
        ("pearlwake-flask", Kind::Play, 0, vec![Effect::Heal(18), Effect::Blink(4)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.kind, kind);
        assert_eq!(card.cooldown, cooldown);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn breach_collection_is_checked_distinct_reachable_loot() {
    use crate::rules::cards::{self, Effect, Kind, Shape};
    let book = crate::rules::Book::builtin();
    for (id, kind, cooldown, effects) in [
        ("starfall-warrant", Kind::Spell, 24, vec![Effect::Cast(Shape::Meteor), Effect::Ward(2)]),
        ("breachbell-tonic", Kind::Play, 0, vec![Effect::Nova(24), Effect::Heal(12)]),
        ("glassway-ticket", Kind::Play, 0, vec![Effect::Ward(3), Effect::Blink(4)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.kind, kind);
        assert_eq!(card.cooldown, cooldown);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn tollhouse_equipment_is_checked_distinct_reachable_loot() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effect) in [
        ("tollkeepers-visor", Effect::Armor(1)),
        ("needlewake-spool", Effect::Pierce(1)),
        ("redshift-cuff", Effect::Vamp(1)),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, vec![effect]);
        assert_eq!(card.kind, Kind::Hold);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn gallows_and_rookstep_are_checked_distinct_reachable_spells() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, cooldown, effects) in [
        ("gallows-sun", 23, vec![Effect::Nova(22), Effect::Ward(2)]),
        ("rookstep-sigil", 18, vec![Effect::Blink(4), Effect::Heal(12)]),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, effects);
        assert_eq!(card.kind, Kind::Spell);
        assert_eq!(card.cooldown, cooldown);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn escapement_and_tempest_are_checked_unique_reachable_equipment() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effect) in [
        ("escapement-heart", Effect::Rate(10)),
        ("tempest-skate", Effect::Speed(10)),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, vec![effect]);
        assert_eq!(card.effects, checked.card.effects);
        assert_eq!(card.kind, Kind::Hold);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn apiary_and_ampoule_are_checked_unique_reachable_summons() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effect) in [
        ("vesper-apiary", Effect::Moth(30)),
        ("sanguine-ampoule", Effect::Leech(1)),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(checked.card.effects, vec![effect]);
        assert_eq!(card.effects, checked.card.effects);
        assert_eq!(card.kind, Kind::Play);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn candlewick_exorcist_has_real_summons_and_a_reachable_tavern_seat() {
    use crate::rules::{alehouse, cards::{Kind, Effect}};
    let kit = knight("waxwarden").unwrap();
    let book = crate::rules::Book::builtin();
    assert_eq!(kit.hand, &["needlewake-spool", "vesper-apiary", "sanguine-ampoule"]);
    assert_eq!(book.get(kit.hand[0]).unwrap().effects, vec![Effect::Pierce(1)]);
    assert_eq!(book.get(kit.hand[1]).unwrap().effects, vec![Effect::Moth(30)]);
    assert_eq!(book.get(kit.hand[2]).unwrap().effects, vec![Effect::Leech(1)]);
    assert_eq!(kit.spells, &["gallows-sun", "rookstep-sigil"]);
    for id in kit.spells { assert_eq!(book.get(id).unwrap().kind, Kind::Spell); }
    let mut run = Run::new(3, 1, None);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.max_hp, 90);
    assert_eq!(hero.arm.as_deref(), Some("handgonne"));
    assert_eq!(hero.guard, Guard::Shield);
    assert_eq!(hero.colours(), kit.colours);
    assert_eq!(kit.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&p| alehouse::patron_visible(p, index)).unwrap();
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, patron.at.1 * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}

#[test]
fn last_call_service_has_checked_unique_loot_and_summon_roles() {
    use crate::rules::cards::{self, Effect, Kind};
    use crate::rules::imps::Behaviour;
    let book = crate::rules::Book::builtin();
    for (id, kind, effect) in [
        ("siege-samovar", Kind::Spell, Effect::Summon(1, Behaviour::Turret)),
        ("last-call-saucers", Kind::Play, Effect::Summon(3, Behaviour::Guard)),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, kind);
        assert_eq!(card.effects, vec![effect]);
        assert_eq!(checked.card.effects, card.effects);
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn parlour_tactics_are_unique_registered_reusable_loot() {
    use crate::rules::cards::{self, Effect, Kind};
    let book = crate::rules::Book::builtin();
    for (id, effect, cooldown) in [
        ("borrowed-regular", Effect::Imposture(1), 19),
        ("parlour-labyrinth", Effect::Hedge(1), 21),
    ] {
        let (_, source) = cards::BUILTIN.iter().find(|(key, _)| *key == id).unwrap();
        let checked = cards::check(id, source).unwrap();
        let card = book.get(id).unwrap();
        assert_eq!(card.kind, Kind::Spell);
        assert_eq!(card.effects, vec![effect]);
        assert_eq!(checked.card.effects, card.effects);
        assert!(source.contains(&format!("cooldown {cooldown}\n")));
        assert!(card.drop > 0 && card.chest > 0 && card.only_in.is_none());
        assert_eq!(card.art.len(), 16);
        assert!(card.art.iter().all(|row| row.len() == 16));
        assert_eq!(book.cards.iter().filter(|other| other.art == card.art).count(), 1);
    }
}

#[test]
fn siege_host_has_checked_service_loadout_and_reachable_tavern_seat() {
    use crate::rules::{alehouse, cards::Kind};
    let kit = knight("vesperhost").unwrap();
    let book = crate::rules::Book::builtin();
    assert_eq!(COMPANY.iter().filter(|k| k.id == kit.id).count(), 1);
    for id in kit.hand { assert!(book.get(id).is_some()); }
    assert_eq!(kit.spells, &["siege-samovar", "borrowed-regular"]);
    for id in kit.spells { assert_eq!(book.get(id).unwrap().kind, Kind::Spell); }
    let mut run = Run::new(3, 1, None);
    run.outfit(1, kit);
    let hero = &run.players[&1];
    assert_eq!(hero.max_hp, 85);
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.guard, Guard::Shield);
    assert_eq!(hero.colours(), kit.colours);
    assert_eq!(kit.bombs, 0);
    let index = alehouse::PATRONS.iter().position(|p| p.kit == kit.id).unwrap();
    let patron = &alehouse::PATRONS[index];
    let page = (0..alehouse::COMPANY_PAGES).find(|&p| alehouse::patron_visible(p, index)).unwrap();
    for (i, other) in alehouse::PATRONS.iter().enumerate() {
        if i != index && alehouse::patron_visible(page, i) {
            assert!((patron.at.0-other.at.0).hypot(patron.at.1-other.at.1) > 1.2, "shared page collision with {}", other.kit);
        }
    }
    assert_eq!(alehouse::room((0, 0)).tile(patron.at.0 as i32, (patron.at.1-1.0) as i32), crate::rules::Tile::Floor);
    assert!(matches!(alehouse::spot_near_in(alehouse::patron_room(index), patron.at.0 * crate::rules::TILE_UNITS, (patron.at.1-1.0) * crate::rules::TILE_UNITS, 0, page), Some(alehouse::Spot::Patron(i)) if i == index));
}
