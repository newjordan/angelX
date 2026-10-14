//! Fixed moments from the original plate writers, paired from one Run.
use super::*;
use crate::rules::{
    Shot, Weapon, cards, chariot, crawl_floor, patterns, pitstop, signatures as fight, votes,
};
fn quiet() -> Run {
    let mut r = Run::new(4, 1, None);
    let at = r
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    r.enter_for_test(at);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    r.window = None;
    let room = &mut r.dungeon.rooms[r.at];
    for row in 1..room.rows - 1 {
        for col in 1..room.cols - 1 {
            room.set_for_test(col, row, Tile::Floor);
        }
    }
    let h = r.players.get_mut(&1).unwrap();
    h.x = 24.0;
    h.y = 20.0;
    h.invulnerable = 100_000;
    r
}
fn handgonne() -> Run {
    let mut r = quiet();
    r.dungeon.pack = Pack::Archive;
    let h = r.players.get_mut(&1).unwrap();
    h.x = 10.0;
    h.y = 14.0;
    h.weapon = Weapon::Handgonne;
    h.temper = 2;
    h.arm = Some("handgonne".into());
    h.hand = vec!["potion".into()];
    h.ult_charge = 650;
    for n in 0..cards::SPELL_SLOTS {
        let id = format!("review-spell-{n}");
        let card = cards::check(
            &id,
            &format!("name Ember {}\nkind spell\ncooldown 4\ncast ring\n", n + 1),
        )
        .unwrap()
        .card;
        r.book.insert(card);
        r.players.get_mut(&1).unwrap().spells[n] = Some(id);
    }
    for (kind, x, y) in [
        (EnemyKind::Wraith, 30.0, 7.0),
        (EnemyKind::Skeleton, 36.0, 18.0),
        (EnemyKind::Imp, 38.0, 10.0),
    ] {
        r.spawn_at_for_test(kind, x, y);
        let e = r.enemies.last_mut().unwrap();
        e.hp = 5000;
        e.max_hp = 5000;
    }
    for _ in 0..45 {
        r.step(&BTreeMap::new());
    }
    for _ in 0..7 {
        r.step(&BTreeMap::from([(
            1,
            Input {
                aim_x: 1,
                fire: true,
                ..Default::default()
            },
        )]));
    }
    r
}
fn flower() -> Run {
    let mut r = quiet();
    r.dungeon.pack = Pack::Crypt;
    let h = r.players.get_mut(&1).unwrap();
    h.hp = 100_000;
    h.max_hp = 100_000;
    r.spawn_at_for_test(EnemyKind::Wraith, 24.0, 7.0);
    let e = r.enemies.last_mut().unwrap();
    e.hp = 100_000;
    e.max_hp = 100_000;
    e.seen_hp = 100_000;
    e.age = 100;
    e.frozen = 100_000;
    r.exhibit_pattern_for_test(
        patterns::Emitter::Flower,
        Shot::Wisp,
        (24.0, 7.0),
        (24.0, 20.0),
    );
    for t in 0..40 {
        r.step(&BTreeMap::new());
        let h = r.players.get_mut(&1).unwrap();
        h.x = 24.0 + t as f32 * 0.1;
        h.y = 20.0;
    }
    r.enemies[0].frozen = 0;
    r
}
fn boss(id: &str, depth: u32, pack: Pack) -> Run {
    let mut r = Run::new(17, 1, Some("Friend"));
    r.dungeon = crawl_floor(depth, pack, 17);
    let at = r
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap();
    r.enter_for_test(at);
    r.enemies.clear();
    r.projectiles.clear();
    r.calm_for_test();
    r.works.clouds.clear();
    r.works.perils.clear();
    let i = r.bosses.iter().position(|b| b.id == id).unwrap();
    r.spawn_boss_at_for_test(i, 24.0, 20.0);
    r.enemies[0].signature.active = true;
    for (&id, h) in &mut r.players {
        h.x = if id == 1 { 10.0 } else { 36.0 };
        h.y = 22.0;
        h.invulnerable = 10000;
    }
    r.phase = Phase::Fighting;
    r.tick = 90;
    r
}
fn ripper() -> Run {
    let mut r = boss(fight::RIPPER, 5, Pack::Fungal);
    for y in 9..16 {
        for x in 5..20 {
            r.dungeon.rooms[r.at].set_for_test(x, y, Tile::Floor);
        }
    }
    let e = &mut r.enemies[0];
    e.stage = fight::DASH;
    e.x = 34.0;
    e.y = 25.0;
    e.dir = (-0.98, -0.20);
    e.signature.left = 1;
    e.signature.cycle = 1;
    for _ in 0..14 {
        r.step(&BTreeMap::new());
    }
    r
}
fn engine() -> Run {
    let mut r = boss(fight::ENGINE, 2, Pack::Cavern);
    r.enemies[0].timer = 104;
    for _ in 0..19 {
        r.step(&BTreeMap::new());
    }
    r
}
fn college() -> Run {
    let mut r = Run::at_home(17, 1, Some("Scholar"), Home::default(), Default::default());
    r.join_seat(3, "Researcher");
    r.join_seat(4, "Librarian");
    r.come_to_the_gate();
    r.tick = 83;
    for kind in [RoomKind::CollegeGrounds, RoomKind::CollegeHall] {
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 10.5));
        for _ in 0..60 {
            r.step(&BTreeMap::new());
        }
    }
    r
}
fn ranch(kind: RoomKind) -> Run {
    let mut r = Run::at_home(23, 1, Some("Rider"), Home::default(), Default::default());
    r.join_seat(3, "Squire");
    r.come_to_the_gate();
    r.arrive(
        world::room_of(&r.dungeon, kind).unwrap(),
        if kind == RoomKind::Ranch {
            (14.5, 5.5)
        } else {
            (12.0, 10.5)
        },
    );
    r.tick = if kind == RoomKind::Ranch { 60 } else { 160 };
    r
}
fn map() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Home::default(), Default::default());
    let geo = super::super::super::super::kingdom::geography(&r.kingdom.wishes);
    let site = geo
        .sites
        .iter()
        .find(|s| s.room == RoomKind::CollegeGrounds)
        .unwrap();
    r.walk_kingdom(Some(site.stand));
    r.follow_kingdom_cameras();
    r.tick = 80;
    r
}
fn privy() -> Run {
    let mut r = Run::new(0, 1, Some("You"));
    r.descend_for_test();
    r.window = None;
    r.pits.set(2, "Matt", 282);
    r.pits.set(3, "Sir Bors", 230);
    r.pits.set(1, "You", 212);
    r.players.get_mut(&1).unwrap().pit.result = Some(pitstop::Result {
        quality: pitstop::Quality::Royal,
        total: 212,
        left: pitstop::RESULT_TICKS - 24,
        party_record: true,
    });
    r.tick = 180;
    r
}
fn race() -> Run {
    use crate::realm::chivalry::{Horse, Stable, bloodline};
    let mut stable = Stable::default();
    stable.herd.push(Horse {
        name: "Gallery".into(),
        house: "qwen".into(),
        blood: bloodline("qwen"),
        ..Default::default()
    });
    let mut r = Run::at_home(
        7,
        1,
        None,
        Home {
            stable,
            ..Default::default()
        },
        Default::default(),
    );
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::Hippodrome).unwrap(),
        (20.0, 10.5),
    );
    r.step(&BTreeMap::new());
    for _ in 0..crate::rules::home::BUY_HOLD + 2 {
        r.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
    }
    for _ in 0..chariot::GATE_TICKS + 260 {
        r.step(&BTreeMap::from([(
            1,
            Input {
                dash: true,
                ..Default::default()
            },
        )]));
    }
    assert!(r.chariot.is_some());
    r
}
fn vote() -> Run {
    let mut r = quiet();
    r.dungeon.pack = Pack::Crypt;
    r.players.get_mut(&1).unwrap().name = "You".into();
    r.join_seat(2, "Friend");
    r.playtest.arrive(1, "You");
    r.playtest.arrive(2, "Friend");
    for (serial, (id, action)) in [
        (1, votes::Action::Toggle),
        (1, votes::Action::Rate(4)),
        (1, votes::Action::Down),
        (1, votes::Action::Untried),
        (2, votes::Action::Toggle),
        (2, votes::Action::Rate(2)),
    ]
    .into_iter()
    .enumerate()
    {
        r.vote_command(
            id,
            votes::Command {
                action,
                serial: serial as u64 + 1,
            },
        );
    }
    r
}
pub fn fixtures() -> Vec<(&'static str, Run)> {
    vec![
        ("handgonne", handgonne()),
        ("flower", flower()),
        ("ripper", ripper()),
        ("grinwick", engine()),
        ("kingdom", map()),
        ("college", college()),
        ("ranch", ranch(RoomKind::Ranch)),
        ("fields", ranch(RoomKind::Pasture)),
        ("privy", privy()),
        ("race", race()),
        ("vote", vote()),
    ]
}
