//! The kit's promises: the same call paints the same pixels, a pen never
//! paints past its budget, light never dims what it lands on, every ink a
//! ramp names is the realm's, and the beats run in their order.

use super::*;
use crate::motion::Beat;
use crate::render::overworld::ink::{Img, ink};

fn painted(cv: &Img) -> usize {
    (0..cv.h).flat_map(|y| (0..cv.w).map(move |x| (x, y))).filter(|&(x, y)| cv.get(x, y).is_some()).count()
}

fn scene(cv: &mut Img, budget: u32) -> u32 {
    let mut pen = Pen::new(cv, budget);
    pen.glow((40.0, 30.0), 12.0, 1.0, ramps::FIRE, 1.0);
    pen.ring((40.0, 30.0), 18.0, 2.0, 0.6, ramps::FROST, 0.2);
    pen.glyph((40.0, 30.0), 14.0, 0.5, 0.3, 1.0, 5, ramps::ARCANE, 0.1, 7);
    pen.arc((10.0, 5.0), (60.0, 50.0), 3, 6.0, ramps::STORM, 0.0);
    pen.wisp((40.0, 50.0), 12.0, (0.0, 10.0), 16.0, 14.0, 2.5, ramps::SMOKE, 4);
    rays(&mut pen, (40.0, 30.0), 8, 12.0, 1.0, 0.0, ramps::HOLY, 0.3);
    let spray = Spray::round(9, 30, (40.0, 30.0), (1.0, 3.0), (10.0, 20.0));
    spray.each(6.0, |m| mote(&mut pen, &m, Shape::Spark, ramps::FIRE, 1, 6));
    pen.left()
}

#[test]
fn the_same_call_paints_the_same_pixels() {
    let (mut a, mut b) = (Img::new(80, 64), Img::new(80, 64));
    scene(&mut a, 5000);
    scene(&mut b, 5000);
    assert_eq!(a, b);
    assert!(painted(&a) > 200, "the scene paints");
}

#[test]
fn a_pen_never_paints_past_its_budget() {
    let mut cv = Img::new(80, 64);
    let left = scene(&mut cv, 60);
    assert_eq!(left, 0);
    assert!(painted(&cv) <= 60);
}

#[test]
fn light_never_dims_a_brighter_pixel() {
    let mut cv = Img::new(20, 20);
    cv.rect(0, 0, 20, 20, 'w');
    let before = cv.clone();
    let mut pen = Pen::new(&mut cv, 1000);
    pen.glow((10.0, 10.0), 8.0, 1.0, ramps::VOID, 1.0);
    pen.ring((10.0, 10.0), 5.0, 2.0, 1.0, ramps::POOP, 0.0);
    assert_eq!(cv, before, "white stays white under a dim glow");
}

#[test]
fn every_ramp_is_the_realms_inks() {
    for r in [
        ramps::FIRE, ramps::EMBER, ramps::FROST, ramps::HOLY, ramps::GOLD, ramps::ARCANE, ramps::GENIE, ramps::STORM,
        ramps::POISON, ramps::NATURE, ramps::BLOOD, ramps::ROSE, ramps::POOP, ramps::BREW, ramps::VOID, ramps::WATER,
        ramps::BONE, ramps::SMOKE, ramps::EARTH,
    ] {
        for ch in r.0 {
            assert!(ink(ch).is_some(), "{ch} is an ink");
        }
    }
}

#[test]
fn the_beats_run_wind_burst_settle_linger() {
    let b = Beats::new(4.0, 4.0, 4.0, 8.0);
    assert!(matches!(b.at(1.0), Beat::Anticipation(_)));
    assert!(matches!(b.at(5.0), Beat::Action(_)));
    assert!(matches!(b.at(9.0), Beat::Follow(_)));
    assert!(matches!(b.at(13.0), Beat::Settle(_)));
    assert_eq!(b.at(21.0), Beat::Done);
    assert!(b.size(8.0) > 1.0, "it overshoots at the end of the burst");
    assert!((b.size(13.0) - 1.0).abs() < 1e-4, "and settles");
    assert!(b.power(19.5) < 0.2, "and goes");
}

#[test]
fn a_spray_is_bounded_and_lands_where_it_was_thrown_at_first() {
    let spray = Spray::round(1, 500, (10.0, 10.0), (2.0, 2.0), (5.0, 5.0));
    let mut n = 0;
    spray.each(0.0, |m| {
        n += 1;
        assert_eq!(m.at, (10.0, 10.0));
    });
    assert_eq!(n, MOST_MOTES);
}
