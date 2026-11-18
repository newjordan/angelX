//! Knight Racers pod specs. A pod is a knight in a chariot towed by a pair of
//! engines; these numbers are what really changes how it drives. The order and
//! names follow the pod meshes the renderer builds.
//!
//! `top` and `boost` are the speed caps in metres a second (plain and boosting),
//! `accel` scales the thrust, `grip` scales steering and how well the pod holds
//! a bend, and `mass` decides how much a wall, a block, a slam or a shove takes off
//! (and how far it is thrown). `burn` scales what gas and boost put into the engines'
//! heat and `vent` how fast they shed it. `len`, `tail` and `half_w` are the pod's
//! footprint on the road in metres: the engines are `len` long and `half_w` out
//! either side of the centre; `tail` is how far behind the pod's position the
//! chariot's tail trails.
//!
//! The pods are tuned to race each other: the tops sit within a few metres a second, so
//! a pod's edge is how it uses its engines, not how fast it is on paper. A pod that
//! rides another's wake gains more than that spread.

#[derive(Clone, Copy, Debug)]
pub struct PodSpec {
    pub name: &'static str,
    pub top: f32,
    pub boost: f32,
    pub accel: f32,
    pub grip: f32,
    pub mass: f32,
    pub burn: f32,
    pub vent: f32,
    pub len: f32,
    pub tail: f32,
    pub half_w: f32,
}

const PODS: [PodSpec; 6] = [
    PodSpec { name: "WARDEN", top: 46., boost: 58., accel: 1.00, grip: 1.00, mass: 1.00, burn: 1.00, vent: 1.00, len: 4.4, tail: 7.8, half_w: 1.85 },
    PodSpec { name: "BELLFOUNDER", top: 45., boost: 63., accel: 1.02, grip: 0.98, mass: 1.10, burn: 1.15, vent: 0.95, len: 3.8, tail: 7.1, half_w: 2.25 },
    PodSpec { name: "RINGWRIGHT", top: 45., boost: 56., accel: 1.00, grip: 1.18, mass: 1.05, burn: 0.90, vent: 1.20, len: 3.0, tail: 7.5, half_w: 2.8 },
    PodSpec { name: "LANCEWAIN", top: 48., boost: 62., accel: 0.86, grip: 0.90, mass: 0.70, burn: 1.12, vent: 0.90, len: 6.4, tail: 9.5, half_w: 1.3 },
    PodSpec { name: "GATEHOUSE", top: 45., boost: 56., accel: 0.94, grip: 1.08, mass: 1.40, burn: 0.85, vent: 0.90, len: 3.8, tail: 7.0, half_w: 2.25 },
    PodSpec { name: "WYVERN", top: 46., boost: 57., accel: 1.25, grip: 1.02, mass: 0.80, burn: 1.00, vent: 1.10, len: 4.6, tail: 8.0, half_w: 1.9 },
];

pub fn pods() -> &'static [PodSpec] { &PODS }

pub fn seat_pod(id: u32) -> u8 { ((id.max(1) - 1) as usize % PODS.len()) as u8 }
