//! Explicit art exports, never part of the ordinary test run. The fixtures
//! exercise the persisted company and the real room/crowd renderer, not a
//! second drawing implementation. Output: `ANGEL_SQUIRES_SHOTS` (required).

use super::super::{frame, frame_for};
use super::*;
use crate::realm::Spoil;
use crate::rules::{
    home::Home,
    lane_squires::{Company, Roster},
    world,
};

fn recruit(id: u32, kind: Kind, level: u8) -> Recruit {
    Recruit {
        id,
        name: ["PIP", "MALLOW", "WREN", "OSWIN", "TANSY", "ASH"][id as usize % 6].into(),
        kind,
        level,
    }
}

fn company() -> Company {
    let mut company = Company::default();
    company.board = 3;
    company.hall = 3;
    company.yard = 2;
    company.offers = (0..6)
        .map(|n| recruit(n, Kind::ALL[n as usize % 5], 1 + n as u8 % 4))
        .collect();
    company
}

fn town() -> Run {
    let mut run = Run::at_home(23, 1, Some("ROWAN"), Home::default(), Default::default());
    run.come_to_the_gate();
    run.home.goblins = 1; // The eastern stall must remain intact.
    run.arrive(
        world::room_of(&run.dungeon, RoomKind::Yard).unwrap(),
        (12.0, 12.0),
    );
    run.home.lane_squires = company();
    let who = Home::knight_key(&run.players[&1]);
    let members: Vec<_> = (0..5)
        .map(|n| {
            recruit(
                10 + n,
                Kind::ALL[n as usize % 5],
                if n == 5 { 4 } else { 1 + n as u8 % 4 },
            )
        })
        .collect();
    let active = Some(members[0].id);
    let drills = members.iter().map(|r| (r.id, 3)).collect();
    run.home.lane_squires.knights.insert(
        who,
        Roster {
            members,
            active,
            drills,
        },
    );
    run.treasury.add(Spoil::Gold, 1250);
    run.tick = 84;
    run
}

fn move_to(run: &mut Run, action: Action) {
    let (_, (c, r, w, h)) = lane_squires::plates(&run.home.lane_squires)
        .into_iter()
        .find(|(a, _)| *a == action)
        .unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
}

fn save(name: &str, im: &Img) {
    let dir = std::env::var_os("ANGEL_SQUIRES_SHOTS")
        .map(std::path::PathBuf::from)
        .expect("set ANGEL_SQUIRES_SHOTS to the output directory");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.png"));
    image::save_buffer(
        &path,
        &im.rgba_scaled(3),
        im.w as u32 * 3,
        im.h as u32 * 3,
        image::ColorType::Rgba8,
    )
    .unwrap();
    println!("{} ({}x{}, 3x nearest pixels)", path.display(), im.w, im.h);
}

fn crop(im: &Img, (x, y, w, h): Rect) -> Img {
    let mut out = Img::black(w, h);
    out.stamp(im, -x, -y);
    out
}

fn checked_frame(run: &Run, focus: Option<u32>) -> Img {
    let a = frame_for(run, 384, 224, focus);
    assert_eq!(a, frame_for(run, 384, 224, focus), "same run, same pixels");
    assert!(a.pixels().all(|p| p == super::super::super::ink::BLACK
        || super::super::super::ink::palette_index(p).is_some()));
    a
}

#[test]
#[ignore = "exports squire yard services, offers, roster and training PNGs"]
fn write_squires_town_shots() {
    let mut run = town();
    let overview = checked_frame(&run, Some(1));
    save("01-yard-company", &overview);
    // A native service closeup includes all three signboards, offer figures
    // and upgrade plates. It stops before the existing quintain feet.
    let mut yard = super::super::scenery(&run);
    figures(&mut yard, &run, Some(1));
    save("02-yard-services-closeup", &crop(&yard, (24, 4, 336, 94)));
    for (building, name) in [
        (Building::Board, "board"),
        (Building::Hall, "hall"),
        (Building::Yard, "yard"),
    ] {
        move_to(&mut run, Action::Upgrade(building));
        save(&format!("03-service-{name}"), &checked_frame(&run, Some(1)));
    }
    for n in 0..run.home.lane_squires.offer_count() {
        move_to(&mut run, Action::Hire(n));
        save(&format!("04-offer-{n}"), &checked_frame(&run, Some(1)));
    }
    let who = Home::knight_key(&run.players[&1]);
    run.home
        .lane_squires
        .knights
        .get_mut(&who)
        .unwrap()
        .members
        .push(recruit(15, Kind::Ninja, 4));
    for n in 0..run.home.lane_squires.cap() {
        move_to(&mut run, Action::Select(n));
        save(&format!("05-roster-{n}"), &checked_frame(&run, Some(1)));
    }
    for n in 0..6 {
        let roster = run.home.lane_squires.knights.get_mut(&who).unwrap();
        roster.active = Some(roster.members[n].id);
        move_to(&mut run, Action::Train);
        save(
            &format!("06-training-{}-{n}", Kind::ALL[n % 5].name().to_lowercase()),
            &checked_frame(&run, Some(1)),
        );
    }
    // A second buyer's company must not leak into the focused player's UI.
    run.join_seat(2, "OTHER");
    let foreign = Home::knight_key(&run.players[&2]);
    run.home.lane_squires.knights.insert(
        foreign.clone(),
        Roster {
            members: vec![recruit(80, Kind::Shield, 1)],
            ..Default::default()
        },
    );
    let h = run.players.get_mut(&2).unwrap();
    (h.x, h.y) = (5.0, 7.0);
    let a = checked_frame(&run, Some(1));
    let mut ledger = Img::new(384, 224);
    boards(&mut ledger, &run, Some(1));
    run.home.lane_squires.knights.get_mut(&foreign).unwrap().members[0].name = "NOT YOUR COMPANY".into();
    let mut other_ledger = Img::new(384, 224);
    boards(&mut other_ledger, &run, Some(1));
    assert_eq!(
        ledger, other_ledger,
        "another roster cannot change this buyer's ledger"
    );
    save("07-personal-roster", &a);
    run.home
        .lane_squires
        .knights
        .get_mut(&who)
        .unwrap()
        .members
        .pop();
    run.treasury.0.insert(Spoil::Gold, 0);
    move_to(&mut run, Action::Hire(0));
    save("08-hire-no-gold", &checked_frame(&run, Some(1)));
    run.home.lane_squires.board = 0;
    run.home.lane_squires.hall = 0;
    run.home.lane_squires.yard = 0;
    run.home.lane_squires.knights.clear();
    move_to(&mut run, Action::Select(0));
    save("09-empty-roster", &checked_frame(&run, Some(1)));
    run.home.lane_squires.offers.truncate(3);
    move_to(&mut run, Action::Upgrade(Building::Board));
    save("09-starting-company", &checked_frame(&run, Some(1)));
}

#[test]
#[ignore = "exports six-knight dungeon, five companion roles, downed and smoke PNGs"]
fn write_squires_dungeon_shots() {
    let mut run = Run::new(23, 1, Some("ROWAN"));
    for (id, name) in [
        (2, "BRIAR"),
        (3, "CLOVER"),
        (4, "FLINT"),
        (5, "HAZEL"),
        (6, "RUE"),
    ] {
        run.join_seat(id, name);
    }
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.enemies.clear();
    run.projectiles.clear();
    run.tick = 84;
    run.lane_squires.clear();
    for (n, (&id, hero)) in run.players.iter_mut().enumerate() {
        let (x, y) = (9.0 + (n % 3) as f32 * 13.0, 9.0 + (n / 3) as f32 * 9.0);
        (hero.x, hero.y) = (x, y);
        hero.hp = hero.max_hp;
        let recruit = recruit(30 + n as u32, Kind::ALL[n % 5], 4);
        run.lane_squires.push(Squire {
            owner: id,
            hp: recruit.kind.health(recruit.level),
            recruit,
            x: x + 3.0,
            y: y + 1.5,
            cooldown: 0,
            grace: 0,
            smoke: 0,
        });
    }
    assert_eq!(run.players.len(), 6);
    assert_eq!(run.lane_squires.len(), 6);
    save(
        "10-dungeon-six-knights-five-roles",
        &checked_frame(&run, None),
    );
    run.lane_squires[0].smoke = 90;
    run.lane_squires[3].hp = 0;
    run.lane_squires[4].hp = 19;
    save(
        "11-dungeon-smoke-downed-wounded",
        &checked_frame(&run, None),
    );
    save("12-dungeon-personal-view", &checked_frame(&run, Some(1)));
    run.tick += 12;
    save("13-dungeon-smoke-next-frame", &checked_frame(&run, None));
    // Native-size contact sheet makes role and livery differences reviewable
    // without asking the reviewer to zoom a whole dungeon frame.
    let mut sheet = Img::black(384, 120);
    for (n, kind) in Kind::ALL.into_iter().enumerate() {
        let x = 10 + n as i32 * 75;
        sheet.text(x, 5, kind.name(), 'H');
        for owner in 1..=6 {
            let im = sprite(&recruit(90, kind, 4), owner, false, run.tick as u32);
            stand(
                &mut sheet,
                &im,
                x + 8 + ((owner - 1) % 3) as i32 * 21,
                32 + ((owner - 1) / 3) as i32 * 22,
            );
        }
        let down = sprite(&recruit(90, kind, 4), 1, true, 0);
        stand(&mut sheet, &down, x + 12, 74);
    }
    let sprites: Vec<_> = Kind::ALL
        .into_iter()
        .map(|kind| sprite(&recruit(90, kind, 4), 1, false, 0))
        .collect();
    for i in 0..sprites.len() {
        for j in i + 1..sprites.len() {
            assert_ne!(sprites[i], sprites[j], "role silhouettes differ");
        }
    }
    assert_ne!(
        sprite(&recruit(90, Kind::Ranger, 4), 1, false, 0),
        sprite(&recruit(90, Kind::Ranger, 4), 2, false, 0)
    );
    sheet.text(10, 95, "SIX LIVERIES / FIVE ROLES / RESTING BELOW", 'h');
    save("14-companion-contact-sheet", &sheet);
    assert_eq!(frame(&run, 384, 224).w, 384);
}
