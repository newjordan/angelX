//! The Works: castles raised stone by stone. The architect (Blocks, offline)
//! writes a castle as `angel-castle/v1` JSON: every stone in the order masons
//! would set it, the falsework that comes and goes, the earth to move and the
//! stages of the build. [`Castle::from_json`] bakes that into a compact
//! binary asset (`assets/works/<plan>.castle`, about 150 KB for the great
//! tower, `examples/bake_castle.rs`), [`Castle::decode`] reads it back, and
//! [`castle`] is the one loaded copy of each, made once.
//!
//! Everything visible and solid about a castle is a pure function of its
//! plan and how many stones stand (`laid`, `rules::works::Site`): stone `i`
//! stands once `laid > i`. The sim only ever advances `laid`.
//!
//! **Scale and place.** One tile is 1.5 m and 16 kingdom pixels, so a metre
//! is [`PX_PER_M`] pixels. A castle's own frame has `x` east, `y` north,
//! `z` up, its middle at the origin on the ground and its gate facing `-y`;
//! the kingdom's is `x` east, `z` south (down the screen), `y` up. A castle
//! stands with its middle on tile corner [`ORIGIN`], so its tiles line up
//! with the map's: tile `(ox + i, oy + j)` covers the castle's `x` from
//! `1.5 i` to `1.5 (i + 1)` and its `y` (north) from `-1.5 (j + 1)` to
//! `-1.5 j`.
//!
//! **Solid.** A tile is blocked as the wall there reaches knee height, or
//! the ditch is dug there. That is baked per tile as the `laid` at which it
//! becomes so ([`Castle::need`]): a tile is blocked once `laid >= need`.
//! The gate passage and the causeway before it are never blocked.

use std::sync::OnceLock;

/// Kingdom pixels in a metre: a tile is 1.5 m and 16 px.
pub const PX_PER_M: f32 = 16.0 / 1.5;

/// The tile corner every castle's middle stands on (the Keep's site).
pub const ORIGIN: (i32, i32) = (133, 81);

/// The tiles a castle's solid grid spans on a side, centred on [`ORIGIN`].
pub const GRID: i32 = 28;

/// Half of [`GRID`].
pub const HALF: i32 = GRID / 2;

/// A tile that is never blocked.
pub const NEVER: u32 = u32::MAX;

/// Set in a tile's baked `need` when it is the ditch that blocks it (a pit:
/// stops feet, not shots) and not a wall.
pub const DITCH: u32 = 1 << 31;

/// How high a wall must stand to stop a knight: a knee, in metres.
pub const KNEE: f32 = 0.5;

/// What the baked file starts with.
const MAGIC: &[u8; 4] = b"ACB1";

/// Sample points to a tile's side, and how many of a tile's samples a wall
/// (an eighth of it: a shell of 0.6 m split across two tiles still shuts
/// both) or a ditch (half of it) must cover before the tile is blocked.
const PER_SIDE: usize = 8;
const WALL_SAMPLES: usize = PER_SIDE * PER_SIDE / 8;
const DITCH_SAMPLES: usize = PER_SIDE * PER_SIDE / 2;

/// Roles that never block a walker: the deck over the ditch, its chains and
/// the portcullis (the gate stays open).
const WALKABLE_ROLES: [&str; 3] = ["drawbridge", "chain", "portcullis"];

/// One stone: its material and role (indexes into the castle's tables) and
/// its shape, in centimetres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stone {
    pub mat: u8,
    pub role: u8,
    pub shape: Shape,
}

/// An axis-aligned box, or eight corners (a wedge, a voussoir, a battered
/// face) in the architect's order: corners 0 to 3 are one face's ring and 4
/// to 7 the opposite face's, in the same order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// `[x0, y0, z0, x1, y1, z1]`.
    Box([i16; 6]),
    Hex([[i16; 3]; 8]),
}

impl Stone {
    /// The box round the stone, `[x0, y0, z0, x1, y1, z1]`, in centimetres.
    pub fn aabb(&self) -> [i16; 6] {
        match self.shape {
            Shape::Box(b) => b,
            Shape::Hex(c) => {
                let mut out = [i16::MAX, i16::MAX, i16::MAX, i16::MIN, i16::MIN, i16::MIN];
                for p in c {
                    for k in 0..3 {
                        out[k] = out[k].min(p[k]);
                        out[k + 3] = out[k + 3].max(p[k]);
                    }
                }
                out
            }
        }
    }

    /// The eight corners in metres: a box's in the architect's ring order.
    pub fn corners(&self) -> [[f32; 3]; 8] {
        let m = |v: i16| f32::from(v) / 100.0;
        match self.shape {
            Shape::Box([x0, y0, z0, x1, y1, z1]) => {
                let (x0, y0, z0, x1, y1, z1) = (m(x0), m(y0), m(z0), m(x1), m(y1), m(z1));
                [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]
            }
            Shape::Hex(c) => c.map(|p| [m(p[0]), m(p[1]), m(p[2])]),
        }
    }
}

/// A stage of the build (a lift of the keep, the curtain, the roofs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stage {
    pub id: String,
    pub name: String,
    pub first: u32,
    pub count: u32,
}

/// What the falsework is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    /// Putlog scaffolding climbing the walls.
    Scaffold,
    /// A treadwheel crane's frame, one a lift.
    Crane,
    /// Timber centring under an arch.
    Centring,
    Other,
}

/// A piece of falsework: there from stone `appears` (it stands once `laid >=
/// appears`) until stone `gone` is laid (`laid < gone`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Falsework {
    pub kind: Work,
    pub appears: u32,
    pub gone: u32,
    pub mat: u8,
    /// `[x0, y0, z0, x1, y1, z1]`, centimetres.
    pub bx: [i16; 6],
}

/// Earth moved: a ring of ground dug down (or raised) by `depth` between
/// `ring` and `hole`, as soon as stone `at` is laid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Earth {
    pub at: u32,
    pub raise: bool,
    /// Centimetres, castle frame `(x, y)`.
    pub ring: Vec<[i16; 2]>,
    pub hole: Vec<[i16; 2]>,
    pub depth: i16,
}

/// A castle, baked.
#[derive(Clone, Debug, PartialEq)]
pub struct Castle {
    /// Its plan: `great-tower`.
    pub id: String,
    pub seed: u32,
    pub materials: Vec<String>,
    pub roles: Vec<String>,
    pub stages: Vec<Stage>,
    pub stones: Vec<Stone>,
    pub falsework: Vec<Falsework>,
    pub earth: Vec<Earth>,
    /// Per tile of the [`GRID`] by [`GRID`] about [`ORIGIN`], row by row: the
    /// `laid` at which the tile is blocked ([`NEVER`]: it never is).
    pub solid: Vec<u32>,
    /// The box the stones fill, metres.
    pub bounds: [f32; 6],
}

impl Castle {
    /// How many stones the castle is.
    pub fn len(&self) -> u32 {
        self.stones.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.stones.is_empty()
    }

    /// The name of material `m` and of role `r`.
    pub fn material(&self, m: u8) -> &str {
        self.materials.get(usize::from(m)).map_or("", String::as_str)
    }

    pub fn role(&self, r: u8) -> &str {
        self.roles.get(usize::from(r)).map_or("", String::as_str)
    }

    /// The stage stone `index` belongs to (the last stage that begins at or
    /// before it).
    pub fn stage_of(&self, index: u32) -> usize {
        self.stages
            .iter()
            .rposition(|s| s.count > 0 && s.first <= index)
            .unwrap_or(0)
    }

    /// The stage the next stone to be laid (`laid`) is in; the last stage
    /// once the castle stands.
    pub fn stage_at(&self, laid: u32) -> usize {
        self.stage_of(laid.min(self.len().saturating_sub(1)))
    }

    /// The tiles the castle's solid grid spans on the map: `(x, y, w, h)`.
    pub fn tiles(&self) -> (i32, i32, i32, i32) {
        (ORIGIN.0 - HALF, ORIGIN.1 - HALF, GRID, GRID)
    }

    /// A tile's baked word: its `need` and whether the ditch is what blocks
    /// it ([`NEVER`] outside the grid).
    fn word(&self, tx: i32, ty: i32) -> u32 {
        let (i, j) = (tx - ORIGIN.0 + HALF, ty - ORIGIN.1 + HALF);
        if i < 0 || j < 0 || i >= GRID || j >= GRID {
            return NEVER;
        }
        self.solid.get((j * GRID + i) as usize).copied().unwrap_or(NEVER)
    }

    /// The `laid` at which map tile `(tx, ty)` is blocked: [`NEVER`] if it
    /// never is.
    pub fn need(&self, tx: i32, ty: i32) -> u32 {
        match self.word(tx, ty) {
            NEVER => NEVER,
            w => w & !DITCH,
        }
    }

    /// Whether the ditch (a pit, not a wall) is what blocks map tile `(tx, ty)`.
    pub fn is_ditch(&self, tx: i32, ty: i32) -> bool {
        let w = self.word(tx, ty);
        w != NEVER && w & DITCH != 0
    }

    /// Whether map tile `(tx, ty)` is blocked with `laid` stones standing.
    pub fn blocked(&self, tx: i32, ty: i32, laid: u32) -> bool {
        laid >= self.need(tx, ty)
    }

    /// Whether map tile `(tx, ty)` is blocked with `laid` stones standing and the stones `lost`
    /// (sorted) knocked out: a wall whose stone that closed the tile is gone is a breach.
    pub fn blocked_without(&self, tx: i32, ty: i32, laid: u32, lost: &[u32]) -> bool {
        if !self.blocked(tx, ty, laid) {
            return false;
        }
        self.is_ditch(tx, ty) || lost.is_empty() || self.need(tx, ty).checked_sub(1).is_none_or(|stone| lost.binary_search(&stone).is_err())
    }

    /// The earth lifts done with `laid` stones standing.
    pub fn earth_done(&self, laid: u32) -> usize {
        self.earth.iter().filter(|e| e.at <= laid).count()
    }

    /// Where a point of the castle's frame (metres) stands in the kingdom's
    /// pixels: `[x, y, z]` with `y` up.
    pub fn world(&self, [x, y, z]: [f32; 3]) -> [f32; 3] {
        let (ox, oz) = origin_px();
        [ox + x * PX_PER_M, z * PX_PER_M, oz - y * PX_PER_M]
    }

    // ----------------------------------------------------------- the bake --

    /// The castle `text` (`angel-castle/v1` JSON) as the game keeps it, named
    /// `id`.
    pub fn from_json(id: &str, text: &str) -> Result<Castle, String> {
        use serde_json::Value;
        let v: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
        if v["schema"] != "angel-castle/v1" {
            return Err(format!("schema {} is not angel-castle/v1", v["schema"]));
        }
        let strings = |k: &str| -> Result<Vec<String>, String> {
            v[k].as_array()
                .ok_or(format!("{k} is missing"))?
                .iter()
                .map(|s| s.as_str().map(str::to_string).ok_or(format!("{k} holds a non-name")))
                .collect()
        };
        let cm = |x: &Value| -> Result<i16, String> {
            let f = x.as_f64().ok_or("a number is not a number")?;
            let c = (f * 100.0).round();
            if !(-32000.0..=32000.0).contains(&c) {
                return Err(format!("{f} m is off the map"));
            }
            Ok(c as i16)
        };
        let uint = |x: &Value, what: &str| x.as_u64().map(|n| n as u32).ok_or(format!("{what} is not a count"));
        let materials = strings("materials")?;
        let roles = strings("roles")?;
        let mut stages = Vec::new();
        for s in v["stages"].as_array().ok_or("stages are missing")? {
            stages.push(Stage {
                id: s["id"].as_str().unwrap_or("").to_string(),
                name: s["name"].as_str().unwrap_or("").to_string(),
                first: uint(&s["first"], "a stage's first")?,
                count: uint(&s["count"], "a stage's count")?,
            });
        }
        let mut stones = Vec::new();
        for (n, s) in v["stones"].as_array().ok_or("stones are missing")?.iter().enumerate() {
            let a = s.as_array().ok_or(format!("stone {n} is not a list"))?;
            let (mat, role) = (a[0].as_u64().unwrap_or(0) as u8, a[1].as_u64().unwrap_or(0) as u8);
            let shape = if a.len() == 8 {
                let mut b = [0i16; 6];
                for k in 0..6 {
                    b[k] = cm(&a[2 + k])?;
                }
                // A box is lo to hi whatever order the architect wrote it in.
                for k in 0..3 {
                    if b[k] > b[k + 3] {
                        b.swap(k, k + 3);
                    }
                }
                Shape::Box(b)
            } else {
                let pts = a[2].as_array().filter(|p| p.len() == 8).ok_or(format!("stone {n} has no eight corners"))?;
                let mut c = [[0i16; 3]; 8];
                for (k, p) in pts.iter().enumerate() {
                    for d in 0..3 {
                        c[k][d] = cm(&p[d])?;
                    }
                }
                Shape::Hex(c)
            };
            stones.push(Stone { mat, role, shape });
        }
        let mut falsework = Vec::new();
        for f in v["falsework"].as_array().ok_or("falsework is missing")? {
            let kind = match f["kind"].as_str().unwrap_or("") {
                "scaffold" => Work::Scaffold,
                "crane" => Work::Crane,
                "centring" => Work::Centring,
                _ => Work::Other,
            };
            let b = f["box"].as_array().filter(|b| b.len() == 6).ok_or("a falsework box is not six numbers")?;
            let mut bx = [0i16; 6];
            for k in 0..6 {
                bx[k] = cm(&b[k])?;
            }
            falsework.push(Falsework {
                kind,
                appears: uint(&f["appears"], "appears")?,
                gone: uint(&f["gone"], "gone")?,
                mat: f["material"].as_u64().unwrap_or(0) as u8,
                bx,
            });
        }
        let poly = |p: &Value| -> Result<Vec<[i16; 2]>, String> {
            p.as_array().map_or(Ok(Vec::new()), |a| a.iter().map(|q| Ok([cm(&q[0])?, cm(&q[1])?])).collect())
        };
        let mut earth = Vec::new();
        for e in v["earth"].as_array().ok_or("earth is missing")? {
            earth.push(Earth {
                at: uint(&e["at"], "earth's at")?,
                raise: e["kind"] == "raise",
                ring: poly(&e["ring"])?,
                hole: poly(&e["hole"])?,
                depth: cm(&e["depth"])?,
            });
        }
        let mut bounds = [0.0f32; 6];
        for (k, b) in v["bounds"].as_array().ok_or("bounds are missing")?.iter().take(6).enumerate() {
            // Whole centimetres, as the file keeps them.
            bounds[k] = (b.as_f64().unwrap_or(0.0) * 100.0).round() as f32 / 100.0;
        }
        let mut castle = Castle {
            id: id.to_string(),
            seed: v["seed"].as_u64().unwrap_or(0) as u32,
            materials,
            roles,
            stages,
            stones,
            falsework,
            earth,
            solid: Vec::new(),
            bounds,
        };
        castle.solid = castle.bake_solid();
        Ok(castle)
    }

    /// Each tile's `need`: a wall that reaches a knee, or a dug ditch.
    fn bake_solid(&self) -> Vec<u32> {
        // Samples an eighth of a tile apart over the grid.
        let side = GRID as usize * PER_SIDE;
        let step = 1.5 / PER_SIDE as f32;
        // The sample `(sx, sy)`'s point, castle metres: `sy` counts south from
        // the grid's north edge, as the map's rows do.
        let at = |sx: usize, sy: usize| -> (f32, f32) {
            let x = (sx as f32 + 0.5) * step - HALF as f32 * 1.5;
            let y = -((sy as f32 + 0.5) * step - HALF as f32 * 1.5);
            (x, y)
        };
        let mut first = vec![NEVER; side * side];
        for (index, stone) in self.stones.iter().enumerate() {
            let [x0, y0, z0, x1, y1, z1] = stone.aabb().map(|v| f32::from(v) / 100.0);
            // Stones that stand on the ground and reach a knee; not a deck.
            if z0 >= 0.9 || z1 <= KNEE || WALKABLE_ROLES.contains(&self.role(stone.role)) {
                continue;
            }
            let lo = |v: f32| (((v + HALF as f32 * 1.5) / step).floor() as i32).clamp(0, side as i32 - 1) as usize;
            let (sx0, sx1) = (lo(x0), lo(x1));
            // North is down the rows: y1 is the northern edge.
            let row = |y: f32| (((HALF as f32 * 1.5 - y) / step).floor() as i32).clamp(0, side as i32 - 1) as usize;
            let (sy0, sy1) = (row(y1), row(y0));
            for sy in sy0..=sy1 {
                for sx in sx0..=sx1 {
                    let (x, y) = at(sx, sy);
                    if x >= x0 && x <= x1 && y >= y0 && y <= y1 {
                        let cell = &mut first[sy * side + sx];
                        *cell = (*cell).min(index as u32 + 1);
                    }
                }
            }
        }
        let apron = self.apron();
        let mut solid = vec![NEVER; (GRID * GRID) as usize];
        for j in 0..GRID {
            for i in 0..GRID {
                // The tile's middle, castle metres.
                let (mx, my) = ((i - HALF) as f32 * 1.5 + 0.75, -((j - HALF) as f32 * 1.5 + 0.75));
                let in_apron = apron.is_some_and(|[x0, y0, x1, y1]| mx >= x0 && mx <= x1 && my >= y0 && my <= y1);
                let mut needs: Vec<u32> = Vec::new();
                let mut dug = [0usize; 8];
                for b in 0..PER_SIDE {
                    for a in 0..PER_SIDE {
                        let (sx, sy) = (i as usize * PER_SIDE + a, j as usize * PER_SIDE + b);
                        let n = first[sy * side + sx];
                        if n != NEVER {
                            needs.push(n);
                        }
                        let (x, y) = at(sx, sy);
                        // The causeway before the gate stays undug.
                        if x.abs() < 1.5 && y < 0.0 {
                            continue;
                        }
                        for (k, e) in self.earth.iter().enumerate().take(8) {
                            if !e.raise && inside(&e.ring, x, y) && !inside(&e.hole, x, y) {
                                dug[k] += 1;
                            }
                        }
                    }
                }
                needs.sort_unstable();
                // A wall wins where a wall stands in the ditch's edge.
                let mut word = if in_apron { NEVER } else { needs.get(WALL_SAMPLES - 1).copied().unwrap_or(NEVER) };
                if word == NEVER {
                    for (k, e) in self.earth.iter().enumerate().take(8) {
                        if dug[k] >= DITCH_SAMPLES {
                            word = word.min(e.at | DITCH);
                        }
                    }
                }
                solid[(j * GRID + i) as usize] = word;
            }
        }
        solid
    }

    /// The ground just inside the gate: the row of tiles north of the
    /// gatehouse's inner end, across the passage and a few metres either side.
    /// A footing's edge across the way (the forebuilding's, here) must never
    /// shut a whole tile and with it the only way into the bailey, so the
    /// tiles there are never blocked by stonework. `[x0, y0, x1, y1]`, metres.
    fn apron(&self) -> Option<[f32; 4]> {
        // The gate's own stones: jambs within the passage's reach, south of the bailey.
        let mouth = self
            .stones
            .iter()
            .filter(|s| self.role(s.role) == "jamb")
            .map(|s| s.aabb().map(|v| f32::from(v) / 100.0))
            .filter(|b| (b[0] + b[3]) / 2.0 > -4.0 && (b[0] + b[3]) / 2.0 < 4.0 && b[4] < -10.0 && b[1] > -17.5 && b[2] < 0.9)
            .map(|b| b[4])
            .fold(f32::MIN, f32::max);
        (mouth > f32::MIN).then_some([-6.0, mouth, 6.0, mouth + 1.5])
    }

    // ---------------------------------------------------------- the bytes --

    /// The castle as the baked file: its tables, then every stone as varints
    /// (a lay order is spatially coherent, so each stone is a small step from
    /// the one before).
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.bytes.extend_from_slice(MAGIC);
        w.text(&self.id);
        w.uint(self.seed);
        for table in [&self.materials, &self.roles] {
            w.uint(table.len() as u32);
            for s in table {
                w.text(s);
            }
        }
        w.uint(self.stages.len() as u32);
        for s in &self.stages {
            w.text(&s.id);
            w.text(&s.name);
            w.uint(s.first);
            w.uint(s.count);
        }
        w.uint(self.stones.len() as u32);
        let mut prev = [0i16; 3];
        for s in &self.stones {
            w.bytes.push(s.mat);
            w.bytes.push(s.role);
            match s.shape {
                Shape::Box(b) => {
                    w.bytes.push(0);
                    let at = [b[0], b[1], b[2]];
                    for k in 0..3 {
                        w.int(i32::from(at[k]) - i32::from(prev[k]));
                    }
                    for k in 0..3 {
                        w.uint((i32::from(b[k + 3]) - i32::from(b[k])).max(0) as u32);
                    }
                    prev = at;
                }
                Shape::Hex(c) => {
                    w.bytes.push(1);
                    for k in 0..3 {
                        w.int(i32::from(c[0][k]) - i32::from(prev[k]));
                    }
                    for p in &c[1..] {
                        for k in 0..3 {
                            w.int(i32::from(p[k]) - i32::from(c[0][k]));
                        }
                    }
                    prev = c[0];
                }
            }
        }
        w.uint(self.falsework.len() as u32);
        for f in &self.falsework {
            w.bytes.push(match f.kind {
                Work::Scaffold => 0,
                Work::Crane => 1,
                Work::Centring => 2,
                Work::Other => 3,
            });
            w.uint(f.appears);
            w.uint(f.gone);
            w.bytes.push(f.mat);
            for v in f.bx {
                w.int(i32::from(v));
            }
        }
        w.uint(self.earth.len() as u32);
        for e in &self.earth {
            w.uint(e.at);
            w.bytes.push(u8::from(e.raise));
            w.int(i32::from(e.depth));
            for poly in [&e.ring, &e.hole] {
                w.uint(poly.len() as u32);
                for p in poly {
                    w.int(i32::from(p[0]));
                    w.int(i32::from(p[1]));
                }
            }
        }
        for v in self.bounds {
            w.int((v * 100.0).round() as i32);
        }
        for &n in &self.solid {
            w.uint(if n == NEVER { 0 } else { n + 1 });
        }
        w.bytes
    }

    /// A castle from the baked file.
    pub fn decode(bytes: &[u8]) -> Result<Castle, String> {
        let mut r = Reader { bytes, at: 0 };
        if r.take(4)? != MAGIC {
            return Err("not a baked castle".into());
        }
        let id = r.text()?;
        let seed = r.uint()?;
        let mut tables = [Vec::new(), Vec::new()];
        for table in &mut tables {
            for _ in 0..r.count(4096)? {
                table.push(r.text()?);
            }
        }
        let [materials, roles] = tables;
        let mut stages = Vec::new();
        for _ in 0..r.count(4096)? {
            stages.push(Stage { id: r.text()?, name: r.text()?, first: r.uint()?, count: r.uint()? });
        }
        let n = r.count(1 << 20)?;
        let mut stones = Vec::with_capacity(n);
        let mut prev = [0i16; 3];
        for _ in 0..n {
            let (mat, role, tag) = (r.byte()?, r.byte()?, r.byte()?);
            let shape = if tag == 0 {
                let mut at = [0i16; 3];
                for k in 0..3 {
                    at[k] = (i32::from(prev[k]) + r.int()?) as i16;
                }
                let mut b = [at[0], at[1], at[2], 0, 0, 0];
                for k in 0..3 {
                    b[k + 3] = (i32::from(at[k]) + r.uint()? as i32) as i16;
                }
                prev = at;
                Shape::Box(b)
            } else {
                let mut c = [[0i16; 3]; 8];
                for k in 0..3 {
                    c[0][k] = (i32::from(prev[k]) + r.int()?) as i16;
                }
                for p in 1..8 {
                    for k in 0..3 {
                        c[p][k] = (i32::from(c[0][k]) + r.int()?) as i16;
                    }
                }
                prev = c[0];
                Shape::Hex(c)
            };
            stones.push(Stone { mat, role, shape });
        }
        let mut falsework = Vec::new();
        for _ in 0..r.count(1 << 16)? {
            let kind = match r.byte()? {
                0 => Work::Scaffold,
                1 => Work::Crane,
                2 => Work::Centring,
                _ => Work::Other,
            };
            let (appears, gone, mat) = (r.uint()?, r.uint()?, r.byte()?);
            let mut bx = [0i16; 6];
            for v in &mut bx {
                *v = r.int()? as i16;
            }
            falsework.push(Falsework { kind, appears, gone, mat, bx });
        }
        let mut earth = Vec::new();
        for _ in 0..r.count(256)? {
            let (at, raise, depth) = (r.uint()?, r.byte()? != 0, r.int()? as i16);
            let mut polys = [Vec::new(), Vec::new()];
            for poly in &mut polys {
                for _ in 0..r.count(1 << 12)? {
                    poly.push([r.int()? as i16, r.int()? as i16]);
                }
            }
            let [ring, hole] = polys;
            earth.push(Earth { at, raise, ring, hole, depth });
        }
        let mut bounds = [0.0f32; 6];
        for v in &mut bounds {
            *v = r.int()? as f32 / 100.0;
        }
        let mut solid = Vec::with_capacity((GRID * GRID) as usize);
        for _ in 0..GRID * GRID {
            let n = r.uint()?;
            solid.push(if n == 0 { NEVER } else { n - 1 });
        }
        Ok(Castle { id, seed, materials, roles, stages, stones, falsework, earth, solid, bounds })
    }
}

/// Whether `(x, y)` (metres) is inside the polygon `poly` (centimetres).
fn inside(poly: &[[i16; 2]], x: f32, y: f32) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = (f32::from(poly[i][0]) / 100.0, f32::from(poly[i][1]) / 100.0);
        let (xj, yj) = (f32::from(poly[j][0]) / 100.0, f32::from(poly[j][1]) / 100.0);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Whether point `(x, y)` (metres) is within the dug ground of `earth`.
pub fn dug(e: &Earth, x: f32, y: f32) -> bool {
    inside(&e.ring, x, y) && !inside(&e.hole, x, y)
}

/// The kingdom's pixel under the castle's middle.
pub fn origin_px() -> (f32, f32) {
    (ORIGIN.0 as f32 * 16.0, ORIGIN.1 as f32 * 16.0)
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn uint(&mut self, mut v: u32) {
        while v >= 0x80 {
            self.bytes.push((v & 0x7f) as u8 | 0x80);
            v >>= 7;
        }
        self.bytes.push(v as u8);
    }

    fn int(&mut self, v: i32) {
        self.uint(((v << 1) ^ (v >> 31)) as u32);
    }

    fn text(&mut self, s: &str) {
        self.uint(s.len() as u32);
        self.bytes.extend_from_slice(s.as_bytes());
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len()).ok_or("the baked castle is cut short")?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }

    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn uint(&mut self) -> Result<u32, String> {
        let (mut v, mut shift) = (0u32, 0);
        loop {
            let b = self.byte()?;
            if shift > 28 {
                return Err("a number runs on".into());
            }
            v |= u32::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
        }
    }

    fn int(&mut self) -> Result<i32, String> {
        let v = self.uint()?;
        Ok(((v >> 1) as i32) ^ -((v & 1) as i32))
    }

    /// A count of at most `most` things.
    fn count(&mut self, most: u32) -> Result<usize, String> {
        let n = self.uint()?;
        if n > most { Err(format!("{n} things is too many")) } else { Ok(n as usize) }
    }

    fn text(&mut self) -> Result<String, String> {
        let n = self.count(1 << 12)?;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| "a name is not text".into())
    }
}

// --------------------------------------------------------- the loaded ones --

/// Whether the Works stands on this map: the Keep's site is the blank-slate
/// plan's (`world::plan`); the old realm has its own castles and the site is
/// inert there.
pub fn on_map() -> bool {
    super::plan::tabula()
}

/// The plans the game ships, each baked from `angel-castle/v1`.
pub const PLANS: [&str; 1] = ["great-tower"];

/// The Keep's plan.
pub const KEEP: &str = "great-tower";

/// Castle `plan`, loaded once; `None` for a plan this build does not know.
pub fn castle(plan: &str) -> Option<&'static Castle> {
    static GREAT_TOWER: OnceLock<Castle> = OnceLock::new();
    match plan {
        "great-tower" => Some(GREAT_TOWER.get_or_init(|| {
            Castle::decode(include_bytes!("../../assets/works/great-tower.castle")).expect("the great tower's baked asset reads")
        })),
        _ => None,
    }
}

/// What each stone of castle `plan` rests on (`works_siege`), derived once from its stones.
pub fn rests(plan: &str) -> Option<&'static super::works_siege::Rests> {
    static GREAT_TOWER: OnceLock<super::works_siege::Rests> = OnceLock::new();
    match plan {
        "great-tower" => castle(plan).map(|c| GREAT_TOWER.get_or_init(|| super::works_siege::Rests::of(c))),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../../tests/world/works__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/world/works_siege__tests.rs"]
mod siege_tests;
