//! The phrasebook: wishes the Delve knows by heart (`phrasebook.txt`).
//!
//! A player's words are matched to a known wish in microseconds, and the
//! wish is granted in the running game: no model, no wait. Granted again,
//! it climbs its ladder of tiers, each a card the card checker bounds.
//!
//! A wish's words only match an entry whose phrases cover all of them (or
//! all but one of four or more): "golden spiky armor that explodes" is not
//! Iron Skin with extra words, it is a new thing, and goes to be learned.
//! An entry may dress its cards: `draw <slot> [tier]` blocks (`looks`),
//! from the tier named up, and `sheet` lines that the workspace's pictures
//! are cut into when the book loads (`together_sheets`).

use super::*;

pub const TEXT: &str = include_str!("../../../../cockpit/assets/dungeon/phrasebook.txt");

/// One known wish: its id and name, the ways players put it, its tiers
/// (effect lines and what the scroll says).
#[derive(Clone, Debug, PartialEq)]
pub struct Wish {
    pub id: String,
    pub name: String,
    /// The checked card capability granted by every tier (hold by default).
    pub kind: String,
    pub says: Vec<Vec<String>>,
    pub tiers: Vec<(Vec<String>, String)>,
    /// Its own card art, when the entry carries an `art` block.
    pub art: Vec<String>,
    /// Its cards' looks on the field: from which tier (0 for every one),
    /// the slot, and the card lines that draw it (`draw <slot>` and its
    /// rows, or `look <colour> <shape>`).
    pub looks: Vec<(usize, String, String)>,
    /// Learned in this workspace (`.angel/dungeon/phrasebook.txt`), not one
    /// of the delve's own: a tie goes to it.
    pub learned: bool,
}

/// The line that marks where the workspace's own wishes begin.
pub const LEARNED: &str = "## learned in this workspace";
// Leave room for `wish-`, a dash and the tier in the card's 32-char id.
pub const MAX_ID: usize = 24;
pub const MAX_TIERS: usize = 5;

/// The phrasebook in play: the delve's own, then the workspace's
/// (`.angel/dungeon/phrasebook.txt`, which players and agents grow), as
/// last read by the host.
struct Live {
    text: String,
    book: std::sync::Arc<Vec<Wish>>,
    /// Why the workspace's entries (or their sheets) were turned away, as
    /// last read: what a lesson that taught nothing is told.
    errors: Vec<String>,
}

fn live() -> &'static std::sync::RwLock<Live> {
    static LIVE: std::sync::OnceLock<std::sync::RwLock<Live>> = std::sync::OnceLock::new();
    LIVE.get_or_init(|| {
        std::sync::RwLock::new(Live {
            text: TEXT.to_string(),
            book: std::sync::Arc::new(read(TEXT)),
            errors: Vec::new(),
        })
    })
}

/// The phrasebook in play.
pub fn book() -> std::sync::Arc<Vec<Wish>> {
    live()
        .read()
        .map_or_else(|_| std::sync::Arc::new(read(TEXT)), |l| l.book.clone())
}

/// Why the workspace's entries were turned away, as last read.
pub fn errors() -> Vec<String> {
    live()
        .read()
        .map_or_else(|_| Vec::new(), |l| l.errors.clone())
}

/// Take the phrasebook the host read from disk (the delve's own, then the
/// workspace's below [`LEARNED`], its pictures cut into rows), with why the
/// workspace's entries were turned away. True when the book changed;
/// `None` when the book in play could not be reached.
pub fn install(text: String, mut errors: Vec<String>) -> Option<bool> {
    let (book, rejected) = read_checked(&text);
    errors.extend(rejected);
    let Ok(mut l) = live().write() else {
        return None;
    };
    l.errors = errors;
    if l.text == text {
        return Some(false);
    }
    l.book = std::sync::Arc::new(book);
    l.text = text;
    Some(true)
}

/// Words as the matcher sees them: lower case, letters and digits, a
/// plural's `s` dropped, small words gone.
fn words(text: &str) -> Vec<String> {
    const SMALL: &[&str] = &[
        "a", "an", "the", "my", "me", "i", "to", "of", "and", "when", "them", "it", "please",
        "make", "give", "want", "can", "be", "that", "which", "with", "so", "is", "are", "some",
        "very", "really", "just", "lot", "lots", "much", "you", "your", "us", "we", "our", "get",
        "have", "let", "like", "would", "could", "for", "im", "id", "wish",
    ];
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !SMALL.contains(w))
        .map(|w| {
            if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") {
                w[..w.len() - 1].to_string()
            } else {
                w.to_string()
            }
        })
        .collect()
}

pub fn read(text: &str) -> Vec<Wish> {
    read_checked(text).0
}

/// Read a phrasebook: the wishes whose every tier checks, and why each of
/// the others was turned away.
pub fn read_checked(text: &str) -> (Vec<Wish>, Vec<String>) {
    let mut book: Vec<Wish> = Vec::new();
    let mut learned = false;
    // Inside an `art` or `draw` block: rows until a blank line or a
    // keyword.
    let mut drawing = false;
    let mut sprite: Option<(usize, String, String)> = None;
    let mut bad_lines: Vec<(usize, String)> = Vec::new();
    let keyword = |line: &str| {
        line.split_whitespace().next().is_some_and(|w| {
            matches!(
                w,
                "wish" | "kind" | "say" | "tier" | "art" | "draw" | "look" | "sheet"
            )
        })
    };
    for raw in text.lines() {
        let line = raw.trim();
        if let Some((from, slot, mut block)) = sprite.take() {
            if !line.is_empty() && !keyword(line) && super::script::sprite_row(line) {
                block.push_str(line);
                block.push('\n');
                sprite = Some((from, slot, block));
                continue;
            }
            if let Some(wish) = book.last_mut() {
                wish.looks.push((from, slot, block));
            }
        }
        if drawing {
            if line.is_empty() || keyword(line) {
                drawing = false;
            } else {
                if let Some(wish) = book.last_mut() {
                    // Every row is kept: the card checker says when the
                    // art is too big, rather than the reader cutting it.
                    wish.art.push(line.to_string());
                }
                continue;
            }
        }
        if line == LEARNED {
            learned = true;
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "art" {
            drawing = true;
            continue;
        }
        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        match word {
            "wish" => {
                let (id, name) = rest.split_once('|').unwrap_or((rest, rest));
                book.push(Wish {
                    id: cards::slug(id.trim()),
                    name: name.trim().to_string(),
                    kind: "hold".into(),
                    says: Vec::new(),
                    tiers: Vec::new(),
                    art: Vec::new(),
                    looks: Vec::new(),
                    learned,
                });
            }
            "kind" => {
                if let Some(wish) = book.last_mut() {
                    wish.kind = rest.to_string();
                }
            }
            "say" => {
                if let Some(wish) = book.last_mut() {
                    wish.says
                        .extend(rest.split('|').map(words).filter(|w| !w.is_empty()));
                }
            }
            "tier" => {
                if let Some(wish) = book.last_mut() {
                    let (effects, says) = rest.split_once("::").unwrap_or((rest, ""));
                    wish.tiers.push((
                        effects.split(';').map(|e| e.trim().to_string()).collect(),
                        says.trim().to_string(),
                    ));
                }
            }
            // `draw <slot> [from tier]`, rows to follow.
            "draw" => {
                let mut parts = rest.split_whitespace();
                let word = parts.next().unwrap_or("").to_ascii_lowercase();
                let slot = super::looks::Slot::parse(&word).map_or(word, |s| s.word().to_string());
                let mut from = 0;
                let mut has_tier = false;
                let mut header = format!("draw {slot}");
                for part in parts {
                    match part.parse::<usize>() {
                        Ok(tier) if tier > 0 && !has_tier => {
                            from = tier - 1;
                            has_tier = true;
                        }
                        Ok(_) => {
                            if !book.is_empty() {
                                bad_lines.push((
                                    book.len() - 1,
                                    "a draw block takes one tier, starting at 1".into(),
                                ));
                            }
                        }
                        Err(_) => {
                            header.push(' ');
                            header.push_str(part);
                        }
                    }
                }
                sprite = Some((from, slot, format!("{header}\n")));
            }
            "look" => {
                if let Some(wish) = book.last_mut() {
                    wish.looks
                        .push((0, "look".into(), format!("look {rest}\n")));
                }
            }
            _ => {
                if !book.is_empty() {
                    bad_lines.push((book.len() - 1, format!("unknown line `{word}`")));
                }
            }
        }
    }
    if let Some((from, slot, block)) = sprite
        && let Some(wish) = book.last_mut()
    {
        wish.looks.push((from, slot, block));
    }
    // Publish only complete checked ladders. A broken workspace override
    // must not hide a valid earlier wish or partially grant its first tier.
    let mut errors = Vec::new();
    let mut index = 0;
    book.retain(|w| {
        let here = index;
        index += 1;
        let invalid: Vec<_> = bad_lines.iter().filter(|(i, _)| *i == here).collect();
        if !invalid.is_empty() {
            errors.extend(
                invalid
                    .iter()
                    .map(|(_, why)| format!("wish {}: {why}", w.id)),
            );
            return false;
        }
        if w.id.is_empty() {
            errors.push(format!("an entry `wish {}` has no id", w.name));
            return false;
        }
        if w.id.len() > MAX_ID || w.tiers.len() > MAX_TIERS {
            errors.push(format!(
                "wish {}: at most {MAX_ID} characters in its id and {MAX_TIERS} tiers",
                w.id
            ));
            return false;
        }
        if w.tiers.is_empty() {
            errors.push(format!("wish {}: no `tier` lines", w.id));
            return false;
        }
        if w.says.is_empty() {
            errors.push(format!("wish {}: no `say` phrases", w.id));
            return false;
        }
        if w.looks.iter().any(|(from, _, _)| *from >= w.tiers.len()) {
            errors.push(format!(
                "wish {}: a draw block starts past the last tier",
                w.id
            ));
            return false;
        }
        for tier in 0..w.tiers.len() {
            if let Err(why) = w.card(tier) {
                errors.push(format!("wish {} tier {}: {why}", w.id, tier + 1));
                return false;
            }
        }
        true
    });
    // A later wish of the same id (the workspace's) replaces the earlier.
    let mut kept: Vec<Wish> = Vec::new();
    for wish in book {
        match kept.iter().position(|k| k.id == wish.id) {
            Some(i) => kept[i] = wish,
            None => kept.push(wish),
        }
    }
    (kept, errors)
}

/// The known wish a player's words mean, if any (see `find_for`).
#[cfg(any(test, feature = "testing"))]
pub fn find(said: &str) -> Option<Wish> {
    find_for(said, &[])
}

/// The known wish a player's words mean, for a knight holding `deck`, if
/// any. A phrase counts when all its words are there; an entry matches
/// when its phrases cover all the words (or, of four or more, all but
/// one), so a wish with words no stock phrase has ("golden spiky armor that
/// explodes", "a bigger spiked shield") is a new thing to learn, not the
/// nearest stock card. The entry covering the most words wins, so "fire
/// faster" is Quick Hands and not Fleet Foot; a tie goes to a learned entry,
/// then to the later. A ladder the knight holds in full is passed over.
pub fn find_for(said: &str, deck: &[String]) -> Option<Wish> {
    find_in(&book(), said, deck)
}

/// `find_for` in a given book.
pub fn find_in(book: &[Wish], said: &str, deck: &[String]) -> Option<Wish> {
    let mut said = words(said);
    said.sort();
    said.dedup();
    if said.is_empty() {
        return None;
    }
    let mut best: Option<((usize, bool, usize), &Wish)> = None;
    for (i, wish) in book.iter().enumerate() {
        if wish.next_tier(deck).is_none() {
            continue;
        }
        let mut covered: Vec<&str> = wish
            .says
            .iter()
            .filter(|phrase| phrase.iter().all(|w| said.contains(w)))
            .flatten()
            .map(String::as_str)
            .collect();
        covered.sort_unstable();
        covered.dedup();
        if covered.is_empty() {
            continue;
        }
        let left = said.len() - covered.len();
        if !(left == 0 || (left == 1 && said.len() >= 4)) {
            continue;
        }
        let key = (covered.len(), wish.learned, i);
        if best.as_ref().is_none_or(|(k, _)| key > *k) {
            best = Some((key, wish));
        }
    }
    best.map(|(_, wish)| wish.clone())
}

/// The delve's own wishes' card art (`wish-art/<id>.art`, ink rows), built
/// into the game so a shipped copy draws them too.
const WISH_ART: &[(&str, &str)] = &[
    (
        "fork",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/fork.art"),
    ),
    (
        "fart",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/fart.art"),
    ),
    (
        "chain",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/chain.art"),
    ),
    (
        "fleet",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/fleet.art"),
    ),
    (
        "heavy",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/heavy.art"),
    ),
    (
        "iron",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/iron.art"),
    ),
    (
        "leech",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/leech.art"),
    ),
    (
        "mend",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/mend.art"),
    ),
    (
        "morningstar",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/morningstar.art"),
    ),
    (
        "pierce",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/pierce.art"),
    ),
    (
        "powder",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/powder.art"),
    ),
    (
        "quick",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/quick.art"),
    ),
    (
        "ricochet",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/ricochet.art"),
    ),
    (
        "seeker",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/seeker.art"),
    ),
    (
        "stout",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/stout.art"),
    ),
    (
        "twin",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/twin.art"),
    ),
    (
        "volley",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/volley.art"),
    ),
];

/// A wish's own card art, as a card's `art` block, when one has been drawn.
fn art_for(id: &str) -> Option<String> {
    let (_, rows) = WISH_ART.iter().find(|(art, _)| *art == id)?;
    let rows: Vec<&str> = rows
        .lines()
        .map(str::trim_end)
        .filter(|r| !r.is_empty() && !r.starts_with('#'))
        .take(cards::ART)
        .collect();
    (!rows.is_empty()).then(|| format!("art\n{}", rows.join("\n")))
}

/// The wish named by id.
pub fn get(id: &str) -> Option<Wish> {
    book().iter().find(|w| w.id == id).cloned()
}

impl Wish {
    /// A tier's card id.
    pub fn card_id(&self, tier: usize) -> String {
        format!("wish-{}-{}", self.id, tier + 1)
    }

    /// The tier a knight holding `deck` would be granted next, if the ladder
    /// has one left.
    pub fn next_tier(&self, deck: &[String]) -> Option<usize> {
        let held = (0..self.tiers.len())
            .rev()
            .find(|&t| deck.contains(&self.card_id(t)));
        let next = held.map_or(0, |t| t + 1);
        (next < self.tiers.len()).then_some(next)
    }

    /// The card a tier grants, as the card file a player could write.
    pub fn card_text(&self, tier: usize) -> String {
        let (effects, says) = &self.tiers[tier];
        let rarity = match tier {
            0 => "common",
            t if t + 1 < self.tiers.len() => "rare",
            _ => "relic",
        };
        let numeral = ["I", "II", "III", "IV", "V"]
            .get(tier)
            .copied()
            .unwrap_or("+");
        let art = if self.art.is_empty() {
            art_for(&self.id).unwrap_or_default()
        } else {
            format!("art\n{}", self.art.join("\n"))
        };
        // For each slot, the look drawn for the highest tier at or below
        // this one.
        let mut looks: Vec<(&str, usize, &str)> = Vec::new();
        for (from, slot, block) in &self.looks {
            if *from > tier {
                continue;
            }
            match looks.iter_mut().find(|(s, _, _)| s == slot) {
                Some(look) if look.1 <= *from => *look = (slot, *from, block),
                Some(_) => {}
                None => looks.push((slot, *from, block)),
            }
        }
        let looks: String = looks.iter().map(|(_, _, block)| *block).collect();
        format!(
            "name {} {numeral}\nkind {}\nrarity {rarity}\ntext {says}\ndrop 0\nchest 0\nby the phrasebook\n{}\n{looks}\n{art}",
            self.name,
            self.kind,
            effects.join("\n"),
        )
    }

    /// The checked card for a tier.
    pub fn card(&self, tier: usize) -> Result<Card, String> {
        if !matches!(self.kind.as_str(), "hold" | "guard" | "arm" | "spell") {
            return Err("a wish's kind must be hold, guard, arm or spell".into());
        }
        if tier >= self.tiers.len() {
            return Err("no such wish tier".into());
        }
        cards::check(&self.card_id(tier), &self.card_text(tier))
            .map(|checked| checked.card)
            .map_err(|rejected| rejected.errors.join("; "))
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_phrasebook__tests.rs"]
mod tests;
