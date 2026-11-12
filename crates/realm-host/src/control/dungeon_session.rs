//! Private, atomic checkpoints for live game rebuilds. No harness conversation
//! or credentials are saved here; the game invite itself is owner-readable.
use super::*;
use super::dungeon::Hosting;
use crate::together_shooter::Run;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};

/// The checkpoint as it is read back (`together_sim::write_checkpoint`
/// writes it).
#[derive(Deserialize)]
struct Saved {
    version: u32,
    raid_serial: u64,
    run: Run,
    /// The hosted session's address and join key: a restart opens the same
    /// link again. (An older save's invitation tokens: the first is the key.)
    invitation: Option<(String, String)>,
    /// The host played no knight (`/dungeon host --watch`). Absent in an older
    /// save and for a host that played its own: the env switch decides then.
    #[serde(default)]
    watch: bool,
    /// Country intents and unpaid Squires' Yard gold, awaiting a realm commit.
    /// Keep the historical field name and default for old checkpoints. Never
    /// serialize general combat/bounty marks or expose this on the guest wire.
    #[serde(default)]
    country_intents: BTreeMap<String, u32>,
    #[serde(default)]
    squire_payment: Option<crate::together_shooter::squires_town::CheckpointPayment>,
    #[serde(default)]
    commands: Option<serde_json::Value>,
    #[serde(default)]
    home_outbox: Option<angel_delve::rules::home_settlement::HomeOutbox>,
    #[serde(default)]
    realm_triumph: Option<crate::together_shooter::Triumph>,
    #[serde(default)]
    brew_outbox: Option<Vec<crate::together_shooter::cauldron::Brewing>>,
}

/// A realm commit may be newer than the independently written run checkpoint.
/// Read its company before starting the sim (including a restored listener),
/// never after an old checkpoint has had a chance to settle over paid goods.
/// A missing realm is a legacy/checkpoint-only session; an unreadable existing
/// realm is an error, not an empty company that could erase paid companions.
#[cfg(test)]
fn restore_company(run: &mut Run, rewards: Option<&Path>) -> Result<(), String> {
    restore_company_with_journal(run, rewards, None)
}

fn restore_company_with_journal(
    run: &mut Run,
    rewards: Option<&Path>,
    journal: Option<&angel_delve::rules::host_commands::Journal>,
) -> Result<(), String> {
    let Some(path) = rewards.map(|p| p.with_extension("realm.json")) else {
        return Ok(());
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("could not read realm company: {e}")),
    };
    let realm: crate::together_realm::Realm = serde_json::from_slice(&bytes)
        .map_err(|e| format!("could not restore realm company: {e}"))?;
    if journal.is_some_and(|journal| !journal.needs_company_reconciliation(&realm.home.squire_company)) {
        return Ok(());
    }
    let company_changed = run.home.squire_company != realm.home.squire_company;
    if company_changed {
        run.home.squire_company = realm.home.squire_company;
        run.treasury = realm.treasury;
    }
    // Only a stale company needs redeployment. Do not heal/reset actors in
    // ordinary current-company checkpoints on every restart.
    if company_changed && run.dungeon.depth > 0 {
        run.deploy_company_squires();
    }
    Ok(())
}

impl DungeonView {
    /// Save the delve now (`force`), or when the last save is three
    /// seconds old. The sim thread saves on its own clock as it plays;
    /// this is for the moments the cockpit must be sure of one.
    pub fn checkpoint(&mut self, now: Instant, force: bool) -> Result<(), String> {
        if !force
            && self
                .last_saved
                .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_secs(3))
        {
            return Ok(());
        }
        let gate = self.shooter.checkpoint_gate();
        let gate_at = Instant::now();
        let _checkpoint = gate.as_ref().map(|gate| {
            gate.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        });
        if crate::stress_log() {
            eprintln!("stress-gate: wait {:.1} ms", gate_at.elapsed().as_secs_f64() * 1e3);
        }
        self.shooter.latch();
        let Some(path) = &self.save_path else {
            return Ok(());
        };
        self.shooter.save_checkpoint(path, self.raid_serial)?;
        self.last_saved = Some(now);
        Ok(())
    }

    pub fn restore(&mut self, workspace: &Path) -> Result<(), String> {
        let path = workspace.join(".angel/dungeon/session.json");
        self.save_path = Some(path.clone());
        // The knobs moved from a dev panel or `/dungeon tune`, beside it.
        self.shooter
            .set_tune_file(workspace.join(".angel/dungeon/tune.json"));
        // Its paintings, which the window draws from.
        crate::services::paint_dir(&crate::together_sheets::paint_dir(workspace));
        // Its wished looks, which the window lays over its HD pack.
        crate::services::looks_dir(&crate::together_looker::looks_dir(workspace));
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(crate::together_sim::MAX_GAME_SAVE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > crate::together_sim::MAX_GAME_SAVE_BYTES {
            return Err("dungeon save is too large".into());
        }
        let mut saved: Saved = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        for (&player, hero) in &mut saved.run.players {
            if saved.version < 7 && let Some(offer) = &mut hero.offer {
                if offer.binding.is_some() {
                    return Err("old checkpoint contains retained scroll sources".into());
                }
                let epoch = saved.run.seat_epochs.get(&player).copied().unwrap_or(0);
                offer.binding = Some(angel_delve::rules::scrolls::Binding::legacy(
                    saved.run.raid_id, player, epoch,
                ));
            }
        }
        angel_delve::rules::scrolls::validate_run(&saved.run)?;
        // What can be put right is, so a restart keeps the run and its link.
        let mended = saved.run.mend();
        if !mended.is_empty() {
            tracing::warn!(?mended, "dungeon checkpoint mended on restore");
        }
        if !matches!(saved.version, 1 | 2 | 3 | 4 | 5 | 6 | 7)
            || (saved.version >= 2 && saved.commands.is_none())
            || ((saved.version >= 3) != saved.home_outbox.is_some())
            || ((saved.version >= 4) != saved.brew_outbox.is_some())
            || (saved.version < 5 && saved.run.players.values().any(|h| h.reforge_job.is_some()))
            || (saved.version < 6 && (!saved.run.seat_epochs.is_empty() || saved.run.players.values().any(|h| !h.wish_jobs.is_empty())))
            || saved.squire_payment.is_some_and(|payment| !payment.valid(&saved.country_intents))
            || !saved.run.valid_snapshot()
            || saved.country_intents.len()
                > crate::together_shooter::squires_town::CHECKPOINT_INTENT_LIMIT
            || !saved.country_intents.iter().all(|(key, value)| {
                crate::together_shooter::squires_town::checkpoint_intent_valid(key, *value)
            })
        {
            return Err("invalid dungeon checkpoint".into());
        }
        if let Some(outbox) = saved.home_outbox {
            // The compatibility projection must agree with the authoritative
            // private facts; neither copy can hide an invalid country intent.
            let country: BTreeMap<_, _> = outbox
                .marks
                .iter()
                .filter(|(key, _)| crate::together_shooter::squires_town::checkpoint_intent_key(key))
                .map(|(key, value)| (key.clone(), *value))
                .collect();
            if country != saved.country_intents {
                return Err("inconsistent private country intents".into());
            }
            outbox.install(&mut saved.run)?;
        } else {
            saved.run.marks = saved.country_intents;
        }
        saved.run.brewing = saved.brew_outbox.unwrap_or_default();
        if !angel_delve::rules::brew_jobs::valid_outbox(&saved.run.brewing, saved.run.raid_id) {
            return Err("invalid private brew outbox".into());
        }
        saved.run.triumph = saved.realm_triumph;
        // A valid old checkpoint must receive authored roads immediately,
        // even when no reward/purchase will trigger rebuild_home this tick.
        crate::together_shooter::world::raise_world(&mut saved.run.dungeon);
        saved.run.migrate_kingdom();
        if !saved.run.valid_snapshot() {
            return Err("invalid migrated dungeon checkpoint".into());
        }
        // Match World::for_workspace's canonical island identity without
        // constructing another display world just to find its rewards path.
        let identity = crate::hooks::repo_identity(workspace);
        let rewards = std::env::var_os("HOME").map(|home| {
            std::path::PathBuf::from(home)
                .join(".angelX/world-rewards")
                .join(format!("{}.json", identity.key))
        });
        // Legacy lodge batches were checkpointed before the realm commit.
        // Retire only a batch known durable in the matching realm file.
        if let Some(payment) = saved.squire_payment {
            let committed = if let Some(path) = rewards.as_ref().map(|p| p.with_extension("realm.json")) {
                match std::fs::read(path) {
                    Ok(bytes) => serde_json::from_slice::<crate::together_realm::Realm>(&bytes)
                        .map_err(|e| format!("could not restore lodge payment: {e}"))?.squires_paid_batch,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
                    Err(e) => return Err(format!("could not read lodge payment: {e}")),
                }
            } else { 0 };
            payment.retire(committed, &mut saved.run.marks);
        }
        let door = saved
            .invitation
            .map(|(saved_bind, saved_key)| {
                let key = saved_key.split(',').next().unwrap_or_default().to_string();
                crate::together_door::bind(&saved_bind).map(|bind| (bind, key, saved_bind, saved_key))
            })
            .transpose()?;
        let commands = saved
            .commands
            .map(|commands| {
                let session = commands
                    .get("session")
                    .and_then(serde_json::Value::as_str)
                    .ok_or("missing saved game session identity")?
                    .to_owned();
                angel_delve::rules::host_commands::Journal::restore(
                    commands,
                    &session,
                    saved.run.raid_id,
                )
            })
            .transpose()?;
        // Reconcile a stale checkpoint before the sim starts, but never
        // replace a pending claim or edits newer than its observed realm.
        restore_company_with_journal(&mut saved.run, rewards.as_deref(), commands.as_ref())?;
        if let Some(commands) = &commands {
            if saved.version < 4 && !commands.pending_brews()?.is_empty() {
                return Err("old private checkpoint cannot contain brew jobs".into());
            }
            commands.validate_home_world(&saved.run)?;
            commands.validate_brew_world(&saved.run)?;
            if saved.version < 5 && !commands.pending_reforges()?.is_empty() {
                return Err("old private checkpoint cannot contain reforge jobs".into());
            }
            commands.validate_reforge_world(&saved.run)?;
            if saved.version < 6 && !commands.pending_wishes()?.is_empty() { return Err("old private checkpoint cannot contain wish jobs".into()); }
            commands.validate_wish_world(&saved.run)?;
        }
        let browser = if door.is_some() {
            crate::together_web::shipped_prepared().map_err(|e| e.to_string())?
        } else { None };
        self.raid_serial = saved.raid_serial.max(saved.run.raid_id);
        // Install world and journal under one lock, before its server starts.
        self.shooter.restore_session(saved.run, commands);
        if let Some((bind, key, saved_bind, saved_key)) = door {
            // A host that watched still watches; the env switch only ever
            // makes a host watch, as it does for `/dungeon host`.
            let watch = saved.watch || Hosting::watch_by_default();
            let hosting = Hosting {
                friends: Hosting::most_friends(watch),
                watch,
            };
            // The saved link is on record until it is open again: an address
            // that is not up yet must not cost the next checkpoint its key.
            self.shooter.keep_door(saved_bind, saved_key, watch);
            self.shooter.host_prepared(
                crate::together_sim::Door {
                    bind,
                    seats: hosting.seats(),
                    key: Some(key),
                    bundle: None,
                },
                watch,
                browser,
            )?;
            self.hosting = Some(hosting);
        }
        self.notice = "Delve restored · same run and link · F6 rejoins".into();
        self.last_saved = Some(Instant::now());
        Ok(())
    }
}

impl Drop for DungeonView {
    fn drop(&mut self) {
        self.collect_settled(true);
        if let Err(error) = self.checkpoint(Instant::now(), true) {
            tracing::warn!(%error, "could not save dungeon on exit");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "delve-save-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn dungeon_checkpoint_restores_same_run_and_invitation() {
        let _guard = crate::tests::browser_capacity();
        let dir = TempDir::new();
        let mut view = DungeonView::default();
        view.restore(dir.path()).unwrap();
        view.shooter.set(Some(Run::new(12, 7, Some("Friend"))));
        view.raid_serial = 7;
        view.shooter
            .host(crate::together_sim::Door {
                bind: "127.0.0.1:0".parse().unwrap(),
                seats: 2,
                key: None,
                bundle: None,
            })
            .unwrap();
        view.hosting = Some(Hosting {
            friends: 1,
            watch: false,
        });
        let invitation = view.shooter.invitation().unwrap();
        view.shooter
            .as_mut()
            .unwrap()
            .players
            .get_mut(&1)
            .unwrap()
            .stone = true;
        view.checkpoint(Instant::now(), true).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(view.save_path.as_ref().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let run = serde_json::to_value(view.shooter.as_ref().unwrap()).unwrap();
        drop(view);
        let mut restored = DungeonView::default();
        restored.restore(dir.path()).unwrap();
        assert_eq!(restored.shooter.invitation().unwrap(), invitation);
        assert!(restored.hosting.is_some());
        assert_eq!(
            serde_json::to_value(restored.shooter.as_ref().unwrap()).unwrap(),
            run
        );
        assert_eq!(restored.raid_serial, 7);
    }

    #[test]
    fn playtest_outbox_restart_reuses_receipt_and_dedupes_file_rows() {
        let _guard = crate::tests::env_lock();
        let dir = TempDir::new();
        let export = dir.path().join("ratings.jsonl");
        let mut view = DungeonView::default();
        view.restore(dir.path()).unwrap();
        view.shooter.set(Some(Run::new(12, 7, Some("Friend"))));
        view.raid_serial = 7;
        view.shooter.prepare_votes(Default::default(), Default::default(), true).unwrap();
        for (serial, action) in [
            (1, crate::together_shooter::votes::Action::Toggle),
            (2, crate::together_shooter::votes::Action::Rate(4)),
        ] {
            view.shooter.command(angel_delve::rules::host_commands::Command::Vote {
                player: 2, local_focus: true,
                command: crate::together_shooter::votes::Command { serial, action },
            }).unwrap();
        }
        let claim = view.shooter.claim_votes().unwrap().unwrap();
        assert_eq!(claim.changes.len(), 1);
        view.checkpoint(Instant::now(), true).unwrap();
        crate::together_files::append_vote_batch(&export, &claim).unwrap();
        drop(view); // The export completed; its ACK never reached a checkpoint.

        let mut restored = DungeonView::default();
        restored.restore(dir.path()).unwrap();
        restored.shooter.prepare_votes(Default::default(), Default::default(), true).unwrap();
        let retry = restored.shooter.claim_votes().unwrap().unwrap();
        assert_eq!(retry, claim);
        crate::together_files::append_vote_batch(&export, &retry).unwrap();
        restored.shooter.acknowledge_votes(&retry).unwrap();
        assert!(restored.shooter.claim_votes().unwrap().is_none());
        restored.checkpoint(Instant::now(), true).unwrap();
        drop(restored);

        let mut final_view = DungeonView::default();
        final_view.restore(dir.path()).unwrap();
        final_view.shooter.prepare_votes(claim.records.clone(), claim.seen.clone(), true).unwrap();
        assert!(final_view.shooter.claim_votes().unwrap().is_none());
        let rows = std::fs::read_to_string(export).unwrap();
        assert_eq!(rows.lines().count(), 1);
        let row: serde_json::Value = serde_json::from_str(rows.trim()).unwrap();
        assert_eq!(row["stars"], 4);
        assert_eq!(row["player"], "Friend");
    }

    #[test]
    fn dungeon_checkpoint_rejects_corruption_without_starting_a_listener() {
        let dir = TempDir::new();
        let mut view = DungeonView::default();
        view.restore(dir.path()).unwrap();
        view.shooter.set(Some(Run::new(12, 7, None)));
        view.checkpoint(Instant::now(), true).unwrap();
        let path = view.save_path.clone().unwrap();
        drop(view);
        let saved = std::fs::read(&path).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        // A room out of range is mended on restore (`Run::mend`), not refused.
        let mut mendable = value.clone();
        mendable["run"]["at"] = 999.into();
        std::fs::write(&path, serde_json::to_vec(&mendable).unwrap()).unwrap();
        let mut restored = DungeonView::default();
        restored.restore(dir.path()).unwrap();
        assert_eq!(restored.shooter.as_ref().unwrap().at, 0);
        assert!(restored.hosting.is_none());
        // What cannot be read, or is not a checkpoint this build writes, is.
        let mut undecodable = value.clone();
        undecodable["run"]["at"] = "nowhere".into();
        let mut missing_journal = value.clone();
        missing_journal["commands"] = serde_json::Value::Null;
        let mut wrong_raid = value.clone();
        wrong_raid["commands"]["raid"] = 8.into();
        let mut unknown = value;
        unknown["version"] = 999.into();
        for corrupt in [
            saved[..saved.len() / 2].to_vec(),
            serde_json::to_vec(&undecodable).unwrap(),
            serde_json::to_vec(&unknown).unwrap(),
            serde_json::to_vec(&missing_journal).unwrap(),
            serde_json::to_vec(&wrong_raid).unwrap(),
        ] {
            std::fs::write(&path, corrupt).unwrap();
            let mut restored = DungeonView::default();
            assert!(restored.restore(dir.path()).is_err());
            assert!(restored.shooter.is_none() && restored.hosting.is_none());
        }
    }

    #[test]
    fn old_checkpoint_cannot_overwrite_a_paid_realm_company_or_balance() {
        use crate::together_shooter::{
            home::Home,
            squire_company::{Kind, Roster, Squire},
        };
        let dir = TempDir::new();
        let rewards = dir.path().join("island.json");
        let mut c0 = Run::at_home(61, 1, None, Home::default(), Default::default());
        c0.players.get_mut(&1).unwrap().name = "Ada".into();
        c0.treasury
            .add(crate::together_realm::Spoil::Gold, 1_000);
        let checkpoint = dir.path().join("session.json");
        crate::together_sim::write_checkpoint(&checkpoint, 1, &c0, None).unwrap();
        let mut paid = crate::together_realm::Realm::beside(Some(&rewards));
        paid.treasury
            .add(crate::together_realm::Spoil::Gold, 900);
        paid.home.squire_company.next_id = 1;
        paid.home.squire_company.rosters.insert(
            "Ada".into(),
            Roster {
                squires: vec![Squire {
                    id: 1,
                    name: "Ash".into(),
                    kind: Kind::Ninja,
                    level: 1,
                    trained_on: None,
                }],
                active: Some(1),
                claimed: vec![0],
            },
        );
        paid.save().unwrap();
        // Crash between the realm commit and the next service checkpoint.
        // Reconcile the actual older serialized checkpoint before starting a
        // sim; no listener or live host is needed to test the restore boundary.
        let mut old: Saved = serde_json::from_slice(&std::fs::read(checkpoint).unwrap()).unwrap();
        assert!(old.run.home.squire_company.rosters.is_empty());
        restore_company(&mut old.run, Some(&rewards)).unwrap();
        assert_eq!(old.run.home.squire_company, paid.home.squire_company);
        assert_eq!(old.run.treasury, paid.treasury);
        // The first settlement copy-back now starts from paid C1, not old C0.
        paid.home.squire_company = old.run.reserve_squire_company().unwrap();
        paid.save().unwrap();
        let durable = crate::together_realm::Realm::beside(Some(&rewards));
        assert_eq!(durable.home.squire_company.rosters["Ada"].squires.len(), 1);
        assert_eq!(
            durable
                .treasury
                .get(crate::together_realm::Spoil::Gold),
            900
        );
    }

    #[test]
    fn restore_keeps_legacy_checkpoint_without_realm_but_refuses_corrupt_existing_realm() {
        let dir = TempDir::new();
        let rewards = dir.path().join("island.json");
        let mut run = Run::at_home(61, 1, None, Default::default(), Default::default());
        run.home.squire_company.board = 2;
        restore_company(&mut run, Some(&rewards)).unwrap();
        assert_eq!(run.home.squire_company.board, 2);
        std::fs::write(rewards.with_extension("realm.json"), b"not a realm").unwrap();
        assert!(restore_company(&mut run, Some(&rewards)).is_err());
        assert_eq!(
            run.home.squire_company.board, 2,
            "corruption is never an empty replacement"
        );
    }
}


#[cfg(test)]
#[path = "../../tests/dungeon_session__tests.rs"]
mod view_tests;
