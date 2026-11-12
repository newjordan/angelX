//! Reproducible views of the actual production game, using founding supplies.
use crate::{
    kingdom::{Claim, Command, catalog},
    rules::{Input, Run},
};
use std::{collections::BTreeMap, path::Path};
fn shot(run: &Run, path: &Path) {
    let im = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
    image::save_buffer(
        path,
        &im.rgba_bytes(),
        im.w as u32,
        im.h as u32,
        image::ColorType::Rgba8,
    )
    .unwrap();
}
fn wait(run: &mut Run, cost: &crate::kingdom::Stock) {
    for _ in 0..200 {
        if cost.iter().all(|(r, n)| run.home.industry.amount(r) >= *n) {
            return;
        }
        run.home.industry.work(catalog());
    }
    panic!("fixture production stalled: {:?}", run.home.industry.stock);
}
fn build(run: &mut Run, plan: &str, x: i16, y: i16) -> u32 {
    wait(run, &catalog().plans[plan].cost);
    run.home
        .industry
        .apply(
            catalog(),
            Command::Build {
                plan: plan.into(),
                tile: (x, y),
            },
        )
        .unwrap();
    run.home.industry.at((x, y)).unwrap()
}
fn research(run: &mut Run, id: &str) {
    wait(run, &catalog().technologies[id].cost);
    run.home
        .industry
        .apply(catalog(), Command::Research(id.into()))
        .unwrap();
}
fn work(run: &mut Run, n: usize) {
    for _ in 0..n {
        run.home.industry.work(catalog());
    }
}
fn claim(run: &mut Run, mine: u32, id: &str, resource: &str) {
    run.home.industry.discover(
        catalog(),
        id.into(),
        Claim {
            resource: resource.into(),
            name: if resource == "crystal" {
                "Moon crystal vein"
            } else {
                "Ironroot vein"
            }
            .into(),
            source: "Visual review encounter fixture".into(),
            secured: false,
            rate: 2,
            yields: Default::default(), surface: false },
    );
    run.home.industry.secure(id);
    run.home
        .industry
        .apply(
            catalog(),
            Command::Link {
                building: mine,
                claim: id.into(),
            },
        )
        .unwrap();
}
#[test]
#[ignore = "writes real factory, logistics, research, armory and walking frames"]
fn write_frontier_review() {
    let dest =
        std::env::var("ANGEL_FRONTIER_SHOTS").unwrap_or_else(|_| "/tmp/frontier-review".into());
    let dest = Path::new(&dest);
    std::fs::create_dir_all(dest).unwrap();
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    run.step(&BTreeMap::from([(
        1,
        Input {
            cast: 4,
            ..Default::default()
        },
    )]));
    shot(&run, &dest.join("01-foundation.png"));
    build(&mut run, "forester", 8, 8);
    build(&mut run, "quarry", 9, 8);
    build(&mut run, "cottage", 10, 8);
    build(&mut run, "sawmill", 12, 8);
    build(&mut run, "kiln", 13, 8);
    research(&mut run, "masonry");
    build(&mut run, "mason", 14, 8);
    let mine = build(&mut run, "mine", 15, 8);
    claim(&mut run, mine, "review-iron", "iron-ore");
    research(&mut run, "logistics");
    build(&mut run, "caravanserai", 8, 10);
    build(&mut run, "warehouse", 9, 10);
    research(&mut run, "metallurgy");
    build(&mut run, "cottage", 10, 10);
    build(&mut run, "smelter", 11, 10);
    let forge = build(&mut run, "forge", 12, 10);
    wait(&mut run, &BTreeMap::from([("iron-oathblade".into(), 1)]));
    run.kingdom_gear_action(1, "iron-oathblade").unwrap();
    run.home
        .industry
        .apply(catalog(), Command::Pause(forge))
        .unwrap();
    let moon = build(&mut run, "mine", 13, 10);
    claim(&mut run, moon, "review-moon", "crystal");
    research(&mut run, "arcana");
    build(&mut run, "conduit", 14, 10);
    let tower = build(&mut run, "enchanter", 15, 10);
    wait(&mut run, &BTreeMap::from([("hearth-wardstone".into(), 1)]));
    run.home
        .industry
        .apply(catalog(), Command::Pause(tower))
        .unwrap();
    research(&mut run, "sovereignty");
    run.home
        .industry
        .apply(catalog(), Command::Road((16, 9)))
        .unwrap();
    build(&mut run, "keep", 16, 8);
    work(&mut run, 5);
    run.kingdom.industry.get_mut(&1).unwrap().tile = (12, 8);
    run.kingdom.industry.get_mut(&1).unwrap().message =
        "Delivered iron has become an equipped Oathblade".into();
    run.tick = run.home.industry.second * 30 + 15;
    shot(&run, &dest.join("02-production.png"));
    for (page, name) in [
        (1, "03-charters"),
        (2, "04-armory"),
        (3, "05-claims"),
        (4, "06-ore-guide"),
        (5, "07-library"),
    ] {
        run.kingdom.industry.get_mut(&1).unwrap().page = page;
        shot(&run, &dest.join(format!("{name}.png")));
    }
    run.enter_frontier();
    run.players.get_mut(&1).unwrap().y = 19.;
    shot(&run, &dest.join("08-walk.png"));
    std::fs::write(
        dest.join("kingdom.json"),
        serde_json::to_vec_pretty(&run.home.industry).unwrap(),
    )
    .unwrap();
    println!(
        "Eight actual game views written to {}, at {} minutes of simulated play",
        dest.display(),
        run.home.industry.second / 60
    );
}
