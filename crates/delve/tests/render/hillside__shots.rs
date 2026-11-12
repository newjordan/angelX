//! The hillside works in the real renderer (flat, CPU): a small works built by the knights'
//! own keys and commands, frames written by ignored tests to `ANGEL_HILLSIDE_SHOTS`
//! (default `/tmp/hillside-shots`). Run with `ANGEL_REALM_PLAN=tabula`.
use super::*;
use crate::kingdom::{Claim, Command, LineKind, catalog};
use std::path::{Path, PathBuf};

pub(crate) fn out() -> PathBuf {
    PathBuf::from(std::env::var("ANGEL_HILLSIDE_SHOTS").unwrap_or_else(|_| "/tmp/hillside-shots".into()))
}

pub(crate) fn frame(run: &Run, dir: &Path, name: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let im = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
    image::save_buffer(dir.join(format!("{name}.png")), &im.rgba_bytes(), im.w as u32, im.h as u32, image::ColorType::Rgba8).unwrap();
}

fn keys(input: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, input)])
}

/// A hillside run, rich, the knight standing east of the camp with the builder not yet out.
pub(crate) fn run_at(x: f32, y: f32) -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(catalog(), 7);
    let k = &mut run.home.industry;
    for r in catalog().resources.keys() {
        k.stock.insert(r.clone(), 600);
    }
    k.stock.insert("fodder".into(), 100_000);
    k.researched = catalog().all_techs().map(|(k, _)| k.clone()).collect();
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
    h.invulnerable = 1_000_000;
    h.stone = false;
    for _ in 0..4 {
        run.step(&keys(Input::default()));
    }
    run
}

/// Build `plan` at `tile`, finished.
pub(crate) fn raise(run: &mut Run, plan: &str, tile: (i16, i16)) -> u32 {
    let k = &mut run.home.industry;
    k.apply(catalog(), Command::Build { plan: plan.into(), tile }).unwrap_or_else(|e| panic!("{plan} at {tile:?}: {e}"));
    let id = k.at(tile).unwrap();
    k.buildings.get_mut(&id).unwrap().construction = 0;
    id
}

/// A small works round the camp: the carters' yard and its carts, a kiln, a banker, a quarry
/// on a won seam, a woodsman's camp and a sawpit, all joined to the camp's road.
pub(crate) fn small_works() -> Run {
    let mut run = run_at(150.5, 100.5);
    let c = catalog();
    let k = &mut run.home.industry;
    let claim = Claim { resource: "limestone".into(), name: "Sound limestone face".into(), source: "The Mines / floor 3".into(), secured: false, rate: 4, yields: [("limestone".to_string(), 4), ("iron-ore".to_string(), 1)].into(), surface: false };
    assert!(k.discover(c, "mines-3".into(), claim));
    assert!(k.secure("mines-3"));
    let yard = raise(&mut run, "carters-yard", (143, 98));
    let quarry = raise(&mut run, "claim-quarry", (160, 101));
    let kiln = raise(&mut run, "lime-kiln", (153, 100));
    let banker = raise(&mut run, "banker", (153, 104));
    let wood = raise(&mut run, "woodsman-camp", (143, 103));
    let saw = raise(&mut run, "sawpit", (148, 103));
    raise(&mut run, "well", (150, 97));
    raise(&mut run, "cottage", (137, 99));
    let k = &mut run.home.industry;
    k.apply(c, Command::Link { building: quarry, claim: "mines-3".into() }).unwrap();
    k.apply(c, Command::Recipe { building: banker, recipe: "dress-limestone".into() }).unwrap();
    for id in [yard, quarry, kiln, banker, wood, saw] {
        k.apply(c, Command::Connect(id)).unwrap();
    }
    k.apply(c, Command::Carts { yard, carts: 4 }).unwrap();
    run
}

#[test]
#[ignore = "writes the hillside proof frames; needs ANGEL_REALM_PLAN=tabula"]
fn write_a_small_works() {
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let dir = out().join("small-works");
    let mut run = small_works();
    for n in 0..8 {
        for _ in 0..30 * 10 {
            run.step(&keys(Input::default()));
        }
        frame(&run, &dir, &format!("{n:02}"));
    }
}
