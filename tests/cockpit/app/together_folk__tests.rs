use super::*;
use crate::drive::together_shooter::barony::{Barony, Court, Work, work};
use crate::drive::together_shooter::world;

fn home_with(b: Barony) -> Home {
    Home {
        barony: b,
        ..Home::default()
    }
}

/// Every room of the world, as the run lays it out.
fn rooms() -> Vec<Room> {
    let run = Run::at_home(7, 1, None, Home::default(), Default::default());
    run.dungeon.rooms.clone()
}

/// Everyone in sight over a long stretch of ticks, by room.
fn sight(home: &Home) -> Vec<(RoomKind, Figure)> {
    let mut out = Vec::new();
    for kind in [
        RoomKind::Gate,
        RoomKind::MineHead,
        RoomKind::KingsHall,
        RoomKind::Stables,
        RoomKind::Lists,
        RoomKind::Home,
    ] {
        for tick in (0..HZ as u64 * 300).step_by(7) {
            for f in figures(home, kind, Pack::Cavern, tick) {
                out.push((kind, f));
            }
        }
    }
    out
}

fn walkable(rooms: &[Room], kind: RoomKind, f: &Figure) -> bool {
    let room = rooms.iter().find(|r| r.kind == kind).unwrap();
    matches!(
        room.tile((f.x / TILE_UNITS) as i32, (f.y / TILE_UNITS) as i32),
        Tile::Floor | Tile::Door | Tile::Stairs
    )
}

#[test]
fn nobody_is_about_until_the_king_comes() {
    assert!(sight(&Home::default()).is_empty());
}

#[test]
fn the_camped_king_and_his_guards_wait_at_the_mine_head() {
    let mut b = Barony::default();
    b.court = Court::Camped;
    let seen = sight(&home_with(b));
    assert!(
        seen.iter()
            .all(|(k, _)| matches!(*k, RoomKind::MineHead | RoomKind::Gate))
    );
    assert!(
        seen.iter()
            .any(|(k, f)| f.folk == Folk::King && *k == RoomKind::MineHead)
    );
    // His man waits in the gate courtyard, the main way in.
    assert!(
        seen.iter()
            .any(|(k, f)| f.folk == Folk::Guard && *k == RoomKind::Gate)
    );
    assert_eq!(
        figures(
            &home_with(Barony {
                court: Court::Camped,
                ..Barony::default()
            }),
            RoomKind::MineHead,
            Pack::Cavern,
            0
        )
        .iter()
        .filter(|f| f.folk == Folk::Guard)
        .count(),
        2
    );
}

#[test]
fn miners_walk_to_the_shaft_and_come_up_with_ore_on_walkable_ground() {
    let mut b = Barony::default();
    b.court = Court::Met;
    b.done.push("workings".into());
    let rooms = rooms();
    let seen = sight(&home_with(b));
    let miners: Vec<_> = seen.iter().filter(|(_, f)| f.folk == Folk::Miner).collect();
    assert!(miners.iter().any(|(_, f)| f.loaded), "porters with sacks");
    assert!(miners.iter().any(|(_, f)| !f.loaded), "miners going down");
    for (kind, f) in &seen {
        assert!(walkable(&rooms, *kind, f), "{f:?} in {kind:?}");
    }
}

#[test]
fn builders_haul_stone_to_the_ruin_while_the_hall_goes_up() {
    let mut b = Barony::default();
    b.court = Court::Met;
    b.works.push(Work {
        id: "hall".into(),
        labour: 10,
    });
    let rooms = rooms();
    let seen = sight(&home_with(b.clone()));
    assert!(
        seen.iter()
            .any(|(_, f)| f.folk == Folk::Builder && f.loaded)
    );
    assert!(
        seen.iter()
            .any(|(_, f)| f.folk == Folk::Builder && f.pose == Pose::Work)
    );
    for (kind, f) in &seen {
        assert!(walkable(&rooms, *kind, f), "{f:?}");
    }
    // Built, they are gone.
    b.works[0].labour = work("hall").unwrap().labour;
    b.court = Court::Sworn;
    let after = sight(&home_with(b));
    assert!(!after.iter().any(|(_, f)| f.folk == Folk::Builder));
    assert!(
        after
            .iter()
            .any(|(k, f)| f.folk == Folk::King && *k == RoomKind::KingsHall)
    );
}

#[test]
fn a_burning_forge_sends_a_carter_through_the_gate_and_smiths_to_its_anvils() {
    let mut b = Barony::default();
    b.court = Court::Sworn;
    b.done.push("workings".into());
    b.lit.push("ore-forge".into());
    let home = home_with(b);
    let rooms = rooms();
    let seen = sight(&home);
    let gate: Vec<_> = seen
        .iter()
        .filter(|(k, f)| *k == RoomKind::Gate && f.folk == Folk::Carter)
        .collect();
    assert!(gate.iter().any(|(_, f)| f.loaded) && gate.iter().any(|(_, f)| !f.loaded));
    for (kind, f) in &seen {
        assert!(walkable(&rooms, *kind, f), "{f:?}");
    }
    let smiths = figures(&home, RoomKind::Forge, Pack::Cavern, 40);
    assert_eq!(smiths.iter().filter(|f| f.folk == Folk::Smith).count(), 2);
    assert!(figures(&home, RoomKind::Forge, Pack::Crypt, 40).is_empty());
}

#[test]
fn where_anyone_is_is_the_same_for_the_same_realm_and_tick() {
    let mut b = Barony::default();
    b.court = Court::Met;
    b.done.push("workings".into());
    b.works.push(Work {
        id: "hall".into(),
        labour: 900,
    });
    let home = home_with(b);
    for tick in [0, 1, 77, 9_999, 123_456] {
        assert_eq!(
            figures(&home, RoomKind::MineHead, Pack::Cavern, tick),
            figures(&home, RoomKind::MineHead, Pack::Cavern, tick)
        );
    }
    // And they move.
    assert_ne!(
        figures(&home, RoomKind::MineHead, Pack::Cavern, 100),
        figures(&home, RoomKind::MineHead, Pack::Cavern, 160)
    );
    let _ = world::ROOMS;
}
