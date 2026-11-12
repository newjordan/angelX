//! A real seeded settlement. Placement uses the same keyboard-shaped Input
//! as the client; long waits advance the complete authoritative Run.
use super::*;
use crate::kingdom::{Stock, catalog};
use std::{collections::BTreeMap, path::Path};

fn step(run: &mut Run, input: Input) {
    run.step(&BTreeMap::from([(1, input)]));
}
fn press(run: &mut Run, input: Input) {
    step(run, input);
    step(run, Input::default());
}
fn plan(run: &mut Run, id: &str) {
    let target = catalog().menu.iter().position(|p| p == id).unwrap() + 1;
    for _ in 0..catalog().menu.len() + 2 {
        if run.kingdom.industry[&1].plan == target {
            return;
        }
        press(
            run,
            Input {
                swing: true,
                ..Default::default()
            },
        );
    }
    panic!("could not select {id}");
}
fn plot(run: &mut Run, target: (i16, i16)) {
    for _ in 0..1000 {
        let current = run.kingdom.industry[&1].tile;
        if current == target {
            return;
        }
        step(
            run,
            Input {
                move_x: (target.0 - current.0).signum() as i8,
                move_y: if current.0 == target.0 {
                    (target.1 - current.1).signum() as i8
                } else {
                    0
                },
                ..Default::default()
            },
        );
    }
    panic!("could not reach plot {target:?}");
}
fn wait(run: &mut Run, cost: &Stock) {
    for _ in 0..900 * u32::from(HZ) {
        if cost.iter().all(|(r, n)| run.home.industry.amount(r) >= *n) {
            return;
        }
        step(run, Input::default());
    }
    panic!("village stalled: {:?}", run.home.industry.stock);
}
fn build(run: &mut Run, id: &str, tile: (i16, i16)) {
    wait(run, &catalog().plans[id].cost);
    plan(run, id);
    plot(run, tile);
    press(
        run,
        Input {
            interact: true,
            take: true,
            ..Default::default()
        },
    );
    assert_eq!(
        run.home.industry.buildings[&run.home.industry.at(tile).unwrap()].plan,
        id
    );
}
fn shot(run: &Run, dest: &Path, name: &str) {
    let im = crate::render::overworld::arena::frame_for(run, 1152, 672, Some(1));
    image::save_buffer(
        dest.join(format!("{name}.png")),
        &im.rgba_bytes(),
        im.w as u32,
        im.h as u32,
        image::ColorType::Rgba8,
    )
    .unwrap();
}

fn village_review(dest: &Path) -> Run {
    std::fs::create_dir_all(dest).unwrap();
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    press(
        &mut run,
        Input {
            cast: 4,
            ..Default::default()
        },
    );
    shot(&run, dest, "01-foundation");
    build(&mut run, "forester", (8, 8));
    build(&mut run, "farm", (10, 8));
    build(&mut run, "quarry", (15, 8));
    build(&mut run, "sawmill", (12, 8));
    build(&mut run, "clay-pit", (8, 10));
    build(&mut run, "brick-kiln", (13, 8));
    build(&mut run, "brick-cottage", (10, 10));
    shot(&run, dest, "02-cottage-foundation");
    for _ in 0..12 * u32::from(HZ) {
        step(&mut run, Input::default());
    }
    shot(&run, dest, "03-cottage-walls");
    build(&mut run, "brick-gate", (12, 9));
    build(&mut run, "brick-wall", (12, 10));
    build(&mut run, "brick-wall", (12, 11));
    build(&mut run, "brick-wall", (11, 11));
    build(&mut run, "brick-wall", (10, 11));
    build(&mut run, "brick-wall", (9, 11));
    for _ in 0..150 * u32::from(HZ) {
        step(&mut run, Input::default());
    }
    assert!(run.home.industry.population.citizens.len() > 6);
    assert!(run.valid_snapshot());
    plot(&mut run, (11, 9));
    press(
        &mut run,
        Input {
            dash: true,
            ..Default::default()
        },
    );
    shot(&run, dest, "04-brick-village");
    for page in 1..=8 {
        press(
            &mut run,
            Input {
                cycle: true,
                ..Default::default()
            },
        );
        assert_eq!(run.kingdom.industry[&1].page, page);
        if page >= 6 {
            shot(&run, dest, &format!("0{page}-ledger"));
        }
    }
    let saved = serde_json::to_string(&run).unwrap();
    let restored: Run = serde_json::from_str(&saved).unwrap();
    assert!(restored.valid_snapshot());
    assert_eq!(run.home.industry, restored.home.industry);
    std::fs::write(dest.join("run.json"), saved).unwrap();
    press(
        &mut run,
        Input {
            ride: true,
            ..Default::default()
        },
    );
    assert!(run.kingdom.frontier);
    for _ in 0..32 {
        step(
            &mut run,
            Input {
                move_y: -1,
                ..Default::default()
            },
        );
    }
    shot(&run, dest, "09-walk");
    println!(
        "Village review: {} seconds, {} residents, {} bricks delivered; normal input and full Run ticks",
        run.home.industry.second,
        run.home.industry.population.citizens.len(),
        run.home
            .industry
            .delivered
            .get("brick")
            .copied()
            .unwrap_or(0)
    );
    run
}

#[test]
#[ignore = "writes actual-input village progression and animation frames"]
fn write_village_review() {
    let folder =
        std::env::var("ANGEL_VILLAGE_SHOTS").unwrap_or_else(|_| "/tmp/village-review".into());
    village_review(Path::new(&folder));
}

fn pointer(run: &mut Run, action: crate::rules::builder_input::BuilderAction) {
    step(
        run,
        Input {
            builder: Some(action),
            ..Default::default()
        },
    );
}
fn workshop(run: &mut Run, id: u32) {
    use crate::rules::builder_input::BuilderAction;
    pointer(run, BuilderAction::Page(8));
    let index = run
        .home
        .industry
        .buildings
        .keys()
        .position(|&key| key == id)
        .unwrap();
    pointer(run, BuilderAction::Selection(index as u16));
}
fn seconds(run: &mut Run, n: u32) {
    for _ in 0..n * u32::from(HZ) {
        step(run, Input::default());
    }
}

#[test]
#[ignore = "writes a paid waterwheel progression, severed shaft and restored production"]
fn write_powered_village_review() {
    use crate::kingdom::{PowerKind, Status};
    use crate::rules::builder_input::BuilderAction;
    let folder =
        std::env::var("ANGEL_POWER_SHOTS").unwrap_or_else(|_| "/tmp/powered-village-review".into());
    let dest = Path::new(&folder);
    let mut run = village_review(dest);
    press(
        &mut run,
        Input {
            cast: 4,
            ..Default::default()
        },
    );
    pointer(&mut run, BuilderAction::Page(1));
    let charter = catalog()
        .technologies
        .keys()
        .position(|t| t == "masonry")
        .unwrap();
    pointer(&mut run, BuilderAction::Selection(charter as u16));
    pointer(&mut run, BuilderAction::Confirm);
    assert!(run.home.industry.researched.contains("masonry"));
    // The player reallocates two workers now that the first brick home is built.
    for name in ["clay-pit", "brick-kiln"] {
        let id = run
            .home
            .industry
            .buildings
            .iter()
            .find(|(_, b)| b.plan == name)
            .unwrap()
            .0
            .to_owned();
        workshop(&mut run, id);
        press(
            &mut run,
            Input {
                fire: true,
                ..Default::default()
            },
        );
        assert!(run.home.industry.buildings[&id].paused);
    }
    pointer(&mut run, BuilderAction::Page(0));
    build(&mut run, "mason", (14, 8));
    wait(&mut run, &Stock::from([("block".into(), 6)]));
    let wheel = (16..24)
        .map(|x| (x, 8))
        .find(|&tile| {
            run.home
                .industry
                .check_site(catalog(), "waterwheel", tile)
                .is_ok()
        })
        .unwrap();
    pointer(&mut run, BuilderAction::Plan(0));
    for x in 16..=wheel.0 {
        pointer(
            &mut run,
            BuilderAction::Plot {
                tile: (x as u8, 9),
                build: true,
            },
        );
        assert!(run.home.industry.roads.contains(&(x, 9)));
    }
    build(&mut run, "waterwheel", wheel);
    // A northern shaft avoids the finished archway, preserving its road.
    plan(&mut run, "drive-shaft");
    for x in 12..=wheel.0 {
        pointer(
            &mut run,
            BuilderAction::Plot {
                tile: (x as u8, 7),
                build: true,
            },
        );
        assert!(
            run.home
                .industry
                .power
                .contains(PowerKind::Mechanical, (x, 7))
        );
    }
    let saw = run.home.industry.at((12, 8)).unwrap();
    workshop(&mut run, saw);
    press(
        &mut run,
        Input {
            cast: 1,
            ..Default::default()
        },
    );
    assert!(run.home.industry.buildings[&saw].power_upgrade);
    seconds(&mut run, 40);
    let report = run.home.industry.report(catalog());
    assert!(
        report.energy.powered(PowerKind::Mechanical, saw),
        "{report:?}"
    );
    shot(&run, dest, "10-powered-ledger");
    pointer(&mut run, BuilderAction::Page(0));
    plot(&mut run, (16, 8));
    shot(&run, dest, "11-waterwheel-village");
    plot(&mut run, (16, 7));
    press(
        &mut run,
        Input {
            bomb: true,
            ..Default::default()
        },
    );
    assert!(
        !run.home
            .industry
            .power
            .contains(PowerKind::Mechanical, (16, 7))
    );
    let produced = run
        .home
        .industry
        .produced
        .get("plank")
        .copied()
        .unwrap_or(0);
    seconds(&mut run, 30);
    assert_eq!(
        run.home
            .industry
            .produced
            .get("plank")
            .copied()
            .unwrap_or(0),
        produced
    );
    assert_eq!(
        run.home.industry.report(catalog()).status[&saw],
        Status::PowerDisconnected
    );
    workshop(&mut run, saw);
    shot(&run, dest, "12-cut-shaft-ledger");
    pointer(&mut run, BuilderAction::Page(0));
    pointer(
        &mut run,
        BuilderAction::Plot {
            tile: (16, 7),
            build: true,
        },
    );
    seconds(&mut run, 30);
    assert!(
        run.home
            .industry
            .produced
            .get("plank")
            .copied()
            .unwrap_or(0)
            > produced
    );
    assert!(run.valid_snapshot());
    workshop(&mut run, saw);
    shot(&run, dest, "13-restored-ledger");
    let saved = serde_json::to_string(&run).unwrap();
    let restored: Run = serde_json::from_str(&saved).unwrap();
    assert!(restored.valid_snapshot());
    assert_eq!(run.home.industry, restored.home.industry);
    std::fs::write(dest.join("powered-run.json"), saved).unwrap();
    println!(
        "Paid waterwheel, drive upgrade, cut and repair verified at {}s with {} residents",
        run.home.industry.second,
        run.home.industry.population.citizens.len()
    );
}
