//! Living paintings follow real tool destinations. Only authored material
//! regions animate; the camera, architecture and activity state remain fixed.
//! Each trusted, embedded 256×224 plate is decoded once. The eight-entry cache
//! retains at most 1.75 MiB of RGBA pixels. The resident image worker prepares
//! graphics or ordinary-terminal half blocks; memoized braille stays visible
//! for the current destination while its image is being prepared.
//! Native graphics additionally cache the eight original paintings at 768×672
//! (15.75 MiB). Their motion uses the same admitted material masks and clock.
use std::sync::OnceLock;

use image::RgbaImage;

use super::{Building, World};
use crate::ui::viz::lifecycle_viz::MotionMode;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 224;
#[cfg_attr(not(test), allow(dead_code))]
pub(super) const DETAIL_WIDTH: u32 = WIDTH * 3;
#[cfg_attr(not(test), allow(dead_code))]
pub(super) const DETAIL_HEIGHT: u32 = HEIGHT * 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(super) struct Pose {
    phase: u8,
    mode: u8,
}

impl Pose {
    pub(super) fn at(tick: u64, motion: MotionMode) -> Self {
        match motion {
            MotionMode::Off => Self::default(),
            MotionMode::Reduced => Self {
                phase: ((tick / 80) % 8) as u8,
                mode: 1,
            },
            MotionMode::Full => Self {
                phase: ((tick / 10) % 16) as u8,
                mode: 2,
            },
        }
    }
}

#[derive(serde::Deserialize)]
struct Matrix {
    locations: Vec<Location>,
}
#[derive(serde::Deserialize)]
struct Location {
    id: String,
    effects: Vec<Effect>,
}
#[derive(serde::Deserialize)]
pub(super) struct Effect {
    kind: Kind,
    rect: [u32; 4],
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Flame,
    Water,
}

fn matrix() -> &'static Matrix {
    static MATRIX: OnceLock<Matrix> = OnceLock::new();
    MATRIX.get_or_init(|| {
        let matrix: Matrix =
            serde_json::from_str(include_str!("../../../assets/realm/ambient/motion.json"))
                .expect("embedded ambient matrix is valid");
        let ids = [
            "keep",
            "gatehouse",
            "rookery",
            "scriptorium",
            "smithy",
            "chapel",
            "round-table",
            "observatory",
        ];
        assert_eq!(matrix.locations.len(), ids.len());
        for (location, id) in matrix.locations.iter().zip(ids) {
            assert_eq!(location.id, id);
            assert!(location.effects.len() <= 8);
            for effect in &location.effects {
                let [x, y, w, h] = effect.rect;
                assert!(w > 0 && h > 0 && x < WIDTH && y < HEIGHT);
                assert!(w <= WIDTH - x && h <= HEIGHT - y);
            }
        }
        matrix
    })
}

// Existing material colors only: scene decoration never consumes state colors.
const WARM: [[u8; 3]; 6] = [
    [0x78, 0x46, 0x20],
    [0x92, 0x4f, 0x1d],
    [0xa9, 0x76, 0x39],
    [0xbe, 0x91, 0x4b],
    [0xd8, 0xa9, 0x5e],
    [0xe2, 0xcb, 0x8b],
];
const WATER: [[u8; 3]; 5] = [
    [0x52, 0x75, 0x97],
    [0x6e, 0x8a, 0x8e],
    [0x8f, 0xa3, 0xa8],
    [0xb3, 0xc2, 0xc4],
    [0xd5, 0xde, 0xe0],
];

pub(super) fn frame(building: Building, pose: Pose) -> RgbaImage {
    let mut image = plate(building).clone();
    let location = &matrix().locations[super::cinematics::building_index(building) as usize];
    animate(&mut image, &location.effects, pose);
    image
}

/// Apply the existing low-resolution material animation as colour deltas to
/// the source painting. Looking for palette colours in the full-colour source
/// would silently drop the masks; using the admitted plate keeps their exact
/// placement, phase, reduced-motion behavior and architecture exclusions.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn detail_frame(building: Building, pose: Pose) -> RgbaImage {
    let mut image = detail_plate(building).clone();
    if pose.mode == 0 {
        return image;
    }
    let base = plate(building);
    let animated = frame(building, pose);
    for (x, y, after) in animated.enumerate_pixels() {
        let before = base.get_pixel(x, y);
        if before == after {
            continue;
        }
        for dy in 0..3 {
            for dx in 0..3 {
                let pixel = image.get_pixel_mut(x * 3 + dx, y * 3 + dy);
                for c in 0..3 {
                    let delta = i16::from(after[c]) - i16::from(before[c]);
                    pixel[c] = (i16::from(pixel[c]) + delta).clamp(0, 255) as u8;
                }
            }
        }
    }
    image
}

pub(super) fn animate(image: &mut RgbaImage, effects: &[Effect], pose: Pose) {
    if pose.mode == 0 {
        return;
    }
    for (slot, effect) in effects.iter().enumerate() {
        let [x, y, w, h] = effect.rect;
        for py in y..y + h {
            for px in x..x + w {
                let pixel = image.get_pixel_mut(px, py);
                let rgb = [pixel[0], pixel[1], pixel[2]];
                match effect.kind {
                    Kind::Flame => {
                        // Gentle brightness travel inside the flame, never a whole-room pulse.
                        let phase = if pose.mode == 2 {
                            pose.phase / 2
                        } else {
                            pose.phase
                        };
                        let dim = [false, false, true, true, true, false, false, false]
                            [(usize::from(phase) + slot * 3 + (py / 5) as usize) % 8];
                        if dim
                            && (px + py) % 3 == 0
                            && let Some(i) = WARM.iter().position(|c| *c == rgb)
                        {
                            let color = WARM[i.saturating_sub(1)];
                            *pixel = image::Rgba([color[0], color[1], color[2], 255]);
                        }
                    }
                    Kind::Water if pose.mode == 2 => {
                        // A downward highlight band repeats seamlessly every 32 source pixels.
                        // Material masking keeps cliffs and vegetation inside the box untouched.
                        let band = (py + 32 - u32::from(pose.phase) * 2) % 32;
                        if band < 5
                            && let Some(i) = WATER.iter().position(|c| *c == rgb)
                        {
                            let color = WATER[(i + 1).min(WATER.len() - 1)];
                            *pixel = image::Rgba([color[0], color[1], color[2], 255]);
                        }
                    }
                    Kind::Water => {}
                }
            }
        }
    }
}
const PLATES: [&[u8]; 8] = [
    include_bytes!("../../../assets/realm/ambient/keep-waterfall.png"),
    include_bytes!("../../../assets/realm/ambient/gatehouse-mountain-v2.png"),
    include_bytes!("../../../assets/realm/ambient/rookery-mountain-v2.png"),
    include_bytes!("../../../assets/realm/ambient/scriptorium-mountain-v2.png"),
    include_bytes!("../../../assets/realm/ambient/smithy-mountain.png"),
    include_bytes!("../../../assets/realm/ambient/chapel-mountain-v2.png"),
    include_bytes!("../../../assets/realm/ambient/round-table-mountain.png"),
    include_bytes!("../../../assets/realm/ambient/observatory-mountain.png"),
];

// Same approved painting and version as each corresponding PLATES entry.
#[cfg_attr(not(test), allow(dead_code))]
const DETAIL_PLATES: [&[u8]; 8] = [
    include_bytes!("../../../assets/realm/ambient/keep-waterfall-source.png"),
    include_bytes!("../../../assets/realm/ambient/gatehouse-mountain-v2-source.png"),
    include_bytes!("../../../assets/realm/ambient/rookery-mountain-v2-source.png"),
    include_bytes!("../../../assets/realm/ambient/scriptorium-mountain-v2-source.png"),
    include_bytes!("../../../assets/realm/ambient/smithy-mountain-source.png"),
    include_bytes!("../../../assets/realm/ambient/chapel-mountain-v2-source.png"),
    include_bytes!("../../../assets/realm/ambient/round-table-mountain-source.png"),
    include_bytes!("../../../assets/realm/ambient/observatory-mountain-source.png"),
];

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn detail_plate(building: Building) -> &'static RgbaImage {
    static DECODED: [OnceLock<RgbaImage>; 8] = [const { OnceLock::new() }; 8];
    let index = super::cinematics::building_index(building) as usize;
    DECODED[index].get_or_init(|| {
        assert!(DETAIL_PLATES[index].len() <= 4 * 1024 * 1024);
        let source = image::load_from_memory(DETAIL_PLATES[index])
            .expect("embedded original ambient painting is valid")
            .to_rgb8();
        assert_eq!(source.dimensions(), (1340, 1174));
        // The palette admission script center-crops to round(width / aspect)
        // before resizing. Preserve that crop so the old masks still align.
        let crop_h = (f64::from(source.width()) * f64::from(HEIGHT) / f64::from(WIDTH))
            .round_ties_even() as u32;
        let crop = image::imageops::crop_imm(
            &source,
            0,
            (source.height() - crop_h) / 2,
            source.width(),
            crop_h,
        );
        let resized = image::imageops::resize(
            &crop.to_image(),
            DETAIL_WIDTH,
            DETAIL_HEIGHT,
            image::imageops::FilterType::Lanczos3,
        );
        image::DynamicImage::ImageRgb8(resized).to_rgba8()
    })
}

pub(super) fn plate(building: Building) -> &'static RgbaImage {
    static DECODED: [OnceLock<RgbaImage>; 8] = [const { OnceLock::new() }; 8];
    let index = super::cinematics::building_index(building) as usize;
    DECODED[index].get_or_init(|| {
        let image = image::load_from_memory(PLATES[index])
            .expect("embedded ambient location art is valid")
            .to_rgba8();
        assert_eq!(image.dimensions(), (WIDTH, HEIGHT));
        image
    })
}

impl World {
    pub(crate) fn ambient_interior_visible(&self) -> bool {
        self.interior.is_some()
    }

    pub(crate) fn ambient_building(&self) -> Building {
        self.interior.unwrap_or(self.target)
    }

    pub(super) fn ambient_pose(&self, motion: MotionMode) -> Pose {
        Pose::at(self.tick, motion)
    }

    /// New tool traffic and operator map travel share the existing destination.
    /// Only an admitted animation phase changes the cache; off is a true still.
    /// The high bits keep the ambient key distinct from other scene domains.
    pub(crate) fn ambient_scene_sequence(&self, motion: MotionMode) -> u64 {
        let pose = self.ambient_pose(motion);
        0xAAB1_E170_0000_0000
            | u64::from(super::cinematics::building_index(self.ambient_building()))
            | (u64::from(pose.phase) << 8)
            | (u64::from(pose.mode) << 16)
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/ambient__tests.rs"]
mod tests;
