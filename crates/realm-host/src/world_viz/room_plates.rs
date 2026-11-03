//! Preserved room/location plates test verification snapshots.

#[cfg(any(test, feature = "testing"))]
use super::{Building, World};
#[cfg(any(test, feature = "testing"))]
use image::RgbaImage;
#[cfg(any(test, feature = "testing"))]
use std::sync::OnceLock;

#[cfg(any(test, feature = "testing"))]
pub(super) const WIDTH: u32 = 256;
#[cfg(any(test, feature = "testing"))]
pub(super) const HEIGHT: u32 = 224;

#[cfg(any(test, feature = "testing"))]
#[derive(Clone, Copy)]
struct Snapshot {
    target: Building,
    ambient_detail: bool,
    ambient_pose: super::ambient::Pose,
}

#[cfg(any(test, feature = "testing"))]
pub struct Frame {
    pub pixels: Pixels,
    pub width: u32,
    pub height: u32,
}

#[cfg(any(test, feature = "testing"))]
pub struct Pixels {
    snapshot: Snapshot,
    composed: OnceLock<Vec<u8>>,
}

#[cfg(any(test, feature = "testing"))]
impl AsRef<[u8]> for Pixels {
    fn as_ref(&self) -> &[u8] {
        self.composed
            .get_or_init(|| compose(self.snapshot).into_raw())
    }
}

#[cfg(any(test, feature = "testing"))]
impl World {
    pub fn ambient_frame(&self, motion: crate::telemetry::MotionMode) -> Frame {
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
    pub fn ambient_detail_frame(
        &self,
        motion: crate::telemetry::MotionMode,
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

#[cfg(any(test, feature = "testing"))]
fn compose(snapshot: Snapshot) -> RgbaImage {
    if snapshot.ambient_detail {
        super::ambient::detail_frame(snapshot.target, snapshot.ambient_pose)
    } else {
        super::ambient::frame(snapshot.target, snapshot.ambient_pose)
    }
}
