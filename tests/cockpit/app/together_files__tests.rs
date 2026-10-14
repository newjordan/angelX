//! The Delve's files read from the host's disk.
use crate::drive::together_shooter::Run;

use super::*;
use crate::drive::together_shooter::cards::{self, check};
use crate::drive::together_shooter::looks::PAVISE;
use crate::drive::together_shooter::phrasebook;

#[test]
fn spell_files_hot_load_once_into_first_empty_host_slot_with_flourish() {
    let dir = std::env::temp_dir().join(format!("angel-spell-cards-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::new(19, 1, Some("friend"));
    let raw = "name Spark\nkind spell\ncooldown 3\ncast bolt\n";
    std::fs::write(dir.join("spark.card"), "name incomplete\nkind spell\n").unwrap();
    assert_eq!(hot_load_spells(&mut run, &dir), 0);
    std::fs::write(dir.join("spark.card"), raw).unwrap();
    assert_eq!(hot_load_spells(&mut run, &dir), 1);
    assert_eq!(run.players[&1].spells[0].as_deref(), Some("spark"));
    assert_eq!(run.players[&2].spells, [None, None, None, None]);
    assert_eq!(run.players[&1].hand.len(), 0);
    assert_eq!(run.blasts.len(), 1);
    assert!(run.sounds.contains(&"card_pickup"));
    assert!(run.found_line().unwrap().contains("Spark"));
    assert_eq!(
        hot_load_spells(&mut run, &dir),
        0,
        "unchanged file must not flourish twice"
    );
    run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 25;
    std::fs::write(
        dir.join("spark.card"),
        raw.replace("cooldown 3", "cooldown 4"),
    )
    .unwrap();
    assert_eq!(hot_load_spells(&mut run, &dir), 1);
    assert_eq!(
        run.players[&1].spell_cooldowns[0], 25,
        "editing cannot reset recharge"
    );
    assert_eq!(run.blasts.len(), 1);
    for id in ["a", "b", "c", "d"] {
        std::fs::write(dir.join(format!("{id}.card")), raw).unwrap();
    }
    assert_eq!(hot_load_spells(&mut run, &dir), 4);
    assert_eq!(run.players[&1].spells[1].as_deref(), Some("a"));
    assert_eq!(run.players[&1].spells[2].as_deref(), Some("b"));
    assert_eq!(run.players[&1].spells[3].as_deref(), Some("c"));
    assert!(run.room().items.iter().any(|i| i.card == "d"));
    assert_eq!(hot_load_spells(&mut run, &dir), 0);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_missing_picture_rejects_only_its_entry_and_a_later_picture_can_load() {
    let dir = std::env::temp_dir().join(format!("wish-picture-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let raw = "wish devil | Devil\nkind spell\nsay golden devil\ntier summon 1 swarm; cooldown 4 :: A devil.\nsheet foe missing.png frames=2\n\nwish fine | Fine\nsay fine\ntier damage 5 :: Fine.\n";
    let (text, errors) = phrasebook_with_sheets(raw, &dir);
    assert_eq!(
        phrasebook::read(&text)
            .iter()
            .map(|w| w.id.as_str())
            .collect::<Vec<_>>(),
        ["fine"]
    );
    assert!(errors.join(" ").contains("wish devil: line"));
    std::fs::write(
        dir.join("missing.png"),
        include_bytes!("../../../cockpit/fixtures/sheets/golden-devils.png"),
    )
    .unwrap();
    let (text, errors) = phrasebook_with_sheets(raw, &dir);
    assert!(errors.is_empty(), "{errors:?}");
    let book = phrasebook::read(&text);
    assert_eq!(book.len(), 2);
    assert_eq!(
        book[0].card(0).unwrap().looks.summon.unwrap().frames.len(),
        2
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// Wish plates: `ANGEL_ARENA_SHOTS=<dir> cargo test write_wish_look_shots -- --ignored`.
/// A knight in a big spiked pavise beside one in Iron Skin's spiked
/// pauldrons; flaming skulls for morningstars; venom comets for arrows; a
/// swarm of golden devils cut from the fixture sheet. Each as the arena
/// draws it, and a close-up.
#[test]
#[ignore]
fn write_wish_look_shots() {
    use crate::drive::together_shooter::{Input, RoomKind};
    use crate::stage::world_viz::overworld::arena;
    use std::collections::BTreeMap;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &mut Run, name: &str, around: &[u32]| {
        // No wish window's sign over the knights.
        run.window = None;
        for hero in run.players.values_mut() {
            hero.invulnerable = 0;
        }
        let run = &*run;
        let img = arena::frame(run, arena::NATIVE_W, arena::NATIVE_H);
        let (ox, oy) = arena::camera(run, None);
        let write = |img: &crate::stage::world_viz::overworld::Img, k: u32, file: String| {
            let mut out = image::RgbImage::new(img.w as u32 * k, img.h as u32 * k);
            for (x, y, px) in out.enumerate_pixels_mut() {
                let c = img.get((x / k) as i32, (y / k) as i32).unwrap_or([0, 0, 0]);
                *px = image::Rgb(c);
            }
            out.save(dir.join(file)).unwrap();
        };
        write(&img, 2, format!("{name}.png"));
        // A close-up round the knights named.
        let mut heroes: Vec<(i32, i32)> = around
            .iter()
            .map(|id| {
                let h = &run.players[id];
                ((h.x * 8.0) as i32 - ox, (h.y * 8.0) as i32 - oy)
            })
            .collect();
        let screen = |x: f32, y: f32| ((x * 8.0) as i32 - ox, (y * 8.0) as i32 - oy);
        heroes.extend(
            run.projectiles
                .iter()
                .filter(|p| p.sprite_card().is_some())
                .map(|p| screen(p.x, p.y)),
        );
        heroes.extend(
            run.imps
                .iter()
                .filter(|p| around.contains(&p.owner))
                .map(|p| screen(p.x, p.y)),
        );
        for id in around {
            heroes.extend(run.orbits(*id).into_iter().map(|(x, y)| screen(x, y)));
        }
        let (x0, x1) = (
            heroes.iter().map(|h| h.0).min().unwrap() - 40,
            heroes.iter().map(|h| h.0).max().unwrap() + 40,
        );
        let (y0, y1) = (
            heroes.iter().map(|h| h.1).min().unwrap() - 36,
            heroes.iter().map(|h| h.1).max().unwrap() + 16,
        );
        let crop =
            crate::stage::world_viz::overworld::Img::from_view(&img, x0, y0, x1 - x0, y1 - y0);
        write(&crop, 6, format!("{name}-close.png"));
    };
    let room = |run: &mut Run| {
        let room = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Fight && !r.great())
            .unwrap();
        run.enter_for_test(room);
        run.calm_for_test();
        run.enemies.clear();
        run.window = None;
    };
    let give = |run: &mut Run, id: u32, card: cards::Card| {
        run.book.insert(card.clone());
        let book = run.book.clone();
        let hero = run.players.get_mut(&id).unwrap();
        hero.deck.push(card.id);
        hero.rebonus(&book);
    };

    // Worn: the pavise, and Iron Skin III's spiked pauldrons.
    let mut run = Run::new(9, 1, None);
    run.join("Friend");
    room(&mut run);
    let pavise = check(
        "spiked-pavise",
        &format!("name Spiked Pavise\nkind hold\narmor 1\ndraw worn at=left\n{PAVISE}\n"),
    )
    .unwrap()
    .card;
    give(&mut run, 1, pavise);
    let iron = crate::drive::together_shooter::phrasebook::get("iron")
        .unwrap()
        .card(2)
        .unwrap();
    give(&mut run, 2, iron);
    for (id, x) in [(1, 21.0), (2, 27.0)] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.aim_x, hero.aim_y) = (x, 14.0, 1.0, 0.0);
        hero.hp = hero.max_hp;
    }
    save(&mut run, "worn", &[1, 2]);

    // Orbit: flaming skulls.
    let mut run = Run::new(9, 1, None);
    room(&mut run);
    let skulls = check(
        "skull-ring",
        &format!("name Skull Ring\nkind hold\norbit 2\ndraw orbit\n{SKULL}\n"),
    )
    .unwrap()
    .card;
    give(&mut run, 1, skulls);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    for _ in 0..4 {
        run.step(&BTreeMap::new());
    }
    save(&mut run, "orbit", &[1]);

    // Shots: venom comets from a plain bow.
    let mut run = Run::new(9, 1, None);
    room(&mut run);
    let comets = check(
        "venom-comets",
        &format!("name Venom Comets\nkind hold\ndamage 5\ndraw shot\n{COMET}\n"),
    )
    .unwrap()
    .card;
    give(&mut run, 1, comets);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (16.0, 14.0);
    let fire = Input {
        fire: true,
        aim_x: 1,
        aim_y: 0,
        ..Default::default()
    };
    for _ in 0..26 {
        run.step(&BTreeMap::from([(1, fire)]));
    }
    assert!(
        run.projectiles
            .iter()
            .any(|p| p.sprite_card() == Some("venom-comets")),
        "the bow's shots wear the comet"
    );
    save(&mut run, "shot", &[1]);

    // Summons: golden devils, cut from the fixture sheet.
    let ws = std::env::temp_dir().join(format!("delve-devils-{}", std::process::id()));
    std::fs::create_dir_all(ws.join(".angel/dungeon/sheets")).unwrap();
    std::fs::create_dir_all(ws.join(".angel/dungeon/cards")).unwrap();
    std::fs::write(
        ws.join(".angel/dungeon/sheets/golden-devils.png"),
        include_bytes!("../../../cockpit/fixtures/sheets/golden-devils.png"),
    )
    .unwrap();
    std::fs::write(
        ws.join(".angel/dungeon/cards/golden-devils.card"),
        "name Golden Devils\nkind spell\ncooldown 16\nsummon 4 swarm\nsheet summon golden-devils.png frames=2\n",
    )
    .unwrap();
    let mut run = Run::new(9, 1, None);
    room(&mut run);
    let report = load_cards(&ws.join(".angel/dungeon/cards"), &mut run.book);
    assert!(run.book.get("golden-devils").is_some(), "{report:?}");
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (18.0, 14.0);
    hero.spells[0] = Some("golden-devils".into());
    for (x, y) in [(31.0, 11.0), (32.0, 16.0), (29.0, 19.0)] {
        run.spawn_at_for_test(crate::drive::together_shooter::EnemyKind::Dummy, x, y);
    }
    for enemy in &mut run.enemies {
        enemy.age = crate::drive::together_shooter::TELEGRAPH + 5;
    }
    run.step(&BTreeMap::from([(
        1,
        Input {
            cast: 1,
            ..Default::default()
        },
    )]));
    for _ in 0..12 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.imps.len(), 4);
    save(&mut run, "swarm", &[1]);
    for _ in 0..30 {
        run.step(&BTreeMap::new());
        if run.imps.len() < 4 {
            break;
        }
    }
    save(&mut run, "swarm-burst", &[1]);
    let _ = std::fs::remove_dir_all(&ws);
}

/// A flaming skull that spins round the knight in place of a morningstar.
const SKULL: &str = "\
..7.@.7..|.7..@..7.
.7@6@6@7.|..7@6@7..
..HHHHH..|..HHHHH..
.HHHHHHH.|.HHHHHHH.
.HkkHkkH.|.HkkHkkH.
.Hk8Hk8H.|.HkkHkkH.
..HHkHH..|..HHkHH..
..HiHiH..|..HiHiH..
...iii...|...iii...";

/// A venom comet, facing the way it flies.
const COMET: &str = "\
....yY.|...yY..
.myYCYy|m.yYCYy
mLyCwCY|.LyCwCY
.myYCYy|m.yYCYy
....yY.|...yY..";
