//! Squires' Yard: physical, hold-to-use town ledgers. No menu commands.

use super::squires_lodge::{self as squires, Action, Building, Error, Kind};
use super::*;

/// On Castle Town's east green, not the Lists' authored (34,13).
/// Reserved before procedural sites so future doors cannot claim this yard.
pub const YARD: (i32, i32, i32, i32) = (94, 74, 14, 7);
pub const BUILDINGS: [Building; 3] = [Building::Recruiting, Building::Hiring, Building::Training];
pub const GOLD_MARK: &str = "squires:gold";

pub const TITLES: [&str; 3] = ["RECRUIT", "ROSTER", "TRAIN"];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    pub owner: String,
    pub page: [usize; 3],
    pub holding: Option<(usize, usize)>,
}

/// NEXT / HIRE, SELECT or TRAIN / UPGRADE, below each separate building.
pub fn plate(station: usize, control: usize) -> (i32, i32) {
    (YARD.0 + station as i32 * 5 + control as i32, YARD.1 + 4)
}
pub fn building(station: usize) -> (i32, i32, i32, i32) {
    (YARD.0 + station as i32 * 5, YARD.1, 4, 3)
}
/// Whether the yard stands on the map: it does in the full plan (Castle
/// Town's east green) and not in the blank-slate plan (`world::plan`).
pub fn on_map() -> bool {
    crate::world::plan::full()
}
pub fn station_at(x: f32, y: f32) -> Option<(usize, usize)> {
    if !on_map() {
        return None;
    }
    for s in 0..3 {
        for c in 0..3 {
            let (px, py) = plate(s, c);
            if x >= px as f32 * TILE_UNITS
                && x < (px + 1) as f32 * TILE_UNITS
                && y >= py as f32 * TILE_UNITS
                && y < (py + 1) as f32 * TILE_UNITS
            {
                return Some((s, c));
            }
        }
    }
    None
}
pub fn nearby(x: f32, y: f32) -> Option<usize> {
    if !on_map() {
        return None;
    }
    let (x, y) = (x / TILE_UNITS, y / TILE_UNITS);
    (0..3).find(|&s| {
        let (bx, by, _, _) = building(s);
        x >= bx as f32 - 0.6 && x < bx as f32 + 4.4 && y >= by as f32 + 2.5 && y < by as f32 + 6.5
    })
}
pub fn ability(kind: Kind, level: u8) -> String {
    kind.ability_summary(level)
}

impl Run {
    pub fn squire_page(&self, seat: u32, station: usize) -> usize {
        let Some(h) = self.players.get(&seat) else {
            return 0;
        };
        self.kingdom
            .squire_yard
            .get(&seat)
            .filter(|l| l.owner == squires::owner_key(h))
            .and_then(|l| l.page.get(station).copied())
            .unwrap_or(0)
    }
    fn lodge_squire_choices(&self, seat: u32, station: usize) -> usize {
        match station {
            0 => self.squire_recruits(seat).len(),
            1 => self.squire_roster(seat).map_or(1, |r| r.members.len() + 1),
            _ => self.squire_roster(seat).map_or(0, |r| r.members.len()),
        }
    }
    pub(super) fn tick_squires_town(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Kingdom || !on_map() {
            return;
        }
        let mut ready = Vec::new();
        for (&id, h) in &mut self.players {
            let on = station_at(h.x, h.y).filter(|_| h.hp > 0 && !h.stone && !h.absent && !h.away);
            let held = inputs.get(&id).is_some_and(|i| i.valid() && i.interact);
            if on.is_none() && !self.kingdom.squire_yard.contains_key(&id) {
                continue;
            }
            let l = self.kingdom.squire_yard.entry(id).or_default();
            let owner = squires::owner_key(h);
            if l.owner != owner {
                *l = Ledger {
                    owner,
                    ..Ledger::default()
                };
            }
            if !held {
                h.buy_spent = false;
            }
            if l.holding != on {
                h.buying = 0;
                l.holding = on;
            }
            // Leave unrelated station holds alone away from this yard.
            if on.is_none() {
                continue;
            }
            if !held || h.buy_spent {
                h.buying = 0;
                continue;
            }
            h.buying = h.buying.saturating_add(1);
            if h.buying >= home::BUY_HOLD {
                h.buying = 0;
                h.buy_spent = true;
                ready.push((id, on.unwrap()));
            }
        }
        self.kingdom
            .squire_yard
            .retain(|id, _| self.players.contains_key(id));
        for (id, (s, c)) in ready {
            let n = self.lodge_squire_choices(id, s);
            let page = self.squire_page(id, s) % n.max(1);
            if c == 0 {
                self.kingdom.squire_yard.get_mut(&id).unwrap().page[s] = (page + 1) % n.max(1);
                self.sounds.push("play_card");
                continue;
            }
            let action = if c == 2 {
                Some(Action::Upgrade(BUILDINGS[s]))
            } else {
                match s {
                    0 => self
                        .squire_recruits(id)
                        .get(page)
                        .map(|r| Action::Hire(r.id)),
                    1 => Some(Action::Select(
                        self.squire_roster(id)
                            .and_then(|r| r.members.get(page))
                            .map(|m| m.id),
                    )),
                    _ => self
                        .squire_roster(id)
                        .and_then(|r| r.members.get(page))
                        .map(|m| Action::Train(m.id)),
                }
            };
            let before = self.treasury.get(Spoil::Gold);
            let result = action.map_or(Err(Error::Missing), |a| self.squire_action(id, a));
            let spent = before - self.treasury.get(Spoil::Gold);
            if spent > 0 {
                *self.marks.entry(GOLD_MARK.into()).or_default() += spent;
            }
            let word = match result {
                Ok(()) => {
                    self.sounds.push("play_card");
                    match c {
                        2 => "BUILDING UPGRADED",
                        _ => match s {
                            0 => "SQUIRE HIRED",
                            1 => "SELECTION SAVED",
                            _ => "SQUIRE TRAINED",
                        },
                    }
                }
                Err(Error::Funds) => "NEED TREASURY GOLD",
                Err(Error::RosterFull) => "ROSTER FULL: 12 MAX",
                Err(Error::AlreadyHired) => "ALREADY HIRED",
                Err(Error::MaxLevel) if c == 2 => "BUILDING AT MAX LEVEL",
                Err(Error::MaxLevel) if self.home.squires_lodge.training == squires::MAX_BUILDING => {
                    "MAX TRAINING L8"
                }
                Err(Error::MaxLevel) => "AT CAP: UPGRADE TRAINING",
                _ => "NO SQUIRE HERE",
            };
            self.found = Some((self.tick, id, word.into()));
        }
    }
}

/// Private checkpoint policy shared by the host writer and restore validator.
/// General combat counters remain transient and are never put on the guest wire.
pub fn checkpoint_intent_key(key: &str) -> bool {
    key == GOLD_MARK || super::countryside::intent_key(key)
}
pub fn checkpoint_intent_valid(key: &str, value: u32) -> bool {
    if key == GOLD_MARK {
        value > 0
    } else {
        super::countryside::intent_valid(key, value)
    }
}
pub const CHECKPOINT_INTENT_LIMIT: usize = super::countryside::INTENT_LIMIT + 1;

/// Host-private receipt for the batch in a pre-drain checkpoint. The realm
/// commits its sequence beside the guild and gold. It is not part of Run or
/// the guest wire. A restart may already have made more purchases before the
/// cockpit reconciles this receipt, so retire only this batch's debit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointPayment {
    pub sequence: u64,
    pub gold: u32,
}
impl CheckpointPayment {
    pub fn valid(self, marks: &BTreeMap<String, u32>) -> bool {
        self.sequence > 0 && self.gold <= marks.get(GOLD_MARK).copied().unwrap_or(0)
    }

    /// Consume this receipt once before draining the next batch. Its caller
    /// replaces/clears it even when no purchase remains to be settled.
    pub fn retire(self, committed: u64, marks: &mut BTreeMap<String, u32>) {
        if self.sequence <= committed
            && let Some(gold) = marks.get_mut(GOLD_MARK)
        {
            *gold = gold.saturating_sub(self.gold);
            if *gold == 0 {
                marks.remove(GOLD_MARK);
            }
        }
    }
}

/// Reconcile a completed realm save with a run that kept stepping during IO.
/// `captured` is the guild whose debit was drained, not the current live guild.
pub fn rebase(
    captured: &squires::Guild,
    committed: &squires::Guild,
    live: &squires::Guild,
    treasury: &Spoils,
    pending: &BTreeMap<String, u32>,
) -> (squires::Guild, Spoils) {
    // Only this cockpit writes the durable guild. While it saves, the sim can
    // extend the captured guild (including zero-price selections). Prefer that
    // live extension, not a fieldwise merge that could resurrect a selection.
    let mut guild = if live == captured { committed } else { live }.clone();
    guild.rent_days = guild.rent_days.max(committed.rent_days);
    guild.delves = guild.delves.max(committed.delves);
    // The durable purse paid only the drained batch. Keep later purchases
    // reserved locally until their own settlement; never refund their gold.
    let mut purse = treasury.clone();
    let reserved = pending.get(GOLD_MARK).copied().unwrap_or(0);
    purse.take(&Spoils(BTreeMap::from([(Spoil::Gold, reserved)])));
    (guild, purse)
}

/// The run already paid locally. Charge the durable purse once, alongside
/// its ledger, in the host's existing save transaction. A failed save keeps
/// the mark for retry; an identical ledger is idempotent.
pub fn settle(
    guild: &mut squires::Guild,
    treasury: &mut Spoils,
    proposed: &squires::Guild,
    marks: &BTreeMap<String, u32>,
) -> Result<bool, Error> {
    let mut next = proposed.clone();
    // A rent settlement may have run just before this home settlement.
    next.rent_days = next.rent_days.max(guild.rent_days);
    next.delves = next.delves.max(guild.delves);
    if *guild == next {
        return Ok(false);
    }
    let cost = marks.get(GOLD_MARK).copied().unwrap_or(0);
    let price = Spoils(BTreeMap::from([(Spoil::Gold, cost)]));
    if !treasury.covers(&price) {
        return Err(Error::Funds);
    }
    treasury.take(&price);
    *guild = next;
    Ok(true)
}

/// One refresh per existing rent settlement, never per UI visit or bank haul.
pub fn rent_day(guild: &mut squires::Guild, cleared_floors: usize) {
    if cleared_floors > 0 {
        guild.rent_day();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::realm::{Spoil, Spoils};
    use crate::rules::home::Home;
    fn yard() -> Run {
        let mut r = Run::at_home(23, 1, Some("Jordan"), Home::default(), Spoils::default());
        r.come_to_the_gate();
        r.walk_kingdom(Some((YARD.0 + 1, YARD.1 + 4)));
        r.players.get_mut(&1).unwrap().name = "Jordan".into();
        r.treasury.add(Spoil::Gold, 5000);
        r
    }
    fn stand(r: &mut Run, station: usize, control: usize) {
        let (x, y) = plate(station, control);
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y, h.stone) = (
            (x as f32 + 0.5) * TILE_UNITS,
            (y as f32 + 0.5) * TILE_UNITS,
            false,
        );
    }
    fn hold(r: &mut Run, ticks: u32) {
        let inputs = BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Input::default()
            },
        )]);
        for _ in 0..ticks {
            r.tick_squires_town(&inputs);
        }
    }
    #[test]
    fn hold_hire_select_train_and_three_persisted_buildings() {
        let mut r = yard();
        stand(&mut r, 0, 1);
        hold(&mut r, home::BUY_HOLD - 1);
        assert!(r.squire_roster(1).is_none());
        hold(&mut r, 1);
        assert_eq!(r.squire_roster(1).unwrap().members.len(), 1);
        hold(&mut r, home::BUY_HOLD * 2);
        assert_eq!(
            r.squire_roster(1).unwrap().members.len(),
            1,
            "held E must not repeat"
        );
        for (station, control) in [(1, 1), (2, 1), (0, 2), (1, 2), (2, 2)] {
            r.tick_squires_town(&BTreeMap::new());
            stand(&mut r, station, control);
            hold(&mut r, home::BUY_HOLD);
        }
        let roster = r.squire_roster(1).unwrap();
        assert_eq!(roster.active, Some(roster.members[0].id));
        assert_eq!(roster.members[0].level, 2);
        assert_eq!(
            (
                r.home.squires_lodge.recruiting,
                r.home.squires_lodge.hiring,
                r.home.squires_lodge.training
            ),
            (1, 1, 1)
        );
        assert!(r.treasury.get(Spoil::Gold) < 5000);
        let copy: Home = serde_json::from_str(&serde_json::to_string(&r.home).unwrap()).unwrap();
        assert_eq!(copy.squires_lodge, r.home.squires_lodge);
    }
    #[test]
    fn yard_is_walkable_on_actual_map_and_does_not_cover_a_door() {
        let r = yard();
        let geo = map::geography(&[]);
        for s in 0..3 {
            for c in 0..3 {
                let (x, y) = plate(s, c);
                assert_eq!(geo.room.tile(x, y), Tile::Floor, "plate {s}/{c} at {x},{y}");
                assert!(
                    kingdom::door_at(
                        &geo,
                        (x as f32 + 0.5) * TILE_UNITS,
                        (y as f32 + 0.5) * TILE_UNITS
                    )
                    .is_none()
                );
            }
        }
        assert_eq!(r.room().kind, RoomKind::Kingdom);
    }
    #[test]
    fn settlement_charges_once_retries_atomically_and_preserves_rent_refresh() {
        let mut r = yard();
        stand(&mut r, 0, 1);
        hold(&mut r, home::BUY_HOLD);
        let mut guild = squires::Guild::default();
        let mut purse = Spoils::default();
        assert_eq!(
            settle(&mut guild, &mut purse, &r.home.squires_lodge, &r.marks),
            Err(Error::Funds)
        );
        assert!(guild.is_empty());
        purse.add(Spoil::Gold, 5000);
        rent_day(&mut guild, 0);
        assert_eq!(guild.rent_days, 0);
        rent_day(&mut guild, 3);
        assert_eq!(guild.rent_days, 1);
        assert_eq!(
            settle(&mut guild, &mut purse, &r.home.squires_lodge, &r.marks),
            Ok(true)
        );
        assert_eq!(guild.rent_days, 1);
        assert_eq!(purse.get(Spoil::Gold), r.treasury.get(Spoil::Gold));
        let gold = purse.get(Spoil::Gold);
        assert_eq!(
            settle(&mut guild, &mut purse, &r.home.squires_lodge, &r.marks),
            Ok(false)
        );
        assert_eq!(purse.get(Spoil::Gold), gold);
    }

    fn checkpoint_round_trip(run: &Run) -> Run {
        // Same private envelope as cockpit: Run deliberately skips marks.
        let journal: BTreeMap<String, u32> = run
            .marks
            .iter()
            .filter(|(key, _)| checkpoint_intent_key(key))
            .map(|(key, &n)| (key.clone(), n))
            .collect();
        assert!(journal.len() <= CHECKPOINT_INTENT_LIMIT);
        assert!(
            journal
                .iter()
                .all(|(key, &n)| checkpoint_intent_valid(key, n))
        );
        let bytes = serde_json::to_vec(&(run, journal)).unwrap();
        let (mut restored, journal): (Run, BTreeMap<String, u32>) =
            serde_json::from_slice(&bytes).unwrap();
        assert!(restored.marks.is_empty());
        restored.marks = journal;
        restored
    }

    #[test]
    fn purchase_checkpoint_restore_settle_twice_charges_once() {
        let mut run = yard();
        stand(&mut run, 0, 1);
        hold(&mut run, home::BUY_HOLD);
        run.marks.insert("kill:rat".into(), 7);
        let debit = run.marks[GOLD_MARK];
        let restored = checkpoint_round_trip(&run);
        assert_eq!(restored.marks.get(GOLD_MARK), Some(&debit));
        assert!(!restored.marks.contains_key("kill:rat"));
        assert!(!checkpoint_intent_key("squires:other"));
        assert!(!checkpoint_intent_valid(GOLD_MARK, 0));
        assert!(checkpoint_intent_valid(GOLD_MARK, u32::MAX));
        let mut guild = squires::Guild::default();
        let mut treasury = Spoils::default();
        treasury.add(Spoil::Gold, 5000);
        assert_eq!(
            settle(
                &mut guild,
                &mut treasury,
                &restored.home.squires_lodge,
                &restored.marks
            ),
            Ok(true)
        );
        assert_eq!(treasury.get(Spoil::Gold), 5000 - debit);
        assert_eq!(guild.rosters, run.home.squires_lodge.rosters);
        assert_eq!(
            settle(
                &mut guild,
                &mut treasury,
                &restored.home.squires_lodge,
                &restored.marks
            ),
            Ok(false)
        );
        assert_eq!(treasury.get(Spoil::Gold), 5000 - debit);
    }

    #[test]
    fn interleaved_purchase_settlement_preserves_actions_and_reserved_gold() {
        let mut run = yard();
        stand(&mut run, 0, 1);
        hold(&mut run, home::BUY_HOLD);
        let captured = run.home.squires_lodge.clone();
        let marks = std::mem::take(&mut run.marks);
        let first_debit = marks[GOLD_MARK];
        // Complete another hire, training and selection while the first hire
        // is saved. None of their mutations or debit may be replaced/refunded.
        let before = run.treasury.get(Spoil::Gold);
        let second = run.squire_recruits(1)[1].id;
        run.squire_action(1, Action::Hire(second)).unwrap();
        run.squire_action(1, Action::Upgrade(Building::Training))
            .unwrap();
        let id = run.squire_roster(1).unwrap().members[0].id;
        run.squire_action(1, Action::Train(id)).unwrap();
        run.squire_action(1, Action::Select(None)).unwrap();
        let second_debit = before - run.treasury.get(Spoil::Gold);
        run.marks.insert(GOLD_MARK.into(), second_debit);
        let live = run.home.squires_lodge.clone();
        let mut guild = squires::Guild::default();
        let mut treasury = Spoils::default();
        treasury.add(Spoil::Gold, 5000);
        rent_day(&mut guild, 1);
        assert_eq!(
            settle(&mut guild, &mut treasury, &captured, &marks),
            Ok(true)
        );
        let (home, purse) = rebase(&captured, &guild, &run.home.squires_lodge, &treasury, &run.marks);
        assert_eq!(home.rosters, live.rosters);
        assert_eq!(home.training, live.training);
        assert_eq!(home.rent_days, 1);
        assert_eq!(purse.get(Spoil::Gold), 5000 - first_debit - second_debit);
        run.home.squires_lodge = home;
        run.treasury = purse;
        let restored = checkpoint_round_trip(&run);
        assert_eq!(
            settle(
                &mut guild,
                &mut treasury,
                &restored.home.squires_lodge,
                &restored.marks
            ),
            Ok(true)
        );
        assert_eq!(
            treasury.get(Spoil::Gold),
            restored.treasury.get(Spoil::Gold)
        );
        assert_eq!(
            settle(
                &mut guild,
                &mut treasury,
                &restored.home.squires_lodge,
                &restored.marks
            ),
            Ok(false)
        );
        assert_eq!(treasury.get(Spoil::Gold), 5000 - first_debit - second_debit);
    }

    #[test]
    fn checkpoint_payment_replay_before_and_after_commit_keeps_later_actions() {
        // Both crash boundaries: before realm commit and after realm commit
        // but before the reconciled checkpoint. The user may buy again before
        // the first cockpit settlement after restore.
        for committed_before_crash in [false, true] {
            let mut run = yard();
            stand(&mut run, 0, 1);
            hold(&mut run, home::BUY_HOLD);
            let first_cost = run.marks[GOLD_MARK];
            let payment = CheckpointPayment {
                sequence: 1,
                gold: first_cost,
            };
            let receipt = serde_json::to_vec(&payment).unwrap();
            let payment: CheckpointPayment = serde_json::from_slice(&receipt).unwrap();
            let mut restored = checkpoint_round_trip(&run);
            let mut guild = squires::Guild::default();
            let mut treasury = Spoils::default();
            treasury.add(Spoil::Gold, 5000);
            let committed = if committed_before_crash {
                assert_eq!(
                    settle(&mut guild, &mut treasury, &run.home.squires_lodge, &run.marks),
                    Ok(true)
                );
                1
            } else {
                0
            };
            rent_day(&mut guild, 1);
            // Another paid mutation and a free selection after restore, before
            // the cockpit reconciles the receipt. Retire only the old debit.
            let id = restored.squire_roster(1).unwrap().members[0].id;
            let before = restored.treasury.get(Spoil::Gold);
            restored.squire_action(1, Action::Train(id)).unwrap();
            restored.squire_action(1, Action::Select(None)).unwrap();
            let later_cost = before - restored.treasury.get(Spoil::Gold);
            *restored.marks.get_mut(GOLD_MARK).unwrap() += later_cost;
            payment.retire(committed, &mut restored.marks);
            assert_eq!(
                restored.marks[GOLD_MARK],
                later_cost + if committed == 0 { first_cost } else { 0 }
            );
            assert_eq!(
                settle(
                    &mut guild,
                    &mut treasury,
                    &restored.home.squires_lodge,
                    &restored.marks
                ),
                Ok(true)
            );
            assert_eq!(guild.rosters, restored.home.squires_lodge.rosters);
            assert_eq!(guild.rent_days, 1);
            assert_eq!(treasury.get(Spoil::Gold), 5000 - first_cost - later_cost);
            let paid = treasury.clone();
            assert_eq!(
                settle(
                    &mut guild,
                    &mut treasury,
                    &restored.home.squires_lodge,
                    &restored.marks
                ),
                Ok(false)
            );
            assert_eq!(treasury, paid);
            let mut marks = BTreeMap::from([(GOLD_MARK.into(), first_cost)]);
            payment.retire(1, &mut marks);
            assert!(!marks.contains_key(GOLD_MARK));
            payment.retire(1, &mut BTreeMap::new());
        }
    }

    #[test]
    fn failed_save_requeues_drained_debit_beside_interleaved_purchase() {
        let mut run = yard();
        stand(&mut run, 0, 1);
        hold(&mut run, home::BUY_HOLD);
        let captured = run.home.squires_lodge.clone();
        let drained = std::mem::take(&mut run.marks);
        let guild_before = squires::Guild::default();
        let mut guild = guild_before.clone();
        let mut treasury = Spoils::default();
        treasury.add(Spoil::Gold, 5000);
        let treasury_before = treasury.clone();
        assert_eq!(
            settle(&mut guild, &mut treasury, &captured, &drained),
            Ok(true)
        );
        let before = run.treasury.get(Spoil::Gold);
        run.squire_action(1, Action::Upgrade(Building::Hiring))
            .unwrap();
        run.marks
            .insert(GOLD_MARK.into(), before - run.treasury.get(Spoil::Gold));
        let live = run.home.squires_lodge.clone();
        // Exactly the cockpit's failure path: roll back only the realm, not
        // the live guild, and prepend the drained debit to the later marks.
        guild = guild_before;
        treasury = treasury_before;
        for (key, n) in drained {
            let pending = run.marks.entry(key).or_default();
            *pending = pending.saturating_add(n);
        }
        let restored = checkpoint_round_trip(&run);
        let mut no_funds = Spoils::default();
        assert_eq!(
            settle(
                &mut guild,
                &mut no_funds,
                &restored.home.squires_lodge,
                &restored.marks
            ),
            Err(Error::Funds)
        );
        assert_eq!(guild, squires::Guild::default());
        assert!(no_funds.is_empty());
        assert_eq!(
            settle(
                &mut guild,
                &mut treasury,
                &restored.home.squires_lodge,
                &restored.marks
            ),
            Ok(true)
        );
        assert_eq!(guild, live);
        assert_eq!(treasury, run.treasury);
        let paid = treasury.clone();
        assert_eq!(
            settle(
                &mut guild,
                &mut treasury,
                &restored.home.squires_lodge,
                &restored.marks
            ),
            Ok(false)
        );
        assert_eq!(treasury, paid);
    }

    #[test]
    fn rebase_preserves_free_selection_during_other_realm_work() {
        let mut run = yard();
        stand(&mut run, 0, 1);
        hold(&mut run, home::BUY_HOLD);
        run.marks.clear(); // Hire already committed; next settlement is unrelated.
        let captured = run.home.squires_lodge.clone();
        run.squire_action(1, Action::Select(None)).unwrap();
        let (guild, purse) = rebase(
            &captured,
            &captured,
            &run.home.squires_lodge,
            &run.treasury,
            &run.marks,
        );
        assert_eq!(guild, run.home.squires_lodge);
        assert_eq!(purse, run.treasury);
        assert!(run.marks.is_empty());
        let mut durable = captured;
        let mut treasury = purse;
        assert_eq!(
            settle(&mut durable, &mut treasury, &guild, &run.marks),
            Ok(true)
        );
        assert_eq!(treasury, run.treasury);
        assert_eq!(
            settle(&mut durable, &mut treasury, &guild, &run.marks),
            Ok(false)
        );
    }

    #[test]
    fn private_checkpoint_journal_keeps_legacy_validation_and_bounds_receipts() {
        assert_eq!(
            CHECKPOINT_INTENT_LIMIT,
            super::super::countryside::INTENT_LIMIT + 1
        );
        let legacy = super::super::countryside::game_mark(0);
        assert!(checkpoint_intent_key(&legacy));
        assert!(checkpoint_intent_valid(&legacy, 1));
        assert!(!checkpoint_intent_valid(&legacy, 2));
        assert!(!checkpoint_intent_valid("kill:rat", 1));
        let marks = BTreeMap::from([(GOLD_MARK.into(), 350)]);
        assert!(
            CheckpointPayment {
                sequence: 1,
                gold: 350
            }
            .valid(&marks)
        );
        assert!(
            CheckpointPayment {
                sequence: 1,
                gold: 0
            }
            .valid(&BTreeMap::new())
        );
        assert!(
            !CheckpointPayment {
                sequence: 0,
                gold: 350
            }
            .valid(&marks)
        );
        assert!(
            !CheckpointPayment {
                sequence: 1,
                gold: 351
            }
            .valid(&marks)
        );
        assert!(
            !CheckpointPayment {
                sequence: 1,
                gold: 1
            }
            .valid(&BTreeMap::new())
        );
    }

    #[test]
    fn actual_step_holds_cycle_hire_select_none_and_train() {
        let mut r = yard();
        let input = BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Input::default()
            },
        )]);
        let use_plate = |r: &mut Run, s, c| {
            r.step(&BTreeMap::new());
            stand(r, s, c);
            for _ in 0..home::BUY_HOLD + 1 {
                r.step(&input);
            }
        };
        use_plate(&mut r, 0, 0);
        assert_eq!(r.squire_page(1, 0), 1);
        use_plate(&mut r, 0, 1);
        assert_eq!(r.squire_roster(1).unwrap().members[0].kind, Kind::Healer);
        use_plate(&mut r, 2, 1);
        assert_eq!(r.squire_roster(1).unwrap().members[0].level, 2);
        use_plate(&mut r, 1, 0); // NONE is after the last member.
        use_plate(&mut r, 1, 1);
        assert_eq!(r.squire_roster(1).unwrap().active, None);
        assert!(r.valid_snapshot());
    }

    #[test]
    fn all_plates_reachable_from_town_gate_buildings_solid_and_no_castle_overlap() {
        use std::collections::{BTreeSet, VecDeque};
        let geo = map::geography(&[]);
        let gate = map::gate_stand(&[]);
        let mut seen = BTreeSet::from([gate]);
        let mut todo = VecDeque::from([gate]);
        while let Some((x, y)) = todo.pop_front() {
            for (dx, dy) in DIRS {
                let next = (x + dx, y + dy);
                if geo.room.tile(next.0, next.1) == Tile::Floor && seen.insert(next) {
                    todo.push_back(next);
                }
            }
        }
        for s in 0..3 {
            for c in 0..3 {
                assert!(seen.contains(&plate(s, c)), "unreachable {s}/{c}");
            }
            let (x, y, w, h) = building(s);
            for y in y..y + h {
                for x in x..x + w {
                    assert_eq!(geo.room.tile(x, y), Tile::Block);
                }
            }
        }
        for (x, y, w, h) in crate::world::castles::footprints() {
            assert!(
                x + w <= YARD.0 || x >= YARD.0 + YARD.2 || y + h <= YARD.1 || y >= YARD.1 + YARD.3,
                "castle {x},{y}"
            );
        }
    }
    #[test]
    fn holds_reset_between_plates_identity_resets_pages_and_cap_failures_are_atomic() {
        let mut r = yard();
        stand(&mut r, 0, 1);
        hold(&mut r, home::BUY_HOLD - 1);
        stand(&mut r, 0, 2);
        hold(&mut r, 1);
        assert_eq!(r.home.squires_lodge.recruiting, 0, "hold cannot transfer plates");
        r.tick_squires_town(&BTreeMap::new());
        stand(&mut r, 0, 0);
        hold(&mut r, home::BUY_HOLD);
        assert_eq!(r.squire_page(1, 0), 1);
        r.players.get_mut(&1).unwrap().name = "Another".into();
        r.tick_squires_town(&BTreeMap::new());
        assert_eq!(r.squire_page(1, 0), 0);
        stand(&mut r, 0, 1);
        hold(&mut r, home::BUY_HOLD);
        r.tick_squires_town(&BTreeMap::new());
        stand(&mut r, 2, 1);
        hold(&mut r, home::BUY_HOLD);
        let guild = r.home.squires_lodge.clone();
        let gold = r.treasury.clone();
        r.tick_squires_town(&BTreeMap::new());
        hold(&mut r, home::BUY_HOLD);
        assert_eq!(r.home.squires_lodge, guild);
        assert_eq!(r.treasury, gold);
        assert!(r.found.as_ref().unwrap().2.contains("AT CAP"));
        r.treasury = Spoils::default();
        r.tick_squires_town(&BTreeMap::new());
        stand(&mut r, 2, 2);
        hold(&mut r, home::BUY_HOLD);
        assert_eq!(r.home.squires_lodge, guild);
        assert!(r.found.as_ref().unwrap().2.contains("NEED TREASURY"));
    }
    #[test]
    fn home_rebuild_clears_rejected_companion_immediately_and_epochs_never_rewind() {
        let mut r = yard();
        stand(&mut r, 0, 1);
        hold(&mut r, home::BUY_HOLD);
        assert!(r.players[&1].squire.is_some());
        r.rebuild_home(Home::default(), Spoils::default());
        assert!(r.players[&1].squire.is_none());
        let mut guild = squires::Guild::default();
        guild.advance_delve();
        guild.rent_day();
        assert_eq!(
            settle(
                &mut guild,
                &mut Spoils::default(),
                &squires::Guild::default(),
                &BTreeMap::new()
            ),
            Ok(false)
        );
        assert_eq!((guild.delves, guild.rent_days), (1, 1));
    }
    #[test]
    fn old_kingdom_snapshot_migrates_the_yard_and_default_ledger_is_not_sent() {
        let mut r = yard();
        let value = serde_json::to_value(&r.kingdom).unwrap();
        assert!(value.get("squire_yard").is_none());
        let room = &mut r.dungeon.rooms[r.at];
        for s in 0..3 {
            let (x, y) = plate(s, 1);
            room.set(x as usize, y as usize, Tile::Block);
        }
        r.migrate_kingdom();
        for s in 0..3 {
            let (x, y) = plate(s, 1);
            assert_eq!(r.room().tile(x, y), Tile::Floor);
        }
        stand(&mut r, 0, 0);
        hold(&mut r, home::BUY_HOLD);
        let restored: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        assert_eq!(restored.squire_page(1, 0), 1);
        assert!(restored.valid_snapshot());
    }
}
