//! The Works' material layer: every plan made of something, a ramp a
//! material that keeps the hillside's ladder and leans its hue, and the
//! buildings near the view dyed in zones that merge a street, keep the Keep
//! its slot and give way to the nearest.

use super::*;
use crate::render::overworld::works_kit as flat;
use crate::world3d::realm_look::{self, BANNER_FEATHER, CAMP_ZONES};
use angel_stage::brand::{audit_ramp, hue, ladder_of};
use angel_stage::look::unit_rgb;

/// The plans the sim has besides the kit's seventy-two: none now, the kit draws every one.
const EXTRA: [&str; 0] = [];

fn view() -> View {
    View { origin: [0.0, 0.0], size: [480.0, 270.0], knight: None, others: [None; 3], time: 0.0 }
}

fn at(material: Material, x: f32, z: f32, w: f32, d: f32) -> Footprint {
    Footprint { rect: [x, z, x + w, z + d], material }
}

/// The tonal distance between two ramps' same entry: how far apart two
/// colours of one brightness are (what their hue and chroma make).
fn apart(a: &[[u8; 3]; 8], b: &[[u8; 3]; 8], entry: usize) -> f32 {
    let (a, b) = (unit_rgb(a[entry]), unit_rgb(b[entry]));
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

#[test]
fn every_plan_has_a_material_in_the_table_once() {
    let listed = |id: &str| TABLE.iter().filter(|(t, _)| *t == id).count();
    let mut kit = 0;
    for id in flat::plans() {
        assert_eq!(listed(id), 1, "{id} is in the table once");
        kit += 1;
    }
    assert_eq!(kit, 76, "the hillside's fifty-eight, the carter's yard, the castle yard, the camp, the five shaft heads, the six the first kit left undrawn and the four that stand against the Deep");
    for id in EXTRA {
        assert_eq!(listed(id), 1, "{id} is in the table once");
    }
    assert_eq!(TABLE.len(), kit + EXTRA.len(), "and nothing else is");
    // A plan the table does not know is timber.
    assert_eq!(material_of("no-such-plan"), Material::Timber);
    // Every material is some plan's: no ramp is made for nothing.
    for m in Material::ALL {
        assert!(TABLE.iter().any(|&(_, t)| t == m), "{m:?} is used");
    }
    // Spot checks of the readings the table is for.
    assert_eq!(material_of("smithy"), Material::Iron);
    assert_eq!(material_of("bakery"), Material::Brick);
    assert_eq!(material_of("brick-kiln"), Material::Brick);
    assert_eq!(material_of("cottage"), Material::Whitewash);
    assert_eq!(material_of("farmstead"), Material::Thatch);
    assert_eq!(material_of("pasture"), Material::Thatch);
    assert_eq!(material_of("sawpit"), Material::Timber);
    assert_eq!(material_of("claim-quarry"), Material::Stone);
    assert_eq!(material_of("clamp"), Material::Earth);
}

#[test]
fn a_ramp_is_the_hillsides_ladder_entry_for_entry_in_its_own_hue() {
    for m in Material::ALL {
        let r = ramp(m);
        assert_eq!(r.len(), 8);
        for (i, (entry, base)) in r.iter().zip(HILLSIDE).enumerate() {
            // The same brightness as the hillside's step, so the brand's
            // eight steps hold: only the hue is the material's.
            assert!((luma(*entry) - luma(base)).abs() < 0.012, "{m:?}[{i}] {entry:?} against {base:?}");
            // Tonal greys: never more coloured than the guide allows.
            assert!(chroma(*entry) <= ENV_CHROMA + 1e-4, "{m:?}[{i}] chroma {}", chroma(*entry));
        }
        // Dark to light, every step.
        for i in 1..8 {
            assert!(luma(r[i]) > luma(r[i - 1]), "{m:?} climbs at {i}");
        }
        // As much of the guide as the hillside keeps: its colour and one hue.
        let breaches: Vec<_> = audit_ramp("material", &r, &ladder_of(&r)).into_iter().filter(|b| b.rule != "ramp-value").collect();
        assert_eq!(breaches, vec![], "{m:?}");
        // No hypercontrast: nothing brighter than the hillside's top.
        assert!(luma(r[7]) <= luma(HILLSIDE[7]) + 0.012);
    }
    // The stone is the Keep's.
    assert_eq!(ramp(Material::Stone), *works_site::stone_ramp());
}

#[test]
fn the_ramps_are_clearly_different_from_each_other_and_from_the_grass() {
    // Entries from the mid steps up carry the hue; the darkest few are near black.
    for entry in [4, 5, 6] {
        let green = HILLSIDE;
        for (i, a) in Material::ALL.iter().enumerate() {
            let d = apart(&ramp(*a), &green, entry);
            assert!(d >= 0.06, "{a:?} against the grass at {entry}: {d:.3}");
            for b in &Material::ALL[i + 1..] {
                let d = apart(&ramp(*a), &ramp(*b), entry);
                assert!(d >= 0.04, "{a:?} against {b:?} at {entry}: {d:.3}");
            }
        }
    }
    // The coloured ones lean their own way: a hue each, told apart round the wheel.
    let hues: Vec<(Material, f32)> = [Material::Timber, Material::Brick, Material::Thatch, Material::Iron]
        .into_iter()
        .map(|m| (m, hue(ramp(m)[6]).expect("a hue")))
        .collect();
    let grass = hue(HILLSIDE[6]).unwrap();
    for (i, (a, ha)) in hues.iter().enumerate() {
        assert!(((ha - grass + 540.0).rem_euclid(360.0) - 180.0).abs() > 30.0, "{a:?} {ha} against the grass {grass}");
        for (b, hb) in &hues[i + 1..] {
            assert!(((ha - hb + 540.0).rem_euclid(360.0) - 180.0).abs() > 12.0, "{a:?} {ha} against {b:?} {hb}");
        }
    }
}

#[test]
fn a_street_of_one_material_is_one_zone() {
    // Five sawpits, 3 x 2 tiles, a tile between them.
    let street: Vec<Footprint> = (0..5).map(|k| at(Material::Timber, 100.0 + k as f32 * 64.0, 100.0, 48.0, 32.0)).collect();
    let zones = building_zones(&view(), &street);
    assert_eq!(zones.len(), 1, "{zones:?}");
    let z = &zones[0];
    assert_eq!(z.ramp, ramp(Material::Timber).to_vec());
    // As tight as the street: the box of the five.
    assert_eq!((z.at, z.half), ([100.0 + 304.0 / 2.0 - 0.0, 116.0], [152.0, 16.0]));
    assert_eq!((z.radius, z.feather), (0.0, FEATHER));
}

#[test]
fn neighbours_merge_only_when_near_alike_and_not_much_bigger_together() {
    let zones = |b: &[Footprint]| building_zones(&view(), b).len();
    // Three tiles apart is too far (two is the most).
    assert_eq!(zones(&[at(Material::Timber, 100.0, 100.0, 48.0, 32.0), at(Material::Timber, 196.0, 100.0, 48.0, 32.0)]), 2);
    assert_eq!(zones(&[at(Material::Timber, 100.0, 100.0, 48.0, 32.0), at(Material::Timber, 180.0, 100.0, 48.0, 32.0)]), 1, "two tiles apart merge");
    // Another material beside it is its own.
    assert_eq!(zones(&[at(Material::Timber, 100.0, 100.0, 48.0, 32.0), at(Material::Stone, 150.0, 100.0, 48.0, 32.0)]), 2);
    // A corner: the box that holds an L is more than a half bigger than the L.
    assert_eq!(zones(&[at(Material::Timber, 100.0, 100.0, 96.0, 16.0), at(Material::Timber, 100.0, 116.0, 16.0, 96.0)]), 2);
    // Two sheds two tiles apart are one; a stone house standing between them
    // is not swallowed by the box that would hold them, so they are not.
    let (a, b) = (at(Material::Timber, 100.0, 100.0, 48.0, 32.0), at(Material::Timber, 180.0, 100.0, 48.0, 32.0));
    assert_eq!(zones(&[a, b]), 1);
    assert_eq!(zones(&[a, at(Material::Stone, 156.0, 100.0, 16.0, 32.0), b]), 3);
    // Degenerate and non-finite boxes are no building.
    assert_eq!(zones(&[at(Material::Timber, 100.0, 100.0, 0.0, 32.0), at(Material::Timber, f32::NAN, 100.0, 48.0, 32.0)]), 0);
}

#[test]
fn only_what_is_in_sight_is_dyed() {
    let v = view();
    let near = at(Material::Stone, v.size[0] + SEEN_BEYOND - 50.0, 100.0, 48.0, 32.0);
    let far = at(Material::Stone, v.size[0] + SEEN_BEYOND + FEATHER + 10.0, 100.0, 48.0, 32.0);
    let behind = at(Material::Stone, -200.0, 100.0, 48.0, 32.0);
    assert_eq!(building_zones(&v, &[near]).len(), 1);
    assert_eq!(building_zones(&v, &[far, behind]).len(), 0);
    // The zone is in the stage's own pixels, the kingdom's less the origin.
    let moved = View { origin: [1000.0, 500.0], ..v };
    let zones = building_zones(&moved, &[at(Material::Iron, 1100.0, 600.0, 32.0, 32.0)]);
    assert_eq!((zones[0].at, zones[0].half), ([116.0, 116.0], [16.0, 16.0]));
}

#[test]
fn the_nearest_win_the_budget_and_lie_over_the_rest() {
    // Forty lone buildings in a grid, far apart enough that none merge.
    let all: Vec<Footprint> = (0..40)
        .map(|k| at(Material::ALL[k % 7], 8.0 + (k % 8) as f32 * 58.0, 8.0 + (k / 8) as f32 * 52.0, 24.0, 24.0))
        .collect();
    let zones = building_zones(&view(), &all);
    assert_eq!(zones.len(), BUILDING_ZONES, "as many as the look has room for, the Keep's slot kept");
    assert_eq!(BUILDING_ZONES, ZONE_MAX - KEEP_ZONES);
    // Farthest first: the distances from the view's middle (to the box) fall.
    let away = |r: [f32; 4]| (r[0] - 240.0).max(240.0 - r[2]).max(0.0).hypot((r[1] - 135.0).max(135.0 - r[3]).max(0.0));
    let rect_of = |z: &Zone| [z.at[0] - z.half[0], z.at[1] - z.half[1], z.at[0] + z.half[0], z.at[1] + z.half[1]];
    for pair in zones.windows(2) {
        assert!(away(rect_of(&pair[0])) >= away(rect_of(&pair[1])) - 1e-3, "the nearest lie last");
    }
    // And they are the nearest of the forty: nothing left out is nearer than the farthest kept.
    let kept_far = away(rect_of(&zones[0]));
    let left_out_nearer = all
        .iter()
        .filter(|b| !zones.iter().any(|z| (rect_of(z)[0] - b.rect[0]).abs() < 1e-3 && (rect_of(z)[1] - b.rect[1]).abs() < 1e-3))
        .filter(|b| away(b.rect) < kept_far - 1e-3)
        .count();
    assert_eq!(left_out_nearer, 0);
    assert_eq!(all.len() - zones.len(), 40 - BUILDING_ZONES);
    // The streets themselves are all forty, the nearest first.
    let groups = street_groups(&view(), &all);
    assert_eq!(groups.len(), 40);
    assert!(groups.windows(2).all(|g| away(g[0].1) <= away(g[1].1) + 1e-3));
}

#[test]
fn the_keep_keeps_its_slot_and_the_camps_theirs() {
    let street = |n: usize| -> Vec<Footprint> { (0..n).map(|k| at(Material::ALL[k % 7], 8.0 + (k % 8) as f32 * 58.0, 8.0 + (k / 8) as f32 * 52.0, 24.0, 24.0)).collect() };
    // An empty look: the buildings take what the Keep's slots leave.
    let mut look = Look::new(HILLSIDE.to_vec());
    dye_buildings(&mut look, &view(), &street(40));
    assert_eq!(look.zones.len(), ZONE_MAX - KEEP_ZONES);
    // The Keep's zones are pushed after, as `works_site::dye` does (it cuts to
    // ZONE_MAX - KEEP_ZONES and pushes its limestone, its yard and its lane), and the look is still whole.
    look.zones.truncate(ZONE_MAX - KEEP_ZONES);
    let before = look.zones.clone();
    for _ in 0..KEEP_ZONES {
        look.zones.push(Zone { ramp: ramp(Material::Stone).to_vec(), at: [200.0, 100.0], half: [100.0, 100.0], radius: 18.0, feather: 22.0 });
    }
    assert_eq!(&look.zones[..ZONE_MAX - KEEP_ZONES], &before[..], "the Keep took its free slots and cut no building's");
    assert_eq!(look.zones.len(), ZONE_MAX);
    assert_eq!(look.validate(), Ok(()));
    // A look already holding the camps' cloth: the buildings get the rest, the
    // nearest of them.
    let mut look = Look::new(HILLSIDE.to_vec());
    look.zones = vec![Zone { ramp: ramp(Material::Brick).to_vec(), at: [10.0, 10.0], half: [8.0, 8.0], radius: 0.0, feather: BANNER_FEATHER }; CAMP_ZONES];
    dye_buildings(&mut look, &view(), &street(40));
    assert_eq!(look.zones.len(), ZONE_MAX - KEEP_ZONES);
    assert_eq!(&look.zones[..CAMP_ZONES], &vec![look.zones[0].clone(); CAMP_ZONES][..], "the camps' zones are as they were, first");
    let full = building_zones(&view(), &street(40));
    assert_eq!(&look.zones[CAMP_ZONES..], &full[full.len() - (ZONE_MAX - KEEP_ZONES - CAMP_ZONES)..], "the nearest of the buildings, farthest first");
    // A look with no room gets nothing and loses nothing.
    let mut full_look = Look::new(HILLSIDE.to_vec());
    full_look.zones = vec![look.zones[0].clone(); ZONE_MAX - KEEP_ZONES];
    dye_buildings(&mut full_look, &view(), &street(40));
    assert_eq!(full_look.zones.len(), ZONE_MAX - KEEP_ZONES);
}

#[test]
fn a_look_dyed_over_the_real_realm_is_whole() {
    // The realm's own look for a view (whichever plan the tests run under), dyed.
    let v = View { origin: [1900.0, 1200.0], size: [480.0, 270.0], knight: Some([2100.0, 1300.0]), others: [None; 3], time: 1.0 };
    let mut look = realm_look::look(&v, &[]);
    let before = look.zones.len();
    let baseline = angel_stage::brand::audit_look(&look);
    let sheds: Vec<Footprint> = (0..6).map(|k| at(Material::ALL[k], 1960.0 + k as f32 * 70.0, 1300.0, 40.0, 30.0)).collect();
    dye_buildings(&mut look, &v, &sheds);
    assert_eq!(look.validate(), Ok(()));
    assert_eq!(look.zones.len(), before + 6);
    // The brand's audit finds nothing new in the zones' ramps but the
    // hillside's own brightness step (which the stone's has too).
    let new: Vec<_> = angel_stage::brand::audit_look(&look).into_iter().filter(|b| !baseline.contains(b)).collect();
    assert!(new.iter().all(|b| b.rule == "ramp-value"), "{new:?}");
    // And the same ramp-value breach the hillside's own ramp has, no worse.
    let hill = audit_ramp("hillside", &HILLSIDE, &ladder_of(&HILLSIDE));
    assert!(!hill.is_empty() || new.is_empty(), "the hillside's own ladder: {hill:?}");
}

#[test]
fn a_zone_dyes_its_footprint_and_not_the_ground_round_it() {
    let mut look = Look::new(HILLSIDE.to_vec());
    let v = view();
    let shed = at(Material::Thatch, 200.0, 100.0, 48.0, 32.0);
    look.zones = building_zones(&v, &[shed]);
    let entry = 5;
    let thatch = unit_rgb(ramp(Material::Thatch)[entry]);
    let grass = unit_rgb(HILLSIDE[entry]);
    let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5);
    assert!(close(look.ramp_at(224.0, 116.0)[entry], thatch), "its middle is thatch");
    assert!(close(look.ramp_at(201.0, 101.0)[entry], thatch), "its corner is thatch");
    assert!(close(look.ramp_at(200.0 - FEATHER - 1.0, 116.0)[entry], grass), "past the feather the grass is the grass");
    assert!(close(look.ramp_at(224.0, 132.0 + FEATHER + 1.0)[entry], grass));
    let edge = look.ramp_at(200.0 - FEATHER / 2.0, 116.0)[entry];
    assert!(!close(edge, thatch) && !close(edge, grass), "and between, in the feather, a mix");
}
