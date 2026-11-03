//! The card bridge: a card's `vfx` lines, as the game reads them
//! (`crates/delve/src/rules/vfx.rs`), made into an fx recipe, so every
//! wish's effect renders at any grain.
//!
//! ```text
//! vfx <when> <form> [palette] [motion] [ease] [count=N] [speed=N] [spread=N] [life=N] [size=N] [wobble=N] [spin=N]
//! ```
//!
//! The reading is the game's, written again here so kiln stands alone: the
//! same words, the same defaults by `when`, the same bounds, and a line that
//! does not read is set aside with a note, never the card. The card's
//! numbers are in the game's ticks (30 a second) and half pixels a tick;
//! the bridge plays at 15 frames a second, two ticks a frame.
//!
//! Each `when` is staged where it happens: `cast` about the caster, `fire`
//! at the hand, `trail` behind a shot crossing the stage, `hit` where it
//! lands, `kill` there a moment later, `aura` about the caster all along.

use super::recipe::{Form, Kind, Layer, Motion, Range, Recipe, Spawn};
use super::rng::unit;
use super::time::Ease;

/// Ticks a second in the game.
const HZ: f32 = 30.0;
/// The bridge's frames a second.
pub const FPS: u32 = 15;
/// Ticks a frame.
const R: f32 = HZ / FPS as f32;
/// The stage: four tiles by two.
pub const STAGE: [u32; 2] = [64, 32];
/// The caster's middle, the hand, and where the shot lands.
const MID: (f32, f32) = (10.0, 19.0);
const HAND: (f32, f32) = (15.0, 18.0);
const TARGET: (f32, f32) = (46.0, 18.0);
/// Frames a shot takes to cross.
const FLIGHT: f32 = 8.0;
/// The most layers a card has.
const MOST_LAYERS: usize = 6;

const WHENS: [&str; 6] = ["cast", "fire", "trail", "hit", "kill", "aura"];
const FORMS: [&str; 19] = [
    "sparks", "embers", "motes", "droplets", "shards", "flakes", "bubbles", "notes", "stars", "petals", "flies", "coins", "hearts",
    "worms", "smoke", "ring", "glyph", "arc", "rays",
];
const PALETTES: [&str; 19] = [
    "fire", "ember", "frost", "holy", "gold", "arcane", "genie", "storm", "poison", "nature", "blood", "rose", "poop", "brew", "void",
    "water", "bone", "smoke", "earth",
];
const MOTIONS: [&str; 8] = ["straight", "wobble", "spiral", "zigzag", "curl", "bounce", "rise", "fall"];
const EASES: [&str; 5] = ["out", "in", "both", "back", "spring"];

/// One `vfx` line, read.
#[derive(Clone, Debug, PartialEq)]
pub struct CardLayer {
    pub on: &'static str,
    pub form: &'static str,
    pub palette: &'static str,
    pub motion: &'static str,
    pub ease: &'static str,
    pub count: u8,
    pub speed: u8,
    pub spread: u16,
    pub life: u8,
    pub size: u8,
    pub wobble: u8,
    pub spin: i8,
}

fn word(list: &[&'static str], w: &str) -> Option<&'static str> {
    list.iter().copied().find(|x| *x == w)
}

/// The most motes a layer of each kind throws at once.
fn most(on: &str) -> u8 {
    match on {
        "trail" => 6,
        "fire" => 12,
        "hit" => 16,
        "aura" => 8,
        _ => 24,
    }
}

impl CardLayer {
    /// Read the words after `vfx` as the game does: the layer and the notes
    /// on what was held to its bounds, or why it does not read.
    pub fn read(rest: &str) -> Result<(CardLayer, Vec<String>), String> {
        let lower = rest.to_ascii_lowercase();
        let mut words = lower.split_whitespace();
        let on = words.next().and_then(|w| word(&WHENS, w)).ok_or_else(|| format!("vfx starts with when it plays: {}", WHENS.join(" ")))?;
        let form = words.next().and_then(|w| word(&FORMS, w)).ok_or_else(|| format!("vfx {on} needs a form: {}", FORMS.join(" ")))?;
        let (count, speed, spread, life) = match on {
            "cast" => (12, 4, 360, 24),
            "fire" => (5, 4, 70, 8),
            "trail" => (3, 1, 50, 10),
            "hit" => (10, 4, 360, 14),
            "kill" => (16, 5, 360, 22),
            _ => (4, 1, 360, 36),
        };
        let mut l = CardLayer { on, form, palette: "arcane", motion: "straight", ease: "out", count, speed, spread, life, size: 1, wobble: 0, spin: 0 };
        for w in words {
            if let Some((key, value)) = w.split_once('=') {
                let number = || value.parse::<i32>().map_err(|_| format!("vfx {key}={value}: a whole number"));
                let byte = |v: i32| v.clamp(0, 255) as u8;
                match key {
                    "count" => l.count = byte(number()?),
                    "speed" => l.speed = byte(number()?),
                    "spread" => l.spread = number()?.clamp(0, 9999) as u16,
                    "life" => l.life = byte(number()?),
                    "size" => l.size = byte(number()?),
                    "wobble" => l.wobble = byte(number()?),
                    "spin" => l.spin = number()?.clamp(-128, 127) as i8,
                    "palette" => l.palette = word(&PALETTES, value).ok_or_else(|| format!("vfx palette={value}: one of {}", PALETTES.join(" ")))?,
                    "motion" => l.motion = word(&MOTIONS, value).ok_or_else(|| format!("vfx motion={value}: one of {}", MOTIONS.join(" ")))?,
                    "ease" => l.ease = word(&EASES, value).ok_or_else(|| format!("vfx ease={value}: one of {}", EASES.join(" ")))?,
                    _ => return Err(format!("vfx: unknown setting `{key}` (count speed spread life size wobble spin palette motion ease)")),
                }
            } else if let Some(p) = word(&PALETTES, w) {
                l.palette = p;
            } else if let Some(m) = word(&MOTIONS, w) {
                l.motion = m;
            } else if let Some(e) = word(&EASES, w) {
                l.ease = e;
            } else {
                return Err(format!("vfx: `{w}` is no palette, motion or ease"));
            }
        }
        let mut notes = Vec::new();
        let mut clamp = |name: &str, v: i32, lo: i32, hi: i32| -> i32 {
            let c = v.clamp(lo, hi);
            if c != v {
                notes.push(format!("vfx {name} {v} clamped to {c}"));
            }
            c
        };
        l.count = clamp("count", l.count.into(), 1, most(on).into()) as u8;
        l.speed = clamp("speed", l.speed.into(), 0, 8) as u8;
        l.spread = clamp("spread", l.spread.into(), 0, 360) as u16;
        l.life = clamp("life", l.life.into(), 4, 40) as u8;
        l.size = clamp("size", l.size.into(), 1, 3) as u8;
        l.wobble = clamp("wobble", l.wobble.into(), 0, 8) as u8;
        l.spin = clamp("spin", l.spin.into(), -8, 8) as i8;
        Ok((l, notes))
    }

    /// Its life in the bridge's frames.
    fn frames(&self) -> f32 {
        f32::from(self.life) / R
    }
}

fn ease_of(word: &str) -> Ease {
    match word {
        "in" => Ease::In,
        "both" => Ease::Both,
        "back" => Ease::Back,
        "spring" => Ease::Spring,
        _ => Ease::Out,
    }
}

fn motion_of(word: &str) -> Motion {
    match word {
        "wobble" => Motion::Wobble,
        "spiral" => Motion::Spiral,
        "zigzag" => Motion::Zigzag,
        "curl" => Motion::Curl,
        "bounce" => Motion::Bounce,
        "rise" => Motion::Rise,
        "fall" => Motion::Fall,
        _ => Motion::Straight,
    }
}

fn form_of(word: &str) -> Option<Form> {
    Some(match word {
        "sparks" => Form::Spark,
        "embers" => Form::Ember,
        "motes" => Form::Mote,
        "droplets" => Form::Droplet,
        "shards" => Form::Shard,
        "flakes" => Form::Flake,
        "bubbles" => Form::Bubble,
        "notes" => Form::Note,
        "stars" => Form::Star,
        "petals" => Form::Petal,
        "flies" => Form::Fly,
        "coins" => Form::Coin,
        "hearts" => Form::Heart,
        "worms" => Form::Worm,
        "smoke" => Form::Smoke,
        _ => return None,
    })
}

/// A blank layer of `kind`.
fn blank(kind: Kind) -> Layer {
    serde_json::from_value(serde_json::json!({ "type": kind })).expect("a bare layer reads")
}

/// A card's `vfx` lines (with or without the leading `vfx`) as an fx
/// recipe, and the notes on what was set aside or held to its bounds.
pub fn from_card(lines: &[&str]) -> (Recipe, Vec<String>) {
    let mut notes = Vec::new();
    let mut read = Vec::new();
    for line in lines {
        let rest = line.trim();
        let rest = rest.strip_prefix("vfx").map_or(rest, str::trim_start);
        if rest.is_empty() {
            continue;
        }
        if read.len() >= MOST_LAYERS {
            notes.push(format!("{line:?}: set aside, a card has at most {MOST_LAYERS} layers"));
            continue;
        }
        match CardLayer::read(rest) {
            Ok((l, n)) => {
                notes.extend(n);
                read.push(l);
            }
            Err(e) => notes.push(format!("{line:?}: set aside, {e}")),
        }
    }
    let has = |on: &str| read.iter().any(|l| l.on == on);
    let cast = read.iter().filter(|l| l.on == "cast").map(CardLayer::frames).fold(0.0f32, f32::max);
    let shot = has("fire") || has("trail") || has("hit") || has("kill");
    // The cast plays first; its shot is loosed as it peaks.
    let loose = if cast > 0.0 { (cast * 0.4).round() } else { 0.0 };
    let lands = loose + FLIGHT;
    let mut layers: Vec<Layer> = Vec::new();
    if shot {
        // The shot itself: a small hot head of the wish's colour crossing,
        // a short tail behind it.
        let head = read.iter().find(|l| l.on == "trail").or_else(|| read.iter().find(|l| l.on == "fire")).or(read.first());
        let mut l = blank(Kind::Comet);
        l.ramp = Some(head.map_or("arcane", |h| h.palette).to_string());
        (l.x, l.y, l.x1, l.y1) = (Some(HAND.0), Some(HAND.1), Some(TARGET.0), Some(TARGET.1));
        (l.r1, l.size) = (Some(1.1), Some(Range::One(5.0)));
        (l.t0, l.t1) = (Some(super::Time::Frame(loose)), Some(super::Time::Frame(lands)));
        layers.push(l);
    }
    for (n, c) in read.iter().enumerate() {
        let (t0, at, angle, scale) = match c.on {
            "cast" => (0.0, MID, -90.0, 1.0),
            "fire" => (loose, HAND, 0.0, 1.0),
            "trail" => (loose, HAND, 180.0, 1.0),
            "hit" => (lands, TARGET, 180.0, 1.0),
            "kill" => (lands + 1.0, (TARGET.0, TARGET.1 - 4.0), -90.0, 1.15),
            _ => (0.0, MID, 0.0, 1.0),
        };
        layers.extend(staged(c, n as u32, t0, at, angle, scale, lands));
    }
    if layers.is_empty() {
        notes.push("no line read: the stage alone".into());
        let mut l = blank(Kind::Orb);
        (l.x, l.y) = (Some(MID.0), Some(MID.1));
        layers.push(l);
    }
    let aura_only = !shot && has("aura") && cast == 0.0;
    let ends = read.iter().map(|c| match c.on {
        "cast" => c.frames(),
        "aura" => 0.0,
        "trail" => lands + c.frames(),
        "kill" => lands + 1.0 + c.frames() * 1.15,
        _ => lands + c.frames(),
    });
    let frames = if aura_only { 30.0 } else { ends.fold(if shot { lands + 4.0 } else { 12.0 }, f32::max) + 1.0 };
    let recipe = Recipe {
        fx: super::recipe::VERSION,
        name: "card".into(),
        canvas: STAGE,
        pivot: Some([0.0, 0.0]),
        fps: Some(FPS),
        frames: Some(frames.ceil().min(120.0) as u32),
        looping: aura_only.then_some(true),
        seed: Some(7),
        beats: None,
        outline: None,
        layers,
    };
    (recipe, notes)
}

/// The fx layers card layer `c` (the `n`th) plays as, starting at `t0` at
/// `at`, thrown along `angle`, `scale` times its size; `lands` is when the
/// shot lands, where a trail ends.
fn staged(c: &CardLayer, n: u32, t0: f32, at: (f32, f32), angle: f32, scale: f32, lands: f32) -> Vec<Layer> {
    let life = c.frames() * scale.clamp(0.6, 1.6);
    let speed = f32::from(c.speed) * 0.5 * R;
    let t1 = |len: f32| Some(super::Time::Frame(t0 + len.max(0.5)));
    let mut out = Vec::new();
    let base = |kind: Kind| {
        let mut l = blank(kind);
        l.ramp = Some(c.palette.to_string());
        (l.x, l.y) = (Some(at.0), Some(at.1));
        l.t0 = Some(super::Time::Frame(t0));
        l.seed = Some(n.wrapping_mul(97));
        l
    };
    match c.form {
        "ring" => {
            let reach = (5.0 + speed / R * 3.0) * scale;
            let mut l = base(Kind::Ring);
            (l.r0, l.r1) = if c.ease == "in" { (Some(reach), Some(1.0)) } else { (Some(1.0), Some(reach)) };
            l.th = Some(1.2 + scale * 0.6);
            l.squash = Some(0.62);
            l.ease = Some(if c.ease == "in" { Ease::Out } else { ease_of(c.ease) });
            l.t1 = t1(life);
            out.push(l);
        }
        "glyph" => {
            let mut l = base(Kind::Glyph);
            l.r1 = Some((7.0 + speed / R * 1.5) * scale);
            l.squash = Some(0.55);
            l.spin = Some(if c.spin == 0 { 3.0 } else { f32::from(c.spin) * 2.3 });
            l.ease = Some(ease_of(c.ease));
            l.t1 = t1(life);
            out.push(l);
        }
        "arc" => {
            for i in 0..u32::from(c.count).min(4) {
                let a = unit(i as i32, n as i32, 0xa1) * std::f32::consts::TAU;
                let len = (6.0 + speed / R * 2.5) * scale * (0.6 + unit(i as i32, n as i32, 0xa2) * 0.6);
                let mut l = base(Kind::Bolt);
                (l.x1, l.y1) = (Some(at.0 + a.cos() * len), Some(at.1 + a.sin() * len * 0.7));
                l.jag = Some(2.0 * scale);
                l.count = Some(1);
                l.flicker = Some(1.0);
                l.th = Some(1.0);
                l.seed = Some(n * 31 + i);
                l.t1 = t1(life * 0.55);
                out.push(l);
            }
        }
        "rays" => {
            let mut l = base(Kind::Rays);
            l.count = Some(u32::from(c.count).clamp(4, 12));
            (l.r0, l.r1) = (Some(1.0), Some((5.0 + speed / R * 2.0) * scale + 1.0));
            l.th = Some(0.7);
            l.spin = Some(f32::from(c.spin) * 2.3);
            l.t1 = t1(life);
            out.push(l);
        }
        form => {
            let Some(form) = form_of(form) else { return out };
            let mut l = base(Kind::Particles);
            l.form = Some(form);
            let size = f32::from(c.size) * scale;
            let small = matches!(form, Form::Mote | Form::Droplet | Form::Ember | Form::Petal);
            l.size = Some(Range::One(if c.on == "trail" && small { size.max(2.0) } else { size }));
            l.life = Some(Range::Two([life * 0.55, life]));
            l.speed = Some(Range::Two([speed * 0.55 * scale, speed * 1.25 * scale]));
            l.angle = Some(angle);
            l.spread = Some(f32::from(c.spread));
            l.drag = Some(0.14 * R);
            let heavy = matches!(form, Form::Shard | Form::Droplet | Form::Coin | Form::Ember | Form::Petal);
            let g = match c.motion {
                "fall" => 0.14,
                "rise" => -0.07,
                "bounce" => 0.18,
                _ if heavy => 0.08,
                _ => 0.0,
            };
            l.gravity = Some(g * R * R);
            l.squash = Some(0.8);
            l.motion = Some(motion_of(c.motion));
            if c.wobble > 0 {
                l.wobble = Some(f32::from(c.wobble) * 0.6);
            }
            if c.spin != 0 {
                l.spin = Some(f32::from(c.spin) * 10.3);
            }
            if c.ease != "out" {
                l.ease = Some(ease_of(c.ease));
            }
            match c.on {
                "trail" => {
                    // Motes let go behind the shot as it crosses, about as
                    // far apart as the game lays them along its path.
                    let wide = matches!(form, Form::Note | Form::Heart | Form::Bubble | Form::Coin | Form::Worm | Form::Fly);
                    let gap = if wide { 7.0 } else { 3.0 } + f32::from(c.size);
                    let per = (TARGET.0 - HAND.0) / FLIGHT;
                    (l.x1, l.y1) = (Some(TARGET.0), Some(TARGET.1));
                    l.count = Some(0);
                    l.rate = Some((per / gap).min(8.0));
                    l.t1 = Some(super::Time::Frame(lands));
                    l.ease = Some(Ease::Linear);
                }
                "aura" => {
                    l.t1 = None;
                    l.count = Some(0);
                    l.rate = Some((f32::from(c.count) / life.max(1.0)).max(0.1));
                    l.spawn = Some(Spawn::Disc);
                    l.area = Some([8.0, 9.0]);
                    if c.motion == "straight" {
                        l.motion = Some(Motion::Orbit);
                        l.area = Some([11.0, 4.5]);
                        l.spin = Some(6.0);
                    }
                    l.speed = Some(Range::Two([0.1, 0.4]));
                    l.angle = Some(-90.0);
                }
                _ => {
                    l.count = Some(u32::from(c.count));
                    l.t1 = t1(life);
                }
            }
            out.push(l);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_reads_as_the_game_reads_it() {
        let (l, notes) = CardLayer::read("hit shards frost back count=12 speed=5 spread=360 life=16").unwrap();
        assert_eq!((l.on, l.form, l.palette, l.ease, l.count, l.speed, l.life), ("hit", "shards", "frost", "back", 12, 5, 16));
        assert!(notes.is_empty());
        let (l, notes) = CardLayer::read("trail worms poop wobble count=9 wobble=2").unwrap();
        assert_eq!((l.motion, l.count, l.wobble), ("wobble", 6, 2));
        assert_eq!(notes, vec!["vfx count 9 clamped to 6".to_string()]);
        assert!(CardLayer::read("hit plaid").is_err());
    }

    #[test]
    fn a_card_becomes_a_recipe_and_a_bad_line_only_a_note() {
        let (r, notes) = from_card(&["vfx trail worms poop wobble count=3 wobble=2", "vfx hit nonsense", "vfx hit shards frost back count=12 speed=5 spread=360 life=16"]);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(r.check().is_ok());
        // The shot, the trail and the hit.
        assert_eq!(r.layers.len(), 3);
        assert_eq!(r.layers[0].kind, Kind::Comet);
        assert_eq!(r.layers[1].form, Some(Form::Worm));
        assert_eq!(r.layers[2].ease, Some(Ease::Back));
        let fx = super::super::Fx::new(r).unwrap();
        assert!(fx.frames() > 10);
    }
}
