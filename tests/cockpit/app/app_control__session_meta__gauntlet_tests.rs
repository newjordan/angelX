use super::*;

/// A stand-in club: a label + whether it's a synthesizing meta-driver (the
/// selection logic only reads those two signals).
struct FakeClub {
    label: &'static str,
    synth: bool,
}
impl crate::agent::club::Club for FakeClub {
    fn respond(&self, _: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn label(&self) -> &str {
        self.label
    }
    fn reports_to_palace(&self) -> bool {
        self.synth
    }
}
fn club(label: &'static str) -> Arc<dyn crate::agent::club::Club> {
    Arc::new(FakeClub {
        label,
        synth: false,
    })
}
fn synth_club(label: &'static str) -> Arc<dyn crate::agent::club::Club> {
    Arc::new(FakeClub { label, synth: true })
}

#[test]
fn chroniclers_drop_practice_sota_and_synths_dedup_and_lead_with_in_hand() {
    let roster = vec![
        club("gemma4"),
        club("practice"),
        club("gpt-5-codex"), // SOTA — metered, dropped
        synth_club("swarm"), // meta-driver — a whole fan-out, dropped
        club("nex2-mini"),
        club("gemma4"), // dup, dropped
        club("qwen3"),
    ];
    let picked = select_chroniclers(roster, "nex2-mini", 6);
    let labels: Vec<&str> = picked.iter().map(|c| c.label()).collect();
    assert_eq!(labels, vec!["nex2-mini", "gemma4", "qwen3"]);
}

#[test]
fn chroniclers_respect_the_cap() {
    let roster = vec![club("a"), club("b"), club("c"), club("d")];
    let picked = select_chroniclers(roster, "b", 2);
    // in-hand first, then the cap.
    assert_eq!(picked.len(), 2);
    assert_eq!(picked[0].label(), "b");
}

#[test]
fn note_charts_the_party_and_footnotes_the_absent() {
    let charted = vec![
        (
            "gemma4".to_string(),
            crate::stage::questmap::SAMPLE_TRACE.to_string(),
        ),
        (
            "qwen3".to_string(),
            crate::stage::questmap::SAMPLE_TRACE.to_string(),
        ),
    ];
    let skipped = vec!["atlas (unreachable)".to_string()];
    let note = gauntlet_note(&charted, &skipped);
    assert!(note.contains("THE GAUNTLET"));
    assert!(note.contains("gemma4") && note.contains("qwen3"));
    assert!(note.contains("sat out: atlas (unreachable)"));
}

#[test]
fn note_degrades_when_nobody_answers() {
    let note = gauntlet_note(&[], &["atlas (unreachable)".to_string()]);
    assert!(note.contains("no club left a chronicle"));
    assert!(note.contains("atlas (unreachable)"));
}
