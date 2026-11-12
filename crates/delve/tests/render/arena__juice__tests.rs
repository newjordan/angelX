use super::*;
use crate::rules::juice::Weight;

fn lit(im: &Img) -> usize {
    im.pixels().filter(|p| *p != super::super::super::ink::BLACK).count()
}
#[test]
fn shake_scales_with_the_blow_and_settles() {
    let still = shake([(Weight::Graze, 0)], 3);
    assert_eq!(still, (0, 0), "a scratch moves nothing");
    let big = (0..12).map(|t| shake([(Weight::Finale, 0)], t)).map(|(x, y)| x.abs() + y.abs()).max().unwrap();
    let small = (0..12).map(|t| shake([(Weight::Solid, 0)], t)).map(|(x, y)| x.abs() + y.abs()).max().unwrap();
    assert!(big > small && small > 0, "{big} {small}");
    assert_eq!(shake([(Weight::Finale, 40)], 9), (0, 0), "it settles");
}
#[test]
fn squash_keeps_the_feet_and_comes_to_rest() {
    let sprite = Img::from_rows(&["hhhh", "hhhh", "hhhh", "hhhh"]);
    for age in 0..30 {
        let (im, grow) = squash_stretch(&sprite, age, 0.4);
        assert!(grow >= 1.0);
        assert!((0..im.w).any(|x| im.get(x, im.h - 1).is_some()), "feet stay on the bottom row at {age}");
    }
    let (rest, grow) = squash_stretch(&sprite, 60, 0.4);
    assert_eq!((rest.w, rest.h, grow), (4, 4, 1.0));
}
#[test]
fn the_banner_slams_holds_and_leaves() {
    let draw = |age| {
        let mut im = Img::black(384, 224);
        banner(&mut im, 40, "DOUBLE KILL", "KNIGHT 2", age, 78, Tone::Warm, 3);
        im
    };
    assert!(lit(&draw(0)) > 0 && lit(&draw(30)) > 0);
    assert!(lit(&draw(30)) > lit(&draw(76)), "it dissolves at the end");
    assert_eq!(lit(&draw(78)), 0, "and is gone");
    // The longest word still fits the screen at every age.
    for age in 0..78 {
        let mut im = Img::black(384, 224);
        banner(&mut im, 40, "TAKES THE LEAD", "", age, 78, Tone::Warm, 3);
        for y in 0..224 {
            assert!(im.get(0, y).is_none_or(|p| p == super::super::super::ink::BLACK), "touches the left edge at {age}");
            assert!(im.get(383, y).is_none_or(|p| p == super::super::super::ink::BLACK), "touches the right edge at {age}");
        }
    }
}
#[test]
fn countdown_and_podium_draw_inside_the_screen() {
    for age in 0..24 {
        let mut im = Img::black(384, 224);
        countdown(&mut im, (192, 80), "3", age, 24, false);
        assert!(lit(&im) > 0 || age > 22, "age {age}");
    }
    let mut im = Img::black(384, 224);
    let place = |n: &str| Place { sprite: Img::from_rows(&["hh", "hh"]), name: n.into(), line: "7 KILLS".into() };
    let crown = podium(&mut im, (0, 0, 384, 224), &[place("A"), place("B"), place("C")], 60);
    assert!(crown.1 > 0 && crown.1 < 224);
    let none = podium(&mut Img::black(384, 224), (0, 0, 384, 224), &[place("A")], 0);
    assert!(none.1 > 0);
}
#[test]
fn confetti_is_a_pure_function_of_the_tick() {
    let mut a = Img::black(384, 224);
    let mut b = Img::black(384, 224);
    confetti(&mut a, (0, 0, 384, 224), 17, 5, 90);
    confetti(&mut b, (0, 0, 384, 224), 17, 5, 90);
    assert_eq!(a.content_hash(), b.content_hash());
    let mut c = Img::black(384, 224);
    confetti(&mut c, (0, 0, 384, 224), 18, 5, 90);
    assert_ne!(a.content_hash(), c.content_hash());
}
#[test]
fn podium_words_fit_their_steps_and_never_meet() {
    let black = super::super::super::ink::BLACK;
    let meet = |a: Rect, b: Rect| a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3;
    let inside = |r: Rect| r.0 >= 0 && r.1 >= 0 && r.0 + r.2 <= 384 && r.1 + r.3 <= 224;
    let names = ["YOU", "SIR PERCIVAL THE UNYIELDING OF THE LONG ROAD", "WWWWWWWWWWWWWWWWWWWW", "MMMMMMMMMM", "KNIGHT 12", "A"];
    let lines = ["12 NO FALLS", "123456 CRATES AND BARRELS BROKEN", "1234567890123 PTS", "999 KILLS", "OUT", "7"];
    // The boxes the games use (floors at 182, 162 and 174), and a low one.
    for area in [(0, 0, 384, 188), (0, 34, 384, 134), (0, 56, 384, 124), (0, 0, 384, 210)] {
        for n in 3..=6 {
            for you in 0..n {
                let places: Vec<Place> = (0..n)
                    .map(|k| Place {
                        sprite: Img::from_rows(&["hh", "hh"]),
                        name: if k == you { "YOU".into() } else { names[(k + 1) % names.len()].into() },
                        line: lines[(k + you) % lines.len()].into(),
                    })
                    .collect();
                let stands = podium_stands((384, 224), area, &places);
                assert_eq!(stands.len(), 3, "the podium shows three of {n}");
                let boxes: Vec<(usize, &str, Rect)> =
                    stands.iter().enumerate().flat_map(|(i, s)| [(i, "rank", s.rank), (i, "name", s.name), (i, "line", s.line)]).collect();
                for &(i, what, r) in &boxes {
                    assert!(inside(r), "{what} of place {} leaves the frame: {r:?} ({area:?}, {n} places)", i + 1);
                }
                for (a, &(i, wa, ra)) in boxes.iter().enumerate() {
                    for &(j, wb, rb) in &boxes[a + 1..] {
                        assert!(!meet(ra, rb), "{wa} of place {} meets {wb} of place {}: {ra:?} {rb:?} ({area:?}, {n} places)", i + 1, j + 1);
                    }
                }
                // Each digit sits inside its own step's face.
                let floor = area.1 + area.3 - 6;
                for (s, h) in stands.iter().zip([58, 40, 28]) {
                    assert!(s.rank.1 >= floor - h + 3 && s.rank.1 + s.rank.3 <= floor, "digit {} spills off its step: {:?}", s.rank_scale, s.rank);
                }
                // And in the picture: with the words gone, nothing below the floor is lit.
                let bare: Vec<Place> = places.iter().map(|p| Place { sprite: p.sprite.clone(), name: String::new(), line: String::new() }).collect();
                let mut im = Img::black(384, 224);
                podium(&mut im, area, &bare, 80);
                for y in floor..224 {
                    for x in 0..384 {
                        assert!(im.get(x, y).is_none_or(|p| p == black), "ink below the steps at {x},{y} ({area:?})");
                    }
                }
                // And with the words, they are drawn.
                let mut full = Img::black(384, 224);
                podium(&mut full, area, &places, 80);
                assert!(lit(&full) > lit(&im), "the words are drawn");
            }
        }
    }
}
