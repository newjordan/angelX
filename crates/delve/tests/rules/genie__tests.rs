use super::*;

fn press(id: u32, input: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, input)])
}

fn cast(n: u8) -> Input {
    Input { cast: n, ..Default::default() }
}

/// A run whose knight 1 holds `plain` wishes and `great` super ones.
fn holding(plain: u8, great: u8) -> Run {
    let mut run = Run::new(4, 2, Some("Matt"));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.wishes, hero.great_wishes) = (plain, great);
    run
}

#[test]
fn the_scrolls_choices_are_the_same_for_the_same_seed_and_stay_on_the_ladder() {
    let book = phrasebook::book();
    for great in [0, 1] {
        for wish in book.iter().filter(|w| !w.learned) {
            let offer = |run: &mut Run| {
                run.offer_wish(1, &wish.name, Some(wish)).unwrap();
                run.players[&1].offer.clone().unwrap()
            };
            let (mut a, mut b) = (holding(1, great), holding(1, great));
            let offer_a = offer(&mut a);
            assert_eq!(offer_a, offer(&mut b), "{}: the same seed offers the same", wish.id);
            assert_eq!(offer_a.choices.len(), 3, "{}: three ways to have it", wish.id);
            assert_eq!(offer_a.choices[0].take, genie::Take::Said);
            assert_eq!(offer_a.choices[2].take, genie::Take::Deal, "the genie's deal is always offered");
            assert!(a.players[&1].wishes + a.players[&1].great_wishes > 0, "nothing is spent by asking");
            for choice in &offer_a.choices {
                let w = phrasebook::get(choice.wish.as_deref().unwrap()).unwrap();
                let tier = w.next_tier(&a.players[&1].deck).unwrap();
                let top = (tier + usize::from(choice.steps)).min(w.tiers.len()) - 1;
                // Every card a choice can grant is a tier of the ladder the
                // card checker passed.
                for t in tier..=top {
                    assert!(w.card(t).is_ok(), "{} tier {t}", w.id);
                }
                if choice.take == genie::Take::Bolder {
                    assert!(tier + usize::from(choice.steps) <= w.tiers.len(), "{}: bolder only with the step there", w.id);
                }
            }
        }
    }
}

#[test]
fn a_seed_changes_the_genies_mind_and_every_pick_stays_under_the_wish_caps() {
    let book = phrasebook::book();
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..24u64 {
        let mut run = Run::new(seed, 2, Some("Matt"));
        let hero = run.players.get_mut(&1).unwrap();
        (hero.wishes, hero.great_wishes) = (9, (seed % 2) as u8 * 3);
        for round in 0..9 {
            let Some(wish) = book.iter().filter(|w| !w.learned && w.next_tier(&run.players[&1].deck).is_some()).nth(round + seed as usize % 5).cloned() else {
                break;
            };
            if run.offer_wish(1, &wish.name, Some(&wish)).is_err() {
                continue;
            }
            let offer = run.players[&1].offer.clone().unwrap();
            seen.insert(format!("{:?} {:?}", offer.choices[1].take, offer.choices[2].tip.as_ref().map(std::mem::discriminant)));
            let _ = run.choose(1, (seed as usize + round) % offer.choices.len());
            let bonus = &run.players[&1].bonus;
            for (word, value) in [("damage", bonus.damage), ("rate", bonus.rate), ("shots", bonus.shots), ("pierce", bonus.pierce)] {
                assert!(value <= cards::cap(word), "{word} {value} over its cap");
            }
        }
    }
    assert!(seen.len() >= 3, "the genie's offers vary with the seed: {seen:?}");
}

#[test]
fn each_pick_seals_what_it_says() {
    let volley = phrasebook::get("volley").unwrap();
    // As you said: one step, the knight's wish spent.
    let mut run = holding(2, 0);
    run.offer_wish(1, "volley", Some(&volley)).unwrap();
    run.choose(1, 0).unwrap();
    assert!(run.players[&1].deck.contains(&volley.card_id(0)));
    assert_eq!(run.players[&1].wishes, 1);
    assert!(run.players[&1].offer.is_none());

    // Bolder: two steps, for a third of the knight's health.
    let mut run = holding(1, 0);
    let (hp, max) = (run.players[&1].hp, run.players[&1].max_hp);
    run.offer_wish(1, "volley", Some(&volley)).unwrap();
    let offer = run.players[&1].offer.clone().unwrap();
    let bolder = offer.choices.iter().position(|c| c.take == genie::Take::Bolder);
    if let Some(n) = bolder {
        run.choose(1, n).unwrap();
        assert!(run.players[&1].deck.contains(&volley.card_id(1)), "a step further");
        assert_eq!(run.players[&1].hp, hp.saturating_sub(max / genie::BLOOD_PRICE).max(1));
    } else {
        let whim = offer.choices.iter().position(|c| c.take == genie::Take::Whim).unwrap();
        let other = offer.choices[whim].wish.clone().unwrap();
        run.choose(1, whim).unwrap();
        assert!(run.players[&1].deck.iter().any(|c| c.starts_with(&format!("wish-{other}-"))));
    }
    assert_eq!(run.players[&1].wishes, 0);

    // The genie's deal: the wish, and his tip; the gauge stays where it was.
    let mut run = holding(1, 0);
    run.offer_wish(1, "volley", Some(&volley)).unwrap();
    let deal = run.players[&1].offer.clone().unwrap().choices[2].clone();
    run.players.get_mut(&1).unwrap().hp = 1;
    run.choose(1, 2).unwrap();
    let hero = &run.players[&1];
    assert!(hero.deck.contains(&volley.card_id(0)));
    match deal.tip.unwrap() {
        genie::Tip::Purse(gold) => assert_eq!(hero.carried.get(crate::realm::Spoil::Gold), gold),
        genie::Tip::Mend(gold) => {
            assert_eq!(hero.hp, hero.max_hp);
            assert_eq!(hero.carried.get(crate::realm::Spoil::Gold), gold);
        }
    }
    assert_eq!(run.genie.freedom, 0, "his deal fills none of his freedom");

    // Esc rolls it up: nothing spent.
    let mut run = holding(1, 0);
    run.offer_wish(1, "volley", Some(&volley)).unwrap();
    assert!(run.drop_offer(1));
    assert_eq!(run.players[&1].wishes, 1);
    // A super wish with no step past two (tier III is a teacher's): save
    // some, one step now and a plain wish kept.
    let mut run = holding(0, 1);
    run.offer_wish(1, "volley", Some(&volley)).unwrap();
    let offer = run.players[&1].offer.clone().unwrap();
    if let Some(n) = offer.choices.iter().position(|c| c.take == genie::Take::Split) {
        run.choose(1, n).unwrap();
        assert!(run.players[&1].deck.contains(&volley.card_id(0)));
        assert_eq!((run.players[&1].wishes, run.players[&1].great_wishes), (1, 0));
    }
}

#[test]
fn the_number_keys_pick_on_the_scroll_and_cast_nothing_while_held() {
    let volley = phrasebook::get("volley").unwrap();
    let mut run = holding(2, 0);
    run.offer_wish(1, "volley", Some(&volley)).unwrap();
    run.step(&press(1, cast(1)));
    assert!(run.players[&1].offer.is_none(), "1 picked");
    assert!(run.players[&1].deck.contains(&volley.card_id(0)));
    // Still held: taken by the scroll, not a spell.
    let held = press(1, cast(1));
    let out = run.offer_inputs(&held).unwrap_or(held);
    assert_eq!(out[&1].cast, 0);
    let released = press(1, Input::default());
    let out = run.offer_inputs(&released).unwrap_or(released);
    assert_eq!(out[&1].cast, 0);
    let again = press(1, cast(1));
    let out = run.offer_inputs(&again).unwrap_or(again);
    assert_eq!(out[&1].cast, 1, "let go and pressed again: the spell key again");
    // The menu key rolls a scroll up, for its own knight only.
    run.join_seat(2, "Ada");
    run.players.get_mut(&2).unwrap().wishes = 1;
    run.offer_wish(1, "volley", Some(&volley)).unwrap();
    run.offer_wish(2, "volley", Some(&volley)).unwrap();
    run.step(&press(1, Input { activity_menu: true, ..Default::default() }));
    assert!(run.players[&1].offer.is_none());
    assert!(run.players[&2].offer.is_some(), "another knight's scroll is theirs");
    assert_eq!(run.players[&1].wishes, 1, "nothing spent");
}

#[test]
fn the_gauge_fills_by_grade_and_tier_and_a_super_wish_counts_double() {
    use artistry::Grade;
    assert_eq!(genie::freedom_of(Some(Grade::Crude), 0, false), 1);
    assert_eq!(genie::freedom_of(None, 0, false), 2);
    assert_eq!(genie::freedom_of(Some(Grade::Masterwork), 1, false), 6);
    assert_eq!(genie::freedom_of(Some(Grade::Legendary), 2, false), 12, "a Legendary top tier fills most");
    assert_eq!(genie::freedom_of(Some(Grade::Legendary), 2, true), 24, "super wishes count double");
    let volley = phrasebook::get("volley").unwrap();
    let mut run = holding(1, 1);
    run.offer_wish(1, "volley", Some(&volley)).unwrap();
    run.choose(1, 0).unwrap();
    // A super wish: two steps, to tier II, doubled.
    assert_eq!(run.genie.freedom, genie::freedom_of(None, 1, true));
    let mut run = holding(1, 0);
    run.offer_wish(1, "volley", Some(&volley)).unwrap();
    run.choose(1, 0).unwrap();
    assert_eq!(run.genie.freedom, genie::freedom_of(None, 0, false));
    assert!(run.genie.last.as_ref().is_some_and(|s| s.knight == 1), "the scroll shows the wish sealed");
}

#[test]
fn freeing_the_genie_needs_a_full_gauge_and_a_super_wish() {
    let mut run = holding(1, 0);
    assert!(genie::frees("I wish the genie were free"));
    assert!(genie::frees("free the genie"));
    assert!(!genie::frees("free arrows"));
    assert!(run.offer_wish(1, genie::FREE_WORDS, None).unwrap_err().contains("gauge"), "not full");
    run.genie.freedom = genie::FREEDOM;
    assert!(run.offer_wish(1, genie::FREE_WORDS, None).unwrap_err().contains("super"), "full, but no super wish");
    run.players.get_mut(&1).unwrap().great_wishes = 1;
    run.offer_wish(1, genie::FREE_WORDS, None).unwrap();
    let offer = run.players[&1].offer.clone().unwrap();
    assert_eq!(offer.choices[0].take, genie::Take::Free);
    assert_eq!(run.floor_wishes(), overclass::FLOOR_WISHES);
    run.choose(1, 0).unwrap();
    assert!(run.genie.free());
    assert_eq!(run.players[&1].great_wishes, 0, "the super wish is spent");
    assert_eq!(run.players[&1].wishes, 1, "the plain one is kept");
    assert_eq!(run.floor_wishes(), genie::FREED_FLOOR_WISHES, "his thanks: a wish more each floor");
    assert!(run.cues.iter().any(|c| c == "genie_free"));
    assert!(run.offer_wish(1, genie::FREE_WORDS, None).is_err(), "free once");
}

#[test]
fn a_freed_genie_gives_every_knight_three_wishes_a_floor() {
    let mut run = Run::at_home(4, 2, Some("Matt"), Default::default(), Default::default());
    run.descend_for_test();
    run.genie.freed = Some((run.tick, "Matt".into()));
    let before = run.players[&1].wishes;
    run.descend_for_test();
    assert_eq!(run.players[&1].wishes, before + genie::FREED_FLOOR_WISHES);
}

#[test]
fn a_save_keeps_the_gauge_the_freed_genie_and_an_open_scroll_and_old_saves_load() {
    let volley = phrasebook::get("volley").unwrap();
    let mut run = holding(2, 0);
    run.offer_wish(1, "volley", Some(&volley)).unwrap();
    run.genie.freedom = 42;
    run.genie.freed = Some((7, "Matt".into()));
    let saved = serde_json::to_value(&run).unwrap();
    let back: Run = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(back.genie, run.genie);
    assert_eq!(back.players[&1].offer, run.players[&1].offer);
    // A save from before the genie: no gauge, no scroll, and it loads.
    let mut old = saved;
    old.as_object_mut().unwrap().remove("genie");
    for hero in old["players"].as_object_mut().unwrap().values_mut() {
        hero.as_object_mut().unwrap().remove("offer");
    }
    let back: Run = serde_json::from_value(old).unwrap();
    assert!(back.genie.quiet());
    assert!(back.players[&1].offer.is_none());
    // An untouched genie changes nothing on the wire.
    let quiet = holding(2, 0);
    assert!(!serde_json::to_string(&quiet).unwrap().contains("\"genie\""));
}

#[test]
fn a_wish_on_unknown_words_survives_the_save_and_the_grant_still_lands() {
    // A plain wish, and a super one. Both are spent at the scroll and wait
    // in the save for the host to teach them; a restart must not lose either.
    let mut freedoms = [0u32; 2];
    for (i, (plain, great)) in [(1u8, 0u8), (0, 1)].into_iter().enumerate() {
        let mut run = holding(plain, great);
        run.offer_wish(1, "a sword of pure starlight", None).unwrap();
        run.choose(1, 0).unwrap();
        assert_eq!(run.players[&1].wishes, 0);
        assert_eq!(run.players[&1].great_wishes, 0);
        assert_eq!(run.asked.len(), 1);
        assert_eq!(run.asked[0].words, "a sword of pure starlight");
        assert_eq!(run.asked[0].great, great > 0, "a super wish stays a super one");
        let saved = serde_json::to_value(&run).unwrap();
        assert!(
            saved.get("asked").is_some(),
            "the spent wish is in the save the checkpoint writes"
        );
        let mut back: Run = serde_json::from_value(saved).unwrap();
        assert_eq!(back.asked, run.asked, "a restart still has the words to teach");
        assert_eq!(back.players[&1].wishes, 0, "the wish stays spent until it lands");
        assert_eq!(back.players[&1].great_wishes, 0);
        // The host's angelX has written the words: the grant lands on the
        // restored run, sealed as the choice that was saved.
        let wish = phrasebook::get("volley").unwrap();
        let asked = back.asked.remove(0);
        let name = back.grant(asked.knight, &wish).unwrap();
        for _ in 1..asked.steps {
            if back.grant_more(asked.knight, &wish).is_none() {
                break;
            }
        }
        back.sealed(asked.knight, &wish, asked.great, asked.fills);
        assert!(
            back.players[&1].deck.iter().any(|c| c.starts_with("wish-volley-")),
            "{name}"
        );
        assert!(back.asked.is_empty(), "a landed grant leaves the queue");
        let tier = wish.next_tier(&back.players[&1].deck).map_or(wish.tiers.len(), |t| t).saturating_sub(1);
        let grade = wish.grade().map(|(g, _)| g);
        assert_eq!(
            back.genie.freedom,
            genie::freedom_of(grade, tier, asked.great),
            "a super wish is sealed as a super one"
        );
        freedoms[i] = back.genie.freedom;
    }
    assert!(freedoms[1] > freedoms[0], "the restored super wish fills the gauge more");
}

fn knight_notice(events: &[crate::wire::Event], id: u32) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            crate::wire::Event::Notice { hear: crate::wire::Hear::Knight(k), text } if *k == id => {
                Some(text.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A known spell wish the scroll cannot grant (every slot taken) spends
/// nothing, and the knight who wished hears why on their own screen.
#[test]
fn a_refused_spell_wish_tells_the_knight_why() {
    let mut run = holding(1, 0);
    let wish = phrasebook::get("firebolt").expect("firebolt");
    run.players.get_mut(&1).unwrap().spells = std::array::from_fn(|i| Some(format!("pad-{i}")));
    run.offer_wish(1, &wish.name, Some(&wish)).unwrap();
    let err = run.choose(1, 0).unwrap_err();
    assert!(err.contains("all four spell slots are full"), "{err}");
    assert_eq!(run.players[&1].wishes, 1, "a refused grant spends nothing");
    let events = crate::wire::take_notices(&mut run);
    let line = knight_notice(&events, 1);
    assert!(line.contains("stays with you"), "{line}");
    assert!(line.contains("all four spell slots are full"), "{line}");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, crate::wire::Event::Notice { hear: crate::wire::Hear::All, .. })),
        "the why is for the knight who wished, not the room"
    );
}

/// A super wish's first step lands; the next cannot (the book is full).
/// The step that did not go through comes back as a plain wish, and the
/// knight hears why.
#[test]
fn a_second_step_that_cannot_land_gives_the_wish_back_and_says_why() {
    let mut run = holding(0, 1);
    let wish = phrasebook::get("volley").expect("volley");
    run.book.cards.retain(|card| !card.id.starts_with("wish-volley"));
    run.players.get_mut(&1).unwrap().deck.retain(|id| !id.starts_with("wish-volley"));
    let sample = run.book.cards.first().cloned().expect("a card");
    let mut n = 0u32;
    while run.book.cards.len() + 1 < cards::MAX_BOOK {
        let mut card = sample.clone();
        card.id = format!("pad-{n}");
        n += 1;
        assert!(run.book.insert(card), "a pad card");
    }
    run.offer_wish(1, &wish.name, Some(&wish)).unwrap();
    let name = run.choose(1, 0).expect("the first step lands");
    assert!(!name.is_empty(), "{name}");
    assert_eq!(run.players[&1].great_wishes, 0, "the super wish was spent");
    assert_eq!(run.players[&1].wishes, 1, "the step that could not land came back");
    let events = crate::wire::take_notices(&mut run);
    let line = knight_notice(&events, 1);
    assert!(line.contains("given back"), "{line}");
    assert!(line.contains("the book is full"), "{line}");
    assert!(!events.iter().any(|event| matches!(
        event,
        crate::wire::Event::Notice { hear: crate::wire::Hear::Knight(k), .. } if *k != 1
    )));
}

#[test]
fn wish_upgrades_list_spells_first_with_the_next_tier_text() {
    let mut run = holding(3, 0);
    let spell = phrasebook::get("firebolt").unwrap();
    let hold = phrasebook::get("volley").unwrap();
    run.grant(1, &hold).unwrap();
    run.grant(1, &spell).unwrap();
    let list = run.upgrades(1);
    assert_eq!(list.len(), 2, "kit spells have no wish ladder");
    for (up, wish) in list.iter().zip([&spell, &hold]) {
        assert_eq!((&up.name, up.tier, up.next), (&wish.name, 0, Some(1)));
        assert_eq!(up.tiers(), "I → II");
        assert_eq!(up.says, wish.tiers[1].1);
        assert!(up.available);
        assert_eq!(up.words(), format!("upgrade {}", wish.name));
    }
    assert!(list[0].spell && !list[1].spell);
}

#[test]
fn picking_a_wish_upgrade_offers_then_grants_one_next_tier_for_one_wish() {
    for id in ["firebolt", "volley"] {
        let mut run = holding(2, 0);
        let wish = phrasebook::get(id).unwrap();
        run.grant(1, &wish).unwrap();
        let spell_slot = run.players[&1].spells.iter().position(|s| s.as_ref() == Some(&wish.card_id(0)));
        if let Some(slot) = spell_slot {
            run.players.get_mut(&1).unwrap().spell_cooldowns[slot] = 17;
        }
        let words = run.upgrades(1)[0].words();
        let found = phrasebook::find_for(&words, &run.players[&1].deck).unwrap();
        run.offer_wish(1, &words, Some(&found)).unwrap();
        assert_eq!(run.players[&1].offer.as_ref().unwrap().choices[0].gives, format!("{} II", wish.name));
        assert_eq!(run.players[&1].wishes, 2, "opening an offer spends nothing");
        run.choose(1, 0).unwrap();
        let hero = &run.players[&1];
        assert_eq!(hero.wishes, 1);
        assert!(hero.deck.contains(&wish.card_id(1)));
        assert!(!hero.deck.contains(&wish.card_id(0)));
        assert!(!hero.deck.contains(&wish.card_id(2)));
        if let Some(slot) = spell_slot {
            assert_eq!(hero.spells[slot].as_ref(), Some(&wish.card_id(1)));
            assert_eq!(hero.spell_cooldowns[slot], 17);
        }
    }
}

#[test]
fn wish_upgrades_disable_full_ladders_and_every_pick_without_a_wish() {
    let mut run = holding(1, 0);
    // Deep enough for a ladder's fifth (`overclass::top_tier`).
    run.dungeon.depth = crate::rules::FLOORS;
    let wish = phrasebook::get("firebolt").unwrap();
    for _ in &wish.tiers {
        run.grant(1, &wish).unwrap();
    }
    let list = run.upgrades(1);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].next, None);
    assert!(!list[0].available);
    assert!(list[0].says.is_empty());
    run.grant(1, &phrasebook::get("volley").unwrap()).unwrap();
    run.players.get_mut(&1).unwrap().wishes = 0;
    let list = run.upgrades(1);
    assert!(list.iter().all(|u| !u.available));
    assert_eq!(run.upgrade_note(1, &list), Some(genie::NO_WISHES));
    let empty = holding(1, 0);
    assert!(empty.upgrades(1).is_empty());
    assert_eq!(empty.upgrade_note(1, &[]), Some(genie::NOTHING_HELD));
}

#[test]
fn learned_wish_upgrade_lines_survive_a_save_without_a_local_phrasebook_entry() {
    let mut run = holding(1, 0);
    let mut wish = phrasebook::read("wish moon-thread | Moon Thread\nkind spell\nsay moon thread\ntier cast bolt; cooldown 6 :: A silver bolt.\ntier cast bolt; cooldown 4 :: A brighter silver bolt.\n").remove(0);
    wish.learned = true;
    run.grant(1, &wish).unwrap();
    let saved = serde_json::to_value(&run).unwrap();
    let restored: Run = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(restored.upgrades(1), run.upgrades(1));
    let up = &restored.upgrades(1)[0];
    assert_eq!((&up.name, up.says.as_str(), up.next), (&wish.name, wish.tiers[1].1.as_str(), Some(1)));
    assert!(up.spell && up.available);
    let mut old = saved;
    old.as_object_mut().unwrap().remove("ladders");
    let mut restored: Run = serde_json::from_value(old).expect("old saves still load");
    restored.remember_ladders(&[wish]);
    assert_eq!(restored.upgrades(1), run.upgrades(1), "the host restores missing ladder text");
}

/// The numeral a choice's line says it grants for `name`: "Ricochet IV",
/// "Ricochet V + 30 gold", "Ricochet III, and keep a wish".
fn numeral_in(gives: &str, name: &str) -> usize {
    let after = gives.strip_prefix(name).unwrap_or_else(|| panic!("{gives} is {name}")).trim_start();
    let numeral: String = after.chars().take_while(|c| matches!(c, 'I' | 'V')).collect();
    ["I", "II", "III", "IV", "V"].iter().position(|n| *n == numeral).unwrap_or_else(|| panic!("{gives}"))
}

/// The tier (from 0) knight 1 holds of `wish`, if any.
fn held_tier(run: &Run, wish: &phrasebook::Wish) -> Option<usize> {
    (0..wish.tiers.len()).rev().find(|&t| run.players[&1].deck.contains(&wish.card_id(t)))
}

/// The scroll offers what the grant gives, on every floor: each choice's
/// line names the tier its pick lands, a ladder's fourth and fifth only
/// deep enough down (`overclass::top_tier`), and Bolder and a super wish
/// reach the fifth there. A ladder at its floor's top is not offered.
#[test]
fn the_scroll_offers_what_the_grant_gives_up_to_the_fifth_on_every_floor() {
    let heavy = phrasebook::get("heavy").unwrap();
    let mut reached_five = std::collections::BTreeSet::new();
    for depth in [1, 2, crate::rules::FLOORS] {
        let top = overclass::top_tier(depth);
        for level in 0..heavy.tiers.len() {
            for great in [0, 1] {
                for seed in 0..6u64 {
                    let mut run = Run::new(seed, 2, Some("Matt"));
                    run.dungeon.depth = depth;
                    let hero = run.players.get_mut(&1).unwrap();
                    (hero.wishes, hero.great_wishes) = (1, great);
                    hero.deck.extend((0..level).map(|t| heavy.card_id(t)).last());
                    let offered = run.offer_wish(1, &heavy.name, Some(&heavy));
                    if level > top {
                        let why = offered.unwrap_err();
                        assert!(why.contains("climbs no higher here") && why.contains(&format!("floor {}", overclass::tier_floor(level).unwrap())), "{why}");
                        assert!(run.locked(1, &heavy, level).is_some(), "the grant refuses it too");
                        continue;
                    }
                    let offer = run.players[&1].offer.clone().unwrap();
                    for (n, choice) in offer.choices.iter().enumerate() {
                        let id = choice.wish.clone().unwrap();
                        let wish = phrasebook::get(&id).unwrap();
                        let said = numeral_in(&choice.gives, &wish.name);
                        assert!(said <= top, "floor {depth}: {} offers past the floor's top", choice.gives);
                        if choice.take == genie::Take::Bolder {
                            reached_five.extend((said == 4).then_some("bolder"));
                        }
                        if choice.take == genie::Take::Said && great == 1 {
                            reached_five.extend((said == 4).then_some("super"));
                        }
                        let mut picked = run.clone();
                        picked.choose(1, n).unwrap_or_else(|why| panic!("floor {depth} {}: {why}", choice.gives));
                        assert_eq!(held_tier(&picked, &wish), Some(said), "floor {depth}, {:?}: {}", choice.take, choice.gives);
                    }
                }
            }
        }
    }
    assert_eq!(reached_five.len(), 2, "Bolder and a super wish both reach V: {reached_five:?}");
}

/// A super wish one step from its floor's top lands that step and gives a
/// plain wish back, as at a ladder's top; on the floor below the fifth it
/// waits, and Dragon Keep's floor gives it.
#[test]
fn a_super_wish_at_its_floors_top_keeps_a_plain_wish_and_the_fifth_waits_for_the_deep() {
    let ricochet = phrasebook::get("ricochet").unwrap();
    let mut run = holding(0, 1);
    run.players.get_mut(&1).unwrap().deck.push(ricochet.card_id(1));
    run.offer_wish(1, &ricochet.name, Some(&ricochet)).unwrap();
    let offer = run.players[&1].offer.clone().unwrap();
    assert_eq!(offer.choices[0].gives, "Ricochet III", "the first floor's top");
    assert!(offer.choices.iter().all(|c| c.take != genie::Take::Bolder), "no step past it to be bolder with");
    run.choose(1, 0).unwrap();
    assert_eq!(held_tier(&run, &ricochet), Some(2));
    assert_eq!((run.players[&1].wishes, run.players[&1].great_wishes), (1, 0), "the second step comes back as a plain wish");
    let err = run.offer_wish(1, &ricochet.name, Some(&ricochet)).unwrap_err();
    assert_eq!(err, "Ricochet climbs no higher here: its tier IV waits for floor 2.");
    run.dungeon.depth = 2;
    run.offer_wish(1, &ricochet.name, Some(&ricochet)).unwrap();
    assert_eq!(run.players[&1].offer.as_ref().unwrap().choices[0].gives, "Ricochet IV");
    run.choose(1, 0).unwrap();
    run.players.get_mut(&1).unwrap().wishes = 1;
    assert!(run.offer_wish(1, &ricochet.name, Some(&ricochet)).unwrap_err().contains("waits for floor 3"));
    run.dungeon.depth = crate::rules::FLOORS;
    run.offer_wish(1, &ricochet.name, Some(&ricochet)).unwrap();
    run.choose(1, 0).unwrap();
    assert_eq!(held_tier(&run, &ricochet), Some(4));
    let top = run.book.get(&ricochet.card_id(4)).unwrap();
    assert_eq!((top.name.as_str(), top.rarity), ("Ricochet V", cards::Rarity::Relic));
    assert!(run.genie.last.as_ref().is_some_and(|s| s.cheer && s.line.ends_with("Ricochet V")), "the ladder's top: he cheers");
}

/// The gauge counts the tier a wish reached, the fifth the most, and never
/// past its whole.
#[test]
fn the_freedom_gauge_counts_the_fifth_and_stays_whole() {
    use artistry::Grade;
    assert_eq!(genie::freedom_of(None, 4, false), 10);
    assert_eq!(genie::freedom_of(Some(Grade::Legendary), 4, true), 40, "a Legendary super wish to V");
    assert!((0..5).all(|t| genie::freedom_of(None, t + 1, false) > genie::freedom_of(None, t, false)));
    let ricochet = phrasebook::get("ricochet").unwrap();
    let mut run = holding(0, 0);
    run.dungeon.depth = crate::rules::FLOORS;
    for _ in 0..4 {
        for t in 0..ricochet.tiers.len() {
            run.players.get_mut(&1).unwrap().deck.retain(|c| !c.starts_with("wish-ricochet-"));
            if t > 0 {
                run.players.get_mut(&1).unwrap().deck.push(ricochet.card_id(t - 1));
            }
            run.grant(1, &ricochet).unwrap();
            run.sealed(1, &ricochet, true, true);
            assert!(run.genie.freedom <= genie::FREEDOM);
        }
    }
    assert!(run.genie.ready(), "a deep delve's great wishes fill it");
}

/// An old save's learned wish, written to three tiers and held at its
/// third with the scroll open on it, loads, keeps its open offer as it was
/// captured, and climbs on to the fifth from the same phrasebook text.
#[test]
fn an_old_saves_three_tier_learned_wish_loads_and_climbs_to_the_fifth() {
    let text = format!(
        "{}\nwish bigger-arrows | Bigger Arrows\nsay bigger arrows | big arrows\n\
tier size 1 :: Your arrows are bigger.\ntier size 2 :: Bigger again.\ntier size 2; knock 1 :: Huge arrows that shove.\n\
draw shot\n.2.......jW.\n22ooooooojWW\n.2.......jW.\n",
        phrasebook::LEARNED
    );
    let climbed = phrasebook::read(&text).remove(0);
    assert_eq!(climbed.tiers.len(), 5);
    // The ladder as the save knew it: three tiers, captured whole.
    let old = climbed.written();
    let mut run = holding(3, 0);
    run.dungeon.depth = crate::rules::FLOORS;
    run.grant(1, &old).unwrap();
    run.grant(1, &old).unwrap();
    run.offer_wish(1, &old.name, Some(&old)).unwrap();
    assert_eq!(run.ladders["bigger-arrows"].1.len(), 3);
    let saved = serde_json::to_value(&run).unwrap();
    // Loaded where the workspace's phrasebook has the same three tiers.
    let _book = phrasebook::scoped(phrasebook::prepare_scoped(text.clone(), Vec::new()));
    let mut back: Run = serde_json::from_value(saved).expect("old saves still load");
    crate::rules::scrolls::validate_run(&back).expect("its open scroll is still its own");
    back.choose(1, 0).unwrap();
    assert_eq!(held_tier(&back, &climbed), Some(2), "the open scroll grants what it offered");
    // The host refreshes the ladder text it carries for every seat.
    back.remember_ladders(&phrasebook::book());
    assert_eq!(back.ladders["bigger-arrows"].1.len(), 5);
    let up = back.upgrades(1).into_iter().find(|u| u.name == "Bigger Arrows").unwrap();
    assert_eq!((up.tier, up.next, up.tiers()), (2, Some(3), "III → IV".to_string()));
    assert!(up.available && up.says.starts_with("Mighty: "), "{}", up.says);
    for (tier, numeral) in [(3, "IV"), (4, "V")] {
        let words = back.upgrades(1).into_iter().find(|u| u.name == "Bigger Arrows").unwrap().words();
        let found = phrasebook::find_for(&words, &back.players[&1].deck).unwrap();
        assert_eq!(found, climbed, "the upgrade finds the climbed ladder");
        back.offer_wish(1, &words, Some(&found)).unwrap();
        assert_eq!(back.players[&1].offer.as_ref().unwrap().choices[0].gives, format!("Bigger Arrows {numeral}"));
        back.choose(1, 0).unwrap();
        assert_eq!(held_tier(&back, &climbed), Some(tier));
    }
    let top = back.book.get(&climbed.card_id(4)).unwrap();
    assert!(top.effects.iter().any(|e| matches!(e, cards::Effect::Knock(_))));
    assert!(back.upgrades(1).iter().any(|u| u.name == "Bigger Arrows" && u.next.is_none()));
}

/// A fairy wish spent through the scroll on a new wish lands three steps
/// up, at its third tier of five, Legendary; the fourth and fifth are
/// still to climb.
#[test]
fn a_fairy_wish_through_the_scroll_lands_at_the_third_of_five_legendary() {
    let breastplate = phrasebook::get("breastplate").unwrap();
    let mut run = holding(1, 0);
    run.dungeon.depth = nooks::gnome::DEPTH;
    run.players.get_mut(&1).unwrap().fairy_wishes = 1;
    run.offer_wish(1, &breastplate.name, Some(&breastplate)).unwrap();
    run.choose(1, 0).unwrap();
    assert_eq!(held_tier(&run, &breastplate), Some(2));
    let legendary = breastplate.card_graded(2, artistry::Grade::Legendary).unwrap();
    assert_eq!(run.book.get(&breastplate.card_id(2)).unwrap().effects, legendary.effects);
    assert_eq!((run.players[&1].wishes, run.players[&1].fairy_wishes), (1, 0));
    assert!(run.genie.last.as_ref().is_some_and(|s| s.line.starts_with("Legendary Breastplate III")));
    assert_eq!(run.upgrades(1).iter().find(|u| u.name == "Breastplate").and_then(|u| u.next), Some(3));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_wish_tier_shots -- --ignored`:
/// the scroll deep enough down for the fifth (As you said IV, Bolder V),
/// its seal at the top, five knights in a row wearing a Great Helm and a
/// Breastplate at tiers I to V with a Firebolt V on the bar, and the card
/// pictures of three ladders at every tier, the fourth and fifth framed.
#[test]
#[ignore]
fn write_wish_tier_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |img: &crate::render::overworld::ink::Img, name: &str| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let frame = |run: &Run| {
        let mut img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        crate::render::genie::paint(&mut img, run, Some(1));
        img
    };
    let ricochet = phrasebook::get("ricochet").unwrap();
    // The scroll, on Dragon Keep's floor, a knight at Ricochet III.
    let mut bolder = (0..32u64)
        .find_map(|seed| {
            let mut run = Run::new(seed, 2, Some("Matt"));
            run.dungeon.depth = crate::rules::FLOORS;
            run.calm_for_test();
            run.enemies.clear();
            let hero = run.players.get_mut(&1).unwrap();
            (hero.wishes, hero.great_wishes) = (2, 0);
            for _ in 0..3 {
                run.grant(1, &ricochet).unwrap();
            }
            run.offer_wish(1, &ricochet.name, Some(&ricochet)).unwrap();
            let offer = run.players[&1].offer.as_ref().unwrap();
            offer.choices.iter().any(|c| c.take == genie::Take::Bolder).then_some(run)
        })
        .expect("a seed whose genie offers Bolder");
    for _ in 0..4 {
        bolder.step(&BTreeMap::new());
    }
    save(&frame(&bolder), "a-scroll-iv-or-bolder-v");
    let mut sealed = bolder.clone();
    let n = sealed.players[&1].offer.as_ref().unwrap().choices.iter().position(|c| c.take == genie::Take::Bolder).unwrap();
    sealed.choose(1, n).unwrap();
    sealed.step(&BTreeMap::new());
    save(&frame(&sealed), "b-sealed-ricochet-v");
    // Five knights, tiers I to V of the same two pieces, and a Firebolt V.
    let mut run = Run::new(7, 3, Some("One"));
    run.dungeon.depth = crate::rules::FLOORS;
    for id in 2..=5 {
        run.join_seat(id, &format!("Tier {id}"));
    }
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run.window = None;
    let helm = phrasebook::get("great-helm").unwrap();
    let plate = phrasebook::get("breastplate").unwrap();
    let firebolt = phrasebook::get("firebolt").unwrap();
    for id in 1..=5u32 {
        for _ in 0..id {
            run.grant(id, &helm).unwrap();
            run.grant(id, &plate).unwrap();
        }
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (14.0 + 5.0 * (id - 1) as f32, 14.0);
        hero.invulnerable = 1_000_000;
    }
    for _ in 0..firebolt.tiers.len() {
        run.grant(1, &firebolt).unwrap();
    }
    for _ in 0..8 {
        run.step(&BTreeMap::new());
        for (id, hero) in run.players.iter_mut() {
            (hero.x, hero.y) = (14.0 + 5.0 * (*id - 1) as f32, 14.0);
        }
    }
    save(&frame(&run), "c-knights-tier-i-to-v");
    // The cards' pictures, I to V, four times their size.
    let ladders = ["ricochet", "firebolt", "great-helm"];
    let (cell, k) = (22, 4);
    let mut sheet = crate::render::overworld::ink::Img::new(5 * cell * k, ladders.len() as i32 * cell * k);
    sheet.rect(0, 0, sheet.w, sheet.h, 'K');
    for (row, id) in ladders.iter().enumerate() {
        let wish = phrasebook::get(id).unwrap();
        for tier in 0..wish.tiers.len() {
            let art = crate::render::overworld::arena::card_art(&wish.card(tier).unwrap());
            let (x, y) = (tier as i32 * cell * k + (cell - art.w) * k / 2, row as i32 * cell * k + (cell - art.h) * k / 2);
            sheet.stamp_times(&art, x, y, k);
        }
    }
    save(&sheet, "d-card-art-i-to-v");
}
