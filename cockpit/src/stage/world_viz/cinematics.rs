//! Cockpit-native world cinematics for the Scryglass pane.
//!
//! This is deliberately independent of terminal implementation and the model
//! harness. It observes the miniworld's display state and provides the shared
//! cinematic vocabulary — the semantic frame key, continuous raycast travel
//! and arrival scenes, and captions — that the braille ride renders from.

use super::raycast::{self, RayMap, RaySprite, RayView};
use super::*;
use image::DynamicImage;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

pub(super) const ARRIVAL_HOLD_TICKS: u32 = 6;
const ARRIVAL_ZOOM_TICKS: u32 = 64;
pub(super) const CINEMATIC_SETTLE_TICKS: u32 = ARRIVAL_HOLD_TICKS + ARRIVAL_ZOOM_TICKS;
const LOCATION_ZOOM_STEPS: f32 = 32.0;
/// Ticks over which a departure dissolves from the story plate into the
/// first-person world.
pub(super) const DEPART_FADE_TICKS: u32 = 8;
/// Half-extent of the raycast map window around the knight, in world tiles —
/// comfortably past the art layer's fog horizon so the window edge never
/// pops into view.
const FP_WINDOW: i32 = 15;
const TRAVEL_TERRAIN_HEIGHT_CAP: f32 = 0.72;
/// Geometry safety rail for every travelling camera pose. Authored arrival
/// marks sit much farther back; this only prevents an approach clipping walls.
const DOOR_PULL_UP: f32 = 1.45;
const VISTA_EASE_START: f32 = 9.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct VistaMark {
    pub(super) standoff: f32,
    /// Rotation from the facade normal pointing toward the road or keep.
    pub(super) bearing_offset_rad: f32,
    /// Camera-heading offset from looking directly at the landmark.
    pub(super) thirds_offset_rad: f32,
    pub(super) eye_lift: Option<f32>,
}

/// Keep, Gatehouse, Rookery, Scriptorium, Smithy, Chapel, Round Table,
/// Observatory. Alternating thirds make the sequence read as authored cuts.
pub(super) const VISTA_MARKS: [VistaMark; 8] = [
    VistaMark {
        standoff: 5.8,
        bearing_offset_rad: 0.10,
        thirds_offset_rad: -0.19,
        eye_lift: Some(0.06),
    },
    VistaMark {
        standoff: 6.5,
        bearing_offset_rad: 0.00,
        thirds_offset_rad: 0.18,
        eye_lift: None,
    },
    VistaMark {
        standoff: 4.8,
        bearing_offset_rad: -0.12,
        thirds_offset_rad: -0.20,
        eye_lift: Some(0.11),
    },
    VistaMark {
        standoff: 5.0,
        bearing_offset_rad: 0.14,
        thirds_offset_rad: 0.19,
        eye_lift: Some(0.05),
    },
    VistaMark {
        standoff: 4.4,
        bearing_offset_rad: -0.18,
        thirds_offset_rad: -0.19,
        eye_lift: Some(0.03),
    },
    VistaMark {
        standoff: 5.5,
        bearing_offset_rad: 0.38,
        thirds_offset_rad: 0.20,
        eye_lift: Some(0.07),
    },
    VistaMark {
        standoff: 5.9,
        bearing_offset_rad: -0.24,
        thirds_offset_rad: -0.18,
        eye_lift: Some(0.10),
    },
    VistaMark {
        standoff: 6.3,
        bearing_offset_rad: 0.16,
        thirds_offset_rad: 0.19,
        eye_lift: None,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LandmarkFootprint {
    half_width: i32,
    depth: i32,
}

/// Landmark depth in cells, in stable `building_index` order. Width remains
/// part of the shared landmark identity; this local table gives the outdoor
/// raycast enough return wall to read as architecture from a quarter view.
const LANDMARK_DEPTHS: [i32; 8] = [3, 2, 2, 2, 2, 3, 2, 3];

/// Prop billboards live well above landmark ids (0..=7), leaving room for
/// future building silhouettes without collisions.
pub(super) const PROP_ID_BASE: u8 = 32;
/// Fleet-village silhouettes occupy their own gap between civic landmarks
/// (0..=7) and arrival props (32..=39).
pub(super) const VILLAGE_FORGE_ID: u8 = 16;
pub(super) const VILLAGE_GRANARY_ID: u8 = 17;
pub(super) const VILLAGE_COTTAGE_ID: u8 = 18;
/// Outdoor ambient life reuses existing authored silhouettes, avoiding a new
/// atlas or renderer branch.
// Keep ambient ids disjoint while retaining the low three-bit authored prop
// shapes (raven/soft puff/standing figure) in the shared scene vocabulary.
pub(super) const RAVEN_SPRITE_ID: u8 = 84;
pub(super) const SMOKE_SPRITE_ID: u8 = 81;
pub(super) const VILLAGER_SPRITE_ID: u8 = 86;
/// Ceremony pennants are separate from permanent arrival dressing.
pub(super) const CEREMONY_PENNANT_ID: u8 = 87;
/// Reserved solely to make the first-person invariant explicit and testable.
pub(super) const KNIGHT_SELF_SPRITE_ID: u8 = 15;
/// District waymarkers occupy the upper half of the prop id range; the low
/// three bits preserve the deterministic district banner palette.
pub(super) const DISTRICT_WAYMARKER_ID_BASE: u8 = 48;
/// Natural scenery has a disjoint id range above district banners (48..=55).
pub(super) const SCENERY_TREE_ID: u8 = 72;
pub(super) const SCENERY_BOULDER_ID: u8 = 73;
pub(super) const SCENERY_BUSH_ID: u8 = 74;
pub(super) const SCENERY_FLOWERS_ID: u8 = 75;
const SCENERY_SPRITE_CAP: usize = 48;
pub(super) const RAVEN_SPRITE_CAP: usize = 3;
pub(super) const SMOKE_SPRITE_CAP: usize = 3;
pub(super) const VILLAGER_SPRITE_CAP: usize = 18;
pub(super) const AMBIENT_SPRITE_CAP: usize =
    RAVEN_SPRITE_CAP + SMOKE_SPRITE_CAP + VILLAGER_SPRITE_CAP;
pub(super) const AMBIENT_TICK_TICKS: u64 = 8;
/// Interior firelight is atmospheric scenery. Ten fresh frames per second at
/// the 40 Hz simulation clock are enough for a tiny terminal pane and keep the
/// mesh rasterizer out of latency-sensitive UI frames.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) const INTERIOR_FRAME_HOLD_TICKS: u64 = 4;
const ARRIVAL_PROP_DISTANCE: f32 = 4.25;
static LOCATION_ATLAS: OnceLock<Option<DynamicImage>> = OnceLock::new();
pub(super) const RIDER_FRAME_HOLD_TICKS: u64 = crate::stage::knight_cast::STEP_TICKS;
pub(super) type RiderFrameKey = crate::stage::knight_cast::FrameKey;

pub(crate) fn warm_assets() {
    static WARMING: std::sync::Once = std::sync::Once::new();
    WARMING.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("angel-world-art".to_string())
            .spawn(|| {
                let _ = LOCATION_ATLAS.get_or_init(load_location_atlas);
                crate::stage::knight_cast::warm();
            });
    });
}

impl World {
    /// Stable semantic key for the current miniviz frame. Pane dimensions live
    /// in the viewer cache key; this key changes only when visible world state
    /// crosses a useful animation step.
    pub(crate) fn cinematic_key(&self) -> u64 {
        // Room entry is a semantic change even when the outdoor pacing lane
        // is relaxed; room and outdoor keys occupy distinct domains.
        if self.ambient_interior_visible() {
            return self.ambient_scene_sequence(crate::ui::viz::lifecycle_viz::MotionMode::Off);
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        building_index(self.target).hash(&mut hash);
        // Z1: the adventure re-keys the frame — region, danger, party, a live
        // banner, and the loot count all change what Z2 draws.
        hash_quest_state(&mut hash, self.quest());
        // Z3b: the Swamp's wisps drift one step every `WISP_TICKS`; every other
        // stage reports phase 0, so only the marsh is ever re-keyed by the clock.
        super::world3d::region::wisp_phase_at(self.quest(), self.tick).hash(&mut hash);
        // Z2: only the camera-top arm paints the authored regions, and it
        // reads more than the quest summary — the camera window, the room
        // slide, the hero's facing and walk frame, the monsters' animation
        // bucket, the party line and the chests. Hashing it under any other
        // view would churn the first-person memos for nothing. The nominal
        // canvas keeps the key independent of pane size, which the ride
        // cache's own view key already carries.
        // The first-person palette follows the persisted realm clock. Keep
        // beat drift out of this semantic key: the hearth bucket already
        // quantizes daylight to five steps and carries the two weather bits.
        self.outcome_hearth_render_bucket().hash(&mut hash);
        // Presence matters as well as light: an all-dark enabled village must
        // not reuse a pre-village frame. Bit 0 is the working forge; bits
        // 1..=63 follow cottage layout order and therefore head identity.
        self.village_lit_mask().hash(&mut hash);
        // Two live-work bits plus one slow movement bucket. Health already
        // rides village_lit_mask above.
        let ambient_bits = self.active_ambient_bits();
        ambient_bits.hash(&mut hash);
        self.completion_ceremony_active.hash(&mut hash);
        if ambient_bits != 0 || self.village_lit_mask().is_some_and(|mask| mask != 0) {
            (self.tick / AMBIENT_TICK_TICKS).hash(&mut hash);
        }
        // Outdoor first-person always carries the bottom rider overlay. Its
        // observed activity and clamped frame must re-key the frame
        // whether travelling or settled.
        if self.interior.is_none() {
            crate::stage::knight_cast::available().hash(&mut hash);
            rider_frame_key(self).hash(&mut hash);
        }
        if self.cinematic_travelling() {
            0u8.hash(&mut hash);
            quantize_quarter(self.avatar_vis.0).hash(&mut hash);
            quantize_quarter(self.avatar_vis.1).hash(&mut hash);
            self.facing_left.hash(&mut hash);
            // The first-person view also turns with the eased heading, steps
            // through the departure dissolve, and dissolves from a specific
            // origin plate.
            quantize_heading(self.travel_heading).hash(&mut hash);
            self.travel_ticks.min(DEPART_FADE_TICKS).hash(&mut hash);
            building_index(self.plate_from).hash(&mut hash);
        } else {
            1u8.hash(&mut hash);
            location_atlas().is_some().hash(&mut hash);
            self.cinematic_zoom_step().hash(&mut hash);
            // The arrival camera hold advances the door pulse and prop reveal.
            self.settle_ticks.min(ARRIVAL_HOLD_TICKS).hash(&mut hash);
            // The settled camera's slow sway re-keys the frame as it turns.
            ((self.orbit_sway() * 24.0).round() as i16).hash(&mut hash);
        }
        hash.finish()
    }

    /// True only for an authored outdoor arrival composition. The reveal hold
    /// and later settled pose share this grade; travel and rooms do not.
    pub(super) fn settled_vista_grade(&self) -> bool {
        self.interior.is_none() && !self.cinematic_travelling()
    }

    /// The raycast scene for the current travel frame: a window of world
    /// tiles becomes the grid map, buildings use compact multi-cell footprints
    /// (the destination's front-center is its accent-framed door), and the
    /// eased travel heading points the saddle camera. The braille ride renders
    /// its geometry from this scene.
    pub(super) fn travel_scene(&self) -> (RayMap, RayView) {
        if let Some(building) = self.interior
            && let Some(scene) = super::interiors::scene(building, self.tick, self.orbit_sway())
        {
            return scene;
        }
        let origin_x = self.avatar_vis.0.round() as i32 - FP_WINDOW;
        let origin_y = self.avatar_vis.1.round() as i32 - FP_WINDOW;
        let span = (FP_WINDOW * 2 + 1) as u32;
        let door = self.building_pos(self.target);
        let door_world = (door.0 as f32 + 0.5, door.1 as f32 + 0.5);
        let rider_world = (self.avatar_vis.0 + 0.5, self.avatar_vis.1 + 0.5);
        let approach_dx = rider_world.0 - door_world.0;
        let approach_dy = rider_world.1 - door_world.1;
        let approach_dist = (approach_dx * approach_dx + approach_dy * approach_dy).sqrt();
        let vista_ease = smoothstep((1.0 - approach_dist / VISTA_EASE_START).clamp(0.0, 1.0));
        let (vista_world, vista_heading, vista_eye_lift) = self.vista_camera_pose(self.target);
        let camera_world = (
            lerp(rider_world.0, vista_world.0, vista_ease),
            lerp(rider_world.1, vista_world.1, vista_ease),
        );
        let camera_world_x = camera_world.0.round() as i32;
        let camera_world_y = camera_world.1.round() as i32;
        let smoothed_elevation = self.smoothed_travel_elevation(camera_world_x, camera_world_y);
        let relief = travel_terrain_height(smoothed_elevation);
        // travel_terrain_height caps at 0.72; normalize that world-space
        // camera-ground height to RayView's plain [0,1] eye-height contract.
        let eye_h =
            ((relief / TRAVEL_TERRAIN_HEIGHT_CAP) + vista_eye_lift * vista_ease).clamp(0.0, 1.0);
        let mut cells = vec![0u8; (span * span) as usize];
        // Per-cell normalized terrain height (0.0..=1.0) for the open-ground
        // cells. Walls/buildings/water keep their facade behavior; only the
        // open terrain cells (today's floor) gain height here. Built from the
        // island's continuous elevation field so the landscape the rider sees
        // stays shared with the world geometry.
        let mut terrain = vec![0.0f32; (span * span) as usize];
        let mut terrain_kind = vec![raycast::TERRAIN_MEADOW; (span * span) as usize];
        let mut biomes = vec![Biome::DeepWater; (span * span) as usize];
        for wy in 0..span as i32 {
            for wx in 0..span as i32 {
                let idx = wy as usize * span as usize + wx as usize;
                let world_x = origin_x + wx;
                let world_y = origin_y + wy;
                let biome = self.tile_at_world(world_x as f32 + 0.5, world_y as f32 + 0.5);
                biomes[idx] = biome;
                let m = self.travel_material(world_x, world_y);
                cells[idx] = m;
                terrain_kind[idx] = match biome {
                    Biome::Path => raycast::TERRAIN_ROAD,
                    Biome::Sand => raycast::TERRAIN_SAND,
                    Biome::Hill | Biome::Peak => raycast::TERRAIN_HILL,
                    _ => raycast::TERRAIN_MEADOW,
                };
                if m == 0 {
                    let elevation = if world_x >= 0
                        && world_y >= 0
                        && world_x < WORLD_W as i32
                        && world_y < WORLD_H as i32
                    {
                        self.tile_elevation[world_y as usize * WORLD_W + world_x as usize]
                    } else {
                        0.0
                    };
                    terrain[idx] = match biome {
                        // Hills are open near-wall-tall masses. Eye-relative
                        // relief lets a crest overlook its peers, but the
                        // 0.55 floor keeps every mountain materially present.
                        Biome::Hill => (0.88_f32 - relief).clamp(0.55, TRAVEL_TERRAIN_HEIGHT_CAP),
                        Biome::Peak => (1.0_f32 - relief).clamp(0.55, TRAVEL_TERRAIN_HEIGHT_CAP),
                        _ => (travel_terrain_height(elevation as f64) - relief).clamp(0.0, 1.0),
                    };
                }
            }
        }
        let village_capacity = self.village.as_ref().map_or(0, |v| v.cottages.len() + 2);
        let mut sprites =
            Vec::with_capacity(self.buildings.len() + village_capacity + AMBIENT_SPRITE_CAP + 8);
        for &(building, building_pos) in &self.buildings {
            // The front lies across the road's broad approach vector. A short
            // solid footprint gives every landmark real mass at
            // the authored quarter-view arrival angles instead of leaving a
            // paper-thin façade. The skyline sprite remains one cell behind
            // the front so its authored roof/tower silhouette still rises over
            // the unit-height footprint.
            let axis = self.travel_facade_axis(building_pos);
            let behind = self.travel_facade_behind(building_pos);
            let footprint = travel_landmark_footprint(building);
            let wing = travel_facade_material(building);
            let arrival_dressed = self.arrival_dressing_visible(building, building_pos);
            for depth in 0..=footprint.depth {
                for offset in -footprint.half_width..=footprint.half_width {
                    let world_x = building_pos.0 as i32 + axis.0 * offset + behind.0 * depth;
                    let world_y = building_pos.1 as i32 + axis.1 * offset + behind.1 * depth;
                    let (map_x, map_y) = (world_x - origin_x, world_y - origin_y);
                    if map_x >= 0 && map_y >= 0 && map_x < span as i32 && map_y < span as i32 {
                        let idx = map_y as usize * span as usize + map_x as usize;
                        cells[idx] = if depth == 0 && offset == 0 && building == self.target {
                            if arrival_dressed {
                                arrival_door_material(self.settle_ticks)
                            } else {
                                raycast::MATERIAL_DOOR
                            }
                        } else {
                            wing
                        };
                        terrain[idx] = 0.0;
                    }
                }
            }
            let facade_x = building_pos.0 as i32 - origin_x;
            let facade_y = building_pos.1 as i32 - origin_y;
            if facade_x >= 0 && facade_y >= 0 && facade_x < span as i32 && facade_y < span as i32 {
                let keep = self.building_pos(Building::Keep);
                let toward_keep = (
                    keep.0 as i32 - building_pos.0 as i32,
                    keep.1 as i32 - building_pos.1 as i32,
                );
                sprites.push(RaySprite {
                    x: facade_x as f32 + behind.0 as f32 + 0.5,
                    y: facade_y as f32 + behind.1 as f32 + 0.5,
                    width: (footprint.half_width * 2 + 1) as f32,
                    height: travel_landmark_height(building),
                    id: building_index(building),
                    lit: building == self.target,
                });
                if arrival_dressed {
                    self.push_arrival_props(
                        &mut sprites,
                        building,
                        (facade_x as f32 + 0.5, facade_y as f32 + 0.5),
                        axis,
                        toward_keep,
                    );
                }
            }
        }
        for district in &self.districts {
            let map_x = district.anchor.0 - origin_x;
            let map_y = district.anchor.1 - origin_y;
            if map_x >= 0 && map_y >= 0 && map_x < span as i32 && map_y < span as i32 {
                sprites.push(RaySprite {
                    x: map_x as f32 + 0.5,
                    y: map_y as f32 + 0.5,
                    width: 0.34,
                    height: 0.8,
                    id: district_waymarker_id(district.banner_color),
                    lit: false,
                });
            }
        }
        if let Some(village) = &self.village {
            let mut push_village =
                |pos: (usize, usize), width: f32, height: f32, id: u8, lit: bool| {
                    let map_x = pos.0 as i32 - origin_x;
                    let map_y = pos.1 as i32 - origin_y;
                    if map_x >= 0 && map_y >= 0 && map_x < span as i32 && map_y < span as i32 {
                        sprites.push(RaySprite {
                            x: map_x as f32 + 0.5,
                            y: map_y as f32 + 0.5,
                            width,
                            height,
                            id,
                            lit,
                        });
                    }
                };
            push_village(
                village.forge_pos,
                1.35,
                1.55,
                VILLAGE_FORGE_ID,
                village.forge_up && village.training,
            );
            push_village(village.granary_pos, 1.30, 1.08, VILLAGE_GRANARY_ID, false);
            for (head, pos) in &village.cottages {
                push_village(*pos, 1.05, 1.00, VILLAGE_COTTAGE_ID, village.head_up(head));
            }
        }
        self.push_truthful_ambient_sprites(&mut sprites, origin_x, origin_y, span);
        if self.completion_ceremony_active {
            let keep = self.building_pos(Building::Keep);
            let axis = self.travel_facade_axis(keep);
            for side in [-1.0_f32, 1.0] {
                let world_x = keep.0 as f32 + 0.5 + axis.0 as f32 * side * 1.35;
                let world_y = keep.1 as f32 + 0.5 + axis.1 as f32 * side * 1.35;
                let x = world_x - origin_x as f32;
                let y = world_y - origin_y as f32;
                if x >= 0.0 && y >= 0.0 && x < span as f32 && y < span as f32 {
                    sprites.push(RaySprite {
                        x,
                        y,
                        width: 0.34,
                        height: 2.25,
                        id: CEREMONY_PENNANT_ID,
                        lit: true,
                    });
                }
            }
        }
        let mut cam_x = camera_world.0 - origin_x as f32;
        let mut cam_y = camera_world.1 - origin_y as f32;
        // Authored vistas own the arrival. This is only the hard geometry
        // safety rail for an intermediate eased pose.
        let door_x = door.0 as f32 - origin_x as f32 + 0.5;
        let door_y = door.1 as f32 - origin_y as f32 + 0.5;
        let dx = cam_x - door_x;
        let dy = cam_y - door_y;
        let door_dist = (dx * dx + dy * dy).sqrt();
        if door_dist < DOOR_PULL_UP {
            if door_dist > 1e-3 {
                cam_x = door_x + dx / door_dist * DOOR_PULL_UP;
                cam_y = door_y + dy / door_dist * DOOR_PULL_UP;
            } else {
                let bearing = (vista_world.1 - door_world.1).atan2(vista_world.0 - door_world.0);
                cam_x = door_x + bearing.cos() * DOOR_PULL_UP;
                cam_y = door_y + bearing.sin() * DOOR_PULL_UP;
            }
        }
        // The saddle cell is always open.
        let cam_cell = (cam_y.max(0.0) as usize).min(span as usize - 1) * span as usize
            + (cam_x.max(0.0) as usize).min(span as usize - 1);
        cells[cam_cell] = 0;
        scatter_scenery(
            self.seed,
            origin_x,
            origin_y,
            span,
            cam_cell,
            &cells,
            &biomes,
            &terrain_kind,
            &mut sprites,
        );
        let map = RayMap {
            width: span,
            height: span,
            cells,
            terrain,
            terrain_kind,
            sprites,
        };
        let view = RayView {
            x: cam_x,
            y: cam_y,
            // Once settled, the saddle camera sways in a slow arc — the facade
            // keeps shifting against the sky (the living-realm orbit).
            heading_rad: lerp_angle(self.travel_heading, vista_heading, vista_ease)
                + self.orbit_sway(),
            look_yaw: 0.0,
            fov_rad: 1.05,
            // Follow the rider's clamped cadence so the silhouette and horizon
            // cannot strobe against one another.
            bob: if (self.tick / RIDER_FRAME_HOLD_TICKS).is_multiple_of(2) {
                0.5
            } else {
                -0.5
            },
            eye_h,
        };
        (map, view)
    }

    /// Compact truth payload for the first-person village render.
    pub(super) fn village_lit_mask(&self) -> Option<u64> {
        self.village.as_ref().map(|village| {
            let mut bits = u64::from(village.forge_up && village.training);
            for (index, (head, _)) in village.cottages.iter().take(63).enumerate() {
                if village.head_up(head) {
                    bits |= 1u64 << (index + 1);
                }
            }
            bits
        })
    }

    pub(super) fn active_research_landmark(&self) -> Option<Building> {
        self.active_work
            .values()
            .filter(|work| {
                matches!(
                    work.activity,
                    RealmActivity::Research | RealmActivity::Dispatch
                )
            })
            .max_by_key(|work| work.seq)
            .map(|work| work.landmark)
    }

    pub(super) fn active_build_landmark(&self) -> Option<Building> {
        self.active_work
            .values()
            .filter(|work| work.activity == RealmActivity::Forge)
            .max_by_key(|work| work.seq)
            .map(|work| work.landmark)
    }

    fn active_ambient_bits(&self) -> u8 {
        u8::from(self.active_research_landmark().is_some())
            | (u8::from(self.active_build_landmark().is_some()) << 1)
    }

    fn push_truthful_ambient_sprites(
        &self,
        sprites: &mut Vec<RaySprite>,
        origin_x: i32,
        origin_y: i32,
        span: u32,
    ) {
        let mut ambient_count = 0usize;
        let bucket = self.tick / AMBIENT_TICK_TICKS;
        let mut push = |world_x: f32, world_y: f32, width: f32, height: f32, id: u8, lit: bool| {
            if ambient_count >= AMBIENT_SPRITE_CAP {
                return;
            }
            let (x, y) = (world_x - origin_x as f32, world_y - origin_y as f32);
            if x >= 0.0 && y >= 0.0 && x < span as f32 && y < span as f32 {
                sprites.push(RaySprite {
                    x,
                    y,
                    width,
                    height,
                    id,
                    lit,
                });
                ambient_count += 1;
            }
        };

        if let Some(building) = self.active_research_landmark() {
            let (bx, by) = self.building_pos(building);
            const ORBIT: [(f32, f32); 8] = [
                (1.25, 0.00),
                (0.88, 0.88),
                (0.00, 1.25),
                (-0.88, 0.88),
                (-1.25, 0.00),
                (-0.88, -0.88),
                (0.00, -1.25),
                (0.88, -0.88),
            ];
            for index in 0..(2 + (self.seed as usize & 1)).min(RAVEN_SPRITE_CAP) {
                let (dx, dy) = ORBIT[(bucket as usize + index * 3) % ORBIT.len()];
                push(
                    bx as f32 + 0.5 + dx,
                    by as f32 + 0.5 + dy,
                    0.24,
                    1.95 + index as f32 * 0.08,
                    RAVEN_SPRITE_ID,
                    false,
                );
            }
        }

        if let Some(building) = self.active_build_landmark() {
            let (bx, by) = self.building_pos(building);
            let drift = (bucket % 3) as f32 * 0.16;
            for index in 0..SMOKE_SPRITE_CAP {
                push(
                    bx as f32 + 0.72 + drift + index as f32 * 0.18,
                    by as f32 + 0.42 - index as f32 * 0.12,
                    0.28 + index as f32 * 0.05,
                    1.58 + index as f32 * 0.42,
                    SMOKE_SPRITE_ID,
                    false,
                );
            }
        }

        if let Some(village) = &self.village {
            for (index, (head, cottage)) in village
                .cottages
                .iter()
                .take(VILLAGER_SPRITE_CAP)
                .enumerate()
            {
                if village.head_up(head) {
                    let (x, y) = super::life::villager_walk_position(
                        *cottage,
                        village.granary_pos,
                        index,
                        bucket,
                    );
                    push(x, y, 0.30, 0.58, VILLAGER_SPRITE_ID, true);
                }
            }
        }
        debug_assert!(ambient_count <= AMBIENT_SPRITE_CAP);
        debug_assert!(
            sprites
                .iter()
                .all(|sprite| sprite.id != KNIGHT_SELF_SPRITE_ID)
        );
    }

    /// Unit axis across the landmark's approach. Paths connect destinations to
    /// the keep, so using that stable vector makes the façade face the rider
    /// without storing another orientation system in the world state.
    fn travel_facade_front(&self, building: (usize, usize)) -> (f32, f32) {
        let keep = self.building_pos(Building::Keep);
        let toward_keep = (
            keep.0 as i32 - building.0 as i32,
            keep.1 as i32 - building.1 as i32,
        );
        let axis = self.travel_facade_axis(building);
        let normal = (-axis.1, axis.0);
        let front = if normal.0 * toward_keep.0 + normal.1 * toward_keep.1 >= 0 {
            normal
        } else {
            (-normal.0, -normal.1)
        };
        (front.0 as f32, front.1 as f32)
    }

    fn travel_facade_behind(&self, building: (usize, usize)) -> (i32, i32) {
        let front = self.travel_facade_front(building);
        (-front.0 as i32, -front.1 as i32)
    }

    fn vista_camera_pose(&self, building: Building) -> ((f32, f32), f32, f32) {
        let mark = vista_mark(building);
        let door = self.building_pos(building);
        let front = self.travel_facade_front(door);
        let bearing = front.1.atan2(front.0) + mark.bearing_offset_rad;
        let camera = (
            door.0 as f32 + 0.5 + bearing.cos() * mark.standoff,
            door.1 as f32 + 0.5 + bearing.sin() * mark.standoff,
        );
        let heading = bearing + std::f32::consts::PI + mark.thirds_offset_rad;
        (camera, heading, mark.eye_lift.unwrap_or(0.0))
    }

    fn travel_facade_axis(&self, building: (usize, usize)) -> (i32, i32) {
        let keep = self.building_pos(Building::Keep);
        let toward_keep = (
            keep.0 as i32 - building.0 as i32,
            keep.1 as i32 - building.1 as i32,
        );
        if toward_keep.0.abs() >= toward_keep.1.abs() {
            (0, 1)
        } else {
            (1, 0)
        }
    }

    fn arrival_dressing_visible(&self, building: Building, pos: (usize, usize)) -> bool {
        if building != self.target || self.settle_ticks == 0 {
            return false;
        }
        let dx = self.avatar_vis.0 - pos.0 as f32;
        let dy = self.avatar_vis.1 - pos.1 as f32;
        let radius = if self.arrived_building() == Some(building) {
            ARRIVAL_PROP_DISTANCE
        } else {
            4.0 + self.orbit_sway().abs().min(0.25)
        };
        dx * dx + dy * dy <= radius * radius
    }

    fn push_arrival_props(
        &self,
        sprites: &mut Vec<RaySprite>,
        building: Building,
        facade: (f32, f32),
        axis: (i32, i32),
        toward_keep: (i32, i32),
    ) {
        let normal = (-axis.1, axis.0);
        let front = if normal.0 * toward_keep.0 + normal.1 * toward_keep.1 >= 0 {
            normal
        } else {
            (-normal.0, -normal.1)
        };
        let jitter = arrival_prop_jitter(self.seed, building_index(building));
        let mut prop = |along: f32, forward: f32, width: f32, height: f32, id: u8, lit: bool| {
            sprites.push(RaySprite {
                x: facade.0 + axis.0 as f32 * (along + jitter) + front.0 as f32 * forward,
                y: facade.1 + axis.1 as f32 * (along + jitter) + front.1 as f32 * forward,
                width,
                height,
                id,
                lit,
            });
        };
        match building {
            Building::Smithy => {
                prop(-0.82, 0.72, 0.30, 0.48, PROP_ID_BASE, true);
                prop(0.82, 0.72, 0.30, 0.48, PROP_ID_BASE, true);
            }
            Building::Chapel => {
                prop(-0.72, 0.68, 0.24, 0.55, PROP_ID_BASE + 1, true);
                prop(0.72, 0.68, 0.24, 0.55, PROP_ID_BASE + 1, true);
            }
            Building::RoundTable => {
                for (along, forward) in [
                    (-1.10, 0.55),
                    (-0.60, 1.05),
                    (0.0, 1.22),
                    (0.60, 1.05),
                    (1.10, 0.55),
                ] {
                    prop(along, forward, 0.38, 0.30, PROP_ID_BASE + 2, false);
                }
            }
            Building::Observatory => prop(0.48, 0.82, 0.65, 0.58, PROP_ID_BASE + 3, false),
            Building::Rookery => prop(0.68, 0.72, 0.46, 0.58, PROP_ID_BASE + 4, false),
            Building::Keep => {
                prop(-1.42, 0.65, 0.34, 0.60, PROP_ID_BASE + 5, false);
                prop(1.42, 0.65, 0.34, 0.60, PROP_ID_BASE + 5, false);
            }
            Building::Gatehouse => prop(0.0, 0.48, 0.34, 0.60, PROP_ID_BASE + 6, true),
            Building::Scriptorium => prop(0.58, 0.76, 0.48, 0.38, PROP_ID_BASE + 7, false),
        }
    }

    fn smoothed_travel_elevation(&self, x: i32, y: i32) -> f64 {
        let mut total = 0.0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let sample_x = (x + dx).clamp(0, WORLD_W as i32 - 1) as usize;
                let sample_y = (y + dy).clamp(0, WORLD_H as i32 - 1) as usize;
                total += self.tile_elevation[sample_y * WORLD_W + sample_x] as f64;
            }
        }
        total / 9.0
    }

    /// Biome → wall material for the shared travel scene. Open ground is
    /// anything the knight could plausibly ride over; water alone walls the
    /// route; hills and peaks are tall open terrain the road winds between.
    fn travel_material(&self, x: i32, y: i32) -> u8 {
        match self.tile_at_world(x as f32 + 0.5, y as f32 + 0.5) {
            Biome::DeepWater | Biome::Water => raycast::MATERIAL_WATER,
            // Forest is traversable ground dressed with sparse tree
            // billboards. A hedge wall in every forest cell would turn the
            // continuous island back into corridors.
            Biome::Forest => 0,
            Biome::Hill | Biome::Peak => 0,
            Biome::Sand | Biome::Grass | Biome::Path => 0,
        }
    }

    /// The landmark the knight has settled at — the braille plate scene shows
    /// its illustrated card. `None` while travelling (the town map owns the
    /// pane so the ride stays visible).
    pub(crate) fn arrived_building(&self) -> Option<Building> {
        (!self.cinematic_travelling()).then_some(self.target)
    }

    /// Noir caption row for the braille plate scene: gold sigil bar, the
    /// landmark bold, the live activity murmured dim after an em-dash.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn plate_caption(&self) -> Line<'static> {
        use crate::ui::hud::{HUD_DIM, HUD_GOLD, HUD_TEXT};
        Line::from(vec![
            Span::styled("▌ ", Style::new().fg(HUD_GOLD)),
            Span::styled(
                building_name(self.ambient_building()).to_string(),
                Style::new().fg(HUD_TEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" — {}", self.ride_caption()),
                Style::new().fg(HUD_DIM),
            ),
        ])
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn ride_ward_caption(&self) -> String {
        let destination = self.building_pos(self.target);
        self.district_on_route(
            self.avatar_vis,
            (destination.0 as f32, destination.1 as f32),
        )
        .map(|district| format!(" — through the {} ward", district.name))
        .unwrap_or_default()
    }

    pub(super) fn cinematic_zoom_pending(&self) -> bool {
        !self.cinematic_travelling() && self.settle_ticks < CINEMATIC_SETTLE_TICKS
    }

    fn cinematic_travelling(&self) -> bool {
        self.avatar != self.dest() || !self.avatar_vis_settled()
    }

    fn cinematic_zoom_progress(&self) -> f32 {
        let active = self.settle_ticks.saturating_sub(ARRIVAL_HOLD_TICKS);
        let linear = active as f32 / ARRIVAL_ZOOM_TICKS as f32;
        smoothstep(linear.clamp(0.0, 1.0))
    }

    fn cinematic_zoom_step(&self) -> u8 {
        (self.cinematic_zoom_progress() * LOCATION_ZOOM_STEPS).round() as u8
    }
}

/// Material identity for the first-person façade wings. Reading and making
/// spaces stay timber-framed; civic, sacred and science landmarks use stone.
fn travel_facade_material(building: Building) -> u8 {
    crate::stage::identity::landmark(building).facade_material
}

/// Half-width of the façade in world cells (door excluded). Gatehouse and
/// keep need twin-tower breadth; the council pavilion and dome get a wider
/// civic silhouette; work buildings remain intimate.
fn travel_facade_span(building: Building) -> i32 {
    crate::stage::identity::landmark(building).facade_span
}

fn travel_landmark_footprint(building: Building) -> LandmarkFootprint {
    LandmarkFootprint {
        half_width: travel_facade_span(building),
        depth: LANDMARK_DEPTHS[building_index(building) as usize],
    }
}

/// 64 steps per turn: coarse enough that easing doesn't thrash the frame
/// cache, fine enough that turns stay smooth.
fn quantize_heading(heading: f32) -> i16 {
    (heading / std::f32::consts::TAU * 64.0).round() as i16
}

/// Map an island elevation sample to a normalized terrain height the
/// renderer treats as a fraction of wall height (0.0 = flat floor, 1.0 =
/// wall-tall). Open terrain spans sand at 0.02 through meadow/forest floor at
/// 0.42. The eased curve keeps meadows low, lets hill-feet rise, and caps every
/// open cell below a full wall. Stable and continuous, so neighbouring cells
/// project to a smooth heightfield.
fn travel_landmark_height(building: Building) -> f32 {
    crate::stage::identity::landmark(building).landmark_height
}

const ARRIVAL_DOOR_PULSE_BASE: u8 = 48;

fn arrival_door_material(settle_ticks: u32) -> u8 {
    ARRIVAL_DOOR_PULSE_BASE + settle_ticks.min(ARRIVAL_HOLD_TICKS) as u8
}

fn arrival_prop_jitter(seed: u64, building: u8) -> f32 {
    let mut bits = seed ^ (building as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    bits ^= bits >> 30;
    bits = bits.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    bits ^= bits >> 27;
    (((bits >> 56) as f32 / 255.0) - 0.5) * 0.12
}

fn travel_terrain_height(elevation: f64) -> f32 {
    let t = ((elevation - 0.02) / 0.40).clamp(0.0, 1.0);
    t.powf(1.5) as f32 * TRAVEL_TERRAIN_HEIGHT_CAP
}

fn location_atlas() -> Option<&'static DynamicImage> {
    LOCATION_ATLAS.get().and_then(Option::as_ref)
}

/// Observe actual work state. Idle time cannot rotate the cast or imply work.
pub(super) fn rider_frame_key(world: &World) -> RiderFrameKey {
    use crate::stage::knight_cast::Activity;
    let activity = if world.riding() {
        Activity::Travel
    } else if world
        .active_work()
        .any(|work| work.landmark == world.target)
    {
        match world.target {
            Building::Scriptorium | Building::Chapel | Building::Observatory => Activity::Study,
            Building::Smithy => Activity::Craft,
            Building::RoundTable => Activity::Council,
            Building::Gatehouse | Building::Rookery => Activity::Guard,
            Building::Keep => Activity::Rest,
        }
    } else {
        Activity::Rest
    };
    RiderFrameKey::at(activity, world.tick)
}

fn load_location_atlas() -> Option<DynamicImage> {
    let path = crate::platform::runtime_paths::cockpit_dir()
        .join("assets/world-cinematics/location-atlas.png");
    image::ImageReader::open(path).ok()?.decode().ok()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn building_name(building: Building) -> &'static str {
    match building {
        Building::Keep => "keep",
        Building::Gatehouse => "gatehouse",
        Building::Rookery => "rookery",
        Building::Scriptorium => "scriptorium",
        Building::Smithy => "smithy",
        Building::Chapel => "chapel",
        Building::RoundTable => "Round Table",
        Building::Observatory => "observatory",
    }
}

fn district_waymarker_id(color: (u8, u8, u8)) -> u8 {
    const COLORS: [(u8, u8, u8); 8] = [
        (244, 114, 182),
        (96, 165, 250),
        (250, 204, 21),
        (52, 211, 153),
        (192, 132, 252),
        (251, 146, 60),
        (34, 211, 238),
        (248, 113, 113),
    ];
    DISTRICT_WAYMARKER_ID_BASE
        + COLORS
            .iter()
            .position(|&candidate| candidate == color)
            .unwrap_or(0) as u8
}

pub(super) fn building_index(building: Building) -> u8 {
    crate::stage::identity::landmark(building).index
}

/// The quest facts a Z2 frame depends on: region, danger level, party size,
/// a live banner, and the loot count. Hashed in every `cinematic_key` branch
/// so adventure transitions never stick on a cached frame.
fn hash_quest_state(hash: &mut std::collections::hash_map::DefaultHasher, quest: &Quest) {
    quest.region().hash(hash);
    quest.danger().level().hash(hash);
    quest.party().hash(hash);
    quest.banner_is_some().hash(hash);
    quest.treasures().hash(hash);
    // Where the hero stands in the region: Dotmax keys its camera on
    // it (Z3), so a frame that has moved on must not be served from the memo.
    super::world3d::region::waypoint(quest).hash(hash);
}

fn vista_mark(building: Building) -> VistaMark {
    VISTA_MARKS[building_index(building) as usize]
}

fn lerp(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount
}

fn lerp_angle(from: f32, to: f32, amount: f32) -> f32 {
    let mut delta = to - from;
    while delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    while delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    from + delta * amount
}

fn quantize_quarter(value: f32) -> i16 {
    (value * 4.0).round() as i16
}

fn smoothstep(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

#[derive(Clone, Copy)]
struct SceneryCandidate {
    id: u8,
    width: f32,
    height: f32,
    jitter_x: f32,
    jitter_y: f32,
}

/// Stable world-cell noise: moving the camera window cannot move scenery.
fn scenery_hash01(seed: u64, x: i32, y: i32, salt: u64) -> f32 {
    let mut bits = seed
        ^ (x as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (y as i64 as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9)
        ^ salt;
    bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    bits ^= bits >> 31;
    (bits >> 40) as f32 / (1u32 << 24) as f32
}

fn scenery_candidate(
    seed: u64,
    world_x: i32,
    world_y: i32,
    biome: Biome,
    beside_rock: bool,
) -> Option<SceneryCandidate> {
    // Rocks may cluster beside high ground, but scenery itself only occupies
    // rideable natural surfaces, never a Hill/Peak cell.
    if !matches!(biome, Biome::Grass | Biome::Forest | Biome::Sand) {
        return None;
    }
    let roll = scenery_hash01(seed, world_x, world_y, 0x53_43_45_4e_45_52_59);
    let detail = scenery_hash01(seed, world_x, world_y, 0x53_43_45_4e_45_5f_44);
    let (id, width, height) = match biome {
        Biome::Forest if roll < 0.20 => {
            (SCENERY_TREE_ID, 0.68 + detail * 0.18, 0.90 + detail * 0.40)
        }
        Biome::Grass if roll < 0.10 => {
            if beside_rock && detail < 0.28 {
                (SCENERY_BOULDER_ID, 0.58, 0.35)
            } else if detail > 0.78 {
                (SCENERY_FLOWERS_ID, 0.28, 0.15)
            } else {
                (SCENERY_BUSH_ID, 0.48, 0.30)
            }
        }
        _ => return None,
    };
    Some(SceneryCandidate {
        id,
        width,
        height,
        jitter_x: (scenery_hash01(seed, world_x, world_y, 0x53_43_45_4e_45_5f_58) - 0.5) * 0.34,
        jitter_y: (scenery_hash01(seed, world_x, world_y, 0x53_43_45_4e_45_5f_59) - 0.5) * 0.34,
    })
}

#[allow(clippy::too_many_arguments)]
fn scatter_scenery(
    seed: u64,
    origin_x: i32,
    origin_y: i32,
    span: u32,
    camera_cell: usize,
    cells: &[u8],
    biomes: &[Biome],
    terrain_kind: &[u8],
    sprites: &mut Vec<RaySprite>,
) {
    let span_usize = span as usize;
    let occupied: std::collections::HashSet<(i32, i32)> = sprites
        .iter()
        .map(|sprite| (sprite.x.floor() as i32, sprite.y.floor() as i32))
        .collect();
    let center = span as i32 / 2;
    let mut candidates = Vec::new();
    for map_y in 0..span as i32 {
        for map_x in 0..span as i32 {
            let idx = map_y as usize * span_usize + map_x as usize;
            if idx == camera_cell
                || cells[idx] != 0
                || terrain_kind[idx] != raycast::TERRAIN_MEADOW
                || occupied.contains(&(map_x, map_y))
            {
                continue;
            }
            let beside_rock = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|&(dx, dy)| {
                let x = map_x + dx;
                let y = map_y + dy;
                x >= 0
                    && y >= 0
                    && x < span as i32
                    && y < span as i32
                    && matches!(
                        biomes[y as usize * span_usize + x as usize],
                        Biome::Hill | Biome::Peak
                    )
            });
            let world_x = origin_x + map_x;
            let world_y = origin_y + map_y;
            if let Some(candidate) =
                scenery_candidate(seed, world_x, world_y, biomes[idx], beside_rock)
            {
                let dx = map_x - center;
                let dy = map_y - center;
                candidates.push((dx * dx + dy * dy, world_y, world_x, map_x, map_y, candidate));
            }
        }
    }
    candidates.sort_by_key(|&(distance, world_y, world_x, ..)| (distance, world_y, world_x));
    sprites.extend(candidates.into_iter().take(SCENERY_SPRITE_CAP).map(
        |(_, _, _, map_x, map_y, candidate)| RaySprite {
            x: map_x as f32 + 0.5 + candidate.jitter_x,
            y: map_y as f32 + 0.5 + candidate.jitter_y,
            width: candidate.width,
            height: candidate.height,
            id: candidate.id,
            lit: false,
        },
    ));
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/cinematics__tests.rs"]
mod tests;
