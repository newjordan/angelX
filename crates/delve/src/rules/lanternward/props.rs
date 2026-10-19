//! What stands in a room: pots and grass, braziers, gates, plates, blocks,
//! crystals, chests and doors; what lies on the floor; the puffs and sparks
//! the picture needs. The puzzles are the rows of `Session::tick_stage`.
use super::world::*;
use super::*;

/// A kind of thing that stands on the floor, by the letter that marks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pk {
    Pot,
    Grass,
    Sign,
    /// A brazier to be lit; it gutters out (`Prop::t`) unless lit for good.
    Brazier,
    /// A resting brazier: always lit, mends the flame, the party's checkpoint.
    Rest,
    /// The ember bowl: E takes a flame from it.
    Ember,
    /// The great brazier behind bars: lit only by a thrown bolt.
    Great,
    Chest,
    Gate,
    Plate,
    Block,
    Crystal,
    Cracked,
    Sigil,
    /// A small-key door.
    Lock,
    /// The Boss Key door.
    Boss,
    Beacon,
    Fissure,
    /// A kingsman of the Keep: E talks.
    Folk,
    /// A lever: E pulls it.
    Lever,
}

impl Pk {
    pub fn of(c: u8) -> Option<Pk> {
        Some(match c {
            b'o' | b'x' => Pk::Pot,
            b'"' => Pk::Grass,
            b's' => Pk::Sign,
            b'B' => Pk::Brazier,
            b'C' => Pk::Rest,
            b'E' => Pk::Ember,
            b'b' => Pk::Great,
            b'c' => Pk::Chest,
            b'G' | b'1'..=b'9' => Pk::Gate,
            b'P' | b'Y' => Pk::Plate,
            b'X' => Pk::Block,
            b'K' => Pk::Crystal,
            b'~' => Pk::Cracked,
            b'N' => Pk::Sigil,
            b'L' => Pk::Lock,
            b'M' => Pk::Boss,
            b'*' => Pk::Beacon,
            b'f' => Pk::Fissure,
            b'F' => Pk::Folk,
            b'/' => Pk::Lever,
            _ => return None,
        })
    }
}

/// A thing in the room. `on` and `t` mean what the kind says (see `Pk`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prop {
    pub kind: Pk,
    pub tx: u8,
    pub ty: u8,
    /// A gate's number (0 the exit), a chest's, sign's or folk's place in
    /// its room; a bomb pot is 1; the second plate is 1.
    pub id: u8,
    /// Pot, grass: 1 gone. Chest: 1 open, 2 not yet there. Gate, lock,
    /// boss door, cracked wall: 1 open. Plate: 1 pressed. Crystal, great
    /// brazier, beacon: 1 lit. Sigil: 1 found. Brazier: 1 lit for good.
    /// Block: the way it slides (1 east, 2 west, 3 south, 4 north). Lever:
    /// 1 pulled.
    pub on: u8,
    /// Brazier: ticks left alight. Block: ticks left sliding. Sigil,
    /// beacon: progress. Fissure: ticks open.
    pub t: u16,
    /// A block's middle, in `Q` pixels.
    pub x: i32,
    pub y: i32,
}

impl Prop {
    pub fn new(kind: Pk, tx: i32, ty: i32, id: u8) -> Prop {
        Prop {
            kind,
            tx: tx as u8,
            ty: ty as u8,
            id,
            on: 0,
            t: 0,
            x: middle(tx),
            y: middle(ty),
        }
    }

    /// Where its middle is, in `Q` pixels.
    pub fn at(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    /// Whether it is lit now.
    pub fn lit(&self) -> bool {
        match self.kind {
            Pk::Rest | Pk::Ember => true,
            Pk::Brazier => self.t > 0 || self.on == 1,
            Pk::Great | Pk::Beacon => self.on == 1,
            _ => false,
        }
    }

    /// Whether a body at this point (in `Q` pixels) is stopped by it.
    pub fn blocks(&self, x: i32, y: i32) -> bool {
        let here = tile_of(x) == i32::from(self.tx) && tile_of(y) == i32::from(self.ty);
        match self.kind {
            Pk::Grass | Pk::Plate | Pk::Sigil | Pk::Fissure => false,
            Pk::Pot | Pk::Gate | Pk::Lock | Pk::Boss | Pk::Cracked => self.on == 0 && here,
            Pk::Chest => self.on != 2 && here,
            Pk::Block => (x - self.x).abs() < 8 * Q && (y - self.y).abs() < 8 * Q,
            _ => here,
        }
    }

    /// Whether a bolt flies through it. Bars, gates, the great brazier's
    /// cage and the crystals' niches let light and iron pass.
    pub fn lets_bolts(&self) -> bool {
        matches!(self.kind, Pk::Gate | Pk::Sigil | Pk::Grass | Pk::Plate | Pk::Fissure | Pk::Pot)
    }

    /// Whether the knight in front of it can lock on to it.
    pub fn target(&self) -> bool {
        match self.kind {
            Pk::Sign => true,
            Pk::Crystal | Pk::Great | Pk::Lever => self.on == 0,
            Pk::Folk => true,
            Pk::Brazier => !self.lit(),
            _ => false,
        }
    }
}

/// What a chest holds, and what a knight lifts overhead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    Map,
    Lantern,
    /// The Moon Key, the vault's small key.
    SmallKey,
    BossKey,
    /// A heart container.
    Container,
    Gems(u8),
    Bombs(u8),
}

impl Item {
    pub fn name(self) -> &'static str {
        match self {
            Item::Map => "THE MAP",
            Item::Lantern => "THE LANTERN",
            Item::SmallKey => "THE MOON KEY",
            Item::BossKey => "THE BOSS KEY",
            Item::Container => "A HEART CONTAINER",
            Item::Gems(_) => "GEMS",
            Item::Bombs(_) => "BOMBS",
        }
    }
    /// The words under the name: what to press.
    pub fn how(self) -> &'static str {
        match self {
            Item::Map => "THE KEEP IN YOUR CORNER",
            Item::Lantern => "F THROW LIGHT   HOLD F FLARE",
            Item::SmallKey => "E AT A LOCKED DOOR",
            Item::BossKey => "CARRY IT TO THE DOOR",
            Item::Container => "ONE HEART MORE",
            Item::Gems(_) => "FOR THE PURSE",
            Item::Bombs(_) => "X LAYS ONE",
        }
    }
}

/// What a chest holds, by its room and its place in it.
pub fn chest_item(room: usize, id: u8) -> Item {
    match (room, id) {
        (0, _) => Item::Map,
        (1, 0) => Item::Lantern,
        (1, _) => Item::Gems(25),
        (2, _) => Item::Gems(20),
        (3, 0) => Item::BossKey,
        (3, 1) => Item::SmallKey,
        (3, _) => Item::Gems(25),
        _ => Item::Gems(10),
    }
}

/// What lies on the floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dk {
    Heart,
    Gem,
    Bomb,
    Container,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drop {
    pub kind: Dk,
    pub x: i32,
    pub y: i32,
    /// Ticks lying here (a heart or gem fades away after a while).
    pub age: u16,
}

/// A puff, a spark or a ring for the picture, ticking down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fxk {
    /// A snuffer's cold smoke.
    Puff,
    /// The spark of light a fallen snuffer gives back.
    Light,
    /// A hit: sparks off the blade.
    Spark,
    /// A shield ring.
    Ring,
    /// The white flash and chime of a parry.
    Parry,
    /// The wick-spark that lands on a locked target.
    Tick,
    /// Dust where a body lands or a pot breaks.
    Dust,
    /// A puzzle solved: a star burst.
    Solved,
    /// A bomb's blast.
    Blast,
    /// A shockwave ring at the Warden's feet.
    Wave,
}

impl Fxk {
    pub fn life(self) -> u8 {
        match self {
            Fxk::Puff | Fxk::Light => 16,
            Fxk::Spark | Fxk::Tick => 8,
            Fxk::Ring | Fxk::Dust => 10,
            Fxk::Parry => 12,
            Fxk::Solved => 24,
            Fxk::Blast => 14,
            Fxk::Wave => 20,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fx {
    pub kind: Fxk,
    pub x: i32,
    pub y: i32,
    pub t: u8,
    /// An angle or a size, as the kind says.
    pub a: i16,
}

/// A bomb with its fuse burning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fuse {
    pub x: i32,
    pub y: i32,
    pub t: u8,
    pub owner: u8,
}

/// The room's things, in the order the plan's rows list them.
pub fn build(room: usize) -> Vec<Prop> {
    let plan = plan(room);
    let mut out = Vec::new();
    let (mut chests, mut signs, mut folk) = (0u8, 0u8, 0u8);
    for (y, row) in plan.rows.iter().enumerate() {
        for (x, &c) in row.as_bytes().iter().enumerate() {
            let Some(kind) = Pk::of(c) else { continue };
            let id = match kind {
                Pk::Gate => {
                    if c == b'G' {
                        0
                    } else {
                        c - b'0'
                    }
                }
                Pk::Chest => {
                    chests += 1;
                    chests - 1
                }
                Pk::Sign => {
                    signs += 1;
                    signs - 1
                }
                Pk::Folk => {
                    folk += 1;
                    folk - 1
                }
                Pk::Pot | Pk::Plate => u8::from(matches!(c, b'x' | b'Y')),
                _ => 0,
            };
            let mut p = Prop::new(kind, x as i32, y as i32, id);
            match kind {
                // The vault's key chest is not there until the dark is beaten.
                Pk::Chest if room == 3 && id == 1 => p.on = 2,
                // A resting brazier burns for good.
                Pk::Rest => p.on = 1,
                _ => {}
            }
            out.push(p);
        }
    }
    out
}

/// A sign's few words, by its room and place.
pub fn sign_text(room: usize, id: u8) -> &'static str {
    match (room, id) {
        (0, _) => "HOLD R TO LOCK ON",
        (1, _) => "FLARE SHOWS WHAT IS HIDDEN",
        (2, _) => "SHIELD UP AS IT FLIES",
        (3, _) => "LIGHT BURNS THE DARK OFF",
        _ => "",
    }
}

/// What a kingsman of the Keep says.
pub fn folk_text(room: usize, id: u8) -> &'static str {
    match (room, id) {
        (0, _) => "THE KEEP DARKENS. FIND THE LANTERN.",
        _ => "",
    }
}

impl Session {
    pub(super) fn fx(&mut self, kind: Fxk, x: i32, y: i32, a: i16) {
        if self.fx.len() < FX_CAP {
            self.fx.push(Fx { kind, x, y, t: 0, a });
        }
    }

    pub(super) fn drop_at(&mut self, kind: Dk, x: i32, y: i32) {
        if self.drops.len() < DROP_CAP {
            self.drops.push(Drop { kind, x, y, age: 0 });
        }
    }

    /// The fixed dice: a whole-number step and the high bits of it.
    pub(super) fn roll(&mut self, n: u32) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.seed >> 33) % u64::from(n.max(1))) as u32
    }

    /// A pot or a tuft of grass breaks: what it held lies on the floor. A bomb
    /// pot (`Prop::id` 1) always holds three bombs.
    pub(super) fn shatter(&mut self, i: usize, ev: &mut Events) {
        let (kind, id, x, y) = {
            let p = &self.props[i];
            (p.kind, p.id, p.x, p.y)
        };
        self.props[i].on = 1;
        self.fx(Fxk::Dust, x, y, 0);
        if kind == Pk::Pot && id == 1 {
            for n in 0..3 {
                self.drop_at(Dk::Bomb, x + (n - 1) * 5 * Q, y + 3 * Q);
            }
        } else {
            self.scatter(kind, x, y);
        }
        ev.sound(if kind == Pk::Pot { "rock_land" } else { "step_moss" });
    }

    /// What a cut or broken thing lets fall.
    pub(super) fn scatter(&mut self, kind: Pk, x: i32, y: i32) {
        let r = self.roll(100);
        let drop = match kind {
            Pk::Grass => match r {
                0..=11 => Some(Dk::Gem),
                12..=21 => Some(Dk::Heart),
                _ => None,
            },
            Pk::Pot => match r {
                0..=17 => Some(Dk::Heart),
                18..=44 => Some(Dk::Gem),
                45..=54 => Some(Dk::Bomb),
                _ => None,
            },
            // A fallen snuffer.
            _ => match r {
                0..=29 => Some(Dk::Heart),
                30..=64 => Some(Dk::Gem),
                _ => None,
            },
        };
        if let Some(d) = drop {
            self.drop_at(d, x, y);
        }
    }

    /// Whether any closed thing in the room stops a body at this point.
    pub fn blocked(&self, x: i32, y: i32) -> bool {
        if wall(plan(usize::from(self.room)).at(tile_of(x), tile_of(y))) {
            return true;
        }
        self.props.iter().any(|p| p.blocks(x, y))
    }

    /// Whether the room's secret closet has been opened.
    pub fn secret_open(&self) -> bool {
        self.props
            .iter()
            .any(|p| (p.kind == Pk::Cracked || p.kind == Pk::Gate && p.id == 2) && p.on == 1)
    }

    /// Whether a body of radius `r` (in `Q` pixels) fits at this point.
    pub fn clear(&self, x: i32, y: i32, r: i32) -> bool {
        !(self.blocked(x - r, y - r)
            || self.blocked(x + r, y - r)
            || self.blocked(x - r, y + r)
            || self.blocked(x + r, y + r))
    }

    /// Whether this point is over a pit.
    pub fn pit(&self, x: i32, y: i32) -> bool {
        plan(usize::from(self.room)).at(tile_of(x), tile_of(y)) == b'='
    }

    /// Whether the way from `a` to `b` is open to a bolt: only walls stop it.
    pub fn line_open(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let steps = (dist(dx, dy) / (6 * Q)).max(1);
        let p = plan(usize::from(self.room));
        let (ta, tb) = ((tile_of(a.0), tile_of(a.1)), (tile_of(b.0), tile_of(b.1)));
        (1..steps).all(|i| {
            let (x, y) = (a.0 + dx * i / steps, a.1 + dy * i / steps);
            // The two ends' own tiles are not in the way.
            if (tile_of(x), tile_of(y)) == ta || (tile_of(x), tile_of(y)) == tb {
                return true;
            }
            p.at(tile_of(x), tile_of(y)) != b'#' && !self.props.iter().any(|q| !q.lets_bolts() && q.blocks(x, y))
        })
    }

    /// Whether a point is held lit by a flare or a burning brazier (what
    /// strips a snuffer's cloak).
    pub fn burning(&self, x: i32, y: i32) -> bool {
        self.seats.values().any(|k| {
            k.flaring() && dist(k.x - x, k.y - y) < FLARE_R * Q
        }) || self.props.iter().any(|p| {
            p.lit() && dist(p.x - x, p.y - y) < BRAZIER_R * Q
        })
    }

    /// Every light in the room as `(x, y, reach)` in `Q` pixels: knights'
    /// lanterns (wide when flaring), lit braziers, bolts of light, an open
    /// lamp-heart, and what a camera cue looks at.
    pub fn lights(&self) -> Vec<(i32, i32, i32)> {
        let mut out = Vec::new();
        for k in self.seats.values().filter(|k| k.act != Act::Down) {
            out.push((k.x, k.y, if k.flaring() { FLARE_R } else { GLOW_R } * Q));
        }
        for p in self.props.iter().filter(|p| p.lit()) {
            out.push((p.x, p.y, BRAZIER_R * Q));
        }
        for b in self.bolts.iter().filter(|b| b.light()) {
            out.push((b.x, b.y, 40 * Q));
        }
        if let Some(w) = &self.boss
            && self.boss_exposed()
        {
            let (hx, hy) = w.heart();
            out.push((hx, hy, 64 * Q));
        }
        if let Some((cx, cy, t)) = self.cue
            && t > 6
        {
            out.push((cx, cy, 44 * Q));
        }
        out
    }

    /// How brightly a point is lit, 0 dark to 3 full: for the picture and
    /// for where a body can be seen.
    pub fn light_at(&self, x: i32, y: i32) -> u8 {
        light_in(&self.lights(), x, y)
    }

    /// The gate with this number, opened or shut.
    pub(super) fn set_gate(&mut self, id: u8, open: bool) -> bool {
        let mut changed = false;
        for p in self.props.iter_mut().filter(|p| p.kind == Pk::Gate && p.id == id) {
            let want = u8::from(open);
            if p.on != want {
                p.on = want;
                changed = true;
            }
        }
        changed
    }

    /// A puzzle came right: the chime, the star burst, a cue on the screen.
    pub(super) fn solved(&mut self, x: i32, y: i32, ev: &mut Events) {
        self.fx(Fxk::Solved, x, y, 0);
        if self.cue.is_none() {
            self.cue = Some((x, y, CUE_TICKS));
        }
        ev.sound("wish_grant");
    }
}

/// The brightest of `lights` at a point, 0 to 3, by squared distances (no
/// roots: the picture asks this for every cell).
pub fn light_in(lights: &[(i32, i32, i32)], x: i32, y: i32) -> u8 {
    let mut best = 0u8;
    for &(lx, ly, r) in lights {
        let (dx, dy) = (i64::from(lx - x), i64::from(ly - y));
        let d2 = dx * dx + dy * dy;
        let r = i64::from(r);
        let level = if d2 <= r * r / 4 {
            3
        } else if d2 <= r * r * 9 / 16 {
            2
        } else if d2 <= r * r {
            1
        } else {
            0
        };
        best = best.max(level);
    }
    best
}
