//! The Moonwell Road, laid out once: four acts of one road at dusk.
//!
//! 1. The Downhill Gate: a run-up, a downhill, a pad, the first snuffer, a
//!    trench with a hay bale at its foot, the first lantern post, a rolling
//!    meadow and a hay loft.
//! 2. The Wheel Arch: the loop, the half-pipe, rolling crests, the corkscrew
//!    coil, a rhythm of small valleys, then the fork: riders at a gallop take the
//!    high rails, a rider who hops over its mouth the valley road, and the two
//!    meet again.
//! 3. The Reed Fen: a valley of reeds and brambles with snuffers drifting
//!    over it in a line to be chained, a bridge of sparks on the far bank,
//!    reed hills and a second, shorter marsh.
//! 4. The Bell Gate: a last downhill, a second loop, the warden, the bell.
//!
//! Heights are written as offsets from `G`; a larger number is lower.
use super::math::Q;
use super::terrain::{Builder, Kind, Link, Path, Portal, Terrain, attach_ring, ring};
use super::world::*;
use std::sync::OnceLock;

/// The road's base height, pixels.
pub const G: i32 = 600;
/// How far behind a lit post the Dimming is held, pixels, for one knight.
pub const HOLD: i32 = 520;
/// The same for the last lantern, the one before the warden's gate.
pub const LAST_HOLD: i32 = 1000;
/// Where each act begins, and where the world ends.
pub const ACTS: [i32; 4] = [0, 4300, 11300, 15700];
pub const END: i32 = 20000;

/// The road, built once.
pub fn level() -> &'static Level {
    static LEVEL: OnceLock<Level> = OnceLock::new();
    LEVEL.get_or_init(build)
}

/// A road being laid left to right, remembering its height.
struct Road {
    b: Builder,
    y: i32,
}

impl Road {
    fn new() -> Self {
        Road { b: Builder::new(Kind::Road, 0, G), y: 0 }
    }
    /// Level ground to column `x`.
    fn flat(&mut self, x: i32) -> &mut Self {
        self.b.line(x, G + self.y);
        self
    }
    /// An S-curve to column `x`, `dy` pixels lower (negative: higher).
    fn slope(&mut self, x: i32, dy: i32) -> &mut Self {
        self.y += dy;
        self.b.ease(x, G + self.y);
        self
    }
}

fn build() -> Level {
    let [a1, a2, a3, a4] = ACTS;
    let mut r = Road::new();

    // ---- Act 1: the Downhill Gate -------------------------------------
    r.flat(a1 + 380).slope(760, 60).flat(1060).slope(1260, -35).slope(1500, 35).flat(1700);
    // The trench: down, a foot of floor, up.
    r.slope(1840, 70).flat(1870).slope(2010, -70).flat(2560);
    // The rolling meadow, then the long downhill into the Wheel Arch.
    r.slope(2760, -30).slope(2960, 30).slope(3160, -30).slope(3360, 30).flat(3560);
    r.slope(a2, 110).flat(a2 + 380);
    let y_loop1 = r.y;

    // ---- Act 2: the Wheel Arch ----------------------------------------
    // Loop one stands on the flat at a2 + 400; the road runs under it.
    let loop1 = a2 + 400;
    r.flat(a2 + 900).slope(a2 + 1040, -30);
    // The half-pipe: a drop, a floor, a climb.
    let pipe = a2 + 1120;
    r.flat(pipe).slope(pipe + 80, 66).flat(pipe + 240).slope(pipe + 320, -66).flat(pipe + 680);
    // Four rolling crests.
    let crests = pipe + 680;
    r.slope(crests + 220, -50).slope(crests + 440, 50).slope(crests + 660, -50).slope(crests + 880, 50).flat(crests + 1020);
    let y_coil = r.y;
    // The corkscrew coil stands at coil_x and leaves 130 pixels on.
    let coil_x = crests + 1020;
    r.flat(coil_x + 700);
    // Three small valleys in a row: a rhythm to ride.
    let rhythm = coil_x + 700;
    for k in 0..3 {
        let x = rhythm + k * 300;
        r.slope(x + 120, 40).slope(x + 240, -40).flat(x + 300);
    }
    let fork_x = rhythm + 900;
    let y_fork = r.y;
    r.flat(fork_x);
    // The valley road under the fork.
    r.slope(fork_x + 300, 70).flat(fork_x + 600).slope(fork_x + 1040, -70).flat(a3 - 700);

    // ---- Act 3: the Reed Fen ------------------------------------------
    r.flat(a3).slope(a3 + 240, 50).slope(a3 + 420, 90).flat(a3 + 1300).slope(a3 + 1580, -90).slope(a3 + 1720, -50).flat(a3 + 2300);
    // Reed hills.
    let reeds = a3 + 2300;
    r.slope(reeds + 220, -14).slope(reeds + 440, 14).slope(reeds + 660, -14).slope(reeds + 880, 14).flat(reeds + 1400);
    // The second, shorter marsh.
    let marsh = reeds + 1400;
    r.slope(marsh + 160, 70).flat(marsh + 480).slope(marsh + 700, -70).flat(a4);

    // ---- Act 4: the Bell Gate -----------------------------------------
    r.slope(a4 + 600, 60).flat(a4 + 1060);
    let y_loop2 = r.y;
    let loop2 = a4 + 1060;
    r.flat(a4 + 1500).slope(a4 + 1800, -40).flat(END - 40);
    let main_y = |paths: &[Path], x: i32| paths[0].y_at_x(x) / Q;

    let mut paths = vec![r.b.build()];
    let main = 0;
    // The loops and the coil, hung on the road.
    attach_ring(&mut paths, main, ring(loop1, G + y_loop1, 40, 16));
    attach_ring(&mut paths, main, ring(coil_x, G + y_coil, 26, 130));
    attach_ring(&mut paths, main, ring(loop2, G + y_loop2, 40, 16));

    // The high rails over the valley: level with the road at the fork, then
    // lifting away from it. A rider fast enough at the fork is carried on.
    let top = G + y_fork - 78;
    let rail_a = paths.len();
    {
        let mut b = Builder::new(Kind::Rail, fork_x, G + y_fork);
        b.line(fork_x + 100, G + y_fork).ease(fork_x + 380, top).line(fork_x + 560, top);
        paths.push(b.build());
    }
    // Planks with gaps between them, to jump.
    let rail_b = paths.len();
    {
        let mut b = Builder::new(Kind::Rail, fork_x + 620, top);
        b.line(fork_x + 860, top);
        paths.push(b.build());
    }
    let rail_c = paths.len();
    {
        let mut b = Builder::new(Kind::Rail, fork_x + 940, top);
        b.line(fork_x + 1000, top).ease(fork_x + 1180, G + y_fork);
        paths.push(b.build());
    }
    // The hay loft of the meadow: reached by the bale, left by falling.
    let loft_top = G + 60 - 92;
    {
        let mut b = Builder::new(Kind::Rail, 3540, loft_top);
        b.line(3860, loft_top);
        paths.push(b.build());
    }
    // The fork's mouth: a rider at speed rides on to the rail.
    let s_fork = paths[main].s_at_x(fork_x);
    paths[main].portals.push(Portal { s: s_fork, dir: 1, min_speed: 1200, to: Link { path: rail_a, s: 0 } });
    // The rail's far end drops back onto the road.
    let s_rejoin = paths[main].s_at_x(fork_x + 1180);
    paths[rail_c].next = Some(Link { path: main, s: s_rejoin });

    let terrain = Terrain::new(paths);
    let t = &terrain;
    let road = |x: i32| main_y(&t.paths, x);

    // ---- Things on the road -------------------------------------------
    let mut sparks: Vec<(i32, i32)> = Vec::new();
    let mut line = |x0: i32, x1: i32, step: i32, lift: i32, y_of: &dyn Fn(i32) -> i32| {
        let mut x = x0;
        while x <= x1 {
            sparks.push((x, y_of(x) - lift));
            x += step;
        }
    };
    let rail_y = |_: i32| top;
    // Act 1.
    line(220, 340, 20, 14, &road);
    line(820, 960, 20, 14, &road);
    line(1160, 1420, 26, 16, &road);
    line(1720, 2000, 28, 40, &|_| G + 20);
    line(2060, 2200, 24, 14, &road);
    line(2600, 3480, 44, 14, &road);
    line(3560, 3840, 28, 14, &|_| loft_top);
    line(3600, 4180, 48, 14, &road);
    // Act 2: the run-up, round the loop, the pipe's curve, the crests, the
    // coil, the rhythm.
    line(a2 + 40, a2 + 340, 26, 14, &road);
    line(loop1 + 90, loop1 + 520, 34, 14, &road);
    line(pipe + 10, pipe + 330, 24, 20, &road);
    line(pipe + 400, pipe + 660, 40, 14, &road);
    line(pipe + 700, pipe + 1000, 44, 14, &road);
    // An arc of sparks along the flight the lip's bale throws: forward and up.
    line(pipe + 470, pipe + 680, 20, 0, &|x| {
        let dx = x - (pipe + 420);
        road(x) - (78 * dx * (266 - dx).max(0) / (133 * 133)).clamp(10, 90)
    });
    line(crests + 1040, coil_x - 20, 40, 14, &road);
    line(coil_x + 150, rhythm - 20, 44, 14, &road);
    line(rhythm, fork_x - 60, 38, 14, &road);
    // The valley road, and the high rails.
    line(fork_x + 40, fork_x + 1040, 44, 14, &road);
    line(fork_x + 400, fork_x + 540, 28, 14, &rail_y);
    line(fork_x + 620, fork_x + 860, 30, 14, &rail_y);
    line(fork_x + 940, fork_x + 1100, 30, 14, &rail_y);
    line(fork_x + 1200, a3 - 40, 48, 14, &road);
    // Act 3: the fen, the bridge of sparks, the reeds and the marsh.
    line(a3 + 60, a3 + 230, 30, 14, &road);
    line(a3 + 440, a3 + 1280, 56, 14, &road);
    line(a3 + 1340, a3 + 1700, 28, 40, &|x| road(x) - (((x - (a3 + 1340)) * (a3 + 1700 - x)) / 560).min(90));
    line(a3 + 1760, a3 + 2260, 46, 14, &road);
    line(reeds + 20, reeds + 1380, 48, 14, &road);
    line(marsh + 20, marsh + 140, 30, 14, &road);
    line(marsh + 180, marsh + 460, 56, 14, &road);
    line(marsh + 720, a4 - 20, 44, 14, &road);
    // Act 4.
    line(a4 + 40, a4 + 560, 40, 14, &road);
    line(a4 + 640, a4 + 1030, 36, 14, &road);
    line(loop2 + 90, loop2 + 400, 34, 14, &road);
    line(a4 + 1560, a4 + 2000, 44, 14, &road);
    line(a4 + 2100, a4 + 2700, 46, 14, &road);
    line(a4 + 3000, a4 + 3300, 28, 14, &road);
    // The spark lines over the chains of floaters: the fen's, and the marsh's.
    for k in 0..9 {
        sparks.push((a3 + 340 + k * 100, road(a3 + 600) - 100 - (k % 3) * 8));
    }
    for k in 0..6 {
        sparks.push((marsh + 90 + k * 110, road(marsh + 300) - 112 - (k % 2) * 30));
    }
    sparks.sort();
    sparks.dedup();

    let springs = vec![
        Spring { x: 1855, y: road(1855), kind: SpringKind::Up },
        Spring { x: 3480, y: road(3480), kind: SpringKind::Up },
        Spring { x: pipe + 420, y: road(pipe + 420), kind: SpringKind::Diag },
        Spring { x: a3 + 700, y: road(a3 + 700), kind: SpringKind::Up },
        Spring { x: marsh + 330, y: road(marsh + 330), kind: SpringKind::Up },
    ];
    let pads = vec![
        Pad { x: 880, y: road(880) },
        Pad { x: fork_x + 450, y: top },
        Pad { x: fork_x + 700, y: top },
        Pad { x: a4 + 460, y: road(a4 + 460) },
    ];
    let post_at = |x: i32, hold: i32| {
        let s = t.paths[main].s_at_x(x);
        Post { x, y: road(x), path: main, s, hold }
    };
    // The last lantern, before the warden, holds the Gloam furthest back.
    let posts = vec![
        post_at(2300, HOLD),
        post_at(a2 + 960, HOLD),
        post_at(a3 - 900, HOLD),
        post_at(a3 + 2340, HOLD),
        post_at(a4 + 1560, HOLD),
        post_at(a4 + 2800, LAST_HOLD),
    ];
    let crates = vec![
        Crate { x: 1560, y: road(1560), kind: CrateKind::Shield },
        Crate { x: 3700, y: loft_top, kind: CrateKind::Sparks },
        Crate { x: crests + 140, y: road(crests + 140), kind: CrateKind::Shield },
        Crate { x: fork_x + 500, y: top, kind: CrateKind::Spur },
        Crate { x: a3 + 2200, y: road(a3 + 2200), kind: CrateKind::Shield },
        Crate { x: a4 + 300, y: road(a4 + 300), kind: CrateKind::Spur },
        Crate { x: a4 + 2400, y: road(a4 + 2400), kind: CrateKind::Shield },
    ];

    let spot = |kind, x: i32, range, min_seats, phase| FoeSpot {
        kind,
        x,
        y: road(x),
        path: main,
        range,
        min_seats,
        phase,
    };
    use FoeKind::*;
    let mut foes = vec![
        // Act 1.
        spot(Bramble, 640, 0, 0, 0),
        spot(Snuffer, 1000, 60, 0, 0),
        spot(Bramble, 1420, 0, 0, 0),
        spot(Snuffer, 2120, 80, 0, 40),
        spot(Snuffer, 2660, 70, 0, 10),
        spot(Snuffer, 3000, 70, 2, 30),
        spot(Bramble, 3100, 0, 3, 0),
        spot(Snuffer, 3920, 60, 4, 0),
        // Act 2.
        spot(Snuffer, a2 + 700, 90, 0, 20),
        spot(Snuffer, a2 + 800, 60, 3, 30),
        spot(Bramble, crests - 100, 0, 0, 0),
        spot(Snuffer, crests + 300, 70, 0, 0),
        spot(Snuffer, crests + 760, 60, 4, 50),
        spot(Bramble, crests + 900, 0, 0, 0),
        spot(Snuffer, coil_x + 330, 90, 0, 0),
        spot(Bramble, coil_x + 430, 0, 5, 0),
        spot(Snuffer, rhythm + 60, 60, 0, 0),
        spot(Snuffer, rhythm + 480, 60, 2, 25),
        spot(Bramble, rhythm + 760, 0, 0, 0),
        // The valley road (the slow, safe way) has company.
        spot(Snuffer, fork_x + 450, 50, 0, 0),
        spot(Bramble, fork_x + 560, 0, 0, 0),
        spot(Snuffer, fork_x + 720, 60, 2, 20),
        spot(Snuffer, fork_x + 1240, 80, 0, 0),
        spot(Snuffer, a3 - 1000, 90, 2, 0),
        spot(Snuffer, a3 - 500, 80, 0, 0),
        // The rails carry a bramble or two.
        FoeSpot { kind: Bramble, x: fork_x + 530, y: top, path: rail_a, range: 0, min_seats: 0, phase: 0 },
        FoeSpot { kind: Bramble, x: fork_x + 780, y: top, path: rail_b, range: 0, min_seats: 3, phase: 0 },
        FoeSpot { kind: Snuffer, x: fork_x + 1020, y: top, path: rail_c, range: 40, min_seats: 0, phase: 0 },
        // Act 3: the fen's low road.
        spot(Bramble, a3 + 600, 0, 0, 0),
        spot(Snuffer, a3 + 740, 90, 0, 0),
        spot(Bramble, a3 + 1000, 0, 0, 0),
        spot(Snuffer, a3 + 1140, 70, 2, 35),
        spot(Bramble, a3 + 1280, 0, 4, 0),
        spot(Snuffer, reeds - 200, 80, 0, 0),
        spot(Snuffer, reeds + 300, 80, 0, 0),
        spot(Bramble, reeds + 500, 0, 0, 0),
        spot(Snuffer, reeds + 700, 80, 3, 0),
        spot(Snuffer, reeds + 1000, 90, 0, 0),
        spot(Bramble, reeds + 1160, 0, 5, 0),
        spot(Snuffer, marsh + 250, 60, 0, 0),
        spot(Bramble, marsh + 150, 0, 0, 0),
        spot(Bramble, marsh + 450, 0, 3, 0),
        // Act 4.
        spot(Snuffer, a4 + 100, 80, 0, 0),
        spot(Snuffer, a4 + 800, 90, 0, 0),
        spot(Snuffer, loop2 + 420, 80, 0, 0),
        spot(Snuffer, loop2 + 480, 40, 4, 20),
        spot(Bramble, a4 + 1760, 0, 0, 0),
        spot(Snuffer, a4 + 2100, 70, 0, 0),
        spot(Snuffer, a4 + 2400, 70, 3, 0),
        spot(Snuffer, a4 + 2900, 60, 2, 0),
        spot(Warden, a4 + 3350, 0, 0, 0),
    ];
    for k in 0..9i32 {
        foes.push(FoeSpot {
            kind: Floater,
            x: a3 + 340 + k * 100,
            y: road(a3 + 600) - 96 - (k % 3) * 8,
            path: main,
            range: 6,
            min_seats: 0,
            phase: (k * 13) as u32,
        });
    }
    // More floaters for a fuller table: a second, higher line over the fen.
    for k in 0..4i32 {
        foes.push(FoeSpot {
            kind: Floater,
            x: a3 + 490 + k * 220,
            y: road(a3 + 600) - 160,
            path: main,
            range: 10,
            min_seats: 2 + k as u32,
            phase: (k * 29) as u32,
        });
    }
    // The marsh's chain, staggered over the short water.
    for k in 0..6i32 {
        foes.push(FoeSpot {
            kind: Floater,
            x: marsh + 90 + k * 110,
            y: road(marsh + 300) - 112 - (k % 2) * 30,
            path: main,
            range: 8,
            min_seats: if k < 4 { 0 } else { k as u32 - 2 },
            phase: (k * 17) as u32,
        });
    }
    foes.sort_by_key(|f| (f.x, f.min_seats));

    let signs = vec![
        Sign { x: 150, y: road(150), words: "A D RUN" },
        Sign { x: 560, y: road(560), words: "E JUMP" },
        Sign { x: 760, y: road(760), words: "S ROLL" },
        Sign { x: 1700, y: road(1700), words: "SPACE SPUR" },
        Sign { x: 3400, y: road(3400), words: "HAY LOFT" },
        Sign { x: a2 + 200, y: road(a2 + 200), words: "SPEED HOLDS" },
        Sign { x: pipe - 60, y: road(pipe - 60), words: "S E CHARGE" },
        Sign { x: fork_x - 230, y: road(fork_x - 230), words: "FAST UP" },
        Sign { x: fork_x - 60, y: road(fork_x - 60), words: "SLOW DOWN" },
        Sign { x: a3 + 160, y: road(a3 + 160), words: "E E E CHAIN" },
        Sign { x: fork_x + 440, y: top, words: "E HOP" },
        Sign { x: a4 + 3100, y: road(a4 + 3100), words: "ROLL OR LEAP" },
    ];

    let start = (main, terrain.paths[main].s_at_x(96));
    Level {
        width: END,
        dead_y: G + 420,
        acts: ACTS,
        main,
        start,
        sparks,
        springs,
        pads,
        posts,
        crates,
        foes,
        signs,
        bell: (a4 + 3900, road(a4 + 3900)),
        par: 170 * 30,
        terrain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_road_is_built_and_every_thing_stands_on_it() {
        let l = level();
        assert!(l.terrain.paths.len() >= 8);
        assert!(l.sparks.len() > 250 && l.sparks.len() <= 64 * super::super::rider::SPARK_WORDS);
        assert!(l.sparks.windows(2).all(|w| w[0].0 <= w[1].0), "sorted by x");
        for s in &l.springs {
            assert!((s.y - l.terrain.paths[l.main].y_at_x(s.x) / Q).abs() <= 1);
        }
        for p in &l.posts {
            assert!((p.y - l.terrain.paths[p.path].sample(p.s).y / Q).abs() <= 1);
        }
        assert!(l.foes.windows(2).all(|w| w[0].x <= w[1].x));
        assert!(l.foes.iter().filter(|f| f.kind == FoeKind::Warden).count() == 1);
        assert!(l.crates.len() <= 32 && l.posts.len() <= 16);
        assert!(l.posts.windows(2).all(|w| w[0].x < w[1].x), "posts in road order");
        assert!(l.bell.0 < END && l.signs.iter().all(|s| s.x > 0 && s.x < END));
    }
}
