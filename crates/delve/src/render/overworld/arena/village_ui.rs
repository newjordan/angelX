//! Settlement ledgers display the same named citizens, geography and workshop
//! controls that the authoritative simulation uses.
use super::industry::{fill, panel, small, text, words};
use super::*;
use crate::kingdom::{Terrain, catalog};

pub(super) fn land(im: &mut Img, run: &Run, selection: usize) {
    let k = &run.home.industry;
    text(im, 24, 91, "SURVEY THE VALLEY", 'y');
    for py in 0..3i16 {
        for px in 0..3i16 {
            let x = 24 + i32::from(px) * 142;
            let y = 122 + i32::from(py) * 82;
            panel(im, x, y, 134, 76);
            for ty in 0..18i16 {
                for tx in 0..24i16 {
                    let tile = (px * 24 + tx, py * 18 + ty);
                    let c = match k.world.terrain(tile) {
                        Terrain::Meadow => 0x678257,
                        Terrain::Forest => 0x365a3e,
                        Terrain::Stone => 0x92978b,
                        Terrain::ClayBank => 0xbb8059,
                        Terrain::River => 0x3e788b,
                        Terrain::Ford => 0x77a3a4,
                    };
                    fill(
                        im,
                        x + 6 + i32::from(tx) * 5,
                        y + 4 + i32::from(ty) * 3,
                        5,
                        3,
                        c,
                    );
                }
            }
            if selection == usize::try_from(py * 3 + px).unwrap() {
                fill(im, x, y, 134, 2, 0xe5c481);
                fill(im, x, y, 2, 76, 0xe5c481);
            }
            small(
                im,
                x + 7,
                y + 64,
                if k.world.parcels.contains(&(px, py)) {
                    "SETTLED LAND"
                } else {
                    "SURVEY PARCEL"
                },
                'w',
            );
        }
    }
    let parcel = ((selection % 3) as i16, (selection / 3) as i16);
    if let Ok(offer) = k.expansion_offer(parcel) {
        words(im, 490, 106, &offer.name.to_uppercase(), 21, 2, 'y');
        if let Some(goal) = run.frontier_objective_for(parcel) {
            words(im, 490, 151, &goal.route.label(), 21, 3, 'w');
            words(im, 490, 210, goal.status.label(), 21, 2, 'y');
            words(im, 490, 252, &goal.resource, 21, 1, 't');
        } else {
            words(im, 490, 151, &offer.objective, 21, 7, 'w');
        }
        small(im, 490, 287, "SURVEY SUPPLIES", 't');
        words(
            im,
            490,
            301,
            &crate::kingdom::label(&offer.cost),
            21,
            2,
            'w',
        );
        words(
            im,
            490,
            346,
            offer.blocked.as_deref().unwrap_or("[E] OPEN THIS PARCEL"),
            21,
            2,
            'y',
        );
    } else {
        text(im, 490, 110, "LAND OF YOUR OWN", 'y');
        words(
            im,
            490,
            149,
            "Build on your owned land. Orange banks supply clay, pale rock supplies stone and woodland supplies timber.",
            21,
            7,
            'w',
        );
        words(
            im,
            490,
            286,
            "Roads cross shallow fords. Deep water needs a timber bridge: 4 planks and 2 stone.",
            21,
            5,
            't',
        );
    }
    small(
        im,
        24,
        373,
        if k.survey_goal == Some(parcel) {
            "PINNED: YOUR EXPEDITIONS WILL TRACK THIS PARCEL'S RESOURCE"
        } else {
            "[2] PIN THIS PARCEL AS YOUR EXPEDITION OBJECTIVE"
        },
        't',
    );
}

pub(super) fn works(im: &mut Img, run: &Run, selection: usize) {
    let k = &run.home.industry;
    let c = catalog();
    let report = k.report(c);
    text(im, 24, 91, "THE WORKSHOP LEDGER", 'y');
    let energy = k.energy_report(c);
    let mechanical = energy.totals(crate::kingdom::PowerKind::Mechanical);
    let arcane = energy.totals(crate::kingdom::PowerKind::Arcane);
    small(
        im,
        24,
        111,
        &format!(
            "POWER SERVED: SHAFT {}/{}  /  LEY {}/{}",
            mechanical.allocated, mechanical.requested, arcane.allocated, arcane.requested
        ),
        't',
    );
    let start = selection.saturating_sub(4);
    for (i, (&id, b)) in k.buildings.iter().enumerate().skip(start).take(5) {
        let y = 122 + (i - start) as i32 * 48;
        panel(im, 24, y, 424, 43);
        if i == selection {
            fill(im, 24, y, 3, 43, 0xd6bd79);
        }
        text(im, 35, y + 5, &c.plans[&b.plan].name, 'w');
        small(
            im,
            35,
            y + 28,
            &format!(
                "{} / {} PRIORITY",
                report.status[&id].label(),
                ["LOW", "NORMAL", "HIGH"][usize::from(b.priority)]
            ),
            't',
        );
    }
    if let Some((&id, b)) = k.buildings.iter().nth(selection) {
        words(
            im,
            490,
            106,
            &c.plans[&b.plan].name.to_uppercase(),
            21,
            2,
            'y',
        );
        text(
            im,
            490,
            153,
            &format!(
                "[E] {} PRIORITY",
                ["LOW", "NORMAL", "HIGH"][usize::from(b.priority)]
            ),
            'w',
        );
        text(
            im,
            490,
            180,
            &format!(
                "[2] TARGET {}",
                if b.production_limit == 0 {
                    "NONE".into()
                } else {
                    b.production_limit.to_string()
                }
            ),
            'w',
        );
        if let Some(power) = c.plans[&b.plan].power.as_ref() {
            let control = if power.is_source() {
                if b.power_enabled {
                    "[1] STOP GENERATOR"
                } else {
                    "[1] START GENERATOR"
                }
            } else if !b.power_upgrade {
                "[1] FIT POWER DRIVE"
            } else if b.power_enabled {
                "[1] USE MANUAL WORK"
            } else {
                "[1] USE POWER DRIVE"
            };
            text(im, 490, 210, control, 'w');
            let detail = if power.is_source() {
                if power.fuel.is_empty() {
                    format!(
                        "{} capacity: {}. Needs a road and a worker.",
                        power.kind.name(),
                        power.capacity
                    )
                } else {
                    format!(
                        "Capacity {}. Fuel: {} per {}s. Charge: {}s.",
                        power.capacity,
                        crate::kingdom::label(&power.fuel),
                        power.fuel_seconds,
                        b.fuel_left
                    )
                }
            } else if !b.power_upgrade {
                format!(
                    "Fit: {}. Needs {} {} power for {}% speed.",
                    crate::kingdom::label(&power.upgrade_cost),
                    power.demand,
                    power.kind.name(),
                    power.multiplier
                )
            } else if !b.power_enabled {
                "Manual work. Switch the drive on when the network is ready.".into()
            } else {
                energy.consumer(power.kind, id).map_or_else(
                    || "Waiting for construction.".into(),
                    |p| {
                        format!(
                            "{}. Draw {}/{}; {}% powered speed.",
                            p.status.label(),
                            p.allocated,
                            power.demand,
                            power.multiplier
                        )
                    },
                )
            };
            words(im, 490, 236, &detail, 21, 4, 't');
        } else {
            words(
                im,
                490,
                211,
                "High priority gets workers and goods first. Targets count stores, local buffers and cargo.",
                21,
                6,
                't',
            );
        }
        small(
            im,
            490,
            326,
            &format!(
                "ASSIGNED {} / {} WORKERS",
                report.population.assigned.get(&id).copied().unwrap_or(0),
                c.plans[&b.plan].workers
            ),
            'y',
        );
        small(
            im,
            490,
            342,
            &format!(
                "EFFECTIVE WORK {}%",
                u32::from(report.population.efficiency.get(&id).copied().unwrap_or(0))
                    * u32::from(k.energy_multiplier(c, id, &energy))
                    / 100
            ),
            'w',
        );
        small(im, 490, 360, "[F] PAUSE / RESUME", 'w');
    }
    small(
        im,
        24,
        373,
        &format!(
            "[3] RESERVE {} MEALS EACH / {} FOOD BEFORE RATIONS",
            k.food_reserve.unwrap_or(2),
            k.food_delivery_target(c)
        ),
        'y',
    );
}

pub(super) fn people(im: &mut Img, run: &Run, selection: usize) {
    let k = &run.home.industry;
    let report = k.report(catalog()).population;
    text(im, 24, 91, "PEOPLE OF THE FRONTIER", 'y');
    small(
        im,
        24,
        115,
        &format!(
            "{} RESIDENTS / {} BEDS / {} NEED FOOD",
            report.population, report.beds, report.hungry
        ),
        't',
    );
    let start = selection.saturating_sub(4);
    for (i, p) in report.citizens.iter().enumerate().skip(start).take(5) {
        let y = 135 + (i - start) as i32 * 44;
        panel(im, 24, y, 424, 40);
        if i == selection {
            fill(im, 24, y, 3, 40, 0xd6bd79);
        }
        text(im, 35, y + 4, &p.name, 'w');
        small(
            im,
            35,
            y + 26,
            &format!(
                "{} / {} / {}%",
                p.role.label(),
                p.activity.label(),
                p.efficiency
            ),
            't',
        );
    }
    if let Some(p) = report.citizens.get(selection) {
        words(im, 490, 106, &p.name.to_uppercase(), 21, 2, 'y');
        for (y, label, value) in [
            (155, "FED", 100 - p.hunger),
            (182, "REST", p.rest),
            (209, "MORALE", p.morale),
        ] {
            small(im, 490, y, label, 't');
            fill(im, 548, y, 184, 7, 0x112023);
            fill(
                im,
                548,
                y,
                184 * i32::from(value) / 100,
                7,
                if value < 35 { 0xb96b4e } else { 0x9dbb78 },
            );
        }
        let home = p
            .home
            .and_then(|id| k.buildings.get(&id))
            .map_or("No home", |b| catalog().plans[&b.plan].name.as_str());
        let job = p
            .job
            .and_then(|id| k.buildings.get(&id))
            .map_or("Awaiting work", |b| catalog().plans[&b.plan].name.as_str());
        words(
            im,
            490,
            244,
            &format!("Home: {home}. Work: {job}."),
            21,
            4,
            'w',
        );
    }
    words(
        im,
        490,
        321,
        "Spare beds and food welcome newcomers.",
        21,
        3,
        't',
    );
    small(
        im,
        24,
        373,
        "EACH RESIDENT EATS 1 HARVEST EVERY 90 SECONDS. PRIORITIES ASSIGN JOBS.",
        't',
    );
}
