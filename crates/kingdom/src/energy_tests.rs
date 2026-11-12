use super::*;

fn fixture(kind: PowerKind, fueled: bool) -> (Catalog, Kingdom, u32, u32) {
    let mut c = catalog().clone();
    c.id = "energy-fixture".into();
    c.starter.insert("plank".into(), 64);
    c.starter.insert("mana".into(), 16);
    for (id, kind, resource) in [
        ("drive-shaft", PowerKind::Mechanical, "plank"),
        ("leyline", PowerKind::Arcane, "mana"),
    ] {
        let mut plan = c.plans.get(id).unwrap_or(&c.plans["farm"]).clone();
        plan.connector = Some(kind);
        plan.power = None;
        plan.recipes.clear();
        plan.requires.clear();
        plan.extractor = false;
        plan.build_seconds = 0;
        plan.workers = 0;
        plan.housing = 0;
        plan.storage = 0;
        plan.carriers = 0;
        plan.passable = true;
        plan.siting = Siting::Any;
        plan.cost = Stock::from([(resource.into(), 1)]);
        c.plans.insert(id.into(), plan);
    }
    let wheel = c.plans.get_mut("waterwheel").unwrap();
    wheel.cost.clear();
    wheel.requires.clear();
    wheel.recipes.clear();
    wheel.build_seconds = 0;
    wheel.workers = 1;
    wheel.siting = Siting::Any;
    wheel.power = Some(MachinePower {
        kind,
        capacity: 8,
        fuel: if fueled {
            Stock::from([("mana".into(), 2)])
        } else {
            Stock::new()
        },
        fuel_seconds: if fueled { 3 } else { 0 },
        ..Default::default()
    });
    let saw = c.plans.get_mut("sawmill").unwrap();
    saw.cost.clear();
    saw.requires.clear();
    saw.build_seconds = 0;
    saw.workers = 1;
    saw.power = Some(MachinePower {
        kind,
        demand: 4,
        multiplier: 200,
        upgrade_cost: Stock::from([("plank".into(), 4)]),
        ..Default::default()
    });
    c.validate().unwrap();
    let mut k = Kingdom::default();
    k.start(&c);
    k.apply(
        &c,
        Command::Build {
            plan: "waterwheel".into(),
            tile: (8, 8),
        },
    )
    .unwrap();
    k.apply(
        &c,
        Command::Build {
            plan: "sawmill".into(),
            tile: (15, 8),
        },
    )
    .unwrap();
    let source = k.at((8, 8)).unwrap();
    let consumer = k.at((15, 8)).unwrap();
    k.buildings
        .get_mut(&consumer)
        .unwrap()
        .input
        .insert("timber".into(), 24);
    for x in 8..=15 {
        k.lay_power(&c, kind, (x, 9)).unwrap();
    }
    (c, k, source, consumer)
}

#[test]
fn energy_catalog_requires_one_role_and_bounded_source_only_fuel() {
    let c = catalog();
    let source = MachinePower {
        capacity: 8,
        ..Default::default()
    };
    let consumer = MachinePower {
        demand: 4,
        multiplier: 200,
        ..Default::default()
    };
    assert!(source.valid(c) && consumer.valid(c));
    assert!(!MachinePower::default().valid(c));
    assert!(
        !MachinePower {
            demand: 1,
            ..source.clone()
        }
        .valid(c)
    );
    assert!(
        !MachinePower {
            fuel: Stock::from([("mana".into(), 1)]),
            ..consumer.clone()
        }
        .valid(c)
    );
    assert!(
        !MachinePower {
            fuel: Stock::from([("mana".into(), 1)]),
            fuel_seconds: 0,
            ..source.clone()
        }
        .valid(c)
    );
    assert!(
        !MachinePower {
            fuel_seconds: 5,
            ..source.clone()
        }
        .valid(c)
    );
    assert!(
        !MachinePower {
            upgrade_cost: Stock::from([("plank".into(), 1)]),
            ..source
        }
        .valid(c)
    );
    assert!(
        !MachinePower {
            multiplier: 301,
            ..consumer
        }
        .valid(c)
    );
}

#[test]
fn energy_upgrades_are_opt_in_paid_once_and_can_return_to_manual_work() {
    let (c, mut k, _, consumer) = fixture(PowerKind::Mechanical, false);
    let report = k.energy_report(&c);
    assert_eq!(k.energy_multiplier(&c, consumer, &report), 100);
    assert!(report.consumer(PowerKind::Mechanical, consumer).is_none());
    let plank = k.amount("plank");
    k.enable_power(&c, consumer).unwrap();
    assert_eq!(k.amount("plank"), plank - 4);
    assert!(k.energy_report(&c).powered(PowerKind::Mechanical, consumer));
    assert_eq!(k.energy_multiplier(&c, consumer, &k.energy_report(&c)), 200);
    k.enable_power(&c, consumer).unwrap();
    assert_eq!(k.energy_multiplier(&c, consumer, &k.energy_report(&c)), 100);
    k.enable_power(&c, consumer).unwrap();
    assert_eq!(
        k.amount("plank"),
        plank - 4,
        "re-enabling cannot charge for the same upgrade twice"
    );
    assert!(k.energy_valid(&c));
}

#[test]
fn energy_sources_burn_only_local_fuel_and_reports_never_consume_it() {
    let (c, mut k, source, consumer) = fixture(PowerKind::Arcane, true);
    k.enable_power(&c, consumer).unwrap();
    let global = k.amount("mana");
    k.energy_tick(&c);
    assert_eq!(k.amount("mana"), global);
    assert_eq!(
        k.buildings[&source].fuel_left, 0,
        "camp mana is not delivered engine fuel"
    );
    k.buildings
        .get_mut(&source)
        .unwrap()
        .input
        .insert("mana".into(), 6);
    let before = k.clone();
    for _ in 0..4 {
        k.energy_report(&c);
    }
    assert_eq!(k, before);
    k.energy_tick(&c);
    assert_eq!(k.buildings[&source].input["mana"], 4);
    assert_eq!(k.buildings[&source].fuel_left, 3);
    assert!(k.energy_report(&c).powered(PowerKind::Arcane, consumer));
    k.energy_tick(&c);
    k.energy_tick(&c);
    assert_eq!(k.buildings[&source].fuel_left, 1);
    assert_eq!(k.buildings[&source].input["mana"], 4);
    k.energy_tick(&c);
    assert_eq!(k.buildings[&source].fuel_left, 3);
    assert_eq!(k.buildings[&source].input["mana"], 2);
    assert_eq!(k.amount("mana"), global);
}

#[test]
fn energy_one_second_fuel_provides_a_full_interval_and_pausing_preserves_charge() {
    let (mut c, mut k, source, consumer) = fixture(PowerKind::Arcane, true);
    c.plans
        .get_mut("waterwheel")
        .unwrap()
        .power
        .as_mut()
        .unwrap()
        .fuel_seconds = 1;
    k.buildings
        .get_mut(&source)
        .unwrap()
        .input
        .insert("mana".into(), 2);
    k.enable_power(&c, consumer).unwrap();
    k.energy_tick(&c);
    assert_eq!(k.buildings[&source].fuel_left, 1);
    assert!(k.energy_report(&c).powered(PowerKind::Arcane, consumer));
    k.buildings.get_mut(&source).unwrap().paused = true;
    for _ in 0..4 {
        k.energy_tick(&c);
    }
    assert_eq!(k.buildings[&source].fuel_left, 1);
    assert!(!k.energy_report(&c).powered(PowerKind::Arcane, consumer));
    k.buildings.get_mut(&source).unwrap().paused = false;
    k.energy_tick(&c);
    assert_eq!(k.buildings[&source].fuel_left, 0);
    assert!(!k.energy_report(&c).powered(PowerKind::Arcane, consumer));
}

#[test]
fn energy_unstaffed_sources_and_construction_blueprints_supply_nothing() {
    let (mut c, mut k, source, consumer) = fixture(PowerKind::Arcane, true);
    k.buildings
        .get_mut(&source)
        .unwrap()
        .input
        .insert("mana".into(), 4);
    c.plans.get_mut("waterwheel").unwrap().workers = 7;
    k.enable_power(&c, consumer).unwrap();
    k.energy_tick(&c);
    assert_eq!(k.buildings[&source].input["mana"], 4);
    assert_eq!(
        k.energy_report(&c)
            .totals(PowerKind::Arcane)
            .active_capacity,
        0
    );
    c.plans.get_mut("waterwheel").unwrap().workers = 1;
    c.plans.get_mut("waterwheel").unwrap().build_seconds = 1;
    k.buildings.get_mut(&source).unwrap().construction = 1;
    assert_eq!(k.energy_report(&c).totals(PowerKind::Arcane).sources, 0);
}

#[test]
fn energy_cut_and_repair_preserve_recipe_inputs_and_real_jobs_resume() {
    let (c, mut k, _, consumer) = fixture(PowerKind::Mechanical, false);
    k.enable_power(&c, consumer).unwrap();
    let input = k.buildings[&consumer].input.clone();
    let progress = k.buildings[&consumer].progress;
    k.remove_power(PowerKind::Mechanical, (12, 9)).unwrap();
    for _ in 0..30 {
        k.advance_second(&c);
    }
    assert_eq!(k.buildings[&consumer].input, input);
    assert_eq!(k.buildings[&consumer].progress, progress);
    assert_eq!(k.energy_multiplier(&c, consumer, &k.energy_report(&c)), 0);
    k.lay_power(&c, PowerKind::Mechanical, (12, 9)).unwrap();
    for _ in 0..30 {
        k.advance_second(&c);
    }
    assert!(k.buildings[&consumer].input["timber"] < input["timber"]);
    assert!(k.produced.get("plank").copied().unwrap_or(0) > 0);
}

#[test]
fn energy_cut_stops_the_same_workers_pose_and_repair_resumes_without_teleporting() {
    let (c, mut k, source, consumer) = fixture(PowerKind::Mechanical, false);
    k.apply(&c, Command::EnablePower(consumer)).unwrap();
    for _ in 0..12 { k.advance_second(&c); }
    let worker = k.population.citizens.iter()
        .find(|(_, p)| p.job == Some(consumer)).map(|(&id, _)| id).unwrap();
    let miller = k.population.citizens.iter()
        .find(|(_, p)| p.job == Some(source)).map(|(&id, _)| id).unwrap();
    let place = k.motion.actors[&worker].tile;
    assert!(k.motion.actors[&worker].working && !k.motion.actors[&worker].walking);
    assert!(k.motion.actors[&miller].working);
    k.apply(&c, Command::RemovePower { kind: PowerKind::Mechanical, tile: (12, 9) }).unwrap();
    assert_eq!(k.report(&c).status[&consumer], Status::PowerDisconnected);
    assert!(!k.motion.actors[&worker].working);
    assert_eq!(k.motion.actors[&worker].tile, place);
    let made = k.produced.get("plank").copied().unwrap_or(0);
    let mut restored: Kingdom = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
    for _ in 0..10 {
        k.advance_second(&c);
        restored.advance_second(&c);
    }
    assert_eq!(k, restored);
    assert_eq!(k.population.citizens[&worker].job, Some(consumer));
    assert_eq!(k.motion.actors[&worker].tile, place);
    assert!(!k.motion.actors[&worker].working);
    assert!(k.motion.actors[&miller].working);
    assert_eq!(k.produced.get("plank").copied().unwrap_or(0), made);
    k.apply(&c, Command::PowerLine { kind: PowerKind::Mechanical, tile: (12, 9) }).unwrap();
    assert!(k.motion.actors[&worker].working);
    assert_eq!(k.motion.actors[&worker].tile, place);
    for _ in 0..10 { k.advance_second(&c); }
    assert!(k.produced.get("plank").copied().unwrap_or(0) > made);
    assert!(k.valid(&c));
}

#[test]
fn energy_speed_bonus_increases_real_output_and_pays_inputs_for_every_batch() {
    let (c, mut manual, _, consumer) = fixture(PowerKind::Mechanical, false);
    let mut powered = manual.clone();
    powered.enable_power(&c, consumer).unwrap();
    let timber = powered.total("timber");
    for _ in 0..40 {
        manual.advance_second(&c);
        powered.advance_second(&c);
        assert!(manual.valid(&c) && powered.valid(&c));
    }
    let plain = manual.produced.get("plank").copied().unwrap_or(0);
    let boosted = powered.produced.get("plank").copied().unwrap_or(0);
    assert!(
        plain > 0 && boosted >= plain * 2,
        "200% power must finish extra paid batches, not accumulate unusable work credit"
    );
    assert_eq!(
        timber - powered.total("timber"),
        boosted / 3 * 2,
        "every 3 planks still consume 2 timber through the actual recipe"
    );
}

#[test]
fn energy_connectors_charge_once_share_roads_and_cross_paid_bridges() {
    let (c, mut k, _, _) = fixture(PowerKind::Mechanical, false);
    let before = k.clone();
    assert!(k.lay_power(&c, PowerKind::Mechanical, (8, 9)).is_err());
    assert!(k.lay_power(&c, PowerKind::Mechanical, (8, 8)).is_err());
    assert!(k.lay_power(&c, PowerKind::Mechanical, (30, 9)).is_err());
    assert_eq!(k, before);
    let road = k.route((15, 9), 1).unwrap();
    k.remove_power(PowerKind::Mechanical, (12, 9)).unwrap();
    assert_eq!(k.route((15, 9), 1).unwrap(), road);
    k.world = World::new(41);
    let river = (0..18)
        .flat_map(|y| (0..24).map(move |x| (x, y)))
        .find(|&t| k.world.terrain(t) == Terrain::River)
        .unwrap();
    let before = k.clone();
    assert!(k.lay_power(&c, PowerKind::Mechanical, river).is_err());
    assert_eq!(k, before);
    k.apply(&c, Command::Bridge(river)).unwrap();
    k.lay_power(&c, PowerKind::Mechanical, river).unwrap();
    assert!(k.bridges.contains(&river) && k.roads.contains(&river));
    assert!(k.energy_valid(&c));
}

#[test]
fn energy_fields_migrate_and_reject_impossible_charges_or_hidden_lines() {
    let old: Building = serde_json::from_str("{}").unwrap();
    assert!(!old.power_upgrade && old.power_enabled && old.fuel_left == 0);
    let (c, mut k, source, consumer) = fixture(PowerKind::Arcane, true);
    assert!(k.energy_valid(&c));
    k.buildings.get_mut(&source).unwrap().fuel_left = 4;
    assert!(!k.energy_valid(&c));
    k.buildings.get_mut(&source).unwrap().fuel_left = 0;
    k.power.leylines.insert(k.buildings[&consumer].tile);
    assert!(!k.energy_valid(&c));
    k.power.leylines.remove(&k.buildings[&consumer].tile);
    k.enable_power(&c, consumer).unwrap();
    let restored: Kingdom = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
    assert_eq!(k, restored);
    assert_eq!(k.energy_report(&c), restored.energy_report(&c));
}
