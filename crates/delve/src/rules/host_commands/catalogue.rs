//! Resolved host-file catalogue admission. Disk and media remain host resources;
//! the identified command journal owns atomic admission and replay.
use super::super::{Run, bosses, card_paint, cards};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BossSource {
    pub id: String,
    pub text: String,
    pub resolved: bosses::Boss,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seat {
    pub present: bool,
    pub epoch: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationError {
    Missing,
    OwnerChanged,
    SeatChanged,
    Changed,
}
impl std::fmt::Display for ObservationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Missing => "catalogue capture is missing observations",
            Self::OwnerChanged => "catalogue owner changed; captured batch cancelled",
            Self::SeatChanged => "catalogue delivery seat changed; captured delivery cancelled",
            Self::Changed => "catalogue stale: observation changed; recapture source",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observed {
    pub seat_one: Option<Seat>,
    pub session: String,
    pub raid: u64,
    pub cards: Vec<Option<cards::Card>>,
    pub bosses: Vec<Vec<bosses::Boss>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub observed: Option<Observed>,
    pub cards: Vec<cards::Card>,
    pub bosses: Vec<BossSource>,
    /// Polling delivers spells to seat one; explicit reload only installs.
    pub deliver_spells: bool,
}
impl Batch {
    pub fn observe(&mut self, run: &Run, session: &str) {
        self.observed = Some(Observed {
            seat_one: self.deliver_spells.then(|| Seat {
                present: run.players.contains_key(&1),
                epoch: run.seat_epoch(1),
            }),
            session: session.to_owned(),
            raid: run.raid_id,
            cards: self
                .cards
                .iter()
                .map(|c| run.book.get(&c.id).cloned())
                .collect(),
            bosses: self
                .bosses
                .iter()
                .map(|source| {
                    run.bosses
                        .iter()
                        .filter(|b| b.only_in == source.resolved.only_in)
                        .cloned()
                        .collect()
                })
                .collect(),
        });
    }

    pub(super) fn valid(&self) -> bool {
        let mut card_ids = std::collections::HashSet::new();
        let mut boss_targets = std::collections::HashSet::new();
        self.observed.as_ref().is_some_and(|o| {
            !o.session.is_empty()
                && o.session.len() <= 1024
                && o.seat_one.is_some() == self.deliver_spells
                && o.cards.len() == self.cards.len()
                && o.bosses.len() == self.bosses.len()
                && o.cards.iter().flatten().all(card_paint::valid_card)
                && o.bosses.iter().all(|v| v.len() <= 32)
        }) && self.cards.len() <= cards::MAX_BOOK
            && self.bosses.len() <= 8
            && self
                .cards
                .iter()
                .all(|card| card_ids.insert(&card.id) && card_paint::valid_card(card))
            && (!self.deliver_spells
                || (self.bosses.is_empty()
                    && self.cards.iter().all(|c| c.kind == cards::Kind::Spell)))
            && self.bosses.iter().all(|b| {
                boss_targets.insert(b.resolved.only_in)
                    && b.text.len() <= bosses::MAX_BYTES
                    && bosses::check(&b.id, &b.text).is_ok_and(|(checked, _)| checked == b.resolved)
            })
    }
    pub fn check_observed(&self, run: &Run, session: &str) -> Result<(), ObservationError> {
        let observed = self.observed.as_ref().ok_or(ObservationError::Missing)?;
        if observed.session != session || observed.raid != run.raid_id {
            return Err(ObservationError::OwnerChanged);
        }
        if self.deliver_spells
            && observed.seat_one
                != Some(Seat {
                    present: run.players.contains_key(&1),
                    epoch: run.seat_epoch(1),
                })
        {
            return Err(ObservationError::SeatChanged);
        }
        for (source, expected) in self.cards.iter().zip(&observed.cards) {
            if run.book.get(&source.id) != expected.as_ref() {
                return Err(ObservationError::Changed);
            }
        }
        for (source, expected) in self.bosses.iter().zip(&observed.bosses) {
            let current: Vec<_> = run
                .bosses
                .iter()
                .filter(|b| b.only_in == source.resolved.only_in)
                .cloned()
                .collect();
            if &current != expected {
                return Err(ObservationError::Changed);
            }
        }
        Ok(())
    }
    pub(super) fn deliver(&self, run: &mut Run, journal: &super::Journal) -> Result<bool, String> {
        self.check_observed(run, journal.session())
            .map_err(|e| e.to_string())?;
        let mut reserved = Vec::new();
        let mut recipe_cards = std::collections::BTreeSet::new();
        for claim in journal.pending_brews()? {
            if journal.brew_completion(&claim)?.is_none() {
                reserved.push(claim.work.result_id);
                recipe_cards.extend(claim.work.inputs.into_iter().map(|card| card.id));
                recipe_cards.insert(claim.work.placeholder.id);
            }
        }
        for claim in journal.pending_reforges()? {
            if journal.reforge_completion(&claim)?.is_none() {
                reserved.push(claim.work.result_id);
            }
        }
        if let Some(card) = self.cards.iter().find(|c| reserved.contains(&c.id)) {
            return Err(format!(
                "{}: catalogue result id is reserved by pending work",
                card.id
            ));
        }
        // Check the entire batch before mutation, including capacity and shared
        // held identities. Rejecting a later entry must not install an earlier one.
        let mut book = run.book.clone();
        let mut admitted = Vec::new();
        for source in &self.cards {
            let mut card = source.clone();
            let mut recharge_only = false;
            if let Some(current) = book.get(&card.id) {
                let source_owned = recipe_cards.contains(&card.id)
                    || run
                        .brewing
                        .iter()
                        .any(|ask| ask.cards.contains(&card.id) || ask.placeholder == card.id)
                    || run
                        .players
                        .values()
                        .filter_map(|hero| hero.offer.as_ref()?.binding.as_ref())
                        .any(|binding| {
                            binding
                                .spell
                                .as_ref()
                                .is_some_and(|spell| spell.id == card.id)
                                || binding.sources.iter().any(|source| {
                                    crate::rules::dead_wishes::wish_of(&card.id)
                                        == Some(source.wish.id.as_str())
                                })
                        });
                let held = source_owned
                    || run
                        .dungeon
                        .rooms
                        .iter()
                        .any(|room| room.items.iter().any(|item| item.card == card.id))
                    || run.home.wardrobe.knights.values().any(|kept| {
                        kept.arm.as_ref() == Some(&card.id) || kept.guard.as_ref() == Some(&card.id)
                    })
                    || run.players.values().any(|h| {
                        h.deck.contains(&card.id)
                            || h.hand.contains(&card.id)
                            || h.arm.as_ref() == Some(&card.id)
                            || h.guard_card.as_ref() == Some(&card.id)
                            || h.bar.iter().flatten().any(|slot| match slot {
                                cards::BarSlot::Spell(id) | cards::BarSlot::Draught(id) => {
                                    id == &card.id
                                }
                            })
                            || h.spells.iter().any(|s| s.as_ref() == Some(&card.id))
                    });
                if held {
                    // Recharge is read on the next spell cast. Existing timers,
                    // issued effects, authored appearance and makers stay owned
                    // by their current state. Recipes and open scrolls retain
                    // their complete captured definitions, including recharge.
                    let mut recharge = current.clone();
                    recharge.cooldown = card.cooldown;
                    let cooldown_only = current.kind == cards::Kind::Spell
                        && current.cooldown != card.cooldown
                        && !source_owned
                        && card_paint::same_definition(&recharge, &card);
                    if cooldown_only {
                        card = recharge;
                        recharge_only = true;
                    } else if card_paint::same_definition(current, &card)
                        && card_paint::retains_identity(current, &card)
                    {
                        card = current.clone();
                    } else {
                        return Err(format!(
                            "{}: a knight holds a different definition; use a distinct card id",
                            card.id
                        ));
                    }
                } else {
                    card.by = current.by.clone();
                }
            }
            if !book.insert(card.clone()) {
                return Err(format!("{}: the book is full", card.id));
            }
            admitted.push((
                !recharge_only && run.book.get(&card.id) != Some(&card),
                card,
            ));
        }
        let mut changed = book.cards != run.book.cards;
        run.book = book;
        for source in &self.bosses {
            let boss = &source.resolved;
            if !run.bosses.iter().any(|b| b == boss) {
                run.bosses.retain(|b| b.only_in != boss.only_in);
                run.bosses.push(boss.clone());
                changed = true;
            }
        }
        if self.deliver_spells {
            for (new, card) in admitted {
                let needs_slot = run.players.get(&1).is_some_and(|h| h.takes(&card));
                // Existing delivery owns slots, floor drops and the pickup cue.
                // Only missing spells are delivered on subsequent polls.
                let already = run
                    .players
                    .get(&1)
                    .is_some_and(|h| h.spells.iter().any(|s| s.as_ref() == Some(&card.id)));
                if new || (!already && needs_slot) {
                    changed |= run.add_card(card, 1);
                }
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Command, Journal, Reply, Request};
    use super::*;
    fn card(id: &str, cooldown: u32) -> cards::Card {
        cards::check(
            id,
            &format!("name Spark\nkind spell\ncooldown {cooldown}\ncast bolt\n"),
        )
        .unwrap()
        .card
    }
    fn changed_card(id: &str, cooldown: u32) -> cards::Card {
        cards::check(
            id,
            &format!("name Spark\nkind spell\ncooldown {cooldown}\ncast meteor\n"),
        )
        .unwrap()
        .card
    }
    fn apply(
        journal: &mut Journal,
        run: &mut Run,
        mut request: Request,
    ) -> Result<super::super::Receipt, String> {
        for command in &mut request.commands {
            if let Command::Catalogue { batch } = command {
                if batch.observed.is_none() {
                    batch.observe(run, journal.session());
                }
            }
        }
        journal.apply(run, request)
    }
    fn request(id: u64, batch: Batch) -> Request {
        Request {
            session: "catalogue-test".into(),
            raid: 9,
            id,
            commands: vec![Command::Catalogue { batch }],
        }
    }
    fn state(run: &Run) -> serde_json::Value {
        crate::rules::runtime_state::capture(run)
            .unwrap()
            .into_value()
    }
    #[test]
    fn catalogue_cooldown_only_update_preserves_recharge_visuals_issued_effects_and_replay() {
        for full_slots in [false, true] {
            let mut run = Run::new(3, 9, None);
            if full_slots {
                for slot in 0..cards::SPELL_SLOTS {
                    run.add_card(card(&format!("filler-{slot}"), 3), 1);
                }
            }
            let original = cards::check("catalogue-recharge", "name Recharge\nkind spell\nby Original maker\ncooldown 2\ncast bolt\nart\n00\n00\ndraw shot\n00\n00\n").unwrap().card;
            run.add_card(original.clone(), 1);
            run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 45;
            run.projectiles.push(crate::rules::Projectile {
                x: 8.0,
                y: 8.0,
                vx: 2.0,
                vy: 0.0,
                hostile: false,
                kind: crate::rules::Shot::Bolt,
                look: Some((original.id.clone(), "shot".into())),
                damage: 17,
                pierce: 1,
                last_hit: None,
                ttl: 90,
                empowered: false,
                traits: Default::default(),
            });
            run.cues.push("already issued".into());
            let before = state(&run);
            let wait = run.players[&1].spell_wait(&original);
            let mut authored = original.clone();
            authored.cooldown = 3;
            authored.by = "Different source author".into();
            authored.art = vec!["11".into(), "11".into()];
            authored.looks = Default::default();
            let mut batch = Batch {
                cards: vec![authored],
                deliver_spells: true,
                ..Default::default()
            };
            let mut journal = Journal::new("catalogue-test".into(), 9);
            batch.observe(&run, journal.session());
            let ask = request(1, batch);
            let receipt = journal.apply(&mut run, ask.clone()).unwrap();
            assert_eq!(receipt.result, Ok(vec![Reply::Changed(true)]));
            let mut expected = original;
            expected.cooldown = 3;
            assert_eq!(run.book.get(&expected.id), Some(&expected));
            assert_eq!(run.players[&1].spell_cooldowns[0], 45);
            assert_ne!(
                run.players[&1].spell_wait(&expected),
                wait,
                "future casts read the updated recharge"
            );
            let mut after = state(&run);
            after["run"]["book"] = before["run"]["book"].clone();
            assert_eq!(after, before, "cooldown metadata changed issued game state");
            assert!(run.valid_snapshot());
            let mut run = crate::rules::runtime_state::restore(
                crate::rules::runtime_state::capture(&run).unwrap(),
            )
            .unwrap();
            let mut journal =
                Journal::restore(journal.capture().unwrap(), "catalogue-test", 9).unwrap();
            run.score += 7;
            let before_replay = state(&run);
            assert_eq!(journal.apply(&mut run, ask).unwrap(), receipt);
            assert_eq!(state(&run), before_replay);
        }
    }

    #[test]
    fn catalogue_cooldown_changes_cannot_rewrite_queued_or_claimed_brew_sources() {
        let mut run = Run::new(0, 9, None);
        run.descend_for_test();
        queue_brew(&mut run);
        let queued = run.brewing[0].clone();
        let mut journal = Journal::new("catalogue-test".into(), 9);
        for id in [queued.cards[0].clone(), queued.placeholder.clone()] {
            let mut changed = run.book.get(&id).unwrap().clone();
            changed.cooldown += 1;
            let before = state(&run);
            let next = journal.next_id().unwrap();
            assert!(
                apply(
                    &mut journal,
                    &mut run,
                    request(
                        next,
                        Batch {
                            cards: vec![changed],
                            ..Default::default()
                        }
                    )
                )
                .unwrap()
                .result
                .is_err()
            );
            assert_eq!(state(&run), before);
        }
        let claim = journal.claim_brew(&mut run).unwrap().unwrap();
        for id in [&claim.work.inputs[0].id, &claim.work.placeholder.id] {
            let mut changed = run.book.get(id).unwrap().clone();
            changed.cooldown += 1;
            let before = state(&run);
            let next = journal.next_id().unwrap();
            assert!(
                apply(
                    &mut journal,
                    &mut run,
                    request(
                        next,
                        Batch {
                            cards: vec![changed],
                            ..Default::default()
                        }
                    )
                )
                .unwrap()
                .result
                .is_err()
            );
            assert_eq!(state(&run), before);
        }
        let run = crate::rules::runtime_state::restore(
            crate::rules::runtime_state::capture(&run).unwrap(),
        )
        .unwrap();
        let journal = Journal::restore(journal.capture().unwrap(), "catalogue-test", 9).unwrap();
        journal.validate_brew_world(&run).unwrap();
        assert_eq!(journal.pending_brews().unwrap(), vec![claim]);
    }

    #[test]
    fn catalogue_cooldown_changes_preserve_open_spell_and_wish_sources() {
        for wish_source in [false, true] {
            let mut run = Run::new(0, 9, None);
            run.descend_for_test();
            run.players.get_mut(&1).unwrap().wishes = 3;
            let id = if wish_source {
                let wish = crate::rules::phrasebook::read("wish frozen-recharge | Frozen Recharge\nkind spell\nsay frozen recharge\ntier cooldown 3; cast bolt :: Captured recharge\n").remove(0);
                let card = wish.card(0).unwrap();
                let id = card.id.clone();
                run.book.insert(card);
                run.offer_wish(1, &wish.name, Some(&wish)).unwrap();
                assert!(
                    run.players[&1]
                        .offer
                        .as_ref()
                        .unwrap()
                        .binding
                        .as_ref()
                        .unwrap()
                        .sources
                        .iter()
                        .any(|source| source.wish == wish)
                );
                id
            } else {
                let original = card("frozen-spell", 3);
                run.add_card(original.clone(), 1);
                run.offer_wish(1, "upgrade Spark", None).unwrap();
                assert!(
                    run.players[&1]
                        .offer
                        .as_ref()
                        .unwrap()
                        .binding
                        .as_ref()
                        .unwrap()
                        .spell
                        .is_some()
                );
                original.id
            };
            let mut changed = run.book.get(&id).unwrap().clone();
            changed.cooldown += 1;
            let before = state(&run);
            let mut journal = Journal::new("catalogue-test".into(), 9);
            assert!(
                apply(
                    &mut journal,
                    &mut run,
                    request(
                        1,
                        Batch {
                            cards: vec![changed],
                            ..Default::default()
                        }
                    )
                )
                .unwrap()
                .result
                .is_err()
            );
            assert_eq!(state(&run), before);
            let restored = crate::rules::runtime_state::restore(
                crate::rules::runtime_state::capture(&run).unwrap(),
            )
            .unwrap();
            crate::rules::scrolls::validate_run(&restored).unwrap();
            assert!(restored.valid_snapshot());
        }
    }

    #[test]
    fn catalogue_replay_after_handoff_does_not_deliver_twice_or_reset_cooldowns() {
        let mut run = Run::new(3, 9, Some("friend"));
        let mut journal = Journal::new("catalogue-test".into(), 9);
        let mut ask = request(
            1,
            Batch {
                cards: vec![card("catalogue-spark", 3)],
                deliver_spells: true,
                ..Default::default()
            },
        );
        if let Command::Catalogue { batch } = &mut ask.commands[0] {
            batch.observe(&run, journal.session());
        }
        let receipt = apply(&mut journal, &mut run, ask.clone()).unwrap();
        assert_eq!(receipt.result, Ok(vec![Reply::Changed(true)]));
        assert_eq!(
            run.players[&1].spells[0].as_deref(),
            Some("catalogue-spark")
        );
        assert!(run.players[&2].spells.iter().all(Option::is_none));
        run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 42;
        let before = state(&run);
        let checkpoint = crate::rules::runtime_state::capture(&run).unwrap();
        let mut run = crate::rules::runtime_state::restore(checkpoint).unwrap();
        let mut journal =
            Journal::restore(journal.capture().unwrap(), "catalogue-test", 9).unwrap();
        assert_eq!(apply(&mut journal, &mut run, ask).unwrap(), receipt);
        assert_eq!(state(&run), before);
        assert_eq!(run.players[&1].spell_cooldowns[0], 42);
    }
    #[test]
    fn catalogue_conflicting_held_source_rolls_back_the_entire_batch() {
        let mut run = Run::new(3, 9, None);
        run.add_card(card("catalogue-held", 3), 1);
        run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 17;
        let before = state(&run);
        let mut journal = Journal::new("catalogue-test".into(), 9);
        let receipt = apply(
            &mut journal,
            &mut run,
            request(
                1,
                Batch {
                    cards: vec![card("catalogue-new", 3), changed_card("catalogue-held", 4)],
                    deliver_spells: true,
                    ..Default::default()
                },
            ),
        )
        .unwrap();
        assert!(receipt.result.unwrap_err().contains("different definition"));
        assert_eq!(state(&run), before);
        assert!(run.book.get("catalogue-new").is_none());
    }
    #[test]
    fn catalogue_equivalent_held_source_preserves_canonical_author_and_visuals() {
        let mut run = Run::new(3, 9, None);
        let mut original = card("catalogue-held", 3);
        original.by = "Original maker".into();
        run.add_card(original.clone(), 1);
        run.makers
            .insert("catalogue-held".into(), (1, "Original maker".into()));
        let before = state(&run);
        let mut authored = original.clone();
        authored.by = "New source author".into();
        authored.art.clear();
        let mut journal = Journal::new("catalogue-test".into(), 9);
        let receipt = apply(
            &mut journal,
            &mut run,
            request(
                1,
                Batch {
                    cards: vec![authored],
                    ..Default::default()
                },
            ),
        )
        .unwrap();
        assert_eq!(receipt.result, Ok(vec![Reply::Changed(false)]));
        assert_eq!(run.book.get(&original.id), Some(&original));
        assert_eq!(state(&run), before);
    }
    #[test]
    fn catalogue_boss_source_is_checked_and_bad_candidate_cannot_install_cards() {
        let mut run = Run::new(3, 9, None);
        let boss = bosses::builtin().remove(0);
        let before = state(&run);
        let mut journal = Journal::new("catalogue-test".into(), 9);
        let result = apply(
            &mut journal,
            &mut run,
            request(
                1,
                Batch {
                    cards: vec![card("catalogue-new", 3)],
                    bosses: vec![BossSource {
                        id: boss.id.clone(),
                        text: "invalid".into(),
                        resolved: boss,
                    }],
                    ..Default::default()
                },
            ),
        );
        assert!(result.unwrap().result.is_err());
        assert_eq!(state(&run), before);
    }
    #[test]
    fn catalogue_retired_definition_can_change_and_boss_install_keeps_live_world() {
        let mut run = Run::new(3, 9, Some("friend"));
        let mut journal = Journal::new("catalogue-test".into(), 9);
        let initial = card("catalogue-unheld", 3);
        run.book.insert(initial);
        run.players.get_mut(&1).unwrap().hp = 21;
        run.players.get_mut(&2).unwrap().spell_cooldowns[0] = 7;
        let text = include_str!("../../../../../cockpit/assets/dungeon/bosses/the-big-ripper.boss")
            .to_owned();
        let (resolved, _) = bosses::check("catalogue-boss", &text).unwrap();
        let target = resolved.only_in;
        let receipt = apply(
            &mut journal,
            &mut run,
            request(
                1,
                Batch {
                    cards: vec![card("catalogue-unheld", 4)],
                    bosses: vec![BossSource {
                        id: "catalogue-boss".into(),
                        text,
                        resolved: resolved.clone(),
                    }],
                    ..Default::default()
                },
            ),
        )
        .unwrap();
        assert_eq!(receipt.result, Ok(vec![Reply::Changed(true)]));
        assert_eq!(
            run.book.get("catalogue-unheld").unwrap(),
            &card("catalogue-unheld", 4)
        );
        assert_eq!(
            run.bosses
                .iter()
                .filter(|b| b.only_in == target)
                .collect::<Vec<_>>(),
            vec![&resolved]
        );
        assert_eq!(run.players[&1].hp, 21);
        assert_eq!(run.players[&2].spell_cooldowns[0], 7);
    }
    #[test]
    fn catalogue_checks_all_held_reference_forms_before_replacing_a_definition() {
        for holder in 0..4 {
            let mut run = Run::new(3, 9, None);
            run.book.insert(card("catalogue-held", 3));
            let hero = run.players.get_mut(&1).unwrap();
            match holder {
                0 => hero.hand.push("catalogue-held".into()),
                1 => hero.arm = Some("catalogue-held".into()),
                2 => hero.guard_card = Some("catalogue-held".into()),
                _ => hero.bar[0] = Some(cards::BarSlot::Draught("catalogue-held".into())),
            }
            let before = state(&run);
            let mut journal = Journal::new("catalogue-test".into(), 9);
            let receipt = apply(
                &mut journal,
                &mut run,
                request(
                    1,
                    Batch {
                        cards: vec![changed_card("catalogue-held", 4)],
                        ..Default::default()
                    },
                ),
            )
            .unwrap();
            assert!(
                receipt.result.is_err(),
                "held reference {holder} was replaced"
            );
            assert_eq!(state(&run), before);
        }
    }
    #[test]
    fn catalogue_observations_and_owner_fences_reject_late_capture_atomically() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("catalogue-test".into(), 9);
        let mut batch = Batch {
            cards: vec![card("catalogue-late", 3)],
            ..Default::default()
        };
        batch.observe(&run, journal.session());
        run.book.insert(card("catalogue-late", 4));
        let before = state(&run);
        let receipt = apply(&mut journal, &mut run, request(1, batch.clone())).unwrap();
        assert!(receipt.result.unwrap_err().starts_with("catalogue stale:"));
        assert_eq!(state(&run), before);
        batch.observe(&run, "another-session");
        let receipt = apply(&mut journal, &mut run, request(2, batch.clone())).unwrap();
        assert!(
            receipt
                .result
                .unwrap_err()
                .starts_with("catalogue owner changed")
        );
        assert_eq!(state(&run), before);
        batch.observe(&run, journal.session());
        batch.observed.as_mut().unwrap().raid += 1;
        let receipt = apply(&mut journal, &mut run, request(3, batch)).unwrap();
        assert!(
            receipt
                .result
                .unwrap_err()
                .starts_with("catalogue owner changed")
        );
        assert_eq!(state(&run), before);
    }
    #[test]
    fn catalogue_floor_and_kept_gear_own_their_executable_definition() {
        for wardrobe in [false, true] {
            let mut run = Run::new(3, 9, None);
            run.book.insert(card("catalogue-owned", 3));
            if wardrobe {
                let mut kept = crate::rules::gear::Kept::equipped();
                kept.arm = Some("catalogue-owned".into());
                kept.guard = Some("catalogue-owned".into());
                run.home.wardrobe.knights.insert("Away knight".into(), kept);
            } else {
                run.add_card(card("catalogue-owned", 3), 99);
                // add_card for an absent player only installs; issue an actual
                // full-slot floor delivery through the production path instead.
                for (i, slot) in run
                    .players
                    .get_mut(&1)
                    .unwrap()
                    .spells
                    .iter_mut()
                    .enumerate()
                {
                    *slot = Some(format!("slot-{i}"));
                }
                run.add_card(card("catalogue-owned", 3), 1);
                assert!(run.room().items.iter().any(|i| i.card == "catalogue-owned"));
            }
            let before = state(&run);
            let mut journal = Journal::new("catalogue-test".into(), 9);
            let receipt = apply(
                &mut journal,
                &mut run,
                request(
                    1,
                    Batch {
                        cards: vec![changed_card("catalogue-owned", 4)],
                        ..Default::default()
                    },
                ),
            )
            .unwrap();
            assert!(receipt.result.is_err());
            assert_eq!(state(&run), before);
        }
    }
    fn queue_brew(run: &mut Run) {
        use crate::rules::cauldron;
        let placeholder = cauldron::brewed_card(
            cauldron::recipe(cauldron::Essence::Garden, cauldron::Essence::Garden),
            cauldron::Tier::Common,
        );
        run.players.get_mut(&1).unwrap().spells[0] = Some(placeholder.id.clone());
        run.brewing.push(cauldron::Brewing {
            knight: 1,
            raid: run.raid_id,
            depth: run.dungeon.depth,
            cards: ["cabbage-storm".into(), "hedge-maze".into()],
            placeholder: placeholder.id.clone(),
        });
        run.book.insert(placeholder);
    }
    #[test]
    fn catalogue_respects_queued_brew_inputs_and_journal_owned_future_result_ids() {
        let mut run = Run::new(0, 9, None);
        run.descend_for_test();
        queue_brew(&mut run);
        let mut journal = Journal::new("catalogue-test".into(), 9);
        let mut changed = run.book.get("cabbage-storm").unwrap().clone();
        changed.name = "Changed while queued".into();
        let before = state(&run);
        let receipt = apply(
            &mut journal,
            &mut run,
            request(
                1,
                Batch {
                    cards: vec![changed],
                    ..Default::default()
                },
            ),
        )
        .unwrap();
        assert!(receipt.result.is_err());
        assert_eq!(state(&run), before);
        let brew = journal.claim_brew(&mut run).unwrap().unwrap();
        let reforge = journal
            .claim_reforge(
                &mut run,
                1,
                crate::rules::knights::Part::Defense,
                "a silver shield".into(),
            )
            .unwrap();
        for result_id in [&brew.work.result_id, &reforge.work.result_id] {
            let before = state(&run);
            let id = journal.next_id().unwrap();
            let receipt = apply(
                &mut journal,
                &mut run,
                request(
                    id,
                    Batch {
                        cards: vec![card(result_id, 3)],
                        ..Default::default()
                    },
                ),
            )
            .unwrap();
            assert!(
                receipt
                    .result
                    .unwrap_err()
                    .contains("reserved by pending work")
            );
            assert!(run.book.get(result_id).is_none());
            assert_eq!(state(&run), before);
        }
        assert_eq!(journal.pending_brews().unwrap(), vec![brew.clone()]);
        assert_eq!(journal.pending_reforges().unwrap(), vec![reforge.clone()]);
        let raw = "name Frostbriar Orchard\nkind spell\nrarity rare\ntext Thorns, a crop and a cold wind.\ndrop 0\nchest 0\ncooldown 20\ncabbages 3\nhedge 1\nward 1\nart\n..fDDf..\n.fDEEDf.\nfDE44EDf\nfDE44EDf\n.fDEEDf.\n..fDDf..\n";
        let input = cards::check(&brew.work.result_id, raw).unwrap().card;
        let accepted = crate::rules::brew_jobs::PolicyRequest {
            work: brew.work.clone(),
            card: input.clone(),
            taken: crate::rules::cauldron::taken(&run.book),
        }
        .evaluate()
        .unwrap()
        .unwrap();
        assert!(
            journal
                .complete_brew(&mut run, &brew, input, accepted)
                .unwrap()
                .result
                .is_ok()
        );
        let mut guard = run
            .book
            .cards
            .iter()
            .find(|c| c.kind == cards::Kind::Guard)
            .unwrap()
            .clone();
        guard.id = reforge.work.result_id.clone();
        guard.name = "Silver Shield".into();
        let guard = cards::check(&guard.id, &guard.definition_file())
            .unwrap()
            .card;
        assert!(
            journal
                .complete_reforge(&mut run, &reforge, guard.clone(), guard)
                .unwrap()
                .result
                .is_ok()
        );
    }
    #[test]
    fn catalogue_stale_boss_observation_cannot_replace_a_newer_guardian() {
        let mut run = Run::new(3, 9, None);
        let mut journal = Journal::new("catalogue-test".into(), 9);
        let (id, text) = bosses::BUILTIN[0];
        let resolved = bosses::check(id, text).unwrap().0;
        let mut batch = Batch {
            bosses: vec![BossSource {
                id: id.into(),
                text: text.into(),
                resolved: resolved.clone(),
            }],
            ..Default::default()
        };
        batch.observe(&run, journal.session());
        run.bosses
            .iter_mut()
            .find(|b| b.only_in == resolved.only_in)
            .unwrap()
            .name = "Newer guardian".into();
        let before = state(&run);
        let receipt = apply(&mut journal, &mut run, request(1, batch)).unwrap();
        assert!(receipt.result.unwrap_err().starts_with("catalogue stale:"));
        assert_eq!(state(&run), before);
    }
    #[test]
    fn catalogue_duplicate_card_ids_or_boss_targets_reject_the_whole_batch() {
        for duplicate_boss in [false, true] {
            let mut run = Run::new(3, 9, None);
            let before = state(&run);
            let mut journal = Journal::new("catalogue-test".into(), 9);
            let batch = if duplicate_boss {
                let (id, text) = bosses::BUILTIN[0];
                let first = BossSource {
                    id: id.into(),
                    text: text.into(),
                    resolved: bosses::check(id, text).unwrap().0,
                };
                let mut second = first.clone();
                second.id = "other-guardian".into();
                second.text = text.replacen("name ", "name Other ", 1);
                second.resolved = bosses::check(&second.id, &second.text).unwrap().0;
                Batch {
                    cards: vec![card("catalogue-new", 3)],
                    bosses: vec![first, second],
                    ..Default::default()
                }
            } else {
                Batch {
                    cards: vec![
                        card("catalogue-new", 3),
                        card("catalogue-duplicate", 3),
                        card("catalogue-duplicate", 4),
                    ],
                    deliver_spells: true,
                    ..Default::default()
                }
            };
            let receipt = apply(&mut journal, &mut run, request(1, batch)).unwrap();
            assert!(receipt.result.is_err());
            assert_eq!(
                state(&run),
                before,
                "duplicate boss target: {duplicate_boss}"
            );
            assert!(run.book.get("catalogue-new").is_none());
            assert!(!run.sounds.contains(&"card_pickup"));
        }
    }
    #[test]
    fn catalogue_delivery_fences_first_seat_epoch_and_presence() {
        for presence_change in [false, true] {
            let mut run = Run::new(3, 9, None);
            let mut journal = Journal::new("catalogue-test".into(), 9);
            let mut batch = Batch {
                cards: vec![card("catalogue-delivery", 3)],
                deliver_spells: true,
                ..Default::default()
            };
            let old = if presence_change {
                run.players.remove(&1)
            } else {
                None
            };
            batch.observe(&run, journal.session());
            if let Some(hero) = old {
                run.players.insert(1, hero);
            } else {
                run.seat_first("Replacement occupant");
            }
            assert_eq!(
                batch.check_observed(&run, journal.session()),
                Err(ObservationError::SeatChanged)
            );
            let before = state(&run);
            let receipt = apply(&mut journal, &mut run, request(1, batch)).unwrap();
            assert!(
                receipt
                    .result
                    .unwrap_err()
                    .contains("delivery seat changed")
            );
            assert_eq!(state(&run), before);
            assert!(run.book.get("catalogue-delivery").is_none());
        }
    }
}
