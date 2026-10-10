//! The realm the party builds by playing: spoils carried out of the delve,
//! a shared treasury, and wishes that the treasury pays to raise in the wild.
//!
//! The harness's work grows the town (`hearth`); the party's adventures win
//! the wild. Nothing here is free: a wish stands in the overworld only once
//! spoils from real runs have paid for it, and its plaque names who wished and
//! who paid.
//!
//! A wish starts as plain words from anyone in the party (`Asked`). The host's
//! angelX drafts it as checked data — pixel art in realm inks, a price, a
//! place it stands near — in a `.wish` file (`Drafted`). When the treasury
//! covers the price, granting it pays and builds it (`Built`). Like cards,
//! wishes are data, never code.
//!
//! Persistence: `<island>.realm.json` beside the world's rewards file, so each
//! workspace's island keeps its own treasury, written atomically.

mod catalog;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(crate) const MAX_WISHES: usize = 64;
pub(crate) const MAX_WORDS: usize = 160;
pub(crate) const MAX_NAME: usize = 28;
/// Wish art is at most this many ink pixels on a side (two overworld tiles).
pub(crate) const ART: usize = 32;
pub(crate) const MAX_BYTES: usize = 8192;

/// What a delve yields. Each pack has its own; gold is everywhere; a bond is
/// earned only by clearing a room with two knights standing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Spoil {
    Gold,
    Bone,
    Wax,
    Ore,
    Gem,
    Ember,
    Scale,
    Bond,
}

impl Spoil {
    pub(crate) const ALL: [Spoil; 8] = [
        Spoil::Gold,
        Spoil::Bone,
        Spoil::Wax,
        Spoil::Ore,
        Spoil::Gem,
        Spoil::Ember,
        Spoil::Scale,
        Spoil::Bond,
    ];

    pub(crate) fn word(self) -> &'static str {
        match self {
            Spoil::Gold => "gold",
            Spoil::Bone => "bone",
            Spoil::Wax => "wax",
            Spoil::Ore => "ore",
            Spoil::Gem => "gem",
            Spoil::Ember => "ember",
            Spoil::Scale => "scale",
            Spoil::Bond => "bond",
        }
    }

    pub(crate) fn from_word(word: &str) -> Option<Spoil> {
        Spoil::ALL
            .into_iter()
            .find(|s| s.word() == word.trim_end_matches('s'))
    }

    /// Where it comes from, for a player deciding where to delve.
    pub(crate) fn source(self) -> &'static str {
        match self {
            Spoil::Gold => "anywhere",
            Spoil::Bone | Spoil::Wax => "the Crypt",
            Spoil::Ore | Spoil::Gem => "the Mines",
            Spoil::Ember => "Dragon Keep",
            Spoil::Scale => "slaying the dragon",
            Spoil::Bond => "clearing a room together",
        }
    }

    /// Worth in gold, for a wish's floor price.
    fn worth(self) -> u32 {
        match self {
            Spoil::Gold => 1,
            Spoil::Bone | Spoil::Wax | Spoil::Ore => 20,
            Spoil::Gem | Spoil::Ember => 60,
            Spoil::Bond => 80,
            Spoil::Scale => 600,
        }
    }
}

/// An amount of each spoil.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Spoils(pub(crate) BTreeMap<Spoil, u32>);

impl Spoils {
    pub(crate) fn get(&self, spoil: Spoil) -> u32 {
        self.0.get(&spoil).copied().unwrap_or(0)
    }

    pub(crate) fn add(&mut self, spoil: Spoil, n: u32) {
        if n > 0 {
            let slot = self.0.entry(spoil).or_insert(0);
            *slot = slot.saturating_add(n);
        }
    }

    pub(crate) fn merge(&mut self, other: &Spoils) {
        for (&spoil, &n) in &other.0 {
            self.add(spoil, n);
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.values().all(|&n| n == 0)
    }

    /// Half of each, rounded down: what a wiped party carries home.
    pub(crate) fn half(&self) -> Spoils {
        Spoils(
            self.0
                .iter()
                .map(|(&s, &n)| (s, n / 2))
                .filter(|&(_, n)| n > 0)
                .collect(),
        )
    }

    pub(crate) fn covers(&self, price: &Spoils) -> bool {
        price.0.iter().all(|(&s, &n)| self.get(s) >= n)
    }

    pub(crate) fn take(&mut self, price: &Spoils) {
        for (&s, &n) in &price.0 {
            if let Some(slot) = self.0.get_mut(&s) {
                *slot = slot.saturating_sub(n);
            }
        }
        self.0.retain(|_, n| *n > 0);
    }

    fn worth(&self) -> u32 {
        self.0.iter().map(|(&s, &n)| s.worth() * n).sum()
    }

    /// `12 bone · 3 wax · 150 gold`, in the order spoils are listed.
    pub(crate) fn label(&self) -> String {
        let parts: Vec<String> = Spoil::ALL
            .into_iter()
            .filter(|&s| self.get(s) > 0)
            .map(|s| format!("{} {}", self.get(s), s.word()))
            .collect();
        if parts.is_empty() {
            "nothing".into()
        } else {
            parts.join(" · ")
        }
    }

    /// `need 4 more bone · 1 scale`, or empty when covered.
    pub(crate) fn shortfall(&self, price: &Spoils) -> String {
        let mut missing = Spoils::default();
        for (&s, &n) in &price.0 {
            missing.add(s, n.saturating_sub(self.get(s)));
        }
        if missing.is_empty() {
            String::new()
        } else {
            format!("need {}", missing.label())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Status {
    /// Plain words, waiting for the host's angelX to draft it.
    Asked,
    /// Drafted with art and a price; waiting for the treasury.
    Drafted,
    /// Paid for and standing in the realm.
    Built,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Wish {
    pub(crate) id: String,
    pub(crate) name: String,
    /// The wish in the words of whoever made it.
    pub(crate) words: String,
    pub(crate) by: String,
    pub(crate) status: Status,
    #[serde(default)]
    pub(crate) price: Spoils,
    /// Ink rows, at most 32 by 32.
    #[serde(default)]
    pub(crate) art: Vec<String>,
    /// The overworld place it should stand near (`Place` name), if any.
    #[serde(default)]
    pub(crate) near: String,
    /// Who paid what, once built.
    #[serde(default)]
    pub(crate) paid: Vec<(String, Spoils)>,
    /// Built wishes keep the order they were raised in.
    #[serde(default)]
    pub(crate) raised: u32,
}

/// One knight's spoils banked at once, with why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Haul {
    pub(crate) hero: u32,
    pub(crate) spoils: Spoils,
    pub(crate) why: String,
}

/// The island's play economy.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Realm {
    pub(crate) treasury: Spoils,
    /// Everything each player ever banked, by name.
    #[serde(default)]
    pub(crate) brought: BTreeMap<String, Spoils>,
    #[serde(default)]
    pub(crate) wishes: Vec<Wish>,
    /// Floors cleared per delve pack name — the wild places reclaimed.
    #[serde(default)]
    pub(crate) reclaimed: BTreeMap<String, u32>,
    #[serde(default)]
    pub(crate) raids_won: u32,
    /// Delves that reached the bottom of the Unknown and found the Grail.
    #[serde(default)]
    pub(crate) grails: u32,
    /// The Undercroft as the party has built it out.
    #[serde(default)]
    pub(crate) home: crate::drive::together_shooter::home::Home,
    /// Host-local practice game; malformed practice data must never discard the
    /// surrounding realm's treasury, home or progression during load/save.
    #[serde(default, deserialize_with = "crate::drive::chivalry::deserialize_saved")]
    pub(crate) chivalry: crate::drive::chivalry::Chivalry,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Realm {
    /// The realm beside a world rewards file (`<key>.json` → `<key>.realm.json`).
    pub(crate) fn beside(rewards: Option<&Path>) -> Realm {
        let path = rewards.map(|p| p.with_extension("realm.json"));
        let mut realm = path
            .as_deref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice::<Realm>(&bytes).ok())
            .unwrap_or_default();
        realm.chivalry.normalize();
        realm.path = path;
        realm.offer_catalog();
        realm.publish();
        realm
    }

    pub(crate) fn save(&self) -> std::io::Result<()> {
        self.publish();
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("realm.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, path)
    }

    /// Put a knight's haul in the treasury under their name.
    pub(crate) fn bank(&mut self, name: &str, spoils: &Spoils) {
        self.treasury.merge(spoils);
        self.brought
            .entry(name.to_string())
            .or_default()
            .merge(spoils);
    }

    /// Plain words become an asked wish. Returns its id.
    pub(crate) fn ask(&mut self, by: &str, words: &str) -> Result<String, String> {
        let words: String = words
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_WORDS)
            .collect();
        let words = words.trim().to_string();
        if words.chars().filter(|c| c.is_alphanumeric()).count() < 3 {
            return Err("say what you wish for in a few words".into());
        }
        if self.wishes.len() >= MAX_WISHES {
            return Err(format!("the wishing stone holds {MAX_WISHES} wishes"));
        }
        let mut id = slug(&words);
        id.truncate(24);
        let id = id.trim_end_matches('-').to_string();
        let mut unique = id.clone();
        let mut n = 2;
        while self.wishes.iter().any(|w| w.id == unique) {
            unique = format!("{id}-{n}");
            n += 1;
        }
        self.wishes.push(Wish {
            id: unique.clone(),
            name: String::new(),
            words,
            by: by.chars().filter(|c| !c.is_control()).take(32).collect(),
            status: Status::Asked,
            price: Spoils::default(),
            art: Vec::new(),
            near: String::new(),
            paid: Vec::new(),
            raised: 0,
        });
        Ok(unique)
    }

    /// A checked draft fills in its wish (or a catalog wish adds itself).
    pub(crate) fn draft(&mut self, draft: Draft) -> Result<(), String> {
        let full = self.wishes.len() >= MAX_WISHES;
        match self.wishes.iter_mut().find(|w| w.id == draft.id) {
            Some(wish) if wish.status == Status::Built => {
                Err(format!("{} already stands in the realm", wish.name))
            }
            Some(wish) => {
                wish.name = draft.name;
                wish.price = draft.price;
                wish.art = draft.art;
                wish.near = draft.near;
                if !draft.by.is_empty() && wish.by.is_empty() {
                    wish.by = draft.by;
                }
                wish.status = Status::Drafted;
                Ok(())
            }
            None if !full => {
                self.wishes.push(Wish {
                    id: draft.id,
                    name: draft.name,
                    words: draft.words,
                    by: draft.by,
                    status: Status::Drafted,
                    price: draft.price,
                    art: draft.art,
                    near: draft.near,
                    paid: Vec::new(),
                    raised: 0,
                });
                Ok(())
            }
            None => Err(format!("the wishing stone holds {MAX_WISHES} wishes")),
        }
    }

    /// Offer the catalog's wishes the island does not have yet.
    pub(crate) fn offer_catalog(&mut self) {
        let earned: &[(&str, &str)] = if self.raids_won > 0 {
            catalog::AFTER_VICTORY
        } else {
            &[]
        };
        for (id, raw) in catalog::CATALOG.iter().chain(earned) {
            if self.wishes.iter().any(|w| w.id == *id) {
                continue;
            }
            let (draft, _) = check(id, raw).expect("catalog wishes pass their own checker");
            let _ = self.draft(draft);
        }
    }

    /// Pay for a drafted wish from the treasury and raise it.
    pub(crate) fn grant(&mut self, id: &str) -> Result<String, String> {
        let raised = self
            .wishes
            .iter()
            .filter(|w| w.status == Status::Built)
            .count() as u32
            + 1;
        let Some(wish) = self.wishes.iter_mut().find(|w| w.id == id) else {
            return Err(format!("no wish `{id}`"));
        };
        match wish.status {
            Status::Built => return Err(format!("{} already stands", wish.name)),
            Status::Asked => {
                return Err(format!(
                    "“{}” has no draft yet — angelX drafts it first",
                    wish.words
                ));
            }
            Status::Drafted => {}
        }
        if !self.treasury.covers(&wish.price) {
            return Err(format!(
                "{}: {}",
                wish.name,
                self.treasury.shortfall(&wish.price)
            ));
        }
        self.treasury.take(&wish.price);
        // Credit the price to whoever brought those spoils, in proportion.
        let mut paid = Vec::new();
        for (name, brought) in &self.brought {
            let mut share = Spoils::default();
            for (&s, &n) in &wish.price.0 {
                let total: u32 = self.brought.values().map(|b| b.get(s)).sum();
                if total > 0 {
                    share.add(s, (n * brought.get(s)).div_ceil(total).min(n));
                }
            }
            if !share.is_empty() {
                paid.push((name.clone(), share));
            }
        }
        wish.paid = paid;
        wish.status = Status::Built;
        wish.raised = raised;
        Ok(format!("{} rises in the realm", wish.name))
    }

    /// Wishes standing in the realm, for the overworld.
    pub(crate) fn built(&self) -> Vec<Wish> {
        let mut built: Vec<Wish> = self
            .wishes
            .iter()
            .filter(|w| w.status == Status::Built)
            .cloned()
            .collect();
        built.sort_by_key(|w| w.raised);
        built
    }

    /// The overworld reads built wishes from here, without file access.
    fn publish(&self) {
        let built = std::sync::Arc::new(self.built());
        if let Ok(mut slot) = standing().write() {
            *slot = built;
        }
    }
}

fn standing() -> &'static std::sync::RwLock<std::sync::Arc<Vec<Wish>>> {
    static STANDING: std::sync::OnceLock<std::sync::RwLock<std::sync::Arc<Vec<Wish>>>> =
        std::sync::OnceLock::new();
    STANDING.get_or_init(Default::default)
}

/// The wishes standing in the realm, as last loaded or saved.
pub(crate) fn standing_wishes() -> std::sync::Arc<Vec<Wish>> {
    standing().read().map(|w| w.clone()).unwrap_or_default()
}

fn slug(text: &str) -> String {
    crate::drive::together_shooter::cards::slug(text)
}

/// A checked `.wish` draft.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Draft {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) words: String,
    pub(crate) by: String,
    pub(crate) price: Spoils,
    pub(crate) art: Vec<String>,
    pub(crate) near: String,
}

/// Places a wish may stand near (the overworld's `Place` names).
pub(crate) const NEAR: &[&str] = &[
    "keep",
    "gatehouse",
    "rookery",
    "scriptorium",
    "smithy",
    "chapel",
    "round table",
    "observatory",
    "lists",
    "mines",
    "dragon keep",
    "dark forest",
    "swamp",
    "fields",
    "village",
];

/// Check a `.wish` file. `id` is the asked wish it drafts (its file name).
pub(crate) fn check(id: &str, raw: &str) -> Result<(Draft, Vec<String>), Vec<String>> {
    let mut errors = Vec::new();
    let mut notes = Vec::new();
    if raw.len() > MAX_BYTES {
        return Err(vec![format!("the file is over {MAX_BYTES} bytes")]);
    }
    let (mut name, mut words, mut by, mut near) =
        (String::new(), String::new(), String::new(), String::new());
    let mut price = Spoils::default();
    let script = crate::drive::together_shooter::script::read(raw);
    let art = script.art;
    let clean = crate::drive::together_shooter::script::clean;
    for line in &script.lines {
        let (n, word, rest) = (line.n, line.word.as_str(), line.rest);
        match word {
            "name" => name = clean(rest, MAX_NAME),
            "words" => words = clean(rest, MAX_WORDS),
            "by" => by = clean(rest, 32),
            "near" => {
                let place = rest.to_ascii_lowercase();
                if NEAR.contains(&place.as_str()) {
                    near = place;
                } else {
                    errors.push(format!(
                        "line {n}: near `{rest}` — use one of: {}",
                        NEAR.join(", ")
                    ));
                }
            }
            "price" => {
                for part in rest.split([',', '·']) {
                    let mut it = part.split_whitespace();
                    let (Some(a), Some(b)) = (it.next(), it.next()) else {
                        if !part.trim().is_empty() {
                            errors.push(format!(
                                "line {n}: `{}` — write prices like `ore 20, gem 2`",
                                part.trim()
                            ));
                        }
                        continue;
                    };
                    let (spoil, amount) = match (
                        Spoil::from_word(a),
                        b.parse::<u32>(),
                        Spoil::from_word(b),
                        a.parse::<u32>(),
                    ) {
                        (Some(s), Ok(v), _, _) | (_, _, Some(s), Ok(v)) => (s, v),
                        _ => {
                            errors.push(format!(
                                "line {n}: `{}` — spoils are {}",
                                part.trim(),
                                Spoil::ALL.map(Spoil::word).join(", ")
                            ));
                            continue;
                        }
                    };
                    price.add(spoil, amount.min(9999));
                }
            }
            other => errors.push(format!(
                "line {n}: unknown word `{other}`; a wish has name, words, by, near, price, art"
            )),
        }
    }
    if name.trim().is_empty() {
        errors.push("the wish needs a `name`".into());
    }
    if art.is_empty() {
        errors.push("the wish needs `art`: rows of realm inks, up to 32 by 32".into());
    } else if art.len() > ART || art.iter().any(|r| r.chars().count() > ART) {
        errors.push(format!("art is at most {ART} by {ART} inks"));
    }
    let inked: u32 = art
        .iter()
        .map(|r| r.chars().filter(|&c| c != '.').count() as u32)
        .sum();
    if !art.is_empty() && inked == 0 {
        errors.push("art has no inked pixel".into());
    }
    // A bigger landmark costs more: two gold of worth per inked pixel at least.
    let floor = inked * 2;
    if errors.is_empty() && price.worth() < floor {
        let add = floor - price.worth();
        price.add(Spoil::Gold, add);
        notes.push(format!(
            "price raised by {add} gold: a landmark this size is worth at least {floor}"
        ));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let id = if id.is_empty() { slug(&name) } else { slug(id) };
    Ok((
        Draft {
            id,
            name: name.trim().into(),
            words,
            by,
            price,
            art,
            near,
        },
        notes,
    ))
}

/// What angelX gets asked when the host grants an asked wish.
pub(crate) fn brief(wish: &Wish, path: &Path, treasury: &Spoils) -> String {
    format!(
        "Draft a wish for our angelX realm as a data file. Write ONLY the file {path} — no code changes.\n\n\
{by} wished, in their words: “{words}”\n\n\
Format (plain text, one field per line):\n\
name   <a name for the landmark, up to {MAX_NAME} chars>\n\
words  {words}\n\
by     {by}\n\
near   <one of: {near}>\n\
price  <spoils, e.g. `bone 12, wax 4, gold 200`>\n\
art\n\
<up to {ART} rows of up to {ART} characters; `.` is black paper>\n\n\
Art: a top-down pixel landmark in the style of a Zelda-1 overworld drawn on black paper — at most two 16px tiles across. Use only these inks, each group dark→light:\n\
greys k K Z X g j G J h i H · greens f D F E e l N m L A M y C Y · wood n b I B p P r R o O t T · stone s x S u U v V W q Q z · signal (glows; only for fire, light, gems) 0 1 2 3 w a 4 @ 5 6 $ c 9 8 7.\n\
No black inner outlines (they read as holes on black paper). Light from the top-left.\n\n\
Price: spoils are won only by playing the delve — gold anywhere, bone and wax in the Crypt, ore and gems in the Mines, embers in Dragon Keep, a dragon scale for slaying the dragon, and bonds for clearing rooms together. Price it like a meaningful goal: a small shrine ≈ a couple of runs, a great hall ≈ a dragon. The game raises any price below two gold per inked pixel. The treasury holds {treasury}.\n\n\
When the file is written, tell the party to run /dungeon wishes reload and then /dungeon grant {id}.",
        path = path.display(),
        by = if wish.by.is_empty() {
            "Someone"
        } else {
            &wish.by
        },
        words = wish.words,
        near = NEAR.join(", "),
        treasury = treasury.label(),
        id = wish.id,
    )
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_realm__tests.rs"]
mod tests;
