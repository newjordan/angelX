//! Party skills shared by two seated knights. H taps relay; mutual holds tether.
use super::*;
use std::collections::BTreeSet;

pub const RANGE: f32 = 16.0;
pub const LINK_HOLD: u8 = 8;
pub const CATCH: f32 = 2.0;
pub const MANA: f32 = 9.0;
pub const REST: u64 = HZ as u64;
pub const COMBO_MAX: u8 = 8;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub shields: Vec<Relay>,
    pub links: Vec<Link>,
    held: BTreeMap<u32, u8>,
    used: BTreeSet<u32>,
    rest: BTreeMap<u32, u64>,
    room: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relay {
    pub pair: [u32; 2],
    pub owner: u32,
    pub target: u32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub combo: u8,
    pub dropped: bool,
    pub age: u32,
    pub travel: f32,
    pub struck: BTreeSet<u32>,
}
impl State {
    pub fn quiet(&self) -> bool {
        self.shields.is_empty()
            && self.links.is_empty()
            && self.held.is_empty()
            && self.used.is_empty()
            && self.rest.is_empty()
    }
    pub(super) fn valid(&self) -> bool {
        self.shields.len() <= MAX_PLAYERS as usize / 2
            && self.links.len() <= MAX_PLAYERS as usize / 2
            && self.held.len() <= MAX_PLAYERS as usize
            && self.used.len() <= MAX_PLAYERS as usize
            && self.rest.len() <= MAX_PLAYERS as usize
            && self.shields.iter().all(|r| {
                r.pair[0] < r.pair[1]
                    && r.pair.contains(&r.owner)
                    && r.pair.contains(&r.target)
                    && r.owner != r.target
                    && r.combo <= COMBO_MAX
                    && r.struck.len() <= 512
                    && [r.x, r.y, r.vx, r.vy, r.travel]
                        .iter()
                        .all(|v| v.is_finite())
            })
            && self.links.iter().all(|l| l.pair[0] < l.pair[1])
    }
}
impl Relay {
    pub fn radius(&self) -> f32 {
        0.6 + self.combo.min(COMBO_MAX) as f32 * 0.07
    }
    pub fn speed(&self) -> f32 {
        skills::SHIELD_SPEED_L[1] + self.combo.min(COMBO_MAX) as f32 * 2.0
    }
    fn drop(&mut self) {
        self.dropped = true;
        self.combo = 0;
        self.age = 0;
        self.vx = 0.0;
        self.vy = 0.0;
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link {
    pub pair: [u32; 2],
    pub taut: bool,
    #[serde(default)]
    pub snap_until: u64,
}
fn pair(a: u32, b: u32) -> [u32; 2] {
    if a < b { [a, b] } else { [b, a] }
}
fn ready(h: &Hero) -> bool {
    h.hp > 0
        && !h.stone
        && h.privy == 0
        && h.pit.stop.is_none()
        && h.tossed == 0
        && !h.frog()
        && h.slashes == 0
        && h.aiming.is_none()
}
fn facing(h: &Hero, other: &Hero) -> bool {
    let (dx, dy) = (other.x - h.x, other.y - h.y);
    let (ax, ay) = unit(h.aim_x, h.aim_y);
    let d = dx.hypot(dy);
    d >= 0.5 && d <= RANGE && (dx * ax + dy * ay) >= d * 0.7
}
pub fn small(e: &Enemy) -> bool {
    !e.is_boss()
        && e.kind != EnemyKind::Dummy
        && e.kind.hp() <= 120
        && e.radius() <= if e.kind == EnemyKind::Slime { 1.1 } else { 0.8 }
}

impl Run {
    pub fn team_available(&self) -> bool {
        self.active()
            && (!self.at_home_now()
                || self.room().kind == RoomKind::Yard
                || self.room().kind.in_world())
    }
    fn team_clear(&self, a: (f32, f32), b: (f32, f32)) -> bool {
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        let n = ((b.0 - a.0).hypot(b.1 - a.1) * 4.0).ceil().max(1.0) as u32;
        (0..=n).all(|i| {
            let t = i as f32 / n as f32;
            grid.clear(
                a.0 + (b.0 - a.0) * t,
                a.1 + (b.1 - a.1) * t,
                0.1,
                Mover::Flier,
            )
        })
    }
    fn team_target(&self, id: u32, willing: Option<&BTreeMap<u32, u8>>) -> Option<u32> {
        let h = self.players.get(&id).filter(|h| ready(h))?;
        self.players
            .iter()
            .filter(|(other, k)| {
                willing.is_none_or(|held| {
                    held.get(other).copied().unwrap_or(0) >= LINK_HOLD && facing(k, h)
                })
            })
            .filter(|(other, k)| **other != id && ready(k) && facing(h, k))
            .filter(|(_, k)| self.team_clear((h.x, h.y), (k.x, k.y)))
            .min_by(|(a, x), (b, y)| {
                (x.x - h.x)
                    .hypot(x.y - h.y)
                    .total_cmp(&(y.x - h.x).hypot(y.y - h.y))
                    .then(a.cmp(b))
            })
            .map(|(&id, _)| id)
    }
    fn relay_send(&self, r: &mut Relay, by: u32) {
        let h = &self.players[&by];
        let (ux, uy) = unit(h.aim_x, h.aim_y);
        r.owner = by;
        r.target = if by == r.pair[0] {
            r.pair[1]
        } else {
            r.pair[0]
        };
        (r.x, r.y) = (h.x, h.y);
        (r.vx, r.vy) = (ux * r.speed(), uy * r.speed());
        r.dropped = false;
        r.age = 0;
        r.travel = 0.0;
        r.struck.clear();
    }
    fn team_strike(
        &mut self,
        by: u32,
        a: (f32, f32),
        b: (f32, f32),
        radius: f32,
        combo: u8,
        lightning: bool,
        taut: bool,
        struck: &mut BTreeSet<u32>,
    ) {
        for e in self
            .enemies
            .iter_mut()
            .filter(|e| e.hp > 0 && e.kind != EnemyKind::Dummy)
        {
            if struck.contains(&e.id)
                || !hit_segment(a.0, a.1, b.0, b.1, e.x, e.y, e.radius() + radius)
            {
                continue;
            }
            struck.insert(e.id);
            let little = small(e);
            let damage = if little {
                e.hp
            } else if lightning {
                if taut { 5 } else { 2 }
            } else {
                4 + combo.min(COMBO_MAX) as u32
            };
            let damage = damage.min(e.hp);
            e.hp -= damage;
            e.wits.foe = by;
            // Large foes cannot be held permanently by the channel.
            if !lightning {
                e.frozen = e.frozen.max(if e.is_boss() {
                    3
                } else {
                    9 + combo.min(COMBO_MAX) as u32 * 2
                });
            }
            self.feel.credit(by, e.id, damage, self.tick);
            if e.hp == 0 {
                self.feel.blow(
                    e.id,
                    by,
                    gore::Blow::Team {
                        dir: unit(b.0 - a.0, b.1 - a.1),
                        halve: lightning,
                    },
                );
            }
            if !lightning {
                self.sounds.push("shield_block");
            }
        }
    }
    pub(super) fn tick_team(&mut self, inputs: &BTreeMap<u32, Input>) {
        let room = ((self.dungeon.depth as u64) << 32) | self.at as u64;
        if self.team.room != Some(room) || !self.team_available() {
            self.team = State {
                room: Some(room),
                ..State::default()
            };
            if !self.team_available() {
                return;
            }
        }
        let mut state = std::mem::take(&mut self.team);
        state
            .rest
            .retain(|id, until| self.players.contains_key(id) && *until > self.tick);
        let mut presses = BTreeSet::new();
        let mut taps = Vec::new();
        for (&id, h) in &self.players {
            let on = ready(h) && inputs.get(&id).is_some_and(|i| i.valid() && i.team);
            let was = state.held.get(&id).copied().unwrap_or(0);
            if on {
                if was == 0 {
                    presses.insert(id);
                }
                state.held.insert(id, was.saturating_add(1));
            } else {
                if was > 0 && was < LINK_HOLD && !state.used.contains(&id) && ready(h) {
                    taps.push(id);
                }
                state.held.remove(&id);
                state.used.remove(&id);
            }
        }
        state.held.retain(|id, _| self.players.contains_key(id));
        state.used.retain(|id| self.players.contains_key(id));
        let mut busy = BTreeSet::new();
        state.links.retain(|link| {
            let [a, b] = link.pair;
            let valid = self
                .players
                .get(&a)
                .zip(self.players.get(&b))
                .is_some_and(|(h, k)| {
                    ready(h)
                        && ready(k)
                        && state.held.contains_key(&a)
                        && state.held.contains_key(&b)
                        && h.mana >= MANA * DT
                        && k.mana >= MANA * DT
                        && (h.x - k.x).hypot(h.y - k.y) <= RANGE
                        && self.team_clear((h.x, h.y), (k.x, k.y))
                });
            if valid {
                busy.extend([a, b]);
            } else {
                state.rest.insert(a, self.tick + REST);
                state.rest.insert(b, self.tick + REST);
                self.sounds.push("clang");
            }
            valid
        });
        for (&id, &held) in &state.held {
            if held < LINK_HOLD || busy.contains(&id) || state.rest.contains_key(&id) {
                continue;
            }
            let Some(other) = self.team_target(id, Some(&state.held)) else {
                continue;
            };
            if other < id
                || busy.contains(&other)
                || state.rest.contains_key(&other)
                || state.held.get(&other).copied().unwrap_or(0) < LINK_HOLD
                || !facing(&self.players[&other], &self.players[&id])
                || self.players[&id].mana < MANA * DT
                || self.players[&other].mana < MANA * DT
            {
                continue;
            }
            state.links.push(Link {
                pair: pair(id, other),
                taut: false,
                snap_until: 0,
            });
            busy.extend([id, other]);
            state.used.extend([id, other]);
            self.sounds.push("shot_spark");
        }
        for link in &mut state.links {
            let [a, b] = link.pair;
            let h = &self.players[&a];
            let k = &self.players[&b];
            let (from, to) = ((h.x, h.y), (k.x, k.y));
            let taut = (h.x - k.x).hypot(h.y - k.y) >= RANGE * 0.85;
            let snap = taut && !link.taut && self.tick >= link.snap_until;
            if snap {
                link.snap_until = self.tick + REST;
            }
            if self.tick % 6 == 0 || snap {
                self.team_strike(a, from, to, 0.18, 0, true, snap, &mut BTreeSet::new());
                if snap {
                    self.sounds.push("hit_bolt");
                }
            }
            link.taut = taut;
            for id in [a, b] {
                let h = self.players.get_mut(&id).unwrap();
                h.mana = (h.mana - MANA * DT).max(0.0);
                h.mana_rest = 0;
            }
        }
        state.shields.retain(|r| {
            r.pair
                .iter()
                .all(|id| self.players.get(id).is_some_and(ready))
                && r.age < 20 * HZ
        });
        let mut shield_owners: BTreeSet<_> = state.shields.iter().flat_map(|r| r.pair).collect();
        for r in &mut state.shields {
            r.age += 1;
            if r.dropped {
                if let Some(&id) = presses.iter().find(|&&id| {
                    self.players.get(&id).is_some_and(|h| {
                        ready(h)
                            && !busy.contains(&id)
                            && (!shield_owners.contains(&id) || r.pair.contains(&id))
                            && (h.x - r.x).hypot(h.y - r.y) <= CATCH
                    })
                }) {
                    for id in r.pair {
                        shield_owners.remove(&id);
                    }
                    let other = if id == r.pair[0] {
                        r.pair[1]
                    } else {
                        r.pair[0]
                    };
                    r.pair = pair(id, other);
                    shield_owners.extend(r.pair);
                    self.relay_send(r, id);
                    state.used.insert(id);
                    self.sounds.push("swing");
                }
                continue;
            }
            let from = (r.x, r.y);
            let to = (r.x + r.vx * DT, r.y + r.vy * DT);
            if !self.team_clear(from, to) {
                r.drop();
                self.sounds.push("clang");
                continue;
            }
            self.team_strike(
                r.owner,
                from,
                to,
                r.radius(),
                r.combo,
                false,
                false,
                &mut r.struck,
            );
            let h = &self.players[&r.target];
            let arrives = hit_segment(from.0, from.1, to.0, to.1, h.x, h.y, CATCH);
            if arrives && presses.contains(&r.target) && !busy.contains(&r.target) {
                r.combo = r.combo.saturating_add(1).min(COMBO_MAX);
                state.used.insert(r.target);
                self.relay_send(r, r.target);
                self.sounds.push("shield_block");
                continue;
            }
            (r.x, r.y) = to;
            r.travel += r.speed() * DT;
            let past = (r.x - h.x) * r.vx + (r.y - h.y) * r.vy > CATCH * r.speed();
            if past || r.travel >= RANGE || r.age >= 2 * HZ {
                r.drop();
                self.sounds.push("clang");
            }
        }
        for id in taps {
            if busy.contains(&id)
                || state.shields.iter().any(|r| r.pair.contains(&id))
                || state.rest.contains_key(&id)
            {
                continue;
            }
            let Some(other) = self.team_target(id, None) else {
                continue;
            };
            if state.shields.iter().any(|r| r.pair.contains(&other)) || busy.contains(&other) {
                continue;
            }
            let mut r = Relay {
                pair: pair(id, other),
                owner: id,
                target: other,
                x: 0.0,
                y: 0.0,
                vx: 0.0,
                vy: 0.0,
                combo: 0,
                dropped: false,
                age: 0,
                travel: 0.0,
                struck: BTreeSet::new(),
            };
            self.relay_send(&mut r, id);
            let h = &self.players[&id];
            let k = &self.players[&other];
            let (ux, uy) = unit(k.x - h.x, k.y - h.y);
            (r.vx, r.vy) = (ux * r.speed(), uy * r.speed());
            state.shields.push(r);
            state.rest.insert(id, self.tick + REST);
            self.sounds.push("swing");
        }
        self.team = state;
    }
}
