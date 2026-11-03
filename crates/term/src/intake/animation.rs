//! Conformance checks for the APC graphics dialect's animation: frame
//! transmission (`a=f`), animation control (`a=a`), frame composition
//! (`a=c`), the store's budget, and the waking the window does. Each
//! test names the behaviour it guards, as the intake's own conformance
//! tests do.

use crate::engine::{Engine, HostInfo};
use crate::intake::testing::FakeEngine;
use crate::intake::{AnimState, FramePut, Intake, Media, Splitter, View, FRAME_CAP};
use crate::session::pump;
#[allow(unused_imports)]
use crate::intake::{Delete, Picture as IntakePicture, Placement};
use image::AnimationDecoder;

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn apc(intake: &mut Intake, engine: &mut FakeEngine, s: &str) {
    intake.handle(Media::Apc(s.as_bytes().to_vec()), engine);
}

/// A 2×2 picture, transmitted as the root frame: `a=T,f=32` with RGBA
/// data, kept under id 1 and placed.
fn rig() -> (Intake, FakeEngine) {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    let rgba = [
        0xff, 0x00, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00,
        0xff,
    ];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=T,f=32,s=2,v=2,i=1;{}", b64(&rgba)),
    );
    (intake, engine)
}

/// One frame's pixels as (rgba, gap), 1-based.
fn frame(intake: &Intake, n: usize) -> (&[u8], Option<i64>) {
    intake
        .frame_of(1, n)
        .unwrap_or_else(|| panic!("no frame {n}"))
}

fn assert_px(frame: &[u8], index: usize, rgba: [u8; 4]) {
    assert_eq!(
        &frame[index * 4..index * 4 + 4],
        &rgba,
        "pixel {index}: {:?}",
        &frame[index * 4..index * 4 + 4]
    );
}

// ------------------------------------------------------------ a=f: the canvas

#[test]
fn a_frame_over_a_blank_canvas_places_the_data_at_x_y() {
    // `x=1,y=1,s=1,v=1`: one pixel at (1,1) on a blank (transparent
    // black) canvas. The rest is the picture's own pixels only when
    // `c=` names them; here the canvas starts blank.
    let (mut intake, mut engine) = rig();
    let px = [0x40, 0x30, 0x20, 0xff];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,x=1,y=1,z=100;{}", b64(&px)),
    );
    let (rgba, gap) = frame(&intake, 2);
    assert_eq!(rgba.len(), 2 * 2 * 4, "the frame is the picture's size");
    assert_px(rgba, 0, [0, 0, 0, 0]);
    assert_px(rgba, 1, [0, 0, 0, 0]);
    assert_px(rgba, 2, [0, 0, 0, 0]);
    assert_px(rgba, 3, px);
    assert_eq!(gap, Some(100));
    // Frame 1 is the picture as first transmitted.
    assert_px(frame(&intake, 1).0, 0, [0xff, 0, 0, 0xff]);
}

#[test]
fn a_frame_from_a_base_frame_carries_its_pixels() {
    // `c=1` starts from the root frame's pixels; the data lands on top.
    let (mut intake, mut engine) = rig();
    let px = [0x40, 0x30, 0x20, 0xff];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0;{}", b64(&px)),
    );
    let (rgba, _) = frame(&intake, 2);
    assert_px(rgba, 0, px);
    assert_px(rgba, 1, [0, 0xff, 0, 0xff]);
    assert_px(rgba, 2, [0, 0, 0xff, 0xff]);
    assert_px(rgba, 3, [0xff, 0xff, 0, 0xff]);
}

#[test]
fn a_frame_with_a_background_colour_fills_the_canvas() {
    // `Y=0xff00ff80`: 32-bit RGBA, `0xAABBGGRR`, so translucent green
    // (alpha 0xff, blue 0x00, green 0xff, red 0x80)? No — the packing
    // is `0xAABBGGRR`: alpha 0xff, blue 0x00, green 0xff, red 0xff in
    // this value. The pixels the data does not reach are this colour.
    let (mut intake, mut engine) = rig();
    let px = [0x40, 0x30, 0x20, 0xff];
    apc(
        &mut intake,
        &mut engine,
        &format!(
            "Ga=f,f=32,s=1,v=1,i=1,Y={},x=1,y=0;{}",
            0xff00_ff80u32,
            b64(&px)
        ),
    );
    let (rgba, _) = frame(&intake, 2);
    // R = 0x80, G = 0xff, B = 0x00, A = 0xff.
    assert_px(rgba, 0, [0x80, 0xff, 0x00, 0xff]);
    assert_px(rgba, 1, px);
    assert_px(rgba, 2, [0x80, 0xff, 0x00, 0xff]);
    assert_px(rgba, 3, [0x80, 0xff, 0x00, 0xff]);
}

#[test]
fn a_frame_blends_by_default_and_replaces_with_x_1() {
    // Blending: a half-transparent red over the picture's red gives
    // half-way pixels. Replacement: the data lands whole.
    let (mut intake, mut engine) = rig();
    let half = [0x00, 0x00, 0xff, 0x80];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0;{}", b64(&half)),
    );
    let blended = frame(&intake, 2).0[0..4].to_vec();
    assert_eq!(
        blended,
        vec![0x7f, 0x00, 0x80, 0xff],
        "half blue over red, weighted by the source's alpha"
    );
    // `X=1` replaces.
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0,X=1;{}", b64(&half)),
    );
    let replaced = frame(&intake, 3).0[0..4].to_vec();
    assert_eq!(replaced, vec![0x00, 0x00, 0xff, 0x80]);
}

#[test]
fn an_edit_writes_the_frame_r_names() {
    // `r=2` edits the second frame in place; the frame count stays.
    let (mut intake, mut engine) = rig();
    let px = [0x40, 0x30, 0x20, 0xff];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0;{}", b64(&px)),
    );
    let px2 = [0x11, 0x22, 0x33, 0xff];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,r=2,x=1,y=1;{}", b64(&px2)),
    );
    let (frames, _, _, _) = intake.animation_of(1).unwrap();
    assert_eq!(frames, 2);
    let (rgba, _) = frame(&intake, 2);
    assert_px(rgba, 0, px);
    assert_px(rgba, 3, px2);
}

#[test]
fn frame_1_is_the_picture_as_first_transmitted() {
    let (intake, _) = rig();
    let (rgba, gap) = frame(&intake, 1);
    assert_eq!(
        rgba,
        &[
            0xff, 0, 0, 0xff, 0, 0xff, 0, 0xff, 0, 0, 0xff, 0xff, 0xff, 0xff, 0, 0xff
        ]
    );
    assert_eq!(
        gap, None,
        "the root frame's gap is unset until a transmit says"
    );
}

#[test]
fn a_gapless_frame_is_negative() {
    let (mut intake, mut engine) = rig();
    let px = [9u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0,z=-1;{}", b64(&px)),
    );
    assert_eq!(frame(&intake, 2).1, Some(-1));
}

#[test]
fn frame_zero_and_absent_gap_are_ignored() {
    // `z=0` reads as absent: the frame keeps the default gap.
    let (mut intake, mut engine) = rig();
    let px = [9u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0,z=0;{}", b64(&px)),
    );
    assert_eq!(frame(&intake, 2).1, None);
    // The root frame's own `z=` sets its gap.
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,r=1,x=0,y=0,z=30;{}", b64(&px)),
    );
    assert_eq!(frame(&intake, 1).1, Some(30));
}

#[test]
fn a_frame_without_an_image_answers_enoent() {
    let (mut intake, mut engine) = rig();
    intake.replies.clear();
    let px = [9u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=99,c=1;{}", b64(&px)),
    );
    assert!(
        intake.replies.starts_with(b"\x1b_Gi=99;ENOENT"),
        "{:?}",
        String::from_utf8_lossy(&intake.replies)
    );
}

// ------------------------------------------------------------ a=a: control

#[test]
fn a_equals_a_runs_stops_and_sets_current() {
    let (mut intake, mut engine) = rig();
    for colour in 0..3u8 {
        let px = [colour + 1; 4];
        apc(
            &mut intake,
            &mut engine,
            &format!(
                "Ga=f,f=32,s=1,v=1,i=1,c=1,x={},y={};{}",
                colour % 2,
                colour / 2,
                b64(&px)
            ),
        );
    }
    // Four frames now; run it.
    apc(&mut intake, &mut engine, "Ga=a,i=1,s=3");
    let (frames, current, state, loops) = intake.animation_of(1).unwrap();
    assert_eq!((frames, current), (4, 0));
    assert_eq!(state, AnimState::Running);
    assert_eq!(loops, None, "no `v=` means forever");
    // The root's gap is unset (0), so the second frame is due at once
    // and, at t=50, the third frame's 40ms boundary has also passed.
    intake.set_animation_clock(50);
    assert!(intake.tick_animations(50), "a frame was due");
    let (_, current, _, _) = intake.animation_of(1).unwrap();
    assert_eq!(
        current, 2,
        "the third frame shows: 50ms ran past the 40ms boundary"
    );
    // Stop it.
    apc(&mut intake, &mut engine, "Ga=a,i=1,s=1");
    assert_eq!(intake.animation_of(1).unwrap().2, AnimState::Stopped);
    assert!(
        !intake.tick_animations(1000),
        "a stopped animation wakes nothing"
    );
    // Make a frame current.
    apc(&mut intake, &mut engine, "Ga=a,i=1,c=3");
    assert_eq!(intake.animation_of(1).unwrap().1, 2);
}

#[test]
fn the_loops_off_by_one_rule_plays_the_sequence_then_rests() {
    // `v=2` means one loop past the default: the animation plays the
    // frames twice in all and rests on the first.
    let (mut intake, mut engine) = rig();
    for i in 0..2u8 {
        let px = [i + 1; 4];
        apc(
            &mut intake,
            &mut engine,
            &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x={i},y=0;{}", b64(&px)),
        );
    }
    apc(&mut intake, &mut engine, "Ga=a,i=1,s=3,v=2");
    // Three frames: gaps 0 (root), 40 (default), 40 (default).
    let _ = intake.tick_animations(1);
    // t=0: frame 1 due at 0 → frame 2 at once (gap 0).
    assert_eq!(
        intake.animation_of(1).unwrap().1,
        1,
        "the 0ms root gap moves at once"
    );
    assert_eq!(
        intake.animation_deadline(),
        Some(40),
        "frame 2's default 40ms from the boundary"
    );
    // t=41: frame 2's wait ended at 40 → the third frame.
    intake.set_animation_clock(41);
    assert!(intake.tick_animations(41));
    assert_eq!(intake.animation_of(1).unwrap().1, 2, "the third frame");
    // t=81: the third frame's wait ended at 80 → wrap, loop 1 done, and
    // with `v=2` (one loop past the default) the loops are spent.
    intake.set_animation_clock(81);
    assert!(intake.tick_animations(81));
    let (_, current, _, loops) = intake.animation_of(1).unwrap();
    assert_eq!(current, 0);
    let _ = loops;
    assert_eq!(
        intake.animation_deadline(),
        None,
        "the loops are spent: nothing wakes"
    );
}

#[test]
fn loading_mode_waits_for_more_frames() {
    let (mut intake, mut engine) = rig();
    let px = [5u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0,z=10;{}", b64(&px)),
    );
    apc(&mut intake, &mut engine, "Ga=a,i=1,s=2");
    // The root's 0ms gap: the second frame lands at once.
    assert!(intake.tick_animations(1));
    assert_eq!(intake.animation_of(1).unwrap().1, 1);
    // At the last frame in loading mode, the clock cannot move it.
    intake.set_animation_clock(5000);
    assert!(
        !intake.tick_animations(5000),
        "loading mode holds the last frame"
    );
    // A new frame arrives: the animation carries on from where it stood.
    let px2 = [6u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=2,x=0,y=0,z=10;{}", b64(&px2)),
    );
    assert_eq!(
        intake.animation_of(1).unwrap().1,
        2,
        "the new frame is current"
    );
    // At the last frame in loading mode the clock still cannot move it:
    // the animation waits for the arrival of more frames, not the clock.
    intake.set_animation_clock(5011);
    assert!(!intake.tick_animations(5011), "still held for more frames");
    // Another frame arrives: it becomes current, and its own wait runs.
    let px3 = [7u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=3,x=0,y=0,z=10;{}", b64(&px3)),
    );
    assert_eq!(
        intake.animation_of(1).unwrap().1,
        3,
        "the newest frame is current"
    );
    // `s=3` runs it: the held wait ends and it loops.
    apc(&mut intake, &mut engine, "Ga=a,i=1,s=3");
    assert!(intake.tick_animations(5021), "running again, the wait ends");
}

#[test]
fn r_with_z_changes_one_frames_gap() {
    let (mut intake, mut engine) = rig();
    let px = [9u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0,z=100;{}", b64(&px)),
    );
    apc(&mut intake, &mut engine, "Ga=a,i=1,r=2,z=25");
    assert_eq!(intake.frame_gap(1, 2), Some(Some(25)));
    // `z=0` is ignored.
    apc(&mut intake, &mut engine, "Ga=a,i=1,r=2,z=0");
    assert_eq!(
        intake.frame_gap(1, 2),
        Some(Some(25)),
        "z=0 reads as absent"
    );
}

#[test]
fn a_gapless_frame_is_skipped_over() {
    let (mut intake, mut engine) = rig();
    let px = [9u8; 4];
    // Frame 2 is gapless: it is skipped over when running.
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0,z=-1;{}", b64(&px)),
    );
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=1,y=0,z=50;{}", b64(&px)),
    );
    apc(&mut intake, &mut engine, "Ga=a,i=1,s=3");
    // t=0: root's 0ms gap → frame 2, which is gapless → frame 3.
    assert!(intake.tick_animations(1));
    assert_eq!(
        intake.animation_of(1).unwrap().1,
        2,
        "the gapless frame is skipped"
    );
}

// ------------------------------------------------------------ a=c: composition

#[test]
fn composition_copies_a_rectangle_with_blending() {
    let (mut intake, mut engine) = rig();
    let opaque = [0x00, 0x00, 0xff, 0xff];
    // Frame 2: one opaque blue pixel at (0,0) on a blank canvas.
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,x=0,y=0;{}", b64(&opaque)),
    );
    // Frame 3: the picture's pixels (from frame 1).
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0;{}", b64(&[0u8; 4])),
    );
    // Compose frame 2's (0,0) 1×1 onto frame 3 at (1,0).
    apc(
        &mut intake,
        &mut engine,
        "Ga=c,i=1,r=2,c=3,X=0,Y=0,w=1,h=1,x=1,y=1",
    );
    let (rgba, _) = frame(&intake, 3);
    // The base is the picture's own pixels, so frame 3 is the picture
    // everywhere; the composed pixel replaces the picture's yellow.
    assert_px(rgba, 3, [0x00, 0x00, 0xff, 0xff]);
    assert_px(rgba, 0, [0xff, 0x00, 0x00, 0xff]);
}

#[test]
fn composition_with_c_1_replaces_without_blending() {
    let (mut intake, mut engine) = rig();
    // Frame 2: a half-transparent blue pixel.
    let half = [0x00, 0x00, 0xff, 0x80];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,x=0,y=0;{}", b64(&half)),
    );
    // Frame 3: from frame 1 (the picture).
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0;{}", b64(&[0u8; 4])),
    );
    // Without `C=1`: blended. Half blue over a transparent canvas is
    // [0,0,128,128]; that over the picture's red is [127,0,64,255].
    apc(
        &mut intake,
        &mut engine,
        "Ga=c,i=1,r=2,c=3,X=0,Y=0,w=1,h=1,x=0,y=0",
    );
    assert_px(frame(&intake, 3).0, 0, [0x7f, 0x00, 0x40, 0xff]);
    // With `C=1`: replaced whole.
    apc(
        &mut intake,
        &mut engine,
        "Ga=c,i=1,r=2,c=3,X=0,Y=0,w=1,h=1,x=0,y=0,C=1",
    );
    assert_px(frame(&intake, 3).0, 0, [0x00, 0x00, 0x80, 0x80]);
}

#[test]
fn composition_out_of_bounds_answers_einval() {
    let (mut intake, mut engine) = rig();
    let px = [9u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,x=0,y=0;{}", b64(&px)),
    );
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0;{}", b64(&px)),
    );
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        "Ga=c,i=1,r=2,c=3,X=2,Y=0,w=1,h=1,x=0,y=0",
    );
    assert!(
        intake.replies.starts_with(b"\x1b_Gi=1;EINVAL"),
        "{:?}",
        String::from_utf8_lossy(&intake.replies)
    );
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        "Ga=c,i=1,r=2,c=3,X=0,Y=0,w=1,h=1,x=0,y=2",
    );
    assert!(
        intake.replies.starts_with(b"\x1b_Gi=1;EINVAL"),
        "{:?}",
        String::from_utf8_lossy(&intake.replies)
    );
    // The same frame, overlapping rectangles.
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        "Ga=c,i=1,r=2,c=2,X=0,Y=0,w=1,h=1,x=0,y=0",
    );
    assert!(
        intake.replies.starts_with(b"\x1b_Gi=1;EINVAL"),
        "{:?}",
        String::from_utf8_lossy(&intake.replies)
    );
    // A missing frame answers ENOENT.
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        "Ga=c,i=1,r=9,c=3,X=0,Y=0,w=1,h=1,x=0,y=0",
    );
    assert!(
        intake.replies.starts_with(b"\x1b_Gi=1;ENOENT"),
        "{:?}",
        String::from_utf8_lossy(&intake.replies)
    );
    // A missing image.
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        "Ga=c,i=9,r=1,c=2,X=0,Y=0,w=1,h=1,x=0,y=0",
    );
    assert!(
        intake.replies.starts_with(b"\x1b_Gi=9;ENOENT"),
        "{:?}",
        String::from_utf8_lossy(&intake.replies)
    );
}

// ------------------------------------------------------------ the store

#[test]
fn frames_count_against_the_budget_and_free_with_the_picture() {
    let mut intake = Intake::with_budget(1, 2 * 2 * 4 * 2 + 1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    let rgba = [0xffu8; 16];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=T,f=32,s=2,v=2,i=1;{}", b64(&rgba)),
    );
    // Frame 2: shares nothing (counted), 16 bytes. Frame 3: another 16,
    // over the budget (16 picture + 16 frame = 32 ≤ 33; a third 16 puts
    // it over).
    let px = [1u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,x=0,y=0;{}", b64(&px)),
    );
    assert_eq!(intake.animation_of(1).unwrap().0, 2, "the frame was kept");
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,x=1,y=0;{}", b64(&px)),
    );
    assert_eq!(
        intake.animation_of(1).unwrap().0,
        2,
        "the third frame was refused"
    );
    assert!(
        intake.replies.starts_with(b"\x1b_Gi=1;ENOSPC"),
        "{:?}",
        String::from_utf8_lossy(&intake.replies)
    );
    // Deleting the picture frees every frame.
    apc(&mut intake, &mut engine, "Ga=d,d=I,i=1");
    assert!(intake.animation_of(1).is_none());
}

#[test]
fn deleting_the_frames_keeps_the_picture() {
    let (mut intake, mut engine) = rig();
    let px = [9u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0;{}", b64(&px)),
    );
    apc(&mut intake, &mut engine, "Ga=d,d=f,i=1");
    // A picture no frame was sent for has no animation to delete;
    // one that has is emptied, and the picture itself stays.
    assert!(intake.animation_of(1).is_none(), "the frames are gone");
    assert!(
        intake.pictures.iter().any(|p| p.dialect == 1),
        "the picture stays"
    );
}

#[test]
fn a_frame_larger_than_the_picture_is_refused() {
    let (mut intake, mut engine) = rig();
    let px = [9u8; 3 * 4 * 3];
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=3,v=3,i=1,x=0,y=0;{}", b64(&px)),
    );
    assert!(
        intake.replies.starts_with(b"\x1b_Gi=1;EINVAL"),
        "{:?}",
        String::from_utf8_lossy(&intake.replies)
    );
}

#[test]
fn frame_data_outside_the_picture_is_refused() {
    let (mut intake, mut engine) = rig();
    let px = [9u8; 4];
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,x=5,y=0;{}", b64(&px)),
    );
    assert!(
        intake.replies.starts_with(b"\x1b_Gi=1;EINVAL"),
        "{:?}",
        String::from_utf8_lossy(&intake.replies)
    );
}

// ------------------------------------------------------------ the showing

#[test]
fn the_show_names_the_current_frames_gpu_and_uploads_each_once() {
    let (mut intake, mut engine) = rig();
    let px = [7u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0,z=10;{}", b64(&px)),
    );
    apc(&mut intake, &mut engine, "Ga=a,i=1,s=3");
    // First describe: the picture's own pixels (the root frame) go up
    // once; frame 2 waits until it shows.
    let d1 = intake.describe(View {
        top: 0,
        rows: 24,
        alternate: false,
    });
    assert_eq!(d1.pictures.len(), 1, "the picture's own pixels");
    assert_eq!(d1.placements.len(), 1);
    let shown_gpu = d1.placements[0].gpu;
    // Frame 2 becomes current: its pixels go up once, and the
    // placement names its own id.
    intake.set_animation_clock(10);
    assert!(intake.tick_animations(10));
    let d2 = intake.describe(View {
        top: 0,
        rows: 24,
        alternate: false,
    });
    assert_eq!(d2.pictures.len(), 1, "frame 2's pixels, once");
    assert_eq!(d2.placements.len(), 1);
    assert_ne!(
        d2.placements[0].gpu, shown_gpu,
        "the layer points at the current frame's id"
    );
    // Coming round again uploads nothing: the root frame's pixels were
    // already up, as the picture's own. The wrap's boundary (20) and
    // the root's instant return land on frame 2 again — nothing new.
    intake.set_animation_clock(20);
    assert!(intake.tick_animations(20));
    let d3 = intake.describe(View {
        top: 0,
        rows: 24,
        alternate: false,
    });
    assert_eq!(
        d3.pictures.len(),
        0,
        "a frame come round again is not re-sent"
    );
    assert_eq!(
        d3.placements[0].gpu, d2.placements[0].gpu,
        "frame 2's id again, not re-sent"
    );
}

// ------------------------------------------------------------ a real GIF

#[test]
fn a_real_animated_gif_plays_its_frames_and_gaps() {
    // Encode a real three-frame GIF with the `image` crate's encoder
    // (the crate is already a dependency), send its frames the way
    // `kitten icat` would, and check the sequence and the gaps.
    let frames: Vec<(u8, u32, image::RgbaImage)> = vec![
        (
            0x10,
            80,
            image::RgbaImage::from_pixel(4, 4, image::Rgba([0x10, 0x10, 0x10, 0xff])),
        ),
        (
            0x50,
            120,
            image::RgbaImage::from_pixel(4, 4, image::Rgba([0x50, 0x50, 0x50, 0xff])),
        ),
        (
            0x90,
            0,
            image::RgbaImage::from_pixel(4, 4, image::Rgba([0x90, 0x90, 0x90, 0xff])),
        ),
    ];
    let mut gif = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut gif);
        for (_, delay, image) in &frames {
            encoder
                .encode_frame(image::Frame::from_parts(
                    image.clone(),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(*delay, 1),
                ))
                .unwrap();
        }
    }
    // Decode it back, as the intake's own loader would.
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    // The first frame arrives as the picture itself.
    let first = image::load_from_memory(&gif).unwrap().to_rgba8();
    let (w, h) = first.dimensions();
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=T,f=100,i=7;{}", b64(&gif)),
    );
    assert!(
        intake
            .pictures
            .iter()
            .any(|p| p.dialect == 7 && p.width == w && p.height == h)
    );
    // The frames arrive as a client sends them: the GIF's first frame
    // already is the picture (frame 1), so each later frame follows as
    // an `a=f` composed on the one before it, with the GIF's delay as
    // the gap. The GIF's own decoder hands them and their delays back.
    let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&gif)).unwrap();
    let mut index = 1;
    for item in decoder.into_frames() {
        let frame = item.unwrap();
        if index == 1 {
            // The GIF's first frame is the picture itself, already sent.
            index += 1;
            continue;
        }
        let keys = format!("a=f,f=100,i=7,c={}", index - 1);
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay_ms = numer / denom.max(1);
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(frame.buffer().clone())
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let keys = if numer > 0 {
            format!("{keys},z={delay_ms}")
        } else {
            keys
        };
        apc(&mut intake, &mut engine, &format!("G{keys};{}", b64(&png)));
        index += 1;
    }
    // The picture plus each later GIF frame.
    let (count, _, _, _) = intake.animation_of(7).expect("the animation exists");
    assert_eq!(count, index - 1, "the root frame plus each later GIF frame");
    // The gaps are the delays the GIF recorded: 0 means absent.
    assert_eq!(
        intake.frame_gap(7, 1),
        Some(None),
        "the root frame's gap is unset"
    );
    assert_eq!(
        intake.frame_gap(7, 2),
        Some(Some(120)),
        "the second frame's 120ms"
    );
    assert_eq!(
        intake.frame_gap(7, 3),
        Some(None),
        "a zero delay is an absent gap"
    );
    // Run it: the frames advance by their gaps.
    apc(&mut intake, &mut engine, "Ga=a,i=7,s=3");
    // The root's own gap is unset (it waits nothing), so the second
    // frame is due at once; the second's 120ms carries to the third.
    assert!(intake.tick_animations(1));
    assert_eq!(
        intake.animation_of(7).unwrap().1,
        1,
        "the root's unset gap moves at once"
    );
    intake.set_animation_clock(120);
    assert!(intake.tick_animations(120));
    assert_eq!(intake.animation_of(7).unwrap().1, 2);
    // The third frame's delay was zero: an absent gap, so it defaults
    // to 40ms — the boundary is 160; the clock at 200 crossed it and
    // the root's instant return besides, landing on frame 2.
    intake.set_animation_clock(200);
    assert!(intake.tick_animations(200));
    assert_eq!(
        intake.animation_of(7).unwrap().1,
        1,
        "the wrap and the root's unset gap both landed"
    );
}

// ------------------------------------------------------------ the deadline

#[test]
fn the_deadline_the_session_reports_is_right() {
    let (mut intake, mut engine) = rig();
    let px = [9u8; 4];
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x=0,y=0,z=250;{}", b64(&px)),
    );
    apc(&mut intake, &mut engine, "Ga=a,i=1,s=3");
    // t=0: the root's 0ms gap means the second frame is due at once.
    assert_eq!(intake.animation_deadline(), Some(0));
    assert!(intake.tick_animations(0));
    assert_eq!(
        intake.animation_deadline(),
        Some(250),
        "frame 2's 250ms gap"
    );
    // A stopped animation wakes nothing.
    apc(&mut intake, &mut engine, "Ga=a,i=1,s=1");
    assert_eq!(intake.animation_deadline(), None);
    // Restarting: the gap counts from the restart.
    apc(&mut intake, &mut engine, "Ga=a,i=1,s=3");
    assert_eq!(
        intake.animation_deadline(),
        Some(250),
        "counted from the restart"
    );
}

#[test]
fn falling_behind_skips_frames_rather_than_drifting() {
    let (mut intake, mut engine) = rig();
    for i in 0..3u8 {
        let px = [i + 1; 4];
        apc(
            &mut intake,
            &mut engine,
            &format!("Ga=f,f=32,s=1,v=1,i=1,c=1,x={i},y=0,z=40;{}", b64(&px)),
        );
    }
    apc(&mut intake, &mut engine, "Ga=a,i=1,s=3");
    // The clock runs to t=250 in one jump: the frames the clock ran
    // past are skipped, not held, and the current frame is timed from
    // the last boundary it crossed.
    intake.set_animation_clock(250);
    assert!(intake.tick_animations(250));
    let (_, current, _, _) = intake.animation_of(1).unwrap();
    // Boundaries: 0 (root, unset → at once), 40, 80, 120 (wrap to the
    // root), 120 (the root's unset gap moves on at once), 160, 200,
    // 240 (wrap), 240, 280...
    // t=250 crossed the 240 wrap and the root's instant return: the
    // animation shows the second frame (index 1), and the deadline is
    // the next true boundary.
    assert_eq!(current, 1, "the frames the clock ran past are skipped");
    // And the next deadline is the boundary after, not 250+40.
    assert_eq!(
        intake.animation_deadline(),
        Some(280),
        "the boundary, not the drift"
    );
}

// ------------------------------------------------------- the frame count

fn put_frame(intake: &mut Intake, image: u32, rgba: &[u8]) {
    intake
        .frame_put(FramePut {
            image,
            edit: None,
            base: None,
            background: None,
            replace: true,
            x: 0,
            y: 0,
            data_size: (1, 1),
            rgba: rgba.to_vec(),
            gap: None,
        })
        .expect("a frame under the cap is stored");
}

/// Pictures stay at or under 4096 and frames at or under [`FRAME_CAP`],
/// so the two together stay at or under 8000 — under the window's 8192.
#[test]
fn frames_are_capped_store_wide_and_the_room_comes_back() {
    let started = std::time::Instant::now();
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    let px = [1u8, 2, 3, 4];
    let encoded = b64(&px);
    let pictures = 4u32;
    for id in 1..=pictures {
        apc(
            &mut intake,
            &mut engine,
            &format!("Ga=T,f=32,s=1,v=1,i={id};{encoded}"),
        );
    }
    let each = FRAME_CAP / pictures as usize;
    assert_eq!(FRAME_CAP % pictures as usize, 0, "the cap splits evenly");
    for id in 1..=pictures {
        for _ in 0..each {
            put_frame(&mut intake, id, &px);
        }
    }
    assert_eq!(intake.frame_uploads(), FRAME_CAP);
    let counts: Vec<_> = (1..=pictures)
        .map(|id| intake.animation_of(id).map(|a| a.0))
        .collect();
    let root = intake.frame_of(1, 1).unwrap().0.to_vec();
    let later = intake.frame_of(1, 2).unwrap().0.to_vec();
    let pictures_kept = intake.pictures.len();

    let unchanged = |intake: &Intake| {
        assert_eq!(intake.frame_uploads(), FRAME_CAP);
        assert_eq!(
            (1..=pictures)
                .map(|id| intake.animation_of(id).map(|a| a.0))
                .collect::<Vec<_>>(),
            counts
        );
        assert_eq!(intake.frame_of(1, 1).unwrap().0, root);
        assert_eq!(intake.frame_of(1, 2).unwrap().0, later);
        assert_eq!(intake.pictures.len(), pictures_kept);
    };

    // The next frame is refused. The byte budget would have taken it;
    // the count is what says no, and nothing is stored.
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,x=0,y=0;{encoded}"),
    );
    assert!(
        intake.replies.starts_with(b"\x1b_Gi=1;ENOSPC"),
        "no room is ENOSPC, got {}",
        String::from_utf8_lossy(&intake.replies)
    );
    unchanged(&intake);

    // Composing onto the shared root would mint an upload. Refused,
    // and the root's pixels stay the picture's.
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        "Ga=c,i=1,r=2,c=1,X=0,Y=0,w=1,h=1,x=0,y=0",
    );
    assert!(intake.replies.starts_with(b"\x1b_Gi=1;ENOSPC"));
    unchanged(&intake);

    // Quiet keys: q=2 sends nothing, q=1 still reports the error.
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,q=2,f=32,s=1,v=1,i=1,x=0,y=0;{encoded}"),
    );
    assert!(intake.replies.is_empty(), "q=2 silences the error");
    unchanged(&intake);
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,q=1,f=32,s=1,v=1,i=1,x=0,y=0;{encoded}"),
    );
    assert!(intake.replies.starts_with(b"\x1b_Gi=1;ENOSPC"));
    unchanged(&intake);

    // Editing a frame that already has its own upload does not add one.
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,r=2,x=0,y=0;{encoded}"),
    );
    assert!(intake.replies.starts_with(b"\x1b_Gi=1;OK"));
    assert_eq!(intake.frame_uploads(), FRAME_CAP);
    assert_eq!(intake.animation_of(1).unwrap().0, counts[0].unwrap());

    // `d=f` gives that picture's frames back. A frame is accepted again.
    let freed = each;
    apc(&mut intake, &mut engine, "Ga=d,d=f,i=4");
    assert!(intake.animation_of(4).is_none());
    assert_eq!(intake.frame_uploads(), FRAME_CAP - freed);
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,x=0,y=0;{encoded}"),
    );
    assert!(
        intake.replies.starts_with(b"\x1b_Gi=1;OK"),
        "room came back, got {}",
        String::from_utf8_lossy(&intake.replies)
    );
    assert_eq!(intake.frame_uploads(), FRAME_CAP - freed + 1);
    assert_eq!(intake.animation_of(2).unwrap().0, counts[1].unwrap());

    // Deleting a picture frees its frames too.
    let before = intake.frame_uploads();
    apc(&mut intake, &mut engine, "Ga=d,d=I,i=3");
    assert!(intake.animation_of(3).is_none());
    assert_eq!(intake.frame_uploads(), before - each);

    // RIS frees whatever is left.
    intake.reset();
    assert_eq!(intake.frame_uploads(), 0);
    assert!(intake.animation_of(1).is_none());
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=T,f=32,s=1,v=1,i=1;{encoded}"),
    );
    intake.replies.clear();
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=f,f=32,s=1,v=1,i=1,x=0,y=0;{encoded}"),
    );
    assert!(intake.replies.starts_with(b"\x1b_Gi=1;OK"));
    assert_eq!(intake.frame_uploads(), 1);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "the cap test took {:?}",
        started.elapsed()
    );
}

/// Eviction drops a picture's frames through the same path as deletion,
/// and the count comes back with them.
#[test]
fn eviction_gives_the_frame_room_back() {
    // Picture 1 is 4 bytes; two extra frames are 4 each (12). A second
    // 1x1 picture makes 16, past a budget of 15, so the oldest picture
    // and its frames go. What remains is 4 bytes, and a frame fits.
    let mut intake = Intake::with_budget(1, 15);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    let px = b64(&[1, 2, 3, 4]);
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=T,f=32,s=1,v=1,i=1;{px}"),
    );
    put_frame(&mut intake, 1, &[1, 2, 3, 4]);
    put_frame(&mut intake, 1, &[1, 2, 3, 4]);
    assert_eq!(intake.frame_uploads(), 2);
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=T,f=32,s=1,v=1,i=2;{px}"),
    );
    assert!(
        intake.animation_of(1).is_none(),
        "eviction freed the framed picture"
    );
    assert_eq!(intake.frame_uploads(), 0, "the frames' room came back");
    put_frame(&mut intake, 2, &[9, 9, 9, 9]);
    assert_eq!(intake.frame_uploads(), 1);
}

/// icat's three-frame loop, driven the way the window drives it.
///
/// The reader pumps while the loop is in its wait: touch the clock, note
/// the deadline, pump whatever arrived, then tick the elapsed time. 20 ms
/// steps over 3 seconds must show frames 1, 2, 3, 1… at 500 ms each, and
/// the deadline must be the next due time so the window sleeps until then.
#[test]
fn window_loop_advances_an_icat_animation() {
    let image = 1413138611u32;
    let colours: [[u8; 3]; 3] = [[255, 0, 0], [0, 255, 0], [0, 0, 255]];
    let mut commands = Vec::new();
    commands.push(apc_stream(
        &format!("a=T,q=2,f=24,t=d,s=1,v=1,I={image}"),
        b64(&colours[0]).as_bytes(),
    ));
    commands.push(apc_stream(&format!("a=a,v=1,r=1,I={image},z=500"), b""));
    commands.push(apc_stream(
        &format!("a=f,q=2,f=24,t=d,s=1,v=1,c=1,I={image},z=500"),
        b64(&colours[1]).as_bytes(),
    ));
    commands.push(apc_stream(&format!("a=a,s=2,v=1,r=1,I={image},z=500"), b""));
    commands.push(apc_stream(
        &format!("a=f,q=2,f=24,t=d,s=1,v=1,c=2,I={image},z=500"),
        b64(&colours[2]).as_bytes(),
    ));
    commands.push(apc_stream(&format!("a=a,s=3,v=1,r=1,I={image},z=500"), b""));

    let mut engine = Engine::new(
        24,
        80,
        HostInfo {
            colors: [[0xd0; 3], [0, 0, 0], [0xff; 3]],
            cell_px: (8, 16),
        },
    );
    let mut intake = Intake::new(angel_glass::LAYER_PICTURES);
    intake.set_cell((8, 16));
    let mut splitter = Splitter::default();
    let mut seen: std::collections::HashMap<u32, [u8; 3]> = std::collections::HashMap::new();
    let mut shown = Vec::new();
    // The window is already up. icat's keys arrive during the first wait,
    // between the touch and the tick, as the reader thread pumps them.
    for step in 0..150 {
        let now = step * 20;
        intake.set_animation_clock(now);
        if step == 0 {
            let mut replies = Vec::new();
            for command in &commands {
                replies.extend(feed_pump(&mut engine, &mut intake, &mut splitter, command));
            }
            let id = 1u32 << 30 | 1;
            assert!(
                intake.animation_of(id).is_some() || intake.animation_of(1).is_some(),
                "animation missing after icat keys; replies {}",
                String::from_utf8_lossy(&replies)
            );
        }
        let deadline = intake.animation_deadline();
        if step > 0 {
            assert!(
                deadline.is_some() && deadline.unwrap() > now,
                "step {step} now {now}: deadline {deadline:?} would spin or sleep past the frame"
            );
        }
        let elapsed = now + 20;
        intake.tick_animations(elapsed);
        shown.push(describe_frame(&mut intake, &engine, &mut seen));
    }
    // The reader can pump the whole animation before the window loop's
    // clock starts: spawn happens, icat writes, then the loop touches.
    let mut early = Intake::new(angel_glass::LAYER_PICTURES);
    early.set_cell((8, 16));
    let mut early_engine = Engine::new(
        24,
        80,
        HostInfo {
            colors: [[0xd0; 3], [0, 0, 0], [0xff; 3]],
            cell_px: (8, 16),
        },
    );
    let mut early_split = Splitter::default();
    for command in &commands {
        feed_pump(&mut early_engine, &mut early, &mut early_split, command);
    }
    let mut early_seen = std::collections::HashMap::new();
    let mut early_shown = Vec::new();
    for step in 0..150 {
        let now = step * 20;
        early.set_animation_clock(now);
        let deadline = early.animation_deadline();
        early_shown.push((deadline, describe_frame(&mut early, &early_engine, &mut early_seen)));
        early.tick_animations(now + 20);
    }
    let stuck = early_shown.iter().skip(30).all(|(_, frame)| *frame == Some(3));
    assert!(
        !stuck,
        "pumped before the loop, then touched: {:?}",
        early_shown.iter().step_by(10).collect::<Vec<_>>()
    );
    let expected: Vec<Option<usize>> = (0..150)
        .map(|step| {
            let elapsed = step * 20 + 20;
            Some(((elapsed / 500) % 3) as usize + 1)
        })
        .collect();
    assert_eq!(shown, expected, "describe must cycle 1, 2, 3 at 500 ms");
    // Frames that arrive while loading, after the held gap has elapsed,
    // become current. Starting the run must still cycle from there.
    let mut slow = Intake::new(angel_glass::LAYER_PICTURES);
    slow.set_cell((8, 16));
    let mut slow_engine = Engine::new(
        24,
        80,
        HostInfo {
            colors: [[0xd0; 3], [0, 0, 0], [0xff; 3]],
            cell_px: (8, 16),
        },
    );
    let mut slow_split = Splitter::default();
    for command in &commands[..4] {
        feed_pump(&mut slow_engine, &mut slow, &mut slow_split, command);
    }
    slow.set_animation_clock(0);
    slow.tick_animations(600);
    for command in &commands[4..] {
        feed_pump(&mut slow_engine, &mut slow, &mut slow_split, command);
    }
    let mut slow_seen = std::collections::HashMap::new();
    let mut slow_shown = Vec::new();
    for step in 30..180 {
        let now = step * 20;
        slow.set_animation_clock(now);
        let _deadline = slow.animation_deadline();
        slow.tick_animations(now + 20);
        slow_shown.push(describe_frame(&mut slow, &slow_engine, &mut slow_seen));
    }
    assert!(
        slow_shown.contains(&Some(1))
            && slow_shown.contains(&Some(2))
            && slow_shown.contains(&Some(3)),
        "loading-to-running stayed on one frame: {:?}",
        slow_shown.iter().step_by(10).collect::<Vec<_>>()
    );
}

fn apc_stream(keys: &str, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(keys.len() + data.len() + 4);
    out.extend_from_slice(b"\x1b_G");
    out.extend_from_slice(keys.as_bytes());
    out.push(b';');
    out.extend_from_slice(data);
    out.extend_from_slice(b"\x1b\\");
    out
}

fn feed_pump(engine: &mut Engine, intake: &mut Intake, splitter: &mut Splitter, bytes: &[u8]) -> Vec<u8> {
    let mut pieces = Vec::new();
    splitter.split(bytes, &mut pieces);
    pump(engine, intake, &pieces).replies
}

fn describe_frame(
    intake: &mut Intake,
    engine: &Engine,
    seen: &mut std::collections::HashMap<u32, [u8; 3]>,
) -> Option<usize> {
    let view = View {
        top: engine.top_line(),
        rows: engine.size().0 as u16,
        alternate: engine.alternate_screen(),
    };
    let description = intake.describe(view);
    for upload in &description.pictures {
        if upload.rgba.len() >= 3 {
            seen.insert(upload.gpu, [upload.rgba[0], upload.rgba[1], upload.rgba[2]]);
        }
    }
    let gpu = description.placements.first()?.gpu;
    let colour = seen.get(&gpu).copied()?;
    match colour {
        [255, 0, 0] => Some(1),
        [0, 255, 0] => Some(2),
        [0, 0, 255] => Some(3),
        _ => Some(0),
    }
}

/// icat's animation stream. Transmits ask for quiet (`q=2`); the controls
/// carry no `q`. A success must send nothing, or the OK is echoed into the
/// shell after the viewer has stopped reading. An error still follows `q`.
#[test]
fn icat_animation_controls_are_silent_on_success() {
    let image = 1413138611u32;
    let colours: [[u8; 3]; 3] = [[255, 0, 0], [0, 255, 0], [0, 0, 255]];
    let commands = [
        apc_stream(&format!("a=T,q=2,f=24,t=d,s=1,v=1,I={image}"), b64(&colours[0]).as_bytes()),
        apc_stream(&format!("a=a,v=1,r=1,I={image},z=500"), b""),
        apc_stream(
            &format!("a=f,q=2,f=24,t=d,s=1,v=1,c=1,I={image},z=500"),
            b64(&colours[1]).as_bytes(),
        ),
        apc_stream(&format!("a=a,s=2,v=1,r=1,I={image},z=500"), b""),
        apc_stream(
            &format!("a=f,q=2,f=24,t=d,s=1,v=1,c=2,I={image},z=500"),
            b64(&colours[2]).as_bytes(),
        ),
        apc_stream(&format!("a=a,s=3,v=1,r=1,I={image},z=500"), b""),
    ];
    let mut engine = Engine::new(
        24,
        80,
        HostInfo {
            colors: [[0xd0; 3], [0, 0, 0], [0xff; 3]],
            cell_px: (8, 16),
        },
    );
    let mut intake = Intake::new(angel_glass::LAYER_PICTURES);
    intake.set_cell((8, 16));
    let mut splitter = Splitter::default();
    let mut replies = Vec::new();
    for command in &commands {
        replies.extend(feed_pump(&mut engine, &mut intake, &mut splitter, command));
    }
    assert!(
        replies.is_empty(),
        "icat waits for nothing here; got {}",
        String::from_utf8_lossy(&replies)
    );

    // A missing image still answers, and `q` still applies.
    let missing = apc_stream("a=a,s=3,i=9", b"");
    let err = feed_pump(&mut engine, &mut intake, &mut splitter, &missing);
    assert!(
        String::from_utf8_lossy(&err).contains("ENOENT"),
        "an error is still owed: {}",
        String::from_utf8_lossy(&err)
    );
    let quiet = apc_stream("a=a,s=3,q=2,i=9", b"");
    assert!(
        feed_pump(&mut engine, &mut intake, &mut splitter, &quiet).is_empty(),
        "q=2 hides the error"
    );

    // `a=c` answers only on failure, same as the specification. `a=f`
    // answers like a transmit.
    let compose = apc_stream("a=c,I=1413138611,c=1,r=2,w=1,h=1,x=0,y=0,X=0,Y=0", b"");
    assert!(
        feed_pump(&mut engine, &mut intake, &mut splitter, &compose).is_empty(),
        "a successful composition sends nothing"
    );
    let bad = apc_stream("a=c,I=1413138611,c=9,r=2,w=1,h=1,x=0,y=0,X=0,Y=0", b"");
    let err = feed_pump(&mut engine, &mut intake, &mut splitter, &bad);
    assert!(
        String::from_utf8_lossy(&err).contains("ENOENT"),
        "a composition error is still owed: {}",
        String::from_utf8_lossy(&err)
    );
    let frame = apc_stream(
        &format!("a=f,f=24,t=d,s=1,v=1,c=1,I={image},z=40"),
        b64(&[9, 9, 9]).as_bytes(),
    );
    let ok = feed_pump(&mut engine, &mut intake, &mut splitter, &frame);
    assert!(
        String::from_utf8_lossy(&ok).contains("OK"),
        "a=f answers like a transmit: {}",
        String::from_utf8_lossy(&ok)
    );
}
