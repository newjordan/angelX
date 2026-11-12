//! The racer's law, engine side. Each pod is towed by two nacelles and each has its own
//! heat and its own wear:
//!
//! * gas heats both a little and a boost heats them a lot; a draft eases them;
//! * lifting off the gas cools them and the squires patch the wear while the pod coasts;
//! * a wall, a slag block, a jet of flame or a hard slam dumps heat (and wear) into the
//!   engine nearest to it;
//! * a hot engine limps: less push, a lower top speed and a pull toward its own side
//!   (the strong engine shoves the pod round it);
//! * an engine at full heat blows: the pod skids to a stop and sits out a few seconds
//!   while its squires patch it, then goes again on cooled engines.
//!
//! Std only; `race.rs` calls these once a tick for each pod.
use super::{knob, Car, HZ};

/// An engine's heat past this is on fire: the pod limps hard and the gauge goes red.
pub const FIRE_HEAT: f32 = 0.75;
/// Where the gauge turns amber and smoke starts to show.
pub const WARN_HEAT: f32 = 0.5;
/// Full heat: the engine blows.
pub const BLOW_HEAT: f32 = 1.;
/// Heat at which an engine starts to give up push.
const LIMP_FROM: f32 = 0.6;
/// What an engine gives at full heat, as a share of its push.
const LIMP_FLOOR: f32 = 0.55;
/// Ticks a blown pod is out: the skid and then the squires' patch.
pub const BLOWOUT_TICKS: u32 = 105;
/// How long the skid takes of those: the pod is still while the squires work.
pub const SKID_TICKS: u32 = 35;
/// What the nacelles hold after a patch.
const PATCHED_HEAT: f32 = 0.35;
const PATCHED_WEAR: f32 = 0.12;

/// Heat a second that flat-out gas puts in, net of the airflow over the engine.
pub const HEAT_GAS: f32 = 0.0135;
/// What a boost adds to that, a second.
pub const HEAT_BOOST: f32 = 0.085;
/// What an engine sheds a second with the gas off, and with the brake on.
pub const COOL_COAST: f32 = 0.20;
pub const COOL_BRAKE: f32 = 0.11;
/// Wear the squires take out a second while the pod is off the gas, and while it sits blown.
pub const PATCH_COAST: f32 = 0.030;
pub const PATCH_SITTING: f32 = 0.060;
/// The most wear an engine carries: past this only a blowout's full patch clears it.
const WEAR_CAP: f32 = 0.55;
/// Lateral drift, metres a second, at a full pull.
const PULL_DRIFT: f32 = 9.;

/// What an engine at `heat` gives, as a share of its push: 1 cool, `LIMP_FLOOR` blown.
pub fn output_at(heat: f32) -> f32 {
    let u = ((heat - LIMP_FROM) / (BLOW_HEAT - LIMP_FROM)).clamp(0., 1.);
    1. - (1. - LIMP_FLOOR) * u * u * (3. - 2. * u)
}

impl Car {
    /// The hotter nacelle, for the gauges, the pilots and the fire cap.
    pub fn hottest(&self) -> f32 { self.heat[0].max(self.heat[1]) }
    /// An engine is on fire: the pod limps until it is cooled.
    pub fn burning(&self) -> bool { self.hottest() >= FIRE_HEAT }
    /// Sitting out a blowout.
    pub fn blown(&self) -> bool { self.crash > 0 }
    /// The share of full push the two engines give together: 1 on cool engines.
    pub fn push(&self) -> f32 { (output_at(self.heat[0]) + output_at(self.heat[1])) * 0.5 }
    /// Which way a limp drags the pod across the road, -1 to +1: toward the side of the weaker
    /// engine, positive toward greater `y`. Zero on two matched engines.
    pub fn pull(&self) -> f32 { (output_at(self.heat[0]) - output_at(self.heat[1])) / (1. - LIMP_FLOOR) }
    /// Metres a second the limp drags the pod sideways (toward greater `y` when positive).
    pub(super) fn pull_drift(&self, v: f32) -> f32 { self.pull() * PULL_DRIFT * (v / 30.).min(1.) }
    /// The top speed a limping pair allows, as a share of the pod's own.
    pub fn cap_share(&self) -> f32 { self.push().powf(0.75) }
}

/// What the pod is doing to its engines this tick.
pub(super) struct Load { pub gas: bool, pub brake: bool, pub boosting: bool, pub flying: bool }

/// One tick of heat and patching for both nacelles.
pub(super) fn heat_step(c: &mut Car, load: &Load) {
    let dt = 1. / HZ;
    let (burn, vent) = { let s = c.spec(); (s.burn, s.vent) };
    let working = load.gas && !load.flying;
    for i in 0..2 {
        let mut dh = 0.;
        if working { dh += knob("HEAT_GAS", HEAT_GAS) * burn * (1. - 0.5 * c.draft) * (1. + c.wear[i]); }
        if load.boosting { dh += knob("HEAT_BOOST", HEAT_BOOST) * burn; }
        if !working { dh -= if load.brake { knob("COOL_BRAKE", COOL_BRAKE) } else { knob("COOL_COAST", COOL_COAST) } * vent; }
        c.heat[i] = (c.heat[i] + dh * dt).clamp(c.wear[i], BLOW_HEAT);
        // Off the gas the squires patch as the pod runs.
        if !working { c.wear[i] = (c.wear[i] - PATCH_COAST * dt).max(0.); }
    }
}

/// Dump heat and wear into one nacelle (`Some(0)` or `Some(1)`) or both (`None`). Returns whether
/// a blowout is now due.
pub(super) fn dump(c: &mut Car, side: Option<usize>, heat: f32, wear: f32) {
    for i in 0..2 {
        if side.is_some_and(|s| s != i) { continue; }
        c.wear[i] = (c.wear[i] + wear).min(WEAR_CAP);
        c.heat[i] = (c.heat[i] + heat).clamp(c.wear[i], BLOW_HEAT);
    }
}

/// The nacelle that a thing at lateral position `y_other` is nearest to, for a pod at `y`.
/// Nacelle 0 is the pod's lesser-`y` side (the left of the picture), nacelle 1 the greater.
pub(super) fn near(y: f32, y_other: f32) -> usize { if y_other >= y { 1 } else { 0 } }

/// Blows an engine that has reached full heat: the pod is out for `BLOWOUT_TICKS`.
/// Returns whether it just blew.
pub(super) fn blow_if_due(c: &mut Car) -> bool {
    if c.crash > 0 || c.heat[0] < BLOW_HEAT && c.heat[1] < BLOW_HEAT { return false; }
    let hot = if c.heat[0] >= c.heat[1] { 0 } else { 1 };
    c.crash = BLOWOUT_TICKS;
    c.blown_side = hot as u8 + 1;
    c.blowouts += 1;
    c.boosting = false;
    c.draft = 0.;
    c.impact = 11;
    true
}

/// Dropped into a chasm and hauled back for another run: the squires cool the engines on the way, so
/// a pod that came to the leap hot (and fell for it) is not sent at it hot again, for ever. Without
/// this a slow, hot pod (a Squire-class Bellfounder on Moatwalk) cooked its engines on the run-up, limped at
/// the lip, fell, and was set down as hot as it fell, 193 times over, so the race never finished.
pub(super) fn haul_back(c: &mut Car) {
    c.heat = [c.heat[0].min(PATCHED_HEAT), c.heat[1].min(PATCHED_HEAT)];
}

/// A blown pod's tick: it skids on its own momentum, then sits while the squires work, and goes
/// again with the engines cooled and patched. Returns whether the tick was spent sitting out.
pub(super) fn sit_out(c: &mut Car) -> bool {
    if c.crash == 0 { return false; }
    let dt = 1. / HZ;
    c.crash -= 1;
    let skid = c.crash + SKID_TICKS >= BLOWOUT_TICKS;
    // The skid: the pod's momentum runs out; after it the pod does not move.
    // Over a leap the pod keeps its momentum: the skid starts at the landing.
    if !c.flying { c.v = if skid { (c.v - 38. * dt).max(0.) } else { 0. }; }
    c.vy = 0.;
    c.boosting = false;
    c.draft = 0.;
    // The squires patch the engine that blew, a little each tick, and then the rest at once.
    for i in 0..2 { c.wear[i] = (c.wear[i] - PATCH_SITTING * dt).max(0.); }
    if c.crash == 0 {
        c.heat = [c.heat[0].min(PATCHED_HEAT), c.heat[1].min(PATCHED_HEAT)];
        let b = usize::from(c.blown_side.max(1) - 1);
        c.heat[b] = PATCHED_HEAT;
        c.wear[b] = PATCHED_WEAR;
        c.blown_side = 0;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hot_engine_gives_less_and_a_blown_one_least() {
        assert_eq!(output_at(0.), 1.);
        assert_eq!(output_at(LIMP_FROM), 1.);
        assert!(output_at(0.75) < 0.95 && output_at(0.75) > 0.75);
        assert!((output_at(BLOW_HEAT) - LIMP_FLOOR).abs() < 1e-6);
        let mut last = 1.;
        for n in 0..=100 { let o = output_at(n as f32 / 100.); assert!(o <= last + 1e-6, "output must only fall as heat rises"); last = o; }
    }
}
