//! Tweens: the easings at known points (cubic-bezier against reference
//! values, springs against their closed form and a numeric integration),
//! timing on a fake clock, replacement that starts where the value stands,
//! `after=` chains, wake-ups, and the `tween` op through the store.

use super::glass_scene::slerp;
use super::glass_tween::*;
use super::gltf_tests::Rig;

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

// ------------------------------------------------------------- easings

#[test]
fn easings_hit_their_known_values() {
    let cases: &[(&str, f64, f64)] = &[
        ("linear", 0.3, 0.3),
        ("quad-in", 0.5, 0.25),
        ("quad-out", 0.5, 0.75),
        ("quad-in-out", 0.25, 0.125),
        ("quad-in-out", 0.75, 0.875),
        ("cubic-in", 0.5, 0.125),
        ("cubic-out", 0.5, 0.875),
        ("cubic-in-out", 0.25, 0.0625),
        ("quart-in", 0.5, 0.0625),
        ("quart-out", 0.5, 0.9375),
        ("quart-in-out", 0.25, 0.03125),
        ("quint-in", 0.5, 0.03125),
        ("quint-out", 0.5, 0.96875),
        ("quint-in-out", 0.25, 0.015625),
        ("sine-in", 0.5, 0.2928932188134524),
        ("sine-out", 0.5, std::f64::consts::FRAC_1_SQRT_2),
        ("sine-in-out", 0.5, 0.5),
        ("sine-in-out", 0.25, 0.1464466094067262),
        ("expo-in", 0.5, 0.03125),
        ("expo-in", 0.0, 0.0),
        ("expo-out", 0.5, 0.96875),
        ("expo-out", 1.0, 1.0),
        ("expo-in-out", 0.25, 0.015625),
        ("circ-in", 0.5, 0.1339745962155614),
        ("circ-out", 0.5, 0.8660254037844386),
        ("circ-in-out", 0.25, 0.0669872981077807),
        ("back-in", 0.5, -0.0876975),
        ("back-out", 0.5, 1.0876975),
        ("back-in-out", 0.25, -0.09968184375),
        ("back-in-out", 0.75, 1.09968184375),
        ("elastic-in", 0.5, -0.015625),
        ("elastic-out", 0.5, 1.015625),
        ("elastic-in-out", 0.25, 0.011969444423734044),
        ("elastic-in-out", 0.75, 0.988030555576266),
        ("bounce-out", 0.5, 0.765625),
        ("bounce-in", 0.5, 0.234375),
        ("bounce-in-out", 0.25, 0.1171875),
        ("bounce-out", 0.2, 0.3025),
    ];
    for (name, u, want) in cases {
        let got = Ease::parse(name).expect(name).at(*u);
        assert!(close(got, *want, 1e-9), "{name}({u}) = {got}, not {want}");
    }
}

#[test]
fn every_easing_starts_at_zero_and_ends_at_one() {
    let curves = ["quad", "cubic", "quart", "quint", "sine", "expo", "circ", "back", "elastic", "bounce"];
    let mut names: Vec<String> = vec!["linear".into(), "cubic-bezier(0.68,-0.6,0.32,1.6)".into(), "spring(170,26)".into()];
    for c in curves {
        for side in ["in", "out", "in-out"] {
            names.push(format!("{c}-{side}"));
        }
    }
    for name in &names {
        let ease = Ease::parse(name).expect(name);
        assert!(close(ease.at(0.0), 0.0, 1e-9), "{name}(0) = {}", ease.at(0.0));
        assert!(close(ease.at(1.0), 1.0, 1e-9), "{name}(1) = {}", ease.at(1.0));
    }
}

#[test]
fn cubic_bezier_matches_reference_values() {
    // Each reference solved by bisection to the double's last bit, apart
    // from the code under test.
    let cases: &[([f64; 4], f64, f64)] = &[
        ([0.25, 0.1, 0.25, 1.0], 0.5, 0.802403387584857),
        ([0.25, 0.1, 0.25, 1.0], 0.25, 0.40851059135539586),
        ([0.42, 0.0, 1.0, 1.0], 0.5, 0.31535681257253945),
        ([0.42, 0.0, 0.58, 1.0], 0.3, 0.18739590670531253),
        ([0.0, 0.0, 0.58, 1.0], 0.5, 0.6846431874274606),
        ([0.68, -0.6, 0.32, 1.6], 0.5, 0.5),
        ([0.68, -0.6, 0.32, 1.6], 0.2, -0.1046120645973008),
    ];
    for (p, x, want) in cases {
        let ease = Ease::parse(&format!("cubic-bezier({},{},{},{})", p[0], p[1], p[2], p[3])).expect("bezier");
        let got = ease.at(*x);
        assert!(close(got, *want, 1e-7), "{p:?} at {x}: {got}, not {want}");
    }
}

#[test]
fn springs_follow_their_closed_form_and_settle() {
    // Under-, critically and over-damped, and with a mass.
    let cases: &[([f64; 3], f64, f64)] = &[
        ([100.0, 10.0, 1.0], 0.1, 0.34029984660829826),
        ([100.0, 10.0, 1.0], 0.3, 1.1243547674084116),
        ([100.0, 20.0, 1.0], 0.2, 0.5939941502901619),
        ([100.0, 40.0, 1.0], 0.2, 0.3696399777219824),
        ([170.0, 26.0, 1.0], 0.15, 0.5829282826301045),
        ([200.0, 10.0, 2.0], 0.25, 1.310836098648911),
    ];
    for ([k, c, m], t, want) in cases {
        let got = spring(*k, *c, *m, *t);
        assert!(close(got, *want, 1e-9), "spring({k},{c},{m}) at {t}: {got}, not {want}");
    }
    // The closed form against the equation itself, stepped finely.
    let (mut x, mut v) = (0.0f64, 0.0f64);
    let dt = 1e-6;
    for _ in 0..300_000 {
        let a = -100.0 * (x - 1.0) - 10.0 * v;
        v += a * dt;
        x += v * dt;
    }
    assert!(close(x, spring(100.0, 10.0, 1.0, 0.3), 1e-4), "{x}");
    // Settled: within the epsilon from its settle time on, and not long
    // before it (the envelope's own bound: ln(1.1547 / 0.001) / 5).
    let settle = spring_settle(100.0, 10.0, 1.0);
    assert!(close(settle, 1.4103192630416055, 1e-6), "{settle}");
    for i in 0..2000 {
        let t = settle + f64::from(i) * 0.005;
        assert!((spring(100.0, 10.0, 1.0, t) - 1.0).abs() < SPRING_EPSILON, "unsettled at {t}");
    }
    let ease = Ease::parse("spring(100,10)").unwrap();
    assert!(close(ease.own_duration_ms().unwrap(), settle * 1000.0, 1e-6));
    assert!(close(ease.at(0.1 / settle), spring(100.0, 10.0, 1.0, 0.1), 1e-9));
    // A spring that never settles in time ends on its target at the cap.
    assert_eq!(spring_settle(10_000.0, 0.001, 1000.0), SPRING_SETTLE_MAX);
}

#[test]
fn easing_names_that_are_not_easings_are_refused() {
    for bad in [
        "", "quad", "quad-sideways", "wobble-in", "cubic-bezier(1.2,0,0,1)", "cubic-bezier(0,0,1)",
        "cubic-bezier(0,0,1,1,1)", "cubic-bezier(a,b,c,d)", "spring(0,10)", "spring(100,0)",
        "spring(100,10,0)", "spring(100)", "spring(1,2,3,4)", "cubic-bezier(0,NaN,1,1)",
    ] {
        assert!(Ease::parse(bad).is_err(), "{bad} was taken");
    }
}

#[test]
fn slerp_takes_the_short_way_at_an_even_speed() {
    let (s, c) = (std::f32::consts::FRAC_PI_4.sin(), std::f32::consts::FRAC_PI_4.cos());
    // Identity to a half turn about y: halfway is a quarter turn.
    let q = slerp([0.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 0.0], 0.5);
    assert!((q[1] - s).abs() < 1e-5 && (q[3] - c).abs() < 1e-5, "{q:?}");
    // -q is q: the way round is the short one, so nothing turns.
    let q = slerp([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, -1.0], 0.5);
    assert!((q[3].abs() - 1.0).abs() < 1e-5, "{q:?}");
    // A quarter of the way along a quarter turn is a sixteenth.
    let q = slerp([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, s, c], 0.25);
    let e = std::f32::consts::PI / 16.0;
    assert!((q[2] - e.sin()).abs() < 1e-5 && (q[3] - e.cos()).abs() < 1e-5, "{q:?}");
}

// ------------------------------------------------------- the engine

/// Property values kept by target and property, standing in for the
/// dialect's layers and scenes.
#[derive(Default)]
struct Board {
    values: Vec<((Target, Prop), Value)>,
    gone: Vec<Target>,
}

impl Board {
    fn with(mut self, target: Target, prop: Prop, value: Value) -> Self {
        self.values.push(((target, prop), value));
        self
    }

    fn get(&self, target: Target, prop: Prop) -> Value {
        self.values.iter().find(|(k, _)| *k == (target, prop)).map(|(_, v)| *v).expect("a value")
    }

    fn f(&self, target: Target, prop: Prop) -> f32 {
        match self.get(target, prop) {
            Value::F(v) => v,
            other => panic!("{other:?}"),
        }
    }
}

impl Targets for Board {
    fn read(&mut self, target: Target, prop: Prop) -> Option<Value> {
        if self.gone.contains(&target) {
            return None;
        }
        self.values.iter().find(|(k, _)| *k == (target, prop)).map(|(_, v)| *v)
    }

    fn write(&mut self, target: Target, prop: Prop, value: Value) -> bool {
        if self.gone.contains(&target) {
            return false;
        }
        match self.values.iter_mut().find(|(k, _)| *k == (target, prop)) {
            Some(slot) => slot.1 = value,
            None => self.values.push(((target, prop), value)),
        }
        true
    }
}

const L: Target = Target::Layer(1);

fn timing(delay_ms: u64, run_ms: f64, repeat: Repeat, yoyo: bool) -> Timing {
    Timing { delay_ms, run_ms, repeat, yoyo }
}

fn fade(to: f32, t: Timing) -> Tween {
    Tween::new(L, vec![Track::new(Prop::Opacity, Value::F(to))], Ease::Linear, t)
}

#[test]
fn delay_repeat_and_yoyo_run_to_the_millisecond_on_a_fake_clock() {
    let mut board = Board::default().with(L, Prop::Opacity, Value::F(0.0));
    let mut tweens = Tweens::default();
    tweens.add(fade(1.0, timing(200, 1000.0, Repeat::Count(2), true))).unwrap();
    assert_eq!(tweens.deadline(1000, 16), Some(1000), "a new tween wants a tick at once");
    assert!(!tweens.tick(1000, &mut board), "the delay writes nothing");
    assert_eq!(tweens.deadline(1000, 16), Some(1200), "it sleeps through its delay");
    tweens.tick(1200, &mut board);
    assert_eq!(board.f(L, Prop::Opacity), 0.0);
    assert_eq!(tweens.deadline(1200, 16), Some(1216), "a display frame on");
    for (now, want) in [(1700, 0.5), (2200, 1.0), (2450, 0.75), (3200, 0.0), (3450, 0.25)] {
        tweens.tick(now, &mut board);
        assert!((board.f(L, Prop::Opacity) - want).abs() < 1e-6, "at {now}: {}", board.f(L, Prop::Opacity));
    }
    assert_eq!(tweens.deadline(4190, 16), Some(4200), "the end, when it is nearer than a frame");
    tweens.tick(4200, &mut board);
    assert_eq!(board.f(L, Prop::Opacity), 1.0, "three runs, there, back and there");
    assert!(tweens.is_empty());
    assert_eq!(tweens.deadline(4200, 16), None, "all still: nothing wakes the window");
    // An even number of yoyo runs ends where it began.
    tweens.add(fade(0.0, timing(0, 100.0, Repeat::Count(1), true))).unwrap();
    tweens.tick(5000, &mut board);
    tweens.tick(5200, &mut board);
    assert_eq!(board.f(L, Prop::Opacity), 1.0);
}

#[test]
fn a_tween_that_replaces_another_starts_where_the_value_stands() {
    let rect = |x: f32| Value::V4([x, 0.0, 10.0, 1.0]);
    let mut board = Board::default().with(L, Prop::Rect, rect(0.0)).with(L, Prop::Opacity, Value::F(1.0));
    let mut tweens = Tweens::default();
    let both = Tween::new(
        L,
        vec![Track::new(Prop::Rect, rect(100.0)), Track::new(Prop::Opacity, Value::F(0.0))],
        Ease::Linear,
        timing(0, 1000.0, Repeat::Count(0), false),
    );
    tweens.add(both).unwrap();
    tweens.tick(0, &mut board);
    tweens.tick(500, &mut board);
    assert_eq!(board.get(L, Prop::Rect), rect(50.0));
    // A new tween of the rectangle only: it takes that property over at
    // once; the fade runs on.
    let back = Tween::new(L, vec![Track::new(Prop::Rect, rect(0.0))], Ease::Linear, timing(0, 1000.0, Repeat::Count(0), false));
    tweens.add(back).unwrap();
    assert_eq!(tweens.len(), 2);
    tweens.tick(600, &mut board);
    assert_eq!(board.get(L, Prop::Rect), rect(50.0), "no jump: it starts from 50");
    assert!((board.f(L, Prop::Opacity) - 0.4).abs() < 1e-6, "the fade carries on");
    tweens.tick(1100, &mut board);
    assert_eq!(board.get(L, Prop::Rect), rect(25.0));
    tweens.tick(1600, &mut board);
    assert_eq!(board.get(L, Prop::Rect), rect(0.0));
    assert!(tweens.is_empty());
    // One that takes every property of the other over leaves it nothing,
    // and it goes.
    tweens.add(fade(0.5, timing(0, 1000.0, Repeat::Forever, false))).unwrap();
    tweens.add(fade(0.0, timing(0, 1000.0, Repeat::Count(0), false))).unwrap();
    assert_eq!(tweens.len(), 1);
}

#[test]
fn chains_start_where_the_last_ended_however_far_the_clock_jumped() {
    let mut board = Board::default().with(L, Prop::Opacity, Value::F(0.0));
    let mut tweens = Tweens::default();
    let mut a = fade(1.0, timing(0, 1000.0, Repeat::Count(0), false));
    a.tid = Some(1);
    let mut b = fade(0.0, timing(200, 1000.0, Repeat::Count(0), false));
    b.tid = Some(2);
    b.after = Some(1);
    let mut c = fade(0.5, timing(0, 1000.0, Repeat::Count(0), false));
    c.after = Some(2);
    tweens.add(a).unwrap();
    tweens.add(b).unwrap();
    tweens.add(c).unwrap();
    assert_eq!(tweens.len(), 3, "a chained tween does not take over the one it waits on");
    tweens.tick(0, &mut board);
    assert_eq!(tweens.deadline(0, 16), Some(16), "only the running one wakes");
    // One jump past the first's end and into the second: the second began
    // at 1000 plus its 200 delay, from where the first ended.
    tweens.tick(1700, &mut board);
    assert!((board.f(L, Prop::Opacity) - 0.5).abs() < 1e-6, "{}", board.f(L, Prop::Opacity));
    // A jump past both: the third began at 2200 from 0.
    tweens.tick(2700, &mut board);
    assert!((board.f(L, Prop::Opacity) - 0.25).abs() < 1e-6, "{}", board.f(L, Prop::Opacity));
    // After a tween that is not there (never was, or already ended): at once.
    let mut d = fade(1.0, timing(0, 100.0, Repeat::Count(0), false));
    d.after = Some(77);
    tweens.add(d).unwrap();
    tweens.tick(5000, &mut board);
    tweens.tick(5100, &mut board);
    assert_eq!(board.f(L, Prop::Opacity), 1.0);
}

#[test]
fn a_target_that_goes_takes_its_tweens_with_it() {
    let mut board = Board::default().with(L, Prop::Opacity, Value::F(0.0));
    let mut tweens = Tweens::default();
    tweens.add(fade(1.0, timing(0, 1000.0, Repeat::Forever, false))).unwrap();
    tweens.tick(0, &mut board);
    board.gone.push(L);
    tweens.tick(100, &mut board);
    assert!(tweens.is_empty());
    assert_eq!(tweens.deadline(100, 16), None);
}

#[test]
fn a_visibility_tween_wakes_only_at_its_end() {
    let node = Target::Node { scene: 1, node: 2 };
    let mut board = Board::default().with(node, Prop::NodeVisible, Value::B(true));
    let mut tweens = Tweens::default();
    tweens.add(Tween::new(node, vec![Track::new(Prop::NodeVisible, Value::B(false))], Ease::Linear, timing(0, 2000.0, Repeat::Count(0), false))).unwrap();
    tweens.tick(0, &mut board);
    assert_eq!(tweens.deadline(0, 16), Some(2000), "no frames while nothing changes");
    tweens.tick(1999, &mut board);
    assert_eq!(board.get(node, Prop::NodeVisible), Value::B(true));
    tweens.tick(2000, &mut board);
    assert_eq!(board.get(node, Prop::NodeVisible), Value::B(false));
}

// ------------------------------------------------------------ the wire

fn layer_at(rig: &Rig, id: u64) -> angel_glass::Layer {
    rig.intake
        .expansion
        .frame(0, 24, false)
        .into_iter()
        .find(|l| l.id & 0xffff_ffff == u64::from(super::glass_dialect::ID_BASE) + id)
        .expect("the layer")
}

fn cells(layer: &angel_glass::Layer) -> [f32; 4] {
    match layer.placement {
        angel_glass::Placement::Cells(c) => c,
        angel_glass::Placement::Pixels(p) => p,
    }
}

#[test]
fn a_layer_tween_moves_it_every_frame_while_the_program_is_silent() {
    let mut rig = Rig::new();
    rig.send("layer;id=1;k=solid;c=ff0000;at=0,0,4,2;anchor=window");
    assert_eq!(rig.send("tween;layer=1;at=10,4,4,2;o=0.5;c=0000ff;dur=1;ease=linear;r=1"), "\x1b_glass;r=1;OK;\x1b\\");
    rig.intake.set_display_frame(7);
    rig.intake.set_animation_clock(1000);
    assert_eq!(rig.intake.animation_deadline(), Some(1000));
    rig.intake.tick_animations(1000);
    rig.intake.tick_animations(1500);
    let l = layer_at(&rig, 1);
    assert_eq!(cells(&l), [5.0, 2.0, 4.0, 2.0]);
    assert!((l.opacity - 0.75).abs() < 1e-6);
    let angel_glass::LayerKind::Solid(c) = l.kind else { panic!("solid") };
    assert!((c[0] - 0.5).abs() < 1e-6 && (c[2] - 0.5).abs() < 1e-6, "{c:?}");
    assert_eq!(rig.intake.animation_deadline(), Some(1507), "one display frame on");
    rig.intake.tick_animations(2000);
    assert_eq!(cells(&layer_at(&rig, 1)), [10.0, 4.0, 4.0, 2.0]);
    assert_eq!(rig.intake.animation_deadline(), None, "done, it sleeps");
    assert_eq!(rig.intake.expansion.tween_count(), 0);
}

#[test]
fn a_text_anchored_layer_moves_down_its_rows_and_still_rides_the_text() {
    let mut rig = Rig::new();
    rig.send("layer;id=2;k=glow;at=2,1,4,1");
    rig.send("tween;layer=2;at=6,3.5,4,1;dur=1");
    rig.intake.tick_animations(0);
    rig.intake.tick_animations(1000);
    assert_eq!(cells(&layer_at(&rig, 2)), [6.0, 3.5, 4.0, 1.0]);
    // Seen two lines further down the text, it is two rows higher.
    let moved = rig.intake.expansion.frame(2, 24, false);
    let l = moved.iter().find(|l| l.id & 0xffff_ffff == u64::from(super::glass_dialect::ID_BASE) + 2).unwrap();
    assert_eq!(cells(l), [6.0, 1.5, 4.0, 1.0]);
}

fn scene_body(id: u32) -> String {
    let json = r#"{"v":1,"scene":{"camera":{"eye":[0,0,3],"target":[0,0,0],"fov":0.8},"nodes":[{"id":1,"mesh":"cube","color":[1,0,0,1],"lit":true}]}}"#;
    format!("scene;id={id};s=32;v=32;px=0,0,32,32;{}", super::gltf_tests::b64(json.as_bytes()))
}

#[test]
fn scene_nodes_and_cameras_tween_through_the_wire() {
    let mut rig = Rig::new();
    rig.send(&scene_body(3));
    let h = std::f32::consts::FRAC_1_SQRT_2;
    assert!(rig.send(&format!("tween;id=3;node=1;rot=0,{h},0,{h};scale=2,2,2;color=0,0,1,0.5;emissive=1;visible=0;dur=1;r=1")).contains("OK"));
    assert!(rig.send("tween;id=3;camera=1;eye=3,0,0;fov=0.4;dur=1;ease=sine-in-out;r=2").contains("OK"));
    rig.intake.tick_animations(0);
    rig.intake.tick_animations(500);
    let scene = rig.intake.expansion.scene(3).unwrap();
    let n = &scene.nodes[0];
    let e = std::f32::consts::FRAC_PI_8;
    assert!((n.transform.rotation[1] - e.sin()).abs() < 1e-5 && (n.transform.rotation[3] - e.cos()).abs() < 1e-5, "slerped halfway: {:?}", n.transform.rotation);
    assert_eq!(n.transform.scale, [1.5, 1.5, 1.5]);
    assert_eq!(n.material.tint, [0.5, 0.0, 0.5, 0.75]);
    assert_eq!(n.material.alpha, angel_glass::scene::AlphaMode::Blend);
    assert!(n.visible, "visibility changes only at the end");
    assert_eq!(scene.camera.transform.translation, [1.5, 0.0, 1.5]);
    rig.intake.tick_animations(1000);
    let scene = rig.intake.expansion.scene(3).unwrap();
    assert!(!scene.nodes[0].visible);
    assert_eq!(scene.camera.transform.translation, [3.0, 0.0, 0.0]);
    let angel_glass::scene::Projection::Perspective { fov_y, .. } = scene.camera.projection else { panic!("perspective") };
    assert!((fov_y - 0.4).abs() < 1e-6);
    // From +x looking at the origin, the camera's own -z is the world's -x.
    let forward = angel_glass::scene::quat_rotate(scene.camera.transform.rotation, [0.0, 0.0, -1.0]);
    assert!((forward[0] + 1.0).abs() < 1e-5 && forward[1].abs() < 1e-5 && forward[2].abs() < 1e-5, "{forward:?}");
}

#[test]
fn setting_a_target_outright_stops_its_tweens() {
    let mut rig = Rig::new();
    rig.send("layer;id=1;k=solid;at=0,0,1,1");
    rig.send("tween;layer=1;o=0;dur=5");
    rig.send(&scene_body(3));
    rig.send("tween;id=3;node=1;at=1,1,1;dur=5");
    rig.send("tween;id=3;camera=1;eye=1,1,3;dur=5");
    rig.send("tween;layer=3;o=0.5;dur=5");
    assert_eq!(rig.intake.expansion.tween_count(), 4);
    rig.send("layer;id=1;k=solid;at=0,0,1,1");
    assert_eq!(rig.intake.expansion.tween_count(), 3);
    rig.send(&scene_body(3));
    assert_eq!(rig.intake.expansion.tween_count(), 0);
    // stop=1 stops them by target or by name; what they moved stays put.
    rig.send("tween;layer=1;o=0;dur=5;tid=9");
    rig.intake.tick_animations(0);
    rig.intake.tick_animations(2500);
    rig.send("tween;tid=9;stop=1");
    assert_eq!(rig.intake.expansion.tween_count(), 0);
    assert!((layer_at(&rig, 1).opacity - 0.5).abs() < 1e-6);
    // A removed layer takes its tweens with it at the next tick.
    rig.send("tween;layer=1;o=1;dur=5");
    rig.send("remove;id=1");
    rig.intake.tick_animations(2600);
    assert_eq!(rig.intake.expansion.tween_count(), 0);
}

#[test]
fn a_spring_sets_its_own_time() {
    let mut rig = Rig::new();
    rig.send("layer;id=1;k=solid;at=0,0,1,1;anchor=window");
    rig.send("tween;layer=1;at=10,0,1,1;ease=spring(100,10);dur=99");
    rig.intake.tick_animations(0);
    rig.intake.tick_animations(300);
    let x = cells(&layer_at(&rig, 1))[0];
    assert!((x - 11.243548).abs() < 1e-3, "past the target at 0.3 s: {x}");
    rig.intake.tick_animations(1411);
    assert_eq!(cells(&layer_at(&rig, 1))[0], 10.0);
    assert_eq!(rig.intake.expansion.tween_count(), 0);
}

#[test]
fn tween_errors_name_themselves() {
    let mut rig = Rig::new();
    rig.send("layer;id=1;k=solid;at=0,0,1,1");
    rig.send("layer;id=4;k=solid;px=0,0,10,10");
    rig.send(&scene_body(3));
    let cases = [
        ("tween;o=1", "EINVAL:a tween needs layer="),
        ("tween;layer=9;o=1", "EINVAL:no layer 9"),
        ("tween;layer=1;id=3;o=1", "EINVAL:a tween names layer="),
        ("tween;layer=1", "EINVAL:a tween needs a property"),
        ("tween;layer=1;rot=0,0,0,1", "EINVAL:rot= is not a property of a layer"),
        ("tween;layer=1;o=2", "EINVAL:o= is outside 0..1"),
        ("tween;layer=1;size=3", "EINVAL:size= is for particles"),
        ("tween;layer=4;at=0,0,1,1", "EINVAL:layer 4 is placed with px="),
        ("tween;layer=1;px=0,0,1,1", "EINVAL:layer 1 is placed with at="),
        ("tween;layer=1;at=1,2,3", "EINVAL:at= is not four finite numbers"),
        ("tween;layer=1;o=1;ease=wobbly", "EINVAL:ease=wobbly is not an easing"),
        ("tween;layer=1;o=1;dur=-1", "EINVAL:dur= must be"),
        ("tween;layer=1;o=1;delay=9999", "EINVAL:delay= must be"),
        ("tween;layer=1;o=1;repeat=often", "EINVAL:repeat= is a count"),
        ("tween;layer=1;o=1;tid=4;after=4", "EINVAL:a tween cannot wait on itself"),
        ("tween;id=7;camera=1;eye=0,0,1", "EINVAL:no scene 7"),
        ("tween;id=3;eye=0,0,1", "EINVAL:a scene tween needs node="),
        ("tween;id=3;node=8;at=0,0,0", "EINVAL:scene 3 has no node 8"),
        ("tween;id=3;node=1;camera=1;at=0,0,0", "EINVAL:a tween names node= or camera=1"),
        ("tween;id=3;node=1;eye=0,0,0", "EINVAL:eye= is not a property of a scene node"),
        ("tween;id=3;node=1;rot=0,0,0,0", "EINVAL:rot= must not be zero"),
        ("tween;id=3;node=1;color=2,0,0,1", "EINVAL:color= components"),
        ("tween;id=3;camera=1;fov=4", "EINVAL:fov= must be in (0, pi)"),
        ("tween;id=3;camera=1;o=1", "EINVAL:o= is not a property of a camera"),
        ("tween;stop=1", "EINVAL:stop=1 needs a target or tid="),
        ("tween;layer=1;o=1;bogus=1", "EINVAL:unknown key bogus"),
    ];
    for (body, want) in cases {
        let reply = rig.send(&format!("{body};r=1"));
        assert!(reply.contains(want), "{body}: {reply}");
    }
    assert_eq!(rig.intake.expansion.tween_count(), 0);
}

#[test]
fn tweens_are_capped() {
    let mut rig = Rig::new();
    rig.send("layer;id=1;k=solid;at=0,0,1,1");
    // Waiting tweens take nothing over on arrival, so these all stay.
    for _ in 0..TWEEN_CAP {
        rig.send("tween;layer=1;o=0.5;after=7");
    }
    assert_eq!(rig.intake.expansion.tween_count(), TWEEN_CAP);
    let reply = rig.send("tween;layer=1;o=0.5;after=7;r=1");
    assert!(reply.contains("ENOMEM:tweens are capped at 4096"), "{reply}");
}
