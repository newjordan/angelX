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

    /// The prop of this kind and number, if the room has one.
    pub fn prop_of(&self, kind: Pk, id: u8) -> Option<usize> {
        self.props.iter().position(|p| p.kind == kind && p.id == id)
    }

    /// Whether the way on is open.
    pub fn exit_open(&self) -> bool {
        match self.room {
            0 | 2 => self.stage >= 1,
            1 | 3 => self.stage >= 4,
            _ => false,
        }
    }

    /// Where a lock-on sits, if what it holds is still there.
    pub fn lock_point(&self, l: Lock) -> Option<(i32, i32)> {
        match l {
            Lock::Foe(id) => self.foes.iter().find(|f| f.id == id && f.alive()).map(|f| (f.x, f.y)),
            Lock::Boss => self.boss.as_ref().filter(|w| w.hp > 0).map(|w| w.heart()),
            Lock::Prop(i) => self.props.get(usize::from(i)).filter(|p| p.target()).map(|p| (p.x, p.y)),
        }
    }

    /// Whether the Warden's lamp-heart stands open.
    pub fn boss_open(&self) -> bool {
        self.boss.as_ref().is_some_and(|w| w.exposed(self.elapsed, self.seats.len() as u32))
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
            && self.boss_open()
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
