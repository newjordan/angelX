//! What a wish may be made of, as one machine-readable answer.
//!
//! An agent answering a player's wish needs the vocabulary before it needs
//! imagination: the words this engine already honours, the four slots a wish
//! fills, and the bounds the checker enforces. Given that, a wild request is a
//! composition rather than a new feature — and a request that *cannot* be
//! composed is answered honestly instead of half-built.
//!
//! The list must not rot. [`tests`] reads `cards.rs` as text and fails if a
//! word the checker honours is missing here, so this stays the truth as the
//! vocabulary grows.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Trigger,
    Target,
    Effect,
    Budget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Shot,
    Self_,
    Unit,
    Field,
    Cast,
    Look,
    Meta,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Bound {
    pub what: &'static str,
    pub low: i64,
    pub high: i64,
}

pub const VOCABULARY: &[(Family, &[&str])] = &[
    (
        Family::Shot,
        &[
            "damage", "rate", "speed", "velocity", "burst", "shots", "pierce", "bounce", "homing",
            "chain", "orbit", "fire", "frost", "leech", "trail", "split", "rail", "fan", "seek",
            "bolt", "ring", "meteor",
        ],
    ),
    (
        Family::Self_,
        &[
            "armor", "speed", "shots", "rate", "damage", "leech", "guard", "roll", "shield", "spin",
            "wall", "spell", "cast", "cooldown", "mana", "hold", "play", "take", "arm", "move",
            "ult", "stones", "bow", "crossbow", "handgonne",
        ],
    ),
    (
        Family::Unit,
        &["summon", "ghoul", "warden", "totem", "zombie", "hireling", "companion"],
    ),
    (
        Family::Field,
        &["place", "barracks", "nest", "well", "beacon", "wall", "aura", "zone", "trap"],
    ),
    (Family::Cast, &["cast", "bolt", "ring", "meteor", "summon", "frame"]),
    (
        Family::Look,
        &["paint", "look", "sheet", "name", "text", "notes", "normal", "hd", "pal", "vfx"],
    ),
    (
        Family::Meta,
        &[
            "window", "reroll", "rewind", "law", "teach", "learn", "reforge", "rarity", "common",
            "rare", "relic", "where", "by", "kind", "move", "ult", "stones", "bow", "crossbow",
            "handgonne",
        ],
    ),
];

pub const BOUNDS: &[Bound] = &[
    Bound { what: "spell bolts a casting", low: 1, high: super::invented::MAX_BOLTS as i64 },
    Bound { what: "spell damage a bolt", low: 1, high: super::invented::MAX_DAMAGE },
    Bound { what: "spell slow, ms", low: 0, high: super::invented::MAX_SLOW as i64 },
    Bound { what: "spell mana", low: 0, high: super::invented::MAX_MANA as i64 },
    Bound { what: "structure units alive", low: 1, high: super::field::MAX_UNITS as i64 },
    Bound { what: "structure ticks between units", low: super::field::MIN_EVERY as i64, high: 30 * 600 },
    Bound { what: "structure ticks standing", low: 1, high: super::field::MAX_LIFETIME as i64 },
    Bound { what: "structure armour", low: 1, high: super::field::MAX_ARMOUR },
    Bound { what: "mana a wish may take", low: 0, high: super::invented::MAX_MANA as i64 },
];

pub fn manifest() -> serde_json::Value {
    serde_json::json!({
        "slots": [Slot::Trigger, Slot::Target, Slot::Effect, Slot::Budget],
        "families": VOCABULARY
            .iter()
            .map(|(family, words)| serde_json::json!({"family": family, "words": words}))
            .collect::<Vec<_>>(),
        "bounds": BOUNDS,
        "rule": "A wish that composes these words is granted from data, and can be \
                 granted while the party fights. A wish that needs a word not here is \
                 one new verb: engine work, said plainly, never half-built.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn honoured() -> Vec<String> {
        let source = include_str!("cards.rs");
        let mut words = Vec::new();
        for line in source.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix('"') else { continue };
            let Some((word, tail)) = rest.split_once('"') else { continue };
            if tail.trim_start().starts_with("=>") && word.len() > 1 && !words.contains(&word.to_owned()) {
                words.push(word.to_owned());
            }
        }
        words
    }

    #[test]
    fn every_word_the_checker_honours_is_in_the_manifest() {
        let listed: Vec<&str> = VOCABULARY.iter().flat_map(|(_, words)| *words).copied().collect();
        let missing: Vec<String> =
            honoured().into_iter().filter(|word| !listed.contains(&word.as_str())).collect();
        assert!(missing.is_empty(), "the manifest has lost words the checker honours: {missing:?}");
    }

    #[test]
    fn the_manifest_answers_the_three_cases_a_wish_can_be() {
        let manifest = manifest();
        assert!(manifest["slots"].as_array().is_some_and(|it| it.len() == 4));
        assert!(manifest["families"].as_array().is_some_and(|it| it.len() >= 5));
        let bounds = manifest["bounds"].as_array().expect("bounds");
        assert!(bounds.iter().all(|it| it["low"].as_i64().unwrap() <= it["high"].as_i64().unwrap()));
        let words: Vec<&str> = manifest["families"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|family| family["words"].as_array().unwrap().iter())
            .filter_map(|word| word.as_str())
            .collect();
        assert!(words.contains(&"burst") && words.contains(&"place") && words.contains(&"summon"));
    }
}
