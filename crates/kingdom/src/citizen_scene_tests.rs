//! Authoritative settlement commands exercise actor migration and construction.
use super::*;

fn build(k: &mut Kingdom, plan: &str, tile: Tile) -> u32 {
    k.apply(
        catalog(),
        Command::Build {
            plan: plan.into(),
            tile,
        },
    )
    .unwrap();
    k.at(tile).unwrap()
}
fn walk_steps(k: &mut Kingdom, seconds: usize) {
    for _ in 0..seconds {
        let before = k.motion.actors.clone();
        k.advance_second(catalog());
        assert!(k.valid(catalog()));
        for (&id, actor) in &k.motion.actors {
            if let Some(old) = before.get(&id) {
                let distance =
                    old.tile.0.abs_diff(actor.tile.0) + old.tile.1.abs_diff(actor.tile.1);
                assert!(
                    distance <= 1,
                    "citizen {id} teleported from {:?} to {:?}",
                    old.tile,
                    actor.tile
                );
            }
        }
    }
}

#[test]
fn demolition_reassigns_the_same_person_without_teleport_and_replays_after_reload() {
    let mut k = Kingdom::default();
    k.start(catalog());
    let farm = build(&mut k, "farm", (8, 8));
    build(&mut k, "forester", (13, 8));
    walk_steps(&mut k, 30);
    let (&person, citizen) = k
        .population
        .citizens
        .iter()
        .find(|(_, p)| p.job == Some(farm))
        .unwrap();
    let name = citizen.name.clone();
    let before = k.motion.actors[&person].tile;
    assert!(k.motion.actors[&person].working);
    assert!(
        k.at(before).is_none(),
        "workers should stand at an entrance, not in a wall"
    );
    k.apply(catalog(), Command::Demolish((8, 8))).unwrap();
    assert_eq!(k.motion.actors[&person].tile, before);
    assert_eq!(k.population.citizens[&person].name, name);
    assert_ne!(k.population.citizens[&person].job, Some(farm));
    let mut replay: Kingdom = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
    for _ in 0..100 {
        walk_steps(&mut k, 1);
        walk_steps(&mut replay, 1);
        assert_eq!(k, replay);
    }
    let actor = &k.motion.actors[&person];
    assert_eq!(actor.tile, actor.destination);
    assert!(!actor.blocked);
}

#[test]
fn a_building_started_under_a_citizen_preserves_the_save_and_allows_adjacent_escape() {
    let mut k = Kingdom::default();
    k.start(catalog());
    let farm = build(&mut k, "farm", (8, 8));
    walk_steps(&mut k, 30);
    let person = *k
        .population
        .citizens
        .iter()
        .find(|(_, p)| p.job == Some(farm))
        .unwrap()
        .0;
    let position = k.motion.actors[&person].tile;
    assert_eq!(position, (8, 9));
    k.apply(catalog(), Command::Demolish(position)).unwrap();
    assert_eq!(k.motion.actors[&person].tile, position);
    let cottage = build(&mut k, "cottage", position);
    assert_eq!(
        k.motion.actors[&person].tile, position,
        "starting construction must not teleport its occupant"
    );
    assert!(
        k.valid(catalog()),
        "stale current passability is a recoverable route state"
    );
    let mut resumed: Kingdom = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
    walk_steps(&mut k, 1);
    walk_steps(&mut resumed, 1);
    assert_eq!(k, resumed);
    assert_ne!(k.motion.actors[&person].tile, position);
    assert!(!k.motion.actors[&person].blocked);
    walk_steps(&mut k, 30);
    assert_eq!(k.buildings[&cottage].construction, 0);
    assert!(k.motion.actors.values().all(|a| {
        k.at(a.tile)
            .is_none_or(|id| catalog().plans[&k.buildings[&id].plan].passable)
    }));
}
