use super::*;

fn on_palette(img: &Img) -> bool {
    img.pixels().all(|c| c == BLACK || super::super::ink::palette_index(c).is_some())
}

#[test]
fn material_steps_stay_in_their_bank_and_never_touch_signal() {
    for &v in PALETTE.iter().take(BANK * SIGNAL_BANK) {
        let c = rgb(v);
        if c == BLACK {
            continue;
        }
        assert_eq!(bank_of(lighter(c)), bank_of(c));
        assert!(lum(lighter(c)) >= lum(c));
        assert!(lum(darker(c)) <= lum(c));
    }
    for ch in ['1', '4', '9', 'w'] {
        let c = ink(ch).unwrap();
        assert_eq!(lighter(c), c);
        assert_eq!(darker(c), c);
    }
}

#[test]
fn materials_draw_on_the_palette_the_same_way_every_time() {
    let draw = || {
        let mut cv = Img::black(160, 96);
        let keep = |_: i32, _: i32| true;
        water(&mut cv, (4, 4, 70, 30), 3, 12);
        tree(&mut cv, 100, 40, 11.0, Leaf::Oak, 5, &keep);
        tree(&mut cv, 130, 40, 9.0, Leaf::Pine, 6, &keep);
        boulder(&mut cv, 20, 80, 6.0, ramp::STONE, 7);
        reeds(&mut cv, 50, 80, 8);
        house(&mut cv, (80, 50, 40, 36), &House { roof: ramp::THATCH, stone: true, windows: 2 }, 9);
        for y in 0..cv.h {
            for x in 0..cv.w {
                if cv.get(x, y) == Some(BLACK)
                    && let Some(ch) = ground(Ground::Meadow, x, y)
                {
                    cv.put(x, y, ch);
                }
            }
        }
        relief(&mut cv);
        cv
    };
    let a = draw();
    assert_eq!(a, draw());
    assert!(on_palette(&a));
    assert!(a.pixels().all(|c| !super::super::ink::is_signal(c)), "material never borrows a signal ink");
}

#[test]
fn water_never_leaves_its_rectangle() {
    let mut cv = Img::new(80, 60);
    water(&mut cv, (10, 12, 40, 30), 1, 0);
    for y in 0..cv.h {
        for x in 0..cv.w {
            if cv.get(x, y).is_some() {
                assert!((10..50).contains(&x) && (12..42).contains(&y), "{x},{y}");
            }
        }
    }
}

#[test]
fn ground_leaves_most_of_the_paper_black() {
    for g in [Ground::Meadow, Ground::Sand, Ground::Snow, Ground::Earth, Ground::Mud, Ground::Scree] {
        let inked = (0..384 * 224).filter(|i| ground(g, i % 384, i / 384).is_some()).count();
        assert!(inked * 4 < 384 * 224, "{g:?} covers {inked}");
        assert!(inked > 0, "{g:?} draws nothing");
    }
}
