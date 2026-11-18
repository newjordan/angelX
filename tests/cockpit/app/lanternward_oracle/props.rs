//! What stands in a room: pots and grass, braziers, gates, plates, blocks,
//! crystals, chests and doors; what lies on the floor; the puffs and sparks
//! the picture needs. The puzzles are the rows of `Session::tick_stage`.
use super::world::*;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pk {
    Pot,
    Grass,
    Sign,
    Brazier,
    Rest,
    Ember,
    Great,
    Chest,
    Gate,
    Plate,
    Block,
    Crystal,
    Cracked,
    Sigil,
    Lock,
    Boss,
    Beacon,
    Fissure,
    Folk,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prop {
    pub kind: Pk,
    pub tx: u8,
    pub ty: u8,
    pub id: u8,
    pub on: u8,
    pub t: u16,
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

    pub fn at(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn lit(&self) -> bool {
        match self.kind {
            Pk::Rest | Pk::Ember => true,
            Pk::Brazier => self.t > 0 || self.on == 1,
            Pk::Great | Pk::Beacon => self.on == 1,
            _ => false,
        }
    }

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

    pub fn lets_bolts(&self) -> bool {
        matches!(self.kind, Pk::Gate | Pk::Sigil | Pk::Grass | Pk::Plate | Pk::Fissure | Pk::Pot)
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    Map,
    Lantern,
    SmallKey,
    BossKey,
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
    pub age: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fxk {
    Puff,
    Light,
    Spark,
    Ring,
    Parry,
    Tick,
    Dust,
    Solved,
    Blast,
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
    pub a: i16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fuse {
    pub x: i32,
    pub y: i32,
    pub t: u8,
    pub owner: u8,
}

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
                Pk::Chest if room == 3 && id == 1 => p.on = 2,
                Pk::Rest => p.on = 1,
                _ => {}
            }
            out.push(p);
        }
    }
    out
}

pub fn sign_text(room: usize, id: u8) -> &'static str {
    match (room, id) {
        (0, _) => "HOLD R TO LOCK ON",
        (1, _) => "FLARE SHOWS WHAT IS HIDDEN",
        (2, _) => "SHIELD UP AS IT FLIES",
        (3, _) => "LIGHT BURNS THE DARK OFF",
        _ => "",
    }
}

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

    pub(super) fn roll(&mut self, n: u32) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.seed >> 33) % u64::from(n.max(1))) as u32
    }

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

    pub fn blocked(&self, x: i32, y: i32) -> bool {
        if wall(plan(usize::from(self.room)).at(tile_of(x), tile_of(y))) {
            return true;
        }
        self.props.iter().any(|p| p.blocks(x, y))
    }

    pub fn secret_open(&self) -> bool {
        self.props
            .iter()
            .any(|p| (p.kind == Pk::Cracked || p.kind == Pk::Gate && p.id == 2) && p.on == 1)
    }

    pub fn clear(&self, x: i32, y: i32, r: i32) -> bool {
        !(self.blocked(x - r, y - r)
            || self.blocked(x + r, y - r)
            || self.blocked(x - r, y + r)
            || self.blocked(x + r, y + r))
    }

    pub fn pit(&self, x: i32, y: i32) -> bool {
        plan(usize::from(self.room)).at(tile_of(x), tile_of(y)) == b'='
    }

    pub fn line_open(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let steps = (dist(dx, dy) / (6 * Q)).max(1);
        let p = plan(usize::from(self.room));
        let (ta, tb) = ((tile_of(a.0), tile_of(a.1)), (tile_of(b.0), tile_of(b.1)));
        (1..steps).all(|i| {
            let (x, y) = (a.0 + dx * i / steps, a.1 + dy * i / steps);
            if (tile_of(x), tile_of(y)) == ta || (tile_of(x), tile_of(y)) == tb {
                return true;
            }
            p.at(tile_of(x), tile_of(y)) != b'#' && !self.props.iter().any(|q| !q.lets_bolts() && q.blocks(x, y))
        })
    }

    pub fn burning(&self, x: i32, y: i32) -> bool {
        self.seats.values().any(|k| {
            k.flaring() && dist(k.x - x, k.y - y) < FLARE_R * Q
        }) || self.props.iter().any(|p| {
            p.lit() && dist(p.x - x, p.y - y) < BRAZIER_R * Q
        })
    }

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

    pub fn light_at(&self, x: i32, y: i32) -> u8 {
        light_in(&self.lights(), x, y)
    }

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

    pub(super) fn solved(&mut self, x: i32, y: i32, ev: &mut Events) {
        self.fx(Fxk::Solved, x, y, 0);
        if self.cue.is_none() {
            self.cue = Some((x, y, CUE_TICKS));
        }
        ev.sound("wish_grant");
    }
}

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
