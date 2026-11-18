//! The preview feed: a made creature arrives as baked frames in the boss library's own format
//! (rest, windup, strike; feet at the bottom centre; the library's display recipe beside them) and
//! is drawn through the boss library's own dressing: its material grade, its rim light, and its
//! idle rule (the breathing frame). Real creature atlases enter here later (SPEC-CREATOR 3,
//! `art` and `motions`): `Baked::from_library` takes the import record and the frames, nothing else.
//!
//! A set may bring more than the library asks for, all optional and all read from the files
//! `scripts/creator/export-baked-set.py` writes: walk strips, an idle strip, the whole smash as an
//! attack strip, and `animation.json` (anchors, the attack's keys). A set from the pipeline's frame
//! sets brings more: the ways atlas, every clip (idle, walk, run, attack, hit, death, cast) in all
//! eight directions, which the stage turns through and the Test drive faces where it walks. A set
//! with only the two old ways (east, south) behaves as it always did. The registry table at the foot
//! of the file says which set shows which creation; a role without a row gets the Bailiff.

use super::super::overworld::arena;
use super::super::overworld::ink::{Img, bayer};
use crate::rules::creation::drive::{RECOVER, SETTLE, STRIKE, WINDUP};
use crate::rules::creation::{Blow, Creation, Pigment, Role, Shot, Timing};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub use crate::rules::creation::drive::Way;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Idle,
    Walk(Way),
    Run(Way),
    Windup,
    Strike,
    Hit,
    Death,
    Cast,
}

#[derive(serde::Deserialize)]
struct Import {
    boss: String,
    display_policy: Policy,
}

#[derive(serde::Deserialize)]
struct Policy {
    v1: [i32; 2],
    grade: BTreeMap<String, [u32; 2]>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct Animation {
    fps: u32,
    states: BTreeMap<String, State>,
    attack_events: Option<Keys>,
    creator: Creator,
    ways: Option<WaysRecord>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct WaysRecord {
    directions: Vec<String>,
    cell: [i32; 2],
    fps: u32,
    clips: BTreeMap<String, WayClipRecord>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct WayClipRecord {
    row: i32,
    frames: usize,
    anchor_y: BTreeMap<String, i32>,
    #[serde(default)]
    events: BTreeMap<String, usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Clip {
    Idle,
    Walk,
    Run,
    Attack,
    Hit,
    Death,
    Cast,
}

impl Clip {
    const ALL: [(&'static str, Clip); 7] = [
        ("idle", Clip::Idle),
        ("walk", Clip::Walk),
        ("run", Clip::Run),
        ("attack", Clip::Attack),
        ("hit", Clip::Hit),
        ("death", Clip::Death),
        ("cast", Clip::Cast),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reel {
    Blow,
    Cast,
    Hit,
    Fall,
    Rise,
}

#[derive(Clone, Debug)]
pub struct Played<'a> {
    pub frame: Cow<'a, Img>,
    pub drop: i32,
}

const MIN_STRIKE: u64 = 3;

#[derive(Clone, Debug)]
struct WayClip {
    frames: [Vec<Img>; 8],
    anchor: [i32; 8],
    events: BTreeMap<String, usize>,
}

#[derive(Clone, Debug)]
struct Ways {
    fps: u32,
    clips: BTreeMap<Clip, WayClip>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct State {
    anchor: [i32; 2],
}

#[derive(Clone, Debug, serde::Deserialize)]
struct Creator {
    name: String,
    dye: Option<Dye>,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
struct Keys {
    anticipation_starts: u32,
    windup_key: u32,
    swing: u32,
    strike: u32,
    recovery_starts: u32,
    total_ticks: u32,
}

const WALK_STRIDE: f32 = 0.62;
const WALK_FRAMES: f32 = 12.0;
const RUN_REACH: f32 = 1.75;
pub const TURN_TICKS: u64 = 40;

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub struct Dye {
    hues: [f32; 2],
    min_sat: f32,
    min_val: f32,
}

pub const GREENS: Dye = Dye { hues: [62.0, 170.0], min_sat: 0.2, min_val: 0.08 };

#[derive(Clone, Debug)]
pub struct Baked {
    pub id: String,
    pub size: (i32, i32),
    exposure: u32,
    rest: Img,
    breath: Img,
    windup: Img,
    strike: Img,
    walk: [Vec<Img>; 2],
    idle: Vec<Img>,
    swing: Vec<Img>,
    animation: Option<Animation>,
    ways: Option<Ways>,
    dye: Option<Dye>,
    name: String,
}

pub struct Set {
    pub import: &'static str,
    pub animation: &'static str,
    pub rest: &'static [u8],
    pub windup: &'static [u8],
    pub strike: &'static [u8],
    pub walk_east: &'static [u8],
    pub walk_south: &'static [u8],
    pub idle: &'static [u8],
    pub attack: &'static [u8],
    pub ways: &'static [u8],
}

macro_rules! embedded_set {
    ($id:literal) => {
        embedded_set!($id, &[])
    };
    ($id:literal, ways) => {
        embedded_set!($id, include_bytes!(concat!("../../../assets/creator/baked/", $id, "/library-v1-ways.png")))
    };
    ($id:literal, $ways:expr) => {{
        static SET: OnceLock<Baked> = OnceLock::new();
        SET.get_or_init(|| {
            Baked::from_set(Set {
                import: include_str!(concat!("../../../assets/creator/baked/", $id, "/library-import.json")),
                animation: include_str!(concat!("../../../assets/creator/baked/", $id, "/animation.json")),
                rest: include_bytes!(concat!("../../../assets/creator/baked/", $id, "/library-v1-rest.png")),
                windup: include_bytes!(concat!("../../../assets/creator/baked/", $id, "/library-v1-windup.png")),
                strike: include_bytes!(concat!("../../../assets/creator/baked/", $id, "/library-v1-strike.png")),
                walk_east: include_bytes!(concat!("../../../assets/creator/baked/", $id, "/library-v1-walk-east-strip.png")),
                walk_south: include_bytes!(concat!("../../../assets/creator/baked/", $id, "/library-v1-walk-south-strip.png")),
                idle: include_bytes!(concat!("../../../assets/creator/baked/", $id, "/library-v1-idle-strip.png")),
                attack: include_bytes!(concat!("../../../assets/creator/baked/", $id, "/library-v1-attack-strip.png")),
                ways: $ways,
            })
            .unwrap_or_else(|e| panic!("the embedded set `{}`: {e}", $id))
        })
    }};
}

fn decode(png: &[u8]) -> Result<Img, String> {
    let rgba = image::load_from_memory(png).map_err(|e| e.to_string())?.to_rgba8();
    let mut im = Img::new(rgba.width() as i32, rgba.height() as i32);
    for (x, y, c) in rgba.enumerate_pixels() {
        if c[3] > 127 {
            im.set(x as i32, y as i32, [c[0], c[1], c[2]]);
        }
    }
    Ok(im)
}

impl Baked {
    pub fn from_library(import: &str, rest: &[u8], windup: &[u8], strike: &[u8]) -> Result<Self, String> {
        let record: Import = serde_json::from_str(import).map_err(|e| e.to_string())?;
        let [w, h] = record.display_policy.v1;
        let exposure = record.display_policy.grade.get("v1").map_or(256, |g| g[0]);
        let mut frames = Vec::new();
        for png in [rest, windup, strike] {
            let im = decode(png)?;
            if (im.w, im.h) != (w, h) {
                return Err(format!("a frame is {}x{}, the import says {w}x{h}", im.w, im.h));
            }
            frames.push(arena::library_dress(im, exposure));
        }
        let [rest, windup, strike]: [Img; 3] = frames.try_into().map_err(|_| "three frames".to_string())?;
        Ok(Self {
            id: record.boss,
            size: (w, h),
            exposure,
            breath: arena::library_breathe(&rest),
            rest,
            windup,
            strike,
            walk: [Vec::new(), Vec::new()],
            idle: Vec::new(),
            swing: Vec::new(),
            animation: None,
            ways: None,
            dye: Some(GREENS),
            name: "BAILIFF".into(),
        })
    }

    pub fn from_set(set: Set) -> Result<Self, String> {
        let record: Animation = serde_json::from_str(set.animation).map_err(|e| format!("animation: {e}"))?;
        let has = |state: &str| record.states.contains_key(state);
        let mut baked = Self::from_library(set.import, set.rest, set.windup, set.strike)?;
        if has("walk-east") {
            baked.walk[slot(Way::East)] = baked.cells(set.walk_east).map_err(|e| format!("walk: {e}"))?;
        }
        if has("walk-south") {
            baked.walk[slot(Way::South)] = baked.cells(set.walk_south).map_err(|e| format!("walk: {e}"))?;
        }
        if has("idle") {
            baked = baked.with_idle(set.idle)?;
        }
        if has("attack") {
            baked = baked.with_swing(set.attack)?;
        }
        let mut baked = baked.with_animation(set.animation)?;
        match (&record.ways, set.ways.is_empty()) {
            (Some(ways), false) => baked.ways = Some(baked.load_ways(ways, set.ways)?),
            (None, true) => {}
            (Some(_), true) => return Err("animation: the record has eight ways but the set brought no atlas".into()),
            (None, false) => return Err("animation: an atlas of eight ways with no `ways` block to read it by".into()),
        }
        Ok(baked)
    }

    fn load_ways(&self, record: &WaysRecord, png: &[u8]) -> Result<Ways, String> {
        let bad = |m: String| format!("ways: {m}");
        let (w, h) = self.size;
        if record.cell != [w, h] {
            return Err(bad(format!("cells are {:?}, the set's are {w}x{h}", record.cell)));
        }
        let mut slot_of = [usize::MAX; 8];
        for (row, name) in record.directions.iter().enumerate() {
            let way = Way::from_short(name).ok_or_else(|| bad(format!("`{name}` is not a direction")))?;
            if row >= 8 || slot_of[way.index()] != usize::MAX {
                return Err(bad(format!("the directions are eight, each once: {:?}", record.directions)));
            }
            slot_of[way.index()] = row;
        }
        if slot_of.contains(&usize::MAX) {
            return Err(bad(format!("the directions are eight, each once: {:?}", record.directions)));
        }
        let atlas = decode(png).map_err(bad)?;
        if atlas.w % w != 0 || atlas.h % h != 0 || atlas.w == 0 {
            return Err(bad(format!("the atlas is {}x{}, cells are {w}x{h}", atlas.w, atlas.h)));
        }
        let (cols, rows) = (atlas.w / w, atlas.h / h);
        let mut clips = BTreeMap::new();
        for (name, c) in &record.clips {
            let clip = Clip::ALL.iter().find(|(n, _)| n == name).map(|(_, c)| *c).ok_or_else(|| bad(format!("`{name}` is not a clip")))?;
            if c.frames == 0 || c.frames as i32 > cols || c.row < 0 || c.row + 8 > rows {
                return Err(bad(format!("`{name}`: {} frames at row {} do not sit in a {cols}x{rows} atlas", c.frames, c.row)));
            }
            if c.events.values().any(|&f| f >= c.frames) {
                return Err(bad(format!("`{name}`: an event past its {} frames", c.frames)));
            }
            let mut frames: [Vec<Img>; 8] = Default::default();
            let mut anchor = [0; 8];
            for way in Way::ALL {
                let row = c.row + slot_of[way.index()] as i32;
                anchor[way.index()] = *c.anchor_y.get(way.short()).ok_or_else(|| bad(format!("`{name}` has no sole edge for {}", way.short())))?;
                for i in 0..c.frames as i32 {
                    let mut cell = Img::new(w, h);
                    for y in 0..h {
                        for x in 0..w {
                            if let Some(rgb) = atlas.get(i * w + x, row * h + y) {
                                cell.set(x, y, rgb);
                            }
                        }
                    }
                    if !(0..h).any(|y| (0..w).any(|x| cell.get(x, y).is_some())) {
                        return Err(bad(format!("`{name}` {} frame {i} is blank", way.short())));
                    }
                    frames[way.index()].push(arena::library_dress(cell, self.exposure));
                }
            }
            clips.insert(clip, WayClip { frames, anchor, events: c.events.clone() });
        }
        if !clips.contains_key(&Clip::Idle) {
            return Err(bad("an eight-way set has an idle clip".into()));
        }
        if let Some(attack) = clips.get(&Clip::Attack)
            && attack.frames[0].len() != self.swing.len()
        {
            return Err(bad(format!("its attack is {} frames, the smash strip is {}", attack.frames[0].len(), self.swing.len())));
        }
        Ok(Ways { fps: record.fps.max(1), clips })
    }

    fn cells(&self, png: &[u8]) -> Result<Vec<Img>, String> {
        let strip = decode(png)?;
        let (w, h) = self.size;
        if strip.h != h || strip.w % w != 0 || strip.w == 0 {
            return Err(format!("a strip is {}x{}, cells are {w}x{h}", strip.w, strip.h));
        }
        Ok((0..strip.w / w)
            .map(|i| {
                let mut cell = Img::new(w, h);
                for y in 0..h {
                    for x in 0..w {
                        if let Some(c) = strip.get(i * w + x, y) {
                            cell.set(x, y, c);
                        }
                    }
                }
                arena::library_dress(cell, self.exposure)
            })
            .collect())
    }

    pub fn with_walks(mut self, east: &[u8], south: &[u8]) -> Result<Self, String> {
        let cycles = [self.cells(east).map_err(|e| format!("walk: {e}"))?, self.cells(south).map_err(|e| format!("walk: {e}"))?];
        self.walk = cycles;
        Ok(self)
    }

    pub fn with_idle(mut self, strip: &[u8]) -> Result<Self, String> {
        self.idle = self.cells(strip).map_err(|e| format!("idle: {e}"))?;
        Ok(self)
    }

    pub fn with_swing(mut self, strip: &[u8]) -> Result<Self, String> {
        self.swing = self.cells(strip).map_err(|e| format!("attack: {e}"))?;
        Ok(self)
    }

    pub fn with_animation(mut self, json: &str) -> Result<Self, String> {
        let a: Animation = serde_json::from_str(json).map_err(|e| format!("animation: {e}"))?;
        for (state, loaded) in [
            ("idle", !self.idle.is_empty()),
            ("walk-east", !self.walk[slot(Way::East)].is_empty()),
            ("walk-south", !self.walk[slot(Way::South)].is_empty()),
            ("attack", !self.swing.is_empty()),
        ] {
            if a.states.contains_key(state) != loaded {
                return Err(format!("animation: `{state}` is {} in the record but {} loaded", if loaded { "missing" } else { "listed" }, if loaded { "is" } else { "not" }));
            }
        }
        match (a.attack_events, self.swing.len() as u32) {
            (None, 0) => {}
            (Some(k), frames) if frames > 0 => {
                let keys = [k.anticipation_starts, k.windup_key, k.swing, k.strike, k.recovery_starts];
                if k.total_ticks != frames || keys.iter().any(|&f| f >= frames) || !keys.windows(2).all(|p| p[0] <= p[1]) {
                    return Err(format!("animation: the smash's keys {keys:?} do not fit its {frames} frames"));
                }
            }
            _ => return Err("animation: the smash and its keys come together".into()),
        }
        if a.creator.name.trim().is_empty() || a.creator.name.chars().count() > 12 {
            return Err("animation: a set's name is 1 to 12 letters (it is engraved on the stage)".into());
        }
        self.dye = a.creator.dye;
        self.name = a.creator.name.to_uppercase();
        self.animation = Some(a);
        Ok(self)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn carries_motion(&self) -> bool {
        self.animation.is_some()
    }

    pub fn stand_in(&self) -> bool {
        self.ways.is_none()
    }

    fn clip_ticks(&self, frames: usize) -> u64 {
        let fps = self.ways.as_ref().map_or(30, |w| u64::from(w.fps));
        (frames as u64 * 30).div_ceil(fps)
    }

    pub fn timing(&self) -> Timing {
        let Some(ways) = &self.ways else { return Timing::FIXED };
        let one = |c: Clip, event: Option<&str>| {
            ways.clips.get(&c).map(|clip| Shot {
                ticks: self.clip_ticks(clip.frames[0].len()),
                event: event.and_then(|e| clip.events.get(e)).map_or(0, |&f| self.clip_ticks(f)),
                settle: SETTLE,
            })
        };
        let blow = ways.clips.get(&Clip::Attack).map_or(Blow::FIXED, |clip| {
            let n = clip.frames[0].len();
            let impact = clip.events.get("hit").copied().unwrap_or(n / 3).min(n - 1);
            let windup = self.clip_ticks(impact);
            let strike = (self.clip_ticks(impact + 1) - windup).max(MIN_STRIKE);
            Blow { windup, strike, recover: self.clip_ticks(n).saturating_sub(windup + strike), settle: SETTLE }
        });
        Timing { blow, cast: one(Clip::Cast, Some("cast")), hit: one(Clip::Hit, None), death: one(Clip::Death, None) }
    }

    pub fn shot(&self, reel: Reel, since: u64, facing: Way, tick: u64) -> Option<Played<'_>> {
        let ways = self.ways.as_ref()?;
        let id = match reel {
            Reel::Blow => Clip::Attack,
            Reel::Cast => Clip::Cast,
            Reel::Hit => Clip::Hit,
            Reel::Fall | Reel::Rise => Clip::Death,
        };
        let clip = ways.clips.get(&id)?;
        let frames = &clip.frames[facing.index()];
        let n = frames.len();
        let clip_ticks = if reel == Reel::Blow { self.timing().blow.clip() } else { self.clip_ticks(n) };
        let own = (self.size.1 - 1) - clip.anchor[facing.index()];
        let rising = reel == Reel::Rise;
        if since < clip_ticks {
            let k = ((since * u64::from(ways.fps) / 30) as usize).min(n - 1);
            return Some(Played { frame: Cow::Borrowed(&frames[if rising { n - 1 - k } else { k }]), drop: own });
        }
        if reel == Reel::Fall {
            return Some(Played { frame: Cow::Borrowed(&frames[n - 1]), drop: own });
        }
        let j = since - clip_ticks;
        if j >= SETTLE {
            return None;
        }
        let share = (j + 1) as f32 / (SETTLE + 1) as f32;
        let from = if rising { &frames[0] } else { &frames[n - 1] };
        let to = self.frame_facing(Motion::Idle, facing, tick, 0.0);
        let stance = self.drop_facing(Motion::Idle, facing);
        let drop = own + ((stance - own) as f32 * share).round() as i32;
        Some(Played { frame: Cow::Owned(dissolve(from, to, share)), drop })
    }

    pub fn bailiff() -> &'static Baked {
        static BAILIFF: OnceLock<Baked> = OnceLock::new();
        BAILIFF.get_or_init(|| {
            Baked::from_library(
                include_str!("../../../assets/creator/baked/marsh-bailiff/library-import.json"),
                include_bytes!("../../../assets/creator/baked/marsh-bailiff/library-v1-rest.png"),
                include_bytes!("../../../assets/creator/baked/marsh-bailiff/library-v1-windup.png"),
                include_bytes!("../../../assets/creator/baked/marsh-bailiff/library-v1-strike.png"),
            )
            .and_then(|b| {
                b.with_walks(
                    include_bytes!("../../../assets/creator/baked/marsh-bailiff/library-v1-walk-east-strip.png"),
                    include_bytes!("../../../assets/creator/baked/marsh-bailiff/library-v1-walk-south-strip.png"),
                )
            })
            .expect("the embedded Bailiff")
        })
    }

    fn stride(&self, way: Way) -> f32 {
        WALK_STRIDE * (WALK_FRAMES / self.walk[slot(way)].len().max(1) as f32)
    }

    pub fn eight_way(&self) -> bool {
        self.ways.is_some()
    }

    pub fn turntable(&self, tick: u64) -> Way {
        if self.ways.is_some() { Way::nth((tick / TURN_TICKS) as usize) } else { Way::South }
    }

    pub fn way_clips(&self) -> Vec<&'static str> {
        self.ways.as_ref().map_or_else(Vec::new, |w| Clip::ALL.iter().filter(|(_, c)| w.clips.contains_key(c)).map(|(n, _)| *n).collect())
    }

    pub fn way_frames(&self, clip: &str, way: Way) -> Option<&[Img]> {
        let (_, clip) = Clip::ALL.iter().find(|(n, _)| *n == clip)?;
        Some(&self.ways.as_ref()?.clips.get(clip)?.frames[way.index()])
    }

    pub fn way_event(&self, clip: &str, event: &str) -> Option<usize> {
        let (_, clip) = Clip::ALL.iter().find(|(n, _)| *n == clip)?;
        self.ways.as_ref()?.clips.get(clip)?.events.get(event).copied()
    }

    pub fn frame(&self, motion: Motion, tick: u64, walked: f32) -> &Img {
        self.frame_facing(motion, Way::South, tick, walked)
    }

    pub fn frame_facing(&self, motion: Motion, facing: Way, tick: u64, walked: f32) -> &Img {
        if let Some(ways) = &self.ways
            && let Some(frame) = self.way_frame(ways, motion, facing, tick, walked)
        {
            return frame;
        }
        match motion {
            Motion::Idle | Motion::Hit | Motion::Death => match &self.animation {
                Some(a) if !self.idle.is_empty() => &self.idle[(tick * u64::from(a.fps) / 30) as usize % self.idle.len()],
                _ if arena::library_breathing(tick, 0) => &self.breath,
                _ => &self.rest,
            },
            Motion::Walk(way) | Motion::Run(way) => {
                let cycle = &self.walk[slot(way)];
                if cycle.is_empty() {
                    if (walked / 1.1) as u64 % 2 == 1 { &self.breath } else { &self.rest }
                } else {
                    &cycle[(walked / self.stride(way)) as usize % cycle.len()]
                }
            }
            Motion::Windup | Motion::Cast => &self.windup,
            Motion::Strike => &self.strike,
        }
    }

    fn way_frame<'a>(&'a self, ways: &'a Ways, motion: Motion, facing: Way, tick: u64, walked: f32) -> Option<&'a Img> {
        let facing = match motion {
            Motion::Walk(way) | Motion::Run(way) => way,
            _ => facing,
        };
        let clip = |c: Clip| ways.clips.get(&c).map(|c| c.frames[facing.index()].as_slice());
        let clock = (tick * u64::from(ways.fps) / 30) as usize;
        fn looped(frames: &[Img], clock: usize) -> &Img {
            &frames[clock % frames.len()]
        }
        fn once(frames: &[Img], clock: usize) -> &Img {
            &frames[clock.min(frames.len() - 1)]
        }
        let travel = |c: Clip, reach: f32| {
            let frames = ways.clips.get(&c).map(|c| c.frames[facing.index()].as_slice())?;
            let stride = WALK_STRIDE * reach * (WALK_FRAMES / frames.len() as f32);
            Some(&frames[(walked / stride) as usize % frames.len()])
        };
        let keys = || self.animation.as_ref().and_then(|a| a.attack_events);
        match motion {
            Motion::Idle => clip(Clip::Idle).map(|f| looped(f, clock)),
            Motion::Walk(_) => travel(Clip::Walk, 1.0).or_else(|| clip(Clip::Idle).map(|f| looped(f, clock))),
            Motion::Run(_) => travel(Clip::Run, RUN_REACH).or_else(|| travel(Clip::Walk, 1.0)).or_else(|| clip(Clip::Idle).map(|f| looped(f, clock))),
            Motion::Windup => clip(Clip::Attack).zip(keys()).map(|(f, k)| &f[windup_of(&k)]),
            Motion::Strike => clip(Clip::Attack).zip(keys()).map(|(f, k)| &f[k.strike as usize]),
            Motion::Hit => clip(Clip::Hit).map(|f| once(f, clock)).or_else(|| clip(Clip::Idle).map(|f| looped(f, clock))),
            Motion::Death => clip(Clip::Death).map(|f| once(f, clock)).or_else(|| clip(Clip::Idle).map(|f| looped(f, clock))),
            Motion::Cast => clip(Clip::Cast).map(|f| once(f, clock)).or_else(|| clip(Clip::Attack).zip(keys()).map(|(f, k)| &f[windup_of(&k)])),
        }
    }

    pub fn swing(&self, since: u64) -> Option<&Img> {
        self.swing_facing(since, Way::South)
    }

    pub fn swing_facing(&self, since: u64, facing: Way) -> Option<&Img> {
        let at = self.swing_index(since)?;
        match self.ways.as_ref().and_then(|w| w.clips.get(&Clip::Attack)) {
            Some(attack) => attack.frames[facing.index()].get(at),
            None => self.swing.get(at),
        }
    }

    pub(super) fn swing_index(&self, since: u64) -> Option<usize> {
        let keys = self.animation.as_ref()?.attack_events?;
        let last = self.swing.len().checked_sub(1)?;
        let (w, s, r) = (WINDUP, STRIKE, RECOVER);
        let lerp = |from: u32, to: u32, at: u64, over: u64| -> usize {
            let (from, to) = (i64::from(from), i64::from(to));
            (from + ((to - from) * at as i64 + over as i64 / 2) / over.max(1) as i64) as usize
        };
        let frame = if since + 2 < w {
            lerp(keys.anticipation_starts, keys.windup_key, since, w - 3)
        } else if since + 2 == w {
            (keys.swing as usize).saturating_sub(1).max(keys.windup_key as usize)
        } else if since + 1 == w {
            keys.swing as usize
        } else if since < w + s {
            lerp(keys.strike, keys.recovery_starts.saturating_sub(1).max(keys.strike), since - w, s - 1)
        } else if since < w + s + r {
            lerp(keys.recovery_starts, last as u32, since - w - s, r - 1)
        } else {
            return None;
        };
        Some(frame.min(last))
    }

    pub fn drop(&self, motion: Motion) -> i32 {
        self.drop_facing(motion, Way::South)
    }

    pub fn drop_facing(&self, motion: Motion, facing: Way) -> i32 {
        let facing = match motion {
            Motion::Walk(way) | Motion::Run(way) => way,
            _ => facing,
        };
        let edge = |c: Clip| self.ways.as_ref().and_then(|w| w.clips.get(&c)).map(|c| c.anchor[facing.index()]);
        if self.ways.is_some() {
            let own = match motion {
                Motion::Idle => edge(Clip::Idle),
                Motion::Walk(_) => edge(Clip::Walk),
                Motion::Run(_) => edge(Clip::Run).or_else(|| edge(Clip::Walk)),
                Motion::Windup | Motion::Strike => edge(Clip::Attack),
                Motion::Hit => edge(Clip::Hit),
                Motion::Death => edge(Clip::Death),
                Motion::Cast => edge(Clip::Cast).or_else(|| edge(Clip::Attack)),
            };
            if let Some(edge) = own.or_else(|| edge(Clip::Idle)) {
                return (self.size.1 - 1) - edge;
            }
        }
        let key = match motion {
            Motion::Idle | Motion::Hit | Motion::Death => "idle",
            Motion::Walk(way) | Motion::Run(way) => if slot(way) == slot(Way::East) { "walk-east" } else { "walk-south" },
            Motion::Windup | Motion::Strike | Motion::Cast => "attack",
        };
        self.animation.as_ref().and_then(|a| a.states.get(key)).map_or(0, |s| (self.size.1 - 1) - s.anchor[1])
    }
}

pub(super) fn dissolve(from: &Img, to: &Img, share: f32) -> Img {
    let (w, h) = (from.w.min(to.w), from.h.min(to.h));
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let pick = if bayer(x, y) < share { to.get(x, y) } else { from.get(x, y) };
            if let Some(rgb) = pick {
                out.set(x, y, rgb);
            }
        }
    }
    out
}

fn windup_of(k: &Keys) -> usize {
    (k.swing as usize).saturating_sub(1).max(k.windup_key as usize)
}

fn slot(way: Way) -> usize {
    usize::from(way.flat().0 == Way::South)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn tint(c: [u8; 3], dye: Pigment) -> [u8; 3] {
    tint_with(c, dye, GREENS)
}

pub fn tint_with(c: [u8; 3], pigment: Pigment, window: Dye) -> [u8; 3] {
    let (target_hue, sat, val) = match pigment {
        Pigment::Ochre | Pigment::Moss => return c,
        Pigment::Slate => (214.0, 0.55, 1.0),
        Pigment::Russet => (14.0, 1.0, 1.0),
        Pigment::Bone => (46.0, 0.30, 1.12),
        Pigment::Charcoal => (262.0, 0.22, 0.66),
    };
    let (r, g, b) = (f32::from(c[0]) / 255.0, f32::from(c[1]) / 255.0, f32::from(c[2]) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    if max < window.min_val || d / max.max(0.001) < window.min_sat {
        return c;
    }
    let hue = if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    }
    .rem_euclid(360.0);
    if !(window.hues[0]..=window.hues[1]).contains(&hue) {
        return c;
    }
    let s = (d / max * sat).min(1.0);
    let v = (max * val).min(1.0);
    let hh: f32 = target_hue / 60.0;
    let x = 1.0 - (hh % 2.0 - 1.0).abs();
    let (r1, g1, b1) = match hh as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    let m = v * (1.0 - s);
    [
        ((r1 * s * v + m) * 255.0).round().min(255.0) as u8,
        ((g1 * s * v + m) * 255.0).round().min(255.0) as u8,
        ((b1 * s * v + m) * 255.0).round().min(255.0) as u8,
    ]
}

pub(super) struct Row {
    pub role: Option<Role>,
    pub look: Option<&'static str>,
    pub set: fn() -> &'static Baked,
}

fn knight() -> &'static Baked {
    embedded_set!("knight")
}

fn a6_crusader() -> &'static Baked {
    embedded_set!("a6-crusader", ways)
}

fn frog_knight() -> &'static Baked {
    embedded_set!("frog-knight", ways)
}

pub(super) const REGISTRY: &[Row] = &[
    Row { role: Some(Role::Hero), look: None, set: knight },
    Row { role: Some(Role::Hero), look: Some("knight"), set: knight },
    Row { role: Some(Role::Hero), look: Some("a6-crusader"), set: a6_crusader },
    Row { role: Some(Role::Hero), look: Some("frog-knight"), set: frog_knight },
];

pub fn for_creation(creation: &Creation) -> &'static Baked {
    for_look(creation, creation.look.as_deref())
}

pub fn for_look(creation: &Creation, look: Option<&str>) -> &'static Baked {
    pick(REGISTRY, creation, look)
}

pub(super) fn pick(rows: &[Row], creation: &Creation, look: Option<&str>) -> &'static Baked {
    let role = if creation.role == Role::Folk { Role::Hero } else { creation.role };
    let row = look
        .and_then(|l| rows.iter().find(|r| r.look == Some(l) && r.role.is_none_or(|r| r == role)))
        .or_else(|| rows.iter().find(|r| r.look.is_none() && r.role == Some(role)));
    row.map_or_else(Baked::bailiff, |r| (r.set)())
}

#[cfg(test)]
pub fn all_sets() -> Vec<&'static Baked> {
    let mut sets = vec![Baked::bailiff()];
    for row in REGISTRY {
        let set = (row.set)();
        if !sets.iter().any(|known| std::ptr::eq(*known, set)) {
            sets.push(set);
        }
    }
    sets
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn stand_in(creation: &Creation, motion: Motion, tick: u64, walked: f32) -> Img {
    figure(creation, motion, tick, walked, None)
}

pub fn preview(creation: &Creation, tick: u64) -> Img {
    let way = for_creation(creation).turntable(tick);
    figure_facing(creation, Motion::Idle, way, tick, 0.0, None)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn figure(creation: &Creation, motion: Motion, tick: u64, walked: f32, since: Option<u64>) -> Img {
    figure_facing(creation, motion, Way::South, tick, walked, since)
}

pub fn figure_facing(creation: &Creation, motion: Motion, facing: Way, tick: u64, walked: f32, since: Option<u64>) -> Img {
    let set = for_creation(creation);
    let played = since.and_then(|s| set.shot(Reel::Blow, s, facing, tick));
    let source = match &played {
        Some(p) => &p.frame,
        None => since.and_then(|s| set.swing_facing(s, facing)).unwrap_or_else(|| set.frame_facing(motion, facing, tick, walked)),
    };
    dress(creation, set, source)
}

pub fn figure_fixed(creation: &Creation, motion: Motion, facing: Way, tick: u64, walked: f32, since: Option<u64>) -> Img {
    let set = for_creation(creation);
    dress(creation, set, since.and_then(|s| set.swing_facing(s, facing)).unwrap_or_else(|| set.frame_facing(motion, facing, tick, walked)))
}

pub fn figure_shot(creation: &Creation, reel: Reel, since: u64, facing: Way, tick: u64) -> Option<(Img, i32)> {
    let set = for_creation(creation);
    let played = set.shot(reel, since, facing, tick)?;
    Some((dress(creation, set, &played.frame), played.drop))
}

fn dress(creation: &Creation, set: &Baked, source: &Img) -> Img {
    let h = source.h + (i32::from(creation.sculpt.height.min(100)) - 50) / 5;
    let w = source.w + (i32::from(creation.sculpt.bulk.min(100)) - 50) / 5;
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(rgb) = source.get(x * source.w / w, y * source.h / h) {
                out.set(x, y, set.dye.map_or(rgb, |window| tint_with(rgb, creation.paint.scheme, window)));
            }
        }
    }
    out
}
