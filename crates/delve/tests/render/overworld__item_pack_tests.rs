use super::*;
use super::super::pickup_cards::PLATES;
use super::super::pickup_meshes::{DROP_FROM, PickupKind, ScenePickup, fall, plop};
use angel_stage::scene::{MeshRef, quat_rotate};

fn plate(name: &str) -> u8 {
    PLATES.iter().position(|&p| p == name).unwrap() as u8
}

#[test]
fn the_shipped_pack_reads_and_every_model_is_one_the_gpu_takes() {
    let models = read(PACK).expect("the pack reads");
    assert!(!models.is_empty());
    for (i, m) in models.iter().enumerate() {
        assert!(m.mesh.validate().is_ok(), "{}", m.name);
        assert!(models[..i].iter().all(|o| o.name != m.name), "{} twice", m.name);
        // Centred on its box, its longest side 1.
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &m.mesh.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let long = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f32::max);
        assert!((long - 1.0).abs() < 0.01, "{} is {long} long", m.name);
        assert!((0..3).all(|k| (hi[k] + lo[k]).abs() < 0.01), "{} is off centre", m.name);
        assert!(m.half.iter().all(|&h| h > 0.0 && h <= 0.5), "{}", m.name);
    }
}

#[test]
fn a_pack_that_is_not_one_reads_as_nothing() {
    assert!(read(b"").is_none());
    assert!(read(b"AIM0\0\0\0\0").is_none());
    let mut cut = PACK.to_vec();
    cut.truncate(PACK.len() - 3);
    assert!(read(&cut).is_none());
}

#[test]
fn only_the_approved_models_stand_for_their_drawings() {
    for name in APPROVED {
        let (id, _) = model(plate(name), false).unwrap_or_else(|| panic!("{name} is in the pack"));
        assert!(mesh(id).is_some(), "{name}");
    }
    // Looked at and turned down: these keep their cards.
    for name in ["bow", "crossbow", "handgonne", "key", "bone", "spell", "rune-haste", "chest"] {
        assert!(model(plate(name), false).is_none(), "{name}");
    }
}

#[test]
fn a_bottle_that_wards_is_pale_glass_and_one_that_mends_red() {
    let (red, mended) = model(plate("flask"), false).unwrap();
    let (pale, warded) = model(plate("flask"), true).unwrap();
    assert_ne!(red, pale);
    for id in [red, pale] {
        assert_eq!(mesh(id).map(|m| m.positions.len()), Some(mended.mesh.positions.len()));
    }
    let mean = |m: &Model| {
        let n = m.mesh.colors.len() as f32;
        m.mesh.colors.iter().fold([0.0; 3], |a, c| [a[0] + c[0] / n, a[1] + c[1] / n, a[2] + c[2] / n])
    };
    let (r, p) = (mean(mended), mean(warded));
    assert!(r[0] > 2.0 * r[2] && r[0] > 0.4, "a potion is red, and reads: {r:?}");
    assert!(p[2] > p[0] && p[2] > 0.5, "holy water is pale blue: {p:?}");
}

fn dropped(name: &str, age: u32, seed: u32) -> (ScenePickup, u8) {
    let item = ScenePickup { x: 30.0, y: 20.0, kind: PickupKind::Blade, plate: Some(plate(name)), ink: 'H', accent: 'Z', shine: false, tick: 500, age, seed, pale: false };
    (item, plate(name))
}

/// The node's lowest point over the floor.
fn low(node: &angel_stage::scene::Node, m: &Model) -> f32 {
    m.mesh
        .positions
        .iter()
        .map(|&p| quat_rotate(node.transform.rotation, p.map(|v| v * node.transform.scale[0]))[1] + node.transform.translation[1])
        .fold(f32::MAX, f32::min)
}

#[test]
fn a_dropped_sword_falls_hops_once_and_lies_flat_on_the_floor() {
    let (heights, turns): (Vec<f32>, Vec<f32>) = (0..40).map(|t| fall(t as f32 / crate::rules::HZ as f32)).unzip();
    assert!((heights[0] - DROP_FROM).abs() < 1e-3, "let go of at chest height");
    let landed = heights.iter().position(|&h| h <= 0.0).expect("it reaches the floor");
    assert!(heights[landed..].iter().any(|&h| h > 0.5), "a small hop");
    assert!(heights[landed..].iter().all(|&h| h < 2.5), "a small one");
    assert!(heights[30..].iter().all(|&h| h == 0.0) && turns[30..].iter().all(|&a| a == 0.0), "then still");
    let (item, p) = dropped("sword", u32::MAX, 77);
    let (id, m) = model(p, false).unwrap();
    let node = plop(9, &item, id, m, p, (0.0, 0.0));
    assert_eq!(node.mesh, Some(MeshRef::Uploaded(id)));
    // Lying: its long side level, resting on the floor, not floating.
    let long = quat_rotate(node.transform.rotation, [0.0, 1.0, 0.0]);
    assert!(long[1].abs() < 0.2, "lies along the floor: {long:?}");
    let floor = low(&node, m);
    assert!((-0.3..0.6).contains(&floor), "rests on the floor: {floor}");
    // Let go of, it is up in the air.
    let (item, _) = dropped("sword", 0, 77);
    assert!(low(&plop(9, &item, id, m, p, (0.0, 0.0)), m) > 3.0);
}

#[test]
fn each_drop_lies_its_own_way_and_the_same_drop_the_same_way() {
    let (a, p) = dropped("axe", u32::MAX, 1);
    let (b, _) = dropped("axe", u32::MAX, 2);
    let (id, m) = model(p, false).unwrap();
    let ra = plop(1, &a, id, m, p, (0.0, 0.0)).transform.rotation;
    assert_ne!(ra, plop(1, &b, id, m, p, (0.0, 0.0)).transform.rotation);
    assert_eq!(ra, plop(1, &a, id, m, p, (0.0, 0.0)).transform.rotation);
    // A bottle stands on its foot, near enough upright.
    let (bottle, p) = dropped("flask", u32::MAX, 5);
    let (id, m) = model(p, false).unwrap();
    let node = plop(2, &bottle, id, m, p, (0.0, 0.0));
    assert!(quat_rotate(node.transform.rotation, [0.0, 1.0, 0.0])[1] > 0.95);
}
