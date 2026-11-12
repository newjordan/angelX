//! angel-face: the reader, the track, the mind, and (when the avatar is on
//! this machine) the goddess cut from it.

use angel_face::glb;
use angel_face::mind::ARRIVAL;
use angel_face::model::{ARKIT, Goddess};
use angel_face::{Cue, Drawing, Mind, Track};
use angel_stage::scene::IDENTITY_MATRIX;

/// A goddess with no parts: only her channels, for the mind and tracks.
fn bare() -> Goddess {
    Goddess {
        joints: Vec::new(),
        skin_joints: Vec::new(),
        inverse_bind: Vec::new(),
        parts: Vec::new(),
        channels: ARKIT.iter().map(|s| s.to_string()).collect(),
        ponytail: Vec::new(),
        into_head: IDENTITY_MATRIX,
        eye_centres: [[0.032, 1.65, 0.13], [-0.032, 1.65, 0.13]],
        mouth: [0.0, 1.57, 0.14],
        forward: [0.0, 0.0, 1.0],
        forehead: [0.0, 1.73, 0.1],
        forehead_out: [0.0, 0.8, 0.6],
        arm: None,
        leg: None,
        neck: 0,
        head: 0,
        chest: 0,
        eyes_at: [0.0, 1.65, 0.13],
        head_at: [0.0, 1.56, 0.02],
        neck_at: [0.0, 1.5, 0.0],
    }
}

/// A one-triangle `.glb` whose one morph target is a sparse accessor
/// moving vertex 2 only.
fn sparse_glb() -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let push_f32 = |bin: &mut Vec<u8>, v: &[f32]| {
        v.iter()
            .for_each(|x| bin.extend_from_slice(&x.to_le_bytes()))
    };
    push_f32(&mut bin, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]); // positions 0..36
    for i in [0u16, 1, 2] {
        bin.extend_from_slice(&i.to_le_bytes()); // indices 36..42
    }
    bin.extend_from_slice(&[0, 0]); // pad to 44
    bin.extend_from_slice(&2u16.to_le_bytes()); // sparse index 44..46
    bin.extend_from_slice(&[0, 0]); // pad to 48
    push_f32(&mut bin, &[0.0, 0.0, 0.5]); // sparse value 48..60
    let json = serde_json::json!({
        "asset": {"version": "2.0"},
        "scene": 0, "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "tri", "mesh": 0}],
        "meshes": [{"name": "tri", "extras": {"targetNames": ["jawOpen"]},
            "primitives": [{"attributes": {"POSITION": 0}, "indices": 1, "targets": [{"POSITION": 2}]}]}],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": 36},
            {"buffer": 0, "byteOffset": 36, "byteLength": 6},
            {"buffer": 0, "byteOffset": 44, "byteLength": 2},
            {"buffer": 0, "byteOffset": 48, "byteLength": 12}
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"},
            {"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"},
            {"componentType": 5126, "count": 3, "type": "VEC3",
             "sparse": {"count": 1, "indices": {"bufferView": 2, "componentType": 5123}, "values": {"bufferView": 3}}}
        ]
    });
    let mut text = serde_json::to_vec(&json).unwrap();
    while text.len() % 4 != 0 {
        text.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + text.len() + 8 + bin.len();
    let mut out = Vec::new();
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(&text);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

#[test]
fn a_sparse_morph_target_moves_only_its_vertex() {
    let file = glb::read(&sparse_glb()).expect("reads");
    let prim = &file.meshes[0].primitives[0];
    assert_eq!(prim.positions.len(), 3);
    assert_eq!(prim.indices, vec![0, 1, 2]);
    let target = &prim.targets[0];
    assert_eq!(target.name, "jawOpen");
    assert_eq!(target.deltas.len(), 1);
    assert_eq!(target.deltas[0].0, 2);
    assert_eq!(target.deltas[0].1, [0.0, 0.0, 0.5]);
}

#[test]
fn the_reader_refuses_what_is_not_a_model() {
    assert!(glb::read(b"not a model at all").is_err());
    let mut cut = sparse_glb();
    cut.truncate(40);
    assert!(glb::read(&cut).is_err());
}

fn track_json(frames: &[[f32; 2]]) -> String {
    serde_json::json!({"fps": 10, "names": ["jawOpen", "mouthFrownLeft"], "frames": frames})
        .to_string()
}

#[test]
fn a_track_samples_between_frames_and_ends() {
    let g = bare();
    let t = Track::from_json(&track_json(&[[0.0, 0.0], [1.0, 0.0]]), &g).unwrap();
    let jaw = g.channel("jawOpen").unwrap();
    let (w, loud) = t.sample(0.05, g.channels.len()).unwrap();
    assert!(
        (w[jaw] - 0.5).abs() < 1e-5,
        "halfway between frames: {}",
        w[jaw]
    );
    assert!(loud > 0.5);
    assert!(t.sample(0.25, g.channels.len()).is_none(), "past its end");
    assert!(t.sample(-0.1, g.channels.len()).is_none());
}

#[test]
fn the_actors_rest_is_taken_off() {
    let g = bare();
    let rest = Track::from_json(&track_json(&[[0.0, 0.3], [0.0, 0.3]]), &g).unwrap();
    let line = Track::from_json(&track_json(&[[0.2, 0.25], [0.4, 0.5]]), &g)
        .unwrap()
        .without_rest(&rest);
    let frown = g.channel("mouthFrownLeft").unwrap();
    let (w0, _) = line.sample(0.0, g.channels.len()).unwrap();
    assert_eq!(w0[frown], 0.0, "below her rest is no frown at all");
    let (w1, _) = line.sample(0.1, g.channels.len()).unwrap();
    assert!((w1[frown] - 0.2).abs() < 1e-5);
}

#[test]
fn a_bad_track_is_refused() {
    let g = bare();
    assert!(Track::from_json("{}", &g).is_err());
    assert!(
        Track::from_json(
            r#"{"fps": 30, "names": ["jawOpen"], "frames": [[0.1, 0.2]]}"#,
            &g
        )
        .is_err()
    );
    assert!(
        Track::from_json(r#"{"fps": 0, "names": ["jawOpen"], "frames": [[0.1]]}"#, &g).is_err()
    );
}

/// Run a mind from `from` to `to` at 30 fps, `cue` at each step.
fn run(
    mind: &mut Mind,
    from: f64,
    to: f64,
    mut cue: impl FnMut(&mut Mind, f64),
) -> angel_face::Moment {
    let mut last = None;
    let mut t = from;
    while t <= to {
        cue(mind, t);
        last = Some(mind.moment(t, None));
        t += 1.0 / 30.0;
    }
    last.unwrap()
}

#[test]
fn she_forms_then_is_fully_here_then_leaves() {
    let g = bare();
    let mut mind = Mind::new(&g, [0.0, 0.03], 1);
    assert_eq!(mind.presence(0.0), 0.0);
    mind.cue(Cue::Arrive, 0.0);
    let forming = run(&mut mind, 0.0, 0.5, |_, _| {});
    assert!(
        forming.presence < 0.9 && forming.liquid > 0.5,
        "molten while she forms: {forming:?}"
    );
    let here = run(&mut mind, 0.5, ARRIVAL + 1.0, |_, _| {});
    assert!((here.presence - 1.0).abs() < 1e-3);
    assert!(here.liquid < 0.2);
    mind.cue(Cue::Depart, ARRIVAL + 1.0);
    let gone = run(&mut mind, ARRIVAL + 1.0, ARRIVAL + 4.0, |_, _| {});
    assert!(gone.presence < 1e-3);
    assert!(mind.gone(ARRIVAL + 4.0));
}

#[test]
fn she_reads_what_you_type_with_her_eyes_down_sometimes() {
    let g = bare();
    let down = [
        g.channel("eyeLookDownLeft").unwrap(),
        g.channel("eyeLookDownRight").unwrap(),
    ];
    // Over many seeds, a typing burst makes her look down in some and not all.
    let mut read = 0;
    for seed in 0..40u64 {
        let mut mind = Mind::new(&g, [0.0, 0.03], seed * 7919 + 1);
        mind.cue(Cue::Arrive, 0.0);
        run(&mut mind, 0.0, 5.0, |_, _| {});
        let m = run(&mut mind, 5.0, 7.0, |m, t| m.cue(Cue::Typing, t));
        if m.weights[down[0]] > 0.3 && m.weights[down[1]] > 0.3 && m.head.pitch > 0.1 {
            read += 1;
        }
    }
    assert!(
        (10..40).contains(&read),
        "curious, not dutiful: read in {read} of 40"
    );
}

#[test]
fn she_blinks() {
    let g = bare();
    let blink = g.channel("eyeBlinkLeft").unwrap();
    let mut mind = Mind::new(&g, [0.0, 0.03], 3);
    mind.cue(Cue::Arrive, 0.0);
    run(&mut mind, 0.0, ARRIVAL + 0.5, |_, _| {});
    let mut closed = 0;
    let mut t = ARRIVAL + 0.5;
    while t < ARRIVAL + 20.0 {
        if mind.moment(t, None).weights[blink] > 0.9 {
            closed += 1;
        }
        t += 1.0 / 60.0;
    }
    assert!(closed > 0, "twenty seconds without a blink");
    assert!(
        closed < 60 * 4,
        "her eyes mostly open: {closed} closed frames"
    );
}

#[test]
fn speech_moves_her_mouth_and_lights_her() {
    let g = bare();
    let jaw = g.channel("jawOpen").unwrap();
    let mut mind = Mind::new(&g, [0.0, 0.03], 5);
    mind.cue(Cue::Arrive, 0.0);
    run(&mut mind, 0.0, 6.0, |_, _| {});
    let mut speech = vec![0.0; g.channels.len()];
    speech[jaw] = 0.4;
    let mut m = mind.moment(6.0, Some((&speech, 0.9)));
    for i in 1..10 {
        m = mind.moment(6.0 + i as f64 / 30.0, Some((&speech, 0.9)));
    }
    assert!(m.weights[jaw] >= 0.4);
    assert!(m.voice > 0.5 && m.glow > 1.0);
}

/// The real avatar, when it is here (`ANGEL_FACE_HOME`, else ~/angelface).
fn avatar() -> Option<Vec<u8>> {
    let home = std::env::var_os("ANGEL_FACE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("angelface"))
        })?;
    std::fs::read(home.join("assets/goddess.glb")).ok()
}

#[test]
fn the_goddess_is_cut_from_the_avatar_and_her_jaw_opens() {
    let Some(bytes) = avatar() else {
        eprintln!("no avatar on this machine: skipped");
        return;
    };
    let g = std::sync::Arc::new(Goddess::from_glb(&bytes).unwrap());
    for name in ARKIT {
        assert!(g.channel(name).is_some(), "{name} is a channel");
    }
    let skin = g
        .parts
        .iter()
        .find(|p| p.kind == angel_face::model::PartKind::Skin)
        .unwrap();
    assert_eq!(skin.mesh.groups.len(), 3, "face, lips, the fading neck");
    assert!(skin.mesh.groups[1].1 > 0, "her lips were found");
    let mut w = g.neutral();
    let rest = skin.shaped(&w);
    w[g.channel("jawOpen").unwrap()] = 1.0;
    let open = skin.shaped(&w);
    let moved = rest
        .positions
        .iter()
        .zip(&open.positions)
        .filter(|(a, b)| (a[1] - b[1]).abs() > 0.005)
        .count();
    assert!(moved > 100, "her jaw opens: {moved} vertices moved");
    let mut drawing = Drawing::new(g.clone());
    let mut mind = Mind::new(&g, drawing.viewer(), 9);
    mind.cue(Cue::Arrive, 0.0);
    let m = run(&mut mind, 0.0, 5.0, |_, _| {});
    let meshes = drawing.meshes(&m);
    assert!(meshes.iter().all(|(_, mesh)| mesh.validate().is_ok()));
    assert!(
        drawing.meshes(&m).is_empty(),
        "nothing is sent twice for the same face"
    );
    let scene = drawing.scene(&m, 5.0);
    assert!(
        scene.world_matrices().iter().all(Option::is_some),
        "every node is placed"
    );
    for limb in [
        g.arm.as_ref().expect("her arm"),
        g.leg.as_ref().expect("her leg"),
    ] {
        assert!(
            limb.reach > 0.2 && limb.tip > limb.reach,
            "{:?}: reach {} tip {}",
            limb.kind,
            limb.reach,
            limb.tip
        );
        let long = limb.stretched(limb.tip * 2.5);
        let tip = long
            .positions
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (tip - limb.tip * 2.5).abs() < 1e-3,
            "{:?} stretched to its tip: {tip}",
            limb.kind
        );
        assert!(long.validate().is_ok());
    }
}
