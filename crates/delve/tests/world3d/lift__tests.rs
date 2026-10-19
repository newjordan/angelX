//! The lift's three moves are pure functions of time: each is exactly flat at
//! 0 and exactly lit at its end, has no jump between, never leaves its range
//! or stops being a number, and rises as it should (a stone climbs and locks,
//! flames catch last, the sun comes down, the tilt lands); and `apply` makes
//! a real lit room flat at the start, leaves it untouched at the end, and
//! never hands the GPU a look or a node it cannot draw.

use super::*;
use crate::rules::{Pack, Rng, RoomKind, layout};
use crate::world3d::room::{Moment, RoomWorld};
use crate::world3d::rooms_lit::{self, flame_nodes};
use crate::world3d::rooms_lit_look::{Activity, Tone};
use angel_stage::scene::{MeshData, Props};
use std::sync::Arc;

/// A spread of places: near and far from the source, every seed, low things
/// and walls.
fn places() -> Vec<Place> {
    let mut out = Vec::new();
    for order in [0.0, 0.2, 0.5, 0.8, 1.0] {
        for seed in [0.0, 0.3, 0.7, 1.0] {
            for tall in [0.0, 6.0, 24.0, 48.0, 120.0] {
                out.push(Place { order, seed, tall });
            }
        }
    }
    out
}

/// Times from before 0 to past `end`, a thousandth of a second apart.
fn sweep(end: f32) -> impl Iterator<Item = f32> {
    (-200..=((end + 0.5) * 1000.0) as i32).map(|i| i as f32 / 1000.0)
}

/// The most any number of a move may change in a thousandth of a second (the
/// quickest is about 12 a second).
const STEP: f32 = 0.02;

#[test]
fn every_move_is_flat_at_0_and_lit_at_its_end() {
    for place in places() {
        for t in [0.0, -1.0, f32::NAN, f32::NEG_INFINITY, -0.0] {
            assert_eq!(prop(t, place), Prop::CARD, "{t}");
            assert_eq!(pool(t, place), Pool::OUT, "{t}");
        }
        for t in [PROP_DURATION, PROP_DURATION + 1.0, 1.0e9, f32::INFINITY] {
            assert_eq!(prop(t, place), Prop::SOLID, "{t}");
        }
        for t in [FLOOR_DURATION, FLOOR_DURATION + 1.0, f32::INFINITY] {
            assert_eq!(pool(t, place), Pool::LIT, "{t}");
        }
    }
    for t in [0.0, -1.0, f32::NAN, f32::NEG_INFINITY] {
        assert_eq!(floor(t), Floor::FLAT, "{t}");
        assert_eq!(tip(t), Tip::FLAT, "{t}");
    }
    for t in [FLOOR_DURATION, FLOOR_DURATION + 1.0, f32::INFINITY] {
        assert_eq!(floor(t), Floor::LIT, "{t}");
    }
    for t in [TIP_DURATION, TIP_DURATION + 1.0, f32::INFINITY] {
        assert_eq!(tip(t), Tip::TIPPED, "{t}");
    }
    // The card is a card: none of its way up, no pool, a spark.
    assert!(Prop::CARD.height == 0.0 && Prop::CARD.flame < 0.1 && Prop::CARD.lamp.strength == 0.0);
    // The whole climb starts with all three flat and ends with all three done.
    let start = climb(0.0);
    assert!(start.floor <= 0.0 && start.prop < 0.0 && start.tip < 0.0);
    let end = climb(DURATION);
    assert!(end.floor >= FLOOR_DURATION && end.prop >= PROP_DURATION && end.tip >= TIP_DURATION, "{end:?}");
    assert!(climb(DURATION - 0.01).prop < PROP_DURATION + 0.01, "the climb ends as its last move does");
    assert_eq!(DURATION, PROP_AT + PROP_DURATION, "the props are the last to finish");
}

#[test]
fn a_move_never_jumps() {
    let mut worst = 0.0f32;
    let mut step = |a: f32, b: f32, what: &str, t: f32| {
        worst = worst.max((a - b).abs());
        assert!((a - b).abs() <= STEP, "{what} jumps {} at {t}", (a - b).abs());
    };
    for place in places() {
        for t in sweep(PROP_DURATION) {
            let (a, b) = (prop(t, place), prop(t + 0.001, place));
            step(a.height, b.height, "height", t);
            step(a.flame, b.flame, "flame", t);
            step(a.lamp.strength, b.lamp.strength, "pool", t);
            step(a.lamp.reach, b.lamp.reach, "reach", t);
        }
        for t in sweep(FLOOR_DURATION) {
            let (a, b) = (pool(t, place), pool(t + 0.001, place));
            step(a.strength, b.strength, "bloom", t);
            step(a.reach, b.reach, "bloom reach", t);
        }
    }
    for t in sweep(FLOOR_DURATION) {
        let (a, b) = (floor(t), floor(t + 0.001));
        step(a.relief, b.relief, "relief", t);
        step(a.sun, b.sun, "sun", t);
        step(a.key, b.key, "key", t);
    }
    for t in sweep(TIP_DURATION) {
        let (a, b) = (tip(t), tip(t + 0.001));
        step(a.tilt, b.tilt, "tilt", t);
        step(a.catch, b.catch, "catch", t);
    }
    assert!(worst > 0.003, "the sweep saw the moves ({worst})");
}

#[test]
fn every_number_is_finite_and_in_its_range() {
    let past = 1.0e-4;
    for place in places() {
        for t in sweep(PROP_DURATION) {
            let p = prop(t, place);
            assert!((0.0..=1.0 + LOCK_OVER + past).contains(&p.height), "height {} at {t}", p.height);
            assert!((SPARK..=1.0 + FLARE + past).contains(&p.flame), "flame {} at {t}", p.flame);
            assert!((0.0..=1.0 + BLOOM_FLARE + past).contains(&p.lamp.strength), "pool {} at {t}", p.lamp.strength);
            assert!((POOL_FROM - past..=1.0 + BLOOM_REACH + past).contains(&p.lamp.reach), "reach {} at {t}", p.lamp.reach);
        }
        for t in sweep(FLOOR_DURATION) {
            let p = pool(t, place);
            assert!((0.0..=1.0 + BLOOM_FLARE + past).contains(&p.strength) && p.reach > 0.0 && p.reach.is_finite(), "{p:?} at {t}");
        }
    }
    for t in sweep(FLOOR_DURATION) {
        let f = floor(t);
        assert!((0.0..=1.0 + RELIEF_OVER + past).contains(&f.relief), "relief {} at {t}", f.relief);
        assert!((0.0..=1.0).contains(&f.sun) && (FLAT_KEY..=1.0).contains(&f.key), "{f:?} at {t}");
    }
    for t in sweep(TIP_DURATION) {
        let k = tip(t);
        assert!((0.0..=1.0 + TIP_OVER + past).contains(&k.tilt), "tilt {} at {t}", k.tilt);
        assert!((1.0..=1.0 + CATCH + past).contains(&k.catch), "catch {} at {t}", k.catch);
    }
}

#[test]
fn a_stone_climbs_then_locks_past_its_height_and_settles() {
    for place in places() {
        let weight = (place.tall / TALL).clamp(0.0, 1.0);
        let rise = RISE_LOW + (RISE_TALL - RISE_LOW) * weight;
        let start = PROP_SPREAD * place.order + PROP_JITTER * place.seed;
        let mut last = 0.0;
        for t in sweep(PROP_DURATION) {
            let h = prop(t, place).height;
            if t <= start + rise {
                assert!(h >= last - 1.0e-6, "the climb only rises ({h} after {last} at {t})");
            } else {
                assert!(h >= 1.0 - 1.0e-5, "after its climb it is up ({h} at {t})");
            }
            last = h;
        }
        // Not yet started: a card. Done: exactly up, past it in between.
        assert_eq!(prop(start - 0.01, place).height, 0.0);
        let over = (0..=100).map(|i| prop(start + rise + LOCK_TIME * i as f32 / 100.0, place).height).fold(0.0, f32::max);
        assert!(over > 1.0 + LOCK_OVER * 0.4 && over <= 1.0 + LOCK_OVER, "the lock goes past: {over}");
        assert!((prop(start + rise + LOCK_TIME + 0.01, place).height - 1.0).abs() < 1.0e-5);
    }
}

#[test]
fn a_wall_climbs_slower_than_a_low_stone_and_the_far_after_the_near() {
    let at = |order, tall| Place { order, seed: 0.5, tall };
    // Half way up the low stone's climb, the wall is behind it.
    let t = PROP_JITTER * 0.5 + RISE_LOW * 0.5;
    assert!(prop(t, at(0.0, 0.0)).height > prop(t, at(0.0, TALL)).height + 0.05);
    // And what is near the source is up, and burning, before what is far.
    let first = |place: Place, reached: &dyn Fn(Prop) -> bool| sweep(PROP_DURATION).find(|&t| reached(prop(t, place))).expect("it gets there");
    for tall in [0.0, 24.0, 48.0] {
        let (near, far) = (at(0.1, tall), at(0.9, tall));
        assert!(first(near, &|p| p.height >= 1.0) < first(far, &|p| p.height >= 1.0), "tall {tall}");
        assert!(first(near, &|p| p.flame >= 1.0) < first(far, &|p| p.flame >= 1.0), "tall {tall}");
        assert!(first(near, &|p| p.lamp.strength >= 1.0) < first(far, &|p| p.lamp.strength >= 1.0), "tall {tall}");
    }
}

#[test]
fn flames_and_lamps_catch_only_after_every_stone_has_seated() {
    for place in places() {
        for t in sweep(IGNITE_AT).take_while(|&t| t < IGNITE_AT) {
            let p = prop(t, place);
            assert_eq!(p.flame, SPARK, "a spark until {IGNITE_AT} (at {t})");
            assert_eq!(p.lamp, Pool::OUT, "no pool until then (at {t})");
        }
        // Every stone is up and still by then.
        assert!((prop(IGNITE_AT, place).height - 1.0).abs() < 1.0e-5);
        // A flame flares past its size on the way, then settles; its pool
        // follows it.
        let catch = IGNITE_AT + IGNITE_SPREAD * place.order + IGNITE_JITTER * place.seed;
        let flare = (0..=100).map(|i| prop(catch + IGNITE_TIME * i as f32 / 100.0, place).flame).fold(0.0, f32::max);
        assert!(flare > 1.0 + FLARE * 0.4, "a flame flares: {flare}");
        assert_eq!(prop(catch + POOL_LAG * 0.5, place).lamp.strength, 0.0, "the pool is a beat behind");
        assert!(prop(catch + POOL_LAG + BLOOM_TIME * 0.5, place).lamp.strength > 0.5);
        assert!(prop(catch + POOL_LAG + BLOOM_TIME * 0.7, place).lamp.strength > 1.1, "and it flares");
    }
}

#[test]
fn the_floors_pools_bloom_after_the_sun_is_down() {
    let place = Place { order: 0.0, seed: 0.5, tall: 0.0 };
    for t in sweep(POOL_AT).take_while(|&t| t <= POOL_AT) {
        assert_eq!(pool(t, place), Pool::OUT, "{t}");
    }
    assert!(POOL_AT >= SUN_AT + SUN_TIME - 0.3, "the sun is nearly down as they come");
    assert!(pool(POOL_AT + BLOOM_TIME * 0.5, place).strength > 0.5);
    let far = Place { order: 1.0, ..place };
    assert!(pool(POOL_AT + BLOOM_TIME * 0.5, far).strength < pool(POOL_AT + BLOOM_TIME * 0.5, place).strength, "door to heart");
    // A pool flares past its light and its reach, and draws back.
    let peak = (0..=100).map(|i| pool(POOL_AT + BLOOM_TIME * i as f32 / 100.0, place)).fold(Pool::OUT, |a, p| Pool { strength: a.strength.max(p.strength), reach: a.reach.max(p.reach) });
    assert!(peak.strength > 1.1 && peak.reach > 1.02, "{peak:?}");
}

#[test]
fn the_ground_rises_the_sun_comes_down_and_the_room_lands() {
    let (mut relief, mut sun, mut key, mut tilt) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for t in sweep(FLOOR_DURATION) {
        let f = floor(t);
        if t <= RELIEF_TIME {
            assert!(f.relief >= relief - 1.0e-6, "relief only rises to its height ({t})");
            relief = f.relief;
        } else {
            assert!(f.relief >= 1.0 - 1.0e-5, "and stays up ({} at {t})", f.relief);
        }
        assert!(f.sun >= sun - 1.0e-6 && f.key >= key - 1.0e-6, "the sun only comes down, the light only on ({t})");
        (sun, key) = (f.sun, f.key);
    }
    assert!(floor(RELIEF_TIME + RELIEF_SETTLE * 0.3).relief > 1.0 + RELIEF_OVER * 0.4, "the relief locks past its height");
    for t in sweep(TIP_DURATION) {
        let k = tip(t);
        if t <= TIP_TIME {
            assert!(k.tilt >= tilt - 1.0e-6, "the room only tips ({t})");
            assert_eq!(k.catch, 1.0, "the lamps catch at the landing, not before ({t})");
            tilt = k.tilt;
        } else {
            assert!(k.tilt >= 1.0 - 1.0e-5, "and stays down ({} at {t})", k.tilt);
        }
    }
    assert!(tip(TIP_TIME + TIP_SETTLE * 0.3).tilt > 1.0 + TIP_OVER * 0.4, "the room lands past its tilt");
    assert!(tip(TIP_TIME + TIP_SETTLE * 0.3).catch > 1.0 + CATCH * 0.4, "and the lamps flare");
    // The tip slow in: a slab lifted at an edge.
    assert!(tip(TIP_TIME * 0.25).tilt < 0.1);
}

#[test]
fn the_sun_comes_down_from_overhead_and_its_shadows_grow_from_nothing() {
    let key = ([-0.70, 0.36, 0.50], 1.2);
    let sun_at = |progress: f32, k: f32| {
        let mut sun = Sun { toward: key.0, strength: key.1, ambient: 0.1, shadow: None, rise: 0.0 };
        lower(&mut sun, progress, k);
        sun
    };
    let own = Sun { toward: key.0, strength: key.1, ambient: 0.1, shadow: None, rise: 0.0 };
    // Down: its own, exactly the way it was made, but for rounding.
    let down = sun_at(1.0, 1.0);
    for k in 0..3 {
        assert!((down.toward_unit()[k] - own.toward_unit()[k]).abs() < 1.0e-5, "{down:?}");
    }
    assert!((down.strength - own.strength).abs() < 1.0e-5);
    // Overhead: nothing's shadow has any length.
    let up = sun_at(0.0, 1.0);
    assert_eq!(up.toward, [0.0, 1.0, 0.0]);
    // A shadow's length is its height's cotangent of the sun's height: 0 up
    // there, growing the whole way down.
    let shadow = |p: f32| {
        let t = sun_at(p, 1.0).toward_unit();
        t[0].hypot(t[2]) / t[1]
    };
    assert_eq!(shadow(0.0), 0.0);
    let mut last = 0.0;
    for i in 1..=100 {
        let p = i as f32 / 100.0;
        let s = shadow(p);
        assert!(s > last, "a shadow only grows ({s} after {last})");
        assert!((s - shadow(1.0) * p).abs() < 1.0e-3 * shadow(1.0), "and grows evenly with the sun's progress ({s} at {p})");
        last = s;
    }
    // The floor keeps its brightness all the way down; a wall's face grows.
    let floor_light = |p: f32| {
        let s = sun_at(p, 1.0);
        s.strength * s.toward_unit()[1]
    };
    for i in 0..=100 {
        let p = i as f32 / 100.0;
        assert!((floor_light(p) - floor_light(1.0)).abs() < 1.0e-4, "the floor holds its tone ({p})");
    }
    let face = |p: f32| {
        let s = sun_at(p, 1.0);
        let t = s.toward_unit();
        s.strength * t[0].hypot(t[2])
    };
    assert_eq!(face(0.0), 0.0);
    assert!(face(0.5) > 0.0 && face(1.0) > face(0.5));
    // With less of the key on, the floor has less of it.
    assert!(sun_at(0.0, FLAT_KEY).strength < sun_at(0.0, 1.0).strength);
}

#[test]
fn the_ripple_runs_from_its_source_and_its_seed_is_smooth() {
    let from = Ripple::centre(544.0, 320.0);
    assert_eq!(from.order(272.0, 160.0), 0.0);
    assert_eq!(from.order(0.0, 0.0), 1.0);
    assert!(from.order(272.0 + 100.0, 160.0) < from.order(272.0 + 200.0, 160.0));
    let doors = Ripple::doors(&[[272.0, 8.0]], 544.0, 320.0);
    assert_eq!(doors.order(272.0, 8.0), 0.0);
    assert!(doors.order(272.0, 100.0) < doors.order(272.0, 300.0), "from the door, the far end is last");
    assert_eq!(doors.order(0.0, 320.0), 1.0, "the farthest corner");
    assert_eq!(Ripple::doors(&[], 544.0, 320.0), from, "no door, the middle");
    for x in 0..544 {
        for z in (0..320).step_by(8) {
            let (a, b) = (seed(x as f32, z as f32), seed(x as f32 + 1.0, z as f32));
            assert!((0.0..=1.0).contains(&a) && (a - b).abs() < 0.05, "{a} {b}");
            assert!((0.0..=1.0).contains(&doors.order(x as f32, z as f32)));
        }
    }
    assert_eq!(from.order(f32::NAN, 0.0), 0.0, "a point that is not one is the source");
}

// ------------------------------------------------------- a real room ----

fn crypt_fight() -> crate::rules::Room {
    let floor = layout::floor_party(1, Pack::Crypt, 1, &mut Rng::new(3));
    floor.rooms.iter().find(|r| r.kind == RoomKind::Fight).expect("a fight room").clone()
}

/// The crypt's fight room as the window stands it: its frame with the
/// room's floor, blocks, flames and look, scrolled by a few pixels.
fn frame() -> (Frame, Ripple) {
    frame_of(&crypt_fight(), Pack::Crypt)
}

fn frame_of(room: &crate::rules::Room, pack: Pack) -> (Frame, Ripple) {
    let lit = rooms_lit::lit_for(room, pack).expect("lit");
    let size = (room.cols as i32 * 16, room.rows as i32 * 16);
    let world = RoomWorld::lit_world(Arc::clone(&lit), size);
    let world = RoomWorld { moment: Moment { activity: Activity::Fought, chest: None, time: 0.0, rise: 1.0 }, ..world };
    let staged = world.lit_staged((8.0, 4.0), ([0.0, 0.0], 1.0), (544.0, 320.0), &[], &|_, _| true).expect("lit");
    let world = World { floor: staged.floor, blocks: staged.blocks, extra: staged.flames, look: Some(staged.look), meshes: staged.meshes, ..Default::default() };
    let frame = Frame { size: (544, 320), tilt: 0.3, props: Some(Props { at: 0, grain: 2, nodes: Vec::new(), world: Some(world) }), ..Default::default() };
    (frame, Ripple::centre(size.0 as f32, size.1 as f32))
}

fn world_of(frame: &Frame) -> &World {
    frame.props.as_ref().and_then(|p| p.world.as_ref()).expect("a world")
}

#[test]
fn a_frame_whose_moves_are_done_is_left_exactly_as_it_is() {
    let (frame, ripple) = frame();
    let mut done = frame.clone();
    apply(&mut done, Times::DONE, &ripple);
    assert_eq!(done, frame);
    let mut later = frame.clone();
    apply(&mut later, climb(DURATION), &ripple);
    assert_eq!(later, frame);
    // And a move not begun leaves its own part alone: the tip alone changes only the tilt and the lamps.
    let mut tipped = frame.clone();
    apply(&mut tipped, Times { tip: 0.0, ..Times::DONE }, &ripple);
    assert_eq!(tipped.tilt, 0.0);
    assert_eq!(world_of(&tipped).blocks, world_of(&frame).blocks);
    assert_eq!(world_of(&tipped).floor, world_of(&frame).floor);
}

#[test]
fn the_flat_plan_is_flat_with_the_sun_overhead_and_the_lamps_out() {
    let (frame, ripple) = frame();
    let mut flat = frame.clone();
    apply(&mut flat, climb(0.0), &ripple);
    let (before, after) = (world_of(&frame), world_of(&flat));
    assert_eq!(flat.tilt, 0.0, "not tipped");
    // Every mass is a card of its own footprint: x and z as they were, no
    // thicker than a card, whatever it was.
    assert!(!after.blocks.is_empty());
    let scale = before.floor.iter().find(|n| is_ground(n)).expect("a ground").transform.scale[0];
    for (a, b) in before.blocks.iter().zip(&after.blocks) {
        assert_eq!(a.transform.scale[0], b.transform.scale[0]);
        assert_eq!(a.transform.scale[2], b.transform.scale[2]);
        assert_eq!(a.transform.translation[0], b.transform.translation[0]);
        let top = b.transform.translation[1] + b.transform.scale[1] * 0.5;
        assert!(top <= CARD * scale * 1.0001 + 0.01, "a card is {top} thick");
    }
    assert!(after.blocks.iter().any(|n| n.transform.translation[1] + n.transform.scale[1] * 0.5 > CARD * scale * 0.99), "and the tall ones are cards, not nothing");
    // The ground is flat but for the least a node can be.
    let ground = |w: &World| w.floor.iter().find(|n| is_ground(n)).expect("a ground").transform.scale[1];
    assert_eq!(ground(after), ground(before) * MIN_RELIEF);
    // The flames are sparks on the cards, riding them down.
    let flames: Vec<_> = before.extra.iter().zip(&after.extra).filter(|(a, _)| is_flame(a)).collect();
    assert!(!flames.is_empty(), "torches and braziers");
    for (a, b) in flames {
        assert!(b.transform.scale[1] < a.transform.scale[1] * 0.1 && b.transform.scale[0] < a.transform.scale[0]);
    }
    // The sun is overhead and the lamps are out, their pools shrunk.
    let (look, was) = (after.look.as_ref().expect("a look"), before.look.as_ref().expect("a look"));
    let sun = look.sun.expect("a sun");
    assert_eq!(sun.toward, [0.0, 1.0, 0.0]);
    assert!(sun.strength < was.sun.expect("a sun").strength);
    assert_eq!(look.lamps.len(), was.lamps.len());
    assert!(!look.lamps.is_empty() && look.lamps.iter().all(|l| l.strength == 0.0 && l.reach > 0.0));
    assert!(look.lights() && look.validate().is_ok(), "{:?}", look.validate());
}

#[test]
fn the_whole_climb_is_always_a_frame_the_gpu_can_draw() {
    let (frame, ripple) = frame();
    let finite = |n: &Node| {
        let t = &n.transform;
        t.translation.iter().chain(&t.scale).all(|v| v.is_finite()) && t.scale.iter().all(|&v| v >= 0.0)
    };
    for i in -20..=((DURATION + 0.3) * 50.0) as i32 {
        let t = i as f32 / 50.0;
        for times in [climb(t), Times { floor: t, ..Times::DONE }, Times { prop: t, ..Times::DONE }, Times { tip: t, ..Times::DONE }] {
            let mut f = frame.clone();
            apply(&mut f, times, &ripple);
            assert!(f.tilt.is_finite() && (0.0..=angel_stage::MAX_TILT).contains(&f.tilt), "tilt {} at {t}", f.tilt);
            let w = world_of(&f);
            assert!(w.floor.iter().chain(&w.blocks).chain(&w.extra).all(finite), "{times:?}");
            let look = w.look.as_ref().expect("a look");
            assert!(look.validate().is_ok() && look.lights(), "{:?} at {times:?}", look.validate());
            // Nothing stands taller than it is: a mass never past its lock.
            for (a, b) in world_of(&frame).blocks.iter().zip(&w.blocks) {
                assert!(b.transform.scale[1] <= a.transform.scale[1] * (1.0 + LOCK_OVER + 1.0e-4));
            }
        }
    }
}

#[test]
fn a_flame_stands_on_its_mass_as_it_rises_and_grows_about_its_foot() {
    let (frame, ripple) = frame();
    let before = world_of(&frame);
    let foot = |n: &Node| {
        let low = n.mesh.and_then(|m| m.generate()).and_then(|d| d.positions.iter().map(|p| p[1]).reduce(f32::min)).unwrap_or(-0.5);
        n.transform.translation[1] + n.transform.scale[1] * low
    };
    let top = |n: &Node| n.transform.translation[1] + n.transform.scale[1] * 0.5;
    let ground = before.floor.iter().find(|n| is_ground(n)).expect("a ground").transform;
    let field = Field::of(&before.meshes[0].1).expect("a ground field");
    let flames: Vec<usize> = (0..before.extra.len()).filter(|&i| is_flame(&before.extra[i])).collect();
    assert!(!flames.is_empty());
    let mut rode = 0;
    for t in [0.0, 0.3, 0.6, 0.9, 1.2, 1.5, 1.8, 2.1] {
        let mut f = frame.clone();
        apply(&mut f, Times { prop: t, ..Times::DONE }, &ripple);
        let after = world_of(&f);
        for &i in &flames {
            let (a, b) = (&before.extra[i], &after.extra[i]);
            // Its mass: the tallest thing standing at its middle, and what is
            // taller still at the middle of that.
            let (x, z) = (a.transform.translation[0], a.transform.translation[2]);
            let tallest = |x: f32, z: f32| {
                (0..before.blocks.len())
                    .filter(|&k| Column::of(&before.blocks[k]).holds(x, z))
                    .max_by(|&p, &q| top(&before.blocks[p]).total_cmp(&top(&before.blocks[q])))
            };
            let Some(mut k) = tallest(x, z) else {
                assert_eq!(foot(b), foot(a), "a flame on no mass stays");
                continue;
            };
            while let Some(next) = tallest(Column::of(&before.blocks[k]).at[0], Column::of(&before.blocks[k]).at[1]).filter(|&n| top(&before.blocks[n]) > top(&before.blocks[k])) {
                k = next;
            }
            let m = after.blocks[k].transform.scale[1] / before.blocks[k].transform.scale[1];
            let base = field.height((x - ground.translation[0]) / ground.scale[0], (z - ground.translation[2]) / ground.scale[0]) * ground.scale[1];
            let want = base + (foot(a) - base) * m;
            assert!(
                (foot(b) - want).abs() < 1.0e-3 * want.abs().max(1.0),
                "its foot rides its mass at {t}: {} vs {want} (foot {}, m {m}, its mass {} tall at {:?}, flame at {x},{z})",
                foot(b),
                foot(a),
                top(&before.blocks[k]),
                Column::of(&before.blocks[k]).at,
            );
            rode += 1;
        }
    }
    assert!(rode > 0, "some flame stood on a mass");
}

#[test]
fn the_ground_field_reads_the_sets_heights() {
    let room = crypt_fight();
    let lit = rooms_lit::lit_for(&room, Pack::Crypt).expect("lit");
    let piece = lit.piece((0, 0)).expect("reads");
    let ground = piece.set.ground.as_ref().expect("a ground");
    let mesh = piece.nodes().2.clone().expect("its mesh");
    let field = Field::of(&mesh).expect("a field");
    let mut sunk = 0;
    for j in 0..=(room.rows as i32 * 4) {
        for i in 0..=(room.cols as i32 * 4) {
            let (x, z) = (i as f32 * 4.0 + 1.3, j as f32 * 4.0 + 0.7);
            let (a, b) = (field.height(x, z), ground.height_at(x, z));
            assert!((a - b).abs() < 1.0e-4, "{x},{z}: {a} vs {b}");
            sunk += usize::from(a < -2.0);
        }
    }
    assert!(sunk > 10, "the crypt's pits are in the field ({sunk})");
    assert!(Field::of(&MeshData::default()).is_none());
}

#[test]
fn a_card_stands_on_the_dais_it_is_never_buried_in() {
    // Blocks of the floor stand on a dais of the ground (3 pixels, as many
    // tiles across), their masses on it.
    let mut room = crypt_fight();
    let (c, r) = (room.cols as usize / 2 - 1, room.rows as usize / 2 + 2);
    for dc in 0..3 {
        for dr in 0..3 {
            room.set_for_test(c + dc, r + dr, crate::rules::Tile::Block);
        }
    }
    let (frame, ripple) = frame_of(&room, Pack::Crypt);
    let before = world_of(&frame);
    let g = before.floor.iter().find(|n| is_ground(n)).expect("a ground").transform;
    let field = Field::of(&before.meshes[0].1).expect("a field");
    let top = |n: &Node| n.transform.translation[1] + n.transform.scale[1] * 0.5;
    let base = |n: &Node| field.height((n.transform.translation[0] - g.translation[0]) / g.scale[0], (n.transform.translation[2] - g.translation[2]) / g.scale[0]) * g.scale[1];
    let on_dais: Vec<usize> = (0..before.blocks.len()).filter(|&k| base(&before.blocks[k]) > 2.9).collect();
    assert!(!on_dais.is_empty(), "something stands on the dais");
    for t in [0.0, 0.1, 0.2, 0.4, 0.8] {
        let mut f = frame.clone();
        apply(&mut f, Times { prop: t, ..Times::DONE }, &ripple);
        let after = world_of(&f);
        // No part of any mass is under the ground it stands on.
        for (a, b) in before.blocks.iter().zip(&after.blocks) {
            if top(a) > base(a) + 1.0e-3 {
                assert!(top(b) >= base(a) - 1.0e-3, "a mass buried at {t}: top {} under {}", top(b), base(a));
            }
        }
        // And at the start the tallest of what stands on it is a card on it.
        if t == 0.0 {
            let tallest = on_dais.iter().copied().max_by(|&p, &q| top(&before.blocks[p]).total_cmp(&top(&before.blocks[q]))).expect("some");
            let card = top(&after.blocks[tallest]) - base(&before.blocks[tallest]);
            assert!(card > 0.0 && card <= CARD * g.scale[0] * 1.001, "a card thick: {card}");
        }
    }
    // A card on no dais is as thick over the floor.
    let mut flat = frame.clone();
    apply(&mut flat, Times { prop: 0.0, ..Times::DONE }, &ripple);
    for (a, b) in before.blocks.iter().zip(&world_of(&flat).blocks) {
        assert!(top(b) - base(a) <= CARD * g.scale[0] * 1.001, "no mass thicker than a card at 0");
    }
}

#[test]
fn what_stands_together_rises_together() {
    let (frame, ripple) = frame();
    let before = world_of(&frame);
    for t in [0.2, 0.5, 0.8, 1.1] {
        let mut f = frame.clone();
        apply(&mut f, Times { prop: t, ..Times::DONE }, &ripple);
        let after = world_of(&f);
        let m = |k: usize| after.blocks[k].transform.scale[1] / before.blocks[k].transform.scale[1];
        let mut together = 0;
        for (p, a) in before.blocks.iter().enumerate() {
            for (q, b) in before.blocks.iter().enumerate().skip(p + 1) {
                let same = (a.transform.translation[0] - b.transform.translation[0]).abs() < 0.01 && (a.transform.translation[2] - b.transform.translation[2]).abs() < 0.01;
                if same {
                    assert!((m(p) - m(q)).abs() < 1.0e-5, "a plinth, a shaft and a crown rise as one ({} {}) at {t}", m(p), m(q));
                    together += 1;
                }
            }
        }
        assert!(together > 0, "the room has a pillar built of parts");
    }
}

#[test]
fn a_mass_holds_what_is_under_it_and_not_what_is_near_it() {
    use angel_stage::scene::{MeshRef, Transform};
    let node = |translation: [f32; 3], scale: [f32; 3], yaw: f32| {
        let mut n = Node::new(1, MeshRef::Cube, angel_stage::scene::Material::default());
        n.transform = Transform { translation, scale, ..Transform::from_rotation_axis_angle([0.0, 1.0, 0.0], yaw) };
        n
    };
    // A long thin wall holds the ground it covers, not the floor across from it.
    let wall = Column::of(&node([200.0, 20.0, 10.0], [360.0, 40.0, 8.0], 0.0));
    assert!(wall.holds(30.0, 12.0) && wall.holds(370.0, 7.0));
    assert!(!wall.holds(200.0, 40.0), "not the floor 30 pixels off across it");
    assert!(!wall.holds(10.0, 10.0), "nor off its end");
    assert_eq!(wall.top, 40.0);
    // Turned a quarter, it lies across the other way.
    let turned = Column::of(&node([200.0, 20.0, 10.0], [360.0, 40.0, 8.0], std::f32::consts::FRAC_PI_2));
    assert!(turned.holds(200.0, 150.0) && turned.holds(203.0, -150.0));
    assert!(!turned.holds(30.0, 12.0));
}

#[test]
fn what_the_lift_calls_a_flame_is_what_the_room_draws_as_one() {
    let room = crypt_fight();
    let lit = rooms_lit::lit_for(&room, Pack::Crypt).expect("lit");
    let piece = lit.piece((0, 0)).expect("reads");
    let fire = Tone::Delve(Pack::Crypt).flame();
    let nodes = flame_nodes(&[Arc::clone(&piece)], Activity::Fought, 0.0, fire);
    assert_eq!(nodes.iter().filter(|n| is_flame(n)).count(), piece.flames.len());
    let nodes = flame_nodes(&[piece], Activity::Cleared, 0.0, fire);
    assert!(nodes.iter().filter(|n| is_flame(n)).count() > 0, "embers are flames to it too");
}

#[test]
fn water_lies_on_the_flat_plan_and_goes_down_with_the_ground() {
    let floor = layout::floor_party(2, Pack::Cavern, 2, &mut Rng::new(5));
    let room = floor.rooms.iter().find(|r| r.kind == RoomKind::Fight).expect("a fight room").clone();
    let (frame, ripple) = frame_of(&room, Pack::Cavern);
    let before = world_of(&frame);
    let top = |n: &Node| n.transform.translation[1] + n.transform.scale[1] * 0.5;
    let pools: Vec<usize> = (0..before.extra.len()).filter(|&i| !is_flame(&before.extra[i]) && top(&before.extra[i]) <= SUNK).collect();
    assert!(!pools.is_empty(), "the mines' room has water in its bed");
    assert!(pools.iter().any(|&i| top(&before.extra[i]) < -1.0), "sunk below the floor");
    let mut flat = frame.clone();
    apply(&mut flat, Times { floor: 0.0, ..Times::DONE }, &ripple);
    let mut mid = frame.clone();
    apply(&mut mid, Times { floor: 0.3, ..Times::DONE }, &ripple);
    for &i in &pools {
        let (a, f, m) = (top(&before.extra[i]), top(&world_of(&flat).extra[i]), top(&world_of(&mid).extra[i]));
        // A film over the flat ground, so the pool shows on the plan; then it
        // goes down with the bed, never above where it ends or the film.
        assert!(f > 0.0 && f <= FILM * 1.2, "a film over the floor: {f}");
        assert!(m <= f + 1.0e-4 && m >= a.min(f) - 1.0e-4, "on its way down: {f} {m} {a}");
    }
    // Done, the water is untouched.
    let mut done = frame.clone();
    apply(&mut done, Times::DONE, &ripple);
    assert_eq!(world_of(&done).extra, before.extra);
}
