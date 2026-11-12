//! Optional resource guardians. A claim is won by a settled corpse, never by
//! an empty enemy vector. The existing boss engine owns attacks and damage.
use super::*;
use std::collections::BTreeSet;

pub const MAX_ENCOUNTERS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Guardian {
    Ironroot,
    Cindermaw,
    Amethyst,
}

impl Guardian {
    pub const ALL: [Self; 3] = [Self::Ironroot, Self::Cindermaw, Self::Amethyst];
    pub fn for_resource(resource: &str) -> Option<Self> {
        Some(match resource {
            "iron-ore" => Self::Ironroot,
            "coal" | "ember" => Self::Cindermaw,
            "crystal" => Self::Amethyst,
            _ => return None,
        })
    }
    pub fn for_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.id() == id)
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Ironroot => "claim-ironroot",
            Self::Cindermaw => "claim-cindermaw",
            Self::Amethyst => "claim-amethyst",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Ironroot => "IRONROOT COLOSSUS",
            Self::Cindermaw => "CINDERMAW",
            Self::Amethyst => "AMETHYST-CROWNED WYRM",
        }
    }
    pub fn lesson(self) -> &'static str {
        match self {
            Self::Ironroot => {
                "Find the gap in its stone rings. Sidestep its charge; strike while it rests."
            }
            Self::Cindermaw => {
                "Leave the landing circle. Walk away from its inhale; punish the recovery."
            }
            Self::Amethyst => {
                "Cross the beam before it fires. Weave between crystal rings; close in after."
            }
        }
    }
    pub fn definition(self) -> Boss {
        let (rules, art) = match self {
            Self::Ironroot => (
                "where mines\nhp 1000\nsize 1.8\nmove chase speed=1.8\nattack aimed shots=1 speed=4 every=75 damage=11 shot=ore from=open\nattack fan shots=3 arc=70 speed=5 every=140 damage=10 shot=ore from=rage\nwindup slam every=130 damage=18 shots=3 arc=85 speed=4 from=open cry=FAULTLINE\nwindup charge wind=45 every=140 damage=24 speed=24 from=middle cry=IRONFALL\nrage 30%\ndrops ore 4, gold 100\n",
                &[
                    "....gSSgg....",
                    "...oSSHSSo...",
                    "..oSS9SSSSo..",
                    ".oSSSSSSSSSo.",
                    "SSS..SSS..SSS",
                    "SS...SSS...SS",
                    ".S..SSSSS..S.",
                    "....SS.SS....",
                    "...SS...SS...",
                ] as &[&str],
            ),
            Self::Cindermaw => (
                "where crypt\nhp 920\nsize 1.6\nmove chase speed=2.5\nattack fan shots=3 arc=55 speed=4 every=85 damage=12 shot=ember from=open\nattack ring shots=6 speed=4 every=180 damage=10 shot=flame from=rage\nwindup leap wind=45 every=125 damage=22 shots=4 speed=4 shot=ember from=open cry=COALFALL\nwindup inhale wind=54 every=170 damage=22 shots=6 speed=4 shot=flame from=middle cry=INHALE\nrage 30%\ndrops ember 3, gold 100\n",
                &[
                    ".....7..7.....",
                    "...g7gg7gg....",
                    "..gGggggGGg...",
                    ".gG9ggggggGg..",
                    "gGggggggggGGg.",
                    ".ggg7ggggggggg",
                    "..GGggggggG..g",
                    ".GG..GG..GG...",
                ] as &[&str],
            ),
            Self::Amethyst => (
                "where mines\nhp 1000\nsize 1.5\nmove anchor speed=2.6\nattack ring shots=8 speed=4 every=120 damage=11 shot=orb from=open\nattack fan shots=3 arc=80 speed=5 every=100 damage=13 shot=wisp from=middle\nwindup beam wind=54 every=135 damage=22 arc=75 from=open cry=REFRACT\nwindup slam every=200 damage=12 shots=2 arc=90 speed=4 from=rage cry=RESONATE\nrage 30%\ndrops gem 4, gold 100\n",
                &[
                    ".....^.......",
                    "....]^]......",
                    "...][^][.....",
                    "...U2UUU.....",
                    "....UUUU.....",
                    "....UUVU.....",
                    "...UUUVUU....",
                    "..UUUVVUUU...",
                    ".UU..UU..UU..",
                    "..UUUUUUUU...",
                ] as &[&str],
            ),
        };
        let raw = format!(
            "name {}\ntext {}\nby Knights Wish\n{}art\n{}\n",
            self.name(),
            self.lesson(),
            rules,
            art.join("\n")
        );
        bosses::check(self.id(), &raw)
            .expect("checked claim guardian")
            .0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Encounter {
    pub seed: u64,
    pub raid: u64,
    pub depth: u32,
    pub room: usize,
    pub guardian: Guardian,
    pub enemy: u32,
    pub defeated: bool,
}
impl Encounter {
    pub fn valid(&self) -> bool {
        self.enemy > 0 && self.depth > 0 && self.depth <= DEEPEST && self.room < 512
    }
    fn here(&self, run: &Run) -> bool {
        self.seed == run.seed
            && self.raid == run.raid_id
            && self.depth == run.dungeon.depth
            && self.room == run.at
    }
}

impl Run {
    /// Called after discovery and before secure. E challenges once; repeated E
    /// cannot create copies. An abandoned body must be challenged afresh.
    pub(super) fn tick_claim_guardian(
        &mut self,
        deposit: &industry::Deposit,
        inputs: &BTreeMap<u32, Input>,
    ) {
        if !deposit.clear
            || !self.active()
            || self.dungeon.depth == 0
            || !self.ore_sentries_defeated(&deposit.id)
        {
            return;
        }
        let Some(kind) = Guardian::for_resource(deposit.resource) else {
            return;
        };
        if !self
            .home
            .industry
            .claims
            .get(&deposit.id)
            .is_some_and(|c| !c.secured)
        {
            return;
        }
        if self.kingdom.claim_bosses.get(&deposit.id).is_some_and(|e| {
            e.defeated
                || self
                    .enemies
                    .iter()
                    .chain(&self.parked)
                    .any(|body| body.id == e.enemy && body.hp > 0)
        }) {
            return;
        }
        if !self.players.iter().any(|(id, h)| {
            h.hp > 0
                && !h.stone
                && !h.absent
                && (h.x - deposit.x).hypot(h.y - deposit.y) < 3.5
                && inputs.get(id).is_some_and(|i| i.valid() && i.interact)
        }) {
            return;
        }
        if self.kingdom.claim_bosses.len() >= MAX_ENCOUNTERS
            && !self.kingdom.claim_bosses.contains_key(&deposit.id)
        {
            return;
        }
        if self.enemies.len() >= boss_ecology::ENTITY_CAP || self.next_id == u32::MAX {
            return;
        }
        let definition = kind.definition();
        // Leave a readable arrival gap. A blocked vein never creates a boss in
        // a wall or directly on a knight, and never silently grants the claim.
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        let spot = (2..self.room().rows as i32 - 2)
            .flat_map(|y| (2..self.room().cols as i32 - 2).map(move |x| (x, y)))
            .map(|(x, y)| ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.5) * TILE_UNITS))
            .filter(|&(x, y)| {
                (x - deposit.x).hypot(y - deposit.y) <= 12.0
                    && grid.clear(x, y, definition.radius, Mover::Walker)
                    && self
                        .players
                        .values()
                        .filter(|h| h.hp > 0 && !h.absent)
                        .all(|h| (h.x - x).hypot(h.y - y) > 5.5)
            })
            .min_by(|a, b| {
                ((a.0 - deposit.x + 7.0).hypot(a.1 - deposit.y))
                    .total_cmp(&((b.0 - deposit.x + 7.0).hypot(b.1 - deposit.y)))
            });
        let Some((x, y)) = spot else {
            return;
        };
        let index = if let Some(i) = self.bosses.iter().position(|b| b.id == kind.id()) {
            i
        } else {
            if self.bosses.len() >= 64 {
                return;
            }
            self.bosses.push(definition);
            self.bosses.len() - 1
        };
        let before = self.next_id;
        self.spawn_boss(index);
        if self.next_id == before {
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
        let enemy = self.enemies.last_mut().expect("spawned guardian");
        (enemy.x, enemy.y, enemy.origin_x, enemy.origin_y) = (x, y, x, y);
        enemy.home = home;
        // An arrival hold provides an unambiguous start without surprise hits.
        enemy.frozen = HZ;
        self.kingdom.claim_bosses.insert(
            deposit.id.clone(),
            Encounter {
                seed: self.seed,
                raid: self.raid_id,
                depth: self.dungeon.depth,
                room: self.at,
                guardian: kind,
                enemy: enemy.id,
                defeated: false,
            },
        );
        self.notices.push(format!(
            "{} guards the claim. {}",
            kind.name(),
            kind.lesson()
        ));
        self.sounds.push("boss_roar");
    }

    pub fn claim_guardian_defeated(&self, claim: &str) -> bool {
        self.kingdom
            .claim_bosses
            .get(claim)
            .is_some_and(|e| e.defeated)
    }

    /// An admitted body is independently bounded by its claim encounter. It
    /// does not borrow the floor's finite camp or emission budget.
    pub(super) fn admitted_claim_guardian(&self, enemy: &Enemy) -> bool {
        enemy.kind == EnemyKind::Boss
            && enemy.camp_slot.is_none()
            && enemy.faction.is_none()
            && self.kingdom.claim_bosses.iter().any(|(id, e)| {
                !e.defeated
                    && e.here(self)
                    && e.enemy == enemy.id
                    && self.home.industry.claims.get(id).is_some_and(|c| {
                        !c.secured && Guardian::for_resource(&c.resource) == Some(e.guardian)
                    })
                    && enemy
                        .boss
                        .and_then(|i| self.bosses.get(i as usize))
                        .is_some_and(|b| b.id == e.guardian.id())
            })
    }

    /// Only the normal death settlement calls this. Missing/parked/retired
    /// enemies provide no victory evidence. The returned IDs may pay once.
    pub(super) fn settle_claim_guardians(&mut self) -> BTreeSet<u32> {
        let fallen: BTreeSet<_> = self
            .enemies
            .iter()
            .filter(|e| e.hp == 0 && self.admitted_claim_guardian(e))
            .map(|e| e.id)
            .collect();
        for encounter in self.kingdom.claim_bosses.values_mut() {
            if !encounter.defeated && fallen.contains(&encounter.enemy) {
                encounter.defeated = true;
                self.notices.push(format!(
                    "{} defeated. Return to the vein and press E to secure the claim.",
                    encounter.guardian.name()
                ));
                // Claim unlocking is deliberately not a resource payout. The
                // mine is still the only way the kingdom obtains this output.
            }
        }
        fallen
    }
}

#[cfg(test)]
#[path = "../../tests/rules/claim_bosses__tests.rs"]
pub(crate) mod tests;
