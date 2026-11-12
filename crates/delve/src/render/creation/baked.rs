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

/// What the creature is doing. The library names three poses (rest, windup, strike); a baked
/// creature may bring walk cycles too, and without them a walk is the idle bob at a stride. A set
/// with eight ways also has a run, a hit, a death and a cast; a set without them plays what is
/// nearest (a run is its walk, a hit its stance, a cast its windup). A walk or run says which way
/// it goes; everything else faces the way the caller says (`Baked::frame_facing`), south if no one does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Idle,
    Walk(Way),
    Run(Way),
    Windup,
    Strike,
    /// Flinches; `tick` is the ticks since the blow landed.
    Hit,
    /// Falls and stays down; `tick` is the ticks since it fell.
    Death,
    /// Casts; `tick` is the ticks since the spell began.
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

/// What a set's `animation.json` tells the Creator (the rest of the file is for other readers).
#[derive(Clone, Debug, serde::Deserialize)]
struct Animation {
    fps: u32,
    /// The clips the set has, by name (`idle`, `walk-east`, `walk-south`, `attack`, and `walk-west`
    /// as the east walk mirrored). A clip the set lacks has no state, and its strip is one blank cell.
    states: BTreeMap<String, State>,
    attack_events: Option<Keys>,
    creator: Creator,
    /// The eight-way clips, for a set from the pipeline's frame sets (absent: the two old ways).
    ways: Option<WaysRecord>,
}

/// `animation.json`'s `ways` block: the atlas (a block of eight direction rows per clip, the frames
/// along each row) and what each clip is.
#[derive(Clone, Debug, serde::Deserialize)]
struct WaysRecord {
    /// The direction of each row of a block, in order (`s sw w nw n ne e se` as the exporter writes them).
    directions: Vec<String>,
    cell: [i32; 2],
    fps: u32,
    clips: BTreeMap<String, WayClipRecord>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct WayClipRecord {
    /// The first row of the clip's block of eight direction rows, counted in cells.
    row: i32,
    frames: usize,
    /// The sole edge of each direction's frames, by direction name.
    anchor_y: BTreeMap<String, i32>,
    #[serde(default)]
    events: BTreeMap<String, usize>,
}

/// The clips an eight-way set can bring, by the name the exporter writes.
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

/// The clips that play once, started and timed by the drive: a blow, a cast, a flinch, a fall and the
/// rise from it (the death clip backwards).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reel {
    Blow,
    Cast,
    Hit,
    Fall,
    Rise,
}

/// One frame of a clip the drive plays once, and the rows lower than bottom-centre to draw it so its soles meet the ground.
#[derive(Clone, Debug)]
pub struct Played<'a> {
    pub frame: Cow<'a, Img>,
    pub drop: i32,
}

/// The fewest ticks a blow's impact frame holds, so a one-frame impact still reads as a blow.
const MIN_STRIKE: u64 = 3;

/// One clip in all eight directions, dressed, with the sole edge of each direction's frames and the
/// clip's events by name (the frame each falls on).
#[derive(Clone, Debug)]
struct WayClip {
    frames: [Vec<Img>; 8],
    anchor: [i32; 8],
    events: BTreeMap<String, usize>,
}

/// The eight-way clips of a set, by clip.
#[derive(Clone, Debug)]
struct Ways {
    fps: u32,
    clips: BTreeMap<Clip, WayClip>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct State {
    /// The pixel edge the soles stand on in this state's own frames (bottom-centre is `[32, 53]`).
    anchor: [i32; 2],
}

/// The Creator's own block of the record: what to call the set and which pixels the pigment dyes.
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

/// Ground (run units) one frame of a twelve-frame walk cycle spans: the Bailiff's pace. A strip of
/// any other length keeps the same cycle length on the ground (24 frames: 0.31 units each), so
/// every creature steps at one cadence. Phase follows distance, never the clock.
const WALK_STRIDE: f32 = 0.62;
const WALK_FRAMES: f32 = 12.0;
/// A run's cycle is this many times as long on the ground as a walk's (the legs reach further).
const RUN_REACH: f32 = 1.75;
/// Ticks the stage holds each of the eight ways: one pass of a sixteen-frame idle at 12 fps (40 at 30 Hz).
pub const TURN_TICKS: u64 = 40;

/// Which pixels of a set the main pigment dyes: its main material's hue window (degrees), and how
/// little saturation or value a pixel may have and still count as that material. The exporter
/// derives it from the art (or takes it from the frame-set spec) and writes it into `animation.json`.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub struct Dye {
    hues: [f32; 2],
    min_sat: f32,
    min_val: f32,
}

/// The Bailiff's moss greens and yellow-greens: its tan highlights and dark outline stay.
pub const GREENS: Dye = Dye { hues: [62.0, 170.0], min_sat: 0.2, min_val: 0.08 };

/// One baked creature in the boss library's format.
#[derive(Clone, Debug)]
pub struct Baked {
    pub id: String,
    pub size: (i32, i32),
    exposure: u32,
    rest: Img,
    breath: Img,
    windup: Img,
    strike: Img,
    /// Optional walk cycles, east then south, in the boss library's strip format.
    walk: [Vec<Img>; 2],
    /// Optional idle loop (empty: the library's own breathing rule).
    idle: Vec<Img>,
    /// Optional whole smash, frame by frame (empty: windup and strike are its two poses).
    swing: Vec<Img>,
    /// `animation.json`: soles' anchors per state, the smash's keys, the clip rate.
    animation: Option<Animation>,
    /// Every clip in all eight directions, for a set from the pipeline's frame sets.
    ways: Option<Ways>,
    /// Which pixels the main pigment dyes (`None`: the set stays as made, whatever the pigment).
    dye: Option<Dye>,
    name: String,
}

/// The files of one baked set, all required (the exporter always writes every one).
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
    /// The ways atlas (`library-v1-ways.png`), for a set whose record has a `ways` block; empty otherwise.
    pub ways: &'static [u8],
}

/// A set embedded from `assets/creator/baked/<id>/`, built once: `embedded_set!("knight")`, or
/// `embedded_set!("a6-crusader", ways)` for a set that carries its eight-way atlas.
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
    /// A baked boss-library creature: its `library-import.json` and the three frames. Frames the
    /// import does not size, or of unequal size, are refused rather than drawn crooked.
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

    /// A whole set as `scripts/creator/export-baked-set.py` writes it: the library's three frames
    /// and import record, and whichever clips the animation record says the set has (both walks,
    /// the idle loop, the smash). A clip it lacks keeps the library's own fallback: the breathing
    /// rule for idle, the idle bob for a walk, the windup and strike poses for a blow.
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

    /// The eight-way clips: the atlas cut into cells, each dressed as the library dresses a frame, and
    /// checked against the record (every direction named once, every clip's block and frames inside the
    /// atlas and none blank, the sole edges there for all eight, `idle` present, the smash as long as the
    /// legacy smash strip it was written beside).
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

    /// The strip's equal cells, each dressed as the library dresses a frame.
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

    /// Walk cycles as strips of equal cells, east then south (`library-v1-walk-*-strip.png`).
    pub fn with_walks(mut self, east: &[u8], south: &[u8]) -> Result<Self, String> {
        let cycles = [self.cells(east).map_err(|e| format!("walk: {e}"))?, self.cells(south).map_err(|e| format!("walk: {e}"))?];
        self.walk = cycles;
        Ok(self)
    }

    /// The idle loop as a strip (`library-v1-idle-strip.png`), played at the clip's own rate.
    pub fn with_idle(mut self, strip: &[u8]) -> Result<Self, String> {
        self.idle = self.cells(strip).map_err(|e| format!("idle: {e}"))?;
        Ok(self)
    }

    /// The whole smash as a strip (`library-v1-attack-strip.png`); `with_animation` supplies its keys.
    pub fn with_swing(mut self, strip: &[u8]) -> Result<Self, String> {
        self.swing = self.cells(strip).map_err(|e| format!("attack: {e}"))?;
        Ok(self)
    }

    /// `animation.json`: where each state's soles stand and where the smash's keys fall. A set that
    /// carries it carries its own bob and lunge, so the drive adds none. Every clip that was loaded
    /// needs its state and every state its clip, and the smash's keys must fit its frames.
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

    /// What the stage and the plate call it ("STAND-IN: KNIGHT").
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The set draws its own bob and lunge (it came with `animation.json`).
    pub fn carries_motion(&self) -> bool {
        self.animation.is_some()
    }

    /// A real stand-in: the Bailiff, and the 2D knight that holds the Hero's place. A character from
    /// the pipeline's frame sets (eight ways, its own clips) is finished work and is not called one.
    pub fn stand_in(&self) -> bool {
        self.ways.is_none()
    }

    /// Ticks a clip of `frames` frames plays at the set's clip rate (a frame shows from the first tick
    /// that reaches it: 12 fps at 30 Hz is a frame every 2.5 ticks).
    fn clip_ticks(&self, frames: usize) -> u64 {
        let fps = self.ways.as_ref().map_or(30, |w| u64::from(w.fps));
        (frames as u64 * 30).div_ceil(fps)
    }

    /// How the set's own clips are timed, for the drive: the blow lands on the clip's impact frame
    /// (its `hit` event) and is the clip's length, a cast releases on its `cast` event, a hit and a
    /// death are their clips' lengths, and each melts into the stance over `SETTLE` ticks. A set
    /// without eight ways is the library's fixed blow and none of the others, as it always was.
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

    /// The frame of a clip the drive plays once, `since` ticks after it began, facing a way: the clip
    /// at its own frame times, then (a fall aside, which lies where it fell) its last frame melting
    /// into the stance over `SETTLE` ticks, in an ordered dither of the two frames' own pixels, so
    /// no colour is made and no pose snaps. A rise is the death clip backwards and melts from its
    /// first frame. `tick` is the set's clock for the stance it melts into. `None`: the set has no
    /// such clip (or no eight ways), or the clip and its settle are over.
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

    /// The in-game Marsh Bailiff, baked by the in-game lane (`assets/creator/baked/marsh-bailiff`).
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

    /// Ground one walk frame spans: a cycle is always as long on the ground as the Bailiff's.
    fn stride(&self, way: Way) -> f32 {
        WALK_STRIDE * (WALK_FRAMES / self.walk[slot(way)].len().max(1) as f32)
    }

    /// Whether the set has all eight ways (a set from the pipeline's frame sets) or only the old two.
    pub fn eight_way(&self) -> bool {
        self.ways.is_some()
    }

    /// Which way the stage shows `tick`: round all eight, each held for one pass of the idle loop
    /// (`TURN_TICKS`), south first (the way the old art always stood). A set with two ways stays south.
    pub fn turntable(&self, tick: u64) -> Way {
        if self.ways.is_some() { Way::nth((tick / TURN_TICKS) as usize) } else { Way::South }
    }

    /// The clips the set brings in all eight directions, by name (`idle`, `walk`, `run`, `attack`,
    /// `hit`, `death`, `cast`); none for a set with two ways.
    pub fn way_clips(&self) -> Vec<&'static str> {
        self.ways.as_ref().map_or_else(Vec::new, |w| Clip::ALL.iter().filter(|(_, c)| w.clips.contains_key(c)).map(|(n, _)| *n).collect())
    }

    /// One clip's frames in one direction (`None`: the set has no such clip, or no eight ways).
    pub fn way_frames(&self, clip: &str, way: Way) -> Option<&[Img]> {
        let (_, clip) = Clip::ALL.iter().find(|(n, _)| *n == clip)?;
        Some(&self.ways.as_ref()?.clips.get(clip)?.frames[way.index()])
    }

    /// The frame a clip's event falls on (`hit`, `cast`, `windup`...), by name: `None` where the set has no such clip or event.
    pub fn way_event(&self, clip: &str, event: &str) -> Option<usize> {
        let (_, clip) = Clip::ALL.iter().find(|(n, _)| *n == clip)?;
        self.ways.as_ref()?.clips.get(clip)?.events.get(event).copied()
    }

    /// The frame for a motion, facing south (a set with eight ways has the others: `frame_facing`).
    pub fn frame(&self, motion: Motion, tick: u64, walked: f32) -> &Img {
        self.frame_facing(motion, Way::South, tick, walked)
    }

    /// The frame for a motion facing a way. Idle plays the set's own loop at its clip rate, or follows
    /// the library's own breathing rule; a walk or run follows the ground covered (a set without strips
    /// swaps its two idle frames once per stride, so feet and bob stay together). A set with two ways
    /// shows every way as the nearer of east and south (the caller turns an east walk about for the
    /// west); a set with eight shows the way itself. `tick` is the set's clock, or for a hit, death and
    /// cast the ticks since it began.
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

    /// The eight-way frame for a motion, or `None` where the set has no clip for it and the legacy
    /// frames answer (a cast with no cast clip is the windup pose, a hit or a fall the stance).
    fn way_frame<'a>(&'a self, ways: &'a Ways, motion: Motion, facing: Way, tick: u64, walked: f32) -> Option<&'a Img> {
        // A walk and a run go the way they say; everything else faces the way it is told.
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

    /// The smash frame `since` ticks after the blow began, for a set that has the whole smash,
    /// facing south. `None` once the smash is over, or without a smash.
    pub fn swing(&self, since: u64) -> Option<&Img> {
        self.swing_facing(since, Way::South)
    }

    /// `swing`, in the way the creature faces (a set with two ways has its one smash).
    pub fn swing_facing(&self, since: u64, facing: Way) -> Option<&Img> {
        let at = self.swing_index(since)?;
        match self.ways.as_ref().and_then(|w| w.clips.get(&Clip::Attack)) {
            Some(attack) => attack.frames[facing.index()].get(at),
            None => self.swing.get(at),
        }
    }

    /// Which frame of the smash strip: the drive's windup, strike and recovery
    /// (`rules::creation::drive`) laid over the clip's own keys. The rise plays a frame a tick,
    /// holds at its peak, swings on the tick before the blow and lands its strike frame on the first
    /// strike tick; recovery plays the clip's long settle in the ticks the drive gives it.
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

    /// Rows lower than bottom-centre to draw a motion's frames so its soles meet the ground, facing
    /// south: the front walk is authored two rows high (room for the stepping foot), and says so.
    pub fn drop(&self, motion: Motion) -> i32 {
        self.drop_facing(motion, Way::South)
    }

    /// `drop`, for the way the creature faces: an eight-way set declares each direction's own sole edge.
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

/// `to` showing through `from` by `share` (`0..1`), a pixel at a time in the game's own 4x4 ordered
/// dither: every pixel is one frame's or the other's, so the palette is the two frames' own. A pixel
/// that is empty in the frame it takes from stays empty.
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

/// The attack frame the library's windup pose is: the coil's peak, the frame before the swing (never
/// before the windup key), as the exporter picks it and the smash holds it.
fn windup_of(k: &Keys) -> usize {
    (k.swing as usize).saturating_sub(1).max(k.windup_key as usize)
}

/// Which of a two-way set's walk strips a way plays: east's, or south's (north is the front walk,
/// every way with a left part is east's turned about by the caller).
fn slot(way: Way) -> usize {
    usize::from(way.flat().0 == Way::South)
}

/// The stand-in is moss green already; Ochre (the default) and Moss leave it as made, and the
/// other pigments turn its greens, keeping every step of its shading.
#[cfg_attr(not(test), allow(dead_code))]
pub fn tint(c: [u8; 3], dye: Pigment) -> [u8; 3] {
    tint_with(c, dye, GREENS)
}

/// `tint` for any set: the pigment turns the pixels inside the set's dye window and no others.
/// Ochre and Moss are the earth pigments: they leave a baked creature as its artist made it.
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
    // Only the set's main material: its highlights, trim and dark outline stay.
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

/// One row of the registry: a role (`None`: any) and a look (`None`: any) to the set that shows it.
pub(super) struct Row {
    pub role: Option<Role>,
    pub look: Option<&'static str>,
    pub set: fn() -> &'static Baked,
}

/// The Hero's three looks, one set each (`assets/creator/baked/<id>/`): the 2D knight (the T-pose rig
/// route, test data from lane H), the A6 crusader (the night pipeline: SpriteKiln body, painted from the
/// approved concept, eight ways, every clip) and the frog knight (a wish through `wish2char`, the same
/// eight ways). A fn, not a closure per row, so two rows that name one set share its decoded frames.
fn knight() -> &'static Baked {
    embedded_set!("knight")
}

fn a6_crusader() -> &'static Baked {
    embedded_set!("a6-crusader", ways)
}

fn frog_knight() -> &'static Baked {
    embedded_set!("frog-knight", ways)
}

/// Which baked set shows which creation. A look (a wish's or a bake's art id) outranks a role;
/// a role with no row gets the Bailiff. A new creature is one new row here after its set is
/// exported (`scripts/creator/export-baked-set.py`). The Hero's first row, with no look, is its
/// default: the 2D knight. The rows with a look are what the Creator's Look row and the Test
/// drive's 1 to 3 keys pick (`rules::creation::HERO_LOOKS` names them; a test holds the two tables
/// together).
pub(super) const REGISTRY: &[Row] = &[
    Row { role: Some(Role::Hero), look: None, set: knight },
    Row { role: Some(Role::Hero), look: Some("knight"), set: knight },
    Row { role: Some(Role::Hero), look: Some("a6-crusader"), set: a6_crusader },
    Row { role: Some(Role::Hero), look: Some("frog-knight"), set: frog_knight },
];

/// The set that shows a creation (the preview stage and the Test drive both ask): the look it
/// wears, or its role's default.
pub fn for_creation(creation: &Creation) -> &'static Baked {
    for_look(creation, creation.look.as_deref())
}

/// `for_creation`, for a creation that has a look: a row naming that look wins, then the first row
/// for the role (and no look of its own), then the Bailiff.
pub fn for_look(creation: &Creation, look: Option<&str>) -> &'static Baked {
    pick(REGISTRY, creation, look)
}

pub(super) fn pick(rows: &[Row], creation: &Creation, look: Option<&str>) -> &'static Baked {
    // A Folk is a person of the world and wears the Hero's looks (`Role::looks`).
    let role = if creation.role == Role::Folk { Role::Hero } else { creation.role };
    let row = look
        .and_then(|l| rows.iter().find(|r| r.look == Some(l) && r.role.is_none_or(|r| r == role)))
        .or_else(|| rows.iter().find(|r| r.look.is_none() && r.role == Some(role)));
    row.map_or_else(Baked::bailiff, |r| (r.set)())
}

/// Every set the registry can show, the stand-in first, each once (two rows may share a set).
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

/// The made creature's figure at this moment: height and bulk reshape it, the main pigment dyes
/// it. This is the stand-in's whole response; limbs, head and parts wait for assembled art. (The
/// stage and the drive ask `preview` and `figure_facing`; the tests and the shot writers ask these.)
#[cfg_attr(not(test), allow(dead_code))]
pub fn stand_in(creation: &Creation, motion: Motion, tick: u64, walked: f32) -> Img {
    figure(creation, motion, tick, walked, None)
}

/// What the preview stage shows `tick`: the stand-in idling, turning through the eight ways if its
/// set has them (`Baked::turntable`), and still facing the camera if not.
pub fn preview(creation: &Creation, tick: u64) -> Img {
    let way = for_creation(creation).turntable(tick);
    figure_facing(creation, Motion::Idle, way, tick, 0.0, None)
}

/// `stand_in`, with the smash's own frame when the drive is `since` ticks into a blow and the
/// creature's set has the whole smash (otherwise the motion's pose, as before).
#[cfg_attr(not(test), allow(dead_code))]
pub fn figure(creation: &Creation, motion: Motion, tick: u64, walked: f32, since: Option<u64>) -> Img {
    figure_facing(creation, motion, Way::South, tick, walked, since)
}

/// `figure`, facing a way (the way matters to a set with eight; a set with two draws its one art). A set
/// with a clip of its own for the blow plays it at the clip's own frame times when `since` says the blow
/// is on; the others play the library's fixed smash.
pub fn figure_facing(creation: &Creation, motion: Motion, facing: Way, tick: u64, walked: f32, since: Option<u64>) -> Img {
    let set = for_creation(creation);
    let played = since.and_then(|s| set.shot(Reel::Blow, s, facing, tick));
    let source = match &played {
        Some(p) => &p.frame,
        None => since.and_then(|s| set.swing_facing(s, facing)).unwrap_or_else(|| set.frame_facing(motion, facing, tick, walked)),
    };
    dress(creation, set, source)
}

/// `figure_facing` with only the library's fixed smash, whatever clips the set has (what a set with
/// two ways has always been shown, and the proof's "before").
pub fn figure_fixed(creation: &Creation, motion: Motion, facing: Way, tick: u64, walked: f32, since: Option<u64>) -> Img {
    let set = for_creation(creation);
    dress(creation, set, since.and_then(|s| set.swing_facing(s, facing)).unwrap_or_else(|| set.frame_facing(motion, facing, tick, walked)))
}

/// A frame of a clip the drive plays once (`Baked::shot`), reshaped and dyed as the creation says,
/// and the rows lower than bottom-centre to draw it. `None`: the set has no such clip, or it is over.
pub fn figure_shot(creation: &Creation, reel: Reel, since: u64, facing: Way, tick: u64) -> Option<(Img, i32)> {
    let set = for_creation(creation);
    let played = set.shot(reel, since, facing, tick)?;
    Some((dress(creation, set, &played.frame), played.drop))
}

/// The made creature's figure from a frame of its set: height and bulk reshape it, the main pigment dyes it.
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
