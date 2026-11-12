//! Runtime contract for the quick-rig asset factory (no save/wire fields).
//!
//! `assets/quick-rig/knight.json` and the four seat designs beside it
//! (`knight-<design>.json`, see [`Design`]) are compiled verbatim, once each.
//! Version 1 is
//! `{ "version":1, "canvas":[w,h], "pivot":[x,y], "clips": { "idle": [
//! { "ticks":6, "rows":["..."], "anchors": { "grip":[x,y],
//! "off_hand":[x,y], "head":[x,y], "back":[x,y], "feet":[x,y],
//! "tip":[x,y] } } ] } }`. Rows are **final facing-right** realm inks,
//! not source silhouettes: h strings of w ASCII inks (`.` is transparent).
//! All six sockets and the standing pivot are pixel coordinates in the canvas.
//! Mirroring is exactly `x = w - 1 - x`, including the pivot, never `w - x`.
//!
//! `head` is the bottom contact of a hat/helm; `back` the torso/back centre;
//! `grip` and `off_hand` are weapon/shield contacts, `feet` the shoe contact;
//! `tip` gives the weapon's direction from `grip`. Export the knight body
//! (armor/helm may be painted), without a weapon or shield: runtime equipment
//! and wishes meet those same sockets. Rigged right-slot gear points up at
//! source and turns about its existing fitted contact toward `tip` (not scaled
//! to tip distance). Native right-slot centre contacts remain centre contacts;
//! neither a Sprite painting pivot nor silhouette heuristics redefine a hilt.
//! Wished-body fit sizes are inferred once from idle ink bounds, not an
//! attack's changing ink box. The built-in knight retains native 16px equipment
//! sizes at the sampled sockets.
//!
//! Idle is required. Other standard clips may be absent: run falls back to
//! walk, attacks to slash, and everything ultimately to idle. Loop sampling
//! uses integer tick sums; one-shots clamp, not loop. No wall clock or mutable
//! renderer animation state is used. Parsing is bounded and fails atomically;
//! a bad compiled asset leaves the old knight renderer available.
//!
//! A wish still saves/transmits its checked `Sprite` ink rows. A client asset
//! factory can associate those exact rows with a validated rig for a render
//! scope using [`with_sprite_rig`]. Only a whole-body worn sprite consumes that
//! override; no filename, rig pointer, or unvalidated JSON enters a Hero/save.
//! The checked compiled [`super::factory`] catalog supplies this association
//! automatically for its exact body source rows. Explicit scopes still take
//! precedence. Without either association the old sheet is drawn normally. This is an
//! in-process registration API, not a disk loader or wish-card field: parse
//! JSON with `Rig::parse`, then wrap rendering in `with_sprite_rig`.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use serde::Deserialize;

pub mod auto;
pub mod smart;

use super::super::ink::{self, Img};
use crate::rules::looks::{Body, Sprite};

/// The names accepted by the version-1 exporter and runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clip {
    Idle,
    Walk,
    Run,
    Roll,
    Hurt,
    Die,
    Cast,
    Slash,
    Overhead,
    Thrust,
    Flail,
    Bow,
    Gun,
}

impl Clip {
    pub const ALL: [Self; 13] = [
        Self::Idle,
        Self::Walk,
        Self::Run,
        Self::Roll,
        Self::Hurt,
        Self::Die,
        Self::Cast,
        Self::Slash,
        Self::Overhead,
        Self::Thrust,
        Self::Flail,
        Self::Bow,
        Self::Gun,
    ];
}

pub type Point = [i32; 2];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchors {
    pub grip: Point,
    pub off_hand: Point,
    pub head: Point,
    pub back: Point,
    pub feet: Point,
    pub tip: Point,
}

impl Anchors {
    fn points(self) -> [Point; 6] {
        [
            self.grip,
            self.off_hand,
            self.head,
            self.back,
            self.feet,
            self.tip,
        ]
    }

    pub fn mirrored(self, width: i32) -> Self {
        let flip = |[x, y]: Point| [width - 1 - x, y];
        Self {
            grip: flip(self.grip),
            off_hand: flip(self.off_hand),
            head: flip(self.head),
            back: flip(self.back),
            feet: flip(self.feet),
            tip: flip(self.tip),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    version: u32,
    canvas: [i32; 2],
    pivot: Point,
    #[serde(deserialize_with = "unique_clips")]
    clips: BTreeMap<Clip, Vec<Frame>>,
}

// serde's default BTreeMap visitor keeps the last duplicate. Export mistakes
// must fail atomically, rather than silently throwing away an authored clip.
fn unique_clips<'de, D>(deserializer: D) -> Result<BTreeMap<Clip, Vec<Frame>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Unique;
    impl<'de> serde::de::Visitor<'de> for Unique {
        type Value = BTreeMap<Clip, Vec<Frame>>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("unique clip names")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut clips = BTreeMap::new();
            while let Some((name, frames)) = map.next_entry()? {
                if clips.insert(name, frames).is_some() {
                    return Err(serde::de::Error::custom("duplicate clip"));
                }
            }
            Ok(clips)
        }
    }
    deserializer.deserialize_map(Unique)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    ticks: u32,
    rows: Vec<String>,
    anchors: Anchors,
}

impl Frame {
    pub fn ticks(&self) -> u32 {
        self.ticks
    }
    pub fn rows(&self) -> &[String] {
        &self.rows
    }
    pub fn anchors(&self) -> Anchors {
        self.anchors
    }
}

#[derive(Debug)]
struct Animation {
    frames: Vec<Frame>,
    ticks: u64,
}

/// Validated, immutable render data. Construct only through [`Rig::parse`].
#[derive(Debug)]
pub struct Rig {
    canvas: [i32; 2],
    pivot: Point,
    clips: BTreeMap<Clip, Animation>,
    bounds: [i32; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Playback {
    Loop,
    Once,
}

/// A frame and its canvas-space transform. Images, sockets and fit are sampled
/// together so an attachment can never be one frame ahead of its body.
pub struct Sample<'a> {
    pub clip: Clip,
    pub index: usize,
    pub frame: &'a Frame,
    rig: &'a Rig,
    mirrored: bool,
}

impl Rig {
    /// Version 1 limits: 128x128 canvas, 128 frames/clip, 1024 total frames,
    /// 1..=3600 ticks/frame, 8 MiB JSON. Unknown fields/clip names are errors.
    pub fn parse(json: &str) -> Result<Self, Error> {
        let fail = |s: &str| Error(s.to_owned());
        if json.len() > 8 * 1024 * 1024 {
            return Err(fail("rig exceeds 8 MiB"));
        }
        let raw: Export = serde_json::from_str(json).map_err(|e| Error(e.to_string()))?;
        if raw.version != 1 {
            return Err(fail("unsupported rig version"));
        }
        let [w, h] = raw.canvas;
        if !(1..=128).contains(&w) || !(1..=128).contains(&h) {
            return Err(fail("rig canvas must be 1..=128 on each axis"));
        }
        let inside = |[x, y]: Point| (0..w).contains(&x) && (0..h).contains(&y);
        if !inside(raw.pivot) {
            return Err(fail("rig pivot outside canvas"));
        }
        if !raw.clips.contains_key(&Clip::Idle) {
            return Err(fail("rig needs idle"));
        }
        if raw.clips.values().map(Vec::len).sum::<usize>() > 1024 {
            return Err(fail("too many rig frames"));
        }
        let mut clips = BTreeMap::new();
        for (name, frames) in raw.clips {
            if frames.is_empty() || frames.len() > 128 {
                return Err(fail("each clip needs 1..=128 frames"));
            }
            let mut ticks = 0;
            for frame in &frames {
                if !(1..=3600).contains(&frame.ticks) {
                    return Err(fail("frame ticks must be 1..=3600"));
                }
                if frame.rows.len() != h as usize
                    || frame.rows.iter().any(|r| {
                        r.len() != w as usize
                            || !r.is_ascii()
                            || r.chars().any(|c| c != '.' && ink::ink(c).is_none())
                    })
                {
                    return Err(fail("frame rows must match canvas and use realm inks"));
                }
                if frame.anchors.points().into_iter().any(|p| !inside(p)) {
                    return Err(fail("frame anchor outside canvas"));
                }
                ticks += u64::from(frame.ticks);
            }
            clips.insert(name, Animation { frames, ticks });
        }
        let idle: Vec<_> = clips[&Clip::Idle]
            .frames
            .iter()
            .map(|f| f.rows.clone())
            .collect();
        let bounds =
            crate::rules::looks::ink_bounds(&idle).ok_or_else(|| fail("idle body has no ink"))?;
        Ok(Self {
            canvas: raw.canvas,
            pivot: raw.pivot,
            clips,
            bounds,
        })
    }

    pub fn canvas(&self) -> [i32; 2] {
        self.canvas
    }
    pub fn pivot(&self) -> Point {
        self.pivot
    }
    pub fn has(&self, clip: Clip) -> bool {
        self.clips.contains_key(&clip)
    }

    pub fn resolve(&self, clip: Clip) -> Clip {
        if self.has(clip) {
            return clip;
        }
        match clip {
            Clip::Run => self.resolve(Clip::Walk),
            Clip::Overhead | Clip::Thrust | Clip::Flail => self.resolve(Clip::Slash),
            _ => Clip::Idle,
        }
    }

    pub fn duration(&self, clip: Clip) -> u64 {
        self.clips[&self.resolve(clip)].ticks
    }

    pub fn sample(
        &self,
        clip: Clip,
        elapsed: u64,
        playback: Playback,
        mirrored: bool,
    ) -> Sample<'_> {
        let clip = self.resolve(clip);
        let animation = &self.clips[&clip];
        let mut at = match playback {
            Playback::Loop => elapsed % animation.ticks,
            Playback::Once => elapsed.min(animation.ticks - 1),
        };
        for (index, frame) in animation.frames.iter().enumerate() {
            if at < u64::from(frame.ticks) {
                return Sample {
                    clip,
                    index,
                    frame,
                    rig: self,
                    mirrored,
                };
            }
            at -= u64::from(frame.ticks);
        }
        unreachable!("validated positive duration")
    }

    /// Fit an authored one-shot into a gameplay action's countdown. This keeps
    /// a 10-tick roll or an 8-tick swing from looping a longer export midway.
    /// Scale the full period before sampling: do not cap elapsed ticks at a
    /// fixed attack length (heavy-arm cooldowns can outlast that length).
    pub fn action(&self, clip: Clip, remaining: u32, duration: u32, mirrored: bool) -> Sample<'_> {
        let duration = duration.max(1);
        let elapsed = duration.saturating_sub(remaining.min(duration));
        let at = u64::from(elapsed) * self.duration(clip) / u64::from(duration);
        self.sample(clip, at, Playback::Once, mirrored)
    }
}

impl Sample<'_> {
    pub fn image_size(&self) -> [i32; 2] {
        self.rig.canvas
    }
    pub fn anchors(&self) -> Anchors {
        if self.mirrored {
            self.frame.anchors.mirrored(self.rig.canvas[0])
        } else {
            self.frame.anchors
        }
    }
    pub fn pivot(&self) -> Point {
        let [x, y] = self.rig.pivot;
        if self.mirrored {
            [self.rig.canvas[0] - 1 - x, y]
        } else {
            [x, y]
        }
    }
    pub fn rows(&self) -> Vec<String> {
        if self.mirrored {
            self.frame
                .rows
                .iter()
                .map(|r| r.chars().rev().collect())
                .collect()
        } else {
            self.frame.rows.clone()
        }
    }
    pub fn image(&self) -> Img {
        let refs: Vec<_> = self.frame.rows.iter().map(String::as_str).collect();
        let im = Img::from_rows(&refs);
        if self.mirrored { im.flip_h() } else { im }
    }

    /// Existing wish slots: right=grip, left=off_hand, head/back/feet as named.
    /// Preserve handedness on a mirrored sample; never swap grip/off_hand.
    pub fn body(&self) -> Body {
        let [x, y, w, h] = self.rig.bounds;
        let mut body = Body::human((x, y), (w, h));
        let a = self.frame.anchors;
        let point = |[x, y]: Point| (x, y);
        body.head.anchor = point(a.head);
        body.face.anchor = (a.head[0], (a.head[1] + a.back[1]) / 2);
        body.shoulders.anchor = (a.back[0], (a.head[1] + a.back[1]) / 2);
        body.body.anchor = point(a.back);
        body.back.anchor = point(a.back);
        body.left.anchor = point(a.off_hand);
        body.right.anchor = point(a.grip);
        body.feet.anchor = point(a.feet);
        body.aura.anchor = point(a.feet);
        for part in [
            &mut body.head,
            &mut body.face,
            &mut body.shoulders,
            &mut body.body,
            &mut body.back,
            &mut body.left,
            &mut body.right,
            &mut body.feet,
            &mut body.aura,
        ] {
            part.size.0 = part.size.0.max(1);
            part.size.1 = part.size.1.max(1);
        }
        if self.mirrored {
            let flip = |p: &mut crate::rules::looks::Part| {
                p.anchor.0 = self.rig.canvas[0] - 1 - p.anchor.0
            };
            body.bounds[0] = self.rig.canvas[0] - x - w;
            for part in [
                &mut body.head,
                &mut body.face,
                &mut body.shoulders,
                &mut body.body,
                &mut body.back,
                &mut body.left,
                &mut body.right,
                &mut body.feet,
                &mut body.aura,
            ] {
                flip(part);
            }
        }
        body
    }

    /// Pin an attachment's local contact pixel to a socket (before mirroring).
    pub fn attach(&self, canvas: &mut Img, art: &Img, contact: Point, socket: Point) {
        canvas.stamp(art, socket[0] - contact[0], socket[1] - contact[1]);
    }
}

/// Rig-only right-weapon geometry, reusable by preview renderers. Coordinates
/// are pixel indices: `contact` is the *existing* fitted Part contact (not an
/// inferred hilt). Source art points up; `grip -> tip` supplies direction, not
/// length. No Sprite/save field or native attachment convention is changed.
/// Horizontal, diagonal, or downward-authored weapons need to be authored up
/// for this convention; a silhouette cannot reliably identify a hilt.
///
/// Preview recipe (also for Python's anchor overlay): `angle = atan2(tip.x -
/// grip.x, grip.y - tip.y)`, or zero for coincident anchors. Rotate pixel indices
/// by [`Self::point`]; for PNG edge coordinates subtract 0.5 before rotating,
/// then add 0.5. Mirror the final canvas with `width - 1 - x`. A preview which
/// only draws the tip vector still needs to supply fitted gear and its Part
/// contact; the rig JSON intentionally contains neither equipped art nor hilt
/// metadata. Painting pivots remain motion pivots, not hand contacts.
#[derive(Clone, Copy, Debug)]
pub struct WeaponOrientation {
    pub contact: Point,
    pub grip: Point,
    pub angle: f32,
}

impl WeaponOrientation {
    pub fn new(contact: Point, grip: Point, tip: Point) -> Self {
        let dx = (tip[0] - grip[0]) as f32;
        let dy = (tip[1] - grip[1]) as f32;
        Self {
            contact,
            grip,
            // A degenerate export keeps the source orientation, never NaN.
            angle: if dx == 0.0 && dy == 0.0 {
                0.0
            } else {
                dx.atan2(-dy)
            },
        }
    }

    fn sin_cos(&self) -> (f32, f32) {
        // Cardinal directions must not grow a spurious row/column because
        // cos(pi/2) is a tiny nonzero float.
        let snap = |v: f32| if v.abs() < 1e-6 { 0.0 } else { v };
        let (sin, cos) = self.angle.sin_cos();
        (snap(sin), snap(cos))
    }

    /// Turn a source pixel index about the exact contact pixel.
    pub fn point(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        let (sin, cos) = self.sin_cos();
        let (x, y) = (x - self.contact[0] as f32, y - self.contact[1] as f32);
        [
            self.grip[0] as f32 + x * cos - y * sin,
            self.grip[1] as f32 + x * sin + y * cos,
        ]
    }

    /// Inverse-nearest hard ink rotation. Return the full rectangle and its
    /// top-left in body pixels, including negative and below-floor extents.
    pub fn stamp(&self, source: &Img) -> (Img, Point) {
        if source.w <= 0 || source.h <= 0 {
            return (
                source.clone(),
                [
                    self.grip[0] - self.contact[0],
                    self.grip[1] - self.contact[1],
                ],
            );
        }
        let corners = [
            self.point([0.0, 0.0]),
            self.point([(source.w - 1) as f32, 0.0]),
            self.point([0.0, (source.h - 1) as f32]),
            self.point([(source.w - 1) as f32, (source.h - 1) as f32]),
        ];
        let min = |axis: usize| {
            corners
                .iter()
                .map(|p| p[axis])
                .fold(f32::INFINITY, f32::min)
                .floor() as i32
        };
        let max = |axis: usize| {
            corners
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil() as i32
        };
        let at = [min(0), min(1)];
        let mut out = Img::new(max(0) - at[0] + 1, max(1) - at[1] + 1);
        let (sin, cos) = self.sin_cos();
        for y in 0..out.h {
            for x in 0..out.w {
                let dx = (x + at[0] - self.grip[0]) as f32;
                let dy = (y + at[1] - self.grip[1]) as f32;
                let sx = (self.contact[0] as f32 + dx * cos + dy * sin).round() as i32;
                let sy = (self.contact[1] as f32 - dx * sin + dy * cos).round() as i32;
                if let Some(colour) = source.get(sx, sy) {
                    out.set(x, y, colour);
                }
            }
        }
        (out, at)
    }
}

/// The same clockwise pixel-index transform as `sprites::quarter_turn`.
/// Apply before swapping canvas dimensions, including to authored pivots.
pub(super) fn quarter_turn_point([x, y]: Point, height: i32) -> Point {
    [height - 1 - y, x]
}

/// The company's four knights, one to a seat: each its own rig
/// (`assets/quick-rig/knight-<name>.json`), every one on the same 46x46
/// canvas, pivot and clip set as `knight.json`, so whichever seat's body is
/// drawn, staging, rolls and pivots measure the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Design {
    /// The flat-topped great helm in a surcoat with a cross (seat one).
    GreatHelm,
    /// The pointed sugarloaf in a white surcoat, the cross in the seat's colour.
    SugarloafCrusader,
    /// The bucket helm with its swept plume and tabard.
    PlumedBucket,
    /// The broad-brimmed kettle hat over mail and a tunic.
    KettleHat,
}

impl Design {
    pub const ALL: [Self; 4] = [
        Self::GreatHelm,
        Self::SugarloafCrusader,
        Self::PlumedBucket,
        Self::KettleHat,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::GreatHelm => "great-helm",
            Self::SugarloafCrusader => "sugarloaf-crusader",
            Self::PlumedBucket => "plumed-bucket",
            Self::KettleHat => "kettle-hat",
        }
    }

    /// The knight a seat plays. The first four seats have one each; the fifth
    /// (sea-green) and sixth (ivory) wear the great helm and the plumed bucket
    /// again: the crusader's cross would vanish into an ivory seat's white.
    pub fn of_seat(seat: u32) -> Self {
        match seat {
            2 => Self::SugarloafCrusader,
            3 | 6 => Self::PlumedBucket,
            4 => Self::KettleHat,
            _ => Self::GreatHelm,
        }
    }

    fn json(self) -> &'static str {
        match self {
            Self::GreatHelm => include_str!("../../../../assets/quick-rig/knight-great-helm.json"),
            Self::SugarloafCrusader => {
                include_str!("../../../../assets/quick-rig/knight-sugarloaf-crusader.json")
            }
            Self::PlumedBucket => {
                include_str!("../../../../assets/quick-rig/knight-plumed-bucket.json")
            }
            Self::KettleHat => include_str!("../../../../assets/quick-rig/knight-kettle-hat.json"),
        }
    }
}

/// A design's rig, parsed once. A malformed one falls back to the realm's
/// old quick-rig knight (and that, failing, to the legacy art): never fatal.
pub fn design(design: Design) -> Option<&'static Rig> {
    #[cfg(test)]
    if LEGACY.with(std::cell::Cell::get) {
        return None;
    }
    static RIGS: [OnceLock<Option<Rig>>; 4] = [const { OnceLock::new() }; 4];
    RIGS[design as usize]
        .get_or_init(|| match Rig::parse(design.json()) {
            Ok(rig) => Some(rig),
            Err(error) => {
                tracing::warn!(%error, design = design.name(), "invalid knight rig; using the old knight");
                None
            }
        })
        .as_ref()
        .or_else(knight)
}

/// The rig a seat's plain knight is drawn with.
pub fn seat_knight(seat: u32) -> Option<&'static Rig> {
    design(Design::of_seat(seat))
}

/// The old realm knight (`knight.json`): every design's fallback.
/// A malformed replacement is deliberately not fatal to game startup.
pub fn knight() -> Option<&'static Rig> {
    #[cfg(test)]
    if LEGACY.with(std::cell::Cell::get) {
        return None;
    }
    static RIG: OnceLock<Option<Rig>> = OnceLock::new();
    RIG.get_or_init(
        || match Rig::parse(include_str!("../../../../assets/quick-rig/knight.json")) {
            Ok(rig) => Some(rig),
            Err(error) => {
                tracing::warn!(%error, "invalid quick-rig knight; using legacy art");
                None
            }
        },
    )
    .as_ref()
}

thread_local! {
    static SPRITES: std::cell::RefCell<Vec<(Vec<Vec<String>>, Arc<Rig>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Associate a wished whole-body sheet with a rig during rendering, including
/// its Body fit and roll bypass. Nested scopes restore even during unwinding.
/// The legacy checked rows remain the portable fallback on other clients.
pub fn with_sprite_rig<T>(sprite: &Sprite, rig: Arc<Rig>, draw: impl FnOnce() -> T) -> T {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            SPRITES.with(|r| {
                r.borrow_mut().pop();
            });
        }
    }
    SPRITES.with(|r| r.borrow_mut().push((sprite.frames.clone(), rig)));
    let _restore = Restore;
    draw()
}

pub(super) fn sprite_rig(sprite: &Sprite) -> Option<Arc<Rig>> {
    SPRITES.with(|r| {
        r.borrow()
            .iter()
            .rev()
            .find(|(frames, _)| *frames == sprite.frames)
            .map(|(_, rig)| rig.clone())
    })
    .or_else(|| super::factory::sprite_rig(sprite))
}

// Frozen seat-frame hashes and the mini-viz equivalence proof predate rigs.
// Keep proving that fallback byte-for-byte, without making new art a rebaseline.
#[cfg(test)]
thread_local! { static LEGACY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
pub(super) fn with_legacy_knight<T>(draw: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            LEGACY.with(|v| v.set(self.0));
        }
    }
    let _restore = Restore(LEGACY.with(|v| v.replace(true)));
    draw()
}
