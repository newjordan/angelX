//! The one seam between the world renderer and the race's courses: what the
//! renderer asks of a course, and the course (`rules::Course`) answering.
use super::super::rules;


/// How a stretch of land looks: its palette, sky, fog and what lies beside the road.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Look { Dune, Forge, Ash, Deep, Castle }

impl Look {
    /// The course a look belongs to: each look is one course's own, so a look is also a place.
    pub fn course(self) -> u32 { match self { Look::Dune => 0, Look::Forge | Look::Ash => 1, Look::Deep => 2, Look::Castle => 3 } }
}

/// The set pieces the renderer knows how to raise.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Bridge, Viaduct, Tunnel, Arch, Grandstand, CastleGate, Foundry, Waterfall, CrystalVault,
    ChasmJump, Drawbridge, LavaFall, Statues, Lanterns, Pylons, Gantry, Rampart, Spires,
    // Set pieces of the world renderer's own, set by name of the stretch (see `extras`).
    Stacks, Anvil,
}

/// `side` is -1 (right) or +1 (left) for a piece beside the road, 0 for one that spans it.
#[derive(Clone, Copy, Debug)]
pub struct Piece { pub kind: Kind, pub s0: f32, pub s1: f32, pub side: f32, pub seed: u32 }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HazardKind { Slag, Rockfall, Jet, Slick }

#[derive(Clone, Copy, Debug)]
pub struct Hazard { pub kind: HazardKind, pub s: f32, pub y: f32, pub half_l: f32, pub half_w: f32, pub period: u32, pub on: u32, pub phase: u32 }

/// A boost strip across the road at `y`.
#[derive(Clone, Copy, Debug)]
pub struct Strip { pub s0: f32, pub s1: f32, pub half_w: f32, pub y: f32 }

/// A stretch where the road is missing.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct Gap { pub s0: f32, pub s1: f32, pub ramp: f32 }

/// An alternate road between two points of the main line, over main-line
/// distances `s0..s1`. Its own pieces, hazards and strips use the same distances.
pub struct ForkInfo { pub s0: f32, pub s1: f32, pub side: f32, pub look: Look, pub pieces: Vec<Piece>, pub hazards: Vec<Hazard>, pub strips: Vec<Strip> }

/// The road at one distance on one of a course's roads.
#[derive(Clone, Copy, Debug)]
pub struct RoadFrame { pub x: f32, pub y: f32, pub th: f32, pub h: f32, pub bank: f32, pub wl: f32, pub wr: f32 }

/// A course, as the world renderer needs to see it.
pub trait Route {
    /// Distinguishes this course's cached chunks from every other's.
    fn key(&self) -> u32;
    fn lap(&self) -> f32;
    fn pose(&self, s: f32) -> (f32, f32, f32);
    fn height(&self, s: f32) -> f32;
    /// Left and right half-widths of the racing surface.
    fn width(&self, s: f32) -> (f32, f32);
    fn zone(&self, s: f32) -> (Look, &'static str);
    /// How far the land drops beside the road at `s` (a moat, a chasm, a lake): 0 for none.
    fn sink(&self, s: f32) -> f32;
    /// Terrain height anywhere in world xy.
    fn ground(&self, x: f32, y: f32) -> f32;
    fn pieces(&self) -> &[Piece];
    /// Alternate roads, numbered from 1 (road 0 is the main line).
    fn forks(&self) -> &[ForkInfo];
    /// The road `line` at main-line distance `s`.
    fn frame(&self, line: u8, s: f32) -> RoadFrame;
    fn hazards(&self) -> Vec<Hazard>;
    fn strips(&self) -> Vec<Strip>;
    fn gaps(&self) -> Vec<Gap>;
    fn landmarks(&self) -> Vec<f32>;
    /// Road curvature at `s`, positive in a left-hand bend (1/m).
    fn curve(&self, s: f32) -> f32;
    /// Every named stretch of the lap: (from, to, look, name).
    fn zones(&self) -> Vec<(f32, f32, Look, &'static str)>;
}

/// The look of a biome on course `id`: the moor of the Foundry Run is ash flats under its smoke,
/// the moor of the Glassway is the mouth of the mine, the moor of Moatwalk is its own night.
fn look_of(id: rules::CourseId, b: rules::Biome) -> Look {
    use rules::Biome as B;
    match (id, b) {
        (_, B::Proving) => Look::Dune,
        (rules::CourseId::Emberfall, B::Moor) => Look::Ash,
        (rules::CourseId::Glassway, B::Moor) => Look::Deep,
        (_, B::Forge | B::Foundry | B::Magma) => Look::Forge,
        (_, B::Crystal | B::Cavern) => Look::Deep,
        (_, B::Moor | B::Castle | B::Town | B::Moat) => Look::Castle,
    }
}

impl From<rules::PieceKind> for Kind {
    fn from(k: rules::PieceKind) -> Kind {
        use rules::PieceKind as P;
        match k {
            P::Bridge => Kind::Bridge, P::Viaduct => Kind::Viaduct, P::Tunnel => Kind::Tunnel, P::Arch => Kind::Arch,
            P::Grandstand => Kind::Grandstand, P::CastleGate => Kind::CastleGate, P::Foundry => Kind::Foundry,
            P::Waterfall => Kind::Waterfall, P::CrystalVault => Kind::CrystalVault, P::ChasmJump => Kind::ChasmJump,
            P::Drawbridge => Kind::Drawbridge, P::LavaFall => Kind::LavaFall, P::Statues => Kind::Statues,
            P::Lanterns => Kind::Lanterns, P::Pylons => Kind::Pylons, P::Gantry => Kind::Gantry,
            P::Rampart => Kind::Rampart, P::Spires => Kind::Spires,
        }
    }
}

fn hazards(list: &[rules::Hazard]) -> Vec<Hazard> {
    use rules::HazardKind as H;
    list.iter().map(|h| Hazard {
        kind: match h.kind { H::Slag => HazardKind::Slag, H::Rockfall => HazardKind::Rockfall, H::Jet => HazardKind::Jet, H::Slick => HazardKind::Slick },
        s: h.s, y: h.y, half_l: h.half_l, half_w: h.half_w, period: h.period, on: h.on, phase: h.phase,
    }).collect()
}

fn strips(list: &[rules::Strip]) -> Vec<Strip> { list.iter().map(|p| Strip { s0: p.s0, s1: p.s1, half_w: p.half_w, y: p.y }).collect() }

fn pieces(list: &[rules::Piece]) -> Vec<Piece> { list.iter().map(|p| Piece { kind: p.kind.into(), s0: p.s0, s1: p.s1, side: p.side, seed: p.seed }).collect() }

/// A course's own pieces, converted once.
struct Dressed { route: &'static rules::Course, pieces: Vec<Piece>, forks: Vec<ForkInfo>, sinks: Vec<(f32, f32, f32)> }

impl Route for Dressed {
    fn key(&self) -> u32 { self.route.id as u32 }
    fn lap(&self) -> f32 { self.route.lap() }
    fn pose(&self, s: f32) -> (f32, f32, f32) { self.route.pose(s) }
    fn height(&self, s: f32) -> f32 { self.route.height(s) }
    fn width(&self, s: f32) -> (f32, f32) { self.route.width(s) }
    fn zone(&self, s: f32) -> (Look, &'static str) { let (b, n) = self.route.zone(s); (look_of(self.route.id, b), n) }
    fn curve(&self, s: f32) -> f32 { self.route.curve(s) }
    fn zones(&self) -> Vec<(f32, f32, Look, &'static str)> { self.route.zones().iter().map(|z| (z.s0, z.s1, look_of(self.route.id, z.biome), z.name)).collect() }
    fn sink(&self, s: f32) -> f32 {
        let s = s.rem_euclid(self.route.lap());
        self.sinks.iter().find(|z| s >= z.0 && s < z.1).map_or(0., |z| z.2)
    }
    fn ground(&self, x: f32, y: f32) -> f32 { self.route.ground(x, y) }
    fn pieces(&self) -> &[Piece] { &self.pieces }
    fn forks(&self) -> &[ForkInfo] { &self.forks }
    fn frame(&self, line: u8, s: f32) -> RoadFrame {
        let f = self.route.frame(line, s);
        RoadFrame { x: f.x, y: f.y, th: f.th, h: f.z, bank: f.bank, wl: f.wl, wr: f.wr }
    }
    fn hazards(&self) -> Vec<Hazard> { hazards(self.route.hazards()) }
    fn strips(&self) -> Vec<Strip> { strips(self.route.strips()) }
    fn gaps(&self) -> Vec<Gap> { self.route.gaps().iter().map(|g| Gap { s0: g.s0, s1: g.s1, ramp: g.ramp }).collect() }
    fn landmarks(&self) -> Vec<f32> { self.route.landmarks() }
}

/// The set pieces the renderer adds of its own to a course, found by the name of the stretch they
/// stand in: the landmarks a pod sees coming from a kilometre off.
fn extras(route: &rules::Course) -> Vec<Piece> {
    let mut out = Vec::new();
    if route.id == rules::CourseId::Emberfall {
        for z in route.zones() {
            let mid = (z.s0 + z.s1) * 0.5;
            // The outside of a bend is the side a monument is seen from; the turn's own curve sets it.
            let outer = if route.curve(mid) > 0. { -1. } else { 1. };
            match z.name {
                "CHAIN BRIDGES" => out.push(Piece { kind: Kind::Stacks, s0: z.s0 + 30., s1: z.s1 - 30., side: 0., seed: 101 }),
                "THE HAMMER TURN" => out.push(Piece { kind: Kind::Anvil, s0: z.s0, s1: z.s1, side: outer, seed: 102 }),
                "ASHFALL ESSES" => out.push(Piece { kind: Kind::Stacks, s0: z.s0 + 20., s1: z.s1 - 20., side: 0., seed: 103 + z.s0 as u32 % 7 }),
                _ => {}
            }
        }
    }
    out
}

/// The world renderer's view of course `id`, dressed once.
pub fn on(id: rules::CourseId) -> &'static dyn Route {
    static ALL: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<rules::CourseId, &'static Dressed>>> = std::sync::OnceLock::new();
    let mut all = ALL.get_or_init(Default::default).lock().unwrap();
    *all.entry(id).or_insert_with(|| {
        let route = rules::course(id);
        let forks = route.forks().iter().map(|f| ForkInfo { s0: f.s0, s1: f.s1, side: f.side, look: look_of(id, f.biome), pieces: pieces(&f.pieces), hazards: hazards(&f.hazards), strips: strips(&f.strips) }).collect();
        let sinks = route.zones().iter().map(|z| (z.s0, z.s1, z.shape.sink)).collect();
        let mut all = pieces(route.pieces());
        all.extend(extras(route));
        Box::leak(Box::new(Dressed { route, pieces: all, forks, sinks }))
    })
}

thread_local! { static SEEN: std::cell::Cell<(rules::CourseId, u32)> = const { std::cell::Cell::new((rules::CourseId::Proving, 0)) }; }

/// Chooses the course and the race clock the free functions (`point`, `pose`,
/// `world`) read, for this thread: the race's view calls it once before it draws.
pub fn select(id: rules::CourseId, ticks: u32) { SEEN.with(|c| c.set((id, ticks))); }

pub fn current() -> &'static dyn Route { on(SEEN.with(|c| c.get().0)) }

/// The race clock the view last selected, in ticks.
pub fn ticks() -> u32 { SEEN.with(|c| c.get().1) }
