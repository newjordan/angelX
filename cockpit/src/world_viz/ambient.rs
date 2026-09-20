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
use crate::lifecycle_viz::MotionMode;

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
            serde_json::from_str(include_str!("../../assets/realm/ambient/motion.json"))
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
    include_bytes!("../../assets/realm/ambient/keep-waterfall.png"),
    include_bytes!("../../assets/realm/ambient/gatehouse-mountain-v2.png"),
    include_bytes!("../../assets/realm/ambient/rookery-mountain-v2.png"),
    include_bytes!("../../assets/realm/ambient/scriptorium-mountain-v2.png"),
    include_bytes!("../../assets/realm/ambient/smithy-mountain.png"),
    include_bytes!("../../assets/realm/ambient/chapel-mountain-v2.png"),
    include_bytes!("../../assets/realm/ambient/round-table-mountain.png"),
    include_bytes!("../../assets/realm/ambient/observatory-mountain.png"),
];

// Same approved painting and version as each corresponding PLATES entry.
#[cfg_attr(not(test), allow(dead_code))]
const DETAIL_PLATES: [&[u8]; 8] = [
    include_bytes!("../../assets/realm/ambient/keep-waterfall-source.png"),
    include_bytes!("../../assets/realm/ambient/gatehouse-mountain-v2-source.png"),
    include_bytes!("../../assets/realm/ambient/rookery-mountain-v2-source.png"),
    include_bytes!("../../assets/realm/ambient/scriptorium-mountain-v2-source.png"),
    include_bytes!("../../assets/realm/ambient/smithy-mountain-source.png"),
    include_bytes!("../../assets/realm/ambient/chapel-mountain-v2-source.png"),
    include_bytes!("../../assets/realm/ambient/round-table-mountain-source.png"),
    include_bytes!("../../assets/realm/ambient/observatory-mountain-source.png"),
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
mod tests {
    use super::*;

    #[test]
    fn dotmax_arrival_requires_entry_and_leaving_restores_the_world() {
        let _env = crate::tests::env_lock();
        let _ink = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_INK");
        let _view = super::super::world3d::pin(super::super::world3d::WorldView::Mesh3d);
        for building in BUILDINGS {
            let mut world = World::new(71);
            world.select_landmark(building);
            assert!(!world.ambient_interior_visible(), "travel stays in Dotmax");
            for _ in 0..500 {
                world.tick();
            }
            assert!(!world.ambient_interior_visible(), "arrival is not entry");
            let outdoors = world
                .scryglass_frame_paced(40, 16, true, 0.0, 0.0, 1.05)
                .unwrap();
            assert!(world.enter_interior());
            assert!(world.ambient_interior_visible());
            assert!(
                world.ambient_interior_visible(),
                "no map actor in the painting"
            );
            let frame = world.ambient_frame(MotionMode::Off);
            assert_eq!(
                frame.pixels.as_ref(),
                plate(building).as_raw(),
                "interior is the approved painting without a placed knight"
            );
            let entered = world
                .scryglass_frame_paced(40, 16, true, 0.0, 0.0, 1.05)
                .unwrap();
            assert_ne!(
                entered, outdoors,
                "{building:?}: relaxed entry must replace outdoors"
            );
            let reused = world
                .scryglass_frame_paced(40, 16, true, 0.3, 0.2, 1.30)
                .unwrap();
            assert!(
                std::sync::Arc::ptr_eq(&entered, &reused),
                "room art ignores camera offsets"
            );
            let resized = world
                .scryglass_frame_paced(32, 14, true, 0.0, 0.0, 1.05)
                .unwrap();
            assert_eq!((resized.width, resized.height), (32, 14));
            assert!(!std::sync::Arc::ptr_eq(&entered, &resized));
            // Put a same-size room frame back into the cache immediately before
            // leaving, so a stale relaxed-cache hit cannot hide behind a resize.
            let entered = world
                .scryglass_frame_paced(40, 16, true, 0.0, 0.0, 1.05)
                .unwrap();
            assert!(world.leave_interior());
            assert!(!world.ambient_interior_visible());
            let returned = world
                .scryglass_frame_paced(40, 16, true, 0.0, 0.0, 1.05)
                .unwrap();
            assert_eq!(
                returned, outdoors,
                "{building:?}: same-tick leave restores the Dotmax frame"
            );
            assert!(!std::sync::Arc::ptr_eq(&entered, &returned));
            assert_eq!(
                super::super::world3d::current(),
                super::super::world3d::WorldView::Mesh3d
            );
        }
    }

    const BUILDINGS: [Building; 8] = [
        Building::Keep,
        Building::Gatehouse,
        Building::Rookery,
        Building::Scriptorium,
        Building::Smithy,
        Building::Chapel,
        Building::RoundTable,
        Building::Observatory,
    ];

    #[test]
    fn living_paintings_keep_architecture_fixed_and_use_material_colors() {
        for building in BUILDINGS {
            let base = plate(building);
            let location =
                &matrix().locations[super::super::cinematics::building_index(building) as usize];
            let mut changes = 0;
            for tick in (0..160).step_by(10) {
                let animated = frame(building, Pose::at(tick, MotionMode::Full));
                for (x, y, pixel) in animated.enumerate_pixels() {
                    if pixel != base.get_pixel(x, y) {
                        changes += 1;
                        assert!(
                            location.effects.iter().any(|effect| {
                                let [rx, ry, w, h] = effect.rect;
                                x >= rx && x < rx + w && y >= ry && y < ry + h
                            }),
                            "architecture moved in {building:?} at {x},{y}"
                        );
                        assert_eq!(pixel[3], 255);
                        let rgb = [pixel[0], pixel[1], pixel[2]];
                        assert!(WARM.contains(&rgb) || WATER.contains(&rgb));
                    }
                }
            }
            assert!(changes > 0, "{building:?} has no visible animated material");
            assert_eq!(frame(building, Pose::at(900, MotionMode::Off)), *base);
            assert_eq!(
                frame(building, Pose::at(0, MotionMode::Full)),
                frame(building, Pose::at(160, MotionMode::Full)),
                "loop seam"
            );
        }
    }

    #[test]
    fn native_paintings_preserve_source_detail_and_localized_motion_masks() {
        for building in BUILDINGS {
            let source = detail_plate(building);
            assert_eq!(source.dimensions(), (DETAIL_WIDTH, DETAIL_HEIGHT));
            assert!(
                std::ptr::eq(source, detail_plate(building)),
                "bounded cache reuses its plate"
            );
            assert_eq!(
                detail_frame(building, Pose::at(900, MotionMode::Off)),
                *source
            );
            let colours: std::collections::HashSet<_> =
                source.pixels().map(|pixel| pixel.0).collect();
            assert!(
                colours.len() > 75,
                "native art must retain original colours"
            );
            let base = plate(building);
            for mode in [MotionMode::Reduced, MotionMode::Full] {
                let mut changes = 0;
                for tick in [0, 40, 80, 120] {
                    let pose = Pose::at(tick, mode);
                    let low = frame(building, pose);
                    let high = detail_frame(building, pose);
                    for (x, y, pixel) in high.enumerate_pixels() {
                        assert_eq!(pixel[3], 255);
                        if pixel != source.get_pixel(x, y) {
                            changes += 1;
                            assert_ne!(
                                low.get_pixel(x / 3, y / 3),
                                base.get_pixel(x / 3, y / 3),
                                "native animation escaped its admitted mask in {building:?}"
                            );
                        }
                    }
                }
                assert!(
                    changes > 0,
                    "native {building:?} silently lost its material animation"
                );
            }
            assert_eq!(
                detail_frame(building, Pose::at(0, MotionMode::Full)),
                detail_frame(building, Pose::at(160, MotionMode::Full)),
                "native loop seam"
            );
        }
    }

    #[test]
    fn native_ambient_frame_keeps_room_identity_and_declared_pixel_dimensions() {
        let _pin = super::super::world3d::pin(super::super::world3d::WorldView::Mesh3d);
        let mut world = World::new(71);
        for building in BUILDINGS {
            world.interior = Some(building);
            let frame = world.ambient_detail_frame(MotionMode::Off);
            assert_eq!((frame.width, frame.height), (DETAIL_WIDTH, DETAIL_HEIGHT));
            let pixels = frame.pixels.as_ref();
            assert_eq!(pixels.len(), (frame.width * frame.height * 4) as usize);
            assert_eq!(pixels, detail_plate(building).as_raw());
            assert!(std::ptr::eq(pixels, frame.pixels.as_ref()));
        }
    }

    #[test]
    fn room_motion_cache_is_paced_and_off_freezes_paintings() {
        let mut world = World::new(71);
        world.settle_at_for_test(Building::Keep);
        assert!(world.enter_interior());
        let key = world.ambient_scene_sequence(MotionMode::Full);
        let still = world
            .ambient_braille_frame(30, 12, MotionMode::Off)
            .unwrap();
        let pixels = world
            .ambient_frame(MotionMode::Off)
            .pixels
            .as_ref()
            .to_vec();
        world.tick = 9;
        assert_eq!(key, world.ambient_scene_sequence(MotionMode::Full));
        world.tick = 10;
        assert_ne!(key, world.ambient_scene_sequence(MotionMode::Full));
        assert!(std::sync::Arc::ptr_eq(
            &still,
            &world
                .ambient_braille_frame(30, 12, MotionMode::Off)
                .unwrap()
        ));
        assert_eq!(pixels, world.ambient_frame(MotionMode::Off).pixels.as_ref());
        let moving = world
            .ambient_braille_frame(30, 12, MotionMode::Full)
            .unwrap();
        assert!(!std::sync::Arc::ptr_eq(&still, &moving));
        world.tick = 19;
        assert!(std::sync::Arc::ptr_eq(
            &moving,
            &world
                .ambient_braille_frame(30, 12, MotionMode::Full)
                .unwrap()
        ));
        let reduced = frame(Building::Keep, Pose::at(240, MotionMode::Reduced));
        let base = plate(Building::Keep);
        let [x, y, w, h] = matrix().locations[0]
            .effects
            .iter()
            .find(|e| matches!(e.kind, Kind::Water))
            .unwrap()
            .rect;
        for py in y..y + h {
            for px in x..x + w {
                assert_eq!(reduced.get_pixel(px, py), base.get_pixel(px, py));
            }
        }
    }

    #[test]
    #[ignore = "manual living-painting art review: set REALM_AMBIENT_DUMP"]
    fn dump_living_paintings_for_review() {
        let out = std::path::PathBuf::from(
            std::env::var("REALM_AMBIENT_DUMP").expect("set REALM_AMBIENT_DUMP"),
        );
        std::fs::create_dir_all(&out).unwrap();
        for building in BUILDINGS {
            let name =
                &matrix().locations[super::super::cinematics::building_index(building) as usize].id;
            for phase in 0..16 {
                frame(building, Pose::at(phase * 10, MotionMode::Full))
                    .save(out.join(format!("{name}-{phase:02}.png")))
                    .unwrap();
            }
        }
    }

    #[test]
    fn all_locations_have_distinct_bounded_opaque_art_without_state_colors() {
        let palette: serde_json::Value =
            serde_json::from_str(include_str!("../../assets/realm/palette.json")).unwrap();
        let materials: std::collections::HashSet<[u8; 3]> = palette["banks"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(name, _)| name.as_str() != "signal")
            .flat_map(|(_, bank)| bank["colors"].as_array().unwrap())
            .map(|value| {
                let text = value.as_str().unwrap();
                [1, 3, 5].map(|offset| u8::from_str_radix(&text[offset..offset + 2], 16).unwrap())
            })
            .collect();
        for (index, building) in BUILDINGS.into_iter().enumerate() {
            assert!(
                PLATES[index].len() < 128 * 1024,
                "{building:?} compressed budget"
            );
            let image = plate(building);
            assert_eq!(image.dimensions(), (256, 224));
            assert_eq!(image.as_raw().len(), 229_376);
            assert!(image.pixels().all(
                |pixel| pixel[3] == 255 && materials.contains(&[pixel[0], pixel[1], pixel[2]])
            ));
            assert!(std::ptr::eq(image, plate(building)), "decode is retained");
            for prior in &BUILDINGS[..index] {
                assert_ne!(
                    image.as_raw(),
                    plate(*prior).as_raw(),
                    "locations share a plate"
                );
            }
        }
    }

    #[test]
    fn ambient_fallback_is_visible_narrow_and_reuses_static_frames() {
        let mut world = World::new(0xA11CE);
        for building in BUILDINGS {
            world.settle_at_for_test(building);
            assert!(world.enter_interior());
            let first = world
                .scryglass_frame_paced(20, 7, false, 0.0, 0.0, 1.05)
                .unwrap();
            assert_eq!((first.width, first.height), (20, 7));
            assert!(first.cells.iter().any(|cell| cell.glyph != '\u{2800}'));
            world.tick();
            let second = world
                .scryglass_frame_paced(20, 7, true, 0.4, 0.2, 1.30)
                .unwrap();
            assert!(
                std::sync::Arc::ptr_eq(&first, &second),
                "still art does not animate"
            );
        }
        assert!(
            world
                .scryglass_frame_paced(0, 7, false, 0.0, 0.0, 1.05)
                .is_none()
        );
    }

    #[test]
    fn real_tool_event_leaves_the_entered_plate_before_rendering_new_outdoors() {
        let mut world = World::new(0xA11CE);
        world.settle_at_for_test(Building::Chapel);
        assert!(world.enter_interior());
        let first = world
            .scryglass_frame_paced(32, 14, false, 0.0, 0.0, 1.05)
            .unwrap();
        assert_eq!(world.ambient_building(), Building::Chapel);
        world.note_tool_call_event(
            crate::harness::ToolEventId("ambient-read-event".to_string()),
            "read_file",
            "path=research.md",
        );
        assert_eq!(
            world.ambient_building(),
            Building::Chapel,
            "entered room stays authoritative until leave"
        );
        assert_eq!(world.plate_caption().spans[1].content.as_ref(), "chapel");
        world.tick();
        assert!(
            !world.inside_interior(),
            "new work retains the established room-exit lifecycle"
        );
        assert_eq!(world.ambient_building(), Building::Scriptorium);
        assert_eq!(
            world.plate_caption().spans[1].content.as_ref(),
            "scriptorium"
        );
        let outside = world
            .scryglass_frame_paced(32, 14, true, 0.0, 0.0, 1.05)
            .unwrap();
        assert!(
            !std::sync::Arc::ptr_eq(&first, &outside),
            "leaving must not reuse the plate in relaxed mode"
        );
        assert_eq!(
            super::super::world3d::current(),
            super::super::world3d::WorldView::Mesh3d
        );
    }
}
