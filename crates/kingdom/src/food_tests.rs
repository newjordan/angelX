//! Full economy scenarios: every input is grown, processed and delivered.
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
fn wait(k: &mut Kingdom, limit: usize, goal: impl Fn(&Kingdom) -> bool) {
    for _ in 0..limit {
        if goal(k) {
            return;
        }
        k.advance_second(catalog());
        assert!(k.valid(catalog()));
    }
    panic!(
        "food economy stalled: stock={:?}; population={:?}",
        k.stock, k.population
    );
}
fn kitchen_village() -> Kingdom {
    let mut k = Kingdom::default();
    k.found(catalog(), 41);
    build(&mut k, "forester", (8, 8));
    build(&mut k, "farm", (10, 8));
    build(&mut k, "sawmill", (12, 8));
    wait(&mut k, 300, |k| k.amount("plank") >= 6);
    let kitchen = build(&mut k, "kitchen", (14, 8));
    wait(&mut k, 120, |k| k.buildings[&kitchen].construction == 0);
    assert_eq!(k.population.citizens.len(), 6);
    assert!(k.population.citizens.values().all(|p| p.hunger == 0));
    k
}

#[test]
fn two_meal_reserve_feeds_citizens_and_makes_rations_from_real_farm_deliveries() {
    let mut k = kitchen_village();
    assert_eq!(k.reserved_food(), 12);
    for _ in 0..1800 {
        k.advance_second(catalog());
        assert!(k.valid(catalog()));
        assert!(
            k.population.citizens.values().all(|p| p.hunger == 0),
            "food was taken from the protected meal reserve at second {}",
            k.second
        );
    }
    assert!(k.produced.get("rations").copied().unwrap_or(0) >= 40);
    assert!(k.delivered.get("rations").copied().unwrap_or(0) >= 20);
    assert!(k.delivered.get("food").copied().unwrap_or(0) > 0);
    assert_eq!(k.population.citizens.len(), 6);
    let harvested = k.produced["food"];
    let packed = k.produced["rations"] / 2;
    let eaten = (k.population.seconds / u64::from(population::MEAL_SECONDS)) as u32 * 6;
    assert_eq!(
        k.total("food"),
        catalog().starter["food"] + harvested - packed * 3 - eaten,
        "meals, recipes, storage and cargo must conserve every food unit"
    );
}

#[test]
fn zero_reserve_has_observable_output_and_hunger_tradeoff_without_free_food() {
    let mut protected = kitchen_village();
    let mut unreserved = protected.clone();
    unreserved
        .apply(catalog(), Command::FoodReserve(0))
        .unwrap();
    assert_eq!(unreserved.reserved_food(), 0);
    let mut hungry_seconds = 0;
    let mut ration_advantage = false;
    for _ in 0..900 {
        protected.advance_second(catalog());
        unreserved.advance_second(catalog());
        assert!(
            protected
                .population
                .citizens
                .values()
                .all(|p| p.hunger == 0)
        );
        hungry_seconds += usize::from(
            unreserved
                .population
                .citizens
                .values()
                .any(|p| p.hunger > 0),
        );
        ration_advantage |= unreserved.produced.get("rations").copied().unwrap_or(0)
            > protected.produced.get("rations").copied().unwrap_or(0);
        assert!(unreserved.valid(catalog()));
    }
    assert!(
        ration_advantage,
        "releasing the food reserve must allow extra early packing"
    );
    assert!(
        hungry_seconds > 90,
        "unchecked kitchen consumption should have a visible needs cost"
    );
    let before = unreserved.clone();
    assert!(
        unreserved
            .apply(catalog(), Command::FoodReserve(4))
            .is_err()
    );
    assert_eq!(unreserved, before);
    // Reinstating the reserve permits farm output to refill the meal store.
    unreserved
        .apply(catalog(), Command::FoodReserve(2))
        .unwrap();
    wait(&mut unreserved, 900, |k| {
        k.population.citizens.values().all(|p| p.hunger == 0)
    });
}

#[test]
fn existing_kitchen_allows_gradual_arrivals_and_reserve_tracks_every_new_mouth() {
    let mut k = kitchen_village();
    let cottage = build(&mut k, "cottage", (10, 10));
    wait(&mut k, 120, |k| k.buildings[&cottage].construction == 0);
    wait(&mut k, 1200, |k| k.population.citizens.len() > 6);
    assert_eq!(k.reserved_food(), k.population.citizens.len() as u32 * 2);
    let before_rations = k.produced.get("rations").copied().unwrap_or(0);
    for _ in 0..900 {
        k.advance_second(catalog());
        assert!(k.population.citizens.values().all(|p| p.hunger == 0));
        assert_eq!(k.reserved_food(), k.population.citizens.len() as u32 * 2);
        assert!(k.valid(catalog()));
    }
    assert!(k.produced["rations"] > before_rations);
    assert!(
        k.population.citizens.len() <= 10,
        "migration stays within actual beds"
    );
}
