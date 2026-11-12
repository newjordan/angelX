//! How the fleet row and the slot watcher read the status words a competition
//! board sends. These pin both readings word for word, including the four
//! words they read differently.

use angel_live::telemetry::SlotOutcome;
use angel_live::telemetry::SubmissionPhase::{self, *};

const FLEET_READING: [(&str, SubmissionPhase); 24] = [
    ("queued", Queued),
    ("pending", Queued),
    ("submitted", Queued),
    ("validating", Running),
    ("running", Running),
    ("in-flight", Running),
    ("in_flight", Running),
    ("accepted", Accepted),
    ("promoted", Accepted),
    ("complete", Accepted),
    ("completed", Accepted),
    ("timeout", TimedOut),
    ("timed_out", TimedOut),
    ("timed-out", TimedOut),
    ("rejected", Rejected),
    ("failed", Rejected),
    ("promotion failed", Rejected),
    ("error", Rejected),
    ("cancelled", Rejected),
    ("canceled", Rejected),
    // Not words the board is known to send as these: nothing settles on them.
    ("superseded", Unknown),
    ("scoring", Unknown),
    ("", Unknown),
    ("not accepted", Unknown),
];

#[test]
fn the_fleet_row_reads_each_status_word_as_it_always_has() {
    for (word, phase) in FLEET_READING {
        for spelled in [word.to_string(), word.to_uppercase(), format!("  {word}\t")] {
            assert_eq!(SubmissionPhase::from_status(&spelled), phase, "{spelled:?}");
        }
    }
    assert_eq!(SubmissionPhase::from_status("   "), Unknown);
    assert_eq!(SubmissionPhase::from_status("accepted."), Unknown);
    assert_eq!(SubmissionPhase::from_status("promotion  failed"), Unknown);
}

/// How the slot watcher reads the same words: waiting, or settled one of three ways.
const SLOT_READING: [(&str, SlotOutcome); 24] = [
    ("queued", SlotOutcome::InFlight),
    ("pending", SlotOutcome::InFlight),
    ("submitted", SlotOutcome::InFlight),
    ("validating", SlotOutcome::InFlight),
    ("running", SlotOutcome::InFlight),
    ("in-flight", SlotOutcome::InFlight),
    ("in_flight", SlotOutcome::InFlight),
    ("accepted", SlotOutcome::Accepted),
    ("promoted", SlotOutcome::Accepted),
    ("complete", SlotOutcome::InFlight),
    ("completed", SlotOutcome::InFlight),
    ("timeout", SlotOutcome::TimedOut),
    ("timed_out", SlotOutcome::TimedOut),
    ("timed-out", SlotOutcome::TimedOut),
    ("rejected", SlotOutcome::Rejected),
    ("failed", SlotOutcome::Rejected),
    ("promotion failed", SlotOutcome::InFlight),
    ("error", SlotOutcome::Rejected),
    ("cancelled", SlotOutcome::Rejected),
    ("canceled", SlotOutcome::Rejected),
    ("superseded", SlotOutcome::Rejected),
    ("scoring", SlotOutcome::InFlight),
    ("", SlotOutcome::InFlight),
    ("not accepted", SlotOutcome::InFlight),
];

#[test]
fn the_slot_watcher_reads_each_status_word_as_it_always_has() {
    for (word, outcome) in SLOT_READING {
        for spelled in [word.to_string(), word.to_uppercase(), format!("  {word}\t")] {
            assert_eq!(SlotOutcome::from_status(&spelled), outcome, "{spelled:?}");
            assert_eq!(outcome.is_settled(), outcome != SlotOutcome::InFlight);
        }
    }
    assert_eq!(SlotOutcome::from_status("   "), SlotOutcome::InFlight);
    assert_eq!(SlotOutcome::from_status("accepted."), SlotOutcome::InFlight);
}

/// The two readings are two tables only because four words are read
/// differently. This is the whole list; resolving one is one edit to the row.
#[test]
fn the_two_readings_differ_on_exactly_these_words() {
    let as_slot = |phase: SubmissionPhase| match phase {
        Queued | Running | Unknown => SlotOutcome::InFlight,
        Accepted => SlotOutcome::Accepted,
        Rejected => SlotOutcome::Rejected,
        TimedOut => SlotOutcome::TimedOut,
    };
    let differing: Vec<&str> = FLEET_READING
        .iter()
        .map(|(word, _)| *word)
        .filter(|word| {
            as_slot(SubmissionPhase::from_status(word)) != SlotOutcome::from_status(word)
        })
        .collect();
    assert_eq!(
        differing,
        ["complete", "completed", "promotion failed", "superseded"]
    );
}
