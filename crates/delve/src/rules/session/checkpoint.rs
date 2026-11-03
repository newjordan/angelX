//! Bounded, private checkpoint for the rules-owned live session.
//!
//! This is separate from the durable save format and public game wire: it
//! preserves the complete runtime Run, Journal receipts, and source context
//! needed to hand a live session to another rules owner.
use super::{Context, frame};
use crate::rules::{Run, host_commands::Journal, runtime_state};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Write};

pub const SESSION_CHECKPOINT_SCHEMA: u32 = 1;
/// Reserve room for the existing System JSON method/serial/reply envelope.
/// Accepted checkpoint payloads therefore fit inside its 16 MiB frame cap.
pub const MAX_SESSION_CHECKPOINT_BYTES: usize = 16 * 1024 * 1024 - 4096;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionCheckpoint {
    pub schema: u32,
    pub session: String,
    pub raid: u64,
    pub event_seq: u64,
    pub view_revision: u64,
    pub run: runtime_state::Checkpoint,
    pub journal: Value,
    pub context: Context,
    #[serde(deserialize_with = "required_pending_frame")]
    pub pending_frame: Option<frame::Pending>,
}

/// The independently validated, owning state recovered from an envelope.
#[derive(Debug)]
pub struct Restored {
    pub run: Run,
    pub journal: Journal,
    pub context: Context,
    pub event_seq: u64,
    pub view_revision: u64,
    pub pending_frame: Option<frame::Pending>,
}

// An unreleased schema-1 checkpoint must explicitly carry null or an owned
// phase marker; omission must not silently erase an in-flight native frame.
fn required_pending_frame<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<frame::Pending>, D::Error> {
    Option::<frame::Pending>::deserialize(deserializer)
}

struct BoundedBytes(Vec<u8>);
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > MAX_SESSION_CHECKPOINT_BYTES {
            return Err(io::Error::other(
                "session checkpoint exceeds its bounded payload",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn bounded_json(value: &SessionCheckpoint) -> Result<Vec<u8>, String> {
    let mut output = BoundedBytes(Vec::new());
    serde_json::to_writer(&mut output, value)
        .map_err(|error| format!("invalid session checkpoint: {error}"))?;
    Ok(output.0)
}

/// Capture an immutable checkpoint from one live ownership boundary.
///
/// The session identity comes from the existing Journal and the raid identity
/// from its Run; they are cross-validated before any state is returned.
pub fn capture(
    run: &Run,
    journal: &Journal,
    context: &Context,
    event_seq: u64,
    view_revision: u64,
) -> Result<SessionCheckpoint, String> {
    capture_with_frame(run, journal, context, event_seq, view_revision, None)
}

/// Live owners must include their exact native-ability-gap phase marker.
pub fn capture_with_frame(
    run: &Run,
    journal: &Journal,
    context: &Context,
    event_seq: u64,
    view_revision: u64,
    pending_frame: Option<&frame::Pending>,
) -> Result<SessionCheckpoint, String> {
    let session = journal.session().to_owned();
    let checkpoint = SessionCheckpoint {
        schema: SESSION_CHECKPOINT_SCHEMA,
        session,
        raid: run.raid_id,
        event_seq,
        view_revision,
        run: runtime_state::capture_for_rules(run)?,
        journal: journal.capture()?,
        context: context.clone(),
        pending_frame: pending_frame.cloned(),
    };
    bounded_json(&checkpoint)?;
    // Avoid decoding/cloning nested Run and Journal state until the aggregate
    // serialized-size preflight has established the payload bound.
    validate(&checkpoint)?;
    Ok(checkpoint)
}

/// Restore without changing queues, latches, claims, or their ownership.
pub fn restore(checkpoint: SessionCheckpoint) -> Result<Restored, String> {
    bounded_json(&checkpoint)?;
    validate(&checkpoint)?;

    let run = runtime_state::restore(checkpoint.run)?;
    let journal = Journal::restore(checkpoint.journal, &checkpoint.session, checkpoint.raid)?;
    validate_pending_world(&run, &journal)?;
    Ok(Restored {
        run,
        journal,
        context: checkpoint.context,
        event_seq: checkpoint.event_seq,
        view_revision: checkpoint.view_revision,
        pending_frame: checkpoint.pending_frame,
    })
}

/// Decode only complete, bounded envelopes. Callers must not deserialize an
/// untrusted frame into `SessionCheckpoint` before applying this size guard.
pub fn decode(bytes: &[u8]) -> Result<Restored, String> {
    if bytes.len() > MAX_SESSION_CHECKPOINT_BYTES {
        return Err("session checkpoint exceeds its bounded payload".into());
    }
    let checkpoint: SessionCheckpoint = serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid session checkpoint: {error}"))?;
    restore(checkpoint)
}

impl SessionCheckpoint {
    /// Serialize and validate the complete envelope under the system frame bound.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let bytes = bounded_json(self)?;
        validate(self)?;
        Ok(bytes)
    }
}

fn validate(checkpoint: &SessionCheckpoint) -> Result<(), String> {
    if checkpoint.schema != SESSION_CHECKPOINT_SCHEMA {
        return Err(format!(
            "unsupported session checkpoint schema {} (expected {})",
            checkpoint.schema, SESSION_CHECKPOINT_SCHEMA
        ));
    }
    if checkpoint.run.schema != runtime_state::SCHEMA && checkpoint.run.schema != 7 {
        return Err(format!(
            "unsupported nested Run schema {} (expected {})",
            checkpoint.run.schema,
            runtime_state::SCHEMA
        ));
    }
    checkpoint.context.validate()?;
    reject_host_appearance(&checkpoint.run.run)?;

    let journal_schema = checkpoint
        .journal
        .get("schema")
        .ok_or("session checkpoint is missing its Journal schema")?;
    let current_journal_schema = Journal::new(checkpoint.session.clone(), checkpoint.raid)
        .capture()?
        .get("schema")
        .cloned()
        .ok_or("current Journal is missing its schema")?;
    if journal_schema != &current_journal_schema {
        return Err("session checkpoint must use the current Journal schema".into());
    }
    let run = runtime_state::restore(checkpoint.run.clone())?;
    if run.tick > super::MAX_TICK || run.raid_id != checkpoint.raid || !run.valid_snapshot() {
        return Err("session checkpoint Run is invalid or belongs to another raid".into());
    }
    let journal = Journal::restore(
        checkpoint.journal.clone(),
        &checkpoint.session,
        checkpoint.raid,
    )?;
    // A schema-6 Journal with an omitted defaulted ownership marker can
    // deserialize successfully. Requiring its current canonical capture
    // rejects missing or noncanonical fields after restore normalization.
    if journal.capture()? != checkpoint.journal {
        return Err("session checkpoint Journal is incomplete or noncanonical".into());
    }
    validate_pending_world(&run, &journal)?;
    if let Some(pending) = &checkpoint.pending_frame {
        pending.validate(&run, checkpoint.event_seq, &checkpoint.session)?;
    }
    Ok(())
}

/// Check that a quiescent candidate handoff preserves the complete checkpoint.
/// Session events and acknowledgements remain queued until ownership swaps, so
/// no Journal retirement or context change is valid during this interval.
pub fn validate_handoff(prior: &SessionCheckpoint, next: &SessionCheckpoint) -> Result<(), String> {
    validate(prior)?;
    validate(next)?;
    // Exact canonical equality covers session/raid, both nested payloads,
    // source context, sequencing, and every Journal receipt/completion.
    let mut prior = prior.clone();
    if prior.run.schema == 7 {
        let migrated = runtime_state::restore(prior.run)?;
        prior.run = runtime_state::capture_for_rules(&migrated)?;
    }
    if serde_json::to_value(&prior).map_err(|error| error.to_string())?
        != serde_json::to_value(next).map_err(|error| error.to_string())?
    {
        return Err("session checkpoint handoff changed state during quiescent replacement".into());
    }
    Ok(())
}

fn reject_host_appearance(run: &Value) -> Result<(), String> {
    let default_scene = serde_json::to_value(crate::realm::effects::Scene::default())
        .map_err(|error| error.to_string())?;
    if run.get("module_visuals") != Some(&default_scene) {
        return Err("rules checkpoint contains host-owned module visuals".into());
    }
    let players = run
        .get("players")
        .and_then(Value::as_object)
        .ok_or("rules checkpoint is missing its player map")?;
    if players
        .values()
        .any(|hero| hero.get("avatar").is_some_and(|avatar| !avatar.is_null()))
    {
        return Err("rules checkpoint contains host-owned avatar data".into());
    }
    Ok(())
}

fn validate_pending_world(run: &Run, journal: &Journal) -> Result<(), String> {
    if journal.session().is_empty() || run.raid_id != journal_raid(journal, run.raid_id)? {
        return Err("session checkpoint Journal ownership differs from its Run".into());
    }
    journal.validate_home_world(run)?;
    journal.validate_brew_world(run)?;
    journal.validate_reforge_world(run)?;
    journal.validate_wish_world(run)?;
    if !crate::rules::brew_jobs::valid_outbox(&run.brewing, run.raid_id) {
        return Err("session checkpoint contains an invalid brewing outbox".into());
    }
    // Decode every retained claim through the Journal's existing identity and
    // payload validators; no checkpoint-specific receipt ledger is introduced.
    let _ = journal.pending_realm()?;
    let _ = journal.pending_home()?;
    let _ = journal.pending_brews()?;
    let _ = journal.pending_reforges()?;
    let _ = journal.pending_wishes()?;
    Ok(())
}

fn journal_raid(journal: &Journal, run_raid: u64) -> Result<u64, String> {
    // Journal's persisted raid is intentionally private. Its own restore API
    // validates the raid against this Run when the envelope is decoded.
    let saved = journal.capture()?;
    let raid = saved
        .get("raid")
        .and_then(Value::as_u64)
        .ok_or("Journal checkpoint is missing its raid")?;
    if raid != run_raid {
        return Err("Journal raid differs from Run".into());
    }
    Ok(raid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn context() -> Context {
        Context {
            settings: crate::settings::Settings::default(),
            phrasebook: (crate::rules::phrasebook::TEXT.to_owned(), Vec::new()),
            wishes: Arc::new(Vec::new()),
        }
    }

    #[test]
    fn checkpoint_round_trip_preserves_full_rules_state_and_identity() {
        let mut run = Run::new(17, 9, Some("Guest"));
        let mut journal = Journal::new("session-9".into(), run.raid_id);
        journal
            .prepare_votes(&mut run, Default::default(), Default::default(), true)
            .unwrap();
        run.playtest.command(
            1,
            "Guest",
            crate::rules::votes::Command {
                serial: 1,
                action: crate::rules::votes::Action::Toggle,
            },
        );
        run.playtest.command(
            1,
            "Guest",
            crate::rules::votes::Command {
                serial: 2,
                action: crate::rules::votes::Action::Rate(4),
            },
        );
        let claimed = journal.claim_votes(&mut run).unwrap().unwrap();
        run.playtest.command(
            1,
            "Guest",
            crate::rules::votes::Command {
                serial: 3,
                action: crate::rules::votes::Action::Rate(2),
            },
        );
        assert!(!run.playtest.changes.is_empty());
        assert_eq!(claimed.session, journal.session());
        let context = context();
        let checkpoint = capture(&run, &journal, &context, 23, 41).unwrap();
        let bytes = checkpoint.encode().unwrap();
        let restored = decode(&bytes).unwrap();

        assert_eq!(restored.event_seq, 23);
        assert_eq!(restored.view_revision, 41);
        assert_eq!(restored.run.raid_id, run.raid_id);
        assert_eq!(
            runtime_state::capture_for_rules(&restored.run).unwrap().run,
            runtime_state::capture_for_rules(&run).unwrap().run,
        );
        assert_eq!(
            restored.journal.capture().unwrap(),
            journal.capture().unwrap()
        );
        assert_eq!(
            serde_json::to_value(restored.context).unwrap(),
            serde_json::to_value(context).unwrap(),
        );
    }

    #[test]
    fn checkpoint_rejects_mislabeled_schema_foreign_journal_and_host_appearance() {
        let mut run = Run::new(17, 9, Some("Guest"));
        let mut journal = Journal::new("session-9".into(), run.raid_id);
        journal
            .prepare_votes(&mut run, Default::default(), Default::default(), true)
            .unwrap();
        let checkpoint = capture(&run, &journal, &context(), 0, 0).unwrap();

        let mut wrong_outer = checkpoint.clone();
        wrong_outer.schema += 1;
        assert!(restore(wrong_outer).is_err());

        let mut foreign_journal = checkpoint.clone();
        foreign_journal.journal["session"] = "another-session".into();
        assert!(restore(foreign_journal).is_err());

        let mut missing_marker = checkpoint.clone();
        missing_marker
            .journal
            .as_object_mut()
            .unwrap()
            .remove("votes_initialized");
        assert!(restore(missing_marker).is_err());

        let mut appearance = checkpoint;
        appearance.run.run["module_visuals"]["revision"] = 1.into();
        assert!(restore(appearance).is_err());
    }

    #[test]
    fn checkpoint_enforces_frame_bound_and_pending_world_ownership() {
        assert!(decode(&vec![0; MAX_SESSION_CHECKPOINT_BYTES + 1]).is_err());

        let mut run = Run::new(17, 9, Some("Guest"));
        run.players.get_mut(&1).unwrap().reforge_job = Some("unowned-result".into());
        let journal = Journal::new("session-9".into(), run.raid_id);
        assert!(capture(&run, &journal, &context(), 0, 0).is_err());

        let mut over_tick = Run::new(17, 9, Some("Guest"));
        over_tick.tick = super::super::MAX_TICK + 1;
        let journal = Journal::new("session-9".into(), over_tick.raid_id);
        assert!(capture(&over_tick, &journal, &context(), 0, 0).is_err());
    }

    #[test]
    fn schema_seven_handoff_migrates_only_default_squire_hold() {
        let run = Run::new(17, 9, Some("Guest"));
        let journal = Journal::new("session-9".into(), run.raid_id);
        let current = capture(&run, &journal, &context(), 0, 0).unwrap();
        let mut old = current.clone();
        old.run.schema = 7;
        let fields = old.run.run.as_object_mut().unwrap();
        fields.remove("squire_company_reserved");
        fields.remove("squire_company_claimed");
        fields.remove("squire_pending_delves");
        assert!(restore(old.clone()).is_ok());
        validate_handoff(&old, &current).unwrap();
        let mut changed = current.clone();
        changed.run.run["squire_company_claimed"] = true.into();
        assert!(validate_handoff(&old, &changed).is_err());
    }

    #[test]
    fn handoff_requires_exact_checkpoint_continuity() {
        let mut run = Run::new(17, 9, Some("Guest"));
        let mut journal = Journal::new("session-9".into(), run.raid_id);
        journal
            .prepare_votes(&mut run, Default::default(), Default::default(), true)
            .unwrap();
        run.playtest.command(
            1,
            "Guest",
            crate::rules::votes::Command {
                serial: 1,
                action: crate::rules::votes::Action::Toggle,
            },
        );
        run.playtest.command(
            1,
            "Guest",
            crate::rules::votes::Command {
                serial: 2,
                action: crate::rules::votes::Action::Rate(4),
            },
        );
        let batch = journal.claim_votes(&mut run).unwrap().unwrap();
        let prior = capture(&run, &journal, &context(), 4, 7).unwrap();
        assert!(validate_handoff(&prior, &prior.clone()).is_ok());

        let mut deleted = prior.clone();
        deleted.journal["records"] = serde_json::json!([]);
        assert!(validate_handoff(&prior, &deleted).is_err());

        let mut changed_context = prior.clone();
        changed_context.context.phrasebook.0.push_str(" changed");
        assert!(validate_handoff(&prior, &changed_context).is_err());

        journal.acknowledge_votes(&mut run, &batch).unwrap();
        let next = capture(&run, &journal, &context(), 5, 8).unwrap();
        assert!(validate_handoff(&prior, &next).is_err());
    }
}
