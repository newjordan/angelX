//! The Heartglass without a window: the campaign reader's bounds and its
//! refusals, the board's tolerance of missing fields, the folder watcher,
//! the panel's place in the window, the heart's meshes, its stage cycle
//! and its scene. Every cloud and board here is made in code: the real
//! campaign data is unpublished and never enters the repository.

use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use super::campaign::{
    self, Campaign, Cloud, CloudError, MAX_CLOUD_BYTES, MAX_GROUPS, MAX_POINTS, MAX_STAGES,
    Watcher, decode_cloud, parse_board, sha256_hex,
};
use super::heart::{self, Part};
use super::{Grid, Layout, Panel};
use crate::ui::orrery;

// ------------------------------------------------------------ synthetic ----

/// The palette the cartridge writes (its `cloud.py`), with each group's
/// cardiac flag.
pub(super) const GROUPS: [(&str, [u8; 3], bool); 9] = [
    ("myocardium", [255, 64, 88], true),
    ("endocardium", [64, 224, 255], true),
    ("epicardium", [255, 196, 64], true),
    ("progenitors", [176, 112, 255], true),
    ("neural", [80, 140, 255], false),
    ("endoderm", [96, 220, 140], false),
    ("mesoderm", [150, 160, 190], false),
    ("extraembryonic", [110, 110, 130], false),
    ("other", [200, 200, 210], false),
];

/// A small deterministic generator: no clock, no global state.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// One synthetic stage: `(label, kind, board, points)`, each point
/// `(x, y, z, group)`.
pub(super) type StageSpec = (String, u8, String, Vec<([f32; 3], u8)>);

/// A made-up heart-ish stage: a tube of cells bent like a C, `n` of them.
pub(super) fn tube(label: &str, kind: u8, board: &str, n: usize, seed: u64) -> StageSpec {
    let mut rng = Rng(seed | 1);
    let points = (0..n)
        .map(|_| {
            let a = rng.unit() * 4.4 - 2.2;
            let r = 0.35 * rng.unit().sqrt();
            let b = rng.unit() * std::f32::consts::TAU;
            let pos = [
                0.7 * a.sin() + r * b.cos(),
                0.9 * a.cos() * 0.9 - 0.1 + r * b.sin() * 0.5,
                r * b.sin(),
            ];
            (pos, rng.below(GROUPS.len()) as u8)
        })
        .collect();
    (label.to_string(), kind, board.to_string(), points)
}

/// Encode stages exactly as the cartridge's `cloud.encode` does.
pub(super) fn encode(stages: &[StageSpec]) -> Vec<u8> {
    let mut out = b"VECCLD01".to_vec();
    out.extend_from_slice(&(GROUPS.len() as u32).to_le_bytes());
    for (name, rgb, _) in GROUPS {
        out.extend_from_slice(&rgb);
        out.push(name.len() as u8);
        out.extend_from_slice(name.as_bytes());
    }
    out.extend_from_slice(&(stages.len() as u32).to_le_bytes());
    for (label, kind, board, points) in stages {
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
        out.push(*kind);
        out.push(board.len() as u8);
        out.extend_from_slice(board.as_bytes());
        out.extend_from_slice(&(points.len() as u32).to_le_bytes());
        for (pos, group) in points {
            for v in pos {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.push(*group);
            out.push(u8::from(GROUPS[usize::from(*group)].2));
            out.extend_from_slice(&[0, 0]);
        }
    }
    out
}

/// Encode a decoded cloud again, from its own groups.
fn reencode(cloud: &Cloud) -> Vec<u8> {
    let mut out = b"VECCLD01".to_vec();
    out.extend_from_slice(&(cloud.groups.len() as u32).to_le_bytes());
    for g in &cloud.groups {
        out.extend_from_slice(&g.rgb);
        out.push(g.name.len() as u8);
        out.extend_from_slice(g.name.as_bytes());
    }
    out.extend_from_slice(&(cloud.stages.len() as u32).to_le_bytes());
    for s in &cloud.stages {
        out.push(s.label.len() as u8);
        out.extend_from_slice(s.label.as_bytes());
        out.push(u8::from(s.predicted));
        out.push(s.board.len() as u8);
        out.extend_from_slice(s.board.as_bytes());
        out.extend_from_slice(&(s.points.len() as u32).to_le_bytes());
        for p in &s.points {
            for v in p.pos {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&[p.group, u8::from(p.cardiac), 0, 0]);
        }
    }
    out
}

pub(super) fn three_stages() -> Vec<StageSpec> {
    vec![
        tube("E8.25", 0, "", 600, 11),
        tube("E8.75", 0, "", 700, 12),
        tube("E9.5", 0, "", 800, 13),
        tube("E10.5*", 1, "T2:heart:val_extrap", 500, 14),
    ]
}

/// Where in an encoded file the first stage's count of points is, and the
/// first point's group byte.
fn offsets(bytes: &[u8]) -> (usize, usize) {
    let mut at = 8 + 4;
    for (name, _, _) in GROUPS {
        at += 4 + name.len();
    }
    at += 4;
    let label = bytes[at] as usize;
    at += 1 + label + 1;
    let board = bytes[at] as usize;
    at += 1 + board;
    (at, at + 4 + 12)
}

/// A board as the contract writes it, with every field filled.
fn board_json(sha: &str) -> String {
    let board = |key: &str,
                 short: &str,
                 label: &str,
                 heart: bool,
                 ours: Option<f64>,
                 history: &str| {
        let ours = ours.map_or("null".to_string(), |s| {
            format!(r#"{{"score": {s}, "at": "2026-10-21T03:10:00Z", "ve_id": "x", "model": "ot-drift-v3"}}"#)
        });
        format!(
            r#"{{"key": "{key}", "track": "t", "label": "{label}", "short": "{short}", "heart": {heart},
               "floor": 50.0, "baseline": 50.0, "ve_leader": 73.0, "ve_top10": 69.5, "ve_partner": 56.5,
               "yukon_frontier": 50.0, "ours_best": {ours},
               "proxy_best": {{"score": 63.0, "at": "2026-10-21T02:40:00Z", "model": "m"}},
               "budget": {{"used": 1, "limit": 2, "resets": "2026-10-22T00:00:00Z"}},
               "history": [{history}]}}"#
        )
    };
    let boards = [
        board("T1:val", "T1", "Single-cell E10.5", false, None, ""),
        board(
            "T2:heart:val_interp",
            "HI",
            "Heart interp E8.5",
            true,
            None,
            "",
        ),
        board(
            "T2:heart:val_extrap",
            "HX",
            "Heart extrap E10.5",
            true,
            Some(61.2),
            r#"{"at": "2026-10-21T02:40:00Z", "kind": "proxy", "score": 63.0},
               {"at": "2026-10-21T03:10:00Z", "kind": "official", "score": 61.2}"#,
        ),
        board(
            "T2:embryo:val_interp",
            "EI",
            "Embryo interp E7.5",
            false,
            None,
            "",
        ),
        board("T3:gata4", "G4", "Gata4 KO E8.75", true, None, ""),
    ];
    format!(
        r#"{{"schema": "vec.board/1", "updated": "2026-10-20T21:05:00Z",
            "phase": {{"name": "P2", "collab_ends": "2026-10-27T00:00:00Z", "final_due": "2026-12-02T23:59:59Z"}},
            "boards": [{}],
            "quest": {{"vec_data_staged": 1, "vec_heart_proxy_runs": 7, "vec_heart_submitted": 1,
                      "vec_heart_top10": 0, "vec_something_new": 9}},
            "events": [{{"at": "2026-10-21T03:10:00Z", "kind": "scored", "board": "T2:heart:val_extrap",
                         "text": "HX 61.2 (floor 50, leader 73.0)"}}],
            "cloud": {{"file": "heart_cloud.bin", "sha256": "{sha}", "stages": ["E8.25", "E8.75", "E9.5", "E10.5*"]}}}}"#,
        boards.join(",")
    )
}

// ---------------------------------------------------------------- cloud ----

#[test]
fn a_cloud_round_trips_through_the_reader() {
    let stages = three_stages();
    let cloud = decode_cloud(&encode(&stages)).expect("a well formed cloud");
    assert_eq!(cloud.groups.len(), 9);
    assert_eq!(cloud.groups[0].name, "myocardium");
    assert_eq!(cloud.groups[1].rgb, [64, 224, 255]);
    assert_eq!(cloud.stages.len(), 4);
    for (got, (label, kind, board, points)) in cloud.stages.iter().zip(&stages) {
        assert_eq!(&got.label, label);
        assert_eq!(got.predicted, *kind == 1);
        assert_eq!(&got.board, board);
        assert_eq!(got.points.len(), points.len());
        for (p, (pos, group)) in got.points.iter().zip(points) {
            assert_eq!(p.pos, *pos);
            assert_eq!(p.group, *group);
            assert_eq!(p.cardiac, GROUPS[usize::from(*group)].2);
        }
    }
    // An empty cloud (no groups, no stages) is a cloud with nothing in it.
    let mut empty = b"VECCLD01".to_vec();
    empty.extend_from_slice(&0u32.to_le_bytes());
    empty.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        decode_cloud(&empty),
        Ok(Cloud {
            groups: vec![],
            stages: vec![]
        })
    );
}

#[test]
fn the_reader_holds_the_contracts_bounds() {
    let good = encode(&three_stages());
    // Not a cloud.
    assert_eq!(decode_cloud(b""), Err(CloudError::NotCloud));
    assert_eq!(
        decode_cloud(b"VECCLD02\0\0\0\0\0\0\0\0"),
        Err(CloudError::NotCloud)
    );
    // Past 16 MiB, whatever it holds.
    let mut big = good.clone();
    big.resize(MAX_CLOUD_BYTES + 1, 0);
    assert_eq!(decode_cloud(&big), Err(CloudError::TooBig));
    // 65 groups.
    let mut groups = good.clone();
    groups[8..12].copy_from_slice(&(MAX_GROUPS + 1).to_le_bytes());
    assert_eq!(decode_cloud(&groups), Err(CloudError::TooManyGroups));
    // 17 stages.
    let mut stages_at = 12;
    for (name, _, _) in GROUPS {
        stages_at += 4 + name.len();
    }
    let mut many = good.clone();
    many[stages_at..stages_at + 4].copy_from_slice(&(MAX_STAGES + 1).to_le_bytes());
    assert_eq!(decode_cloud(&many), Err(CloudError::TooManyStages));
    // 20,001 points in a stage.
    let (count_at, group_at) = offsets(&good);
    let mut points = good.clone();
    points[count_at..count_at + 4].copy_from_slice(&(MAX_POINTS + 1).to_le_bytes());
    assert_eq!(decode_cloud(&points), Err(CloudError::TooManyPoints));
    // A group index out of range.
    let mut group = good.clone();
    group[group_at] = 9;
    assert_eq!(decode_cloud(&group), Err(CloudError::GroupOutOfRange));
    // A stage kind that is neither observed nor ours.
    let kind_at = count_at - 1 - 1;
    assert_eq!(good[kind_at], 0, "the first stage is observed");
    let mut kind = good.clone();
    kind[kind_at] = 2;
    assert_eq!(decode_cloud(&kind), Err(CloudError::BadKind));
    // A coordinate that is not a number.
    let mut nan = good.clone();
    nan[count_at + 4..count_at + 8].copy_from_slice(&f32::NAN.to_le_bytes());
    assert_eq!(decode_cloud(&nan), Err(CloudError::NotFinite));
    // A name that is not UTF-8.
    let mut name = good.clone();
    name[16] = 0xFF;
    assert_eq!(decode_cloud(&name), Err(CloudError::NotUtf8));
    // A byte past the end.
    let mut trailing = good.clone();
    trailing.push(0);
    assert_eq!(decode_cloud(&trailing), Err(CloudError::TrailingBytes));
    // Every proper prefix is refused, never read in part.
    for end in 0..good.len() {
        assert!(
            decode_cloud(&good[..end]).is_err(),
            "a cloud cut at {end} bytes was accepted"
        );
    }
}

#[test]
fn thousands_of_mutated_clouds_are_refused_or_read_within_bounds() {
    let good = encode(&[
        tube("E8.25", 0, "", 40, 3),
        tube("E9.5*", 1, "T3:gata4", 30, 4),
    ]);
    let mut rng = Rng(0x5EED_4EA2);
    let (mut accepted, mut refused) = (0, 0);
    for round in 0..6000 {
        let mut bytes = good.clone();
        match round % 6 {
            // Flip a few bytes anywhere.
            0 | 1 => {
                for _ in 0..1 + rng.below(4) {
                    let i = rng.below(bytes.len());
                    bytes[i] ^= 1 << rng.below(8);
                }
            }
            // Overwrite a byte with an extreme.
            2 => {
                let i = rng.below(bytes.len());
                bytes[i] = [0x00, 0xFF, 0x7F, 0x80][rng.below(4)];
            }
            // Cut it short.
            3 => {
                let end = rng.below(bytes.len());
                bytes.truncate(end);
            }
            // Insert or remove a run.
            4 => {
                let i = rng.below(bytes.len());
                let n = 1 + rng.below(16);
                if rng.below(2) == 0 {
                    bytes.splice(i..i, (0..n).map(|_| rng.next() as u8));
                } else {
                    bytes.drain(i..(i + n).min(bytes.len()));
                }
            }
            // Lie in a count.
            _ => {
                let i = rng.below(bytes.len().saturating_sub(4));
                bytes[i..i + 4].copy_from_slice(&(rng.next() as u32).to_le_bytes());
            }
        }
        match decode_cloud(&bytes) {
            Ok(cloud) => {
                accepted += 1;
                assert!(cloud.groups.len() <= MAX_GROUPS as usize);
                assert!(cloud.stages.len() <= MAX_STAGES as usize);
                for stage in &cloud.stages {
                    assert!(stage.points.len() <= MAX_POINTS as usize);
                    for p in &stage.points {
                        assert!(usize::from(p.group) < cloud.groups.len());
                        assert!(p.pos.iter().all(|v| v.is_finite()));
                    }
                }
                // What was accepted re-encodes to exactly its own length:
                // nothing was skipped or guessed.
                assert_eq!(
                    reencode(&cloud).len(),
                    bytes.len(),
                    "an accepted mutation reads as a different file"
                );
            }
            Err(_) => refused += 1,
        }
    }
    println!("mutated clouds: {accepted} read within bounds, {refused} refused");
    // Both happen often: a flipped coordinate is still a cloud, a lying
    // count or a cut is not.
    assert!(
        refused > 1500 && accepted > 1500,
        "{accepted} accepted, {refused} refused"
    );
}

// ---------------------------------------------------------------- board ----

#[test]
fn a_full_board_parses_as_the_contract_writes_it() {
    let sha = "a".repeat(64);
    let c = parse_board(board_json(&sha).as_bytes()).expect("the contract's board");
    assert_eq!(c.boards.len(), 5);
    assert_eq!(
        c.boards
            .iter()
            .map(|b| b.short.as_str())
            .collect::<Vec<_>>(),
        ["T1", "HI", "HX", "EI", "G4"]
    );
    assert_eq!(c.boards.iter().filter(|b| b.heart).count(), 3);
    let hx = &c.boards[2];
    assert_eq!(hx.ours_best.as_ref().map(|b| b.score), Some(61.2));
    assert_eq!(
        hx.ours_best.as_ref().map(|b| b.model.as_str()),
        Some("ot-drift-v3")
    );
    assert_eq!(hx.proxy_best.as_ref().map(|b| b.score), Some(63.0));
    assert_eq!(hx.ve_partner, Some(56.5));
    assert_eq!(hx.budget.used, 1);
    assert_eq!(hx.budget.limit, 2);
    assert_eq!(hx.history.len(), 2);
    assert!(!hx.history[0].official && hx.history[1].official);
    assert_eq!(hx.latest_official().map(|m| m.score), Some(61.2));
    // 2026-10-27T00:00:00Z.
    assert_eq!(c.phase.collab_ends, Some(1_793_059_200));
    assert_eq!(c.phase.name, "P2");
    assert_eq!(c.quest.heart_proxy_runs, 7);
    assert_eq!(c.quest.heart_submitted, 1);
    assert_eq!(c.events.len(), 1);
    assert_eq!(c.events[0].board.as_deref(), Some("T2:heart:val_extrap"));
    assert_eq!(c.cloud.as_ref().map(|c| c.stages.len()), Some(4));
    assert_eq!(
        c.cloud.as_ref().map(|c| c.sha256.as_str()),
        Some(sha.as_str())
    );
}

#[test]
fn missing_and_null_fields_read_as_unknown() {
    // Only a key: everything else is unknown, the floor is 50.
    let c = parse_board(br#"{"schema": "vec.board/1", "boards": [{"key": "T1:val"}]}"#)
        .expect("a sparse board");
    let b = &c.boards[0];
    assert_eq!(b.short, "T1:v");
    assert_eq!(b.floor, 50.0);
    assert!(b.ve_leader.is_none() && b.ve_top10.is_none() && b.ve_partner.is_none());
    assert!(b.ours_best.is_none() && b.proxy_best.is_none() && b.history.is_empty());
    assert_eq!(b.budget, campaign::Budget::default());
    assert!(c.cloud.is_none() && c.events.is_empty() && c.phase.collab_ends.is_none());
    // Nulls, wrong types and a best with no score.
    let c = parse_board(
        br#"{"schema": "vec.board/1", "boards": [{"key": "HX", "ve_leader": null, "ve_top10": "71",
            "ours_best": {"score": null}, "proxy_best": {"at": "x"}, "budget": {"used": -1, "limit": "2"},
            "history": [{"kind": "official"}, {"kind": "official", "score": 52.5}, 7]}],
            "cloud": {"sha256": "not-a-digest"}, "phase": {"collab_ends": "soon"}}"#,
    )
    .expect("a board with odd fields");
    let b = &c.boards[0];
    assert!(b.ve_leader.is_none() && b.ve_top10.is_none());
    assert!(b.ours_best.is_none() && b.proxy_best.is_none());
    assert_eq!((b.budget.used, b.budget.limit), (0, 0));
    assert_eq!(b.history.len(), 1);
    assert_eq!(b.history[0].score, 52.5);
    assert!(c.cloud.is_none(), "a digest that is not one names no cloud");
    assert!(c.phase.collab_ends.is_none());
    // Boards without a key are left out; a board list with none is no campaign.
    let c = parse_board(br#"{"schema": "vec.board/1", "boards": [{"label": "x"}, {"key": "G4"}]}"#)
        .unwrap();
    assert_eq!(c.boards.len(), 1);
    assert!(parse_board(br#"{"schema": "vec.board/1", "boards": [{"label": "x"}]}"#).is_none());
}

#[test]
fn what_is_not_a_board_is_no_campaign() {
    for bad in [
        &b""[..],
        b"{",
        b"[]",
        b"null",
        br#"{"boards": [{"key": "T1:val"}]}"#,
        br#"{"schema": "vec.board/2", "boards": [{"key": "T1:val"}]}"#,
        br#"{"schema": "vec.board/1"}"#,
        br#"{"schema": "vec.board/1", "boards": {}}"#,
        br#"{"schema": "vec.board/1", "boards": []}"#,
    ] {
        assert!(
            parse_board(bad).is_none(),
            "{:?} parsed",
            String::from_utf8_lossy(bad)
        );
    }
    // A cut board is no campaign, at every length.
    let full = board_json(&"b".repeat(64));
    for end in (0..full.len() - 1).step_by(7) {
        assert!(
            parse_board(&full.as_bytes()[..end]).is_none(),
            "cut at {end}"
        );
    }
    // Past a megabyte, whatever it holds.
    let mut big = full.clone().into_bytes();
    big.truncate(big.len() - 1);
    big.extend(std::iter::repeat_n(b' ', 1 << 20));
    big.push(b'}');
    assert!(parse_board(&big).is_none());
}

#[test]
fn long_lists_keep_their_newest() {
    let history: Vec<String> = (0..100)
        .map(|i| format!(r#"{{"kind": "proxy", "score": {}}}"#, 40 + i % 30))
        .collect();
    let events: Vec<String> = (0..50)
        .map(|i| format!(r#"{{"kind": "proxy", "text": "event {i}"}}"#))
        .collect();
    let boards: Vec<String> = (0..12).map(|i| format!(r#"{{"key": "B{i}"}}"#)).collect();
    let json = format!(
        r#"{{"schema": "vec.board/1", "boards": [{{"key": "T1", "history": [{}], "label": "{}"}}, {}], "events": [{}]}}"#,
        history.join(","),
        "x".repeat(1000),
        boards.join(","),
        events.join(",")
    );
    let c = parse_board(json.as_bytes()).unwrap();
    assert_eq!(c.boards.len(), campaign::MAX_BOARDS);
    assert_eq!(c.boards[0].history.len(), campaign::MAX_HISTORY);
    assert_eq!(
        c.boards[0].history[0].score,
        40.0 + (36 % 30) as f64,
        "the oldest 36 went"
    );
    assert!(c.boards[0].label.chars().count() <= 160);
    assert_eq!(c.events.len(), campaign::MAX_EVENTS);
    assert_eq!(c.events.last().map(|e| e.text.as_str()), Some("event 49"));
    assert_eq!(c.events[0].text, "event 30");
}

#[test]
fn a_new_official_score_is_noticed_once() {
    let sha = "c".repeat(64);
    let before = parse_board(board_json(&sha).as_bytes()).unwrap();
    assert_eq!(
        before.new_official_since(&before),
        None,
        "nothing new against itself"
    );
    // A second official score on HI.
    let after = board_json(&sha).replacen(
        r#""key": "T2:heart:val_interp", "track": "t", "label": "Heart interp E8.5", "short": "HI", "heart": true,
               "floor": 50.0, "baseline": 50.0, "ve_leader": 73.0, "ve_top10": 69.5, "ve_partner": 56.5,
               "yukon_frontier": 50.0, "ours_best": null,"#,
        r#""key": "T2:heart:val_interp", "track": "t", "label": "Heart interp E8.5", "short": "HI", "heart": true,
               "floor": 50.0, "baseline": 50.0, "ve_leader": 73.0, "ve_top10": 69.5, "ve_partner": 56.5,
               "yukon_frontier": 50.0, "ours_best": {"score": 58.4, "at": "2026-10-21T05:00:00Z"},"#,
        1,
    );
    assert_ne!(after, board_json(&sha), "the board changed");
    let after = parse_board(after.as_bytes()).unwrap();
    let (row, mark) = after
        .new_official_since(&before)
        .expect("HI's first official score");
    assert_eq!(row, 1);
    assert_eq!(mark.score, 58.4);
    assert_eq!(after.new_official_since(&after), None);
    // A proxy score alone is not official.
    let proxied = board_json(&sha).replace(
        r#""proxy_best": {"score": 63.0"#,
        r#""proxy_best": {"score": 66.0"#,
    );
    let proxied = parse_board(proxied.as_bytes()).unwrap();
    assert_eq!(proxied.new_official_since(&before), None);
}

// -------------------------------------------------------------- watcher ----

/// A fresh folder of its own under the test's temporary directory.
fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("heartglass-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write `bytes` to `path` the cartridge's way (tmp + rename), with a
/// modification time of its own, so a poll always sees it changed.
fn write(path: &Path, bytes: &[u8], stamp: u64) {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).unwrap();
    let file = std::fs::File::options().write(true).open(&tmp).unwrap();
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000 + stamp))
        .unwrap();
    drop(file);
    std::fs::rename(&tmp, path).unwrap();
}

#[test]
fn the_watcher_reads_a_campaign_and_its_cloud_only_when_they_change() {
    let dir = folder("watch");
    let mut w = Watcher::new(dir.clone());
    let mut now = Instant::now();
    let tick = |w: &mut Watcher, now: &mut Instant| {
        *now += Duration::from_millis(1100);
        w.poll(*now)
    };
    // Nothing there: no campaign yet.
    assert_eq!(tick(&mut w, &mut now), campaign::Changes::default());
    assert!(w.campaign().is_none() && w.cloud().is_none());

    let cloud_a = encode(&three_stages());
    let sha_a = sha256_hex(&cloud_a);
    write(&dir.join("heart_cloud.bin"), &cloud_a, 1);
    write(&dir.join("board.json"), board_json(&sha_a).as_bytes(), 2);
    let changes = tick(&mut w, &mut now);
    assert!(changes.board && changes.cloud);
    assert_eq!(w.campaign().map(|c| c.boards.len()), Some(5));
    assert_eq!(w.cloud().map(|c| c.stages.len()), Some(4));
    let first = std::sync::Arc::clone(w.cloud().unwrap());

    // Polled again within the second: nothing is looked at.
    assert_eq!(
        w.poll(now + Duration::from_millis(200)),
        campaign::Changes::default()
    );

    // The cloud file changes under a board that still names the old one:
    // it is not read (the board's digest is what says a cloud changed).
    let cloud_b = encode(&three_stages()[..2]);
    write(&dir.join("heart_cloud.bin"), &cloud_b, 3);
    assert_eq!(tick(&mut w, &mut now), campaign::Changes::default());
    assert!(std::sync::Arc::ptr_eq(w.cloud().unwrap(), &first));

    // A board that changes but names the same cloud: the cloud stays as it is.
    let same = board_json(&sha_a).replace("HX 61.2", "HX 61.2 again");
    write(&dir.join("board.json"), same.as_bytes(), 4);
    let changes = tick(&mut w, &mut now);
    assert!(changes.board && !changes.cloud);
    assert!(std::sync::Arc::ptr_eq(w.cloud().unwrap(), &first));

    // The board names the new cloud: read, checked against the digest.
    write(
        &dir.join("board.json"),
        board_json(&sha256_hex(&cloud_b)).as_bytes(),
        5,
    );
    let changes = tick(&mut w, &mut now);
    assert!(changes.board && changes.cloud);
    assert_eq!(w.cloud().map(|c| c.stages.len()), Some(2));

    // A board naming a cloud whose bytes are not those: the shown cloud stays.
    write(
        &dir.join("board.json"),
        board_json(&"d".repeat(64)).as_bytes(),
        6,
    );
    let changes = tick(&mut w, &mut now);
    assert!(changes.board && !changes.cloud);
    assert_eq!(w.cloud().map(|c| c.stages.len()), Some(2));

    // A malformed cloud the board names: no cloud yet, and not read again
    // until the board names another.
    let mut broken = encode(&three_stages());
    broken.push(7);
    write(&dir.join("heart_cloud.bin"), &broken, 7);
    write(
        &dir.join("board.json"),
        board_json(&sha256_hex(&broken)).as_bytes(),
        8,
    );
    let changes = tick(&mut w, &mut now);
    assert!(changes.board && changes.cloud);
    assert!(w.cloud().is_none());
    assert_eq!(tick(&mut w, &mut now), campaign::Changes::default());

    // A malformed board: no campaign, no cloud.
    write(
        &dir.join("board.json"),
        b"{\"schema\": \"vec.board/1\", \"boards\": [",
        9,
    );
    let changes = tick(&mut w, &mut now);
    assert!(changes.board);
    assert!(w.campaign().is_none() && w.cloud().is_none());

    // The board goes away: still nothing, and no panic.
    std::fs::remove_file(dir.join("board.json")).unwrap();
    tick(&mut w, &mut now);
    assert!(w.campaign().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

// --------------------------------------------------------------- layout ----

/// The window sizes the panel is checked in: the nested seat's, laptop and
/// desktop screens, a 4K one, and a tall narrow one.
const WINDOWS: [(u32, u32); 6] = [
    (1576, 880),
    (1600, 900),
    (1920, 1080),
    (2560, 1440),
    (3840, 2160),
    (1100, 1300),
];

/// The four corners of `rect` grown by `grow`, leaned with the panel.
fn leaned_corners(rect: Panel, grow: f32, centre: [f32; 2]) -> [[f32; 2]; 4] {
    let (x0, y0, x1, y1) = (
        rect.x - grow,
        rect.y - grow,
        rect.x + rect.w + grow,
        rect.y + rect.h + grow,
    );
    [[x0, y0], [x1, y0], [x0, y1], [x1, y1]].map(|p| super::leaned_point(p, centre))
}

/// The Orrery's corners grown by `grow`, leaned its own way.
fn orrery_corners(rect: Panel, grow: f32) -> [[f32; 2]; 4] {
    let (x0, y0, x1, y1) = (
        rect.x - grow,
        rect.y - grow,
        rect.x + rect.w + grow,
        rect.y + rect.h + grow,
    );
    let centre = rect.centre();
    [[x0, y0], [x1, y0], [x0, y1], [x1, y1]].map(|p| orrery::leaned_point(p, centre))
}

fn inside(inner: Panel, outer: Panel) -> bool {
    inner.x >= outer.x - 1e-3
        && inner.y >= outer.y - 1e-3
        && inner.x + inner.w <= outer.x + outer.w + 1e-3
        && inner.y + inner.h <= outer.y + outer.h + 1e-3
}

/// The header's rows the panel must stay below, and the composer's above.
const HEADER_PX: f32 = 3.0 * 25.0;
const COMPOSER_PX: f32 = 3.0 * 25.0;

#[test]
fn the_panel_stays_in_the_window_and_off_the_header() {
    for (width, height) in WINDOWS {
        let grid = Grid { width, height };
        let l = Layout::for_grid(grid, None, 5)
            .unwrap_or_else(|| panic!("{width}x{height} floats the panel"));
        let p = l.panel;
        // Its brackets reach 7 px past the plate; the lean turns them.
        for [x, y] in leaned_corners(p, 7.0, p.centre()) {
            assert!(x >= 0.0 && x <= width as f32, "{width}x{height}: x {x}");
            assert!(y >= HEADER_PX, "{width}x{height}: over the header at y {y}");
            assert!(
                y <= height as f32 - COMPOSER_PX,
                "{width}x{height}: over the composer at y {y}"
            );
        }
        // Every part sits inside the plate, top to bottom without overlap.
        let parts = [l.title, l.heart, l.stages, l.axis];
        for part in parts.iter().chain(&l.rows).chain([&l.legend, &l.ticker]) {
            assert!(inside(*part, p), "{width}x{height}: {part:?} outside {p:?}");
        }
        let mut stack: Vec<Panel> = parts.to_vec();
        stack.extend(l.rows.iter().copied());
        stack.extend([l.legend, l.ticker]);
        for pair in stack.windows(2) {
            assert!(
                pair[1].y >= pair[0].y + pair[0].h - 1e-3,
                "{width}x{height}: {pair:?} overlap"
            );
        }
        assert!(l.heart.h >= super::MIN_HEART && l.heart.h <= super::MAX_HEART);
        assert_eq!(l.rows.len(), 5);
        // Right-anchored two cells in (and in by its lean), below the header.
        let lean = p.h * 0.5 * super::TILT.abs().sin();
        assert!((p.x + p.w + lean - (width as f32 - 24.0)).abs() < 1e-3);
        assert!((p.y - 110.0).abs() < 1e-3);
    }
    // In the nested seat (1576 x 880) it is 40 columns wide and the heart
    // gets the room left after the boards.
    let l = Layout::for_grid(
        Grid {
            width: 1576,
            height: 880,
        },
        None,
        5,
    )
    .unwrap();
    assert_eq!(l.panel.w, 576.0);
    assert!(l.panel.x + l.panel.w < 1576.0 - 24.0);
    assert_eq!(l.heart.h, 880.0 - 110.0 - 120.0 - super::fixed_height(5));
}

#[test]
fn small_windows_float_no_panel() {
    // Too narrow, too short, or with no room for the heart over the boards.
    for (width, height) in [(900, 1000), (1600, 560), (1280, 720), (400, 300), (0, 0)] {
        assert_eq!(
            Layout::for_grid(Grid { width, height }, None, 5),
            None,
            "{width}x{height}"
        );
    }
}

#[test]
fn the_panel_stands_beside_the_orrery_or_below_it_never_on_it() {
    for (width, height) in WINDOWS {
        let grid = Grid { width, height };
        let Some(o) = orrery::Panel::for_grid(grid) else {
            continue;
        };
        let Some(l) = Layout::for_grid(grid, Some(o), 5) else {
            continue;
        };
        let p = l.panel;
        let ours = leaned_corners(p, 7.0, p.centre());
        let theirs = orrery_corners(o, 7.0);
        let bounds = |c: [[f32; 2]; 4]| {
            let xs = c.map(|p| p[0]);
            let ys = c.map(|p| p[1]);
            (
                xs.iter().copied().fold(f32::INFINITY, f32::min),
                xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                ys.iter().copied().fold(f32::INFINITY, f32::min),
                ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
            )
        };
        let (ax0, ax1, ay0, ay1) = bounds(ours);
        let (bx0, bx1, by0, by1) = bounds(theirs);
        let apart = ax1 <= bx0 || bx1 <= ax0 || ay1 <= by0 || by1 <= ay0;
        assert!(
            apart,
            "{width}x{height}: the Heartglass {p:?} is on the Orrery {o:?}"
        );
        for [x, y] in ours {
            assert!(
                x >= 0.0 && x <= width as f32 && y >= HEADER_PX && y <= height as f32 - COMPOSER_PX
            );
        }
    }
    // A wide window puts it beside, level with the Orrery's top.
    let grid = Grid {
        width: 1920,
        height: 1080,
    };
    let o = orrery::Panel::for_grid(grid).unwrap();
    let l = Layout::for_grid(grid, Some(o), 5).unwrap();
    assert!((l.panel.y - o.y).abs() < 1e-3 && l.panel.x + l.panel.w < o.x);
}

#[test]
fn rows_follow_the_board_count() {
    let grid = Grid {
        width: 1920,
        height: 1080,
    };
    let five = Layout::for_grid(grid, None, 5).unwrap();
    let six = Layout::for_grid(grid, None, 6).unwrap();
    assert_eq!(six.rows.len(), 6);
    assert!(six.heart.h <= five.heart.h);
    assert_eq!(
        Layout::for_grid(grid, None, 99).unwrap().rows.len(),
        campaign::MAX_BOARDS
    );
}

// ---------------------------------------------------------------- heart ----

fn cloud() -> Cloud {
    decode_cloud(&encode(&three_stages())).unwrap()
}

#[test]
fn a_stage_becomes_slab_meshes_the_gpu_takes() {
    let cloud = cloud();
    for i in 0..cloud.stages.len() {
        let (built, meshes) = heart::stage_meshes(&cloud, i).unwrap();
        let stage = &cloud.stages[i];
        assert_eq!(built.label, stage.label);
        assert_eq!(built.predicted, stage.predicted);
        assert_eq!(built.cells, stage.points.len(), "the tube has no strays");
        assert_eq!(
            built.cardiac,
            stage.points.iter().filter(|p| p.cardiac).count()
        );
        assert_eq!(built.slabs.len(), meshes.len());
        assert!(meshes.len() <= heart::SLABS);
        let mut cores = 0;
        let mut halos = 0;
        let mut ids = std::collections::HashSet::new();
        for (slab, (id, mesh)) in built.slabs.iter().zip(&meshes) {
            assert_eq!(slab.id, *id);
            assert!(ids.insert(*id), "slab ids are unique");
            assert_eq!(mesh.validate(), Ok(()), "slab {id:#x}");
            assert_eq!(mesh.groups.len(), slab.parts.len());
            assert!(slab.band[0] < slab.band[1]);
            for (&(_, count), part) in mesh.groups.iter().zip(&slab.parts) {
                // Eight triangles a gem.
                assert_eq!(count % 24, 0);
                match part {
                    Part::Halo => halos += count / 24,
                    _ => cores += count / 24,
                }
            }
        }
        assert_eq!(cores as usize, stage.points.len(), "one core a cell");
        assert_eq!(halos as usize, built.cardiac, "one glow a cardiac cell");
    }
    assert_eq!(heart::ring_mesh().validate(), Ok(()));
}

#[test]
fn cardiac_cells_keep_their_lineage_colour_and_the_rest_dim() {
    let cloud = cloud();
    let (built, meshes) = heart::stage_meshes(&cloud, 2).unwrap();
    for (slab, (_, mesh)) in built.slabs.iter().zip(&meshes) {
        for (&(first, count), part) in mesh.groups.iter().zip(&slab.parts) {
            for &index in &mesh.indices[first as usize..(first + count) as usize] {
                let c = mesh.colors[index as usize];
                let palette = |rgb: [u8; 3]| rgb.map(|v| f32::from(v) / 255.0);
                match part {
                    Part::Cardiac | Part::Halo => {
                        let known = GROUPS
                            .iter()
                            .filter(|g| g.2)
                            .any(|g| palette(g.1) == [c[0], c[1], c[2]]);
                        assert!(
                            known,
                            "a cardiac gem in {c:?}, not a cardiac lineage's colour"
                        );
                    }
                    Part::Context => {
                        assert!(
                            c[0].max(c[1]).max(c[2]) < 0.5,
                            "a non-cardiac gem as bright as {c:?}"
                        );
                    }
                }
            }
        }
    }
    // Strays far from the heart are left out.
    let mut stages = three_stages();
    stages[0].3.push(([9.0, 0.0, 0.0], 0));
    let cloud = decode_cloud(&encode(&stages)).unwrap();
    let (built, _) = heart::stage_meshes(&cloud, 0).unwrap();
    assert_eq!(built.cells, stages[0].3.len() - 1);
}

#[test]
fn the_cycle_builds_the_first_stage_then_scans_from_each_to_the_next() {
    use heart::{HOLD, SCAN, moment};
    assert_eq!(moment(0.0, 0), None);
    let m = moment(0.0, 3).unwrap();
    assert_eq!((m.from, m.to), (None, 0));
    assert!(m.k == 0.0 && m.scanning());
    let m = moment(SCAN + 0.1, 3).unwrap();
    assert_eq!((m.from, m.to, m.k), (None, 0, 1.0));
    let m = moment(HOLD + SCAN + SCAN * 0.5, 3).unwrap();
    assert_eq!((m.from, m.to), (Some(0), 1));
    assert!((m.k - 0.5).abs() < 1e-3);
    let m = moment(3.0 * (HOLD + SCAN) + 0.1, 3).unwrap();
    assert_eq!((m.from, m.to), (Some(2), 0), "it comes round");
    // One stage builds once and then holds.
    let m = moment(5.0 * (HOLD + SCAN) + 0.1, 1).unwrap();
    assert_eq!((m.from, m.to, m.k), (None, 0, 1.0));
    // A clock that is not a number is the start.
    assert_eq!(moment(f32::NAN, 2).unwrap().to, 0);
}

#[test]
fn a_scan_trades_each_slab_of_the_old_stage_for_the_new() {
    let band = [0.1, 0.3];
    let mut last = 0.0;
    for i in 0..=40 {
        let scan = heart::scan_height(i as f32 / 40.0);
        let incoming = heart::presence(band, scan, true);
        let outgoing = heart::presence(band, scan, false);
        assert!((incoming + outgoing - 1.0).abs() < 1e-5);
        assert!(incoming >= last - 1e-6, "a slab only grows in");
        last = incoming;
    }
    assert_eq!(heart::presence(band, heart::scan_height(0.0), true), 0.0);
    assert_eq!(heart::presence(band, heart::scan_height(1.0), true), 1.0);
    // Lower slabs come in first.
    let scan = heart::scan_height(0.5);
    assert!(heart::presence([-1.0, -0.8], scan, true) > heart::presence([0.8, 1.0], scan, true));
}

#[test]
fn the_scene_draws_the_stage_in_front_and_both_while_scanning() {
    use angel_stage::scene::MeshRef;
    let cloud = cloud();
    let built: Vec<_> = (0..cloud.stages.len())
        .map(|i| heart::stage_meshes(&cloud, i).unwrap().0)
        .collect();
    let ids = |stage: usize| {
        built[stage]
            .slabs
            .iter()
            .map(|s| s.id)
            .collect::<std::collections::HashSet<_>>()
    };
    let meshes = |scene: &angel_stage::scene::Scene| {
        scene
            .nodes
            .iter()
            .filter_map(|n| match n.mesh {
                Some(MeshRef::Uploaded(id)) if id != heart::RING_MESH => Some(id),
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>()
    };
    // Held: stage 1 alone, every slab.
    let held = heart::scene(
        &built,
        heart::Pose {
            t: heart::HOLD + 2.0 * heart::SCAN,
            flare: 0.0,
        },
    );
    assert_eq!(meshes(&held), ids(1));
    assert!(
        held.nodes
            .iter()
            .any(|n| n.mesh == Some(MeshRef::Uploaded(heart::RING_MESH)))
    );
    // Mid-scan: some of stage 1 going, some of stage 2 coming.
    let mid = heart::scene(
        &built,
        heart::Pose {
            t: 2.0 * (heart::HOLD + heart::SCAN) + heart::SCAN * 0.5,
            flare: 0.0,
        },
    );
    let shown = meshes(&mid);
    assert!(!shown.is_disjoint(&ids(1)) && !shown.is_disjoint(&ids(2)));
    assert!(shown.is_subset(&ids(1).union(&ids(2)).copied().collect()));
    // Every slab hangs from the beating root, and the draw list is sound.
    for node in &mid.nodes {
        if let Some(MeshRef::Uploaded(id)) = node.mesh
            && id != heart::RING_MESH
        {
            assert_eq!(node.parent, Some(1));
            assert!(
                node.transform
                    .scale
                    .iter()
                    .all(|s| s.is_finite() && *s > 0.0)
            );
        }
    }
    assert!(!mid.draw_list().is_empty());
    // No stages: the ring idles alone.
    let idle = heart::scene(&[], heart::Pose { t: 3.0, flare: 0.0 });
    assert!(meshes(&idle).is_empty());
    assert_eq!(idle.draw_list().len(), 1);
}

#[test]
fn the_heart_beats_and_swells_more_in_a_flare() {
    let beats: Vec<f32> = (0..400).map(|i| heart::beat(i as f32 * 0.01)).collect();
    let (lo, hi) = beats
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
    assert!(
        (0.0..0.05).contains(&lo) && hi > 0.95 && hi < 1.2,
        "{lo}..{hi}"
    );
    // The trace's spike is on the beat's lub.
    let spike = (0..1000)
        .map(|i| i as f32 * heart::BEAT_PERIOD / 1000.0)
        .fold((0.0, f32::MIN), |best, t| {
            if heart::ecg(t) > best.1 {
                (t, heart::ecg(t))
            } else {
                best
            }
        });
    assert!(
        spike.0 < 0.02 || spike.0 > heart::BEAT_PERIOD - 0.02,
        "the R wave at {}",
        spike.0
    );
    assert!(heart::beat(spike.0) > 0.9);
    let scale = |flare: f32| {
        heart::scene(&[], heart::Pose { t: 0.0, flare }).nodes[0]
            .transform
            .scale[0]
    };
    assert!(scale(1.0) > scale(0.0));
    // The centre of the heart lands inside its viewport.
    let c = heart::project([0.0, 0.0, 0.0], (900, 640)).unwrap();
    assert!(
        (0.3..0.7).contains(&c[0]) && (0.2..0.8).contains(&c[1]),
        "{c:?}"
    );
    let ring = heart::project([0.0, heart::RING_Y, 0.0], (900, 640)).unwrap();
    assert!(
        ring[1] > c[1] && ring[1] < 1.0,
        "the ring is under the heart and in view: {ring:?}"
    );
}

#[test]
fn a_flare_rises_from_its_first_frame_and_runs_its_course() {
    let flare = heart::Flare { at: 10.0, row: 2 };
    // Its first frame is dark but the flare is not over: it rises next.
    assert_eq!(flare.strength(10.0), 0.0);
    assert!(!flare.over(10.0));
    assert!(flare.strength(10.15) > 0.9);
    let mut last = flare.strength(10.15);
    for i in 1..70 {
        let s = flare.strength(10.15 + i as f32 * 0.1);
        assert!(s <= last + 1e-6, "it only fades after its peak");
        last = s;
    }
    assert!(!flare.over(17.9) && flare.over(18.0));
    assert_eq!(flare.strength(18.0), 0.0);
    assert_eq!(flare.strength(9.0), 0.0, "nothing before it began");
}

/// Campaigns with nothing in them still have a sane score range.
#[test]
fn an_empty_campaign_is_handled_everywhere() {
    let c = Campaign {
        updated: None,
        phase: campaign::Phase::default(),
        boards: vec![],
        quest: campaign::Quest::default(),
        events: vec![],
        cloud: None,
    };
    assert_eq!(c.next_reset(), None);
    assert_eq!(c.new_official_since(&c), None);
}

// -------------------------------------------------------------- command ----

#[test]
fn heart_is_a_built_in_command_ahead_of_any_cartridges() {
    use crate::ui::input::{ParsedInput, cartridge_command, parse, slash_command_matches};
    assert!(matches!(parse("/heart").unwrap(), ParsedInput::Heart));
    assert!(matches!(parse("  /heart  ").unwrap(), ParsedInput::Heart));
    // A cartridge's own `/heart` is never reached: the built-in wins.
    assert!(cartridge_command("heart").is_none());
    assert!(slash_command_matches("/hea").contains(&"heart"));
    assert!(
        slash_command_matches("/hea").contains(&"hear"),
        "/hear is still there"
    );
    // `/hear <audio>` keeps its meaning.
    assert!(!matches!(parse("/hear clip.wav"), Ok(ParsedInput::Heart)));
    // Its two looks: floating over the window, and back on the plate; and
    // the floating modules back where they were designed.
    assert!(matches!(parse("/heart float").unwrap(), ParsedInput::HeartLook(true)));
    assert!(matches!(parse("/heart  plate ").unwrap(), ParsedInput::HeartLook(false)));
    assert!(matches!(parse("/heart reset").unwrap(), ParsedInput::HeartReset));
    assert!(matches!(parse("/heart demo").unwrap(), ParsedInput::HeartDemo));
    assert!(!matches!(parse("/heart gold"), Ok(ParsedInput::HeartLook(_))));
}

#[test]
fn the_heart_panel_floats_by_default() {
    assert!(super::DEFAULT_FLOATING);
    assert!(super::floating(), "the Heartglass must start as the floating HUD, not on its plate");
    assert_eq!(super::launch_look("1"), Some(true));
    assert_eq!(super::launch_look("float"), Some(true));
    assert_eq!(super::launch_look("plate"), Some(false), "the plate is an explicit opt-in");
    assert_eq!(super::launch_look(""), None);
}

#[test]
fn the_demo_campaign_is_synthetic_labelled_and_in_bounds() {
    let now = 1_793_000_000;
    let campaign = super::demo::campaign(now);
    assert_eq!(campaign.phase.name, "DEMO", "the panel names what it shows");
    assert_eq!(campaign.boards.len(), 5);
    assert!(
        campaign.events.iter().any(|e| e.text.contains("no VE data")),
        "the ticker says it is a demo"
    );
    for board in &campaign.boards {
        assert!(board.key.starts_with("demo:") && board.label.contains("demo"), "{}", board.key);
        let ours = board.ours_best.as_ref().expect("a demo best").score;
        assert!(ours > board.floor && ours < 100.0);
        assert!(board.history.iter().all(|m| m.score.is_finite()));
        assert!(board.latest_official().is_some());
    }
    assert!(campaign.cloud.is_none(), "the demo names no file cloud");

    let cloud = super::demo::cloud();
    assert_eq!(cloud.groups.len(), 9);
    assert_eq!(cloud.stages.len(), 4);
    assert_eq!(cloud.stages.iter().filter(|s| s.predicted).count(), 1);
    for stage in &cloud.stages {
        assert!(stage.points.len() <= campaign::MAX_POINTS as usize);
        assert!(stage.points.iter().all(|p| {
            p.pos.iter().all(|v| v.is_finite())
                && (p.group as usize) < cloud.groups.len()
                && p.cardiac == (p.group < 4)
        }));
    }
    // The demo cloud survives the contract's own codec, both ways.
    assert_eq!(decode_cloud(&reencode(&cloud)).unwrap(), cloud);
}

#[test]
fn the_demo_watcher_serves_from_memory_and_never_polls_a_folder() {
    let mut watcher = Watcher::demo(1_793_000_000);
    assert!(watcher.is_demo());
    assert!(watcher.campaign().is_some() && watcher.cloud().is_some());
    let changes = watcher.poll(Instant::now());
    assert!(!changes.board && !changes.cloud, "the demo is static");
    assert_eq!(
        watcher.campaign().unwrap().phase.name, "DEMO",
        "a poll must not replace the demo with a folder read"
    );
}

#[test]
fn outside_the_window_heart_says_where_the_panel_lives() {
    // A test run has no window: the toggle changes nothing.
    assert_eq!(super::toggle(), super::Toggled::NoWindow);
    assert!(!super::visible());
    super::launch_if_requested();
    assert!(!super::visible());
    assert_eq!(super::reset_layout(), super::Toggled::NoWindow);
    assert!(!super::take_reset(), "no reset is asked for outside the window");
    assert_eq!(super::show_demo(), super::Toggled::NoWindow);
    assert!(!super::demo(), "no demo is armed outside the window");
}
