//! The ball's, the cart's and the griffin's tick, asked of the Bend module `race3d-rules`.
//!
//! The race goes over as the saves and the wire hold it, with the keys held,
//! who stands, the course table and the one sine the sluice reads (a libm sine
//! is not something the leg can reproduce bit for bit). The module's adapter
//! does the rest of the marshalling. A refused or missing answer is an error
//! the caller surfaces: the race never guesses a native one.
use super::*;

/// The live module's id.
pub const LEG: &str = "race3d-rules";

/// The sounds a tick may ask for, by the names the room plays them under.
const SOUNDS: [&str; 12] = [
    "roll", "clang", "gold", "knight_fall", "helmet_pop", "rock_land", "wheel_land", "step_wet", "wheel_spin", "swing",
    "shot_bolt", "hit_arrow",
];

#[derive(Serialize)]
struct Ask<'a> {
    op: &'static str,
    race: &'a Race3d,
    inputs: BTreeMap<u32, Input>,
    standing: &'a [u32],
    track: &'a Track,
    sine: BTreeMap<u32, f32>,
}

/// The next field, and what the tick did.
#[derive(Deserialize)]
struct Answer {
    riders: BTreeMap<u32, Rider>,
    taken: BTreeSet<u16>,
    planks: BTreeMap<u16, u32>,
    bolts: Vec<Bolt>,
    sounds: Vec<String>,
    all_home: bool,
    timed_out: bool,
}

/// What a tick did that the caller acts on.
pub struct Ran {
    pub sounds: Vec<&'static str>,
    pub all_home: bool,
    pub timed_out: bool,
}

/// One tick of the Shaft Run, the Ore Cart or the Griffin Run: the field and its keys in, the next field out.
pub fn run(race: &mut Race3d, inputs: &BTreeMap<u32, Input>, standing: &[u32]) -> Result<Ran, String> {
    #[cfg(test)]
    super::install_oracle();
    let ask = Ask {
        op: "tick",
        race,
        inputs: inputs.iter().filter(|(_, i)| i.valid()).map(|(&id, &i)| (id, i)).collect(),
        standing,
        track: track(race.course),
        sine: race.riders.iter().map(|(&id, r)| (id, (r.s * 0.35).sin())).collect(),
    };
    let out: Answer = crate::scripted::step(LEG, &ask)?;
    let next = Race3d { riders: out.riders, taken: out.taken, planks: out.planks, bolts: out.bolts, ..race.clone() };
    if !next.valid() || !next.riders.keys().eq(race.riders.keys()) {
        return Err(format!("{LEG}: the next field does not hold the race"));
    }
    let sounds = out.sounds.iter().map(|name| SOUNDS.into_iter().find(|s| s == name));
    let sounds = sounds.collect::<Option<_>>().ok_or_else(|| format!("{LEG}: a sound the room has no name for"))?;
    *race = next;
    Ok(Ran { sounds, all_home: out.all_home, timed_out: out.timed_out })
}
