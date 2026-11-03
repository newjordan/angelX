//! A summoner wishes for his dead, for himself, or for both: 4 on his open
//! scroll turns it, his words set where it starts, a wish for his dead
//! puts its numbers and its look on them and none on him, a wish for both
//! halves on each, a save and a guest's copy keep whom each wish was for,
//! and a knight who keeps no dead sees nothing new.
use super::*;
use crate::rules::hireling::testing::{fight_with, foe_at};
use crate::rules::minions::{GRAVECALLER, RISE};

/// The Gravecaller in an open first-floor fight room, at (12, 12), aiming
/// east, every spell ready, three wishes held.
fn gravecaller() -> Run {
    let mut run = fight_with(false, &[]);
    assert!(run.take_up(1, GRAVECALLER));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.aim_x, hero.aim_y) = (12.0, 12.0, 1.0, 0.0);
    hero.spell_cooldowns = [0; 4];
    (hero.wishes, hero.great_wishes) = (3, 0);
    run
}

fn press(run: &mut Run, cast: u8) {
    run.step(&BTreeMap::from([(1, Input { cast, ..Default::default() })]));
}

fn target(run: &Run) -> Option<Target> {
    run.players[&1].offer.as_ref().and_then(|o| o.target)
}

/// Raise a minion with spell `slot`, and wait until it has climbed out.
fn raise(run: &mut Run, slot: u8) {
    run.players.get_mut(&1).unwrap().spell_cooldowns = [0; 4];
    run.step(&BTreeMap::from([(1, Input { cast: slot, aim_x: 1, ..Default::default() })]));
    for _ in 0..=RISE {
        run.step(&BTreeMap::new());
    }
}

/// The scroll opened on known wish `id`, turned to `to`, and its first
/// choice picked.
fn wish_for(run: &mut Run, id: &str, to: Target) {
    let wish = phrasebook::get(id).unwrap();
    run.offer_wish(1, &wish.name, Some(&wish)).unwrap();
    while target(run) != Some(to) {
        assert!(run.turn_wish(1));
    }
    run.choose(1, 0).unwrap();
}

/// A colour wished over the dead's whole bodies: a learned wish's entry,
/// as the host's learner writes one.
pub(crate) const BRIGHT_PINK: &str = "wish bright-pink | Bright Pink
say make my ghouls bright pink | pink ghouls | pink dead
tier max_hp 10 :: Your dead turn bright pink, and 10 more health.
draw worn at=body
.7777.
7cccc7
7c99c7
7cccc7
7cccc7
.7777.
";

#[test]
fn four_turns_the_wish_from_him_to_his_dead_to_both_and_back() {
    let mut run = gravecaller();
    let heavy = phrasebook::get("heavy").unwrap();
    run.offer_wish(1, &heavy.name, Some(&heavy)).unwrap();
    assert_eq!(target(&run), Some(Target::You), "words naming no dead open on him");
    let mut seen = vec![];
    for _ in 0..3 {
        press(&mut run, 4);
        // Held, it turns nothing more.
        press(&mut run, 4);
        press(&mut run, 0);
        seen.push(target(&run).unwrap());
    }
    assert_eq!(seen, [Target::Dead, Target::Both, Target::You]);
    let hero = &run.players[&1];
    assert!(hero.offer.is_some(), "the scroll stays open");
    assert_eq!(hero.wishes, 3, "nothing is spent by turning it");
    assert!(run.notices.iter().all(|n| !n.contains("No such choice")), "{:?}", run.notices);
    assert_eq!(Target::Both.next(), Target::You);
}

#[test]
fn it_opens_on_his_dead_when_his_words_name_them() {
    for words in ["make my ghouls bright pink", "Zombies with hats!", "my army hits harder", "the dead walk faster", "give my pets wings"] {
        assert!(dead_wishes::named(words), "{words}");
    }
    for words in ["bright pink", "golden trumpets", "a deadly bow", "summon a dragon", "warm armies of light"] {
        assert_eq!(dead_wishes::named(words), words.contains("armies"), "{words}");
    }
    let mut run = gravecaller();
    run.offer_wish(1, "make my ghouls bright pink", None).unwrap();
    assert_eq!(target(&run), Some(Target::Dead));
    run.drop_offer(1);
    run.offer_wish(1, "bright pink", None).unwrap();
    assert_eq!(target(&run), Some(Target::You));
    run.drop_offer(1);
    // A known wish found in his words keeps only its name: the words say
    // whom it is for.
    let heavy = phrasebook::get("heavy").unwrap();
    run.offer_wish(1, &heavy.name, Some(&heavy)).unwrap();
    run.heard(1, "my skeletons hit harder");
    assert_eq!(target(&run), Some(Target::Dead));
    // Words still to be learned go to the host's learner saying so.
    run.drop_offer(1);
    run.offer_wish(1, "make my ghouls bright pink", None).unwrap();
    run.choose(1, 0).unwrap();
    assert_eq!(run.asked.len(), 1);
    assert_eq!(run.asked[0].target, Target::Dead);
}

#[test]
fn a_wish_for_his_dead_strikes_and_mends_through_them_and_not_him() {
    let mut run = gravecaller();
    raise(&mut run, 1);
    let (damage, max_hp) = (run.players[&1].bonus.damage, run.players[&1].max_hp);
    wish_for(&mut run, "heavy", Target::Dead);
    wish_for(&mut run, "breastplate", Target::Dead);
    let hero = &run.players[&1];
    assert_eq!(hero.dead_wishes.get("heavy"), Some(&Target::Dead));
    assert_eq!((hero.bonus.damage, hero.max_hp), (damage, max_hp), "none of it on him");
    assert_eq!((hero.dead_gifts.bonus.damage, hero.dead_gifts.hp), (10, 12), "all of it on them");
    assert!(run.notices.iter().any(|n| n.ends_with("'s dead wear Heavy Heads")), "{:?}", run.notices);
    // The ghoul already up is as hardy as the wish makes it, and mended
    // with it; one raised now comes up so.
    run.step(&BTreeMap::new());
    let ghoul = &run.minions.raised[0];
    assert_eq!((ghoul.max_hp, ghoul.hp), (90 + 12, 90 + 12));
    raise(&mut run, 2);
    assert_eq!(run.minions.raised[1].max_hp, 240 + 12);
    // Its claw lands a tenth harder.
    let (gx, gy) = (run.minions.raised[0].x, run.minions.raised[0].y);
    let foe = foe_at(&mut run, EnemyKind::Skeleton, (gx + 1.0, gy), u32::MAX / 2);
    let enemy = run.enemies.iter_mut().find(|e| e.id == foe).unwrap();
    (enemy.hp, enemy.max_hp) = (10_000, 10_000);
    run.phase = Phase::Fighting;
    let mut first = None;
    for _ in 0..2 * HZ {
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = hero.max_hp;
        run.step(&BTreeMap::new());
        let hp = run.enemies.iter().find(|e| e.id == foe).unwrap().hp;
        if hp < 10_000 {
            first = Some(10_000 - hp);
            break;
        }
    }
    assert_eq!(first, Some(15 * 110 / 100), "the claw's 15, harder by the wish");
    // A lifesteal wish for them: each blow mends the one that lands it.
    let thirst = phrasebook::read("wish grave-thirst | Grave Thirst\nsay my dead drink blood\ntier vamp 1 :: Your dead mend as they bite.\n").remove(0);
    run.aim_wish(1, &thirst, Target::Dead);
    run.grant(1, &thirst).unwrap();
    run.minions.raised[0].hp = 50;
    let before = run.enemies.iter().find(|e| e.id == foe).unwrap().hp;
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
        if run.enemies.iter().find(|e| e.id == foe).unwrap().hp < before {
            break;
        }
    }
    assert_eq!(run.minions.raised[0].hp, 50 + dead_wishes::DEAD_VAMP);
}

#[test]
fn the_gifts_quicken_harden_and_mail_his_dead() {
    let mut gifts = Gifts::default();
    assert_eq!((gifts.hit(15), gifts.rest(15), gifts.pace(), gifts.taken(100)), (15, 15, 1.0, 100), "none, as they were");
    (gifts.bonus.damage, gifts.bonus.rate, gifts.bonus.speed, gifts.bonus.armor) = (20, 25, 20, 2);
    assert_eq!((gifts.hit(15), gifts.rest(15), gifts.pace(), gifts.taken(100)), (18, 12, 1.2, 89));
    assert_eq!(gifts.taken(0), 0);
}

#[test]
fn a_wish_for_both_gives_each_half_rounded_up() {
    let mut run = gravecaller();
    let max_hp = run.players[&1].max_hp;
    wish_for(&mut run, "heavy", Target::Both);
    wish_for(&mut run, "breastplate", Target::Both);
    let hero = &run.players[&1];
    assert_eq!(hero.bonus.damage, 5);
    assert_eq!(hero.dead_gifts.bonus.damage, 5);
    assert_eq!((hero.max_hp, hero.dead_gifts.hp), (max_hp + 6, 6));
    assert!(run.notices.iter().any(|n| n.ends_with("and their dead wear Heavy Heads")), "{:?}", run.notices);
    // An odd number rounds up for each: armor 1 is one piece on each.
    let mut run = gravecaller();
    wish_for(&mut run, "iron", Target::Both);
    let hero = &run.players[&1];
    assert_eq!((hero.armor, hero.dead_gifts.bonus.armor), (1, 1));
    // Turned back to himself, the wish is his whole again.
    let mut run = gravecaller();
    wish_for(&mut run, "breastplate", Target::Both);
    let breastplate = phrasebook::get("breastplate").unwrap();
    run.aim_wish(1, &breastplate, Target::You);
    let hero = &run.players[&1];
    assert_eq!((hero.max_hp, hero.dead_gifts.hp), (max_hp + 12, 0));
    assert!(hero.dead_wishes.is_empty());
}

#[test]
fn his_dead_wear_the_look_wished_for_them_and_he_does_not() {
    let mut run = gravecaller();
    let pink = phrasebook::read(BRIGHT_PINK).remove(0);
    run.aim_wish(1, &pink, Target::Dead);
    run.grant(1, &pink).unwrap();
    let helm = phrasebook::get("great-helm").unwrap();
    run.aim_wish(1, &helm, Target::Both);
    run.grant(1, &helm).unwrap();
    let hero = &run.players[&1];
    let theirs: Vec<&str> = hero.dead_looks(&run.book).iter().map(|c| c.name.as_str()).collect();
    assert_eq!(theirs, ["Bright Pink I", "Great Helm I"]);
    let (pink_card, helm_card) = (run.book.get(&pink.card_id(0)).unwrap(), run.book.get(&helm.card_id(0)).unwrap());
    let worn = hero.worn(&run.book);
    let wears = |card: &Card| worn.iter().any(|s| std::ptr::eq(*s, card.looks.worn.as_ref().unwrap()));
    assert!(wears(helm_card) && !wears(pink_card), "he wears the helm, which is his too, and not the pink");
    assert!(dead_wishes::colours_body(pink_card), "a colour over the whole body");
    assert!(!dead_wishes::colours_body(helm_card), "a helm sits on a head");
    assert_eq!(hero.dead_gifts.hp, 10 + 4, "the pink's 10 and half the helm's 8");
}

#[test]
fn a_colour_wish_is_only_a_colour_and_a_coloured_thing_is_worn() {
    for name in ["Bright Pink I", "Purple", "Make Me Deep Purple II", "Gold And Black III"] {
        assert!(dead_wishes::only_colour(name), "{name} colours a body");
    }
    for name in ["Purple Drank Armor III", "Golden Armor II", "Full Poop Armor I", "Great Helm I"] {
        assert!(!dead_wishes::only_colour(name), "{name} is worn");
    }
}

#[test]
fn the_totems_grave_fire_takes_the_shot_words() {
    let mut run = gravecaller();
    wish_for(&mut run, "twin", Target::Dead);
    let foe = foe_at(&mut run, EnemyKind::Skeleton, (22.0, 12.0), u32::MAX / 2);
    let enemy = run.enemies.iter_mut().find(|e| e.id == foe).unwrap();
    (enemy.hp, enemy.max_hp) = (10_000, 10_000);
    run.phase = Phase::Fighting;
    raise(&mut run, 3);
    let bolts = |run: &Run| run.projectiles.iter().filter(|p| !p.hostile && p.kind == Shot::Bolt).count();
    for _ in 0..HZ {
        if bolts(&run) > 0 {
            break;
        }
        run.step(&BTreeMap::new());
    }
    assert_eq!(bolts(&run), 2, "a bolt more in each volley");
}

#[test]
fn a_spell_wished_for_his_dead_is_his_to_cast_in_the_wish_slot() {
    let mut run = gravecaller();
    wish_for(&mut run, "firebolt", Target::Dead);
    let hero = &run.players[&1];
    assert!(hero.spells[3].as_deref().is_some_and(|c| c.starts_with("wish-firebolt-")));
    assert!(hero.dead_wishes.is_empty(), "a spell is his");
}

#[test]
fn a_save_and_a_guests_copy_keep_whom_each_wish_was_for() {
    let mut run = gravecaller();
    let plain = serde_json::to_string(&run).unwrap();
    assert!(!plain.contains("dead_wishes") && !plain.contains("dead_gifts"), "nothing written while there are none");
    raise(&mut run, 1);
    wish_for(&mut run, "heavy", Target::Dead);
    wish_for(&mut run, "breastplate", Target::Both);
    let pink = phrasebook::read(BRIGHT_PINK).remove(0);
    run.aim_wish(1, &pink, Target::Dead);
    run.grant(1, &pink).unwrap();
    // A scroll left open keeps its toggle too.
    let quick = phrasebook::get("quick").unwrap();
    run.offer_wish(1, &quick.name, Some(&quick)).unwrap();
    run.turn_wish(1);
    run.turn_wish(1);
    let hero = &run.players[&1];
    let back: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    let saved = &back.players[&1];
    assert_eq!(saved.dead_wishes, hero.dead_wishes);
    assert_eq!(saved.dead_wishes.get("breastplate"), Some(&Target::Both));
    assert_eq!(saved.dead_gifts, hero.dead_gifts);
    assert_eq!(saved.offer.as_ref().unwrap().target, Some(Target::Both));
    // Counted again from the save, the same.
    let mut again = saved.clone();
    again.rebonus(&back.book);
    assert_eq!((again.bonus, again.dead_gifts), (hero.bonus, hero.dead_gifts));
    // A guest's copy over the wire draws the same looks on his dead.
    let tree = crate::wire::publish(&run).unwrap();
    let guest = crate::wire::read(&tree).unwrap();
    assert!(guest.valid_snapshot());
    let looks = |run: &Run| run.players[&1].dead_looks(&run.book).iter().map(|c| c.id.clone()).collect::<Vec<_>>();
    assert_eq!(looks(&guest), looks(&run));
    assert!(!looks(&guest).is_empty());
}

#[test]
fn a_knight_who_keeps_no_dead_has_no_toggle() {
    let mut run = Run::new(4, 2, Some("Matt"));
    let hero = run.players.get_mut(&1).unwrap();
    hero.wishes = 2;
    run.offer_wish(1, "make my ghouls bright pink", None).unwrap();
    assert_eq!(target(&run), None, "the words name dead he does not keep");
    press(&mut run, 4);
    assert_eq!(target(&run), None);
    assert!(!run.turn_wish(1));
    run.choose(1, 0).unwrap();
    assert_eq!(run.asked[0].target, Target::You);
    // Asked for his dead all the same, a wish is his own.
    let heavy = phrasebook::get("heavy").unwrap();
    assert_eq!(run.aim_wish(1, &heavy, Target::Dead), Target::You);
    run.grant(1, &heavy).unwrap();
    let hero = &run.players[&1];
    assert!(hero.dead_wishes.is_empty());
    assert_eq!(hero.bonus.damage, 10);
}
