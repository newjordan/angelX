//! The VFX cartridge's v1 recipes (`~/.angelX/cartridges/vfx`) read into
//! kiln's, so what was made there carries over: each of its 22 layer types
//! onto kiln's own of the same name, its times onto frames, its places and
//! sizes onto game pixels, and its free colours onto the nearest of the
//! realm's ramps.
//!
//! A v1 recipe is drawn at one native size, its shorter side spanning from
//! -1 to 1. Kiln reads a native pixel as a quarter of a game pixel, so the effect
//! comes out at its own size at grain 4 (an HD pack's) and a quarter of it
//! at grain 1. Its times run 0 to 1 over its frames; kiln's are frames.
//! What kiln does differently (a slash sweeps rather than turns, motes have
//! drag and their own lives) is noted, never refused.

use super::ramps;
use super::recipe::{Form, Kind, Layer, Motion, Range, Recipe, Spawn};
use super::time::{Ease, Time};
use serde_json::{Map, Value};

/// Game pixels a v1 native pixel.
pub const SCALE: f32 = 0.25;

const RECIPE_FIELDS: &[&str] =
    &["name", "w", "h", "frames", "fps", "loop", "seed", "outline", "dither", "clean", "palette", "hue", "pivot", "ramps", "layers"];
const LAYER_FIELDS: &[&str] =
    &["type", "ramp", "t0", "t1", "x", "y", "r0", "r1", "th", "angle", "squash", "gain", "speed", "spin", "count", "size", "life", "mode", "gravity", "text"];

/// v1's named ramps, dark to light, as it ships them.
const V1_RAMPS: &[(&str, [u32; 5])] = &[
    ("fire", [0x43172b, 0xa13235, 0xe86a2d, 0xffc45c, 0xfff1b0]),
    ("ice", [0x182e61, 0x286c9a, 0x52bfd0, 0xa7eaf4, 0xf0ffff]),
    ("lightning", [0x32204d, 0x7455b8, 0xb69aff, 0xe9d9ff, 0xffffff]),
    ("poison", [0x1b3031, 0x346346, 0x70a735, 0xb9df55, 0xf2ffc5]),
    ("holy", [0x584026, 0xb38337, 0xe9bd62, 0xffe8a4, 0xffffe8]),
    ("shadow", [0x17142b, 0x39234f, 0x654383, 0x9b70ba, 0xd8b3ed]),
    ("arcane", [0x232a59, 0x4250a8, 0x737bea, 0xb5b9ff, 0xf1ebff]),
    ("water", [0x102f51, 0x185d85, 0x309fbb, 0x81d3e4, 0xd1fbff]),
    ("wind", [0x244147, 0x47756c, 0x83b29c, 0xc3dfc0, 0xf2ffe7]),
    ("earth", [0x36293a, 0x635044, 0xa57b50, 0xd1af75, 0xf0ddac]),
    ("blood", [0x30172c, 0x6b203a, 0xb23748, 0xea6b6a, 0xffc1aa]),
    ("acid", [0x24352c, 0x547b2b, 0x9cba27, 0xdeec55, 0xfaffbf]),
    ("nature", [0x153c37, 0x267351, 0x53a46b, 0x9ed58c, 0xe2f5be]),
    ("crystal", [0x3b284f, 0x835494, 0xb788bd, 0xe2bde5, 0xfff1fc]),
    ("metal", [0x232d3b, 0x4d5a6d, 0x8894a7, 0xc0ceda, 0xf1f8ff]),
    ("rainbow", [0x5839a5, 0xa64c9e, 0xe76b75, 0xf1bb64, 0xd9edba]),
    ("smoke", [0x202333, 0x424456, 0x6e7180, 0xa3a6b0, 0xdce0e4]),
    ("frost", [0x253c60, 0x47798f, 0x7fbbca, 0xc8e7eb, 0xf7ffff]),
];

fn rgb(c: u32) -> [u8; 3] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}

/// A colour as `rrggbb` (a `#` allowed).
fn hex(v: &Value) -> Result<[u8; 3], String> {
    let s = v.as_str().ok_or("a colour is a hex string")?.trim_start_matches('#');
    if s.len() != 6 {
        return Err(format!("{s:?} is not rrggbb"));
    }
    u32::from_str_radix(s, 16).map(rgb).map_err(|_| format!("{s:?} is not rrggbb"))
}

/// v1's hue turn, in YIQ, as it does it.
fn turned(c: [u8; 3], deg: f32) -> [u8; 3] {
    if deg == 0.0 {
        return c;
    }
    let (r, g, b) = (f32::from(c[0]), f32::from(c[1]), f32::from(c[2]));
    let (y, i, q) = (0.299 * r + 0.587 * g + 0.114 * b, 0.596 * r - 0.274 * g - 0.322 * b, 0.211 * r - 0.523 * g + 0.312 * b);
    let (co, si) = (deg.to_radians().cos(), deg.to_radians().sin());
    let (ii, qq) = (i * co - q * si, i * si + q * co);
    let px = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    [px(y + 0.956 * ii + 0.621 * qq), px(y - 0.272 * ii - 0.647 * qq), px(y - 1.106 * ii + 1.703 * qq)]
}

/// A layer's colours: a v1 ramp's name, `$` and a recipe ramp's name, or
/// the colours themselves.
fn colours(v: Option<&Value>, own: &Map<String, Value>) -> Result<Vec<[u8; 3]>, String> {
    let v = v.cloned().unwrap_or(Value::String("fire".into()));
    let v = match v.as_str() {
        Some(s) if s.starts_with('$') => own.get(&s[1..]).cloned().ok_or_else(|| format!("ramp {s}: no such recipe ramp"))?,
        _ => v,
    };
    match &v {
        Value::String(name) => V1_RAMPS
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c.iter().map(|&c| rgb(c)).collect())
            .ok_or_else(|| format!("ramp {name:?}: not one of v1's")),
        Value::Array(list) if (2..=16).contains(&list.len()) => list.iter().map(hex).collect(),
        _ => Err("ramp: a name or 2 to 16 colours".into()),
    }
}

fn num(o: &Map<String, Value>, key: &str, default: f32) -> Result<f32, String> {
    match o.get(key) {
        None => Ok(default),
        Some(v) => v.as_f64().map(|f| f as f32).filter(|f| f.is_finite()).ok_or_else(|| format!("{key}: a number")),
    }
}

/// A v1 recipe's JSON as a kiln recipe, and notes on what carried over
/// only near enough.
pub fn from_cartridge_v1(json: &str) -> Result<(Recipe, Vec<String>), String> {
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let o = v.as_object().ok_or("a v1 recipe is an object")?;
    if let Some(k) = o.keys().find(|k| !RECIPE_FIELDS.contains(&k.as_str())) {
        return Err(format!("{k}: no v1 recipe field"));
    }
    let mut notes = Vec::new();
    let (w, h) = (num(o, "w", 64.0)?, num(o, "h", 64.0)?);
    let frames = num(o, "frames", 16.0)?.clamp(2.0, 64.0);
    let looping = o.get("loop").and_then(Value::as_bool).unwrap_or(false);
    let hue = num(o, "hue", 0.0)?;
    let own = o.get("ramps").and_then(Value::as_object).cloned().unwrap_or_default();
    for (key, what) in [("dither", "kiln dithers its own way"), ("clean", "kiln's cleanup is its own"), ("palette", "kiln draws in the realm's inks only")] {
        if o.contains_key(key) {
            notes.push(format!("{key}: set aside, {what}"));
        }
    }
    let outline = match o.get("outline").and_then(Value::as_str) {
        Some("outer") => Some(true),
        Some("inner") => {
            notes.push("outline inner: drawn as kiln's outer rim".into());
            Some(true)
        }
        _ => None,
    };
    // A v1 unit (half the shorter side) in game pixels.
    let u = w.min(h) / 2.0 * SCALE;
    let canvas = [(w * SCALE).ceil().max(1.0) as u32, (h * SCALE).ceil().max(1.0) as u32];
    // v1 plays a one-shot's frame f at t = f / (frames - 1); a loop's at f / frames.
    let span = if looping { frames } else { frames - 1.0 };
    let empty = Vec::new();
    let mut layers = Vec::new();
    for (i, raw) in o.get("layers").and_then(Value::as_array).unwrap_or(&empty).iter().enumerate() {
        let at = |e: String| format!("layers[{i}].{e}");
        let l = raw.as_object().ok_or_else(|| at("a layer is an object".into()))?;
        if let Some(k) = l.keys().find(|k| !LAYER_FIELDS.contains(&k.as_str())) {
            return Err(at(format!("{k}: no v1 layer field")));
        }
        let ty = l.get("type").and_then(Value::as_str).ok_or_else(|| at("type: missing".into()))?;
        let n = |key: &str, d: f32| num(l, key, d).map_err(&at);
        let (t0, t1) = (n("t0", 0.0)?, n("t1", 1.0)?);
        let window = ((t1 - t0) * span).max(1.0);
        let cols: Vec<[u8; 3]> = colours(l.get("ramp"), &own).map_err(&at)?.into_iter().map(|c| turned(c, hue)).collect();
        let ramp = ramps::nearest(&cols);
        if let Some(name) = l.get("ramp").and_then(Value::as_str)
            && name != ramp
        {
            notes.push(at(format!("ramp {name} is the realm's {ramp}")));
        }
        let (x, y) = (n("x", 0.0)? * u, n("y", 0.0)? * u);
        let (r0, r1, th) = (n("r0", 0.08)? * u, n("r1", 0.75)? * u, n("th", 0.12)? * u);
        let (angle, squash, gain) = (n("angle", 0.0)?, n("squash", 1.0)?.clamp(0.05, 4.0), n("gain", 1.0)?.clamp(0.0, 2.0));
        let (speed, spin) = (n("speed", 1.0)?, n("spin", 1.0)?);
        let puffs = ty == "puffs";
        let count = n("count", if puffs { 7.0 } else { 12.0 })?.clamp(1.0, 64.0) as u32;
        let size = n("size", if puffs { 0.18 } else if ty == "text" { 0.06 } else { 0.045 })? * u;
        let life = n("life", 1.0)?.clamp(0.05, 1.0);
        let gravity = n("gravity", 0.5)?;
        let mode = l.get("mode").and_then(Value::as_str).unwrap_or("burst");
        let kind: Kind = serde_json::from_value(Value::String(ty.into())).map_err(|_| at(format!("type {ty:?}: not one of v1's")))?;
        let mut k: Layer = serde_json::from_value(serde_json::json!({ "type": kind })).map_err(|e| at(e.to_string()))?;
        k.ramp = Some(ramp.to_string());
        k.t0 = Some(Time::Frame((t0 * span).round()));
        k.t1 = Some(Time::Frame((t0 * span).round() + window.round().max(1.0) + if looping { 0.0 } else { 1.0 }));
        (k.x, k.y) = (Some(x), Some(y));
        if gain != 1.0 {
            k.gain = Some(gain);
        }
        let rad = (angle.to_radians().cos(), angle.to_radians().sin());
        let turn = |dx: f32, dy: f32| (x + dx * rad.0 - dy * rad.1, y + dx * rad.1 + dy * rad.0);
        // One turn a window, as v1's spin counts them, in degrees a frame.
        let per_frame = (spin * 360.0 / window).clamp(-360.0, 360.0);
        match kind {
            Kind::Burst => (k.r0, k.r1, k.squash) = (Some(r0), Some(r1), Some(squash)),
            Kind::Ring | Kind::Shock => (k.r0, k.r1, k.th, k.squash) = (Some(r0), Some(r1), Some(th), Some(squash)),
            Kind::Flame => {
                k.y = Some(y + 0.9 * u);
                (k.r1, k.size, k.count) = (Some(r1), Some(Range::One(1.7 * u)), Some(5));
                k.speed = Some(Range::One(speed));
            }
            Kind::Beam => {
                let (a, b) = (turn(-r1, 0.0), turn(r1, 0.0));
                (k.x, k.y, k.x1, k.y1, k.th) = (Some(a.0), Some(a.1), Some(b.0), Some(b.1), Some(th));
            }
            Kind::Pillar => (k.y, k.r1, k.th) = (Some(y + r1), Some(2.0 * r1), Some(th)),
            Kind::Slash => {
                notes.push(at("slash: kiln sweeps its crescent round rather than turning a fixed one".into()));
                (k.r1, k.th, k.angle, k.arc, k.squash) = (Some(r1), Some(th * 1.5), Some(180.0 + angle - 90.0), Some(180.0), Some(squash));
            }
            Kind::Orb => (k.r0, k.r1, k.speed) = (Some(r1), Some(r1), Some(Range::One(speed))),
            Kind::Flash => {
                k.t1 = Some(Time::Frame((t0 * span).round() + (window * life).round().max(1.0)));
                (k.r0, k.r1) = (Some(r1 * 0.6), Some(r1));
            }
            Kind::Comet => {
                let h = turn(0.4 * u, 0.0);
                (k.x, k.y, k.r1, k.size, k.angle) = (Some(h.0), Some(h.1), Some(th * 1.2), Some(Range::One(1.4 * u)), Some(angle));
            }
            Kind::Vortex => (k.r1, k.th, k.spin, k.squash) = (Some(r1), Some(th), Some(per_frame), Some(squash)),
            Kind::Rays => (k.count, k.r0, k.r1, k.th, k.angle, k.spin) = (Some(count), Some(0.0), Some(r1), Some(0.6), Some(angle), Some(per_frame)),
            Kind::Glyph => (k.r1, k.count, k.spin, k.squash) = (Some(r1), Some(6), Some(per_frame), Some(squash)),
            Kind::Bolt => {
                let (a, b) = (turn(0.0, -u), turn(0.0, u));
                (k.x, k.y, k.x1, k.y1, k.th) = (Some(a.0), Some(a.1), Some(b.0), Some(b.1), Some(th.max(0.6)));
                (k.jag, k.flicker, k.count) = (Some(0.3 * u), Some((window / 8.0).max(1.0)), Some(2));
            }
            Kind::Cracks => (k.count, k.r1, k.squash) = (Some(count.min(12)), Some(r1), Some(squash)),
            Kind::Spikes => (k.y, k.count, k.r1, k.size, k.th) = (Some(y + 0.5 * u), Some(count.min(16)), Some(u), Some(Range::One(0.9 * u)), Some(u / count as f32)),
            Kind::Star => (k.count, k.r1, k.th, k.spin) = (Some(count), Some(r1), Some(th), Some(per_frame)),
            Kind::Streaks => (k.count, k.r1, k.th, k.angle) = (Some(count.min(12)), Some(1.8 * u), Some(th.max(0.4)), Some(angle)),
            Kind::Bubble => (k.r1, k.th) = (Some(r1), Some(th)),
            Kind::Text => {
                k.text = Some(l.get("text").and_then(Value::as_str).unwrap_or("+100").to_string());
                (k.size, k.speed) = (Some(Range::One(size)), Some(Range::One(speed * u)));
            }
            Kind::Tornado => (k.r1, k.size, k.spin) = (Some(r1), Some(Range::One(1.8 * u)), Some(per_frame)),
            Kind::Particles | Kind::Puffs => {
                if kind == Kind::Particles {
                    k.form = Some(Form::Mote);
                }
                k.count = Some(count);
                k.size = Some(Range::One(if puffs { size } else { size / 0.6 }));
                k.life = Some(Range::One((window * life).max(1.0)));
                k.drag = Some(0.0);
                let travel = speed * u / window;
                let g = 2.0 * gravity * u / (window * window);
                k.gravity = Some(g.clamp(-8.0, 8.0));
                k.speed = Some(Range::One(travel));
                k.spread = Some(360.0);
                k.angle = Some(0.0);
                match mode {
                    "rise" | "fall" => {
                        let up = mode == "rise";
                        (k.y, k.spawn, k.area) = (Some(y + if up { 0.9 } else { -0.9 } * u), Some(Spawn::Line), Some([1.8 * u, 0.0]));
                        (k.angle, k.spread, k.speed) = (Some(if up { -90.0 } else { 90.0 }), Some(0.0), Some(Range::One(2.0 * travel)));
                        k.gravity = None;
                        k.motion = Some(if up { Motion::Rise } else { Motion::Fall });
                    }
                    "ambient" => (k.spawn, k.area, k.speed) = (Some(Spawn::Box), Some([1.8 * u, 1.6 * u]), Some(Range::One(0.0))),
                    "implode" => (k.motion, k.area) = (Some(Motion::Gather), Some([speed * u, speed * u])),
                    "orbit" => (k.motion, k.area, k.spin) = (Some(Motion::Orbit), Some([0.65 * u, 0.65 * u]), Some(per_frame)),
                    "trail" => {
                        (k.x, k.spawn, k.area) = (Some(x + 0.5 * u), Some(Spawn::Box), Some([0.0, 1.6 * u]));
                        (k.angle, k.spread, k.speed) = (Some(180.0), Some(0.0), Some(Range::One(1.8 * travel)));
                    }
                    "swirl" => (k.motion, k.spin) = (Some(Motion::Spiral), Some(per_frame)),
                    _ => {}
                }
                k.ease = Some(Ease::Linear);
            }
        }
        layers.push(k);
    }
    if layers.is_empty() {
        return Err("layers: a v1 recipe has at least one".into());
    }
    let recipe = Recipe {
        fx: super::recipe::VERSION,
        name: o.get("name").and_then(Value::as_str).unwrap_or("effect").to_string(),
        canvas,
        pivot: None,
        fps: Some(num(o, "fps", 15.0)?.clamp(1.0, 60.0) as u32),
        frames: Some(frames as u32),
        looping: looping.then_some(true),
        seed: Some(num(o, "seed", 1.0)? as u32),
        beats: None,
        outline,
        layers,
    };
    recipe.check()?;
    Ok((recipe, notes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A v1 recipe in the shape of the cartridge's frost example.
    const NOVA: &str = r#"{ "name": "Nova", "w": 64, "h": 64, "frames": 18, "fps": 18, "seed": 11, "layers": [
        { "type": "rays", "ramp": "ice", "count": 12, "gain": 0.65, "r1": 0.9 },
        { "type": "ring", "ramp": "frost", "r0": 0.08, "r1": 0.9, "th": 0.08 },
        { "type": "particles", "ramp": "ice", "count": 24, "size": 0.035, "speed": 0.9, "gravity": 0.25 },
        { "type": "flash", "ramp": "ice", "r1": 0.32, "life": 0.15 } ] }"#;

    #[test]
    fn a_v1_recipe_carries_over_in_the_realms_inks() {
        let (r, notes) = from_cartridge_v1(NOVA).unwrap();
        assert_eq!(r.canvas, [16, 16]);
        assert_eq!(r.layers.len(), 4);
        assert_eq!(r.layers[0].kind, Kind::Rays);
        assert!(r.layers.iter().all(|l| l.ramp.as_deref().is_some_and(|n| ramps::named(n).is_some())));
        assert!(notes.iter().any(|n| n.contains("ramp ice")), "{notes:?}");
        let fx = super::super::Fx::new(r).unwrap();
        assert_eq!(fx.frames(), 18);
    }

    #[test]
    fn what_v1_cannot_say_is_refused() {
        assert!(from_cartridge_v1(&NOVA.replace("\"seed\"", "\"sede\"")).is_err());
        assert!(from_cartridge_v1(&NOVA.replace("\"type\": \"rays\"", "\"type\": \"laser\"")).is_err());
    }
}
