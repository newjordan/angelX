use super::*;
use crate::realm::{Spoil, Spoils};
use crate::rules::{
    hireling::{AllyKind, Hireling},
    home::Home,
    countryside_squires::{Companion, Company},
};

#[test]
fn five_types_have_distinct_native_figures_and_owner_colours() {
    let a = figure(Kind::Ninja, '7');
    for kind in Kind::ALL.into_iter().skip(1) {
        assert_ne!(a, figure(kind, '7'));
    }
    assert_ne!(a, figure(Kind::Ninja, 'A'));
}

#[test]
#[ignore = "exports squire yard and six-knight PNGs for visual review"]
fn write_squire_shots() {
    let dir =
        std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").expect("ANGEL_ARENA_SHOTS"));
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
    let mut gold = Spoils::default();
    gold.add(Spoil::Gold, 5000);
    let mut home = Home::default();
    home.countryside.squires.board = 3;
    let mut run = Run::at_home(3, 1, None, home, gold);
    let at = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == crate::rules::RoomKind::Yard)
        .unwrap();
    run.at = at;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = PLATES[0].0 * 2.0;
    hero.y = PLATES[0].1 * 2.0;
    save(
        "yard-recruits",
        crate::render::overworld::arena::seat_scene(&run, 640, 360, Some(1)).compose(),
    );
    let h = run.players.get_mut(&1).unwrap();
    h.x = PLATES[8].0 * 2.0;
    h.y = PLATES[8].1 * 2.0;
    save(
        "yard-buildings",
        crate::render::overworld::arena::seat_scene(&run, 640, 360, Some(1)).compose(),
    );
    let mut run = Run::new(7, 1, Some("Pip"));
    for id in 3..=6 {
        run.join_seat(id, &format!("Seat {id}"));
    }
    let mut company = Company::default();
    for (&id, h) in &mut run.players {
        h.x = 14.0 + id as f32 * 4.0;
        h.y = 15.0;
        let s = Squire {
            id,
            name: format!("Squire {id}"),
            kind: Kind::ALL[(id - 1) as usize % 5],
            level: 3,
            drills: 0,
        };
        let hp = s.health();
        let body = Hireling {
            kind: AllyKind::Beaumains,
            x: h.x,
            y: h.y + 3.0,
            hp,
            max_hp: hp,
            invulnerable: 0,
            cooldown: 0,
            facing: 1.0,
            side: 1.0,
            summoner: Some(id),
            power: 0,
            taunt: 0,
            rising: 0,
            made: None,
        };
        run.countryside_squires.push(Companion {
            owner: id,
            recruit: s.clone(),
            body,
            room: run.at,
            depth: 1,
            ability: 0,
            smoke: 0,
            trick: 0,
            marked: None,
        });
        company.roster.push(s);
    }
    save(
        "six-knights-five-types",
        crate::render::overworld::arena::seat_scene(&run, 640, 360, Some(1)).compose(),
    );
    run.countryside_squires[0].body.hp = 0;
    save(
        "downed-squire",
        crate::render::overworld::arena::seat_scene(&run, 640, 360, Some(1)).compose(),
    );
}
