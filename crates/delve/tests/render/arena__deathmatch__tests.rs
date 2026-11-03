use super::*;
fn scene() -> Run {
    let mut r = crate::rules::roundtable::fixture(&dm::ADVENTURE);
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    // The round seats everyone the town door's room holds; no tick (the leg) is needed to draw it.
    r.deathmatch = Some(dm::Session::new(&r.players, &r.book));
    let s = r.deathmatch.as_mut().unwrap();
    s.ticks = dm::TITLE + 60;
    for (id, f) in &mut s.fighters {
        f.protection = 0;
        if *id == 1 {
            (f.hero.x, f.hero.y) = (13., 23.);
            f.yaw = 0.;
        }
    }
    let f = s.fighters.get_mut(&2).unwrap();
    (f.hero.x, f.hero.y) = (25., 23.);
    r
}
#[test]
fn each_seat_has_its_own_first_person_view_and_stage() {
    let r = scene();
    let a = layers(&r, Some(1)).0;
    let b = layers(&r, Some(6)).0;
    assert_ne!(a.content_hash(), b.content_hash());
    assert_eq!((a.w, a.h), (384, 224));
    let native = super::super::frame_for(&r, 384, 224, Some(1));
    assert_eq!(native, a);
    let parts = super::super::scene_for(&r, 384, 224, Some(1));
    assert!(
        parts.figures.is_empty(),
        "the stage carries its own billboards"
    );
}
#[test]
#[ignore = "writes PNG frames and reports the six-seat frame budget"]
fn write_deathmatch_shots() {
    let dir = &std::path::PathBuf::from(std::env::var_os("DELVE_DEATHMATCH_SHOTS").expect("set DELVE_DEATHMATCH_SHOTS to the output directory"));
    std::fs::create_dir_all(dir).unwrap();
    let save = |r: &Run, name: &str, seat| {
        let im = super::super::frame_for(r, 768, 448, Some(seat));
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgba_bytes(),
            im.w as u32,
            im.h as u32,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    let mut r = scene();
    save(&r, "hub", 1);
    save(&r, "corridor", 6);
    r.deathmatch.as_mut().unwrap().ticks = 1;
    save(&r, "title", 1);
    r.deathmatch.as_mut().unwrap().ticks = dm::TITLE + 1;
    r.deathmatch
        .as_mut()
        .unwrap()
        .fighters
        .get_mut(&1)
        .unwrap()
        .flash = 4;
    save(&r, "fire", 1);
    r.deathmatch
        .as_mut()
        .unwrap()
        .fighters
        .get_mut(&1)
        .unwrap()
        .respawn = dm::RESPAWN;
    r.deathmatch
        .as_mut()
        .unwrap()
        .fighters
        .get_mut(&1)
        .unwrap()
        .hero
        .hp = 0;
    save(&r, "respawn", 1);
    let s = r.deathmatch.as_mut().unwrap();
    s.fighters.get_mut(&2).unwrap().kills = 10;
    s.winner = Some(2);
    s.purse = 60;
    save(&r, "winner", 1);
    for kit in [
        "lynette",
        "gareth",
        "galahad",
        "hammerer",
        "butcher",
        "alchemist",
    ] {
        let mut r = scene();
        assert!(r.take_up(1, kit));
        let mut s = dm::Session::new(&r.players, &r.book);
        s.ticks = dm::TITLE + 1;
        let f = s.fighters.get_mut(&1).unwrap();
        (f.hero.x, f.hero.y) = (13., 23.);
        f.yaw = 0.;
        f.flash = 4;
        f.protection = 0;
        r.deathmatch = Some(s);
        save(&r, &format!("arm-{kit}"), 1);
    }
    let r = scene();
    let im = super::super::frame_for(&r, 768, 448, Some(1));
    let im = super::super::seat_lift_frame(&im, &r, Some(1));
    image::save_buffer(
        dir.join("browser-seat.png"),
        &im.rgba_bytes(),
        im.w as u32,
        im.h as u32,
        image::ColorType::Rgba8,
    )
    .unwrap();
    let start = std::time::Instant::now();
    for _ in 0..10 {
        for id in 1..=6 {
            std::hint::black_box(layers(&r, Some(id)));
        }
    }
    eprintln!(
        "six-seat render: {:.2} ms per seat",
        start.elapsed().as_secs_f64() * 1000. / 60.
    );
}
