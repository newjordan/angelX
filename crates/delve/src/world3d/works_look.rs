//! What the Works' buildings are made of, as colour (docs/REALM-MONOLITH.md,
//! "The look"): the look puts every pixel of a place on one eight-step ramp,
//! so on the hillside every building comes out the grass's green. A zone gives
//! a rounded box of ground a ramp of its own (`angel_stage::look::Zone`), as
//! the Keep's limestone (`works_site::dye`) and the camps' cloth
//! (`realm_look::camp_zones`) have. This is the same for a building: each
//! plan is made of something ([`Material`]), each material has a ramp
//! ([`ramp`]), and the buildings near the view get a zone of their material's
//! ramp over their footprint ([`building_zones`], [`dye_buildings`]).
//!
//! - **The ramps** keep the hillside's brightness ladder entry for entry
//!   (docs/BRAND.md: one value ladder for every place) and only lean its hue,
//!   as `realm_look::cloth_ramp` and `works_site::stone_ramp` do, so the
//!   brand's tonal-greys rule holds: no ramp is more coloured than
//!   `angel_stage::brand::ENV_CHROMA`.
//! - **The zones** are as tight as the footprint they dye (radius 0, a few
//!   pixels of feather), so the ground round a building keeps the green.
//!   Neighbours of one material that stand close and together are one zone (a
//!   street of sawpits is one timber zone), for a look keeps sixteen zones
//!   (`angel_stage::look::ZONE_MAX`) and a dense works has dozens of
//!   buildings. When there are more zones than room, the ones nearest the
//!   view's middle win.
//!
//! The order a stage frame makes a look in: `realm_look::look` (the biomes and
//! the camps), then [`dye_buildings`], then `works_site::dye` (the Keep, which
//! lies over them all). [`dye_buildings`] leaves the Keep its slot.

use std::sync::OnceLock;

use angel_stage::brand::{ENV_CHROMA, chroma, luma};
use angel_stage::look::{Look, ZONE_MAX, Zone};

use super::realm_look::{HILLSIDE, KEEP_ZONES, SEEN_BEYOND, View};
use super::works_site;

#[cfg(test)]
#[path = "../../tests/world3d/works_look__tests.rs"]
mod tests;

/// What a building is mostly made of, which is the colour it reads as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    /// Planks and posts: sheds, mills, cranes, carts.
    Timber,
    /// Cut stone and rock: quarries, walls, towers.
    Stone,
    /// Fired clay: the brick kilns and what is built of their brick.
    Brick,
    /// Straw and hay: a farm's roofs and fields.
    Thatch,
    /// Iron, lead and slate: the smiths and the ley engines.
    Iron,
    /// Limewash and plaster: houses, lime, the windmill's tower. On the
    /// hillside's ladder the nearest to white is a pale, nearly neutral grey.
    Whitewash,
    /// Dug works: pits, ponds, the clamp.
    Earth,
}

impl Material {
    /// Every material, for the tests and the sheets.
    pub const ALL: [Material; 7] =
        [Material::Timber, Material::Stone, Material::Brick, Material::Thatch, Material::Iron, Material::Whitewash, Material::Earth];
}

/// What each plan is mostly made of, in one place. The flat kit has a
/// `Mat` a plan (`render::overworld::works_kit`: stone, timber, brick,
/// earth) for what its construction piles up; this is the finer choice of what
/// the finished building looks like from the camera, so a works reads as many
/// things and not one mass:
/// - the smiths, the bloomery, the lead works and the ley engines are iron
///   (their hot, dark, metal look), not the stone their walls are;
/// - the kilns and the bakery are brick (a warm red-brown); the lime kiln,
///   the slaking works and the mills' towers are limewashed, as lime is white;
/// - a farm's roofs and fields, the rope walk and the weaving shed are straw;
/// - a cottage is a whitewashed house, a hut thatched;
/// - pits, ponds, the clamp and the tanyard are earth.
const TABLE: &[(&str, Material)] = &[
    // The stone and the quarry.
    ("field-quarry", Material::Stone),
    ("rubble-pit", Material::Earth),
    ("claim-quarry", Material::Stone),
    ("banker", Material::Stone),
    ("tracing-floor", Material::Whitewash),
    ("slater", Material::Stone),
    ("mason", Material::Stone),
    ("well", Material::Stone),
    ("quay", Material::Stone),
    ("caravanserai", Material::Stone),
    ("wall", Material::Stone),
    ("castle-yard", Material::Stone),
    // The lime, the mortar and the sand.
    ("lime-kiln", Material::Whitewash),
    ("slaking-pit", Material::Earth),
    ("sand-pit", Material::Earth),
    ("mortar-mill", Material::Timber),
    ("intake", Material::Timber),
    // Wood.
    ("woodsman-camp", Material::Timber),
    ("sawpit", Material::Timber),
    ("carpentry", Material::Timber),
    ("wheelwright", Material::Timber),
    ("retting-pond", Material::Earth),
    ("ropewalk", Material::Thatch),
    ("clamp", Material::Earth),
    // Food and cloth.
    ("farmstead", Material::Thatch),
    ("pasture", Material::Thatch),
    ("vineyard", Material::Thatch),
    ("wine-press", Material::Timber),
    ("mill", Material::Whitewash),
    ("windmill", Material::Whitewash),
    ("bakery", Material::Brick),
    ("brewery", Material::Timber),
    ("tanyard", Material::Earth),
    ("weaving-shed", Material::Thatch),
    // Fire and metal.
    ("ashery", Material::Earth),
    ("glassworks", Material::Brick),
    ("bloomery", Material::Iron),
    ("forge-shop", Material::Iron),
    ("smithy", Material::Iron),
    ("lead-works", Material::Iron),
    ("forge", Material::Iron),
    // Brick.
    ("clay-pit", Material::Earth),
    ("brick-kiln", Material::Brick),
    ("brick-cottage", Material::Brick),
    ("brick-wall", Material::Brick),
    ("brick-gate", Material::Brick),
    // What stands against the Deep.
    ("palisade", Material::Timber),
    ("wood-gate", Material::Timber),
    ("watchtower", Material::Timber),
    ("brazier", Material::Iron),
    // Halls and the commons.
    ("lodge", Material::Whitewash),
    ("ditch-gang", Material::Earth),
    ("depot", Material::Timber),
    ("carters-yard", Material::Timber),
    ("hut", Material::Thatch),
    ("cottage", Material::Whitewash),
    ("warehouse", Material::Timber),
    ("camp", Material::Timber),
    // Power and the ley.
    ("treadwheel-crane", Material::Timber),
    ("hod-gang", Material::Timber),
    ("great-crane", Material::Timber),
    ("treadwheel", Material::Timber),
    ("swape", Material::Timber),
    ("aqueduct", Material::Stone),
    ("horse-gin", Material::Timber),
    ("waterwheel", Material::Timber),
    ("drive-shaft", Material::Timber),
    ("leyline", Material::Stone),
    ("mana-engine", Material::Iron),
    ("conduit", Material::Iron),
    ("enchanter", Material::Stone),
    // The shafts that lift from the Delve: plain tackle, then the ley's.
    ("shaft-hand", Material::Timber),
    ("shaft-windlass", Material::Timber),
    ("shaft-gin", Material::Timber),
    ("shaft-water", Material::Timber),
    ("shaft-ley", Material::Iron),
];

/// What plan `plan_id` is mostly made of ([`TABLE`]). A plan the table does
/// not know (the sim's older ids) is timber, the commonest.
pub fn material_of(plan_id: &str) -> Material {
    // The stretches of line laid on the ground (`world3d::hillside`) are not plans: a road, rails, a
    // ropeway, a chute, a rollerway and a mule's way are all timber, the warm brown the realm's works are drawn in.
    if matches!(plan_id, "road-run" | "tram-run" | "chute-run" | "rope-run" | "mule-run" | "roller-run") {
        return Material::Timber;
    }
    TABLE.iter().find(|(id, _)| *id == plan_id).map_or(Material::Timber, |&(_, m)| m)
}

// ------------------------------------------------------------ the ramps ----

/// The hue a material leans its ramp to, and how coloured it goes: chroma per
/// unit of brightness (capped at the brand's `ENV_CHROMA`, so the dim entries
/// stay dim and the light ones are the most coloured). The stone's is
/// `works_site::stone_ramp`.
fn lean_of(m: Material) -> ([u8; 3], f32) {
    match m {
        Material::Timber => ([160, 100, 56], 1.0),
        Material::Stone => ([0xc0, 0xb4, 0x9c], 0.21),
        Material::Brick => ([176, 64, 56], 1.0),
        Material::Thatch => ([210, 184, 64], 1.0),
        Material::Iron => ([88, 118, 156], 0.8),
        // Nearly neutral and a little cool: the ladder has no brighter step to
        // make it white with, and a warm grey is the limestone's own.
        Material::Whitewash => ([200, 210, 214], 0.10),
        // A dusty loam: the stone's brightness with a red-brown lean, a good
        // deal less coloured than the timber or the brick. The ladder leaves
        // this corner of the colour wheel crowded; the stone, the timber and the
        // brick are each about 0.04 of it away.
        Material::Earth => ([178, 89, 71], 0.32),
    }
}

/// A ramp of hue `hue`, entry for entry as bright as the hillside's (the
/// eight-step ladder holds, docs/BRAND.md): the colour's lean from grey,
/// `ratio` of each entry's brightness in chroma and never over the guide's,
/// kept inside what a colour can be at that brightness (as
/// `realm_look::cloth_ramp`, with the chroma chosen instead of the cloth's).
fn leaned(hue: [u8; 3], ratio: f32) -> [[u8; 3]; 8] {
    let u = hue.map(|c| f32::from(c) / 255.0);
    let own = luma(hue);
    let lean = u.map(|c| c - own);
    let spread = chroma(hue).max(1e-3);
    HILLSIDE.map(|entry| {
        let l = luma(entry);
        let mut k = (ratio * l).min(ENV_CHROMA) / spread;
        for d in lean {
            if d > 1e-4 {
                k = k.min((1.0 - l) / d);
            } else if d < -1e-4 {
                k = k.min(l / -d);
            }
        }
        lean.map(|d| ((l + d * k) * 255.0).round().clamp(0.0, 255.0) as u8)
    })
}

/// Material `m`'s ramp, dark to light: the hillside's brightness ladder, entry
/// for entry, in the material's hue.
pub fn ramp(m: Material) -> [[u8; 3]; 8] {
    static RAMPS: OnceLock<[[[u8; 3]; 8]; 7]> = OnceLock::new();
    let all = RAMPS.get_or_init(|| {
        Material::ALL.map(|m| match m {
            Material::Stone => *works_site::stone_ramp(),
            _ => {
                let (hue, ratio) = lean_of(m);
                leaned(hue, ratio)
            }
        })
    });
    let index = Material::ALL.iter().position(|&x| x == m).unwrap_or(0);
    all[index]
}

// ------------------------------------------------------------ the zones ----

/// How far past a building's footprint its dye fades out (kingdom pixels):
/// a few, so the ground round it keeps the grass. (The camps' tents: 8.)
pub const FEATHER: f32 = 4.0;
/// How far apart two buildings of one material may stand to be one zone: two
/// tiles.
pub const MERGE_GAP: f32 = 32.0;
/// How much bigger than the footprints it holds a merged zone's box may be.
pub const MERGE_SLACK: f32 = 1.5;
/// The most zones the buildings may have: all but the Keep's. (What is left
/// in a look is [`dye_buildings`]'s to fit.)
pub const BUILDING_ZONES: usize = ZONE_MAX - KEEP_ZONES;

/// A building as the look sees it: where it stands and what it is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    /// `[x0, z0, x1, z1]` in the kingdom's pixels (16 a tile).
    pub rect: [f32; 4],
    pub material: Material,
}

impl Footprint {
    /// Plan `plan_id`'s building on `rect`.
    pub fn of(plan_id: &str, rect: [f32; 4]) -> Self {
        Self { rect, material: material_of(plan_id) }
    }
}

fn area(r: [f32; 4]) -> f32 {
    (r[2] - r[0]).max(0.0) * (r[3] - r[1]).max(0.0)
}

fn bound(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
}

/// How far apart two boxes stand: the larger of the gaps along each axis (0
/// when they touch or overlap).
fn gap(a: [f32; 4], b: [f32; 4]) -> f32 {
    let dx = (a[0] - b[2]).max(b[0] - a[2]).max(0.0);
    let dz = (a[1] - b[3]).max(b[1] - a[3]).max(0.0);
    dx.max(dz)
}

fn overlap(a: [f32; 4], b: [f32; 4]) -> f32 {
    area([a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])])
}

/// A box of buildings of one material: the box that holds them and the sum of
/// their footprints.
struct Group {
    rect: [f32; 4],
    sum: f32,
    material: Material,
}

/// The streets near `view`: the buildings in sight (the view and
/// [`SEEN_BEYOND`] round it) merged where neighbours of one material stand
/// within [`MERGE_GAP`] and the box that holds them is at most [`MERGE_SLACK`]
/// times their footprints (and covers no building of another material), each
/// as its material and its box `[x0, z0, x1, z1]` in the kingdom's pixels; the
/// nearest the view's middle first.
pub fn street_groups(view: &View, buildings: &[Footprint]) -> Vec<(Material, [f32; 4])> {
    let [ox, oz] = view.origin;
    let centre = [ox + view.size[0] / 2.0, oz + view.size[1] / 2.0];
    let m = SEEN_BEYOND + FEATHER;
    let usable = |b: &Footprint| b.rect.iter().all(|c| c.is_finite()) && b.rect[2] > b.rect[0] && b.rect[3] > b.rect[1];
    let mut groups: Vec<Group> = buildings
        .iter()
        .filter(|b| usable(b))
        .filter(|b| {
            let r = b.rect;
            r[2] + m > ox && r[0] - m < ox + view.size[0] && r[3] + m > oz && r[1] - m < oz + view.size[1]
        })
        .map(|b| Group { rect: b.rect, sum: area(b.rect), material: b.material })
        .collect();
    // Merge the pair whose box is the tightest round what it holds, until none
    // may be.
    loop {
        let mut best: Option<(f32, usize, usize)> = None;
        for i in 0..groups.len() {
            for j in i + 1..groups.len() {
                let (a, b) = (&groups[i], &groups[j]);
                if a.material != b.material || gap(a.rect, b.rect) > MERGE_GAP {
                    continue;
                }
                let union = bound(a.rect, b.rect);
                let ratio = area(union) / (a.sum + b.sum);
                if ratio > MERGE_SLACK || best.is_some_and(|(r, ..)| r <= ratio) {
                    continue;
                }
                // Never a box that covers a building of another material.
                if buildings.iter().any(|o| o.material != a.material && usable(o) && overlap(union, o.rect) > 1.0) {
                    continue;
                }
                best = Some((ratio, i, j));
            }
        }
        let Some((_, i, j)) = best else { break };
        let b = groups.swap_remove(j);
        let a = &mut groups[i];
        a.rect = bound(a.rect, b.rect);
        a.sum += b.sum;
    }
    // The nearest the view's middle (to the box, not its middle: a long street
    // is near wherever it passes).
    let away = |r: [f32; 4]| (r[0] - centre[0]).max(centre[0] - r[2]).max(0.0).hypot((r[1] - centre[1]).max(centre[1] - r[3]).max(0.0));
    groups.sort_by(|a, b| away(a.rect).total_cmp(&away(b.rect)));
    groups.into_iter().map(|g| (g.material, g.rect)).collect()
}

/// The zones that dye the buildings near `view`: its [`street_groups`], the
/// nearest at most [`BUILDING_ZONES`] of them; the farthest first, so the
/// nearest lies over them (as `realm_look::camp_zones`). Each is as tight as
/// its box: radius 0 and a [`FEATHER`].
pub fn building_zones(view: &View, buildings: &[Footprint]) -> Vec<Zone> {
    let [ox, oz] = view.origin;
    let mut groups = street_groups(view, buildings);
    groups.truncate(BUILDING_ZONES);
    groups.reverse();
    groups
        .into_iter()
        .map(|(material, [x0, z0, x1, z1])| Zone {
            ramp: ramp(material).to_vec(),
            at: [(x0 + x1) / 2.0 - ox, (z0 + z1) / 2.0 - oz],
            half: [(x1 - x0) / 2.0, (z1 - z0) / 2.0],
            radius: 0.0,
            feather: FEATHER,
        })
        .collect()
}

/// Dye the look over the buildings near `view` ([`building_zones`]), after the
/// biomes' and the camps' zones (`realm_look::look`) and before the Keep's
/// (`works_site::dye`, which pushes its zone last): the buildings take what
/// room the look has left, less the Keep's slot, the nearest the view's middle
/// first. Their zones lie over the camps' cloth and under the Keep.
pub fn dye_buildings(look: &mut Look, view: &View, buildings: &[Footprint]) {
    let room = ZONE_MAX.saturating_sub(KEEP_ZONES).saturating_sub(look.zones.len());
    let mut zones = building_zones(view, buildings);
    // The farthest are first: those are what give way.
    zones.drain(..zones.len().saturating_sub(room));
    look.zones.extend(zones);
}
