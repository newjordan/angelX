//! Recipe v0: a figure as JSON, the asset kiln draws from at every grain.
//!
//! docs/KILN-RECIPE.md is the reference a person or a model writes from.
//! Here is the schema ([`Source`] and the types under it, read with unknown
//! fields refused) and the checks a recipe passes before it is drawn
//! ([`Recipe::parse`]): every number finite and in its bounds, every name
//! it uses defined, each shape given what it needs and nothing it doesn't.
//! A refusal names every fault with its path (`parts[3].r[1]: ...`), so the
//! author can mend them all at once.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;

use crate::grain::Grain;
use crate::light::Finish;
use crate::palette::{Ink, Palette, Ramp};
use crate::rig::Ease;

/// The recipe version this kiln reads.
pub const VERSION: u32 = 0;
/// The biggest canvas side, in game pixels (1024 px at grain 8).
pub const MAX_CANVAS: u32 = 128;
/// The most parts a recipe has.
pub const MAX_PARTS: usize = 256;

/// A recipe as written: the JSON's own shape.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The schema's version: 0.
    pub kiln: u32,
    /// What it is: lower-case letters, digits, `-` and `_`.
    pub name: String,
    /// Its size in game pixels, `[w, h]`.
    pub canvas: [u32; 2],
    /// Where it stands, in model space: the floor contact under its feet.
    pub pivot: [f32; 2],
    /// Toward the one light, `[x, y, z]` (y down, z toward the viewer).
    #[serde(default = "realm_light")]
    pub light: [f32; 3],
    #[serde(default)]
    pub outline: Outline,
    /// At grain 1, how far lit tones are lifted toward the light end of
    /// their ramps (0..1): at 16 px a tile the shadow is the edge line's.
    #[serde(default = "lift")]
    pub lift: f32,
    /// Inks and finishes by name.
    pub materials: BTreeMap<String, Material>,
    /// Volumes and flat shapes, drawn in order (a later part wins a tie).
    pub parts: Vec<Part>,
    /// Marks drawn for one grain, by grain (`"1"`, `"2"`, `"4"`, `"8"`).
    #[serde(default)]
    pub decals: BTreeMap<String, Vec<Decal>>,
    #[serde(default)]
    pub rig: Rig,
    /// Motion by name (`idle`, `walk`, `slash`, ...).
    #[serde(default)]
    pub clips: BTreeMap<String, Clip>,
    /// Words for people; kiln draws nothing from them.
    #[serde(default)]
    pub notes: String,
}

/// The realm's light: from the upper left and a little toward the viewer.
pub fn realm_light() -> [f32; 3] {
    [-0.55, -0.9, 0.7]
}

/// The silhouette's line and the inner lines.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outline {
    /// The line's ink where it faces away from the light.
    #[serde(default = "outline_ink")]
    pub ink: String,
    /// How many steps down its neighbour's ramp the line is where it faces
    /// the light.
    #[serde(default = "two")]
    pub selout: u8,
    /// The depth break, in game pixels, at which one part meeting another
    /// draws an inner line.
    #[serde(default = "inner_break")]
    pub inner: f32,
}

impl Default for Outline {
    fn default() -> Outline {
        Outline { ink: outline_ink(), selout: 2, inner: inner_break() }
    }
}

fn lift() -> f32 {
    0.35
}
fn outline_ink() -> String {
    "k".into()
}
fn two() -> u8 {
    2
}
fn inner_break() -> f32 {
    0.75
}
fn yes() -> bool {
    true
}

/// A material: its tones and how light falls on it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    /// Ink keys, dark to light (`"sxSuUvVW"`).
    pub ramp: String,
    pub finish: Finish,
    /// Moves every tone along the ramp (-1..1; negative darker).
    #[serde(default)]
    pub shade: f32,
}

/// The kinds of part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeKind {
    /// `at`, `r: [rx, ry, rz]`, `turn`.
    Ellipsoid,
    /// `from`, `to`, `r: [r]` or `[r_from, r_to]`.
    Capsule,
    /// `at`, `size: [w, h, d]`, `round`, `turn`.
    Box,
    /// `points: [[x, y], ...]`, `z`, `thick`, `bevel`, `lean`, `curve`.
    Slab,
    /// `points: [[x, y, z], ...]`, `width: [w]` or `[w_first, w_last]`.
    Stroke,
    /// `points: [[x, y], ...]`, `z`, and `ink` or `mat` with `tone`: unlit.
    Flat,
}

impl ShapeKind {
    /// The shape's name with its article: "an ellipsoid", "a box".
    pub fn a(self) -> &'static str {
        match self {
            ShapeKind::Ellipsoid => "an ellipsoid",
            ShapeKind::Capsule => "a capsule",
            ShapeKind::Box => "a box",
            ShapeKind::Slab => "a slab",
            ShapeKind::Stroke => "a stroke",
            ShapeKind::Flat => "a flat",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ShapeKind::Ellipsoid => "ellipsoid",
            ShapeKind::Capsule => "capsule",
            ShapeKind::Box => "box",
            ShapeKind::Slab => "slab",
            ShapeKind::Stroke => "stroke",
            ShapeKind::Flat => "flat",
        }
    }
}

/// One part: a shape, its material and what it hangs on.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub id: String,
    pub shape: ShapeKind,
    #[serde(default)]
    pub mat: Option<String>,
    /// A flat's ink, by key, in place of a material.
    #[serde(default)]
    pub ink: Option<String>,
    /// A flat's place on its material's ramp (0 darkest, 1 lightest).
    #[serde(default)]
    pub tone: Option<f32>,
    #[serde(default)]
    pub at: Option<[f32; 3]>,
    #[serde(default)]
    pub r: Option<Vec<f32>>,
    #[serde(default)]
    pub from: Option<[f32; 3]>,
    #[serde(default)]
    pub to: Option<[f32; 3]>,
    #[serde(default)]
    pub size: Option<[f32; 3]>,
    #[serde(default)]
    pub round: Option<f32>,
    #[serde(default)]
    pub points: Option<Vec<Vec<f32>>>,
    #[serde(default)]
    pub width: Option<Vec<f32>>,
    #[serde(default)]
    pub z: Option<f32>,
    #[serde(default)]
    pub thick: Option<f32>,
    #[serde(default)]
    pub bevel: Option<f32>,
    #[serde(default)]
    pub lean: Option<[f32; 2]>,
    #[serde(default)]
    pub curve: Option<[f32; 2]>,
    /// Degrees about z (ellipsoid, box), clockwise on screen.
    #[serde(default)]
    pub turn: f32,
    #[serde(default)]
    pub cut: Option<Cut>,
    /// The bone it hangs on; the root of the picture when none.
    #[serde(default)]
    pub bone: Option<String>,
    /// The socket it is held in: its points are then about the socket,
    /// pointing up (-y).
    #[serde(default)]
    pub socket: Option<String>,
    /// A surface mark: drawn only where this other part shows, at its depth.
    #[serde(default)]
    pub on: Option<String>,
    /// Moves its tones along the ramp (-1..1).
    #[serde(default)]
    pub shade: f32,
    /// Added to its depth, to settle ties.
    #[serde(default)]
    pub bias: f32,
    /// Kept at grain 1 when thin: it wins a pixel with a quarter of the vote.
    #[serde(default)]
    pub keep: bool,
    /// The grains it is drawn at; all when none.
    #[serde(default)]
    pub grains: Option<Vec<u8>>,
    /// Drawn only in a clip that shows it.
    #[serde(default)]
    pub hidden: bool,
    /// Whether it draws inner lines where it covers another part.
    #[serde(default = "yes")]
    pub line: bool,
}

/// A part's keep box, in its own space: what lies outside is not drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cut {
    #[serde(default)]
    pub x: Option<[f32; 2]>,
    #[serde(default)]
    pub y: Option<[f32; 2]>,
}

/// A mark drawn for one grain, in that grain's pixels.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decal {
    /// Its top-left, in the grain's pixels, at rest.
    pub at: [i32; 2],
    /// Ink keys, `.` clear.
    pub rows: Vec<String>,
    /// The bone it rides; it moves with it, never turns.
    #[serde(default)]
    pub bone: Option<String>,
    /// Drawn only where this part shows.
    #[serde(default)]
    pub on: Option<String>,
}

/// Bones and sockets.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rig {
    /// Parents before children.
    #[serde(default)]
    pub bones: Vec<Bone>,
    /// Named points on bones (`grip`, `off_hand`, `head`, `back`, `feet`,
    /// `tip`).
    #[serde(default)]
    pub sockets: BTreeMap<String, Socket>,
}

/// A bone: the joint it turns about.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bone {
    pub id: String,
    #[serde(default)]
    pub parent: Option<String>,
    /// Its joint, in model space, at rest.
    pub at: [f32; 2],
}

/// A socket: a point on a bone, and which way what it holds points.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Socket {
    #[serde(default)]
    pub bone: Option<String>,
    pub at: [f32; 2],
    /// Degrees from up (-y), clockwise, that held parts point.
    #[serde(default)]
    pub turn: f32,
}

/// A clip: poses keyed in time.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub frames: u32,
    #[serde(default = "eight")]
    pub fps: f32,
    /// Whether it loops (the last frame runs back into the first key).
    #[serde(default = "yes", rename = "loop")]
    pub looped: bool,
    /// Hidden parts it shows.
    #[serde(default)]
    pub show: Vec<String>,
    /// Parts it hides.
    #[serde(default)]
    pub hide: Vec<String>,
    pub keys: Vec<Key>,
}

fn eight() -> f32 {
    8.0
}

/// A pose at a time.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    /// Its frame (fractions allowed).
    pub t: f32,
    /// How the motion runs from this key to the next.
    #[serde(default)]
    pub ease: Ease,
    /// Each bone's turn and shift from rest; a bone not named is at rest.
    #[serde(default)]
    pub pose: BTreeMap<String, BonePose>,
}

/// A bone's change from rest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BonePose {
    /// Degrees about its joint, clockwise on screen.
    #[serde(default)]
    pub turn: f32,
    #[serde(default)]
    pub dx: f32,
    #[serde(default)]
    pub dy: f32,
    #[serde(default)]
    pub dz: f32,
}

/// What a part is drawn in.
#[derive(Clone, Debug)]
pub enum Paint {
    /// Lit, in a material.
    Lit { mat: usize },
    /// Unlit, one ink.
    Ink(Ink),
}

/// A part with its names resolved.
#[derive(Clone, Debug)]
pub struct PartRef {
    pub paint: Paint,
    pub bone: Option<usize>,
    pub socket: Option<String>,
    pub on: Option<usize>,
}

/// A decal with its inks read and its names resolved.
#[derive(Clone, Debug)]
pub struct DecalRef {
    pub at: [i32; 2],
    pub w: u32,
    pub h: u32,
    pub inks: Vec<Ink>,
    pub bone: Option<usize>,
    pub on: Option<usize>,
}

/// A checked recipe, ready to draw.
#[derive(Clone, Debug)]
pub struct Recipe {
    pub src: Source,
    /// Toward the light, of unit length.
    pub light: [f32; 3],
    pub outline: Ink,
    /// Materials in name order: name, ramp, finish, shade.
    pub materials: Vec<(String, Ramp, Finish, f32)>,
    /// `src.parts`' names resolved, by index.
    pub parts: Vec<PartRef>,
    /// Each bone's parent, by index.
    pub parents: Vec<Option<usize>>,
    /// Decals by grain.
    pub decals: BTreeMap<u8, Vec<DecalRef>>,
}

/// Every fault a recipe has, each with its path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeError(pub Vec<String>);

impl fmt::Display for RecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join("\n"))
    }
}

impl std::error::Error for RecipeError {}

impl Recipe {
    /// Read and check a recipe.
    pub fn parse(json: &str) -> Result<Recipe, RecipeError> {
        if json.len() > 4 << 20 {
            return Err(RecipeError(vec!["the recipe is over 4 MiB".into()]));
        }
        let de = &mut serde_json::Deserializer::from_str(json);
        let src: Source = serde_path_to_error::deserialize(de).map_err(|e| {
            let path = e.path().to_string();
            let path = if path == "." { String::new() } else { format!("{path}: ") };
            RecipeError(vec![format!("{path}{}", e.into_inner())])
        })?;
        Recipe::check(src)
    }

    /// Check a recipe already read.
    pub fn check(src: Source) -> Result<Recipe, RecipeError> {
        Checker::new().run(src)
    }

    /// A bone's index by id.
    pub fn bone(&self, id: &str) -> Option<usize> {
        self.src.rig.bones.iter().position(|b| b.id == id)
    }

    /// A part's index by id.
    pub fn part(&self, id: &str) -> Option<usize> {
        self.src.parts.iter().position(|p| p.id == id)
    }

    /// The canvas in pixels at `grain`.
    pub fn size_at(&self, grain: Grain) -> (u32, u32) {
        (self.src.canvas[0] * grain.0 as u32, self.src.canvas[1] * grain.0 as u32)
    }

    /// A clip by name.
    pub fn clip(&self, name: &str) -> Option<&Clip> {
        self.src.clips.get(name)
    }
}

struct Checker {
    faults: Vec<String>,
}

impl Checker {
    fn new() -> Checker {
        Checker { faults: Vec::new() }
    }

    fn fault(&mut self, path: impl fmt::Display, what: impl fmt::Display) {
        if self.faults.len() < 64 {
            self.faults.push(format!("{path}: {what}"));
        }
    }

    fn finite(&mut self, path: &str, v: f32, lo: f32, hi: f32) -> bool {
        if !v.is_finite() || v < lo || v > hi {
            self.fault(path, format!("{v} is not within {lo}..={hi}"));
            return false;
        }
        true
    }

    fn positive(&mut self, path: &str, v: f32, hi: f32) -> bool {
        if !v.is_finite() || v <= 0.0 || v > hi {
            self.fault(path, format!("{v} is not above 0 and at most {hi}"));
            return false;
        }
        true
    }

    fn name(&mut self, path: &str, s: &str) {
        let ok = !s.is_empty()
            && s.len() <= 32
            && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
        if !ok {
            self.fault(path, format!("{s:?} is not 1..=32 of a-z, 0-9, '_', '-'"));
        }
    }

    fn ink(&mut self, path: &str, key: &str) -> Option<Ink> {
        let mut chars = key.chars();
        match (chars.next(), chars.next()) {
            (Some(k), None) => match Palette::realm().ink(k) {
                Some(i) if !i.is_clear() => Some(i),
                _ => {
                    self.fault(path, format!("{k:?} is not one of the realm's inks"));
                    None
                }
            },
            _ => {
                self.fault(path, format!("{key:?} is not one ink key"));
                None
            }
        }
    }

    fn run(mut self, src: Source) -> Result<Recipe, RecipeError> {
        let palette = Palette::realm();
        if src.kiln != VERSION {
            self.fault("kiln", format!("version {} is not {VERSION}", src.kiln));
        }
        if src.name.len() > 64 || src.name.is_empty() {
            self.fault("name", "is 1..=64 characters");
        } else {
            let name = src.name.clone();
            self.name("name", &name);
        }
        let [w, h] = src.canvas;
        for (i, side) in [w, h].into_iter().enumerate() {
            if !(1..=MAX_CANVAS).contains(&side) {
                self.fault(format!("canvas[{i}]"), format!("{side} is not within 1..={MAX_CANVAS}"));
            }
        }
        let (fw, fh) = (w as f32, h as f32);
        self.finite("pivot[0]", src.pivot[0], 0.0, fw);
        self.finite("pivot[1]", src.pivot[1], 0.0, fh);
        let mut light = src.light;
        for (i, v) in light.iter().enumerate() {
            self.finite(&format!("light[{i}]"), *v, -1e3, 1e3);
        }
        let len = (light[0] * light[0] + light[1] * light[1] + light[2] * light[2]).sqrt();
        if len < 1e-3 || !len.is_finite() {
            self.fault("light", "has no direction");
            light = realm_light();
        }
        let len = (light[0] * light[0] + light[1] * light[1] + light[2] * light[2]).sqrt();
        let light = light.map(|v| v / len);
        let outline = self.ink("outline.ink", &src.outline.ink).unwrap_or(Ink(1));
        if src.outline.selout > 4 {
            self.fault("outline.selout", format!("{} is not within 0..=4", src.outline.selout));
        }
        self.finite("outline.inner", src.outline.inner, 0.0, 64.0);
        self.finite("lift", src.lift, 0.0, 1.0);

        // Materials.
        if src.materials.len() > 32 {
            self.fault("materials", format!("{} materials: at most 32", src.materials.len()));
        }
        let mut materials = Vec::new();
        for (name, m) in &src.materials {
            let path = format!("materials.{name}");
            self.name(&path, name);
            let ramp = match Ramp::parse(&m.ramp, palette) {
                Ok(r) if r.0.len() <= 16 => r,
                Ok(_) => {
                    self.fault(format!("{path}.ramp"), "has more than 16 inks");
                    Ramp(vec![Ink(1)])
                }
                Err(e) => {
                    self.fault(format!("{path}.ramp"), e);
                    Ramp(vec![Ink(1)])
                }
            };
            self.finite(&format!("{path}.shade"), m.shade, -1.0, 1.0);
            materials.push((name.clone(), ramp, m.finish, m.shade));
        }
        let material = |name: &str| materials.iter().position(|(n, ..)| n == name);

        // Bones and sockets.
        let mut bone_ids: Vec<&str> = Vec::new();
        let mut parents = Vec::new();
        if src.rig.bones.len() > 64 {
            self.fault("rig.bones", "has more than 64 bones");
        }
        for (i, b) in src.rig.bones.iter().enumerate() {
            let path = format!("rig.bones[{i}]");
            self.name(&format!("{path}.id"), &b.id);
            if bone_ids.contains(&b.id.as_str()) {
                self.fault(format!("{path}.id"), format!("{:?} is taken", b.id));
            }
            self.finite(&format!("{path}.at[0]"), b.at[0], -fw, 2.0 * fw);
            self.finite(&format!("{path}.at[1]"), b.at[1], -fh, 2.0 * fh);
            let parent = match &b.parent {
                None => None,
                Some(p) => match bone_ids.iter().position(|id| id == p) {
                    Some(at) => Some(at),
                    None => {
                        self.fault(format!("{path}.parent"), format!("{p:?} is not a bone before it"));
                        None
                    }
                },
            };
            parents.push(parent);
            bone_ids.push(&b.id);
        }
        let bone = |id: &str| bone_ids.iter().position(|b| *b == id);
        for (name, s) in &src.rig.sockets {
            let path = format!("rig.sockets.{name}");
            self.name(&path, name);
            if let Some(b) = &s.bone
                && bone(b).is_none()
            {
                self.fault(format!("{path}.bone"), format!("{b:?} is not a bone"));
            }
            self.finite(&format!("{path}.at[0]"), s.at[0], -fw, 2.0 * fw);
            self.finite(&format!("{path}.at[1]"), s.at[1], -fh, 2.0 * fh);
            self.finite(&format!("{path}.turn"), s.turn, -720.0, 720.0);
        }

        // Parts.
        if src.parts.is_empty() || src.parts.len() > MAX_PARTS {
            self.fault("parts", format!("{} parts: 1..={MAX_PARTS}", src.parts.len()));
        }
        let mut part_ids: Vec<&str> = Vec::new();
        let mut parts = Vec::new();
        let reach = 2.0 * fw.max(fh);
        for (i, p) in src.parts.iter().enumerate() {
            let path = format!("parts[{i}]");
            self.name(&format!("{path}.id"), &p.id);
            if part_ids.contains(&p.id.as_str()) {
                self.fault(format!("{path}.id"), format!("{:?} is taken", p.id));
            }
            let field = |f: &str| format!("{path}.{f}");
            let kind = p.shape;
            // Which fields each shape takes.
            let takes: &[&str] = match kind {
                ShapeKind::Ellipsoid => &["at", "r", "turn"],
                ShapeKind::Capsule => &["from", "to", "r"],
                ShapeKind::Box => &["at", "size", "round", "turn"],
                ShapeKind::Slab => &["points", "z", "thick", "bevel", "lean", "curve"],
                ShapeKind::Stroke => &["points", "width"],
                ShapeKind::Flat => &["points", "z", "ink", "tone"],
            };
            let given = [
                ("at", p.at.is_some()),
                ("r", p.r.is_some()),
                ("from", p.from.is_some()),
                ("to", p.to.is_some()),
                ("size", p.size.is_some()),
                ("round", p.round.is_some()),
                ("points", p.points.is_some()),
                ("width", p.width.is_some()),
                ("z", p.z.is_some()),
                ("thick", p.thick.is_some()),
                ("bevel", p.bevel.is_some()),
                ("lean", p.lean.is_some()),
                ("curve", p.curve.is_some()),
                ("ink", p.ink.is_some()),
                ("tone", p.tone.is_some()),
                ("turn", p.turn != 0.0),
            ];
            for (f, is) in given {
                if is && !takes.contains(&f) {
                    self.fault(field(f), format!("{} takes no {f}", kind.a()));
                }
            }
            let need = |s: &mut Checker, f: &str, is: bool| {
                if !is {
                    s.fault(field(f), format!("{} needs {f}", kind.a()));
                }
            };
            let point3 = |s: &mut Checker, f: &str, v: [f32; 3]| {
                s.finite(&format!("{}[0]", field(f)), v[0], -reach, reach);
                s.finite(&format!("{}[1]", field(f)), v[1], -reach, reach);
                s.finite(&format!("{}[2]", field(f)), v[2], -reach, reach);
            };
            match kind {
                ShapeKind::Ellipsoid => {
                    need(&mut self, "at", p.at.is_some());
                    need(&mut self, "r", p.r.is_some());
                    if let Some(at) = p.at {
                        point3(&mut self, "at", at);
                    }
                    if let Some(r) = &p.r {
                        if r.len() != 3 {
                            self.fault(field("r"), "an ellipsoid's r is [rx, ry, rz]");
                        }
                        for (j, v) in r.iter().enumerate() {
                            self.positive(&format!("{}[{j}]", field("r")), *v, 64.0);
                        }
                    }
                    self.finite(&field("turn"), p.turn, -720.0, 720.0);
                }
                ShapeKind::Capsule => {
                    need(&mut self, "from", p.from.is_some());
                    need(&mut self, "to", p.to.is_some());
                    need(&mut self, "r", p.r.is_some());
                    for (f, v) in [("from", p.from), ("to", p.to)] {
                        if let Some(v) = v {
                            point3(&mut self, f, v);
                        }
                    }
                    if let Some(r) = &p.r {
                        if !(1..=2).contains(&r.len()) {
                            self.fault(field("r"), "a capsule's r is [r] or [r_from, r_to]");
                        }
                        for (j, v) in r.iter().enumerate() {
                            self.positive(&format!("{}[{j}]", field("r")), *v, 64.0);
                        }
                    }
                }
                ShapeKind::Box => {
                    need(&mut self, "at", p.at.is_some());
                    need(&mut self, "size", p.size.is_some());
                    if let Some(at) = p.at {
                        point3(&mut self, "at", at);
                    }
                    if let Some(s) = p.size {
                        for (j, v) in s.iter().enumerate() {
                            self.positive(&format!("{}[{j}]", field("size")), *v, 128.0);
                        }
                        if let Some(r) = p.round {
                            let most = s[0].min(s[1]).min(s[2]) / 2.0;
                            self.finite(&field("round"), r, 0.0, most.max(0.0));
                        }
                    }
                    self.finite(&field("turn"), p.turn, -720.0, 720.0);
                }
                ShapeKind::Slab | ShapeKind::Flat => {
                    need(&mut self, "points", p.points.is_some());
                    if let Some(points) = &p.points {
                        if !(3..=64).contains(&points.len()) {
                            self.fault(field("points"), format!("{} points: 3..=64", points.len()));
                        }
                        for (j, pt) in points.iter().enumerate() {
                            let pp = format!("{}[{j}]", field("points"));
                            if pt.len() != 2 {
                                self.fault(&pp, format!("{} point is [x, y]", kind.a()));
                            }
                            for (k, v) in pt.iter().enumerate() {
                                self.finite(&format!("{pp}[{k}]"), *v, -reach, reach);
                            }
                        }
                    }
                    if let Some(z) = p.z {
                        self.finite(&field("z"), z, -reach, reach);
                    }
                    if kind == ShapeKind::Slab {
                        if let Some(t) = p.thick {
                            self.positive(&field("thick"), t, 64.0);
                        }
                        if let Some(b) = p.bevel {
                            self.finite(&field("bevel"), b, 0.0, 16.0);
                        }
                        for (f, v) in [("lean", p.lean), ("curve", p.curve)] {
                            if let Some([a, b]) = v {
                                self.finite(&format!("{}[0]", field(f)), a, -4.0, 4.0);
                                self.finite(&format!("{}[1]", field(f)), b, -4.0, 4.0);
                            }
                        }
                    } else if let Some(t) = p.tone {
                        self.finite(&field("tone"), t, 0.0, 1.0);
                    }
                }
                ShapeKind::Stroke => {
                    need(&mut self, "points", p.points.is_some());
                    need(&mut self, "width", p.width.is_some());
                    if let Some(points) = &p.points {
                        if !(2..=64).contains(&points.len()) {
                            self.fault(field("points"), format!("{} points: 2..=64", points.len()));
                        }
                        for (j, pt) in points.iter().enumerate() {
                            let pp = format!("{}[{j}]", field("points"));
                            if pt.len() != 3 {
                                self.fault(&pp, "a stroke point is [x, y, z]");
                            }
                            for (k, v) in pt.iter().enumerate() {
                                self.finite(&format!("{pp}[{k}]"), *v, -reach, reach);
                            }
                        }
                    }
                    if let Some(wd) = &p.width {
                        if !(1..=2).contains(&wd.len()) {
                            self.fault(field("width"), "a stroke's width is [w] or [w_first, w_last]");
                        }
                        for (j, v) in wd.iter().enumerate() {
                            self.positive(&format!("{}[{j}]", field("width")), *v, 64.0);
                        }
                    }
                }
            }
            // Paint.
            let paint = match (kind, &p.ink, &p.mat) {
                (ShapeKind::Flat, Some(_), Some(_)) => {
                    self.fault(field("ink"), "a flat takes ink or mat, not both");
                    Paint::Ink(Ink(1))
                }
                (ShapeKind::Flat, Some(key), None) => Paint::Ink(self.ink(&field("ink"), key).unwrap_or(Ink(1))),
                (_, _, Some(m)) => match material(m) {
                    Some(mat) => Paint::Lit { mat },
                    None => {
                        self.fault(field("mat"), format!("{m:?} is not a material"));
                        Paint::Ink(Ink(1))
                    }
                },
                (_, _, None) => {
                    self.fault(field("mat"), "a part needs a material (a flat may take an ink)");
                    Paint::Ink(Ink(1))
                }
            };
            // What it hangs on.
            let bone_at = match &p.bone {
                Some(b) => match bone(b) {
                    Some(at) => Some(at),
                    None => {
                        self.fault(field("bone"), format!("{b:?} is not a bone"));
                        None
                    }
                },
                None => None,
            };
            if let Some(s) = &p.socket {
                if p.bone.is_some() {
                    self.fault(field("socket"), "a part hangs on a bone or a socket, not both");
                }
                if !src.rig.sockets.contains_key(s) {
                    self.fault(field("socket"), format!("{s:?} is not a socket"));
                }
            }
            let on = match &p.on {
                Some(o) => match part_ids.iter().position(|id| id == o) {
                    Some(at) => Some(at),
                    None => {
                        self.fault(field("on"), format!("{o:?} is not a part before it"));
                        None
                    }
                },
                None => None,
            };
            if let Some(cut) = p.cut {
                for (f, v) in [("x", cut.x), ("y", cut.y)] {
                    if let Some([a, b]) = v
                        && (!a.is_finite() || !b.is_finite() || a >= b)
                    {
                        self.fault(format!("{path}.cut.{f}"), format!("[{a}, {b}] is not [low, high]"));
                    }
                }
            }
            self.finite(&field("shade"), p.shade, -1.0, 1.0);
            self.finite(&field("bias"), p.bias, -64.0, 64.0);
            if let Some(gs) = &p.grains {
                for (j, g) in gs.iter().enumerate() {
                    if Grain::of(*g).is_none() {
                        self.fault(format!("{}[{j}]", field("grains")), format!("{g} is not a grain (1, 2, 4, 8)"));
                    }
                }
            }
            parts.push(PartRef { paint, bone: bone_at, socket: p.socket.clone(), on });
            part_ids.push(&p.id);
        }
        let part = |id: &str| part_ids.iter().position(|p| *p == id);

        // Decals.
        let mut decals = BTreeMap::new();
        for (g, list) in &src.decals {
            let path = format!("decals.{g}");
            let Some(grain) = g.parse::<u8>().ok().and_then(Grain::of) else {
                self.fault(&path, format!("{g:?} is not a grain (\"1\", \"2\", \"4\", \"8\")"));
                continue;
            };
            let mut out = Vec::new();
            for (i, d) in list.iter().enumerate() {
                let dp = format!("{path}[{i}]");
                let h = d.rows.len();
                let w = d.rows.first().map_or(0, |r| r.chars().count());
                if h == 0 || w == 0 || h > 64 || w > 64 {
                    self.fault(format!("{dp}.rows"), "is 1..=64 rows of 1..=64 keys");
                    continue;
                }
                let mut inks = Vec::with_capacity(w * h);
                for (j, row) in d.rows.iter().enumerate() {
                    if row.chars().count() != w {
                        self.fault(format!("{dp}.rows[{j}]"), format!("is not {w} keys wide"));
                    }
                    for k in row.chars().take(w) {
                        match palette.ink(k) {
                            Some(i) => inks.push(i),
                            None => {
                                self.fault(format!("{dp}.rows[{j}]"), format!("{k:?} is not one of the realm's inks"));
                                inks.push(Ink::CLEAR);
                            }
                        }
                    }
                    while inks.len() < (j + 1) * w {
                        inks.push(Ink::CLEAR);
                    }
                }
                let reach = (MAX_CANVAS as i32) * grain.0 as i32;
                for (k, v) in d.at.iter().enumerate() {
                    if !(-reach..=reach).contains(v) {
                        self.fault(format!("{dp}.at[{k}]"), format!("{v} is off the canvas"));
                    }
                }
                let b = d.bone.as_ref().and_then(|b| {
                    let at = bone(b);
                    if at.is_none() {
                        self.fault(format!("{dp}.bone"), format!("{b:?} is not a bone"));
                    }
                    at
                });
                let on = d.on.as_ref().and_then(|o| {
                    let at = part(o);
                    if at.is_none() {
                        self.fault(format!("{dp}.on"), format!("{o:?} is not a part"));
                    }
                    at
                });
                out.push(DecalRef { at: d.at, w: w as u32, h: h as u32, inks, bone: b, on });
            }
            decals.insert(grain.0, out);
        }

        // Clips.
        if src.clips.len() > 32 {
            self.fault("clips", "has more than 32 clips");
        }
        for (name, c) in &src.clips {
            let path = format!("clips.{name}");
            self.name(&path, name);
            if !(1..=128).contains(&c.frames) {
                self.fault(format!("{path}.frames"), format!("{} is not within 1..=128", c.frames));
            }
            self.finite(&format!("{path}.fps"), c.fps, 1.0, 60.0);
            for (f, list) in [("show", &c.show), ("hide", &c.hide)] {
                for (j, id) in list.iter().enumerate() {
                    if part(id).is_none() {
                        self.fault(format!("{path}.{f}[{j}]"), format!("{id:?} is not a part"));
                    }
                }
            }
            if c.keys.is_empty() || c.keys.len() > 128 {
                self.fault(format!("{path}.keys"), "has 1..=128 keys");
            }
            let mut last = f32::NEG_INFINITY;
            let mut seen = BTreeSet::new();
            for (j, k) in c.keys.iter().enumerate() {
                let kp = format!("{path}.keys[{j}]");
                if self.finite(&format!("{kp}.t"), k.t, 0.0, c.frames as f32) && k.t <= last {
                    self.fault(format!("{kp}.t"), format!("{} is not after the key before it ({last})", k.t));
                }
                last = k.t;
                for (b, pose) in &k.pose {
                    if bone(b).is_none() {
                        self.fault(format!("{kp}.pose.{b}"), "is not a bone");
                    }
                    seen.insert(b.clone());
                    self.finite(&format!("{kp}.pose.{b}.turn"), pose.turn, -720.0, 720.0);
                    for (f, v) in [("dx", pose.dx), ("dy", pose.dy), ("dz", pose.dz)] {
                        self.finite(&format!("{kp}.pose.{b}.{f}"), v, -reach, reach);
                    }
                }
            }
        }

        if self.faults.is_empty() {
            Ok(Recipe { src, light, outline, materials, parts, parents, decals })
        } else {
            Err(RecipeError(self.faults))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = r#"{ "kiln": 0, "name": "pip", "canvas": [8, 8], "pivot": [4, 8],
        "materials": { "steel": { "ramp": "sxSuUvVW", "finish": "metal" } },
        "parts": [ { "id": "ball", "shape": "ellipsoid", "at": [4, 4, 0], "r": [3, 3, 3], "mat": "steel" } ] }"#;

    #[test]
    fn a_small_recipe_reads() {
        let r = Recipe::parse(SMALL).unwrap();
        assert_eq!(r.src.name, "pip");
        assert_eq!(r.materials[0].1.keys(Palette::realm()), "sxSuUvVW");
        assert!((r.light.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 1e-5);
        assert_eq!(r.size_at(Grain::HD), (32, 32));
    }

    #[test]
    fn unknown_fields_are_refused_with_their_path() {
        let bad = SMALL.replace("\"r\": [3, 3, 3]", "\"r\": [3, 3, 3], \"radius\": 2");
        let e = Recipe::parse(&bad).unwrap_err();
        assert!(e.0[0].starts_with("parts[0]"), "{e}");
        assert!(e.0[0].contains("radius"), "{e}");
    }

    #[test]
    fn every_fault_is_named_by_its_path() {
        let bad = SMALL
            .replace("\"r\": [3, 3, 3]", "\"r\": [3, 0, 3], \"points\": [[1, 2]]")
            .replace("\"mat\": \"steel\"", "\"mat\": \"gold\", \"bone\": \"head\"")
            .replace("sxSuUvVW", "sxS#");
        let e = Recipe::parse(&bad).unwrap_err();
        let text = e.to_string();
        for want in ["parts[0].r[1]", "parts[0].points: an ellipsoid", "parts[0].mat", "parts[0].bone", "materials.steel.ramp"] {
            assert!(text.contains(want), "{want} not in:\n{text}");
        }
    }
}
