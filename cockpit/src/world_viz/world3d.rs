//! world3d — true-3D mesh renderer for the scryglass world pane.
//!
//! Replaces the Wolfenstein-style grid raycaster with real geometry: castles,
//! towers, libraries, courtyards, staged as painting-worthy vistas (world vista
//! law). Contract + ownership map: docs/plans/world3d-spec.md.
//!
//! Coordinates: x/y = ground plane in RayMap tile units, +z = up.
//! Triangles are double-sided; the shader flips the flat normal toward the
//! camera. Everything here must be deterministic — byte-identical frames for
//! identical inputs.
//!
//! The seam is `ride.rs::scryglass_frame_paced`: Dotmax paints outdoors and
//! explicit entry paints the retained room plate. The paced cache, rider overlay,
//! Bayer screen and braille bridge retain their established contracts.
#![allow(dead_code)]

pub(crate) mod arch;
pub(crate) mod instruments;
pub(crate) mod interior;
pub(crate) mod math;
pub(crate) mod mesh;
pub(crate) mod raster;
pub(crate) mod region;
pub(crate) mod scene;

use math::v3;
use raster::View3;

/// How much of the RayView's normalized terrain relief lifts the eye. A tile is
/// a map cell, so this is a few inches of saddle, not a hill.
const EYE_RELIEF: f32 = 0.35;

/// Dotmax owns every outdoor surface. Legacy selectors remain parseable so a
/// saved launch flag or muscle-memory command cannot restore a retired style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WorldView {
    Mesh3d,
}

impl WorldView {
    pub(crate) fn label(self) -> &'static str {
        "Dotmax 3D"
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "3d" | "mesh" | "mesh3d" | "dotmax" | "journey" | "living" | "top" | "topdown"
            | "top-down" | "2d" | "raycast" | "ray" | "ambient" | "art" => Some(Self::Mesh3d),
            _ => None,
        }
    }
}

const DEFAULT_VIEW: WorldView = WorldView::Mesh3d;

/// Environment knobs can no longer switch the outdoor renderer. Visibility
/// remains controlled separately by /world off and ANGEL_SCRYGLASS.
pub(crate) fn current() -> WorldView {
    DEFAULT_VIEW
}

/// Compatibility command: selecting a view always retains Dotmax outdoors.
pub(crate) fn set(_view: WorldView) {}

/// Legacy /world 3d and Explore-v controls retain the sole outdoor view.
pub(crate) fn toggle() -> bool {
    true
}

pub(crate) fn cycle() -> WorldView {
    current()
}

pub(crate) fn status_line() -> String {
    format!(
        "world renderer: {} outdoors; Enter opens room plates, Leave returns to the world",
        current().label()
    )
}

/// Shared render fixtures retain their guard-shaped API. There is now only one
/// outdoor renderer, so no process-global or thread-local selection is needed.
#[cfg(test)]
pub(crate) struct RendererPin;

#[cfg(test)]
pub(crate) fn pin(_view: WorldView) -> RendererPin {
    RendererPin
}

#[cfg(test)]
pub(crate) fn pin_world3d() -> RendererPin {
    pin(WorldView::Mesh3d)
}

/// Shared by Dotmax's outdoor and interior lighting. Retained at the same
/// bearing as before the obsolete raycast art implementation was removed.
pub(super) const MOON_BEARING: f32 = 0.62;

/// The ride's 3D frame: the raycast camera, re-read as a real perspective
/// camera, over the staged mesh scene.
///
/// The camera is derived entirely from the `RayView` the ride already built,
/// so the 3D branch reads **no world state the cache key does not already
/// cover** — `travel_scene()`'s pose is a pure function of state
/// `cinematic_key()` hashes.
pub(crate) fn render_ride_frame(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    dot_w: u32,
    dot_h: u32,
) -> image::RgbaImage {
    raster::render_scene(
        &scene::scene_for(scene::SceneKey::COURT),
        &view3_from_ray(map, view, dot_w as usize, dot_h as usize),
        dot_w as usize,
        dot_h as usize,
    )
}

/// The ride's 3D frame for wherever the quest currently is.
///
/// Castle Town keeps [`render_ride_frame`] untouched — it is the realm, and
/// every staged vantage, sweep and test in this file is about it. The five
/// adventure regions (Z1's `Region`) each get their own staged mesh and their
/// own camera mark ([`region`]), picked by the waypoint the hero stands on, so
/// changing the retained world view moves the camera
/// and not the scene.
///
/// Reads only `quest` — region, danger, treasures and the waypoint index — all
/// of which `cinematic_key` hashes (`cinematics::hash_quest_state`) — plus the
/// operator's own `yaw` offset, which the ride cache keys on separately
/// (`RideViewKey::yaw`), and the wisp `bucket` (`tick / region::WISP_TICKS`),
/// which only the Swamp reads and `cinematic_key` hashes as a phase. A region takes the yaw *raw* rather than reading it
/// back out of the ray camera's heading: the ray heading is a bearing in the
/// castle town, and the regions are not in the castle town.
pub(crate) fn render_region_frame(
    quest: &super::Quest,
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    yaw: f32,
    bucket: u64,
    dot_w: u32,
    dot_h: u32,
) -> image::RgbaImage {
    match region::stage_for(quest.region()) {
        None => render_ride_frame(map, view, dot_w, dot_h),
        Some(stage) => {
            region::render_stage_frame(stage, quest, map, view, yaw, bucket, (dot_w, dot_h))
        }
    }
}

/// What the staged frame is made of — `(sky, built mass, ground)` as fractions
/// of the dot canvas. The vista law's own vocabulary, measured on the geometry
/// the ride would actually paint.
#[cfg(test)]
pub(crate) fn composition_mix(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> (f32, f32, f32) {
    raster::composition_mix(
        &scene::scene_for(scene::SceneKey::COURT),
        &view3_from_ray(map, view, dot_w, dot_h),
        dot_w,
        dot_h,
    )
}

/// The staged camera itself: `(x, y, z, heading, pitch, fov)`. Test-only
/// window onto the mapping, so a review dump can print where a vantage
/// actually put the eye rather than where its table entry asked for.
#[cfg(test)]
pub(crate) fn staged_eye(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> (f32, f32, f32, f32, f32, f32) {
    let camera = view3_from_ray(map, view, dot_w, dot_h);
    (
        camera.pos.x,
        camera.pos.y,
        camera.pos.z,
        camera.heading_rad,
        camera.pitch,
        camera.fov_rad,
    )
}

/// Which vantage the frame chose, how far the ride still is from it, and how
/// much of the staged mark has taken over — the three numbers that explain any
/// frame the sweep produces.
#[cfg(test)]
pub(crate) fn staged_progress(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
) -> (i32, f32, f32) {
    let mark = target_mark(map);
    let distance = mark.map_or(f32::INFINITY, |(_, x, y)| {
        ((x - view.x).powi(2) + (y - view.y).powi(2)).sqrt()
    });
    let travel = ((distance - ARRIVE_DIST) / (SWEEP_DIST - ARRIVE_DIST)).clamp(0.0, 1.0);
    (
        mark.map_or(-1, |(index, _, _)| index as i32),
        distance,
        smoothstep(1.0 - travel),
    )
}

/// Horizontal distance, in tiles, from the staged eye to the nearest *built*
/// surface — ground planes excluded, because standing on the road is not
/// standing in a doorway. This is the anti-door-close-up rail: a vista is
/// staged from across the court, never from a facade.
#[cfg(test)]
pub(crate) fn standoff(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> (f32, f32, f32) {
    let camera = view3_from_ray(map, view, dot_w, dot_h);
    let scene = scene::scene_for(scene::SceneKey::COURT);
    // Only masses the frame actually contains count. A cottage behind the eye
    // is scenery the camera has its back to, not a wall in its face — the rail
    // is about what the picture shows.
    let half_fov = camera.fov_rad * 0.5 * 1.1;
    let mut nearest = (f32::INFINITY, 0.0, 0.0);
    for tri in &scene.tris {
        if matches!(
            tri.mat,
            mesh::mat::GRASS | mesh::mat::PATH | mesh::mat::WATER | mesh::mat::FLOOR
        ) {
            continue;
        }
        for point in tri.v {
            let (dx, dy) = (point.x - camera.pos.x, point.y - camera.pos.y);
            if wrap_pi(dy.atan2(dx) - camera.heading_rad).abs() > half_fov {
                continue;
            }
            let d = (dx * dx + dy * dy).sqrt();
            if d < nearest.0 {
                nearest = (d, point.x, point.y);
            }
        }
    }
    nearest
}

// ── vista staging ─────────────────────────────────────────────────────────
//
// One scene, eight paintings. The ride settles on a `Building`; each one gets
// a staged pose around the court chosen so the frame composes as a picture and
// not as a screenshot of a wall:
//
// - the hero mass sits off-centre near a third, whole, never cropped;
// - at least one skyline spike (a tower cone, the chapel spire, the library
//   lantern) breaks the roofline;
// - the horizon sits in the lower third, so sky owns the frame's top;
// - the eye stands ≥ 4 tiles off every facade — a vista, never a doorway;
// - lit WINDOW bays are visible, because they carry the noir mood.
//
// Horizontal and vertical framing are expressed as *screen fractions*, not as
// angles, and the heading/pitch that realize them are solved per frame from the
// actual dot aspect. That is what makes one table hold up across a 96 × 72 dot
// ride pane and a 200 × 304 dot vista plate.

/// A staged camera mark around the court.
struct Vantage {
    /// Eye position, in scene tiles.
    eye: (f32, f32),
    /// The mass the shot is about; the heading is solved so it lands on
    /// `frame_x`.
    anchor: (f32, f32),
    /// Where the anchor sits across the frame: 0 = left edge, 1 = right edge.
    /// Always off the middle — the thirds are the whole point.
    frame_x: f32,
    /// Where the horizon sits down the frame: 0 = top, 1 = bottom. Lower third
    /// means ≥ 0.62.
    horizon: f32,
    /// Eye height above the local ground, in tiles.
    eye_h: f32,
    /// Focal length, as a multiplier on the ride's field of view. `1.0` is the
    /// ride's own lens; below that is a longer lens, which is how a mass that
    /// cannot be approached (the gate has a hamlet in front of it, the court
    /// has a wall around it) still fills its share of the frame.
    lens: f32,
    /// What the frame is about, for the dump sheet and the table in the tests.
    hero: &'static str,
}

/// The eight marks, in `building_index` order: Keep, Gatehouse, Rookery,
/// Scriptorium, Smithy, Chapel, Round Table, Observatory.
///
/// Five of the eight take a mass of their own — keep, gatehouse, library,
/// forge, chapel, star-tower — and the other three take a corner of the court
/// from a distance and a bearing nothing else uses. That split is the lesson of
/// the previous pass: when *eight* marks shared four walls, three of them came
/// back as the same photograph, and no amount of camera work fixed it. The
/// Smithy and Observatory got buildings (`arch::works`), the Rookery got a
/// dressed drum (`arch::rookery_dressing`), and only then was there anything to
/// stage.
///
/// `MOON_BEARING` is 0.62 of a turn (≈ 223°, south-west), which is the single
/// fact that decides every eye position here: a mass is lit on its south and
/// west faces and dark on its north and east ones, so a mark is only worth
/// having where the camera can stand on the lit side — or where the silhouette
/// is the point.
const VANTAGES: [Vantage; 8] = [
    // Keep — the castle from the west meadow: the south-west drum tower and
    // its banner hold the left third, the west curtain runs off to the right
    // with the keep's turrets stacked behind it. Looking north-east with the
    // moon behind the eye, so every facade in frame is a lit one.
    Vantage {
        eye: (-22.0, -16.0),
        anchor: (0.0, 1.5),
        frame_x: 0.60,
        horizon: 0.66,
        eye_h: 2.4,
        lens: 1.00,
        hero: "the west curtain and the keep behind its drum tower",
    },
    // Gatehouse — the arrival, taken from the river bank at the foot of the
    // road: the gate and both its drums off to the left, the bridge parapet
    // leading in from the west and the hamlet gables strung along the verges.
    // The eye moved west with the hamlet: once the cottages stood 7.5 tiles off
    // the road instead of 5.5, the old mark looked straight down a gable end
    // and the foreground went to 5% ground.
    Vantage {
        eye: (8.0, -25.0),
        anchor: (0.0, -11.0),
        frame_x: 0.40,
        horizon: 0.63,
        eye_h: 1.9,
        lens: 0.80,
        hero: "the gatehouse at the head of the road, over the bridge",
    },
    // Rookery — the north-west drum from the west meadow, at 12.8 tiles rather
    // than the old 19. Two things forced the move, and both are about the
    // dressing being *visible*: at 19 tiles the dovecote band and the perched
    // rooks were sub-dot, and against the moon the tower was a flat silhouette
    // that could not show a band at any range. From here the moon rakes the
    // drum's west limb, so the corbel band throws a shadow, the landing ledges
    // catch silver and the birds sit as black notches on a lit rim. The north
    // curtain and the keep's turrets stack up behind.
    Vantage {
        eye: (-22.5, 16.5),
        anchor: (-11.0, 11.0),
        frame_x: 0.36,
        horizon: 0.68,
        eye_h: 1.8,
        lens: 0.85,
        hero: "the rookery drum's moonlit band, birds on the rim",
    },
    // Scriptorium — the library, hero building of the realm, taken broadside
    // across the west meadow: the whole 13-tile flank with its ladder of lit
    // bays, the portico's pediment breaking the middle of it and the crossing
    // lantern breaking the ridge. The flank faces south, so the moon lights it
    // and the east gable falls away dark — the depth cue that stops a long
    // facade reading as a painted flat.
    //
    // 24 tiles is not a taste call. A 200 × 304 dot plate caps the horizontal
    // field at ~45° (VERT_FOV_MAX), and a 13-tile hall only fits inside that,
    // whole and off the middle, from about 22 tiles out.
    Vantage {
        eye: (-14.0, -17.0),
        anchor: (-20.0, 6.0),
        frame_x: 0.42,
        horizon: 0.66,
        eye_h: 1.9,
        lens: 1.00,
        hero: "the library's lit flank and crossing lantern across the west meadow",
    },
    // Smithy — the forge, and the forge is the fire: an open stone mouth with
    // an EMBER throat and a spark-lit anvil in it, held in the right two-thirds
    // with the castle's south-east drum tower rising out of its roofline. The
    // eye stands on the south river bank looking back up the hamlet verge.
    Vantage {
        eye: (16.4, -24.0),
        anchor: (10.8, -16.4),
        frame_x: 0.58,
        horizon: 0.64,
        eye_h: 1.7,
        lens: 1.00,
        hero: "the forge mouth burning under the castle's south-east drum",
    },
    // Chapel — the east rise: the spire off to the right, the court's
    // north-east drum tower behind it on the left.
    Vantage {
        eye: (26.0, -1.0),
        anchor: (17.0, 7.0),
        frame_x: 0.56,
        horizon: 0.66,
        eye_h: 1.7,
        lens: 1.00,
        hero: "the chapel rose window on the east rise",
    },
    // Round Table — the council panorama: the whole south-east face of the
    // castle from a rise, gate to the left, drum tower right, keep between.
    Vantage {
        eye: (20.0, -22.0),
        anchor: (0.0, -3.0),
        frame_x: 0.60,
        horizon: 0.66,
        eye_h: 2.8,
        lens: 0.88,
        hero: "the south-east drum tower over the court wall",
    },
    // Observatory — the star-tower on the east rise, taken on a north-west
    // bearing from the river bank so the dome's south-west facets take the
    // moon and its north-east ones go black: ten flat planes, one hard
    // terminator, a warm slit down the middle and the sighting tube out of it.
    // The court stacks up behind — east curtain, south-east drum, then the
    // keep's turrets at 33 tiles — so the frame has depth without anything
    // eclipsing the hero, which is what sank the previous pass's attempt to
    // shoot the keep past a corner tower.
    Vantage {
        eye: (25.5, -19.5),
        anchor: (18.5, -11.5),
        frame_x: 0.40,
        horizon: 0.64,
        eye_h: 2.4,
        lens: 0.96,
        hero: "the star-tower's faceted dome against the castle's south-east face",
    },
];

/// Travel uses a small dolly around the reviewed gatehouse view. Connecting
/// distant destination eyes through the court crosses solid buildings; even
/// a late eased blend can fill the whole pane with nearby masonry.
fn road_pose(travel: f32) -> (f32, f32) {
    let travel = travel.clamp(0.0, 1.0);
    let eye = VANTAGES[1].eye;
    (eye.0 + 0.3 * travel, eye.1 - 0.6 * travel)
}
/// Travel frames aim at the gatehouse; it is what the road is *for*.
const ROAD_ANCHOR: (f32, f32) = (0.0, -11.0);
const ROAD_FRAME_X: f32 = 0.42;
const ROAD_HORIZON: f32 = 0.62;
const ROAD_EYE_H: f32 = 1.9;

/// Distance from the destination (in RayMap tiles) at which the sweep starts
/// handing over to the staged mark, and the distance at which it has fully
/// arrived. The authored `VISTA_MARKS` stand off 4.4–6.5 tiles, so a settled
/// ride is always past `ARRIVE_DIST`; a landmark can only be seen at all
/// within ~21 tiles (the 31 × 31 window), so `SWEEP_DIST` is the far end of
/// the useful signal.
const SWEEP_DIST: f32 = 20.0;
const ARRIVE_DIST: f32 = 7.0;

/// How far the staged camera may swing off its mark, in radians, before the
/// soft limit bends it. Comfortably past the operator's own yaw clamp (±π/6)
/// plus the settled orbit sway (±0.30), so ordinary looking-around is linear
/// and only a wild travel heading gets bent.
const SWING_LIMIT: f32 = 1.80;

/// How far the rider's sub-tile drift dollies the staged camera, in tiles. The
/// drift is bounded by ±0.5, so the mark breathes by ±0.4 tiles — enough to
/// live, far too little to break the composition.
const PARALLAX: f32 = 0.8;

/// Vertical field of view ceiling. Braille dots are square, so a tall pane
/// (the 200 × 304 vista plate is 1.52:1) would otherwise stretch the vertical
/// field to 80°+ and shrink every mass to a smudge. Capping the vertical field
/// and deriving the horizontal one from it keeps the composition painterly at
/// any pane shape without ever distorting the projection.
const VERT_FOV_MAX: f32 = 1.12;

/// `RayView` → `View3`: the staged camera for this frame.
///
/// Everything here is a pure function of the `RayMap`/`RayView` the ride
/// already built, and therefore of state `cinematic_key()` already hashes —
/// no new world reads, no clock, no randomness.
///
/// - **Which vantage** comes from the map's own sprite list: exactly one
///   landmark billboard (`id` 0..=7) is `lit`, and that is the destination
///   (`cinematics.rs:400-405`). The building index is hashed into the cache
///   key, so a per-target camera is cache-safe.
/// - **How far along** comes from the distance to that billboard, which is the
///   ride's real approach distance. Beyond the window there is no billboard,
///   which is itself the signal for "still a long way out".
/// - **Where the operator is looking** comes from the angle between the ray
///   camera's heading and its own bearing to that billboard. That difference
///   is exactly `thirds_offset + orbit_sway + operator_yaw`; subtracting the
///   authored thirds offset leaves the live part, which is applied as a swing
///   around the staged mark. So arrow-key yaw and the settled sway both drive
///   the 3D camera without a single extra input.
/// - **Pitch** is solved so the horizon lands where the vantage asks, plus the
///   ride's `bob` — the raycaster's horizon offset in units of 3% of frame
///   height (`raycast.rs:169`), which doubles as the operator pitch
///   (`ride.rs:142`).
fn view3_from_ray(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> View3 {
    let mark = target_mark(map);
    let building = mark.map_or(0, |(index, _, _)| index as usize);
    let vantage = &VANTAGES[building.min(VANTAGES.len() - 1)];

    // How far the ride still is from its destination, and therefore how much of
    // the staged mark has taken over from the road sweep.
    let distance = mark.map_or(f32::INFINITY, |(_, x, y)| {
        ((x - view.x).powi(2) + (y - view.y).powi(2)).sqrt()
    });
    let travel = ((distance - ARRIVE_DIST) / (SWEEP_DIST - ARRIVE_DIST)).clamp(0.0, 1.0);
    let arrival = smoothstep(1.0 - travel);

    let road_eye = road_pose(travel);
    // Cut between framed shots rather than interpolating the eye through
    // walls. Both shots retain live operator look and the rider's motion.
    let stage = if arrival >= 0.85 { 1.0 } else { 0.0 };
    let eye = (
        lerp(road_eye.0, vantage.eye.0, stage),
        lerp(road_eye.1, vantage.eye.1, stage),
    );
    let anchor = (
        lerp(ROAD_ANCHOR.0, vantage.anchor.0, stage),
        lerp(ROAD_ANCHOR.1, vantage.anchor.1, stage),
    );
    let frame_x = lerp(ROAD_FRAME_X, vantage.frame_x, stage);
    let horizon = lerp(ROAD_HORIZON, vantage.horizon, stage);
    let eye_h = lerp(ROAD_EYE_H, vantage.eye_h, stage);
    let lens = lerp(VANTAGES[1].lens, vantage.lens, stage);

    // Field of view: the operator's zoom, tamed so a portrait pane cannot blow
    // the vertical field out.
    let aspect = dot_h.max(1) as f32 / dot_w.max(1) as f32;
    let mut tan_h = (view.fov_rad.clamp(0.70, 1.40) * 0.5).tan();
    let tan_v_max = (VERT_FOV_MAX * 0.5).tan();
    if tan_h * aspect > tan_v_max {
        tan_h = tan_v_max / aspect.max(1e-3);
    }
    // The lens rides *after* the cap: a longer lens is a composition choice and
    // must survive the pane-shape correction, not be swallowed by it.
    tan_h *= lens.clamp(0.35, 1.0);
    let tan_v = tan_h * aspect;

    // The world traveller may turn toward another map landmark during a
    // trip. That automatic heading must not turn the staged eye into a wall.
    // Keep intentional look input independent until the settled orbit owns it.
    let swing = if arrival < 1.0 {
        view.look_yaw
    } else {
        operator_swing(map, view, arrival)
    };

    // Solve the heading that puts `anchor` at `frame_x`: a point at angular
    // offset θ from the axis lands at 0.5·(1 + tanθ/tan_h) across the frame.
    let to_anchor = (anchor.1 - eye.1).atan2(anchor.0 - eye.0);
    let heading = to_anchor - (tan_h * (2.0 * frame_x - 1.0)).atan() + swing;

    // Sub-tile parallax: dolly the mark along the camera's own axes so the
    // stage breathes with the rider instead of standing dead still.
    //
    // The window is centred on the **avatar**, not on the camera
    // (`cinematics.rs:283-284`), so once the vista ease pulls the camera back
    // to its authored standoff this offset is several whole tiles, not a
    // fraction. Clamping keeps it what it is meant to be — a breath — instead
    // of a second, unauthored camera move that drags every mark off its spot.
    let drift_x = (view.x - (map.width as f32 - 1.0) * 0.5).clamp(-0.5, 0.5);
    let drift_y = (view.y - (map.height as f32 - 1.0) * 0.5).clamp(-0.5, 0.5);
    let (sin_h, cos_h) = heading.sin_cos();
    let eye = (
        eye.0 + (-sin_h * drift_x + cos_h * drift_y) * PARALLAX,
        eye.1 + (cos_h * drift_x + sin_h * drift_y) * PARALLAX,
    );

    // Solve the pitch that drops the horizon to `horizon` down the frame: a
    // level ray lands at 0.5·(1 + tan(pitch)/tan_v).
    let level = (tan_v * (2.0 * horizon - 1.0)).atan();
    let pitch = (level + view.bob * 0.03).clamp(-0.90, 0.90);

    let ground = arch::court_ground_z(scene::SCENE_SEED, eye.0, eye.1);
    View3 {
        pos: v3(
            eye.0.clamp(-arch::COURT_EXTENT, arch::COURT_EXTENT),
            eye.1.clamp(-arch::COURT_EXTENT, arch::COURT_EXTENT),
            ground + eye_h + view.eye_h.clamp(0.0, 1.0) * EYE_RELIEF,
        ),
        heading_rad: heading,
        pitch,
        fov_rad: 2.0 * tan_h.atan(),
    }
}

/// The live part of the operator/sway heading, measured against the ray
/// camera's own bearing to a world-anchored landmark — see [`view3_from_ray`]'s
/// own note. Shared with the adventure regions ([`region`]), which have no road
/// sweep of their own but pan on exactly the same input.
pub(super) fn operator_swing(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    arrival: f32,
) -> f32 {
    match target_mark(map).or_else(|| any_landmark(map, view)) {
        Some((index, x, y)) => {
            let bearing = (y - view.y).atan2(x - view.x);
            let authored = super::cinematics::VISTA_MARKS[index as usize % 8].thirds_offset_rad;
            let gain = 0.35 + 0.65 * arrival;
            // Soft limit, not a clamp: a hard clamp makes two different
            // operator headings render the identical frame once both saturate,
            // which is a camera that has stopped answering the arrow keys.
            // `tanh` bends without ever flattening.
            let raw = wrap_pi(view.heading_rad - bearing) - authored * arrival;
            SWING_LIMIT * (raw / SWING_LIMIT).tanh() * gain
        }
        None => 0.0,
    }
}

/// The destination billboard: exactly one landmark sprite (`id` 0..=7) is lit,
/// and it stands at the target's facade (`cinematics.rs:396-405`). Returns
/// `(building_index, x, y)` in RayMap tile coordinates.
fn target_mark(map: &super::raycast::RayMap) -> Option<(u8, f32, f32)> {
    map.sprites
        .iter()
        .find(|sprite| sprite.lit && sprite.id < 8)
        .map(|sprite| (sprite.id, sprite.x, sprite.y))
}

/// Any landmark billboard in the window, in the world's own building order —
/// the heading reference for frames whose destination is still over the
/// horizon. Deterministic: `RayMap::sprites` is a `Vec` built in a fixed order.
///
/// Billboards nearly under the eye are skipped: the knight usually *starts* a
/// journey standing on a landmark, and a bearing to a point zero tiles away is
/// noise, which would then be amplified into a random camera heading.
const REFERENCE_MIN_DIST: f32 = 3.0;

fn any_landmark(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
) -> Option<(u8, f32, f32)> {
    map.sprites
        .iter()
        .find(|sprite| {
            sprite.id < 8
                && (sprite.x - view.x).powi(2) + (sprite.y - view.y).powi(2)
                    > REFERENCE_MIN_DIST * REFERENCE_MIN_DIST
        })
        .map(|sprite| (sprite.id, sprite.x, sprite.y))
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn wrap_pi(angle: f32) -> f32 {
    let turn = std::f32::consts::TAU;
    let wrapped = (angle + std::f32::consts::PI).rem_euclid(turn);
    wrapped - std::f32::consts::PI
}

/// The settled camera a `Vantage` produces — the `stage = 1`, `swing = 0`
/// branch of [`view3_from_ray`], factored out so a staging probe can ask "what
/// would this mark look like?" without building a `RayMap` for every candidate.
///
/// It is the same arithmetic, not a copy of the intent: any change to the
/// framing solve has to move both, and `a_probe_camera_matches_the_settled_ride`
/// fails loudly if they drift.
#[cfg(test)]
fn settled_camera(vantage: &Vantage, dot_w: usize, dot_h: usize) -> View3 {
    let aspect = dot_h.max(1) as f32 / dot_w.max(1) as f32;
    let mut tan_h = (1.05_f32 * 0.5).tan();
    let tan_v_max = (VERT_FOV_MAX * 0.5).tan();
    if tan_h * aspect > tan_v_max {
        tan_h = tan_v_max / aspect.max(1e-3);
    }
    tan_h *= vantage.lens.clamp(0.35, 1.0);
    let tan_v = tan_h * aspect;

    let to_anchor = (vantage.anchor.1 - vantage.eye.1).atan2(vantage.anchor.0 - vantage.eye.0);
    let heading = to_anchor - (tan_h * (2.0 * vantage.frame_x - 1.0)).atan();
    let pitch = (tan_v * (2.0 * vantage.horizon - 1.0))
        .atan()
        .clamp(-0.90, 0.90);
    let ground = arch::court_ground_z(scene::SCENE_SEED, vantage.eye.0, vantage.eye.1);
    View3 {
        pos: v3(vantage.eye.0, vantage.eye.1, ground + vantage.eye_h),
        heading_rad: heading,
        pitch,
        fov_rad: 2.0 * tan_h.atan(),
    }
}

/// Horizontal distance to the nearest built surface inside the frame — the
/// anti-door-close-up rail, measured off a probe camera.
#[cfg(test)]
fn camera_standoff(camera: &View3) -> (f32, f32, f32) {
    let scene = scene::scene_for(scene::SceneKey::COURT);
    let half_fov = camera.fov_rad * 0.5 * 1.1;
    let mut nearest = (f32::INFINITY, 0.0, 0.0);
    for tri in &scene.tris {
        if matches!(
            tri.mat,
            mesh::mat::GRASS | mesh::mat::PATH | mesh::mat::WATER | mesh::mat::FLOOR
        ) {
            continue;
        }
        for point in tri.v {
            let (dx, dy) = (point.x - camera.pos.x, point.y - camera.pos.y);
            if wrap_pi(dy.atan2(dx) - camera.heading_rad).abs() > half_fov {
                continue;
            }
            let d = (dx * dx + dy * dy).sqrt();
            if d < nearest.0 {
                nearest = (d, point.x, point.y);
            }
        }
    }
    nearest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_view_commands_and_launch_flags_cannot_select_other_outdoors() {
        let _env = crate::tests::env_lock();
        for word in [
            "3d", "MESH", "mesh3d", "dotmax", "journey", "living", "top", "topdown", "top-down",
            "2d", "raycast", "ray", "ambient", "art",
        ] {
            let view = WorldView::parse(word).expect("supported compatibility alias");
            let _named = crate::tests::TestEnvGuard::set("ANGEL_WORLD_VIEW", word);
            for legacy in [
                "", "0", "off", "false", "no", "n", "1", "on", "true", "yes", "y", "typo",
            ] {
                let _legacy = crate::tests::TestEnvGuard::set("ANGEL_WORLD_3D", legacy);
                set(view);
                assert_eq!(current(), WorldView::Mesh3d);
                assert!(toggle());
                assert_eq!(cycle(), WorldView::Mesh3d);
                assert!(status_line().contains("Dotmax 3D outdoors"));
            }
        }
        assert_eq!(WorldView::parse("unsupported"), None);
    }

    use super::super::raycast::{RayMap, RaySprite, RayView};

    /// A 31 × 31 window with one lit landmark billboard `standoff` tiles away
    /// on the +x bearing — the shape `travel_scene()` hands the seam.
    fn window(building: u8, standoff: f32) -> (RayMap, RayView) {
        let span = 31usize;
        let centre = (span as f32 - 1.0) * 0.5;
        let map = RayMap {
            width: span as u32,
            height: span as u32,
            cells: vec![0; span * span],
            terrain: vec![0.0; span * span],
            terrain_kind: vec![0; span * span],
            sprites: vec![RaySprite {
                x: centre + standoff,
                y: centre,
                width: 3.0,
                height: 3.0,
                id: building,
                lit: true,
            }],
        };
        let view = RayView {
            x: centre,
            y: centre,
            // Looking straight at the landmark, plus the authored thirds
            // offset — exactly what a settled `RayView` carries.
            heading_rad: super::super::cinematics::VISTA_MARKS[building as usize].thirds_offset_rad,
            look_yaw: 0.0,
            fov_rad: 1.05,
            bob: 0.0,
            eye_h: 0.0,
        };
        (map, view)
    }

    #[test]
    fn every_building_lands_on_its_own_staged_vantage() {
        for building in 0u8..8 {
            let (map, view) = window(building, 5.5);
            let camera = view3_from_ray(&map, &view, 200, 304);
            let vantage = &VANTAGES[building as usize];
            assert!(
                (camera.pos.x - vantage.eye.0).abs() < 0.05
                    && (camera.pos.y - vantage.eye.1).abs() < 0.05,
                "{}: settled at {:?}, want {:?}",
                vantage.hero,
                (camera.pos.x, camera.pos.y),
                vantage.eye
            );
            // The eye stands on the court's own ground, not on z = 0.
            let ground = arch::court_ground_z(scene::SCENE_SEED, camera.pos.x, camera.pos.y);
            assert!(
                (camera.pos.z - (ground + vantage.eye_h)).abs() < 1e-3,
                "{}: eye {} is not {} above ground {ground}",
                vantage.hero,
                camera.pos.z,
                vantage.eye_h
            );
        }
    }

    #[test]
    fn the_anchor_lands_on_its_third_and_the_horizon_in_the_lower_third() {
        // Framing is expressed in screen fractions and solved per aspect, so
        // both the wide ride pane and the tall vista plate must honour it.
        for &(dot_w, dot_h) in &[(96usize, 72usize), (200, 304), (216, 200)] {
            for building in 0u8..8 {
                let (map, view) = window(building, 5.5);
                let camera = view3_from_ray(&map, &view, dot_w, dot_h);
                let vantage = &VANTAGES[building as usize];

                let tan_h = (camera.fov_rad * 0.5).tan();
                let tan_v = tan_h * dot_h as f32 / dot_w as f32;
                let bearing =
                    (vantage.anchor.1 - camera.pos.y).atan2(vantage.anchor.0 - camera.pos.x);
                let offset = wrap_pi(bearing - camera.heading_rad);
                let at_x = 0.5 * (1.0 + offset.tan() / tan_h);
                assert!(
                    (at_x - vantage.frame_x).abs() < 0.02,
                    "{} @{dot_w}x{dot_h}: anchor at {at_x:.3}, want {:.3}",
                    vantage.hero,
                    vantage.frame_x
                );
                let at_y = 0.5 * (1.0 + camera.pitch.tan() / tan_v);
                assert!(
                    (at_y - vantage.horizon).abs() < 0.02,
                    "{} @{dot_w}x{dot_h}: horizon at {at_y:.3}, want {:.3}",
                    vantage.hero,
                    vantage.horizon
                );
                assert!(
                    at_y >= 0.60 && at_x > 0.06 && at_x < 0.94,
                    "{}: composition off the frame",
                    vantage.hero
                );
            }
        }
    }

    #[test]
    fn the_table_stages_thirds_standoff_and_open_sky() {
        for (index, vantage) in VANTAGES.iter().enumerate() {
            assert!(
                (vantage.frame_x - 0.5).abs() >= 0.06,
                "{index}: {} centres its hero",
                vantage.hero
            );
            assert!(
                vantage.horizon >= 0.60,
                "{index}: horizon {} is not in the lower third",
                vantage.horizon
            );
            assert!(
                vantage.eye.0.abs() <= arch::COURT_EXTENT - 2.0
                    && vantage.eye.1.abs() <= arch::COURT_EXTENT - 2.0,
                "{index}: the eye stands off the edge of the world"
            );
            let reach = ((vantage.anchor.0 - vantage.eye.0).powi(2)
                + (vantage.anchor.1 - vantage.eye.1).powi(2))
            .sqrt();
            assert!(
                reach >= 8.0,
                "{index}: {} is a close-up at {reach:.1} tiles",
                vantage.hero
            );
        }
    }

    #[test]
    fn the_ride_sweeps_the_approach_road_before_it_settles() {
        // Far out: the camera runs the road up from the river crossing.
        let (map, view) = window(2, 19.0);
        let far = view3_from_ray(&map, &view, 96, 72);
        assert!(
            far.pos.y < -24.5 && (far.pos.x - VANTAGES[1].eye.0).abs() < 1.0,
            "a distant ride should retain the framed approach, got {:?}",
            (far.pos.x, far.pos.y)
        );
        // The approach moves gently before cutting to the destination mark.
        let reach = |camera: &View3| {
            ((camera.pos.x - VANTAGES[2].eye.0).powi(2)
                + (camera.pos.y - VANTAGES[2].eye.1).powi(2))
            .sqrt()
        };
        let (map, view) = window(2, 12.0);
        let mid = view3_from_ray(&map, &view, 96, 72);
        let (map, view) = window(2, 5.5);
        let settled = view3_from_ray(&map, &view, 96, 72);
        assert!(
            reach(&far) > reach(&mid) && reach(&mid) > reach(&settled),
            "the sweep must close on the mark: {:.1} → {:.1} → {:.1}",
            reach(&far),
            reach(&mid),
            reach(&settled)
        );
        assert!(reach(&settled) < 0.6, "arrival must land on the mark");
    }

    #[test]
    fn operator_yaw_and_pitch_turn_the_staged_camera() {
        let (map, mut view) = window(0, 5.5);
        let level = view3_from_ray(&map, &view, 96, 72);
        view.heading_rad += 0.5;
        let turned = view3_from_ray(&map, &view, 96, 72);
        let taken = wrap_pi(turned.heading_rad - level.heading_rad);
        assert!(
            taken > 0.44 && taken < 0.5,
            "a settled camera must follow operator yaw, took {taken:.3} of 0.5"
        );
        // Monotone with no saturation: a bigger turn is always a bigger turn,
        // or the arrow keys stop answering past some angle.
        view.heading_rad += 0.4;
        let further = view3_from_ray(&map, &view, 96, 72);
        assert!(
            wrap_pi(further.heading_rad - turned.heading_rad) > 0.30,
            "the swing must keep answering past half a radian"
        );
        view.heading_rad -= 0.4;
        view.heading_rad -= 0.5;
        view.bob = 10.0;
        let raised = view3_from_ray(&map, &view, 96, 72);
        assert!(
            (raised.pitch - level.pitch - 0.30).abs() < 1e-3,
            "the ride's bob must still ride in as pitch"
        );
        view.bob = 400.0;
        assert_eq!(
            view3_from_ray(&map, &view, 96, 72).pitch,
            0.90,
            "pitch must stay clamped"
        );
    }

    #[test]
    fn a_tall_pane_keeps_its_vertical_field_painterly() {
        let (map, view) = window(0, 5.5);
        for &(dot_w, dot_h) in &[(96usize, 72usize), (200, 304), (136, 164)] {
            let camera = view3_from_ray(&map, &view, dot_w, dot_h);
            let tan_v = (camera.fov_rad * 0.5).tan() * dot_h as f32 / dot_w as f32;
            let vertical = 2.0 * tan_v.atan();
            assert!(
                vertical <= VERT_FOV_MAX + 1e-3,
                "{dot_w}x{dot_h}: vertical fov {vertical:.3} blew the cap"
            );
            assert!(camera.fov_rad > 0.4, "{dot_w}x{dot_h}: fov collapsed");
        }
    }

    /// The probe camera has to *be* the settled ride camera, or a staging
    /// session tunes a picture nobody will ever see.
    #[test]
    fn a_probe_camera_matches_the_settled_ride() {
        for building in 0u8..8 {
            let (map, view) = window(building, 5.5);
            let live = view3_from_ray(&map, &view, 200, 304);
            let probe = settled_camera(&VANTAGES[building as usize], 200, 304);
            assert!(
                (live.pos.x - probe.pos.x).abs() < 0.9
                    && (live.pos.y - probe.pos.y).abs() < 0.9
                    && (live.heading_rad - probe.heading_rad).abs() < 0.05
                    && (live.pitch - probe.pitch).abs() < 1e-4
                    && (live.fov_rad - probe.fov_rad).abs() < 1e-4,
                "vantage {building}: probe {probe:?} drifted from the ride {live:?}"
            );
        }
    }

    /// Manual staging bench. Prints, for every mark in `VANTAGES` and for any
    /// candidate poses parked in `PROBE`, exactly the numbers the vista law is
    /// written in — sky / wall / ground, standoff, and where the hero's own
    /// silhouette lands across the frame. Restaging by editing the table and
    /// re-reading PNGs costs a render per guess; this costs one compile for a
    /// whole grid.
    ///
    /// `cargo test --bin angel probe_the_staging -- --ignored --nocapture`
    #[test]
    #[ignore = "manual staging bench: run with --nocapture to read the table"]
    fn probe_the_staging() {
        /// Candidate marks under consideration. Empty in a landed tree.
        const PROBE: &[Vantage] = &[];

        let report = |label: &str, vantage: &Vantage| {
            let camera = settled_camera(vantage, 200, 304);
            let scene = scene::scene_for(scene::SceneKey::COURT);
            let (sky, wall, ground) = raster::composition_mix(&scene, &camera, 200, 304);
            let (standoff, near_x, near_y) = camera_standoff(&camera);
            let tan_h = (camera.fov_rad * 0.5).tan();
            let bearing = (vantage.anchor.1 - camera.pos.y).atan2(vantage.anchor.0 - camera.pos.x);
            let at_x = 0.5 * (1.0 + wrap_pi(bearing - camera.heading_rad).tan() / tan_h);
            let reach = ((vantage.anchor.0 - vantage.eye.0).powi(2)
                + (vantage.anchor.1 - vantage.eye.1).powi(2))
            .sqrt();
            eprintln!(
                "{label:<12} eye=({:>6.1},{:>6.1}) h={:.1} lens={:.2} | sky={sky:.3} wall={wall:.3} ground={ground:.3} | standoff={standoff:.1}@({near_x:.1},{near_y:.1}) reach={reach:.1} hero_at={at_x:.2} hdg={:.1}° | {}",
                vantage.eye.0,
                vantage.eye.1,
                vantage.eye_h,
                vantage.lens,
                camera.heading_rad.to_degrees().rem_euclid(360.0),
                vantage.hero,
            );
        };
        for (index, vantage) in VANTAGES.iter().enumerate() {
            report(&format!("mark {index}"), vantage);
        }
        for (index, vantage) in PROBE.iter().enumerate() {
            report(&format!("probe {index}"), vantage);
        }
    }

    #[test]
    fn a_windowless_scene_still_produces_a_finite_camera() {
        // Interiors and the first frames of a long journey carry no landmark.
        let map = RayMap {
            width: 9,
            height: 9,
            cells: vec![0; 81],
            terrain: vec![0.0; 81],
            terrain_kind: vec![0; 81],
            sprites: Vec::new(),
        };
        let view = RayView {
            x: 4.0,
            y: 4.0,
            heading_rad: 2.0,
            look_yaw: 0.0,
            fov_rad: 1.05,
            bob: 0.0,
            eye_h: 0.0,
        };
        let camera = view3_from_ray(&map, &view, 96, 72);
        assert!(camera.pos.x.is_finite() && camera.pos.y.is_finite() && camera.pos.z.is_finite());
        assert!(camera.heading_rad.is_finite() && camera.pitch.is_finite());
    }
    #[test]
    fn travel_heading_cannot_replace_intentional_camera_look() {
        let (map, mut view) = window(2, 13.0);
        let level = view3_from_ray(&map, &view, 96, 72);
        view.heading_rad += 2.0;
        let automatic_turn = view3_from_ray(&map, &view, 96, 72);
        assert_eq!(level.heading_rad, automatic_turn.heading_rad);
        view.look_yaw = 0.4;
        let intentional_turn = view3_from_ray(&map, &view, 96, 72);
        assert!((wrap_pi(intentional_turn.heading_rad - level.heading_rad) - 0.4).abs() < 1e-4);
        view.bob = 10.0;
        let raised = view3_from_ray(&map, &view, 96, 72);
        assert!((raised.pitch - intentional_turn.pitch - 0.30).abs() < 1e-4);
    }
}
