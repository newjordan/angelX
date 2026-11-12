//! The cave's world on its own: generation, the materials' reactions, the
//! shovel, the blast, the spells and the ice. Two scripted walks cross the
//! whole map so a change to generation that cuts a region off, or to the
//! materials that stops a reaction, fails here.

use super::*;

use crate::rules::HZ;

const SIM_EVERY: u32 = 2;
const WALK_TICKS: u32 = HZ * 20;
const TOUR_TICKS: u32 = HZ * 30;
const TURN_ICE: u8 = 3;
const TURN_DRY: u8 = 1;

/// Scenario helpers: a test sets a cell up or pours a fluid by hand.
impl Cave {
    fn ignite(&mut self, x: i32, y: i32) {
        if let Some(c) = self.at_mut(x, y) {
            if c.wood() || c.grass() {
                c.fire = 40;
            }
        }
        self.wake(x, y);
    }

    fn spill_acid(&mut self, x: i32, y: i32) {
        if let Some(c) = self.at_mut(x, y) {
            c.flags &= !(FL_WALL | FL_SEALED | FL_WOOD);
            c.fluid = FLUID_ACID;
            c.amount = 4;
        }
        self.wake(x, y);
    }

    fn slot_mut(&mut self, x: i32, y: i32) -> &mut Cell {
        let i = self.world.idx(x, y).expect("cell");
        &mut self.world.cells[i]
    }

    fn open_wall(&mut self, x: i32, y: i32) {
        if let Some(c) = self.at_mut(x, y) {
            c.flags &= !(FL_WALL | FL_BRICK);
            c.integrity = 0;
        }
        self.wake(x, y);
    }

    fn pour_water(&mut self, x: i32, y: i32) {
        if let Some(c) = self.at_mut(x, y) {
            c.flags &= !(FL_WALL | FL_BRICK | FL_SEALED | FL_WOOD);
            c.fluid = FLUID_WATER;
            c.amount = c.amount.saturating_add(4).min(6);
        }
        self.wake(x, y);
    }

    fn pour_lava(&mut self, x: i32, y: i32) {
        if let Some(c) = self.at_mut(x, y) {
            c.flags &= !(FL_WALL | FL_BRICK | FL_SEALED | FL_WOOD);
            c.fluid = FLUID_LAVA;
            c.amount = c.amount.saturating_add(4).min(6);
            c.fire = 0;
        }
        self.wake(x, y);
    }
}

impl Pack {
    fn new() -> Self {
        Self { magic: MAGIC_MAX, stone: [0; 4], footwear: Footwear::Plain }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    East,
    West,
    North,
    South,
}

fn turn_ticks(flags: u16, footwear: Footwear) -> u8 {
    if slippery(flags, footwear) { TURN_ICE } else { TURN_DRY }
}

/// A knight carried over the cells of a row: the slide's rules on their own,
/// as play applies them a step at a time.
struct Glide {
    x: i32,
    y: i32,
    face: Face,
    want: Face,
    turn: u8,
    coast: i32,
    footwear: Footwear,
    moving: bool,
}

impl Glide {
    fn new(x: i32, y: i32, footwear: Footwear) -> Self {
        Self {
            x,
            y,
            face: Face::East,
            want: Face::East,
            turn: 0,
            coast: 0,
            footwear,
            moving: false,
        }
    }

    fn kick(&mut self, face: Face) {
        self.want = face;
        self.face = face;
        self.turn = 0;
        self.moving = true;
        self.coast = 0;
    }

    fn release(&mut self, flags: u16) {
        self.moving = false;
        self.coast = coast_cells(flags, self.footwear);
    }

    fn step_input(&mut self, cave: &Cave) -> bool {
        self.step_face(cave)
    }

    fn step_coast(&mut self, cave: &Cave) -> bool {
        if self.coast <= 0 {
            return false;
        }
        let flags = cave.at(self.x, self.y).map(|c| c.flags).unwrap_or(0);
        if !slippery(flags, self.footwear) {
            self.coast = 0;
            return false;
        }
        if self.step_face(cave) {
            self.coast -= 1;
            let next = cave.at(self.x, self.y).map(|c| c.flags).unwrap_or(0);
            if !slippery(next, self.footwear) {
                self.coast = 0;
            }
            true
        } else {
            self.coast = 0;
            false
        }
    }

    fn step_face(&mut self, cave: &Cave) -> bool {
        let (dx, dy) = match self.face {
            Face::North => (0, -1),
            Face::South => (0, 1),
            Face::East => (1, 0),
            Face::West => (-1, 0),
        };
        let nx = self.x + dx;
        let ny = self.y + dy;
        if cave.at(nx, ny).is_some_and(|c| c.open()) {
            self.x = nx;
            self.y = ny;
            true
        } else {
            false
        }
    }

    fn ask_turn(&mut self, face: Face, flags: u16) -> bool {
        if face == self.face {
            self.want = face;
            self.turn = 0;
            return true;
        }
        if self.want != face {
            self.want = face;
            self.turn = 0;
        }
        self.turn = self.turn.saturating_add(1);
        if self.turn >= turn_ticks(flags, self.footwear) {
            self.face = face;
            self.turn = 0;
            true
        } else {
            false
        }
    }
}

struct Segment {
    from: u32,
    to: u32,
    a: (i32, i32),
    b: (i32, i32),
    hold: bool,
}

/// A scripted walk through the mine, the hollow and the foundry. The knight is
/// moved; the materials react on their own.
struct Show {
    cave: Cave,
    tick: u32,
    path: Vec<(i32, i32)>,
    fired: bool,
    flooded: bool,
    tipped: bool,
    broke: bool,
    bolt: u8,
}

impl Show {
    fn demo() -> Self {
        let cave = Cave::generate(DEMO_SEED);
        let path = route(&cave);
        Self {
            cave,
            tick: 0,
            path,
            fired: false,
            flooded: false,
            tipped: false,
            broke: false,
            bolt: 0,
        }
    }

    fn tick(&mut self) {
        self.advance_knight_events();
        if self.tick % SIM_EVERY == 0 {
            self.cave.step_sim();
        }
        self.tick += 1;
    }

    fn advance_knight_events(&mut self) {
        if self.tick == 100 && !self.fired {
            self.bolt = 8;
        }
        if self.bolt == 1 {
            for (x, y) in [(42, 26), (42, 28), (42, 30), (38, 28), (46, 28), (36, 27), (48, 27)] {
                self.cave.ignite(x, y);
            }
            self.fired = true;
        }
        if self.tick == 200 {
            for (x, y) in [(22, 27), (26, 27), (32, 27)] {
                self.cave.ignite(x, y);
            }
            for y in 26..30 {
                for x in 20..33 {
                    self.cave.ignite(x, y);
                }
            }
        }
        if self.bolt > 0 {
            self.bolt -= 1;
        }
        if self.tick == 230 && !self.flooded {
            for x in 21..29 {
                for y in 21..24 {
                    self.cave.open_wall(x, y);
                }
            }
            self.flooded = true;
            for x in 22..32 {
                self.cave.pour_water(x, 25);
            }
        }
        if self.tick == 430 && !self.tipped {
            for x in 111..117 {
                self.cave.spill_acid(x, 60);
            }
            self.tipped = true;
        }
        if self.tick == 540 && !self.broke {
            for x in 147..154 {
                for y in 36..38 {
                    self.cave.open_wall(x, y);
                }
            }
            self.broke = true;
            for x in 149..153 {
                self.cave.pour_water(x, 42);
                self.cave.pour_lava(x, 41);
            }
        }
    }

    fn pose(&self) -> ((i32, i32), Face) {
        let segs = segments();
        let tick = self.tick.min(WALK_TICKS.saturating_sub(1));
        let seg = segs.iter().find(|s| tick >= s.from && tick < s.to).unwrap_or(segs.last().unwrap());
        if seg.hold || self.path.is_empty() {
            let face = if seg.b.0 < seg.a.0 { Face::West } else if seg.b.1 < seg.a.1 { Face::North } else if seg.b.1 > seg.a.1 { Face::South } else { Face::East };
            return (seg.a, face);
        }
        let span = (seg.to - seg.from).max(1);
        let t = (tick - seg.from) as f32 / span as f32;
        let along = route_between(&self.path, seg.a, seg.b);
        let f = t * (along.len().saturating_sub(1) as f32);
        let i = (f as usize).min(along.len().saturating_sub(1));
        let j = (i + 1).min(along.len().saturating_sub(1));
        let frac = f - i as f32;
        let (x0, y0) = along[i];
        let (x1, y1) = along[j];
        let x = x0 as f32 + (x1 - x0) as f32 * frac;
        let y = y0 as f32 + (y1 - y0) as f32 * frac;
        let face = if x1 < x0 {
            Face::West
        } else if x1 > x0 {
            Face::East
        } else if y1 < y0 {
            Face::North
        } else {
            Face::South
        };
        ((x.round() as i32, y.round() as i32), face)
    }
}

fn segments() -> [Segment; 8] {
    [
        Segment { from: 0, to: 100, a: (16, spine_y(16)), b: (40, 32), hold: false },
        Segment { from: 100, to: 170, a: (40, 32), b: (40, 32), hold: true },
        Segment { from: 170, to: 230, a: (40, 32), b: (26, 28), hold: false },
        Segment { from: 230, to: 320, a: (26, 28), b: (26, 28), hold: true },
        Segment { from: 320, to: 430, a: (26, 28), b: (114, 54), hold: false },
        Segment { from: 430, to: 510, a: (114, 54), b: (114, 54), hold: true },
        Segment { from: 510, to: 540, a: (114, 54), b: (156, 44), hold: false },
        Segment { from: 540, to: WALK_TICKS, a: (156, 44), b: (156, 44), hold: true },
    ]
}

fn route(cave: &Cave) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    let marks = [(16, spine_y(16)), (40, 32), (26, 28), (96, 40), (114, 54), (156, 44), (176, spine_y(176))];
    for w in marks {
        assert!(cave.at(w.0, w.1).is_some_and(|c| c.open()), "waypoint {w:?} is not open");
    }
    for pair in marks.windows(2) {
        let leg = bfs(cave, pair[0], pair[1]).unwrap_or_else(|| panic!("no path {:?} -> {:?}", pair[0], pair[1]));
        if out.is_empty() {
            out.extend(leg);
        } else {
            out.extend(leg.into_iter().skip(1));
        }
    }
    out
}

fn route_between(path: &[(i32, i32)], a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let ia = path.iter().position(|p| *p == a);
    let ib = path.iter().position(|p| *p == b);
    if let (Some(ia), Some(ib)) = (ia, ib) {
        if ia <= ib {
            return path[ia..=ib].to_vec();
        }
        let mut rev = path[ib..=ia].to_vec();
        rev.reverse();
        return rev;
    }
    vec![a, b]
}

fn bfs(cave: &Cave, start: (i32, i32), goal: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    let n = (MAP_W * MAP_H) as usize;
    let mut prev = vec![u32::MAX; n];
    let mut q = std::collections::VecDeque::new();
    let s = (start.1 * MAP_W + start.0) as usize;
    prev[s] = s as u32;
    q.push_back(start);
    while let Some((x, y)) = q.pop_front() {
        if (x, y) == goal {
            break;
        }
        for (nx, ny) in neighbors(x, y) {
            let Some(c) = cave.at(nx, ny) else { continue };
            if !c.open() {
                continue;
            }
            let j = (ny * MAP_W + nx) as usize;
            if prev[j] != u32::MAX {
                continue;
            }
            prev[j] = (y * MAP_W + x) as u32;
            q.push_back((nx, ny));
        }
    }
    let g = (goal.1 * MAP_W + goal.0) as usize;
    if prev[g] == u32::MAX {
        return None;
    }
    let mut cur = goal;
    let mut leg = vec![goal];
    while cur != start {
        let p = prev[(cur.1 * MAP_W + cur.0) as usize] as i32;
        cur = (p % MAP_W, p / MAP_W);
        leg.push(cur);
    }
    leg.reverse();
    Some(leg)
}

/// A scripted tour of the shovel, the blast and the ice across all four regions.
struct Tour {
    cave: Cave,
    pack: Pack,
    path: Vec<(i32, i32)>,
    marks: [usize; 5],
    cursor: usize,
    pose: (i32, i32),
    tick: u32,
    face: Face,
    stone_rose: bool,
    meter_dropped: bool,
    slid: i32,
    melted: bool,
    coast_left: i32,
}

impl Tour {
    fn demo() -> Self {
        let cave = Cave::generate(DEMO_SEED);
        let points = [
            (16, spine_y(16)),
            (13, 39),
            (19, 39),
            (96, 40),
            (158, 42),
            (100, 10),
        ];
        let mut path = vec![points[0]];
        let mut marks = [0usize; 5];
        for (i, point) in points[1..].iter().enumerate() {
            let leg = bfs(&cave, *path.last().unwrap(), *point).unwrap_or_default();
            if !leg.is_empty() {
                path.extend(leg);
            }
            marks[i] = path.len() - 1;
        }
        let pose = path[0];
        Self {
            cave,
            pack: Pack::new(),
            path,
            marks,
            cursor: 0,
            pose,
            tick: 0,
            face: Face::East,
            stone_rose: false,
            meter_dropped: false,
            slid: 0,
            melted: false,
            coast_left: 0,
        }
    }

    fn tick(&mut self) {
        if self.tick % SIM_EVERY == 0 {
            self.cave.step_sim();
        }
        self.tick += 1;
        let t = self.tick;
        if t < 70 {
            self.walk_toward(0);
            self.face = Face::North;
        } else if t < 150 {
            self.face = Face::North;
            if t == 90 || t == 110 || t == 130 {
                let before = self.pack.total();
                let _ = self.cave.shovel(&mut self.pack, 13, 36);
                if self.pack.total() > before {
                    self.stone_rose = true;
                }
            }
        } else if t < 210 {
            self.walk_toward(1);
        } else if t < 280 {
            self.face = Face::North;
            if t == 230 {
                let before = self.pack.magic;
                let _ = self.cave.blast(&mut self.pack, 19, 36);
                if self.pack.magic < before {
                    self.meter_dropped = true;
                }
            }
        } else if t < 470 {
            self.walk_toward(2);
        } else if t < 650 {
            self.walk_toward(3);
        } else if t < 790 {
            self.walk_toward(4);
        } else if t < 870 {
            self.slide();
        } else if t == 870 {
            self.cave.melt_ice(102, 10);
            self.cave.ignite(102, 9);
            self.melted = true;
        }
    }

    fn walk_toward(&mut self, mark: usize) {
        let goal = self.marks[mark];
        if self.tick % 2 == 0 && self.cursor < goal {
            let prev = self.pose;
            self.cursor += 1;
            self.pose = self.path[self.cursor];
            self.face = face_of(prev, self.pose);
        }
    }

    fn slide(&mut self) {
        if self.cursor < self.marks[4] && self.tick < 820 {
            self.walk_toward(4);
            return;
        }
        if self.pose.1 >= 16 || !self.cave.at(self.pose.0, self.pose.1).is_some_and(|c| c.ice() && c.open()) {
            self.pose = (100, 10);
        }
        self.face = Face::East;
        if self.coast_left == 0 && self.slid == 0 {
            self.coast_left = coast_cells(FL_ICE, self.pack.footwear);
        }
        if self.tick % 4 != 0 {
            return;
        }
        if self.coast_left > 0 {
            let flags = self.cave.at(self.pose.0, self.pose.1).map(|c| c.flags).unwrap_or(0);
            if !slippery(flags, self.pack.footwear) {
                self.coast_left = 0;
                return;
            }
            if self.try_east() {
                self.slid += 1;
                self.coast_left -= 1;
                let flags = self.cave.at(self.pose.0, self.pose.1).map(|c| c.flags).unwrap_or(0);
                if !slippery(flags, self.pack.footwear) {
                    self.coast_left = 0;
                }
            } else {
                self.coast_left = 0;
            }
        }
    }

    fn try_east(&mut self) -> bool {
        let next = (self.pose.0 + 1, self.pose.1);
        if self.cave.at(next.0, next.1).is_some_and(|c| c.open()) {
            self.pose = next;
            true
        } else {
            false
        }
    }
}

fn face_of(from: (i32, i32), to: (i32, i32)) -> Face {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    if dx.abs() > dy.abs() {
        if dx > 0 { Face::East } else { Face::West }
    } else if dy > 0 {
        Face::South
    } else {
        Face::North
    }
}

#[test]
fn every_open_cell_is_reachable() {
    for seed in [1u32, DEMO_SEED, 99] {
        let cave = Cave::generate(seed);
        let start = (8, spine_y(8));
        assert!(cave.at(start.0, start.1).unwrap().open());
        let mut seen = vec![false; (MAP_W * MAP_H) as usize];
        let mut q = vec![start];
        seen[(start.1 * MAP_W + start.0) as usize] = true;
        let mut reached = 0;
        while let Some((x, y)) = q.pop() {
            reached += 1;
            for (nx, ny) in neighbors(x, y) {
                let Some(c) = cave.at(nx, ny) else { continue };
                if !c.open() {
                    continue;
                }
                let j = (ny * MAP_W + nx) as usize;
                if !seen[j] {
                    seen[j] = true;
                    q.push((nx, ny));
                }
            }
        }
        let open = cave.world.cells.iter().filter(|c| c.open()).count();
        assert_eq!(reached, open, "seed {seed} left an unreachable open cell");
    }
}

#[test]
fn step_is_deterministic() {
    let mut a = Cave::generate(DEMO_SEED);
    let mut b = Cave::generate(DEMO_SEED);
    a.ignite(42, 28);
    b.ignite(42, 28);
    a.open_wall(24, 22);
    b.open_wall(24, 22);
    for _ in 0..40 {
        a.step_sim();
        b.step_sim();
    }
    assert_eq!(a.fingerprint(), b.fingerprint());
}

fn tiny() -> World {
    let mut cells = vec![
        Cell {
            flags: 0,
            fire: 0,
            fluid: FLUID_NONE,
            amount: 0,
            integrity: 255,
            steam: 0,
            height: 0,
            stone: 0,
            melt: 0,
            swings: 0,
        };
        16
    ];
    for y in 0..4 {
        for x in 0..4 {
            cells[(y * 4 + x) as usize].height = y as u8;
        }
    }
    World { w: 4, h: 4, cells, awake: vec![true; 16], steps: 0 }
}

#[test]
fn fire_spreads_over_wood_and_grass_then_becomes_ash() {
    let mut w = tiny();
    w.cells[0].flags |= FL_WOOD;
    w.cells[1].flags |= FL_GRASS;
    w.cells[0].fire = 2;
    w.step();
    w.step();
    assert!(w.cells[1].fire > 0, "fire did not spread onto grass");
    for _ in 0..45 {
        w.awake.fill(true);
        w.step();
    }
    assert!(w.cells[0].ash() || w.cells[1].ash(), "fuel did not burn out to ash");
    assert_eq!(w.cells[0].fire, 0);
}

#[test]
fn water_flows_and_puts_fire_out() {
    let mut w = tiny();
    w.cells[0].fluid = FLUID_WATER;
    w.cells[0].amount = 3;
    w.cells[0].height = 0;
    w.cells[4].flags |= FL_GRASS;
    w.cells[4].fire = 12;
    w.cells[4].height = 1;
    w.awake.fill(true);
    w.step();
    assert!(w.cells[4].steam > 0 || w.cells[0].steam > 0, "water did not quench fire into steam");
    assert_eq!(w.cells[4].fire, 0);
    assert!(w.cells[4].fluid == FLUID_WATER || w.cells[0].amount < 3, "water did not flow downhill");
}

#[test]
fn acid_eats_a_rock_wall() {
    let mut w = tiny();
    w.cells[0].fluid = FLUID_ACID;
    w.cells[0].amount = 2;
    w.cells[1].flags |= FL_WALL;
    w.cells[1].integrity = 2;
    w.awake.fill(true);
    w.step();
    w.awake.fill(true);
    w.step();
    assert!(!w.cells[1].wall(), "acid left the rock wall standing");
}

#[test]
fn lava_and_water_make_rock() {
    let mut w = tiny();
    w.cells[0].fluid = FLUID_LAVA;
    w.cells[0].amount = 3;
    w.cells[1].fluid = FLUID_WATER;
    w.cells[1].amount = 3;
    w.awake.fill(true);
    w.step();
    assert!(w.cells[0].fresh() && w.cells[0].wall(), "lava did not become rock");
    assert!(w.cells[1].fresh() && w.cells[1].wall(), "water did not become rock");
    assert!(w.cells[0].steam > 0 && w.cells[1].steam > 0);
}

#[test]
fn scripted_walk_reaches_three_regions_and_reactions() {
    let mut show = Show::demo();
    let mut regions = [false; 3];
    let mut ash = false;
    let mut steam = false;
    let mut opened = false;
    let mut fresh = false;
    let mut fire = false;
    for _ in 0..WALK_TICKS {
        show.tick();
        let (cell, _) = show.pose();
        regions[region_of(cell.0, cell.1).0 as usize] = true;
        for c in &show.cave.world.cells {
            fire |= c.fire > 0;
            ash |= c.ash();
            steam |= c.steam > 0;
            fresh |= c.fresh();
        }
        opened |= show.cave.at(114, 66).is_some_and(|c| c.open());
    }
    assert!(regions.iter().all(|r| *r), "walk missed a region: {regions:?}");
    assert!(fire, "fire never caught");
    assert!(ash, "fire never burned out to ash");
    assert!(steam, "no steam from a quench or a meeting");
    assert!(opened, "acid never opened the hidden chamber");
    assert!(fresh, "lava and water never made rock");
}

#[test]
fn shovel_breaks_dirt_after_three_swings_and_raises_the_tally() {
    let mut cave = Cave::generate(1);
    let mut pack = Pack::new();
    cave.slot_mut(3, 3).flags = FL_WALL | FL_LOOSE;
    cave.slot_mut(3, 3).stone = 0;
    cave.slot_mut(3, 3).swings = SHOVEL_SWINGS;
    assert!(matches!(cave.shovel(&mut pack, 3, 3), Dig::Swing(2)));
    assert!(matches!(cave.shovel(&mut pack, 3, 3), Dig::Swing(1)));
    match cave.shovel(&mut pack, 3, 3) {
        Dig::Broke { kind, got } => {
            assert_eq!(kind, 0);
            assert_eq!(got, 1);
        }
        other => panic!("expected a break, got {other:?}"),
    }
    assert_eq!(pack.total(), 1);
    assert!(!cave.at(3, 3).unwrap().wall());
}

#[test]
fn shovel_rings_on_hard_rock() {
    let mut cave = Cave::generate(1);
    let mut pack = Pack::new();
    cave.slot_mut(3, 3).flags = FL_WALL | FL_HARD;
    cave.slot_mut(3, 3).swings = SHOVEL_SWINGS;
    assert!(matches!(cave.shovel(&mut pack, 3, 3), Dig::Ring));
    assert!(cave.at(3, 3).unwrap().wall());
    assert_eq!(pack.total(), 0);
}

#[test]
fn rubble_falls_and_can_be_shovelled() {
    let mut cave = Cave::generate(1);
    let mut pack = Pack::new();
    cave.slot_mut(3, 3).flags = FL_WALL | FL_LOOSE;
    cave.slot_mut(3, 3).stone = 0;
    cave.slot_mut(3, 3).swings = SHOVEL_SWINGS;
    cave.slot_mut(3, 2).flags = FL_WALL | FL_HARD;
    cave.slot_mut(3, 2).stone = 0;
    for _ in 0..3 {
        let _ = cave.shovel(&mut pack, 3, 3);
    }
    assert!(cave.at(3, 2).unwrap().loose(), "the rock above did not fall loose");
    assert!(!cave.at(3, 2).unwrap().hard());
    for _ in 0..3 {
        let _ = cave.shovel(&mut pack, 3, 2);
    }
    assert!(!cave.at(3, 2).unwrap().wall());
    assert!(pack.total() >= 2);
}

#[test]
fn magic_blast_breaks_hard_rock_and_spends_the_meter() {
    let mut cave = Cave::generate(1);
    let mut pack = Pack::new();
    cave.slot_mut(4, 4).flags = FL_WALL | FL_HARD;
    cave.slot_mut(4, 4).stone = 2;
    assert!(cave.blast(&mut pack, 4, 4));
    assert_eq!(pack.magic, MAGIC_MAX - BLAST_COST);
    assert!(!cave.at(4, 4).unwrap().wall());
    assert!(pack.stone[2] >= 1);
}

#[test]
fn magic_blast_does_nothing_when_the_meter_is_empty() {
    let mut cave = Cave::generate(1);
    let mut pack = Pack::new();
    pack.magic = BLAST_COST - 1;
    cave.slot_mut(4, 4).flags = FL_WALL | FL_HARD;
    assert!(!cave.blast(&mut pack, 4, 4));
    assert!(cave.at(4, 4).unwrap().wall());
    assert_eq!(pack.magic, BLAST_COST - 1);
}

#[test]
fn carry_limit_is_fixed() {
    let mut cave = Cave::generate(1);
    let mut pack = Pack::new();
    pack.stone[0] = CARRY_LIMIT;
    cave.slot_mut(2, 2).flags = FL_WALL | FL_LOOSE;
    cave.slot_mut(2, 2).swings = 1;
    match cave.shovel(&mut pack, 2, 2) {
        Dig::Broke { got, .. } => assert_eq!(got, 0),
        other => panic!("expected a full pack, got {other:?}"),
    }
    assert_eq!(pack.total(), CARRY_LIMIT);
}

#[test]
fn stone_type_follows_the_region() {
    let cave = Cave::generate(DEMO_SEED);
    assert_eq!(cave.at(13, 36).unwrap().stone, 0, "mine stone");
    assert_eq!(cave.at(96, 20).unwrap().stone, 1, "hollow stone");
    assert_eq!(cave.at(160, 30).unwrap().stone, 2, "foundry stone");
    assert_eq!(cave.at(100, 8).unwrap().stone, 3, "frost stone");
    assert!(cave.at(13, 36).unwrap().loose());
    assert!(cave.at(19, 36).unwrap().hard());
}

#[test]
fn slide_goes_farther_on_ice_than_on_rock() {
    assert_eq!(coast_cells(FL_ICE, Footwear::Plain), SLIDE_ICE);
    assert_eq!(coast_cells(FL_ICE, Footwear::Blades), SLIDE_BLADES);
    assert_eq!(SLIDE_BLADES, 8);
    assert!(SLIDE_BLADES > SLIDE_ICE, "blades coast farther than plain boots on the same ice");
    assert_eq!(coast_cells(0, Footwear::Plain), SLIDE_DRY);
    assert!(coast_cells(FL_ICE, Footwear::Plain) > coast_cells(0, Footwear::Plain));
    assert_eq!(run_glide(FL_ICE, Footwear::Plain), SLIDE_ICE);
    assert_eq!(run_glide(0, Footwear::Plain), SLIDE_DRY);
}

#[test]
fn grip_boots_cancel_ice() {
    assert_eq!(coast_cells(FL_ICE, Footwear::Grip), 0);
    assert_eq!(run_glide(FL_ICE, Footwear::Grip), 0);
}

#[test]
fn blades_slide_on_smooth_stone() {
    assert_eq!(coast_cells(FL_SMOOTH, Footwear::Blades), SLIDE_ICE);
    assert_eq!(coast_cells(FL_SMOOTH, Footwear::Plain), SLIDE_DRY);
    assert_eq!(coast_cells(FL_RUBBLE, Footwear::Blades), SLIDE_DRY);
    assert_eq!(run_glide(FL_SMOOTH, Footwear::Blades), SLIDE_ICE);
}

#[test]
fn ice_turns_slower_than_dry_ground() {
    let mut glide = Glide::new(4, 4, Footwear::Plain);
    glide.face = Face::East;
    assert!(!glide.ask_turn(Face::North, FL_ICE));
    assert!(!glide.ask_turn(Face::North, FL_ICE));
    assert!(glide.ask_turn(Face::North, FL_ICE));
    assert!(glide.face == Face::North);
    let mut dry = Glide::new(4, 4, Footwear::Plain);
    dry.face = Face::East;
    assert!(dry.ask_turn(Face::South, 0));
    assert!(dry.face == Face::South);
}

#[test]
fn fire_melts_ice_and_water_refreezes() {
    let mut cave = Cave::generate(1);
    cave.slot_mut(6, 6).flags = FL_ICE;
    cave.slot_mut(6, 6).fluid = FLUID_NONE;
    for (nx, ny) in neighbors(6, 6) {
        cave.slot_mut(nx, ny).flags |= FL_WALL;
        cave.slot_mut(nx, ny).height = 255;
    }
    cave.melt_ice(6, 6);
    assert_eq!(cave.at(6, 6).unwrap().fluid, FLUID_WATER);
    assert!(!cave.at(6, 6).unwrap().ice());
    for _ in 0..REFREEZE_TICKS {
        cave.world.awake.fill(true);
        cave.step_sim();
    }
    let cell = cave.at(6, 6).unwrap();
    assert_eq!(cell.fluid, FLUID_NONE, "water did not refreeze");
    assert!(cell.ice(), "refrozen cell lost its ice");
}

#[test]
fn frost_bridges_water_and_lava() {
    let mut cave = Cave::generate(1);
    let mut pack = Pack::new();
    cave.slot_mut(7, 7).fluid = FLUID_WATER;
    cave.slot_mut(7, 7).amount = 4;
    cave.slot_mut(7, 7).flags &= !FL_WALL;
    assert!(cave.frost_bridge(&mut pack, 7, 7));
    assert_eq!(cave.at(7, 7).unwrap().fluid, FLUID_NONE);
    assert!(cave.at(7, 7).unwrap().ice());
    cave.slot_mut(8, 7).fluid = FLUID_LAVA;
    cave.slot_mut(8, 7).amount = 4;
    cave.slot_mut(8, 7).flags &= !FL_WALL;
    assert!(cave.frost_bridge(&mut pack, 8, 7));
    assert!(cave.at(8, 7).unwrap().ice());
    assert_ne!(cave.at(8, 7).unwrap().fluid, FLUID_LAVA);
    assert!(pack.magic < MAGIC_MAX);
}

#[test]
fn tour_shows_dig_blast_slide_and_the_four_regions() {
    let mut tour = Tour::demo();
    let mut regions = [false; 4];
    for _ in 0..TOUR_TICKS {
        tour.tick();
        let r = region_of(tour.pose.0, tour.pose.1).0 as usize;
        if r < 4 {
            regions[r] = true;
        }
    }
    assert!(tour.stone_rose, "shovel tally never rose");
    assert!(tour.meter_dropped, "magic meter never dropped");
    assert!(tour.slid > 0, "knight did not coast on ice");
    assert!(tour.melted, "fire never melted ice");
    assert!(regions.iter().all(|r| *r), "tour missed a region: {regions:?}");
}

fn run_glide(flags: u16, footwear: Footwear) -> i32 {
    let mut cave = Cave::generate(1);
    for x in 0..30 {
        let cell = &mut cave.slot_mut(x, 10);
        cell.flags &= !(FL_WALL | FL_WOOD | FL_SEALED | FL_ICE | FL_SMOOTH | FL_RUBBLE | FL_MOSS | FL_HARD);
        cell.flags |= flags;
        cell.fluid = FLUID_NONE;
        cell.amount = 0;
    }
    let mut glide = Glide::new(2, 10, footwear);
    glide.kick(Face::East);
    assert!(glide.step_input(&cave));
    let x0 = glide.x;
    glide.release(flags);
    for _ in 0..12 {
        if !glide.step_coast(&cave) {
            break;
        }
    }
    glide.x - x0
}
