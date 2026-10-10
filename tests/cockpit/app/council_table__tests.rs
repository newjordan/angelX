use super::*;

fn brightness(img: &RgbaImage, at: (i32, i32)) -> u32 {
    let (x, y) = ((at.0 - CROP.0 as i32) as u32, (at.1 - CROP.1 as i32) as u32);
    let mut sum = 0u32;
    for dy in 0..3 {
        for dx in 0..3 {
            let Rgba([r, g, b, _]) = *img.get_pixel(x + dx - 1, y + dy - 1);
            sum += u32::from(r) + u32::from(g) + u32::from(b);
        }
    }
    sum
}

#[test]
fn the_station_plate_is_the_picture() {
    let plate = station();
    assert_eq!((plate.width(), plate.height()), (144, 144), "plate decodes");
    let table = compose(&[], Deed::Council);
    assert_eq!((table.width(), table.height()), (WIDTH, HEIGHT));
    // Every opaque pixel of the plate inside the crop is painted, and the
    // air around the table stays clear for the panel behind it.
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let src = plate.get_pixel(x + CROP.0, y + CROP.1)[3];
            let dst = table.get_pixel(x, y)[3];
            assert_eq!(src > 0, dst > 0, "alpha differs at {x},{y}");
        }
    }
    let opaque = table.pixels().filter(|p| p[3] > 0).count();
    assert!(
        opaque > (WIDTH * HEIGHT / 2) as usize,
        "the table fills its box"
    );
}

#[test]
fn sitting_seats_light_their_thrones_and_answers_lie_on_the_table() {
    let empty = compose(&[], Deed::Council);
    let council = compose(
        &[SeatState::Running, SeatState::Returned, SeatState::Failed],
        Deed::Council,
    );
    // Three seats spread over the thrones: the head, the right, the
    // lower left.
    let places = seat_places(3);
    assert_eq!(
        places,
        vec![Place::Chair(0), Place::Chair(2), Place::Chair(5)]
    );
    let (head, head_place) = CHAIRS[0];
    assert!(
        brightness(&council, head) > brightness(&empty, head) * 3 / 2,
        "a sitting seat's throne is lit"
    );
    // The running seat's candle burns at its place.
    let flame = council.get_pixel(
        (head_place.0 - CROP.0 as i32) as u32,
        (head_place.1 - 3 - CROP.1 as i32) as u32,
    );
    assert_eq!(flame.0[..3], INK_FLAME_CORE, "candle flame at the head");
    // The answered seat's parchment is laid out.
    let (_, right_place) = CHAIRS[2];
    let sheet = council.get_pixel(
        (right_place.0 - 2 - CROP.0 as i32) as u32,
        (right_place.1 - 2 - CROP.1 as i32) as u32,
    );
    assert_eq!(sheet.0[..3], INK_C, "parchment before the answered seat");
    // The failed seat's lies there in red.
    let (_, failed_place) = CHAIRS[5];
    let red = council.get_pixel(
        (failed_place.0 - 2 - CROP.0 as i32) as u32,
        (failed_place.1 - 2 - CROP.1 as i32) as u32,
    );
    assert_eq!(red.0[..3], INK_WAX_LIGHT, "failed answer in red");
    // An empty throne stays in the dark.
    let (vacant, _) = CHAIRS[1];
    assert!(brightness(&council, vacant) < brightness(&council, head) / 2);
}

#[test]
fn the_deed_shows_at_the_heart_and_seats_past_eight_take_stools() {
    let council = compose(&[SeatState::Running; 3], Deed::Council);
    for deed in [Deed::Judge, Deed::Verify, Deed::Synthesis] {
        let marked = compose(&[SeatState::Running; 3], deed);
        let (x, y) = (
            (MAP_CENTRE.0 - CROP.0 as i32) as u32,
            (MAP_CENTRE.1 - CROP.1 as i32) as u32,
        );
        let differs = (0..9)
            .flat_map(|dy| (0..9).map(move |dx| (x + dx - 4, y + dy - 4)))
            .any(|(x, y)| marked.get_pixel(x, y) != council.get_pixel(x, y));
        assert!(differs, "{deed:?} marks the map");
    }
    assert_eq!(Deed::of_stage("judge panel"), Deed::Judge);
    assert_eq!(Deed::of_stage("verify"), Deed::Verify);
    assert_eq!(Deed::of_stage("synthesis"), Deed::Synthesis);
    assert_eq!(Deed::of_stage("proposer wave 2"), Deed::Council);

    let places = seat_places(12);
    assert_eq!(places.len(), 12);
    assert_eq!(places[8], Place::Stool(0));
    let twelve = compose(&[SeatState::Running; 12], Deed::Council);
    let stool = STOOLS[0];
    let flame = twelve.get_pixel(
        (stool.0 - CROP.0 as i32) as u32,
        (stool.1 - 3 - CROP.1 as i32) as u32,
    );
    assert_eq!(
        flame.0[..3],
        INK_FLAME_CORE,
        "ninth seat's candle on the rim"
    );
    assert_eq!(seat_places(40).len(), 16, "sixteen seats at most");
}

#[test]
fn fit_keeps_the_shape_and_hard_pixels() {
    let table = compose(&[SeatState::Running; 4], Deed::Judge);
    let big = fit(&table, (WIDTH * 3, HEIGHT * 3));
    assert_eq!((big.width(), big.height()), (WIDTH * 3, HEIGHT * 3));
    let small = fit(&table, (200, 120));
    assert!(small.width() <= 200 && small.height() <= 120);
    assert!(small.height() >= 119, "fills the rows it is given");
    assert_eq!(columns_for(6, (10, 20)), 13);
    assert_ne!(
        picture_key(&[SeatState::Running], Deed::Council),
        picture_key(&[SeatState::Returned], Deed::Council)
    );
}

/// `ANGEL_COUNCIL_SHOTS=<dir>`: the composed tables, native and at panel
/// sizes, for looking at.
#[test]
fn write_council_table_shots() {
    let Some(dir) = std::env::var_os("ANGEL_COUNCIL_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    use SeatState::{Cut, Failed, Returned, Running};
    let scenes: [(&str, Vec<SeatState>, Deed); 6] = [
        ("empty", vec![], Deed::Council),
        ("wave3", vec![Running, Returned, Running], Deed::Council),
        (
            "judge5",
            vec![Returned, Running, Failed, Returned, Running],
            Deed::Judge,
        ),
        ("verify2", vec![Running, Returned], Deed::Verify),
        (
            "synth8",
            vec![
                Returned, Returned, Running, Returned, Cut, Returned, Failed, Returned,
            ],
            Deed::Synthesis,
        ),
        ("wave12", vec![Running; 12], Deed::Council),
    ];
    for (name, states, deed) in scenes {
        let table = compose(&states, deed);
        table.save(dir.join(format!("{name}-native.png"))).unwrap();
        let big = fit(&table, (WIDTH * 4, HEIGHT * 4));
        big.save(dir.join(format!("{name}-x4.png"))).unwrap();
        for (cw, ch) in [(10u16, 20u16), (20, 40)] {
            let rows = 6u16;
            let cols = columns_for(rows, (cw, ch));
            let shown = fit(&table, (u32::from(cols * cw), u32::from(rows * ch)));
            shown
                .save(dir.join(format!("{name}-panel-{cw}x{ch}.png")))
                .unwrap();
        }
    }
}
