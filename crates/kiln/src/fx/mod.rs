//! Effects: emitters, smears, rings, bolts, flames, glows and the rest,
//! in the realm's inks at any grain (docs/KILN-FX.md).
//!
//! An effect is a [`Recipe`] (JSON, versioned, every field checked) made
//! into an [`Fx`], which renders any frame on its own: nothing is kept
//! between frames and nothing is rolled, so frame 7 is the same pixels
//! whether it is drawn first or after frame 6, on any machine.
//!
//! Every layer is a field of heat in game pixels ([`pen`]), sampled at the
//! grain being drawn, so one recipe gives a pixel-true spark at grain 1 and
//! a lit, cored one at grain 4. A card's `vfx` lines become a recipe
//! through [`from_card`], and the VFX cartridge's v1 recipes through
//! [`from_cartridge_v1`].

pub mod card;
pub mod cartridge;
mod emit;
pub mod forms;
pub mod pen;
pub mod ramps;
pub mod recipe;
pub mod rng;
mod shapes;
pub mod time;

pub use card::from_card;
pub use cartridge::from_cartridge_v1;
pub use pen::{Mode, Pen};
pub use recipe::{Form, Kind, Layer, Motion, Range, Recipe, Spawn, Z};
pub use time::{Beats, Ease, Time};

use crate::grain::Grain;
use crate::palette::{Palette, Ramp};
use crate::raster::{PartId, Raster};

/// The part number an effect's layer `i` draws with: effects are
/// `0xF000` and up, so the look tools can tell them from a figure's parts.
pub const fn part(i: usize) -> PartId {
    0xF000 + i as PartId
}

/// A layer made ready to draw: its type's defaults filled in, its ramp,
/// times and places resolved.
#[derive(Clone, Debug)]
pub(crate) struct Plan {
    pub kind: Kind,
    pub form: Form,
    pub ramp: Ramp,
    pub mode: Mode,
    pub depth: f32,
    pub part: PartId,
    pub t0: f32,
    pub t1: f32,
    pub ease: Ease,
    pub at: (f32, f32),
    pub end: Option<(f32, f32)>,
    pub lift: f32,
    pub r0: f32,
    pub r1: f32,
    pub th: f32,
    pub angle: f32,
    pub arc: f32,
    pub squash: f32,
    pub gain: f32,
    pub heat: (f32, f32),
    pub count: u32,
    pub rate: f32,
    pub life: (f32, f32),
    pub speed: (f32, f32),
    pub spread: f32,
    pub gravity: f32,
    pub drag: f32,
    pub size: (f32, f32),
    pub spawn: Spawn,
    pub area: (f32, f32),
    pub motion: Motion,
    pub wobble: f32,
    pub spin: f32,
    pub jag: f32,
    pub flicker: f32,
    pub seed: u32,
    pub text: String,
}

impl Plan {
    /// How far through its window the layer is at frame `t`, 0 to 1.
    pub fn k(&self, t: f32) -> f32 {
        ((t - self.t0) / (self.t1 - self.t0).max(1e-3)).clamp(0.0, 1.0)
    }

    /// Where the layer is at frame `t`: its place, or along its travel to
    /// its end, eased, arcing by its lift.
    pub fn anchor(&self, t: f32) -> (f32, f32) {
        let travels = matches!(self.kind, Kind::Particles | Kind::Puffs | Kind::Orb | Kind::Flash | Kind::Comet);
        match self.end {
            Some(b) if travels => {
                let e = self.ease.at(self.k(t));
                let (a, b) = (self.at, b);
                (a.0 + (b.0 - a.0) * e, a.1 + (b.1 - a.1) * e - self.lift * 4.0 * e * (1.0 - e))
            }
            _ => self.at,
        }
    }

    /// The layer's heat at `k` through its window, faded out over its last
    /// fifth so nothing stops dead.
    pub fn heat_at(&self, k: f32) -> f32 {
        let h = self.heat.0 + (self.heat.1 - self.heat.0) * k;
        h * self.gain * ((1.0 - k) * 5.0).clamp(0.0, 1.0)
    }
}

/// An effect ready to render.
#[derive(Clone, Debug)]
pub struct Fx {
    pub recipe: Recipe,
    plans: Vec<Plan>,
    frames: u32,
    fps: u32,
    looping: bool,
}

impl Fx {
    /// An effect from its JSON.
    pub fn parse(json: &str) -> Result<Fx, String> {
        Fx::new(Recipe::parse(json)?)
    }

    /// An effect from its recipe, checked and resolved.
    pub fn new(recipe: Recipe) -> Result<Fx, String> {
        recipe.check()?;
        let beats = recipe.beats.unwrap_or_default();
        let seed = recipe.seed.unwrap_or(1);
        let pivot = recipe.pivot.map_or((recipe.canvas[0] as f32 / 2.0, recipe.canvas[1] as f32 / 2.0), |p| (p[0], p[1]));
        // How long it runs: as it says, else its beats, else what its
        // layers ask for.
        let frames = match (recipe.frames, beats.total()) {
            (Some(f), _) => f,
            (None, b) if b > 0.0 => b.ceil() as u32,
            (None, _) => {
                let mut end: f32 = 1.0;
                for l in &recipe.layers {
                    for t in [&l.t0, &l.t1].into_iter().flatten() {
                        if let Time::Frame(f) = t {
                            end = end.max(*f);
                        }
                    }
                    if let (Some(Time::Frame(t1)), Some(life)) = (&l.t1, l.life) {
                        end = end.max(t1 + life.pair().1);
                    } else if let (Some(Time::Frame(t0)), Some(life)) = (&l.t0, l.life) {
                        end = end.max(t0 + life.pair().1);
                    }
                }
                if end <= 1.0 { 12 } else { end.ceil() as u32 }
            }
        };
        let mut plans = Vec::with_capacity(recipe.layers.len());
        for (i, l) in recipe.layers.iter().enumerate() {
            plans.push(plan(l, i, &beats, frames as f32, pivot, seed).map_err(|e| format!("layers[{i}].{e}"))?);
        }
        Ok(Fx { fps: recipe.fps.unwrap_or(time::FPS), looping: recipe.looping.unwrap_or(false), recipe, plans, frames })
    }

    pub fn name(&self) -> &str {
        &self.recipe.name
    }

    pub fn frames(&self) -> u32 {
        self.frames
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn looping(&self) -> bool {
        self.looping
    }

    /// Its size in game pixels.
    pub fn canvas(&self) -> (u32, u32) {
        (self.recipe.canvas[0], self.recipe.canvas[1])
    }

    /// Frame `frame` at `grain`, on a clear canvas of its own.
    pub fn render(&self, grain: Grain, frame: u32) -> Raster {
        let (w, h) = self.canvas();
        let mut r = Raster::new(w * u32::from(grain.0), h * u32::from(grain.0));
        self.draw(&mut r, grain, (0.0, 0.0), frame as f32);
        if self.recipe.outline.unwrap_or(false) {
            outline(&mut r);
        }
        r
    }

    /// Draw frame `t` (fractions allowed) into `raster`, drawn at `grain`,
    /// the effect's canvas `off` game pixels into it. Front layers go over
    /// whatever is there (a figure), back layers behind it.
    pub fn draw(&self, raster: &mut Raster, grain: Grain, off: (f32, f32), t: f32) {
        let n = self.frames as f32;
        let t = if self.looping { t.rem_euclid(n) } else { t };
        let mut pen = Pen::new(raster, grain.px(), off);
        for p in &self.plans {
            pen.depth = p.depth;
            pen.part = p.part;
            pen.mode = p.mode;
            match p.kind {
                Kind::Particles | Kind::Puffs => emit::draw(&mut pen, p, t, self.looping.then_some(n)),
                _ => {
                    if self.looping && p.t0 <= 0.0 && p.t1 >= n {
                        // A layer the length of the loop goes round with it.
                        let mut q = p.clone();
                        q.heat.1 = q.heat.0;
                        q.t1 = n + 1.0;
                        shapes::draw(&mut pen, &q, t);
                    } else if t >= p.t0 && t < p.t1 {
                        shapes::draw(&mut pen, p, t);
                    }
                }
            }
        }
    }
}

/// A rim of the darkest ink round everything drawn, a picture pixel wide.
fn outline(r: &mut Raster) {
    let k = Palette::realm().ink('k').unwrap_or_default();
    let was = r.clone();
    for y in 0..r.h as i32 {
        for x in 0..r.w as i32 {
            if !was.is_inked(x, y) && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| was.is_inked(x + dx, y + dy)) {
                r.put(x, y, k, -5000.0, part(0xfff));
            }
        }
    }
}

/// Layer `l` (the `i`th) made ready.
fn plan(l: &Layer, i: usize, beats: &Beats, frames: f32, pivot: (f32, f32), seed: u32) -> Result<Plan, String> {
    let ramp_name = l.ramp.clone().unwrap_or_else(|| {
        match l.kind {
            Kind::Slash | Kind::Streaks => "steel",
            Kind::Puffs => "smoke",
            Kind::Cracks => "earth",
            Kind::Flame | Kind::Burst | Kind::Comet => "fire",
            Kind::Bolt => "storm",
            Kind::Pillar => "holy",
            _ => "arcane",
        }
        .to_string()
    });
    let ramp = match &l.inks {
        Some(keys) => Ramp::parse(keys, Palette::realm()).map_err(|e| format!("inks: {e}"))?,
        None => ramps::named(&ramp_name).ok_or_else(|| format!("ramp: no ramp {ramp_name:?} ({})", ramps::names()))?,
    };
    let form = match l.kind {
        Kind::Puffs => Form::Smoke,
        _ => l.form.unwrap_or_default(),
    };
    let t0 = l.t0.as_ref().map_or(Ok(0.0), |t| t.resolve(beats, frames)).map_err(|e| format!("t0: {e}"))?;
    let t1 = l.t1.as_ref().map_or(Ok(frames), |t| t.resolve(beats, frames)).map_err(|e| format!("t1: {e}"))?;
    if t1 <= t0 {
        return Err(format!("t1: {t1} is not after t0 {t0}"));
    }
    let at = (pivot.0 + l.x.unwrap_or(0.0), pivot.1 + l.y.unwrap_or(0.0));
    let end = match (l.x1, l.y1) {
        (None, None) => match l.kind {
            Kind::Beam => Some((at.0 + 32.0, at.1)),
            Kind::Bolt => Some((at.0, at.1 + 24.0)),
            _ => None,
        },
        (x1, y1) => Some((pivot.0 + x1.unwrap_or(l.x.unwrap_or(0.0)), pivot.1 + y1.unwrap_or(l.y.unwrap_or(0.0)))),
    };
    // Each type's own defaults: (r0, r1, th, squash, heat, count, size).
    let (r0, r1, th, squash, heat, count, size) = match l.kind {
        Kind::Particles => (0.0, 0.0, 0.0, 1.0, (1.0, 0.35), 12, form_size(form)),
        Kind::Puffs => (0.0, 0.0, 0.0, 1.0, (0.8, 0.35), 6, (2.0, 2.0)),
        Kind::Burst => (1.0, 9.0, 0.0, 1.0, (1.0, 0.15), 0, (0.0, 0.0)),
        Kind::Ring => (1.0, 12.0, 1.4, 1.0, (1.0, 0.2), 0, (0.0, 0.0)),
        Kind::Shock => (2.0, 16.0, 1.2, 0.45, (1.0, 0.15), 0, (0.0, 0.0)),
        Kind::Flame => (0.0, 4.0, 0.0, 1.0, (1.0, 1.0), 5, (10.0, 10.0)),
        Kind::Beam => (0.0, 0.0, 2.0, 1.0, (1.0, 0.8), 0, (0.0, 0.0)),
        Kind::Pillar => (0.0, 30.0, 3.0, 1.0, (1.0, 0.6), 0, (0.0, 0.0)),
        Kind::Slash => (0.0, 10.0, 3.0, 1.0, (1.0, 0.8), 0, (0.0, 0.0)),
        Kind::Orb => (3.0, 3.0, 0.0, 1.0, (1.0, 1.0), 0, (0.0, 0.0)),
        Kind::Flash => (2.0, 7.0, 0.0, 1.0, (1.0, 0.0), 0, (0.0, 0.0)),
        Kind::Comet => (0.0, 2.2, 0.0, 1.0, (1.0, 1.0), 0, (10.0, 10.0)),
        Kind::Vortex => (0.0, 8.0, 2.0, 0.6, (1.0, 0.6), 4, (0.0, 0.0)),
        Kind::Rays => (2.0, 14.0, 0.8, 1.0, (1.0, 0.3), 10, (0.0, 0.0)),
        Kind::Glyph => (0.0, 10.0, 0.0, 0.45, (1.0, 1.0), 5, (0.0, 0.0)),
        Kind::Bolt => (0.0, 0.0, 1.2, 1.0, (1.0, 0.4), 2, (0.0, 0.0)),
        Kind::Cracks => (0.0, 12.0, 0.8, 0.45, (1.0, 1.0), 6, (0.0, 0.0)),
        Kind::Spikes => (0.0, 8.0, 1.6, 1.0, (1.0, 1.0), 5, (9.0, 9.0)),
        Kind::Star => (0.0, 6.0, 1.0, 1.0, (1.0, 0.3), 8, (0.0, 0.0)),
        Kind::Streaks => (0.0, 18.0, 1.6, 1.0, (1.0, 0.3), 5, (0.0, 0.0)),
        Kind::Bubble => (0.0, 7.0, 0.8, 1.0, (1.0, 0.6), 0, (0.0, 0.0)),
        Kind::Text => (0.0, 0.0, 0.0, 1.0, (1.0, 1.0), 0, (1.0, 1.0)),
        Kind::Tornado => (0.0, 6.0, 0.0, 1.0, (1.0, 1.0), 0, (20.0, 20.0)),
    };
    let emits = matches!(l.kind, Kind::Particles | Kind::Puffs);
    let motion = l.motion.unwrap_or_default();
    let heavy = matches!(form, Form::Shard | Form::Droplet | Form::Coin | Form::Petal);
    let gravity = l.gravity.unwrap_or(match motion {
        Motion::Rise => -0.06,
        Motion::Fall => 0.12,
        Motion::Bounce => 0.15,
        _ if l.kind == Kind::Puffs => -0.03,
        _ if heavy => 0.1,
        _ => 0.0,
    });
    let mode = l.mode.unwrap_or(match (l.kind, form) {
        (Kind::Puffs, _) | (Kind::Particles, Form::Smoke) => Mode::Under,
        (Kind::Particles, Form::Shard | Form::Coin | Form::Worm | Form::Petal | Form::Droplet | Form::Fly) => Mode::Over,
        (Kind::Spikes | Kind::Cracks | Kind::Text, _) => Mode::Over,
        _ => Mode::Light,
    });
    let z = l.z.unwrap_or(match l.kind {
        Kind::Cracks | Kind::Shock => Z::Back,
        _ => Z::Front,
    });
    let depth = if z == Z::Front { 1000.0 } else { -1000.0 } + i as f32 * 0.01;
    Ok(Plan {
        kind: l.kind,
        form,
        ramp,
        mode,
        depth,
        part: part(i),
        t0,
        t1,
        ease: l.ease.unwrap_or(match l.kind {
            Kind::Glyph => Ease::Back,
            Kind::Particles | Kind::Puffs | Kind::Comet | Kind::Orb => Ease::Linear,
            _ => Ease::Out,
        }),
        at,
        end,
        lift: l.lift.unwrap_or(0.0),
        r0: l.r0.unwrap_or(r0),
        r1: l.r1.unwrap_or(r1),
        th: l.th.unwrap_or(th),
        angle: l.angle.unwrap_or(match l.kind {
            Kind::Slash => -60.0,
            Kind::Puffs => -90.0,
            _ => 0.0,
        }),
        arc: l.arc.unwrap_or(150.0),
        squash: l.squash.unwrap_or(squash),
        gain: l.gain.unwrap_or(1.0),
        heat: l.heat.map_or(heat, Range::pair),
        count: l.count.unwrap_or(count),
        rate: l.rate.unwrap_or(0.0),
        life: l.life.map_or(if l.kind == Kind::Puffs { (10.0, 16.0) } else { (6.0, 10.0) }, Range::pair),
        speed: l.speed.map_or(
            match l.kind {
                Kind::Puffs => (0.4, 0.9),
                Kind::Text => (6.0, 6.0),
                Kind::Particles => (1.5, 3.0),
                _ => (1.0, 1.0),
            },
            Range::pair,
        ),
        spread: l.spread.unwrap_or(if l.kind == Kind::Puffs { 70.0 } else { 360.0 }),
        gravity: if emits { gravity } else { 0.0 },
        drag: l.drag.unwrap_or(if l.kind == Kind::Puffs { 0.1 } else { 0.15 }),
        size: l.size.map_or(size, Range::pair),
        spawn: l.spawn.unwrap_or_default(),
        area: l.area.map_or((4.0, 4.0), |a| (a[0], a[1])),
        motion,
        wobble: l.wobble.unwrap_or(1.5),
        spin: l.spin.unwrap_or(match l.kind {
            Kind::Vortex => 12.0,
            Kind::Glyph => 3.0,
            Kind::Tornado => 20.0,
            Kind::Particles if matches!(motion, Motion::Spiral | Motion::Curl | Motion::Orbit) => 12.0,
            Kind::Particles => 25.0,
            _ => 0.0,
        }),
        jag: l.jag.unwrap_or(match l.kind {
            Kind::Bolt => end.map_or(3.0, |e| (e.0 - at.0).hypot(e.1 - at.1) * 0.14),
            _ => 1.5,
        }),
        flicker: l.flicker.unwrap_or(2.0),
        seed: rng::hash(i as i32, l.seed.unwrap_or(0) as i32, seed),
        text: l.text.clone().unwrap_or_else(|| "+1".into()),
    })
}

/// A form's size and how it changes over a mote's life, by default.
fn form_size(f: Form) -> (f32, f32) {
    match f {
        Form::Spark => (1.0, 0.6),
        Form::Ember | Form::Mote => (1.0, 0.6),
        Form::Smoke => (2.0, 2.0),
        Form::Note | Form::Heart => (1.6, 1.4),
        Form::Star => (1.2, 1.0),
        Form::Fly | Form::Worm | Form::Droplet => (1.0, 1.0),
        Form::Shard | Form::Flake | Form::Bubble | Form::Petal | Form::Coin => (1.2, 1.0),
    }
}

#[cfg(test)]
mod tests;
