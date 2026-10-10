//! Floor-local access, immutable encounter identity and authoritative death
//! evidence. Population/resource succession lives in boss_ecology, not here.
use super::{EnemyKind, Floor, RoomKind, Run, boss_ecology::Economy};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Access {
    Any,
    All,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Leader {
    pub room: usize,
    pub boss: u8,
    pub faction: u8,
    pub prerequisite: bool,
    pub defeated: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Location {
    pub cell: (i32, i32),
    pub doors: [bool; 4],
    pub kind: RoomKind,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Gates {
    pub depth: u32,
    pub(super) geography: Vec<Location>,
    pub guardian: usize,
    pub guardian_boss: Option<u8>,
    pub guardian_defeated: bool,
    pub(super) optional_pit: Option<usize>,
    pub(super) pit_defeated: bool,
    pub(super) access: Access,
    pub(super) leaders: Vec<Leader>,
    pub(super) economy: Economy,
}
impl Gates {
    pub(super) fn within_bounds(&self, rooms: usize) -> bool {
        rooms <= 64
            && self.geography.len() == rooms
            && self.guardian < rooms
            && !self.leaders.is_empty()
            && self.leaders.len() <= 4
            && self.optional_pit.is_none_or(|r| r < rooms)
            && self.economy.within_bounds()
            && self.leaders.iter().all(|l| l.room < rooms && l.faction < 4)
            && self.economy.camps.iter().all(|c| c.room < rooms)
            && self.economy.profile.sites.iter().all(|s| s.room < rooms)
            && self.economy.events.iter().all(|e| match *e {
                super::boss_ecology::Event::Emit { room, .. } => room < rooms,
                _ => true,
            })
    }

    pub(super) fn same_encounters(&self, other: &Self) -> bool {
        (self.depth, self.guardian, self.guardian_boss, self.access)
            == (
                other.depth,
                other.guardian,
                other.guardian_boss,
                other.access,
            )
            && self.leaders.len() == other.leaders.len()
            && self.leaders.iter().zip(&other.leaders).all(|(a, b)| {
                (a.room, a.boss, a.faction, a.prerequisite)
                    == (b.room, b.boss, b.faction, b.prerequisite)
            })
            && self.optional_pit == other.optional_pit
            && self.geography == other.geography
            && self.economy.same_identity(&other.economy)
    }
    /// Mirrors may skip host ticks but cannot rewind deaths, deployment or reserves.
    pub(super) fn permits_progress(&self, other: &Self) -> bool {
        self.same_encounters(other)
            && (!self.guardian_defeated || other.guardian_defeated)
            && (!self.pit_defeated || other.pit_defeated)
            && self
                .leaders
                .iter()
                .zip(&other.leaders)
                .all(|(a, b)| !a.defeated || b.defeated)
            && other.economy.events.starts_with(&self.economy.events)
    }
    pub(super) fn unlocked(&self) -> bool {
        let required = self.leaders.iter().filter(|l| l.prerequisite);
        match self.access {
            Access::Any => required.clone().any(|l| l.defeated),
            Access::All => required.clone().count() > 0 && required.clone().all(|l| l.defeated),
        }
    }
    pub(super) fn leader(&self, room: usize) -> Option<&Leader> {
        self.leaders.iter().find(|l| l.room == room)
    }
    pub(super) fn encounter(&self, at: usize, kind: EnemyKind, boss: Option<u8>) -> bool {
        if kind == EnemyKind::PitTyrant && self.optional_pit == Some(at) {
            return true;
        }
        if kind == EnemyKind::Boss {
            self.leaders
                .iter()
                .any(|l| l.room == at && Some(l.boss) == boss)
                || (at == self.guardian
                    && self.guardian_boss.is_some()
                    && boss == self.guardian_boss)
        } else {
            kind == EnemyKind::Dragon && self.depth == super::FLOORS && at == self.guardian
        }
    }
    pub(super) fn already_defeated(&self, at: usize, kind: EnemyKind, boss: Option<u8>) -> bool {
        self.encounter(at, kind, boss)
            && if kind == EnemyKind::PitTyrant {
                self.pit_defeated
            } else if at == self.guardian {
                self.guardian_defeated
            } else {
                self.leader(at).is_some_and(|l| l.defeated)
            }
    }
    pub(super) fn record(&mut self, at: usize, kind: EnemyKind, boss: Option<u8>) -> bool {
        if !self.encounter(at, kind, boss) || self.already_defeated(at, kind, boss) {
            return false;
        }
        if kind == EnemyKind::PitTyrant {
            self.pit_defeated = true;
        } else if at == self.guardian {
            if !self.unlocked() {
                return false;
            }
            self.guardian_defeated = true;
        } else {
            let i = self.leaders.iter().position(|l| l.room == at).unwrap();
            self.leaders[i].defeated = true;
            self.economy.lose(i as u8);
        }
        true
    }
}

fn neighbour(floor: &Floor, at: usize, dir: usize) -> Option<usize> {
    let room = floor.rooms.get(at)?;
    if !room.doors[dir] {
        return None;
    }
    let (dx, dy) = super::layout::DIRS[dir];
    let cell = (room.cell.0.checked_add(dx)?, room.cell.1.checked_add(dy)?);
    floor
        .rooms
        .iter()
        .position(|r| r.cell == cell && r.doors[(dir + 2) % 4])
}
pub(super) fn reachable(floor: &Floor, sealed: usize) -> Vec<bool> {
    let mut seen = vec![false; floor.rooms.len()];
    if seen.is_empty() || sealed == 0 {
        return seen;
    }
    seen[0] = true;
    let mut queue = VecDeque::from([0]);
    while let Some(at) = queue.pop_front() {
        for dir in 0..4 {
            if let Some(next) = neighbour(floor, at, dir)
                && next != sealed
                && !seen[next]
            {
                seen[next] = true;
                queue.push_back(next);
            }
        }
    }
    seen
}

impl Run {
    pub(super) fn populate_boss_gates(&mut self) {
        self.boss_gates = super::boss_population::populate(
            &self.dungeon,
            &mut self.bosses,
            self.seed,
            self.raid_id,
            self.mode,
        );
    }
    pub(super) fn guardian_locked(&self) -> bool {
        self.boss_gates
            .as_ref()
            .is_some_and(|g| g.guardian == self.at && !g.unlocked())
    }
    pub(super) fn boss_room_can_clear(&self) -> bool {
        self.boss_gates.as_ref().is_none_or(|g| {
            if self.at == g.guardian {
                g.unlocked()
                    && (g.guardian_defeated
                        || (g.guardian_boss.is_none() && g.depth != super::FLOORS))
            } else if g.optional_pit == Some(self.at) {
                g.pit_defeated
            } else {
                g.leader(self.at).is_none_or(|l| l.defeated)
            }
        })
    }
    pub(super) fn show_boss_lock(&mut self) {
        if let Some(line) = self.boss_gate_line() {
            self.cues.push(format!("boss_lock:{line}"));
        }
    }
    pub(crate) fn boss_gate_line(&self) -> Option<String> {
        let g = self.boss_gates.as_ref()?;
        let n = g.leaders.iter().filter(|l| l.prerequisite).count();
        let done = g
            .leaders
            .iter()
            .filter(|l| l.prerequisite && l.defeated)
            .count();
        let route = match g.access {
            Access::Any => "ANY",
            Access::All => "ALL",
        };
        let access = if self.guardian_locked() {
            format!("sealed {route} {done}/{n}; retreat")
        } else {
            format!(
                "{route} {done}/{n} {}",
                if g.unlocked() {
                    "route open"
                } else {
                    "resource leaders"
                }
            )
        };
        let leader = g
            .leader(self.at)
            .map(|l| {
                format!(
                    " | {} ({})",
                    self.bosses[usize::from(l.boss)].name,
                    if l.prerequisite {
                        "route chief"
                    } else {
                        "optional chief"
                    }
                )
            })
            .unwrap_or_default();
        Some(format!(
            "{}: {}{}",
            g.economy.profile.name(),
            access,
            leader
        ))
    }
    /// Short, separate projection keeps faction/resource and aftermath readable
    /// on compact terminals and browser guests, not lost at the end of a notice.
    pub(crate) fn boss_support_line(&self) -> Option<String> {
        let g = self.boss_gates.as_ref()?;
        Some(g.economy.line(self.at))
    }
    pub(crate) fn boss_gate_state(&self) -> Option<Gates> {
        self.boss_gates.clone()
    }
    pub(crate) fn boss_leader_rooms(&self) -> impl Iterator<Item = usize> + '_ {
        self.boss_gates
            .iter()
            .flat_map(|g| g.leaders.iter().filter(|l| !l.defeated).map(|l| l.room))
    }
    pub(super) fn valid_boss_gates(&self) -> bool {
        let Some(g) = &self.boss_gates else {
            return true;
        }; // absent legacy state stays active
        if !g.within_bounds(self.dungeon.rooms.len())
            || !self.dungeon.valid_secret()
            || g.depth != self.dungeon.depth
            || !(1..=super::DEEPEST).contains(&g.depth)
            || g.guardian >= self.dungeon.rooms.len()
            || g.leaders.is_empty()
            || g.leaders.len() > 4
            || self.enemies.len() > super::boss_ecology::ENTITY_CAP
        {
            return false;
        }
        if self
            .dungeon
            .rooms
            .iter()
            .any(|r| !(-16..=16).contains(&r.cell.0) || !(-16..=16).contains(&r.cell.1))
        {
            return false;
        }
        let expected_kind = if g.depth == super::FLOORS {
            RoomKind::Lair
        } else if g.depth == super::DEEPEST {
            RoomKind::Threshold
        } else {
            RoomKind::Stairs
        };
        let guardian = &self.dungeon.rooms[g.guardian];
        if guardian.kind != expected_kind
            && !(g.depth == 1
                && guardian.kind == RoomKind::Sanctuary
                && guardian.cleared
                && g.unlocked())
        {
            return false;
        }
        // Reconstruct genesis independently of mutable progress. Geometry, pack,
        // roster identities and initial budgets cannot be forged by a host delta.
        if g.geography.len() != self.dungeon.rooms.len() {
            return false;
        }
        let mut floor = self.dungeon.clone();
        floor.rooms[g.guardian].kind = expected_kind;
        for (i, (room, identity)) in floor.rooms.iter_mut().zip(&g.geography).enumerate() {
            if room.cell != identity.cell || room.kind != identity.kind {
                return false;
            }
            let mut doors = room.doors;
            if let Some(secret) = floor.secret.filter(|s| s.found && s.host == i) {
                // Bombing the one declared secret wall is not a new political edge.
                if secret.side >= 4 {
                    return false;
                }
                doors[secret.side] = false;
            }
            if doors != identity.doors {
                return false;
            }
            room.doors = identity.doors;
        }
        let mut catalog = self.bosses.clone();
        let Some(genesis) = super::boss_population::populate(
            &floor,
            &mut catalog,
            self.seed,
            self.raid_id,
            self.mode,
        ) else {
            return false;
        };
        if catalog != self.bosses
            || !g.same_encounters(&genesis)
            || !g.economy.valid_progress(&genesis.economy, &g.leaders)
        {
            return false;
        }
        let seen = reachable(&floor, g.guardian);
        if !(0..4).any(|dir| neighbour(&floor, g.guardian, dir).is_some_and(|i| seen[i])) {
            return false;
        }
        for l in &g.leaders {
            let Some(room) = self.dungeon.rooms.get(l.room) else {
                return false;
            };
            if l.room == g.guardian
                || !seen[l.room]
                || room.kind != RoomKind::Fight
                || (room.cleared && !l.defeated)
                || (l.defeated && !room.visited)
            {
                return false;
            }
        }
        for c in &g.economy.camps {
            let room = &self.dungeon.rooms[c.room];
            if (c.deployed && !room.visited) || (room.cleared && (!c.deployed || c.remaining != 0))
            {
                return false;
            }
        }
        if g.pit_defeated
            && g.optional_pit
                .is_none_or(|room| !self.dungeon.rooms[room].visited)
        {
            return false;
        }
        let tyrants = self
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::PitTyrant)
            .count();
        if tyrants
            != usize::from(
                g.optional_pit == Some(self.at) && !g.pit_defeated && !self.room().cleared,
            )
        {
            return false;
        }
        if self.light.is_some_and(|(x, y)| {
            g.depth != super::FLOORS
                || self.at != g.guardian
                || !guardian.cleared
                || !g.guardian_defeated
                || x != guardian.width() / 2.0
                || y != guardian.height() / 2.0 - 4.0
        }) {
            return false;
        }
        if self.phase == super::Phase::Won
            && !(self.at == g.guardian
                && (g.depth == super::DEEPEST
                    || (g.depth == super::FLOORS && self.light.is_some()))
                && guardian.cleared
                && g.guardian_defeated)
        {
            return false;
        }
        for (i, e) in self.enemies.iter().enumerate() {
            if e.id == 0 || e.id > self.next_id {
                return false;
            }
            if let Some(slot) = e.camp_slot {
                if g.economy.emission(e.id).is_some()
                    || !g.economy.alive_slot(self.at, slot, e.kind)
                    || e.faction != g.economy.slot_faction(self.at, slot)
                    || self.enemies[..i].iter().any(|p| p.camp_slot == Some(slot))
                {
                    return false;
                }
            } else if let Some(f) = e.faction {
                let expected = g
                    .leader(self.at)
                    .map(|l| l.faction)
                    .or_else(|| (self.at == g.guardian).then_some(0));
                if e.kind != EnemyKind::Boss && e.kind != EnemyKind::Dragon || Some(f) != expected {
                    return false;
                }
            }
            if e.camp_slot.is_none()
                && !g.encounter(self.at, e.kind, e.boss)
                && !g.economy.alive_emission(self.at, e.id, e.kind, e.stage)
            {
                return false;
            }
        }
        for event in &g.economy.events {
            if let super::boss_ecology::Event::Emit {
                room,
                id,
                kind,
                stage,
            } = *event
            {
                if id > self.next_id || !self.dungeon.rooms[room].visited {
                    return false;
                }
                if g.economy.alive_emission(room, id, kind, stage)
                    && (room != self.at
                        || !self.enemies.iter().any(|e| {
                            e.id == id
                                && e.kind == kind
                                && (kind != EnemyKind::Slime || e.stage == stage)
                                && e.camp_slot.is_none()
                        }))
                {
                    return false;
                }
            }
        }
        if let Some(c) = g.economy.camp(self.at) {
            for (slot, _) in c.roster.iter().enumerate() {
                let alive = c.remaining & (1u32 << slot) != 0;
                if self
                    .enemies
                    .iter()
                    .filter(|e| e.camp_slot == Some(slot as u8))
                    .count()
                    != usize::from(alive)
                {
                    return false;
                }
            }
            if c.deployed && self.room().cleared && c.remaining != 0 {
                return false;
            }
        }
        if (g.guardian_defeated
            && (!g.unlocked()
                || !guardian.visited
                || (g.guardian_boss.is_none() && expected_kind != RoomKind::Lair)))
            || (guardian.cleared
                && (!g.unlocked()
                    || ((g.guardian_boss.is_some() || expected_kind == RoomKind::Lair)
                        && !g.guardian_defeated)))
        {
            return false;
        }
        let expected = g
            .leader(self.at)
            .filter(|l| !l.defeated)
            .map(|l| l.boss)
            .or_else(|| {
                (self.at == g.guardian && g.unlocked() && !g.guardian_defeated)
                    .then_some(g.guardian_boss)
                    .flatten()
            });
        let bosses: Vec<_> = self
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Boss)
            .collect();
        let optional_pit = self.room().kind == RoomKind::Pit; // native Pit Tyrant is not a graph boss
        if !optional_pit
            && (bosses.len() != usize::from(expected.is_some())
                || bosses.iter().any(|e| e.boss != expected))
        {
            return false;
        }
        let dragons = self
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Dragon)
            .count();
        let dragon_expected = self.at == g.guardian
            && expected_kind == RoomKind::Lair
            && g.unlocked()
            && !g.guardian_defeated;
        if dragons != usize::from(dragon_expected) {
            return false;
        }
        let deployed = g.economy.camp(self.at).is_some_and(|c| c.deployed);
        if g.economy.camp(self.at).is_some()
            && !self.guardian_locked()
            && !self.room().cleared
            && !deployed
        {
            return false;
        }
        if self.guardian_locked()
            && (guardian.cleared
                || !self.enemies.is_empty()
                || self.waves.pending()
                || self.projectiles.iter().any(|p| p.hostile)
                || !self.traps.is_empty()
                || !self.rocks.is_empty()
                || self.light.is_some()
                || !guardian.items.is_empty()
                || guardian.chest.is_some()
                || self.phase != super::Phase::Exploring)
        {
            return false;
        }
        true
    }
}
