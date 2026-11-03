//! A synthetic campaign for showing the Heartglass anywhere: demo
//! videos, screenshots, and a first run before the `vec` cartridge has
//! written a board. Everything here is made up in code — invented
//! scores, a bent tube of generated cells — and labelled demo wherever
//! the panel names what it shows (the phase reads `DEMO`, the ticker
//! says so). No VE data, real or derived, ever passes through here:
//! that data is unpublished and must not be redistributed, which is
//! exactly why this mode exists.
//!
//! The generator is deterministic (a small xorshift, fixed seeds), so
//! every demo run and screenshot shows the same heart. The watcher
//! serves it from memory and never reads the cartridge's folder.

use super::campaign::{
    Best, Board, Budget, Campaign, Cloud, Event, Group, Mark, Phase, Point, Quest, Stage,
};

/// The demo's lineage groups: the documented palette (docs/HEARTGLASS.md)
/// with each group's cardiac flag, as the test fixtures carry it.
const GROUPS: [(&str, [u8; 3], bool); 9] = [
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

/// A made-up heart-ish stage: a tube of cells bent like a C, `n` of them,
/// the same shape the reader's tests generate.
fn tube(label: &str, predicted: bool, board: &str, n: usize, seed: u64) -> Stage {
    let mut rng = Rng(seed | 1);
    let points = (0..n)
        .map(|_| {
            let a = rng.unit() * 4.4 - 2.2;
            let r = 0.35 * rng.unit().sqrt();
            let b = rng.unit() * std::f32::consts::TAU;
            let group = rng.below(GROUPS.len()) as u8;
            Point {
                pos: [
                    0.7 * a.sin() + r * b.cos(),
                    0.9 * a.cos() * 0.9 - 0.1 + r * b.sin() * 0.5,
                    r * b.sin(),
                ],
                group,
                cardiac: GROUPS[usize::from(group)].2,
            }
        })
        .collect();
    Stage {
        label: label.to_string(),
        predicted,
        board: board.to_string(),
        points,
    }
}

/// The demo cloud: three observed stages growing, one predicted.
pub(crate) fn cloud() -> Cloud {
    Cloud {
        groups: GROUPS
            .iter()
            .map(|(name, rgb, _)| Group {
                name: name.to_string(),
                rgb: *rgb,
            })
            .collect(),
        stages: vec![
            tube("E8.25", false, "", 1800, 11),
            tube("E8.75", false, "", 2200, 23),
            tube("E9.5", false, "", 2600, 37),
            tube("E10.5*", true, "HX", 2800, 53),
        ],
    }
}

/// A demo board with an invented rising history ending near `ours`.
fn board(
    now: i64,
    key: &str,
    label: &str,
    short: &str,
    heart: bool,
    leader: f64,
    top10: f64,
    ours: f64,
) -> Board {
    let day = 86_400;
    Board {
        key: key.to_string(),
        label: label.to_string(),
        short: short.to_string(),
        heart,
        floor: 50.0,
        baseline: Some(50.0),
        ve_leader: Some(leader),
        ve_top10: Some(top10),
        ve_partner: Some(ours + 0.8),
        yukon_frontier: Some(ours + 0.8),
        ours_best: Some(Best {
            score: ours,
            at: Some(now - day / 4),
            model: "demo model".to_string(),
        }),
        proxy_best: Some(Best {
            score: ours - 12.0,
            at: Some(now - day / 8),
            model: "demo proxy".to_string(),
        }),
        budget: Budget {
            used: 3,
            limit: 8,
            resets: Some(now + 6 * 3600),
        },
        history: vec![
            Mark {
                at: Some(now - 5 * day),
                official: false,
                score: 49.0,
            },
            Mark {
                at: Some(now - 4 * day),
                official: true,
                score: ours - 14.0,
            },
            Mark {
                at: Some(now - 2 * day),
                official: true,
                score: ours - 6.0,
            },
            Mark {
                at: Some(now - day / 4),
                official: true,
                score: ours,
            },
        ],
    }
}

/// The demo campaign at `now` (Unix seconds): five invented boards, all
/// above their (invented) 10th-place lines, clearly a demonstration.
pub(crate) fn campaign(now: i64) -> Campaign {
    Campaign {
        updated: Some(now),
        phase: Phase {
            name: "DEMO".to_string(),
            collab_ends: Some(now + 2 * 86_400),
            final_due: Some(now + 30 * 86_400),
        },
        boards: vec![
            board(
                now,
                "demo:t1",
                "Single-cell E10.5 (demo)",
                "T1",
                false,
                80.0,
                74.0,
                77.0,
            ),
            board(
                now,
                "demo:heart-interp",
                "Heart interp E8.5 (demo)",
                "HI",
                true,
                76.0,
                72.0,
                75.0,
            ),
            board(
                now,
                "demo:heart-extrap",
                "Heart extrap E10.5 (demo)",
                "HX",
                true,
                74.0,
                67.0,
                71.0,
            ),
            board(
                now,
                "demo:embryo-interp",
                "Embryo interp E7.5 (demo)",
                "EI",
                false,
                79.0,
                76.0,
                80.0,
            ),
            board(
                now,
                "demo:gata4",
                "Gata4 KO E8.75 (demo)",
                "G4",
                true,
                82.0,
                80.0,
                81.0,
            ),
        ],
        quest: Quest {
            data_staged: 1,
            heart_proxy_runs: 42,
            heart_submitted: 7,
            heart_top10: 3,
        },
        events: vec![Event {
            at: Some(now),
            kind: "note".to_string(),
            board: None,
            text: "DEMO - synthetic campaign, no VE data".to_string(),
        }],
        cloud: None,
    }
}
