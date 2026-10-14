use super::super::{frame, frame_for};
use super::*;
use crate::rules::{college, home::Home, world};
#[test]
fn college_frames_are_deterministic_animated_and_each_seat_has_its_own_view() {
    let mut r = Run::at_home(17, 1, Some("Scholar"), Home::default(), Default::default());
    r.come_to_the_gate();
    for kind in college::ROOMS {
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 10.5));
        r.tick = 83;
        let a = frame(&r, 384, 224);
        let ambient = scenery(&r);
        assert_eq!(a, frame(&r, 384, 224));
        assert!(a.pixels().all(|v| v == super::super::super::ink::BLACK
            || super::super::super::ink::palette_index(v).is_some()));
        r.tick += 5;
        assert_ne!(ambient, scenery(&r), "room ambience {kind:?}");
        assert!(a != frame(&r, 384, 224), "ambient animation {kind:?}");
        assert_eq!(
            (
                frame_for(&r, 384, 224, Some(2)).w,
                frame_for(&r, 384, 224, Some(2)).h
            ),
            (384, 224)
        );
    }
}
#[test]
fn college_labels_stay_clear_when_the_party_crowds_bartosz_or_the_walls() {
    let mut r = Run::at_home(17, 1, Some("Scholar"), Home::default(), Default::default());
    r.join_seat(3, "Researcher");
    r.join_seat(4, "Librarian");
    r.come_to_the_gate();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::WizardStudy).unwrap(),
        (12.0, 7.0),
    );
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (24.0, 14.0);
    assert_eq!(
        college::nearest(&r, &r.players[&1]).unwrap().0,
        college::Interaction::Wizard
    );
    let a = frame_for(&r, 384, 224, Some(1));
    for (&id, h) in r.players.iter_mut() {
        if id != 1 {
            (h.x, h.y) = (24.0, 12.0);
        }
    }
    let crowded = frame_for(&r, 384, 224, Some(1));
    assert_ne!(a, crowded);
    for y in 208..224 {
        for x in 0..384 {
            assert_eq!(
                a.get(x, y),
                crowded.get(x, y),
                "unobscured footer at {x},{y}"
            );
        }
    }
    for h in r.players.values_mut() {
        (h.x, h.y) = (24.0, 2.1);
    }
    let north = frame_for(&r, 384, 224, Some(1));
    for y in 0..16 {
        for x in 0..384 {
            assert_eq!(a.get(x, y), north.get(x, y));
        }
    }
    assert!(college::nearest(&r, &r.players[&1]).is_none());
    let h = r.players.get_mut(&3).unwrap();
    (h.x, h.y) = (24.0, 14.0);
    let shared = frame_for(&r, 384, 224, None);
    assert!(
        (208..224).any(|y| (0..384).any(|x| shared.get(x, y) != north.get(x, y))),
        "the shared camera shows the interaction beside another party member"
    );
}
#[test]
#[ignore = "writes every College room and gate-side map for visual review"]
fn write_college_shots() {
    use crate::render::overworld::{Scene, View, render_view};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, im: Img| {
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgba_scaled(3),
            im.w as u32 * 3,
            im.h as u32 * 3,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    let mut r = Run::at_home(17, 1, Some("Scholar"), Home::default(), Default::default());
    r.join_seat(3, "Researcher");
    r.join_seat(4, "Librarian");
    r.come_to_the_gate();
    r.tick = 83;
    save("gate", frame(&r, 384, 224));
    for (i, kind) in college::ROOMS.into_iter().enumerate() {
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 10.5));
        for _ in 0..60 {
            r.step(&Default::default());
        }
        save(
            &format!(
                "{:02}-{}",
                i + 1,
                college::label(kind)
                    .to_ascii_lowercase()
                    .replace([' ', '\''], "-")
            ),
            frame(&r, 384, 224),
        );
        r.tick += 24;
        save(&format!("ambient-{:02}", i + 1), frame(&r, 384, 224));
    }
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::WizardStudy).unwrap(),
        (12.0, 7.0),
    );
    for _ in 0..60 {
        r.step(&Default::default());
    }
    save("study-bartosz-prompt", frame(&r, 384, 224));
    r.college.owner = Some(1);
    r.college.page = college::Page::Quest(0);
    save("study-quest-dialogue", frame(&r, 384, 224));
    r.college.page = college::Page::Topics;
    save("study-topic-menu", frame(&r, 384, 224));
    r.college.page = college::Page::Topic(3);
    save("study-tunnel-lesson", frame(&r, 384, 224));
    r.college.page = college::Page::Bestiary;
    save("study-bestiary", frame(&r, 384, 224));
    r.home.college.bookshelf = true;
    r.home.college.stars = true;
    r.home.college.familiar = true;
    r.college.close();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::TowerEntry).unwrap(),
        (12.0, 10.5),
    );
    for _ in 0..60 {
        r.step(&Default::default());
    }
    save("tower-shelf-secret", frame(&r, 384, 224));
    for (name, kind, spot) in [
        ("library-book-prompt", RoomKind::CollegeLibrary, (12.0, 6.0)),
        (
            "study-familiar-prompt",
            RoomKind::WizardStudy,
            college::FAMILIAR,
        ),
        (
            "tower-shelf-prompt",
            RoomKind::TowerEntry,
            college::BOOKSHELF,
        ),
        ("hall-rug-prompt", RoomKind::CollegeHall, (12.0, 6.5)),
    ] {
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), spot);
        for _ in 0..60 {
            r.step(&Default::default());
        }
        save(name, frame(&r, 384, 224));
    }
    let lot = crate::render::overworld::college::lot();
    let gate = crate::render::overworld::wishes::delve_gate();
    let mut scene = Scene::resting();
    scene.tick = 83;
    scene.delve_lit = true;
    let view = View::around(
        ((lot.tx + gate.tx + lot.tw) / 2 * 16) as f32,
        ((lot.ty + gate.ty + lot.th) / 2 * 16) as f32,
        512,
        288,
    );
    save("map-college-and-delve", render_view(&scene, view));
}
