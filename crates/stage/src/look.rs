//! A scene's look (docs/GLASS_CORE.md, "The world's look"): the palette it is
//! seen through, as plain data. A scene with a [`Look`] is drawn at a small
//! internal size, every pixel is moved onto one colour ramp by an ordered
//! dither, a few accent colours are kept as they are, and the distance fades
//! toward the ramp, so a 3D scene reads as pixel art. The reference is Jason
//! Strachan's dithered night shoreline (x.com/jasonstrachan/status/2111047350609453291):
//! real-time 3D at about 540 rows, a 4x4 Bayer dither between two neighbouring
//! colours of one indigo ramp, one warm accent, fog.
//!
//! This file is the description and the reference maths. `angel-gpu`'s
//! `look.wgsl` does the same thing per pixel on the GPU, and the functions
//! here ([`Look::quantize`], [`bayer_threshold`]) are what its tests compare
//! the pixels read back against. Nothing here panics on bad input: a look that
//! cannot be made usable ([`Look::normalized`] is `None`) leaves its scene
//! drawn as it would be without one.
//!
//! How a pixel is placed on the ramp:
//! - **Brightness is the pixel's luminance** (Rec. 709 weights on the colour as
//!   drawn). Each ramp entry has its own luminance, and a pixel is placed
//!   between the two entries whose luminances hold it, in proportion. So a
//!   material coloured like a ramp entry comes out as that entry, whatever
//!   its hue, and a mix between two entries has the brightness the eye will
//!   read from the dither. The other choice, a position along the ramp's own
//!   line in colour space, makes a hue the ramp does not have (a warm sand, a
//!   knight's red plume) land on whatever the line passes nearest, which is
//!   not what its brightness says.
//! - Between the two entries the choice is made by the Bayer threshold at the
//!   pixel, so 50% is a checkerboard and the levels between are the crosses
//!   and the sparser patterns the reference shows.
//! - A pixel of an `accent` material is the nearest accent colour, undithered.

use std::ops::RangeInclusive;

/// The fewest and most entries a ramp has.
pub const RAMP_LEN: RangeInclusive<usize> = 2..=16;

/// The most accent colours a look keeps undithered.
pub const ACCENT_MAX: usize = 4;

/// The fewest and most rows of the internal size.
pub const PIXEL_ROWS: RangeInclusive<u32> = 16..=2160;

/// The widest the internal size may be.
pub const MAX_PIXEL_WIDTH: u32 = 8192;

/// The most lamps a look lights a scene with.
pub const LAMP_MAX: usize = 16;

/// The most zones a look has, besides its own ramp.
pub const ZONE_MAX: usize = 4;

/// The most points a look keeps in sight (`reveal` and `reveals`).
pub const REVEALS_MAX: usize = 4;

/// How the in-between levels are made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dither {
    /// A 4x4 Bayer matrix: 16 levels between two entries (the reference).
    #[default]
    Bayer4,
    /// An 8x8 Bayer matrix: 64 levels, a finer weave.
    Bayer8,
    /// None: each pixel takes the nearer of its two entries, in bands.
    None,
}

impl Dither {
    /// The bits of each coordinate the matrix spends: 2 for 4x4, 3 for 8x8,
    /// 0 for none (what the shader is told).
    pub fn bits(self) -> u32 {
        match self {
            Dither::Bayer4 => 2,
            Dither::Bayer8 => 3,
            Dither::None => 0,
        }
    }
}

/// Distance fog toward one colour of the ramp, by distance from the eye in
/// the scene's units. Nothing is fogged nearer than `start`; past `end`
/// everything is the fog colour; between, it mixes in a straight line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    /// Which ramp entry the distance fades to.
    pub color: usize,
    pub start: f32,
    pub end: f32,
}

/// The light a look's lit surfaces take instead of the scene's fixed key
/// light: one far light (a low sun, the moon) that casts shadows, over a
/// floor of sky light. Lit by it, a face's brightness is its colour's times
/// `ambient + strength * max(0, n . toward)`, darkened to `ambient` alone
/// where something stands between it and the light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    /// The direction toward the light (normalized when used).
    pub toward: [f32; 3],
    /// How much a face square to the light gets, over the ambient.
    pub strength: f32,
    /// What every lit face gets, in shadow or turned away.
    pub ambient: f32,
    /// Where shadows are cast: a square `reach` units across round `focus`,
    /// seen from the light. None: no shadows.
    pub shadow: Option<Shadow>,
    /// How high the sky's light grows: at y 0 a face gets 60% of the
    /// ambient, rising smoothly to all of it this many units up, so a tall
    /// face pales toward its top. 0: the ambient is the same everywhere.
    pub rise: f32,
}

/// The square of the world a [`Sun`] casts shadows in: centred on `focus`
/// (the eye's target, where the play is), `reach` units across. Past it a
/// surface is lit as if nothing stood in the way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub focus: [f32; 3],
    pub reach: f32,
}

impl Sun {
    /// The unit direction toward the light (straight up when it has none).
    pub fn toward_unit(&self) -> [f32; 3] {
        let t = self.toward;
        let length = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
        if length.is_finite() && length > 1e-6 { t.map(|c| c / length) } else { [0.0, 1.0, 0.0] }
    }

    /// World to the shadow map's clip space (WebGPU's: x and y -1..1, z
    /// 0..1, z 0 nearest the light) for a map `texels` on a side: an
    /// orthographic view from the light over the shadow's square, its
    /// middle moved to a whole texel so a moving focus does not make the
    /// shadows' edges crawl. Depth covers `reach` either side of the
    /// focus along the light. `None` without a shadow.
    pub fn shadow_matrix(&self, texels: u32) -> Option<crate::scene::Mat4> {
        let shadow = self.shadow.filter(|s| s.reach > 0.0 && s.reach.is_finite())?;
        let b = self.toward_unit();
        // Right and up across the light's view; up leans on +Y unless the
        // light is overhead, where +Z serves.
        let helper = if b[1].abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
        let cross = |a: [f32; 3], c: [f32; 3]| [a[1] * c[2] - a[2] * c[1], a[2] * c[0] - a[0] * c[2], a[0] * c[1] - a[1] * c[0]];
        let dot = |a: [f32; 3], c: [f32; 3]| a[0] * c[0] + a[1] * c[1] + a[2] * c[2];
        let r = cross(helper, b);
        let rl = dot(r, r).sqrt();
        let r = r.map(|c| c / rl);
        let u = cross(b, r);
        let reach = shadow.reach;
        let texel = reach / texels.max(1) as f32;
        let snap = |v: f32| (v / texel).round() * texel;
        let (cx, cy) = (snap(dot(r, shadow.focus)), snap(dot(u, shadow.focus)));
        let s = 2.0 / reach;
        let depth = 2.0 * reach;
        let near = dot(b, shadow.focus) + reach;
        // Column-major: m[column][row].
        let m = [
            [r[0] * s, u[0] * s, -b[0] / depth, 0.0],
            [r[1] * s, u[1] * s, -b[1] / depth, 0.0],
            [r[2] * s, u[2] * s, -b[2] / depth, 0.0],
            [-cx * s, -cy * s, near / depth, 1.0],
        ];
        crate::scene::mat_finite(&m).then_some(m)
    }
}

/// A lamp: a warm pool of light round a point (a brazier, a lit gate, a
/// window), the way the realm shows activity. Within `reach` units it adds
/// light falling to nothing at the edge, `strength` at the middle, and
/// moves what it lights toward the look's `warm` ramp as far as it lights
/// it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lamp {
    pub at: [f32; 3],
    pub reach: f32,
    pub strength: f32,
}

/// What a look keeps in sight through anything in the way: the eye sees
/// `at` (the knight's chest) through a stippled opening in whatever stands
/// between, `radius` units round the line of sight, fading out at its edge
/// by the dither. Only what rises above the feet (0.7 under `at`) is
/// opened, never the ground in front; and only what is drawn: shadows
/// still fall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reveal {
    pub at: [f32; 3],
    pub radius: f32,
}

/// A place with its own ramp: within `radius` units of the box `half`
/// either side of `at` (x and z, on the ground; a box of no size is a
/// circle) a pixel is seen through `ramp` instead of the look's own, and
/// over the next `feather` units it passes smoothly from one to the other,
/// entry by entry, so walking from a place to the next shifts the whole
/// palette's hue without a seam. A zone's ramp has as many entries as the
/// look's. Zones apply in order, each over what came before.
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub ramp: Vec<[u8; 3]>,
    pub at: [f32; 2],
    pub half: [f32; 2],
    pub radius: f32,
    pub feather: f32,
}

impl Zone {
    /// How far a ground point is outside the zone's box (0 inside it).
    pub fn distance(&self, x: f32, z: f32) -> f32 {
        let dx = ((x - self.at[0]).abs() - self.half[0]).max(0.0);
        let dz = ((z - self.at[1]).abs() - self.half[1]).max(0.0);
        (dx * dx + dz * dz).sqrt()
    }
}

/// A scene's palette and pixel size.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    /// The ramp, dark to light: [`RAMP_LEN`] entries, 8-bit RGB.
    pub ramp: Vec<[u8; 3]>,
    /// Colours kept as they are, never dithered: 0 to [`ACCENT_MAX`].
    pub accent: Vec<[u8; 3]>,
    pub dither: Dither,
    pub fog: Option<Fog>,
    /// The internal size's height, in pixels, within [`PIXEL_ROWS`]; its width
    /// follows the aspect of the target the scene is shown in.
    pub pixel_rows: u32,
    /// The light lit surfaces take, with shadows; none: the fixed key light.
    pub sun: Option<Sun>,
    /// Pools of warm light: 0 to [`LAMP_MAX`].
    pub lamps: Vec<Lamp>,
    /// The ramp lamplight moves a pixel toward, as many entries as `ramp`;
    /// none: lamps only brighten.
    pub warm: Option<Vec<[u8; 3]>>,
    /// Places with their own ramps: 0 to [`ZONE_MAX`].
    pub zones: Vec<Zone>,
    /// The ramp water (`Role::Water`) is seen through wherever it runs, as
    /// many entries as `ramp`; none: its place's, like any surface. A river
    /// is long and winds, which no round zone follows.
    pub water: Option<Vec<[u8; 3]>>,
    /// A point kept in sight through what stands before it.
    pub reveal: Option<Reveal>,
    /// More points kept in sight so (the party's other knights): at most
    /// [`REVEALS_MAX`] with `reveal`.
    pub reveals: Vec<Reveal>,
    /// The scene's clock in seconds, for surfaces that move by themselves
    /// (water, `Role::Water`).
    pub time: f32,
    /// World units in a metre, so water's bands run at the same pace in a
    /// scene in metres (1) and on a stage in pixels (about 11).
    pub metre: f32,
}

/// Why a [`Look`] is not usable as it stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookError {
    /// The ramp has this many entries, outside [`RAMP_LEN`].
    RampLen(usize),
    /// More than [`ACCENT_MAX`] accents.
    AccentLen(usize),
    /// `pixel_rows` outside [`PIXEL_ROWS`].
    PixelRows(u32),
    /// The fog names a ramp entry that is not there.
    FogColor,
    /// The fog's distances are not finite, `start` is negative, or `end` is
    /// not past `start`.
    FogRange,
    /// The sun's direction is zero or not finite, or its light or shadow
    /// reach is negative or not finite.
    Sun,
    /// More than [`LAMP_MAX`] lamps, or one not finite or with no reach.
    Lamps,
    /// The warm or the water ramp is not as long as the ramp.
    Warm,
    /// More than [`ZONE_MAX`] zones, or one whose ramp is not as long as the
    /// look's, or whose place or size is not finite or negative.
    Zones,
}

impl std::fmt::Display for LookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LookError::RampLen(n) => write!(
                f,
                "a ramp has {} to {} entries, this has {n}",
                RAMP_LEN.start(),
                RAMP_LEN.end()
            ),
            LookError::AccentLen(n) => write!(f, "at most {ACCENT_MAX} accents, this has {n}"),
            LookError::PixelRows(n) => write!(
                f,
                "pixel_rows is {} to {}, this is {n}",
                PIXEL_ROWS.start(),
                PIXEL_ROWS.end()
            ),
            LookError::FogColor => write!(f, "the fog colour is not an entry of the ramp"),
            LookError::FogRange => write!(f, "the fog needs finite distances, 0 <= start < end"),
            LookError::Sun => write!(f, "the sun needs a direction and finite, non-negative light and reach"),
            LookError::Lamps => write!(f, "at most {LAMP_MAX} lamps, each finite with a reach"),
            LookError::Warm => write!(f, "the warm and water ramps are as long as the ramp"),
            LookError::Zones => write!(f, "at most {ZONE_MAX} zones, each with a ramp as long as the look's"),
        }
    }
}

impl std::error::Error for LookError {}

/// The luminance of a colour in 0..1 (Rec. 709 weights).
pub fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// 0 at or below `lo`, 1 at or above `hi`, a smooth S between (the
/// shader's `smoothstep`).
pub fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn finite3(v: [f32; 3]) -> bool {
    v.iter().all(|c| c.is_finite())
}

fn sun_usable(sun: &Sun) -> bool {
    let length = sun.toward.iter().map(|c| c * c).sum::<f32>();
    finite3(sun.toward)
        && length > 1e-12
        && sun.strength.is_finite()
        && sun.strength >= 0.0
        && sun.ambient.is_finite()
        && sun.ambient >= 0.0
        && sun.shadow.is_none_or(|s| finite3(s.focus) && s.reach.is_finite() && s.reach >= 0.0)
        && sun.rise.is_finite()
        && sun.rise >= 0.0
}

fn lamp_usable(lamp: &Lamp) -> bool {
    finite3(lamp.at) && lamp.reach.is_finite() && lamp.reach > 0.0 && lamp.strength.is_finite() && lamp.strength >= 0.0
}

fn zone_usable(zone: &Zone, len: usize) -> bool {
    zone.ramp.len() == len
        && zone.at.iter().all(|c| c.is_finite())
        && zone.half.iter().all(|c| c.is_finite() && *c >= 0.0)
        && zone.radius.is_finite()
        && zone.radius >= 0.0
        && zone.feather.is_finite()
        && zone.feather >= 0.0
}

/// An 8-bit colour as 0..1 floats.
pub fn unit_rgb(rgb: [u8; 3]) -> [f32; 3] {
    rgb.map(|c| f32::from(c) / 255.0)
}

/// The Bayer matrix value at `(x, y)` for a matrix of `2^bits` on a side:
/// the coordinates' bits, lowest first, each contributing the pair
/// `(x ^ y, y)` to the value from its top down. That is the matrix
/// `M(2n) = [[4M, 4M+2], [4M+3, 4M+1]]` built from `M(1) = [0]`, and it is
/// what the shader computes.
pub fn bayer_index(x: u32, y: u32, bits: u32) -> u32 {
    let mut v = 0;
    for i in 0..bits {
        let (xb, yb) = ((x >> i) & 1, (y >> i) & 1);
        v = (v << 2) | (((xb ^ yb) << 1) | yb);
    }
    v
}

/// The 4x4 Bayer matrix, `[row][column]`.
pub fn bayer4() -> [[u8; 4]; 4] {
    let mut m = [[0; 4]; 4];
    for (y, row) in m.iter_mut().enumerate() {
        for (x, v) in row.iter_mut().enumerate() {
            *v = bayer_index(x as u32, y as u32, 2) as u8;
        }
    }
    m
}

/// The 8x8 Bayer matrix, `[row][column]`.
pub fn bayer8() -> [[u8; 8]; 8] {
    let mut m = [[0; 8]; 8];
    for (y, row) in m.iter_mut().enumerate() {
        for (x, v) in row.iter_mut().enumerate() {
            *v = bayer_index(x as u32, y as u32, 3) as u8;
        }
    }
    m
}

/// The threshold at pixel `(x, y)`: strictly between 0 and 1, `(m + 0.5) / N`
/// for matrix value `m` of `N` levels, so no level is ever quite 0 or 1 and a
/// 50% level is exactly half the pixels. `Dither::None` is a flat 0.5.
pub fn bayer_threshold(dither: Dither, x: u32, y: u32) -> f32 {
    match dither.bits() {
        0 => 0.5,
        bits => (bayer_index(x, y, bits) as f32 + 0.5) / (1u32 << (2 * bits)) as f32,
    }
}

impl Look {
    /// A look over `ramp`, dark to light: a 4x4 Bayer dither, no accent, no
    /// fog, 540 rows.
    pub fn new(ramp: Vec<[u8; 3]>) -> Self {
        Self {
            ramp,
            accent: Vec::new(),
            dither: Dither::Bayer4,
            fog: None,
            pixel_rows: 540,
            sun: None,
            lamps: Vec::new(),
            warm: None,
            zones: Vec::new(),
            water: None,
            reveal: None,
            reveals: Vec::new(),
            time: 0.0,
            metre: 1.0,
        }
    }

    /// Whether the look lights the scene itself (a sun, lamps or zones): the
    /// scene's pixels are then placed on their ramps as they are drawn, each
    /// with its own place's palette and light, not afterwards.
    pub fn lights(&self) -> bool {
        self.sun.is_some() || !self.lamps.is_empty() || !self.zones.is_empty()
    }

    /// Whether this look can be used as it stands, and the first reason it
    /// cannot. The renderer does not refuse a look that fails this: it uses
    /// [`Look::normalized`].
    pub fn validate(&self) -> Result<(), LookError> {
        if !RAMP_LEN.contains(&self.ramp.len()) {
            return Err(LookError::RampLen(self.ramp.len()));
        }
        if self.accent.len() > ACCENT_MAX {
            return Err(LookError::AccentLen(self.accent.len()));
        }
        if !PIXEL_ROWS.contains(&self.pixel_rows) {
            return Err(LookError::PixelRows(self.pixel_rows));
        }
        if let Some(fog) = &self.fog {
            if fog.color >= self.ramp.len() {
                return Err(LookError::FogColor);
            }
            if !(fog.start.is_finite() && fog.end.is_finite() && fog.start >= 0.0 && fog.end > fog.start) {
                return Err(LookError::FogRange);
            }
        }
        if let Some(sun) = &self.sun
            && !sun_usable(sun)
        {
            return Err(LookError::Sun);
        }
        if self.lamps.len() > LAMP_MAX || !self.lamps.iter().all(lamp_usable) {
            return Err(LookError::Lamps);
        }
        if self.warm.as_ref().is_some_and(|w| w.len() != self.ramp.len())
            || self.water.as_ref().is_some_and(|w| w.len() != self.ramp.len())
        {
            return Err(LookError::Warm);
        }
        if self.zones.len() > ZONE_MAX || !self.zones.iter().all(|z| zone_usable(z, self.ramp.len())) {
            return Err(LookError::Zones);
        }
        Ok(())
    }

    /// The look the renderer uses: this one made usable. A ramp longer than
    /// 16 or accents past 4 are cut; `pixel_rows` is clamped; a fog colour
    /// past the ramp is the last entry; a fog whose distances are not usable
    /// is dropped. `None` when the ramp has fewer than two entries, which
    /// leaves nothing to dither between: the scene is then drawn plainly.
    pub fn normalized(&self) -> Option<Look> {
        if self.ramp.len() < *RAMP_LEN.start() {
            return None;
        }
        let mut look = self.clone();
        look.ramp.truncate(*RAMP_LEN.end());
        look.accent.truncate(ACCENT_MAX);
        look.pixel_rows = look.pixel_rows.clamp(*PIXEL_ROWS.start(), *PIXEL_ROWS.end());
        look.fog = look.fog.and_then(|fog| {
            let usable = fog.start.is_finite() && fog.end.is_finite() && fog.end > fog.start.max(0.0);
            usable.then(|| Fog {
                color: fog.color.min(look.ramp.len() - 1),
                start: fog.start.max(0.0),
                end: fog.end,
            })
        });
        look.sun = look.sun.filter(sun_usable).map(|mut sun| {
            sun.shadow = sun.shadow.filter(|s| s.reach > 0.0);
            sun
        });
        look.lamps.retain(lamp_usable);
        look.lamps.truncate(LAMP_MAX);
        look.warm = look.warm.filter(|w| w.len() == look.ramp.len());
        look.water = look.water.filter(|w| w.len() == look.ramp.len());
        let len = look.ramp.len();
        look.zones.retain(|z| zone_usable(z, len));
        look.zones.truncate(ZONE_MAX);
        let usable = |r: &Reveal| finite3(r.at) && r.radius.is_finite() && r.radius > 0.0;
        look.reveal = look.reveal.filter(usable);
        look.reveals.retain(usable);
        look.reveals.truncate(REVEALS_MAX - 1);
        if !look.time.is_finite() {
            look.time = 0.0;
        }
        if !(look.metre.is_finite() && look.metre > 0.0) {
            look.metre = 1.0;
        }
        Some(look)
    }

    /// The look as text, a statement a line, for a wire (the stage frame's
    /// look block, `STG6`): what [`Look::from_text`] reads back, exactly.
    pub fn to_text(&self) -> String {
        use std::fmt::Write;
        let hex = |ramp: &[[u8; 3]]| ramp.iter().map(|c| format!(" {:02x}{:02x}{:02x}", c[0], c[1], c[2])).collect::<String>();
        let mut t = String::new();
        let _ = writeln!(t, "ramp{}", hex(&self.ramp));
        if !self.accent.is_empty() {
            let _ = writeln!(t, "accent{}", hex(&self.accent));
        }
        let _ = writeln!(t, "dither {}", self.dither.bits());
        if let Some(f) = self.fog {
            let _ = writeln!(t, "fog {} {} {}", f.color, f.start, f.end);
        }
        let _ = writeln!(t, "rows {}", self.pixel_rows);
        if let Some(sun) = self.sun {
            let [x, y, z] = sun.toward;
            let _ = write!(t, "sun {x} {y} {z} {} {} {}", sun.strength, sun.ambient, sun.rise);
            if let Some(sh) = sun.shadow {
                let [fx, fy, fz] = sh.focus;
                let _ = write!(t, " shadow {fx} {fy} {fz} {}", sh.reach);
            }
            t.push('\n');
        }
        for l in &self.lamps {
            let _ = writeln!(t, "lamp {} {} {} {} {}", l.at[0], l.at[1], l.at[2], l.reach, l.strength);
        }
        if let Some(w) = &self.warm {
            let _ = writeln!(t, "warm{}", hex(w));
        }
        if let Some(w) = &self.water {
            let _ = writeln!(t, "water{}", hex(w));
        }
        for z in &self.zones {
            let _ = writeln!(t, "zone {} {} {} {} {} {}{}", z.at[0], z.at[1], z.half[0], z.half[1], z.radius, z.feather, hex(&z.ramp));
        }
        if let Some(r) = self.reveal {
            let _ = writeln!(t, "reveal {} {} {} {}", r.at[0], r.at[1], r.at[2], r.radius);
        }
        for r in &self.reveals {
            let _ = writeln!(t, "also {} {} {} {}", r.at[0], r.at[1], r.at[2], r.radius);
        }
        let _ = writeln!(t, "time {}", self.time);
        let _ = writeln!(t, "metre {}", self.metre);
        t
    }

    /// A look read from [`Look::to_text`]'s text; `None` for text it does
    /// not read (a frame then draws its world without a look).
    pub fn from_text(text: &str) -> Option<Look> {
        let colours = |words: &[&str]| -> Option<Vec<[u8; 3]>> {
            words
                .iter()
                .map(|w| {
                    let v = u32::from_str_radix(w, 16).ok().filter(|_| w.len() == 6)?;
                    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
                })
                .collect()
        };
        let nums = |words: &[&str]| -> Option<Vec<f32>> { words.iter().map(|w| w.parse::<f32>().ok()).collect() };
        let mut look = Look::new(Vec::new());
        for line in text.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            let Some((&head, rest)) = words.split_first() else {
                continue;
            };
            match head {
                "ramp" => look.ramp = colours(rest)?,
                "accent" => look.accent = colours(rest)?,
                "dither" => {
                    look.dither = match *rest.first()? {
                        "2" => Dither::Bayer4,
                        "3" => Dither::Bayer8,
                        "0" => Dither::None,
                        _ => return None,
                    }
                }
                "fog" => {
                    let n = nums(rest)?;
                    let [c, start, end] = n[..] else { return None };
                    look.fog = Some(Fog { color: c as usize, start, end });
                }
                "rows" => look.pixel_rows = rest.first()?.parse().ok()?,
                "sun" => {
                    let (head, shadow) = match rest.iter().position(|w| *w == "shadow") {
                        Some(k) => (&rest[..k], Some(&rest[k + 1..])),
                        None => (rest, None),
                    };
                    let n = nums(head)?;
                    let [x, y, z, strength, ambient, rise] = n[..] else { return None };
                    let shadow = match shadow {
                        Some(w) => {
                            let n = nums(w)?;
                            let [fx, fy, fz, reach] = n[..] else { return None };
                            Some(Shadow { focus: [fx, fy, fz], reach })
                        }
                        None => None,
                    };
                    look.sun = Some(Sun { toward: [x, y, z], strength, ambient, shadow, rise });
                }
                "lamp" => {
                    let n = nums(rest)?;
                    let [x, y, z, reach, strength] = n[..] else { return None };
                    look.lamps.push(Lamp { at: [x, y, z], reach, strength });
                }
                "warm" => look.warm = Some(colours(rest)?),
                "water" => look.water = Some(colours(rest)?),
                "zone" => {
                    if rest.len() < 6 {
                        return None;
                    }
                    let n = nums(&rest[..6])?;
                    look.zones.push(Zone { ramp: colours(&rest[6..])?, at: [n[0], n[1]], half: [n[2], n[3]], radius: n[4], feather: n[5] });
                }
                "reveal" | "also" => {
                    let n = nums(rest)?;
                    let [x, y, z, radius] = n[..] else { return None };
                    let r = Reveal { at: [x, y, z], radius };
                    if head == "reveal" { look.reveal = Some(r) } else { look.reveals.push(r) }
                }
                "time" => look.time = rest.first()?.parse().ok()?,
                "metre" => look.metre = rest.first()?.parse().ok()?,
                _ => return None,
            }
        }
        Some(look)
    }

    /// How far a ground point `(x, z)` is in each zone, 0 to 1 (1 inside its
    /// radius, falling smoothly to 0 across its feather), in the zones' order.
    pub fn zone_weights(&self, x: f32, z: f32) -> Vec<f32> {
        self.zones
            .iter()
            .map(|zone| 1.0 - smoothstep(zone.radius, zone.radius + zone.feather.max(1e-6), zone.distance(x, z)))
            .collect()
    }

    /// The ramp seen at ground point `(x, z)`: the look's own, each zone
    /// mixed over it in turn by its weight there, entry by entry (as 0..1
    /// colours). The shader does the same per pixel.
    pub fn ramp_at(&self, x: f32, z: f32) -> Vec<[f32; 3]> {
        let mut ramp: Vec<[f32; 3]> = self.ramp.iter().map(|&c| unit_rgb(c)).collect();
        for (zone, w) in self.zones.iter().zip(self.zone_weights(x, z)) {
            for (entry, &c) in ramp.iter_mut().zip(&zone.ramp) {
                let c = unit_rgb(c);
                for k in 0..3 {
                    entry[k] += (c[k] - entry[k]) * w;
                }
            }
        }
        ramp
    }

    /// The internal size for a scene shown `shown` pixels big: `pixel_rows`
    /// (clamped) tall, as wide as the shown aspect says (at least 1, at most
    /// [`MAX_PIXEL_WIDTH`]). `(0, 0)` when the shown size is empty. Asking
    /// again of an internal size gives the same size.
    pub fn internal_size(&self, shown: (u32, u32)) -> (u32, u32) {
        if shown.0 == 0 || shown.1 == 0 {
            return (0, 0);
        }
        let rows = self.pixel_rows.clamp(*PIXEL_ROWS.start(), *PIXEL_ROWS.end());
        let width = (f64::from(rows) * f64::from(shown.0) / f64::from(shown.1)).round();
        (width.clamp(1.0, f64::from(MAX_PIXEL_WIDTH)) as u32, rows)
    }

    /// The luminance of each ramp entry, made never to fall (an entry darker
    /// than the one before it is held at that one's level), so a pixel always
    /// has two neighbours to be placed between.
    pub fn ramp_levels(&self) -> Vec<f32> {
        let mut top = 0.0f32;
        self.ramp
            .iter()
            .map(|&c| {
                top = top.max(luminance(unit_rgb(c)));
                top
            })
            .collect()
    }

    /// The fog colour, an entry of the ramp as 0..1 floats.
    pub fn fog_rgb(&self) -> Option<[f32; 3]> {
        let fog = self.fog?;
        self.ramp.get(fog.color).copied().map(unit_rgb)
    }

    /// The ramp entry a pixel of luminance `luma` takes when the threshold at
    /// its place is `threshold`: below the first entry's level the first, at
    /// or above the last's the last, otherwise the upper of the two entries
    /// that hold it when its place between them is past the threshold, the
    /// lower when it is not.
    pub fn pick(&self, luma: f32, threshold: f32) -> usize {
        let levels = self.ramp_levels();
        let n = levels.len();
        if n == 0 {
            return 0;
        }
        if luma.is_nan() || luma <= levels[0] {
            return 0;
        }
        if luma >= levels[n - 1] {
            return n - 1;
        }
        // The last entry at or under the pixel; the next is over it.
        let mut k = 0;
        for (i, &level) in levels.iter().enumerate() {
            if level <= luma {
                k = i;
            }
        }
        let t = (luma - levels[k]) / (levels[k + 1] - levels[k]);
        if t > threshold { k + 1 } else { k }
    }

    /// The nearest accent to `rgb` (squared distance in RGB, the first of
    /// equals), if there are any.
    pub fn nearest_accent(&self, rgb: [f32; 3]) -> Option<[u8; 3]> {
        let mut best: Option<([u8; 3], f32)> = None;
        for &a in &self.accent {
            let a_f = unit_rgb(a);
            let d: f32 = (0..3).map(|k| (a_f[k] - rgb[k]).powi(2)).sum();
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((a, d));
            }
        }
        best.map(|(a, _)| a)
    }

    /// The reference quantizer, which `look.wgsl` mirrors: the colour `rgb`
    /// (0..1, as drawn and fogged) at internal pixel `(x, y)`. An `accent`
    /// pixel is its nearest accent, undithered (a look with no accents has
    /// none to give: it is quantized as any pixel). Any other pixel is placed
    /// on the ramp by its luminance and the Bayer threshold at `(x, y)`.
    pub fn quantize(&self, rgb: [f32; 3], accent: bool, x: u32, y: u32) -> [u8; 3] {
        if accent && let Some(color) = self.nearest_accent(rgb) {
            return color;
        }
        let threshold = bayer_threshold(self.dither, x, y);
        self.ramp[self.pick(luminance(rgb), threshold)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The measured ramp of the reference, and its yellow.
    fn blues() -> Look {
        let mut look = Look::new(vec![
            [0x1e, 0x24, 0x31],
            [0x2d, 0x35, 0x55],
            [0x44, 0x4e, 0x6f],
            [0x50, 0x59, 0x96],
            [0x5c, 0x66, 0x9c],
            [0x6c, 0x77, 0xae],
            [0x8a, 0x93, 0xc7],
        ]);
        look.accent = vec![[0xf0, 0xe0, 0x88]];
        look
    }

    /// The colour exactly between ramp entries `k` and `k + 1`.
    fn between(look: &Look, k: usize, t: f32) -> [f32; 3] {
        let (a, b) = (unit_rgb(look.ramp[k]), unit_rgb(look.ramp[k + 1]));
        [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
    }

    #[test]
    fn the_reference_look_is_valid() {
        let mut look = blues();
        look.fog = Some(Fog { color: 0, start: 10.0, end: 60.0 });
        assert_eq!(look.validate(), Ok(()));
        assert_eq!(look.normalized(), Some(look));
    }

    #[test]
    fn a_look_that_cannot_be_used_says_why() {
        let ok = blues();
        let mut short = ok.clone();
        short.ramp.truncate(1);
        assert_eq!(short.validate(), Err(LookError::RampLen(1)));
        let mut long = ok.clone();
        long.ramp = vec![[1, 2, 3]; 17];
        assert_eq!(long.validate(), Err(LookError::RampLen(17)));
        let mut accents = ok.clone();
        accents.accent = vec![[255, 255, 0]; 5];
        assert_eq!(accents.validate(), Err(LookError::AccentLen(5)));
        let mut rows = ok.clone();
        rows.pixel_rows = 8;
        assert_eq!(rows.validate(), Err(LookError::PixelRows(8)));
        rows.pixel_rows = 2161;
        assert_eq!(rows.validate(), Err(LookError::PixelRows(2161)));
        for (fog, why) in [
            (Fog { color: 7, start: 1.0, end: 2.0 }, LookError::FogColor),
            (Fog { color: 0, start: 2.0, end: 2.0 }, LookError::FogRange),
            (Fog { color: 0, start: -1.0, end: 2.0 }, LookError::FogRange),
            (Fog { color: 0, start: f32::NAN, end: 2.0 }, LookError::FogRange),
            (Fog { color: 0, start: 1.0, end: f32::INFINITY }, LookError::FogRange),
        ] {
            let mut look = ok.clone();
            look.fog = Some(fog);
            assert_eq!(look.validate(), Err(why), "{fog:?}");
        }
        // A reason is a sentence.
        assert!(LookError::RampLen(1).to_string().contains("2 to 16"));
    }

    #[test]
    fn a_look_is_made_usable_by_clamping() {
        let mut look = blues();
        look.ramp = (0..40).map(|i| [i as u8 * 6, 0, 0]).collect();
        look.accent = vec![[255, 255, 0]; 9];
        look.pixel_rows = 5;
        look.fog = Some(Fog { color: 99, start: -3.0, end: 40.0 });
        let n = look.normalized().expect("usable");
        assert_eq!((n.ramp.len(), n.accent.len(), n.pixel_rows), (16, 4, 16));
        // The fog colour is the last entry; a negative start is zero.
        assert_eq!(n.fog, Some(Fog { color: 15, start: 0.0, end: 40.0 }));
        assert_eq!(n.validate(), Ok(()));

        look.pixel_rows = u32::MAX;
        assert_eq!(look.normalized().unwrap().pixel_rows, 2160);
        // A fog that cannot be made sense of is dropped, the look kept.
        look.fog = Some(Fog { color: 0, start: 9.0, end: 3.0 });
        assert_eq!(look.normalized().unwrap().fog, None);
        look.fog = Some(Fog { color: 0, start: f32::NAN, end: 3.0 });
        assert_eq!(look.normalized().unwrap().fog, None);
        // A ramp with nothing to dither between is no look.
        look.ramp = vec![[9, 9, 9]];
        assert_eq!(look.normalized(), None);
        look.ramp.clear();
        assert_eq!(look.normalized(), None);
    }

    #[test]
    fn the_internal_size_follows_the_aspect_and_clamps() {
        let look = blues();
        assert_eq!(look.internal_size((1600, 900)), (960, 540));
        assert_eq!(look.internal_size((900, 1600)), (304, 540));
        assert_eq!(look.internal_size((2160, 2174)), (537, 540));
        assert_eq!(look.internal_size((0, 900)), (0, 0));
        assert_eq!(look.internal_size((900, 0)), (0, 0));
        // Rows clamp; width is at least 1 and at most the cap.
        let mut tall = blues();
        tall.pixel_rows = 100_000;
        assert_eq!(tall.internal_size((100, 100)), (2160, 2160));
        tall.pixel_rows = 1;
        assert_eq!(tall.internal_size((100, 100)).1, 16);
        assert_eq!(look.internal_size((1, 100_000)).0, 1);
        assert_eq!(look.internal_size((100_000, 1)).0, MAX_PIXEL_WIDTH);
        // Asking of an internal size gives it back.
        for shown in [(1600, 900), (901, 1333), (64, 64), (4000, 3)] {
            let size = look.internal_size(shown);
            assert_eq!(look.internal_size(size), size, "{shown:?}");
        }
    }

    #[test]
    fn the_bayer_matrices_are_the_standard_ones() {
        assert_eq!(
            bayer4(),
            [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
        );
        let m8 = bayer8();
        // The standard 8x8, built the recursive way: M(2n) = [[4M, 4M+2], [4M+3, 4M+1]].
        let m4 = bayer4();
        for y in 0..8 {
            for x in 0..8 {
                let quad = [[0, 2], [3, 1]][y / 4][x / 4];
                assert_eq!(m8[y][x], 4 * m4[y % 4][x % 4] + quad, "({x},{y})");
            }
        }
        assert_eq!(m8[0], [0, 32, 8, 40, 2, 34, 10, 42]);
        // Each holds every level once.
        let mut seen: Vec<u8> = m8.iter().flatten().copied().collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..64).collect::<Vec<u8>>());
        let mut seen: Vec<u8> = m4.iter().flatten().copied().collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..16).collect::<Vec<u8>>());
    }

    #[test]
    fn thresholds_sit_strictly_inside_and_repeat() {
        for dither in [Dither::Bayer4, Dither::Bayer8, Dither::None] {
            for y in 0..16 {
                for x in 0..16 {
                    let t = bayer_threshold(dither, x, y);
                    assert!(t > 0.0 && t < 1.0, "{dither:?} ({x},{y}) {t}");
                }
            }
        }
        assert_eq!(bayer_threshold(Dither::Bayer4, 1, 2), bayer_threshold(Dither::Bayer4, 5, 6));
        assert_eq!(bayer_threshold(Dither::Bayer4, 0, 0), 0.5 / 16.0);
        assert_eq!(bayer_threshold(Dither::Bayer4, 1, 0), 8.5 / 16.0);
        assert_eq!(bayer_threshold(Dither::Bayer8, 1, 0), 32.5 / 64.0);
        assert_eq!(bayer_threshold(Dither::None, 3, 9), 0.5);
    }

    #[test]
    fn a_gradient_comes_out_only_in_ramp_colours_and_never_gets_darker() {
        let look = blues();
        let mut last_level = 0.0f32;
        let levels = look.ramp_levels();
        // Left to right, black to white; rows differ only in the dither.
        for x in 0..256u32 {
            let v = x as f32 / 255.0;
            for y in 0..8u32 {
                let out = look.quantize([v, v, v], false, x, y);
                assert!(look.ramp.contains(&out), "({x},{y}) {out:?}");
            }
        }
        // The average over a tile follows the input: monotone in brightness,
        // and within the ramp's own range.
        for x in (0..256u32).step_by(4) {
            let v = x as f32 / 255.0;
            let mut sum = 0.0;
            for dy in 0..4 {
                for dx in 0..4 {
                    sum += luminance(unit_rgb(look.quantize([v, v, v], false, dx, dy)));
                }
            }
            let mean = sum / 16.0;
            assert!(mean + 1e-4 >= last_level, "x {x}: {mean} after {last_level}");
            last_level = mean;
            let luma = luminance([v, v, v]);
            if luma > levels[0] && luma < levels[6] {
                // Where the ramp covers it, the dither matches the input's
                // brightness to within the tile's 1/16 of a step.
                let step = levels.windows(2).map(|w| w[1] - w[0]).fold(0.0f32, f32::max);
                assert!((mean - luma).abs() <= step / 16.0 + 1e-4, "x {x}: {mean} vs {luma}");
            }
        }
        // Above the top and below the bottom: the ends.
        assert_eq!(look.quantize([1.0; 3], false, 3, 3), look.ramp[6]);
        assert_eq!(look.quantize([0.0; 3], false, 3, 3), look.ramp[0]);
    }

    #[test]
    fn the_halfway_level_is_an_exact_checkerboard() {
        for dither in [Dither::Bayer4, Dither::Bayer8] {
            let mut look = blues();
            look.dither = dither;
            for k in 0..6 {
                let mid = between(&look, k, 0.5);
                for y in 0..16u32 {
                    for x in 0..16u32 {
                        let out = look.quantize(mid, false, x, y);
                        // The upper entry on one colour of the board, the
                        // lower on the other.
                        let upper = (x + y) % 2 == 0;
                        let want = if upper { look.ramp[k + 1] } else { look.ramp[k] };
                        assert_eq!(out, want, "{dither:?} entries {k}/{}, ({x},{y})", k + 1);
                    }
                }
            }
        }
    }

    #[test]
    fn each_level_covers_its_share_of_a_tile() {
        // At k/16 of the way between two entries a 4x4 tile has k upper pixels;
        // at k/64 an 8x8 tile does.
        for (dither, side, levels) in [(Dither::Bayer4, 4u32, 16u32), (Dither::Bayer8, 8, 64)] {
            let mut look = blues();
            look.dither = dither;
            for k in 0..=levels {
                let rgb = between(&look, 2, k as f32 / levels as f32);
                let mut upper = 0;
                for y in 0..side {
                    for x in 0..side {
                        let out = look.quantize(rgb, false, x, y);
                        assert!(out == look.ramp[2] || out == look.ramp[3]);
                        upper += u32::from(out == look.ramp[3]);
                    }
                }
                // k = 0 is all lower and k = levels all upper, except where
                // float rounding lands exactly on a threshold: stay within one.
                assert!(upper.abs_diff(k) <= 1, "{dither:?} k {k}: {upper}");
            }
        }
        // The quarter and three-quarter levels of a 4x4 are 4 and 12 pixels.
        let look = blues();
        let count = |t: f32| {
            let rgb = between(&look, 4, t);
            (0..16u32)
                .filter(|i| look.quantize(rgb, false, i % 4, i / 4) == look.ramp[5])
                .count()
        };
        assert_eq!((count(0.26), count(0.76)), (4, 12));
    }

    #[test]
    fn without_a_dither_each_pixel_takes_the_nearer_entry() {
        let mut look = blues();
        look.dither = Dither::None;
        assert_eq!(look.quantize(between(&look, 3, 0.4), false, 0, 0), look.ramp[3]);
        assert_eq!(look.quantize(between(&look, 3, 0.6), false, 5, 7), look.ramp[4]);
        // The same at every pixel: no pattern.
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(look.quantize(between(&look, 3, 0.4), false, x, y), look.ramp[3]);
            }
        }
    }

    #[test]
    fn a_colour_equal_to_a_ramp_entry_is_that_entry_at_every_pixel() {
        let look = blues();
        for (i, &entry) in look.ramp.iter().enumerate() {
            for y in 0..4 {
                for x in 0..4 {
                    let out = look.quantize(unit_rgb(entry), false, x, y);
                    // Its own level, within the float error of 8-bit colours:
                    // that entry, or never anything further than a neighbour.
                    assert!(out == entry || look.ramp.iter().position(|c| *c == out).unwrap().abs_diff(i) <= 1);
                }
            }
        }
        // Whatever the hue: brightness decides.
        let warm = [0.8, 0.5, 0.2];
        let luma = luminance(warm);
        let out = luminance(unit_rgb(blues().quantize(warm, false, 0, 0)));
        assert!(out > 0.0 && luma > 0.0);
    }

    #[test]
    fn an_accent_stays_exact_and_is_never_dithered() {
        let mut look = blues();
        look.accent = vec![[0xf0, 0xe0, 0x88], [0xc8, 0x90, 0x60]];
        for y in 0..8 {
            for x in 0..8 {
                // Exactly an accent: itself.
                assert_eq!(look.quantize(unit_rgb([0xf0, 0xe0, 0x88]), true, x, y), [0xf0, 0xe0, 0x88]);
                // Near one: the nearer.
                assert_eq!(look.quantize(unit_rgb([0xf8, 0xe8, 0x70]), true, x, y), [0xf0, 0xe0, 0x88]);
                assert_eq!(look.quantize(unit_rgb([0xb0, 0x80, 0x50]), true, x, y), [0xc8, 0x90, 0x60]);
                // A bright blue is still an accent when told it is one.
                let out = look.quantize([0.3, 0.3, 0.9], true, x, y);
                assert!(look.accent.contains(&out));
            }
        }
        // Not told, the same colour is on the ramp.
        let plain = look.quantize(unit_rgb([0xf0, 0xe0, 0x88]), false, 0, 0);
        assert!(look.ramp.contains(&plain));
        // A look with no accent has none to give.
        look.accent.clear();
        assert!(look.ramp.contains(&look.quantize([1.0, 1.0, 0.0], true, 1, 1)));
    }

    #[test]
    fn a_ramp_that_dips_is_held_level_and_still_places_pixels() {
        let mut look = Look::new(vec![[100, 100, 100], [20, 20, 20], [200, 200, 200]]);
        let levels = look.ramp_levels();
        assert!(levels.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(levels[0], levels[1]);
        look.dither = Dither::None;
        for v in 0..=255u8 {
            let out = look.quantize(unit_rgb([v, v, v]), false, 0, 0);
            assert!(look.ramp.contains(&out));
        }
        // Non-finite colours do not panic.
        let _ = look.quantize([f32::NAN; 3], false, 0, 0);
        let _ = look.quantize([f32::INFINITY; 3], true, 0, 0);
    }

    fn sun() -> Sun {
        Sun { toward: [-0.6, 0.5, 0.4], strength: 1.0, ambient: 0.3, shadow: Some(Shadow { focus: [10.0, 2.0, -5.0], reach: 80.0 }), rise: 0.0 }
    }

    fn reds() -> Vec<[u8; 3]> {
        (0..7).map(|k| [40 + k * 30, 10 + k * 10, 10 + k * 8]).collect()
    }

    #[test]
    fn a_look_with_a_sun_lamps_or_zones_lights_its_scene() {
        let mut look = blues();
        assert!(!look.lights());
        look.sun = Some(sun());
        assert!(look.lights());
        assert_eq!(look.validate(), Ok(()));
        let mut lamps = blues();
        lamps.lamps = vec![Lamp { at: [0.0; 3], reach: 5.0, strength: 1.0 }];
        assert!(lamps.lights());
    }

    #[test]
    fn bad_lights_and_ramps_are_refused_and_dropped() {
        let mut look = blues();
        look.sun = Some(Sun { toward: [0.0; 3], ..sun() });
        assert_eq!(look.validate(), Err(LookError::Sun));
        assert_eq!(look.normalized().and_then(|l| l.sun), None);
        let mut look = blues();
        look.lamps = vec![Lamp { at: [0.0; 3], reach: 0.0, strength: 1.0 }; 2];
        assert_eq!(look.validate(), Err(LookError::Lamps));
        assert!(look.normalized().is_some_and(|l| l.lamps.is_empty()));
        let mut look = blues();
        look.warm = Some(vec![[0; 3]; 3]);
        assert_eq!(look.validate(), Err(LookError::Warm));
        look.warm = None;
        look.water = Some(vec![[0; 3]; 2]);
        assert_eq!(look.validate(), Err(LookError::Warm));
        assert_eq!(look.normalized().and_then(|l| l.water), None);
        let mut look = blues();
        look.zones = vec![Zone { ramp: vec![[0; 3]; 3], at: [0.0; 2], half: [0.0; 2], radius: 5.0, feather: 1.0 }];
        assert_eq!(look.validate(), Err(LookError::Zones));
        assert!(look.normalized().is_some_and(|l| l.zones.is_empty()));
        let mut look = blues();
        look.reveal = Some(Reveal { at: [0.0; 3], radius: -1.0 });
        assert_eq!(look.normalized().and_then(|l| l.reveal), None);
    }

    #[test]
    fn a_zone_is_whole_in_its_box_and_fades_over_its_feather() {
        let mut look = blues();
        look.zones = vec![Zone { ramp: reds(), at: [0.0, 0.0], half: [10.0, 2.0], radius: 1.0, feather: 4.0 }];
        assert_eq!(look.validate(), Ok(()));
        let w = |x: f32, z: f32| look.zone_weights(x, z)[0];
        assert_eq!(w(0.0, 0.0), 1.0);
        assert_eq!(w(10.0, 2.0), 1.0, "the box's corner");
        assert_eq!(w(11.0, 0.0), 1.0, "within its radius");
        assert_eq!(w(15.0, 0.0), 0.0, "past the feather");
        let middle = w(13.0, 0.0);
        assert!(middle > 0.2 && middle < 0.8, "{middle}");
        // The ramp there is the zone's inside, the look's outside, between in the feather.
        let inside = look.ramp_at(0.0, 0.0);
        let outside = look.ramp_at(40.0, 0.0);
        let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-6);
        assert!(near(inside[3], unit_rgb(reds()[3])), "{:?}", inside[3]);
        assert!(near(outside[3], unit_rgb(look.ramp[3])), "{:?}", outside[3]);
        let between = look.ramp_at(13.0, 0.0)[3];
        assert!((0..3).all(|k| between[k] >= inside[3][k].min(outside[3][k]) && between[k] <= inside[3][k].max(outside[3][k])));
    }

    #[test]
    fn the_shadow_map_is_centred_on_its_focus_and_deeper_away_from_the_light() {
        let s = sun();
        let m = s.shadow_matrix(2048).expect("a shadow");
        let clip = |p: [f32; 3]| crate::scene::mat_point(&m, p);
        let focus = clip([10.0, 2.0, -5.0]);
        // Within a texel of the middle, halfway through the depth.
        assert!(focus[0].abs() < 2.0 / 2048.0 * 2.0 && focus[1].abs() < 2.0 / 2048.0 * 2.0, "{focus:?}");
        assert!((focus[2] - 0.5).abs() < 1e-4, "{focus:?}");
        let t = s.toward_unit();
        let nearer = clip([10.0 + t[0] * 10.0, 2.0 + t[1] * 10.0, -5.0 + t[2] * 10.0]);
        assert!(nearer[2] < focus[2], "toward the light is nearer it");
        // A point half the reach across is at the edge of the map.
        let r = cross_unit(t);
        let edge = clip([10.0 + r[0] * 40.0, 2.0 + r[1] * 40.0, -5.0 + r[2] * 40.0]);
        assert!((edge[0].abs().max(edge[1].abs()) - 1.0).abs() < 0.01, "{edge:?}");
        assert_eq!(Sun { shadow: None, ..s }.shadow_matrix(2048), None);
    }

    /// Some unit direction square to `t`.
    fn cross_unit(t: [f32; 3]) -> [f32; 3] {
        let c = [t[1] * 0.0 - t[2] * 1.0, t[2] * 0.0 - t[0] * 0.0, t[0] * 1.0 - t[1] * 0.0];
        let l = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
        c.map(|v| v / l)
    }

    #[test]
    fn a_look_goes_to_text_and_back_exactly() {
        let mut look = blues();
        look.fog = Some(Fog { color: 2, start: 10.5, end: 600.0 });
        look.sun = Some(sun());
        look.lamps = vec![Lamp { at: [1.5, 2.0, -3.25], reach: 12.0, strength: 0.75 }];
        look.warm = Some(reds());
        look.water = Some(reds());
        look.zones = vec![Zone { ramp: reds(), at: [3.0, -4.0], half: [10.0, 2.0], radius: 1.0, feather: 4.0 }];
        look.reveal = Some(Reveal { at: [7.0, 8.0, 9.0], radius: 6.0 });
        look.reveals = vec![Reveal { at: [1.0, 2.0, 3.0], radius: 5.0 }];
        look.time = 12.375;
        look.metre = 11.0;
        look.dither = Dither::Bayer8;
        assert_eq!(Look::from_text(&look.to_text()), Some(look.clone()));
        let plain = blues();
        assert_eq!(Look::from_text(&plain.to_text()), Some(plain));
        assert_eq!(Look::from_text("ramp zz"), None);
        assert_eq!(Look::from_text("nonsense 1"), None);
    }
}
