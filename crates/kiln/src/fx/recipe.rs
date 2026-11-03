//! The fx recipe, version 0: an effect as JSON, layers of shapes and
//! emitters with their timing and their ramps (docs/KILN-FX.md).
//!
//! Units are the game's, so one recipe renders at every grain: places and
//! sizes in game pixels (x right, y down), times in frames, angles in
//! degrees (0 right, 90 down). A layer's place is offset from the recipe's
//! pivot. Unknown fields are refused, and so is a field a layer's type does
//! not take; every error names its path.
//!
//! The vocabulary follows the VFX cartridge's v1 where the two mean the
//! same (`type`, `t0` `t1`, `x` `y`, `r0` `r1`, `th`, `angle`, `squash`,
//! `gain`, `speed`, `spin`, `count`, `size`, `life`, `gravity`), so a recipe
//! carries from one to the other (`fx::from_cartridge_v1`).

use super::time::{Beats, Ease, Time};
use super::pen::Mode;
use serde::{Deserialize, Serialize};

/// The recipe version this reads.
pub const VERSION: u32 = 0;
/// The most layers a recipe has.
pub const MOST_LAYERS: usize = 32;
/// The most motes one emitter throws over its life.
pub const MOST_MOTES: u32 = 512;

/// An effect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// The version, 0.
    pub fx: u32,
    pub name: String,
    /// Its size in game pixels.
    pub canvas: [u32; 2],
    /// The point it is played at (a hand, an impact, a figure's feet), in
    /// game pixels from the canvas's top left; its centre if not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<u32>,
    /// How many frames it runs; the beats' total, else its last layer's end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<u32>,
    /// Whether it plays round and round: an aura, a burning brazier.
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    pub looping: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beats: Option<Beats>,
    /// A one-pixel rim of the realm's darkest ink round everything drawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<bool>,
    pub layers: Vec<Layer>,
}

/// What a layer draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// An emitter: motes of a form, thrown, slowed, pulled and let die.
    Particles,
    /// Smoke puffs: an emitter whose form is smoke, drawn under.
    Puffs,
    /// A fireball's body: a rough hot disc swelling and cooling into holes.
    Burst,
    /// A ring opening, its thickness falling away.
    Ring,
    /// A shockwave: a ring with a dark wake on the floor inside it.
    Shock,
    /// Tongues of flame licking up from a base.
    Flame,
    /// A beam from the layer's place to its end.
    Beam,
    /// A pillar of light from on high down to the layer's place.
    Pillar,
    /// A blade's smear: a crescent swept round, thick at its leading edge.
    Slash,
    /// A glowing ball, pulsing; it travels when given an end.
    Orb,
    /// A flash: a hard bright disc that cools at once.
    Flash,
    /// A comet: a hot head with a tapering tail behind it.
    Comet,
    /// A swirl turning in on itself.
    Vortex,
    /// Rays shooting out with an overshoot and drawing back.
    Rays,
    /// A rune circle drawing itself on: rings, a star, runes round its band.
    Glyph,
    /// Lightning: a jagged bolt with forks, re-struck every few frames.
    Bolt,
    /// Cracks running out over the ground.
    Cracks,
    /// Spikes shooting up out of the ground and breaking.
    Spikes,
    /// A spiky star of light.
    Star,
    /// A thrust's streak with speed lines beside it.
    Streaks,
    /// A shield's bubble popping into being.
    Bubble,
    /// Numbers rising: `+12`, `-3`, `!`.
    Text,
    /// A funnel of wind, striped and swaying.
    Tornado,
}

/// What an emitter's motes are drawn as.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Form {
    #[default]
    Spark,
    Ember,
    Mote,
    Droplet,
    Shard,
    Flake,
    Bubble,
    Note,
    Star,
    Petal,
    Coin,
    Heart,
    Worm,
    Smoke,
    Fly,
}

/// How an emitter's motes move beyond being thrown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Motion {
    #[default]
    Straight,
    /// Side to side across their line, `wobble` pixels.
    Wobble,
    /// Turning about where they were thrown, `spin` degrees a frame.
    Spiral,
    /// A slower turn.
    Curl,
    /// A sharp side to side.
    Zigzag,
    /// Hopping as they go.
    Bounce,
    /// Floating up.
    Rise,
    /// Dropping.
    Fall,
    /// Drawn in from their spawn to the layer's place: a wind-up's gather.
    Gather,
    /// Going round the layer's place.
    Orbit,
}

/// Where an emitter's motes are born about its place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Spawn {
    #[default]
    Point,
    /// Along a level line `area[0]` long.
    Line,
    /// On an ellipse of radii `area`, thrown out from its middle.
    Ring,
    /// Anywhere in an ellipse of radii `area`.
    Disc,
    /// Anywhere in a box `area` wide and high: a figure's box.
    Box,
}

/// Whether a layer is drawn in front of a figure or behind it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Z {
    #[default]
    Front,
    Back,
}

/// A number, or a pair: a range (`[low, high]`, each mote its own share of
/// it) or a sweep (`[from, to]` over a mote's or a layer's life).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Range {
    One(f32),
    Two([f32; 2]),
}

impl Range {
    pub fn pair(self) -> (f32, f32) {
        match self {
            Range::One(v) => (v, v),
            Range::Two([a, b]) => (a, b),
        }
    }
}

/// One layer. Every field but `type` may be left out and takes its type's
/// own default; a field its type does not take is refused.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    #[serde(rename = "type")]
    pub kind: Kind,
    /// Particles: the motes' form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<Form>,
    /// A named ramp (`fire`, `frost`, ...); or `inks`, its keys dark to light.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inks: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<Z>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    /// When it starts and ends: a frame or a beat (`"action"`, `"end-2"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t0: Option<Time>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t1: Option<Time>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<Ease>,
    /// Its place, offset from the pivot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f32>,
    /// Its end, offset from the pivot: where a bolt or beam reaches, or
    /// where an orb, comet or emitter travels to over its life.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x1: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y1: Option<f32>,
    /// How high a travel arcs, in game pixels (a lob).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lift: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r0: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r1: Option<f32>,
    /// Thickness: a ring's, a beam's half width, a slash's leading edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub th: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f32>,
    /// A slash's sweep in degrees; the sign is its way round.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squash: Option<f32>,
    /// Its heat, times.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain: Option<f32>,
    /// Its heat over its life, or a mote's over the mote's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heat: Option<Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Particles: motes born a frame from `t0` to `t1`, after the burst of `count`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f32>,
    /// Particles: a mote's life in frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub life: Option<Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<Range>,
    /// Particles: how wide they are thrown, in degrees about `angle`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spread: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn: Option<Spawn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<Motion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wobble: Option<f32>,
    /// Degrees a frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spin: Option<f32>,
    /// A bolt's or crack's jaggedness, in game pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jag: Option<f32>,
    /// Frames a bolt holds its shape before it is struck again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flicker: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// The fields every type takes.
const COMMON: &[&str] = &["type", "ramp", "inks", "z", "mode", "t0", "t1", "ease", "x", "y", "gain", "heat", "seed"];

impl Kind {
    /// The fields a type takes beyond the common ones.
    pub fn takes(self) -> &'static [&'static str] {
        const EMIT: &[&str] = &[
            "form", "count", "rate", "life", "speed", "angle", "spread", "gravity", "drag", "size", "spawn", "area", "motion",
            "wobble", "spin", "squash", "x1", "y1", "lift",
        ];
        match self {
            Kind::Particles => EMIT,
            Kind::Puffs => &EMIT[1..],
            Kind::Burst => &["r0", "r1", "squash"],
            Kind::Ring | Kind::Shock => &["r0", "r1", "th", "squash"],
            Kind::Flame => &["count", "r1", "size", "speed"],
            Kind::Beam => &["x1", "y1", "th", "speed"],
            Kind::Pillar => &["r1", "th", "speed"],
            Kind::Slash => &["r1", "th", "angle", "arc", "squash"],
            Kind::Orb | Kind::Flash => &["r0", "r1", "speed", "x1", "y1", "lift"],
            Kind::Comet => &["r1", "size", "angle", "x1", "y1", "lift"],
            Kind::Vortex => &["r1", "th", "count", "spin", "squash"],
            Kind::Rays => &["count", "r0", "r1", "th", "angle", "spin", "squash"],
            Kind::Glyph => &["r1", "count", "angle", "spin", "squash"],
            Kind::Bolt => &["x1", "y1", "count", "jag", "flicker", "th"],
            Kind::Cracks => &["count", "r1", "th", "angle", "spread", "squash", "jag"],
            Kind::Spikes => &["count", "r1", "th", "size", "angle"],
            Kind::Star => &["count", "r1", "th", "spin"],
            Kind::Streaks => &["count", "r1", "th", "angle"],
            Kind::Bubble => &["r1", "th", "squash"],
            Kind::Text => &["text", "size", "speed"],
            Kind::Tornado => &["r1", "size", "spin"],
        }
    }

    pub fn word(self) -> String {
        serde_json::to_value(self).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
    }
}

impl Recipe {
    /// A recipe from its JSON, every field checked.
    pub fn parse(json: &str) -> Result<Recipe, String> {
        let r: Recipe = serde_json::from_str(json).map_err(|e| e.to_string())?;
        r.check()?;
        Ok(r)
    }

    /// Within every bound, every layer taking only what its type takes.
    pub fn check(&self) -> Result<(), String> {
        if self.fx != VERSION {
            return Err(format!("fx: version {} (this kiln reads {VERSION})", self.fx));
        }
        let within = |path: &str, v: f32, lo: f32, hi: f32| -> Result<(), String> {
            if v.is_finite() && (lo..=hi).contains(&v) { Ok(()) } else { Err(format!("{path}: {v} is not in {lo}..={hi}")) }
        };
        within("canvas[0]", self.canvas[0] as f32, 1.0, 256.0)?;
        within("canvas[1]", self.canvas[1] as f32, 1.0, 256.0)?;
        if let Some(f) = self.fps {
            within("fps", f as f32, 1.0, 60.0)?;
        }
        if let Some(f) = self.frames {
            within("frames", f as f32, 1.0, 240.0)?;
        }
        if self.layers.is_empty() || self.layers.len() > MOST_LAYERS {
            return Err(format!("layers: {} of them (1 to {MOST_LAYERS})", self.layers.len()));
        }
        for (i, l) in self.layers.iter().enumerate() {
            let at = |f: &str| format!("layers[{i}].{f}");
            let fields = serde_json::to_value(l).map_err(|e| e.to_string())?;
            for key in fields.as_object().into_iter().flat_map(|o| o.keys()) {
                if !COMMON.contains(&key.as_str()) && !l.kind.takes().contains(&key.as_str()) {
                    return Err(format!("{}: a {} takes no {key} (it takes {})", at(key), l.kind.word(), l.kind.takes().join(" ")));
                }
            }
            if l.ramp.is_some() && l.inks.is_some() {
                return Err(format!("{}: a ramp or inks, not both", at("inks")));
            }
            for (f, v, lo, hi) in [
                ("x", l.x, -512.0, 512.0),
                ("y", l.y, -512.0, 512.0),
                ("x1", l.x1, -512.0, 512.0),
                ("y1", l.y1, -512.0, 512.0),
                ("lift", l.lift, -256.0, 256.0),
                ("r0", l.r0, 0.0, 256.0),
                ("r1", l.r1, 0.0, 256.0),
                ("th", l.th, 0.0, 64.0),
                ("angle", l.angle, -720.0, 720.0),
                ("arc", l.arc, -720.0, 720.0),
                ("squash", l.squash, 0.05, 4.0),
                ("gain", l.gain, 0.0, 4.0),
                ("rate", l.rate, 0.0, 64.0),
                ("spread", l.spread, 0.0, 360.0),
                ("gravity", l.gravity, -8.0, 8.0),
                ("drag", l.drag, 0.0, 4.0),
                ("wobble", l.wobble, 0.0, 32.0),
                ("spin", l.spin, -360.0, 360.0),
                ("jag", l.jag, 0.0, 64.0),
                ("flicker", l.flicker, 0.0, 240.0),
            ] {
                if let Some(v) = v {
                    within(&at(f), v, lo, hi)?;
                }
            }
            for (f, v, lo, hi) in [("heat", l.heat, 0.0, 4.0), ("life", l.life, 0.5, 240.0), ("speed", l.speed, 0.0, 64.0), ("size", l.size, 0.0, 64.0)] {
                if let Some(r) = v {
                    let (a, b) = r.pair();
                    within(&at(f), a, lo, hi)?;
                    within(&at(f), b, lo, hi)?;
                }
            }
            if let Some(c) = l.count {
                within(&at("count"), c as f32, 0.0, MOST_MOTES as f32)?;
            }
            if let Some(a) = l.area {
                within(&at("area"), a[0], 0.0, 256.0)?;
                within(&at("area"), a[1], 0.0, 256.0)?;
            }
            if let Some(t) = &l.text
                && (t.is_empty() || t.chars().count() > 8 || !t.chars().all(|c| super::shapes::FONT_CHARS.contains(c)))
            {
                return Err(format!("{}: 1 to 8 of {:?}", at("text"), super::shapes::FONT_CHARS));
            }
        }
        Ok(())
    }

    /// The recipe as JSON, one layer a line.
    pub fn to_json(&self) -> String {
        let mut head = self.clone();
        head.layers.clear();
        let head = serde_json::to_string(&head).unwrap_or_default();
        let layers: Vec<String> = self.layers.iter().map(|l| format!("    {}", serde_json::to_string(l).unwrap_or_default())).collect();
        let head = head.trim_end_matches("\"layers\":[]}").to_string();
        format!("{head}\"layers\": [\n{}\n  ]\n}}\n", layers.join(",\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RING: &str = r#"{ "fx": 0, "name": "t", "canvas": [16, 16], "layers": [ { "type": "ring", "ramp": "frost", "r1": 6 } ] }"#;

    #[test]
    fn a_recipe_reads_and_writes_back() {
        let r = Recipe::parse(RING).unwrap();
        assert_eq!(r.layers[0].kind, Kind::Ring);
        assert_eq!(Recipe::parse(&r.to_json()).unwrap(), r);
    }

    #[test]
    fn what_a_recipe_cannot_say_is_refused_by_its_path() {
        let unknown = RING.replace("\"r1\": 6", "\"radius\": 6");
        assert!(Recipe::parse(&unknown).unwrap_err().contains("radius"));
        let wrong = RING.replace("\"r1\": 6", "\"arc\": 90");
        assert!(Recipe::parse(&wrong).unwrap_err().contains("layers[0].arc: a ring takes no arc"));
        let big = RING.replace("\"r1\": 6", "\"r1\": 900");
        assert!(Recipe::parse(&big).unwrap_err().contains("layers[0].r1"));
        assert!(Recipe::parse(&RING.replace("\"fx\": 0", "\"fx\": 3")).is_err());
    }
}
