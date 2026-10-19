//! What walks, floats and rolls in Bannerpeak besides the knights: snuffers
//! (hooded wisps with a wick-stub tail), the ones that wear a cap, wisps on the
//! wing, the loaf and lantern flame a rune block gives, caps to kick or carry,
//! bolts of light, and the bell boulder. Each is a small integer record; the
//! session decides who touches whom.
use super::body::{Q, TQ};
use super::world::{Grid, TILE};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FoeKind {
    /// Walks its ledge back and forth.
    Snuffer,
    /// A snuffer wearing a cap: a stomp leaves the cap behind.
    Capped,
    /// Floats to and fro and bobs.
    Wisp,
}

impl FoeKind {
    /// Width and height in pixels.
    pub fn size(self) -> (i32, i32) {
        match self {
            FoeKind::Snuffer => (14, 14),
            FoeKind::Capped => (14, 18),
            FoeKind::Wisp => (12, 12),
        }
    }
    pub fn slot(self) -> &'static str {
        match self {
            FoeKind::Snuffer => "bannerpeak.snuffer",
            FoeKind::Capped => "bannerpeak.capped",
            FoeKind::Wisp => "bannerpeak.wisp",
        }
    }
}

/// Ticks a defeated foe pops into cold smoke before it is gone.
pub const POP: u32 = 12;
const WALK: i32 = 170;
const CAPPED_WALK: i32 = 130;
const WISP_REACH: i32 = 44;
const WISP_PERIOD: u32 = 150;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Foe {
    pub kind: FoeKind,
    /// The feet's centre, 1/256 pixel.
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub dir: i8,
    pub t: u32,
    /// Ticks left of the pop; 0 while alive.
    pub dead: u32,
    /// Where a wisp floats around, pixels.
    pub home: i32,
    /// Which mark of the map it came from.
    pub mark: u16,
}

impl Foe {
    pub fn new(kind: FoeKind, cell: (i32, i32), mark: u16) -> Foe {
        let x = cell.0 * TILE + TILE / 2;
        let (y, home) = match kind {
            // A wisp's mark is where it floats; the others stand on the cell.
            FoeKind::Wisp => (cell.1 * TILE + TILE / 2 + 6, x),
            _ => ((cell.1 + 1) * TILE, x),
        };
        Foe {
            kind,
            x: x * Q,
            y: y * Q,
            vx: 0,
            vy: 0,
            dir: if mark % 2 == 0 { -1 } else { 1 },
            t: 0,
            dead: 0,
            home,
            mark,
        }
    }
    pub fn alive(&self) -> bool {
        self.dead == 0
    }
    /// The body box in pixels: left, top, right, bottom.
    pub fn rect(&self) -> (i32, i32, i32, i32) {
        let (w, h) = self.kind.size();
        let (x, y) = (self.x.div_euclid(Q), self.y.div_euclid(Q));
        (x - w / 2, y - h, x + w / 2, y)
    }
    pub fn defeat(&mut self) {
        self.dead = POP;
        self.vx = 0;
    }
    pub fn valid(&self, width_px: i32, height_px: i32) -> bool {
        (0..=width_px * Q).contains(&self.x)
            && (-4 * TQ..=(height_px + 64) * Q).contains(&self.y)
            && (-4096..=4096).contains(&self.vx)
            && (-6000..=6000).contains(&self.vy)
            && [-1, 1].contains(&self.dir)
            && self.dead <= POP
            && self.t < 1 << 24
    }
    /// One tick. A foe far from every knight stands still.
    pub fn step(&mut self, g: &dyn Grid, near: bool) {
        if self.dead > 0 {
            self.dead -= 1;
            return;
        }
        if !near {
            return;
        }
        self.t = self.t.wrapping_add(1);
        match self.kind {
            FoeKind::Wisp => {
                let swing = super::layout::ping_pong(self.t, WISP_PERIOD, 0, -WISP_REACH, WISP_REACH);
                let x = self.home + swing;
                self.dir = if x * Q >= self.x { 1 } else { -1 };
                self.x = x * Q;
            }
            FoeKind::Snuffer | FoeKind::Capped => {
                let speed = if self.kind == FoeKind::Capped {
                    CAPPED_WALK
                } else {
                    WALK
                };
                self.vx = i32::from(self.dir) * speed;
                self.vy = (self.vy + 210).min(3000);
                // Along, turning at a wall or at the edge of its ledge.
                let (w, h) = self.kind.size();
                let nx = self.x + self.vx;
                let edge = if self.dir > 0 {
                    nx + w * Q / 2
                } else {
                    nx - w * Q / 2
                };
                let col = edge.div_euclid(TQ);
                let (top, bottom) = (
                    (self.y - h * Q).div_euclid(TQ),
                    (self.y - 1).div_euclid(TQ),
                );
                let wall = (top..=bottom).any(|r| g.tile(col, r).solid());
                let ground = self.vy == 0 || self.on_floor(g);
                let drop = ground && !g.tile(col, self.y.div_euclid(TQ)).solid()
                    && !g.tile(col, self.y.div_euclid(TQ)).one_way();
                if wall || drop {
                    self.dir = -self.dir;
                    self.vx = 0;
                } else {
                    self.x = nx;
                }
                // Down onto what is below.
                let ny = self.y + self.vy;
                let foot_col = self.x.div_euclid(TQ);
                let row = ny.div_euclid(TQ);
                let below = g.tile(foot_col, row);
                if self.vy > 0 && (below.solid() || below.one_way()) && self.y <= row * TQ {
                    self.y = row * TQ;
                    self.vy = 0;
                } else {
                    self.y = ny;
                }
            }
        }
    }
    fn on_floor(&self, g: &dyn Grid) -> bool {
        self.y % TQ == 0 && g.tile(self.x.div_euclid(TQ), self.y.div_euclid(TQ)).solid()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    /// A hearth loaf: a small knight grows, a big one is mended.
    Loaf,
    /// A lantern flame: a big knight throws bolts of light.
    Flame,
    /// What a capped snuffer leaves: kick it, or carry it.
    Cap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapState {
    Rest,
    Slide,
    Carried,
}

/// Ticks a cap may be carried before what is under it wakes.
pub const CARRY: u32 = 150;
/// Ticks a block's gift takes to rise out of it.
pub const RISE: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub kind: ItemKind,
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub dir: i8,
    pub t: u32,
    pub cap: CapState,
    /// The knight who kicked or carries it.
    pub owner: u32,
}

impl Item {
    pub fn new(kind: ItemKind, x_px: i32, y_px: i32) -> Item {
        Item {
            kind,
            x: x_px * Q,
            y: y_px * Q,
            vx: 0,
            vy: 0,
            dir: 1,
            t: 0,
            cap: CapState::Rest,
            owner: 0,
        }
    }
    pub fn size(&self) -> (i32, i32) {
        match self.kind {
            ItemKind::Loaf => (12, 12),
            ItemKind::Flame => (10, 14),
            ItemKind::Cap => (14, 10),
        }
    }
    pub fn rect(&self) -> (i32, i32, i32, i32) {
        let (w, h) = self.size();
        let (x, y) = (self.x.div_euclid(Q), self.y.div_euclid(Q));
        (x - w / 2, y - h, x + w / 2, y)
    }
    /// Still coming out of its block: touching does nothing yet.
    pub fn rising(&self) -> bool {
        matches!(self.kind, ItemKind::Loaf | ItemKind::Flame) && self.t < RISE
    }
    pub fn valid(&self, width_px: i32, height_px: i32) -> bool {
        (0..=width_px * Q).contains(&self.x)
            && (-4 * TQ..=(height_px + 64) * Q).contains(&self.y)
            && (-4096..=4096).contains(&self.vx)
            && (-6000..=6000).contains(&self.vy)
            && [-1, 1].contains(&self.dir)
            && self.t < 1 << 24
            && self.owner <= 8
    }
    pub fn step(&mut self, g: &dyn Grid) {
        self.t = self.t.saturating_add(1);
        match self.kind {
            ItemKind::Flame => {
                // Up out of the block, then it stays and flickers.
                if self.t <= RISE {
                    self.y -= Q;
                }
            }
            ItemKind::Loaf => {
                if self.t <= RISE {
                    self.y -= Q;
                    return;
                }
                self.walk(g, 384, true);
            }
            ItemKind::Cap => match self.cap {
                CapState::Rest | CapState::Carried => {
                    if self.cap == CapState::Rest {
                        self.walk(g, 0, false);
                    }
                }
                CapState::Slide => self.walk(g, 1800, false),
            },
        }
    }
    /// Along at `speed` (0: stand), falling, turning at walls; a loaf also
    /// turns at a ledge's edge, a cap goes over it.
    fn walk(&mut self, g: &dyn Grid, speed: i32, careful: bool) {
        let (w, h) = self.size();
        self.vy = (self.vy + 210).min(3000);
        if speed != 0 {
            self.vx = i32::from(self.dir) * speed;
            let nx = self.x + self.vx;
            let edge = if self.dir > 0 {
                nx + w * Q / 2
            } else {
                nx - w * Q / 2
            };
            let col = edge.div_euclid(TQ);
            let (top, bottom) = (
                (self.y - h * Q).div_euclid(TQ),
                (self.y - 1).div_euclid(TQ),
            );
            let wall = (top..=bottom).any(|r| g.tile(col, r).solid());
            let drop = careful
                && self.vy == 0
                && !g.tile(col, self.y.div_euclid(TQ)).solid()
                && !g.tile(col, self.y.div_euclid(TQ)).one_way();
            if wall || drop {
                self.dir = -self.dir;
            } else {
                self.x = nx;
            }
        }
        let ny = self.y + self.vy;
        let row = ny.div_euclid(TQ);
        let below = g.tile(self.x.div_euclid(TQ), row);
        if self.vy > 0 && (below.solid() || below.one_way()) && self.y <= row * TQ {
            self.y = row * TQ;
            self.vy = 0;
        } else {
            self.y = ny;
        }
    }
}

/// A bolt of light a flame throws: it bounces along the ground.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bolt {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub t: u32,
    pub owner: u32,
}

pub const BOLT_LIFE: u32 = 70;
pub const BOLT_SPEED: i32 = 1900;
pub const BOLT_MOST: usize = 2;

impl Bolt {
    pub fn valid(&self, width_px: i32, height_px: i32) -> bool {
        (0..=width_px * Q).contains(&self.x)
            && (-4 * TQ..=(height_px + 64) * Q).contains(&self.y)
            && (-4096..=4096).contains(&self.vx)
            && (-6000..=6000).contains(&self.vy)
            && self.t <= BOLT_LIFE
            && self.owner <= 8
    }
    pub fn rect(&self) -> (i32, i32, i32, i32) {
        let (x, y) = (self.x.div_euclid(Q), self.y.div_euclid(Q));
        (x - 3, y - 3, x + 3, y + 3)
    }
    /// One tick; false once it has spent itself on a wall.
    pub fn step(&mut self, g: &dyn Grid) -> bool {
        self.t += 1;
        let nx = self.x + self.vx;
        if g.tile(nx.div_euclid(TQ), self.y.div_euclid(TQ)).solid() {
            return false;
        }
        self.x = nx;
        self.vy = (self.vy + 130).min(2400);
        let ny = self.y + self.vy;
        let below = g.tile(self.x.div_euclid(TQ), (ny + 3 * Q).div_euclid(TQ));
        if self.vy > 0 && (below.solid() || below.one_way()) {
            // A low hop: it stays at a snuffer's height.
            self.vy = -900;
        } else if self.vy < 0 && g.tile(self.x.div_euclid(TQ), (ny - 3 * Q).div_euclid(TQ)).solid() {
            self.vy = 0;
        } else {
            self.y = ny;
        }
        self.t <= BOLT_LIFE
    }
}

/// A bell boulder, rolling along its track.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Boulder {
    pub x: i32,
    pub t: u32,
}

pub const BOULDER: (i32, i32) = (28, 28);
