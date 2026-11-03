use super::*;
use crate::drive::competition::board::SourceAccessClaimV1;
use crate::drive::competition::schema::ComparatorKindV1;

fn comparator() -> ObjectiveComparatorV1 {
    ObjectiveComparatorV1 {
        objective_id: "score".into(),
        version: "1".into(),
        kind: ComparatorKindV1::HigherIsBetter,
    }
}

fn entry(id: &str, score: &str, personal: bool, source: bool) -> BoardEntryV1 {
    BoardEntryV1 {
        entry_id: id.into(),
        participant_id: format!("participant-{id}"),
        submission_id: Some(format!("submission-{id}")),
        rank: None,
        score: ScoreV1::new(score).unwrap(),
        personal,
        source: source.then(|| SourceAccessClaimV1 {
            source_id: format!("source-{id}"),
            commit_oid: format!("commit-{id}"),
            tree_oid: format!("tree-{id}"),
            workspace_sha256: crate::knowledge::cut::sha256_hex(
                format!("workspace-{id}").as_bytes(),
            ),
            access_proof_sha256: crate::knowledge::cut::sha256_hex(
                format!("proof-{id}").as_bytes(),
            ),
        }),
    }
}

fn reduce(entries: &[BoardEntryV1], epoch: u64, observation: &str) -> FrontierV1 {
    reduce_frontier(
        entries,
        &comparator(),
        epoch,
        observation,
        |contract, candidate, baseline| Ok(contract.compare(candidate, baseline).unwrap()),
    )
    .unwrap()
}

/// FG-BASE-020 / AC-04.
#[test]
fn global_personal_and_accessible_frontiers_remain_distinct() {
    let frontier = reduce(
        &[
            entry("external", "100", false, false),
            entry("personal", "90", true, false),
            entry("accessible", "80", false, true),
        ],
        1,
        "observation-one",
    );
    assert_eq!(frontier.global_target.unwrap().entry_id, "external");
    assert_eq!(frontier.personal_best.unwrap().entry_id, "personal");
    assert_eq!(
        frontier.source_accessible_base.unwrap().entry_id,
        "accessible"
    );
}

/// FG-BASE-021 / AC-04.
#[test]
fn inaccessible_global_leader_changes_target_not_source_base() {
    let first = reduce(
        &[
            entry("external-old", "100", false, false),
            entry("accessible", "80", false, true),
        ],
        1,
        "observation-one",
    );
    let moved = reduce(
        &[
            entry("external-new", "110", false, false),
            entry("accessible", "80", false, true),
        ],
        2,
        "observation-two",
    );
    assert_eq!(
        moved.global_target.as_ref().unwrap().entry_id,
        "external-new"
    );
    assert_eq!(
        first.source_accessible_base.as_ref().unwrap().source,
        moved.source_accessible_base.as_ref().unwrap().source
    );
    assert_eq!(
        moved.source_accessible_base.as_ref().unwrap().entry_id,
        "accessible"
    );
    assert!(
        !first.same_decision(&moved),
        "target movement advances decision"
    );
}

#[test]
fn source_selection_metadata_is_not_part_of_decision_digest() {
    let entries = [entry("accessible", "80", true, true)];
    let first = reduce(&entries, 1, "observation-one");
    let refresh = reduce(&entries, 99, "observation-later");
    assert!(first.same_decision(&refresh));
    assert_eq!(
        first.decision_sha256(&comparator()).unwrap(),
        refresh.decision_sha256(&comparator()).unwrap()
    );
}

/// The comparator the tests share, as the plain closure `reduce_frontier`
/// takes. Scores are canonical decimals, so comparison cannot fail here.
fn compare(
    contract: &ObjectiveComparatorV1,
    candidate: &ScoreV1,
    baseline: &ScoreV1,
) -> Result<std::cmp::Ordering, AdapterFailureV1> {
    Ok(contract.compare(candidate, baseline).unwrap())
}

/// One board row with an explicit rank, so tie-breaking is exercised.
fn ranked(id: &str, score: &str, rank: Option<u64>, personal: bool, source: bool) -> BoardEntryV1 {
    let mut row = entry(id, score, personal, source);
    row.rank = rank;
    row
}

/// The parity table: hand-picked edges first, then generated boards that vary
/// scores, ranks, duplicate ids, id order and the two scopes.
fn frontier_boards() -> Vec<(String, Vec<BoardEntryV1>)> {
    let mut boards: Vec<(String, Vec<BoardEntryV1>)> = Vec::new();
    boards.push(("empty".into(), Vec::new()));
    boards.push(("lone-row".into(), vec![entry("only", "7", false, false)]));
    boards.push((
        "lone-claimed-row".into(),
        vec![ranked("only", "7", Some(0), true, true)],
    ));
    boards.push((
        "score-tie-different-ranks".into(),
        vec![
            ranked("slow", "5", Some(30), false, false),
            ranked("fast", "5", Some(2), false, false),
        ],
    ));
    boards.push((
        "score-tie-missing-rank-loses".into(),
        vec![
            ranked("unranked", "5", None, false, false),
            ranked("ranked", "5", Some(u64::MAX - 1), false, false),
        ],
    ));
    boards.push((
        "score-tie-same-rank-breaks-on-id".into(),
        vec![
            ranked("zeta", "5", Some(1), false, false),
            ranked("alpha", "5", Some(1), false, false),
        ],
    ));
    boards.push((
        "duplicate-entry-ids-keep-the-first".into(),
        vec![
            ranked("same", "5", Some(1), true, true),
            ranked("same", "5", Some(1), true, true),
        ],
    ));
    boards.push((
        "both-unranked-break-on-id".into(),
        vec![
            ranked("beta", "6", None, false, true),
            ranked("alpha", "6", None, false, true),
        ],
    ));
    boards.push((
        "global-and-personal-disagree".into(),
        vec![
            entry("external", "100", false, false),
            entry("personal", "90", true, false),
            entry("accessible", "80", false, true),
        ],
    ));
    boards.push((
        "higher-is-not-always-the-leader".into(),
        vec![
            ranked("a", "3", Some(9), false, true),
            ranked("b", "9", Some(4), true, true),
            ranked("c", "4", Some(1), true, false),
            ranked("d", "9", None, false, false),
        ],
    ));
    // Lexicographic ids, not numeric ones: "10" sorts before "9".
    for (name, ids) in [
        ("numeric-looking-ids", ["10", "9", "100", "1"]),
        ("mixed-case-ids", ["B", "a", "A", "b"]),
        ("punctuated-ids", ["a-b", "a_b", "a.b", "a0b"]),
    ] {
        boards.push((
            name.into(),
            ids.iter()
                .enumerate()
                .map(|(index, id)| {
                    ranked(
                        id,
                        "3",
                        Some(if index % 2 == 0 { 2 } else { 1 }),
                        index % 3 == 0,
                        index % 2 == 1,
                    )
                })
                .collect(),
        ));
    }
    // Generated boards: rotations of scores, ranks and scopes over two widths.
    let ids = ["a", "b", "c", "d", "e", "f"];
    let scores = ["9", "5", "5", "1", "9", "5"];
    let ranks = [Some(0), Some(2), None, Some(1), None, Some(2)];
    for width in [1usize, 2, 3, 4, 5, 6] {
        for shift in 0..width {
            let board = (0..width)
                .map(|step| {
                    let at = (step + shift) % ids.len();
                    ranked(
                        ids[at],
                        scores[(at + shift) % scores.len()],
                        ranks[(at + shift * 2) % ranks.len()],
                        (at + step) % 2 == 0,
                        (at + shift) % 3 == 0,
                    )
                })
                .collect();
            boards.push((format!("generated-{width}-{shift}"), board));
        }
    }
    boards
}

/// The shipped leg and the native scan must agree on every selection, and the
/// leg must be the one that answered.
#[test]
fn live_frontier_leg_matches_every_native_selection() {
    use crate::agent::harness::live_policy::{LegSource, decide_shipped};
    let boards = frontier_boards();
    assert!(boards.len() >= 30, "the parity table needs real coverage");
    let mut checked = 0;
    for (name, board) in &boards {
        let contract = comparator();
        let native = select_native(board, &contract, &compare).unwrap();
        let input = frontier_input(board, &contract, &compare).unwrap();
        let (selection, source) = decide_shipped("frontier", &input, |selection: &Selection| {
            selection.check(board)
        });
        assert_eq!(selection, native, "{name}");
        assert!(
            matches!(source, LegSource::Shipped { .. }),
            "{name} kept the {} leg: {source:?}",
            source.as_str()
        );
        checked += 1;
    }
    assert!(checked >= 30, "only {checked} boards were compared");
}

/// A throwaway policy module, so a deliberately out-of-contract leg can be
/// pinned without touching the shipped one.
fn temporary_policy_leg(id: &str, render: &str) -> angel_live::Program {
    let root = std::env::temp_dir().join(format!("angel-frontier-leg-{}-{id}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let module = root.join(id);
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(
        module.join("module.json"),
        format!(
            r#"{{"api_version":1,"id":"{id}","kind":"policy","entry":"index.js","probe":{{}}}}"#
        ),
    )
    .unwrap();
    std::fs::write(module.join("index.js"), render).unwrap();
    let mut registry =
        angel_live::Registry::validated(root, crate::agent::hotload::evaluate, |_, _| Ok(()));
    registry.refresh(true);
    registry
        .snapshot()
        .policies
        .get(id)
        .cloned()
        .unwrap_or_else(|| panic!("fixture leg {id} did not publish"))
}

/// An out-of-contract selection is rejected and the native scan answers, so a
/// bad hot edit never moves the target.
#[test]
fn an_out_of_contract_live_frontier_keeps_the_native_selection() {
    use crate::agent::harness::live_policy::{LegSource, decide_with};
    let board = vec![
        entry("external", "100", false, false),
        entry("personal", "90", true, false),
        entry("accessible", "80", false, true),
    ];
    let contract = comparator();
    let native = select_native(&board, &contract, &compare).unwrap();
    let input = frontier_input(&board, &contract, &compare).unwrap();
    for (id, render) in [
        (
            "frontier-index-beyond-board",
            "module.exports={apiVersion:1,render:()=>({global:9,personal:null,source:null})}",
        ),
        (
            "frontier-seat-wrong-scope",
            "module.exports={apiVersion:1,render:()=>({global:null,personal:0,source:null})}",
        ),
        (
            "frontier-missing-seats",
            "module.exports={apiVersion:1,render:()=>({global:0})}",
        ),
    ] {
        let leg = temporary_policy_leg(id, render);
        let (selection, source) = decide_with(
            Some(leg),
            "frontier",
            &input,
            |selection: &Selection| selection.check(&board),
            || select_native(&board, &contract, &compare).unwrap(),
        );
        assert!(
            matches!(source, LegSource::LiveFallback { .. }),
            "{id} reported {source:?}"
        );
        assert_eq!(selection, native, "{id}");
    }
}

/// Reported, not asserted: the leg is a pure in-process call.
#[test]
fn live_frontier_leg_decision_latency_is_reported() {
    use crate::agent::harness::live_policy::decide_shipped;
    let board: Vec<_> = (0..12)
        .map(|index| {
            ranked(
                &format!("entry-{index}"),
                &format!("{}", 100 - index * 3),
                Some(index as u64 % 4),
                index % 3 == 0,
                index % 2 == 0,
            )
        })
        .collect();
    let contract = comparator();
    let native = select_native(&board, &contract, &compare).unwrap();
    let input = frontier_input(&board, &contract, &compare).unwrap();
    let mut samples: Vec<std::time::Duration> = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let (selection, source) = decide_shipped("frontier", &input, |selection: &Selection| {
            selection.check(&board)
        });
        samples.push(started.elapsed());
        assert_eq!(source.as_str(), "shipped");
        assert_eq!(selection, native);
    }
    samples.sort();
    eprintln!(
        "frontier leg: {:.1} µs median, {:.1} µs p95 over 200 decisions",
        samples[100].as_secs_f64() * 1e6,
        samples[190].as_secs_f64() * 1e6
    );
}

// --- the parity oracle ------------------------------------------------------
//
// The native scan, with `best` and its tie key, moved here out of production:
// the shipped `frontier` leg is the only thing that decides now, and this copy
// exists so the parity table above can prove it agrees.

/// The native scan, kept intact: `best` over the whole board, over the personal
/// entries and over the source-accessible entries.
fn select_native<F>(
    entries: &[BoardEntryV1],
    comparator: &ObjectiveComparatorV1,
    compare: &F,
) -> Result<Selection, AdapterFailureV1>
where
    F: Fn(&ObjectiveComparatorV1, &ScoreV1, &ScoreV1) -> Result<Ordering, AdapterFailureV1>,
{
    let index_of = |entry: &BoardEntryV1| {
        entries
            .iter()
            .position(|candidate| std::ptr::eq(candidate, entry))
    };
    let global = best(entries.iter(), comparator, compare)?.and_then(index_of);
    let personal = best(
        entries.iter().filter(|entry| entry.personal),
        comparator,
        compare,
    )?
    .and_then(index_of);
    let source = best(
        entries.iter().filter(|entry| entry.source.is_some()),
        comparator,
        compare,
    )?
    .and_then(index_of);
    Ok(Selection {
        global: Seat(global),
        personal: Seat(personal),
        source: Seat(source),
    })
}

fn best<'a, I, F>(
    entries: I,
    comparator: &ObjectiveComparatorV1,
    compare: &F,
) -> Result<Option<&'a BoardEntryV1>, AdapterFailureV1>
where
    I: Iterator<Item = &'a BoardEntryV1>,
    F: Fn(&ObjectiveComparatorV1, &ScoreV1, &ScoreV1) -> Result<Ordering, AdapterFailureV1>,
{
    let mut selected: Option<&BoardEntryV1> = None;
    for candidate in entries {
        let replace = match selected {
            None => true,
            Some(current) => match compare(comparator, &candidate.score, &current.score)? {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => tie_key(candidate) < tie_key(current),
            },
        };
        if replace {
            selected = Some(candidate);
        }
    }
    Ok(selected)
}

fn tie_key(entry: &BoardEntryV1) -> (u64, &str) {
    (entry.rank.unwrap_or(u64::MAX), entry.entry_id.as_str())
}
