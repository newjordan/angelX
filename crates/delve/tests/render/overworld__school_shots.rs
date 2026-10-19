use super::*;
use crate::render::overworld::kingdom as geography;
use crate::rules::home::Home;
use crate::rules::school::{self, Rank, Standing};

fn kingdom() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Home::default(), Default::default());
    r.walk_kingdom(Some(geography::keep_stand()));
    r
}

/// Four frames for the operator's eye, in the ranch-shots style:
/// the school with its masters, a master's mission board, a padlocked
/// card on the floor and a signature gear piece worn. Never run in CI:
/// it writes PNGs when ANGEL_ARENA_SHOTS names a directory.
#[test]
#[ignore = "writes the school's frames for visual review"]
fn write_school_shots() {
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
    // 1. The kingdom's school: each class's master stands at its banner.
    let mut r = kingdom();
    r.tick = 40;
    save("school-masters", &frame(&r, 384, 224));

    // 2. A master's mission board, open over the kingdom.
    r.school.owner = Some(1);
    r.school.class = 0; // the Archer's board
    save("school-board", &frame(&r, 384, 224));

    // 3. A padlocked card on a delve floor: the Blood Drinker waits below
    //    the reader's rank.
    let mut d = Run::new(4, 1, Some("Friend"));
    d.book.insert(school::gear_cards().next().unwrap());
    let hero = d.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (12.0 * 24.0, 8.0 * 24.0);
    let card = "leech-fang".to_string();
    let (x, y) = (hero.x, hero.y + 24.0);
    let at = d.at;
    d.dungeon.rooms[at].items.push(crate::rules::Item {
        card,
        x,
        y,
        held_off: None, took: Vec::new(),
    });
    d.tick = 40;
    save("school-padlock", &frame(&d, 384, 224));

    // 4. The same knight a Master of the Archer, wearing the Rookery
    //    Longbow the master handed over.
    let key = Home::school_key(&d.players[&1]);
    d.home.school.knights.insert(
        key,
        Standing {
            rank: Rank::Master,
            ..Default::default()
        },
    );
    // The Archer's Master mission done: the handover path takes the bow up.
    let longbow = school::MISSIONS
        .iter()
        .find(|m| m.gear == Some("rookery-longbow"))
        .unwrap()
        .id;
    let standing = d
        .home
        .school
        .knights
        .entry(Home::school_key(&d.players[&1]))
        .or_default();
    standing.done.insert(longbow.to_string());
    standing.gear.clear();
    d.school.owner = Some(1);
    d.school_grant(1);
    d.tick = 44;
    save("school-gear-worn", &frame(&d, 384, 224));
}
