//! Conformance checks for glass's own dialect: the splitter lifts an
//! APC whose body starts `glass;` and nothing else, `hello` answers
//! with the version and ops, each op does exactly what the reference
//! says, replies ride the same rules as the intake's own answers, and
//! the lifecycle rules hold. Each test feeds real byte sequences.

use super::testing::{encode_png, FakeEngine};
use super::*;

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Feed one `glass;` sequence to a fresh intake, and return the intake,
/// the engine and the replies it earned.
fn glass(input: &[u8]) -> (Intake, FakeEngine, Vec<u8>) {
    let mut intake = Intake::new(angel_glass::LAYER_PICTURES);
    intake.set_cell((8, 16));
    let mut engine = FakeEngine::new();
    let mut pieces = Vec::new();
    Splitter::default().split(input, &mut pieces);
    let mut replies = Vec::new();
    for piece in pieces {
        match piece {
            Piece::Text(t) => engine.fed.extend_from_slice(&t),
            Piece::Media(m) => {
                intake.handle(m, &mut engine);
                replies.extend_from_slice(&intake.take_replies());
            }
        }
    }
    (intake, engine, replies)
}

fn apc(body: &str) -> Vec<u8> {
    format!("\x1b_glass;{body}\x1b\\").into_bytes()
}

// ---------------------------------------------------------------- splitter

#[test]
fn the_dialect_is_lifted_and_text_stays_exact() {
    let input = b"before\x1b_glass;hello\x1b\\after";
    let mut pieces = Vec::new();
    Splitter::default().split(input, &mut pieces);
    let mut text = Vec::new();
    let mut media = Vec::new();
    for p in pieces {
        match p {
            Piece::Text(t) => text.extend_from_slice(&t),
            Piece::Media(m) => media.push(m),
        }
    }
    assert_eq!(text, b"beforeafter");
    assert_eq!(media, vec![Media::Glass(b"glass;hello".to_vec())]);
}

#[test]
fn a_bel_terminates_the_dialect_too() {
    let mut pieces = Vec::new();
    Splitter::default().split(b"\x1b_glass;hello\x07mid\x1b_glass;hello\x1b\\", &mut pieces);
    let media: Vec<Media> = pieces.into_iter().filter_map(|p| match p {
        Piece::Media(m) => Some(m),
        _ => None,
    }).collect();
    assert_eq!(media.len(), 2);
    assert_eq!(media[0], Media::Glass(b"glass;hello".to_vec()));
}

#[test]
fn a_g_head_that_is_not_the_dialect_passes_through() {
    let input = b"\x1b_go\x1b\\X";
    let mut pieces = Vec::new();
    Splitter::default().split(input, &mut pieces);
    let mut text = Vec::new();
    for p in pieces {
        if let Piece::Text(t) = p {
            text.extend_from_slice(&t);
        }
    }
    assert_eq!(text, input.to_vec(), "the whole APC stays byte-exact text");
}

#[test]
fn glass_sequences_survive_every_split() {
    let whole = b"a\x1b_glass;layer;id=1,k=glow,c=ff8800,at=0,0,4,2\x1b\\b\x1b_go\x1b\\c";
    let mut want = (Vec::new(), Vec::new());
    {
        let mut pieces = Vec::new();
        Splitter::default().split(whole, &mut pieces);
        for p in pieces {
            match p {
                Piece::Text(t) => want.0.extend_from_slice(&t),
                Piece::Media(m) => want.1.push(m),
            }
        }
    }
    for cut in 1..whole.len() {
        let mut s = Splitter::default();
        let (mut text, mut media) = (Vec::new(), Vec::new());
        for read in [whole[..cut].to_vec(), whole[cut..].to_vec()] {
            let mut pieces = Vec::new();
            s.split(&read, &mut pieces);
            for p in pieces {
                match p {
                    Piece::Text(t) => text.extend_from_slice(&t),
                    Piece::Media(m) => media.push(m),
                }
            }
        }
        assert_eq!(text, want.0, "cut at {cut}");
        assert_eq!(media, want.1, "cut at {cut}");
    }
}

// ---------------------------------------------------------------- hello

#[test]
fn hello_answers_the_version_and_ops() {
    let (_, _, replies) = glass(&apc("hello;r=1"));
    assert_eq!(replies, b"\x1b_glass;r=1;OK;v=3,ops=hello,picture,layer,group,remove,showcase,scene,frame,chart,model,page,progress,asset,tween,play\x1b\\".to_vec());
}

#[test]
fn hello_with_no_token_is_silent() {
    let (_, _, replies) = glass(&apc("hello"));
    assert!(replies.is_empty(), "no r= means no reply");
}

#[test]
fn an_unknown_op_is_an_error_and_never_a_panic() {
    let (_, _, replies) = glass(&apc("wat;r=9"));
    assert_eq!(replies, b"\x1b_glass;r=9;EINVAL:unknown op wat\x1b\\".to_vec());
}

// ---------------------------------------------------------------- layer

#[test]
fn a_layer_covers_every_field() {
    let (intake, _, _) = glass(&apc(
        "layer;id=7;k=glow;c=8844ccff;o=0.5;z=-3;a=1.5;h=8;b=2;add=1;fl=0.25;at=2,1,4,2",
    ));
    let layers = intake.expansion.frame(0, 24, false);
    assert_eq!(layers.len(), 1);
    let l = &layers[0];
    assert_eq!(l.id, u64::from(glass_dialect::ID_BASE + 7));
    assert_eq!(l.z, -3);
    assert_eq!(l.opacity, 0.5);
    assert_eq!(l.angle, 1.5);
    assert_eq!(l.height, 8.0);
    assert_eq!(l.breathe, 2.0);
    assert!(l.additive);
    assert_eq!(l.flicker, 0.25);
    assert_eq!(l.placement, angel_glass::Placement::Cells([2.0, 1.0, 4.0, 2.0]));
    assert_eq!(l.kind, angel_glass::LayerKind::Glow([0x88, 0x44, 0xcc, 0xff].map(|b| f32::from(b as u8) / 255.0)));
}

#[test]
fn rings_and_lines_are_strokes_with_their_width_and_glow() {
    let (intake, _, _) = glass(&[
        apc("layer;id=1;k=ring;c=40b0ffff;lw=2.5;glow=7;add=1;px=100,100,200,200").as_slice(),
        apc("layer;id=2;k=line;c=ff60c0;a=0.5;px=10,20,300,2").as_slice(),
        apc("layer;id=3;k=ring;lw=9999;glow=-1;px=0,0,10,10").as_slice(),
    ].concat());
    let layers = intake.expansion.frame(0, 24, false);
    let strokes: Vec<&angel_glass::Stroke> = layers
        .iter()
        .filter_map(|l| match &l.kind {
            angel_glass::LayerKind::Stroke(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(strokes.len(), 3);
    assert_eq!(strokes[0].shape, angel_glass::StrokeShape::Ring);
    assert_eq!((strokes[0].width, strokes[0].glow), (2.5, 7.0));
    assert!(layers[0].additive);
    assert_eq!(strokes[1].shape, angel_glass::StrokeShape::Line);
    // The defaults: a stroke a pixel and a half wide, with no glow.
    assert_eq!((strokes[1].width, strokes[1].glow), (1.5, 0.0));
    assert_eq!(layers[1].angle, 0.5);
    // Out of range is held to the range, as the other sizes are.
    assert_eq!((strokes[2].width, strokes[2].glow), (angel_glass::MAX_STROKE_PX, 0.0));
}

#[test]
fn every_kind_is_reached() {
    let (intake, _, _) = glass(&[
        apc("picture;id=1;f=24;s=1;v=1;////").as_slice(),
        apc("layer;id=1;k=solid;c=ff0000;at=0,0,1,1").as_slice(),
        apc("layer;id=2;k=glow;c=00ff00;at=0,0,1,1").as_slice(),
        apc("layer;id=3;k=particles;c=ffffff;n=8;at=0,0,10,4").as_slice(),
        apc("layer;id=4;k=prim;shape=3;c=0000ff;at=0,0,3,3").as_slice(),
        apc("layer;id=5;k=picture;p=1;at=0,0,2,2").as_slice(),
        apc("layer;id=6;k=part;p=1;src=0,0,1,1;at=3,0,1,1").as_slice(),
    ].concat());
    let layers = intake.expansion.frame(0, 24, false);
    assert_eq!(layers.len(), 6);
    let kinds: Vec<&str> = layers
        .iter()
        .map(|l| match &l.kind {
            angel_glass::LayerKind::Solid(_) => "solid",
            angel_glass::LayerKind::Picture(_) => "picture",
            angel_glass::LayerKind::PicturePart(_, _) => "part",
            angel_glass::LayerKind::Glow(_) => "glow",
            angel_glass::LayerKind::Particles(_) => "particles",
            angel_glass::LayerKind::Primitive { .. } => "prim",
            angel_glass::LayerKind::Stroke(_) => "stroke",
        })
        .collect();
    assert_eq!(kinds, vec!["solid", "glow", "particles", "prim", "picture", "part"]);
}

#[test]
fn px_pins_to_the_window_and_at_scrolls_with_the_text() {
    let mut intake = Intake::new(angel_glass::LAYER_PICTURES);
    intake.set_cell((8, 16));
    let mut engine = FakeEngine::new();
    intake.handle(Media::Glass(b"glass;layer;id=1;k=solid;c=ff0000;at=0,0,2,8".to_vec()), &mut engine);
    intake.handle(Media::Glass(b"glass;layer;id=2;k=solid;c=00ff00;px=10,20,30,40".to_vec()), &mut engine);
    // The text-anchored layer moves with the view; the pinned one never.
    let top0 = intake.expansion.frame(0, 24, false);
    let top5 = intake.expansion.frame(5, 24, false);
    let find = |layers: &Vec<angel_glass::Layer>, id: u32| {
        layers.iter().find(|l| l.id == u64::from(glass_dialect::ID_BASE + id)).cloned().unwrap()
    };
    assert_eq!(find(&top0, 1).placement, angel_glass::Placement::Cells([0.0, 0.0, 2.0, 8.0]));
    assert_eq!(find(&top5, 1).placement, angel_glass::Placement::Cells([0.0, -5.0, 2.0, 8.0]));
    assert_eq!(find(&top0, 2).placement, angel_glass::Placement::Pixels([10.0, 20.0, 30.0, 40.0]));
    assert_eq!(find(&top5, 2).placement, angel_glass::Placement::Pixels([10.0, 20.0, 30.0, 40.0]));
}

#[test]
fn anchor_window_keeps_its_own_row_wherever_the_view_is() {
    let (mut intake, mut engine, _) = glass(&[]);
    feed(&mut intake, &mut engine, "layer;id=1;k=solid;c=ff0000;anchor=window;at=3,7.5,2,8");
    let record = "layer;id=2;k=glow;c=00ff00;anchor=window;at=4,12.25,6,3";
    feed(&mut intake, &mut engine, &format!("group;g=1;n=1;{}", b64(record.as_bytes())));
    feed(&mut intake, &mut engine, &format!("scene;id=3;at=1,9,10,8;s=64;v=48;anchor=window;{}", cube_scene()));
    let find = |layers: &Vec<angel_glass::Layer>, id: u32| {
        layers.iter().find(|l| l.id == u64::from(glass_dialect::ID_BASE + id)).cloned().unwrap()
    };
    for top in [0, 5, 40] {
        let layers = intake.expansion.frame(top, 24, false);
        assert_eq!(find(&layers, 1).placement, angel_glass::Placement::Cells([3.0, 7.5, 2.0, 8.0]), "top {top}");
        assert_eq!(find(&layers, 2).placement, angel_glass::Placement::Cells([4.0, 12.25, 6.0, 3.0]), "top {top}");
        assert_eq!(find(&layers, 3).placement, angel_glass::Placement::Cells([1.0, 9.0, 10.0, 8.0]), "top {top}");
    }
}

// ---------------------------------------------------------------- picture

#[test]
fn a_chunked_picture_assembles_under_its_first_chunk_keys() {
    let mut intake = Intake::new(angel_glass::LAYER_PICTURES);
    let mut engine = FakeEngine::new();
    intake.handle(
        Media::Glass(format!("glass;picture;id=9;f=24;s=2;v=1;m=1;r=3;{}", b64(b"\xff\x00\x00\x00\xff")).into_bytes()),
        &mut engine,
    );
    assert_eq!(intake.take_replies().len(), 0, "a first chunk holds its reply");
    intake.handle(
        Media::Glass(format!("glass;picture;id=9;m=0;{}", b64(b"\x00\xcc\x00")).into_bytes()),
        &mut engine,
    );
    let replies = intake.take_replies();
    assert_eq!(replies, b"\x1b_glass;r=3;OK;\x1b\\".to_vec());
    let described = intake.describe(View { top: 0, rows: 24, alternate: false });
    assert_eq!(described.pictures.len(), 1);
    assert_eq!(described.pictures[0].width, 2);
}

#[test]
fn a_png_picture_decodes_through_the_shared_loader() {
    let png = encode_png(3, 2);
    let (mut intake, _, _) = glass(&apc(&format!("picture;id=4;f=100;{}", b64(&png))));
    let described = intake.describe(View { top: 0, rows: 24, alternate: false });
    assert_eq!(described.pictures.len(), 1);
    assert_eq!(described.pictures[0].width, 3);
    assert_eq!(described.pictures[0].height, 2);
}

// ---------------------------------------------------------------- group

#[test]
fn a_group_replaces_whole_and_ten_thousand_layers_settle_fast() {
    let records: String = (0..10_000)
        .map(|i| format!("layer;id={};k=solid;c={:06x};at={},{},1,1", i, i % 0xffffff, i % 80, i % 24))
        .collect::<Vec<_>>()
        .join("\n");
    let payload = b64(records.as_bytes());
    let start = std::time::Instant::now();
    let (intake, _, replies) = glass(&apc(&format!("group;g=1;n=10000;r=2;{payload}")));
    let parse = start.elapsed();
    let layers = intake.expansion.frame(0, 24, false);
    let frame = start.elapsed();
    println!("10_000-layer group: parse {parse:?}, with frame {frame:?}");
    assert_eq!(layers.len(), 10_000);
    assert_eq!(replies, b"\x1b_glass;r=2;OK;\x1b\\".to_vec());
    // A second group with the same id replaces, it does not add.
    let one = b64(b"layer;id=1;k=solid;c=000000;at=0,0,1,1");
    let (intake, _, _) = glass(&[
        apc(&format!("group;g=1;n=10000;{payload}")).as_slice(),
        apc(&format!("group;g=1;n=1;{one}")).as_slice(),
    ].concat());
    assert_eq!(intake.expansion.frame(0, 24, false).len(), 1);
}

#[test]
fn a_group_past_the_layer_cap_is_refused() {
    let (_, _, replies) = glass(&apc("group;g=1;n=70000;r=1;dHlwZQ=="));
    assert_eq!(replies, b"\x1b_glass;r=1;EINVAL:n is capped at 65536\x1b\\".to_vec());
}

// ---------------------------------------------------------------- remove

#[test]
fn remove_takes_one_layer_a_range_a_group_or_all() {
    let mk = |id: u32| apc(&format!("layer;id={id};k=solid;c=ff0000;at=0,0,1,1"));
    let group = b64(b"layer;id=50;k=solid;c=00ff00;at=0,0,1,1");
    let mut intake = Intake::new(angel_glass::LAYER_PICTURES);
    let mut engine = FakeEngine::new();
    for m in [mk(1), mk(2), mk(3), mk(4), apc(&format!("group;g=9;n=1;{group}"))] {
        intake.handle(Media::Glass(m[2..m.len() - 2].to_vec()), &mut engine);
    }
    assert_eq!(intake.expansion.frame(0, 24, false).len(), 5);
    intake.handle(Media::Glass(b"glass;remove;id=2".to_vec()), &mut engine);
    assert_eq!(intake.expansion.frame(0, 24, false).len(), 4);
    intake.handle(Media::Glass(b"glass;remove;id=3-4".to_vec()), &mut engine);
    assert_eq!(intake.expansion.frame(0, 24, false).len(), 2, "one solo and the group");
    intake.handle(Media::Glass(b"glass;remove;g=9".to_vec()), &mut engine);
    assert_eq!(intake.expansion.frame(0, 24, false).len(), 1);
    intake.handle(Media::Glass(b"glass;remove;all=1".to_vec()), &mut engine);
    assert_eq!(intake.expansion.frame(0, 24, false).len(), 0);
}

// ---------------------------------------------------------------- showcase

#[test]
fn the_showcase_opens_on_an_uploaded_picture_and_closes() {
    let mut intake = Intake::new(angel_glass::LAYER_PICTURES);
    let mut engine = FakeEngine::new();
    let (_, w, h) = ((), 2u32, 1u32);
    intake.handle(
        Media::Glass(format!("glass;picture;id=5;f=24;s={w};v={h};{}", b64(&[0xff; 8])).into_bytes()),
        &mut engine,
    );
    assert!(intake.expansion.showcase().is_none());
    intake.handle(Media::Glass(b"glass;showcase;id=5".to_vec()), &mut engine);
    assert_eq!(intake.expansion.showcase(), Some(5));
    // The showcase shows the picture's own pixels, kept in the store
    // under the dialect's window id.
    let (width, height, rgba, _) =
        intake.picture_pixels(super::glass_dialect::ID_BASE + 5).expect("the picture is kept");
    assert_eq!((width, height, rgba.len()), (2, 1, 8));
    intake.handle(Media::Glass(b"glass;showcase;close=1".to_vec()), &mut engine);
    assert_eq!(intake.expansion.showcase(), None);
    // A picture that was never uploaded is an error, not a panic.
    intake.handle(Media::Glass(b"glass;showcase;id=99;r=4".to_vec()), &mut engine);
    assert!(intake.take_replies().starts_with(b"\x1b_glass;r=4;EINVAL:"));
}

// ---------------------------------------------------------------- lifecycle

#[test]
fn text_anchored_layers_leave_with_their_text() {
    let mut intake = Intake::new(angel_glass::LAYER_PICTURES);
    let mut engine = FakeEngine::new();
    engine.scrolled = 5;
    intake.handle(Media::Glass(b"glass;layer;id=1;k=solid;c=ff0000;at=0,0,1,1".to_vec()), &mut engine);
    // A pinned layer survives everything but its own remove.
    intake.handle(Media::Glass(b"glass;layer;id=2;k=solid;c=ff0000;px=0,0,1,1".to_vec()), &mut engine);
    // ED 2 on the primary screen: scrolled becomes the new boundary and
    // the anchored layer, above it, goes with the screen.
    intake.expansion.clear_screen(false, 6);
    let layers = intake.expansion.frame(6, 24, false);
    assert_eq!(layers.len(), 1, "the pinned layer stays");
    // ED 3: the saved lines go, and a layer anchored to them.
    intake.handle(Media::Glass(b"glass;layer;id=3;k=solid;c=ff0000;at=0,0,1,1".to_vec()), &mut engine);
    intake.expansion.clear_saved(6);
    assert_eq!(intake.expansion.frame(6, 24, false).len(), 1);
}

#[test]
fn alternate_screen_layers_go_when_it_is_left() {
    let mut intake = Intake::new(angel_glass::LAYER_PICTURES);
    let mut engine = FakeEngine::new();
    engine.alternate = true;
    intake.handle(Media::Glass(b"glass;layer;id=1;k=solid;c=ff0000;at=0,0,1,1".to_vec()), &mut engine);
    engine.alternate = false;
    intake.expansion.leave_alternate();
    assert_eq!(intake.expansion.frame(0, 24, false).len(), 0);
}

#[test]
fn ris_removes_everything_the_dialect_made() {
    let mut intake = Intake::new(angel_glass::LAYER_PICTURES);
    let mut engine = FakeEngine::new();
    intake.handle(Media::Glass(format!("glass;picture;id=1;f=24;s=1;v=1;{}", b64(&[0xff; 4])).into_bytes()), &mut engine);
    intake.handle(Media::Glass(b"glass;layer;id=1;k=solid;c=ff0000;at=0,0,1,1".to_vec()), &mut engine);
    intake.handle(Media::Glass(b"glass;showcase;id=1".to_vec()), &mut engine);
    intake.expansion.reset();
    assert_eq!(intake.expansion.frame(0, 24, false).len(), 0);
    assert_eq!(intake.expansion.showcase(), None);
}

// ---------------------------------------------------------------- errors

#[test]
fn malformed_values_are_errors_never_panics() {
    for body in [
        "layer;r=1",
        "layer;id=1;r=1",
        "layer;id=1;k=wat;at=0,0,1,1;r=1",
        "layer;id=1;k=solid;c=zz;at=0,0,1,1;r=1",
        "layer;id=1;k=solid;at=a,b,c,d;r=1",
        "layer;id=1;k=solid;at=0,0,1,1;o=9;r=1",
        "group;g=1;n=1;r=1;!!!",
        "remove;r=1",
        "showcase;id=1;r=1",
        "picture;r=1",
        "scene;id=1;r=1",
    ] {
        let (intake, _, replies) = glass(&apc(body));
        assert!(
            replies.starts_with(b"\x1b_glass;r=1;"),
            "{body} earned {:?}",
            String::from_utf8_lossy(&replies)
        );
        assert!(intake.expansion.frame(0, 24, false).is_empty());
    }
}

#[test]
fn opacity_out_of_range_and_bad_keys_are_errors() {
    let (_, _, replies) = glass(&apc("layer;id=1;k=solid;at=0,0,1,1;q=nothing;r=1"));
    // Unknown keys are refused, as the reference says.
    assert_eq!(replies, b"\x1b_glass;r=1;EINVAL:unknown key q\x1b\\".to_vec());
}


fn feed(intake: &mut Intake, engine: &mut FakeEngine, body: &str) -> Vec<u8> {
    let mut pieces = Vec::new();
    let bytes = apc(body);
    Splitter::default().split(&bytes, &mut pieces);
    let mut replies = Vec::new();
    for piece in pieces {
        if let Piece::Media(m) = piece {
            intake.handle(m, engine);
            replies.extend_from_slice(&intake.take_replies());
        }
    }
    replies
}

fn cube_scene() -> String {
    b64(br#"{"v":1,"scene":{"nodes":[{"id":1,"mesh":"cube","color":[1,0.4,0.2,1]}]}}"#)
}

#[test]
fn scene_round_trips_and_remove_covers_it() {
    let (mut intake, mut engine, _) = glass(&[]);
    let replies = feed(
        &mut intake,
        &mut engine,
        &format!("scene;id=3;at=1,2,10,8;s=64;v=48;anchor=window;r=1;{}", cube_scene()),
    );
    assert_eq!(replies, b"\x1b_glass;r=1;OK;\x1b\\".to_vec());
    let scene = intake.expansion.scene(3).expect("scene stored");
    assert!(matches!(scene.nodes[0].mesh, Some(angel_glass::scene::MeshRef::Cube)));
    assert_eq!(scene.camera.transform.translation, [0.0, 0.0, 3.0]);
    assert_eq!(intake.expansion.scene_size(3), Some((64, 48)));
    let layers = intake.expansion.frame(0, 24, false);
    assert!(layers.iter().any(|l| matches!(l.kind, angel_glass::LayerKind::Picture(_))));
    let replies = feed(&mut intake, &mut engine, "remove;id=3;r=2");
    assert_eq!(replies, b"\x1b_glass;r=2;OK;\x1b\\".to_vec());
    assert!(intake.expansion.scene(3).is_none());
    assert!(intake.expansion.frame(0, 24, false).is_empty());
}

#[test]
fn scene_placement_and_size_errors_reach_the_program() {
    let cases = [
        ("scene;id=1;r=1", "EINVAL", "needs s="),
        ("scene;id=1;at=0,0,4,4;s=0;v=8;r=1", "EINVAL", "1..="),
        ("scene;id=1;at=0,0,4,4;s=64;v=9000;r=1", "EINVAL", "1..="),
        ("scene;at=0,0,4,4;s=8;v=8;r=1", "EINVAL", "needs id"),
        ("scene;id=1;at=0,0,4,4;s=8;v=8;r=1;!!!!", "EBADF", "not base64"),
        ("scene;id=1;at=0,0,4,4;s=8;v=8;r=1;", "EINVAL", "needs v"),
    ];
    for (body, code, needle) in cases {
        let payload = if body.ends_with(';') { format!("{body}{}", b64(b"{}")) } else { body.to_string() };
        let (_, _, replies) = glass(&apc(&payload));
        let text = String::from_utf8_lossy(&replies);
        assert!(text.contains(code) && text.contains(needle), "{body} -> {text}");
    }
}

#[test]
fn each_showcase_op_keeps_its_store_state() {
    let (mut intake, mut engine, _) = glass(&[]);
    let pixels = [9u8, 8, 7, 255, 1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255];
    let replies = feed(
        &mut intake,
        &mut engine,
        "showcase;s=32;v=16;events=1;r=1",
    );
    assert_eq!(replies, b"\x1b_glass;r=1;OK;\x1b\\".to_vec());
    assert!(intake.expansion.events_on());
    let opened = intake.expansion.showcase_gen();
    let _ = feed(&mut intake, &mut engine, "showcase;s=32;v=16");
    assert_eq!(intake.expansion.showcase_gen(), opened, "the same video request must not reopen");

    let replies = feed(
        &mut intake,
        &mut engine,
        &format!("frame;f=32;s=2;v=2;r=2;{}", b64(&pixels)),
    );
    assert_eq!(replies, b"\x1b_glass;r=2;OK;\x1b\\".to_vec());
    assert!(matches!(
        intake.expansion.pending_frame(),
        Some(glass_dialect::PendingFrame::Pixels { width: 2, height: 2, .. })
    ));
    assert_eq!(intake.expansion.showcase_gen(), opened, "a frame is not a new showcase request");

    let replies = feed(&mut intake, &mut engine, "progress;t=0.25;pause=1;r=3");
    assert_eq!(replies, b"\x1b_glass;r=3;OK;\x1b\\".to_vec());
    assert_eq!(intake.expansion.progress(), Some((Some(0.25), true)));

    let marks = b64(b"mark;x=1;y=2;w=10;h=8;t=hi");
    let _ = feed(&mut intake, &mut engine, &format!("picture;id=3;f=32;s=2;v=2;{}", b64(&pixels)));
    let replies = feed(&mut intake, &mut engine, &format!("chart;p=3;n=1;r=4;{marks}"));
    assert_eq!(replies, b"\x1b_glass;r=4;OK;\x1b\\".to_vec());
    let generation = match intake.expansion.ask() {
        Some(glass_dialect::ShowcaseAsk::Chart { marks, size, picture, generation, .. }) => {
            assert_eq!(marks.len(), 1);
            assert_eq!(marks[0].rect, [1.0, 2.0, 10.0, 8.0]);
            assert_eq!(marks[0].text, vec!["hi".to_string()]);
            assert_eq!(*size, (2, 2), "the picture's size is the chart's size");
            assert_eq!(*picture, 3);
            *generation
        }
        other => panic!("chart ask was {other:?}"),
    };
    let _ = feed(&mut intake, &mut engine, &format!("chart;p=3;n=1;{marks}"));
    match intake.expansion.ask() {
        Some(glass_dialect::ShowcaseAsk::Chart { generation: again, .. }) => {
            assert_eq!(*again, generation, "the same chart request must not reopen");
        }
        other => panic!("chart ask was {other:?}"),
    }
    let missing = feed(&mut intake, &mut engine, "chart;n=1;r=9");
    assert_eq!(missing, b"\x1b_glass;r=9;EINVAL:chart needs p=\x1b\\".to_vec());
    let absent = feed(&mut intake, &mut engine, "chart;p=9;n=0;r=8");
    assert_eq!(absent, b"\x1b_glass;r=8;ENOENT:no such picture\x1b\\".to_vec());

    let replies = feed(
        &mut intake,
        &mut engine,
        &format!("model;s=80;v=40;r=5;{}", cube_scene()),
    );
    assert_eq!(replies, b"\x1b_glass;r=5;OK;\x1b\\".to_vec());
    match intake.expansion.ask() {
        Some(glass_dialect::ShowcaseAsk::Model { nodes, size, .. }) => {
            assert_eq!(nodes.len(), 1);
            assert_eq!(*size, (80, 40));
        }
        other => panic!("model ask was {other:?}"),
    }

    let replies = feed(
        &mut intake,
        &mut engine,
        &format!("page;cols=20;rows=4;r=6;{}", b64(b"one\ntwo")),
    );
    assert_eq!(replies, b"\x1b_glass;r=6;OK;\x1b\\".to_vec());
    match intake.expansion.ask() {
        Some(glass_dialect::ShowcaseAsk::Page { text, cols, rows, .. }) => {
            assert_eq!(text, "one\ntwo");
            assert_eq!((*cols, *rows), (20, 4));
        }
        other => panic!("page ask was {other:?}"),
    }
}

#[test]
fn showcase_events_are_encoded_only_when_opted_in() {
    use glass_dialect::{showcase_event_bytes, ShowcaseNow, ShowcaseSeen};
    let now = ShowcaseNow { seek: None, steps: 1, copies: 2, dismissals: 1, clicks: 3 };
    let mut seen = ShowcaseSeen::default();
    assert!(showcase_event_bytes(false, &mut seen, now).is_empty());
    assert!(!seen.baselined, "an opted-out read must not consume the baseline");
    let first = showcase_event_bytes(true, &mut seen, ShowcaseNow { seek: Some(0.5), ..ShowcaseNow::default() });
    let text = String::from_utf8(first).unwrap();
    assert_eq!(text, "\u{1b}_glass;event;showcase;seek=0.5\u{1b}\\");
    let second = showcase_event_bytes(true, &mut seen, now);
    let text = String::from_utf8(second).unwrap();
    assert_eq!(
        text,
        "\u{1b}_glass;event;showcase;steps=1\u{1b}\\\
         \u{1b}_glass;event;showcase;copies=2\u{1b}\\\
         \u{1b}_glass;event;showcase;dismissals=1\u{1b}\\\
         \u{1b}_glass;event;showcase;clicks=3\u{1b}\\\
         \u{1b}_glass;event;showcase;close=1\u{1b}\\"
    );
    let quiet = showcase_event_bytes(true, &mut seen, now);
    assert!(quiet.is_empty(), "unchanged counters write nothing");
}

#[test]
fn events_opt_in_is_a_showcase_key_and_stays_off_by_default() {
    let (intake, _, _) = glass(&apc("showcase;events=1;r=1"));
    assert!(intake.expansion.events_on());
    let (intake, _, _) = glass(&[]);
    assert!(!intake.expansion.events_on());
}
