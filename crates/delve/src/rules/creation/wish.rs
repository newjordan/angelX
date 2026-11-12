//! The Wish box's brain: `wish_to_spec(text, history) -> Spec`.
//!
//! THE SEAM. Offline and deterministic: keywords and tags matched against the part catalogues
//! (the arms library for weapons, the authored creature parts), plus refinements ("bigger skull",
//! "make the flame blue"). The Wish-it lane's model-backed parser will replace the body of
//! [`wish_to_spec`] and keep its contract:
//!
//! * `history` is the conversation so far, oldest first. The Creator seeds it, and re-seeds its
//!   last turn from the live recipe before every call, so a hand edit between two wishes is kept
//!   and an unclear wish keeps the builder's own role.
//! * The answer is a COMPLETE recipe in [`Spec::make`] (never a patch), already legal: every part
//!   fits its slot and joint class, and it has been haggled to the role's power budget when it can
//!   be. The session applies it as it stands and re-derives stats and abilities itself.
//! * A part the library lacks is a [`Want`]: the spec carries the nearest library part as the
//!   `stand_in` it actually uses, so the preview always works, and `look` is the sentence image
//!   generation will be given to paint the missing part (SPEC-CREATOR 1c). A generated part,
//!   once gated and added to the library, simply becomes a candidate for the same slot.
//! * [`Spec::reply`] is the genie's answer in his voice and [`Spec::quip`] a few words for his
//!   bubble; a model may write both, the cost line is always checked against the recipe.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Make {
    Creature(Creation),
    Weapon(WeaponCreation),
}

/// A part the library lacks, to be painted: its place, what it should look like, and what stands in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Want {
    pub slot: String,
    pub look: String,
    pub stand_in: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub wish: String,
    pub make: Make,
    pub wants: Vec<Want>,
    /// What the genie trimmed to fit the budget, in order.
    pub trimmed: Vec<String>,
    pub asked: u16,
    pub cost: u16,
    pub budget: u16,
    pub notes: Vec<String>,
    /// A weapon named inside a creature's wish: creatures are bare-handed, it is its own wish.
    pub fork: Option<String>,
    pub reply: String,
    pub quip: String,
    pub parser: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub said: String,
    pub spec: Spec,
}

impl Spec {
    /// The live recipe as a spec, for seeding a conversation.
    pub fn of(creation: &Creation, weapon: &WeaponCreation) -> Self {
        let make = if creation.role.is_weapon() {
            Make::Weapon(weapon.clone())
        } else {
            Make::Creature(creation.clone())
        };
        let (cost, budget) = match &make {
            Make::Weapon(w) => (w.spent(), WEAPON_BUDGET),
            Make::Creature(c) => (c.spent(), c.role.budget()),
        };
        Self {
            wish: String::new(),
            make,
            wants: Vec::new(),
            trimmed: Vec::new(),
            asked: cost,
            cost,
            budget,
            notes: Vec::new(),
            fork: None,
            reply: String::new(),
            quip: String::new(),
            parser: PARSER,
        }
    }
}

pub const PARSER: &str = "offline keywords";

// ------------------------------------------------------------------------------------ the game's gate

/// Every word `refused` names, covered by `bar` (a censor bar) and counted. Whole words, any case.
pub fn censor_by(text: &str, bar: &str, refused: &dyn Fn(&str) -> bool) -> (String, usize) {
    let (mut out, mut word, mut covered) = (String::new(), String::new(), 0);
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            if refused(&word) {
                out.push_str(bar);
                covered += 1;
            } else {
                out.push_str(&word);
            }
            word.clear();
        }
        out.push(c);
    }
    out.pop();
    (out, covered)
}

/// The game's own gate on what a wish may say (`cards::not_for_work`: a public build refuses a long
/// list, a private table only slurs, both from the one rule). A wish with such a word is not turned
/// away: the word gets a censor bar, in the transcript and in everything made of the words, and the
/// rest is granted. Nothing refused ever reaches a `Want`'s look, a name or a reply.
pub fn censor(text: &str, bar: &str) -> (String, usize) {
    censor_by(text, bar, &|w| crate::rules::cards::not_for_work(w).is_some())
}

// ------------------------------------------------------------------------------------ words

fn stem(w: &str) -> String {
    if w.len() > 4 && w.ends_with("ies") {
        format!("{}y", &w[..w.len() - 3])
    } else if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") {
        w[..w.len() - 1].to_string()
    } else {
        w.to_string()
    }
}

fn words(text: &str) -> Vec<String> {
    let lower = text.to_lowercase().replace("'s", " ").replace('\u{2019}', "'");
    lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(stem)
        .collect()
}

fn any(ws: &[String], list: &[&str]) -> bool {
    ws.iter().any(|w| list.contains(&w.as_str()))
}

fn pos(ws: &[String], list: &[&str]) -> Option<usize> {
    ws.iter().position(|w| list.contains(&w.as_str()))
}

const STOP: &[&str] = &[
    "a", "an", "the", "with", "of", "and", "for", "its", "it", "on", "in", "that", "has", "have", "made", "from",
    "into", "some", "very", "my", "me", "i", "want", "wish", "like", "give", "make", "please", "to", "is", "be", "can",
    "held", "together", "called", "named", "out", "bit", "little", "kind", "sort", "thing", "one", "just", "also", "now",
    "this", "your", "yours", "mine", "wear", "wearing", "holding", "carrying", "who", "which", "or", "but", "so", "as",
    "by", "at", "up", "down", "all", "any", "new", "more", "less", "too", "much", "really", "bound", "tied", "carved",
    "covered", "wrapped", "no", "not", "s", "than", "that", "then", "there", "their", "them", "was", "are", "do", "get",
    "let", "should", "would", "could", "need", "something", "someone", "good", "nice", "cool", "great", "awesome",
    "themed", "theme", "style", "styled", "shaped", "inspired", "like", "based", "looking", "dripping_", "wielding",
];

const WEAPON_NOUNS: &[&str] = &[
    "sword", "longsword", "saber", "sabre", "rapier", "katana", "cutlass", "falchion", "blade", "cleaver", "machete",
    "knife", "dagger", "mace", "flail", "plunger", "axe", "hatchet", "tomahawk", "hammer", "maul", "mallet", "club",
    "cudgel", "spear", "lance", "pike", "javelin", "trident", "halberd", "scythe", "reaper", "sickle", "bow",
    "longbow", "shortbow", "crossbow", "shield", "buckler", "aegis", "bulwark", "targe", "staff", "stave", "rod",
    "crook", "wand", "scepter", "sceptre", "weapon",
];

const CREATURE_NOUNS: &[&str] = &[
    "hero", "knight", "champion", "paladin", "mage", "wizard", "witch", "ranger", "warrior", "rogue", "minion", "pet",
    "sidekick", "imp", "monster", "beast", "creature", "goblin", "ogre", "troll", "orc", "ghoul", "skeleton", "ghost",
    "dragon", "frog", "toad", "boss", "king", "queen", "lord", "demon", "golem", "spider", "wolf", "rat", "bat",
    "slime", "giant", "gnome", "dwarf", "elf", "kobold", "lizard", "serpent", "bear", "bull", "goat", "sprite", "fairy",
    "mummy", "zombie", "vampire", "bandit", "soldier", "brute", "fiend", "barman", "barmaid", "bartender", "innkeeper",
    "publican", "smith", "blacksmith", "pilgrim", "merchant", "baker", "farmer", "priest", "monk", "cook", "villager",
    "peasant", "traveller", "traveler", "hermit", "bard", "fisherman", "miller", "tailor", "clerk", "herald",
    "folk", "npc", "townsfolk", "shopkeeper", "waiter", "maid", "butcher", "brewer", "healer", "scribe", "jester",
];

/// A Folk's occupations (the rite, 2026-11-07): a wish naming one makes a world NPC, not a fighter.
const FOLK_NOUNS: &[&str] = &[
    "barman", "barmaid", "bartender", "innkeeper", "publican", "smith", "blacksmith", "pilgrim", "merchant", "baker",
    "farmer", "priest", "monk", "cook", "villager", "peasant", "traveller", "traveler", "hermit", "bard", "fisherman",
    "miller", "tailor", "clerk", "herald", "folk", "npc", "townsfolk", "shopkeeper", "waiter", "maid",
    "butcher", "brewer", "healer", "scribe", "jester",
];

/// What a Folk of an occupation says on E, unless its maker puts words in its mouth.
fn folk_line(ws: &[String]) -> &'static str {
    const LINES: &[(&[&str], &str)] = &[
        (&["barman", "barmaid", "bartender", "innkeeper", "publican", "waiter", "brewer"], "What'll it be, traveller? The ale is cold."),
        (&["smith", "blacksmith"], "Bring me steel and I will make it sing."),
        (&["pilgrim", "priest", "monk", "hermit"], "The old stair remembers every knight."),
        (&["merchant", "baker", "cook", "miller", "butcher", "shopkeeper", "tailor"], "Honest goods, honest price."),
        (&["bard", "jester"], "Shall I sing of your deeds? Later, perhaps."),
        (&["herald", "clerk", "scribe"], "Halt. Oh, it is you. Carry on."),
        (&["farmer", "villager", "peasant", "fisherman", "healer"], "Fine weather for a quest."),
    ];
    LINES.iter().find(|(names, _)| any(ws, names)).map_or(FOLK_LINES[0], |(_, line)| *line)
}

/// A Folk's calling from the words: where it stands, whether it wanders, what it says.
fn edit_folk(c: &mut Creation, ws: &[String], fresh: bool) -> bool {
    let mut changed = false;
    if !c.spells.is_empty() {
        c.spells.clear();
        changed = true;
    }
    let post = if any(ws, &["courtyard", "gate", "yard", "outside", "lawn", "square"]) {
        Some(Post::Courtyard)
    } else if any(ws, &["pub", "tavern", "griffin", "alehouse", "taproom", "inn", "bar"]) {
        Some(Post::Griffin)
    } else {
        None
    };
    if let Some(post) = post && c.calling.post != post {
        c.calling.post = post;
        changed = true;
    }
    let habit = if any(ws, &["wander", "walk", "roam", "stroll", "pace", "wandering", "walking", "roaming"]) {
        Some(Habit::Wanders)
    } else if any(ws, &["stand", "stay", "still", "behind", "stationary"]) {
        Some(Habit::Stands)
    } else {
        None
    };
    if let Some(habit) = habit && c.calling.habit != habit {
        c.calling.habit = habit;
        changed = true;
    }
    // "who says welcome home": the rest of the wish is the line.
    let said = pos(ws, &["say", "says", "saying", "shout", "shouts", "whisper", "whispers"]).map(|i| {
        let rest = ws[i + 1..].join(" ");
        let mut chars = rest.chars();
        let line = chars.next().map_or(String::new(), |f| f.to_uppercase().chain(chars).collect::<String>());
        let line: String = line.chars().take(LINE_MAX - 1).collect();
        format!("{}.", line.trim_end())
    });
    let line = said.filter(|l| l.len() > 3).or_else(|| (fresh || !any(ws, &["say", "says"])).then(|| folk_line(ws).to_string()).filter(|_| fresh));
    if let Some(line) = line && c.calling.line != line {
        c.calling.line = line;
        changed = true;
    }
    changed
}

/// Words that resize what is already there.
const SIZE_WORDS: &[&str] = &[
    "bigger", "larger", "huge", "enlarge", "smaller", "tiny", "shrink", "thicker", "wider", "thinner", "longer",
    "shorter", "extend", "lengthen", "shorten", "taller",
];

/// Words that mean a change to what already stands.
const TRIGGERS: &[&str] = &[
    "bigger", "larger", "huge", "enlarge", "smaller", "tiny", "shrink", "longer", "shorter", "extend", "lengthen",
    "shorten", "taller", "make", "add", "change", "swap", "glow", "more", "less", "instead", "paint", "colour", "color",
    "recolor", "recolour", "remove", "another", "different", "rename", "call", "enchant", "now", "also", "turn",
    "thicker", "thinner", "wider", "stronger", "weaker", "cheaper",
];

fn school_of(ws: &[String]) -> Option<School> {
    const TABLE: &[(School, &[&str])] = &[
        (School::Fire, &["fire", "flame", "flaming", "burning", "burn", "ember", "blaze", "inferno", "molten", "fiery", "flam"]),
        (School::Ice, &["ice", "frost", "frozen", "snow", "snowflake", "cold", "winter", "chill", "icy", "frosty", "glacier"]),
        (School::Lightning, &["lightning", "thunder", "storm", "spark", "shock", "electric", "static", "stormy"]),
        (School::Arcane, &["arcane", "rune", "runic", "mystic", "magical", "magic", "enchanted"]),
        (School::Holy, &["holy", "sun", "solar", "divine", "angelic", "sacred", "radiant", "blessed", "sunlit"]),
        (School::Shadow, &["shadow", "grave", "ink", "night", "gloom", "cursed", "necro", "shade", "umbral", "void"]),
        (School::Nature, &["nature", "vine", "thorn", "briar", "forest", "bloom", "flower", "leafy", "poison", "venom"]),
    ];
    TABLE.iter().find(|(_, l)| any(ws, l)).map(|(s, _)| *s)
}

/// A colour said of a flame or a gem: the school whose glow wears it.
fn glow_colour(ws: &[String]) -> Option<School> {
    const TABLE: &[(&str, School)] = &[
        ("blue", School::Ice),
        ("cyan", School::Ice),
        ("teal", School::Ice),
        ("red", School::Fire),
        ("orange", School::Fire),
        ("crimson", School::Fire),
        ("green", School::Nature),
        ("purple", School::Arcane),
        ("violet", School::Arcane),
        ("pink", School::Arcane),
        ("yellow", School::Holy),
        ("golden", School::Holy),
        ("white", School::Lightning),
        ("black", School::Shadow),
    ];
    ws.iter().find_map(|w| TABLE.iter().find(|(c, _)| c == w).map(|(_, s)| *s))
}

fn metal_word(ws: &[String]) -> Option<Material> {
    const TABLE: &[(Material, &[&str])] = &[
        (Material::Gold, &["gold", "golden", "brass", "gilded"]),
        (Material::Plate, &["steel", "iron", "silver", "metal", "metallic", "polished"]),
        (Material::Bone, &["bone", "ivory", "skeletal"]),
        (Material::Shell, &["rusty", "rust", "rusted", "copper", "bronze"]),
        (Material::Chitin, &["violet", "purple", "obsidian"]),
        (Material::Glow, &["frost", "icy", "frozen"]),
        (Material::Skin, &["moss", "mossy", "green", "frog"]),
        (Material::Eye, &["pearl", "pearly", "white"]),
        (Material::FleshInner, &["blood", "bloody", "crimson"]),
        (Material::GagPink, &["pink", "rose"]),
    ];
    TABLE.iter().find(|(_, l)| any(ws, l)).map(|(m, _)| *m)
}

/// The paint a socketless weapon wears in place of a gem.
fn school_metal(school: School) -> Material {
    match school {
        School::Fire => Material::Shell,
        School::Ice => Material::Glow,
        School::Lightning => Material::Eye,
        School::Arcane | School::Shadow => Material::Chitin,
        School::Holy => Material::Gold,
        School::Nature => Material::Skin,
    }
}

const SLOT_WORDS: &[(&str, &[&str])] = &[
    ("pommel", &["pommel", "butt", "knob", "counterweight"]),
    ("grip", &["grip", "handle", "hilt"]),
    ("guard", &["guard", "crossguard", "crosspiece"]),
    ("blade", &["blade", "edge"]),
    ("haft", &["haft", "shaft", "pole", "stem"]),
    ("head", &["head"]),
    ("boss", &["boss", "stud"]),
    ("rim", &["rim"]),
    ("face", &["face"]),
    ("limb", &["limb"]),
];

/// What a word means to a part's own name and materials.
fn synonyms(w: &str) -> &'static [&'static str] {
    match w {
        "skull" | "skeleton" => &["skull"],
        "rope" | "rag" | "cord" | "twine" | "cloth" => &["rag"],
        "rusty" | "rust" | "crude" | "goblin" | "ragged" | "scrap" | "rough" => &["goblin", "crude"],
        "knight" | "royal" | "noble" | "steel" | "silver" => &["knight", "steel", "plate"],
        "winged" | "wing" | "angel" | "angelic" | "seraph" | "feather" | "holy" => &["winged"],
        "spike" | "spiked" | "spiky" | "thorn" | "thorny" => &["spike"],
        "wood" | "wooden" | "oak" | "timber" => &["wood", "leather"],
        "banded" | "reinforced" => &["banded"],
        "giant" | "huge" | "big" => &["great"],
        "twisted" | "crooked" | "knotty" | "natural" | "branch" => &["gnarled"],
        "iron" => &["iron", "plate"],
        "dark" | "black" | "ebony" => &["dark"],
        "white" | "pale" => &["ivory"],
        "leaf" | "lily" | "pad" | "leafy" | "frond" => &["leaf"],
        "studded" => &["flanged"],
        "battle" => &["war"],
        "golden" | "brass" | "gilded" => &["gold"],
        "viking" | "circular" => &["round"],
        "small" => &["buckler", "short"],
        "domed" => &["dome"],
        "bone" | "bony" => &["bone"],
        "leather" => &["leather"],
        "gold" => &["gold"],
        _ => &[],
    }
}

fn part_tokens(p: &KitPart) -> Vec<String> {
    let mut t: Vec<String> = p.short().split('-').skip(1).map(String::from).collect();
    t.extend(p.materials.iter().cloned());
    t.extend(
        p.tags
            .iter()
            .filter(|g| !matches!(g.as_str(), "canonical" | "w5w" | "weapon" | "small" | "enchant"))
            .cloned(),
    );
    t
}

fn means(word: &str, toks: &[String]) -> bool {
    toks.iter().any(|t| t == word) || synonyms(word).iter().any(|s| toks.iter().any(|t| t == s))
}

/// Every word the catalogues, lexicons and fillers understand; the rest is a motif to be painted.
fn known(w: &str) -> bool {
    use std::collections::BTreeSet;
    use std::sync::OnceLock;
    static VOCAB: OnceLock<BTreeSet<String>> = OnceLock::new();
    let vocab = VOCAB.get_or_init(|| {
        let mut v: BTreeSet<String> = BTreeSet::new();
        for p in &kit().parts {
            v.extend(part_tokens(p));
        }
        for list in [STOP, WEAPON_NOUNS, CREATURE_NOUNS, TRIGGERS, OVERSIZE] {
            v.extend(list.iter().map(|s| s.to_string()));
        }
        for (_, l) in SLOT_WORDS {
            v.extend(l.iter().map(|s| s.to_string()));
        }
        for w in [
            "bigger", "larger", "smaller", "longer", "shorter", "big", "large", "small", "long", "short", "tall", "thin",
            "thick", "heavy", "light", "sharp", "blunt", "old", "ancient", "magic", "strong", "weak", "fast", "slow",
            "red", "orange", "blue", "cyan", "teal", "green", "purple", "pink", "yellow", "white", "black", "grey", "gray",
            "brown", "tan", "banded", "striped", "stripe", "spotted", "speckled", "dotted", "colour", "color", "glow",
            "glowing", "enchant", "bolt", "nova", "beam", "ward", "summon", "aura", "cast", "spell", "shoot", "breath",
            "hurl", "blast", "crystal", "gem", "stone", "armour", "armor", "mail", "plate", "claw", "horn", "tail",
            "mask", "eye", "fist", "palm", "bell", "charm", "soul", "lantern", "hand", "head", "body", "arm", "leg",
            "bark", "crown", "royal", "ram", "ember", "rock", "granite", "brow", "heater", "round", "buckler", "scale",
            "paint", "one", "hand", "two", "handed", "single", "double", "towering", "lanky", "stout", "squat", "bulky",
            "burly", "hulking", "fat", "broad", "slim", "skinny", "lean", "slender", "wider", "thicker", "thinner", "slimmer", "leaner",
            "pet", "armoured", "regal", "majestic",
        ] {
            v.insert(w.to_string());
        }
        for w in [
            "fire", "flame", "flaming", "burning", "burn", "blaze", "inferno", "molten", "fiery", "ice", "frost",
            "frozen", "snow", "snowflake", "cold", "winter", "chill", "icy", "frosty", "lightning", "thunder", "storm",
            "spark", "shock", "electric", "static", "arcane", "rune", "runic", "mystic", "magical", "holy", "sun",
            "solar", "divine", "angelic", "sacred", "radiant", "blessed", "shadow", "grave", "ink", "night", "gloom",
            "cursed", "necro", "shade", "void", "nature", "vine", "thorn", "briar", "forest", "bloom", "flower", "poison",
            "venom", "gold", "golden", "brass", "silver", "copper", "bronze", "rust", "rusty", "rusted", "moss",
            "mossy", "pearl", "blood", "bloody", "obsidian", "violet", "rose", "polished", "metal", "metallic",
            "bone", "ivory", "skeletal", "frog", "toad", "lily", "pad", "leafy", "rope", "cord", "twine", "oak",
            "wooden", "timber", "crude", "goblin", "scrap", "rough", "ragged", "noble", "winged", "angel", "seraph",
            "feather", "spiked", "spiky", "thorny", "reinforced", "twisted", "crooked", "knotty", "natural", "branch",
            "dark", "ebony", "pale", "frond", "studded", "battle", "viking", "circular", "domed", "bony", "skull",
        ] {
            v.insert(w.to_string());
        }
        v
    });
    vocab.contains(w) || w.chars().all(|c| c.is_ascii_digit())
}

// ------------------------------------------------------------------------------------ genie voice

fn pick<'a>(seed: &str, options: &[&'a str]) -> &'a str {
    let h = seed
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
    options[(h >> 17) as usize % options.len()]
}

fn caps(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// Where a noun phrase ends: what follows describes, it does not name.
const CUT: &[&str] = &[
    "with", "in", "of", "that", "who", "holding", "wearing", "carrying", "for", "wielding", "using", "held", "and",
    "dripping", "which",
];

fn title(ws: &[String], limit: usize) -> String {
    let end = ws.iter().position(|w| CUT.contains(&w.as_str())).unwrap_or(ws.len());
    let mut keep: Vec<&str> = ws[..end]
        .iter()
        .map(String::as_str)
        .filter(|w| !STOP.contains(w) && !["add", "change", "swap", "turn", "call", "rename", "paint", "enchant"].contains(w) && w.len() > 1)
        .take(4)
        .collect();
    while keep.join(" ").chars().count() > limit && keep.len() > 1 {
        keep.remove(0);
    }
    let name = caps(&keep.join(" "));
    if name.is_empty() { "Wished thing".into() } else { name.chars().take(limit).collect() }
}

// ------------------------------------------------------------------------------------ the seam

pub fn wish_to_spec(text: &str, history: &[Turn]) -> Spec {
    let (said, covered) = censor(text, " ");
    let shown = censor(text, "####").0;
    let text = said.as_str();
    let all = words(text);
    let trigger = any(&all, TRIGGERS);
    // "Call it Mossy Tom": what follows is a name, not a wish.
    let (ws, rename): (Vec<String>, Vec<String>) = match pos(&all, &["call", "name", "rename"]) {
        Some(i) => (
            all[..i].to_vec(),
            all[i + 1..].iter().filter(|x| !["it", "the", "weapon", "this", "him", "her"].contains(&x.as_str())).cloned().collect(),
        ),
        None => (all.clone(), Vec::new()),
    };
    let prev = history.last().map(|t| &t.spec);
    let prev_make = prev.map(|p| &p.make);
    let prev_weapon = matches!(prev_make, Some(Make::Weapon(_)));
    // A new kind of thing named: the head phrase decides ("a frog-themed spear with ..." is a
    // spear; "a frog knight in ... with a shield" is a knight), the last noun of it leading.
    let decide = |ws: &[String]| match (pos(ws, WEAPON_NOUNS), pos(ws, CREATURE_NOUNS)) {
        (Some(_), None) => Some(true),
        (None, Some(_)) => Some(false),
        (Some(w), Some(c)) => Some(w > c),
        (None, None) => None,
    };
    let head = &ws[..ws.iter().position(|w| CUT.contains(&w.as_str())).unwrap_or(ws.len())];
    let fresh_weapon = decide(head).or_else(|| decide(&ws));
    let refine = match (prev_make, fresh_weapon) {
        (None, _) => false,
        (Some(Make::Weapon(w)), Some(true)) => trigger && kind_of_words(&ws).is_none_or(|k| k == w.kind),
        (Some(Make::Creature(_)), Some(false)) => trigger && !any(&ws, &["hero", "minion", "monster", "boss"]),
        (Some(_), Some(_)) => false,
        (Some(_), None) => true,
    };
    let weapon = fresh_weapon.unwrap_or(prev_weapon);
    let mut wants = Vec::new();
    let mut notes = Vec::new();
    if covered > 0 {
        notes.push(format!("a censor bar covers {covered} word{}: the game does not carry it", if covered == 1 { "" } else { "s" }));
    }
    let mut fork = None;
    let mut changed = covered > 0;
    let make = if weapon {
        let mut w = match prev_make {
            Some(Make::Weapon(w)) if refine => w.clone(),
            Some(Make::Weapon(w)) => WeaponCreation { kind: WeaponKind::Blade, ..w.clone() },
            _ => WeaponCreation::default(),
        };
        if !refine {
            let kind = kind_of_words(&ws).unwrap_or(WeaponKind::Blade);
            let (id, maker, created) = (w.id.clone(), w.maker.clone(), w.created);
            w = WeaponCreation::of_kind(kind);
            (w.id, w.maker, w.created) = (id, maker, created);
            w.name = title(&ws, 24);
            changed = true;
        }
        changed |= edit_weapon(&mut w, &ws, text, !refine, &mut wants, &mut notes);
        if any(&ws, OVERSIZE) {
            grandest(&mut w);
            changed = true;
        }
        Make::Weapon(w)
    } else {
        let mut c = match prev_make {
            Some(Make::Creature(c)) if refine => c.clone(),
            Some(Make::Creature(c)) => Creation { parts: Creation::default().parts, sculpt: Sculpt::default(), paint: Paint::default(), spells: Vec::new(), ..c.clone() },
            _ => Creation::default(),
        };
        if !refine {
            c.name = title(&ws, 24);
            changed = true;
        }
        changed |= edit_creature(&mut c, &ws, !refine, &mut wants, &mut notes, &mut fork);
        Make::Creature(c)
    };
    let mut spec = Spec::of(&Creation::default(), &WeaponCreation::default());
    spec.wish = shown;
    spec.make = make;
    if !rename.is_empty() {
        let name = title(&rename, 24);
        match &mut spec.make {
            Make::Weapon(w) => w.name = name,
            Make::Creature(c) => c.name = name,
        }
        changed = true;
    }
    spec.wants = wants;
    spec.notes = notes;
    spec.fork = fork;
    (spec.asked, spec.budget) = budget_of(&spec.make);
    haggle(&mut spec);
    (spec.cost, spec.budget) = budget_of(&spec.make);
    speak(&mut spec, &ws, refine, changed);
    spec
}

/// Wishes that ask for the world: the genie builds the grandest weapon he can, then haggles.
const OVERSIZE: &[&str] = &[
    "infinite", "unlimited", "endless", "godlike", "omnipotent", "invincible", "unstoppable", "apocalypse",
    "strongest", "mightiest", "ultimate", "everything", "universe", "unbeatable",
];

fn grandest(w: &mut WeaponCreation) {
    for slot in w.kind.slots() {
        if let Some(best) = w.candidates(slot).into_iter().max_by_key(|p| (p.stat.cost, std::cmp::Reverse(p.id.clone()))) {
            let id = best.id.clone();
            let _ = w.equip(slot, Some(&id));
        }
    }
    if w.can_enchant() && w.gem.is_none() {
        let _ = w.set_gem(Some(School::Arcane));
    }
    if let Some(p) = w.parts.last_mut() {
        p.scale = 200;
    }
}

fn budget_of(make: &Make) -> (u16, u16) {
    match make {
        Make::Weapon(w) => (w.spent(), WEAPON_BUDGET),
        Make::Creature(c) => (c.spent(), c.role.budget()),
    }
}

fn kind_of_words(ws: &[String]) -> Option<WeaponKind> {
    const KINDS: &[(WeaponKind, &[&str])] = &[
        (WeaponKind::Wand, &["wand", "scepter", "sceptre"]),
        (WeaponKind::Staff, &["staff", "stave", "rod", "crook"]),
        (WeaponKind::Bow, &["bow", "longbow", "shortbow", "crossbow"]),
        (WeaponKind::Shield, &["shield", "buckler", "aegis", "bulwark", "targe"]),
        (
            WeaponKind::Haft,
            &["mace", "flail", "plunger", "axe", "hatchet", "tomahawk", "hammer", "maul", "mallet", "club", "cudgel",
              "spear", "lance", "pike", "javelin", "trident", "halberd", "scythe", "reaper", "sickle"],
        ),
        (
            WeaponKind::Blade,
            &["sword", "longsword", "saber", "sabre", "rapier", "katana", "cutlass", "falchion", "blade", "cleaver", "machete", "knife", "dagger"],
        ),
    ];
    // The first one named leads: "a wand with a blade" is a wand.
    ws.iter().find_map(|w| KINDS.iter().find(|(_, l)| l.contains(&w.as_str())).map(|(k, _)| *k))
}

/// The tip a noun names: "axe" is a war axe head, "plunger" a mace head with a gag attached.
fn tip_for(ws: &[String]) -> Option<(&'static str, &'static str)> {
    const TIPS: &[(&[&str], &str, &str)] = &[
        (&["cleaver", "machete", "chopper"], "blade", "w5w.blade-cleaver"),
        (&["knife", "dagger"], "blade", "w5w.blade-cleaver"),
        (&["mace", "flail", "plunger"], "head", "w5w.head-mace-flanged"),
        (&["axe", "hatchet", "tomahawk"], "head", "w5w.head-axe-war"),
        (&["hammer", "maul", "mallet"], "head", "w5w.head-hammer-war"),
        (&["club", "cudgel"], "head", "w5w.head-club-great"),
        (&["spear", "lance", "pike", "javelin", "trident", "halberd"], "head", "w5w.head-spear-leaf"),
        (&["scythe", "reaper", "sickle"], "head", "w5w.blade-scythe"),
        (&["longbow"], "limb", "w5w.limb-long"),
        (&["shortbow"], "limb", "w5w.limb-short"),
        (&["buckler"], "face", "w5w.face-buckler"),
    ];
    ws.iter().find_map(|w| TIPS.iter().find(|(l, _, _)| l.contains(&w.as_str())).map(|(_, s, id)| (*s, *id)))
}

/// Words that sit just before a slot noun belong to that slot ("skull pommel", "gold guard").
fn attached(ws: &[String]) -> Vec<(&'static str, Vec<String>)> {
    let mut out: Vec<(&'static str, Vec<String>)> = Vec::new();
    for (i, w) in ws.iter().enumerate() {
        let Some((slot, _)) = SLOT_WORDS.iter().find(|(_, l)| l.contains(&w.as_str())) else {
            continue;
        };
        let mut mine = Vec::new();
        let mut j = i;
        while j > 0 && mine.len() < 3 {
            let p = &ws[j - 1];
            if STOP.contains(&p.as_str()) || WEAPON_NOUNS.contains(&p.as_str()) {
                break;
            }
            mine.push(p.clone());
            j -= 1;
        }
        out.push((slot, mine));
    }
    out
}

fn score(p: &KitPart, attached: &[String], loose: &[String]) -> i32 {
    let toks = part_tokens(p);
    4 * attached.iter().filter(|w| means(w, &toks)).count() as i32
        + 2 * loose.iter().filter(|w| means(w, &toks)).count() as i32
}

fn edit_weapon(
    w: &mut WeaponCreation,
    ws: &[String],
    text: &str,
    fresh: bool,
    wants: &mut Vec<Want>,
    notes: &mut Vec<String>,
) -> bool {
    let mut changed = false;
    let att = attached(ws);
    let tip_slot = match w.kind {
        WeaponKind::Blade => "blade",
        WeaponKind::Haft | WeaponKind::Staff | WeaponKind::Wand => "head",
        WeaponKind::Bow => "limb",
        WeaponKind::Shield => "face",
    };
    // The noun's own part.
    let mut chosen_by_noun: Option<&str> = None;
    if fresh && let Some((slot, id)) = tip_for(ws) {
        if w.equip(slot, Some(id)).is_ok() {
            chosen_by_noun = Some(slot);
        }
    }
    if fresh && w.kind == WeaponKind::Bow && !any(ws, &["longbow", "shortbow"]) {
        let id = if any(ws, &["long", "tall", "great"]) { "w5w.limb-long" } else { "w5w.limb-short" };
        let _ = w.equip("limb", Some(id));
    }
    if fresh && w.kind == WeaponKind::Shield && chosen_by_noun.is_none() {
        let id = if any(ws, &["round", "viking", "circular", "wooden", "wood"]) {
            "w5w.face-round"
        } else if any(ws, &["skull", "bone", "skeleton"]) {
            "w5w.face-skull"
        } else if any(ws, &["small", "little", "tiny"]) {
            "w5w.face-buckler"
        } else {
            "w5w.face-heater"
        };
        let _ = w.equip("face", Some(id));
        chosen_by_noun = Some("face");
    }
    // A school: a focus takes its head, a socketed weapon a gem, a hammer the colours.
    let said_colour = glow_colour(ws).filter(|_| any(ws, &["flame", "fire", "glow", "glowing", "gem", "crystal", "light", "spark", "flaming", "burning"]));
    // "Make the flame blue" tints what burns; it does not change the school.
    let named_school = if said_colour.is_some() && !fresh { None } else { school_of(ws) };
    if let Some(school) = named_school.filter(|_| !fresh || w.kind != WeaponKind::Bow) {
        if w.kind.is_focus() {
            let kind = if w.kind == WeaponKind::Wand { "wand" } else { "staff" };
            let id = format!("w5w.head-{kind}-{}", school.name().to_lowercase());
            changed |= w.equip("head", Some(&id)).is_ok() && w.school() == Some(school);
        } else if w.can_enchant() {
            changed |= w.set_gem(Some(school)).is_ok();
        } else if matches!(w.kind, WeaponKind::Haft | WeaponKind::Shield | WeaponKind::Blade) {
            w.paint.metal = school_metal(school);
            notes.push(format!("no socket for a {} gem: painted in its colours", school.name().to_lowercase()));
            changed = true;
        }
    }
    if let Some(school) = said_colour {
        w.paint.glow = Some(school);
        // With no spark to tint, the colours the weapon wears for its school follow the word.
        if w.gem.is_none() && !w.kind.is_focus() {
            w.paint.metal = school_metal(school);
        }
        changed = true;
    }
    // Parts by their names, slot by slot (a refinement that only resizes swaps nothing).
    let resizing = !fresh && any(ws, SIZE_WORDS);
    let loose: Vec<String> = ws
        .iter()
        .filter(|x| !STOP.contains(&x.as_str()) && !WEAPON_NOUNS.contains(&x.as_str()))
        .cloned()
        .collect();
    for slot in w.kind.slots() {
        if chosen_by_noun == Some(*slot) || resizing {
            continue;
        }
        let mine: Vec<String> = att.iter().filter(|(s, _)| s == slot).flat_map(|(_, a)| a.clone()).collect();
        let cands = w.candidates(slot);
        let current = w.chosen(slot).and_then(|c| kit().part(&c.part_id)).map_or(0, |p| score(p, &mine, &loose));
        let best = cands
            .iter()
            .map(|p| (score(p, &mine, &loose), *p))
            .max_by_key(|(s, p)| (*s, std::cmp::Reverse(p.id.clone())));
        if let Some((s, p)) = best
            && s >= 2
            && s > current
            && w.chosen(slot).is_none_or(|c| c.part_id != p.id)
            && w.equip(slot, Some(&p.id)).is_ok()
        {
            changed = true;
        }
    }
    // Colour of one part's metal ("gold guard"), or of the whole weapon.
    if let Some(m) = metal_word(ws).filter(|m| !matches!(m, Material::Bone) || !fresh) {
        let mut painted = false;
        for (slot, mine) in &att {
            if w.chosen(slot).is_some() && metal_word(mine) == Some(m) {
                w.paint.slot_metal.insert((*slot).to_string(), m);
                painted = true;
            }
        }
        if !painted && !fresh && any(ws, TRIGGERS) && !any(ws, &["glow", "flame", "fire"]) {
            w.paint.metal = m;
            painted = true;
        }
        if !painted && fresh && !matches!(m, Material::Plate | Material::Bone) {
            // "A rusty cleaver", "a gold hammer": the head wears it.
            w.paint.slot_metal.insert(tip_slot.to_string(), m);
            painted = true;
        }
        changed |= painted;
    }
    // Refinements: size, length, a gem.
    if !fresh {
        changed |= refine_weapon(w, ws, &att, notes, wants);
    }
    // What the library lacks becomes a want, with the nearest part standing in.
    let missing: Vec<&String> = ws
        .iter()
        .filter(|x| !known(x) && !x.chars().all(|c| c.is_ascii_digit()))
        .collect();
    // Said again, an odd word is only a part to paint when it sits on a slot ("add a lily-pad blade").
    let missing: Vec<&String> = if fresh {
        missing
    } else {
        missing.into_iter().filter(|m| att.iter().any(|(_, a)| a.contains(m))).collect()
    };
    if !missing.is_empty() {
        let first = ws.iter().position(|x| x == missing[0]).unwrap_or(0);
        let mut run = missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ");
        // The noun right after the odd word is part of the look: "toilet plunger".
        match ws.get(first + missing.len()) {
            Some(next) if WEAPON_NOUNS.contains(&next.as_str()) || SLOT_WORDS.iter().any(|(_, l)| l.contains(&next.as_str())) => {
                run = format!("{run} {next}");
            }
            _ => {
                let slot_now = att.iter().find(|(_, a)| a.iter().any(|x| missing.contains(&x))).map(|(s, _)| *s).unwrap_or(tip_slot);
                if let Some(p) = w.chosen(slot_now).and_then(|p| kit().part(&p.part_id)) {
                    run = format!("{run} {}", part_name(p).rsplit(' ').collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" "));
                }
            }
        }
        let slot = att
            .iter()
            .find(|(_, a)| a.iter().any(|x| missing.contains(&x)))
            .map(|(s, _)| *s)
            .unwrap_or(tip_slot);
        let slot = if w.kind.slots().contains(&slot) { slot } else { tip_slot };
        let stand_in = w.chosen(slot).map(|p| p.part_id.clone()).unwrap_or_default();
        let _ = text;
        wants.push(Want {
            slot: slot.to_string(),
            look: run.trim().to_string(),
            stand_in,
        });
        changed = true;
    }
    // A skull or bone motif the chain has no part for ("skull pommel" on a J11 haft).
    for (slot, mine) in &att {
        let wanted = mine.iter().find(|m| matches!(m.as_str(), "skull" | "bone" | "gold" | "spike"));
        if let Some(m) = wanted && w.kind.slots().contains(slot) {
            let has = w.chosen(slot).and_then(|p| kit().part(&p.part_id)).is_some_and(|p| means(m, &part_tokens(p)));
            if !has && !wants.iter().any(|x| x.slot == *slot) && !matches!(m.as_str(), "gold") {
                let stand_in = w.chosen(slot).map(|p| p.part_id.clone()).unwrap_or_default();
                let noun = kit().part(&stand_in).map_or(String::new(), part_name);
                wants.push(Want { slot: slot.to_string(), look: format!("{m} {slot}"), stand_in: stand_in.clone() });
                let _ = noun;
            }
        }
    }
    changed
}

fn refine_weapon(
    w: &mut WeaponCreation,
    ws: &[String],
    att: &[(&'static str, Vec<String>)],
    notes: &mut Vec<String>,
    wants: &mut Vec<Want>,
) -> bool {
    let mut changed = false;
    let up = any(ws, &["bigger", "larger", "huge", "enlarge", "thicker", "wider"]);
    let down = any(ws, &["smaller", "tiny", "shrink", "thinner"]);
    if up || down {
        // The thing named: a slot noun, a part word, else (if no thing is named at all) the tip.
        let named: Vec<&String> = ws.iter().filter(|x| !STOP.contains(&x.as_str()) && !TRIGGERS.contains(&x.as_str())).collect();
        let slot = att
            .iter()
            .map(|(s, _)| *s)
            .find(|s| w.chosen(s).is_some())
            .map(str::to_string)
            .or_else(|| {
                w.parts
                    .iter()
                    .find(|p| kit().part(&p.part_id).is_some_and(|k| named.iter().any(|x| means(x, &part_tokens(k)))))
                    .map(|p| p.slot.clone())
            })
            .or_else(|| named.is_empty().then(|| w.parts.last().map_or(String::new(), |p| p.slot.clone())));
        if slot.is_none() && let Some(n) = named.first() {
            notes.push(format!("i see no {n} on this weapon to make {}", if up { "bigger" } else { "smaller" }));
        }
        let slot = slot.unwrap_or_default();
        if let Some(p) = w.parts.iter_mut().find(|p| p.slot == slot) {
            let steps = [50u16, 100, 150, 200];
            let at = steps.iter().position(|&s| s >= p.scale).unwrap_or(1);
            let next = if up { (at + 1).min(3) } else { at.saturating_sub(1) };
            changed |= steps[next] != p.scale;
            p.scale = steps[next];
        }
    }
    let longer = any(ws, &["longer", "extend", "lengthen", "taller"]);
    let shorter = any(ws, &["shorter", "shorten"]);
    if longer || shorter {
        match w.natural_len() {
            Some((nat, _, _)) => {
                let now = i32::from(w.stretch.unwrap_or(nat as u16));
                let want = (now + if longer { 24 } else { -24 }).clamp(1, 600) as u16;
                let before = w.stretch;
                let _ = w.set_stretch(Some(want));
                changed |= w.stretch != before;
            }
            None => {
                notes.push("nothing on this weapon stretches".into());
                wants.push(Want { slot: "limb".into(), look: "a longer bow".into(), stand_in: String::new() });
            }
        }
    }
    if any(ws, &["another", "different"]) {
        let slot = att.iter().map(|(s, _)| *s).find(|s| w.chosen(s).is_some());
        if let Some(slot) = slot {
            let cands = w.candidates(slot);
            let now = w.chosen(slot).map(|p| p.part_id.clone());
            if let Some(ix) = cands.iter().position(|p| Some(&p.id) == now.as_ref()) {
                let next = cands[(ix + 1) % cands.len()];
                changed |= w.equip(slot, Some(&next.id.clone())).is_ok();
            }
        }
    }
    changed
}

// ------------------------------------------------------------------------------------ creatures

fn edit_creature(
    c: &mut Creation,
    ws: &[String],
    fresh: bool,
    wants: &mut Vec<Want>,
    notes: &mut Vec<String>,
    fork: &mut Option<String>,
) -> bool {
    let mut changed = false;
    // Role by the words.
    let role = [
        (Role::Folk, FOLK_NOUNS),
        (Role::Minion, &["minion", "pet", "sidekick", "imp", "tiny", "little"][..]),
        (Role::Boss, &["boss", "legend", "colossal", "giant", "dragon", "king", "queen", "lord", "titan"][..]),
        (Role::Monster, &["monster", "beast", "creature", "ogre", "troll", "ghoul", "golem", "orc", "fiend", "brute"][..]),
        (Role::Hero, &["hero", "champion", "paladin", "knight", "mage", "wizard", "ranger", "warrior", "rogue", "witch"][..]),
    ]
    .into_iter()
    .find(|(_, l)| any(ws, l))
    .map(|(r, _)| r);
    if let Some(r) = role && c.role != r && !c.role.is_weapon() {
        c.role = r;
        changed = true;
    } else if c.role.is_weapon() {
        c.role = role.unwrap_or(Role::Hero);
        changed = true;
    }
    if c.role == Role::Folk {
        changed |= edit_folk(c, ws, fresh);
    }
    // A weapon named in a creature's wish is its own wish.
    if let Some(i) = pos(ws, WEAPON_NOUNS) && ws[i] != "weapon" {
        let from = i.saturating_sub(2);
        let phrase = ws[from..=i].iter().filter(|w| !STOP.contains(&w.as_str())).cloned().collect::<Vec<_>>().join(" ");
        *fork = Some(phrase);
    }
    // Parts.
    const PART_WORDS: &[(&str, &[&str])] = &[
        ("ember-mask", &["ember", "flame", "mask", "burning", "fire"]),
        ("stone-brow", &["stone", "brow", "rock", "granite", "ogre", "troll"]),
        ("seer-eye", &["eye", "seer", "cyclops", "watcher", "sight"]),
        ("ram-horns", &["horn", "ram", "horned", "goat", "bull"]),
        ("crown-horns", &["crown", "antler", "stag", "crowned"]),
        ("iron-plate", &["iron", "plate", "armour", "armor", "steel", "armoured", "knight"]),
        ("bark-mail", &["bark", "wooden", "wood", "tree", "vine", "mossy", "leaf"]),
        ("royal-mail", &["royal", "regal", "majestic", "king", "queen"]),
        ("raking-claws", &["claw", "talon", "rake", "cat", "scratch"]),
        ("ember-palms", &["palm", "caster", "spellcaster", "mage", "wizard"]),
        ("stone-fists", &["fist", "punch", "brawler", "smash", "slam", "brute"]),
        ("whip-tail", &["tail", "whip", "lizard"]),
        ("lantern-tail", &["lantern", "lamp", "glowing"]),
        ("wish-bell", &["bell", "chime", "rally"]),
        ("soul-ember", &["soul", "phoenix", "rekindle"]),
    ];
    for p in PARTS {
        let words = PART_WORDS.iter().find(|(id, _)| *id == p.id).map_or(&[][..], |(_, l)| *l);
        let slot_hit = ws.iter().any(|w| w == &p.slot.name().to_lowercase());
        let hit = words.iter().filter(|w| ws.iter().any(|x| x == *w)).count();
        // "royal armour" means the royal mail, "iron" alone only an armour.
        if hit > 0 && c.chosen(p.slot).is_none_or(|x| x.part_id != p.id) {
            // The part that names the most of its own words wins its slot.
            let rival = PARTS.iter().filter(|q| q.slot == p.slot && q.id != p.id).any(|q| {
                let qw = PART_WORDS.iter().find(|(id, _)| *id == q.id).map_or(&[][..], |(_, l)| *l);
                qw.iter().filter(|w| ws.iter().any(|x| x == *w)).count() > hit
            });
            if !rival && c.equip(p.slot, Some(p.id)).is_ok() {
                changed = true;
            }
        }
        let _ = slot_hit;
    }
    if any(ws, &["no", "without", "remove"]) {
        for (slot, word) in [(Slot::Horns, "horn"), (Slot::Tail, "tail"), (Slot::Armour, "armour"), (Slot::Charm, "charm")] {
            if any(ws, &[word]) && c.chosen(slot).is_some() {
                let _ = c.equip(slot, None);
                changed = true;
            }
        }
    }
    // Paint.
    const PIGMENTS: &[(Pigment, &[&str])] = &[
        (Pigment::Moss, &["moss", "mossy", "green", "frog", "toad", "swamp", "forest", "lizard"]),
        (Pigment::Slate, &["slate", "grey", "gray", "blue", "stone", "steel", "ghost"]),
        (Pigment::Russet, &["russet", "red", "rust", "rusty", "brown", "copper", "crimson", "fox"]),
        (Pigment::Bone, &["bone", "white", "pale", "ivory", "skeleton", "snow"]),
        (Pigment::Charcoal, &["charcoal", "black", "coal", "night", "shadow", "ink", "dark"]),
        (Pigment::Ochre, &["ochre", "amber", "tan", "yellow", "gold", "golden", "sand"]),
    ];
    let mut found: Vec<Pigment> = Vec::new();
    for w in ws {
        if let Some((p, _)) = PIGMENTS.iter().find(|(_, l)| l.contains(&w.as_str())) {
            if !found.contains(p) {
                found.push(*p);
            }
        }
    }
    if let Some(first) = found.first() {
        if c.paint.scheme != *first {
            c.paint.scheme = *first;
            changed = true;
        }
        if let Some(second) = found.get(1) && c.paint.accent != *second {
            c.paint.accent = *second;
            changed = true;
        }
    }
    let pattern = if any(ws, &["banded", "striped", "stripe", "tiger", "stripy"]) {
        Some(Pattern::Bands)
    } else if any(ws, &["spotted", "speckled", "dotted", "freckled", "spot"]) {
        Some(Pattern::Speckles)
    } else {
        None
    };
    if let Some(p) = pattern && c.paint.pattern != p {
        c.paint.pattern = p;
        changed = true;
    }
    // Sculpt.
    let s = &mut c.sculpt;
    let bump = |v: &mut u8, d: i32| *v = (i32::from(*v) + d).clamp(0, 100) as u8;
    let head_ws = pos(ws, &["head"]).is_some();
    for (list, d) in [(&["tall", "towering", "lanky", "taller"][..], 25), (&["short", "stout", "squat", "shorter"][..], -25)] {
        if any(ws, list) {
            bump(&mut s.height, d);
            changed = true;
        }
    }
    for (list, d) in [(&["bulky", "burly", "hulking", "fat", "heavy", "broad", "wider", "thicker"][..], 25), (&["slim", "thin", "skinny", "lean", "slender", "thinner", "slimmer", "leaner"][..], -25)] {
        if any(ws, list) {
            bump(&mut s.bulk, d);
            changed = true;
        }
    }
    if head_ws && any(ws, &["big", "bigger", "large", "huge", "larger"]) {
        bump(&mut s.head_size, 25);
        changed = true;
    }
    if head_ws && any(ws, &["small", "smaller", "tiny"]) {
        bump(&mut s.head_size, -25);
        changed = true;
    }
    if any(ws, &["long", "longer"]) && any(ws, &["arm", "limb", "leg"]) {
        bump(&mut s.limb_len, 25);
        changed = true;
    }
    if any(ws, &["bigger", "larger", "huge"]) && !head_ws {
        bump(&mut s.height, 15);
        bump(&mut s.bulk, 15);
        changed = true;
    }
    if any(ws, &["smaller", "tiny"]) && !head_ws {
        bump(&mut s.height, -15);
        bump(&mut s.bulk, -15);
        changed = true;
    }
    // Magic: a school with a way of casting it.
    let schoolly = school_of(ws);
    let castly = any(ws, &["cast", "spell", "bolt", "nova", "beam", "ward", "summon", "aura", "breath", "shoot", "hurl", "blast", "casts", "magic", "shoots", "breathes", "throws"]);
    if let Some(school) = schoolly.filter(|_| castly || pos(ws, &["spell"]).is_some()) {
        let form = [
            (Form::Nova, &["nova", "burst", "explode", "blast"][..]),
            (Form::Beam, &["beam", "ray", "breath"][..]),
            (Form::Ward, &["ward", "shield", "barrier"][..]),
            (Form::Summon, &["summon", "call"][..]),
            (Form::Aura, &["aura"][..]),
            (Form::Bolt, &["bolt", "shoot", "hurl", "throw"][..]),
        ]
        .into_iter()
        .find(|(_, l)| any(ws, l))
        .map_or(Form::Bolt, |(f, _)| f);
        let want = Spell { school, form, element: school, power: 1 };
        if c.spells.first() != Some(&want) {
            c.spells = vec![want];
            changed = true;
        }
    }
    // What the catalogue lacks.
    let unknown: Vec<&String> = ws.iter().filter(|w| !known(w) && !PIGMENTS.iter().any(|(_, l)| l.contains(&w.as_str()))).collect();
    if !unknown.is_empty() && fresh {
        let slot = if any(ws, &["head", "face"]) { "head" } else { "head" };
        let look = format!("{} {slot}", unknown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" "));
        let stand_in = c.chosen(Slot::Head).map_or_else(String::new, |p| p.part_id.clone());
        wants.push(Want { slot: slot.into(), look, stand_in });
        changed = true;
    }
    let _ = notes;
    changed
}

// ------------------------------------------------------------------------------------ haggling

/// A boss-sized wish: trim what costs most until it fits, and say what went. Never a refusal.
fn haggle(spec: &mut Spec) {
    let mut steps = Vec::new();
    match &mut spec.make {
        Make::Weapon(w) => {
            let mut guard = 0;
            while w.spent() > WEAPON_BUDGET && guard < 12 {
                guard += 1;
                if let Some(big) = w.parts.iter_mut().filter(|p| p.scale > 100).max_by_key(|p| p.scale) {
                    let name = kit().part(&big.part_id).map_or(String::new(), part_name);
                    big.scale = if big.scale > 150 { 150 } else { 100 };
                    steps.push(format!("shrank the {name}"));
                    continue;
                }
                if let Some(g) = w.gem {
                    steps.push(format!("left off the {} gem", g.name().to_lowercase()));
                    w.gem = None;
                    continue;
                }
                let mut best: Option<(i32, String, String)> = None;
                for wp in w.parts.clone() {
                    let Some(cur) = kit().part(&wp.part_id) else { continue };
                    let alts = w.candidates(&wp.slot);
                    if let Some(a) = alts
                        .iter()
                        .filter(|a| a.stat.cost < cur.stat.cost && !(wp.slot == "head" && w.kind.is_focus()))
                        .max_by_key(|a| a.stat.cost)
                    {
                        let saved = cur.stat.cost - a.stat.cost;
                        if best.as_ref().is_none_or(|b| saved > b.0) {
                            best = Some((saved, wp.slot.clone(), a.id.clone()));
                        }
                    }
                }
                let Some((_, slot, id)) = best else { break };
                let old = w.chosen(&slot).and_then(|p| kit().part(&p.part_id)).map_or(String::new(), part_name);
                let new = kit().part(&id).map_or(String::new(), part_name);
                if w.equip(&slot, Some(&id)).is_ok() {
                    steps.push(format!("swapped the {old} for a {new}"));
                } else {
                    break;
                }
            }
        }
        Make::Creature(c) => {
            let mut guard = 0;
            while c.spent() > c.role.budget() && guard < 12 {
                guard += 1;
                if let Some(s) = c.spells.pop() {
                    steps.push(format!("left out the {} {}", s.school.name().to_lowercase(), s.form.name().to_lowercase()));
                    continue;
                }
                let costly = c
                    .parts
                    .iter()
                    .filter(|p| p.mount != Slot::Head)
                    .filter_map(|p| part(&p.part_id))
                    .max_by_key(|p| p.cost);
                let Some(p) = costly else { break };
                let cheaper = PARTS.iter().filter(|q| q.slot == p.slot && q.cost < p.cost).max_by_key(|q| q.cost);
                match cheaper {
                    Some(q) => {
                        steps.push(format!("swapped {} for {}", p.name.to_lowercase(), q.name.to_lowercase()));
                        let _ = c.equip(p.slot, Some(q.id));
                    }
                    None => {
                        steps.push(format!("took off the {}", p.name.to_lowercase()));
                        let _ = c.equip(p.slot, None);
                    }
                }
            }
        }
    }
    spec.trimmed = steps;
}

// ------------------------------------------------------------------------------------ the answer

fn speak(spec: &mut Spec, ws: &[String], refine: bool, changed: bool) {
    let seed = spec.wish.clone();
    let open = pick(&seed, &["Behold!", "As you wish!", "Mm. Yes.", "Done, and done.", "A fine whim!", "Easily done."]);
    let (name, parts): (String, Vec<String>) = match &spec.make {
        Make::Weapon(w) => (
            w.name.clone(),
            w.parts
                .iter()
                .filter_map(|p| kit().part(&p.part_id))
                .filter(|p| !matches!(p.slot.as_str(), "pommel" | "grip") || p.cost() > 1)
                .take(3)
                .map(part_name)
                .collect(),
        ),
        Make::Creature(c) => (
            c.name.clone(),
            c.parts.iter().filter_map(|p| part(&p.part_id)).take(3).map(|p| p.name.to_lowercase()).collect(),
        ),
    };
    let mut lines: Vec<String> = Vec::new();
    if !changed && refine {
        lines.push("I heard no change in that. Say what to alter: bigger, longer, gold, glow blue.".into());
    } else if refine {
        lines.push(format!("{open} {name}, mended."));
    } else {
        lines.push(format!("{open} {name}: {}.", parts.join(", ")));
    }
    for w in &spec.wants {
        lines.push(format!(
            "No {} in my lamp. The painters will make one; for now, {}.",
            w.look,
            kit().part(&w.stand_in).map_or_else(|| "what you have".into(), part_name)
        ));
    }
    if let Some(f) = &spec.fork {
        lines.push(format!("Creatures hold nothing. Wish the {f} at the forge."));
    }
    if let Some(n) = spec.notes.first() {
        lines.push(format!("{}.", caps(n)));
    }
    if !spec.trimmed.is_empty() {
        let who = match &spec.make {
            Make::Weapon(_) => "A weapon".to_string(),
            Make::Creature(c) => format!("A {}", c.role.name().to_lowercase()),
        };
        lines.push(format!("{who} only holds {} power! I {}.", spec.budget, spec.trimmed.join(", ")));
    }
    let (cost, budget) = budget_of(&spec.make);
    let over = cost > budget;
    lines.push(if over {
        format!("{cost} OF {budget} POWER: still too grand. Trim it yourself.")
    } else {
        format!("{cost} OF {budget} POWER.")
    });
    spec.reply = lines.join(" ");
    spec.quip = if !spec.trimmed.is_empty() {
        "Too grand!".into()
    } else if any(ws, &["plunger"]) {
        "Ha! A plunger.".into()
    } else if !spec.wants.is_empty() {
        "I need paint!".into()
    } else {
        pick(&seed, &["Lovely!", "Behold!", "Mm, yes.", "Hah!", "Done."]).into()
    };
}

#[cfg(test)]
#[path = "../../../tests/rules/wish__tests.rs"]
mod tests;
