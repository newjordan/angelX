//! Every shot is a few solid shapes, in its sprite's inks, pointing the way it flies.
use super::super::super::ink::{PALETTE, rgb};
use super::*;
use super::super::room_scene::Hue;
use angel_stage::scene::{MeshRef, TextureSource, mat_point};

const KINDS: [Shot; 17] = [
    Shot::Blade,
    Shot::Arrow,
    Shot::Bolt,
    Shot::Ball,
    Shot::Bone,
    Shot::Orb,
    Shot::Ember,
    Shot::Sacred,
    Shot::Hex,
    Shot::Frost,
    Shot::Missile,
    Shot::Wisp,
    Shot::Ore,
    Shot::Flame,
    Shot::Page,
    Shot::Spore,
    Shot::Eye,
];

const COLOURS: [ForgedColour; 7] = [
    ForgedColour::Ember,
    ForgedColour::Ice,
    ForgedColour::Gold,
    ForgedColour::Venom,
    ForgedColour::Void,
    ForgedColour::Rose,
    ForgedColour::Plain,
];

const SHAPES: [ForgedShape; 6] = [
    ForgedShape::Arrow,
    ForgedShape::Shard,
    ForgedShape::Star,
    ForgedShape::Wave,
    ForgedShape::Coal,
    ForgedShape::Bolt,
];

fn shot(form: ShotForm, at: (f32, f32), v: (f32, f32)) -> SceneShot {
    SceneShot {
        x: at.0,
        y: at.1,
        vx: v.0,
        vy: v.1,
        hue: Hue::Cold,
        bright: true,
        hostile: false,
        empowered: false,
        form,
        dimension: Default::default(),
        mark: super::super::Img::new(1, 1),
    }
}

fn built(form: ShotForm, at: (f32, f32), v: (f32, f32)) -> Vec<Node> {
    let mut out = Vec::new();
    assert!(build(&mut out, 0, &shot(form, at, v), (0.0, 0.0)), "{form:?}");
    out
}

fn every_form() -> Vec<ShotForm> {
    let mut forms: Vec<ShotForm> = KINDS.iter().map(|&k| ShotForm::Kind(k)).collect();
    for colour in COLOURS {
        for shape in SHAPES {
            forms.push(ShotForm::Forged(colour, shape));
        }
    }
    forms.push(ShotForm::Class(ClassShot::Arcane));
    forms.push(ShotForm::Class(ClassShot::Holy));
    forms
}

#[test]
fn every_kind_of_shot_is_made_of_shapes_but_a_cards_own_drawing_is_not() {
    for form in every_form() {
        let nodes = built(form, (100.0, 80.0), (40.0, -20.0));
        assert!(!nodes.is_empty() && nodes.len() <= NODES_PER_SHOT as usize, "{form:?}: {} shapes", nodes.len());
    }
    let mut out = Vec::new();
    assert!(!build(&mut out, 0, &shot(ShotForm::Card, (1.0, 1.0), (1.0, 0.0)), (0.0, 0.0)));
    assert!(out.is_empty(), "a card's drawing stays a sprite");
    assert_eq!(count(ShotForm::Kind(Shot::Orb)), 2);
}

#[test]
fn the_shapes_are_the_games_inks_built_in_meshes_and_lit() {
    let palette: Vec<[u8; 3]> = PALETTE.iter().map(|&v| rgb(v)).collect();
    for form in every_form() {
        for node in built(form, (100.0, 80.0), (40.0, -20.0)) {
            let m = node.material;
            assert!(m.lit && m.source == TextureSource::None, "{form:?}: lit and flat");
            assert!(!matches!(node.mesh, Some(MeshRef::Uploaded(_)) | None), "{form:?}: a built-in mesh");
            assert!((0.0..=1.0).contains(&m.emissive) && m.tint[3] > 0.0 && m.tint[3] <= 1.0);
            let ink = [0, 1, 2].map(|k| (m.tint[k] * 255.0).round() as u8);
            assert!(palette.contains(&ink), "{form:?}: {ink:?} is not on the palette");
            assert_eq!(m.alpha.blended(), m.tint[3] < 1.0, "a veil blends and nothing else does");
            let t = node.transform;
            assert!(t.translation.iter().chain(&t.scale).chain(&t.rotation).all(|v| v.is_finite()));
            assert!(t.scale.iter().all(|&s| s > 0.0), "{form:?}: scale {:?}", t.scale);
        }
    }
}

#[test]
fn a_shot_is_where_it_is_lifted_and_each_has_ids_of_its_own() {
    let mut all = Vec::new();
    for (n, form) in every_form().into_iter().enumerate() {
        let mut out = Vec::new();
        assert!(build(&mut out, n as u64, &shot(form, (150.0, 90.0), (20.0, 10.0)), (50.0, 30.0)));
        // Within a shot's own reach of its place on the stage (the camera taken off).
        for node in &out {
            let [x, y, z] = node.transform.translation;
            assert!((x - 100.0).abs() < 12.0 && (z - 60.0).abs() < 12.0, "{form:?}: ({x}, {z})");
            assert!((y - LIFT).abs() < 6.0, "{form:?}: height {y}");
        }
        all.extend(out);
    }
    let mut ids: Vec<u64> = all.iter().map(|n| n.id).collect();
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count, "no id twice across shots");
}

/// Where a shape's own +X points in the world.
fn heading(node: &Node) -> [f32; 3] {
    quat_rotate(node.transform.rotation, [1.0, 0.0, 0.0])
}

#[test]
fn an_arrow_points_the_way_it_flies_whichever_way_that_is() {
    for degrees in (0..360).step_by(30) {
        let a = (degrees as f32).to_radians();
        let (vx, vy) = (a.cos() * 60.0, a.sin() * 60.0);
        for form in [ShotForm::Kind(Shot::Arrow), ShotForm::Kind(Shot::Bolt), ShotForm::Kind(Shot::Missile)] {
            let nodes = built(form, (100.0, 80.0), (vx, vy));
            // The shaft is the first shape: a box whose long side is its X.
            let shaft = &nodes[0];
            assert!(matches!(shaft.mesh, Some(MeshRef::Cube)) && shaft.transform.scale[0] > 4.0);
            let h = heading(shaft);
            // The world's z is down the room, the room's y.
            assert!((h[0] - a.cos()).abs() < 1e-4 && (h[2] - a.sin()).abs() < 1e-4 && h[1].abs() < 1e-4, "{form:?} at {degrees}: {h:?}");
            // The head is further along the flight than the shaft's middle.
            let head = nodes.iter().find(|n| matches!(n.mesh, Some(MeshRef::Cone { .. }))).expect("a head");
            let to_head = [
                head.transform.translation[0] - shaft.transform.translation[0],
                head.transform.translation[2] - shaft.transform.translation[2],
            ];
            assert!(to_head[0] * a.cos() + to_head[1] * a.sin() > 2.0, "{form:?} at {degrees}: the head leads");
            // The head's point (the cone's +Y) is on the heading too.
            let point = quat_rotate(head.transform.rotation, [0.0, 1.0, 0.0]);
            assert!((point[0] - a.cos()).abs() < 1e-4 && (point[2] - a.sin()).abs() < 1e-4, "{form:?}: point {point:?}");
        }
    }
}

#[test]
fn a_comets_tail_trails_behind_it() {
    for degrees in (0..360).step_by(45) {
        let a = (degrees as f32).to_radians();
        let nodes = built(ShotForm::Kind(Shot::Wisp), (100.0, 80.0), (a.cos() * 50.0, a.sin() * 50.0));
        // The head orb (the first two shapes) is at the shot's point; the beads are behind it.
        let at = nodes[0].transform.translation;
        for bead in &nodes[2..] {
            let d = [bead.transform.translation[0] - at[0], bead.transform.translation[2] - at[2]];
            assert!(d[0] * a.cos() + d[1] * a.sin() < -3.0, "bead behind at {degrees}");
        }
    }
}

#[test]
fn round_things_are_flattened_in_depth_and_long_things_are_not() {
    let orb = built(ShotForm::Kind(Shot::Orb), (100.0, 80.0), (40.0, 0.0));
    for node in &orb {
        let s = node.transform.scale;
        assert!((s[2] / s[0] - DEPTH).abs() < 1e-5, "a ball's depth is {DEPTH} of its width: {s:?}");
        assert_eq!(node.transform.rotation, QUAT_IDENTITY, "a ball's flattening is the world's");
    }
    // An arrow flying down the room keeps its length on the floor.
    let arrow = built(ShotForm::Kind(Shot::Arrow), (100.0, 80.0), (0.0, 40.0));
    let shaft = &arrow[0];
    let m = shaft.transform.matrix();
    let end = mat_point(&m, [0.5, 0.0, 0.0]);
    let start = mat_point(&m, [-0.5, 0.0, 0.0]);
    let length = ((end[0] - start[0]).powi(2) + (end[2] - start[2]).powi(2)).sqrt();
    assert!((length - 8.0).abs() < 1e-3, "{length}");
}

#[test]
fn a_spinning_shot_turns_with_where_it_is_and_only_with_that() {
    let at = |x: f32| built(ShotForm::Kind(Shot::Blade), (x, 80.0), (60.0, 0.0));
    let (here, again, later) = (at(100.0), at(100.0), at(104.0));
    assert_eq!(here, again, "the same place is the same pose");
    assert_ne!(here[0].transform.rotation, later[0].transform.rotation, "a little further, turned");
    // A whole turn of travel is the same pose again.
    let whole = at(100.0 + TURN_PX);
    for (a, b) in here.iter().zip(&whole) {
        let (qa, qb) = (a.transform.rotation, b.transform.rotation);
        let dot = (0..4).map(|k| qa[k] * qb[k]).sum::<f32>().abs();
        assert!(dot > 0.999, "a turn is a turn: {qa:?} {qb:?}");
    }
    // One that stands still does not fall over: it has a pose.
    assert!(!built(ShotForm::Kind(Shot::Blade), (100.0, 80.0), (0.0, 0.0)).is_empty());
}

#[test]
fn the_eye_looks_at_the_viewer_however_it_flies() {
    let pupil = |v: (f32, f32)| {
        let nodes = built(ShotForm::Kind(Shot::Eye), (100.0, 80.0), v);
        let (ball, dark) = (&nodes[0], nodes.last().unwrap());
        let (a, b) = (ball.transform.translation, dark.transform.translation);
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
    };
    let east = pupil((40.0, 0.0));
    for v in [(0.0, 40.0), (-40.0, 0.0), (0.0, -40.0), (0.0, 0.0)] {
        assert_eq!(pupil(v), east);
    }
    assert!(east[1] > 0.0 && east[2] > 0.0, "up and toward the viewer");
}
