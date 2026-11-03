use super::*;
use angel_delve::rules::host_commands::{
    Command, Reply, Request,
    catalogue::{Batch, ObservationError, Observed, Seat},
};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Origin {
    Cards(PathBuf, bool),
    Bosses(PathBuf),
}
struct Pending {
    origin: Origin,
    batch: Batch,
}
#[derive(Default)]
pub(super) struct Host {
    pending: VecDeque<Pending>,
    issue: Option<String>,
}

/// The native queue owns the exact resolved completion through admission
/// pressure/source removal. It is neither a second game receipt ledger nor a
/// watcher: admitted effects/replay remain exclusively in the existing Journal.
fn capture(shared: &Shared, origin: &Origin) -> (Batch, Vec<String>) {
    // Observe before IO. Resolving native files/sheets never holds the Run lock.
    let (session, raid, book, bosses, seat_one) = {
        let core = lock(&shared.core);
        (
            core.commands.session().to_owned(),
            core.run.raid_id,
            core.run.book.clone(),
            core.run.bosses.clone(),
            Seat {
                present: core.run.players.contains_key(&1),
                epoch: core.run.seat_epoch(1),
            },
        )
    };
    let (mut batch, report) = match origin {
        Origin::Cards(path, spells) => {
            let (mut cards, report) = crate::together_files::capture_cards(path);
            if *spells {
                cards.retain(|c| c.kind == cards::Kind::Spell);
            }
            (
                Batch {
                    cards,
                    deliver_spells: *spells,
                    ..Default::default()
                },
                report,
            )
        }
        Origin::Bosses(path) => {
            let (bosses, report) = crate::together_files::capture_bosses(path);
            (
                Batch {
                    bosses,
                    ..Default::default()
                },
                report,
            )
        }
    };
    batch.observed = Some(Observed {
        seat_one: batch.deliver_spells.then_some(seat_one),
        session,
        raid,
        cards: batch
            .cards
            .iter()
            .map(|c| book.get(&c.id).cloned())
            .collect(),
        bosses: batch
            .bosses
            .iter()
            .map(|source| {
                bosses
                    .iter()
                    .filter(|b| b.only_in == source.resolved.only_in)
                    .cloned()
                    .collect()
            })
            .collect(),
    });
    (batch, report)
}
enum Attempt {
    Accepted(bool),
    Deferred(String),
    Refused(String),
    Invalidated(ObservationError),
}
fn attempt(core: &mut Core, batch: Batch) -> Attempt {
    let Core {
        run,
        commands,
        edited,
        ..
    } = core;
    if let Err(error) = batch.check_observed(run, commands.session()) {
        return Attempt::Invalidated(error);
    }
    let id = match commands.next_id() {
        Ok(id) => id,
        Err(e) => return Attempt::Deferred(e),
    };
    let request = Request {
        session: commands.session().to_owned(),
        raid: run.raid_id,
        id,
        commands: vec![Command::Catalogue { batch }],
    };
    if let Err(error) = request.validate_payload() {
        return Attempt::Refused(error);
    }
    // An unadmitted request must preserve its exact payload. An admitted
    // refusal is authoritative feedback and may be corrected by new source.
    let receipt = match commands.apply(run, request) {
        Ok(receipt) => receipt,
        Err(error) => return Attempt::Deferred(error),
    };
    let outcome = match receipt.result {
        Ok(replies) => match replies.as_slice() {
            [Reply::Changed(changed)] => {
                *edited |= *changed;
                Attempt::Accepted(*changed)
            }
            _ => Attempt::Refused("catalogue command returned an invalid reply".into()),
        },
        Err(error) => Attempt::Refused(error),
    };
    match commands.acknowledge(run.raid_id, receipt.id) {
        Ok(()) => outcome,
        Err(error) => Attempt::Deferred(error),
    }
}
fn empty(batch: &Batch) -> bool {
    batch.cards.is_empty() && batch.bosses.is_empty()
}
fn enqueue(shared: &Shared, host: &mut Host, origin: Origin) -> Vec<String> {
    if host.pending.iter().any(|p| p.origin == origin) {
        return vec!["catalogue: captured batch retained; admission will retry".into()];
    }
    // At most one cards/manual, cards/spell and bosses source per session.
    // Do not discard an older capture to make room for a different directory.
    if host.pending.len() >= 4 {
        return vec![
            "catalogue: pending source capacity reached; reload again after admission".into(),
        ];
    }
    let (batch, report) = capture(shared, &origin);
    if !empty(&batch) {
        host.pending.push_back(Pending { origin, batch });
    }
    report
}
enum Refresh {
    Same,
    Replacement(Pending),
    Empty,
    OwnerChanged,
}
fn recapture(shared: &Shared, pending: &Pending, changed_source_only: bool) -> Refresh {
    let (batch, _) = capture(shared, &pending.origin);
    let same_owner = batch
        .observed
        .as_ref()
        .zip(pending.batch.observed.as_ref())
        .is_some_and(|(a, b)| {
            a.session == b.session && a.raid == b.raid && a.seat_one == b.seat_one
        });
    if !same_owner {
        return Refresh::OwnerChanged;
    }
    let source_changed = batch.cards != pending.batch.cards || batch.bosses != pending.batch.bosses;
    if changed_source_only && !source_changed {
        return Refresh::Same;
    }
    if empty(&batch) {
        return Refresh::Empty;
    }
    Refresh::Replacement(Pending {
        origin: pending.origin.clone(),
        batch,
    })
}
fn retry(shared: &Shared, host: &mut Host) -> Vec<String> {
    let mut report = Vec::new();
    let count = host.pending.len();
    for _ in 0..count {
        let pending = host.pending.pop_front().unwrap();
        let result = attempt(&mut lock(&shared.core), pending.batch.clone());
        match result {
            Attempt::Accepted(_) => {
                // A no-op may have waited while App consumed a newer stamp.
                if let Refresh::Replacement(next) = recapture(shared, &pending, true) {
                    host.pending.push_back(next);
                }
            }
            Attempt::Invalidated(error) => {
                report.push(error.to_string());
                if error == ObservationError::Changed {
                    if let Refresh::Replacement(next) = recapture(shared, &pending, false) {
                        host.pending.push_back(next);
                    }
                }
            }
            Attempt::Deferred(error) => {
                report.push(format!("catalogue: {error}; captured batch retained"));
                host.pending.push_back(pending);
            }
            Attempt::Refused(error) => {
                report.push(format!(
                    "catalogue: {error}; awaiting source correction or owner release"
                ));
                match recapture(shared, &pending, true) {
                    Refresh::Replacement(next) => host.pending.push_back(next),
                    Refresh::Same => host.pending.push_back(pending),
                    Refresh::Empty | Refresh::OwnerChanged => report.push(
                        "catalogue: refused capture cancelled after source/owner change".into(),
                    ),
                }
            }
        }
    }
    report
}
fn reload(shared: &Shared, origin: Origin) -> Vec<String> {
    let mut host = lock(&shared.catalogue);
    let mut report = enqueue(shared, &mut host, origin);
    report.extend(retry(shared, &mut host));
    report
}
pub(super) fn poll(shared: &Shared, cards: Option<&Path>) {
    let mut host = lock(&shared.catalogue);
    let had_origin = cards.is_some_and(|path| {
        host.pending
            .iter()
            .any(|p| p.origin == Origin::Cards(path.to_owned(), true))
    });
    let mut issues = retry(shared, &mut host);
    if let Some(path) = cards.filter(|_| !had_origin) {
        let origin = Origin::Cards(path.to_owned(), true);
        if !host.pending.iter().any(|p| p.origin == origin) {
            let _ = enqueue(shared, &mut host, origin);
            issues.extend(retry(shared, &mut host));
        }
    }
    let issue = issues.into_iter().next();
    if issue != host.issue {
        if let Some(error) = &issue {
            tracing::warn!(%error, "catalogue admission awaits retry or source recapture");
        }
        host.issue = issue;
    }
}
impl Sim {
    pub fn load_catalogue_cards(&mut self, path: &Path) -> Vec<String> {
        let Some(live) = self.live.as_mut() else {
            return Vec::new();
        };
        live.view.take();
        reload(live.shared(), Origin::Cards(path.to_owned(), false))
    }
    pub fn load_catalogue_bosses(&mut self, path: &Path) -> Vec<String> {
        let Some(live) = self.live.as_mut() else {
            return Vec::new();
        };
        live.view.take();
        reload(live.shared(), Origin::Bosses(path.to_owned()))
    }
}
#[cfg(test)]
fn admit_spells(core: &mut Core, cards: Vec<cards::Card>) -> Result<bool, String> {
    let mut batch = Batch {
        cards,
        deliver_spells: true,
        ..Default::default()
    };
    batch.observe(&core.run, core.commands.session());
    match attempt(core, batch) {
        Attempt::Accepted(changed) => Ok(changed),
        Attempt::Deferred(e) | Attempt::Refused(e) => Err(e),
        Attempt::Invalidated(e) => Err(e.to_string()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn core() -> Core {
        let run = Run::new(13, 9, Some("friend"));
        Core {
            run,
            commands: angel_delve::rules::host_commands::Journal::new("catalogue-native".into(), 9),
            abilities: Default::default(),
            engine: None,
            engine_error: None,
            edited: false,
        }
    }
    fn spell(id: &str, cooldown: u32) -> cards::Card {
        cards::check(
            id,
            &format!("name Spark\nkind spell\ncooldown {cooldown}\ncast bolt\n"),
        )
        .unwrap()
        .card
    }
    fn changed_spell(id: &str, cooldown: u32) -> cards::Card {
        cards::check(
            id,
            &format!("name Spark\nkind spell\ncooldown {cooldown}\ncast meteor\n"),
        )
        .unwrap()
        .card
    }
    #[test]
    fn catalogue_service_updates_only_spell_cooldown_without_redelivery() {
        let mut core = core();
        let mut original = spell("native-recharge", 2);
        original.by = "Original maker".into();
        assert!(admit_spells(&mut core, vec![original.clone()]).unwrap());
        core.run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 45;
        core.run.cues.push("already issued".into());
        let slots = core.run.players[&1].spells.clone();
        let sounds = core.run.sounds.clone();
        let blasts = core.run.blasts.clone();
        let items = serde_json::to_value(&core.run.room().items).unwrap();
        let mut source = spell("native-recharge", 3);
        source.by = "Different source author".into();
        assert!(admit_spells(&mut core, vec![source.clone()]).unwrap());
        assert!(!admit_spells(&mut core, vec![source]).unwrap());
        original.cooldown = 3;
        assert_eq!(core.run.book.get(&original.id), Some(&original));
        assert_eq!(core.run.players[&1].spell_cooldowns[0], 45);
        assert_eq!(core.run.players[&1].spells, slots);
        assert_eq!(core.run.sounds, sounds);
        assert_eq!(core.run.blasts, blasts);
        assert_eq!(serde_json::to_value(&core.run.room().items).unwrap(), items);
        assert!(core.run.cues.iter().any(|cue| cue == "already issued"));
    }
    #[test]
    fn catalogue_service_rejects_changed_held_source_without_resetting_live_state() {
        let mut core = core();
        assert!(admit_spells(&mut core, vec![spell("native-spark", 3)]).unwrap());
        core.run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 27;
        core.edited = false;
        let original = core.run.book.get("native-spark").unwrap().clone();
        let sounds = core.run.sounds.clone();
        assert!(
            admit_spells(
                &mut core,
                vec![spell("native-new", 3), changed_spell("native-spark", 4)]
            )
            .is_err()
        );
        assert_eq!(core.run.book.get("native-spark"), Some(&original));
        assert!(core.run.book.get("native-new").is_none());
        assert_eq!(core.run.players[&1].spell_cooldowns[0], 27);
        assert_eq!(core.run.sounds, sounds);
        assert!(!core.edited);
        assert!(!admit_spells(&mut core, vec![spell("native-spark", 3)]).unwrap());
    }
    #[test]
    fn catalogue_service_full_slots_drop_once_then_equip_when_a_slot_opens() {
        let mut core = core();
        let cards: Vec<_> = (0..5)
            .map(|i| spell(&format!("native-spell-{i}"), 3))
            .collect();
        assert!(admit_spells(&mut core, cards.clone()).unwrap());
        assert!(core.run.players[&1].spells.iter().all(Option::is_some));
        let items = core.run.room().items.len();
        assert!(
            core.run
                .room()
                .items
                .iter()
                .any(|i| i.card == "native-spell-4")
        );
        assert!(!admit_spells(&mut core, cards.clone()).unwrap());
        assert_eq!(core.run.room().items.len(), items);
        let original = core.run.book.get("native-spell-4").unwrap().clone();
        assert!(admit_spells(&mut core, vec![changed_spell("native-spell-4", 4)]).is_err());
        assert_eq!(core.run.book.get("native-spell-4"), Some(&original));
        assert_eq!(core.run.room().items.len(), items);
        let issued = serde_json::to_value(&core.run.room().items).unwrap();
        assert!(admit_spells(&mut core, vec![spell("native-spell-4", 4)]).unwrap());
        assert_eq!(
            serde_json::to_value(&core.run.room().items).unwrap(),
            issued
        );
        assert!(!admit_spells(&mut core, vec![spell("native-spell-4", 4)]).unwrap());
        assert_eq!(
            serde_json::to_value(&core.run.room().items).unwrap(),
            issued
        );
        core.run.players.get_mut(&1).unwrap().spells[0] = None;
        assert!(admit_spells(&mut core, vec![spell("native-spell-4", 4)]).unwrap());
        assert_eq!(
            core.run.players[&1].spells[0].as_deref(),
            Some("native-spell-4")
        );
        assert!(core.run.players[&2].spells.iter().all(Option::is_none));
    }
    fn shared() -> Shared {
        let core = core();
        Shared::new(core.run, Some(core.commands))
    }
    fn directory(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("angel-catalogue-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
    fn pressure(shared: &Shared) -> Vec<u64> {
        let mut core = lock(&shared.core);
        let mut ids = Vec::new();
        for _ in 0..64 {
            let Core { run, commands, .. } = &mut *core;
            let id = commands.next_id().unwrap();
            commands
                .apply(
                    run,
                    Request {
                        session: commands.session().into(),
                        raid: run.raid_id,
                        id,
                        commands: vec![Command::DropOffer { player: 1 }],
                    },
                )
                .unwrap();
            ids.push(id);
        }
        ids
    }
    fn release(shared: &Shared, ids: Vec<u64>) {
        let mut core = lock(&shared.core);
        for id in ids {
            core.commands.acknowledge(9, id).unwrap();
        }
    }
    #[test]
    fn catalogue_app_retains_exact_capture_after_receipt_pressure_and_source_removal() {
        let shared = shared();
        let dir = directory("app-pressure");
        std::fs::write(
            dir.join("captured.card"),
            spell("captured", 3).definition_file(),
        )
        .unwrap();
        let ids = pressure(&shared);
        let report = reload(&shared, Origin::Cards(dir.clone(), false));
        assert!(
            report
                .iter()
                .any(|line| line.contains("captured batch retained"))
        );
        let exact = lock(&shared.catalogue).pending[0].batch.clone();
        std::fs::remove_file(dir.join("captured.card")).unwrap();
        poll(&shared, None);
        assert_eq!(lock(&shared.catalogue).pending[0].batch, exact);
        release(&shared, ids);
        poll(&shared, None);
        assert_eq!(
            lock(&shared.core).run.book.get("captured"),
            Some(&spell("captured", 3))
        );
        assert!(lock(&shared.catalogue).pending.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn catalogue_service_retains_exact_spell_through_pressure_then_takes_new_source_once() {
        let shared = shared();
        let dir = directory("spell-pressure");
        std::fs::write(
            dir.join("captured.card"),
            spell("captured", 3).definition_file(),
        )
        .unwrap();
        let ids = pressure(&shared);
        poll(&shared, Some(&dir));
        let exact = lock(&shared.catalogue).pending[0].batch.clone();
        std::fs::write(
            dir.join("captured.card"),
            changed_spell("captured", 4).definition_file(),
        )
        .unwrap();
        poll(&shared, Some(&dir));
        assert_eq!(lock(&shared.catalogue).pending[0].batch, exact);
        release(&shared, ids);
        poll(&shared, Some(&dir));
        let core = lock(&shared.core);
        assert_eq!(core.run.book.get("captured"), Some(&spell("captured", 3)));
        assert_eq!(core.run.players[&1].spells[0].as_deref(), Some("captured"));
        let cues = core.run.sounds.len();
        drop(core);
        poll(&shared, Some(&dir));
        assert_eq!(lock(&shared.core).run.sounds.len(), cues);
        assert_eq!(
            lock(&shared.catalogue).pending[0].batch.cards[0],
            changed_spell("captured", 4)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn catalogue_late_capture_recaptures_changed_source_and_cancels_old_session() {
        let shared = shared();
        let dir = directory("stale");
        std::fs::write(
            dir.join("captured.card"),
            spell("captured", 3).definition_file(),
        )
        .unwrap();
        let origin = Origin::Cards(dir.clone(), false);
        let (batch, _) = capture(&shared, &origin);
        lock(&shared.core).run.book.insert(spell("captured", 4));
        std::fs::write(
            dir.join("captured.card"),
            spell("captured", 5).definition_file(),
        )
        .unwrap();
        lock(&shared.catalogue)
            .pending
            .push_back(Pending { origin, batch });
        poll(&shared, None);
        assert_eq!(
            lock(&shared.core).run.book.get("captured"),
            Some(&spell("captured", 4))
        );
        assert_eq!(
            lock(&shared.catalogue).pending[0].batch.cards[0],
            spell("captured", 5)
        );
        poll(&shared, None);
        assert_eq!(
            lock(&shared.core).run.book.get("captured"),
            Some(&spell("captured", 5))
        );
        // A separate capture from the old session cannot enter a new owner.
        let origin = Origin::Cards(dir.clone(), false);
        let (batch, _) = capture(&shared, &origin);
        lock(&shared.catalogue)
            .pending
            .push_back(Pending { origin, batch });
        lock(&shared.core).commands =
            angel_delve::rules::host_commands::Journal::new("new-owner".into(), 9);
        poll(&shared, None);
        assert!(lock(&shared.catalogue).pending.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn catalogue_noop_pending_capture_still_recaptures_a_newer_app_source() {
        let shared = shared();
        let dir = directory("noop-recheck");
        lock(&shared.core).run.book.insert(spell("captured", 3));
        std::fs::write(
            dir.join("captured.card"),
            spell("captured", 3).definition_file(),
        )
        .unwrap();
        let ids = pressure(&shared);
        reload(&shared, Origin::Cards(dir.clone(), false));
        std::fs::write(
            dir.join("captured.card"),
            spell("captured", 4).definition_file(),
        )
        .unwrap();
        // The App has already consumed its stamp; no further reload callback.
        release(&shared, ids);
        poll(&shared, None);
        assert_eq!(
            lock(&shared.catalogue).pending[0].batch.cards[0],
            spell("captured", 4)
        );
        poll(&shared, None);
        assert_eq!(
            lock(&shared.core).run.book.get("captured"),
            Some(&spell("captured", 4))
        );
        assert!(lock(&shared.catalogue).pending.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn catalogue_refused_source_can_be_corrected_without_hiding_distinct_new_cards() {
        let shared = shared();
        let dir = directory("refusal-corrected");
        lock(&shared.core).run.add_card(spell("held", 3), 1);
        std::fs::write(
            dir.join("held.card"),
            changed_spell("held", 4).definition_file(),
        )
        .unwrap();
        poll(&shared, Some(&dir));
        assert_eq!(
            lock(&shared.catalogue).pending[0].batch.cards,
            vec![changed_spell("held", 4)]
        );
        // Logical refusal is admitted feedback: a corrected source can supersede
        // that refused payload, unlike a request still waiting on receipt space.
        std::fs::write(dir.join("held.card"), spell("held", 3).definition_file()).unwrap();
        std::fs::write(
            dir.join("distinct.card"),
            spell("distinct", 3).definition_file(),
        )
        .unwrap();
        poll(&shared, Some(&dir));
        assert_eq!(lock(&shared.catalogue).pending[0].batch.cards.len(), 2);
        poll(&shared, Some(&dir));
        let core = lock(&shared.core);
        assert_eq!(core.run.book.get("held"), Some(&spell("held", 3)));
        assert_eq!(core.run.players[&1].spells[1].as_deref(), Some("distinct"));
        assert!(lock(&shared.catalogue).pending.is_empty());
        drop(core);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn catalogue_unchanged_refused_source_retries_when_its_owner_releases() {
        let shared = shared();
        let dir = directory("owner-released");
        lock(&shared.core).run.add_card(spell("held", 3), 1);
        std::fs::write(
            dir.join("held.card"),
            changed_spell("held", 4).definition_file(),
        )
        .unwrap();
        poll(&shared, Some(&dir));
        let exact = lock(&shared.catalogue).pending[0].batch.clone();
        poll(&shared, Some(&dir));
        assert_eq!(lock(&shared.catalogue).pending[0].batch, exact);
        {
            let mut core = lock(&shared.core);
            let hero = core.run.players.get_mut(&1).unwrap();
            hero.spells = Default::default();
            hero.bar = Default::default();
        }
        poll(&shared, Some(&dir));
        assert_eq!(
            lock(&shared.core).run.book.get("held"),
            Some(&changed_spell("held", 4))
        );
        assert!(lock(&shared.catalogue).pending.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn catalogue_pending_spell_delivery_cannot_cross_first_seat_incarnation() {
        let shared = shared();
        let dir = directory("seat-reused");
        std::fs::write(
            dir.join("captured.card"),
            spell("captured", 3).definition_file(),
        )
        .unwrap();
        let ids = pressure(&shared);
        poll(&shared, Some(&dir));
        let epoch = lock(&shared.catalogue).pending[0]
            .batch
            .observed
            .as_ref()
            .unwrap()
            .seat_one
            .as_ref()
            .unwrap()
            .epoch;
        lock(&shared.core).run.seat_first("Replacement occupant");
        assert_ne!(lock(&shared.core).run.seat_epoch(1), epoch);
        release(&shared, ids);
        // This invocation retires the old capture; it cannot manufacture a new
        // capture for the replacement seat in the same retry pass.
        poll(&shared, Some(&dir));
        let core = lock(&shared.core);
        assert!(core.run.book.get("captured").is_none());
        assert!(core.run.players[&1].spells.iter().all(Option::is_none));
        assert!(lock(&shared.catalogue).pending.is_empty());
        drop(core);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn catalogue_oversized_capture_can_recover_after_source_correction() {
        let shared = shared();
        let dir = directory("oversized-corrected");
        let frame = "f".repeat(32);
        let rows = (0..24)
            .map(|_| [frame.as_str(); 4].join("|"))
            .collect::<Vec<_>>()
            .join("\n");
        let raw = format!("name Large\nkind hold\ndamage 1\ndraw aura\n{rows}\n");
        // Individually checked source files fit their card bounds; their combined
        // captured request exceeds the centralized Journal payload budget.
        cards::check("large-000", &raw).unwrap();
        for i in 0..320 {
            std::fs::write(dir.join(format!("large-{i:03}.card")), &raw).unwrap();
        }
        reload(&shared, Origin::Cards(dir.clone(), false));
        assert_eq!(lock(&shared.catalogue).pending.len(), 1);
        assert_eq!(lock(&shared.core).commands.next_id().unwrap(), 1);
        assert!(lock(&shared.core).run.book.get("large-000").is_none());
        for i in 1..320 {
            std::fs::remove_file(dir.join(format!("large-{i:03}.card"))).unwrap();
        }
        std::fs::write(
            dir.join("large-000.card"),
            spell("large-000", 3).definition_file(),
        )
        .unwrap();
        poll(&shared, None); // Structural refusal permits a changed-source recapture.
        assert_eq!(
            lock(&shared.catalogue).pending[0].batch.cards,
            vec![spell("large-000", 3)]
        );
        poll(&shared, None);
        assert_eq!(
            lock(&shared.core).run.book.get("large-000"),
            Some(&spell("large-000", 3))
        );
        assert!(lock(&shared.catalogue).pending.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
