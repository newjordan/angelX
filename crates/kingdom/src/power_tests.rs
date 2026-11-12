use super::*;
const BOUNDS: Tile = (72, 54);
fn source(id: u32, tile: Tile, kind: PowerKind, capacity: u16, active: bool) -> PowerNode {
    PowerNode {
        id,
        tile,
        kind,
        priority: 1,
        role: PowerRole::Source { capacity, active },
    }
}
fn machine(id: u32, tile: Tile, kind: PowerKind, demand: u16, priority: u8) -> PowerNode {
    PowerNode {
        id,
        tile,
        kind,
        priority,
        role: PowerRole::Consumer {
            demand,
            enabled: true,
        },
    }
}
fn line(kind: PowerKind, row: i16, first: i16, last: i16) -> PowerGrid {
    let mut grid = PowerGrid::default();
    for x in first..=last {
        grid.lay(kind, (x, row), BOUNDS).unwrap();
    }
    grid
}

#[test]
fn power_connections_follow_orthogonal_connectors_and_ignore_diagonal_or_direct_contacts() {
    use PowerKind::Mechanical as M;
    let nodes = [source(1, (1, 1), M, 8, true), machine(2, (2, 1), M, 4, 1)];
    assert_eq!(
        PowerGrid::default()
            .evaluate(&nodes, BOUNDS)
            .unwrap()
            .consumer(M, 2)
            .unwrap()
            .status,
        PowerStatus::Disconnected
    );
    let mut grid = line(M, 2, 1, 2);
    assert!(grid.evaluate(&nodes, BOUNDS).unwrap().powered(M, 2));
    grid.remove(M, (1, 2));
    assert_eq!(
        grid.evaluate(&nodes, BOUNDS)
            .unwrap()
            .consumer(M, 2)
            .unwrap()
            .status,
        PowerStatus::Disconnected
    );
}

#[test]
fn mechanical_and_arcane_lines_cross_without_exchanging_capacity() {
    use PowerKind::{Arcane as A, Mechanical as M};
    let mut grid = line(M, 3, 1, 5);
    for y in 1..=5 {
        grid.lay(A, (3, y), BOUNDS).unwrap();
    }
    let nodes = [
        source(1, (0, 3), M, 8, true),
        machine(2, (6, 3), M, 8, 1),
        source(3, (3, 0), A, 3, true),
        machine(4, (3, 6), A, 4, 1),
    ];
    let report = grid.evaluate(&nodes, BOUNDS).unwrap();
    assert!(report.powered(M, 2));
    assert_eq!(
        report.consumer(A, 4).unwrap().status,
        PowerStatus::Overloaded
    );
    assert_ne!(
        report.component_at(M, (3, 3)).unwrap().id,
        report.component_at(A, (3, 3)).unwrap().id
    );
    assert_eq!(report.totals(M).active_capacity, 8);
    assert_eq!(report.totals(A).active_capacity, 3);
}

#[test]
fn capacity_allocation_is_whole_demand_high_priority_then_stable_id() {
    use PowerKind::Mechanical as M;
    let grid = line(M, 2, 1, 7);
    let mut nodes = vec![
        source(1, (0, 2), M, 10, true),
        machine(7, (7, 1), M, 6, 2),
        machine(4, (4, 1), M, 4, 1),
        machine(2, (2, 1), M, 4, 1),
    ];
    let report = grid.evaluate(&nodes, BOUNDS).unwrap();
    assert!(report.powered(M, 7));
    assert!(report.powered(M, 2));
    assert_eq!(report.consumer(M, 4).unwrap().allocated, 0);
    assert_eq!(
        report.consumer(M, 4).unwrap().status,
        PowerStatus::Overloaded
    );
    assert_eq!(report.totals(M).requested, 14);
    assert_eq!(report.totals(M).allocated, 10);
    nodes.reverse();
    assert_eq!(report, grid.evaluate(&nodes, BOUNDS).unwrap());
}

#[test]
fn multiple_sources_share_capacity_once_through_branching_machine_manifolds() {
    use PowerKind::Mechanical as M;
    let mut grid = line(M, 2, 1, 2);
    for x in 4..=5 {
        grid.lay(M, (x, 2), BOUNDS).unwrap();
    }
    let nodes = [
        source(1, (0, 2), M, 6, true),
        machine(2, (3, 2), M, 5, 1),
        source(3, (6, 2), M, 4, true),
        machine(4, (5, 3), M, 5, 1),
    ];
    let report = grid.evaluate(&nodes, BOUNDS).unwrap();
    assert!(report.powered(M, 2) && report.powered(M, 4));
    assert_eq!(report.components.len(), 1);
    assert_eq!(report.totals(M).active_capacity, 10);
    assert_eq!(report.totals(M).allocated, 10);
    assert_eq!(report.totals(M).sources, 2);
}

#[test]
fn inactive_machines_draw_nothing_but_keep_their_physical_manifold() {
    use PowerKind::Arcane as A;
    let mut grid = line(A, 2, 1, 2);
    grid.lay(A, (4, 2), BOUNDS).unwrap();
    let nodes = [
        source(1, (0, 2), A, 4, true),
        PowerNode {
            id: 2,
            tile: (3, 2),
            kind: A,
            priority: 2,
            role: PowerRole::Consumer {
                demand: 4,
                enabled: false,
            },
        },
        machine(3, (5, 2), A, 4, 1),
    ];
    let report = grid.evaluate(&nodes, BOUNDS).unwrap();
    assert_eq!(report.consumer(A, 2).unwrap().status, PowerStatus::Inactive);
    assert!(report.powered(A, 3));
    assert_eq!(report.totals(A).requested, 4);
}

#[test]
fn inactive_sources_explain_no_supply_and_fuel_changes_are_explicit_inputs() {
    use PowerKind::Arcane as A;
    let grid = line(A, 2, 1, 3);
    let mut nodes = [source(1, (0, 2), A, 8, false), machine(2, (4, 2), A, 6, 1)];
    let before = (grid.clone(), nodes);
    let report = grid.evaluate(&nodes, BOUNDS).unwrap();
    assert_eq!(
        report.consumer(A, 2).unwrap().status,
        PowerStatus::Overloaded
    );
    assert_eq!(report.totals(A).rated_capacity, 8);
    assert_eq!(report.totals(A).active_capacity, 0);
    assert_eq!(report.totals(A).active_sources, 0);
    assert_eq!(
        (grid.clone(), nodes),
        before,
        "reports never change fuel or state"
    );
    nodes[0].role = PowerRole::Source {
        capacity: 8,
        active: true,
    };
    assert!(grid.evaluate(&nodes, BOUNDS).unwrap().powered(A, 2));
}

#[test]
fn cutting_one_shaft_disconnects_the_job_and_repair_restores_it() {
    use PowerKind::Mechanical as M;
    let mut grid = line(M, 9, 2, 8);
    let nodes = [source(1, (1, 9), M, 8, true), machine(2, (9, 9), M, 4, 1)];
    let before = grid.evaluate(&nodes, BOUNDS).unwrap();
    assert!(before.powered(M, 2));
    grid.remove(M, (5, 9));
    let cut = grid.evaluate(&nodes, BOUNDS).unwrap();
    assert_eq!(
        cut.consumer(M, 2).unwrap().status,
        PowerStatus::Disconnected
    );
    assert_eq!(cut.totals(M).allocated, 0);
    grid.lay(M, (5, 9), BOUNDS).unwrap();
    assert_eq!(before, grid.evaluate(&nodes, BOUNDS).unwrap());
}

#[test]
fn lines_can_follow_real_transport_roads_without_replacing_the_cargo_graph() {
    use PowerKind::Mechanical as M;
    let mut kingdom = crate::Kingdom::default();
    kingdom.start(crate::catalog());
    let original_route = kingdom.route((15, 9), 1).unwrap();
    let mut grid = line(M, 9, 8, 15);
    let nodes = [source(2, (8, 8), M, 6, true), machine(3, (15, 8), M, 6, 1)];
    assert!(grid.evaluate(&nodes, BOUNDS).unwrap().powered(M, 3));
    assert_eq!(kingdom.route((15, 9), 1).unwrap(), original_route);
    grid.remove(M, (12, 9));
    assert!(!grid.evaluate(&nodes, BOUNDS).unwrap().powered(M, 3));
    assert_eq!(
        kingdom.route((15, 9), 1).unwrap(),
        original_route,
        "cutting a shaft must not strand a road carrier"
    );
    grid.lay(M, (12, 9), BOUNDS).unwrap();
    kingdom.roads.remove(&(12, 9));
    assert!(kingdom.route((15, 9), 1).is_none());
    assert!(
        grid.evaluate(&nodes, BOUNDS).unwrap().powered(M, 3),
        "a missing road is not a missing shaft"
    );
}

#[test]
fn network_saves_and_terminal_validation_are_bounded_and_old_saves_default_empty() {
    use PowerKind::{Arcane as A, Mechanical as M};
    let empty: PowerGrid = serde_json::from_str("{}").unwrap();
    assert_eq!(empty, PowerGrid::default());
    assert!(empty.valid(BOUNDS));
    let mut grid = line(M, 9, 2, 8);
    grid.lay(A, (4, 9), BOUNDS).unwrap();
    let restored: PowerGrid = serde_json::from_str(&serde_json::to_string(&grid).unwrap()).unwrap();
    assert_eq!(restored, grid);
    let before = grid.clone();
    assert!(grid.lay(M, (2, 9), BOUNDS).is_err());
    assert!(grid.lay(M, (-1, 9), BOUNDS).is_err());
    assert!(grid.lay(M, (72, 9), BOUNDS).is_err());
    assert_eq!(grid, before);
    assert!(!grid.valid((0, 54)));
    grid.shafts.insert((-1, 0));
    assert!(!grid.valid(BOUNDS));
    let mut oversized = PowerGrid::default();
    oversized.shafts = (0..=MAX_CONNECTORS)
        .map(|n| ((n % 100) as i16, (n / 100) as i16))
        .collect();
    assert!(!oversized.valid((100, 100)));
    let base = source(1, (1, 1), M, 4, true);
    for bad in [
        PowerNode { id: 0, ..base },
        PowerNode {
            id: u32::MAX,
            ..base
        },
        PowerNode {
            tile: (i16::MAX, i16::MIN),
            ..base
        },
        PowerNode {
            priority: 3,
            ..base
        },
        PowerNode {
            role: PowerRole::Source {
                capacity: 0,
                active: true,
            },
            ..base
        },
        PowerNode {
            role: PowerRole::Source {
                capacity: MAX_UNITS + 1,
                active: true,
            },
            ..base
        },
    ] {
        assert!(empty.evaluate(&[bad], BOUNDS).is_err());
    }
    assert!(empty.evaluate(&vec![base; MAX_NODES + 1], BOUNDS).is_err());
    assert!(
        empty
            .evaluate(&[base, machine(1, (2, 2), M, 4, 1)], BOUNDS)
            .is_err()
    );
    assert!(
        empty
            .evaluate(&[base, machine(2, (1, 1), M, 4, 1)], BOUNDS)
            .is_err()
    );
    let mut occupied = PowerGrid::default();
    occupied.lay(A, base.tile, BOUNDS).unwrap();
    assert!(
        occupied.evaluate(&[base], BOUNDS).is_err(),
        "a building cannot contain either infrastructure kind"
    );
}
