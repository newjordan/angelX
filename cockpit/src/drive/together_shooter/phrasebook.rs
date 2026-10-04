//! The phrasebook: wishes the Delve knows by heart (`phrasebook.txt`).
//!
//! A player's words are matched to a known wish in microseconds, and the
//! wish is granted in the running game: no model, no wait. Granted again,
//! it climbs its ladder of tiers, each a card the card checker bounds.

use super::*;

const TEXT: &str = include_str!("../../../assets/dungeon/phrasebook.txt");

/// One known wish: its id and name, the ways players put it, its tiers
/// (effect lines and what the scroll says).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Wish {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) says: Vec<Vec<String>>,
    pub(crate) tiers: Vec<(Vec<String>, String)>,
    /// Its own card art, when the entry carries an `art` block.
    pub(crate) art: Vec<String>,
}

/// The phrasebook in play: the delve's own, then the workspace's
/// (`.angel/dungeon/phrasebook.txt`, which players and agents grow), as
/// last read; and its version, which every reload bumps.
struct Live {
    text: String,
    book: std::sync::Arc<Vec<Wish>>,
    version: u64,
    seen: Option<(Option<std::time::SystemTime>, Option<std::time::SystemTime>)>,
}

fn live() -> &'static std::sync::RwLock<Live> {
    static LIVE: std::sync::OnceLock<std::sync::RwLock<Live>> = std::sync::OnceLock::new();
    LIVE.get_or_init(|| {
        std::sync::RwLock::new(Live {
            text: TEXT.to_string(),
            book: std::sync::Arc::new(read(TEXT)),
            version: 1,
            seen: None,
        })
    })
}

/// The phrasebook in play.
pub(crate) fn book() -> std::sync::Arc<Vec<Wish>> {
    live()
        .read()
        .map_or_else(|_| std::sync::Arc::new(read(TEXT)), |l| l.book.clone())
}

/// Its version: a friend fetches it again when this moves.
pub(crate) fn version() -> u64 {
    live().read().map_or(0, |l| l.version)
}

/// Its text, for a friend's angelX to read the same book.
pub(crate) fn text() -> String {
    live()
        .read()
        .map_or_else(|_| TEXT.to_string(), |l| l.text.clone())
}

/// Take a phrasebook's text as the one in play (a friend's copy of the
/// host's).
pub(crate) fn take(text: &str, version: u64) {
    if let Ok(mut l) = live().write() {
        l.text = text.to_string();
        l.book = std::sync::Arc::new(read(text));
        l.version = version;
    }
}

/// Where the delve's own phrasebook lives on disk, to read it live.
fn own_path() -> Option<std::path::PathBuf> {
    [
        std::env::var_os("ANGEL_RESOURCE_DIR")
            .map(|d| std::path::PathBuf::from(d).join("cockpit/assets/dungeon/phrasebook.txt")),
        Some(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets/dungeon/phrasebook.txt"),
        ),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.is_file())
}

/// Read the phrasebook again if either file changed since last time: the
/// game takes new wishes while it runs. True when the book changed.
pub(crate) fn refresh(workspace: &std::path::Path) -> bool {
    let own = own_path();
    let mine = workspace.join(".angel/dungeon/phrasebook.txt");
    let stamp = |p: &std::path::Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let seen = (own.as_deref().and_then(stamp), stamp(&mine));
    if live().read().is_ok_and(|l| l.seen == Some(seen)) {
        return false;
    }
    let mut text = own
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_else(|| TEXT.to_string());
    if let Ok(more) = std::fs::read_to_string(&mine) {
        text.push('\n');
        text.push_str(&more);
    }
    let Ok(mut l) = live().write() else {
        return false;
    };
    l.seen = Some(seen);
    if l.text == text {
        return false;
    }
    l.book = std::sync::Arc::new(read(&text));
    l.text = text;
    l.version += 1;
    true
}

/// Words as the matcher sees them: lower case, letters and digits, a
/// plural's `s` dropped, small words gone.
fn words(text: &str) -> Vec<String> {
    const SMALL: &[&str] = &[
        "a", "an", "the", "my", "me", "i", "to", "of", "and", "when", "them", "it", "please",
        "make", "give", "want", "can", "be",
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

pub(crate) fn read(text: &str) -> Vec<Wish> {
    let mut book: Vec<Wish> = Vec::new();
    // Inside an `art` block: rows of inks until a blank line or a keyword.
    let mut drawing = false;
    for raw in text.lines() {
        let line = raw.trim();
        if drawing {
            let keyword = line
                .split_whitespace()
                .next()
                .is_some_and(|w| matches!(w, "wish" | "say" | "tier" | "art"));
            if line.is_empty() || keyword {
                drawing = false;
            } else {
                if let Some(wish) = book.last_mut()
                    && wish.art.len() < cards::ART
                {
                    wish.art.push(line.chars().take(cards::ART).collect());
                }
                continue;
            }
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
                    says: Vec::new(),
                    tiers: Vec::new(),
                    art: Vec::new(),
                });
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
            _ => {}
        }
    }
    book.retain(|w| !w.id.is_empty() && !w.tiers.is_empty());
    // A later wish of the same id (the workspace's) replaces the earlier.
    let mut kept: Vec<Wish> = Vec::new();
    for wish in book {
        match kept.iter().position(|k| k.id == wish.id) {
            Some(i) => kept[i] = wish,
            None => kept.push(wish),
        }
    }
    kept
}

/// The known wish a player's words mean, if any: the phrase that the words
/// cover most fully wins (a phrase counts only when all its words are
/// there), so "fire faster" is Quick Hands and not Fleet Foot.
pub(crate) fn find(said: &str) -> Option<Wish> {
    let said = words(said);
    let book = book();
    let mut best: Option<(usize, &Wish)> = None;
    for wish in book.iter() {
        for phrase in &wish.says {
            if phrase.iter().all(|w| said.contains(w)) && best.is_none_or(|(n, _)| phrase.len() > n)
            {
                best = Some((phrase.len(), wish));
            }
        }
    }
    best.map(|(_, wish)| wish.clone())
}

/// A wish's own card art (`wish-art/<id>.art`, ink rows), as a card's
/// `art` block, when one has been drawn.
fn art_for(id: &str) -> Option<String> {
    let dirs = [
        std::env::var_os("ANGEL_RESOURCE_DIR")
            .map(|d| std::path::PathBuf::from(d).join("cockpit/assets/dungeon/wish-art")),
        Some(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/dungeon/wish-art")),
    ];
    let rows = dirs
        .into_iter()
        .flatten()
        .find_map(|dir| std::fs::read_to_string(dir.join(format!("{id}.art"))).ok())?;
    let rows: Vec<&str> = rows
        .lines()
        .map(str::trim_end)
        .filter(|r| !r.is_empty() && !r.starts_with('#'))
        .take(cards::ART)
        .collect();
    (!rows.is_empty()).then(|| format!("art\n{}", rows.join("\n")))
}

/// The wish named by id.
pub(crate) fn get(id: &str) -> Option<Wish> {
    book().iter().find(|w| w.id == id).cloned()
}

impl Wish {
    /// A tier's card id.
    pub(crate) fn card_id(&self, tier: usize) -> String {
        format!("wish-{}-{}", self.id, tier + 1)
    }

    /// The tier a knight holding `deck` would be granted next, if the ladder
    /// has one left.
    pub(crate) fn next_tier(&self, deck: &[String]) -> Option<usize> {
        let held = (0..self.tiers.len())
            .rev()
            .find(|&t| deck.contains(&self.card_id(t)));
        let next = held.map_or(0, |t| t + 1);
        (next < self.tiers.len()).then_some(next)
    }

    /// The card a tier grants, as the card file a player could write.
    pub(crate) fn card_text(&self, tier: usize) -> String {
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
        format!(
            "name {} {numeral}\nkind hold\nrarity {rarity}\ntext {says}\ndrop 0\nchest 0\nby the phrasebook\n{}\n{art}",
            self.name,
            effects.join("\n"),
        )
    }

    /// The checked card for a tier.
    pub(crate) fn card(&self, tier: usize) -> Result<Card, String> {
        cards::check(&self.card_id(tier), &self.card_text(tier))
            .map(|checked| checked.card)
            .map_err(|rejected| rejected.errors.join("; "))
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_phrasebook__tests.rs"]
mod tests;
