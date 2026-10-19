//! The monolith's building language (docs/REALM-MONOLITH.md, the proof in
//! `angel_glass::monolith`) translated to set shapes and the kingdom's
//! pixels: stepped plinths and batters, full-height fins, deep slots,
//! string courses and heavy cornices, slits high in a face (a few lit),
//! spires massed into woods, slabs for rock. A [`Mono`] builds in a piece's
//! own frame (its front toward `+z`, its middle at the origin) and turns it
//! to face its door, as the kit does (`kit::Face`).

use std::fmt::Write as _;

use angel_stage::set::Kind;

use super::blockout::num;
use super::kit::Face;

/// One shape of a set as the realm writes it: a built-in mesh by name
/// (`box`, `cone-6`, `wedge`, ...), its kind, the middle of its foot, its
/// size along x, y and z before the turn, and the turn.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub shape: &'static str,
    pub kind: Kind,
    pub at: [f32; 3],
    pub size: [f32; 3],
    pub turn: f32,
}

impl Piece {
    pub fn new(shape: &'static str, kind: Kind, at: [f32; 3], size: [f32; 3]) -> Self {
        Self { shape, kind, at, size, turn: 0.0 }
    }

    /// The piece as a set file's line.
    pub fn line(&self) -> String {
        let mut l = format!(
            "{} {} at {} {} {} size {} {} {}",
            self.shape,
            self.kind.name(),
            num(self.at[0]),
            num(self.at[1]),
            num(self.at[2]),
            num(self.size[0].max(0.0)),
            num(self.size[1].max(0.0)),
            num(self.size[2].max(0.0)),
        );
        let turn = self.turn.rem_euclid(360.0);
        if turn.abs() > 1e-3 && (turn - 360.0).abs() > 1e-3 {
            let _ = write!(l, " turn {}", num(turn));
        }
        l
    }

    /// Its footprint `[x0, z0, x1, z1]` (a turned one's square round it).
    pub fn footprint(&self) -> [f32; 4] {
        let quarter = (self.turn.rem_euclid(90.0)).abs() < 1e-3 || (self.turn.rem_euclid(90.0) - 90.0).abs() < 1e-3;
        let (hx, hz) = if quarter {
            if (self.turn.rem_euclid(180.0) - 90.0).abs() < 1.0 { (self.size[2] / 2.0, self.size[0] / 2.0) } else { (self.size[0] / 2.0, self.size[2] / 2.0) }
        } else {
            let (s, c) = self.turn.to_radians().sin_cos();
            let (a, b) = (self.size[0] / 2.0, self.size[2] / 2.0);
            ((a * c).abs() + (b * s).abs(), (a * s).abs() + (b * c).abs())
        };
        [self.at[0] - hx, self.at[2] - hz, self.at[0] + hx, self.at[2] + hz]
    }

    pub fn top(&self) -> f32 {
        self.at[1] + self.size[1]
    }
}

/// How far a shape's own frame is turned to face `face` (degrees about the
/// vertical, counter-clockwise from above: a quarter turn takes `+x` to `-z`
/// and the front, `+z`, to `+x`).
pub fn face_turn(face: Face) -> f32 {
    match face {
        Face::South => 0.0,
        Face::East => 90.0,
        Face::North => 180.0,
        Face::West => 270.0,
    }
}

/// A piece built in its own frame: its front toward `+z`, its middle at
/// the origin, then turned and moved onto its footprint.
pub struct Mono {
    pub pieces: Vec<Piece>,
    centre: [f32; 2],
    turn: f32,
    /// Its width across its front and its depth, in its own frame.
    pub w: f32,
    pub d: f32,
    pub seed: u32,
}

impl Mono {
    /// A piece over the footprint `[x0, z0, x1, z1]` (kingdom pixels)
    /// facing `face`.
    pub fn on(foot: [f32; 4], face: Face, seed: u32) -> Self {
        let [x0, z0, x1, z1] = foot;
        let (fw, fd) = (x1 - x0, z1 - z0);
        let (w, d) = match face {
            Face::South | Face::North => (fw, fd),
            Face::East | Face::West => (fd, fw),
        };
        Self { pieces: Vec::new(), centre: [(x0 + x1) / 2.0, (z0 + z1) / 2.0], turn: face_turn(face), w, d, seed }
    }

    /// The world point of the frame's `(x, z)`.
    pub fn world(&self, x: f32, z: f32) -> [f32; 2] {
        let (s, c) = self.turn.to_radians().sin_cos();
        [self.centre[0] + x * c + z * s, self.centre[1] - x * s + z * c]
    }

    /// A number in 0..1 from the piece's seed and `salt`.
    pub fn unit(&self, salt: u32) -> f32 {
        let mut h = self.seed ^ salt.wrapping_mul(0x9E37_79B9);
        h ^= h >> 15;
        h = h.wrapping_mul(0x85EB_CA6B);
        h ^= h >> 13;
        (h & 0xffff) as f32 / 65535.0
    }

    /// Shape `shape` of `kind`, its foot's middle at the frame's
    /// `(x, y, z)`, `size` in the frame, turned `turn` more.
    pub fn put(&mut self, shape: &'static str, kind: Kind, [x, y, z]: [f32; 3], size: [f32; 3], turn: f32) {
        let [wx, wz] = self.world(x, z);
        self.pieces.push(Piece { shape, kind, at: [wx, y, wz], size, turn: self.turn + turn });
    }

    /// A box over the frame's `x0..x1` by `z0..z1`, `y0` to `y1`.
    pub fn block(&mut self, kind: Kind, [x0, x1]: [f32; 2], [y0, y1]: [f32; 2], [z0, z1]: [f32; 2]) {
        if x1 - x0 <= 0.05 || y1 - y0 <= 0.05 || z1 - z0 <= 0.05 {
            return;
        }
        self.put("box", kind, [(x0 + x1) / 2.0, y0, (z0 + z1) / 2.0], [x1 - x0, y1 - y0, z1 - z0], 0.0);
    }

    /// A stepped plinth under a body over `x` by `z`: each tier `(rise,
    /// out)` stands out `out` past the body on the sides `sides` says
    /// (`[front, east, back, west]`), its top a lit ledge, its face a band
    /// of shadow. Returns the height reached.
    pub fn stepped(&mut self, kind: Kind, x: [f32; 2], z: [f32; 2], y0: f32, tiers: &[(f32, f32)], sides: [bool; 4]) -> f32 {
        let mut y = y0;
        for &(rise, out) in tiers {
            let o = |on: bool| if on { out } else { 0.0 };
            let [f, e, b, w] = sides;
            self.block(kind, [x[0] - o(w), x[1] + o(e)], [y, y + rise], [z[0] - o(b), z[1] + o(f)]);
            y += rise;
        }
        y
    }

    /// Fins standing `out` proud of the face at `z` (the front, facing
    /// `+z`), `every` pixels apart between `x0` and `x1`, from `y0` to `y1`.
    pub fn fins(&mut self, kind: Kind, [x0, x1]: [f32; 2], z: f32, [y0, y1]: [f32; 2], every: f32, out: f32) {
        let count = ((x1 - x0) / every).floor() as i32;
        if count < 1 {
            return;
        }
        let start = x0 + ((x1 - x0) - count as f32 * every) / 2.0;
        for k in 1..count {
            let fx = start + k as f32 * every;
            self.block(kind, [fx - 1.0, fx + 1.0], [y0, y1], [z, z + out]);
        }
    }

    /// Slits in the bays of a face at `z` (the front), one a bay `every`
    /// apart between `x0` and `x1`, from `y` up `tall`; about `lit` of them
    /// lit (a window: an accent), the rest dark.
    pub fn slits(&mut self, [x0, x1]: [f32; 2], z: f32, y: f32, tall: f32, every: f32, lit: f32) {
        let count = ((x1 - x0) / every).floor() as i32;
        let start = x0 + ((x1 - x0) - count as f32 * every) / 2.0;
        for k in 0..count {
            let x = start + (k as f32 + 0.5) * every;
            let kind = if self.unit(300 + k as u32) < lit { Kind::Window } else { Kind::Door };
            self.block(kind, [x - 0.8, x + 0.8], [y, y + tall], [z - 0.2, z + 0.25]);
        }
    }

    /// A door as a deep slot in the front face at `z`: dark, `w` wide and
    /// `h` high from `y`, between jambs standing `deep` proud of the face,
    /// a lintel over them.
    pub fn door(&mut self, kind: Kind, x: f32, z: f32, y: f32, [w, h]: [f32; 2], deep: f32) {
        self.block(Kind::Door, [x - w / 2.0, x + w / 2.0], [y - 3.0, y + h], [z - 0.4, z + 0.3]);
        for side in [-1.0f32, 1.0] {
            let jx = x + side * (w / 2.0 + 1.5);
            self.block(kind, [jx - 1.5, jx + 1.5], [y - 3.0, y + h + 3.0], [z, z + deep]);
        }
        self.block(kind, [x - w / 2.0 - 3.0, x + w / 2.0 + 3.0], [y + h, y + h + 4.0], [z, z + deep]);
    }

    /// A heavy cornice round the top of a body over `x` by `z`, standing
    /// `out` past it.
    pub fn cornice(&mut self, kind: Kind, x: [f32; 2], z: [f32; 2], top: f32, out: f32) {
        self.block(kind, [x[0] - out, x[1] + out], [top - 4.0, top + 1.0], [z[0] - out, z[1] + out]);
    }

    /// A string course: a shallow band round a body at `y`, a line of shadow
    /// under it.
    pub fn course(&mut self, kind: Kind, x: [f32; 2], z: [f32; 2], y: f32) {
        self.block(kind, [x[0] - 0.8, x[1] + 0.8], [y - 1.2, y], [z[0] - 0.8, z[1] + 0.8]);
    }

    /// A brazier: a low plinth with a fire on it, at the frame's `(x, z)` on
    /// ground `y`. Returns where its light is (the frame's x, y, z).
    pub fn brazier(&mut self, x: f32, z: f32, y: f32) -> [f32; 3] {
        self.block(Kind::Stone, [x - 2.5, x + 2.5], [y - 2.0, y + 5.0], [z - 2.5, z + 2.5]);
        self.block(Kind::Metal, [x - 2.0, x + 2.0], [y + 5.0, y + 6.0], [z - 2.0, z + 2.0]);
        self.block(Kind::Fire, [x - 1.4, x + 1.4], [y + 6.0, y + 8.0], [z - 1.4, z + 1.4]);
        [x, y + 9.0, z]
    }

    /// The frame's point `(x, y, z)` in the world.
    pub fn point(&self, [x, y, z]: [f32; 3]) -> [f32; 3] {
        let [wx, wz] = self.world(x, z);
        [wx, y, wz]
    }
}

/// A spire (a cypress, a pine): a tall six-sided cone, `r` across at its
/// foot on ground `y`, `h` high.
pub fn spire(x: f32, y: f32, z: f32, r: f32, h: f32, turn: f32) -> Piece {
    Piece { turn, ..Piece::new("cone-6", Kind::Foliage, [x, y, z], [2.0 * r, h, 2.0 * r]) }
}

/// A slab of rock, `w` by `d` and `h` high from `y`, turned `turn`.
pub fn slab(kind: Kind, x: f32, y: f32, z: f32, [w, h, d]: [f32; 3], turn: f32) -> Piece {
    Piece { turn, ..Piece::new("box", kind, [x, y, z], [w, h, d]) }
}
