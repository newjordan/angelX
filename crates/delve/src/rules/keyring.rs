//! The Boss Key. Each floor's chief (the one `boss_gates` places mid-floor)
//! is no boss: its camp is led by its toughest, an elite who carries the
//! key, marked so a party can find it, with an elite's gold bar and no boss
//! bar or phases. The floor's one boss is its guardian at the stairs. The
//! carrier whose fall opens the route drops the key where it dies; a knight
//! walks over it to carry it; a knight who falls (turns to stone, or leaves)
//! lets it drop where they stood. The boss room's door, in the room beside
//! it, is shut and chained until the knight carrying the key stands at it,
//! and the key is spent opening it.
//!
//! The sealed room behind (`boss_gates`) stays as it was: the key is the
//! door's lock, not a second rule for the route.

use super::*;

pub const KEY_REACH: f32 = 1.4;
pub const DOOR_REACH: f32 = 5.0;
const REMIND: u64 = 3 * HZ as u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Door {
    #[default]
    Locked,
    Open,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hold {
    #[default]
    Kept,
    Lying { room: usize, x: f32, y: f32 },
    Carried { knight: u32, x: f32, y: f32 },
    Spent,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub door: Door,
    pub hold: Hold,
    #[serde(default)]
    pub told: u64,
}

impl Key {
    pub fn valid(&self) -> bool {
        match self.hold {
            Hold::Lying { room, x, y } => room < 64 && x.is_finite() && y.is_finite(),
            Hold::Carried { knight, x, y } => {
                (1..=MAX_PLAYERS).contains(&knight) && x.is_finite() && y.is_finite()
            }
            _ => true,
        }
    }
}

fn settle(room: &Room, x: f32, y: f32) -> (f32, f32) {
    for ring in 0..16 {
        let r = ring as f32 * 0.5;
        for step in 0..16 {
            let a = step as f32 / 16.0 * std::f32::consts::TAU;
            let (px, py) = (x + a.cos() * r, y + a.sin() * r);
            let (col, row) = (
                (px / TILE_UNITS).floor() as i32,
                (py / TILE_UNITS).floor() as i32,
            );
            if room.tile(col, row) == Tile::Floor {
                return (px, py);
            }
        }
    }
    (room.width() / 2.0, room.height() / 2.0)
}

pub fn bearer_name(kind: EnemyKind) -> String {
    let name = bestiary::ENTRIES
        .iter()
        .find(|e| e.kind == kind)
        .map_or("monster", |e| e.name);
    format!("the elite {name}")
}

impl boss_gates::Gates {
    pub fn key_carrier(&self, at: usize) -> Option<u8> {
        self.leader(at).filter(|l| !l.defeated)?;
        let camp = self.economy.camp(at).filter(|c| c.deployed)?;
        camp.roster
            .iter()
            .enumerate()
            .filter(|&(slot, _)| self.economy.slot_faction(at, slot as u8) == Some(camp.faction))
            .max_by_key(|&(slot, kind)| (kind.hp(), std::cmp::Reverse(slot)))
            .map(|(slot, _)| slot as u8)
    }

    pub fn carrier_kind(&self, at: usize) -> Option<EnemyKind> {
        let slot = self.key_carrier(at)?;
        self.economy
            .camp(at)?
            .roster
            .get(usize::from(slot))
            .copied()
    }

    pub(super) fn carrier_fell(&mut self, at: usize, slot: u8) -> bool {
        if self.key_carrier(at) != Some(slot) {
            return false;
        }
        let Some(i) = self.leaders.iter().position(|l| l.room == at) else {
            return false;
        };
        self.leaders[i].defeated = true;
        self.economy.lose(i as u8);
        true
    }
}

impl Run {
    pub(super) fn carrier_lost(&mut self, x: f32, y: f32, kind: EnemyKind, was_open: bool) {
        self.cues.push(format!("leader_fall:{}", bearer_name(kind)));
        if !was_open && self.boss_gates.as_ref().is_some_and(boss_gates::Gates::unlocked) {
            self.cues.push("boss_unsealed".into());
            self.drop_boss_key(x, y);
        }
    }

    pub(super) fn heal_lost_carrier(&mut self) {
        let at = self.at;
        let Some(g) = &mut self.boss_gates else { return };
        let (Some(slot), Some(kind)) = (g.key_carrier(at), g.carrier_kind(at)) else { return };
        if g.economy.alive_slot(at, slot, kind) || self.enemies.iter().any(|e| e.camp_slot == Some(slot)) {
            return;
        }
        let was_open = g.unlocked();
        if !g.carrier_fell(at, slot) {
            return;
        }
        let (x, y) = self
            .players
            .values()
            .find(|h| h.hp > 0 && !h.absent)
            .map_or((self.room().width() / 2.0, self.room().height() / 2.0), |h| (h.x, h.y));
        self.carrier_lost(x, y, kind, was_open);
    }

    pub fn carries_key(&self, enemy: &Enemy) -> bool {
        enemy.hp > 0
            && self.works.key.hold == Hold::Kept
            && enemy.camp_slot.is_some()
            && self
                .boss_gates
                .as_ref()
                .is_some_and(|g| g.key_carrier(self.at) == enemy.camp_slot)
    }

    pub fn bearer_here(&self) -> Option<String> {
        self.boss_gates
            .as_ref()?
            .carrier_kind(self.at)
            .map(bearer_name)
    }

    pub fn boss_door_shut(&self) -> bool {
        self.boss_gates.is_some() && self.works.key.door == Door::Locked
    }

    pub fn shut_door_side(&self) -> Option<usize> {
        let g = self.boss_gates.as_ref().filter(|_| self.boss_door_shut())?;
        (0..4).find(|&dir| self.dungeon.neighbour(self.at, dir) == Some(g.guardian))
    }

    pub fn key_bearer(&self) -> Option<u32> {
        match self.works.key.hold {
            Hold::Carried { knight, .. } => Some(knight),
            _ => None,
        }
    }

    pub fn key_on_floor(&self) -> Option<(f32, f32)> {
        match self.works.key.hold {
            Hold::Lying { room, x, y } if room == self.at => Some((x, y)),
            _ => None,
        }
    }

    pub fn key_keepers(&self) -> Vec<String> {
        let Some(g) = self.boss_gates.as_ref() else {
            return Vec::new();
        };
        g.leaders
            .iter()
            .filter(|l| l.prerequisite && !l.defeated)
            .map(|l| match g.carrier_kind(l.room) {
                Some(kind) => bearer_name(kind),
                None => g
                    .economy
                    .factions
                    .get(usize::from(l.faction))
                    .map_or("the camp's elite".to_string(), |f| {
                        format!("the elite of {}", f.name)
                    }),
            })
            .collect()
    }

    pub fn guardian_room(&self) -> bool {
        match &self.boss_gates {
            Some(g) => g.guardian == self.at,
            None => matches!(
                self.room().kind,
                RoomKind::Stairs | RoomKind::Threshold | RoomKind::Lair
            ),
        }
    }

    pub fn key_line(&self) -> Option<String> {
        if !self.boss_door_shut() {
            return None;
        }
        Some(match self.works.key.hold {
            Hold::Kept => match self.key_keepers().first() {
                Some(name) => format!("Boss Key: slay {name}"),
                None => "Boss Key: slay its elite".to_string(),
            },
            Hold::Lying { .. } => "Boss Key: take it to the door".to_string(),
            Hold::Carried { knight, .. } => {
                let who = self
                    .players
                    .get(&knight)
                    .map_or_else(|| "a knight".to_string(), |h| h.name.clone());
                format!("Boss Key: {who} carries it")
            }
            Hold::Spent => return None,
        })
    }

    pub(super) fn drop_boss_key(&mut self, x: f32, y: f32) {
        if self.boss_gates.is_none()
            || self.works.key.door == Door::Open
            || self.works.key.hold != Hold::Kept
        {
            return;
        }
        let (x, y) = settle(self.room(), x, y);
        self.works.key.hold = Hold::Lying {
            room: self.at,
            x,
            y,
        };
        self.cues.push("key_drop".into());
        self.sounds.push("chest_open");
        self.found = Some((self.tick, 0, "the Boss Key falls".into()));
    }

    pub(super) fn tick_key(&mut self) {
        if self.boss_gates.is_none() || self.works.key.door == Door::Open {
            return;
        }
        if self.works.key.hold == Hold::Kept
            && self.boss_gates.as_ref().is_some_and(|g| g.unlocked())
            && let Some((x, y)) = self
                .players
                .values()
                .find(|h| h.hp > 0 && !h.stone)
                .map(|h| (h.x, h.y))
        {
            self.drop_boss_key(x, y);
        }
        match self.works.key.hold {
            Hold::Lying { room, x, y } if room == self.at => {
                let taker = self
                    .players
                    .iter()
                    .find(|(_, h)| {
                        h.hp > 0 && !h.stone && (h.x - x).hypot(h.y - y) < KEY_REACH + HERO_RADIUS
                    })
                    .map(|(&id, h)| (id, h.x, h.y));
                if let Some((knight, hx, hy)) = taker {
                    self.works.key.hold = Hold::Carried {
                        knight,
                        x: hx,
                        y: hy,
                    };
                    self.cues.push("key_taken".into());
                    self.sounds.push("spoil_pickup");
                    let name = self.players[&knight].name.clone();
                    self.found = Some((self.tick, knight, format!("{name} takes the Boss Key")));
                }
            }
            Hold::Carried { knight, .. } => match self.players.get(&knight) {
                Some(hero) if hero.hp > 0 && !hero.stone => {
                    self.works.key.hold = Hold::Carried {
                        knight,
                        x: hero.x,
                        y: hero.y,
                    };
                }
                Some(hero) => {
                    let (x, y) = settle(self.room(), hero.x, hero.y);
                    self.works.key.hold = Hold::Lying {
                        room: self.at,
                        x,
                        y,
                    };
                    self.cues.push("key_lost".into());
                    self.sounds.push("spoil_pickup");
                }
                None => {
                    let Hold::Carried { x, y, .. } = self.works.key.hold else {
                        return;
                    };
                    let (x, y) = settle(self.room(), x, y);
                    self.works.key.hold = Hold::Lying {
                        room: self.at,
                        x,
                        y,
                    };
                    self.cues.push("key_lost".into());
                }
            },
            _ => {}
        }
    }

    pub(super) fn boss_door_holds(&mut self, next: usize, dir: usize) -> bool {
        let Some(guardian) = self.boss_gates.as_ref().map(|g| g.guardian) else {
            return false;
        };
        if next != guardian || self.works.key.door == Door::Open {
            return false;
        }
        let (door, _) = hazards::doorway(self.room(), dir);
        let at_door = self
            .key_bearer()
            .and_then(|id| self.players.get(&id))
            .is_some_and(|h| {
                h.hp > 0 && !h.stone && (h.x - door.0).hypot(h.y - door.1) <= DOOR_REACH
            });
        if at_door {
            self.works.key.door = Door::Open;
            self.works.key.hold = Hold::Spent;
            self.cues.push("door_unlocked".into());
            self.sounds.push("door_open");
            self.fx.push(Fx::Shake(8));
            return false;
        }
        let (cx, cy) = (self.room().width() / 2.0, self.room().height() / 2.0);
        let barred = self.barred();
        let at = self.at;
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
            let (ux, uy) = unit(cx - hero.x, cy - hero.y);
            let room = &self.dungeon.rooms[at];
            if hero.x < 1.0
                || hero.y < 1.0
                || hero.x > room.width() - 1.0
                || hero.y > room.height() - 1.0
            {
                let grid = Grid { room, barred };
                let was = (hero.x, hero.y);
                (hero.x, hero.y) = grid.halted(was, (ux * 1.2, uy * 1.2));
            }
        }
        if self.works.key.told == 0 || self.tick.saturating_sub(self.works.key.told) >= REMIND {
            self.works.key.told = self.tick.max(1);
            self.sounds.push("door_seal");
            self.cues.push("door_locked".into());
            if let Some(line) = self.boss_gate_line() {
                self.cues.push(format!("boss_lock:{line}"));
            }
        }
        true
    }
}
