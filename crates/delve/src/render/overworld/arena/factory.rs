//! Checked, compiled factory assets. Nothing here adds a save or wire field.
//!
//! Bodies travel through the ordinary Book/worn Sprite path; their exact
//! checked source rows identify the full quick rig on every rendering client.
//! Props, tiles and effects use [`with_placements`] around the normal arena
//! frame. This is also the preview path: no second compositor is involved.
//!
//! ```no_run
//! use angel_delve::render::overworld::arena::{self, factory};
//! # fn show(run: &mut angel_delve::rules::Run) -> Result<(), Box<dyn std::error::Error>> {
//! let catalog = factory::catalog()?;
//! catalog.install(&mut run.book, factory::Replace::No)?;
//! catalog.get("marsh-watchman").ok_or("missing body")?
//!     .apply(run, 1, factory::Replace::No)?;
//! catalog.get("chain-flail").ok_or("missing weapon")?
//!     .apply(run, 1, factory::Replace::No)?;
//! catalog.get("rivet-cuirass").ok_or("missing torso")?
//!     .apply(run, 1, factory::Replace::No)?;
//! let props = [factory::Placement::new(
//!     catalog.get("tavern-cask").ok_or("missing prop")?, 12.0, 10.0, 0, false)?];
//! let image = factory::with_placements(&props, ||
//!     arena::frame_for(run, arena::NATIVE_W, arena::NATIVE_H, Some(1)));
//! # let _ = image;
//! # Ok(())
//! # }
//! ```
//!
//! `Placement::attack(weapon, x, y, elapsed, mirrored)` consumes the checked
//! swing VFX in the same path. `weapon.attack()` exposes its clip and existing
//! sound pick; presentation clients own sound timing, not the renderer.
//!
//! From `crates/`, after the factory supplies its real compiled catalog:
//! `ANGEL_ARENA_SHOTS=/absolute/shots cargo test -p angel-delve --lib
//! write_factory_shots -- --ignored` writes `<asset-id>-1x.png` and `-4x.png`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use serde::Deserialize;

use super::quick_rig::{Clip, Rig};
use super::{Img, Run};
use crate::rules::cards::{self, Book, Card};
use crate::rules::looks::{self, Slot, Sprite};

include!(concat!(env!("OUT_DIR"), "/factory_sounds.rs"));

/// The native catalog supplied by the factory, parsed once, atomically.
pub fn catalog() -> Result<&'static Catalog, &'static Error> {
    static CATALOG: OnceLock<Result<Catalog, Error>> = OnceLock::new();
    CATALOG
        .get_or_init(|| Catalog::parse(include_str!("../../../../assets/factory/catalog.json")))
        .as_ref()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
fn fail(message: impl Into<String>) -> Error {
    Error(message.into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Body,
    Weapon,
    Worn,
    Vfx,
    Prop,
    Tile,
}

/// Checked attack metadata. Sound remains a game sound pick, not a local path.
#[derive(Clone, Debug)]
pub struct Attack {
    clip: Clip,
    sound: String,
    vfx: Sprite,
}
impl Attack {
    pub fn clip(&self) -> Clip {
        self.clip
    }
    pub fn sound(&self) -> &str {
        &self.sound
    }
    pub fn vfx(&self) -> &Sprite {
        &self.vfx
    }
}

/// Immutable after checking; callers cannot replace rows behind a checked rig.
#[derive(Clone, Debug)]
pub struct Asset {
    id: String,
    name: String,
    brief: String,
    kind: Kind,
    sprite: Sprite,
    pivot: [i32; 2],
    anchors: BTreeMap<String, [i32; 2]>,
    part: String,
    rig: Option<Arc<Rig>>,
    attack: Option<Attack>,
    card: Option<Card>,
}
impl Asset {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn brief(&self) -> &str {
        &self.brief
    }
    pub fn kind(&self) -> Kind {
        self.kind
    }
    /// Ordinary source Sprite, not a padded rig canvas. Clone to own it.
    pub fn sprite(&self) -> &Sprite {
        &self.sprite
    }
    pub fn pivot(&self) -> [i32; 2] {
        self.pivot
    }
    pub fn anchors(&self) -> &BTreeMap<String, [i32; 2]> {
        &self.anchors
    }
    pub fn part(&self) -> &str {
        &self.part
    }
    pub fn rig(&self) -> Option<&Arc<Rig>> {
        self.rig.as_ref()
    }
    pub fn attack(&self) -> Option<&Attack> {
        self.attack.as_ref()
    }
    pub fn card_id(&self) -> String {
        format!("factory-{}", self.id)
    }

    /// Generate a normal checked card. Weapon runes use the existing Forge
    /// checker; armour and bodies are hold cards, not new equipment kinds.
    pub fn card(&self) -> Result<Card, Error> {
        self.card
            .clone()
            .ok_or_else(|| fail(format!("{} is not equipment", self.id)))
    }

    fn build_card(&self) -> Result<Card, Error> {
        let raw = match self.kind {
            Kind::Body | Kind::Worn => format!(
                "name {}\nkind hold\ntext {}\narmor 1\n",
                self.name, self.brief
            ),
            Kind::Weapon => format!(
                "name {}\nkind arm\ntext {}\nlook steel blade\nmelee damage=10 reach=2 arc=120 every=45\n",
                self.name, self.brief
            ),
            _ => return Err(fail(format!("{} is not equipment", self.id))),
        };
        let mut card = cards::check(&self.card_id(), &raw)
            .map_err(|e| fail(e.errors.join("; ")))?
            .card;
        let mut sprite = self.sprite.clone();
        if self.kind == Kind::Body {
            // Do not run source rows through place_on_body: that would change
            // the exact idle-row identity (and lose the full rig association).
            sprite.at = Some("whole".into());
        } else {
            let part = if self.kind == Kind::Weapon {
                "right"
            } else {
                &self.part
            };
            sprite.frames = looks::place_on_body(&sprite.frames, part).map_err(fail)?;
            sprite.at = Some(part.into());
        }
        card.looks.worn = Some(sprite);
        Ok(card)
    }
}

impl Asset {
    /// Install and apply equipment immediately using the normal Book and Hero
    /// equipment transition. Reapplying is idempotent. A body replaces older factory bodies; a worn
    /// piece replaces factory clothing on the same part. Other wishes remain.
    pub fn apply(&self, run: &mut Run, hero: u32, replace: Replace) -> Result<String, Error> {
        if !run.players.contains_key(&hero) {
            return Err(fail("no such hero"));
        }
        let card = self.card()?;
        if replace == Replace::No && run.book.get(&card.id).is_some_and(|old| old != &card) {
            return Err(fail("card already exists; replacement must be explicit"));
        }
        let displaced = if matches!(self.kind, Kind::Body | Kind::Worn) {
            run.book
                .cards
                .iter()
                .filter(|c| {
                    c.id.starts_with("factory-")
                        && c.looks.worn.as_ref().is_some_and(|s| {
                            s.at.as_deref()
                                == Some(if self.kind == Kind::Body {
                                    "whole"
                                } else {
                                    &self.part
                                })
                        })
                })
                .map(|c| c.id.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if !run.book.insert(card.clone()) {
            return Err(fail("Book is full"));
        }
        let h = run.players.get_mut(&hero).expect("checked");
        h.deck
            .retain(|id| id != &card.id && !displaced.contains(id));
        h.deck.push(card.id.clone());
        if self.kind == Kind::Weapon {
            h.equip(&card);
        }
        h.rebonus(&run.book);
        Ok(card.id)
    }
}

#[derive(Clone, Debug)]
pub struct Catalog {
    assets: BTreeMap<String, Asset>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    version: u32,
    assets: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    name: String,
    brief: String,
    kind: Kind,
    frames: Vec<Vec<String>>,
    pivot: [i32; 2],
    #[serde(deserialize_with = "unique_anchors")]
    anchors: BTreeMap<String, [i32; 2]>,
    #[serde(default = "body_part")]
    part: String,
    rig: Option<serde_json::Value>,
    attack: Option<AttackExport>,
}
fn body_part() -> String {
    "body".into()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttackExport {
    clip: Clip,
    sound: String,
    vfx: Vec<Vec<String>>,
}

/// Use the existing Sprite gate, including palette, frame count and slot caps.
fn sprite(frames: &[Vec<String>], slot: Slot) -> Result<Sprite, Error> {
    let first = frames.first().ok_or_else(|| fail("no frames"))?;
    let h = first.len();
    let w = first.first().map_or(0, String::len);
    if w == 0
        || h == 0
        || frames.iter().any(|f| {
            f.len() != h
                || f.iter()
                    .any(|r| r.len() != w || !r.is_ascii() || r.contains('|'))
        })
    {
        return Err(fail(
            "frames must be nonempty, rectangular ASCII ink rows of equal size",
        ));
    }
    let rows = (0..h)
        .map(|y| {
            frames
                .iter()
                .map(|f| f[y].as_str())
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect::<Vec<_>>();
    Sprite::from_strip(slot, &rows).map_err(fail)
}

impl Catalog {
    /// Source Sprites obey the existing four-frame limit: equipment, props
    /// and tiles are at most 24x24; VFX are at most the 32x24 aura slot.
    /// Entry pivot/anchors are source-frame pixels; rig sockets are the
    /// complete export's padded-canvas pixels and are never mixed with them.
    /// Bounded, atomic validation. Unlike a general quick rig, factory bodies
    /// must export every Clip::ALL, even when several clips share poses.
    pub fn parse(json: &str) -> Result<Self, Error> {
        if json.len() > 32 * 1024 * 1024 {
            return Err(fail("catalog exceeds 32 MiB"));
        }
        let raw: Export = serde_json::from_str(json).map_err(|e| fail(e.to_string()))?;
        if raw.version != 1 || raw.assets.len() > 512 {
            return Err(fail("unsupported catalog version or more than 512 assets"));
        }
        let mut assets = BTreeMap::new();
        for entry in raw.assets {
            let id = entry.id.clone();
            let asset = entry.check().map_err(|e| fail(format!("{id}: {e}")))?;
            if assets.insert(id.clone(), asset).is_some() {
                return Err(fail(format!("duplicate asset id {id}")));
            }
        }
        // Two different rigs must never compete for the same saved body rows.
        let bodies = assets
            .values()
            .filter(|a| a.kind == Kind::Body)
            .collect::<Vec<_>>();
        for (i, a) in bodies.iter().enumerate() {
            if bodies[i + 1..]
                .iter()
                .any(|b| a.sprite.frames == b.sprite.frames)
            {
                return Err(fail("body source rows must uniquely identify a rig"));
            }
        }
        Ok(Self { assets })
    }
    pub fn assets(&self) -> impl ExactSizeIterator<Item = &Asset> {
        self.assets.values()
    }
    pub fn get(&self, id: &str) -> Option<&Asset> {
        self.assets.get(id)
    }

    /// Install all equipment cards atomically. Existing ids are never silently
    /// replaced; clients must opt in with Replace::Yes for an explicit update.
    pub fn install(&self, book: &mut Book, replace: Replace) -> Result<Vec<String>, Error> {
        let mut next = book.clone();
        let mut ids = Vec::new();
        for asset in self
            .assets()
            .filter(|a| matches!(a.kind, Kind::Body | Kind::Weapon | Kind::Worn))
        {
            let card = asset.card()?;
            if replace == Replace::No && next.get(&card.id).is_some_and(|c| c != &card) {
                return Err(fail(format!(
                    "{} already exists; replacement must be explicit",
                    card.id
                )));
            }
            ids.push(card.id.clone());
            if !next.insert(card) {
                return Err(fail("Book is full"));
            }
        }
        *book = next;
        Ok(ids)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Replace {
    No,
    Yes,
}

impl Entry {
    fn check(self) -> Result<Asset, Error> {
        if self.id.is_empty() || self.id.len() > 24 || self.id != cards::slug(&self.id) {
            return Err(fail("id must be a lowercase card slug of at most 24 bytes"));
        }
        if self.name.trim().is_empty()
            || self.name.len() > 120
            || self.brief.len() > 2048
            || self.name.contains(['\n', '\r'])
            || self.brief.contains(['\n', '\r'])
        {
            return Err(fail("name/brief must be bounded single lines"));
        }
        let part = looks::place(&self.part)
            .ok_or_else(|| fail("unknown worn part"))?
            .to_owned();
        let checked_sprite = sprite(
            &self.frames,
            if self.kind == Kind::Vfx {
                Slot::Aura
            } else {
                Slot::Worn
            },
        )?;
        let (w, h) = (
            checked_sprite.frames[0][0].len() as i32,
            checked_sprite.frames[0].len() as i32,
        );
        let inside = |p: [i32; 2]| p[0] >= 0 && p[1] >= 0 && p[0] < w && p[1] < h;
        if !inside(self.pivot)
            || self.anchors.len() > 32
            || self.anchors.iter().any(|(name, p)| {
                name.is_empty()
                    || name.len() > 40
                    || !name.bytes().all(|c| c.is_ascii_lowercase() || c == b'_')
                    || !inside(*p)
            })
        {
            return Err(fail(
                "pivot/anchors must be named source-frame pixel coordinates",
            ));
        }
        let rig = match (self.kind, self.rig) {
            (Kind::Body, Some(raw)) => {
                let rig = Rig::parse(&raw.to_string()).map_err(|e| fail(e.to_string()))?;
                if Clip::ALL.into_iter().any(|c| !rig.has(c)) {
                    return Err(fail("body must include every Clip::ALL"));
                }
                Some(Arc::new(rig))
            }
            (Kind::Body, None) => return Err(fail("body requires a complete rig")),
            (_, Some(_)) => return Err(fail("only a body may contain a rig")),
            _ => None,
        };
        let attack = match (self.kind, self.attack) {
            (Kind::Weapon, Some(raw)) => {
                if !matches!(
                    raw.clip,
                    Clip::Slash | Clip::Overhead | Clip::Thrust | Clip::Flail
                ) {
                    return Err(fail("weapon requires a melee attack clip"));
                }
                if !SOUNDS.contains(&raw.sound.as_str()) {
                    return Err(fail("sound must be an existing game sound pick"));
                }
                Some(Attack {
                    clip: raw.clip,
                    sound: raw.sound,
                    vfx: sprite(&raw.vfx, Slot::Aura)?,
                })
            }
            (Kind::Weapon, None) => return Err(fail("weapon requires attack metadata")),
            (_, Some(_)) => return Err(fail("only a weapon may contain attack metadata")),
            _ => None,
        };
        let mut asset = Asset {
            id: self.id,
            name: self.name,
            brief: self.brief,
            kind: self.kind,
            sprite: checked_sprite,
            pivot: self.pivot,
            anchors: self.anchors,
            part,
            rig,
            attack,
            card: None,
        };
        if matches!(asset.kind, Kind::Body | Kind::Weapon | Kind::Worn) {
            asset.card = Some(asset.build_card()?);
        }
        Ok(asset)
    }
}

/// Exact rows only; a vaguely similar legacy sheet must never acquire a rig.
pub(super) fn sprite_rig(sprite: &Sprite) -> Option<Arc<Rig>> {
    catalog()
        .ok()?
        .assets()
        .find(|a| a.kind == Kind::Body && a.sprite.frames == sprite.frames)
        .and_then(|a| a.rig.clone())
}

/// A checked placement in world units, painted by the normal arena layers.
/// Its pivot lands at (x,y); ticks are deterministic (the ordinary Sprite frame cadence).
#[derive(Clone, Debug)]
pub struct Placement<'a> {
    asset: &'a Asset,
    x: f32,
    y: f32,
    elapsed: u64,
    attack: bool,
    mirrored: bool,
}
impl<'a> Placement<'a> {
    /// Place a weapon attack VFX centred on a world point. The caller owns
    /// effect lifetime and may play `asset.attack().unwrap().sound()` once
    /// through its existing game sound event path. No combat state is changed.
    pub fn attack(
        asset: &'a Asset,
        x: f32,
        y: f32,
        elapsed: u64,
        mirrored: bool,
    ) -> Result<Self, Error> {
        if asset.attack.is_none() || !x.is_finite() || !y.is_finite() {
            return Err(fail(
                "attack placement needs a checked weapon and finite world coordinates",
            ));
        }
        Ok(Self {
            asset,
            x,
            y,
            elapsed,
            mirrored,
            attack: true,
        })
    }
    pub fn new(
        asset: &'a Asset,
        x: f32,
        y: f32,
        elapsed: u64,
        mirrored: bool,
    ) -> Result<Self, Error> {
        if !matches!(asset.kind, Kind::Prop | Kind::Tile | Kind::Vfx)
            || !x.is_finite()
            || !y.is_finite()
        {
            return Err(fail(
                "placement needs a prop, tile or VFX and finite world coordinates",
            ));
        }
        Ok(Self {
            asset,
            x,
            y,
            elapsed,
            mirrored,
            attack: false,
        })
    }
}
#[derive(Clone)]
struct Paint {
    image: Img,
    pivot: [i32; 2],
    x: f32,
    y: f32,
    over: bool,
}
thread_local! { static PLACEMENTS: RefCell<Vec<Vec<Paint>>> = const { RefCell::new(Vec::new()) }; }

/// Supply client-owned props/effects to any normal arena frame/layer capture.
/// Scoped and thread-local: nested calls restore their caller, including panic.
/// Persist these placements in client presentation state, not in Hero or saves.
pub fn with_placements<T>(placements: &[Placement<'_>], draw: impl FnOnce() -> T) -> T {
    let paints = placements
        .iter()
        .map(|p| {
            let sprite = if p.attack {
                &p.asset.attack.as_ref().expect("checked").vfx
            } else {
                &p.asset.sprite
            };
            let image = super::knights::frame_img(sprite.frame(p.elapsed));
            let pivot = if p.attack {
                [image.w / 2, image.h / 2]
            } else {
                p.asset.pivot
            };
            let pivot = if p.mirrored {
                [image.w - 1 - pivot[0], pivot[1]]
            } else {
                pivot
            };
            Paint {
                image: if p.mirrored { image.flip_h() } else { image },
                pivot,
                x: p.x,
                y: p.y,
                over: p.attack || p.asset.kind == Kind::Vfx,
            }
        })
        .collect();
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            PLACEMENTS.with(|p| {
                p.borrow_mut().pop();
            });
        }
    }
    PLACEMENTS.with(|p| p.borrow_mut().push(paints));
    let _restore = Restore;
    draw()
}

pub(super) fn paint(cv: &mut Img, over: bool) {
    PLACEMENTS.with(|p| {
        if let Some(paints) = p.borrow().last() {
            for p in paints.iter().filter(|p| p.over == over) {
                let (x, y) = super::at(p.x, p.y);
                let (x, y) = (x.saturating_sub(p.pivot[0]), y.saturating_sub(p.pivot[1]));
                if x >= cv.w
                    || y >= cv.h
                    || i64::from(x) + i64::from(p.image.w) <= 0
                    || i64::from(y) + i64::from(p.image.h) <= 0
                {
                    continue;
                }
                super::note_body(&p.image, x, y);
                cv.stamp(&p.image, x, y);
            }
        }
    });
}

/// Match the actual checked card as well as its id: an edited/replaced legacy
/// card must not accidentally inherit factory attack metadata.
pub(super) fn equipped_attack_clip(hero: &crate::rules::Hero, book: &Book) -> Option<Clip> {
    let card = book.get(hero.arm.as_deref()?)?;
    let id = card.id.strip_prefix("factory-")?;
    let asset = catalog().ok()?.get(id)?;
    if asset.card.as_ref() != Some(card) {
        return None;
    }
    asset.attack.as_ref().map(|a| a.clip)
}

fn unique_anchors<'de, D>(deserializer: D) -> Result<BTreeMap<String, [i32; 2]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Unique;
    impl<'de> serde::de::Visitor<'de> for Unique {
        type Value = BTreeMap<String, [i32; 2]>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("unique anchor names")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut anchors = BTreeMap::new();
            while let Some((name, point)) = map.next_entry::<String, [i32; 2]>()? {
                if anchors.insert(name.clone(), point).is_some() {
                    return Err(serde::de::Error::custom(format!("duplicate anchor {name}")));
                }
            }
            Ok(anchors)
        }
    }
    deserializer.deserialize_map(Unique)
}

/// Shipped native props, also available to room authors without a disk path.
pub fn prop_image(id: &str, tick: u64) -> Option<Img> {
    let asset = catalog().ok()?.get(id)?;
    if !matches!(asset.kind, Kind::Prop | Kind::Tile) {
        return None;
    }
    Some(super::knights::frame_img(asset.sprite.frame(tick)))
}

thread_local! {
    static ENEMY_SKINS: RefCell<Vec<BTreeMap<u32, String>>> = const { RefCell::new(Vec::new()) };
}

/// Bind authored bodies to existing enemies in client presentation state.
/// The explicit ids leave old saves, default bestiary art and rules untouched.
pub fn with_enemy_skins<T>(skins: &[(u32, &str)], draw: impl FnOnce() -> T) -> Result<T, Error> {
    let catalog = catalog().map_err(|e| fail(e.to_string()))?;
    let mut selected = BTreeMap::new();
    for &(enemy, id) in skins {
        let asset = catalog
            .get(id)
            .ok_or_else(|| fail("unknown monster skin"))?;
        if asset.kind != Kind::Body || selected.insert(enemy, id.to_owned()).is_some() {
            return Err(fail(
                "monster skins require unique enemy ids and body assets",
            ));
        }
    }
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            ENEMY_SKINS.with(|s| {
                s.borrow_mut().pop();
            });
        }
    }
    ENEMY_SKINS.with(|s| s.borrow_mut().push(selected));
    let _restore = Restore;
    Ok(draw())
}

pub(super) fn enemy_image(enemy: &crate::rules::Enemy, tick: u32) -> Option<Img> {
    let id = ENEMY_SKINS.with(|skins| skins.borrow().last()?.get(&enemy.id).cloned())?;
    let rig = catalog().ok()?.get(&id)?.rig.as_ref()?;
    let clip = if enemy.hurt > 0 {
        Clip::Hurt
    } else if enemy.dir.0.abs() + enemy.dir.1.abs() > 0.1 {
        Clip::Walk
    } else {
        Clip::Idle
    };
    let sample = rig.sample(
        clip,
        u64::from(tick),
        super::quick_rig::Playback::Loop,
        enemy.dir.0 < 0.0,
    );
    let image = sample.image();
    let pivot = sample.pivot();
    // A fixed source-size crop about the shared floor, never per-frame trim.
    let mut native = Img::new(24, 24);
    native.stamp(&image, 12 - pivot[0], 23 - pivot[1]);
    Some(native)
}

/// Native attack stamps follow the current rig's weapon tip, not an unrelated
/// preview offset. Timing is presentation-only; clients may play attack.sound().
pub(super) fn attacks(cv: &mut Img, run: &Run) {
    let Ok(catalog) = catalog() else {
        return;
    };
    for (&id, hero) in &run.players {
        if hero.fire_cooldown == 0 {
            continue;
        }
        let Some(card) = hero.arm.as_deref().and_then(|id| run.book.get(id)) else {
            continue;
        };
        let Some(asset) = card
            .id
            .strip_prefix("factory-")
            .and_then(|id| catalog.get(id))
        else {
            continue;
        };
        if asset.card.as_ref() != Some(card) {
            continue;
        }
        let Some(attack) = asset.attack.as_ref() else {
            continue;
        };
        let owned = hero
            .worn(&run.book)
            .into_iter()
            .rev()
            .find_map(super::quick_rig::sprite_rig);
        let rig = match owned.as_deref() {
            Some(rig) => rig,
            None => match super::quick_rig::seat_knight(id) {
                Some(rig) => rig,
                None => continue,
            },
        };
        let sample = super::knights::rig_sample_event(rig, hero, &run.book, run.tick, None);
        if sample.clip != attack.clip {
            continue;
        }
        let frame = sample.index.min(3) as u64 * 6;
        let image = super::knights::frame_img(attack.vfx.frame(frame));
        let tip = sample.anchors().tip;
        let pivot = sample.pivot();
        let (x, y) = super::at(hero.x, hero.y);
        cv.stamp(
            &image,
            x + tip[0] - pivot[0] - image.w / 2,
            y + tip[1] - pivot[1] - image.h / 2,
        );
    }
}

/// Factory weapons are authored upright with their grip at the bottom. Use
/// the canonical right-part contact while retaining this frame's rig socket.
pub(super) fn weapon_part(
    sprite: &Sprite,
    mut part: crate::rules::looks::Part,
) -> crate::rules::looks::Part {
    let checked = catalog().ok().is_some_and(|catalog| {
        catalog.assets().any(|asset| {
            asset.kind == Kind::Weapon
                && asset.card.as_ref().and_then(|c| c.looks.worn.as_ref()) == Some(sprite)
        })
    });
    if checked {
        let canonical = crate::rules::looks::profile("knight").right;
        part.size = canonical.size;
        part.bottom = canonical.bottom;
    }
    part
}

/// Cropping and nearest fitting can move an asymmetric handle away from the
/// rectangle centre. Resolve the painted grip after that fit, before rotation.
pub(super) fn weapon_origin(
    original: &Sprite,
    fitted: &Sprite,
    part: crate::rules::looks::Part,
) -> Option<(i32, i32)> {
    let catalog = catalog().ok()?;
    let asset = catalog.assets().find(|asset| {
        asset.kind == Kind::Weapon
            && asset.card.as_ref().and_then(|c| c.looks.worn.as_ref()) == Some(original)
    })?;
    let [gx, gy] = *asset.anchors.get("grip")?;
    let [x, y, w, h] = looks::ink_bounds(&asset.sprite.frames)?;
    let frame = fitted.frames.first()?;
    let (tw, th) = (frame.first()?.len(), frame.len());
    if tw == 0 || th == 0 {
        return None;
    }
    let target = (
        ((gx - x) * tw as i32 / w).clamp(0, tw as i32 - 1),
        ((gy - y) * th as i32 / h).clamp(0, th as i32 - 1),
    );
    let (px, py) = frame
        .iter()
        .enumerate()
        .flat_map(|(y, row)| {
            row.bytes()
                .enumerate()
                .filter(|(_, c)| *c != b'.')
                .map(move |(x, _)| (x as i32, y as i32))
        })
        .min_by_key(|&(x, y)| (x - target.0).pow(2) + (y - target.1).pow(2))?;
    Some((part.anchor.0 - px, part.anchor.1 - py))
}
