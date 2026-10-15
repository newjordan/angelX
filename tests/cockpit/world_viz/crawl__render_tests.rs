use super::*;

/// A corridor three cells long running east into a wall.
fn corridor() -> Grid {
    let (w, h) = (6usize, 3usize);
    let mut cells = vec![Cell::Wall(0); w * h];
    for x in 1..5 {
        cells[w + x] = Cell::Open(Ground::Flags);
    }
    Grid {
        w,
        h,
        cells,
        decals: Default::default(),
    }
}

fn plain(ink: &str) -> Tex {
    Tex::from_rows(&[ink, ink])
}

#[test]
fn a_wall_ahead_fills_the_middle_of_the_view_and_hides_what_stands_behind_it() {
    let grid = corridor();
    let (wall, floor) = (plain("hh"), plain("XX"));
    let walls = vec![wall];
    let kit = Kit {
        walls: &walls,
        decals: &[],
        floor: &floor,
        water: &floor,
        lava: &floor,
        stairs: &floor,
        ceiling: &floor,
    };
    let eye = Eye {
        x: 1.5,
        y: 1.5,
        heading: 0.0,
        fov: 1.15,
        z: 0.5,
        pitch: 0.0,
    };
    let torch = Torch {
        reach: 6.0,
        gain: 1.0,
        glow: None,
        sconces: [None; 3],
    };
    let monster = plain("77");
    // One standing in the corridor, one beyond the end wall.
    let things = [
        Thing {
            x: 3.5,
            y: 1.5,
            tex: &monster,
            height: 0.6,
            lift: 0.0,
            lit: true,
            flip: false,
        },
        Thing {
            x: 5.5,
            y: 1.5,
            tex: &monster,
            height: 0.6,
            lift: 0.0,
            lit: true,
            flip: false,
        },
    ];
    let a = render(&grid, &eye, &things, &kit, torch, (64, 48));
    let b = render(&grid, &eye, &things, &kit, torch, (64, 48));
    assert_eq!(a, b, "the same frame every time");
    // The middle column: the end wall (3.5 cells off) spans a band round the
    // horizon; the monster in front is lit red low in the middle.
    let red = |p: &image::Rgba<u8>| p[0] > 120 && p[1] < 80;
    let column: Vec<_> = (0..48).map(|y| *a.get_pixel(32, y)).collect();
    assert!(column.iter().any(red), "the near monster shows");
    // Nothing red off to the sides where only the hidden one would be.
    assert!(!(0..48).any(|y| red(a.get_pixel(5, y))));
    // The far monster alone (the near one gone) is hidden by the wall.
    let hidden = render(&grid, &eye, &things[1..], &kit, torch, (64, 48));
    assert!(!(0..48).any(|y| (0..64).any(|x| red(hidden.get_pixel(x, y)))));
}

#[test]
fn cached_surface_softening_keeps_coverage_and_quiets_checker_energy() {
    let source = Tex::from_rows(&["hkhk", "khkh", "hkhk", "khkh"]);
    let finish = source.clone().soft_surface();
    let contrast = |tex: &Tex| {
        (i16::from(tex.at(0.125, 0.125).unwrap()[0]) - i16::from(tex.at(0.375, 0.125).unwrap()[0]))
            .abs()
    };
    assert!(contrast(&finish) * 3 < contrast(&source));
    assert_eq!((finish.w, finish.h), (source.w, source.h));
    let holes = Tex::from_rows(&[".hh.", "hhhh", "hhhh", ".hh."]);
    let soft = holes.clone().soft_surface();
    for y in 0..4 {
        for x in 0..4 {
            let (u, v) = ((x as f32 + 0.5) / 4.0, (y as f32 + 0.5) / 4.0);
            assert_eq!(holes.at(u, v).is_some(), soft.at(u, v).is_some());
        }
    }
}
