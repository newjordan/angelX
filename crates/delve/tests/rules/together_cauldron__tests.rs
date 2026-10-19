use super::*;
use crate::rules::command::DelveCmd;

/// A party of two on floor `depth`, its guardian's room entered and won.
fn won_floor(seed: u64, depth: u32) -> Run {
    let mut run = Run::new(seed, 2, Some("Matt"));
    run.join_seat(2, "Wren");
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    run.boss_gates = None;
    let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    run.enter_for_test(room);
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    run.clear_for_test();
    risen(&mut run);
    run
}

/// Steps until the pot has climbed out of the floor.
fn risen(run: &mut Run) {
    while run
        .cauldron_here()
        .is_some_and(|p| run.tick < p.rose + u64::from(cauldron::RISE_TICKS))
    {
        run.step_sim_for_test(&BTreeMap::new());
    }
}

/// Knight `id` learns these spells, in order.
fn learn(run: &mut Run, id: u32, spells: &[&str]) {
    for spell in spells {
        let card = run.book.get(spell).unwrap().clone();
        assert!(run.players.get_mut(&id).unwrap().equip_spell(&card));
    }
}

/// Knight `id` a step south of the pot.
fn step_up(run: &mut Run, id: u32) {
    let pot = run.cauldron_here().unwrap().clone();
    let hero = run.players.get_mut(&id).unwrap();
    (hero.x, hero.y) = (pot.x - 0.5 + id as f32 * 0.5, pot.y + 2.0);
}

/// One press and its let-go, for knight `id`, as a tick's keys.
fn press(run: &mut Run, id: u32, input: Input) {
    run.step_sim_for_test(&BTreeMap::from([(id, input)]));
    run.step_sim_for_test(&BTreeMap::new());
}

fn interact() -> Input {
    Input { interact: true, take: true, ..Input::default() }
}

fn cast(slot: u8) -> Input {
    Input { cast: slot, ..Input::default() }
}

/// Knight `id` throws spell slots `a` and `b` (1–4) into the pot.
fn brew(run: &mut Run, id: u32, a: u8, b: u8) {
    press(run, id, interact());
    press(run, id, cast(a));
    press(run, id, cast(b));
    press(run, id, interact());
}

fn boil(run: &mut Run) {
    for _ in 0..cauldron::PUFF + 4 {
        run.step_sim_for_test(&BTreeMap::new());
    }
}

/// The seed whose brew of these two cards for knight `id` passes `want`,
/// laid on the run.
fn seed_for(run: &mut Run, id: u32, a: &str, b: &str, want: impl Fn(&cauldron::Outcome) -> bool) {
    let (ca, cb) = (run.book.get(a).unwrap().clone(), run.book.get(b).unwrap().clone());
    let seed = (1..10_000u64)
        .find(|&s| want(&cauldron::decide(s, run.dungeon.depth, id, &ca, &cb).0))
        .expect("some seed brews it");
    run.seed = seed;
}

fn is_spell(o: &cauldron::Outcome) -> bool {
    matches!(o, cauldron::Outcome::Spell { .. })
}

#[test]
fn the_cauldron_rises_by_the_stairs_once_the_guardian_falls() {
    let mut run = Run::new(5, 2, Some("Matt"));
    run.descend_for_test();
    run.boss_gates = None;
    let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    run.enter_for_test(room);
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    run.step(&BTreeMap::new());
    assert!(run.cauldron_here().is_none(), "no pot while the guardian stands");
    run.clear_for_test();
    let pot = run.cauldron_here().expect("the pot rises when the guardian falls").clone();
    let room = run.room();
    let (col, row) = ((pot.x / TILE_UNITS) as i32, (pot.y / TILE_UNITS) as i32);
    assert_eq!(room.tile(col, row), Tile::Floor, "it stands on the floor");
    let stairs = (0..room.rows as i32)
        .flat_map(|r| (0..room.cols as i32).map(move |c| (c, r)))
        .filter(|&(c, r)| room.tile(c, r) == Tile::Stairs)
        .map(|(c, r)| ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS))
        .fold(f32::MAX, |d, (x, y)| d.min((x - pot.x).hypot(y - pot.y)));
    assert!(stairs < 16.0, "beside the stairs ({stairs} away)");
    // The next floor's pot is its own; this one is left behind.
    run.descend_for_test();
    assert!(run.cauldron_here().is_none());
}

#[test]
fn the_first_floors_sanctuary_keeps_its_pot_clear_of_the_altar() {
    let run = won_floor(11, 1);
    assert_eq!(run.room().kind, RoomKind::Sanctuary);
    let pot = run.cauldron_here().expect("the pot rises in the Sanctuary too");
    let (ax, ay) = run.room().altar();
    assert!((pot.x - ax).hypot(pot.y - ay) >= 6.0, "clear of Tobbin's anvil");
}

#[test]
fn the_cauldron_rises_in_an_open_floors_guardian_chamber() {
    let mut run = Run::new(9, 7, None);
    run.join_seat(2, "Wren");
    run.open_floor_for_test(3);
    let open = run.dungeon.open.clone().unwrap();
    let g = usize::from(open.guardian);
    let (cx, cy) = open.chambers[g].centre();
    let inside = run.open_spots(open.map, (cx, cy + 4.0), 1)[0];
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = inside;
        hero.invulnerable = 100_000;
    }
    run.step(&BTreeMap::new());
    assert!(run.cauldron_here().is_none());
    for e in run.enemies.iter_mut().filter(|e| e.home == Some(g as u8)) {
        e.hp = 0;
    }
    run.step(&BTreeMap::new());
    let pot = run.cauldron_here().expect("the pot rises in the guardian's chamber");
    let ch = &run.dungeon.open.as_ref().unwrap().chambers[g];
    assert!(ch.holds(pot.x, pot.y, 0.0), "inside the chamber");
}

#[test]
fn two_spells_go_in_and_one_new_spell_comes_out() {
    let mut run = won_floor(5, 2);
    learn(&mut run, 1, &["cabbage-storm", "hedge-maze", "the-imposture"]);
    seed_for(&mut run, 1, "cabbage-storm", "hedge-maze", is_spell);
    step_up(&mut run, 1);
    assert_eq!(run.cauldron_ask(1), Some(cauldron::Ask::Brew));
    press(&mut run, 1, interact());
    assert_eq!(run.cauldron_ask(1), Some(cauldron::Ask::Pick(0)));
    press(&mut run, 1, cast(1));
    press(&mut run, 1, cast(2));
    assert_eq!(run.cauldron_ask(1), Some(cauldron::Ask::Pick(2)));
    press(&mut run, 1, interact());
    let hero = &run.players[&1];
    assert_eq!(hero.spells[..2], [None, None], "both cards are in the pot");
    boil(&mut run);
    let hero = &run.players[&1];
    let made = hero.spells[0].clone().expect("the new spell takes the first slot picked");
    assert!(made.starts_with("brew-kitchen-garden"), "garden and garden make a Kitchen Garden: {made}");
    assert_eq!(hero.spells[1], None, "the second slot is free for the next find");
    assert_eq!(hero.spells[2].as_deref(), Some("the-imposture"), "the spell left out stays");
    let card = run.book.get(&made).expect("the brew is in the run's book");
    assert_eq!(card.kind, cards::Kind::Spell);
    // It casts like any spell.
    run.step_sim_for_test(&BTreeMap::from([(1, cast(1))]));
    assert!(run.players[&1].spell_cooldowns[0] > 0, "cast, and recharging");
}

#[test]
fn the_same_seed_brews_the_same_spell_whichever_card_goes_first() {
    let a = won_floor(21, 2);
    let (storm, hedge) = (a.book.get("cabbage-storm").unwrap(), a.book.get("hedge-maze").unwrap());
    for seed in 0..64 {
        let one = cauldron::decide(seed, 2, 1, storm, hedge).0;
        assert_eq!(one, cauldron::decide(seed, 2, 1, storm, hedge).0);
        assert_eq!(one, cauldron::decide(seed, 2, 1, hedge, storm).0, "either order");
    }
    // Two runs from one seed, the same keys: the same brew.
    let outcome = |picks: (u8, u8)| {
        let mut run = won_floor(21, 2);
        learn(&mut run, 1, &["cabbage-storm", "the-imposture"]);
        step_up(&mut run, 1);
        brew(&mut run, 1, picks.0, picks.1);
        boil(&mut run);
        (run.cauldron_here().unwrap().brew(1).unwrap().outcome.clone(), run.players[&1].spells.clone())
    };
    assert_eq!(outcome((1, 2)), outcome((1, 2)));
    assert_eq!(outcome((1, 2)).0, outcome((2, 1)).0);
}

#[test]
fn a_knight_brews_once_a_floor_and_a_friend_still_may() {
    let mut run = won_floor(5, 2);
    learn(&mut run, 1, &["cabbage-storm", "hedge-maze"]);
    learn(&mut run, 2, &["cabbage-storm", "the-imposture"]);
    seed_for(&mut run, 1, "cabbage-storm", "hedge-maze", is_spell);
    step_up(&mut run, 1);
    brew(&mut run, 1, 1, 2);
    boil(&mut run);
    // A second spell found; the pot will not take a second brew.
    learn(&mut run, 1, &["the-imposture"]);
    assert_eq!(run.cauldron_ask(1), Some(cauldron::Ask::Brewed));
    let before = run.players[&1].spells.clone();
    press(&mut run, 1, interact());
    press(&mut run, 1, cast(1));
    press(&mut run, 1, cast(2));
    press(&mut run, 1, interact());
    boil(&mut run);
    assert_eq!(run.players[&1].spells, before, "once a knight a floor");
    assert!(run.cauldron_here().unwrap().stir(1).is_none());
    // Wren has not brewed.
    step_up(&mut run, 2);
    assert_eq!(run.cauldron_ask(2), Some(cauldron::Ask::Brew));
    brew(&mut run, 2, 1, 2);
    assert!(run.cauldron_here().unwrap().brewed(2));
    assert_eq!(run.cauldron_here().unwrap().brews.len(), 2);
}

#[test]
fn a_knight_with_one_spell_is_told_so_and_cannot_brew() {
    let mut run = won_floor(5, 2);
    learn(&mut run, 1, &["cabbage-storm"]);
    step_up(&mut run, 1);
    assert_eq!(run.cauldron_ask(1), Some(cauldron::Ask::Short));
    press(&mut run, 1, interact());
    assert!(run.cauldron_here().unwrap().stir(1).is_none());
}

#[test]
fn spell_keys_at_the_pot_pick_rather_than_cast_and_walking_off_lets_go() {
    let mut run = won_floor(5, 2);
    learn(&mut run, 1, &["cabbage-storm", "hedge-maze"]);
    step_up(&mut run, 1);
    press(&mut run, 1, interact());
    press(&mut run, 1, cast(1));
    assert_eq!(run.players[&1].spell_cooldowns[0], 0, "picked, not cast");
    assert!(run.strikes.is_empty(), "no cabbages fell");
    assert_eq!(run.cauldron_here().unwrap().stir(1).unwrap().picks, vec![0]);
    // Pressing it again puts it back.
    press(&mut run, 1, cast(1));
    assert!(run.cauldron_here().unwrap().stir(1).unwrap().picks.is_empty());
    let pot = run.cauldron_here().unwrap().clone();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (pot.x, pot.y + cauldron::LET_GO + 3.0);
    run.step_sim_for_test(&BTreeMap::new());
    assert!(run.cauldron_here().unwrap().stir(1).is_none(), "walked away: the pot lets go");
    assert!(!run.cauldron_here().unwrap().brewed(1), "and nothing was spent");
}

#[test]
fn a_misfire_is_silly_harmless_and_hands_both_cards_back() {
    for misfire in cauldron::Misfire::ALL {
        let mut run = won_floor(5, 2);
        learn(&mut run, 1, &["cabbage-storm", "hedge-maze"]);
        seed_for(&mut run, 1, "cabbage-storm", "hedge-maze", |o| *o == cauldron::Outcome::Misfire(misfire));
        step_up(&mut run, 1);
        let hp = run.players[&1].hp;
        brew(&mut run, 1, 2, 1);
        boil(&mut run);
        let hero = &run.players[&1];
        assert_eq!(hero.spells[0].as_deref(), Some("cabbage-storm"), "{misfire:?}: back where it was");
        assert_eq!(hero.spells[1].as_deref(), Some("hedge-maze"));
        assert_eq!(hero.hp, hp, "{misfire:?} hurts nobody");
        assert_eq!(hero.frog(), misfire == cauldron::Misfire::Frog);
        assert!(run.cauldron_here().unwrap().brewed(1), "and the brew is spent");
    }
}

#[test]
fn a_guest_seats_keys_brew_across_the_wire() {
    let mut run = won_floor(5, 2);
    learn(&mut run, 2, &["cabbage-storm", "hedge-maze"]);
    seed_for(&mut run, 2, "cabbage-storm", "hedge-maze", is_spell);
    step_up(&mut run, 2);
    // Each press as a browser seat sends it: a command's bytes, read back
    // on the host.
    let wire = |input: Input| {
        let mut bytes = Vec::new();
        DelveCmd { held: input, pressed: input }.encode(&mut bytes);
        DelveCmd::decode(&bytes).expect("the command reads").input()
    };
    for input in [interact(), cast(1), cast(2), interact()] {
        run.step_sim_for_test(&BTreeMap::from([(2, wire(input))]));
        run.step_sim_for_test(&BTreeMap::from([(2, wire(Input::default()))]));
    }
    boil(&mut run);
    let made = run.players[&2].spells[0].clone().unwrap();
    assert!(made.starts_with("brew-"), "the guest's brew: {made}");
    assert!(run.cauldron_here().unwrap().brewed(2) && !run.cauldron_here().unwrap().brewed(1));
    // And the guest's own screen is sent the pot.
    let tree = crate::wire::publish(&run).unwrap();
    let angel_net::doc::value::Value::Struct(fields) = tree else { panic!("a struct") };
    assert!(fields.iter().any(|(name, _)| name == "cauldron"), "the mirror carries the pot");
}

#[test]
fn an_old_save_loads_without_a_cauldron_and_a_brewing_one_round_trips() {
    let run = Run::new(8, 4, Some("Friend"));
    let mut save = serde_json::to_value(&run).unwrap();
    save.as_object_mut().unwrap().remove("cauldron");
    let back: Run = serde_json::from_value(save).unwrap();
    assert!(back.cauldron.is_none());
    // A pot saved by its first build: no brews, stirs or keys.
    let pot: cauldron::Cauldron =
        serde_json::from_str(r#"{"raid":1,"depth":2,"room":3,"x":4.0,"y":5.0,"rose":6}"#).unwrap();
    assert!(pot.brewed.is_empty() && pot.brews.is_empty());
    // Mid-brew, it saves and loads as it was.
    let mut run = won_floor(5, 2);
    learn(&mut run, 1, &["cabbage-storm", "hedge-maze"]);
    step_up(&mut run, 1);
    brew(&mut run, 1, 1, 2);
    let again: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert_eq!(again.cauldron, run.cauldron);
}

#[test]
fn every_brew_passes_the_card_checker_and_stays_within_two_spells_of_power() {
    let book = cards::Book::builtin();
    let storm = cauldron::power(book.get("cabbage-storm").unwrap());
    assert!(cauldron::POWER_BUDGET <= 2.05 * storm, "the budget is two Cabbage Storms");
    for recipe in &cauldron::RECIPES {
        for tier in cauldron::Tier::ALL {
            let card = cauldron::brewed_card(recipe, tier);
            assert_eq!(card.kind, cards::Kind::Spell);
            assert!(card.name.len() <= cards::MAX_NAME, "{} fits a card", card.name);
            assert_eq!((card.drop, card.chest), (0, 0), "a brew never drops");
            let checked = cards::check(&card.id, &cauldron::card_text(recipe, tier)).unwrap();
            assert!(checked.notes.is_empty(), "{}: nothing clamped: {:?}", card.name, checked.notes);
            let p = cauldron::power(&card);
            assert!(p <= cauldron::POWER_BUDGET, "{} is {p:.1} a second", card.name);
        }
    }
    // Every pair of essences has its brew, and fire with frost is steam.
    for a in cauldron::Essence::ALL {
        for b in cauldron::Essence::ALL {
            assert_eq!(cauldron::recipe(a, b).pair, cauldron::recipe(b, a).pair);
        }
    }
    assert_eq!(cauldron::recipe(cauldron::Essence::Frost, cauldron::Essence::Fire).name, "Steam Burst");
}

#[test]
fn a_spells_essence_reads_its_words_then_its_look_then_what_it_does() {
    let book = cards::Book::builtin();
    use cauldron::Essence;
    assert_eq!(Essence::of(book.get("cabbage-storm").unwrap()), Essence::Garden);
    assert_eq!(Essence::of(book.get("hedge-maze").unwrap()), Essence::Garden);
    assert_eq!(Essence::of(book.get("the-imposture").unwrap()), Essence::Trick);
    let ember = cards::check("ember-lance", "name Ember Lance\nkind spell\ncooldown 6\ncast bolt\n").unwrap().card;
    assert_eq!(Essence::of(&ember), Essence::Fire);
    let rime = cards::check("pale-ring", "name Pale Ring\nkind spell\ncooldown 6\ncast ring\nlook ice orb\n").unwrap().card;
    assert_eq!(Essence::of(&rime), Essence::Frost);
    let mend = cards::check("mend", "name Mend\nkind spell\ncooldown 9\nheal 20\n").unwrap().card;
    assert_eq!(Essence::of(&mend), Essence::Life);
    // Common brews most often, misfires now and then, perfect ones rarely.
    let mut tally = [0u32; 4];
    for seed in 0..2000 {
        match cauldron::decide(seed, 3, 1, &ember, &rime).0 {
            cauldron::Outcome::Misfire(_) => tally[0] += 1,
            cauldron::Outcome::Spell { tier, name, .. } => {
                assert!(name.ends_with("Steam Burst"), "fire and frost: {name}");
                tally[1 + tier as usize] += 1;
            }
        }
    }
    assert!(tally[1] > tally[2] && tally[2] > tally[3] && tally[3] > 0 && tally[0] > 0, "{tally:?}");
}
