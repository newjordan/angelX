//! The Works' scenes for the dev host (`--scene works|works-won|works-small|works-big|works-castle`):
//! a fresh blank-slate realm as a new knight meets it, and the same realm with a seam won, a small
//! works at work, a large one (seventy buildings) and the Keep most of the way up, for a browser
//! seat to join. Development only, like the host.
use angel_delve::kingdom::{Claim, Command, catalog};
use angel_delve::rules::layout::TILE_UNITS;
use angel_delve::rules::{Input, Run};
use angel_delve::world::plan::tabula as plan;
use std::collections::BTreeMap;

pub fn scene(name: &str) -> Option<&'static str> {
    ["works", "works-won", "works-small", "works-big", "works-castle", "works-face", "works-block", "works-old", "works-gate"].into_iter().find(|n| *n == name)
}

/// A fresh run of the realm as the live host starts one, and its knight on the spawn in the bailey,
/// the hillside not yet founded (the first key 4 does that), no stock but what the camp begins with.
fn fresh() -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(Some(plan::KEEP_STAND));
    run
}

fn stand(run: &mut Run, x: f32, y: f32) {
    let h = run.players.get_mut(&1).expect("a knight");
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
    h.camera = Some((h.x, h.y - 1.0));
    h.invulnerable = 1_000_000;
}

fn settle(run: &mut Run, steps: u32) {
    for _ in 0..steps {
        run.step(&BTreeMap::from([(1, Input::default())]));
    }
}

fn win_seam(run: &mut Run) {
    let claim = Claim { resource: "limestone".into(), name: "Sound limestone face".into(), source: "The Mines / floor 3".into(), secured: false, rate: 4, yields: [("limestone".to_string(), 4), ("iron-ore".to_string(), 1)].into(), surface: false };
    run.home.industry.discover(catalog(), "mines-3".into(), claim);
    run.home.industry.secure("mines-3");
}

fn raise(run: &mut Run, plan: &str, tile: (i16, i16)) -> Result<u32, String> {
    let k = &mut run.home.industry;
    k.apply(catalog(), Command::Build { plan: plan.into(), tile }).map_err(|e| format!("{plan} at {tile:?}: {e}"))?;
    let id = k.at(tile).ok_or("it did not stand")?;
    k.buildings.get_mut(&id).ok_or("it did not stand")?.construction = 0;
    Ok(id)
}

/// A hillside founded and rich (every good in stock, every charter read), the knight at `at`.
fn rich(at: (f32, f32)) -> Run {
    let mut run = fresh();
    run.home.industry.found_hillside(catalog(), 7);
    let k = &mut run.home.industry;
    for r in catalog().resources.keys() {
        k.stock.insert(r.clone(), 600);
    }
    k.stock.insert("fodder".into(), 100_000);
    k.researched = catalog().all_techs().map(|(k, _)| k.clone()).collect();
    stand(&mut run, at.0, at.1);
    settle(&mut run, 4);
    run
}

fn small() -> Run {
    let mut run = rich((150.5, 100.5));
    let c = catalog();
    win_seam(&mut run);
    let made = (|| -> Result<(), String> {
        let yard = raise(&mut run, "carters-yard", (143, 98))?;
        let quarry = raise(&mut run, "claim-quarry", (160, 101))?;
        let kiln = raise(&mut run, "lime-kiln", (153, 100))?;
        let banker = raise(&mut run, "banker", (153, 104))?;
        let wood = raise(&mut run, "woodsman-camp", (143, 103))?;
        let saw = raise(&mut run, "sawpit", (148, 103))?;
        raise(&mut run, "well", (150, 97))?;
        raise(&mut run, "cottage", (137, 99))?;
        raise(&mut run, "mortar-mill", (157, 97))?;
        let k = &mut run.home.industry;
        k.apply(c, Command::Link { building: quarry, claim: "mines-3".into() })?;
        k.apply(c, Command::Recipe { building: banker, recipe: "dress-limestone".into() })?;
        for id in [yard, quarry, kiln, banker, wood, saw] {
            k.apply(c, Command::Connect(id))?;
        }
        k.apply(c, Command::Carts { yard, carts: 4 })?;
        Ok(())
    })();
    if let Err(why) = made {
        eprintln!("works-small: {why}");
    }
    run
}

fn big() -> Run {
    let mut run = rich((160.5, 108.5));
    let c = catalog();
    win_seam(&mut run);
    let Ok(yard) = raise(&mut run, "carters-yard", (143, 98)) else { return run };
    let mut ids = vec![yard];
    let menu: Vec<&String> = c.hillside_menu.iter().filter(|p| !c.plans[*p].yard && c.plans[*p].connector.is_none() && c.plans[*p].stalls == 0 && c.plans[*p].replaces.is_none()).collect();
    run.home.industry.population = angel_delve::kingdom::population::Population::founders_of(60);
    let mut n = 0usize;
    'fill: for row in 0..10i16 {
        for col in 0..12i16 {
            let tile = (148 + col * 7, 98 + row * 6);
            for k in 0..menu.len() {
                if let Ok(id) = raise(&mut run, menu[(n + k) % menu.len()], tile) {
                    ids.push(id);
                    n += 1;
                    break;
                }
            }
            if n >= 70 {
                break 'fill;
            }
        }
    }
    let k = &mut run.home.industry;
    for id in &ids {
        let _ = k.apply(c, Command::Connect(*id));
    }
    let _ = k.apply(c, Command::Carts { yard, carts: 12 });
    eprintln!("works-big: {} buildings, {} road tiles", run.home.industry.buildings.len(), run.home.industry.roads.len());
    run
}

fn castle() -> Run {
    let mut run = small();
    run.lay_to("great-tower", 9000);
    stand(&mut run, plan::KEEP_STAND.0 as f32 + 0.5, plan::KEEP_STAND.1 as f32 + 0.7);
    settle(&mut run, 30);
    run
}

/// A realm saved before the hillside: a village of a few works on the old 72 x 54 meadow, the
/// knight on the surface. The first second of play brings it up onto the hillside round the camp.
fn old() -> Run {
    let mut run = fresh();
    let c = catalog();
    run.home.industry.found(c, 7);
    let mut built = 0;
    // Each work on the first plot of the old meadow that takes it.
    for plan in ["forester", "quarry", "cottage", "cottage", "clay-pit", "sawmill"] {
        let spot = (4..40i16).flat_map(|y| (6..60i16).map(move |x| (x, y))).find(|&tile| {
            run.home.industry.apply(c, Command::Build { plan: plan.into(), tile }).is_ok()
        });
        match spot {
            Some(_) => built += 1,
            None => eprintln!("works-old: nowhere on the meadow for a {plan}"),
        }
    }
    eprintln!("works-old: {built} buildings on the old meadow");
    run
}

pub fn run(name: &str) -> Run {
    match name {
        "works-old" => old(),
        // The knight at the Delve's gate, a step from E (the walk to it is the walker's own).
        "works-gate" => {
            let mut run = fresh();
            let (gx, gy) = plan::delve_stand();
            stand(&mut run, gx as f32 + 0.5, gy as f32 + 0.7);
            settle(&mut run, 30);
            run
        }
        // A knight at a won face in the Delve, to cut a block (Q).
        "works-face" => angel_delve::rules::quarrying::won_face_run(),
        // A knight in the bailey with a block on their back, the stockyard to find.
        "works-block" => {
            let mut run = fresh();
            run.home.industry.found_hillside(catalog(), 7);
            run.players.get_mut(&1).expect("a knight").block = Some("limestone".into());
            run
        }
        "works-won" => {
            let mut run = fresh();
            run.home.industry.found_hillside(catalog(), 7);
            win_seam(&mut run);
            run
        }
        "works-small" => small(),
        "works-big" => big(),
        "works-castle" => castle(),
        _ => fresh(),
    }
}
