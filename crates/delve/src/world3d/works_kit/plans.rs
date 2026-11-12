//! Every plan of the hillside as a few solids in the monolith language, in
//! its own frame (front toward `+z`). See the parent module.

use std::f32::consts::{PI, TAU};

use angel_stage::set::Kind;

use super::W;
use crate::render::overworld::works_kit::Load;

/// Plan `id` drawn into `b`: its finished body, running when `b.work`.
pub(super) fn draw(id: &str, b: &mut W) -> Option<()> {
    match id {
        "field-quarry" => field_quarry(b),
        "rubble-pit" => rubble_pit(b),
        "claim-quarry" => claim_quarry(b),
        "banker" => banker(b),
        "tracing-floor" => tracing_floor(b),
        "slater" => slater(b),
        "lime-kiln" => lime_kiln(b),
        "slaking-pit" => slaking_pit(b),
        "mortar-mill" => mortar_mill(b),
        "sand-pit" => sand_pit(b),
        "well" => well(b),
        "intake" => intake(b),
        "clamp" => clamp(b),
        "woodsman-camp" => woodsman_camp(b),
        "sawpit" => sawpit(b),
        "carpentry" => carpentry(b),
        "wheelwright" => wheelwright(b),
        "retting-pond" => retting_pond(b),
        "ropewalk" => ropewalk(b),
        "farmstead" => farmstead(b),
        "mill" => mill(b),
        "bakery" => bakery(b),
        "brewery" => brewery(b),
        "pasture" => pasture(b),
        "tanyard" => tanyard(b),
        "weaving-shed" => weaving_shed(b),
        "ashery" => ashery(b),
        "glassworks" => glassworks(b),
        "bloomery" => bloomery(b),
        "forge-shop" => forge_shop(b),
        "smithy" => smithy(b),
        "lead-works" => lead_works(b),
        "lodge" => lodge(b),
        "ditch-gang" => ditch_gang(b),
        "depot" => depot(b),
        "carters-yard" => super::yards::carters_yard(b),
        "castle-yard" => super::yards::castle_yard(b),
        "camp" => super::yards::camp(b),
        "shaft-hand" => super::yards::shaft_hand(b),
        "shaft-windlass" => super::yards::shaft_windlass(b),
        "shaft-gin" => super::yards::shaft_gin(b),
        "shaft-water" => super::yards::shaft_water(b),
        "shaft-ley" => super::yards::shaft_ley(b),
        "quay" => quay(b),
        "hut" => hut(b),
        "treadwheel-crane" => treadwheel_crane(b),
        "treadwheel" => treadwheel(b),
        "horse-gin" => horse_gin(b),
        "windmill" => windmill(b),
        "cottage" => cottage(b),
        "mason" => mason_yard(b),
        "warehouse" => warehouse(b),
        "caravanserai" => caravanserai(b),
        "wall" => wall(b, Kind::Stone),
        "clay-pit" => clay_pit(b),
        "brick-kiln" => brick_kiln(b),
        "brick-cottage" => brick_cottage(b),
        "brick-wall" => wall(b, Kind::Brick),
        "brick-gate" => brick_gate(b),
        "waterwheel" => waterwheel(b),
        "drive-shaft" => drive_shaft(b),
        "leyline" => leyline(b),
        "mana-engine" => mana_engine(b),
        "conduit" => conduit(b),
        "enchanter" => enchanter(b),
        "forge" => royal_forge(b),
        "hod-gang" => super::gaps::hod_gang(b),
        "great-crane" => super::gaps::great_crane(b),
        "vineyard" => super::gaps::vineyard(b),
        "wine-press" => super::gaps::wine_press(b),
        "aqueduct" => super::gaps::aqueduct(b),
        "swape" => super::gaps::swape(b),
        "palisade" => palisade(b),
        "wood-gate" => wood_gate(b),
        "watchtower" => watchtower(b),
        "brazier" => brazier(b),
        _ => return None,
    }
    Some(())
}

// -------------------------------------------------------------- helpers ----

/// A hall: a plinth, a body of `wall` over `x` by `z` to height `h`, a roof of
/// `roof` on it (`roof_h` high; hipped or gabled, its ridge along x or z),
/// a door slot in the front, lit slits along it. Returns the wall's top.
#[allow(clippy::too_many_arguments)]
fn hall(b: &mut W, x: [f32; 2], z: [f32; 2], wall: Kind, h: f32, roof: Kind, roof_h: f32, hipped: bool, ridge_z: bool) -> f32 {
    let y0 = b.plinth(Kind::Stone, x, z, &[(2.0, 1.4)]);
    b.bx(wall, x, [y0, y0 + h], z);
    // A string course and the heavy eave.
    b.bx(Kind::Stone, [x[0] - 1.0, x[1] + 1.0], [y0 + h, y0 + h + 1.6], [z[0] - 1.0, z[1] + 1.0]);
    let top = y0 + h + 1.6;
    if hipped {
        b.hip(roof, [x[0] - 1.0, x[1] + 1.0], [z[0] - 1.0, z[1] + 1.0], top, roof_h);
    } else {
        b.gable(roof, [x[0] - 1.0, x[1] + 1.0], [z[0] - 1.0, z[1] + 1.0], top, roof_h, ridge_z);
    }
    let cx = (x[0] + x[1]) / 2.0;
    b.door(cx, z[1], y0, [(x[1] - x[0]) * 0.18, h * 0.62]);
    let n = (((x[1] - x[0]) / 12.0).floor() as i32).clamp(1, 4);
    for k in 0..n {
        let sx = x[0] + (k as f32 + 0.5) * (x[1] - x[0]) / n as f32;
        if (sx - cx).abs() > (x[1] - x[0]) * 0.16 {
            b.slit(sx, z[1], y0 + h * 0.5, [1.8, h * 0.32]);
        }
    }
    top
}

/// A chimney stack standing on `(cx, cz)`, `r` across, from `y0` to `y1`.
fn stack(b: &mut W, cx: f32, cz: f32, r: f32, y0: f32, y1: f32) {
    b.bx(Kind::Stone, [cx - r, cx + r], [y0, y1], [cz - r, cz + r]);
    b.bx(Kind::Stone, [cx - r - 0.8, cx + r + 0.8], [y1 - 1.5, y1], [cz - r - 0.8, cz + r + 0.8]);
}

/// A barrow: a tray on one wheel, handles behind.
fn barrow(b: &mut W, x: f32, z: f32, load: Option<Load>) {
    b.bx(Kind::Floor, [x, x + 9.0], [2.5, 4.5], [z - 2.5, z + 2.5]);
    b.bx(Kind::Floor, [x - 5.0, x], [3.0, 3.8], [z - 2.5, z - 1.7]);
    b.bx(Kind::Floor, [x - 5.0, x], [3.0, 3.8], [z + 1.7, z + 2.5]);
    b.cyl(Kind::Metal, x + 9.5, z, 2.2, 0.0, 1.0);
    if let Some(load) = load {
        b.heap(load, x + 4.5, z, 7.0, 4.5, 4.0);
    }
}

/// A pit's floor, its rim of ground on three sides and a low lip before it so the hole shows.
fn hole(b: &mut W, rim: Kind, floor: Kind) {
    let (w, d) = (b.w, b.d);
    b.bx(Kind::Mud, [4.0, w - 8.0], [0.0, 1.2], [5.0, d - 12.0]);
    b.bx(floor, [6.0, w - 10.0], [1.2, 1.9], [7.0, d - 14.0]);
    for (x0, x1, z0, z1, h) in [(2.0, w - 6.0, 2.0, 6.0, 6.0), (2.0, 6.0, 4.0, d - 11.0, 5.0), (w - 10.0, w - 6.0, 4.0, d - 11.0, 5.0), (2.0, w - 6.0, d - 13.0, d - 10.0, 2.0)] {
        b.bx(rim, [x0, x1], [0.0, h], [z0, z1]);
    }
}

fn rubble_pit(b: &mut W) {
    let (w, d) = (b.w, b.d);
    hole(b, Kind::Ground, Kind::Rock);
    // Tossed stones on the rim, a broken heap of rubble, a barrow of it.
    for k in 0..6 {
        let a = k as f32 / 6.0 * TAU;
        b.rock(Kind::Rock, 14.0 + a.cos() * 11.0, 11.0 + a.sin() * 5.0, 5.0, 5.0 + (k % 2) as f32 * 1.5, 3.0, 40 + k);
    }
    for k in 0..7 {
        let (x, z) = (w - 12.0 + (k % 3) as f32 * 3.4 - (k / 3) as f32 * 1.5, d - 11.0 + (k / 3) as f32 * 3.0);
        b.rock(Kind::Rock, x, z, 4.6, (k / 3) as f32 * 3.0, 3.6 + (k % 2) as f32, 60 + k);
    }
    barrow(b, 8.0, d - 6.0, Some(Load::Rubble));
    if b.work {
        let s = (b.t % 8) as f32;
        b.rock(Kind::Rock, 14.0 + s * 1.6, 9.0 + s * (8.0 - s) * 0.5, 2.2, 2.0 + s * (8.0 - s) * 0.5, 2.0, 80);
        b.smoke(14.0, 4.0, 12.0, 8.0, 2);
    }
}

fn sand_pit(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A cut in a sandy bank: terraces of pale sand stepping down to the floor, a smooth cone of spoil, a spade.
    b.bx(Kind::Ground, [2.0, w - 2.0], [0.0, 3.0], [2.0, d - 14.0]);
    for (k, (x0, x1)) in [(3.0, w - 3.0), (6.0, w - 8.0), (9.0, w - 13.0)].into_iter().enumerate() {
        b.bx(Kind::Sand, [x0, x1], [3.0 + k as f32 * 3.0, 6.0 + k as f32 * 3.0], [3.0 + k as f32 * 3.0, 9.0 + k as f32 * 3.0]);
    }
    b.bx(Kind::Sand, [6.0, w - 8.0], [0.0, 2.0], [14.0, d - 10.0]);
    b.cone(Kind::Sand, w - 9.0, d - 9.0, 8.5, 0.0, 12.0);
    b.bx(Kind::Floor, [14.0, 15.0], [1.0, 12.0], [d - 16.0, d - 15.0]);
    b.bx(Kind::Metal, [13.0, 16.0], [12.0, 14.0], [d - 17.0, d - 14.0]);
    barrow(b, 6.0, d - 6.0, Some(Load::Sand));
    if b.work {
        let s = (b.t % 8) as f32;
        b.bx(Kind::Sand, [14.0 + s * 1.5, 15.6 + s * 1.5], [4.0 + s * (8.0 - s) * 0.5, 5.6 + s * (8.0 - s) * 0.5], [d - 15.0, d - 13.4]);
        b.smoke(18.0, 6.0, 14.0, 9.0, 2);
    }
}

fn clay_pit(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A wet pit: a red rim of clay round a pool of standing water; bricks cut and set out to dry on a rack.
    hole(b, Kind::Brick, Kind::Mud);
    b.bx(Kind::Water, [8.0, w - 12.0], [1.9, 2.5], [9.0, d - 17.0]);
    for (k, row) in [0.0, 3.2, 6.4].into_iter().enumerate() {
        for c in 0..4 {
            b.bx(Kind::Brick, [w - 14.0 + c as f32 * 3.3, w - 11.2 + c as f32 * 3.3], [1.6 + k as f32 * 0.2, 4.4 + k as f32 * 0.2], [d - 11.0 + row, d - 8.4 + row]);
        }
    }
    b.bx(Kind::Floor, [w - 15.0, w - 1.0], [0.0, 1.6], [d - 12.0, d - 1.0]);
    barrow(b, 6.0, d - 6.0, Some(Load::Clay));
    if b.work {
        for k in 0..3 {
            let age = ((b.t + k * 3) % 8) as f32;
            b.ball(Kind::Plaster, 14.0 + k as f32 * 5.0, 3.0 + age * 0.6, 12.0 + (k % 2) as f32 * 3.0, 0.8, 0.8, 0.8);
        }
    }
}

// ---------------------------------------------------------------- stone ----

fn field_quarry(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // Three benches of raw rock stepping down toward the front, caps pale.
    for (x, z, h, ww, dd) in [(3.0, 2.0, 24.0, w - 6.0, 10.0), (3.0, 10.0, 15.0, 26.0, 10.0), (w - 20.0, 12.0, 9.0, 17.0, 10.0)] {
        b.bx(Kind::Rock, [x, x + ww], [0.0, h], [z, z + dd]);
        b.bx(Kind::Stone, [x, x + ww], [h - 1.4, h], [z, z + dd]);
        b.rock(Kind::Rock, x + ww * 0.3, z + dd - 2.0, 9.0, h - 0.5, 4.0, (x as u32) + 5);
    }
    for k in 0..5 {
        let x = w - 18.0 + k as f32 * 3.0;
        b.bx(Kind::Metal, [x, x + 1.0], [9.0, 10.6], [14.0, 15.0]);
    }
    b.heap(Load::Stone, w - 12.0, d - 5.0, 16.0, 0.0, 10.0);
    b.heap(Load::Rubble, 11.0, d - 4.0, 11.0, 0.0, 6.0);
    b.smoke(w * 0.55, 12.0, 14.0, 10.0, 3);
}

fn claim_quarry(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w / 2.0, d * 0.5);
    // The collar: a square of dressed stone round the black shaft.
    b.plinth(Kind::Stone, [cx - 12.0, cx + 12.0], [cz - 12.0, cz + 12.0], &[(3.0, 2.0), (3.0, 0.0)]);
    b.bx(Kind::Door, [cx - 8.0, cx + 8.0], [5.9, 6.2], [cz - 8.0, cz + 8.0]);
    // The headframe: four legs leaning to one head, a sheave on it.
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        for k in 0..6 {
            let s = k as f32 / 6.0;
            let (x, z) = (cx + sx * (11.0 - 9.0 * s), cz + sz * (11.0 - 9.0 * s));
            b.bx(Kind::Floor, [x - 0.9, x + 0.9], [6.0 + s * 40.0, 6.0 + (s + 0.17) * 40.0], [z - 0.9, z + 0.9]);
        }
    }
    b.bx(Kind::Floor, [cx - 3.0, cx + 3.0], [46.0, 49.0], [cz - 3.0, cz + 3.0]);
    b.wheel(Kind::Metal, cx, 51.0, cz, 3.0, b.turn(), 4, 2.0);
    // The bucket on its rope: down into the dark and up with rock.
    let up = if b.work { ((b.turn().cos() + 1.0) * 14.0).round() } else { 20.0 };
    b.bx(Kind::Cloth, [cx - 0.4, cx + 0.4], [6.0 + up + 6.0, 51.0], [cz - 0.4, cz + 0.4]);
    b.bx(Kind::Floor, [cx - 3.0, cx + 3.0], [6.0 + up, 6.0 + up + 6.0], [cz - 3.0, cz + 3.0]);
    // The windlass drum, the charter banner, rough blocks for the cart.
    b.cyl(Kind::Floor, w - 6.0, cz, 3.0, 5.0, 6.0);
    b.bx(Kind::Floor, [4.0, 5.4], [0.0, 34.0], [4.0, 5.4]);
    b.bx(Kind::Cloth, [5.4, 17.0], [24.0, 33.0], [4.2, 5.2]);
    b.heap(Load::Stone, w - 10.0, d - 5.0, 14.0, 0.0, 9.0);
}

fn banker(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A shelter: four posts, a lean roof of shingle over a stone bench.
    for (x, z) in [(4.0, 4.0), (w - 6.0, 4.0), (4.0, d - 5.0), (w - 6.0, d - 5.0)] {
        b.bx(Kind::Floor, [x, x + 1.8], [0.0, 22.0], [z, z + 1.8]);
    }
    b.wedge(Kind::Roof, [2.0, w - 2.0], [3.0, d - 3.0], 22.0, 7.0);
    b.bx(Kind::Stone, [10.0, w - 12.0], [0.0, 9.0], [d * 0.35, d * 0.35 + 12.0]);
    b.bx(Kind::Plaster, [15.0, 27.0], [9.0, 16.0], [d * 0.35 + 2.0, d * 0.35 + 10.0]);
    for k in 0..3 {
        b.bx(Kind::Floor, [6.0 + k as f32 * 3.0, 8.0 + k as f32 * 3.0], [8.0, 17.0 + k as f32 * 2.0], [4.0, 4.8]);
    }
    b.heap(Load::Ashlar, w - 10.0, d - 5.0, 14.0, 0.0, 9.0);
    b.heap(Load::Stone, 9.0, d - 4.0, 10.0, 0.0, 6.0);
    b.smoke(21.0, 16.0, d * 0.35 + 6.0, 8.0, 2);
}

fn tracing_floor(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A raised bed of pale plaster edged in stone, the figures scribed in it as shallow grooves.
    b.bx(Kind::Stone, [2.0, w - 2.0], [0.0, 2.4], [2.0, d - 2.0]);
    b.bx(Kind::Plaster, [3.4, w - 3.4], [2.4, 3.0], [3.4, d - 3.4]);
    for r in [6.0f32, 11.0, 16.0] {
        for k in 0..16 {
            let a = k as f32 / 16.0 * TAU;
            let (x, z) = (w * 0.64 + a.cos() * r, d * 0.5 + a.sin() * r * 0.8);
            b.bx(Kind::Door, [x - 0.5, x + 0.5], [3.0, 3.3], [z - 0.5, z + 0.5]);
        }
    }
    // The compass at work: a standing arm and a sweeping leg.
    let s = if b.work { b.turn() } else { 2.4 };
    let (px, pz) = (w * 0.64, d * 0.5);
    b.bx(Kind::Metal, [px - 0.5, px + 0.5], [3.0, 18.0], [pz - 0.5, pz + 0.5]);
    for k in 1..8 {
        let f = k as f32 / 8.0;
        let (x, z) = (px + s.cos() * 16.0 * f, pz + s.sin() * 13.0 * f);
        b.bx(Kind::Metal, [x - 0.5, x + 0.5], [18.0 - 15.0 * f, 18.6 - 15.0 * f], [z - 0.5, z + 0.5]);
    }
    // The trestle table, template boards leaning on it.
    b.bx(Kind::Floor, [6.0, 22.0], [8.0, 9.4], [d - 14.0, d - 6.0]);
    for x in [7.0, 20.0] {
        b.bx(Kind::Floor, [x, x + 1.4], [3.0, 8.0], [d - 13.0, d - 12.0]);
    }
    for k in 0..3 {
        b.bx(Kind::Floor, [24.0 + k as f32 * 3.0, 25.4 + k as f32 * 3.0], [3.0, 11.0 + k as f32], [d - 9.0, d - 7.6]);
    }
}

fn slater(b: &mut W) {
    let (w, d) = (b.w, b.d);
    for x in [3.0, w - 5.0] {
        b.bx(Kind::Floor, [x, x + 1.8], [0.0, 20.0], [4.0, 5.8]);
        b.bx(Kind::Floor, [x, x + 1.8], [0.0, 20.0], [d - 6.0, d - 4.2]);
    }
    b.wedge(Kind::Slate, [1.0, w - 1.0], [3.0, d - 3.0], 20.0, 7.0);
    b.bx(Kind::Floor, [8.0, 18.0], [0.0, 9.0], [d * 0.4, d * 0.4 + 7.0]);
    b.bx(Kind::Slate, [6.0, 20.0], [9.0, 12.0], [d * 0.4 - 1.0, d * 0.4 + 8.0]);
    b.heap(Load::Slate, w - 9.0, d - 7.0, 11.0, 0.0, 9.0);
    // Slates already hung on the shed's roof edge, a few leaning against it.
    for k in 0..3 {
        b.bx(Kind::Slate, [20.0 + k as f32, 21.0 + k as f32], [0.0, 14.0], [d - 10.0 + k as f32, d - 8.5 + k as f32]);
    }
    b.smoke(13.0, 12.0, d * 0.4 + 3.0, 6.0, 2);
}

fn mason_yard(b: &mut W) {
    let (w, d) = (b.w, b.d);
    b.fence(Kind::Floor, [2.0, w - 2.0], 4.0, 7.0);
    for (i, (x, z)) in [(5.0, 9.0), (19.0, 9.0), (33.0, 9.0), (7.0, 21.0), (21.0, 21.0)].into_iter().enumerate() {
        b.bx(if i % 2 == 0 { Kind::Stone } else { Kind::Slate }, [x, x + 11.0], [0.0, 6.0 + (i % 3) as f32 * 1.2], [z, z + 8.0]);
    }
    // A carved block on trestles and sheer legs over it.
    b.bx(Kind::Plaster, [34.0, 44.0], [5.0, 12.0], [22.0, 30.0]);
    for k in 0..3 {
        b.bx(Kind::Door, [35.0 + k as f32 * 3.0, 36.0 + k as f32 * 3.0], [11.0, 12.2], [23.0, 29.0]);
    }
    for x in [34.0, 43.0] {
        b.bx(Kind::Floor, [x, x + 1.4], [0.0, 5.0], [23.0, 29.0]);
    }
    let dy = if b.work { (b.turn().sin() * 1.5).round() } else { 0.0 };
    for sx in [30.0, 33.0] {
        b.bx(Kind::Floor, [sx, sx + 1.0], [0.0, 26.0], [d - 5.0, d - 4.0]);
    }
    b.bx(Kind::Cloth, [31.0, 32.0], [14.0 + dy, 26.0], [d - 4.7, d - 4.3]);
    b.bx(Kind::Stone, [30.0, 34.0], [10.0 + dy, 14.0 + dy], [d - 6.0, d - 3.0]);
    b.smoke(38.0, 12.0, 26.0, 8.0, 2);
}

// ---------------------------------------------------------------- fire ----

fn lime_kiln(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w * 0.5, d * 0.42);
    // A squat pot of rough stone dug into a bank of spoil, wider at its foot.
    b.bx(Kind::Ground, [1.0, w - 1.0], [0.0, 9.0], [2.0, d * 0.7]);
    b.frus(Kind::Rock, cx, cz, 11.5, 0.0, 30.0);
    b.cyl(Kind::Stone, cx, cz, 9.0, 30.0, 2.2);
    b.cyl(Kind::Door, cx, cz, 6.6, 31.5, 1.0);
    for y in [10.0, 20.0] {
        b.cyl(Kind::Metal, cx, cz, 10.6 - y * 0.05, y, 0.9);
    }
    // The draw-arch at its foot, a lintel of dressed stone over it.
    b.bx(Kind::Door, [cx - 4.0, cx + 4.0], [0.0, 9.0], [cz + 9.5, cz + 11.0]);
    b.bx(Kind::Stone, [cx - 6.0, cx + 6.0], [9.0, 11.0], [cz + 8.0, cz + 11.5]);
    b.fire(cx, 32.5, cz, 6.0);
    if b.work {
        b.bx(Kind::Fire, [cx - 3.0, cx + 3.0], [2.0, 6.0], [cz + 9.6, cz + 10.2]);
        b.lamp([cx, 4.0, cz + 14.0], 30.0, 0.7);
    }
    b.smoke(cx, 36.0, cz, 22.0, 4);
    b.heap(Load::Ashlar, 7.0, d - 5.0, 11.0, 0.0, 7.0);
    b.heap(Load::Lime, w - 8.0, d - 5.0, 11.0, 0.0, 7.0);
}

fn clamp(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w * 0.5, d * 0.42);
    // A dome of turf over stacked wood, a chimney hole at the crown, vents low round it.
    b.bx(Kind::Mud, [2.0, w - 2.0], [0.0, 0.7], [3.0, d - 6.0]);
    b.ball(Kind::Hedge, cx, 8.0, cz, 13.0, 8.0, 11.0);
    b.bx(Kind::Door, [cx - 2.0, cx + 2.0], [16.4, 17.0], [cz - 2.0, cz + 2.0]);
    for (x, z) in [(cx - 12.0, cz + 2.0), (cx + 12.0, cz + 2.0), (cx, cz + 9.5)] {
        b.bx(if b.work { Kind::Fire } else { Kind::Door }, [x - 1.2, x + 1.2], [2.0, 4.0], [z - 0.8, z + 0.8]);
    }
    if b.work {
        b.lamp([cx, 6.0, cz + 11.0], 34.0, 0.65);
    }
    b.smoke(cx, 18.0, cz, 18.0, 5);
    // Log ends ringing the foot, a collier's lean-to, sacks of charcoal.
    for k in 0..6 {
        b.cyl(Kind::Trunk, 4.0 + k as f32 * 4.6, d - 9.0, 1.8, 0.0, 2.6);
    }
    b.wedge(Kind::Thatch, [1.0, 8.0], [d - 14.0, d - 3.0], 0.0, 9.0);
    b.heap(Load::Charcoal, w - 7.0, d - 5.0, 9.0, 0.0, 7.0);
}

fn bloomery(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w * 0.5, d * 0.42);
    // A tall clay stack, hooped with iron, narrowing as it climbs.
    b.plinth(Kind::Stone, [cx - 6.0, cx + 6.0], [cz - 6.0, cz + 6.0], &[(2.5, 2.0)]);
    b.frus(Kind::Brick, cx, cz, 8.0, 2.5, 40.0);
    for y in [10.0, 20.0, 30.0] {
        b.cyl(Kind::Metal, cx, cz, 7.2 - y * 0.05, y, 0.9);
    }
    b.cyl(Kind::Stone, cx, cz, 4.2, 42.5, 1.5);
    b.fire(cx, 44.0, cz, 6.0);
    b.smoke(cx, 46.0, cz, 18.0, 4);
    // The tuyere hole, and a bellows of boards and leather pumping at it.
    b.bx(if b.work { Kind::Fire } else { Kind::Door }, [cx - 2.0, cx + 2.0], [4.0, 8.0], [cz + 6.6, cz + 7.6]);
    let pump = if b.work { ((b.turn().sin() + 1.0) * 1.8).round() } else { 0.0 };
    b.bx(Kind::Floor, [cx - 12.0, cx - 4.0], [3.0, 4.0], [cz + 7.0, cz + 15.0]);
    b.bx(Kind::Cloth, [cx - 11.5, cx - 4.5], [4.0, 6.0 + pump], [cz + 7.5, cz + 14.5]);
    b.bx(Kind::Floor, [cx - 12.0, cx - 4.0], [6.0 + pump, 7.0 + pump], [cz + 7.0, cz + 15.0]);
    b.bx(Kind::Metal, [cx - 4.0, cx - 1.0], [4.4, 5.4], [cz + 10.5, cz + 11.5]);
    b.heap(Load::Ore, w - 7.0, d - 7.0, 9.0, 0.0, 7.0);
    b.heap(Load::Charcoal, 7.0, d - 6.0, 9.0, 0.0, 6.0);
}

fn ashery(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A burn heap of ash and brush at the back, a rack of leaching tubs beside it.
    b.ball(Kind::Mud, 10.0, 5.0, d * 0.4, 8.0, 5.0, 7.0);
    for k in 0..4 {
        b.bx(if b.work { Kind::Fire } else { Kind::Rock }, [6.0 + k as f32 * 2.0, 7.4 + k as f32 * 2.0], [8.0, 9.2], [d * 0.4 - 1.0 + (k % 2) as f32, d * 0.4 + 0.5 + (k % 2) as f32]);
    }
    if b.work {
        b.lamp([10.0, 10.0, d * 0.4], 22.0, 0.55);
    }
    b.smoke(10.0, 9.0, d * 0.4, 14.0, 4);
    for x in [w - 14.0, w - 6.0] {
        b.bx(Kind::Floor, [x - 3.0, x + 3.0], [8.0, 9.0], [d * 0.35, d * 0.35 + 6.0]);
        b.cyl(Kind::Floor, x, d * 0.35 + 3.0, 3.0, 9.0, 8.0);
        b.cyl(Kind::Metal, x, d * 0.35 + 3.0, 3.1, 12.0, 0.8);
        b.bx(Kind::Floor, [x - 3.0, x - 2.0], [0.0, 8.0], [d * 0.35, d * 0.35 + 1.0]);
        b.bx(Kind::Floor, [x + 2.0, x + 3.0], [0.0, 8.0], [d * 0.35 + 5.0, d * 0.35 + 6.0]);
        b.cyl(Kind::Metal, x, d * 0.35 + 8.0, 2.2, 0.0, 3.0);
    }
    b.heap(Load::Glass, w - 9.0, d - 5.0, 9.0, 0.0, 6.0);
}

fn glassworks(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A beehive furnace of clay brick, glory holes in its flank, a tall brick cone beside it.
    b.plinth(Kind::Stone, [8.0, 32.0], [3.5, 3.5 + 24.0], &[(2.0, 1.5)]);
    b.ball(Kind::Brick, 20.0, 18.0, 3.5 + 12.0, 13.0, 14.0, 13.0);
    for (i, x) in [12.0, 20.0, 28.0].into_iter().enumerate() {
        b.bx(if b.work { Kind::Fire } else { Kind::Door }, [x - 1.8, x + 1.8], [6.0 + (i % 2) as f32 * 3.0, 9.5 + (i % 2) as f32 * 3.0], [3.5 + 23.0, 3.5 + 24.5]);
    }
    b.frus(Kind::Brick, 40.0, 3.5 + 6.0, 4.4, 0.0, 44.0);
    b.cyl(Kind::Stone, 40.0, 3.5 + 6.0, 3.0, 44.0, 1.5);
    b.smoke(40.0, 46.0, 3.5 + 6.0, 16.0, 4);
    if b.work {
        b.lamp([20.0, 8.0, 3.5 + 28.0], 32.0, 0.75);
        // The gather on the pipe: a swelling bead of glass.
        let r = 1.4 + (b.turn().sin() + 1.0) * 0.8;
        b.bx(Kind::Metal, [10.0, 20.0], [9.0, 9.6], [3.5 + 25.0, 3.5 + 25.6]);
        b.ball(Kind::Fire, 8.0, 9.4, 3.5 + 25.3, r, r, r);
    }
    b.heap(Load::Sand, 8.0, d - 4.0, 12.0, 0.0, 7.0);
    b.heap(Load::Glass, w - 8.0, d - 4.0, 10.0, 0.0, 8.0);
}

fn smithy(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // An open-fronted shed: back wall, two side walls, a lean roof, a stone stack, a hearth.
    b.bx(Kind::Floor, [2.0, w - 2.0], [0.0, 17.0], [3.0, 5.0]);
    b.bx(Kind::Floor, [2.0, 4.0], [0.0, 17.0], [3.0, d - 5.0]);
    b.bx(Kind::Floor, [w - 4.0, w - 2.0], [0.0, 17.0], [3.0, d - 5.0]);
    b.wedge(Kind::Roof, [1.0, w - 1.0], [2.0, d - 3.0], 17.0, 6.0);
    stack(b, w - 8.0, 8.0, 3.0, 0.0, 30.0);
    b.bx(Kind::Stone, [w - 14.0, w - 5.0], [0.0, 7.0], [5.0, 12.0]);
    b.bx(if b.work { Kind::Fire } else { Kind::Door }, [w - 12.0, w - 7.0], [7.0, 8.4], [6.0, 11.0]);
    if b.work {
        b.lamp([w - 9.0, 11.0, 10.0], 30.0, 0.75);
    }
    b.smoke(w - 8.0, 31.0, 8.0, 14.0, 3);
    // The anvil on its stump, the quench trough, tools hung on the back wall.
    b.bx(Kind::Trunk, [8.0, 13.0], [0.0, 7.0], [14.0, 19.0]);
    b.bx(Kind::Metal, [6.0, 15.0], [7.0, 10.0], [14.0, 19.0]);
    b.bx(Kind::Floor, [w - 12.0, w - 4.0], [0.0, 4.0], [d - 11.0, d - 6.0]);
    b.bx(Kind::Water, [w - 11.0, w - 5.0], [3.6, 4.0], [d - 10.0, d - 7.0]);
    for k in 0..3 {
        b.bx(Kind::Metal, [6.0 + k as f32 * 4.0, 7.0 + k as f32 * 4.0], [9.0, 14.0 - k as f32], [5.0, 5.8]);
    }
    if b.work && b.t % 4 < 2 {
        b.bx(Kind::Fire, [10.0, 11.5], [10.4, 11.4], [15.0, 16.5]);
    }
}

fn forge_shop(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A closed stone hall, a slate roof, a wide arched bay with the hearth blazing in it.
    let top = hall(b, [3.0, w - 11.0], [4.0, d - 4.0], Kind::Stone, 26.0, Kind::Slate, 9.0, false, false);
    b.arch(Kind::Stone, (3.0 + w - 11.0) / 2.0 - 4.0, d - 4.0, 18.0, 20.0, 3.0, 2.0);
    b.bx(if b.work { Kind::Fire } else { Kind::Door }, [8.0, 24.0], [3.0, 18.0], [d - 6.0, d - 5.2]);
    if b.work {
        b.lamp([16.0, 10.0, d - 1.0], 42.0, 0.9);
    }
    stack(b, w - 22.0, 9.0, 4.0, top - 2.0, top + 20.0);
    b.smoke(w - 22.0, top + 21.0, 9.0, 16.0, 4);
    // The tilt-hammer on the right: a post, a long helve, the iron head over an anvil.
    let swing = if b.work { ((b.turn().sin() + 1.0) * 3.0).round() } else { 4.0 };
    b.bx(Kind::Floor, [w - 8.0, w - 4.0], [0.0, 16.0], [d - 14.0, d - 10.0]);
    b.bx(Kind::Floor, [w - 24.0, w - 4.0], [14.0 + swing * 0.3, 15.5 + swing * 0.3], [d - 13.0, d - 11.0]);
    b.bx(Kind::Metal, [w - 27.0, w - 22.0], [11.0 + swing, 15.0 + swing], [d - 14.0, d - 10.0]);
    b.bx(Kind::Metal, [w - 28.0, w - 21.0], [3.0, 7.0], [d - 14.0, d - 10.0]);
    b.heap(Load::Iron, 8.0, d - 3.0, 9.0, 0.0, 6.0);
}

fn lead_works(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A low stone hearth under a conical hood and flue, moulds in the sand beside it.
    b.bx(Kind::Stone, [3.0, 21.0], [0.0, 9.0], [4.0, 19.0]);
    b.frus(Kind::Metal, 12.0, 11.0, 7.0, 9.0, 18.0);
    b.cyl(Kind::Metal, 12.0, 11.0, 2.0, 27.0, 8.0);
    b.bx(if b.work { Kind::Fire } else { Kind::Door }, [8.0, 16.0], [3.0, 7.0], [18.4, 19.4]);
    if b.work {
        b.lamp([12.0, 5.0, 22.0], 26.0, 0.7);
    }
    b.smoke(12.0, 36.0, 11.0, 14.0, 4);
    for k in 0..3 {
        b.bx(Kind::Metal, [24.0, 30.0], [0.0, 1.4], [8.0 + k as f32 * 6.0, 12.0 + k as f32 * 6.0]);
        if b.work && (b.t % 3) as usize == k {
            b.bx(Kind::Fire, [25.0, 29.0], [1.4, 1.8], [9.0 + k as f32 * 6.0, 11.0 + k as f32 * 6.0]);
        }
    }
    b.heap(Load::Ore, 8.0, d - 5.0, 10.0, 0.0, 6.0);
    b.heap(Load::Lead, w - 7.0, d - 5.0, 9.0, 0.0, 6.0);
}

fn brick_kiln(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A stack of unfired brick on a plastered base, fire-holes along its foot, a brick chimney.
    b.bx(Kind::Brick, [3.0, w - 14.0], [0.0, 22.0], [5.0, d - 6.0]);
    b.bx(Kind::Thatch, [2.0, w - 13.0], [22.0, 24.5], [4.0, d - 5.0]);
    for (i, x) in [10.0, 19.0, 28.0].into_iter().enumerate() {
        b.bx(if b.work { Kind::Fire } else { Kind::Door }, [x - 2.4, x + 2.4], [1.0, 7.0], [d - 7.0, d - 5.8]);
        b.smoke(x, 25.0, d * 0.5, 9.0, 2);
        let _ = i;
    }
    if b.work {
        b.lamp([w * 0.4, 5.0, d - 3.0], 40.0, 0.8);
    }
    stack(b, w - 7.0, d * 0.45, 3.2, 0.0, 44.0);
    b.smoke(w - 7.0, 45.0, d * 0.45, 16.0, 4);
    b.heap(Load::Brick, w - 6.0, d - 4.0, 9.0, 0.0, 8.0);
}

fn royal_forge(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A grand hall of dark stone, a steep slate roof, two chimneys, the great doors thrown wide on the fire.
    let top = hall(b, [3.0, w - 3.0], [4.0, d - 4.0], Kind::Stone, 28.0, Kind::Slate, 11.0, false, false);
    b.arch(Kind::Stone, w / 2.0, d - 4.0, 22.0, 24.0, 3.0, 2.0);
    b.bx(if b.work { Kind::Fire } else { Kind::Door }, [w / 2.0 - 8.0, w / 2.0 + 8.0], [3.0, 22.0], [d - 6.0, d - 5.2]);
    if b.work {
        b.lamp([w / 2.0, 12.0, d + 2.0], 52.0, 1.0);
    }
    for x in [12.0, w - 12.0] {
        stack(b, x, 10.0, 4.0, top - 2.0, top + 22.0);
        b.smoke(x, top + 23.0, 10.0, 16.0, 4);
    }
    // The royal banner on its staff, over the ridge.
    let wave = if b.work { (b.turn().sin() * 1.2).round() } else { 0.0 };
    b.bx(Kind::Floor, [w / 2.0 - 0.6, w / 2.0 + 0.6], [top, top + 22.0], [d * 0.5, d * 0.5 + 1.2]);
    b.bx(Kind::Cloth, [w / 2.0 + 0.6, w / 2.0 + 10.0 + wave], [top + 13.0, top + 21.0], [d * 0.5 + 0.3, d * 0.5 + 0.9]);
    for x in [9.0, w - 9.0] {
        b.bx(Kind::Metal, [x - 3.5, x + 3.5], [0.0, 3.0], [d - 4.0, d - 1.0]);
    }
}

// ----------------------------------------------------------------- wood ----

fn woodsman_camp(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A lean-to of poles under a roof of brush, stumps, a bundle of coppice poles standing.
    for k in 0..4 {
        b.bx(Kind::Trunk, [3.0 + k as f32 * 4.0, 4.4 + k as f32 * 4.0], [0.0, 20.0], [6.0, 7.4]);
    }
    b.wedge(Kind::Foliage, [1.0, 20.0], [5.0, 22.0], 18.0, 8.0);
    b.bx(Kind::Door, [4.0, 18.0], [0.0, 14.0], [8.0, 9.0]);
    for (x, z, h) in [(28.0, d - 8.0, 4.0), (38.0, d - 16.0, 3.0)] {
        b.cyl(Kind::Trunk, x, z, 3.5, 0.0, h);
        b.cyl(Kind::Floor, x, z, 2.4, h, 0.3);
    }
    for k in 0..6 {
        b.bx(Kind::Floor, [w - 11.0 + k as f32 * 1.6, w - 9.6 + k as f32 * 1.6], [0.0, 26.0 - k as f32], [10.0, 11.4]);
    }
    b.heap(Load::Timber, 25.0, d - 4.0, 15.0, 0.0, 9.0);
    if b.work {
        b.ball(Kind::Floor, 26.0 + (b.turn().sin() * 3.0).round(), 6.0 + (b.t % 3) as f32, d - 10.0, 1.2, 1.0, 1.2);
    }
}

fn sawpit(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // Two trestles, a log laid long across them, a boarded pit under it.
    b.bx(Kind::Mud, [10.0, w - 10.0], [0.0, 0.6], [d * 0.3, d * 0.3 + 14.0]);
    b.bx(Kind::Floor, [9.0, w - 9.0], [0.0, 3.0], [d * 0.3 - 1.0, d * 0.3]);
    b.bx(Kind::Floor, [9.0, w - 9.0], [0.0, 3.0], [d * 0.3 + 14.0, d * 0.3 + 15.0]);
    for x in [8.0, w - 10.0] {
        b.bx(Kind::Floor, [x, x + 2.0], [0.0, 14.0], [d * 0.3 + 3.0, d * 0.3 + 11.0]);
    }
    b.bx(Kind::Sand, [4.0, w - 4.0], [14.0, 21.0], [d * 0.3 + 3.0, d * 0.3 + 11.0]);
    b.bx(Kind::Floor, [w - 5.0, w - 4.0], [15.0, 20.0], [d * 0.3 + 4.0, d * 0.3 + 10.0]);
    // The saw: a long blade through the log, handles top and bottom, going up and down.
    let dy = if b.work { (b.turn().sin() * 3.0).round() } else { 0.0 };
    let sx = w * 0.5;
    b.bx(Kind::Plaster, [sx - 0.8, sx + 0.8], [4.0 + dy, 28.0 + dy], [d * 0.3 + 6.0, d * 0.3 + 8.0]);
    b.bx(Kind::Floor, [sx - 2.5, sx + 2.5], [28.0 + dy, 29.4 + dy], [d * 0.3 + 6.0, d * 0.3 + 8.0]);
    b.bx(Kind::Floor, [sx - 2.5, sx + 2.5], [3.0 + dy, 4.4 + dy], [d * 0.3 + 6.0, d * 0.3 + 8.0]);
    b.heap(Load::Planks, 10.0, d - 5.0, 14.0, 0.0, 8.0);
    b.heap(Load::Timber, w - 9.0, d - 5.0, 13.0, 0.0, 8.0);
    if b.work {
        b.ball(Kind::Floor, sx, 1.7, d * 0.3 + 14.5, 3.0 + (b.t % 4) as f32 * 0.3, 1.6, 2.0);
    }
}

fn carpentry(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A boarded shop with an open front under a shingled roof, joists racked in it; a vault's centring rib standing outside.
    let top = hall(b, [3.0, 32.0], [4.0, d - 6.0], Kind::Floor, 17.0, Kind::Roof, 8.0, false, false);
    for k in 0..5 {
        b.bx(Kind::Floor, [6.0, 28.0], [4.0 + k as f32 * 2.4, 5.0 + k as f32 * 2.4], [d - 8.0, d - 7.0]);
    }
    for k in 0..12 {
        let a = PI * k as f32 / 11.0;
        let (x, y) = (w - 9.0 + a.cos() * 7.5, 2.0 + a.sin() * 14.0);
        b.bx(Kind::Floor, [x - 0.9, x + 0.9], [y, y + 1.6], [d - 14.0, d - 12.4]);
    }
    b.bx(Kind::Floor, [w - 17.0, w - 1.0], [0.0, 1.4], [d - 14.0, d - 12.0]);
    b.heap(Load::Planks, 10.0, d - 2.5, 12.0, 0.0, 6.0);
    if b.work {
        b.smoke(18.0, top, d * 0.4, 7.0, 2);
    }
}

fn wheelwright(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A boarded shop under a lean roof, wheels hung on it; one on a stand in the yard with its tyre going on hot.
    let top = hall(b, [3.0, 30.0], [4.0, d - 6.0], Kind::Floor, 20.0, Kind::Roof, 7.0, true, false);
    b.wheel(Kind::Floor, 12.0, 12.0, d - 4.5, 6.0, 0.4, 8, 1.6);
    b.wheel(Kind::Floor, 22.0, 9.0, d - 4.5, 7.0, 1.1, 8, 1.6);
    b.bx(Kind::Floor, [w - 10.0, w - 8.0], [0.0, 13.0], [d - 11.0, d - 9.0]);
    b.wheel(Kind::Floor, w - 9.0, 14.0, d - 10.0, 7.0, b.turn() * 0.2, 8, 1.6);
    b.ball(if b.work { Kind::Fire } else { Kind::Metal }, w - 9.0, 14.0, d - 10.0, 8.2, 8.2, 0.8);
    if b.work {
        b.lamp([w - 9.0, 14.0, d - 6.0], 28.0, 0.7);
    }
    b.smoke(w - 9.0, 24.0, d - 10.0, 10.0, 3);
    b.bx(Kind::Floor, [5.0, 23.0], [0.0, 5.0], [d - 9.0, d - 3.0]);
    let _ = top;
}

fn ropewalk(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // The long low shed: a back wall, a thatch roof the whole length on posts, the lane clear under it.
    b.bx(Kind::Floor, [3.0, w - 3.0], [0.0, 10.0], [4.0, 6.0]);
    for k in 0..9 {
        let x = 4.0 + k as f32 * (w - 10.0) / 8.0;
        b.bx(Kind::Floor, [x, x + 1.8], [0.0, 13.0], [d - 8.0, d - 6.2]);
    }
    b.gable(Kind::Thatch, [1.0, w - 1.0], [3.0, d - 4.0], 12.0, 6.0, false);
    // Strands paying out along the lane, thin and pale; the traveller walks back as it lengthens.
    for k in 0..3 {
        b.bx(Kind::Thatch, [14.0, w - 16.0], [4.0 + k as f32 * 2.5, 4.8 + k as f32 * 2.5], [d - 15.0, d - 14.4]);
    }
    let back = if b.work { (b.turn().sin() * 3.0).round() } else { 0.0 };
    b.bx(Kind::Floor, [w - 17.0 + back, w - 12.0 + back], [1.0, 11.0], [d - 17.0, d - 12.0]);
    // The jack at the left: a great wheel with a handle going round.
    b.wheel(Kind::Floor, 10.0, 8.0, d - 14.0, 7.0, b.turn() * 0.5, 8, 1.6);
    // A twist of yarn running down the strands while they are spun.
    if b.work {
        for k in 0..3 {
            let x = 16.0 + ((b.t * 8 + k * 24) % 60) as f32;
            b.bx(Kind::Cloth, [x, x + 2.0], [4.0 + k as f32 * 2.5 - 0.4, 5.4 + k as f32 * 2.5], [d - 15.4, d - 13.6]);
        }
    }
    b.heap(Load::Rope, w - 8.0, d - 4.0, 12.0, 0.0, 7.0);
    b.heap(Load::Hemp, 8.0, d - 3.0, 10.0, 0.0, 6.0);
}

fn retting_pond(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A pond of dark water lowered into a rim of earth, sheaves of hemp sunk in it, a drying rack.
    b.bx(Kind::Ground, [2.0, w - 12.0], [0.0, 1.6], [3.0, d - 5.0]);
    b.bx(Kind::Water, [4.0, w - 14.0], [1.4, 1.9], [5.0, d - 7.0]);
    for (x, z) in [(10.0, 10.0), (20.0, 16.0), (28.0, 11.0), (14.0, 20.0)] {
        b.bx(Kind::Hedge, [x, x + 5.0], [1.9, 3.4], [z, z + 3.0]);
    }
    if b.work {
        for k in 0..4 {
            let age = ((b.t + k * 2) % 8) as f32;
            b.ball(Kind::Plaster, 11.0 + k as f32 * 7.0, 2.4 + age * 0.9, 12.0 + (k % 2) as f32 * 6.0, 0.8, 0.8, 0.8);
        }
    }
    for k in 0..4 {
        b.bx(Kind::Hedge, [w - 10.0 + k as f32 * 2.0, w - 8.6 + k as f32 * 2.0], [0.0, 16.0], [8.0, 9.4]);
    }
    b.bx(Kind::Floor, [w - 11.0, w - 1.0], [10.0, 11.0], [7.5, 9.9]);
    b.heap(Load::Hemp, 7.0, d - 3.0, 10.0, 0.0, 6.0);
}

// --------------------------------------------------------------- crafts ----

fn farmstead(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // The farmhouse and the barn at the back, furrowed strips of crop before them.
    hall(b, [3.0, 28.0], [4.0, 24.0], Kind::Stone, 12.0, Kind::Thatch, 8.0, true, false);
    hall(b, [34.0, w - 3.0], [4.0, 26.0], Kind::Floor, 15.0, Kind::Thatch, 9.0, false, true);
    for (x0, x1, z0, z1, k) in [(3.0, 30.0, 30.0, 40.0, Kind::Crop), (34.0, w - 3.0, 30.0, 40.0, Kind::Hedge), (3.0, w - 3.0, 42.0, d - 2.0, Kind::Crop)] {
        let n = ((x1 - x0) / 3.0) as i32;
        for r in 0..n {
            let x = x0 + r as f32 * 3.0;
            let sway = if b.work && ((r as u32 + b.t) % 8) < 3 { 0.8 } else { 0.0 };
            b.bx(k, [x, x + 1.8], [0.0, 3.0 + sway], [z0, z1]);
        }
    }
    b.heap(Load::Grain, w - 7.0, 32.0, 11.0, 0.0, 8.0);
    if b.work {
        b.smoke(24.0, 27.0, 14.0, 8.0, 3);
    }
}

fn mill(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A stone mill house of two stages, the upper one the hoop of the stones; a hopper at the loft, a hoist
    // beam with a sack riding it, a great stone leaning at the door.
    let top = hall(b, [4.0, w - 4.0], [4.0, d - 6.0], Kind::Stone, 24.0, Kind::Slate, 3.0, true, false);
    b.bx(Kind::Plaster, [8.0, w - 8.0], [top - 0.5, top + 11.0], [8.0, d - 10.0]);
    b.hip(Kind::Slate, [7.0, w - 7.0], [7.0, d - 9.0], top + 11.0, 7.0);
    b.frus(Kind::Floor, w / 2.0, d - 11.0, 4.0, top + 11.0, 7.0);
    let lift = if b.work { ((b.turn().cos() + 1.0) * 8.0).round() } else { 16.0 };
    b.bx(Kind::Floor, [w - 4.0, w + 0.0], [top + 6.0, top + 7.4], [d - 9.0, d - 8.0]);
    b.bx(Kind::Cloth, [w - 0.7, w - 0.3], [top - 10.0 + lift, top + 6.4], [d - 8.8, d - 8.4]);
    b.ball(Kind::Plaster, w - 0.5, top - 12.0 + lift, d - 8.6, 2.2, 2.4, 2.2);
    b.cyl(Kind::Rock, 8.0, d - 4.0, 5.5, 0.0, 2.2);
    b.cyl(Kind::Stone, 8.0, d - 4.0, 3.2, 2.2, 0.8);
    if b.work {
        b.smoke(w * 0.5, 3.0, d - 4.0, 9.0, 3);
    }
    b.heap(Load::Flour, w - 8.0, d - 3.0, 10.0, 0.0, 7.0);
}

fn bakery(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A brick house under tile; a great domed oven bulging out of its flank with an arched mouth; a tall stack.
    let top = hall(b, [3.0, 20.0], [4.0, d - 6.0], Kind::Brick, 22.0, Kind::Roof, 8.0, false, false);
    b.ball(Kind::Brick, 25.0, 9.0, d * 0.55, 8.0, 9.0, 9.0);
    b.arch(Kind::Brick, 25.0, d * 0.55 + 7.0, 7.0, 7.0, 3.0, 0.0);
    b.bx(if b.work { Kind::Fire } else { Kind::Door }, [22.5, 27.5], [0.5, 5.0], [d * 0.55 + 8.0, d * 0.55 + 8.6]);
    if b.work {
        b.lamp([25.0, 5.0, d * 0.55 + 11.0], 28.0, 0.75);
    }
    stack(b, 8.0, 10.0, 2.4, top - 4.0, top + 16.0);
    b.smoke(8.0, top + 17.0, 10.0, 14.0, 4);
    b.bx(Kind::Floor, [5.0, 15.0], [9.0, 10.0], [d - 6.6, d - 5.0]);
    b.heap(Load::Bread, 10.0, d - 5.0, 8.0, 10.0, 3.0);
    b.heap(Load::Bread, w - 5.0, d - 3.0, 7.0, 0.0, 5.0);
}

fn brewery(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A big timber hall, a steep roof with a louvred vent at the ridge, a copper kettle visible in its bay.
    let top = hall(b, [3.0, w - 3.0], [4.0, d - 5.0], Kind::Floor, 22.0, Kind::Roof, 11.0, false, false);
    b.bx(Kind::Floor, [w / 2.0 - 4.0, w / 2.0 + 4.0], [top + 4.0, top + 10.0], [d * 0.4 - 3.0, d * 0.4 + 3.0]);
    b.bx(Kind::Door, [w / 2.0 - 3.0, w / 2.0 + 3.0], [top + 6.0, top + 8.0], [d * 0.4 + 3.0, d * 0.4 + 3.4]);
    b.ball(Kind::Metal, 13.0, 8.0, d - 6.0, 7.5, 8.0, 5.0);
    b.bx(Kind::Door, [4.0, 22.0], [0.0, 16.0], [d - 5.4, d - 5.0]);
    b.smoke(w / 2.0, top + 10.5, d * 0.4, 12.0, 4);
    b.heap(Load::Ale, w - 10.0, d - 3.0, 14.0, 0.0, 10.0);
    if b.work {
        for k in 0..3 {
            b.ball(Kind::Plaster, 9.0 + k as f32 * 4.0, 17.0 + ((b.t + k * 3) % 4) as f32, d - 6.0, 1.0, 1.0, 1.0);
        }
    }
}

fn pasture(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A fenced paddock open at the front, a trough and a hay rick, stock grazing.
    b.fence(Kind::Floor, [1.0, w - 1.0], 3.0, 6.0);
    b.bx(Kind::Floor, [1.0, 3.0], [0.0, 6.0], [4.0, d - 8.0]);
    b.bx(Kind::Floor, [w - 3.0, w - 1.0], [0.0, 6.0], [4.0, d - 8.0]);
    b.fence(Kind::Floor, [1.0, 18.0], d - 2.0, 6.0);
    b.fence(Kind::Floor, [w - 18.0, w - 1.0], d - 2.0, 6.0);
    let shift = if b.work { (b.turn().sin() * 3.0).round() } else { 0.0 };
    for (i, (x, z)) in [(10.0, 14.0), (36.0, 24.0)].into_iter().enumerate() {
        let x = x + if i == 0 { shift } else { -shift };
        b.bx(Kind::Floor, [x, x + 12.0], [3.0, 9.0], [z, z + 5.0]);
        b.bx(Kind::Floor, [x + 11.0, x + 14.0], [5.0, 10.0 - if b.work && (b.t / 2) % 2 == i as u32 { 3.0 } else { 0.0 }], [z + 1.0, z + 4.0]);
        for lx in [0.8, 2.6, 8.2, 10.4] {
            b.bx(Kind::Trunk, [x + lx, x + lx + 1.2], [0.0, 3.0], [z + 0.6, z + 1.8]);
            b.bx(Kind::Trunk, [x + lx, x + lx + 1.2], [0.0, 3.0], [z + 3.2, z + 4.4]);
        }
    }
    for (x, z) in [(26.0, 12.0), (46.0, 22.0), (50.0, 36.0), (20.0, 32.0)] {
        b.ball(Kind::Plaster, x, 3.0, z, 3.4, 2.6, 2.4);
    }
    b.bx(Kind::Floor, [w - 14.0, w - 4.0], [0.0, 3.0], [8.0, 11.0]);
    b.bx(Kind::Water, [w - 13.0, w - 5.0], [2.6, 3.0], [8.6, 10.4]);
    b.heap(Load::Grain, w - 9.0, 22.0, 12.0, 0.0, 9.0);
}

fn tanyard(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // Two rows of timber-edged tan pits, hides stretched on frames, a scraping beam.
    for (i, (x, z)) in [(3.0, 4.0), (19.0, 4.0), (3.0, 16.0), (19.0, 16.0)].into_iter().enumerate() {
        b.bx(Kind::Floor, [x, x + 13.0], [0.0, 2.2], [z, z + 9.0]);
        b.bx(Kind::Mud, [x + 1.4, x + 11.6], [1.8, 2.5], [z + 1.4, z + 7.6]);
        if b.work && ((b.t + i as u32 * 2) % 5) < 2 {
            b.ball(Kind::Floor, x + 4.0 + i as f32, 3.0, z + 4.0, 0.9, 0.9, 0.9);
        }
    }
    for (k, x) in [35.0, 41.0].into_iter().enumerate() {
        b.bx(Kind::Floor, [x, x + 1.0], [0.0, 17.0], [10.0, 11.0]);
        b.bx(Kind::Floor, [x + 5.0, x + 6.0], [0.0, 17.0], [10.0, 11.0]);
        b.bx(Kind::Floor, [x, x + 6.0], [16.0, 17.0], [10.0, 11.0]);
        b.bx(if k == 0 { Kind::Floor } else { Kind::Plaster }, [x + 1.0, x + 5.0], [3.0, 16.0], [10.2, 10.8]);
    }
    b.bx(Kind::Floor, [32.0, w - 3.0], [6.0, 8.0], [d - 12.0, d - 8.0]);
    b.heap(Load::Hides, 9.0, d - 3.0, 10.0, 0.0, 5.0);
    b.heap(Load::Timber, 26.0, d - 3.0, 10.0, 0.0, 6.0);
}

fn weaving_shed(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A shed with its front open, thatch over a boarded back; a tall loom inside it, cloth hung on a line outside.
    b.bx(Kind::Floor, [3.0, 32.0], [0.0, 17.0], [4.0, 6.0]);
    for x in [3.0, 30.0] {
        b.bx(Kind::Floor, [x, x + 1.8], [0.0, 17.0], [4.0, d - 4.0]);
    }
    b.gable(Kind::Thatch, [1.0, 34.0], [3.0, d - 3.0], 17.0, 9.0, false);
    b.bx(Kind::Floor, [8.0, 9.4], [0.0, 22.0], [d * 0.45, d * 0.45 + 1.4]);
    b.bx(Kind::Floor, [24.0, 25.4], [0.0, 22.0], [d * 0.45, d * 0.45 + 1.4]);
    b.bx(Kind::Floor, [8.0, 25.4], [20.6, 22.0], [d * 0.45, d * 0.45 + 1.4]);
    let shuttle = if b.work { 11.0 + ((b.t * 2) % 11) as f32 } else { 14.0 };
    b.bx(Kind::Cloth, [10.0, 23.0], [3.0, 8.0 + (b.t / 4) as f32 * u32::from(b.work) as f32], [d * 0.45 + 0.3, d * 0.45 + 1.1]);
    b.bx(Kind::Floor, [shuttle, shuttle + 3.0], [10.0, 11.0], [d * 0.45 - 0.4, d * 0.45 + 1.8]);
    b.bx(Kind::Floor, [w - 12.0, w - 2.0], [26.0, 27.0], [8.0, 9.0]);
    for k in 0..3 {
        b.bx(Kind::Cloth, [w - 11.0 + k as f32 * 3.4, w - 8.6 + k as f32 * 3.4], [13.0, 26.0], [8.1, 8.9]);
    }
    b.heap(Load::Cloth, w - 7.0, d - 3.0, 9.0, 0.0, 6.0);
}

// ------------------------------------------------------------ the halls ----

fn lodge(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A long stone hall with its gable to the front, the lodge's mark in the pediment (a square and compasses),
    // masons' marks cut in the jambs, a banner staff.
    let top = hall(b, [3.0, w - 3.0], [4.0, d - 4.0], Kind::Stone, 24.0, Kind::Roof, 10.0, false, true);
    let (cx, py) = (w / 2.0, top + 1.0);
    b.bx(Kind::Door, [cx - 7.0, cx + 7.0], [py, py + 7.5], [d - 5.2, d - 4.8]);
    b.bx(Kind::Bone, [cx - 0.6, cx + 0.6], [py + 1.0, py + 6.5], [d - 5.6, d - 5.2]);
    for k in 0..5 {
        let f = k as f32 / 4.0;
        b.bx(Kind::Bone, [cx - 4.5 + f * 1.6 - 0.5, cx - 4.5 + f * 1.6 + 0.5], [py + 1.0 + f * 4.0, py + 1.9 + f * 4.0], [d - 5.6, d - 5.2]);
        b.bx(Kind::Bone, [cx + 4.5 - f * 1.6 - 0.5, cx + 4.5 - f * 1.6 + 0.5], [py + 1.0 + f * 4.0, py + 1.9 + f * 4.0], [d - 5.6, d - 5.2]);
    }
    b.bx(Kind::Bone, [cx - 4.0, cx + 4.0], [py + 2.4, py + 3.2], [d - 5.6, d - 5.2]);
    for (k, x) in [cx - 9.0, cx + 9.0].into_iter().enumerate() {
        b.bx(Kind::Bone, [x - 0.6, x + 0.6], [4.0 + k as f32, 6.0 + k as f32], [d - 3.6, d - 3.0]);
    }
    stack(b, w - 12.0, 12.0, 3.0, top - 3.0, top + 10.0);
    b.smoke(w - 12.0, top + 11.0, 12.0, 12.0, 3);
    let wave = if b.work { (b.turn().sin() * 1.2).round() } else { 0.0 };
    b.bx(Kind::Floor, [w - 3.6, w - 2.6], [0.0, 38.0], [d - 6.0, d - 5.0]);
    b.bx(Kind::Cloth, [w - 14.0 + wave, w - 3.6], [28.0, 36.0], [d - 5.8, d - 5.2]);
    b.heap(Load::Ashlar, 9.0, d - 2.5, 10.0, 0.0, 6.0);
}

fn depot(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A gravelled yard behind a rail fence, a thatched shed on posts over the stock, a signpost.
    b.bx(Kind::Path, [2.0, w - 2.0], [0.0, 0.4], [3.0, d - 3.0]);
    for x in [3.0, w / 2.0, w - 5.0] {
        b.bx(Kind::Floor, [x, x + 1.8], [0.0, 17.0], [5.0, 6.8]);
    }
    b.gable(Kind::Thatch, [1.0, w - 1.0], [3.0, 16.0], 17.0, 7.0, false);
    b.heap(Load::Crate, 11.0, 12.0, 12.0, 0.0, 8.0);
    b.heap(Load::Flour, w - 12.0, 12.0, 14.0, 0.0, 9.0);
    b.cyl(Kind::Floor, 24.0, 11.0, 2.4, 0.0, 5.0);
    b.bx(Kind::Floor, [w - 4.0, w - 3.0], [0.0, 16.0], [d - 10.0, d - 9.0]);
    b.bx(Kind::Floor, [w - 9.0, w - 1.0], [12.0, 15.0], [d - 10.4, d - 9.6]);
    b.bx(Kind::Floor, [4.0, 14.0], [2.0, 4.0], [d - 9.0, d - 4.0]);
    let x = if b.work { 18.0 + ((b.turn().sin() + 1.0) * 8.0).round() } else { 20.0 };
    b.bx(Kind::Floor, [x, x + 4.0], [4.0, 8.0], [d - 7.0, d - 4.0]);
}

fn quay(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // Water along the front, a stone quay wall rising from it, bollards, a jib crane swinging a crate.
    b.bx(Kind::Water, [0.0, w], [0.0, 0.6], [d - 10.0, d]);
    b.bx(Kind::Stone, [0.0, w], [0.0, 10.0], [2.0, d - 10.0]);
    b.bx(Kind::Stone, [0.0, w], [10.0, 11.4], [1.0, d - 9.0]);
    for x in (6..(w as i32 - 4)).step_by(14) {
        b.cyl(Kind::Metal, x as f32, d - 12.0, 1.5, 11.4, 3.0);
    }
    b.bx(Kind::Floor, [w - 10.0, w - 7.0], [11.4, 36.0], [8.0, 11.0]);
    let swing = if b.work { ((b.turn().sin() + 1.0) * 5.0).round() } else { 7.0 };
    b.bx(Kind::Floor, [w - 28.0 + swing, w - 7.0], [34.0, 36.0], [8.5, 10.5]);
    let hook = if b.work { 24.0 + ((b.turn().cos() + 1.0) * 3.0).round() } else { 26.0 };
    b.bx(Kind::Cloth, [w - 27.0 + swing, w - 26.4 + swing], [hook + 5.0, 34.0], [9.2, 9.8]);
    b.bx(Kind::Floor, [w - 30.0 + swing, w - 24.0 + swing], [hook, hook + 5.0], [7.0, 12.0]);
    b.heap(Load::Crate, 10.0, 8.0, 12.0, 11.4, 8.0);
    b.heap(Load::Planks, 8.0, d - 14.0, 10.0, 11.4, 4.0);
    b.bx(Kind::Floor, [4.0, 24.0], [0.0, 3.5], [d - 7.0, d - 2.0]);
    b.bx(Kind::Roof, [6.0, 22.0], [3.5, 4.0], [d - 6.0, d - 3.0]);
}

fn hut(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A round wattle-and-daub hut under a pointed thatch, a smoke hole at its crown, a low door; a woodpile beside it.
    let (cx, cz) = (w * 0.38, d / 2.0);
    b.cyl(Kind::Plaster, cx, cz, 6.8, 0.0, 8.0);
    b.cone(Kind::Thatch, cx, cz, 7.9, 8.0, 10.0);
    b.door(cx, cz + 6.7, 0.0, [3.6, 5.5]);
    b.slit(cx + 4.0, cz + 5.4, 3.0, [1.6, 2.2]);
    b.smoke(cx, 19.0, cz, 12.0, 3);
    b.heap(Load::Timber, w - 8.0, d - 6.0, 9.0, 0.0, 7.0);
}

fn cottage(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A low stone cottage under a deep thatch hip, a squat stack, a porch with two posts.
    hall(b, [3.0, w - 3.0], [3.0, d - 5.0], Kind::Stone, 9.0, Kind::Thatch, 10.0, true, false);
    stack(b, w - 8.0, 6.0, 2.0, 8.0, 21.0);
    b.bx(Kind::Floor, [5.0, 15.0], [8.0, 9.0], [d - 5.0, d - 1.0]);
    for x in [5.5, 14.0] {
        b.bx(Kind::Floor, [x, x + 1.0], [0.0, 8.0], [d - 2.0, d - 1.0]);
    }
    b.smoke(w - 8.0, 22.0, 6.0, 12.0, 3);
}

fn warehouse(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A big timber hall, double doors, a hoist beam from the loft gable with a crate going up and down.
    let top = hall(b, [3.0, w - 3.0], [4.0, d - 4.0], Kind::Floor, 22.0, Kind::Roof, 11.0, false, false);
    b.bx(Kind::Door, [w / 2.0 - 8.0, w / 2.0 + 8.0], [0.0, 16.0], [d - 4.5, d - 3.7]);
    b.bx(Kind::Door, [w - 20.0, w - 12.0], [top - 12.0, top - 4.0], [d - 4.4, d - 3.8]);
    b.bx(Kind::Floor, [w - 18.0, w - 3.0], [top - 3.0, top - 1.6], [d - 5.0, d - 3.4]);
    let lift = if b.work { ((b.turn().cos() + 1.0) * 6.0).round() } else { 10.0 };
    b.bx(Kind::Cloth, [w - 4.0, w - 3.4], [top - 18.0 + lift, top - 2.0], [d - 3.6, d - 3.2]);
    b.bx(Kind::Floor, [w - 8.0, w - 1.0], [top - 24.0 + lift, top - 18.0 + lift], [d - 4.0, d - 1.5]);
    b.heap(Load::Flour, 10.0, d - 2.5, 14.0, 0.0, 8.0);
    b.heap(Load::Ale, w - 12.0, d - 2.5, 10.0, 0.0, 6.0);
}

fn caravanserai(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A long inn of stone: a low arcaded front, a great arch between, a tower at one end, a wagon in the yard.
    hall(b, [3.0, w - 14.0], [4.0, d - 6.0], Kind::Stone, 30.0, Kind::Roof, 11.0, true, false);
    b.arch(Kind::Stone, (3.0 + w - 14.0) / 2.0, d - 6.0, 22.0, 26.0, 4.0, 0.0);
    b.bx(Kind::Door, [(3.0 + w - 14.0) / 2.0 - 7.0, (3.0 + w - 14.0) / 2.0 + 7.0], [0.0, 22.0], [d - 6.4, d - 5.8]);
    if b.work {
        b.bx(Kind::Fire, [(3.0 + w - 14.0) / 2.0 - 1.0, (3.0 + w - 14.0) / 2.0 + 1.0], [14.0, 16.0], [d - 6.2, d - 5.6]);
        b.lamp([(3.0 + w - 14.0) / 2.0, 15.0, d - 3.0], 36.0, 0.75);
    }
    b.bx(Kind::Stone, [w - 14.0, w - 3.0], [0.0, 54.0], [6.0, 17.0]);
    b.pyr(Kind::Slate, w - 8.5, 11.5, 14.0, 14.0, 54.0, 12.0);
    b.slit(w - 8.5, 17.0, 36.0, [2.0, 7.0]);
    let wave = if b.work { (b.turn().sin() * 1.0).round() } else { 0.0 };
    b.bx(Kind::Floor, [w - 8.8, w - 8.2], [66.0, 78.0], [11.2, 11.8]);
    b.bx(Kind::Cloth, [w - 8.2, w + wave - 0.5], [72.0, 78.0], [11.3, 11.7]);
    b.bx(Kind::Floor, [5.0, 17.0], [3.0, 6.0], [d - 10.0, d - 3.0]);
    b.bx(Kind::Plaster, [6.0, 16.0], [6.0, 12.0], [d - 9.0, d - 4.0]);
    for x in [7.0, 15.0] {
        b.wheel(Kind::Floor, x, 3.0, d - 2.5, 3.0, b.turn() * 0.4, 6, 1.0);
    }
}

fn wall(b: &mut W, kind: Kind) {
    let (w, d) = (b.w, b.d);
    // A length of curtain: a batter, a string course, crenellated; a cresset on the walk when it is manned.
    b.bx(kind, [0.0, w], [0.0, 3.0], [3.0, d - 3.0]);
    b.bx(kind, [1.0, w - 1.0], [3.0, 17.0], [4.0, d - 4.0]);
    b.bx(Kind::Stone, [0.0, w], [17.0, 19.0], [3.0, d - 3.0]);
    for k in 0..3 {
        let x = 1.0 + k as f32 * 5.0;
        b.bx(kind, [x, x + 3.0], [19.0, 23.0], [4.0, d - 4.0]);
    }
    if b.work {
        b.bx(Kind::Metal, [w - 5.0, w - 2.0], [19.0, 20.0], [6.0, 9.0]);
        b.fire(w - 3.5, 20.0, 7.5, 2.4);
    }
}

fn brick_cottage(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // Red brick under tile, a tall end stack, a dormer on the front slope, a stoop.
    hall(b, [3.0, w - 3.0], [3.0, d - 3.0], Kind::Brick, 12.0, Kind::Roof, 9.0, false, false);
    stack(b, 6.0, 6.0, 2.4, 10.0, 28.0);
    b.bx(Kind::Brick, [w - 14.0, w - 7.0], [14.0, 19.0], [d - 6.0, d - 3.0]);
    b.gable(Kind::Roof, [w - 15.0, w - 6.0], [d - 7.0, d - 2.0], 19.0, 4.0, true);
    b.slit(w - 10.5, d - 3.0, 14.5, [2.5, 3.0]);
    b.bx(Kind::Stone, [w / 2.0 - 5.0, w / 2.0 + 5.0], [0.0, 1.6], [d - 3.0, d - 0.6]);
    b.smoke(6.0, 29.0, 6.0, 12.0, 3);
}

fn brick_gate(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A brick archway between two piers, a stone coping, a portcullis in its throat.
    b.bx(Kind::Brick, [2.0, w - 2.0], [0.0, 28.0], [3.0, d - 3.0]);
    b.bx(Kind::Stone, [1.0, w - 1.0], [28.0, 31.0], [2.0, d - 2.0]);
    for k in 0..4 {
        b.bx(Kind::Brick, [2.0 + k as f32 * 7.5, 5.0 + k as f32 * 7.5], [31.0, 35.0], [4.0, d - 4.0]);
    }
    b.arch(Kind::Door, w / 2.0, d / 2.0, 17.0, 24.0, d - 4.0, 0.0);
    let up = if b.work { ((b.turn().cos() + 1.0) * 6.0).round() } else { 0.0 };
    for k in 0..5 {
        let x = w / 2.0 - 6.0 + k as f32 * 3.0;
        b.bx(Kind::Metal, [x, x + 0.8], [up, 22.0], [d - 4.0, d - 3.2]);
    }
    for y in [6.0, 12.0, 18.0] {
        b.bx(Kind::Metal, [w / 2.0 - 7.0, w / 2.0 + 7.0], [y + up, y + 0.8 + up], [d - 4.0, d - 3.2]);
    }
}

// ---------------------------------------------------------------- power ----

fn mortar_mill(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w / 2.0, d * 0.45);
    // A round stone pan with mortar in it, a post at its middle, a sweep carrying a heavy runner round.
    b.cyl(Kind::Stone, cx, cz, 13.0, 0.0, 4.0);
    b.cyl(Kind::Path, cx, cz, 11.0, 3.8, 0.6);
    b.bx(Kind::Floor, [cx - 1.0, cx + 1.0], [3.0, 20.0], [cz - 1.0, cz + 1.0]);
    let a = if b.work { b.turn() } else { 0.8 };
    let (rx, rz) = (cx + a.cos() * 7.5, cz + a.sin() * 7.5);
    for k in 1..8 {
        let f = k as f32 / 8.0;
        b.bx(Kind::Floor, [cx + a.cos() * 8.0 * f - 0.7, cx + a.cos() * 8.0 * f + 0.7], [10.0, 11.4], [cz + a.sin() * 8.0 * f - 0.7, cz + a.sin() * 8.0 * f + 0.7]);
    }
    b.cyl(Kind::Rock, rx, rz, 3.5, 4.0, 5.0);
    b.heap(Load::Sand, w - 8.0, d - 3.0, 10.0, 0.0, 6.0);
    b.heap(Load::Lime, 7.0, d - 3.0, 9.0, 0.0, 5.0);
}

fn intake(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // The river along the back, a stone wall holding its bank with a gate cut through it, a trough leading out.
    b.bx(Kind::Water, [0.0, w], [0.0, 0.8], [1.0, 10.0]);
    b.bx(Kind::Stone, [0.0, 12.0], [0.0, 8.0], [10.0, 20.0]);
    b.bx(Kind::Stone, [20.0, w], [0.0, 8.0], [10.0, 20.0]);
    for x in [12.0, 19.0] {
        b.bx(Kind::Floor, [x - 0.5, x + 1.0], [0.0, 14.0], [10.0, 12.0]);
    }
    let open = if b.work { 6.0 } else { 1.0 };
    b.bx(Kind::Floor, [13.0, 19.0], [open, 12.0 + open * 0.2], [10.6, 11.6]);
    b.bx(Kind::Metal, [15.6, 16.4], [12.0, 17.0], [10.8, 11.4]);
    b.bx(Kind::Floor, [12.0, 20.0], [0.0, 1.4], [20.0, d - 1.0]);
    b.bx(Kind::Water, [13.0, 19.0], [1.4, if b.work { 2.4 } else { 1.8 }], [20.0, d - 1.0]);
    // Glints running down the race and across the river.
    if b.work {
        for k in 0..3 {
            let s = ((b.t + k * 3) % 8) as f32;
            b.bx(Kind::Plaster, [14.0 + (k % 2) as f32 * 3.0, 16.0 + (k % 2) as f32 * 3.0], [2.4, 2.8], [20.0 + s * 1.4, 21.4 + s * 1.4]);
            b.bx(Kind::Plaster, [s * 3.4, s * 3.4 + 3.0], [0.9, 1.2], [3.0 + k as f32 * 2.5, 4.0 + k as f32 * 2.5]);
        }
    }
}

fn treadwheel(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A great drum on a frame, its tread of boards turning, the axle carried off to the right.
    b.bx(Kind::Floor, [2.0, 4.0], [0.0, 28.0], [4.0, d - 3.0]);
    b.bx(Kind::Floor, [w - 4.0, w - 2.0], [0.0, 28.0], [4.0, d - 3.0]);
    b.wheel(Kind::Floor, w / 2.0, 14.0, d / 2.0, 12.0, if b.work { b.turn() / 10.0 * 2.0 } else { 0.1 }, 10, 6.0);
    b.bx(Kind::Metal, [w / 2.0, w + 2.0], [13.0, 15.0], [d / 2.0 - 1.0, d / 2.0 + 1.0]);
    if b.work {
        b.ball(Kind::Plaster, w / 2.0, 7.0 + (b.t % 2) as f32, d / 2.0, 1.6, 2.4, 1.6);
    }
}

fn treadwheel_crane(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A timber mast, a jib, the great treadwheel at its foot, a cut block hanging in its sling.
    b.bx(Kind::Floor, [2.0, w - 2.0], [0.0, 2.0], [d * 0.35, d - 4.0]);
    b.bx(Kind::Floor, [w - 8.0, w - 5.0], [2.0, 50.0], [d * 0.45, d * 0.45 + 3.0]);
    for k in 1..8 {
        let f = k as f32 / 8.0;
        b.bx(Kind::Floor, [w - 8.0 - f * 20.0, w - 5.0 - f * 20.0], [50.0 - f * 7.0, 52.0 - f * 7.0], [d * 0.45, d * 0.45 + 3.0]);
    }
    b.wheel(Kind::Floor, 13.0, 12.0, d * 0.45 + 1.5, 11.0, if b.work { b.turn() / 4.0 } else { 0.2 }, 8, 4.0);
    let lift = if b.work { ((b.turn().cos() + 1.0) * 8.0).round() } else { 14.0 };
    b.bx(Kind::Cloth, [4.4, 5.0], [26.0 + lift, 43.0], [d * 0.45 + 1.0, d * 0.45 + 1.6]);
    b.bx(Kind::Plaster, [1.0, 8.0], [20.0 + lift, 26.0 + lift], [d * 0.45, d * 0.45 + 4.0]);
    b.heap(Load::Ashlar, w - 7.0, d - 3.0, 9.0, 0.0, 6.0);
}

fn horse_gin(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w / 2.0, d * 0.5);
    // A drum-house under a thatched cone at the middle of a trodden ring, the sweep out over the ring and a horse yoked to it.
    b.cyl(Kind::Path, cx, cz, 22.0, 0.0, 0.5);
    b.cyl(Kind::Ground, cx, cz, 10.0, 0.5, 0.3);
    b.cyl(Kind::Floor, cx, cz, 8.0, 0.5, 14.0);
    b.cone(Kind::Thatch, cx, cz, 10.0, 14.5, 22.0);
    let a = if b.work { b.turn() } else { 2.2 };
    for k in 1..9 {
        let f = k as f32 / 9.0 * 17.0;
        b.bx(Kind::Floor, [cx + a.cos() * f - 0.8, cx + a.cos() * f + 0.8], [8.0, 9.4], [cz + a.sin() * f - 0.8, cz + a.sin() * f + 0.8]);
    }
    let (hx, hz) = (cx + a.cos() * 17.0, cz + a.sin() * 17.0);
    b.bx(Kind::Floor, [hx - 4.0, hx + 4.0], [4.0, 9.0], [hz - 2.0, hz + 2.0]);
    b.bx(Kind::Floor, [hx + 3.0, hx + 6.0], [7.0, 11.0], [hz - 1.0, hz + 1.0]);
    for (lx, lz) in [(-3.0, -1.5), (2.5, -1.5), (-3.0, 1.0), (2.5, 1.0)] {
        b.bx(Kind::Trunk, [hx + lx, hx + lx + 1.0], [0.0, 4.0], [hz + lz, hz + lz + 1.0]);
    }
    b.door(cx, cz + 8.5, 0.5, [5.0, 8.0]);
    b.heap(Load::Grain, w - 7.0, d - 3.0, 9.0, 0.0, 6.0);
}

fn windmill(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w / 2.0, d * 0.45);
    // A tapering whitewashed tower, a boat-cap on it, four sails turning on the hub.
    b.plinth(Kind::Stone, [cx - 9.0, cx + 9.0], [cz - 9.0, cz + 9.0], &[(2.0, 1.0)]);
    b.frus(Kind::Plaster, cx, cz, 10.0, 2.0, 40.0);
    b.cyl(Kind::Roof, cx, cz, 7.0, 42.0, 4.0);
    b.cone(Kind::Roof, cx, cz, 7.5, 46.0, 7.0);
    b.door(cx, cz + 9.5, 2.0, [5.0, 9.0]);
    b.slit(cx, cz + 8.5, 22.0, [2.0, 5.0]);
    let a0 = if b.work { b.turn() * 0.5 } else { 0.5 };
    let (hy, hz) = (44.0, cz + 8.5);
    b.bx(Kind::Floor, [cx - 2.0, cx + 2.0], [hy - 2.0, hy + 2.0], [hz - 1.0, hz + 3.0]);
    for k in 0..4 {
        let a = a0 + k as f32 * PI / 2.0;
        for s in 1..8 {
            let r = 2.5 + s as f32 * 2.0;
            let (x, y) = (cx + a.cos() * r, hy + a.sin() * r);
            b.bx(Kind::Floor, [x - 1.0, x + 1.0], [y - 1.0, y + 1.0], [hz + 2.0, hz + 3.6]);
            if s > 2 {
                let (nx, ny) = (-a.sin() * 2.0, a.cos() * 2.0);
                b.bx(Kind::Plaster, [x + nx - 1.7, x + nx + 1.7], [y + ny - 1.7, y + ny + 1.7], [hz + 2.2, hz + 3.0]);
            }
        }
    }
    b.heap(Load::Flour, w - 6.0, d - 3.0, 8.0, 0.0, 6.0);
}

fn waterwheel(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A river across the front, a great wheel on stone piers with its lower boards in it, a gear-shed on the right.
    b.bx(Kind::Water, [0.0, w], [0.0, 0.9], [d - 14.0, d]);
    b.bx(Kind::Stone, [1.0, 7.0], [0.0, 30.0], [10.0, 22.0]);
    b.bx(Kind::Stone, [30.0, 36.0], [0.0, 30.0], [10.0, 22.0]);
    b.wheel(Kind::Floor, 18.0, 20.0, 16.0, 15.0, if b.work { b.turn() / 8.0 * 2.0 } else { 0.3 }, 12, 5.0);
    b.bx(Kind::Metal, [18.0, 40.0], [19.0, 21.0], [15.0, 17.0]);
    hall(b, [37.0, w - 2.0], [6.0, 24.0], Kind::Floor, 22.0, Kind::Slate, 8.0, true, false);
    b.heap(Load::Flour, w - 8.0, d - 3.0, 8.0, 0.0, 5.0);
}

fn drive_shaft(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // Two posts with bearings and a long round shaft across them, a cog riding it.
    for x in [2.0, w - 4.0] {
        b.bx(Kind::Floor, [x, x + 2.0], [0.0, 10.0], [d / 2.0 - 1.5, d / 2.0 + 1.5]);
        b.bx(Kind::Metal, [x - 0.6, x + 2.6], [9.0, 11.0], [d / 2.0 - 2.0, d / 2.0 + 2.0]);
    }
    b.bx(Kind::Floor, [0.0, w], [9.4, 10.8], [d / 2.0 - 0.7, d / 2.0 + 0.7]);
    let a = if b.work { 0.12 + b.turn() / 4.0 } else { 0.0 };
    for k in 0..8 {
        let ang = a + k as f32 / 8.0 * TAU;
        b.bx(Kind::Metal, [w / 2.0 + ang.cos() * 4.0 - 0.7, w / 2.0 + ang.cos() * 4.0 + 0.7], [10.1 + ang.sin() * 4.0 - 0.7, 10.1 + ang.sin() * 4.0 + 0.7], [d / 2.0 - 1.0, d / 2.0 + 1.0]);
    }
}

fn leyline(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A bed of dark flags, a channel inlaid across it, runes down the channel, a pulse running along it.
    b.bx(Kind::Slate, [0.0, w], [0.0, 1.6], [2.0, d - 2.0]);
    b.bx(Kind::Door, [0.0, w], [1.6, 2.0], [d / 2.0 - 2.0, d / 2.0 + 2.0]);
    for k in 0..4 {
        let on = b.work && ((k * 4 + 16 - (b.t as i32 * 2) % 16) % 16) < 4;
        b.bx(if on { Kind::Window } else { Kind::Metal }, [1.0 + k as f32 * 4.0, 3.0 + k as f32 * 4.0], [2.0, 2.5], [d / 2.0 - 1.0, d / 2.0 + 1.0]);
    }
}

fn mana_engine(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w / 2.0, d * 0.5);
    // A stepped dais, six coils round a socket, a tall crystal floating over it that bobs and throws light when it runs.
    b.plinth(Kind::Slate, [cx - 10.0, cx + 10.0], [cz - 10.0, cz + 10.0], &[(3.0, 3.0), (3.0, 1.5)]);
    for k in 0..6 {
        let a = k as f32 / 6.0 * TAU;
        b.bx(Kind::Metal, [cx + a.cos() * 8.0 - 1.2, cx + a.cos() * 8.0 + 1.2], [6.0, 11.0], [cz + a.sin() * 8.0 - 1.2, cz + a.sin() * 8.0 + 1.2]);
    }
    let bob = if b.work { (b.turn().sin() * 2.0).round() } else { 0.0 };
    b.gem(if b.work { Kind::Window } else { Kind::Metal }, cx, 18.0 + bob, cz, 7.0, 16.0, if b.work { b.turn() * 30.0 } else { 0.0 });
    if b.work {
        b.lamp([cx, 24.0, cz], 44.0, 0.9);
        for k in 0..3 {
            let a = b.turn() + k as f32 * 2.1;
            b.bx(Kind::Window, [cx + a.cos() * 8.0 - 0.6, cx + a.cos() * 8.0 + 0.6], [24.0 + (k as f32), 25.2 + (k as f32)], [cz + a.sin() * 6.0 - 0.6, cz + a.sin() * 6.0 + 0.6]);
        }
    }
}

fn conduit(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A squat stone pylon with a band of brass and a crystal in a cage at its head.
    b.bx(Kind::Slate, [3.0, w - 3.0], [0.0, 10.0], [3.0, d - 3.0]);
    b.bx(Kind::Metal, [4.0, w - 4.0], [10.0, 11.4], [4.0, d - 4.0]);
    b.bx(Kind::Slate, [5.0, w - 5.0], [11.4, 18.0], [5.0, d - 5.0]);
    let bob = if b.work { b.turn().sin().round() } else { 0.0 };
    b.gem(if b.work { Kind::Window } else { Kind::Metal }, w / 2.0, 19.0 + bob, d / 2.0, 5.0, 8.0, 0.0);
    if b.work {
        b.lamp([w / 2.0, 24.0, d / 2.0], 30.0, 0.7);
    }
}

fn enchanter(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w / 2.0, d * 0.5);
    // A round tower of slate-blue stone, a violet cone of roof, a ring of runes, a crystal over the spire.
    b.plinth(Kind::Stone, [cx - 9.0, cx + 9.0], [cz - 9.0, cz + 9.0], &[(2.5, 1.5)]);
    b.frus(Kind::Slate, cx, cz, 11.0, 2.5, 52.0);
    b.cyl(Kind::Stone, cx, cz, 12.0, 54.5, 2.0);
    b.cone(Kind::Cloth, cx, cz, 13.0, 56.5, 22.0);
    b.door(cx, cz + 9.4, 2.5, [5.0, 10.0]);
    b.slit(cx, cz + 7.8, 36.0, [2.0, 8.0]);
    for k in 0..8 {
        let a = k as f32 / 8.0 * TAU;
        let on = b.work && (k as u32 + b.t) % 4 < 2;
        b.bx(if on { Kind::Window } else { Kind::Metal }, [cx + a.cos() * 10.2 - 0.9, cx + a.cos() * 10.2 + 0.9], [26.0, 29.0], [cz + a.sin() * 10.2 - 0.9, cz + a.sin() * 10.2 + 0.9]);
    }
    let bob = if b.work { (b.turn().sin() * 1.5).round() } else { 0.0 };
    b.gem(if b.work { Kind::Window } else { Kind::Metal }, cx, 80.0 + bob, cz, 6.0, 12.0, if b.work { b.turn() * 40.0 } else { 0.0 });
    if b.work {
        b.lamp([cx, 86.0, cz], 52.0, 0.9);
    }
}

// ----------------------------------------------------- the rest of the set ----

fn well(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w / 2.0, d / 2.0);
    // A ring of dressed stone round black water, two posts, a windlass bar, a bucket on its rope.
    b.cyl(Kind::Stone, cx, cz, 6.5, 0.0, 6.0);
    b.cyl(Kind::Water, cx, cz, 4.5, 5.6, 0.5);
    for x in [cx - 6.5, cx + 5.0] {
        b.bx(Kind::Floor, [x, x + 1.6], [0.0, 22.0], [cz - 0.8, cz + 0.8]);
    }
    b.wedge(Kind::Roof, [cx - 8.0, cx + 8.0], [cz - 4.0, cz + 4.0], 22.0, 5.0);
    b.bx(Kind::Floor, [cx - 6.0, cx + 6.0], [17.0, 18.4], [cz - 0.6, cz + 0.6]);
    let drop = if b.work { ((b.turn().cos() + 1.0) * 5.0).round() } else { 8.0 };
    b.bx(Kind::Cloth, [cx - 0.3, cx + 0.3], [10.0 - drop * 0.3, 17.0], [cz - 0.3, cz + 0.3]);
    b.cyl(Kind::Floor, cx, cz, 1.8, 7.0 - drop * 0.3, 3.0);
}

fn slaking_pit(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A boarded pit of milk-white lime, boiling when the water goes on.
    b.bx(Kind::Floor, [3.0, w - 3.0], [0.0, 3.0], [5.0, d - 8.0]);
    b.bx(Kind::Plaster, [4.5, w - 4.5], [2.4, 3.2], [6.5, d - 9.5]);
    if b.work {
        for k in 0..5 {
            b.ball(Kind::Plaster, 8.0 + k as f32 * 4.5, 3.6, 9.0 + ((k * 7 + b.t as i32) % 8) as f32, 1.2, 1.0, 1.2);
        }
    }
    b.smoke(10.0, 4.0, d * 0.4, 14.0, 3);
    b.smoke(w - 10.0, 4.0, d * 0.4, 14.0, 3);
    b.bx(Kind::Floor, [w - 8.0, w - 4.0], [0.0, 4.0], [d - 6.0, d - 2.0]);
    b.heap(Load::Lime, 8.0, d - 3.0, 9.0, 0.0, 6.0);
}

fn ditch_gang(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // The cut: a dark trench across the width between two banks with planked faces, water seeping in its foot;
    // spoil heaped along the near bank, pegs and a cord along the far, spoil flying off the spades when it is worked.
    b.bx(Kind::Mud, [2.0, w - 2.0], [0.0, 1.0], [8.0, 17.0]);
    b.bx(Kind::Water, [4.0, w - 4.0], [1.0, 1.6], [13.0, 16.0]);
    b.bx(Kind::Ground, [2.0, w - 2.0], [0.0, 4.0], [3.0, 8.0]);
    b.bx(Kind::Ground, [2.0, w - 2.0], [0.0, 3.0], [17.0, 20.0]);
    b.bx(Kind::Floor, [2.0, w - 2.0], [3.0, 4.6], [7.4, 8.2]);
    for (x0, x1, h) in [(3.0, 21.0, 8.0), (26.0, 45.0, 6.5)] {
        b.heap(Load::Clay, (x0 + x1) / 2.0, d - 10.0, x1 - x0, 0.0, h);
    }
    for x in [3.0, 24.0, 44.0] {
        b.bx(Kind::Floor, [x, x + 1.0], [0.0, 9.0], [4.0, 5.0]);
    }
    b.bx(Kind::Cloth, [3.0, 45.0], [8.0, 8.6], [4.2, 4.8]);
    b.bx(Kind::Floor, [34.0, 39.0], [2.0, 4.0], [d - 3.0, d - 0.5]);
    if b.work {
        for k in 0..4 {
            let s = ((b.t + k * 2) % 8) as f32;
            let x = 8.0 + (k % 2) as f32 * 22.0 + s * 2.4;
            let y = 6.0 + s * (8.0 - s) * 0.6;
            b.bx(Kind::Ground, [x - 0.9, x + 0.9], [y, y + 1.8], [11.0, 12.8]);
        }
        b.smoke(10.0, 5.0, 14.0, 8.0, 2);
    }
}

// ------------------------------------------------- what stands against the Deep ----

/// A sharpened stake, standing on `(x, z)` with its point at `top`.
fn stake(b: &mut W, x: f32, z: f32, top: f32, r: f32) {
    b.bx(Kind::Floor, [x - r, x + r], [0.0, top], [z - r, z + r]);
    b.pyr(Kind::Floor, x, z, r, r, top, r * 2.2);
}

fn palisade(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // A row of sharpened logs lashed with two rails, no two of a height; a cresset on the middle one when it is manned.
    b.bx(Kind::Mud, [0.0, w], [0.0, 0.5], [d / 2.0 - 3.0, d / 2.0 + 3.0]);
    for k in 0..5 {
        let x = 1.6 + k as f32 * 3.2;
        let top = 19.0 + (b.r(k) % 5) as f32;
        stake(b, x, d / 2.0, top, 1.3);
    }
    for y in [8.0, 14.0] {
        b.bx(Kind::Cloth, [0.0, w], [y, y + 1.0], [d / 2.0 + 1.2, d / 2.0 + 2.0]);
    }
    b.bx(Kind::Metal, [w / 2.0 - 1.0, w / 2.0 + 1.0], [22.0, 23.5], [d / 2.0 - 1.0, d / 2.0 + 1.0]);
    b.fire(w / 2.0, 23.5, d / 2.0, 2.2);
}

fn wood_gate(b: &mut W) {
    let (w, d) = (b.w, b.d);
    // Two heavy posts with pointed heads, a lintel, two leaves of boards with iron straps; the leaves stand open a little while carts go through.
    for x in [1.0, w - 4.0] {
        b.bx(Kind::Floor, [x, x + 3.0], [0.0, 26.0], [d / 2.0 - 2.0, d / 2.0 + 2.0]);
        b.pyr(Kind::Floor, x + 1.5, d / 2.0, 1.5, 2.0, 26.0, 4.0);
    }
    b.bx(Kind::Floor, [1.0, w - 1.0], [22.0, 25.0], [d / 2.0 - 2.0, d / 2.0 + 2.0]);
    let gap = if b.work { 2.0 + (b.turn().cos() + 1.0) * 3.0 } else { 0.0 };
    let mid = w / 2.0;
    for (x0, x1) in [(4.0, mid - gap), (mid + gap, w - 4.0)] {
        if x1 - x0 < 1.0 {
            continue;
        }
        b.bx(Kind::Floor, [x0, x1], [0.0, 20.0], [d / 2.0 - 0.8, d / 2.0 + 0.8]);
        for y in [5.0, 14.0] {
            b.bx(Kind::Metal, [x0, x1], [y, y + 1.4], [d / 2.0 + 0.8, d / 2.0 + 1.4]);
        }
    }
}

fn watchtower(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w / 2.0, d / 2.0);
    // A stone footing, four timber legs leaning in, cross braces, a deck with a palisade rail, a shingled roof on posts, a ladder up the front;
    // an archer at the rail and a cresset when it is manned.
    b.bx(Kind::Stone, [cx - 12.0, cx + 12.0], [0.0, 4.0], [cz - 12.0, cz + 12.0]);
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        b.bx(Kind::Floor, [cx + sx * 10.0 - 1.2, cx + sx * 10.0 + 1.2], [4.0, 44.0], [cz + sz * 10.0 - 1.2, cz + sz * 10.0 + 1.2]);
        b.bx(Kind::Floor, [cx + sx * 8.0 - 0.8, cx + sx * 8.0 + 0.8], [44.0, 52.0], [cz + sz * 8.0 - 0.8, cz + sz * 8.0 + 0.8]);
    }
    for y in [14.0, 28.0] {
        b.bx(Kind::Floor, [cx - 10.0, cx + 10.0], [y, y + 1.2], [cz - 10.0, cz - 8.8]);
        b.bx(Kind::Floor, [cx - 10.0, cx + 10.0], [y, y + 1.2], [cz + 8.8, cz + 10.0]);
    }
    b.bx(Kind::Floor, [cx - 12.0, cx + 12.0], [44.0, 46.0], [cz - 12.0, cz + 12.0]);
    for k in 0..6 {
        let x = cx - 11.0 + k as f32 * 4.4;
        stake(b, x, cz + 11.0, 50.0 + (b.r(k) % 3) as f32, 1.0);
        stake(b, x, cz - 11.0, 50.0 + (b.r(k + 7) % 3) as f32, 1.0);
    }
    b.hip(Kind::Roof, [cx - 13.0, cx + 13.0], [cz - 13.0, cz + 13.0], 62.0, 9.0);
    b.bx(Kind::Floor, [cx - 1.0, cx + 1.0], [4.0, 44.0], [cz + 12.0, cz + 13.0]);
    for y in (6..44).step_by(5) {
        b.bx(Kind::Floor, [cx - 3.0, cx + 3.0], [y as f32, y as f32 + 0.8], [cz + 12.0, cz + 13.4]);
    }
    if b.work {
        let sway = (b.turn().sin() * 1.2).round();
        b.bx(Kind::Cloth, [cx - 2.0 + sway, cx + 2.0 + sway], [46.0, 56.0], [cz + 8.0, cz + 10.0]);
        b.ball(Kind::Plaster, cx + sway, 58.0, cz + 9.0, 1.7, 1.7, 1.7);
        b.bx(Kind::Metal, [cx + 7.0, cx + 9.0], [46.0, 48.0], [cz + 8.0, cz + 10.0]);
        b.fire(cx + 8.0, 48.0, cz + 9.0, 2.0);
    }
}

fn brazier(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w / 2.0, d / 2.0);
    // Three iron legs, a bowl, coals or ash; a fire when it is lit.
    for a in [0.0f32, 2.1, 4.2] {
        b.bx(Kind::Metal, [cx + a.cos() * 3.0 - 0.5, cx + a.cos() * 3.0 + 0.5], [0.0, 9.0], [cz + a.sin() * 3.0 - 0.5, cz + a.sin() * 3.0 + 0.5]);
    }
    b.cyl(Kind::Metal, cx, cz, 4.5, 9.0, 3.0);
    b.cyl(Kind::Rock, cx, cz, 3.6, 11.6, 0.6);
    b.fire(cx, 12.2, cz, 4.0);
    b.smoke(cx, 17.0, cz, 12.0, 3);
}
