use super::*;

/// A durable view of a vote export claim. `request` is the original Journal
/// receipt identity, not a newly allocated acknowledgement request.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoteBatch {
    pub session: String,
    pub raid: u64,
    pub request: u64,
    pub records: votes::Records,
    pub seen: std::collections::BTreeSet<String>,
    pub changes: Vec<votes::Change>,
}

/// Reconstruct changes lost only from the transient delta queue. Run records
/// and the realm snapshot are both durable; their difference recovers clear,
/// untried and rating edits after a restart before a claim was checkpointed.
pub(super) fn vote_delta(
    records: &votes::Records,
    baseline: &votes::Records,
) -> Vec<votes::Change> {
    let players: std::collections::BTreeSet<_> =
        records.keys().chain(baseline.keys()).cloned().collect();
    let mut changes = Vec::new();
    for player in players {
        let items: std::collections::BTreeSet<_> = records
            .get(&player)
            .into_iter()
            .flat_map(|ratings| ratings.keys())
            .chain(
                baseline
                    .get(&player)
                    .into_iter()
                    .flat_map(|ratings| ratings.keys()),
            )
            .cloned()
            .collect();
        for item in items {
            let rating = records
                .get(&player)
                .and_then(|ratings| ratings.get(&item))
                .copied();
            let old = baseline
                .get(&player)
                .and_then(|ratings| ratings.get(&item))
                .copied();
            if rating != old {
                changes.push(votes::Change {
                    player: player.clone(),
                    item,
                    rating,
                });
            }
        }
    }
    changes
}

impl Journal {
    pub fn votes_ready(&self, run: &Run) -> bool {
        self.votes_initialized && run.playtest.ready
    }

    /// Initialize once from realm facts. The durable Journal marker prevents
    /// a restart from replacing newer run facts with a stale realm snapshot.
    pub fn prepare_votes(
        &mut self,
        run: &mut Run,
        records: votes::Records,
        seen: std::collections::BTreeSet<String>,
        couch: bool,
    ) -> Result<(), String> {
        if run.raid_id != self.raid {
            return Err("playtest vote preparation belongs to another raid".into());
        }
        if run.playtest.ready && self.votes_initialized {
            return Ok(());
        }
        // While an export receipt is pending, its captured snapshot is the
        // baseline: the realm may still be older because IO has not finished.
        // Recover only edits made after that claim, never its owned queue.
        let (records, seen) = self
            .pending_votes()?
            .map(|batch| (batch.records, batch.seen))
            .unwrap_or((records, seen));
        // Schema-5 private checkpoints retained vote rows/seats but not the
        // Journal initialization marker. Preserve those durable facts when
        // the transient `ready` latch is absent after restore.
        let has_durable_vote_facts = !run.playtest.records.is_empty()
            || !run.playtest.seen.is_empty()
            || !run.playtest.seats.is_empty();
        let seats = run
            .players
            .iter()
            .map(|(&id, hero)| {
                (
                    id,
                    hero.name.clone(),
                    run.seat_epochs.get(&id).copied().unwrap_or(0),
                )
            })
            .collect();
        let receipt = self.apply(
            run,
            Request {
                session: self.session.clone(),
                raid: self.raid,
                id: self.next_id()?,
                commands: vec![Command::PrepareVotes {
                    seats,
                    records,
                    seen,
                    couch,
                    initialize: !self.votes_initialized && !has_durable_vote_facts,
                }],
            },
        )?;
        let result = receipt.result.map(|_| ());
        self.acknowledge(self.raid, receipt.id)?;
        result
    }

    pub fn pending_votes(&self) -> Result<Option<VoteBatch>, String> {
        let Some(record) = self
            .records
            .iter()
            .find(|record| matches!(record.commands.as_slice(), [Command::ClaimVotes { .. }]))
        else {
            return Ok(None);
        };
        let [
            Command::ClaimVotes {
                records,
                seen,
                changes,
            },
        ] = record.commands.as_slice()
        else {
            unreachable!();
        };
        if record.receipt.result.is_err() {
            return Err("retained playtest vote export claim was rejected".into());
        }
        Ok(Some(VoteBatch {
            session: self.session.clone(),
            raid: self.raid,
            request: record.receipt.id,
            records: records.clone(),
            seen: seen.clone(),
            changes: changes.clone(),
        }))
    }

    /// Persist the exact export work in the bounded Journal before any realm
    /// or file IO. Repeating this method returns the same receipt and rows.
    pub fn claim_votes(&mut self, run: &mut Run) -> Result<Option<VoteBatch>, String> {
        if run.raid_id != self.raid {
            return Err("playtest vote export belongs to another raid".into());
        }
        let rejected = self.records.iter().find_map(|record| {
            (matches!(record.commands.as_slice(), [Command::ClaimVotes { .. }])
                && record.receipt.result.is_err())
            .then_some(record.receipt.id)
        });
        if let Some(id) = rejected {
            self.acknowledge(self.raid, id)?;
        } else if let Some(batch) = self.pending_votes()? {
            return Ok(Some(batch));
        }
        if !self.votes_initialized || !run.playtest.ready {
            return Err("playtest votes are not initialized".into());
        }
        if !run.playtest.dirty && run.playtest.changes.is_empty() {
            return Ok(None);
        }
        let receipt = self.apply(
            run,
            Request {
                session: self.session.clone(),
                raid: self.raid,
                id: self.next_id()?,
                commands: vec![Command::ClaimVotes {
                    records: run.playtest.records.clone(),
                    seen: run.playtest.seen.clone(),
                    changes: run.playtest.changes.clone(),
                }],
            },
        )?;
        if let Err(error) = receipt.result {
            self.acknowledge(self.raid, receipt.id)?;
            return Err(error);
        }
        self.pending_votes()
    }

    /// Retire the original persisted claim. A changed seat epoch does not
    /// invalidate an already-issued rating; request identity fences retries.
    pub fn acknowledge_votes(&mut self, run: &mut Run, batch: &VoteBatch) -> Result<(), String> {
        if batch.session != self.session || batch.raid != self.raid || run.raid_id != self.raid {
            return Err("playtest vote acknowledgement belongs to another session".into());
        }
        let record = self
            .records
            .iter()
            .find(|record| record.receipt.id == batch.request)
            .ok_or("playtest vote export receipt is no longer pending")?;
        let expected = [Command::ClaimVotes {
            records: batch.records.clone(),
            seen: batch.seen.clone(),
            changes: batch.changes.clone(),
        }];
        if record.commands.as_slice() != expected.as_slice() || record.receipt.result.is_err() {
            return Err("playtest vote acknowledgement differs from its durable claim".into());
        }

        let mut candidate = run.clone();
        // The captured prefix was transferred to the durable claim before
        // external IO. Never consume the current queue by value: after a
        // restore it may contain new events equal to the old claim's values.
        candidate.playtest.dirty = !candidate.playtest.changes.is_empty()
            || candidate.playtest.records != batch.records
            || candidate.playtest.seen != batch.seen;
        let index = self
            .records
            .iter()
            .position(|record| record.receipt.id == batch.request)
            .expect("validated vote receipt remains retained");
        let retired = self
            .records
            .remove(index)
            .expect("validated vote receipt remains retained");
        self.held_bytes -= retired.held_bytes;
        *run = candidate;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: u64, command: Command) -> Request {
        Request {
            session: "vote-session".into(),
            raid: 9,
            id,
            commands: vec![command],
        }
    }

    fn initialized() -> (Run, Journal) {
        let mut run = Run::new(7, 9, Some("Guest"));
        let mut journal = Journal::new("vote-session".into(), 9);
        journal
            .prepare_votes(&mut run, votes::Records::new(), Default::default(), true)
            .unwrap();
        (run, journal)
    }

    #[test]
    fn preparation_rejects_stale_seats_or_full_journal_without_partial_initialization() {
        let mut run = Run::new(7, 9, Some("Guest"));
        let mut journal = Journal::new("vote-session".into(), 9);
        run.playtest.seats.insert(
            1,
            votes::Seat {
                open: true,
                ..Default::default()
            },
        );
        let before = run.playtest.clone();
        let reply = journal
            .apply(
                &mut run,
                request(
                    1,
                    Command::PrepareVotes {
                        seats: Vec::new(),
                        records: votes::Records::new(),
                        seen: Default::default(),
                        couch: true,
                        initialize: true,
                    },
                ),
            )
            .unwrap();
        assert!(reply.result.is_err());
        assert_eq!(run.playtest, before);

        let mut full = Journal::new("vote-session".into(), 9);
        for id in 1..=64 {
            full.apply(&mut run, request(id, Command::Retreat)).unwrap();
        }
        let before = run.playtest.clone();
        assert!(
            full.prepare_votes(&mut run, votes::Records::new(), Default::default(), true)
                .is_err()
        );
        assert_eq!(run.playtest, before);
        assert!(!full.votes_initialized);
    }

    #[test]
    fn replayed_prepare_receipt_keeps_newer_vote_facts() {
        let mut run = Run::new(7, 9, Some("Guest"));
        let mut journal = Journal::new("vote-session".into(), 9);
        let seats = run
            .players
            .iter()
            .map(|(&id, hero)| {
                (
                    id,
                    hero.name.clone(),
                    run.seat_epochs.get(&id).copied().unwrap_or(0),
                )
            })
            .collect();
        let command = request(
            1,
            Command::PrepareVotes {
                seats,
                records: votes::Records::new(),
                seen: Default::default(),
                couch: false,
                initialize: true,
            },
        );
        let receipt = journal.apply(&mut run, command.clone()).unwrap();
        assert!(receipt.result.is_ok());
        run.playtest.command(
            1,
            "You",
            votes::Command {
                serial: 1,
                action: votes::Action::Toggle,
            },
        );
        run.playtest.command(
            1,
            "You",
            votes::Command {
                serial: 2,
                action: votes::Action::Rate(5),
            },
        );
        let newer = run.playtest.clone();
        assert_eq!(journal.apply(&mut run, command).unwrap(), receipt);
        assert_eq!(run.playtest, newer);
        assert!(journal.votes_initialized);
    }

    #[test]
    fn restored_initialization_keeps_newer_ballot_facts_over_stale_realm_values() {
        let (mut run, journal) = initialized();
        run.playtest.command(
            1,
            "You",
            votes::Command {
                serial: 1,
                action: votes::Action::Toggle,
            },
        );
        run.playtest.command(
            1,
            "You",
            votes::Command {
                serial: 2,
                action: votes::Action::Rate(4),
            },
        );
        let records = run.playtest.records.clone();
        let mut restored_run = Run::new(7, 9, Some("Guest"));
        restored_run.playtest = run.playtest.clone();
        restored_run.playtest.ready = false;
        let mut restored = Journal::restore(journal.capture().unwrap(), "vote-session", 9).unwrap();
        restored
            .prepare_votes(
                &mut restored_run,
                votes::Records::new(),
                Default::default(),
                false,
            )
            .unwrap();
        assert_eq!(restored_run.playtest.records, records);
        assert_eq!(restored_run.playtest.changes.len(), 1);
        assert!(restored_run.playtest.dirty);
        assert!(restored_run.playtest.seats[&2].couch);
    }

    #[test]
    fn schema_five_journal_migrates_with_uninitialized_vote_marker() {
        let old = Journal::new("vote-session".into(), 9);
        let mut saved = old.capture().unwrap();
        saved["schema"] = 5.into();
        saved.as_object_mut().unwrap().remove("votes_initialized");
        let mut journal = Journal::restore(saved, "vote-session", 9).unwrap();
        let mut run = Run::new(7, 9, Some("Guest"));
        run.playtest
            .records
            .entry("Guest".into())
            .or_default()
            .insert(votes::ITEMS[0].id.into(), votes::Rating::Stars(3));
        run.playtest.seats.insert(
            1,
            votes::Seat {
                couch: true,
                ..Default::default()
            },
        );
        // Private durable serialization omits ready/dirty/changes. Restore
        // through the production serde checkpoint codec, as a schema-5 save
        // does, rather than setting the transient latch in the test.
        let private_bytes = serde_json::to_vec(&run).unwrap();
        let mut run: Run = serde_json::from_slice(&private_bytes).unwrap();
        assert!(!run.playtest.ready);
        assert!(run.playtest.changes.is_empty());
        let current = run.playtest.records.clone();
        journal
            .prepare_votes(&mut run, votes::Records::new(), Default::default(), true)
            .unwrap();
        assert_eq!(run.playtest.records, current);
        assert_eq!(run.playtest.changes.len(), 1);
        assert!(run.playtest.seats[&1].couch);
        assert!(journal.votes_initialized);
        assert!(journal.claim_votes(&mut run).unwrap().is_some());
    }

    #[test]
    fn vote_claim_admission_requires_initialized_exact_run_snapshot() {
        let mut run = Run::new(7, 9, Some("Guest"));
        let mut journal = Journal::new("vote-session".into(), 9);
        let arbitrary = Command::ClaimVotes {
            records: votes::Records::new(),
            seen: Default::default(),
            changes: Vec::new(),
        };
        assert!(journal.apply(&mut run, request(1, arbitrary)).is_err());
        assert_eq!(journal.high_water, 0);

        let (mut run, mut journal) = initialized();
        run.playtest.dirty = true;
        run.playtest.changes.push(votes::Change {
            player: "Guest".into(),
            item: votes::ITEMS[0].id.into(),
            rating: Some(votes::Rating::Stars(2)),
        });
        let stale = Command::ClaimVotes {
            records: votes::Records::new(),
            seen: Default::default(),
            changes: Vec::new(),
        };
        assert!(journal.apply(&mut run, request(2, stale)).is_err());
        assert_eq!(journal.high_water, 1);
        assert!(journal.pending_votes().unwrap().is_none());
    }

    #[test]
    fn fresh_initialize_command_cannot_replace_initialized_vote_facts() {
        let (mut run, mut journal) = initialized();
        let before = run.playtest.clone();
        let seats = run
            .players
            .iter()
            .map(|(&id, hero)| {
                (
                    id,
                    hero.name.clone(),
                    run.seat_epochs.get(&id).copied().unwrap_or(0),
                )
            })
            .collect();
        let next_id = journal.next_id().unwrap();
        assert!(
            journal
                .apply(
                    &mut run,
                    request(
                        next_id,
                        Command::PrepareVotes {
                            seats,
                            records: votes::Records::new(),
                            seen: Default::default(),
                            couch: false,
                            initialize: true,
                        },
                    ),
                )
                .is_err()
        );
        assert_eq!(run.playtest, before);
    }

    #[test]
    fn claimed_export_survives_restore_and_aba_retry_cannot_ack_a_later_change() {
        let (mut run, mut journal) = initialized();
        run.playtest.command(
            1,
            "You",
            votes::Command {
                serial: 1,
                action: votes::Action::Toggle,
            },
        );
        run.playtest.command(
            1,
            "You",
            votes::Command {
                serial: 2,
                action: votes::Action::Rate(4),
            },
        );
        let first = journal.claim_votes(&mut run).unwrap().unwrap();
        let mut journal = Journal::restore(journal.capture().unwrap(), "vote-session", 9).unwrap();
        assert_eq!(journal.claim_votes(&mut run).unwrap(), Some(first.clone()));

        // The in-flight claim remains valid after a seat incarnation change.
        run.seat_epochs.insert(1, 88);
        journal.acknowledge_votes(&mut run, &first).unwrap();
        run.playtest.command(
            1,
            "You",
            votes::Command {
                serial: 3,
                action: votes::Action::Rate(2),
            },
        );
        let second = journal.claim_votes(&mut run).unwrap().unwrap();
        journal.acknowledge_votes(&mut run, &second).unwrap();
        run.playtest.command(
            1,
            "You",
            votes::Command {
                serial: 4,
                action: votes::Action::Rate(4),
            },
        );
        let third = journal.claim_votes(&mut run).unwrap().unwrap();
        assert!(journal.acknowledge_votes(&mut run, &first).is_err());
        assert!(run.playtest.changes.is_empty());
        assert_eq!(journal.pending_votes().unwrap(), Some(third.clone()));
        journal.acknowledge_votes(&mut run, &third).unwrap();
        assert!(run.playtest.changes.is_empty());
    }

    #[test]
    fn pending_claim_restore_recovers_only_newer_aba_equal_vote_events() {
        let (mut run, mut journal) = initialized();
        run.playtest.command(1, "You", votes::Command { serial: 1, action: votes::Action::Toggle });
        for (serial, rating) in [(2, 4), (3, 2)] {
            run.playtest.command(
                1,
                "You",
                votes::Command {
                    serial,
                    action: votes::Action::Rate(rating),
                },
            );
        }
        let original = journal.claim_votes(&mut run).unwrap().unwrap();
        assert_eq!(original.changes.len(), 2);
        assert!(run.playtest.changes.is_empty());

        let private_bytes = serde_json::to_vec(&run).unwrap();
        let mut restored_run: Run = serde_json::from_slice(&private_bytes).unwrap();
        assert!(!restored_run.playtest.ready);
        assert!(restored_run.playtest.changes.is_empty());
        let mut restored_journal =
            Journal::restore(journal.capture().unwrap(), "vote-session", 9).unwrap();
        assert_eq!(
            restored_journal.claim_votes(&mut restored_run).unwrap(),
            Some(original.clone())
        );
        restored_journal
            .prepare_votes(
                &mut restored_run,
                votes::Records::new(),
                Default::default(),
                false,
            )
            .unwrap();
        assert!(restored_run.playtest.changes.is_empty());

        // Recreate the exact same value sequence while the old receipt is
        // still pending. Its retry must not consume these newer events.
        for (serial, rating) in [(4, 4), (5, 2)] {
            restored_run.playtest.command(
                1,
                "You",
                votes::Command {
                    serial,
                    action: votes::Action::Rate(rating),
                },
            );
        }
        assert_eq!(restored_run.playtest.changes, original.changes);
        let newer = restored_run.playtest.changes.clone();
        restored_journal
            .acknowledge_votes(&mut restored_run, &original)
            .unwrap();
        assert_eq!(restored_run.playtest.changes, newer);
        assert!(restored_run.playtest.dirty);
    }

    #[test]
    fn acknowledged_claim_preserves_newer_changes() {
        let (mut run, mut journal) = initialized();
        run.playtest.command(
            1,
            "You",
            votes::Command {
                serial: 1,
                action: votes::Action::Toggle,
            },
        );
        run.playtest.command(
            1,
            "You",
            votes::Command {
                serial: 2,
                action: votes::Action::Rate(4),
            },
        );
        let batch = journal.claim_votes(&mut run).unwrap().unwrap();
        run.playtest.command(
            1,
            "You",
            votes::Command {
                serial: 3,
                action: votes::Action::Rate(2),
            },
        );
        let later = run.playtest.changes.last().unwrap().clone();
        journal.acknowledge_votes(&mut run, &batch).unwrap();
        assert_eq!(run.playtest.changes, vec![later]);
        assert!(run.playtest.dirty);
        assert!(journal.acknowledge_votes(&mut run, &batch).is_err());
    }
}
