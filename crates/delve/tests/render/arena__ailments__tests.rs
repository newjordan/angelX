use super::*;
use crate::render::overworld::ink::Img;

/// Every ailment stays near its figure: what it paints lies in a box a
/// little bigger than the figure, whatever the tick.
#[test]
fn each_mark_keeps_to_its_figure() {
    for &a in Ail::ALL.iter() {
        for tick in (0..240u64).step_by(7) {
            let mut cv = Img::new(160, 120);
            let body = Body { feet: (80.0, 80.0), h: 20.0, w: 14.0, seed: 7 };
            let mut pen = super::super::pfx::Pen::new(&mut cv, 400);
            mark(&mut pen, a, &body, tick);
            for y in 0..cv.h {
                for x in 0..cv.w {
                    if cv.get(x, y).is_some() {
                        assert!((x - 80).abs() <= 22 && (40..=86).contains(&y), "{a:?} at tick {tick} painted ({x}, {y})");
                    }
                }
            }
        }
    }
}

/// The same figure, the same tick, the same pixels.
#[test]
fn a_mark_is_the_same_on_every_seat() {
    for &a in Ail::ALL.iter() {
        let draw = || {
            let mut cv = Img::new(80, 80);
            let mut pen = super::super::pfx::Pen::new(&mut cv, 400);
            mark(&mut pen, a, &Body { feet: (40.0, 60.0), h: 20.0, w: 14.0, seed: 3 }, 123);
            cv
        };
        assert_eq!(draw(), draw(), "{a:?}");
    }
}
