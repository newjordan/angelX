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

/// How near a knight must stand to a key to take it, and to the boss door
/// to open it with the key in hand.
pub const KEY_REACH: f32 = 1.4;
pub const DOOR_REACH: f32 = 5.0;
/// Ticks between two reminders at a locked door.
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
    /// Not yet made: the chiefs still hold the route.
    #[default]
    Kept,
    /// Lying on the floor of `room`.
    Lying { room: usize, x: f32, y: f32 },
    /// In knight `knight`'s keeping, last seen at `(x, y)`.
    Carried { knight: u32, x: f32, y: f32 },
    /// Spent on the door.
    Spent,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub door: Door,
    pub hold: Hold,
    /// The tick a knight last stood at the shut door, so the reminder is
    /// not said every step.
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

/// The nearest open floor to `(x, y)` in `room`, so the key never lies in a
/// wall or over a pit.
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

/// A carrier of the key by its kind, as the gate's lines name it.
pub fn bearer_name(kind: EnemyKind) -> String {
    let name = bestiary::ENTRIES
        .iter()
        .find(|e| e.kind == kind)
        .map_or("monster", |e| e.name);
    format!("the elite {name}")
}

impl boss_gates::Gates {
    /// The camp slot in room `at` that carries the Boss Key: the toughest
    /// of a standing chief's camp (its first of the hardiest kind, of the
    /// camp's own faction), once the camp has mustered.
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

    /// The kind of room `at`'s key carrier, if its camp has mustered.
    pub fn carrier_kind(&self, at: usize) -> Option<EnemyKind> {
        let slot = self.key_carrier(at)?;
        self.economy
            .camp(at)?
            .roster
            .get(usize::from(slot))
            .copied()
    }

    /// Camp slot `slot` of room `at` fell: if it carried the key, its chief's
    /// route is won (the leader's loss, once). Actual corpse settlement is
    /// the only caller.
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
    /// Whether `enemy` carries the Boss Key: the elite at the head of the
    /// chief's camp here, alive, the key not yet fallen.
    pub fn carries_key(&self, enemy: &Enemy) -> bool {
        enemy.hp > 0
            && self.works.key.hold == Hold::Kept
            && enemy.camp_slot.is_some()
            && self
                .boss_gates
                .as_ref()
                .is_some_and(|g| g.key_carrier(self.at) == enemy.camp_slot)
    }

    /// The key carrier in the room the party stands in, by name.
    pub fn bearer_here(&self) -> Option<String> {
        self.boss_gates
            .as_ref()?
            .carrier_kind(self.at)
            .map(bearer_name)
    }

    /// Whether this floor's boss door is shut for want of the key.
    pub fn boss_door_shut(&self) -> bool {
        self.boss_gates.is_some() && self.works.key.door == Door::Locked
    }

    /// The side of this room whose doorway leads to the shut boss door, if
    /// any: the lock is drawn there.
    pub fn shut_door_side(&self) -> Option<usize> {
        let g = self.boss_gates.as_ref().filter(|_| self.boss_door_shut())?;
        (0..4).find(|&dir| self.dungeon.neighbour(self.at, dir) == Some(g.guardian))
    }

    /// Which knight carries the key.
    pub fn key_bearer(&self) -> Option<u32> {
        match self.works.key.hold {
            Hold::Carried { knight, .. } => Some(knight),
            _ => None,
        }
    }

    /// The key lying in the room the party stands in.
    pub fn key_on_floor(&self) -> Option<(f32, f32)> {
        match self.works.key.hold {
            Hold::Lying { room, x, y } if room == self.at => Some((x, y)),
            _ => None,
        }
    }

    /// Who still stands between the party and the key: each standing
    /// chief's elite, by kind once its camp has mustered, else by camp.
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

    /// Whether the room the party stands in is its floor's guardian's, so
    /// a boss here is the floor's own and not a route chief: the gate's
    /// apex room, or (with no gates) the stairs, the threshold and the lair.
    pub fn guardian_room(&self) -> bool {
        match &self.boss_gates {
            Some(g) => g.guardian == self.at,
            None => matches!(
                self.room().kind,
                RoomKind::Stairs | RoomKind::Threshold | RoomKind::Lair
            ),
        }
    }

    /// What the gate line adds about the key.
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

    /// The carrier whose fall opened the route fell at `(x, y)`: it drops
    /// the key there.
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

    /// The key is taken up, dropped by whoever falls, and made good if the
    /// chiefs fell by any road that did not leave it.
    pub(super) fn tick_key(&mut self) {
        if self.boss_gates.is_none() || self.works.key.door == Door::Open {
            return;
        }
        // The route is open but no key was made (a save from before, or a
        // carrier that fell out of sight): it lies at the first knight's feet.
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
                    // Fallen, or turned to stone with their player away: the
                    // key drops at their feet for a knight still standing.
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
                    // Gone home: it lies where they last stood.
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

    /// The party reaches for the doorway `dir` toward `next`. True when the
    /// boss door holds them back. The knight with the key at the door
    /// unlocks it, spends the key, and the way is open.
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
        // Held back: stepped in from the doorway, and told why now and then.
        let (cx, cy) = (self.room().width() / 2.0, self.room().height() / 2.0);
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
            let (ux, uy) = unit(cx - hero.x, cy - hero.y);
            if hero.x < 1.0
                || hero.y < 1.0
                || hero.x > self.dungeon.rooms[self.at].width() - 1.0
                || hero.y > self.dungeon.rooms[self.at].height() - 1.0
            {
                hero.x += ux * 1.2;
                hero.y += uy * 1.2;
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
