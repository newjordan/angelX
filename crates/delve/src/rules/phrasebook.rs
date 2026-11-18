//! The phrasebook: wishes the Delve knows by heart (`phrasebook.txt`).
//!
//! A player's words are matched to a known wish in microseconds, and the
//! wish is granted in the running game: no model, no wait. Granted again,
//! it climbs its ladder of up to five tiers, each a card the card checker
//! bounds; a learned ladder written to three or four climbs on to the fifth
//! by rule (`climb_on`), and the floor says how far a ladder may climb on
//! it (`overclass::top_tier`).
//!
//! A wish's words only match an entry whose phrases cover all of them (or
//! all but one of four or more): "golden spiky armor that explodes" is not
//! Iron Skin with extra words, it is a new thing, and goes to be learned.
//! An entry may dress its cards: `draw <slot> [tier]` blocks (`looks`),
//! from the tier named up, and `sheet` lines that the workspace's pictures
//! are cut into when the book loads (`together_sheets`).

use super::*;

pub const TEXT: &str = include_str!("../../../../cockpit/assets/dungeon/phrasebook.txt");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wish {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub says: Vec<Vec<String>>,
    pub tiers: Vec<(Vec<String>, String)>,
    pub art: Vec<String>,
    pub looks: Vec<(usize, String, String, bool)>,
    pub learned: bool,
    #[serde(default, skip_serializing_if = "none_derived")]
    pub derived: usize,
}

fn none_derived(n: &usize) -> bool {
    *n == 0
}

pub const LEARNED: &str = "## learned in this workspace";
pub const MAX_ID: usize = 24;
pub const MAX_TIERS: usize = 5;

struct Live {
    text: String,
    book: std::sync::Arc<Vec<Wish>>,
    errors: Vec<String>,
}

pub struct ScopedBook {
    text: String,
    book: std::sync::Arc<Vec<Wish>>,
    errors: Vec<String>,
}

struct ScopedState {
    next_token: u64,
    stack: Vec<(u64, std::sync::Arc<ScopedBook>)>,
}

impl ScopedState {
    fn token(&mut self) -> u64 {
        loop {
            let token = self.next_token;
            self.next_token = self.next_token.wrapping_add(1);
            if !self.stack.iter().any(|(live, _)| *live == token) {
                return token;
            }
        }
    }
}

thread_local! {
    static SCOPED: std::cell::RefCell<ScopedState> = std::cell::RefCell::new(ScopedState {
        next_token: 0,
        stack: Vec::new(),
    });
}

#[cfg(test)]
thread_local! {
    static PREPARATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub struct Scope {
    token: u64,
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        SCOPED.with(|state| {
            state.borrow_mut().stack.retain(|(token, _)| *token != self.token);
        });
    }
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

pub fn prepare_scoped(text: String, errors: Vec<String>) -> std::sync::Arc<ScopedBook> {
    #[cfg(test)]
    PREPARATIONS.with(|count| count.set(count.get() + 1));
    let (parsed, _rejected) = read_checked(&text);
    let book = std::sync::Arc::new(parsed);
    std::sync::Arc::new(ScopedBook { text, book, errors })
}

pub fn scoped(book: std::sync::Arc<ScopedBook>) -> Scope {
    SCOPED.with(|state| {
        let mut state = state.borrow_mut();
        let token = state.token();
        state.stack.push((token, book));
        Scope { token, _thread_bound: std::marker::PhantomData }
    })
}

pub fn book() -> std::sync::Arc<Vec<Wish>> {
    if let Some(book) = SCOPED.with(|state| {
        state.borrow().stack.last().map(|(_, context)| context.book.clone())
    }) {
        return book;
    }
    live()
        .read()
        .map_or_else(|_| std::sync::Arc::new(read(TEXT)), |l| l.book.clone())
}

pub fn snapshot() -> (String, Vec<String>) {
    if let Some(snapshot) = SCOPED.with(|state| {
        state.borrow().stack.last().map(|(_, context)| (context.text.clone(), context.errors.clone()))
    }) {
        return snapshot;
    }
    live().read().map_or_else(
        |_| (TEXT.to_owned(), Vec::new()),
        |l| (l.text.clone(), l.errors.clone()),
    )
}

pub fn errors() -> Vec<String> {
    if let Some(errors) = SCOPED.with(|state| {
        state.borrow().stack.last().map(|(_, context)| context.errors.clone())
    }) {
        return errors;
    }
    live()
        .read()
        .map_or_else(|_| Vec::new(), |l| l.errors.clone())
}

#[cfg(test)]
mod scoped_context_tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn prepared(id: &str, issue: &str) -> Arc<ScopedBook> {
        let text = format!("{LEARNED}\nwish {id} | {id}\nsay words for {id}\ntier damage 10 :: {id}\n");
        prepare_scoped(text, vec![issue.to_owned()])
    }

    #[test]
    fn phrasebook_scope_nests_restores_and_reuses_prepared_book() {
        let baseline = prepared("scope-baseline", "baseline diagnostic");
        let _baseline_scope = scoped(baseline.clone());
        let outer = prepared("scope-outer", "outer source diagnostic");
        let outer_book = outer.book.clone();
        {
            let _outer_scope = scoped(outer.clone());
            assert_eq!(snapshot(), (outer.text.clone(), outer.errors.clone()));
            assert_eq!(errors(), outer.errors);
            assert!(book().iter().any(|wish| wish.id == "scope-outer"));
            assert!(Arc::ptr_eq(&book(), &outer_book));
            let inner = prepared("scope-inner", "inner source diagnostic");
            let inner_book = inner.book.clone();
            {
                let _inner_scope = scoped(inner.clone());
                assert_eq!(snapshot(), (inner.text.clone(), inner.errors.clone()));
                assert_eq!(errors(), inner.errors);
                assert!(book().iter().any(|wish| wish.id == "scope-inner"));
                assert!(Arc::ptr_eq(&book(), &inner_book));
            }
            assert_eq!(snapshot(), (outer.text.clone(), outer.errors.clone()));
            assert!(Arc::ptr_eq(&book(), &outer_book));
        }
        assert_eq!(snapshot(), (baseline.text.clone(), baseline.errors.clone()));
        assert!(Arc::ptr_eq(&book(), &baseline.book));
    }

    #[test]
    fn phrasebook_out_of_order_drop_and_counter_wrap_keep_tokens_distinct() {
        SCOPED.with(|state| state.borrow_mut().next_token = u64::MAX);
        let baseline = prepared("scope-token-base", "baseline diagnostic");
        let base_scope = scoped(baseline.clone());
        let outer = prepared("scope-token-outer", "outer diagnostic");
        let outer_scope = scoped(outer);
        let inner = prepared("scope-token-inner", "inner diagnostic");
        let inner_scope = scoped(inner.clone());
        SCOPED.with(|state| state.borrow_mut().next_token = u64::MAX);
        let newer = prepared("scope-token-newer", "newer diagnostic");
        let newer_scope = scoped(newer.clone());
        let tokens: Vec<_> = SCOPED.with(|state| state.borrow().stack.iter().map(|(token, _)| *token).collect());
        assert_eq!(tokens, vec![u64::MAX, 0, 1, 2]);
        drop(outer_scope);
        assert_eq!(snapshot(), (newer.text.clone(), newer.errors.clone()));
        drop(newer_scope);
        assert_eq!(snapshot(), (inner.text.clone(), inner.errors.clone()));
        drop(inner_scope);
        assert_eq!(snapshot(), (baseline.text.clone(), baseline.errors.clone()));
        drop(base_scope);
    }

    #[test]
    fn phrasebook_scope_restores_after_unwind() {
        let baseline = prepared("scope-panic-base", "baseline diagnostic");
        let _baseline_scope = scoped(baseline.clone());
        let context = prepared("scope-panic", "captured diagnostic");
        let result = std::panic::catch_unwind(|| {
            let _scope = scoped(context.clone());
            assert_eq!(snapshot(), (context.text.clone(), context.errors.clone()));
            panic!("exercise phrasebook scope drop during unwind");
        });
        assert!(result.is_err());
        assert_eq!(snapshot(), (baseline.text.clone(), baseline.errors.clone()));
    }

    #[test]
    fn thread_contexts_are_isolated_and_cheap_scopes_do_not_reparse() {
        let baseline = prepared("scope-thread-base", "baseline diagnostic");
        let _baseline_scope = scoped(baseline.clone());
        let rendezvous = Arc::new(Barrier::new(3));
        let contexts = [prepared("scope-alpha", "alpha diagnostic"), prepared("scope-beta", "beta diagnostic")];
        let threads: Vec<_> = contexts.into_iter().map(|context| {
            let rendezvous = rendezvous.clone();
            std::thread::spawn(move || {
                let parsed = context.book.clone();
                let preparations = PREPARATIONS.with(std::cell::Cell::get);
                rendezvous.wait();
                for _ in 0..4 {
                    let _scope = scoped(context.clone());
                    assert_eq!(snapshot(), (context.text.clone(), context.errors.clone()));
                    assert_eq!(errors(), context.errors);
                    assert!(Arc::ptr_eq(&book(), &parsed));
                    assert!(book().iter().any(|wish| wish.id == context.book[0].id));
                    assert_eq!(PREPARATIONS.with(std::cell::Cell::get), preparations);
                }
                rendezvous.wait();
            })
        }).collect();
        rendezvous.wait();
        assert_eq!(snapshot(), (baseline.text.clone(), baseline.errors.clone()));
        assert!(Arc::ptr_eq(&book(), &baseline.book));
        rendezvous.wait();
        for thread in threads { thread.join().unwrap(); }
    }
}

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

pub fn read_checked(text: &str) -> (Vec<Wish>, Vec<String>) {
    let mut book: Vec<Wish> = Vec::new();
    let mut learned = false;
    let mut drawing = false;
    let mut sprite: Option<(usize, String, String, bool)> = None;
    let mut bad_lines: Vec<(usize, String)> = Vec::new();
    let mut wish_vfx: Vec<(usize, String)> = Vec::new();
    let keyword = |line: &str| {
        line.split_whitespace().next().is_some_and(|w| {
            matches!(
                w,
                "wish" | "kind" | "say" | "tier" | "art" | "draw" | "look" | "sheet" | "paint" | "vfx"
            )
        })
    };
    for raw in text.lines() {
        let line = raw.trim();
        if let Some((from, slot, mut block, own)) = sprite.take() {
            if !line.is_empty() && !keyword(line) && super::script::sprite_row(line) {
                block.push_str(line);
                block.push('\n');
                sprite = Some((from, slot, block, own));
                continue;
            }
            if let Some(wish) = book.last_mut() {
                wish.looks.push((from, slot, block, own));
            }
        }
        if drawing {
            if line.is_empty() || keyword(line) {
                drawing = false;
            } else {
                if let Some(wish) = book.last_mut() {
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
                    derived: 0,
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
                sprite = Some((from, slot, format!("{header}\n"), has_tier));
            }
            "look" => {
                if let Some(wish) = book.last_mut() {
                    wish.looks
                        .push((0, "look".into(), format!("look {rest}\n"), false));
                }
            }
            "vfx" => {
                if !book.is_empty() {
                    wish_vfx.push((book.len() - 1, format!("vfx {rest}")));
                }
            }
            _ => {
                if !book.is_empty() {
                    bad_lines.push((book.len() - 1, format!("unknown line `{word}`")));
                }
            }
        }
    }
    if let Some((from, slot, block, own)) = sprite
        && let Some(wish) = book.last_mut()
    {
        wish.looks.push((from, slot, block, own));
    }
    let is_vfx = |line: &String| line.split_whitespace().next() == Some("vfx");
    for (index, wish) in book.iter_mut().enumerate() {
        let said: Vec<&String> = wish_vfx.iter().filter(|(i, _)| *i == index).map(|(_, l)| l).collect();
        if said.is_empty() {
            continue;
        }
        for (lines, _) in &mut wish.tiers {
            if !lines.iter().any(is_vfx) {
                lines.extend(said.iter().map(|l| (*l).clone()));
            }
        }
    }
    let mut errors = Vec::new();
    let mut index = 0;
    book.retain_mut(|w| {
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
        let rude = [&w.id, &w.name]
            .into_iter()
            .chain(w.says.iter().flatten())
            .chain(w.tiers.iter().map(|(_, says)| says))
            .find_map(|s| super::cards::not_for_work(s));
        if let Some(word) = rude {
            errors.push(format!(
                "wish {}: `{word}` is not for this game: every wish stays safe for work (a rude ask is granted as a harmless, funny stand-in)",
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
        if let Some(why) = w.ladder_fault() {
            errors.push(format!("wish {} {why}", w.id));
            return false;
        }
        w.climb_on();
        if w.looks.iter().any(|(from, _, _, _)| *from >= w.tiers.len()) {
            errors.push(format!(
                "wish {}: a draw block starts past the last tier",
                w.id
            ));
            return false;
        }
        true
    });
    let mut kept: Vec<Wish> = Vec::new();
    for wish in book {
        match kept.iter().position(|k| k.id == wish.id) {
            Some(i) => kept[i] = wish,
            None => kept.push(wish),
        }
    }
    (kept, errors)
}

#[cfg(any(test, feature = "testing"))]
pub fn find(said: &str) -> Option<Wish> {
    find_for(said, &[])
}

pub fn find_for(said: &str, deck: &[String]) -> Option<Wish> {
    find_in(&book(), said, deck)
}

pub fn find_in(book: &[Wish], said: &str, deck: &[String]) -> Option<Wish> {
    let mut said = words(said);
    said.sort();
    said.dedup();
    if said.is_empty() {
        return None;
    }
    if let Some(wish) = upgrade_of(book, &said, deck) {
        return Some(wish.clone());
    }
    let mut best: Option<((usize, bool, usize), &Wish)> = None;
    for (i, wish) in book.iter().enumerate() {
        if wish.next_tier(deck).is_none() {
            continue;
        }
        let name = words(&wish.name);
        let named = !name.is_empty() && name.iter().all(|w| said.contains(w));
        let mut covered: Vec<&str> = wish
            .says
            .iter()
            .filter(|phrase| phrase.iter().all(|w| said.contains(w)))
            .flatten()
            .map(String::as_str)
            .chain(name.iter().filter(|_| named).map(String::as_str))
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

const UPGRADE: &[&str] = &[
    "upgrade", "upgraded", "improve", "improved", "increase", "increased", "enhance",
    "enhanced", "boost", "boosted", "buff", "buffed", "strengthen", "stronger", "bigger",
    "larger", "better", "more", "level", "tier", "next", "size", "power", "again", "up", "even",
];

fn upgrade_of<'a>(book: &'a [Wish], said: &[String], deck: &[String]) -> Option<&'a Wish> {
    if !said.iter().any(|w| UPGRADE.contains(&w.as_str())) {
        return None;
    }
    let tier = |w: &str| w.chars().all(|c| c.is_ascii_digit()) || matches!(w, "ii" | "iii" | "iv" | "v");
    let rest: Vec<&String> = said.iter().filter(|w| !UPGRADE.contains(&w.as_str()) && !tier(w)).collect();
    if rest.is_empty() {
        return None;
    }
    let mut best: Option<((usize, bool, bool, usize), &Wish)> = None;
    for (i, wish) in book.iter().enumerate() {
        let held = (0..wish.tiers.len()).any(|t| deck.contains(&wish.card_id(t)));
        if !held || wish.next_tier(deck).is_none() {
            continue;
        }
        let name = words(&wish.name);
        if !rest
            .iter()
            .all(|w| name.contains(w) || wish.says.iter().flatten().any(|s| s == *w))
        {
            continue;
        }
        let named = rest.iter().filter(|w| name.contains(w)).count();
        let phrase = wish.says.iter().any(|p| p.iter().all(|w| rest.contains(&w)));
        if named == 0 && !phrase {
            continue;
        }
        let key = (named, phrase, wish.learned, i);
        if best.as_ref().is_none_or(|(k, _)| key > *k) {
            best = Some((key, wish));
        }
    }
    best.map(|(_, wish)| wish)
}

const WISH_ART: &[(&str, &str)] = &[
    (
        "plain-frame",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/plain-frame.art"),
    ),
    ("pilgrims-arrow", include_str!("../../../../cockpit/assets/dungeon/wish-art/ultimate.art")),
    ("ladys-veil", include_str!("../../../../cockpit/assets/dungeon/wish-art/ultimate.art")),
    ("trebuchet", include_str!("../../../../cockpit/assets/dungeon/wish-art/ultimate.art")),
    ("bladewind", include_str!("../../../../cockpit/assets/dungeon/wish-art/ultimate.art")),
    ("grand-chord", include_str!("../../../../cockpit/assets/dungeon/wish-art/ultimate.art")),
    ("sealed-writs", include_str!("../../../../cockpit/assets/dungeon/wish-art/ultimate.art")),
    ("stillhour", include_str!("../../../../cockpit/assets/dungeon/wish-art/ultimate.art")),
    ("midas-touch", include_str!("../../../../cockpit/assets/dungeon/wish-art/ultimate.art")),
    ("assassinate", include_str!("../../../../cockpit/assets/dungeon/wish-art/ultimate.art")),
    ("phantasm", include_str!("../../../../cockpit/assets/dungeon/wish-art/ultimate.art")),
    ("axe-storm", include_str!("../../../../cockpit/assets/dungeon/wish-art/ultimate.art")),
    (
        "great-helm",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/great-helm.art"),
    ),
    (
        "breastplate",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/breastplate.art"),
    ),
    (
        "buckler",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/buckler.art"),
    ),
    (
        "gauntlet",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/gauntlet.art"),
    ),
    (
        "boots",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/boots.art"),
    ),
    (
        "yew-bow",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/yew-bow.art"),
    ),
    (
        "plain-hound",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/plain-hound.art"),
    ),
    (
        "war-crossbow",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/war-crossbow.art"),
    ),
    (
        "hand-cannon",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/hand-cannon.art"),
    ),
    (
        "bright-blade",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/bright-blade.art"),
    ),
    (
        "firebolt",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/firebolt.art"),
    ),
    (
        "spark-ring",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/spark-ring.art"),
    ),
    (
        "mending-light",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/mending-light.art"),
    ),
    (
        "falling-star",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/falling-star.art"),
    ),
    (
        "fork",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/fork.art"),
    ),
    (
        "fart",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/fart.art"),
    ),
    (
        "frost-fletching",
        include_str!("../../../../cockpit/assets/dungeon/wish-art/frost-fletching.art"),
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

pub fn get(id: &str) -> Option<Wish> {
    book().iter().find(|w| w.id == id).cloned()
}

impl Wish {
    pub fn definition_file(&self) -> String {
        let mut text = String::new();
        if self.learned {
            text.push_str(LEARNED);
            text.push('\n');
        }
        text.push_str(&format!(
            "wish {} | {}\nkind {}\n",
            self.id, self.name, self.kind
        ));
        for say in &self.says {
            text.push_str(&format!("say {}\n", say.join(" ")));
        }
        for (lines, says) in &self.tiers[..self.tiers.len().saturating_sub(self.derived)] {
            text.push_str(&format!("tier {} :: {says}\n", lines.join("; ")));
        }
        if !self.art.is_empty() {
            text.push_str("art\n");
            text.push_str(&self.art.join("\n"));
            text.push_str("\n\n");
        }
        for (from, _, block, own) in &self.looks {
            if *own && let Some((header, rows)) = block.split_once('\n') {
                text.push_str(&format!("{header} {}\n{rows}", from + 1));
            } else {
                text.push_str(block);
            }
            if !text.ends_with('\n') {
                text.push('\n');
            }
            text.push('\n');
        }
        text
    }

    pub fn card_id(&self, tier: usize) -> String {
        format!("wish-{}-{}", self.id, tier + 1)
    }

    pub fn next_tier(&self, deck: &[String]) -> Option<usize> {
        let held = (0..self.tiers.len())
            .rev()
            .find(|&t| deck.contains(&self.card_id(t)));
        let next = held.map_or(0, |t| t + 1);
        (next < self.tiers.len()).then_some(next)
    }

    pub fn grade(&self) -> Option<(artistry::Grade, u32)> {
        if !self.learned {
            return None;
        }
        let pictures = self.drawn();
        Some(artistry::grade(pictures.iter().map(|(_, p)| p.as_slice())))
    }

    fn drawn(&self) -> Vec<(String, Vec<String>)> {
        let mut pictures: Vec<(String, Vec<String>)> = Vec::new();
        if !self.art.is_empty() {
            pictures.push(("its art".into(), self.art.clone()));
        }
        for (from, slot, block, _) in &self.looks {
            let mut lines = block.lines();
            let head = lines.next().unwrap_or("");
            if !head.starts_with("draw") || head.contains(" paint=") {
                continue;
            }
            let frame: Vec<String> = lines
                .map(|l| l.split('|').next().unwrap_or("").to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if !frame.is_empty() {
                let what = if *from > 0 { format!("its {slot} look from tier {}", from + 1) } else { format!("its {slot} look") };
                pictures.push((what, super::looks::realm_rows(&frame, &super::looks::header_pal(head))));
            }
        }
        pictures
    }

    pub fn pictures(&self) -> Vec<(String, Vec<String>)> {
        let mut pictures = self.drawn();
        if self.art.is_empty()
            && let Some(block) = art_for(&self.id)
        {
            let rows: Vec<String> = block.lines().skip(1).map(str::to_string).collect();
            pictures.insert(0, ("its card art (wish-art)".into(), rows));
        }
        pictures
    }

    pub fn card_text(&self, tier: usize) -> String {
        self.text_at(tier, self.grade().map(|(grade, _)| grade))
    }

    fn text_at(&self, tier: usize, grade: Option<artistry::Grade>) -> String {
        let (effects, says) = &self.tiers[tier];
        let graded: Vec<String> = match grade {
            Some(grade) => effects.iter().map(|e| graded_line(e, grade)).collect(),
            None => effects.clone(),
        };
        let effects = &graded;
        let rarity = rarity_of(tier, self.tiers.len());
        let numeral = ["I", "II", "III", "IV", "V"]
            .get(tier)
            .copied()
            .unwrap_or("+");
        let step = fidelity::Step::of(tier, self.tiers.len());
        let art = if self.art.is_empty() {
            art_for(&self.id).unwrap_or_default()
        } else {
            format!("art\n{}", self.art.join("\n"))
        };
        let art = fidelity::block(&art, step);
        let mut looks: Vec<(&str, usize, &str, bool)> = Vec::new();
        for (from, slot, block, own) in &self.looks {
            if *from > tier {
                continue;
            }
            match looks.iter_mut().find(|(s, _, _, _)| s == slot) {
                Some(look) if look.1 <= *from => *look = (slot, *from, block, *own),
                Some(_) => {}
                None => looks.push((slot, *from, block, *own)),
            }
        }
        let painted = |block: &str| block.lines().next().is_some_and(|h| h.split_whitespace().any(|w| w.starts_with("paint=")));
        let later = |slot: &str| {
            self.looks
                .iter()
                .filter(|(f, s, b, _)| *f > tier && s == slot && painted(b))
                .min_by_key(|(f, ..)| *f)
        };
        let mut looks: Vec<(&str, usize, &str, bool, bool)> = looks
            .into_iter()
            .map(|(slot, from, block, own)| match later(slot).filter(|_| !painted(block)) {
                Some((_, _, b, _)) => (slot, from, b.as_str(), own, true),
                None => (slot, from, block, own, false),
            })
            .collect();
        for (_, slot, _, _) in &self.looks {
            if !looks.iter().any(|(s, ..)| s == slot)
                && let Some((from, _, b, _)) = later(slot)
            {
                looks.push((slot, *from, b, false, true));
            }
        }
        let looks: String = looks
            .iter()
            .map(|(_, from, block, own, borrowed)| {
                let drawn = (*from > 0 || *own) && !borrowed;
                fidelity::block(block, if drawn { fidelity::Step::Drawn } else { step })
            })
            .collect();
        format!(
            "name {} {numeral}\nkind {}\nrarity {rarity}\ntext {says}\ndrop 0\nchest 0\nby the phrasebook\n{}\n{looks}\n{art}",
            self.name,
            self.kind,
            effects.join("\n"),
        )
    }

    pub fn card_graded(&self, tier: usize, grade: artistry::Grade) -> Result<Card, String> {
        if tier >= self.tiers.len() {
            return Err("no such wish tier".into());
        }
        cards::check(&self.card_id(tier), &self.text_at(tier, Some(grade)))
            .map(|checked| checked.card)
            .map_err(|rejected| rejected.errors.join("; "))
    }

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

fn worth(card: &Card) -> Vec<(String, u32)> {
    use cards::Effect;
    let mut worth: Vec<(String, u32)> = card
        .effects
        .iter()
        .map(|e| match *e {
            Effect::Cast(shape) => (format!("cast {}", shape.word()), 1),
            Effect::Area(area) => (format!("cast {} {}", area.form.word(), area.element.word()), u32::from(area.size)),
            Effect::Summon(n, behaviour) => (format!("summon {}", behaviour.word()), n),
            Effect::Volley(n) => ("volley".into(), 100u32.saturating_sub(n)),
            _ => {
                let (word, v) = e.parts();
                (word.to_string(), cards::wish_most(word).map_or(v, |most| v.min(most)))
            }
        })
        .collect();
    if card.kind == cards::Kind::Spell {
        worth.push(("cooldown".into(), cards::SPELL_COOLDOWN.1 + 1 - card.cooldown));
    }
    match card.guard {
        Some(cards::Guard::Wall(wall)) => worth.extend([
            ("wall width".into(), wall.width),
            ("wall empower".into(), wall.empower),
            ("wall drain".into(), cards::Wall::DRAIN.1 + 1 - wall.drain),
        ]),
        Some(guard) => worth.push((format!("guard {guard:?}"), 1)),
        None => {}
    }
    if let Some(arm) = &card.arm {
        worth.push((format!("arm {arm:?}"), 1));
    }
    if let Some(ult) = card.ult {
        worth.push((format!("ult {ult:?}"), 1));
    }
    worth
}

impl Wish {
    fn ladder_fault(&self) -> Option<String> {
        let mut below: Option<Vec<(String, u32)>> = None;
        for tier in 0..self.tiers.len() {
            match self.step(tier, below.as_deref()) {
                Ok(worth) => below = Some(worth),
                Err(why) => return why,
            }
        }
        None
    }

    fn step(&self, tier: usize, below: Option<&[(String, u32)]>) -> Result<Vec<(String, u32)>, Option<String>> {
        let checked = cards::check(&self.card_id(tier), &self.text_at(tier, None)).map_err(|_| None)?;
        let card = &checked.card;
        if card.kind == cards::Kind::Hold {
            for effect in &card.effects {
                let (word, v) = effect.parts();
                if let Some(most) = cards::wish_most(word)
                    && v > most
                {
                    return Err(Some(format!(
                        "tier {}: {word} {v} is more than wishes ever give ({word} {most} at most, from all of a knight's wishes together)",
                        tier + 1
                    )));
                }
            }
        }
        let worth = worth(card);
        if let Some(below) = below
            && !worth
                .iter()
                .any(|(k, v)| below.iter().find(|(b, _)| b == k).is_none_or(|(_, b)| v > b))
        {
            let notes = if checked.notes.is_empty() {
                String::new()
            } else {
                format!(" ({})", checked.notes.join("; "))
            };
            return Err(Some(format!(
                "tier {}: gives nothing more than tier {} once the game's limits apply{notes}; each tier must give more than the one below",
                tier + 1,
                tier
            )));
        }
        Ok(worth)
    }

    pub fn written(&self) -> Wish {
        let mut wish = self.clone();
        wish.tiers.truncate(self.tiers.len().saturating_sub(self.derived));
        wish.derived = 0;
        wish
    }

    fn climb_on(&mut self) {
        if !self.learned || self.derived > 0 || !(3..MAX_TIERS).contains(&self.tiers.len()) {
            return;
        }
        let Ok(mut below) = self.step(self.tiers.len() - 1, None) else {
            return;
        };
        while self.tiers.len() < MAX_TIERS {
            let tier = self.tiers.len();
            let top = self.tiers[tier - 1].0.clone();
            let mut climbed = None;
            for (lines, what) in climbs(&self.kind, &top) {
                self.tiers.push((lines, format!("{}: {what}.", CLIMBED[tier - 3])));
                if self.card(tier).is_ok()
                    && let Ok(worth) = self.step(tier, Some(&below))
                {
                    climbed = Some(worth);
                    break;
                }
                self.tiers.pop();
            }
            let Some(worth) = climbed else {
                break;
            };
            below = worth;
            self.derived += 1;
        }
    }
}

fn rarity_of(tier: usize, tiers: usize) -> &'static str {
    match tier {
        0 => "common",
        t if t + 1 >= tiers || t >= 3 => "relic",
        _ => "rare",
    }
}

const CLIMBED: [&str; MAX_TIERS - 3] = ["Mighty", "The full gift"];

const EXTRAS: [(&str, u32, &str); 3] = [
    ("damage", 8, "it hits harder"),
    ("velocity", 20, "the shots strike sooner"),
    ("max_hp", 10, "more health"),
];

fn climbs(kind: &str, below: &[String]) -> Vec<(Vec<String>, &'static str)> {
    let mut out = Vec::new();
    for (i, line) in below.iter().enumerate() {
        let Some((raised, what)) = step_up(kind, line) else {
            continue;
        };
        let mut lines = below.to_vec();
        lines[i] = raised;
        if let ["summon", n, behaviour, ..] = lines[i].split_whitespace().collect::<Vec<_>>().as_slice()
            && let (Ok(n), Some(behaviour)) = (n.parse::<u32>(), imps::Behaviour::parse(behaviour))
        {
            let floor = (n * behaviour.seconds_each()).min(cards::SPELL_COOLDOWN.1);
            for line in lines.iter_mut() {
                if let Some(Ok(cooldown)) = line.strip_prefix("cooldown ").map(|n| n.trim().parse::<u32>()) {
                    *line = format!("cooldown {}", cooldown.max(floor));
                }
            }
        }
        out.push((lines, what));
    }
    if kind == "hold" {
        for (word, n, what) in EXTRAS {
            if !below.iter().any(|l| l.split_whitespace().next() == Some(word)) {
                let mut lines = below.to_vec();
                lines.push(format!("{word} {n}"));
                out.push((lines, what));
            }
        }
    }
    out
}

fn step_up(kind: &str, line: &str) -> Option<(String, &'static str)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    match parts.as_slice() {
        ["cooldown", n] if kind == "spell" => {
            let n: u32 = n.parse().ok()?;
            (n > cards::SPELL_COOLDOWN.0).then(|| (format!("cooldown {}", n - (n / 6).max(1)), "it comes sooner"))
        }
        ["summon", n, behaviour, rest @ ..] => {
            let n: u32 = n.parse().ok()?;
            let most = imps::Behaviour::parse(behaviour)?.most();
            let rest = rest.iter().map(|w| format!(" {w}")).collect::<String>();
            (n < most).then(|| (format!("summon {} {behaviour}{rest}", n + 1), "one more answers"))
        }
        ["guard", "wall", params @ ..] => {
            let mut wall: Vec<(String, u32)> = params
                .iter()
                .map(|p| {
                    let (k, v) = p.split_once('=')?;
                    Some((k.to_string(), v.parse().ok()?))
                })
                .collect::<Option<_>>()?;
            let raise = |wall: &mut Vec<(String, u32)>, key: &str, most: u32, by: u32| {
                let v = wall.iter_mut().find(|(k, _)| k == key)?;
                (v.1 < most).then(|| v.1 = (v.1 + by).min(most))
            };
            let what = if raise(&mut wall, "width", cards::Wall::WIDTH.1, 1).is_some() {
                "a wider wall"
            } else if raise(&mut wall, "empower", cards::Wall::EMPOWER.1, 5).is_some() {
                "a kinder wall"
            } else {
                return None;
            };
            let params: Vec<String> = wall.iter().map(|(k, v)| format!("{k}={v}")).collect();
            Some((format!("guard wall {}", params.join(" ")), what))
        }
        [rune @ ("melee" | "throw" | "bolt"), params @ ..] if kind == "arm" => {
            let most = match *rune {
                "melee" => 60,
                "throw" => 50,
                _ => 100,
            };
            let mut raised = false;
            let params: Vec<String> = params
                .iter()
                .map(|p| match p.split_once('=') {
                    Some(("damage", v)) => match v.parse::<u32>() {
                        Ok(v) if v < most => {
                            raised = true;
                            format!("damage={}", (v + (v / 7).max(2)).min(most))
                        }
                        _ => p.to_string(),
                    },
                    _ => p.to_string(),
                })
                .collect();
            raised.then(|| (format!("{rune} {}", params.join(" ")), "it strikes deeper"))
        }
        [word, n] => {
            let (low, high) = cards::range(word)?;
            let v: u32 = n.trim_start_matches('+').trim_end_matches(['%', 's']).parse().ok()?;
            if *word == "volley" {
                return (v > low).then(|| (format!("volley {}", v - 1), "three wide oftener"));
            }
            let most = cards::wish_most(word).unwrap_or(high).min(high);
            if v >= most {
                return None;
            }
            let by = if most <= 3 { 1 } else { (most / 5).max(1) };
            let what = match *word {
                "damage" => "it hits harder",
                "rate" => "you shoot and swing sooner",
                "speed" => "longer strides",
                "velocity" => "the shots strike sooner",
                "max_hp" => "more health",
                "armor" => "more turned aside",
                "knock" => "a harder shove",
                "size" => "bigger still",
                "heal" => "it mends more",
                _ => "more of it",
            };
            Some((format!("{word} {}", (v + by).min(most)), what))
        }
        _ => None,
    }
}

fn graded_line(line: &str, grade: artistry::Grade) -> String {
    let parts: Vec<&str> = line.split_whitespace().collect();
    match parts.as_slice() {
        ["cooldown", n] => match n.parse::<u32>() {
            Ok(v) => {
                let (lo, hi) = cards::SPELL_COOLDOWN;
                format!("cooldown {}", (v * 100).div_ceil(grade.power()).clamp(lo, hi))
            }
            Err(_) => line.to_string(),
        },
        ["summon", n, rest @ ..] => match n.parse::<u32>() {
            Ok(v) => {
                let v = artistry::graded(v, grade).min(cards::range("summon").map_or(v, |r| r.1));
                std::iter::once(format!("summon {v}"))
                    .chain(rest.iter().map(|w| w.to_string()))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            Err(_) => line.to_string(),
        },
        [word, n] => match (cards::range(word), n.parse::<u32>()) {
            (Some((lo, hi)), Ok(v)) if hi > 0 => {
                format!("{word} {}", artistry::graded(v, grade).clamp(lo, hi))
            }
            _ => line.to_string(),
        },
        _ => line.to_string(),
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_phrasebook__tests.rs"]
mod tests;
