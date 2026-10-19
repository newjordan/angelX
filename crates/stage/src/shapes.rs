//! The built-in meshes by name, for text that names a shape: a set file
//! (a blockout of a room or a stretch of the kingdom, docs/WORLD-3D.md) and
//! any tool that writes one. A name is lower case; [`by_name`] answers the
//! mesh it stands for, sized like the others (a unit across, centred), and
//! the node's scale makes it the size it needs to be.
//!
//! A shape with a parameter takes it as a `-<n>` suffix on its name:
//! `rock-7` (the seed), `blob-3` (the seed), `stairs-6` (the steps),
//! `fence-5` (the posts), `cylinder-24`, `frustum-8`, `arch-12`, `sphere-24`
//! and `cone-8` (the sides). The bare name is the shape's default.

use crate::primitives::{FENCE_POSTS, RING_SEGMENTS, STAIR_STEPS};
use crate::scene::{MeshRef, SPHERE_SEGMENTS};

/// Every name [`by_name`] knows, one per shape (aliases are left out).
pub const NAMES: &[&str] = &[
    "quad",
    "box",
    "plane",
    "sphere",
    "gem",
    "cone",
    "cylinder",
    "frustum",
    "wedge",
    "gable",
    "hip",
    "pyramid",
    "arch",
    "stairs",
    "rock",
    "blob",
    "fence",
];

/// The built-in mesh a name stands for; `None` for a name it does not know.
/// A name with a parameter may carry it as a suffix (`rock-7`, the module
/// doc lists them): digits only, clamped the way the shape clamps its own
/// parameter, and a seed that does not fit 16 bits is refused. A shape with
/// no parameter (`box`) takes none, and an empty or non-numeric suffix
/// (`rock-`, `rock-x`) is refused.
pub fn by_name(name: &str) -> Option<MeshRef> {
    if let Some((base, digits)) = name.split_once('-') {
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        // Past the u32 range a number is only a very large one: it clamps.
        let n = digits.parse::<u32>().unwrap_or(u32::MAX);
        let sides = |range: &std::ops::RangeInclusive<u16>| n.clamp(u32::from(*range.start()), u32::from(*range.end())) as u16;
        return match base {
            "sphere" => Some(MeshRef::Sphere { segments: sides(&SPHERE_SEGMENTS) }),
            "cone" => Some(MeshRef::Cone { segments: sides(&SPHERE_SEGMENTS) }),
            "cylinder" => Some(MeshRef::Cylinder { segments: sides(&RING_SEGMENTS) }),
            "frustum" => Some(MeshRef::Frustum { segments: sides(&RING_SEGMENTS) }),
            "arch" => Some(MeshRef::Arch { segments: sides(&RING_SEGMENTS) }),
            "stairs" => Some(MeshRef::Stairs { steps: sides(&STAIR_STEPS) }),
            "fence" => Some(MeshRef::Fence { posts: sides(&FENCE_POSTS) }),
            "rock" => u16::try_from(n).ok().map(|seed| MeshRef::Rock { seed }),
            "blob" => u16::try_from(n).ok().map(|seed| MeshRef::Blob { seed }),
            _ => None,
        };
    }
    match name {
        "quad" => Some(MeshRef::Quad),
        "box" | "cube" => Some(MeshRef::Cube),
        "plane" => Some(MeshRef::Plane),
        "sphere" => Some(MeshRef::Sphere { segments: 16 }),
        "gem" => Some(MeshRef::Gem),
        "cone" => Some(MeshRef::Cone { segments: 12 }),
        "cylinder" => Some(MeshRef::Cylinder { segments: 12 }),
        "frustum" => Some(MeshRef::Frustum { segments: 12 }),
        "wedge" => Some(MeshRef::Wedge),
        "gable" => Some(MeshRef::Gable),
        "hip" => Some(MeshRef::Hip),
        "pyramid" => Some(MeshRef::Pyramid),
        "arch" => Some(MeshRef::Arch { segments: 8 }),
        "stairs" => Some(MeshRef::Stairs { steps: 4 }),
        "rock" => Some(MeshRef::Rock { seed: 1 }),
        "blob" => Some(MeshRef::Blob { seed: 1 }),
        "fence" => Some(MeshRef::Fence { posts: 4 }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_is_a_built_in_that_generates_valid_data() {
        for name in NAMES {
            let mesh = by_name(name).unwrap_or_else(|| panic!("{name} is not known"));
            let data = mesh.generate().unwrap_or_else(|| panic!("{name} is not built in"));
            assert_eq!(data.validate(), Ok(()), "{name}");
        }
    }

    #[test]
    fn an_unknown_name_is_none() {
        assert_eq!(by_name("castle"), None);
        assert_eq!(by_name("Box"), None);
    }

    #[test]
    fn a_suffix_sets_the_parameter() {
        assert_eq!(by_name("rock-7"), Some(MeshRef::Rock { seed: 7 }));
        assert_eq!(by_name("rock-0"), Some(MeshRef::Rock { seed: 0 }));
        assert_eq!(by_name("rock-65535"), Some(MeshRef::Rock { seed: 65535 }));
        assert_eq!(by_name("blob-3"), Some(MeshRef::Blob { seed: 3 }));
        assert_eq!(by_name("stairs-6"), Some(MeshRef::Stairs { steps: 6 }));
        assert_eq!(by_name("fence-5"), Some(MeshRef::Fence { posts: 5 }));
        assert_eq!(by_name("cylinder-24"), Some(MeshRef::Cylinder { segments: 24 }));
        assert_eq!(by_name("frustum-8"), Some(MeshRef::Frustum { segments: 8 }));
        assert_eq!(by_name("arch-12"), Some(MeshRef::Arch { segments: 12 }));
        assert_eq!(by_name("sphere-24"), Some(MeshRef::Sphere { segments: 24 }));
        assert_eq!(by_name("cone-8"), Some(MeshRef::Cone { segments: 8 }));
        // A different seed is a different rock and a different blob.
        assert_ne!(by_name("rock-7").unwrap().generate(), by_name("rock-8").unwrap().generate());
        assert_ne!(by_name("blob-3").unwrap().generate(), by_name("blob-4").unwrap().generate());
        // The bare name keeps its default.
        assert_eq!(by_name("rock"), Some(MeshRef::Rock { seed: 1 }));
        assert_eq!(by_name("stairs"), Some(MeshRef::Stairs { steps: 4 }));
    }

    #[test]
    fn a_suffix_clamps_like_the_shape_does() {
        assert_eq!(by_name("cylinder-1"), Some(MeshRef::Cylinder { segments: 3 }));
        assert_eq!(by_name("cylinder-999"), Some(MeshRef::Cylinder { segments: 64 }));
        assert_eq!(by_name("frustum-0"), Some(MeshRef::Frustum { segments: 3 }));
        assert_eq!(by_name("arch-100"), Some(MeshRef::Arch { segments: 64 }));
        assert_eq!(by_name("sphere-2"), Some(MeshRef::Sphere { segments: 3 }));
        assert_eq!(by_name("sphere-500"), Some(MeshRef::Sphere { segments: 128 }));
        assert_eq!(by_name("cone-99999999999"), Some(MeshRef::Cone { segments: 128 }));
        assert_eq!(by_name("stairs-0"), Some(MeshRef::Stairs { steps: 2 }));
        assert_eq!(by_name("stairs-40"), Some(MeshRef::Stairs { steps: 16 }));
        assert_eq!(by_name("fence-1"), Some(MeshRef::Fence { posts: 2 }));
        assert_eq!(by_name("fence-20"), Some(MeshRef::Fence { posts: 8 }));
        // A seed is not clamped, so one that does not fit is refused.
        assert_eq!(by_name("rock-65536"), None);
        assert_eq!(by_name("blob-99999999999"), None);
    }

    #[test]
    fn a_bad_suffix_is_refused() {
        for name in ["rock-", "rock-x", "rock-7x", "rock--7", "rock-+7", "rock- 7", "rock-7-8", "blob-", "stairs-x", "arch-1.5", "-7", "-"] {
            assert_eq!(by_name(name), None, "{name}");
        }
        // A shape with no parameter takes none, and neither does an alias.
        for name in ["box-3", "cube-3", "quad-1", "plane-2", "gem-4", "wedge-2", "gable-2", "hip-2", "pyramid-4", "castle-3"] {
            assert_eq!(by_name(name), None, "{name}");
        }
    }

    #[test]
    fn every_suffixed_shape_generates_valid_data() {
        for name in ["rock-7", "blob-3", "stairs-6", "fence-5", "cylinder-24", "frustum-8", "arch-12", "sphere-24", "cone-8", "cylinder-999", "stairs-0"] {
            let data = by_name(name).unwrap().generate().unwrap_or_else(|| panic!("{name} is not built in"));
            assert_eq!(data.validate(), Ok(()), "{name}");
        }
    }
}
