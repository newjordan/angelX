//! Salvage: pieces of old dwarf-work hauled up from the deep, each wished by
//! its finder into whatever they want, until a whole set is in and the
//! model reads them all and makes something new of them (the operator,
//! 2026-10-28: "a ship has a bunch of parts that can be found, and are
//! varying tiers (scrap yard style) and when a player finds a piece, they
//! make a wish and change the piece how they want. then eventually all the
//! pieces are combined and the AI will assess, read all the descriptions
//! and create an original new larger ship for them ... ad infinitum"; "its
//! not a ship until all the pieces are assembled, and the prow digs").
//!
//! - **Finding.** From Dragon Keep down, a guardian's fall always gives up
//!   a piece and a fight room won now and then does, better the deeper
//!   ([`grade_of`]). The knight who has found least takes it, and the
//!   piece is theirs to wish.
//! - **Wishing.** The finder's next words on the scroll go to the piece
//!   (`Run::wish_on_piece`), free: no held wish is spent. The words are
//!   the piece now.
//! - **Assembling.** Build `n` needs the four core places (the prow digs)
//!   and more pieces each time ([`needs`]), so every vessel is bigger than
//!   the last. Until then it is a yard of scrap and nothing names it.
//! - **The making.** At home, the host asks the model for her ([`brief`]),
//!   checks the answer ([`check`]), and builds her (`Run::build_vessel`);
//!   with no model, or no good answer, [`fallback`] makes her from the
//!   pieces alone. She is data: a name, a description, traits and quirks
//!   from a checked menu, and her own voice.
//!
//! The yard is the realm's (`Home::salvage`), and every change bumps its
//! `rev`, so the host and the run keep whichever saw it last.

use super::*;

/// Floors whose guardians and rooms give up salvage, shallowest first: the
/// deeper, the better the pieces (`grade_of`).
pub const SALVAGE_PACKS: [Pack; 4] = [Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown];
/// One won fight room in this many gives up a piece.
pub const ROOM_EVERY: usize = 4;
/// The longest a wish on a piece may be, in characters.
pub const WISH_CHARS: usize = 140;

/// A piece's tier, scrap-yard style, worst first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Grade {
    #[default]
    Scrap,
    Salvage,
    Sound,
    Fine,
    Relic,
}

impl Grade {
    pub const ALL: [Grade; 5] = [Grade::Scrap, Grade::Salvage, Grade::Sound, Grade::Fine, Grade::Relic];

    pub fn word(self) -> &'static str {
        match self {
            Grade::Scrap => "scrap",
            Grade::Salvage => "salvage",
            Grade::Sound => "sound",
            Grade::Fine => "fine",
            Grade::Relic => "relic",
        }
    }

    /// What the piece adds to the vessel's size.
    pub fn worth(self) -> u32 {
        match self {
            Grade::Scrap => 1,
            Grade::Salvage => 2,
            Grade::Sound => 3,
            Grade::Fine => 5,
            Grade::Relic => 8,
        }
    }
}

/// The grade a find rolls: the floor's depth among the salvage floors
/// (`rank`, 0 for Dragon Keep), a guardian's fall or a room's, and the
/// dice (`draw`, 0..100).
pub fn grade_of(rank: u32, guardian: bool, draw: u32) -> Grade {
    let score = rank * 12 + if guardian { 25 } else { 0 } + draw.min(99);
    match score {
        0..55 => Grade::Scrap,
        55..85 => Grade::Salvage,
        85..110 => Grade::Sound,
        110..130 => Grade::Fine,
        _ => Grade::Relic,
    }
}

/// Where a piece goes on the whole. The first four are the core every
/// build needs; the prow is what digs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Place {
    Prow,
    Hull,
    Heart,
    Treads,
    Tower,
    Turret,
    Hold,
    Stack,
    Crest,
    Lamps,
}

impl Place {
    pub const ALL: [Place; 10] = [
        Place::Prow,
        Place::Hull,
        Place::Heart,
        Place::Treads,
        Place::Tower,
        Place::Turret,
        Place::Hold,
        Place::Stack,
        Place::Crest,
        Place::Lamps,
    ];
    pub const CORE: [Place; 4] = [Place::Prow, Place::Hull, Place::Heart, Place::Treads];

    pub fn word(self) -> &'static str {
        match self {
            Place::Prow => "prow",
            Place::Hull => "hull",
            Place::Heart => "heart",
            Place::Treads => "treads",
            Place::Tower => "tower",
            Place::Turret => "turret",
            Place::Hold => "hold",
            Place::Stack => "stack",
            Place::Crest => "crest",
            Place::Lamps => "lamps",
        }
    }

    /// What the piece is for, in the shipwrights' brief.
    pub fn role(self) -> &'static str {
        match self {
            Place::Prow => "the prow: it digs; what she can bore through",
            Place::Hull => "the hull: how much she can take",
            Place::Heart => "the heart: her engine and her speed",
            Place::Treads => "the treads: how she lands and rams",
            Place::Tower => "the tower: what she sees ahead",
            Place::Turret => "the turret: what she fights with",
            Place::Hold => "the hold: what she carries",
            Place::Stack => "the stack: her heat, her smoke, her sound",
            Place::Crest => "the crest: her face",
            Place::Lamps => "the lamps: her light in the dark",
        }
    }

    /// What the piece looks like when it is found, by grade: old dwarf-work
    /// that does not say what it was part of.
    pub fn found_as(self, grade: Grade) -> &'static str {
        const FOUND: [[&str; 5]; 10] = [
            ["a bent auger bit", "a chipped boar-tusk cutter", "a boar-tusk cutter", "a polished auger crown", "the Tusk of the Deep King"],
            ["a rust-eaten iron plate", "a dented keel-rib", "a riveted hull plate", "a brass-bound bulkhead", "the Barrow-Keel of Old Hollow"],
            ["a cracked firebox", "a sooty little boiler", "a forge-heart, still warm", "a twin-chambered forge-heart", "the Ember-Heart that never cooled"],
            ["a few loose track links", "a buckled drive wheel", "a run of track links", "a geared drive wheel", "the Mountain-Walker's treads"],
            ["a cloudy porthole glass", "a bent periscope tube", "a brass periscope", "a lensed conning hood", "the All-Seeing Eye of Caer Dwfn"],
            ["a seized bolt-thrower", "a rusty tusk-gun", "a working bolt-thrower", "a rune-sighted tusk-gun", "the Stormcaller's turret"],
            ["a split cask-rack", "a dented sea-chest", "an iron-bound strongbox", "a dwarven vault-hold", "the Hoard-Hold of King Bryn"],
            ["a crushed chimney pipe", "a soot-black smokestack", "a fluted smokestack", "a bellows-stack with a whistle", "the Dragon's Throat"],
            ["a snapped figurehead", "a pitted banner-iron", "a carved boar figurehead", "a gilded crest-iron", "the Crowned Boar of the First Kings"],
            ["a cracked ember lamp", "a dim miner's lantern", "a pair of ember lamps", "a lantern-eye in brass", "the Lamps of the Lost Hold"],
        ];
        FOUND[self as usize][grade as usize]
    }
}

/// One piece in the yard.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Piece {
    pub id: u32,
    pub place: Option<Place>,
    pub grade: Grade,
    /// What it looked like when it was found.
    pub found: String,
    /// The finder's wish for it: what it is now. None until they wish.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wish: Option<String>,
    /// The finder, by their key (`talents::knight_key`).
    pub by: String,
    /// The pack of the floor it came up from.
    #[serde(default)]
    pub from: Pack,
}

impl Piece {
    pub fn place(&self) -> Place {
        self.place.unwrap_or(Place::Hull)
    }

    /// The piece as the yard tells it: what it is now, and what it was.
    pub fn says(&self) -> String {
        match &self.wish {
            Some(wish) => format!("{} (once {})", wish, self.found),
            None => self.found.clone(),
        }
    }
}

/// A vessel the yard made: what the model (or the fallback) said of her,
/// and the pieces she is made of.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vessel {
    /// Which build she is: 1, 2, 3 and on.
    pub n: u32,
    pub name: String,
    pub says: String,
    pub traits: Vec<String>,
    pub quirks: Vec<String>,
    /// Her lines, one for each of [`MOMENTS`].
    pub voice: Vec<(String, String)>,
    pub pieces: Vec<Piece>,
    /// The model made her (or the fallback did).
    #[serde(default)]
    pub by_model: bool,
    /// The party has seen her (`Run::reveal_vessel`).
    #[serde(default)]
    pub revealed: bool,
}

impl Vessel {
    /// Her size: what her pieces are worth together.
    pub fn size(&self) -> u32 {
        self.pieces.iter().map(|p| p.grade.worth()).sum()
    }

    /// Her line for `moment`, if she has one.
    pub fn line(&self, moment: &str) -> Option<&str> {
        self.voice.iter().find(|(m, _)| m == moment).map(|(_, l)| l.as_str())
    }
}

/// The realm's yard: pieces waiting, and every vessel made from them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Salvage {
    #[serde(default)]
    pub pieces: Vec<Piece>,
    #[serde(default)]
    pub vessels: Vec<Vessel>,
    #[serde(default)]
    pub next: u32,
    /// Bumped on every change: the newer yard wins a merge.
    #[serde(default)]
    pub rev: u64,
}

/// The size class of build `n`: each one bigger, ad infinitum.
pub fn class(n: u32) -> String {
    const CLASSES: [&str; 6] = ["bore-skiff", "bore-cutter", "bore-barque", "bore-galleon", "dreadbore", "leviathan"];
    let n = n.max(1) as usize;
    if n <= CLASSES.len() {
        CLASSES[n - 1].to_string()
    } else {
        format!("leviathan of the {} rank", ordinal(n - CLASSES.len() + 1))
    }
}

fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// What build `n` needs: the places it must have (the core four, then two
/// more each build until all ten), and how many pieces in all.
pub fn needs(n: u32) -> (Vec<Place>, usize) {
    let n = n.max(1) as usize;
    let places = (Place::CORE.len() + 2 * (n - 1)).min(Place::ALL.len());
    (Place::ALL[..places].to_vec(), Place::CORE.len() + 2 * (n - 1))
}

/// The pieces a build would be made of, chosen from the yard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub n: u32,
    pub pieces: Vec<Piece>,
}

impl Salvage {
    /// Nothing ever found: the realm writes no yard.
    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty() && self.vessels.is_empty() && self.next == 0 && self.rev == 0
    }

    /// The next build's number.
    pub fn next_build(&self) -> u32 {
        self.vessels.iter().map(|v| v.n).max().unwrap_or(0) + 1
    }

    /// The vessel the realm sails now: the newest.
    pub fn flagship(&self) -> Option<&Vessel> {
        self.vessels.last()
    }

    /// The oldest piece knight `key` found and has not wished yet.
    pub fn awaiting(&self, key: &str) -> Option<&Piece> {
        self.pieces.iter().find(|p| p.by == key && p.wish.is_none())
    }

    /// A piece found: into the yard, its number back.
    pub fn add(&mut self, place: Place, grade: Grade, by: &str, from: Pack) -> u32 {
        let id = self.next;
        self.next += 1;
        self.pieces.push(Piece {
            id,
            place: Some(place),
            grade,
            found: place.found_as(grade).to_string(),
            wish: None,
            by: by.to_string(),
            from,
        });
        self.rev += 1;
        id
    }

    /// The next build, if the yard holds enough: for each place it needs,
    /// the best piece there (the higher grade, a wished one first, then the
    /// oldest); then the best of the rest until the count is met.
    pub fn plan(&self) -> Option<Plan> {
        let n = self.next_build();
        let (places, count) = needs(n);
        let rank = |p: &Piece| (std::cmp::Reverse(p.grade), p.wish.is_none(), p.id);
        let mut pool: Vec<&Piece> = self.pieces.iter().collect();
        pool.sort_by_key(|p| rank(p));
        let mut chosen: Vec<Piece> = Vec::new();
        for place in places {
            let best = pool.iter().position(|p| p.place() == place)?;
            chosen.push(pool.remove(best).clone());
        }
        while chosen.len() < count {
            if pool.is_empty() {
                return None;
            }
            chosen.push(pool.remove(0).clone());
        }
        Some(Plan { n, pieces: chosen })
    }
}

/// Traits a vessel may have, from what was wished.
pub const TRAITS: [&str; 16] = [
    "holy", "infernal", "frozen", "toxic", "drunk", "gluttonous", "greedy", "vain", "musical", "rotten", "regal",
    "feral", "explosive", "sweet", "arcane", "ancient",
];

/// Quirks: each a rule her trait earns (`quirk`).
pub const QUIRKS: [(&str, &str, &str); 16] = [
    ("sanctified", "holy", "her docks mend the knights who stand in them"),
    ("hot-blooded", "infernal", "she bores a quarter faster and her heat climbs"),
    ("frostbound", "frozen", "she cuts magma cold but is slow to start"),
    ("plaguey", "toxic", "she leaves a poison wake behind her"),
    ("lush", "drunk", "the pilot's steering wobbles; embers last twice as long"),
    ("gluttonous", "gluttonous", "she eats what she rams: mends, but draws them"),
    ("greedy-keel", "greedy", "she pulls toward veins: half again the ore, longer bores"),
    ("vain", "vain", "she will not dock dirty; her claims pay double"),
    ("songful", "musical", "her bell wakes a floor, then lulls it"),
    ("mouldering", "rotten", "her hull mends itself, slowly, and smells"),
    ("crowned", "regal", "floor chiefs parley before they raid her claims"),
    ("feral", "feral", "she rams harder and steers worse"),
    ("blasting", "explosive", "she breaches with a blast that stuns the room"),
    ("sugared", "sweet", "monsters follow her wake"),
    ("rune-sighted", "arcane", "her sonar finds secret rooms"),
    ("old-iron", "ancient", "she never breaks down, and never hurries"),
];

/// The moments she speaks at.
pub const MOMENTS: [&str; 8] = ["launch", "breach", "ore", "hurt", "gnawer", "home", "fallen", "idle"];

/// The trait a quirk needs.
pub fn quirk(id: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    QUIRKS.iter().find(|q| q.0 == id)
}

/// A vessel as designed: what [`check`] reads from the model, or
/// [`fallback`] makes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Design {
    pub name: String,
    pub says: String,
    pub traits: Vec<String>,
    pub quirks: Vec<String>,
    pub voice: Vec<(String, String)>,
}

/// The shipwrights' brief: every piece, read to the model, and the shape of
/// the answer the game checks.
pub fn brief(plan: &Plan, fleet: &[Vessel]) -> String {
    let mut out = String::new();
    out.push_str("You are the shipwrights of Caer Dwfn, the dwarf-kingdom under the realm. A party of knights has hauled up salvage from the deep: pieces of old dwarf-work, each wished by the knight who found it into whatever they wanted. Read every piece and design ONE original vessel that bores through solid stone the way a submarine moves through water; the prow is what digs.\n\n");
    let prev = fleet.last().map_or(String::new(), |v| format!(", bigger than their last, {}", v.name));
    out.push_str(&format!(
        "This is their build number {}: a {}{}. Every piece below must be part of her and show in her. Her temper comes from what they wished.\n\nPIECES:\n",
        plan.n,
        class(plan.n),
        prev
    ));
    for p in &plan.pieces {
        out.push_str(&format!("- {} ({}), {}: {}\n", p.place().word(), p.place().role(), p.grade.word(), p.says()));
    }
    out.push_str("\nReply in exactly this format and nothing else:\n");
    out.push_str("vessel <her name, at most 32 characters>\n");
    out.push_str("says <what she is, built of these pieces, 20 to 300 characters>\n");
    out.push_str("traits <2 to 4, comma separated>\n");
    out.push_str("quirks <1 to 3, comma separated; each needs its trait>\n");
    for m in MOMENTS {
        out.push_str(&format!("voice {m} :: <her line, at most 90 characters>\n"));
    }
    out.push_str(&format!("\nTRAITS: {}\n", TRAITS.join(", ")));
    out.push_str("QUIRKS (quirk: needs trait: what it does):\n");
    for (id, needs, does) in QUIRKS {
        out.push_str(&format!("- {id}: {needs}: {does}\n"));
    }
    out.push_str("\nVOICE: launch (setting off), breach (bursting into a floor), ore (striking a vein), hurt (hull struck), gnawer (the stone-worm hunts her), home (docking at the forge), fallen (a knight falls), idle (waiting). Her lines are her own voice: short, funny, a little dangerous, never a list of her parts.\n");
    out
}

/// The model's answer, read and checked: the design, or why not.
pub fn check(reply: &str, fleet: &[Vessel]) -> Result<Design, Vec<String>> {
    let mut d = Design::default();
    let mut errs = Vec::new();
    let mut seen_name = false;
    for raw in reply.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("```") {
            continue;
        }
        let (key, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        match key.to_ascii_lowercase().as_str() {
            "vessel" => {
                d.name = rest.to_string();
                seen_name = true;
            }
            "says" => d.says = rest.to_string(),
            "traits" => d.traits = list(rest),
            "quirks" => d.quirks = list(rest),
            "voice" => match rest.split_once("::") {
                Some((m, l)) => d.voice.push((m.trim().to_ascii_lowercase(), l.trim().to_string())),
                None => errs.push(format!("voice `{rest}`: a moment, ::, her line")),
            },
            _ => {}
        }
    }
    let clean = |s: &str| !s.chars().any(char::is_control);
    let name_ok = |s: &str| s.chars().all(|c| c.is_alphanumeric() || " '-,.!".contains(c));
    if !seen_name || d.name.chars().count() < 2 || d.name.chars().count() > 32 || !name_ok(&d.name) {
        errs.push("vessel: her name, 2 to 32 letters, digits, spaces or ' - , . !".into());
    } else if fleet.iter().any(|v| v.name.eq_ignore_ascii_case(&d.name)) {
        errs.push(format!("vessel: the realm already has a {}", d.name));
    }
    let says = d.says.chars().count();
    if !(20..=300).contains(&says) || !clean(&d.says) {
        errs.push(format!("says: 20 to 300 characters (it was {says})"));
    }
    if !(2..=4).contains(&d.traits.len()) {
        errs.push(format!("traits: 2 to 4 (it had {})", d.traits.len()));
    }
    for t in &d.traits {
        if !TRAITS.contains(&t.as_str()) {
            errs.push(format!("traits: `{t}` is not one of {}", TRAITS.join(", ")));
        }
    }
    if !(1..=3).contains(&d.quirks.len()) {
        errs.push(format!("quirks: 1 to 3 (it had {})", d.quirks.len()));
    }
    for q in &d.quirks {
        match quirk(q) {
            None => errs.push(format!("quirks: `{q}` is not a quirk")),
            Some((_, needs, _)) if !d.traits.iter().any(|t| t == needs) => {
                errs.push(format!("quirks: `{q}` needs the trait `{needs}`"))
            }
            _ => {}
        }
    }
    for m in MOMENTS {
        let n = d.voice.iter().filter(|(k, _)| k == m).count();
        if n != 1 {
            errs.push(format!("voice {m}: exactly one line (it had {n})"));
        }
    }
    for (m, l) in &d.voice {
        if !MOMENTS.contains(&m.as_str()) {
            errs.push(format!("voice `{m}`: the moments are {}", MOMENTS.join(", ")));
        }
        let n = l.chars().count();
        if n == 0 || n > 90 || !clean(l) {
            errs.push(format!("voice {m}: one line, 1 to 90 characters"));
        }
    }
    if errs.is_empty() { Ok(d) } else { Err(errs) }
}

fn list(s: &str) -> Vec<String> {
    s.split(',').map(|t| t.trim().to_ascii_lowercase()).filter(|t| !t.is_empty()).collect()
}

/// The words that lean a vessel toward each trait (the fallback's reading).
const LEANS: [(&str, &[&str]); 16] = [
    ("holy", &["holy", "angel", "church", "cathedral", "halo", "saint", "bless", "prayer", "chapel", "divine"]),
    ("infernal", &["fire", "flame", "hell", "demon", "lava", "brimstone", "burn", "inferno", "magma", "ember"]),
    ("frozen", &["ice", "frost", "frozen", "snow", "cold", "glacier", "rime"]),
    ("toxic", &["poison", "toxic", "slime", "venom", "acid", "sludge"]),
    ("drunk", &["drank", "syrup", "ale", "beer", "wine", "mead", "booze", "lean", "rum", "grog"]),
    ("gluttonous", &["bacon", "meat", "feast", "hungry", "teeth", "mouth", "pie", "sausage", "ham", "eat"]),
    ("greedy", &["gold", "coin", "treasure", "jackpot", "gem", "loot", "money", "hoard"]),
    ("vain", &["disco", "mirror", "shiny", "glitter", "pretty", "sparkle", "fabulous", "polished"]),
    ("musical", &["organ", "bell", "song", "drum", "music", "horn", "flute", "sing"]),
    ("rotten", &["rot", "zombie", "mushroom", "fungus", "corpse", "mould", "mold", "grave"]),
    ("regal", &["king", "queen", "crown", "throne", "royal", "emperor", "lord"]),
    ("feral", &["wolf", "beast", "claw", "fang", "feral", "boar", "bear", "wild", "tusk"]),
    ("explosive", &["bomb", "explode", "rocket", "boom", "powder", "blast", "dynamite"]),
    ("sweet", &["candy", "sweet", "sugar", "honey", "cake", "cookie", "jam"]),
    ("arcane", &["rune", "magic", "arcane", "wizard", "spell", "crystal", "star", "moon"]),
    ("ancient", &["old", "ancient", "elder", "first", "lost", "deep"]),
];

/// Her traits as the pieces' words lean, strongest first (two at least).
pub fn leanings(pieces: &[Piece]) -> Vec<&'static str> {
    let text: String = pieces.iter().map(|p| p.says().to_ascii_lowercase() + " ").collect();
    let words: Vec<&str> = text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let mut scored: Vec<(&'static str, usize)> = LEANS
        .iter()
        .map(|(t, keys)| (*t, words.iter().filter(|w| keys.iter().any(|k| w.starts_with(k))).count()))
        .filter(|(_, n)| *n > 0)
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    let mut out: Vec<&'static str> = scored.into_iter().map(|(t, _)| t).take(4).collect();
    for spare in ["ancient", "feral"] {
        if out.len() >= 2 {
            break;
        }
        if !out.contains(&spare) {
            out.push(spare);
        }
    }
    out
}

/// A vessel made from the pieces alone, when no model answers well: named
/// by her strongest leaning, her quirks from her traits, her voice plain.
pub fn fallback(plan: &Plan, fleet: &[Vessel]) -> Design {
    let traits = leanings(&plan.pieces);
    const ADJ: [(&str, &str); 16] = [
        ("holy", "Blessed"),
        ("infernal", "Brimstone"),
        ("frozen", "Rime"),
        ("toxic", "Septic"),
        ("drunk", "Sozzled"),
        ("gluttonous", "Hungry"),
        ("greedy", "Grasping"),
        ("vain", "Gilded"),
        ("musical", "Singing"),
        ("rotten", "Mouldering"),
        ("regal", "Crowned"),
        ("feral", "Wild"),
        ("explosive", "Thundering"),
        ("sweet", "Sugared"),
        ("arcane", "Rune-Bright"),
        ("ancient", "Old"),
    ];
    const NOUNS: [&str; 6] = ["Mole", "Tusk", "Boar", "Earth-Boar", "Mountain", "Leviathan"];
    let adj = ADJ.iter().find(|(t, _)| *t == traits[0]).map_or("Old", |(_, a)| a);
    let noun = NOUNS[(plan.n.max(1) as usize - 1).min(NOUNS.len() - 1)];
    let mut name = format!("The {adj} {noun}");
    let mut k = 2;
    while fleet.iter().any(|v| v.name.eq_ignore_ascii_case(&name)) {
        name = format!("The {adj} {noun} {}", roman(k));
        k += 1;
    }
    let mut says = format!("A {} built from ", class(plan.n));
    let parts: Vec<String> = plan.pieces.iter().map(|p| p.says()).collect();
    says.push_str(&parts.join("; "));
    says.push('.');
    if says.chars().count() > 300 {
        says = says.chars().take(297).collect::<String>() + "...";
    }
    let quirks: Vec<String> = traits
        .iter()
        .filter_map(|t| QUIRKS.iter().find(|q| q.1 == *t).map(|q| q.0.to_string()))
        .take(3)
        .collect();
    let a = adj.to_ascii_lowercase();
    let voice = [
        ("launch", format!("{adj} and ready. Hold on to something.")),
        ("breach", "Knock knock.".to_string()),
        ("ore", "Ooh. Shiny. Mine.".to_string()),
        ("hurt", "Mind the paint!".to_string()),
        ("gnawer", "Something down here is hungrier than me.".to_string()),
        ("home", format!("Home. Tell the forge its {a} girl is back.")),
        ("fallen", "Up, knight. I am not hauling you home in the hold.".to_string()),
        ("idle", "Are we digging or brooding?".to_string()),
    ]
    .into_iter()
    .map(|(m, l)| (m.to_string(), l))
    .collect();
    Design {
        name,
        says,
        traits: traits.into_iter().map(str::to_string).collect(),
        quirks,
        voice,
    }
}

fn roman(n: usize) -> &'static str {
    ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"][n.min(10)]
}

/// A wish's words, cleaned for the yard: no control characters, trimmed,
/// at most [`WISH_CHARS`].
pub fn clean_wish(words: &str) -> String {
    words.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(WISH_CHARS).collect()
}

impl Run {
    /// Salvage found this tick: a guardian of the salvage floors always
    /// gives up a piece, a won fight room one in [`ROOM_EVERY`]. The knight
    /// standing who has found least takes it, to wish.
    pub(super) fn find_salvage(&mut self, heard: usize) {
        let Some(rank) = SALVAGE_PACKS.iter().position(|&p| p == self.dungeon.pack) else {
            return;
        };
        let finds: Vec<bool> = self
            .cues
            .get(heard..)
            .unwrap_or_default()
            .iter()
            .filter_map(|c| {
                if c.starts_with("boss_fall:") {
                    Some(true)
                } else if c == "room_clear" || c == "flawless" {
                    Some(false)
                } else {
                    None
                }
            })
            .collect();
        // The yard's own dice, off the run's seed and the moment: the
        // delve's dice (`rng`) are never drawn, so a run plays tick for
        // tick as it did before salvage.
        for (k, guardian) in finds.into_iter().enumerate() {
            let roll = |salt: u64| {
                mix(self.seed ^ self.tick.rotate_left(17) ^ (u64::from(self.home.salvage.next) << 40) ^ ((k as u64) << 8) ^ salt)
            };
            if !guardian && roll(1) % ROOM_EVERY as u64 != 0 {
                continue;
            }
            let grade = grade_of(rank as u32, guardian, (roll(2) % 100) as u32);
            let place = Place::ALL[(roll(3) % Place::ALL.len() as u64) as usize];
            self.salvage_to_finder(place, grade);
        }
    }

    /// A piece of `place` and `grade` into the yard, for the standing knight
    /// who has found least (the lowest seat on a tie).
    pub fn salvage_to_finder(&mut self, place: Place, grade: Grade) -> Option<u32> {
        let finder = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
            .map(|(&id, h)| {
                let key = super::talents::knight_key(h);
                let had = self.home.salvage.pieces.iter().filter(|p| p.by == key).count();
                (had, id, key)
            })
            .min()?;
        let (_, id, key) = finder;
        let pack = self.dungeon.pack;
        let piece = self.home.salvage.add(place, grade, &key, pack);
        let found = place.found_as(grade);
        self.found = Some((self.tick, id, format!("{} salvage: {found}", capital(grade.word()))));
        self.tell(id, format!("You found {found} ({}). Press T and wish it into whatever you want.", grade.word()));
        self.cues.push(format!("salvage:{}", grade.word()));
        self.sounds.push("spoil_pickup");
        Some(piece)
    }

    /// The piece knight `id` found and has yet to wish, if any.
    pub fn piece_awaiting(&self, id: u32) -> Option<&Piece> {
        let key = super::talents::knight_key(self.players.get(&id)?);
        self.home.salvage.awaiting(&key)
    }

    /// Knight `id` wishes on the piece they found: the words are the piece
    /// now. No held wish is spent.
    pub fn wish_on_piece(&mut self, id: u32, words: &str) -> Result<String, String> {
        let key = super::talents::knight_key(self.players.get(&id).ok_or("No such knight.")?);
        let words = clean_wish(words);
        if words.is_empty() {
            return Err("Say what you want it to be.".into());
        }
        let yard = &mut self.home.salvage;
        let piece = yard
            .pieces
            .iter_mut()
            .find(|p| p.by == key && p.wish.is_none())
            .ok_or("You have no unwished salvage.")?;
        let line = format!("Your {} is now: {words}", piece.found);
        piece.wish = Some(words);
        yard.rev += 1;
        self.tell(id, line.clone());
        self.cues.push("salvage_wished".into());
        self.sounds.push("wish_grant");
        Ok(line)
    }

    /// The yard's next build, if it is ready and the party is home.
    pub fn salvage_plan(&self) -> Option<Plan> {
        self.at_home_now().then(|| self.home.salvage.plan()).flatten()
    }

    /// Build the vessel `design` from the pieces numbered `ids` (the plan
    /// the design was asked for): they leave the yard and she is the
    /// realm's. Refused if any piece is gone or the build has been made.
    pub fn build_vessel(&mut self, n: u32, ids: &[u32], design: Design, by_model: bool) -> Result<String, String> {
        let yard = &mut self.home.salvage;
        if yard.next_build() != n {
            return Err(format!("build {n} is already made"));
        }
        if !ids.iter().all(|id| yard.pieces.iter().any(|p| p.id == *id)) {
            return Err("a piece of that plan has left the yard".into());
        }
        let mut pieces = Vec::new();
        yard.pieces.retain(|p| {
            if ids.contains(&p.id) {
                pieces.push(p.clone());
                false
            } else {
                true
            }
        });
        let name = design.name.clone();
        yard.vessels.push(Vessel {
            n,
            name: design.name,
            says: design.says,
            traits: design.traits,
            quirks: design.quirks,
            voice: design.voice,
            pieces,
            by_model,
            revealed: false,
        });
        yard.rev += 1;
        self.cues.push(format!("vessel_built:{n}"));
        Ok(name)
    }

    /// The newest vessel shown to the party: the reveal's cue, once.
    pub fn reveal_vessel(&mut self) -> Option<String> {
        let yard = &mut self.home.salvage;
        let v = yard.vessels.iter_mut().rev().find(|v| !v.revealed)?;
        v.revealed = true;
        let (n, name) = (v.n, v.name.clone());
        let first = v.line("launch").unwrap_or("").to_string();
        yard.rev += 1;
        self.cues.push(format!("vessel_reveal:{n}"));
        self.fx.push(Fx::Shake(30));
        self.sounds.push("boss_fall");
        self.found = Some((self.tick, 0, format!("{name}: \u{201c}{first}\u{201d}")));
        Some(name)
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

#[cfg(test)]
#[path = "../../tests/rules/together_salvage__tests.rs"]
mod tests;
