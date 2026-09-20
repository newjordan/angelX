use super::*;
use crate::stage::world_viz::World;

const ROOMS: [(Building, u8); 8] = [
    (Building::Keep, KEEP_HEARTH_ID),
    (Building::Gatehouse, PORTCULLIS_ID),
    (Building::Rookery, RAVEN_ID),
    (Building::Scriptorium, LECTERN_ID),
    (Building::Smithy, ANVIL_ID),
    (Building::Chapel, ALTAR_ID),
    (Building::RoundTable, ROUND_TABLE_ID),
    (Building::Observatory, TELESCOPE_ID),
];

#[test]
fn every_supported_building_has_exactly_one_furnished_place() {
    for (building, _) in ROOMS {
        let matches = PLACES
            .iter()
            .filter(|place| place.building == building)
            .count();
        assert_eq!(matches, 1, "{building:?} should have exactly one place");
        assert!(
            supports(building),
            "{building:?} has a place but reads as unsupported"
        );
    }
    assert_eq!(PLACES.len(), ROOMS.len());
    for place in PLACES {
        assert!(
            !place.stations.is_empty(),
            "{:?} is an empty room",
            place.building
        );
    }
}

#[test]
fn no_nudge_escapes_the_bay_grid() {
    // A nudge is art tuning on top of a bay. Once one grows past half a
    // bay the station belongs in a different bay, and the grid has stopped
    // describing the room.
    const MAX_NUDGE: f32 = 0.5;
    for place in PLACES {
        for placed in place.stations {
            let (dx, dy) = placed.bay.nudge;
            assert!(
                dx.abs() <= MAX_NUDGE && dy.abs() <= MAX_NUDGE,
                "{:?} {} nudged ({dx}, {dy}) past {MAX_NUDGE}",
                place.building,
                placed.station.name
            );
        }
    }
}

#[test]
fn every_station_stands_inside_its_shell_and_clear_of_the_entry() {
    // Walls occupy cells 0 and 8, so the open floor is (1.0, 8.0). The
    // camera stands at y 7.25 and the door is behind it; stations must
    // leave that approach clear or the player walks into furniture.
    const ENTRY_CLEARANCE_Y: f32 = 6.0;
    for place in PLACES {
        for placed in place.stations {
            let (x, y) = placed.bay.xy();
            let half_width = placed.station.size.0 / 2.0;
            let name = placed.station.name;
            assert!(
                x - half_width > 1.0 && x + half_width < 8.0,
                "{:?} {name} spans {:.2}..{:.2}, outside the walls",
                place.building,
                x - half_width,
                x + half_width
            );
            assert!(
                y > 1.0 && y < ENTRY_CLEARANCE_Y,
                "{:?} {name} sits at depth {y:.2}, not inside 1.0..{ENTRY_CLEARANCE_Y}",
                place.building
            );
        }
    }
}

#[test]
fn station_art_kinds_are_shared_only_where_intended() {
    // Prop art dispatches on `id - PROP_ID_BASE` with no knowledge of the
    // room, so two stations sharing an id draw the identical silhouette.
    // Sharing is legitimate while a station is waiting for its own plate;
    // it is a bug when it happens by accident. Pin the known set so a new
    // collision has to be declared here before it can ship.
    const APPROVED_SHARING: [(u8, &[&str]); 2] = [
        // One flame silhouette serves every open flame in the realm.
        (SMITHY_FIRE_ID, &["forge fire", "chapel taper"]),
        // One bird, two poses. Kind 10 already spreads or settles its
        // wings on `lit`, so the pair is the plate working as intended.
        (RAVEN_ID, &["raven", "roosting raven"]),
    ];

    let mut by_id: std::collections::BTreeMap<u8, std::collections::BTreeSet<&str>> =
        std::collections::BTreeMap::new();
    for place in PLACES {
        for placed in place.stations {
            by_id
                .entry(placed.station.id)
                .or_default()
                .insert(placed.station.name);
        }
    }

    for (id, names) in by_id {
        if names.len() == 1 {
            continue;
        }
        let approved = APPROVED_SHARING
            .iter()
            .find(|(approved_id, _)| *approved_id == id)
            .map(|(_, names)| {
                names
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
            });
        assert_eq!(
            Some(names.clone()),
            approved,
            "stations {names:?} all draw art kind {} — declare the sharing or give one its own plate",
            id.saturating_sub(PROP_ID_BASE)
        );
    }
}

#[test]
fn scriptorium_contains_each_named_tutor_station_once() {
    let tutor_names: std::collections::BTreeSet<_> = SCRIPTORIUM_TUTOR_STATIONS
        .iter()
        .map(|station| station.name)
        .collect();
    let tutor_ids: std::collections::BTreeSet<_> = SCRIPTORIUM_TUTOR_STATIONS
        .iter()
        .map(|station| station.id)
        .collect();
    assert_eq!(tutor_names.len(), SCRIPTORIUM_TUTOR_STATIONS.len());
    assert_eq!(
        tutor_ids.len(),
        SCRIPTORIUM_TUTOR_STATIONS.len(),
        "every tutor station needs its own silhouette"
    );
    for station in SCRIPTORIUM_TUTOR_STATIONS {
        let placements = SCRIPTORIUM
            .iter()
            .filter(|placed| placed.station.id == station.id)
            .count();
        assert_eq!(
            placements, 1,
            "Scriptorium should contain one {}",
            station.name
        );
    }
}

#[test]
fn a_place_that_gains_a_station_shows_it_without_a_code_change() {
    // The growth path: append to a place's station list, nothing else.
    let grown_stations: &'static [Placement] = Box::leak(
        SCRIPTORIUM
            .iter()
            .copied()
            .chain(std::iter::once(put(TUTOR_BENCH, bay(0, 2))))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let grown = Place {
        building: Building::Scriptorium,
        shell: T,
        stations: grown_stations,
    };

    let before = compose(place_for(Building::Scriptorium).unwrap(), 8, 0.0125);
    let after = compose(&grown, 8, 0.0125);
    assert_eq!(after.0.sprites.len(), before.0.sprites.len() + 1);
}

#[test]
fn interior_authored_rooms_have_centerpieces_and_one_south_door() {
    for (building, centerpiece) in ROOMS {
        let (map, _) = scene(building, 0, 0.0).unwrap();
        assert!(map.sprites.iter().any(|sprite| sprite.id == centerpiece));
        assert_eq!(
            map.cells
                .iter()
                .filter(|&&cell| cell == MATERIAL_DOOR)
                .count(),
            1
        );
        assert_eq!(map.cells[8 * ROOM_SIDE as usize + 4], MATERIAL_DOOR);
        assert!(map.terrain.iter().all(|&height| height == 0.0));
    }
}

#[test]
fn interior_entry_rejects_a_knight_not_at_the_destination() {
    let mut world = World::new(7);
    world.target = Building::Smithy;
    world.enter_interior();
    assert_eq!(world.interior, None);
}

fn settle_at(world: &mut World, building: Building) {
    world.target = building;
    let dest = world.dest();
    world.avatar = dest;
    world.avatar_vis = (dest.0 as f32, dest.1 as f32);
    world.settle_ticks = super::super::cinematics::ARRIVAL_HOLD_TICKS;
}

#[test]
fn room_plate_ignores_first_person_flicker_and_leaving_restores_outdoor_key() {
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let mut world = World::new(17);
    settle_at(&mut world, Building::Smithy);
    let outdoor = world.cinematic_key();
    world.enter_interior();
    let inside = world.cinematic_key();
    assert_ne!(inside, outdoor);
    world.tick += super::super::cinematics::INTERIOR_FRAME_HOLD_TICKS;
    assert_eq!(
        world.cinematic_key(),
        inside,
        "room plate does not paint the first-person flicker clock"
    );
    // The outdoor key legitimately animates with the clock (the settled
    // first-person plate re-keys on the two-pose rider canter, keyed off
    // `tick`), so restore the clock before the round-trip check — otherwise
    // this asserts a tick-stability the outdoor key no longer has. Leaving
    // toggles only `interior`, so at the original tick the key is identical.
    world.tick -= super::super::cinematics::INTERIOR_FRAME_HOLD_TICKS;
    world.leave_interior();
    assert_eq!(world.cinematic_key(), outdoor);
}

#[test]
fn interior_scene_is_deterministic_and_differs_from_outdoors() {
    let mut a = World::new(23);
    let mut b = World::new(23);
    settle_at(&mut a, Building::RoundTable);
    settle_at(&mut b, Building::RoundTable);
    let outdoor = a.travel_scene();
    a.enter_interior();
    b.enter_interior();
    let first = a.travel_scene();
    let second = b.travel_scene();
    assert_ne!(first.0.cells, outdoor.0.cells);
    assert_eq!(first.0.cells, second.0.cells);
    assert_eq!(
        first.1.heading_rad.to_bits(),
        second.1.heading_rad.to_bits()
    );
}

#[test]
fn a_new_journey_auto_exits_the_room() {
    let mut world = World::new(29);
    settle_at(&mut world, Building::Smithy);
    world.enter_interior();
    assert_eq!(world.interior, Some(Building::Smithy));
    world.target = Building::RoundTable;
    world.tick();
    assert_eq!(world.interior, None);
}
