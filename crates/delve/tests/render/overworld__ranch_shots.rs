use super::super::frame;
use super::*;
use crate::rules::{home::Home, ranch, world};

fn run() -> Run {
    let mut r = Run::at_home(23, 1, Some("Rider"), Home::default(), Default::default());
    r.come_to_the_gate();
    r
}

#[test]
fn ranch_frames_are_deterministic_in_the_palette_and_alive() {
    let mut r = run();
    for kind in ranch::ROOMS {
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 10.5));
        r.tick = 83;
        let a = frame(&r, 384, 224);
        assert_eq!(a, frame(&r, 384, 224));
        assert!(a.pixels().all(|v| v == super::super::super::ink::BLACK
            || super::super::super::ink::palette_index(v).is_some()));
        r.tick += 7;
        assert!(a != frame(&r, 384, 224), "the {kind:?} moves");
    }
}

#[test]
fn helios_burns_only_at_the_gallop() {
    let ((_, _), _, gait) = ranch::helios(30);
    assert!(gait.is_some());
    let fire = draft(HELIOS, Pose::Gallop(1), 1.0, Some(30), false);
    let calm = draft(HELIOS, Pose::Gallop(1), 1.0, None, false);
    let ink = super::super::super::ink::ink;
    let white = |im: &Img| im.pixels().filter(|&p| Some(p) == ink('w') || Some(p) == ink('9')).count();
    let red = |im: &Img| im.pixels().filter(|&p| Some(p) == ink('7')).count();
    assert!(white(&fire) > white(&calm) + 20, "{} vs {}", white(&fire), white(&calm));
    assert_eq!(red(&fire), 0, "white fire, gold to white: never red");
}

#[test]
#[ignore = "writes the ranch, the fields and Helios for visual review"]
fn write_ranch_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, im: &Img| {
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgba_scaled(3),
            im.w as u32 * 3,
            im.h as u32 * 3,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    let mut r = run();
    r.join_seat(3, "Squire");
    r.arrive(world::room_of(&r.dungeon, RoomKind::Stables).unwrap(), (3.5, 9.5));
    r.tick = 40;
    save("01-stables-gate", &frame(&r, 384, 224));
    r.arrive(world::room_of(&r.dungeon, RoomKind::Ranch).unwrap(), (14.5, 5.5));
    r.tick = 60;
    save("02-ranch", &frame(&r, 384, 224));
    r.tick = 64;
    save("03-ranch-brushing", &frame(&r, 384, 224));
    r.arrive(world::room_of(&r.dungeon, RoomKind::Pasture).unwrap(), (12.0, 10.5));
    for t in [20u64, 60, 110, 160, 230] {
        r.tick = t;
        save(&format!("04-fields-{t:03}"), &frame(&r, 384, 224));
    }
    let mut home = r.home.clone();
    let dam = crate::realm::chivalry::Horse {
        name: "Cloud".into(), house: "qwen".into(), mare: true,
        blood: crate::realm::chivalry::bloodline("qwen"), ..Default::default()
    };
    home.stable.herd.push(dam.clone());
    home.stable.herd.push(ranch::helios_foal("Phaethon", false, 0, &dam));
    home.ranch.helios_bred = true;
    r.rebuild_home(home, Default::default());
    r.tick = 300;
    save("05-fields-foal", &frame(&r, 384, 224));
    r.arrive(world::room_of(&r.dungeon, RoomKind::Ranch).unwrap(), (14.5, 5.5));
    save("05-ranch-herd-foal", &frame(&r, 384, 224));
    r.arrive(world::room_of(&r.dungeon, RoomKind::Pasture).unwrap(), (12.0, 10.5));
    for (wins, name) in [(ranch::YEARLING, "05-fields-yearling"), (ranch::GROWN, "05-fields-grown")] {
        r.home.ranch.wins = wins;
        save(name, &frame(&r, 384, 224));
    }
    // Helios alone, large: standing and at the gallop in the sun.
    for (name, im) in [
        ("06-helios-stand", draft(HELIOS, Pose::Stand, 2.0, None, false)),
        ("07-helios-gallop-fire", draft(HELIOS, Pose::Gallop(1), 2.0, Some(30), false)),
        ("08-helios-gallop-fire-b", draft(HELIOS, Pose::Gallop(3), 2.0, Some(41), false)),
        ("09-barley-braided", draft(BARLEY, Pose::Stand, 2.0, None, true)),
    ] {
        let mut bg = Img::black(im.w + 8, im.h + 8);
        for y in 0..bg.h {
            for x in 0..bg.w {
                bg.put(x, y, 'm');
            }
        }
        bg.stamp(&im, 4, 4);
        save(name, &bg);
    }
}
