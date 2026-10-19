//! A pickup on the floor as a few solid shapes, in the same pipeline as a shot
//! (docs/GLASS_CORE.md, "3D shots"): the window and the browser both draw these
//! nodes, in the inks of the card's sprite, from the built-in meshes.
//!
//! The world is the stage's. `x` is across the room, `z` down it (the room's
//! `y`), `y` the height off the floor. A pickup sits [`LIFT`] over its floor
//! point, bobs a little and turns slowly. Both are a function of the run's
//! tick, so nothing is kept between frames.
//!
//! The realm sits at dusk, so a pickup is an object lit by the scene's one
//! light in the dusk's inks ([`at_dusk`]), never a lamp: steel, wood, bone and
//! stone take the light as they are. Only the rare and the valuable (a gem, a
//! rune, an ember, a spell, a staff's orb) shine, and then faintly ([`FAINT`]),
//! a coin or a heart a glint ([`GLINT`]). A rune's ring and any floor shadow
//! stay layers of the room, under the shape, the way a shot keeps its halo.

use angel_stage::scene::{MeshRef, Material, Node, QUAT_IDENTITY, Transform, quat_axis_angle, quat_mul};

use crate::render::overworld::ink::{Img, ink};
use crate::rules::cards::{Arm, Card, Effect, Kind};
use crate::rules::Weapon;
use crate::rules::runes::RuneKind;
use crate::realm::Spoil;

/// How much a ball is flattened in depth so the oblique camera shows it round.
pub const DEPTH: f32 = 0.45;

/// How far a pickup sits off the floor, before its bob.
pub const LIFT: f32 = 4.0;

/// First node id of the pickup block. Shots own `0x5_0000_0000`; this does not meet them.
pub const FIRST_NODE: u64 = 0x6_0000_0000;

/// Ids reserved for one pickup, more than [`BUDGET`] so a later pickup cannot collide.
pub const NODES_PER: u64 = 16;

/// The most nodes one pickup may spend.
pub const BUDGET: usize = 12;

/// How much of its colour a rare or valuable thing gives back by itself: a
/// faint lift on the faces the light misses, no more.
pub const FAINT: f32 = 0.2;

/// What a coin, a heart or any other shining thing gives back: a glint.
pub const GLINT: f32 = 0.1;

/// What lies on the floor, enough to pick a shape. A card's own inks colour it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupKind {
    Bow,
    Blade,
    Axe,
    Staff,
    Spear,
    Hammer,
    Dagger,
    Crossbow,
    Gun,
    Weapon,
    Shield,
    Spell,
    Flask,
    Food,
    Heart,
    Gold,
    Bone,
    Wax,
    Ore,
    Gem,
    Ember,
    Scale,
    Gear,
    Key,
    Rune(RuneKind),
    Chest,
    Pouch,
}

/// A recipe for one card of the book: a family, whether it shines, and two inks
/// taken from its floor sprite (a family default when the sprite is plain).
#[derive(Clone, Copy, Debug)]
pub struct Recipe {
    pub kind: PickupKind,
    pub shine: bool,
    pub ink: char,
    pub accent: char,
}

/// One thing lying on the floor, already in room pixels, ready for the props block.
#[derive(Clone, Debug)]
pub struct ScenePickup {
    pub x: f32,
    pub y: f32,
    pub kind: PickupKind,
    /// Its drawing on the plate sheet when its card names one
    /// (`pickup_cards::plate_for`); else its kind's.
    pub plate: Option<u8>,
    pub ink: char,
    pub accent: char,
    pub shine: bool,
    pub tick: u32,
}

/// The family a built-in card falls into. Every card gets at least a family default.
pub fn recipe(card: &Card) -> Recipe {
    let words = words_of(card);
    let (kind, shine) = family(card, &words);
    let (fallback, accent) = family_inks(kind);
    let (ink, accent) = sprite_inks(&crate::render::overworld::arena::cards::floor_plate(Some(card), 0), fallback, accent);
    Recipe { kind, shine: shine || magic_words(&words), ink, accent }
}

fn words_of(card: &Card) -> String {
    let mut s = format!("{} {}", card.id, card.name).to_ascii_lowercase();
    if let Some(Arm::Forged(w)) = &card.arm {
        s.push(' ');
        s.push_str(&w.shape.to_ascii_lowercase());
        s.push(' ');
        s.push_str(&w.name.to_ascii_lowercase());
    }
    if let Some(Arm::Plain(w)) = &card.arm {
        s.push(' ');
        s.push_str(match w {
            Weapon::Bow => "bow",
            Weapon::Crossbow => "crossbow",
            Weapon::Handgonne => "handgonne",
            Weapon::Fists => "fists",
            Weapon::Notes => "notes",
            Weapon::Stones => "stones",
        });
    }
    s
}

fn magic_words(words: &str) -> bool {
    ["magic", "arcane", "enchant", "hex", "holy", "fae", "rune"].iter().any(|w| words.contains(w))
}

fn family(card: &Card, words: &str) -> (PickupKind, bool) {
    if words.contains("key") {
        return (PickupKind::Key, false);
    }
    if let Some((spoil, _)) = card.effects.iter().find_map(|e| e.spoil()) {
        return (spoil_kind(spoil), matches!(spoil, Spoil::Gem | Spoil::Ember));
    }
    if card.effects.iter().any(|e| matches!(e, Effect::Gold(_))) || words.contains("gold") || words.contains("coin") {
        return (PickupKind::Gold, true);
    }
    if card.kind == Kind::Spell {
        return (PickupKind::Spell, true);
    }
    if card.kind == Kind::Guard {
        return (PickupKind::Shield, false);
    }
    if card.kind == Kind::Arm || card.arm.is_some() {
        return (weapon_kind(words), false);
    }
    if flask_words(words) || card.effects.iter().any(|e| matches!(e, Effect::Heal(_))) && card.kind == Kind::Take {
        return (PickupKind::Flask, true);
    }
    if words.contains("heart") || card.effects.iter().any(|e| matches!(e, Effect::MaxHp(_))) {
        return (PickupKind::Heart, true);
    }
    if food_words(words) {
        return (PickupKind::Food, false);
    }
    if card.kind == Kind::Hold || gear_words(words) {
        return (PickupKind::Gear, false);
    }
    (PickupKind::Pouch, false)
}

fn spoil_kind(spoil: Spoil) -> PickupKind {
    match spoil {
        Spoil::Gold => PickupKind::Gold,
        Spoil::Bone => PickupKind::Bone,
        Spoil::Wax => PickupKind::Wax,
        Spoil::Ore => PickupKind::Ore,
        Spoil::Gem => PickupKind::Gem,
        Spoil::Ember => PickupKind::Ember,
        Spoil::Scale => PickupKind::Scale,
        Spoil::Bond => PickupKind::Gear,
    }
}

fn weapon_kind(words: &str) -> PickupKind {
    if words.contains("crossbow") {
        PickupKind::Crossbow
    } else if words.contains("handgonne") || words.contains("gonne") || words.contains("gun") {
        PickupKind::Gun
    } else if words.contains("staff") || words.contains("sceptre") || words.contains("scepter") || words.contains("wand") {
        PickupKind::Staff
    } else if words.contains("axe") {
        PickupKind::Axe
    } else if words.contains("spear") || words.contains("lance") || words.contains("pike") {
        PickupKind::Spear
    } else if words.contains("hammer") || words.contains("mace") || words.contains("maul") {
        PickupKind::Hammer
    } else if words.contains("dagger") || words.contains("knife") || words.contains("dirk") {
        PickupKind::Dagger
    } else if words.contains("bow") {
        PickupKind::Bow
    } else if words.contains("blade") || words.contains("sword") || words.contains("saber") || words.contains("sabre") {
        PickupKind::Blade
    } else if words.contains("melee") {
        PickupKind::Blade
    } else {
        PickupKind::Weapon
    }
}

fn flask_words(words: &str) -> bool {
    ["potion", "draught", "draft", "phial", "vial", "flask", "elixir", "brew", "sip"].iter().any(|w| words.contains(w))
}

fn food_words(words: &str) -> bool {
    ["bread", "loaf", "apple", "pie", "meat", "feast", "ration", "cheese", "stew", "food", "meal"].iter().any(|w| words.contains(w))
}

fn gear_words(words: &str) -> bool {
    ["boot", "helm", "hat", "ring", "cloak", "mail", "armour", "armor", "glove", "belt", "amulet"].iter().any(|w| words.contains(w))
}

fn family_inks(kind: PickupKind) -> (char, char) {
    match kind {
        PickupKind::Bow | PickupKind::Staff | PickupKind::Spear => ('Z', 'r'),
        PickupKind::Blade | PickupKind::Axe | PickupKind::Hammer | PickupKind::Dagger | PickupKind::Crossbow | PickupKind::Gun | PickupKind::Weapon | PickupKind::Key => ('H', 'Z'),
        PickupKind::Shield => ('J', 'r'),
        PickupKind::Spell => ('v', 'w'),
        PickupKind::Flask => ('q', 'c'),
        PickupKind::Food => ('o', 'r'),
        PickupKind::Heart => ('r', 'R'),
        PickupKind::Gold => ('4', 'y'),
        PickupKind::Bone => ('c', 'H'),
        PickupKind::Wax => ('y', 'o'),
        PickupKind::Ore => ('g', 'H'),
        PickupKind::Gem => ('z', 'w'),
        PickupKind::Ember => ('r', 'y'),
        PickupKind::Scale => ('f', 'G'),
        PickupKind::Gear => ('4', 'H'),
        PickupKind::Chest => ('Z', '4'),
        PickupKind::Pouch => ('Z', 'o'),
        PickupKind::Rune(kind) => rune_inks(kind),
    }
}

fn rune_inks(kind: RuneKind) -> (char, char) {
    match kind {
        RuneKind::Haste => ('y', 'w'),
        RuneKind::DoubleDamage => ('r', '4'),
        RuneKind::Regeneration => ('f', 'w'),
        RuneKind::Arcane => ('v', 'w'),
        RuneKind::Invisibility => ('z', 'H'),
        RuneKind::Illusion => ('p', 'w'),
        RuneKind::Bounty => ('4', 'y'),
        RuneKind::Wisdom => ('q', 'w'),
    }
}

/// The two inks a sprite uses most, ignoring near-black floor, or the defaults.
pub fn sprite_inks(im: &Img, fallback: char, accent: char) -> (char, char) {
    let mut counts: Vec<(char, u32)> = Vec::new();
    for px in im.pixels() {
        let Some(ch) = char_of(px) else { continue };
        if matches!(ch, 'k' | 'K') {
            continue;
        }
        if let Some(slot) = counts.iter_mut().find(|(c, _)| *c == ch) {
            slot.1 += 1;
        } else {
            counts.push((ch, 1));
        }
    }
    counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    match counts.as_slice() {
        [(a, _), (b, _), ..] => (*a, *b),
        [(a, _), ..] => (*a, accent),
        _ => (fallback, accent),
    }
}

fn char_of(px: [u8; 3]) -> Option<char> {
    const INKS: &str = "kKZXGgjJhHiHfFnoOyYrRvVmMpPqQzZ";
    INKS.chars().find(|&ch| ink(ch) == Some(px))
}

/// An ink as it lies on the floor at dusk: lamps, cream and near-white step
/// down to the realm's brass and quiet greys (the pattern of
/// `light::dusk_ink`), so nothing on the floor out-shines the stone it lies
/// on, and each keeps its family (gold stays brass, steel grey, magic a cool
/// slate) to be told apart by colour. Every answer is at most the brightness
/// of brass.
pub fn at_dusk(ch: char) -> char {
    match ch {
        '4' | '5' | '6' | '@' | 'T' | 't' => 'O',
        'w' | '3' => 'v',
        '9' | 'c' | '$' | 'H' | 'i' => 'h',
        'W' | 'V' => 'v',
        'Y' => 'y',
        '2' => 'U',
        '1' => 'q',
        'a' => 'r',
        other => other,
    }
}

struct Paint {
    ink: char,
    emissive: f32,
}

impl Paint {
    const fn lit(ink: char) -> Self {
        Self { ink, emissive: 0.0 }
    }

    const fn glow(ink: char, emissive: f32) -> Self {
        Self { ink, emissive }
    }

    fn material(self) -> Material {
        let [r, g, b] = ink(at_dusk(self.ink)).unwrap_or([255, 0, 255]);
        Material::lit([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0], self.emissive)
    }
}

struct Maker<'a> {
    out: &'a mut Vec<Node>,
    next: u64,
    at: [f32; 3],
    yaw: [f32; 4],
}

impl Maker<'_> {
    fn push(&mut self, mesh: MeshRef, offset: [f32; 3], turn: [f32; 4], size: [f32; 3], paint: Paint) {
        let rotation = quat_mul(self.yaw, turn);
        let p = angel_stage::scene::quat_rotate(self.yaw, offset);
        let mut node = Node::new(self.next, mesh, paint.material());
        node.transform = Transform {
            translation: [self.at[0] + p[0], self.at[1] + p[1], self.at[2] + p[2]],
            rotation,
            scale: size,
        };
        self.next += 1;
        self.out.push(node);
    }

    fn ball(&mut self, offset: [f32; 3], d: f32, paint: Paint) {
        self.push(MeshRef::Sphere { segments: 10 }, offset, QUAT_IDENTITY, [d, d, d * DEPTH], paint);
    }

    fn box_at(&mut self, offset: [f32; 3], size: [f32; 3], paint: Paint) {
        self.push(MeshRef::Cube, offset, QUAT_IDENTITY, size, paint);
    }

    /// A shaft: a thin cube standing up. The built-in set has no cylinder.
    fn rod(&mut self, offset: [f32; 3], h: f32, r: f32, paint: Paint) {
        self.box_at(offset, [r, h, r * DEPTH], paint);
    }

    fn gem(&mut self, offset: [f32; 3], size: [f32; 3], paint: Paint) {
        self.push(MeshRef::Gem, offset, QUAT_IDENTITY, size, paint);
    }

    fn point(&mut self, offset: [f32; 3], width: f32, len: f32, paint: Paint) {
        self.push(MeshRef::Cone { segments: 6 }, offset, QUAT_IDENTITY, [width, len, width * DEPTH], paint);
    }
}

/// Build one pickup into `out`. `n` is its index in the room. Returns whether a shape was added.
pub fn build(out: &mut Vec<Node>, n: u64, item: &ScenePickup, centre: (f32, f32)) -> bool {
    let before = out.len();
    let t = item.tick as f32;
    let bob = (t * 0.18).sin() * 1.4;
    let yaw = quat_axis_angle([0.0, 1.0, 0.0], t * 0.04);
    let mut maker = Maker {
        out,
        next: FIRST_NODE + n * NODES_PER,
        at: [item.x - centre.0, LIFT + bob, item.y - centre.1],
        yaw,
    };
    let ink = item.ink;
    let accent = item.accent;
    let lit = |ch| Paint::lit(ch);
    let hot = |ch| if item.shine { Paint::glow(ch, GLINT) } else { Paint::lit(ch) };
    match item.kind {
        PickupKind::Bow => {
            maker.box_at([-1.6, 0.2, 0.0], [0.6, 6.0, 0.45], lit(ink));
            maker.box_at([1.6, 0.2, 0.0], [0.6, 6.0, 0.45], lit(ink));
            maker.rod([0.0, 0.0, 0.0], 6.4, 0.25, lit(accent));
        }
        PickupKind::Blade | PickupKind::Weapon => {
            maker.box_at([0.0, 1.5, 0.0], [1.1, 8.0, 0.4], lit(ink));
            maker.box_at([0.0, -3.2, 0.0], [2.4, 0.6, 0.6], lit(accent));
            maker.ball([0.0, -4.2, 0.0], 1.2, lit('4'));
        }
        PickupKind::Axe => {
            maker.rod([0.0, -1.0, 0.0], 7.0, 0.45, lit(accent));
            maker.box_at([1.2, 2.4, 0.0], [2.2, 2.6, 0.5], lit(ink));
        }
        PickupKind::Staff => {
            maker.rod([0.0, -0.5, 0.0], 9.0, 0.4, lit(accent));
            maker.ball([0.0, 4.6, 0.0], 2.2, Paint::glow(ink, FAINT));
        }
        PickupKind::Spear => {
            maker.rod([0.0, -1.0, 0.0], 9.0, 0.35, lit(accent));
            maker.point([0.0, 4.2, 0.0], 1.2, 2.4, lit(ink));
        }
        PickupKind::Hammer => {
            maker.rod([0.0, -1.2, 0.0], 6.0, 0.45, lit(accent));
            maker.box_at([0.0, 2.6, 0.0], [3.4, 1.6, 1.2], lit(ink));
        }
        PickupKind::Dagger => {
            maker.box_at([0.0, 0.6, 0.0], [0.7, 5.0, 0.3], lit(ink));
            maker.box_at([0.0, -2.4, 0.0], [1.8, 0.45, 0.45], lit(accent));
        }
        PickupKind::Crossbow => {
            maker.box_at([0.0, 0.0, 0.0], [6.0, 0.8, 1.2], lit(accent));
            maker.rod([2.2, 0.6, 0.0], 3.2, 0.3, lit(ink));
        }
        PickupKind::Gun => {
            maker.box_at([1.0, 0.4, 0.0], [5.0, 0.9, 0.9], lit(ink));
            maker.box_at([-1.4, -0.6, 0.0], [1.6, 1.4, 0.8], lit(accent));
        }
        PickupKind::Shield => {
            maker.box_at([0.0, 0.0, 0.0], [5.2, 6.0, 0.5], lit(ink));
            maker.gem([0.0, 0.2, 0.3], [1.4, 1.4, 0.6], hot(accent));
        }
        PickupKind::Spell => {
            maker.box_at([0.0, -0.6, 0.0], [3.4, 0.35, 2.4], lit('c'));
            maker.ball([0.0, 1.4, 0.0], 2.4, Paint::glow(ink, FAINT));
        }
        PickupKind::Flask => {
            maker.rod([0.0, -0.4, 0.0], 3.2, 1.3, hot(ink));
            maker.ball([0.0, 1.8, 0.0], 1.1, lit(accent));
        }
        PickupKind::Food => {
            maker.ball([0.0, 0.0, 0.0], 3.2, lit(ink));
            maker.box_at([0.0, 1.4, 0.0], [2.2, 0.4, 1.6], lit(accent));
        }
        PickupKind::Heart => {
            maker.ball([-0.8, 0.4, 0.0], 2.0, Paint::glow(ink, GLINT));
            maker.ball([0.8, 0.4, 0.0], 2.0, Paint::glow(ink, GLINT));
        }
        PickupKind::Gold => {
            maker.box_at([0.0, 0.0, 0.0], [2.6, 0.4, 2.6], Paint::glow(ink, GLINT));
        }
        PickupKind::Bone => {
            maker.box_at([0.0, 0.0, 0.0], [3.6, 0.7, 0.7], lit(ink));
            maker.ball([-1.6, 0.0, 0.0], 1.1, lit(ink));
            maker.ball([1.6, 0.0, 0.0], 1.1, lit(ink));
        }
        PickupKind::Wax => maker.ball([0.0, 0.0, 0.0], 2.4, lit(ink)),
        PickupKind::Ore => maker.box_at([0.0, 0.0, 0.0], [2.6, 1.8, 2.0], lit(ink)),
        PickupKind::Gem => maker.gem([0.0, 0.4, 0.0], [2.2, 2.4, 2.2 * DEPTH], Paint::glow(ink, FAINT)),
        PickupKind::Ember => maker.ball([0.0, 0.2, 0.0], 1.8, Paint::glow(ink, FAINT)),
        PickupKind::Scale => maker.box_at([0.0, 0.0, 0.0], [2.8, 0.35, 2.0], lit(ink)),
        PickupKind::Gear => {
            maker.box_at([0.0, 0.0, 0.0], [2.2, 0.45, 2.2], lit(ink));
            maker.gem([0.0, 0.4, 0.0], [1.2, 1.2, 0.5], hot(accent));
        }
        PickupKind::Key => {
            maker.rod([0.0, 0.4, 0.0], 4.2, 0.35, lit(ink));
            maker.box_at([0.5, -1.8, 0.0], [1.2, 0.4, 0.35], lit(accent));
            maker.gem([0.0, 2.6, 0.0], [1.3, 1.3, 0.4], lit(accent));
        }
        PickupKind::Rune(_) => {
            maker.box_at([0.0, -1.0, 0.0], [4.4, 0.35, 4.4], lit(accent));
            maker.ball([0.0, 0.6, 0.0], 2.0, Paint::glow(ink, FAINT));
        }
        PickupKind::Chest => {
            maker.box_at([0.0, -0.4, 0.0], [6.0, 3.2, 4.0], lit(ink));
            maker.box_at([0.0, 1.8, 0.0], [6.2, 1.2, 4.2], lit(accent));
        }
        PickupKind::Pouch => {
            maker.ball([0.0, 0.0, 0.0], 2.8, lit(ink));
            maker.box_at([0.0, 1.5, 0.0], [1.6, 0.35, 0.4], lit(accent));
        }
    }
    let spent = out.len() - before;
    debug_assert!(spent <= BUDGET);
    spent > 0 && spent <= BUDGET
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__pickup_meshes_tests.rs"]
mod tests;
