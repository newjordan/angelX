use super::*;
use crate::render::overworld::arena;
use crate::rules::phrasebook;

/// A knight on floor one holding wishes, two of them granted, with
/// `volley` asked for on the scroll.
fn wishing(great: u8) -> Run {
    let mut run = Run::new(4, 2, Some("Matt"));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.wishes, hero.great_wishes) = (4, great);
    for id in ["fork", "iron"] {
        if let Some(wish) = phrasebook::get(id) {
            let _ = run.grant(1, &wish);
        }
    }
    run.genie.freedom = 38;
    let volley = phrasebook::get("volley").unwrap();
    run.offer_wish(1, "triple volley", Some(&volley)).unwrap();
    run
}

/// A seat's screen as its client draws it: the room's view with the
/// scroll over it, the HUD's band under it.
fn seat(run: &Run, w: i32, h: i32) -> Img {
    arena::in_seat(|| {
        let mut img = arena::frame_for(run, w, arena::view_rows(h), Some(1));
        paint(&mut img, run, Some(1));
        let map = arena::MapView { full: false, model: None, party: None };
        arena::seat_lift_frame_with(&img, run, Some(1), &map)
    })
}

fn changed(a: &Img, b: &Img) -> usize {
    (0..a.h).flat_map(|y| (0..a.w).map(move |x| (x, y))).filter(|&(x, y)| a.get(x, y) != b.get(x, y)).count()
}

#[test]
fn the_scroll_shows_only_for_its_own_knight_and_never_on_a_terminals_cells() {
    let mut run = wishing(0);
    run.join_seat(2, "Ada");
    let h = arena::view_rows(448);
    let bare = arena::frame_for(&run, 768, h, Some(1));
    let mut mine = bare.clone();
    paint(&mut mine, &run, Some(1));
    assert!(changed(&bare, &mine) > 20_000, "the scroll and the genie are drawn");
    let bare2 = arena::frame_for(&run, 768, h, Some(2));
    let mut theirs = bare2.clone();
    paint(&mut theirs, &run, Some(2));
    assert_eq!(changed(&bare2, &theirs), 0, "a friend's view is their own");
    let mut cells = arena::frame_for(&run, 120, 60, Some(1));
    let before = cells.clone();
    paint(&mut cells, &run, Some(1));
    assert_eq!(cells, before, "a terminal's cells draw their own scroll");
    // The scroll is laid out to the view, so it never runs off it.
    for (w, h) in [(768, arena::view_rows(448)), (384, arena::view_rows(224))] {
        let k = if w >= 768 { 2 } else { 1 };
        let hero = &run.players[&1];
        let body = Body::Offer(hero.offer.as_ref().unwrap());
        assert!(scroll(&run, hero, &body, h / k).h <= h / k, "{w}x{h}");
    }
}

#[test]
fn the_freeing_is_shown_to_the_whole_party() {
    let mut run = wishing(1);
    run.join_seat(2, "Ada");
    run.drop_offer(1);
    run.genie.freedom = genie::FREEDOM;
    run.offer_wish(1, genie::FREE_WORDS, None).unwrap();
    run.choose(1, 0).unwrap();
    let h = arena::view_rows(448);
    for id in [1, 2] {
        let bare = arena::frame_for(&run, 768, h, Some(id));
        let mut seen = bare.clone();
        paint(&mut seen, &run, Some(id));
        assert!(changed(&bare, &seen) > 10_000, "knight {id} sees him freed");
    }
}

#[test]
fn a_wish_that_lands_mid_play_brings_no_scroll_over_the_view() {
    // Sealed from the scroll: its last look stays up a moment.
    let mut run = wishing(0);
    run.drop_offer(1);
    let volley = phrasebook::get("volley").unwrap();
    run.sealed(1, &volley, false, true);
    let h = arena::view_rows(448);
    let bare = arena::frame_for(&run, 768, h, Some(1));
    let mut seen = bare.clone();
    paint(&mut seen, &run, Some(1));
    assert!(changed(&bare, &seen) > 10_000, "the scroll's last look");
    // Landed later, while they played on: nothing darkens their view.
    run.sealed_quietly(1);
    let mut seen = bare.clone();
    paint(&mut seen, &run, Some(1));
    assert_eq!(changed(&bare, &seen), 0, "no scroll over a knight mid-fight");
}

#[test]
fn the_painted_genie_is_embedded_at_the_size_drawn() {
    let a = art(true);
    for strip in [&a.idle, &a.talk, &a.cheer] {
        assert_eq!(strip.len(), 4);
        assert!(strip.iter().all(|f| f.h == strip[0].h && f.w == strip[0].w && f.h <= 140));
    }
    assert_eq!(art(false).idle[0].h, a.idle[0].h / 2);
}

/// `ANGEL_WISH_SCROLL_SHOTS=DIR`: the scroll as a window and a browser draw
/// it (768 wide) and as a 384-wide seat: the choices, a super wish's, the
/// sealing with the genie's cheer, and the freeing.
#[test]
#[ignore]
fn write_wish_scroll_shots() {
    let Some(dir) = std::env::var_os("ANGEL_WISH_SCROLL_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, im: &Img| {
        image::save_buffer(dir.join(name), &im.rgb_bytes(), im.w as u32, im.h as u32, image::ColorType::Rgb8).unwrap();
    };
    let mut run = wishing(0);
    for t in [0, 7] {
        run.tick += t;
        save(&format!("scroll-choices-{t}.png"), &seat(&run, 768, 448));
    }
    save("scroll-choices-384.png", &seat(&run, 384, 224));
    let great = wishing(1);
    save("scroll-super.png", &seat(&great, 768, 448));
    // Sealed: the bolder take, the genie pleased.
    let mut sealed = wishing(0);
    sealed.choose(1, 1).unwrap();
    if let Some(last) = sealed.genie.last.as_mut() {
        last.cheer = true;
    }
    sealed.tick += 2;
    save("scroll-sealed.png", &seat(&sealed, 768, 448));
    let mut free = wishing(1);
    free.drop_offer(1);
    free.genie.freedom = genie::FREEDOM;
    free.offer_wish(1, genie::FREE_WORDS, None).unwrap();
    save("scroll-free-offer.png", &seat(&free, 768, 448));
    free.choose(1, 0).unwrap();
    free.tick += 40;
    save("scroll-freed.png", &seat(&free, 768, 448));
    // Frame by frame, a folder each: the scroll opening as each kind of
    // wish, idling a while, and a wish sealed to the last roll.
    let film = |name: &str, run: &Run, ticks: &[u64]| {
        let sub = dir.join(name);
        std::fs::create_dir_all(&sub).unwrap();
        for &t in ticks {
            let mut at = run.clone();
            at.tick += t;
            let im = seat(&at, 768, 448);
            image::save_buffer(sub.join(format!("{t:03}.png")), &im.rgb_bytes(), im.w as u32, im.h as u32, image::ColorType::Rgb8).unwrap();
        }
    };
    let opening: Vec<u64> = (0..=20).step_by(2).collect();
    film("open-plain", &wishing(0), &opening);
    film("open-super", &wishing(1), &opening);
    let mut fae = wishing(0);
    fae.players.get_mut(&1).unwrap().fairy_wishes = 1;
    film("open-fairy", &fae, &opening);
    let mut freedom = wishing(0);
    if let Some(h) = freedom.players.get_mut(&1) {
        h.freedom_wishes = 1;
        h.place_wish = Some(crate::rules::freedom::Place::Safe);
    }
    film("open-freedom", &freedom, &opening);
    film("idle-plain", &wishing(0), &(20..=140).step_by(6).collect::<Vec<_>>());
    let mut sealing = wishing(0);
    sealing.choose(1, 1).unwrap();
    if let Some(last) = sealing.genie.last.as_mut() {
        last.cheer = true;
    }
    film("sealed", &sealing, &(0..=88).step_by(4).collect::<Vec<_>>());
}
