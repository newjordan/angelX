//! The one context button. E opens, reads, lifts, throws, takes a flame,
//! lights, unlocks, relights; its label (`Ctx::label`) changes with what the
//! knight faces. With nothing to do it is the roll, or, locked on, the hop.
use super::kit::*;
use super::world::*;
use super::*;

/// Where the party stands on keys: shared, like the Delve's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hold {
    /// Not found yet.
    Unfound,
    /// Carried by a seat, last seen here.
    Carried(u8, i32, i32),
    /// Put down where its carrier fell.
    Lying(i32, i32),
    /// Spent on the door.
    Spent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Party {
    pub map: bool,
    pub lantern: bool,
    pub small_keys: u8,
    pub boss: Hold,
    /// Heart containers found.
    pub containers: u8,
}

impl Party {
    pub fn new() -> Party {
        Party {
            map: false,
            lantern: false,
            small_keys: 0,
            boss: Hold::Unfound,
            containers: 0,
        }
    }
}

impl Default for Party {
    fn default() -> Self {
        Self::new()
    }
}

/// The item held up for all to see, whose it is and for how long.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Banner {
    pub item: Item,
    pub seat: u8,
    pub t: u16,
}

impl Session {
    /// The nearest thing in reach and in front of the knight that E would
    /// act on.
    pub(super) fn context(&self, k: &Seat) -> Ctx {
        if k.carry == Carry::Pot {
            return Ctx::Throw;
        }
        for (&d, o) in &self.seats {
            if o.act == Act::Down && dist(o.x - k.x, o.y - k.y) < REVIVE_REACH * Q {
                return Ctx::Revive(d as u8);
            }
        }
        let mut best: Option<(i32, Ctx)> = None;
        for (i, p) in self.props.iter().enumerate() {
            let c = match p.kind {
                Pk::Chest if p.on == 0 => Ctx::Open(i as u8),
                Pk::Sign => Ctx::Read(i as u8),
                Pk::Folk => Ctx::Talk(i as u8),
                Pk::Lever if p.on == 0 => Ctx::Pull(i as u8),
                // A bomb pot is too heavy to lift: break it.
                Pk::Pot if p.on == 0 && p.id == 0 => Ctx::Lift(i as u8),
                Pk::Ember => Ctx::Flame,
                Pk::Brazier if !p.lit() && k.taper > 0 => Ctx::Light(i as u8),
                Pk::Lock if p.on == 0 && self.inv.small_keys > 0 => Ctx::Unlock(i as u8),
                Pk::Boss if p.on == 0 && self.carrying_boss_key(k) => Ctx::Unlock(i as u8),
                Pk::Beacon if p.on == 0 && self.beacon_ready() => Ctx::Relight,
                _ => continue,
            };
            let (dx, dy) = (p.x - k.x, p.y - k.y);
            let d = dist(dx, dy);
            if d > CTX_REACH * Q {
                continue;
            }
            // Facing it, or close enough to be at it from any side.
            if d > 12 * Q && dot(k.face, unit(dx, dy)) < CTX_FRONT {
                continue;
            }
            if best.is_none_or(|(b, _)| d < b) {
                best = Some((d, c));
            }
        }
        best.map_or(Ctx::None, |(_, c)| c)
    }

    pub(super) fn carrying_boss_key(&self, k: &Seat) -> bool {
        matches!(self.inv.boss, Hold::Carried(c, _, _) if c == k.id)
    }

    /// E pressed with a thing in reach. False when there is nothing to do
    /// (the caller rolls or hops).
    pub(super) fn use_context(&mut self, id: u32, k: &mut Seat, r: &Rules, ev: &mut Events) -> bool {
        match self.context_for(id, k) {
            Ctx::None => false,
            Ctx::Open(i) => {
                self.open_chest(id, k, usize::from(i), r, ev);
                true
            }
            Ctx::Read(i) => {
                let p = &self.props[usize::from(i)];
                k.read = Some((p.id, READ_TICKS));
                ev.sound("card_pickup");
                true
            }
            Ctx::Talk(i) => {
                let p = &self.props[usize::from(i)];
                k.read = Some((100 + p.id, READ_TICKS));
                ev.sound("card_pickup");
                true
            }
            Ctx::Pull(i) => {
                self.props[usize::from(i)].on = 1;
                ev.sound("wall_up");
                if let Some(g) = self.props.iter().find(|q| q.kind == Pk::Gate && q.id == 2) {
                    let (x, y) = (g.x, g.y);
                    self.set_gate(2, true);
                    self.solved(x, y, ev);
                    ev.sound("door_open");
                }
                true
            }
            Ctx::Lift(i) => {
                let p = &mut self.props[usize::from(i)];
                p.on = 1;
                k.carry = Carry::Pot;
                ev.sound("bomb_toss");
                true
            }
            Ctx::Throw => {
                k.carry = Carry::Nothing;
                if self.bolts.len() < BOLT_CAP {
                    self.bolts.push(Bolt {
                        kind: Bk::Pot,
                        x: k.x + k.face.0 * 8,
                        y: k.y + k.face.1 * 8,
                        vx: k.face.0 * POT_SPEED / Q,
                        vy: k.face.1 * POT_SPEED / Q,
                        ttl: POT_LIFE,
                        owner: id as u8,
                    });
                }
                ev.sound("bomb_toss");
                true
            }
            Ctx::Flame => {
                k.taper = TAPER;
                self.fx(Fxk::Light, k.x, k.y - 8 * Q, 0);
                ev.sound("spell_fire");
                true
            }
            Ctx::Light(i) => {
                let burn = self.burn_ticks();
                let p = &mut self.props[usize::from(i)];
                p.t = burn;
                let (x, y) = (p.x, p.y);
                self.fx(Fxk::Light, x, y, 0);
                ev.sound("spell_fire");
                true
            }
            Ctx::Unlock(i) => {
                let kind = self.props[usize::from(i)].kind;
                if kind == Pk::Lock {
                    self.inv.small_keys = self.inv.small_keys.saturating_sub(1);
                } else {
                    self.inv.boss = Hold::Spent;
                }
                let p = &mut self.props[usize::from(i)];
                p.on = 1;
                let (x, y) = (p.x, p.y);
                self.solved(x, y, ev);
                ev.sound("door_open");
                true
            }
            // Held, not pressed: the pass over the fallen and the beacon.
            Ctx::Revive(_) | Ctx::Relight => true,
        }
    }

    /// As `context`, for a knight who is out of the map for the tick.
    fn context_for(&self, _id: u32, k: &Seat) -> Ctx {
        self.context(k)
    }

    fn open_chest(&mut self, id: u32, k: &mut Seat, i: usize, r: &Rules, ev: &mut Events) {
        let (room, cid) = (usize::from(self.room), self.props[i].id);
        self.props[i].on = 1;
        let item = props::chest_item(room, cid);
        ev.sound("chest_open");
        let (x, y) = (self.props[i].x, self.props[i].y);
        self.fx(Fxk::Light, x, y, 0);
        self.grant(id, k, item, r, ev);
    }

    /// An item comes to the party; its finder holds it up.
    pub(super) fn grant(&mut self, id: u32, k: &mut Seat, item: Item, r: &Rules, ev: &mut Events) {
        match item {
            Item::Map => self.inv.map = true,
            Item::Lantern => {
                self.inv.lantern = true;
                k.flame = r.flame;
                for o in self.seats.values_mut() {
                    o.flame = r.flame;
                }
            }
            Item::SmallKey => self.inv.small_keys = self.inv.small_keys.saturating_add(1),
            Item::BossKey => self.inv.boss = Hold::Carried(id as u8, k.x, k.y),
            Item::Container => {
                self.inv.containers = self.inv.containers.saturating_add(1);
                k.max_hp += HEART;
                k.hp = k.max_hp;
                for o in self.seats.values_mut() {
                    o.max_hp += HEART;
                    o.hp = o.max_hp;
                }
            }
            Item::Gems(n) => k.gems += u32::from(n),
            Item::Bombs(n) => k.bombs += u32::from(n),
        }
        k.act = Act::Item;
        k.t = 0;
        k.lamp = 0;
        k.item = Some(item);
        k.lock = None;
        k.shield = 0;
        k.chain = 0;
        k.step = 0;
        k.charge = 0;
        self.banner = Some(Banner {
            item,
            seat: id as u8,
            t: ITEM_TICKS as u16,
        });
        ev.sound(match item {
            Item::Gems(_) | Item::Bombs(_) => "spoil_pickup",
            _ => "wheel_land",
        });
    }

    /// Ticks a brazier lit from an ember burns: shorter the more hands.
    pub(super) fn burn_ticks(&self) -> u16 {
        let s = self.seats.len().max(1) as u16;
        BURN.saturating_sub(BURN_STEP * (s - 1)).max(BURN_MIN)
    }

    pub(super) fn beacon_ready(&self) -> bool {
        self.room == 4 && self.stage >= 3
    }

    /// A crystal wakes: the chime, and whatever it is wired to (the stage
    /// rows read `Prop::on`).
    pub(super) fn light_crystal(&mut self, i: usize, ev: &mut Events) {
        if self.props[i].on == 1 {
            return;
        }
        self.props[i].on = 1;
        let (x, y) = (self.props[i].x, self.props[i].y);
        self.solved(x, y, ev);
    }

    /// Start a block sliding one tile the way it was pushed, if the place is
    /// open.
    pub(super) fn start_slide(&mut self, b: usize) {
        let (dx, dy) = match self.props[b].on {
            1 => (1, 0),
            2 => (-1, 0),
            3 => (0, 1),
            _ => (0, -1),
        };
        let (x, y) = (self.props[b].x, self.props[b].y);
        let (nx, ny) = (x + dx * TILE * Q, y + dy * TILE * Q);
        let p = plan(usize::from(self.room));
        // A block never goes where it could not be pushed out again: not onto
        // a tile against a wall or a pit, so none is ever stuck for good.
        let (dtx, dty) = (tile_of(nx), tile_of(ny));
        let against = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .any(|&(ax, ay)| matches!(p.at(dtx + ax, dty + ay), b'#' | b'V' | b'=' | b'Q'));
        let open = !against
            && !wall(p.at(tile_of(nx), tile_of(ny)))
            && p.at(tile_of(nx), tile_of(ny)) != b'='
            && p.at(tile_of(nx), tile_of(ny)) != b'Q'
            && !self
                .props
                .iter()
                .enumerate()
                .any(|(j, q)| j != b && q.kind != Pk::Plate && q.blocks(nx, ny));
        if open {
            self.props[b].t = TILE as u16;
        } else {
            self.props[b].on = 0;
            self.props[b].t = 0;
        }
    }
}
