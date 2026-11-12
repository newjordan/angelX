//! What sits on top of a pod's stock numbers: the parts a knight has fitted in
//! the garage, and the class a rival races in. One small bundle of multiples
//! that every car carries (`Car::tune`), so the sim reads one tuned spec and
//! never has to know where the change came from.
use super::pods::PodSpec;
use serde::{Deserialize, Serialize};

/// Multiples of a pod's spec: 1.0 is stock.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tune {
    pub top: f32,
    pub boost: f32,
    pub accel: f32,
    pub grip: f32,
    pub mass: f32,
}

impl Default for Tune {
    fn default() -> Tune { Tune::STOCK }
}

/// How far a multiple may stray from stock, either way: a bad save cannot make a pod that
/// breaks the sim's limits (the speed cap, the steering).
const LO: f32 = 0.7;
const HI: f32 = 1.3;

impl Tune {
    pub const STOCK: Tune = Tune { top: 1., boost: 1., accel: 1., grip: 1., mass: 1. };

    pub fn is_stock(&self) -> bool { *self == Tune::STOCK }

    /// The same, every number held to what a pod can take.
    pub fn held(self) -> Tune {
        let h = |v: f32| if v.is_finite() { v.clamp(LO, HI) } else { 1. };
        Tune { top: h(self.top), boost: h(self.boost), accel: h(self.accel), grip: h(self.grip), mass: h(self.mass) }
    }

    /// One tune on top of another.
    pub fn then(self, o: Tune) -> Tune {
        Tune { top: self.top * o.top, boost: self.boost * o.boost, accel: self.accel * o.accel, grip: self.grip * o.grip, mass: self.mass * o.mass }.held()
    }
}

impl PodSpec {
    /// This spec with a tune on it: the numbers the sim drives by.
    pub fn tuned(&self, t: &Tune) -> PodSpec {
        if t.is_stock() { return *self; }
        let t = t.held();
        PodSpec { top: self.top * t.top, boost: self.boost * t.boost, accel: self.accel * t.accel, grip: self.grip * t.grip, mass: self.mass * t.mass, ..*self }
    }
}
