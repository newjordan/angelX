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

pub const FIRE_HEAT: f32 = 0.75;
pub const WARN_HEAT: f32 = 0.5;
pub const BLOW_HEAT: f32 = 1.;
const LIMP_FROM: f32 = 0.6;
const LIMP_FLOOR: f32 = 0.55;
pub const BLOWOUT_TICKS: u32 = 105;
pub const SKID_TICKS: u32 = 35;
const PATCHED_HEAT: f32 = 0.35;
const PATCHED_WEAR: f32 = 0.12;

pub const HEAT_GAS: f32 = 0.0135;
pub const HEAT_BOOST: f32 = 0.085;
pub const COOL_COAST: f32 = 0.20;
pub const COOL_BRAKE: f32 = 0.11;
pub const PATCH_COAST: f32 = 0.030;
pub const PATCH_SITTING: f32 = 0.060;
const WEAR_CAP: f32 = 0.55;
const PULL_DRIFT: f32 = 9.;

pub fn output_at(heat: f32) -> f32 {
    let u = ((heat - LIMP_FROM) / (BLOW_HEAT - LIMP_FROM)).clamp(0., 1.);
    1. - (1. - LIMP_FLOOR) * u * u * (3. - 2. * u)
}

impl Car {
    pub fn hottest(&self) -> f32 { self.heat[0].max(self.heat[1]) }
    pub fn burning(&self) -> bool { self.hottest() >= FIRE_HEAT }
    pub fn blown(&self) -> bool { self.crash > 0 }
    pub fn push(&self) -> f32 { (output_at(self.heat[0]) + output_at(self.heat[1])) * 0.5 }
    pub fn pull(&self) -> f32 { (output_at(self.heat[0]) - output_at(self.heat[1])) / (1. - LIMP_FLOOR) }
    pub(super) fn pull_drift(&self, v: f32) -> f32 { self.pull() * PULL_DRIFT * (v / 30.).min(1.) }
    pub fn cap_share(&self) -> f32 { self.push().powf(0.75) }
}

pub(super) struct Load { pub gas: bool, pub brake: bool, pub boosting: bool, pub flying: bool }

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
        if !working { c.wear[i] = (c.wear[i] - PATCH_COAST * dt).max(0.); }
    }
}

pub(super) fn dump(c: &mut Car, side: Option<usize>, heat: f32, wear: f32) {
    for i in 0..2 {
        if side.is_some_and(|s| s != i) { continue; }
        c.wear[i] = (c.wear[i] + wear).min(WEAR_CAP);
        c.heat[i] = (c.heat[i] + heat).clamp(c.wear[i], BLOW_HEAT);
    }
}

pub(super) fn near(y: f32, y_other: f32) -> usize { if y_other >= y { 1 } else { 0 } }

pub(super) fn blow_if_due(c: &mut Car) -> bool {
    if c.crash > 0 || c.heat[0] < BLOW_HEAT && c.heat[1] < BLOW_HEAT { return false; }
    let hot = if c.heat[0] >= c.heat[1] { 0 } else { 1 };
    c.crash = BLOWOUT_TICKS;
    c.blown_side = hot as u8 + 1;
    c.blowouts += 1;
    c.scorch = Some((c.d, c.y, c.route));
    c.boosting = false;
    c.draft = 0.;
    c.impact = 11;
    true
}

pub(super) fn haul_back(c: &mut Car) {
    c.heat = [c.heat[0].min(PATCHED_HEAT), c.heat[1].min(PATCHED_HEAT)];
}

pub(super) fn sit_out(c: &mut Car) -> bool {
    if c.crash == 0 { return false; }
    let dt = 1. / HZ;
    c.crash -= 1;
    let skid = c.crash + SKID_TICKS >= BLOWOUT_TICKS;
    if !c.flying { c.v = if skid { (c.v - 38. * dt).max(0.) } else { 0. }; }
    c.vy = 0.;
    c.boosting = false;
    c.draft = 0.;
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
