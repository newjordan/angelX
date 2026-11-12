//! The Deep comes up, shown: a first raid, a broken line and its repair, a tower shooting, a wall
//! holding them and the Keep's stones knocked out and laid again, all in the real renderer (flat,
//! CPU) from a knight's own keys. Frames go to `ANGEL_THREAT_SHOTS` (default `/tmp/threat-shots`) from
//! ignored tests. Run with `ANGEL_REALM_PLAN=tabula`.
use super::shots::run_at;
use super::*;
use crate::kingdom::{Command, Dir, Piece, Status, Stock, catalog};
use crate::rules::deep_raid;
use crate::world::works;
use std::path::PathBuf;

fn out() -> PathBuf {
    PathBuf::from(std::env::var("ANGEL_THREAT_SHOTS").unwrap_or_else(|_| "/tmp/threat-shots".into()))
}

fn step(run: &mut Run, input: Input) {
    run.step(&BTreeMap::from([(1, input)]));
}

/// A hillside at home with the Keep staked and the knight in the camp, at the dusk of a day.
fn night_run() -> Run {
    let mut run = run_at(146.5, 97.5);
    for _ in 0..30 * 12 {
        step(&mut run, Input::default());
    }
    for b in run.home.industry.buildings.values_mut() {
        b.construction = 0;
    }
    run.home.industry.stock.remove("coin");
    run
}

fn at_dusk(run: &mut Run, day: u64, before: u64) {
    run.home.industry.second = day * deep_raid::DAY + deep_raid::DUSK + deep_raid::JUDGE_AFTER - before;
}

/// Frames of a reel: one every `every` ticks, the camera held on `look` (a tile) when it is set.
struct Reel {
    dir: PathBuf,
    every: u64,
    n: u32,
    ticks: u64,
    look: Option<(f32, f32)>,
}

impl Reel {
    fn new(scene: &str, every: u64) -> Reel {
        let dir = out().join(scene);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Reel { dir, every, n: 0, ticks: 0, look: None }
    }

    fn shot(&mut self, run: &Run) {
        let im = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        image::save_buffer(self.dir.join(format!("{:04}.png", self.n)), &im.rgba_bytes(), im.w as u32, im.h as u32, image::ColorType::Rgba8).unwrap();
        self.n += 1;
    }

    /// A frame under a name of its own, beside the reel's.
    fn named(&mut self, run: &Run, name: &str) {
        let im = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        image::save_buffer(self.dir.join(format!("{name}.png")), &im.rgba_bytes(), im.w as u32, im.h as u32, image::ColorType::Rgba8).unwrap();
    }

    fn tick(&mut self, run: &mut Run, input: Input) {
        step(run, input);
        if let Some((x, y)) = self.look {
            run.players.get_mut(&1).unwrap().camera = Some((x * TILE_UNITS, y * TILE_UNITS));
        }
        self.ticks += 1;
        if self.ticks % self.every == 0 {
            self.shot(run);
        }
    }

    fn wait(&mut self, run: &mut Run, seconds: u64) {
        for _ in 0..seconds * 30 {
            self.tick(run, Input::default());
        }
    }

    fn press(&mut self, run: &mut Run, input: Input) {
        self.tick(run, input);
        self.tick(run, Input::default());
    }

    /// Walk to the middle of `to`, one axis at a time, holding `extra` besides.
    fn walk(&mut self, run: &mut Run, to: (i16, i16), extra: Input) {
        let (tx, ty) = ((f32::from(to.0) + 0.5) * TILE_UNITS, (f32::from(to.1) + 0.5) * TILE_UNITS);
        for _ in 0..2500 {
            let h = &run.players[&1];
            let (dx, dy) = (tx - h.x, ty - h.y);
            if dx.abs() < 0.25 * TILE_UNITS && dy.abs() < 0.25 * TILE_UNITS {
                return;
            }
            let (mx, my) = if dx.abs() >= dy.abs() { (dx.signum() as i8, 0) } else { (0, dy.signum() as i8) };
            self.tick(run, Input { move_x: mx, move_y: my, ..extra });
        }
        panic!("the knight did not reach {to:?}");
    }

    /// Face a way without leaving the tile: a tick of the key.
    fn face(&mut self, run: &mut Run, dir: Dir) {
        let (mx, my) = dir.delta();
        self.tick(run, Input { move_x: mx as i8, move_y: my as i8, ..Default::default() });
        self.tick(run, Input::default());
    }

    /// The builder's tool, found as a player finds it: Q steps on, the left arrow steps back (the shorter way is
    /// taken); no frames while it is hunted for.
    fn tool(&mut self, run: &mut Run, tool: Tool) {
        let all = tools();
        let want = all.iter().position(|t| *t == tool).expect("a tool of the builder");
        let every = std::mem::replace(&mut self.every, u64::MAX);
        for _ in 0..all.len() {
            let have = run.kingdom.industry[&1].plan;
            if have == want {
                break;
            }
            let (fwd, back) = ((want + all.len() - have) % all.len(), (have + all.len() - want) % all.len());
            if back < fwd {
                self.press(run, Input { aim_x: -1, ..Default::default() });
                for _ in 0..8 {
                    self.tick(run, Input::default());
                }
            } else {
                self.press(run, Input { swing: true, ..Default::default() });
            }
        }
        self.every = every;
        assert_eq!(run.kingdom.industry[&1].plan, want, "the tool {tool:?} is in hand");
    }

    /// The knight fights the nearest foe: aims along the larger axis, holds fire, and swings the blade when it is close.
    fn shoot(&mut self, run: &mut Run, seconds: u64) {
        for _ in 0..seconds * 30 {
            let h = &run.players[&1];
            let near = run
                .enemies
                .iter()
                .filter(|e| e.hp > 0)
                .min_by(|a, b| (a.x - h.x).hypot(a.y - h.y).total_cmp(&(b.x - h.x).hypot(b.y - h.y)))
                .map(|e| (e.x - h.x, e.y - h.y));
            let Some((dx, dy)) = near else { return };
            let (ax, ay) = if dx.abs() >= dy.abs() { (dx.signum() as i8, 0) } else { (0, dy.signum() as i8) };
            self.tick(run, Input { aim_x: ax, aim_y: ay, fire: true, swing: dx.hypot(dy) < 5.0, ..Default::default() });
        }
    }
}

fn raise(run: &mut Run, plan: &str, tile: (i16, i16)) -> u32 {
    super::shots::raise(run, plan, tile)
}

fn raiders(run: &Run) -> usize {
    run.raid_status().map_or(0, |(_, alive, _)| alive)
}

fn report(run: &Run, what: &str) {
    let k = &run.home.industry;
    println!(
        "{what}: night {} raid {:?} fires {} worn {} cuts {} ghosts {} tally {:?} lost stones {} arrows {:?}",
        run.deep_clock().0,
        run.raid_status(),
        k.havoc.fires.len(),
        k.havoc.worn.len(),
        k.havoc.cuts.len(),
        k.plans.ghosts.len(),
        k.havoc.tally,
        run.sites.keep().map_or(0, |s| s.lost.len()),
        k.buildings.values().filter(|b| catalog().plans[&b.plan].watch > 0).map(|b| b.input.get("arrow").copied().unwrap_or(0)).collect::<Vec<_>>()
    );
}

// --------------------------------------------------------------- a first raid ----

/// The first raid: two imps climb the Great Shaft at nightfall, march for the smoke of the kiln and set
/// it alight. The knight shoots them, then beats the fire out with the builder's MEND tool: three presses.
#[test]
#[ignore = "writes the first-raid proof frames; needs ANGEL_REALM_PLAN=tabula"]
fn write_the_first_raid() {
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let c = catalog();
    let mut run = night_run();
    {
        // The knight waits east of the works, out of the imps' sight until they have done their work.
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (152.5 * TILE_UNITS, 97.5 * TILE_UNITS);
    }
    let kiln = raise(&mut run, "lime-kiln", (141, 98));
    {
        let k = &mut run.home.industry;
        k.apply(c, Command::Connect(kiln)).unwrap();
        k.buildings.get_mut(&kiln).unwrap().recipe = Some("burn-lime-coal".into());
        k.buildings.get_mut(&kiln).unwrap().input = Stock::from([("limestone".into(), 32), ("coal".into(), 8)]);
    }
    run.kingdom.stir.level = 80;
    for _ in 0..30 * 4 {
        step(&mut run, Input::default());
    }
    assert_eq!(run.home.industry.report(c).status[&kiln], Status::Working, "the kiln smokes at its work");
    let mut reel = Reel::new("first-raid", 15);
    reel.look = Some((141.0, 95.0));
    // By day: nothing. Then dusk.
    reel.wait(&mut run, 1);
    at_dusk(&mut run, 3, 3);
    reel.wait(&mut run, 4);
    assert!(run.kingdom.stir.watch.on.is_some(), "the Deep answers");
    report(&run, "dusk");
    // They climb, march, reach the kiln.
    let mut lit = false;
    for _ in 0..60 {
        reel.wait(&mut run, 1);
        if !run.home.industry.havoc.fires.is_empty() {
            lit = true;
            break;
        }
    }
    report(&run, "alight");
    assert!(lit, "the imps set the kiln alight");
    reel.wait(&mut run, 2);
    // The knight comes to the kiln (at the camp's road, a step from it) and shoots.
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (141.5 * TILE_UNITS, 96.5 * TILE_UNITS);
    }
    reel.wait(&mut run, 1);
    for _ in 0..8 {
        if raiders(&run) == 0 {
            break;
        }
        reel.shoot(&mut run, 4);
    }
    report(&run, "shot");
    // The builder comes out (key 4), the MEND tool in hand, E on the burning kiln: three presses.
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    reel.tool(&mut run, Tool::Mend);
    reel.walk(&mut run, (141, 97), Input::default());
    reel.face(&mut run, Dir::S);
    let tile = run.kingdom.industry[&1].tile;
    println!("the knight's cursor stands at {tile:?}; the kiln at {:?}", run.home.industry.buildings[&kiln].tile);
    for _ in 0..4 {
        if run.home.industry.havoc.fires.is_empty() {
            break;
        }
        reel.press(&mut run, Input { interact: true, ..Default::default() });
        reel.wait(&mut run, 1);
    }
    report(&run, "mended");
    reel.wait(&mut run, 3);
    println!("frames {}", reel.n);
}

// ----------------------------------------------------------- a line cut, mended ----

/// A depot's limestone runs along a rollerway to a kiln. Sappers cut the line in two places; the hired hands
/// mend one, and the knight mends the other with the MEND tool, at once.
#[test]
#[ignore = "writes the broken-line proof frames; needs ANGEL_REALM_PLAN=tabula"]
fn write_a_cut_line_and_its_mending() {
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let c = catalog();
    let mut run = night_run();
    let depot = raise(&mut run, "depot", (144, 102));
    let kiln = raise(&mut run, "lime-kiln", (160, 102));
    let brazier = raise(&mut run, "brazier", (152, 99));
    {
        let k = &mut run.home.industry;
        k.stock.insert("timber".into(), 800);
        k.buildings.get_mut(&depot).unwrap().output.insert("limestone".into(), 200);
        k.stock.insert("limestone".into(), 0);
        k.apply(c, Command::Plan { plan: "swape".into(), tile: (147, 103), priority: 1, by: 1 }).ok();
        for (tile, dir) in [((147, 103), Dir::E), ((157, 103), Dir::E)] {
            k.apply(c, Command::Build { plan: "swape".into(), tile }).ok();
            if let Some(id) = k.at(tile) {
                k.buildings.get_mut(&id).unwrap().construction = 0;
                k.apply(c, Command::Face { tile, dir }).unwrap();
            }
        }
        for x in 148..157 {
            k.apply(c, Command::Roller { piece: Piece::Roller, tile: (x, 103), dir: Dir::E }).unwrap();
        }
    }
    run.kingdom.stir.level = 80;
    run.kingdom.stir.watch.raids = 2;
    let mut reel = Reel::new("cut-line", 15);
    reel.look = Some((152.0, 101.0));
    reel.wait(&mut run, 8);
    report(&run, "running");
    at_dusk(&mut run, 3, 3);
    // The knight waits at the line; the sappers come.
    reel.walk(&mut run, (152, 105), Input::default());
    let mut cut = false;
    for _ in 0..90 {
        reel.wait(&mut run, 1);
        if run.home.industry.havoc.tally.cut >= 2 {
            cut = true;
            break;
        }
    }
    report(&run, "cut");
    assert!(cut, "the line is cut");
    // The knight shoots the sappers.
    reel.shoot(&mut run, 12);
    report(&run, "sappers down");
    // The builder out, MEND in hand: stand beside a cut tile, face it, E.
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    reel.tool(&mut run, Tool::Mend);
    let cuts: Vec<(i16, i16)> = run.home.industry.havoc.cuts.iter().copied().collect();
    println!("cuts {cuts:?}");
    if let Some(&t) = cuts.first() {
        reel.walk(&mut run, (t.0, t.1 + 1), Input::default());
        reel.face(&mut run, Dir::N);
        reel.press(&mut run, Input { interact: true, ..Default::default() });
        println!("the knight says: {}", run.kingdom.industry[&1].message);
        report(&run, "knight mended one");
    }
    reel.wait(&mut run, 40);
    report(&run, "hands mended the rest");
    let _ = (kiln, brazier);
    println!("frames {}", reel.n);
}

// --------------------------------------------------- a wall holds, a tower shoots ----

/// A ring of stone wall round a brazier and a kiln, a watchtower with its quiver by the wall. Five imps come at it:
/// they batter the wall (its hold shown), the tower looses its arrows, the wall holds.
#[test]
#[ignore = "writes the wall-and-tower proof frames; needs ANGEL_REALM_PLAN=tabula"]
fn write_a_wall_that_holds_and_a_tower_that_shoots() {
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let c = catalog();
    let mut run = night_run();
    let brazier = raise(&mut run, "brazier", (153, 101));
    run.home.industry.stock.insert("timber".into(), 800);
    let ring: Vec<(i16, i16)> = (150..=156).flat_map(|x| [(x, 97), (x, 104)]).chain((98..=103).flat_map(|y| [(150, y), (156, y)])).collect();
    for t in &ring {
        raise(&mut run, "wall", *t);
    }
    let tower = raise(&mut run, "watchtower", (158, 100));
    let tower2 = raise(&mut run, "watchtower", (147, 100));
    for t in [tower, tower2] {
        run.home.industry.apply(c, Command::Connect(t)).ok();
        run.home.industry.buildings.get_mut(&t).unwrap().input.insert("arrow".into(), 24);
    }
    run.kingdom.stir.level = 100;
    // Five imps: the fourth raid's size at this stir.
    run.kingdom.stir.watch.raids = 4;
    for _ in 0..30 * 4 {
        step(&mut run, Input::default());
    }
    assert_eq!(run.home.industry.report(c).status[&tower], Status::Working, "manned and armed");
    let mut reel = Reel::new("wall-and-tower", 6);
    reel.look = Some((153.0, 99.0));
    reel.wait(&mut run, 1);
    at_dusk(&mut run, 3, 3);
    {
        // The knight stands off to the south-east, in sight of the wall.
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (162.5 * TILE_UNITS, 108.5 * TILE_UNITS);
    }
    let mut peak_worn = 0;
    for _ in 0..90 {
        reel.wait(&mut run, 1);
        peak_worn = peak_worn.max(run.home.industry.havoc.worn.len());
        if run.kingdom.stir.watch.on.is_none() && run.kingdom.stir.watch.raids > 4 {
            break;
        }
    }
    report(&run, "after");
    println!("most walls battered at once {peak_worn}; the brazier stands {}", run.home.industry.buildings.contains_key(&brazier));
    reel.wait(&mut run, 3);
    println!("frames {}", reel.n);
}

// ---------------------------------------------------- the Keep's stones, laid again ----

/// Warboars reach the Keep's wall, knock stones out of the lift in hand (a true one holds longer) and what rested on them
/// comes down; the masons lay them again.
#[test]
#[ignore = "writes the siege proof frames; needs ANGEL_REALM_PLAN=tabula"]
fn write_the_keeps_stones_knocked_out_and_laid_again() {
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let c = catalog();
    let mut run = night_run();
    raise(&mut run, "brazier", (136, 99));
    run.home.industry.stock.insert("timber".into(), 800);
    run.kingdom.stir.level = 100;
    run.kingdom.stir.watch.raids = 3;
    // The last course of the lift in hand is set true, to see one hold.
    let laid = run.sites.keep().unwrap().laid;
    run.sites.keep_mut().unwrap().trues = vec![(laid - 6, laid)];
    let castle = works::castle(works::KEEP).unwrap();
    let last = castle.stones[(laid - 1) as usize].aabb().map(|v| f32::from(v) / 100.0);
    let (ox, oz) = works::origin_px();
    let at = (((ox + (last[0] + last[3]) / 2.0 * works::PX_PER_M) / 16.0), ((oz - (last[1] + last[4]) / 2.0 * works::PX_PER_M) / 16.0));
    println!("the wall's last stone is at tile {at:?}; {laid} stones laid");
    let mut reel = Reel::new("keep-siege", 15);
    reel.look = Some((at.0, at.1 + 3.0));
    {
        // The knight stands off in the camp, out of the boars' way.
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (140.5 * TILE_UNITS, 97.5 * TILE_UNITS);
    }
    reel.wait(&mut run, 1);
    at_dusk(&mut run, 3, 3);
    let mut down = 0;
    for _ in 0..120 {
        reel.wait(&mut run, 1);
        down = run.sites.keep().unwrap().lost.len();
        if down >= 4 {
            break;
        }
    }
    report(&run, "stones out");
    println!("{down} stones out: {:?}", run.sites.keep().unwrap().lost);
    reel.wait(&mut run, 4);
    // The wall as it stood and as the raid left it, seen from the same place: the stones that were pulled out,
    // and where they were.
    {
        let lost = run.sites.keep().unwrap().lost.clone();
        let (mut cx, mut cy) = (0.0f32, 0.0f32);
        for &i in &lost {
            let b = castle.stones[i as usize].aabb().map(|v| f32::from(v) / 100.0);
            cx += (ox + (b[0] + b[3]) / 2.0 * works::PX_PER_M) / 16.0;
            cy += (oz - (b[1] + b[4]) / 2.0 * works::PX_PER_M - b[5] * works::PX_PER_M) / 16.0;
        }
        let (cx, cy) = (cx / lost.len().max(1) as f32, cy / lost.len().max(1) as f32);
        println!("the stones were round tile ({cx:.1}, {cy:.1})");
        let h = run.players.get_mut(&1).unwrap();
        h.camera = Some((cx * TILE_UNITS, cy * TILE_UNITS));
        run.sites.keep_mut().unwrap().lost.clear();
        reel.named(&run, "compare-before");
        run.sites.keep_mut().unwrap().lost = lost;
        reel.named(&run, "compare-after");
    }
    // The raid is over; the lodge's masons lay them again.
    run.end_raid_for_test();
    let sim = run.keep_live().expect("live");
    {
        let k = &mut run.home.industry;
        for g in ["timber", "plank", "stone", "block"] {
            k.stock.insert(g.into(), 400);
        }
        k.apply(c, Command::Build { plan: "lodge".into(), tile: (150, 90) }).expect("a lodge");
        let lodge = k.at((150, 90)).unwrap();
        k.buildings.get_mut(&lodge).unwrap().construction = 0;
        k.apply(c, Command::Connect(lodge)).expect("a road to the lodge");
    }
    run.hands.yard.timber = 4_000;
    let yard = run.home.industry.sites[&sim].yard;
    let site = &run.home.industry.sites[&sim];
    let needs = crate::kingdom::site::stage_needs(c, &c.castles[&site.plan].stages[usize::from(site.stage)]);
    for (r, n) in needs {
        *run.home.industry.buildings.get_mut(&yard).unwrap().input.entry(r).or_default() += n;
    }
    for r in ["pole", "centring"] {
        *run.home.industry.buildings.get_mut(&yard).unwrap().input.entry(r.into()).or_default() += 40;
    }
    reel.every = 30;
    for _ in 0..20 {
        reel.wait(&mut run, 6);
        if run.sites.keep().unwrap().lost.is_empty() {
            break;
        }
    }
    report(&run, "laid again");
    println!("frames {}", reel.n);
}
