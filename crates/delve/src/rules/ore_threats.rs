//! Finite, ore-specific sentries reveal a seam's danger before its guardian.
//! Native enemy state machines own shields, kegs, reflection and burning rushes.
use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OreKind {
    Iron,
    Coal,
    Crystal,
    Ember,
}
impl OreKind {
    pub const ALL: [Self; 4] = [Self::Iron, Self::Coal, Self::Crystal, Self::Ember];
    /// The four ores a land charter asks for, and the sentries each has.
    pub fn legacy(resource: &str) -> Option<Self> {
        Some(match resource {
            "iron-ore" => Self::Iron,
            "coal" => Self::Coal,
            "crystal" => Self::Crystal,
            "ember" => Self::Ember,
            _ => return None,
        })
    }
    /// The sentries that hold a seam whose claim is of `resource`: an ore,
    /// or the quarry stone of its pack (`geology`).
    pub fn for_resource(resource: &str) -> Option<Self> {
        super::geology::guard_for(resource)
    }
    pub fn resource(self) -> &'static str {
        match self {
            Self::Iron => "iron-ore",
            Self::Coal => "coal",
            Self::Crystal => "crystal",
            Self::Ember => "ember",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Iron => "Ironplate sentinel",
            Self::Coal => "Blast crawler",
            Self::Crystal => "Prism stalker",
            Self::Ember => "Cinderhound",
        }
    }
    pub fn warning(self) -> &'static str {
        match self {
            Self::Iron => "Iron plates turn frontal shots. A raised shield warns of a heavy bash.",
            Self::Coal => {
                "Coal crawlers plant blinking kegs and lob bombs at marked landing circles."
            }
            Self::Crystal => "Prism stalkers flash, then return a copy of your last shot.",
            Self::Ember => "Cinderhounds mark a charge lane, then rush and leave burning embers.",
        }
    }
    pub fn counter(self) -> &'static str {
        match self {
            Self::Iron => {
                "Flank or break the plate. Punish its shield-down pause."
            }
            Self::Coal => {
                "Leave fuse and landing circles. Shoot planted kegs from range."
            }
            Self::Crystal => {
                "Sidestep after firing, or block the copied shot with a barrier."
            }
            Self::Ember => {
                "Sidestep the locked lane. Avoid fire; punish its recovery."
            }
        }
    }
    pub fn kind(self) -> EnemyKind {
        match self {
            Self::Iron => EnemyKind::Pavise,
            Self::Coal => EnemyKind::Sapper,
            Self::Crystal => EnemyKind::Mirrorling,
            Self::Ember => EnemyKind::Hound,
        }
    }
    pub fn asset(self) -> &'static str {
        match self {
            Self::Iron => "iron-ore-sentinel",
            Self::Coal => "coal-blast-crawler",
            Self::Crystal => "crystal-prism-stalker",
            Self::Ember => "ember-cinderhound",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sentry {
    pub id: u32,
    pub defeated: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Encounter {
    pub seed: u64,
    pub raid: u64,
    pub depth: u32,
    pub room: usize,
    pub kind: OreKind,
    pub sentries: Vec<Sentry>,
}
impl Encounter {
    pub fn valid(&self) -> bool {
        self.depth > 0
            && self.depth <= DEEPEST
            && self.room < 512
            && self.sentries.len() == 2
            && self.sentries.iter().all(|s| s.id > 0)
            && self.sentries[0].id != self.sentries[1].id
    }
    fn here(&self, run: &Run) -> bool {
        self.seed == run.seed
            && self.raid == run.raid_id
            && self.depth == run.dungeon.depth
            && self.room == run.at
    }
}
impl Run {
    pub fn ore_sentry_kind(&self, id: u32) -> Option<OreKind> {
        self.kingdom
            .ore_sentries
            .values()
            .find(|e| e.sentries.iter().any(|s| s.id == id))
            .map(|e| e.kind)
    }
    pub fn ore_sentries_defeated(&self, claim: &str) -> bool {
        // Only an already-challenged legacy guardian can lack this roster.
        // A fresh crowded seam must wait for all sentries to be admitted.
        self.kingdom
            .ore_sentries
            .get(claim)
            .map_or_else(
                || self.kingdom.claim_bosses.contains_key(claim),
                |e| e.sentries.iter().all(|s| s.defeated),
            )
    }
    pub(super) fn admitted_ore_sentry(&self, enemy: &Enemy) -> bool {
        enemy.boss.is_none()
            && enemy.camp_slot.is_none()
            && enemy.faction.is_none()
            && self.kingdom.ore_sentries.iter().any(|(id, e)| {
                e.here(self)
                    && e.kind.kind() == enemy.kind
                    && e.sentries.iter().any(|s| s.id == enemy.id && !s.defeated)
                    && self
                        .home
                        .industry
                        .claims
                        .get(id)
                        .is_some_and(|c| {
                            (!c.secured || self.kingdom.stir.raiding(id))
                                && super::geology::guards_for(&c.resource).contains(&e.kind)
                        })
            })
    }
    pub(super) fn tick_ore_sentries(&mut self, deposit: &industry::Deposit) {
        if !self.active()
            || self.dungeon.depth == 0
            || (self.kingdom.claim_bosses.contains_key(&deposit.id)
                && !self.kingdom.stir.raiding(&deposit.id))
        {
            return;
        }
        let kind = deposit.seam.guard();
        // A claim's sentries stand until it is won, and again while a raid
        // of the Deep is on (`stir`).
        if !self
            .home
            .industry
            .claims
            .get(&deposit.id)
            .is_some_and(|c| !c.secured || self.kingdom.stir.raiding(&deposit.id))
        {
            return;
        }
        let existing = self.kingdom.ore_sentries.get(&deposit.id).cloned();
        let missing: Vec<usize> = if let Some(e) = &existing {
            e.sentries
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    !s.defeated
                        && !self
                            .enemies
                            .iter()
                            .chain(&self.parked)
                            .any(|b| b.id == s.id && b.hp > 0)
                })
                .map(|(i, _)| i)
                .collect()
        } else {
            vec![0, 1]
        };
        if missing.is_empty()
            || (existing.is_none()
                && self.kingdom.ore_sentries.len() >= claim_bosses::MAX_ENCOUNTERS)
            || self.enemies.len() + missing.len() > boss_ecology::ENTITY_CAP
            || self.next_id > u32::MAX - missing.len() as u32
        {
            return;
        }
        // Select all spots before mutation. A crowded seam waits; it never
        // places an unseen body inside a knight or grants a partial victory.
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        let mut spots = Vec::new();
        for y in 2..self.room().rows as i32 - 2 {
            for x in 2..self.room().cols as i32 - 2 {
                let (x, y) = ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.5) * TILE_UNITS);
                if (x - deposit.x).hypot(y - deposit.y) > 10.0
                    || !grid.clear(x, y, kind.kind().radius(), Mover::Walker)
                    || self
                        .players
                        .values()
                        .filter(|h| h.hp > 0 && !h.absent)
                        .any(|h| (h.x - x).hypot(h.y - y) < 6.0)
                    || self.enemies.iter().any(|e| (e.x - x).hypot(e.y - y) < 2.5)
                {
                    continue;
                }
                spots.push((x, y));
            }
        }
        spots.sort_by(|a, b| {
            ((a.0 - deposit.x).hypot(a.1 - deposit.y))
                .total_cmp(&((b.0 - deposit.x).hypot(b.1 - deposit.y)))
        });
        let mut chosen: Vec<(f32, f32)> = Vec::new();
        for p in spots {
            if chosen.iter().all(|&(x, y)| (p.0 - x).hypot(p.1 - y) > 3.0) {
                chosen.push(p);
            }
            if chosen.len() == missing.len() {
                break;
            }
        }
        if chosen.len() != missing.len() {
            return;
        }
        let home = self
            .open_here()
            .and_then(|o| {
                o.chambers
                    .iter()
                    .position(|c| c.holds(deposit.x, deposit.y, 0.0))
            })
            .map(|i| i as u8);
        let fresh = existing.is_none();
        let mut encounter = existing.unwrap_or(Encounter {
            seed: self.seed,
            raid: self.raid_id,
            depth: self.dungeon.depth,
            room: self.at,
            kind,
            sentries: vec![
                Sentry {
                    id: 0,
                    defeated: false
                };
                2
            ],
        });
        for (index, (x, y)) in missing.into_iter().zip(chosen) {
            let id = self
                .spawn_body(kind.kind(), x, y)
                .expect("ore sentry admission preflight");
            let e = self.enemies.last_mut().unwrap();
            e.home = home;
            e.frozen = HZ / 2;
            encounter.sentries[index] = Sentry {
                id,
                defeated: false,
            };
        }
        self.kingdom
            .ore_sentries
            .insert(deposit.id.clone(), encounter);
        if fresh {
            self.notices.push(format!(
                "{}: {} {}",
                kind.name(),
                kind.warning(),
                kind.counter()
            ));
        }
    }
    pub(super) fn tick_ore_pressure(&mut self) {
        let active: BTreeSet<_> = self
            .enemies
            .iter()
            .filter(|e| {
                e.hp > 0 && e.age >= TELEGRAPH && e.frozen == 0 && self.admitted_ore_sentry(e)
            })
            .map(|e| e.id)
            .collect();
        for e in self.enemies.iter_mut().filter(|e| active.contains(&e.id)) {
            // A prism must finish its tell even when the knight keeps firing.
            if e.kind == EnemyKind::Mirrorling
                && e.stage & 7 != 0
                && e.stage & 8 == 0
                && e.timer >= 90
            {
                e.stage |= 8;
                e.timer = 0;
            }
            // Native marked lobs make the crawler a threat to stationary
            // shooters too. The existing landing circle lasts a full second.
            if e.kind == EnemyKind::Sapper && e.age % 150 == 90 && self.lobs.len() < 8 {
                if let Some(h) = self
                    .players
                    .values()
                    .filter(|h| h.targetable() && !h.absent && (h.x - e.x).hypot(h.y - e.y) < 18.0)
                    .min_by(|a, b| {
                        ((a.x - e.x).hypot(a.y - e.y)).total_cmp(&((b.x - e.x).hypot(b.y - e.y)))
                    })
                {
                    self.lobs.push(foes::Lob {
                        from: (e.x, e.y),
                        x: h.x,
                        y: h.y,
                        fall: foes::LOB_FALL,
                    });
                }
            }
        }
    }
    pub(super) fn settle_ore_sentries(&mut self) -> BTreeSet<u32> {
        let fallen: BTreeSet<_> = self
            .enemies
            .iter()
            .filter(|e| e.hp == 0 && self.admitted_ore_sentry(e))
            .map(|e| e.id)
            .collect();
        for encounter in self.kingdom.ore_sentries.values_mut() {
            for sentry in &mut encounter.sentries {
                if fallen.contains(&sentry.id) {
                    sentry.defeated = true;
                }
            }
        }
        fallen
    }
}

#[cfg(test)]
#[path = "../../tests/rules/ore_threats__tests.rs"]
pub(crate) mod tests;
