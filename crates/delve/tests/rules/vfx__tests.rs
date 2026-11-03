//! Recipes: read and written back, held to their bounds, set aside and
//! never refused, made from a card's words the same way every time and a
//! different way for different words; a card without one is unchanged.

use super::*;
use crate::rules::cards;

fn card(raw: &str) -> Card {
    cards::check("test-card", raw).unwrap().card
}

#[test]
fn a_line_reads_and_writes_back_the_same() {
    let (l, notes) = Layer::read("hit shards frost back count=12 speed=5 spread=360 life=16 size=2 wobble=1 spin=-2").unwrap();
    assert!(notes.is_empty());
    assert_eq!((l.on, l.form, l.palette, l.ease), (When::Hit, Form::Shards, Palette::Frost, Ease::Back));
    let again = Layer::read(l.line().strip_prefix("vfx ").unwrap()).unwrap().0;
    assert_eq!(l, again);
}

#[test]
fn every_number_is_held_to_its_bounds() {
    let (l, notes) = Layer::read("trail sparks fire count=200 speed=99 spread=9000 life=1 size=9 wobble=50 spin=-100").unwrap();
    assert_eq!(l.count, most(When::Trail));
    assert_eq!((l.speed, l.spread, l.life, l.size, l.wobble, l.spin), (SPEED.1, SPREAD.1, LIFE.0, SIZE.1, WOBBLE.1, SPIN.0));
    assert_eq!(notes.len(), 7, "{notes:?}");
}

#[test]
fn a_card_carries_its_recipe_and_writes_it_back() {
    let c = card("name Comet Tail\nkind hold\ndamage 5\nvfx trail sparks storm zigzag count=5 wobble=3\nvfx hit ring storm back\n");
    assert_eq!(c.looks.vfx.layers.len(), 2);
    assert!(c.looks.vfx.valid());
    let again = cards::check("test-card", &c.file()).unwrap().card;
    assert_eq!(again.looks.vfx, c.looks.vfx);
    assert_eq!(of(&c), c.looks.vfx, "a card's own recipe wins");
}

#[test]
fn a_bad_line_is_set_aside_and_the_card_kept() {
    let checked = cards::check("test-card", "name Odd\nkind hold\ndamage 5\nvfx sometimes glitter\nvfx hit sparks plaid\n").unwrap();
    assert!(checked.card.looks.vfx.is_empty());
    assert_eq!(checked.notes.iter().filter(|n| n.contains("set aside")).count(), 2, "{:?}", checked.notes);
    assert!(!generated(&checked.card).is_empty(), "its words give it a look");
}

#[test]
fn a_card_without_a_recipe_serializes_as_it_did() {
    let c = card("name Plain\nkind hold\ndamage 5\n");
    let json = serde_json::to_string(&c).unwrap();
    assert!(!json.contains("vfx"), "{json}");
    let back: Card = serde_json::from_str(&json).unwrap();
    assert_eq!(back, c);
}

#[test]
fn the_same_words_give_the_same_look_and_different_words_another() {
    let names = [
        "Poop Worms", "Purple Drank Armor", "Buttcheek Bullets", "Ice Shatter", "Life Steal Bullets", "Storm Bolts",
        "Holy Arrows", "Dragon Fire", "Bee Swarm", "Shadow Darts", "Lucky Coin Shot", "Bubble Gun", "Rose Petal Storm",
        "Song Shots",
    ];
    let mut looks = Vec::new();
    for name in names {
        let c = card(&format!("name {name}\nkind hold\ndamage 5\n"));
        let r = generated(&c);
        assert_eq!(r, generated(&c), "{name} is the same every time");
        assert!(r.valid(), "{name}: {r:?}");
        let trail = r.first(When::Trail).unwrap();
        looks.push((name, trail.palette, trail.form, trail.motion));
    }
    for (i, a) in looks.iter().enumerate() {
        for b in &looks[i + 1..] {
            assert_ne!((a.1, a.2, a.3), (b.1, b.2, b.3), "{} and {} look alike", a.0, b.0);
        }
    }
    let find = |n: &str| looks.iter().find(|l| l.0 == n).unwrap();
    assert_eq!(find("Poop Worms").1, Palette::Poop);
    assert_eq!(find("Poop Worms").2, Form::Worms);
    assert_eq!(find("Ice Shatter").1, Palette::Frost);
    assert_eq!(find("Dragon Fire").1, Palette::Fire);
    assert_eq!(find("Life Steal Bullets").1, Palette::Blood);
}

#[test]
fn a_wish_cards_tier_is_read_from_its_id() {
    assert_eq!(tier("wish-dragon-fire-4"), Some(4));
    assert_eq!(tier("wish-x-1"), Some(1));
    assert_eq!(tier("thunder-scroll"), None);
}

#[test]
fn the_brief_names_every_word() {
    let b = brief();
    for w in [When::words(), Form::words(), Palette::words()] {
        for word in w.split(' ') {
            assert!(b.contains(word), "{word}");
        }
    }
}

#[test]
fn a_spell_casts_and_breaks_in_its_familys_voice() {
    for (raw, voice) in [
        ("name Fire Bolt\nkind spell\ncooldown 6\ncast bolt\n", Voice::Fire),
        ("name Frost Ring\nkind spell\ncooldown 6\ncast ring\n", Voice::Frost),
        ("name Holy Light\nkind spell\ncooldown 6\nheal 20\n", Voice::Holy),
        ("name Thunder Call\nkind spell\ncooldown 6\ncast bolt\n", Voice::Storm),
        ("name Acid Spray\nkind spell\ncooldown 6\ncast bolt\n", Voice::Poison),
        ("name Stone Fist\nkind spell\ncooldown 6\ncast meteor\n", Voice::Earth),
        ("name Moon Thread\nkind spell\ncooldown 6\ncast bolt\n", Voice::Arcane),
        // A recipe of its own speaks first.
        ("name Plain Bolt\nkind spell\ncooldown 6\ncast bolt\nvfx cast glyph frost\n", Voice::Frost),
    ] {
        let c = card(raw);
        assert_eq!(palette(&c).voice(), voice, "{raw}");
        assert_eq!((cast_sound(&c), impact_sound(&c)), (voice.cast(), voice.impact()), "{raw}");
    }
    // Every palette sounds in a family, and every family is some palette's.
    let all = [Voice::Fire, Voice::Frost, Voice::Holy, Voice::Storm, Voice::Poison, Voice::Arcane, Voice::Earth];
    for v in all {
        assert!(Palette::ALL.iter().any(|p| p.voice() == v), "{v:?}");
        assert!(v.cast().starts_with("cast_") && v.impact().starts_with("impact_"));
        assert_eq!(v.cast()["cast_".len()..], v.impact()["impact_".len()..]);
    }
}
