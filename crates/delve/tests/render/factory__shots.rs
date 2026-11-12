//! The factory lane's proof: rollerways, swapes and rails built by a knight's own keys in the real
//! renderer (flat, CPU). The knight walks, turns, lays with E, rotates with P, takes up with X, and the
//! frames (the seat's whole picture) are written to `ANGEL_FACTORY_SHOTS` (default `/tmp/factory-shots`) by
//! ignored tests. Run with `ANGEL_REALM_PLAN=tabula`; `factory/make-gifs.sh` joins the frames.
use super::shots::{raise, run_at};
use super::*;
use crate::kingdom::{Claim, Command, Dir, Piece, catalog};
use crate::render::overworld::ink::{Img, hash};
use crate::render::overworld::works_kit::{Load, State, rollers as kroll};
use std::path::{Path, PathBuf};

fn out() -> PathBuf {
    PathBuf::from(std::env::var("ANGEL_FACTORY_SHOTS").unwrap_or_else(|_| "/tmp/factory-shots".into()))
}

fn save(dir: &Path, name: &str, im: &Img, scale: u32) {
    std::fs::create_dir_all(dir).unwrap();
    image::RgbaImage::from_raw(im.w as u32 * scale, im.h as u32 * scale, im.rgba_scaled(scale)).unwrap().save(dir.join(format!("{name}.png"))).unwrap();
}

fn grass(w: i32, h: i32) -> Img {
    let mut im = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let n = hash(x, y, 5) % 23;
            im.put(x, y, if n == 0 { 'l' } else if n < 3 { 'F' } else { 'E' });
        }
    }
    im
}

/// The kit's rollerway pieces, every way, working and still, and the swape in each state.
#[test]
#[ignore = "writes the kit's rollerway sheet to ANGEL_FACTORY_SHOTS"]
fn write_the_rollerway_pieces() {
    let mut sheet = grass(16 * 25 + 8, 4 * 34 + 8);
    for (row, dir) in (0u8..4).enumerate() {
        let y = 6 + row as i32 * 34;
        let mut x = 4;
        for piece in Piece::ALL {
            for moving in [false, true] {
                let pic = kroll::piece_flat(piece, dir, moving, true, 1);
                sheet.stamp(&pic, x, y + 24 - pic.h + 6);
                x += 18;
            }
        }
        for state in [State::Idle, State::Working, State::Starved(Load::Planks), State::BackedUp(Load::Planks), State::Building(0), State::Building(2)] {
            let pic = kroll::swape_flat(dir, state, 2);
            sheet.stamp(&pic, x, y + 24 - pic.h + 6);
            x += 18;
        }
        for load in [Load::Stone, Load::Ashlar, Load::Planks, Load::Lime, Load::Iron, Load::Bread, Load::Ale] {
            sheet.stamp(&kroll::good_flat(load), x + 2, y + 12);
            x += 8;
        }
        for held in [false, true] {
            let pic = kroll::bay_flat(dir, held, 0);
            sheet.stamp(&pic, x + 4, y + 24 - pic.h + 6);
            x += 18;
        }
    }
    save(&out(), "pieces", &sheet, 5);
}

// ------------------------------------------------------------------------ the knight's hands ----

fn step(run: &mut Run, input: Input) {
    run.step(&BTreeMap::from([(1, input)]));
}

/// Frames of a reel: one every `every` ticks.
struct Reel {
    dir: PathBuf,
    every: u64,
    n: u32,
    ticks: u64,
}

impl Reel {
    fn new(scene: &str, every: u64) -> Reel {
        let dir = out().join(scene);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Reel { dir, every, n: 0, ticks: 0 }
    }

    fn tick(&mut self, run: &mut Run, input: Input) {
        step(run, input);
        self.ticks += 1;
        if self.ticks % self.every == 0 {
            let im = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
            image::save_buffer(self.dir.join(format!("{:04}.png", self.n)), &im.rgba_bytes(), im.w as u32, im.h as u32, image::ColorType::Rgba8).unwrap();
            self.n += 1;
        }
    }

    fn wait(&mut self, run: &mut Run, seconds: u64) {
        for _ in 0..seconds * 30 {
            self.tick(run, Input::default());
        }
    }

    /// A key pressed and let go.
    fn press(&mut self, run: &mut Run, input: Input) {
        self.tick(run, input);
        self.tick(run, Input::default());
    }

    /// Walk to the middle of `to`, holding `extra` besides the walking, one axis at a time.
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
        panic!("the knight did not reach {to:?}, stands at {:?}", tile_of(run));
    }

    /// Face a way without leaving the tile: a tick of the key.
    fn face(&mut self, run: &mut Run, dir: Dir) {
        let (mx, my) = dir.delta();
        self.tick(run, Input { move_x: mx as i8, move_y: my as i8, ..Default::default() });
        self.tick(run, Input::default());
        assert_eq!(run.kingdom.industry[&1].face, dir.index(), "the knight faces {dir:?}");
    }

    /// The builder's tool, found by pressing Q (the tool key) as a player does.
    fn tool(&mut self, run: &mut Run, tool: Tool) {
        let all = tools();
        let want = all.iter().position(|t| *t == tool).expect("a tool of the builder");
        for _ in 0..all.len() {
            if run.kingdom.industry[&1].plan == want {
                return;
            }
            self.press(run, Input { swing: true, ..Default::default() });
        }
        panic!("the tool {tool:?} was not found");
    }

    /// Turn what is to be laid until it faces `dir` (P, the turn key), the knight facing as he does.
    fn turn_to(&mut self, run: &mut Run, dir: Dir) {
        for _ in 0..4 {
            let b = &run.kingdom.industry[&1];
            if Dir::from_index(b.face.wrapping_add(b.turn)) == dir {
                return;
            }
            self.press(run, Input { cycle: true, ..Default::default() });
        }
    }

    /// Lay what the tool in hand is on the tile ahead.
    fn lay(&mut self, run: &mut Run) {
        self.press(run, Input { interact: true, ..Default::default() });
    }
}

fn tile_of(run: &Run) -> (i16, i16) {
    let h = &run.players[&1];
    ((h.x / TILE_UNITS) as i16, (h.y / TILE_UNITS) as i16)
}

fn won_seam(run: &mut Run) {
    let c = catalog();
    let claim = Claim { resource: "limestone".into(), name: "Sound limestone face".into(), source: "The Mines / floor 3".into(), secured: false, rate: 4, yields: [("limestone".to_string(), 4), ("iron-ore".to_string(), 1)].into(), surface: false };
    run.home.industry.discover(c, "mines-3".into(), claim);
    run.home.industry.secure("mines-3");
}

/// A swape facing `dir`, laid by the knight standing at `from` and facing `face` (it goes on the tile ahead).
fn lay_swape(reel: &mut Reel, run: &mut Run, from: (i16, i16), face: Dir, dir: Dir) {
    reel.tool(run, Tool::Plan("swape"));
    reel.walk(run, from, Input::default());
    reel.face(run, face);
    reel.turn_to(run, dir);
    reel.lay(run);
    let ahead = face.step(from);
    let id = run.home.industry.at(ahead).unwrap_or_else(|| panic!("no swape at {ahead:?}: {}", run.kingdom.industry[&1].message));
    assert_eq!(Dir::from_index(run.home.industry.buildings[&id].facing), dir, "the swape faces {dir:?}");
    // The camp's hands raise it at once, for the proof's sake (a swape is eight seconds' work).
    run.home.industry.buildings.get_mut(&id).unwrap().construction = 0;
}

/// One piece of rollerway at `at` facing `dir`, laid by the knight standing at `from` and facing `face`.
fn lay_piece(reel: &mut Reel, run: &mut Run, piece: Piece, from: (i16, i16), face: Dir, dir: Dir) {
    reel.tool(run, Tool::Roller(piece));
    reel.walk(run, from, Input::default());
    reel.face(run, face);
    reel.turn_to(run, dir);
    reel.lay(run);
    let ahead = face.step(from);
    let cell = run.home.industry.roller_at(ahead).unwrap_or_else(|| panic!("no {piece:?} at {ahead:?}: {}", run.kingdom.industry[&1].message));
    assert_eq!((cell.piece, cell.dir), (piece, dir));
}

/// A run of `piece` along a row or column from `from` to `to`, painted by walking it with E held.
fn paint(reel: &mut Reel, run: &mut Run, piece: Piece, from: (i16, i16), to: (i16, i16)) {
    reel.tool(run, Tool::Roller(piece));
    let dir = if to.0 != from.0 { if to.0 > from.0 { Dir::E } else { Dir::W } } else if to.1 > from.1 { Dir::S } else { Dir::N };
    // Stand on the tile before the first, face along the run, hold E and walk to the last tile.
    let before = dir.back().step(from);
    reel.walk(run, before, Input::default());
    reel.face(run, dir);
    reel.turn_to(run, dir);
    reel.walk(run, dir.back().step(to), Input { interact: true, ..Default::default() });
    reel.tick(run, Input { move_x: dir.delta().0 as i8, move_y: dir.delta().1 as i8, interact: true, ..Default::default() });
    reel.tick(run, Input::default());
}

fn depot(run: &mut Run, tile: (i16, i16)) -> u32 {
    raise(run, "depot", tile)
}

fn report_line(run: &Run, what: &str) {
    let k = &run.home.industry;
    let rows = k.roller_report(catalog());
    println!(
        "{what}: {} tiles of rollerway, {} swapes, {} goods on the line, rows {:?}",
        k.rollers.cells.len(),
        k.buildings.values().filter(|b| b.swape).count(),
        k.rollers.cells.values().map(|c| c.held()).sum::<u32>(),
        rows.iter().map(|r| (r.tiles, r.items, r.per_min, r.state.label(), r.jams.iter().map(|j| format!("{:?} {}", j.tile, j.why)).collect::<Vec<_>>())).collect::<Vec<_>>()
    );
}

// ----------------------------------------------------------------------- the streaming line ----

/// A quarry's limestone streams along a rollerway the knight lays by hand past three bankers, each lifting
/// what it wants off the line with its own swape, and the rest runs on to a warehouse at the end.
#[test]
#[ignore = "writes the streaming proof frames; needs ANGEL_REALM_PLAN=tabula"]
fn write_a_rollerway_streaming_into_benches() {
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let c = catalog();
    let mut run = run_at(163.5, 103.5);
    won_seam(&mut run);
    let quarry = raise(&mut run, "claim-quarry", (158, 105));
    let benches: Vec<u32> = [164, 169, 174].iter().map(|&x| raise(&mut run, "banker", (x, 108))).collect();
    let store = depot(&mut run, (179, 106));
    {
        let k = &mut run.home.industry;
        k.apply(c, Command::Link { building: quarry, claim: "mines-3".into() }).unwrap();
        for &b in &benches {
            k.apply(c, Command::Recipe { building: b, recipe: "dress-limestone".into() }).unwrap();
        }

        // The quarry has been working: a heap of rough stone waits at its face.
        k.buildings.get_mut(&quarry).unwrap().output.insert("limestone".into(), 24);
        k.stock.insert("limestone".into(), 0);
        // The first two benches have been at work a while: their hoppers are nearly full.
        k.buildings.get_mut(&benches[0]).unwrap().input.insert("limestone".into(), 28);
        k.buildings.get_mut(&benches[1]).unwrap().input.insert("limestone".into(), 24);
        k.buildings.get_mut(&benches[2]).unwrap().input.insert("limestone".into(), 29);
    }
    let mut reel = Reel::new("streaming", 10);
    reel.wait(&mut run, 2);
    // The builder comes out (key 4).
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    // The loading swape: east of the quarry's face, facing east. The knight stands north of the tile and
    // faces south, so the swape is turned (P) to face east.
    lay_swape(&mut reel, &mut run, (161, 105), Dir::S, Dir::E);
    // The rollerway: its first tile laid from the north, the rest painted by walking east with E held.
    lay_piece(&mut reel, &mut run, Piece::Roller, (162, 105), Dir::S, Dir::E);
    paint(&mut reel, &mut run, Piece::Roller, (163, 106), (177, 106));
    // The unloading swapes: one over each bench, facing south, the knight standing on the line and facing them.
    for x in [165, 170, 175] {
        lay_swape(&mut reel, &mut run, (x, 106), Dir::S, Dir::S);
    }
    // The end of the line: a swape that unloads into the warehouse.
    lay_swape(&mut reel, &mut run, (177, 106), Dir::E, Dir::E);
    report_line(&run, "laid");
    // Put the builder away and walk to a place to see the line from.
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    reel.walk(&mut run, (169, 104), Input::default());
    // (The quarry's own work is the sim's; here the face is kept stocked, so that the stream is the thing shown.)
    let mut keep_stocked = |run: &mut Run| {
        run.home.industry.buildings.get_mut(&quarry).unwrap().output.insert("limestone".into(), 24);
    };
    reel.every = 6;
    for _ in 0..6 {
        keep_stocked(&mut run);
        reel.wait(&mut run, 5);
        report_line(&run, "running");
    }
    reel.every = 30;
    for _ in 0..3 {
        for _ in 0..4 {
            keep_stocked(&mut run);
            reel.wait(&mut run, 5);
        }
        report_line(&run, "later");
    }
    let k = &run.home.industry;
    println!("warehouse holds {:?}; benches hold {:?}", k.buildings[&store].output, benches.iter().map(|b| k.buildings[b].input.clone()).collect::<Vec<_>>());
    println!("frames {}", reel.n);
}

// ---------------------------------------------------------------------- a jam and a splitter ----

fn words_of(run: &Run) -> Vec<String> {
    crate::world::hillside::of(run).map(|v| v.marks.iter().map(|m| m.words.clone()).collect()).unwrap_or_default()
}

/// A store of limestone feeds one lime kiln along a rollerway; the kiln burns less than the line brings, and
/// the line backs up from its end tile by tile with a red mark over the head of the jam. The knight lays a
/// splitter in the line and a short branch to a warehouse: the surplus goes there and the jam clears.
#[test]
#[ignore = "writes the jam proof frames; needs ANGEL_REALM_PLAN=tabula"]
fn write_a_jam_cleared_by_a_splitter() {
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let mut run = run_at(165.5, 103.5);
    let source = depot(&mut run, (158, 105));
    let kiln = raise(&mut run, "lime-kiln", (171, 106));
    let spare = depot(&mut run, (164, 110));
    {
        let k = &mut run.home.industry;
        k.buildings.get_mut(&source).unwrap().output.insert("limestone".into(), 200);
        k.stock.insert("limestone".into(), 0);
        // The kiln's hopper is nearly full (and nobody is at it to burn what it holds).
        k.buildings.get_mut(&kiln).unwrap().input.insert("limestone".into(), 26);
    }
    let mut reel = Reel::new("jam", 10);
    reel.wait(&mut run, 1);
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    lay_swape(&mut reel, &mut run, (161, 105), Dir::S, Dir::E);
    lay_piece(&mut reel, &mut run, Piece::Roller, (162, 105), Dir::S, Dir::E);
    paint(&mut reel, &mut run, Piece::Roller, (163, 106), (169, 106));
    lay_swape(&mut reel, &mut run, (169, 106), Dir::E, Dir::E);
    report_line(&run, "laid");
    // Put the builder away; watch the line fill and back up.
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    reel.walk(&mut run, (165, 104), Input::default());
    reel.every = 20;
    for _ in 0..11 {
        reel.wait(&mut run, 10);
        report_line(&run, "backing up");
    }
    // The overlay (Space, with the builder out) shows the mark's words.
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    reel.press(&mut run, Input { dash: true, ..Default::default() });
    reel.every = 10;
    reel.wait(&mut run, 5);
    println!("what the overlay says: {:?}", words_of(&run));
    reel.press(&mut run, Input { dash: true, ..Default::default() });
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    reel.every = 8;
    // The fix, by hand: a splitter laid over a tile of the line, a branch south to a warehouse.
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    lay_piece(&mut reel, &mut run, Piece::Splitter, (164, 106), Dir::E, Dir::E);
    println!("splitter: {}", run.kingdom.industry[&1].message);
    // The branch: south from the splitter, two tiles of roller and a swape into the warehouse.
    lay_piece(&mut reel, &mut run, Piece::Roller, (166, 107), Dir::W, Dir::S);
    lay_piece(&mut reel, &mut run, Piece::Roller, (166, 108), Dir::W, Dir::S);
    lay_swape(&mut reel, &mut run, (164, 109), Dir::E, Dir::S);
    report_line(&run, "fixed");
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    reel.walk(&mut run, (165, 104), Input::default());
    reel.every = 15;
    for _ in 0..8 {
        reel.wait(&mut run, 10);
        report_line(&run, "cleared");
    }
    let k = &run.home.industry;
    println!("kiln input {:?}; spare store {:?}", k.buildings[&kiln].input, k.buildings[&spare].output);
    println!("frames {}", reel.n);
}

// -------------------------------------------------------------------------------- a rail junction ----

/// Two stores send wagons along their own rails to a switch and on to a third: the wagons meet at the
/// switch and take turns. The rails, the bays and the routes are all laid by the knight's keys.
#[test]
#[ignore = "writes the rail proof frames; needs ANGEL_REALM_PLAN=tabula"]
fn write_a_rail_junction_with_two_wagons_taking_turns() {
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let mut run = run_at(166.5, 108.5);
    let north = depot(&mut run, (158, 106));
    let south = depot(&mut run, (158, 114));
    let out = depot(&mut run, (176, 109));
    {
        let k = &mut run.home.industry;
        k.buildings.get_mut(&north).unwrap().output.insert("plank".into(), 80);
        k.buildings.get_mut(&south).unwrap().output.insert("beam".into(), 80);
        k.buildings.get_mut(&out).unwrap().output.clear();
    }
    let mut reel = Reel::new("rails", 6);
    reel.wait(&mut run, 1);
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    // Rail, a tile at a time, painted by walking with E held: the north branch east and then south to the
    // switch at (168, 110); the south branch east and then north; the line out to the east.
    reel.tool(&mut run, Tool::Rail);
    reel.walk(&mut run, (161, 105), Input::default());
    reel.face(&mut run, Dir::S);
    reel.lay(&mut run);
    paint_rail(&mut reel, &mut run, (162, 106), (168, 106));
    paint_rail(&mut reel, &mut run, (168, 107), (168, 109));
    reel.walk(&mut run, (161, 113), Input::default());
    reel.face(&mut run, Dir::S);
    reel.lay(&mut run);
    paint_rail(&mut reel, &mut run, (162, 114), (168, 114));
    paint_rail(&mut reel, &mut run, (168, 113), (168, 111));
    reel.tool(&mut run, Tool::Rail);
    paint_rail(&mut reel, &mut run, (168, 110), (175, 110));
    println!("rails laid: {} tiles; switch at (168, 110): {}", run.home.industry.rails.cells.len(), run.home.industry.rails.is_switch((168, 110)));
    // Bays: the north store's, the south's and the out store's.
    reel.tool(&mut run, Tool::Bay);
    for (from, face) in [((161, 105), Dir::S), ((161, 113), Dir::S), ((174, 110), Dir::E)] {
        reel.walk(&mut run, from, Input::default());
        reel.face(&mut run, face);
        reel.lay(&mut run);
        println!("bay: {}", run.kingdom.industry[&1].message);
    }
    // Routes: E at the bay the wagons load at, E at the bay they unload at; `.` sends another wagon.
    reel.tool(&mut run, Tool::Route);
    for (from, to) in [((161, 105), (174, 110)), ((161, 113), (174, 110))] {
        reel.walk(&mut run, from, Input::default());
        reel.face(&mut run, Dir::S);
        reel.lay(&mut run);
        reel.walk(&mut run, (174, 110), Input::default());
        reel.face(&mut run, Dir::E);
        reel.lay(&mut run);
        println!("route: {}", run.kingdom.industry[&1].message);
        let _ = to;
        reel.walk(&mut run, from, Input::default());
        reel.face(&mut run, Dir::S);
        reel.press(&mut run, Input { skill: 3, ..Default::default() });
        println!("a second wagon: {}", run.kingdom.industry[&1].message);
    }
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    reel.walk(&mut run, (170, 107), Input::default());
    reel.every = 8;
    for _ in 0..7 {
        reel.wait(&mut run, 10);
        let k = &run.home.industry;
        println!("running: wagons {:?} out {:?} jams {}", k.rails.wagons.values().map(|w| (w.tile, w.amount)).collect::<Vec<_>>(), k.buildings[&out].output, k.rail_jams(catalog()).len());
    }
    // The overlay (Space, with the builder out) names the bays.
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    reel.press(&mut run, Input { dash: true, ..Default::default() });
    reel.wait(&mut run, 8);
    println!("frames {}", reel.n);
}

/// Rail painted along a row or column from `from` to `to` by walking it with E held.
fn paint_rail(reel: &mut Reel, run: &mut Run, from: (i16, i16), to: (i16, i16)) {
    reel.tool(run, Tool::Rail);
    let dir = if to.0 != from.0 { if to.0 > from.0 { Dir::E } else { Dir::W } } else if to.1 > from.1 { Dir::S } else { Dir::N };
    let before = dir.back().step(from);
    reel.walk(run, before, Input::default());
    reel.face(run, dir);
    reel.walk(run, dir.back().step(to), Input { interact: true, ..Default::default() });
    reel.tick(run, Input { move_x: dir.delta().0 as i8, move_y: dir.delta().1 as i8, interact: true, ..Default::default() });
    reel.tick(run, Input::default());
}

// ------------------------------------------------------------------------------------ the hand-chain ----

/// The cheap first line: a store, a hand-chain of labourers, a bench. Nothing to lay it but a swape each
/// end and the labourers' hands.
#[test]
#[ignore = "writes the hand-chain proof frames; needs ANGEL_REALM_PLAN=tabula"]
fn write_a_hand_chain_between_two_stores() {
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let mut run = run_at(166.5, 103.5);
    let from = depot(&mut run, (158, 105));
    let to = depot(&mut run, (168, 105));
    run.home.industry.buildings.get_mut(&from).unwrap().output.insert("plank".into(), 60);
    let mut reel = Reel::new("chain", 8);
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    lay_swape(&mut reel, &mut run, (161, 104), Dir::S, Dir::E);
    lay_piece(&mut reel, &mut run, Piece::Chain, (162, 104), Dir::S, Dir::E);
    paint(&mut reel, &mut run, Piece::Chain, (163, 105), (166, 105));
    lay_swape(&mut reel, &mut run, (166, 105), Dir::E, Dir::E);
    println!("hand-chain: {} tiles, crew wanted {}, store holds {:?}", run.home.industry.rollers.cells.len(), run.home.industry.chain_crew(catalog()), run.home.industry.buildings[&to].output);
    reel.press(&mut run, Input { cast: 4, ..Default::default() });
    reel.every = 6;
    for _ in 0..6 {
        reel.wait(&mut run, 10);
        let k = &run.home.industry;
        println!("hand-chain: {} goods on it, {} in the store, crew at it {}", k.rollers.cells.values().map(|c| c.held()).sum::<u32>(), k.buildings[&to].output.get("plank").copied().unwrap_or(0), k.report(catalog()).population.assigned.get(&1).copied().unwrap_or(0));
    }
    println!("frames {}", reel.n);
}
