//! How she holds her head: a turn about the world's axes, shared between
//! the chest, the neck and the head joints (the head turning most), made
//! into each joint's own rotation whatever way the rig's bones point.

use angel_stage::scene::{QUAT_IDENTITY, Transform, quat_axis_angle, quat_mul, quat_normalize};

use crate::model::{Goddess, local_turn};

/// Her head's attitude, radians: `yaw` turns toward her left (+), `pitch`
/// tips her face down (+), `roll` tilts her head toward her left shoulder
/// (+). `rise` lifts her whole, meters (a breath, a float). `sway` swings
/// her hair: toward her left (+) and back (+), radians at its root.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeadPose {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub rise: f32,
    pub sway: [f32; 2],
}

/// How much of the turn each joint takes, chest to head.
const SHARES: [f32; 3] = [0.15, 0.35, 0.5];

impl Goddess {
    /// Every joint's local transform for `pose`: rest, with the chest, neck
    /// and head turned and the root lifted.
    pub fn posed(&self, pose: &HeadPose) -> Vec<Option<Transform>> {
        let mut out: Vec<Option<Transform>> = self
            .joints
            .iter()
            .map(|j| j.as_ref().map(|j| j.rest))
            .collect();
        // The whole turn about the world axes, as she faces +Z: yaw about
        // +Y, pitch about +X (her face down), roll about +Z.
        let turn_by = |share: f32| {
            let y = quat_axis_angle([0.0, 1.0, 0.0], pose.yaw * share);
            let x = quat_axis_angle([1.0, 0.0, 0.0], pose.pitch * share);
            let z = quat_axis_angle([0.0, 0.0, -1.0], pose.roll * share);
            quat_normalize(quat_mul(y, quat_mul(x, z)))
        };
        let mut carried = QUAT_IDENTITY;
        for (joint, share) in [self.chest, self.neck, self.head].into_iter().zip(SHARES) {
            let Some(j) = self.joints[joint].as_ref() else {
                continue;
            };
            let parent_world = j
                .parent
                .map_or(QUAT_IDENTITY, |p| self.rest_world_rotation(p));
            let turn = turn_by(share);
            if let Some(t) = out[joint].as_mut() {
                t.rotation = local_turn(j.rest.rotation, parent_world, carried, turn);
            }
            carried = quat_normalize(quat_mul(turn, carried));
        }
        // Her hair swings, each joint down the tail a little more.
        for (k, &joint) in self.ponytail.iter().enumerate() {
            let Some(j) = self.joints[joint].as_ref() else {
                continue;
            };
            let parent_world = j
                .parent
                .map_or(QUAT_IDENTITY, |p| self.rest_world_rotation(p));
            let share = 0.5 + 0.25 * k as f32;
            let turn = quat_normalize(quat_mul(
                quat_axis_angle([0.0, 0.0, -1.0], pose.sway[0] * share),
                quat_axis_angle([1.0, 0.0, 0.0], -pose.sway[1] * share),
            ));
            if let Some(t) = out[joint].as_mut() {
                t.rotation = local_turn(j.rest.rotation, parent_world, carried, turn);
            }
            carried = quat_normalize(quat_mul(turn, carried));
        }
        if pose.rise != 0.0
            && let Some(root) = self
                .joints
                .iter()
                .position(|j| j.as_ref().is_some_and(|j| j.parent.is_none()))
            && let Some(t) = out[root].as_mut()
        {
            t.translation[1] += pose.rise;
        }
        out
    }

    /// A joint's world rotation at rest: its chain's rotations composed.
    pub fn rest_world_rotation(&self, mut joint: usize) -> [f32; 4] {
        let mut chain = Vec::new();
        while let Some(j) = self.joints.get(joint).and_then(|j| j.as_ref()) {
            chain.push(j.rest.rotation);
            match j.parent {
                Some(p) if chain.len() < 256 => joint = p,
                _ => break,
            }
        }
        chain.iter().rev().fold(QUAT_IDENTITY, |acc, &q| {
            quat_normalize(quat_mul(acc, quat_normalize(q)))
        })
    }
}
