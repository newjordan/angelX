//! The brand guide's clamps (docs/BRAND.md): how bright and how coloured
//! each class of thing in the world may be, as numbers the engine holds
//! every asset to, so a world built quickly by many hands still reads as
//! one place and never as eye chaos.
//!
//! The backbone is one value ladder: every place's ramp has the same eight
//! brightnesses, dark to light, and only its hue is its own. Each surface
//! kind belongs to a class; the class fixes the tone it is made with (its
//! brightness before light, [`Band::tone`]) and the highest step of the
//! ladder light may lift it to ([`Band::ceiling`], held by the shader). The
//! top step is kept for what deserves the eye: water's shimmer and what a
//! knight can use. Only light (lamps, fire, a lit window, a glint) and the
//! figures stand above the ladder, and a frame may have little of either.
//!
//! The audits ([`audit_ramp`], [`audit_look`], [`audit_frame`],
//! [`audit_sprite`]) say what breaks the guide, rule by rule, so an agent
//! building the world checks its own work before anyone looks at it.
//!
//! Brightness here is luma: Rec. 709 weights on sRGB-encoded channels, 0 to
//! 1 ([`crate::look::luminance`], the shader's `luma`). Chroma is the
//! largest channel less the smallest, 0 to 1.

use crate::look::{Look, luminance};
use crate::set::Kind;

/// What a thing is to the brand guide: the class sets its brightness and
/// colour, so whoever makes it says only what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    /// The dark itself: a pit, the black past the room. Step 0.
    Void,
    /// A way through: a doorway, a slot, a gate's mouth. Dark, and found by
    /// the light set at it, never by paint.
    Opening,
    /// What is walked on: floors, ground, sand, rugs. The brightest stone in
    /// a room, so the play stands out on it, but never the top step.
    Ground,
    /// The way on: a road, a path. Ground's ceiling, a brighter tone.
    Route,
    /// Water: it runs in bands and glints.
    Water,
    /// The architecture and the land's bulk: walls, pillars, plinths, roofs,
    /// rock, the woods. Darker than the ground it stands on.
    Mass,
    /// Dressing nobody uses: crates, rubble, bones, cloth, timber. Never
    /// brighter than a mass.
    Prop,
    /// What a knight can use: a chest, a lever, a pickup, the stairs down.
    /// The top step is theirs, and a light marks them.
    Interactable,
    /// What shines by itself: a lamp, fire, a lit window, a glint. The only
    /// warm colour and the only world brightness above the ladder.
    Light,
    /// Knights, monsters, folk: sprites in their own colours.
    Figure,
    /// Shots, sparks, spells: brief, small.
    Effect,
}

/// A class's clamps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// The tone a surface of it is made with (brightness before light),
    /// least and most, 0 to 1.
    pub tone: (f32, f32),
    /// The highest step of an eight-step ladder light may lift it to (0
    /// darkest, 7 lightest); `None`: it is not put on the ladder (a light, a
    /// figure, an effect).
    pub ceiling: Option<u8>,
    /// The most chroma its colour may have where it is drawn.
    pub chroma: f32,
}

impl Class {
    pub const ALL: [Class; 11] = [
        Class::Void,
        Class::Opening,
        Class::Ground,
        Class::Route,
        Class::Water,
        Class::Mass,
        Class::Prop,
        Class::Interactable,
        Class::Light,
        Class::Figure,
        Class::Effect,
    ];

    /// Its name in docs/BRAND.md.
    pub fn name(self) -> &'static str {
        match self {
            Class::Void => "void",
            Class::Opening => "opening",
            Class::Ground => "ground",
            Class::Route => "route",
            Class::Water => "water",
            Class::Mass => "mass",
            Class::Prop => "prop",
            Class::Interactable => "interactable",
            Class::Light => "light",
            Class::Figure => "figure",
            Class::Effect => "effect",
        }
    }

    /// Its clamps (docs/BRAND.md, "Classes").
    pub fn band(self) -> Band {
        let band = |lo, hi, ceiling, chroma| Band { tone: (lo, hi), ceiling, chroma };
        match self {
            Class::Void => band(0.0, 0.06, Some(0), ENV_CHROMA),
            Class::Opening => band(0.04, 0.14, Some(2), ENV_CHROMA),
            Class::Ground => band(0.12, 0.40, Some(6), ENV_CHROMA),
            Class::Route => band(0.36, 0.50, Some(6), ENV_CHROMA),
            Class::Water => band(0.40, 0.55, Some(7), ENV_CHROMA),
            Class::Mass => band(0.10, 0.34, Some(5), ENV_CHROMA),
            Class::Prop => band(0.12, 0.34, Some(5), ENV_CHROMA),
            Class::Interactable => band(0.30, 0.45, Some(7), ENV_CHROMA),
            Class::Light => band(0.80, 1.0, None, WARM_CHROMA),
            Class::Figure => band(0.0, 1.0, None, 1.0),
            Class::Effect => band(0.0, 1.0, None, 1.0),
        }
    }

    /// A tone for this class: `tone` held inside its band.
    pub fn clamp_tone(self, tone: f32) -> f32 {
        let (lo, hi) = self.band().tone;
        if tone.is_finite() { tone.clamp(lo, hi) } else { lo }
    }

    /// Its ceiling on a ramp of `len` steps: the eight-step ladder's step
    /// scaled to the ramp, `None` off the ladder.
    pub fn ceiling_on(self, len: usize) -> Option<usize> {
        let step = self.band().ceiling?;
        let last = len.max(1) - 1;
        Some(((f32::from(step) * last as f32 / 7.0).round() as usize).min(last))
    }

    /// What a material carries for the shader to hold it to
    /// ([`crate::scene::Material::ceiling`]): the eight-step ladder's step
    /// plus one; 0 for no ceiling.
    pub fn material_ceiling(self) -> u8 {
        self.band().ceiling.map_or(0, |s| s + 1)
    }
}

/// The class a set's surface kind belongs to.
pub fn class_of(kind: Kind) -> Class {
    match kind {
        Kind::Ground | Kind::Grass | Kind::Sand | Kind::Mud | Kind::Floor | Kind::Rug | Kind::Crop => Class::Ground,
        Kind::Path | Kind::Road => Class::Route,
        Kind::Water => Class::Water,
        Kind::Stone
        | Kind::Brick
        | Kind::Plaster
        | Kind::Roof
        | Kind::Thatch
        | Kind::Slate
        | Kind::Rock
        | Kind::Foliage
        | Kind::Trunk
        | Kind::Hedge => Class::Mass,
        Kind::Wood | Kind::Metal | Kind::Cloth | Kind::Bone => Class::Prop,
        Kind::Door => Class::Opening,
        Kind::Window | Kind::Fire => Class::Light,
    }
}

/// The most chroma an environment ramp's entry (and a surface on it) has.
pub const ENV_CHROMA: f32 = 0.20;
/// How far a ramp's hue may lean from its dark end to its light, degrees.
pub const HUE_DRIFT: f32 = 90.0;
/// The most chroma the warm ramp (lamplight) and a light's colour have.
pub const WARM_CHROMA: f32 = 0.55;

/// A value ladder: the eight brightnesses every place's ramp keeps, dark to
/// light. Its top step is the brightest the environment can be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ladder {
    pub name: &'static str,
    pub steps: [f32; 8],
}

/// Stone and dark: dungeons, castles' masses, interiors, night. The
/// reference corridor (docs/BRAND.md) and the monolith's castle.
pub const DUNGEON: Ladder = Ladder { name: "dungeon", steps: [0.060, 0.090, 0.125, 0.170, 0.215, 0.280, 0.365, 0.500] };
/// Open country under the sky: a step lighter at the top, the same dark.
pub const OPEN: Ladder = Ladder { name: "open", steps: [0.066, 0.112, 0.162, 0.218, 0.287, 0.368, 0.468, 0.604] };
/// The deepest places (Dragon Keep, the Fungal Deep, the Unknown, the
/// wizard's tower): a step darker all the way up. Going down, it gets
/// darker.
pub const DEEP: Ladder = Ladder { name: "deep", steps: [0.044, 0.069, 0.098, 0.132, 0.172, 0.226, 0.302, 0.417] };
pub const LADDERS: [Ladder; 3] = [DUNGEON, OPEN, DEEP];

impl Ladder {
    /// How far a step may stray from the ladder's: an eighth of it, and
    /// never less than 0.012.
    pub fn tolerance(&self, step: usize) -> f32 {
        (self.steps[step.min(7)] * 0.125).max(0.012)
    }
}

/// The least any pixel of a frame is, but for a sliver (its first
/// percentile): the dark is deep, never crushed to black.
pub const BLACK_POINT: f32 = 0.03;
/// Where the middle of a frame sits (its median's least and most).
pub const MIDDLE: (f32, f32) = (0.12, 0.26);
/// The most its 95th percentile is: the bulk of a frame stays on the
/// ladder's lower two thirds.
pub const BULK_TOP: f32 = 0.42;
/// Over the dungeon ladder's top: the share of a frame (0 to 1) that may
/// be brighter, and the share that may be strongly coloured (chroma over
/// [`VIVID`]). Lights and figures only.
pub const BRIGHT: f32 = 0.505;
pub const BRIGHT_SHARE: f32 = 0.01;
pub const VIVID: f32 = 0.30;
pub const VIVID_SHARE: f32 = 0.01;

/// Lamps: the most each may be, and the most a look lights with (light =
/// activity: a few places, not a field of them; a set has fewer still,
/// [`SET_LAMPS`]).
pub const LAMP_STRENGTH: f32 = 1.0;
pub const LAMPS_PER_LOOK: usize = 8;
/// The sun's light: its ambient's least and most, and its strength's (a
/// room's high window is dimmer than the open sky's sun: its lamps carry
/// it).
pub const SUN_AMBIENT: (f32, f32) = (0.18, 0.32);
pub const SUN_STRENGTH: (f32, f32) = (0.5, 1.25);
/// Accents (what keeps its colour undithered: a glint, a flame, a lit
/// window, an ember crust): the most a look keeps.
pub const ACCENTS: usize = 4;

/// Sprites (figures): the most their mean brightness is, and the share of
/// their pixels that may be above [`BRIGHT`]. A knight is the brightest
/// figure in the room (the live knights sit at 0.46 to 0.56); a monster or
/// a neighbour never outshines one.
pub const KNIGHT_MEAN: f32 = 0.58;
pub const KNIGHT_BRIGHT_SHARE: f32 = 0.70;
pub const FIGURE_MEAN: f32 = 0.50;
pub const FIGURE_BRIGHT_SHARE: f32 = 0.50;

/// One broken rule: which, and what broke it.
#[derive(Clone, Debug, PartialEq)]
pub struct Breach {
    pub rule: &'static str,
    pub detail: String,
}

fn breach(rule: &'static str, detail: String) -> Breach {
    Breach { rule, detail }
}

fn unit(rgb: [u8; 3]) -> [f32; 3] {
    rgb.map(|c| f32::from(c) / 255.0)
}

/// A colour's luma, 0 to 1.
pub fn luma(rgb: [u8; 3]) -> f32 {
    luminance(unit(rgb))
}

/// A colour's chroma: its largest channel less its smallest, 0 to 1.
pub fn chroma(rgb: [u8; 3]) -> f32 {
    let (hi, lo) = (rgb.iter().max().copied().unwrap_or(0), rgb.iter().min().copied().unwrap_or(0));
    f32::from(hi - lo) / 255.0
}

/// A colour's hue in degrees, `None` for a grey.
pub fn hue(rgb: [u8; 3]) -> Option<f32> {
    let [r, g, b] = unit(rgb);
    let (hi, lo) = (r.max(g).max(b), r.min(g).min(b));
    let c = hi - lo;
    if c < 1e-4 {
        return None;
    }
    let h = if hi == r {
        ((g - b) / c).rem_euclid(6.0)
    } else if hi == g {
        (b - r) / c + 2.0
    } else {
        (r - g) / c + 4.0
    };
    Some(h * 60.0)
}

/// The ladder a ramp keeps best (the one its steps stray from least).
pub fn ladder_of(ramp: &[[u8; 3]]) -> Ladder {
    let miss = |l: &Ladder| -> f32 {
        ramp.iter().enumerate().map(|(i, &c)| (luma(c) - l.steps[step_of(i, ramp.len())]).abs()).sum()
    };
    LADDERS.into_iter().min_by(|a, b| miss(a).total_cmp(&miss(b))).unwrap_or(DUNGEON)
}

/// Entry `i` of a ramp of `len` as a step of the eight-step ladder.
fn step_of(i: usize, len: usize) -> usize {
    if len <= 1 { 0 } else { ((i as f32 * 7.0 / (len - 1) as f32).round() as usize).min(7) }
}

/// A place's ramp against `ladder`: its brightnesses on the ladder's, its
/// colour within [`ENV_CHROMA`], one hue throughout (its entries from the
/// third up within [`HUE_DRIFT`] degrees of each other: a ramp may lean
/// warmer toward its light, as the meadow's sage does, but never runs
/// through the rainbow).
pub fn audit_ramp(name: &str, ramp: &[[u8; 3]], ladder: &Ladder) -> Vec<Breach> {
    let mut out = Vec::new();
    for (i, &c) in ramp.iter().enumerate() {
        let step = step_of(i, ramp.len());
        let (want, l) = (ladder.steps[step], luma(c));
        if (l - want).abs() > ladder.tolerance(step) {
            out.push(breach(
                "ramp-value",
                format!("{name}[{i}] is {l:.3} bright; the {} ladder's step {step} is {want:.3}", ladder.name),
            ));
        }
        if chroma(c) > ENV_CHROMA + 1e-4 {
            out.push(breach("ramp-chroma", format!("{name}[{i}] has chroma {:.2}, over {ENV_CHROMA}", chroma(c))));
        }
    }
    let hues: Vec<f32> = ramp.iter().skip(2).filter(|&&c| chroma(c) > 0.04).filter_map(|&c| hue(c)).collect();
    if let Some(&first) = hues.first() {
        let off = |h: f32| ((h - first + 540.0).rem_euclid(360.0) - 180.0).abs();
        if let Some(worst) = hues.iter().map(|&h| off(h)).reduce(f32::max)
            && worst > HUE_DRIFT
        {
            out.push(breach("ramp-hue", format!("{name} wanders {worst:.0} degrees in hue; a place has one hue")));
        }
    }
    out
}

/// A warm (lamplight) ramp against the base ramp's ladder: lamplight warms
/// a surface and lifts it by at most a step, never more.
pub fn audit_warm(ramp: &[[u8; 3]], ladder: &Ladder) -> Vec<Breach> {
    let mut out = Vec::new();
    for (i, &c) in ramp.iter().enumerate() {
        let step = step_of(i, ramp.len());
        let most = ladder.steps[(step + 1).min(7)] + 1.5 * ladder.tolerance(step + 1) + if step == 7 { 0.25 } else { 0.0 };
        if luma(c) > most {
            out.push(breach("warm-value", format!("warm[{i}] is {:.3} bright, over a step above the ladder ({most:.3})", luma(c))));
        }
        if chroma(c) > WARM_CHROMA + 1e-4 {
            out.push(breach("warm-chroma", format!("warm[{i}] has chroma {:.2}, over {WARM_CHROMA}", chroma(c))));
        }
    }
    out
}

/// A look against the guide: every ramp (its own, its zones', its water's)
/// on a ladder, its lamplight a step at most, its accents few and lights,
/// its sun and lamps in their bounds.
pub fn audit_look(look: &Look) -> Vec<Breach> {
    let mut out = Vec::new();
    let ladder = ladder_of(&look.ramp);
    out.extend(audit_ramp("ramp", &look.ramp, &ladder));
    for (z, zone) in look.zones.iter().enumerate() {
        out.extend(audit_ramp(&format!("zone {z}"), &zone.ramp, &ladder_of(&zone.ramp)));
    }
    if let Some(water) = &look.water {
        out.extend(audit_ramp("water", water, &ladder_of(water)));
    }
    if let Some(warm) = &look.warm {
        out.extend(audit_warm(warm, &ladder));
    }
    if look.accent.len() > ACCENTS {
        out.push(breach("accents", format!("{} accents; a look keeps {ACCENTS} at most", look.accent.len())));
    }
    if let Some(sun) = &look.sun {
        if !(SUN_AMBIENT.0..=SUN_AMBIENT.1).contains(&sun.ambient) {
            out.push(breach("sun-ambient", format!("ambient {} outside {:?}", sun.ambient, SUN_AMBIENT)));
        }
        if !(SUN_STRENGTH.0..=SUN_STRENGTH.1).contains(&sun.strength) {
            out.push(breach("sun-strength", format!("strength {} outside {:?}", sun.strength, SUN_STRENGTH)));
        }
        if sun.shadow.is_none() {
            out.push(breach("sun-shadow", "a sun with no shadows: the long shadow is the look".into()));
        }
    }
    if look.lamps.len() > LAMPS_PER_LOOK {
        out.push(breach("lamps", format!("{} lamps; light = activity, {LAMPS_PER_LOOK} at most", look.lamps.len())));
    }
    for (i, lamp) in look.lamps.iter().enumerate() {
        if lamp.strength > LAMP_STRENGTH + 0.1 {
            out.push(breach("lamp-strength", format!("lamp {i} at {:.2}; {LAMP_STRENGTH} at most (a flicker may lift it a tenth)", lamp.strength)));
        }
    }
    out
}

/// A set (a room, a place, a chunk; crates/stage/src/set.rs) against the
/// brutalist guide: few, big shapes of few kinds, nothing finer than a
/// quarter tile but a light, and light only where something happens.
pub const SET_SHAPES: usize = 60;
pub const SET_KINDS: usize = 5;
/// The least a shape's extent may be, stage pixels (a quarter of a tile);
/// an axis a shape is flat along does not count. Lights may be finer.
pub const MIN_FEATURE: f32 = 4.0;
pub const SET_LAMPS: usize = 3;

pub fn audit_set(set: &crate::set::Set) -> Vec<Breach> {
    let mut out = Vec::new();
    let name = &set.id;
    if set.shapes.len() > SET_SHAPES {
        out.push(breach("set-shapes", format!("{name}: {} shapes; fewer, bigger ones ({SET_SHAPES} at most)", set.shapes.len())));
    }
    let mut kinds: Vec<Kind> = set.shapes.iter().map(|s| s.kind).filter(|&k| class_of(k) != Class::Light).collect();
    kinds.sort_by_key(|k| k.name());
    kinds.dedup();
    if kinds.len() > SET_KINDS {
        let names: Vec<&str> = kinds.iter().map(|k| k.name()).collect();
        out.push(breach("set-kinds", format!("{name}: {} kinds ({}); {SET_KINDS} at most", kinds.len(), names.join(" "))));
    }
    let fine: Vec<usize> = set
        .shapes
        .iter()
        .filter(|s| class_of(s.kind) != Class::Light)
        .filter(|s| s.size.iter().any(|&v| v > 0.0 && v < MIN_FEATURE))
        .map(|s| s.line)
        .collect();
    if !fine.is_empty() {
        let lines: Vec<String> = fine.iter().take(8).map(|l| l.to_string()).collect();
        out.push(breach(
            "set-detail",
            format!("{name}: {} shapes finer than {MIN_FEATURE} px (lines {}{})", fine.len(), lines.join(", "), if fine.len() > 8 { ", ..." } else { "" }),
        ));
    }
    if set.lamps.len() > SET_LAMPS {
        out.push(breach("set-lamps", format!("{name}: {} lamps; light = activity, {SET_LAMPS} at most", set.lamps.len())));
    }
    for (i, lamp) in set.lamps.iter().enumerate() {
        if lamp.strength > LAMP_STRENGTH {
            out.push(breach("lamp-strength", format!("{name}: lamp {i} at {}; {LAMP_STRENGTH} at most", lamp.strength)));
        }
    }
    if let Some(ground) = &set.ground {
        let (lo, hi) = (Class::Ground.band().tone.0, Class::Route.band().tone.1);
        let off = ground.tones.iter().filter(|&&t| !(lo..=hi).contains(&(f32::from(t) / 100.0))).count();
        if off > 0 {
            out.push(breach("ground-tone", format!("{name}: {off} ground tones outside {:.0}..{:.0}", lo * 100.0, hi * 100.0)));
        }
    }
    out
}

/// What a frame's brightness is, the numbers [`audit_frame`] judges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameStats {
    pub p1: f32,
    pub p50: f32,
    pub p95: f32,
    pub p99: f32,
    /// The share of pixels brighter than [`BRIGHT`], and more coloured
    /// than [`VIVID`].
    pub bright: f32,
    pub vivid: f32,
}

/// The brightness numbers of a frame of RGB pixels.
pub fn frame_stats(pixels: &[[u8; 3]]) -> FrameStats {
    if pixels.is_empty() {
        return FrameStats { p1: 0.0, p50: 0.0, p95: 0.0, p99: 0.0, bright: 0.0, vivid: 0.0 };
    }
    let mut counts = [0u32; 256];
    let (mut bright, mut vivid) = (0usize, 0usize);
    for &p in pixels {
        let l = luma(p);
        counts[(l * 255.0).round().clamp(0.0, 255.0) as usize] += 1;
        bright += usize::from(l > BRIGHT);
        vivid += usize::from(chroma(p) > VIVID);
    }
    let n = pixels.len() as f32;
    let at = |q: f32| {
        let want = (q * n).ceil().max(1.0) as u32;
        let mut seen = 0u32;
        for (v, &c) in counts.iter().enumerate() {
            seen += c;
            if seen >= want {
                return v as f32 / 255.0;
            }
        }
        1.0
    };
    FrameStats { p1: at(0.01), p50: at(0.5), p95: at(0.95), p99: at(0.99), bright: bright as f32 / n, vivid: vivid as f32 / n }
}

/// A finished frame against the guide: deep dark that is never crushed,
/// its middle low, its bulk on the lower ladder, and little brightness or
/// strong colour (lights and figures only).
pub fn audit_frame(pixels: &[[u8; 3]]) -> (FrameStats, Vec<Breach>) {
    let s = frame_stats(pixels);
    let mut out = Vec::new();
    if s.p1 < BLACK_POINT {
        out.push(breach("black-point", format!("1st percentile {:.3}, under {BLACK_POINT}: the dark is crushed", s.p1)));
    }
    if !(MIDDLE.0..=MIDDLE.1).contains(&s.p50) {
        out.push(breach("middle", format!("median {:.3} outside {MIDDLE:?}", s.p50)));
    }
    if s.p95 > BULK_TOP {
        out.push(breach("bulk", format!("95th percentile {:.3}, over {BULK_TOP}: too much of the frame is bright", s.p95)));
    }
    if s.bright > BRIGHT_SHARE {
        out.push(breach("bright-share", format!("{:.2}% of the frame over {BRIGHT}; {:.0}% at most", s.bright * 100.0, BRIGHT_SHARE * 100.0)));
    }
    if s.vivid > VIVID_SHARE {
        out.push(breach("vivid-share", format!("{:.2}% of the frame over chroma {VIVID}; {:.0}% at most", s.vivid * 100.0, VIVID_SHARE * 100.0)));
    }
    (s, out)
}

/// A figure's sprite (RGBA, its transparent pixels left out) against the
/// guide: figures keep their colours, but a knight is the brightest thing
/// that walks, and nothing else that walks outshines one.
pub fn audit_sprite(name: &str, pixels: &[[u8; 4]], knight: bool) -> Vec<Breach> {
    let shown: Vec<[u8; 3]> = pixels.iter().filter(|p| p[3] >= 128).map(|p| [p[0], p[1], p[2]]).collect();
    let mut out = Vec::new();
    if shown.is_empty() {
        return out;
    }
    let n = shown.len() as f32;
    let mean = shown.iter().map(|&p| luma(p)).sum::<f32>() / n;
    let bright = shown.iter().filter(|&&p| luma(p) > BRIGHT).count() as f32 / n;
    let (most, share, who) =
        if knight { (KNIGHT_MEAN, KNIGHT_BRIGHT_SHARE, "a knight") } else { (FIGURE_MEAN, FIGURE_BRIGHT_SHARE, "a figure that is not a knight") };
    if mean > most {
        out.push(breach("figure-mean", format!("{name}: mean brightness {mean:.2}; {who} is {most} at most")));
    }
    if bright > share {
        out.push(breach("figure-bright", format!("{name}: {:.0}% of it over {BRIGHT}; {who} has {:.0}% at most", bright * 100.0, share * 100.0)));
    }
    out
}

/// How much of its own colour a figure that is not a knight keeps when it
/// is held to the guide ([`grade_figure`]): its chroma, this part of it.
pub const FOE_CHROMA_KEEP: f32 = 0.7;

/// A figure that is not a knight (a monster, a neighbour), held to the
/// guide in place: its colours pulled a little toward their own grey
/// ([`FOE_CHROMA_KEEP`]), then the whole sprite darkened evenly, keeping
/// its drawing, till its mean is under [`FIGURE_MEAN`] and no more than
/// [`FIGURE_BRIGHT_SHARE`] of it is over [`BRIGHT`]. Straight RGBA; its
/// transparent pixels (alpha under 128) are left as they are. What it
/// makes passes [`audit_sprite`] as a figure that is not a knight.
pub fn grade_figure(pixels: &mut [[u8; 4]]) {
    let shown = |p: &[u8; 4]| p[3] >= 128;
    let grey = |c: [f32; 3]| luminance(c);
    let mut colours: Vec<[f32; 3]> = pixels
        .iter()
        .filter(|p| shown(p))
        .map(|p| {
            let c = unit([p[0], p[1], p[2]]);
            let l = grey(c);
            c.map(|v| l + (v - l) * FOE_CHROMA_KEEP)
        })
        .collect();
    if colours.is_empty() {
        return;
    }
    let n = colours.len() as f32;
    let mean = colours.iter().map(|&c| grey(c)).sum::<f32>() / n;
    // A hair under the limits, so rounding to bytes cannot tip it over.
    let mut k = if mean > FIGURE_MEAN * 0.97 { FIGURE_MEAN * 0.97 / mean } else { 1.0 };
    for _ in 0..40 {
        let bright = colours.iter().filter(|&&c| grey(c) * k > BRIGHT * 0.98).count() as f32 / n;
        if bright <= FIGURE_BRIGHT_SHARE * 0.97 {
            break;
        }
        k *= 0.97;
    }
    for c in &mut colours {
        *c = c.map(|v| (v * k).clamp(0.0, 1.0));
    }
    let mut graded = colours.into_iter();
    for p in pixels.iter_mut().filter(|p| shown(p)) {
        let c = graded.next().unwrap_or_default();
        let [r, g, b] = c.map(|v| (v * 255.0).round() as u8);
        *p = [r, g, b, p[3]];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The monolith's ramps (crates/glass/src/monolith.rs), the look the
    /// user approved: each must sit on a ladder.
    const SLATE: [[u8; 3]; 8] = [
        [0x0e, 0x0f, 0x19],
        [0x14, 0x17, 0x27],
        [0x1b, 0x1f, 0x36],
        [0x24, 0x2a, 0x47],
        [0x2f, 0x36, 0x58],
        [0x3e, 0x46, 0x6b],
        [0x54, 0x5c, 0x82],
        [0x77, 0x7f, 0xa2],
    ];
    const MEADOW: [[u8; 3]; 8] = [
        [0x0e, 0x12, 0x10],
        [0x17, 0x1f, 0x1b],
        [0x22, 0x2d, 0x27],
        [0x2f, 0x3c, 0x33],
        [0x40, 0x4f, 0x42],
        [0x55, 0x65, 0x52],
        [0x70, 0x7e, 0x67],
        [0x96, 0x9f, 0x84],
    ];
    const EMBER: [[u8; 3]; 8] = [
        [0x17, 0x10, 0x0e],
        [0x29, 0x19, 0x12],
        [0x41, 0x25, 0x17],
        [0x5d, 0x33, 0x1d],
        [0x7f, 0x45, 0x23],
        [0xa3, 0x5e, 0x2b],
        [0xc6, 0x83, 0x3f],
        [0xe6, 0xb2, 0x66],
    ];

    #[test]
    fn every_kinds_tone_sits_in_its_class_band() {
        for kind in Kind::ALL {
            let class = class_of(kind);
            let (lo, hi) = class.band().tone;
            let t = kind.tone();
            assert!((lo..=hi).contains(&t), "{} ({}) is {t}, outside {lo}..{hi}", kind.name(), class.name());
        }
    }

    #[test]
    fn masses_and_props_never_outshine_the_ground_and_the_top_step_is_kept() {
        let top = |c: Class| c.band().ceiling.unwrap();
        assert!(Class::Mass.band().tone.1 < Class::Ground.band().tone.1);
        assert!(Class::Prop.band().tone.1 <= Class::Mass.band().tone.1);
        assert!(top(Class::Mass) < top(Class::Ground));
        for c in [Class::Ground, Class::Route, Class::Mass, Class::Prop, Class::Opening, Class::Void] {
            assert!(top(c) < 7, "{} may not reach the top step", c.name());
        }
        assert_eq!(top(Class::Interactable), 7);
    }

    #[test]
    fn a_tone_out_of_its_band_is_held_in_it() {
        assert_eq!(Class::Ground.clamp_tone(0.9), 0.40);
        assert_eq!(Class::Mass.clamp_tone(0.01), 0.10);
        assert_eq!(Class::Mass.clamp_tone(f32::NAN), 0.10);
        assert_eq!(Class::Ground.material_ceiling(), 7);
        assert_eq!(Class::Light.material_ceiling(), 0);
        assert_eq!(Class::Ground.ceiling_on(8), Some(6));
        assert_eq!(Class::Ground.ceiling_on(16), Some(13));
    }

    #[test]
    fn the_approved_ramps_sit_on_their_ladders() {
        assert_eq!(ladder_of(&SLATE).name, "dungeon");
        assert_eq!(audit_ramp("slate", &SLATE, &DUNGEON), vec![]);
        assert_eq!(ladder_of(&MEADOW).name, "open");
        assert_eq!(audit_ramp("meadow", &MEADOW, &OPEN), vec![]);
        assert_eq!(audit_warm(&EMBER, &DUNGEON), vec![]);
    }

    #[test]
    fn a_garish_ramp_is_called_out_rule_by_rule() {
        // Bright, saturated, and a rainbow: everything the guide forbids.
        let garish: Vec<[u8; 3]> =
            vec![[0, 0, 0], [40, 0, 0], [80, 40, 0], [120, 120, 0], [0, 160, 0], [0, 160, 200], [80, 80, 255], [255, 120, 255]];
        let rules: Vec<&str> = audit_ramp("garish", &garish, &DUNGEON).iter().map(|b| b.rule).collect();
        for rule in ["ramp-value", "ramp-chroma", "ramp-hue"] {
            assert!(rules.contains(&rule), "{rule} in {rules:?}");
        }
    }

    #[test]
    fn a_brutalist_room_passes_and_a_cluttered_one_is_called_out() {
        let good = "set hall\ncamera room 64 64\nground 1 1 step 64 at 0 0 heights 0 0 0 0 tones 30 30 36 36\n\
                    box stone at 8 0 32 size 16 48 64\nbox stone at 56 0 32 size 16 48 64\nbox door at 32 0 2 size 12 24 4\n\
                    box fire at 32 26 6 size 2 2 2\nlamp at 32 26 8 reach 40 strength 0.9\n";
        let set = crate::set::Set::parse(good).expect("reads");
        assert_eq!(audit_set(&set), vec![]);
        let mut busy = String::from("set clutter\ncamera room 64 64\n");
        for (i, kind) in ["stone", "brick", "plaster", "wood", "metal", "cloth", "bone"].iter().enumerate() {
            busy += &format!("box {kind} at {} 0 8 size 2 2 2\n", 4 + i * 8);
        }
        for i in 0..4 {
            busy += &format!("lamp at {} 4 4 reach 10 strength 1.4\n", i * 10);
        }
        let set = crate::set::Set::parse(&busy).expect("reads");
        let rules: Vec<&str> = audit_set(&set).iter().map(|b| b.rule).collect();
        for rule in ["set-kinds", "set-detail", "set-lamps", "lamp-strength"] {
            assert!(rules.contains(&rule), "{rule} in {rules:?}");
        }
    }

    #[test]
    fn a_glaring_monster_is_held_under_the_knights_and_keeps_its_drawing() {
        // A ghost-white body with a saturated green eye and a dark outline.
        let mut ghost = vec![[0xe8, 0xec, 0xf4, 255]; 300];
        ghost[..20].fill([0x20, 0xe0, 0x40, 255]);
        ghost[20..60].fill([0x18, 0x18, 0x20, 255]);
        ghost[60..80].fill([0, 0, 0, 0]);
        let rules: Vec<&str> = audit_sprite("ghost", &ghost, false).iter().map(|b| b.rule).collect();
        assert!(rules.contains(&"figure-mean"));
        let before = ghost.clone();
        grade_figure(&mut ghost);
        assert_eq!(audit_sprite("ghost", &ghost, false), vec![]);
        assert_eq!(ghost[60..80], before[60..80], "what is transparent stays so");
        assert!(luma([ghost[100][0], ghost[100][1], ghost[100][2]]) > luma([ghost[30][0], ghost[30][1], ghost[30][2]]), "light stays lighter than the outline");
        assert!(chroma([ghost[0][0], ghost[0][1], ghost[0][2]]) < chroma([before[0][0], before[0][1], before[0][2]]), "its colour is quieter");
        // A sprite already within the guide only loses a little colour.
        let mut calm = vec![[0x50, 0x40, 0x38, 255]; 50];
        grade_figure(&mut calm);
        assert!((luma([calm[0][0], calm[0][1], calm[0][2]]) - luma([0x50, 0x40, 0x38])).abs() < 0.01);
    }

    #[test]
    fn a_dark_frame_with_one_lamp_passes_and_a_bright_one_does_not() {
        let mut frame = vec![[0x24, 0x2a, 0x47]; 10_000];
        frame[..300].fill([0x0e, 0x0f, 0x19]);
        frame[9_950..].fill([0xed, 0xad, 0x54]);
        let (_, breaches) = audit_frame(&frame);
        assert_eq!(breaches, vec![]);
        let loud = vec![[0xe0, 0x90, 0x30]; 10_000];
        let rules: Vec<&str> = audit_frame(&loud).1.iter().map(|b| b.rule).collect();
        for rule in ["middle", "bulk", "bright-share", "vivid-share"] {
            assert!(rules.contains(&rule), "{rule} in {rules:?}");
        }
    }
}
