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

    pub fn prop_of(&self, kind: Pk, id: u8) -> Option<usize> {
        self.props.iter().position(|p| p.kind == kind && p.id == id)
    }

    pub fn exit_open(&self) -> bool {
        match self.room {
            0 | 2 => self.stage >= 1,
            1 | 3 => self.stage >= 4,
            _ => false,
        }
    }

    pub fn lock_point(&self, l: Lock) -> Option<(i32, i32)> {
        match l {
            Lock::Foe(id) => self.foes.iter().find(|f| f.id == id && f.alive()).map(|f| (f.x, f.y)),
            Lock::Boss => self.boss.as_ref().filter(|w| w.hp > 0).map(|w| w.heart()),
            Lock::Prop(i) => self.props.get(usize::from(i)).filter(|p| p.target()).map(|p| (p.x, p.y)),
        }
    }

    pub fn boss_open(&self) -> bool {
        self.boss.as_ref().is_some_and(|w| w.exposed(self.elapsed, self.seats.len() as u32))
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

    pub fn light_at(&self, x: i32, y: i32) -> u8 {
        light_in(&self.lights(), x, y)
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
