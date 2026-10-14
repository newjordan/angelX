use super::super::home::Home;
use super::super::world;
use super::*;

fn run() -> Run {
    let mut r = Run::at_home(23, 1, Some("Rider"), Home::default(), Spoils::default());
    r.come_to_the_gate();
    r
}

fn go(r: &mut Run, kind: RoomKind, at: (f32, f32)) {
    r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
    r.step(&BTreeMap::new());
}

fn press(r: &mut Run, ticks: u32) {
    let keys = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    for _ in 0..ticks {
        r.step(&keys);
    }
}

fn gold(n: u32) -> Spoils {
    let mut s = Spoils::default();
    s.add(Spoil::Gold, n);
    s
}

#[test]
fn ranch_and_fields_are_reached_from_the_stables_and_lead_back() {
    let r = run();
    for kind in ROOMS {
        assert!(world::room_of(&r.dungeon, kind).is_some(), "{kind:?} stands");
    }
    let to = |from: RoomKind| {
        DOORS
            .iter()
            .filter(|e| e.room == from)
            .filter_map(|e| match e.to {
                Dest::Room(k, _) => Some(k),
                Dest::Delve(_) => None,
            })
            .collect::<Vec<_>>()
    };
    assert!(to(RoomKind::Stables).contains(&RoomKind::Ranch));
    assert!(to(RoomKind::Ranch).contains(&RoomKind::Stables));
    assert!(to(RoomKind::Ranch).contains(&RoomKind::Pasture));
    assert!(to(RoomKind::Pasture).contains(&RoomKind::Ranch));
    // Every gate's tiles are stairs in its room, and every arrival is open
    // ground off the stairs.
    for e in DOORS {
        let room = &r.dungeon.rooms[world::room_of(&r.dungeon, e.room).unwrap()];
        let (c, row, w, h) = e.at;
        for y in row..row + h {
            for x in c..c + w {
                assert_eq!(room.tile(x, y), Tile::Stairs, "{} at {x},{y}", e.id);
            }
        }
        let Dest::Room(k, (ax, ay)) = e.to else { continue };
        let there = &r.dungeon.rooms[world::room_of(&r.dungeon, k).unwrap()];
        assert_eq!(there.tile(ax as i32, ay as i32), Tile::Floor, "{} arrives on floor", e.id);
    }
}

#[test]
fn the_fields_are_wide_open_and_helios_runs_inside_them() {
    let r = run();
    let fields = &r.dungeon.rooms[world::room_of(&r.dungeon, RoomKind::Pasture).unwrap()];
    let open = (1..ROWS as i32 - 1)
        .flat_map(|y| (1..COLS as i32 - 1).map(move |x| (x, y)))
        .filter(|&(x, y)| fields.tile(x, y) == Tile::Floor)
        .count();
    assert!(open > (COLS - 2) * (ROWS - 2) * 9 / 10, "{open} open tiles");
    let mut galloped = false;
    let mut rested = false;
    for tick in 0..(12 * HZ as u64) {
        let ((x, y), _, gait) = helios(tick);
        assert!((1.5..22.5).contains(&x) && (1.5..12.5).contains(&y), "{x},{y}");
        galloped |= gait.is_some();
        rested |= gait.is_none();
    }
    assert!(galloped && rested);
}

#[test]
fn helios_cannot_be_caught_only_admired() {
    let mut r = run();
    let ((x, y), _, _) = helios(r.tick + 1);
    go(&mut r, RoomKind::Pasture, (x, y + 0.5));
    let ((x, y), _, _) = helios(r.tick + 1);
    r.players.get_mut(&1).unwrap().x = x * TILE_UNITS;
    r.players.get_mut(&1).unwrap().y = (y + 0.5) * TILE_UNITS;
    press(&mut r, 1);
    let said = r.found.as_ref().map(|f| f.2.clone()).unwrap_or_default();
    assert!(said.contains("Helios") || said.contains("star"), "{said}");
    assert!(
        !r.marks.keys().any(|m| m.starts_with("ranch:")),
        "nothing is bought or taken"
    );
}

#[test]
fn the_stud_book_takes_a_full_hold_and_marks_a_covering() {
    let mut r = run();
    go(&mut r, RoomKind::Ranch, (14.5, 4.5));
    press(&mut r, super::super::home::BUY_HOLD - 2);
    assert!(!r.marks.contains_key("ranch:breed"), "a tap buys nothing");
    press(&mut r, 3);
    assert_eq!(r.marks.get("ranch:breed"), Some(&1));
    // Held on, it does not ask twice.
    press(&mut r, super::super::home::BUY_HOLD * 2);
    assert_eq!(r.marks.get("ranch:breed"), Some(&1));
}

fn mare(name: &str) -> Horse {
    Horse { name: name.into(), house: "qwen".into(), mare: true,
        blood: crate::realm::chivalry::bloodline("qwen"), ..Horse::default() }
}

#[test]
fn a_covering_needs_a_herd_mare_and_the_fee_and_is_rare() {
    let marks = BTreeMap::from([("ranch:breed".to_string(), 1)]);
    let mut ranch = Ranch::default();
    let mut stable = Stable::default();
    let mut t = gold(10_000);
    let (changed, said) = settle(&mut ranch, &mut t, &mut stable, 0, &marks);
    assert!(!changed && said[0].contains("mare"));
    stable.herd.push(mare("Cloud"));
    let mut poor = gold(FEE - 1);
    let (changed, _) = settle(&mut ranch, &mut poor, &mut stable, 0, &marks);
    assert!(!changed);
    assert_eq!(poor.get(Spoil::Gold), FEE - 1, "nothing taken when short");
    let mut tries = 0;
    let mut t = gold(1_000_000);
    while herd_foal(&stable).is_none() && tries < 400 {
        let before = t.get(Spoil::Gold);
        let (changed, _) = settle(&mut ranch, &mut t, &mut stable, 3, &marks);
        assert!(changed);
        assert_eq!(t.get(Spoil::Gold), before - FEE);
        tries += 1;
    }
    let foal = herd_foal(&stable).cloned().expect("a foal within 400 tries");
    assert!(tries > 1, "not on the first try");
    assert_eq!(foal.born, Some(3));
    assert!(foal.name == "Phaethon" || foal.name == "Aurora");
    assert_eq!(foal.sire, "Helios");
    assert_eq!(foal.dam, "Cloud");
    assert_eq!(foal.house, "qwen");
    assert_eq!(foal.blood, HELIOS_BLOOD);
    assert!(ranch.foal.is_none(), "only the herd keeps the horse");
    assert_eq!(stable.foals, 1);
    let before = t.get(Spoil::Gold);
    let (changed, said) = settle(&mut ranch, &mut t, &mut stable, 3, &marks);
    assert!(!changed && said[0].contains(&foal.name));
    assert_eq!(t.get(Spoil::Gold), before);
    assert_eq!(age(&foal, 3), Age::Foal);
    assert_eq!(age(&foal, 3 + YEARLING), Age::Yearling);
    assert_eq!(age(&foal, 3 + GROWN), Age::Grown);
    assert!(!foal.raceable(3 + YEARLING));
    assert!(foal.raceable(3 + GROWN));
}

#[test]
fn the_stud_book_prefers_the_selected_mare_and_falls_back_to_the_first_mare() {
    let mut stable = Stable::default();
    stable.herd = vec![Horse { name: "Stallion".into(), ..Horse::default() }, mare("Cloud"), mare("Rain")];
    stable.selected_herd = Some(2);
    assert_eq!(stable.mare(0).unwrap().name, "Rain");
    stable.selected_herd = Some(0);
    assert_eq!(stable.mare(0).unwrap().name, "Cloud");
    stable.herd[1].born = Some(0);
    assert_eq!(stable.mare(YEARLING).unwrap().name, "Rain", "yearlings cannot cover");
    let marks = BTreeMap::from([("ranch:breed".to_string(), 1)]);
    let mut ranch = Ranch::default();
    let mut treasury = gold(1_000_000);
    stable.selected_herd = Some(2);
    for _ in 0..400 {
        settle(&mut ranch, &mut treasury, &mut stable, 0, &marks);
        if herd_foal(&stable).is_some() { break; }
    }
    assert_eq!(herd_foal(&stable).unwrap().dam, "Rain");
}

#[test]
fn a_full_herd_is_not_charged_and_selling_the_foal_does_not_reset_the_stud_book() {
    let marks = BTreeMap::from([("ranch:breed".to_string(), 1)]);
    let mut ranch = Ranch::default();
    let mut stable = Stable { herd: vec![mare("Cloud"); 12], ..Stable::default() };
    let mut treasury = gold(1000);
    let (_, said) = settle(&mut ranch, &mut treasury, &mut stable, 0, &marks);
    assert!(said[0].contains("full"));
    assert_eq!(treasury.get(Spoil::Gold), 1000);
    ranch.helios_bred = true;
    stable.herd.truncate(1);
    settle(&mut ranch, &mut treasury, &mut stable, 0, &marks);
    assert_eq!(treasury.get(Spoil::Gold), 1000);
}

#[test]
fn an_old_ranch_foal_migrates_once_even_into_a_full_herd() {
    let mut home: Home = serde_json::from_str(r#"{"ranch":{"coverings":17,"foal":{"name":"Aurora","filly":true,"born":4},"wins":6}}"#).unwrap();
    home.stable.herd = vec![mare("Cloud"); 12];
    assert!(migrate(&mut home.ranch, &mut home.stable));
    assert!(!migrate(&mut home.ranch, &mut home.stable));
    assert!(home.ranch.foal.is_none());
    assert!(home.ranch.helios_bred);
    let horse = herd_foal(&home.stable).unwrap();
    assert_eq!((&horse.name[..], &horse.sire[..], &horse.dam[..]), ("Aurora", "Helios", "Mist"));
    assert!(horse.mare);
    assert_eq!(horse.born, Some(4));
    assert_eq!(age(horse, home.ranch.wins), Age::Yearling);
    assert_eq!(horse.blood, HELIOS_BLOOD);
    let mut loaded: Home = serde_json::from_str(&serde_json::to_string(&home).unwrap()).unwrap();
    assert!(!migrate(&mut loaded.ranch, &mut loaded.stable));
    assert_eq!(loaded.stable.herd.len(), 13, "saved birth is never lost at the yard cap");
    // A partially migrated save also cannot duplicate the same foal.
    loaded.ranch.foal = Some(Foal { name: "Aurora".into(), filly: true, born: 4 });
    assert!(migrate(&mut loaded.ranch, &mut loaded.stable));
    assert_eq!(loaded.stable.herd.len(), 13);
}

#[test]
fn the_luck_is_about_one_in_odds() {
    let hits = (1..=5000u32).filter(|&n| lucky(n, 7_000 + n * 13, n / 40)).count();
    let expect = 5000 / ODDS as usize;
    assert!(hits > expect / 2 && hits < expect * 2, "{hits} lucky of 5000");
}

#[test]
fn an_old_realm_without_a_ranch_still_loads() {
    let home: Home = serde_json::from_str("{}").unwrap();
    assert_eq!(home.ranch, Ranch::default());
    let bad: Home = serde_json::from_str(r#"{"ranch": 7}"#).unwrap();
    assert_eq!(bad.ranch, Ranch::default());
}
