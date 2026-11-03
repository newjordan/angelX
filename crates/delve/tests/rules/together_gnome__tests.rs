//! The Gnome's Garden: the third open floor hangs it, the herbs and the
//! mushrooms give once a knight, the party sends four waves at the cellar
//! door (not at the knights), and a held house pays every knight a fairy
//! wish that lands at its third tier, Legendary.
use super::*;
use crate::rules::{layout, Pack, Rng};

fn garden(seats: u32) -> Run {
    let mut run = Run::new(41, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.enemies.clear();
    run.dungeon.rooms.push(nooks::room(RoomKind::GnomeParty, (12, 12), 3));
    let index = run.dungeon.rooms.len() - 1;
    run.nook = Some(Nook { room: index, ..Default::default() });
    run.enter_for_test(index);
    run.enemies.clear();
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    run
}

fn put(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

fn press(run: &mut Run, id: u32) {
    run.step(&BTreeMap::from([(id, Input { interact: true, ..Default::default() })]));
    run.step(&BTreeMap::new());
}

fn yard(run: &Run) -> &Garden {
    run.nook.as_ref().unwrap().garden.as_ref().unwrap()
}

#[test]
fn the_third_open_floor_hangs_the_garden_behind_a_door() {
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Fungal, Pack::Archive] {
        for seed in 0..6u64 {
            let mut rng = Rng::new(311 + seed * 17);
            let floor = layout::open::open_floor(DEPTH, pack, 4, &mut rng);
            let gardens = floor.rooms.iter().filter(|r| r.kind == RoomKind::GnomeParty).count();
            assert_eq!(gardens, 1, "{pack:?} seed {seed}");
            let other = layout::open::open_floor(DEPTH - 1, pack, 4, &mut rng);
            assert!(other.rooms.iter().all(|r| r.kind != RoomKind::GnomeParty));
        }
    }
}

#[test]
fn herbs_and_mushrooms_give_once_and_the_mushrooms_go_strange() {
    let mut run = garden(1);
    put(&mut run, 1, HERBS);
    press(&mut run, 1);
    assert_eq!(run.players[&1].boon.rate, HERB_RATE);
    press(&mut run, 1);
    assert_eq!(yard(&run).took[&1], 1, "once a knight");
    assert!(!run.tripping(1));
    put(&mut run, 1, SHROOMS);
    press(&mut run, 1);
    assert_eq!(run.players[&1].boon.damage, SHROOM_DAMAGE);
    assert!(run.tripping(1));
    for _ in 0..SHROOM_TICKS {
        run.step(&BTreeMap::new());
    }
    assert!(!run.tripping(1), "it wears off");
    assert!(run.valid_snapshot());
}

#[test]
fn the_guests_make_for_the_cellar_door_not_the_knights() {
    let mut run = garden(2);
    put(&mut run, 1, GNOME);
    put(&mut run, 2, (20.0, 11.0));
    run.players.get_mut(&1).unwrap().hp = 10;
    press(&mut run, 1);
    assert_eq!(yard(&run).party, Party::Holding);
    assert_eq!(run.players[&1].hp, run.players[&1].max_hp, "staying a while mends");
    assert!(run.tripping(1) && run.tripping(2), "the whole garden goes strange");
    for _ in 0..FIRST_WAVE + SPAWN_EVERY * 2 {
        run.step(&BTreeMap::new());
    }
    let guests = yard(&run).guests.clone();
    assert!(!guests.is_empty(), "the first wave comes through the gates");
    let (door, lured) = run.garden_lure().unwrap();
    assert_eq!(lured, guests);
    let id = guests[0];
    let e = run.enemies.iter().find(|e| e.id == id).unwrap().clone();
    assert_eq!(press::warning_target(&run, &e), Some(door), "a guest sees only the door");
    let recruit = crate::rules::countryside_squires::Squire {
        id: 1, name: "Kit".into(), kind: crate::rules::countryside_squires::Kind::Shieldbearer,
        level: 1, drills: 0,
    };
    let mut body = crate::rules::hireling::Hireling::minion(
        crate::rules::hireling::AllyKind::Beaumains, 0, 1, (e.x, e.y), 1,
    );
    body.taunt = 1;
    run.countryside_squires.push(crate::rules::countryside_squires::Companion {
        owner: 1, recruit, body, room: run.at, depth: 1,
        ability: 0, smoke: 0, trick: 0, marked: None,
    });
    assert_eq!(press::warning_target(&run, &e), Some(door), "a guest ignores a squire's roar");
    let mut ordinary = e.clone();
    ordinary.id = u32::MAX;
    assert_eq!(press::warning_target(&run, &ordinary), Some((e.x, e.y)), "other foes heed the squire");
    run.countryside_squires.clear();
    // One at the door is in, and the house pays for it.
    let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
    (e.x, e.y) = door;
    run.step(&BTreeMap::new());
    assert_eq!(yard(&run).house, HOUSE_HP - BITE);
    assert!(!run.enemies.iter().any(|e| e.id == id));
    assert!(run.valid_snapshot());
}

#[test]
fn four_waves_held_pay_every_knight_a_fairy_wish() {
    let mut run = garden(2);
    put(&mut run, 1, GNOME);
    press(&mut run, 1);
    let mut waves = 0;
    for _ in 0..HZ * 400 {
        // The knights hold the door: every guest falls where it comes in.
        let guests = yard(&run).guests.clone();
        for e in run.enemies.iter_mut().filter(|e| guests.contains(&e.id)) {
            e.hp = 0;
        }
        run.step(&BTreeMap::new());
        waves = waves.max(yard(&run).wave);
        if yard(&run).party != Party::Holding {
            break;
        }
    }
    assert_eq!(yard(&run).party, Party::Held);
    assert_eq!(usize::from(waves) + 1, WAVES.len());
    assert!(run.players.values().all(|h| h.fairy_wishes == 1));
    assert!(run.can_wish(1));
    assert!(!run.tripping(1), "the party's over, the strangeness with it");
    assert!(run.valid_snapshot());
}

#[test]
fn the_house_falls_and_the_party_is_lost() {
    let mut run = garden(1);
    put(&mut run, 1, GNOME);
    press(&mut run, 1);
    for _ in 0..HZ * 120 {
        let guests = yard(&run).guests.clone();
        let door = (DOOR.0 * TILE_UNITS, DOOR.1 * TILE_UNITS);
        for e in run.enemies.iter_mut().filter(|e| guests.contains(&e.id)) {
            (e.x, e.y) = door;
        }
        run.step(&BTreeMap::new());
        if yard(&run).party != Party::Holding {
            break;
        }
    }
    assert_eq!(yard(&run).party, Party::Lost);
    assert_eq!(yard(&run).house, 0);
    assert_eq!(run.players[&1].fairy_wishes, 0);
}

#[test]
fn a_fairy_wish_lands_whole_at_its_third_tier_and_legendary() {
    let mut run = garden(1);
    let wish = phrasebook::get("breastplate").unwrap();
    assert!(wish.tiers.len() >= FAIRY_STEPS as usize);
    let plain = wish.card(2).unwrap();
    let fairy = wish.card_graded(2, artistry::Grade::Legendary).unwrap();
    assert_ne!(plain.effects, fairy.effects, "Legendary numbers are the stronger");
    let hero = run.players.get_mut(&1).unwrap();
    (hero.wishes, hero.great_wishes, hero.fairy_wishes) = (2, 0, 1);
    run.fairy_grant = Some(1);
    for _ in 0..FAIRY_STEPS {
        run.grant(1, &wish).unwrap();
    }
    run.fairy_grant = None;
    assert!(run.spend_fairy_or_wish(1, true).is_some());
    let hero = &run.players[&1];
    assert!(hero.deck.contains(&wish.card_id(2)));
    assert_eq!((hero.wishes, hero.fairy_wishes), (2, 0), "the fairy wish is spent, not a plain one");
    assert_eq!(run.book.get(&wish.card_id(2)).unwrap().effects, fairy.effects);
}

fn yard_mut(run: &mut Run) -> &mut Garden {
    run.nook.as_mut().unwrap().garden.as_mut().unwrap()
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_garden_shots -- --ignored`:
/// a folder a scene, a PNG a frame at twice the room's size. The garden
/// quiet with knights by the blue folk; the herbs taken; the mushrooms
/// taken, the room going strange (and as a calm screen sees it); the
/// party starting and at its height with every kind of guest, as it
/// looks and as the tripping knights see it; a bite at the hatch from
/// each kind; the house near gone; the house held; the house lost; and
/// the Lawn Flamingo.
#[test]
#[ignore]
fn write_garden_shots() {
    use crate::render::viewer::{self, Preferences};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir).join("garden");
    let film = |run: &Run, scene: &str, n: u32, trip: bool| {
        let sub = dir.join(scene);
        std::fs::create_dir_all(&sub).unwrap();
        let mut img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        if trip {
            viewer::trip(&mut img, run.tick);
        }
        let rgba = img.rgba_scaled(1);
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, rgba).unwrap().save(sub.join(format!("{n:03}.png"))).unwrap();
    };
    let idle = |run: &mut Run| run.step(&BTreeMap::new());
    // Quiet: a knight up close to two of the blue folk.
    let mut run = garden(3);
    put(&mut run, 1, (12.0, 9.0));
    put(&mut run, 2, (8.2, 5.8));
    put(&mut run, 3, (18.5, 8.0));
    for t in 0..60 {
        if t % 2 == 0 {
            film(&run, "a-quiet", t, false);
        }
        idle(&mut run);
    }
    // The herbs, then the mushrooms.
    put(&mut run, 1, HERBS);
    press(&mut run, 1);
    for t in 0..72 {
        if t % 2 == 0 {
            film(&run, "b-herbs", t, false);
        }
        idle(&mut run);
    }
    put(&mut run, 1, SHROOMS);
    press(&mut run, 1);
    for t in 0..96 {
        if t % 2 == 0 {
            film(&run, "c-shrooms", t, run.tripping(1));
        }
        idle(&mut run);
    }
    for t in 0..48 {
        if t % 2 == 0 {
            viewer::with(Preferences { calm_screen: true }, || film(&run, "c-shrooms-calm", t, true));
        }
        idle(&mut run);
    }
    // The party: the gnome's invitation, the lights coming on, the first
    // wave's banner.
    let mut run = garden(3);
    put(&mut run, 1, GNOME);
    put(&mut run, 2, (8.0, 9.5));
    put(&mut run, 3, (16.5, 9.0));
    press(&mut run, 1);
    for t in 0..FIRST_WAVE + 60 {
        if t % 3 == 0 {
            film(&run, "d-party-start", t, false);
        }
        idle(&mut run);
    }
    // At its height: every kind of guest about the lawn.
    put(&mut run, 1, (12.0, 8.6));
    for (kind, (x, y)) in [
        (EnemyKind::Biter, (5.5, 10.0)),
        (EnemyKind::Spiderling, (18.0, 10.5)),
        (EnemyKind::Spiderling, (15.5, 11.8)),
        (EnemyKind::Silkmother, (20.0, 9.2)),
        (EnemyKind::Leech, (7.5, 11.6)),
        (EnemyKind::Biter, (13.5, 12.0)),
        (EnemyKind::Spiderling, (9.5, 11.0)),
    ] {
        if let Some(id) = run.spawn_at(kind, x * TILE_UNITS, y * TILE_UNITS) {
            yard_mut(&mut run).guests.push(id);
        }
    }
    for e in run.enemies.iter_mut() {
        e.age = 99;
    }
    for t in 0..72 {
        if t % 2 == 0 {
            film(&run, "e-party", t, false);
            film(&run, "e-party-tripping", t, true);
        }
        idle(&mut run);
    }
    // A bite at the hatch from each kind, the rest of the guests gone.
    let guests = yard(&run).guests.clone();
    run.enemies.retain(|e| !guests.contains(&e.id));
    yard_mut(&mut run).guests.clear();
    for kind in [EnemyKind::Leech, EnemyKind::Biter, EnemyKind::Spiderling, EnemyKind::Silkmother] {
        let (x, y) = (DOOR.0 * TILE_UNITS, DOOR.1 * TILE_UNITS + 0.6);
        let Some(id) = run.spawn_at(kind, x, y) else { continue };
        yard_mut(&mut run).guests.push(id);
        let name = format!("f-bite-{kind:?}").to_lowercase();
        for t in 0..36 {
            film(&run, &name, t, false);
            idle(&mut run);
        }
    }
    // The house near gone.
    yard_mut(&mut run).house = 20;
    for t in 0..30 {
        if t % 2 == 0 {
            film(&run, "g-shaky", t, false);
        }
        idle(&mut run);
    }
    // Held: the knights hold the door through every wave.
    let mut run = garden(2);
    put(&mut run, 1, GNOME);
    put(&mut run, 2, (14.5, 8.5));
    press(&mut run, 1);
    for _ in 0..HZ * 400 {
        let guests = yard(&run).guests.clone();
        for e in run.enemies.iter_mut().filter(|e| guests.contains(&e.id)) {
            e.hp = 0;
        }
        idle(&mut run);
        if yard(&run).party != Party::Holding {
            break;
        }
    }
    put(&mut run, 1, (10.5, 9.0));
    put(&mut run, 2, (14.5, 9.0));
    for t in 0..150 {
        if t % 3 == 0 {
            film(&run, "h-held", t, false);
        }
        idle(&mut run);
    }
    // Lost: every guest gets in.
    let mut run = garden(2);
    put(&mut run, 1, GNOME);
    put(&mut run, 2, (16.0, 9.0));
    press(&mut run, 1);
    for _ in 0..HZ * 120 {
        let guests = yard(&run).guests.clone();
        let door = (DOOR.0 * TILE_UNITS, DOOR.1 * TILE_UNITS);
        for e in run.enemies.iter_mut().filter(|e| guests.contains(&e.id)) {
            (e.x, e.y) = door;
        }
        idle(&mut run);
        if yard(&run).party != Party::Holding {
            break;
        }
    }
    put(&mut run, 1, (9.0, 9.5));
    run.enemies.clear();
    for t in 0..150 {
        if t % 3 == 0 {
            film(&run, "i-lost", t, false);
        }
        idle(&mut run);
    }
    // The Lawn Flamingo, up close, beside a knight.
    let mut boss = Run::new(41, 1, None);
    boss.enemies.clear();
    let at = boss.flamingo().unwrap();
    let (x, y) = (boss.players[&1].x + 10.0, boss.players[&1].y - 2.0);
    boss.spawn_boss_at_for_test(at, x, y);
    for _ in 0..10 {
        idle(&mut boss);
    }
    film(&boss, "j-flamingo", 0, false);
}

#[test]
fn the_lawn_flamingo_guards_the_gardens_floor_and_no_other() {
    let flamingo = bosses::lawn_flamingo();
    assert!(flamingo.hp > 0);
    assert!(!bosses::builtin().iter().any(|b| b.id == FLAMINGO), "not in the built-in population");
    // Its own pack's floors never draw it.
    let mut run = Run::new(41, 1, None);
    assert!(run.flamingo().is_some());
    let at = run.flamingo();
    assert_eq!(run.flamingo(), at, "found once, never pushed twice");
}

#[test]
fn an_open_third_floor_keeps_the_garden_and_the_flamingo_in_the_guardians_chamber() {
    let mut run = Run::new(41, 1, None);
    run.open_floor_for_test(DEPTH);
    run.ready_nook();
    let nook = run.nook.clone().expect("the third floor readies its garden");
    assert_eq!(run.dungeon.rooms[nook.room].kind, RoomKind::GnomeParty);
    let flamingo = run.bosses.iter().position(|b| b.id == FLAMINGO).unwrap();
    let raised = run.enemies.iter().chain(run.parked.iter()).any(|e| e.boss == Some(flamingo as u8));
    let bosses: Vec<_> = run.enemies.iter().chain(run.parked.iter()).filter_map(|e| e.boss).map(|b| run.bosses[usize::from(b)].id.clone()).collect();
    assert!(raised, "the guardian is the lawn flamingo: {bosses:?}");
    assert!(!run.enemies.iter().chain(run.parked.iter()).any(|e| e.kind == EnemyKind::Dragon), "no dragon in its place");
    assert!(run.valid_snapshot());
    // A floor below has its pack's own guardian.
    let mut other = Run::new(41, 1, None);
    other.open_floor_for_test(DEPTH + 1);
    let flamingo = other.bosses.iter().position(|b| b.id == FLAMINGO);
    assert!(!other.enemies.iter().chain(other.parked.iter()).any(|e| e.boss.is_some() && e.boss == flamingo.map(|i| i as u8)));
}
