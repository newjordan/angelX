//! The Deep Stirs. Digging at claims and working the shaft raise a stir in
//! the rock; left alone it settles. Past a threshold the Delve answers: a
//! secured claim the party has left unguarded too long has its sentries sent
//! back, once an expedition, and only from the third floor down. The
//! sentries are the ordinary ore sentries (`ore_threats`), so a raid is a
//! fight the party already knows, never a lock: it is bounded to one raid a
//! claim an expedition, it never starts on the first two floors, and beating
//! it quiets the Deep.
use super::*;
use std::collections::BTreeSet;

/// The most the Deep can be stirred.
pub const MAX: u16 = 100;
/// What a block cut from a claim stirs.
pub const CUT: u16 = 2;
/// What a block lifted into the stockyard stirs.
pub const LIFT: u16 = 1;
/// What raising the shaft a tier stirs.
pub const UPGRADE: u16 = 5;
/// One point settles this often while the party plays.
pub const CALM_EVERY: u64 = 20 * HZ as u64;
/// The stir at which the Delve answers.
pub const RAID_AT: u16 = 30;
/// The shallowest floor a raid reaches: the first two are never raided.
pub const RAID_FROM: u32 = 3;
/// How long a claim goes unguarded before a raid can return to it.
pub const UNGUARDED: u64 = 90 * HZ as u64;
/// How far from a seam a knight guards it, in units.
pub const GUARD_REACH: f32 = 14.0;
/// What a broken raid takes off the stir.
pub const RELIEF: u16 = 15;
/// Blocks one face gives in an expedition: thin seams push you deeper.
pub const FACE_BLOCKS: u8 = 12;
/// Most claims the books keep.
const MOST: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Raid {
    /// Raids this expedition.
    pub n: u8,
    /// The sentries are back and not yet beaten.
    pub live: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stir {
    pub level: u16,
    /// Blocks cut from each face this expedition.
    pub cut: BTreeMap<String, u8>,
    /// Swings that have landed on each face since its last block parted.
    pub chips: BTreeMap<String, u8>,
    /// The face struck last and when, for the chips that fly from it.
    pub struck: Option<(String, u64)>,
    /// When a knight last stood by each secured claim.
    pub watched: BTreeMap<String, u64>,
    pub raids: BTreeMap<String, Raid>,
    /// The night raids on the works (`deep_raid`): how many, and the one in hand.
    pub watch: super::deep_raid::Watch,
}

impl Stir {
    pub fn is_calm(&self) -> bool {
        *self == Self::default()
    }
    pub fn raiding(&self, claim: &str) -> bool {
        self.raids.get(claim).is_some_and(|r| r.live)
    }
    pub fn valid(&self) -> bool {
        fn keys<'a>(mut m: impl Iterator<Item = &'a String>) -> bool {
            m.all(|k| !k.is_empty() && k.len() <= 96)
        }
        self.level <= MAX
            && self.cut.len() <= MOST
            && self.chips.len() <= MOST
            && self.watched.len() <= MOST
            && self.raids.len() <= MOST
            && self.watch.valid()
            && self.cut.values().all(|&n| n <= FACE_BLOCKS)
            && self.chips.values().all(|&n| n < CHIPS)
            && keys(self.cut.keys())
            && keys(self.chips.keys())
            && keys(self.watched.keys())
            && keys(self.raids.keys())
            && self.struck.as_ref().is_none_or(|(k, _)| !k.is_empty() && k.len() <= 96)
    }
    /// A new expedition starts with no face cut and no claim watched; the
    /// Deep's stir carries over and settles on its own.
    pub fn new_expedition(&mut self) {
        self.cut.clear();
        self.chips.clear();
        self.struck = None;
        self.watched.clear();
        self.raids.clear();
    }
}

/// Swings to part one rough block from a face.
pub const CHIPS: u8 = 3;

impl Run {
    pub fn stir_up(&mut self, n: u16) {
        let before = self.kingdom.stir.level;
        self.kingdom.stir.level = (before + n).min(MAX);
        // Said once, as it crosses: from here the Delve may answer.
        if before < RAID_AT && self.kingdom.stir.level >= RAID_AT {
            self.notices
                .push("The rock is restless. The Deep stirs: leave a claim unguarded and its sentries may return.".into());
        }
    }

    /// Once a tick while the party plays: the stir settles, and each shift
    /// the working mines and the shaft's machinery stir it a little.
    pub(super) fn tick_stir(&mut self) {
        if self.tick % CALM_EVERY == 0 {
            self.kingdom.stir.level = self.kingdom.stir.level.saturating_sub(1);
        }
        if self.tick % industry::SHIFT_TICKS == 0 && !self.home.industry.is_empty() {
            let cat = crate::kingdom::catalog();
            let mines = self
                .home
                .industry
                .buildings
                .values()
                .filter(|b| {
                    b.construction == 0
                        && !b.paused
                        && b.claim.is_some()
                        && cat.plans.get(&b.plan).is_some_and(|p| p.extractor)
                })
                .count() as u16;
            if mines > 0 {
                self.stir_up(mines + self.home.stockyard.tier.noise());
            }
        }
    }

    /// Each secured claim in this room: note when a knight last guarded it
    /// and, if the Deep is stirred and it went unguarded too long, send its
    /// sentries back. A raid already on ends when its sentries are down.
    pub(super) fn tick_stir_claim(&mut self, d: &industry::Deposit) {
        if !self.active() || self.dungeon.depth == 0 {
            return;
        }
        let Some(claim) = self.home.industry.claims.get(&d.id) else {
            return;
        };
        if !claim.secured {
            return;
        }
        let name = claim.name.clone();
        let tick = self.tick;
        let guarded = self
            .players
            .values()
            .any(|h| !h.absent && h.hp > 0 && (h.x - d.x).hypot(h.y - d.y) < GUARD_REACH);
        if self.kingdom.stir.raiding(&d.id) {
            if self.ore_sentries_defeated(&d.id) && self.kingdom.ore_sentries.contains_key(&d.id) {
                if let Some(r) = self.kingdom.stir.raids.get_mut(&d.id) {
                    r.live = false;
                }
                self.kingdom.stir.level = self.kingdom.stir.level.saturating_sub(RELIEF);
                self.notices
                    .push(format!("The raid on the {name} is broken. The Deep is quieter."));
            }
            return;
        }
        let stir = &mut self.kingdom.stir;
        let seen = stir.watched.get(&d.id).copied();
        if guarded {
            if stir.watched.len() < MOST || seen.is_some() {
                stir.watched.insert(d.id.clone(), tick);
            }
        }
        let away = seen.map_or(0, |at| tick.saturating_sub(at));
        let tried = stir.raids.get(&d.id).is_some_and(|r| r.n > 0);
        if guarded
            && seen.is_some()
            && away >= UNGUARDED
            && stir.level >= RAID_AT
            && self.dungeon.depth >= RAID_FROM
            && !tried
            && stir.raids.len() < MOST
        {
            stir.raids.insert(d.id.clone(), Raid { n: 1, live: true });
            // The same two sentries, stood up again: they are no longer
            // fallen, and the ordinary sentry tick finds their bodies gone
            // and raises them afresh (ids only ever grow, so a fallen one's
            // is never another body's). Until it does, the books still hold
            // two distinct sentries and stay valid.
            if let Some(e) = self.kingdom.ore_sentries.get_mut(&d.id) {
                for s in &mut e.sentries {
                    s.defeated = false;
                }
            }
            self.notices.push(format!(
                "RAID: the Deep has sent its sentries back to the {name}. Clear them before you cut."
            ));
            self.sounds.push("door_seal");
        }
    }

    /// Claims under raid now, for the HUD line and the picture.
    pub fn raided_claims(&self) -> BTreeSet<&str> {
        self.kingdom
            .stir
            .raids
            .iter()
            .filter(|(_, r)| r.live)
            .map(|(k, _)| k.as_str())
            .collect()
    }
}

#[cfg(test)]
#[path = "../../tests/rules/stir__tests.rs"]
mod tests;
