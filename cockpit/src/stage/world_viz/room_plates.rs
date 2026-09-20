//! Preserved room/location plates test verification snapshots.

#[cfg(test)]
use super::{Building, World};
#[cfg(test)]
use image::RgbaImage;
#[cfg(test)]
use std::sync::OnceLock;

#[cfg(test)]
pub(super) const WIDTH: u32 = 256;
#[cfg(test)]
pub(super) const HEIGHT: u32 = 224;

#[cfg(test)]
#[derive(Clone, Copy)]
struct Snapshot {
    target: Building,
    ambient_detail: bool,
    ambient_pose: super::ambient::Pose,
}

#[cfg(test)]
pub(crate) struct Frame {
    pub(crate) pixels: Pixels,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[cfg(test)]
pub(crate) struct Pixels {
    snapshot: Snapshot,
    composed: OnceLock<Vec<u8>>,
}

#[cfg(test)]
impl AsRef<[u8]> for Pixels {
    fn as_ref(&self) -> &[u8] {
        self.composed
            .get_or_init(|| compose(self.snapshot).into_raw())
    }
}

#[cfg(test)]
impl World {
    pub(crate) fn ambient_frame(&self, motion: crate::ui::viz::lifecycle_viz::MotionMode) -> Frame {
        Frame {
            pixels: Pixels {
                snapshot: Snapshot {
                    target: self.ambient_building(),
                    ambient_detail: false,
                    ambient_pose: self.ambient_pose(motion),
                },
                composed: OnceLock::new(),
            },
            width: WIDTH,
            height: HEIGHT,
        }
    }

    /// The same entered painting and pose, sampled from its approved source
    /// for native image protocols. Ordinary terminal fallback keeps its plate.
    pub(crate) fn ambient_detail_frame(
        &self,
        motion: crate::ui::viz::lifecycle_viz::MotionMode,
    ) -> Frame {
        Frame {
            pixels: Pixels {
                snapshot: Snapshot {
                    target: self.ambient_building(),
                    ambient_detail: true,
                    ambient_pose: self.ambient_pose(motion),
                },
                composed: OnceLock::new(),
            },
            width: super::ambient::DETAIL_WIDTH,
            height: super::ambient::DETAIL_HEIGHT,
        }
    }
}

#[cfg(test)]
fn compose(snapshot: Snapshot) -> RgbaImage {
    if snapshot.ambient_detail {
        super::ambient::detail_frame(snapshot.target, snapshot.ambient_pose)
    } else {
        super::ambient::frame(snapshot.target, snapshot.ambient_pose)
    }
}
